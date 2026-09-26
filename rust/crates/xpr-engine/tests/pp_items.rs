//! PP items in the engine: PP Up / PP Max raise a move's PP Ups (validated,
//! always consumed), Ether-type items only have their target checked, and a
//! learned move resets its slot's PP Ups. Current PP itself lives in the
//! app's PP ledger (docs/rust_port/design/pp_tracking/PLAN.md).

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use xpr_core::consts;
use xpr_data::Registry;
use xpr_engine::{EventDefinition, InventoryEventDefinition, LearnMoveEventDefinition, LevelVal, NodeId, Router};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn registry() -> Arc<Registry> {
    Arc::new(Registry::new(repo_root().join("raw_pkmn_data"), PathBuf::new()))
}

fn find(item: &str, n: i64) -> Value {
    EventDefinition::with_item(InventoryEventDefinition::new(item, n, true, false, None)).serialize()
}

fn use_on(item: &str, n: i64, target: Option<&str>) -> Value {
    EventDefinition::with_item(InventoryEventDefinition::use_on(item, n, target)).serialize()
}

fn route(version: &str, mon: &str, events: Vec<Value>) -> Value {
    json!({
        "name": mon,
        "Version": version,
        "events": [{"Event Folder Name": "ROOT", "Just Notes": "", "events": [
            {"Event Folder Name": "Main", "Just Notes": "", "Expanded": true, "Enabled": true, "events": events}
        ]}]
    })
}

fn load(version: &str, mon: &str, events: Vec<Value>) -> Router {
    let mut router = Router::new(registry());
    router.load_value(&route(version, mon, events), false).expect("route loads");
    router
}

fn after(router: &Router, id: NodeId) -> &xpr_engine::SoloPokemon {
    &router.group(id).unwrap().final_state.as_ref().unwrap().solo_pkmn
}

/// The first move the starter knows at the start of the route.
fn first_move(version: &str, mon: &str) -> String {
    let router = load(version, mon, vec![json!({"Enabled": true, "Tags": [], "Just Notes": "x"})]);
    router.init_route_state.as_ref().unwrap().solo_pkmn.move_list[0].clone().expect("a starting move")
}

fn count_in_bag(router: &Router, id: NodeId, item: &str) -> i64 {
    let inv = &router.group(id).unwrap().final_state.as_ref().unwrap().inventory;
    inv.cur_items.iter().filter(|b| b.base_item.name == item).map(|b| b.num).sum()
}

// ---- definition ----------------------------------------------------------------

#[test]
fn target_and_toss_round_trip_and_old_shapes_stay_identical() {
    // no extras: the Python four-element array, byte for byte
    let plain = InventoryEventDefinition::new("Potion", 1, false, false, None);
    assert_eq!(plain.serialize(), json!(["Potion", 1, false, false]));
    let priced = InventoryEventDefinition::new("Potion", 2, true, true, Some(150));
    assert_eq!(priced.serialize(), json!(["Potion", 2, true, true, {"custom_price": 150}]));

    let targeted = InventoryEventDefinition::use_on("Ether", 1, Some("Surf"));
    let raw = targeted.serialize();
    assert_eq!(raw, json!(["Ether", 1, false, false, {consts::TARGET_MOVE_KEY: "Surf"}]));
    assert_eq!(InventoryEventDefinition::deserialize(Some(&raw)), Some(targeted.clone()));
    assert_eq!(targeted.to_string(), "Use Ether x1 on Surf");

    let mut tossed = InventoryEventDefinition::new("Elixir", 1, false, false, None);
    tossed.no_effect = true;
    let raw = tossed.serialize();
    assert_eq!(raw, json!(["Elixir", 1, false, false, {consts::NO_EFFECT_KEY: true}]));
    assert_eq!(InventoryEventDefinition::deserialize(Some(&raw)), Some(tossed.clone()));
    assert_eq!(tossed.to_string(), "Toss Elixir x1");

    // an untargeted use keeps the old label
    assert_eq!(InventoryEventDefinition::use_on("Elixir", 1, None).to_string(), "Use/Drop Elixir x1");
    // the legacy bare-int custom price still loads
    let legacy = json!(["Potion", 1, true, true, 300]);
    assert_eq!(InventoryEventDefinition::deserialize(Some(&legacy)).unwrap().custom_price, Some(300));
}

// ---- PP Up / PP Max ------------------------------------------------------------------

#[test]
fn pp_up_raises_the_target_slot_and_refuses_past_three() {
    let mv = first_move("Emerald", "Torchic");
    let router = load(
        "Emerald",
        "Torchic",
        vec![
            find("PP Up", 4),
            use_on("PP Up", 1, Some(&mv)),
            use_on("PP Up", 3, Some(&mv)),
            find("Rare Candy", 1),
            json!({"Enabled": true, "Tags": [], "Rare Candy": 1}),
        ],
    );
    let g = router.all_groups();
    let first = router.group(g[1]).unwrap();
    assert!(first.error_messages.is_empty(), "{:?}", first.error_messages);
    assert_eq!(first.name, format!("Use PP Up x1 on {}", mv));
    assert_eq!(after(&router, g[1]).pp_ups, [1, 0, 0, 0]);

    // two of the three apply; the third is refused but still consumed
    let second = router.group(g[2]).unwrap();
    assert_eq!(second.error_messages, vec![format!("Ineffective PP Up: {} already has 3 PP Ups", mv)]);
    assert_eq!(after(&router, g[2]).pp_ups, [3, 0, 0, 0]);
    assert_eq!(count_in_bag(&router, g[2], "PP Up"), 0);

    // other mutators carry the PP Ups forward
    assert_eq!(after(&router, g[4]).pp_ups, [3, 0, 0, 0]);
    // and they show in the state record only once set
    let state = router.group(g[0]).unwrap().final_state.as_ref().unwrap().solo_pkmn.serialize();
    assert!(state.get(consts::PP_UPS_KEY).is_none());
    let state = after(&router, g[2]).serialize();
    assert_eq!(state[consts::PP_UPS_KEY], json!([3, 0, 0, 0]));
}

#[test]
fn pp_max_goes_straight_to_three_and_a_second_one_is_refused() {
    let mv = first_move("Emerald", "Torchic");
    let router = load("Emerald", "Torchic", vec![find("PP Max", 2), use_on("PP Max", 1, Some(&mv)), use_on("PP Max", 1, Some(&mv))]);
    let g = router.all_groups();
    assert!(router.group(g[1]).unwrap().error_messages.is_empty());
    assert_eq!(after(&router, g[1]).pp_ups, [3, 0, 0, 0]);
    assert_eq!(router.group(g[2]).unwrap().error_messages, vec![format!("Ineffective PP Max: {} already has 3 PP Ups", mv)]);
    assert_eq!(count_in_bag(&router, g[2], "PP Max"), 0);
}

#[test]
fn missing_targets_warn_and_unknown_targets_error() {
    let router = load(
        "Emerald",
        "Torchic",
        vec![
            find("PP Up", 1),
            use_on("PP Up", 1, None),
            find("Ether", 2),
            use_on("Ether", 1, None),
            use_on("Ether", 1, Some("Surf")),
            find("Elixir", 1),
            use_on("Elixir", 1, None),
        ],
    );
    let g = router.all_groups();
    let pp_up = router.group(g[1]).unwrap();
    assert!(pp_up.error_messages.is_empty());
    assert_eq!(pp_up.warning_messages, vec!["Choose the move PP Up was used on".to_string()]);
    assert!(!pp_up.has_errors());
    assert_eq!(after(&router, g[1]).pp_ups, [0; 4]);

    let ether = router.group(g[3]).unwrap();
    assert_eq!(ether.warning_messages, vec!["Choose the move Ether was used on".to_string()]);
    let unknown = router.group(g[4]).unwrap();
    assert_eq!(unknown.error_messages, vec!["Torchic does not know Surf".to_string()]);

    // an Elixir needs no target
    let elixir = router.group(g[6]).unwrap();
    assert!(elixir.error_messages.is_empty() && elixir.warning_messages.is_empty());
}

#[test]
fn tossed_pp_items_have_no_effect_and_no_warning() {
    let mut tossed = InventoryEventDefinition::new("PP Up", 1, false, false, None);
    tossed.no_effect = true;
    let router = load("Emerald", "Torchic", vec![find("PP Up", 1), EventDefinition::with_item(tossed).serialize()]);
    let g = router.all_groups();
    let grp = router.group(g[1]).unwrap();
    assert!(grp.error_messages.is_empty() && grp.warning_messages.is_empty());
    assert_eq!(after(&router, g[1]).pp_ups, [0; 4]);
    assert_eq!(count_in_bag(&router, g[1], "PP Up"), 0);
}

#[test]
fn learning_a_move_resets_only_that_slots_pp_ups() {
    let start = load("Emerald", "Torchic", vec![json!({"Enabled": true, "Tags": [], "Just Notes": "x"})]);
    let moves = start.init_route_state.as_ref().unwrap().solo_pkmn.move_list.clone();
    let known: Vec<String> = moves.iter().flatten().cloned().collect();
    assert!(known.len() >= 2, "Torchic starts with two moves: {:?}", known);
    let tutor = EventDefinition::with_learn_move(LearnMoveEventDefinition::new(Some("Ember"), Some(0), consts::MOVE_SOURCE_TUTOR, LevelVal::any(), None, true)).serialize();
    let router = load(
        "Emerald",
        "Torchic",
        vec![find("PP Up", 2), use_on("PP Up", 1, Some(&known[0])), use_on("PP Up", 1, Some(&known[1])), tutor],
    );
    let g = router.all_groups();
    assert_eq!(after(&router, g[2]).pp_ups, [1, 1, 0, 0]);
    let learned = after(&router, g[3]);
    assert_eq!(learned.move_list[0].as_deref(), Some("Ember"));
    assert_eq!(learned.pp_ups, [0, 1, 0, 0]);
}

// ---- per-generation rules -----------------------------------------------------------------

#[test]
fn generation_tables() {
    let reg = registry();
    let yellow = reg.get_version("Yellow").unwrap();
    let crystal = reg.get_version("Crystal").unwrap();
    let emerald = reg.get_version("Emerald").unwrap();

    // gen 1-2 cap the per-use bonus at 7: a 40-PP move tops out at 61
    assert_eq!(yellow.max_pp("Tackle", 0), Some(35));
    assert_eq!(yellow.max_pp("Tackle", 1), Some(42));
    assert_eq!(yellow.max_pp("Tackle", 3), Some(56));
    assert_eq!(yellow.max_pp("Growl", 3), Some(61));
    assert_eq!(crystal.max_pp("Growl", 3), Some(61));
    assert_eq!(emerald.max_pp("Growl", 3), Some(64));
    assert_eq!(emerald.max_pp("Hyper Beam", 3), Some(8));

    // the data's spellings resolve
    use xpr_data::{PpAmount, PpItemEffect};
    assert_eq!(yellow.pp_item_effect("Elixer"), Some(PpItemEffect::RestoreAll(PpAmount::Points(10))));
    assert_eq!(yellow.pp_item_effect("Max Elixer"), Some(PpItemEffect::RestoreAll(PpAmount::Full)));
    assert_eq!(yellow.pp_item_effect("PP Max"), None);
    assert_eq!(crystal.pp_item_effect("Pp Up"), Some(PpItemEffect::PpUp));
    assert_eq!(crystal.pp_item_effect("Mysteryberry"), Some(PpItemEffect::RestoreOne(PpAmount::Points(5))));
    assert_eq!(crystal.pp_item_effect("Leppa Berry"), None);
    assert_eq!(emerald.pp_item_effect("Leppa Berry"), Some(PpItemEffect::RestoreOne(PpAmount::Points(10))));
    assert_eq!(emerald.pp_item_effect("PP Max"), Some(PpItemEffect::PpMax));
    assert_eq!(emerald.pp_item_effect("Potion"), None);

    // Sketch cannot take a PP Up from gen 2 on
    assert!(crystal.can_raise_pp("Tackle"));
    assert!(!crystal.can_raise_pp("Sketch"));
    assert!(!emerald.can_raise_pp("Sketch"));
    assert!(emerald.can_raise_pp("Hyper Beam"));

    // locked moves pay PP once per lock
    use xpr_data::PpLock;
    assert_eq!(yellow.pp_lock("Thrash"), PpLock::Lock(3));
    assert_eq!(yellow.pp_lock("Petal Dance"), PpLock::Lock(3));
    assert_eq!(yellow.pp_lock("Rage"), PpLock::OncePerFight);
    assert_eq!(yellow.pp_lock("Wrap"), PpLock::PerTurn, "one gen 1 Wrap use is the whole trap in the calc");
    assert_eq!(crystal.pp_lock("Outrage"), PpLock::Lock(2));
    assert_eq!(crystal.pp_lock("Rollout"), PpLock::PerTurn);
    assert_eq!(crystal.pp_lock("Rage"), PpLock::PerTurn);
    assert_eq!(emerald.pp_lock("Rollout"), PpLock::Lock(5));
    assert_eq!(emerald.pp_lock("Uproar"), PpLock::Lock(2));
    assert!(!crystal.has_pressure_pp_cost());
    assert!(emerald.has_pressure_pp_cost());
}

#[test]
fn gen2_pp_up_uses_the_data_spelling() {
    let mv = first_move("Crystal", "Cyndaquil");
    let router = load("Crystal", "Cyndaquil", vec![find("Pp Up", 1), use_on("Pp Up", 1, Some(&mv))]);
    let g = router.all_groups();
    assert!(router.group(g[1]).unwrap().error_messages.is_empty(), "{:?}", router.group(g[1]).unwrap().error_messages);
    assert_eq!(after(&router, g[1]).pp_ups, [1, 0, 0, 0]);
}
