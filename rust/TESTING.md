# Rust port — test plan

Everything that needs to be exercised before the Rust app (`rust/`) can be
trusted as a replacement for the Python app (branch `ui`). Nothing here has
been run "for real" beyond the smoke checks listed at the end; the whole list
is the sweep we agreed to do once the implementation was finished.

Reference for intentional differences: `docs/rust_port/KNOWN_ISSUES.md`.

---

## 0. Setup

```
cd rust
cargo build --release -p xpr-app          # ~2 min clean; toolchain 1.98 msvc
cargo run --release -p xpr-app            # same config/data dirs as the Python app
cargo run --release -p xpr-app -- --debug # debug logging
```

`target/release/xpr-app.exe` is self-contained (~24 MB): the game data
(`raw_pkmn_data/**/*.json`, deflated) and the images (`icons/**`, box art
and Pokémon icons prescaled to 160 / 64 px) are compiled in by the crates'
`build.rs`, so the exe can be copied anywhere on its own. Custom gens, saved
routes and the config stay on disk as before. Editing a data or image file
triggers the rebuild automatically. `cargo test -p xpr-app --test
embedded_data` checks the pack with a `raw_pkmn_data` path that does not
exist.

Standalone program (what a release ships): `py -3.14 rust/windows_build.py
[--smoke]` builds the release exe, checks that it imports only Windows
system DLLs (the MSVC CRT is linked statically through
`rust/.cargo/config.toml`, so no VC++ redistributable is needed), copies it
to `dist/rust/pkmn_xp_router.exe` and zips it as
`dist/rust/windows_pkmn_xp_router_<APP_VERSION>.zip` — one exe at the top
level, the layout the old Python updater and `xpr-update` require. `--smoke`
then runs the packaged exe from an empty temp directory (nothing from the
repo in reach, a scratch config dir, `XPR_SMOKE_NEW_ROUTE=Yellow|Charmander`)
and fails unless it exits cleanly with a screenshot and no `ERROR` in its
log. `--no-build` packages the existing `target/release/xpr-app.exe`.

Isolated run (does not touch the real config / routes):

```
mkdir C:\tmp\xpr-cfg C:\tmp\xpr-data
copy config.json C:\tmp\xpr-cfg\config.json     # or let the app create one
# set "user_data_location": "C:/tmp/xpr-data" and "auto_load_most_recent_route": true in it
set XPR_GLOBAL_CONFIG_DIR=C:\tmp\xpr-cfg
set XPR_DISABLE_AUTO_UPDATE=1
cargo run --release -p xpr-app
```

Test-hook environment variables (all optional):

| Var | Effect |
|---|---|
| `XPR_GLOBAL_CONFIG_DIR` | where `config.json`, the log and `xpr_route_index.json` live |
| `XPR_DISABLE_AUTO_UPDATE=1` | no request to GitHub at start, updates count as not possible (nothing prompted) |
| `XPR_GAMEHOOK_URL` | recorder base URL (default `http://localhost:8085`) |
| `XPR_SMOKE_SCREENSHOT=<png>` | capture the window ~4 s after start, then exit |
| `XPR_SMOKE_ROUTE=<route name>` / `XPR_SMOKE_NEW_ROUTE=<version>\|<solo mon>` | with a smoke screenshot: load that saved route / start a fresh route from the built-in data instead of the auto-load preference |
| `XPR_SMOKE_ACTION=battle\|battle_last\|newroute\|summary\|inline\|candy` | drive the UI into a state before the smoke capture; `candy` clicks "+" candy six times on the biggest fight (or `XPR_SMOKE_FIGHT=<substring>`) |
| `XPR_FRAME_LOG=1` | log every frame slower than 1 ms (with the route-list / details draw split) and every route-list rebuild |

Python side for comparisons: `py -3.14 main.py` (the plain `python` on PATH
is the devkitPro one).

---

## 1. Automated

| Command | What it covers | Expected |
|---|---|---|
| `cd rust && cargo test --workspace` | unit tests in every crate; 4 route regression tests (`xpr-engine/tests/route_processing.rs`, incl. KI-1 = 3,240); 437 recorded damage-calc cases (`xpr-calc`); ui-kit colour/shortcut/geometry tests | green |
| `py -3.14 docs/rust_port/golden/dump.py OUT --tests` then `cargo run --release -p xpr-golden -- verify OUT` | the 4 test routes: saved bytes, notes export, every event row/tag/error/state | 4/4 identical |
| `py -3.14 docs/rust_port/golden/dump.py OUT --battles --data-dir` then `verify OUT` | **every** saved + outdated route of the real data dir, every trainer/wild battle summary under the real config | all identical (previously: first 200 = 200/200) |
| `verify OUT --filter <substr>` | narrow to one game / route when something differs | — |
| `py -3.14 docs/rust_port/golden/record_damage_cases.py` then `cargo test -p xpr-calc` | re-record damage cases after a Python change | green |
| `cargo run --release -p xpr-golden -- bench <route.json>` | load / recalc / save timing of the biggest routes vs. Python | Rust ≤ Python |
| `cargo run --release -p xpr-app --bin candy_bench -- <route.json> [fight substring] [iterations]` | the pre-fight candy step through the real controllers (mutation + undo snapshot + incremental recalc + summary reload), and the summary refresh / kill search on their own; `--pieces <route.json>` micro-benchmarks the recalc building blocks | a candy step stays under ~12 ms on the 2,800-event routes (see `PORT_STATUS.md`) |
| `XPR_ROUTE_FILES="a.json;b.json" XPR_CUSTOM_GENS_DIR=<dir> cargo test -p xpr-engine --release --test incremental_recalc` | the incremental recalculation against a full one on extra (big / custom-gen) routes; the plain `cargo test` run covers the 4 test routes | green |
| `cargo clippy --workspace` (optional) | lints | no errors |

Re-run the golden verify with the config toggles that change battle output
(`damage_search_depth`, `force_full_search`, `ignore_accuracy_in_damage_calcs`,
`consistent_highlight_threshold`, both highlight strategies,
`test_moves_enabled`) — dump and verify must use the same `config.json`.

---

## 2. Startup and landing page

- [ ] First start with no config dir: `config.json` created with the same keys/defaults as Python; data dir + `saved_routes/`, `outdated_routes/`, `custom_gens/`, `images/` created.
- [ ] Log file rotates on every start (`pkmn_router_logs.log`, `.1` … `.20`, like Python's `RotatingFileHandler(backupCount=20)`).
- [ ] Window geometry / maximised state restored from `tkinter_window_geometry` / `window_state`; saved back on close.
- [ ] Configured font is picked up (Segoe UI on Windows); an unknown font name falls back without crashing.
- [ ] `auto_load_most_recent_route` = true → the most recently modified route opens in the editor; false → landing page.
- [ ] Landing page lists every route in `saved_routes/` with game and species columns; the index file (`xpr_route_index.json`) is written and reused; adding/removing/re-saving a route file externally is picked up on the next start.
- [ ] Sort: Most Recent / Alphabetical / Game. Game filter dropdown. Search box (300 ms debounce). Enter or double-click loads.
- [ ] "New Route" button → new-route page; "Load Route" → load dialog.
- [ ] Big data dir (the user's ~5,000 routes): landing page stays responsive; index scan runs on a thread.

## 3. New route page

- [ ] Game table in official order (Red, Blue, Yellow, Gold, …) with box art; Generation/Platform/Recorder columns; custom gens appear at the end with their base game's art.
- [ ] Filters: game filter, pkmn filter (300 ms), "min battles" base-route filter.
- [ ] Solo Pokémon dropdown is searchable (type to filter, Enter/Tab commit, arrows move highlight).
- [ ] Base route list: "PRESET: …" entries (the `preset_routes/` of the selected game) + existing routes of the same game; the route is created from the preset / copied with the new species.
- [ ] Custom DVs frame: gen 1–2 shows 5 DV fields (HP DV derived, read-only); gen 3+ shows 6 IVs + ability + nature selector (increase/decrease stat buttons); Hidden Power type/power update live.
- [ ] Route name validation (empty name → "No route name" message; duplicate name warning).
- [ ] Create: lands in the editor with the correct init state; `Save` writes a file byte-identical to Python for the same inputs (compare with a Python-created route of the same species/DVs/base route).
- [ ] Custom-gen route: Backport dialog result message when the base route's version differs.

## 4. Editor — route list

- [ ] Columns: Name, Levels Up, Level, %TNL, Exp, Exp/sec, Exp Gain, ToNextLevel, LvlsGained; widths and elision like Qt.
- [ ] Row colours: folder/trainer categories (gym, rival `#12196b`, elite four, champion `#054d3f`, …), highlight colours 1–9, disabled rows greyed, error rows, "Fade Folder Text" toggle, "Highlight Branched Mandatory Battles" toggle.
- [ ] Quantity suffix ("×N", blue link) for items/candies; clicking it opens the inline quantity editor; Enter commits, Escape cancels.
- [ ] Expand/collapse arrows; expand state survives recalc/reload keyed by folder path; double-click.
- [ ] Checkbox (and right-click on a row): folders and groups toggle enabled (state recalcs); items are no-ops (Qt raised).
- [ ] Selection: click, Ctrl+click, Shift+click ranges, click on empty space clears; selected colour `#0078d4`; hover tint.
- [ ] Keyboard while the list is focused: Up/Down move selection (Shift extends), Space toggles enabled, Q/W/E/R/T open quick-add categories, Delete deletes (confirm dialog), Home/End scroll. Keys are ignored while any text field has focus.
- [ ] Hover "+" button on a row opens the inline creator below it (does not select the row).
- [ ] Drag & drop: single and multi-select rows onto another row / into a folder; insert indicator line; drop outside the list cancels.
- [ ] Empty route: "Add New Event" placeholder button.
- [ ] Scrolling to the selection after keyboard nav never scrolls horizontally.
- [ ] Filter bar: every type toggle (Trainer, Rare Candy, TM/HM, Vitamin, Wild Pkmn, Acquire, Purchase, Use, Reorder Bag, Sell, Hold, Level-up move, Save, Heal, Blackout, Evolution, Notes), "Common" and "Reset", the search box (300 ms) — same rows shown as Qt for the same filter (Reorder Bag is Rust only).
- [ ] Run Status chip: "Invalid" when any event errors; clicking it cycles through the invalid events.
- [ ] Bag reorder (gen 1, Rust only): quick-add "Reorder" / inline "Reorder Bag" insert an empty event; its editor lists the pre-event bag with a drag handle and ▲/▼ per row, a change saves after the 2 s delay and the row label reads `Reorder Bag: A (n) <-> B (m)`; the pre-event inventory panel counts slots from 1. Insert a purchase before a reorder: the row turns amber (not red), the run stays Valid, the details panel shows "Warning: … was at slot …", and rearranging + saving clears it. Recording on Yellow: a SELECT swap in the bag records one reorder event; using up the first item (everything shifts up) records none.

## 5. Inline event creator

- [ ] Opens from the "+" hover button, Events → Add New Event, quick-add, or the empty-route button; inserted after the selected event (or at the end of the selected folder).
- [ ] Event type dropdown (Q/W/E/R/T also switch it), every type's fields: trainer (searchable, "Add all trainers" for an area), wild pkmn (species/level/count/trainer-pkmn flag), rare candy (count), vitamins (kind/count), TM/HM (move/source), acquire/purchase/use/sell/hold item (item/count/held), save/heal/blackout, evolution (target), notes.
- [ ] Enter on the last field creates; Tab wraps; Escape discards; the spacer row sits after the target's subtree and before level-up sibling rows.
- [ ] Editing an existing event inline (double-click / edit) pre-fills and replaces it.
- [ ] Created event is selected and the details pane updates.

## 6. Event details (right pane)

- [ ] Splitter drag between list and details; min widths; fraction persisted (500 ms debounce) and restored.
- [ ] Tabs: Pre-State / Battle Summary; "auto switch" checkbox (config `auto_switch`) switches to Battle for trainer/wild fights and back for other events.
- [ ] Pre-state viewer: Pokémon viewer (species, level, exp, held item "None" when absent, moves, ability/nature for gen 3+, stats with badge-boost markers, gen-2 Sp.Def glitch note), stat-exp/EV viewer, badge list, inventory (20 slots, `# 00:` format, money).
- [ ] Editor per event type (trainer: order/exp-split menus, per-mon cards with speed preview; vitamin, rare candy, learn move incl. TM & tutor slot "Move #N (Over X)", wild pkmn, inventory events, location, evolution, notes with visibility mode) — every change saves after 2 s or immediately on Enter/focus loss, and the route recalcs.
- [ ] Notes footer: collapsed / expanded states, visibility modes (always / when space allows / never).
- [ ] Undo (Ctrl+Z / Events → Undo) after each mutation restores the previous tree and selection.

## 7. Battle summary

- [ ] Loads for a selected trainer / wild fight; "empty" state for other events.
- [ ] Controls bar: pre-fight candy stepper (applies on every click, F3/F4; the damage ranges change on the same frame even on the biggest routes; clicks < 250 ms apart are one undo step, slower clicks are separate steps), vitamin steppers per stat (300 ms debounce), held item searchable dropdown (a "find item" event is auto-inserted when the item is not in the bag), HP/Speed chips, weather / terrain / custom-data dropdowns.
- [ ] Candies keep the unsaved selections: pick a setup move / stat stage / weather / screen / intimidate on a fight, click "+" candy, the selection is still there; after the 2 s delayed save it is in the event definition; Undo reverts the candies.
- [ ] Legacy controls section (toggle in the Battle Summary menu).
- [ ] Mon pair cards: header speed colours (faster blue / tie yellow / slower red), stat-stage setup dropdowns for both sides, setup moves, intimidate toggles, mimic selection, transform, collapsed toggle, drag grip reorder (also "reorder matchup" action).
- [ ] Damage columns: range, kill %, N-hit kill, recoil line, best-move flags, highlight colours 1/2/3, fade for non-highlighted moves; both highlight strategies (Fastest Kill / Consistent) and the consistent threshold dialog; test-move slots (Toggle Test Moves) with searchable dropdowns; assign move via tutor dialog.
- [ ] Export button per matchup → matchup export dialog → PNG in the image folder (full / player / enemy).
- [ ] Message box formatting (`format_message`, recoil) matches Python for a few known fights (compare screenshots side by side).
- [ ] Config → Battle Config dialog (search depth, force full search, ignore accuracy) refreshes the open summary.

## 8. Menus, dialogs and shortcuts

Every menu item, with its default shortcut; each must do the same thing as the Qt app.

**File**: Customize DVs (Ctrl+X) · New Route (Ctrl+N) · Load Route (Ctrl+L) · Save Route (Ctrl+S) · Close Route (Ctrl+Shift+C) · Auto-load most recent (F2, check) · Export Notes (Ctrl+Shift+W) · Screenshot Event List (F5) · Screenshot Battle Summary (F6) · Screenshot Player Ranges (F7) · Screenshot Enemy Ranges (F8) · Open Image Folder (F12) · Config Font (Ctrl+Shift+D) · Custom Gens (Ctrl+Shift+E) · App Config (Ctrl+Shift+A) · Open Data Folder (Ctrl+Shift+O) · Keyboard Shortcuts (Ctrl+Shift+K).

**Events**: Add New Event · New Route Based on Current Route · Undo (Ctrl+Z) · Move Event Up/Down (Ctrl+E / Ctrl+D) · Move Event Up/Down To Next Folder (Ctrl+Shift+E / Ctrl+Shift+D) · Enable/Disable (Ctrl+C) · Toggle Highlight (Ctrl+V) · Transfer Event · Delete Event (Ctrl+B, Delete) · Highlight Branched Mandatory Battles (check) · Fade Folder Text (check) · Setup Summary · Run Summary.

**Highlight**: Highlight 1–9 (Shift+1 … Shift+9) · Configure Colors (highlight palette dialog incl. "Reset to Defaults").

**Folders**: New Folder (Ctrl+Shift+Alt+F) · Rename Cur Folder (Ctrl+Shift+F) · Split Folder (Alt+X, enabled only when the selection allows a split).

**Recording**: Enable/Disable Recording (F1) · Automatically Stop Recording (check) · Final Trainers… (dialog).

**Battle Summary**: Show Notes in Battle Summary (check) · Show Legacy Controls (check) · Player/Enemy Highlight Strategy submenus · Toggle Move Highlights (Shift+F2) · Toggle Fade Moves Without Highlight (Shift+F3) · Toggle Test Moves (Shift+F1) · Configure Consistent Threshold… · Decrement/Increment Pre-Fight Candies (F3/F4) · Toggle Player/Enemy Highlight Strategy (F9/F10).

**Update**: Check for Updates · Never prompt for updates (check) · "Update to vX and restart" (enabled after a deferred check).

Other shortcuts: gym leaders 1–8, Blue 9, Elite Four/Champion Ctrl+1…Ctrl+7 (select the fight), filter toggles Ctrl+F/R/T/G/W, Common Ctrl+A, Reset Ctrl+Shift+R, Home/End.

- [ ] Keyboard Shortcuts dialog: rebind, duplicate detection message, "Reset all" confirmation; bindings persist to `config.json` and take effect immediately; labels in the menus update.
- [ ] Every dialog opens as a modal, Escape closes, values persist: Load Route (list + search, Enter loads), New Folder, Transfer Event (folder list excludes the selection's own folder/descendants), Custom DVs (applies to the current route), Custom Gens (create / load all; reload after a change), Battle Config, Color Config (colours + font; applied live on close), Highlight Colors, App Config (data dir change → routes re-listed), Final Trainers, Matchup Export, Assign Move.
- [ ] Message boxes: unsaved-changes prompts (quit, close route, new route from current), no-route-name, delete confirmation, update found / no update / not possible / error / apply, new-route error, info, backport result, reset-all-shortcuts, duplicate shortcuts.
- [ ] Exception path: an operation that errors shows the exception dialog with the same message text as Python (`@handle_exceptions`).

## 9. Quick add popover

- [ ] Opens above the selected row (Q/W/E/R/T while the list is focused and the selection allows an insert); category keys switch pages; Escape/Space or clicking outside closes.
- [ ] Trainer page: search, multi-partner mode, "Add Area" (folder named `Trip:N`), adds after the selection.
- [ ] Item page: Get / Drop / Use / Hold / TM-HM / Buy / Sell.
- [ ] Wild Pokémon page; misc page (save / heal / blackout / notes / evolution / rare candy / vitamins).
- [ ] Route change / close resets the popover.

## 10. Folders and structure

- [ ] New folder at end / after selection; rename; split at the current event; delete folder (with contents; confirm); purge empty folders on save; transfer events into another folder; move groups up/down across folder boundaries.
- [ ] Nested folders (3+ levels) keep enabled state / expansion / colours through recalc.

## 11. Summaries and screenshots

- [ ] Run Summary: docked panel at the bottom (toggle Dock/Undock), undocked = secondary OS window; gradient cells per stat; Crystal/HeartGold exclusions; elite-four dedupe; held-item row for gen ≠ 1; Export writes the PNG.
- [ ] Setup Summary: secondary window with the text `setup_summary_text` produces; compare with Python for the same route.
- [ ] Screenshots F5–F8 and the matchup export: files named like Python (`<route>_<kind>_<n>.png` etc.) in `images/` (or the custom Image Path from the status bar); toast with "Open Folder"; rounded corners; content crops match Qt (full window / player half / enemy half / event list). Known: opaque background.

## 12. Recorder

Needs GameHook running with a mapper (per gen) and/or Super Shuckie. Without
an emulator, `docs/rust_port/recording/run_pair.py --scenario <name>` replays
a scripted session to both apps and diffs the saved routes (Emerald, Yellow,
Crystal, Platinum and Black scenarios; all byte-identical as of 2026-09-11) —
run it after any change to `xpr-recorder`, the route engine's event
handling, or the route list's folder handling.

- [ ] Record button in the status bar (needs an open route): enables record mode, status text updates (connecting / connected / lost), "↻" reconnect button, automatic reconnect with backoff after killing GameHook.
- [ ] Gen 1 (Yellow/Red/Blue), Gen 2 (G/S/C), Gen 3 (RS/E/FRLG), Gen 4/5: start a fresh game; the recorder inserts trainer fights, wild fights, items get/use/sell/buy, rare candies, vitamins, level-up moves, TM/HM, evolutions, heals/saves/blackouts, in the same order and folders as the Python recorder on the same session. Compare the saved route files afterwards. (Covered without an emulator by the replay harness above.)
- [x] Recorded events land in an enabled, expanded folder that is scrolled into view, with the new event selected (the folder-defaults bug of 2026-09-11; `xpr-app/tests/recorder_folders.rs`).
- [x] Gen 1 `UninitializedState` start condition (KNOWN_ISSUES B-11): ported as-is, recording always starts in the overworld like Python.
- [ ] Missing mapper property → warning in the log, no crash (B-13), for every gen.
- [ ] Automatically Stop Recording + Final Trainers: recording stops after the configured final trainer.
- [ ] Recording error fragments appear as notes events with the error message (`RECORDING_ERROR_FRAGMENT`) and mark the row invalid.
- [ ] Recorder never touches the router from another thread: no UI freezes; the app stays responsive while GameHook is unreachable.

## 13. Update flow

- [ ] Start-up check against GitHub releases (skip with `XPR_DISABLE_AUTO_UPDATE`); "Never prompt" suppresses the dialog; Update → Check for Updates re-checks (button disabled while running).
- [ ] Update found → defer → the menu's "Update to vX and restart" applies it on quit; the app restarts from the new binary; old version cleaned up on the next start (`auto_cleanup_old_version`).
- [ ] "Update not possible" path (e.g. running from `cargo run` / no release asset for this platform).

## 14. Persistence and shutdown

- [ ] Every config toggle in the menus writes `config.json` immediately (compare the file with what Python writes for the same toggles — same keys, same order).
- [ ] Unsaved route → close window → prompt (Save / Don't save / Cancel); Cancel keeps the app open.
- [ ] Route saved from Rust → opened in Python (and vice-versa) without error; `git diff`-style comparison of both saves of the same edits is empty.

## 15. Cross-testing with the Python app (side by side)

1. Same route, same edits in both apps (add a trainer, a candy, a vitamin, a TM, move an event, change DVs, split/rename a folder, toggle a highlight, disable a group) → save both → diff the JSON (expected: identical apart from nothing).
2. Export Notes from both → diff.
3. Battle summary screenshots from both for the same trainers (gen 1, 2, 3, 4, 5 and a custom gen) → visual compare.
4. Run summary + setup summary text → compare.
5. Landing page route list (count, order per sort mode) → compare.
6. Performance: time to open the largest route, recalc after a candy edit, scroll of a 2,000-row route.

## 16. Already smoke-checked during development (screenshots only, isolated config)

Landing page (routes table + sort toggles); editor page (tree colours, quantities, highlights, state viewer, inventory, notes footer, status bar chips); battle summary (8 damage columns with ranges/kill %/recoil/best-move flags, custom-data & stat-stage dropdowns, intimidate, export); new-route page (box-art table, DV frame); docked run summary (gradient cells); inline creator (type dropdown, trainer editor cards). `cargo test --workspace` and the golden verify (204/204 with battles) are green.
