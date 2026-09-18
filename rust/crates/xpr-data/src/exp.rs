//! Port of `pkmn/universal_utils.py`: EXP curves, level lookup, yields.

use xpr_core::consts;
use xpr_core::floor_div;

use crate::model::{EnemyPkmn, TrainerTimingStats};

/// `calc_xp_yield`
pub fn calc_xp_yield(base_yield: i64, level: i64, is_trainer_battle: bool, exp_split: i64) -> i64 {
    let mut result = base_yield * level;
    result = floor_div(result, 7);
    result = if exp_split == 0 { result } else { floor_div(result, exp_split) };
    if is_trainer_battle {
        result = floor_div(result * 3, 2);
    }
    result
}

/// `calc_level_gain`
pub fn calc_level_gain(init_level: i64, init_tnl: i64, final_level: i64, final_tnl: i64) -> String {
    if init_level == final_level {
        if init_tnl == final_tnl {
            return String::new();
        }
        return format!("{:.1}", ((init_tnl - final_tnl) as f64) / 100.0);
    }
    let full_levels_gained = final_level - (init_level + 1);
    let partial_gain = ((init_tnl + (100 - final_tnl)) as f64) / 100.0;
    format!("{:.1}", (full_levels_gained as f64) + partial_gain)
}

/// `xp_needed_for_level`
pub fn xp_needed_for_level(l: i64, growth_rate: &str) -> Result<i64, String> {
    let l3 = l * l * l;
    match growth_rate {
        consts::GROWTH_RATE_FAST => Ok(floor_div(4 * l3, 5)),
        consts::GROWTH_RATE_MEDIUM_FAST => Ok(l3),
        consts::GROWTH_RATE_MEDIUM_SLOW => Ok(floor_div(6 * l3 - 75 * l * l + 500 * l - 700, 5)),
        consts::GROWTH_RATE_SLOW => Ok(floor_div(5 * l3, 4)),
        consts::GROWTH_RATE_ERRATIC => {
            if l < 50 {
                Ok(floor_div(l3 * (100 - l), 50))
            } else if l < 68 {
                Ok(floor_div(l3 * (150 - l), 100))
            } else if l < 98 {
                let partial = floor_div(1911 - (10 * l), 3);
                Ok(floor_div(l3 * partial, 500))
            } else {
                Ok(floor_div(l3 * (160 - l), 100))
            }
        }
        consts::GROWTH_RATE_FLUCTUATING => {
            if l < 15 {
                let partial = floor_div(l + 1, 3);
                Ok(floor_div(l3 * (partial + 24), 50))
            } else if l < 36 {
                Ok(floor_div(l3 * (l + 14), 50))
            } else {
                let partial = floor_div(l, 2);
                Ok(floor_div(l3 * (partial + 32), 50))
            }
        }
        other => Err(format!("Unknown growth rate: {}", other)),
    }
}

pub const ALL_GROWTH_RATES: [&str; 6] = [
    consts::GROWTH_RATE_FAST,
    consts::GROWTH_RATE_MEDIUM_FAST,
    consts::GROWTH_RATE_MEDIUM_SLOW,
    consts::GROWTH_RATE_SLOW,
    consts::GROWTH_RATE_ERRATIC,
    consts::GROWTH_RATE_FLUCTUATING,
];

#[derive(Clone, Debug)]
pub struct LevelLookup {
    pub growth_rate: String,
    pub thresholds: Vec<i64>,
}

impl LevelLookup {
    pub fn new(growth_rate: &str) -> Result<LevelLookup, String> {
        let mut thresholds = Vec::with_capacity(100);
        for i in 0..100 {
            thresholds.push(xp_needed_for_level(i + 1, growth_rate)?);
        }
        Ok(LevelLookup {
            growth_rate: growth_rate.to_string(),
            thresholds,
        })
    }

    pub fn get_xp_for_level(&self, target_level: i64) -> Result<i64, String> {
        if target_level <= 0 || target_level > 100 {
            return Err(format!(
                "Pkmn cannot be level {}, cannot get XP needed for invalid level",
                target_level
            ));
        }
        Ok(self.thresholds[(target_level - 1) as usize])
    }

    /// `(level, xp_to_next_level)`
    pub fn get_level_info(&self, cur_xp: i64) -> (i64, i64) {
        let mut cur_level: i64 = 99;
        let mut req_exp: i64 = 0;
        let mut did_break = false;
        for (idx, req) in self.thresholds.iter().enumerate() {
            cur_level = idx as i64;
            req_exp = *req;
            if cur_xp < *req {
                did_break = true;
                break;
            }
        }
        if !did_break {
            cur_level = 100;
        }
        if cur_level == 100 {
            return (100, 0);
        }
        (cur_level, req_exp - cur_xp)
    }
}

/// Cached lookups for the six growth rates (`universal_utils.level_lookups`).
pub fn level_lookup(growth_rate: &str) -> Option<&'static LevelLookup> {
    use once_cell::sync::Lazy;
    static LOOKUPS: Lazy<Vec<LevelLookup>> = Lazy::new(|| {
        ALL_GROWTH_RATES
            .iter()
            .map(|g| LevelLookup::new(g).expect("static growth rates are valid"))
            .collect()
    });
    LOOKUPS.iter().find(|l| l.growth_rate == growth_rate)
}

/// `experience_per_second`: `str(round(exp / seconds))` with banker's rounding.
pub fn experience_per_second(timing: &TrainerTimingStats, pkmn_list: &[EnemyPkmn]) -> String {
    let total: i64 = pkmn_list.iter().map(|x| x.xp).sum();
    let result = timing.get_optimal_exp_per_second(pkmn_list.len() as i64, total);
    if !result.is_finite() {
        return String::new();
    }
    xpr_core::pyjson::python_round_int(result).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves() {
        assert_eq!(xp_needed_for_level(100, consts::GROWTH_RATE_FAST).unwrap(), 800000);
        assert_eq!(xp_needed_for_level(100, consts::GROWTH_RATE_MEDIUM_FAST).unwrap(), 1000000);
        assert_eq!(xp_needed_for_level(100, consts::GROWTH_RATE_MEDIUM_SLOW).unwrap(), 1059860);
        assert_eq!(xp_needed_for_level(100, consts::GROWTH_RATE_SLOW).unwrap(), 1250000);
        assert_eq!(xp_needed_for_level(100, consts::GROWTH_RATE_ERRATIC).unwrap(), 600000);
        assert_eq!(xp_needed_for_level(100, consts::GROWTH_RATE_FLUCTUATING).unwrap(), 1640000);
        assert_eq!(xp_needed_for_level(1, consts::GROWTH_RATE_MEDIUM_SLOW).unwrap(), -54);
        assert_eq!(xp_needed_for_level(5, consts::GROWTH_RATE_MEDIUM_SLOW).unwrap(), 135);
    }

    #[test]
    fn level_info() {
        let l = level_lookup(consts::GROWTH_RATE_MEDIUM_SLOW).unwrap();
        assert_eq!(l.get_level_info(135), (5, 44));
        assert_eq!(l.get_level_info(1059860), (100, 0));
    }

    #[test]
    fn level_gain_strings() {
        assert_eq!(calc_level_gain(5, 50, 5, 50), "");
        assert_eq!(calc_level_gain(5, 50, 5, 25), "0.2");
        assert_eq!(calc_level_gain(5, 50, 7, 80), "1.7");
    }
}
