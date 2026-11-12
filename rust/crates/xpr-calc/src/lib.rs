//! Damage calculation for gens 1-5, the kill-percentage search and the
//! battle-summary model (`controllers/battle_summary_controller.py`).

pub mod battle_summary;
pub mod damage;
pub mod gen1;
pub mod gen2;
pub mod gen3;
pub mod gen4;
pub mod gen5;
pub mod pp;

use xpr_data::model::{EnemyPkmn, FieldStatus, Gen, Move, StageModifiers, StatBlock};
use xpr_data::GenData;

pub use battle_summary::{BattleSummary, MoveRenderInfo, PkmnRenderInfo};
pub use damage::{find_kill, find_kill_hits, DamageRange, HitModel, KillRange};

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
    /// Wild encounter (Ruby/Sapphire only apply the Atk/Def/SpA/SpD badge
    /// boosts in trainer battles).
    pub is_wild_battle: bool,
    /// The attacker is at HP <= max HP / 3 (the move's "Blaze (HP <= 1/3)"
    /// style option), so Overgrow / Blaze / Torrent / Swarm boost a move of
    /// their type. Eruption / Water Spout read it from their HP % instead.
    pub attacker_pinch: bool,
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
            is_wild_battle: false,
            attacker_pinch: false,
        }
    }
}

/// Whether the attacker's pinch ability (Overgrow / Blaze / Torrent / Swarm)
/// boosts this use, `move_type` being the resolved type (Hidden Power,
/// Weather Ball, ...). The game test is `curHP <= maxHP / 3`; Eruption and
/// Water Spout take the HP from their own HP % dropdown.
pub fn pinch_boost_applies(args: &DamageArgs, move_type: &str) -> bool {
    use xpr_data::gen_consts as gc;
    if gc::pinch_ability_type(&args.attacking.ability) != Some(move_type) {
        return false;
    }
    if args.mv.name == gc::ERUPTION_MOVE || args.mv.name == gc::WATER_SPOUT_MOVE {
        if let Some(pct) = damage::py_int(args.custom_move_data) {
            let max_hp = args.attacking.cur_stats.hp;
            return xpr_core::floor_div(max_hp * pct, 100) <= xpr_core::floor_div(max_hp, 3);
        }
    }
    args.attacker_pinch
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

/// The per-hit ranges of a multi-hit use (gens 2-5), or `None` when the
/// use is a single hit (or gen 1, whose hits share one roll).
pub fn hit_model(gen: &GenData, args: &DamageArgs) -> Option<HitModel> {
    match gen.gen {
        Gen::One => None,
        Gen::Two => gen2::hit_model(gen, args),
        Gen::Three => gen3::hit_model(gen, args),
        Gen::Four => gen4::hit_model(gen, args),
        Gen::Five => gen5::hit_model(gen, args),
    }
}

/// `CurrentGen.get_crit_rate`: the chance that one hit of `args.mv` by
/// `args.attacking` against `args.defending` is a critical hit.
pub fn get_crit_rate(gen: &GenData, args: &DamageArgs) -> f64 {
    let custom = args.custom_move_data;
    match gen.gen {
        Gen::One => gen1::get_crit_rate(args.attacking, args.mv, custom),
        Gen::Two => gen2::get_crit_rate(args.attacking, args.mv),
        Gen::Three => gen3::get_crit_rate(args.attacking, args.mv, Some(custom), args.defending),
        Gen::Four => gen4::get_crit_rate(args, Some(custom)),
        Gen::Five => gen5::get_crit_rate(args.attacking, args.mv, Some(custom)),
    }
}

/// `CurrentGen.get_move_accuracy`: the hit chance in percent; `None` means
/// the move always hits.
pub fn get_move_accuracy(gen: &GenData, args: &DamageArgs) -> Option<f64> {
    let custom = args.custom_move_data;
    match gen.gen {
        Gen::One => gen1::get_move_accuracy(args),
        Gen::Two => gen2::get_move_accuracy(args),
        Gen::Three => gen3::get_move_accuracy(gen, args, custom),
        Gen::Four => gen4::get_move_accuracy(args, custom),
        Gen::Five => gen5::get_move_accuracy(args.attacking, args.mv, custom, args.defending, args.weather),
    }
}
