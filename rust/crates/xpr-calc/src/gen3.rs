//! Port of `pkmn/gen_3/pkmn_damage_calc.py`.

use xpr_core::consts;
use xpr_core::{floor_div, floor_mul_ratio};
use xpr_data::gen_consts as gc;
use xpr_data::model::{EnemyPkmn, Gen, Move, StageModifiers};
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
pub fn get_crit_rate(_mon: &EnemyPkmn, mv: &Move, custom: Option<&str>) -> f64 {
    let mut stage = 0;
    if mv.has_flavor(consts::FLAVOR_HIGH_CRIT) {
        stage += 1;
    }
    if mv.name == gc::NATURE_POWER_MOVE && custom.map(|c| c.contains(gc::LONG_GRASS_TERRAIN)).unwrap_or(false) {
        stage += 1;
    }
    crit_stage_rate(stage)
}

/// `get_move_accuracy`
pub fn get_move_accuracy(gen: &GenData, pkmn: &EnemyPkmn, mv: &Move, custom: &str, defending: &EnemyPkmn, weather: &str) -> Option<f64> {
    if gc::OHKO_MOVE_NAMES.contains(&mv.name.as_str()) {
        if defending.ability == gc::STURDY {
            return Some(0.0);
        }
        if pkmn.level < defending.level {
            return Some(0.0);
        }
        // hits when `Random() % 100 + 1 < accuracy + level difference`
        return Some(((mv.accuracy.unwrap_or(0) + (pkmn.level - defending.level) - 1).clamp(0, 100)) as f64);
    }
    let mut result: Option<i64> = if mv.name == gc::NATURE_POWER_MOVE {
        Some(if custom.contains(gc::TALL_GRASS_TERRAIN) {
            75
        } else if custom.contains(gc::LONG_GRASS_TERRAIN) {
            95
        } else if custom.contains(gc::UNDERWATER_TERRAIN) {
            80
        } else if custom.contains(gc::ROCK_TERRAIN) {
            90
        } else {
            100
        })
    } else {
        mv.accuracy
    };
    let weather_has_effect = damage::is_weather_active(&pkmn.ability, &defending.ability, weather);
    if mv.name == gc::THUNDER_MOVE && weather_has_effect {
        if weather == consts::WEATHER_RAIN {
            return None;
        } else if weather == consts::WEATHER_SUN {
            result = Some(50);
        }
    }
    let mut r = result?;
    if pkmn.ability == gc::COMPOUND_EYES {
        r = floor_mul_ratio(r, 13, 10);
    } else if pkmn.ability == gc::HUSTLE {
        let is_physical = if mv.name == gc::NATURE_POWER_MOVE {
            [gc::PLAIN_TERRAIN, gc::SAND_TERRAIN, gc::CAVE_TERRAIN, gc::ROCK_TERRAIN]
                .iter()
                .any(|t| custom.contains(t))
        } else if mv.name == consts::HIDDEN_POWER_MOVE_NAME {
            !gen.special_types.iter().any(|t| t == stats::hidden_power_type_gen345(&pkmn.dvs))
        } else {
            !gen.special_types.contains(&mv.move_type)
        };
        if is_physical {
            r = floor_div(r * 3277, 4096);
        }
    }
    if defending.ability == gc::SAND_VEIL && weather == consts::WEATHER_SANDSTORM && weather_has_effect {
        r = floor_div(r * 3277, 4096);
    }
    Some(r.min(100) as f64)
}

fn wonder_guard_blocks(gen: &GenData, move_type: &str, d1: &str, d2: &str, strict: bool) -> bool {
    let e1 = gen.effectiveness(move_type, d1);
    let e2 = gen.effectiveness(move_type, d2);
    if strict
        && (e1 == Some(consts::IMMUNE)
            || e1 == Some(consts::NOT_VERY_EFFECTIVE)
            || e2 == Some(consts::IMMUNE)
            || e2 == Some(consts::NOT_VERY_EFFECTIVE))
    {
        return true;
    }
    e1 != Some(consts::SUPER_EFFECTIVE) && e2 != Some(consts::SUPER_EFFECTIVE)
}

/// `calculate_gen_three_damage`
pub fn calculate_damage(gen: &GenData, a: &DamageArgs) -> Option<DamageRange> {
    let mv = a.mv;
    let attacking_pkmn = a.attacking;
    let defending_pkmn = a.defending;
    let custom = a.custom_move_data;
    let weather = a.weather;

    let attacking_species = gen.pkmn_db().get_pkmn(&attacking_pkmn.name)?;
    let defending_species = gen.pkmn_db().get_pkmn(&defending_pkmn.name)?;

    let is_weather_active = damage::is_weather_active(&attacking_pkmn.ability, &defending_pkmn.ability, weather);
    let (a1, a2) = damage::apply_forecast(&attacking_species.first_type, &attacking_species.second_type, &attacking_pkmn.ability, weather, is_weather_active);
    let (d1, d2) = damage::apply_forecast(&defending_species.first_type, &defending_species.second_type, &defending_pkmn.ability, weather, is_weather_active);

    if defending_pkmn.ability == gc::WONDER_GUARD
        && [consts::DRAGON_RAGE_MOVE_NAME, "SonicBoom", "Seismic Toss", "Night Shade"].contains(&mv.name.as_str())
        && wonder_guard_blocks(gen, &mv.move_type, &d1, &d2, false)
    {
        return None;
    }

    if let Some(r) = damage::get_special_damage_override(mv, attacking_pkmn, Some((&d1, &d2)), Some(&gen.type_chart)) {
        return Some(r);
    }

    let (mut move_type, base_power): (String, Option<i64>) = if mv.name == consts::HIDDEN_POWER_MOVE_NAME {
        (
            stats::hidden_power_type_gen345(&attacking_pkmn.dvs).to_string(),
            Some(stats::hidden_power_base_power_gen345(&attacking_pkmn.dvs)),
        )
    } else {
        (mv.move_type.clone(), mv.base_power)
    };
    let mut base_power = match base_power {
        None | Some(0) => return None,
        Some(bp) => bp,
    };

    if mv.name == consts::WEATHER_BALL_MOVE_NAME && is_weather_active {
        move_type = damage::get_weather_ball_type(weather, is_weather_active, &move_type).to_string();
    }

    let skip_type_and_immunity = [
        consts::FUTURE_SIGHT_MOVE_NAME,
        consts::DOOM_DESIRE_MOVE_NAME,
        consts::STRUGGLE_MOVE_NAME,
        gc::BEAT_UP_MOVE,
    ]
    .contains(&mv.name.as_str());

    if !skip_type_and_immunity {
        if gen.effectiveness(&move_type, &d1) == Some(consts::IMMUNE) || gen.effectiveness(&move_type, &d2) == Some(consts::IMMUNE) {
            return None;
        } else if defending_pkmn.ability == gc::LEVITATE && move_type == consts::TYPE_GROUND {
            return None;
        } else if defending_pkmn.ability == gc::DAMP && (mv.name == consts::SELFDESTRUCT_MOVE_NAME || mv.name == consts::EXPLOSION_MOVE_NAME) {
            return None;
        } else if defending_pkmn.ability == gc::VOLT_ABSORB && move_type == consts::TYPE_ELECTRIC {
            return None;
        } else if defending_pkmn.ability == gc::WATER_ABSORB && move_type == consts::TYPE_WATER {
            return None;
        } else if defending_pkmn.ability == gc::FLASH_FIRE && move_type == consts::TYPE_FIRE {
            return None;
        } else if defending_pkmn.ability == gc::SOUNDPROOF && gc::GEN3_SOUND_MOVES.contains(&mv.name.as_str()) {
            return None;
        } else if defending_pkmn.ability == gc::WONDER_GUARD && wonder_guard_blocks(gen, &move_type, &d1, &d2, true) {
            return None;
        }
    }

    if mv.has_flavor(consts::FLAVOR_FIXED_DAMAGE) {
        if defending_pkmn.ability == gc::WONDER_GUARD && wonder_guard_blocks(gen, &move_type, &d1, &d2, false) {
            return None;
        }
        return Some(DamageRange::single(base_power));
    } else if mv.has_flavor(consts::FLAVOR_LEVEL_DAMAGE) {
        return Some(DamageRange::single(attacking_pkmn.level));
    } else if mv.has_flavor(consts::FLAVOR_PSYWAVE) {
        let pairs: Vec<(i64, i64)> = (0..11).map(|k| (floor_div(attacking_pkmn.level * (50 + 10 * k), 100), 1)).collect();
        return DamageRange::from_pairs(&pairs, 1);
    } else if mv.name == gc::SUPER_FANG_MOVE {
        return Some(DamageRange::single(floor_div(defending_pkmn.cur_stats.hp, 2).max(1)));
    } else if gc::OHKO_MOVE_NAMES.contains(&mv.name.as_str()) {
        if defending_pkmn.ability == gc::STURDY {
            return None;
        }
        if attacking_pkmn.level < defending_pkmn.level {
            return None;
        }
        return Some(DamageRange::single(defending_pkmn.cur_stats.hp));
    } else if mv.name == gc::ENDEAVOR_MOVE {
        let user_hp_pct = py_int(custom).unwrap_or(100);
        let user_hp = floor_div(attacking_pkmn.cur_stats.hp * user_hp_pct, 100);
        let target_hp = defending_pkmn.cur_stats.hp;
        if target_hp <= user_hp {
            return None;
        }
        return Some(DamageRange::single(target_hp - user_hp));
    } else if mv.name == gc::TRIPLE_KICK_MOVE && !custom.starts_with(gc::TRIPLE_KICK_SENTINEL_PREFIX) {
        let num_kicks = py_int(custom).unwrap_or(1).clamp(1, 3);
        let mut total: Option<DamageRange> = None;
        for kick_num in 1..=num_kicks {
            let sentinel = format!("{}{}", gc::TRIPLE_KICK_SENTINEL_PREFIX, 10 * kick_num);
            let sub = DamageArgs {
                attacking: attacking_pkmn,
                mv,
                defending: defending_pkmn,
                attacking_stages: a.attacking_stages,
                defending_stages: a.defending_stages,
                attacking_field: None,
                defending_field: a.defending_field,
                is_crit: a.is_crit && kick_num == num_kicks,
                custom_move_data: &sentinel,
                weather,
                is_double_battle: a.is_double_battle,
                attacking_battle_stats: None,
                defending_battle_stats: None,
                attacker_is_enemy: false,
            };
            if let Some(kick) = calculate_damage(gen, &sub) {
                total = Some(match total {
                    None => kick,
                    Some(t) => t.add(&kick),
                });
            }
        }
        return total;
    } else if mv.name == gc::BEAT_UP_MOVE {
        let num_hits = py_int(custom).unwrap_or(1).clamp(1, 6);
        let beat_up_base = floor_div(2 * attacking_pkmn.level, 5) + 2;
        let mut pre_roll = floor_div(
            floor_div(beat_up_base * base_power * attacking_species.stats.attack, defending_species.stats.defense),
            50,
        ) + 2;
        if a.is_crit {
            pre_roll *= 2;
        }
        let one_hit = DamageRange::from_rolls(pre_roll, MIN_RANGE, MAX_RANGE);
        let mut total = one_hit.clone();
        for _ in 1..num_hits {
            total = total.add(&one_hit);
        }
        return Some(total);
    } else if mv.name == gc::COUNTER_MOVE || mv.name == gc::MIRROR_COAT_MOVE || mv.name == gc::BIDE_MOVE {
        let damage_taken = py_int(custom)?;
        if damage_taken <= 0 {
            return None;
        }
        return Some(DamageRange::single(damage_taken * 2));
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
    } else if mv.name == gc::PRESENT_MOVE {
        if custom.contains(gc::PRESENT_80) {
            base_power = 80;
        } else if custom.contains(gc::PRESENT_120) {
            base_power = 120;
        } else {
            base_power = 40;
        }
    } else if mv.name == gc::TRIPLE_KICK_MOVE {
        if let Some(rest) = custom.strip_prefix(gc::TRIPLE_KICK_SENTINEL_PREFIX) {
            base_power = py_int(rest)?;
        }
    } else if mv.name == gc::ROLLOUT_MOVE || mv.name == gc::ICE_BALL_MOVE {
        let (turn, curl) = match py_int(custom) {
            Some(t) => (t, false),
            None => (5, true),
        };
        base_power = base_power * pow2(turn - 1) * if curl { 2 } else { 1 };
    } else if mv.name == gc::FURY_CUTTER_MOVE {
        let turn = py_int(custom).map(|t| t.min(5)).unwrap_or(1);
        base_power *= pow2(turn - 1);
    } else if mv.name == gc::NATURE_POWER_MOVE {
        if custom.contains(gc::PLAIN_TERRAIN) {
            base_power = 60;
            move_type = consts::TYPE_NORMAL.to_string();
        } else if custom.contains(gc::SAND_TERRAIN) {
            base_power = 100;
            move_type = consts::TYPE_GROUND.to_string();
        } else if custom.contains(gc::CAVE_TERRAIN) {
            base_power = 80;
            move_type = consts::TYPE_GHOST.to_string();
        } else if custom.contains(gc::ROCK_TERRAIN) {
            base_power = 75;
            move_type = consts::TYPE_ROCK.to_string();
        } else if custom.contains(gc::TALL_GRASS_TERRAIN) {
            return None;
        } else if custom.contains(gc::LONG_GRASS_TERRAIN) {
            base_power = 55;
            move_type = consts::TYPE_GRASS.to_string();
        } else if custom.contains(gc::POND_WATER_TERRAIN) {
            base_power = 65;
            move_type = consts::TYPE_WATER.to_string();
        } else if custom.contains(gc::SEA_WATER_TERRAIN) {
            base_power = 95;
            move_type = consts::TYPE_WATER.to_string();
        } else if custom.contains(gc::UNDERWATER_TERRAIN) {
            base_power = 120;
            move_type = consts::TYPE_WATER.to_string();
        }
    }

    let default_stages = StageModifiers::default();
    let original_attacking_stages = a.attacking_stages.unwrap_or(&default_stages);
    let original_defending_stages = a.defending_stages.unwrap_or(&default_stages);
    let mut attacking_stages = *original_attacking_stages;
    let mut defending_stages = *original_defending_stages;

    // Rage's Attack-stage change is driven by its moves.json ATK+1-self effect entry
    // through the normal stat-stage-setup dropdown (like Metal Claw/Charge Beam), not a
    // per-call custom-data override here -- that lets the boost persist and affect every
    // other move in the matchup, not just Rage's own damage number.

    let is_real_crit = a.is_crit
        && ![consts::SPIT_UP_MOVE_NAME, consts::FUTURE_SIGHT_MOVE_NAME, consts::DOOM_DESIRE_MOVE_NAME].contains(&mv.name.as_str());

    let is_special = gen.special_types.contains(&move_type);
    if is_real_crit {
        if is_special {
            if attacking_stages.special_attack_stage < 0 {
                attacking_stages = StageModifiers::default();
            }
            if defending_stages.special_defense_stage > 0 {
                defending_stages = StageModifiers::default();
            }
        } else {
            if attacking_stages.attack_stage < 0 {
                attacking_stages = StageModifiers::default();
            }
            if defending_stages.defense_stage > 0 {
                defending_stages = StageModifiers::default();
            }
        }
    }

    let reorder_attacker_stage = a.attacking_battle_stats.is_none();
    let reorder_defender_stage = a.defending_battle_stats.is_none();
    let mut atk = match a.attacking_battle_stats {
        Some(s) => *s,
        None => attacking_pkmn.get_battle_stats(&StageModifiers::default(), false, None),
    };
    let mut def = match a.defending_battle_stats {
        Some(s) => *s,
        None => defending_pkmn.get_battle_stats(&StageModifiers::default(), false, None),
    };

    let held = attacking_pkmn.held_item_str();
    if held == gc::THICK_CLUB && (attacking_pkmn.name == gc::GEN2_MAROWAK || attacking_pkmn.name == gc::GEN2_CUBONE) {
        atk.attack *= 2;
    } else if attacking_pkmn.name == gc::PIKACHU && held == gc::LIGHT_BALL {
        atk.special_attack *= 2;
    } else if attacking_pkmn.name == gc::CLAMPERL && held == gc::DEEP_SEA_TOOTH {
        atk.special_attack *= 2;
    } else if (attacking_pkmn.name == gc::LATIOS || attacking_pkmn.name == gc::LATIAS) && held == gc::SOUL_DEW {
        atk.special_attack = floor_mul_ratio(atk.special_attack, 3, 2);
    }

    let dheld = defending_pkmn.held_item_str();
    if (defending_pkmn.name == gc::LATIOS || defending_pkmn.name == gc::LATIAS) && dheld == gc::SOUL_DEW {
        def.special_defense = floor_mul_ratio(def.special_defense, 3, 2);
    } else if defending_pkmn.name == gc::CLAMPERL && dheld == gc::DEEP_SEA_SCALE {
        def.special_defense *= 2;
    } else if defending_pkmn.name == gc::DITTO && dheld == gc::METAL_POWDER {
        def.defense *= 2;
    }

    if attacking_pkmn.ability == gc::HUSTLE {
        atk.attack = floor_mul_ratio(atk.attack, 3, 2);
    }
    if attacking_pkmn.ability == gc::HUGE_POWER || attacking_pkmn.ability == gc::PURE_POWER {
        atk.attack *= 2;
    }
    if held == gc::CHOICE_BAND {
        atk.attack = floor_mul_ratio(atk.attack, 3, 2);
    }

    let type_boost_type = gen.held_item_boosts.get(held).map(|s| s.as_str());
    if type_boost_type == Some(move_type.as_str()) {
        if is_special {
            atk.special_attack = floor_mul_ratio(atk.special_attack, 11, 10);
        } else {
            atk.attack = floor_mul_ratio(atk.attack, 11, 10);
        }
    } else if held == gc::SEA_INCENSE && move_type == consts::TYPE_WATER {
        atk.special_attack = floor_mul_ratio(atk.special_attack, 21, 20);
    }

    if defending_pkmn.ability == gc::THICK_FAT && (move_type == consts::TYPE_FIRE || move_type == consts::TYPE_ICE) {
        atk.special_attack = floor_div(atk.special_attack, 2);
        atk.attack = floor_div(atk.attack, 2);
    }

    if mv.name == consts::EXPLOSION_MOVE_NAME || mv.name == consts::SELFDESTRUCT_MOVE_NAME {
        def.defense = floor_div(def.defense, 2).max(1);
    }

    if reorder_attacker_stage {
        atk.attack = stats::modify_stat_by_stage(Gen::Three, atk.attack, attacking_stages.attack_stage);
        atk.special_attack = stats::modify_stat_by_stage(Gen::Three, atk.special_attack, attacking_stages.special_attack_stage);
    }
    if reorder_defender_stage {
        def.defense = stats::modify_stat_by_stage(Gen::Three, def.defense, defending_stages.defense_stage);
        def.special_defense = stats::modify_stat_by_stage(Gen::Three, def.special_defense, defending_stages.special_defense_stage);
    }

    let light_screen = a.defending_field.map(|f| f.light_screen).unwrap_or(false);
    let reflect = a.defending_field.map(|f| f.reflect).unwrap_or(false);
    let (attacking_stat, defending_stat, screen_active) = if is_special {
        (atk.special_attack, def.special_defense, light_screen && !is_real_crit && mv.name != gc::BRICK_BREAK_MOVE)
    } else {
        (atk.attack, def.defense, reflect && !is_real_crit && mv.name != gc::BRICK_BREAK_MOVE)
    };

    let mut is_stab = a1 == move_type || a2 == move_type;
    if mv.name == consts::FUTURE_SIGHT_MOVE_NAME || mv.name == consts::DOOM_DESIRE_MOVE_NAME {
        is_stab = false;
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
            temp = 2 * floor_div(temp, 3);
        } else {
            temp = floor_div(temp, 2);
        }
    }
    if a.is_double_battle && mv.targeting == consts::TARGETING_BOTH_ENEMIES {
        temp = floor_div(temp, 2);
    }
    if !is_special && temp == 0 {
        temp = 1;
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

    if is_real_crit && defending_pkmn.ability != gc::BATTLE_ARMOR && defending_pkmn.ability != gc::SHELL_ARMOR {
        temp *= 2;
    }

    let mut move_modifier = 1;
    if mv.name == gc::SPIT_UP_MOVE {
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
        gc::EXTRASENSORY_MOVE,
        gc::ASTONISH_MOVE,
        gc::NEEDLE_ARM_MOVE,
        gc::FACADE_MOVE,
        gc::SMELLING_SALT_MOVE,
        gc::REVENGE_MOVE,
    ];
    let double_damage = if bonus_moves.contains(&mv.name.as_str()) {
        !custom.is_empty() && !custom.contains(gc::NO_BONUS)
    } else if mv.name == gc::MAGNITUDE_MOVE {
        custom.contains(gc::DIG_BONUS)
    } else if mv.name == gc::NATURE_POWER_MOVE {
        custom.contains(gc::DIG_BONUS) || custom.contains(gc::DIVE_BONUS)
    } else if mv.name == consts::WEATHER_BALL_MOVE_NAME {
        is_weather_active
    } else {
        false
    };
    if double_damage {
        temp *= 2;
    }

    if is_stab {
        temp = floor_mul_ratio(temp, 3, 2);
    }

    if !skip_type_and_immunity {
        if let Some(row) = gen.type_chart.get(&move_type) {
            for (test_type, eff) in row {
                if *test_type == d1 || *test_type == d2 {
                    if eff == consts::SUPER_EFFECTIVE {
                        temp *= 2;
                    } else if eff == consts::NOT_VERY_EFFECTIVE {
                        temp = floor_div(temp, 2);
                    }
                }
            }
        }
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
                attacking_stages: Some(original_attacking_stages),
                defending_stages: Some(original_defending_stages),
                attacking_field: None,
                defending_field: a.defending_field,
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
        // Python: 2 ** negative is a float < 1; base power then floors to 0 later
        0
    } else {
        1i64 << exp.min(40)
    }
}
