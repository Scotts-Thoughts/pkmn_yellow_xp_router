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
//!
//! And the send-out prediction: a trainer event opens in the order the AI
//! will use, and the ROAR update replaces it with the order the game really used.
//!
//! And the Day Care: the exp, levels and moves a mon picks up there arrive all
//! at once as it is handed back, while it is not in slot 1.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use xpr_data::model::{Nature, StatBlock};
use xpr_data::{GenData, Registry};
use xpr_engine::state::SoloPokemonArgs;
use xpr_engine::{EventDefinition, LearnMoveEventDefinition, NodeId, SoloPokemon};
use xpr_recorder::controller::GameRecorder;
use xpr_recorder::games::gen3::{Gen3Keys, Gen3Machine};
use xpr_recorder::host::{HostHandle, PrevEvent, RecorderHost, StartInfo};
use xpr_recorder::{PropertyStore, RecorderController};

/// Every event the recorder added, in order, plus the updates it applied.
#[derive(Default)]
struct Log {
    events: Vec<(NodeId, EventDefinition)>,
    next_id: NodeId,
    /// the level-up moves the recorder set a destination or an "ignore" on
    levelup: Vec<LearnMoveEventDefinition>,
}

struct FakeHost {
    gen: Arc<GenData>,
    log: Arc<Mutex<Log>>,
    solo: Option<SoloPokemon>,
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
        Some(0)
    }
    fn update_levelup_move(&mut self, def: LearnMoveEventDefinition) {
        self.log.lock().unwrap().levelup.push(def);
    }
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
    fn final_solo_pkmn(&self) -> Option<SoloPokemon> {
        self.solo.clone()
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
    for i in 1..6 {
        add(&k.team_exp[i], json!(0));
    }
    for (i, m) in [json!("BITE"), json!("EMBER"), json!("THIEF"), json!("ROAR")].iter().enumerate() {
        add(&k.player_moves[i], m.clone());
    }
    for path in &k.team_moves[4..] {
        add(path, Value::Null);
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
    /// `fight_and_win`: the enemy mons whose send-out the feed drops
    missed_send_outs: Vec<usize>,
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

    /// `slot` of the party holding this mon (an empty species clears the slot).
    fn party_slot(&self, slot: usize, species: Value, level: i64, exp: i64, ivs: i64, moves: [Value; 4]) -> Vec<(String, Value)> {
        let k = &self.keys;
        let mut out = vec![
            (k.team_species[slot].clone(), species),
            (k.team_level[slot].clone(), json!(level)),
            (k.team_exp[slot].clone(), json!(exp)),
            (k.team_iv_attack[slot].clone(), json!(ivs)),
            (k.team_iv_defense[slot].clone(), json!(ivs + 1)),
            (k.team_iv_speed[slot].clone(), json!(ivs + 1)),
            (k.team_iv_special_attack[slot].clone(), json!(ivs)),
            (k.team_iv_special_defense[slot].clone(), json!(ivs + 1)),
        ];
        out.extend(moves.iter().enumerate().map(|(i, m)| (k.team_moves[slot * 4 + i].clone(), m.clone())));
        out
    }

    fn apply(&mut self, changes: Vec<(String, Value)>) {
        self.batch(&changes.iter().map(|(p, v)| (p.as_str(), v.clone())).collect::<Vec<_>>());
    }

    /// The solo mon goes to the Day Care or the PC, leaving a Poochyena in slot 1.
    fn leave_party(&mut self) {
        let lead = self.party_slot(0, json!("Poochyena"), 4, 64, 10, [json!("TACKLE"), Value::Null, Value::Null, Value::Null]);
        self.apply(lead);
        self.tick(4);
    }

    /// The game puts a mon coming back in the last party slot, here slot 2.
    fn rejoin(&mut self, level: i64, exp: i64, moves: [Value; 4]) {
        let back = self.party_slot(1, json!("Houndoom"), level, exp, 30, moves);
        self.apply(back);
        self.tick(4);
    }

    /// The player swaps the two, which puts the solo mon back in slot 1.
    fn back_to_slot_1(&mut self, level: i64, exp: i64, moves: [Value; 4]) {
        let mut swap = self.party_slot(0, json!("Houndoom"), level, exp, 30, moves);
        swap.extend(self.party_slot(1, json!("Poochyena"), 4, 64, 10, [json!("TACKLE"), Value::Null, Value::Null, Value::Null]));
        self.apply(swap);
        self.tick(4);
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
            if i > 0 && !self.missed_send_outs.contains(&i) {
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

fn emerald() -> Arc<GenData> {
    let root = xpr_core::consts::find_source_root().expect("source root");
    let reg = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    reg.get_version("Emerald").expect("emerald")
}

/// A recording in the overworld, the host's final state holding `solo`.
fn record(gen: &Arc<GenData>, solo: Option<SoloPokemon>) -> (Sim, Arc<Mutex<Log>>) {
    let log = Arc::new(Mutex::new(Log::default()));
    let host = FakeHost { gen: gen.clone(), log: log.clone(), solo };
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
    let mut sim = Sim { store, machine, keys, seconds: 0, exp: 21012, missed_send_outs: Vec::new() };
    sim.tick(4); // UNINITIALIZED -> OVERWORLD
    assert_eq!(controller.get_game_state(), Some(xpr_recorder::GameState::Overworld));
    (sim, log)
}

#[test]
fn emerald_counts_the_party_after_the_batch_that_raised_the_battle_flag() {
    let gen = emerald();
    let (mut sim, log) = record(&gen, None);

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

fn houndoom(gen: &GenData) -> SoloPokemon {
    let species = gen.pkmn_db().get_pkmn("Houndoom").expect("Houndoom").clone();
    let moves = ["Bite", "Ember", "Thief"].iter().map(|m| Some(m.to_string())).chain([None]).collect();
    let args = SoloPokemonArgs { move_list: Some(moves), ..SoloPokemonArgs::default() };
    let dvs = gen.make_stat_block(30, 30, 31, 30, 31, 31, false);
    SoloPokemon::new("Houndoom", species, dvs, gen.make_badge_list(), gen.make_stat_block(0, 0, 0, 0, 0, 0, true), 0, Nature::HARDY, args).expect("solo mon")
}

fn team_of(gen: &GenData, def: &EventDefinition) -> Vec<String> {
    def.pokemon_list(gen).expect("pokemon list").iter().map(|p| p.name.clone()).collect()
}

/// Winona against a Fire/Dark mon. Pass 1 skips Tropius and Skarmory, the hardest
/// hit, for having nothing super effective, so Pelipper (Water Gun) and Altaria
/// (Earthquake) come first. Pass 2 then has Skarmory's Sand Attack, a status move
/// that still scores (Ground x2), beat Tropius's Aerial Ace (STAB x1.5).
/// The event opens that way, before anything has fainted.
#[test]
fn a_trainer_event_opens_in_the_predicted_order_and_ends_in_the_real_one() {
    let gen = emerald();
    let (mut sim, log) = record(&gen, Some(houndoom(&gen)));

    sim.start_flag_first(WINONA.0, WINONA.1);
    let events = wait_for_events(&log, 1);
    let opened = events.iter().find(|d| d.trainer_def.is_some()).expect("trainer event");
    assert_eq!(opened.trainer_def.as_ref().unwrap().mon_order, vec![1, 5, 2, 4, 3]);
    assert_eq!(team_of(&gen, opened), ["Swablu", "Pelipper", "Altaria", "Skarmory", "Tropius"]);

    // the game goes another way (a voluntary switch, say): what it did wins
    sim.fight_and_win(WINONA.1, None, None);
    let deadline = Instant::now() + Duration::from_secs(10);
    while log.lock().unwrap().events.iter().flat_map(|(_, d)| d.trainer_def.as_ref()).any(|t| t.mon_order != vec![1, 2, 3, 4, 5]) {
        assert!(Instant::now() < deadline, "the ROAR update never replaced the prediction");
        std::thread::sleep(Duration::from_millis(20));
    }
    let events = wait_for_events(&log, 1);
    let closed = events.iter().find(|d| d.trainer_def.is_some()).expect("trainer event");
    assert_eq!(team_of(&gen, closed), ["Swablu", "Tropius", "Pelipper", "Skarmory", "Altaria"]);
    sim.machine.shutdown();
}

/// Frames 1142160-1159419 of `e-nosepass-2-35147`, on a Houndoom: in with four
/// moves at level 31; out with 12,000 exp from steps, level 35, and Odor Sleuth
/// (learned at 35) in place of the oldest move, with nothing to accept.
#[test]
fn a_day_care_stay_records_its_exp_and_the_moves_it_pushed_out() {
    let gen = emerald();
    let (mut sim, log) = record(&gen, None);
    sim.tick(1);

    sim.leave_party();
    assert!(log.lock().unwrap().events.is_empty(), "leaving the party records nothing");

    let moves = [json!("EMBER"), json!("THIEF"), json!("ROAR"), json!("ODOR SLEUTH")];
    sim.rejoin(35, 33012, moves.clone());
    let events = wait_for_events(&log, 1);
    assert_eq!(events.len(), 1, "{:?}", events.iter().map(|e| e.get_event_type()).collect::<Vec<_>>());
    assert_eq!(events[0].day_care.map(|d| d.exp_gained), Some(12000));
    let levelup = log.lock().unwrap().levelup.clone();
    let learned = levelup.iter().find(|l| l.destination == Some(0)).expect("Odor Sleuth over Bite");
    assert_eq!(learned.move_to_learn.as_deref(), Some("Odor Sleuth"));
    assert_eq!(learned.level.display(), "35");

    // swapping it to the front adds nothing: the same stay is not counted twice
    sim.back_to_slot_1(35, 33012, moves);
    sim.tick(4);
    let events = wait_for_events(&log, 1);
    assert_eq!(events.len(), 1, "{:?}", events.iter().map(|e| e.get_event_type()).collect::<Vec<_>>());
    sim.machine.shutdown();
}

/// A mon back from the PC comes back as it left, and whatever the mon that took
/// over slot 1 earned is not the solo mon's.
#[test]
fn a_mon_back_from_the_pc_records_no_day_care_exp() {
    let gen = emerald();
    let (mut sim, log) = record(&gen, None);
    sim.tick(1);

    sim.leave_party();
    let k = sim.keys.clone();
    sim.set(&k.mon_exppoints, json!(21900));
    sim.tick(1);
    sim.rejoin(31, 21012, [json!("BITE"), json!("EMBER"), json!("THIEF"), json!("ROAR")]);
    sim.tick(4);
    std::thread::sleep(Duration::from_millis(300));
    assert!(log.lock().unwrap().events.is_empty(), "{:?}", log.lock().unwrap().events.iter().map(|(_, e)| e.get_event_type()).collect::<Vec<_>>());
    sim.machine.shutdown();
}

/// The trainer event once the ROAR update has given it `order`.
fn wait_for_order(log: &Arc<Mutex<Log>>, order: &[i64]) -> EventDefinition {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let got = log.lock().unwrap().events.iter().map(|(_, d)| d.clone()).find(|d| d.trainer_def.is_some());
        if let Some(d) = &got {
            if d.trainer_def.as_ref().unwrap().mon_order == order {
                return d.clone();
            }
        }
        assert!(Instant::now() < deadline, "mon_order is {:?}", got.map(|d| d.trainer_def.unwrap().mon_order));
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_missed_send_out_keeps_the_whole_team() {
    let gen = emerald();
    let (mut sim, log) = record(&gen, None);
    // Numel out, Slugma's send-out dropped, then Camerupt and Torkoal: seen slots
    // first, then what is left in party order
    sim.missed_send_outs = vec![1];
    sim.start_flag_first(FLANNERY.0, FLANNERY.1);
    sim.fight_and_win(FLANNERY.1, None, None);
    let closed = wait_for_order(&log, &[1, 4, 2, 3]);
    assert_eq!(team_of(&gen, &closed), ["Numel", "Camerupt", "Torkoal", "Slugma"]);
    sim.machine.shutdown();

    // the last send-out dropped
    let (mut sim, log) = record(&gen, None);
    sim.missed_send_outs = vec![3];
    sim.start_flag_first(FLANNERY.0, FLANNERY.1);
    sim.fight_and_win(FLANNERY.1, None, None);
    let closed = wait_for_order(&log, &[1, 2, 3, 4]);
    assert_eq!(team_of(&gen, &closed), ["Numel", "Slugma", "Camerupt", "Torkoal"]);
    sim.machine.shutdown();
}

/// Winona opens as [Swablu, Pelipper, Altaria, Skarmory, Tropius]. The game then
/// shows only Swablu and Altaria: those are facts, and the three the feed lost
/// keep the prediction's order.
#[test]
fn a_missed_send_out_keeps_the_prediction_for_the_mons_not_seen() {
    let gen = emerald();
    let (mut sim, log) = record(&gen, Some(houndoom(&gen)));
    sim.missed_send_outs = vec![1, 2, 3];
    sim.start_flag_first(WINONA.0, WINONA.1);
    sim.fight_and_win(WINONA.1, None, None);
    let closed = wait_for_order(&log, &[1, 5, 3, 4, 2]);
    assert_eq!(team_of(&gen, &closed), ["Swablu", "Altaria", "Pelipper", "Skarmory", "Tropius"]);
    sim.machine.shutdown();
}
