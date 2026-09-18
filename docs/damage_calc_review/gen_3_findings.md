# Gen 3 damage-calc audit

Scope: `pkmn/gen_3/pkmn_damage_calc.py` (+ `data_objects.py`, `gen_three_constants.py`, `gen_three_object.py`,
shared `pkmn/damage_calc.py`, `raw_pkmn_data/gen_three/{moves,type_info}.json`) versus pokeemerald, with every
formula step cross-checked against pokefirered and pokeruby. All numeric "game" values below were produced by a
reference implementation transcribed line-by-line from the C (`<scratchpad>/ref_gen3.py`) and the "app" values by
actually running `gen.calculate_damage(...)` under Emerald (`<scratchpad>/run_cases.py`, `run_cases2.py`).

Unless stated otherwise, the test subject is **Emerald, trainer Machop L50 vs trainer Machop L50**
(app stats: HP 134 / Atk 89 / Def 59 / SpA 44 / SpD 44 / Spe 44; DVs 8/9/8/8/8/8, 0 EVs, Hardy, no badges).

---

## 0. Sources consulted

### pokeemerald (`A:\decomps\pokeemerald`)
- `src/pokemon.c:3100-3104` `APPLY_STAT_MOD`; `3106-3372` `CalculateBaseDamage` (whole function); `3407-3419`
  `ShouldGetStatBadgeBoost`; `1868-1883` `gStatStageRatios`; `2814-2821` `CALC_STAT` macro; `2823-2865`
  `CalculateMonStats`; `5878-5910` `ModifyStatByNature`.
- `src/battle_script_commands.c:588-604` `sAccuracyStageRatios`; `606` `sCriticalHitChance`; `749-756`
  `sFlailHpScaleToPowerTable`; `759-771` `sNaturePowerMoves`; `774-782` `sWeightToDamageTable`; `915-1000`
  `Cmd_attackcanceler` (+`1002-1016` `JumpIfMoveFailed` -> `ABILITYEFFECT_ABSORBING`); `1054-1097`
  `AccuracyCalcHelper`; `1099-1203` `Cmd_accuracycheck`; `1253-1288` `Cmd_critcalc`; `1290-1303`
  `Cmd_damagecalc`; `1318-1353` `ModulateDmgByType`; `1355-1425` `Cmd_typecalc`; `1427-1500`
  `CheckWonderGuardAndLevitate`; `1639-1651` `ApplyRandomDmgMultiplier`; `1658-1699` `Cmd_adjustnormaldamage`;
  `1701-1740` `Cmd_adjustnormaldamage2`; `1849-1860` `Cmd_datahpupdate` (Hidden Power physicality note);
  `3620-3628` `MoveValuesCleanUp`; `3638-3650` `Cmd_setmultihit/decrementmultihit`; `4500-4600` `Cmd_typecalc2`;
  `5861-5899` `Cmd_adjustsetdamage`; `6868-6892` `Cmd_stockpiletobasedamage`; `7139-7154`
  `Cmd_setmultihitcounter`; `7490-7575` `Cmd_tryKO`; `7577-7584` `Cmd_damagetohalftargethp`; `7926-7930`
  `Cmd_dmgtolevel`; `7932-7941` `Cmd_psywavedamageeffect`; `7943-7967` `Cmd_counterdamagecalculator`;
  `7969-7990` `Cmd_mirrorcoatdamagecalculator`; `8134-8140` `Cmd_setalwayshitflag`; `8304-8316`
  `Cmd_remaininghptopower`; `8536-8569` `Cmd_rolloutdamagecalculation`; `8580-8601` `Cmd_furycuttercalc`;
  `8603-8611` `Cmd_friendshiptodamagecalculation`; `8613-8650` `Cmd_presentdamagecalculation`; `8670-8721`
  `Cmd_magnitudedamagecalculation`; `8723-8765` `Cmd_jumpifnopursuitswitchdmg`; `8889-8911`
  `Cmd_hiddenpowercalc`; `8929-8955` `Cmd_trysetfutureattack`; `8957-9005` `Cmd_trydobeatup`; `9100-9106`
  `Cmd_setcharge`; `9112-9119` `Cmd_callenvironmentattack` (Nature Power); `9339-9349`
  `Cmd_doubledamagedealtifdamaged` (Revenge); `9366-9376` `Cmd_setdamagetohealthdifference` (Endeavor);
  `9378-9388` `Cmd_scaledamagebyhealthratio` (Eruption); `9466-9482` `Cmd_weightdamagecalculation` (Low Kick);
  `9619-9652` `Cmd_getsecretpowereffect`; `9787-9806` `Cmd_setweatherballtype`; `9847-9868` `Cmd_pursuitdoubles`.
- `src/battle_util.c:82-96` `HandleAction_UseMove` (resets `gCritMultiplier`/`dmgMultiplier`); `150-180`
  Lightning Rod target redirection; `688-692` `sSoundMovesTable`; `2205-2230` Bide (`CANCELER_BIDE`);
  `2645-2660` `ABILITYEFFECT_MOVES_BLOCK` (Soundproof); `2662-2720` `ABILITYEFFECT_ABSORBING` (Volt Absorb,
  Water Absorb, Flash Fire); `3835-3848` Lightning Rod in `GetMoveTarget`.
- `src/battle_main.c:335-450` `gTypeEffectiveness`; `4638-4646` speed badge boost.
- `src/battle_interface.c:2517-2525` `GetScaledHPFraction`.
- `src/data/battle_moves.h` (all 354 moves: effect/power/type/accuracy/target), `src/data/items.h` (hold effects
  & params: BRIGHTPOWDER 2188-2192, DRAGON SCALE 2460-2464, type items 2299-2671, SEA INCENSE 2705-2709,
  LAX INCENSE 2718-2722).
- `include/battle.h:466-467` (`IS_TYPE_PHYSICAL/SPECIAL`), `include/constants/pokemon.h:5-23` (type ids),
  `include/constants/battle.h:232-243` (weather flags), `include/battle_util.h:47-48` (`WEATHER_HAS_EFFECT`),
  `include/constants/hold_effects.h`.
- `data/battle_scripts_1.s`: `236-250` (EffectHit/Surf-vs-Dive), `573-582` (Bide), `604-640` (MultiHit loop),
  `762-775` (OHKO), `809-817` (Super Fang), `819-828` (Dragon Rage), `830-837` (Trap/Whirlpool-vs-Dive),
  `839-847` (DoubleHit), `1064-1073` (Sky Attack), `1122-1132` (Rage), `1195-1204` (Level damage), `1206-1215`
  (Psywave), `1217-1225` (Counter), `1345-1347` (Flail), `1384-1425` (Triple Kick), `1596-1606` (Rollout),
  `1631-1641` (Fury Cutter), `1657-1662` (Return/Frustration), `1664-1681` (Present), `1683` (Magnitude),
  `1801-1809` (Mirror Coat), `1825-1831` (Twister), `1833-1849` (Earthquake/Magnitude-vs-Dig), `1881-1890`
  (Future Sight), `1892-1896` (Gust), `1898-1901` (Stomp/Astonish/Needle Arm/Extrasensory), `1903-1918`
  (SolarBeam), `1940-1972` (Beat Up), `2072-2081` (Uproar), `2094-2103` (Spit Up), `2252-2258` (Facade),
  `2270-2276` (SmellingSalt), `2288-2295` (Nature Power), `2414-2416` (Revenge), `2418-2428` (Brick Break),
  `2479-2491` (Endeavor), `2493-2495` (Eruption/Water Spout), `2555-2561` (Low Kick), `2563-2565` (Secret
  Power), `2644-2646` (Weather Ball), `3086-3097` (Pursuit on switch), `3292-3314` (Bide attack), `3508-3535`
  (Future Sight hit).

### pokefirered (`A:\decomps\pokefirered`)
- `src/pokemon.c:2381-2382` `ShouldGetStatBadgeBoost` macro; `2385-2640` `CalculateBaseDamage` (diffed
  against Emerald: identical except identifier names and `BATTLE_TYPE_BATTLE_TOWER` in the Soul Dew check).
- `src/battle_script_commands.c:741-752` `sNaturePowerMoves`; `1003-1130` `Cmd_accuracycheck`; `1170-1207`
  `Cmd_critcalc`; `1274-1300` `Cmd_typecalc`; `1558-1570` `ApplyRandomDmgMultiplier`; `7557-7566`
  `Cmd_psywavedamageeffect`; `8503-8530` `Cmd_hiddenpowercalc`; `8718-8725` `Cmd_callterrainattack`;
  `9345-9364` `Cmd_setweatherballtype`. `src/battle_main.c:3440-3447` speed badge.
- `src/data/battle_moves.h`, `src/data/items.h` (same hold-effect params as Emerald).
- `data/battle_scripts_1.s:1936-1950` (Beat Up), `2090-2099` (Spit Up), `3461-3470` (Future Sight hit),
  `1384-1399` (Triple Kick), `1592-1602` (Rollout).

### pokeruby (`A:\decomps\pokeruby`)
- `src/calculate_base_damage.c:30-44` `gStatStageRatios`; `48-66` `gHoldEffectToType`; `70-90` `BADGE_BOOST`
  macro; `93-337` `CalculateBaseDamage`.
- `src/battle_script_commands.c:822` `sCriticalHitChance`; `969-976` flail table; `979-990` `sNaturePowerMoves`;
  `995-1003` weight table; `1222-1330` `atk01_accuracycheck`; `1373-1408` `atk04_critcalc`; `1475-1560`
  `atk06_typecalc`; `1751-1763` `ApplyRandomDmgMultiplier`; `1770-1800` `atk07_adjustnormaldamage`; `6697-6710`
  `atk8D_setmultihitcounter`; `7690-7697` `atkA0_psywavedamageeffect`; `8211-8243`
  `atkB3_rolloutdamagecalculation`; `8250-8270` `atkB5_furycuttercalc`; `8281-8322`
  `atkB7_presentdamagecalculation`; `8324-8370` `atkB9_magnitudedamagecalculation`; `8521-8545`
  `atkC1_hiddenpowercalc`; `8586-8636` `atkC4_trydobeatup`; `8955-8965` `atkD9_scaledamagebyhealthratio`.
- `src/battle_main.c:4628-4634` speed badge. `src/data/battle_moves.c`, `src/data/items_en.h`.
- `data/battle_scripts_1.s:2031-2050` (Beat Up), `2196-2205` (Spit Up), `3549-3565` (Future Sight hit).

### App
- `pkmn/gen_3/pkmn_damage_calc.py` (all 565 lines), `pkmn/gen_3/data_objects.py` (1-629),
  `pkmn/gen_3/gen_three_constants.py` (1-205), `pkmn/gen_3/gen_three_object.py` (1-140, 165-205, 380-390,
  537-555), `pkmn/damage_calc.py` (1-356), `pkmn/universal_data_objects.py` (100-140, 336-420, 457-560),
  `utils/constants.py` (520-670), `raw_pkmn_data/gen_three/{moves,type_info,items}.json`,
  `raw_pkmn_data/gen_three/emerald/{pokemon,trainers}.json` (field names / held-item spellings),
  `controllers/battle_summary_controller.py` (690-1000), `tests/conftest.py`, `tests/test_unit_calcs.py`
  (395-460).

### RS vs Emerald vs FRLG differences that matter for damage (all verified against the three repos)
1. **Badge boosts in wild battles.** Ruby/Sapphire `BADGE_BOOST` (`calculate_base_damage.c:70-90`) only applies
   when `gBattleTypeFlags & BATTLE_TYPE_TRAINER` (and not Secret Base): Atk/Def/SpA/SpD badge boosts do **not**
   apply in wild battles in RS. Emerald (`pokemon.c:3407-3419`) and FRLG (`pokemon.c:2381`) apply them in wild
   battles too. Speed's Dynamo/Thunder boost applies in wild battles in all three (`battle_main.c`). App: always
   applies -> wrong for RS wild battles.
2. Everything else in the damage pipeline is identical across the three games: same `CalculateBaseDamage`
   (FRLG diff is cosmetic), same crit table/stages, same random multiplier, same type chart, same Hidden Power,
   same Weather Ball, same Nature Power terrain table (`sNaturePowerMoves` is the same 10 entries in all three),
   same Flail/Low Kick/Magnitude/Present/Psywave/Rollout/Fury Cutter/Beat Up/Spit Up/Future Sight scripts.
3. Move data: `battle_moves.h/.c` power / type / accuracy / effect / target are identical across RS, E and FRLG
   for all 354 moves (scripted comparison), and `raw_pkmn_data/gen_three/moves.json` matches them for every
   move (power, type, accuracy, target, high-crit flag, multi-hit/two-hit flags, always-hit flags, spread
   targets). The only mismatch is Endeavor (game power 1 placeholder, app 0) which is harmless.

---

## 1. Core formula verification

Notation: `//` = integer (truncating) division as in the C.

### 1.1 Pipeline order (game) — `pokemon.c:3106-3372`, `battle_script_commands.c:1290-1303, 1355-1425, 1639-1699`

```
CalculateBaseDamage:
  power = move power (or gDynamicBasePower override); type = move type (or dynamic type)
  attack/defense/spAttack/spDefense = raw battle stats (already include nature; NO stages yet)
  [stat modifiers, all integer, in THIS order]
    Huge/Pure Power:            attack *= 2
    badge (player only):        attack = 110*attack/100 ; defense = 110*defense/100 ; spAttack, spDefense likewise
    type-boost item (matches):  physical type -> attack = attack*(param+100)/100 ; special -> spAttack ...  (param 10, Sea Incense 5)
    Choice Band:                attack = 150*attack/100
    Soul Dew (Lati@s, atk):     spAttack = 150*spAttack/100 ;  (def) spDefense = 150*spDefense/100
    DeepSeaTooth (Clamperl):    spAttack *= 2 ;  DeepSeaScale (Clamperl, def): spDefense *= 2
    Light Ball (Pikachu):       spAttack *= 2
    Metal Powder (Ditto, def):  defense *= 2       (no "transformed" check)
    Thick Club (Cubone/Marowak):attack *= 2
    Thick Fat (def) & Fire/Ice: spAttack /= 2      (only spAttack)
    Hustle:                     attack = 150*attack/100
    Plus with Minus on field / Minus with Plus:  spAttack = 150*spAttack/100
    Guts + any status:          attack = 150*attack/100
    Marvel Scale (def)+status:  defense = 150*defense/100
    Mud Sport / Water Sport:    power /= 2 (Electric / Fire)
    Overgrow/Blaze/Torrent/Swarm, hp <= maxHP/3:  power = 150*power/100
    Explosion/Selfdestruct:     defense /= 2
  [physical branch (type < TYPE_MYSTERY)]
    A = stage(attack)  -- on crit: stage applied only if attacker stage > 0 (negative/zero stages ignored)
    damage = A * power * (2*level/5 + 2)
    D = stage(defense) -- on crit: stage applied only if defender stage < 0 (positive stages ignored)
    damage = damage / D ; damage /= 50
    burn (no Guts): damage /= 2
    Reflect (no crit): doubles with 2 alive on def side -> damage = 2*(damage/3) ; else damage /= 2
    doubles & MOVE_TARGET_BOTH & 2 alive: damage /= 2
    if damage == 0: damage = 1          <-- physical only
  [special branch (type > TYPE_MYSTERY)] same with spAttack/spDefense/Light Screen, no min-1, then:
    weather (unless Cloud Nine/Air Lock on field):
      rain(TEMPORARY flag): Fire /2, Water = 15*d/10
      rain|sand|hail and SolarBeam: /2
      sun: Fire = 15*d/10, Water /2
    Flash Fire active & Fire: damage = 15*damage/10
  return damage + 2
Cmd_damagecalc:   dmg = base * gCritMultiplier(1|2) * dmgMultiplier(1|2) ; Charge&Electric *2 ; Helping Hand = dmg*15/10
Cmd_typecalc:     Struggle: skip everything. STAB: dmg = dmg*15/10. Levitate vs Ground -> no effect.
                  for each gTypeEffectiveness row (atk type match) in TABLE ORDER: if def type1 matches -> dmg = dmg*mult/10 (min 1 unless mult 0);
                  if def type2 matches and type1 != type2 -> same. Wonder Guard -> miss unless (SE flag set and NVE flag clear).
Cmd_adjustnormaldamage: dmg = dmg * (100 - Random()%16) / 100 ; if 0 -> 1.  (False Swipe/Endure/Focus Band leave 1 HP)
```

### 1.2 Per-stage verdicts

| Stage | Game (citation) | App (file:line) | Verdict |
|---|---|---|---|
| Base stat calc | `pokemon.c:2814-2821`: `((2*base+iv+ev/4)*level)/100 + 5`, then nature `*110/100` or `*90/100` (`5878-5910`); HP `+level+10` | `data_objects.py:480-494` identical (`floor(x*1.1)`/`floor(x*0.9)` give identical integers for all stat sizes, checked) | CORRECT |
| Stage multiplier table | `pokemon.c:1868-1883` `{10,40},{10,35},{10,30},{10,25},{10,20},{10,15},{10,10},{15,10}…{40,10}`; `APPLY_STAT_MOD` = `stat*num/den` (`3100-3104`) | `data_objects.py:23-37` `(2,8)…(8,2)`, `modify_stat_by_stage` `floor(raw*n/d)` (`464-477`) — same ratios | CORRECT (ratios). See ordering bug below. |
| No 999 clamp | None in gen 3 (`CalculateBaseDamage` uses u16 stats, no clamp) | `STAT_MAX = 999` defined (`data_objects.py:21`) but never applied in gen 3 stat code | CORRECT |
| Badge boosts | `pokemon.c:3161-3168` + `3407-3419`: Stone/Boulder→Atk, Balance/Soul→Def, Mind/Volcano→SpA **and** SpD, player side only, not link/frontier; speed Dynamo/Thunder `battle_main.c:4638-4646`. **RS: trainer battles only** (`pokeruby calculate_base_damage.c:70-90`). FRLG has them (`pokefirered/src/pokemon.c:2381`). Applied on raw stat BEFORE stage multiplier: `(110*atk)/100` | `data_objects.py:121-134` mapping identical; `calc_battle_stat` applies badge before stage (`448-461`) ✓; crits keep badges in the normal path ✓ (verified numerically, case [9]) | CORRECT for E/FRLG; **INCORRECT for RS wild battles** (bug #17) |
| Stat-modifier vs stage ORDER | All item/ability multipliers are applied to the raw stat, THEN the stage ratio (`3161-3218` then `3224-3232`) | `pkmn_damage_calc.py:292-295` computes stage-modified stats first, then applies Thick Club/Light Ball/DeepSea/Hustle/Huge Power/Choice Band/Thick Fat (`297-339`) and finally the type item ×1.1 (`380-381`) on the post-stage stat | **INCORRECT** rounding (bug #9). Examples: Def 63 −1 Explosion: game 20 vs app 21 ([B]); SpA 45 −1 vs Thick Fat: game 14 vs app 15 ([C]); Atk 92 −1 Choice Band: game 92 vs app 91 ([D]); Huge Power +1 Atk 89: game 115-136 vs app 114-135 ([30]) |
| Type-boost items | `3171-3182` via `sHoldEffectToType`, param 10 (`items.h`), Sea Incense param **5** (`items.h:2705-2709`), Dragon **Fang** = Dragon (`2654-2658`), Dragon **Scale** has `HOLD_EFFECT_DRAGON_SCALE` (no boost, `2460-2464`) | `type_info.json held_item_boosts`: has `"Dragon Scale": "Dragon"` (wrong), lacks `Dragon Fang`, lacks `Sea Incense`, key `"SilverPowder"` never matches the item-db spelling `"Silverpowder"` (trainers.json even uses `"Silver Powder"`) | **INCORRECT** (bug #12) — verified: Dragon Fang/Sea Incense/Silverpowder give no boost, Dragon Scale gives one ([24]) |
| Choice Band | `3185`: `attack = 150*attack/100` | `331-332` `floor(atk*1.5)` (post-stage) | CORRECT value, wrong order (bug #9) |
| Soul Dew | `3186-3189`: Lati@s SpA ×1.5 (attacker) / SpD ×1.5 (defender) | `SOULD_DEW_NAME` defined (`gen_three_constants.py:55`) but never used; app instead applies ×1.5 SpA/SpD when Lati@s holds **DeepSeaScale** (`305-317`) | **INCORRECT** (bug #11): Latios+Soul Dew Psychic vs Machop: game 481-566, app 323-380 ([A]) |
| DeepSeaTooth / DeepSeaScale | `3190-3193`: Clamperl SpA ×2 (attacker) / Clamperl SpD ×2 (**defender**) | `301-304`: only modifies the *attacking* Clamperl's stats; defending Clamperl+DeepSeaScale not doubled (`312-320` handles only Lati@s/Ditto) | **INCORRECT** (bug #11): Confusion vs Clamperl+DSS: game 11-14, app 22-26 ([23]) |
| Light Ball | `3194-3195` Pikachu SpA ×2 | `299-300` ✓ (order caveat) | CORRECT |
| Metal Powder | `3196-3197` Ditto Def ×2 (no transform condition in gen 3) | `318-320` ✓ | CORRECT |
| Thick Club | `3198-3199` **Cubone or Marowak** Atk ×2 | `297-298` Marowak only | **INCORRECT** (bug #13): Cubone L30 Bone Club: game 30-36, app 16-19 ([21]) |
| Huge/Pure Power | `3158-3159` Atk ×2 (first modifier) | `325-329` ✓ value, post-stage | CORRECT value, order bug #9 |
| Hustle | `3204-3205` Atk `150/100` | `322-323` ✓ | CORRECT (order caveat) |
| Thick Fat | `3202-3203` halves **spAttack** only, pre-stage | `334-339` halves both SpA and Atk, post-stage | value CORRECT for Fire/Ice (always special), order bug #9 |
| Plus/Minus, Guts, Marvel Scale, Mud/Water Sport, Overgrow/Blaze/Torrent/Swarm, burn | `3206-3218`, `3245-3246` | not implemented (`341-356` disabled with `and False`) | MISSING (section 4) |
| Explosion def halving | `3220-3221`: `defense /= 2` BEFORE stage | `373-374`: after stage, `max(...,1)` | value usually equal; rounding differs (bug #9, case [B]) |
| Physical/special split | `battle.h:466-467`: type id < 9 physical; Fire, Water, Grass, Electric, Psychic, Ice, Dragon, Dark special; uses the **resulting** dynamic type (Hidden Power / Weather Ball) | `type_info.json special_types` = same 8 types; `358` uses `move_type` which is already the HP/Weather Ball resolved type | CORRECT |
| Base formula & truncation | `A*power*(2L/5+2)`, `/D`, `/50` | `384-391`: `floor(2L/5)+2`, `*bp*atk`, `floor(/def)`, `floor(/50)` — same product/division order | CORRECT |
| Crit stat-stage rule | attacker stage used only if `> 6` (i.e. positive); defender stage used only if `< 6` (negative) (`3226-3231`, `3240-3247`) | `280-290`: resets attacker mods when stage `< 0`, defender when `> 0` (0 is a no-op either way) — equivalent | CORRECT numerically (case [L]). The reset replaces the whole `StageModifiers` object, wiping unrelated stats, but no other stat feeds that move's damage, so no numeric effect. Caveat: the reset object is then passed to the multi-hit recursion (bug #14). |
| Burn | `3253-3254` physical `/2` unless Guts | none | MISSING (no status input) |
| Reflect / Light Screen | `3257-3263`, `3308-3314`: skipped on crit; `/2` singles; doubles with 2 alive `2*(d/3)` | `361-371, 394-395`: `/2` always; skipped on crit ✓; Brick Break excluded ✓ (`removelightscreenreflect` runs before damagecalc, `battle_scripts_1.s:2418-2428`) | CORRECT singles, **INCORRECT doubles** (bug #16): 22-27 vs game 30-36 ([12]) |
| Doubles spread | `3266-3267`: only `MOVE_TARGET_BOTH` (Earthquake/Magnitude/Explosion are `FOES_AND_ALLY` and are NOT halved) and only with 2 alive foes | `397-398`: `target_both_enemies` only; moves.json targets match the game's `MOVE_TARGET_BOTH` set exactly | CORRECT (assumes both foes alive) |
| Physical min-1 before +2 | `3270-3271` (physical branch only) | absent | **INCORRECT** edge (bug #18): Rattata L2 Tackle vs Machop: game {4:1,3:15}, app {3:1,2:15} ([F]) |
| Weather | `3325-3364`: special branch only (fine — all affected types are special); rain uses `B_WEATHER_RAIN_TEMPORARY` (Rain Dance/overworld rain set it; Drizzle sets both, `battle_util.c:2486`), sun/sand/hail any; SolarBeam `/2` in rain/sand/hail; Cloud Nine/Air Lock negate | `400-420` + `damage_calc.is_weather_active` | CORRECT (cases [29]: SolarBeam rain/sand, Surf sun, Flamethrower rain all match) |
| Flash Fire boost | `3367-3368` `15*d/10` when attacker's Flash Fire was triggered | stub `423-425` | MISSING (needs "Flash Fire activated" input) |
| +2 | `3371` | `427` | CORRECT |
| Crit multiplier | `Cmd_damagecalc:1296` ×2, applied right after +2, before dmgMultiplier/STAB/type | `429-434` ✓; excludes Spit Up/Future Sight/Doom Desire ✓ (those scripts never call `critcalc`; `gCritMultiplier` is reset to 1 at `battle_util.c:91` and `MoveValuesCleanUp`); Battle/Shell Armor ✓ | CORRECT |
| dmgMultiplier (Gust/Twister vs Fly, Surf/Whirlpool vs Dive, EQ/Magnitude vs Dig, Stomp/Astonish/Needle Arm/Extrasensory vs Minimize, Facade, SmellingSalt, Revenge, Pursuit-on-switch, Weather Ball) | `setbyte sDMG_MULTIPLIER, 2` in the scripts; applied in `Cmd_damagecalc` after crit, before STAB | `458-482` ✓ order for all except Weather Ball which doubles base power instead (`131-133`) | CORRECT except Weather Ball (bug #8) |
| Charge / Helping Hand | `1298-1301` | stubs `485-492` (off) | MISSING (doubles-only / needs input) |
| STAB | `1370-1374` `dmg*15/10` | `494-495` `floor(*1.5)` | CORRECT |
| Type effectiveness | `1385-1406`: per table row, `dmg*mult/10`, min 1 per step when mult≠0; row order = `gTypeEffectiveness` order | `497-503`: iterates json dict order; `floor(/2)` / `*2`; single `min 1` at the end (`505-507`) | CORRECT for all real cases: json row order equals the game's for every attacking type except (a) Immune rows placed first (irrelevant: immunity returns None earlier) and (b) Grass: Dragon/Steel swapped (both NVE, commutative). Only difference: intermediate min-1 (game 1→NVE→1→SE→2 vs app 1→0→0→1) — only when damage is 1 before type calc. |
| Immunities / abilities | Levitate (`1375`), Wonder Guard (`1409-1416`, blocked unless SE-and-not-NVE, power≠0), Volt/Water Absorb (`battle_util.c:2667-2688`, power≠0), Flash Fire (`2689-2712`), Damp (`battle_script_commands.c:6546`), Soundproof (`2645-2660`, Growl/Roar/Sing/Supersonic/Screech/Snore/Uproar/Metal Sound/GrassWhistle/Hyper Voice), Lightning Rod = **doubles redirection only, no immunity** (`battle_util.c:150-180`, `3835-3848`; no absorb case) | `135-186`: Levitate ✓, Damp ✓, Volt Absorb ✓, Water Absorb ✓ (but see Flash Fire), Wonder Guard ✓ logic; **Lightning Rod treated as immunity** (`153-160`); **Flash Fire constant = "Water Absorb"** (`gen_three_constants.py:63`) so Fire moves vs Water Absorb mons return None and real Flash Fire mons take damage ([11],[I]); Soundproof missing | **INCORRECT** (bugs #5, #6, #7) |
| Random roll | `1639-1651`: `dmg*(100 - rand%16)/100`, min 1; 16 equiprobable outcomes 85..100 | `529-537` identical | CORRECT |
| Min damage | 1 after random (`1649-1650`) and after each type step | `530` `max(...,1)` | CORRECT |
| Damage cap | none in gen 3 | none | CORRECT |
| Multi-hit | `battle_scripts_1.s:604-640`: each hit runs `critcalc`, `damagecalc`, `typecalc`, `adjustnormaldamage` → independent crit and random roll per hit; hit count `Random()&3` → 2/3 with 3/8 each, 4/5 with 1/8 each (`7139-7154`) | `509-563`: sums per-hit ranges; crit modelled as exactly one hit critting; the non-crit hits are computed by a recursive call that (a) drops `weather` and `is_double_battle`, (b) passes the crit-reset stage modifiers ([25]: game 24-30, app 27-33); `DamageRange.add` uses `my_count + your_count` instead of `my_count * your_count` so the joint distribution/size is wrong (Double Kick size 192 instead of 256, [27]) | **INCORRECT** (bugs #14, #15) |

### 1.3 `get_crit_rate` — `pkmn_damage_calc.py:19-35` vs `Cmd_critcalc` (`1253-1288`)
- Game: `critChance = 2*FocusEnergy + HighCrit(EFFECT_HIGH_CRITICAL|SKY_ATTACK|BLAZE_KICK|POISON_TAIL) + ScopeLens + 2*(LuckyPunch&&Chansey) + 2*(Stick&&Farfetch'd)`, clamped to 4; chance `1/sCriticalHitChance[stage]` = 1/16, 1/8, 1/4, 1/3, 1/2; **0** if target has Battle Armor/Shell Armor (or `STATUS3_CANT_SCORE_A_CRIT`).
- App: table 1/16,1/8,1/4,1/3,1/2 ✓; counts `high_crit` flavour (all 11 game moves flagged ✓) and Nature Power/Long Grass (Razor Leaf) ✓. **Missing:** Scope Lens (+1), Lucky Punch on Chansey (+2), Stick on Farfetch'd (+2) — the held item is available on the mon; Focus Energy (+2, needs a field-status flag); Battle Armor/Shell Armor → rate 0 (function has no defender argument). Verified: Chansey+Lucky Punch Pound returns 1/16, game 1/4 ([17]).

### 1.4 `get_move_accuracy` — `pkmn_damage_calc.py:38-80` vs `Cmd_accuracycheck` (`1099-1203`) / `AccuracyCalcHelper` (`1054-1097`) / `Cmd_tryKO` (`7490-7575`)
Game: `buff = clamp(accStage + 6 - evasionStage)` (evasion ignored under Foresight); `moveAcc` (Thunder in sun → 50 if weather has effect); `calc = ratio[buff].dividend*moveAcc / divisor` (`sAccuracyStageRatios` 33/100..3/1); Compound Eyes `calc*130/100`; Sand Veil in sandstorm (weather has effect) `calc*80/100`; Hustle `calc*80/100` only if `IS_TYPE_PHYSICAL(type)` where type is the **resolved** type (Hidden Power's real type); BrightPowder `calc*(100-10)/100`, Lax Incense `*(100-5)/100`; hit if `Random()%100+1 <= calc` (no clamp needed). Always-hit: Lock-On/Mind Reader target, `EFFECT_ALWAYS_HIT` (Swift, Faint Attack, Shadow Punch, Aerial Ace, Magical Leaf, Shock Wave), Vital Throw, Thunder in rain. Semi-invulnerable targets are missed unless the move ignores that state. OHKO (`tryKO`): fails vs Sturdy; if Lock-On'd and Latk ≥ Ldef always; else `chance = acc + (Latk - Ldef)`, hits if `Random()%100 + 1 < chance` **and** `Latk >= Ldef` → effective probability `(chance-1)/100` (Fissure at equal level = **29%**, not 30%). **Blizzard has no hail interaction in gen 3** (only `EFFECT_THUNDER` is special-cased).
App findings:
- Nature Power branch (`39-48`): every terrain sets `result` then line 48 unconditionally overwrites with `100` → Rock Slide (90), Razor Leaf (95), Hydro Pump (80) all reported as 100. **Bug #19.**
- Blizzard in hail → `None` (always hits) (`54-55`). Gen 4 rule, not gen 3. **Bug #20.**
- Thunder in sun not reduced to 50 (`[16]`: app 70). **Bug #20.**
- Compound Eyes clamps to 100 *before* Sand Veil (`63-66`): Gust with Compound Eyes vs Sand Veil in sand: game `130→104` (always hits), app 80. **Bug #21.**
- Sand Veil / Thunder-rain checks use raw `weather`, ignoring Cloud Nine/Air Lock (`54-57, 77-78`): Cloud Nine attacker vs Sand Veil in sand → app 80, game 100.
- Hustle uses `move.move_type` (db type, Normal for Hidden Power) instead of the resolved HP/Weather Ball type (`67-75`).
- `*3277/4096` instead of `*80/100` (`75, 78`): numerically identical for integer accuracies ≤ 100 (verified), fine.
- Accuracy/evasion stages, BrightPowder/Lax Incense, Lock-On, OHKO level formula, Sturdy: not implemented.

---

## 2. Per-move table

215 damaging moves in `raw_pkmn_data/gen_three/moves.json`; 139 status moves (base_power 0) were skipped and are
correctly treated as non-damaging (`pkmn_damage_calc.py:125-126`) — with two exceptions noted at the end of the
table (Nature Power is wrongly non-damaging; Endeavor is non-damaging but should be a damage move).

Legend for "App status": C = CORRECT, I = INCORRECT, NI = NOT IMPLEMENTED (app returns a bogus number or None
where the game deals damage).

| Move | App status | What the game does | What the app does | Fix / plan |
|---|---|---|---|---|
| **Vanilla physical/special, no special casing (77):** Pound, Mega Punch, Fire Punch, Ice Punch, ThunderPunch, Scratch, ViceGrip, Cut, Wing Attack, Slam, Vine Whip, Mega Kick, Rolling Kick, Headbutt, Horn Attack, Tackle, Body Slam, Poison Sting, Bite, Ember, Flamethrower, Water Gun, Hydro Pump, Ice Beam, Psybeam, BubbleBeam, Aurora Beam, Peck, Drill Peck, Strength, Thunder Shock, Thunderbolt, Rock Throw, Confusion, Psychic, Quick Attack, Egg Bomb, Lick, Smog, Sludge, Bone Club, Fire Blast, Waterfall, Constrict, Dizzy Punch, Hyper Fang, Mach Punch, Sludge Bomb, Mud-Slap, Octazooka, Zap Cannon, Spark, Steel Wing, DynamicPunch, Megahorn, DragonBreath, Iron Tail, Metal Claw, Crunch, ExtremeSpeed, AncientPower, Shadow Ball, Rock Smash, Superpower, Luster Purge, Mist Ball, Poison Fang, Crush Claw, Meteor Mash, Overheat, Rock Tomb, Silver Wind, Signal Beam, Dragon Claw, Mud Shot, Water Pulse, Psycho Boost | C (subject to the global bugs #9/#12/#5-7/#18) | `BattleScript_EffectHit` → `critcalc/damagecalc/typecalc/adjustnormaldamage` | vanilla formula `pkmn_damage_calc.py:383-537` | — |
| **High-crit vanilla (8):** Karate Chop, Crabhammer, Slash, Aeroblast, Cross Chop, Blaze Kick, Poison Tail, Leaf Blade | C | +1 crit stage (`Cmd_critcalc:1268-1271`) | `high_crit` flavour → stage 1 (`21-22`) | — |
| **Spread vanilla (7 + Razor Leaf, Air Cutter, Swift):** Acid, Bubble, Rock Slide, Powder Snow, Icy Wind, Heat Wave, Muddy Water, Razor Leaf (high crit), Air Cutter (high crit), Swift (always hit) | C | `MOVE_TARGET_BOTH` → `/2` in doubles with two foes (`pokemon.c:3266`) | `397-398` | — |
| **Trap (5):** Bind, Wrap, Fire Spin, Clamp, Sand Tomb | C | vanilla (Whirlpool is the only trap move with a bonus) | vanilla | — |
| **Recoil / rampage / recharge / drain / self-thaw / miss-recoil (17):** Take Down, Submission, Double-Edge, Volt Tackle, Thrash, Petal Dance, Outrage, Hyper Beam, Blast Burn, Hydro Cannon, Frenzy Plant, Absorb, Mega Drain, Leech Life, Giga Drain, Flame Wheel, Sacred Fire, Jump Kick, Hi Jump Kick, Tri Attack | C | vanilla damage | vanilla | — |
| **Always-hit vanilla (6):** Faint Attack, Vital Throw, Shadow Punch, Aerial Ace, Magical Leaf, Shock Wave | C | `EFFECT_ALWAYS_HIT`/`EFFECT_VITAL_THROW` skip accuracy (`AccuracyCalcHelper:1090-1094`) | accuracy `None` in moves.json → 100 % | — |
| Thief, Covet, Knock Off, Pay Day, Rapid Spin, Fake Out, Focus Punch, Snore, Uproar, Dream Eater, Secret Power, Sky Uppercut, Skull Bash, Razor Wind, Sky Attack, Fly, Dig, Dive, Bounce, Hyper Voice | C | vanilla damage; Secret Power's terrain only changes the secondary effect (`9619-9652`), power stays 70; Sky Attack has +1 crit stage (`1270`), Razor Wind does not; Dream Eater requires a sleeping target; Snore requires a sleeping user; Fake Out first turn only; Hyper Voice/Snore/Uproar blocked by Soundproof (`battle_util.c:688-692`) | vanilla; Sky Attack flagged high_crit ✓; Soundproof not modelled (Hyper Voice vs Soundproof → damage, [11]) | add Soundproof immunity (bug #7) |
| **Multi-hit (12):** DoubleSlap, Comet Punch, Fury Attack, Pin Missile, Spike Cannon, Barrage, Fury Swipes, Bone Rush, Arm Thrust, Bullet Seed, Icicle Spear, Rock Blast | I | each hit rolls crit + random independently (`battle_scripts_1.s:604-640`); 2-5 hits (3/8,3/8,1/8,1/8) | per-hit ranges summed (`509-563`); dropdown 2-5 hits ✓; crit = exactly one hit; non-crit hits lose weather/doubles args and inherit crit-reset stages; `DamageRange.add` weights wrong | bugs #14, #15 |
| **Two-hit (3):** Double Kick, Twineedle, Bonemerang | I (same as multi-hit) | `setmultihitcounter 2` → same loop | `two_hit` flavour → 2 hits | bugs #14, #15 |
| Explosion, Selfdestruct | C (rounding caveat) | `defense /= 2` before stage (`3220-3221`); `FOES_AND_ALLY` → not spread-halved; Damp blocks | `373-374` halves after stage; Damp ✓ (`145-152`) | bug #9 |
| Struggle | C | `typecalc` skipped entirely (`1360-1364`): no STAB, no type effectiveness, no Wonder Guard check; physical (TYPE_NORMAL) | type `none` → no STAB/effectiveness, physical ✓ ([10] 29-35 = game). Edge: app returns None vs Wonder Guard, game hits | — |
| SolarBeam | C | `/2` in rain, sand, hail (`3346-3347`) | `405-415` ✓ ([29]) | — |
| Thunder | I (accuracy) | never misses in rain (`1090`); 50 % in sun (`1146-1147`) | rain ✓; sun stays 70 | bug #20 |
| Blizzard | I (accuracy) | no weather interaction in gen 3 | always hits in hail (`54-55`) | bug #20 |
| Brick Break | C | screens removed before `damagecalc` (`2418-2428`) | screens ignored (`361, 368`) | — |
| Weather Ball | I | `setweatherballtype` (`9787-9806`): `dmgMultiplier = 2` and type Water/Rock/Fire/Ice (weather has effect); applied after +2 and crit | base power ×2 before the formula (`131-133`); type ✓ | bug #8: Castform Weather Ball in sand vs Machop: game 26-31, app 25-30 ([5]) |
| Hidden Power | C | `Cmd_hiddenpowercalc:8889-8911`: power `(40*bits)/63+30`, type index `(15*typeBits)/63 + 1` skipping Normal & Mystery → Fighting..Dark; category by resulting type (`IS_TYPE_PHYSICAL` on the dynamic type) | `data_objects.py:582-629` identical bit-by-bit (verified on 7 DV sets incl. all-31, all-0, 31/30 mixes: e.g. (2,3,2,0,1,3) → Grass 39 both) | — (accuracy Hustle caveat only) |
| Magnitude | C | `8670-8721`: 10/30/50/70/90/110/150 (5/10/20/30/20/10/5 %); Dig ×2 via `dmgMultiplier` (`1833-1849`); `FOES_AND_ALLY` not spread-halved | `200-214`, `474-477` ✓ | — |
| Flail, Reversal | C | `Cmd_remaininghptopower:8304-8316` with `GetScaledHPFraction(hp,maxHP,48)` = `hp*48/maxHP` (min 1 if hp>0): ≤1→200, ≤4→150, ≤9→100, ≤16→80, ≤32→40, ≤48→20 | `215-227` powers ✓. Dropdown labels are approximate: exact brackets are hp·48/maxHP ∈ {33..48}→20 (>66.7 %), {17..32}→40, {10..16}→80, {5..9}→100, {2..4}→150, {≤1}→200 (<4.17 %) | rename labels: "≥ 68.75 %", "35.4-68.7 %", "20.8-35.4 %", "10.4-20.8 %", "4.2-10.4 %", "< 4.2 %" |
| Return | C | `10*friendship/25` (`8603-8607`) | dropdown 102..1 = power directly (`228-232`) | — |
| Frustration | NI | `10*(255-friendship)/25` (`8608-8609`) | no branch → base_power 1 → 1-2 dmg ([O]) | bug #2: give Frustration the same dropdown as Return |
| Eruption, Water Spout | C | `Cmd_scaledamagebyhealthratio:9378-9388`: `hp*150/maxHP`, min 1 | `floor(150*pct/100)` (`233-242`) — exact whenever pct = 100·hp/maxHP; tiny rounding otherwise (100 %/50 % cases match, [28]) | optional: accept an HP value instead of a percent |
| Rollout, Ice Ball | I | `8536-8569`: turn n power = base·2^(n−1) (timer starts 5, loop `i < 5 - timer`), Defense Curl ×2 on every turn; it is a **base power** change (`gDynamicBasePower`), then normal crit/random | `439-446`: damage ×2^n (turn 1 = ×2!) applied after +2/crit; "5 + DefenseCurl" → ×64 (game ×32); Defense Curl only offered on turn 5 | bug #1: turn 1 game 8-10, app 17-21; turn 5+DC game 271-319, app 571-672 ([2]) |
| Fury Cutter | I | `8580-8601`: counter caps at 5 → power 10·2^(n−1), max 160; base-power change | `447-448`: damage ×2^(n−1) after +2/crit, no cap ("6" = ×32) | bug #3: use 6 game 45-54, app 108-128; use 3 game 11-14, app 13-16 ([3]) |
| Triple Kick | I | `1384-1425`: three separate hits with power 10, 20, 30 (`sTRIPLE_KICK_POWER += 10` then `copyhword gDynamicBasePower`), each with own accuracy check, crit and roll | `451-452`: single 10-BP hit × N | bug #4: "3" game 54-65 total, app 30-36 ([4]) |
| Rage | I | 20 BP vanilla hit; `MOVE_EFFECT_RAGE` sets `STATUS2_RAGE` (`2735`); when hit afterwards, **Attack stage +1** (`MOVEEND_RAGE:4241-4250`). No damage multiplier exists in gen 3 | `449-450`: damage × counter ("6" = ×6) | bug #10: remove the multiplier; "Rage N" should mean +N Attack stages (model via stage modifiers) |
| Spit Up | C (minor) | `6868-6892`: `CalculateBaseDamage` (crit multiplier 1, screens apply) × stockpile count, Helping Hand, then `typecalc`, `adjustsetdamage` (no random roll, no crit) | `453-454, 525-527` ✓ ([13] 204 = game). With `is_crit=True` the app drops screens and applies the crit stage rule even though no crit exists (app 204 vs game 105 behind Reflect) | treat Spit Up's crit range as identical to the normal range |
| Pursuit | C | `dmgMultiplier = 2` when target switches (`3086-3097`) | Switch Bonus ×2 after crit (`465, 473`) | — |
| Gust, Twister | C | ×2 vs Fly/Bounce target via `dmgMultiplier` (`1825-1831`, `1892-1896`); Twister is spread | Fly/Bounce Bonus ×2 ✓ | — |
| Surf, Whirlpool | C | ×2 vs Dive target (`236-241`, `830-834`) | Dive Bonus ✓ | — |
| Earthquake | C | ×2 vs Dig target (`1833-1849`); `FOES_AND_ALLY` → no spread halving | Dig Bonus ✓; not halved in doubles ✓ ([M]) | — |
| Stomp, Astonish, Needle Arm, Extrasensory | C | ×2 vs Minimized target (`1898-1901`) | Minimize Bonus ✓ | — |
| Facade | C | ×2 when user poisoned/burned/paralysed (`2252-2258`); burn's Atk halving still applies | Status Bonus ✓ (burn halving not modelled) | note burn |
| SmellingSalt | C | ×2 vs paralysed target (`2270-2276`) | Paralysis Bonus ✓ | — |
| Revenge | C | ×2 if damaged by the target this turn (`9339-9349`) | Damaged Bonus ✓ | — |
| Nature Power | NI | `9112-9119` becomes Stun Spore (grass), Razor Leaf (long grass), Earthquake (sand), Hydro Pump (underwater), Surf (water), BubbleBeam (pond), Rock Slide (mountain), Shadow Ball (cave), Swift (building/plain) — identical table in RS/E/FRLG; the called move's own script runs (own accuracy, target, crit stage, spread, Dig bonus for EQ, Dive bonus for Surf) | moves.json base_power 0 → `return None` at `125-126` **before** the terrain branch at `243-272`; the terrain block, the crit-rate special case and the accuracy special case are dead code. Verified: every terrain returns None ([10b]) | bug #2 (see plan §4.1) |
| SonicBoom, Dragon Rage | C | fixed 20 / 40 after `typecalc` (immunity + Wonder Guard) (`819-828`) | `damage_calc.get_special_damage_override` → 20/40, immunity ✓; Wonder Guard not checked (Dragon Rage vs Shedinja: app 40, game 0, [20]) | minor |
| Seismic Toss, Night Shade | C | level (`1195-1204`, `dmgtolevel`), after `typecalc` | override → level, immunity ✓ | — |
| Psywave | I | `7932-7941`: `r = Random()%16` re-rolled until ≤10 → r∈{0..10} uniform; `dmg = level*(10r+50)/100` → 11 equiprobable values `level*(50+10k)/100`, k=0..10 (L50: 25,30,…,75); no min-1 (L1 can deal 0) | uniform 1..floor(1.5L)−1 (`193-195`): L50 → 74 values 1..74 | bug #22 |
| Guillotine, Horn Drill, Fissure, Sheer Cold | NI | `tryKO:7490-7575`: damage = target HP (or HP−1 with Endure/Focus Band); fails vs Sturdy; chance `acc + Latk − Ldef`, hits if `rand%100+1 < chance` and `Latk ≥ Ldef` (29 % at equal level, 30 base) | base_power 1 → 1-2 damage through the normal formula ([10]); accuracy 30 flat | bug #23 |
| Low Kick | NI | `9466-9482`: pokedex weight (hectograms) <100→20, <250→40, <500→60, <1000→80, <2000→100, else 120; then normal hit | TODO comment (`197`); base_power 1 → 2-3 dmg (game vs Machop 195 hg: 40 BP → 35-42; vs Onix 2100 hg: 120 BP → 73-86, [P]) | §4.3 |
| Counter, Mirror Coat | NI | 2× last physical / special damage taken this turn from an opponent (`7943-7990`), `typecalc2` (Ghost immune to Counter, Dark to Mirror Coat), no random | base_power 1 → 2-4 dmg | §4.4: requires enemy damage input |
| Bide | NI | 2× damage taken during the 2 storing turns (`battle_util.c:2205-2230`, script `3292-3314`: `typecalc` for immunity only, no random) | base_power 1 → 1-2 dmg | §4.4 |
| Super Fang | NI | `target hp / 2`, min 1 (`7577-7584`); `typecalc` first (Ghost immune) | base_power 1 → 1-2 dmg (game vs full-HP Machop: 67, [Q]) | §4.5 |
| Present | NI | `8613-8650`: `Random()&0xFF`: <102 → 40 BP (39.8 %), <178 → 80 (29.7 %), <204 → 120 (10.2 %), else heals target maxHP/4 (20.3 %); damaging branch is a normal hit (crit/STAB/type/random) | base_power 1 → 1-2 dmg (game vs Machop: 23-28 / 46-55 / 68-81, [R]) | §4.6 |
| Endeavor | NI | `9366-9376`: `target hp − user hp`, fails if target hp ≤ user hp; `typecalc` for immunity only; no random | base_power 0 → None (silently "no damage") | §4.7 |
| Beat Up | NI | `8957-9005`: one hit per party member (incl. user) that is alive and status-free; `dmg = baseAtk(member) * 10 * (2*Lmember/5+2) / baseDef(target) / 50 + 2`; then `critcalc` + ×2 on crit + random per hit; **no STAB, no type effectiveness, no stats/stages/items/screens**; Helping Hand ×1.5 | base_power 10 through the normal formula with the user's stats ([H]: app 2-3 vs game 7-9 per hit for a 1-mon party) | §4.8 |
| Future Sight, Doom Desire | I | `8929-8955`: `CalculateBaseDamage` at time of use (`gCritMultiplier` = 1 → never crits; screens/weather/stats at time of use), stored; on hit (`3508-3535`): `accuracycheck`, `adjustnormaldamage2` (random) — **no `typecalc`: no STAB, no type effectiveness, no immunity** (hits Dark types) — same in RS/FRLG (`pokeruby 3549-3565`, `pokefirered 3461-3470`) | crit excluded ✓, STAB excluded ✓ (`377-378`), but type effectiveness and immunities applied (`135-139, 497-503`): Future Sight vs Machop app 62-74 vs game 31-37; vs Poochyena app None vs game hits ([G]) | bug #24 |
| Sky Attack (crit), Razor Wind | C | see above | ✓ ([N]) | — |
| False Swipe | C (note) | never KOs (`Cmd_adjustnormaldamage:1683-1697`) | vanilla; kill % is meaningless | optional: cap at HP−1 |
| Status moves wrongly damaging / vice-versa | — | — | Nature Power is wrongly non-damaging (above). No status move produces damage: all 139 base_power-0 moves return None. Moves with placeholder power 1 that DO produce bogus damage: Bide, Counter, Fissure, Frustration, Guillotine, Horn Drill, Low Kick, Mirror Coat, Present, Sheer Cold, Super Fang (see above). Flail/Reversal/Magnitude/Return/Hidden Power/Psywave/Seismic Toss/Night Shade/SonicBoom (also placeholder 1) are handled. | — |

---

## 3. Bugs found (ranked by impact)

### 1. Rollout / Ice Ball damage is doubled at every turn and mis-rounded
- **Affected:** Rollout, Ice Ball, all 6 dropdown values.
- **Root cause:** `pkmn_damage_calc.py:439-446` multiplies the *final damage* by `2**n` where `n` is the dropdown
  number (turn 1 → ×2); "5 + DefenseCurl" falls into the `except` → `n = 6` → ×64. Game (`8536-8569`): base
  power = `30 * 2**(turn-1)` (×2 more with Defense Curl, on every turn), i.e. a **base power** change.
- **Fix:**
  ```python
  elif move.name in (ROLLOUT, ICE_BALL):
      try: turn = int(custom_move_data); dc = False
      except ValueError: turn = 5; dc = True          # "5 + DefenseCurl"
      base_power = base_power * (2 ** (turn - 1)) * (2 if dc else 1)
  ```
  (do it in the base-power section, lines 199-272, and drop the `move_modifier` branch). Better: offer
  dropdowns "1".."5" × "with Defense Curl".
- **Test:** Machop L50 (Atk 89) Rollout vs Machop L50 (Def 59, Fighting → Rock NVE): turn 1 → base
  `89*30*22/59/50 = 19`, +2 = 21, ×0.5 = 10, rolls → {8:5, 9:10, 10:1} (min 8, max 10). App gives 17-21.
  Turn 5 + Defense Curl (BP 960): 271-319 (app 571-672).

### 2. Nature Power never deals damage; Frustration deals 1-2
- **Affected:** Nature Power (all 9 terrains), Frustration.
- **Root cause:** `moves.json` Nature Power `base_power = 0` → `pkmn_damage_calc.py:125-126` returns None before
  the terrain block at `243-272`. Frustration has no branch → placeholder power 1 is used.
- **Fix:** move the `base_power == 0` early-out after the Nature Power/Frustration overrides, or give Nature Power
  a placeholder power 1 like Magnitude (the existing test `test_move_db_has_usable_power` documents this
  pattern). Nature Power should then *delegate* to the real move (`Swift`, `Earthquake` with `No Bonus`/`Dig
  Bonus`, `Surf` with `No Bonus`/`Dive Bonus`, `Razor Leaf`, `Rock Slide`, `Shadow Ball`, `BubbleBeam`, `Hydro
  Pump`) so crit stage, accuracy, targeting (spread) and Dig/Dive bonuses come for free. Frustration: reuse the
  Return dropdown (power = `10*(255-friendship)/25`, 102..1).
- **Test:** Machop L50 Nature Power "Sand" (Earthquake, 100 Ground, neutral vs Fighting): base
  `89*100*22/59/50 = 66`, +2 = 68 → rolls 57-68 (same as Earthquake "No Bonus" in [M]). Frustration with
  friendship 0 (102 BP): 58-69, identical to Return 102.

### 3. Fury Cutter: no cap and multiplier applied to damage instead of base power
- **Root cause:** `447-448` `move_modifier = 2**(n-1)` applied after +2/crit; dropdown allows "6" (×32). Game
  (`8580-8601`): counter caps at 5, base power `10*2**(n-1)` ≤ 160.
- **Fix:** `base_power = 10 * 2 ** (min(int(custom_move_data), 5) - 1)` in the base-power section; drop "6" or
  keep it as an alias of 5.
- **Test:** Machop L50 vs Machop L50, use 3 (40 BP, Bug → NVE): base `89*40*22/59/50 = 26`, +2 = 28, ×0.5 = 14 →
  rolls {11:1,12:7,13:7,14:1}. App 13-16. Use 6: game 45-54, app 108-128.

### 4. Triple Kick: hits have power 10/20/30 and independent rolls
- **Root cause:** `451-452` multiplies one 10-BP hit by N. Game (`battle_scripts_1.s:1384-1425`): hit k has base
  power 10k, separate accuracy check, crit and random roll.
- **Fix:** compute three DamageRanges with base powers 10, 20, 30 and add them (like multi-hit); dropdown "N"
  = number of hits that land (1..3). Crit modelling: one hit critting, like multi-hit.
- **Test:** Machop L50 STAB, "3": hit1 base `89*10*22/59/50=6`+2=8 ×1.5=12 → 10-12; hit2 `13`+2=15→22 → 18-22;
  hit3 `19`+2=21→31 → 26-31; total 54-65. App 30-36.

### 5. Flash Fire constant is "Water Absorb"
- **Affected:** every Fire move vs a Water Absorb Pokémon (Poliwag/Poliwhirl/Poliwrath, Lapras, Vaporeon,
  Wooper/Quagsire, Mantine, Chinchou/Lanturn, Politoed…) → app says "no damage"; every Fire move vs a real
  Flash Fire Pokémon (Vulpix/Ninetales, Growlithe/Arcanine, Ponyta/Rapidash, Flareon, Houndour/Houndoom) → app
  shows damage.
- **Root cause:** `gen_three_constants.py:63` `self.FLASH_FIRE_ABILITY = "Water Absorb"`; `pkmn_damage_calc.py:166-170`.
- **Fix:** `self.FLASH_FIRE_ABILITY = "Flash Fire"`.
- **Test:** Machop L50 Ember vs Machop L50 with ability "Water Absorb": game 16-19 (`44*40*22/44/50=17`,+2=19);
  app None. Same vs "Flash Fire": game None (absorbed, `battle_util.c:2689-2712`); app 16-19.

### 6. Lightning Rod is treated as Electric immunity
- **Root cause:** `pkmn_damage_calc.py:153-160`. In gen 3 Lightning Rod only redirects Electric moves in
  double battles (`battle_util.c:150-180, 3835-3848`); it is not in `ABILITYEFFECT_ABSORBING` (`2662-2720`).
- **Fix:** delete `LIGHTNING_ROD_ABILITY` from that condition.
- **Test:** Machop L50 Thunderbolt vs Electrike L50 (Lightning Rod, Electric → NVE): app None; game deals damage.

### 7. Soundproof not implemented
- **Affected:** Snore, Uproar, Hyper Voice vs Soundproof (Voltorb/Electrode, Whismur line, Mr. Mime,
  Shroomish? (no — Shroomish is Effect Spore), Loudred/Exploud).
- **Root cause:** no check; `battle_util.c:2645-2660` blocks the listed sound moves.
- **Fix:** `if defending_pkmn.ability == "Soundproof" and move.name in {"Snore","Uproar","Hyper Voice"}: return None`.
- **Test:** Hyper Voice vs Whismur (Soundproof) → None; app currently 51-61 vs Machop-with-Soundproof ([11]).

### 8. Weather Ball doubles base power instead of final damage
- **Root cause:** `131-133` `base_power *= 2`. Game: `dmgMultiplier = 2` (`9787-9806`), applied after the +2 and
  crit (`Cmd_damagecalc:1296`).
- **Fix:** keep the type change at `131-133`, remove `base_power *= 2`, and set `double_damage = True` for
  Weather Ball in the `458-482` block when weather is active.
- **Test:** Castform L50 (Atk from app stats) Weather Ball in sandstorm vs Machop L50: game 26-31, app 25-30
  ([5]). In rain the two happen to coincide (102-120) because `(2*39*1.5)+... ` rounds the same way.

### 9. Stat modifiers applied after the stage multiplier (rounding)
- **Affected:** any attacker/defender with Choice Band, Huge/Pure Power, Hustle, Thick Club, Light Ball, Deep Sea
  items, Metal Powder, Thick Fat, type-boost items, or Explosion, **combined with a non-zero stage**.
- **Root cause:** `292-295` computes stage-modified stats first; items/abilities at `297-339`, type item at
  `380-381`, Explosion at `373-374` multiply the post-stage value. Game multiplies the raw stat (with badge
  boost) and applies the stage last (`3158-3232`).
- **Fix:** restructure: build `atk, dfn, spa, spd` from `calc_stat(... badge ...)` (no stage), apply the modifiers
  in the exact order of §1.1, then `modify_stat_by_stage` (respecting the crit rule), then the formula.
- **Tests:** (a) Machop L50 Explosion vs Geodude L28 (Def 63) at Def −1: game `63//2=31 → 31*10//15=20` →
  `89*250*22/20/50 = 489`+2 = 491 → NVE (Rock) → 245 → rolls 208-245; app 198-234. (b) Machop L52 (SpA 45) Ember
  vs Thick Fat at SpA −1: game `45//2=22 → 22*10//15 = 14` → damage 5-7; app 6-8. (c) Machop L52 (Atk 92) +
  Choice Band Tackle at Atk −1: game `138*10//15 = 92` → 22-26; app 21-25. (d) Azumarill L50 Huge Power Return
  102 at Atk +1: game 115-136, app 114-135.

### 10. Rage multiplies damage by the "rage counter"
- **Root cause:** `449-450`. Gen 3 Rage is a plain 20-BP hit whose effect raises the user's Attack stage by one
  each time it is hit (`battle_script_commands.c:2735`, `4241-4250`).
- **Fix:** remove the Rage entry from `CUSTOM_MOVE_DATA` and the multiplier; if a dropdown is kept, map "N" to
  `attacking_stage_modifiers.attack_stage + N` (clamped to +6).
- **Test:** Machop L50 Rage vs Machop L50, any dropdown: 12-15 (`89*20*22/59/50=13`+2=15 → rolls 12-15). App "3"
  gives 38-45, "6" gives 76-90.

### 11. Soul Dew / DeepSeaScale wired to the wrong species and side
- **Root cause:** `305-317` boost Lati@s when holding *DeepSeaScale* (should be Soul Dew, `SOULD_DEW_NAME`
  unused); `318-320` handles only Ditto for the defender; the defending Clamperl + DeepSeaScale (SpD ×2,
  `pokemon.c:3192-3193`) and the defending Lati@s + Soul Dew (SpD ×1.5, `3188-3189`) are missing. (Frontier
  battles disable Soul Dew — irrelevant to routing.)
- **Fix:** attacker: `Lati@s + "Soul Dew" → spa ×150/100`; defender: `Lati@s + "Soul Dew" → spd ×150/100`,
  `Clamperl + "DeepSeaScale" → spd ×2`; delete the DeepSeaScale/Lati@s branches.
- **Tests:** Latios L50 (SpA 139) + Soul Dew Psychic vs Machop L50: `139*150/100=208 → 208*90*22/44/50=187`+2=189
  → STAB 283 → SE 566 → rolls 481-566 (app 323-380). Machop L50 Confusion vs Clamperl L30 + DeepSeaScale:
  game 11-14, app 22-26.

### 12. `held_item_boosts` table: Dragon Scale wrong, Dragon Fang & Sea Incense missing, Silverpowder key mismatch
- **Root cause:** `type_info.json`: `"Dragon Scale": "Dragon"` (game: `HOLD_EFFECT_DRAGON_SCALE`, no boost,
  `items.h:2460-2464`); Dragon Fang (`HOLD_EFFECT_DRAGON_POWER`, param 10) absent; Sea Incense
  (`HOLD_EFFECT_WATER_POWER`, **param 5** → ×1.05) absent; key `"SilverPowder"` vs item-db name
  `"Silverpowder"` (and `"Silver Powder"` in `emerald/trainers.json` for Masquerain) — exact-string lookup fails.
- **Fix:** replace Dragon Scale with Dragon Fang; add `"Sea Incense": "Water"` and make the table carry the
  param (`{"Sea Incense": {"type": "Water", "param": 5}}`) or special-case it; normalise the key to
  `"Silverpowder"` (and fix the trainers.json spelling). `_validate_held_item_boosts` only checks types
  (`gen_three_object.py:380-390`); make it also check that every key exists in the item db.
- **Test:** Machop L50 + Dragon Fang DragonBreath vs Machop L50: `44*110/100=48` → `48*60*22/44/50=28`+2=30 →
  25-30 (no item: 28 max). Sea Incense Water Gun: `44*105/100=46` → `46*40*22/44/50=18`+2 = 20 → 17-20.

### 13. Thick Club ignores Cubone
- `297-298` vs `pokemon.c:3198-3199` (Cubone **or** Marowak). Test: Cubone L30 (app Atk from stats) + Thick Club
  Bone Club vs Machop L50: game 30-36, app 16-19.

### 14. Multi-hit crit path drops weather/doubles and reuses crit-reset stages
- **Root cause:** recursive call at `543-557` omits `weather=` and `is_double_battle=`, and passes the
  `attacking_stage_modifiers` object that the crit rule replaced with `StageModifiers()` at `280-290`.
- **Fix:** pass `weather=weather, is_double_battle=is_double_battle` and the *original* stage modifiers
  (save copies before line 280).
- **Test:** Machop L50 Fury Attack "2 Hits", crit, Atk −1: crit hit (stage ignored) `89*15*22/59/50=9`+2=11 ×2 = 22
  → 18-22; non-crit hit (stage applies: `89*10//15=59` → `59*15*22/59/50=6`+2=8) → 6-8; total 24-30. App 27-33.

### 15. `DamageRange.add` combines counts with `+` instead of `*` (shared)
- `pkmn/damage_calc.py:153-161`: `result_damage_vals[total] += my_count + your_count`. The joint distribution of
  independent hits must use `my_count * your_count` (size 16·16 = 256 for two hits; app reports 192).
  Affects kill-percent math for every multi-hit move in every gen (min/max unaffected).

### 16. Reflect / Light Screen in double battles
- `394-395` always halves; game uses `2*(dmg/3)` when two defenders are alive (`3257-3263`). Test: Machop L50
  Karate Chop vs Machop L50 behind Reflect in doubles (both foes alive): base `89*50*22/59/50 = 33` →
  `2*(33//3) = 22` → +2 = 24 → STAB 36 → rolls 30-36 ([12]); app halves (`33//2=16`, +2, ×1.5 = 27) → 22-27.
  Singles: 22-27 in both.

### 17. RS badge boosts applied in wild battles
- `pokeruby calculate_base_damage.c:70-90` requires `BATTLE_TYPE_TRAINER`. The app applies badge boosts for
  Ruby/Sapphire in wild encounters. Fix: in `calc_battle_stats` (or the gen-3 object) skip Atk/Def/SpA/SpD badge
  boosts when `version in (RUBY, SAPPHIRE)` and the fight is wild; keep the speed boost.
  Test: Ruby, player Machop L50 (Atk 89) with Stone badge, Karate Chop vs wild Machop: 44-52 (no boost); in a
  trainer battle 48-57.

### 18. Physical base damage minimum of 1 before +2 is missing
- `pokemon.c:3270-3271`. Test: Rattata L2 (Atk 7) Tackle vs Machop L50 (Def 59): `7*35*2/59/50 = 0 → 1`, +2 = 3,
  STAB → 4 → rolls {4:1, 3:15}. App {3:1, 2:15}. Fix: `if not special and temp == 0: temp = 1` before `+= 2`.

### 19. Nature Power accuracy always 100
- `get_move_accuracy:39-48`: `result = 100` after the if/elif chain. Fix: `else: result = 100` (and Rock → 90,
  Long Grass → 95, Underwater → 80, Plain/Sand/Cave/Pond/Sea → 100, Tall Grass → 75 but non-damaging).

### 20. Thunder in sun / Blizzard in hail
- Thunder: `moveAcc = 50` under sun with effect (`1146-1147`); app leaves 70. Blizzard: no hail rule in gen 3;
  app returns "always hits" (`54-55`). Fix: add `if Thunder and weather == SUN and weather_active: result = 50`;
  delete the Blizzard line (keep it for gen 4/5 only).

### 21. Compound Eyes clamp happens before Sand Veil
- `63-66` clamps to 100, then Sand Veil ×0.8. Game does `(acc*130/100)*80/100` and only compares against
  `rand%100+1`. Fix: drop the clamp (clamp at the very end, or let callers treat ≥100 as 100). Test: Butterfree
  (Compound Eyes) Gust vs Sandshrew (Sand Veil) in sandstorm: game 104 → 100 %; app 80 %.

### 22. Psywave distribution
- Game: 11 equiprobable values `level*(50+10k)//100`, k=0..10 (`7932-7941`). App: uniform 1..floor(1.5L)−1.
  Fix: `DamageRange({level*(50+10*k)//100: 1 for k in range(11)})` (collapse duplicates). Test L50: {25,30,35,40,
  45,50,55,60,65,70,75} each 1/11.

### 23. OHKO moves produce 1-2 damage
- base_power 1 goes through the formula. Fix: return `DamageRange({defending_pkmn.cur_stats.hp: 1})` (or a
  sentinel) for `one_hit_ko` flavour, None if defender has Sturdy or `attacker.level < defender.level`; accuracy
  `acc + Latk − Ldef − 1` (the strict `<`), ignoring stages. Test: Machop L50 Fissure vs Machop L50: damage
  134, hit chance 29 %.

### 24. Future Sight / Doom Desire get type effectiveness and immunities
- Game applies neither STAB, type effectiveness, nor immunity (no `typecalc` in `3508-3535`); also no crit.
  App applies effectiveness at `497-503` and immunity at `135-144`. Fix: for these two moves skip the immunity
  checks and the type loop (keep Wonder Guard? — no `typecalc` → Wonder Guard does not block either).
  Test: Machop L50 Future Sight vs Machop L50: `44*80*22/44/50=35`+2=37 → 31-37 (app 62-74); vs Poochyena:
  damage (app None).

### 25. Crit rate ignores Scope Lens / Lucky Punch / Stick / Battle Armor
- `get_crit_rate:19-35`. Fix: `+1` if `held_item == "Scope Lens"`, `+2` if Chansey + "Lucky Punch", `+2` if
  Farfetch'd + "Stick"; return 0 when the defender has Battle Armor/Shell Armor (needs a defender parameter —
  `gen_three_object.get_crit_rate` has none; the controller does have `defending_mon`). Focus Energy (+2) needs
  a new field-status flag.

### 26. Miscellaneous small ones
- `Spit Up` with `is_crit=True` drops screens and applies the crit stage rule (no crit exists): return the same
  range as the non-crit call.
- Dragon Rage / SonicBoom vs Wonder Guard (Shedinja): game blocks (typecalc runs), app deals 40/20.
- Struggle vs Wonder Guard: game hits (typecalc skipped), app None.
- Weather-dependent accuracy (`Thunder`, `Sand Veil`) ignores Cloud Nine/Air Lock; Hustle's accuracy penalty uses
  the db type instead of the resolved Hidden Power/Weather Ball type.
- Rollout/Ice Ball dropdown only offers Defense Curl on turn 5; the game doubles every turn with it.

---

## 4. Not-implemented moves / mechanics

### 4.1 Nature Power (currently dead code)
Game: `sNaturePowerMoves` (`759-771`; same in RS `979-990` and FRLG `741-752`): Tall grass → Stun Spore (status),
Long grass → Razor Leaf (55 Grass, spread, +1 crit, 95 %), Sand → Earthquake (100 Ground, all, Dig ×2, 100 %),
Underwater → Hydro Pump (120 Water, 80 %), Sea/water → Surf (95 Water, spread, Dive ×2), Pond → BubbleBeam (65
Water), Mountain/rock → Rock Slide (75 Rock, spread, 90 %), Cave → Shadow Ball (80 Ghost), Building & Plain → Swift
(60 Normal, spread, always hits). The chosen move's own script runs, so all its special cases apply.
Plan: in `calculate_gen_three_damage`, if `move.name == NATURE_POWER`, look up the mapped move from the move db
(`gen_three_object` can pass a callback or the move db) and recurse with that `Move` object and a derived
`custom_move_data` ("No Bonus"/"Dig Bonus"/"Dive Bonus" — extend the dropdown to e.g. "Sand", "Sand (Dig
Bonus)", "Sea Water", "Sea Water (Dive Bonus)"). Do the same in `get_crit_rate` and `get_move_accuracy`
(delegate to the mapped move). Remove the hard-coded powers at `243-272`.

### 4.2 Frustration
Game: `10*(255-friendship)/25` (`8608-8609`). Plan: add `FRUSTRATION_MOVE_NAME` to the Return branch (`228-232`)
and `CUSTOM_MOVE_DATA` with the same 102..1 list.

### 4.3 Low Kick
Game: weight table (`774-782`, `9466-9482`): `w < 100 hg → 20; < 250 → 40; < 500 → 60; < 1000 → 80; < 2000 → 100;
else 120`, weight = `GetPokedexHeightWeight(...,1)` (national dex weight in hectograms). Then a normal hit.
Inputs needed: species weight. `raw_pkmn_data/gen_three/*/pokemon.json` has no weight field; add `weight_hg`
per species (from `pokeemerald/src/data/pokemon/pokedex_entries.h`, field `.weight`) and compute
`base_power` from the defender's species. Test: Machop L50 Low Kick vs Machop (195 hg → 40 BP, STAB): 35-42;
vs Onix L50 (2100 hg → 120 BP, STAB, Rock SE): 73-86.

### 4.4 Counter / Mirror Coat / Bide
Game: Counter = 2× last physical damage taken this turn from an opponent (`7943-7967`), Mirror Coat = 2× last
special damage (`7969-7990`), both after `typecalc2` (immunity: Ghost vs Counter, Dark vs Mirror Coat) and with
no random roll (`adjustsetdamage`). Bide = 2× total damage taken during the two storing turns
(`battle_util.c:2221`), `typecalc` for immunity only. Requires the enemy's damage as input → not implemented;
return None (or a "needs input" sentinel) instead of the current 1-4 damage. Optional: a numeric
`custom_move_data` "damage taken" dropdown/entry → `DamageRange({2*taken: 1})`.

### 4.5 Super Fang
Game: `target current HP / 2`, min 1 (`7577-7584`), Ghost immune (`typecalc`), no random. Plan: dropdown of
target HP % (like Eruption) or default to full HP: `DamageRange({max(defending_hp*pct//100 // 2, 1): 1})`.
Test: vs Machop L50 (134 HP) at full HP → 67.

### 4.6 Present
Game (`8613-8650`): `Random()&0xFF`: 0-101 → 40 BP, 102-177 → 80 BP, 178-203 → 120 BP, 204-255 → heal target
`maxHP/4` (fails if target at full HP). Damaging branches are normal hits (`BattleScript_HitFromCritCalc`).
Plan: dropdown "40 BP" / "80 BP" / "120 BP" (probabilities 102/256, 76/256, 26/256; heal 52/256) setting
`base_power`. Test vs Machop L50 (Machop attacker): 23-28 / 46-55 / 68-81.

### 4.7 Endeavor
Game: `damage = target hp − user hp`, fails if `target hp ≤ user hp` (`9366-9376`); `typecalc` only for immunity
(Ghost); no random. Plan: needs user current HP (dropdown "user HP %" or reuse the Eruption-style percent) and
target HP; `DamageRange({target_hp - user_hp: 1})` when positive else None. Also give it base_power 1 in
moves.json so it is not filtered as a status move.

### 4.8 Beat Up
Game (`8957-9005`, RS `8586-8636`, FRLG script `1936-1950`): for each party slot (incl. the user) with HP > 0,
not an egg and no status: `dmg = baseAtk_member * 10 * (2*L_member/5 + 2) / baseDef_target / 50 + 2`, `critcalc`
per hit (×2 via `manipulatedamage DMG_DOUBLED`), `adjustnormaldamage` (random) per hit; **no** STAB, type
effectiveness, stages, items, screens, weather; Helping Hand ×1.5. Plan: needs the attacker's party (the router
has the player's party for the player side; for enemy trainers the trainer db has the full team) → sum one
range per eligible member; a dropdown "N members" as a fallback. Test: Machop L50 party of one vs Machop
(base Atk 80, base Def 50): `80*10*22/50/50 = 7`+2 = 9 → {9:1, 8:11, 7:4}; crit 15-18.

### 4.9 Abilities touching damage/accuracy — status list
| Ability | Game effect (citation) | App |
|---|---|---|
| Levitate | Ground immunity (`typecalc:1375`) | implemented |
| Damp | blocks Explosion/Selfdestruct (`6546`) | implemented |
| Volt Absorb / Water Absorb | absorb Electric/Water with power≠0 (`battle_util.c:2667-2688`) | implemented |
| Flash Fire | absorb Fire (`2689-2712`); afterwards user's Fire damage `×15/10` (`pokemon.c:3367-3368`) | immunity broken (bug #5); boost stub `423-425` — needs a "Flash Fire active" field flag |
| Lightning Rod | doubles redirection only | wrongly immune (bug #6) |
| Wonder Guard | blocks unless SE (and not also NVE), power≠0, not on a charging turn (`1409-1416`) | implemented |
| Battle Armor / Shell Armor | no crits (`1280`) | damage ✓; `get_crit_rate` should return 0 |
| Hustle | Atk ×1.5 (`3204`); physical accuracy ×0.8 (`1154`) | implemented (order caveat) |
| Huge Power / Pure Power | Atk ×2 (`3158`) | implemented (order caveat) |
| Thick Fat | SpA /2 for Fire/Ice (`3202`) | implemented (order caveat) |
| Compound Eyes | accuracy ×1.3 (`1150`) | implemented (clamp bug #21) |
| Sand Veil | accuracy ×0.8 in sandstorm with effect (`1152`) | implemented (ignores Cloud Nine/Air Lock) |
| Soundproof | blocks sound moves (`2645-2660`) | **missing** (bug #7) |
| Sturdy | blocks OHKO (`7513-7518`) | missing (with OHKO, bug #23) |
| Guts | Atk ×1.5 with any status (`3210`); also negates burn's halving (`3253`) | disabled (`347-348`); needs a "user status" input |
| Marvel Scale | Def ×1.5 with status (`3212`) | disabled (`345-346`); needs "target status" input |
| Overgrow / Blaze / Torrent / Swarm | power `×150/100` when `hp <= maxHP/3` (integer division) (`3222-3229`) | disabled (`349-356`); needs user HP (a "≤ 1/3 HP" checkbox/dropdown) |
| Plus / Minus | SpA ×1.5 when partner has the other (`3206-3209`) | missing (doubles only) |
| Cloud Nine / Air Lock | negate weather (`WEATHER_HAS_EFFECT`) | implemented for damage; missing for accuracy |
| Forecast | Castform type change | implemented (`damage_calc.apply_forecast`) |
| Intimidate / Shadow Tag / Rough Skin / Drizzle / Drought / Sand Stream / Pressure / etc. | not part of the damage formula | n/a |
| Burn (status, not ability) | physical damage /2 unless Guts (`3253`) | missing (no status input) |
| Mud Sport / Water Sport, Charge, Helping Hand | power /2 (`3214-3217`), dmg ×2 (`1298`), dmg ×1.5 (`1300`) | stubs / missing; need field-status flags |
| Focus Energy | +2 crit stages (`1266`) | missing; needs field flag |
| BrightPowder / Lax Incense | accuracy ×0.9 / ×0.95 (`1174-1175`) | missing (defender held item is available) |
| Accuracy / evasion stages, Lock-On | (`1133-1148`) | missing (`StageModifiers` has the fields; `get_move_accuracy` ignores them) |

---

## 5. Things verified correct (brief)
- Base stat formula, nature ×1.1/×0.9 truncation, HP formula; badge → stat mapping for Hoenn and Kanto badges
  (FRLG does have badge boosts: Boulder/Soul/Volcano/Thunder); badge boosts survive crits; stage ratio table.
- Core formula order and every truncation point; `(2L/5+2)` factor; `/50`; +2; crit ×2 placement and the crit
  stat-stage rule (numerically identical on all tested stage combinations); Battle/Shell Armor; Spit Up /
  Future Sight / Doom Desire never crit.
- Physical/special split list; Hidden Power type/power bit formula and the Fighting..Dark table order; Hidden
  Power category follows the resulting type.
- STAB ×1.5, type chart contents and row order (all 17 attacking types match `gTypeEffectiveness` after ignoring
  the immune rows and the commutative Grass Dragon/Steel swap), dual-type rounding, min-1.
- Random roll 85..100 (16 equiprobable), min 1, no damage cap.
- Weather: rain/sun Fire/Water ×1.5/×0.5, SolarBeam ×0.5 in rain/sand/hail, Cloud Nine/Air Lock suppression,
  Forecast; Weather Ball type map (rain→Water, sun→Fire, sand→Rock, hail→Ice).
- Screens in singles (and ignored on crit, Brick Break); doubles spread halving set (`MOVE_TARGET_BOTH` only —
  Earthquake/Explosion/Magnitude correctly not halved).
- Explosion/Selfdestruct defence halving (value; rounding order aside), Damp.
- Magnitude powers and Dig bonus; Flail/Reversal powers; Return dropdown; Eruption/Water Spout scaling;
  Spit Up ×N with no random roll and no crit; all `dmgMultiplier` bonuses (Gust/Twister vs Fly, Surf/Whirlpool
  vs Dive, EQ vs Dig, Stomp/Astonish/Needle Arm/Extrasensory vs Minimize, Facade, SmellingSalt, Revenge,
  Pursuit) applied at the right point; Struggle typeless & physical; Secret Power fixed 70.
- Fixed-damage moves (SonicBoom 20, Dragon Rage 40, Seismic Toss/Night Shade = level) with type immunity.
- Multi-hit count dropdown (2-5) and independent per-hit ranges (modulo bugs #14/#15); two-hit moves.
- Crit stage table 1/16, 1/8, 1/4, 1/3, 1/2 and the 11 high-crit moves (incl. Sky Attack, Blaze Kick, Poison
  Tail; Razor Wind correctly not).
- Accuracy: Compound Eyes ×1.3, Hustle ×0.8 physical-only, Sand Veil ×0.8 in sand, Thunder never misses in rain;
  always-hit move list; `3277/4096` is numerically equivalent to `80/100` for all integer inputs used.
- Move data: every damaging move's power/type/accuracy/target/flags in `moves.json` matches `battle_moves.h`
  (identical across RS/E/FRLG).
