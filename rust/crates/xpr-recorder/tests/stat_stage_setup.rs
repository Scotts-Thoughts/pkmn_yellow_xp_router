//! The stat-stage setup the recorder takes from a trainer battle: the solo
//! mon's own moves count (Swords Dance, a Charge Beam that boosted, its
//! Growl), the enemy's don't, and the counts land on the battle page's
//! matchups whatever order the trainer sent its mons out in.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};

use xpr_calc::battle_summary::{BattleSummary, SummaryConfig};
use xpr_data::model::CustomMoveData;
use xpr_data::{GenData, Registry};
use xpr_engine::state::SoloPokemonArgs;
use xpr_engine::{EventDefinition, Inventory, RouteState, SoloPokemon, TrainerEventDefinition};
use xpr_recorder::games::stage_setup::{remap_stat_stage_setup, StageSetupTracker};
use xpr_recorder::PropertyStore;

const STD_STATS: [&str; 7] = ["attack", "defense", "speed", "special_attack", "special_defense", "accuracy", "evasion"];

fn registry() -> Arc<Registry> {
    let root = xpr_core::consts::find_source_root().expect("source root");
    Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()))
}

/// A mapper with the active mons' stages, standard (`modifiers.*`) naming.
fn std_store() -> PropertyStore {
    let mut props = Vec::new();
    for side in ["player", "opponent"] {
        for stat in STD_STATS {
            props.push(json!({"path": format!("battle.{}.active_pokemon.modifiers.{}", side, stat), "value": 0}));
        }
    }
    PropertyStore::from_mapper(&json!({"meta": {"gameName": "test"}, "glossary": {}, "properties": props})).unwrap()
}

/// The deprecated gen 1 mapper: one Special stage, the enemy's as `modEnemyStage*`.
fn gen1_deprecated_store() -> PropertyStore {
    let mut props = Vec::new();
    for stat in ["Attack", "Defense", "Speed", "Special", "Accuracy", "Evasion"] {
        props.push(json!({"path": format!("battle.yourPokemon.modStage{}", stat), "value": "0"}));
        props.push(json!({"path": format!("battle.enemyPokemon.modEnemyStage{}", stat), "value": "0"}));
    }
    PropertyStore::from_mapper(&json!({"meta": {"gameName": "Pokemon Yellow (Deprecated)"}, "glossary": {}, "properties": props})).unwrap()
}

struct Battle {
    gen: Arc<GenData>,
    store: PropertyStore,
    tracker: StageSetupTracker,
    moves: Vec<Option<String>>,
    slot: Option<usize>,
    solo_active: bool,
    ms: u64,
}

impl Battle {
    fn new(gen: Arc<GenData>, store: PropertyStore, moves: &[&str]) -> Battle {
        xpr_recorder::clock::set_offline(true);
        xpr_recorder::clock::set_offline_ms(0);
        let mut tracker = StageSetupTracker::default();
        tracker.configure(&store);
        let mut b = Battle { gen, store, tracker, moves: moves.iter().map(|m| Some(m.to_string())).collect(), slot: Some(0), solo_active: true, ms: 0 };
        // the battle starts; the enemy's send-out settles
        b.observe();
        b.wait(5000);
        b
    }

    fn observe(&mut self) {
        xpr_recorder::clock::set_offline_ms(self.ms);
        self.tracker.observe(&self.gen, &self.store, self.slot, self.solo_active, &self.moves);
    }

    fn wait(&mut self, ms: u64) {
        self.ms += ms;
        self.observe();
    }

    /// One batch of stage changes (`side` "player" / "opponent"), then the
    /// rest of the turn.
    fn set(&mut self, changes: &[(&str, &str, i64)]) {
        for (side, stat, v) in changes {
            self.store.set_value(&format!("battle.{}.active_pokemon.modifiers.{}", side, stat), json!(v)).expect("known path");
        }
        self.wait(100);
    }

    fn turn(&mut self, changes: &[(&str, &str, i64)]) {
        self.set(changes);
        self.wait(6000);
    }

    fn send_out(&mut self, slot: usize) {
        self.slot = Some(slot);
        for stat in STD_STATS {
            self.store.set_value(&format!("battle.opponent.active_pokemon.modifiers.{}", stat), json!(0)).unwrap();
        }
        self.wait(100);
        self.wait(5000);
    }

    fn finish(&mut self) -> Vec<CustomMoveData> {
        let r = self.tracker.finish(&self.gen, &self.moves);
        xpr_recorder::clock::set_offline(false);
        r
    }
}

fn counts(setup: &[CustomMoveData], slot: usize) -> Vec<(String, String)> {
    setup.get(slot).map(|c| c.player.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default()
}

fn pair(m: &str, n: &str) -> (String, String) {
    (m.to_string(), n.to_string())
}

#[test]
fn three_swords_dances_are_swords_dance_3() {
    let reg = registry();
    let gen = reg.get_version("Platinum").unwrap();
    let mut b = Battle::new(gen, std_store(), &["Swords Dance", "Slash", "Fury Cutter", "Cut"]);
    b.turn(&[("player", "attack", 2)]);
    b.turn(&[("player", "attack", 4)]);
    b.turn(&[("player", "attack", 6)]);
    // a fourth changes nothing, and is nothing
    b.turn(&[]);
    let setup = b.finish();
    assert_eq!(counts(&setup, 0), vec![pair("Swords Dance", "3")]);
}

#[test]
fn the_enemys_stat_moves_are_left_out() {
    let reg = registry();
    let gen = reg.get_version("Platinum").unwrap();
    let mut b = Battle::new(gen, std_store(), &["Swords Dance", "Growl", "Slash", "Cut"]);
    // the enemy's Growl lowers the solo mon's Attack, its Harden raises its own Defense
    b.turn(&[("player", "attack", -1)]);
    b.turn(&[("opponent", "defense", 1)]);
    // the solo mon's Swords Dance and Growl
    b.turn(&[("player", "attack", 1)]);
    b.turn(&[("opponent", "attack", -1)]);
    let setup = b.finish();
    assert_eq!(counts(&setup, 0), vec![pair("Swords Dance", "1"), pair("Growl", "1")]);
}

#[test]
fn a_charge_beam_that_boosted_counts_and_one_that_didnt_doesnt() {
    let reg = registry();
    let gen = reg.get_version("Platinum").unwrap();
    let mut b = Battle::new(gen, std_store(), &["Charge Beam", "Thunderbolt"]);
    b.turn(&[("player", "special_attack", 1)]);
    b.turn(&[]);
    b.turn(&[("player", "special_attack", 2)]);
    let setup = b.finish();
    assert_eq!(counts(&setup, 0), vec![pair("Charge Beam", "2")]);
}

#[test]
fn a_dragon_dance_raising_one_stat_at_a_time_is_one_use() {
    let reg = registry();
    let gen = reg.get_version("Platinum").unwrap();
    let mut b = Battle::new(gen, std_store(), &["Dragon Dance", "Meteor Mash", "Outrage"]);
    // "Attack rose!", then a message later "Speed rose!"
    b.set(&[("player", "attack", 1)]);
    b.wait(1500);
    b.turn(&[("player", "speed", 1)]);
    // a Meteor Mash that boosted: +1 Attack and nothing more
    b.turn(&[("player", "attack", 2)]);
    let setup = b.finish();
    assert_eq!(counts(&setup, 0), vec![pair("Dragon Dance", "1"), pair("Meteor Mash", "1")]);
}

#[test]
fn setup_lands_on_the_enemy_it_was_used_against() {
    let reg = registry();
    let gen = reg.get_version("Platinum").unwrap();
    let mut b = Battle::new(gen, std_store(), &["Swords Dance", "Screech", "Slash"]);
    b.turn(&[("player", "attack", 2)]);
    b.turn(&[("opponent", "defense", -2)]);
    // the next one comes out; an Intimidate drops the solo mon's Attack
    b.send_out(2);
    b.turn(&[("player", "attack", 1)]);
    b.turn(&[("player", "attack", 3)]);
    let setup = b.finish();
    assert_eq!(counts(&setup, 0), vec![pair("Swords Dance", "1"), pair("Screech", "1")]);
    assert_eq!(counts(&setup, 1), vec![]);
    assert_eq!(counts(&setup, 2), vec![pair("Swords Dance", "1")]);
}

#[test]
fn an_intimidate_on_send_out_is_not_the_solo_mons_growl() {
    let reg = registry();
    let gen = reg.get_version("Platinum").unwrap();
    let mut b = Battle::new(gen, std_store(), &["Growl", "Tackle"]);
    // the solo mon's Intimidate as the next enemy comes out
    b.slot = Some(1);
    b.set(&[("opponent", "attack", -1)]);
    b.wait(5000);
    b.turn(&[("opponent", "attack", -2)]);
    let setup = b.finish();
    assert_eq!(counts(&setup, 1), vec![pair("Growl", "1")]);
}

#[test]
fn nothing_counts_while_a_teammate_is_out() {
    let reg = registry();
    let gen = reg.get_version("Platinum").unwrap();
    let mut b = Battle::new(gen, std_store(), &["Swords Dance"]);
    b.solo_active = false;
    b.turn(&[("player", "attack", 2)]);
    b.solo_active = true;
    b.turn(&[("player", "attack", 0)]);
    let setup = b.finish();
    assert!(setup.is_empty(), "{:?}", setup);
}

#[test]
fn belly_drum_holds_the_attack_stage_it_left() {
    let reg = registry();
    let gen = reg.get_version("Crystal").unwrap();
    let mut b = Battle::new(gen, std_store(), &["Belly Drum", "Return"]);
    b.turn(&[("player", "attack", -1)]);
    b.turn(&[("player", "attack", 6)]);
    let setup = b.finish();
    assert_eq!(counts(&setup, 0), vec![pair("Belly Drum", "6")]);
}

#[test]
fn gen_1_amnesia_raises_the_one_special_stage() {
    let reg = registry();
    let gen = reg.get_version("Yellow").unwrap();
    let mut b = Battle::new(gen, gen1_deprecated_store(), &["Amnesia", "Psychic"]);
    b.store.set_value("battle.yourPokemon.modStageSpecial", Value::String("2".into())).unwrap();
    b.wait(6000);
    b.store.set_value("battle.yourPokemon.modStageSpecial", Value::String("4".into())).unwrap();
    b.wait(6000);
    // a Psychic that lowered the enemy's Special
    b.store.set_value("battle.enemyPokemon.modEnemyStageSpecial", Value::String("-1".into())).unwrap();
    b.wait(6000);
    let setup = b.finish();
    assert_eq!(counts(&setup, 0), vec![pair("Amnesia", "2"), pair("Psychic", "1")]);
}

// ---- matchup order -------------------------------------------------------------------

fn solo_state(gen: &GenData, species: &str, level: i64, moves: &[&str]) -> RouteState {
    let def = gen.pkmn_db().get_pkmn(species).unwrap().clone();
    let xp = xpr_data::exp::level_lookup(&def.growth_rate).unwrap().get_xp_for_level(level).unwrap();
    let move_list: Vec<Option<String>> = moves.iter().map(|m| Some(m.to_string())).chain(std::iter::repeat(None)).take(4).collect();
    let mon = SoloPokemon::new(
        species,
        def,
        gen.make_stat_block(15, 15, 15, 15, 15, 15, false),
        gen.make_badge_list(),
        gen.make_stat_block(0, 0, 0, 0, 0, 0, true),
        0,
        Default::default(),
        SoloPokemonArgs { move_list: Some(move_list), cur_xp: xp, ..Default::default() },
    )
    .unwrap();
    RouteState::new(mon, gen.make_badge_list(), Inventory::new(None, Vec::new(), gen.bag_limit()))
}

/// The battle page's setup per matchup (the order the mons were fought in).
fn page_setup(gen: &GenData, ev: &EventDefinition) -> Vec<Vec<(String, String)>> {
    let state = Arc::new(solo_state(gen, "Scizor", 50, &["Swords Dance", "X-Scissor"]));
    let fights: Vec<_> = ev.pokemon_list(gen).unwrap().into_iter().map(|m| (m, state.clone())).collect();
    let mut s = BattleSummary::new();
    s.load_from_parts(gen, 1, ev, &fights, &SummaryConfig::default()).unwrap();
    (0..fights.len()).map(|i| counts(&s.stat_stage_setup, i)).collect()
}

fn setup_for(move_name: &str, n: i64) -> CustomMoveData {
    let mut c = CustomMoveData::default();
    c.player.insert(move_name.to_string(), n.to_string());
    c
}

#[test]
fn the_counts_follow_each_mon_to_its_matchup_in_any_send_out_order() {
    let reg = registry();
    let gen = reg.get_version("Platinum").unwrap();
    let trainer = gen.trainer_db().iter().find(|t| t.pkmn.len() == 4 && !t.double_battle).expect("a four-mon trainer").clone();
    // party slot s gets Swords Dance s + 1
    let by_slot: Vec<CustomMoveData> = (0..4).map(|s| setup_for("Swords Dance", s as i64 + 1)).collect();
    for seen in [vec![0, 1, 2, 3], vec![2, 0, 3, 1], vec![1, 3, 0, 2], vec![3, 2, 1, 0]] {
        let mut td = TrainerEventDefinition::new(&trainer.name);
        td.mon_order = xpr_recorder::games::common::full_mon_order(&seen, 4, &[]);
        td.stat_stage_setup = by_slot.clone();
        let mut ev = EventDefinition::with_trainer(td);
        remap_stat_stage_setup(&gen, &mut ev);
        let page = page_setup(&gen, &ev);
        let fought: Vec<i64> = ev.get_pokemon_list(&gen, false).unwrap().iter().map(|(slot, _)| *slot).collect();
        assert_eq!(fought, seen);
        for (i, slot) in seen.iter().enumerate() {
            assert_eq!(page[i], vec![pair("Swords Dance", &(slot + 1).to_string())], "send-out order {:?}, matchup {}", seen, i);
        }
        // and it saves the way it loaded
        let saved = ev.trainer_def.as_ref().unwrap().serialize();
        let reloaded = TrainerEventDefinition::deserialize(Some(&saved)).unwrap().unwrap();
        assert_eq!(reloaded.stat_stage_setup, ev.trainer_def.as_ref().unwrap().stat_stage_setup);
    }
}

#[test]
fn no_setup_stays_no_setup() {
    let reg = registry();
    let gen = reg.get_version("Platinum").unwrap();
    let trainer = gen.trainer_db().iter().find(|t| t.pkmn.len() == 3 && !t.double_battle).unwrap().clone();
    let mut td = TrainerEventDefinition::new(&trainer.name);
    td.stat_stage_setup = vec![CustomMoveData::default(); 3];
    let mut ev = EventDefinition::with_trainer(td);
    remap_stat_stage_setup(&gen, &mut ev);
    assert!(ev.trainer_def.unwrap().stat_stage_setup.is_empty());
}
