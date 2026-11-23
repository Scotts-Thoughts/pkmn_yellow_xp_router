//! Drives the Crystal FSM through a Day Care stay. The game hands a mon back at
//! the level its step exp buys and teaches it the moves of those levels with
//! `FillMoves`, shifting the first move out when all four slots are full, HMs
//! included (pokecrystal `RetrieveBreedmon`). That is how a player drops an HM
//! without the Move Deleter, all the way back to Gen 2. The mon comes back
//! behind the one in slot 1, so the recorder used to see none of it.

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
    /// level-up moves the recorder set on the route's level-up table
    levelups: Vec<LearnMoveEventDefinition>,
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
    fn get_move_idx(&self, move_name: &str) -> Option<i64> {
        // the route's move list, which has Cut in the last slot
        ["Scratch", "Leer", "Rage", "Cut"].iter().position(|m| *m == move_name).map(|i| i as i64)
    }
    fn update_levelup_move(&mut self, def: LearnMoveEventDefinition) {
        self.log.lock().unwrap().levelups.push(def);
    }
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

const DEPRECATED: &str = "Pokemon Crystal - Deprecated Mapper";
const STANDARD: &str = "Pokemon Crystal";


/// The four moves as the mapper spells them.
type Moves = [&'static str; 4];
const TOTODILE_IN: Moves = ["CUT", "SCRATCH", "LEER", "RAGE"];
const TOTODILE_OUT: Moves = ["SCRATCH", "LEER", "RAGE", "WATER GUN"];
const RATTATA: Moves = ["TACKLE", "TAIL WHIP", "", ""];

fn mv(m: &str) -> Value {
    if m.is_empty() {
        Value::Null
    } else {
        json!(m)
    }
}

/// A property list for `game_name`'s mapper with every key the machine reads:
/// a level 12 Totodile with Cut moved to the front, and a Rattata.
fn mapper(game_name: &str) -> Value {
    let k = Gen2Keys::configure_for_mapper(Some(game_name));
    let mut props: Vec<Value> = Vec::new();
    let mut add = |path: &str, value: Value| props.push(json!({"path": path, "value": value}));
    for (path, value) in [
        (&k.overworld_map, json!(26)),
        (&k.overworld_map_num, json!(1)),
        (&k.overworld_x, json!(5)),
        (&k.overworld_y, json!(7)),
        (&k.player_id, json!(7777)),
        (&k.player_money, json!(3000)),
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
        (&k.battle_enemy_max_hp, json!(0)),
        (&k.battle_enemy_mon_party_pos, json!(0)),
        (&k.item_count, json!(1)),
        (&k.ball_count, json!(0)),
        (&k.key_item_count, json!(0)),
    ] {
        add(path, value);
    }
    for (path, value) in party_slot(&k, 0, Some(("Totodile", 12, 1000, 15, TOTODILE_IN))) {
        add(&path, value);
    }
    for (path, value) in party_slot(&k, 1, Some(("Rattata", 4, 64, 8, RATTATA))) {
        add(&path, value);
    }
    for i in 2..6 {
        for (path, value) in party_slot(&k, i, None) {
            add(&path, value);
        }
    }
    for path in k.stat_exp.iter().chain(k.battle_enemy_dvs.iter()) {
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
    for path in &k.scripted_heal_flags {
        add(path, json!(false));
    }
    if !k.audio_music_id.is_empty() {
        add(&k.audio_music_id, json!(0));
        add(&k.overworld_map_name, json!("Route 34"));
    }
    json!({"meta": {"gameName": game_name}, "glossary": {}, "properties": props})
}

/// Every field of party slot `slot`, species first as the mapper lists them:
/// (species, level, exp, DVs, moves), or an empty slot.
fn party_slot(k: &Gen2Keys, slot: usize, mon: Option<(&str, i64, i64, i64, Moves)>) -> Vec<(String, Value)> {
    let (species, level, exp, dvs, moves) = match mon {
        Some((s, l, e, d, m)) => (json!(s), json!(l), json!(e), json!(d), m),
        None => (Value::Null, json!(0), json!(0), json!(0), ["", "", "", ""]),
    };
    let mut out = vec![(k.team_species[slot].clone(), species), (k.team_level[slot].clone(), level), (k.team_exp[slot].clone(), exp)];
    for list in [&k.team_dv_attack, &k.team_dv_defense, &k.team_dv_speed, &k.team_dv_special] {
        out.push((list[slot].clone(), dvs.clone()));
    }
    out.extend(moves.iter().enumerate().map(|(i, m)| (k.team_moves[slot * 4 + i].clone(), mv(m))));
    out
}

struct Sim {
    store: PropertyStore,
    machine: Gen2Machine,
    keys: Gen2Keys,
    seconds: i64,
}

impl Sim {
    /// One `PropertiesChanged` batch, then the idle pass.
    fn batch(&mut self, changes: &[(String, Value)]) {
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
            let k = self.keys.gametime_seconds.clone();
            self.batch(&[(k, json!(self.seconds))]);
        }
    }
    /// Totodile goes to the Day Care (or the PC): the Rattata moves up to slot 1.
    fn leave_party(&mut self) {
        let mut changes = party_slot(&self.keys, 0, Some(("Rattata", 4, 64, 8, RATTATA)));
        changes.extend(party_slot(&self.keys, 1, None));
        self.batch(&changes);
        self.tick(6);
    }
    /// The game puts a mon coming back in the last party slot, here slot 2.
    fn rejoin(&mut self, level: i64, exp: i64, moves: Moves) {
        let changes = party_slot(&self.keys, 1, Some(("Totodile", level, exp, 15, moves)));
        self.batch(&changes);
        self.tick(6);
    }
    /// The player swaps the two, which puts Totodile back in slot 1.
    fn back_to_slot_1(&mut self, level: i64, exp: i64, moves: Moves) {
        let mut changes = party_slot(&self.keys, 0, Some(("Totodile", level, exp, 15, moves)));
        changes.extend(party_slot(&self.keys, 1, Some(("Rattata", 4, 64, 8, RATTATA))));
        self.batch(&changes);
        self.tick(6);
    }
}

fn start(game_name: &str) -> (Sim, Arc<Mutex<Log>>) {
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
    let store = PropertyStore::from_mapper(&mapper(game_name)).unwrap();
    let (_watch, invalid) = machine.on_mapper_loaded(&store);
    assert!(invalid.is_empty(), "invalid keys: {:?}", invalid);
    machine.startup(&store);
    let keys = Gen2Keys::configure_for_mapper(Some(game_name));
    let mut sim = Sim { store, machine, keys, seconds: 0 };
    sim.tick(4); // UNINITIALIZED -> OVERWORLD
    assert_eq!(controller.get_game_state(), Some(xpr_recorder::GameState::Overworld));
    (sim, log)
}

/// Waits for `done`, or a few seconds: the event queue is pumped on another thread.
fn settle(log: &Arc<Mutex<Log>>, done: impl Fn(&Log) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !done(&log.lock().unwrap()) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(300));
}

fn describe(log: &Log) -> Vec<String> {
    log.events.iter().map(|(_, d)| d.get_event_type().to_string()).collect()
}

/// Totodile goes in at level 12 with Cut first and comes out at 13 with Water
/// Gun in its place. The game sets its exp to the level 13 minimum, 1261.
#[test]
fn a_day_care_stay_records_its_exp_and_the_hm_it_pushed_out() {
    for game in [STANDARD, DEPRECATED] {
        let (mut sim, log) = start(game);
        sim.leave_party();
        settle(&log, |_| false);
        assert!(log.lock().unwrap().events.is_empty(), "{}: leaving the party records nothing: {:?}", game, describe(&log.lock().unwrap()));

        sim.rejoin(13, 1261, TOTODILE_OUT);
        settle(&log, |l| l.events.iter().any(|(_, d)| d.day_care.is_some()) && l.levelups.iter().any(|lm| lm.destination.is_some()));
        {
            let l = log.lock().unwrap();
            let day_care: Vec<i64> = l.events.iter().filter_map(|(_, d)| d.day_care.map(|d| d.exp_gained)).collect();
            assert_eq!(day_care, vec![261], "{}: {:?}", game, describe(&l));
            let water_gun = l.levelups.iter().rev().find(|lm| lm.move_to_learn.as_deref() == Some("Water Gun")).expect("Water Gun on the level-up table");
            assert_eq!(water_gun.destination, Some(3), "{}: Water Gun goes over Cut: {:?}", game, l.levelups);
            assert_eq!(water_gun.level.display(), "13");
        }

        // swapping it to the front adds nothing: the same stay is not counted twice
        let events_before = log.lock().unwrap().events.len();
        sim.back_to_slot_1(13, 1261, TOTODILE_OUT);
        sim.tick(6);
        settle(&log, |_| false);
        assert_eq!(log.lock().unwrap().events.len(), events_before, "{}: {:?}", game, describe(&log.lock().unwrap()));
        sim.machine.shutdown();
    }
}

/// A mon back from the PC comes back as it left, and whatever the mon that took
/// over slot 1 earned is not the solo mon's.
#[test]
fn a_mon_back_from_the_pc_records_no_day_care_exp() {
    let (mut sim, log) = start(STANDARD);
    sim.leave_party();
    let k = sim.keys.mon_exppoints.clone();
    sim.batch(&[(k, json!(900))]);
    sim.tick(2);
    sim.rejoin(12, 1000, TOTODILE_IN);
    settle(&log, |_| false);
    let l = log.lock().unwrap();
    assert!(l.events.is_empty(), "{:?}", describe(&l));
    assert!(l.levelups.iter().all(|lm| lm.destination.is_none()), "{:?}", l.levelups);
}
