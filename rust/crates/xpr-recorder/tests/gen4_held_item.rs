//! Drives the Platinum machine through the overworld with a scripted property
//! store and checks the held item events.
//!
//! The party's held item is written back as a battle ends, so a berry eaten
//! in a fight shows up in the overworld with no bag change. The next bag
//! change, seconds later, used to read as "took the held item off": a Rare
//! Candy picked up after a fight that ate an Oran Berry was recorded as
//! `Hold None` (putting the berry back in the bag) and the candy was lost.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use xpr_data::model::StatBlock;
use xpr_data::{GenData, Registry};
use xpr_engine::{EventDefinition, LearnMoveEventDefinition, NodeId};
use xpr_recorder::controller::GameRecorder;
use xpr_recorder::games::gen45::{Flavor, Gen45Keys, Gen45Machine};
use xpr_recorder::host::{HostHandle, PrevEvent, RecorderHost, StartInfo};
use xpr_recorder::{GameState, PropertyStore, RecorderController};

#[derive(Default)]
struct Log {
    events: Vec<(NodeId, EventDefinition)>,
    next_id: NodeId,
}

struct FakeHost {
    gen: Arc<GenData>,
    log: Arc<Mutex<Log>>,
}

impl RecorderHost for FakeHost {
    fn is_record_mode_active(&self) -> bool {
        true
    }
    fn set_record_mode(&mut self, _active: bool) {}
    fn is_empty(&self) -> bool {
        self.log.lock().unwrap().events.is_empty()
    }
    fn get_all_folder_names(&self) -> Vec<String> {
        vec!["ROOT".into()]
    }
    fn get_previous_event(&self, cur_event_id: Option<NodeId>) -> Option<PrevEvent> {
        let log = self.log.lock().unwrap();
        let idx = match cur_event_id {
            None => log.events.len().checked_sub(1)?,
            Some(id) => log.events.iter().position(|(i, _)| *i == id)?.checked_sub(1)?,
        };
        let (id, def) = &log.events[idx];
        Some(PrevEvent {
            group_id: *id,
            parent_name: "ROOT".into(),
            definition: def.clone(),
            final_held_item: None,
            first_trainer_name: def.trainer_def.as_ref().map(|t| t.trainer_name.clone()),
        })
    }
    fn delete_events(&mut self, ids: &[NodeId]) {
        self.log.lock().unwrap().events.retain(|(i, _)| !ids.contains(i));
    }
    fn purge_empty_folders(&mut self) {}
    fn new_event(&mut self, def: EventDefinition, _dest_folder_name: &str) {
        let mut log = self.log.lock().unwrap();
        log.next_id += 1;
        let id = log.next_id;
        log.events.push((id, def));
    }
    fn finalize_new_folder(&mut self, _name: &str) {}
    fn get_defeated_trainers(&self) -> Vec<String> {
        Vec::new()
    }
    fn get_move_idx(&self, _move_name: &str) -> Option<i64> {
        None
    }
    fn update_levelup_move(&mut self, _def: LearnMoveEventDefinition) {}
    fn update_existing_event(&mut self, id: NodeId, def: EventDefinition) {
        for e in self.log.lock().unwrap().events.iter_mut() {
            if e.0 == id {
                e.1 = def.clone();
            }
        }
    }
    fn get_dvs(&self) -> Option<StatBlock> {
        Some(self.gen.make_stat_block(31, 31, 31, 31, 31, 31, false))
    }
    fn final_solo_species(&self) -> Option<String> {
        Some("Drifloon".into())
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
        Some("Platinum".into())
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
        false
    }
    fn gamehook_url(&self) -> String {
        String::new()
    }
}

/// A Platinum mapper property list with every key the machine reads: a
/// Drifloon holding an Oran Berry, two Potions in the bag.
fn mapper(k: &Gen45Keys) -> Value {
    let mut paths = k.all_keys_to_register();
    paths.push(k.mon_friendship.clone());
    for list in [
        &k.team_level,
        &k.team_hp,
        &k.team_iv_attack,
        &k.team_iv_defense,
        &k.team_iv_speed,
        &k.team_iv_special_attack,
        &k.team_iv_special_defense,
        &k.enemy_team_species,
        &k.enemy_2_team_species,
        &k.battle_enemy_1_pid,
        &k.battle_enemy_2_pid,
    ] {
        paths.extend(list.iter().cloned());
    }
    for list in &k.team_ev {
        paths.extend(list.iter().cloned());
    }
    paths.sort();
    paths.dedup();
    let value = |path: &str| -> Value {
        match path {
            "meta.state" => json!("Overworld"),
            "overworld.map_name" => json!("Route 207"),
            "player.player_id" => json!(1234),
            "bag.money" => json!(3000),
            "game_time.seconds" => json!(0),
            "player.team.0.species" => json!("Drifloon"),
            "player.team.0.level" => json!(20),
            "player.team.0.exp" => json!(8000),
            "player.team.0.held_item" => json!("Oran Berry"),
            "player.team.0.internals.personality_value" => json!(70675),
            "player.team.0.moves.0.move" => json!("Gust"),
            "bag.medicine.0.item" => json!("Potion"),
            "bag.medicine.0.quantity" => json!(2),
            p if p.starts_with("player.team.0.ivs.") => json!(31),
            p if p.ends_with(".quantity") || p.contains(".evs.") => json!(0),
            _ => Value::Null,
        }
    };
    let props: Vec<Value> = paths.iter().map(|p| json!({"path": p, "value": value(p)})).collect();
    json!({"meta": {"gameName": "STP Pokemon Platinum"}, "glossary": {}, "properties": props})
}

struct Sim {
    store: PropertyStore,
    machine: Gen45Machine,
    seconds: i64,
}

impl Sim {
    /// One `PropertiesChanged` batch, then the idle pass.
    fn batch(&mut self, changes: &[(&str, Value)]) {
        for (path, value) in changes {
            let (new, old) = self.store.set_value(path, value.clone()).unwrap_or_else(|| panic!("unknown path {}", path));
            if new.value != old.value {
                self.machine.handle_event(&self.store, &new, &old);
            }
        }
        self.machine.on_idle(&self.store);
    }
    fn tick(&mut self, n: usize) {
        for _ in 0..n {
            self.seconds += 1;
            self.batch(&[("game_time.seconds", json!(self.seconds))]);
        }
    }
}

fn start() -> (Sim, Arc<Mutex<Log>>, Arc<RecorderController>) {
    let root = xpr_core::consts::find_source_root().expect("source root");
    let reg = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    let gen = reg.get_version("Platinum").expect("platinum");
    let log = Arc::new(Mutex::new(Log::default()));
    let host = FakeHost { gen: gen.clone(), log: log.clone() };
    let controller = RecorderController::new(HostHandle::direct(Box::new(host)), Arc::new(|| {}));
    let info = StartInfo {
        version: "Platinum".into(),
        gen: gen.clone(),
        url: String::new(),
        debug_mode: false,
        dvs: Some(gen.make_stat_block(31, 31, 31, 31, 31, 31, false)),
        solo_species: Some("Drifloon".into()),
    };
    let mut machine = Gen45Machine::new(controller.clone(), &info, Flavor::Platinum);
    let store = PropertyStore::from_mapper(&mapper(&Gen45Keys::configure(Flavor::Platinum))).unwrap();
    let (_watch, invalid) = machine.on_mapper_loaded(&store);
    assert!(invalid.is_empty(), "invalid keys: {:?}", invalid);
    machine.startup(&store);
    let mut sim = Sim { store, machine, seconds: 0 };
    sim.tick(4); // UNINITIALIZED -> OVERWORLD
    assert_eq!(controller.get_game_state(), Some(GameState::Overworld));
    (sim, log, controller)
}

/// The item and hold events, as short strings, once `n` have arrived.
fn wait_for_events(log: &Arc<Mutex<Log>>, n: usize) -> Vec<String> {
    let describe = |log: &Log| -> Vec<String> {
        log.events
            .iter()
            .filter_map(|(_, d)| {
                if let Some(h) = &d.hold_item {
                    Some(format!("hold {} consumed={}", h.item_name.as_deref().unwrap_or("None"), h.consumed))
                } else {
                    d.item_event_def.as_ref().map(|i| format!("{} {} x{}", if i.is_acquire { "get" } else { "lose" }, i.item_name, i.item_amount))
                }
            })
            .collect()
    };
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let got = describe(&log.lock().unwrap());
        if got.len() >= n {
            std::thread::sleep(Duration::from_millis(300));
            return describe(&log.lock().unwrap());
        }
        assert!(Instant::now() < deadline, "recorder produced {:?}", got);
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn berry_eaten_in_battle_then_a_pickup_records_both() {
    let (mut sim, log, _controller) = start();
    // written back as the fight ends: the berry is gone, nothing in the bag
    sim.batch(&[("player.team.0.held_item", Value::Null)]);
    sim.tick(6);
    // a Rare Candy from an item ball
    sim.batch(&[("bag.medicine.1.item", json!("Rare Candy")), ("bag.medicine.1.quantity", json!(1))]);
    sim.tick(4);
    assert_eq!(wait_for_events(&log, 2), vec!["hold None consumed=true", "get Rare Candy x1"]);
}

#[test]
fn taking_the_held_item_off_records_only_the_hold() {
    let (mut sim, log, _controller) = start();
    sim.batch(&[
        ("player.team.0.held_item", Value::Null),
        ("bag.berries.0.item", json!("Oran Berry")),
        ("bag.berries.0.quantity", json!(1)),
    ]);
    sim.tick(4);
    assert_eq!(wait_for_events(&log, 1), vec!["hold None consumed=false"]);
}

#[test]
fn swapping_the_held_item_records_only_the_hold() {
    let (mut sim, log, _controller) = start();
    // the Potion goes on, the berry back to the bag; a pickup in the same window
    sim.batch(&[
        ("bag.medicine.0.quantity", json!(1)),
        ("player.team.0.held_item", json!("Potion")),
        ("bag.berries.0.item", json!("Oran Berry")),
        ("bag.berries.0.quantity", json!(1)),
        ("bag.items.0.item", json!("Nugget")),
        ("bag.items.0.quantity", json!(1)),
    ]);
    sim.tick(4);
    assert_eq!(wait_for_events(&log, 2), vec!["hold Potion consumed=false", "get Nugget x1"]);
}
