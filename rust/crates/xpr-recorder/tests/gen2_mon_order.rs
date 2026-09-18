//! Drives the Crystal FSM through trainer battles with a scripted property
//! store and checks the `mon_order` the ROAR update writes: 1-based, one entry
//! per enemy mon, whether or not the game's -1 (255) "no mon" party position
//! was seen as a change before the lead came out (KNOWN_ISSUES KI-2).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use xpr_data::model::StatBlock;
use xpr_data::{GenData, Registry};
use xpr_engine::{EventDefinition, LearnMoveEventDefinition, NodeId};
use xpr_recorder::controller::GameRecorder;
use xpr_recorder::games::gen2::{Gen2Keys, Gen2Machine};
use xpr_recorder::host::{HostHandle, PrevEvent, RecorderHost, StartInfo};
use xpr_recorder::{PropertyStore, RecorderController};

/// Every event the recorder added, in order, plus the updates it applied.
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
        Some(self.gen.make_stat_block(15, 15, 15, 15, 15, 15, false))
    }
    fn final_solo_species(&self) -> Option<String> {
        Some("Totodile".into())
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
        Some("Crystal".into())
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

/// A deprecated-mapper property list with every key the machine reads.
fn mapper() -> Value {
    let k = Gen2Keys::configure_for_mapper(Some("Pokemon Crystal - Deprecated Mapper"));
    let mut props: Vec<Value> = Vec::new();
    let mut add = |path: &str, value: Value| props.push(json!({"path": path, "value": value}));
    for (path, value) in [
        (&k.overworld_map, json!(26)),
        (&k.overworld_map_num, json!(1)),
        (&k.overworld_x, json!(5)),
        (&k.overworld_y, json!(7)),
        (&k.player_id, json!(7777)),
        (&k.player_money, json!(3000)),
        (&k.mon_exppoints, json!(135)),
        (&k.mon_level, json!(5)),
        (&k.mon_species, json!("Totodile")),
        (&k.mon_held_item, Value::Null),
        (&k.mon_friendship, json!(70)),
        (&k.gametime_seconds, json!(0)),
        (&k.gametime_frames, json!(0)),
        (&k.audio_current_sound, json!(0)),
        (&k.battle_mode, Value::Null),
        (&k.battle_type, Value::Null),
        (&k.battle_text_buffer, json!("")),
        (&k.battle_result, Value::Null),
        (&k.battle_start, json!(0)),
        (&k.battle_trainer_class, Value::Null),
        (&k.battle_trainer_name, Value::Null),
        (&k.battle_trainer_number, json!(0)),
        (&k.battle_trainer_total_pokemon, json!(0)),
        (&k.battle_player_mon_party_pos, json!(0)),
        (&k.battle_player_mon_species, Value::Null),
        (&k.battle_player_mon_hp, json!(0)),
        (&k.battle_enemy_species, Value::Null),
        (&k.battle_enemy_level, json!(0)),
        (&k.battle_enemy_hp, json!(0)),
        (&k.battle_enemy_mon_party_pos, json!(0)),
        (&k.item_count, json!(1)),
        (&k.ball_count, json!(0)),
        (&k.key_item_count, json!(0)),
    ] {
        add(path, value);
    }
    for i in 0..6 {
        add(&k.team_species[i], if i == 0 { json!("Totodile") } else { Value::Null });
        add(&k.team_level[i], json!(if i == 0 { 5 } else { 0 }));
        for list in [&k.team_dv_attack, &k.team_dv_defense, &k.team_dv_speed, &k.team_dv_special] {
            add(&list[i], json!(15));
        }
    }
    for (i, m) in [json!("SCRATCH"), json!("LEER"), Value::Null, Value::Null].iter().enumerate() {
        add(&k.player_moves[i], m.clone());
    }
    for path in &k.stat_exp {
        add(path, json!(0));
    }
    for (i, path) in k.item_type.iter().enumerate() {
        add(path, if i == 0 { json!("POTION") } else if i == 1 { json!("--End of list--") } else { Value::Null });
        add(&k.item_quantity[i], json!(if i == 0 { 1 } else { 0 }));
    }
    for (i, path) in k.ball_type.iter().enumerate() {
        add(path, Value::Null);
        add(&k.ball_quantity[i], json!(0));
    }
    for path in &k.key_items {
        add(path, Value::Null);
    }
    for path in k.tm_keys.iter().chain(k.hm_keys.iter()) {
        add(path, json!(0));
    }
    json!({"meta": {"gameName": "Pokemon Crystal - Deprecated Mapper"}, "glossary": {}, "properties": props})
}

struct Sim {
    store: PropertyStore,
    machine: Gen2Machine,
    keys: Gen2Keys,
    seconds: i64,
    exp: i64,
}

impl Sim {
    fn set(&mut self, path: &str, value: Value) {
        let (new, old) = self.store.set_value(path, value).unwrap_or_else(|| panic!("unknown path {}", path));
        if new.value != old.value {
            self.machine.handle_event(&self.store, &new, &old);
        }
    }
    fn tick(&mut self, n: usize) {
        for _ in 0..n {
            self.seconds += 1;
            let k = self.keys.gametime_seconds.clone();
            self.set(&k, json!(self.seconds));
        }
    }
    /// A trainer battle. `sentinel`: the game's -1 (255) "no mon" write is seen
    /// as a change while setting up. `positions[i]`: the party position reported
    /// when the i-th mon comes out (`None` = no change seen, e.g. the lead's 0
    /// when the previous fight already left the value at 0).
    fn trainer_battle(&mut self, class: &str, id: i64, team: &[(&str, i64)], sentinel: bool, positions: &[Option<i64>]) {
        let k = self.keys.clone();
        self.set(&k.battle_trainer_class, json!(class));
        self.set(&k.battle_trainer_number, json!(id));
        self.set(&k.battle_trainer_total_pokemon, json!(team.len()));
        self.set(&k.battle_start, json!(1));
        self.set(&k.battle_mode, json!("Trainer"));
        if sentinel {
            self.set(&k.battle_enemy_mon_party_pos, json!(255));
        }
        self.tick(1);
        self.set(&k.battle_text_buffer, json!(format!("{} wants to battle!", class)));
        self.set(&k.battle_player_mon_party_pos, json!(0));
        self.set(&k.battle_player_mon_species, json!("Totodile"));
        self.set(&k.battle_player_mon_hp, json!(30));
        for (i, (species, level)) in team.iter().enumerate() {
            if let Some(p) = positions.get(i).copied().flatten() {
                self.set(&k.battle_enemy_mon_party_pos, json!(p));
            }
            self.set(&k.battle_enemy_species, json!(species));
            self.set(&k.battle_enemy_level, json!(level));
            self.set(&k.battle_enemy_hp, json!(20));
            if i == 0 {
                self.set(&k.battle_start, json!(0));
                self.tick(2);
            }
            self.set(&k.battle_enemy_hp, json!(0));
            self.tick(1);
            self.exp += 20;
            self.set(&k.mon_exppoints, json!(self.exp));
            self.tick(1);
        }
        self.set(&k.battle_mode, Value::Null);
        self.set(&k.battle_enemy_species, Value::Null);
        self.set(&k.battle_text_buffer, json!(""));
        self.tick(2);
    }
}

fn wait_for_events(log: &Arc<Mutex<Log>>, n: usize) -> Vec<EventDefinition> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let got: Vec<EventDefinition> = log.lock().unwrap().events.iter().map(|(_, d)| d.clone()).collect();
        if got.len() >= n && got.iter().filter(|d| d.trainer_def.is_some()).all(|d| d.notes.is_empty()) {
            // every trainer event has had its ROAR update applied (notes cleared)
            std::thread::sleep(Duration::from_millis(300));
            return log.lock().unwrap().events.iter().map(|(_, d)| d.clone()).collect();
        }
        assert!(Instant::now() < deadline, "recorder produced {:?}", got.iter().map(|d| d.notes.clone()).collect::<Vec<_>>());
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn crystal_mon_order_is_one_based_with_or_without_the_sentinel() {
    let root = xpr_core::consts::find_source_root().expect("source root");
    let reg = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    let gen = reg.get_version("Crystal").expect("crystal");
    let log = Arc::new(Mutex::new(Log::default()));
    let host = FakeHost { gen: gen.clone(), log: log.clone() };
    let controller = RecorderController::new(HostHandle::direct(Box::new(host)), Arc::new(|| {}));
    let info = StartInfo {
        version: "Crystal".into(),
        gen: gen.clone(),
        url: String::new(),
        debug_mode: false,
        dvs: Some(gen.make_stat_block(15, 15, 15, 15, 15, 15, false)),
        solo_species: Some("Totodile".into()),
    };
    let mut machine = Gen2Machine::new(controller.clone(), &info);
    let store = PropertyStore::from_mapper(&mapper()).unwrap();
    let (_watch, invalid) = machine.on_mapper_loaded(&store);
    assert!(invalid.is_empty(), "invalid keys: {:?}", invalid);
    machine.startup(&store);
    let keys = Gen2Keys::configure_for_mapper(Some("Pokemon Crystal - Deprecated Mapper"));
    let mut sim = Sim { store, machine, keys, seconds: 0, exp: 135 };
    sim.tick(4); // UNINITIALIZED -> OVERWORLD
    assert_eq!(controller.get_game_state(), Some(xpr_recorder::GameState::Overworld));

    // the game writes -1 while setting up, then 0 for the lead: Youngster Mikey (Pidgey, Rattata)
    sim.trainer_battle("YOUNGSTER", 2, &[("Pidgey", 2), ("Rattata", 4)], true, &[Some(0), Some(1)]);
    // a one-mon fight leaves the position at 0: Youngster Joey (Rattata)
    sim.trainer_battle("YOUNGSTER", 1, &[("Rattata", 4)], true, &[Some(0)]);
    // ... so in the next fight neither -1 nor the lead's 0 is seen as a change:
    // Sage Chow (three Bellsprout)
    sim.trainer_battle("SAGE", 1, &[("Bellsprout", 3), ("Bellsprout", 3), ("Bellsprout", 3)], false, &[None, Some(1), Some(2)]);
    // the third mon came out before the second (a roar): Sage Li (Bellsprout, Bellsprout, Hoothoot)
    sim.trainer_battle("SAGE", 9, &[("Bellsprout", 7), ("Hoothoot", 10), ("Bellsprout", 7)], true, &[Some(0), Some(2), Some(1)]);

    let events = wait_for_events(&log, 4);
    let orders: Vec<(String, Vec<i64>)> = events
        .iter()
        .filter_map(|d| d.trainer_def.as_ref().map(|t| (t.trainer_name.clone(), t.mon_order.clone())))
        .collect();
    assert_eq!(
        orders,
        vec![
            ("Youngster Mikey".to_string(), vec![1, 2]),
            ("Youngster Joey".to_string(), vec![1]),
            ("Sage Chow".to_string(), vec![1, 2, 3]),
            ("Sage Li".to_string(), vec![1, 3, 2]),
        ]
    );
    // and what the route engine makes of them: the whole party, in battle order
    for (def, expect) in events.iter().filter(|d| d.trainer_def.is_some()).zip([vec!["Pidgey", "Rattata"], vec!["Rattata"], vec!["Bellsprout", "Bellsprout", "Bellsprout"], vec!["Bellsprout", "Hoothoot", "Bellsprout"]]) {
        let names: Vec<String> = def.pokemon_list(&gen).expect("pokemon list").iter().map(|p| p.name.clone()).collect();
        assert_eq!(names, expect);
    }
    sim.machine.shutdown();
}
