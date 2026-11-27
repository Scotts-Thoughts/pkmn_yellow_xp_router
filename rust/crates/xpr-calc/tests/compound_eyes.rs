//! Compound Eyes: x1.3 accuracy (gens 3-5). The Gen 4 data and the Gen 5
//! trainer files spell it "Compoundeyes" (and "Lightningrod"); the loaders
//! canonicalize those so the ability checks match and the accuracy (and the
//! turns-to-KO built on it) include the boost.

use std::path::PathBuf;
use std::sync::Arc;

use xpr_calc::{get_move_accuracy, DamageArgs};
use xpr_core::consts;
use xpr_data::gen_consts as gc;
use xpr_data::model::EnemyPkmn;
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

fn accuracy(gen: &GenData, attacker: &EnemyPkmn, move_name: &str, defender: &EnemyPkmn) -> Option<f64> {
    let mv = gen.move_db().get_move(move_name).unwrap();
    get_move_accuracy(gen, &DamageArgs::new(attacker, mv, defender))
}

#[test]
fn species_abilities_use_the_canonical_spelling() {
    let reg = registry();
    for version in [
        consts::EMERALD_VERSION,
        consts::DIAMOND_VERSION,
        consts::PLATINUM_VERSION,
        consts::HEART_GOLD_VERSION,
        consts::BLACK_VERSION,
        consts::BLACK_2_VERSION,
    ] {
        let gen = reg.get_version(version).unwrap();
        let butterfree = gen.pkmn_db().get_pkmn("Butterfree").unwrap();
        assert_eq!(butterfree.abilities[0], gc::COMPOUND_EYES, "{version}");
        let electrike = gen.pkmn_db().get_pkmn("Electrike").unwrap();
        if version != consts::EMERALD_VERSION {
            assert!(electrike.abilities.iter().any(|a| a == gc::LIGHTNING_ROD), "{version}: {:?}", electrike.abilities);
        }
    }
}

#[test]
fn heartgold_compound_eyes_boosts_accuracy() {
    let reg = registry();
    let gen = reg.get_version(consts::HEART_GOLD_VERSION).unwrap();
    // the player's ability comes from the species list (by index), as the router does
    let species_ability = gen.pkmn_db().get_pkmn("Butterfree").unwrap().abilities[0].clone();
    let butterfree = mon(&gen, "Butterfree", 30, &species_ability);
    let plain = mon(&gen, "Butterfree", 30, "Tinted Lens");
    let defender = mon(&gen, "Rattata", 30, "Run Away");

    // Sleep Powder 75 -> 97 (75 * 130 / 100)
    assert_eq!(accuracy(&gen, &plain, "Sleep Powder", &defender), Some(75.0));
    assert_eq!(accuracy(&gen, &butterfree, "Sleep Powder", &defender), Some(97.0));
    // capped at 100
    assert_eq!(accuracy(&gen, &butterfree, "Tackle", &defender), Some(100.0));
    // OHKO moves ignore it
    assert_eq!(accuracy(&gen, &butterfree, "Fissure", &defender), Some(30.0));
}

#[test]
fn trainer_mons_keep_compound_eyes() {
    // HGSS trainer files list "Compoundeyes" too
    let reg = registry();
    let gen = reg.get_version(consts::HEART_GOLD_VERSION).unwrap();
    let mons = || gen.trainer_db().iter().flat_map(|t| t.pkmn.iter());
    assert!(mons().any(|p| p.ability == gc::COMPOUND_EYES));
    assert!(!mons().any(|p| p.ability == "Compoundeyes" || p.ability == "Lightningrod"));
}
