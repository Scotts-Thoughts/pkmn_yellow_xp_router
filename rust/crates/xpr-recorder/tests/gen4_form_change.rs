//! Drives the Platinum machine through the overworld with a scripted property
//! store and checks Shaymin's form changes.
//!
//! The mapper names both forms "Shaymin"; only the party struct's form bits
//! (`flags.alternate_forms`) change when the Gracidea turns it into the Sky
//! Forme (or the Sky Forme reverts), so the form bits must drive the species.

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
    // level-up learn / ignore events land here: record them as events
    fn update_levelup_move(&mut self, def: LearnMoveEventDefinition) {
        self.new_event(EventDefinition::with_learn_move(def), "ROOT");
    }
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
        Some("Shaymin (Land)".into())
    }
    fn final_held_item(&self) -> Option<String> {
        None
    }
    fn final_inventory_has(&self, _item_name: &str) -> bool {
        false
    }
    fn can_evolve_into(&self, species: &str) -> bool {
        // what the app checks: the same growth rate as the route's mon
        self.gen.pkmn_db().get_pkmn(species).map(|p| p.growth_rate == "growth_medium_slow").unwrap_or(false)
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
/// Land Forme Shaymin, two Potions in the bag.
fn mapper(k: &Gen45Keys) -> Value {
    let mut paths = k.all_keys_to_register();
    paths.push(k.mon_friendship.clone());
    for list in [
        &k.team_level,
        &k.team_form,
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
            "player.team.0.species" => json!("Shaymin"),
            "player.team.0.flags.alternate_forms" => json!(0),
            "player.team.0.level" => json!(28),
            "player.team.0.exp" => json!(19000),
            "player.team.0.internals.personality_value" => json!(70675),
            "player.team.0.moves.0.move" => json!("Growth"),
            "bag.medicine.0.item" => json!("Potion"),
            "bag.medicine.0.quantity" => json!(2),
            p if p.starts_with("player.team.0.ivs.") => json!(31),
            "player.team.1.species" => json!("Bidoof"),
            "player.team.1.level" => json!(10),
            "player.team.1.flags.alternate_forms" => json!(0),
            p if p.starts_with("player.team.1.ivs.") => json!(10),
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
        solo_species: Some("Shaymin (Land)".into()),
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

/// Every event so far, as short strings, once `n` have arrived.
fn wait_for_events(log: &Arc<Mutex<Log>>, n: usize) -> Vec<String> {
    let describe = |log: &Log| -> Vec<String> {
        log.events
            .iter()
            .map(|(_, d)| match (&d.evolution, &d.learn_move) {
                (Some(e), _) => format!("evolve {}", e.evolved_species.as_deref().unwrap_or("None")),
                (_, Some(l)) => format!("learn {}", l.move_to_learn.as_deref().unwrap_or("None")),
                _ => d.get_event_type().to_string(),
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
fn gracidea_and_revert_record_form_changes() {
    let (mut sim, log, _controller) = start();
    // the Gracidea: same species name, Sky Forme bits
    sim.batch(&[("player.team.0.flags.alternate_forms", json!(1))]);
    sim.tick(4);
    assert_eq!(wait_for_events(&log, 1), vec!["evolve Shaymin (Sky)"]);
    // nightfall reverts it
    sim.batch(&[("player.team.0.flags.alternate_forms", json!(0))]);
    sim.tick(4);
    // a form change teaches nothing (no learn / ignore events for the new form)
    assert_eq!(wait_for_events(&log, 2), vec!["evolve Shaymin (Sky)", "evolve Shaymin (Land)"]);
}

#[test]
fn sky_forme_back_from_the_pc_is_land_forme() {
    let (mut sim, log, _controller) = start();
    sim.batch(&[("player.team.0.flags.alternate_forms", json!(1))]);
    sim.tick(4);
    assert_eq!(wait_for_events(&log, 1), vec!["evolve Shaymin (Sky)"]);
    // deposited (the Sky Forme reverts in the PC): the Bidoof moves up
    let slot = |i: usize, species: Option<&str>, level: i64, iv: i64| -> Vec<(String, Value)> {
        let mut v = vec![
            (format!("player.team.{i}.species"), species.map(|s| json!(s)).unwrap_or(Value::Null)),
            (format!("player.team.{i}.level"), json!(level)),
            (format!("player.team.{i}.flags.alternate_forms"), json!(0)),
        ];
        for s in ["attack", "defense", "speed", "special_attack", "special_defense"] {
            v.push((format!("player.team.{i}.ivs.{s}"), json!(iv)));
        }
        v
    };
    let batch = |sim: &mut Sim, changes: Vec<(String, Value)>| {
        let c: Vec<(&str, Value)> = changes.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        sim.batch(&c);
    };
    batch(&mut sim, [slot(0, Some("Bidoof"), 10, 10), slot(1, None, 0, 0)].concat());
    sim.tick(8);
    // withdrawn into slot 2
    batch(&mut sim, slot(1, Some("Shaymin"), 28, 31));
    sim.tick(8);
    // level 28: Land Forme learns Synthesis there, but a form change teaches nothing
    assert_eq!(wait_for_events(&log, 2), vec!["evolve Shaymin (Sky)", "evolve Shaymin (Land)"]);
}


#[test]
fn a_move_tutor_paid_in_shards_records_the_move() {
    let (mut sim, log, _controller) = start();
    sim.batch(&[("bag.items.0.item", json!("Red Shard")), ("bag.items.0.quantity", json!(2))]);
    sim.tick(4);
    assert_eq!(wait_for_events(&log, 1).len(), 1);
    // the shards go first, the move a moment later: the bag change leads
    sim.batch(&[("bag.items.0.item", Value::Null), ("bag.items.0.quantity", json!(0))]);
    sim.batch(&[("player.team.0.moves.1.move", json!("Air Cutter"))]);
    sim.tick(4);
    let events = wait_for_events(&log, 3);
    assert_eq!(&events[1..], ["Use/Drop Item", "learn Air Cutter"], "{:?}", events);
}
