//! Drives the HeartGold machine through two things a Pidgeot run did that the
//! recorder missed (h-pidgeot-line-2-23520).
//!
//! * Pidgeotto learned Wing Attack over Quick Attack at level 37 in a battle and
//!   evolved into Pidgeot right after it. HGSS writes the species, the level and
//!   the moves to the party in one frame, so the overworld saw the evolution but
//!   never the move change, and the route kept Quick Attack for good.
//! * The solo mon fainted, a teammate came out, and the mapper's game state
//!   read "Overworld" for the rest of the fight (it is "Battle" only while the
//!   slot-1 mon is active). The party is healed before the map changes to the
//!   Pokemon Center, so the blackout read as "team HP is not 0".

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
        ["Quick Attack", "Gust", "Roost", "Substitute"].iter().position(|m| *m == move_name).map(|i| i as i64)
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
        Some("Pidgeotto".into())
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

/// A HeartGold mapper property list with every key the machine reads: a
/// Pidgeotto at level 36 with Quick Attack / Gust / Roost / Substitute, and a
/// Rattata.
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
    paths.extend((0..6).map(|i| format!("battle.opponent.team.{}.species", i)));
    paths.sort();
    paths.dedup();
    let value = |path: &str| -> Value {
        match path {
            "meta.state" => json!("Overworld"),
            "overworld.map_name" => json!("Seafoam Islands - Map 3"),
            "player.player_id" => json!(1234),
            "bag.money" => json!(30000),
            "game_time.seconds" => json!(0),
            "player.team_count" => json!(2),
            "player.team.0.species" => json!("Pidgeotto"),
            "player.team.0.level" => json!(36),
            "player.team.0.exp" => json!(50000),
            "player.team.0.stats.hp" => json!(100),
            "player.team.0.internals.personality_value" => json!(70400),
            "player.team.0.moves.0.move" => json!("Quick Attack"),
            "player.team.0.moves.1.move" => json!("Gust"),
            "player.team.0.moves.2.move" => json!("Roost"),
            "player.team.0.moves.3.move" => json!("Substitute"),
            "player.team.1.species" => json!("Rattata"),
            "player.team.1.level" => json!(4),
            "player.team.1.stats.hp" => json!(19),
            p if p.starts_with("player.team.0.ivs.") => json!(31),
            p if p.starts_with("player.team.1.ivs.") => json!(8),
            p if p.ends_with(".quantity") || p.contains(".evs.") => json!(0),
            _ => Value::Null,
        }
    };
    let props: Vec<Value> = paths.iter().map(|p| json!({"path": p, "value": value(p)})).collect();
    json!({"meta": {"gameName": "STP Pokemon HeartGold and SoulSilver"}, "glossary": {}, "properties": props})
}

struct Sim {
    store: PropertyStore,
    machine: Gen45Machine,
    controller: Arc<RecorderController>,
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

    /// Into a trainer battle against one mon, with the solo mon (slot 1) and
    /// the Rattata (slot 2) on the battle's team.
    fn start_battle(&mut self) {
        self.batch(&[
            ("battle.opponent.id", json!(686)),
            ("battle.mode", json!("Trainer")),
            ("battle.opponent.team.0.species", json!("Seel")),
            ("battle.player.party_position", json!(0)),
            ("battle.player.team.0.stats.hp", json!(100)),
            ("battle.player.team.1.stats.hp", json!(19)),
            ("battle.player.active_pokemon.stats.hp", json!(100)),
            ("meta.state", json!("Battle")),
        ]);
        self.tick(8); // into BATTLE, then the delayed initialization
        assert_eq!(self.controller.get_game_state(), Some(GameState::Battle));
    }

    /// The solo mon faints and the Rattata comes out: the mapper reads the game
    /// state as "No Pokemon", then "Overworld", for the rest of the fight.
    fn solo_faints_teammate_comes_out(&mut self) {
        self.batch(&[("battle.player.active_pokemon.stats.hp", json!(0)), ("battle.player.team.0.stats.hp", json!(0))]);
        self.tick(1);
        self.batch(&[
            ("battle.player.active_pokemon.stats.hp", json!(19)),
            ("battle.player.party_position", json!(1)),
            ("meta.state", json!("No Pokemon")),
        ]);
        self.batch(&[("meta.state", json!("Overworld"))]);
        self.tick(1);
        assert_eq!(self.controller.get_game_state(), Some(GameState::Overworld));
    }
}

fn heartgold() -> Arc<GenData> {
    let root = xpr_core::consts::find_source_root().expect("source root");
    Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new())).get_version("HeartGold").expect("heartgold")
}

fn start() -> (Sim, Arc<Mutex<Log>>) {
    let gen = heartgold();
    let log = Arc::new(Mutex::new(Log::default()));
    let host = FakeHost { gen: gen.clone(), log: log.clone() };
    let controller = RecorderController::new(HostHandle::direct(Box::new(host)), Arc::new(|| {}));
    let info = StartInfo {
        version: "HeartGold".into(),
        gen: gen.clone(),
        url: String::new(),
        debug_mode: false,
        dvs: Some(gen.make_stat_block(31, 31, 31, 31, 31, 31, false)),
        solo_species: Some("Pidgeotto".into()),
    };
    let mut machine = Gen45Machine::new(controller.clone(), &info, Flavor::HeartGoldSoulSilver);
    let store = PropertyStore::from_mapper(&mapper(&Gen45Keys::configure(Flavor::HeartGoldSoulSilver))).unwrap();
    let (_watch, invalid) = machine.on_mapper_loaded(&store);
    assert!(invalid.is_empty(), "invalid keys: {:?}", invalid);
    machine.startup(&store);
    let mut sim = Sim { store, machine, controller: controller.clone(), seconds: 0 };
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

fn has_blackout(log: &Log) -> bool {
    log.events.iter().any(|(_, d)| d.blackout.is_some())
}

fn describe(log: &Log) -> Vec<String> {
    log.events.iter().map(|(_, d)| format!("{:?}", d)).collect()
}

#[test]
fn a_move_learned_just_before_an_evolution_is_recorded() {
    let (mut sim, log) = start();
    // the frame the evolution lands in the party: species, level and the new
    // move together (frame 774719 of the replay)
    sim.batch(&[
        ("player.team.0.species", json!("Pidgeot")),
        ("player.team.0.level", json!(37)),
        ("player.team.0.moves.0.move", json!("Wing Attack")),
    ]);
    sim.tick(6);
    settle(&log, |l| !l.levelups.is_empty());
    let log = log.lock().unwrap();
    assert!(log.events.iter().any(|(_, d)| d.evolution.is_some()), "no evolution event: {:?}", describe(&log));
    let learned: Vec<String> = log
        .levelups
        .iter()
        .filter(|lm| lm.move_to_learn.is_some())
        .map(|lm| format!("{} {:?} {} slot {:?}", lm.move_to_learn.as_deref().unwrap_or(""), lm.level, lm.mon.as_deref().unwrap_or(""), lm.destination))
        .collect();
    assert_eq!(learned, vec!["Wing Attack Int(37) Pidgeotto slot Some(0)".to_string()]);
}

#[test]
fn a_blackout_after_the_solo_mon_is_replaced_is_recorded() {
    let (mut sim, log) = start();
    sim.start_battle();
    sim.solo_faints_teammate_comes_out();
    // the Rattata faints too
    sim.batch(&[("battle.player.active_pokemon.stats.hp", json!(0)), ("battle.player.team.1.stats.hp", json!(0))]);
    sim.tick(2);
    // whited out: the party is healed a few frames before the map name changes
    sim.batch(&[("player.team.0.stats.hp", json!(0)), ("player.team.1.stats.hp", json!(0))]);
    sim.batch(&[("player.team.0.stats.hp", json!(100)), ("player.team.1.stats.hp", json!(19))]);
    sim.batch(&[("overworld.map_name", json!("Viridian City - Map 7"))]);
    // the battle's data is cleared afterwards
    sim.batch(&[("battle.player.party_position", json!(255))]);
    sim.tick(4);
    settle(&log, has_blackout);
    let log = log.lock().unwrap();
    assert!(has_blackout(&log), "no blackout recorded: {:?}", describe(&log));
}

/// h-typhlosion-line-2-23925 (Leader Clair, Ace Trainer Gaven): the solo mon
/// levelled up on the first enemy, then fainted, and the whole party after it.
/// The game writes the new level back to the party while whiting out, which
/// sends the recorder through its Rare Candy state; when it came back to the
/// overworld the party was already healed, so a build without the faint watch
/// called it "not a blackout" and kept the whole fight's exp.
#[test]
fn a_blackout_after_a_level_up_in_the_same_fight_is_recorded() {
    let (mut sim, log) = start();
    sim.start_battle();
    sim.solo_faints_teammate_comes_out();
    sim.batch(&[("battle.player.active_pokemon.stats.hp", json!(0)), ("battle.player.team.1.stats.hp", json!(0))]);
    sim.tick(2);
    sim.batch(&[("player.team.0.level", json!(37))]);
    sim.batch(&[("player.team.0.stats.hp", json!(100)), ("player.team.1.stats.hp", json!(19))]);
    sim.batch(&[("overworld.map_name", json!("Viridian City - Map 7"))]);
    sim.tick(1); // out of the Rare Candy state, on the new map with a healed party
    sim.batch(&[("battle.player.party_position", json!(255))]);
    sim.tick(4);
    settle(&log, has_blackout);
    let log = log.lock().unwrap();
    assert!(has_blackout(&log), "no blackout recorded: {:?}", describe(&log));
}

fn trainer_events(log: &Log) -> usize {
    log.events.iter().filter(|(_, d)| d.trainer_def.is_some()).count()
}

impl Sim {
    /// A Roar sends the Rattata out in place of the healthy solo mon: the mapper
    /// sets the party position, then reads the game state as "No Pokemon" and
    /// "Overworld" while the fight goes on.
    fn solo_roared_out(&mut self) {
        self.batch(&[
            ("battle.player.active_pokemon.stats.hp", json!(19)),
            ("battle.player.party_position", json!(1)),
            ("battle.mode", Value::Null),
            ("meta.state", json!("No Pokemon")),
        ]);
        self.batch(&[("meta.state", json!("Overworld"))]);
        self.tick(3);
        assert_eq!(self.controller.get_game_state(), Some(GameState::Battle), "the battle ended when the solo mon was roared out");
    }
}

/// h-typhlosion-line-2-23925: Leader Blue's Arcanine roared Typhlosion out, it
/// came back and won, and the route had Leader Blue twice.
#[test]
fn a_solo_mon_roared_out_and_back_is_one_battle() {
    let (mut sim, log) = start();
    sim.start_battle();
    sim.solo_roared_out();
    sim.batch(&[
        ("battle.player.party_position", json!(0)),
        ("battle.player.active_pokemon.stats.hp", json!(100)),
        ("battle.mode", json!("Trainer")),
        ("meta.state", json!("Battle")),
    ]);
    sim.tick(3);
    assert_eq!(sim.controller.get_game_state(), Some(GameState::Battle));
    // won with the solo mon out
    sim.batch(&[("meta.state", json!("From Battle"))]);
    sim.tick(2);
    sim.batch(&[("meta.state", json!("Overworld")), ("battle.player.party_position", json!(255))]);
    sim.tick(4);
    settle(&log, |l| trainer_events(l) > 1);
    assert_eq!(sim.controller.get_game_state(), Some(GameState::Overworld));
    let log = log.lock().unwrap();
    assert_eq!(trainer_events(&log), 1, "{:?}", describe(&log));
    assert!(!has_blackout(&log), "a blackout was recorded: {:?}", describe(&log));
}

#[test]
fn a_teammate_finishing_the_battle_with_the_solo_mon_out_ends_it() {
    let (mut sim, log) = start();
    sim.start_battle();
    sim.solo_roared_out();
    // the Rattata wins; the battle's data is cleared once it is over
    sim.batch(&[("battle.player.party_position", json!(255))]);
    sim.tick(4);
    assert_eq!(sim.controller.get_game_state(), Some(GameState::Overworld));
    settle(&log, |_| false);
    let log = log.lock().unwrap();
    assert_eq!(trainer_events(&log), 1, "{:?}", describe(&log));
    assert!(!has_blackout(&log), "a blackout was recorded: {:?}", describe(&log));
}

#[test]
fn a_teammate_winning_after_the_solo_faints_is_no_blackout() {
    let (mut sim, log) = start();
    sim.start_battle();
    sim.solo_faints_teammate_comes_out();
    // the Rattata wins; the battle's data is cleared once it is over
    sim.batch(&[("battle.player.party_position", json!(255))]);
    sim.batch(&[("battle.player.team.1.stats.hp", json!(0))]);
    sim.tick(4);
    sim.batch(&[("overworld.map_name", json!("Seafoam Islands - Map 2"))]);
    sim.tick(4);
    settle(&log, |_| false);
    let log = log.lock().unwrap();
    assert!(!has_blackout(&log), "a blackout was recorded: {:?}", describe(&log));
}

