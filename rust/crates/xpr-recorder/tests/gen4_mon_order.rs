//! Drives the Platinum machine through trainer battles with a scripted property
//! store and checks the `mon_order` the ROAR update writes when a send-out is
//! missed. A short list would lose a mon, and its exp, from the route.

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

struct Battle<'a> {
    id: i64,
    second_id: Option<i64>,
    team: &'a [&'a str],
    /// the party positions the feed shows, in order; the lead is seeded
    shown: &'a [i64],
}

impl Sim {
    /// A won trainer battle.
    fn win(&mut self, b: Battle) {
        let species: Vec<String> = (0..6).map(|i| format!("battle.opponent.team.{}.species", i)).collect();
        let mut changes: Vec<(&str, Value)> = vec![("battle.opponent.id", json!(b.id)), ("battle.mode", json!("Trainer"))];
        if let Some(id) = b.second_id {
            changes.push(("battle.opponent_2.id", json!(id)));
        }
        for (i, name) in b.team.iter().enumerate() {
            changes.push((&species[i], json!(name)));
        }
        changes.push(("meta.state", json!("Battle")));
        self.batch(&changes);
        self.tick(8); // into BATTLE, then the delayed initialization
        for pos in b.shown {
            self.batch(&[("battle.opponent.party_position", json!(pos))]);
            self.tick(1);
        }
        // far more than any prize, so the update leaves a Pay Day on the event
        self.batch(&[("bag.money", json!(103000))]);
        self.batch(&[("meta.state", json!("Overworld"))]);
        self.tick(2);
    }
}

/// The trainer event once its ROAR update has landed: the opening event has no
/// Pay Day, only the update can give it one.
fn trainer_event(log: &Arc<Mutex<Log>>) -> EventDefinition {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let got = log.lock().unwrap().events.iter().map(|(_, d)| d.clone()).find(|d| d.trainer_def.as_ref().map(|t| t.pay_day_amount.unwrap_or(0) > 0).unwrap_or(false));
        if let Some(d) = got {
            return d;
        }
        assert!(Instant::now() < deadline, "recorder produced {:?}", log.lock().unwrap().events.iter().map(|(_, d)| format!("{:?}", d)).collect::<Vec<_>>());
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn platinum() -> Arc<GenData> {
    let root = xpr_core::consts::find_source_root().expect("source root");
    Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new())).get_version("Platinum").expect("platinum")
}

#[test]
fn a_missed_send_out_keeps_the_whole_team() {
    let gen = platinum();
    // Youngster Austin: Luxio's send-out or Gligar's is dropped. Seen slots first,
    // in the order seen, then what is left in party order
    for (shown, order, expect) in [
        (vec![2], vec![1, 3, 2], ["Buizel", "Gligar", "Luxio"]),
        (vec![1], vec![1, 2, 3], ["Buizel", "Luxio", "Gligar"]),
    ] {
        let (mut sim, log, _controller) = start();
        sim.win(Battle { id: 33, second_id: None, team: &["Buizel", "Luxio", "Gligar"], shown: &shown });
        let def = trainer_event(&log);
        assert_eq!(def.trainer_def.as_ref().unwrap().mon_order, order);
        let names: Vec<String> = def.pokemon_list(&gen).unwrap().iter().map(|p| p.name.clone()).collect();
        assert_eq!(names, expect);
        sim.machine.shutdown();
    }
}

#[test]
fn every_send_out_seen_is_still_the_order_seen() {
    let gen = platinum();
    let (mut sim, log, _controller) = start();
    sim.win(Battle { id: 33, second_id: None, team: &["Buizel", "Luxio", "Gligar"], shown: &[2, 1] });
    let def = trainer_event(&log);
    assert_eq!(def.trainer_def.as_ref().unwrap().mon_order, vec![1, 3, 2]);
    assert_eq!(def.pokemon_list(&gen).unwrap().len(), 3);
    sim.machine.shutdown();
}

/// Two trainers interleave their mons, so no custom order is recorded for them.
#[test]
fn a_two_trainer_battle_records_no_order() {
    let gen = platinum();
    let (mut sim, log, _controller) = start();
    sim.win(Battle { id: 201, second_id: Some(204), team: &["Wurmple", "Silcoon", "Beautifly"], shown: &[2] });
    let def = trainer_event(&log);
    let td = def.trainer_def.as_ref().unwrap();
    assert_eq!(td.trainer_name, "Bug Catcher Jack");
    assert!(td.mon_order.is_empty(), "{:?}", td.mon_order);
    assert_eq!(def.pokemon_list(&gen).unwrap().len(), 4);
    sim.machine.shutdown();
}

// exp_split in double battles. The game keeps one exp mask per enemy field slot
// (`sideGetExpMask`): an enemy coming in clears its slot's mask and flags who is out,
// one of ours coming in is flagged on both, and the count at the faint skips mons at
// 0 HP (`BtlCmd_CalcExpGain`). The scripts below never touch
// `battle.opponent_2.active_pokemon`: HeartGold's mapper has that at the wrong address.

const SOLO: i64 = 70675;
const ALLY: i64 = 111;
const NEXT_ALLY: i64 = 222;

impl Sim {
    /// Into a double battle with the solo mon and `ALLY` out and `NEXT_ALLY` behind them.
    fn start_double(&mut self, id: i64, second_id: i64, team: &[&str], second_team: &[&str]) {
        let mut changes: Vec<(String, Value)> = vec![("battle.opponent.id".into(), json!(id)), ("battle.opponent_2.id".into(), json!(second_id)), ("battle.ally.id".into(), json!(0)), ("battle.mode".into(), json!("Trainer"))];
        for (i, pid) in [SOLO, ALLY, NEXT_ALLY].iter().enumerate() {
            changes.push((format!("battle.player.team.{}.internals.personality_value", i), json!(pid)));
            changes.push((format!("battle.player.team.{}.stats.hp", i), json!(50)));
        }
        changes.push(("battle.player.active_pokemon.internals.personality_value".into(), json!(SOLO)));
        changes.push(("battle.player.active_pokemon_2.internals.personality_value".into(), json!(ALLY)));
        changes.push(("battle.player.active_pokemon.stats.hp".into(), json!(50)));
        changes.push(("battle.player.active_pokemon_2.stats.hp".into(), json!(50)));
        changes.push(("battle.opponent.active_pokemon.stats.hp".into(), json!(30)));
        for (i, name) in team.iter().enumerate() {
            changes.push((format!("battle.opponent.team.{}.species", i), json!(name)));
            changes.push((format!("battle.opponent.team.{}.stats.hp", i), json!(30)));
        }
        for (i, name) in second_team.iter().enumerate() {
            changes.push((format!("battle.opponent_2.team.{}.species", i), json!(name)));
            changes.push((format!("battle.opponent_2.team.{}.stats.hp", i), json!(30)));
        }
        changes.push(("meta.state".into(), json!("Battle")));
        let changes: Vec<(&str, Value)> = changes.iter().map(|(p, v)| (p.as_str(), v.clone())).collect();
        self.batch(&changes);
        self.tick(8);
    }

    /// The solo mon's exp bar moves.
    fn pay_exp(&mut self) {
        let exp = self.store.get_value(Some("battle.player.team.0.exp")).as_i64().unwrap_or(0) + 100;
        self.batch(&[("battle.player.team.0.exp", json!(exp))]);
    }

    /// The first slot's enemy, party slot `mon`, falls and pays out.
    fn first_faints(&mut self, mon: usize) {
        self.batch(&[(format!("battle.opponent.team.{}.stats.hp", mon).as_str(), json!(0)), ("battle.opponent.active_pokemon.stats.hp", json!(0))]);
        self.pay_exp();
    }

    /// The second slot's enemy, party slot `mon` of the second trainer's party (of
    /// the only party, for twins), falls and pays out.
    fn second_faints(&mut self, mon: usize, twins: bool) {
        let side = if twins { "opponent" } else { "opponent_2" };
        self.batch(&[(format!("battle.{}.team.{}.stats.hp", side, mon).as_str(), json!(0))]);
        self.pay_exp();
    }

    fn first_enemy_comes_in(&mut self, mon: i64) {
        self.batch(&[("battle.opponent.party_position", json!(mon)), ("battle.opponent.active_pokemon.stats.hp", json!(30))]);
    }

    fn second_enemy_comes_in(&mut self, mon: i64) {
        self.batch(&[("battle.opponent_2.party_position", json!(mon))]);
    }

    fn ally_faints(&mut self, party_slot: usize) {
        self.batch(&[(format!("battle.player.team.{}.stats.hp", party_slot).as_str(), json!(0)), ("battle.player.active_pokemon_2.stats.hp", json!(0))]);
    }

    fn ally_comes_in(&mut self, pid: i64) {
        self.batch(&[("battle.player.active_pokemon_2.internals.personality_value", json!(pid)), ("battle.player.active_pokemon_2.stats.hp", json!(50))]);
    }

    fn finish_battle(&mut self) {
        self.batch(&[("bag.money", json!(103000))]);
        self.batch(&[("meta.state", json!("Overworld"))]);
        self.tick(2);
    }
}

/// Galactic Grunts at Lake Verity (Platinum event 358): the ally falls and its
/// replacement comes in before the second trainer's later mons, so each of those
/// shares with it. All of it read with the second slot's active mon unseen.
#[test]
fn a_replacement_ally_shares_with_every_enemy_after_it() {
    let (mut sim, log, _controller) = start();
    sim.start_double(204, 201, &["Pachirisu"], &["Wurmple", "Silcoon", "Beautifly"]);
    sim.ally_faints(1);
    sim.first_faints(0);
    sim.ally_comes_in(NEXT_ALLY);
    sim.second_faints(0, false);
    sim.second_enemy_comes_in(1);
    sim.second_faints(1, false);
    sim.second_enemy_comes_in(2);
    sim.second_faints(2, false);
    sim.finish_battle();
    let def = trainer_event(&log);
    assert_eq!(def.trainer_def.as_ref().unwrap().exp_split, vec![1, 2, 2, 2]);
    sim.machine.shutdown();
}

/// The route deals two trainers' mons out in turn, so a split goes by that order
/// and not party after party.
#[test]
fn two_trainers_splits_follow_the_routes_order() {
    let gen = platinum();
    let (mut sim, log, _controller) = start();
    sim.start_double(201, 204, &["Wurmple", "Silcoon", "Beautifly"], &["Pachirisu"]);
    sim.first_faints(0);
    sim.second_faints(0, false);
    sim.ally_faints(1);
    sim.first_enemy_comes_in(1);
    sim.first_faints(1);
    sim.first_enemy_comes_in(2);
    sim.first_faints(2);
    sim.finish_battle();
    let def = trainer_event(&log);
    assert_eq!(def.trainer_def.as_ref().unwrap().exp_split, vec![2, 2, 1, 1]);
    let names: Vec<String> = def.get_pokemon_list(&gen, true).unwrap().into_iter().map(|(_, m)| m.name).collect();
    assert_eq!(names, ["Wurmple", "Pachirisu", "Silcoon", "Beautifly"]);
    sim.machine.shutdown();
}

/// Twins: one trainer named twice and one party behind both slots, which the
/// mapper's second team mirrors. The split has one entry per mon, not four.
#[test]
fn twins_split_both_mons() {
    let (mut sim, log, _controller) = start();
    sim.start_double(236, 236, &["Lumineon", "Rapidash"], &["Lumineon", "Rapidash"]);
    sim.second_faints(1, true);
    sim.first_faints(0);
    sim.finish_battle();
    let def = trainer_event(&log);
    assert_eq!(def.trainer_def.as_ref().unwrap().exp_split, vec![2, 2]);
    sim.machine.shutdown();
}

/// Only who is out when an enemy arrives counts: an ally that left alive before
/// it came in has no share of it, and one that was out for it does keep its
/// share after leaving.
#[test]
fn a_share_is_set_by_who_is_out_when_the_enemy_comes() {
    let (mut sim, log, _controller) = start();
    sim.start_double(204, 201, &["Pachirisu"], &["Wurmple", "Silcoon", "Beautifly"]);
    sim.second_faints(0, false);
    sim.second_enemy_comes_in(1);
    sim.ally_comes_in(NEXT_ALLY);
    // ALLY and NEXT_ALLY were both out against Silcoon and Pachirisu, alive
    sim.second_faints(1, false);
    sim.first_faints(0);
    sim.second_enemy_comes_in(2);
    sim.second_faints(2, false);
    sim.finish_battle();
    let def = trainer_event(&log);
    assert_eq!(def.trainer_def.as_ref().unwrap().exp_split, vec![3, 2, 3, 2]);
    sim.machine.shutdown();
}
