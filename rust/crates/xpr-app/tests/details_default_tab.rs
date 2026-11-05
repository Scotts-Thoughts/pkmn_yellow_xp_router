//! Starting to record, or opening a replay's route, leaves the Map tab for
//! the tab that suits the selected event, whatever auto-switch says.

use std::path::PathBuf;
use std::sync::Arc;

use xpr_app::controller::MainController;
use xpr_app::event_details::{DetailsActions, EventDetails, BATTLE_SUMMARY_TAB, MAP_TAB, PRE_STATE_TAB};
use xpr_core::{Config, Paths};
use xpr_data::Registry;
use xpr_engine::NodeId;

fn setup() -> (Config, MainController) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap();
    let paths = Paths::new(root.clone());
    let cfg = Config::load(&xpr_app::scratch_config_path());
    let registry = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    let mut ctrl = MainController::new(registry, paths);
    ctrl.load_route(&root.join("tests/test_data/yellow-pinsir-lv10brock.json"));
    let _ = ctrl.take_signals();
    (cfg, ctrl)
}

fn first_group(ctrl: &MainController, battle: bool) -> NodeId {
    let is_battle = |g: &NodeId| {
        let def = &ctrl.router.group(*g).unwrap().event_definition;
        def.trainer_def.is_some() || def.wild_pkmn_info.is_some()
    };
    ctrl.router.all_groups().into_iter().find(|g| is_battle(g) == battle).expect("a matching event")
}

/// The Map tab open and `id` selected, as a route is when recording starts.
fn on_map_with(ctrl: &mut MainController, cfg: &Config, id: NodeId, auto_switch: bool) -> EventDetails {
    ctrl.select_new_events(vec![id]);
    let _ = ctrl.take_signals();
    let mut details = EventDetails::new(cfg);
    details.auto_switch = auto_switch;
    details.set_tab(cfg, MAP_TAB, &mut DetailsActions::default());
    details
}

#[test]
fn recording_a_battle_opens_the_battle_summary() {
    for auto_switch in [true, false] {
        let (cfg, mut ctrl) = setup();
        let fight = first_group(&ctrl, true);
        let mut details = on_map_with(&mut ctrl, &cfg, fight, auto_switch);
        ctrl.set_record_mode(true);
        details.handle_selection(&cfg, &mut ctrl, &mut DetailsActions::default());
        assert_eq!(details.tab, BATTLE_SUMMARY_TAB, "auto-switch {auto_switch}");
        assert!(!details.is_map_home());
    }
}

#[test]
fn recording_any_other_event_opens_the_pre_event_state() {
    for auto_switch in [true, false] {
        let (cfg, mut ctrl) = setup();
        let event = first_group(&ctrl, false);
        let mut details = on_map_with(&mut ctrl, &cfg, event, auto_switch);
        ctrl.set_record_mode(true);
        details.handle_selection(&cfg, &mut ctrl, &mut DetailsActions::default());
        assert_eq!(details.tab, PRE_STATE_TAB, "auto-switch {auto_switch}");
    }
}

#[test]
fn recording_with_nothing_selected_opens_the_pre_event_state() {
    let (cfg, mut ctrl) = setup();
    let mut details = EventDetails::new(&cfg);
    details.set_tab(&cfg, MAP_TAB, &mut DetailsActions::default());
    ctrl.set_record_mode(true);
    details.handle_selection(&cfg, &mut ctrl, &mut DetailsActions::default());
    assert_eq!(details.tab, PRE_STATE_TAB);
}

#[test]
fn a_recorded_non_battle_event_does_not_force_the_battle_summary() {
    let (cfg, mut ctrl) = setup();
    let event = first_group(&ctrl, false);
    let mut details = EventDetails::new(&cfg);
    ctrl.set_record_mode(true);
    details.handle_selection(&cfg, &mut ctrl, &mut DetailsActions::default());
    ctrl.select_new_events(vec![event]);
    details.handle_selection(&cfg, &mut ctrl, &mut DetailsActions::default());
    assert_eq!(details.tab, PRE_STATE_TAB);
}

#[test]
fn a_replay_route_leaves_the_map() {
    let (cfg, ctrl) = setup();
    let mut details = EventDetails::new(&cfg);
    details.set_tab(&cfg, MAP_TAB, &mut DetailsActions::default());
    details.show_default_tab(&cfg, &ctrl, &mut DetailsActions::default());
    assert_eq!(details.tab, PRE_STATE_TAB);
    assert!(!details.is_map_home());
}
