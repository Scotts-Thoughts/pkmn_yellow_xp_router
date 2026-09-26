//! The PP ledger: fight spends, refills, PP items, PP Ups and the learn-move
//! rules, walked over real routes (docs/rust_port/design/pp_tracking/PLAN.md).
//!
//! The routes turn level-up learning off (`"Learn Levelup Move": []`) and
//! teach Hyper Beam (5 PP) as the only damaging move, so every weak wild mon
//! costs exactly one PP.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use xpr_app::pp_ledger::{PpLedger, PpSnapshot};
use xpr_calc::battle_summary::SummaryConfig;
use xpr_core::consts;
use xpr_data::Registry;
use xpr_engine::{EventDefinition, InventoryEventDefinition, LearnMoveEventDefinition, LevelVal, NodeId, Router, WildPkmnEventDefinition};

fn registry() -> Arc<Registry> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()))
}

fn ev(def: EventDefinition) -> Value {
    def.serialize()
}

fn tutor(mv: Option<&str>, slot: i64) -> Value {
    ev(EventDefinition::with_learn_move(LearnMoveEventDefinition::new(mv, Some(slot), consts::MOVE_SOURCE_TUTOR, LevelVal::any(), None, true)))
}

fn machine(mv: &str, slot: i64, tm: &str) -> Value {
    ev(EventDefinition::with_learn_move(LearnMoveEventDefinition::new(Some(mv), Some(slot), tm, LevelVal::any(), None, true)))
}

fn wild(name: &str, level: i64, n: i64) -> Value {
    ev(EventDefinition::with_wild(WildPkmnEventDefinition::new(name, level, n, false)))
}

fn find(item: &str, n: i64) -> Value {
    ev(EventDefinition::with_item(InventoryEventDefinition::new(item, n, true, false, None)))
}

fn use_on(item: &str, target: Option<&str>) -> Value {
    ev(EventDefinition::with_item(InventoryEventDefinition::use_on(item, 1, target)))
}

fn heal() -> Value {
    ev(EventDefinition::with_heal("Oldale Town"))
}

fn notes() -> Value {
    json!({"Enabled": true, "Tags": [], "Just Notes": "x"})
}

fn load(version: &str, mon: &str, events: Vec<Value>) -> Router {
    let route = json!({
        "name": mon,
        "Version": version,
        "Learn Levelup Move": [],
        "events": [{"Event Folder Name": "ROOT", "Just Notes": "", "events": [
            {"Event Folder Name": "Main", "Just Notes": "", "Expanded": true, "Enabled": true, "events": events}
        ]}]
    });
    let mut router = Router::new(registry());
    router.load_value(&route, false).expect("route loads");
    router
}

fn ledger(router: &Router) -> PpLedger {
    let mut l = PpLedger::new();
    assert!(l.ensure(router, 1, &SummaryConfig::default()));
    l
}

fn pp(l: &PpLedger, id: NodeId, slot: usize) -> (i64, i64) {
    let s: &PpSnapshot = l.before(id).expect("a snapshot");
    let s = s.slots[slot].as_ref().expect("a move in the slot");
    (s.cur, s.max)
}

/// Hyper Beam in slot 0, nothing else.
fn beam_only() -> Vec<Value> {
    vec![tutor(Some("Hyper Beam"), 0), tutor(None, 1)]
}

#[test]
fn fights_spend_and_heals_and_blackouts_refill() {
    let mut events = beam_only();
    events.extend([wild("Zigzagoon", 2, 3), heal(), wild("Zigzagoon", 2, 1), ev(EventDefinition::with_blackout()), notes()]);
    let router = load(consts::EMERALD_VERSION, "Mudkip", events);
    let g = router.all_groups();
    let l = ledger(&router);
    assert_eq!(pp(&l, g[2], 0), (5, 5));
    assert_eq!(l.spend(g[2]), Some([3, 0, 0, 0]), "one Hyper Beam per Zigzagoon");
    assert_eq!(pp(&l, g[3], 0), (2, 5), "before the heal");
    assert_eq!(pp(&l, g[4], 0), (5, 5), "after the heal");
    assert_eq!(pp(&l, g[5], 0), (4, 5));
    assert_eq!(pp(&l, g[6], 0), (5, 5), "a blackout refills too");
    // each fight item carries its own matchup
    let items = &router.group(g[2]).unwrap().event_items;
    assert_eq!(pp(&l, items[1], 0), (4, 5));
    assert_eq!(l.spend(items[1]), Some([1, 0, 0, 0]));
    // the tooltip history starts at the last refill
    let hist = l.history(g[5], 0);
    assert_eq!(hist[0].amount, None);
    assert!(hist[0].text.starts_with("Full at PkmnCenter Heal"), "{:?}", hist);
    assert_eq!(hist.len(), 2);
    assert_eq!(hist[1].amount, Some(-1));
}

#[test]
fn pp_goes_negative_and_restores_are_arithmetic() {
    let mut events = beam_only();
    events.extend([
        wild("Zigzagoon", 2, 11),
        find("Ether", 1),
        use_on("Ether", Some("Hyper Beam")),
        find("Elixir", 1),
        use_on("Elixir", None),
        notes(),
    ]);
    let router = load(consts::EMERALD_VERSION, "Mudkip", events);
    let g = router.all_groups();
    let l = ledger(&router);
    assert_eq!(pp(&l, g[3], 0), (-6, 5));
    assert!(l.messages(&router, g[3]).iter().any(|m| m.contains("Hyper Beam is at \u{2212}6 PP before this event: heal first")));
    // -6 + 10 = 4: the shortfall stays visible
    assert_eq!(pp(&l, g[5], 0), (4, 5));
    // an untargeted Elixir counts as used, capped at the max
    assert_eq!(pp(&l, g[7], 0), (5, 5));
}

#[test]
fn a_restore_on_a_full_move_is_noted() {
    let mut events = beam_only();
    events.extend([find("Ether", 1), use_on("Ether", Some("Hyper Beam")), notes()]);
    let router = load(consts::EMERALD_VERSION, "Mudkip", events);
    let g = router.all_groups();
    let l = ledger(&router);
    assert_eq!(l.notes(g[3]), &["Hyper Beam was already full; the game would refuse this Ether".to_string()]);
    assert_eq!(pp(&l, g[4], 0), (5, 5));
}

#[test]
fn pp_ups_raise_max_and_current_and_learning_resets() {
    let mut events = beam_only();
    events.extend([
        wild("Zigzagoon", 2, 2),
        find("PP Up", 1),
        use_on("PP Up", Some("Hyper Beam")),
        find("PP Max", 1),
        use_on("PP Max", Some("Hyper Beam")),
        heal(),
        tutor(Some("Tackle"), 0),
        notes(),
    ]);
    let router = load(consts::EMERALD_VERSION, "Mudkip", events);
    let g = router.all_groups();
    let l = ledger(&router);
    assert_eq!(pp(&l, g[4], 0), (3, 5));
    // PP Up: 5 -> 6 max, and the current PP rises with it
    assert_eq!(pp(&l, g[5], 0), (4, 6));
    assert_eq!(l.before(g[5]).unwrap().slots[0].as_ref().unwrap().pp_ups, 1);
    // PP Max: 6 -> 8
    assert_eq!(pp(&l, g[7], 0), (6, 8));
    assert_eq!(pp(&l, g[8], 0), (8, 8));
    // a tutor move over it starts full, without the PP Ups
    let tackle = l.before(g[9]).unwrap().slots[0].clone().unwrap();
    assert_eq!((tackle.move_name.as_str(), tackle.cur, tackle.max, tackle.pp_ups), ("Tackle", 35, 35, 0));
}

#[test]
fn gen5_machines_keep_the_old_moves_pp() {
    let events = vec![
        tutor(Some("Hyper Beam"), 0),
        tutor(None, 1),
        tutor(None, 2),
        tutor(None, 3),
        wild("Patrat", 2, 3),
        // TM over Hyper Beam (2 PP left): keeps 2
        machine("Thunderbolt", 0, "TM24"),
        wild("Patrat", 2, 4),
        // negative carries over too, and an empty slot fills up
        machine("Ice Beam", 0, "TM13"),
        machine("Flamethrower", 1, "TM35"),
        // a tutor move starts full
        tutor(Some("Tackle"), 2),
        notes(),
    ];
    let router = load(consts::BLACK_VERSION, "Tepig", events);
    let g = router.all_groups();
    let l = ledger(&router);
    assert_eq!(pp(&l, g[5], 0), (2, 5));
    let tb = l.before(g[6]).unwrap().slots[0].clone().unwrap();
    assert_eq!((tb.move_name.as_str(), tb.cur, tb.max), ("Thunderbolt", 2, 15));
    let spent = l.spend(g[6]).unwrap()[0];
    assert!(spent >= 4, "four more Patrat");
    assert_eq!(pp(&l, g[7], 0).0, 2 - spent);
    let after = l.before(g[9]).unwrap();
    assert_eq!(after.slots[0].as_ref().map(|s| (s.move_name.as_str(), s.cur)), Some(("Ice Beam", 2 - spent)));
    assert_eq!(after.slots[1].as_ref().map(|s| (s.move_name.as_str(), s.cur)), Some(("Flamethrower", 15)));
    assert_eq!(l.before(g[10]).unwrap().slots[2].as_ref().map(|s| s.cur), Some(35));

    // capped at the new move's max
    let events = vec![tutor(Some("Tackle"), 0), machine("Hyper Beam", 0, "TM68"), notes()];
    let router = load(consts::BLACK_VERSION, "Tepig", events);
    let l = ledger(&router);
    let g = router.all_groups();
    assert_eq!(pp(&l, g[2], 0), (5, 5));
}

#[test]
fn earlier_gens_give_a_machine_move_full_pp() {
    let mut events = beam_only();
    events.extend([wild("Zigzagoon", 2, 3), machine("Water Pulse", 0, "TM03"), notes()]);
    let router = load(consts::EMERALD_VERSION, "Mudkip", events);
    let g = router.all_groups();
    let l = ledger(&router);
    assert_eq!(pp(&l, g[4], 0), (20, 20));
}

#[test]
fn disabled_fights_spend_nothing() {
    let mut events = beam_only();
    let mut fight = wild("Zigzagoon", 2, 3);
    fight["Enabled"] = json!(false);
    events.extend([fight, notes()]);
    let router = load(consts::EMERALD_VERSION, "Mudkip", events);
    let g = router.all_groups();
    let l = ledger(&router);
    assert_eq!(pp(&l, g[3], 0), (5, 5));
    assert_eq!(l.spend(g[2]), None);
}

#[test]
fn ensure_is_free_until_something_changes_and_reuses_fights() {
    let mut events = beam_only();
    events.extend([wild("Zigzagoon", 2, 2), wild("Poochyena", 2, 2), notes()]);
    let mut router = load(consts::EMERALD_VERSION, "Mudkip", events);
    let mut l = PpLedger::new();
    let cfg = SummaryConfig::default();
    assert!(l.ensure(&router, 1, &cfg));
    assert_eq!(l.last_computed, 2);
    assert!(!l.ensure(&router, 1, &cfg), "same revision and config: nothing to do");

    // a change at the end of the route: a rebuild, but no fight recomputed
    let last = *router.all_groups().last().unwrap();
    router.replace_event_group(last, EventDefinition::notes_only("changed")).unwrap();
    assert!(l.ensure(&router, 2, &cfg));
    assert_eq!(l.last_computed, 0);

    // another strategy: every fight again
    let other = SummaryConfig { player_strategy: consts::HIGHLIGHT_GUARANTEED_KILL.to_string(), ..SummaryConfig::default() };
    assert!(l.ensure(&router, 2, &other));
    assert_eq!(l.last_computed, 2);
}

#[test]
fn banner_names_a_fight_that_overdraws_a_move() {
    let mut events = beam_only();
    events.extend([wild("Zigzagoon", 2, 3), wild("Zigzagoon", 2, 4), notes()]);
    let router = load(consts::EMERALD_VERSION, "Mudkip", events);
    let g = router.all_groups();
    let l = ledger(&router);
    let msgs = l.messages(&router, g[3]);
    assert!(msgs.iter().any(|m| m == "This fight needs Hyper Beam ×4, but it has 2 PP left"), "{:?}", msgs);
    // one matchup of it
    let item = router.group(g[3]).unwrap().event_items[0];
    assert!(l.messages(&router, item).is_empty(), "{:?}", l.messages(&router, item));
}
