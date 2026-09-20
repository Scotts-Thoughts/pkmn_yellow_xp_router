//! Drives the Emerald FSM through trainer battles with a scripted property
//! store, delivering the properties the way the GameHook client does (one
//! `PropertiesChanged` batch at a time, in mapper order, `on_idle` after each)
//! and checks the `mon_order` / `thief_mons` the ROAR update writes.
//!
//! The game fills the enemy party, then raises `battle.type.is_battle`
//! (`BATTLE_TYPE_IS_MASTER`), then clears `battle.outcome`; when one poll
//! catches all three, the outcome and the flag come before the party in the
//! batch. Counting the party as the flag arrives used to read the *previous*
//! fight's team, so Flannery after a one-mon Kindler recorded as `[1]`.
//!
//! Also: a berry eaten mid-fight must not turn the Thief that follows into
//! a "Hold" from the bag (the held item the steal is compared against has to
//! follow the mon's real one, not stay at what it held as the fight began).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use xpr_data::model::StatBlock;
use xpr_data::{GenData, Registry};
use xpr_engine::{EventDefinition, LearnMoveEventDefinition, NodeId};
use xpr_recorder::controller::GameRecorder;
use xpr_recorder::games::gen3::{Gen3Keys, Gen3Machine};
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
        Some(self.gen.make_stat_block(30, 30, 31, 30, 31, 31, false))
    }
    fn final_solo_species(&self) -> Option<String> {
        Some("Houndoom".into())
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
        Some("Emerald".into())
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
    let k = Gen3Keys::configure(false);
    let mut props: Vec<Value> = Vec::new();
    let mut add = |path: &str, value: Value| props.push(json!({"path": path, "value": value}));
    for (path, value) in [
        (&k.dma_a, json!(33554432)),
        (&k.dma_b, json!(33554432)),
        (&k.dma_c, json!(33554432)),
        (&k.overworld_map, json!("LavaridgeTown Gym")),
        (&k.player_id, json!(7777)),
        (&k.player_money, json!(3000)),
        (&k.mon_exppoints, json!(21012)),
        (&k.mon_level, json!(31)),
        (&k.mon_species, json!("Houndoom")),
        (&k.mon_held_item, json!("CHERI BERRY")),
        (&k.mon_friendship, json!(70)),
        (&k.gametime_seconds, json!(0)),
        (&k.gametime_frames, json!(0)),
        (&k.battle_flag, json!(false)),
        (&k.trainer_battle_flag, json!(false)),
        (&k.double_battle_flag, json!(false)),
        (&k.battle_outcome, json!("WON")),
        (&k.battle_background_tiles, json!(0)),
        (&k.battle_player_mon_party_pos, json!(0)),
        (&k.battle_player_mon_hp, json!(0)),
        (&k.battle_ally_mon_party_pos, json!(0)),
        (&k.battle_ally_mon_hp, json!(0)),
        (&k.battle_trainer_a_number, json!(0)),
        (&k.battle_first_enemy_species, Value::Null),
        (&k.battle_first_enemy_level, json!(0)),
        (&k.battle_first_enemy_hp, json!(0)),
        (&k.battle_first_enemy_party_pos, json!(0)),
        (&k.battle_second_enemy_species, Value::Null),
        (&k.battle_second_enemy_level, json!(0)),
        (&k.battle_second_enemy_hp, json!(0)),
        (&k.battle_second_enemy_party_pos, json!(0)),
        (&k.audio_sound_effect_1, json!(0)),
        (&k.audio_sound_effect_2, json!(0)),
        (&k.sstp_tracking, json!(0)),
    ] {
        add(path, value);
    }
    for path in [&k.two_opponents_battle_flag, &k.tutorial_battle_flag].into_iter().flatten() {
        add(path, json!(false));
    }
    if let Some(path) = &k.battle_trainer_b_number {
        add(path, json!(0));
    }
    for i in 0..6 {
        add(&k.team_species[i], if i == 0 { json!("Houndoom") } else { Value::Null });
        add(&k.team_level[i], json!(if i == 0 { 31 } else { 0 }));
        for (list, iv) in [
            (&k.team_iv_attack, 30),
            (&k.team_iv_defense, 31),
            (&k.team_iv_speed, 31),
            (&k.team_iv_special_attack, 30),
            (&k.team_iv_special_defense, 31),
        ] {
            add(&list[i], json!(if i == 0 { iv } else { 0 }));
        }
        add(&k.enemy_team_species[i], Value::Null);
    }
    for (i, m) in [json!("BITE"), json!("EMBER"), json!("THIEF"), Value::Null].iter().enumerate() {
        add(&k.player_moves[i], m.clone());
    }
    for path in &k.stat_exp {
        add(path, json!(0));
    }
    for (types, quantities) in [
        (&k.item_type, &k.item_quantity),
        (&k.ball_type, &k.ball_quantity),
        (&k.berry_type, &k.berry_quantity),
        (&k.tmhm_type, &k.tmhm_quantity),
    ] {
        for (t, q) in types.iter().zip(quantities.iter()) {
            add(t, Value::Null);
            add(q, json!(0));
        }
    }
    for path in &k.key_items {
        add(path, Value::Null);
    }
    json!({"meta": {"gameName": "Pokemon Emerald - Deprecated Mapper"}, "glossary": {}, "properties": props})
}

struct Sim {
    store: PropertyStore,
    machine: Gen3Machine,
    keys: Gen3Keys,
    seconds: i64,
    exp: i64,
}

impl Sim {
    /// One `PropertiesChanged` batch: applied in the given (mapper) order, a
    /// change delivered per property as it is applied, then the idle pass.
    fn batch(&mut self, changes: &[(&str, Value)]) {
        for (path, value) in changes {
            let (new, old) = self.store.set_value(path, value.clone()).unwrap_or_else(|| panic!("unknown path {}", path));
            if new.value != old.value {
                self.machine.handle_event(&self.store, &new, &old);
            }
        }
        self.machine.on_idle(&self.store);
    }
    fn set(&mut self, path: &str, value: Value) {
        self.batch(&[(path, value)]);
    }
    fn tick(&mut self, n: usize) {
        for _ in 0..n {
            self.seconds += 1;
            let k = self.keys.gametime_seconds.clone();
            self.set(&k, json!(self.seconds));
        }
    }

    /// The trainer's team as the game leaves it in `gEnemyParty`.
    fn team_changes(&self, team: &[(&str, i64)]) -> Vec<(String, Value)> {
        (0..6).map(|i| (self.keys.enemy_team_species[i].clone(), team.get(i).map(|(s, _)| json!(s)).unwrap_or(Value::Null))).collect()
    }

    /// The lead enemy mon, sent out as the battle starts.
    fn lead_changes(&self, team: &[(&str, i64)]) -> Vec<(String, Value)> {
        let k = &self.keys;
        vec![
            (k.battle_first_enemy_party_pos.clone(), json!(0)),
            (k.battle_first_enemy_species.clone(), json!(team[0].0)),
            (k.battle_first_enemy_level.clone(), json!(team[0].1)),
            (k.battle_first_enemy_hp.clone(), json!(60)),
            (k.battle_player_mon_party_pos.clone(), json!(0)),
            (k.battle_player_mon_hp.clone(), json!(90)),
        ]
    }

    /// `trainerbattle`: the script sets the opponent id and the battle-type
    /// flags (which drops the IS_MASTER bit) before the transition animation.
    fn script_start(&mut self, id: i64) {
        let k = self.keys.clone();
        self.batch(&[(&k.battle_trainer_a_number, json!(id)), (&k.battle_flag, json!(false)), (&k.trainer_battle_flag, json!(true))]);
        self.tick(1);
    }

    /// The fight after the intro: every mon KO'd in party order, the solo
    /// mon's berry eaten against mon `eat_at`, a Thief on `steal_from`.
    fn fight_and_win(&mut self, team: &[(&str, i64)], eat_at: Option<usize>, steal_from: Option<(usize, &str)>) {
        let k = self.keys.clone();
        for (i, (species, level)) in team.iter().enumerate() {
            if i > 0 {
                self.batch(&[
                    (&k.battle_first_enemy_party_pos, json!(i)),
                    (&k.battle_first_enemy_species, json!(species)),
                    (&k.battle_first_enemy_level, json!(level)),
                    (&k.battle_first_enemy_hp, json!(60)),
                ]);
                self.tick(1);
            }
            if eat_at == Some(i) {
                self.set(&k.mon_held_item, Value::Null);
                self.tick(4); // the delayed held-item update fires
            }
            if let Some((pos, item)) = steal_from {
                if pos == i {
                    self.set(&k.mon_held_item, json!(item));
                    self.tick(4);
                }
            }
            self.set(&k.battle_first_enemy_hp, json!(0));
            self.tick(1);
            self.exp += 300;
            self.set(&k.mon_exppoints, json!(self.exp));
            self.tick(1);
        }
        self.set(&k.battle_outcome, json!("WON"));
        self.tick(1);
        self.set(&k.battle_background_tiles, json!(0));
        self.tick(2);
    }

    /// The battle start as the mock harness scripts it: the party, then the
    /// outcome reset, then the flag, each in its own batch.
    fn start_in_separate_batches(&mut self, id: i64, team: &[(&str, i64)]) {
        let k = self.keys.clone();
        self.script_start(id);
        let team_changes = self.team_changes(team);
        self.batch(&team_changes.iter().map(|(p, v)| (p.as_str(), v.clone())).collect::<Vec<_>>());
        let lead = self.lead_changes(team);
        self.batch(&lead.iter().map(|(p, v)| (p.as_str(), v.clone())).collect::<Vec<_>>());
        self.set(&k.battle_outcome, Value::Null);
        self.set(&k.battle_flag, json!(true));
        self.set(&k.battle_background_tiles, json!(44));
        self.tick(4);
    }

    /// The battle start as one GameHook poll catches it: outcome reset, flag,
    /// party and lead in a single batch, in mapper order.
    fn start_in_one_batch(&mut self, id: i64, team: &[(&str, i64)]) {
        let k = self.keys.clone();
        self.script_start(id);
        let mut changes: Vec<(String, Value)> = vec![(k.battle_outcome.clone(), Value::Null), (k.battle_flag.clone(), json!(true))];
        changes.extend(self.team_changes(team));
        changes.extend(self.lead_changes(team));
        changes.push((k.battle_background_tiles.clone(), json!(44)));
        self.batch(&changes.iter().map(|(p, v)| (p.as_str(), v.clone())).collect::<Vec<_>>());
        self.tick(4);
    }

    /// The flag's page read a poll earlier than the outcome's: the flag and
    /// party in one batch, the outcome reset in the next.
    fn start_flag_first(&mut self, id: i64, team: &[(&str, i64)]) {
        let k = self.keys.clone();
        self.script_start(id);
        let mut changes: Vec<(String, Value)> = vec![(k.battle_flag.clone(), json!(true))];
        changes.extend(self.team_changes(team));
        changes.extend(self.lead_changes(team));
        self.batch(&changes.iter().map(|(p, v)| (p.as_str(), v.clone())).collect::<Vec<_>>());
        self.batch(&[(&k.battle_outcome, Value::Null), (&k.battle_background_tiles, json!(44))]);
        self.tick(4);
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

const JACE: (i64, &[(&str, i64)]) = (204, &[("Slugma", 23)]);
const FLANNERY: (i64, &[(&str, i64)]) = (268, &[("Numel", 24), ("Slugma", 24), ("Camerupt", 26), ("Torkoal", 29)]);
const WINONA: (i64, &[(&str, i64)]) = (270, &[("Swablu", 29), ("Tropius", 29), ("Pelipper", 30), ("Skarmory", 31), ("Altaria", 33)]);

#[test]
fn emerald_counts_the_party_after_the_batch_that_raised_the_battle_flag() {
    let root = xpr_core::consts::find_source_root().expect("source root");
    let reg = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    let gen = reg.get_version("Emerald").expect("emerald");
    let log = Arc::new(Mutex::new(Log::default()));
    let host = FakeHost { gen: gen.clone(), log: log.clone() };
    let controller = RecorderController::new(HostHandle::direct(Box::new(host)), Arc::new(|| {}));
    let info = StartInfo {
        version: "Emerald".into(),
        gen: gen.clone(),
        url: String::new(),
        debug_mode: false,
        dvs: Some(gen.make_stat_block(30, 30, 31, 30, 31, 31, false)),
        solo_species: Some("Houndoom".into()),
    };
    let mut machine = Gen3Machine::new(controller.clone(), &info, false);
    let store = PropertyStore::from_mapper(&mapper()).unwrap();
    let (_watch, invalid) = machine.on_mapper_loaded(&store);
    assert!(invalid.is_empty(), "invalid keys: {:?}", invalid);
    machine.startup(&store);
    let keys = Gen3Keys::configure(false);
    let mut sim = Sim { store, machine, keys, seconds: 0, exp: 21012 };
    sim.tick(4); // UNINITIALIZED -> OVERWORLD
    assert_eq!(controller.get_game_state(), Some(xpr_recorder::GameState::Overworld));

    // Kindler Jace, scripted the way the mock harness does it
    sim.start_in_separate_batches(JACE.0, JACE.1);
    sim.fight_and_win(JACE.1, None, None);
    // Flannery: the whole start in one poll, straight after a one-mon fight;
    // the Cheri Berry is eaten against Camerupt, then the White Herb comes
    // off Torkoal (party position 3)
    sim.start_in_one_batch(FLANNERY.0, FLANNERY.1);
    sim.fight_and_win(FLANNERY.1, Some(2), Some((3, "WHITE HERB")));
    // Winona: the flag's poll landed before the outcome's
    sim.start_flag_first(WINONA.0, WINONA.1);
    sim.fight_and_win(WINONA.1, None, None);

    let events = wait_for_events(&log, 4);
    let orders: Vec<(String, Vec<i64>, Vec<i64>, Vec<i64>)> = events
        .iter()
        .filter_map(|d| d.trainer_def.as_ref().map(|t| (t.trainer_name.clone(), t.mon_order.clone(), t.exp_split.clone(), t.thief_mons.clone())))
        .collect();
    assert_eq!(
        orders,
        vec![
            ("Kindler Jace".to_string(), vec![1], vec![], vec![]),
            ("Leader Flannery".to_string(), vec![1, 2, 3, 4], vec![], vec![3]),
            ("Leader Winona".to_string(), vec![1, 2, 3, 4, 5], vec![], vec![]),
        ]
    );
    // the eaten berry is a "Hold None"; the steal is on the trainer event, not a "Hold White Herb"
    let holds: Vec<(Option<String>, bool)> = events.iter().filter_map(|d| d.hold_item.as_ref().map(|h| (h.item_name.clone(), h.consumed))).collect();
    assert_eq!(holds, vec![(None, true)]);
    // and what the route engine makes of them: the whole party, in battle order
    for (def, expect) in events.iter().filter(|d| d.trainer_def.is_some()).zip([
        vec!["Slugma"],
        vec!["Numel", "Slugma", "Camerupt", "Torkoal"],
        vec!["Swablu", "Tropius", "Pelipper", "Skarmory", "Altaria"],
    ]) {
        let names: Vec<String> = def.pokemon_list(&gen).expect("pokemon list").iter().map(|p| p.name.clone()).collect();
        assert_eq!(names, expect);
    }
    sim.machine.shutdown();
}
