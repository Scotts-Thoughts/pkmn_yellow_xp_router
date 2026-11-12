//! Overgrow / Blaze / Torrent / Swarm: a move of the ability's type gets
//! 1.5x when the user is at `curHP <= maxHP / 3`.
//! - gen 3 (pokeemerald `CalculateBaseDamage`) and gen 4 (pokeplatinum
//!   `BattleSystem_CalcMoveDamage`): `movePower = movePower * 150 / 100`
//! - gen 5: Atk / SpA x1.5
//!
//! The move row offers it as a "<Ability> (HP <= 1/3)" dropdown option.

use std::path::PathBuf;
use std::sync::Arc;

use xpr_calc::battle_summary::{pinch_option_label, split_pinch_selection, with_pinch_options};
use xpr_calc::{calculate_damage, DamageArgs, DamageRange};
use xpr_core::consts;
use xpr_data::gen_consts as gc;
use xpr_data::model::{EnemyPkmn, Move};
use xpr_data::{GenData, Registry};

fn registry() -> Arc<Registry> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()))
}

fn mon(gen: &GenData, name: &str, level: i64, ability: &str) -> EnemyPkmn {
    let mut m = gen.create_trainer_pkmn(name, level).unwrap();
    m.ability = ability.to_string();
    m
}

fn calc(gen: &GenData, attacker: &EnemyPkmn, mv: &Move, defender: &EnemyPkmn, pinch: bool, custom: &str) -> DamageRange {
    let mut args = DamageArgs::new(attacker, mv, defender);
    args.attacker_pinch = pinch;
    args.custom_move_data = custom;
    calculate_damage(gen, &args).unwrap()
}

fn with_power(mv: &Move, power: i64) -> Move {
    let mut m = mv.clone();
    m.base_power = Some(power);
    m
}

#[test]
fn gen3_and_gen4_boost_the_power() {
    let reg = registry();
    for version in [consts::EMERALD_VERSION, consts::PLATINUM_VERSION] {
        let gen = reg.get_version(version).unwrap();
        let attacker = mon(&gen, "Combusken", 30, gc::BLAZE);
        let defender = mon(&gen, "Machop", 30, "Guts");
        let flamethrower = gen.move_db().get_move("Flamethrower").unwrap();
        let plain = calc(&gen, &attacker, flamethrower, &defender, false, "");
        let boosted = calc(&gen, &attacker, flamethrower, &defender, true, "");
        // 95 * 150 / 100 = 142
        let expected = calc(&gen, &attacker, &with_power(flamethrower, 142), &defender, false, "");
        assert_eq!(boosted, expected, "{version}");
        assert!(boosted.max_damage > plain.max_damage, "{version}");

        // a Fighting move is not boosted by Blaze
        let double_kick = gen.move_db().get_move("Double Kick").unwrap();
        assert_eq!(calc(&gen, &attacker, double_kick, &defender, true, ""), calc(&gen, &attacker, double_kick, &defender, false, ""));

        // nor is a Fire move from a mon without Blaze
        let other = mon(&gen, "Combusken", 30, "Speed Boost");
        assert_eq!(calc(&gen, &other, flamethrower, &defender, true, ""), calc(&gen, &other, flamethrower, &defender, false, ""));
    }
}

#[test]
fn gen5_boosts_the_attack_stat() {
    let reg = registry();
    let gen = reg.get_version(consts::BLACK_VERSION).unwrap();
    let attacker = mon(&gen, "Pignite", 30, gc::BLAZE);
    let defender = mon(&gen, "Patrat", 30, "Run Away");
    let flame = gen.move_db().get_move("Flame Charge").unwrap();
    let plain = calc(&gen, &attacker, flame, &defender, false, "");
    let boosted = calc(&gen, &attacker, flame, &defender, true, "");
    assert!(boosted.max_damage * 10 >= plain.max_damage * 14, "{plain:?} -> {boosted:?}");
    assert!(boosted.max_damage * 10 <= plain.max_damage * 16, "{plain:?} -> {boosted:?}");
}

#[test]
fn eruption_reads_the_pinch_from_its_hp_dropdown() {
    let reg = registry();
    let gen = reg.get_version(consts::HEART_GOLD_VERSION).unwrap();
    let attacker = mon(&gen, "Typhlosion", 50, gc::BLAZE);
    let defender = mon(&gen, "Snorlax", 50, "Thick Fat");
    let eruption = gen.move_db().get_move("Eruption").unwrap();
    let max_hp = attacker.cur_stats.hp;
    let pinch_pct = (1..=100).rev().find(|p| max_hp * p / 100 <= max_hp / 3).unwrap();
    let low = pinch_pct.to_string();
    let above = (pinch_pct + 1).to_string();
    // at the pinch HP the boost applies on its own, without the flag:
    // power = max(1, 150 * pct / 100), then * 150 / 100
    let boosted_power = (150 * pinch_pct / 100).max(1) * 150 / 100;
    assert_eq!(calc(&gen, &attacker, eruption, &defender, false, &low), calc(&gen, &attacker, &with_power(eruption, boosted_power), &defender, false, "100"));
    // one percent higher: no boost, and the flag can't force one
    assert_eq!(calc(&gen, &attacker, eruption, &defender, true, &above), calc(&gen, &attacker, eruption, &defender, false, &above));
    let plain_power = (150 * (pinch_pct + 1) / 100).max(1);
    assert_eq!(calc(&gen, &attacker, eruption, &defender, false, &above), calc(&gen, &attacker, &with_power(eruption, plain_power), &defender, false, "100"));
}

#[test]
fn dropdown_offers_the_pinch_option() {
    let reg = registry();
    let gen = reg.get_version(consts::EMERALD_VERSION).unwrap();
    let torchic = mon(&gen, "Torchic", 10, gc::BLAZE);
    let poochyena = mon(&gen, "Poochyena", 10, "Run Away");
    let db = gen.move_db();
    let label = pinch_option_label(&gen, db.get_move("Ember").unwrap(), &torchic, &poochyena, consts::WEATHER_NONE);
    assert_eq!(label.as_deref(), Some("Blaze (HP <= 1/3)"));
    // not for a Normal move, a status move, or Eruption
    assert_eq!(pinch_option_label(&gen, db.get_move("Scratch").unwrap(), &torchic, &poochyena, consts::WEATHER_NONE), None);
    assert_eq!(pinch_option_label(&gen, db.get_move("Will-O-Wisp").unwrap(), &torchic, &poochyena, consts::WEATHER_NONE), None);
    assert_eq!(pinch_option_label(&gen, db.get_move("Eruption").unwrap(), &torchic, &poochyena, consts::WEATHER_NONE), None);
    // Weather Ball in sun is a Fire move
    assert!(pinch_option_label(&gen, db.get_move("Weather Ball").unwrap(), &torchic, &poochyena, consts::WEATHER_SUN).is_some());
    // gen 2 has no abilities
    let crystal = reg.get_version(consts::CRYSTAL_VERSION).unwrap();
    let cyndaquil = mon(&crystal, "Cyndaquil", 10, gc::BLAZE);
    assert_eq!(pinch_option_label(&crystal, crystal.move_db().get_move("Ember").unwrap(), &cyndaquil, &cyndaquil, consts::WEATHER_NONE), None);

    let label = "Blaze (HP <= 1/3)";
    assert_eq!(with_pinch_options(None, label), vec![gc::NO_BONUS.to_string(), label.to_string()]);
    let surf = vec![gc::NO_BONUS.to_string(), gc::DIVE_BONUS.to_string()];
    let opts = with_pinch_options(Some(&surf), label);
    assert_eq!(opts, vec![gc::NO_BONUS.to_string(), gc::DIVE_BONUS.to_string(), label.to_string(), format!("{} + {label}", gc::DIVE_BONUS)]);

    assert_eq!(split_pinch_selection(label, Some(label)), (gc::NO_BONUS.to_string(), true));
    assert_eq!(split_pinch_selection(&opts[3], Some(label)), (gc::DIVE_BONUS.to_string(), true));
    assert_eq!(split_pinch_selection(gc::DIVE_BONUS, Some(label)), (gc::DIVE_BONUS.to_string(), false));
    assert_eq!(split_pinch_selection("3 Hits", None), ("3 Hits".to_string(), false));
}
