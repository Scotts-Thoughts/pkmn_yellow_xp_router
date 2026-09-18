use std::path::PathBuf;
use xpr_data::Registry;

fn raw_dir() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.join("../../../raw_pkmn_data").canonicalize().unwrap()
}

#[test]
fn loads_every_builtin_version() {
    let reg = Registry::new(raw_dir(), PathBuf::new());
    let errors = reg.load_all_builtin();
    assert!(errors.is_empty(), "errors: {:?}", errors);
    let yellow = reg.get_version("Yellow").unwrap();
    assert_eq!(yellow.pkmn_db().len(), 151);
    assert_eq!(yellow.trainer_db().get_trainer("Brock 1").map(|t| t.pkmn.len()), Some(2));
    let pikachu = yellow.pkmn_db().get_pkmn("pikachu").unwrap();
    assert_eq!(pikachu.stats.hp, 35);
    let crystal = reg.get_version("Crystal").unwrap();
    assert_eq!(crystal.get_generation(), 2);
    let falkner = crystal.trainer_db().get_trainer("Leader Falkner").unwrap();
    assert_eq!(falkner.trainer_id, 1);
    assert_eq!(falkner.pkmn[0].dvs.attack, 9);
    let plat = reg.get_version("Platinum").unwrap();
    let t = plat.trainer_db().get_trainer("Youngster Tristan").unwrap();
    assert_eq!(t.money, 80);
    let black = reg.get_version("Black").unwrap();
    assert!(black.pkmn_db().get_pkmn("Nidoran\u{2642}").is_some());
    let fs = black.move_db().get_move("Fury Attack").unwrap();
    assert!(fs.has_flavor("multi_hit"));
    let e4 = reg.get_version("Emerald").unwrap().get_elite_four_and_champion_names();
    assert!(!e4.is_empty());
    println!("emerald e4: {:?}", e4);
    println!("yellow gyms: {:?}", yellow.get_gym_leader_names());
}
