//! Drives the HeartGold machine through a Day Care stay (h-togekiss-line-2-24424).
//!
//! The game adds the exp a mon earned in the Day Care as it hands it back, and
//! teaches it the moves of every level that bought, deleting the first move and
//! appending the new one when all four slots are full (pokeheartgold
//! `Daycare_LearnLevelUpMoves`). HMs are not protected, so this is how a player
//! gets rid of one without the Move Deleter: Togepi went in with Rock Smash
//! first and came out at level 24 with Follow Me in its place. The mon is not in
//! slot 1 as it comes back, so the recorder used to see none of it, and the
//! route kept Rock Smash and called Follow Me ignored.

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
            Some(id) => log
                .events
                .iter()
                .position(|(i, _)| *i == id)?
                .checked_sub(1)?,
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
        self.log
            .lock()
            .unwrap()
            .events
            .retain(|(i, _)| !ids.contains(i));
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
        // the route's move list, which has Rock Smash in the last slot
        ["Substitute", "Headbutt", "Flamethrower", "Rock Smash"]
            .iter()
            .position(|m| *m == move_name)
            .map(|i| i as i64)
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
        Some(self.gen.make_stat_block(31, 31, 31, 31, 31, 31, false))
    }
    fn final_solo_species(&self) -> Option<String> {
        Some("Togepi".into())
    }
    fn final_held_item(&self) -> Option<String> {
        None
    }
    fn final_inventory_has(&self, _item_name: &str) -> bool {
        false
    }
    fn can_evolve_into(&self, species: &str) -> bool {
        species == "Pidgeot"
    }
    fn is_valid_levelup_move(&self, _def: &LearnMoveEventDefinition) -> bool {
        true
    }
    fn get_version(&self) -> Option<String> {
        Some("HeartGold".into())
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

/// A HeartGold mapper property list with every key the machine reads: a level
/// 23 Togepi with Rock Smash moved to the front, and a Rattata.
fn mapper(k: &Gen45Keys) -> Value {
    let mut paths = k.all_keys_to_register();
    paths.push(k.mon_friendship.clone());
    for list in [
        &k.team_level,
        &k.team_exp,
        &k.team_moves,
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
            "overworld.map_name" => json!("Route 34 - Map 3"),
            "player.player_id" => json!(1234),
            "bag.money" => json!(30000),
            "game_time.seconds" => json!(0),
            p if p.starts_with("player.team.0.") => togepi(p, 23, 10900, TOGEPI_IN),
            p if p.starts_with("player.team.1.") => rattata(p, 1),
            p if p.ends_with(".quantity") || p.contains(".evs.") => json!(0),
            _ => Value::Null,
        }
    };
    let props: Vec<Value> = paths
        .iter()
        .map(|p| json!({"path": p, "value": value(p)}))
        .collect();
    json!({"meta": {"gameName": "STP Pokemon HeartGold and SoulSilver"}, "glossary": {}, "properties": props})
}

const TOGEPI_IN: [&str; 4] = ["Rock Smash", "Headbutt", "Flamethrower", "Substitute"];
const TOGEPI_OUT: [&str; 4] = ["Headbutt", "Flamethrower", "Substitute", "Follow Me"];

/// One of Togepi's party fields, `path` being any party slot's.
fn togepi(path: &str, level: i64, exp: i64, moves: [&str; 4]) -> Value {
    let field = path.splitn(4, '.').nth(3).unwrap_or("");
    match field {
        "species" => json!("Togepi"),
        "level" => json!(level),
        "exp" => json!(exp),
        "stats.hp" => json!(60),
        "internals.personality_value" => json!(70400),
        f if f.starts_with("ivs.") => json!(31),
        f if f.starts_with("moves.") && f.ends_with(".move") => {
            json!(moves[f[6..7].parse::<usize>().unwrap()])
        }
        f if f.starts_with("evs.") => json!(0),
        _ => Value::Null,
    }
}

/// One of the Rattata's party fields, for the party slot `slot`.
fn rattata(path: &str, slot: usize) -> Value {
    let _ = slot;
    let field = path.splitn(4, '.').nth(3).unwrap_or("");
    match field {
        "species" => json!("Rattata"),
        "level" => json!(4),
        "exp" => json!(64),
        "stats.hp" => json!(19),
        "internals.personality_value" => json!(1111),
        f if f.starts_with("ivs.") => json!(8),
        "moves.0.move" => json!("Tackle"),
        "moves.1.move" => json!("Tail Whip"),
        f if f.starts_with("evs.") => json!(0),
        _ => Value::Null,
    }
}

struct Sim {
    store: PropertyStore,
    machine: Gen45Machine,
    paths: Vec<String>,
    seconds: i64,
}

impl Sim {
    /// One `PropertiesChanged` batch, then the idle pass.
    fn batch(&mut self, changes: &[(String, Value)]) {
        for (path, value) in changes {
            let (new, old) = self
                .store
                .set_value(path, value.clone())
                .unwrap_or_else(|| panic!("unknown path {}", path));
            if new.value != old.value {
                self.machine.handle_event(&self.store, &new, &old);
            }
        }
        self.machine.on_idle(&self.store);
    }

    fn tick(&mut self, n: usize) {
        for _ in 0..n {
            self.seconds += 1;
            self.batch(&[("game_time.seconds".to_string(), json!(self.seconds))]);
        }
    }

    /// Every field of party slot `slot`, from `f` (a path to its value).
    fn slot(&self, slot: usize, f: impl Fn(&str) -> Value) -> Vec<(String, Value)> {
        // the species first, as the mapper lists the party fields
        let prefix = format!("player.team.{}.", slot);
        let mut paths: Vec<&String> = self
            .paths
            .iter()
            .filter(|p| p.starts_with(&prefix))
            .collect();
        paths.sort_by_key(|p| !p.ends_with(".species"));
        paths.into_iter().map(|p| (p.clone(), f(p))).collect()
    }

    /// Togepi goes to the Day Care (or the PC): the Rattata moves up to slot 1.
    fn leave_party(&mut self) {
        let mut changes = self.slot(0, |p| rattata(p, 0));
        changes.extend(self.slot(1, |_| Value::Null));
        self.batch(&changes);
        self.tick(6);
    }

    /// The game puts a mon coming back in the last party slot, here slot 2.
    fn rejoin(&mut self, level: i64, exp: i64, moves: [&str; 4]) {
        let changes = self.slot(1, |p| togepi(p, level, exp, moves));
        self.batch(&changes);
        self.tick(6);
    }

    /// The player swaps the two, which puts Togepi back in slot 1.
    fn back_to_slot_1(&mut self, level: i64, exp: i64, moves: [&str; 4]) {
        let mut changes = self.slot(0, |p| togepi(p, level, exp, moves));
        changes.extend(self.slot(1, |p| rattata(p, 1)));
        self.batch(&changes);
        self.tick(6);
    }
}

fn heartgold() -> Arc<GenData> {
    let root = xpr_core::consts::find_source_root().expect("source root");
    Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()))
        .get_version("HeartGold")
        .expect("heartgold")
}

fn start() -> (Sim, Arc<Mutex<Log>>) {
    let gen = heartgold();
    let log = Arc::new(Mutex::new(Log::default()));
    let host = FakeHost {
        gen: gen.clone(),
        log: log.clone(),
    };
    let controller = RecorderController::new(HostHandle::direct(Box::new(host)), Arc::new(|| {}));
    let info = StartInfo {
        version: "HeartGold".into(),
        gen: gen.clone(),
        url: String::new(),
        debug_mode: false,
        dvs: Some(gen.make_stat_block(31, 31, 31, 31, 31, 31, false)),
        solo_species: Some("Togepi".into()),
    };
    let mut machine = Gen45Machine::new(controller.clone(), &info, Flavor::HeartGoldSoulSilver);
    let keys = Gen45Keys::configure(Flavor::HeartGoldSoulSilver);
    let mapper = mapper(&keys);
    let paths: Vec<String> = mapper["properties"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["path"].as_str().unwrap().to_string())
        .collect();
    let store = PropertyStore::from_mapper(&mapper).unwrap();
    let (_watch, invalid) = machine.on_mapper_loaded(&store);
    assert!(invalid.is_empty(), "invalid keys: {:?}", invalid);
    machine.startup(&store);
    let mut sim = Sim {
        store,
        machine,
        paths,
        seconds: 0,
    };
    sim.tick(4); // UNINITIALIZED -> OVERWORLD
    assert_eq!(controller.get_game_state(), Some(GameState::Overworld));
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
    log.events
        .iter()
        .map(|(_, d)| d.get_event_type().to_string())
        .collect()
}

#[test]
fn a_day_care_stay_records_its_exp_and_the_hm_it_pushed_out() {
    let (mut sim, log) = start();
    sim.leave_party();
    settle(&log, |_| false);
    assert!(
        log.lock().unwrap().events.is_empty(),
        "leaving the party records nothing: {:?}",
        describe(&log.lock().unwrap())
    );

    sim.rejoin(24, 11100, TOGEPI_OUT);
    settle(&log, |l| {
        l.events.iter().any(|(_, d)| d.day_care.is_some())
            && l.levelups.iter().any(|lm| lm.destination.is_some())
    });
    {
        let l = log.lock().unwrap();
        let day_care: Vec<i64> = l
            .events
            .iter()
            .filter_map(|(_, d)| d.day_care.map(|d| d.exp_gained))
            .collect();
        assert_eq!(day_care, vec![200], "{:?}", describe(&l));
        // the level-up table's Follow Me (24) goes over Rock Smash, wherever the
        // route has it, not "ignored"
        let follow_me = l
            .levelups
            .iter()
            .rev()
            .find(|lm| lm.move_to_learn.as_deref() == Some("Follow Me"))
            .expect("Follow Me on the level-up table");
        assert_eq!(follow_me.destination, Some(3), "{:?}", l.levelups);
        assert_eq!(follow_me.level.display(), "24");
        assert_eq!(follow_me.mon.as_deref(), Some("Togepi"));
    }

    // swapping it to the front adds nothing: the same stay is not counted twice,
    // and the moves it has are the ones it came back with
    let events_before = log.lock().unwrap().events.len();
    sim.back_to_slot_1(24, 11100, TOGEPI_OUT);
    sim.tick(6);
    settle(&log, |_| false);
    assert_eq!(
        log.lock().unwrap().events.len(),
        events_before,
        "{:?}",
        describe(&log.lock().unwrap())
    );

    // and the recorder knows them: the Move Deleter taking Headbutt is a
    // deletion of Headbutt, not of one of the Rattata's moves
    sim.batch(&[
        (
            "player.team.0.moves.0.move".to_string(),
            json!("Flamethrower"),
        ),
        (
            "player.team.0.moves.1.move".to_string(),
            json!("Substitute"),
        ),
        ("player.team.0.moves.2.move".to_string(), json!("Follow Me")),
        ("player.team.0.moves.3.move".to_string(), Value::Null),
    ]);
    sim.tick(8);
    settle(&log, |l| l.events.len() > events_before);
    let l = log.lock().unwrap();
    let deleted: Vec<(Option<String>, Option<i64>)> = l.events[events_before..]
        .iter()
        .filter_map(|(_, d)| d.learn_move.as_ref())
        .map(|lm| (lm.move_to_learn.clone(), lm.destination))
        .collect();
    assert_eq!(deleted, vec![(None, Some(1))], "{:?}", describe(&l));
}

/// A mon back from the PC comes back as it left, and whatever the mon that took
/// over slot 1 earned is not the solo mon's.
#[test]
fn a_mon_back_from_the_pc_records_no_day_care_exp() {
    let (mut sim, log) = start();
    sim.leave_party();
    sim.batch(&[("player.team.0.exp".to_string(), json!(900))]);
    sim.tick(2);
    sim.rejoin(23, 10900, TOGEPI_IN);
    settle(&log, |_| false);
    let l = log.lock().unwrap();
    assert!(l.events.is_empty(), "{:?}", describe(&l));
    assert!(
        l.levelups.iter().all(|lm| lm.destination.is_none()),
        "{:?}",
        l.levelups
    );
}
