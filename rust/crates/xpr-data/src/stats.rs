//! Per-generation stat arithmetic: `pkmn/gen_1/pkmn_utils.py` and the
//! `data_objects.py` modules of gens 2-5. Every float multiplier in the
//! Python source is computed as exact rational integer math (see the note on
//! `xpr_core::floor_mul_ratio`).

use xpr_core::consts;
use xpr_core::{ceil_sqrt_capped, floor_div, floor_mul_ratio};

use crate::badges::BadgeList;
use crate::model::{EnemyPkmn, FieldStatus, Gen, Nature, PokemonSpecies, StageModifiers, StatBlock};

pub const STAT_MIN: i64 = 1;
pub const STAT_MAX: i64 = 999;
const BASE_STAGE_INDEX: i64 = 6;

const STAGE_MODIFIERS_GEN12: [(i64, i64); 13] = [
    (25, 100),
    (28, 100),
    (33, 100),
    (40, 100),
    (50, 100),
    (66, 100),
    (1, 1),
    (15, 10),
    (2, 1),
    (25, 10),
    (3, 1),
    (35, 10),
    (4, 1),
];

const STAGE_MODIFIERS_GEN345: [(i64, i64); 13] = [
    (2, 8),
    (2, 7),
    (2, 6),
    (2, 5),
    (2, 4),
    (2, 3),
    (2, 2),
    (3, 2),
    (4, 2),
    (5, 2),
    (6, 2),
    (7, 2),
    (8, 2),
];

/// Per-gen vitamin / stat-XP constants.
#[derive(Clone, Copy, Debug)]
pub struct VitaminConsts {
    pub vit_amt: i64,
    pub vit_cap: i64,
    pub value_cap: i64,
}

pub fn vitamin_consts(gen: Gen) -> VitaminConsts {
    match gen {
        Gen::One | Gen::Two => VitaminConsts {
            vit_amt: 2560,
            vit_cap: 25600,
            value_cap: 65535,
        },
        Gen::Three | Gen::Four | Gen::Five => VitaminConsts {
            vit_amt: 10,
            vit_cap: 100,
            value_cap: 100,
        },
    }
}

pub const EV_BERRY_AMT: i64 = 10;
pub const EV_BERRY_DROP_TARGET_GEN4: i64 = 100;

/// Bulbapedia blackout base values keyed by badge count (gens 3-5).
pub fn blackout_base_val(num_badges: i64) -> i64 {
    match num_badges {
        0 => 8,
        1 => 16,
        2 => 24,
        3 => 36,
        4 => 48,
        5 => 60,
        6 => 80,
        7 => 100,
        8 => 120,
        _ => 120,
    }
}

fn clamp_stat(v: i64) -> i64 {
    v.clamp(STAT_MIN, STAT_MAX)
}

/// `modify_stat_by_stage`; out-of-range stages raise in Python.
pub fn modify_stat_by_stage(gen: Gen, raw_stat: i64, stage: i64) -> i64 {
    if stage == 0 {
        return raw_stat;
    }
    let idx = BASE_STAGE_INDEX + stage;
    let table: &[(i64, i64); 13] = match gen {
        Gen::One | Gen::Two => &STAGE_MODIFIERS_GEN12,
        _ => &STAGE_MODIFIERS_GEN345,
    };
    let (num, den) = table[idx.clamp(0, 12) as usize];
    floor_div(raw_stat * num, den)
}

/// Gen 1/2 `calc_unboosted_stat`
fn calc_unboosted_stat_gen12(base_val: i64, level: i64, dv: i64, stat_xp: i64, is_hp: bool) -> i64 {
    let mut temp = (base_val + dv) * 2;
    temp += floor_div(ceil_sqrt_capped(stat_xp), 4);
    temp = floor_div(temp * level, 100);
    if is_hp {
        temp + level + 10
    } else {
        temp + 5
    }
}

/// Gens 3-5 `calc_unboosted_stat`
fn calc_unboosted_stat_gen345(
    base_val: i64,
    level: i64,
    iv: i64,
    ev: i64,
    is_hp: bool,
    nature_raised: bool,
    nature_lowered: bool,
) -> i64 {
    let mut temp = (2 * base_val) + iv;
    temp += floor_div(ev, 4);
    temp = floor_div(temp * level, 100);
    if is_hp {
        return temp + level + 10;
    }
    let result = temp + 5;
    if nature_raised {
        return floor_mul_ratio(result, 11, 10);
    }
    if nature_lowered {
        return floor_mul_ratio(result, 9, 10);
    }
    result
}

/// `badge_boost_single_stat` per gen.
pub fn badge_boost_single_stat(gen: Gen, v: i64) -> i64 {
    match gen {
        Gen::One | Gen::Two => clamp_stat(floor_mul_ratio(v, 9, 8)),
        _ => floor_mul_ratio(v, 11, 10),
    }
}

/// Gen 1/2 `calc_stat`
pub fn calc_stat_gen12(base_val: i64, level: i64, dv: i64, stat_xp: i64, is_hp: bool, badge: bool) -> i64 {
    let mut result = calc_unboosted_stat_gen12(base_val, level, dv, stat_xp, is_hp);
    if badge {
        result = badge_boost_single_stat(Gen::One, result);
    }
    clamp_stat(result)
}

/// Gen 1/2 `calc_battle_stat`
pub fn calc_battle_stat_gen12(gen: Gen, base_val: i64, level: i64, dv: i64, stat_xp: i64, stage: i64, badge: bool) -> i64 {
    let mut result = calc_unboosted_stat_gen12(base_val, level, dv, stat_xp, false);
    result = modify_stat_by_stage(gen, result, stage);
    if badge {
        result = badge_boost_single_stat(gen, result);
    }
    clamp_stat(result)
}

/// Gens 3-5 `calc_stat` (badge boost only exists in gen 3).
pub fn calc_stat_gen345(
    base_val: i64,
    level: i64,
    dv: i64,
    stat_xp: i64,
    is_hp: bool,
    badge: bool,
    nature_raised: bool,
    nature_lowered: bool,
) -> i64 {
    let mut result = calc_unboosted_stat_gen345(base_val, level, dv, stat_xp, is_hp, nature_raised, nature_lowered);
    if badge {
        result = badge_boost_single_stat(Gen::Three, result);
    }
    result
}

/// Gen 3 `calc_battle_stat`
#[allow(clippy::too_many_arguments)]
pub fn calc_battle_stat_gen3(
    base_val: i64,
    level: i64,
    dv: i64,
    stat_xp: i64,
    stage: i64,
    badge: bool,
    nature_raised: bool,
    nature_lowered: bool,
    macho_brace: bool,
) -> i64 {
    let mut result = calc_unboosted_stat_gen345(base_val, level, dv, stat_xp, false, nature_raised, nature_lowered);
    if macho_brace {
        result = floor_div(result, 2);
    }
    if badge {
        result = badge_boost_single_stat(Gen::Three, result);
    }
    modify_stat_by_stage(Gen::Three, result, stage)
}

/// Gen 4/5 `calc_battle_stat`
#[allow(clippy::too_many_arguments)]
pub fn calc_battle_stat_gen45(
    gen: Gen,
    base_val: i64,
    level: i64,
    dv: i64,
    stat_xp: i64,
    stage: i64,
    nature_raised: bool,
    nature_lowered: bool,
    slowed_speed: bool,
    choice_scarf: bool,
) -> i64 {
    let mut result = calc_unboosted_stat_gen345(base_val, level, dv, stat_xp, false, nature_raised, nature_lowered);
    if slowed_speed {
        result = floor_div(result, 2);
    }
    if choice_scarf {
        result = floor_mul_ratio(result, 3, 2);
    }
    modify_stat_by_stage(gen, result, stage)
}

/// Gen 2 `should_ignore_spd_badge_boost` (the Glacier badge Sp.Def glitch window).
pub fn should_ignore_spd_badge_boost(unboosted_spa: i64) -> bool {
    (0..=205).contains(&unboosted_spa) || (433..=660).contains(&unboosted_spa)
}

/// `StatBlock.calc_level_stats` for every gen (`self` = base stats).
pub fn calc_level_stats(
    base: &StatBlock,
    level: i64,
    dv: &StatBlock,
    stat_xp: &StatBlock,
    badges: &BadgeList,
    nature: Nature,
    held_item: Option<&str>,
) -> StatBlock {
    let gen = base.gen;
    match gen {
        Gen::One => {
            let special = calc_stat_gen12(base.special_attack, level, dv.special_attack, stat_xp.special_attack, false, badges.has("volcano"));
            StatBlock::new(
                gen,
                calc_stat_gen12(base.hp, level, dv.hp, stat_xp.hp, true, false),
                calc_stat_gen12(base.attack, level, dv.attack, stat_xp.attack, false, badges.has("boulder")),
                calc_stat_gen12(base.defense, level, dv.defense, stat_xp.defense, false, badges.has("thunder")),
                special,
                special,
                calc_stat_gen12(base.speed, level, dv.speed, stat_xp.speed, false, badges.has("soul")),
                false,
            )
        }
        Gen::Two => {
            let unboosted_spa = calc_stat_gen12(base.special_attack, level, dv.special_attack, stat_xp.special_attack, false, false);
            let final_spd = if should_ignore_spd_badge_boost(unboosted_spa) {
                calc_stat_gen12(base.special_defense, level, dv.special_attack, stat_xp.special_attack, false, false)
            } else {
                calc_stat_gen12(base.special_defense, level, dv.special_attack, stat_xp.special_attack, false, badges.has("glacier"))
            };
            StatBlock::new(
                gen,
                calc_stat_gen12(base.hp, level, dv.hp, stat_xp.hp, true, false),
                calc_stat_gen12(base.attack, level, dv.attack, stat_xp.attack, false, badges.has("zephyr")),
                calc_stat_gen12(base.defense, level, dv.defense, stat_xp.defense, false, badges.has("mineral")),
                calc_stat_gen12(base.special_attack, level, dv.special_attack, stat_xp.special_attack, false, badges.has("glacier")),
                final_spd,
                calc_stat_gen12(base.speed, level, dv.speed, stat_xp.speed, false, badges.has("plain")),
                false,
            )
        }
        Gen::Three => {
            let mut speed_stat = calc_stat_gen345(
                base.speed,
                level,
                dv.speed,
                stat_xp.speed,
                false,
                badges.is_speed_boosted(),
                nature.is_stat_raised(consts::SPEED),
                nature.is_stat_lowered(consts::SPEED),
            );
            if held_item == Some(consts::MACHO_BRACE_ITEM_NAME) {
                speed_stat = floor_div(speed_stat, 2);
            }
            StatBlock::new(
                gen,
                calc_stat_gen345(base.hp, level, dv.hp, stat_xp.hp, true, false, false, false),
                calc_stat_gen345(
                    base.attack,
                    level,
                    dv.attack,
                    stat_xp.attack,
                    false,
                    badges.is_attack_boosted(),
                    nature.is_stat_raised(consts::ATTACK),
                    nature.is_stat_lowered(consts::ATTACK),
                ),
                calc_stat_gen345(
                    base.defense,
                    level,
                    dv.defense,
                    stat_xp.defense,
                    false,
                    badges.is_defense_boosted(),
                    nature.is_stat_raised(consts::DEFENSE),
                    nature.is_stat_lowered(consts::DEFENSE),
                ),
                calc_stat_gen345(
                    base.special_attack,
                    level,
                    dv.special_attack,
                    stat_xp.special_attack,
                    false,
                    badges.is_special_attack_boosted(),
                    nature.is_stat_raised(consts::SPECIAL_ATTACK),
                    nature.is_stat_lowered(consts::SPECIAL_ATTACK),
                ),
                calc_stat_gen345(
                    base.special_defense,
                    level,
                    dv.special_defense,
                    stat_xp.special_defense,
                    false,
                    badges.is_special_defense_boosted(),
                    nature.is_stat_raised(consts::SPECIAL_DEFENSE),
                    nature.is_stat_lowered(consts::SPECIAL_DEFENSE),
                ),
                speed_stat,
                false,
            )
        }
        Gen::Four | Gen::Five => {
            let mut speed_stat = calc_stat_gen345(
                base.speed,
                level,
                dv.speed,
                stat_xp.speed,
                false,
                false,
                nature.is_stat_raised(consts::SPEED),
                nature.is_stat_lowered(consts::SPEED),
            );
            if held_item == Some(consts::MACHO_BRACE_ITEM_NAME) {
                speed_stat = floor_div(speed_stat, 2);
            }
            StatBlock::new(
                gen,
                calc_stat_gen345(base.hp, level, dv.hp, stat_xp.hp, true, false, false, false),
                calc_stat_gen345(
                    base.attack,
                    level,
                    dv.attack,
                    stat_xp.attack,
                    false,
                    false,
                    nature.is_stat_raised(consts::ATTACK),
                    nature.is_stat_lowered(consts::ATTACK),
                ),
                calc_stat_gen345(
                    base.defense,
                    level,
                    dv.defense,
                    stat_xp.defense,
                    false,
                    false,
                    nature.is_stat_raised(consts::DEFENSE),
                    nature.is_stat_lowered(consts::DEFENSE),
                ),
                calc_stat_gen345(
                    base.special_attack,
                    level,
                    dv.special_attack,
                    stat_xp.special_attack,
                    false,
                    false,
                    nature.is_stat_raised(consts::SPECIAL_ATTACK),
                    nature.is_stat_lowered(consts::SPECIAL_ATTACK),
                ),
                calc_stat_gen345(
                    base.special_defense,
                    level,
                    dv.special_defense,
                    stat_xp.special_defense,
                    false,
                    false,
                    nature.is_stat_raised(consts::SPECIAL_DEFENSE),
                    nature.is_stat_lowered(consts::SPECIAL_DEFENSE),
                ),
                speed_stat,
                false,
            )
        }
    }
}

/// `StatBlock.calc_battle_stats` for every gen.
#[allow(clippy::too_many_arguments)]
pub fn calc_battle_stats(
    base: &StatBlock,
    level: i64,
    dv: &StatBlock,
    stat_xp: &StatBlock,
    stage_modifiers: &StageModifiers,
    badges: Option<&BadgeList>,
    nature: Nature,
    held_item: Option<&str>,
    is_crit: bool,
    field_status: Option<&FieldStatus>,
) -> StatBlock {
    let gen = base.gen;
    match gen {
        Gen::One => {
            let badge_holder;
            let (badges, stage_modifiers) = if is_crit {
                badge_holder = badges.map(|b| b.empty_like());
                (
                    badge_holder.as_ref(),
                    StageModifiers::only_accuracy_evasion(stage_modifiers.accuracy_stage, stage_modifiers.evasion_stage),
                )
            } else {
                (badges, *stage_modifiers)
            };
            let hp = calc_stat_gen12(base.hp, level, dv.hp, stat_xp.hp, true, false);
            let mut result = StatBlock::new(gen, hp, 0, 0, 0, 0, 0, false);

            let atk_badge = badges.map(|b| b.is_attack_boosted()).unwrap_or(false);
            result.attack = calc_battle_stat_gen12(gen, base.attack, level, dv.attack, stat_xp.attack, stage_modifiers.attack_stage, atk_badge);
            if let Some(b) = badges {
                if b.has("boulder") && stage_modifiers.attack_badge_boosts != 0 {
                    for _ in 0..stage_modifiers.attack_badge_boosts.max(0) {
                        result.attack = badge_boost_single_stat(gen, result.attack);
                    }
                }
            }

            let def_badge = badges.map(|b| b.is_defense_boosted()).unwrap_or(false);
            result.defense = calc_battle_stat_gen12(gen, base.defense, level, dv.defense, stat_xp.defense, stage_modifiers.defense_stage, def_badge);
            if let Some(b) = badges {
                if b.has("thunder") && stage_modifiers.defense_badge_boosts != 0 {
                    for _ in 0..stage_modifiers.defense_badge_boosts.max(0) {
                        result.defense = badge_boost_single_stat(gen, result.defense);
                    }
                }
            }

            let spe_badge = badges.map(|b| b.is_speed_boosted()).unwrap_or(false);
            result.speed = calc_battle_stat_gen12(gen, base.speed, level, dv.speed, stat_xp.speed, stage_modifiers.speed_stage, spe_badge);
            if let Some(b) = badges {
                if b.has("soul") && stage_modifiers.speed_badge_boosts != 0 {
                    for _ in 0..stage_modifiers.speed_badge_boosts.max(0) {
                        result.speed = badge_boost_single_stat(gen, result.speed);
                    }
                }
            }

            let spa_badge = badges.map(|b| b.is_special_attack_boosted()).unwrap_or(false);
            result.special_attack = calc_battle_stat_gen12(
                gen,
                base.special_attack,
                level,
                dv.special_attack,
                stat_xp.special_attack,
                stage_modifiers.special_attack_stage,
                spa_badge,
            );
            if let Some(b) = badges {
                if b.has("volcano") && stage_modifiers.special_badge_boosts != 0 {
                    for _ in 0..stage_modifiers.special_badge_boosts.max(0) {
                        result.special_attack = badge_boost_single_stat(gen, result.special_attack);
                    }
                }
            }
            result.special_defense = result.special_attack;
            result
        }
        Gen::Two => {
            let badge_holder;
            let badges = if is_crit {
                badge_holder = badges.map(|b| b.empty_like());
                badge_holder.as_ref()
            } else {
                badges
            };
            let hp = calc_stat_gen12(base.hp, level, dv.hp, stat_xp.hp, true, false);
            let mut result = StatBlock::new(gen, hp, 0, 0, 0, 0, 0, false);
            result.attack = calc_battle_stat_gen12(
                gen,
                base.attack,
                level,
                dv.attack,
                stat_xp.attack,
                stage_modifiers.attack_stage,
                badges.map(|b| b.is_attack_boosted()).unwrap_or(false),
            );
            result.defense = calc_battle_stat_gen12(
                gen,
                base.defense,
                level,
                dv.defense,
                stat_xp.defense,
                stage_modifiers.defense_stage,
                badges.map(|b| b.is_defense_boosted()).unwrap_or(false),
            );
            result.speed = calc_battle_stat_gen12(
                gen,
                base.speed,
                level,
                dv.speed,
                stat_xp.speed,
                stage_modifiers.speed_stage,
                badges.map(|b| b.is_speed_boosted()).unwrap_or(false),
            );
            result.special_attack = calc_battle_stat_gen12(
                gen,
                base.special_attack,
                level,
                dv.special_attack,
                stat_xp.special_attack,
                stage_modifiers.special_attack_stage,
                badges.map(|b| b.is_special_attack_boosted()).unwrap_or(false),
            );
            let unboosted_spa = calc_battle_stat_gen12(
                gen,
                base.special_attack,
                level,
                dv.special_attack,
                stat_xp.special_attack,
                stage_modifiers.special_attack_stage,
                false,
            );
            if should_ignore_spd_badge_boost(unboosted_spa) {
                result.special_defense = calc_battle_stat_gen12(
                    gen,
                    base.special_defense,
                    level,
                    dv.special_attack,
                    stat_xp.special_attack,
                    stage_modifiers.special_defense_stage,
                    false,
                );
            } else {
                result.special_defense = calc_battle_stat_gen12(
                    gen,
                    base.special_defense,
                    level,
                    dv.special_attack,
                    stat_xp.special_attack,
                    stage_modifiers.special_defense_stage,
                    badges.map(|b| b.is_special_defense_boosted()).unwrap_or(false),
                );
            }
            result
        }
        Gen::Three => {
            let badge_holder;
            let badges = if is_crit {
                badge_holder = badges.map(|b| b.empty_like());
                badge_holder.as_ref()
            } else {
                badges
            };
            let hp = calc_stat_gen345(base.hp, level, dv.hp, stat_xp.hp, true, false, false, false);
            let mut result = StatBlock::new(gen, hp, 0, 0, 0, 0, 0, false);
            result.attack = calc_battle_stat_gen3(
                base.attack,
                level,
                dv.attack,
                stat_xp.attack,
                stage_modifiers.attack_stage,
                badges.map(|b| b.is_attack_boosted()).unwrap_or(false),
                nature.is_stat_raised(consts::ATTACK),
                nature.is_stat_lowered(consts::ATTACK),
                false,
            );
            result.defense = calc_battle_stat_gen3(
                base.defense,
                level,
                dv.defense,
                stat_xp.defense,
                stage_modifiers.defense_stage,
                badges.map(|b| b.is_defense_boosted()).unwrap_or(false),
                nature.is_stat_raised(consts::DEFENSE),
                nature.is_stat_lowered(consts::DEFENSE),
                false,
            );
            result.speed = calc_battle_stat_gen3(
                base.speed,
                level,
                dv.speed,
                stat_xp.speed,
                stage_modifiers.speed_stage,
                badges.map(|b| b.is_speed_boosted()).unwrap_or(false),
                nature.is_stat_raised(consts::SPEED),
                nature.is_stat_lowered(consts::SPEED),
                held_item == Some(consts::MACHO_BRACE_ITEM_NAME),
            );
            result.special_attack = calc_battle_stat_gen3(
                base.special_attack,
                level,
                dv.special_attack,
                stat_xp.special_attack,
                stage_modifiers.special_attack_stage,
                badges.map(|b| b.is_special_attack_boosted()).unwrap_or(false),
                nature.is_stat_raised(consts::SPECIAL_ATTACK),
                nature.is_stat_lowered(consts::SPECIAL_ATTACK),
                false,
            );
            result.special_defense = calc_battle_stat_gen3(
                base.special_defense,
                level,
                dv.special_defense,
                stat_xp.special_defense,
                stage_modifiers.special_defense_stage,
                badges.map(|b| b.is_special_defense_boosted()).unwrap_or(false),
                nature.is_stat_raised(consts::SPECIAL_DEFENSE),
                nature.is_stat_lowered(consts::SPECIAL_DEFENSE),
                false,
            );
            result
        }
        Gen::Four | Gen::Five => {
            let default_field = FieldStatus::default();
            let field = field_status.unwrap_or(&default_field);
            let hp = calc_stat_gen345(base.hp, level, dv.hp, stat_xp.hp, true, false, false, false);
            let mut result = StatBlock::new(gen, hp, 0, 0, 0, 0, 0, false);
            result.attack = calc_battle_stat_gen45(
                gen,
                base.attack,
                level,
                dv.attack,
                stat_xp.attack,
                0,
                nature.is_stat_raised(consts::ATTACK),
                nature.is_stat_lowered(consts::ATTACK),
                false,
                false,
            );
            let def_stage = if gen == Gen::Four { 0 } else { stage_modifiers.defense_stage };
            result.defense = calc_battle_stat_gen45(
                gen,
                base.defense,
                level,
                dv.defense,
                stat_xp.defense,
                def_stage,
                nature.is_stat_raised(consts::DEFENSE),
                nature.is_stat_lowered(consts::DEFENSE),
                false,
                false,
            );
            let slowed = match held_item {
                Some(item) => {
                    consts::SPEED_SLOWING_ITEMS.contains(&item) || (gen == Gen::Four && item == "Iron Ball")
                }
                None => false,
            };
            result.speed = calc_battle_stat_gen45(
                gen,
                base.speed,
                level,
                dv.speed,
                stat_xp.speed,
                stage_modifiers.speed_stage,
                nature.is_stat_raised(consts::SPEED),
                nature.is_stat_lowered(consts::SPEED),
                slowed,
                held_item == Some(consts::CHOICE_SCARF_ITEM_NAME),
            );
            result.special_attack = calc_battle_stat_gen45(
                gen,
                base.special_attack,
                level,
                dv.special_attack,
                stat_xp.special_attack,
                0,
                nature.is_stat_raised(consts::SPECIAL_ATTACK),
                nature.is_stat_lowered(consts::SPECIAL_ATTACK),
                false,
                false,
            );
            result.special_defense = calc_battle_stat_gen45(
                gen,
                base.special_defense,
                level,
                dv.special_defense,
                stat_xp.special_defense,
                stage_modifiers.special_defense_stage,
                nature.is_stat_raised(consts::SPECIAL_DEFENSE),
                nature.is_stat_lowered(consts::SPECIAL_DEFENSE),
                false,
                false,
            );

            if gen == Gen::Four {
                if field.power_trick {
                    std::mem::swap(&mut result.attack, &mut result.defense);
                }
                if field.slow_start {
                    result.attack /= 2;
                    result.speed /= 2;
                }
                result.attack = modify_stat_by_stage(gen, result.attack, stage_modifiers.attack_stage);
                result.defense = modify_stat_by_stage(gen, result.defense, stage_modifiers.defense_stage);
                result.special_attack = modify_stat_by_stage(gen, result.special_attack, stage_modifiers.special_attack_stage);
            } else {
                if field.power_trick {
                    std::mem::swap(&mut result.attack, &mut result.special_attack);
                }
                if field.slow_start {
                    result.attack /= 2;
                    result.special_attack /= 2;
                    result.speed /= 2;
                }
                result.attack = modify_stat_by_stage(gen, result.attack, stage_modifiers.attack_stage);
                result.special_attack = modify_stat_by_stage(gen, result.special_attack, stage_modifiers.special_attack_stage);
            }
            if field.tailwind {
                result.speed *= 2;
            }
            if field.trick_room {
                result.speed = (10_000 - result.speed).rem_euclid(8_192);
            }
            result
        }
    }
}

/// `get_move_list` (identical in every gen).
pub fn get_move_list(initial_moves: &[String], learned_moves: &[(i64, String)], target_level: i64, special_moves: Option<&[Option<String>]>) -> Vec<Option<String>> {
    let mut result: Vec<Option<String>> = initial_moves.iter().cloned().map(Some).collect();
    for (lvl, mv) in learned_moves {
        if result.iter().any(|m| m.as_deref() == Some(mv.as_str())) {
            continue;
        }
        if target_level < *lvl {
            break;
        }
        result.push(Some(mv.clone()));
    }
    if result.len() > 4 {
        result = result[result.len() - 4..].to_vec();
    }
    if let Some(special) = special_moves {
        if !special.is_empty() {
            while result.len() < 4 {
                result.push(None);
            }
            for (idx, new_move) in special.iter().enumerate() {
                if let Some(m) = new_move {
                    if idx < result.len() {
                        result[idx] = Some(m.clone());
                    }
                }
            }
            result.retain(|m| m.as_deref().map(|s| !s.is_empty()).unwrap_or(false));
        }
    }
    result
}

/// `instantiate_trainer_pokemon` for every gen (`nature` is only meaningful in gens 3+).
pub fn instantiate_trainer_pokemon(gen: Gen, pkmn_data: &PokemonSpecies, target_level: i64, special_moves: Option<&[Option<String>]>, nature: Nature) -> EnemyPkmn {
    let stats = match gen {
        Gen::One | Gen::Two => StatBlock::new(
            gen,
            calc_stat_gen12(pkmn_data.stats.hp, target_level, 8, 0, true, false),
            calc_stat_gen12(pkmn_data.stats.attack, target_level, 9, 0, false, false),
            calc_stat_gen12(pkmn_data.stats.defense, target_level, 8, 0, false, false),
            calc_stat_gen12(pkmn_data.stats.special_attack, target_level, 8, 0, false, false),
            calc_stat_gen12(pkmn_data.stats.special_defense, target_level, 8, 0, false, false),
            calc_stat_gen12(pkmn_data.stats.speed, target_level, 8, 0, false, false),
            false,
        ),
        _ => StatBlock::new(
            gen,
            calc_stat_gen345(pkmn_data.stats.hp, target_level, 8, 0, true, false, false, false),
            calc_stat_gen345(pkmn_data.stats.attack, target_level, 9, 0, false, false, nature.is_stat_raised(consts::ATTACK), nature.is_stat_lowered(consts::ATTACK)),
            calc_stat_gen345(pkmn_data.stats.defense, target_level, 8, 0, false, false, nature.is_stat_raised(consts::DEFENSE), nature.is_stat_lowered(consts::DEFENSE)),
            calc_stat_gen345(pkmn_data.stats.special_attack, target_level, 8, 0, false, false, nature.is_stat_raised(consts::SPECIAL_ATTACK), nature.is_stat_lowered(consts::SPECIAL_ATTACK)),
            calc_stat_gen345(pkmn_data.stats.special_defense, target_level, 8, 0, false, false, nature.is_stat_raised(consts::SPECIAL_DEFENSE), nature.is_stat_lowered(consts::SPECIAL_DEFENSE)),
            calc_stat_gen345(pkmn_data.stats.speed, target_level, 8, 0, false, false, nature.is_stat_raised(consts::SPEED), nature.is_stat_lowered(consts::SPEED)),
            false,
        ),
    };
    EnemyPkmn {
        name: pkmn_data.name.clone(),
        level: target_level,
        xp: crate::exp::calc_xp_yield(pkmn_data.base_xp, target_level, true, 1),
        move_list: get_move_list(&pkmn_data.initial_moves, &pkmn_data.levelup_moves, target_level, special_moves),
        cur_stats: stats,
        base_stats: pkmn_data.stats,
        dvs: StatBlock::new(gen, 8, 9, 8, 8, 8, 8, false),
        stat_xp: StatBlock::zero(gen, true),
        badges: None,
        held_item: None,
        custom_move_data: None,
        is_trainer_mon: true,
        exp_split: 1,
        mon_order: 1,
        definition_order: 1,
        ability: String::new(),
        nature: Nature::HARDY,
    }
}

/// `instantiate_wild_pokemon`
pub fn instantiate_wild_pokemon(gen: Gen, pkmn_data: &PokemonSpecies, target_level: i64, dv: i64, nature: Nature) -> EnemyPkmn {
    let stats = match gen {
        Gen::One | Gen::Two => StatBlock::new(
            gen,
            calc_stat_gen12(pkmn_data.stats.hp, target_level, dv, 0, true, false),
            calc_stat_gen12(pkmn_data.stats.attack, target_level, dv, 0, false, false),
            calc_stat_gen12(pkmn_data.stats.defense, target_level, dv, 0, false, false),
            calc_stat_gen12(pkmn_data.stats.special_attack, target_level, dv, 0, false, false),
            calc_stat_gen12(pkmn_data.stats.special_defense, target_level, dv, 0, false, false),
            calc_stat_gen12(pkmn_data.stats.speed, target_level, dv, 0, false, false),
            false,
        ),
        _ => StatBlock::new(
            gen,
            calc_stat_gen345(pkmn_data.stats.hp, target_level, dv, 0, true, false, false, false),
            calc_stat_gen345(pkmn_data.stats.attack, target_level, dv, 0, false, false, nature.is_stat_raised(consts::ATTACK), nature.is_stat_lowered(consts::ATTACK)),
            calc_stat_gen345(pkmn_data.stats.defense, target_level, dv, 0, false, false, nature.is_stat_raised(consts::DEFENSE), nature.is_stat_lowered(consts::DEFENSE)),
            calc_stat_gen345(pkmn_data.stats.special_attack, target_level, dv, 0, false, false, nature.is_stat_raised(consts::SPECIAL_ATTACK), nature.is_stat_lowered(consts::SPECIAL_ATTACK)),
            calc_stat_gen345(pkmn_data.stats.special_defense, target_level, dv, 0, false, false, nature.is_stat_raised(consts::SPECIAL_DEFENSE), nature.is_stat_lowered(consts::SPECIAL_DEFENSE)),
            calc_stat_gen345(pkmn_data.stats.speed, target_level, dv, 0, false, false, nature.is_stat_raised(consts::SPEED), nature.is_stat_lowered(consts::SPEED)),
            false,
        ),
    };
    EnemyPkmn {
        name: pkmn_data.name.clone(),
        level: target_level,
        xp: crate::exp::calc_xp_yield(pkmn_data.base_xp, target_level, false, 1),
        move_list: get_move_list(&pkmn_data.initial_moves, &pkmn_data.levelup_moves, target_level, None),
        cur_stats: stats,
        base_stats: pkmn_data.stats,
        dvs: StatBlock::uniform(gen, dv),
        stat_xp: StatBlock::zero(gen, true),
        badges: None,
        held_item: None,
        custom_move_data: None,
        is_trainer_mon: false,
        exp_split: 1,
        mon_order: 1,
        definition_order: 1,
        ability: String::new(),
        nature: Nature::HARDY,
    }
}

const HIDDEN_POWER_TABLE: [&str; 16] = [
    consts::TYPE_FIGHTING,
    consts::TYPE_FLYING,
    consts::TYPE_POISON,
    consts::TYPE_GROUND,
    consts::TYPE_ROCK,
    consts::TYPE_BUG,
    consts::TYPE_GHOST,
    consts::TYPE_STEEL,
    consts::TYPE_FIRE,
    consts::TYPE_WATER,
    consts::TYPE_GRASS,
    consts::TYPE_ELECTRIC,
    consts::TYPE_PSYCHIC,
    consts::TYPE_ICE,
    consts::TYPE_DRAGON,
    consts::TYPE_DARK,
];

/// Gen 2 `get_hidden_power_type`
pub fn hidden_power_type_gen2(dvs: &StatBlock) -> &'static str {
    let mut idx = 4;
    idx *= dvs.attack.rem_euclid(4);
    idx += dvs.defense.rem_euclid(4);
    HIDDEN_POWER_TABLE[idx.clamp(0, 15) as usize]
}

/// Gen 2 `get_hidden_power_base_power`
pub fn hidden_power_base_power_gen2(dvs: &StatBlock) -> i64 {
    let mut result = 5 * if dvs.special_attack >= 8 { 1 } else { 0 };
    result += dvs.special_attack.rem_euclid(4);
    result = floor_div(result, 2);
    result += 5 * if dvs.speed >= 8 { 1 } else { 0 };
    result += 10 * if dvs.defense >= 8 { 1 } else { 0 };
    result += 20 * if dvs.attack >= 8 { 1 } else { 0 };
    result + 31
}

/// Gens 3-5 `get_hidden_power_type`
pub fn hidden_power_type_gen345(dvs: &StatBlock) -> &'static str {
    let mut idx = dvs.hp.rem_euclid(2);
    idx += dvs.attack.rem_euclid(2) * 2;
    idx += dvs.defense.rem_euclid(2) * 4;
    idx += dvs.speed.rem_euclid(2) * 8;
    idx += dvs.special_attack.rem_euclid(2) * 16;
    idx += dvs.special_defense.rem_euclid(2) * 32;
    idx *= 15;
    idx = floor_div(idx, 63);
    HIDDEN_POWER_TABLE[idx.clamp(0, 15) as usize]
}

fn power_bit(v: i64) -> i64 {
    if v.rem_euclid(4) >= 2 {
        1
    } else {
        0
    }
}

/// Gens 3-5 `get_hidden_power_base_power`
pub fn hidden_power_base_power_gen345(dvs: &StatBlock) -> i64 {
    let mut result = power_bit(dvs.hp);
    result += power_bit(dvs.attack) * 2;
    result += power_bit(dvs.defense) * 4;
    result += power_bit(dvs.speed) * 8;
    result += power_bit(dvs.special_attack) * 16;
    result += power_bit(dvs.special_defense) * 32;
    result *= 40;
    result = floor_div(result, 63);
    result + 30
}
