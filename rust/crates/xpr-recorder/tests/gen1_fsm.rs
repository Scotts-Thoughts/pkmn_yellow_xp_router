//! Drives the Yellow FSM with a scripted property store and checks the
//! events that reach the (fake) application.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use xpr_data::model::StatBlock;
use xpr_data::{GenData, Registry};
use xpr_engine::{EventDefinition, LearnMoveEventDefinition, NodeId};
use xpr_recorder::controller::GameRecorder;
use xpr_recorder::games::gen1::Gen1Machine;
use xpr_recorder::host::{HostHandle, PrevEvent, RecorderHost, StartInfo};
use xpr_recorder::{PropertyStore, RecorderController};

struct FakeHost {
    gen: Arc<GenData>,
    events: Arc<Mutex<Vec<(NodeId, String, EventDefinition)>>>,
    folders: Vec<String>,
    next_id: NodeId,
}

impl RecorderHost for FakeHost {
    fn is_record_mode_active(&self) -> bool {
        true
    }
    fn set_record_mode(&mut self, _active: bool) {}
    fn is_empty(&self) -> bool {
        self.events.lock().unwrap().is_empty()
    }
    fn get_all_folder_names(&self) -> Vec<String> {
        self.folders.clone()
    }
    fn get_previous_event(&self, cur_event_id: Option<NodeId>) -> Option<PrevEvent> {
        let events = self.events.lock().unwrap();
        let idx = match cur_event_id {
            None => events.len().checked_sub(1)?,
            Some(id) => events.iter().position(|(i, _, _)| *i == id)?.checked_sub(1)?,
        };
        let (id, folder, def) = &events[idx];
        Some(PrevEvent {
            group_id: *id,
            parent_name: folder.clone(),
            definition: def.clone(),
            final_held_item: None,
            first_trainer_name: def.trainer_def.as_ref().map(|t| t.trainer_name.clone()),
        })
    }
    fn delete_events(&mut self, ids: &[NodeId]) {
        self.events.lock().unwrap().retain(|(i, _, _)| !ids.contains(i));
    }
    fn purge_empty_folders(&mut self) {}
    fn new_event(&mut self, def: EventDefinition, dest_folder_name: &str) {
        self.next_id += 1;
        self.events.lock().unwrap().push((self.next_id, dest_folder_name.to_string(), def));
    }
    fn finalize_new_folder(&mut self, name: &str) {
        self.folders.push(name.to_string());
    }
    fn get_defeated_trainers(&self) -> Vec<String> {
        Vec::new()
    }
    fn get_move_idx(&self, _move_name: &str) -> Option<i64> {
        None
    }
    fn update_levelup_move(&mut self, _def: LearnMoveEventDefinition) {}
    fn update_existing_event(&mut self, id: NodeId, def: EventDefinition) {
        for e in self.events.lock().unwrap().iter_mut() {
            if e.0 == id {
                e.2 = def.clone();
            }
        }
    }
    fn get_dvs(&self) -> Option<StatBlock> {
        Some(self.gen.make_stat_block(15, 15, 15, 15, 15, 15, false))
    }
    fn final_solo_species(&self) -> Option<String> {
        Some("Pikachu".into())
    }
    fn final_held_item(&self) -> Option<String> {
        None
    }
    fn final_inventory_has(&self, _item_name: &str) -> bool {
        false
    }
    fn can_evolve_into(&self, _species: &str) -> bool {
        false
    }
    fn is_valid_levelup_move(&self, _def: &LearnMoveEventDefinition) -> bool {
        true
    }
    fn get_version(&self) -> Option<String> {
        Some("Yellow".into())
    }
    fn gen(&self) -> Option<Arc<GenData>> {
        Some(self.gen.clone())
    }
    fn trigger_exception(&mut self, _msg: &str) {}
    fn send_message(&mut self, _msg: &str) {}
    fn final_trainers(&self, _version: &str) -> Vec<String> {
        Vec::new()
    }
    fn recording_auto_stop_enabled(&self) -> bool {
        false
    }
    fn is_debug_mode(&self) -> bool {
        true
    }
    fn gamehook_url(&self) -> String {
        "http://localhost:8085".into()
    }
}

fn mapper() -> Value {
    mapper_with_bag(&[("POTION", 1)])
}

/// The mapper with the bag seeded in slot order (GameHook item names).
fn mapper_with_bag(bag: &[(&str, i64)]) -> Value {
    let mut props = vec![
        json!({"path": "overworld.map_name", "value": "Pallet Town"}),
        json!({"path": "audio.channels.3", "value": 0}),
        json!({"path": "audio.channels.4", "value": 0}),
        json!({"path": "audio.channels.6", "value": 0}),
        json!({"path": "player.player_id", "value": 12345}),
        json!({"path": "bag.money", "value": 3000}),
        json!({"path": "player.team.0.exp", "value": 125}),
        json!({"path": "game_time.seconds", "value": 0}),
        json!({"path": "battle.mode", "value": null}),
        json!({"path": "battle.opponent.trainer", "value": null}),
        json!({"path": "battle.opponent.id", "value": 0}),
        json!({"path": "battle.player.active_pokemon.species", "value": null}),
        json!({"path": "battle.player.active_pokemon.stats.hp", "value": 20}),
        json!({"path": "battle.opponent.active_pokemon.species", "value": null}),
        json!({"path": "battle.opponent.active_pokemon.level", "value": 0}),
        json!({"path": "bag.item_count", "value": bag.len()}),
    ];
    for i in 0..6 {
        let species = if i == 0 { json!("Pikachu") } else { Value::Null };
        props.push(json!({"path": format!("player.team.{}.species", i), "value": species}));
        props.push(json!({"path": format!("player.team.{}.level", i), "value": if i == 0 { 5 } else { 0 }}));
        for dv in ["attack", "defense", "speed", "special"] {
            props.push(json!({"path": format!("player.team.{}.ivs.{}", i, dv), "value": 15}));
        }
    }
    let moves = [json!("Thundershock"), json!("Growl"), Value::Null, Value::Null];
    for (i, m) in moves.iter().enumerate() {
        props.push(json!({"path": format!("player.team.0.moves.{}.move", i), "value": m}));
    }
    for ev in ["hp", "attack", "defense", "speed", "special"] {
        props.push(json!({"path": format!("player.team.0.evs.{}", ev), "value": 0}));
    }
    for i in 0..20 {
        let (item, qty) = match bag.get(i) {
            Some((name, qty)) => (json!(name), *qty),
            None => (Value::Null, 0),
        };
        props.push(json!({"path": format!("bag.items.{}.item", i), "value": item}));
        props.push(json!({"path": format!("bag.items.{}.quantity", i), "value": qty}));
    }
    json!({"meta": {"gameName": "Pokemon Yellow"}, "glossary": {}, "properties": props})
}

struct Sim {
    store: PropertyStore,
    machine: Gen1Machine,
    seconds: i64,
}

impl Sim {
    fn set(&mut self, path: &str, value: Value) {
        let (new, old) = self.store.set_value(path, value).expect("known path");
        self.machine.handle_event(&self.store, &new, &old);
    }
    fn tick(&mut self, n: usize) {
        for _ in 0..n {
            self.seconds += 1;
            self.set("game_time.seconds", json!(self.seconds));
        }
    }
}

#[test]
fn yellow_wild_fight_reaches_the_route() {
    let root = xpr_core::consts::find_source_root().expect("source root");
    let reg = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    let gen = reg.get_version("Yellow").expect("yellow");
    let events = Arc::new(Mutex::new(Vec::new()));
    let host = FakeHost { gen: gen.clone(), events: events.clone(), folders: vec!["ROOT".into()], next_id: 0 };
    let handle = HostHandle::direct(Box::new(host));
    let controller = RecorderController::new(handle, Arc::new(|| {}));
    let info = StartInfo {
        version: "Yellow".into(),
        gen: gen.clone(),
        url: String::new(),
        debug_mode: false,
        dvs: Some(gen.make_stat_block(15, 15, 15, 15, 15, 15, false)),
        solo_species: Some("Pikachu".into()),
    };
    let mut machine = Gen1Machine::new(controller.clone(), &info, false);
    let store = PropertyStore::from_mapper(&mapper()).unwrap();
    let (watch, invalid) = machine.on_mapper_loaded(&store);
    assert!(invalid.is_empty(), "invalid keys: {:?}", invalid);
    assert!(watch.contains(&"battle.mode".to_string()));
    machine.startup(&store);
    let mut sim = Sim { store, machine, seconds: 0 };

    // UNINITIALIZED -> OVERWORLD after the start-up delay
    sim.tick(4);
    assert_eq!(controller.get_game_state(), Some(xpr_recorder::GameState::Overworld));

    // a wild Pidgey appears, is defeated, and the battle ends
    sim.set("battle.mode", json!("Wild"));
    assert_eq!(controller.get_game_state(), Some(xpr_recorder::GameState::Battle));
    sim.set("battle.opponent.active_pokemon.species", json!("Pidgey"));
    sim.set("battle.opponent.active_pokemon.level", json!(3));
    sim.tick(2);
    sim.set("player.team.0.exp", json!(140));
    sim.set("battle.mode", Value::Null);
    assert_eq!(controller.get_game_state(), Some(xpr_recorder::GameState::Overworld));

    // the processing thread hands the event to the host
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let got = events.lock().unwrap().clone();
        if let Some((_, folder, def)) = got.iter().find(|(_, _, d)| d.wild_pkmn_info.is_some()) {
            let w = def.wild_pkmn_info.as_ref().unwrap();
            assert_eq!((w.name.as_str(), w.level, w.trainer_pkmn), ("Pidgey", 3, false));
            assert_eq!(folder, "Pallet Town");
            break;
        }
        assert!(Instant::now() < deadline, "no wild event arrived: {:?}", got.iter().map(|e| e.2.notes.clone()).collect::<Vec<_>>());
        std::thread::sleep(Duration::from_millis(20));
    }
    sim.machine.shutdown();
}

// ---- bag reordering -----------------------------------------------------------------

type Events = Arc<Mutex<Vec<(NodeId, String, EventDefinition)>>>;

/// A Yellow machine in the overworld with the given bag.
fn overworld_sim(bag: &[(&str, i64)]) -> (Sim, Arc<RecorderController>, Events) {
    let root = xpr_core::consts::find_source_root().expect("source root");
    let reg = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    let gen = reg.get_version("Yellow").expect("yellow");
    let events: Events = Arc::new(Mutex::new(Vec::new()));
    let host = FakeHost { gen: gen.clone(), events: events.clone(), folders: vec!["ROOT".into()], next_id: 0 };
    let handle = HostHandle::direct(Box::new(host));
    let controller = RecorderController::new(handle, Arc::new(|| {}));
    let info = StartInfo {
        version: "Yellow".into(),
        gen: gen.clone(),
        url: String::new(),
        debug_mode: false,
        dvs: Some(gen.make_stat_block(15, 15, 15, 15, 15, 15, false)),
        solo_species: Some("Pikachu".into()),
    };
    let mut machine = Gen1Machine::new(controller.clone(), &info, false);
    let store = PropertyStore::from_mapper(&mapper_with_bag(bag)).unwrap();
    let (_, invalid) = machine.on_mapper_loaded(&store);
    assert!(invalid.is_empty(), "invalid keys: {:?}", invalid);
    machine.startup(&store);
    let mut sim = Sim { store, machine, seconds: 0 };
    sim.tick(4);
    assert_eq!(controller.get_game_state(), Some(xpr_recorder::GameState::Overworld));
    (sim, controller, events)
}

/// Write the whole bag (slot order) as the game would leave it, then let the
/// inventory-change state time out.
fn set_bag(sim: &mut Sim, controller: &RecorderController, bag: &[(&str, i64)]) {
    sim.set("bag.item_count", json!(bag.len()));
    for i in 0..20 {
        let (item, qty) = match bag.get(i) {
            Some((name, qty)) => (json!(name), *qty),
            None => (Value::Null, 0),
        };
        let cur_item = sim.store.get(&format!("bag.items.{}.item", i)).map(|p| p.value.clone());
        if cur_item.as_ref() != Some(&item) {
            sim.set(&format!("bag.items.{}.item", i), item);
        }
        let cur_qty = sim.store.get(&format!("bag.items.{}.quantity", i)).map(|p| p.value.clone());
        if cur_qty != Some(json!(qty)) {
            sim.set(&format!("bag.items.{}.quantity", i), json!(qty));
        }
    }
    assert_eq!(controller.get_game_state(), Some(xpr_recorder::GameState::InventoryChange));
    sim.tick(4);
    assert_eq!(controller.get_game_state(), Some(xpr_recorder::GameState::Overworld));
}

/// Wait for the processing thread to deliver `n` events (and a little longer
/// for any it should *not* deliver).
fn collect_events(events: &Events, n: usize) -> Vec<EventDefinition> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while events.lock().unwrap().len() < n {
        assert!(Instant::now() < deadline, "only {} of {} events arrived: {:?}", events.lock().unwrap().len(), n, labels(events));
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(400));
    events.lock().unwrap().iter().map(|(_, _, d)| d.clone()).collect()
}

fn labels(events: &Events) -> Vec<String> {
    events.lock().unwrap().iter().map(|(_, _, d)| d.serialize().to_string()).collect()
}

fn swap_strings(def: &EventDefinition) -> Vec<String> {
    def.bag_reorder.as_ref().map(|r| r.swaps.iter().map(|s| s.to_string()).collect()).unwrap_or_default()
}

#[test]
fn a_select_swap_records_one_reorder_event() {
    let (mut sim, controller, events) = overworld_sim(&[("POTION", 3), ("POKé BALL", 5), ("ANTIDOTE", 1)]);
    set_bag(&mut sim, &controller, &[("ANTIDOTE", 1), ("POKé BALL", 5), ("POTION", 3)]);
    let got = collect_events(&events, 1);
    assert_eq!(got.len(), 1, "{:?}", labels(&events));
    assert!(got[0].item_event_def.is_none(), "a swap is not an item change");
    assert_eq!(swap_strings(&got[0]), vec!["Potion (1) <-> Antidote (3)".to_string()]);
    sim.machine.shutdown();
}

#[test]
fn using_the_last_of_an_item_is_not_a_reorder() {
    let (mut sim, controller, events) = overworld_sim(&[("POTION", 1), ("POKé BALL", 5), ("ANTIDOTE", 1)]);
    // the Potion slot goes away and everything below shifts up
    set_bag(&mut sim, &controller, &[("POKé BALL", 5), ("ANTIDOTE", 1)]);
    let got = collect_events(&events, 1);
    assert_eq!(got.len(), 1, "{:?}", labels(&events));
    let item = got[0].item_event_def.as_ref().expect("a use event");
    assert_eq!((item.item_name.as_str(), item.item_amount, item.is_acquire, item.with_money), ("Potion", 1, false, false));
    sim.machine.shutdown();
}

#[test]
fn a_pickup_at_the_end_is_not_a_reorder() {
    let (mut sim, controller, events) = overworld_sim(&[("POTION", 1), ("POKé BALL", 5)]);
    set_bag(&mut sim, &controller, &[("POTION", 1), ("POKé BALL", 5), ("ANTIDOTE", 1)]);
    let got = collect_events(&events, 1);
    assert_eq!(got.len(), 1, "{:?}", labels(&events));
    let item = got[0].item_event_def.as_ref().expect("an acquire event");
    assert_eq!((item.item_name.as_str(), item.item_amount, item.is_acquire), ("Antidote", 1, true));
    sim.machine.shutdown();
}

#[test]
fn a_use_and_a_swap_in_one_window_record_both() {
    let (mut sim, controller, events) = overworld_sim(&[("POTION", 1), ("POKé BALL", 5), ("ANTIDOTE", 1)]);
    // the Potion is used up (bag shifts to Poke Ball, Antidote), then the two survivors are swapped
    set_bag(&mut sim, &controller, &[("ANTIDOTE", 1), ("POKé BALL", 5)]);
    let got = collect_events(&events, 2);
    assert_eq!(got.len(), 2, "{:?}", labels(&events));
    let item = got[0].item_event_def.as_ref().expect("the use event comes first");
    assert_eq!((item.item_name.as_str(), item.item_amount, item.is_acquire), ("Potion", 1, false));
    // slots are those of the bag *after* the use, which is what the engine will hold
    assert_eq!(swap_strings(&got[1]), vec!["Poke Ball (1) <-> Antidote (2)".to_string()]);
    sim.machine.shutdown();
}

#[test]
fn oaks_parcel_does_not_count_as_a_slot() {
    let (mut sim, controller, events) = overworld_sim(&[("OAK'S PARCEL", 1), ("POTION", 2), ("POKé BALL", 5)]);
    set_bag(&mut sim, &controller, &[("OAK'S PARCEL", 1), ("POKé BALL", 5), ("POTION", 2)]);
    let got = collect_events(&events, 1);
    assert_eq!(got.len(), 1, "{:?}", labels(&events));
    assert_eq!(swap_strings(&got[0]), vec!["Potion (1) <-> Poke Ball (2)".to_string()]);
    sim.machine.shutdown();
}

#[test]
fn several_swaps_in_one_window_become_one_event() {
    let (mut sim, controller, events) = overworld_sim(&[("POTION", 3), ("POKé BALL", 5), ("ANTIDOTE", 1), ("ESCAPE ROPE", 1)]);
    // a 3-cycle of the first three slots
    set_bag(&mut sim, &controller, &[("POKé BALL", 5), ("ANTIDOTE", 1), ("POTION", 3), ("ESCAPE ROPE", 1)]);
    let got = collect_events(&events, 1);
    assert_eq!(got.len(), 1, "{:?}", labels(&events));
    assert_eq!(swap_strings(&got[0]), vec!["Potion (1) <-> Poke Ball (2)".to_string(), "Potion (2) <-> Antidote (3)".to_string()]);
    sim.machine.shutdown();
}

#[test]
fn an_item_the_route_cannot_hold_is_not_a_slot() {
    // an item the item DB does not know never reaches the engine's bag, so it
    // must not shift the recorded slots either
    let (mut sim, controller, events) = overworld_sim(&[("POTION", 3), ("GLITCH ITEM", 1), ("POKé BALL", 5), ("ANTIDOTE", 1)]);
    set_bag(&mut sim, &controller, &[("ANTIDOTE", 1), ("GLITCH ITEM", 1), ("POKé BALL", 5), ("POTION", 3)]);
    let got = collect_events(&events, 1);
    assert_eq!(got.len(), 1, "{:?}", labels(&events));
    assert_eq!(swap_strings(&got[0]), vec!["Potion (1) <-> Antidote (3)".to_string()]);
    sim.machine.shutdown();
}
