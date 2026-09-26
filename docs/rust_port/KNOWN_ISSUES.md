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
| KI-2 | **Fixed on both sides (2026-09-12).** The gen 2 recorder built `mon_order` from the enemy party positions it saw change (`crystal_states.py`), 0-based where gens 3/4 are 1-based (`+ 1`) and the engine reads them as 1-based. It only worked because the game writes -1 (255) to `wCurOTMon` while setting a trainer battle up (`InitEnemyTrainer`) and that sentinel usually arrives as the first change, making the real mons' indices 1-based by accident (recorded routes carry `[1, 2, 0]`, the trailing 0 being the sentinel's slot); when the -1 write landed in the same GameHook poll as the lead's 0, the fight came out as `[0, 1]` (the lead dropped) or `[0]` (no mons at all — the Python engine then raises and stops recalculating the route). Both recorders now seed the lead like gen 3, ignore positions outside the party, and emit 1-based values (`[1, 2]`); existing routes with the old shapes keep resolving as before. | `crystal_states.py`, `xpr-recorder::games::gen2`, `xpr-recorder/tests/gen2_mon_order.rs`, `docs/rust_port/recording/scenarios/crystal_totodile.py` |

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
| 17 | Documented damage-calc divergences from the games | Addressed on 2026-09-23: gens 1–4 re-verified against the decompilations with a sweep harness and fixed (see `docs/damage_calc_review/2026-09-23_rust_verification.md`); the golden corpus entry for gen 2 Beat Up was re-recorded (the move is now implemented) |

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
- **Mimic dropdown options are per-matchup, not per-battle (deliberate fix,
  2026-09-16).** Python's `_mimic_options` accumulates every move seen across
  every enemy Pokémon in the whole fight, so every Mimic dropdown in a
  multi-Pokémon battle offers the same combined list. Rust's
  `BattleSummary::matchup_mimic_options` scopes the list to the enemy
  Pokémon actually being faced in that matchup. `battles.py`'s golden dump
  still reproduces the Python (whole-battle) list, so `xpr-golden verify`
  will report a `mimic_options` diff on any multi-Pokémon battle that has
  Mimic — expected, not a bug.

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

### Bag reordering (gen 1, Rust only; added 2026-09-16)

The gen 1 recorder records the in-game SELECT swap as a `"Reorder Bag"`
event: a list of
`[item, slot, item, slot]` swaps, 1-based, computed against the bag as the
engine holds it after the quantity events of the same detection window
(Oak's Parcel, which never reaches the route, is not a slot). The Python app
has no such event type, so it opens one as an empty notes event and drops it
on re-save; the replay harness cannot compare it. Things to know:

- **Slots are exact only when recorded.** The engine applies a swap by item
  *name*; a stored slot that no longer matches (an item was added or removed
  earlier in the route, or the route's starting bag differs from the
  console's) still swaps and flags a **warning** (amber row, label kept, run
  status still Valid, not an "invalid event"); a swap naming an item that is
  no longer in the bag is skipped with a warning. Re-saving the event from
  its editor stores the order as shown, which refreshes the slots.
- Warnings are a new third row state (`EVENT_TAG_WARNINGS`,
  `EventItem::warning_message`, `EventGroup::warning_messages`); folders do
  not propagate them (errors keep their `child_errors` propagation).
- The pre-event inventory panel numbers slots from 1 (the Qt panel counts
  from 0) so that it matches the event text.
- Two stacks of the same item (99 cap) collapse to one in the recorder's
  cache, as before (first stack's position, second stack's quantity). A swap
  that only moves the second stack is not seen, and SELECT-merging the two
  stacks in the game can record a spurious acquire plus a reorder. Needs
  more than 99 of one item.

### Thief / Covet (gens 2-5, Rust only; added 2026-09-17)

A trainer fight's definition can name the enemy mons the solo mon robs
(`"thief_mons": [definition indices]`, the `collapsed_mons` convention; the
key is written only when set, so untouched routes stay byte-identical). The
engine applies it the way Crystal's `BattleCommand_Thief` and Emerald's
`MOVE_EFFECT_STEAL_ITEM` do: the item goes into the solo mon's **held slot**
(never the bag), only when it holds nothing, before that mon's KO (a stolen
Lucky Egg / Macho Brace counts for it). Selling the loot is a "Hold Item"
(hold nothing) event, which returns the held item to the bag, followed by
the Sell. Things to know:

- The Python app ignores the key on load and drops it on re-save; the
  golden `verify` and the replay harness cannot compare a route that uses
  it.
- A steal that cannot happen (already holding something, the mon has no
  item, unknown item name) is an **error** on the fight; the fight still
  resolves.
- The gen 3 and gen 4/5 recorders no longer record an in-battle held-item
  change from nothing to something as a `Hold Item` event (which failed:
  the item was never in the bag). In a trainer battle it ticks the current
  first enemy mon's flag (a double battle's second enemy is not told apart);
  in a wild battle it records `Acquire` + `Hold` of the item, which lands
  before the wild fight when the steal was not the KO hit (the wild event
  is only queued at the KO). The held item is now sampled as the battle
  *starts* rather than when the delayed initialisation fires, so a Thief on
  the first turn is still seen as a change. The gen 2 recorder never
  watched the held item mid-battle and still does not.
- **Taking the held item off** in the overworld (bag gains it, the mon holds
  nothing) is recorded as `Hold Item` (hold nothing) by the gen 2, 3 and 4/5
  recorders; Python recorded nothing, so a route that stole twice erred on
  the second steal ("already holding"). The replay scenario
  `emerald_thief` (Rust only, `run_pair.py --only rust`) covers both steals,
  the take-offs, the sale and a wild steal; the recorded route must load
  without errors.
- Emerald's trainer data spells Bug Maniac Jeffrey (rematch 4)'s held item
  `Silver Powder` where the item DB has `Silverpowder`; stealing it reports
  an unknown item.
- **A berry eaten mid-fight, then a Thief** (gen 3; fixed 2026-09-20): the
  held item the steal is compared against now follows the mon's real one,
  so the steal is recorded on the fight. Before, it was still the item held
  as the battle began, and the steal came out as a `Hold Item` from the bag
  ("Cannot sell/use item that you do not have"). The recorder still queues
  the eaten berry's `Hold Item` (hold nothing) *after* the trainer event
  (added at battle start), so the engine sees the steal while the berry is
  still held ("already holding"): move that `Hold Item` ahead of the fight.

### Gen 3 trainer battle start vs. GameHook batches (fixed 2026-09-20)

Emerald (and FireRed) fill `gEnemyParty`, then set `BATTLE_TYPE_IS_MASTER`
(the mapper's `battle.type.is_battle`, bit 2 of `gBattleTypeFlags`), then
clear `gBattleOutcome` in `BattleStartClearSetData`. One GameHook poll
usually catches all three, and the client applies a `PropertiesChanged`
batch one property at a time in mapper order: `battle.outcome` (#858, enters
the Battle state) and `battle.type.is_battle` (#861) before
`battle.trainer.team.*` (#892+). The gen 3 recorder ran `_battle_ready` as
the flag arrived, so its enemy-party census was the *previous* fight's:
after a one-mon trainer, a leader got `exp_split` of length 1, every switch
failed the `real < exp_split.len()` check, and the fight was saved with
`mon_order = [1]` (the app then shows only the lead; the initial event also
errs with "list index out of range"). It only came out right when the
previous battle had at least as many mons, or when the poll split the flag
from the outcome. The census now runs from the client's idle pass, once the
batch is in (`GameRecorder::on_idle`, `init_after_batch`); the tick-delayed
path stays as the fallback. `xpr-recorder/tests/gen3_mon_order.rs` replays
the same-batch, flag-first and mock-style starts. Gens 4/5 never had the
immediate path; gens 1/2 take the party from the trainer data.

Routes recorded before the fix carry the truncated `mon_order`; opening such
a fight in the editor and saving pads it to `[1, -1, ...]` and then
`[1, 1, ...]`, smearing a `thief_mons` entry onto every slot. Repair: set
`mon_order` to definition order, drop an all-ones `exp_split`, put
`thief_mons` back on the mon the log names ("stole X from the enemy mon at
party position N").

### Gen 5 recorder (rewritten 2026-09-25)

Black/White and Black 2/White 2 no longer use the gen 4 machine: the gen 5
games update the party only when a battle ends, keep per-party-slot battle
structs, decrypt party mons in place while editing them and never use up TMs,
and the old port of the Python `BlackRecorder` (never run against a real
game) mis-recorded most of that (level-ups as rare candies, TM moves as
level-up moves, no enemy faints, no saves, trainer vs wild decided before the
id was set). `xpr-recorder::games::gen5` reads settled snapshots instead and
was checked against full Super Shuckie replays of real runs; what the games
do, what the mappers get wrong and the gaps that remain are in
`docs/rust_port/recording/gen5.md`. The two that affect users:

- **Black 2 / White 2 saves are not detected** with the current mappers
  (`flags.new_game` reads a byte that never changes). The recorder says so
  once, and a reset then adds a note instead of rolling the route back to a
  save it never saw. Fixed by pointing the mappers' `flags.new_game` at the
  party block's save counter (`0x221E958`, Black 2 `- 0x40`).
- **Area names** come from the recorder's own zone table (read from the
  ROMs): the Black/White mappers name zones from the place-name list.

### EV Override (all gens, Rust only; added 2026-09-18)

A testing aid with no in-game equivalent: an `"EV Override"` event pins
the solo mon's EVs (stat exp in gens 1-2) to fixed values from that point
of the route on, so a damage range can be checked against a chosen spread
without re-routing the yields that would produce it. Stored as an object
keyed like a serialized stat block (`hp`, `attack`, `defense`, `speed`,
`special_attack`, `special_defense`; a missing key is 0). Things to know:

- The override replaces both the realized and the unrealized stat XP, so
  the stats reflect it immediately (like a vitamin, unlike a battle yield
  in gen 1-2, which waits for the next level-up). Later yields stack on top
  of the override as usual.
- Gens 1-2 have one Special stat exp (the engine reads `special_attack`
  for both special stats), so the editor and the dialog show a single
  "Special" field there and store its value in both fields; the label
  reads "Stat Exp Override: ... Spc ...".
- Values the game cannot hold are applied capped (65535 per stat in gens
  1-2; 255 per stat and 510 total in gens 3-5, filled in HP, Atk, Def, Spe,
  SpA, SpD order like the rest of the engine) and flagged as a **warning**
  on the event, not an error.
- A new override created from the inline creator starts at the mon's
  values at the insertion point; the numbers are edited in the details
  panel ("Use current" resets them). Clicking the "EVs real / total"
  column of the Pre-Event State stats card opens a dialog prefilled with
  those values that inserts an override right before the selected event.
- The Python app has no such event type, so it opens one as an empty notes
  event and drops it on re-save; the golden records carry no filter key
  for it.

### PP tracking (all gens, Rust only; added 2026-09-25)

The Pre-Event State Moves card shows each move's `cur / max` PP before the
selected event. Design and rules: `docs/rust_port/design/pp_tracking/PLAN.md`.

**How it works.**

- **Where current PP lives.** Current PP is not part of the route state.
  The PP ledger (`xpr-app/src/pp_ledger.rs`) derives it after
  recalculation, from:
  - fight spends: `xpr-calc/src/pp.rs`, cached per fight and computed on
    rayon threads;
  - refills (heal, blackout);
  - PP items;
  - the learn-move rules.
- **What the engine keeps.** Only the per-slot PP-Up counts
  (`SoloPokemon.pp_ups`) and the PP-item target move.
- **Target move storage.** Stored as Rust-only `target_move` / `no_effect`
  keys in the trailing object of an inventory event, written only when
  set.
- **When it rebuilds.** Only when something shows PP (Moves card, banner,
  quick add), and only if the route revision or the calc settings
  changed.
- **Cost.** `pp_bench` on the three big routes (24 threads): cold build
  8–29 ms, a no-op check 0 ms. A late edit costs 4–19 ms (every fight
  from the cache). A candy before the first gym costs 5–24 ms.

**Behaviour to know.**

- **Existing routes get amber rows.** An Ether / Max Ether / PP Up / PP
  Max / Leppa Berry / Mysteryberry "Use/Drop" event without a target move
  now shows an amber **warning** ("Choose the move … was used on") until a
  move is picked in the details panel or the event is marked **Tossed**.
  Across 67 saved routes this flags 288 existing events; nothing else in
  their records changed. An Elixir without a target counts as used.
- **Refused uses are consumed anyway.** A PP Up / PP Max past 3 PP Ups
  (or on Sketch) is an **error** on the row but still leaves the bag, like
  a vitamin over the cap. The game refuses it and keeps the item. An
  Ether or Elixir on a full move is also consumed; the ledger adds a note
  for it on the event ("the game would refuse this Ether").
- **Negative PP.** PP may go negative (the banner says to heal first).
  Restores are plain arithmetic: −3 plus an Ether is 7.
- **Wild Pressure mons.** A wild mon's ability is unknown, so PP assumes
  Pressure when its species can have it (the worst case, like the damage's
  DV range).
- **Quick add.** Quick add's Use button accepts PP items, with an **On
  move** menu for single-target ones, and its Drop button marks a PP item
  as tossed. A move row's right-click menu in the Moves card inserts a
  PP-item use before the selected event.

**Known gaps, to close later.**

| Gap | Effect |
|---|---|
| Held Leppa Berry (gens 3–5) / held Mysteryberry (gen 2) restoring PP when a move hits 0 | Ignored: PP reads up to 10 lower than the game until the next heal. Using one from the bag is modeled. |
| Damaging setup moves (Mud-Slap ×2 on the battle page) | Their PP is charged, but their damage is not taken off the enemy's HP, so the KO move's count is slightly high. |
| A gen 1 Thrash / Wrap lock carrying into the next enemy mon | Each matchup is charged on its own (slight over-count). |
| Spread moves in double battles | Charged once per enemy mon. |
| Skill Link (gens 4–5) | Not modeled by the calc; PP assumes 2 hits like any variable multi-hit move. |
| Thief / Covet steals, Pay Day, weather / screen moves on the battle page | Their own PP is not charged. |
| Recorder | Records Ethers and PP Ups without a target (amber until one is picked). No PP data is read from GameHook yet. |

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
- **Screenshots are rendered offscreen** (`xpr-app/src/screenshot.rs`): the
  widget is drawn into a private egui context on a transparent canvas and
  rasterised in software, like Qt's `widget.render(pixmap)`, so the PNGs have
  a transparent background between the cards, anti-aliased rounded corners
  (also on the cut edge of the player / enemy halves), the whole content
  regardless of the scroll position, and none of the on-screen chrome (menu,
  tooltips, hover, toolbar). The file names and crop rects (full / player /
  enemy / matchup / summary) are the same as Qt's. Unlike Qt the export is
  at the window's pixel scale, so on a 125 % display the PNG is 1.25× the
  logical size.
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
| `XPR_DISABLE_AUTO_UPDATE=1` | No request to GitHub at start (the check reports "no release information") and updates count as not possible, so nothing is prompted |
| `XPR_GAMEHOOK_URL=<url>` | GameHook base URL for the recorder (default `http://localhost:8085`) |
| `XPR_SMOKE_SCREENSHOT=<file.png>` | Capture the window ~4 s after start and exit (unattended smoke run) |
| `XPR_SMOKE_DELAY_MS=<ms>` | How long the smoke run waits before the capture (default 4000); raise it to drive the window by hand or with injected input first |
| `XPR_SMOKE_ROUTE=<route name>` / `XPR_SMOKE_NEW_ROUTE=<version>\|<solo mon>` | With a smoke screenshot: load that saved route / start a fresh route from the built-in data instead of honouring the auto-load preference (`rust/windows_build.py --smoke` uses the latter to prove the packaged exe runs on its own) |
| `XPR_SMOKE_ACTION=battle\|battle_last\|last\|prestate\|newroute\|summary\|inline\|candy\|record\|quickstart` | Before the smoke screenshot: open the battle tab of the first / last trainer, select the last event of the route (its editor shows in the details panel), open the new-route page, the docked run summary, or the inline creator; `candy` opens the biggest trainer fight (or the one named by `XPR_SMOKE_FIGHT=<substring>`) and clicks "+" candy six times, 700 ms apart, before an 8 s screenshot; `record` starts recording against `XPR_GAMEHOOK_URL`, stops after `XPR_SMOKE_RECORD_SECS` (default 30) or ~3 s after `XPR_SMOKE_STOP_URL` (a JSON endpoint) reports `"done": true`, saves the route as `XPR_SMOKE_SAVE_NAME` (if set), then screenshots and exits; `quickstart` presses the landing page's "Start Recording" (no route loaded) and then behaves like `record`; `prestate` selects the group whose name contains `XPR_SMOKE_EVENT` (or the folder named by `XPR_SMOKE_EVENT=folder:<substring>`; default: the first trainer fight) and forces the Pre-Event State tab, with `XPR_SMOKE_NOTES=open\|closed` setting the notes footer and `XPR_SMOKE_SPLIT=<0..1>` the splitter fraction for the shot (neither is saved) |
| `XPR_SMOKE_EXPORT=<kind>[,<kind>...]` | Right before the smoke capture, run these exports (they land in the configured images dir): `event_list`, `battle_summary`, `player_ranges`, `enemy_ranges`, `run_summary`, `setup_summary`, `matchup:<n>[:player\|:enemy]`; combine with `XPR_SMOKE_ACTION=battle_last` / `summary` to have something to export |
| `XPR_FRAME_LOG=1` | Log every frame slower than 1 ms with the route-list and event-details draw times, and every route-list rebuild |
