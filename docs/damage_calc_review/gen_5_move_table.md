| Move | Power | Type | Cat | Acc | Effect code | App status (gen 5, today) | Notes |
|---|---|---|---|---|---|---|---|
| Pound | 40 | Normal | Physical | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Karate Chop | 50 | Fighting | Physical | 100 | `high_crit_rate` | **BROKEN** | Gen 5 moves.json has no attack_flavor, so FLAVOR_HIGH_CRIT never matches: crit stage stays 0 (1/16 instead of 1/8). |
| Double Slap | 15 | Normal | Physical | 85 | `two_to_five_hits` | **BROKEN** | FLAVOR_MULTI_HIT never matches: no "2 Hits".."5 Hits" dropdown is offered and only ONE hit is ever calculated. |
| Comet Punch | 18 | Normal | Physical | 85 | `two_to_five_hits` | **BROKEN** | FLAVOR_MULTI_HIT never matches: no "2 Hits".."5 Hits" dropdown is offered and only ONE hit is ever calculated. |
| Mega Punch | 80 | Normal | Physical | 85 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Pay Day | 40 | Normal | Physical | 100 | `pay_day` | **VANILLA** | Plain formula. |
| Fire Punch | 75 | Fire | Physical | 100 | `may_burn` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Ice Punch | 75 | Ice | Physical | 100 | `may_freeze` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Thunder Punch | 75 | Electric | Physical | 100 | `may_paralyze` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Scratch | 40 | Normal | Physical | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Vise Grip | 55 | Normal | Physical | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Guillotine | None | Normal | Physical | 30 | `one_hit_ko` | **NONE** | power=null -> calc returns None (no damage shown). Same as gens 1-4: OHKO has no representation. |
| Razor Wind | 80 | Normal | Special | 100 | `razor_wind` | **CHECK** | Two-turn; hit is plain formula. Razor Wind has a high crit ratio from gen 3 on; no flavor in gen 5 json (unverified). |
| Cut | 50 | Normal | Physical | 95 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Gust | 40 | Flying | Special | 100 | `double_damage_airborne_targets` | **OK*** | Gust: Fly bonus dropdown works by name. |
| Wing Attack | 60 | Flying | Physical | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Fly | 90 | Flying | Physical | 95 | `semi_invulnerable_turn` | **VANILLA** | Fly/Dig/Dive/Bounce: the hit is the plain formula. |
| Bind | 15 | Normal | Physical | 85 | `trap_enemy` | **VANILLA** | Whirlpool keeps its Dive-bonus dropdown; the others are plain. |
| Slam | 80 | Normal | Physical | 75 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Vine Whip | 35 | Grass | Physical | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Stomp | 65 | Normal | Physical | 100 | `bonus_minimize_damage_and_may_flinch` | **OK*** | Stomp: Minimize dropdown works by name. |
| Double Kick | 30 | Fighting | Physical | 100 | `two_hits` | **BROKEN** | DOUBLE_HIT_FLAVOR never matches: damage is for ONE hit, should be two. |
| Mega Kick | 120 | Normal | Physical | 75 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Jump Kick | 100 | Fighting | Physical | 95 | `recoil_on_miss` | **VANILLA** | Jump Kick: plain formula. |
| Rolling Kick | 60 | Fighting | Physical | 85 | `may_flinch` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Headbutt | 70 | Normal | Physical | 100 | `may_flinch` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Horn Attack | 65 | Normal | Physical | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Fury Attack | 15 | Normal | Physical | 85 | `two_to_five_hits` | **BROKEN** | FLAVOR_MULTI_HIT never matches: no "2 Hits".."5 Hits" dropdown is offered and only ONE hit is ever calculated. |
| Horn Drill | None | Normal | Physical | 30 | `one_hit_ko` | **NONE** | power=null -> calc returns None (no damage shown). Same as gens 1-4: OHKO has no representation. |
| Tackle | 50 | Normal | Physical | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Body Slam | 85 | Normal | Physical | 100 | `may_paralyze` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Wrap | 15 | Normal | Physical | 90 | `trap_enemy` | **VANILLA** | Whirlpool keeps its Dive-bonus dropdown; the others are plain. |
| Take Down | 90 | Normal | Physical | 85 | `quarter_recoil_on_hit` | **VANILLA** | Plain formula (recoil is not damage dealt). |
| Thrash | 120 | Normal | Physical | 100 | `rampage` | **VANILLA** | Plain formula. |
| Double-Edge | 120 | Normal | Physical | 100 | `third_recoil_on_hit` | **VANILLA** | Plain formula. |
| Poison Sting | 15 | Poison | Physical | 100 | `may_poison` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Twineedle | 25 | Bug | Physical | 100 | `two_hits_and_may_poison` | **BROKEN** | DOUBLE_HIT_FLAVOR never matches: damage is for ONE hit, should be two. |
| Pin Missile | 14 | Bug | Physical | 85 | `two_to_five_hits` | **BROKEN** | FLAVOR_MULTI_HIT never matches: no "2 Hits".."5 Hits" dropdown is offered and only ONE hit is ever calculated. |
| Bite | 60 | Dark | Physical | 100 | `may_flinch` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Sonic Boom | None | Normal | Special | 90 | `basic_hit` | **OK** | Handled by name ("Sonic Boom" spelling is in _FIXED_DAMAGE_BY_NAME). |
| Acid | 40 | Poison | Special | 100 | `may_lower_enemy_special_defense_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Ember | 40 | Fire | Special | 100 | `may_burn` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Flamethrower | 95 | Fire | Special | 100 | `may_burn` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Water Gun | 40 | Water | Special | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Hydro Pump | 120 | Water | Special | 80 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Surf | 95 | Water | Special | 100 | `double_damage_underwater_targets` | **OK*** | Surf: Dive bonus dropdown works by name. |
| Ice Beam | 95 | Ice | Special | 100 | `may_freeze` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Blizzard | 120 | Ice | Special | 70 | `blizzard` | **OK*** | Hail -> always hits handled. |
| Psybeam | 65 | Psychic | Special | 100 | `may_confuse` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Bubble Beam | 65 | Water | Special | 100 | `damage_lower_stat` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Aurora Beam | 65 | Ice | Special | 100 | `may_lower_enemy_attack_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Hyper Beam | 150 | Normal | Special | 90 | `recharge_on_hit` | **VANILLA** | Plain formula. |
| Peck | 35 | Flying | Physical | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Drill Peck | 80 | Flying | Physical | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Submission | 80 | Fighting | Physical | 80 | `quarter_recoil_on_hit` | **VANILLA** | Plain formula (recoil is not damage dealt). |
| Low Kick | None | Fighting | Physical | 100 | `weight_based_power` | **NONE** | power=null -> None before the weight branch. Gen 5 thresholds (kg): <10:20, <25:40, <50:60, <100:80, <200:100, else 120 (same as gen 4). |
| Counter | None | Fighting | Physical | 100 | `counter` | **NONE** | Needs incoming damage; not implemented in any gen. |
| Seismic Toss | None | Fighting | Physical | 100 | `level_based_damage` | **OK** | Handled by name in damage_calc.get_special_damage_override (level damage, immunity respected). |
| Strength | 80 | Normal | Physical | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Absorb | 20 | Grass | Special | 100 | `drain_hp_on_hit` | **VANILLA** | Plain formula. |
| Mega Drain | 40 | Grass | Special | 100 | `drain_hp_on_hit` | **VANILLA** | Plain formula. |
| Razor Leaf | 55 | Grass | Physical | 95 | `high_crit_rate` | **BROKEN** | Gen 5 moves.json has no attack_flavor, so FLAVOR_HIGH_CRIT never matches: crit stage stays 0 (1/16 instead of 1/8). |
| Solar Beam | 120 | Grass | Special | 100 | `basic_hit` | **BROKEN** | Gen 5 name "Solar Beam" != const.SOLAR_BEAM_MOVE_NAME "SolarBeam": the x0.5 in rain/sand/hail never applies in gen 5. |
| Petal Dance | 120 | Grass | Special | 100 | `rampage` | **VANILLA** | Plain formula. |
| Dragon Rage | None | Dragon | Special | 100 | `fixed_damage` | **OK** | Handled by name (Dragon Rage 40). |
| Fire Spin | 35 | Fire | Special | 85 | `trap_enemy` | **VANILLA** | Whirlpool keeps its Dive-bonus dropdown; the others are plain. |
| Thunder Shock | 40 | Electric | Special | 100 | `may_paralyze` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Thunderbolt | 95 | Electric | Special | 100 | `may_paralyze` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Thunder | 120 | Electric | Special | 70 | `thunder` | **OK*** | Rain -> always hits is handled; sun -> 50% is not. |
| Rock Throw | 50 | Rock | Physical | 90 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Earthquake | 100 | Ground | Physical | 100 | `double_damage_underground_targets` | **OK*** | Earthquake: Dig bonus dropdown works by name. |
| Fissure | None | Ground | Physical | 30 | `one_hit_ko` | **NONE** | power=null -> calc returns None (no damage shown). Same as gens 1-4: OHKO has no representation. |
| Dig | 80 | Ground | Physical | 100 | `semi_invulnerable_turn` | **VANILLA** | Fly/Dig/Dive/Bounce: the hit is the plain formula. |
| Confusion | 50 | Psychic | Special | 100 | `may_confuse` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Psychic | 90 | Psychic | Special | 100 | `may_lower_enemy_special_defense_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Quick Attack | 40 | Normal | Physical | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Rage | 20 | Normal | Physical | 100 | `rage` | **OK*** | Dropdown "1".."6" multiplies damage by n. From gen 3 Rage raises the Attack STAGE by 1 per hit taken instead; the multiplier model is wrong for gen 5 too. |
| Night Shade | None | Ghost | Special | 100 | `level_based_damage` | **OK** | Handled by name in damage_calc.get_special_damage_override (level damage, immunity respected). |
| Bide | None | Normal | Physical | None | `bide` | **NONE** | Needs accumulated damage; not implemented in any gen. |
| Self-Destruct | 200 | Normal | Physical | 100 | `basic_hit` | **BROKEN** | Gen 5 name is "Self-Destruct" but const.SELFDESTRUCT_MOVE_NAME is "Selfdestruct": Damp immunity is NOT applied to it (Explosion still is). Defense halving is (accidentally) not applied, which happens to be right for gen 5. |
| Egg Bomb | 100 | Normal | Physical | 75 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Lick | 20 | Ghost | Physical | 100 | `may_paralyze` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Smog | 20 | Poison | Special | 70 | `may_poison` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Sludge | 65 | Poison | Special | 100 | `may_poison` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Bone Club | 65 | Ground | Physical | 85 | `may_flinch` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Fire Blast | 120 | Fire | Special | 85 | `may_burn` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Waterfall | 80 | Water | Physical | 100 | `may_flinch` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Clamp | 35 | Water | Physical | 85 | `trap_enemy` | **VANILLA** | Whirlpool keeps its Dive-bonus dropdown; the others are plain. |
| Swift | 60 | Normal | Special | None | `always_hit` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Skull Bash | 100 | Normal | Physical | 100 | `raise_self_defense_1_and_charge_turn` | **VANILLA** | Skull Bash: plain formula. |
| Spike Cannon | 20 | Normal | Physical | 100 | `two_to_five_hits` | **BROKEN** | FLAVOR_MULTI_HIT never matches: no "2 Hits".."5 Hits" dropdown is offered and only ONE hit is ever calculated. |
| Constrict | 10 | Normal | Physical | 100 | `may_lower_enemy_speed_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| High Jump Kick | 130 | Fighting | Physical | 90 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Dream Eater | 100 | Psychic | Special | 100 | `dream_eater` | **VANILLA** | Plain formula (requires sleeping target; no gating in any gen). |
| Barrage | 15 | Normal | Physical | 85 | `two_to_five_hits` | **BROKEN** | FLAVOR_MULTI_HIT never matches: no "2 Hits".."5 Hits" dropdown is offered and only ONE hit is ever calculated. |
| Leech Life | 20 | Bug | Physical | 100 | `drain_hp_on_hit` | **VANILLA** | Plain formula. |
| Sky Attack | 140 | Flying | Physical | 90 | `sky_attack` | **CHECK** | Two-turn move; hit is the plain formula. Sky Attack has a high crit ratio from gen 3 on; no flavor in gen 5 json, so crit stage stays 0 (unverified). |
| Bubble | 20 | Water | Special | 100 | `may_lower_enemy_speed_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Dizzy Punch | 70 | Normal | Physical | 100 | `may_confuse` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Psywave | None | Psychic | Special | 80 | `psywave` | **NONE** | power=null -> None. Gen 5 Psywave = floor(level * (rand(0..100)+50) / 100), min 1 (unverified, no decomp). |
| Crabhammer | 90 | Water | Physical | 90 | `high_crit_rate` | **BROKEN** | Gen 5 moves.json has no attack_flavor, so FLAVOR_HIGH_CRIT never matches: crit stage stays 0 (1/16 instead of 1/8). |
| Explosion | 250 | Normal | Physical | 100 | `self_destruct` | **CHECK** | Gen 5 removed the defense halving; the app still halves the defense (gen 2-4 rule). Damp immunity works. |
| Fury Swipes | 18 | Normal | Physical | 80 | `two_to_five_hits` | **BROKEN** | FLAVOR_MULTI_HIT never matches: no "2 Hits".."5 Hits" dropdown is offered and only ONE hit is ever calculated. |
| Bonemerang | 50 | Ground | Physical | 90 | `two_hits` | **BROKEN** | DOUBLE_HIT_FLAVOR never matches: damage is for ONE hit, should be two. |
| Rock Slide | 75 | Rock | Physical | 90 | `may_flinch` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Hyper Fang | 80 | Normal | Physical | 90 | `may_flinch` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Tri Attack | 80 | Normal | Special | 100 | `tri_attack` | **VANILLA** | Plain formula. |
| Super Fang | None | Normal | Physical | 90 | `super_fang` | **NONE** | Needs target current HP (half, min 1); not implemented in any gen. |
| Slash | 70 | Normal | Physical | 100 | `high_crit_rate` | **BROKEN** | Gen 5 moves.json has no attack_flavor, so FLAVOR_HIGH_CRIT never matches: crit stage stays 0 (1/16 instead of 1/8). |
| Struggle | 50 | Normal | Physical | None | `quarter_max_hp_recoil` | **CHECK** | Typeless from gen 5: hits Ghost, no STAB. App treats it as Normal (Ghost immune, Normal STAB). |
| Triple Kick | 10 | Fighting | Physical | 90 | `triple_kick` | **OK*** | Dropdown "1"/"2"/"3" multiplies a single kick; the app never sums the three kicks (same as gen 2-4). |
| Thief | 40 | Dark | Physical | 100 | `thief` | **VANILLA** | Plain 40. |
| Flame Wheel | 60 | Fire | Physical | 100 | `thaw_self_and_may_burn` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Snore | 40 | Normal | Special | 100 | `snore` | **VANILLA** | Plain formula (requires sleeping user). |
| Flail | None | Normal | Physical | 100 | `reversal` | **NONE** | power=null -> None. Gen 5 Flail/Reversal keep the 48-step table (p = floor(48*curHP/maxHP): <2:200, <5:150, <10:100, <17:80, <33:40, else 20). |
| Aeroblast | 100 | Flying | Special | 95 | `high_crit_rate` | **BROKEN** | Gen 5 moves.json has no attack_flavor, so FLAVOR_HIGH_CRIT never matches: crit stage stays 0 (1/16 instead of 1/8). |
| Reversal | None | Fighting | Physical | 100 | `reversal` | **NONE** | power=null -> None. See Flail. |
| Powder Snow | 40 | Ice | Special | 100 | `may_freeze` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Mach Punch | 40 | Fighting | Physical | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Feint Attack | 60 | Dark | Physical | None | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Sludge Bomb | 90 | Poison | Special | 100 | `may_poison` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Mud-Slap | 20 | Ground | Special | 100 | `may_lower_enemy_accuracy_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Octazooka | 65 | Water | Special | 85 | `may_lower_enemy_accuracy_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Zap Cannon | 120 | Electric | Special | 50 | `may_paralyze` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Icy Wind | 55 | Ice | Special | 95 | `may_lower_enemy_speed_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Bone Rush | 25 | Ground | Physical | 90 | `two_to_five_hits` | **BROKEN** | FLAVOR_MULTI_HIT never matches: no "2 Hits".."5 Hits" dropdown is offered and only ONE hit is ever calculated. |
| Outrage | 120 | Dragon | Physical | 100 | `rampage` | **VANILLA** | Plain formula. |
| Giga Drain | 75 | Grass | Special | 100 | `drain_hp_on_hit` | **VANILLA** | Plain formula. |
| Rollout | 30 | Rock | Physical | 90 | `rollout` | **OK*** | Dropdown by name; the exponent convention is audited in gen 4 (turn n should be 30*2^(n-1), Defense Curl doubles again). |
| False Swipe | 40 | Normal | Physical | 100 | `non_lethal_damage` | **VANILLA** | False Swipe: plain formula (leaves 1 HP; not modelled). |
| Spark | 65 | Electric | Physical | 100 | `may_paralyze` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Fury Cutter | 20 | Bug | Physical | 95 | `fury_cutter` | **CHECK** | Gen 5: 20 -> 40 -> 80 -> 160 (cap after the 4th hit). App: 2^(n-1) for n=1..6 with no cap ("5" -> 320, "6" -> 640 equivalents). |
| Steel Wing | 70 | Steel | Physical | 90 | `may_raise_self_defense_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Return | None | Normal | Physical | 100 | `return` | **NONE** | power=null -> None. floor(happiness*10/25), min 1 - the "102".."1" dropdown exists but is never reached. |
| Present | None | Normal | Physical | 90 | `present` | **NONE** | Not implemented in any gen: 40% 40 / 30% 80 / 10% 120 / 20% heals 1/4. power=null -> None. |
| Frustration | None | Normal | Physical | 100 | `frustration` | **NONE** | power=null -> None; no dropdown in any gen. floor((255-happiness)*10/25), min 1. |
| Sacred Fire | 100 | Fire | Physical | 95 | `thaw_self_and_may_burn` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Magnitude | 1 | Ground | Physical | 100 | `magnitude` | **OK*** | Dropdown "Mag 4".."Mag 10" + Dig bonus works (power 10/30/50/70/90/110/150). |
| Dynamic Punch | 100 | Fighting | Physical | 50 | `may_confuse` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Megahorn | 120 | Bug | Physical | 85 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Dragon Breath | 60 | Dragon | Special | 100 | `may_paralyze` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Pursuit | 40 | Dark | Physical | 100 | `pursuit` | **OK*** | Dropdown works by name. |
| Rapid Spin | 20 | Normal | Physical | 100 | `clear_hazards` | **VANILLA** | Rapid Spin: plain formula. |
| Iron Tail | 100 | Steel | Physical | 75 | `may_lower_enemy_defense_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Metal Claw | 50 | Steel | Physical | 95 | `may_raise_self_attack_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Vital Throw | 70 | Fighting | Physical | None | `always_hit` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Hidden Power | 60 | Normal | Special | 100 | `hidden_power` | **OK*** | json power 60 is ignored: the app recomputes 30-70 from IVs (correct for gen 5; IV-based power was removed in gen 6). Same formula as gen 4. |
| Cross Chop | 100 | Fighting | Physical | 80 | `high_crit_rate` | **BROKEN** | Gen 5 moves.json has no attack_flavor, so FLAVOR_HIGH_CRIT never matches: crit stage stays 0 (1/16 instead of 1/8). |
| Twister | 40 | Dragon | Special | 100 | `may_flinch_and_double_damage_airborne_targets` | **OK*** | Twister: Fly bonus dropdown works by name. |
| Crunch | 80 | Dark | Physical | 100 | `may_lower_enemy_defense_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Mirror Coat | None | Psychic | Special | 100 | `mirror_coat` | **NONE** | Needs incoming damage; not implemented in any gen. |
| Extreme Speed | 80 | Normal | Physical | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Ancient Power | 60 | Rock | Special | 100 | `damage_raise_stat` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Shadow Ball | 80 | Ghost | Special | 100 | `may_lower_enemy_special_defense_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Future Sight | 100 | Psychic | Special | 100 | `future_sight` | **CHECK** | App: no STAB, no crit (gen 2-4 rule). Gen 5: normal damage calc when it hits, incl. STAB and crits (unverified). |
| Rock Smash | 40 | Fighting | Physical | 100 | `may_lower_enemy_defense_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Whirlpool | 35 | Water | Special | 85 | `trap_enemy` | **VANILLA** | Whirlpool keeps its Dive-bonus dropdown; the others are plain. |
| Beat Up | None | Dark | Physical | 100 | `beat_up` | **NONE** | power=null -> None. Gen 5: one hit per healthy party member, power = floor(baseAtk/10) + 5, typed Dark, uses the user Attack stat. |
| Fake Out | 40 | Normal | Physical | 100 | `fake_out` | **VANILLA** | Plain formula. |
| Uproar | 90 | Normal | Special | 100 | `uproar` | **VANILLA** | Plain 90. |
| Spit Up | None | Normal | Special | 100 | `spit_up` | **NONE** | power=null -> None. 100 * stockpiles. |
| Heat Wave | 100 | Fire | Special | 90 | `may_burn` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Facade | 70 | Normal | Physical | 100 | `facade` | **OK*** | Dropdown works by name. (Gen 5: burn still halves physical damage, which the app never models.) |
| Focus Punch | 150 | Fighting | Physical | 100 | `focus_punch` | **VANILLA** | Plain formula. |
| Smelling Salts | 60 | Normal | Physical | 100 | `basic_hit` | **BROKEN** | Gen 5 name "Smelling Salts" != SMELLING_SALT_MOVE_NAME "SmellingSalt": no Paralysis-bonus dropdown and no x2 in gen 5. |
| Superpower | 120 | Fighting | Physical | 100 | `lower_self_attack_defense_1` | **VANILLA** | Plain formula. |
| Revenge | 60 | Fighting | Physical | 100 | `double_power_if_damaged` | **OK*** | Revenge/Avalanche dropdowns work by name. |
| Brick Break | 75 | Fighting | Physical | 100 | `destroy_screens` | **OK*** | Screen bypass by name works. |
| Knock Off | 20 | Dark | Physical | 100 | `knock_off` | **VANILLA** | Plain formula (gen 5: no power boost yet). |
| Endeavor | None | Normal | Physical | 100 | `endeavor` | **NONE** | Needs both current HPs; not implemented in any gen. |
| Eruption | 150 | Fire | Special | 100 | `eruption` | **OK*** | Dropdown HP% works by name (150 * HP% / 100). |
| Secret Power | 70 | Normal | Physical | 100 | `secret_power` | **VANILLA** | Plain 70 power; terrain only changes the secondary effect. |
| Dive | 80 | Water | Physical | 100 | `semi_invulnerable_turn` | **VANILLA** | Fly/Dig/Dive/Bounce: the hit is the plain formula. |
| Arm Thrust | 15 | Fighting | Physical | 100 | `two_to_five_hits` | **BROKEN** | FLAVOR_MULTI_HIT never matches: no "2 Hits".."5 Hits" dropdown is offered and only ONE hit is ever calculated. |
| Luster Purge | 70 | Psychic | Special | 100 | `may_lower_enemy_special_defense_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Mist Ball | 70 | Psychic | Special | 100 | `may_lower_enemy_special_attack_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Blaze Kick | 85 | Fire | Physical | 90 | `may_burn_and_high_crit_rate` | **BROKEN** | High-crit stage not applied (no attack_flavor). |
| Ice Ball | 30 | Ice | Physical | 90 | `rollout` | **OK*** | Same as Rollout. |
| Needle Arm | 60 | Grass | Physical | 100 | `may_flinch` | **CHECK** | Minimize bonus dropdown is offered but gen 5 removed the bonus for Needle Arm. |
| Hyper Voice | 90 | Normal | Special | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Poison Fang | 50 | Poison | Physical | 100 | `may_toxic` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Crush Claw | 75 | Normal | Physical | 95 | `may_lower_enemy_defense_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Blast Burn | 150 | Fire | Special | 90 | `recharge_on_hit` | **VANILLA** | Plain formula. |
| Hydro Cannon | 150 | Water | Special | 90 | `recharge_on_hit` | **VANILLA** | Plain formula. |
| Meteor Mash | 100 | Steel | Physical | 85 | `may_raise_self_attack_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Astonish | 30 | Ghost | Physical | 100 | `may_flinch` | **CHECK** | Minimize bonus dropdown is offered but gen 5 removed the bonus for Astonish (only Stomp/Steamroller keep it). |
| Weather Ball | 50 | Normal | Special | 100 | `weather_ball` | **OK*** | x2 power + weather type works; Fog is offered as a weather in gen 5 but does not exist there. |
| Air Cutter | 55 | Flying | Special | 95 | `high_crit_rate` | **BROKEN** | Gen 5 moves.json has no attack_flavor, so FLAVOR_HIGH_CRIT never matches: crit stage stays 0 (1/16 instead of 1/8). |
| Overheat | 140 | Fire | Special | 90 | `lower_self_special_attack_2` | **VANILLA** | Plain formula. |
| Rock Tomb | 50 | Rock | Physical | 80 | `may_lower_enemy_speed_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Silver Wind | 60 | Bug | Special | 100 | `may_raise_self_all_stats_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Water Spout | 150 | Water | Special | 100 | `eruption` | **OK*** | Dropdown HP% works by name (150 * HP% / 100). |
| Signal Beam | 75 | Bug | Special | 100 | `may_confuse` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Shadow Punch | 60 | Ghost | Physical | None | `always_hit` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Extrasensory | 80 | Psychic | Special | 100 | `may_flinch` | **CHECK** | Minimize bonus dropdown is offered but gen 5 removed the bonus for Extrasensory. |
| Sky Uppercut | 85 | Fighting | Physical | 90 | `hits_airborne_targets` | **VANILLA** | Sky Uppercut: plain formula. |
| Sand Tomb | 35 | Ground | Physical | 85 | `trap_enemy` | **VANILLA** | Whirlpool keeps its Dive-bonus dropdown; the others are plain. |
| Sheer Cold | None | Ice | Special | 30 | `one_hit_ko` | **NONE** | power=null -> calc returns None (no damage shown). Same as gens 1-4: OHKO has no representation. |
| Muddy Water | 95 | Water | Special | 85 | `may_lower_enemy_accuracy_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Bullet Seed | 25 | Grass | Physical | 100 | `two_to_five_hits` | **BROKEN** | FLAVOR_MULTI_HIT never matches: no "2 Hits".."5 Hits" dropdown is offered and only ONE hit is ever calculated. |
| Aerial Ace | 60 | Flying | Physical | None | `always_hit` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Icicle Spear | 25 | Ice | Physical | 100 | `two_to_five_hits` | **BROKEN** | FLAVOR_MULTI_HIT never matches: no "2 Hits".."5 Hits" dropdown is offered and only ONE hit is ever calculated. |
| Dragon Claw | 80 | Dragon | Physical | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Frenzy Plant | 150 | Grass | Special | 90 | `recharge_on_hit` | **VANILLA** | Plain formula. |
| Bounce | 85 | Flying | Physical | 85 | `semi_invulnerable_turn` | **VANILLA** | Fly/Dig/Dive/Bounce: the hit is the plain formula. |
| Mud Shot | 55 | Ground | Special | 95 | `may_lower_enemy_speed_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Poison Tail | 50 | Poison | Physical | 100 | `may_poison_and_high_crit_rate` | **BROKEN** | High-crit stage not applied (no attack_flavor). |
| Covet | 60 | Normal | Physical | 100 | `thief` | **VANILLA** | Plain 60. |
| Volt Tackle | 120 | Electric | Physical | 100 | `third_recoil_may_paralyze` | **VANILLA** | Plain formula. |
| Magical Leaf | 60 | Grass | Special | None | `always_hit` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Leaf Blade | 90 | Grass | Physical | 100 | `high_crit_rate` | **BROKEN** | Gen 5 moves.json has no attack_flavor, so FLAVOR_HIGH_CRIT never matches: crit stage stays 0 (1/16 instead of 1/8). |
| Rock Blast | 25 | Rock | Physical | 90 | `two_to_five_hits` | **BROKEN** | FLAVOR_MULTI_HIT never matches: no "2 Hits".."5 Hits" dropdown is offered and only ONE hit is ever calculated. |
| Shock Wave | 60 | Electric | Special | None | `always_hit` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Water Pulse | 60 | Water | Special | 100 | `may_confuse` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Doom Desire | 140 | Steel | Special | 100 | `future_sight` | **CHECK** | Same as Future Sight. |
| Psycho Boost | 140 | Psychic | Special | 90 | `lower_self_special_attack_2` | **VANILLA** | Plain formula. |
| Wake-Up Slap | 60 | Fighting | Physical | 100 | `wake_up_slap` | **OK*** | Dropdown works by name. |
| Hammer Arm | 100 | Fighting | Physical | 90 | `lower_self_speed_1` | **VANILLA** | Plain formula. |
| Gyro Ball | None | Steel | Physical | 100 | `gyro_ball` | **NONE** | power=null -> None. Gen 5: min(150, 25*targetSpeed/userSpeed + 1) - NOTE the gen-4 branch has the ratio inverted (user/target); see gen 4 audit. |
| Brine | 65 | Water | Special | 100 | `brine` | **OK*** | Dropdown works by name. |
| Natural Gift | None | Normal | Physical | 100 | `natural_gift` | **NONE** | power=null -> None. Gen 5 berry powers are the gen-4 table +20 (Cheri 80 ... Liechi 100 etc.); constants still hold the gen-4 table. |
| Feint | 30 | Normal | Physical | 100 | `feint` | **VANILLA** | Plain formula (30 in gen 5). |
| Pluck | 60 | Flying | Physical | 100 | `pluck` | **VANILLA** | Plain formula. |
| Metal Burst | None | Steel | Physical | 100 | `metal_burst` | **NONE** | Needs incoming damage (x1.5); not implemented in any gen. |
| U-turn | 70 | Bug | Physical | 100 | `u_turn` | **VANILLA** | Plain formula. |
| Close Combat | 120 | Fighting | Physical | 100 | `lower_self_defense_special_defense_1` | **VANILLA** | Plain formula. |
| Payback | 50 | Dark | Physical | 100 | `payback` | **OK*** | Dropdown works by name. |
| Assurance | 50 | Dark | Physical | 100 | `assurance` | **OK*** | Dropdown works by name. |
| Fling | None | Dark | Physical | 100 | `fling` | **NONE** | power=null -> None. Power from the held item (10..130 table). |
| Trump Card | None | Normal | Special | None | `trump_card` | **NONE** | power=null -> None. Table by PP remaining after use: 4+:40, 3:50, 2:60, 1:80, 0:200. |
| Wring Out | None | Normal | Special | 100 | `crush_grip` | **NONE** | power=null -> None. Same as Crush Grip. |
| Punishment | None | Dark | Physical | 100 | `punishment` | **NONE** | power=null -> None. 60 + 20*positive target stages, cap 200. |
| Last Resort | 140 | Normal | Physical | 100 | `last_resort` | **VANILLA** | Plain formula (140). |
| Sucker Punch | 80 | Dark | Physical | 100 | `sucker_punch` | **VANILLA** | Plain formula (conditional success not modelled). |
| Flare Blitz | 120 | Fire | Physical | 100 | `third_recoil_and_may_burn` | **VANILLA** | Plain formula. |
| Force Palm | 60 | Fighting | Physical | 100 | `may_paralyze` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Aura Sphere | 90 | Fighting | Special | None | `always_hit` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Poison Jab | 80 | Poison | Physical | 100 | `may_poison` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Dark Pulse | 80 | Dark | Special | 100 | `may_flinch` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Night Slash | 70 | Dark | Physical | 100 | `high_crit_rate` | **BROKEN** | Gen 5 moves.json has no attack_flavor, so FLAVOR_HIGH_CRIT never matches: crit stage stays 0 (1/16 instead of 1/8). |
| Aqua Tail | 90 | Water | Physical | 90 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Seed Bomb | 80 | Grass | Physical | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Air Slash | 75 | Flying | Special | 95 | `may_flinch` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| X-Scissor | 80 | Bug | Physical | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Bug Buzz | 90 | Bug | Special | 100 | `may_lower_enemy_special_defense_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Dragon Pulse | 90 | Dragon | Special | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Dragon Rush | 100 | Dragon | Physical | 75 | `may_flinch` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Power Gem | 70 | Rock | Special | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Drain Punch | 75 | Fighting | Physical | 100 | `drain_hp_on_hit` | **VANILLA** | Plain formula. |
| Vacuum Wave | 40 | Fighting | Special | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Focus Blast | 120 | Fighting | Special | 70 | `may_lower_enemy_special_defense_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Energy Ball | 80 | Grass | Special | 100 | `may_lower_enemy_special_defense_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Brave Bird | 120 | Flying | Physical | 100 | `third_recoil_on_hit` | **VANILLA** | Plain formula. |
| Earth Power | 90 | Ground | Special | 100 | `may_lower_enemy_special_defense_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Giga Impact | 150 | Normal | Physical | 90 | `recharge_on_hit` | **VANILLA** | Plain formula. |
| Bullet Punch | 40 | Steel | Physical | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Avalanche | 60 | Ice | Physical | 100 | `double_power_if_damaged` | **OK*** | Revenge/Avalanche dropdowns work by name. |
| Ice Shard | 40 | Ice | Physical | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Shadow Claw | 70 | Ghost | Physical | 100 | `high_crit_rate` | **BROKEN** | Gen 5 moves.json has no attack_flavor, so FLAVOR_HIGH_CRIT never matches: crit stage stays 0 (1/16 instead of 1/8). |
| Thunder Fang | 65 | Electric | Physical | 95 | `may_paralyze_and_may_flinch` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Ice Fang | 65 | Ice | Physical | 95 | `may_freeze_and_may_flinch` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Fire Fang | 65 | Fire | Physical | 95 | `may_burn_and_may_flinch` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Shadow Sneak | 40 | Ghost | Physical | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Mud Bomb | 65 | Ground | Special | 85 | `may_lower_enemy_accuracy_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Psycho Cut | 70 | Psychic | Physical | 100 | `high_crit_rate` | **BROKEN** | Gen 5 moves.json has no attack_flavor, so FLAVOR_HIGH_CRIT never matches: crit stage stays 0 (1/16 instead of 1/8). |
| Zen Headbutt | 80 | Psychic | Physical | 90 | `may_flinch` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Mirror Shot | 65 | Steel | Special | 85 | `may_lower_enemy_accuracy_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Flash Cannon | 80 | Steel | Special | 100 | `may_lower_enemy_special_defense_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Rock Climb | 90 | Normal | Physical | 85 | `may_confuse` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Draco Meteor | 140 | Dragon | Special | 90 | `lower_self_special_attack_2` | **VANILLA** | Plain formula. |
| Discharge | 80 | Electric | Special | 100 | `may_paralyze` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Lava Plume | 80 | Fire | Special | 100 | `may_burn` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Leaf Storm | 140 | Grass | Special | 90 | `lower_self_special_attack_2` | **VANILLA** | Plain formula. |
| Power Whip | 120 | Grass | Physical | 85 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Rock Wrecker | 150 | Rock | Physical | 90 | `recharge_on_hit` | **VANILLA** | Plain formula. |
| Cross Poison | 70 | Poison | Physical | 100 | `may_poison_and_high_crit_rate` | **BROKEN** | High-crit stage not applied (no attack_flavor). |
| Gunk Shot | 120 | Poison | Physical | 70 | `may_poison` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Iron Head | 80 | Steel | Physical | 100 | `may_flinch` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Magnet Bomb | 60 | Steel | Physical | None | `always_hit` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Stone Edge | 100 | Rock | Physical | 80 | `high_crit_rate` | **BROKEN** | Gen 5 moves.json has no attack_flavor, so FLAVOR_HIGH_CRIT never matches: crit stage stays 0 (1/16 instead of 1/8). |
| Grass Knot | None | Grass | Special | 100 | `weight_based_power` | **NONE** | power=null -> None before the weight branch (same table as Low Kick). |
| Chatter | 60 | Flying | Special | 100 | `chatter` | **VANILLA** | Plain formula. |
| Judgment | 100 | Normal | Special | 100 | `judgment` | **CHECK** | Plate lookup: "Earth Plate" -> Dark (typo, should be Ground). Otherwise fine. |
| Bug Bite | 60 | Bug | Physical | 100 | `pluck` | **VANILLA** | Plain formula. |
| Charge Beam | 50 | Electric | Special | 90 | `may_raise_self_special_attack_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Wood Hammer | 120 | Grass | Physical | 100 | `third_recoil_on_hit` | **VANILLA** | Plain formula. |
| Aqua Jet | 40 | Water | Physical | 100 | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Attack Order | 90 | Bug | Physical | 100 | `high_crit_rate` | **BROKEN** | Gen 5 moves.json has no attack_flavor, so FLAVOR_HIGH_CRIT never matches: crit stage stays 0 (1/16 instead of 1/8). |
| Head Smash | 150 | Rock | Physical | 80 | `half_recoil` | **VANILLA** | Plain formula. |
| Double Hit | 35 | Normal | Physical | 90 | `two_hits` | **BROKEN** | DOUBLE_HIT_FLAVOR never matches: damage is for ONE hit, should be two. |
| Roar of Time | 150 | Dragon | Special | 90 | `recharge_on_hit` | **VANILLA** | Plain formula. |
| Spacial Rend | 100 | Dragon | Special | 95 | `high_crit_rate` | **BROKEN** | Gen 5 moves.json has no attack_flavor, so FLAVOR_HIGH_CRIT never matches: crit stage stays 0 (1/16 instead of 1/8). |
| Crush Grip | None | Normal | Physical | 100 | `crush_grip` | **NONE** | power=null -> None. Gen 5: floor(120 * curHP / maxHP), min 1 (unverified). |
| Magma Storm | 120 | Fire | Special | 75 | `trap_enemy` | **VANILLA** | Whirlpool keeps its Dive-bonus dropdown; the others are plain. |
| Seed Flare | 120 | Grass | Special | 85 | `may_lower_enemy_special_defense_2` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Ominous Wind | 60 | Ghost | Special | 100 | `may_raise_self_all_stats_1` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Shadow Force | 120 | Ghost | Physical | 100 | `shadow_force` | **VANILLA** | Plain formula. |
| Psyshock | 80 | Psychic | Special | 100 | `basic_hit` | **NONE** | Special move that uses the target DEFENSE (and Def stages). App uses Sp.Def. |
| Venoshock | 65 | Poison | Special | 100 | `basic_hit` | **NONE** | Power x2 (130) if the target is poisoned. Needs a dropdown. |
| Smack Down | 50 | Rock | Physical | 100 | `may_unknown` | **CHECK** | Plain 50; grounds the target (Flying/Levitate lose Ground immunity afterwards) and hits Fly/Bounce/Sky Drop users. Not modelled. |
| Storm Throw | 40 | Fighting | Physical | 100 | `high_crit_rate` | **NONE** | ALWAYS a critical hit in gen 5 (json mislabels it high_crit_rate). App: crit stage 0. |
| Flame Burst | 70 | Fire | Special | 100 | `basic_hit` | **VANILLA** | Plain formula (splash damage to ally ignored). |
| Sludge Wave | 95 | Poison | Special | 100 | `may_poison` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Heavy Slam | None | Steel | Physical | 100 | `basic_hit` | **NONE** | Power by weight ratio user/target: >=5x:120, >=4x:100, >=3x:80, >=2x:60, else 40. Species weights exist in the gen 5 pokemon db. power=null -> None today. |
| Synchronoise | 70 | Psychic | Special | 100 | `basic_hit` | **NONE** | Fails unless the target shares a type with the user. App always deals damage. |
| Electro Ball | None | Electric | Special | 100 | `basic_hit` | **NONE** | Power by speed ratio user/target: >=4x:150, >=3x:120, >=2x:80, >=1x:60, else 40 (battle speeds incl. stages/items). power=null -> None today. |
| Flame Charge | 50 | Fire | Physical | 100 | `damage_raise_stat` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Low Sweep | 60 | Fighting | Physical | 100 | `damage_lower_stat` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Acid Spray | 40 | Poison | Special | 100 | `damage_lower_stat` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Foul Play | 95 | Dark | Physical | 100 | `basic_hit` | **NONE** | Uses the TARGET Attack stat and Attack stages (user ability/items still apply). App uses the user Attack. |
| Round | 60 | Normal | Special | 100 | `basic_hit` | **NONE** | Power x2 if an ally used Round earlier the same turn (doubles only). Vanilla in singles; acceptable to leave as 60. |
| Echoed Voice | 40 | Normal | Special | 100 | `basic_hit` | **NONE** | Power 40, +40 per consecutive turn it is used by anyone, cap 200. Needs a "1".."5" dropdown. |
| Chip Away | 70 | Normal | Physical | 100 | `basic_hit` | **NONE** | Ignores the target Defense and evasion stages. App applies them. |
| Clear Smog | 50 | Poison | Special | None | `basic_hit` | **VANILLA** | Plain formula. Only the gen-5 formula-level differences (section 2) apply. |
| Stored Power | 20 | Psychic | Special | 100 | `basic_hit` | **NONE** | Power 20 + 20 * (sum of the user positive stat stages, incl. accuracy/evasion), cap 860. Derivable from attacking_stage_modifiers with no new input. |
| Scald | 80 | Water | Special | 100 | `may_burn` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Hex | 50 | Ghost | Special | 100 | `basic_hit` | **NONE** | Power x2 (100) if the target has a major status. Needs a "Status Bonus" dropdown like Facade. |
| Sky Drop | 60 | Flying | Physical | 100 | `basic_hit` | **VANILLA** | Two-turn; the hit is the plain formula. |
| Circle Throw | 60 | Fighting | Physical | 90 | `basic_hit` | **VANILLA** | Plain formula. |
| Incinerate | 30 | Fire | Special | 100 | `basic_hit` | **VANILLA** | Plain 30 (destroys the target berry; no damage effect). |
| Acrobatics | 55 | Flying | Physical | 100 | `basic_hit` | **NONE** | Power x2 (110) when the user holds no item. App: flat 55. Easy: check attacking_pkmn.held_item is None. |
| Retaliate | 70 | Normal | Physical | 100 | `basic_hit` | **NONE** | Power x2 (140) if an ally fainted on the previous turn. Needs a dropdown. |
| Final Gambit | None | Fighting | Special | 100 | `basic_hit` | **NONE** | Damage = user CURRENT HP (fixed, no formula). power=null -> None today. Needs an HP input (default cur_stats.hp = full HP). |
| Inferno | 100 | Fire | Special | 50 | `may_burn` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Water Pledge | 50 | Water | Special | 100 | `basic_hit` | **NONE** | Power 150 when combined with another Pledge (doubles only). Vanilla in singles. |
| Fire Pledge | 50 | Fire | Special | 100 | `basic_hit` | **NONE** | Power 150 when combined (doubles only). Vanilla in singles. |
| Grass Pledge | 50 | Grass | Special | 100 | `basic_hit` | **NONE** | Power 150 when combined (doubles only). Vanilla in singles. |
| Volt Switch | 70 | Electric | Special | 100 | `basic_hit` | **VANILLA** | Plain formula. |
| Struggle Bug | 30 | Bug | Special | 100 | `damage_lower_stat` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Bulldoze | 60 | Ground | Physical | 100 | `damage_lower_stat` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Frost Breath | 40 | Ice | Special | 90 | `high_crit_rate` | **NONE** | ALWAYS a critical hit in gen 5 (json mislabels it high_crit_rate). App: crit stage 0. |
| Dragon Tail | 60 | Dragon | Physical | 90 | `basic_hit` | **VANILLA** | Plain formula. |
| Electroweb | 55 | Electric | Special | 95 | `damage_lower_stat` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Wild Charge | 90 | Electric | Physical | 100 | `quarter_recoil_on_hit` | **VANILLA** | Plain formula (recoil is not damage dealt). |
| Drill Run | 80 | Ground | Physical | 95 | `high_crit_rate` | **BROKEN** | Gen 5 moves.json has no attack_flavor, so FLAVOR_HIGH_CRIT never matches: crit stage stays 0 (1/16 instead of 1/8). |
| Dual Chop | 40 | Dragon | Physical | 90 | `two_hits` | **BROKEN** | DOUBLE_HIT_FLAVOR never matches: damage is for ONE hit, should be two. |
| Heart Stamp | 60 | Psychic | Physical | 100 | `may_flinch` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Horn Leech | 75 | Grass | Physical | 100 | `drain_hp` | **VANILLA** | Horn Leech: plain formula. |
| Sacred Sword | 90 | Fighting | Physical | 100 | `basic_hit` | **NONE** | Ignores the target Defense and evasion stages. App applies them. |
| Razor Shell | 75 | Water | Physical | 95 | `damage_lower_stat` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Heat Crash | None | Fire | Physical | 100 | `basic_hit` | **NONE** | Same weight-ratio table as Heavy Slam. power=null -> None today. |
| Leaf Tornado | 65 | Grass | Special | 90 | `damage_lower_stat` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Steamroller | 65 | Bug | Physical | 100 | `may_flinch` | **NONE** | Gen 5 gives Steamroller (and Stomp) x2 vs a Minimized target; no dropdown for Steamroller. |
| Night Daze | 85 | Dark | Special | 95 | `damage_lower_stat` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Psystrike | 100 | Psychic | Special | 100 | `basic_hit` | **NONE** | Special move that uses the target DEFENSE. App uses Sp.Def. |
| Tail Slap | 25 | Normal | Physical | 85 | `two_to_five_hits` | **BROKEN** | FLAVOR_MULTI_HIT never matches: no "2 Hits".."5 Hits" dropdown is offered and only ONE hit is ever calculated. |
| Hurricane | 120 | Flying | Special | 70 | `may_confuse` | **CHECK** | Accuracy: always hits in rain, 50% in sun; hits Fly/Bounce/Sky Drop users. get_move_accuracy only special-cases Thunder/Blizzard. |
| Head Charge | 120 | Normal | Physical | 100 | `quarter_recoil_on_hit` | **VANILLA** | Plain formula (recoil is not damage dealt). |
| Gear Grind | 50 | Steel | Physical | 85 | `two_hits` | **BROKEN** | DOUBLE_HIT_FLAVOR never matches: damage is for ONE hit, should be two. |
| Searing Shot | 100 | Fire | Special | 100 | `may_burn` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Techno Blast | 85 | Normal | Special | 100 | `basic_hit` | **NONE** | Type follows the held Drive (Burn/Douse/Chill/Shock -> Fire/Water/Ice/Electric), else Normal. App: always Normal. |
| Relic Song | 75 | Normal | Special | 100 | `may_sleep` | **VANILLA** | Plain formula. |
| Secret Sword | 85 | Fighting | Special | 100 | `basic_hit` | **NONE** | Special move that uses the target DEFENSE. App uses Sp.Def. |
| Glaciate | 65 | Ice | Special | 95 | `damage_lower_stat` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Bolt Strike | 130 | Electric | Physical | 85 | `may_paralyze` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Blue Flare | 130 | Fire | Special | 85 | `may_burn` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Fiery Dance | 80 | Fire | Special | 100 | `damage_raise_stat` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Freeze Shock | 140 | Ice | Physical | 90 | `may_paralyze` | **VANILLA** | Two-turn; hit is plain formula. |
| Ice Burn | 140 | Ice | Special | 90 | `may_burn` | **VANILLA** | Two-turn; hit is plain formula. |
| Snarl | 55 | Dark | Special | 95 | `damage_lower_stat` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| Icicle Crash | 85 | Ice | Physical | 90 | `may_flinch` | **VANILLA** | Plain formula; secondary effect does not change damage. |
| V-create | 180 | Fire | Physical | 95 | `damage_raise_stat` | **VANILLA** | Plain formula (180). |
| Fusion Flare | 100 | Fire | Special | 100 | `basic_hit` | **NONE** | Power x2 if Fusion Bolt was used earlier in the turn. Vanilla otherwise; needs a dropdown if wanted. |
| Fusion Bolt | 100 | Electric | Physical | 100 | `basic_hit` | **NONE** | Power x2 if Fusion Flare was used earlier in the turn. Vanilla otherwise. |
