//! The structural undo snapshot must serialize to exactly the JSON the
//! router writes for its tree, and undo must bring the route back.

use std::path::PathBuf;
use std::sync::Arc;

use xpr_data::Registry;
use xpr_engine::{EventDefinition, InsertSpec, Router, UndoManager};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn load(name: &str) -> Router {
    let root = repo_root();
    let reg = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    let mut router = Router::new(reg);
    router.load(&root.join("tests/test_data").join(name), false).expect("route loads");
    router
}

#[test]
fn snapshot_json_matches_serialize_folder() {
    for name in ["yellow-pinsir-lv10brock.json", "c-porygon-1-.json", "f-ditto-tests.json", "platinum_chimchar.json"] {
        let router = load(name);
        let mut undo = UndoManager::new(15);
        undo.save_state(&router, true);
        let snapshot = undo.get_current_state().expect("snapshot taken");
        assert_eq!(snapshot.events.to_json(), router.serialize_folder(router.root_id), "{}", name);
    }
}

#[test]
fn undo_restores_the_previous_tree() {
    let mut router = load("yellow-pinsir-lv10brock.json");
    let before_bytes = router.save_bytes().unwrap();
    let before_final = router.get_final_state().unwrap().solo_pkmn.cur_level;
    let mut undo = UndoManager::new(15);
    undo.save_state(&router, false);

    // insert candies before the first trainer, the way the app does
    let trainer = router.all_groups().into_iter().find(|g| router.group(*g).unwrap().event_definition.trainer_def.is_some()).unwrap();
    undo.save_state(&router, false);
    router
        .add_event_object(Some(EventDefinition::with_rare_candy(5)), None, InsertSpec { insert_before: Some(trainer), ..Default::default() }, true, None, None)
        .unwrap();
    undo.save_state(&router, true);
    assert_ne!(router.save_bytes().unwrap(), before_bytes);
    assert_ne!(router.get_final_state().unwrap().solo_pkmn.cur_level, before_final);

    assert!(undo.can_undo());
    let previous = undo.get_undo_state().unwrap();
    router.restore_events_from_state(&previous).unwrap();
    undo.set_current_state(previous);
    assert_eq!(router.save_bytes().unwrap(), before_bytes);
    assert_eq!(router.get_final_state().unwrap().solo_pkmn.cur_level, before_final);
    assert!(!undo.can_undo());
}
