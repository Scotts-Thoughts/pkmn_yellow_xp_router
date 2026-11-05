//! What a too-short `mon_order` on a saved route costs. The recorder used to
//! write one when it missed a send-out (see `open_issues_2026-11-04.md`, issue 1).
//! It no longer does, but routes saved before the fix can still hold such a list
//! and the reader takes them at face value: this pins down what happens to them.
//! If the reader is ever changed to pad them, this test is the one to change.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use xpr_data::Registry;
use xpr_engine::Router;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn registry() -> Arc<Registry> {
    Arc::new(Registry::new(repo_root().join("raw_pkmn_data"), PathBuf::new()))
}

/// Emerald's Leader Flannery, whose four mons all differ, as the only event.
fn route(mon_order: Value) -> Value {
    let flannery = json!({
        "trainer_name": "Leader Flannery",
        "verbose": false,
        "setup_moves": [],
        "mimic_selection": "",
        "mon_order": mon_order,
    });
    json!({
        "name": "Mudkip",
        "Version": "Emerald",
        "ability": 0,
        "nature": 0,
        "events": [{"Event Folder Name": "ROOT", "Just Notes": "", "events": [
            {"Event Folder Name": "Main", "Just Notes": "", "Expanded": true, "Enabled": true, "events": [
                {"Enabled": true, "Tags": [], "Fight Trainer": flannery}
            ]}
        ]}]
    })
}

/// The mons the fight has, and the solo mon's exp when it is over.
fn fight(tag: &str, mon_order: Value) -> (Vec<String>, i64) {
    let dir = std::env::temp_dir().join(format!("xpr-short-mon-order-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("route.json");
    std::fs::write(&path, serde_json::to_vec(&route(mon_order)).unwrap()).unwrap();
    let mut router = Router::new(registry());
    router.load(&path, false).expect("route loads");
    let groups = router.all_groups();
    let group = router.group(groups[0]).unwrap();
    assert!(group.error_messages.is_empty(), "{:?}", group.error_messages);
    let gen = registry().get_version("Emerald").unwrap();
    let names = group.event_definition.pokemon_list(&gen).unwrap().iter().map(|p| p.name.clone()).collect();
    (names, group.final_state.as_ref().unwrap().solo_pkmn.cur_xp)
}

#[test]
fn a_short_mon_order_on_a_saved_route_loses_a_mon_and_its_exp() {
    let (party_order, full_exp) = fight("empty", json!([]));
    assert_eq!(party_order, ["Numel", "Slugma", "Camerupt", "Torkoal"]);

    // the game sent out slots 0, 2 and 3 and the recorder missed slot 1: it
    // ranked what it saw into [1, 2, 3], so slot 1 (never seen) takes a place and
    // slot 3 (fought) falls off the end
    let (short, short_exp) = fight("short", json!([1, 2, 3]));
    assert_eq!(short, ["Numel", "Slugma", "Camerupt"]);

    let gen = registry().get_version("Emerald").unwrap();
    let torkoal = gen.trainer_db().get_trainer("Leader Flannery").unwrap().pkmn[3].xp;
    assert!(torkoal > 0);
    assert_eq!(full_exp - short_exp, torkoal, "the route is short by exactly Torkoal's exp");

    // the list the recorder writes now: nothing lost
    let (full, exp) = fight("full", json!([1, 4, 2, 3]));
    assert_eq!(full, ["Numel", "Camerupt", "Torkoal", "Slugma"]);
    assert_eq!(exp, full_exp);
}
