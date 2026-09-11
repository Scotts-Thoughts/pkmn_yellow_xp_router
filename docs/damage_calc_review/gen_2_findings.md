# Gen 2 damage-calc audit

Scope: the app's Generation 2 (Gold / Silver / Crystal) damage calculation, verified line-by-line against
pokecrystal and diffed against pokegold. The app uses ONE `moves.json` and ONE calculator for all three
versions; every Gold/Silver vs Crystal difference that affects damage is called out explicitly (section 1.15).

Reference implementation of the asm formula used to compute every number in this report:
`<scratchpad>/gen2_ref.py`; the side-by-side run against the app is `<scratchpad>/gen2_compare.py`
(run with `C:\Users\scott\AppData\Local\Python\pythoncore-3.14-64\python.exe`, the interpreter that has
the project's deps). Output of that run is quoted in section 3.

---

## 0. Sources consulted

### pokecrystal (`A:\Cygwin\home\scott\pokecrystal`)
- `engine/battle/effect_commands.asm`
  - `BattleCommand_Critical` 1120-1210 (+ includes `data/moves/critical_hit_moves.asm`, `data/battle/critical_hit_chances.asm`)
  - `BattleCommand_Stab` 1214-1391 (weather farcall 1246, badge-type farcall 1253, `.stab` 1268-1287, `.SkipStab`/`.TypesLoop` 1290-1380, `.end` 1382-1391); `BattleCheckTypeMatchup`/`CheckTypeMatchup` 1393-1471; `BattleCommand_ResetTypeMatchup` 1473-1490
  - `BattleCommand_DamageVariation` 1496-1544
  - `BattleCommand_CheckHit` 1546-1866 (`.BrightPowder` 1580-1596, random compare 1598-1604, `.Miss` 1606-1617, `.DreamEater` 1619-1629, `.Protect` 1631-1650, `.LockOn` 1652-1676, `.DrainSub` 1678-1694, `.FlyDigMoves` 1696-1723, `.ThunderRain` 1725-1733, `.XAccuracy` 1735-1739, `.StatModifiers` 1741-1826); include `data/battle/accuracy_multipliers.asm`
  - `BattleCommand_FailureText` 2065-2105; `BattleCommand_ApplyDamage` 2107-2190; `GetFailureResultText` 2192-2250 (Jump Kick crash 1/8)
  - `BattleCommand_CriticalText` 2275-2300; `BattleCommand_StartLoop` 2305-2314
  - `BattleCommand_BuildOpponentRage` 2422-2449; `BattleCommand_RageDamage` 2451-2475
  - `DittoMetalPowder` 2488-2523; `BattleCommand_DamageStats` / `PlayerAttackDamage` 2525-2612; `TruncateHL_BC` 2614-2658; `CheckDamageStatsCritical` 2660-2695; `ThickClubBoost` 2697-2708; `LightBallBoost` 2720-2731; `SpeciesItemBoost` 2733-2766; `EnemyAttackDamage` 2768-2855
  - `BattleCommand_ClearMissDamage` 2857-2862; `HitSelfInConfusion` 2864-2898
  - `BattleCommand_DamageCalc` 2900-3131 (Explosion halving 2907-2913, zero-power exit 2916-2924, min def 2927-2931, level*2 2939-2945, /5 2948-2953, +2 2956-2957, *bp 2960-2962, *atk 2965-2966, /def 2969-2971, /50 2974-2976, item boost 2979-3007, crit 3010/`.CriticalMultiplier` 3108-3129, 997 cap 3013-3090, +2 3093-3099); include `data/types/type_boost_items.asm`
  - `BattleCommand_ConstantDamage` 3133-3275 (level damage 3140-3146, Psywave 3163-3173, Super Fang 3175-3195, Flail/Reversal 3203-3275); include `data/moves/flail_reversal_power.asm`
  - `CalcPlayerStats` 4778-4795, `CalcEnemyStats` 4799-4815, `CalcBattleStats` 4817-4886
  - `BattleCommand_EndLoop` 5203-5330 (multi-hit count rolls 5222-5290, `.loop_back_to_critical` 5318-5330)
  - `BattleCommand_OHKO` 5420-5462
  - `BattleCommand_DoubleFlyingDamage` 5962-5967, `BattleCommand_DoubleUndergroundDamage` 5969-5975, `DoubleDamage` 5977-5986; `BattleCommand_DoubleMinimizeDamage` 6515-6533
  - `SkipToBattleCommand` 6721-6736
- `engine/battle/misc.asm`: `DoWeatherModifiers` 52-145; `DoBadgeTypeBoosts` 147-215
- `engine/battle/core.asm`: `InitBattleMon` stat copy + `BadgeStatBoosts` call 3899-3907; `ApplyStatLevelMultiplierOnAllStats` 6671-6680; `ApplyStatLevelMultiplier` 6682-6764; `BadgeStatBoosts` 6768-6824; `BoostStat` 6826-6855; level-up recalc order 7268-7281
- `engine/battle/hidden_power.asm` 1-108
- `engine/battle/move_effects/`: `magnitude.asm` 1-27, `fury_cutter.asm` 1-53, `rollout.asm` 1-91, `triple_kick.asm` 1-30, `pursuit.asm` 1-23, `present.asm` 1-87, `return.asm` 1-25, `frustration.asm` 1-26, `beat_up.asm` 1-220, `counter.asm` 1-57, `mirror_coat.asm` 1-58, `bide.asm` 1-96, `thief.asm` 1-110, `false_swipe.asm` 1-46, `rage.asm` 1-5, `future_sight.asm` 1-77, `selfdestruct.asm` 1-29, `snore.asm` 1-10, `sleep_talk.asm` 1-141, `thunder.asm` 1-16
- `engine/pokemon/move_mon.asm`: `CalcMonStats` 1402-1422, `CalcMonStatC` 1424-1612; `engine/math/get_square_root.asm` 1-30
- `data/moves/effects.asm` (full file; script line refs: NormalHit 5-23, Selfdestruct 139-158, Bide 795-809, Rampage 811-832, MultiHit 842-866, PoisonMultiHit 868-894, OHKOHit 917-930, Rage 1111-1131, TrapTarget 1216-1235, SuperFang/Psywave/StaticDamage 1237-1252, Reversal 1254-1268, Counter 1270-1281, Snore 1299-1320, FalseSwipe 1374-1393, TripleKick 1402-1428, Thief 1430-1450, Rollout 1538-1558, FuryCutter 1578-1597, Return 1607-1626, Present 1628-1647, Frustration 1649-1668, Magnitude 1699-1719, Pursuit 1728-1747, HiddenPower 1791-1809, MirrorCoat 1849-1860, Twister 1887-1907, Earthquake 1909-1928, FutureSight 1930-1945, Gust 1947-1965, Stomp 1967-1987, Solarbeam 1989-2010, Thunder 2012-2032, BeatUp 2041-2066)
- `data/moves/moves.asm` 17-267 (every move's effect/power/type/accuracy/pp/effect chance)
- `data/moves/critical_hit_moves.asm`, `data/moves/flail_reversal_power.asm`, `data/moves/magnitude_power.asm`, `data/moves/present_power.asm`
- `data/battle/critical_hit_chances.asm`, `data/battle/stat_multipliers.asm`, `data/battle/weather_modifiers.asm`, `data/battle/accuracy_multipliers.asm`
- `data/types/type_matchups.asm` 1-118, `data/types/badge_type_boosts.asm`, `data/types/type_boost_items.asm`
- `data/items/attributes.asm` (HELD_*_BOOST parameter = 10, e.g. lines 162-351; HELD_BRIGHTPOWDER 20 line 16; HELD_FOCUS_BAND 30 line 248; HELD_CRITICAL_UP line 290)
- `constants/battle_constants.asm` (MAX_STAT_LEVEL 13 l.11; SUPER_EFFECTIVE 20 / MORE_EFFECTIVE 15 / EFFECTIVE 10 / NOT_VERY_EFFECTIVE 5 / NO_EFFECT 0 l.21-25; STAT_MIN_NORMAL 5 / STAT_MIN_HP 10 l.75-76; MAX_STAT_VALUE 999 l.78); `constants/type_constants.asm` (type ids); `constants/gfx_constants.asm` l.12-14 (HP_BAR_LENGTH_PX = 48); `macros/data.asm` l.23-26 (`percent` = `* $ff / 100`, `out_of` = `* $100 /`); `ram/hram.asm` 69-84 (math buffer union)
- `docs/bugs_and_glitches.md` sections: Thick Club/Light Ball wrap (172), Metal Powder (198), Reflect/Light Screen wrap (285), Confusion damage (412), Beat Up one-mon (673), Present link (722), Return/Frustration zero power (751), Glacier badge SpDef (1255)

### pokegold (`A:\Cygwin\home\scott\pokegold`) — diffed against the above
- `engine/battle/effect_commands.asm`: Critical+Stab (1123-1300) identical; DamageVariation/CheckHit (1503-1960) identical apart from comments/battle-scene check; RageDamage..EnemyAttackDamage (2463-2900) identical EXCEPT `TruncateHL_BC` 2625-2656 (no re-loop, see 1.15); DamageCalc/ConstantDamage (2897-3300) identical
- `engine/battle/move_effects/present.asm` 1-31 (no push/pop around the Stab call, see 1.15); all other move_effects files listed above identical (or comment/label-only diffs)
- `data/types/type_matchups.asm`, `data/battle/*.asm`, `data/moves/*.asm` (moves, effects, crit list, flail/magnitude/present tables), `data/types/*.asm`, `engine/battle/hidden_power.asm` (`add a` vs `sla a`, same result): identical
- `engine/battle/core.asm` `BadgeStatBoosts` / `ApplyStatLevelMultiplier`: identical logic (gold lacks the Battle Tower early-out); `engine/battle/misc.asm` weather/badge routines: identical logic

### App (`A:\pkmn_yellow_xp_router`)
- `pkmn/gen_2/pkmn_damage_calc.py` (whole file, 1-366)
- `pkmn/gen_2/data_objects.py` (whole file; stat calc 224-423, hidden power 497-534)
- `pkmn/gen_2/gen_two_constants.py` (40-122), `pkmn/gen_2/gen_two_object.py` (174-217)
- `pkmn/damage_calc.py` (get_special_damage_override 29-55, DamageRange 108-204)
- `pkmn/universal_data_objects.py` (StageModifiers 100-160, EnemyPkmn 336-413, FieldStatus 521-550)
- `pkmn/pkmn_db.py` (MoveDB 235+), `raw_pkmn_data/gen_two/moves.json`, `raw_pkmn_data/gen_two/type_info.json`, `utils/constants.py`
- `controllers/battle_summary_controller.py` 760-1000

---

## 1. Core formula verification

Notation: `L` = attacker level, `bp` = base power, `A`/`D` = the 8-bit truncated attack/defense that reach
`BattleCommand_DamageCalc` (register b / c), `X` = running damage. All divisions are integer floor divisions
(`Divide` is an integer routine); nothing in this pipeline rounds to nearest.

### 1.1 Command order (NormalHit script, `data/moves/effects.asm:5-23`)

```
critical -> damagestats -> damagecalc -> stab -> damagevariation -> checkhit -> ... -> applydamage
```
Game order of the arithmetic (with app line refs for the same step):

| # | Game step (citation) | App step (pkmn_damage_calc.py) | Verdict |
|---|---|---|---|
| 1 | Crit roll (`BattleCommand_Critical` 1120-1210) | `get_crit_rate` 19-22 (chance only) | see 1.3 |
| 2 | Pick stats: stage-modified, badge-boosted battle stats (`PlayerAttackDamage` 2538-2586) | `get_battle_stats` 100-103 | CORRECT |
| 3 | Reflect / Light Screen: `sla c; rl b` = defence x2 (2549-2552, 2571-2574) — BEFORE the crit rule and BEFORE truncation | 113-129 (`doubled_def`) | INCORRECT on crits (bug 3.2) |
| 4 | Crit rule (`CheckDamageStatsCritical` 2660-2695): if crit AND defender's def stage >= attacker's atk stage, reload BOTH stats from `wPlayerStats`/`wEnemyStats` (no stages, no badge boosts) and this reload DISCARDS the screen doubling | 84-98 | CORRECT for the stage rule; screen handling INCORRECT (bug 3.2) |
| 5 | Thick Club (Cubone OR Marowak) / Light Ball (Pikachu): `sla l; rl h` = attack x2, 16-bit (2697-2766) | 105-108 | Cubone missing (bug 3.6) |
| 6 | `TruncateHL_BC` (2614-2658): while (atk >= 256 or def >= 256): atk //= 4, def //= 4, each min 1 (Crystal loops; G/S one pass then low byte) | nothing | MISSING (bug 3.1) |
| 7 | Metal Powder on a Ditto DEFENDER, physical OR special, applied to the 8-bit def (`DittoMetalPowder` 2488-2523) | 110-111 (physical only, un-truncated) | INCORRECT (bug 3.7) |
| 8 | Explosion/Selfdestruct: `D = max(D >> 1, 1)` on the 8-bit value (2907-2913) | 131-132 | CORRECT modulo truncation |
| 9 | `bp == 0` -> no damage (2916-2924); `D = max(D,1)` (2927-2931) | 57-58 | CORRECT |
| 10 | `X = (2L // 5) + 2` (2939-2957) | 214-215 | CORRECT |
| 11 | `X = X * bp; X = X * A; X = X // D; X = X // 50` (2960-2976) | 217-221 | CORRECT (order and floors match) |
| 12 | Type-boost item: `X = X * (100 + 10) // 100` if item's type == move type (2979-3007; `data/items/attributes.asm` param 10) | 223-224 `floor(X*1.1)` | CORRECT (verified `floor(x*1.1) == x*110//100` for all x < 3000) |
| 13 | Crit: `X = min(2X, 65535)` (3108-3129) | 226-228 | CORRECT |
| 14 | `X = min(X, 997)` (3013-3090) then `X += 2` (3093-3099) | 230 (`+2` only) | 997 cap MISSING (bug 3.12) |
| 15 | `stab` command: skipped entirely for Struggle (1214-1218) | 134-138 (type "none") | CORRECT except Pink Bow (bug 3.14) |
| 16 | Weather (`DoWeatherModifiers`): `X = max(X*15//10, 1)` or `max(X*5//10, 1)`, cap 65535 (misc.asm 52-145) | 232-248 | CORRECT |
| 17 | Badge type boost (`DoBadgeTypeBoosts`): `X = min(X + max(X >> 3, 1), 65535)`, player only, not in link (misc.asm 147-215) | 250-251 `floor(X*1.125)` | INCORRECT when X < 8 (bug 3.9) |
| 18 | STAB: `X = X + (X >> 1)` (1268-1287) | 253-257 | CORRECT |
| 19 | Type effectiveness in table order, per matching entry `X = max(X*m//10, 1)` (m = 20 / 5) or 0 = immune (1290-1380) | 264-270 | CORRECT (see 1.8 for ordering proof; min-1 nuance) |
| 20 | Move multipliers: Rollout / Fury Cutter / Rage are applied AFTER stab and BEFORE damagevariation (effects.asm 1538-1558, 1578-1597, 1111-1131); Triple Kick is applied BETWEEN damagecalc and stab (1402-1428) | 272-289 (all after type, before random) | Triple Kick order INCORRECT (bug 3.11); Fury Cutter cap INCORRECT (bug 3.5) |
| 21 | `damagevariation`: if X >= 2: `X = X * r // 255`, r uniform in 217..255 (1496-1544) | 331-337 | CORRECT |
| 22 | Double-damage flags (Gust/Twister vs Fly, Earthquake/Magnitude vs Dig, Stomp vs Minimize, Pursuit vs switch): `X = min(2X, 65535)` AFTER damagevariation (effects.asm 1887-1987, 1699-1719, 1728-1747) | 291-305 (before random) | INCORRECT ordering (bug 3.10, distribution only) |

### 1.2 Base damage & truncation — INCORRECT (missing `TruncateHL_BC`)
`TruncateHL_BC` (effect_commands.asm 2614-2658) is called from both `PlayerAttackDamage` (2604) and
`EnemyAttackDamage` (2847) after the screen doubling and the species-item doubling:

```
.loop: if (h | b) == 0 -> .finish            ; both 16-bit stats < 256?
       bc >>= 2 (min 1); hl >>= 2 (min 1)
.finish: (not LINK_COLOSSEUM) if (h | b) != 0 -> .loop   ; Crystal only
       b = l                                 ; b = attack (8-bit), c = defense (8-bit)
```
So whenever EITHER stat is >= 256, BOTH are divided by 4 (floored, min 1). With Reflect/Light Screen the
doubled defence is what is tested, so any defence >= 128 behind a screen triggers it. The app never does
this (`pkmn_damage_calc.py:213-221` uses the full 16-bit stats). See bug 3.1 for numbers.

### 1.3 Crit rate — `get_crit_rate` PARTIALLY CORRECT
`BattleCommand_Critical` (1120-1210): stage c = 0; Chansey+Lucky Punch or Farfetch'd+Stick: +2; Focus
Energy: +1; move in `CriticalHitMoves`: +2; Scope Lens (`HELD_CRITICAL_UP`): +1. Then crit iff
`BattleRandom < CriticalHitChances[c]`. `data/battle/critical_hit_chances.asm` with `out_of` = `*256/`:

| stage | byte | probability |
|---|---|---|
| 0 | 256//15 = 17 | 17/256 = 6.64% |
| +1 | 32 | 12.5% |
| +2 | 64 | 25% |
| +3 | 85 | 33.2% |
| +4,+5,+6 | 128 | 50% |

`CriticalHitMoves` = Karate Chop, Razor Wind, Razor Leaf, Crabhammer, Slash, Aeroblast, Cross Chop.
App (`pkmn_damage_calc.py:19-22`): 1/4 with `high_crit`, else 17/256 — CORRECT for stages 0 and +2.
INCORRECT: Aeroblast lacks the `high_crit` flavor in moves.json (bug 3.8). MISSING: Focus Energy (+1),
Scope Lens (+1), Lucky Punch / Stick (+2) — see 4.10. Note `ret z` at 1127: a move with power 0 never crits.

### 1.4 Crit damage and stat-stage handling — CORRECT rule, INCORRECT screen interaction
- Multiplier: x2 on the post-item, pre-`+2` value, capped at 65535 (3108-3129). App 226-228 CORRECT.
- Stage rule (`CheckDamageStatsCritical` 2660-2695): compares the defender's Def (SpDef for special moves)
  stage LEVEL with the attacker's Atk (SpAtk) level; `cp b` sets carry iff `defLevel < atkLevel`. Carry =>
  keep the boosted battle stats (stages + badges + screens). No carry (crit and `defStage >= atkStage`) =>
  reload `wPlayerAttack`/`wEnemyDefense` etc. from `wPlayerStats`/`wEnemyStats`, which are the raw party
  stats copied at send-out BEFORE `BadgeStatBoosts` (core.asm 3899-3907, 7268-7281). So the "unboosted"
  stats have no stages AND no badge boosts, for both sides. App 84-98 implements exactly `atk <= def` =>
  zero both sides' stages and drop badge boosts. CORRECT ("<=" verified).
- Screens: the doubling happens at 2549-2552 / 2571-2574 BEFORE `CheckDamageStatsCritical`; the reload
  only happens on the unfavourable branch. Therefore a crit with `atkStage > defStage` STILL has Reflect /
  Light Screen applied. App 116-126 drops screens on every crit. INCORRECT (bug 3.2).
- Crits never occur for Flail/Reversal/Future Sight (their scripts have no `critical` command;
  `wCriticalHit` is zeroed at battle start `start_battle.asm:163`, by `BattleCommand_Critical`, by
  `CriticalText` 2295 and by `GetFailureResultText` 2218, so it is always 0 when they run). App 85-86,
  227 CORRECT.

### 1.5 Stat calculation & stage multipliers — CORRECT (one 1-point corner case)
- `CalcMonStatC` (move_mon.asm 1424-1612): `stat = ((base + DV) * 2 + (sqrt_ceil(statexp) >> 2)) * L // 100 + 5`
  (HP: `+ L + 10`), capped 999. `GetSquareRoot` returns the first b with `b*b >= statexp`, capped at 255
  (`NUM_SQUARE_ROOTS` 255): for statexp in 65026..65535 the game uses 255 where the app's
  `math.ceil(math.sqrt())` (`data_objects.py:404`) gives 256 -> stat term 63 vs 64 (at most +1 to the
  final stat, only at maxed stat exp). Otherwise `calc_unboosted_stat` 402-410 is CORRECT.
- Stage multipliers `data/battle/stat_multipliers.asm` = app `STAGE_MOFIDIERS` (data_objects.py 18-32)
  entry for entry. Applied as `stat * num // den`, min 1, cap 999 (`CalcBattleStats` 4817-4886,
  `ApplyStatLevelMultiplier` 6682-6764). App `modify_stat_by_stage` 387-399 + `_clamp_stat` CORRECT.
- Order: `CalcPlayerStats` (4778-4795) = stage multipliers from the raw party stats, THEN `BadgeStatBoosts`,
  then PRZ/BRN. App `calc_battle_stat` 371-384 (stage then badge) CORRECT.

### 1.6 Badge stat boosts — CORRECT (including the Glacier/SpDef glitch)
`BadgeStatBoosts` (core.asm 6768-6824): Zephyr -> Attack, Mineral -> Defense, Plain -> Speed (bits 2/4
swapped at 6785-6797), Glacier -> Special Attack; `BoostStat` (6826-6855) = `stat + (stat >> 3)`, cap 999.
App `GenTwoBadgeList.is_*_boosted` 106-119 and `badge_boost_single_stat` 421-423 (`floor(x*1.125)` ==
`x + x>>3`, verified) CORRECT. Not applied in link battles / Battle Tower, and never to the enemy (app:
enemy badges = None) CORRECT.

SpDef glitch (6817-6823 + doc l.1255): after the loop, `srl a; call c, BoostStat` on SpDef, but `a` was
clobbered by `BoostStat`(SpAtk) and equals `(H - 3 - borrow) & 0xFF` where H/L are the high/low bytes of
the boosted SpAtk and borrow = (L < 231). Working this through: SpDef is boosted iff the unboosted
(stage-modified) SpAtk S satisfies 206 <= S <= 432 or S >= 661. App `should_ignore_spd_badge_boost`
(240-246): ignore if 0..205 or 433..660 — exactly the complement. CORRECT. The check is on the SpAtk
value present in `wBattleMonSpclAtk` when `BadgeStatBoosts` runs, i.e. after stage multipliers
(`CalcPlayerStats` order), which is what `calc_battle_stats` 340-347 feeds it. CORRECT.

### 1.7 Held-item type boost — CORRECT
`DamageCalc` 2979-3007: after `/50`, if `GetUserItem`'s effect matches a `TypeBoostItems` row whose type
== `BATTLE_VARS_MOVE_TYPE`: `X = X * (100 + c) // 100`, c = 10 for every boost item
(`data/items/attributes.asm`). Position (after /50, before crit) and rounding match app 223-224.
`type_info.json` `held_item_boosts` (18 entries incl. Pink Bow + Polkadot Bow -> Normal, Dragon Scale ->
Dragon — the game really uses Dragon Scale, doc l.800) matches `data/types/type_boost_items.asm`.
Note: the check uses the move type directly, so Struggle (type NORMAL in `moves.asm:181`) IS boosted by
Pink Bow / Polkadot Bow even though `stab` skips it (bug 3.14).

### 1.8 STAB and type effectiveness — CORRECT (order verified), tiny min-1 nuance
- STAB (1268-1287): `X += X >> 1`. App 253-257 CORRECT.
- Type loop (1290-1380): walks `TypeMatchups` top to bottom; each row whose attacker type == move type and
  defender type == defender type1 OR type2 multiplies `X = X * m // 10` (m = 20 SE, 5 NVE), forcing 1 if
  the quotient is 0; m = 0 sets damage 0 and `wAttackMissed`. A mono-type defender (type1 == type2)
  matches each row once. Rows after the `-2` marker (Normal/Ghost, Fighting/Ghost immunities) are skipped
  when the target is Foresight-identified.
- Order proof: I compared the per-attacking-type row order of `type_matchups.asm` with the dict order of
  `type_info.json` for all 17 types. They match exactly except Grass, where the JSON lists Steel before
  Dragon while the table has Dragon then Steel — both are NVE, so the result is identical. Since a dual
  SE+NVE matchup is the only case where order matters (e.g. Fire vs Grass/Rock: 20 then 5), and every such
  pair is in the same order in both, app 264-270 is CORRECT.
- Min-1 nuance: the game clamps to 1 after EACH NVE step; the app only clamps at the end (307-309). They
  differ only when X reaches 1 before a later SE row, which requires X = 1 before typing — possible only
  via rain halving a 2 (Fire move in rain, e.g. Fire vs Rock/Ice-like combos). Negligible.
- Foresight is not modelled by the app (4.11).

### 1.9 Weather — CORRECT (Thunder accuracy in sun INCORRECT)
`WeatherTypeModifiers`: Rain+Water x15/10, Rain+Fire x5/10, Sun+Fire x15/10, Sun+Water x5/10;
`WeatherMoveModifiers`: Rain + `EFFECT_SOLARBEAM` x5/10. Sandstorm has no damage modifier. Applied inside
`stab` (so Hidden Power uses its computed type, Struggle is unaffected). App 232-248 CORRECT (SolarBeam
halved only in rain, CORRECT). Thunder: `thunderaccuracy` (`move_effects/thunder.asm`) sets the accuracy
byte to 255 in rain, `50 percent + 1` = 128 in sun; `CheckHit.ThunderRain` returns "hit" in rain before
any check. App `get_move_accuracy` (gen_two_object.py 177-182): rain -> None (always hits) CORRECT; sun
returns 70 instead of 50 INCORRECT (bug 3.13).

### 1.10 Screens — see 1.4 (doubling position CORRECT; crit interaction INCORRECT; wrap in G/S 1.15)
Doubling is 16-bit (`sla c; rl b`), so no wrap by itself (max 1998). The "wrap above 1024" bug
(doc l.285) is in `TruncateHL_BC` in G/S / Colosseum link: one /4 pass leaves 256..499, and `ld b, l` /
`c` keep only the low byte. Crystal loops, so def 302 -> 75 -> (attack /16 too).

### 1.11 Random roll — CORRECT
`DamageVariation` (1496-1544): skipped if damage < 2; `r = rotate(BattleRandom)` rejected until
`r >= 85 percent + 1` = 217; `X = X * r // 255` (`100 percent` = 255). Each r in 217..255 is equally
likely (rejection sampling of a uniform byte). App 331-337 (`MIN_RANGE 217`, `MAX_RANGE 255`,
`floor(X*r/255)`, `max(...,1)`) CORRECT.

### 1.12 Minimum damage / damage cap — cap MISSING
No explicit final min; the `+2` (3093-3099) guarantees >= 2 before stab, weather/type clamp at 1, and the
variation of 1 is skipped, so damage >= 1 unless immune. App 307-309 CORRECT. The `min(X, 997)` cap
before the `+2` (3013-3090) is not implemented (bug 3.12; only matters for overkill).

### 1.13 Double-battle spread, Explosion halving
No double battles in gen 2 (app ignores `is_double_battle`) CORRECT. Explosion/Selfdestruct
(`EFFECT_SELFDESTRUCT`, 2907-2913): `D = max(D >> 1, 1)` on the truncated 8-bit defence, after the
screen doubling. App 131-132 does the same on the un-truncated value: CORRECT modulo bug 3.1.

### 1.14 Accuracy function (`get_move_accuracy`)
`CheckHit` (1546-1866) order: Dream Eater target awake -> miss; Protect -> miss; drain move vs
Substitute -> miss; Lock-On -> hit (unless target flying and move is EQ/Fissure/Magnitude); target in
Fly/Dig -> miss unless move is Gust/Whirlwind/Thunder/Twister (vs Fly) or Earthquake/Fissure/Magnitude
(vs Dig); Thunder in rain -> hit; X Accuracy -> hit; `EFFECT_ALWAYS_HIT` (Swift, Faint Attack, Vital
Throw) -> hit; then `.StatModifiers`: `acc = move_acc_byte * AccMult[accStage] // den`, min 1, then
`* EvaMult[14 - evaStage]`, min 1, cap 255 (skipped entirely if evaStage > accStage and the target is
Foresight-identified); Bright Powder: `acc = max(acc - 20, 0)`; finally hit iff `acc == 255` or
`BattleRandom < acc`. The accuracy byte is `pct * 255 // 100`, so there is NO 1/256 miss for 100%
moves (255 -> unconditional hit), but every other listed accuracy is slightly below its nominal value:
95 -> 242/256 = 94.53%, 90 -> 229/256 = 89.45%, 85 -> 216/256 = 84.38%, 80 -> 204/256 = 79.69%,
75 -> 191/256 = 74.61%, 70 -> 178/256 = 69.53%, 55 -> 140/256 = 54.69%, 50 -> 127/256 = 49.61%,
30 -> 76/256 = 29.69%. `AccuracyLevelMultipliers` (-6..+6): 33/100, 36/100, 43/100, 50/100, 60/100,
75/100, 1/1, 133/100, 166/100, 2/1, 233/100, 133/50, 3/1.
App (gen_two_object.py 177-182): returns `move.accuracy` (percent) except Thunder+rain. Verdict: nominal
percentages (0.4-0.6% high) — acceptable; sun Thunder INCORRECT (bug 3.13); accuracy/evasion stages,
Bright Powder, OHKO accuracy formula not modelled (4.9, 4.10).

### 1.15 Gold/Silver vs Crystal differences affecting damage (from the diffs)
1. `TruncateHL_BC` (pokegold 2625-2656): ONE /4 pass, then `b = l` / `c` = low bytes. If a stat is still
   >= 256 after one pass (i.e. was >= 1024: Reflect/Light Screen on def >= 512, Thick Club/Light Ball on
   atk >= 512) it wraps modulo 256. Crystal (non-link) re-loops until both < 256. Colosseum link battles
   in Crystal keep the G/S behaviour.
2. Present (pokegold `present.asm` 1-31): `BattleCommand_Present` calls `BattleCommand_Stab` WITHOUT
   preserving bc/de. Script order is `checkhit, critical, damagestats, present, damagecalc, stab, ...`
   (effects.asm 1628-1647), so `damagecalc` receives the registers Stab left behind: `b` =
   `wTypeMatchup` (10 neutral, 5 vs Rock or Steel, 2 vs Rock/Steel, 0 vs Ghost -> Present fails),
   `c` = the USER's type-2 id, or 0 -> forced to 1 if the user has STAB (Normal-type: the `.stab` branch
   at 1268-1287 leaves `c` = low byte of `0 >> 1`), `d` = the rolled power (40/80/120), `e` = the
   TARGET's type-2 id used as the "level". Type ids: Normal 0, Fighting 1, Flying 2, Poison 3, Ground 4,
   Rock 5, Bug 7, Ghost 8, Steel 9, Fire 20, Water 21, Grass 22, Electric 23, Psychic 24, Ice 25,
   Dragon 26, Dark 27 (`constants/type_constants.asm`). The result is level- and stat-independent; STAB,
   type effectiveness, item boost, crit and the random roll still apply afterwards. Crystal pushes/pops
   (correct) except in Colosseum link mode. Numbers in 4.4.
3. Everything else in the damage pipeline (Critical, DamageStats, DamageCalc, Stab, weather, badge
   boosts, DamageVariation, CheckHit, ConstantDamage, OHKO, all data tables, all effect scripts, all move
   power/type/accuracy) is byte-for-byte equivalent between pokegold and pokecrystal.

### 1.16 moves.json data check
Scripted comparison of every move in `raw_pkmn_data/gen_two/moves.json` against `data/moves/moves.asm`
(power, type, accuracy, pp): all match (Curse's `???` type and Return's placeholder power are the only
non-matches and are intentional). Flavor check: `high_crit` is missing on Aeroblast (bug 3.8); all other
special flavors present.

---

## 2. Per-move table

Status moves (base_power 0 in moves.json: Swords Dance, Whirlwind, Sand Attack, Tail Whip, Leer, Growl,
Roar, Sing, Supersonic, Disable, Mist, Leech Seed, Growth, PoisonPowder, Stun Spore, Sleep Powder,
String Shot, Thunder Wave, Toxic, Hypnosis, Meditate, Agility, Teleport, Mimic, Screech, Double Team,
Recover, Harden, Minimize, SmokeScreen, Confuse Ray, Withdraw, Defense Curl, Barrier, Light Screen, Haze,
Reflect, Focus Energy, Metronome, Mirror Move, Amnesia, Kinesis, Softboiled, Glare, Poison Gas, Lovely
Kiss, Transform, Spore, Flash, Splash, Acid Armor, Rest, Sharpen, Conversion, Substitute, Sketch, Spider
Web, Mind Reader, Nightmare, Curse, Conversion2, Cotton Spore, Spite, Protect, Scary Face, Sweet Kiss,
Belly Drum, Spikes, Foresight, Destiny Bond, Perish Song, Detect, Lock On, Sandstorm, Endure, Charm,
Swagger, Milk Drink, Mean Look, Attract, Sleep Talk, Heal Bell, Safeguard, Pain Split, Baton Pass,
Encore, Sweet Scent, Morning Sun, Synthesis, Moonlight, Rain Dance, Sunny Day, Psych Up) were skipped;
the app correctly returns None for all of them. No status move is treated as damaging. Guillotine (power
0), Counter (0) and Bide (0) return None, which is "no damage" rather than "not implemented" — see rows.

Legend for "App status": CORRECT means the move goes through the app's vanilla path and matches the game
once the core bugs of section 3 (truncation 3.1, crit+screen 3.2, badge-type +1 3.9, 997 cap 3.12) are
fixed; those core bugs apply to every row and are not repeated per row.

| Move | App status | What the game does (cite) | What the app does (file:line) | Fix / implementation plan |
|---|---|---|---|---|
| **Plain physical/special moves (105):** Pound, Mega Punch, Pay Day, Fire Punch, Ice Punch, ThunderPunch, Scratch, Vicegrip, Cut, Wing Attack, Fly, Bind, Slam, Vine Whip, Mega Kick, Jump Kick, Rolling Kick, Headbutt, Horn Attack, Tackle, Body Slam, Wrap, Take Down, Thrash, Double Edge, Poison Sting, Bite, Acid, Ember, Flamethrower, Water Gun, Hydro Pump, Surf, Ice Beam, Blizzard, Psybeam, BubbleBeam, Aurora Beam, Hyper Beam, Peck, Drill Peck, Submission, Low Kick, Strength, Absorb, Mega Drain, SolarBeam, Petal Dance, Fire Spin, ThunderShock, Thunderbolt, Rock Throw, Dig, Confusion, Psychic, Quick Attack, Egg Bomb, Lick, Smog, Sludge, Bone Club, Fire Blast, Waterfall, Clamp, Swift, Skull Bash, Constrict, Hi Jump Kick, Dream Eater, Leech Life, Sky Attack, Bubble, Dizzy Punch, Rock Slide, Hyper Fang, Tri Attack, Thief, Flame Wheel, Snore, Powder Snow, Mach Punch, Faint Attack, Sludge Bomb, Mud Slap, Octazooka, Zap Cannon, Icy Wind, Outrage, Giga Drain, Spark, Steel Wing, Sacred Fire, DynamicPunch, Megahorn, DragonBreath, Rapid Spin, Iron Tail, Metal Claw, Vital Throw, Crunch, ExtremeSpeed, AncientPower, Shadow Ball, Rock Smash, Whirlpool | CORRECT | NormalHit-family scripts (effects.asm 5-23, 34-137, 139-158 etc.): critical, damagestats, damagecalc, stab, damagevariation. Secondary effects, recoil, drain, flinch, trap residual, recharge, two-turn charge, priority, Jump Kick crash (1/8 of the computed damage to the user, 2222-2246) do not change damage dealt. SolarBeam: x0.5 in rain (weather_modifiers.asm:9). Dream Eater / Snore: fail unless target / user asleep (CheckHit 1619-1629, snore.asm 1-10) — conditions not simulated. | Vanilla path pkmn_damage_calc.py 100-339 | Fix core bugs 3.1, 3.2, 3.9, 3.12. |
| Karate Chop, Razor Wind, Razor Leaf, Crabhammer, Slash, Cross Chop | CORRECT | `CriticalHitMoves` (+2 crit stage = 64/256) (1172-1182, critical_hit_moves.asm) | `high_crit` flavor -> 1/4 (19-22) | none |
| Aeroblast | INCORRECT (crit rate) | In `CriticalHitMoves` -> 1/4 | moves.json has no `high_crit` -> 17/256 | Add `"high_crit"` to Aeroblast's attack_flavor (bug 3.8). |
| Struggle | CORRECT (minor) | `EFFECT_RECOIL_HIT`, type NORMAL 50 bp (moves.asm:181). `stab` returns immediately (1216-1218): no STAB, no type effectiveness (hits Ghost), no weather, no badge type boost. Item boost in DamageCalc still keys on type NORMAL -> Pink Bow/Polkadot Bow x1.1. | type "none" -> no STAB/type/item (134-138) | Optional: treat Pink/Polkadot Bow as boosting Struggle (bug 3.14). |
| DoubleSlap, Comet Punch, Fury Attack, Pin Missile, Spike Cannon, Barrage, Fury Swipes, Bone Rush | CORRECT (approximation) | MultiHit script 842-866: `checkhit` once, then `critical..damagevariation` per hit (EndLoop 5318-5330 loops back to `critical`), so each hit has its own crit roll and random roll; count 2/3/4/5 with 3/8, 3/8, 1/8, 1/8 (5262-5272). A miss ends the loop (FailureText 2087-2097). | 311-324 + 340-364: N independent ranges summed; crit modelled as exactly one crit hit; the non-crit hits in the crit case are recomputed with the (possibly zeroed) stage modifiers and without `weather` | Pass the ORIGINAL stage modifiers (not the zeroed copies) and `weather` into the recursive call at 345-359; document the one-crit approximation (bug 3.15). No Fire/Water multi-hit move exists so the weather omission is harmless. |
| Double Kick, Twineedle, Bonemerang | CORRECT (approximation) | `EFFECT_DOUBLE_HIT` / `EFFECT_POISON_MULTI_HIT`: exactly 2 loops (5232-5235, 5297-5299), same per-hit rolls | `DOUBLE_HIT_FLAVOR` -> 2 hits (312-313) | as above |
| Triple Kick | INCORRECT (order + semantics) | TripleKick script 1402-1428: `damagecalc, triplekick, stab`: kick n's damage = `X * n` BEFORE STAB/type, then random; number of kicks is random 1/2/3 with 1/3 each (EndLoop 5236-5247), one accuracy check, crit per kick. | `move_modifier = int(custom)` applied AFTER type (286-289) — the dropdown value is the n-th kick's damage, not a total | Apply the multiplier between `+2` and weather/STAB; change dropdown to "1 Kick"/"2 Kicks"/"3 Kicks" = sum of kicks 1..n (each its own random roll) (bug 3.11). |
| Rollout | CORRECT | Rollout script 1538-1558: `stab, checkhit, rolloutpower, damagevariation`; `rolloutpower` doubles (count-1) times, +1 if Defense Curl; max 5 hits (rollout.asm 23-91) -> x1,2,4,8,16 (x32 with curl on hit 5) | 274-281: 2^(n-1); "5 + DefenseCurl" -> int() fails -> 6 -> x32 | Optionally add "n + DefenseCurl" entries (equal to n+1). |
| Fury Cutter | INCORRECT ("6") | fury_cutter.asm 15-20: `b = min(count, 5)`, doubled (b-1) times -> x1,2,4,8,16, capped at 16 | 282-283: 2^(n-1) -> "6" gives x32 | `move_modifier = 2 ** (min(n, 5) - 1)` (bug 3.5). |
| Rage | CORRECT | Rage script 1111-1131: `stab, checkhit, ragedamage, damagevariation`; `RageDamage` 2451-2475: X *= (1 + rage counter), counter +1 per hit taken while Rage is active (2422-2449) | 284-285: multiplier = dropdown 1..6 | none |
| Gust, Twister | CORRECT (distribution) | Gust 1947-1965 / Twister 1887-1907: `damagevariation` then `doubleflyingdamage` (x2 after the random roll, cap 65535) | 291-305 doubles BEFORE the roll | Move the doubling after the roll (bug 3.10). Note 298: empty `custom_move_data` means "bonus" — default should be no bonus. |
| Earthquake | CORRECT (distribution) | 1909-1928: same, `doubleundergrounddamage` after roll | same | same |
| Magnitude | CORRECT (distribution) | 1699-1719: `getmagnitude` picks power 10/30/50/70/90/110/150 with 5/10/20/30/20/10/5% (magnitude_power.asm), `doubleundergrounddamage` after roll | 179-193 powers CORRECT; doubling before roll | same as Gust |
| Stomp | CORRECT (distribution) | 1967-1987: `doubleminimizedamage` after roll (6515-6533) | same | same |
| Pursuit | CORRECT (distribution) | 1728-1747: `damagevariation, pursuit`: x2 after roll if target is switching (pursuit.asm) | same | same |
| Selfdestruct, Explosion | CORRECT | `EFFECT_SELFDESTRUCT`: `D = max(D>>1, 1)` on the truncated 8-bit defence (2907-2913) | 131-132 | modulo bug 3.1 |
| Hidden Power | INCORRECT (power for DV 8) | hidden_power.asm 11-62: power = `(5 * (topbit(Atk)<<3 \| topbit(Def)<<2 \| topbit(Spd)<<1 \| topbit(Spc)) + (Spc & 3)) // 2 + 31`, top bit = DV >= 8; type index = `(Atk & 3) << 2 \| (Def & 3)` mapped over Fighting, Flying, Poison, Ground, Rock, Bug, Ghost, Steel, Fire, Water, Grass, Electric, Psychic, Ice, Dragon, Dark (64-92). Physical/special decided by the computed type (103-107 -> DamageStats reads the overwritten move type); STAB, weather, items all use the computed type. Crit allowed (power byte 1). | `get_hidden_power_base_power` 523-534 uses `dv > 8` (so DV 8 counts as low); type table/index 497-520 CORRECT; calc 50-52 uses computed type CORRECT | Change all four `> 8` to `>= 8` (bug 3.4). |
| Return | CORRECT | return.asm: power = `happiness * 10 // 25` (max 102; 0..2 happiness -> power 0 -> no damage, doc l.751) | dropdown "102".."1" = power (207-211) | none |
| Frustration | NOT IMPLEMENTED | frustration.asm: power = `(255 - happiness) * 10 // 25`, otherwise identical to Return | base_power -1 falls through the formula -> reports 1 damage (57-58 don't catch -1) | Add `CUSTOM_MOVE_DATA["Frustration"] = ["102".."1"]` and treat like Return at 207 (4.2). |
| Present | NOT IMPLEMENTED (Crystal) / INCORRECT formula (G/S) | present.asm: 40% -> 40 bp, 30% -> 80, 10% -> 120, 20% heals target 1/4 max HP (fails at full HP). Crystal: normal formula with the user's stats, STAB (Normal), type, crit, random. G/S: register clobber, see 1.15 (2). | -1 -> 1 damage | Dropdown "40"/"80"/"120" (+ "Heal"); G/S-specific formula (4.4). |
| Flail, Reversal | CORRECT | Reversal script 1254-1268: `constantdamage, stab, checkhit` — no `critical`, no `damagevariation`. ConstantDamage 3203-3275: `p = (curHP * 48) // maxHP` (both >> 2 first if maxHP >= 256); power 200 if p <= 1, 150 if <= 4, 100 if <= 9, 80 if <= 16, 40 if <= 32, else 20 (flail_reversal_power.asm); then DamageStats + DamageCalc (items, +2, no crit) + STAB/type; single value. HP breakpoints: <4.17%, <10.42%, <20.83%, <35.42%, <68.75%. | 194-206 powers, 85-86/227 no crit, 327-329 single value | Dropdown labels are approximate (69/35/20/10/4 vs 68.75/35.42/20.83/10.42/4.17); optional relabel. |
| SonicBoom, Dragon Rage | CORRECT | StaticDamage 1237-1252: `constantdamage` = move power byte (20/40) then `checkhit, resettypematchup` (immune -> fails) | `get_special_damage_override` 29-55 | none |
| Seismic Toss, Night Shade | CORRECT | `EFFECT_LEVEL_DAMAGE`: damage = user level; immune -> fails | override, level | none |
| Psywave | CORRECT | 3163-3173: `b = L + L//2`; reroll while `r == 0 or r >= b` -> uniform 1..floor(1.5L)-1; vs Dark fails | 71-73 `range(1, floor(1.5L))` | none (L = 1 would raise; impossible in practice) |
| Super Fang | NOT IMPLEMENTED | 3175-3195: damage = `targetHP // 2`, min 1; vs Ghost fails | -1 -> 1 damage | Return `max(defender.cur_stats.hp // 2, 1)` (full-HP assumption) or an HP-fraction dropdown (4.5). |
| Guillotine, Horn Drill, Fissure | NOT IMPLEMENTED | OHKOHit 917-930 + `BattleCommand_OHKO` 5420-5462: fails if immune or user level < target level; accuracy byte = `min(76 + 2*(Luser - Ltarget), 255)`, then normal CheckHit (stages apply); damage 65535 (KO) | Guillotine (power 0) -> None; Horn Drill/Fissure (-1) -> 1 damage | Return `DamageRange({defender.cur_stats.hp: 1})` and make `get_move_accuracy` return `(76 + 2*dL)/256*100` or 0 (4.6). |
| Counter | NOT IMPLEMENTED (returns None) | counter.asm: 2x the last damage the user took, only if the last move was physical (type < SPECIAL), power > 0, opponent moved first; vs Ghost fails | power 0 -> None | Requires enemy damage input (4.7). |
| Mirror Coat | NOT IMPLEMENTED | mirror_coat.asm: same for special moves; vs Dark fails | -1 -> 1 damage | as Counter (4.7). |
| Bide | NOT IMPLEMENTED (returns None) | bide.asm 23-46: 2x total damage taken over 2-3 turns (`wPlayerDamageTaken`), type reset to neutral, Ghost immune | power 0 -> None | requires damage input (4.7). |
| Beat Up | NOT IMPLEMENTED (wrong numbers) | BeatUp script 2041-2066: per non-fainted, status-free party member (incl. user): `beatup` sets b = that member's BASE Attack, c = target's BASE Defense, e = member's level, d = 10 (beat_up.asm 54-79); no `TruncateHL_BC`; `damagecalc` (item boost, crit per hit, +2); NO `stab` (no STAB, no type, no weather, no badge boost); random per hit. Wild-mon user: one hit with normal stats (`.wild` -> EnemyAttackDamage; EndLoop `.only_one_beatup`). 1-mon party: one hit then fails. | vanilla 10 bp with the user's battle stats + STAB + type | 4.3 |
| Future Sight | INCORRECT | FutureSight script 1930-1945: on use, `damagestats, damagecalc` (SpAtk vs SpDef incl. Light Screen and stages, item boost, +2, NO crit) and the result is stored; two turns later `checkfuturesight` restores it and skips to `damagevariation, checkhit, applydamage`. NO `stab` at all: no STAB, no type effectiveness, no weather, no badge type boost. | 135-136 disables STAB and crit, but still applies type effectiveness (incl. Dark immunity), weather, badge type boost | Skip weather/badge/type for Future Sight (bug 3.3). |
| Thunder | CORRECT (damage); accuracy INCORRECT in sun | 2012-2032; thunder.asm: rain 100% (bypasses CheckHit), sun 128/256 = 50% | damage vanilla; accuracy rain None, sun 70 | bug 3.13 |
| False Swipe | CORRECT (damage), KO semantics missing | FalseSwipe 1374-1393: `falseswipe` clamps damage to target HP - 1 (false_swipe.asm) | vanilla | Optional: cap the range at `defender.cur_stats.hp - 1` so `find_kill` never reports a KO (4.8). |
| Snore, Dream Eater, Thief, Rapid Spin, Sacred Fire, Flame Wheel, Pay Day, Tri Attack, Leech Life/Absorb/Mega Drain/Giga Drain | CORRECT | vanilla damage; extra effects irrelevant to damage | vanilla | none |

Immunities for the special-damage moves (4-way check at pkmn_damage_calc.py 60-64 and damage_calc.py
48-53) match `resettypematchup` in the StaticDamage script: verified in the run (SonicBoom vs Gastly ->
None, Seismic Toss vs Gastly -> None, Psywave vs Umbreon -> None).

---

## 3. Bugs found (ranked by impact)

Expected numbers below were produced by `gen2_ref.py` (asm transcription) and cross-checked against the
app via `gen2_compare.py`; "APP" lines are the app's actual output at HEAD. All mons are built with the
app's own trainer-style DVs (HP 8 / Atk 9 / Def 8 / Spc 8 / Spd 8) and 0 stat exp unless noted.

### 3.1 Missing `TruncateHL_BC` (stats > 255 are divided by 4 — both of them)
- Affects: every damaging move whenever attack or (screen-doubled) defence >= 256; common late game
  (level 70+ Machamp/Snorlax/Tyranitar-class stats, or any Reflect/Light Screen on def >= 128).
- Root cause: `pkmn_damage_calc.py:213-221` uses the full 16-bit stats; game `effect_commands.asm:2614-2658`.
- Fix (in `calculate_gen_two_damage` right after the Explosion halving is NOT the place — do it before it,
  because the game halves the truncated value):
  ```python
  # after doubled_def / Thick Club / Light Ball / Metal Powder ordering fixed (see 3.7), before Explosion:
  while attacking_stat >= 256 or defending_stat >= 256:
      defending_stat = max(defending_stat >> 2, 1)
      attacking_stat = max(attacking_stat >> 2, 1)
      if version_is_gold_silver:   # one pass, then wrap
          attacking_stat &= 0xFF; defending_stat &= 0xFF
          break
  ```
  (order in game: screens -> crit reload -> Thick Club/Light Ball x2 -> truncate -> Metal Powder ->
  Explosion halve).
- Test: L100 Machamp (Atk 283) Cross Chop vs L100 Snorlax (Def 151). Game: 283/151 -> 70/37;
  `(42*100*70)//37 = 7945; //50 = 158; +2 = 160`; STAB 240; SE 480; rolls 408..480.
  APP: 405..476. Assert `max_damage == 480`, `min_damage == 408`.

### 3.2 Crit with attacker's stage > defender's stage must KEEP Reflect / Light Screen
- Affects: any crit vs a screen when the attacker has boosted Atk/SpAtk (Swords Dance, Curse, Growth,
  Amnesia mirror, etc.) more than the defender's Def/SpDef stage.
- Root cause: `pkmn_damage_calc.py:116-126` (`and not is_crit`); game doubles at 2549-2552/2571-2574
  BEFORE `CheckDamageStatsCritical` and only the unfavourable branch reloads unboosted stats.
- Fix: `doubled_def = defender_has_reflect and not ignore_badge_boosts` (i.e. use the same flag that
  decides "unboosted stats"), same for Light Screen.
- Test: L30 Machop +2 Atk (58 -> 116) Mega Punch, crit, vs L30 Rattata (Def 30) with Reflect.
  Game: def 60; `(14*80*116)//60 = 2165; //50 = 43; x2 = 86; +2 = 88`; no STAB; rolls 74..88.
  APP: 148..174. Control (stages equal): both give 74..88.

### 3.3 Future Sight must skip type effectiveness, weather and badge type boost
- Affects: Future Sight vs Dark (app: None, game hits), vs Psychic/Steel (app halves), vs
  Fighting/Poison (app doubles); Marsh badge; (weather irrelevant, Psychic type).
- Root cause: no `stab` command in the FutureSight script (effects.asm 1930-1945); app only disables
  STAB (135-136) and crit (227).
- Fix: `if move.name == FUTURE_SIGHT: skip weather (232-248), badge (250-251), type loop (264-270)` and
  skip the immunity early-out at 60-64.
- Test: L40 Espeon (SpA 115) Future Sight vs L40 Umbreon (SpD 115). Game: `(18*80*115)//115 = 1440;
  //50 = 28; +2 = 30`; rolls 25..30. APP: None. Vs L40 Alakazam (SpD 79): `165600//79 = 2096; //50 = 41;
  +2 = 43` -> game 36..43, APP 17..21. Vs L40 Machoke (SpD 59): `165600//59 = 2806; //50 = 56; +2 = 58`
  -> game 49..58, APP 98..116.

### 3.4 Hidden Power: DV 8 has its top bit set
- Affects: Hidden Power power for any mon with a DV of exactly 8 in Atk/Def/Spd/Spc (very common for
  trainer-style DVs and for the app's own default trainer DVs 8/9/8/8/8).
- Root cause: `data_objects.py:525-531` uses `dv > 8`; hidden_power.asm 16-43 tests bit 3 (`and %1000`)
  i.e. `dv >= 8`.
- Fix: replace the four `> 8` with `>= 8`.
- Test: DVs 8/8/8/8 -> game ("Fighting", 68); app ("Fighting", 31). DVs Atk 9 / Def 8 / Spd 8 / Spc 9 ->
  game ("Rock", 69); app ("Rock", 54). DVs 15/15/15/15 -> both ("Dark", 70).

### 3.5 Fury Cutter caps at x16
- Root cause: `pkmn_damage_calc.py:282-283` `2**(n-1)`; fury_cutter.asm 15-20 `b = min(count, 5)`.
- Fix: `move_modifier = 2 ** (min(int(custom_move_data), 5) - 1)` (keep "6" in the dropdown only as a
  label for "5+").
- Test: L30 Scyther Fury Cutter vs L30 Rattata: base after STAB 13; "6" must equal "5" = 177..208;
  APP "6" gives 354..416.

### 3.6 Thick Club also works for Cubone
- Root cause: `pkmn_damage_calc.py:105` checks Marowak only; `ThickClubBoost` 2697-2708 passes
  `b = CUBONE, c = MAROWAK`.
- Fix: `if attacking_pkmn.name in (CUBONE, MAROWAK) and held_item == Thick Club`.
- Test: L30 Cubone + Thick Club (Atk 40 -> 80) Bone Club vs L30 Rattata (Def 30): `(14*65*80)//30 =
  2426; //50 = 48; +2 = 50`; STAB 75; rolls 63..75. APP 33..39.

### 3.7 Metal Powder: applies to SpDef too, and to the truncated 8-bit value
- Root cause: `pkmn_damage_calc.py:110-111` (physical only, x1.5 on the 16-bit value);
  `DittoMetalPowder` 2488-2523 is called from `.done` for both branches, after `TruncateHL_BC`:
  `c = c + c>>1`; if that overflows 255: `b = max(b>>1, 1)`, `c = (c + c>>1) >> 1`.
- Fix: apply to `defending_stat` after truncation regardless of category, with the overflow rule.
- Test: L30 Quilava (SpA 57) Ember vs L30 Ditto + Metal Powder (SpD 38 -> 57): `(14*40*57)//57 = 560;
  //50 = 11; +2 = 13`; STAB 19; rolls 16..19. APP 22..27.

### 3.8 Aeroblast is a high-crit move
- Root cause: `raw_pkmn_data/gen_two/moves.json` Aeroblast `attack_flavor: []`; `critical_hit_moves.asm:7`.
- Fix: add `"high_crit"`. Test: `get_crit_rate(mon, Aeroblast) == 0.25` (app: 0.0664).

### 3.9 Badge type boost adds at least 1
- Root cause: `pkmn_damage_calc.py:250-251` `floor(X*1.125)`; `DoBadgeTypeBoosts` misc.asm 190-199
  adds `max(X>>3, 1)`.
- Fix: `temp = temp + max(temp >> 3, 1)`.
- Test: L5 Pidgey with Zephyr badge (Atk 10 -> 11) Gust "No Bonus" vs L5 Rattata (Def 9): `(4*40*11)//9 =
  195; //50 = 3; +2 = 5`; badge 6; STAB 9; rolls 7..9. APP 5..7.

### 3.10 Gust/Twister/Earthquake/Magnitude/Stomp/Pursuit double AFTER the random roll
- Root cause: `pkmn_damage_calc.py:291-305` doubles before the roll; scripts put `double*damage` /
  `pursuit` after `damagevariation`.
- Fix: apply the x2 to each rolled value (`cur_damage = min(cur_damage*2, 65535)`).
- Test: L20 Pidgeotto Gust "Fly Bonus" vs L20 Rattata: pre-roll 19 -> rolls 16..19 -> doubled {32,34,36,38}
  only (4 distinct values). APP: 32..38 with 7 distinct values (odd values impossible in game). Min/max
  are equal in this case; the distribution (kill %) differs. Also harden 298: an empty
  `custom_move_data` currently means "bonus on".

### 3.11 Triple Kick multiplier is applied before STAB/type, and the dropdown means "n-th kick"
- Root cause: `pkmn_damage_calc.py:286-289`; script 1402-1428 (`damagecalc, triplekick, stab`).
- Fix: multiply right after `+2` (before weather); make the dropdown "1/2/3 kicks" = sum of DamageRanges
  of kicks 1..n. Rounding test: pre-STAB X = 9 -> kick 3: game `27 + 13 = 40`, app `3 * 13 = 39`
  (for the run's X = 8 both give 72 — even X hides the bug).

### 3.12 997 cap before the +2
- Root cause: no cap at `pkmn_damage_calc.py:230`; `DamageCalc` 3013-3090.
- Fix: `temp = min(temp, 997) + 2`.
- Test: L50 Golem (Atk 124) Explosion crit vs L30 Rattata (Def 30 -> 15): `(22*250*124)//15 = 45466;
  //50 = 909; x2 = 1818; cap 997; +2 = 999`; STAB (Golem is Rock/Ground: no) -> rolls 850..999.
  APP 1548..1820. (Overkill either way; low impact.)

### 3.13 Thunder accuracy in sun is 50%
- Root cause: `gen_two_object.py:177-182` handles rain only; thunder.asm 8-11 sets `50 percent + 1` = 128.
- Fix: `if move.name == Thunder and weather == SUN: return 50`.

### 3.14 Struggle is boosted by Pink Bow / Polkadot Bow
- Root cause: item check keyed on type "none" (`pkmn_damage_calc.py:138`); game keys on the move's NORMAL
  type inside DamageCalc (2989-3003) while `stab` is what gets skipped.
- Fix: `held_item_boost = table.get(item) == (TYPE_NORMAL if move.name == STRUGGLE else move_type)`.
- Test: L30 Raticate + Pink Bow (Atk 59) Struggle vs L30 Rattata (Def 30): `(14*50*59)//30 = 1376;
  //50 = 27; x110//100 = 29; +2 = 31`; no STAB/type; rolls 26..31. APP (no item boost): 29 pre-roll ->
  24..29.

### 3.15 Multi-hit crit recursion uses the zeroed stage modifiers and drops weather
- Root cause: `pkmn_damage_calc.py:345-359` passes the local `attacking_stage_modifiers` /
  `defending_stage_modifiers` (already replaced by empty `StageModifiers()` at 92-98 when the crit rule
  fired) and omits `weather`. The non-crit hits should use the real stages.
- Fix: keep the originals in separate variables before 88 and pass those (plus `weather`).
- Test: L30 attacker, defender at +2 Def, Fury Swipes "2 Hits", `is_crit=True`: the second (non-crit) hit
  must be computed with Def x2, not Def x1.

### 3.16 (shared code) `DamageRange.add` weights combinations by `count_a + count_b`
- `pkmn/damage_calc.py:159` should be `count_a * count_b` for a correct joint distribution (the run shows
  "897 rolls" for 3 hits instead of 39^3). Affects kill-percentages of multi-hit moves in every gen;
  min/max are unaffected.

### 3.17 (minor) `attacking_battle_stats` is mutated in place
- `pkmn_damage_calc.py:106/108/111` multiply the caller's StatBlock; when the controller passes a
  precomputed `attacking_battle_stats` (transformed mon path, `battle_summary_controller.py` ~780) the
  same object is reused for the normal and crit calls, doubling twice. Copy before mutating.

### 3.18 (minor, stat calc) sqrt cap at 255
- `data_objects.py:404` `ceil(sqrt(statexp))` is 256 for statexp > 65025; the game caps at 255
  (`get_square_root.asm:1,13-16`). At most +1 on a maxed stat.

---

## 4. Not-implemented moves / mechanics

### 4.1 Present (Crystal) — needs a power dropdown
Game: `present.asm` 26-47 rolls 40 bp (40%), 80 bp (30%), 120 bp (10%), heal (20%). Damage path is the
vanilla one (crit, items, STAB, type, random). Inputs: `CUSTOM_MOVE_DATA["Present"] = ["40", "80",
"120"]`; in the calc set `base_power = int(custom_move_data)` like Return at 207-211. Expected (Crystal),
L30 Machamp (Atk 88) Present "120" vs L30 Rattata (Def 30): `(14*120*88)//30 = 4928; //50 = 98; +2 =
100`; no STAB; rolls 85..100.

### 4.2 Frustration — needs the same dropdown as Return
`frustration.asm`: power = `(255 - happiness) * 10 // 25`, max 102 at happiness 0; power 0 (happiness
253..255) does no damage. Add `CUSTOM_MOVE_DATA["Frustration"] = [str(x) for x in range(102, 0, -1)]`
and extend the `RETURN_MOVE_NAME` branch at `pkmn_damage_calc.py:207` to Frustration.

### 4.3 Beat Up
Per party member i that is not fainted and has no status (beat_up.asm 28-47), including the user:
`X_i = ((2*L_i//5 + 2) * 10 * BaseAtk_i) // BaseDef_target // 50`, `x110//100` with BlackGlasses,
x2 on a per-hit crit (17/256 each; +1 stage with Focus Energy/Scope Lens), `+2` (cap 997), then random
217..255 per hit. No STAB, no type effectiveness, no weather, no badge type boost (no `stab` command,
effects.asm 2041-2066). Number of hits = party size (only 1 hit if the party has 1 member; wild user: one
hit with normal `EnemyAttackDamage` stats). Inputs needed: the attacker's whole party (species base Attack
+ level per member) — the app only has the solo mon and a `Trainer.pkmn` list for enemies. Plan: for
enemy trainers iterate `trainer.pkmn`; for the player expose a dropdown "1 Hit".."6 Hits" and use the
solo mon's base Attack/level for each hit (or accept the approximation). Example: L30 Sneasel (base Atk
95) vs Rattata (base Def 35): `(14*10*95)//35 = 380; //50 = 7; +2 = 9` -> 7..9 per hit.

### 4.4 Present (Gold/Silver) — version-specific formula
Because `BattleCommand_Present` clobbers bc/de (1.15 (2)), `damagecalc` runs with:
`level = id(target type 2)`, `bp = 40/80/120`, `A = wTypeMatchup` (10, or 5 vs Rock or Steel, 2 vs
Rock/Steel), `D = 1 if user is Normal-type else id(user type 2)` (0 -> forced to 1 for Normal). Then
item boost, crit x2, `+2`, STAB (Normal users), type, random. Examples (from the run): Normal user vs
mono-Normal target (level 0 -> `2*0//5+2 = 2`): power 40 -> `2*40*10//1//50 = 16 + 2 = 18` -> STAB 27 ->
22..27; power 80 -> 34 -> 51 -> 43..51; power 120 -> 50 -> 75 -> 63..75. Delibird (Ice/Flying, no STAB,
D = 2) vs a Dark target (level 27 -> `54//5+2 = 12`): power 80 -> `12*80*10//2//50 = 96 + 2 = 98` ->
83..98. Implementation: branch on `version_name in (GOLD, SILVER)` inside `calculate_gen_two_damage`,
build a type-id table from `constants/type_constants.asm`, and compute `wTypeMatchup` = product of the
matching `TypeMatchups` entries /10 starting from 10.

### 4.5 Super Fang
`damage = max(target current HP // 2, 1)`; fails vs Ghost (effects.asm 1237-1252, 3175-3195). Needs
current HP; simplest: `DamageRange({max(defending_pkmn.cur_stats.hp // 2, 1): 1})` (full-HP assumption),
or a dropdown of HP fractions. Also stop the `-1` base power from reaching the formula (guard at 57-58:
`base_power <= 0` -> None) so unimplemented moves show "no damage" instead of "1".

### 4.6 OHKO moves (Guillotine, Horn Drill, Fissure)
Damage 65535 (i.e. a KO) if it hits; fails when the target is immune or `L_user < L_target`; accuracy
byte `min(76 + 2*(L_user - L_target), 255)` further modified by accuracy/evasion stages, hit iff
`rand < byte` (5420-5462 + CheckHit). Plan: `DamageRange({defender.cur_stats.hp: 1})`; in
`get_move_accuracy` return `0` if under-levelled else `min(76 + 2*dL, 255) / 256 * 100`; crit rate 0.

### 4.7 Counter / Mirror Coat / Bide
Counter: 2x the damage of the last move that hit the user, only if that move's type is physical
(`< SPECIAL`), its power > 0, and the opponent moved first; Ghost immune (counter.asm). Mirror Coat:
same for special types; Dark immune. Bide: 2x the sum of damage taken over the 2-3 stored turns
(`wPlayerDamageTaken`), neutral type, Ghost immune (bide.asm 23-46). All require an "enemy damage taken"
input the app does not have; document as "requires enemy damage input; not implemented" and make them
return None (Mirror Coat currently shows 1 damage).

### 4.8 False Swipe KO cap
`falseswipe` (false_swipe.asm) sets damage to `HP - 1` when it would KO. Optional: clamp the returned
range at `defender.cur_stats.hp - 1` so `find_kill` never claims a KO.

### 4.9 Accuracy/evasion stages, Bright Powder, Foresight, Lock-On, Fly/Dig
`CheckHit.StatModifiers` (1741-1826) and `.BrightPowder` (1580-1596); table in 1.14. The app's
`StageModifiers` already carries accuracy/evasion stages; `get_move_accuracy` could apply
`byte = pct*255//100; byte = max(byte*num//den,1)` twice, cap 255, `-20` for Bright Powder on the
defender, and return `byte/256*100` (or 100 if byte == 255).

### 4.10 Crit-stage modifiers
Focus Energy +1, Scope Lens +1, Lucky Punch (Chansey) +2, Stick (Farfetch'd) +2, cumulative with the
high-crit +2; table in 1.3. Held items are already on `EnemyPkmn.held_item`; Focus Energy would need a
FieldStatus flag.

### 4.11 Foresight
With `SUBSTATUS_IDENTIFIED`, Normal/Fighting hit Ghost neutrally (type loop stops at the `-2` marker,
1300-1310) and evasion boosts above the user's accuracy are ignored. Not modelled.

### 4.12 Gold/Silver `TruncateHL_BC` wrap
When implementing 3.1, add the one-pass-then-wrap behaviour for Gold/Silver (1.15 (1)); e.g. Reflect on
Def 600 -> 1200 -> one pass 300 -> `c = 300 & 0xFF = 44`.

---

## 5. Things verified correct (brief)
- Base formula order and every floor: `(2L//5+2) * bp * A // D // 50`, item x110//100, crit x2, `+2`.
- Crit stage rule (`atkStage <= defStage` => both sides unboosted, no badges) and that unboosted means
  "raw party stats" (copied before `BadgeStatBoosts`).
- Flail/Reversal/Future Sight never crit; Flail/Reversal have no random roll and a single value; their
  HP-bracket powers 20/40/80/100/150/200.
- Crit rates 17/256 and 1/4 for stage 0 / high-crit moves; high-crit list except Aeroblast.
- Stat formula, stage multiplier table (25/28/33/40/50/66/100/150/200/250/300/350/400 %), min 1 / cap 999,
  badge boost `+1/8` cap 999, badge->stat mapping (Zephyr Atk, Mineral Def, Plain Spd, Glacier SpA), and
  the exact Glacier/SpDef glitch windows (206..432, >= 661).
- Badge type boost mapping (all 16 badges), player-only.
- Held-item boost table and its position; `floor(x*1.1)` equals the integer math.
- STAB `+X>>1`; type chart contents and per-type application order vs the game table; immunity handling
  incl. Foresight-less Ghost immunities; dual-type rounding (floor after each step).
- Weather table (rain/sun x1.5/x0.5 on Water/Fire, SolarBeam x0.5 in rain only, sandstorm none) and its
  position (before badge boost and STAB); Thunder always hits in rain.
- Random roll 217..255 over 255, skipped for damage < 2; min damage 1.
- Explosion/Selfdestruct defence halving (min 1) after screens.
- Light Ball (Pikachu, SpAtk x2), Metal Powder species check (original species, even when transformed).
- Magnitude power table (10/30/50/70/90/110/150) and Dig bonus; Rollout doublings incl. Defense Curl and
  the 5-hit cap; Rage x(1+counter); multi-hit count distribution 3/8-3/8-1/8-1/8; Double Kick/Twineedle/
  Bonemerang 2 hits; accuracy checked once per multi-hit move.
- Fixed/level damage moves (SonicBoom 20, Dragon Rage 40, Seismic Toss/Night Shade = level) and their
  type immunities; Psywave distribution 1..floor(1.5L)-1 uniform.
- Hidden Power type table/index and the physical/special split by computed type.
- Return power dropdown; Struggle skipping STAB/type; Swift/Faint Attack/Vital Throw accuracy None.
- moves.json power/type/accuracy/pp for all 251 moves vs `moves.asm`; special-type list vs `SPECIAL`.
