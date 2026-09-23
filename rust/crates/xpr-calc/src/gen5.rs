//! Port of `pkmn/gen_5/pkmn_damage_calc.py`.

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
pub fn get_crit_rate(mon: &EnemyPkmn, mv: &Move, custom: Option<&str>) -> f64 {
    if gc::GEN5_ALWAYS_CRIT_MOVES.contains(&mv.name.as_str()) {
        return 1.0;
    }
    let mut stage = 0;
    if mv.has_flavor(consts::FLAVOR_HIGH_CRIT) {
        stage += 1;
    }
    if mv.name == gc::NATURE_POWER_MOVE && custom == Some(gc::LONG_GRASS_TERRAIN) {
        stage += 1;
    }
    if mon.ability == gc::SUPER_LUCK {
        stage += 1;
    }
    crit_stage_rate(stage)
}

/// `get_move_accuracy`
pub fn get_move_accuracy(pkmn: &EnemyPkmn, mv: &Move, custom: &str, defending: &EnemyPkmn, weather: &str) -> Option<f64> {
    if pkmn.ability == gc::NO_GUARD || defending.ability == gc::NO_GUARD {
        return None;
    }
    if gc::OHKO_MOVE_NAMES.contains(&mv.name.as_str()) {
        // OHKO moves skip the usual accuracy modifiers: base accuracy plus the
        // level difference, out of 100; fails outright against a higher level.
        if pkmn.level < defending.level {
            return Some(0.0);
        }
        return Some((mv.accuracy.unwrap_or(0) + (pkmn.level - defending.level)).min(100) as f64);
    }
    let mut result: Option<i64> = mv.accuracy;
    if mv.name == gc::BLIZZARD_MOVE && weather == consts::WEATHER_HAIL {
        return None;
    }
    if mv.name == gc::THUNDER_MOVE && weather == consts::WEATHER_RAIN {
        return None;
    }
    if mv.name == gc::THUNDER_MOVE && weather == consts::WEATHER_SUN {
        result = Some(50);
    }
    if mv.name == gc::HURRICANE_MOVE {
        if weather == consts::WEATHER_RAIN {
            return None;
        } else if weather == consts::WEATHER_SUN {
            result = Some(50);
        }
    }
    let mut r = result?;
    if pkmn.ability == gc::COMPOUND_EYES {
        r = floor_mul_ratio(r, 13, 10).min(100);
    } else if pkmn.ability == gc::HUSTLE {
        let mut is_physical = mv.category == consts::CATEGORY_PHYSICAL;
        if mv.name == gc::NATURE_POWER_MOVE {
            is_physical = [gc::SAND_TERRAIN, gc::CAVE_TERRAIN, gc::TALL_GRASS_TERRAIN].contains(&custom);
        }
        if is_physical {
            r = floor_div(r * 3277, 4096);
        }
    }
    if defending.ability == gc::SAND_VEIL && weather == consts::WEATHER_SANDSTORM {
        r = floor_div(r * 3277, 4096);
    }
    if defending.ability == gc::SNOW_CLOAK && weather == consts::WEATHER_HAIL {
        r = floor_div(r * 3277, 4096);
    }
    if pkmn.held_item_str() == gc::WIDE_LENS && pkmn.ability != gc::KLUTZ {
        r = floor_div(r * 4506, 4096).min(100);
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

/// `calculate_gen_five_damage`
pub fn calculate_damage(gen: &GenData, a: &DamageArgs) -> Option<DamageRange> {
    let mv = a.mv;
    let attacking_pkmn = a.attacking;
    let defending_pkmn = a.defending;
    let custom = a.custom_move_data;
    let weather = a.weather;

    let default_field = FieldStatus::default();
    let attacking_field = a.attacking_field.unwrap_or(&default_field);
    let defending_field = a.defending_field.unwrap_or(&default_field);
    let default_stages = StageModifiers::default();
    let mut attacking_stages = *a.attacking_stages.unwrap_or(&default_stages);
    let mut defending_stages = *a.defending_stages.unwrap_or(&default_stages);

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

    let attacking_species = gen.pkmn_db().get_pkmn(&attacking_pkmn.name)?;
    let defending_species = gen.pkmn_db().get_pkmn(&defending_pkmn.name)?;

    let is_weather_active = damage::is_weather_active(attacking_ability, defending_ability, weather);
    let (a1, a2) = damage::apply_forecast(&attacking_species.first_type, &attacking_species.second_type, attacking_ability, weather, is_weather_active);
    let (d1, d2) = damage::apply_forecast(&defending_species.first_type, &defending_species.second_type, defending_ability, weather, is_weather_active);

    if let Some(r) = damage::get_special_damage_override(mv, attacking_pkmn, Some((&d1, &d2)), Some(&gen.type_chart)) {
        return Some(r);
    }

    let is_struggle = mv.name == consts::STRUGGLE_MOVE_NAME;

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

    if mv.name == gc::NATURE_POWER_MOVE {
        match custom {
            gc::PLAIN_TERRAIN => {
                base_power = Some(60);
                move_type = consts::TYPE_NORMAL.to_string();
            }
            gc::SAND_TERRAIN => {
                base_power = Some(100);
                move_type = consts::TYPE_GROUND.to_string();
            }
            gc::CAVE_TERRAIN => {
                base_power = Some(80);
                move_type = consts::TYPE_GHOST.to_string();
            }
            gc::ROCK_TERRAIN => {
                base_power = Some(75);
                move_type = consts::TYPE_ROCK.to_string();
            }
            gc::TALL_GRASS_TERRAIN => return None,
            gc::LONG_GRASS_TERRAIN => {
                base_power = Some(55);
                move_type = consts::TYPE_GRASS.to_string();
            }
            gc::POND_WATER_TERRAIN => {
                base_power = Some(65);
                move_type = consts::TYPE_WATER.to_string();
            }
            gc::SEA_WATER_TERRAIN => {
                base_power = Some(95);
                move_type = consts::TYPE_WATER.to_string();
            }
            gc::UNDERWATER_TERRAIN => {
                base_power = Some(120);
                move_type = consts::TYPE_WATER.to_string();
            }
            _ => {}
        }
    } else if mv.name == consts::WEATHER_BALL_MOVE_NAME && is_weather_active {
        base_power = base_power.map(|b| b * 2);
        move_type = damage::get_weather_ball_type(weather, is_weather_active, &move_type).to_string();
    } else if mv.name == consts::NATURAL_GIFT_MOVE_NAME {
        if attacking_ability == gc::KLUTZ {
            return None;
        }
        let (bp, ty) = gc::natural_gift(xpr_data::Gen::Five, attacking_pkmn.held_item.as_deref())?;
        base_power = Some(bp);
        move_type = ty.to_string();
    }

    // `base_power <= 60` with None raises TypeError in Python; every move
    // that reaches here with a null power has an override below, so the
    // Python code only ever compares real numbers.
    if attacking_ability == gc::TECHNICIAN && base_power.map(|b| b <= 60).unwrap_or(false) {
        base_power = Some(90);
    }

    if attacking_ability == gc::NORMALIZE {
        move_type = consts::TYPE_NORMAL.to_string();
    }
    if mv.name == gc::JUDGMENT_MOVE {
        if let Some(t) = gc::plate_type(attacking_pkmn.held_item.as_deref()) {
            move_type = t.to_string();
        }
    }
    if mv.name == gc::PUNISHMENT_MOVE {
        let total: i64 = [
            defending_stages.attack_stage,
            defending_stages.defense_stage,
            defending_stages.special_attack_stage,
            defending_stages.special_defense_stage,
            defending_stages.speed_stage,
            defending_stages.accuracy_stage,
            defending_stages.evasion_stage,
        ]
        .iter()
        .map(|s| (*s).max(0))
        .sum();
        base_power = Some((60 + 20 * total).min(200));
    }

    let is_scrappy_active = (d1 == consts::TYPE_GHOST || d2 == consts::TYPE_GHOST)
        && (mv.move_type == consts::TYPE_NORMAL || mv.move_type == consts::TYPE_FIGHTING)
        && attacking_ability == gc::SCRAPPY;
    let ignore_ground_immunity = (d1 == consts::TYPE_FLYING || d2 == consts::TYPE_FLYING)
        && mv.move_type == consts::TYPE_GROUND
        && (defending_field.gravity || defending_field.roost);
    let ignore_dark_immunity = (d1 == consts::TYPE_DARK || d2 == consts::TYPE_DARK)
        && mv.move_type == consts::TYPE_PSYCHIC
        && defending_field.miracle_eye;

    if is_struggle {
        // typeless
    } else {
        let immune = gen.effectiveness(&move_type, &d1) == Some(consts::IMMUNE) || gen.effectiveness(&move_type, &d2) == Some(consts::IMMUNE);
        if immune && !is_scrappy_active && !ignore_ground_immunity && !ignore_dark_immunity {
            return None;
        } else if defending_ability == gc::LEVITATE && move_type == consts::TYPE_GROUND && !ignore_ground_immunity {
            return None;
        } else if defending_ability == gc::DAMP
            && (mv.name == consts::SELFDESTRUCT_MOVE_NAME || mv.name == gc::SELFDESTRUCT_MOVE_GEN5 || mv.name == consts::EXPLOSION_MOVE_NAME)
        {
            return None;
        } else if (defending_ability == gc::VOLT_ABSORB || defending_ability == gc::LIGHTNING_ROD || defending_ability == gc::MOTOR_DRIVE)
            && move_type == consts::TYPE_ELECTRIC
        {
            return None;
        } else if defending_ability == gc::WATER_ABSORB && move_type == consts::TYPE_WATER {
            return None;
        } else if defending_ability == gc::FLASH_FIRE && move_type == consts::TYPE_FIRE {
            return None;
        } else if defending_ability == gc::DRY_SKIN && move_type == consts::TYPE_WATER {
            return None;
        } else if defending_ability == gc::WONDER_GUARD {
            if wonder_guard_blocks(gen, &move_type, &d1, &d2) {
                return None;
            }
        } else if defending_field.magnet_rise && mv.move_type == consts::TYPE_GROUND {
            return None;
        }
    }

    if mv.has_flavor(consts::FLAVOR_FIXED_DAMAGE) {
        return Some(DamageRange::single(base_power?));
    } else if mv.has_flavor(consts::FLAVOR_LEVEL_DAMAGE) {
        return Some(DamageRange::single(attacking_pkmn.level));
    } else if mv.has_flavor(consts::FLAVOR_PSYWAVE) {
        let pairs: Vec<(i64, i64)> = (0..=100).map(|r| (floor_div(attacking_pkmn.level * (r + 50), 100).max(1), 1)).collect();
        return DamageRange::from_pairs(&pairs, 1);
    }

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

    let mut base_power_opt = base_power;
    if mv.name == gc::MAGNITUDE_MOVE {
        if custom.contains(gc::MAGNITUDE_4) {
            base_power_opt = Some(10);
        } else if custom.contains(gc::MAGNITUDE_5) {
            base_power_opt = Some(30);
        } else if custom.contains(gc::MAGNITUDE_6) {
            base_power_opt = Some(50);
        } else if custom.contains(gc::MAGNITUDE_7) {
            base_power_opt = Some(70);
        } else if custom.contains(gc::MAGNITUDE_8) {
            base_power_opt = Some(90);
        } else if custom.contains(gc::MAGNITUDE_9) {
            base_power_opt = Some(110);
        } else if custom.contains(gc::MAGNITUDE_10) {
            base_power_opt = Some(150);
        }
    } else if mv.name == consts::FLAIL_MOVE_NAME || mv.name == consts::REVERSAL_MOVE_NAME {
        if custom.contains(gc::FLAIL_FULL_HP) {
            base_power_opt = Some(20);
        } else if custom.contains(gc::FLAIL_HALF_HP) {
            base_power_opt = Some(40);
        } else if custom.contains(gc::FLAIL_QUARTER_HP) {
            base_power_opt = Some(80);
        } else if custom.contains(gc::FLAIL_TEN_PERCENT_HP) {
            base_power_opt = Some(100);
        } else if custom.contains(gc::FLAIL_FIVE_PERCENT_HP) {
            base_power_opt = Some(150);
        } else if custom.contains(gc::FLAIL_MIN_HP) {
            base_power_opt = Some(200);
        }
    } else if mv.name == gc::RETURN_MOVE {
        if let Some(v) = py_int(custom) {
            base_power_opt = Some(v);
        }
    } else if mv.name == gc::ERUPTION_MOVE || mv.name == gc::WATER_SPOUT_MOVE {
        if let (Some(v), Some(bp)) = (py_int(custom), base_power_opt) {
            base_power_opt = Some(floor_div(bp * v, 100));
        }
    } else if mv.name == gc::CRUSH_GRIP_MOVE || mv.name == gc::WRING_OUT_MOVE {
        if let Some(v) = py_int(custom) {
            base_power_opt = Some(1 + floor_div(120 * v, 100));
        }
    } else if mv.name == gc::GYRO_BALL_MOVE {
        if atk.speed == 0 {
            return None;
        }
        base_power_opt = Some((1 + floor_div(25 * def.speed, atk.speed)).min(150));
    } else if mv.name == gc::TRUMP_CARD_MOVE {
        match custom {
            "4+" => base_power_opt = Some(40),
            "3" => base_power_opt = Some(50),
            "2" => base_power_opt = Some(60),
            "1" => base_power_opt = Some(80),
            "0" => base_power_opt = Some(200),
            _ => {}
        }
    } else if [gc::LOW_KICK_MOVE, gc::GRASS_KNOT_MOVE, gc::HEAVY_SLAM_MOVE, gc::HEAT_CRASH_MOVE].contains(&mv.name.as_str()) {
        let is_ratio = mv.name == gc::HEAVY_SLAM_MOVE || mv.name == gc::HEAT_CRASH_MOVE;
        match (defending_species.weight, attacking_species.weight) {
            (Some(dw), aw) if !(is_ratio && aw.is_none()) => {
                if is_ratio {
                    let ratio = aw.unwrap() / dw;
                    base_power_opt = Some(if ratio >= 5.0 {
                        120
                    } else if ratio >= 4.0 {
                        100
                    } else if ratio >= 3.0 {
                        80
                    } else if ratio >= 2.0 {
                        60
                    } else {
                        40
                    });
                } else {
                    base_power_opt = Some(if dw <= 10.0 {
                        20
                    } else if dw <= 25.0 {
                        40
                    } else if dw <= 50.0 {
                        60
                    } else if dw <= 100.0 {
                        80
                    } else if dw <= 200.0 {
                        100
                    } else {
                        120
                    });
                }
            }
            _ => base_power_opt = Some(if is_ratio { 40 } else { 20 }),
        }
    } else if mv.name == gc::ELECTRO_BALL_MOVE {
        let ratio = if def.speed <= 0 { f64::INFINITY } else { (atk.speed as f64) / (def.speed as f64) };
        base_power_opt = Some(if ratio >= 4.0 {
            150
        } else if ratio >= 3.0 {
            120
        } else if ratio >= 2.0 {
            80
        } else if ratio >= 1.0 {
            60
        } else {
            40
        });
    } else if mv.name == gc::STORED_POWER_MOVE {
        let total: i64 = [
            attacking_stages.attack_stage,
            attacking_stages.defense_stage,
            attacking_stages.special_attack_stage,
            attacking_stages.special_defense_stage,
            attacking_stages.speed_stage,
            attacking_stages.accuracy_stage,
            attacking_stages.evasion_stage,
        ]
        .iter()
        .map(|s| (*s).max(0))
        .sum();
        base_power_opt = Some((20 + 20 * total).min(860));
    } else if mv.name == gc::HEX_MOVE {
        if custom == gc::STATUS_BONUS {
            base_power_opt = Some(100);
        }
    } else if mv.name == gc::VENOSHOCK_MOVE {
        if custom == gc::POISONED_BONUS {
            base_power_opt = Some(130);
        }
    } else if mv.name == gc::RETALIATE_MOVE {
        if custom == gc::ALLY_FAINTED_BONUS {
            base_power_opt = Some(140);
        }
    } else if mv.name == gc::ECHOED_VOICE_MOVE {
        let turns = py_int(custom).unwrap_or(1);
        base_power_opt = Some((40 * turns).min(200));
    } else if mv.name == gc::ACROBATICS_MOVE {
        if attacking_pkmn.held_item_str().is_empty() {
            base_power_opt = Some(110);
        }
    } else if mv.name == gc::PRESENT_MOVE {
        base_power_opt = match custom {
            "80" => Some(80),
            "120" => Some(120),
            gc::HEAL_OPTION => None,
            _ => Some(40),
        };
    } else if mv.name == gc::FRUSTRATION_MOVE {
        if let Some(v) = py_int(custom) {
            base_power_opt = Some(v);
        }
    }

    if gc::OHKO_MOVE_NAMES.contains(&mv.name.as_str()) {
        if defending_ability == gc::STURDY {
            return None;
        }
        if attacking_pkmn.level < defending_pkmn.level {
            return None;
        }
        return Some(DamageRange::single(defending_pkmn.cur_stats.hp));
    } else if mv.name == gc::SUPER_FANG_MOVE {
        return Some(DamageRange::single(floor_div(defending_pkmn.cur_stats.hp, 2).max(1)));
    } else if mv.name == gc::ENDEAVOR_MOVE {
        let pct = py_int(custom).unwrap_or(100);
        let attacker_hp = floor_div(attacking_pkmn.cur_stats.hp * pct, 100);
        let remaining = defending_pkmn.cur_stats.hp - attacker_hp;
        if remaining <= 0 {
            return None;
        }
        return Some(DamageRange::single(remaining));
    } else if mv.name == gc::FINAL_GAMBIT_MOVE {
        return Some(DamageRange::single(attacking_pkmn.cur_stats.hp));
    }

    let held = attacking_pkmn.held_item_str();
    if attacking_pkmn.name == gc::GEN2_MAROWAK && held == gc::THICK_CLUB {
        atk.attack *= 2;
    } else if attacking_pkmn.name == gc::PIKACHU && held == gc::LIGHT_BALL {
        atk.special_attack *= 2;
    } else if attacking_pkmn.name == gc::CLAMPERL && held == gc::DEEP_SEA_TOOTH {
        atk.special_attack *= 2;
    } else if attacking_pkmn.name == gc::CLAMPERL && held == gc::DEEP_SEA_SCALE {
        atk.special_defense *= 2;
    } else if (attacking_pkmn.name == gc::LATIOS || attacking_pkmn.name == gc::LATIAS) && held == gc::DEEP_SEA_SCALE {
        atk.special_attack = floor_mul_ratio(atk.special_attack, 3, 2);
        atk.special_defense = floor_mul_ratio(atk.special_defense, 3, 2);
    }

    let dheld = defending_pkmn.held_item_str();
    if (defending_pkmn.name == gc::LATIOS || defending_pkmn.name == gc::LATIAS) && dheld == gc::DEEP_SEA_SCALE {
        def.special_attack = floor_mul_ratio(def.special_attack, 3, 2);
        def.special_defense = floor_mul_ratio(def.special_defense, 3, 2);
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
    if defending_ability == gc::FLOWER_GIFT && is_weather_active && weather == consts::WEATHER_SUN {
        def.special_attack = floor_mul_ratio(def.special_attack, 3, 2);
        def.special_defense = floor_mul_ratio(def.special_attack, 3, 2);
    } else if attacking_ability == gc::FLOWER_GIFT && is_weather_active && weather == consts::WEATHER_SUN {
        atk.special_attack = floor_mul_ratio(atk.special_attack, 3, 2);
        atk.special_defense = floor_mul_ratio(atk.special_attack, 3, 2);
    } else if attacking_ability == gc::SOLAR_POWER && is_weather_active && weather == consts::WEATHER_SUN {
        atk.special_attack = floor_mul_ratio(atk.special_attack, 3, 2);
    }
    if is_weather_active && weather == consts::WEATHER_SANDSTORM && (d1 == consts::TYPE_ROCK || d2 == consts::TYPE_ROCK) {
        def.special_defense = floor_mul_ratio(def.special_defense, 3, 2);
    }

    let (mut attacking_stat, mut defending_stat, screen_active) = if mv.category == consts::CATEGORY_SPECIAL {
        let (dstat, screen_field) = if gc::GEN5_USES_TARGET_DEFENSE_MOVES.contains(&mv.name.as_str()) {
            (def.defense, defending_field.reflect)
        } else {
            (def.special_defense, defending_field.light_screen)
        };
        (atk.special_attack, dstat, screen_field && !a.is_crit && mv.name != gc::BRICK_BREAK_MOVE)
    } else {
        (atk.attack, def.defense, defending_field.reflect && !a.is_crit && mv.name != gc::BRICK_BREAK_MOVE)
    };

    if mv.name == gc::FOUL_PLAY_MOVE {
        attacking_stat = def.attack;
    }
    if gc::GEN5_IGNORES_DEFENSE_STAGES_MOVES.contains(&mv.name.as_str()) {
        let unboosted = defending_pkmn.get_battle_stats(&StageModifiers::default(), false, Some(defending_field));
        defending_stat = unboosted.defense;
    }

    let is_stab = !is_struggle && (attacking_first == move_type || attacking_second == move_type);

    if gen.held_item_boosts.get(held).map(|s| s.as_str()) == Some(move_type.as_str()) && attacking_ability != gc::KLUTZ {
        attacking_stat = floor_mul_ratio(attacking_stat, 6, 5);
    } else if attacking_pkmn.name == gc::DIALGA && held == gc::ADAMANT_ORB && (mv.move_type == consts::TYPE_DRAGON || mv.move_type == consts::TYPE_STEEL) {
        attacking_stat = floor_mul_ratio(attacking_stat, 6, 5);
    } else if attacking_pkmn.name == gc::PALKIA && held == gc::LUSTROUS_ORB && (mv.move_type == consts::TYPE_DRAGON || mv.move_type == consts::TYPE_WATER) {
        attacking_stat = floor_mul_ratio(attacking_stat, 6, 5);
    } else if attacking_pkmn.name == gc::GIRATINA && held == gc::GRISEOUS_ORB && (mv.move_type == consts::TYPE_DRAGON || mv.move_type == consts::TYPE_GHOST) {
        attacking_stat = floor_mul_ratio(attacking_stat, 6, 5);
    }

    let base_power = match base_power_opt {
        None | Some(0) => return None,
        Some(bp) => bp,
    };

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
        temp = floor_div(temp, 2);
    }
    if a.is_double_battle && (mv.targeting == consts::TARGETING_BOTH_ENEMIES || mv.targeting == "All Foes" || mv.targeting == "Others") {
        temp = floor_div(temp * 3, 4);
    }

    let is_solar_beam = mv.name == consts::SOLAR_BEAM_MOVE_NAME || mv.name == gc::SOLAR_BEAM_MOVE_GEN5;
    let mut weather_boost = false;
    let mut weather_penalty = false;
    if is_weather_active {
        if weather == consts::WEATHER_RAIN {
            weather_boost = move_type == consts::TYPE_WATER;
            weather_penalty = move_type == consts::TYPE_FIRE || is_solar_beam;
        } else if weather == consts::WEATHER_SUN {
            weather_boost = move_type == consts::TYPE_FIRE;
            weather_penalty = move_type == consts::TYPE_WATER;
        } else if weather != consts::WEATHER_NONE {
            weather_penalty = is_solar_beam;
        }
    }
    if weather_boost {
        temp = floor_mul_ratio(temp, 3, 2);
    } else if weather_penalty {
        temp = floor_div(temp, 2);
    }

    temp += 2;

    if a.is_crit && defending_ability != gc::BATTLE_ARMOR && defending_ability != gc::SHELL_ARMOR {
        if attacking_ability == gc::SNIPER {
            temp *= 3;
        } else {
            temp *= 2;
        }
    }

    let mut move_modifier: i64 = 1;
    if mv.name == gc::ROLLOUT_MOVE || mv.name == gc::ICE_BALL_MOVE {
        let (turns, curl) = if custom.contains("DefenseCurl") {
            (5, true)
        } else {
            (py_int(custom).unwrap_or(5), false)
        };
        move_modifier = pow2(turns - 1);
        if curl {
            move_modifier *= 2;
        }
    } else if mv.name == gc::FURY_CUTTER_MOVE {
        move_modifier = pow2(py_int(custom)?.min(5) - 1);
    } else if mv.name == gc::TRIPLE_KICK_MOVE {
        move_modifier = py_int(custom)?;
    } else if mv.name == gc::SPIT_UP_MOVE {
        move_modifier = py_int(custom)?;
    }
    temp *= move_modifier;

    let bonus_moves = [
        gc::GUST_MOVE,
        gc::TWISTER_MOVE,
        gc::SURF_MOVE,
        gc::WHIRLPOOL_MOVE,
        gc::EARTHQUAKE_MOVE,
        gc::PURSUIT_MOVE,
        gc::STOMP_MOVE,
        gc::STEAMROLLER_MOVE,
        gc::FACADE_MOVE,
        gc::SMELLING_SALTS_MOVE_GEN5,
        gc::REVENGE_MOVE,
        gc::ASSURANCE_MOVE,
        gc::AVALANCHE_MOVE,
        gc::BRINE_MOVE,
        gc::PAYBACK_MOVE,
        gc::WAKE_UP_SLAP_MOVE,
    ];
    let double_damage = if bonus_moves.contains(&mv.name.as_str()) {
        !custom.is_empty() && !custom.contains(gc::NO_BONUS)
    } else if mv.name == gc::MAGNITUDE_MOVE {
        custom.contains(gc::DIG_BONUS)
    } else {
        false
    };
    if double_damage {
        temp *= 2;
    }

    if is_stab {
        if attacking_ability == gc::ADAPTABILITY {
            temp *= 2;
        } else {
            temp = floor_mul_ratio(temp, 3, 2);
        }
    }

    let mut tinted_lens_active = false;
    if !is_struggle {
        if let Some(row) = gen.type_chart.get(&move_type) {
            for (test_type, eff) in row {
                if *test_type == d1 || *test_type == d2 {
                    if eff == consts::SUPER_EFFECTIVE {
                        if defending_ability == gc::FILTER || defending_ability == gc::SOLID_ROCK {
                            temp = floor_mul_ratio(temp, 3, 2);
                        } else {
                            temp *= 2;
                        }
                    } else if eff == consts::NOT_VERY_EFFECTIVE {
                        if attacking_ability == gc::TINTED_LENS {
                            tinted_lens_active = true;
                        }
                        temp = floor_div(temp, 2);
                    }
                }
            }
        }
    }
    if tinted_lens_active {
        temp *= 2;
    }

    if defending_ability == gc::DRY_SKIN && mv.move_type == consts::TYPE_FIRE {
        temp = floor_mul_ratio(temp, 13, 10);
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
            // NOTE: the gen 5 Python passes the crit-adjusted stage
            // modifiers to the non-crit hits (unlike gens 2-4)
            let sub = DamageArgs {
                attacking: attacking_pkmn,
                mv,
                defending: defending_pkmn,
                attacking_stages: Some(&attacking_stages),
                defending_stages: Some(&defending_stages),
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
