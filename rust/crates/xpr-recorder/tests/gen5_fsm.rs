//! Drives the gen 5 machine through a Black session with a scripted property
//! store that behaves the way the games were measured to (see
//! `docs/rust_port/recording/gen5.md`): the party only changes when a battle
//! ends, the trainer id arrives a few frames after the battle word, enemies
//! faint in their per-party-slot battle structs, a party read can be garbage
//! for a moment, TMs are not used up, and saves flip `flags.new_game`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use xpr_data::model::StatBlock;
use xpr_data::{GenData, Registry};
use xpr_engine::{EventDefinition, LearnMoveEventDefinition, NodeId};
use xpr_recorder::controller::GameRecorder;
use xpr_recorder::games::gen45::Flavor;
use xpr_recorder::games::gen5::{Gen5Keys, Gen5Machine};
use xpr_recorder::host::{HostHandle, PrevEvent, RecorderHost, StartInfo};
use xpr_recorder::{GameState, PropertyStore, RecorderController};

#[derive(Default)]
struct Log {
    events: Vec<(NodeId, EventDefinition)>,
    levelups: Vec<LearnMoveEventDefinition>,
    messages: Vec<String>,
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
        Some(1)
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
        Some("Tepig".into())
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
        Some("Black".into())
    }
    fn gen(&self) -> Option<Arc<GenData>> {
        Some(self.gen.clone())
    }
    fn trigger_exception(&mut self, _msg: &str) {}
    fn send_message(&mut self, msg: &str) {
        self.log.lock().unwrap().messages.push(msg.to_string());
    }
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

const SOLO_PID: i64 = 3321910785;
const PLAYER_ID: i64 = 43059;
/// `flags.new_game` read from the trainer-info block's save counter (Black)
const SAVE_FLAG_ADDRESS: i64 = 0x2235014;

/// A Black mapper property list: the solo Tepig in slot 1, a small bag.
fn mapper(save_flag_address: i64) -> Value {
    let mut props: Vec<Value> = Vec::new();
    let mut add = |path: &str, value: Value| props.push(json!({"path": path, "value": value}));
    for (path, value) in [
        ("meta.state", json!("Overworld")),
        ("player.player_id", json!(PLAYER_ID)),
        ("player.team_count", json!(1)),
        ("bag.money", json!(3000)),
        ("overworld.map_name", json!("Nuvema Town")),
        ("overworld.map_index", json!(389)),
        ("game_time.seconds", json!(0)),
        ("battle.other.battle", json!(0)),
        ("battle.other.battle_end", json!(0)),
        ("battle.opponent.id", json!(550)),
        ("battle.opponent_2.id", json!(0)),
        ("battle.opponent.team_count", json!(0)),
        ("battle.opponent.party_position", json!(0)),
        ("battle.player.party_position", json!(0)),
    ] {
        add(path, value);
    }
    let slot0: [(&str, Value); 11] = [
        ("species", json!("Tepig")),
        ("level", json!(8)),
        ("exp", json!(400)),
        ("held_item", Value::Null),
        ("friendship", json!(70)),
        ("internals.personality_value", json!(SOLO_PID)),
        ("flags.skip_checksum", json!(0)),
        ("stats.hp", json!(30)),
        ("stats.hp_max", json!(30)),
        ("moves.0.move", json!("Tackle")),
        ("moves.1.move", json!("Tail Whip")),
    ];
    for i in 0..6 {
        for (suffix, value) in &slot0 {
            add(&format!("player.team.{}.{}", i, suffix), if i == 0 { value.clone() } else { Value::Null });
        }
        add(&format!("player.team.{}.moves.2.move", i), if i == 0 { json!("Ember") } else { Value::Null });
        add(&format!("player.team.{}.moves.3.move", i), Value::Null);
        for stat in ["hp", "attack", "defense", "speed", "special_attack", "special_defense"] {
            add(&format!("player.team.{}.ivs.{}", i, stat), json!(if i == 0 { 31 } else { 0 }));
            add(&format!("player.team.{}.evs.{}", i, stat), json!(0));
        }
        for field in ["species", "level", "stats.hp"] {
            add(&format!("battle.opponent.pkmn_ram.pokemon_{}_ram.{}", i, field), Value::Null);
        }
        add(&format!("battle.player.pkmn_ram.pokemon_{}_ram.stats.hp", i), json!(0));
        add(&format!("battle.opponent.team.{}.held_item", i), Value::Null);
    }
    let bag = [("items", 4), ("medicine", 4), ("berries", 2), ("tmhm", 4)];
    for (pocket, n) in bag {
        for i in 0..n {
            let (item, qty) = match (pocket, i) {
                ("medicine", 0) => (json!("Potion"), 2),
                ("medicine", 1) => (json!("Rare Candy"), 1),
                _ => (Value::Null, 0),
            };
            add(&format!("bag.{}.{}.item", pocket, i), item);
            add(&format!("bag.{}.{}.quantity", pocket, i), json!(qty));
        }
    }
    props.push(json!({"path": "flags.new_game", "value": true, "address": save_flag_address}));
    json!({"meta": {"gameName": "Pokemon Black - Beta"}, "glossary": {}, "properties": props})
}

struct Sim {
    store: PropertyStore,
    machine: Gen5Machine,
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
    /// Quiet long enough for the party and bag to count as settled.
    fn settle(&mut self) {
        std::thread::sleep(Duration::from_millis(450));
        self.machine.on_idle(&self.store);
    }
    fn battle_start(&mut self, opp: (&str, i64, i64), trainer: i64) {
        // the battle word first; the structs and the trainer id a few frames later
        self.batch(&[("battle.other.battle", json!(18002))]);
        self.batch(&[
            ("battle.other.battle", json!(21828)),
            ("battle.opponent.id", json!(0)),
            ("battle.opponent.team_count", json!(1)),
            ("battle.opponent.pkmn_ram.pokemon_0_ram.species", json!(opp.0)),
            ("battle.opponent.pkmn_ram.pokemon_0_ram.level", json!(opp.1)),
            ("battle.opponent.pkmn_ram.pokemon_0_ram.stats.hp", json!(opp.2)),
            ("battle.player.pkmn_ram.pokemon_0_ram.stats.hp", json!(30)),
        ]);
        if trainer != 0 {
            self.batch(&[("battle.opponent.id", json!(trainer))]);
        }
    }
    /// The outcome: `battle_end`, then the party update, then the word clears.
    fn battle_end(&mut self, party: &[(&str, Value)], money: i64) {
        self.batch(&[("battle.other.battle_end", json!(1))]);
        let mut changes: Vec<(&str, Value)> = party.to_vec();
        changes.push(("bag.money", json!(money)));
        changes.push(("battle.opponent.id", json!(0)));
        self.batch(&changes);
        self.batch(&[("battle.other.battle", json!(0)), ("battle.other.battle_end", json!(0)), ("battle.opponent.id", json!(550))]);
        self.settle();
    }
    /// A soft reset: the player id, the party and the money are cleared together.
    fn reset(&mut self) {
        self.batch(&[("player.player_id", json!(0)), ("player.team_count", json!(0)), ("bag.money", json!(0))]);
        self.settle();
    }
    /// The save loads: `state` on top of the party slot the reset left.
    fn load(&mut self, state: &[(&str, Value)]) {
        let mut changes: Vec<(&str, Value)> = vec![("player.team_count", json!(1)), ("player.player_id", json!(PLAYER_ID))];
        changes.extend(state.iter().cloned());
        self.batch(&changes);
        self.settle();
    }
}

fn wait_for(log: &Arc<Mutex<Log>>, what: &str, pred: impl Fn(&Log) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !pred(&log.lock().unwrap()) {
        let l = log.lock().unwrap();
        assert!(Instant::now() < deadline, "{}: events {:?}", what, l.events.iter().map(|(_, d)| format!("{:?}", d)).collect::<Vec<_>>());
        drop(l);
        std::thread::sleep(Duration::from_millis(20));
    }
    // let the processing thread finish what it started
    std::thread::sleep(Duration::from_millis(250));
}

fn setup(save_flag_address: i64) -> (Arc<Mutex<Log>>, Sim, Arc<RecorderController>) {
    setup_game("Black", Flavor::BlackWhite, save_flag_address)
}

fn setup_game(version: &str, flavor: Flavor, save_flag_address: i64) -> (Arc<Mutex<Log>>, Sim, Arc<RecorderController>) {
    let root = xpr_core::consts::find_source_root().expect("source root");
    let reg = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    let gen = reg.get_version(version).expect("version");
    let log = Arc::new(Mutex::new(Log::default()));
    let host = FakeHost { gen: gen.clone(), log: log.clone() };
    let controller = RecorderController::new(HostHandle::direct(Box::new(host)), Arc::new(|| {}));
    let info = StartInfo {
        version: version.into(),
        gen: gen.clone(),
        url: String::new(),
        debug_mode: false,
        dvs: Some(gen.make_stat_block(31, 31, 31, 31, 31, 31, false)),
        solo_species: Some("Tepig".into()),
    };
    let mut machine = Gen5Machine::new(controller.clone(), &info, flavor);
    let store = PropertyStore::from_mapper(&mapper(save_flag_address)).unwrap();
    let (watch, invalid) = machine.on_mapper_loaded(&store);
    assert!(invalid.is_empty(), "invalid keys: {:?}", invalid);
    assert_eq!(watch.len(), Gen5Keys::configure(&store).all().len());
    machine.startup(&store);
    let mut sim = Sim { store, machine };
    sim.settle(); // UNINITIALIZED -> OVERWORLD
    assert_eq!(controller.get_game_state(), Some(GameState::Overworld));
    (log, sim, controller)
}

fn trainer_names(log: &Log) -> Vec<String> {
    log.events.iter().filter_map(|(_, d)| d.trainer_def.as_ref().map(|t| t.trainer_name.clone())).collect()
}

#[test]
fn black_session_records_what_the_game_did() {
    let (log, mut sim, _controller) = setup(SAVE_FLAG_ADDRESS);

    // Youngster Jimmy (id 1, Patrat Lv7): the id arrives after the structs;
    // Tepig reaches Lv9 and learns Odor Sleuth into its empty slot, and the
    // prize is 4 * 4 * 7
    sim.battle_start(("Patrat", 7, 23), 1);
    sim.batch(&[("battle.opponent.pkmn_ram.pokemon_0_ram.stats.hp", json!(0))]);
    sim.battle_end(
        &[
            ("player.team.0.level", json!(9)),
            ("player.team.0.exp", json!(700)),
            ("player.team.0.moves.3.move", json!("Odor Sleuth")),
            ("player.team.0.stats.hp_max", json!(33)),
            ("player.team.0.stats.hp", json!(20)),
        ],
        3112,
    );
    wait_for(&log, "trainer event", |l| l.events.iter().any(|(_, d)| d.trainer_def.as_ref().map(|t| t.trainer_name == "Youngster Jimmy" && d.notes.is_empty()).unwrap_or(false)));
    {
        let l = log.lock().unwrap();
        let jimmy = l.events.iter().find_map(|(_, d)| d.trainer_def.clone()).unwrap();
        assert_eq!(jimmy.pay_day_amount, Some(0), "the prize money is not Pay Day");
        assert_eq!(jimmy.mon_order, vec![1]);
        assert!(l.levelups.iter().any(|m| m.move_to_learn.as_deref() == Some("Odor Sleuth") && m.level.as_int() == Some(9)), "{:?}", l.levelups);
    }

    // the summary screen decrypts the party in place for a frame: nothing happens
    sim.batch(&[("player.team.0.species", json!("Mewtwo")), ("player.team.0.level", json!(86)), ("player.team.0.flags.skip_checksum", json!(3))]);
    sim.batch(&[("player.team.0.species", json!("Tepig")), ("player.team.0.level", json!(9)), ("player.team.0.flags.skip_checksum", json!(0))]);
    sim.settle();

    // a TM found and taught over Tail Whip: it stays in the bag
    sim.batch(&[("bag.tmhm.0.item", json!("TM94")), ("bag.tmhm.0.quantity", json!(1))]);
    sim.settle();
    sim.batch(&[("player.team.0.moves.1.move", json!("Rock Smash"))]);
    sim.settle();

    // a Rare Candy, then a Pokémon Center heal, then a save
    sim.batch(&[("bag.medicine.1.item", Value::Null), ("bag.medicine.1.quantity", json!(0)), ("player.team.0.level", json!(10)), ("player.team.0.stats.hp_max", json!(35)), ("player.team.0.stats.hp", json!(22))]);
    sim.settle();
    sim.batch(&[("player.team.0.stats.hp", json!(35))]);
    sim.settle();
    sim.batch(&[("flags.new_game", json!(false))]);
    sim.settle();

    // a wild Lillipup: no trainer id ever arrives
    sim.battle_start(("Lillipup", 6, 20), 0);
    sim.batch(&[("battle.opponent.pkmn_ram.pokemon_0_ram.stats.hp", json!(0))]);
    sim.battle_end(&[("player.team.0.exp", json!(1300))], 3112);

    wait_for(&log, "wild event", |l| l.events.iter().any(|(_, d)| d.wild_pkmn_info.is_some()));
    {
        let l = log.lock().unwrap();
        let kinds: Vec<String> = l
            .events
            .iter()
            .map(|(_, d)| {
                if let Some(t) = &d.trainer_def {
                    format!("trainer {}", t.trainer_name)
                } else if let Some(i) = &d.item_event_def {
                    format!("item {} x{} {}", i.item_name, i.item_amount, if i.is_acquire { "+" } else { "-" })
                } else if let Some(m) = &d.learn_move {
                    format!("learn {} from {}", m.move_to_learn.clone().unwrap_or_default(), m.source)
                } else if d.rare_candy.is_some() {
                    "candy".to_string()
                } else if d.heal.is_some() {
                    "heal".to_string()
                } else if d.save.is_some() {
                    "save".to_string()
                } else if let Some(w) = &d.wild_pkmn_info {
                    format!("wild {} {}", w.name, w.level)
                } else {
                    format!("{:?}", d)
                }
            })
            .collect();
        assert_eq!(
            kinds,
            vec![
                "trainer Youngster Jimmy",
                "item TM94 Rock Smash x1 +",
                "learn Rock Smash from TM94 Rock Smash",
                "candy",
                "heal",
                "save",
                "wild Lillipup 6",
            ]
        );
    }

    // Lv10 -> 17 in one fight, and the evolution already in the party update
    // that ends it (seen in White 2): Pignite learns Arm Thrust over Odor Sleuth
    sim.battle_start(("Lillipup", 6, 20), 0);
    sim.batch(&[("battle.opponent.pkmn_ram.pokemon_0_ram.stats.hp", json!(0))]);
    sim.battle_end(
        &[
            ("player.team.0.species", json!("Pignite")),
            ("player.team.0.level", json!(17)),
            ("player.team.0.exp", json!(5000)),
            ("player.team.0.moves.3.move", json!("Arm Thrust")),
            ("player.team.0.stats.hp_max", json!(55)),
            ("player.team.0.stats.hp", json!(40)),
        ],
        3112,
    );
    wait_for(&log, "evolution", |l| l.events.iter().any(|(_, d)| d.evolution.is_some()));
    {
        let l = log.lock().unwrap();
        let evolved_after_wild = l.events.iter().rev().take(2).map(|(_, d)| (d.wild_pkmn_info.is_some(), d.evolution.is_some())).collect::<Vec<_>>();
        assert_eq!(evolved_after_wild, vec![(false, true), (true, false)], "the wild fight, then the evolution");
        let arm_thrust = l.levelups.iter().find(|m| m.move_to_learn.as_deref() == Some("Arm Thrust")).expect("Arm Thrust");
        assert_eq!((arm_thrust.mon.as_deref(), arm_thrust.level.as_int()), (Some("Pignite"), Some(17)));
        assert!(l.levelups.iter().any(|m| m.move_to_learn.as_deref() == Some("Flame Charge") && m.mon.as_deref() == Some("Tepig")), "the levels on the way are walked");
    }

    // three Rare Candies to Lv20: the game asks which move Smog replaces only
    // after the level has changed, and the answer comes in a later update
    sim.batch(&[("bag.medicine.1.item", json!("Rare Candy")), ("bag.medicine.1.quantity", json!(3))]);
    sim.settle();
    sim.batch(&[("bag.medicine.1.item", Value::Null), ("bag.medicine.1.quantity", json!(0)), ("player.team.0.level", json!(20)), ("player.team.0.stats.hp_max", json!(62)), ("player.team.0.stats.hp", json!(47))]);
    sim.settle();
    sim.batch(&[("player.team.0.moves.0.move", json!("Smog"))]);
    sim.settle();
    wait_for(&log, "Smog", |l| l.levelups.iter().filter(|m| m.move_to_learn.as_deref() == Some("Smog")).count() >= 2);
    {
        let l = log.lock().unwrap();
        let smog: Vec<_> = l.levelups.iter().filter(|m| m.move_to_learn.as_deref() == Some("Smog")).collect();
        assert_eq!(smog.last().map(|m| (m.level.as_int(), m.mon.clone(), m.destination)), Some((Some(20), Some("Pignite".to_string()), Some(1))), "learned at 20 over Tackle");
        assert!(!l.events.iter().any(|(_, d)| d.learn_move.as_ref().map(|m| m.move_to_learn.as_deref() == Some("Smog")).unwrap_or(false)), "not a tutor move");
    }

    // a lost trainer fight: every HP 0 in the party update, money lost
    sim.battle_start(("Lillipup", 12, 30), 21);
    sim.batch(&[("battle.player.pkmn_ram.pokemon_0_ram.stats.hp", json!(0))]);
    sim.battle_end(&[("player.team.0.stats.hp", json!(0))], 2856);
    wait_for(&log, "blackout", |l| l.events.iter().any(|(_, d)| d.blackout.is_some()));
    // the heal with the warp is part of the blackout
    sim.batch(&[("player.team.0.stats.hp", json!(62))]);
    sim.settle();
    {
        let l = log.lock().unwrap();
        assert_eq!(trainer_names(&l), vec!["Youngster Jimmy".to_string()], "the lost fight is not kept");
        assert_eq!(l.events.iter().filter(|(_, d)| d.heal.is_some()).count(), 1);
    }

    // a soft reset, and the save made after the heal loads: everything after it goes
    sim.reset();
    sim.load(&[
        ("player.team.0.species", json!("Tepig")),
        ("player.team.0.level", json!(10)),
        ("player.team.0.exp", json!(700)),
        ("player.team.0.moves.0.move", json!("Tackle")),
        ("player.team.0.moves.3.move", json!("Odor Sleuth")),
        ("player.team.0.stats.hp_max", json!(35)),
        ("player.team.0.stats.hp", json!(35)),
        ("bag.money", json!(3112)),
    ]);
    wait_for(&log, "reset", |l| l.events.last().map(|(_, d)| d.save.is_some()).unwrap_or(false));
    let l = log.lock().unwrap();
    assert!(!l.events.iter().any(|(_, d)| d.wild_pkmn_info.is_some() || d.blackout.is_some() || d.evolution.is_some()), "the fights after the save are gone");
    assert_eq!(trainer_names(&l), vec!["Youngster Jimmy".to_string()]);
}

fn potions(log: &Log) -> usize {
    log.events.iter().filter(|(_, d)| d.item_event_def.as_ref().map(|i| i.item_name == "Potion").unwrap_or(false)).count()
}

fn saves(log: &Log) -> usize {
    log.events.iter().filter(|(_, d)| d.save.is_some()).count()
}

#[test]
fn a_white_2_reset_waits_for_the_real_player_id() {
    let (log, mut sim, controller) = setup_game("White 2", Flavor::Black2White2, 0x221EAD4);
    assert!(log.lock().unwrap().messages.is_empty(), "saves can be seen");
    // a Potion bought, the game saved, another Potion bought
    sim.batch(&[("bag.medicine.0.quantity", json!(3)), ("bag.money", json!(2700))]);
    sim.settle();
    sim.batch(&[("flags.new_game", json!(false))]);
    sim.settle();
    sim.batch(&[("bag.medicine.0.quantity", json!(4)), ("bag.money", json!(2400))]);
    sim.settle();
    wait_for(&log, "purchases", |l| potions(l) == 2);
    let ready = controller.is_ready();

    // the save loads while the title screen is up; the player id reads garbage
    // (and 0 now and then) until the game goes on
    sim.reset();
    sim.batch(&[("player.team_count", json!(1)), ("bag.medicine.0.quantity", json!(3)), ("bag.money", json!(2700)), ("flags.new_game", json!(true))]);
    sim.settle();
    for id in [8354702, 0, 4096, 49, 0] {
        sim.batch(&[("player.player_id", json!(id))]);
        sim.settle();
        assert_eq!(controller.get_game_state(), Some(GameState::Resetting), "player id {}", id);
    }
    sim.batch(&[("player.player_id", json!(PLAYER_ID))]);
    sim.settle();
    assert_eq!(controller.get_game_state(), Some(GameState::Overworld));
    wait_for(&log, "back to the save", |l| l.events.last().map(|(_, d)| d.save.is_some()).unwrap_or(false) && potions(l) == 1);
    assert_eq!(controller.is_ready(), ready, "the route was not restarted");

    // the id reads 0 for a moment with the party still there: not a reset
    sim.batch(&[("player.player_id", json!(0))]);
    sim.batch(&[("player.player_id", json!(PLAYER_ID))]);
    sim.settle();
    assert_eq!(controller.get_game_state(), Some(GameState::Overworld));
    std::thread::sleep(Duration::from_millis(250));
    assert_eq!(saves(&log.lock().unwrap()), 1);
}

#[test]
fn a_black_2_reset_keeps_the_player_id() {
    let (log, mut sim, controller) = setup_game("Black 2", Flavor::Black2White2, 0x221EA94);
    sim.batch(&[("flags.new_game", json!(false))]);
    sim.settle();
    sim.batch(&[("bag.medicine.0.quantity", json!(1))]);
    sim.settle();
    wait_for(&log, "potion", |l| potions(l) == 1);
    // only the party and the money are cleared; the id turns to garbage after the save loads
    sim.batch(&[("player.team_count", json!(0)), ("bag.money", json!(0))]);
    sim.settle();
    assert_eq!(controller.get_game_state(), Some(GameState::Resetting));
    sim.batch(&[("player.team_count", json!(1)), ("bag.money", json!(3000)), ("bag.medicine.0.quantity", json!(2))]);
    for id in [31630, 0, 4096, 49] {
        sim.batch(&[("player.player_id", json!(id))]);
        sim.settle();
        assert_eq!(controller.get_game_state(), Some(GameState::Resetting), "player id {}", id);
    }
    sim.batch(&[("player.player_id", json!(PLAYER_ID))]);
    sim.settle();
    wait_for(&log, "back to the save", |l| potions(l) == 0 && l.events.last().map(|(_, d)| d.save.is_some()).unwrap_or(false));
}

#[test]
fn a_save_the_mapper_missed_is_found_when_the_game_loads_it() {
    // the Black and White mappers read the party block's save counter, which
    // a save with the party as it was at the previous save leaves alone
    let (log, mut sim, _controller) = setup(0x2234EE0);
    assert!(log.lock().unwrap().messages.is_empty());
    sim.batch(&[("flags.new_game", json!(false))]);
    sim.settle();
    sim.batch(&[("bag.medicine.0.quantity", json!(3)), ("bag.money", json!(2700))]);
    sim.settle();
    wait_for(&log, "purchase", |l| potions(l) == 1);
    // saved again (unseen), then reset: the save that loads has the Potion
    sim.reset();
    sim.load(&[("bag.medicine.0.quantity", json!(3)), ("bag.money", json!(2700))]);
    wait_for(&log, "the missed save", |l| saves(l) == 2);
    let l = log.lock().unwrap();
    assert!(l.events.last().unwrap().1.save.is_some());
    assert_eq!(potions(&l), 1, "the purchase was saved");
}

#[test]
fn without_saves_a_reset_is_undone_only_when_the_loaded_save_is_known() {
    // the Black 2 / White 2 mappers read `flags.new_game` from a byte that never changes
    let (log, mut sim, _controller) = setup(0x221DA14);
    assert_eq!(log.lock().unwrap().messages.len(), 1, "the user is told that saves cannot be seen");

    // a reset loads the state from before it: that is where the game was saved
    sim.reset();
    sim.load(&[("bag.money", json!(3000))]);
    wait_for(&log, "save found by the reset", |l| saves(l) == 1);

    // a Potion used, then a reset loads that save again: the Potion comes back
    sim.batch(&[("bag.medicine.0.quantity", json!(1))]);
    sim.settle();
    wait_for(&log, "potion", |l| potions(l) == 1);
    sim.reset();
    sim.load(&[("bag.money", json!(3000)), ("bag.medicine.0.quantity", json!(2))]);
    wait_for(&log, "rolled back", |l| potions(l) == 0);

    // a Potion used, saved without being seen after another one, then a reset:
    // the loaded save is no state the recorder knows, so it only leaves a note
    sim.batch(&[("bag.medicine.0.quantity", json!(1))]);
    sim.settle();
    wait_for(&log, "potion again", |l| potions(l) == 1);
    sim.reset();
    sim.load(&[("bag.money", json!(3000)), ("bag.medicine.0.quantity", json!(0))]);
    wait_for(&log, "reset note", |l| l.events.iter().any(|(_, d)| d.notes.contains("The game was reset")));
    assert_eq!(potions(&log.lock().unwrap()), 1, "nothing was rolled back");
}

#[test]
fn a_double_battle_orders_the_enemies_as_they_fell() {
    // Twins Kumi & Amy (id 18): both Purrloin come out together; the second one falls first
    let (log, mut sim, _controller) = setup(SAVE_FLAG_ADDRESS);
    sim.batch(&[("battle.other.battle", json!(18002))]);
    sim.batch(&[
        ("battle.other.battle", json!(21828)),
        ("battle.opponent.id", json!(0)),
        ("battle.opponent.team_count", json!(2)),
        ("battle.opponent.pkmn_ram.pokemon_0_ram.species", json!("Purrloin")),
        ("battle.opponent.pkmn_ram.pokemon_0_ram.level", json!(10)),
        ("battle.opponent.pkmn_ram.pokemon_0_ram.stats.hp", json!(30)),
        ("battle.opponent.pkmn_ram.pokemon_1_ram.species", json!("Purrloin")),
        ("battle.opponent.pkmn_ram.pokemon_1_ram.level", json!(10)),
        ("battle.opponent.pkmn_ram.pokemon_1_ram.stats.hp", json!(30)),
        ("battle.player.pkmn_ram.pokemon_0_ram.stats.hp", json!(30)),
    ]);
    sim.batch(&[("battle.opponent.id", json!(18))]);
    sim.batch(&[("battle.opponent.pkmn_ram.pokemon_1_ram.stats.hp", json!(0))]);
    sim.batch(&[("battle.opponent.pkmn_ram.pokemon_0_ram.stats.hp", json!(0))]);
    sim.battle_end(&[("player.team.0.exp", json!(900))], 3160);
    wait_for(&log, "twins", |l| l.events.iter().any(|(_, d)| d.trainer_def.as_ref().map(|t| t.trainer_name == "Twins Kumi & Amy" && d.notes.is_empty()).unwrap_or(false)));
    let l = log.lock().unwrap();
    let twins = l.events.iter().find_map(|(_, d)| d.trainer_def.clone()).unwrap();
    assert_eq!(twins.mon_order, vec![2, 1]);
    // one Pokémon in the party: nobody shares the experience
    assert!(twins.exp_split.is_empty());
}
