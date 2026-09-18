//! Port of `tests/test_route_processing.py`.

use std::path::PathBuf;
use std::sync::Arc;

use xpr_data::Registry;
use xpr_engine::{NodeId, Router};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn load_route(name: &str) -> (Router, Vec<NodeId>) {
    let root = repo_root();
    let reg = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    let mut router = Router::new(reg);
    router.load(&root.join("tests/test_data").join(name), false).expect("route loads");
    let groups = router.all_groups();
    (router, groups)
}

fn final_mon(router: &Router) -> &xpr_engine::SoloPokemon {
    &router.get_final_state().unwrap().solo_pkmn
}

fn moves(m: &[Option<String>]) -> Vec<Option<&str>> {
    m.iter().map(|x| x.as_deref()).collect()
}

fn stats(s: &xpr_data::StatBlock) -> (i64, i64, i64, i64, i64, i64) {
    (s.hp, s.attack, s.defense, s.special_attack, s.special_defense, s.speed)
}

fn assert_no_errors(router: &Router, events: &[NodeId]) {
    for g in events {
        for item in &router.group(*g).unwrap().event_items {
            assert_eq!(router.item(*item).unwrap().error_message, "", "event {}", router.group(*g).unwrap().name);
        }
    }
}

#[test]
fn yellow_pinsir_route() {
    let (router, events) = load_route("yellow-pinsir-lv10brock.json");
    assert_eq!(router.pkmn_version.as_deref(), Some("Yellow"));
    assert_eq!(events.len(), 25);
    let init = &router.init_route_state.as_ref().unwrap().solo_pkmn;
    assert_eq!(init.name, "Pinsir");
    assert_eq!(init.cur_level, 5);
    let f = final_mon(&router);
    assert_eq!(f.cur_level, 11);
    assert_eq!(f.cur_xp, 1947);
    assert_eq!(f.xp_to_next_level, 213);
    assert_eq!(stats(&f.cur_stats), (39, 40, 31, 20, 20, 27));
    let sx = f.realized_stat_xp;
    assert_eq!((sx.hp, sx.attack, sx.defense, sx.special_attack, sx.speed), (800, 660, 835, 485, 791));
    assert_eq!(router.get_final_state().unwrap().inventory.cur_money, 1753);
    let e6 = &router.final_state_of(events[6]).unwrap().solo_pkmn;
    assert_eq!((e6.cur_level, e6.cur_xp), (8, 660));
    assert_eq!((e6.cur_stats.hp, e6.cur_stats.attack, e6.cur_stats.defense, e6.cur_stats.speed), (31, 27, 23, 21));
    let e12 = &router.final_state_of(events[12]).unwrap().solo_pkmn;
    assert_eq!((e12.cur_level, e12.cur_xp, e12.cur_stats.hp, e12.cur_stats.attack), (9, 988, 33, 30));
    let brock = router.group(events[24]).unwrap();
    assert!(brock.event_definition.trainer_def.as_ref().unwrap().trainer_name.contains("Brock"));
    let before = &brock.init_state.as_ref().unwrap().solo_pkmn;
    assert_eq!((before.cur_level, before.cur_stats.hp, before.cur_stats.attack), (10, 36, 33));
    let after = &brock.final_state.as_ref().unwrap().solo_pkmn;
    assert_eq!((after.cur_level, after.cur_stats.hp, after.cur_stats.attack), (11, 39, 40));
    assert_no_errors(&router, &events);
}

#[test]
fn crystal_porygon_route() {
    let (router, events) = load_route("c-porygon-1-.json");
    assert_eq!(router.pkmn_version.as_deref(), Some("Crystal"));
    assert_eq!(events.len(), 19);
    let init = &router.init_route_state.as_ref().unwrap().solo_pkmn;
    assert_eq!(init.name, "Charizard");
    assert_eq!(init.cur_level, 5);
    let f = final_mon(&router);
    assert_eq!((f.cur_level, f.cur_xp, f.xp_to_next_level), (10, 613, 129));
    assert_eq!(stats(&f.cur_stats), (39, 25, 23, 30, 25, 28));
    assert_eq!(stats(&f.realized_stat_xp), (325, 360, 320, 250, 276, 445));
    assert_eq!(moves(&f.move_list), vec![Some("Scratch"), Some("Growl"), Some("Ember"), Some("Smokescreen")]);
    assert_eq!(router.get_final_state().unwrap().inventory.cur_money, 3796);
    let e4 = &router.final_state_of(events[4]).unwrap().solo_pkmn;
    assert_eq!((e4.cur_level, e4.cur_xp, e4.cur_stats.hp), (5, 135, 24));
    let e9 = &router.final_state_of(events[9]).unwrap().solo_pkmn;
    assert_eq!((e9.cur_level, e9.cur_xp), (7, 272));
    assert_eq!((e9.cur_stats.hp, e9.cur_stats.attack, e9.cur_stats.speed), (30, 19, 21));
    assert_no_errors(&router, &events);
}

#[test]
fn firered_ditto_route() {
    let (router, events) = load_route("f-ditto-tests.json");
    assert_eq!(router.pkmn_version.as_deref(), Some("FireRed"));
    assert_eq!(events.len(), 36);
    let f = final_mon(&router);
    assert_eq!(f.name, "Ditto");
    assert_eq!((f.cur_level, f.cur_xp, f.xp_to_next_level), (99, 984653, 15347));
    assert_eq!(stats(&f.cur_stats), (234, 159, 131, 118, 131, 131));
    assert_eq!(stats(&f.realized_stat_xp), (2, 60, 6, 10, 4, 7));
    assert_eq!(router.get_final_state().unwrap().inventory.cur_money, 32100);
    let e9 = &router.final_state_of(events[9]).unwrap().solo_pkmn;
    assert_eq!((e9.cur_level, e9.cur_xp), (97, 928753));
    let mut prev_xp = 0;
    let mut prev_level = 0;
    for g in &events {
        let s = &router.final_state_of(*g).unwrap().solo_pkmn;
        assert!(s.cur_xp >= prev_xp);
        assert!(s.cur_level >= prev_level);
        prev_xp = s.cur_xp;
        prev_level = s.cur_level;
    }
}

/// KI-1: the Python reference produces 3,240 here (gen-4 money x4); the
/// original test expected 3,060. The port reproduces the reference.
#[test]
fn platinum_chimchar_route() {
    let (router, events) = load_route("platinum_chimchar.json");
    assert_eq!(router.pkmn_version.as_deref(), Some("Platinum"));
    assert_eq!(events.len(), 15);
    let f = final_mon(&router);
    assert_eq!(f.name, "Chimchar");
    assert_eq!((f.cur_level, f.cur_xp, f.xp_to_next_level), (8, 328, 91));
    assert_eq!(stats(&f.cur_stats), (27, 16, 14, 16, 14, 17));
    assert_eq!(stats(&f.realized_stat_xp), (3, 0, 0, 0, 0, 1));
    assert_eq!(moves(&f.move_list), vec![Some("Scratch"), Some("Leer"), Some("Ember"), None]);
    assert_eq!(router.get_final_state().unwrap().inventory.cur_money, 3240, "KI-1");
    let e3 = &router.final_state_of(events[3]).unwrap().solo_pkmn;
    assert_eq!((e3.cur_level, e3.cur_xp), (5, 159));
    let e7 = &router.final_state_of(events[7]).unwrap().solo_pkmn;
    assert_eq!((e7.cur_level, e7.cur_xp, e7.cur_stats.hp, e7.cur_stats.speed), (8, 328, 27, 17));
    assert_no_errors(&router, &events);
}

/// Round trip: saving a loaded route, loading those bytes again and saving
/// once more must be a fixed point. (The test routes were written by older
/// Python versions and gain keys on re-save -- byte-for-byte agreement with
/// what *Python* writes today is checked by `xpr-golden verify`.)
#[test]
fn save_round_trip_is_a_fixed_point() {
    for name in ["yellow-pinsir-lv10brock.json", "c-porygon-1-.json", "f-ditto-tests.json", "platinum_chimchar.json"] {
        let (router, _) = load_route(name);
        let first = router.save_bytes().unwrap();
        let tmp = std::env::temp_dir().join(format!("xpr_round_trip_{}", name));
        std::fs::write(&tmp, &first).unwrap();
        let mut again = Router::new(router.registry().clone());
        again.load(&tmp, false).unwrap_or_else(|e| panic!("{}: reload failed: {}", name, e));
        let second = again.save_bytes().unwrap();
        let _ = std::fs::remove_file(&tmp);
        assert_eq!(String::from_utf8_lossy(&first), String::from_utf8_lossy(&second), "{}: re-save differs", name);
    }
}
