//! The player's current HP set on the battle page (`attacker_cur_hp`):
//! - Eruption / Water Spout: `power * curHP / maxHP`, at least 1 (pokeemerald
//!   `Cmd_scaledamagebyhealthratio`)
//! - Flail / Reversal: the `48 * curHP / maxHP` buckets (pokeemerald
//!   `Cmd_remaininghptopower`, `sFlailHpScaleToPowerTable`)
//! - Overgrow / Blaze / Torrent / Swarm: `curHP <= maxHP / 3`
//! - it round-trips through the trainer event's `player_cur_hp` key

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::json;
use xpr_calc::{calculate_damage, flail_option_for_hp, hp_scaled_power, DamageArgs, DamageRange};
use xpr_core::consts;
use xpr_data::gen_consts as gc;
use xpr_data::model::{EnemyPkmn, Move};
use xpr_data::{GenData, Registry};
use xpr_engine::TrainerEventDefinition;

fn registry() -> Arc<Registry> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()))
}

fn mon(gen: &GenData, name: &str, level: i64, ability: &str) -> EnemyPkmn {
    let mut m = gen.create_trainer_pkmn(name, level).unwrap();
    m.ability = ability.to_string();
    m
}

fn calc(gen: &GenData, attacker: &EnemyPkmn, mv: &Move, defender: &EnemyPkmn, cur_hp: Option<i64>, custom: &str) -> DamageRange {
    let mut args = DamageArgs::new(attacker, mv, defender);
    args.attacker_cur_hp = cur_hp;
    args.custom_move_data = custom;
    calculate_damage(gen, &args).unwrap()
}

fn with_power(mv: &Move, power: i64) -> Move {
    let mut m = mv.clone();
    m.base_power = Some(power);
    m
}

#[test]
fn eruption_scales_from_the_exact_hp_and_blaze_applies_at_a_third() {
    let reg = registry();
    for version in [consts::EMERALD_VERSION, consts::HEART_GOLD_VERSION] {
        let gen = reg.get_version(version).unwrap();
        let attacker = mon(&gen, "Typhlosion", 50, gc::BLAZE);
        let defender = mon(&gen, "Snorlax", 50, "Thick Fat");
        let eruption = gen.move_db().get_move("Eruption").unwrap();
        let max = attacker.cur_stats.hp;
        let third = max / 3;

        // at a third: 150 * hp / max, then Blaze's 150 / 100
        let power = hp_scaled_power(150, third, max);
        let expected = calc(&gen, &attacker, &with_power(eruption, power * 150 / 100), &defender, None, "100");
        assert_eq!(calc(&gen, &attacker, eruption, &defender, Some(third), ""), expected, "{version}");
        // the HP wins over a stale % dropdown value
        assert_eq!(calc(&gen, &attacker, eruption, &defender, Some(third), "100"), expected, "{version}");

        // one HP higher: no Blaze
        let power = hp_scaled_power(150, third + 1, max);
        let expected = calc(&gen, &attacker, &with_power(eruption, power), &defender, None, "100");
        assert_eq!(calc(&gen, &attacker, eruption, &defender, Some(third + 1), ""), expected, "{version}");

        // a Fire move other than Eruption gets Blaze from the HP too
        let flamethrower = gen.move_db().get_move("Flamethrower").unwrap();
        let boosted = calc(&gen, &attacker, &with_power(flamethrower, 142), &defender, None, "");
        assert_eq!(calc(&gen, &attacker, flamethrower, &defender, Some(third), ""), boosted, "{version}");
        assert_eq!(calc(&gen, &attacker, flamethrower, &defender, Some(third + 1), ""), calc(&gen, &attacker, flamethrower, &defender, None, ""), "{version}");
    }
}

#[test]
fn hp_scaled_power_matches_the_game() {
    assert_eq!(hp_scaled_power(150, 264, 264), 150);
    assert_eq!(hp_scaled_power(150, 88, 264), 50);
    assert_eq!(hp_scaled_power(150, 87, 264), 49);
    // never below 1
    assert_eq!(hp_scaled_power(150, 1, 400), 1);
}

#[test]
fn flail_buckets_match_the_game_table() {
    // 48 * hp / max: <=1 200, <=4 150, <=9 100, <=16 80, <=32 40, else 20
    let max = 480;
    assert_eq!(flail_option_for_hp(10, max), gc::FLAIL_MIN_HP);
    assert_eq!(flail_option_for_hp(19, max), gc::FLAIL_MIN_HP);
    assert_eq!(flail_option_for_hp(20, max), gc::FLAIL_FIVE_PERCENT_HP);
    assert_eq!(flail_option_for_hp(49, max), gc::FLAIL_FIVE_PERCENT_HP);
    assert_eq!(flail_option_for_hp(50, max), gc::FLAIL_TEN_PERCENT_HP);
    assert_eq!(flail_option_for_hp(100, max), gc::FLAIL_QUARTER_HP);
    assert_eq!(flail_option_for_hp(169, max), gc::FLAIL_QUARTER_HP);
    assert_eq!(flail_option_for_hp(170, max), gc::FLAIL_HALF_HP);
    assert_eq!(flail_option_for_hp(329, max), gc::FLAIL_HALF_HP);
    assert_eq!(flail_option_for_hp(330, max), gc::FLAIL_FULL_HP);
    assert_eq!(flail_option_for_hp(max, max), gc::FLAIL_FULL_HP);
}

#[test]
fn player_cur_hp_round_trips_and_is_omitted_when_unset() {
    let mut td = TrainerEventDefinition::new("Leader Flannery");
    let plain = td.serialize();
    assert!(plain.get(consts::PLAYER_CUR_HP_KEY).is_none());
    td.player_cur_hp = Some(88);
    let raw = td.serialize();
    assert_eq!(raw.get(consts::PLAYER_CUR_HP_KEY), Some(&json!(88)));
    let back = TrainerEventDefinition::deserialize(Some(&raw)).unwrap().unwrap();
    assert_eq!(back.player_cur_hp, Some(88));
    let back = TrainerEventDefinition::deserialize(Some(&plain)).unwrap().unwrap();
    assert_eq!(back.player_cur_hp, None);
}
