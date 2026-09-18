# Rust port — review (2026-09-11)

Review of the side-by-side port under `rust/` against the Python reference
(branch `ui`), performed after the implementing agent reported completion.
Nothing in `rust/` was modified by this review. Each finding is marked
CONFIRMED (verified by execution or by reading both sides) or PLAUSIBLE.

## What was checked

- `cargo build --workspace`, `cargo test --workspace` (32 tests, all green),
  `cargo clippy --workspace` (no errors; ~90 warnings, mostly style),
  `cargo build --release` (`xpr-app.exe` 15.8 MB).
- Golden corpus: `dump.py --battles --data-dir` was run over the user's real
  data dir (1,951 saved + 3,260 outdated routes; ~2,800 records were
  generated in the review window) and replayed with `xpr-golden verify`.
  Where `verify` reported a failure, the Rust side was re-dumped with battle
  summaries and compared per group index, skipping fights where **Python
  itself crashes** (see "Golden results").
- The release binary was smoke-launched against an isolated config in six
  states (editor, editor with a trainer selected, battle tab, new-route page,
  docked run summary, inline creator) and compared to Qt screenshots of the
  same route taken through a small PySide harness.
- Six parallel code reviews, one per area (engine, data/core, calc,
  recorder, app part 1, app part 2 + ui-kit/update/golden), each comparing
  the Rust source function by function with the Python.

## Golden results

`PORT_STATUS.md` reports 204/204 on the first 200 routes. On the larger
corpus:

| Records verified | Pass | Fail |
|---|---|---|
| 2,837 (saved routes + the first outdated routes, alphabetical) | 2,759 | 78 |

Every failing record falls into one of these classes:

1. **Python crashes, Rust computes** — 1,006 of ~15,150 fights in the
   failing routes. The corpus generator records `error` for Python and the verifier
   reports `summary: missing in python`. The Python exceptions are:
   - `TypeError: '<=' not supported between 'NoneType' and 'int'` (217
     fights): gen 4 Technician with a null-power move
     (`pkmn/gen_4/pkmn_damage_calc.py:274`; gen 5 has the same at `:246`).
     Every HeartGold/Platinum fight where the player mon has Technician and a
     variable-power move (Return, Low Kick, Gyro Ball…) crashes the Python
     battle summary; Rust skips the boost and shows numbers.
   - `KeyError: ('Normal', 'Steel')` etc. (~600 fights): custom-gen type
     charts (the `y-*`, `yellow-jirachi`, `yellow-malamar` backport routes)
     lack rows for types the backport introduced. Python crashes; Rust treats
     the missing entry as neutral (`GenData::effectiveness` returns `None`).
   - `IndexError: list index out of range` (42 fights):
     `BattleSummaryController.get_partial_trainer_definition`
     (`battle_summary_controller.py:1245`) reads `_custom_move_data` left
     over from a previous fight with a different party size. Reproduced on
     `crystal-igglybuff-1` (Elite Four Karen after the preceding fights).
     Rust keeps the lists consistent.
   None of these are port defects, but none are in `KNOWN_ISSUES.md` either,
   and the Rust numbers for those fights are verified by nothing.
2. **Gen 3 Psywave distribution** — the only real numeric divergence found.
   `pkmn/gen_3/pkmn_damage_calc.py:253` builds the table with a dict
   comprehension, so duplicate floor values collapse to count 1 (a Python
   bug); gen 4/5 Python accumulate counts, and Rust
   (`xpr-calc/src/gen3.rs:188`, `DamageRange::from_pairs`) accumulates in
   every gen. Kill percentages for Psywave in Emerald/RS/FRLG differ at any
   level that is not a multiple of 10 (all 8 Latias/Latios routes). Rust is
   mathematically right but not faithful, and it is undocumented.
3. **`exp_split == 0` on a trainer event** (`crystal-tyrogue-1`): Python
   raises `ZeroDivisionError` at load (`universal_utils.py:20`) and falls
   back to a new route; Rust (`xpr-data/src/exp.rs:12`) silently treats the
   split as 1 and loads the route with no error on the event. Undocumented.

Once the Python-crash fights are skipped, all 14,145 remaining fights in the
failing routes match, and every route/tree/save/notes record matches except
`crystal-tyrogue-1`. The engine side of the port is in good shape. The
corpus generator (`dump.py`) was still running when this review closed; the
scratch comparison script and the Rust-side dumper with battles are in the
session scratchpad, not the repo.

## Findings — must fix before use

| # | Sev | Where | Finding | Status |
|---|---|---|---|---|
| 1 | critical | `xpr-app/src/controller.rs:727,729,678` → `xpr-engine/src/router.rs:464` | New Folder / Split Folder pass `None, None` for `folder_enabled` / `folder_expanded`; `Router::is_enabled` treats `None` as disabled. Every folder created in the Rust app is disabled and collapsed, its events show as `Disabled:`, state passes through, and the file is saved with `"Enabled": null` so it stays disabled in Python too. Python defaults both to `True` (`router.py:271`). | CONFIRMED (executed) |
| 2 | critical | `xpr-app/src/app.rs:891,1080-1097` | "Update to vX and restart" calls `cancel_and_quit`, which sets `allow_close` and closes without the unsaved-changes prompt and without saving geometry. Python goes through `closeEvent` and asks. Edits are lost. | CONFIRMED |
| 3 | critical | `xpr-ui-kit/src/shortcuts.rs:154` + egui-winit 0.33.3 `lib.rs:815-830` | egui-winit converts Cmd/Ctrl+C, +X, +V (Shift ignored) into `Event::Copy/Cut/Paste` and never emits a `Key` event, so the default bindings Enable/Disable (Ctrl+C), Toggle Highlight (Ctrl+V), Customize DVs (Ctrl+X) and Close Route (Ctrl+Shift+C) can never fire. Menu labels still advertise them. | CONFIRMED |
| 4 | major | `xpr-app/src/app.rs:1636-1639` vs `:1923` | Window geometry is saved in physical pixels (`* pixels_per_point`) but restored as logical points. On a 125/150 % display the window grows every launch. | CONFIRMED |
| 5 | major | `xpr-ui-kit/src/shortcuts.rs:154`, dispatch order `app.rs:388,475,481-488` | egui `consume_key` matches modifiers "logically" (extra Shift ignored), so Shift+F1 toggles recording (F1) and Shift+F2 toggles auto-load (F2) instead of test moves / move highlights. Shift+Up/Down in the list becomes a plain single-select (`route_list.rs:1109`). | CONFIRMED |
| 6 | major | `xpr-app/src/route_list.rs:760-765,792-795` | Descendants of a disabled folder are drawn checked and in normal colour (own flag used instead of the effective one). Python greys the whole subtree. | CONFIRMED |
| 7 | major | `xpr-app/src/route_list.rs:208,712` | Programmatic selection inside a collapsed folder (gym-leader keys 1-8, Ctrl+1..7, "Run Status: Invalid" cycling) neither expands the ancestors nor scrolls. Python `scroll_to_selected_events` does both. | CONFIRMED |
| 8 | major | `xpr-app/src/quick_add.rs:589-597` | The quick-add popover consumes Q/W/E/R/T/Space every frame while open, including while typing in its own search fields; typing "water" jumps pages. | CONFIRMED |
| 9 | major | `xpr-ui-kit/src/widgets.rs:660-711` | Searchable dropdown opens on focus filtered by the current value and commits row 0 on Tab/Enter; tabbing through a "Mew" field changes it to "Mewtwo", "Potion" to "Hyper Potion". Qt exact-matches. | CONFIRMED |
| 10 | major | `xpr-app/src/editors.rs:855-877,786` | Inventory editor never re-runs the item filter on load; after filtering items on one event, selecting another item event shows the wrong item and the next save writes it into the route. | CONFIRMED |
| 11 | major | `xpr-app/src/battle.rs:303-575`, `event_details.rs:126,145` | Candy / vitamin / held-item steppers mutate the route, then the same frame's `route_changed` signal reloads the battle from the not-yet-saved event, discarding unsaved stat-stage / weather / intimidate selections. Python restores after the cascade. | **FIXED** in the 2026-09-11 performance pass (`BattleController::take_handled_route_change`; test `xpr-app/tests/prefight_candies.rs`) |
| 12 | major | `xpr-core/src/io_utils.rs:103-146` | Changing the data location merges into an existing target folder, overwriting same-named routes and deleting the source; Python refuses when the target sub-folders exist. | CONFIRMED |
| 13 | major | `xpr-app/src/controller.rs:533`, `xpr-engine/src/router.rs:121` | Load-failure fallback reuses the bad version name (set before `get_version` fails), so a route whose custom gen is missing leaves the app with an empty tree and the previous gen's state. Python falls back to Yellow/Abra. | CONFIRMED (executed) |
| 14 | major | `xpr-ui-kit/src/theme.rs:357-410`, `dialogs.rs:793-826` | Font lookup by file stem: "Times New Roman", "Courier New", "Consolas", "Comic Sans MS" etc. silently fall back to the bundled font; the font dialog lists raw file stems (`segoeui`, `arialbd`). Segoe UI itself resolves. | CONFIRMED |
| 15 | major | `xpr-update/src/lib.rs:56-68`, `xpr-core/src/consts.rs:6` | Asset selection is "first `windows*` asset of the latest release", `APP_VERSION` is `v6.0a`. The first release tagged above that with the existing PyInstaller zip would replace `xpr-app.exe` with the Python exe. Needs a release-process decision. | PLAUSIBLE |
| 16 | major | `xpr-app/src/app.rs` (Shift+1) | winit reports Shift+1 as `!` on US layouts; egui maps it to `Key::Exclamationmark`, so "Highlight 1" never fires (2-9 work by accident). | CONFIRMED |
| 17 | major | `xpr-recorder/src/gamehook.rs:257,349` | The ↻ reconnect button only sets a flag polled inside the backoff sleep; while the socket is up but the mapper failed to load it does nothing. Python re-ran `_connect_helper`. | CONFIRMED |
| 18 | major | `xpr-recorder/src/gamehook.rs:247`, `games/gen1.rs:1406` | Stopping recording does not join the worker / processing thread; a stale FSM can drain leftover events into a freshly started session. Python joins. | PLAUSIBLE |

## Findings — should fix or document

| # | Sev | Where | Finding |
|---|---|---|---|
| 19 | minor | `xpr-calc/src/gen3.rs:188` | Gen 3 Psywave distribution (see Golden results 2). Either reproduce the Python collapse or fix Python first per plan D4 and document. |
| 20 | minor | `xpr-data/src/exp.rs:12` | `exp_split == 0` silently treated as 1 (Golden results 3). Should at least flag the event. |
| 21 | minor | KNOWN_ISSUES | Python battle-summary crashes that Rust does not reproduce (gen 4/5 Technician null power, custom-gen type-chart gaps → neutral, stale `_custom_move_data` IndexError, gen 3/5 in-place stat mutation with Transform) are not listed. |
| 22 | minor | `xpr-calc/src/battle_summary.rs:370` | Intimidate toggles are mapped through the correct definition order on load; Python maps through the previous battle's order (usually empty), so saved intimidate lists render differently. Rust is right; document. |
| 23 | minor | `xpr-app/src/route_list.rs:629-641` | `row_values`, nine column strings and up to five galleys are computed for every row every frame before virtualisation (3,000-row route → ~30k allocations per frame while hovering). Plan §7 promised visible-rows-only. Not measured. **FIXED** in the 2026-09-11 performance pass: values and fit widths are cached per rebuild (measured 3.4 ms → 0.6 ms per frame on the 2,834-group route). |
| 24 | minor | `xpr-app/src/main.rs` | No panic hook: a panic ends the process with no dialog. One reachable panic found: `editors.rs:194-199` indexes `order_menus[idx]` for a trainer with more than 6 mons (custom gen); `theme.rs:14-18` slices a 6-byte non-ASCII colour string. |
| 25 | minor | `xpr-app/src/dialogs.rs:137` + `widgets.rs:373` | Enter does not submit Load Route / Transfer / Custom Gens while their filter field has focus (the entry consumes it); filters are not auto-focused. |
| 26 | minor | `xpr-app/src/app.rs:1784,1888` | Run-summary PNG export includes the toolbar; Python renders only the grid. |
| 27 | minor | `xpr-app/src/battle_ui.rs:728-740` | Faded moves also fade the kill frame; Python only fades header and range. |
| 28 | minor | `xpr-app/src/state_views.rs:218` | Inventory viewer with no state shows "Current Money: 0"; Python shows 3000. |
| 29 | minor | `xpr-app/src/dialogs.rs:668` | Battle Config dialog recalculates and marks the route unsaved; Python only re-renders. Rust is the intended behaviour; note the extra prompt on quit. |
| 30 | minor | `xpr-data/src/registry.rs:159-166` | `get_gen_names` order differs (Yellow first vs Red first; Platinum before Diamond/Pearl in Python): landing-page game filter order, Custom Gens "Base Version" default, Final Trainers fallback. |
| 31 | minor | `xpr-core/src/pyjson.rs:161` | U+007F written raw; Python writes ``. |
| 32 | minor | `xpr-data/src/backport.rs:525-538` | `get_close_matches` tie-break differs from difflib (first vs lexicographically greatest). |
| 33 | minor | `xpr-update/src/lib.rs:77`, `app.rs:130` | `XPR_DISABLE_AUTO_UPDATE` does not skip the network check (only `is_upgrade_possible`); TESTING.md and KNOWN_ISSUES say it does. Download scratch dir is `%TEMP%` instead of the data dir. |
| 34 | minor | `xpr-recorder/src/games/gen2.rs:1590` etc. | `_has_pending_loss` fixed in gens 2-5 (Python's guard was always false) — good, but uncommented, unlike the gen 1 deviations. |
| 35 | minor | `xpr-recorder/src/host.rs:105-121` | Every FSM host call blocks the worker until a UI frame; while a native file dialog is open the SignalR socket is neither read nor pinged and drops after ~30 s. |
| 36 | minor | `xpr-golden/src/main.rs:101-116,164` | Verifier counts a record as passed when both sides have *any* `load_error` (text not compared); `dump` does not emit battles although the README says it does; an empty corpus dir passes. |
| 37 | minor | `xpr-engine/src/events.rs:99-655`, `router.rs:303` | Truncated legacy event arrays and malformed `dv` blocks load silently (notes-only event / default DVs) where Python rejects the file. |
| 38 | minor | `xpr-engine/src/undo.rs:38` | Undo reverts highlight toggles; Python's snapshots alias the live tag lists so highlights survive undo. Rust is arguably right; document. |

## Visual comparison (one route, Yellow, 100 % scale)

Battle summary: same numbers, kill lines, recoil line, colours and layout.
Pre-Event State: same content and strings, including the `Exp: -1` for the
solo mon (Python shows it too, so the KNOWN_ISSUES note that limits it to
gen 3+ is too narrow). Differences to raise at the per-screen sign-off:

- The Pokémon viewer's stat labels/values render ~2 pt larger than the
  Net/Realized/Total StatExp blocks in Rust; in Qt all use the same size.
- The list/details split differs by ~100 px for the same persisted fraction
  (Rust left pane ≈ 820 px, Qt ≈ 920 px at 1655 px wide).
- "Switch tabs automatically" is right-aligned as one checkbox in Rust; in
  Qt the label sits left and the checkbox at the far right.
- The notes-visibility dropdown is compact in Rust, full width in Qt.
- Once in three smoke launches the "select first trainer" hook ended with
  Brock selected instead of Rival1 while the mouse hovered that row. The
  click code requires a real press+release, so this is most likely an
  environmental stray click, but it was not explained.

## Performance (release build, this machine)

| | Python (plan §1) | Rust |
|---|---|---|
| Start to editor with a route auto-loaded | ~12 s | < 1 s (screenshot at 4 s was a fixed delay) |
| Load `crystal-sunflora-1-refined` (2,834 groups) | 87 ms | 32 ms |
| Full recalc | 40-60 ms | 7 ms |
| Serialize | 70 ms | 6.5 ms |

## Verdict

The engine, data and calc layers are a faithful port: config files are
byte-identical, every stat/exp/badge table matched a full programmatic dump,
and 14,000+ battle summaries match apart from one Python bug (Psywave) that
the port fixed without saying so. The problems are concentrated in the app
layer and in egui input semantics: one critical engine-glue bug (every new
folder is disabled), one data-loss path (update-and-restart), four dead or
misrouted default shortcuts, and a handful of editor/battle-state bugs that
silently write wrong values into routes. The recorder is a close
transliteration but has not been run against GameHook. The `TESTING.md`
sweep has not been done and should not start until findings 1-11 are fixed.
