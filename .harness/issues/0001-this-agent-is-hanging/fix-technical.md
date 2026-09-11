## What was wrong

On Windows, `ResizePseudoConsole` called while conhost is still on its
inherit-cursor handshake wedges the console. `portable_pty` creates every
pseudoconsole with `PSEUDOCONSOLE_INHERIT_CURSOR`, so conhost sends
`ESC [ 6 n` and holds the child until it reads a cursor report on its input.
The harness answers that query correctly (`instances.rs`, `answer_cursor_query`
— I verified the bytes, the ESC is there). But the desktop pane sends a
`Resize` over the WebSocket the moment its xterm mounts, a few milliseconds
after the spawn, and `Proc::resize` passed it straight to
`ResizePseudoConsole`. When that lands inside the handshake, conhost never
takes the answer, never releases the child, and the pane stays black. The
agent's whole terminal log is the four bytes of the query.

`claude.exe` is a Node process and attaches slowly, so its handshake window is
wide enough for the pane's resize to land inside it; `cmd.exe` attaches in
under 5 ms and never hits it. On a loaded machine the window is wider still —
hence the burst of twelve hangs in one minute at 14:22 today.

The previous fix (the cursor-report watchdog, 40 × 250 ms) could not help:
it repeats an answer to a console that is wedged, not one that is waiting.

Evidence: 24 instance logs in `%APPDATA%\ClaudeHarness\logs` from today are
exactly `1b 5b 36 6e`, including the fable/xhigh agent in the screenshot
(73c85e96…, 14:35:14).

## What changed

`crates/harness-runtime/src/instances.rs`:

- `Proc` gains `handshake_done` (the reader thread's existing `moved_on`
  flag, now shared) and `pending_resize: Mutex<Option<(u16, u16)>>`.
- `Proc::resize` on Windows parks the size while `!handshake_done` instead of
  applying it; the actual `ResizePseudoConsole` call moved to `apply_resize`.
  After parking it re-checks the flag and flushes itself if the reader
  finished in between — without that re-check a parked resize could be
  orphaned (found by the new test under parallel load).
- `flush_pending_resize` applies whatever is parked; idempotent.
- The reader thread holds a `Weak<Proc>` (no cycle) and flushes on the
  `moved_on` false→true transition. The watchdog flushes on every exit path
  as a backstop.
- Non-Windows is unchanged: resize applies immediately.
- `handshake_done()` and `size()` accessors, for the test.

## How it was checked

Reproduced first, with a throwaway test spawning the real `claude.exe
--version` 30× at 42×38 (not committed — it depends on a user path):

- no resize: **0/30** hung
- `resize()` 0 ms after spawn, before the fix: **4/30** and **5/30** hung,
  each with the exact `1b5b366e` 4-byte log
- same, after the fix: **0/30, 0/30, 0/30**, and the 6 s timeouts vanished

Committed regression test
`instances::tests::a_resize_during_the_handshake_is_parked_and_then_applied`:
spawns `cmd.exe`, resizes immediately, asserts the size is *not* applied
during the handshake, then that it *is* applied and the child ran. Confirmed
the parked branch is hit every run. Stable over 6 full-suite runs.

- `cargo test -p harness-runtime -p harness-core`: 95 core + 94 runtime pass,
  `end_to_end` 17/17.
- One failure, pre-existing and unrelated: `costs::tests::round_trips_through_the_log_file`
  reads the real `costs.jsonl` (27 rows vs 2). It fails identically on the
  baseline with my change stashed. Worth its own fix.

Rust changes need the desktop app rebuilt and restarted; the running one
still has the old code.

## Still open

- The five instances currently in `phase=launching` with large logs are a
  *different* issue: Claude Code ran fine but no hook moved them out of
  Launching. Not the reported bug; not touched here.
- `costs` test isolation, as above.
- `claude.exe` was modified at 14:36 today (auto-update) — the exact minute
  the issue was filed. Coincidental as far as I can tell, since the hangs
  predate it, but worth knowing.