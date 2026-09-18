# Gen 4 damage-calc audit

Scope: Diamond / Pearl / Platinum / HeartGold / SoulSilver as implemented in
`pkmn/gen_4/` of the app, verified line-by-line against the pokeplatinum decompilation,
cross-checked against pokeheartgold, with move data additionally checked against the
pokediamond move table. All app line numbers refer to the working tree at the time of
review. All decomp line numbers are exact (`grep -n` verified) unless marked "approx".

Numeric test cases in section 3 were produced by running the app's real gen 4
calculator (`gen_four_object.gen_four_platinum.calculate_damage`) side-by-side with a
reference re-implementation of `BattleSystem_CalcMoveDamage` +
`BattleSystem_ApplyTypeChart` + `BattleSystem_CalcDamageVariance` (script:
`<scratchpad>/app_probe.py`, output `<scratchpad>/probe_out.txt`). Fixtures used
throughout unless stated otherwise (trainer-mon IVs 8/9/8/8/8/8, 0 EVs, Hardy):

| Mon | Lv | HP | Atk | Def | SpA | SpD | Spe | Types |
|---|---|---|---|---|---|---|---|---|
| Chimchar | 20 | 49 | 30 | 24 | 29 | 24 | 31 | Fire |
| Starly | 20 | 47 | 28 | 18 | 18 | 18 | 30 | Normal/Flying |
| Gastly | 20 | – | – | 18 | – | 20 | – | Ghost/Poison |
| Geodude | 20 | – | – | 46 | – | – | – | Rock/Ground |
| Magikarp | 20 | – | – | 28 | – | – | – | Water (10.0 kg) |
| Scizor | 30 | – | 85 | – | – | – | – | Bug/Steel |
| Cherrim | 20 | – | – | – | – | 37 | – | Grass |
| Bronzor | 20 | – | 16 | – | – | – | 15 | Steel/Psychic |
| Pikachu | 20 | – | 28 | – | – | – | – | Electric |
| Latios | 50 | – | – | – | 139 | – | – | Dragon/Psychic |

---

## 0. Sources consulted

### Decompilations (read in full for the cited ranges)
- `A:\decomps\pokeplatinum\src\battle\battle_lib.c`
  - `sStatStageBoosts` 6550-6564
  - `BattleSystem_CalcMoveDamage` 6601-7079 (every line)
  - `BattleSystem_CalcDamageVariance` 7081-7095
  - `sCriticalStageRates` 7097-7103, `BattleSystem_CalcCriticalMulti` 7105-7143
  - `sTypeMatchupMultipliers` 2399-2517, `BasicTypeMulApplies` 2529-2558,
    `BattleSystem_ApplyTypeChart` 2560-2680, `BattleSystem_CalcEffectiveness` 2682-2760,
    `ApplyTypeMultiplier` 7542-7580, `MoveIsOnDamagingTurn` 7588-7605
  - `BattleSystem_Divide` 3599-3618, `Battler_Ability` 3088-3106,
    `Battler_IgnorableAbility` 3108-3124, `Battler_HeldItem` 5352-5362,
    `Battler_HeldItemEffect/Power` 5438-5463, `Battler_NaturalGiftPower/Type` 5465-5475,
    `BattleSystem_FlingItem` 5854-5900
  - `BattleSystem_CompareBattlerSpeed` (speed values incl. `monSpeedValues`) 1195-1373,
    `sSpeedHalvingItemEffects` 1160-1169
  - `BattleContext_Init` 1974-2020, redirection abilities (Lightning Rod / Storm Drain) 1747-1785
  - `sTypeBoostingItems` 6512-6548, `sPunchingMoves` 6566-6582
- `A:\decomps\pokeplatinum\src\battle\battle_script.c`
  - `BattleScript_CalcMoveDamage` 1323-1368 (crit ×, Life Orb, Metronome, Me First),
    `BtlCmd_CalcDamage` 1380-1389, `BtlCmd_CalcMaxDamage` 1402-1409, `BtlCmd_CalcCrit` 2150-2167,
    `BtlCmd_SetMultiHit` 2634-2660, `BtlCmd_Random` 3200-3210, `BtlCmd_TryOHKOMove` 4334-4390,
    `BtlCmd_Counter` 4615-4655, `BtlCmd_MirrorCoat` 4668-4708, `sHPPixelsToFlailPower` 4941-4948,
    `BtlCmd_CalcFlailPower` 4958-4972, `BtlCmd_CalcRolloutPower` 5661-5688,
    `BtlCmd_CalcFuryCutterPower` 5700-5714, `BtlCmd_Present` 5789-5808,
    `BtlCmd_CalcMagnitudePower` 5820-5852, `BtlCmd_CalcHiddenPowerParams` 6007-6031,
    `BtlCmd_TryFutureSight` 6065-6100, `BtlCmd_CheckMoveHit` 6115-6133, `BtlCmd_BeatUp` 6173-6250,
    `BtlCmd_CalcRevengePowerMul` 6525-6537, `BtlCmd_CalcHPFalloffPower` 6645-6658,
    `BtlCmd_CalcWeightBasedPower` 6808-6828, `BtlCmd_CalcWeatherBallParams` 6843-6871,
    `BtlCmd_TryPursuit` 6884-6960, `BtlCmd_ApplyTypeEffectiveness` 6963-6976,
    `BtlCmd_CalcGyroBallPower` 7123-7133, `BtlCmd_TryMetalBurst` 7153-7185,
    `BtlCmd_CalcPaybackPower` 7198-7209, `sCurrentPPScaledPower` 7211-7217,
    `BtlCmd_CalcTrumpCardPower` 7233-7244, `BtlCmd_CalcWringOutPower` 7261-7268,
    `BtlCmd_TryMeFirst` 7286-7312, `BtlCmd_CalcPunishmentPower` 7354-7372,
    `BtlCmd_TrySuckerPunch` 7385-7405, `BtlCmd_TryFeint` 7518-7528, `BtlCmd_TryLastResort` 7567-7578,
    `BtlCmd_GetTerrainMove` 8036-8047, `BtlCmd_CalcNaturalGiftParams` 8088-8103,
    `BtlCmd_TryPluck` 8116-8131, `BtlCmd_TryFling` 8144-8153, `BtlCmd_CheckChatterActivation` 8397-8435
- `A:\decomps\pokeplatinum\src\battle\battle_controller_player.c`
  - `BattleControllerPlayer_CheckMoveHitAccuracy` 2865-2980, `..._CheckMoveHitOverrides` 3001-3040,
    multi-hit loop `BattleControllerPlayer_LoopMultiHit` 3775-3812, try-move accuracy gating 3256-3272
- `A:\decomps\pokeplatinum\include\data\hit_rate_stages.h` 4-18,
  `include\data\battle\weight_to_power.h`, `include\data\terrain\to_move.h`,
  `include\constants\battle\condition.h` 100-140, `include\battle\common.h` 48-72,
  `include\move_table.h`, `generated\move_battle_effects.txt`, `generated\pokemon_types.txt`,
  `generated\move_ranges.txt`, `src\unk_0208C098.c` 41-48 (`App_PixelCount`)
- `A:\decomps\pokeplatinum\res\battle\scripts\effects\effect_script_{0000,0007,0026,0029,0036,0038,0039,0040,0041,0043,0044,0045,0047,0048,0075,0081,0087,0088,0089,0092,0099,0104,0105,0117,0119,0121,0122,0123,0126,0128,0130,0135,0144,0146,0147,0148,0149,0150,0151,0152,0154,0155,0158,0159,0161,0169,0171,0173,0185,0188,0189,0190,0196,0197,0198,0200,0203,0207,0209,0217,0219,0221,0222,0223,0224,0227,0228,0230,0231,0233,0235,0237,0245,0246,0248,0253,0254,0255,0256,0257,0261,0262,0263,0267,0268,0269,0272}.s`
  and `subscripts\subscript_{type_resist_berry,future_sight_damage,bide_end,update_hp}.s`
- `A:\decomps\pokeplatinum\res\moves\*\data.json` (all 467 moves) and `res\items\data\*.json` (all 446 items)
- `A:\Dropbox\...\pokeheartgold\src\battle\overlay_12_0224E4FC.c`: `CalcMoveDamage` 5537-5958,
  `ApplyDamageRange` 5960-5970, `TryCriticalHit` 5964-5990, `ov12_02251D28` (type chart) 2228-2330,
  `ov12_022583B4` 6450-6485, `sTypeEffectiveness` 2083-2200
- `A:\Dropbox\...\pokeheartgold\src\battle\battle_command.c`: `DamageCalcDefault` 747-780,
  `BtlCmd_CalcDamage` 782-790, `BtlCmd_CalcCrit` 1210-1220, `BtlCmd_Random` 1892-1902,
  `BtlCmd_CalcRevengeDamageMul` 3992-4002, `BtlCmd_ApplyTypeEffectiveness` 4266-4272,
  `BtlCmd_Calc{Flail,Rollout,FuryCutter,TrumpCard,GyroBall,WringOut,Punishment,WeightBased,WeatherBall,HiddenPower,Magnitude,HPFalloff,Payback}Power`, `BtlCmd_Present`, `BtlCmd_TryOHKOMove`, `BtlCmd_BeatUp`, `BtlCmd_GetTerrainMove` (4903-4912)
- `A:\Dropbox\...\pokeheartgold\src\battle\battle_controller_player.c`: `sHitChanceTable` 2399-2413, accuracy check 2425-2550
- `A:\Dropbox\...\pokeheartgold\asm\overlay_12_battle_command.s` 20-112 (Trump Card / Flail / Low Kick / Nature Power tables)
- `A:\Dropbox\...\pokeheartgold\files\poketool\waza\waza_tbl.narc` (parsed, all 467 moves)
- `A:\Dropbox\...\pokediamond\files\poketool\waza\waza_tbl.json` + `include\constants\battle.h` (DP move table; the DP repo has no decompiled battle code)

### App files
- `pkmn/gen_4/pkmn_damage_calc.py` (all 816 lines), `pkmn/gen_4/data_objects.py`,
  `pkmn/gen_4/gen_four_constants.py`, `pkmn/gen_4/gen_four_object.py`,
  `pkmn/damage_calc.py`, `pkmn/universal_data_objects.py`, `utils/constants.py`,
  `raw_pkmn_data/gen_four/moves.json`, `raw_pkmn_data/gen_four/type_info.json`,
  `raw_pkmn_data/gen_four/{platinum,diamond_pearl,heartgold_soulsilver}/pokemon.json` (weights),
  `raw_pkmn_data/gen_four/items.json`, `controllers/battle_summary_controller.py` 760-990.

### Version differences (verified)
- **Platinum vs HGSS move table**: identical for every move (effect id, class, power, type, accuracy, PP,
  effect chance, range, priority) — compared the 467 Pt `data.json` files against the parsed HGSS
  `waza_tbl.narc`. **Zero differences.**
- **Platinum vs DP move table**: identical except **Hypnosis accuracy 70 (DP) vs 60 (Pt/HGSS)**.
  Effect ids also identical index-wise (verified against DP's `MOVE_EFFECT_*` enum). No damaging
  move differs in power/type/category/accuracy/PP between DP, Pt and HGSS. The app's single
  `moves.json` (Hypnosis = 60) is therefore correct for Pt/HGSS and wrong (status only) for DP.
- **Platinum vs HGSS damage code**: `CalcMoveDamage`, `TryCriticalHit`, the type-chart function,
  the variance function, the crit-application wrapper (`DamageCalcDefault`) and the accuracy
  check are line-for-line equivalent. No Pt/HGSS behavioural difference was found in anything
  audited below. HGSS's Nature Power terrain table is the same 13-entry list (asm 109-111).
- **DP battle code**: not decompiled anywhere available. Everything below is "verified for
  Pt/HGSS, assumed identical for DP". Known documented DP-specific damage differences: none that I
  can cite from code; treat as unverified.

---

## 1. Core formula verification

### 1.1 The game's exact order (Platinum `BattleSystem_CalcMoveDamage`, battle_lib.c 6601-7079)

All arithmetic is C integer arithmetic (truncation toward zero; all values positive here).
`movePower` starts as the script-supplied power (variable-power moves) or the move-table power
(6672-6677). `moveType` = Normal if Normalize, else the script-resolved type, else table type
(6679-6685).

| # | Step (game) | Line |
|---|---|---|
| 1 | `movePower = movePower * powerMul / 10` — powerMul is 10 normally, **20** for the "double power" moves (Gust/Twister vs airborne, Earthquake/Magnitude vs Dig, Surf/Whirlpool vs Dive, Stomp vs Minimize, Facade, SmellingSalt, Wake-Up Slap, Brine, Revenge/Avalanche, Pursuit-on-switch) and **12** for Reckless recoil moves | 6688 |
| 2 | Charge (Electric): `movePower *= 2` | 6690 |
| 3 | Helping Hand: `movePower = movePower*15/10` | 6694 |
| 4 | **Technician**: if `move != Struggle && movePower <= 60`: `movePower = movePower*15/10` (uses the power AFTER steps 1-3, i.e. after Rollout/Fury Cutter/Magnitude/Hidden Power/Return/Weather Ball/Natural Gift/Low Kick/Gyro Ball/... resolution and after Revenge-style doubling) | 6698-6702 |
| 5 | Huge Power / Pure Power: `attackStat *= 2` | 6706 |
| 6 | Slow Start (first 5 turns): `attackStat /= 2` | 6709 |
| 7 | Type-boost items incl. all 16 plates (`sTypeBoostingItems`, 6512-6548): `movePower = movePower*(100+20)/100` | 6716-6722 |
| 8 | Choice Band `attackStat = attackStat*150/100`; Choice Specs `spAttackStat*150/100` | 6724-6729 |
| 9 | Soul Dew (Latios/Latias, not in Frontier): attacker `spAttack*150/100`; defender `spDefense*150/100` | 6730-6739 |
| 10 | DeepSeaTooth (attacker Clamperl) `spAttack *= 2`; DeepSeaScale (defender Clamperl) `spDefense *= 2` | 6740-6747 |
| 11 | **Light Ball (Pikachu): `movePower *= 2`** (applies to physical AND special) | 6748 |
| 12 | Metal Powder (defender Ditto): `defenseStat *= 2` | 6752 |
| 13 | Thick Club (Cubone **or** Marowak): `attackStat *= 2` | 6756 |
| 14 | Adamant/Lustrous/Griseous Orb: `movePower = movePower*120/100` (Griseous only if not Transformed) | 6760-6775 |
| 15 | Muscle Band (physical) / Wise Glasses (special): `movePower = movePower*110/100` | 6776-6783 |
| 16 | **Thick Fat (defender, Mold-Breakable)** Fire/Ice: `movePower /= 2` | 6785-6788 |
| 17 | Hustle: `attackStat = attackStat*150/100` | 6790 |
| 18 | Guts (any status): `attackStat*150/100` | 6793 |
| 19 | Marvel Scale (defender statused): `defenseStat*150/100` | 6797 |
| 20 | Plus/Minus (partner has the other): `spAttackStat*150/100` | 6802-6809 |
| 21 | Mud Sport (Electric) / Water Sport (Fire): `movePower /= 2` | 6811-6818 |
| 22 | Overgrow/Blaze/Torrent/Swarm: if `curHP <= maxHP/3` (integer div): `movePower*150/100` | 6820-6839 |
| 23 | Heatproof (defender) Fire: `movePower /= 2` | 6841-6844 |
| 24 | **Dry Skin (defender) Fire: `movePower = movePower*125/100`** | 6845-6848 |
| 25 | Simple: attacker's Atk/SpA stages ×2 (clamped ±6); defender's Def/SpD stages ×2 | 6850-6884 |
| 26 | Unaware (defender): attacker's Atk/SpA stage = 0; Unaware (attacker): defender's Def/SpD stage = 0 | 6886-6895 |
| 27 | Rivalry: same gender `movePower*125/100`, different gender `*75/100` (genderless: nothing) | 6901-6912 |
| 28 | Iron Fist (15 punch moves): `movePower*12/10` | 6915 |
| 29 | Weather stat mods (skipped if any Cloud Nine/Air Lock): sun+Solar Power `spAttack*15/10`; sandstorm & defender Rock-type `spDefense*15/10`; sun & any ally Flower Gift `attackStat*15/10`; sun & any defender-side Flower Gift (unless attacker Mold Breaker) `spDefense*15/10` | 6921-6941 |
| 30 | Explosion/Selfdestruct (`BATTLE_EFFECT_HALVE_DEFENSE`): `defenseStat /= 2` (raw stat, BEFORE stage) | 6943 |
| 31 | Physical: `damage = atk * stageNum / stageDen` (on a crit, a negative stage is ignored: `damage = atk`); `damage *= movePower; damage *= (level*2/5 + 2)`; divisor `= def * stageNum/stageDen` (on a crit a positive stage is ignored); `damage /= divisor; damage /= 50`; burn (no Guts) `/= 2`; Reflect (not crit, not Brick Break) `/= 2` (or `*2/3` in doubles with 2 alive defenders) | 6947-6990 |
| 32 | Special: same with SpA/SpD/Light Screen | 6992-7031 |
| 33 | Doubles spread: `RANGE_ADJACENT_OPPONENTS` with 2 alive opponents, or `RANGE_ALL_ADJACENT` with ≥2 alive others: `damage = damage*3/4` | 7033-7043 |
| 34 | Weather (no Cloud Nine): rain: Fire `/=2`, Water `*15/10`; `FIELD_CONDITION_SOLAR_DOWN` (rain \| sand \| hail \| fog) & SolarBeam `/=2`; sun: Fire `*15/10`, Water `/=2` | 7046-7071 |
| 35 | Flash Fire (attacker flagged) & Fire: `*15/10` | 7074 |
| 36 | `return damage + 2` | 7078 |

Then in the script wrapper `BattleScript_CalcMoveDamage` (battle_script.c 1323-1368):

| # | Step | Line |
|---|---|---|
| 37 | `damage *= criticalMul` (1, 2, or 3 with Sniper) | 1344 |
| 38 | Life Orb: `damage = damage*(100+30)/100` | 1346-1348 |
| 39 | Metronome item: `damage = damage*(10+metronomeTurns)/10` (turns 0..10 → ×1.0..×2.0) | 1350-1352 |
| 40 | Me First: `damage = damage*15/10` | 1354-1366 |

Then `BtlCmd_ApplyTypeEffectiveness` → `BattleSystem_ApplyTypeChart` (battle_lib.c 2560-2680):

| # | Step | Line |
|---|---|---|
| 41 | Struggle: return unchanged (typeless: no STAB, no chart, no immunity) | 2573 |
| 42 | STAB (attacker has moveType, not for `SYSCTL_IGNORE_TYPE_CHECKS` moves): Adaptability `*2` else `*15/10` | 2592-2598 |
| 43 | Levitate (Mold-Breakable; `Battler_Ability` already returns none under Gravity/Ingrain, 3088-3106) & Ground & no Iron Ball → LEVITATED (immune); else Magnet Rise (not Ingrained, no Iron Ball) → immune; else walk `sTypeMatchupMultipliers` in table order: for type1 then type2 (only if type2 != type1): `damage = BattleSystem_Divide(damage*mul, 10)` with mul 20/5/0; the Ghost immunities (Normal/Fighting) sit behind the 0xFE sentinel and are skipped entirely if defender is Foresighted or attacker has Scrappy. `BasicTypeMulApplies` drops the Ground-vs-Flying immunity under Iron Ball / Ingrain / Roost / Gravity, and Psychic-vs-Dark under Miracle Eye | 2600-2645, 2529-2558 |
| 44 | Wonder Guard (Mold-Breakable): blocked unless net super-effective flag set | 2649-2655 |
| 45 | If net SE: Filter/Solid Rock `damage = BattleSystem_Divide(damage*3, 4)`; Expert Belt `damage*(100+20)/100` | 2657-2666 |
| 46 | If net NVE: Tinted Lens `damage *= 2` | 2668-2672 |

`BattleSystem_Divide` (3599-3618): plain truncation, but a non-zero dividend never yields 0 (returns ±1).
The SE / NVE flags are *net* flags: SE then NVE cancel each other (`ApplyTypeMultiplier` 7542-7580), so
Filter/Solid Rock/Expert Belt/Tinted Lens apply **once** based on the overall multiplier (4×: still one ×3/4;
0.25×: still one ×2).

Then `BtlCmd_CalcDamage` (1380-1389): `damage = BattleSystem_CalcDamageVariance(damage)`
= `damage * (100 - rand%16) / 100`, minimum 1 (7081-7095). Finally the type-resist berries halve the
*applied* HP change (`subscript_update_hp.s:10` → `subscript_type_resist_berry.s`: `DivideVarByValue HP_CALC_TEMP, 2`,
only when the net-SE flag is set (Chilan: any Normal move), consumes the berry, skipped for fixed-damage/OHKO).

Multi-hit moves reload and re-run the whole move script per hit (`LoopMultiHit` 3775-3812), so **each hit rolls
its own crit and its own variance**; the accuracy check is skipped on hits 2+ for `SYSCTL_MULTI_HIT_MOVE`
moves, but re-run for Triple Kick (`SYSCTL_TRIPLE_KICK`; 3256-3272).

### 1.2 The app's order (`pkmn/gen_4/pkmn_damage_calc.py`, `calculate_gen_four_damage` 104-816)

1. Worry Seed / Gastro Acid ability override (131-139); weather active; Forecast (143-148)
2. Fixed-damage override by name (152-154)
3. Hidden Power type/power (156-161); **early-out if `base_power is None or == 0` (163-164)**
4. Multitype (169-173); Nature Power / Weather Ball / Natural Gift (176-216)
5. Technician: `base_power = floor(60*1.5)` if `base_power <= 60` (218-222)
6. Normalize (224); Judgment plate (227-230); Punishment (232-245)
7. Immunity checks (247-334); fixed/level/psywave flavours (337-343)
8. Crit stage handling (351-361); battle stats with stages (363-367)
9. Magnitude/Flail/Return/Eruption/Water Spout/Crush Grip/Wring Out/Gyro Ball/Trump Card/Low Kick (371-449)
10. Species items on **staged** stats (453-476); Hustle ×1.5; Huge Power ×2; Choice Band/Specs ×1.5 (478-497)
11. Thick Fat / Heatproof halve **both attacking stats** (499-508); Flower Gift / Solar Power (510-529); sandstorm SpD (531-536)
12. `Marvel Scale`, `Guts`, `Overgrow/Blaze/Torrent/Swarm` all `and False` (542-553)
13. Stat/screen selection (555-568); Explosion halves the **staged** defence (570-571); STAB flag (573-575)
14. Type items / orbs ×1.2 on the **attacking stat** (577-599)
15. `floor(2L/5)+2` × power × atk / def / 50 (602-609); screen ÷2 (612); spread ÷2 (615); weather (618-638); Flash Fire stub; +2 (645)
16. Crit ×2/×3 (647-655)
17. `move_modifier` ×2^n (Rollout), ×2^(n-1) (Fury Cutter), ×n (Rage, Triple Kick, Spit Up) on **damage** (658-677)
18. "double damage" ×2 on **damage** (679-708); Helping Hand / Charge stubs
19. STAB ×1.5 / ×2 (720-724)
20. Type chart: iterate chart entries; SE ×2 (Filter ×1.5 per SE type); NVE ÷2; Tinted Lens ×2 once (726-745)
21. Dry Skin ×1.3 on damage (749-753); min 1 (756-758)
22. Rolls 85..100 ÷100, min 1 (780-786); multi-hit summation (789-814)

### 1.3 Verdict table (core formula)

| Stage | Game | App | Verdict |
|---|---|---|---|
| Base damage & truncation order (no modifiers) | `floor(floor(floor(atk*stage)*P*(2L/5+2) / floor(def*stage)) / 50) + 2` | identical (602-609) | **CORRECT** — probe: Scratch {12:2,13:7,14:6,15:1} and Ember {17..21} match exactly |
| `2L/5+2` | `level*2/5 + 2` int | `floor(2L/5)+2` | CORRECT |
| Stat calc (level stats) | `(2B+IV+EV/4)*L/100 + 5`, nature ×1.1/×0.9 truncated; HP `+L+10` | data_objects.py 515-529 | CORRECT |
| Stage table | 10/40 … 40/10 (6550-6564), truncation | 2/8 … 8/2 (data_objects.py 25-39, 512) | CORRECT (same ratios) |
| Crit vs stages | on crit: attacker's *negative* stage of the *used* attacking stat ignored; defender's *positive* stage of the *used* defending stat ignored; other stats untouched (6947-7017) | 351-361: tests **`special_attack_stage` for PHYSICAL moves and `attack_stage` for SPECIAL moves**, and replaces the whole `StageModifiers` object | **INCORRECT** (bug #1) |
| Badge boosts | none in gen 4 | none (GenFourBadgeList returns False) | CORRECT |
| Hustle / Huge Power / Choice / Thick Club / DeepSeaTooth / Metal Powder | on the raw stat, before the stage multiplier, in the order of 1.1 | on the *staged* stat, order Hustle→Huge→Choice (478-497) | ordering/rounding difference only; **matches in the common cases** (probe: Choice Band +1 Atk identical). Differs when ×2 and ×1.5 combine on odd stats (e.g. Huge Power + Hustle: game floor(floor(a*2)*1.5), app floor(floor(a*1.5)*2)) |
| Light Ball | `movePower *= 2` for Pikachu, physical and special (6748) | doubles `special_attack` only (455-456) | **INCORRECT** (bug #12) |
| Thick Club | Cubone or Marowak, atk ×2 | Marowak only (453) | INCORRECT (minor) |
| Soul Dew | attacker SpA ×1.5 / defender SpD ×1.5 | code checks `held_item == DEEP_SEA_SCALE_NAME` for Latios/Latias (461-473) — never fires for "Soul Dew" | **INCORRECT** (bug #13) |
| DeepSeaScale | defender Clamperl SpD ×2 | applied to the *attacking* Clamperl's SpD (459-460), never to the defender | INCORRECT (minor) |
| Type items / plates | `movePower*120/100` (power) | attacking stat ×1.2 (577-581) | rounding-level difference; item list has errors (bug #14) |
| Adamant/Lustrous/Griseous Orb | power ×1.2, resolved move type, Griseous not while Transformed | stat ×1.2, uses raw `move.move_type` (583-599) | rounding-level; raw type misses Judgment/Weather Ball/HP/Natural Gift of Dragon/Steel/Water/Ghost type |
| Life Orb / Expert Belt / Metronome / Muscle Band / Wise Glasses | ×1.3 (after crit), ×1.2 on SE (after chart), ×(10+n)/10, ×1.1 power, ×1.1 power | none | **MISSING** |
| Thick Fat / Heatproof | `movePower /= 2` | halves both Atk and SpA stat (499-508) | effect equivalent, rounding differs (probe Ember vs Snorlax matched) |
| Dry Skin (Fire) | `movePower*125/100` | `floor(damage*1.3)` at the very end (749-753), keyed on raw `move.move_type` | **INCORRECT** value (1.25 vs 1.3) and position; misses Weather Ball/Judgment/HP/Natural Gift Fire |
| Flower Gift | sun: attacker-side Atk ×1.5 (any ally with it); defender-side SpD ×1.5 | boosts SpA and then sets `special_defense = floor(special_attack*1.5)` (510-523) | **INCORRECT** (bug #11) |
| Solar Power | SpA ×1.5 in sun | same (524-529) | CORRECT |
| Sandstorm Rock SpD ×1.5 | yes (6926-6929) — Pt/HGSS verified, DP assumed | same (531-536) | CORRECT |
| Guts / Marvel Scale / Overgrow-Blaze-Torrent-Swarm / Rivalry / Iron Fist / Reckless / Plus-Minus / Simple / Unaware / Mud-Water Sport / Charge / Helping Hand / Me First / burn halving | see 1.1 | disabled or absent | **MISSING** (section 4) |
| Explosion / Selfdestruct | raw `defenseStat /= 2` before stage | staged stat `/2` (570-571) | rounding-level (differs e.g. def 25 at +2: game 24, app 25) |
| Screens | `/2` (not on crit, not Brick Break); `*2/3` in doubles with 2 alive defenders | `/2` always (612-613) | CORRECT for singles; doubles value MISSING |
| Doubles spread | `*3/4` for `RANGE_ADJACENT_OPPONENTS` / `RANGE_ALL_ADJACENT` | `/2` if `move.targeting == "target_both_enemies"` (615-616) — moves.json targets are "All Foes"/"Others", so the condition is **never true** | **INCORRECT** twice (bug #16) |
| Weather ×1.5 / ×0.5 | rain/sun on the resolved type; SolarBeam halved in rain, sand, hail **and fog** | same, incl. fog (620-638) | CORRECT (probe: SolarBeam in fog matched) |
| Flash Fire ×1.5 | attacker flag | stub `False` (641-643) | MISSING |
| +2 | after weather, before crit | 645 | CORRECT |
| Crit multiplier | ×2, Sniper ×3, blocked by Battle/Shell Armor (Mold-Breakable), Lucky Chant, and for Future Sight/Doom Desire (never crit-checked); **Spit Up CAN crit** | 647-655 excludes Spit Up | Spit Up exclusion INCORRECT; rest CORRECT |
| Variable-power multipliers (Rollout, Fury Cutter, double-power moves, Triple Kick, Spit Up…) | on `movePower` before everything | on `damage` after crit (658-708) | rounding differs for all of them; several are numerically wrong (section 2/3) |
| STAB | ×1.5 / Adaptability ×2 after crit and item mods, before chart | same position (720-724) | CORRECT |
| Type chart order | type1 then type2 with `BattleSystem_Divide` each | chart-key iteration order, floor each (726-741) | equivalent for all 20/5 combinations (×2 exact, ÷2 floors identically; the "min 1" of `BattleSystem_Divide` is covered by the app's min-1 at 756) — sole exception: pre-chart damage of exactly 1 hit by NVE-then-SE (game 1→1→2, app 1→0→0→1), negligible |
| Filter / Solid Rock | once, ×3/4 on the net-SE result | ×1.5 **per SE type** (731-735) → 4× becomes 2.25× instead of 3× | **INCORRECT** for double-SE (bug #20) |
| Tinted Lens | once when *net* NVE | once if *any* NVE entry seen (739-745) — fires on SE×NVE = neutral | INCORRECT (minor) |
| Expert Belt | ×1.2 once on net SE | none | MISSING |
| Type-resist berries / Chilan | halve applied damage (min 1), consumed | none | MISSING |
| Random roll | `damage*(100-r)/100`, r=0..15, min 1 | 85..100 /100 floor, min 1 (780-786) | CORRECT |
| Min damage | 1 (variance) | 1 | CORRECT |
| Damage cap | none | none | CORRECT |
| Multi-hit | each hit: own crit, own roll | per-hit independent ranges summed; crit case = exactly one crit hit (789-814) | acceptable model; but `DamageRange.add` combines counts with `+` instead of `*` (bug #22) |
| Struggle | typeless (2573), Technician-excluded | Normal-typed → STAB for Normal types and **immune vs Ghost** (returns None) | **INCORRECT** (bug #17) |
| Fixed damage (SonicBoom 20 / Dragon Rage 40 / Seismic Toss / Night Shade = level) | `SYSCTL_IGNORE_TYPE_CHECKS`: no STAB/chart, immunities still apply | `get_special_damage_override` by name, immunity respected | CORRECT (names "SonicBoom", "Dragon Rage", "Seismic Toss", "Night Shade" all match gen 4 moves.json) |
| Psywave | `(rand%11 + 5) * level / 10`, min 1 (effect_script_0088) → 11 equiprobable values | uniform 1..floor(1.5L)−1 (341-343) | **INCORRECT** (bug #21) |

### 1.4 `get_crit_rate` (pkmn_damage_calc.py 20-38) vs `BattleSystem_CalcCriticalMulti` (7105-7143)

Game stage = `2*FocusEnergy + (Scope Lens/Razor Claw) + criticalBoosts(move) + Super Luck + 2*(Lucky Punch on Chansey) + 2*(Stick on Farfetch'd)`, capped at 4; rates 1/16, 1/8, 1/4, 1/3, 1/2 (7097-7103). Crit is then cancelled by Battle Armor / Shell Armor (Mold-Breakable), Lucky Chant, and `MOVE_EFFECT_NO_CRITICAL`. `criticalBoosts` is +1 only for effects `HIGH_CRITICAL` (0043), `CHARGE_TURN_HIGH_CRIT` (0039 = Razor Wind), `CHARGE_TURN_HIGH_CRIT_FLINCH` (0075 = Sky Attack), `HIGH_CRITICAL_BURN_HIT` (0200 = Blaze Kick), `HIGH_CRITICAL_POISON_HIT` (0209 = Poison Tail, Cross Poison). Nature Power never gives a crit boost in gen 4 (it calls Earthquake/Seed Bomb/Rock Slide/…).

| Item | Verdict |
|---|---|
| Table 1/16,1/8,1/4,1/3,1/2 | CORRECT |
| high-crit flavour +1 | CORRECT for moves that carry the `high_crit` flavour; **moves.json omits the flavour on Night Slash, Shadow Claw, Psycho Cut, Stone Edge, Cross Poison, Attack Order, Spacial Rend, Razor Wind** (bug #8) |
| Super Luck +1 | CORRECT |
| Nature Power long grass +1 | INCORRECT (gen 3 leftover; remove) |
| Scope Lens / Razor Claw +1, Focus Energy +2, Lucky Punch +2, Stick +2, Lucky Chant, Battle/Shell Armor → 0 | MISSING |

### 1.5 `get_move_accuracy` (41-101) vs `BattleControllerPlayer_CheckMoveHitAccuracy` (2865-2980)

Game: `hitRate = accuracy` (0 = never misses; charge turn / OHKO / Future Sight use other paths); Thunder in sun → 50 (2915); `hitRate = hitRate * HitRateByStage[6 + acc − eva]` with the table 33/36/43/50/60/75/100/133/166/200/233/266/300 %, stages summed and clamped 0..12, Simple doubles, Unaware zeroes, Foresight/Miracle Eye clamp negative evasion (2876-2920); then in order: Compound Eyes ×130/100 (2922); sand+Sand Veil ×80/100, hail+Snow Cloak ×80/100, **fog ×6/10** (2927-2937, all skipped under Cloud Nine); Hustle physical ×80/100 (2940); Tangled Feet (confused) ×50/100 (2944); BrightPowder/Lax Incense ×(100−10)/100 (2952); Wide Lens ×110/100 (2959); Zoom Lens ×120/100 if the target already moved (2963); Micle Berry ×120/100 (2967); Gravity ×10/6 (2972). Miss if `rand%100 + 1 > hitRate` (2976) — i.e. P(hit) = hitRate/100 exactly, and hitRate ≥ 100 always hits (no cap needed). Overrides (3001-3040): Lock-On / No Guard force hit; Thunder in rain and Blizzard in hail never miss; semi-invulnerable targets are missed. OHKO (4334-4390): Sturdy blocks; else `hit = acc + (Latk − Ldef)`, `rand%100 < hit` **and** `Latk >= Ldef`; Lock-On/No Guard bypass the roll (still need `Latk >= Ldef`).

| Item | App | Verdict |
|---|---|---|
| No Guard → always hits | 48-49 | CORRECT |
| Thunder in rain / Blizzard in hail → always hit | 66-69 (no Cloud Nine check) | CORRECT (modulo Cloud Nine) |
| Thunder in sun → 50 | missing (app 70) | **INCORRECT** |
| Compound Eyes ×1.3 floor, cap 100 | 74-78 | CORRECT (cap is harmless) |
| Hustle ×0.8 physical (`floor(x*3277/4096)`) | 79-84 | CORRECT for all base accuracies 30..100 (identical results to `*80/100`), Nature Power sub-case uses gen-3 terrains |
| Wide Lens ×1.1 | 86-93 | CORRECT |
| Sand Veil ×0.8 in sandstorm | 95-96 | CORRECT |
| Snow Cloak | 98-99 checks `WEATHER_SANDSTORM` | **INCORRECT** — must be hail (probe: app hail 100 / sand 80; game hail 80 / sand 100) |
| Accuracy/evasion stages | not used at all | MISSING (the app has `accuracy_stage`/`evasion_stage` in StageModifiers but never passes them here) |
| Fog ×0.6, Gravity ×10/6, Tangled Feet, BrightPowder/Lax Incense, Zoom Lens, Micle, Simple/Unaware, Foresight/Miracle Eye | – | MISSING |
| Nature Power branch (51-60) | `result` set per terrain then unconditionally overwritten by `100` | INCORRECT; gen 4 accuracy = called move's accuracy: Earthquake/Seed Bomb/Ice Beam/Tri Attack 100, Rock Slide 90, Blizzard 70, Hydro Pump 80, Mud Bomb 85, Air Slash 95 |
| OHKO | returns the flat 30 | MISSING level term and `Latk >= Ldef` requirement |

### 1.6 Stat calculation extras (`data_objects.py`, `calc_battle_stats` 354-443)

| Item | Game | App | Verdict |
|---|---|---|---|
| Nature | ×1.1 / ×0.9 truncated | `floor(x*1.1)` / `floor(x*0.9)` (524-527) — safe in IEEE floats for these magnitudes | CORRECT |
| Power Trick | swaps **Attack and Defense** (`BATTLE_EFFECT_SWAP_ATK_DEF`) | swaps **Attack and Special Attack** (426-427) | **INCORRECT** (bug #15) |
| Slow Start | Atk /2 before stages (6709-6714), Speed /2 in speed calc (battle_lib.c 1286-1290) | Atk & SpA & Spe /2 (429-432) | Atk/Spe correct; **SpA must not be halved** |
| Speed: Macho Brace / Power items /2 | after stage (1240-1246); Iron Ball also halves | before stage (491-492); no Iron Ball | rounding-level; Iron Ball MISSING |
| Choice Scarf ×1.5 | after stage (1248) | before stage (493-494) | rounding-level |
| Tailwind ×2 | yes | yes | CORRECT |
| Trick Room | only reverses ordering; `monSpeedValues` (used by Gyro Ball) are untouched | speed transformed to `(10000−spe)%8192` (440-441) → **feeds the app's Gyro Ball** | wrong input to Gyro Ball under Trick Room |
| Paralysis /4, Quick Feet, Swift Swim/Chlorophyll, Unburden | speed calc | none | MISSING (no status model) |
| Hidden Power type/power | bit0 of hp,atk,def,spe,spa,spd → `*15/63` → Fighting..Dark (skipping ???); bit1 → `*40/63+30` (6007-6031) | 627-656 | **CORRECT** bit-for-bit (probe: IVs 8/9/8/8/8/8 → Fighting 30 both) |

---

## 2. Per-move table

Status moves (Swords Dance … Lunar Dance, 190 of them) were skipped. Data-level notes on non-damaging
entries: Curse is `TYPE_MYSTERY` in the game but "Ghost" in moves.json; Memento accuracy is 100 in the
game, `None` in moves.json; Hypnosis is 70 in DP. Nature Power is a status move in the data but calls
a damaging move (see its row). No status move is wrongly treated as damaging (all have `power: None`).

Legend: "vanilla" = goes through the plain formula with no move-specific code; such moves are CORRECT
*subject to the global issues in section 1/3* (spread, items, abilities…). "≈" = numerically right for
the common case but the multiplier is applied to damage instead of power (rounding may differ by 1).

| Move | App status | What the game does (cite) | What the app does (file:line) | Fix / plan |
|---|---|---|---|---|
| **Vanilla single-hit, no crit boost (145 moves)**: Pound, Mega Punch, Pay Day, Fire Punch, Ice Punch, ThunderPunch, Scratch, ViceGrip, Cut, Wing Attack, Fly, Bind, Slam, Vine Whip, Mega Kick, Rolling Kick, Headbutt, Horn Attack, Tackle, Body Slam, Wrap, Thrash, Poison Sting, Bite, Acid, Ember, Flamethrower, Water Gun, Hydro Pump, Ice Beam, Psybeam, BubbleBeam, Aurora Beam, Hyper Beam, Peck, Drill Peck, Strength, Absorb, Mega Drain, Petal Dance, Fire Spin, ThunderShock, Thunderbolt, Rock Throw, Dig, Confusion, Psychic, Quick Attack, Egg Bomb, Lick, Smog, Sludge, Bone Club, Fire Blast, Waterfall, Clamp, Swift, Skull Bash, Constrict, Dream Eater, Leech Life, Bubble, Dizzy Punch, Rock Slide, Hyper Fang, Tri Attack, Thief, Flame Wheel, Snore, Powder Snow, Mach Punch, Faint Attack, Sludge Bomb, Mud-Slap, Octazooka, Zap Cannon, Icy Wind, Outrage, Giga Drain, False Swipe, Spark, Steel Wing, Sacred Fire, DynamicPunch, Megahorn, DragonBreath, Rapid Spin, Iron Tail, Metal Claw, Vital Throw, ExtremeSpeed, AncientPower, Shadow Ball, Rock Smash, Fake Out, Uproar, Heat Wave, Focus Punch, Superpower, Knock Off, Secret Power, Dive, Luster Purge, Mist Ball, Hyper Voice, Poison Fang, Crush Claw, Blast Burn, Hydro Cannon, Meteor Mash, Overheat, Rock Tomb, Silver Wind, Signal Beam, Shadow Punch, Sand Tomb, Muddy Water, Aerial Ace, Dragon Claw, Frenzy Plant, Bounce, Mud Shot, Covet, Magical Leaf, Shock Wave, Water Pulse, Psycho Boost, Hammer Arm, Feint, Pluck, U-turn, Close Combat, Force Palm, Aura Sphere, Poison Jab, Dark Pulse, Aqua Tail, Seed Bomb, Air Slash, X-Scissor, Bug Buzz, Dragon Pulse, Dragon Rush, Power Gem, Drain Punch, Vacuum Wave, Focus Blast, Energy Ball, Earth Power, Giga Impact, Bullet Punch, Ice Shard, Thunder Fang, Ice Fang, Fire Fang, Shadow Sneak, Mud Bomb, Zen Headbutt, Mirror Shot, Flash Cannon, Rock Climb, Draco Meteor, Discharge, Lava Plume, Leaf Storm, Power Whip, Rock Wrecker, Gunk Shot, Iron Head, Magnet Bomb, Chatter, Bug Bite, Charge Beam, Aqua Jet, Roar of Time, Magma Storm, Seed Flare, Ominous Wind, Shadow Force, Last Resort, Sucker Punch, Sky Uppercut | CORRECT | `effect_script_0000` pattern: `CalcCrit; CalcDamage` with table power; conditional-failure moves (Fake Out, Focus Punch, Sucker Punch, Feint, Last Resort, Dream Eater, Snore) have no damage difference when they succeed | vanilla path | – (Sky Uppercut only adds `SYSCTL_HIT_DURING_FLY`; False Swipe's 1-HP floor is not modelled but doesn't change the range) |
| **Reckless recoil moves**: Jump Kick, Hi Jump Kick (0045), Take Down, Submission (0048), Double-Edge, Volt Tackle (0262), Brave Bird, Wood Hammer, Flare Blitz (0253/0198), Head Smash (0269) | CORRECT w/o Reckless | `powerMul = 12` if attacker has Reckless (`effect_script_0045/0048/0198/0253/0262/0269`) | vanilla | add Reckless ×1.2 power (section 4) |
| **High-crit vanilla with flavour present**: Karate Chop, Razor Leaf, Crabhammer, Slash, Aeroblast, Cross Chop, Blaze Kick, Air Cutter, Poison Tail, Leaf Blade, Sky Attack | CORRECT | `criticalBoosts += 1` (0043/0200/0209/0075) | flavour `high_crit` → stage +1 | – |
| **High-crit moves missing the flavour**: Razor Wind, Night Slash, Shadow Claw, Psycho Cut, Stone Edge, Cross Poison, Attack Order, Spacial Rend | INCORRECT (crit 1/16 instead of 1/8) | `BATTLE_EFFECT_HIGH_CRITICAL` / `CHARGE_TURN_HIGH_CRIT` (0039) / `HIGH_CRITICAL_POISON_HIT` (0209) | moves.json `attack_flavor: []` for these; `get_crit_rate` finds no `high_crit` | add `"high_crit"` to their `attack_flavor` in moves.json (bug #8) |
| **2–5 hit**: DoubleSlap, Comet Punch, Fury Attack, Pin Missile, Spike Cannon, Barrage, Fury Swipes, Bone Rush, Arm Thrust, Bullet Seed, Icicle Spear, Rock Blast | CORRECT (per hit) | `SetMultiHit 0` (0029): 2/3/4/5 hits with 3/8,3/8,1/8,1/8 (2634-2660; Skill Link → 5); each hit re-runs CalcCrit+CalcDamage; accuracy checked once | flavour `multi_hit`, dropdown 2–5 hits, per-hit ranges summed | fix `DamageRange.add` counting (bug #22) |
| **2-hit**: Double Kick, Twineedle, Bonemerang | CORRECT | `SetMultiHit 2` (0044) | flavour `two_hit` ×2 | – |
| Double Hit | INCORRECT (single hit) | `BATTLE_EFFECT_HIT_TWICE` (0044), 2 × 35 | moves.json `attack_flavor: []` → 1 hit (761-762) | add `"two_hit"` flavour (bug #9) |
| Triple Kick | INCORRECT | `SetMultiHit 3, SYSCTL_TRIPLE_KICK; UpdateVar ADD MOVE_POWER 10` per hit (0104): hits have power 10/20/30, each with its own crit/roll **and its own accuracy check**; stops on a miss | `move_modifier = int(custom)` multiplies the 10-power *damage* by n; dropdown "1/2/3" returns only the n-th kick (672-673) | compute hit k with power 10k, sum hits 1..n; per-hit accuracy 90% |
| SonicBoom, Dragon Rage | CORRECT | 20 / 40 flat; `SYSCTL_IGNORE_TYPE_CHECKS` (0130/0041); immunities still apply | `get_special_damage_override` | – |
| Seismic Toss, Night Shade | CORRECT | = level (0087) | same | – |
| Psywave | INCORRECT | `Random 10,5` → r∈[0,10]; `damage = level*(r+5)/10`, min 1 (0088); no STAB/chart; Dark immune | uniform 1..floor(1.5L)−1 (341-343) | bug #21 |
| Guillotine, Horn Drill, Fissure, Sheer Cold | NOT IMPLEMENTED | damage = target's current HP; hit = `30 + (Latk−Ldef)`, fails if `Latk < Ldef`; Sturdy immune; No Guard/Lock-On bypass (4334-4390) | vanilla with power 1 → 1–2 damage; accuracy 30 flat | section 4 |
| Counter / Mirror Coat | NOT IMPLEMENTED | 2 × last physical / special damage taken this turn (4615-4708), typeless | vanilla power 1 | needs damage input |
| Metal Burst | NOT IMPLEMENTED | 1.5 × last damage taken this turn (7153-7185), typeless | vanilla power 1 | needs damage input |
| Bide | NOT IMPLEMENTED | 2 × damage stored over 2 turns (`subscript_bide_end`) | vanilla power 1 | needs damage input |
| Endeavor | NOT IMPLEMENTED | `damage = defHP − atkHP` if positive (0189), typeless | vanilla power 1 | needs HP inputs |
| Super Fang | NOT IMPLEMENTED | half of target's current HP, min 1 (0040) | vanilla power 1 | needs HP input |
| Low Kick, Grass Knot | INCORRECT at boundaries | weight in 0.1 kg: `<=100:20, <=250:40, <=500:60, <=1000:80, <=2000:100, else 120` (6808-6828, `weight_to_power.h`) | `< 10, < 25, < 50, < 100, < 200` kg (434-449) — species at exactly 10.0/25.0/50.0/100.0/200.0 kg get one bracket too high (Magikarp, Silcoon, Kakuna 10.0; Croconaw, Seadra, Flareon 25.0; Tropius, Venusaur 100.0 in the app's DBs) | use `<=` (bug #10); Technician never applies (bug #4) |
| Flail, Reversal | CORRECT (mapping) | `pixels = cur*64/max` (min 1 if cur>0); `<=1:200, <=5:150, <=12:100, <=21:80, <=42:40, else 20` (4941-4972) | dropdown → 20/40/80/100/150/200 | labels are approximate: real brackets are HP < 2/64 (3.1%) → 200, < 6/64 (9.4%) → 150, < 13/64 (20.3%) → 100, < 22/64 (34.4%) → 80, < 43/64 (67.2%) → 40, else 20 |
| Return | CORRECT | `friendship*10/25` (0121) | dropdown 1..102 used as power | – |
| Frustration | INCORRECT | `(255−friendship)*10/25` (0123) | no dropdown → power 1 | bug #7: share Return's dropdown |
| Present | NOT IMPLEMENTED | rand&0xFF: `<102`:40, `<178`:80, `<204`:120, else heal max/4 (5789-5808) → 40%/30%/10%/20% | vanilla power 1 | section 4 |
| Magnitude | ≈ CORRECT | 4:10 (5%),5:30 (10%),6:50 (20%),7:70 (30%),8:90 (20%),9:110 (10%),10:150 (5%); `powerMul=20` vs Dig (0126) | dropdown Mag 4..10 + "Dig Bonus" ×2 on damage (371-385, 700-703) | move the ×2 to power |
| Rollout, Ice Ball | INCORRECT | turn n power = 30·2^(n−1), n=1..5 (5661-5688: count set to 5, decremented, `for i=1; i<5−count`), Defense Curl ×2 on power; Technician applies to the resulting power | `2^n` on damage (660-667): turn 1 already ×2; "5 + DefenseCurl" → 2^6=64 instead of 32 | bug #2 |
| Fury Cutter | INCORRECT at 6+ | count capped at 5 → 10/20/40/80/160 (5700-5714) | `2^(n−1)` on damage with dropdown 1..6 → "6" = ×32 = 320 | bug #3 |
| Rage | INCORRECT | plain 20-power move; the *effect* is +1 Attack stage each time the user is hit while raging (`subscript_update_hp` "rage is building") — no damage multiplier | `move_modifier = int(custom)` ×1..6 on damage (670-671) | bug #6: remove the multiplier/dropdown (user models Rage via Attack stages) |
| Spit Up | INCORRECT | power = 100 × stockpiles, no variance (`CalcMaxDamage`, 0161); **crits allowed** | power 1 × n (674-675) → 2/4/6 damage; crit excluded (649) | bug #5 |
| Eruption, Water Spout | ≈ CORRECT | `power = 150*cur/max`, min 1 (6645-6658) | `floor(150*pct/100)` from a 1..100% dropdown (404-413) | fine (percent granularity); min 1 |
| Crush Grip, Wring Out | INCORRECT | `power = 1 + 120*cur/max` (7261-7268) | `floor(1 + 120*int(pct))` (414-421) → 12001 at 100% | bug #4b |
| Gyro Ball | INCORRECT | `power = 1 + 25*defSpeed/atkSpeed`, cap 150 (7123-7133); speeds from `monSpeedValues` (stages, items, weather abilities, paralysis, Tailwind — *not* Trick Room) | `1 + 25*atkSpeed/defSpeed` (422-424) — inverted; also fed the Trick-Room-transformed speed | bug #4c |
| Punishment | INCORRECT | `60 + 20*Σ(positive stages over all 8 stats incl. acc/eva)`, cap 200 (7354-7372) | `move.base_power (=1) + 20*Σ` over 5 stats, `min(Σ,7)` (232-245) | bug #4d: use 60, include acc/eva, cap power at 200 |
| Trump Card | INCORRECT | PP left after this use: 0:200, 1:80, 2:60, 3:50, ≥4:40 (7211-7244) | dropdown "4+"/"3"/"2"/"1"; "4+" matches no branch → power stays 1; no "0" option (425-433) | bug #4e |
| Hidden Power | CORRECT | see 1.6 | 156-158 | Technician bug #4 applies (HP 30–60 → app 90) |
| Weather Ball | CORRECT | ×2 power in any weather incl. fog; type Water/Rock/Fire/Ice for rain/sand/sun/hail, Normal in fog (6843-6871) | 206-208 + `get_weather_ball_type` | – (Technician: 50→75 in no weather; app 90) |
| Natural Gift | INCORRECT (table) | power/type from item data; 17 berries wrong in the app: Occa, Passho, Wacan, Rindo, Yache, Chople, Kebia, Shuca, Coba, Payapa, Tanga, Charti, Kasib, Colbur, Haban, Babiri, Chilan are **60**, not 80 (types correct); all other 44 entries match; Klutz/Embargo → fails ✓ | `NATURAL_GIFT_BERRY_DATA` (gen_four_constants.py 256-272) | bug #18 |
| Judgment | INCORRECT (Earth Plate) | type from plate (0268) | `PLATE_TYPE_LOOKUP["Earth Plate"] = Dark` (gen_four_constants.py 78) → also mis-types Multitype Arceus | bug #19: Ground |
| Nature Power | NOT IMPLEMENTED (always None) | Pt/HGSS terrain → move (`to_move.h`, HGSS asm 109-111): Plain/Sand → **Earthquake**, Grass/Puddle → **Seed Bomb**, Mountain/Cave → **Rock Slide**, Snow → **Blizzard**, Water → **Hydro Pump**, Ice → **Ice Beam**, Building/Special(E4 rooms) → **Tri Attack**, Great Marsh → **Mud Bomb**, Bridge → **Air Slash**; the called move runs its own script (`GoToMoveScript`, 0173) with its own category/accuracy/target/STAB | `base_power is None` early-out at 163-164 fires **before** the terrain override at 176-205, so every terrain returns `None`; the override table itself is the gen-3 one (Swift/Earthquake/Shadow Ball/Rock Slide/Stun Spore/Razor Leaf/BubbleBeam/Surf/Hydro Pump) | bug #23 |
| Secret Power | CORRECT | 70 Normal physical; terrain only changes the secondary effect (0197) | vanilla | – |
| Beat Up | NOT IMPLEMENTED | per eligible party member (alive, no status, not egg): `baseAtk*10*(2L/5+2)/baseDef(target)/50 + 2`, ×crit (rolled per hit), ×1.5 Helping Hand, ×variance; **no STAB, no chart, no items** (`SYSCTL_IGNORE_IMMUNITIES`; 6173-6250) | vanilla 10-power Dark move | section 4 |
| Fling | NOT IMPLEMENTED | power = item's `flingPower` (full table in section 4); fails with no item / Multitype / Griseous Orb (0233) | vanilla power 1 | section 4 |
| Struggle | INCORRECT | 50 power, **typeless** (2573): no STAB, no chart, hits Ghosts; Technician excluded | Normal type → STAB, Ghost immunity (returns None vs Gastly) | bug #17 |
| Selfdestruct, Explosion | ≈ CORRECT | raw Def /2 before stage; Damp blocks (0007) | staged Def /2 (570-571); Damp ✓ | rounding-level |
| Future Sight, Doom Desire | INCORRECT | damage computed at setup with `BattleSystem_CalcMoveDamage(...crit=1)` + variance (+HH) and **never passed through `ApplyTypeChart`**: no STAB, no effectiveness, no immunity (Dark is hit), no crit; Light Screen/weather/items at setup apply (6065-6100, `subscript_future_sight_damage`) | STAB off ✓, crit off ✓, but type chart & immunity applied (probe: vs Gastly app 42–50, game 21–25) | skip the chart and immunity for these two |
| Brick Break | CORRECT | ignores Reflect/Light Screen in the formula (6982-6990, 7023-7031) | 558/565 | – |
| Gust, Twister | ≈ CORRECT | `powerMul=20` vs airborne target (0149/0146) | "Fly/Bounce Bonus" ×2 on damage | move to power |
| Earthquake (and Magnitude) | ≈ CORRECT | `powerMul=20` vs Dig (0147/0126) | "Dig Bonus" ×2 on damage | move to power; spread bug #16 |
| Surf, Whirlpool | ≈ CORRECT | `powerMul=20` vs Dive (0257/0261) | "Dive Bonus" ×2 | move to power |
| Stomp | ≈ CORRECT | `powerMul=20` vs Minimized (0150) | "Minimize Bonus" ×2 | move to power |
| Needle Arm, Astonish, Extrasensory | INCORRECT (option should not exist) | plain `BATTLE_EFFECT_FLINCH_HIT` in gen 4 — **no** Minimize bonus | offer "Minimize Bonus" ×2 (332-334, 686-689) | remove the dropdown for these three |
| Facade | ≈ CORRECT | ×2 power if burned/poisoned/badly poisoned/paralysed (0169, `MON_CONDITION_FACADE_BOOST`); burn still halves physical damage | "Status Bonus" ×2 | move to power |
| SmellingSalt | ≈ CORRECT | ×2 power if target paralysed (not behind Substitute), then cures (0171) | "Paralysis Bonus" ×2 | move to power |
| Wake-Up Slap | ≈ CORRECT | ×2 power if target asleep, then wakes (0217) | "Sleeping Bonus" ×2 | move to power |
| Revenge, Avalanche | ≈ CORRECT | `powerMul=20` if the *target* damaged the user this turn (6525-6537) | "Damaged Bonus" ×2 | move to power |
| Brine | ≈ CORRECT | ×2 power if target `curHP <= maxHP/2` (0221) | "Low Health Bonus" ×2 | move to power |
| Assurance | ≈ CORRECT | ×2 power if the target already took damage this turn (0231) | "Damaged Bonus" ×2 | move to power |
| Payback | ≈ CORRECT | ×2 power if the target already moved (7198-7209) | "Move Second Bonus" ×2 | move to power |
| Pursuit | ≈ CORRECT | `powerMul=20` when the target switches (6884-6960) | "Switch Bonus" ×2 | move to power |
| SolarBeam | CORRECT | halved in rain/sand/hail/fog (7058) | 623-633 | – |
| Thunder | INCORRECT (accuracy only) | 50% accuracy in sun; never misses in rain; hits Fly | rain ✓, sun ✗ | set 50 in sun |
| Blizzard | CORRECT | never misses in hail | ✓ | – |
| Razor Wind | INCORRECT (crit) | +1 crit stage (0039) | no `high_crit` flavour | bug #8 |
| Uproar, Snore, Hyper Voice, Bug Buzz, Chatter | CORRECT (damage) | sound moves; Soundproof immune (`subscript_blocked_by_soundproof`) | vanilla; Soundproof not modelled | add Soundproof immunity (section 4) |
| Pluck, Bug Bite, Knock Off, Thief, Covet, U-turn, Feint, Sucker Punch, Last Resort, Fake Out, Focus Punch, Dream Eater, Snore | CORRECT | vanilla damage when they succeed | vanilla | – |
| Fly, Dig, Dive, Bounce, Shadow Force, Sky Attack, Razor Wind, Skull Bash, SolarBeam | CORRECT (damage turn) | charge-turn moves; Power Herb skips charge | vanilla | – |

---

## 3. Bugs found (ranked by impact)

Each test case is *attacker → move → defender* with the section-0 fixtures; "game" values come from
the reference implementation of the C above, "app" values from running the app.

### 1. Crit stage handling checks the wrong stat and wipes every stage
- **Affected**: every critical-hit calculation whenever any stage modifier is set.
- **Root cause**: `pkmn_damage_calc.py:351-361` — for `CATEGORY_PHYSICAL` it inspects
  `special_attack_stage`/`special_defense_stage` (and vice versa), and on a match replaces the entire
  `StageModifiers` object (zeroing Attack, Defense, Speed, accuracy… stages too). Game (6947-7017):
  only the *used* attacking stat's negative stage and the *used* defending stat's positive stage are ignored.
- **Fix**:
  ```python
  if is_crit:
      if move.category == const.CATEGORY_PHYSICAL:
          if attacking_stage_modifiers.attack_stage < 0:
              attacking_stage_modifiers = attacking_stage_modifiers.set_attack_stage(0)
          if defending_stage_modifiers.defense_stage > 0:
              defending_stage_modifiers = defending_stage_modifiers.apply_stat_mod([(const.DEF, -defending_stage_modifiers.defense_stage)])
      else:
          if attacking_stage_modifiers.special_attack_stage < 0:
              attacking_stage_modifiers = attacking_stage_modifiers.apply_stat_mod([(const.SPA, -attacking_stage_modifiers.special_attack_stage)])
          if defending_stage_modifiers.special_defense_stage > 0:
              defending_stage_modifiers = defending_stage_modifiers.apply_stat_mod([(const.SPD, -defending_stage_modifiers.special_defense_stage)])
  ```
  (or add explicit `set_*_stage` helpers to `StageModifiers`).
- **Test**: Chimchar (Atk +2, SpA −1) Scratch → Starly, crit. Game: atk 30×2=60 → 60·40·10=24000; /18=1333; /50=26; +2=28; ×2=56 → `{47:1,48:2,49:2,50:2,51:1,52:2,53:2,54:2,55:1,56:1}`. App: `{25:2,26:3,27:4,28:3,29:3,30:1}` (stages wiped).
  Second: Chimchar Scratch → Starly (Def +2), crit. Game ignores the +2: `{25..30}` as above. App `{13:3,14:6,15:6,16:1}`.

### 2. Rollout / Ice Ball doubles on turn 1 and over-doubles with Defense Curl
- **Root cause**: `pkmn_damage_calc.py:660-667` uses `2**n`; game (5661-5688) uses power `30·2^(n−1)`,
  then `×2` for Defense Curl, i.e. max 30·16·2 = 960.
- **Fix**: `move_modifier = 2 ** (n - 1)` for "1".."5"; "5 + DefenseCurl" → `2**4 * 2 = 32`. Better: apply it
  to `base_power` *before* the Technician check (see #4).
- **Test**: Chimchar Rollout "1" → Starly (Rock vs Flying SE): game 30·30·10=9000; /18=500; /50=10; +2=12; ×2=24 → `{20:3,21:4,22:4,23:4,24:1}`; app `{40..48}`.
  Rollout "5 + DefenseCurl": game power 960 → 30·960·10=288000; /18=16000; /50=320; +2=322; ×2=644 → 547..644; app 1305..1536.

### 3. Fury Cutter has no cap at 160
- **Root cause**: 668-669, dropdown allows "6" (`gen_four_constants.py:324`); game caps the counter at 5 (5700-5714).
- **Fix**: `move_modifier = 2 ** (min(int(custom), 5) - 1)` or drop "6" from the dropdown.
- **Test**: Chimchar Fury Cutter "6" → Starly (Bug vs Flying NVE): game 160 → 48000/18=2666; /50=53; +2=55; ÷2=27 → `{22:1,23:3,24:4,25:4,26:3,27:1}`; app 68..80.

### 4. Technician is hard-coded to 90 and evaluated before the real power is known
- **Root cause**: `pkmn_damage_calc.py:218-222`: `base_power = floor(60*1.5)` for any power ≤ 60, and it
  runs before Magnitude/Flail/Return/Eruption/Wring Out/Gyro Ball/Trump Card/Low Kick are resolved
  (371-449) and before the Rollout/Fury Cutter/double-power multipliers (658-708). Game (6698-6702):
  `movePower = movePower*15/10` on the fully resolved power, after `powerMul` and before items.
- **Fix**: resolve all variable powers and the ×2 "bonus" multipliers into `base_power` first, then
  `if technician and move.name != "Struggle" and base_power <= 60: base_power = base_power * 15 // 10`,
  then items ×1.2 etc.
- **Tests** (Scizor L30 Atk 85, Technician): Bullet Punch → Starly: game 60 → 85·60·14=71400; /18=3966; /50=79; +2=81; ×1.5 STAB=121 → 102..121; app 153..181.
  Low Kick → Magikarp (10.0 kg, Def 28): game 20→30: 85·30·14=35700; /28=1275; /50=25; +2=27 → 22..27; app 30..36 (40 power from bug #10, no Technician).
  Fury Cutter "2": game 20→30, STAB, NVE: 35700/18=1983; /50=39; +2=41; ×1.5=61; ÷2=30 → 25..30; app 153..181.

#### 4b. Crush Grip / Wring Out power formula
- **Root cause**: 414-421 computes `1 + 120*pct`. Game: `1 + 120*cur/max` (7261-7268).
- **Fix**: `base_power = 1 + (120 * int(custom)) // 100` (exact with an HP dropdown: `1 + 120*cur//max`).
- **Test**: Chimchar Wring Out "100" → Starly: game 121 → 29·121·10=35090; /18=1949; /50=38; +2=40 → `{34:3,35:2,36:3,37:2,38:3,39:2,40:1}`; app 3287..3868.

#### 4c. Gyro Ball ratio is inverted (and uses Trick-Room-mangled speed)
- **Root cause**: 422-424 `1 + 25*atk.speed/def.speed`; game `1 + 25*defSpeed/atkSpeed` (7123-7133), cap 150,
  speeds from `monSpeedValues` (1195-1373: stages, Choice Scarf, Macho Brace/Iron Ball, Tailwind, paralysis, weather abilities — no Trick Room).
- **Fix**: `base_power = min(150, 1 + (25 * defending_battle_stats.speed) // attacking_battle_stats.speed)` using a speed that bypasses the Trick Room transform in `calc_battle_stats` (440-441).
- **Test**: Bronzor L20 (Atk 16, Spe 15, Steel STAB) Gyro Ball → Starly (Spe 30, Def 18): game power 1+25·30/15=51 → 16·51·10=8160; /18=453; /50=9; +2=11; ×1.5=16 → `{13:3,14:6,15:6,16:1}`. App: power 1+25·15//30=13 → 2080/18=115; /50=2; +2=4; ×1.5=6 → `{5:15,6:1}`.

#### 4d. Punishment starts from power 1
- **Root cause**: 232-245 adds `20*Σ` to `move.base_power` (=1 in moves.json); game `60 + 20*Σ` over all 8 stat stages (incl. accuracy/evasion), cap 200 (7354-7372).
- **Fix**: `base_power = min(200, 60 + 20 * sum(max(0, s) for s in all_8_stages))`.
- **Test**: Chimchar Punishment → Starly (+2 Atk): game 100 → 30000/18=1666; /50=33; +2=35 → `{29:1,30:3,31:3,32:3,33:3,34:2,35:1}`; app 12..15.

#### 4e. Trump Card "4+" never sets the power; no "0" option
- **Root cause**: 425-433 handles "3/2/1/0" only; dropdown (`gen_four_constants.py:352`) offers "4+","3","2","1".
- **Fix**: `{"4+":40, "3":50, "2":60, "1":80, "0":200}` (PP remaining *after* the current use) and add "0".
- **Test**: Chimchar Trump Card "4+" → Starly: game 40 → 11600/18=644; /50=12; +2=14 → `{11:1,12:7,13:7,14:1}`; app `{1:15,2:1}`.

### 5. Spit Up uses power 1×n and cannot crit
- **Root cause**: 674-675 (`move_modifier=n` on a 1-power move); 649 excludes it from crits. Game: power 100·n, `CalcMaxDamage` (no variance), `CalcCrit` runs (0161).
- **Fix**: `base_power = 100 * int(custom)`; remove from the crit exclusion; keep the single-value range.
- **Test**: Chimchar Spit Up "3" → Starly: game 29·300·10=87000; /18=4833; /50=96; +2=98 → `{98:1}`; app `{6:1}`.

### 6. Rage multiplies damage by the "count"
- **Root cause**: 670-671; gen 4 Rage is a plain 20-power hit whose effect is +1 Atk stage when hit (`subscript_update_hp.s` 58-70).
- **Fix**: delete the Rage branch and its dropdown (users express the boost through Attack stages).
- **Test**: Chimchar Rage "3" → Starly: game 6000/18=333; /50=6; +2=8 → `{6:3,7:12,8:1}`; app `{20..24}`.

### 7. Frustration has no power input
- **Root cause**: no `CUSTOM_MOVE_DATA` entry → moves.json power 1. Game `(255−friendship)*10/25` (0123).
- **Fix**: reuse Return's dropdown for `Frustration` in `CUSTOM_MOVE_DATA` and handle it at 399-403.
- **Test**: Chimchar Frustration (friendship 0 → 102) → Starly: 30600/18=1700; /50=34; +2=36 → `{30:2,31:2,32:3,33:3,34:3,35:2,36:1}`; app `{1:15,2:1}`.

### 8. Eight high-crit moves lack the `high_crit` flavour
- Night Slash, Shadow Claw, Psycho Cut, Stone Edge, Cross Poison, Attack Order, Spacial Rend, Razor Wind
  (moves.json; all have Pt effect `HIGH_CRITICAL`/`HIGH_CRITICAL_POISON_HIT`/`CHARGE_TURN_HIGH_CRIT`).
- **Fix**: add `"high_crit"` to `attack_flavor`. **Test**: `get_crit_rate(mon, Night Slash)` should be `1/8` (app returns `1/16`).

### 9. Double Hit is a single hit
- moves.json `attack_flavor: []`; game `SetMultiHit 2` (0044). **Fix**: add `"two_hit"`. **Test**: Chimchar Double Hit → Starly: two hits of `{11:8,12:7,13:1}`; app one hit.

### 10. Low Kick / Grass Knot weight brackets use `<` on kg instead of `<=` on 0.1 kg
- 434-449 vs `weight_to_power.h` + 6808-6828. **Fix**: `w = round(weight*10)`; `<=100:20, <=250:40, <=500:60, <=1000:80, <=2000:100, else 120`.
- **Test**: Chimchar Low Kick → Magikarp (10.0 kg, Def 28): game 20 → 6000/28=214; /50=4; +2=6 → `{5:15,6:1}`; app 40 → `{8:5,9:10,10:1}`.

### 11. Flower Gift boosts the wrong stats
- 510-523: boosts SpA and sets `special_defense = floor(special_attack*1.5)`. Game (6930-6941): attacker side Atk ×1.5, defender side SpD ×1.5, sun only, per side (any ally with the ability), attacker Mold Breaker negates the defender's.
- **Fix**: `if sun and attacking_ability == FLOWER_GIFT: attacking.attack = floor(attack*1.5)`; `if sun and defending_ability == FLOWER_GIFT: defending.special_defense = floor(special_defense*1.5)`.
- **Test**: Chimchar Ember → Cherrim (Flower Gift, sun; SpD 37→55): 29·40·10=11600; /55=210; /50=4; sun ×1.5=6; +2=8; STAB=12; SE=24 → `{20:3,21:4,22:4,23:4,24:1}`; app `{11:1,12:7,13:7,14:1}`.

### 12. Light Ball doubles Special Attack instead of move power
- 455-456; game `movePower *= 2` (6748). **Fix**: `base_power *= 2` when Pikachu holds Light Ball (after Technician, with the item boosts).
- **Test**: Pikachu L20 (Atk 28) Quick Attack → Starly: game 80 → 22400/18=1244; /50=24; +2=26 → `{22:4,23:4,24:4,25:3,26:1}`; app `{11:1,12:7,13:7,14:1}`.

### 13. Soul Dew never triggers
- 461-473 compares against `DEEP_SEA_SCALE_NAME` instead of `SOULD_DEW_NAME`. **Fix**: use the Soul Dew constant; attacker: SpA ×1.5; defender: SpD ×1.5.
- **Test**: Latios L50 (SpA 139) Dragon Pulse → Starly: game SpA 208 → 208·90·22=411840; /18=22880; /50=457; +2=459; ×1.5=688 → 584..688; app 391..460.

### 14. Type-boost item table errors
- `type_info.json` `held_item_boosts` lists **Dragon Scale** (no effect in gen 4: `HOLD_EFFECT_EVOLVE_SEADRA`) and omits **Dragon Fang**, **Odd Incense** (Psychic), **Rock Incense** (Rock), **Rose Incense** (Grass), **Sea Incense** and **Wave Incense** (Water) — all `HOLD_EFFECT_STRENGTHEN_*` param 20 in `res/items/data` and present in the app's `items.json`.
- **Test**: Chimchar Dragon Pulse → Starly holding Dragon Fang: game 108 → 31320/18=1740; /50=34; +2=36 → 30..36; app gives the plain 26..31, while Dragon Scale wrongly gives 30..36.

### 15. Power Trick swaps Attack and Special Attack
- `data_objects.py:426-427`; game swaps Attack and **Defense** (`BATTLE_EFFECT_SWAP_ATK_DEF`). **Fix**: `result.attack, result.defense = result.defense, result.attack`.
- **Test**: Chimchar (Atk 30, Def 24, SpA 29) under Power Trick: app Atk 29 / SpA 30; game Atk 24 / Def 30.

### 16. Doubles spread halving never fires and uses the wrong factor
- 615-616 tests `move.targeting == "target_both_enemies"`; moves.json uses "All Foes"/"Others". Game: `×3/4` (7033-7043).
- **Fix**: `if is_double_battle and move.targeting in ("All Foes", "Others"): temp = temp*3//4`; also Reflect/Light Screen `*2/3` in doubles with two defenders.
- **Test**: Chimchar Earthquake → Geodude (Def 46) in doubles: 30000/46=652; /50=13; ×3/4=9; +2=11; ×2 (Rock) =22 → `{18:2,19:4,20:5,21:4,22:1}`; app `{25:2,26:3,27:4,28:3,29:3,30:1}`.

### 17. Struggle is typed Normal
- Game: typeless (2573). App: STAB for Normal types, returns `None` vs Ghosts.
- **Fix**: for Struggle skip STAB, chart, all immunity checks, Technician.
- **Test**: Chimchar Struggle → Gastly (Def 18): 15000/18=833; /50=16; +2=18 → `{15:4,16:6,17:5,18:1}`; app `None`.

### 18. Natural Gift: 17 berries have power 80 instead of 60
- `gen_four_constants.py:256-272` (Occa…Babiri, Chilan). Game `res/items/data/*.json` `naturalGiftPower` = 60 for all of them.
- **Test**: Chimchar (Occa Berry) Natural Gift → Starly: game 60 Fire STAB → 18000/18=1000; /50=20; +2=22; ×1.5=33 → `{28:3,29:3,30:3,31:3,32:3,33:1}`; app 35..42.

### 19. Earth Plate maps to Dark
- `gen_four_constants.py:78`. Affects Judgment's type and Multitype Arceus's type/STAB. **Fix**: `TYPE_GROUND`.
- **Test**: Arceus (Multitype, Earth Plate) Judgment → Starly: game immune (`None`); app 403..475.

### 20. Filter / Solid Rock applied per SE type
- 731-735 gives ×1.5 for each SE type (4× → 2.25×); game applies `Divide(damage*3,4)` once on the net result (2657-2661) (4× → 3×). Tinted Lens (739-745) similarly fires on any NVE entry even when the net result is neutral (SE×NVE), unlike the game's net-flag test (2668-2672).
- **Fix**: compute the net multiplier first, then `if net > 1: temp = max(1, temp*3//4)`; `if net < 1 and tinted: temp *= 2`.
- **Test**: Ice Beam (95, SpA 29, L20) vs a Grass/Flying Solid Rock target with SpD 30: base 29·95·10/30/50=18; +2=20; ×2×2=80; game ×3/4 → 60 → 51..60; app ×1.5×1.5 → 45 → 38..45.

### 21. Psywave distribution
- 341-343 uniform over 1..floor(1.5L)−1; game `level*(r+5)/10`, r uniform 0..10, min 1 (0088). For L20: `{10,12,14,…,30}` each 1/11 (11 values). App: 29 values 1..29. **Fix**: `DamageRange({max(1, level*(r+5)//10): 1 for r in range(11)})` (merge duplicates); no STAB/chart, Dark immune.

### 22. `DamageRange.add` mis-weights multi-hit combinations (shared code)
- `pkmn/damage_calc.py:153-159` accumulates `my_count + your_count`; the number of roll combinations producing a total is `my_count * your_count` (and `size` should be the product of sizes). Min/max are unaffected; kill-percentages for multi-hit moves are wrong. Example: two hits `{11:8,12:7,13:1}` → correct `{22:64,23:112,24:65,25:14,26:1}` (256 combos); app `{22:16,23:30,24:32,25:16,26:2}`.

### 23. Nature Power always returns `None` (and uses the gen-3 table)
- 163-164 early-out precedes the override at 176-205. Fix: move the early-out below all power overrides, and replace the table by the gen 4 terrain→move list (section 2 row), reusing the called move's data (category, accuracy, targeting, STAB by the called type). Dropdown values should become the 9 gen 4 terrains (Plain/Sand, Grass/Puddle, Mountain/Cave, Snow, Water, Ice, Building, Great Marsh, Bridge) or simply the 9 called moves.
- **Test**: Chimchar Nature Power "Plain" → Starly: game Earthquake vs Flying = `None`; "Cave" → Rock Slide 75 SE: 30·75·10=22500; /18=1250; /50=25; +2=27; ×2=54 → 45..54; app `None` for every terrain.

### 24. Snow Cloak keyed on sandstorm
- 98-99. **Fix**: `weather == const.WEATHER_HAIL`. Test: Scratch vs Snow Cloak: hail → 80, sandstorm → 100 (app reversed).

### 25. Flash Fire immunity constant is "Water Absorb"
- `gen_four_constants.py:100`. Fire moves are never treated as absorbed by Flash Fire mons. **Fix**: `"Flash Fire"`.

### 26. Lightning Rod is treated as an Electric immunity
- 290-298. In gen 4 Lightning Rod (and Storm Drain) only *redirect* single-target moves in doubles (battle_lib.c 1747-1785); there is no absorption. **Fix**: remove Lightning Rod from the immunity list (keep Volt Absorb / Motor Drive).

### 27. Future Sight / Doom Desire go through the type chart
- see section 2 row. Test: Chimchar Future Sight → Gastly: game 23200/20=1160; /50=23; +2=25 → 21..25; app 42..50.

### 28. Minor / rounding-level (listed for completeness)
- Dry Skin ×1.3 on damage (749-753) vs ×1.25 on power; keyed on raw move type.
- Thick Fat / Heatproof halve stats instead of power (499-508).
- Type items / orbs on the stat instead of power (577-599); orb check uses raw `move.move_type`.
- `ignore_ground_immunity` (253-257), `is_scrappy_active` (247-251), Dry Skin and the orbs use the raw
  `move.move_type` rather than the resolved `move_type` → wrong for Judgment/Weather Ball/Hidden
  Power/Natural Gift/Nature Power/Normalize edge cases.
- Explosion halves the staged Def (570) instead of the raw Def.
- Thick Club ignores Cubone; DeepSeaScale applied to the wrong side.
- Slow Start also halves SpA (`data_objects.py:431`).
- Needle Arm / Astonish / Extrasensory offer a Minimize bonus that does not exist in gen 4.
- Battle Armor / Shell Armor suppress crit damage but `get_crit_rate` still reports 1/16.
- Thunder accuracy in sun (50) not modelled.
- Flail labels vs real pixel brackets (section 2).

---

## 4. Not-implemented moves / mechanics

1. **Present** (5789-5808): rand&255 <102 → 40 (40%), <178 → 80 (30%), <204 → 120 (10%), else heals target `maxHP/4` (20%). Plan: dropdown "40 / 80 / 120 / Heal" mapping to `base_power`; optionally weight the kill-% by 0.4/0.3/0.1.
2. **Beat Up** (6173-6250): for each party member that is alive, un-statused and not an egg (in party order): `d = baseAtk_member * 10 * (2L_member/5+2) / baseDef_target / 50 + 2`, `d *= crit` (crit rolled per hit), `*15/10` if Helping Hand, variance. No STAB/chart/items/abilities. Plan: needs the party (species+level) as input; compute one range per member and sum.
3. **Fling** (0233, `BattleSystem_FlingItem` 5854+): power = `flingPower` of the held item; fails without an item, with Multitype, or holding Griseous Orb. Table (`res/items/data`): 130 Iron Ball; 100 Hard Stone, Rare Bone, all fossils/Old Amber; 90 all 16 plates, DeepSeaTooth, Thick Club, Grip Claw; 80 Razor Claw, Quick Claw, Sticky Barb, Dawn/Dusk/Shiny Stone, Electirizer, Magmarizer, Protector, Oval Stone, Odd Keystone; 70 Dragon Fang, Poison Barb, Power Anklet/Band/Belt/Bracer/Lens/Weight; 60 Adamant/Lustrous/Griseous Orb, Damp/Heat Rock, Macho Brace, Stick; 50 Sharp Beak, Dubious Disc; 40 Lucky Punch, Icy Rock; 30 Life Orb, Light Ball, Scope Lens, Metronome, Soul Dew, DeepSeaScale, King's Rock, Razor Fang, Shell Bell, Amulet Coin, Lucky Egg, Everstone, Exp. Share, Black Sludge, Flame/Toxic Orb, Light Clay, Cleanse Tag, Smoke Ball, Up-Grade, Dragon Scale, all type items (Black Belt, BlackGlasses, Charcoal, Magnet, Metal Coat, Miracle Seed, Mystic Water, NeverMeltIce, Spell Tag, TwistedSpoon) 30 except Sharp Beak 50, Poison Barb 70, Hard Stone 100, Silk Scarf/SilverPowder/Soft Sand 10; all evolution stones, medicines, vitamins, shards, mulch, flutes, Nugget etc. 30; 10 all berries, Choice items, Expert Belt, Focus Band/Sash, Muscle Band, Wise Glasses, Wide/Zoom Lens, BrightPowder, Lax/Full/Odd/Rock/Rose/Sea/Wave/Luck/Pure Incense, Leftovers, Metal/Quick Powder, Big Root, Destiny Knot, Mental Herb, Power Herb, Shed Shell, Smooth Rock, Soothe Bell, White Herb, Lagging Tail, Reaper Cloth, scarves. Plan: `base_power = FLING_POWER[held_item]`, Dark physical, one use.
4. **Counter / Mirror Coat** (×2 last physical/special damage), **Metal Burst** (×1.5 last damage), **Bide** (×2 stored): need a "damage taken" input; typeless (`SYSCTL_IGNORE_TYPE_CHECKS`) but Ghost/Dark immunities still apply for Counter/Mirror Coat.
5. **Endeavor** (`defHP − atkHP`), **Super Fang** (`defHP/2`, min 1): need HP inputs; typeless.
6. **OHKO moves**: damage = target HP; accuracy `30 + (Latk−Ldef)`, only if `Latk >= Ldef`; Sturdy immune; No Guard / Lock-On bypass. Plan: return `DamageRange({def_hp: 1})` and a level-aware accuracy.
7. **Nature Power** (gen 4 table) — see bug #23.
8. **Triple Kick** as a true 3-hit sequence (10/20/30, per-hit accuracy 90%).
9. **Abilities**: Guts (Atk ×1.5 when statused), Marvel Scale (Def ×1.5 when statused), Overgrow/Blaze/Torrent/Swarm (power ×1.5 at `HP <= maxHP/3`), Rivalry (×1.25 same gender / ×0.75 opposite; needs gender), Iron Fist (×1.2: Ice/Fire/ThunderPunch, Mach Punch, Focus Punch, Dizzy Punch, DynamicPunch, Hammer Arm, Mega Punch, Comet Punch, Meteor Mash, Shadow Punch, Drain Punch, Bullet Punch, Sky Uppercut), Reckless (×1.2 on the moves in section 2), Plus/Minus, Simple, Unaware, Mold Breaker (ignore defender's Levitate/Thick Fat/Heatproof/Dry Skin/Marvel Scale/Filter/Solid Rock/Wonder Guard/Battle Armor/Shell Armor/Sand Veil/Snow Cloak/Tangled Feet/Sturdy/Simple/Unaware/Flower Gift), Flash Fire boost ×1.5, Soundproof immunity (Uproar, Snore, Hyper Voice, Bug Buzz, Chatter), Tangled Feet, Sturdy vs OHKO, burn halving, Skill Link (5 hits).
10. **Field effects**: Charge ×2 Electric, Helping Hand ×1.5, Mud Sport/Water Sport ÷2, Me First ×1.5, Foresight/Odor Sleuth (Ghost immunity removal + evasion clamp), Ingrain (grounds; also Levitate off), Iron Ball (grounds + speed /2), Gravity ×10/6 accuracy + Levitate off (partly done), fog ×0.6 accuracy, doubles 2/3 screens.
11. **Items**: Life Orb ×1.3 (after crit), Expert Belt ×1.2 (net SE), Metronome ×(10+n)/10, Muscle Band / Wise Glasses ×1.1 power, type-resist berries (halve applied SE damage; Chilan any Normal), Scope Lens/Razor Claw +1 crit, Lucky Punch/Stick +2, BrightPowder/Lax Incense ×0.9 acc, Zoom Lens ×1.2 acc when slower, Micle Berry, Iron Ball, Quick Powder (Ditto speed ×2).
12. **Accuracy/evasion stages** in `get_move_accuracy` (table in 1.5).

---

## 5. Things verified correct (brief)

- Base damage formula, truncation order, `+2`, crit ×2/×3 position, STAB ×1.5/×2 position, chart order (type1 then type2 with per-step floor), random 85..100 %, min 1, no cap — probe matches the game exactly for plain moves.
- Stat formula, nature rounding, stage table, no badge boosts, HP formula.
- Hidden Power type & power bit extraction and tables.
- Weather ×1.5/×0.5 incl. SolarBeam halving in rain/sand/hail/**fog**; Weather Ball ×2 and typing incl. fog (Normal); sandstorm Rock SpD ×1.5; Solar Power; Cloud Nine/Air Lock suppression; Forecast typing.
- Brick Break ignores screens; screens skipped on crits; Explosion/Selfdestruct Def halving (rounding aside); Damp.
- Hustle ×1.5 Atk and ×0.8 physical accuracy; Huge/Pure Power ×2; Choice Band/Specs ×1.5; Choice Scarf ×1.5; Macho Brace/Power items ×0.5 speed; Tailwind ×2; Slow Start Atk/Spe ×0.5; Metal Powder; DeepSeaTooth; Thick Club (Marowak).
- Adaptability ×2 STAB; Sniper ×3; Super Luck +1; Normalize (type + STAB); Scrappy; Multitype STAB (except Earth Plate); Klutz; Wonder Guard net-effectiveness logic; Volt Absorb / Water Absorb / Motor Drive / Dry Skin (Water) immunities; Levitate; Magnet Rise; Gravity/Roost/Miracle Eye immunity removal; Worry Seed / Gastro Acid ability suppression.
- Fixed-damage moves (SonicBoom/Dragon Rage/Seismic Toss/Night Shade) incl. immunity handling and name matching.
- Magnitude power table; Flail/Reversal power mapping; Return power; Eruption/Water Spout; Trump Card values for 3/2/1 PP; Punishment cap; Gyro Ball cap 150.
- Multi-hit: 2–5 hit moves and two-hit moves modelled per hit with independent crit/roll (matches the per-hit script reload).
- Compound Eyes ×1.3, Wide Lens ×1.1, Sand Veil ×0.8 in sand, No Guard, Thunder-in-rain and Blizzard-in-hail always hit.
- Move data (power/type/category/accuracy/PP/target) for every damaging move matches Platinum and HGSS exactly; DP differs only in Hypnosis accuracy.
