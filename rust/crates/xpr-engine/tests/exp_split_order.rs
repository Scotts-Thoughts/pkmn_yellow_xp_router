//! A trainer fight's `exp_split` is indexed by the trainer's definition, also
//! when the fight has a mon order: h-typhlosion-line-2-23925 recorded Leader
//! Blue (HeartGold) fought Exeggutor, Rhydon, Gyarados, Machamp, Arcanine,
//! Pidgeot with Arcanine's exp shared with a teammate, and the route halved
//! Gyarados's instead (+131 exp against the game).

use std::path::PathBuf;
use std::sync::Arc;

use xpr_data::Registry;
use xpr_engine::{EventDefinition, TrainerEventDefinition};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

#[test]
fn exp_split_follows_the_definition_with_a_mon_order() {
    let gen = Arc::new(Registry::new(repo_root().join("raw_pkmn_data"), PathBuf::new())).get_version("HeartGold").unwrap();
    let mut td = TrainerEventDefinition::new("Leader Blue");
    td.mon_order = vec![1, 5, 2, 3, 4, 6];
    td.exp_split = vec![1, 2, 1, 1, 1, 1];
    let def = EventDefinition::with_trainer(td);
    let fought: Vec<(i64, String, i64)> = def.get_pokemon_list(&gen, false).unwrap().into_iter().map(|(i, m)| (i, m.name.clone(), m.exp_split)).collect();
    let names: Vec<&str> = fought.iter().map(|(_, n, _)| n.as_str()).collect();
    assert_eq!(names, ["Exeggutor", "Rhydon", "Gyarados", "Machamp", "Arcanine", "Pidgeot"]);
    // each mon's definition index, and the split the route pays it with
    for (idx, name, split) in &fought {
        let expected_idx = ["Exeggutor", "Arcanine", "Rhydon", "Gyarados", "Machamp", "Pidgeot"].iter().position(|n| n == name).unwrap() as i64;
        assert_eq!(*idx, expected_idx, "{name}");
        assert_eq!(*split, if name == "Arcanine" { 2 } else { 1 }, "{name}");
    }
}
