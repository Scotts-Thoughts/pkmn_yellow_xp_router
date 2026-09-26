# PP tracking — implementation plan

Status: implemented 2026-09-25 (phases 1–3); §11 lists where the code
lives and where it differs from this text. Decisions D1–D11 are confirmed
(§10). The plan has been revised after a review. Phase 4 (the §9 gaps) is
open. Nothing is implemented yet.
Rust only (the Python app is deprecated).

## 0. Summary

Track the PP of each of the solo mon's four moves along the route. The
Pre-Event State Moves card shows `cur / max` for every move before the
selected event. The rules:

- The route starts with every move at full PP.
- For each enemy mon a fight KOs, the fight spends the **guaranteed KO
  count** of the **best move** against that mon. The count is the number
  of hits that KO even at the lowest damage roll: a chance to 2HKO plus a
  100 % 3HKO spends 3.
  - Three things adjust that count: locked moves (Thrash, Wrap, gen 1
    Rage, …), Pressure, and the two-hit assumption for variable multi-hit
    moves.
- **Setup moves** entered on the battle page cost their own PP (e.g.
  Swords Dance ×2 = 2 PP). The boosted stages then lower the best move's
  KO count.
- Heals refill everything.
- Ether-type items restore one move, Elixir-type items restore all four.
- PP Up and PP Max raise one move's max PP. The event records which move
  they were used on.
- A newly learned move starts at full PP. The exception: in gen 5, a TM or
  HM taught over a move keeps the old move's remaining PP, capped at the
  new move's max.
- PP can go negative. A negative number tells the user to heal before this
  event.

The work splits into three layers:

| Layer | Crate | Owns | Needs damage calc? |
|---|---|---|---|
| Route structure | `xpr-data`, `xpr-engine` | PP-Up count per move slot, the target move on PP-item events, validation errors/warnings | no |
| Fight spend | `xpr-calc` | For each enemy mon: which move slots, how many PP | yes |
| PP ledger + UI | `xpr-app` | Walks the route: spends, restores, learn-move rules. Moves card, banner, editors | no (reads the spend) |

### Why current PP is not a `RouteState` field

The obvious design would put current PP next to `move_list` in
`SoloPokemon` and update it in `Router::recalc`. That design fails for
three reasons:

1. **Crate direction.** `xpr-calc` depends on `xpr-engine`
   (`battle_summary.rs` takes a `&Router`). Recalc cannot run a damage
   calculation without inverting that dependency.
2. **Cost.** The benchmark routes have many fights:

   | Route | Trainer fights | Wild fights |
   |---|---|---|
   | `crystal-sunflora-1-refined` | 168 | 86 |
   | `e-geodude-1-20359r` | 199 | 129 |
   | `y-weedle-1-failed` | 270 | 1,545 |

   One battle-summary refresh takes about 0.8 ms for a 6-mon gen 2 fight,
   and less for smaller fights. A full recalc of the Crystal route takes
   8.5 ms (both from `rust/PORT_STATUS.md`, "Performance pass"). Running
   every fight's summary inside recalc would add very roughly 65–150 ms
   to each Crystal recalc (8–18×), and a few hundred ms on the Yellow
   route. That includes the pre-fight candy clicks the perf pass made
   fast. `pp_bench` (§4.6) replaces these estimates with measurements.
3. **Config.** The best move depends on the user's highlight strategy and
   calc settings, which live in `Config`, not in the route.

None of this blocks a derived layer, because PP never feeds back into the
route. Negative PP is allowed, and the best move is chosen without looking
at PP. So the ledger can run after recalc, read the states recalc
produced, and cache the expensive part (fight spends) per fight.

PP-Up counts are different. They are deterministic, damage-free, and need
validation errors on the event row, so they do live in the engine state.

---

## 1. Behaviour

### 1.1 Rules

| Event | Effect on PP |
|---|---|
| Route start (`init_route_state`) | Every known move at `max_pp(move, 0)` |
| Fight item: one enemy mon of a trainer or wild fight | First, each setup move used in this matchup pays its PP (§1.2). Then the best move pays its KO PP (§1.2, §1.3), computed at the stages after that setup. The KO part is skipped if the mon has no usable damaging move |
| Heal (`TASK_HEAL`) | Every move to its max (PP Ups included) |
| Black out (`TASK_BLACKOUT`) | Every move to its max. The games heal the party on a whiteout (§2) |
| Learn move: level-up or tutor (any gen); TM/HM in gens 1–4 | The slot's new move starts at its base max; the slot's PP Ups reset to 0 |
| Learn move: TM or HM in gen 5, over an existing move | `min(old move's current PP, new move's max)` (can be negative); PP Ups reset to 0 (D3) |
| Learn move into an empty slot (any source) | Full PP |
| Delete move (`move_to_learn = None`) | Slot becomes empty |
| Use Ether / Max Ether / Leppa Berry / Mysteryberry on move M, ×n | n times: `M = min(M + amount, max)` (Max Ether: `M = max`) |
| Use Elixir / Max Elixir (gen 1–2 spelling "Elixer"), ×n | Same, applied to every move. An Elixir with no target counts as used (D6) |
| Use PP Up on move M, ×n | M's PP Ups +1 each use (cap 3). The max rises, and the current PP rises by the same amount (§2) |
| Use PP Max on move M | M's PP Ups → 3; the current PP rises by the max-PP delta |
| Use a PP item marked "tossed" | Bag only, no PP effect |
| Buy / sell / find a PP item | Bag only |
| Everything else (evolution, rare candy, vitamins, hold item, save, notes, bag reorder, EV override) | No change |
| Disabled event | No change |

Notes on these rules:

- **Negative restores.** Restoring from a negative value is plain
  arithmetic: −3 plus an Ether is 7 (D4).
- **Refused Ethers.** The game refuses an Ether or Elixir on a full move
  and keeps the item. The engine still consumes it (D7). Current PP is not
  in `RouteState`, so the ledger notes it on that event instead: `Surf was
  already full; the game would refuse this Ether`.
- **Disabling.** Only whole events can be disabled. `set_events_enabled`
  ignores item ids (`router.rs:733`), and fight items share their group's
  definition. A disabled group (or a group in a disabled folder) produces
  no fight items at all, so a disabled fight never shapes another fight's
  numbers.

### 1.2 Best move, KO turns, setup

- **Best move (D2).** The move the Battle Summary highlights as best for
  the player in that matchup, chosen among the four known moves. It uses
  the user's player highlight strategy and the fight's saved setup:
  weather, screens, stat-stage steppers, custom move data, Mimic
  selection, Metronome pick, transform. Test moves and Struggle never
  count.
  - **No highlight.** When the strategy is "None", the page highlights
    nothing. PP then uses its own rule: fewest guaranteed turns, then
    higher max damage.
  - **Why not Guaranteed Kill as the fallback.** The page's Guaranteed
    Kill strategy skips a move whose only kill line is the unquantified
    `N-hit kill, IGNORING ACC` line (unless accuracy is ignored). So it
    can pick a move that needs more guaranteed turns.
  - **When the strategy is Guaranteed Kill.** PP follows the highlight,
    quirk included, so it matches what the user sees.
- **KO turns.** `ceil(enemy HP / minimum non-crit damage of one use)`, with
  misses and crits ignored. It uses the same formula as the battle page's
  `N-hit kill, IGNORING ACC` line (`damage.rs` `find_kill` /
  `find_kill_hits`), but it is computed for every move. A 3-turn line that
  shows 99.6 % counts as 4, because 3 hits are not guaranteed.
- **Multi-hit moves (D9).**
  - A move with a random 2–5 hit count is assumed to hit **twice** per
    use, whatever hit count is picked on the battle page. The pick still
    decides which move the page highlights as best.
  - Gen 1 Wrap, Bind, Fire Spin and Clamp count the same way. The gen 1
    calc treats one use as the whole 2–5 turn trap (`gen1.rs`, the
    `FLAVOR_PARTIAL_TRAPPING` branch), and the trap costs one PP, so PP
    assumes the shortest trap (2 turns). They are therefore **not** in the
    lock table below; dividing again would double-count.
  - Fixed-count moves use their count: Double Kick and the other two-hit
    moves use 2. Triple Kick uses 3, because its kick count varies only
    through misses, and KO turns ignore misses.
  - Beat Up uses the page's value (it depends on the party, not on
    chance).
  - One use costs 1 PP however many times it hits. The per-use damage
    range already sums the hits in both the trainer and the wild branch of
    `recalculate_single_move` (e.g. `gen3.rs:765-776`), so wild fights
    need no special case.
- **Setup uses (D1).** PP follows the setup the battle page already feeds
  into the damage calc. The fight's damage ranges already include the
  stages from that setup: `calc_per_matchup_stage_modifiers` applies
  them, and self-boosts carry into later matchups. So the best move's KO
  turns already reflect the setup. On top of that, the setup moves
  themselves cost PP:
  - **Per-matchup stat-stage steppers** (`stat_stage_setup`). Each
    player-side count is that many uses in that matchup. This covers
    self-boosts (Swords Dance), enemy drops (Growl, String Shot) and
    damaging moves with a stat effect (Mud-Slap).
    - Belly Drum's stepper stores the resulting attack stage (6, or 2 in
      gen 2), not a count, so it costs 1.
    - A count on a Mimic slot is stored under the mimicked move and
      charged to the Mimic slot.
  - **Legacy global setup list** (`setup_moves`, which turns the steppers
    off). Each entry is one use, charged once per fight, in the first
    matchup.
  - **Same filter as the calc.** Only uses that also change the damage
    are counted, using the calc's own filter: a known move, a stat
    effect, and guaranteed or damaging. Stepper entries for moves the mon
    no longer knows are ignored, as the calc ignores them.
  - **Not saved.** The battle page's manual move highlights (the colour
    cycling) are not saved with the route, so PP ignores them.

### 1.3 From turns to PP: locks and Pressure

A KO needing `n` turns costs `n` PP, except in two cases.

**Locked moves (D10).** These charge PP once per lock, not per turn:

| Gen | Moves | Lock | PP for `n` turns | Source |
|---|---|---|---|---|
| 1 | Thrash, Petal Dance | 3–4 turns | `ceil(n / 3)` | `core.asm` `.ThrashingAboutCheck` skips `DecrementPP`; `effects.asm` `ThrashPetalDanceEffect` sets 2–3 more turns |
| 1 | Rage | until the fight ends | 1 per fight, charged in the first matchup that uses it | `decrement_pp.asm` returns while `USING_RAGE`; `FaintEnemyPokemon` clears only `ATTACKING_MULTIPLE_TIMES`, so the lock outlives enemy switches |
| 2 | Thrash, Outrage, Petal Dance | 2–3 turns | `ceil(n / 2)` | `effect_commands.asm:983` `DoTurn` skips PP under `SUBSTATUS_RAMPAGE` |
| 3–5 | Thrash, Outrage, Petal Dance, Uproar | 2+ turns | `ceil(n / 2)` | gen 3 `battle_scripts_1.s` runs `ppreduce` only without `STATUS2_MULTIPLETURNS`; gen 4 `battle_controller_player.c:335` marks PP as spent when the mon cannot pick a command |
| 3–5 | Rollout, Ice Ball | 5 turns | `ceil(n / 5)` | same |

- **Gen 2 Rollout** is charged every turn: `ROLLOUT` is not in
  `DoTurn`'s skip mask.
- **Rage in gens 2–5** is an ordinary move, charged per turn.
- **Locks don't carry into the next matchup.** A gen 1 Thrash or Wrap lock
  can carry over into the next enemy mon, but each matchup is charged on
  its own. This slightly over-counts PP (§9).
- **Gen 5** is assumed to match gen 4.
- The lock tables live in `xpr-data` `gen_consts.rs` as per-gen move-name
  lists (there is no attack flavour for them).

**Pressure (D8), gens 3–5.** Every use of a move that targets a Pressure
mon costs 1 extra PP. The sources are pokeemerald `Cmd_ppreduce` and
pokeplatinum `BattleControllerPlayer_DecrementPP`.

- **What doubles.** When the enemy in a matchup has Pressure
  (`EnemyPkmn.ability`), these cost double: the KO PP (after the lock
  rule, so a Thrash lock costs 2) and enemy-targeting setup (Growl).
- **What doesn't.** Self-targeting setup (Swords Dance) is unaffected.
- **Wild mons.** A wild mon's ability is unknown (the data gives wild mons
  none), so PP assumes Pressure when the species can have it: the worst
  case, like the damage's DV range.
- **Doubles.** Each enemy mon is its own matchup, so only that mon's
  Pressure counts. The spread-move case is in §9.

### 1.4 Edge cases

| Case | Rule |
|---|---|
| No damaging move, or every move is immune | Nothing is spent. The Moves card tooltip notes the fight |
| Metronome slot is best | Spend from the Metronome slot, using the picked move's numbers. With no pick, the slot has no damage and is not a candidate |
| Mimic slot is best, gen 1 | The copied move uses Mimic's PP (`effects.asm` `MimicEffect` swaps it into Mimic's slot). Spend the KO PP from the Mimic slot, plus 1 in the matchup where Mimic is first used |
| Mimic slot is best, gens 2–5 | The copied move has 5 PP of its own and never touches the party's PP: gen 2 `mimic.asm` sets it to 5, and `CheckMimicUsed` skips the party decrement. Spend 1 from the Mimic slot in the matchup where Mimic is first used, and nothing for the copied move's uses |
| Player transformed (`transformed = true`) | The copied moves have their own PP and are gone after the battle (gen 1 `decrement_pp.asm` skips the party PP while `TRANSFORMED`). Spend 1 PP of the Transform slot once per fight, and nothing else |
| Double battles | Spend per enemy mon (§9 for spread moves) |
| Wild fights | Each wild mon is its own matchup, built from its own fight item's init state (§4.2) |
| EXP split > 1 | Still spend (assumes the solo mon lands the KO) |
| Tiny minimum damage (Psywave, Super Fang-style moves) | The count can be large. It is shown as computed, and the tooltip names the move |
| Gen 1 crits ignore stat stages | Counted with the non-crit minimum, as the UI does |
| Setup the enemy does (enemy steppers) | Costs the player no PP. It already shows up in the damage, and so in the KO count |

---

## 2. Game facts (decomp-verified)

These come from the local decomps: `pokered`/`pokeyellow`,
`pokegold`/`pokecrystal`, `pokeruby`/`pokeemerald`/`pokefirered`, and
`pokediamond`/`pokeplatinum`/`pokeheartgold`. Gen 5 has no local decomp,
so the gen 5 column is assumed to match gen 4. The gen 5 TM/HM rule in
§1.1 is the user's (D3).

| | Gen 1 | Gen 2 | Gen 3 | Gen 4 | Gen 5 (assumed) |
|---|---|---|---|---|---|
| Max PP with `u` PP Ups | `base + u·min(⌊base/5⌋, 7)` | same as gen 1 | `base + ⌊base·20·u/100⌋` | same as gen 3 | same as gen 4 |
| 40-PP move, 3 PP Ups | 61 | 61 | 64 | 64 | 64 |
| PP Max item | — | — | yes (u → 3) | yes | yes |
| PP Up refused on | — | Sketch (ID check) | current max ≤ 4 (only Sketch) | base PP < 5 (`PP_UP_REQUIREMENT`, only Sketch) | base PP < 5 |
| Current PP on PP Up / PP Max | rises by the max-PP delta | same | same | same | same |
| Ether / Elixir | +10 | +10 | +10 | +10 (amount in NARC data) | +10 |
| Leppa / Mysteryberry (from bag) | — | Mysteryberry +5 | Leppa +10 | Leppa +10 | Leppa +10 |
| Pressure | — | — | +1 PP per use of a move targeting it | same | same |

These rules hold in every verified generation:

- **Refused items are kept.** PP Up / PP Max at 3 PP Ups, or on an
  ineligible move, shows a message and the item is not used up. An Ether
  or Elixir on a full move is the same. (The app consumes them anyway;
  see D7.)
- **Restores cap at the full max.** Ether and Elixir top out at the max
  including PP Ups.
- **Learning resets the slot.** Any learn (level-up, TM/HM, tutor,
  relearner, an evolution move) gives the slot the new move's base max
  and clears its PP Ups. Examples:
  - gen 1 `learn_move.asm:39`
  - gen 3 `SetMonMoveSlot` + `RemoveMonPPBonus`
  - gen 4: every caller zeroes `MON_DATA_MOVE1_PP_UPS`
- **Center heals and whiteouts refill PP**, PP-Up bonus included:
  - gen 1 `HealParty`
  - gen 2 `special HealParty` (`whiteout.asm:14`, `std_scripts.asm:118`)
  - gen 3 `HealPlayerParty`
  - gen 4 `Party_HealAllMembers`
- **Evolution never touches PP or PP Ups.**
- **Struggle** only happens when every move is at 0.

---

## 3. Data and engine (`xpr-data`, `xpr-engine`)

### 3.1 PP helpers on `GenData`

New in `xpr-data` (`gen_consts.rs` for the tables, `gen_data.rs` for the
methods):

```rust
pub enum PpAmount { Points(i64), Full }

pub enum PpItemEffect {
    RestoreOne(PpAmount),   // Ether 10, Max Ether full, Leppa Berry 10, Mysteryberry 5
    RestoreAll(PpAmount),   // Elixir/Elixer 10, Max Elixir/Elixer full
    PpUp,                   // one more PP Up (cap 3)
    PpMax,                  // PP Ups -> 3
}

impl GenData {
    /// `None` for anything that is not a PP item in this generation.
    pub fn pp_item_effect(&self, item_name: &str) -> Option<PpItemEffect>;
    /// Base PP plus the PP-Up bonus: gens 1-2 `base + u*min(base/5, 7)`,
    /// gens 3-5 `base + base*20*u/100` (§2).
    pub fn max_pp(&self, move_name: &str, pp_ups: u8) -> Option<i64>;
    /// Whether PP Up / PP Max can target this move: always in gen 1,
    /// otherwise only Sketch fails (§2).
    pub fn can_raise_pp(&self, move_name: &str) -> bool;
    /// Turns one PP buys while the move is locked in (§1.3); 1 for normal
    /// moves. Gen 1 Rage is its own case (one PP per fight).
    pub fn lock_turns_per_pp(&self, move_name: &str) -> i64;
}
```

Item names in the data files are listed below. Match on `sanitize_string`
so that casing differences resolve. Custom gens inherit the tables of
their generation.

| Gen | Names in `raw_pkmn_data/*/items.json` |
|---|---|
| 1 | `PP Up`, `Ether`, `Max Ether`, `Elixer`, `Max Elixer` |
| 2 | `Pp Up` (sic), `Ether`, `Max Ether`, `Elixer`, `Max Elixer`, `Mysteryberry` |
| 3–5 | `PP Up`, `PP Max`, `Ether`, `Max Ether`, `Elixir`, `Max Elixir`, `Leppa Berry` |

Move PP data is already complete. All five `moves.json` files have `pp`
on every move. Gen 5's 18 Shadow moves have `null` PP, and
`loaders.rs:778` already skips them.

### 3.2 PP Ups in `SoloPokemon`

- Add `SoloPokemon.pp_ups: [u8; 4]` and `SoloPokemonArgs.pp_ups:
  Option<[u8; 4]>`. `SoloPokemon::new` with `None` gives zeros (a fresh
  mon).
- **Carry forward in one place.** `RouteState::rebuild` uses
  `args.pp_ups.or(Some(cur.pp_ups))`. The eight existing rebuild call
  sites (`take_vitamin`, `eat_ev_berry`, `override_evs`, `rare_candy`,
  `defeat_pkmn`, `steal_held_item`, `hold_item`, `evolve`) then keep PP
  Ups without edits. Without this, a single missed site would silently
  wipe PP Ups.
- `learn_move`: zero the destination slot's PP Ups for every source,
  including deletion.
- `SoloPokemon::py_eq`: compare `pp_ups`. `recalc_suffix` stops early when
  a folder's leaving state is unchanged, so without this a PP-Up edit
  would not propagate.
- `SoloPokemon::serialize`: add a `pp_ups` key **only when non-zero**, so
  the golden state records of existing routes stay identical.
- PP Ups are not stored in the route file. They are derived from the
  events.

### 3.3 Target move on "Use item" events

```rust
pub struct InventoryEventDefinition {
    // ... existing fields ...
    /// The move a single-target PP item (Ether, PP Up, …) was used on.
    pub target_move: Option<String>,
    /// The item was tossed, not used: no PP effect.
    pub no_effect: bool,
}
```

- **Serialization.** Store both in the trailing object that already holds
  `custom_price` (`events.rs:81`), e.g. `{"target_move": "Surf"}` or
  `{"custom_price": 300, "no_effect": true}`. Write each key only when it
  is set. Existing routes stay byte-identical, and older builds ignore
  the keys because they only read `custom_price`.
- **Target by move name, not slot.** An edit to an earlier learn-move
  destination can change which slot holds a move, but the event still
  means "the Ether went on Surf". At apply time the name resolves to
  whichever slot holds that move.
- **Labels.** `to_string` becomes `Use Ether x1 on Surf` or `Toss Ether
  x1` for PP items. Other items keep `Use/Drop …`. The event type stays
  `TASK_USE_ITEM`, so filters, search and route compare are unaffected.
- **Undo.** `SnapNode` snapshots copy definitions, so undo covers the new
  fields with no extra work.

### 3.4 `RouteState::use_item`

Replace the `remove_item` call for the Use case with:

```rust
pub fn use_item(&self, gen: &GenData, item: &str, amount: i64,
                target_move: Option<&str>, no_effect: bool)
    -> Result<(RouteState, String /* error */, String /* warning */), String>
```

**Bag first, always (D7).** The item always leaves the bag, including
uses the game would refuse. This follows the app's existing convention:
a vitamin over the cap is consumed and its row shows the error. Ethers
must be consumed anyway, because the engine cannot see current PP, so
this keeps the two cases alike.

1. Remove the item from the bag exactly as today: strict first, then
   forced with the error recorded.
2. Stop here if `no_effect` is set or the item is not a PP item.
3. `PpUp` / `PpMax`:
   - No target: **warning** `Choose the move PP Up was used on`.
   - Target not known: **error** `<mon> does not know Surf`.
   - `!can_raise_pp`: **error**.
   - Already 3 PP Ups: **error** `Ineffective PP Up: Surf already has 3 PP
     Ups`.
   - Otherwise apply up to the cap. The uses beyond the cap get the same
     error, and the items stay consumed.
4. `RestoreOne`:
   - No target: **warning** (amber row, like bag-reorder warnings; D5).
   - Target not known: **error**.
   - The engine does not change PP; the ledger applies the restore.
5. `RestoreAll`: nothing to validate.

In `recalc.rs` `apply_item`, route `!is_acquire && !with_money` to
`use_item` and put its warning into `EventItem.warning_message`.

---

## 4. Fight spend (`xpr-calc`)

### 4.1 `MoveRenderInfo.pp_turns`

In `recalculate_single_move`, compute the KO turns that PP uses:

```
pp_min    = minimum damage of one use under §1.2's hit-count rules
pp_turns  = (pp_min > 0).then(|| ceil(hp / pp_min))
```

- **Single-hit and fixed-count moves.** `pp_min` is the min of the range
  the page already computed: `hits.min_damage()` when a hit model exists,
  otherwise `normal.min_damage`.
- **Variable 2–5 hit moves.** `pp_min` comes from one extra
  `calculate_damage` / `hit_model` call with `custom_move_data =
  MULTI_HIT_2`, and Triple Kick uses its 3-kick model. This is a plain
  damage calc (no kill search), so it is cheap.
- **Wild branch.** `recalculate_single_move`'s wild branch builds no hit
  model, but its per-use range already sums the hits, so the same rule
  applies there.
- **Golden output.** The field is not added to `to_json`, so the golden
  battle output is unchanged.

### 4.2 Fight inputs without `&Router`

```rust
/// Everything a fight's spend depends on. Owned and `Send`, so it can be
/// computed on rayon threads or a worker thread.
pub struct FightInput {
    pub group_id: NodeId,
    pub def: EventDefinition,
    pub is_wild: bool,
    /// One per fight item in battle order: the mon KO'd and the item's init state.
    pub matchups: Vec<(EnemyPkmn, Arc<RouteState>)>,
}
impl FightInput { pub fn from_router(router: &Router, group_id: NodeId) -> Option<FightInput>; }
```

**Trainer fights.** Split `BattleSummary::load_from_event` into
`load_from_parts(gen, def, &matchups, cfg)` plus a thin wrapper that keeps
today's signature. The split must be output-identical: run
`xpr-golden verify` with battles. The k-th fight item of a group is the
k-th matchup. `load_from_event` already pairs them this way:
`to_defeat_mon` matched in `mon_order`, with level-up learn items skipped.
A move learned mid-fight therefore shows up from the next matchup on.

**Wild fights.** Build the matchups per item as well, through a separate
`load_wild_from_parts`, instead of `load_from_state`.

- `load_from_state` starts from the group's init state and chains
  `defeat_pkmn` itself. That keeps levels right but misses moves learned
  between wild mons in one group.
- The wild-only parts of `load_from_state` (min/max-DV mons,
  `is_wild_battle`) move into the parts loader.
- The battle page and `xpr-golden` keep calling `load_from_state`, so
  their output doesn't change.
- All 1,760 wild groups in the three benchmark routes hold one mon, so
  the difference is rare, but the per-item version costs nothing extra.

### 4.3 `pp::fight_pp_spend`

New module `xpr-calc/src/pp.rs`:

```rust
pub enum SpendKind { Setup, Ko, Mimic, Transform }

pub struct SlotSpend {
    pub slot: usize,
    pub move_name: String,
    pub pp: i64,          // after locks and Pressure
    pub turns: i64,       // before them (for the tooltip)
    pub kind: SpendKind,
}

pub struct MatchupSpend {
    pub spends: Vec<SlotSpend>,   // setup first, then the KO move
    pub note: Option<String>,     // "no damaging move", "Pressure", …
}

pub fn fight_pp_spend(gen: &GenData, input: &FightInput, cfg: &SummaryConfig) -> Vec<MatchupSpend>;
```

- Build the summary with `test_moves_enabled: false` and an empty
  `test_moves` list. This keeps test moves out of the choice and skips
  computing them.
- **Setup spends.** Add
  `BattleSummary::player_setup_uses(gen, mon_idx) -> Vec<(slot, uses)>`
  (§1.2).
  - **Shared filter.** Move the per-move filter out of
    `calc_per_matchup_stage_modifiers` (its "Step 1" loop) into a helper
    that both functions call. PP then counts exactly the uses that the
    damage reflects. Golden verify guards the refactor.
- **KO spend.**
  - Pick the best of `player_move_data[i][0..4]`: the highlighted move,
    or the PP rule of §1.2 when the strategy is None. Skip Struggle, empty
    slots and `min_damage == -1`.
  - Take its `pp_turns` and convert them with the lock rule (§1.3). Gen 1
    Rage is charged once per fight, which is one reason the function sees
    the whole fight rather than one matchup at a time.
  - The ranges already use the matchup's post-setup stages, so nothing
    extra is needed for "setup lowers the KO count".
- **Pressure.** If the matchup's enemy has Pressure (gens 3–5), double
  the KO PP and the PP of enemy-targeting setup uses.
- **Special cases.** Apply the rules from §1.4: the Mimic generation
  split, Transform, Metronome.

### 4.4 Cache

```rust
pub struct FightSpendCache { entries: HashMap<NodeId, (FightKey, Arc<Vec<MatchupSpend>>)> }
struct FightKey {
    def: EventDefinition,              // PartialEq already derived
    players: Vec<EnemyPkmn>,           // compared with py_eq (+ badges)
    enemies: Vec<EnemyPkmn>,
    calc: CalcConfig, strategy: String,
    version: String,
}
```

- Look up by group id. It is a hit only if the stored key equals the new
  one. Item ids cannot be used: `apply_group` mints new ones on every
  recalc.
- Clear the cache on route load or version change.
- **Derived from `def`, so they need no separate key fields:**
  `is_wild`, the double-battle flag, and second trainers.
- **Vitamin shadows.** The battle page's vitamin "shadows" are real
  vitamin events inserted into the route (`battle.rs:567`, `574`). The
  page only remembers their ids, so they reach the key through the player
  states like any other event.
- **Why it pays off.** An edit late in the route invalidates only the
  fights after it. Editing notes on a fight invalidates just that fight.

### 4.5 Performance

- Compute cache misses with `rayon` `par_iter` over `FightInput`s. `rayon`
  is already a workspace dependency (used by `xpr-golden`).
- Add a `pp_bench` bin next to `candy_bench`. Measure the three routes
  from §0 in two cases: (a) a cold cache, (b) a candy at the first gym,
  then asking for the PP before the last event.
- Budget: under 50 ms worst case, and near 0 on cache hits.
- The Yellow route (1,815 fights) is the most likely to miss the budget.
  If it does, move the misses to a worker thread (`FightInput` is
  `Send`). Consumers then show the last PP dimmed, with `updating…`,
  until the result arrives.
- **Measured** (release build, 24 threads). Every case is within budget,
  so there is no worker thread:

  | Route | Cold | No-op | Late edit | Candy before first gym |
  |---|---|---|---|---|
  | Crystal (254 fights) | 15.7 ms | 0.0 ms | 9.1 ms | 10.0 ms (28 fights recomputed) |
  | Emerald (328 fights) | 8.2 ms | 0.0 ms | 3.6 ms | 4.9 ms (42) |
  | Yellow (1,815 fights) | 29.4 ms | 0.0 ms | 18.8 ms | 23.8 ms (924) |

  A candy recomputes fewer fights than expected, because the level curve
  soon absorbs the extra exp and the later matchups match the cache
  again.

---

## 5. Ledger and UI (`xpr-app`)

### 5.1 `pp_ledger.rs`

```rust
pub struct SlotPp { pub move_name: String, pub cur: i64, pub max: i64, pub pp_ups: u8 }
pub struct PpSnapshot { pub slots: [Option<SlotPp>; 4] }
pub struct PpChange { pub amount: i64, pub reason: String }   // "Brock: Onix (Water Gun, 3 hits)", "Brock: Onix (Swords Dance ×2)"

pub struct PpLedger {
    revision: u64,
    cfg_key: (CalcConfig, String),
    before: HashMap<NodeId, PpSnapshot>,          // folders, groups and items
    notes: HashMap<NodeId, Vec<String>>,          // "game would refuse this Ether", …
    history: HashMap<NodeId, [Vec<PpChange>; 4]>, // per slot since its last refill (tooltips)
    spends: FightSpendCache,
}

impl PpLedger {
    /// Recomputes only when the route revision or the calc config /
    /// strategy changed since the last call; otherwise free.
    pub fn ensure(&mut self, ctrl: &MainController, cfg: &Config);
    pub fn before(&self, id: NodeId) -> Option<&PpSnapshot>;
}
```

- **Pull, not push.** Every consumer calls `ensure` before reading: the
  Moves card, the banner, the Moves-card context menu (quick add's **On
  move** menu lists move names only, so it needs no PP). Nothing rebuilds
  the ledger unless something asks for PP. Candy clicks on the Battle
  Summary tab, where nothing shows PP, pay nothing.
- Add a `route_revision: u64` to `MainController`. Bump it in
  `on_route_change`, on version change and on route load.
- **Rebuild.** Collect a `FightInput` for every fight group (cheap).
  Resolve spends through the cache. Then walk the tree depth-first:
  - Folder: record `before[folder]`.
  - Group: record `before[group]`; then for each item of `event_items`:
    1. Record `before[item]`.
    2. Reconcile the ledger's slots with `item.init_state`'s move list.
    3. Apply the item's effect (§1.1).
    4. Reconcile with `item.final_state`'s move list, using the learn
       rules for learn-move items.

  The reconcile steps keep the ledger correct for any move-list change it
  does not model explicitly. An unexpected change gets full PP for the
  new move and a `log::debug!`.
- Read max PP from the state the ledger is looking at:
  `gen.max_pp(move, solo_pkmn.pp_ups[slot])`. On a PP-Up item, the
  current PP rises by the max-PP delta.

### 5.2 Moves card (`state_views.rs`)

- `moves_card` gains `pp: Option<&PpSnapshot>`.
- Each row shows the move name left and `cur / max` right-aligned. The
  34 px rows have room.
- PP Ups show as up to three small pips in `theme.secondary` before the
  numbers.
- Colours:
  - `cur <= 0`: `theme.failure`.
  - The selected event is a fight, and this slot's spend is more than
    `cur`: `theme.warning`.
  - Otherwise: `theme.secondary`.
- The hover tooltip lists `history` for the slot, e.g. `−3 Brock: Onix
  (Water Gun, 3 hits)`, `−2 Brock: Geodude (Swords Dance ×2)`, and `Full
  at Pewter City PkmnCenter`.
- With no state, the numbers show `—` and the card keeps `last_pkmn` as
  today.
- Update `design/pre_event_state/SPEC.md` §5.4 and the `gen1_yellow.html`
  mockup in the same change.

### 5.3 Warning banner

`event_details.rs` `selected_warnings` (line 555) appends PP messages for
the selected event:

- **Any event.** `Surf is at −3 PP before this event: heal first`, for
  each move at or below 0.
- **A whole fight (group) selected.** The totals per move across every
  matchup, against the PP before the fight: `This fight uses Surf ×6 (2
  PP left)`.
- **One matchup (fight item) selected.** That matchup's spend only:
  `This matchup uses Surf ×2 (1 PP left)`.
- **Ledger notes** for the event, e.g. `the game would refuse this Ether`.

These are informational. They do not make the row amber: row state stays
engine-owned.

### 5.4 Creating and editing PP-item events

- **`editors.rs` `InventoryEventEditor`** (line 897, `get_event` line
  1010).
  - Single-target PP items get an **On move** dropdown with the four moves
    of the event's init state, plus a **Tossed (no effect)** checkbox.
  - Elixirs get only the checkbox.
- **`inline_creator.rs` `use_item`.**
  - `fields()` starts at line 403; `build()` starts at line 447, and its
    `use_item` branch is at line 479.
  - Add a `Move` field when the chosen item needs a target, reusing the
    `refresh_dest` / `slot_template` pattern (line 249). Today PP items
    fall through to the plain `InventoryEventDefinition` branch.
- **`quick_add.rs`** (line 391).
  - Enable **Use** for PP items. Elixirs create the event directly.
  - Single-target items get an **On move** menu in the item grid; there is
    no dialog (see §11).
  - **Drop** on a PP item sets `no_effect: true`.
- **Moves card context menu.** Right-click a move row to get `Use <item>
  on <move>` for each PP item in the bag. This inserts the event before
  the selected event. Left-click keeps opening the tutor dialog.
- **Modal rule (CLAUDE.md).** Everything above uses real widgets
  (`ui.interact`, `context_menu`, dialogs through `dialogs::modal`). If
  any raw pointer or key read is added, check `behind_modal` and add a
  case to `tests/modal_input.rs`.

### 5.5 Later: route list

A PP marker (or column) on fights whose spend exceeds the moves' PP.
This needs the whole-route ledger on every change, so it waits until the
§4.5 numbers are known.

---

## 6. Recorder

The recorder reads no PP data today. It reads only move names per slot
(`xpr-recorder/src/games/gen1.rs:100` `player_moves`). Every PP item lost
from the bag becomes a plain Use event through the generic branch of
`item_cache_update`: `xpr-recorder/src/games/gen1.rs:849`, and its
equivalents in `gen2.rs`, `gen3.rs`, `gen45.rs`, `gen5.rs` in the same
folder.

- **v1.** Recorded Elixirs count as used (D6). Recorded Ethers and PP Ups
  arrive without a target and show the amber "choose the move" warning
  (D5).
- **Later.** Read the lead's per-move PP and PP-Up bits from GameHook
  (the property paths per mapper still need finding). Diff them across
  the bag change and fill `target_move`, the same way `move_cache_update`
  diffs move names for TMs. Cover it with a scenario in
  `docs/rust_port/recording/scenarios/`.

---

## 7. Tests

| Where | What |
|---|---|
| `xpr-data/tests` | `pp_item_effect` for every name and gen, including `Elixer`, `Pp Up` and `Mysteryberry`; `max_pp` per gen, including the caps; `can_raise_pp(Sketch)`; `lock_turns_per_pp` per gen |
| `xpr-engine/tests/pp_items.rs` (new) | PP Up and PP Max: apply, cap, error texts, items consumed even when refused; missing target → warning; unknown target → error; learn move (level-up, TM, delete) zeroes only that slot's PP Ups; PP Ups survive rare candy, vitamins, evolution and hold item (the carry-forward); round trip of `target_move` / `no_effect`; an old route saves byte-identical |
| `xpr-engine/tests/incremental_recalc.rs` | Add routes with PP-Up events: incremental recalc equals full recalc |
| `xpr-calc/tests/fight_pp.rs` (new): KO | `pp_turns` equals the `-1` kill line for single-hit moves wherever that line exists; the user's example (2HKO chance + 100 % 3HKO → 3); a 99.x % line counts `ceil`; strategy None picks fewest turns; test moves ignored; `mon_order` reordering maps spends to the right fight items; a mid-fight level-up move; a wild group with two mons and a level-up move between them |
| `fight_pp.rs`: multi-hit | Fury Attack with "5 hits" picked still counts 2 hits; Double Kick 2; Triple Kick 3; Beat Up follows the page |
| `fight_pp.rs`: locks and Pressure | Gen 1 Thrash 4 turns → 2 PP, 3 turns → 1; gen 1 Wrap one PP per use whatever the trap-length pick; gen 1 Rage over the fight → 1 PP total; gen 2 Outrage 3 turns → 2; gen 2 Rollout per turn; gen 3 Rollout 5 turns → 1; Pressure doubles KO PP (including after a lock) and Growl, not Swords Dance; no Pressure in gens 1–2 |
| `fight_pp.rs`: setup | Swords Dance ×2 costs 2 and lowers the KO count vs. no setup; a self-boost in matchup 1 lowers matchup 2's count without charging again; Growl ×1 on one matchup only; Belly Drum (stage 6, gen 2 stage 2) costs 1; a Mimic-slot stepper charges the Mimic slot; the legacy global list is charged once per fight; stepper entries for unknown moves are ignored; enemy steppers cost nothing |
| `fight_pp.rs`: special moves | Gen 1 Mimic: KO PP + 1 from the Mimic slot; gen 2+ Mimic: only 1; Transform: 1 per fight; Metronome with and without a pick |
| `xpr-golden verify` | Unchanged output: `load_from_parts` split, shared setup filter, no new keys for existing routes |
| `xpr-app/tests/pp_ledger.rs` (new) | Heal and blackout refill; Ether caps at max; negative arithmetic; Elixir hits all four, and an untargeted Elixir counts as used; a refused-Ether note; gen 5 TM and HM carry-over (positive, negative, capped at the new max, into an empty slot → full); gens 1–4 TM, level-up and tutor → full; disabled events; revision invalidation; `ensure` is free when nothing changed; the cache is reused when an unrelated late event changes |

---

## 8. Phases

1. **Data and engine.** §3, plus the target fields in the three editors
   (§5.4 minus the dialog and the context menu). Routes can save targets,
   and PP Ups are validated. There is no PP display yet.
2. **Fight spend.** §4, including setup, locks, Pressure and the
   multi-hit rule. Also `pp_bench` and the numbers from §4.5.
3. **Ledger and UI.** §5.1–5.4: Moves card, banner,
   quick-add **On move** menu, context menu, SPEC/mockup update. Add runtime
   notes and the §9 gaps to `KNOWN_ISSUES.md`.
4. **Close gaps.** Pick from §9, plus a saved per-matchup PP override, the
   route-list marker, recorder target detection, and PP in the notes
   export.

Each phase ships on its own: `cargo test --workspace` green and
`xpr-golden verify` identical.

---

## 9. Known gaps (documented, to close later)

These are deliberate v1 simplifications. Phase 3 copies them into
`KNOWN_ISSUES.md` so they show up next to the feature.

| Gap | Effect | Closing it |
|---|---|---|
| **Held berries (D11).** A held Leppa Berry (gens 3–5) restores 10 PP once when a move hits 0; a held Mysteryberry (gen 2) restores 5 | The ledger ignores them, so PP reads up to 10 lower than the game until the next heal | Model the trigger in the ledger. The berry is eaten, and that must reach the engine's held item, which breaks the "ledger never feeds back" rule. So it needs either a held-berry event the user adds, or an engine hook. (Using a Leppa / Mysteryberry from the bag is modeled; it is an Ether-type item) |
| Damaging setup moves (Mud-Slap ×2) | Their uses cost PP, but their damage is not taken off the enemy's HP, so the KO move is over-counted | Sum the per-use damage at each stage and subtract it before computing KO turns |
| Locks spanning matchups (gen 1 Thrash / Wrap) | Each matchup is charged on its own, so PP is slightly over-counted | Carry the remaining lock turns across matchups in `fight_pp_spend` |
| Spread moves in double battles | Charged once per enemy mon, though one use hits both | Detect spread targeting and share the use across the two matchups |
| Skill Link (gens 4–5) | The calc does not model it, and PP assumes 2 hits where the game always hits 5, so PP is over-counted | Model Skill Link in the calc, then use 5 hits for PP |
| Thief / Covet steals, Pay Day, weather or screen moves toggled on the battle page | Their own PP is not charged | Charge them like setup uses |
| Manual highlights on the battle page | Not saved, so PP cannot follow them | A saved per-matchup PP override |

---

## 10. Decisions

Blackout is not listed here: it refills PP, because the decomps show a
whiteout heals the party in every generation (§2).

| # | Question | Decision (all confirmed 2026-09-25) |
|---|---|---|
| D1 | Should setup moves recorded on the battle page cost PP? | **Yes.** Stat-stage setup lowers the KO count through the damage calc, and the setup move pays its own PP (§1.2) |
| D2 | Best move for PP | **The page's highlight strategy.** When the strategy is None, fewest guaranteed turns (§1.2) |
| D3 | Gen 5 TM or HM over a move | **The new move keeps the old move's PP, capped at the new max.** The old slot's PP Ups reset, and HMs follow the TM rule. The §2 formulas are assumed to be gen 4's (no local decomp to check) |
| D4 | Restoring a negative move | **Arithmetic, for now:** −3 + Ether = 7 |
| D5 | Existing untargeted Ether / PP Up "Use/Drop" events | **Amber warning** until a target is set, or until the event is marked tossed |
| D6 | Untargeted Elixir events | **Count as used.** Tossing must be marked explicitly |
| D7 | Uses the game would refuse (PP Up at 3, Ether on a full move) | **Always consume the item.** A refused PP Up is an error on the row; an Ether on a full move gets a ledger note |
| D8 | Pressure | **v1**, gens 3–5 (§1.3) |
| D9 | Variable multi-hit moves | **Assume 2 hits per use.** Fixed-count moves use their count (Double Kick 2, Triple Kick 3) (§1.2) |
| D10 | Locked moves | **v1, following the decomps** (§1.3). Gen 1 Thrash/Petal Dance/Wrap-type moves charge per lock, and gen 1 Rage costs 1 per fight. Rage is per turn in gens 2–5. Gens 2–5 rampage moves, and Rollout/Uproar in gens 3–5, also charge per lock |
| D11 | Held Leppa / Mysteryberry | **Ignored in v1**, and documented as a gap to close (§9) |

---

## 11. Implementation notes (2026-09-25)

Where the code lives:

| Part | Code | Tests |
|---|---|---|
| PP item, max-PP, PP-Up and lock tables | `xpr-data/src/gen_consts.rs` (`PpItemEffect`, `pp_item_effect`, `PpLock`, `pp_lock`); `gen_data.rs` (`max_pp`, `can_raise_pp`, `pp_lock`, `has_pressure_pp_cost`) | `xpr-engine/tests/pp_items.rs` `generation_tables` |
| PP Ups, target move, `use_item` | `xpr-engine/src/state.rs` (`SoloPokemon.pp_ups`, `slot_of_move`, `RouteState::use_item`); `events.rs` (`target_move`, `no_effect`, `use_on`); `recalc.rs` | `xpr-engine/tests/pp_items.rs` |
| KO turns, loaders, shared setup filter | `xpr-calc/src/battle_summary.rs` (`MoveRenderInfo.pp_turns`, `pp_hit_count_option`, `load_from_parts`, `load_wild_from_parts`, `player_setup_uses`) | `xpr-calc/tests/fight_pp.rs` |
| Fight spend and cache | `xpr-calc/src/pp.rs` | `xpr-calc/tests/fight_pp.rs` |
| Ledger | `xpr-app/src/pp_ledger.rs`; `MainController::ensure_pp`, `route_revision`, `use_pp_item_before_selected` | `xpr-app/tests/pp_ledger.rs` |
| UI | `state_views.rs` (Moves card, tooltip, right-click menu); `event_details.rs` (banner lines); `editors.rs` (**On move**, **Tossed**); `inline_creator.rs` (**On:** field); `quick_add.rs` | the Pre-Event State smoke screenshot |
| Benchmark | `xpr-app/src/bin/pp_bench.rs` | |

Differences from the text above:

- **No `PpItemTargetDialog`.** The quick-add popover is a raw `Area`, and
  opening a modal from it would need new plumbing. An **On move** menu in
  its item grid does the same job with one click fewer. The Moves card's
  right-click menu covers "use it on this move" from the Pre-Event State.
- **Gen 1 trapping moves** follow the multi-hit rule, not the lock table
  (§1.2): the calc already counts one use as the whole trap.
- **Wild Pressure** assumes the worst case (§1.3), because wild mons carry
  no ability.
- **Golden check.** No golden corpus is left on disk (the Python generator
  was removed). `xpr-golden dump --battles` was added instead, and both
  the old and the new build dumped 67 saved and test routes.
  - Every state, label and all 8,428 battle summaries were identical.
  - The only differences were the intended amber warnings on 288
    untargeted Ether / Max Ether / PP Up / Leppa Berry uses (D5).
