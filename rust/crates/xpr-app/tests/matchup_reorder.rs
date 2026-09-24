//! Dragging an enemy mon to a new slot in the battle summary rewrites the
//! fight's `mon_order`; a later battle-summary change (stage modifiers) must
//! not write the old order back through the delayed save.

use std::path::PathBuf;
use std::sync::Arc;

use xpr_app::controller::MainController;
use xpr_app::event_details::{DetailsActions, EventDetails};
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
    let cfg = Config::load(&root.join("rust/target/no-such-config.json"));
    let registry = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    let mut ctrl = MainController::new(registry, paths);
    ctrl.load_route(&root.join("tests/test_data").join(route));
    let _ = ctrl.take_signals();
    (cfg, ctrl)
}

/// The first trainer fight with at least two enemy mons.
fn multi_mon_trainer(ctrl: &MainController) -> NodeId {
    let gen = ctrl.gen().unwrap();
    ctrl.router
        .all_groups()
        .into_iter()
        .find(|g| {
            let def = &ctrl.router.group(*g).unwrap().event_definition;
            def.trainer_def.is_some() && def.pokemon_list(&gen).map(|l| l.len() >= 2).unwrap_or(false)
        })
        .expect("a trainer fight with two or more mons")
}

fn mon_order(ctrl: &MainController, id: NodeId) -> Vec<i64> {
    ctrl.router.group(id).unwrap().event_definition.trainer_def.as_ref().unwrap().mon_order.clone()
}

/// The frame's signal handling as `App` runs it for the details panel.
fn frame(cfg: &Config, ctrl: &mut MainController, details: &mut EventDetails) {
    let sig = ctrl.take_signals();
    if sig.route_changed {
        details.handle_route_change(cfg, ctrl);
    }
    details.drain_battle_signals(cfg, ctrl);
}

#[test]
fn reorder_survives_a_later_stage_modifier_change() {
    let (cfg, mut ctrl) = setup("yellow-pinsir-lv10brock.json");
    let fight = multi_mon_trainer(&ctrl);
    ctrl.select_new_events(vec![fight]);
    let _ = ctrl.take_signals();
    let mut details = EventDetails::new(&cfg);
    details.handle_selection(&cfg, &mut ctrl, &mut DetailsActions::default());
    frame(&cfg, &mut ctrl, &mut details);
    let before = mon_order(&ctrl, fight);

    details.handle_matchup_reorder(&cfg, &mut ctrl, 0, 1);
    frame(&cfg, &mut ctrl, &mut details);
    let reordered = mon_order(&ctrl, fight);
    assert_ne!(reordered, before, "the drag rewrote mon_order");

    // a stage modifier schedules the delayed save of the whole event
    details.bc.update_stat_stage_setup(&cfg, &ctrl, 0, 0, true, "1");
    frame(&cfg, &mut ctrl, &mut details);
    details.force_and_clear_event_update(&cfg, &mut ctrl);
    frame(&cfg, &mut ctrl, &mut details);
    assert_eq!(mon_order(&ctrl, fight), reordered, "the delayed save kept the new order");
}
