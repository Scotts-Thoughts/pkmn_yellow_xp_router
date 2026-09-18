//! Port of `pkmn/gen_4/pkmn_damage_calc.py`.

use xpr_core::consts;
use xpr_core::{floor_div, floor_mul_ratio};
use xpr_data::gen_consts as gc;
use xpr_data::model::{EnemyPkmn, FieldStatus, Move, StageModifiers};
use xpr_data::stats;
use xpr_data::GenData;

use crate::damage::{self, py_int, DamageRange};
use crate::DamageArgs;

const MIN_RANGE: i64 = 85;
const MAX_RANGE: i64 = 100;

fn crit_stage_rate(stage: i64) -> f64 {
    match stage {
        0 => 1.0 / 16.0,
        1 => 1.0 / 8.0,
        2 => 1.0 / 4.0,
        3 => 1.0 / 3.0,
        _ => 1.0 / 2.0,
    }
}

/// `get_crit_rate`
pub fn get_crit_rate(mon: &EnemyPkmn, mv: &Move, _custom: Option<&str>) -> f64 {
    let mut stage = 0;
    if mv.has_flavor(consts::FLAVOR_HIGH_CRIT) {
        stage += 1;
    }
    if mon.ability == gc::SUPER_LUCK {
        stage += 1;
    }
    let held = mon.held_item_str();
    if held == gc::SCOPE_LENS || held == gc::RAZOR_CLAW {
        stage += 1;
    }
    if mon.name == gc::CHANSEY && held == gc::LUCKY_PUNCH {
        stage += 2;
    }
    if mon.name == gc::FARFETCHD && held == gc::STICK {
        stage += 2;
    }
    crit_stage_rate(stage.min(4))
}

/// `get_move_accuracy`
pub fn get_move_accuracy(pkmn: &EnemyPkmn, mv: &Move, custom: &str, defending: &EnemyPkmn, weather: &str) -> Option<f64> {
    if pkmn.ability == gc::NO_GUARD || defending.ability == gc::NO_GUARD {
        return None;
    }
    let mut result: Option<i64> = if mv.name == gc::NATURE_POWER_MOVE {
        Some(gc::gen4_nature_power(custom).map(|(_, _, acc)| acc).unwrap_or(100))
    } else if gc::OHKO_MOVE_NAMES.contains(&mv.name.as_str()) {
        Some((30 + (pkmn.level - defending.level)).max(0))
    } else {
        mv.accuracy
    };
    if mv.name == gc::BLIZZARD_MOVE && weather == consts::WEATHER_HAIL {
        return None;
    }
    if mv.name == gc::THUNDER_MOVE && weather == consts::WEATHER_RAIN {
        return None;
    }
    if mv.name == gc::THUNDER_MOVE && weather == consts::WEATHER_SUN {
        result = Some(50);
    }
    let mut r = result?;
    if pkmn.ability == gc::COMPOUND_EYES {
        r = floor_mul_ratio(r, 13, 10).min(100);
    } else if pkmn.ability == gc::HUSTLE {
        let mut is_physical = mv.category == consts::CATEGORY_PHYSICAL;
        if mv.name == gc::NATURE_POWER_MOVE {
            is_physical = gc::GEN4_NATURE_POWER_PHYSICAL_TERRAINS.contains(&custom);
        }
        if is_physical {
            r = floor_div(r * 3277, 4096);
        }
    }
    let dheld = defending.held_item_str();
    if dheld == gc::BRIGHT_POWDER || dheld == gc::LAX_INCENSE {
        r = (r - 10).max(0);
    }
    if pkmn.held_item_str() == gc::WIDE_LENS && pkmn.ability != gc::KLUTZ {
        r = floor_mul_ratio(r, 11, 10).min(100);
    }
    if defending.ability == gc::SAND_VEIL && weather == consts::WEATHER_SANDSTORM {
        r = floor_div(r * 3277, 4096);
    }
    if defending.ability == gc::SNOW_CLOAK && weather == consts::WEATHER_HAIL {
        r = floor_div(r * 3277, 4096);
    }
    if weather == consts::WEATHER_FOG {
        r = floor_div(r * 6, 10);
    }
    Some(r as f64)
}

fn wonder_guard_blocks(gen: &GenData, move_type: &str, d1: &str, d2: &str) -> bool {
    let e1 = gen.effectiveness(move_type, d1);
    let e2 = gen.effectiveness(move_type, d2);
    if e1 == Some(consts::IMMUNE)
        || e1 == Some(consts::NOT_VERY_EFFECTIVE)
        || e2 == Some(consts::IMMUNE)
        || e2 == Some(consts::NOT_VERY_EFFECTIVE)
    {
        return true;
    }
    e1 != Some(consts::SUPER_EFFECTIVE) && e2 != Some(consts::SUPER_EFFECTIVE)
}

/// `calculate_gen_four_damage`
pub fn calculate_damage(gen: &GenData, a: &DamageArgs) -> Option<DamageRange> {
    let mv = a.mv;
    let attacking_pkmn = a.attacking;
    let defending_pkmn = a.defending;
    let custom = a.custom_move_data;
    let weather = a.weather;

    let default_field = FieldStatus::default();
    let attacking_field = a.attacking_field.unwrap_or(&default_field);
    let defending_field = a.defending_field.unwrap_or(&default_field);

    let attacking_ability: &str = if attacking_field.worry_seed {
        gc::INSOMNIA
    } else if attacking_field.gastro_acid {
        ""
    } else {
        &attacking_pkmn.ability
    };
    let defending_ability: &str = if defending_field.worry_seed {
        gc::INSOMNIA
    } else if defending_field.gastro_acid {
        ""
    } else {
        &defending_pkmn.ability
    };

    let default_stages = StageModifiers::default();
    let mut attacking_stages = *a.attacking_stages.unwrap_or(&default_stages);
    let mut defending_stages = *a.defending_stages.unwrap_or(&default_stages);

    let attacking_species = gen.pkmn_db().get_pkmn(&attacking_pkmn.name)?;
    let defending_species = gen.pkmn_db().get_pkmn(&defending_pkmn.name)?;

    let is_weather_active = damage::is_weather_active(attacking_ability, defending_ability, weather);
    let (a1, a2) = damage::apply_forecast(&attacking_species.first_type, &attacking_species.second_type, attacking_ability, weather, is_weather_active);
    let (d1, d2) = damage::apply_forecast(&defending_species.first_type, &defending_species.second_type, defending_ability, weather, is_weather_active);

    if let Some(r) = damage::get_special_damage_override(mv, attacking_pkmn, Some((&d1, &d2)), Some(&gen.type_chart)) {
        return Some(r);
    }

    if mv.name == gc::ENDEAVOR_MOVE {
        let diff = defending_pkmn.cur_stats.hp - attacking_pkmn.cur_stats.hp;
        if diff <= 0 {
            return None;
        }
        return Some(DamageRange::single(diff));
    }

    let (mut move_type, mut base_power): (String, Option<i64>) = if mv.name == consts::HIDDEN_POWER_MOVE_NAME {
        (
            stats::hidden_power_type_gen345(&attacking_pkmn.dvs).to_string(),
            Some(stats::hidden_power_base_power_gen345(&attacking_pkmn.dvs)),
        )
    } else {
        (mv.move_type.clone(), mv.base_power)
    };

    let mut attacking_first = a1.clone();
    let mut attacking_second = a2.clone();
    if attacking_ability == gc::MULTITYPE {
        if let Some(t) = gc::plate_type(attacking_pkmn.held_item.as_deref()) {
            attacking_first = t.to_string();
            attacking_second = t.to_string();
        }
    }

    let is_no_type_effects = [consts::STRUGGLE_MOVE_NAME, consts::FUTURE_SIGHT_MOVE_NAME, consts::DOOM_DESIRE_MOVE_NAME].contains(&mv.name.as_str());

    if mv.name == gc::TRIPLE_KICK_MOVE && !custom.starts_with("__tk_hit_") {
        let num_kicks = py_int(custom).unwrap_or(1).clamp(1, 3);
        let mut combined: Option<DamageRange> = None;
        for kick_num in 1..=num_kicks {
            let sentinel = format!("__tk_hit_{}__", kick_num * 10);
            let sub = DamageArgs {
                attacking: attacking_pkmn,
                mv,
                defending: defending_pkmn,
                attacking_stages: Some(&attacking_stages),
                defending_stages: Some(&defending_stages),
                attacking_field: Some(attacking_field),
                defending_field: Some(defending_field),
                is_crit: a.is_crit,
                custom_move_data: &sentinel,
                weather,
                is_double_battle: a.is_double_battle,
                attacking_battle_stats: None,
                defending_battle_stats: None,
                attacker_is_enemy: false,
            };
            if let Some(kick) = calculate_damage(gen, &sub) {
                combined = Some(match combined {
                    None => kick,
                    Some(c) => c.add(&kick),
                });
            }
        }
        return combined;
    }

    if mv.name == gc::TRIPLE_KICK_MOVE {
        // "__tk_hit_10__"
        let inner = &custom["__tk_hit_".len()..custom.len() - 2];
        base_power = py_int(inner);
    } else if mv.name == gc::NATURE_POWER_MOVE {
        let (bp, ty, _) = gc::gen4_nature_power(custom)?;
        base_power = Some(bp);
        move_type = ty.to_string();
    } else if mv.name == consts::WEATHER_BALL_MOVE_NAME && is_weather_active {
        base_power = base_power.map(|b| b * 2);
        move_type = damage::get_weather_ball_type(weather, is_weather_active, &move_type).to_string();
    } else if mv.name == consts::NATURAL_GIFT_MOVE_NAME {
        if attacking_ability == gc::KLUTZ {
            return None;
        }
        let (bp, ty) = gc::natural_gift(xpr_data::Gen::Four, attacking_pkmn.held_item.as_deref())?;
        base_power = Some(bp);
        move_type = ty.to_string();
    } else if mv.name == gc::FLING_MOVE {
        if attacking_ability == gc::KLUTZ || attacking_pkmn.held_item_str().is_empty() {
            return None;
        }
        base_power = Some(*gc::GEN4_FLING_POWER.get(attacking_pkmn.held_item_str())?);
    } else if mv.name == gc::PRESENT_MOVE {
        if custom == "Heal" {
            return None;
        }
        base_power = Some(py_int(custom).unwrap_or(40));
    }

    // Python does arithmetic on base_power here even when it is None
    // (TypeError); those moves always have a power in the data.
    if gc::GEN4_RECKLESS_MOVES.contains(&mv.name.as_str()) && attacking_ability == gc::RECKLESS {
        base_power = base_power.map(|b| floor_mul_ratio(b, 6, 5));
    }
    if attacking_ability == gc::TECHNICIAN && mv.name != consts::STRUGGLE_MOVE_NAME && base_power.map(|b| b <= 60).unwrap_or(false) {
        base_power = base_power.map(|b| floor_mul_ratio(b, 3, 2));
    }
    if gc::GEN4_PUNCH_MOVES.contains(&mv.name.as_str()) && attacking_ability == gc::IRON_FIST {
        base_power = base_power.map(|b| floor_mul_ratio(b, 6, 5));
    }
    if attacking_pkmn.name == gc::PIKACHU && attacking_pkmn.held_item_str() == gc::LIGHT_BALL {
        base_power = base_power.map(|b| b * 2);
    }

    let mut base_power = match base_power {
        None | Some(0) => return None,
        Some(bp) => bp,
    };

    if attacking_ability == gc::NORMALIZE {
        move_type = consts::TYPE_NORMAL.to_string();
    }
    if mv.name == gc::JUDGMENT_MOVE {
        if let Some(t) = gc::plate_type(attacking_pkmn.held_item.as_deref()) {
            move_type = t.to_string();
        }
    }
    if mv.name == gc::PUNISHMENT_MOVE {
        let stages = [
            defending_stages.attack_stage,
            defending_stages.defense_stage,
            defending_stages.special_attack_stage,
            defending_stages.special_defense_stage,
            defending_stages.speed_stage,
            defending_stages.accuracy_stage,
            defending_stages.evasion_stage,
        ];
        let num_buffs: i64 = stages.iter().filter(|s| **s > 0).sum();
        base_power = (60 + num_buffs * 20).min(200);
    }

    let is_scrappy_active = (d1 == consts::TYPE_GHOST || d2 == consts::TYPE_GHOST)
        && (move_type == consts::TYPE_NORMAL || move_type == consts::TYPE_FIGHTING)
        && attacking_ability == gc::SCRAPPY;
    let ignore_ground_immunity = (d1 == consts::TYPE_FLYING || d2 == consts::TYPE_FLYING)
        && move_type == consts::TYPE_GROUND
        && (defending_field.gravity || defending_field.roost);
    let ignore_dark_immunity = (d1 == consts::TYPE_DARK || d2 == consts::TYPE_DARK)
        && move_type == consts::TYPE_PSYCHIC
        && defending_field.miracle_eye;

    if !is_no_type_effects {
        let immune = gen.effectiveness(&move_type, &d1) == Some(consts::IMMUNE) || gen.effectiveness(&move_type, &d2) == Some(consts::IMMUNE);
        if immune && !is_scrappy_active && !ignore_ground_immunity && !ignore_dark_immunity {
            return None;
        } else if defending_ability == gc::LEVITATE && move_type == consts::TYPE_GROUND && !ignore_ground_immunity {
            return None;
        } else if defending_ability == gc::DAMP && (mv.name == consts::SELFDESTRUCT_MOVE_NAME || mv.name == consts::EXPLOSION_MOVE_NAME) {
            return None;
        } else if (defending_ability == gc::VOLT_ABSORB || defending_ability == gc::MOTOR_DRIVE) && move_type == consts::TYPE_ELECTRIC {
            return None;
        } else if defending_ability == gc::WATER_ABSORB && move_type == consts::TYPE_WATER {
            return None;
        } else if defending_ability == gc::FLASH_FIRE && move_type == consts::TYPE_FIRE {
            return None;
        } else if defending_ability == gc::DRY_SKIN && move_type == consts::TYPE_WATER {
            return None;
        } else if defending_ability == gc::SOUNDPROOF && gc::GEN4_SOUND_MOVES.contains(&mv.name.as_str()) {
            return None;
        } else if defending_ability == gc::WONDER_GUARD {
            if wonder_guard_blocks(gen, &move_type, &d1, &d2) {
                return None;
            }
        } else if defending_field.magnet_rise && move_type == consts::TYPE_GROUND {
            return None;
        }
    }

    if mv.name == gc::SUPER_FANG_MOVE {
        return Some(DamageRange::single(floor_div(defending_pkmn.cur_stats.hp, 2).max(1)));
    }

    if mv.has_flavor(consts::FLAVOR_FIXED_DAMAGE) {
        return Some(DamageRange::single(base_power));
    } else if mv.has_flavor(consts::FLAVOR_LEVEL_DAMAGE) {
        return Some(DamageRange::single(attacking_pkmn.level));
    } else if mv.has_flavor(consts::FLAVOR_PSYWAVE) {
        let pairs: Vec<(i64, i64)> = (0..11).map(|r| (floor_div(attacking_pkmn.level * (r + 5), 10).max(1), 1)).collect();
        return DamageRange::from_pairs(&pairs, 1);
    }

    let original_attacking_stages = attacking_stages;
    let original_defending_stages = defending_stages;

    if a.is_crit {
        if mv.category == consts::CATEGORY_PHYSICAL {
            if attacking_stages.attack_stage < 0 {
                attacking_stages = attacking_stages.apply_stat_mod(&[(consts::ATK.to_string(), -attacking_stages.attack_stage)]);
            }
            if defending_stages.defense_stage > 0 {
                defending_stages = defending_stages.apply_stat_mod(&[(consts::DEF.to_string(), -defending_stages.defense_stage)]);
            }
        } else {
            if attacking_stages.special_attack_stage < 0 {
                attacking_stages = attacking_stages.apply_stat_mod(&[(consts::SPA.to_string(), -attacking_stages.special_attack_stage)]);
            }
            if defending_stages.special_defense_stage > 0 {
                defending_stages = defending_stages.apply_stat_mod(&[(consts::SPD.to_string(), -defending_stages.special_defense_stage)]);
            }
        }
    }

    let mut atk = match a.attacking_battle_stats {
        Some(s) => *s,
        None => attacking_pkmn.get_battle_stats(&attacking_stages, false, Some(attacking_field)),
    };
    let mut def = match a.defending_battle_stats {
        Some(s) => *s,
        None => defending_pkmn.get_battle_stats(&defending_stages, false, Some(defending_field)),
    };

    if gc::OHKO_MOVE_NAMES.contains(&mv.name.as_str()) {
        if defending_ability == gc::STURDY {
            return None;
        }
        if atk.speed < def.speed {
            return None;
        }
        return Some(DamageRange::single(defending_pkmn.cur_stats.hp));
    }

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
        if custom.contains(gc::DIG_BONUS) {
            base_power *= 2;
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
    } else if mv.name == gc::RETURN_MOVE || mv.name == gc::FRUSTRATION_MOVE {
        if let Some(v) = py_int(custom) {
            base_power = v;
        }
    } else if mv.name == gc::ERUPTION_MOVE || mv.name == gc::WATER_SPOUT_MOVE {
        if let Some(v) = py_int(custom) {
            base_power = floor_div(base_power * v, 100);
        }
    } else if mv.name == gc::CRUSH_GRIP_MOVE || mv.name == gc::WRING_OUT_MOVE {
        if let Some(v) = py_int(custom) {
            base_power = 1 + floor_div(120 * v, 100);
        }
    } else if mv.name == gc::GYRO_BALL_MOVE {
        base_power = (1 + floor_div(25 * def.speed, atk.speed.max(1))).min(150);
    } else if mv.name == gc::TRUMP_CARD_MOVE {
        base_power = match custom {
            "4+" => 40,
            "3" => 50,
            "2" => 60,
            "1" => 80,
            "0" => 200,
            _ => 40,
        };
    } else if mv.name == gc::LOW_KICK_MOVE || mv.name == gc::GRASS_KNOT_MOVE {
        match defending_species.weight {
            None => base_power = 20,
            Some(w) => {
                let weight_hg = (w * 10.0).round_ties_even() as i64;
                base_power = if weight_hg <= 100 {
                    20
                } else if weight_hg <= 250 {
                    40
                } else if weight_hg <= 500 {
                    60
                } else if weight_hg <= 1000 {
                    80
                } else if weight_hg <= 2000 {
                    100
                } else {
                    120
                };
            }
        }
    } else if mv.name == gc::ROLLOUT_MOVE || mv.name == gc::ICE_BALL_MOVE {
        let (num_turns, curl) = if custom.contains("DefenseCurl") {
            (5, true)
        } else {
            (py_int(custom).unwrap_or(1), false)
        };
        let num_turns = num_turns.clamp(1, 5);
        base_power = 30 * (1i64 << (num_turns - 1));
        if curl {
            base_power *= 2;
        }
    } else if mv.name == gc::FURY_CUTTER_MOVE {
        let num_turns = py_int(custom).map(|t| t.min(5)).unwrap_or(1);
        base_power = 10 * pow2(num_turns - 1);
    } else if mv.name == gc::SPIT_UP_MOVE {
        base_power = 100 * py_int(custom).unwrap_or(1);
    }

    let bonus_moves = [
        gc::GUST_MOVE,
        gc::TWISTER_MOVE,
        gc::SURF_MOVE,
        gc::WHIRLPOOL_MOVE,
        gc::EARTHQUAKE_MOVE,
        gc::PURSUIT_MOVE,
        gc::STOMP_MOVE,
        gc::FACADE_MOVE,
        gc::SMELLING_SALT_MOVE,
        gc::REVENGE_MOVE,
        gc::ASSURANCE_MOVE,
        gc::AVALANCHE_MOVE,
        gc::BRINE_MOVE,
        gc::PAYBACK_MOVE,
        gc::WAKE_UP_SLAP_MOVE,
    ];
    if bonus_moves.contains(&mv.name.as_str()) && !custom.is_empty() && !custom.contains(gc::NO_BONUS) {
        base_power *= 2;
    }

    let held = attacking_pkmn.held_item_str();
    if (attacking_pkmn.name == gc::GEN2_MAROWAK || attacking_pkmn.name == gc::GEN2_CUBONE) && held == gc::THICK_CLUB {
        atk.attack *= 2;
    } else if attacking_pkmn.name == gc::PIKACHU && held == gc::LIGHT_BALL {
        // handled via base power
    } else if attacking_pkmn.name == gc::CLAMPERL && held == gc::DEEP_SEA_TOOTH {
        atk.special_attack *= 2;
    } else if attacking_pkmn.name == gc::CLAMPERL && held == gc::DEEP_SEA_SCALE {
        atk.special_defense *= 2;
    } else if (attacking_pkmn.name == gc::LATIOS || attacking_pkmn.name == gc::LATIAS) && held == gc::SOUL_DEW {
        atk.special_attack = floor_mul_ratio(atk.special_attack, 3, 2);
        atk.special_defense = floor_mul_ratio(atk.special_defense, 3, 2);
    }

    let dheld = defending_pkmn.held_item_str();
    if (defending_pkmn.name == gc::LATIOS || defending_pkmn.name == gc::LATIAS) && dheld == gc::SOUL_DEW {
        def.special_attack = floor_mul_ratio(def.special_attack, 3, 2);
        def.special_defense = floor_mul_ratio(def.special_defense, 3, 2);
    } else if defending_pkmn.name == gc::CLAMPERL && dheld == gc::DEEP_SEA_SCALE {
        def.special_defense *= 2;
    } else if defending_pkmn.name == gc::DITTO && dheld == gc::METAL_POWDER {
        def.defense *= 2;
    }

    if attacking_ability == gc::HUSTLE {
        atk.attack = floor_mul_ratio(atk.attack, 3, 2);
    }
    if attacking_ability == gc::HUGE_POWER || attacking_ability == gc::PURE_POWER {
        atk.attack *= 2;
    }
    if held == gc::CHOICE_BAND && attacking_ability != gc::KLUTZ {
        atk.attack = floor_mul_ratio(atk.attack, 3, 2);
    }
    if held == gc::CHOICE_SPECS && attacking_ability != gc::KLUTZ {
        atk.special_attack = floor_mul_ratio(atk.special_attack, 3, 2);
    }
    if defending_ability == gc::THICK_FAT && (move_type == consts::TYPE_FIRE || move_type == consts::TYPE_ICE) {
        atk.special_attack = floor_div(atk.special_attack, 2);
        atk.attack = floor_div(atk.attack, 2);
    }
    if defending_ability == gc::HEATPROOF && move_type == consts::TYPE_FIRE {
        atk.special_attack = floor_div(atk.special_attack, 2);
        atk.attack = floor_div(atk.attack, 2);
    }
    if is_weather_active && weather == consts::WEATHER_SUN {
        if attacking_ability == gc::FLOWER_GIFT {
            atk.attack = floor_mul_ratio(atk.attack, 3, 2);
        }
        if defending_ability == gc::FLOWER_GIFT {
            def.special_defense = floor_mul_ratio(def.special_defense, 3, 2);
        }
        if attacking_ability == gc::SOLAR_POWER {
            atk.special_attack = floor_mul_ratio(atk.special_attack, 3, 2);
        }
    }
    if is_weather_active && weather == consts::WEATHER_SANDSTORM && (d1 == consts::TYPE_ROCK || d2 == consts::TYPE_ROCK) {
        def.special_defense = floor_mul_ratio(def.special_defense, 3, 2);
    }

    let (mut attacking_stat, mut defending_stat, screen_active) = if mv.category == consts::CATEGORY_SPECIAL {
        (
            atk.special_attack,
            def.special_defense,
            defending_field.light_screen && !a.is_crit && mv.name != gc::BRICK_BREAK_MOVE,
        )
    } else {
        (atk.attack, def.defense, defending_field.reflect && !a.is_crit && mv.name != gc::BRICK_BREAK_MOVE)
    };

    if mv.name == consts::EXPLOSION_MOVE_NAME || mv.name == consts::SELFDESTRUCT_MOVE_NAME {
        defending_stat = floor_div(defending_stat, 2).max(1);
    }

    let mut is_stab = attacking_first == move_type || attacking_second == move_type;
    if mv.name == consts::FUTURE_SIGHT_MOVE_NAME {
        is_stab = false;
    }

    if gen.held_item_boosts.get(held).map(|s| s.as_str()) == Some(move_type.as_str()) && attacking_ability != gc::KLUTZ {
        attacking_stat = floor_mul_ratio(attacking_stat, 6, 5);
    } else if attacking_pkmn.name == gc::DIALGA && held == gc::ADAMANT_ORB && (move_type == consts::TYPE_DRAGON || move_type == consts::TYPE_STEEL) {
        attacking_stat = floor_mul_ratio(attacking_stat, 6, 5);
    } else if attacking_pkmn.name == gc::PALKIA && held == gc::LUSTROUS_ORB && (move_type == consts::TYPE_DRAGON || move_type == consts::TYPE_WATER) {
        attacking_stat = floor_mul_ratio(attacking_stat, 6, 5);
    } else if attacking_pkmn.name == gc::GIRATINA && held == gc::GRISEOUS_ORB && (move_type == consts::TYPE_DRAGON || move_type == consts::TYPE_GHOST) {
        attacking_stat = floor_mul_ratio(attacking_stat, 6, 5);
    }
    if held == gc::MUSCLE_BAND && mv.category == consts::CATEGORY_PHYSICAL {
        attacking_stat = floor_mul_ratio(attacking_stat, 11, 10);
    } else if held == gc::WISE_GLASSES && mv.category == consts::CATEGORY_SPECIAL {
        attacking_stat = floor_mul_ratio(attacking_stat, 11, 10);
    }

    let mut temp = 2 * attacking_pkmn.level;
    temp = floor_div(temp, 5) + 2;
    temp *= base_power;
    temp *= attacking_stat;
    if defending_stat == 0 {
        return None;
    }
    temp = floor_div(temp, defending_stat);
    temp = floor_div(temp, 50);

    if screen_active {
        if a.is_double_battle {
            temp = floor_div(temp * 2, 3).max(1);
        } else {
            temp = floor_div(temp, 2);
        }
    }
    if a.is_double_battle && (mv.targeting == gc::GEN4_TARGETING_ALL_FOES || mv.targeting == gc::GEN4_TARGETING_OTHERS) {
        temp = floor_div(temp * 3, 4);
    }

    let mut weather_boost = false;
    let mut weather_penalty = false;
    if is_weather_active {
        if weather == consts::WEATHER_RAIN {
            weather_boost = move_type == consts::TYPE_WATER;
            weather_penalty = move_type == consts::TYPE_FIRE || mv.name == consts::SOLAR_BEAM_MOVE_NAME;
        } else if weather == consts::WEATHER_SUN {
            weather_boost = move_type == consts::TYPE_FIRE;
            weather_penalty = move_type == consts::TYPE_WATER;
        } else if weather != consts::WEATHER_NONE {
            weather_penalty = mv.name == consts::SOLAR_BEAM_MOVE_NAME;
        }
    }
    if weather_boost {
        temp = floor_mul_ratio(temp, 3, 2);
    } else if weather_penalty {
        temp = floor_div(temp, 2);
    }

    temp += 2;

    if a.is_crit
        && mv.name != consts::DOOM_DESIRE_MOVE_NAME
        && mv.name != consts::FUTURE_SIGHT_MOVE_NAME
        && defending_ability != gc::BATTLE_ARMOR
        && defending_ability != gc::SHELL_ARMOR
    {
        if attacking_ability == gc::SNIPER {
            temp *= 3;
        } else {
            temp *= 2;
        }
    }

    if held == gc::LIFE_ORB && attacking_ability != gc::KLUTZ {
        temp = floor_mul_ratio(temp, 13, 10);
    }

    if is_stab && !is_no_type_effects {
        if attacking_ability == gc::ADAPTABILITY {
            temp *= 2;
        } else {
            temp = floor_mul_ratio(temp, 3, 2);
        }
    }

    // net effectiveness as a rational power of two (x2 per SE, /2 per NVE)
    let mut net_exp: i64 = 0;
    if !is_no_type_effects {
        if let Some(row) = gen.type_chart.get(&move_type) {
            for (test_type, eff) in row {
                if *test_type == d1 || *test_type == d2 {
                    if eff == consts::SUPER_EFFECTIVE {
                        temp *= 2;
                        net_exp += 1;
                    } else if eff == consts::NOT_VERY_EFFECTIVE {
                        temp = floor_div(temp, 2);
                        net_exp -= 1;
                    }
                }
            }
        }
        if net_exp > 0 {
            if defending_ability == gc::FILTER || defending_ability == gc::SOLID_ROCK {
                temp = floor_div(temp * 3, 4).max(1);
            } else if held == gc::EXPERT_BELT {
                temp = floor_mul_ratio(temp, 6, 5);
            }
        } else if net_exp < 0 && attacking_ability == gc::TINTED_LENS {
            temp *= 2;
        }
    }

    if defending_ability == gc::DRY_SKIN && move_type == consts::TYPE_FIRE {
        temp = floor_mul_ratio(temp, 5, 4);
    }

    if temp <= 0 {
        temp = 1;
    }

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

    if mv.name == consts::SPIT_UP_MOVE_NAME {
        return Some(DamageRange::single(temp));
    }
    let mut result = DamageRange::from_rolls(temp, MIN_RANGE, MAX_RANGE);
    if multi_hit > 1 {
        let other = if a.is_crit {
            let sub = DamageArgs {
                attacking: attacking_pkmn,
                mv,
                defending: defending_pkmn,
                attacking_stages: Some(&original_attacking_stages),
                defending_stages: Some(&original_defending_stages),
                attacking_field: Some(attacking_field),
                defending_field: Some(defending_field),
                is_crit: false,
                custom_move_data: "",
                weather,
                is_double_battle: a.is_double_battle,
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
    Some(result)
}

fn pow2(exp: i64) -> i64 {
    if exp < 0 {
        0
    } else {
        1i64 << exp.min(40)
    }
}
