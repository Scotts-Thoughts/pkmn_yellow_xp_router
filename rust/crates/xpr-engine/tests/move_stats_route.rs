//! Move stats in the route file (spec §2.3): an old route saves exactly as it
//! loaded, a new route keeps its stats through load and save, the run total
//! comes from the events still in the route plus the wild battles, and an edit
//! to a trainer event keeps the battle's stats.

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

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("xpr-move-stats-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn trainer(name: &str, move_stats: Option<Value>) -> Value {
    let mut t = json!({"trainer_name": name, "verbose": false, "setup_moves": [], "mimic_selection": ""});
    if let Some(ms) = move_stats {
        t["move_stats"] = ms;
    }
    json!({"Enabled": true, "Tags": [], "Fight Trainer": t})
}

fn route(events: Vec<Value>, run: Option<Value>) -> Value {
    let mut r = json!({
        "name": "Mudkip",
        "Version": "Emerald",
        "ability": 0,
        "nature": 0,
        "events": [{"Event Folder Name": "ROOT", "Just Notes": "", "events": [
            {"Event Folder Name": "Main", "Just Notes": "", "Expanded": true, "Enabled": true, "events": events}
        ]}]
    });
    if let Some(run) = run {
        r["move_stats"] = run;
    }
    r
}

fn load(dir: &PathBuf, name: &str, route: &Value) -> Router {
    let path = dir.join(format!("{name}.json"));
    std::fs::write(&path, serde_json::to_vec(route).unwrap()).unwrap();
    let mut router = Router::new(registry());
    router.load(&path, false).expect("route loads");
    router
}

fn battle_stats(id: &str) -> Value {
    json!({
        "battle_id": id,
        "started_at": "2026-11-24T14:00:00Z",
        "finished_at": "2026-11-24T14:02:11Z",
        "outcome": "won",
        "kind": "trainer",
        "trainer": "Youngster Calvin",
        "player": {"Water Gun": {"uses": 3, "acc_rolls": 3, "hits": 3, "misses": 0, "expected_hits": 3.0, "hit_variance": 0.0, "interest": 2}},
        "enemy": {"Tackle": {"uses": 2, "acc_rolls": 2, "hits": 1, "misses": 1, "expected_hits": 1.9, "hit_variance": 0.095, "interest": 1}}
    })
}

#[test]
fn an_old_route_saves_without_move_stats() {
    let dir = temp_dir("old");
    let r = load(&dir, "old", &route(vec![trainer("Youngster Calvin", None)], None));
    assert!(!r.has_move_stats());
    let saved = r.serialize().unwrap();
    assert!(saved.get("move_stats").is_none());
    let text = serde_json::to_string(&saved).unwrap();
    assert!(!text.contains("move_stats"), "an old route gains no move_stats keys");
}

#[test]
fn a_new_route_keeps_its_stats_and_total() {
    let dir = temp_dir("new");
    let run = json!({
        "format": 1,
        "run_id": "run-1",
        "game": "Emerald",
        "solo_species": "Mudkip",
        "recorded_with": "test",
        "wild": {"player": {"Water Gun": {"uses": 1, "acc_rolls": 1, "hits": 1, "expected_hits": 1.0, "interest": 2}}, "enemy": {}},
        "reset_attempts": [battle_stats("reset-1")]
    });
    let r = load(&dir, "new", &route(vec![trainer("Youngster Calvin", Some(battle_stats("b-1")))], Some(run)));
    assert!(r.has_move_stats());
    let total = r.move_stats_total();
    assert_eq!(total.player.moves["Water Gun"].uses, 4, "3 in the battle + 1 wild; the reset attempt is not in the total");
    assert_eq!(total.enemy.moves["Tackle"].misses, 1);

    let saved = r.serialize().unwrap();
    assert_eq!(saved["move_stats"]["run_id"], json!("run-1"));
    assert_eq!(saved["move_stats"]["total"]["player"]["Water Gun"]["uses"], json!(4));
    assert_eq!(saved["move_stats"]["reset_attempts"].as_array().unwrap().len(), 1);

    // load what was saved: the same again
    let r2 = load(&dir, "again", &saved);
    let saved2 = r2.serialize().unwrap();
    assert_eq!(saved, saved2);
    let ev = &saved2["events"].to_string();
    assert!(ev.contains("b-1"), "the battle's stats stay on its trainer event");
}

#[test]
fn editing_a_trainer_event_keeps_its_stats() {
    let dir = temp_dir("edit");
    let mut r = load(&dir, "edit", &route(vec![trainer("Youngster Calvin", Some(battle_stats("b-2")))], None));
    let id = r.all_groups().into_iter().find(|g| r.group(*g).map(|g| g.event_definition.trainer_def.is_some()).unwrap_or(false)).expect("a trainer event");
    let mut def = r.group(id).unwrap().event_definition.clone();
    def.trainer_def.as_mut().unwrap().move_stats = None;
    def.notes = "edited".into();
    r.replace_event_group(id, def).unwrap();
    let td = r.group(id).unwrap().event_definition.trainer_def.clone().unwrap();
    assert_eq!(td.move_stats.map(|m| m.battle_id), Some("b-2".to_string()));
}
