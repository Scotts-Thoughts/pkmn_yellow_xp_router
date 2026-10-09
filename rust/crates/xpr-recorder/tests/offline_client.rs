//! `OfflineClient` hands a session exactly what the GameHook client would:
//! one delivery per registration of a watched path whose value changed, the
//! store updated for every change (watched or not), `on_idle` after each
//! batch, and the recorder clock on the replay's time.

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use xpr_recorder::clock;
use xpr_recorder::gamehook::{GameHookProperty, PropertyStore, SessionEvents};
use xpr_recorder::offline::OfflineClient;

#[derive(Default)]
struct Seen {
    changes: Vec<(String, Value, Value)>,
    idles: usize,
    connected: bool,
}

struct Watcher {
    seen: Arc<Mutex<Seen>>,
    watch: Vec<String>,
}

impl SessionEvents for Watcher {
    fn on_connected(&mut self) {
        self.seen.lock().unwrap().connected = true;
    }
    fn on_connection_error(&mut self) {}
    fn on_disconnected(&mut self) {}
    fn on_game_hook_error(&mut self, _err: &str) {}
    fn on_driver_error(&mut self, _err: &str) {}
    fn on_mapper_loaded(&mut self, _store: &PropertyStore) -> Vec<String> {
        self.watch.clone()
    }
    fn on_mapper_load_error(&mut self, _err: &str) {}
    fn on_property_changed(&mut self, _store: &PropertyStore, new: &GameHookProperty, old: &GameHookProperty) {
        self.seen.lock().unwrap().changes.push((new.path.clone(), old.value.clone(), new.value.clone()));
    }
    fn on_idle(&mut self, _store: &PropertyStore) {
        self.seen.lock().unwrap().idles += 1;
    }
}

fn mapper() -> Value {
    json!({
        "meta": {"gameName": "Pokemon Yellow"},
        "glossary": {},
        "properties": [
            {"path": "player.team.0.level", "value": 5, "bytes": [5], "address": 100},
            {"path": "game_time.seconds", "value": 0, "bytes": [0], "address": 200},
            {"path": "bag.money", "value": 3000, "bytes": [0, 48, 0], "address": 300},
        ]
    })
}

fn change(path: &str, value: Value, fields: &[&str]) -> Value {
    json!({"path": path, "address": null, "value": value, "bytes": null, "isFrozen": false, "fieldsChanged": fields})
}

#[test]
fn batches_reach_the_session_like_gamehook_messages() {
    let seen = Arc::new(Mutex::new(Seen::default()));
    // the level is registered twice (the recorders list some paths twice): two deliveries
    let watcher = Watcher { seen: seen.clone(), watch: vec!["player.team.0.level".into(), "player.team.0.level".into(), "game_time.seconds".into()] };
    let mut client = OfflineClient::new(watcher, &mapper()).unwrap();
    assert!(seen.lock().unwrap().connected);

    client.apply_batch(&[
        change("player.team.0.level", json!(6), &["value", "bytes"]),
        change("bag.money", json!(3100), &["value"]),
        change("game_time.seconds", json!(1), &["value"]),
    ]);
    {
        let s = seen.lock().unwrap();
        let paths: Vec<&str> = s.changes.iter().map(|c| c.0.as_str()).collect();
        assert_eq!(paths, ["player.team.0.level", "player.team.0.level", "game_time.seconds"]);
        assert_eq!(s.changes[0].1, json!(5));
        assert_eq!(s.changes[0].2, json!(6));
        assert_eq!(s.idles, 1);
    }
    // unwatched changes still update the store
    assert_eq!(client.store().get("bag.money").unwrap().value, json!(3100));

    // the same value again, or a change whose fields do not include the value: no delivery
    client.apply_batch(&[change("player.team.0.level", json!(6), &["value"]), change("game_time.seconds", json!(2), &["bytes"])]);
    {
        let s = seen.lock().unwrap();
        assert_eq!(s.changes.len(), 3);
        assert_eq!(s.idles, 2);
    }
    assert_eq!(client.store().get("game_time.seconds").unwrap().value, json!(2));

    client.idle();
    assert_eq!(seen.lock().unwrap().idles, 3);
}

#[test]
fn the_offline_clock_follows_the_replay() {
    clock::set_offline(true);
    clock::set_offline_ms(1_000);
    let a = clock::now();
    clock::set_offline_ms(1_400);
    let b = clock::now();
    assert_eq!(b.duration_since(a).as_millis(), 400);
    clock::set_offline_timer_ms(Some(61_000));
    assert_eq!(xpr_recorder::supershuckie().get_current_time().as_deref(), Some("0:01:01.00"));
    clock::set_offline_timer_ms(None);
    assert_eq!(xpr_recorder::supershuckie().get_current_time(), None);
}
