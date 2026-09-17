//! Port of `route_recording/game_recorders/gen_three/*` (Emerald, FireRed/LeafGreen).

use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::Value;

use xpr_core::consts;
use xpr_core::io_utils::sanitize_string;
use xpr_data::model::CustomMoveData;
use xpr_data::GenData;
use xpr_engine::{EventDefinition, HoldItemEventDefinition, InventoryEventDefinition, LearnMoveEventDefinition, LevelVal, TrainerEventDefinition, VitaminEventDefinition, WildPkmnEventDefinition};

use super::common::*;
use super::gen2::convert_move_name_gen2plus;
use crate::controller::{dedupe, fix_key, fix_keys, is_set, new_active_flag, ActiveFlag, EventQueue, GameRecorder, GameState, RecorderController};
use crate::gamehook::{GameHookProperty, PropertyStore};
use crate::host::StartInfo;

pub const RETURN_MOVE_NAME: &str = "Return";

pub fn reset_flag() -> String {
    format!("{}FLAG TO SIGNAL GAME RESET. USER SHOULD NEVER SEE THIS", consts::RECORDING_ERROR_FRAGMENT)
}
pub fn trainer_loss_flag() -> String {
    format!("{}FLAG TO SIGNAL LOSING TO TRAINER. USER SHOULD NEVER SEE THIS", consts::RECORDING_ERROR_FRAGMENT)
}
pub fn roar_flag() -> String {
    format!("{}FLAG TO SIGNAL ROARS NEED TO BE HANDLED. USER SHOULD NEVER SEE THIS", consts::RECORDING_ERROR_FRAGMENT)
}
pub fn held_check_flag() -> String {
    format!("{}FLAG TO SIGNAL FOR DEEPER HELD ITEM CHECKING. USER SHOULD NEVER SEE THIS", consts::RECORDING_ERROR_FRAGMENT)
}

#[derive(Clone, Debug)]
pub struct Gen3Keys {
    pub is_frlg: bool,
    pub dma_a: String,
    pub dma_b: String,
    pub dma_c: String,
    pub overworld_map: String,
    pub player_id: String,
    pub player_money: String,
    pub mon_exppoints: String,
    pub mon_level: String,
    pub mon_species: String,
    pub mon_held_item: String,
    pub mon_friendship: String,
    pub team_species: Vec<String>,
    pub team_level: Vec<String>,
    pub team_iv_attack: Vec<String>,
    pub team_iv_defense: Vec<String>,
    pub team_iv_speed: Vec<String>,
    pub team_iv_special_attack: Vec<String>,
    pub team_iv_special_defense: Vec<String>,
    pub player_moves: Vec<String>,
    pub stat_exp: Vec<String>,
    pub gametime_seconds: String,
    pub gametime_frames: String,
    pub battle_flag: String,
    pub trainer_battle_flag: String,
    pub double_battle_flag: String,
    pub two_opponents_battle_flag: Option<String>,
    pub tutorial_battle_flag: Option<String>,
    pub battle_outcome: String,
    pub battle_background_tiles: String,
    pub battle_player_mon_party_pos: String,
    pub battle_player_mon_hp: String,
    pub battle_ally_mon_party_pos: String,
    pub battle_ally_mon_hp: String,
    pub battle_trainer_a_number: String,
    pub battle_trainer_b_number: Option<String>,
    pub battle_first_enemy_species: String,
    pub battle_first_enemy_level: String,
    pub battle_first_enemy_hp: String,
    pub battle_first_enemy_party_pos: String,
    pub battle_second_enemy_species: String,
    pub battle_second_enemy_level: String,
    pub battle_second_enemy_hp: String,
    pub battle_second_enemy_party_pos: String,
    pub enemy_team_species: Vec<String>,
    pub audio_sound_effect_1: String,
    pub audio_sound_effect_2: String,
    pub sstp_tracking: String,
    pub item_type: Vec<String>,
    pub item_quantity: Vec<String>,
    pub ball_type: Vec<String>,
    pub ball_quantity: Vec<String>,
    pub berry_type: Vec<String>,
    pub berry_quantity: Vec<String>,
    pub key_items: Vec<String>,
    pub tmhm_type: Vec<String>,
    pub tmhm_quantity: Vec<String>,
    pub all_item_fields: HashSet<String>,
}

impl Gen3Keys {
    pub fn configure(is_frlg: bool) -> Gen3Keys {
        let team = |suffix: &str| -> Vec<String> { (0..6).map(|i| format!("player.team.{}.{}", i, suffix)).collect() };
        let slots = |pocket: &str, field: &str, n: usize| -> Vec<String> { (0..n).map(|i| format!("player.bag.{}.{}.{}", pocket, i, field)).collect() };
        let (items_n, balls_n, berries_n, tmhm_n) = if is_frlg { (42, 13, 43, 58) } else { (30, 16, 46, 64) };
        let mut k = Gen3Keys {
            is_frlg,
            dma_a: "pointers.dma1".into(),
            dma_b: "pointers.dma2".into(),
            dma_c: "pointers.dma3".into(),
            overworld_map: "overworld.mapName".into(),
            player_id: "player.playerId".into(),
            player_money: "player.bag.money".into(),
            mon_exppoints: "player.team.0.expPoints".into(),
            mon_level: "player.team.0.level".into(),
            mon_species: "player.team.0.species".into(),
            mon_held_item: "player.team.0.itemHeld".into(),
            mon_friendship: "player.team.0.friendship".into(),
            team_species: team("species"),
            team_level: team("level"),
            team_iv_attack: team("ivAttack"),
            team_iv_defense: team("ivDefense"),
            team_iv_speed: team("ivSpeed"),
            team_iv_special_attack: team("ivSpecialAttack"),
            team_iv_special_defense: team("ivSpecialDefense"),
            player_moves: (1..=4).map(|i| format!("player.team.0.move{}", i)).collect(),
            stat_exp: ["evHp", "evAttack", "evDefense", "evSpeed", "evSpecialAttack", "evSpecialDefense"].iter().map(|s| format!("player.team.0.{}", s)).collect(),
            gametime_seconds: "gametime.seconds".into(),
            gametime_frames: "gametime.frames".into(),
            battle_flag: "battle.type.is_battle".into(),
            trainer_battle_flag: "battle.type.trainer".into(),
            double_battle_flag: "battle.type.double".into(),
            two_opponents_battle_flag: if is_frlg { None } else { Some("battle.type.two_opponents".into()) },
            tutorial_battle_flag: Some("battle.type.old_man_tutorial".into()),
            battle_outcome: "battle.outcome".into(),
            battle_background_tiles: "battle.turnInfo.battleBackgroundTiles".into(),
            battle_player_mon_party_pos: "battle.yourPokemon.partyPos".into(),
            battle_player_mon_hp: "battle.yourPokemon.hp".into(),
            battle_ally_mon_party_pos: "battle.yourSecondPokemon.partyPos".into(),
            battle_ally_mon_hp: "battle.yourSecondPokemon.hp".into(),
            battle_trainer_a_number: if is_frlg { "battle.trainer.opponentId".into() } else { "battle.trainer.opponentAId".into() },
            battle_trainer_b_number: if is_frlg { None } else { Some("battle.trainer.opponentBId".into()) },
            battle_first_enemy_species: "battle.enemyPokemon.species".into(),
            battle_first_enemy_level: "battle.enemyPokemon.level".into(),
            battle_first_enemy_hp: "battle.enemyPokemon.hp".into(),
            battle_first_enemy_party_pos: "battle.enemyPokemon.partyPos".into(),
            battle_second_enemy_species: "battle.enemySecondPokemon.species".into(),
            battle_second_enemy_level: "battle.enemySecondPokemon.level".into(),
            battle_second_enemy_hp: "battle.enemySecondPokemon.hp".into(),
            battle_second_enemy_party_pos: "battle.enemySecondPokemon.partyPos".into(),
            enemy_team_species: (0..6).map(|i| format!("battle.trainer.team.{}.species", i)).collect(),
            audio_sound_effect_1: "audio.soundEffect1".into(),
            audio_sound_effect_2: "audio.soundEffect2".into(),
            sstp_tracking: "pointers.sStpTracking".into(),
            item_type: slots("items", "item", items_n),
            item_quantity: slots("items", "quantity", items_n),
            ball_type: slots("pokeBalls", "item", balls_n),
            ball_quantity: slots("pokeBalls", "quantity", balls_n),
            berry_type: slots("berries", "item", berries_n),
            berry_quantity: slots("berries", "quantity", berries_n),
            key_items: slots("keyItems", "item", 30),
            tmhm_type: slots("tmhm", "item", tmhm_n),
            tmhm_quantity: slots("tmhm", "quantity", tmhm_n),
            all_item_fields: HashSet::new(),
        };
        k.rebuild_item_fields();
        k
    }

    fn rebuild_item_fields(&mut self) {
        let mut set = HashSet::new();
        for list in [
            &self.item_type,
            &self.item_quantity,
            &self.ball_type,
            &self.ball_quantity,
            &self.berry_type,
            &self.berry_quantity,
            &self.key_items,
            &self.tmhm_type,
            &self.tmhm_quantity,
        ] {
            set.extend(list.iter().cloned());
        }
        self.all_item_fields = set;
    }

    pub fn all_keys_to_register(&self) -> Vec<String> {
        let mut v = vec![
            self.overworld_map.clone(),
            self.player_id.clone(),
            self.player_money.clone(),
            self.mon_exppoints.clone(),
            self.mon_level.clone(),
            self.mon_species.clone(),
            self.mon_held_item.clone(),
            self.gametime_seconds.clone(),
            self.battle_flag.clone(),
            self.trainer_battle_flag.clone(),
            self.double_battle_flag.clone(),
            self.battle_outcome.clone(),
        ];
        if let Some(t) = &self.tutorial_battle_flag {
            v.push(t.clone());
        }
        v.extend([
            self.battle_background_tiles.clone(),
            self.battle_trainer_a_number.clone(),
            self.battle_player_mon_hp.clone(),
            self.battle_player_mon_party_pos.clone(),
            self.battle_ally_mon_hp.clone(),
            self.battle_ally_mon_party_pos.clone(),
            self.battle_first_enemy_species.clone(),
            self.battle_first_enemy_level.clone(),
            self.battle_first_enemy_hp.clone(),
            self.battle_first_enemy_party_pos.clone(),
            self.battle_second_enemy_species.clone(),
            self.battle_second_enemy_level.clone(),
            self.battle_second_enemy_hp.clone(),
            self.battle_second_enemy_party_pos.clone(),
            self.sstp_tracking.clone(),
        ]);
        // (Python only registers the two-opponent keys when `is_frlg`, where they don't exist)
        v.extend(self.player_moves.iter().cloned());
        v.extend(self.stat_exp.iter().cloned());
        v.extend(self.all_item_fields.iter().cloned());
        v.extend(self.team_species.iter().cloned());
        v.extend([self.dma_a.clone(), self.dma_b.clone(), self.dma_c.clone()]);
        v
    }

    pub fn fix_case(&mut self, store: &PropertyStore) -> Vec<String> {
        let mut invalid = Vec::new();
        for k in [
            &mut self.dma_a,
            &mut self.dma_b,
            &mut self.dma_c,
            &mut self.overworld_map,
            &mut self.player_id,
            &mut self.player_money,
            &mut self.mon_exppoints,
            &mut self.mon_level,
            &mut self.mon_species,
            &mut self.mon_held_item,
            &mut self.mon_friendship,
            &mut self.gametime_seconds,
            &mut self.gametime_frames,
            &mut self.battle_flag,
            &mut self.trainer_battle_flag,
            &mut self.double_battle_flag,
            &mut self.battle_outcome,
            &mut self.battle_background_tiles,
            &mut self.battle_player_mon_party_pos,
            &mut self.battle_player_mon_hp,
            &mut self.battle_ally_mon_party_pos,
            &mut self.battle_ally_mon_hp,
            &mut self.battle_trainer_a_number,
            &mut self.battle_first_enemy_species,
            &mut self.battle_first_enemy_level,
            &mut self.battle_first_enemy_hp,
            &mut self.battle_first_enemy_party_pos,
            &mut self.battle_second_enemy_species,
            &mut self.battle_second_enemy_level,
            &mut self.battle_second_enemy_hp,
            &mut self.battle_second_enemy_party_pos,
            &mut self.audio_sound_effect_1,
            &mut self.audio_sound_effect_2,
            &mut self.sstp_tracking,
        ] {
            fix_key(store, k, &mut invalid);
        }
        crate::controller::fix_opt_key(store, &mut self.two_opponents_battle_flag, &mut invalid);
        crate::controller::fix_opt_key(store, &mut self.tutorial_battle_flag, &mut invalid);
        crate::controller::fix_opt_key(store, &mut self.battle_trainer_b_number, &mut invalid);
        for list in [
            &mut self.team_species,
            &mut self.team_level,
            &mut self.team_iv_attack,
            &mut self.team_iv_defense,
            &mut self.team_iv_speed,
            &mut self.team_iv_special_attack,
            &mut self.team_iv_special_defense,
            &mut self.player_moves,
            &mut self.stat_exp,
            &mut self.enemy_team_species,
            &mut self.item_type,
            &mut self.item_quantity,
            &mut self.ball_type,
            &mut self.ball_quantity,
            &mut self.berry_type,
            &mut self.berry_quantity,
            &mut self.key_items,
            &mut self.tmhm_type,
            &mut self.tmhm_quantity,
        ] {
            fix_keys(store, list, &mut invalid);
        }
        self.rebuild_item_fields();
        dedupe(invalid)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Gen3Converter;

impl Gen3Converter {
    pub fn is_game_vitamin(&self, item_name: &str) -> bool {
        let s = sanitize_string(item_name);
        let vit = ["HP UP", "PROTEIN", "IRON", "CARBOS", "CALCIUM", "ZINC"];
        let berries = ["POMEG BERRY", "KELPSY BERRY", "QUALOT BERRY", "HONDEW BERRY", "GREPA BERRY", "TAMATO BERRY"];
        vit.iter().chain(berries.iter()).any(|v| sanitize_string(v) == s)
    }

    pub fn is_game_rare_candy(&self, item_name: &str) -> bool {
        sanitize_string(item_name) == sanitize_string("RARE CANDY")
    }

    pub fn is_game_tm(&self, item_name: &str) -> bool {
        item_name.starts_with("TM")
    }

    const TUTOR_MOVES: [&'static str; 35] = [
        "Body Slam", "Counter", "Double Edge", "Double-Edge", "Dream Eater", "Explosion", "Mega Kick", "Mega Punch", "Metronome", "Mimic", "Rock Slide",
        "Seismic Toss", "Softboiled", "Substitute", "Swords Dance", "Thunder Wave", "Blast Burn", "Frenzy Plant", "Hydro Cannon", "Dynamicpunch",
        "Dynamic Punch", "Fury Cutter", "Rollout", "Sleep Talk", "Swagger", "Defense Curl", "Snore", "Mud Slap", "Swift", "Icy Wind", "Endure", "Psych Up",
        "Ice Punch", "Thunderpunch", "Thunder Punch",
    ];

    pub fn is_tutor_move(&self, gh_move_name: &str) -> bool {
        let p = name_prettify(gh_move_name);
        Self::TUTOR_MOVES.contains(&p.as_str()) || p == "Fire Punch"
    }

    pub fn get_hm_name(&self, gh_move_name: &str) -> Option<&'static str> {
        match name_prettify(gh_move_name).as_str() {
            "Cut" => Some("HM01 Cut"),
            "Fly" => Some("HM02 Fly"),
            "Surf" => Some("HM03 Surf"),
            "Strength" => Some("HM04 Strength"),
            "Flash" => Some("HM05 Flash"),
            "Rock Smash" => Some("HM06 Rock Smash"),
            "Waterfall" => Some("HM07 Waterfall"),
            "Dive" => Some("HM08 Dive"),
            _ => None,
        }
    }

    pub fn item_name_convert(&self, gh_item_name: Option<&str>) -> Option<String> {
        let name = gh_item_name?;
        if name.starts_with("TM") || name.starts_with("HM") {
            return Some(name.to_string());
        }
        let converted = name_prettify(&name.replace('é', "e"));
        Some(
            match converted.as_str() {
                "Thunderstone" => "Thunder Stone",
                "Hp Up" => "HP Up",
                "Guard Spec." => "Guard Spec",
                "Exp.share" => "Exp Share",
                "S.s.ticket" => "S S Ticket",
                "King's Rock" => "Kings Rock",
                "Silverpowder" => "SilverPowder",
                "Twistedspoon" => "TwistedSpoon",
                "Blackbelt" => "Black Belt",
                "Blackglasses" => "BlackGlasses",
                "Up-grade" => "Up Grade",
                "Paralyze Heal" => "Parlyz Heal",
                other => other,
            }
            .to_string(),
        )
    }

    pub fn move_name_convert(&self, gh_move_name: Option<&str>) -> Option<String> {
        let name = gh_move_name?;
        Some(convert_move_name_gen2plus(&name_prettify(&name.replace('-', " "))))
    }

    pub fn pkmn_name_convert(&self, gh_pkmn_name: Option<&str>) -> Option<String> {
        let name = gh_pkmn_name?;
        Some(
            match name {
                "Mr. Mime" => "MrMime",
                "Farfetch'd" => "FarfetchD",
                "Ho-oh" => "HoOh",
                other => other,
            }
            .to_string(),
        )
    }
}

// ---------------------------------------------------------------------------
// machine
// ---------------------------------------------------------------------------

#[derive(Default)]
struct UninitData {
    is_waiting: bool,
    seconds_delay: i64,
}

#[derive(Default)]
struct ResettingData {
    is_waiting: bool,
    seconds_delay: i64,
}

struct BattleData {
    is_trainer_battle: bool,
    trainer_name: Value,
    second_trainer_name: Value,
    enemy_pos_lookup: HashMap<i64, i64>,
    defeated_trainer_mons: Vec<EventDefinition>,
    delayed_move_updater: DelayedUpdate,
    delayed_item_updater: DelayedUpdate,
    delayed_held_item_updater: DelayedUpdate,
    delayed_levelup: DelayedUpdate,
    delayed_initialization: DelayedUpdate,
    loss_detected: bool,
    cached_first_mon_species: String,
    cached_first_mon_level: i64,
    cached_second_mon_species: String,
    cached_second_mon_level: i64,
    exp_split: Vec<HashSet<i64>>,
    enemy_mon_order: Vec<i64>,
    /// Enemy party positions the solo mon stole the held item from
    /// (Thief / Covet); the trainer event's `thief_mons`.
    thief_mons: Vec<i64>,
    friendship_data: Vec<i64>,
    battle_started: bool,
    is_double_battle: bool,
    is_tutorial_battle: bool,
    initial_money: i64,
    init_held_item: Value,
    original_level: i64,
}

impl Default for BattleData {
    fn default() -> Self {
        BattleData {
            is_trainer_battle: false,
            trainer_name: Value::String(String::new()),
            second_trainer_name: Value::String(String::new()),
            enemy_pos_lookup: HashMap::new(),
            defeated_trainer_mons: Vec::new(),
            delayed_move_updater: DelayedUpdate::new(BATTLE_DELAY),
            delayed_item_updater: DelayedUpdate::new(BATTLE_DELAY),
            delayed_held_item_updater: DelayedUpdate::new(BATTLE_DELAY),
            delayed_levelup: DelayedUpdate::new(BATTLE_DELAY),
            delayed_initialization: DelayedUpdate::new(BATTLE_DELAY),
            loss_detected: false,
            cached_first_mon_species: String::new(),
            cached_first_mon_level: 0,
            cached_second_mon_species: String::new(),
            cached_second_mon_level: 0,
            exp_split: Vec::new(),
            enemy_mon_order: Vec::new(),
            thief_mons: Vec::new(),
            friendship_data: Vec::new(),
            battle_started: false,
            is_double_battle: false,
            is_tutorial_battle: false,
            initial_money: 0,
            init_held_item: Value::Null,
            original_level: 0,
        }
    }
}

#[derive(Default)]
struct InventoryData {
    seconds_delay: i64,
    money_gained: bool,
    money_lost: bool,
    held_item_changed: bool,
    external_held_item_flag: bool,
}

#[derive(Default)]
struct CandyData {
    move_learned: bool,
    cur_delay: i64,
}

#[derive(Default)]
struct TmData {
    seconds_delay: i64,
}

#[derive(Default)]
struct MoveDeleteData {
    cur_delay: i64,
}

#[derive(Default)]
struct VitaminData {
    item_removal_detected: bool,
    cur_delay: i64,
    error_delay: i64,
}

#[derive(Default)]
struct OverworldData {
    waiting_for_registration: bool,
    register_delay: i64,
    propagate_held_item_flag: bool,
    validation_delay: i64,
    waiting_for_new_file: bool,
    new_file_delay: i64,
    wrong_mon_delay: i64,
    waiting_for_solo_mon_in_slot_1: bool,
    wrong_mon_in_slot_1: bool,
}

const BASE_DELAY: i64 = 2;
const BATTLE_DELAY: i64 = 3;
const ERROR_DELAY: i64 = 5;

pub struct Gen3Machine {
    controller: Arc<RecorderController>,
    gen: Arc<GenData>,
    keys: Gen3Keys,
    conv: Gen3Converter,
    debug_mode: bool,
    is_frlg: bool,

    player_id: Option<Value>,
    valid_solo_mon: bool,
    cached_team: Vec<MonKey>,
    level_up_moves: HashMap<(String, i64), Vec<String>>,
    cached_items: ItemCache,
    cached_moves: [Option<String>; 4],
    cached_money: i64,
    solo_mon_key: MonKey,

    cur_state: GameState,
    active: ActiveFlag,
    queue: Arc<EventQueue>,
    /// the live value of the tutorial-battle flag, for the processing thread
    tutorial_flag_now: Arc<std::sync::atomic::AtomicBool>,

    uninit: UninitData,
    resetting: ResettingData,
    battle: BattleData,
    inventory: InventoryData,
    candy: CandyData,
    tm: TmData,
    move_delete: MoveDeleteData,
    vitamin: VitaminData,
    overworld: OverworldData,
}

fn mon_key_from(dvs: Option<&xpr_data::model::StatBlock>, species: Option<String>) -> MonKey {
    let v = |x: Option<i64>| x.map(Value::from).unwrap_or(Value::Null);
    MonKey {
        species,
        attack: v(dvs.map(|d| d.attack)),
        defense: v(dvs.map(|d| d.defense)),
        speed: v(dvs.map(|d| d.speed)),
        special_attack: v(dvs.map(|d| d.special_attack)),
        special_defense: v(dvs.map(|d| d.special_defense)),
        level: Value::Null,
    }
}

impl Gen3Machine {
    pub fn new(controller: Arc<RecorderController>, info: &StartInfo, is_frlg: bool) -> Gen3Machine {
        Gen3Machine {
            controller,
            gen: info.gen.clone(),
            keys: Gen3Keys::configure(is_frlg),
            conv: Gen3Converter,
            debug_mode: info.debug_mode,
            is_frlg,
            player_id: None,
            valid_solo_mon: false,
            cached_team: Vec::new(),
            level_up_moves: HashMap::new(),
            cached_items: IndexMap::new(),
            cached_moves: [None, None, None, None],
            cached_money: 0,
            solo_mon_key: mon_key_from(info.dvs.as_ref(), info.solo_species.clone()),
            cur_state: GameState::Uninitialized,
            active: new_active_flag(),
            queue: EventQueue::new(),
            tutorial_flag_now: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            uninit: UninitData::default(),
            resetting: ResettingData::default(),
            battle: BattleData::default(),
            inventory: InventoryData::default(),
            candy: CandyData::default(),
            tm: TmData::default(),
            move_delete: MoveDeleteData::default(),
            vitamin: VitaminData::default(),
            overworld: OverworldData::default(),
        }
    }

    fn route_defined_mon_key(&self) -> MonKey {
        let dvs = self.controller.host().call(|h| h.get_dvs());
        let species = self.controller.host().call(|h| h.final_solo_species());
        mon_key_from(dvs.as_ref(), species)
    }

    fn mon_key(&self, store: &PropertyStore, mon_idx: usize) -> MonKey {
        MonKey {
            species: self.conv.pkmn_name_convert(store.str_of(&self.keys.team_species[mon_idx]).as_deref()),
            attack: store.value(&self.keys.team_iv_attack[mon_idx]),
            defense: store.value(&self.keys.team_iv_defense[mon_idx]),
            speed: store.value(&self.keys.team_iv_speed[mon_idx]),
            special_attack: store.value(&self.keys.team_iv_special_attack[mon_idx]),
            special_defense: store.value(&self.keys.team_iv_special_defense[mon_idx]),
            level: store.value(&self.keys.team_level[mon_idx]),
        }
    }

    fn queue_new_event(&self, event: EventDefinition) {
        self.queue.push(event);
    }

    fn load_level_up_moves(&mut self) {
        let Some(species) = self.solo_mon_key.species.clone() else {
            log::info!("Couldn't load level up moves from invalid mon: None");
            return;
        };
        let Some(new_mon) = self.gen.pkmn_db().get_pkmn(&species) else {
            log::info!("Couldn't load level up moves from invalid mon: {}", species);
            return;
        };
        for (move_level, move_name) in &new_mon.levelup_moves {
            self.level_up_moves.insert((new_mon.name.clone(), *move_level), vec![move_name.clone()]);
        }
    }

    fn can_evolve_into(&self, species: Option<&str>) -> bool {
        let Some(s) = species else { return false };
        let s = s.to_string();
        self.controller.host().call(move |h| h.can_evolve_into(&s))
    }

    fn update_team_cache(&mut self, store: &PropertyStore, generate_events: bool, regenerate_move_cache: bool) {
        if !generate_events {
            self.valid_solo_mon = false;
            self.solo_mon_key = self.route_defined_mon_key();
            self.cached_team = Vec::new();
        }
        let new_cache: Vec<MonKey> = (0..6).map(|i| self.mon_key(store, i)).filter(|x| x.species.as_deref().map(|s| !s.is_empty()).unwrap_or(false)).collect();
        let diff = team_diff(&self.cached_team, &new_cache);
        if !diff.lost.is_empty() || !diff.gained.is_empty() {
            log::info!("Team change detected");
            log::info!("Losing mons: {}", repr_mons(&diff.lost));
            log::info!("Gaining mons: {}", repr_mons(&diff.gained));
            log::info!("New team: {}", repr_team(&new_cache));
        }
        let solo_lost = diff.lost.contains_key(&MonKeyId::of(&self.solo_mon_key));
        if self.valid_solo_mon && solo_lost {
            let mut found_evolutions: Vec<MonKey> = Vec::new();
            for (test_mon, _) in diff.gained.values() {
                if test_mon.same_dvs(&self.solo_mon_key) && self.can_evolve_into(test_mon.species.as_deref()) {
                    found_evolutions.push(test_mon.clone());
                }
            }
            if !found_evolutions.is_empty() {
                if found_evolutions.len() > 1 {
                    let err_msg = format!("Found multiple new valid solo mons, just taking the first one from the list: {}", repr_team(&found_evolutions));
                    log::error!("{}", err_msg);
                    self.queue_new_event(EventDefinition::notes_only(&format!("{}{}", consts::RECORDING_ERROR_FRAGMENT, err_msg)));
                }
                self.trigger_evolution(found_evolutions[0].clone());
            } else {
                self.valid_solo_mon = false;
            }
        } else if !self.valid_solo_mon && !new_cache.is_empty() {
            log::info!("looking for solo mon species: {}", self.solo_mon_key.species.as_deref().unwrap_or("None"));
            if self.cached_team.is_empty() {
                log::info!("Creating mons from empty cache. Prioritizing slot 0: {}", new_cache[0].repr());
                let is_backport = new_cache[0].species.as_deref().map(|s| s.to_lowercase().contains(consts::BACKPORT_SPECIES_CHECK)).unwrap_or(false);
                log::info!("Is mon backport? {} ", if is_backport { "True" } else { "False" });
                if self.solo_mon_key.species == new_cache[0].species || is_backport {
                    if !self.solo_mon_key.same_key(&new_cache[0]) {
                        log::error!("Expected DV spread: {}, but found solo mon with this DV spread: {}", self.solo_mon_key.repr(), new_cache[0].repr());
                    }
                    self.valid_solo_mon = true;
                    self.solo_mon_key = new_cache[0].clone();
                    self.load_level_up_moves();
                }
            }
            if !self.valid_solo_mon {
                let mut found_matches: Vec<MonKey> = Vec::new();
                let mut found_evolutions: Vec<MonKey> = Vec::new();
                for (test_mon, _) in diff.gained.values() {
                    if test_mon.same_dvs(&self.solo_mon_key) {
                        let cur_mon_obj = test_mon.species.as_deref().and_then(|s| self.gen.pkmn_db().get_pkmn(s));
                        if cur_mon_obj.map(|m| Some(m.name.as_str()) == self.solo_mon_key.species.as_deref()).unwrap_or(false) {
                            found_matches.push(test_mon.clone());
                        } else if self.can_evolve_into(test_mon.species.as_deref()) {
                            found_evolutions.push(test_mon.clone());
                        }
                    }
                }
                if !found_matches.is_empty() {
                    if found_matches.len() > 1 {
                        let err_msg = format!("Found multiple new valid solo mons, just taking the first one from the list: {}", repr_team(&found_matches));
                        log::error!("{}", err_msg);
                        self.queue_new_event(EventDefinition::notes_only(&format!("{}{}", consts::RECORDING_ERROR_FRAGMENT, err_msg)));
                    }
                    self.valid_solo_mon = true;
                    self.load_level_up_moves();
                    self.solo_mon_key = found_matches[0].clone();
                } else if !found_evolutions.is_empty() {
                    if found_evolutions.len() > 1 {
                        let err_msg = format!("Found multiple new valid solo mons, just taking the first one from the list: {}", repr_team(&found_evolutions));
                        log::error!("{}", err_msg);
                        self.queue_new_event(EventDefinition::notes_only(&format!("{}{}", consts::RECORDING_ERROR_FRAGMENT, err_msg)));
                    }
                    self.valid_solo_mon = true;
                    self.solo_mon_key = found_evolutions[0].clone();
                    self.trigger_evolution(found_evolutions[0].clone());
                }
            }
        }
        if regenerate_move_cache {
            if !self.valid_solo_mon {
                log::error!("skipping regeneration of move cache, as no valid solo mon is present");
            } else {
                self.move_cache_update(store, false, None, false, false, false);
            }
        }
        log::info!("after team cache update, valid_solo_mon: {}", self.valid_solo_mon);
        self.cached_team = new_cache;
    }

    fn update_all_cached_info(&mut self, store: &PropertyStore) {
        self.update_team_cache(store, false, true);
        self.item_cache_update(store, false, false, false, false, false, false, false);
        self.money_cache_update(store);
        let area = py_str(&store.value(&self.keys.overworld_map));
        self.controller.entered_new_area(&area);
    }

    fn solo_mon_levelup(&mut self, new_level: i64) {
        let species = self.solo_mon_key.species.clone().unwrap_or_default();
        log::info!("levelup detected. {} leveling up to {}", species, new_level);
        let moves = self.level_up_moves.get(&(species.clone(), new_level)).cloned().unwrap_or_default();
        for move_name in moves {
            log::info!("queueing up ignore event of move: {}", move_name);
            self.queue_new_event(EventDefinition::with_learn_move(LearnMoveEventDefinition::new(
                Some(&move_name),
                None,
                consts::MOVE_SOURCE_LEVELUP,
                LevelVal::Int(new_level),
                Some(&species),
                false,
            )));
        }
    }

    fn trigger_evolution(&mut self, new_mon_key: MonKey) {
        log::info!("Evolving into: {}", new_mon_key.species.as_deref().unwrap_or("None"));
        self.solo_mon_key = new_mon_key.clone();
        self.load_level_up_moves();
        self.queue_new_event(EventDefinition::with_evolution(new_mon_key.species.as_deref().unwrap_or("None")));
        self.solo_mon_levelup(new_mon_key.level_i64());
    }

    fn money_cache_update(&mut self, store: &PropertyStore) -> Option<bool> {
        let new_cache = store_i64(store, &self.keys.player_money);
        if new_cache == self.cached_money {
            return None;
        }
        let result = new_cache > self.cached_money;
        self.cached_money = new_cache;
        Some(result)
    }

    fn move_cache_update(&mut self, store: &PropertyStore, generate_events: bool, tm_name: Option<&str>, hm_expected: bool, tutor_expected: bool, levelup_source: bool) {
        let new_cache: [Option<String>; 4] = std::array::from_fn(|i| self.conv.move_name_convert(store.str_of(&self.keys.player_moves[i]).as_deref()));
        if generate_events {
            let old_moves: Vec<&String> = self.cached_moves.iter().flatten().collect();
            let new_moves: Vec<&String> = new_cache.iter().flatten().collect();
            let deleted: Vec<String> = old_moves.iter().filter(|m| !new_moves.contains(m)).map(|m| (*m).clone()).collect();
            let learned: Vec<String> = new_moves.iter().filter(|m| !old_moves.contains(m)).map(|m| (*m).clone()).collect();
            if deleted.len() > 1 {
                log::error!("Got multiple deleted moves..? {:?}, from {:?} to {:?}", deleted, self.cached_moves, new_cache);
            }
            if learned.len() > 1 {
                log::error!("Got multiple learned moves..? {:?}, from {:?} to {:?}", learned, self.cached_moves, new_cache);
            }
            let to_delete_move = deleted.first().cloned();
            let to_learn_move = learned.first().cloned();
            if to_learn_move.is_none() && to_delete_move.is_some() {
                let mut lm = LearnMoveEventDefinition::new(None, None, consts::MOVE_SOURCE_TUTOR, LevelVal::any(), None, false);
                lm.destination_name = to_delete_move.and_then(|d| self.conv.move_name_convert(Some(&d)));
                self.queue_new_event(EventDefinition::with_learn_move(lm));
            } else if let Some(learn) = to_learn_move {
                let (source, level, mon) = if levelup_source {
                    (consts::MOVE_SOURCE_LEVELUP.to_string(), LevelVal::Int(store_i64(store, &self.keys.mon_level)), self.solo_mon_key.species.clone())
                } else if tutor_expected {
                    (consts::MOVE_SOURCE_TUTOR.to_string(), LevelVal::any(), None)
                } else if hm_expected {
                    (self.conv.get_hm_name(&learn).map(|s| s.to_string()).unwrap_or_else(|| "None".into()), LevelVal::any(), None)
                } else {
                    (tm_name.map(|s| s.to_string()).unwrap_or_else(|| "None".into()), LevelVal::any(), None)
                };
                let mut lm = LearnMoveEventDefinition::new(self.conv.move_name_convert(Some(&learn)).as_deref(), None, &source, level, mon.as_deref(), false);
                lm.destination_name = to_delete_move.and_then(|d| self.conv.move_name_convert(Some(&d)));
                self.queue_new_event(EventDefinition::with_learn_move(lm));
            }
        }
        self.cached_moves = new_cache;
    }

    fn item_cache(&self, store: &PropertyStore) -> ItemCache {
        let mut result: ItemCache = IndexMap::new();
        fn load(result: &mut ItemCache, store: &PropertyStore, types: &[String], quantities: &[String]) {
            for (t, q) in types.iter().zip(quantities.iter()) {
                result.insert(store.value(t), store_i64(store, q));
            }
        }
        load(&mut result, store, &self.keys.item_type, &self.keys.item_quantity);
        load(&mut result, store, &self.keys.ball_type, &self.keys.ball_quantity);
        load(&mut result, store, &self.keys.berry_type, &self.keys.berry_quantity);
        for k in &self.keys.key_items {
            result.insert(store.value(k), 1);
        }
        load(&mut result, store, &self.keys.tmhm_type, &self.keys.tmhm_quantity);
        result
    }

    /// Returns whether the "expected" event (vitamin/candy/TM/hold) was generated.
    #[allow(clippy::too_many_arguments)]
    fn item_cache_update(&mut self, store: &PropertyStore, generate_events: bool, purchase_expected: bool, sale_expected: bool, vitamin_flag: bool, candy_flag: bool, tm_flag: bool, held_item_changed: bool) -> bool {
        let new_cache = self.item_cache(store);
        let old_cache = std::mem::replace(&mut self.cached_items, new_cache.clone());
        if !generate_events {
            return false;
        }
        let mut expected_event_generated = false;
        let (gained_items, lost_items) = item_diff(&old_cache, &new_cache);
        if !gained_items.is_empty() && sale_expected {
            log::error!("Gained the following items when expecting to be losing items to selling... {}", repr_items(&gained_items));
        }
        if !held_item_changed {
            for (cur_gained_item, cur_gain_num) in &gained_items {
                let app_item_name = self.conv.item_name_convert(cur_gained_item.as_str()).unwrap_or_else(|| "None".into());
                log::info!("trying to gain item: {}, converted from {}", app_item_name, py_str(cur_gained_item));
                if *cur_gain_num > 100 {
                    log::error!("{}", "#".repeat(50));
                    log::error!("Ignoring attempt to gain the above item, due to excessive number. Assuming this is happening due to DMA issues");
                    continue;
                }
                self.queue_new_event(EventDefinition::with_item(InventoryEventDefinition::new(&app_item_name, *cur_gain_num, true, purchase_expected, None)));
            }
        }
        // taking the held item off: the bag gains it back and the mon holds
        // nothing; the engine's "hold nothing" event returns the held item
        // to the bag, so nothing else is recorded for the gain
        if held_item_changed && lost_items.is_empty() && !gained_items.is_empty() && store.value(&self.keys.mon_held_item).is_null() {
            log::info!("held item taken off: {}", repr_items(&gained_items));
            self.queue_new_event(EventDefinition::with_hold_item(HoldItemEventDefinition::new(None, false)));
            expected_event_generated = true;
        }
        if !lost_items.is_empty() && purchase_expected {
            log::error!("Lost the following items when expecting to be gain items to purchasing... {}", repr_items(&lost_items));
        }
        if lost_items.len() > 1 && held_item_changed {
            log::error!("Lost multiple items when trying to change the held item... {}", repr_items(&lost_items));
        }
        for (cur_lost_item, cur_lost_num) in &lost_items {
            let raw = cur_lost_item.as_str().unwrap_or("");
            let app_item_name = self.conv.item_name_convert(cur_lost_item.as_str()).unwrap_or_else(|| "None".into());
            log::info!("trying to lose item: {}, converted from {}", app_item_name, py_str(cur_lost_item));
            if *cur_lost_num > 100 {
                log::error!("{}", "#".repeat(50));
                log::error!("Ignoring attempt to gain the above item, due to excessive number. Assuming this is happening due to DMA issues");
                continue;
            }
            if vitamin_flag && self.conv.is_game_vitamin(raw) {
                if sale_expected {
                    log::error!("Expected sale, but looks like vitamins were used too???");
                }
                self.queue_new_event(EventDefinition::with_vitamin(VitaminEventDefinition::new(&app_item_name, *cur_lost_num)));
                expected_event_generated = true;
            } else if candy_flag && self.conv.is_game_rare_candy(raw) {
                if sale_expected {
                    log::error!("Expected sale, but looks like rare candy was used too???");
                }
                self.queue_new_event(EventDefinition::with_rare_candy(*cur_lost_num));
                expected_event_generated = true;
            } else if tm_flag && self.conv.is_game_tm(raw) {
                if sale_expected {
                    log::error!("Expected sale, but looks like TM was used too???");
                }
                self.move_cache_update(store, true, Some(&app_item_name), false, false, false);
                expected_event_generated = true;
            } else if held_item_changed {
                if *cur_lost_num > 1 {
                    log::error!("Expected to lose multiple items while telling mon to hold item: {} x{}", app_item_name, cur_lost_num);
                }
                self.queue_new_event(EventDefinition::with_hold_item(HoldItemEventDefinition::new(Some(&app_item_name), false)));
                expected_event_generated = true;
            } else {
                self.queue_new_event(EventDefinition::with_item(InventoryEventDefinition::new(&app_item_name, *cur_lost_num, false, sale_expected, None)));
            }
        }
        expected_event_generated
    }

    // ---- battle helpers ---------------------------------------------------------------

    fn has_return(&self) -> bool {
        self.cached_moves.iter().flatten().any(|m| m == RETURN_MOVE_NAME)
    }

    fn return_custom_data(friendship_values: &[i64]) -> Vec<CustomMoveData> {
        friendship_values
            .iter()
            .map(|f| {
                let mut c = CustomMoveData::default();
                c.player.insert(RETURN_MOVE_NAME.to_string(), (((*f as f64) / 2.5).trunc() as i64).to_string());
                c
            })
            .collect()
    }

    fn num_enemy_trainer_pokemon(&self, store: &PropertyStore) -> usize {
        self.keys.enemy_team_species.iter().filter(|k| store.truthy(k)).count()
    }

    fn enemy_pos_lookup(&self, store: &PropertyStore) -> HashMap<i64, i64> {
        if !xpr_core::pyjson::truthy(&self.battle.second_trainer_name) {
            return (0..6).map(|x| (x, x)).collect();
        }
        let real_order = [0i64, 3, 1, 4, 2, 5];
        let mut result = HashMap::new();
        let mut next_pos_idx = 0;
        for cur_key_idx in real_order {
            if store.truthy(&self.keys.enemy_team_species[cur_key_idx as usize]) {
                result.insert(cur_key_idx, next_pos_idx);
                next_pos_idx += 1;
            }
        }
        result
    }

    fn first_enemy_mon_pos(&self, store: &PropertyStore, value: Option<i64>) -> Option<i64> {
        let v = value.unwrap_or_else(|| store_i64(store, &self.keys.battle_first_enemy_party_pos));
        self.battle.enemy_pos_lookup.get(&v).copied()
    }

    fn second_enemy_mon_pos(&self, store: &PropertyStore, value: Option<i64>) -> Option<i64> {
        let v = value.unwrap_or_else(|| store_i64(store, &self.keys.battle_second_enemy_party_pos));
        self.battle.enemy_pos_lookup.get(&v).copied()
    }

    fn split_at(&mut self, pos: Option<i64>) -> Option<&mut HashSet<i64>> {
        let p = pos?;
        usize::try_from(p).ok().and_then(move |i| self.battle.exp_split.get_mut(i))
    }

    /// `_battle_ready`
    fn battle_ready(&mut self, store: &PropertyStore) {
        if !store.truthy(&self.keys.battle_flag) {
            self.battle.delayed_initialization.reset();
            return;
        }
        self.battle.battle_started = true;
        self.battle.is_double_battle = store.truthy(&self.keys.double_battle_flag);
        self.battle.is_tutorial_battle = self.keys.tutorial_battle_flag.as_ref().map(|k| store.truthy(k)).unwrap_or(false);
        self.battle.is_trainer_battle = store.truthy(&self.keys.trainer_battle_flag);
        self.battle.original_level = store_i64(store, &self.keys.mon_level);
        if self.battle.is_tutorial_battle {
            log::info!("tutorial fight found");
        } else if self.battle.is_trainer_battle {
            log::info!("trainer battle found");
            self.battle.trainer_name = store.value(&self.keys.battle_trainer_a_number);
            if self.is_frlg {
                self.battle.second_trainer_name = Value::String(String::new());
            } else {
                let b = self.keys.battle_trainer_b_number.as_ref().map(|k| store.value(k)).unwrap_or(Value::Null);
                let two = self.keys.two_opponents_battle_flag.as_ref().map(|k| store.truthy(k)).unwrap_or(false);
                self.battle.second_trainer_name = if b.is_null() || !two { Value::String(String::new()) } else { b };
            }
            let num_enemy_pokemon = self.num_enemy_trainer_pokemon(store);
            self.battle.enemy_pos_lookup = self.enemy_pos_lookup(store);
            if self.battle.is_double_battle {
                let ally_mon_pos = store_i64(store, &self.keys.battle_ally_mon_party_pos);
                self.battle.exp_split = (0..num_enemy_pokemon).map(|_| HashSet::from([0, ally_mon_pos])).collect();
                self.battle.enemy_mon_order = vec![0, 1];
            } else {
                self.battle.exp_split = (0..num_enemy_pokemon).map(|_| HashSet::from([0])).collect();
                self.battle.enemy_mon_order = vec![0];
            }
            let mut td = TrainerEventDefinition::new(&py_str(&self.battle.trainer_name));
            td.second_trainer_name = self.battle.second_trainer_name.clone();
            if self.has_return() {
                let cur_friendship = store_i64(store, &self.keys.mon_friendship);
                td.custom_move_data = Self::return_custom_data(&[cur_friendship; 6]);
            }
            td.exp_split = self.battle.exp_split.iter().map(|x| x.len() as i64).collect();
            self.queue_new_event(EventDefinition::with_trainer(td));
        } else {
            log::info!("wild battle found");
        }
    }

    fn delayed_moves_update(&mut self, store: &PropertyStore) {
        if store_i64(store, &self.keys.battle_player_mon_party_pos) == 0 {
            self.update_team_cache(store, true, false);
            self.move_cache_update(store, true, None, false, false, true);
        }
    }

    fn delayed_held_item_update(&mut self, store: &PropertyStore) {
        if store_i64(store, &self.keys.battle_player_mon_party_pos) == 0 {
            let held = store.value(&self.keys.mon_held_item);
            if held.is_null() {
                self.queue_new_event(EventDefinition::with_hold_item(HoldItemEventDefinition::new(None, true)));
            } else if held != self.battle.init_held_item {
                let stolen = self.battle.init_held_item.is_null();
                self.battle.init_held_item = held.clone();
                if stolen && self.battle.is_trainer_battle {
                    // Thief / Covet: the item never touches the bag, so the
                    // trainer event carries the steal (see `thief_mons`)
                    match self.first_enemy_mon_pos(store, None) {
                        Some(pos) if !self.battle.thief_mons.contains(&pos) => {
                            log::info!("stole {} from the enemy mon at party position {}", py_str(&held), pos);
                            self.battle.thief_mons.push(pos);
                        }
                        Some(_) => {}
                        None => log::warn!("stole {} but could not tell which enemy mon held it", py_str(&held)),
                    }
                } else if stolen {
                    // a wild mon's item: it was never in the bag, so acquire it first
                    let app_item_name = self.conv.item_name_convert(held.as_str()).unwrap_or_else(|| py_str(&held));
                    self.queue_new_event(EventDefinition::with_item(InventoryEventDefinition::new(&app_item_name, 1, true, false, None)));
                    self.queue_new_event(EventDefinition::with_hold_item(HoldItemEventDefinition::new(Some(&app_item_name), false)));
                } else {
                    self.queue_new_event(EventDefinition::with_hold_item(HoldItemEventDefinition::new(held.as_str(), true)));
                }
            }
        }
    }

    fn delayed_level_update(&mut self, store: &PropertyStore) {
        if store_i64(store, &self.keys.battle_player_mon_party_pos) == 0 {
            let new_level = store_i64(store, &self.keys.mon_level);
            if new_level > self.battle.original_level {
                self.solo_mon_levelup(new_level);
                self.battle.original_level = new_level;
            }
        }
    }

    // ---- states -------------------------------------------------------------------

    fn watch_for_reset(&self, store: &PropertyStore, new: &GameHookProperty) -> Option<GameState> {
        if self.player_id.is_some() {
            if new.path == self.keys.player_id && new.eq_i64(0) && store.get(&self.keys.dma_a).map(|p| p.eq_i64(0)).unwrap_or(false) {
                return Some(GameState::Resetting);
            } else if new.path == self.keys.dma_a && new.eq_i64(0) && store.get(&self.keys.player_id).map(|p| p.eq_i64(0)).unwrap_or(false) {
                return Some(GameState::Resetting);
            }
        }
        None
    }

    fn in_battle_now(&self, store: &PropertyStore) -> bool {
        store.value(&self.keys.battle_outcome).is_null() && store.truthy(&self.keys.battle_flag)
    }

    fn on_enter(&mut self, state: GameState, store: &PropertyStore) {
        match state {
            GameState::Uninitialized => {
                self.uninit.is_waiting = false;
                self.uninit.seconds_delay = 2;
            }
            GameState::Resetting => {
                self.resetting.is_waiting = false;
                self.resetting.seconds_delay = 2;
                self.queue_new_event(EventDefinition::notes_only(&reset_flag()));
            }
            GameState::Battle => {
                let money = store_i64(store, &self.keys.player_money);
                let held_at_start = store.value(&self.keys.mon_held_item);
                let b = &mut self.battle;
                b.defeated_trainer_mons.clear();
                b.delayed_move_updater.reset();
                b.delayed_item_updater.reset();
                b.delayed_held_item_updater.reset();
                b.delayed_levelup.reset();
                b.delayed_initialization.reset();
                b.is_trainer_battle = false;
                b.loss_detected = false;
                b.cached_first_mon_species.clear();
                b.cached_first_mon_level = 0;
                b.cached_second_mon_species.clear();
                b.cached_second_mon_level = 0;
                b.trainer_name = Value::String(String::new());
                b.second_trainer_name = Value::String(String::new());
                b.enemy_pos_lookup.clear();
                b.exp_split.clear();
                b.enemy_mon_order.clear();
                b.thief_mons.clear();
                b.friendship_data.clear();
                b.battle_started = false;
                b.is_double_battle = false;
                b.is_tutorial_battle = false;
                b.initial_money = money;
                // the held item as the battle starts (not as the delayed
                // initialisation fires): a Thief on the first turn must still
                // read as a change
                b.init_held_item = held_at_start;
                b.delayed_initialization.begin_waiting(true);
            }
            GameState::InventoryChange => {
                self.inventory.seconds_delay = BASE_DELAY;
                self.inventory.money_gained = self.money_cache_update(store).unwrap_or(false);
                self.inventory.money_lost = false;
                self.inventory.held_item_changed = self.inventory.external_held_item_flag;
                self.inventory.external_held_item_flag = false;
            }
            GameState::RareCandy => {
                self.candy.move_learned = false;
                self.candy.cur_delay = BASE_DELAY;
            }
            GameState::Tm => self.tm.seconds_delay = BASE_DELAY,
            GameState::MoveDelete => self.move_delete.cur_delay = BASE_DELAY,
            GameState::Vitamin => {
                self.vitamin.item_removal_detected = false;
                self.vitamin.cur_delay = BASE_DELAY;
                self.vitamin.error_delay = ERROR_DELAY;
            }
            GameState::Overworld => {
                self.money_cache_update(store);
                self.update_team_cache(store, true, false);
                let species = store.value(&self.keys.mon_species);
                let o = &mut self.overworld;
                o.waiting_for_registration = false;
                o.register_delay = BASE_DELAY;
                o.waiting_for_new_file = false;
                o.new_file_delay = BASE_DELAY;
                o.wrong_mon_delay = BASE_DELAY;
                if species.is_null() {
                    o.waiting_for_solo_mon_in_slot_1 = true;
                    o.wrong_mon_in_slot_1 = false;
                } else if species.as_str().map(|s| s.to_string()) != self.solo_mon_key.species {
                    o.waiting_for_solo_mon_in_slot_1 = false;
                    o.wrong_mon_in_slot_1 = true;
                } else {
                    o.waiting_for_solo_mon_in_slot_1 = false;
                    o.wrong_mon_in_slot_1 = false;
                }
                o.validation_delay = 5;
            }
        }
    }

    fn on_exit(&mut self, state: GameState, next: GameState, store: &PropertyStore) {
        match state {
            GameState::Uninitialized => {
                let id = store.value(&self.keys.player_id);
                self.player_id = Some(id);
                if self.player_id.as_ref().and_then(crate::gamehook::value_as_i64) == Some(0) {
                    self.player_id = None;
                }
                self.update_all_cached_info(store);
            }
            GameState::Resetting => {
                let new_player_id = store.value(&self.keys.player_id);
                match &self.player_id {
                    None => self.player_id = Some(new_player_id),
                    Some(id) if *id != new_player_id => self.controller.route_restarted(),
                    _ => {}
                }
                self.update_all_cached_info(store);
            }
            GameState::Battle => {
                if next != GameState::Resetting {
                    // a steal on the final turn must reach the trainer
                    // event, so the held-item check runs before it is built
                    if self.battle.delayed_held_item_updater.trigger(false) {
                        self.delayed_held_item_update(store);
                    }
                    if self.battle.is_trainer_battle {
                        let mut final_exp_split: Vec<i64> = self.battle.exp_split.iter().map(|x| x.len() as i64).collect();
                        if !final_exp_split.iter().any(|x| *x > 1) {
                            final_exp_split = Vec::new();
                        }
                        let mut sorted = self.battle.enemy_mon_order.clone();
                        sorted.sort();
                        let final_mon_order: Vec<i64> = sorted.iter().map(|x| self.battle.enemy_mon_order.iter().position(|y| y == x).unwrap_or(0) as i64 + 1).collect();
                        let mut td = TrainerEventDefinition::new(&py_str(&self.battle.trainer_name));
                        td.second_trainer_name = self.battle.second_trainer_name.clone();
                        td.exp_split = final_exp_split;
                        td.mon_order = final_mon_order;
                        td.thief_mons = self.battle.thief_mons.clone();
                        if self.has_return() {
                            td.custom_move_data = Self::return_custom_data(&self.battle.friendship_data);
                        }
                        td.pay_day_amount = Some(store_i64(store, &self.keys.player_money) - self.battle.initial_money);
                        let mut ev = EventDefinition::with_trainer(td);
                        ev.notes = roar_flag();
                        self.queue_new_event(ev);
                    }
                    if self.battle.loss_detected {
                        if self.battle.is_trainer_battle {
                            let mut ev = EventDefinition::with_trainer(TrainerEventDefinition::new(&py_str(&self.battle.trainer_name)));
                            ev.notes = trainer_loss_flag();
                            self.queue_new_event(ev);
                            let mons = std::mem::take(&mut self.battle.defeated_trainer_mons);
                            for m in mons {
                                self.queue_new_event(m);
                            }
                        }
                        self.queue_new_event(EventDefinition::with_blackout());
                    }
                    if self.battle.delayed_move_updater.trigger(false) {
                        self.delayed_moves_update(store);
                    }
                    if self.battle.delayed_item_updater.trigger(false) {
                        self.item_cache_update(store, true, false, false, false, false, false, false);
                    }
                    if self.battle.delayed_levelup.trigger(false) {
                        self.delayed_level_update(store);
                    }
                }
            }
            GameState::InventoryChange => {
                if next != GameState::Resetting {
                    let (gained, lost, held) = (self.inventory.money_gained, self.inventory.money_lost, self.inventory.held_item_changed);
                    self.item_cache_update(store, true, lost, gained, false, false, false, held);
                }
            }
            GameState::RareCandy => {
                if next != GameState::Resetting {
                    if self.item_cache_update(store, true, false, false, false, true, false, false) {
                        let level = store_i64(store, &self.keys.mon_level);
                        self.solo_mon_levelup(level);
                    }
                    if self.candy.move_learned {
                        self.update_team_cache(store, true, false);
                        self.move_cache_update(store, true, None, false, false, true);
                    }
                }
            }
            GameState::Tm => {
                if next != GameState::Resetting && !self.item_cache_update(store, true, false, false, false, false, true, false) {
                    self.move_cache_update(store, true, None, false, false, true);
                }
            }
            GameState::MoveDelete => {
                if next != GameState::Resetting {
                    self.move_cache_update(store, true, None, false, true, false);
                }
            }
            GameState::Vitamin => {
                if next != GameState::Resetting {
                    if self.vitamin.error_delay <= 0 {
                        log::error!("Vitamin state hit error timeout. Will attempt to see if any vitamins were used anyways");
                    }
                    self.item_cache_update(store, true, false, false, true, false, false, false);
                }
            }
            GameState::Overworld => {
                if next == GameState::InventoryChange {
                    self.inventory.external_held_item_flag = self.overworld.propagate_held_item_flag;
                }
                self.overworld.propagate_held_item_flag = false;
            }
        }
    }

    fn transition(&mut self, store: &PropertyStore, new: &GameHookProperty, prev: &GameHookProperty) -> GameState {
        let state = self.cur_state;
        if state != GameState::Resetting {
            if let Some(r) = self.watch_for_reset(store, new) {
                return r;
            }
        }
        match state {
            GameState::Uninitialized => {
                if new.path == self.keys.gametime_seconds {
                    if self.uninit.seconds_delay <= 0 {
                        if self.in_battle_now(store) {
                            return GameState::Battle;
                        }
                        return GameState::Overworld;
                    } else if !self.uninit.is_waiting && !store.get(&self.keys.player_id).map(|p| p.eq_i64(0)).unwrap_or(false) {
                        self.uninit.is_waiting = true;
                    }
                    if self.uninit.is_waiting {
                        self.uninit.seconds_delay -= 1;
                    }
                }
                state
            }
            GameState::Resetting => {
                if new.path == self.keys.player_id {
                    if prev.eq_i64(0) && !new.eq_i64(0) {
                        self.resetting.is_waiting = true;
                    }
                } else if new.path == self.keys.gametime_seconds {
                    if !self.resetting.is_waiting {
                        self.resetting.is_waiting = true;
                    } else if self.resetting.seconds_delay <= 0 {
                        return GameState::Overworld;
                    } else {
                        self.resetting.seconds_delay -= 1;
                    }
                }
                state
            }
            GameState::Battle => self.battle_transition(store, new, prev),
            GameState::InventoryChange => {
                if new.path == self.keys.player_money {
                    if new.as_i64().unwrap_or(0) > prev.as_i64().unwrap_or(0) {
                        self.inventory.money_gained = true;
                    } else {
                        self.inventory.money_lost = true;
                    }
                } else if self.keys.all_item_fields.contains(&new.path) {
                    self.inventory.seconds_delay = BASE_DELAY;
                } else if new.path == self.keys.mon_held_item {
                    self.inventory.held_item_changed = true;
                } else if new.path == self.keys.gametime_seconds {
                    if self.inventory.seconds_delay <= 0 {
                        return GameState::Overworld;
                    } else {
                        self.inventory.seconds_delay -= 1;
                    }
                }
                state
            }
            GameState::RareCandy => {
                if self.keys.player_moves.contains(&new.path) {
                    self.candy.move_learned = true;
                } else if self.keys.item_quantity.contains(&new.path) || self.keys.item_type.contains(&new.path) {
                    self.candy.cur_delay = BASE_DELAY;
                } else if new.path == self.keys.gametime_seconds {
                    if self.candy.cur_delay <= 0 {
                        return GameState::Overworld;
                    } else {
                        self.candy.cur_delay -= 1;
                    }
                }
                state
            }
            GameState::Tm => {
                if self.keys.all_item_fields.contains(&new.path) {
                    self.tm.seconds_delay = BASE_DELAY;
                } else if new.path == self.keys.gametime_seconds {
                    if self.tm.seconds_delay <= 0 {
                        return GameState::Overworld;
                    } else {
                        self.tm.seconds_delay -= 1;
                    }
                }
                state
            }
            GameState::MoveDelete => {
                if new.path == self.keys.gametime_seconds {
                    if self.move_delete.cur_delay <= 0 {
                        return GameState::Overworld;
                    } else {
                        self.move_delete.cur_delay -= 1;
                    }
                } else if self.keys.player_moves.contains(&new.path) {
                    self.move_delete.cur_delay = BASE_DELAY;
                }
                state
            }
            GameState::Vitamin => {
                if self.keys.item_type.contains(&new.path) {
                    self.vitamin.item_removal_detected = true;
                } else if new.path == self.keys.gametime_seconds {
                    if self.vitamin.item_removal_detected {
                        if self.vitamin.cur_delay <= 0 {
                            return GameState::Overworld;
                        } else {
                            self.vitamin.cur_delay -= 1;
                        }
                    }
                    if self.vitamin.error_delay > 0 {
                        self.vitamin.error_delay -= 1;
                    } else {
                        return GameState::Overworld;
                    }
                }
                state
            }
            GameState::Overworld => self.overworld_transition(store, new, prev),
        }
    }

    fn cache_defeated(&mut self, first: bool) {
        let (species, level) = if first {
            (std::mem::take(&mut self.battle.cached_first_mon_species), std::mem::take(&mut self.battle.cached_first_mon_level))
        } else {
            (std::mem::take(&mut self.battle.cached_second_mon_species), std::mem::take(&mut self.battle.cached_second_mon_level))
        };
        if self.battle.is_trainer_battle {
            self.battle.defeated_trainer_mons.push(EventDefinition::with_wild(WildPkmnEventDefinition::new(&species, level, 1, true)));
        } else {
            self.queue_new_event(EventDefinition::with_wild(WildPkmnEventDefinition::new(&species, level, 1, false)));
        }
    }

    fn battle_transition(&mut self, store: &PropertyStore, new: &GameHookProperty, _prev: &GameHookProperty) -> GameState {
        let state = GameState::Battle;
        if new.path == self.keys.battle_background_tiles && new.eq_i64(0) {
            return GameState::Overworld;
        }
        // don't track anything during the tutorial battle
        if self.battle.is_tutorial_battle {
            return state;
        }
        if new.path == self.keys.mon_exppoints {
            if !self.battle.cached_first_mon_species.is_empty() || self.battle.cached_first_mon_level != 0 {
                self.cache_defeated(true);
            } else if !self.battle.cached_second_mon_species.is_empty() || self.battle.cached_second_mon_level != 0 {
                self.cache_defeated(false);
            } else {
                log::error!("Solo mon gained experience, but we didn't properly cache which enemy mon was defeated... This is normal if the a different pokemon has been pulled into player's slot 1");
            }
        } else if new.path == self.keys.battle_flag {
            if new.truthy() && !self.battle.battle_started && self.battle.delayed_initialization.trigger(false) {
                self.battle_ready(store);
            }
        } else if new.path == self.keys.battle_first_enemy_hp {
            if new.eq_i64(0) {
                self.battle.cached_first_mon_species = self.conv.pkmn_name_convert(store.str_of(&self.keys.battle_first_enemy_species).as_deref()).unwrap_or_default();
                self.battle.cached_first_mon_level = store_i64(store, &self.keys.battle_first_enemy_level);
                self.battle.friendship_data.push(store_i64(store, &self.keys.mon_friendship));
            }
        } else if new.path == self.keys.battle_second_enemy_hp {
            if new.eq_i64(0) {
                self.battle.cached_second_mon_species = self.conv.pkmn_name_convert(store.str_of(&self.keys.battle_second_enemy_species).as_deref()).unwrap_or_default();
                self.battle.cached_second_mon_level = store_i64(store, &self.keys.battle_second_enemy_level);
                self.battle.friendship_data.push(store_i64(store, &self.keys.mon_friendship));
            }
        } else if new.path == self.keys.battle_player_mon_hp {
            let player_mon_pos = store_i64(store, &self.keys.battle_player_mon_party_pos);
            // a non-numeric HP makes Python's `<= 0` raise (and the change is dropped)
            let Some(hp) = new.as_i64() else { return state };
            if player_mon_pos == 0 && hp <= 0 {
                if self.battle.battle_started {
                    self.battle.loss_detected = true;
                }
            } else if hp <= 0 {
                let pos = self.first_enemy_mon_pos(store, None);
                if let Some(set) = self.split_at(pos) {
                    set.remove(&player_mon_pos);
                }
                if self.battle.is_double_battle {
                    let pos2 = self.second_enemy_mon_pos(store, None);
                    if let Some(set) = self.split_at(pos2) {
                        set.remove(&player_mon_pos);
                    }
                }
            }
        } else if new.path == self.keys.battle_ally_mon_hp {
            if self.battle.is_double_battle && new.as_i64().map(|v| v <= 0).unwrap_or(false) {
                let ally_mon_pos = store_i64(store, &self.keys.battle_ally_mon_party_pos);
                let first_alive = store_i64(store, &self.keys.battle_first_enemy_hp) > 0 || !self.battle.cached_first_mon_species.is_empty();
                let pos = self.first_enemy_mon_pos(store, None);
                if let Some(set) = self.split_at(pos) {
                    if set.contains(&ally_mon_pos) && first_alive {
                        set.remove(&ally_mon_pos);
                    }
                }
                let second_alive = store_i64(store, &self.keys.battle_second_enemy_hp) > 0 || !self.battle.cached_second_mon_species.is_empty();
                let pos2 = self.second_enemy_mon_pos(store, None);
                if let Some(set) = self.split_at(pos2) {
                    if set.contains(&ally_mon_pos) && second_alive {
                        set.remove(&ally_mon_pos);
                    }
                }
            }
        } else if self.keys.player_moves.contains(&new.path) {
            self.battle.delayed_move_updater.begin_waiting(true);
        } else if self.keys.all_item_fields.contains(&new.path) {
            self.battle.delayed_item_updater.begin_waiting(true);
        } else if new.path == self.keys.mon_held_item {
            self.battle.delayed_held_item_updater.begin_waiting(true);
        } else if new.path == self.keys.mon_level {
            self.battle.delayed_levelup.begin_waiting(true);
        } else if new.path == self.keys.gametime_seconds {
            if self.battle.delayed_move_updater.tick() {
                self.delayed_moves_update(store);
            }
            if self.battle.delayed_item_updater.tick() {
                self.item_cache_update(store, true, false, false, false, false, false, false);
            }
            if self.battle.delayed_held_item_updater.tick() {
                self.delayed_held_item_update(store);
            }
            if self.battle.delayed_levelup.tick() {
                self.delayed_level_update(store);
            }
            if self.battle.delayed_initialization.tick() {
                self.battle_ready(store);
            }
        } else if new.path == self.keys.battle_player_mon_party_pos {
            let v = new.as_i64().unwrap_or(-1);
            if (0..6).contains(&v) {
                if store_i64(store, &self.keys.battle_first_enemy_hp) > 0 {
                    let pos = self.first_enemy_mon_pos(store, None);
                    if let Some(set) = self.split_at(pos) {
                        set.insert(v);
                    }
                }
                if self.battle.is_double_battle && store_i64(store, &self.keys.battle_second_enemy_hp) > 0 {
                    let pos2 = self.second_enemy_mon_pos(store, None);
                    if let Some(set) = self.split_at(pos2) {
                        set.insert(v);
                    }
                }
            }
        } else if new.path == self.keys.battle_ally_mon_party_pos {
            let v = new.as_i64().unwrap_or(-1);
            if self.battle.is_double_battle && (0..6).contains(&v) {
                if store_i64(store, &self.keys.battle_first_enemy_hp) > 0 {
                    let pos = self.first_enemy_mon_pos(store, None);
                    if let Some(set) = self.split_at(pos) {
                        set.insert(v);
                    }
                }
                if store_i64(store, &self.keys.battle_second_enemy_hp) > 0 {
                    let pos2 = self.second_enemy_mon_pos(store, None);
                    if let Some(set) = self.split_at(pos2) {
                        set.insert(v);
                    }
                }
            }
        } else if new.path == self.keys.battle_first_enemy_party_pos || new.path == self.keys.battle_second_enemy_party_pos {
            let is_first = new.path == self.keys.battle_first_enemy_party_pos;
            let real_new_value = if is_first { self.first_enemy_mon_pos(store, new.as_i64()) } else { self.second_enemy_mon_pos(store, new.as_i64()) };
            let allowed = is_first || self.battle.is_double_battle;
            if let Some(real) = real_new_value {
                if allowed && real >= 0 && (real as usize) < self.battle.exp_split.len() {
                    if store_i64(store, &self.keys.battle_player_mon_hp) > 0 {
                        self.battle.exp_split[real as usize] = HashSet::from([store_i64(store, &self.keys.battle_player_mon_party_pos)]);
                    }
                    if self.battle.is_double_battle && store_i64(store, &self.keys.battle_ally_mon_hp) > 0 {
                        let ally = store_i64(store, &self.keys.battle_ally_mon_party_pos);
                        self.battle.exp_split[real as usize].insert(ally);
                    }
                    if !self.battle.enemy_mon_order.contains(&real) {
                        self.battle.enemy_mon_order.push(real);
                    }
                }
            }
        }
        state
    }

    fn overworld_transition(&mut self, store: &PropertyStore, new: &GameHookProperty, prev: &GameHookProperty) -> GameState {
        let state = GameState::Overworld;
        if self.overworld.waiting_for_new_file || self.overworld.waiting_for_solo_mon_in_slot_1 {
            let mut check_for_battle = false;
            if new.path == self.keys.gametime_seconds {
                if self.overworld.wrong_mon_delay <= 0 {
                    self.overworld.waiting_for_solo_mon_in_slot_1 = false;
                    self.overworld.wrong_mon_in_slot_1 = false;
                    check_for_battle = true;
                }
                if self.overworld.new_file_delay <= 0 {
                    self.overworld.waiting_for_new_file = false;
                    self.controller.route_restarted();
                    self.update_all_cached_info(store);
                    check_for_battle = true;
                }
                self.overworld.new_file_delay -= 1;
                self.overworld.wrong_mon_delay -= 1;
            }
            if check_for_battle && self.in_battle_now(store) {
                log::info!("tranitioning into battle pre-emptively");
                return GameState::Battle;
            }
            return state;
        }
        if new.path == self.keys.battle_outcome {
            // Python compares the property *object* (truthy when mapped)
            if new.is_null() && store.get(&self.keys.battle_flag).is_some() {
                return GameState::Battle;
            }
        } else if new.path == self.keys.battle_flag {
            // Python: `get(KEY_BATTLE_OUTCOME) is None` is never true for a mapped key
            if new.truthy() && store.get(&self.keys.battle_outcome).is_none() {
                return GameState::Battle;
            }
        } else if new.path == self.keys.overworld_map {
            let area = py_str(&store.value(&self.keys.overworld_map));
            self.controller.entered_new_area(&area);
        } else if new.path == self.keys.player_id {
            if prev.truthy() && self.player_id.as_ref() != Some(&store.value(&self.keys.player_id)) {
                self.overworld.waiting_for_new_file = true;
            }
        } else if new.path == self.keys.mon_held_item {
            self.overworld.propagate_held_item_flag = true;
        } else if self.keys.all_item_fields.contains(&new.path) {
            return GameState::InventoryChange;
        } else if new.path == self.keys.player_money {
            // Money can move in the overworld without a bag change arriving first:
            // the whiteout halving, or a sale whose money update lands before the
            // bag slot update. Let InventoryChange classify it against the money
            // cache; otherwise the cache goes stale and the next sale after a
            // blackout is recorded as a plain Use/Drop
            return GameState::InventoryChange;
        } else if new.path == self.keys.mon_species {
            if !prev.truthy() {
                self.overworld.waiting_for_registration = true;
            } else if self.solo_mon_key.species == self.conv.pkmn_name_convert(prev.as_str()) {
                self.overworld.wrong_mon_in_slot_1 = true;
            } else if self.solo_mon_key.species == self.conv.pkmn_name_convert(new.as_str()) {
                self.overworld.wrong_mon_delay = BASE_DELAY;
                self.overworld.waiting_for_solo_mon_in_slot_1 = true;
            }
        } else if new.path == self.keys.mon_level {
            if !self.overworld.waiting_for_registration && !self.overworld.wrong_mon_in_slot_1 {
                return GameState::RareCandy;
            }
        } else if self.keys.player_moves.contains(&new.path) {
            if !self.overworld.waiting_for_registration && !self.overworld.wrong_mon_in_slot_1 {
                let mut all_cur_moves: Vec<Value> = Vec::new();
                for move_path in &self.keys.player_moves {
                    if *move_path == prev.path {
                        all_cur_moves.push(prev.value.clone());
                    } else {
                        all_cur_moves.push(store.value(move_path));
                    }
                }
                if new.is_null() || all_cur_moves.contains(&new.value) {
                    return GameState::MoveDelete;
                } else {
                    return GameState::Tm;
                }
            }
        } else if self.keys.stat_exp.contains(&new.path) {
            if !self.overworld.waiting_for_registration && !self.overworld.wrong_mon_in_slot_1 {
                return GameState::Vitamin;
            }
        } else if new.path == self.keys.sstp_tracking {
            let loc = store.value(&self.keys.overworld_map);
            if new.eq_str("SAVE") {
                self.queue_new_event(EventDefinition::with_save_value(&loc));
            } else if new.eq_str("HEAL") {
                self.queue_new_event(EventDefinition::with_heal_value(&loc));
            }
        } else if new.path == self.keys.gametime_seconds {
            if self.overworld.waiting_for_registration {
                if self.overworld.register_delay <= 0 {
                    self.overworld.waiting_for_registration = false;
                    self.update_team_cache(store, true, true);
                }
                self.overworld.register_delay -= 1;
            } else if self.overworld.wrong_mon_in_slot_1 {
                if self.overworld.wrong_mon_delay <= 0 {
                    self.update_team_cache(store, true, true);
                    self.overworld.wrong_mon_in_slot_1 = self.solo_mon_key.species != self.conv.pkmn_name_convert(store.str_of(&self.keys.mon_species).as_deref());
                }
                self.overworld.wrong_mon_delay -= 1;
            }
            if self.overworld.validation_delay > 0 {
                self.overworld.validation_delay -= 1;
            } else if self.overworld.validation_delay == 0 {
                self.overworld.validation_delay -= 1;
            }
            if self.in_battle_now(store) {
                return GameState::Battle;
            }
        }
        state
    }

    fn spawn_processing_thread(&self) {
        let ctx = ProcessCtx {
            controller: self.controller.clone(),
            gen: self.gen.clone(),
            queue: self.queue.clone(),
            active: self.active.clone(),
        };
        let conv = self.conv.clone();
        let tutorial_now = self.tutorial_flag_now.clone();
        std::thread::Builder::new()
            .name("gen3-recorder-events".into())
            .spawn(move || {
                ctx.run(|ctx, ev| process_one(ctx, ev, &conv, &tutorial_now));
            })
            .ok();
    }
}

/// `_process_events` body (gen 3).
fn process_one(ctx: &ProcessCtx, mut cur_event: EventDefinition, conv: &Gen3Converter, tutorial_now: &std::sync::atomic::AtomicBool) {
    let gen = &ctx.gen;
    if cur_event.notes == reset_flag() {
        log::info!("Resetting to last save...");
        ctx.controller.game_reset();
        return;
    } else if let Some(td) = cur_event.trainer_def.clone() {
        let trainer_id: i64 = match td.trainer_name.trim().parse::<i64>() {
            Ok(i) => i,
            Err(_) => {
                ctx.add_error(format!("Failed to find trainer from GameHook: (<class 'str'>) {}", td.trainer_name));
                return;
            }
        };
        let Some(trainer) = gen.trainer_db().get_trainer_by_id(trainer_id).cloned() else {
            ctx.add_error(format!("Failed to find trainer from GameHook: (<class 'int'>) {}", trainer_id));
            return;
        };
        if let Some(t) = cur_event.trainer_def.as_mut() {
            t.trainer_name = trainer.name.clone();
        }
        let second_raw = td.second_trainer_name.clone();
        if xpr_core::pyjson::truthy(&second_raw) {
            let second_id = crate::gamehook::value_as_i64(&second_raw).or_else(|| second_raw.as_str().and_then(|s| s.trim().parse().ok()));
            let second = second_id.and_then(|i| gen.trainer_db().get_trainer_by_id(i).cloned());
            let Some(second_trainer) = second else {
                ctx.add_error(format!("Failed to find second trainer from GameHook: (<class 'int'>) {}", py_str(&second_raw)));
                return;
            };
            if let Some(t) = cur_event.trainer_def.as_mut() {
                t.second_trainer_name = Value::String(second_trainer.name.clone());
            }
        }
        let trainer_name = trainer.name.clone();
        if cur_event.notes == trainer_loss_flag() {
            log::info!("Handling trainer loss: {}", trainer_name);
            ctx.controller.lost_trainer_battle(&trainer_name);
            return;
        } else if cur_event.notes == roar_flag() {
            log::info!("Updating full trainer event: {}", event_str(gen, &cur_event));
            log::info!("Updating split exp for trainer {} to {:?}", trainer_name, td.exp_split);
            let name = trainer_name.clone();
            let mut test_obj = ctx.controller.host().call(|h| h.get_previous_event(None));
            while !RecorderController::is_trainer_event(test_obj.as_ref(), &name) {
                let Some(obj) = &test_obj else { break };
                let id = obj.group_id;
                test_obj = ctx.controller.host().call(move |h| h.get_previous_event(Some(id)));
            }
            match test_obj {
                None => log::error!("Failed to find trainer fight to update for exp split behavior"),
                Some(obj) => {
                    cur_event.notes = String::new();
                    let mut expected_money = trainer.money;
                    log::info!("held item: {}", obj.final_held_item.as_deref().unwrap_or("None"));
                    if obj.final_held_item.as_deref() == Some(consts::AMULET_COIN_ITEM_NAME) {
                        expected_money *= 2;
                    }
                    if let Some(t) = cur_event.trainer_def.as_mut() {
                        t.pay_day_amount = Some((t.pay_day_amount.unwrap_or(0) - expected_money).max(0));
                    }
                    let id = obj.group_id;
                    let ev = cur_event.clone();
                    ctx.controller.host().call(move |h| h.update_existing_event(id, ev));
                }
            }
            let loss_flag = trainer_loss_flag();
            let has_pending_loss = ctx.queue.any(|e| e.trainer_def.as_ref().map(|t| t.trainer_name == td.trainer_name).unwrap_or(false) && e.notes == loss_flag);
            if !has_pending_loss {
                // Beating the champion runs the Hall of Fame, which saves the game,
                // then the credits reboot to the title screen. The reboot looks
                // exactly like a soft reset to the FSM, so record the autosave here
                // or game_reset() rolls the route back past the champion fight (and
                // anything done between the last manual save and it)
                if trainer.trainer_class == consts::CHAMPION_TRAINER_CLASS {
                    log::info!("Champion {} defeated, recording the Hall of Fame autosave", trainer_name);
                    ctx.controller.add_event(EventDefinition::with_save(consts::POST_CHAMPION_AUTOSAVE_LOCATION));
                }
                ctx.controller.check_final_trainer(&trainer_name);
            }
            return;
        }
    } else if let Some(item_def) = cur_event.item_event_def.clone() {
        log::info!("getting item from {}", item_def.item_name);
        // skip item events during tutorial battles
        if tutorial_now.load(Ordering::SeqCst) {
            log::info!("Skipping item event during tutorial battle: {}", item_def.to_string());
            return;
        }
        let mut item = gen.item_db().get_item(&item_def.item_name).cloned();
        if item.is_none() {
            if let Some(full) = resolve_tm_name(gen, &item_def.item_name) {
                if let Some(i) = cur_event.item_event_def.as_mut() {
                    i.item_name = full.clone();
                }
                item = gen.item_db().get_item(&full).cloned();
            }
        }
        let Some(item) = item else {
            ctx.add_error(format!("Failed to find item from GameHook: {} for event {}", item_def.item_name, event_str(gen, &cur_event)));
            return;
        };
        if item_def.is_acquire {
            let prev_event = ctx.controller.host().call(|h| h.get_previous_event(None));
            let item_name = cur_event.item_event_def.as_ref().map(|i| i.item_name.clone()).unwrap_or_default();
            if let Some(prev) = &prev_event {
                if prev.definition.trainer_def.is_some() {
                    let reward = prev.first_trainer_name.as_deref().and_then(|n| gen.get_fight_reward(n));
                    if reward == Some(item_name.as_str()) {
                        log::info!("Intentionally ignoring item add for battle reward: {}", item_name);
                        return;
                    }
                }
            }
            let name = item.name.clone();
            if item.is_key_item && ctx.controller.host().call(move |h| h.final_inventory_has(&name)) {
                log::info!("Intentionally ignoring item add for duplicate key item: {}", item_name);
                return;
            }
        }
    } else if let Some(lm) = cur_event.learn_move.clone() {
        let to_learn = lm.move_to_learn.as_deref().and_then(|m| gen.move_db().get_move(m));
        let to_forget = lm.destination_name.as_deref().and_then(|m| gen.move_db().get_move(m));
        if lm.move_to_learn.is_some() && to_learn.is_none() {
            ctx.add_error(format!("Failed to find move from GameHook: {} for event {}", lm.move_to_learn.as_deref().unwrap_or("None"), event_str(gen, &cur_event)));
            return;
        } else if lm.destination_name.is_some() && to_forget.is_none() {
            ctx.add_error(format!("Failed to find move from GameHook: {} for event {}", lm.destination_name.as_deref().unwrap_or("None"), event_str(gen, &cur_event)));
            return;
        }
        if lm.source != consts::MOVE_SOURCE_LEVELUP && lm.source != consts::MOVE_SOURCE_TUTOR {
            match resolve_tm_name(gen, &lm.source) {
                Some(full) => {
                    if let Some(l) = cur_event.learn_move.as_mut() {
                        l.source = full;
                    }
                }
                None => cur_event.notes = format!("{}Failed to find tm for item source: {}", consts::RECORDING_ERROR_FRAGMENT, lm.source),
            }
        } else if lm.source == consts::MOVE_SOURCE_LEVELUP {
            let check = lm.clone();
            if !ctx.controller.host().call(move |h| h.is_valid_levelup_move(&check)) {
                log::warn!("Seemingly invalid level up move {} at level {}", lm.move_to_learn.as_deref().unwrap_or("None"), lm.level.display());
                if let Some(l) = cur_event.learn_move.as_mut() {
                    l.level = LevelVal::any();
                    match l.move_to_learn.as_deref().and_then(|m| conv.get_hm_name(m)) {
                        Some(hm) => {
                            log::warn!("Looks like an HM, defaulting to that");
                            l.source = hm.to_string();
                        }
                        None => {
                            log::warn!("Not an HM, defaulting to tutored move");
                            l.source = consts::MOVE_SOURCE_TUTOR.to_string();
                        }
                    }
                }
            }
        }
    } else if let Some(hold) = cur_event.hold_item.clone() {
        if cur_event.notes == held_check_flag() {
            cur_event.notes = String::new();
            let first = ctx.controller.host().call(|h| h.get_previous_event(None));
            let mut list_of_prev_events = vec![first.clone()];
            if let Some(f) = &first {
                let id = f.group_id;
                list_of_prev_events.push(ctx.controller.host().call(move |h| h.get_previous_event(Some(id))));
            }
            let orig_held_item = ctx.controller.host().call(|h| h.final_held_item());
            let mut to_delete = Vec::new();
            for prev in list_of_prev_events.iter().flatten() {
                if let Some(i) = &prev.definition.item_event_def {
                    if Some(i.item_name.as_str()) == hold.item_name.as_deref() && i.item_amount == 1 && !i.is_acquire && !i.with_money {
                        to_delete.push(prev.group_id);
                    }
                }
            }
            if !to_delete.is_empty() {
                if let Some(orig) = &orig_held_item {
                    for prev in list_of_prev_events.iter().flatten() {
                        if let Some(i) = &prev.definition.item_event_def {
                            if i.item_name == *orig && i.item_amount == 1 && i.is_acquire && !i.with_money {
                                to_delete.push(prev.group_id);
                            }
                        }
                    }
                }
            }
            if !to_delete.is_empty() {
                ctx.controller.host().call(move |h| h.delete_events(&to_delete));
            } else {
                log::error!("expected to be fixing events before a hold item event, but no fix was found: {}", event_str(gen, &cur_event));
            }
        }
    } else if let Some(w) = cur_event.wild_pkmn_info.clone() {
        if gen.pkmn_db().get_pkmn(&w.name).is_none() {
            ctx.add_error(format!("Failed to find wild pokemon from GameHook: {} for event {}", w.name, event_str(gen, &cur_event)));
            return;
        }
    }
    let mut auto_save = false;
    if cur_event.heal.as_ref().and_then(|h| h.location.as_deref()) == Some("INDIGO") {
        let prev_event = ctx.controller.host().call(|h| h.get_previous_event(None));
        if let Some(prev) = prev_event {
            if prev.definition.trainer_def.as_ref().map(|t| t.trainer_name == "Champion Lance").unwrap_or(false) {
                auto_save = true;
            }
        }
    }
    log::info!("adding new event: {}", event_str(gen, &cur_event));
    ctx.controller.add_event(cur_event);
    if auto_save {
        ctx.controller.add_event(EventDefinition::with_save("Post-Champion Autosave"));
    }
}

impl GameRecorder for Gen3Machine {
    fn on_mapper_loaded(&mut self, store: &PropertyStore) -> (Vec<String>, Vec<String>) {
        self.keys = Gen3Keys::configure(self.is_frlg);
        let invalid = self.keys.fix_case(store);
        (self.keys.all_keys_to_register(), invalid)
    }

    fn is_active(&self) -> bool {
        is_set(&self.active)
    }

    fn startup(&mut self, store: &PropertyStore) {
        self.active.store(true, Ordering::SeqCst);
        if let Some(k) = &self.keys.tutorial_battle_flag {
            self.tutorial_flag_now.store(store.truthy(k), Ordering::SeqCst);
        }
        self.cur_state = GameState::Uninitialized;
        self.on_enter(GameState::Uninitialized, store);
        self.controller.set_game_state(self.cur_state);
        crate::shuckie::supershuckie().start();
        self.spawn_processing_thread();
    }

    fn handle_event(&mut self, store: &PropertyStore, new: &GameHookProperty, prev: &GameHookProperty) {
        if self.keys.tutorial_battle_flag.as_deref() == Some(new.path.as_str()) {
            self.tutorial_flag_now.store(new.truthy(), Ordering::SeqCst);
        }
        if self.debug_mode && new.path != self.keys.gametime_seconds && !new.path.contains("audio") {
            log::info!("Change of {} from {} to {} for state {}", new.path, py_str(&prev.value), py_str(&new.value), self.cur_state);
        }
        let result = self.transition(store, new, prev);
        if result != self.cur_state {
            log::info!("Moving from {} state to {} due to change {}, from {} to {}", self.cur_state, result, new.path, py_str(&prev.value), py_str(&new.value));
            let old = self.cur_state;
            self.on_exit(old, result, store);
            self.on_enter(result, store);
            self.cur_state = result;
            self.controller.set_game_state(result);
        }
    }

    fn active_flag(&self) -> ActiveFlag {
        self.active.clone()
    }

    fn shutdown(&mut self) {
        log::info!("Shutting down Emerald recording FSM");
        crate::controller::deactivate(&self.active);
    }
}
