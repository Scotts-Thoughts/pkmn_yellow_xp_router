//! A Metronome slot shows the damage of the move picked for it in its
//! dropdown -- exactly what that move shows in the same slot -- and nothing
//! until a move is picked.

use std::path::PathBuf;
use std::sync::Arc;

use xpr_app::controller::MainController;
use xpr_calc::battle_summary::{BattleSummary, MoveRenderInfo, SummaryConfig};
use xpr_core::{consts, Paths};
use xpr_data::Registry;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

/// A loaded summary of the route's first trainer fight.
fn setup(route: &str) -> (MainController, BattleSummary) {
    let root = repo_root();
    let registry = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    let mut ctrl = MainController::new(registry, Paths::new(root.clone()));
    ctrl.load_route(&root.join("tests/test_data").join(route));
    let fight = ctrl
        .router
        .all_groups()
        .into_iter()
        .find(|g| ctrl.router.group(*g).unwrap().event_definition.trainer_def.is_some())
        .expect("a trainer fight");
    let mut summary = BattleSummary::new();
    summary.load_from_event(&ctrl.router, Some(fight), &SummaryConfig::default()).unwrap();
    (ctrl, summary)
}

/// `summary` with the first move of the first matchup's attacker replaced.
fn with_first_move(summary: &BattleSummary, ctrl: &MainController, is_player: bool, move_name: &str) -> BattleSummary {
    let mut s = summary.clone();
    let mons = if is_player { &mut s.original_player_mon_list } else { &mut s.original_enemy_mon_list };
    mons[0].move_list[0] = Some(move_name.to_string());
    s.full_refresh(ctrl.gen().as_deref(), &SummaryConfig::default());
    s
}

fn first_move(summary: &BattleSummary, is_player: bool) -> MoveRenderInfo {
    let data = if is_player { &summary.player_move_data } else { &summary.enemy_move_data };
    data[0][0].clone().expect("a first move")
}

fn check_side(is_player: bool, called: &str) {
    let (ctrl, summary) = setup("yellow-pinsir-lv10brock.json");
    let gen = ctrl.gen().unwrap();
    let sc = SummaryConfig::default();

    let mut metronome = with_first_move(&summary, &ctrl, is_player, consts::METRONOME_MOVE_NAME);
    let unpicked = first_move(&metronome, is_player);
    assert_eq!(unpicked.name, consts::METRONOME_MOVE_NAME);
    assert_eq!(unpicked.metronome_selection.as_deref(), Some(""));
    assert_eq!(unpicked.min_damage, -1, "no damage until a move is picked");

    metronome.update_custom_move_data(&gen, &sc, 0, 0, is_player, called);
    let picked = first_move(&metronome, is_player);

    let direct = first_move(&with_first_move(&summary, &ctrl, is_player, called), is_player);
    assert!(direct.max_damage > 0, "{} is a damaging move", called);
    let mut expected = direct;
    expected.name = consts::METRONOME_MOVE_NAME.to_string();
    expected.metronome_selection = Some(called.to_string());
    // (the Mimic options list the enemy's moves, which differ by setup)
    expected.mimic_options = picked.mimic_options.clone();
    expected.metronome_options = picked.metronome_options.clone();
    assert_eq!(picked, expected);
    // gen 1: every move but Metronome and Struggle, after the blank row
    let opts = picked.metronome_options.expect("a Metronome dropdown");
    assert_eq!(opts.len(), 1 + 165 - 2);
    assert_eq!(opts[0], "");
    assert!(opts.iter().any(|m| m == called));

    // the pick is saved with the fight
    let td = metronome.get_partial_trainer_definition().unwrap();
    let side = td.custom_move_data.iter().map(|c| c.side(is_player).get(consts::METRONOME_MOVE_NAME).cloned()).find(|v| v.is_some()).flatten();
    assert_eq!(side.as_deref(), Some(called));

    // the blank row clears it
    metronome.update_custom_move_data(&gen, &sc, 0, 0, is_player, "");
    assert_eq!(first_move(&metronome, is_player), unpicked);
}

#[test]
fn player_metronome_uses_the_picked_move() {
    check_side(true, "Thunderbolt");
}

#[test]
fn enemy_metronome_uses_the_picked_move() {
    check_side(false, "Body Slam");
}

#[test]
fn metronome_cannot_pick_itself_or_struggle() {
    let (ctrl, summary) = setup("yellow-pinsir-lv10brock.json");
    let gen = ctrl.gen().unwrap();
    let sc = SummaryConfig::default();
    let mut metronome = with_first_move(&summary, &ctrl, true, consts::METRONOME_MOVE_NAME);
    for bad in [consts::METRONOME_MOVE_NAME, consts::STRUGGLE_MOVE_NAME, "Not A Move"] {
        metronome.update_custom_move_data(&gen, &sc, 0, 0, true, bad);
        let m = first_move(&metronome, true);
        assert_eq!(m.metronome_selection.as_deref(), Some(""), "{} is not a pick", bad);
        assert_eq!(m.min_damage, -1);
    }
}

#[test]
fn gen2_metronome_cannot_pick_a_move_its_user_knows() {
    let (ctrl, summary) = setup("c-porygon-1-.json");
    let gen = ctrl.gen().unwrap();
    assert_eq!(gen.get_generation(), 2);
    let sc = SummaryConfig::default();
    let mut metronome = with_first_move(&summary, &ctrl, true, consts::METRONOME_MOVE_NAME);
    let own_move = metronome.original_player_mon_list[0].move_list.iter().flatten().find(|m| *m != consts::METRONOME_MOVE_NAME).cloned();
    let own_move = own_move.expect("another move on the player's mon");
    let opts = first_move(&metronome, true).metronome_options.unwrap();
    assert!(!opts.contains(&own_move), "{} is offered", own_move);
    assert!(!opts.iter().any(|m| m == "Counter" || m == "Thief"));

    // a stored pick it can't call counts as no pick
    metronome.update_custom_move_data(&gen, &sc, 0, 0, true, &own_move);
    let m = first_move(&metronome, true);
    assert_eq!(m.metronome_selection.as_deref(), Some(""));
    assert_eq!(m.min_damage, -1);
}
