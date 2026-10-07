//! The quick-add popover's "Elite4" / "Elite4 + Champ" buttons: one folder of
//! the league in battle order after the selection, undone in one step.

use std::path::PathBuf;
use std::sync::Arc;

use xpr_app::controller::MainController;
use xpr_core::Paths;
use xpr_data::Registry;
use xpr_engine::NodeId;

fn setup(route: &str) -> MainController {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap();
    let registry = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    let mut ctrl = MainController::new(registry, Paths::new(root.clone()));
    ctrl.load_route(&root.join("tests/test_data").join(route));
    let _ = ctrl.take_signals();
    ctrl
}

fn folder_trainers(ctrl: &MainController, last: NodeId) -> (String, Vec<String>) {
    let f = ctrl.router.folder(ctrl.router.parent_of(last).unwrap()).unwrap();
    let names = f.children.iter().map(|c| ctrl.router.group(*c).unwrap().event_definition.trainer_def.as_ref().unwrap().trainer_name.clone()).collect();
    (f.name.clone(), names)
}

#[test]
fn crystal_league_folders_are_in_battle_order_and_undo_in_one_step() {
    let mut ctrl = setup("c-porygon-1-.json");
    let anchor = *ctrl.router.all_groups().first().unwrap();
    ctrl.select_new_events(vec![anchor]);
    let before = ctrl.router.save_bytes();

    let last = ctrl.add_league_folder(false).expect("folder added");
    assert_eq!(folder_trainers(&ctrl, last), ("Elite Four".to_string(), ["Elite Four Will", "Elite Four Koga", "Elite Four Bruno", "Elite Four Karen"].map(String::from).to_vec()));
    assert_eq!(ctrl.get_single_selected_event_id(true), Some(last));
    ctrl.undo();
    assert_eq!(ctrl.router.save_bytes(), before, "one undo removes the whole folder");

    // undo rebuilds the tree, with new ids
    let anchor = *ctrl.router.all_groups().first().unwrap();
    ctrl.select_new_events(vec![anchor]);
    let last = ctrl.add_league_folder(true).unwrap();
    let (name, names) = folder_trainers(&ctrl, last);
    assert_eq!(name, "Elite Four + Champion");
    assert_eq!(names.last().map(String::as_str), Some("Champion Lance"));
    // repeating it makes another folder
    let again = ctrl.add_league_folder(true).unwrap();
    assert_eq!(folder_trainers(&ctrl, again).0, "Elite Four + Champion Trip:2");
}

#[test]
fn a_starter_dependent_champion_is_the_one_the_route_fights() {
    let mut ctrl = setup("yellow-pinsir-lv10brock.json");
    let lineup = ctrl.league_lineup(true).unwrap();
    assert_eq!(lineup, ["Lorelei 1", "Bruno 1", "Agatha 1", "Lance 1", "Rival3 Jolteon"], "first alternative when the route has none");
    let anchor = *ctrl.router.all_groups().last().unwrap();
    ctrl.select_new_events(vec![anchor]);
    ctrl.add_trainer_fight_from_map("Rival3 Vaporeon").unwrap();
    assert_eq!(ctrl.league_lineup(true).unwrap()[4], "Rival3 Vaporeon");
}
