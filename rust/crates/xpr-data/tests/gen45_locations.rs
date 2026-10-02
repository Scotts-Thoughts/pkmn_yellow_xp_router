//! Gen 4/5 trainers carry their location (the in-game place, from the
//! data_objects repo via `tools/data/import_gen45_trainer_locations.py`),
//! like gens 1–3: quick-add's location filter and the folder a map fight goes
//! into read it. Trainers that can't be placed have none.

use std::path::PathBuf;

use xpr_data::Registry;

fn registry() -> Registry {
    Registry::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../raw_pkmn_data"), PathBuf::new())
}

#[test]
fn gen4_and_5_trainers_have_their_places() {
    let reg = registry();
    let loc = |version: &str, name: &str| reg.get_version(version).unwrap().trainer_db().get_trainer(name).unwrap_or_else(|| panic!("{}: {}", version, name)).location.clone();
    assert_eq!(loc("Platinum", "Youngster Logan"), "Route 202");
    assert_eq!(loc("Platinum", "Rival Turtwig 1"), "Route 201");
    assert_eq!(loc("Platinum", "Leader Roark"), "Oreburgh City");
    assert_eq!(loc("Diamond", "Leader Roark"), "Oreburgh City");
    assert_eq!(loc("HeartGold", "Leader Falkner"), "Violet City");
    // a trainer met in several places lists them all
    assert_eq!(loc("Black 2", "Boss Trainer Abigail"), "Black Tower / White Treehollow");
    for v in ["Diamond", "Platinum", "HeartGold", "Black", "Black 2"] {
        let gen = reg.get_version(v).unwrap();
        let locations = gen.trainer_db().get_all_locations();
        assert!(locations.len() > 40, "{}: {} locations", v, locations.len());
        let located = gen.trainer_db().iter().filter(|t| !t.location.is_empty()).count();
        assert!(located * 4 > gen.trainer_db().len() * 3, "{}: {} of {} located", v, located, gen.trainer_db().len());
    }
}
