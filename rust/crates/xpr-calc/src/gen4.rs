//! Gen 4 (Diamond / Pearl / Platinum / HeartGold / SoulSilver) damage,
//! critical-hit and accuracy formulas, verified against pokeplatinum
//! (`src/battle/battle_lib.c` `BattleSystem_CalcMoveDamage`,
//! `BattleSystem_ApplyTypeChart`, `BattleSystem_CalcCriticalMulti`;
//! `src/battle/battle_script.c` `BattleScript_CalcMoveDamage` and the
//! variable-power `BtlCmd_*` commands; `src/battle/battle_controller_player.c`
//! `BattleControllerPlayer_CheckMoveHitAccuracy`) and cross-checked against
//! pokeheartgold. The modifier order below is the game's.

use xpr_core::consts;
use xpr_core::{floor_div, floor_mul_ratio};
use xpr_data::gen_consts as gc;
use xpr_data::model::{FieldStatus, StageModifiers};
use xpr_data::stats;
use xpr_data::GenData;

use crate::damage::{self, py_int, DamageRange, HitModel};
use crate::DamageArgs;

const MIN_RANGE: i64 = 85;
const MAX_RANGE: i64 = 100;

const TRIPLE_KICK_SENTINEL_PREFIX: &str = "__tk_hit_";

fn crit_stage_rate(stage: i64) -> f64 {
    match stage {
        0 => 1.0 / 16.0,
        1 => 1.0 / 8.0,
        2 => 1.0 / 4.0,
        3 => 1.0 / 3.0,
        _ => 1.0 / 2.0,
    }
}

/// Ability after Worry Seed / Gastro Acid.
fn effective_ability<'a>(mon_ability: &'a str, field: &FieldStatus) -> &'a str {
    if field.worry_seed {
        gc::INSOMNIA
    } else if field.gastro_acid {
        ""
    } else {
        mon_ability
    }
}

/// `BattleSystem_Divide`: truncating division that never turns a non-zero
/// dividend into 0.
fn battle_divide(dividend: i64, divisor: i64) -> i64 {
    if dividend == 0 {
        return 0;
    }
    let q = dividend / divisor;
    if q == 0 {
        1
    } else {
        q
    }
}

/// Moves whose damage never goes through `ApplyTypeChart` at all (no STAB,
/// no effectiveness, no immunities): Struggle is typeless, Future Sight /
/// Doom Desire damage is stored at setup.
fn skips_type_chart(name: &str) -> bool {
    name == consts::STRUGGLE_MOVE_NAME || name == consts::FUTURE_SIGHT_MOVE_NAME || name == consts::DOOM_DESIRE_MOVE_NAME
}

/// Moves flagged `SYSCTL_IGNORE_TYPE_CHECKS`: immunities still apply but
/// STAB and the type multipliers do not.
fn ignores_type_multipliers(name: &str) -> bool {
    matches!(
        name,
        consts::DRAGON_RAGE_MOVE_NAME | "SonicBoom" | "Seismic Toss" | "Night Shade" | "Psywave" | "Counter" | "Mirror Coat" | "Metal Burst" | "Bide"
    )
}

/// `BattleSystem_CalcCriticalMulti`: the crit stage of one hit (before the
/// defender's Battle Armor / Shell Armor cancel it).
pub fn get_crit_rate(a: &DamageArgs, _custom: Option<&str>) -> f64 {
    let mon = a.attacking;
    let mv = a.mv;
    let default_field = FieldStatus::default();
    let attacking_field = a.attacking_field.unwrap_or(&default_field);
    let defending_field = a.defending_field.unwrap_or(&default_field);
    let ability = effective_ability(&mon.ability, attacking_field);
    let defending_ability = effective_ability(&a.defending.ability, defending_field);
    if defending_ability == gc::BATTLE_ARMOR || defending_ability == gc::SHELL_ARMOR {
        return 0.0;
    }
    if mv.name == consts::FUTURE_SIGHT_MOVE_NAME || mv.name == consts::DOOM_DESIRE_MOVE_NAME {
        return 0.0;
    }
    if gc::OHKO_MOVE_NAMES.contains(&mv.name.as_str()) || ignores_type_multipliers(&mv.name) || mv.name == gc::SUPER_FANG_MOVE || mv.name == gc::ENDEAVOR_MOVE {
        return 0.0;
    }
    let mut stage = 0;
    if mv.has_flavor(consts::FLAVOR_HIGH_CRIT) {
        stage += 1;
    }
    if ability == gc::SUPER_LUCK {
        stage += 1;
    }
    let held = if ability == gc::KLUTZ { "" } else { mon.held_item_str() };
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

/// `BattleControllerPlayer_CheckMoveHitAccuracy` + `..._CheckMoveHitOverrides`
/// (and `BtlCmd_TryOHKOMove` for the OHKO moves): hit chance in percent,
/// `None` when the move cannot miss.
pub fn get_move_accuracy(a: &DamageArgs, custom: &str) -> Option<f64> {
    let pkmn = a.attacking;
    let defending = a.defending;
    let mv = a.mv;
    let weather = a.weather;
    let default_field = FieldStatus::default();
    let attacking_field = a.attacking_field.unwrap_or(&default_field);
    let defending_field = a.defending_field.unwrap_or(&default_field);
    let ability = effective_ability(&pkmn.ability, attacking_field);
    let defending_ability = effective_ability(&defending.ability, defending_field);
    let weather_active = damage::is_weather_active(ability, defending_ability, weather);

    if ability == gc::NO_GUARD || defending_ability == gc::NO_GUARD {
        return None;
    }
    if gc::OHKO_MOVE_NAMES.contains(&mv.name.as_str()) {
        // `BtlCmd_TryOHKOMove`: `accuracy + (Latk - Ldef)` out of 100, no
        // other modifier; fails outright against a higher level.
        if pkmn.level < defending.level {
            return Some(0.0);
        }
        return Some((mv.accuracy.unwrap_or(0) + (pkmn.level - defending.level)).min(100) as f64);
    }
    let mut result: Option<i64> = if mv.name == gc::NATURE_POWER_MOVE {
        Some(gc::gen4_nature_power(custom).map(|(_, _, acc)| acc).unwrap_or(100))
    } else {
        mv.accuracy
    };
    if weather_active {
        if mv.name == gc::BLIZZARD_MOVE && weather == consts::WEATHER_HAIL {
            return None;
        }
        if mv.name == gc::THUNDER_MOVE && weather == consts::WEATHER_RAIN {
            return None;
        }
        if mv.name == gc::THUNDER_MOVE && weather == consts::WEATHER_SUN {
            result = Some(50);
        }
    }
    let mut r = result?;

    // accuracy / evasion stages: sum clamped to 0..=12
    let default_stages = StageModifiers::default();
    let acc_stage = a.attacking_stages.unwrap_or(&default_stages).accuracy_stage;
    let eva_stage = a.defending_stages.unwrap_or(&default_stages).evasion_stage;
    let sum = (acc_stage - eva_stage + 6).clamp(0, 12);
    let (num, den) = gc::ACCURACY_STAGE_RATIOS[sum as usize];
    r = r * num / den;

    if ability == gc::COMPOUND_EYES {
        r = r * 130 / 100;
    }
    if weather_active {
        if defending_ability == gc::SAND_VEIL && weather == consts::WEATHER_SANDSTORM {
            r = r * 80 / 100;
        }
        if defending_ability == gc::SNOW_CLOAK && weather == consts::WEATHER_HAIL {
            r = r * 80 / 100;
        }
        if weather == consts::WEATHER_FOG {
            r = r * 6 / 10;
        }
    }
    if ability == gc::HUSTLE {
        let mut is_physical = mv.category == consts::CATEGORY_PHYSICAL;
        if mv.name == gc::NATURE_POWER_MOVE {
            is_physical = gc::GEN4_NATURE_POWER_PHYSICAL_TERRAINS.contains(&custom);
        }
        if is_physical {
            r = r * 80 / 100;
        }
    }
    let dheld = if defending_ability == gc::KLUTZ { "" } else { defending.held_item_str() };
    if dheld == gc::BRIGHT_POWDER || dheld == gc::LAX_INCENSE {
        r = r * 90 / 100;
    }
    let held = if ability == gc::KLUTZ { "" } else { pkmn.held_item_str() };
    if held == gc::WIDE_LENS {
        r = r * 110 / 100;
    }
    if attacking_field.gravity || defending_field.gravity {
        r = r * 10 / 6;
    }
    Some(r.min(100) as f64)
}

/// `BattleSystem_CalcMoveDamage` for one Beat Up hit: base Attack of the
/// party member, base Defense of the target, no STAB / chart / items.
fn beat_up_hit(gen: &GenData, a: &DamageArgs, crit: bool) -> Option<DamageRange> {
    let attacking_species = gen.pkmn_db().get_pkmn(&a.attacking.name)?;
    let defending_species = gen.pkmn_db().get_pkmn(&a.defending.name)?;
    let default_field = FieldStatus::default();
    let attacking_field = a.attacking_field.unwrap_or(&default_field);
    let defending_field = a.defending_field.unwrap_or(&default_field);
    let attacking_ability = effective_ability(&a.attacking.ability, attacking_field);
    let defending_ability = effective_ability(&a.defending.ability, defending_field);
    let mut dmg = attacking_species.stats.attack * a.mv.base_power.unwrap_or(10);
    dmg *= floor_div(a.attacking.level * 2, 5) + 2;
    dmg = floor_div(dmg, defending_species.stats.defense.max(1));
    dmg = floor_div(dmg, 50) + 2;
    if crit && defending_ability != gc::BATTLE_ARMOR && defending_ability != gc::SHELL_ARMOR {
        dmg *= if attacking_ability == gc::SNIPER { 3 } else { 2 };
    }
    Some(DamageRange::from_rolls(dmg, MIN_RANGE, MAX_RANGE))
}

fn beat_up_hits(custom: &str) -> i64 {
    py_int(custom).unwrap_or(1).clamp(1, 6)
}

fn triple_kick_kicks(custom: &str) -> i64 {
    py_int(custom).unwrap_or(1).clamp(1, 3)
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

fn with_crit<'a>(a: &DamageArgs<'a>, is_crit: bool) -> DamageArgs<'a> {
    let mut sub = *a;
    sub.is_crit = is_crit;
    sub
}

/// The per-hit ranges of a multi-hit use (multi-hit / two-hit moves, Beat
/// Up, Triple Kick); `None` for single hits.
pub fn hit_model(gen: &GenData, a: &DamageArgs) -> Option<HitModel> {
    let mv = a.mv;
    let custom = a.custom_move_data;
    if mv.name == gc::TRIPLE_KICK_MOVE && !custom.starts_with(TRIPLE_KICK_SENTINEL_PREFIX) {
        let kicks = triple_kick_kicks(custom);
        if kicks < 2 {
            return None;
        }
        let mut hits = Vec::new();
        for kick in 1..=kicks {
            let sentinel = format!("{}{}__", TRIPLE_KICK_SENTINEL_PREFIX, kick * 10);
            let mut sub = *a;
            sub.custom_move_data = &sentinel;
            sub.is_crit = false;
            let normal = calculate_damage_impl(gen, &sub, true)?;
            sub.is_crit = true;
            let crit = calculate_damage_impl(gen, &sub, true)?;
            hits.push((normal, crit));
        }
        return Some(HitModel { hits, accuracy_per_hit: true });
    }
    if mv.name == gc::BEAT_UP_MOVE {
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

/// `calculate_gen_four_damage`: the whole use (every hit of a multi-hit move).
pub fn calculate_damage(gen: &GenData, a: &DamageArgs) -> Option<DamageRange> {
    calculate_damage_impl(gen, a, false)
}

fn calculate_damage_impl(gen: &GenData, a: &DamageArgs, single_hit: bool) -> Option<DamageRange> {
    let mv = a.mv;
    let attacking_pkmn = a.attacking;
    let defending_pkmn = a.defending;
    let custom = a.custom_move_data;
    let weather = a.weather;

    let default_field = FieldStatus::default();
    let attacking_field = a.attacking_field.unwrap_or(&default_field);
    let defending_field = a.defending_field.unwrap_or(&default_field);

    let attacking_ability = effective_ability(&attacking_pkmn.ability, attacking_field);
    let defending_ability = effective_ability(&defending_pkmn.ability, defending_field);
    // Klutz: the holder's item has no effect
    let held: &str = if attacking_ability == gc::KLUTZ { "" } else { attacking_pkmn.held_item_str() };
    let dheld: &str = if defending_ability == gc::KLUTZ { "" } else { defending_pkmn.held_item_str() };

    let default_stages = StageModifiers::default();
    let attacking_stages = *a.attacking_stages.unwrap_or(&default_stages);
    let defending_stages = *a.defending_stages.unwrap_or(&default_stages);

    let attacking_species = gen.pkmn_db().get_pkmn(&attacking_pkmn.name)?;
    let defending_species = gen.pkmn_db().get_pkmn(&defending_pkmn.name)?;

    let is_weather_active = damage::is_weather_active(attacking_ability, defending_ability, weather);
    let (a1, a2) = damage::apply_forecast(&attacking_species.first_type, &attacking_species.second_type, attacking_ability, weather, is_weather_active);
    let (d1, d2) = damage::apply_forecast(&defending_species.first_type, &defending_species.second_type, defending_ability, weather, is_weather_active);
    let mut attacking_first = a1;
    let mut attacking_second = a2;
    if attacking_ability == gc::MULTITYPE {
        if let Some(t) = gc::plate_type(Some(held)) {
            attacking_first = t.to_string();
            attacking_second = t.to_string();
        }
    }

    // ---- move type / power resolution (script level) ----
    let (mut move_type, mut base_power): (String, Option<i64>) = if mv.name == consts::HIDDEN_POWER_MOVE_NAME {
        (
            stats::hidden_power_type_gen345(&attacking_pkmn.dvs).to_string(),
            Some(stats::hidden_power_base_power_gen345(&attacking_pkmn.dvs)),
        )
    } else {
        (mv.move_type.clone(), mv.base_power)
    };
    let mut category = mv.category.clone();
    let mut targeting = mv.targeting.clone();
    let mut move_name = mv.name.as_str();

    if mv.name == gc::TRIPLE_KICK_MOVE && !custom.starts_with(TRIPLE_KICK_SENTINEL_PREFIX) {
        // hits of power 10 / 20 / 30, each with its own roll; the crit
        // range doubles the last kick only
        let kicks = triple_kick_kicks(custom);
        let mut combined: Option<DamageRange> = None;
        for kick in 1..=kicks {
            let sentinel = format!("{}{}__", TRIPLE_KICK_SENTINEL_PREFIX, kick * 10);
            let mut sub = *a;
            sub.custom_move_data = &sentinel;
            sub.is_crit = a.is_crit && kick == kicks;
            if let Some(kick_range) = calculate_damage_impl(gen, &sub, true) {
                combined = Some(match combined {
                    None => kick_range,
                    Some(c) => c.add(&kick_range),
                });
            }
        }
        return combined;
    }

    if mv.name == gc::TRIPLE_KICK_MOVE {
        let inner = &custom[TRIPLE_KICK_SENTINEL_PREFIX.len()..custom.len() - 2];
        base_power = py_int(inner);
    } else if mv.name == gc::NATURE_POWER_MOVE {
        let (bp, ty, _) = gc::gen4_nature_power(custom)?;
        let (called, cat, tgt) = gc::gen4_nature_power_move(custom)?;
        base_power = Some(bp);
        move_type = ty.to_string();
        category = cat.to_string();
        targeting = tgt.to_string();
        move_name = called;
    } else if mv.name == consts::WEATHER_BALL_MOVE_NAME && is_weather_active {
        base_power = base_power.map(|b| b * 2);
        move_type = damage::get_weather_ball_type(weather, is_weather_active, &move_type).to_string();
    } else if mv.name == consts::NATURAL_GIFT_MOVE_NAME {
        if held.is_empty() {
            return None;
        }
        let (bp, ty) = gc::natural_gift(xpr_data::Gen::Four, Some(held))?;
        base_power = Some(bp);
        move_type = ty.to_string();
    } else if mv.name == gc::FLING_MOVE {
        if held.is_empty() || attacking_ability == gc::MULTITYPE || held == gc::GRISEOUS_ORB {
            return None;
        }
        base_power = Some(*gc::GEN4_FLING_POWER.get(held)?);
    } else if mv.name == gc::PRESENT_MOVE {
        if custom == gc::HEAL_OPTION {
            return None;
        }
        base_power = Some(py_int(custom).unwrap_or(40));
    }

    if attacking_ability == gc::NORMALIZE {
        move_type = consts::TYPE_NORMAL.to_string();
    }
    if mv.name == gc::JUDGMENT_MOVE {
        if let Some(t) = gc::plate_type(Some(held)) {
            move_type = t.to_string();
        }
    }
    let is_struggle = mv.name == consts::STRUGGLE_MOVE_NAME;
    let no_type_chart = skips_type_chart(&mv.name);
    let no_type_multipliers = no_type_chart || ignores_type_multipliers(&mv.name);

    // ---- immunities ----
    let defender_grounded = defending_field.gravity || defending_field.roost || dheld == gc::IRON_BALL;
    let is_scrappy_active = (d1 == consts::TYPE_GHOST || d2 == consts::TYPE_GHOST)
        && (move_type == consts::TYPE_NORMAL || move_type == consts::TYPE_FIGHTING)
        && attacking_ability == gc::SCRAPPY;
    let ignore_ground_immunity = (d1 == consts::TYPE_FLYING || d2 == consts::TYPE_FLYING) && move_type == consts::TYPE_GROUND && defender_grounded;
    let ignore_dark_immunity = (d1 == consts::TYPE_DARK || d2 == consts::TYPE_DARK) && move_type == consts::TYPE_PSYCHIC && defending_field.miracle_eye;

    if !no_type_chart {
        let immune = gen.effectiveness(&move_type, &d1) == Some(consts::IMMUNE) || gen.effectiveness(&move_type, &d2) == Some(consts::IMMUNE);
        if immune && !is_scrappy_active && !ignore_ground_immunity && !ignore_dark_immunity {
            return None;
        }
        // Levitate is suppressed by Gravity; Iron Ball grounds the holder
        let levitating = defending_ability == gc::LEVITATE && !defending_field.gravity && dheld != gc::IRON_BALL;
        if levitating && move_type == consts::TYPE_GROUND {
            return None;
        } else if defending_field.magnet_rise && move_type == consts::TYPE_GROUND && dheld != gc::IRON_BALL {
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
        } else if defending_ability == gc::WONDER_GUARD && base_power.unwrap_or(0) != 0 {
            // blocked unless the net effectiveness is super effective (the
            // fixed-damage and OHKO moves carry power 1 in the move table)
            if !wonder_guard_passes(gen, &move_type, &d1, &d2, is_scrappy_active, ignore_ground_immunity, ignore_dark_immunity) {
                return None;
            }
        }
    }

    // ---- fixed-damage moves ----
    match mv.name.as_str() {
        consts::DRAGON_RAGE_MOVE_NAME => return Some(DamageRange::single(40)),
        "SonicBoom" => return Some(DamageRange::single(20)),
        "Seismic Toss" | "Night Shade" => return Some(DamageRange::single(attacking_pkmn.level)),
        _ => {}
    }
    if mv.name == gc::ENDEAVOR_MOVE {
        let diff = defending_pkmn.cur_stats.hp - attacking_pkmn.cur_stats.hp;
        if diff <= 0 {
            return None;
        }
        return Some(DamageRange::single(diff));
    }
    if mv.name == gc::SUPER_FANG_MOVE {
        return Some(DamageRange::single(floor_div(defending_pkmn.cur_stats.hp, 2).max(1)));
    }
    if gc::OHKO_MOVE_NAMES.contains(&mv.name.as_str()) {
        if defending_ability == gc::STURDY {
            return None;
        }
        if attacking_pkmn.level < defending_pkmn.level {
            return None;
        }
        return Some(DamageRange::single(defending_pkmn.cur_stats.hp));
    }
    if mv.has_flavor(consts::FLAVOR_PSYWAVE) {
        let pairs: Vec<(i64, i64)> = (0..11).map(|r| (floor_div(attacking_pkmn.level * (r + 5), 10).max(1), 1)).collect();
        return DamageRange::from_pairs(&pairs, 1);
    }
    if mv.name == gc::COUNTER_MOVE || mv.name == gc::MIRROR_COAT_MOVE || mv.name == gc::BIDE_MOVE || mv.name == gc::METAL_BURST_MOVE {
        // damage taken from the dropdown
        let taken = py_int(custom)?;
        if taken <= 0 {
            return None;
        }
        let dealt = if mv.name == gc::METAL_BURST_MOVE { floor_mul_ratio(taken, 3, 2) } else { taken * 2 };
        return Some(DamageRange::single(dealt));
    }
    if mv.name == gc::BEAT_UP_MOVE {
        let n = if single_hit { 1 } else { beat_up_hits(custom) };
        let first = beat_up_hit(gen, a, a.is_crit)?;
        let mut total = first;
        if n > 1 {
            let other = beat_up_hit(gen, a, false)?;
            for _ in 1..n {
                total = total.add(&other);
            }
        }
        return Some(total);
    }

    // ---- script-level variable power ----
    let mut power_mul = 10;
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
        power_mul = 20;
    }
    let mut base_power = base_power.unwrap_or(0);
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
            power_mul = 20;
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
            base_power = floor_div(base_power * v, 100).max(1);
        }
    } else if mv.name == gc::CRUSH_GRIP_MOVE || mv.name == gc::WRING_OUT_MOVE {
        if let Some(v) = py_int(custom) {
            base_power = 1 + floor_div(120 * v, 100);
        }
    } else if mv.name == gc::GYRO_BALL_MOVE {
        // `monSpeedValues`: battle speeds without the Trick Room transform
        let mut af = *attacking_field;
        af.trick_room = false;
        let mut df = *defending_field;
        df.trick_room = false;
        let atk_speed = attacking_pkmn.get_battle_stats(&attacking_stages, false, Some(&af)).speed.max(1);
        let def_speed = defending_pkmn.get_battle_stats(&defending_stages, false, Some(&df)).speed;
        base_power = (1 + floor_div(25 * def_speed, atk_speed)).min(150);
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
        base_power *= 1i64 << (num_turns - 1);
        if curl {
            base_power *= 2;
        }
    } else if mv.name == gc::FURY_CUTTER_MOVE {
        let num_turns = py_int(custom).unwrap_or(1).clamp(1, 5);
        base_power *= 1i64 << (num_turns - 1);
    } else if mv.name == gc::SPIT_UP_MOVE {
        base_power = 100 * py_int(custom).unwrap_or(1);
    } else if mv.name == gc::PUNISHMENT_MOVE {
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
    if gc::GEN4_RECKLESS_MOVES.contains(&mv.name.as_str()) && attacking_ability == gc::RECKLESS {
        power_mul = 12;
    }
    if base_power == 0 {
        return None;
    }

    // ---- BattleSystem_CalcMoveDamage: power chain ----
    let mut power = floor_div(base_power * power_mul, 10);
    if attacking_ability == gc::TECHNICIAN && !is_struggle && power <= 60 {
        power = floor_mul_ratio(power, 3, 2);
    }

    // raw stats (no stages); a caller-supplied block (transformed mon) is
    // used as-is, stages included
    let override_attacker = a.attacking_battle_stats.is_some();
    let override_defender = a.defending_battle_stats.is_some();
    let mut atk = match a.attacking_battle_stats {
        Some(s) => *s,
        None => attacking_pkmn.get_battle_stats(&StageModifiers::default(), false, Some(attacking_field)),
    };
    let mut def = match a.defending_battle_stats {
        Some(s) => *s,
        None => defending_pkmn.get_battle_stats(&StageModifiers::default(), false, Some(defending_field)),
    };

    if attacking_ability == gc::HUGE_POWER || attacking_ability == gc::PURE_POWER {
        atk.attack *= 2;
    }
    if gen.held_item_boosts.get(held).map(|s| s.as_str()) == Some(move_type.as_str()) {
        power = floor_mul_ratio(power, 6, 5);
    }
    if held == gc::CHOICE_BAND {
        atk.attack = floor_mul_ratio(atk.attack, 3, 2);
    }
    if held == gc::CHOICE_SPECS {
        atk.special_attack = floor_mul_ratio(atk.special_attack, 3, 2);
    }
    if held == gc::SOUL_DEW && (attacking_pkmn.name == gc::LATIOS || attacking_pkmn.name == gc::LATIAS) {
        atk.special_attack = floor_mul_ratio(atk.special_attack, 3, 2);
    }
    if dheld == gc::SOUL_DEW && (defending_pkmn.name == gc::LATIOS || defending_pkmn.name == gc::LATIAS) {
        def.special_defense = floor_mul_ratio(def.special_defense, 3, 2);
    }
    if held == gc::DEEP_SEA_TOOTH && attacking_pkmn.name == gc::CLAMPERL {
        atk.special_attack *= 2;
    }
    if dheld == gc::DEEP_SEA_SCALE && defending_pkmn.name == gc::CLAMPERL {
        def.special_defense *= 2;
    }
    if held == gc::LIGHT_BALL && attacking_pkmn.name == gc::PIKACHU {
        power *= 2;
    }
    if dheld == gc::METAL_POWDER && defending_pkmn.name == gc::DITTO {
        def.defense *= 2;
    }
    if held == gc::THICK_CLUB && (attacking_pkmn.name == gc::GEN2_CUBONE || attacking_pkmn.name == gc::GEN2_MAROWAK) {
        atk.attack *= 2;
    }
    if (held == gc::ADAMANT_ORB && attacking_pkmn.name == gc::DIALGA && (move_type == consts::TYPE_DRAGON || move_type == consts::TYPE_STEEL))
        || (held == gc::LUSTROUS_ORB && attacking_pkmn.name == gc::PALKIA && (move_type == consts::TYPE_DRAGON || move_type == consts::TYPE_WATER))
        || (held == gc::GRISEOUS_ORB && attacking_pkmn.name == gc::GIRATINA && (move_type == consts::TYPE_DRAGON || move_type == consts::TYPE_GHOST))
    {
        power = floor_mul_ratio(power, 6, 5);
    }
    if held == gc::MUSCLE_BAND && category == consts::CATEGORY_PHYSICAL {
        power = floor_mul_ratio(power, 11, 10);
    }
    if held == gc::WISE_GLASSES && category == consts::CATEGORY_SPECIAL {
        power = floor_mul_ratio(power, 11, 10);
    }
    if defending_ability == gc::THICK_FAT && (move_type == consts::TYPE_FIRE || move_type == consts::TYPE_ICE) {
        power = floor_div(power, 2);
    }
    if attacking_ability == gc::HUSTLE {
        atk.attack = floor_mul_ratio(atk.attack, 3, 2);
    }
    if move_type == consts::TYPE_FIRE && defending_ability == gc::HEATPROOF {
        power = floor_div(power, 2);
    }
    if move_type == consts::TYPE_FIRE && defending_ability == gc::DRY_SKIN {
        power = floor_mul_ratio(power, 5, 4);
    }
    if gc::GEN4_PUNCH_MOVES.contains(&mv.name.as_str()) && attacking_ability == gc::IRON_FIST {
        power = floor_mul_ratio(power, 6, 5);
    }
    if is_weather_active {
        if weather == consts::WEATHER_SUN && attacking_ability == gc::SOLAR_POWER {
            atk.special_attack = floor_mul_ratio(atk.special_attack, 3, 2);
        }
        if weather == consts::WEATHER_SANDSTORM && (d1 == consts::TYPE_ROCK || d2 == consts::TYPE_ROCK) {
            def.special_defense = floor_mul_ratio(def.special_defense, 3, 2);
        }
        if weather == consts::WEATHER_SUN && attacking_ability == gc::FLOWER_GIFT {
            atk.attack = floor_mul_ratio(atk.attack, 3, 2);
        }
        if weather == consts::WEATHER_SUN && defending_ability == gc::FLOWER_GIFT {
            def.special_defense = floor_mul_ratio(def.special_defense, 3, 2);
        }
    }
    if mv.name == consts::EXPLOSION_MOVE_NAME || mv.name == consts::SELFDESTRUCT_MOVE_NAME {
        def.defense = floor_div(def.defense, 2);
    }

    // ---- crit multiplier (Future Sight / Doom Desire never crit) ----
    let crit_mul: i64 = if a.is_crit
        && mv.name != consts::FUTURE_SIGHT_MOVE_NAME
        && mv.name != consts::DOOM_DESIRE_MOVE_NAME
        && defending_ability != gc::BATTLE_ARMOR
        && defending_ability != gc::SHELL_ARMOR
    {
        if attacking_ability == gc::SNIPER {
            3
        } else {
            2
        }
    } else {
        1
    };

    let is_physical = category != consts::CATEGORY_SPECIAL;
    let (atk_raw, atk_stage, def_raw, def_stage, screen) = if is_physical {
        (atk.attack, attacking_stages.attack_stage, def.defense, defending_stages.defense_stage, defending_field.reflect)
    } else {
        (atk.special_attack, attacking_stages.special_attack_stage, def.special_defense, defending_stages.special_defense_stage, defending_field.light_screen)
    };
    let attacking_stat = if override_attacker || (crit_mul > 1 && atk_stage <= 0) {
        atk_raw
    } else {
        stats::modify_stat_by_stage(xpr_data::Gen::Four, atk_raw, atk_stage)
    };
    let defending_stat = if override_defender || (crit_mul > 1 && def_stage >= 0) {
        def_raw
    } else {
        stats::modify_stat_by_stage(xpr_data::Gen::Four, def_raw, def_stage)
    };
    if defending_stat == 0 {
        return None;
    }

    let mut temp = attacking_stat * power * (floor_div(attacking_pkmn.level * 2, 5) + 2);
    temp = floor_div(temp, defending_stat);
    temp = floor_div(temp, 50);

    if screen && crit_mul == 1 && move_name != gc::BRICK_BREAK_MOVE {
        if a.is_double_battle {
            temp = floor_div(temp * 2, 3);
        } else {
            temp = floor_div(temp, 2);
        }
    }
    if a.is_double_battle && (targeting == gc::GEN4_TARGETING_ALL_FOES || targeting == gc::GEN4_TARGETING_OTHERS) {
        temp = floor_div(temp * 3, 4);
    }

    if is_weather_active {
        if weather == consts::WEATHER_RAIN {
            if move_type == consts::TYPE_FIRE {
                temp = floor_div(temp, 2);
            } else if move_type == consts::TYPE_WATER {
                temp = floor_mul_ratio(temp, 3, 2);
            }
        }
        if weather != consts::WEATHER_SUN && mv.name == consts::SOLAR_BEAM_MOVE_NAME {
            temp = floor_div(temp, 2);
        }
        if weather == consts::WEATHER_SUN {
            if move_type == consts::TYPE_FIRE {
                temp = floor_mul_ratio(temp, 3, 2);
            } else if move_type == consts::TYPE_WATER {
                temp = floor_div(temp, 2);
            }
        }
    }

    temp += 2;

    // ---- BattleScript_CalcMoveDamage ----
    temp *= crit_mul;
    if held == gc::LIFE_ORB {
        temp = floor_mul_ratio(temp, 13, 10);
    }

    // ---- BattleSystem_ApplyTypeChart ----
    if !no_type_chart {
        if !no_type_multipliers && (attacking_first == move_type || attacking_second == move_type) {
            if attacking_ability == gc::ADAPTABILITY {
                temp *= 2;
            } else {
                temp = floor_mul_ratio(temp, 3, 2);
            }
        }
        let (mult, se, nve) = net_effectiveness(gen, &move_type, &d1, &d2, is_scrappy_active, ignore_ground_immunity, ignore_dark_immunity);
        if !no_type_multipliers {
            for m in mult {
                temp = battle_divide(temp * m, 10);
            }
            if se {
                if defending_ability == gc::FILTER || defending_ability == gc::SOLID_ROCK {
                    temp = battle_divide(temp * 3, 4);
                }
                if held == gc::EXPERT_BELT {
                    temp = floor_mul_ratio(temp, 6, 5);
                }
            }
            if nve && attacking_ability == gc::TINTED_LENS {
                temp *= 2;
            }
        }
    }

    if temp <= 0 {
        temp = 1;
    }

    if mv.name == gc::SPIT_UP_MOVE {
        return Some(DamageRange::single(temp));
    }
    let mut result = DamageRange::from_rolls(temp, MIN_RANGE, MAX_RANGE);
    let multi_hit = if single_hit { 1 } else { multi_hit_count(a) };
    if multi_hit > 1 {
        // every hit rolls on its own; the crit range doubles one hit
        let other = if a.is_crit {
            calculate_damage_impl(gen, &with_crit(a, false), true)?
        } else {
            result.clone()
        };
        for _ in 1..multi_hit {
            result = result.add(&other);
        }
    }
    Some(result)
}

/// The type-chart multipliers that apply, in table order, and the net
/// super-effective / not-very-effective flags (`ApplyTypeMultiplier`: a
/// later NVE cancels an SE flag and vice versa).
fn net_effectiveness(
    gen: &GenData,
    move_type: &str,
    d1: &str,
    d2: &str,
    scrappy: bool,
    ignore_ground_immunity: bool,
    ignore_dark_immunity: bool,
) -> (Vec<i64>, bool, bool) {
    let mut mult = Vec::new();
    let mut se = false;
    let mut nve = false;
    if let Some(row) = gen.type_chart.get(move_type) {
        for (test_type, eff) in row {
            if test_type != d1 && test_type != d2 {
                continue;
            }
            let m = if eff == consts::SUPER_EFFECTIVE {
                20
            } else if eff == consts::NOT_VERY_EFFECTIVE {
                5
            } else if eff == consts::IMMUNE {
                if (test_type == consts::TYPE_GHOST && scrappy)
                    || (test_type == consts::TYPE_FLYING && ignore_ground_immunity)
                    || (test_type == consts::TYPE_DARK && ignore_dark_immunity)
                {
                    continue;
                }
                0
            } else {
                continue;
            };
            mult.push(m);
            if m == 20 {
                if nve {
                    nve = false;
                } else {
                    se = true;
                }
            } else if m == 5 {
                if se {
                    se = false;
                } else {
                    nve = true;
                }
            } else {
                se = false;
                nve = false;
            }
        }
    }
    (mult, se, nve)
}

fn wonder_guard_passes(gen: &GenData, move_type: &str, d1: &str, d2: &str, scrappy: bool, ignore_ground: bool, ignore_dark: bool) -> bool {
    let (_, se, _) = net_effectiveness(gen, move_type, d1, d2, scrappy, ignore_ground, ignore_dark);
    se
}
