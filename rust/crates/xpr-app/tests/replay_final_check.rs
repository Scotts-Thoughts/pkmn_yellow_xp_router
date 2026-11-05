//! The check at the end of an import. In gens 4 and 5 a battle runs on its own
//! copy of the party, and recording stops at the final trainer before the game
//! has copied it back, so `player.team` still holds the party from before the
//! fight. The check waits for the game (`leaving_battle`) and must still fail
//! when the route really is wrong. Platinum `p-eevee-1-22928` read +15,613 here.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};

use xpr_app::controller::MainController;
use xpr_app::replay_import::{check_solo_mon, leaving_battle};
use xpr_core::Paths;
use xpr_data::Registry;
use xpr_engine::state::SoloPokemon;
use xpr_recorder::gamehook::{GameHookProperty, PropertyStore, SessionEvents};
use xpr_recorder::offline::OfflineClient;

struct Quiet;

impl SessionEvents for Quiet {
    fn on_connected(&mut self) {}
    fn on_connection_error(&mut self) {}
    fn on_disconnected(&mut self) {}
    fn on_game_hook_error(&mut self, _err: &str) {}
    fn on_driver_error(&mut self, _err: &str) {}
    fn on_mapper_loaded(&mut self, _store: &PropertyStore) -> Vec<String> {
        Vec::new()
    }
    fn on_mapper_load_error(&mut self, _err: &str) {}
    fn on_property_changed(&mut self, _store: &PropertyStore, _new: &GameHookProperty, _old: &GameHookProperty) {}
}

fn solo() -> SoloPokemon {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap();
    let registry = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    let mut ctrl = MainController::new(registry, Paths::new(root));
    ctrl.create_new_route("Eevee", None, "Platinum", None, None, None);
    ctrl.get_final_state().expect("a route").solo_pkmn.clone()
}

/// A mapper with the game's party holding `solo` at `exp` exp, in `state`.
fn game(solo: &SoloPokemon, exp: i64, state: &str) -> OfflineClient<Quiet> {
    let mut properties = vec![
        json!({"path": "meta.state", "value": state, "bytes": [0], "address": 1}),
        json!({"path": "player.team.0.species", "value": solo.species_def.name, "bytes": [0], "address": 2}),
        json!({"path": "player.team.0.level", "value": solo.cur_level, "bytes": [0], "address": 3}),
        json!({"path": "player.team.0.exp", "value": exp, "bytes": [0], "address": 4}),
    ];
    for i in 0..4 {
        properties.push(json!({"path": format!("player.team.0.moves.{i}.move"), "value": solo.move_list.get(i).cloned().flatten(), "bytes": [0], "address": 10 + i}));
    }
    OfflineClient::new(Quiet, &json!({"meta": {"gameName": "Pokemon Platinum"}, "glossary": {}, "properties": properties})).unwrap()
}

fn change(path: &str, value: Value) -> Value {
    json!({"path": path, "address": null, "value": value, "bytes": null, "isFrozen": false, "fieldsChanged": ["value"]})
}

#[test]
fn the_check_waits_for_the_party_to_come_back() {
    let solo = solo();
    let mut client = game(&solo, solo.cur_xp - 15613, "From Battle");
    assert!(leaving_battle(client.store()), "the party is still the pre-fight copy");
    assert!(!check_solo_mon(client.store(), &solo).0, "read now, the same route looks wrong");

    // frames 2148900 and 2148914 of the replay: the party, then the state
    client.apply_batch(&[change("player.team.0.exp", json!(solo.cur_xp))]);
    assert!(leaving_battle(client.store()));
    client.apply_batch(&[change("meta.state", json!("Overworld"))]);
    assert!(!leaving_battle(client.store()));
    let (passed, lines) = check_solo_mon(client.store(), &solo);
    assert!(passed, "{lines:?}");
}

#[test]
fn a_real_difference_still_fails_after_the_wait() {
    let solo = solo();
    let mut client = game(&solo, solo.cur_xp - 15613, "From Battle");
    client.apply_batch(&[change("player.team.0.exp", json!(solo.cur_xp - 1))]);
    client.apply_batch(&[change("meta.state", json!("Overworld"))]);
    assert!(!leaving_battle(client.store()));
    let (passed, lines) = check_solo_mon(client.store(), &solo);
    assert!(!passed, "one exp off must fail: {lines:?}");
    assert!(lines.iter().any(|l| l.contains("(+1)")), "{lines:?}");
}

#[test]
fn only_the_end_of_a_battle_delays_the_check() {
    let solo = solo();
    for state in ["Overworld", "Battle", "To Battle"] {
        assert!(!leaving_battle(game(&solo, 0, state).store()), "{state}");
    }
    // a mapper without the property
    let client = OfflineClient::new(Quiet, &json!({"meta": {"gameName": "Pokemon Yellow"}, "glossary": {}, "properties": [{"path": "game_time.seconds", "value": 0, "bytes": [0], "address": 1}]})).unwrap();
    assert!(!leaving_battle(client.store()));
}
