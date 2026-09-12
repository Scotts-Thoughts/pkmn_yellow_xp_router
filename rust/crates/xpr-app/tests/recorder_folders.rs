//! The recorder's folder bookkeeping through the real controller: folders it
//! creates (`finalize_new_folder`) are enabled and expanded like Python's
//! `EventFolder` defaults, so the events recorded into them count for the
//! route state and are found by `get_previous_event`; the same defaults hold
//! for the split-folder command.

use std::path::PathBuf;
use std::sync::Arc;

use xpr_app::controller::MainController;
use xpr_core::Paths;
use xpr_data::Registry;
use xpr_engine::{EventDefinition, WildPkmnEventDefinition};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn fresh_controller(species: &str, version: &str) -> MainController {
    let root = repo_root();
    let paths = Paths::new(root.clone());
    let registry = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    let mut ctrl = MainController::new(registry, paths);
    ctrl.create_new_route(species, None, version, None, None, None);
    let _ = ctrl.take_signals();
    ctrl
}

fn wild(name: &str, level: i64) -> EventDefinition {
    EventDefinition::with_wild(WildPkmnEventDefinition::new(name, level, 1, false))
}

#[test]
fn recorded_folders_are_enabled_and_expanded() {
    let mut ctrl = fresh_controller("Geodude", "Emerald");
    assert!(ctrl.is_empty());

    // what `RecorderController::add_event` does for a new area
    ctrl.finalize_new_folder("ROUTE_101", None, None);
    let folder_id = *ctrl.router.folder_lookup.get("ROUTE_101").expect("folder created");
    let folder = ctrl.router.folder(folder_id).unwrap();
    assert_eq!(folder.enabled, Some(true), "a new folder is enabled");
    assert_eq!(folder.expanded, Some(true), "a new folder is expanded");
    assert!(ctrl.router.is_enabled(folder_id));

    let first = ctrl.new_event(wild("Zigzagoon", 2), None, None, Some("ROUTE_101"), true).expect("event added");
    let second = ctrl.new_event(wild("Wurmple", 2), None, None, Some("ROUTE_101"), true).expect("event added");
    assert!(ctrl.router.is_enabled(first));
    assert!(ctrl.router.is_enabled(second));

    // the recorder walks back from the end to find what it just added (trainer updates,
    // item coalescing); a disabled folder would hide the events from it
    assert_eq!(ctrl.get_previous_event(None), Some(second));
    assert_eq!(ctrl.get_previous_event(Some(second)), Some(first));

    // and the events count: the solo mon gained the wild fights' experience
    let init_exp = ctrl.get_init_state().unwrap().solo_pkmn.cur_xp;
    let final_exp = ctrl.get_final_state().unwrap().solo_pkmn.cur_xp;
    assert!(final_exp > init_exp, "events in the new folder are applied to the route ({} -> {})", init_exp, final_exp);

    // the saved bytes carry the flags the same way Python writes them
    let saved = ctrl.router.serialize().unwrap();
    let json = saved.to_string();
    assert!(json.contains("\"Expanded\":true") || json.contains("\"Expanded\": true"), "expanded flag serialised: {}", &json[..json.len().min(400)]);
}

#[test]
fn folder_after_an_event_and_split_folder_keep_the_defaults() {
    let mut ctrl = fresh_controller("Pikachu", "Yellow");
    let a = ctrl.new_event(wild("Pidgey", 3), None, None, None, true).unwrap();
    // "new folder" dialog: insert after the selected event
    ctrl.finalize_new_folder("Route 1", None, Some(a));
    let f = *ctrl.router.folder_lookup.get("Route 1").unwrap();
    assert_eq!((ctrl.router.folder(f).unwrap().enabled, ctrl.router.folder(f).unwrap().expanded), (Some(true), Some(true)));

    let b = ctrl.new_event(wild("Rattata", 2), None, None, Some("Route 1"), true).unwrap();
    let c = ctrl.new_event(wild("Rattata", 3), None, None, Some("Route 1"), true).unwrap();
    ctrl.split_folder_at_current_event(c);
    let split_name = ctrl.get_all_folder_names().into_iter().find(|n| n.starts_with("Route 1 (")).expect("split folder");
    let sid = *ctrl.router.folder_lookup.get(&split_name).unwrap();
    let split = ctrl.router.folder(sid).unwrap();
    assert_eq!((split.enabled, split.expanded), (Some(true), Some(true)));
    assert!(ctrl.router.is_enabled(c), "the moved event stays enabled");
    assert!(ctrl.router.is_enabled(b));
    assert_eq!(ctrl.get_previous_event(None), Some(c));
}
