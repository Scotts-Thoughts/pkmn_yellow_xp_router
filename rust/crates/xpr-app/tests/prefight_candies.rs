//! Pre-fight candies through the real controllers: every click mutates the
//! route immediately, a burst of clicks is one undo step, the battle summary
//! keeps its unsaved selections across the route-change cascade, and the
//! summary matches a fresh load of the same fight.

use std::path::PathBuf;
use std::sync::Arc;

use xpr_app::battle::BattleController;
use xpr_app::controller::MainController;
use xpr_core::{Config, Paths};
use xpr_data::Registry;
use xpr_engine::NodeId;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn setup(route: &str) -> (Config, MainController) {
    let root = repo_root();
    let paths = Paths::new(root.clone());
    // a missing file gives the default config
    let cfg = Config::load(&xpr_app::scratch_config_path());
    let registry = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    let mut ctrl = MainController::new(registry, paths);
    ctrl.load_route(&root.join("tests/test_data").join(route));
    let _ = ctrl.take_signals();
    (cfg, ctrl)
}

fn nth_trainer(ctrl: &MainController, n: usize) -> NodeId {
    ctrl.router.all_groups().into_iter().filter(|g| ctrl.router.group(*g).unwrap().event_definition.trainer_def.is_some()).nth(n).expect("trainer fight")
}

/// The frame's `route_changed` cascade as `EventDetails::handle_route_change` runs it.
fn cascade(cfg: &Config, ctrl: &mut MainController, bc: &mut BattleController, selected: NodeId) {
    let sig = ctrl.take_signals();
    if sig.route_changed && !bc.take_handled_route_change(selected) {
        bc.load_from_event(cfg, ctrl, selected);
    }
}

#[test]
fn clicks_apply_immediately_and_a_burst_is_one_undo_step() {
    let (cfg, mut ctrl) = setup("yellow-pinsir-lv10brock.json");
    let fight = nth_trainer(&ctrl, 2);
    ctrl.select_new_events(vec![fight]);
    let _ = ctrl.take_signals();
    let mut bc = BattleController::new();
    bc.load_from_event(&cfg, &mut ctrl, fight);
    let level_before = bc.get_pkmn_info(0, true).unwrap().attacking_mon_level;
    let bytes_before = ctrl.router.save_bytes().unwrap();
    assert_eq!(bc.get_prefight_candy_count(&ctrl), 0);

    // three quick clicks: 1, 2, 3 candies, the last two coalesced
    bc.update_prefight_candies(&cfg, &mut ctrl, 1, false);
    cascade(&cfg, &mut ctrl, &mut bc, fight);
    assert_eq!(bc.get_prefight_candy_count(&ctrl), 1);
    assert_eq!(bc.get_pkmn_info(0, true).unwrap().attacking_mon_level, level_before + 1, "the summary shows the new level right away");
    bc.update_prefight_candies(&cfg, &mut ctrl, 2, true);
    cascade(&cfg, &mut ctrl, &mut bc, fight);
    bc.update_prefight_candies(&cfg, &mut ctrl, 3, true);
    cascade(&cfg, &mut ctrl, &mut bc, fight);
    assert_eq!(bc.get_prefight_candy_count(&ctrl), 3);
    assert_eq!(bc.get_pkmn_info(0, true).unwrap().attacking_mon_level, level_before + 3);
    assert!(ctrl.has_unsaved_changes());

    // one undo reverts the whole burst
    assert!(ctrl.can_undo());
    ctrl.undo();
    let _ = ctrl.take_signals();
    assert_eq!(ctrl.router.save_bytes().unwrap(), bytes_before);
    assert!(!ctrl.can_undo(), "the burst was a single undo step");
}

#[test]
fn separate_clicks_are_separate_undo_steps() {
    let (cfg, mut ctrl) = setup("yellow-pinsir-lv10brock.json");
    let fight = nth_trainer(&ctrl, 2);
    ctrl.select_new_events(vec![fight]);
    let mut bc = BattleController::new();
    bc.load_from_event(&cfg, &mut ctrl, fight);
    let bytes_before = ctrl.router.save_bytes().unwrap();
    bc.update_prefight_candies(&cfg, &mut ctrl, 1, false);
    cascade(&cfg, &mut ctrl, &mut bc, fight);
    let bytes_one = ctrl.router.save_bytes().unwrap();
    bc.update_prefight_candies(&cfg, &mut ctrl, 2, false);
    cascade(&cfg, &mut ctrl, &mut bc, fight);
    ctrl.undo();
    assert_eq!(ctrl.router.save_bytes().unwrap(), bytes_one);
    ctrl.undo();
    assert_eq!(ctrl.router.save_bytes().unwrap(), bytes_before);
    assert!(!ctrl.can_undo());
}

#[test]
fn unsaved_selections_survive_the_route_change_cascade() {
    let (cfg, mut ctrl) = setup("c-porygon-1-.json");
    let fight = nth_trainer(&ctrl, 3);
    ctrl.select_new_events(vec![fight]);
    let mut bc = BattleController::new();
    bc.load_from_event(&cfg, &mut ctrl, fight);
    // a transient (not yet saved into the event) selection
    bc.update_player_setup_moves(&cfg, &ctrl, vec!["Swords Dance".to_string()]);
    let _ = bc.take_signals();
    assert_eq!(bc.summary.player_setup_move_list, vec!["Swords Dance".to_string()]);

    bc.update_prefight_candies(&cfg, &mut ctrl, 2, false);
    cascade(&cfg, &mut ctrl, &mut bc, fight);
    assert_eq!(bc.get_prefight_candy_count(&ctrl), 2);
    assert_eq!(bc.summary.player_setup_move_list, vec!["Swords Dance".to_string()], "the setup move is kept through the cascade");

    // a later, unrelated route change does reload the fight from the event
    let _ = ctrl.take_signals();
    ctrl.toggle_event_highlight(&[fight]);
    cascade(&cfg, &mut ctrl, &mut bc, fight);
    assert!(bc.summary.player_setup_move_list.iter().all(|m| m.is_empty()), "an unrelated route change reloads from the event as Python does");
}

#[test]
fn summary_after_candies_matches_a_fresh_load() {
    let (cfg, mut ctrl) = setup("f-ditto-tests.json");
    let fight = nth_trainer(&ctrl, 1);
    ctrl.select_new_events(vec![fight]);
    let mut bc = BattleController::new();
    bc.load_from_event(&cfg, &mut ctrl, fight);
    for (target, coalesce) in [(1, false), (2, true), (5, true), (3, false)] {
        bc.update_prefight_candies(&cfg, &mut ctrl, target, coalesce);
        cascade(&cfg, &mut ctrl, &mut bc, fight);
        let mut fresh = BattleController::new();
        fresh.load_from_event(&cfg, &mut ctrl, fight);
        assert_eq!(bc.summary.to_json(), fresh.summary.to_json(), "{} candies", target);
    }
    // back to none: the candy event is removed again
    bc.update_prefight_candies(&cfg, &mut ctrl, 0, false);
    cascade(&cfg, &mut ctrl, &mut bc, fight);
    assert_eq!(bc.get_prefight_candy_count(&ctrl), 0);
    let prev = ctrl.get_previous_event(Some(fight)).unwrap();
    assert!(ctrl.router.group(prev).unwrap().event_definition.rare_candy.is_none());
}
