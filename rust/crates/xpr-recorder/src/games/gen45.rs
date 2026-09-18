//! Port of `route_recording/game_recorders/gen_four/*` (Platinum, HG/SS) and
//! `gen_five/*` (Black/White, B2/W2): one machine parameterised by a key table
//! and a few per-gen switches (the gen 5 files are a copy of gen 4 with the
//! mapper gaps of `MAPPER_GAPS.md` and a data-driven battle initialisation).

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
use crate::controller::{dedupe, fix_key, fix_keys, fix_opt_key, is_set, new_active_flag, ActiveFlag, EventQueue, GameRecorder, GameState, RecorderController};
use crate::gamehook::{GameHookProperty, PropertyStore};
use crate::host::StartInfo;

pub const RETURN_MOVE_NAME: &str = "Return";
pub const GOLDENROD_LOTTERY_MAP: &str = "Goldenrod City - Map 12";
pub const GOLDENROD_LOTTERY_PRICE: i64 = 300;
pub const GOLDENROD_LOTTERY_ITEMS: [&str; 21] = [
    "Dragon Claw", "Shadow Claw", "Flash Cannon", "Charge Beam", "Drain Punch", "Facade", "Silver Wind", "Luxury Ball", "Nest Ball", "Repeat Ball",
    "Net Ball", "Quick Ball", "Dusk Ball", "Timer Ball", "Persim Berry", "Cheri Berry", "Chesto Berry", "Pecha Berry", "Rawst Berry", "Aspear Berry",
    "Oran Berry",
];
pub const SAVE_SOUND_EFFECT_VALUE: i64 = 36342100;
pub const HEAL_SOUND_EFFECT_VALUE: i64 = 36335692;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flavor {
    Platinum,
    HeartGoldSoulSilver,
    BlackWhite,
    Black2White2,
}

impl Flavor {
    pub fn is_gen5(self) -> bool {
        matches!(self, Flavor::BlackWhite | Flavor::Black2White2)
    }
    pub fn is_hgss(self) -> bool {
        self == Flavor::HeartGoldSoulSilver
    }
}

#[derive(Clone, Debug)]
pub struct Gen45Keys {
    pub flavor: Flavor,
    pub meta_state: String,
    pub overworld_map: String,
    pub player_id: String,
    pub player_money: String,
    pub mon_exppoints: String,
    pub mon_level: String,
    pub mon_species: String,
    pub mon_held_item: String,
    pub team_held_item: Vec<String>,
    pub mon_friendship: String,
    pub mon_pid: String,
    pub battle_solo_hp: String,
    pub battle_team_hp: Vec<String>,
    pub team_species: Vec<String>,
    pub team_level: Vec<String>,
    pub team_hp: Vec<String>,
    pub team_iv_attack: Vec<String>,
    pub team_iv_defense: Vec<String>,
    pub team_iv_speed: Vec<String>,
    pub team_iv_special_attack: Vec<String>,
    pub team_iv_special_defense: Vec<String>,
    pub team_ev: [Vec<String>; 6],
    pub player_moves: Vec<String>,
    pub stat_exp: Vec<String>,
    pub gametime_seconds: String,
    pub trainer_battle_flag: Option<String>,
    pub battle_outcome: Option<String>,
    pub battle_flags: String,
    pub battle_player_mon_party_pos: String,
    pub battle_player_mon_hp: String,
    pub battle_player_mon_exp: String,
    pub battle_ally_mon_party_pos: Option<String>,
    pub battle_ally_mon_hp: Option<String>,
    pub battle_ally_mon_pid: Option<String>,
    pub battle_trainer_a_number: String,
    pub battle_trainer_b_number: String,
    pub battle_ally_number: Option<String>,
    pub battle_first_enemy_species: String,
    pub battle_first_enemy_level: String,
    pub battle_first_enemy_hp: String,
    pub battle_first_enemy_party_pos: String,
    pub battle_first_enemy_pid: String,
    pub battle_second_enemy_species: Option<String>,
    pub battle_second_enemy_level: Option<String>,
    pub battle_second_enemy_hp: Option<String>,
    pub battle_second_enemy_party_pos: Option<String>,
    pub battle_second_enemy_pid: Option<String>,
    pub battle_enemy_1_pid: Vec<String>,
    pub battle_enemy_2_pid: Vec<String>,
    pub enemy_team_species: Vec<String>,
    pub enemy_2_team_species: Vec<String>,
    pub save_count: Option<String>,
    pub audio_sound_effect_1: Option<String>,
    pub audio_sound_effect_2: Option<String>,
    pub item_type: Vec<String>,
    pub item_quantity: Vec<String>,
    pub medicine_type: Vec<String>,
    pub medicine_quantity: Vec<String>,
    pub ball_type: Vec<String>,
    pub ball_quantity: Vec<String>,
    pub berry_type: Vec<String>,
    pub berry_quantity: Vec<String>,
    pub tmhm_type: Vec<String>,
    pub tmhm_quantity: Vec<String>,
    pub all_item_fields: HashSet<String>,
}

impl Gen45Keys {
    pub fn configure(flavor: Flavor) -> Gen45Keys {
        let gen5 = flavor.is_gen5();
        let team = |suffix: &str| -> Vec<String> { (0..6).map(|i| format!("player.team.{}.{}", i, suffix)).collect() };
        let bag = |pocket: &str, field: &str, n: usize| -> Vec<String> { (0..n).map(|i| format!("bag.{}.{}.{}", pocket, i, field)).collect() };
        let (items_n, medicine_n, balls_n, berries_n, tmhm_n) = match flavor {
            Flavor::Platinum | Flavor::HeartGoldSoulSilver => (40, 20, 16, 63, 99),
            Flavor::BlackWhite => (40, 48, 0, 30, 102),
            Flavor::Black2White2 => (146, 48, 0, 34, 101),
        };
        let opt = |s: &str| if gen5 { None } else { Some(s.to_string()) };
        let mut k = Gen45Keys {
            flavor,
            meta_state: "meta.state".into(),
            overworld_map: "overworld.map_name".into(),
            player_id: "player.player_id".into(),
            player_money: "bag.money".into(),
            mon_exppoints: "player.team.0.exp".into(),
            mon_level: "player.team.0.level".into(),
            mon_species: "player.team.0.species".into(),
            mon_held_item: "player.team.0.held_item".into(),
            team_held_item: team("held_item"),
            mon_friendship: "player.team.0.friendship".into(),
            mon_pid: "player.team.0.internals.personality_value".into(),
            battle_solo_hp: "battle.player.team.0.stats.hp".into(),
            battle_team_hp: (0..6).map(|i| format!("battle.player.team.{}.stats.hp", i)).collect(),
            team_species: team("species"),
            team_level: team("level"),
            team_hp: team("stats.hp"),
            team_iv_attack: team("ivs.attack"),
            team_iv_defense: team("ivs.defense"),
            team_iv_speed: team("ivs.speed"),
            team_iv_special_attack: team("ivs.special_attack"),
            team_iv_special_defense: team("ivs.special_defense"),
            team_ev: [team("evs.hp"), team("evs.attack"), team("evs.defense"), team("evs.speed"), team("evs.special_attack"), team("evs.special_defense")],
            player_moves: (0..4).map(|i| format!("player.team.0.moves.{}.move", i)).collect(),
            stat_exp: ["hp", "attack", "defense", "speed", "special_attack", "special_defense"].iter().map(|s| format!("player.team.0.evs.{}", s)).collect(),
            gametime_seconds: "game_time.seconds".into(),
            trainer_battle_flag: opt("battle.mode"),
            battle_outcome: opt("battle.outcome"),
            battle_flags: "battle.other.outcome_flags".into(),
            battle_player_mon_party_pos: "battle.player.party_position".into(),
            battle_player_mon_hp: "battle.player.active_pokemon.stats.hp".into(),
            battle_player_mon_exp: "battle.player.team.0.exp".into(),
            battle_ally_mon_party_pos: opt("battle.player.party_position_2"),
            battle_ally_mon_hp: opt("battle.player.active_pokemon_2.stats.hp"),
            battle_ally_mon_pid: opt("battle.player.active_pokemon_2.internals.personality_value"),
            battle_trainer_a_number: "battle.opponent.id".into(),
            battle_trainer_b_number: "battle.opponent_2.id".into(),
            battle_ally_number: opt("battle.ally.id"),
            battle_first_enemy_species: "battle.opponent.active_pokemon.species".into(),
            battle_first_enemy_level: "battle.opponent.active_pokemon.level".into(),
            battle_first_enemy_hp: "battle.opponent.active_pokemon.stats.hp".into(),
            battle_first_enemy_party_pos: "battle.opponent.party_position".into(),
            battle_first_enemy_pid: if gen5 {
                "battle.opponent.team.0.internals.personality_value".into()
            } else {
                "battle.opponent.active_pokemon.internals.personality_value".into()
            },
            battle_second_enemy_species: opt("battle.opponent_2.active_pokemon.species"),
            battle_second_enemy_level: opt("battle.opponent_2.active_pokemon.level"),
            battle_second_enemy_hp: opt("battle.opponent_2.active_pokemon.stats.hp"),
            battle_second_enemy_party_pos: opt("battle.opponent_2.party_position"),
            battle_second_enemy_pid: opt("battle.opponent_2.active_pokemon.internals.personality_value"),
            battle_enemy_1_pid: (0..6).map(|i| format!("battle.opponent.team.{}.internals.personality_value", i)).collect(),
            battle_enemy_2_pid: if gen5 { Vec::new() } else { (0..6).map(|i| format!("battle.opponent_2.team.{}.internals.personality_value", i)).collect() },
            enemy_team_species: (0..6).map(|i| format!("battle.opponent.team.{}.species", i)).collect(),
            enemy_2_team_species: if gen5 { Vec::new() } else { (0..6).map(|i| format!("battle.opponent_2.team.{}.species", i)).collect() },
            save_count: opt("meta.saves"),
            audio_sound_effect_1: opt("audio.save_sound"),
            audio_sound_effect_2: opt("audio.heal_sound"),
            item_type: bag("items", "item", items_n),
            item_quantity: bag("items", "quantity", items_n),
            medicine_type: bag("medicine", "item", medicine_n),
            medicine_quantity: bag("medicine", "quantity", medicine_n),
            ball_type: bag("balls", "item", balls_n),
            ball_quantity: bag("balls", "quantity", balls_n),
            berry_type: bag("berries", "item", berries_n),
            berry_quantity: bag("berries", "quantity", berries_n),
            tmhm_type: bag("tmhm", "item", tmhm_n),
            tmhm_quantity: bag("tmhm", "quantity", tmhm_n),
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
            &self.medicine_type,
            &self.medicine_quantity,
            &self.ball_type,
            &self.ball_quantity,
            &self.berry_type,
            &self.berry_quantity,
            &self.tmhm_type,
            &self.tmhm_quantity,
        ] {
            set.extend(list.iter().cloned());
        }
        self.all_item_fields = set;
    }

    pub fn all_keys_to_register(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        let mut push = |k: Option<&String>| {
            if let Some(k) = k {
                v.push(k.clone());
            }
        };
        push(Some(&self.overworld_map));
        push(Some(&self.player_id));
        push(Some(&self.player_money));
        push(Some(&self.mon_exppoints));
        push(Some(&self.mon_level));
        push(Some(&self.mon_species));
        push(Some(&self.mon_held_item));
        push(Some(&self.mon_pid));
        push(Some(&self.gametime_seconds));
        push(self.trainer_battle_flag.as_ref());
        push(self.battle_outcome.as_ref());
        push(Some(&self.battle_flags));
        push(Some(&self.battle_trainer_a_number));
        push(Some(&self.battle_trainer_b_number));
        push(self.battle_ally_number.as_ref());
        push(Some(&self.battle_player_mon_hp));
        push(Some(&self.battle_player_mon_party_pos));
        push(Some(&self.battle_player_mon_exp));
        push(self.battle_ally_mon_hp.as_ref());
        push(self.battle_ally_mon_party_pos.as_ref());
        push(self.battle_ally_mon_pid.as_ref());
        push(Some(&self.battle_first_enemy_species));
        push(Some(&self.battle_first_enemy_level));
        push(Some(&self.battle_first_enemy_hp));
        push(Some(&self.battle_first_enemy_party_pos));
        push(Some(&self.battle_first_enemy_pid));
        push(self.battle_second_enemy_species.as_ref());
        push(self.battle_second_enemy_level.as_ref());
        push(self.battle_second_enemy_hp.as_ref());
        push(self.battle_second_enemy_party_pos.as_ref());
        push(self.battle_second_enemy_pid.as_ref());
        push(self.audio_sound_effect_1.as_ref());
        push(self.audio_sound_effect_2.as_ref());
        push(self.save_count.as_ref());
        let mut v = v;
        v.extend(self.player_moves.iter().cloned());
        v.extend(self.stat_exp.iter().cloned());
        v.extend(self.all_item_fields.iter().cloned());
        v.extend(self.team_species.iter().cloned());
        if !self.flavor.is_gen5() {
            v.extend(self.team_held_item.iter().cloned());
        }
        v.push(self.battle_solo_hp.clone());
        v.extend(self.battle_team_hp.iter().cloned());
        v.push(self.meta_state.clone());
        v
    }

    pub fn fix_case(&mut self, store: &PropertyStore) -> Vec<String> {
        let mut invalid = Vec::new();
        for k in [
            &mut self.meta_state,
            &mut self.overworld_map,
            &mut self.player_id,
            &mut self.player_money,
            &mut self.mon_exppoints,
            &mut self.mon_level,
            &mut self.mon_species,
            &mut self.mon_held_item,
            &mut self.mon_friendship,
            &mut self.mon_pid,
            &mut self.battle_solo_hp,
            &mut self.gametime_seconds,
            &mut self.battle_flags,
            &mut self.battle_player_mon_party_pos,
            &mut self.battle_player_mon_hp,
            &mut self.battle_player_mon_exp,
            &mut self.battle_trainer_a_number,
            &mut self.battle_trainer_b_number,
            &mut self.battle_first_enemy_species,
            &mut self.battle_first_enemy_level,
            &mut self.battle_first_enemy_hp,
            &mut self.battle_first_enemy_party_pos,
            &mut self.battle_first_enemy_pid,
        ] {
            fix_key(store, k, &mut invalid);
        }
        for k in [
            &mut self.trainer_battle_flag,
            &mut self.battle_outcome,
            &mut self.battle_ally_mon_party_pos,
            &mut self.battle_ally_mon_hp,
            &mut self.battle_ally_mon_pid,
            &mut self.battle_ally_number,
            &mut self.battle_second_enemy_species,
            &mut self.battle_second_enemy_level,
            &mut self.battle_second_enemy_hp,
            &mut self.battle_second_enemy_party_pos,
            &mut self.battle_second_enemy_pid,
            &mut self.save_count,
            &mut self.audio_sound_effect_1,
            &mut self.audio_sound_effect_2,
        ] {
            fix_opt_key(store, k, &mut invalid);
        }
        for list in [
            &mut self.team_held_item,
            &mut self.battle_team_hp,
            &mut self.team_species,
            &mut self.team_level,
            &mut self.team_hp,
            &mut self.team_iv_attack,
            &mut self.team_iv_defense,
            &mut self.team_iv_speed,
            &mut self.team_iv_special_attack,
            &mut self.team_iv_special_defense,
            &mut self.player_moves,
            &mut self.stat_exp,
            &mut self.battle_enemy_1_pid,
            &mut self.battle_enemy_2_pid,
            &mut self.enemy_team_species,
            &mut self.enemy_2_team_species,
            &mut self.item_type,
            &mut self.item_quantity,
            &mut self.medicine_type,
            &mut self.medicine_quantity,
            &mut self.ball_type,
            &mut self.ball_quantity,
            &mut self.berry_type,
            &mut self.berry_quantity,
            &mut self.tmhm_type,
            &mut self.tmhm_quantity,
        ] {
            fix_keys(store, list, &mut invalid);
        }
        for list in self.team_ev.iter_mut() {
            fix_keys(store, list, &mut invalid);
        }
        self.rebuild_item_fields();
        dedupe(invalid)
    }
}

#[derive(Clone, Debug)]
pub struct Gen45Converter {
    pub flavor: Flavor,
}

impl Gen45Converter {
    pub fn is_game_vitamin(&self, item_name: &str) -> bool {
        let s = sanitize_string(item_name);
        let vit = ["HP Up", "Protein", "Iron", "Carbos", "Calcium", "Zinc"];
        let berries = ["Pomeg Berry", "Kelpsy Berry", "Qualot Berry", "Hondew Berry", "Grepa Berry", "Tamato Berry"];
        vit.iter().chain(berries.iter()).any(|v| sanitize_string(v) == s)
    }

    pub fn is_game_rare_candy(&self, item_name: &str) -> bool {
        sanitize_string(item_name) == sanitize_string("Rare Candy")
    }

    pub fn is_game_tm(&self, item_name: &str) -> bool {
        item_name.starts_with("TM")
    }

    const TUTOR_MOVES: [&'static str; 42] = [
        "Blast Burn", "Draco Meteor", "Frenzy Plant", "Hydro Cannon", "Air Cutter", "Dive", "Fire Punch", "Fury Cutter", "Ice Punch", "Icy Wind", "Knock Off",
        "Ominous Wind", "Sucker Punch", "ThunderPunch", "Trick", "Vacuum Wave", "Zen Headbutt", "Helping Hand", "Last Resort", "Magnet Rise", "Snore", "Spite",
        "Swift", "Synthesis", "Uproar", "AncientPower", "Aqua Tail", "Bounce", "Earth Power", "Endeavor", "Gastro Acid", "Gunk Shot", "Heat Wave",
        "Iron Defense", "Iron Head", "Mud-Slap", "Outrage", "Rollout", "Seed Bomb", "Signal Beam", "Superpower", "Twister",
    ];

    pub fn is_tutor_move(&self, gh_move_name: &str) -> bool {
        Self::TUTOR_MOVES.contains(&name_prettify(gh_move_name).as_str())
    }

    pub fn get_hm_name(&self, gh_move_name: &str) -> Option<&'static str> {
        let p = name_prettify(gh_move_name);
        if self.flavor.is_gen5() {
            return match p.as_str() {
                "Cut" => Some("HM01 Cut"),
                "Fly" => Some("HM02 Fly"),
                "Surf" => Some("HM03 Surf"),
                "Strength" => Some("HM04 Strength"),
                "Waterfall" => Some("HM05 Waterfall"),
                "Dive" => Some("HM06 Dive"),
                _ => None,
            };
        }
        match p.as_str() {
            "Cut" => Some("HM01 Cut"),
            "Fly" => Some("HM02 Fly"),
            "Surf" => Some("HM03 Surf"),
            "Strength" => Some("HM04 Strength"),
            "Defog" => {
                if self.flavor.is_hgss() {
                    None
                } else {
                    Some("HM05 Defog")
                }
            }
            "Whirlpool" => {
                if self.flavor.is_hgss() {
                    Some("HM05 Whirlpool")
                } else {
                    None
                }
            }
            "Rock Smash" => Some("HM06 Rock Smash"),
            "Waterfall" => Some("HM07 Waterfall"),
            "Rock Climb" => Some("HM08 Rock Climb"),
            _ => None,
        }
    }

    pub fn item_name_convert(&self, gh_item_name: Option<&str>) -> Option<String> {
        let name = gh_item_name?;
        if name.starts_with("TM") || name.starts_with("HM") {
            return Some(name.to_string());
        }
        let converted = name_prettify(&name.replace('é', "e"));
        let gen5 = self.flavor.is_gen5();
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
                "Dowsing Mchn" if !gen5 => "Dowsing MCHN",
                "Gb Sounds" if !gen5 => "GB Sounds",
                "X Defense" if gen5 => "X Defend",
                other => other,
            }
            .to_string(),
        )
    }

    pub fn move_name_convert(&self, gh_move_name: Option<&str>) -> Option<String> {
        let name = gh_move_name?;
        Some(convert_move_name_gen2plus(&name_prettify(&name.replace('-', " "))))
    }

    pub fn pkmn_name_convert(&self, gh_pkmn_name: Option<&str>, held_item_gh_value: Option<&str>) -> Option<String> {
        let name = gh_pkmn_name?;
        Some(match name {
            "Mr. Mime" => "MrMime".to_string(),
            "Farfetch'd" => "FarfetchD".to_string(),
            "Mime Jr" => "Mime Jr.".to_string(),
            "Ho-oh" => "HoOh".to_string(),
            "Giratina" if !self.flavor.is_gen5() => {
                // the mapper collapses both forms: Griseous Orb forces Origin
                if self.item_name_convert(held_item_gh_value).as_deref() == Some("Griseous Orb") {
                    "Giratina (Origin)".to_string()
                } else {
                    "Giratina (Altered)".to_string()
                }
            }
            other => other.to_string(),
        })
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
    /// `'Trainer'` / `'Wild'` / None
    is_trainer_battle: Option<String>,
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
    multi_battle: bool,
    ally_id: Value,
    initial_money: i64,
    init_held_item: Value,
    original_level: i64,
    solo_hp_zero: bool,
    team_hp_zero: bool,
    watching_for_map_change: bool,
    initial_map: Value,
    solo_mon_pid: Value,
    current_ally_pid: Value,
    ally_hp_zero: bool,
    enemy_pids_in_battle: Vec<Value>,
    enemy_pid_to_participating_player_pids: IndexMap<String, (Value, HashSet<String>)>,
    enemy_pid_to_exp_split: IndexMap<String, i64>,
    trainer_1: Value,
    trainer_2: Value,
}

impl Default for BattleData {
    fn default() -> Self {
        BattleData {
            is_trainer_battle: None,
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
            multi_battle: false,
            ally_id: Value::Null,
            initial_money: 0,
            init_held_item: Value::Null,
            original_level: 0,
            solo_hp_zero: false,
            team_hp_zero: false,
            watching_for_map_change: false,
            initial_map: Value::Null,
            solo_mon_pid: Value::Null,
            current_ally_pid: Value::Null,
            ally_hp_zero: false,
            enemy_pids_in_battle: Vec::new(),
            enemy_pid_to_participating_player_pids: IndexMap::new(),
            enemy_pid_to_exp_split: IndexMap::new(),
            trainer_1: Value::Null,
            trainer_2: Value::Null,
        }
    }
}

#[derive(Default)]
struct InventoryData {
    seconds_delay: i64,
    money_gained: bool,
    money_lost: bool,
    money_change_amount: Option<i64>,
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
    save_detected: bool,
    save_delay: i64,
    heal_detected: bool,
    heal_delay: i64,
    waiting_for_new_file: bool,
    new_file_delay: i64,
    wrong_mon_delay: i64,
    waiting_for_solo_mon_in_slot_1: bool,
    wrong_mon_in_slot_1: bool,
}

/// Machine-level blackout tracking (passed from BATTLE to OVERWORLD).
#[derive(Default)]
struct BlackoutData {
    potential_flag: bool,
    all_team_fainted: bool,
    cached_first_mon_species: String,
    cached_first_mon_level: i64,
    cached_second_mon_species: String,
    cached_second_mon_level: i64,
    defeated_trainer_mons: Vec<EventDefinition>,
    trainer_name: Value,
    initial_map: Value,
}

const BASE_DELAY: i64 = 2;
const BATTLE_DELAY: i64 = 3;
const ERROR_DELAY: i64 = 5;
const SAVE_DELAY: i64 = 2;
const HEAL_DELAY: i64 = 3;
const EV_NAMES: [&str; 6] = ["hp", "attack", "defense", "speed", "special_attack", "special_defense"];

pub struct Gen45Machine {
    controller: Arc<RecorderController>,
    gen: Arc<GenData>,
    keys: Gen45Keys,
    conv: Gen45Converter,
    flavor: Flavor,
    debug_mode: bool,

    player_id: Option<Value>,
    valid_solo_mon: bool,
    cached_team: Vec<MonKey>,
    level_up_moves: HashMap<(String, i64), Vec<String>>,
    cached_items: ItemCache,
    cached_moves: [Option<String>; 4],
    cached_money: i64,
    cached_evs: HashMap<String, Value>,
    blackout: BlackoutData,
    pending_evolution_level_check: Option<i64>,
    pre_evolution_moves: Option<HashSet<String>>,
    pre_evolution_move_list: Option<Vec<Option<String>>>,
    solo_mon_key: MonKey,

    cur_state: GameState,
    active: ActiveFlag,
    queue: Arc<EventQueue>,

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

fn pid_key(v: &Value) -> String {
    v.to_string()
}

impl Gen45Machine {
    pub fn new(controller: Arc<RecorderController>, info: &StartInfo, flavor: Flavor) -> Gen45Machine {
        Gen45Machine {
            controller,
            gen: info.gen.clone(),
            keys: Gen45Keys::configure(flavor),
            conv: Gen45Converter { flavor },
            flavor,
            debug_mode: info.debug_mode,
            player_id: None,
            valid_solo_mon: false,
            cached_team: Vec::new(),
            level_up_moves: HashMap::new(),
            cached_items: IndexMap::new(),
            cached_moves: [None, None, None, None],
            cached_money: 0,
            cached_evs: HashMap::new(),
            blackout: BlackoutData::default(),
            pending_evolution_level_check: None,
            pre_evolution_moves: None,
            pre_evolution_move_list: None,
            solo_mon_key: mon_key_from(info.dvs.as_ref(), info.solo_species.clone()),
            cur_state: GameState::Uninitialized,
            active: new_active_flag(),
            queue: EventQueue::new(),
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

    fn convert_species(&self, store: &PropertyStore, species: Option<&str>, held_key: Option<&str>) -> Option<String> {
        if self.flavor.is_gen5() {
            self.conv.pkmn_name_convert(species, None)
        } else {
            let held = held_key.and_then(|k| store.str_of(k));
            self.conv.pkmn_name_convert(species, held.as_deref())
        }
    }

    fn mon_key(&self, store: &PropertyStore, mon_idx: usize) -> MonKey {
        let species = self.convert_species(store, store.str_of(&self.keys.team_species[mon_idx]).as_deref(), Some(&self.keys.team_held_item[mon_idx]));
        MonKey {
            species,
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
        // moves learned during an evolution that happened while already in the overworld
        if self.pending_evolution_level_check.is_some() && generate_events && self.controller.get_game_state() == Some(GameState::Overworld) {
            self.move_cache_update(store, false, None, false, false, false);
            let pre_moves = self.pre_evolution_moves.clone().unwrap_or_default();
            let level = self.pending_evolution_level_check.unwrap();
            self.solo_mon_levelup(level, Some(&pre_moves));
            self.pending_evolution_level_check = None;
            self.pre_evolution_moves = None;
            self.pre_evolution_move_list = None;
        }
        log::info!("after team cache update, valid_solo_mon: {}", self.valid_solo_mon);
        self.cached_team = new_cache;
    }

    fn update_all_cached_info(&mut self, store: &PropertyStore) {
        self.update_team_cache(store, false, true);
        self.item_cache_update(store, false, false, false, None, false, false, false, false);
        self.money_cache_update(store);
        let area = py_str(&store.value(&self.keys.overworld_map));
        self.controller.entered_new_area(&area);
    }

    fn solo_mon_levelup(&mut self, new_level: i64, pre_evolution_moves: Option<&HashSet<String>>) {
        let species = self.solo_mon_key.species.clone().unwrap_or_default();
        log::info!("levelup detected. {} leveling up to {}", species, new_level);
        let current_moves: HashSet<String> = self.cached_moves.iter().flatten().cloned().collect();
        let empty = HashSet::new();
        let pre_moves = pre_evolution_moves.unwrap_or(&empty);
        let current_move_list: Vec<Option<String>> = self.cached_moves.to_vec();
        let moves = self.level_up_moves.get(&(species.clone(), new_level)).cloned().unwrap_or_default();
        for move_name in moves {
            if current_moves.contains(&move_name) {
                if !pre_moves.contains(&move_name) {
                    log::info!("move {} was learned during evolution/level-up, creating learn event", move_name);
                    let mut replaced_move: Option<String> = None;
                    if let Some(pre_move_list) = &self.pre_evolution_move_list {
                        if !pre_move_list.is_empty() {
                            for idx in 0..4 {
                                if idx < pre_move_list.len() && idx < current_move_list.len() {
                                    let pre_move = &pre_move_list[idx];
                                    let post_move = &current_move_list[idx];
                                    if pre_move != post_move && post_move.as_deref() == Some(move_name.as_str()) {
                                        replaced_move = pre_move.clone();
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    let mut lm = LearnMoveEventDefinition::new(Some(&move_name), None, consts::MOVE_SOURCE_LEVELUP, LevelVal::Int(new_level), Some(&species), false);
                    lm.destination_name = replaced_move;
                    self.queue_new_event(EventDefinition::with_learn_move(lm));
                } else {
                    log::info!("move {} was already learned before evolution, skipping", move_name);
                }
            } else {
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
    }

    fn trigger_evolution(&mut self, new_mon_key: MonKey) {
        log::info!("Evolving into: {}", new_mon_key.species.as_deref().unwrap_or("None"));
        self.pre_evolution_moves = Some(self.cached_moves.iter().flatten().cloned().collect());
        self.pre_evolution_move_list = Some(self.cached_moves.to_vec());
        self.solo_mon_key = new_mon_key.clone();
        self.load_level_up_moves();
        // moves are checked when back in the overworld (party data may lag)
        self.pending_evolution_level_check = Some(new_mon_key.level_i64());
        self.queue_new_event(EventDefinition::with_evolution(new_mon_key.species.as_deref().unwrap_or("None")));
    }

    fn money_cache_update(&mut self, store: &PropertyStore) -> Option<i64> {
        let new_cache = store_i64(store, &self.keys.player_money);
        if new_cache == self.cached_money {
            return None;
        }
        let change = new_cache - self.cached_money;
        self.cached_money = new_cache;
        Some(change)
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
                // skip any slot the loaded mapper does not expose
                if !store.contains(t) || !store.contains(q) {
                    log::debug!("Skipping unmapped item slot: {}", t);
                    continue;
                }
                result.insert(store.value(t), store_i64(store, q));
            }
        }
        load(&mut result, store, &self.keys.item_type, &self.keys.item_quantity);
        load(&mut result, store, &self.keys.medicine_type, &self.keys.medicine_quantity);
        load(&mut result, store, &self.keys.ball_type, &self.keys.ball_quantity);
        load(&mut result, store, &self.keys.berry_type, &self.keys.berry_quantity);
        load(&mut result, store, &self.keys.tmhm_type, &self.keys.tmhm_quantity);
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn item_cache_update(
        &mut self,
        store: &PropertyStore,
        generate_events: bool,
        purchase_expected: bool,
        sale_expected: bool,
        money_change_amount: Option<i64>,
        vitamin_flag: bool,
        candy_flag: bool,
        tm_flag: bool,
        held_item_changed: bool,
    ) -> bool {
        let new_cache = self.item_cache(store);
        let old_cache = std::mem::replace(&mut self.cached_items, new_cache.clone());
        log::info!("_item_cache_update: old_cache = {}, new_cache = {}", repr_items_i64(&old_cache), repr_items_i64(&new_cache));
        if !generate_events {
            return false;
        }
        let mut expected_event_generated = false;
        let (gained_items, lost_items) = item_diff(&old_cache, &new_cache);
        log::info!("_item_cache_update: gained_items = {}, lost_items = {}", repr_items(&gained_items), repr_items(&lost_items));
        if !gained_items.is_empty() && sale_expected {
            log::error!("Gained the following items when expecting to be losing items to selling... {}", repr_items(&gained_items));
        }
        // a 10+ Poke/Great/Ultra Ball purchase grants a free Premier Ball
        let mut free_premier_ball_eligible = false;
        if purchase_expected && !held_item_changed {
            for (cur_gained_item, cur_gain_num) in &gained_items {
                let app_item_name = self.conv.item_name_convert(cur_gained_item.as_str()).unwrap_or_else(|| "None".into());
                if *cur_gain_num >= 10 && ["Poke Ball", "Great Ball", "Ultra Ball"].contains(&app_item_name.as_str()) {
                    free_premier_ball_eligible = true;
                    break;
                }
            }
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
                if app_item_name == "Premier Ball" && free_premier_ball_eligible && purchase_expected {
                    log::info!("Skipping purchase event for Premier Ball (will be added as free acquire)");
                    continue;
                }
                let mut custom_price = None;
                if purchase_expected {
                    let current_map = store.str_of(&self.keys.overworld_map).unwrap_or_default();
                    if current_map == GOLDENROD_LOTTERY_MAP {
                        let mut item_match_name = app_item_name.clone();
                        if app_item_name.starts_with("TM") || app_item_name.starts_with("HM") {
                            let mut item_obj = self.gen.item_db().get_item(&app_item_name).cloned();
                            if item_obj.is_none() {
                                if let Some(full) = resolve_tm_name(&self.gen, &app_item_name) {
                                    item_obj = self.gen.item_db().get_item(&full).cloned();
                                }
                            }
                            if let Some(mv) = item_obj.and_then(|i| i.move_name.clone()) {
                                item_match_name = mv;
                            }
                        }
                        if GOLDENROD_LOTTERY_ITEMS.contains(&item_match_name.as_str()) {
                            custom_price = Some(GOLDENROD_LOTTERY_PRICE);
                            log::info!("Lottery desk purchase detected: {} at ${}", app_item_name, GOLDENROD_LOTTERY_PRICE);
                        }
                    }
                }
                self.queue_new_event(EventDefinition::with_item(InventoryEventDefinition::new(&app_item_name, *cur_gain_num, true, purchase_expected, custom_price)));
                if purchase_expected && *cur_gain_num >= 10 && ["Poke Ball", "Great Ball", "Ultra Ball"].contains(&app_item_name.as_str()) {
                    log::info!("Adding free Premier Ball for purchasing {} {}", cur_gain_num, app_item_name);
                    self.queue_new_event(EventDefinition::with_item(InventoryEventDefinition::new("Premier Ball", 1, true, false, None)));
                }
            }
        }
        // taking the held item off: the bag gains it back and the mon holds
        // nothing; the engine's "hold nothing" event returns the held item
        // to the bag, so nothing else is recorded for the gain
        if held_item_changed && lost_items.is_empty() && !gained_items.is_empty() && store.get_value(Some(&self.keys.mon_held_item)).is_null() {
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
        // a "sale" only counts when the money change matches the sell prices
        let mut actual_is_sale = sale_expected;
        if sale_expected && money_change_amount.is_some() && !lost_items.is_empty() {
            let mut expected_money_from_sale = 0;
            for (cur_lost_item, cur_lost_num) in &lost_items {
                let raw = cur_lost_item.as_str().unwrap_or("");
                let app_item_name = self.conv.item_name_convert(cur_lost_item.as_str()).unwrap_or_else(|| "None".into());
                if vitamin_flag && self.conv.is_game_vitamin(raw) {
                    continue;
                }
                if candy_flag && self.conv.is_game_rare_candy(raw) {
                    continue;
                }
                if tm_flag && self.conv.is_game_tm(raw) {
                    continue;
                }
                if held_item_changed {
                    continue;
                }
                let mut item = self.gen.item_db().get_item(&app_item_name).cloned();
                if item.is_none() {
                    if let Some(full) = resolve_tm_name(&self.gen, &app_item_name) {
                        item = self.gen.item_db().get_item(&full).cloned();
                    }
                }
                match item {
                    Some(i) => expected_money_from_sale += i.sell_price * cur_lost_num,
                    None => log::warn!("Could not find item {} in database for sale validation", app_item_name),
                }
            }
            if expected_money_from_sale > 0 && money_change_amount != Some(expected_money_from_sale) {
                log::warn!(
                    "Money change ({}) does not match expected sell price ({}) for lost items. Treating as use/drop instead of sale.",
                    money_change_amount.unwrap_or(0),
                    expected_money_from_sale
                );
                actual_is_sale = false;
            }
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
                self.queue_new_event(EventDefinition::with_item(InventoryEventDefinition::new(&app_item_name, *cur_lost_num, false, actual_is_sale, None)));
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

    fn opt_value(&self, store: &PropertyStore, key: &Option<String>) -> Value {
        store.get_value(key.as_deref())
    }

    fn opt_i64(&self, store: &PropertyStore, key: &Option<String>) -> i64 {
        crate::gamehook::value_as_i64(&self.opt_value(store, key)).unwrap_or(0)
    }

    fn is_path(key: &Option<String>, path: &str) -> bool {
        key.as_deref() == Some(path)
    }

    fn num_enemy_trainer_pokemon(&self, store: &PropertyStore) -> usize {
        let mut result = self.keys.enemy_team_species.iter().filter(|k| xpr_core::pyjson::truthy(&store.get_value(Some(k)))).count();
        if crate::gamehook::value_as_i64(&self.battle.trainer_2).unwrap_or(0) > 0 {
            result += self.keys.enemy_2_team_species.iter().filter(|k| xpr_core::pyjson::truthy(&store.get_value(Some(k)))).count();
        }
        result
    }

    fn enemy_pos_lookup(&self, store: &PropertyStore) -> HashMap<i64, i64> {
        if !xpr_core::pyjson::truthy(&self.battle.second_trainer_name) {
            return (0..6).map(|x| (x, x)).collect();
        }
        let real_order = [0i64, 3, 1, 4, 2, 5];
        let mut result = HashMap::new();
        let mut next_pos_idx = 0;
        for cur_key_idx in real_order {
            if xpr_core::pyjson::truthy(&store.get_value(Some(&self.keys.enemy_team_species[cur_key_idx as usize]))) {
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
        let v = value.unwrap_or_else(|| self.opt_i64(store, &self.keys.battle_second_enemy_party_pos));
        self.battle.enemy_pos_lookup.get(&v).copied()
    }

    fn split_at(&mut self, pos: Option<i64>) -> Option<&mut HashSet<i64>> {
        let p = pos?;
        usize::try_from(p).ok().and_then(move |i| self.battle.exp_split.get_mut(i))
    }

    fn check_all_team_hp_zero(&self, store: &PropertyStore) -> bool {
        for hp_key in &self.keys.battle_team_hp {
            let hp = store.get_value(Some(hp_key));
            if !hp.is_null() && crate::gamehook::value_as_i64(&hp).unwrap_or(0) > 0 {
                return false;
            }
        }
        true
    }

    fn is_trainer(&self) -> bool {
        self.battle.is_trainer_battle.as_deref() == Some("Trainer")
    }

    /// `_battle_ready`
    fn battle_ready(&mut self, store: &PropertyStore) {
        if self.flavor.is_gen5() {
            if self.battle.battle_started {
                return;
            }
            if !xpr_core::pyjson::truthy(&store.get_value(Some(&self.keys.battle_first_enemy_species))) {
                return;
            }
        } else {
            let battle_mode = self.opt_value(store, &self.keys.trainer_battle_flag);
            if battle_mode.is_null() || battle_mode.as_str() == Some("null") {
                self.battle.delayed_initialization.reset();
                return;
            }
        }
        self.battle.trainer_1 = store.get_value(Some(&self.keys.battle_trainer_a_number));
        self.battle.trainer_2 = store.get_value(Some(&self.keys.battle_trainer_b_number));
        self.battle.battle_started = true;
        if !xpr_core::pyjson::truthy(&self.battle.trainer_2) {
            self.battle.is_double_battle = false;
        } else {
            self.battle.is_double_battle = true;
            if crate::gamehook::value_as_i64(&self.battle.ally_id) != Some(0) {
                self.battle.multi_battle = true;
            }
        }
        if self.flavor.is_gen5() {
            self.battle.is_trainer_battle = Some(if xpr_core::pyjson::truthy(&self.battle.trainer_1) { "Trainer" } else { "Wild" }.to_string());
        } else {
            self.battle.is_trainer_battle = self.opt_value(store, &self.keys.trainer_battle_flag).as_str().map(|s| s.to_string());
        }
        self.battle.original_level = store_i64(store, &self.keys.mon_level);
        if self.is_trainer() {
            log::info!("trainer battle found");
            self.battle.trainer_name = store.get_value(Some(&self.keys.battle_trainer_a_number));
            let b = store.get_value(Some(&self.keys.battle_trainer_b_number));
            self.battle.second_trainer_name = if b.is_null() { Value::String(String::new()) } else { b };
            let num_enemy_pokemon = self.num_enemy_trainer_pokemon(store);
            self.battle.enemy_pos_lookup = self.enemy_pos_lookup(store);
            // enemy PID tracking
            self.battle.enemy_pids_in_battle.clear();
            for k in &self.keys.battle_enemy_1_pid {
                let pid = store.get_value(Some(k));
                if xpr_core::pyjson::truthy(&pid) && crate::gamehook::value_as_i64(&pid) != Some(0) {
                    self.battle.enemy_pids_in_battle.push(pid);
                }
            }
            if crate::gamehook::value_as_i64(&self.battle.trainer_2).unwrap_or(0) > 0 {
                for k in &self.keys.battle_enemy_2_pid {
                    let pid = store.get_value(Some(k));
                    if xpr_core::pyjson::truthy(&pid) && crate::gamehook::value_as_i64(&pid) != Some(0) {
                        self.battle.enemy_pids_in_battle.push(pid);
                    }
                }
            }
            log::info!("[EXP_SPLIT] ===== BATTLE INITIALIZATION =====");
            log::info!("[EXP_SPLIT] Enemy PIDs in battle: {:?}", self.battle.enemy_pids_in_battle);
            log::info!("[EXP_SPLIT] Num enemies: {}", num_enemy_pokemon);
            log::info!("[EXP_SPLIT] Solo PID: {}", py_str(&self.battle.solo_mon_pid));
            log::info!("[EXP_SPLIT] Is double battle: {}", self.battle.is_double_battle);
            log::info!("[EXP_SPLIT] Is multi-battle: {}", self.battle.multi_battle);
            let solo_pid = self.battle.solo_mon_pid.clone();
            let pids = self.battle.enemy_pids_in_battle.clone();
            for enemy_pid in pids {
                let mut set = HashSet::new();
                set.insert(pid_key(&solo_pid));
                if self.battle.is_double_battle && !self.battle.multi_battle {
                    self.battle.current_ally_pid = self.opt_value(store, &self.keys.battle_ally_mon_pid);
                    set.insert(pid_key(&self.battle.current_ally_pid));
                }
                log::info!("[EXP_SPLIT] Enemy PID {} starts with participants: {:?}", py_str(&enemy_pid), set);
                self.battle.enemy_pid_to_participating_player_pids.insert(pid_key(&enemy_pid), (enemy_pid, set));
            }
            if self.battle.is_double_battle && !self.battle.multi_battle {
                log::info!("[EXP_SPLIT] Initial Ally PID: {}", py_str(&self.battle.current_ally_pid));
                self.battle.enemy_mon_order = vec![0, 1];
            } else {
                self.battle.enemy_mon_order = vec![0];
            }
            log::info!("[EXP_SPLIT] ===== END BATTLE INITIALIZATION =====");
            if self.battle.is_double_battle && !self.battle.multi_battle {
                let ally_mon_pos = self.opt_i64(store, &self.keys.battle_ally_mon_party_pos);
                self.battle.exp_split = (0..num_enemy_pokemon).map(|_| HashSet::from([0, ally_mon_pos])).collect();
            } else {
                self.battle.exp_split = (0..num_enemy_pokemon).map(|_| HashSet::from([0])).collect();
            }
            let mut td = TrainerEventDefinition::new(&py_str(&self.battle.trainer_name));
            td.second_trainer_name = self.battle.second_trainer_name.clone();
            if self.has_return() {
                let cur_friendship = store_i64(store, &self.keys.mon_friendship);
                td.custom_move_data = Self::return_custom_data(&[cur_friendship; 6]);
            }
            td.exp_split = self.battle.exp_split.iter().map(|x| x.len() as i64).collect();
            log::info!("exp split: {:?}", td.exp_split);
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
            let held = store.get_value(Some(&self.keys.mon_held_item));
            if held.is_null() {
                self.queue_new_event(EventDefinition::with_hold_item(HoldItemEventDefinition::new(None, true)));
            } else if held != self.battle.init_held_item {
                let stolen = self.battle.init_held_item.is_null();
                self.battle.init_held_item = held.clone();
                if stolen && self.is_trainer() {
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
                self.solo_mon_levelup(new_level, None);
                self.battle.original_level = new_level;
            }
        }
    }

    fn current_evs(&self, store: &PropertyStore) -> HashMap<String, Value> {
        EV_NAMES.iter().enumerate().map(|(i, n)| (n.to_string(), store.get_value(Some(&self.keys.team_ev[i][0])))).collect()
    }

    fn evs_changed(&self, store: &PropertyStore) -> bool {
        let current = self.current_evs(store);
        current.iter().any(|(stat, v)| match self.cached_evs.get(stat) {
            Some(c) if !c.is_null() && !v.is_null() => c != v,
            _ => false,
        })
    }

    fn update_ev_cache(&mut self, store: &PropertyStore) {
        self.cached_evs = self.current_evs(store);
    }

    // ---- states -------------------------------------------------------------------

    fn watch_for_reset(&self, store: &PropertyStore, new: &GameHookProperty) -> Option<GameState> {
        if self.player_id.is_some() {
            if new.path == self.keys.player_id && new.eq_i64(0) {
                return Some(GameState::Resetting);
            } else if new.eq_i64(0) && store.get(&self.keys.player_id).map(|p| p.eq_i64(0)).unwrap_or(false) {
                return Some(GameState::Resetting);
            }
        }
        None
    }

    fn meta_is_battle(&self, store: &PropertyStore) -> bool {
        store.get_value(Some(&self.keys.meta_state)).as_str() == Some("Battle")
    }

    fn in_battle_now(&self, store: &PropertyStore) -> bool {
        self.meta_is_battle(store) && self.opt_value(store, &self.keys.battle_outcome).is_null()
    }

    fn confirm_blackout(&mut self) {
        if xpr_core::pyjson::truthy(&self.blackout.trainer_name) {
            log::info!("[BLACKOUT DEBUG] Queueing TRAINER_LOSS_FLAG event for trainer: {}", py_str(&self.blackout.trainer_name));
            let mut ev = EventDefinition::with_trainer(TrainerEventDefinition::new(&py_str(&self.blackout.trainer_name)));
            ev.notes = trainer_loss_flag();
            self.queue_new_event(ev);
        }
        if !self.blackout.cached_first_mon_species.is_empty() || self.blackout.cached_first_mon_level != 0 {
            let (s, l) = (self.blackout.cached_first_mon_species.clone(), self.blackout.cached_first_mon_level);
            self.blackout.defeated_trainer_mons.push(EventDefinition::with_wild(WildPkmnEventDefinition::new(&s, l, 1, true)));
        }
        if !self.blackout.cached_second_mon_species.is_empty() || self.blackout.cached_second_mon_level != 0 {
            let (s, l) = (self.blackout.cached_second_mon_species.clone(), self.blackout.cached_second_mon_level);
            self.blackout.defeated_trainer_mons.push(EventDefinition::with_wild(WildPkmnEventDefinition::new(&s, l, 1, true)));
        }
        let mons = std::mem::take(&mut self.blackout.defeated_trainer_mons);
        for m in mons {
            self.queue_new_event(m);
        }
        self.queue_new_event(EventDefinition::with_blackout());
        self.clear_blackout_flags();
    }

    fn clear_blackout_flags(&mut self) {
        self.blackout = BlackoutData::default();
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
                let ally_id = self.opt_value(store, &self.keys.battle_ally_number);
                let money = store_i64(store, &self.keys.player_money);
                let held_at_start = store.get_value(Some(&self.keys.mon_held_item));
                let initial_map = store.get_value(Some(&self.keys.overworld_map));
                let solo_pid = store.get_value(Some(&self.keys.mon_pid));
                let b = &mut self.battle;
                b.defeated_trainer_mons.clear();
                b.delayed_move_updater.reset();
                b.delayed_item_updater.reset();
                b.delayed_held_item_updater.reset();
                b.delayed_levelup.reset();
                b.delayed_initialization.reset();
                b.is_trainer_battle = None;
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
                b.multi_battle = false;
                b.ally_id = ally_id;
                log::info!("ally id: {}", py_str(&b.ally_id));
                b.initial_money = money;
                // the held item as the battle starts (not as the delayed
                // initialisation fires): a Thief on the first turn must still
                // read as a change
                b.init_held_item = held_at_start;
                b.solo_hp_zero = false;
                b.team_hp_zero = false;
                b.watching_for_map_change = false;
                b.initial_map = initial_map;
                b.solo_mon_pid = solo_pid;
                b.current_ally_pid = Value::Null;
                b.ally_hp_zero = false;
                b.enemy_pids_in_battle.clear();
                b.enemy_pid_to_participating_player_pids.clear();
                b.enemy_pid_to_exp_split.clear();
                b.trainer_1 = Value::Null;
                b.trainer_2 = Value::Null;
                b.delayed_initialization.begin_waiting(true);
            }
            GameState::InventoryChange => {
                self.inventory.seconds_delay = BASE_DELAY;
                match self.money_cache_update(store) {
                    Some(change) => {
                        self.inventory.money_change_amount = Some(change);
                        self.inventory.money_gained = change > 0;
                        self.inventory.money_lost = change < 0;
                    }
                    None => {
                        self.inventory.money_change_amount = None;
                        self.inventory.money_gained = false;
                        self.inventory.money_lost = false;
                    }
                }
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
                log::info!("Vitamin state entered. Machine's cached EVs (from before change): {:?}", self.cached_evs);
            }
            GameState::Overworld => {
                self.money_cache_update(store);
                self.update_team_cache(store, true, false);
                if let Some(level) = self.pending_evolution_level_check {
                    self.move_cache_update(store, false, None, false, false, false);
                    let pre_moves = self.pre_evolution_moves.clone().unwrap_or_default();
                    self.solo_mon_levelup(level, Some(&pre_moves));
                    self.pending_evolution_level_check = None;
                    self.pre_evolution_moves = None;
                    self.pre_evolution_move_list = None;
                }
                let species_raw = store.get_value(Some(&self.keys.mon_species));
                let species_conv = self.convert_species(store, species_raw.as_str(), Some(&self.keys.mon_held_item));
                let save_count = self.opt_value(store, &self.keys.save_count);
                let o = &mut self.overworld;
                o.waiting_for_registration = false;
                o.register_delay = BASE_DELAY;
                o.waiting_for_new_file = false;
                o.new_file_delay = BASE_DELAY;
                o.wrong_mon_delay = BASE_DELAY;
                if species_raw.is_null() {
                    o.waiting_for_solo_mon_in_slot_1 = true;
                    o.wrong_mon_in_slot_1 = false;
                } else if species_conv != self.solo_mon_key.species {
                    o.waiting_for_solo_mon_in_slot_1 = false;
                    o.wrong_mon_in_slot_1 = true;
                } else {
                    o.waiting_for_solo_mon_in_slot_1 = false;
                    o.wrong_mon_in_slot_1 = false;
                }
                o.validation_delay = 5;
                o.save_detected = false;
                o.save_delay = SAVE_DELAY;
                o.heal_detected = false;
                o.heal_delay = HEAL_DELAY;
                let _ = save_count;
                if self.blackout.potential_flag {
                    let current_map = store.get_value(Some(&self.keys.overworld_map));
                    log::info!("[BLACKOUT DEBUG] Entered OVERWORLD with blackout flag set. Current map: {}, Initial map: {}", py_str(&current_map), py_str(&self.blackout.initial_map));
                    if !self.blackout.initial_map.is_null() && current_map != self.blackout.initial_map {
                        let mut all_team_hp_zero = true;
                        for hp_key in &self.keys.team_hp {
                            let hp = store.get_value(Some(hp_key));
                            if !hp.is_null() && crate::gamehook::value_as_i64(&hp).unwrap_or(0) > 0 {
                                all_team_hp_zero = false;
                                break;
                            }
                        }
                        if all_team_hp_zero || self.blackout.all_team_fainted {
                            log::info!("[BLACKOUT DEBUG] Entered OVERWORLD with blackout flag set, map already changed and team HP is 0 - blackout confirmed");
                            self.confirm_blackout();
                        } else {
                            log::info!("Entered OVERWORLD with blackout flag set, but team HP is not 0 - not a blackout, clearing flag");
                            self.clear_blackout_flags();
                        }
                    }
                }
                self.update_ev_cache(store);
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
                    if next == GameState::Overworld && self.battle.solo_hp_zero {
                        log::info!("[BLACKOUT DEBUG] Passing blackout detection to OVERWORLD state");
                        self.blackout.potential_flag = true;
                        self.blackout.all_team_fainted = self.battle.team_hp_zero;
                        self.blackout.cached_first_mon_species = self.battle.cached_first_mon_species.clone();
                        self.blackout.cached_first_mon_level = self.battle.cached_first_mon_level;
                        self.blackout.cached_second_mon_species = self.battle.cached_second_mon_species.clone();
                        self.blackout.cached_second_mon_level = self.battle.cached_second_mon_level;
                        self.blackout.defeated_trainer_mons = self.battle.defeated_trainer_mons.clone();
                        self.blackout.trainer_name = self.battle.trainer_name.clone();
                        self.blackout.initial_map = self.battle.initial_map.clone();
                    }
                    // a steal on the final turn must reach the trainer
                    // event, so the held-item check runs before it is built
                    if self.battle.delayed_held_item_updater.trigger(false) {
                        self.delayed_held_item_update(store);
                    }
                    if self.is_trainer() {
                        log::info!("[EXP_SPLIT] ===== BATTLE EXIT =====");
                        let mut final_exp_split: Vec<i64> = Vec::new();
                        for enemy_pid in &self.battle.enemy_pids_in_battle {
                            match self.battle.enemy_pid_to_exp_split.get(&pid_key(enemy_pid)) {
                                Some(n) => final_exp_split.push(*n),
                                None => final_exp_split.push(1),
                            }
                        }
                        log::info!("[EXP_SPLIT] Final exp_split in team order: {:?}", final_exp_split);
                        if !final_exp_split.iter().any(|x| *x > 1) {
                            final_exp_split = Vec::new();
                        }
                        // two-trainer battles: interleaving would break a custom mon order
                        let final_mon_order: Vec<i64> = if xpr_core::pyjson::truthy(&self.battle.second_trainer_name) {
                            Vec::new()
                        } else {
                            let mut sorted = self.battle.enemy_mon_order.clone();
                            sorted.sort();
                            sorted.iter().map(|x| self.battle.enemy_mon_order.iter().position(|y| y == x).unwrap_or(0) as i64 + 1).collect()
                        };
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
                    if self.battle.delayed_move_updater.trigger(false) {
                        self.delayed_moves_update(store);
                    }
                    if self.battle.delayed_item_updater.trigger(false) {
                        self.item_cache_update(store, true, false, false, None, false, false, false, false);
                    }
                    if self.battle.delayed_levelup.trigger(false) {
                        self.delayed_level_update(store);
                    }
                }
            }
            GameState::InventoryChange => {
                if next != GameState::Resetting && next != GameState::RareCandy && next != GameState::Vitamin {
                    let evs_changed = self.evs_changed(store);
                    log::info!("InventoryChangeState._on_exit: cached EVs = {:?}, current EVs = {:?}, changed = {}", self.cached_evs, self.current_evs(store), evs_changed);
                    let (gained, lost, amount, held) = (self.inventory.money_gained, self.inventory.money_lost, self.inventory.money_change_amount, self.inventory.held_item_changed);
                    self.item_cache_update(store, true, lost, gained, amount, evs_changed, false, false, held);
                }
            }
            GameState::RareCandy => {
                if next != GameState::Resetting {
                    if self.item_cache_update(store, true, false, false, None, false, true, false, false) {
                        let level = store_i64(store, &self.keys.mon_level);
                        self.solo_mon_levelup(level, None);
                    }
                    if self.candy.move_learned {
                        self.update_team_cache(store, true, false);
                        self.move_cache_update(store, true, None, false, false, true);
                    }
                }
            }
            GameState::Tm => {
                if next != GameState::Resetting && !self.item_cache_update(store, true, false, false, None, false, false, true, false) {
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
                    let evs_changed = self.evs_changed(store);
                    if evs_changed {
                        log::info!("EVs changed, vitamin was applied");
                    } else {
                        log::info!("EVs did not change, vitamin was dropped/sold");
                    }
                    self.item_cache_update(store, true, false, false, None, evs_changed, false, false, false);
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
                        if self.meta_is_battle(store) {
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
                    let change = new.as_i64().unwrap_or(0) - prev.as_i64().unwrap_or(0);
                    if change != 0 {
                        self.inventory.money_change_amount = Some(change);
                        self.inventory.money_gained = change > 0;
                        self.inventory.money_lost = change < 0;
                    }
                } else if self.keys.all_item_fields.contains(&new.path) {
                    self.inventory.seconds_delay = BASE_DELAY;
                } else if new.path == self.keys.mon_held_item {
                    self.inventory.held_item_changed = true;
                } else if new.path == self.keys.mon_level {
                    if let (Some(p), Some(n)) = (prev.as_i64(), new.as_i64()) {
                        if n > p {
                            return GameState::RareCandy;
                        }
                    }
                } else if self.keys.stat_exp.contains(&new.path) {
                    if self.keys.team_ev.iter().any(|l| l[0] == new.path) && !prev.is_null() && !new.is_null() && new.value != prev.value {
                        return GameState::Vitamin;
                    }
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

    fn push_defeated(&mut self, species: String, level: i64) {
        if self.is_trainer() {
            self.battle.defeated_trainer_mons.push(EventDefinition::with_wild(WildPkmnEventDefinition::new(&species, level, 1, true)));
        } else {
            log::info!("Queueing wild Pokemon event: {} level {}", species, level);
            self.queue_new_event(EventDefinition::with_wild(WildPkmnEventDefinition::new(&species, level, 1, false)));
        }
    }

    fn battle_end_check(&self, store: &PropertyStore) -> Option<GameState> {
        if !self.meta_is_battle(store) {
            if self.battle.watching_for_map_change {
                return Some(GameState::Overworld);
            }
            if self.battle.cached_first_mon_species.is_empty() && self.battle.cached_second_mon_species.is_empty() {
                return Some(GameState::Overworld);
            }
        }
        None
    }

    fn battle_transition(&mut self, store: &PropertyStore, new: &GameHookProperty, prev: &GameHookProperty) -> GameState {
        let state = GameState::Battle;
        let k = self.keys.clone();
        // gen 5: initialise the instant the battle data is available
        if self.flavor.is_gen5() && !self.battle.battle_started {
            self.battle_ready(store);
        }
        // solo HP hitting 0 starts blackout detection
        if new.path == k.battle_solo_hp {
            if new.as_i64().map(|v| v <= 0).unwrap_or(false) && !self.battle.solo_hp_zero {
                log::info!("Solo HP hit 0, starting blackout detection");
                self.battle.solo_hp_zero = true;
                if !self.battle.team_hp_zero && self.check_all_team_hp_zero(store) {
                    log::info!("All team HP already 0, watching for map change");
                    self.battle.team_hp_zero = true;
                    self.battle.watching_for_map_change = true;
                }
            }
        }
        if k.battle_team_hp.contains(&new.path) && self.battle.solo_hp_zero && !self.battle.team_hp_zero && self.check_all_team_hp_zero(store) {
            log::info!("All team HP hit 0, watching for map change");
            self.battle.team_hp_zero = true;
            self.battle.watching_for_map_change = true;
        }

        if new.path == k.mon_exppoints || new.path == k.battle_player_mon_exp {
            log::info!("[EXP_SPLIT] ===== EXP CHANGE DETECTED =====");
            let mut defeated_enemy_pid: Option<Value> = None;
            if !self.battle.cached_first_mon_species.is_empty() || self.battle.cached_first_mon_level != 0 {
                let first_enemy_pid = store.get_value(Some(&k.battle_first_enemy_pid));
                if xpr_core::pyjson::truthy(&first_enemy_pid) && self.battle.enemy_pid_to_participating_player_pids.contains_key(&pid_key(&first_enemy_pid)) {
                    defeated_enemy_pid = Some(first_enemy_pid);
                } else if xpr_core::pyjson::truthy(&first_enemy_pid) {
                    log::info!("[EXP_SPLIT] WARNING: First enemy PID {} not in tracking dictionary!", py_str(&first_enemy_pid));
                }
            }
            if defeated_enemy_pid.is_none() && (!self.battle.cached_second_mon_species.is_empty() || self.battle.cached_second_mon_level != 0) {
                let second_enemy_pid = self.opt_value(store, &k.battle_second_enemy_pid);
                if xpr_core::pyjson::truthy(&second_enemy_pid) && self.battle.enemy_pid_to_participating_player_pids.contains_key(&pid_key(&second_enemy_pid)) {
                    defeated_enemy_pid = Some(second_enemy_pid);
                } else if xpr_core::pyjson::truthy(&second_enemy_pid) {
                    log::info!("[EXP_SPLIT] WARNING: Second enemy PID {} not in tracking dictionary!", py_str(&second_enemy_pid));
                }
            }
            match defeated_enemy_pid {
                Some(pid) => {
                    let split_count = self.battle.enemy_pid_to_participating_player_pids.get(&pid_key(&pid)).map(|(_, s)| s.len() as i64).unwrap_or(0);
                    log::info!("[EXP_SPLIT] FINAL exp split for enemy PID {}: {} participants", py_str(&pid), split_count);
                    self.battle.enemy_pid_to_exp_split.insert(pid_key(&pid), split_count);
                }
                None => log::info!("[EXP_SPLIT] WARNING: Could not determine defeated enemy PID for exp split calculation"),
            }
            self.battle.ally_hp_zero = false;
            log::info!("[EXP_SPLIT] ===== END EXP CHANGE =====");
            if !self.battle.cached_first_mon_species.is_empty() || self.battle.cached_first_mon_level != 0 {
                let (s, l) = (std::mem::take(&mut self.battle.cached_first_mon_species), std::mem::take(&mut self.battle.cached_first_mon_level));
                self.push_defeated(s, l);
            } else if !self.battle.cached_second_mon_species.is_empty() || self.battle.cached_second_mon_level != 0 {
                let (s, l) = (std::mem::take(&mut self.battle.cached_second_mon_species), std::mem::take(&mut self.battle.cached_second_mon_level));
                self.push_defeated(s, l);
            } else {
                log::error!("Solo mon gained experience, but we didn't properly cache which enemy mon was defeated... This is normal if the a different pokemon has been pulled into player's slot 1");
            }
            // battle ended (but never while tracking a blackout)
            if !self.meta_is_battle(store) {
                if self.battle.solo_hp_zero {
                    return state;
                }
                return GameState::Overworld;
            }
        } else if new.path == k.battle_first_enemy_hp {
            if new.eq_i64(0) && prev.as_i64().unwrap_or(0) > 0 {
                log::info!("[EXP_SPLIT] ===== FIRST ENEMY FAINTED =====");
                if (!self.battle.cached_first_mon_species.is_empty() || self.battle.cached_first_mon_level != 0) && self.is_trainer() {
                    let (s, l) = (self.battle.cached_first_mon_species.clone(), self.battle.cached_first_mon_level);
                    self.battle.defeated_trainer_mons.push(EventDefinition::with_wild(WildPkmnEventDefinition::new(&s, l, 1, true)));
                    log::info!("[EXP_SPLIT] Adding previously cached Pokemon to defeated list before overwriting: {} level {}", s, l);
                }
                let species_raw = store.str_of(&k.battle_first_enemy_species);
                self.battle.cached_first_mon_species = self.conv.pkmn_name_convert(species_raw.as_deref(), None).unwrap_or_default();
                self.battle.cached_first_mon_level = store_i64(store, &k.battle_first_enemy_level);
                log::info!("[EXP_SPLIT] Cached first enemy: {} level {}", self.battle.cached_first_mon_species, self.battle.cached_first_mon_level);
                self.battle.friendship_data.push(store_i64(store, &k.mon_friendship));
                log::info!("[EXP_SPLIT] ===== END FIRST ENEMY FAINTED =====");
            }
        } else if Self::is_path(&k.battle_second_enemy_hp, &new.path) {
            if new.eq_i64(0) && prev.as_i64().unwrap_or(0) > 0 {
                log::info!("[EXP_SPLIT] ===== SECOND ENEMY FAINTED =====");
                if (!self.battle.cached_second_mon_species.is_empty() || self.battle.cached_second_mon_level != 0) && self.is_trainer() {
                    let (s, l) = (self.battle.cached_second_mon_species.clone(), self.battle.cached_second_mon_level);
                    self.battle.defeated_trainer_mons.push(EventDefinition::with_wild(WildPkmnEventDefinition::new(&s, l, 1, true)));
                }
                let species_raw = self.opt_value(store, &k.battle_second_enemy_species);
                self.battle.cached_second_mon_species = self.conv.pkmn_name_convert(species_raw.as_str(), None).unwrap_or_default();
                self.battle.cached_second_mon_level = self.opt_i64(store, &k.battle_second_enemy_level);
                self.battle.friendship_data.push(store_i64(store, &k.mon_friendship));
                log::info!("[EXP_SPLIT] ===== END SECOND ENEMY FAINTED =====");
            }
        } else if Self::is_path(&k.battle_ally_mon_pid, &new.path) && !self.battle.multi_battle {
            if self.battle.is_double_battle && !new.is_null() && !new.eq_i64(0) {
                let new_pid = new.value.clone();
                if new_pid != self.battle.current_ally_pid {
                    log::info!("[EXP_SPLIT] ===== ALLY PID CHANGE ===== old {} new {}", py_str(&self.battle.current_ally_pid), py_str(&new_pid));
                    self.battle.current_ally_pid = new_pid.clone();
                    self.battle.ally_hp_zero = false;
                    let first_enemy_pid = store.get_value(Some(&k.battle_first_enemy_pid));
                    let second_enemy_pid = self.opt_value(store, &k.battle_second_enemy_pid);
                    let first_enemy_hp = store_i64(store, &k.battle_first_enemy_hp);
                    let second_enemy_hp = self.opt_i64(store, &k.battle_second_enemy_hp);
                    if xpr_core::pyjson::truthy(&first_enemy_pid) && first_enemy_hp > 0 {
                        match self.battle.enemy_pid_to_participating_player_pids.get_mut(&pid_key(&first_enemy_pid)) {
                            Some((_, set)) => {
                                set.insert(pid_key(&new_pid));
                            }
                            None => log::info!("[EXP_SPLIT] WARNING: First enemy PID {} not in tracking dictionary!", py_str(&first_enemy_pid)),
                        }
                    }
                    if self.battle.is_double_battle && xpr_core::pyjson::truthy(&second_enemy_pid) && second_enemy_hp > 0 {
                        match self.battle.enemy_pid_to_participating_player_pids.get_mut(&pid_key(&second_enemy_pid)) {
                            Some((_, set)) => {
                                set.insert(pid_key(&new_pid));
                            }
                            None => log::info!("[EXP_SPLIT] WARNING: Second enemy PID {} not in tracking dictionary!", py_str(&second_enemy_pid)),
                        }
                    }
                    log::info!("[EXP_SPLIT] ===== END ALLY PID CHANGE =====");
                } else {
                    log::info!("[EXP_SPLIT] Ally PID unchanged: {}", py_str(&new_pid));
                }
            } else if new.is_null() || new.eq_i64(0) {
                log::info!("[EXP_SPLIT] Ally PID set to None/0 (value: {})", py_str(&new.value));
            }
        } else if new.path == k.battle_player_mon_hp {
            let player_mon_pos = store_i64(store, &k.battle_player_mon_party_pos);
            let hp = new.as_i64().unwrap_or(0);
            if player_mon_pos == 0 && hp <= 0 {
                if self.battle.battle_started {
                    log::info!("Player mon HP dropped to 0 or below");
                    self.battle.loss_detected = true;
                    log::info!("Loss detected: {}", self.battle.loss_detected);
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
        } else if Self::is_path(&k.battle_ally_mon_hp, &new.path) && !self.battle.multi_battle {
            if self.battle.is_double_battle && new.as_i64().map(|v| v <= 0).unwrap_or(false) && prev.as_i64().unwrap_or(0) > 0 {
                log::info!("[EXP_SPLIT] ===== ALLY FAINTED =====");
                self.battle.ally_hp_zero = true;
                if xpr_core::pyjson::truthy(&self.battle.current_ally_pid) {
                    let ally = pid_key(&self.battle.current_ally_pid);
                    for (_, (_, set)) in self.battle.enemy_pid_to_participating_player_pids.iter_mut() {
                        set.remove(&ally);
                    }
                }
                log::info!("[EXP_SPLIT] ===== END ALLY FAINTED =====");
                // legacy exp_split tracking
                let ally_mon_pos = self.opt_i64(store, &k.battle_ally_mon_party_pos);
                let first_alive = store_i64(store, &k.battle_first_enemy_hp) > 0 || !self.battle.cached_first_mon_species.is_empty();
                let pos = self.first_enemy_mon_pos(store, None);
                if let Some(set) = self.split_at(pos) {
                    if set.contains(&ally_mon_pos) && first_alive {
                        set.remove(&ally_mon_pos);
                    }
                }
                let second_alive = self.opt_i64(store, &k.battle_second_enemy_hp) > 0 || !self.battle.cached_second_mon_species.is_empty();
                let pos2 = self.second_enemy_mon_pos(store, None);
                if let Some(set) = self.split_at(pos2) {
                    if set.contains(&ally_mon_pos) && second_alive {
                        set.remove(&ally_mon_pos);
                    }
                }
            }
        } else if k.player_moves.contains(&new.path) {
            self.battle.delayed_move_updater.begin_waiting(true);
        } else if k.all_item_fields.contains(&new.path) {
            self.battle.delayed_item_updater.begin_waiting(true);
        } else if new.path == k.mon_held_item {
            self.battle.delayed_held_item_updater.begin_waiting(true);
        } else if new.path == k.mon_level {
            self.battle.delayed_levelup.begin_waiting(true);
        } else if new.path == k.gametime_seconds {
            if self.battle.delayed_move_updater.tick() {
                self.delayed_moves_update(store);
            }
            if self.battle.delayed_item_updater.tick() {
                self.item_cache_update(store, true, false, false, None, false, false, false, false);
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
        } else if new.path == k.battle_player_mon_party_pos {
            let v = new.as_i64().unwrap_or(-1);
            if (0..6).contains(&v) {
                if store_i64(store, &k.battle_first_enemy_hp) > 0 {
                    let pos = self.first_enemy_mon_pos(store, None);
                    if let Some(set) = self.split_at(pos) {
                        set.insert(v);
                    }
                }
                if self.battle.is_double_battle && self.opt_i64(store, &k.battle_second_enemy_hp) > 0 {
                    let pos2 = self.second_enemy_mon_pos(store, None);
                    if let Some(set) = self.split_at(pos2) {
                        set.insert(v);
                    }
                }
            }
        } else if Self::is_path(&k.battle_ally_mon_party_pos, &new.path) {
            let v = new.as_i64().unwrap_or(-1);
            if self.battle.is_double_battle && !self.battle.multi_battle && (0..6).contains(&v) {
                if store_i64(store, &k.battle_first_enemy_hp) > 0 {
                    let pos = self.first_enemy_mon_pos(store, None);
                    if let Some(set) = self.split_at(pos) {
                        set.insert(v);
                    }
                }
                if self.opt_i64(store, &k.battle_second_enemy_hp) > 0 {
                    let pos2 = self.second_enemy_mon_pos(store, None);
                    if let Some(set) = self.split_at(pos2) {
                        set.insert(v);
                    }
                }
            }
        } else if new.path == k.battle_first_enemy_party_pos || Self::is_path(&k.battle_second_enemy_party_pos, &new.path) {
            let is_first = new.path == k.battle_first_enemy_party_pos;
            let real_new_value = if is_first { self.first_enemy_mon_pos(store, new.as_i64()) } else { self.second_enemy_mon_pos(store, new.as_i64()) };
            log::info!("[EXP_SPLIT] ===== ENEMY SWITCHED ===== {} -> {} (real {:?})", py_str(&prev.value), py_str(&new.value), real_new_value);
            let allowed = is_first || self.battle.is_double_battle;
            if let Some(real) = real_new_value {
                if allowed && real >= 0 && (real as usize) < self.battle.exp_split.len() {
                    if store_i64(store, &k.battle_player_mon_hp) > 0 {
                        self.battle.exp_split[real as usize] = HashSet::from([store_i64(store, &k.battle_player_mon_party_pos)]);
                    }
                    let ally_check = if is_first { self.battle.is_double_battle && !self.battle.multi_battle } else { !self.battle.multi_battle };
                    if ally_check && self.opt_i64(store, &k.battle_ally_mon_hp) > 0 {
                        let ally = self.opt_i64(store, &k.battle_ally_mon_party_pos);
                        self.battle.exp_split[real as usize].insert(ally);
                    }
                    if !self.battle.enemy_mon_order.contains(&real) {
                        self.battle.enemy_mon_order.push(real);
                    }
                }
            }
        } else if new.path == k.mon_species {
            // an evolution during the battle-end transition
            self.update_team_cache(store, true, false);
            if let Some(next) = self.battle_end_check(store) {
                return next;
            }
        } else if new.path == k.meta_state {
            if !new.eq_str("Battle") {
                if self.battle.watching_for_map_change {
                    log::info!("META_STATE changed to '{}', transitioning to OVERWORLD for blackout detection", py_str(&new.value));
                    return GameState::Overworld;
                }
                if self.battle.cached_first_mon_species.is_empty() && self.battle.cached_second_mon_species.is_empty() {
                    return GameState::Overworld;
                }
            }
        }
        if new.path != k.mon_exppoints {
            if let Some(next) = self.battle_end_check(store) {
                return next;
            }
        }
        state
    }

    fn overworld_transition(&mut self, store: &PropertyStore, new: &GameHookProperty, prev: &GameHookProperty) -> GameState {
        let state = GameState::Overworld;
        let k = self.keys.clone();
        if self.overworld.waiting_for_new_file || self.overworld.waiting_for_solo_mon_in_slot_1 {
            let mut check_for_battle = false;
            if new.path == k.gametime_seconds {
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
        } else if new.path == k.overworld_map {
            let area = py_str(&store.get_value(Some(&k.overworld_map)));
            self.controller.entered_new_area(&area);
            if self.blackout.potential_flag {
                log::info!("[BLACKOUT DEBUG] Map changed, checking for blackout. Old map: {}, New map: {}", py_str(&prev.value), py_str(&new.value));
                let mut all_team_hp_zero = true;
                let mut team_hp_values = Vec::new();
                for hp_key in &k.team_hp {
                    let hp = store.get_value(Some(hp_key));
                    team_hp_values.push(hp.clone());
                    if !hp.is_null() && crate::gamehook::value_as_i64(&hp).unwrap_or(0) > 0 {
                        all_team_hp_zero = false;
                    }
                }
                log::info!("[BLACKOUT DEBUG] Team HP values: {:?}, all_zero: {}", team_hp_values, all_team_hp_zero);
                if all_team_hp_zero || self.blackout.all_team_fainted {
                    log::info!("[BLACKOUT DEBUG] Map changed and team HP is 0 - blackout confirmed");
                    self.confirm_blackout();
                } else {
                    log::info!("Map changed but team HP is not 0 - not a blackout, clearing flag");
                    self.clear_blackout_flags();
                }
            }
        } else if new.path == k.player_id {
            if prev.truthy() && self.player_id.as_ref() != Some(&store.value(&k.player_id)) {
                self.overworld.waiting_for_new_file = true;
            }
        } else if new.path == k.mon_held_item {
            self.overworld.propagate_held_item_flag = true;
        } else if k.all_item_fields.contains(&new.path) {
            return GameState::InventoryChange;
        } else if new.path == k.mon_species {
            let held_key = Some(k.mon_held_item.as_str());
            if !prev.truthy() {
                self.overworld.waiting_for_registration = true;
            } else if self.solo_mon_key.species == self.convert_species(store, prev.as_str(), held_key) {
                self.overworld.wrong_mon_in_slot_1 = true;
            } else if self.solo_mon_key.species == self.convert_species(store, new.as_str(), held_key) {
                self.overworld.wrong_mon_delay = BASE_DELAY;
                self.overworld.waiting_for_solo_mon_in_slot_1 = true;
            }
        } else if new.path == k.mon_level {
            if !self.overworld.waiting_for_registration && !self.overworld.wrong_mon_in_slot_1 {
                return GameState::RareCandy;
            }
        } else if k.player_moves.contains(&new.path) {
            if !self.overworld.waiting_for_registration && !self.overworld.wrong_mon_in_slot_1 {
                let mut all_cur_moves: Vec<Value> = Vec::new();
                for move_path in &k.player_moves {
                    if *move_path == prev.path {
                        all_cur_moves.push(prev.value.clone());
                    } else {
                        all_cur_moves.push(store.get_value(Some(move_path)));
                    }
                }
                if new.is_null() || all_cur_moves.contains(&new.value) {
                    return GameState::MoveDelete;
                } else {
                    return GameState::Tm;
                }
            }
        } else if Self::is_path(&k.save_count, &new.path) {
            if let (Some(p), Some(n)) = (prev.as_i64(), new.as_i64()) {
                if n == p + 1 && (p != 0 || n == 1) {
                    self.overworld.save_detected = true;
                    self.overworld.save_delay = SAVE_DELAY;
                }
            }
        } else if Self::is_path(&k.audio_sound_effect_1, &new.path) {
            // legacy save detection (replaced by the save count)
        } else if Self::is_path(&k.audio_sound_effect_2, &new.path) {
            if new.eq_i64(HEAL_SOUND_EFFECT_VALUE) && !prev.eq_i64(HEAL_SOUND_EFFECT_VALUE) {
                self.overworld.heal_detected = true;
                self.overworld.heal_delay = HEAL_DELAY;
            }
        } else if new.path == k.gametime_seconds {
            if self.overworld.waiting_for_registration {
                if self.overworld.register_delay <= 0 {
                    self.overworld.waiting_for_registration = false;
                    self.update_team_cache(store, true, true);
                }
                self.overworld.register_delay -= 1;
            } else if self.overworld.wrong_mon_in_slot_1 {
                if self.overworld.wrong_mon_delay <= 0 {
                    self.update_team_cache(store, true, true);
                    let cur = self.convert_species(store, store.str_of(&k.mon_species).as_deref(), Some(k.mon_held_item.as_str()));
                    self.overworld.wrong_mon_in_slot_1 = self.solo_mon_key.species != cur;
                }
                self.overworld.wrong_mon_delay -= 1;
            }
            if self.overworld.save_detected {
                if self.overworld.save_delay <= 0 {
                    self.queue_new_event(EventDefinition::with_save_value(&store.get_value(Some(&k.overworld_map))));
                    self.overworld.save_detected = false;
                    self.overworld.save_delay = SAVE_DELAY;
                } else {
                    self.overworld.save_delay -= 1;
                }
            }
            if self.overworld.heal_detected {
                if self.overworld.heal_delay <= 0 {
                    self.queue_new_event(EventDefinition::with_heal_value(&store.get_value(Some(&k.overworld_map))));
                    self.overworld.heal_detected = false;
                    self.overworld.heal_delay = HEAL_DELAY;
                } else {
                    self.overworld.heal_delay -= 1;
                }
            }
            if self.overworld.validation_delay > 0 {
                self.overworld.validation_delay -= 1;
            } else if self.overworld.validation_delay == 0 {
                self.overworld.validation_delay -= 1;
            }
            self.update_ev_cache(store);
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
        std::thread::Builder::new()
            .name("gen45-recorder-events".into())
            .spawn(move || {
                ctx.run(|ctx, ev| process_one(ctx, ev, &conv));
            })
            .ok();
    }
}

fn repr_items_i64(map: &ItemCache) -> String {
    let parts: Vec<String> = map.iter().map(|(k, v)| format!("{}: {}", py_str(k), v)).collect();
    format!("{{{}}}", parts.join(", "))
}

/// `_process_events` body (gens 4/5).
fn process_one(ctx: &ProcessCtx, mut cur_event: EventDefinition, conv: &Gen45Converter) {
    let gen = &ctx.gen;
    let gen5 = conv.flavor.is_gen5();
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
        log::info!("[BLACKOUT DEBUG] Processing trainer event with ID: {}, notes: {}", trainer_id, cur_event.notes);
        let Some(trainer) = gen.trainer_db().get_trainer_by_id(trainer_id).cloned() else {
            ctx.add_error(format!("Failed to find trainer from GameHook: (<class 'int'>) {}", trainer_id));
            return;
        };
        if let Some(t) = cur_event.trainer_def.as_mut() {
            t.trainer_name = trainer.name.clone();
        }
        log::info!("[BLACKOUT DEBUG] Converted trainer ID {} ({}) to name: {}", trainer_id, td.trainer_name, trainer.name);
        let mut second_trainer = None;
        let second_raw = td.second_trainer_name.clone();
        if xpr_core::pyjson::truthy(&second_raw) {
            if !gen5 && trainer.double_battle {
                // twins are one DB entry whose team already holds both opponents' mons
                log::info!("First trainer {} is a merged double-battle entry; ignoring second trainer slot {}", trainer.name, py_str(&second_raw));
                if let Some(t) = cur_event.trainer_def.as_mut() {
                    t.second_trainer_name = Value::String(String::new());
                }
            } else {
                let second_id = crate::gamehook::value_as_i64(&second_raw).or_else(|| second_raw.as_str().and_then(|s| s.trim().parse().ok()));
                let second = second_id.and_then(|i| gen.trainer_db().get_trainer_by_id(i).cloned());
                let Some(st) = second else {
                    ctx.add_error(format!("Failed to find second trainer from GameHook: (<class 'int'>) {}", py_str(&second_raw)));
                    return;
                };
                if let Some(t) = cur_event.trainer_def.as_mut() {
                    t.second_trainer_name = Value::String(st.name.clone());
                }
                second_trainer = Some(st);
            }
        }
        let trainer_name = trainer.name.clone();
        if cur_event.notes == trainer_loss_flag() {
            log::info!("[BLACKOUT DEBUG] Processing TRAINER_LOSS_FLAG event for trainer: {}", trainer_name);
            ctx.controller.lost_trainer_battle(&trainer_name);
            return;
        } else if cur_event.notes == roar_flag() {
            log::info!("[BLACKOUT DEBUG] Processing ROAR_FLAG event for trainer: {}", trainer_name);
            log::info!("Updating full trainer event: {}", event_str(gen, &cur_event));
            log::info!("Updating split exp for trainer {} to {:?}", trainer_name, td.exp_split);
            let name = trainer_name.clone();
            let mut test_obj = ctx.controller.host().call(|h| h.get_previous_event(None));
            let mut search_count = 0;
            while !RecorderController::is_trainer_event(test_obj.as_ref(), &name) {
                let Some(obj) = &test_obj else { break };
                let id = obj.group_id;
                test_obj = ctx.controller.host().call(move |h| h.get_previous_event(Some(id)));
                search_count += 1;
                if search_count > 20 {
                    log::error!("[BLACKOUT DEBUG] ROAR_FLAG: Search limit reached");
                    break;
                }
            }
            match test_obj {
                None => log::error!("[BLACKOUT DEBUG] ROAR_FLAG: Failed to find trainer fight to update for exp split behavior"),
                Some(obj) => {
                    cur_event.notes = String::new();
                    log::info!("held item: {}", obj.final_held_item.as_deref().unwrap_or("None"));
                    let amulet = obj.final_held_item.as_deref() == Some(consts::AMULET_COIN_ITEM_NAME);
                    let mut expected_money = trainer.money;
                    if amulet {
                        expected_money *= 2;
                    }
                    if let Some(st) = &second_trainer {
                        let mut second_money = st.money;
                        if amulet {
                            second_money *= 2;
                        }
                        expected_money += second_money;
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
                ctx.controller.check_final_trainer(&trainer_name);
            }
            return;
        }
    } else if let Some(item_def) = cur_event.item_event_def.clone() {
        log::info!("getting item from {}", item_def.item_name);
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

impl GameRecorder for Gen45Machine {
    fn on_mapper_loaded(&mut self, store: &PropertyStore) -> (Vec<String>, Vec<String>) {
        self.keys = Gen45Keys::configure(self.flavor);
        let invalid = self.keys.fix_case(store);
        (self.keys.all_keys_to_register(), invalid)
    }

    fn is_active(&self) -> bool {
        is_set(&self.active)
    }

    fn startup(&mut self, store: &PropertyStore) {
        self.active.store(true, Ordering::SeqCst);
        self.cur_state = GameState::Uninitialized;
        self.on_enter(GameState::Uninitialized, store);
        self.controller.set_game_state(self.cur_state);
        crate::shuckie::supershuckie().start();
        self.spawn_processing_thread();
    }

    fn handle_event(&mut self, store: &PropertyStore, new: &GameHookProperty, prev: &GameHookProperty) {
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
        log::info!("Shutting down {} recording FSM", if self.flavor.is_gen5() { "Black/White" } else { "Platinum" });
        crate::controller::deactivate(&self.active);
    }
}
