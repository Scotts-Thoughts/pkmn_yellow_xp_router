# Gen 4 Move/Type Data Audit — app JSON vs pokeplatinum/pokediamond/pokeheartgold decompilations

Scope: every one of the 467 Gen 4 moves in `raw_pkmn_data/gen_four/moves.json` (core fields, `attack_flavor`, `effects[]`), the type chart and held-item boosts in `type_info.json`, and the Fling / Natural Gift / plate tables in `rust/crates/xpr-data/src/gen_consts.rs`, verified against pokeplatinum (primary), pokediamond and pokeheartgold. No repository file was modified. Findings are organised as the brief requested: (1) master mismatch tables (§1 core fields, §1b attack_flavor, §1c effects), (2) flavor coverage, (3) effect-chance mechanics, (4) type chart/items, (5) version differences, (6) what matched.

**Headline, by severity.** Real bugs worth fixing: Memento `accuracy: null` (ROM 100 — the move can miss); 22 moves whose `effects[]` is empty despite a ROM-confirmed secondary status/flinch/stat effect (Fire/Ice/Thunder Fang, Flame Wheel, Discharge, Lava Plume, Poison Jab, Cross Poison, Gunk Shot, Rock Climb, Force Palm, Volt Tackle, Flare Blitz, Waterfall, Dark Pulse, Air Slash, Dragon Rush, Zen Headbutt, Iron Head, Charge, Stockpile, Captivate) plus Fake Out's missing guaranteed flinch; Acid's stat key (`def` → `spd`); Tri Attack's per-status chance (`6.3` → `6.67`); 12 missing `attack_flavor` tags (Aura Sphere/Magnet Bomb `always_hit`, Magma Storm `trap`, Drain Punch `drain_hp`, Giga Impact/Rock Wrecker/Roar of Time `recharge`, Shadow Force `two_turn_semi_invulnerable`, Surf/Whirlpool a dive-bonus tag that doesn't exist yet, Grass Knot `weight_damage`, Avalanche `revenge`) and 3 spurious ones (Needle Arm/Astonish/Extrasensory `bonus_minimize_damage`); Cover Fossil and Plume Fossil in `GEN4_FLING_POWER` (Gen 5 items). Cosmetic/inert: Belly Drum `modifier: 13`, Curse `type: "Ghost"` vs `???`. Not bugs (explained below): the six `effect_chance: null` self-target cases, the three type-chart row-order differences, the 151 consumables absent from the Fling table.

## Methodology

- Parsed `raw_pkmn_data/gen_four/moves.json` (467 moves, keyed by name, each with an explicit `rom_id` 1-467) with Python.
- Parsed pokeplatinum's per-move `res/moves/<slug>/data.json` (built from `generated/moves.txt`, mapping ROM id -> `MOVE_<NAME>` -> directory slug; verified 100% bijective, all 467 real moves + `MOVE_NONE` accounted for, no orphans in either direction).
- Parsed pokediamond's `files/poketool/waza/waza_tbl.json` (a flat array of 471 move records, array index = ROM id; fields are named directly: `class`, `power`, `type`, `accuracy`, `pp`, `effectChance`, `priority`, plus three not-fully-reverse-engineered fields `unk8`/`unkB`/`unkC` per `include/move_data.h`'s `struct WazaTbl`).
- Parsed pokeheartgold's `files/poketool/waza/waza_tbl.narc` directly as a binary NARC archive (own NARC-container parser: BTAF file-allocation table + GMIF data section), decoding each of the 471 16-byte records with the exact same `struct WazaTbl` layout documented in pokediamond's `include/move_data.h`:
  `<H effect, B class, B power, B type, B accuracy, B pp, B effectChance, H unk8, b priority, B unkB, B unkC, B contestType, 2 pad bytes>`. Validated by cross-checking `unkB`/`unkC` (opaque contest-appeal bytes) against pokediamond's values for several moves — exact match every time, confirming correct field alignment.
- Confirmed `app.rom_id` = ROM's own enum value = pokediamond/pokeheartgold array index = position (0-based) in pokeplatinum's `generated/moves.txt`, i.e. a single linking key ties all four sources together for all 467 moves.
- Derived and empirically validated these app-schema <-> ROM-schema conversions (see §1 for the validation basis):
  - `category`: `CLASS_PHYSICAL/SPECIAL/STATUS` -> `Physical/Special/Status`.
  - `type`: `TYPE_X` -> `X` title-cased, except `TYPE_MYSTERY` (no clean app equivalent — see finding below).
  - `power`/`accuracy`: ROM `0` -> app `null`, else pass through unchanged.
  - `priority`: app stores the raw **unsigned byte** representation of the ROM's signed priority (e.g. ROM `-5` -> app `251`, ROM `-1` -> app `255`), not the signed value. Confirmed against Counter(-5->251), Focus Punch(-3->253), Whirlwind/Roar(-6->250), Mirror Coat(-5->251), Vital Throw(-1->255), Quick Attack(+1->1).
  - `effect_chance`: ROM `0` -> app `null`, else pass through — **except** a confirmed, 100%-consistent app convention: a damaging move whose ROM `effect_chance` is `100` because it carries a *guaranteed* secondary effect **on the user's own stats** is still recorded as app `effect_chance: null` (the per-effect chance is instead carried inside that move's `effects[]` entries, each correctly at `"chance": 100`). The same guaranteed-100 case targeting the **enemy** instead surfaces as a literal top-level `100`. Validated on all 15 moves in the ROM with a guaranteed (chance=100) stat-change secondary effect — zero exceptions to the self->null / enemy->literal-100 rule except the one bug noted below (Fake Out).
  - `target`: pokeplatinum's `range` enum -> app `target` string is a clean bijection (see table in §1); validated with zero exceptions across all 467 moves.
- Cross-referenced the app's `effect` field (one string id per move, e.g. `"curse"`) against pokeplatinum's `BATTLE_EFFECT_*` id per move: **confirmed a clean 1:1 function, zero cross-assignment errors** across all 467 moves — i.e. the app never has the wrong "kind" of move tagged to the wrong game mechanic. (Where one app string groups multiple ROM ids, e.g. `always_hit` covering both `BATTLE_EFFECT_BYPASS_ACCURACY` and its priority-modified sibling, or `semi_invulnerable_turn` covering Dig/Fly/Dive/Bounce, this was checked and is semantically correct grouping, not an error.)
- Also checked (not explicitly requested by the audit's item 1, but free to verify given the data was on hand): the six boolean flags `makes_contact`, `affected_by_protect`, `affected_by_magic_coat`, `affected_by_snatch`, `affected_by_mirror_move`, `affected_by_kings_rock` against pokeplatinum's `flags` array (`MOVE_FLAG_MAKES_CONTACT`, `MOVE_FLAG_CAN_PROTECT`, `MOVE_FLAG_CAN_MAGIC_COAT`, `MOVE_FLAG_CAN_SNATCH`, `MOVE_FLAG_CAN_MIRROR_MOVE`, `MOVE_FLAG_TRIGGERS_KINGS_ROCK`). **Zero mismatches** across all 467 moves x 6 flags (2,802 checks).

## (1) Master mismatch table — core fields (app vs. Platinum ROM)

All 467 moves were checked on `category`, `type`, `power`, `accuracy`, `pp`, `priority`, `effect_chance`, and `target`. Only the following 8 anomalies were found; everything else (459 moves with zero anomalies, and these two fields on all 467) matched exactly.

| Move (rom_id) | Field | App value | ROM value (Platinum) | Citation | Assessment / suggested fix |
|---|---|---|---|---|---|
| Skull Bash (130) | effect_chance | `null` | `100` | `res/moves/skull_bash/data.json:15-17` `"effect": {"type": "BATTLE_EFFECT_CHARGE_TURN_DEF_UP", "chance": 100}` | **Not a bug.** Guaranteed self-target effect; app's established convention nulls the top-level field and records `"chance":100` inside `effects[]` instead (confirmed present: `{"chance":100,"modifier":1,"stat":"def","target":"self"}`). No fix needed. |
| Fake Out (252) | effect_chance, effects[] | `effect_chance: null`, `effects: []` (empty) | `"effect": {"type":"BATTLE_EFFECT_ALWAYS_FLINCH_FIRST_TURN_ONLY","chance":100}` | `res/moves/fake_out/data.json:15-17` | **Real bug.** This is the one guaranteed effect in the entire table that targets the **enemy** (flinch) yet was not given the literal `effect_chance: 100` + `effects` entry that the app's own convention uses for every other enemy-targeted guaranteed effect (Zap Cannon, DynamicPunch, Icy Wind, Rock Tomb, Mud Shot, Mud-Slap — all literal `100`). Fix: set `effect_chance: 100` and add `{"chance":100,"flinch":true}` to `effects`. |
| Memento (262) | accuracy | `null` | `100` | `res/moves/memento/data.json:11-13` `"accuracy": 100` | **Real bug.** `null` in this schema means "always hits, no accuracy roll" (used correctly for ROM-accuracy-0 self-target moves), but Memento's ROM accuracy is a real, rollable `100`, not `0` — it targets the foe and can miss (e.g. vs. raised evasion). It is the *only* move in the table where app `accuracy=null` but ROM accuracy != 0. Fix: set `accuracy: 100`. |
| Overheat (315) | effect_chance | `null` | `100` | `res/moves/overheat/data.json:15-17` `"effect": {"type":"BATTLE_EFFECT_USER_SP_ATK_DOWN_2","chance":100}` | Not a bug — self-target convention (see Skull Bash row). `effects` correctly has `{"chance":100,"modifier":-2,"stat":"spa","target":"self"}`. |
| Psycho Boost (354) | effect_chance | `null` | `100` | `res/moves/psycho_boost/data.json:15-17` | Not a bug — same self-target convention; `effects` correctly populated. |
| Draco Meteor (434) | effect_chance | `null` | `100` | `res/moves/draco_meteor/data.json:15-17` | Not a bug — same self-target convention; `effects` correctly populated. |
| Leaf Storm (437) | effect_chance | `null` | `100` | `res/moves/leaf_storm/data.json:15-17` | Not a bug — same self-target convention; `effects` correctly populated. |
| Curse (174) | type | `"Ghost"` | `TYPE_MYSTERY` (i.e. `???`) | `res/moves/curse/data.json:10` `"type": "TYPE_MYSTERY"` | **Real data mismatch, low practical impact.** Curse is the *only* `???`-type move in all of Gen 4 (confirmed: grepped all 467 Platinum `type` fields, exactly one `TYPE_MYSTERY` hit). Since Curse has `power: null` (never enters damage calc), STAB/type-effectiveness on it are moot in Gen 4 (no type-based abilities like Protean exist yet). The app's own `xpr-core/src/consts.rs:589` already defines `pub const TYPE_TYPELESS: &str = "none";` for exactly this kind of case, but it is currently unused anywhere in the Rust codebase and `raw_pkmn_data/gen_four/type_info.json`'s `type_chart` has no `"none"` row, so switching Curse to `"none"` outright would need that supporting plumbing added first. Flagging as a data-fidelity issue for the maintainer to decide; not urgent given zero battle-mechanical impact. |

Also checked and **zero mismatches**: `pp` (467/467), `priority` (467/467, after the unsigned-byte conversion above), the `target` field (467/467 — see range/target bijection table below), and the six `makes_contact`/`affected_by_*` booleans (2,802/2,802 checks).

### (1b) Master mismatch table — `attack_flavor` (see §2 for per-category coverage)

All 15 rows were re-confirmed against the live `raw_pkmn_data/gen_four/moves.json` and the Platinum `effect` id, not just the merged scratch data. Effect-script citations are `pokeplatinum/res/battle/scripts/effects/effect_script_NNNN.s` where NNNN is the effect's numeric id (0-based line position in `generated/move_battle_effects.txt`).

| Move (rom_id) | Field | App value | ROM value | Citation | Suggested json fix |
|---|---|---|---|---|---|
| Aura Sphere (396) | attack_flavor | `[]` | bypasses accuracy: `BATTLE_EFFECT_BYPASS_ACCURACY`, accuracy 0 — the exact effect id of Swift/Shock Wave/Aerial Ace/Faint Attack/Shadow Punch/Magical Leaf (all correctly tagged) | `res/moves/aura_sphere/data.json`; handler `effect_script_0017.s` is just `CalcCrit; CalcDamage; End` (no accuracy roll) | add `"always_hit"` |
| Magnet Bomb (443) | attack_flavor | `[]` | `BATTLE_EFFECT_BYPASS_ACCURACY`, accuracy 0 | `res/moves/magnet_bomb/data.json` | add `"always_hit"` |
| Magma Storm (463) | attack_flavor | `[]` | `BATTLE_EFFECT_BIND_HIT` — same id as Bind/Wrap/Fire Spin/Clamp/Sand Tomb (all tagged `trap`) | `res/moves/magma_storm/data.json`; `res/moves/magma_storm/script.s` prints "became trapped by swirling magma"; shared handler `effect_script_0042.s` sets `MOVE_SUBSCRIPT_PTR_BIND_TARGET` | add `"trap"` |
| Drain Punch (409) | attack_flavor | `[]` | `BATTLE_EFFECT_RECOVER_HALF_DAMAGE_DEALT` — same id as Absorb/Mega Drain/Leech Life/Giga Drain (all tagged `drain_hp`) | `res/moves/drain_punch/data.json` | add `"drain_hp"` |
| Giga Impact (416) | attack_flavor | `[]` | `BATTLE_EFFECT_RECHARGE_AFTER` — same id as Hyper Beam/Blast Burn/Hydro Cannon/Frenzy Plant (all tagged `recharge`) | `res/moves/giga_impact/data.json` | add `"recharge"` |
| Rock Wrecker (439) | attack_flavor | `[]` | `BATTLE_EFFECT_RECHARGE_AFTER` | `res/moves/rock_wrecker/data.json` | add `"recharge"` |
| Roar of Time (459) | attack_flavor | `[]` | `BATTLE_EFFECT_RECHARGE_AFTER` | `res/moves/roar_of_time/data.json` | add `"recharge"` |
| Grass Knot (446) | attack_flavor | `[]` | `BATTLE_EFFECT_INCREASE_POWER_WITH_WEIGHT` (effect id 196) — same id as Low Kick (correctly tagged `weight_damage`) | `res/moves/grass_knot/data.json`; shared handler `effect_script_0196.s` = `CalcWeightBasedPower; CalcCrit; CalcDamage; End` | add `"weight_damage"` |
| Avalanche (419) | attack_flavor | `[]` | `BATTLE_EFFECT_DOUBLE_POWER_IF_HIT` (effect id 185) — same id as Revenge (correctly tagged `revenge`) | `res/moves/avalanche/data.json`; shared handler `effect_script_0185.s` = `CalcRevengePowerMul; CalcCrit; CalcDamage; End` | add `"revenge"` |
| Shadow Force (467) | attack_flavor | `[]` | Two-turn semi-invulnerable charge move: `effect_script_0272.s` (`BATTLE_EFFECT_SHADOW_FORCE`) is structurally identical to Fly's `effect_script_0155.s` — sets `MOVE_SUBSCRIPT_PTR_VANISH_CHARGE_TURN`, `SYSCTL_FIRST_OF_MULTI_TURN`, `ToggleVanish BTLSCR_ATTACKER, TRUE`, then `OPCODE_FLAG_OFF … MOVE_EFFECT_SEMI_INVULNERABLE` after the strike (verified by reading both scripts side by side). The app's own `description` ("The user disappears, then strikes the foe on the second turn…") and `affected_by_protect: false` already reflect this. | `res/battle/scripts/effects/effect_script_0272.s` vs `effect_script_0155.s` | add `"two_turn_semi_invulnerable"` (the audit brief's "only Fly/Dig/Dive/Bounce" list omitted Shadow Force; the ROM is unambiguous) |
| Surf (57) | attack_flavor | `[]` | Doubles power vs a target that is underwater (using Dive): `BATTLE_EFFECT_DOUBLE_DAMAGE_DIVE`, `effect_script_0257.s` sets `SYSCTL_HIT_DURING_DIVE` and `BTLVAR_POWER_MULTI` 10→20 when the defender has `MOVE_EFFECT_UNDERWATER` | `res/moves/surf/data.json`; `effect_script_0257.s` | add a dive-bonus tag — **no such tag exists anywhere in the app's current 467-move flavor vocabulary** (there is `bonus_dig_damage` and `bonus_fly_damage` but no `bonus_dive_damage`), so this needs a new tag plus engine support, not just a data edit |
| Whirlpool (250) | attack_flavor | `["trap"]` | `trap` is correct, but `BATTLE_EFFECT_WHIRLPOOL` (`effect_script_0261.s`) *also* carries the identical `SYSCTL_HIT_DURING_DIVE` / power-double-vs-`MOVE_EFFECT_UNDERWATER` logic as Surf | `res/moves/whirlpool/data.json`; `effect_script_0261.s` | add the same dive-bonus tag as Surf |
| Needle Arm (302) | attack_flavor | `["bonus_minimize_damage"]` | plain `BATTLE_EFFECT_FLINCH_HIT` (`effect_script_0031.s`: `MOVE_SUBSCRIPT_PTR_FLINCH` + `CalcCrit/CalcDamage`, nothing else) — the same id as 12 other flinch moves (Rolling Kick, Headbutt, Bite, Bone Club, Waterfall, Rock Slide, Hyper Fang, Dark Pulse, Air Slash, Dragon Rush, Zen Headbutt, Iron Head) that correctly have no minimize bonus. Only Stomp has the distinct `BATTLE_EFFECT_FLINCH_MINIMIZE_DOUBLE_HIT` (`effect_script_0150.s`), which is the sole place in the Platinum battle scripts that checks `MOVE_EFFECT_MINIMIZE` to double power. | `res/moves/needle_arm/data.json`; `effect_script_0031.s` vs `effect_script_0150.s` | remove `"bonus_minimize_damage"` (Gen 6+ mechanic for these three moves) |
| Astonish (310) | attack_flavor | `["bonus_minimize_damage"]` | plain `BATTLE_EFFECT_FLINCH_HIT` | `res/moves/astonish/data.json` | remove `"bonus_minimize_damage"` |
| Extrasensory (326) | attack_flavor | `["bonus_minimize_damage"]` | plain `BATTLE_EFFECT_FLINCH_HIT` | `res/moves/extrasensory/data.json` | remove `"bonus_minimize_damage"` |

### `range` -> `target` mapping (pokeplatinum `RANGE_*` enum -> app `target` string), empirically derived and verified with zero exceptions across all 467 moves

| ROM `range` | App `target` | Moves | Example |
|---|---|---|---|
| `RANGE_SINGLE_TARGET` | `Foe Or Ally` | 334 | Gastro Acid, Thunder |
| `RANGE_SINGLE_TARGET_SPECIAL` | `Other` | 11 | Metal Burst, Mirror Coat, Metronome |
| `RANGE_RANDOM_OPPONENT` | `Random` | 4 | Petal Dance, Thrash, Outrage, Uproar |
| `RANGE_ADJACENT_OPPONENTS` | `All Foes` | 24 | Icy Wind, Rock Slide |
| `RANGE_ALL_ADJACENT` | `Others` | 8 | Surf, Discharge, Explosion |
| `RANGE_USER` | `Self` | 62 | Splash, Baton Pass |
| `RANGE_USER_SIDE` | `Self And Ally` | 8 | Light Screen, Reflect, Tailwind |
| `RANGE_FIELD` | `All` | 10 | Trick Room, Gravity, Hail |
| `RANGE_OPPONENT_SIDE` | `Foe Side` | 3 | Stealth Rock, Spikes, Toxic Spikes |
| `RANGE_ALLY` | `Ally` | 1 | Helping Hand |
| `RANGE_USER_OR_ALLY` | `Self Or Ally` | 1 | Acupressure |
| `RANGE_SINGLE_TARGET_ME_FIRST` | `Any Foe` | 1 | Me First |
| `RANGE_ALL` | *(unused — no move in the table uses this range id)* | 0 | — |

### (1c) Master mismatch table — `effects[]` (see §3 for mechanics and per-move coverage)

Citations are relative to pokeplatinum: `res/moves/<slug>/data.json` for the effect id + raw chance byte, `res/battle/scripts/effects/effect_script_NNNN.s` for the dispatch script, `res/battle/scripts/subscripts/subscript_*.s` for the stat/status application. Every row was re-confirmed against the live `moves.json`; the 20 "empty `effects[]`" rows with a nonzero ROM chance are the complete, exhaustive result of a sweep of all 467 moves for that pattern (not a sample).

| Move (rom_id) | Field | App value | ROM value | Citation | Suggested json fix |
|---|---|---|---|---|---|
| Tri Attack (161) | effects[].chance | `6.3` × 3 (Burn/Freeze/Paralysis) | 20/3 = **6.667%** each | `effect_script_0036.s`: `Random 2, 3` (→ `BtlCmd_Random`, `battle_script.c:3200`, uniform 3/4/5 = `MOVE_SUBSCRIPT_PTR_BURN/FREEZE/PARALYZE`) then one `rand()%100 < 20` gate in `BattleSystem_TriggerSecondaryEffect` (`battle_lib.c:1570-1586`) | set each to `6.67` |
| Acid (51) | effects[].stat | `"def"` | Sp. Def: `BATTLE_EFFECT_LOWER_SP_DEF_HIT` (10%) → `effect_script_0072.s` = `MOVE_SUBSCRIPT_PTR_SP_DEFENSE_DOWN_1_STAGE`; ROM description "lower the target's Sp. Def"; app's own `effect` string is `may_lower_enemy_special_defense_1` | `res/moves/acid/data.json`; `effect_script_0072.s` | change `"stat": "def"` → `"spd"` |
| Belly Drum (187) | effects[].modifier | `13` | Sets `BATTLEMON_ATTACK_STAGE` to raw `12` (= +6 on the relative scale, "maximize") | `subscript_belly_drum.s`: `UpdateMonData OPCODE_SET, BTLSCR_ATTACKER, BATTLEMON_ATTACK_STAGE, 12`. `13` matches neither the raw (12) nor relative (+6) value and is the only `\|modifier\| > 2` in the dataset. Currently inert: `xpr-data/src/db.rs` (~line 620, `get_stat_stage_dropdown_options`) special-cases the `belly_drum` flavor to `[0, 6]` and ignores `.modifier`. | set to `6` (relative, matching the rest of the schema) |
| Fire Fang (424) | effects[] | `[]` | **two independent 10% rolls** — burn and flinch | `data.json` `BATTLE_EFFECT_FLINCH_BURN_HIT` chance 10; `subscript_burn_or_flinch.s` calls `CheckEffectActivation` twice (→ `BtlCmd_CheckEffectActivation`, `battle_script.c:8363`, fresh `rand()%100 < effectChance` each call) | `[{"status":"Burn","chance":10},{"flinch":true,"chance":10}]` |
| Ice Fang (423) | effects[] | `[]` | two independent 10% rolls — freeze and flinch | `FLINCH_FREEZE_HIT` chance 10; `subscript_freeze_or_flinch.s` | `[{"status":"Freeze","chance":10},{"flinch":true,"chance":10}]` |
| Thunder Fang (422) | effects[] | `[]` | two independent 10% rolls — paralysis and flinch | `FLINCH_PARALYZE_HIT` chance 10; `subscript_paralyze_or_flinch.s` | `[{"status":"Paralysis","chance":10},{"flinch":true,"chance":10}]` |
| Flame Wheel (172) | effects[] | `[]` (has flavor `self_thaw`; top-level `effect_chance: 10` is correct) | 10% burn | `THAW_AND_BURN_HIT` chance 10, `effect_script_0125.s` = `TO_DEFENDER \| MOVE_SUBSCRIPT_PTR_BURN` — the identical script Sacred Fire uses, and Sacred Fire correctly has `[{"chance":50,"status":"Burn"}]` | `[{"status":"Burn","chance":10}]` |
| Discharge (435) | effects[] | `[]` | 30% paralysis | `PARALYZE_HIT` chance 30; `effect_script_0006.s` | `[{"status":"Paralysis","chance":30}]` |
| Lava Plume (436) | effects[] | `[]` | 30% burn | `BURN_HIT` chance 30; `effect_script_0004.s` | `[{"status":"Burn","chance":30}]` |
| Poison Jab (398) | effects[] | `[]` | 30% poison | `POISON_HIT` chance 30; `effect_script_0002.s` — same id as Sludge Bomb, which correctly has the entry | `[{"status":"Poison","chance":30}]` |
| Cross Poison (440) | effects[] | `[]` (flavor `high_crit` is correct) | 10% poison | `HIGH_CRITICAL_POISON_HIT` chance 10; `effect_script_0209.s` (crit flag + `TO_DEFENDER \| MOVE_SUBSCRIPT_PTR_POISON`) | `[{"status":"Poison","chance":10}]` |
| Gunk Shot (441) | effects[] | `[]` | 30% poison | `POISON_HIT` chance 30 | `[{"status":"Poison","chance":30}]` |
| Rock Climb (431) | effects[] | `[]` | 20% confusion | `CONFUSE_HIT` chance 20; `effect_script_0076.s` — same id as DynamicPunch, which correctly has its entry | `[{"status":"Confusion","chance":20}]` |
| Force Palm (395) | effects[] | `[]` | 30% paralysis | `PARALYZE_HIT` chance 30 | `[{"status":"Paralysis","chance":30}]` |
| Volt Tackle (344) | effects[] | `[]` (flavor `third_recoil` is correct) | 10% paralysis | `RECOIL_PARALYZE_HIT` chance 10; `effect_script_0262.s` (`MOVE_SIDE_EFFECT_PROBABILISTIC` paralysis roll) | `[{"status":"Paralysis","chance":10}]` |
| Flare Blitz (394) | effects[] | `[]` (flavor `third_recoil` is correct) | 10% burn | `RECOIL_BURN_HIT` chance 10; `effect_script_0253.s` | `[{"status":"Burn","chance":10}]` |
| Waterfall (127) | effects[] | `[]` | 20% flinch | `FLINCH_HIT` chance 20; `effect_script_0031.s` | `[{"flinch":true,"chance":20}]` |
| Dark Pulse (399) | effects[] | `[]` | 20% flinch | `FLINCH_HIT` chance 20 | `[{"flinch":true,"chance":20}]` |
| Air Slash (403) | effects[] | `[]` | 30% flinch | `FLINCH_HIT` chance 30 | `[{"flinch":true,"chance":30}]` |
| Dragon Rush (407) | effects[] | `[]` | 20% flinch | `FLINCH_HIT` chance 20 | `[{"flinch":true,"chance":20}]` |
| Zen Headbutt (428) | effects[] | `[]` | 20% flinch | `FLINCH_HIT` chance 20 | `[{"flinch":true,"chance":20}]` |
| Iron Head (442) | effects[] | `[]` | 30% flinch | `FLINCH_HIT` chance 30 | `[{"flinch":true,"chance":30}]` |
| Charge (268) | effects[] | `[]` | guaranteed SpDef +1 self (ROM chance byte is 0 because it's applied unconditionally by a dedicated subscript, same as Curse) | `BATTLE_EFFECT_SP_DEF_UP_DOUBLE_ELECTRIC_POWER`; `effect_script_0174.s` → `subscript_charge.s` sets `MOVE_SUBSCRIPT_PTR_SP_DEFENSE_UP_1_STAGE` on the attacker; ROM description "also raises the user's Sp. Def". Not special-cased anywhere in the Rust app (`grep -rn '"charge"' rust/crates` → no hits), so the boost is currently unmodelled. | `[{"chance":100,"modifier":1,"stat":"spd","target":"self"}]` |
| Stockpile (254) | effects[] | `[]` | guaranteed Def +1 and SpDef +1 self per use (3-use cap tracked via `BATTLEMON_STOCKPILE_COUNT`) | `subscript_stockpile.s` applies `DEFENSE_UP_1_STAGE` then `SP_DEFENSE_UP_1_STAGE` unconditionally. `grep -rn stockpile rust/crates` → no hits: no in-app stat effect today. | `[{"chance":100,"modifier":1,"stat":"def","target":"self"},{"chance":100,"modifier":1,"stat":"spd","target":"self"}]` |
| Captivate (445) | effects[] | `[]` | guaranteed SpAtk −2 on target (gated only by opposite-gender / Oblivious, not a probability) | `BATTLE_EFFECT_SP_ATK_DOWN_2_OPPOSITE_GENDER`; `effect_script_0265.s` → `MOVE_SUBSCRIPT_PTR_SP_ATTACK_DOWN_2_STAGES` | `[{"chance":100,"modifier":-2,"stat":"spa","target":"enemy"}]` |

## (2) attack_flavor omissions/spurious flags

Method: for each flavor category, the ROM-truth move set was derived from `BATTLE_EFFECT_*` membership (`effect_to_moves.json`), each effect id resolved to its numeric value via `generated/move_battle_effects.txt`, and the corresponding `res/battle/scripts/effects/effect_script_NNNN.s` (plus `res/moves/<slug>/script.s` / `data.json` where useful) read to confirm actual behaviour rather than trusting the id's name. That set was then diffed both ways (missing and spurious) against every one of the 467 moves' `attack_flavor` lists — exhaustive set differences, not sampling. 25 categories in practice (several brief bullets split into distinct ROM effects; `weight_damage` and `revenge` added on review — see below). **105 ROM-implied (move, tag) assignments over 102 distinct moves; 93 correct; 15 anomalies on 15 moves (12 missing tags, 3 spurious).** The 15 rows are itemised in table §1b above.

| Category | Correct / expected | Notes |
|---|---|---|
| `high_crit` | 19/19 | All five crit-raising effect ids covered (plain `HIGH_CRITICAL` ×14; Razor Wind's charge-turn variant; Sky Attack's charge-turn+flinch variant; Blaze Kick's burn variant; Poison Tail/Cross Poison's poison variant). Focus Energy (`BATTLE_EFFECT_CRIT_UP_2`, a status buff) correctly does *not* carry `high_crit`. |
| `multi_hit` | 12/12 | `BATTLE_EFFECT_MULTI_HIT` (2-5 hits). |
| `two_hit` | 4/4 | Double Kick/Bonemerang/Double Hit share `BATTLE_EFFECT_HIT_TWICE`; Twineedle is the separate `BATTLE_EFFECT_POISON_MULTI_HIT` and is correctly `two_hit`, not `multi_hit`. |
| `always_hit` | 7/9 | **Missing: Aura Sphere, Magnet Bomb** (both `BATTLE_EFFECT_BYPASS_ACCURACY`). |
| `one_hit_ko` | 4/4 | Guillotine, Horn Drill, Fissure, Sheer Cold. |
| `fixed_damage` | 2/2 | Sonic Boom (`20_DAMAGE_FLAT`), Dragon Rage (`40_DAMAGE_FLAT`). |
| `level_damage` | 2/2 | Seismic Toss, Night Shade. |
| `psywave` | 1/1 | Note the ROM id naming trap: Magnitude's effect is confusingly named `BATTLE_EFFECT_PSYWAVE` but its script (`effect_script_0126.s`, `CalcMagnitudePower`) implements Magnitude; the real Psywave (149) uses `BATTLE_EFFECT_RANDOM_DAMAGE_1_TO_150_LEVEL`. The app tags both correctly. |
| `super_fang` | 1/1 | |
| `weight_damage` | 1/2 | **Missing: Grass Knot** (`BATTLE_EFFECT_INCREASE_POWER_WITH_WEIGHT`, shared with Low Kick, which is correctly tagged). |
| `revenge` | 1/2 | **Missing: Avalanche** (`BATTLE_EFFECT_DOUBLE_POWER_IF_HIT`, shared with Revenge, which is correctly tagged). |
| `trap` | 6/7 | **Missing: Magma Storm** (`BATTLE_EFFECT_BIND_HIT`). Whirlpool (`BATTLE_EFFECT_WHIRLPOOL`) correctly tagged. |
| `quarter_recoil` | 2/2 | Take Down, Submission (`BATTLE_EFFECT_RECOIL_QUARTER`). |
| `third_recoil` | 5/5 | Double-Edge, Brave Bird, Wood Hammer (`RECOIL_THIRD`) plus Volt Tackle and Flare Blitz (their own ids, same 1/3 recoil). |
| `half_recoil` | 1/1 | Head Smash. |
| crash-on-miss (Jump Kick / Hi Jump Kick) | 2/2 | App tag is `miss_recoil_damage`, applied to both `BATTLE_EFFECT_CRASH_ON_MISS` moves. |
| `quarter_max_hp_recoil` (Struggle) | 1/1 | Confirmed Gen-4-specific: `subscript_struggle.s` divides `BATTLEMON_MAX_HP` by 4 (max-HP-based, not damage-based), so it is correctly distinguished from `quarter_recoil`. |
| `drain_hp` | 5/6 | **Missing: Drain Punch** (`BATTLE_EFFECT_RECOVER_HALF_DAMAGE_DEALT`). |
| `recharge` | 4/7 | **Missing: Giga Impact, Rock Wrecker, Roar of Time** (all `BATTLE_EFFECT_RECHARGE_AFTER`). |
| `two_turn` | 4/4 | Razor Wind, Sky Attack, Solar Beam, Skull Bash — none carry the semi-invulnerable tag, correct. |
| `two_turn_semi_invulnerable` | 4/5 | Fly, Dig, Dive, Bounce correct. **Missing: Shadow Force** (`effect_script_0272.s` is the same vanish/`MOVE_EFFECT_SEMI_INVULNERABLE` pattern as Fly). |
| `bonus_dig_damage` | 2/2 | Earthquake, Magnitude. |
| `bonus_fly_damage` | 2/2 | Gust, Twister. Thunder and Sky Uppercut correctly use the separate no-power-bonus tags `hits_fly` / `hits_air_semi_invulnerable` rather than `bonus_fly_damage`. |
| bonus vs Dive (Surf, Whirlpool) | 0/2 | **Missing on both**, and no dive-bonus tag exists in the app's vocabulary at all (`bonus_dig_damage`/`bonus_fly_damage` exist; nothing for Dive). `effect_script_0257.s` (Surf) and `_0261.s` (Whirlpool) both double `BTLVAR_POWER_MULTI` against `MOVE_EFFECT_UNDERWATER`. |
| `bonus_minimize_damage` | 1/1 (+3 spurious) | Stomp correct (`FLINCH_MINIMIZE_DOUBLE_HIT`). **Spurious: Needle Arm, Astonish, Extrasensory** — plain `FLINCH_HIT`, no minimize check anywhere in their script or in `battle_lib.c`/`battle_script.c`; the brief's expectation ("NO minimize bonus in gen 4") is confirmed by the ROM, and the app data currently contradicts it. |

## (3) effect-chance findings

Method: the 96 moves named in the brief were checked one by one against `res/moves/<slug>/data.json` (effect id + raw chance byte), the dispatch script `effect_script_NNNN.s`, the applying `subscript_*.s`, and the C interpreter that gates them (`BtlCmd_Random` `battle_script.c:3200`, `BtlCmd_CheckEffectActivation` `battle_script.c:8363`, `BattleSystem_TriggerSecondaryEffect` `battle_lib.c:1519`). A separate sweep of **all 467** moves for "ROM `effect_chance > 0` but app `effects[]` empty" was then run to make the empty-array finding exhaustive. **96 named moves: 80 fully correct, 16 anomalies; the full sweep added 9 more; 25 effects anomalies total** (all itemised in table §1c). Mechanics confirmed along the way:

- **Tri Attack**: one 20% "does anything happen" gate, then a uniform 1-in-3 pick (`Random 2, 3` → subscript ptr 3/4/5 = burn/freeze/paralyze). Per-status probability is exactly 20/3 = 6.667%; the app's `6.3` is wrong. (Not three independent rolls — same number, different mechanism.)
- **Fire/Ice/Thunder Fang**: genuinely **two independent** ~10% rolls (status, then flinch) — `subscript_*_or_flinch.s` calls `CheckEffectActivation` twice and each call re-rolls `rand()%100 < effectChance`. App has *nothing* for any of the three.
- **Charge Beam** 70% SpAtk+1 self ✓ · **Metal Claw** 10% Atk+1 ✓ · **Meteor Mash** 20% Atk+1 ✓ · **AncientPower / Silver Wind / Ominous Wind** 10% all-stats+1 ✓ (`subscript_boost_all_stats.s` touches exactly Atk/Def/Spe/SpAtk/SpDef — matches the app's `OMNI_STATS` in `db.rs`; no acc/eva).
- **Flame Wheel 10% / Sacred Fire 50%** burn (same `THAW_AND_BURN_HIT` script, different chance bytes): Sacred Fire's entry present ✓; Flame Wheel's missing ✗.
- **Discharge 30% par, Lava Plume 30% burn, Poison Jab 30% psn, Gunk Shot 30% psn, Cross Poison 10% psn, Sludge Bomb 30% psn**: Sludge Bomb ✓; the other five have empty `effects[]` ✗.
- **Mirror Shot / Mud Bomb / Muddy Water 30%, Octazooka 50%**: all confirmed *Accuracy* −1 (not Evasion) ✓.
- **Rock Climb** 20% confusion ✗ (empty).
- **Draco Meteor / Overheat / Leaf Storm / Psycho Boost** SpAtk −2 self ✓ · **Close Combat** Def −1 & SpDef −1 self (`USER_DEF_AND_SPDEF_DOWN_1_STAGE`) ✓ · **Superpower** Atk −1 & Def −1 self ✓ · **Hammer Arm** Spe −1 self (`TO_ATTACKER`, despite the generic `SPEED_DOWN_HIT` id name) ✓ · **Skull Bash** Def +1 self on charge ✓.
- **Crunch**: Gen 4 = 20% **Def** −1 on target ✓ (not SpDef). **Shadow Ball** 20% SpDef −1 ✓ · **Psychic / Focus Blast / Bug Buzz / Earth Power / Energy Ball / Flash Cannon** 10% SpDef −1 ✓ · **Acid** 10% SpDef −1 — chance ✓ but app stat key says `def` ✗.
- **Iron Tail** 30% Def −1 ✓ (real in Platinum, despite the brief's doubt) · **Rock Smash / Crush Claw** 50% Def −1 ✓. **Razor Shell** and **Low Sweep** correctly absent from the 467-move table (Gen 5 moves).
- **Bubble / BubbleBeam / Constrict** 10% Spe −1 ✓ · **Icy Wind / Rock Tomb / Mud Shot** 100% Spe −1 ✓ · **Seed Flare** 40% SpDef −2 ✓.
- Pure stat/status moves — all stat/magnitude/target verified ✓: Nasty Plot, Swords Dance, Dragon Dance, Rock Polish, Agility, Bulk Up, Calm Mind, Cosmic Power, Defend Order, Iron Defense, Acid Armor, Amnesia, **Growth** (+1 SpAtk *only* — `effect_script_0013.s` touches nothing else), Howl, Meditate, Sharpen, Harden, Withdraw, Defense Curl, **Minimize** (+1 evasion — the ROM enum is misleadingly named `EVA_UP_2_MINIMIZE` but `subscript_minimize.s` uses `EVASION_UP_1_STAGE`), Double Team, Tail Glow, Flatter, Swagger, Fake Tears, Metal Sound, Tickle, Screech, Charm, FeatherDance, Leer, Tail Whip, Growl, String Shot, Sand-Attack, Smokescreen, Flash, Kinesis, Scary Face, Cotton Spore, Memento (−2 Atk / −2 SpAtk on target, matches `subscript_memento.s`). **Psycho Shift**'s empty `effects[]` is *correct* (`TRANSFER_STATUS` moves whatever status the user has — not representable in the `{status,chance}` schema). **Charge**, **Stockpile**, **Captivate** ✗ — empty `effects[]` despite guaranteed ROM stat effects (see §1c).
- **Belly Drum** ✗ `modifier: 13` (ROM sets raw stage 12 = relative +6; value is currently inert because `db.rs` special-cases the flavor).
- Top-level `effect_chance` convention re-verified on every non-damaging Status move checked: uniformly `null` regardless of self/enemy target, and for damaging moves literal-100-if-enemy / null-if-self — no inconsistencies beyond Fake Out.

The 22 empty-`effects[]` moves (19 with a nonzero ROM chance byte, found exhaustively; plus Charge/Stockpile/Captivate whose effects are unconditional and so carry a 0 chance byte) plus Fake Out form one systematic class of gap — every one of them is a Gen-4-era move (rom_id ≥ 344 apart from Waterfall 127, Flame Wheel 172, Fake Out 252, Charge 268, Stockpile 254), which suggests the secondary-effect data for moves added in or after Gen 3/4 was never back-filled.

## (4) Type chart / held items / Fling / Natural Gift / plate findings

Sources: pokeplatinum `src/battle/battle_lib.c` and all 446 `res/items/data/*.json` files; app `raw_pkmn_data/gen_four/type_info.json`, `raw_pkmn_data/gen_four/items.json`, `rust/crates/xpr-data/src/gen_consts.rs`. (pokediamond and pokeheartgold do not yet have the battle-command overlay decompiled to C — the type table there is still raw assembly in `pokeheartgold/asm/overlay_12_battle_command.s` and undecompiled `.s` files under `pokediamond/arm9` — so pokeplatinum was the authoritative source for the type chart; it is the same table across all three games.)

### 4a. Type chart — set equality: PERFECT (110/110)

`sTypeMatchupMultipliers` is at `battle_lib.c:2399-2517` as `{attacking, defending, multiplier}` rows with `TYPE_MULTI_IMMUNE=0`, `TYPE_MULTI_NOT_VERY_EFF=5`, `TYPE_MULTI_SUPER_EFF=20` (`include/constants/battle.h:144-146`; doc comment at `battle_lib.c:2390-2397` explains the values are divided by 10 downstream). A `{0xFE,0xFE,…}` sentinel at `battle_lib.c:2511` separates the main table from the two Ghost-immunity rows (`battle_lib.c:2513-2514`: Normal→Ghost and Fighting→Ghost) so Foresight/Scrappy can conditionally skip them (`battle_lib.c:2612-2620`); `{0xFF,0xFF,…}` at `:2516` terminates. Unlisted pairs are neutral: `BattleSystem_TypeMatchupMultiplier` (`battle_lib.c:3009-3027`) starts at `mul = 40` and only multiplies on a matched row.

ROM: 110 data rows. App `type_chart`: 110 entries. **0 missing, 0 spurious.**

### 4b. Type chart — row order: 0 SE-vs-NVE flips

Verified programmatically: for every attacking type and every (SE entry, NVE entry) pair, the relative order in the ROM table equals the relative order in the JSON. **Zero flips** across all 18 attacking types. The only full-order differences (none of which can change a multiplicative damage result) are:

| Attacking type | ROM order | JSON order (`type_info.json`) | Why it's harmless |
|---|---|---|---|
| Normal | Rock(NVE) `:2400`, Steel(NVE) `:2401`, …, Ghost(Immune) `:2513` | Ghost(Immune) `:46`, Rock(NVE) `:47`, Steel(NVE) `:48` | Immune entry moved; it sits in the ROM's separated Foresight/Scrappy segment. Normal has no SE entries at all in Gen 4, so no SE/NVE flip is possible. |
| Fighting | Normal(SE) `:2440` … Steel(SE) `:2448`, …, Ghost(Immune) `:2514` | Ghost(Immune) `:51` first, then the same SE run | Same cause; all SE entries keep their mutual order. |
| Grass | Rock(SE) `:2429`, Dragon(NVE) `:2430`, Steel(NVE) `:2431` | Rock(SE) `:149`, Steel(NVE) `:150`, Dragon(NVE) `:151` | Dragon/Steel swapped, but **both are NVE** — an NVE-vs-NVE swap is commutative. (Verified directly.) |

### 4c. Held-item type boosts: PERFECT (38/38)

ROM `sTypeBoostingItems` at `battle_lib.c:6514-6547`: 17 `HOLD_EFFECT_STRENGTHEN_*` items (one per type) + 16 `HOLD_EFFECT_ARCEUS_*` Plates (no Normal plate, by design). Boost formula `battle_lib.c:6716-6720`: `movePower = movePower * (100 + heldItemPower) / 100`, applied identically to both groups. All 38 have `effectParam: 20` (i.e. +20%).

- All 38 ROM item names exist verbatim in `items.json` (0 spelling misses); 0 ROM-only, 0 app-only; 0 item→type mismatches; 0 boost-percentage outliers.
- Incense hand-check (`res/items/data/*_incense.json`): **Odd** (`STRENGTHEN_PSYCHIC`), **Rock** (`STRENGTHEN_ROCK`), **Rose** (`STRENGTHEN_GRASS`), **Sea** (`STRENGTHEN_WATER`), **Wave** (`STRENGTHEN_WATER`) are the five real Gen-4 type boosters, all `effectParam: 20`. The other four Incenses are NOT type boosters and are correctly absent: Lax (`HOLD_EFFECT_ACC_REDUCE`), Luck (`HOLD_EFFECT_MONEY_UP`), Pure (`HOLD_EFFECT_ENCOUNTERS_DOWN`), Full (`HOLD_EFFECT_PRIORITY_DOWN`). Sea Incense and Wave Incense both boosting Water matches real Gen-4 behaviour.
- The 17 classic items (Black Belt/Fighting, BlackGlasses/Dark, Charcoal/Fire, Dragon Fang/Dragon, Hard Stone/Rock, Magnet/Electric, Metal Coat/Steel, Miracle Seed/Grass, Mystic Water/Water, Silk Scarf/Normal, Poison Barb/Poison, Sharp Beak/Flying, SilverPowder/Bug, Soft Sand/Ground, Spell Tag/Ghost, NeverMeltIce/Ice, TwistedSpoon/Psychic) and the 16 Plates (see 4e) all match.

### 4d. Fling power table: 126/126 common values match; 2 non-Gen-4 items present in the app table

Field: `flingPower` in each `res/items/data/*.json`. 277 ROM items have nonzero Fling power; app `GEN4_FLING_POWER` (`gen_consts.rs:450-491`) has 128 entries.

| Finding | App | ROM | Citation | Suggested fix |
|---|---|---|---|---|
| **`Cover Fossil` in `GEN4_FLING_POWER`** | `("Cover Fossil", 100)` | *item does not exist in Gen 4* | `gen_consts.rs:455`; no `cover_fossil.json` in `res/items/data/` (only armor/claw/dome/helix/root/skull fossils exist); 0 hits in `items.json` | Remove from the Gen-4 table (Cover Fossil is a Gen 5 item). |
| **`Plume Fossil` in `GEN4_FLING_POWER`** | `("Plume Fossil", 100)` | *item does not exist in Gen 4* | `gen_consts.rs:456`; no `plume_fossil.json`; 0 hits in `items.json` | Remove (Gen 5 item). |
| King's Rock spelling | `"King's Rock"` (ASCII `'`) in `gen_consts.rs:472` and `items.json:1394` | `"King’s Rock"` (U+2019 `’`) in `res/items/data/kings_rock.json` | — | Not a value mismatch (30 = 30 once normalised); noted only because it's a byte-level name difference vs. the ROM asset text. Same curly apostrophe appears in `oaks_letter.json` (no Fling relevance); `res/items/data` also spells `"Poké Doll"` with é vs. `items.json:442` `"Poke Doll"`. |

**Value mismatches among the 126 common items: 0.**

151 ROM items with nonzero Fling power are absent from the app's table. All are consumables/one-shot items (Berries = 10, medicines/battle items/evolution stones/shards/flutes/mulches/scarves = 30) rather than the hold-items the Rust table otherwise covers, which looks like intentional scope rather than error. For completeness the full list (name → ROM `flingPower`, each read from its own `res/items/data/<snake_case>.json`):

Aguav Berry 10; Antidote 30; Apicot Berry 10; Aspear Berry 10; Awakening 30; Babiri Berry 10; Belue Berry 10; Berry Juice 30; Big Mushroom 30; Big Pearl 30; Black Flute 30; Blue Flute 30; Blue Scarf 10; Blue Shard 30; Bluk Berry 10; Burn Heal 30; Calcium 30; Carbos 30; Charti Berry 10; Cheri Berry 10; Chesto Berry 10; Chilan Berry 10; Chople Berry 10; Coba Berry 10; Colbur Berry 10; Cornn Berry 10; Custap Berry 10; Damp Mulch 30; Dire Hit 30; Durin Berry 10; Elixir 30; Energy Root 30; EnergyPowder 30; Enigma Berry 10; Escape Rope 30; Ether 30; Figy Berry 10; Fire Stone 30; Fluffy Tail 30; Fresh Water 30; Full Heal 30; Full Restore 30; Ganlon Berry 10; Gooey Mulch 30; Green Scarf 10; Green Shard 30; Grepa Berry 10; Growth Mulch 30; Guard Spec. 30; HP Up 30; Haban Berry 10; Heal Powder 30; Heart Scale 30; Hondew Berry 10; Honey 30; Hyper Potion 30; Iapapa Berry 10; Ice Heal 30; Iron 30; Jaboca Berry 10; Kasib Berry 10; Kebia Berry 10; Kelpsy Berry 10; Lansat Berry 10; Lava Cookie 30; Leaf Stone 30; Lemonade 30; Leppa Berry 10; Liechi Berry 10; Lum Berry 10; Mago Berry 10; Magost Berry 10; Max Elixir 30; Max Ether 30; Max Potion 30; Max Repel 30; Max Revive 30; Micle Berry 10; Moomoo Milk 30; Moon Stone 30; Nanab Berry 10; Nomel Berry 10; Nugget 30; Occa Berry 10; Old Gateau 30; Oran Berry 10; PP Max 30; PP Up 30; Pamtre Berry 10; Parlyz Heal 30; Passho Berry 10; Payapa Berry 10; Pearl 30; Pecha Berry 10; Persim Berry 10; Petaya Berry 10; Pinap Berry 10; Pink Scarf 10; Poké Doll 30; Pomeg Berry 10; Potion 30; Protein 30; Qualot Berry 10; Rabuta Berry 10; Rare Candy 30; Rawst Berry 10; Razz Berry 10; Red Flute 30; Red Scarf 10; Red Shard 30; Repel 30; Revival Herb 30; Revive 30; Rindo Berry 10; Rowap Berry 10; Sacred Ash 30; Salac Berry 10; Shoal Salt 30; Shoal Shell 30; Shuca Berry 10; Sitrus Berry 10; Soda Pop 30; Spelon Berry 10; Stable Mulch 30; Star Piece 30; Stardust 30; Starf Berry 10; Sun Stone 30; Super Potion 30; Super Repel 30; Tamato Berry 10; Tanga Berry 10; Thunderstone 30; TinyMushroom 30; Wacan Berry 10; Water Stone 30; Watmel Berry 10; Wepear Berry 10; White Flute 30; Wiki Berry 10; X Accuracy 30; X Attack 30; X Defend 30; X Sp. Def 30; X Special 30; X Speed 30; Yache Berry 10; Yellow Flute 30; Yellow Scarf 10; Yellow Shard 30; Zinc 30.

### 4e. Plate lookup: PERFECT (16/16)

`PLATE_TYPE_LOOKUP` (`gen_consts.rs:307-324`) vs. the 16 `HOLD_EFFECT_ARCEUS_*` items in `res/items/data/`: Draco/Dragon, Dread/Dark, Earth/Ground, Fist/Fighting, Flame/Fire, Icicle/Ice, Insect/Bug, Iron/Steel, Meadow/Grass, Mind/Psychic, Sky/Flying, Splash/Water, Spooky/Ghost, Stone/Rock, Toxic/Poison, Zap/Electric — all match.

### 4f. Natural Gift table: PERFECT (64/64)

Fields `naturalGiftPower`/`naturalGiftType` in `res/items/data/*.json`. All 64 Gen-4 berries (`items.json` rom_id 149-212, Cheri through Rowap) have nonzero power; no non-berry does. `NATURAL_GIFT_TABLE` (`gen_consts.rs:367-432`, gen-4 = 2nd tuple field): **0 power mismatches, 0 type mismatches, 0 ROM-only, 0 app-only.** The gen-4 column is genuinely gen-4-specific — it differs from the gen-5 column for all 64 berries (e.g. Cheri 60 vs 80; Occa 60 vs 100, since Gen 5 raised the eight type-resist berries by +40). Spot-verified against source: `liechi_berry.json` → 80/Grass, `occa_berry.json` → 60/Fire, both matching the app.

## (5) Version differences: Platinum vs. HGSS vs. DP

All 467 moves were compared across `category`, `type`, `power`, `accuracy`, `pp`, `priority`, and `effect_chance` for both pokediamond (DP) and pokeheartgold (HGSS) against pokeplatinum (Pt).

**HGSS vs. Platinum: zero differences** across all 467 moves x 7 fields (3,269 checks) — HGSS's move table byte-for-byte matches Platinum's on every field checked.

**DP vs. Platinum: exactly one difference**, matching the prior audit's finding:

| Move (rom_id) | Field | Platinum/HGSS | Diamond/Pearl | Citation |
|---|---|---|---|---|
| Hypnosis (95) | accuracy | 60 | **70** | pokediamond `files/poketool/waza/waza_tbl.json:1429` (`"name": "MOVE_HYPNOSIS"`), `accuracy` field a few lines below = `70`; vs. pokeplatinum `res/moves/hypnosis/data.json:13` `"accuracy": 60` (HGSS narc-decoded value also `60`, confirmed via `hgss_moves.json` id 95). App's current value (60) already matches Platinum/HGSS; this is purely a cross-version note, not an app bug — Diamond & Pearl (only) used 70% base accuracy for Hypnosis; every later Gen-4 game (Platinum, HeartGold, SoulSilver) shipped 60%, which is what the app correctly encodes. No fix needed unless the app wants a DP-specific override for min-version routing. |

Also noted, not an app-relevant discrepancy: pokediamond's and pokeheartgold's raw move tables both physically contain 471 records (0..470) vs. Platinum's logical table of 468 (`MOVE_NONE`..`MOVE_SHADOW_FORCE`, ids 0-467, plus a `MAX_MOVES=468` sentinel). The three extra slots (468/469/470) in DP/HGSS hold non-null-looking but unnamed placeholder data (`MOVE_468`/`469`/`470`, generic 100-power Normal-type Special moves with no name string) that are not wired to any learnable/usable move in any Gen 4 game and are outside the app's `rom_id` range (1-467) — mentioned for completeness only, no action needed.

## (6) What was verified identical

Every one of the 467 Gen 4 moves was compared to pokeplatinum on all eight core fields (3,736 field checks): `pp`, `priority` (after the unsigned-byte conversion), `target` (via a 12-bucket `range`→`target` bijection with zero exceptions), `category` and `power` matched 467/467; `type` matched 466/467 (Curse `???`); `accuracy` 466/467 (Memento); `effect_chance` 460/467 with six of the seven differences being the app's deliberate self-target-null convention and one real gap (Fake Out). The app's `effect` id assignment matched the ROM's `BATTLE_EFFECT_*` id 467/467 with zero cross-assignment errors across 257 distinct effect ids, and the six contact/protect/magic-coat/snatch/mirror-move/king's-rock booleans matched 2,802/2,802. Across versions, HeartGold/SoulSilver's move table is identical to Platinum's on all 467 moves × 7 fields (3,269/3,269) and Diamond/Pearl differs on exactly one value (Hypnosis accuracy 70 vs 60). For `attack_flavor`, 25 tag categories covering 105 ROM-implied (move, tag) assignments over 102 moves were checked in both directions against all 467 moves' arrays: 93/105 correct, 15 anomalies (12 missing, 3 spurious). For `effects[]`, 96 named moves were checked against the dispatch scripts, subscripts and C gating code with 80 fully correct, plus an exhaustive 467-move sweep for the empty-array defect: 25 anomalies in total (22 empty arrays, Tri Attack 6.3, Acid stat key, Belly Drum magnitude). The type chart matched 110/110 matchups with 0 SE-vs-NVE order flips across all 18 attacking types; held-item boosts 38/38 (name, type and +20% param); Natural Gift 64/64 berries (power and type, confirmed Gen-4-specific); plates 16/16; Fling 126/126 common values with only the two non-Gen-4 fossil entries (Cover/Plume) as real defects. Net: **45 actionable data findings** — 3 core-field (Memento accuracy, Fake Out, Curse type [low impact]), 15 flavor, 25 effects, 2 Fling — against roughly 10,000 individual field/entry comparisons that matched the ROM exactly.
