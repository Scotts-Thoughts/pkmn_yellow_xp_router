//! Damage calculation for gens 1-5, the kill-percentage search and the
//! battle-summary model (`controllers/battle_summary_controller.py`).

pub mod battle_summary;
pub mod damage;
pub mod gen1;
pub mod gen2;
pub mod gen3;
pub mod gen4;
pub mod gen5;

use xpr_data::model::{EnemyPkmn, FieldStatus, Gen, Move, StageModifiers, StatBlock};
use xpr_data::GenData;

pub use battle_summary::{BattleSummary, MoveRenderInfo, PkmnRenderInfo};
pub use damage::{find_kill, DamageRange, KillRange};

/// Arguments of `CurrentGen.calculate_damage`.
#[derive(Clone, Copy)]
pub struct DamageArgs<'a> {
    pub attacking: &'a EnemyPkmn,
    pub mv: &'a Move,
    pub defending: &'a EnemyPkmn,
    pub attacking_stages: Option<&'a StageModifiers>,
    pub defending_stages: Option<&'a StageModifiers>,
    pub attacking_field: Option<&'a FieldStatus>,
    pub defending_field: Option<&'a FieldStatus>,
    pub is_crit: bool,
    /// `custom_move_data` (`""` for Python's `None`)
    pub custom_move_data: &'a str,
    pub weather: &'a str,
    pub is_double_battle: bool,
    pub attacking_battle_stats: Option<&'a StatBlock>,
    pub defending_battle_stats: Option<&'a StatBlock>,
    pub attacker_is_enemy: bool,
}

impl<'a> DamageArgs<'a> {
    pub fn new(attacking: &'a EnemyPkmn, mv: &'a Move, defending: &'a EnemyPkmn) -> DamageArgs<'a> {
        DamageArgs {
            attacking,
            mv,
            defending,
            attacking_stages: None,
            defending_stages: None,
            attacking_field: None,
            defending_field: None,
            is_crit: false,
            custom_move_data: "",
            weather: xpr_core::consts::WEATHER_NONE,
            is_double_battle: false,
            attacking_battle_stats: None,
            defending_battle_stats: None,
            attacker_is_enemy: false,
        }
    }
}

/// `CurrentGen.calculate_damage`
pub fn calculate_damage(gen: &GenData, args: &DamageArgs) -> Option<DamageRange> {
    match gen.gen {
        Gen::One => gen1::calculate_damage(gen, args),
        Gen::Two => gen2::calculate_damage(gen, args),
        Gen::Three => gen3::calculate_damage(gen, args),
        Gen::Four => gen4::calculate_damage(gen, args),
        Gen::Five => gen5::calculate_damage(gen, args),
    }
}

/// `CurrentGen.get_crit_rate`
pub fn get_crit_rate(gen: &GenData, pkmn: &EnemyPkmn, mv: &Move, custom_move_data: Option<&str>) -> f64 {
    match gen.gen {
        Gen::One => gen1::get_crit_rate(pkmn, mv, custom_move_data.unwrap_or("")),
        Gen::Two => gen2::get_crit_rate(pkmn, mv),
        Gen::Three => gen3::get_crit_rate(pkmn, mv, custom_move_data),
        Gen::Four => gen4::get_crit_rate(pkmn, mv, custom_move_data),
        Gen::Five => gen5::get_crit_rate(pkmn, mv, custom_move_data),
    }
}

/// `CurrentGen.get_move_accuracy`: `None` means the move always hits.
pub fn get_move_accuracy(gen: &GenData, pkmn: &EnemyPkmn, mv: &Move, custom_move_data: Option<&str>, defending: &EnemyPkmn, weather: &str) -> Option<f64> {
    let custom = custom_move_data.unwrap_or("");
    match gen.gen {
        Gen::One => mv.accuracy.map(|a| a as f64),
        Gen::Two => gen2::get_move_accuracy(pkmn, mv, defending, weather),
        Gen::Three => gen3::get_move_accuracy(gen, pkmn, mv, custom, defending, weather),
        Gen::Four => gen4::get_move_accuracy(pkmn, mv, custom, defending, weather),
        Gen::Five => gen5::get_move_accuracy(pkmn, mv, custom, defending, weather),
    }
}
