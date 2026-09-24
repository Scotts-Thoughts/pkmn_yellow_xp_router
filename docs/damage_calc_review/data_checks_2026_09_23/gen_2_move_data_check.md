# Gen 2 Move Data Verification Report

Verified: `raw_pkmn_data/gen_two/moves.json` (251 moves) and `raw_pkmn_data/gen_two/type_info.json`
against the **pokecrystal** decompilation (byte-for-byte identical to **pokegold** for every file
in scope — see §5). All ROM citations are `pokecrystal/<path>:<line>` and apply equally to
pokegold unless noted otherwise.

Method: wrote Python scripts to parse `moves.asm`, `move_constants.asm`,
`move_effect_constants.asm`, `effects_pointers.asm`, `effects.asm`, `critical_hit_moves.asm`,
`type_matchups.asm`, `type_boost_items.asm`, `type_constants.asm` and `data/items/attributes.asm`
into structured data, diffed field-by-field against the app JSON (matched by `rom_id` = the move's
0x-indexed position in `move_constants.asm`, confirmed identical for all 251 entries), then
hand-read the relevant `effects.asm` scripts and `engine/battle/effect_commands.asm` /
`engine/battle/move_effects/*.asm` command implementations for every anomaly and every move named
explicitly in the verification brief.

---

## 1. Mismatch table (base_power / type / accuracy / pp / effect_chance)

| Move | Field | App value | ROM value | ROM citation | Suggested fix |
|---|---|---|---|---|---|
| Struggle | `type` | `"none"` | `NORMAL` | `data/moves/moves.asm:181` (`move STRUGGLE, EFFECT_RECOIL_HIT, 50, NORMAL, 100, 1, 0`) | Change to `"Normal"`. Struggle is ordinary Normal-type for damage purposes — no special-case bypassing type effectiveness/STAB exists anywhere in `engine/battle/` (checked `core.asm`, `effect_commands.asm`); the only special-casing for `STRUGGLE` is disallowing Mimic/Disable/Sketch/Encore/Spite from targeting it. |
| Curse | `type` | `"Ghost"` | `CURSE_TYPE` (id `$13`/19, declared in `constants/type_constants.asm:23`, one past `UNUSED_TYPES`) | `data/moves/moves.asm:190` (`move CURSE, EFFECT_CURSE, 0, CURSE_TYPE, 100, 10, 0`) | `CURSE_TYPE` is not a real/displayable type and never appears in `type_matchups.asm`. The Ghost-vs-non-Ghost branch inside the `curse` battle command checks the **user's own species type**, not this field, so it doesn't literally matter for damage calc (Curse has 0 power). Still doesn't match the raw byte; consider using the app's existing `"none"` type sentinel (already used for Struggle... which should actually be `"Normal"`) instead of the real type "Ghost", to avoid Curse being treated as a Ghost-type *move* for e.g. type-boost-item logic. |
| Spikes | `effects[0].chance` | `30` | effect_chance byte = `0` (unconditional) | `data/moves/moves.asm:207`; command has no RNG at all — `engine/battle/move_effects/spikes.asm:9-16` (only failure condition is "spikes already down") | Change to `100`, matching the pattern already used for Leech Seed/Toxic/etc. (guaranteed on-hit effects use `chance: 100`). |
| AncientPower | `effects[0].chance` | `20` | `10` | `data/moves/moves.asm:262` (`move ANCIENTPOWER, EFFECT_ALL_UP_HIT, 60, ROCK, 100, 5, 10`) | Change to `10`. |
| Flame Wheel | `effects` | `[]` (missing) | 10% Burn | `data/moves/moves.asm:188` (chance byte `10`); script `data/moves/effects.asm:1466-1486` (`FlameWheel:` calls `effectchance` then `burntarget`, structurally identical to `BurnHit:` at `effects.asm:76-95`) | Add `{"chance": 10, "status": "Burn"}`. |
| Snore | `effects` | `[]` (missing) | 30% flinch | `data/moves/moves.asm:189` (chance byte `30`); script `data/moves/effects.asm:1299-1319` (`Snore:` calls `effectchance` … `flinchtarget`) | Add `{"chance": 30, "flinch": true}`. |
| Twister | `effects` | `[]` (missing) | 20% flinch | `data/moves/moves.asm:255` (chance byte `20`); script `data/moves/effects.asm:1887-1907` (`Twister:` calls `effectchance` … `flinchtarget`, in addition to `doubleflyingdamage` which the app already captures as the `bonus_fly_damage` flavor) | Add `{"chance": 20, "flinch": true}`. |
| Swagger | `effects[1].status` | `"Confuse"` | (Confusion condition) | `data/moves/effects.asm:1575` (`confusetarget`) vs. the 7 other confusion-inflicting moves at `effects.asm:973` (`DoConfuse`/`confuse`) which the app all correctly labels `"Confusion"` | Rename to `"Confusion"` for consistency — every other confuse-inflicting move (Supersonic, Confusion, Confuse Ray, Psybeam, Dizzy Punch, Sweet Kiss, DynamicPunch) uses `"Confusion"`; Swagger is the sole outlier. |
| Fly | `attack_flavor` | `"two_turn_semi_invulrnerable"` | — | `data/moves/moves.asm:35` / `effects_pointers.asm` `EFFECT_FLY` | Both Fly and Dig use the identical `EFFECT_FLY` effect (`data/moves/moves.asm:35` and `:107`, same `effects.asm` `Fly:` script), so they must carry the *same* flavor tag. Currently they use two different misspellings of "invulnerable". Pick one correct spelling (e.g. `two_turn_semi_invulnerable`) and apply to both. |
| Dig | `attack_flavor` | `"two_turn_semi_invlunerable"` | — | `data/moves/moves.asm:107` (`move DIG, EFFECT_FLY, 60, GROUND, 100, 10, 0`) | See Fly, above — same fix. |

All other `base_power`/`type`/`accuracy`/`pp` values for all 251 moves matched `moves.asm` exactly
(accuracy byte for the three `EFFECT_ALWAYS_HIT` moves — Swift, Faint Attack, Vital Throw — is
correctly represented as `null` even though the raw byte is `100`, per the app's documented
convention).

### Base-power placeholder inconsistencies (not functional bugs — flagging for data hygiene)

`moves.asm` stores a dummy power byte (`0` or `1`, inconsistently) for every move whose real damage
is computed by a dedicated formula rather than the normal power stat. I confirmed by reading each
move's `effects.asm` script that the power byte is **never read** by any of these handlers (each
uses its own named command instead):

| Effect | Command (effects.asm → effect_commands.asm) | Moves | App power values |
|---|---|---|---|
| `EFFECT_OHKO` | `ohko` | Guillotine, Horn Drill, Fissure | `0, -1, -1` |
| `EFFECT_COUNTER` | `counter` (`effects.asm:1274`) | Counter | `0` |
| `EFFECT_MIRROR_COAT` | `mirrorcoat` (`effects.asm:1853`) | Mirror Coat | `0` |
| `EFFECT_LEVEL_DAMAGE`/`EFFECT_STATIC_DAMAGE` share `StaticDamage` (`effects.asm:1239`), differentiated at runtime inside `constantdamage` | Seismic Toss, Night Shade, SonicBoom, Dragon Rage | `-1, -1` (SonicBoom/Dragon Rage correctly use their real fixed-damage power, 20/40) |
| `EFFECT_PSYWAVE`/`EFFECT_SUPER_FANG` (fall through to `StaticDamage`) | | Psywave, Super Fang | `-1, -1` |
| `EFFECT_REVERSAL` | `reversal` via `flail_reversal_power.asm` table | Flail, Reversal | `-1, -1` |
| `EFFECT_RETURN`/`EFFECT_FRUSTRATION` | `happinesspower` (`engine/battle/move_effects/return.asm`) / `frustrationpower` (`.../frustration.asm`) | Return, Frustration | `102, -1` |
| `EFFECT_PRESENT` | `present` | Present | `-1` |
| `EFFECT_MAGNITUDE` | `getmagnitude` via `magnitude_power.asm` | Magnitude | `-1` |
| `EFFECT_HIDDEN_POWER` | `hiddenpower` | Hidden Power | `0` |

None of this changes in-game behavior (the field is dead data for these effects), but the app is
inconsistent about which sentinel it uses (`-1` vs `0` vs a computed value):
- **Return vs. Frustration**: I derived both formulas from `return.asm`/`frustration.asm`:
  `Return power = floor(happiness*10/25)`, `Frustration power = floor((255-happiness)*10/25)`.
  Both cap at **102** (at happiness 255 and 0 respectively — perfectly symmetric). The app stores
  Return's computed max (`102`) but Frustration's generic sentinel (`-1`); consider making these
  consistent (either both `102` or both `-1`).
- **Guillotine** (`0`) vs. **Horn Drill/Fissure** (`-1`) — all three are `EFFECT_OHKO`, but use two
  different sentinels.
- **Counter/Mirror Coat** (`0`) vs. **Flail/Reversal/Psywave/Super Fang/Night Shade/Seismic
  Toss/Present/Magnitude/Horn Drill/Fissure** (`-1`) — same situation.
- **Hidden Power** (`0`) is a *damaging* move (unlike status moves, where the brief says `0`/`null`
  is expected) with IV-dependent power; it uses a different sentinel than its variable-power peers.

---

## 2. `attack_flavor` findings

- **`high_crit`**: present on exactly the 7 moves in `data/moves/critical_hit_moves.asm:2-8`
  (Karate Chop, Razor Wind, Razor Leaf, Crabhammer, Slash, Aeroblast, Cross Chop) and no others.
  Confirmed with a full-file grep across all 251 moves. **No omissions, no spurious tags.**
- **`multi_hit`** (`EFFECT_MULTI_HIT`): present on exactly the 8 moves that use it (DoubleSlap,
  Comet Punch, Fury Attack, Pin Missile, Spike Cannon, Barrage, Fury Swipes, Bone Rush). Correct.
- **`two_hit`** (`EFFECT_DOUBLE_HIT` / `EFFECT_POISON_MULTI_HIT`): present on Double Kick,
  Bonemerang (both `EFFECT_DOUBLE_HIT`) and Twineedle (`EFFECT_POISON_MULTI_HIT`). Correct.
- **`one_hit_ko`** (`EFFECT_OHKO`): Guillotine, Horn Drill, Fissure — correct.
- **`fixed_damage`** (`EFFECT_STATIC_DAMAGE`): SonicBoom, Dragon Rage — correct.
- **`level_damage`** (`EFFECT_LEVEL_DAMAGE`): Seismic Toss, Night Shade — correct.
- **`psywave`** / **`super_fang`** / **`trap`** (`EFFECT_TRAP_TARGET`: Bind, Wrap, Fire Spin, Clamp,
  Whirlpool) / **`quarter_recoil`** (`EFFECT_RECOIL_HIT`: Take Down, Double Edge, Submission,
  Struggle) / **`drain_hp`** (`EFFECT_LEECH_HIT`: Absorb, Mega Drain, Leech Life, Giga Drain, plus
  `EFFECT_DREAM_EATER`: Dream Eater) / **`recharge`** (`EFFECT_HYPER_BEAM`: Hyper Beam) /
  **`reversal`** (`EFFECT_REVERSAL`: Flail **and** Reversal, both) — all present on exactly the
  expected move sets, no omissions or spurious tags.
  - Verified `quarter_recoil` is really **1/4**: `engine/battle/effect_commands.asm:5670-5690`
    (`BattleCommand_Recoil`) does `srl b / rr c` **twice** on `wCurDamage` = divide by 4 ("get 1/4
    damage or 1 HP, whichever is higher").
  - Verified `drain_hp` is really **1/2**: `SapHealth` (`effect_commands.asm:3837`, shared by
    `BattleCommand_DrainTarget` at `:3827` and `BattleCommand_EatDream` at `:3832`) does a single
    `srl a` = divide by 2.
- **Two bugs found** (both reported in §1): the **Fly/Dig** two-turn-invulnerable flavor is spelled
  two different (both wrong) ways between the two moves that share the identical `EFFECT_FLY`
  handler, and **Flame Wheel**/**Snore**/**Twister** are entirely missing their `effects` entries
  (see §3 — these are really effect-chance omissions, but they also mean the moves are
  under-flavored relative to their ROM behavior).
- Confirmed **Superpower is absent** from the 251-move list (correct — it doesn't exist until Gen
  4).

---

## 3. Effect-chance findings

Cross-checked every `chance` value in every move's `effects[]` array against the ROM's
`EFFECT_CHANCE` byte (nominal percent) in `moves.asm`, for all 251 moves. Summary in §1's table;
full detail on the named checklist items:

- **Tri Attack** (`rom_id=161`, `moves.asm:177`, chance byte `20`): script is
  `data/moves/effects.asm:1019-1037`, ending in `tristatuschance` (not the generic `effectchance`).
  `BattleCommand_TriStatusChance` (`effect_commands.asm:4730-4747`) first calls the generic
  `BattleCommand_EffectChance` (`effect_commands.asm:1851-1871`, success iff
  `BattleRandom < MOVE_CHANCE_byte`, i.e. `51/256`), then — **only if that succeeds** — rolls a
  second, independent, *exactly*-uniform 1-of-3 choice between Paralyze/Freeze/Burn via rejection
  sampling (`call BattleRandom / swap a / and %11 / jr z, .loop` — rerolls on 0, otherwise maps
  1/2/3 to Paralyze/Freeze/Burn with **exactly** 1/3 probability each, unbiased). So the true
  per-status probability is `(51/256) × (1/3) = 17/256 ≈ 6.6406%`, not the naively-expected
  `20%/3 ≈ 6.667%` and not the app's current `6.3`. The app's own `Note` field already flags this as
  an approximation — **recommend updating the stored value from `6.3` to `6.64`** (or keeping it
  symbolic) if more precision is wanted; this is a report/FYI per the brief, not necessarily a hard
  requirement.
- **Sacred Fire** (50% Burn): app matches exactly (`{"chance": 50, "status": "Burn"}` vs.
  `moves.asm:237` byte `50`, script `data/moves/effects.asm:1677-1689` `effectchance`+`burntarget`).
  Correct.
- **Zap Cannon** (100% Paralysis) / **DynamicPunch** (100% Confusion): both correct, byte `100` in
  `moves.asm:208` and `:239`, matching app `chance: 100`.
- **Metal Claw** (10% Atk+1, target **self**) / **Steel Wing** (10% Def+1, target **self**) /
  **Crunch** & **Shadow Ball** (20% SpDef-1, target enemy) / **Psychic** (10% SpDef-1, enemy) /
  **Acid** (10% Def-1, enemy) / **Aurora Beam** (10% Atk-1, enemy) / **Bubble/BubbleBeam/Constrict**
  (10% Spe-1, enemy) / **Icy Wind** (100% Spe-1, enemy) / **Rock Smash** (50% Def-1, enemy) /
  **Octazooka** (50% acc-1, enemy) / **Mud-Slap** (100% acc-1, enemy) / **Iron Tail** (30% Def-1,
  enemy) — all chance/stat/sign/target values verified correct against `moves.asm` and cross-checked
  programmatically (an automated pass verified stat + sign + magnitude + target for all 40 moves
  using `EFFECT_*_UP`/`_DOWN`/`_UP_2`/`_DOWN_2`/`_UP_HIT`/`_DOWN_HIT`; all 40 passed).
- **Charm** (-2 Atk, enemy) / **Cotton Spore** / **Scary Face** (-2 Spe, enemy) / **Amnesia**
  (+2 SpDef, self) / **Growth** (+1 SpAtk, self) all verified correct.
- **Swagger**: `effects.asm:1560-1576` shows `attackup2` (raises the **enemy's** Attack by 2 — app
  correctly has `target: "enemy"`) and `confusetarget` are both called unconditionally, *not* gated
  by `effectchance` — both components are guaranteed on hit, matching the app's `chance: 100` on
  both entries (only the status name typo noted in §1 needs fixing).
- **Belly Drum** (`modifier: 13`): ROM behavior, confirmed in
  `engine/battle/move_effects/belly_drum.asm:1-32`: costs 50% max HP and **unconditionally sets the
  user's Attack to its maximum stage (+6)**, regardless of its current stage (it does this by
  calling the ordinary "+2 Attack" command up to 6 times in a row, which simply clamps at the
  cap) — it does **not** add a relative `+13`. It fails outright (no HP cost) only if Attack is
  already at +6. `modifier: 13` therefore isn't a stage delta; tracing the Rust engine
  (`rust/crates/xpr-data/src/db.rs:578-630`, `get_stat_stage_info`/`get_stat_stage_dropdown_options`)
  shows it is effectively **dead data**: Belly Drum is special-cased entirely via the
  `is_belly_drum` flag (from the `belly_drum` flavor tag) and calls `set_attack_stage(count)`
  directly rather than reading `modifier`; the `13` only exists to satisfy the `has_stat_mod()`
  check (`stat` and `modifier` both non-null) so the entry registers as a stat effect at all. One
  thing worth a follow-up look (outside this JSON-verification task, flagging for awareness only):
  the Gen 2 dropdown in `db.rs:620-624` offers stage-count choices `"0"`, `"2"`, `"6"` — the `"2"`
  doesn't correspond to anything in the Gen 2 ROM behavior I verified (which always jumps straight
  to +6, never stops at +2); that may be confusing Gen 2 with the well-known Generation 1 Belly Drum
  bug (which really did only add +2).
- **Thief** (100% chance byte, `moves.asm:184`): script (`effects.asm:1430-1450`) confirms the item
  steal itself is gated by the same `effectchance` roll as any other secondary effect. The app's
  `effects` schema has no "steal item" `kind`, so this is left unrepresented there (only the
  `thief` flavor tag exists) — not a wrong value, just a schema gap; flagging for awareness.
- Full automated scan (every `chance` field in every move's `effects[]`, all 251 moves) found
  **exactly the four numeric mismatches listed in §1** (Tri Attack ×3 entries by the same root
  cause, Spikes, AncientPower) and no others — every other `chance` value (98 further entries) is
  byte-exact against the ROM's nominal percent.

---

## 4. Type chart / special types / held items

**(a) Set completeness.** Parsed all 110 effective triples out of `type_matchups.asm` (108 in the
main table, lines 3-110, plus the 2 Foresight-conditional Ghost-immunity entries at lines 115-116,
which are reached by the normal scan whenever Foresight has *not* been used and thus are part of
the effective chart). The app's `type_chart` contains **exactly** these 110 `(attacker, defender,
effectiveness)` triples — **0 missing, 0 extra**.

**(b) Row order (dual-type rounding).** For each of the 17 attacking types, compared the ROM's
in-file order of matchup entries against the JSON dict's key order, specifically checking every
pair where one entry is Super Effective and the other Not Very Effective (the only case where
order affects the truncating multiply-and-round used for dual-type defenders — a same-multiplier
pair, or any pair involving an Immune entry, commutes exactly regardless of order). **Zero
SE-vs-NVE order mismatches found** across all 17 types. Two cosmetic (order-irrelevant) differences
exist and are safe to ignore:
- `Normal`: ROM lists `Ghost:Immune` last (after the Foresight-block, `type_matchups.asm:115`); JSON
  lists it first. Immune's position doesn't affect rounding (0 either way).
- `Fighting`: same situation — `Ghost:Immune` (`type_matchups.asm:116`) is last in ROM, first in
  JSON.
- `Grass`: ROM order has `Dragon:NVE` before `Steel:NVE` (`type_matchups.asm:33-34`); JSON has
  `Steel` before `Dragon`. Both are Not-Very-Effective (same ×0.5 multiplier), so this swap is
  provably rounding-neutral.

**(c) `special_types`.** `constants/type_constants.asm:26-35` defines `SPECIAL EQU const_value`
(=20) followed by `FIRE, WATER, GRASS, ELECTRIC, PSYCHIC_TYPE, ICE, DRAGON, DARK`, then
`TYPES_END`. The app's `special_types` = `{Water, Grass, Fire, Ice, Electric, Psychic, Dragon,
Dark}` — the identical 8-element set (order differs but the field is a set/list membership check,
not used for row-order purposes). **Correct.**

**(d) `held_item_boosts`.** Cross-referenced `data/types/type_boost_items.asm:2-18` (17
`HELD_*_BOOST` constants → type) against `data/items/attributes.asm` (grepped for each
`HELD_*_BOOST` usage to find its owning item) and then against `raw_pkmn_data/gen_two/items.json`
for spelling:

| ROM item constant | Type | App key | In items.json? |
|---|---|---|---|
| PINK_BOW / POLKADOT_BOW | Normal | `Pink Bow` / `Polkadot Bow` | yes / yes |
| BLACKBELT_I | Fighting | `Black Belt` | yes |
| SHARP_BEAK | Flying | `Sharp Beak` | yes |
| POISON_BARB | Poison | `Poison Barb` | yes |
| SOFT_SAND | Ground | `Soft Sand` | yes |
| HARD_STONE | Rock | `Hard Stone` | yes |
| SILVERPOWDER | Bug | `SilverPowder` | yes |
| SPELL_TAG | Ghost | `Spell Tag` | yes |
| CHARCOAL | Fire | `Charcoal` | yes |
| MYSTIC_WATER | Water | `Mystic Water` | yes |
| MIRACLE_SEED | Grass | `Miracle Seed` | yes |
| MAGNET | Electric | `Magnet` | yes |
| TWISTEDSPOON | Psychic | `TwistedSpoon` | yes |
| NEVERMELTICE | Ice | `Nevermeltice` | yes |
| DRAGON_SCALE | Dragon | `Dragon Scale` | yes |
| BLACKGLASSES | Dark | `BlackGlasses` | yes |
| METAL_COAT | Steel | `Metal Coat` | yes |

All 18 entries (17 types, Normal doubled) match in type assignment, and **every key exists verbatim
in `items.json`** — **0 mismatches, 0 unknown keys**. (Note: `data/items/attributes.asm` differs
between pokegold/pokecrystal only for four unrelated key items — Clear Bell/GS Ball/Blue
Card/Egg Ticket vs. Gold's unimplemented `ITEM_46/73/74/81` placeholders — none of which are
type-boost hold items, so this has no bearing on `held_item_boosts`.)

---

## 5. Pokegold vs. pokecrystal diff

Byte-for-byte `diff` of every file named in the brief:

- **Identical**: `data/moves/moves.asm`, `data/moves/effects.asm`, `data/moves/effects_pointers.asm`,
  `data/moves/effects_priorities.asm`, `data/moves/critical_hit_moves.asm`,
  `data/types/type_matchups.asm`, `data/types/type_boost_items.asm`, `constants/move_effect_constants.asm`,
  `constants/type_constants.asm`, `data/battle/critical_hit_chances.asm`.
- **`constants/move_constants.asm`**: differs by one blank line only (cosmetic, no constant values
  changed).
- **`data/items/attributes.asm`**: differs only for 4 key items unrelated to move data (see §4d).
- **`engine/battle/effect_commands.asm`**: 596 lines of diff, all either (1) Crystal-only Battle
  Tower gating (`wInBattleTowerBattle` checks affecting obedience, the sleep-duration RNG mask
  `SLP_MASK` vs `%011`, and Lock-On/Mind-Reader bypass — none of which apply outside Battle Tower
  facility battles), (2) behavior-preserving refactors (`IsInArray`-based continuous-move list,
  `TruncateHL_BC` loop restructuring, `CheckUserIsCharging`/`_CheckBattleScene` extraction,
  `PrintText`→`BattleTextbox` renames), (3) added `docs/bugs_and_glitches.md` reference comments
  with no code change, or (4) new `BATTLETYPE_CELEBI`/`BATTLETYPE_SUICUNE` forced-switch-fail cases
  (Crystal-only legendary-beast/Celebi event battles). **No move base stats, PP, accuracy, or
  secondary-effect logic differs between the two games.**

---

## What was verified identical (no issues)

Of 251 moves × 4 scalar fields (base_power/type/accuracy/pp) = 1,004 data points, **987 matched
exactly** (the 17 flagged above are either the 2 real type bugs or 15 provably-inert placeholder
values for variable-power moves). Of the 106 individual `effects[]` entries present across 98
moves, **102 matched the ROM's status/flinch/stat/chance/target exactly**; the 4 that didn't are
the Spikes/AncientPower/Tri Attack findings above. An additional 3 moves (Flame Wheel, Snore,
Twister) are missing an `effects` entry their ROM script clearly has (secondary chance-based
burn/flinch), for a total of 7 distinct effect-related fixes. All 7 `high_crit` moves, all
`multi_hit`/`two_hit`/`one_hit_ko`/`fixed_damage`/`level_damage`/`trap`/`quarter_recoil`/`drain_hp`/
`recharge`/`reversal` flavor assignments (32 moves across these 10 categories) were exactly right,
with only the Fly/Dig spelling-inconsistency bug found. The type chart is a perfect 110/110 set
match with zero rounding-relevant row-order defects; `special_types` and all 18
`held_item_boosts` entries are fully correct and every item name exists in `items.json`. Gold and
Crystal are functionally identical for every move-data file in scope, differing only in
Battle-Tower-only gating and cosmetic refactors in `effect_commands.asm`.
