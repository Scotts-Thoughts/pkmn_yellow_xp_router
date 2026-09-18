//! Port of `pkmn/gen_2/pkmn_damage_calc.py`.

use xpr_core::consts;
use xpr_core::{floor_div, floor_mul_ratio};
use xpr_data::gen_consts as gc;
use xpr_data::model::{EnemyPkmn, Move, PokemonSpecies, StageModifiers};
use xpr_data::stats;
use xpr_data::GenData;

use crate::damage::{self, py_int, DamageRange};
use crate::DamageArgs;

const MIN_RANGE: i64 = 217;
const MAX_RANGE: i64 = 255;

/// `get_crit_rate`
pub fn get_crit_rate(_pkmn: &EnemyPkmn, mv: &Move) -> f64 {
    if mv.has_flavor(gc::FLAVOR_ONE_HIT_KO) {
        return 0.0;
    }
    if mv.has_flavor(consts::FLAVOR_HIGH_CRIT) {
        return 1.0 / 4.0;
    }
    17.0 / 256.0
}

/// `get_move_accuracy` (gen 2 lives on the GenTwo object).
pub fn get_move_accuracy(pkmn: &EnemyPkmn, mv: &Move, defending: &EnemyPkmn, weather: &str) -> Option<f64> {
    if mv.name == gc::THUNDER_MOVE && weather == consts::WEATHER_RAIN {
        return None;
    }
    if mv.name == gc::THUNDER_MOVE && weather == consts::WEATHER_SUN {
        return Some(50.0);
    }
    if mv.has_flavor(gc::FLAVOR_ONE_HIT_KO) {
        if pkmn.level < defending.level {
            return Some(0.0);
        }
        let v = (76 + 2 * (pkmn.level - defending.level)).min(255);
        return Some((v as f64) / 256.0 * 100.0);
    }
    mv.accuracy.map(|a| a as f64)
}

/// `_type_effectiveness_multiplier` (x10 scale, table order)
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

/// `calculate_present_gold_silver_damage`
fn calculate_present_gold_silver(
    gen: &GenData,
    attacking_pkmn: &EnemyPkmn,
    attacking_species: &PokemonSpecies,
    defending_species: &PokemonSpecies,
    base_power: i64,
    is_crit: bool,
) -> Option<DamageRange> {
    let a_value = type_effectiveness_multiplier(gen, consts::TYPE_NORMAL, &defending_species.first_type, &defending_species.second_type);
    let is_stab = attacking_species.first_type == consts::TYPE_NORMAL || attacking_species.second_type == consts::TYPE_NORMAL;
    let d_value = if is_stab {
        1
    } else {
        let d = gc::present_type_id(&attacking_species.second_type).unwrap_or(1);
        if d == 0 {
            1
        } else {
            d
        }
    };
    let level_proxy = gc::present_type_id(&defending_species.second_type).unwrap_or(0);
    let base = floor_div(2 * level_proxy, 5) + 2;
    let mut temp = base * base_power * a_value;
    temp = floor_div(temp, d_value);
    temp = floor_div(temp, 50);
    if gen.held_item_boosts.get(attacking_pkmn.held_item_str()).map(|s| s.as_str()) == Some(consts::TYPE_NORMAL) {
        temp = floor_mul_ratio(temp, 11, 10);
    }
    if is_crit {
        temp *= 2;
    }
    temp = temp.min(997) + 2;
    if is_stab {
        temp += floor_div(temp, 2);
    }
    temp = floor_div(temp * a_value, 10);
    if temp <= 0 {
        temp = 1;
    }
    Some(DamageRange::from_rolls(temp, MIN_RANGE, MAX_RANGE))
}

/// `calculate_gen_two_damage`
pub fn calculate_damage(gen: &GenData, a: &DamageArgs) -> Option<DamageRange> {
    let mv = a.mv;
    let attacking_pkmn = a.attacking;
    let defending_pkmn = a.defending;
    let custom = a.custom_move_data;
    let weather = a.weather;
    let version_name = gen.effective_version();
    let is_gold_silver = version_name == consts::GOLD_VERSION || version_name == consts::SILVER_VERSION;

    let attacking_species = gen.pkmn_db().get_pkmn(&attacking_pkmn.name)?;
    let defending_species = gen.pkmn_db().get_pkmn(&defending_pkmn.name)?;

    if let Some(r) = damage::get_special_damage_override(
        mv,
        attacking_pkmn,
        Some((&defending_species.first_type, &defending_species.second_type)),
        Some(&gen.type_chart),
    ) {
        return Some(r);
    }

    let (move_type, mut base_power): (String, Option<i64>) = if mv.name == consts::HIDDEN_POWER_MOVE_NAME {
        (
            stats::hidden_power_type_gen2(&attacking_pkmn.dvs).to_string(),
            Some(stats::hidden_power_base_power_gen2(&attacking_pkmn.dvs)),
        )
    } else {
        (mv.move_type.clone(), mv.base_power)
    };
    let d1 = defending_species.first_type.as_str();
    let d2 = defending_species.second_type.as_str();

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
        return None;
    }

    if mv.name == gc::COUNTER_MOVE || mv.name == gc::MIRROR_COAT_MOVE || mv.name == gc::BIDE_MOVE {
        return None;
    }

    let is_present = mv.name == gc::PRESENT_MOVE;
    if is_present {
        if custom.contains(gc::PRESENT_HEAL) {
            return None;
        }
        let bp = py_int(custom).unwrap_or(40);
        base_power = Some(bp);
        if is_gold_silver {
            return calculate_present_gold_silver(gen, attacking_pkmn, attacking_species, defending_species, bp, a.is_crit);
        }
    } else if mv.name == gc::FRUSTRATION_MOVE {
        base_power = Some(py_int(custom).unwrap_or(0));
    }

    let mut base_power = match base_power {
        None | Some(0) => return None,
        Some(bp) => bp,
    };

    if mv.name != consts::FUTURE_SIGHT_MOVE_NAME && is_immune(gen, &move_type, d1, d2) {
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
    let mut attacking_stages = *original_attacking_stages;
    let mut defending_stages = *original_defending_stages;

    let mut is_crit = a.is_crit;
    if mv.name == consts::FLAIL_MOVE_NAME || mv.name == consts::REVERSAL_MOVE_NAME || mv.name == consts::FUTURE_SIGHT_MOVE_NAME {
        is_crit = false;
    }

    let is_special = gen.special_types.contains(&move_type);
    let mut ignore_badge_boosts = false;
    if is_crit {
        if is_special && attacking_stages.special_attack_stage <= defending_stages.special_defense_stage {
            ignore_badge_boosts = true;
            attacking_stages = StageModifiers::default();
            defending_stages = StageModifiers::default();
        } else if !is_special && attacking_stages.attack_stage <= defending_stages.defense_stage {
            ignore_badge_boosts = true;
            attacking_stages = StageModifiers::default();
            defending_stages = StageModifiers::default();
        }
    }

    let mut attacking_battle_stats = match a.attacking_battle_stats {
        Some(s) => *s,
        None => attacking_pkmn.get_battle_stats(&attacking_stages, ignore_badge_boosts, None),
    };
    let defending_battle_stats = match a.defending_battle_stats {
        Some(s) => *s,
        None => defending_pkmn.get_battle_stats(&defending_stages, ignore_badge_boosts, None),
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
            defender_light_screen && !ignore_badge_boosts,
        )
    } else {
        (
            attacking_battle_stats.attack,
            defending_battle_stats.defense,
            defender_reflect && !ignore_badge_boosts,
        )
    };
    if doubled_def {
        defending_stat *= 2;
    }

    while attacking_stat >= 256 || defending_stat >= 256 {
        attacking_stat = floor_div(attacking_stat, 4).max(1);
        defending_stat = floor_div(defending_stat, 4).max(1);
        if is_gold_silver {
            attacking_stat &= 0xFF;
            defending_stat &= 0xFF;
            break;
        }
    }

    if defending_pkmn.name == gc::DITTO && defending_pkmn.held_item_str() == gc::METAL_POWDER {
        let boosted = defending_stat + (defending_stat >> 1);
        if boosted > 255 {
            attacking_stat = (attacking_stat >> 1).max(1);
            defending_stat = (defending_stat + (defending_stat >> 1)) >> 1;
        } else {
            defending_stat = boosted;
        }
    }

    if mv.name == consts::EXPLOSION_MOVE_NAME || mv.name == consts::SELFDESTRUCT_MOVE_NAME {
        defending_stat = floor_div(defending_stat, 2).max(1);
    }

    let is_future_sight = mv.name == consts::FUTURE_SIGHT_MOVE_NAME;
    let mut is_stab = attacking_species.first_type == move_type || attacking_species.second_type == move_type;
    if is_future_sight {
        is_stab = false;
    }

    let held_boost_type = gen.held_item_boosts.get(attacking_pkmn.held_item_str()).map(|s| s.as_str());
    let held_item_boost = if mv.name == consts::STRUGGLE_MOVE_NAME {
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

    let mut temp = 2 * attacking_pkmn.level;
    temp = floor_div(temp, 5) + 2;
    temp *= base_power;
    temp *= attacking_stat;
    temp = floor_div(temp, defending_stat);
    temp = floor_div(temp, 50);
    if held_item_boost {
        temp = floor_mul_ratio(temp, 11, 10);
    }
    if is_crit && mv.name != consts::FLAIL_MOVE_NAME && mv.name != consts::REVERSAL_MOVE_NAME && mv.name != consts::FUTURE_SIGHT_MOVE_NAME {
        temp *= 2;
    }
    temp = temp.min(997) + 2;

    let is_triple_kick = mv.name == gc::TRIPLE_KICK_MOVE;

    let finish = |base_temp: i64| -> Option<DamageRange> {
        let mut t = base_temp;
        let mut weather_boost = false;
        let mut weather_penalty = false;
        if !is_future_sight {
            if weather == consts::WEATHER_RAIN {
                weather_boost = move_type == consts::TYPE_WATER;
                weather_penalty = move_type == consts::TYPE_FIRE || mv.name == consts::SOLAR_BEAM_MOVE_NAME;
            } else if weather == consts::WEATHER_SUN {
                weather_boost = move_type == consts::TYPE_FIRE;
                weather_penalty = move_type == consts::TYPE_WATER;
            }
        }
        if weather_boost {
            t = floor_mul_ratio(t, 3, 2);
        } else if weather_penalty {
            t = floor_div(t, 2);
        }
        if !is_future_sight && badge_type_boost {
            t += (t >> 3).max(1);
        }
        if is_stab {
            t += floor_div(t, 2);
        }
        if !is_future_sight {
            if let Some(row) = gen.type_chart.get(&move_type) {
                for (test_type, eff) in row {
                    if test_type == d1 || test_type == d2 {
                        if eff == consts::SUPER_EFFECTIVE {
                            t *= 2;
                        } else if eff == consts::NOT_VERY_EFFECTIVE {
                            t = floor_div(t, 2);
                        }
                    }
                }
            }
        }
        let mut move_modifier: f64 = 1.0;
        if mv.name == gc::ROLLOUT_MOVE {
            let turns = py_int(custom).unwrap_or(6);
            move_modifier = 2f64.powf((turns - 1) as f64);
        } else if mv.name == gc::FURY_CUTTER_MOVE {
            let turns = py_int(custom)?.min(5);
            move_modifier = 2f64.powf((turns - 1) as f64);
        } else if mv.name == gc::RAGE_MOVE {
            move_modifier = py_int(custom)? as f64;
        }
        // `t *= move_modifier` in Python promotes to float for the pow moves;
        // every value involved is an integer-valued double, so the product
        // (and `int(t)` below) are exact.
        let mut tf = (t as f64) * move_modifier;
        if tf <= 0.0 {
            tf = 1.0;
        }
        if mv.name == consts::FLAIL_MOVE_NAME || mv.name == consts::REVERSAL_MOVE_NAME {
            return Some(DamageRange::single(tf as i64));
        }
        let double_damage = if [gc::GUST_MOVE, gc::TWISTER_MOVE, gc::EARTHQUAKE_MOVE, gc::STOMP_MOVE, gc::PURSUIT_MOVE].contains(&mv.name.as_str()) {
            !custom.contains(gc::NO_BONUS)
        } else if mv.name == gc::MAGNITUDE_MOVE {
            custom.contains(gc::DIG_BONUS)
        } else {
            false
        };
        let pairs: Vec<(i64, i64)> = (MIN_RANGE..=MAX_RANGE)
            .map(|n| {
                // math.floor((t * numerator) / MAX_RANGE) with float t
                let mut dmg = ((tf * (n as f64)) / (MAX_RANGE as f64)).floor() as i64;
                if dmg < 1 {
                    dmg = 1;
                }
                if double_damage {
                    dmg = (dmg * 2).min(65535);
                }
                (dmg, 1)
            })
            .collect();
        DamageRange::from_pairs(&pairs, 1)
    };

    if is_triple_kick {
        let num_kicks = custom
            .chars()
            .next()
            .and_then(|c| c.to_string().parse::<i64>().ok())
            .unwrap_or(3)
            .clamp(1, 3);
        let mut result: Option<DamageRange> = None;
        for kick in 1..=num_kicks {
            let kick_result = finish(temp * kick)?;
            result = Some(match result {
                None => kick_result,
                Some(r) => r.add(&kick_result),
            });
        }
        return result;
    }

    let mut result = finish(temp)?;

    let mut multi_hit = 1;
    if mv.has_flavor(consts::DOUBLE_HIT_FLAVOR) {
        multi_hit = 2;
    } else if mv.has_flavor(consts::FLAVOR_MULTI_HIT) {
        if custom.contains(consts::MULTI_HIT_2) {
            multi_hit = 2;
        } else if custom.contains(consts::MULTI_HIT_3) {
            multi_hit = 3;
        } else if custom.contains(consts::MULTI_HIT_4) {
            multi_hit = 4;
        } else if custom.contains(consts::MULTI_HIT_5) {
            multi_hit = 5;
        }
    }

    if multi_hit > 1 {
        let other = if is_crit {
            let sub = DamageArgs {
                attacking: attacking_pkmn,
                mv,
                defending: defending_pkmn,
                attacking_stages: Some(original_attacking_stages),
                defending_stages: Some(original_defending_stages),
                attacking_field: None,
                defending_field: a.defending_field,
                is_crit: false,
                custom_move_data: "",
                weather,
                is_double_battle: false,
                attacking_battle_stats: None,
                defending_battle_stats: None,
                attacker_is_enemy: false,
            };
            calculate_damage(gen, &sub)?
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
