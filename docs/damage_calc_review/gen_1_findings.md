# Gen 1 damage-calc audit

All ROM citations are `pokeyellow` (A:\Cygwin\home\scott\pokeyellow) unless prefixed `pokered`. The two
games were diffed; **every damage routine is byte-for-byte identical between Red/Blue and Yellow** (see section 0),
so one calc for all three versions is correct. pokered line numbers are given once per routine for cross-reference.

App root: A:\pkmn_yellow_xp_router. All app line numbers refer to the working tree as read during this audit.

---

## 0. Sources consulted

### pokeyellow (engine/battle/core.asm, 6825 lines)
- `ExecutePlayerMove` 3244-3450 (`PlayerCalcMoveDamage` 3305-3321, miss handling 3322-3329, `.moveDidNotMiss`/`ApplyAttackToEnemyPokemon` 3397-3402, `AlwaysHappenSideEffects` 3404-3408, faint check 3409-3413, `HandleBuildingRage` 3414, multi-hit loop 3416-3428, `SpecialEffects` 3429-3440)
- `CheckPlayerStatusConditions` 3499-3759 (confusion 3579-3606, Bide 3652-3700, Thrash 3702-3723, trapping 3725-3737, Rage 3739-3750)
- `HandleSelfConfusionDamage` 3843-3885
- `PrintMoveFailureText` + Jump Kick crash 3889-3944
- `GetDamageVarsForPlayerAttack` 4201-4311 (pokered 4030), `GetDamageVarsForEnemyAttack` 4314-4425 (pokered 4143), `GetEnemyMonStat` 4429-4468 (pokered 4258)
- `CalculateDamage` 4470-4636 (pokered 4299), `JumpToOHKOMoveEffect` 4638-4642
- `CriticalHitTest` 4649-4713 (pokered 4478), `HandleCounterMove` 4718-4781 (pokered 4547)
- `ApplyAttackToEnemyPokemon` 4783-4848 (Super Fang 4795-4812, special-damage 4813-4847) (pokered 4612), `ApplyDamageToEnemyPokemon` 4849-4900
- `ApplyAttackToPlayerPokemon` 4902-4967 (enemy Psywave 4947-4960) (pokered 4731)
- `HandleBuildingRage` 5084-5124 (pokered 4913)
- `AdjustDamageForMoveType` 5246-5356 (pokered 5075); `AIGetTypeEffectiveness` 5363-5405 (AI only; contains the Yellow-only Lorelei/Dewgong branch 5392-5401)
- `MoveHitTest` 5410-5527 (pokered 5228), `CalcHitChance` 5530-5599 (pokered 5348), `RandomizeDamage` 5602-5636 (pokered 5420)
- `ExecuteEnemyMove` 5639-5855 (`EnemyCalcMoveDamage` 5706-5722) (pokered 5457/5524)
- `LoadBattleMonFromParty` 1667-1708 (badge boost at send-out: 1700) (pokered 1626)
- `LoadEnemyMonData` 6174-6331 (trainer DVs 6188-6193) (pokered 5992), `SwapPlayerAndEnemyLevels` 6370-6379
- `QuarterSpeedDueToParalysis` 6468-6509, `HalveAttackDueToBurn` 6511-6548
- `CalculateModifiedStats` 6550-6558, `CalculateModifiedStat` 6561-6637 (pokered 6365/6376), `ApplyBadgeStatBoosts` 6639-6690 (pokered 6454)
- `BattleRandom` 6728-6755, `HandleExplodingAnimation` 6787-6819 (pokered 6602)

### pokeyellow engine/battle/effects.asm (1558 lines)
- `StatModifierUpEffect` 387-542 (pokered 351), `StatModifierDownEffect` 575-734 (pokered 539), `BideEffect` 800-825 (pokered 764), `ThrashPetalDanceEffect` 827-844, `TwoToFiveAttacksEffect` 961-1005 (pokered 925), `ChargeEffect` 1037-1086, `TrappingEffect` 1136-1159 (pokered 1080), `HyperBeamEffect` 1227-1235, `RageEffect` 1249-1257, `MimicEffect`, `CheckTargetSubstitute` 1492-1502
- engine/battle/move_effects/: `one_hit_ko.asm` 1-38, `drain_hp.asm` 1-104, `recoil.asm` 1-70, `pay_day.asm` 1-45, `focus_energy.asm` 1-22, `reflect_light_screen.asm` 1-45, `substitute.asm` 1-77

### Data / constants / home
- data/battle/critical_hit_moves.asm, stat_modifiers.asm, special_effects.asm, set_damage_effects.asm, always_happen_effects.asm, residual_effects_1.asm, residual_effects_2.asm
- data/moves/moves.asm (full table), data/moves/effects_pointers.asm (full table), data/types/type_matchups.asm (full table), constants/type_constants.asm, constants/move_effect_constants.asm
- constants/battle_constants.asm: `MIN_NEUTRAL_DAMAGE EQU 2` (55), `MAX_NEUTRAL_DAMAGE EQU 999` (56), `SONICBOOM_DAMAGE EQU 20` (59), `DRAGON_RAGE_DAMAGE EQU 40` (60), `MAX_STAT_VALUE EQU 999` (77), `ATKDEFDV_TRAINER EQU $98` (80), `SPDSPCDV_TRAINER EQU $88` (81)
- macros/data.asm:3 `DEF percent EQUS "* $ff / 100"` (integer; so `85 percent + 1` = 217, `100 percent` = 255)
- home/move_mon.asm `CalcStat` 54-215 (stat formula and the ceil-sqrt loop 73-92)
- engine/math/multiply_divide.asm `_Divide` 62+ (floor division; divisor 0 never terminates)
- engine/items/item_effects.asm `ItemUseXStat` 1805-1845 (X items go through `StatModifierUpEffect` with `hWhoseTurn = 0`)
- engine/battle/experience.asm 226-241 (mid-battle level-up recomputes stats and re-applies badge boosts once)
- engine/battle/trainer_ai.asm:728 (AI X items also call `StatModifierUpEffect`, but on the enemy's turn)

### pokered vs pokeyellow diffs actually run
- `diff pokered/engine/battle/core.asm pokeyellow/engine/battle/core.asm`: hunks only in CGB palette calls, Pikachu/Prof.Oak battle types, Pikachu happiness, tutorial menus, debug fight menu, a PP-Up move-selection bugfix, VC hooks, the substitute-break animation label, `AIGetTypeEffectiveness` (Lorelei Dewgong), back-pic loading, SRAM access and InitBattle being moved to another file. **No hunk touches any routine listed above.**
- `diff` of effects.asm: Stadium-flag (`wUnknownSerialFlag_d499`) branches in Sleep/Freeze, extra animation calls, `ClearHyperBeam` on flinch in link battles, a push/pop reorder. No numeric change.
- `diff` of data/moves/moves.asm, data/types/type_matchups.asm, data/battle/*.asm (all six), home/move_mon.asm, constants/type_constants.asm: **identical**. constants/battle_constants.asm differs only by two Yellow battle-type constants. move_effects/*.asm identical except transform.asm (PP copy, irrelevant).

### App files read
pkmn/gen_1/pkmn_damage_calc.py (all), pkmn/gen_1/data_objects.py (all), pkmn/gen_1/pkmn_utils.py (all), pkmn/gen_1/gen_one_object.py (all), pkmn/damage_calc.py (all), pkmn/universal_data_objects.py (StageModifiers 100-233, EnemyPkmn 336-413, Move 457-480, FieldStatus 521-550), raw_pkmn_data/gen_one/moves.json, raw_pkmn_data/gen_one/type_info.json, raw_pkmn_data/gen_one/{yellow,red_blue}/pokemon.json (types only), utils/constants.py (FLAVOR_*, MULTI_HIT_*, effectiveness strings), controllers/battle_summary_controller.py ~760-1000, tests/conftest.py, tests/test_unit_calcs.py 130-215.

### Verification scripts (scratchpad, not in the repo)
- `type_order_check.py`: proves the app's chart equals the ROM table as a set (82/82 entries) and lists the 27 (species, move-type) pairs where the ROM row order differs from the app's type_1-then-type_2 order.
- `gen1_ref_harness.py` + `gen1_sweep.py`: a transcription of GetDamageVars/CalculateDamage/AdjustDamageForMoveType/RandomizeDamage run against `gen.calculate_damage` for 5280 (matchup, level, stage, screen, crit) combinations plus targeted cases. All numbers quoted below come from these runs and were re-derived by hand.
- `moves_cmp.py`: moves.json vs data/moves/moves.asm for every move.

---

## 1. Core formula verification

Notation: L = attacker level, BP = base power, A = offensive stat, D = defensive stat as the game reads them.

### 1.1 Which stats are read (GetDamageVarsForPlayerAttack 4201-4311; enemy mirror 4314-4425)
Game:
1. BP = move power byte; if 0 the routine returns Z and damage calc is skipped (4206-4210).
2. Physical if type < `SPECIAL` ($14, i.e. Normal/Fighting/Flying/Poison/Ground/Rock/Bug/Ghost), special otherwise (Fire/Water/Grass/Electric/Psychic/Ice/Dragon) (4211-4213, constants/type_constants.asm).
3. D = defender's **in-battle** stat (`wEnemyMonDefense`/`wEnemyMonSpecial` = stage-modified; for the player as defender `wBattleMonDefense`, which also includes badge boosts and burn/para penalties).
4. If the defender has Reflect (physical) / Light Screen (special): `sla c / rl b` = **16-bit doubling, no cap** (4219-4224, 4249-4254, comment 4255-4256).
5. If crit (`wCriticalHitOrOHKO` != 0): D is **replaced** by `GetEnemyMonStat` (a fresh `CalcStat` from base stats + DVs, stat exp ignored) and A by the **party-struct** stat (`wPartyMon1Attack`/`wPartyMon1Special` + n*PARTYMON_STRUCT_LENGTH) (4227-4243, 4259-4274). Consequences: on a crit the screen doubling is discarded (bc is overwritten), stat stages are discarded, badge boosts are discarded (party stats are never badge-boosted; only `wBattleMon*` are, see 1.6), burn/paralysis penalties are discarded. For the enemy attacking (4340-4356, 4372-4387) it is symmetric: player's party Defense/Special and the enemy's CalcStat'd Attack/Special.
6. `.scaleStats` (4278-4298): if the high byte of A **or** D is non-zero (either > 255), **both** are divided by 4 (`srl/rr` twice = floor(x/4)); if A becomes 0 it is set to 1 (D is not protected: comment 4289 "division by 0 freeze").
7. Only the **low byte** is passed on: `ld b, l` (4300) and `c` (4301). A after scaling is always <= 249 (999/4), but D after Reflect/Light Screen can be up to 1998/4 = 499, so **D wraps modulo 256 when the doubled stat is >= 1024 (unscaled stat >= 512)**. If the doubled stat is 1024-1027 (stat 512-513) D becomes 0 and the game hangs in `_Divide`.
8. E = level; `sla e` doubles it on a crit (4302-4307).

App (pkmn/gen_1/pkmn_damage_calc.py):
- special/physical split via `special_types` (type_info.json `special_types` = Water, Grass, Fire, Ice, Electric, Psychic, Dragon) lines 76-89: **CORRECT**.
- screens: `defending_stat *= 2` unless crit (79-92): doubling **CORRECT**, but see step 6/7.
- crit: `calc_battle_stats(is_crit=True)` (data_objects.py 176-184) drops badges and all stat stages; screens skipped in the calc: **CORRECT** (verified numerically, harness T6).
- **/4 scaling and low-byte wrap: MISSING** (commented out at 97-105, and even the commented version is in the wrong place relative to Explosion; see Bug 1).

### 1.2 Base damage (CalculateDamage 4470-4636)
Game, in order:
1. `EXPLODE_EFFECT`: `srl c` (the 8-bit D **after** scaling), min 1 (4484-4489). So Explosion halving happens **after** the /4 scaling and low-byte truncation.
2. Multi-hit effects skip the 0-BP check; `OHKO_EFFECT` jumps to `OneHitKOEffect_`; BP 0 returns (4492-4505).
3. `a = 2*E` with a carry into the high byte (E is already 2L on a crit, so this is 4L) (4514-4524), `/5` (4526-4532), `+2` (4534-4536). => `base = floor(2L/5)+2`, crit `floor(4L/5)+2`.
4. `*BP` (4540-4542), `*A` (4544-4546), `/D` floor (4548-4551), `/50` floor (4553-4556). => `q = floor(floor(base*BP*A / D) / 50)`.
5. Cap: if `q >= 998` then `q = 997` (`MAX_NEUTRAL_DAMAGE - MIN_NEUTRAL_DAMAGE`), then `+2` (`MIN_NEUTRAL_DAMAGE`) (4558-4631). (`wDamage` was zeroed in GetDamageVars, so the "add [wDamage]" steps add 0.) => `dmg = min(q, 997) + 2`, max 999 before STAB/type.

App 109-119: `temp = 2L (*2 if crit); floor(/5)+2; *BP; *A; floor(/D); floor(/50); +2`: order and truncation **CORRECT**; **cap MISSING** (Bug 4). Explosion halving at 94-95 is applied **before** the (missing) scaling instead of after: wrong order (part of Bug 1).

### 1.3 Critical hits (CriticalHitTest 4649-4713; data/battle/critical_hit_moves.asm)
Game: `b = BaseSpeed >> 1` (4660-4662). If Focus Energy (`GETTING_PUMPED`): `b >>= 1` (bug, 4677-4685). Else `b <<= 1` with cap 255 on carry (4680-4683). High-crit move (Karate Chop, Razor Leaf, Crabhammer, Slash): `b <<= 1` twice, each with cap 255 on carry (4696-4703); otherwise `b >>= 1` (4694). `r = BattleRandom` rotated left 3 times (bijection on 0..255, so still uniform), crit iff `r < b` (4705-4710). Base power 0 => no crit test (4671-4673). SetDamageEffects moves (Super Fang, Seismic Toss, Night Shade, SonicBoom, Dragon Rage, Psywave) skip `CriticalHitTest` entirely (3305-3310).

Exact probabilities (b/256):
| situation | b |
|---|---|
| normal move, no Focus Energy | `floor(bs/2)` |
| high-crit move, no FE | `min(255, 8*floor(bs/2))` (cap reached iff bs >= 64) |
| normal move, Focus Energy | `floor(bs/8)` (a quarter of the normal rate) |
| high-crit move, Focus Energy | `4*floor(bs/4)` (never capped) |
Max is 255/256 = 99.61%, never 100%.

App `get_crit_rate` (13-20): `floor(bs/2)`, `*8` if high_crit, `min(255)`, `/256`: **CORRECT** for the non-Focus-Energy cases (verified for base speeds 30..150, harness T12). Focus Energy is not an input (see 4.7). Crit level doubling: **CORRECT** (110-111).

### 1.4 Stat calculation, stage multipliers, caps (home/move_mon.asm CalcStat 54-215; core.asm 6561-6637; effects.asm 443-449, 477-485, 696-703)
Game `CalcStat`: `b = smallest b with b*b >= statExp, but the loop stops at b = 255` (73-92) => `b = min(255, ceil(sqrt(statExp)))`; `stat = floor(((base+DV)*2 + (b>>2)) * L / 100) + 5` (HP: `+ L + 10`) (154-213), capped at 999 (214+).
App `calc_unboosted_stat` (pkmn_utils.py 67-75): `floor(ceil(sqrt(statExp))/4)` — **INCORRECT only when statExp > 65025** (ceil(sqrt) = 256 -> app 64, game 63). See Bug 6. Otherwise CORRECT.

Stage multipliers: `StatModifierRatios` (data/battle/stat_modifiers.asm) = 25/100, 28/100, 33/100, 40/100, 50/100, 66/100, 1/1, 15/10, 2/1, 25/10, 3/1, 35/10, 4/1. `CalculateModifiedStat`: `floor(unmodified * num / den)`, cap 999 (6616-6625), min 1 (6632-6634). `StatModifierUpEffect` recomputes only the changed stat from the **unmodified** stat (wipes previous badge boosts on it), cap 999 (477-485); if the stat is already 999 the stage is not incremented at all ("nothing happened", 443-449). `StatModifierDownEffect` same, min 1 (696-703), and refuses if the stat is already 1 (663-668).
App `STAGE_MOFIDIERS` (pkmn_utils.py 16-30) identical; `modify_stat_by_stage` floor (51-64); `_clamp_stat` 1..999 (33-34): **CORRECT**.

Trainer DVs: `ATKDEFDV_TRAINER $98`, `SPDSPCDV_TRAINER $88` (LoadEnemyMonData 6188-6193) => Atk 9, Def 8, Spd 8, Spc 8, HP DV = (9&1)<<3 | (8&1)<<2 | (8&1)<<1 | (8&1) = 8 (CalcStat 109-133). App `GenOneStatBlock(8, 9, 8, 8, 8, 8)` (pkmn_utils.py 133, gen_one_object.py 403): **CORRECT**. Trainer/wild stat exp 0 (`GetEnemyMonStat` calls CalcStat with b=0 "no stat exp", 4464-4466): app uses 0: CORRECT.

### 1.5 Badge boosts (ApplyBadgeStatBoosts 6639-6690)
Game: for Attack/Defense/Speed/Special (in RAM order) if the corresponding badge bit is set (Boulder bit0 -> Attack, Thunder bit2 -> Defense, Soul bit4 -> Speed, Volcano bit6 -> Special; 6647-6652): `stat += stat >> 3` (three 16-bit srl/rr, 6666-6680), cap 999 (6681-6689). = `min(999, floor(stat * 9/8))` = `floor(stat*1.125)`. Applied to `wBattleMon*` only, never to the party struct, never to the enemy (only `wBattleMonAttack` is addressed; link battles skip it 6640-6642).
When it runs (each call multiplies **all four** boosted stats by 1.125 again, on top of whatever they currently are):
- send-out / battle start (`LoadBattleMonFromParty` 1700);
- after the player's own stat-up move, **including Accuracy/Evasion moves** (Double Team, Minimize): effects.asm 532-536 (`call z` = player's turn). The changed stat was first recomputed from its unmodified value, so it ends with exactly one boost; the other three stack an extra one;
- after the **enemy's** stat-down move or stat-down side effect that actually lands on the player (effects.asm 722-726, `call nz` = enemy's turn). Not when the player lowers the enemy's stats, not when the enemy raises its own;
- X Attack/Defend/Speed/Special (item_effects.asm 1815-1831 set `hWhoseTurn = 0` and call `StatModifierUpEffect`) -> same as a stat-up move;
- Rage build-up when the player's Rage user is hit (`HandleBuildingRage` 5102-5115 flips `hWhoseTurn` to the player and calls `StatModifierUpEffect`);
- a mid-battle level-up: `CalculateModifiedStats` (all four stats from the new unmodified values x stage) then `ApplyBadgeStatBoosts` **once** (experience.asm 234-241) — all stacked extra boosts are lost, every boosted stat ends with exactly one boost.
- "Nothing happened" cases (stage already +6 / -6, stat already 999 / 1, side-effect roll failed, move missed) do **not** call it.

App: `badge_boost_single_stat` = `_clamp_stat(floor(stat*1.125))` (pkmn_utils.py 86-88): CORRECT (9/8 is exact in binary). Badge->stat mapping (data_objects.py 45-58): CORRECT. `calc_battle_stats` (data_objects.py 194-241): `stage -> one boost -> extra boosts from StageModifiers.*_badge_boosts`: CORRECT model (harness T13: Alakazam L60 Boulder+Volcano, Growth once: game Atk 84->94, Spc 297; app 94 / 297). `StageModifiers.apply_stat_mod` (universal_data_objects.py 150-204) increments all four counts and resets the changed one; no-op mods return `self`: CORRECT. Enemy never boosted: CORRECT (enemy `badges` is None). Not verified here (router-level, out of scope): that X items and the enemy's landed stat-drops go through `apply_stat_mod`, and that a mid-battle level-up calls `clear_badge_boosts()` (controllers/battle_summary_controller.py:1796 exists; trigger not audited).

### 1.6 Item boosts, natures, weather, double-battle spread
N/A in gen 1 (no held items, no natures, no weather, no doubles). App passes none: CORRECT.

### 1.7 STAB and type effectiveness (AdjustDamageForMoveType 5246-5356; data/types/type_matchups.asm)
Game: STAB if move type == attacker type1 or type2: `dmg += dmg >> 1` (5281-5289) = `floor(1.5*dmg)`. Then the **82-row `TypeEffects` table is walked top to bottom**; every row whose attacking type equals the move type and whose defending type equals the defender's type1 **or** type2 is applied in **table order**: `dmg = floor(dmg * mult / 10)` with mult 20 / 5 / 0 (5298-5354). After each row, if `dmg == 0` the move is flagged missed (5342-5347) — this is how immunity works (mult 0), and how a 0.25x hit of 2-3 base damage "misses".
App 107-135: STAB `floor(temp/2)` added: CORRECT. Multipliers `*2` and `floor(/2)`: equal to `floor(d*20/10)` and `floor(d*5/10)`: CORRECT. Immunity -> `None` (73-74, 137-138): CORRECT (both are treated as "no damage"). Chart contents: identical to the ROM as a set (82 entries, including the Ghost->Psychic NO_EFFECT bug): CORRECT.
**Order: INCORRECT.** The app applies type_1 then type_2 (127-135). Because `floor(d/2)*2 != d` for odd d, the order matters when one type is SE and the other NVE. 27 (species, move type) pairs disagree with the ROM order; 21 of them are SE/NVE mixes (the other 6 involve mult 0 and give 0 either way). See Bug 2 for the list and the fix.

### 1.8 Random roll and minimum damage (RandomizeDamage 5602-5636)
Game: if `dmg < 2` return unchanged (5603-5609). Else loop `r = BattleRandom >> rotate` until `r >= 217` (`85 percent + 1`), uniform on 217..255 (39 values), `dmg = floor(dmg * r / 255)` (5618-5635). Minimum result for dmg >= 2 is `floor(2*217/255) = 1`, so the effective floor is 1 and a damage of 1 stays 1.
App 155-163: `max(floor(temp*r/255), 1)` for r in 217..255: identical output in every case (for temp = 1 the app yields 1 for all 39 rolls, the game also 1): **CORRECT**.

### 1.9 Damage cap
Game: `min(q, 997) + 2` before STAB/type (section 1.2), i.e. at most 999 pre-STAB, 1498 with STAB, 5992 with STAB and 4x. App: no cap: **INCORRECT** (Bug 4; only reachable in extreme matchups).

### 1.10 Explosion / Selfdestruct
Game: D (8-bit, post-scaling) `>> 1`, min 1 (4484-4489), on crits too. User faints even if the move misses/is immune (`AlwaysHappenSideEffects`, `ExplodeEffect` effects.asm 185-202). App 94-95: halving with min 1 CORRECT in isolation, **but applied before the missing /4 scaling** (Bug 1).

### 1.11 Multi-hit (TwoToFiveAttacksEffect effects.asm 961-1005; loop core.asm 3416-3428)
Game: damage, crit and random roll are computed **once**; each subsequent hit re-enters at `GetPlayerAnimationType` and re-applies the same `wDamage` (comment 3422-3423). Loop stops when the target faints. Hit count for `TWO_TO_FIVE_ATTACKS_EFFECT`: `r&3`, re-rolled once if >= 2 => 2:3/8, 3:3/8, 4:1/8, 5:1/8 (987-997). `ATTACK_TWICE_EFFECT` and `TWINEEDLE_EFFECT` always 2 (982-986, 1002-1005).
App 140-153: one roll times N (2 for `two_hit`, 2..5 from the "2 Hits".."5 Hits" dropdown for `multi_hit`, controllers/battle_summary_controller.py ~833): **CORRECT**.

### 1.12 Accuracy (MoveHitTest 5410-5527, CalcHitChance 5530-5599)
Game: accuracy byte = `acc percent` = `floor(acc*255/100)`; hit iff `BattleRandom < byte` (5503-5508), so **every move has at least a 1/256 miss** (comment 5504-5505). With stages: `hit = min(255, floor(floor(byte*accNum/accDen) * evNum/evDen))` where the evasion ratio index is `14 - evasionStage` and each intermediate result is floored to at least 1 (5546-5596). Exceptions: `SWIFT_EFFECT` returns before any test (never misses, even vs Dig/Fly, 5429-5432); X Accuracy bypasses the roll (5470-5472); Dream Eater fails unless the target is asleep (5422-5428); Dig/Fly `INVULNERABLE` targets are missed (5441-5443).
Real hit chances: 100 -> 255/256 = 99.61%, 95 -> 242/256 = 94.53%, 90 -> 229/256 = 89.45%, 85 -> 216/256 = 84.38%, 80 -> 204/256 = 79.69%, 75 -> 191/256 = 74.61%, 70 -> 178/256 = 69.53%, 65 -> 165/256 = 64.45%, 55 -> 140/256 = 54.69%, 30 -> 76/256 = 29.69%.
App: `get_move_accuracy` returns `move.accuracy` (gen_one_object.py 164-165), used as acc/100 in `find_kill`; None -> 100: **approximation** (see 4.8). Data errors in moves.json accuracy: Bind 85 (ROM 75), Psywave 100 (ROM 80) — see Bug 7.

### 1.13 Special-damage moves, immunity (SetDamageEffects; PlayerCalcMoveDamage 3305-3310; ApplyAttackToEnemyPokemon 4813-4847)
Game: `SUPER_FANG_EFFECT` and `SPECIAL_DAMAGE_EFFECT` jump straight to `MoveHitTest`, skipping `CriticalHitTest`, `GetDamageVars`, `CalculateDamage`, `AdjustDamageForMoveType` and `RandomizeDamage`. Therefore: no crit, no STAB, **no type effectiveness and no immunity** (Seismic Toss hits Gastly for L, Night Shade hits Rattata for L, Dragon Rage hits anything for 40), still subject to the accuracy roll and Substitute. Damage: Seismic Toss / Night Shade = user level; SonicBoom 20; Dragon Rage 40; Psywave: player `[1, floor(1.5L)-1]` uniform (rejection sampling, 4828-4841), enemy `[0, floor(1.5L)-1]` uniform (4947-4960, comment 4953-4955).
App: `get_special_damage_override` (pkmn/damage_calc.py 29-55) by name, before anything else, immunity ignored in gen 1: **CORRECT** (verified T10). Psywave player range: CORRECT (`range(1, floor(1.5L))`, 54-56); enemy: INCORRECT (Bug 5).

### 1.14 Player vs enemy asymmetries (for routing)
- Enemy stats are never badge-boosted (1.5). Enemy in-battle stats = `CalcStats` from base stats, level, DVs $9888 (trainers) or random (wild), 0 stat exp (LoadEnemyMonData 6199-6209). App matches.
- On a crit the player's A/D come from the party struct (includes stat exp earned up to the last level-up), the enemy's from a fresh CalcStat. App matches modulo its own stat model.
- Enemy Psywave can roll 0 (Bug 5).
- Enemy stat-down moves (non-side-effect) on the player have an extra `25 percent + 1` = 64/256 fail chance in non-link battles (effects.asm 585-590). Not damage; noted for the stat-stage UI.
- The enemy's badge-boost re-application never happens; the player's does on the enemy's landed stat-drops (1.5).
- `SwapPlayerAndEnemyLevels` (6370-6379) is called symmetrically around the enemy's calc so all level reads are the enemy's own level; no quirk for damage.
- AI trainer X items (trainer_ai.asm:728) raise enemy stages with the same 999 cap and never boost badges.

---

## 2. Per-move table

Status moves skipped (55, all `base_power` None in moves.json, all 0 BP in the ROM; none is wrongly treated as damaging): Swords Dance, Whirlwind, Sand Attack, Tail Whip, Leer, Growl, Roar, Sing, Supersonic, Disable, Mist, Leech Seed, Growth, PoisonPowder, Stun Spore, Sleep Powder, String Shot, Thunder Wave, Toxic, Hypnosis, Meditate, Agility, Teleport, Mimic, Screech, Double Team, Recover, Harden, Minimize, Smokescreen, Confuse Ray, Withdraw, Defense Curl, Barrier, Light Screen, Haze, Reflect, Focus Energy, Metronome, Mirror Move, Amnesia, Kinesis, Softboiled, Glare, Poison Gas, Lovely Kiss, Transform, Spore, Flash, Splash, Acid Armor, Rest, Sharpen, Conversion, Substitute. (Data nits that affect the stat-stage dropdowns, not damage: Psychic's effect in moves.json is "self -1 spa/spd 100%", but the ROM effect is `SPECIAL_DOWN_SIDE_EFFECT` = 33.2% chance to lower the **target's** Special by 1 (moves.asm PSYCHIC_M; effects.asm 594-602). Poison Gas accuracy is 55 in the ROM, 90 in json. Swords Dance/Growth/Petal Dance/Jump Kick/Submission/Struggle PP differ from the ROM; irrelevant.)

Game-damaging moves that the app returns `None` for are marked NOT IMPLEMENTED. "vanilla" = plain formula of section 1 (so every vanilla row inherits Bugs 1, 2, 4 where the matchup triggers them).

| Move | App status | What the game does (cite) | What the app does (file:line) | Fix / plan |
|---|---|---|---|---|
| Pound, Mega Punch, Pay Day, Fire Punch, Ice Punch, ThunderPunch, Scratch, Vicegrip, Cut, Gust, Wing Attack, Slam, Vine Whip, Stomp, Mega Kick, Rolling Kick, Headbutt, Horn Attack, Tackle, Body Slam, Poison Sting, Bite, Acid, Ember, Flamethrower, Water Gun, Hydro Pump, Surf, Ice Beam, Blizzard, Psybeam, BubbleBeam, Aurora Beam, Peck, Drill Peck, Low Kick, Strength, Thundershock, Thunderbolt, Thunder, Rock Throw, Earthquake, Confusion, Psychic, Quick Attack, Egg Bomb, Lick, Smog, Sludge, Bone Club, Fire Blast, Waterfall, Constrict, Bubble, Dizzy Punch, Rock Slide, Hyper Fang, Tri Attack (58 moves) | CORRECT (vanilla) | Vanilla formula; side effects (status/flinch/stat-drop/coins) don't touch damage. Power/type/accuracy in moves.json match data/moves/moves.asm for all 58. | pkmn_damage_calc.py 58-164 | Inherit Bugs 1/2/4 fixes. |
| Karate Chop, Razor Leaf, Crabhammer, Slash | CORRECT (vanilla + high crit) | `HighCriticalMoves` table (data/battle/critical_hit_moves.asm) -> crit b = min(255, 8*floor(bs/2)) (1.3). Damage vanilla. | get_crit_rate 14-18 (`high_crit` flavor) | none |
| Doubleslap, Comet Punch, Fury Attack, Pin Missile, Spike Cannon, Barrage, Fury Swipes | CORRECT (one roll x N) | `TWO_TO_FIVE_ATTACKS_EFFECT`: one damage/crit/roll, applied 2-5 times (3/8,3/8,1/8,1/8), stops on faint (effects.asm 961-1001, core.asm 3416-3428). Accuracy checked once. | 140-153 + "N Hits" dropdown (controller ~833) | Optionally weight the dropdown by 3/8,3/8,1/8,1/8 in kill % — design choice. |
| Double Kick, Twineedle, Bonemerang | CORRECT | `ATTACK_TWICE_EFFECT` / `TWINEEDLE_EFFECT`: always 2 hits, one calc (effects.asm 982-986, 1002-1005). | 143-144 (`two_hit` x2) | none |
| Razor Wind, Solar Beam, Skull Bash, Sky Attack | CORRECT (damage) | `CHARGE_EFFECT`: turn 1 charges (effects.asm 1037-1086), turn 2 is a full vanilla calc (core.asm 3282-3319). | vanilla | none (turn cost is not damage) |
| Fly, Dig | CORRECT (damage) | `FLY_EFFECT` / Dig: as above plus `INVULNERABLE` on turn 1 (effects.asm 1051-1060). | vanilla | none |
| Bind, Wrap, Fire Spin, Clamp | INCORRECT for multi-turn kill math; first hit CORRECT | `TRAPPING_EFFECT` (SpecialEffectsCont, run before the calc): sets 2-5 turns (3/8,3/8,1/8,1/8; effects.asm 1150-1158). Turn 1: vanilla calc + accuracy. Turns 2..N: `.MultiturnMoveCheck` jumps to `GetPlayerAnimationType` — **no damage calc, no accuracy test, deals exactly the turn-1 damage** (incl. its crit) (core.asm 3725-3737). Target cannot move meanwhile (3536-3543). If turn 1 misses the sequence ends (MoveHitTest 5521-5526). | vanilla per use; `partial_trapping` flavor unused | See 4.6: treat like multi-hit (one roll x N) with the "N Hits" dropdown. Also fix Bind accuracy 75 (Bug 7). |
| Jump Kick, Hi Jump Kick | CORRECT (damage) | Vanilla. On a miss/immune target the user takes `wDamage>>3` which is always 0 -> 1 HP (core.asm 3911-3944). | vanilla; `miss_crash` unused | none |
| Take Down, Double Edge, Submission | CORRECT (damage) | Vanilla; recoil = floor(dmg/4) min 1 (recoil.asm 11-26). Recoil out of scope. | vanilla | none |
| Thrash, Petal Dance | CORRECT (per turn) | `THRASH_PETAL_DANCE_EFFECT`: 2-3 turns (effects.asm 837-841); every turn re-enters `PlayerCalcMoveDamage` = fresh vanilla calc, crit, roll and accuracy (core.asm 3702-3712). | vanilla | none |
| Hyper Beam | CORRECT | Vanilla 150 BP; recharge only if the target survived and the move hit (`SpecialEffects` path 3429-3436 is skipped on faint 3409-3413). | vanilla | none |
| Absorb, Mega Drain, Leech Life | CORRECT (damage) | Vanilla; heal = floor(dmg/2) min 1 (drain_hp.asm 2-13). | vanilla | none |
| Dream Eater | CORRECT (damage), condition not modelled | Vanilla 100 BP + drain; `MoveHitTest` fails unless target is asleep (core.asm 5422-5428). | vanilla | Optional: note in UI; see 4.8. |
| Rage | CORRECT (per hit) | Vanilla 20 BP each turn (core.asm 3739-3750 re-runs the full calc). Each time the user is hit its Attack stage rises by 1 via `StatModifierUpEffect` (`HandleBuildingRage` 5084-5124), which also re-applies badge boosts for a player Rage user. | vanilla; `rage` flavor unused | Expressible through the Attack stage dropdown; none. |
| Swift | CORRECT | Vanilla 60 BP; never misses (5429-5432); Ghost immunity still applies (0 damage flagged in AdjustDamageForMoveType before MoveHitTest). | vanilla; accuracy None -> 100 | none |
| Struggle | CORRECT (damage) | `RECOIL_EFFECT`, 50 BP Normal, accuracy 255/256, STAB for Normal types, no effect on Ghost; recoil floor(dmg/2) min 1 (recoil.asm 15-26). | vanilla; accuracy None -> 100 | none |
| Selfdestruct, Explosion | INCORRECT when a stat > 255 | D halved (min 1) **after** /4 scaling and low-byte truncation (core.asm 4484-4489); user faints regardless (effects.asm 185-202). | 94-95 halves before the (commented-out) scaling | Bug 1 fix (move halving after scaling). |
| SonicBoom, Dragon Rage | CORRECT | Fixed 20 / 40; no crit/STAB/type/immunity; accuracy roll applies (SonicBoom 229/256, Dragon Rage 255/256) (4822-4827). | damage_calc.py 42-55 by name | none |
| Seismic Toss, Night Shade | CORRECT | Damage = user level; no type/immunity (4814-4821). | damage_calc.py 43-44 | none |
| Psywave | CORRECT (player) / INCORRECT (enemy) | Player: uniform 1..floor(1.5L)-1 (4828-4841). Enemy: uniform 0..floor(1.5L)-1 (4947-4960). Accuracy 80 (204/256). | 54-56: `range(1, floor(1.5L))` for both sides; json accuracy 100 | Bug 5 + Bug 7. |
| Super Fang | NOT IMPLEMENTED (and emits garbage) | floor(target current HP / 2), min 1; skips crit/type/roll; accuracy 229/256; hits Ghosts (4795-4812). | base_power -1 falls into the vanilla formula (46-47 don't catch -1); returns `{1: 39}` for neutral targets, `None` for NVE ones (verified) | Bug 3 / 4.4. |
| Counter | NOT IMPLEMENTED | 2x the last `wDamage` if the target's last selected move was Normal/Fighting with BP > 0 and not Counter; cap 65535; no type check (hits Ghosts), no crit; accuracy 255/256 (4718-4781). | base_power None -> None | 4.2 |
| Bide | NOT IMPLEMENTED | Stores 2-3 turns (effects.asm 818-822); releases 2x accumulated `wDamage` (3652-3700); no accuracy test, no type check. | None | 4.3 |
| Guillotine, Horn Drill, Fissure | NOT IMPLEMENTED | `OHKO_EFFECT`: fails if user's in-battle Speed < target's (one_hit_ko.asm 15-38); else damage 65535 -> KO; then STAB/type run (immune types -> "no effect": Fissure vs Flying, Guillotine/Horn Drill vs Ghost) and accuracy 76/256 (30%) x stages. | None | 4.1 |

---

## 3. Bugs found (ranked by impact)

### Bug 1 — Missing ">255 -> both stats /4, low byte only" scaling, and Explosion halving in the wrong place
**Affected:** every damaging move, whenever the offensive or the (possibly screen-doubled) defensive stat is > 255: any +2..+6 stage on a mid-level Pokémon, X items, high-level fights (E4), Reflect/Light Screen on any defender with 128+ Def/Special. In the sweep (10 matchups, L30-80, stages 0..+6, screens, crits) 826 of 5280 cases differ. Includes the Reflect/Light Screen wrap at 512+ (huge differences) and the Explosion order.
**Root cause:** pkmn_damage_calc.py 97-105 is commented out, and 94-95 halves the defence before the scaling.
**Game logic:** core.asm 4278-4301 (scaling + low byte), 4484-4489 (Explosion after it).
**Exact fix** (replace lines 91-105):
```python
    if doubled_def:                      # Reflect / Light Screen (never on a crit)
        defending_stat *= 2              # 16-bit `sla c / rl b`, no cap  (core.asm 4223-4224)
    # GetDamageVarsForPlayerAttack .scaleStats (core.asm 4278-4298)
    if attacking_stat > 255 or defending_stat > 255:
        attacking_stat //= 4             # two `srl/rr` pairs
        defending_stat //= 4
        if attacking_stat == 0:
            attacking_stat = 1
    attacking_stat &= 0xFF               # `ld b, l`   (4300)
    defending_stat &= 0xFF               # only `c` reaches CalculateDamage (4301)
    # CalculateDamage EXPLODE_EFFECT (core.asm 4484-4489) -- AFTER scaling/truncation
    if move.name in (const.EXPLOSION_MOVE_NAME, const.SELFDESTRUCT_MOVE_NAME):
        defending_stat >>= 1
        if defending_stat == 0:
            defending_stat = 1
    if defending_stat == 0:
        # Reflect/Light Screen with a 512-513 defensive stat: the real game hangs in _Divide.
        return None
```
(`attacking_battle_stats`/`defending_battle_stats` overrides are already resolved to 16-bit stats before this point, so the fix applies to them too.)
**Test cases** (trainer-DV mons via `gen.create_trainer_pkmn`, Yellow):
1. Alakazam L61, +2 Special (unmod Special = floor(286*61/100)+5 = 179 -> 358), Psychic (90, STAB) vs Dewgong L54 (Special floor(206*54/100)+5 = 116). Game: scaled A=89, D=29; base floor(122/5)+2 = 26; 26*90*89 = 208260; //29 = 7181; //50 = 143; +2 = 145; STAB 217; rolls floor(217*r/255) = **184..217**. App today: 26*90*358 = 837720 //116 = 7221 //50 = 144 +2 = 146; STAB 219; 186..219.
2. Nidoking L50 Horn Attack (65) (Atk = 106) vs Onix L50 with +6 Def and Reflect (Def 173 -> 692 -> doubled 1384). Game: scaled A=26, D=346 -> low byte **90**; 22*65*26 = 37180 //90 = 413 //50 = 8 +2 = 10; Rock NVE -> 5; rolls **{4: 38, 5: 1}**. App today: 22*65*106 = 151580 //1384 = 109 //50 = 2 +2 = 4; NVE 2; {1: 38, 2: 1}.
3. Nidoking L50 +6 Attack (106 -> 424) Earthquake (100, STAB) vs Rhydon L50 (Def 133; Rock/Ground -> only Ground SE row). Game: A=106, D=33; 22*100*106 = 233200 //33 = 7066 //50 = 141 +2 = 143; STAB 214; SE 428; rolls **364..428**. App today: 932800 //133 = 7013 //50 = 140 +2 = 142; STAB 213; 426; 362..426.
4. Alakazam L40 (Special 119) Psychic vs Dewgong L40 +2 Special (87 -> 174) with Light Screen (348): Game: A=29, D=87 -> 15..18. App today: 16..19.

### Bug 2 — Type effectiveness applied in type_1/type_2 order instead of ROM table order
**Affected (both versions, from the app's own species data):** attacks where the defender is SE by one type and NVE by the other:
- game applies **NVE first** (app SE first -> app is 1 too high on odd pre-roll damage): Fighting vs Aerodactyl (Rock/Flying), Fighting vs Articuno (Ice/Flying), Fighting vs Jynx (Ice/Psychic), Poison vs Weedle/Kakuna/Beedrill/Venonat/Venomoth (Bug/Poison), Bug vs Zubat/Golbat (Poison/Flying).
- game applies **SE first** (app NVE first -> app is 1 too low on odd pre-roll damage): Fire vs Dewgong/Cloyster/Lapras (Water/Ice), Electric vs Dragonite (Dragon/Flying), Grass vs Nidoking/Nidoqueen (Poison/Ground).
(The 6 remaining order differences involve a 0 multiplier — Ground vs Flying/x, Fighting vs Ghost/Poison — and give 0 either way.)
**Root cause:** pkmn_damage_calc.py 127-135 apply `first_type_effectiveness` then `second_type_effectiveness`; the game walks `TypeEffects` top-down (core.asm 5298-5354).
**Fix:** apply the multipliers in ROM row order. ROM order of `TypeEffects` (data/types/type_matchups.asm), attacking->defending: Water>Fire, Fire>Grass, Fire>Ice, Grass>Water, Electric>Water, Water>Rock, Ground>Flying(0), Water>Water, Fire>Fire, Electric>Electric, Ice>Ice, Grass>Grass, Psychic>Psychic, Fire>Water, Grass>Fire, Water>Grass, Electric>Grass, Normal>Rock, Normal>Ghost(0), Ghost>Ghost, Fire>Bug, Fire>Rock, Water>Ground, Electric>Ground(0), Electric>Flying, Grass>Ground, Grass>Bug, Grass>Poison, Grass>Rock, Grass>Flying, Ice>Water, Ice>Grass, Ice>Ground, Ice>Flying, Fighting>Normal, Fighting>Poison, Fighting>Flying, Fighting>Psychic, Fighting>Bug, Fighting>Rock, Fighting>Ice, Fighting>Ghost(0), Poison>Grass, Poison>Poison, Poison>Ground, Poison>Bug, Poison>Rock, Poison>Ghost, Ground>Fire, Ground>Electric, Ground>Grass, Ground>Bug, Ground>Rock, Ground>Poison, Flying>Electric, Flying>Fighting, Flying>Bug, Flying>Grass, Flying>Rock, Psychic>Fighting, Psychic>Poison, Bug>Fire, Bug>Grass, Bug>Fighting, Bug>Flying, Bug>Psychic, Bug>Ghost, Bug>Poison, Rock>Fire, Rock>Fighting, Rock>Ground, Rock>Flying, Rock>Bug, Rock>Ice, Ghost>Normal(0), Ghost>Psychic(0), Fire>Dragon, Water>Dragon, Electric>Dragon, Grass>Dragon, Ice>Dragon, Dragon>Dragon.
Implementation: store this as an ordered list (e.g. `GEN1_TYPE_ROW_ORDER = {(atk, def): row_index}` in gen_one_constants.py, or make gen 1's `type_chart` an ordered list in type_info.json), then in the calc:
```python
    effs = []
    for def_type in {defending_species.first_type, defending_species.second_type}:
        e = type_chart[move.move_type].get(def_type)
        if e is not None:
            effs.append((GEN1_TYPE_ROW_ORDER[(move.move_type, def_type)], e))
    for _, e in sorted(effs):
        if e == const.SUPER_EFFECTIVE: temp *= 2
        elif e == const.NOT_VERY_EFFECTIVE: temp //= 2
        elif e == const.IMMUNE: return None
        if temp == 0: return None
```
**Test cases:**
1. Tangela L5 Vine Whip (35 Grass, STAB; Special 15) vs Nidoking L5 (Poison/Ground; Special 13). base floor(10/5)+2 = 4; 4*35*15 = 2100 //13 = 161 //50 = 3 +2 = 5; STAB 7. Game (Grass>Ground SE row 26 before Grass>Poison NVE row 28): 14 -> 7 -> rolls **{5: 2, 6: 36, 7: 1}**. App today (Poison NVE first): 3 -> 6 -> {5: 38, 6: 1}.
2. Nidoran M L13 Poison Sting (15 Poison, STAB; Atk 22) vs Weedle L13 (Bug/Poison; Def 14). base floor(26/5)+2 = 7; 7*15*22 = 2310 //14 = 165 //50 = 3 +2 = 5; STAB 7. Game (Poison>Poison NVE row 44 before Poison>Bug SE row 46): 3 -> 6 -> **{5: 38, 6: 1}**. App today: 14 -> 7 -> {5: 2, 6: 36, 7: 1}.
3. Charizard L13 Fire Blast (120 Fire, STAB; Special 29) vs Lapras L13 (Water/Ice; Special 31): 7*120*29 = 24360 //31 = 785 //50 = 15 +2 = 17; STAB 25. Game (Fire>Ice SE row 3 before Fire>Water NVE row 14): 50 -> 25 -> **21..25** ({21:8,22:10,23:10,24:10,25:1}). App today: 12 -> 24 -> 20..24.

### Bug 3 — Super Fang falls through into the vanilla formula with base_power -1
**Affected:** Super Fang (Raticate; Yellow Rocket/Route trainers).
**Root cause:** pkmn_damage_calc.py 46-47 only stop `None`/`0`; the `super_fang` flavor has no branch, so `temp *= -1` etc. Observed: `{1: 39}` vs neutral targets at L10-40, `None` at L60 or vs NVE/immune targets.
**Fix:** add a `const.FLAVOR_SUPER_FANG` branch right after the special-override (or make the `base_power <= 0` early-out cover -1 and return None until 4.4 is implemented). Correct value: `max(floor(target_current_HP/2), 1)`, no type check, no crit (4795-4812).
**Test case:** Raticate L30 Super Fang vs Rattata L30 (max HP 76 for trainer DVs... any full-HP target): game = floor(HP/2) at full HP; app today = 1.

### Bug 4 — Missing 997 cap before the +2
**Affected:** only when `floor(base*BP*A/D/50) >= 998` (e.g. L100 crit Explosion vs a L5 target; crit Selfdestruct/Explosion/Hyper Beam with maxed stats vs very low defence). Harmless for normal routing but trivial to fix.
**Root cause:** pkmn_damage_calc.py 118-119 (`temp += 2` without `min(temp, 997)`); game core.asm 4558-4631.
**Fix:** `temp = min(math.floor(temp / 50), 997) + 2`.
**Test case:** Snorlax L100 (Atk 243) crit Explosion (170, STAB) vs Chansey L5 (Def 6): base floor(400/5)+2 = 82; 82*170*243 = 3387420 //6 = 564570 //50 = 11291 -> cap 997 -> 999; STAB 1498; rolls **1274..1498**. App today: 11293 -> STAB 16939 -> 14766..17352 (non-crit) / 28827..33876 (crit).

### Bug 5 — Enemy Psywave range excludes 0
**Affected:** enemy Psywave (kill-chance/min-damage vs the player). Game: enemy rolls uniform on `0..floor(1.5L)-1` (core.asm 4947-4960), player on `1..floor(1.5L)-1` (4828-4841).
**Fix:** in `calculate_gen_one_damage` the attacker side is not known; simplest is a parameter or `attacking_pkmn.is_trainer_mon`-style check: `lo = 0 if attacker_is_enemy else 1; DamageRange({x: 1 for x in range(lo, floor(1.5L))})`.
**Test case:** L30 enemy Psywave: 45 equiprobable values 0..44 (P(0) = 1/45); app today: 44 values 1..44.

### Bug 6 — Stat-exp square-root term off by one at statExp > 65025
**Affected:** any stat whose stat exp is 65026..65535 (reachable through grinding, not vitamins; VIT_CAP = 25600). Game caps `ceil(sqrt(statExp))` at 255 (move_mon.asm 78-80) -> term 63; app uses 256 -> 64.
**Fix:** pkmn_utils.py 69: `temp += math.floor(min(math.ceil(math.sqrt(stat_xp)), 255) / 4)`.
**Test case:** base 100, DV 15, statExp 65535, L100: game (230+63)*100/100+5 = **298**; app 299. At statExp 65025 both give 298.

### Bug 7 — moves.json accuracy data differs from the ROM (affects kill-chance math only)
From `moves_cmp.py` vs data/moves/moves.asm (identical in RB and Yellow): **Bind 85 -> ROM 75** (191/256), **Psywave 100 -> ROM 80** (204/256), Poison Gas 90 -> ROM 55 (status). Swift and Struggle are `None` (ROM 100; None already maps to 100 — fine). Base power and type of every move match. (`FLAVOR_TWO_TURN_INVULN` in utils/constants.py:544 is misspelled "invlunerable" vs json "two_turn_semi_invulnerable"; unused by the gen 1 calc.)

---

## 4. Not-implemented moves / mechanics

### 4.1 OHKO moves — Guillotine, Horn Drill, Fissure
Game (one_hit_ko.asm 1-38, core.asm 4498-4500, 4638-4642): damage set to 0 and `wCriticalHitOrOHKO = $ff`; if user's **in-battle** Speed (`wBattleMonSpeed`, incl. stages, badges, paralysis) >= target's in-battle Speed, damage = 65535 and flag = 2; otherwise the move fails outright (no accuracy roll). On success the normal `AdjustDamageForMoveType` runs on 65535 (immunity -> "doesn't affect": Fissure vs Flying types, Guillotine/Horn Drill vs Ghost), then `MoveHitTest` with the 30% byte (76/256) scaled by accuracy/evasion stages. Damage is effectively the target's remaining HP.
Inputs needed: attacker/defender battle speeds (already computable from `get_battle_stats`), defender HP (`cur_stats.hp`), type immunity.
Plan: in `calculate_gen_one_damage`, on `const.FLAVOR_ONE_HIT_KO` ("one_hit_ko"): if immune -> None; if `attacking_battle_stats.speed < defending_battle_stats.speed` -> None (or a DamageRange of 0 with a UI note); else `DamageRange({defending_pkmn.cur_stats.hp: 1})`; leave accuracy at 30 (json already 30; exact 76/256 = 29.69%). Crit range should equal the normal range.

### 4.2 Counter
Game (core.asm 4718-4781): only when the target's **last selected** move is Normal- or Fighting-type with BP > 0 and is not Counter; damage = 2 x `wDamage` (the last damage value stored by anyone, capped at 65535), no crit, no STAB, no type check (hits Ghosts), then the normal accuracy roll (255/256). Requires the damage the target dealt: not representable without an enemy-damage input. Plan: a numeric `custom_move_data` "damage taken" -> `DamageRange({min(2*x, 65535): 1})`; or keep `None` and label "requires enemy damage input".

### 4.3 Bide
Game (effects.asm 800-825; core.asm 3652-3700): 2 or 3 storing turns; on release power is set to 1 and damage = 2 x accumulated `wDamage` values seen on the user's turns (typeless, no accuracy test, hits Dig/Fly and Ghosts; 0 accumulated -> miss). Requires damage taken: same plan as Counter.

### 4.4 Super Fang
Game (core.asm 4795-4812): `floor(target current HP / 2)`, min 1; no crit/type/roll; accuracy 229/256; Substitute applies. Plan: use `defending_pkmn.cur_stats.hp` (full HP) for the first use, with an optional "HP %" dropdown like Eruption for later uses; return `DamageRange({max(hp//2, 1): 1})`. Also fixes Bug 3.

### 4.5 Enemy Psywave 0 roll — see Bug 5.

### 4.6 Partial-trapping moves as multi-turn sequences (Bind, Wrap, Fire Spin, Clamp)
Game: turns 2..N repeat the turn-1 damage exactly (core.asm 3725-3737), N = 2..5 with 3/8,3/8,1/8,1/8 (effects.asm 1150-1158), the target cannot act (3536-3543), only turn 1 rolls accuracy. Plan: give `partial_trapping` the same treatment as `multi_hit` (one roll x N via the "N Hits" dropdown) so "kills in one Wrap sequence" is computed correctly; the current per-use independent-roll model is only right for a single turn.

### 4.7 Focus Energy
Game (CriticalHitTest 4677-4685, focus_energy.asm): normal moves b = floor(bs/8) (1/4 of normal), high-crit moves b = 4*floor(bs/4) (about half). Plan: a "Focus Energy" custom option (or a FieldStatus flag) consumed by `get_crit_rate`:
```python
b = bs >> 1
if focus_energy: b >>= 1
else: b = min(255, b << 1)
if high_crit: b = 255 if (b << 2) >= 256 else b << 2   # two capped shifts; equivalent since b < 128
else: b >>= 1
return b / 256
```

### 4.8 Accuracy exactness
Game: hit chance = `floor(acc*255/100)/256` (1/256 miss on 100% moves, e.g. 95% is really 94.53%), stage-scaled per 1.12; Swift/X Accuracy always hit; Dream Eater needs a sleeping target; Dig/Fly targets are missed. Plan: `get_move_accuracy` returns `None` for Swift (already), else `floor(acc*255/100)/256*100`; optionally take accuracy/evasion stages from `StageModifiers` (`14 - evasion` ratio index, floors, min 1 per step, cap 255).

### 4.9 Burn / paralysis penalties
Game: burn halves the in-battle Attack (min 1) (`HalveAttackDueToBurn` 6511-6548), paralysis quarters Speed (6468-6509); both are reapplied after every stat change (effects.asm 540-542, 730-734). Both are ignored on crits (party stats). The app has no status model; a "Burned" attacker toggle would just halve the non-crit physical attack stat.

### 4.10 Mid-battle level-up resets stacked badge boosts
Game (experience.asm 226-241): after a level-up of the active mon, all four stats are recomputed from the new unmodified stats x current stage, then `ApplyBadgeStatBoosts` once => every `*_badge_boosts` count should become 0. Verify the router does `StageModifiers.clear_badge_boosts()` on level-up (controllers/battle_summary_controller.py:1796 is the only caller seen).

### 4.11 X items and enemy stat-drops must go through `apply_stat_mod`
Game: X Attack/Defend/Speed/Special (item_effects.asm 1805-1831) and any enemy stat-drop/side-effect that lands on the player (effects.asm 722-726) re-apply badge boosts exactly like the player's own stat-up moves. Not verified in the router; listed so the badge-count model stays consistent.

### 4.12 Confusion self-hit (not a move)
Game (core.asm 3843-3885): 40 BP, no type, no crit, no random roll, no STAB, uses the user's own in-battle Attack vs its own in-battle Defense (both incl. badge boosts/stages), then the same scaling rules. Optional.

### 4.13 Substitute / Transform interactions — out of scope, not audited.

---

## 5. Things verified correct (brief)
- Base formula order and every truncation: `floor(2L/5)+2` (crit `floor(4L/5)+2`), `*BP`, `*A`, `floor(/D)`, `floor(/50)`, `+2`, STAB `+floor(d/2)`, type `*2` / `floor(/2)`, immunity -> no damage, "0 after a type step -> no damage".
- Random roll 217..255 / 255 with floor; minimum 1; identical distribution to the game in every tested case (5280-case sweep: all mismatches are explained by Bugs 1/2).
- Crit: level doubled; stages, badge boosts, Reflect/Light Screen all discarded; crit rate `floor(bs/2)/256`, x8 with cap 255 for Karate Chop/Razor Leaf/Crabhammer/Slash; `min(...,255)/256` matches the carry-cap semantics exactly; crit and hit rolls are independent so per-hit crit probability in `find_kill` is right.
- Physical/special split by type (`SPECIAL = $14`), special_types list.
- Stat formula (except the 65025+ stat-exp edge), stage ratio table, 999/1 clamps, trainer DVs 9/8/8/8 with HP DV 8, 0 stat exp for trainer and wild mons.
- Badge boost value `floor(stat*9/8)` cap 999; badge->stat mapping; stacking model (changed stat gets exactly one boost, the others one more); enemy never boosted; "nothing happened" cases do not stack.
- Multi-hit: one damage/crit/roll x N; 2-hit moves x2; Twineedle/Double Kick/Bonemerang.
- Fixed/level damage (Dragon Rage 40, SonicBoom 20, Seismic Toss/Night Shade = level) with type immunity deliberately ignored — this is exactly what gen 1 does (SetDamageEffects skip AdjustDamageForMoveType); player Psywave 1..floor(1.5L)-1 uniform.
- Explosion/Selfdestruct halving with min 1 (only the ordering vs scaling is wrong).
- Swift never misses; Struggle is Normal-typed 50 BP with STAB/immunity; two-turn, recoil, drain, Thrash/Petal Dance, Rage, Jump Kick, Hyper Beam, Pay Day, Quick Attack all use the vanilla formula for the damaging turn.
- Type chart contents (82/82 rows, including Ghost->Psychic immunity, Bug->Poison SE, Poison->Bug SE, Ice->Fire neutral, Fire->Ice SE).
- moves.json base power and type for all 165 moves; accuracy for all but Bind/Psywave/Poison Gas.
- Red/Blue vs Yellow: damage engine, move table, type table, crit table, stat ratios and CalcStat are identical; using one moves.json and one calc for all three versions is correct.
