//! Gen 2 (Gold / Silver / Crystal) damage, critical-hit and accuracy
//! formulas, verified against pokecrystal (`engine/battle/effect_commands.asm`
//! `BattleCommand_Critical`, `DamageStats`/`TruncateHL_BC`/`DittoMetalPowder`,
//! `DamageCalc`, `Stab`, `DamageVariation`, `CheckHit`, `ConstantDamage`,
//! `OHKO`, `EndLoop`; `engine/battle/misc.asm` weather and badge type boosts;
//! `engine/battle/move_effects/*.asm`) and diffed against pokegold (the
//! Gold/Silver `TruncateHL_BC` single pass and the Present register clobber).

use xpr_core::consts;
use xpr_core::{floor_div, floor_mul_ratio};
use xpr_data::gen_consts as gc;
use xpr_data::model::{EnemyPkmn, Move, StageModifiers};
use xpr_data::stats;
use xpr_data::GenData;

use crate::damage::{self, py_int, DamageRange, HitModel};
use crate::DamageArgs;

const MIN_RANGE: i64 = 217;
const MAX_RANGE: i64 = 255;

/// Moves whose scripts have no `critical` command.
fn never_crits(name: &str) -> bool {
    matches!(
        name,
        consts::FLAIL_MOVE_NAME
            | consts::REVERSAL_MOVE_NAME
            | consts::FUTURE_SIGHT_MOVE_NAME
            | consts::DRAGON_RAGE_MOVE_NAME
            | "SonicBoom"
            | "Seismic Toss"
            | "Night Shade"
            | "Psywave"
            | "Super Fang"
            | "Counter"
            | "Mirror Coat"
            | "Bide"
            | "Guillotine"
            | "Horn Drill"
            | "Fissure"
    )
}

/// `BattleCommand_Critical`: Chansey + Lucky Punch or Farfetch'd + Stick
/// jump straight to stage 2; otherwise Focus Energy (+1, not modelled),
/// the high-crit move list (+2) and Scope Lens (+1) add up.
/// `CriticalHitChances`: 17, 32, 64, 85, 128 out of 256.
pub fn get_crit_rate(pkmn: &EnemyPkmn, mv: &Move) -> f64 {
    // `ret z` on a 0 power byte (Hidden Power's byte is 1: it crits normally)
    if never_crits(&mv.name) || (mv.base_power.unwrap_or(0) == 0 && mv.name != consts::HIDDEN_POWER_MOVE_NAME) {
        return 0.0;
    }
    let held = pkmn.held_item_str();
    let stage = if (pkmn.name == gc::CHANSEY && held == gc::LUCKY_PUNCH) || (pkmn.name == gc::FARFETCHD && held == gc::STICK) {
        2
    } else {
        let mut c = 0;
        if mv.has_flavor(consts::FLAVOR_HIGH_CRIT) {
            c += 2;
        }
        if held == gc::SCOPE_LENS {
            c += 1;
        }
        c
    };
    let byte = match stage {
        0 => 17,
        1 => 32,
        2 => 64,
        3 => 85,
        _ => 128,
    };
    (byte as f64) / 256.0
}

/// `BattleCommand_CheckHit`: the accuracy byte is `acc * 255 / 100`, scaled
/// by the accuracy / evasion stages (`AccuracyLevelMultipliers`, min 1 per
/// step, cap 255), minus 20 for the target's BrightPowder; 255 always hits,
/// otherwise the move hits when `BattleRandom < byte`. Thunder is set to
/// 255 in rain (never misses) and 128 in sun.
pub fn get_move_accuracy(a: &DamageArgs) -> Option<f64> {
    let pkmn = a.attacking;
    let defending = a.defending;
    let mv = a.mv;
    let weather = a.weather;
    if mv.name == gc::THUNDER_MOVE && weather == consts::WEATHER_RAIN {
        return None;
    }
    let mut byte: i64 = if mv.has_flavor(gc::FLAVOR_ONE_HIT_KO) {
        // `BattleCommand_OHKO`: +2 per level above the target, capped at 255
        if pkmn.level < defending.level {
            return Some(0.0);
        }
        (mv.accuracy.unwrap_or(0) * 255 / 100 + 2 * (pkmn.level - defending.level)).min(255)
    } else if mv.name == gc::THUNDER_MOVE && weather == consts::WEATHER_SUN {
        128
    } else {
        mv.accuracy? * 255 / 100
    };
    let default_stages = StageModifiers::default();
    let acc_stage = a.attacking_stages.unwrap_or(&default_stages).accuracy_stage.clamp(-6, 6);
    let eva_stage = a.defending_stages.unwrap_or(&default_stages).evasion_stage.clamp(-6, 6);
    let (n1, d1) = gc::ACCURACY_STAGE_RATIOS[(acc_stage + 6) as usize];
    byte = floor_div(byte * n1, d1).max(1);
    let (n2, d2) = gc::ACCURACY_STAGE_RATIOS[(6 - eva_stage) as usize];
    byte = floor_div(byte * n2, d2).max(1);
    byte = byte.min(255);
    if defending.held_item_str() == gc::BRIGHT_POWDER {
        byte = (byte - 20).max(0);
    }
    if byte >= 255 {
        return None;
    }
    Some((byte as f64) / 256.0 * 100.0)
}

/// `CheckTypeMatchup` (x10 scale, table order)
fn type_effectiveness_multiplier(gen: &GenData, move_type: &str, first: &str, second: &str) -> i64 {
    let mut result = 10;
    if let Some(row) = gen.type_chart.get(move_type) {
        for (test_type, eff) in row {
            if test_type == first || test_type == second {
                match eff.as_str() {
                    consts::SUPER_EFFECTIVE => result = result * 20 / 10,
                    consts::NOT_VERY_EFFECTIVE => result = result * 5 / 10,
                    consts::IMMUNE => result = 0,
                    _ => {}
                }
            }
        }
    }
    result
}

fn is_immune(gen: &GenData, move_type: &str, first: &str, second: &str) -> bool {
    gen.effectiveness(move_type, first) == Some(consts::IMMUNE) || gen.effectiveness(move_type, second) == Some(consts::IMMUNE)
}

fn multi_hit_count(a: &DamageArgs) -> i64 {
    let mv = a.mv;
    let custom = a.custom_move_data;
    if mv.has_flavor(consts::DOUBLE_HIT_FLAVOR) {
        2
    } else if mv.has_flavor(consts::FLAVOR_MULTI_HIT) {
        if custom.contains(consts::MULTI_HIT_2) {
            2
        } else if custom.contains(consts::MULTI_HIT_3) {
            3
        } else if custom.contains(consts::MULTI_HIT_4) {
            4
        } else if custom.contains(consts::MULTI_HIT_5) {
            5
        } else {
            1
        }
    } else {
        1
    }
}

fn triple_kick_kicks(custom: &str) -> i64 {
    custom
        .chars()
        .next()
        .and_then(|c| c.to_string().parse::<i64>().ok())
        .unwrap_or(3)
        .clamp(1, 3)
}

fn beat_up_hits(custom: &str) -> i64 {
    py_int(custom).unwrap_or(1).clamp(1, 6)
}

fn with_crit<'a>(a: &DamageArgs<'a>, is_crit: bool) -> DamageArgs<'a> {
    let mut sub = *a;
    sub.is_crit = is_crit;
    sub
}

/// `BattleCommand_BeatUp` + `damagecalc` + `damagevariation` for one hit:
/// the party member's base Attack, the target's base Defense, the member's
/// level and power 10; type-boost item and crit apply, `stab` never runs.
fn beat_up_hit(gen: &GenData, a: &DamageArgs, crit: bool) -> Option<DamageRange> {
    let attacking_species = gen.pkmn_db().get_pkmn(&a.attacking.name)?;
    let defending_species = gen.pkmn_db().get_pkmn(&a.defending.name)?;
    let item_boost = gen.held_item_boosts.get(a.attacking.held_item_str()).map(|s| s.as_str()) == Some(a.mv.move_type.as_str());
    let temp = gen2_damage_calc(a.attacking.level, a.mv.base_power.unwrap_or(10), attacking_species.stats.attack, defending_species.stats.defense.max(1), item_boost, crit)?;
    Some(DamageRange::from_rolls(temp, MIN_RANGE, MAX_RANGE))
}

/// `BattleCommand_DamageCalc` (after the Explosion halving): `(2L/5 + 2) *
/// bp * A / D / 50`, type-boost item `*110/100`, crit `*2` (cap 65535),
/// `min(997) + 2`.
fn gen2_damage_calc(level: i64, base_power: i64, attack: i64, defense: i64, item_boost: bool, crit: bool) -> Option<i64> {
    if base_power == 0 {
        return None;
    }
    let defense = defense.max(1);
    let mut temp = floor_div(2 * level, 5) + 2;
    temp *= base_power;
    temp *= attack;
    temp = floor_div(temp, defense);
    temp = floor_div(temp, 50);
    if item_boost {
        temp = floor_mul_ratio(temp, 11, 10);
    }
    if crit {
        temp = (temp * 2).min(65535);
    }
    Some(temp.min(997) + 2)
}

/// The per-hit ranges of a multi-hit use (multi-hit / two-hit moves, Beat
/// Up, Triple Kick); `None` for single hits.
pub fn hit_model(gen: &GenData, a: &DamageArgs) -> Option<HitModel> {
    let mv = a.mv;
    let custom = a.custom_move_data;
    if mv.name == gc::TRIPLE_KICK_MOVE {
        let kicks = triple_kick_kicks(custom);
        if kicks < 2 {
            return None;
        }
        let mut hits = Vec::new();
        for kick in 1..=kicks {
            let normal = triple_kick_single(gen, &with_crit(a, false), kick)?;
            let crit = triple_kick_single(gen, &with_crit(a, true), kick)?;
            hits.push((normal, crit));
        }
        return Some(HitModel { hits, accuracy_per_hit: false });
    }
    if mv.has_flavor(gc::FLAVOR_BEAT_UP) {
        let n = beat_up_hits(custom);
        if n < 2 {
            return None;
        }
        let normal = beat_up_hit(gen, a, false)?;
        let crit = beat_up_hit(gen, a, true)?;
        return Some(HitModel { hits: vec![(normal, crit); n as usize], accuracy_per_hit: false });
    }
    let n = multi_hit_count(a);
    if n < 2 {
        return None;
    }
    let normal = calculate_damage_impl(gen, &with_crit(a, false), true)?;
    let crit = calculate_damage_impl(gen, &with_crit(a, true), true)?;
    Some(HitModel { hits: vec![(normal, crit); n as usize], accuracy_per_hit: false })
}

fn triple_kick_single(gen: &GenData, a: &DamageArgs, kick: i64) -> Option<DamageRange> {
    let label = match kick {
        1 => "1 Kick",
        2 => "2 Kicks",
        _ => "3 Kicks",
    };
    let mut sub = *a;
    sub.custom_move_data = label;
    // the whole-use calc of "k kicks" minus "k-1 kicks" is not separable, so
    // compute the k-th kick directly
    calculate_damage_kick(gen, &sub, kick)
}

/// `calculate_gen_two_damage`: the whole use (every hit of a multi-hit move).
pub fn calculate_damage(gen: &GenData, a: &DamageArgs) -> Option<DamageRange> {
    calculate_damage_impl(gen, a, false)
}

/// One Triple Kick kick (`damagecalc, triplekick, stab, damagevariation`
/// with the k-th kick's damage multiplied by k before `stab`).
fn calculate_damage_kick(gen: &GenData, a: &DamageArgs, kick: i64) -> Option<DamageRange> {
    let mut sub = *a;
    let label = format!("__kick_{}", kick);
    sub.custom_move_data = &label;
    calculate_damage_impl(gen, &sub, true)
}

fn calculate_damage_impl(gen: &GenData, a: &DamageArgs, single_hit: bool) -> Option<DamageRange> {
    let mv = a.mv;
    let attacking_pkmn = a.attacking;
    let defending_pkmn = a.defending;
    let custom = a.custom_move_data;
    let weather = a.weather;
    let version_name = gen.effective_version();
    let is_gold_silver = version_name == consts::GOLD_VERSION || version_name == consts::SILVER_VERSION;

    let attacking_species = gen.pkmn_db().get_pkmn(&attacking_pkmn.name)?;
    let defending_species = gen.pkmn_db().get_pkmn(&defending_pkmn.name)?;
    let d1 = defending_species.first_type.as_str();
    let d2 = defending_species.second_type.as_str();

    if let Some(r) = damage::get_special_damage_override(mv, attacking_pkmn, Some((d1, d2)), Some(&gen.type_chart)) {
        return Some(r);
    }
    if [consts::DRAGON_RAGE_MOVE_NAME, "SonicBoom", "Seismic Toss", "Night Shade"].contains(&mv.name.as_str()) {
        return None; // immune
    }

    let (move_type, mut base_power): (String, Option<i64>) = if mv.name == consts::HIDDEN_POWER_MOVE_NAME {
        (
            stats::hidden_power_type_gen2(&attacking_pkmn.dvs).to_string(),
            Some(stats::hidden_power_base_power_gen2(&attacking_pkmn.dvs)),
        )
    } else {
        (mv.move_type.clone(), mv.base_power)
    };

    if mv.has_flavor(gc::FLAVOR_SUPER_FANG) {
        if is_immune(gen, &move_type, d1, d2) {
            return None;
        }
        return Some(DamageRange::single(floor_div(defending_pkmn.cur_stats.hp, 2).max(1)));
    }

    if mv.has_flavor(gc::FLAVOR_ONE_HIT_KO) {
        if is_immune(gen, &move_type, d1, d2) {
            return None;
        }
        if attacking_pkmn.level < defending_pkmn.level {
            return None;
        }
        return Some(DamageRange::single(defending_pkmn.cur_stats.hp));
    }

    if mv.has_flavor(gc::FLAVOR_BEAT_UP) {
        let n = if single_hit { 1 } else { beat_up_hits(custom) };
        let mut total = beat_up_hit(gen, a, a.is_crit)?;
        if n > 1 {
            let other = beat_up_hit(gen, a, false)?;
            for _ in 1..n {
                total = total.add(&other);
            }
        }
        return Some(total);
    }

    if mv.name == gc::COUNTER_MOVE || mv.name == gc::MIRROR_COAT_MOVE || mv.name == gc::BIDE_MOVE {
        // 2x the damage taken (dropdown); Ghost is immune to Counter and
        // Bide, Dark to Mirror Coat (`resettypematchup`)
        if is_immune(gen, &move_type, d1, d2) {
            return None;
        }
        let taken = py_int(custom)?;
        if taken <= 0 {
            return None;
        }
        return Some(DamageRange::single((taken * 2).min(65535)));
    }

    let is_present = mv.name == gc::PRESENT_MOVE;
    if is_present {
        if custom.contains(gc::PRESENT_HEAL) {
            return None;
        }
        base_power = Some(py_int(custom).unwrap_or(40));
    } else if mv.name == gc::FRUSTRATION_MOVE {
        base_power = Some(py_int(custom).unwrap_or(0));
    }

    let mut base_power = match base_power {
        None | Some(0) => return None,
        Some(bp) => bp,
    };

    let is_future_sight = mv.name == consts::FUTURE_SIGHT_MOVE_NAME;
    let is_struggle = mv.name == consts::STRUGGLE_MOVE_NAME;
    if !is_future_sight && !is_struggle && is_immune(gen, &move_type, d1, d2) {
        return None;
    }

    if mv.has_flavor(consts::FLAVOR_FIXED_DAMAGE) {
        return Some(DamageRange::single(base_power));
    } else if mv.has_flavor(consts::FLAVOR_LEVEL_DAMAGE) {
        return Some(DamageRange::single(attacking_pkmn.level));
    } else if mv.has_flavor(consts::FLAVOR_PSYWAVE) {
        let upper = floor_div(attacking_pkmn.level * 3, 2);
        let pairs: Vec<(i64, i64)> = (1..upper).map(|x| (x, 1)).collect();
        return DamageRange::from_pairs(&pairs, 1);
    }

    let default_stages = StageModifiers::default();
    let original_attacking_stages = a.attacking_stages.unwrap_or(&default_stages);
    let original_defending_stages = a.defending_stages.unwrap_or(&default_stages);

    let is_crit = a.is_crit && !never_crits(&mv.name);
    let is_special = gen.special_types.contains(&move_type) && !is_struggle;

    // `CheckDamageStatsCritical`: on a crit, unless the attacker's stage is
    // higher than the defender's, both stats reload without stages, badge
    // boosts or screens
    let mut use_unboosted = false;
    if is_crit {
        if is_special && original_attacking_stages.special_attack_stage <= original_defending_stages.special_defense_stage {
            use_unboosted = true;
        } else if !is_special && original_attacking_stages.attack_stage <= original_defending_stages.defense_stage {
            use_unboosted = true;
        }
    }

    let (mut attacking_battle_stats, defending_battle_stats) = if use_unboosted {
        (
            a.attacking_battle_stats
                .map(|s| *s)
                .unwrap_or_else(|| attacking_pkmn.get_battle_stats(&StageModifiers::default(), true, None)),
            a.defending_battle_stats
                .map(|s| *s)
                .unwrap_or_else(|| defending_pkmn.get_battle_stats(&StageModifiers::default(), true, None)),
        )
    } else {
        (
            a.attacking_battle_stats
                .map(|s| *s)
                .unwrap_or_else(|| attacking_pkmn.get_battle_stats(original_attacking_stages, false, None)),
            a.defending_battle_stats
                .map(|s| *s)
                .unwrap_or_else(|| defending_pkmn.get_battle_stats(original_defending_stages, false, None)),
        )
    };

    if (attacking_pkmn.name == gc::GEN2_CUBONE || attacking_pkmn.name == gc::GEN2_MAROWAK) && attacking_pkmn.held_item_str() == gc::THICK_CLUB {
        attacking_battle_stats.attack *= 2;
    } else if attacking_pkmn.name == gc::PIKACHU && attacking_pkmn.held_item_str() == gc::LIGHT_BALL {
        attacking_battle_stats.special_attack *= 2;
    }

    let defender_light_screen = a.defending_field.map(|f| f.light_screen).unwrap_or(false);
    let defender_reflect = a.defending_field.map(|f| f.reflect).unwrap_or(false);
    let (mut attacking_stat, mut defending_stat, doubled_def) = if is_special {
        (
            attacking_battle_stats.special_attack,
            defending_battle_stats.special_defense,
            defender_light_screen && !use_unboosted,
        )
    } else {
        (attacking_battle_stats.attack, defending_battle_stats.defense, defender_reflect && !use_unboosted)
    };
    if doubled_def {
        defending_stat *= 2;
    }

    // `TruncateHL_BC`
    while attacking_stat >= 256 || defending_stat >= 256 {
        attacking_stat = floor_div(attacking_stat, 4).max(1);
        defending_stat = floor_div(defending_stat, 4).max(1);
        if is_gold_silver {
            attacking_stat &= 0xFF;
            defending_stat &= 0xFF;
            break;
        }
    }

    // `DittoMetalPowder`
    if defending_pkmn.name == gc::DITTO && defending_pkmn.held_item_str() == gc::METAL_POWDER {
        let boosted = defending_stat + (defending_stat >> 1);
        if boosted > 255 {
            attacking_stat = (attacking_stat >> 1).max(1);
            defending_stat = boosted >> 1;
        } else {
            defending_stat = boosted;
        }
    }

    if mv.name == consts::EXPLOSION_MOVE_NAME || mv.name == consts::SELFDESTRUCT_MOVE_NAME {
        defending_stat = floor_div(defending_stat, 2).max(1);
    }

    let mut is_stab = attacking_species.first_type == move_type || attacking_species.second_type == move_type;
    if is_future_sight || is_struggle {
        is_stab = false;
    }

    let held_boost_type = gen.held_item_boosts.get(attacking_pkmn.held_item_str()).map(|s| s.as_str());
    let held_item_boost = if is_struggle {
        held_boost_type == Some(consts::TYPE_NORMAL)
    } else {
        held_boost_type == Some(move_type.as_str())
    };

    let badge_type_boost = match &attacking_pkmn.badges {
        None => false,
        Some(b) => match move_type.as_str() {
            consts::TYPE_FLYING => b.has("zephyr"),
            consts::TYPE_BUG => b.has("hive"),
            consts::TYPE_NORMAL => b.has("plain"),
            consts::TYPE_GHOST => b.has("fog"),
            consts::TYPE_FIGHTING => b.has("storm"),
            consts::TYPE_STEEL => b.has("mineral"),
            consts::TYPE_ICE => b.has("glacier"),
            consts::TYPE_DRAGON => b.has("rising"),
            consts::TYPE_ROCK => b.has("boulder"),
            consts::TYPE_WATER => b.has("cascade"),
            consts::TYPE_ELECTRIC => b.has("thunder"),
            consts::TYPE_GRASS => b.has("rainbow"),
            consts::TYPE_POISON => b.has("soul"),
            consts::TYPE_PSYCHIC => b.has("marsh"),
            consts::TYPE_FIRE => b.has("volcano"),
            consts::TYPE_GROUND => b.has("earth"),
            _ => false,
        },
    };

    if mv.name == gc::MAGNITUDE_MOVE {
        if custom.contains(gc::MAGNITUDE_4) {
            base_power = 10;
        } else if custom.contains(gc::MAGNITUDE_5) {
            base_power = 30;
        } else if custom.contains(gc::MAGNITUDE_6) {
            base_power = 50;
        } else if custom.contains(gc::MAGNITUDE_7) {
            base_power = 70;
        } else if custom.contains(gc::MAGNITUDE_8) {
            base_power = 90;
        } else if custom.contains(gc::MAGNITUDE_9) {
            base_power = 110;
        } else if custom.contains(gc::MAGNITUDE_10) {
            base_power = 150;
        }
    } else if mv.name == consts::FLAIL_MOVE_NAME || mv.name == consts::REVERSAL_MOVE_NAME {
        if custom.contains(gc::FLAIL_FULL_HP) {
            base_power = 20;
        } else if custom.contains(gc::FLAIL_HALF_HP) {
            base_power = 40;
        } else if custom.contains(gc::FLAIL_QUARTER_HP) {
            base_power = 80;
        } else if custom.contains(gc::FLAIL_TEN_PERCENT_HP) {
            base_power = 100;
        } else if custom.contains(gc::FLAIL_FIVE_PERCENT_HP) {
            base_power = 150;
        } else if custom.contains(gc::FLAIL_MIN_HP) {
            base_power = 200;
        }
    } else if mv.name == gc::RETURN_MOVE {
        if let Some(v) = py_int(custom) {
            base_power = v;
        }
    }

    // `damagecalc`
    let temp = if is_present && is_gold_silver {
        // Gold/Silver `BattleCommand_Present` calls `stab` without saving
        // the registers: level = target type-2 id, A = type matchup (x10),
        // D = user type-2 id (1 with STAB), power = 40 / 80 / 120
        let a_value = type_effectiveness_multiplier(gen, consts::TYPE_NORMAL, d1, d2);
        if a_value == 0 {
            return None;
        }
        let d_value = if is_stab {
            1
        } else {
            gc::present_type_id(&attacking_species.second_type).unwrap_or(1).max(1)
        };
        let level_proxy = gc::present_type_id(&defending_species.second_type).unwrap_or(0);
        gen2_damage_calc(level_proxy, base_power, a_value, d_value, held_item_boost, is_crit)?
    } else {
        gen2_damage_calc(attacking_pkmn.level, base_power, attacking_stat, defending_stat, held_item_boost, is_crit)?
    };

    // `triplekick`: the k-th kick is multiplied by k before `stab`
    let temp = if let Some(rest) = custom.strip_prefix("__kick_") {
        let kick = py_int(rest).unwrap_or(1);
        (temp * kick).min(65535)
    } else {
        temp
    };

    if mv.name == gc::TRIPLE_KICK_MOVE && !custom.starts_with("__kick_") {
        // whole use: the kicks summed; the crit range doubles the last kick
        let num_kicks = triple_kick_kicks(custom);
        let mut result: Option<DamageRange> = None;
        for kick in 1..=num_kicks {
            let mut sub = *a;
            sub.is_crit = a.is_crit && kick == num_kicks;
            let kick_result = calculate_damage_kick(gen, &sub, kick)?;
            result = Some(match result {
                None => kick_result,
                Some(r) => r.add(&kick_result),
            });
        }
        return result;
    }

    // `stab`: weather, badge type boost, STAB, type chart (table order,
    // min 1 after each step); Future Sight has no `stab` command at all,
    // Struggle returns from it immediately
    let mut t = temp;
    if !is_future_sight && !is_struggle {
        let mut weather_boost = false;
        let mut weather_penalty = false;
        if weather == consts::WEATHER_RAIN {
            weather_boost = move_type == consts::TYPE_WATER;
            weather_penalty = move_type == consts::TYPE_FIRE || mv.name == consts::SOLAR_BEAM_MOVE_NAME;
        } else if weather == consts::WEATHER_SUN {
            weather_boost = move_type == consts::TYPE_FIRE;
            weather_penalty = move_type == consts::TYPE_WATER;
        }
        if weather_boost {
            t = floor_mul_ratio(t, 3, 2).max(1).min(65535);
        } else if weather_penalty {
            t = floor_div(t, 2).max(1);
        }
        if badge_type_boost {
            t = (t + (t >> 3).max(1)).min(65535);
        }
        if is_stab {
            t += floor_div(t, 2);
        }
        if let Some(row) = gen.type_chart.get(&move_type) {
            for (test_type, eff) in row {
                if test_type == d1 || test_type == d2 {
                    if eff == consts::SUPER_EFFECTIVE {
                        t *= 2;
                    } else if eff == consts::NOT_VERY_EFFECTIVE {
                        t = floor_div(t, 2).max(1);
                    }
                }
            }
        }
    }

    // Rollout / Fury Cutter / Rage multiply the damage after `stab`
    if mv.name == gc::ROLLOUT_MOVE {
        let turns = py_int(custom).unwrap_or(6);
        for _ in 1..turns {
            t = (t * 2).min(65535);
        }
    } else if mv.name == gc::FURY_CUTTER_MOVE {
        let turns = py_int(custom)?.min(5);
        for _ in 1..turns {
            t = (t * 2).min(65535);
        }
    } else if mv.name == gc::RAGE_MOVE {
        t = (t * py_int(custom)?).min(65535);
    }
    if t <= 0 {
        t = 1;
    }
    if mv.name == consts::FLAIL_MOVE_NAME || mv.name == consts::REVERSAL_MOVE_NAME {
        return Some(DamageRange::single(t));
    }

    // `damagevariation`, then the post-roll doublings
    let double_damage = if [gc::GUST_MOVE, gc::TWISTER_MOVE, gc::EARTHQUAKE_MOVE, gc::STOMP_MOVE, gc::PURSUIT_MOVE].contains(&mv.name.as_str()) {
        !custom.contains(gc::NO_BONUS)
    } else if mv.name == gc::MAGNITUDE_MOVE {
        custom.contains(gc::DIG_BONUS)
    } else {
        false
    };
    let pairs: Vec<(i64, i64)> = if t < 2 {
        vec![(if double_damage { (t * 2).min(65535) } else { t }, 39)]
    } else {
        (MIN_RANGE..=MAX_RANGE)
            .map(|n| {
                let mut dmg = floor_div(t * n, MAX_RANGE).max(1);
                if double_damage {
                    dmg = (dmg * 2).min(65535);
                }
                (dmg, 1)
            })
            .collect()
    };
    let mut result = DamageRange::from_pairs(&pairs, 1)?;

    let multi_hit = if single_hit { 1 } else { multi_hit_count(a) };
    if multi_hit > 1 {
        // every hit rolls crit and damage on its own; the crit range doubles
        // one hit
        let other = if a.is_crit {
            calculate_damage_impl(gen, &with_crit(a, false), true)?
        } else {
            result.clone()
        };
        for _ in 1..multi_hit {
            result = result.add(&other);
        }
    }

    if mv.has_flavor(gc::FLAVOR_FALSE_SWIPE) {
        let cap = (defending_pkmn.cur_stats.hp - 1).max(1);
        let mut clamped: indexmap::IndexMap<i64, i64> = indexmap::IndexMap::new();
        for (d, c) in &result.damage_vals {
            *clamped.entry((*d).min(cap)).or_insert(0) += c;
        }
        result = DamageRange::new(&clamped, result.num_attacks)?;
    }

    Some(result)
}
