//! Port of `pkmn/gen_1/pkmn_damage_calc.py`.

use xpr_core::consts;
use xpr_core::floor_div;
use xpr_data::gen_consts as gc;
use xpr_data::model::{EnemyPkmn, Move, StageModifiers};
use xpr_data::GenData;

use crate::damage::{self, py_int, py_isdigit, DamageRange};
use crate::DamageArgs;

const MIN_RANGE: i64 = 217;
const MAX_RANGE: i64 = 255;

/// `get_crit_rate`
pub fn get_crit_rate(pkmn: &EnemyPkmn, mv: &Move, custom_move_data: &str) -> f64 {
    let is_high_crit = mv.has_flavor(consts::FLAVOR_HIGH_CRIT);
    let has_focus_energy = !custom_move_data.is_empty() && custom_move_data.contains(gc::FLAVOR_FOCUS_ENERGY);
    let mut b = pkmn.base_stats.speed / 2;
    if has_focus_energy {
        b /= 2;
    } else {
        b = 255.min(b * 2);
    }
    if is_high_crit {
        b = 255.min(b * 4);
    } else {
        b /= 2;
    }
    (b.min(255) as f64) / 256.0
}

/// `get_move_accuracy`: the listed accuracy, except for OHKO moves, which
/// report their exact hit chance (30% is stored as 76/256).
pub fn get_move_accuracy(mv: &Move) -> Option<f64> {
    if mv.has_flavor(gc::FLAVOR_ONE_HIT_KO) {
        return mv.accuracy.map(|a| ((a * 255 / 100) as f64) / 256.0 * 100.0);
    }
    mv.accuracy.map(|a| a as f64)
}

/// `calculate_gen_one_damage`
pub fn calculate_damage(gen: &GenData, a: &DamageArgs) -> Option<DamageRange> {
    let mv = a.mv;
    let attacking_pkmn = a.attacking;
    let defending_pkmn = a.defending;
    let custom = a.custom_move_data;

    if let Some(r) = damage::get_special_damage_override(mv, attacking_pkmn, None, None) {
        return Some(r);
    }

    let attacking_species = gen.pkmn_db().get_pkmn(&attacking_pkmn.name)?;
    let defending_species = gen.pkmn_db().get_pkmn(&defending_pkmn.name)?;

    let default_stages = StageModifiers::default();
    let attacking_stages = a.attacking_stages.unwrap_or(&default_stages);
    let attacking_battle_stats = match a.attacking_battle_stats {
        Some(s) => *s,
        None => attacking_pkmn.get_battle_stats(attacking_stages, a.is_crit, None),
    };
    let defending_stages = a.defending_stages.unwrap_or(&default_stages);
    let defending_battle_stats = match a.defending_battle_stats {
        Some(s) => *s,
        None => defending_pkmn.get_battle_stats(defending_stages, a.is_crit, None),
    };

    let first_eff = gen.effectiveness(&mv.move_type, &defending_species.first_type);
    let second_eff = if defending_species.first_type != defending_species.second_type {
        gen.effectiveness(&mv.move_type, &defending_species.second_type)
    } else {
        None
    };
    let is_immune = first_eff == Some(consts::IMMUNE) || second_eff == Some(consts::IMMUNE);

    if mv.has_flavor(gc::FLAVOR_SUPER_FANG) {
        let hp_pct = gc::super_fang_hp_percentage(custom);
        let target_hp = floor_div(defending_pkmn.cur_stats.hp * hp_pct, 100);
        return Some(DamageRange::single(floor_div(target_hp, 2).max(1)));
    }

    if mv.has_flavor(gc::FLAVOR_ONE_HIT_KO) {
        if is_immune {
            return None;
        }
        // OHKO moves can't crit: the speed check always uses the real battle
        // speeds, not the crit stats (which drop stages and badge boosts).
        let (attacking_speed, defending_speed) = if a.is_crit {
            (
                a.attacking_battle_stats.map(|s| s.speed).unwrap_or_else(|| attacking_pkmn.get_battle_stats(attacking_stages, false, None).speed),
                a.defending_battle_stats.map(|s| s.speed).unwrap_or_else(|| defending_pkmn.get_battle_stats(defending_stages, false, None).speed),
            )
        } else {
            (attacking_battle_stats.speed, defending_battle_stats.speed)
        };
        if attacking_speed < defending_speed {
            return None;
        }
        return Some(DamageRange::single(defending_pkmn.cur_stats.hp));
    }

    if mv.has_flavor(gc::FLAVOR_COUNTER) || mv.has_flavor(gc::FLAVOR_BIDE) {
        let prior = if !custom.is_empty() && py_isdigit(custom.trim()) {
            py_int(custom.trim())
        } else {
            None
        };
        let prior = match prior {
            Some(p) if p != 0 => p,
            _ => return None,
        };
        return Some(DamageRange::single((prior * 2).min(65535)));
    }

    let base_power = match mv.base_power {
        None | Some(0) => return None,
        Some(bp) => bp,
    };

    if mv.has_flavor(consts::FLAVOR_FIXED_DAMAGE) {
        return Some(DamageRange::single(base_power));
    } else if mv.has_flavor(consts::FLAVOR_LEVEL_DAMAGE) {
        return Some(DamageRange::single(attacking_pkmn.level));
    } else if mv.has_flavor(consts::FLAVOR_PSYWAVE) {
        let upper = floor_div(attacking_pkmn.level * 3, 2);
        let lower = if a.attacker_is_enemy { 0 } else { 1 };
        let pairs: Vec<(i64, i64)> = (lower..upper).map(|x| (x, 1)).collect();
        return DamageRange::from_pairs(&pairs, 1);
    }

    if is_immune {
        return None;
    }

    let is_special = gen.special_types.contains(&mv.move_type);
    let (mut attacking_stat, mut defending_stat, doubled_def) = if is_special {
        (
            attacking_battle_stats.special_attack,
            defending_battle_stats.special_defense,
            a.defending_field.map(|f| f.light_screen).unwrap_or(false) && !a.is_crit,
        )
    } else {
        (
            attacking_battle_stats.attack,
            defending_battle_stats.defense,
            a.defending_field.map(|f| f.reflect).unwrap_or(false) && !a.is_crit,
        )
    };

    if doubled_def {
        defending_stat *= 2;
    }

    if attacking_stat > 255 || defending_stat > 255 {
        attacking_stat = floor_div(attacking_stat, 4);
        defending_stat = floor_div(defending_stat, 4);
        if attacking_stat == 0 {
            attacking_stat = 1;
        }
    }
    attacking_stat &= 0xFF;
    defending_stat &= 0xFF;

    if mv.name == consts::EXPLOSION_MOVE_NAME || mv.name == consts::SELFDESTRUCT_MOVE_NAME {
        defending_stat >>= 1;
        if defending_stat == 0 {
            defending_stat = 1;
        }
    }

    if defending_stat == 0 {
        return None;
    }

    let is_stab = attacking_species.first_type == mv.move_type || attacking_species.second_type == mv.move_type;

    let mut temp = 2 * attacking_pkmn.level;
    if a.is_crit {
        temp *= 2;
    }
    temp = floor_div(temp, 5) + 2;
    temp *= base_power;
    temp *= attacking_stat;
    temp = floor_div(temp, defending_stat);
    temp = floor_div(temp, 50);
    temp = temp.min(997);
    temp += 2;

    let stab_bonus = if is_stab { floor_div(temp, 2) } else { 0 };
    temp += stab_bonus;

    let mut steps: Vec<(usize, &str)> = Vec::new();
    if let Some(e) = first_eff {
        let row = gc::gen1_type_row(&mv.move_type, &defending_species.first_type)?;
        steps.push((row, e));
    }
    if let Some(e) = second_eff {
        let row = gc::gen1_type_row(&mv.move_type, &defending_species.second_type)?;
        steps.push((row, e));
    }
    steps.sort_by_key(|s| s.0);
    for (_, eff) in steps {
        if eff == consts::SUPER_EFFECTIVE {
            temp *= 2;
        } else if eff == consts::NOT_VERY_EFFECTIVE {
            temp = floor_div(temp, 2);
        }
        if temp == 0 {
            return None;
        }
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
    } else if mv.has_flavor(gc::FLAVOR_PARTIAL_TRAPPING) {
        multi_hit = gc::partial_trap_turn_count(custom);
    }

    let pairs: Vec<(i64, i64)> = (MIN_RANGE..=MAX_RANGE)
        .map(|n| (floor_div(temp * n, MAX_RANGE).max(1) * multi_hit, 1))
        .collect();
    DamageRange::from_pairs(&pairs, 1)
}
