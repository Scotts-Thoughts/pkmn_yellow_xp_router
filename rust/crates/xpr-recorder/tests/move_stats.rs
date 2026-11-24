//! Move stats: scripted property changes go through a generation's detector
//! and the expected counts come out; the collector files battles correctly.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};

use xpr_data::{GenData, Registry};
use xpr_recorder::move_stats::{MoveDetector, MoveOutcome, MoveStatsCollector, Side, PREFIX};
use xpr_recorder::PropertyStore;

fn gen(version: &str) -> Arc<GenData> {
    let root = xpr_core::consts::find_source_root().expect("source root");
    let reg = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    reg.get_version(version).expect("version")
}

fn store(values: &[(&str, Value)]) -> PropertyStore {
    let props: Vec<Value> = values.iter().map(|(p, v)| json!({"path": format!("{}{}", PREFIX, p), "value": v})).collect();
    PropertyStore::from_mapper(&json!({"meta": {"gameName": "test"}, "properties": props})).unwrap()
}

struct Driver {
    store: PropertyStore,
    det: MoveDetector,
    out: Vec<MoveOutcome>,
}

impl Driver {
    fn new(gen: Arc<GenData>, values: &[(&str, Value)]) -> Driver {
        let store = store(values);
        let mut det = MoveDetector::for_gen(gen).unwrap();
        det.on_mapper_loaded(&store);
        det.begin();
        Driver { store, det, out: Vec::new() }
    }

    fn set(&mut self, name: &str, v: Value) {
        let path = format!("{}{}", PREFIX, name);
        let (new, old) = self.store.set_value(&path, v).unwrap_or_else(|| panic!("no {}", path));
        let o = self.det.on_change(&self.store, &new, &old);
        self.out.extend(o);
    }

    fn end(&mut self) -> Vec<MoveOutcome> {
        let o = self.det.end(&self.store);
        self.out.extend(o);
        std::mem::take(&mut self.out)
    }
}

fn gen4_battler(i: usize, species: &str, level: i64, hp: i64) -> Vec<(String, Value)> {
    let mut v = vec![
        (format!("battler_{}_species", i), json!(species)),
        (format!("battler_{}_level", i), json!(level)),
        (format!("battler_{}_hp", i), json!(hp)),
        (format!("battler_{}_status", i), json!(0)),
        (format!("battler_{}_volatile", i), json!(0)),
        (format!("battler_{}_ability", i), json!("")),
        (format!("battler_{}_item", i), json!("")),
    ];
    for s in ["atk", "def", "spe", "spa", "spd", "acc", "eva"] {
        v.push((format!("battler_{}_stage_{}", i, s), json!(6)));
    }
    for j in 0..4 {
        v.push((format!("battler_{}_pp_{}", i, j), json!(10)));
    }
    v
}

#[test]
fn gen4_hit_crit_burn_then_a_miss() {
    let mut vals: Vec<(String, Value)> = Vec::new();
    vals.extend(gen4_battler(0, "Infernape", 40, 120));
    vals.extend(gen4_battler(1, "Bronzor", 38, 100));
    for (k, v) in [
        ("attacker", json!(1)),
        ("defender", json!(0)),
        ("turn_order_counter", json!(5)),
        ("total_turns", json!(1)),
        ("battle_status_mask", json!(0)),
        ("move_status_flags", json!(0)),
        ("multi_hit_num_hits", json!(0)),
        ("action_command_0", json!(13)),
        ("action_command_1", json!(13)),
        ("action_order_0", json!(0)),
        ("action_order_1", json!(1)),
        ("move_current", json!("")),
    ] {
        vals.push((k.to_string(), v));
    }
    let refs: Vec<(&str, Value)> = vals.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    let mut d = Driver::new(gen("Platinum"), &refs);

    // the player's turn: Flamethrower, a critical super-effective hit that burns
    d.set("turn_order_counter", json!(0));
    d.set("attacker", json!(0));
    d.set("defender", json!(1));
    d.set("move_current", json!("Flamethrower"));
    d.set("battler_0_pp_0", json!(9));
    d.set("move_status_flags", json!(2 | 16));
    d.set("battler_1_hp", json!(20));
    d.set("battler_1_status", json!(16));
    // MOVE_END clears the flags as the counter moves on: the enemy's Hypnosis misses
    d.set("move_status_flags", json!(0));
    d.set("turn_order_counter", json!(1));
    d.set("attacker", json!(1));
    d.set("defender", json!(0));
    d.set("move_current", json!("Hypnosis"));
    d.set("battler_1_pp_0", json!(9));
    d.set("move_status_flags", json!(1));
    let out = d.end();

    assert_eq!(out.len(), 2, "{:#?}", out);
    let f = &out[0];
    assert_eq!(f.side, Some(Side::Player));
    assert_eq!(f.move_name, "Flamethrower");
    assert!(f.accuracy_rolled && !f.missed);
    assert_eq!(f.hit_chance, Some(1.0));
    assert_eq!((f.crit_rolls, f.crits), (1, 1));
    assert!(f.super_effective);
    assert!(f.effect_rolled && f.effect_applied, "{:#?}", f);
    assert!((f.effect_chance.unwrap() - 0.1).abs() < 1e-9);

    let h = &out[1];
    assert_eq!(h.side, Some(Side::Enemy));
    assert_eq!(h.move_name, "Hypnosis");
    assert!(h.accuracy_rolled && h.missed);
    assert!((h.hit_chance.unwrap() - 0.6).abs() < 1e-9, "{:?}", h.hit_chance);
    assert_eq!(h.crit_rolls, 0);
}

#[test]
fn gen4_a_turn_with_no_move_counts_nothing() {
    let mut vals: Vec<(String, Value)> = Vec::new();
    vals.extend(gen4_battler(0, "Infernape", 40, 120));
    vals.extend(gen4_battler(1, "Bronzor", 38, 100));
    for (k, v) in [
        ("attacker", json!(1)),
        ("defender", json!(0)),
        ("turn_order_counter", json!(5)),
        ("total_turns", json!(1)),
        ("move_status_flags", json!(0)),
        ("action_command_0", json!(13)),
        ("action_order_0", json!(0)),
        ("move_current", json!("Tackle")),
    ] {
        vals.push((k.to_string(), v));
    }
    let refs: Vec<(&str, Value)> = vals.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    let mut d = Driver::new(gen("Platinum"), &refs);
    // fully paralysed: the turn comes and goes, only end-of-turn damage shows
    d.set("turn_order_counter", json!(0));
    d.set("battler_1_hp", json!(90));
    d.set("turn_order_counter", json!(1));
    assert!(d.end().is_empty());
}

#[test]
fn gen2_turns_taken_open_a_move_and_effect_failed_is_a_failed_roll() {
    let mut vals: Vec<(String, Value)> = vec![
        ("attack_missed".into(), json!(0)),
        ("critical_hit".into(), json!(0)),
        ("effect_failed".into(), json!(0)),
        ("type_modifier".into(), json!(10)),
        ("cur_player_move".into(), json!("Thunderbolt")),
        ("cur_enemy_move".into(), json!("Tackle")),
        ("player_move_chance".into(), json!(25)),
        ("enemy_move_chance".into(), json!(0)),
        ("player_turns_taken".into(), json!(0)),
        ("enemy_turns_taken".into(), json!(0)),
    ];
    for (side, sp, lvl) in [("player", "Pikachu", 30), ("enemy", "Pidgeotto", 28)] {
        vals.push((format!("{}_species", side), json!(sp)));
        vals.push((format!("{}_level", side), json!(lvl)));
        vals.push((format!("{}_hp", side), json!(80)));
        vals.push((format!("{}_status", side), json!(0)));
        for i in 1..=5 {
            vals.push((format!("{}_substatus_{}", side, i), json!(0)));
        }
        for i in 0..7 {
            vals.push((format!("{}_stat_level_{}", side, i), json!(7)));
        }
        for i in 0..4 {
            vals.push((format!("{}_pp_{}", side, i), json!(15)));
        }
    }
    let refs: Vec<(&str, Value)> = vals.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    let mut d = Driver::new(gen("Crystal"), &refs);
    d.set("player_turns_taken", json!(1));
    d.set("player_pp_0", json!(14));
    d.set("type_modifier", json!(20));
    d.set("enemy_hp", json!(30));
    d.set("effect_failed", json!(1));
    // the enemy's turn: CheckTurn resets the flags, then DoTurn counts it
    d.set("effect_failed", json!(0));
    d.set("type_modifier", json!(10));
    d.set("enemy_turns_taken", json!(1));
    d.set("player_hp", json!(70));
    let out = d.end();
    assert_eq!(out.len(), 2, "{:#?}", out);
    let t = &out[0];
    assert_eq!(t.move_name, "Thunderbolt");
    // a 100% move's accuracy byte is 255, which gen 2 treats as a sure hit
    assert!(t.super_effective && !t.accuracy_rolled && !t.missed, "{:#?}", t);
    assert!(t.effect_rolled && !t.effect_applied);
    assert!((t.effect_chance.unwrap() - 25.0 / 256.0).abs() < 1e-9);
    assert_eq!(out[1].move_name, "Tackle");
    assert_eq!(out[1].side, Some(Side::Enemy));
}

fn outcome(side: Side, name: &str, missed: bool) -> MoveOutcome {
    MoveOutcome { side: Some(side), move_name: name.into(), accuracy_rolled: true, hit_chance: Some(0.9), missed, ..Default::default() }
}

#[test]
fn collector_files_trainer_wild_and_reset_battles() {
    let g = gen("Platinum");
    let mut c = MoveStatsCollector::default();
    c.start(g, None, Default::default(), "Platinum", "Infernape");

    // a trainer battle: the opening event is queued, then reaches the route
    c.mark_trainer();
    c.begin_battle();
    c.record(&outcome(Side::Player, "Flamethrower", false));
    c.record(&outcome(Side::Player, "Flamethrower", true));
    c.set_trainer_name("Youngster Tristan");
    c.end_battle(false);
    let ready = c.take_attachable();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].trainer.as_deref(), Some("Youngster Tristan"));
    assert_eq!(ready[0].outcome, "won");
    let fl = &ready[0].stats.player.moves["Flamethrower"];
    assert_eq!((fl.uses, fl.acc_rolls, fl.hits, fl.misses), (2, 2, 1, 1));
    assert!((fl.expected_hits - 1.8).abs() < 1e-9);

    // a wild battle goes into the run's wild total
    c.on_save();
    c.begin_battle();
    c.record(&outcome(Side::Enemy, "Tackle", false));
    c.end_battle(false);
    assert_eq!(c.run().unwrap().wild.enemy.moves["Tackle"].uses, 1);
    assert_eq!(c.total().enemy.moves["Tackle"].uses, 1);

    // a reset takes the wild battle since the save out, plus the battle in progress
    c.begin_battle();
    c.record(&outcome(Side::Player, "Flamethrower", false));
    c.on_reset(Vec::new());
    let run = c.run().unwrap();
    assert!(run.wild.enemy.moves.is_empty());
    assert_eq!(run.reset_attempts.len(), 2);
    assert!(run.reset_attempts.iter().all(|b| b.outcome == "reset"));
    assert!(c.total().enemy.moves.is_empty());
    assert_eq!(c.total().player.moves["Flamethrower"].uses, 2);

    let js = c.server_json();
    assert_eq!(js["battles"].as_array().unwrap().len(), 3);
    assert_eq!(js["format"], json!(1));
}
