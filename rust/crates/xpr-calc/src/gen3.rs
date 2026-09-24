//! Gen 3 (Ruby / Sapphire / Emerald / FireRed / LeafGreen) damage,
//! critical-hit and accuracy formulas, verified against pokeemerald
//! (`src/pokemon.c` `CalculateBaseDamage`; `src/battle_script_commands.c`
//! `Cmd_critcalc`, `Cmd_damagecalc`, `Cmd_typecalc`, `Cmd_accuracycheck`,
//! `ApplyRandomDmgMultiplier` and the variable-power commands) and diffed
//! against pokeruby / pokefirered. The modifier order below is the game's.

use xpr_core::consts;
use xpr_core::{floor_div, floor_mul_ratio};
use xpr_data::gen_consts as gc;
use xpr_data::model::{EnemyPkmn, Gen, Move, StageModifiers};
use xpr_data::stats;
use xpr_data::GenData;

use crate::damage::{self, py_int, DamageRange, HitModel};
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

/// Moves whose scripts never run `critcalc`.
fn never_crits(name: &str) -> bool {
    matches!(
        name,
        consts::SPIT_UP_MOVE_NAME
            | consts::FUTURE_SIGHT_MOVE_NAME
            | consts::DOOM_DESIRE_MOVE_NAME
            | consts::DRAGON_RAGE_MOVE_NAME
            | "SonicBoom"
            | "Seismic Toss"
            | "Night Shade"
            | "Psywave"
            | "Super Fang"
            | "Endeavor"
            | "Counter"
            | "Mirror Coat"
            | "Bide"
            | "Guillotine"
            | "Horn Drill"
            | "Fissure"
            | "Sheer Cold"
    )
}

/// `Cmd_critcalc`: stage = 2*FocusEnergy + high-crit effect + Scope Lens
/// + 2*(Lucky Punch on Chansey) + 2*(Stick on Farfetch'd), capped at 4;
/// 0 against Battle Armor / Shell Armor.
pub fn get_crit_rate(mon: &EnemyPkmn, mv: &Move, custom: Option<&str>, defending: &EnemyPkmn) -> f64 {
    if defending.ability == gc::BATTLE_ARMOR || defending.ability == gc::SHELL_ARMOR {
        return 0.0;
    }
    if never_crits(&mv.name) {
        return 0.0;
    }
    let mut stage = 0;
    if mv.has_flavor(consts::FLAVOR_HIGH_CRIT) {
        stage += 1;
    }
    if mv.name == gc::NATURE_POWER_MOVE && custom.map(|c| c.contains(gc::LONG_GRASS_TERRAIN)).unwrap_or(false) {
        stage += 1;
    }
    let held = mon.held_item_str();
    if held == gc::SCOPE_LENS {
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

/// `Cmd_accuracycheck` (+ `AccuracyCalcHelper`, `Cmd_tryKO`): hit chance in
/// percent, `None` when the move cannot miss.
pub fn get_move_accuracy(gen: &GenData, a: &DamageArgs, custom: &str) -> Option<f64> {
    let pkmn = a.attacking;
    let defending = a.defending;
    let mv = a.mv;
    let weather = a.weather;
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
    let move_acc = result?;

    // `buff = accStage + 6 - evasionStage`, clamped to 0..=12
    let default_stages = StageModifiers::default();
    let acc_stage = a.attacking_stages.unwrap_or(&default_stages).accuracy_stage;
    let eva_stage = a.defending_stages.unwrap_or(&default_stages).evasion_stage;
    let buff = (acc_stage - eva_stage + 6).clamp(0, 12);
    let (num, den) = gc::ACCURACY_STAGE_RATIOS[buff as usize];
    let mut r = floor_div(num * move_acc, den);

    if pkmn.ability == gc::COMPOUND_EYES {
        r = floor_div(r * 130, 100);
    }
    if weather_has_effect && defending.ability == gc::SAND_VEIL && weather == consts::WEATHER_SANDSTORM {
        r = floor_div(r * 80, 100);
    }
    if pkmn.ability == gc::HUSTLE {
        let is_physical = if mv.name == gc::NATURE_POWER_MOVE {
            [gc::PLAIN_TERRAIN, gc::SAND_TERRAIN, gc::CAVE_TERRAIN, gc::ROCK_TERRAIN]
                .iter()
                .any(|t| custom.contains(t))
        } else if mv.name == consts::HIDDEN_POWER_MOVE_NAME {
            !gen.special_types.iter().any(|t| t == stats::hidden_power_type_gen345(&pkmn.dvs))
        } else if mv.name == consts::WEATHER_BALL_MOVE_NAME && weather_has_effect {
            !gen.special_types.iter().any(|t| t == damage::get_weather_ball_type(weather, true, &mv.move_type))
        } else {
            !gen.special_types.contains(&mv.move_type)
        };
        if is_physical {
            r = floor_div(r * 80, 100);
        }
    }
    let dheld = defending.held_item_str();
    if dheld == gc::BRIGHT_POWDER {
        r = floor_div(r * 90, 100);
    } else if dheld == gc::LAX_INCENSE {
        r = floor_div(r * 95, 100);
    }
    Some(r.min(100) as f64)
}

/// `Cmd_typecalc` Wonder Guard rule: blocked unless the net result is super
/// effective (an SE and an NVE entry cancel each other).
fn wonder_guard_blocks(gen: &GenData, move_type: &str, d1: &str, d2: &str) -> bool {
    let mut se = false;
    let mut nve = false;
    if let Some(row) = gen.type_chart.get(move_type) {
        for (test_type, eff) in row {
            if test_type != d1 && test_type != d2 {
                continue;
            }
            if eff == consts::SUPER_EFFECTIVE {
                if nve {
                    nve = false;
                } else {
                    se = true;
                }
            } else if eff == consts::NOT_VERY_EFFECTIVE {
                if se {
                    se = false;
                } else {
                    nve = true;
                }
            } else if eff == consts::IMMUNE {
                return true;
            }
        }
    }
    !se
}

fn beat_up_hits(custom: &str) -> i64 {
    py_int(custom).unwrap_or(1).clamp(1, 6)
}

/// `Cmd_trydobeatup`: one hit from the party member's base Attack against
/// the target's base Defense; no STAB, chart, stats, items or screens.
fn beat_up_hit(gen: &GenData, a: &DamageArgs, crit: bool) -> Option<DamageRange> {
    let attacking_species = gen.pkmn_db().get_pkmn(&a.attacking.name)?;
    let defending_species = gen.pkmn_db().get_pkmn(&a.defending.name)?;
    let base = floor_div(2 * a.attacking.level, 5) + 2;
    let mut pre_roll = floor_div(
        floor_div(attacking_species.stats.attack * a.mv.base_power.unwrap_or(10) * base, defending_species.stats.defense.max(1)),
        50,
    ) + 2;
    if crit && a.defending.ability != gc::BATTLE_ARMOR && a.defending.ability != gc::SHELL_ARMOR {
        pre_roll *= 2;
    }
    Some(DamageRange::from_rolls(pre_roll, MIN_RANGE, MAX_RANGE))
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
    if mv.name == gc::TRIPLE_KICK_MOVE && !custom.starts_with(gc::TRIPLE_KICK_SENTINEL_PREFIX) {
        let kicks = py_int(custom).unwrap_or(1).clamp(1, 3);
        if kicks < 2 {
            return None;
        }
        let mut hits = Vec::new();
        for kick in 1..=kicks {
            let sentinel = format!("{}{}", gc::TRIPLE_KICK_SENTINEL_PREFIX, 10 * kick);
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

/// `calculate_gen_three_damage`: the whole use (every hit of a multi-hit move).
pub fn calculate_damage(gen: &GenData, a: &DamageArgs) -> Option<DamageRange> {
    calculate_damage_impl(gen, a, false)
}

fn calculate_damage_impl(gen: &GenData, a: &DamageArgs, single_hit: bool) -> Option<DamageRange> {
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

    let (mut move_type, base_power): (String, Option<i64>) = if mv.name == consts::HIDDEN_POWER_MOVE_NAME {
        (
            stats::hidden_power_type_gen345(&attacking_pkmn.dvs).to_string(),
            Some(stats::hidden_power_base_power_gen345(&attacking_pkmn.dvs)),
        )
    } else {
        (mv.move_type.clone(), mv.base_power)
    };
    let mut base_power = base_power.unwrap_or(0);
    let mut targeting = mv.targeting.clone();

    if mv.name == consts::WEATHER_BALL_MOVE_NAME && is_weather_active {
        move_type = damage::get_weather_ball_type(weather, is_weather_active, &move_type).to_string();
    }
    if mv.name == gc::NATURE_POWER_MOVE {
        // `Cmd_callenvironmentattack`: the terrain's move runs its own script
        let (bp, ty, tgt): (i64, &str, &str) = if custom.contains(gc::PLAIN_TERRAIN) {
            (60, consts::TYPE_NORMAL, consts::TARGETING_BOTH_ENEMIES) // Swift
        } else if custom.contains(gc::SAND_TERRAIN) {
            (100, consts::TYPE_GROUND, "target_all") // Earthquake
        } else if custom.contains(gc::CAVE_TERRAIN) {
            (80, consts::TYPE_GHOST, "target_single_enemy") // Shadow Ball
        } else if custom.contains(gc::ROCK_TERRAIN) {
            (75, consts::TYPE_ROCK, consts::TARGETING_BOTH_ENEMIES) // Rock Slide
        } else if custom.contains(gc::TALL_GRASS_TERRAIN) {
            return None; // Stun Spore
        } else if custom.contains(gc::LONG_GRASS_TERRAIN) {
            (55, consts::TYPE_GRASS, consts::TARGETING_BOTH_ENEMIES) // Razor Leaf
        } else if custom.contains(gc::POND_WATER_TERRAIN) {
            (65, consts::TYPE_WATER, "target_single_enemy") // BubbleBeam
        } else if custom.contains(gc::SEA_WATER_TERRAIN) {
            (95, consts::TYPE_WATER, consts::TARGETING_BOTH_ENEMIES) // Surf
        } else if custom.contains(gc::UNDERWATER_TERRAIN) {
            (120, consts::TYPE_WATER, "target_single_enemy") // Hydro Pump
        } else {
            return None;
        };
        base_power = bp;
        move_type = ty.to_string();
        targeting = tgt.to_string();
    }

    let is_struggle = mv.name == consts::STRUGGLE_MOVE_NAME;
    // no `typecalc` at all: no STAB, effectiveness, immunity or Wonder Guard
    let skip_type_and_immunity = is_struggle || mv.name == consts::FUTURE_SIGHT_MOVE_NAME || mv.name == consts::DOOM_DESIRE_MOVE_NAME || mv.name == gc::BEAT_UP_MOVE;

    if !skip_type_and_immunity {
        if gen.effectiveness(&move_type, &d1) == Some(consts::IMMUNE) || gen.effectiveness(&move_type, &d2) == Some(consts::IMMUNE) {
            return None;
        } else if defending_pkmn.ability == gc::LEVITATE && move_type == consts::TYPE_GROUND {
            return None;
        } else if defending_pkmn.ability == gc::DAMP && (mv.name == consts::SELFDESTRUCT_MOVE_NAME || mv.name == consts::EXPLOSION_MOVE_NAME) {
            return None;
        } else if defending_pkmn.ability == gc::VOLT_ABSORB && move_type == consts::TYPE_ELECTRIC && base_power != 0 {
            return None;
        } else if defending_pkmn.ability == gc::WATER_ABSORB && move_type == consts::TYPE_WATER && base_power != 0 {
            return None;
        } else if defending_pkmn.ability == gc::FLASH_FIRE && move_type == consts::TYPE_FIRE {
            return None;
        } else if defending_pkmn.ability == gc::SOUNDPROOF && gc::GEN3_SOUND_MOVES.contains(&mv.name.as_str()) {
            return None;
        } else if defending_pkmn.ability == gc::WONDER_GUARD && base_power != 0 && wonder_guard_blocks(gen, &move_type, &d1, &d2) {
            return None;
        }
    }

    // fixed-damage moves (after the immunity checks)
    match mv.name.as_str() {
        consts::DRAGON_RAGE_MOVE_NAME => return Some(DamageRange::single(40)),
        "SonicBoom" => return Some(DamageRange::single(20)),
        "Seismic Toss" | "Night Shade" => return Some(DamageRange::single(attacking_pkmn.level)),
        _ => {}
    }
    if mv.has_flavor(consts::FLAVOR_PSYWAVE) {
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
        // hits of power 10 / 20 / 30, each with its own roll; the crit range
        // doubles the last kick only
        let num_kicks = py_int(custom).unwrap_or(1).clamp(1, 3);
        let mut total: Option<DamageRange> = None;
        for kick_num in 1..=num_kicks {
            let sentinel = format!("{}{}", gc::TRIPLE_KICK_SENTINEL_PREFIX, 10 * kick_num);
            let mut sub = *a;
            sub.custom_move_data = &sentinel;
            sub.is_crit = a.is_crit && kick_num == num_kicks;
            if let Some(kick) = calculate_damage_impl(gen, &sub, true) {
                total = Some(match total {
                    None => kick,
                    Some(t) => t.add(&kick),
                });
            }
        }
        return total;
    } else if mv.name == gc::BEAT_UP_MOVE {
        let num_hits = if single_hit { 1 } else { beat_up_hits(custom) };
        let mut total = beat_up_hit(gen, a, a.is_crit)?;
        if num_hits > 1 {
            let other = beat_up_hit(gen, a, false)?;
            for _ in 1..num_hits {
                total = total.add(&other);
            }
        }
        return Some(total);
    } else if mv.name == gc::COUNTER_MOVE || mv.name == gc::MIRROR_COAT_MOVE || mv.name == gc::BIDE_MOVE {
        let damage_taken = py_int(custom)?;
        if damage_taken <= 0 {
            return None;
        }
        return Some(DamageRange::single(damage_taken * 2));
    }

    // variable base power (`gDynamicBasePower`)
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
            base_power = floor_div(base_power * v, 100).max(1);
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
            Some(t) => (t.clamp(1, 5), false),
            None => (5, true),
        };
        base_power = base_power * (1i64 << (turn - 1)) * if curl { 2 } else { 1 };
    } else if mv.name == gc::FURY_CUTTER_MOVE {
        let turn = py_int(custom).unwrap_or(1).clamp(1, 5);
        base_power *= 1i64 << (turn - 1);
    } else if mv.name == gc::LOW_KICK_MOVE {
        // `Cmd_weightdamagecalculation`: strictly-less thresholds in hectograms
        let weight_hg = match defending_species.weight {
            Some(w) => (w * 10.0).round_ties_even() as i64,
            None => return None,
        };
        base_power = if weight_hg < 100 {
            20
        } else if weight_hg < 250 {
            40
        } else if weight_hg < 500 {
            60
        } else if weight_hg < 1000 {
            80
        } else if weight_hg < 2000 {
            100
        } else {
            120
        };
    }
    if base_power == 0 {
        return None;
    }

    let default_stages = StageModifiers::default();
    let attacking_stages = *a.attacking_stages.unwrap_or(&default_stages);
    let defending_stages = *a.defending_stages.unwrap_or(&default_stages);

    // `gCritMultiplier`: only `critcalc` moves, cancelled by Battle/Shell Armor
    let is_real_crit = a.is_crit && !never_crits(&mv.name) && defending_pkmn.ability != gc::BATTLE_ARMOR && defending_pkmn.ability != gc::SHELL_ARMOR;

    let is_special = gen.special_types.contains(&move_type) && !is_struggle;

    // raw stats (nature only); a caller-supplied block (transformed mon) is
    // used as-is, stages included
    let override_attacker = a.attacking_battle_stats.is_some();
    let override_defender = a.defending_battle_stats.is_some();
    let mut atk = match a.attacking_battle_stats {
        Some(s) => *s,
        None => stats::calc_battle_stats(
            &attacking_pkmn.base_stats,
            attacking_pkmn.level,
            &attacking_pkmn.dvs,
            &attacking_pkmn.stat_xp,
            &StageModifiers::default(),
            None,
            attacking_pkmn.nature,
            attacking_pkmn.held_item.as_deref(),
            false,
            None,
        ),
    };
    let mut def = match a.defending_battle_stats {
        Some(s) => *s,
        None => stats::calc_battle_stats(
            &defending_pkmn.base_stats,
            defending_pkmn.level,
            &defending_pkmn.dvs,
            &defending_pkmn.stat_xp,
            &StageModifiers::default(),
            None,
            defending_pkmn.nature,
            defending_pkmn.held_item.as_deref(),
            false,
            None,
        ),
    };

    // ---- CalculateBaseDamage modifier chain ----
    if attacking_pkmn.ability == gc::HUGE_POWER || attacking_pkmn.ability == gc::PURE_POWER {
        atk.attack *= 2;
    }
    // badge boosts (player side only; Ruby/Sapphire: trainer battles only)
    let version = gen.effective_version();
    let badge_boosts_apply = !(a.is_wild_battle && (version == consts::RUBY_VERSION || version == consts::SAPPHIRE_VERSION));
    if badge_boosts_apply && !override_attacker {
        if let Some(b) = &attacking_pkmn.badges {
            if b.is_attack_boosted() {
                atk.attack = floor_mul_ratio(atk.attack, 11, 10);
            }
            if b.is_special_attack_boosted() {
                atk.special_attack = floor_mul_ratio(atk.special_attack, 11, 10);
            }
        }
    }
    if badge_boosts_apply && !override_defender {
        if let Some(b) = &defending_pkmn.badges {
            if b.is_defense_boosted() {
                def.defense = floor_mul_ratio(def.defense, 11, 10);
            }
            if b.is_special_defense_boosted() {
                def.special_defense = floor_mul_ratio(def.special_defense, 11, 10);
            }
        }
    }
    let held = attacking_pkmn.held_item_str();
    let dheld = defending_pkmn.held_item_str();
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
    if held == gc::CHOICE_BAND {
        atk.attack = floor_mul_ratio(atk.attack, 3, 2);
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
        atk.special_attack *= 2;
    }
    if dheld == gc::METAL_POWDER && defending_pkmn.name == gc::DITTO {
        def.defense *= 2;
    }
    if held == gc::THICK_CLUB && (attacking_pkmn.name == gc::GEN2_MAROWAK || attacking_pkmn.name == gc::GEN2_CUBONE) {
        atk.attack *= 2;
    }
    if defending_pkmn.ability == gc::THICK_FAT && (move_type == consts::TYPE_FIRE || move_type == consts::TYPE_ICE) {
        atk.special_attack = floor_div(atk.special_attack, 2);
    }
    if attacking_pkmn.ability == gc::HUSTLE {
        atk.attack = floor_mul_ratio(atk.attack, 3, 2);
    }
    if mv.name == consts::EXPLOSION_MOVE_NAME || mv.name == consts::SELFDESTRUCT_MOVE_NAME {
        def.defense = floor_div(def.defense, 2);
    }

    let (atk_raw, atk_stage, def_raw, def_stage, screen) = if is_special {
        (
            atk.special_attack,
            attacking_stages.special_attack_stage,
            def.special_defense,
            defending_stages.special_defense_stage,
            a.defending_field.map(|f| f.light_screen).unwrap_or(false),
        )
    } else {
        (atk.attack, attacking_stages.attack_stage, def.defense, defending_stages.defense_stage, a.defending_field.map(|f| f.reflect).unwrap_or(false))
    };
    // crit: a negative attacker stage / positive defender stage is ignored
    let attacking_stat = if override_attacker || (is_real_crit && atk_stage <= 0) {
        atk_raw
    } else {
        stats::modify_stat_by_stage(Gen::Three, atk_raw, atk_stage)
    };
    let defending_stat = if override_defender || (is_real_crit && def_stage >= 0) {
        def_raw
    } else {
        stats::modify_stat_by_stage(Gen::Three, def_raw, def_stage)
    };
    if defending_stat == 0 {
        return None;
    }

    let is_stab = !skip_type_and_immunity && (a1 == move_type || a2 == move_type);

    let mut temp = attacking_stat * base_power * (floor_div(2 * attacking_pkmn.level, 5) + 2);
    temp = floor_div(temp, defending_stat);
    temp = floor_div(temp, 50);

    if screen && !is_real_crit && mv.name != gc::BRICK_BREAK_MOVE {
        if a.is_double_battle {
            temp = 2 * floor_div(temp, 3);
        } else {
            temp = floor_div(temp, 2);
        }
    }
    if a.is_double_battle && targeting == consts::TARGETING_BOTH_ENEMIES {
        temp = floor_div(temp, 2);
    }
    if !is_special && temp == 0 {
        temp = 1;
    }

    if is_special && is_weather_active {
        if weather == consts::WEATHER_RAIN {
            if move_type == consts::TYPE_FIRE {
                temp = floor_div(temp, 2);
            } else if move_type == consts::TYPE_WATER {
                temp = floor_mul_ratio(temp, 3, 2);
            }
        }
        if (weather == consts::WEATHER_RAIN || weather == consts::WEATHER_SANDSTORM || weather == consts::WEATHER_HAIL) && mv.name == consts::SOLAR_BEAM_MOVE_NAME {
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

    // ---- Cmd_damagecalc: crit and dmgMultiplier ----
    if is_real_crit {
        temp *= 2;
    }
    if mv.name == gc::SPIT_UP_MOVE {
        temp *= py_int(custom)?;
    }
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

    // ---- Cmd_typecalc ----
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
                        temp = floor_div(temp, 2).max(1);
                    }
                }
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
