# Damage-calc verification of the Rust port — every move, gens 1–4 (2026-09-23)

Scope: the Rust calculators in `rust/crates/xpr-calc` (`gen1.rs` … `gen4.rs`,
`damage.rs`, `battle_summary.rs`) and the move / type data in
`raw_pkmn_data/gen_{one,two,three,four}`, verified against the game
decompilations for every damaging move of Red/Blue/Yellow, Gold/Silver/Crystal,
Ruby/Sapphire/Emerald/FireRed/LeafGreen and Diamond/Pearl/Platinum/HeartGold/
SoulSilver. Gen 5 was only adapted to the API changes (no decompilation
exists; see the 2026-09-10 gen 5 findings).

This report supersedes the per-gen state described in the 2026-09-10 review
(`gen_N_findings.md`), which audited the Python app. Most of those findings
had been fixed before the Rust port; this pass re-derived the game rules
independently, swept the Rust code against them, and fixed what was left.

---

## 1. Method

**Sources.** The pret decompilations were shallow-cloned on this machine
(the `A:\` paths in `CLAUDE.md` are the Windows checkout): pokered, pokeyellow,
pokegold, pokecrystal, pokeruby, pokeemerald, pokefirered, pokediamond,
pokeplatinum, pokeheartgold. Every rule below was read from the game code:

| Gen | Routines read (file: routine) |
|---|---|
| 1 | pokeyellow `engine/battle/core.asm`: `GetDamageVarsForPlayerAttack`/`..EnemyAttack`, `CalculateDamage`, `CriticalHitTest`, `AdjustDamageForMoveType`, `RandomizeDamage`, `MoveHitTest`, `CalcHitChance`, `ApplyAttackToEnemyPokemon`/`..PlayerPokemon` (Super Fang, fixed damage, Psywave), `HandleCounterMove`, the Bide / trapping / multi-hit paths of `CheckPlayerStatusConditions` and `ExecutePlayerMove`; `move_effects/one_hit_ko.asm`, `focus_energy.asm`; `data/types/type_matchups.asm`, `data/battle/critical_hit_moves.asm`, `data/moves/moves.asm`. pokered diffed (identical for everything cited). |
| 2 | pokecrystal `engine/battle/effect_commands.asm`: `BattleCommand_Critical`, `Stab` (weather, badge type boost, STAB, type loop), `DamageVariation`, `CheckHit` (+ `.StatModifiers`, `.BrightPowder`), `DamageStats`/`PlayerAttackDamage`/`EnemyAttackDamage`, `TruncateHL_BC`, `CheckDamageStatsCritical`, `ThickClubBoost`/`LightBallBoost`, `DittoMetalPowder`, `DamageCalc`, `ConstantDamage`, `OHKO`, `EndLoop`, `DoubleFlyingDamage`/`DoubleUndergroundDamage`/`DoubleMinimizeDamage`; `engine/battle/misc.asm` `DoWeatherModifiers`, `DoBadgeTypeBoosts`; `move_effects/` present, beat_up, triple_kick, fury_cutter, rollout, magnitude, rage, false_swipe, hidden_power, thunder, future_sight, pursuit, counter, mirror_coat, bide, return, frustration; `data/moves/effects.asm` (command order of every damaging script); the data tables under `data/moves`, `data/types`, `data/battle`. pokegold diffed: `TruncateHL_BC` (one pass, low byte) and `present.asm` (register clobber) are the only damage differences. |
| 3 | pokeemerald `src/pokemon.c` `CalculateBaseDamage`, `ShouldGetStatBadgeBoost`; `src/battle_script_commands.c` `Cmd_critcalc`, `Cmd_damagecalc`, `Cmd_typecalc`/`ModulateDmgByType`, `CheckWonderGuardAndLevitate`, `AccuracyCalcHelper`/`Cmd_accuracycheck`, `ApplyRandomDmgMultiplier`/`Cmd_adjustnormaldamage`, `Cmd_tryKO`, `Cmd_damagetohalftargethp`, `Cmd_psywavedamageeffect`, `Cmd_counterdamagecalculator`/`mirrorcoat`, `Cmd_rolloutdamagecalculation`, `Cmd_furycuttercalc`, `Cmd_friendshiptodamagecalculation`, `Cmd_presentdamagecalculation`, `Cmd_magnitudedamagecalculation`, `Cmd_hiddenpowercalc`, `Cmd_trysetfutureattack`, `Cmd_trydobeatup`, `Cmd_setdamagetohealthdifference`, `Cmd_scaledamagebyhealthratio`, `Cmd_weightdamagecalculation`, `Cmd_setweatherballtype`, `Cmd_stockpiletobasedamage`, `Cmd_callenvironmentattack`; `src/battle_util.c` absorbing abilities, Soundproof list; `src/battle_main.c` `gTypeEffectiveness`; `data/battle_scripts_1.s` (MultiHit, TripleKick, BeatUp, SpitUp, FutureSight, OHKO, Psywave, Counter, Endeavor, SuperFang, Present, LowKick, …). pokeruby `BADGE_BOOST` (trainer battles only) and pokefirered `ShouldGetStatBadgeBoost` read; `pokedex_entries.h` weights identical in the three repos. |
| 4 | pokeplatinum `src/battle/battle_lib.c` `BattleSystem_CalcMoveDamage`, `BattleSystem_ApplyTypeChart`, `ApplyTypeMultiplier`, `BasicTypeMulApplies`, `BattleSystem_Divide`, `BattleSystem_CalcDamageVariance`, `BattleSystem_CalcCriticalMulti`, `MoveIsOnDamagingTurn`, `sTypeMatchupMultipliers`; `src/battle/battle_script.c` `BattleScript_CalcMoveDamage`, `BtlCmd_CalcDamage`/`CalcMaxDamage`, `SetMultiHit`, `TryOHKOMove`, `CalcFlailPower`, `CalcRolloutPower`, `CalcFuryCutterPower`, `Present`, `CalcMagnitudePower`, `CalcHiddenPowerParams`, `TryFutureSight`, `BeatUp`, `CalcRevengePowerMul`, `CalcHPFalloffPower`, `CalcWeightBasedPower`, `CalcWeatherBallParams`, `TryPursuit`, `CalcGyroBallPower`, `TryMetalBurst`, `CalcPaybackPower`, `CalcTrumpCardPower`, `CalcWringOutPower`, `CalcPunishmentPower`, `GetTerrainMove`, `CalcNaturalGiftParams`, `TryFling`; `src/battle/battle_controller_player.c` `CheckMoveHitAccuracy`, `CheckMoveHitOverrides`, `CheckTypeChart`, `LoopMultiHit`; `include/data/hit_rate_stages.h`, `battle/weight_to_power.h`, `terrain/to_move.h`; the effect scripts `res/battle/scripts/effects/effect_script_{0000,0029,0038,0041,0044,0087,0088,0089,0104,0126,0144,0154,0155,0159,0161,0209,0227,0233}.s`; `res/moves/*/data.json`. pokeheartgold/pokediamond move tables diffed (identical except DP Hypnosis accuracy 70). |

**Sweep harness.** `rust/crates/xpr-calc/examples/sweep.rs` runs the real
calculators (`calculate_damage`, `get_crit_rate`, `get_move_accuracy`) over a
JSON list of cases and dumps the resolved inputs (battle stats with and
without stages/badges, species types, move data). Python transcriptions of
each generation's routines (`reference_2026_09_23/ref_calcs.py`) recompute
every case from those inputs and the decomp tables (the type charts are parsed
from the decomp files, not from the app); `compare.py` diffs the full damage
distributions (every roll count, crit and non-crit).

Cases (`gen_cases.py`): every damaging move of the version × 3 matchups from
a species pool of 20–43 species across all types × every dropdown option ×
crit / non-crit, plus rotating modifier variants (stages ±1/±2/±6, screens,
badges, weather, doubles, held items, abilities, field effects, wild flag).
Final result:

| Version | Checks (distributions) | Mismatches |
|---|---|---|
| Yellow | 2,610 | 0 |
| Crystal | 5,040 | 0 |
| Gold | 5,040 | 0 |
| Emerald | 9,172 | 0 |
| Platinum | 15,940 (36 Natural Gift / Fling cases skipped by the reference; those tables were verified separately, §5) | 0 |

**Data checks.** Four sub-audits (one per gen, `data_checks_2026_09_23/`)
mechanically diffed every move's power / type / accuracy / PP / priority /
target / flags / secondary effect (kind and chance) / `attack_flavor` and the
type charts, `special_types`, held-item, plate, Natural Gift and Fling tables
against the decomp tables, then hand-verified every anomaly in the effect
routines. Their findings are applied in §9.

**Tests.** `rust/crates/xpr-calc/tests/game_formulas.rs` (62 named cases
whose expected values come from the Python transcriptions, one per fixed
rule), the existing golden replay (`tests/damage_cases.rs`, 439 Python cases;
one entry re-recorded, §9) and `tests/ohko.rs`; `cargo test --workspace` is
green.

---

## 2. Results at a glance

| Gen | Damaging moves covered | Calculator fixes | Data fixes | Newly implemented | Still not implemented (needs new inputs) |
|---|---|---|---|---|---|
| 1 | 110 | exact accuracy (byte/256, stages) | Struggle accuracy, Psychic chance, 4 effect keys, 6 PP values | — | Focus Energy, burn/paralysis stat penalties, Substitute, confusion self-hit |
| 2 | 152 | two-hit crit (3 hits→2), Gold/Silver Present rounding + badge boost, crit items, Hidden Power crit gate, exact accuracy (+stages, BrightPowder), per-step min-1 | AncientPower/Spikes chances, Flame Wheel/Snore/Twister effects, Swagger status, Tri Attack chance, Fly/Dig flavor, Hidden Power power byte | Beat Up, Counter, Mirror Coat, Bide (dropdowns) | Focus Energy, Foresight, Lock-On, X Accuracy |
| 3 | 217 | modifier order (Huge Power → badges → items → …), RS wild-battle badge rule, Beat Up crit, Battle/Shell Armor, two-hit crit, crit items, accuracy stages + BrightPowder/Lax Incense, Nature Power spread, per-step min-1 | Bounce effect, Tri Attack chance, SonicBoom power, Surf/Whirlpool flavor, species weights (389 × 3 versions) | Low Kick, Counter, Mirror Coat, Bide (dropdowns) | Flash Fire boost, Guts, Marvel Scale, Overgrow family, Plus/Minus, Charge, Helping Hand, Sports, Focus Energy, Foresight, Lock-On, burn |
| 4 | 297 | Technician ordering, item/plate/orb/band/glasses/Thick Fat/Heatproof/Dry Skin on power, Light Ball on resolved power, Explosion on raw Def, Filter + Expert Belt, Gravity vs Levitate, Iron Ball grounding, Scrappy/fixed damage, Wonder Guard vs fixed/OHKO, Future Sight/Doom Desire crit, Battle Armor, Gyro Ball under Trick Room, two-hit crit, Nature Power category/targeting, per-step min-1, accuracy (stages, BrightPowder ×0.9, Gravity, Cloud Nine gating), Klutz on crit items | Memento accuracy, Fake Out, Acid stat, Tri Attack chance, 22 missing secondary effects, 12 missing / 3 spurious flavors, 2 Fling entries | Beat Up, Counter, Mirror Coat, Metal Burst, Bide (dropdowns) | the gen 3 list plus Rivalry, Simple, Unaware, Mold Breaker, Skill Link, Tangled Feet, Me First, Metronome item, Zoom Lens, Micle Berry, resist berries, Lucky Chant, Endure/Focus Sash/Band |
| all | — | KO chance: exact per-hit crit / accuracy model for multi-hit moves; crit rate 0 for never-crit moves and armoured targets | — | — | — |

---

## 3. Calculator fixes (what the game does, what the app did, what it does now)

Citations are decomp file:routine; "sweep" means the distribution was checked
by the harness, "test" names the case in `tests/game_formulas.rs`.

### 3.1 Gen 1

| # | Rule (game) | Before | After | Test |
|---|---|---|---|---|
| 1.1 | `MoveHitTest`/`CalcHitChance`: the accuracy byte is `acc * 255 / 100`, scaled by the user's accuracy stage and the target's evasion stage (`StatModifierRatios`, min 1 per step, cap 255); hit iff `BattleRandom < byte`, so a 100% move misses 1/256 of the time and 95% is 242/256. Swift skips the test. | nominal percent, stages ignored | exact byte/256 with stages (`gen1::get_move_accuracy`) | `gen1_accuracy_byte`, `gen1_accuracy_stages` |

Everything else in gen 1 was already exact (§4.1).

### 3.2 Gen 2

| # | Rule (game) | Before | After | Test |
|---|---|---|---|---|
| 2.1 | `EndLoop` re-runs `critical … damagevariation` per hit; a two-hit move (Double Kick, Bonemerang, Twineedle) is exactly two hits. | the crit range of a two-hit move was **three** hits (the non-crit recursion re-applied the two-hit flavor) | one crit hit + one normal hit | `gen2_double_kick_crit_two_hits` |
| 2.2 | Gold/Silver `BattleCommand_Present` clobbers bc/de: `damagecalc` runs with level = target type-2 id, A = type matchup ×10, D = user type-2 id (1 with STAB), then the normal `stab` (weather, badge type boost, STAB, per-row type loop) and `damagevariation`. | the G/S path applied the type multiplier as one `×matchup/10` and skipped the badge type boost | the clobbered `damagecalc` result goes through the same `stab` path as every other move | `gen2_present_gold_silver`, `gen2_present_crystal` |
| 2.3 | `BattleCommand_Critical`: Chansey + Lucky Punch or Farfetch'd + Stick jump straight to stage 2 (64/256); otherwise high-crit move +2 and Scope Lens +1 add up (85/256 for both); a move with power 0 never crits. | Scope Lens / Lucky Punch / Stick ignored | stages as in the game | `gen2_scope_lens`, `gen2_lucky_punch`, `gen2_slash_scope_lens` |
| 2.4 | Hidden Power's move-table power byte is 1 (`moves.asm:253`), so the crit roll runs. | `moves.json` stored 0 → (after 2.3) no crit | byte 1 in the data and a guard in `get_crit_rate` | sweep |
| 2.5 | `CheckHit`: accuracy byte `acc * 255 / 100` (255 = cannot miss), scaled by `AccuracyLevelMultipliers` for the accuracy / evasion stages (min 1 per step, cap 255), minus 20 for the target's BrightPowder; the OHKO byte from `BattleCommand_OHKO` goes through the same scaling. | nominal percent, no stages, no BrightPowder | exact | `gen2_accuracy_byte`, `gen2_accuracy_brightpowder`, `gen2_accuracy_evasion` |
| 2.6 | `Stab` type loop: after each `×5/10` step a result of 0 becomes 1. | single min-1 at the end | per step | sweep |
| 2.7 | Beat Up (`beat_up.asm` + BeatUp script: `critical, beatup, damagecalc, damagevariation`): one hit per party member with the member's base Attack, the target's base Defense and the member's level, power 10; type-boost item and crit per hit; no `stab` (no STAB / type / weather / badge). | returned no damage | implemented with a "1".."6" hits dropdown, every hit using the user's base Attack and level (see §8 for the party approximation) | `gen2_beat_up` |
| 2.8 | Counter / Mirror Coat (`counter.asm`, `mirror_coat.asm`): 2× the last damage taken, capped 65535; Ghost / Dark immune via `resettypematchup`. Bide: 2× the damage stored, Ghost immune. | no damage (no input) | "damage taken" dropdown (25 … 400) → 2× with the immunity check | `gen2_counter`, `gen2_counter_ghost` |

### 3.3 Gen 3

| # | Rule (game) | Before | After | Test |
|---|---|---|---|---|
| 3.1 | `CalculateBaseDamage` modifier order: Huge/Pure Power ×2 → badge boosts `110/100` → type-boost item (`param+100`)/100 → Choice Band → Soul Dew → DeepSeaTooth/Scale → Light Ball → Metal Powder → Thick Club → Thick Fat (SpA/2) → Hustle → Explosion → stage. | badge boost applied inside the stat calc before Huge Power; Hustle before Choice Band and the type item | the game's order on badge-free raw stats | `gen3_huge_power_before_badges`, `gen3_choice_band_then_type_item` |
| 3.2 | Ruby/Sapphire `BADGE_BOOST`: the Atk/Def/SpA/SpD badge boosts require `BATTLE_TYPE_TRAINER` (Emerald/FRLG apply them in wild battles too). | applied in RS wild battles | `DamageArgs::is_wild_battle` (set by the battle summary) disables them for Ruby/Sapphire | `gen3_ruby_wild_no_badge_boost`, `gen3_ruby_trainer_badge_boost` |
| 3.3 | Beat Up script: `critcalc` per hit. | the crit range doubled **every** hit | one crit hit + normal hits (and per-hit crits in the KO model) | `gen3_beat_up_crit_one_hit` |
| 3.4 | `Cmd_critcalc`: no crit against Battle Armor / Shell Armor; Scope Lens +1, Lucky Punch (Chansey) +2, Stick (Farfetch'd) +2; `critcalc` never runs for Spit Up, Future Sight, Doom Desire, the fixed-damage, OHKO, Counter-family and Super Fang scripts. | armoured targets still lost screens / stages on the "crit" range and reported 1/16; items ignored | crit rate 0 and crit range = normal range for armour and never-crit moves; items counted | `gen3_battle_armor`, `gen3_scope_lens` |
| 3.5 | Two-hit moves: same as 2.1 (`BattleScript_MultiHitLoop`). | three hits on a crit | two | sweep (Double Kick, Twineedle, Bonemerang) |
| 3.6 | `Cmd_accuracycheck`: `buff = accStage + 6 - evasionStage` clamped 0..12, `calc = ratio[buff] * acc`; then Compound Eyes ×130/100, Sand Veil ×80/100 (weather active), Hustle ×80/100 (physical), BrightPowder ×90/100 / Lax Incense ×95/100. | no stages, no items | exact | `gen3_accuracy_stages`, `gen3_accuracy_brightpowder` |
| 3.7 | `ModulateDmgByType`: min 1 after each non-zero step. | single min-1 at the end | per step | sweep |
| 3.8 | Low Kick (`Cmd_weightdamagecalculation`): power 20/40/60/80/100/120 for a weight below 100/250/500/1000/2000 hg (strict), else 120. | no weights in the gen 3 species data → 1–2 damage | `weight` (kg) added to the 389 species of Emerald, RS and FRLG from `pokedex_entries.h` (identical across the three); implemented | `gen3_low_kick_heavy`, `gen3_low_kick_light` |
| 3.9 | Nature Power calls the terrain's move with that move's own targeting (Swift / Razor Leaf / Rock Slide / Surf are `MOVE_TARGET_BOTH`). | Nature Power's own target ("depends") → never halved in doubles | the called move's targeting | sweep (doubles) |
| 3.10 | Counter / Mirror Coat / Bide: 2× the damage taken (`typecalc2` immunity). | no dropdown → no damage | "damage taken" dropdown | `gen3_counter` |

### 3.4 Gen 4

| # | Rule (game) | Before | After | Test |
|---|---|---|---|---|
| 4.1 | `BattleSystem_CalcMoveDamage`: Technician applies to `movePower` **after** the script-level power (Rollout, Fury Cutter, Magnitude, Return, Gyro Ball, Grass Knot, Trump Card, …) and after `powerMul` (the ×2 of Revenge/Avalanche/Payback/Pursuit/Gust/…). | Technician applied to the move-table power before any of that (Grass Knot 1 → wrong; Rollout / Fury Cutter hard-coded 30 / 10 lost it; Revenge with the bonus got ×1.5 on 60 then ×2) | resolved power → `powerMul` → Technician | `gen4_technician_grass_knot`, `gen4_technician_rollout`, `gen4_technician_after_bonus` |
| 4.2 | Light Ball: `movePower *= 2` on the resolved power. | applied before Return / Spit Up / Flail / … overwrote the power | on the resolved power | `gen4_light_ball_return` |
| 4.3 | Type-boost items and plates, Adamant/Lustrous/Griseous Orb, Muscle Band / Wise Glasses multiply `movePower` (×120/100, ×120/100, ×110/100); Thick Fat and Heatproof halve it; Dry Skin ×125/100. | applied to the attacking stat (or, for Dry Skin, the final damage) — off by one in the sweep for e.g. Zap Plate Thunder, Muscle Band Superpower, Dry Skin Fire Blast | on the power, in the game's order | `gen4_plate_on_power`, `gen4_muscle_band`, `gen4_thick_fat_on_power`, `gen4_dry_skin_on_power` |
| 4.4 | Explosion / Selfdestruct: `defenseStat /= 2` on the raw stat, before the stage. | halved the staged stat | raw | `gen4_explosion_raw_def` |
| 4.5 | `ApplyTypeChart`: on a net super-effective hit Filter/Solid Rock (`Divide(d*3, 4)`) **and** Expert Belt (×120/100) both apply. | `else if` — Expert Belt skipped when the target had Filter/Solid Rock | both | `gen4_expert_belt_solid_rock` |
| 4.6 | Gravity suppresses Levitate (`Battler_Ability`); Iron Ball grounds the holder (Flying immunity and Levitate/Magnet Rise off, `BasicTypeMulApplies`). | Levitate still blocked Ground moves under Gravity; Iron Ball not modelled | as the game | `gen4_gravity_levitate`, `gen4_iron_ball_grounds` |
| 4.7 | Fixed-damage moves (`SYSCTL_IGNORE_TYPE_CHECKS`) still go through the immunity rows, so Scrappy lets Normal/Fighting fixed damage hit Ghosts, and Wonder Guard blocks any move with a non-zero power byte that is not super effective (fixed damage, OHKO, Super Fang, Counter family have power 1). | Scrappy + SonicBoom dealt 1 damage (fell into the formula with power 1); Wonder Guard not checked for these moves | correct | `gen4_scrappy_sonicboom`, `gen4_wonder_guard_ohko` |
| 4.8 | Future Sight / Doom Desire damage is computed at setup with `criticalMul = 1`: no crit, so stages and screens are never dropped. | the "crit" range applied the crit stage rule and skipped screens | crit range = normal range | `gen4_future_sight_no_crit` |
| 4.9 | Battle Armor / Shell Armor: `criticalMul` stays 1 (stages / screens kept); `get_crit_rate` = 0. Focus-type items are void under Klutz. | 1/16 reported; screens dropped on the crit range | 0 / equal ranges | `gen4_battle_armor_crit_rate` |
| 4.10 | Gyro Ball uses `monSpeedValues` (stages, items, Tailwind — not the Trick Room ordering transform). | fed the Trick-Room-transformed speeds | speeds without the transform | `gen4_gyro_ball_trick_room` |
| 4.11 | Two-hit moves: one crit hit + one normal hit. | three hits on a crit | two | `gen4_double_kick_crit_two_hits` |
| 4.12 | Nature Power (`to_move.h`) runs the called move: Blizzard / Hydro Pump / Ice Beam / Tri Attack / Mud Bomb / Air Slash are special, Earthquake / Seed Bomb / Rock Slide physical; Earthquake, Rock Slide and Blizzard are spread. | always treated as physical (Nature Power's own category is Status) and never spread | category and targeting from the table (`gen_consts::gen4_nature_power_move`) | `gen4_nature_power_snow_special` |
| 4.13 | `ApplyTypeMultiplier` uses `BattleSystem_Divide` (min 1) per row. | single min-1 at the end | per step | sweep |
| 4.14 | Accuracy (`CheckMoveHitAccuracy`): `sum = 6 + acc − eva` clamped 0..12 → `HitRateByStage`; Compound Eyes ×130/100; (no Cloud Nine) Sand Veil / Snow Cloak ×80/100, fog ×6/10; Hustle ×80/100 physical; BrightPowder / Lax Incense ×(100−10)/100; Wide Lens ×110/100; Gravity ×10/6; Thunder-in-rain / Blizzard-in-hail bypass and Thunder 50 in sun only without Cloud Nine. | no stages; BrightPowder subtracted 10 points; no Gravity; weather effects not gated by Cloud Nine | exact | `gen4_accuracy_gravity`, `gen4_accuracy_brightpowder`, `gen4_accuracy_stages`, `gen4_sand_veil_cloud_nine` |
| 4.15 | Crit stage: Razor Claw / Scope Lens +1, Super Luck +1, Lucky Punch / Stick +2, capped 4; Sniper ×3. | Klutz not honoured for the items | as the game | `gen4_razor_claw`, `gen4_sniper_crit` |
| 4.16 | Beat Up (`BtlCmd_BeatUp`): base Attack × 10 × (2L/5+2) / base Defense / 50 + 2, × crit per hit, variance; `SYSCTL_IGNORE_IMMUNITIES` (no STAB / chart / items). | vanilla 10-power Dark move | implemented ("1".."6" hits) | `gen4_beat_up` |
| 4.17 | Counter / Mirror Coat 2×, Metal Burst ×15/10 the damage taken (`SYSCTL_IGNORE_TYPE_CHECKS`: immunities still apply), Bide 2×. | vanilla power 1 (1–2 damage) | "damage taken" dropdown | `gen4_metal_burst`, `gen4_counter_ghost` |
| 4.18 | `GEN4_FLING_POWER`: Cover Fossil and Plume Fossil are gen 5 items. | listed | removed | data check |

### 3.5 Shared: kill chance

`find_kill` modelled every use of a move as "crit with probability p, else
normal" using the whole-use ranges. For multi-hit moves in gens 2–4 that is
wrong twice: each hit rolls its own crit (`EndLoop`, `MultiHitLoop`,
`LoopMultiHit` reload the script per hit), and the joint roll tables of 2–5
hits exceeded the search's 200-entry limit, so most multi-hit moves only ever
showed the guaranteed-kill row.

New model (`damage::HitModel`, `damage::find_kill_hits`, `xpr_calc::hit_model`):
the calculators expose the per-hit non-crit / crit ranges of a use (multi-hit
and two-hit moves, Beat Up, Triple Kick); the kill search convolves the hits
with independent per-hit crit probability, one accuracy roll per use (or one
per kick with a miss ending the sequence for gen 3/4 Triple Kick, as their
scripts do), then convolves uses. It is exact and runs in `O(uses × HP ×
values)` with dense probability vectors (no roll-table size limit). Gen 1
keeps the old model because its multi-hit moves roll once for all hits
(`ExecutePlayerMove`: "damage calculation and accuracy tests only happen for
the first hit"). Single-hit moves keep the existing bit-identical search.

Example: L50 Ralts Bullet Seed (2 hits) — the app previously showed only a
guaranteed row; a KO that needs one crit among the two hits is now 1 −
(15/16)² ≈ 12.1%, not the 6.25% the "one crit per use" model implies.

Crit-rate inputs to the search were also corrected: `get_crit_rate` now
takes the full `DamageArgs` (defender + fields) so Battle Armor / Shell Armor,
Klutz and the never-crit scripts (Flail/Reversal, Future Sight/Doom Desire,
Spit Up (gen 3), fixed damage, OHKO, Counter family, Super Fang, Endeavor)
return 0.

---

## 4. Verified correct (no change needed)

### 4.1 Gen 1
Stat scaling `>255 → both /4, low byte`, Reflect/Light Screen 16-bit doubling
before it, Explosion halving after it, `2L/5+2` (level doubled on a crit),
×bp ×A /D /50, `min(997)+2`, STAB `+d>>1`, type table row order (all 82
rows, incl. the Ghost→Psychic immunity), 0-damage → miss, `217..255/255`
roll (unchanged below 2), one roll × N for multi-hit and trapping turns,
crit rate `bs/2 → ×2 (cap 255) → ×4 high-crit (cap) or /2`, crit stats (party
stats, no stages / badges / screens), Super Fang `HP>>1` min 1, fixed damage
ignoring immunity, Psywave `[1, 1.5L)` player / `[0, 1.5L)` enemy, OHKO speed
gate + 76/256, Counter/Bide ×2 cap 65535.

### 4.2 Gen 2
`DamageStats` (screens ×2, `CheckDamageStatsCritical` reload rule, Thick Club
for Cubone and Marowak, Light Ball), `TruncateHL_BC` (Crystal loop; G/S one
pass + wrap), `DittoMetalPowder` on the 8-bit stat with the overflow path,
Explosion on the truncated stat, `DamageCalc` order (item ×110/100, crit ×2 cap
65535, `min(997)+2`), weather ×15/10 / ×5/10 (SolarBeam in rain only), badge
type boost `+max(d>>3,1)` player-only, STAB, type loop in `TypeMatchups` order
(all 17 attacking types match the JSON order), `damagevariation`, the ×2
after the roll for Gust/Twister/Earthquake/Magnitude/Stomp/Pursuit, Rollout
(×2 per turn, Defense Curl) / Fury Cutter (cap 5) / Rage multipliers after
`stab`, Triple Kick `×k` before `stab` with per-kick rolls, Flail/Reversal
single value with no crit and no roll, Future Sight without `stab`, Magnitude
/ Return / Frustration / Present (Crystal) powers, Hidden Power (DV bits,
`>= 8`), fixed damage / Psywave / Super Fang / OHKO (`76 + 2ΔL`), False Swipe cap,
Struggle (no `stab`, Pink/Polkadot Bow boost), multi-hit counts.

### 4.3 Gen 3
Stat formula and nature rounding, stage ratios, `CalculateBaseDamage` core,
crit stat-stage rule, screens (`/2`, `2*(d/3)` in doubles), spread `/2` for
`MOVE_TARGET_BOTH`, physical min-1, weather (rain / sun / SolarBeam in rain,
sand, hail; Cloud Nine / Air Lock; Forecast), `+2`, crit ×2, `dmgMultiplier`
×2 moves (Gust, Twister, Surf, Whirlpool, Earthquake, Pursuit, Stomp,
Astonish, Needle Arm, Extrasensory, Facade, SmellingSalt, Revenge, Weather
Ball, Magnitude vs Dig), Spit Up (×n, no roll, no crit), STAB ×15/10, type
loop order, immunities (Levitate, Damp, Volt/Water Absorb, Flash Fire,
Soundproof, Wonder Guard with the SE/NVE cancel rule), random 85..100 (16
values), Hidden Power, Magnitude / Flail / Return / Eruption / Present /
Rollout / Fury Cutter / Triple Kick (10/20/30, per-kick accuracy) / Psywave (11
values) / Endeavor / Super Fang / OHKO (`acc + ΔL − 1`, Sturdy) / Future Sight
(no `typecalc`), Sea Incense ×105/100, Choice Band, Soul Dew, DeepSea items,
Metal Powder, Thick Club, Hustle, Thick Fat, Compound Eyes / Sand Veil /
Hustle / Thunder accuracy.

### 4.4 Gen 4
Stat formula, stage table, no badge boosts, `CalcMoveDamage` core with the
crit stat-stage rule, screens (`/2`, `×2/3` in doubles), spread ×3/4 for
`RANGE_ADJACENT_OPPONENTS` / `RANGE_ALL_ADJACENT`, weather (incl. SolarBeam in
fog and Weather Ball in fog), sandstorm Rock SpD, Solar Power, Flower Gift,
`+2`, crit ×2 / Sniper ×3, Life Orb ×130/100, STAB / Adaptability, type chart
order, Filter / Solid Rock once on the net result, Tinted Lens on the net
result, variance (16 values), Hidden Power, Natural Gift (64 berries), Fling
(126 items), plates (16), Judgment, Multitype, Normalize, Scrappy, Magnet
Rise, Miracle Eye, Roost, Worry Seed / Gastro Acid, Klutz, Choice items, Soul
Dew, DeepSea items, Metal Powder, Thick Club, Hustle, Huge/Pure Power,
Reckless, Iron Fist, Magnitude, Flail, Return, Eruption, Crush Grip, Gyro
Ball cap, Trump Card, Low Kick / Grass Knot (≤ hg), Rollout, Fury Cutter,
Spit Up (100n, crit allowed), Punishment, Present, Weather Ball, Endeavor,
Super Fang, OHKO (`acc + ΔL`, Sturdy, Wide Lens ignored), Psywave (11 values),
Struggle typeless, Future Sight / Doom Desire outside the chart, Compound
Eyes / Wide Lens / Sand Veil / Snow Cloak / fog / No Guard / Hustle accuracy.

---

## 5. Type effectiveness

Each generation's `type_chart` in `type_info.json` was compared with the
decomp table as a set and in row order (the order matters for dual-type
rounding because every step floors):

| Gen | Table | Set | Order |
|---|---|---|---|
| 1 | `data/types/type_matchups.asm` (82 rows) | identical | the calculator applies rows in ROM order via `GEN1_TYPE_EFFECT_ORDER` (the JSON dict order differs, which is why the explicit order exists) |
| 2 | `data/types/type_matchups.asm` (108 + 2 Foresight rows) | identical (110) | identical for every attacking type except Grass Dragon/Steel (both NVE: commutative) and the Ghost immunities (0 either way) |
| 3 | `gTypeEffectiveness` (112 rows incl. the Foresight rows) | identical | as gen 2 |
| 4 | `sTypeMatchupMultipliers` (110 rows) | identical | as gen 2 |

`special_types` matches the `SPECIAL` / `TYPE_MYSTERY` boundary in gens 1–3
(gen 4 uses the per-move category). The immunity-removal cases were checked
in code: Foresight is not modelled (§8), Scrappy (gen 4), Gravity / Roost /
Iron Ball / Ingrain-less Magnet Rise for Ground vs Flying, Miracle Eye for
Psychic vs Dark. Held-item type boosts: gen 2 18/18 (`type_boost_items.asm`,
Dragon Scale really boosts Dragon in gen 2), gen 3 17/17 param 10 with Sea
Incense (param 5) special-cased in code, gen 4 38/38 (17 items + 16 plates + 5
incenses, all +20%).

---

## 6. Secondary effects

The app uses `effects[]` for the stat-stage setup dropdowns of the battle
summary (`MoveDB::get_stat_stage_info`: guaranteed self / target stat changes
of status moves and the stat-changing side effects of damaging moves, `omni`
for the all-stats moves, Belly Drum special-cased by flavor) and for display.
Status / flinch entries are informational.

Every entry was checked against the effect routines (gen 1 `effects.asm`
side-effect rolls, gen 2 `effectchance` bytes and scripts, gen 3
`secondaryEffectChance` and `battle_scripts_1.s`, gen 4 `data.json` +
subscripts). Corrections applied (§9): gen 1 Psychic 33.2%, four `percent` →
`chance` keys; gen 2 AncientPower 10%, Spikes 100%, Flame Wheel / Snore /
Twister entries, Swagger "Confusion", Tri Attack 6.64% (= 17/256, the
`effectchance` roll then a 1-of-3 pick); gen 3 Bounce 30% paralysis (not
flinch), Tri Attack 6.67%; gen 4 Fake Out 100% flinch, Acid Sp. Def, Tri
Attack 6.67%, and 22 moves whose `effects[]` was empty (Fire/Ice/Thunder Fang
— two independent 10% rolls each —, Flame Wheel, Discharge, Lava Plume, Poison
Jab, Cross Poison, Gunk Shot, Rock Climb, Force Palm, Volt Tackle, Flare
Blitz, Waterfall, Dark Pulse, Air Slash, Dragon Rush, Zen Headbutt, Iron Head,
Charge +1 SpD, Stockpile +1 Def/+1 SpD, Captivate −2 SpA).

Known representation limits (not bugs): gen 1's single Special stat is
stored as paired `spa`+`spd` entries; Secret Power's terrain-dependent effect
is a placeholder; Thief's steal, Psycho Shift's status transfer and Tri
Attack's three-way pick are not expressible as one `{status, chance}`;
Belly Drum keeps `modifier: 13` (inert: the code keys on the `belly_drum`
flavor and sets +6 / gen 2 +2 or +6).

---

## 7. Chance to KO, accuracy and crits — what the numbers mean now

- **Single-hit moves**: unchanged search (`find_kill`, bit-identical to the
  Python port) with the corrected crit rate and accuracy inputs.
- **Multi-hit moves (gens 2–4)**: exact per-hit model (§3.5). The "N Hits"
  dropdown still picks the hit count (the game rolls 2/3/4/5 with 3/8, 3/8,
  1/8, 1/8; Skill Link always 5 — not modelled).
- **Accuracy**: gen 1 byte/256 (so 100% shows 99.6% and a 2HKO shows
  99.2%; disable with the "ignore accuracy" setting if that noise is
  unwanted), gen 2 byte/256 with 255 = never misses, gens 3/4 percent; all
  four apply the accuracy / evasion stages already tracked by the stat-stage
  setup (Sand-Attack, Double Team, …) and the defender's BrightPowder / Lax
  Incense; gen 4 adds Gravity and gates weather effects on Cloud Nine.
- **Crits**: rates now 0 where the game never rolls one; gen 2/3/4 items and
  abilities counted; gen 1 Focus Energy still needs an input (§8).
- **Displayed crit range**: for multi-hit / Triple Kick / Beat Up it is
  "exactly one hit crits" (the last kick for Triple Kick); the KO percentages
  do not depend on this convention.
- **Wild battles**: the min-DV / max-DV merge keeps the whole-use model for
  multi-hit moves (the merge only carries min/max).

---

## 8. Not implemented — how to implement each

Everything below cannot be computed from the inputs the app has today
(species, level, DVs/IVs, stat exp, badges, held item, ability, stat stages,
screens, weather, doubles, dropdowns). Each entry gives the game rule with its
source and the smallest change that would model it.

### 8.1 All generations

| Mechanic | Game rule | Implementation |
|---|---|---|
| Focus Energy | Gen 1 `CriticalHitTest`: the bug shifts right instead of left (rate ×¼: `b = bs>>3`, high-crit `4*(bs>>2)`). Gen 2 `Critical` +1 stage; gen 3 `Cmd_critcalc` +2; gen 4 `CalcCriticalMulti` +2. | Add a per-side field toggle (like Reflect) in `FieldStatus` (`focus_energy`) set by the setup-move list; consume it in each `get_crit_rate`. Gen 1's formula is already in `gen1::get_crit_rate` behind the `focus_energy` custom string — wire the flag to it. |
| Accuracy / evasion from moves not in the setup list (Flash, Kinesis, Minimize …) | already applied when the stage is set in the battle summary | no calc change; make sure every acc/eva move has an `effects[]` entry (verified). |
| Burn / paralysis / poison status | Gen 1 `HalveAttackDueToBurn` halves in-battle Attack (crit stats ignore it); gen 2 `CalcPlayerStats` PRZ/BRN after badges; gen 3/4 `damage /= 2` for burn unless Guts (physical only), Facade ×2, Guts ×1.5, Marvel Scale ×1.5, Quick Feet, Flame Orb / Toxic Orb. | Add `status` (None/Burn/Paralysis/Poison/Sleep) to `EnemyPkmn` or a per-matchup dropdown; apply burn as `atk/2` (gen 1/2 on the battle stat, gen 3/4 on the damage after `/50`) and the ability boosts in the modifier chains. |
| Substitute, Endure, Focus Band / Focus Sash, False Swipe survive | leave 1 HP / block damage | already partially modelled (False Swipe cap in gen 2); a "target at 1 HP minimum" flag in `find_kill` would cover Endure / Sash. |
| Multi-hit hit-count weighting | 2:3/8, 3:3/8, 4:1/8, 5:1/8 (gen 1 `TwoToFiveAttacksEffect`, gen 2 `EndLoop`, gen 3 `Cmd_setmultihitcounter`, gen 4 `SetMultiHit`; Skill Link 5) | Optional: a "Random" dropdown value that mixes the four `HitModel`s with those weights in `find_kill_hits` (weighted sum of the per-use distributions). |
| Present's heal branch (20%) | `PresentPower` / `Cmd_presentdamagecalculation`: 40/80/120 with 40/30/10% and a 20% heal | Optional: a "Random" option weighting the three damage distributions 0.4/0.3/0.1 and 0.2 zero damage. |
| Beat Up party | Gens 2–4 hit once per healthy party member with **that member's** base Attack (gen 2: and level). | The app tracks the solo Pokémon only. For enemy trainers the party is in the trainer database: pass the trainer's `pkmn` list to the calc (a `party: &[EnemyPkmn]` field on `DamageArgs`) and build the `HitModel` per member; for the player keep the "N hits" dropdown or add party inputs. |
| Counter / Mirror Coat / Bide / Metal Burst | 2× (×1.5) the damage taken | now dropdown-driven; an exact model would read the enemy's damage ranges from the same matchup (feed the opponent's `DamageRange` into a `HitModel`). |

### 8.2 Gen 1

| Mechanic | Game rule (pokeyellow) | Implementation |
|---|---|---|
| X Accuracy | `MoveHitTest`: bypasses the roll (`USING_X_ACCURACY`) | field toggle → `get_move_accuracy` returns `None`. |
| Dig / Fly invulnerability, Dream Eater's sleep requirement, Rage's stat-up on hit | turn semantics | out of scope for a per-turn calc; Rage is expressed through the Attack stage dropdown. |
| Confusion self-hit | `HandleSelfConfusionDamage`: 40 power, typeless, own Attack vs own Defense, no crit / roll | optional pseudo-move. |

### 8.3 Gen 2

| Mechanic | Game rule (pokecrystal) | Implementation |
|---|---|---|
| Foresight | `Stab` stops at the `-2` marker (Normal/Fighting hit Ghost); `CheckHit` ignores evasion above the user's accuracy | field toggle on the defender; skip the Ghost rows and clamp evasion. |
| Lock-On / Mind Reader | `CheckHit.LockOn` always hits | field toggle → accuracy `None`. |
| X Accuracy | `CheckHit.XAccuracy` | as gen 1. |
| Beat Up for a wild user | `.wild → EnemyAttackDamage`: one hit with the user's real stats | when the attacker is a wild mon, compute one hit with `attacking_stat` instead of base stats (needs `is_wild_battle && attacker_is_enemy`, both now on `DamageArgs`). |

### 8.4 Gen 3

| Mechanic | Game rule (pokeemerald) | Implementation |
|---|---|---|
| Flash Fire boost | `RESOURCE_FLAG_FLASH_FIRE` → Fire damage ×15/10 after weather (`pokemon.c`) | field toggle "Flash Fire active" on the attacker; multiply after the weather step (the position is already marked in `gen3.rs`). |
| Guts / Marvel Scale / Overgrow-Blaze-Torrent-Swarm | Atk ×150/100 with any status; Def ×150/100 with status; power ×150/100 at `hp <= maxHP/3` | status input (above) and an "HP ≤ ⅓" toggle (or reuse the Eruption-style HP dropdown as the user's HP). |
| Plus / Minus, Helping Hand, Charge | doubles only / `dmgMultiplier` | doubles field toggles. |
| Mud Sport / Water Sport | `gBattleMovePower /= 2` for Electric / Fire while active | field toggle (the moves exist in the setup list as status moves). |
| Lock-On, Foresight | `AccuracyCalcHelper`, `STATUS2_FORESIGHT` | as gen 2. |

### 8.5 Gen 4

| Mechanic | Game rule (pokeplatinum) | Implementation |
|---|---|---|
| Flash Fire boost, Guts, Marvel Scale, Overgrow family, Plus/Minus, Charge, Helping Hand, Sports | as gen 3 (`CalcMoveDamage` order: Charge ×2 and Helping Hand ×15/10 before Technician; Sports and the HP abilities after Marvel Scale) | as gen 3, at the marked positions of the power chain. |
| Rivalry | power ×125/100 same gender, ×75/100 opposite, genderless nothing | needs genders (species gender ratio is in the gen 4 `pokemon.json`; the mon's gender is not tracked). |
| Simple / Unaware | stages doubled / ignored (`CalcMoveDamage`, accuracy) | apply to the stage values before `modify_stat_by_stage`. |
| Mold Breaker | `Battler_IgnorableAbility`: defender's Levitate, Thick Fat, Heatproof, Dry Skin, Marvel Scale, Filter, Solid Rock, Wonder Guard, Battle/Shell Armor, Sand Veil, Snow Cloak, Tangled Feet, Sturdy, Simple, Unaware, Flower Gift ignored | treat the defender's ability as "" when the attacker has Mold Breaker (one line at the top of `gen4::calculate_damage_impl`). |
| Skill Link | always 5 hits | force the hit count when the attacker has it. |
| Tangled Feet | accuracy ×50/100 while the target is confused | status input. |
| Me First, Metronome (item), Zoom Lens, Micle Berry | ×15/10; ×(10+n)/10; ×120/100 if slower; ×120/100 once | turn-order / consumable inputs; Metronome could be a "consecutive uses" dropdown. |
| Type-resist berries (Occa … Chilan) | halve the applied SE damage once (`subscript_type_resist_berry`) | halve the range when the defender holds the matching berry and the net result is SE (consumed: only the first hit). |
| Lucky Chant, Endure, Focus Sash / Band, Ingrain, Iron Ball speed halving | crit block / survive / grounding / speed | field toggles; Ingrain grounds like Iron Ball (`BasicTypeMulApplies`). |
| Fake Out / Sucker Punch / Last Resort / Feint / Focus Punch / Snore / Dream Eater conditions | move fails outside its condition | damage is the vanilla value when the move succeeds; the condition is the router's call. |

---

## 9. Data changes

`raw_pkmn_data/gen_one/moves.json`: Struggle accuracy `null → 100`
(`RECOIL_EFFECT` takes the normal roll; only `SWIFT_EFFECT` bypasses it);
Psychic side-effect chance `33 → 33.2`; Fire Punch / Ice Punch / ThunderPunch
/ Body Slam effect key `percent → chance`; PP: Swords Dance 30, Jump Kick 25,
Submission 25, Growth 40, Petal Dance 20, Struggle 10 (`data/moves/moves.asm`).

`raw_pkmn_data/gen_two/moves.json`: AncientPower chance `20 → 10`; Spikes
`30 → 100`; Flame Wheel `10% Burn`, Snore `30% flinch`, Twister `20% flinch`
added; Swagger `Confuse → Confusion`; Tri Attack `6.3 → 6.64`; Fly / Dig
flavor spelling → `two_turn_semi_invulnerable`; Hidden Power power `0 → 1`
(the ROM byte; gates the crit roll).

`raw_pkmn_data/gen_three/moves.json`: Bounce `30% flinch → 30% Paralysis`
(`BattleScript_SecondTurnSemiInvulnerable`); Tri Attack `6.3 → 6.67`; SonicBoom
power `1 → 20` (consistency with Dragon Rage); Surf / Whirlpool `bonus_dive_damage`
flavor (informational; the calc keys on the names).
`raw_pkmn_data/gen_three/{emerald,ruby_sapphire,firered_leafgreen}/pokemon.json`:
`weight` (kg) for all 389 species from `pokedex_entries.h`, loaded by
`loaders.rs` (optional key).

`raw_pkmn_data/gen_four/moves.json`: Memento accuracy `null → 100`; Fake Out
`effect_chance 100` + `100% flinch`; Acid stat `def → spd`; Tri Attack
`6.3 → 6.67`; the 22 secondary-effect entries of §6; flavors added:
`always_hit` (Aura Sphere, Magnet Bomb), `trap` (Magma Storm), `drain_hp`
(Drain Punch), `recharge` (Giga Impact, Rock Wrecker, Roar of Time),
`weight_damage` (Grass Knot), `revenge` (Avalanche), `two_turn_semi_invulnerable`
(Shadow Force), `bonus_dive_damage` (Surf, Whirlpool); removed:
`bonus_minimize_damage` from Needle Arm, Astonish, Extrasensory (plain
`FLINCH_HIT` in gen 4).

`rust/crates/xpr-data/src/gen_consts.rs`: Cover / Plume Fossil removed from
`GEN4_FLING_POWER`; new dropdowns — Counter / Mirror Coat / Bide (gens 2–4),
Metal Burst (gen 4), Beat Up (gens 2, 4); `gen4_nature_power_move`;
`GEN1_STAGE_RATIOS`, `ACCURACY_STAGE_RATIOS`.

`docs/rust_port/golden/damage_cases.json`: the one Python case that asserted
"gen 2 Beat Up → no damage" now records the implemented result (noted in the
entry). No other recorded case changed.

Left as-is on purpose: Struggle `type: "none"` in gens 2/3 (the calc's
"skip `stab`/`typecalc`" model), Curse's type (status move), the `-1`/`0`/`1`
placeholder powers of formula moves (all handled by name/flavor), Belly Drum's
`modifier: 13`.

---

## 10. API changes and how to re-run

- `DamageArgs` gained `is_wild_battle` (Ruby/Sapphire badge rule); the
  battle summary sets it.
- `get_crit_rate(gen, &DamageArgs)` and `get_move_accuracy(gen, &DamageArgs)`
  replace the positional signatures (they need the defender, stages and
  fields).
- `hit_model(gen, &DamageArgs) -> Option<HitModel>` and
  `find_kill_hits(...)` (see §3.5); `battle_summary::recalculate_single_move`
  uses them for multi-hit uses.
- Sweep / regression tooling: `reference_2026_09_23/README.md`. The golden
  replay test has a rewrite mode (`XPR_DAMAGE_CASES_REWRITE=<path>`) that
  writes the corpus with the current results for differing cases and prints
  them, for deliberate changes.

## 11. Files changed

`rust/crates/xpr-calc/src/{lib,damage,gen1,gen2,gen3,gen4,gen5,battle_summary}.rs`,
`rust/crates/xpr-calc/examples/sweep.rs` (new), `rust/crates/xpr-calc/tests/{damage_cases,ohko,game_formulas}.rs`,
`rust/crates/xpr-data/src/{gen_consts,loaders}.rs`, `raw_pkmn_data/gen_{one,two,three,four}/moves.json`,
`raw_pkmn_data/gen_three/*/pokemon.json`, `docs/rust_port/golden/damage_cases.json`,
`docs/rust_port/KNOWN_ISSUES.md`, `rust/PORT_STATUS.md`, `docs/damage_calc_review/{README.md, reference_2026_09_23/, data_checks_2026_09_23/}`.
