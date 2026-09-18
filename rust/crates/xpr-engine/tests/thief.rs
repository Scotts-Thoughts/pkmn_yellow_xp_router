//! Thief / Covet in trainer fights: the enemy's held item lands in the solo
//! mon's held slot (never straight into the bag), it can then be taken off
//! and sold, the flag survives a save / reload, and the steal is refused
//! the way the games refuse it (already holding something, nothing to
//! steal).

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use xpr_core::consts;
use xpr_data::Registry;
use xpr_engine::{Router, TrainerEventDefinition};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn registry() -> Arc<Registry> {
    Arc::new(Registry::new(repo_root().join("raw_pkmn_data"), PathBuf::new()))
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("xpr-thief-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Emerald's Rich Boy Winston: one Zigzagoon holding a Nugget.
fn winston(thief_mons: Value) -> Value {
    json!({
        "trainer_name": "Rich Boy Winston",
        "verbose": false,
        "setup_moves": [],
        "mimic_selection": "",
        "thief_mons": thief_mons,
    })
}

fn route(events: Vec<Value>) -> Value {
    json!({
        "name": "Mudkip",
        "Version": "Emerald",
        "ability": 0,
        "nature": 0,
        "events": [{"Event Folder Name": "ROOT", "Just Notes": "", "events": [
            {"Event Folder Name": "Main", "Just Notes": "", "Expanded": true, "Enabled": true, "events": events}
        ]}]
    })
}

fn load(dir: &PathBuf, name: &str, route: &Value) -> Router {
    let path = dir.join(format!("{name}.json"));
    std::fs::write(&path, serde_json::to_vec(route).unwrap()).unwrap();
    let mut router = Router::new(registry());
    router.load(&path, false).expect("route loads");
    router
}

fn bag_count(router: &Router, group_idx: usize, item: &str) -> i64 {
    let g = router.group(router.all_groups()[group_idx]).unwrap();
    g.final_state.as_ref().unwrap().inventory.cur_items.iter().filter(|x| x.base_item.name == item).map(|x| x.num).sum()
}

fn held(router: &Router, group_idx: usize) -> Option<String> {
    let g = router.group(router.all_groups()[group_idx]).unwrap();
    g.final_state.as_ref().unwrap().solo_pkmn.held_item.clone()
}

fn money(router: &Router, group_idx: usize) -> i64 {
    let g = router.group(router.all_groups()[group_idx]).unwrap();
    g.final_state.as_ref().unwrap().inventory.cur_money
}

#[test]
fn stolen_nugget_is_held_then_sold() {
    let dir = temp_dir("sell");
    let router = load(
        &dir,
        "route",
        &route(vec![
            json!({"Enabled": true, "Tags": [], "Fight Trainer": winston(json!([0]))}),
            json!({"Enabled": true, "Tags": [], "Hold Item": [null, false]}),
            json!({"Enabled": true, "Tags": [], "Inventory Event": ["Nugget", 1, false, true]}),
        ]),
    );
    let groups = router.all_groups();
    assert_eq!(groups.len(), 3);
    let fight = router.group(groups[0]).unwrap();
    assert!(fight.error_messages.is_empty(), "{:?}", fight.error_messages);
    let init_money = fight.init_state.as_ref().unwrap().inventory.cur_money;
    let winston_money = registry().get_version("Emerald").unwrap().trainer_db().get_trainer("Rich Boy Winston").unwrap().money;

    // the fight: the Nugget is held, not bagged, and the prize money is unchanged
    assert_eq!(held(&router, 0).as_deref(), Some("Nugget"));
    assert_eq!(bag_count(&router, 0, "Nugget"), 0);
    assert_eq!(money(&router, 0), init_money + winston_money);
    let item = router.item(fight.event_items[0]).unwrap();
    assert!(item.thief);
    assert_eq!(item.name, "Rich Boy Winston: Zigzagoon (stole Nugget)");

    // taking it off puts it in the bag
    let unequip = router.group(groups[1]).unwrap();
    assert!(unequip.error_messages.is_empty(), "{:?}", unequip.error_messages);
    assert_eq!(held(&router, 1), None);
    assert_eq!(bag_count(&router, 1, "Nugget"), 1);

    // and it sells for half its purchase price
    let sale = router.group(groups[2]).unwrap();
    assert!(sale.error_messages.is_empty(), "{:?}", sale.error_messages);
    assert_eq!(bag_count(&router, 2, "Nugget"), 0);
    assert_eq!(money(&router, 2), init_money + winston_money + 5000);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn selling_without_the_steal_is_an_error() {
    let dir = temp_dir("nosteal");
    let router = load(
        &dir,
        "route",
        &route(vec![
            json!({"Enabled": true, "Tags": [], "Fight Trainer": winston(Value::Null)}),
            json!({"Enabled": true, "Tags": [], "Inventory Event": ["Nugget", 1, false, true]}),
        ]),
    );
    let groups = router.all_groups();
    assert_eq!(held(&router, 0), None);
    assert!(!router.item(router.group(groups[0]).unwrap().event_items[0]).unwrap().thief);
    assert!(!router.group(groups[1]).unwrap().error_messages.is_empty(), "sold a Nugget that was never stolen");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn steal_is_refused_when_already_holding() {
    let dir = temp_dir("holding");
    let router = load(
        &dir,
        "route",
        &route(vec![
            json!({"Enabled": true, "Tags": [], "Inventory Event": ["Oran Berry", 1, true, false]}),
            json!({"Enabled": true, "Tags": [], "Hold Item": ["Oran Berry", false]}),
            json!({"Enabled": true, "Tags": [], "Fight Trainer": winston(json!([0]))}),
        ]),
    );
    let fight = router.group(router.all_groups()[2]).unwrap();
    assert_eq!(fight.error_messages.len(), 1, "{:?}", fight.error_messages);
    assert!(fight.error_messages[0].contains("already holding Oran Berry"), "{}", fight.error_messages[0]);
    // the fight still happens, the Oran Berry stays and nothing was stolen
    assert_eq!(held(&router, 2).as_deref(), Some("Oran Berry"));
    assert_eq!(bag_count(&router, 2, "Nugget"), 0);
    assert!(router.group(router.all_groups()[2]).unwrap().final_state.as_ref().unwrap().solo_pkmn.cur_xp > fight.init_state.as_ref().unwrap().solo_pkmn.cur_xp);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn steal_from_an_empty_handed_mon_is_an_error() {
    let dir = temp_dir("empty");
    // Youngster Calvin's Poochyena holds nothing
    let router = load(
        &dir,
        "route",
        &route(vec![json!({"Enabled": true, "Tags": [], "Fight Trainer": {
            "trainer_name": "Youngster Calvin",
            "verbose": false,
            "setup_moves": [],
            "mimic_selection": "",
            "thief_mons": [0],
        }})]),
    );
    let fight = router.group(router.all_groups()[0]).unwrap();
    assert_eq!(fight.error_messages.len(), 1, "{:?}", fight.error_messages);
    assert!(fight.error_messages[0].contains("no held item"), "{}", fight.error_messages[0]);
    assert_eq!(held(&router, 0), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn only_the_flagged_mon_is_robbed() {
    let dir = temp_dir("multi");
    // a flag on an index the trainer does not have steals nothing
    let router = load(
        &dir,
        "route",
        &route(vec![json!({"Enabled": true, "Tags": [], "Fight Trainer": winston(json!([5]))})]),
    );
    let fight = router.group(router.all_groups()[0]).unwrap();
    assert!(fight.error_messages.is_empty(), "{:?}", fight.error_messages);
    assert_eq!(held(&router, 0), None);
    assert!(!router.item(fight.event_items[0]).unwrap().thief);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn flag_round_trips_and_is_absent_when_unset() {
    let dir = temp_dir("save");
    let router = load(
        &dir,
        "route",
        &route(vec![
            json!({"Enabled": true, "Tags": [], "Fight Trainer": winston(json!([0]))}),
            json!({"Enabled": true, "Tags": [], "Fight Trainer": winston(Value::Null)}),
        ]),
    );
    let saved = router.serialize().unwrap();
    let text = serde_json::to_string(&saved).unwrap();
    assert_eq!(text.matches(consts::THIEF_MONS_KEY).count(), 1, "the unset fight must not carry the key: {text}");

    let saved_path = dir.join("saved.json");
    std::fs::write(&saved_path, text.as_bytes()).unwrap();
    let mut reloaded = Router::new(registry());
    reloaded.load(&saved_path, false).expect("saved route loads");
    let groups = reloaded.all_groups();
    let first = reloaded.group(groups[0]).unwrap();
    assert_eq!(first.event_definition.trainer_def.as_ref().unwrap().thief_mons, vec![0]);
    assert_eq!(held(&reloaded, 0).as_deref(), Some("Nugget"));
    // the second Winston cannot be robbed: the Nugget is still held
    let second = reloaded.group(groups[1]).unwrap();
    assert!(second.event_definition.trainer_def.as_ref().unwrap().thief_mons.is_empty());
    assert_eq!(held(&reloaded, 1).as_deref(), Some("Nugget"));

    // the definition's own (de)serialization
    let mut td = TrainerEventDefinition::new("Rich Boy Winston");
    td.thief_mons = vec![1, 0];
    let raw = td.serialize();
    let back = TrainerEventDefinition::deserialize(Some(&raw)).unwrap().unwrap();
    assert_eq!(back.thief_mons, vec![1, 0]);
    assert!(TrainerEventDefinition::new("x").serialize().get(consts::THIEF_MONS_KEY).is_none());

    let _ = std::fs::remove_dir_all(&dir);
}
