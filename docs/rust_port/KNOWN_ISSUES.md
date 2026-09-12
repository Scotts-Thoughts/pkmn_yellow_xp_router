# Rust port — known issues and intentional divergences

The Python app on branch `ui` is the reference. The Rust port under `rust/`
reproduces its observable behaviour (saved bytes, notes export, every rendered
row, every state, every battle summary) except where listed here. Nothing in
the Python tree was changed by the port, so the "Python-first" items of
`rust_port_plan.md` Appendix B are ported **as-is** (bugs included) instead of
being fixed on both sides.

## Carried-over known issues (plan Appendix E)

| Id | Issue | Where |
|---|---|---|
| KI-1 | Gen-4 prize money: the engine multiplies trainer money by 4 (`gen_four_object.py:503`). The Platinum Chimchar regression route historically expected 3,060 final money; both Python and the port produce **3,240**. Whether the multiplier or the expectation is wrong is unresolved. | `rust/crates/xpr-engine/tests/route_processing.rs` (asserts 3,240, tagged `KI-1`); `xpr-data::exp` doc comment |
| KI-2 | Gen-2 recorder `mon_order` is 0-based: `crystal_states.py` builds it as `[order.index(x) for x in sorted(order)]` where gens 3/4 add `+ 1`, but `get_pokemon_list` reads it as 1-based. A fight whose only enemy position change was to slot 0 records `mon_order=[0]`, which resolves to no Pokémon at all; `[0, 1]` drops the first mon. In Python the empty list makes `EventGroup.apply` raise, the exception is swallowed by `@handle_exceptions`, and the route stops recalculating from that event on (later ROAR updates then fail with `'NoneType' object has no attribute 'solo_pkmn'` error events). The port records the same `mon_order` values; its engine marks the fight as an error instead of aborting the recalculation. Fix on both sides: add `+ 1` in `crystal_states.py` (and `xpr-recorder::games::gen2`). | `docs/rust_port/recording/scenarios/crystal_totodile.py` avoids the case |

## Appendix B ledger — what the port did with each item

| # | Item | Disposition in the port |
|---|---|---|
| 1 | Platinum money test expects 3,060 | KI-1 above; Rust test asserts the current value |
| 2 | `SoloPokemon.serialize` writes `xp` / `xp_to_next_level` twice | Rust writes each key once with the value that "wins" in Python (the second write); the serialised bytes are identical |
| 3 | `Router.replace_event_group` raises for a level-up move whose key *already exists* (inverted check) | **Ported as-is** (Python was not to be changed): editing a level-up move to a level/move that already exists is rejected with the same message |
| 4 | `Router.add_event_object` checks the *first* trainer's `refightable` flag when recording the second trainer of a multi-fight as defeated | **Ported as-is** (`router.rs`, "Appendix B item 4" note) |
| 5 | `EventFolder.contains_id` references a field folders do not have | Fixed by construction (folders are a tree node kind; `contains_id` is implemented for every kind) |
| 6 | Highlight "Reset to Defaults" writes a different palette than `Config.get_highlight_color`'s defaults | Fixed: one source of highlight defaults (`xpr-core::config`) used by the getter, the reset button and `config.json` creation |
| 7 | Font change in the colour dialog only applied on the next launch | Fixed: font and colour changes are applied to the running app when the dialog closes |
| 8 | `MainController._safely_invoke_callbacks` removes any callback that raises once | Not applicable: the egui app has no callback bus; controller mutations return `Result` and errors are reported through the status bar / exception dialog, nothing is unsubscribed |
| 9 | `RecorderController` dispatch logs "removing" but never removes | Same design as 8: recorder → app messages are a queue drained every frame; a failing handler is logged and the queue keeps working |
| 10 | `GameHookClient.unignore_properties` calls a misspelled method | Fixed (`xpr-recorder::gamehook`) |
| 11 | Gen 1 `UninitializedState` compares a property object to a string (always false) | **Ported as-is** (2026-09-11): gen 1 recording always starts in the overworld, like Python; a battle already in progress when recording starts is picked up on its next `battle.type` change. (An earlier build compared the value instead.) The gen 2 `UninitializedState` has the same comparison and is ported the same way |
| 12 | No socket-level reconnect / backoff in the GameHook client | Added: automatic reconnect with exponential backoff; the manual "Reconnect" button is kept |
| 13 | Gen 1–4 recorders crash on a missing mapper path | Every gen skips unmapped / `None` paths with a warning (like the Python gen 5 recorder) |
| 14 | Gen 5's damage calc mutates caller-supplied battle stats in place where gens 2–4 copy them | **Not reproduced.** Rust never mutates the caller's stats. In Python the mutation can leak between successive calls with the same stat block; a difference would show up in gen 5 battle summaries with stat stages / setup moves. The golden corpus run (see `rust/PORT_STATUS.md`) did not hit a case where it changed an output, but it is the first place to look if a gen 5 battle diverges |
| 15 | Every config setter rewrites the whole config file | Config writes are atomic (write-temp-then-rename). Only the splitter fractions are debounced (500 ms); other setters still write immediately, so `config.json` on disk always matches the last change |
| 16 | PyInstaller bundle includes `.claude` / `.harness` JSON | Moot (Cargo build) |
| 17 | Documented damage-calc divergences from the games | Out of scope; the port reproduces the current Python results, including the documented divergences |

## Engine / data / calc divergences (all recorded in source comments)

- **Crashes become errors.** Where Python raises (`IndexError`, `TypeError`,
  `KeyError`, `AttributeError`) inside a route operation, Rust returns an
  `Err` with the Python message text where the message reaches the user, or
  `None` where the value was internal. Route files that crash Python load
  with an error message on the offending event instead.
- **Python `TypeError` comparisons evaluate to false.** A few places compare
  an `int` with `None`/`str` in Python and only "work" because the comparison
  raises and is swallowed by `@handle_exceptions`; Rust evaluates them as
  false and continues.
- **Null `base_xp` in a custom gen** loads as `0` with a warning instead of
  failing at load; fighting such a mon reports the same error Python gives.
- **Duplicate level-up keys.** Loading a route file keeps the *last*
  definition for a duplicated `(level, move)` key; generating the list from
  species data keeps the *first* — the same as Python's dict behaviour in
  both code paths.
- **Gen 3 property quirks** (`apply_stat_mod` rounding, Sinnoh badges stored
  in the Johto badge slots, gen 4 short badge names) are reproduced.
- **Gen 4/5 held-item cache.** Python caches the fling / berry lookups on the
  module; Rust computes them per call. No observable difference.
- **`DamageRange` with an empty damage table** is `None` in Rust (Python
  raises inside the battle summary; the summary then shows no row).
- **`BattleSummary.load_from_state` does not clear the transformed-mon
  list** in Python; Rust keeps that behaviour (see the `NOTE` in
  `xpr-calc::battle_summary`).
- **Route-file text that is not valid UTF-8** is read lossily in Rust
  (Python raises); the golden corpus has no such file.

## Recorder (verified with the replay harness, 2026-09-11)

`docs/rust_port/recording/` replays scripted GameHook sessions to both apps
and diffs the saved routes; the Emerald, Yellow, Crystal, Platinum and Black
scenarios record byte-identical routes. Things that were changed or pinned
down while getting there:

- **New folders are enabled and expanded.** `finalize_new_folder` (the
  recorder's area folders, the New Folder dialog, the quick-add folder) and
  the split-folder command create folders with Python's `EventFolder`
  defaults. An earlier build left both flags unset, which the engine reads as
  disabled + collapsed: recorded events did not count, and the recorder could
  not find them again (`get_previous_event` skips disabled groups).
- **Selecting an event expands its ancestors.** `scroll_to_selected_events`
  expands every collapsed folder above the (last) selected row before
  scrolling, as the Qt list does; a recorded event therefore always comes into
  view even in a folder the user collapsed.
- **One delivery per registration.** Python appends one callback per
  `.change()` call and every generation lists `player.team.0.species` twice in
  `ALL_KEYS_TO_REGISTER` (gens 4/5 also `battle.player.team.0.stats.hp`, gen 4
  `player.team.0.held_item`), so those changes reach the FSM twice. The Rust
  client counts registrations and delivers the change that many times.
- **The processing thread polls like Python's** (`pop(0)` or `sleep(0.1)`)
  instead of waking on push. The latency is load-bearing: machines queue
  events and *then* update their bookkeeping in the same call (an evolution
  found by `update_all_cached_info` is queued before `entered_new_area`), and
  the folder an event lands in depends on the processing thread not getting
  to it first.
- **Reconnect reloads the mapper on a live connection** (Python's `connect()`
  → `_establish_connection()`), so a mapper loaded in GameHook after recording
  started is picked up from the button as well as from `MapperLoaded`.
- **Turning recording off stops the machine at once** (Python's `disconnect()`
  joins it). The Rust connection thread winds down on its own a little later;
  its `shutdown` is a no-op by then, so a session restarted in between keeps
  its Super Shuckie poller.
- **A version without a recorder** (Gold/Silver, Ruby/Sapphire, D/P, HG/SS)
  shows "No recorder has been created yet for the current version" in the
  status bar with Reconnect disabled and leaves record mode on, like the Qt
  `RecorderStatus` (an earlier build also raised an error and turned recording
  off, after the tkinter-era controller handler the Qt app never registers).
- **Heal / save locations keep the mapper's raw value.** The gen 2 recorder
  stores the map *group number*; Python writes it back as an int
  (`"PkmnCenter Heal": [26]`), and so does the port (`LocationEventDefinition::raw`).
  A null map name is written as `null`, not `"None"`.
- `BlackoutEventDefinition()` defaults to `location=""` (`[""]`), the
  unresolved learn-move label reads "Learning X over: Y", and a non-numeric
  battle HP change is dropped in gen 3 the way Python's `TypeError` drops it.
- **Thread timing is not identical.** Both apps queue events from the
  GameHook thread and add them from a processing thread; an area change that
  arrives within ~100 ms of a battle ending can put the battle's trailing
  events on either side of the folder boundary in either app.
- Gen 4's `[EXP_SPLIT]` / `InventoryChangeState._on_exit` debug logging is
  not reproduced line for line.

## UI divergences (egui vs. Qt)

Things that are different by design of the toolkit swap, or that the Qt code
could not do either:

- **Transfer-to-folder and new-folder dialogs.** The Qt versions call
  `Router` methods that do not exist (`AttributeError` at runtime); the port
  implements the pre-Qt (tkinter) logic: the folder list is every folder
  name minus the invalid targets (the selection's own folder / descendants).
- **Item checkbox in the route list.** The Qt `EventItem` toggle raises
  (`set_enabled_status` is missing); Rust makes it a no-op. Folders and
  groups toggle as before.
- **Window screenshots are opaque** (Qt captured with a transparent
  background outside the rounded corners; egui's `ViewportCommand::Screenshot`
  returns the framebuffer). Corners are still rounded the way
  `_round_container_corners` does it. The file names and the crop rects
  (full / player / enemy / summary) are the same.
- **Overlays instead of windows.** The quick-add popover, the toast, the
  auto-clearing status label and every dialog are drawn inside the main
  window (egui `Modal` / `Area`). The run summary (undocked) and the setup
  summary are real secondary OS windows, like Qt.
- **Update flow.** No progress window: download / swap progress goes to the
  log and the status bar; the restart is done by `xpr-update::restart` after
  eframe exits (Qt shows a progress dialog and relaunches from inside the
  process). Cleanup of the previous version happens on the next start.
- **Fonts.** The configured font family is looked up as a TTF/OTF file in
  the system font folders (stem `<name>` with whitespace removed, plus the
  `b`/`bd`/`bold` face, then a "contains" match); a family that is not found
  silently falls back to egui's bundled font (`Theme::font_loaded` is false).
  Point sizes are scaled 4/3 to egui pixels.
- **Text-field focus guard.** Qt filters route-list shortcuts by the focused
  widget class; egui has no widget classes, so the rule is "any text field
  has focus → plain-key shortcuts are swallowed (F-keys and Escape excepted),
  and the editing chords Ctrl+A/C/V/X/Z/Y/arrows/Home/End/Backspace/Delete
  go to the field".
- **Route index.** The landing page's route metadata cache lives at
  `<global config dir>/xpr_route_index.json` (Qt keeps it in
  `saved_routes/.xpr_index.json`, where the Rust app would list it as a
  route). Both are rebuilt automatically from file mtimes.
- **Not ported (unreachable in the Qt app):** `BadgeBoostViewer`,
  `EnemyPkmnTeam`, the tkinter-only popups. The trainer *preview* on hover
  is a no-op in Qt and stays a no-op.
- **"Exp: -1" in the Pokémon viewer** for gen ≥ 3 with unknown exp is what
  Python shows; reproduced.
- **Config file.** Same keys, same defaults, same file. The port adds no
  keys. The window geometry is saved on close in the tkinter geometry string
  format as before.
- **Pre-fight candy stepper applies immediately.** The Qt widget only
  updates its label on a click and mutates the route 250 ms after the last
  click ("the actual (expensive) route mutation is deferred until the user
  pauses"). The Rust app mutates the route and refreshes the damage ranges
  on every click (a few ms even on a 2,800-event route, see
  `rust/PORT_STATUS.md` § Performance). To keep the Qt undo granularity,
  clicks less than 250 ms apart on the same fight form **one undo step**
  (`MainController::coalesce_next_undo_step`). The typed legacy candy entry
  keeps its 150 ms debounce; the vitamin steppers keep their 300 ms one.
- **One battle reload per stepper click.** A candy / vitamin / held-item /
  tutor operation reloads the battle summary itself and restores the unsaved
  selections (setup moves, stat stages, weather, screens, intimidate, mimic,
  transform) exactly as Python does inside its synchronous cascade; the
  frame's `route_changed` cascade then skips the second reload
  (`BattleController::take_handled_route_change`) that used to discard
  those selections (review finding 11).

## Test hooks added to the Rust app (not in Python)

| Env var | Effect |
|---|---|
| `XPR_GLOBAL_CONFIG_DIR=<dir>` | Read/write `config.json`, the log and the route index there instead of the appdirs location |
| `XPR_DISABLE_AUTO_UPDATE=1` | Skip the GitHub release check at start |
| `XPR_GAMEHOOK_URL=<url>` | GameHook base URL for the recorder (default `http://localhost:8085`) |
| `XPR_SMOKE_SCREENSHOT=<file.png>` | Capture the window ~4 s after start and exit (unattended smoke run) |
| `XPR_SMOKE_ACTION=battle\|battle_last\|newroute\|summary\|inline\|candy\|record` | Before the smoke screenshot: open the battle tab of the first / last trainer, the new-route page, the docked run summary, or the inline creator; `candy` opens the biggest trainer fight (or the one named by `XPR_SMOKE_FIGHT=<substring>`) and clicks "+" candy six times, 700 ms apart, before an 8 s screenshot; `record` starts recording against `XPR_GAMEHOOK_URL`, stops after `XPR_SMOKE_RECORD_SECS` (default 30) or ~3 s after `XPR_SMOKE_STOP_URL` (a JSON endpoint) reports `"done": true`, saves the route as `XPR_SMOKE_SAVE_NAME` (if set), then screenshots and exits |
| `XPR_FRAME_LOG=1` | Log every frame slower than 1 ms with the route-list and event-details draw times, and every route-list rebuild |
