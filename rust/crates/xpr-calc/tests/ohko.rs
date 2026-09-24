//! OHKO move (Horn Drill, Guillotine, Fissure, Sheer Cold) hit chances and
//! failure rules, checked against the decomps:
//! - gen 1 (pokered `OneHitKOEffect_`): fails if the user is slower; 30% is 76/256
//! - gen 2 (pokecrystal `BattleCommand_OHKO`): fails if the user is a lower
//!   level; accuracy is 76/256 plus 2/256 per level above, capped at 255 (always hits)
//! - gen 3 (pokeemerald `Cmd_tryKO`): fails vs a higher level or Sturdy;
//!   hits when `Random() % 100 + 1 < accuracy + level difference`
//! - gen 4 (pokeplatinum `BtlCmd_TryOHKOMove`) and gen 5: fails vs a higher
//!   level or Sturdy; `accuracy + level difference` out of 100, skipping the
//!   usual accuracy modifiers

use std::path::PathBuf;
use std::sync::Arc;

use xpr_calc::{calculate_damage, get_move_accuracy, DamageArgs, DamageRange};
use xpr_core::consts;
use xpr_data::gen_consts as gc;
use xpr_data::model::{EnemyPkmn, StageModifiers};
use xpr_data::{GenData, Registry};

fn registry() -> Arc<Registry> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()))
}

fn mon(gen: &GenData, name: &str, level: i64) -> EnemyPkmn {
    gen.create_trainer_pkmn(name, level).unwrap()
}

fn accuracy(gen: &GenData, attacker: &EnemyPkmn, move_name: &str, defender: &EnemyPkmn) -> Option<f64> {
    let mv = gen.move_db().get_move(move_name).unwrap();
    get_move_accuracy(gen, &DamageArgs::new(attacker, mv, defender))
}

fn damage(gen: &GenData, attacker: &EnemyPkmn, move_name: &str, defender: &EnemyPkmn, stages: Option<&StageModifiers>, is_crit: bool) -> Option<DamageRange> {
    let mv = gen.move_db().get_move(move_name).unwrap();
    let mut args = DamageArgs::new(attacker, mv, defender);
    args.attacking_stages = stages;
    args.is_crit = is_crit;
    calculate_damage(gen, &args)
}

#[test]
fn gen1_ohko_uses_raw_accuracy_and_speed() {
    let reg = registry();
    let gen = reg.get_version(consts::YELLOW_VERSION).unwrap();
    let rhydon = mon(&gen, "Rhydon", 30);
    let pidgey = mon(&gen, "Pidgey", 30);
    assert_eq!(accuracy(&gen, &rhydon, "Horn Drill", &pidgey), Some(76.0 / 256.0 * 100.0));

    // slower: fails, crit or not
    assert!(rhydon.cur_stats.speed < pidgey.cur_stats.speed);
    assert!(damage(&gen, &rhydon, "Horn Drill", &pidgey, None, false).is_none());
    assert!(damage(&gen, &rhydon, "Horn Drill", &pidgey, None, true).is_none());

    // faster only thanks to speed stages: the crit calc (which drops stages)
    // must agree with the normal one
    let fast = StageModifiers::default().apply_stat_mod(&[(consts::SPE.to_string(), 6)]);
    let normal = damage(&gen, &rhydon, "Horn Drill", &pidgey, Some(&fast), false).unwrap();
    let crit = damage(&gen, &rhydon, "Horn Drill", &pidgey, Some(&fast), true).unwrap();
    assert_eq!((normal.min_damage, normal.max_damage), (pidgey.cur_stats.hp, pidgey.cur_stats.hp));
    assert_eq!(normal, crit);
}

#[test]
fn gen2_ohko_accuracy_scales_with_level() {
    let reg = registry();
    let gen = reg.get_version(consts::CRYSTAL_VERSION).unwrap();
    let pidgey = mon(&gen, "Pidgey", 30);
    assert_eq!(accuracy(&gen, &mon(&gen, "Nidoking", 30), "Horn Drill", &pidgey), Some(76.0 / 256.0 * 100.0));
    assert_eq!(accuracy(&gen, &mon(&gen, "Nidoking", 40), "Horn Drill", &pidgey), Some(96.0 / 256.0 * 100.0));
    // 76 + 2 * 95 caps at 255, which skips the roll entirely
    assert_eq!(accuracy(&gen, &mon(&gen, "Nidoking", 100), "Horn Drill", &mon(&gen, "Pidgey", 5)), None);
    // no speed check, only the level check
    assert!(damage(&gen, &mon(&gen, "Nidoking", 29), "Horn Drill", &pidgey, None, false).is_none());
    assert!(damage(&gen, &mon(&gen, "Nidoking", 30), "Horn Drill", &mon(&gen, "Jolteon", 30), None, false).is_some());
}

#[test]
fn gen3_ohko_accuracy_caps_at_100() {
    let reg = registry();
    let gen = reg.get_version(consts::EMERALD_VERSION).unwrap();
    assert_eq!(accuracy(&gen, &mon(&gen, "Nidoking", 30), "Horn Drill", &mon(&gen, "Pidgey", 30)), Some(29.0));
    assert_eq!(accuracy(&gen, &mon(&gen, "Nidoking", 100), "Horn Drill", &mon(&gen, "Pidgey", 5)), Some(100.0));
}

#[test]
fn gen4_and_gen5_ohko_check_level_not_speed() {
    let reg = registry();
    for version in [consts::PLATINUM_VERSION, consts::BLACK_VERSION] {
        let gen = reg.get_version(version).unwrap();
        let snorlax = mon(&gen, "Snorlax", 50);
        let pikachu = mon(&gen, "Pikachu", 50);
        assert!(snorlax.cur_stats.speed < pikachu.cur_stats.speed);
        let hit = damage(&gen, &snorlax, "Guillotine", &pikachu, None, false).unwrap();
        assert_eq!((hit.min_damage, hit.max_damage), (pikachu.cur_stats.hp, pikachu.cur_stats.hp), "{version}");
        assert!(damage(&gen, &mon(&gen, "Snorlax", 49), "Guillotine", &pikachu, None, false).is_none(), "{version}");
        assert_eq!(accuracy(&gen, &mon(&gen, "Snorlax", 49), "Guillotine", &pikachu), Some(0.0), "{version}");

        let mut sturdy = pikachu.clone();
        sturdy.ability = gc::STURDY.to_string();
        assert!(damage(&gen, &snorlax, "Guillotine", &sturdy, None, false).is_none(), "{version}");

        // base accuracy plus the level difference; Wide Lens doesn't apply
        let mut lens = mon(&gen, "Snorlax", 60);
        lens.held_item = Some(gc::WIDE_LENS.to_string());
        assert_eq!(accuracy(&gen, &lens, "Guillotine", &pikachu), Some(40.0), "{version}");
        assert_eq!(accuracy(&gen, &mon(&gen, "Snorlax", 100), "Guillotine", &mon(&gen, "Pikachu", 5)), Some(100.0), "{version}");
    }
}
