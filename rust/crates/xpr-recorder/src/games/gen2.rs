//! Port of `route_recording/game_recorders/gen_two/*` (Crystal).

use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::Value;

use xpr_core::consts;
use xpr_core::io_utils::sanitize_string;
use xpr_data::model::{CustomMoveData, Trainer};
use xpr_data::GenData;
use xpr_engine::{EventDefinition, HoldItemEventDefinition, InventoryEventDefinition, LearnMoveEventDefinition, LevelVal, TrainerEventDefinition, VitaminEventDefinition, WildPkmnEventDefinition};

use super::common::*;
use crate::controller::{dedupe, fix_key, fix_keys, is_set, new_active_flag, ActiveFlag, EventQueue, GameRecorder, GameState, RecorderController};
use crate::gamehook::{GameHookProperty, PropertyStore};
use crate::host::StartInfo;

pub const PKMN_CENTER_HEAL_SOUND_ID: i64 = 18;
pub const SAVE_HEAL_SOUND_ID: i64 = 37;
pub const TRAINER_BATTLE_TYPE: &str = "Trainer";
pub const WILD_BATTLE_TYPE: &str = "Wild";
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

const TM_NAMES_DEPRECATED: [&str; 50] = [
    "TM01-DynamicPunch", "TM02-Headbutt", "TM03-Curse", "TM04-Rollout", "TM05-Roar", "TM06-Toxic", "TM07-Zap Cannon", "TM08-Rock Smash", "TM09-Psych Up",
    "TM10-Hidden Power", "TM11-Sunny Day", "TM12-Sweet Scent", "TM13-Snore", "TM14-Blizzard", "TM15-Hyper Beam", "TM16-Icy Wind", "TM17-Protect",
    "TM18-Rain Dance", "TM19-Giga Drain", "TM20-Endure", "TM21-Frustration", "TM22-SolarBeam", "TM23-Iron Tail", "TM24-Dragonbreath", "TM25-Thunder",
    "TM26-Earthquake", "TM27-Return", "TM28-Dig", "TM29-Psychic", "TM30-Shadow Ball", "TM31-Mud-Slap", "TM32-Double Team", "TM33-Ice Punch",
    "TM34-Swagger", "TM35-Sleep Talk", "TM36-Sludge Bomb", "TM37-Sandstorm", "TM38-Fire Blast", "TM39-Swift", "TM40-Defense Curl", "TM41-ThunderPunch",
    "TM42-Dream Eater", "TM43-Detect", "TM44-Rest", "TM45-Attract", "TM46-Thief", "TM47-Steel Wing", "TM48-Fire Punch", "TM49-Fury Cutter", "TM50-Nightmare",
];
const TM_NAMES_STANDARD: [&str; 50] = [
    "TM01-DynamicPunch", "TM02-Headbutt", "TM03-Curse", "TM04-Rollout", "TM05-Roar", "TM06-Toxic", "TM07-Zap Cannon", "TM08-Rock Smash", "TM09-Psych Up",
    "TM10-Hidden Power", "TM11-Sunny Day", "TM12-Sweet Scent", "TM13-Snore", "TM14-Blizzard", "TM15-Hyperbeam", "TM16-Icy Wind", "TM17-Protect",
    "TM18-Rain Dance", "TM19-Giga Drain", "TM20-Endure", "TM21-Frustration", "TM22-SolarBeam", "TM23-Iron Tail", "TM24-DragonBreath", "TM25-Thunder",
    "TM26-Earthquake", "TM27-Return", "TM28-Dig", "TM29-Psychic", "TM30-Shadow Ball", "TM31-Mud-Slap", "TM32-Double Team", "TM33-Ice Punch",
    "TM34-Swagger", "TM35-Sleep Talk", "TM36-Sludge Bomb", "TM37-Sandstorm", "TM38-Fireblast", "TM39-Swift", "TM40-Defense Curl", "TM41-ThunderPunch",
    "TM42-Dream Eater", "TM43-Detect", "TM44-Rest", "TM45-Attract", "TM46-Thief", "TM47-Steel Wing", "TM48-Fire Punch", "TM49-Fury Cutter", "TM50-Nightmare",
];
const HM_NAMES_DEPRECATED: [&str; 7] = ["HM01-Cut", "HM02-Fly", "HM03-Surf", "HM04-Strength", "HM05-Flash", "HM06-Whirlpool", "HM07-Waterfall"];
const HM_NAMES_STANDARD: [&str; 7] = ["HM01", "HM02", "HM03", "HM04", "HM05", "HM06", "HM07"];

#[derive(Clone, Debug)]
pub struct Gen2Keys {
    pub overworld_map: String,
    pub overworld_map_num: String,
    pub overworld_x: String,
    pub overworld_y: String,
    pub player_id: String,
    pub player_money: String,
    pub mon_exppoints: String,
    pub mon_level: String,
    pub mon_species: String,
    pub mon_held_item: String,
    pub mon_friendship: String,
    pub team_species: Vec<String>,
    pub team_level: Vec<String>,
    pub team_dv_attack: Vec<String>,
    pub team_dv_defense: Vec<String>,
    pub team_dv_speed: Vec<String>,
    pub team_dv_special: Vec<String>,
    pub player_moves: Vec<String>,
    pub stat_exp: Vec<String>,
    pub gametime_seconds: String,
    pub gametime_frames: String,
    pub audio_current_sound: String,
    pub battle_mode: String,
    pub battle_type: String,
    pub battle_text_buffer: String,
    pub battle_result: String,
    pub battle_start: String,
    pub battle_trainer_class: String,
    pub battle_trainer_name: String,
    pub battle_trainer_number: String,
    pub battle_trainer_total_pokemon: String,
    pub battle_player_mon_party_pos: String,
    pub battle_player_mon_species: String,
    pub battle_player_mon_hp: String,
    pub battle_enemy_species: String,
    pub battle_enemy_level: String,
    pub battle_enemy_hp: String,
    pub battle_enemy_mon_party_pos: String,
    pub item_count: String,
    pub item_type: Vec<String>,
    pub item_quantity: Vec<String>,
    pub ball_count: String,
    pub ball_type: Vec<String>,
    pub ball_quantity: Vec<String>,
    pub key_item_count: String,
    pub key_items: Vec<String>,
    pub tm_keys: Vec<String>,
    pub hm_keys: Vec<String>,
    pub all_item_fields: HashSet<String>,
}

impl Gen2Keys {
    pub fn configure_for_mapper(game_name: Option<&str>) -> Gen2Keys {
        let deprecated = game_name.map(|g| g.contains("Deprecated")).unwrap_or(true);
        let d = |a: &str, b: &str| if deprecated { a.to_string() } else { b.to_string() };
        let team = |dep: &str, std: &str| -> Vec<String> { (0..6).map(|i| format!("player.team.{}.{}", i, if deprecated { dep } else { std })).collect() };
        let item_slots = if deprecated { 20 } else { 21 };
        let mut k = Gen2Keys {
            overworld_map: d("overworld.mapGroup", "overworld.map_group"),
            overworld_map_num: d("overworld.mapNumber", "overworld.map_index"),
            overworld_x: "overworld.x".into(),
            overworld_y: "overworld.y".into(),
            player_id: d("player.playerId", "player.player_id"),
            player_money: d("player.money", "bag.money"),
            mon_exppoints: d("player.team.0.expPoints", "player.team.0.exp"),
            mon_level: "player.team.0.level".into(),
            mon_species: "player.team.0.species".into(),
            mon_held_item: d("player.team.0.heldItem", "player.team.0.held_item"),
            mon_friendship: "player.team.0.friendship".into(),
            team_species: (0..6).map(|i| format!("player.team.{}.species", i)).collect(),
            team_level: (0..6).map(|i| format!("player.team.{}.level", i)).collect(),
            team_dv_attack: team("dvAttack", "ivs.attack"),
            team_dv_defense: team("dvDefense", "ivs.defense"),
            team_dv_speed: team("dvSpeed", "ivs.speed"),
            team_dv_special: team("dvSpecial", "ivs.special"),
            player_moves: if deprecated {
                (1..=4).map(|i| format!("player.team.0.move{}", i)).collect()
            } else {
                (0..4).map(|i| format!("player.team.0.moves.{}.move", i)).collect()
            },
            stat_exp: if deprecated {
                ["statExpHp", "statExpAttack", "statExpDefense", "statExpSpeed", "statExpSpecial"].iter().map(|s| format!("player.team.0.{}", s)).collect()
            } else {
                ["hp", "attack", "defense", "speed", "special"].iter().map(|s| format!("player.team.0.evs.{}", s)).collect()
            },
            gametime_seconds: d("gameTime.seconds", "game_time.seconds"),
            gametime_frames: d("gameTime.frames", "game_time.frames"),
            audio_current_sound: d("audio.currentSound", "audio.current_sound"),
            battle_mode: "battle.mode".into(),
            battle_type: "battle.type".into(),
            battle_text_buffer: d("battle.textBuffer", "battle.other.text_buffer"),
            battle_result: d("battle.result", "battle.outcome"),
            battle_start: d("battle.battleStart", "battle.other.battle_start"),
            battle_trainer_class: d("battle.trainer.class", "battle.opponent.trainer"),
            battle_trainer_name: d("battle.trainer.name", "battle.opponent.name"),
            battle_trainer_number: d("battle.trainer.id", "battle.opponent.id"),
            battle_trainer_total_pokemon: d("battle.trainer.totalPokemon", "battle.opponent.team_count"),
            battle_player_mon_party_pos: d("battle.yourPokemon.partyPos", "battle.player.party_position"),
            battle_player_mon_species: d("battle.yourPokemon.species", "battle.player.active_pokemon.species"),
            battle_player_mon_hp: d("battle.yourPokemon.hp", "battle.player.active_pokemon.stats.hp"),
            battle_enemy_species: d("battle.enemyPokemon.species", "battle.opponent.active_pokemon.species"),
            battle_enemy_level: d("battle.enemyPokemon.level", "battle.opponent.active_pokemon.level"),
            battle_enemy_hp: d("battle.enemyPokemon.hp", "battle.opponent.active_pokemon.stats.hp"),
            battle_enemy_mon_party_pos: d("battle.enemyPokemon.partyPos", "battle.opponent.party_position"),
            item_count: d("player.itemCount", "bag.item_count"),
            item_type: (0..item_slots).map(|i| if deprecated { format!("player.items.{}.item", i) } else { format!("bag.items.{}.item", i) }).collect(),
            item_quantity: (0..item_slots).map(|i| if deprecated { format!("player.items.{}.quantity", i) } else { format!("bag.items.{}.quantity", i) }).collect(),
            ball_count: d("player.pokeBallCount", "bag.ball_count"),
            ball_type: (0..12).map(|i| if deprecated { format!("player.pokeBalls.{}.item", i) } else { format!("bag.balls.{}.item", i) }).collect(),
            ball_quantity: (0..12).map(|i| if deprecated { format!("player.pokeBalls.{}.quantity", i) } else { format!("bag.balls.{}.quantity", i) }).collect(),
            key_item_count: d("player.totalKeyItems", "bag.key_count"),
            key_items: (0..26).map(|i| if deprecated { format!("player.keyItems.{}", i) } else { format!("bag.key_items.{}", i) }).collect(),
            tm_keys: if deprecated {
                TM_NAMES_DEPRECATED.iter().map(|t| format!("player.tms.{}", t)).collect()
            } else {
                TM_NAMES_STANDARD.iter().map(|t| format!("bag.tms.{}", t)).collect()
            },
            hm_keys: if deprecated {
                HM_NAMES_DEPRECATED.iter().map(|t| format!("player.hms.{}", t)).collect()
            } else {
                HM_NAMES_STANDARD.iter().map(|t| format!("bag.hms.{}", t)).collect()
            },
            all_item_fields: HashSet::new(),
        };
        k.rebuild_item_fields();
        k
    }

    fn rebuild_item_fields(&mut self) {
        let mut set = HashSet::new();
        set.insert(self.item_count.clone());
        set.insert(self.key_item_count.clone());
        for list in [&self.item_type, &self.item_quantity, &self.ball_type, &self.ball_quantity, &self.key_items, &self.tm_keys, &self.hm_keys] {
            set.extend(list.iter().cloned());
        }
        self.all_item_fields = set;
    }

    pub fn all_keys_to_register(&self) -> Vec<String> {
        let mut v = vec![
            self.overworld_map.clone(),
            self.overworld_x.clone(),
            self.overworld_y.clone(),
            self.player_id.clone(),
            self.player_money.clone(),
            self.mon_exppoints.clone(),
            self.mon_level.clone(),
            self.mon_species.clone(),
            self.mon_held_item.clone(),
            self.gametime_seconds.clone(),
            self.battle_mode.clone(),
            self.battle_type.clone(),
            self.battle_text_buffer.clone(),
            self.battle_result.clone(),
            self.battle_start.clone(),
            self.battle_player_mon_hp.clone(),
            self.battle_player_mon_party_pos.clone(),
            self.battle_enemy_species.clone(),
            self.battle_enemy_level.clone(),
            self.battle_enemy_hp.clone(),
            self.battle_enemy_mon_party_pos.clone(),
            self.audio_current_sound.clone(),
        ];
        v.extend(self.player_moves.iter().cloned());
        v.extend(self.stat_exp.iter().cloned());
        v.extend(self.all_item_fields.iter().cloned());
        v.extend(self.team_species.iter().cloned());
        v
    }

    pub fn fix_case(&mut self, store: &PropertyStore) -> Vec<String> {
        let mut invalid = Vec::new();
        for k in [
            &mut self.overworld_map,
            &mut self.overworld_map_num,
            &mut self.overworld_x,
            &mut self.overworld_y,
            &mut self.player_id,
            &mut self.player_money,
            &mut self.mon_exppoints,
            &mut self.mon_level,
            &mut self.mon_species,
            &mut self.mon_held_item,
            &mut self.mon_friendship,
            &mut self.gametime_seconds,
            &mut self.gametime_frames,
            &mut self.audio_current_sound,
            &mut self.battle_mode,
            &mut self.battle_type,
            &mut self.battle_text_buffer,
            &mut self.battle_result,
            &mut self.battle_start,
            &mut self.battle_trainer_class,
            &mut self.battle_trainer_name,
            &mut self.battle_trainer_number,
            &mut self.battle_trainer_total_pokemon,
            &mut self.battle_player_mon_party_pos,
            &mut self.battle_player_mon_species,
            &mut self.battle_player_mon_hp,
            &mut self.battle_enemy_species,
            &mut self.battle_enemy_level,
            &mut self.battle_enemy_hp,
            &mut self.battle_enemy_mon_party_pos,
            &mut self.item_count,
            &mut self.ball_count,
            &mut self.key_item_count,
        ] {
            fix_key(store, k, &mut invalid);
        }
        for list in [
            &mut self.team_species,
            &mut self.team_level,
            &mut self.team_dv_attack,
            &mut self.team_dv_defense,
            &mut self.team_dv_speed,
            &mut self.team_dv_special,
            &mut self.player_moves,
            &mut self.stat_exp,
            &mut self.item_type,
            &mut self.item_quantity,
            &mut self.ball_type,
            &mut self.ball_quantity,
            &mut self.key_items,
            &mut self.tm_keys,
            &mut self.hm_keys,
        ] {
            fix_keys(store, list, &mut invalid);
        }
        self.rebuild_item_fields();
        dedupe(invalid)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Gen2Converter;

impl Gen2Converter {
    pub fn is_game_vitamin(&self, item_name: &str) -> bool {
        let s = sanitize_string(item_name);
        ["HP UP", "PROTEIN", "IRON", "CARBOS", "CALCIUM"].iter().any(|v| sanitize_string(v) == s)
    }

    pub fn is_game_rare_candy(&self, item_name: &str) -> bool {
        sanitize_string(item_name) == sanitize_string("RARE CANDY")
    }

    pub fn is_game_tm(&self, item_name: &str) -> bool {
        item_name.starts_with("TM")
    }

    const TUTOR_MOVES: [&'static str; 3] = ["Thunderbolt", "Flamethrower", "Ice Beam"];

    pub fn is_tutor_move(&self, gh_move_name: &str) -> bool {
        Self::TUTOR_MOVES.contains(&name_prettify(gh_move_name).as_str())
    }

    pub fn get_hm_name(&self, gh_move_name: &str) -> Option<&'static str> {
        match name_prettify(gh_move_name).as_str() {
            "Cut" => Some("HM01 Cut"),
            "Fly" => Some("HM02 Fly"),
            "Surf" => Some("HM03 Surf"),
            "Strength" => Some("HM04 Strength"),
            "Flash" => Some("HM05 Flash"),
            "Whirlpool" => Some("HM06 Whirlpool"),
            "Waterfall" => Some("HM07 Waterfall"),
            _ => None,
        }
    }

    /// `TM##` / `HM##` from a property path.
    pub fn get_tmhm_name_from_path(&self, gh_path: &str) -> String {
        gh_path.rsplit('.').next().unwrap_or("").split('-').next().unwrap_or("").to_uppercase()
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

    const LEADER_CLASSES: [&'static str; 17] = [
        "Falkner", "Whitney", "Bugsy", "Morty", "Pryce", "Jasmine", "Chuck", "Clair", "Brock", "Misty", "Lt.Surge", "Erika", "Janine", "Sabrina", "Blaine", "Blue", "Red",
    ];
    const ELITE_FOUR_CLASSES: [&'static str; 4] = ["Will", "Bruno", "Karen", "Koga"];

    fn trainer_class_convert(&self, gh_trainer_class: Option<&str>) -> Option<String> {
        let c = gh_trainer_class?;
        let converted = name_prettify(c);
        Some(
            match converted.as_str() {
                "Cal" => "PkmnTrainer",
                "Cooltrainer M" => "CoolTrainerM",
                "Cooltrainer F" => "CoolTrainerF",
                "Grunt M" => "GruntM",
                "Grunt F" => "GruntF",
                "Swimmer M" => "SwimmerM",
                "Swimmer F" => "SwimmerF",
                "Executive M" => "ExecutiveM",
                "Executive F" => "ExecutiveF",
                "Pokefan M" => "PokefanM",
                "Pokefan F" => "PokefanF",
                "Mystical Man" => "Mysticalman",
                "Lt. Surge" => "Lt.Surge",
                other => other,
            }
            .to_string(),
        )
    }

    pub fn trainer_name_convert(&self, trainer_class: Option<&str>, trainer_num: &Value) -> String {
        let trainer_class = self.trainer_class_convert(trainer_class).unwrap_or_else(|| "None".into());
        if Self::LEADER_CLASSES.contains(&trainer_class.as_str()) {
            return format!("Leader {}", trainer_class);
        } else if Self::ELITE_FOUR_CLASSES.contains(&trainer_class.as_str()) {
            return format!("Elite Four {}", trainer_class);
        }
        format!("{}:{}", trainer_class, py_str(trainer_num))
    }

    pub fn area_name_convert(&self, area_name: &str) -> String {
        area_name.split('-').next().unwrap_or("").trim().to_string()
    }
}

/// The gens 2–5 move-name fix-ups (identical tables).
pub fn convert_move_name_gen2plus(converted: &str) -> String {
    match converted {
        "Doubleslap" => "DoubleSlap",
        "Thunderpunch" => "ThunderPunch",
        "Sand-attack" => "Sand Attack",
        "Double-edge" => "Double-Edge",
        "Sonicboom" => "SonicBoom",
        "Bubblebeam" => "BubbleBeam",
        "Solarbeam" => "SolarBeam",
        "Poisonpowder" => "PoisonPowder",
        "Thundershock" => "ThunderShock",
        "Conversion2" => "Conversion 2",
        "Mud-slap" => "Mud-Slap",
        "Lock-on" => "Lock-On",
        "Dynamicpunch" => "DynamicPunch",
        "Dragonbreath" => "DragonBreath",
        "Extremespeed" => "ExtremeSpeed",
        "Ancientpower" => "AncientPower",
        "Headbeutt" => "Headbutt",
        other => other,
    }
    .to_string()
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

#[derive(Default)]
struct BattleData {
    is_trainer_battle: bool,
    trainer_name: String,
    defeated_trainer_mons: Vec<EventDefinition>,
    waiting_for_moves: bool,
    move_update_delay: i64,
    waiting_for_items: bool,
    item_update_delay: i64,
    loss_detected: bool,
    evolution_detected: bool,
    held_item_consumed: bool,
    cached_mon_species: String,
    cached_mon_level: i64,
    exp_split: Vec<HashSet<i64>>,
    enemy_mon_order: Vec<i64>,
    friendship_data: Vec<i64>,
    battle_started: bool,
    initial_money: i64,
}

#[derive(Default)]
struct InventoryData {
    seconds_delay: i64,
    money_gained: bool,
    money_lost: bool,
    held_item_changed: bool,
}

#[derive(Default)]
struct CandyData {
    move_learned: bool,
    item_removal_detected: bool,
    cur_delay: i64,
}

#[derive(Default)]
struct MoveDeleteData {
    cur_delay: i64,
}

#[derive(Default)]
struct VitaminData {
    item_removal_detected: bool,
    cur_delay: i64,
}

#[derive(Default)]
struct OverworldData {
    waiting_for_registration: bool,
    register_delay: i64,
    waiting_for_new_file: bool,
    new_file_delay: i64,
    waiting_for_solo_mon_in_slot_1: bool,
    wrong_mon_delay: i64,
    wrong_mon_in_slot_1: bool,
}

const BASE_DELAY: i64 = 2;

pub struct Gen2Machine {
    controller: Arc<RecorderController>,
    gen: Arc<GenData>,
    keys: Gen2Keys,
    conv: Gen2Converter,
    debug_mode: bool,

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
    cached_lost_trainer: Arc<std::sync::Mutex<Option<String>>>,

    uninit: UninitData,
    resetting: ResettingData,
    battle: BattleData,
    inventory: InventoryData,
    candy: CandyData,
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
        special_defense: Value::Null,
        level: Value::Null,
    }
}

impl Gen2Machine {
    pub fn new(controller: Arc<RecorderController>, info: &StartInfo) -> Gen2Machine {
        Gen2Machine {
            controller,
            gen: info.gen.clone(),
            keys: Gen2Keys::configure_for_mapper(None),
            conv: Gen2Converter,
            debug_mode: info.debug_mode,
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
            cached_lost_trainer: Arc::new(std::sync::Mutex::new(None)),
            uninit: UninitData::default(),
            resetting: ResettingData::default(),
            battle: BattleData::default(),
            inventory: InventoryData::default(),
            candy: CandyData::default(),
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
            attack: store.value(&self.keys.team_dv_attack[mon_idx]),
            defense: store.value(&self.keys.team_dv_defense[mon_idx]),
            speed: store.value(&self.keys.team_dv_speed[mon_idx]),
            special_attack: store.value(&self.keys.team_dv_special[mon_idx]),
            special_defense: Value::Null,
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
            // see gen1: the Python check never matches, so only the last move per level survives
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
                log::info!("Creating mons from empty cache. Prioritizing slot 0: {}", new_cache[0].species.as_deref().unwrap_or("None"));
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

    fn area_name(&self, store: &PropertyStore) -> String {
        format!("{} Map {}", py_str(&store.value(&self.keys.overworld_map)), py_str(&store.value(&self.keys.overworld_map_num)))
    }

    fn update_all_cached_info(&mut self, store: &PropertyStore) {
        self.update_team_cache(store, false, true);
        self.item_cache_update(store, false, false, false, false, false, false, false);
        self.money_cache_update(store);
        let area = self.area_name(store);
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
        log::info!("new move cache: {:?}", new_cache);
        self.cached_moves = new_cache;
    }

    fn item_cache(&self, store: &PropertyStore) -> ItemCache {
        let mut result: ItemCache = IndexMap::new();
        // the normal pocket
        for i in 0..self.keys.item_type.len() {
            let item_type = store.value(&self.keys.item_type[i]);
            if item_type.is_null() {
                break;
            }
            let qty = store_i64(store, &self.keys.item_quantity[i]);
            *result.entry(item_type).or_insert(0) += qty;
        }
        // the ball pocket
        for i in 0..self.keys.ball_type.len() {
            let item_type = store.value(&self.keys.ball_type[i]);
            if item_type.is_null() {
                break;
            } else if result.contains_key(&item_type) {
                continue;
            }
            result.insert(item_type, store_i64(store, &self.keys.ball_quantity[i]));
        }
        // the key items pocket
        for i in 0..self.keys.key_items.len() {
            let item_type = store.value(&self.keys.key_items[i]);
            if item_type.is_null() {
                break;
            } else if result.contains_key(&item_type) {
                continue;
            }
            result.insert(item_type, 1);
        }
        // the tms pocket
        for i in 0..self.keys.tm_keys.len() {
            let tm_count = store_i64(store, &self.keys.tm_keys[i]);
            if tm_count > 0 {
                result.insert(Value::String(self.conv.get_tmhm_name_from_path(&self.keys.tm_keys[i])), tm_count);
            }
        }
        // the hms
        for i in 0..self.keys.hm_keys.len() {
            if store.truthy(&self.keys.hm_keys[i]) {
                result.insert(Value::String(self.conv.get_tmhm_name_from_path(&self.keys.hm_keys[i])), 1);
            }
        }
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn item_cache_update(&mut self, store: &PropertyStore, generate_events: bool, purchase_expected: bool, sale_expected: bool, vitamin_flag: bool, candy_flag: bool, tm_flag: bool, held_item_changed: bool) {
        let new_cache = self.item_cache(store);
        let old_cache = std::mem::replace(&mut self.cached_items, new_cache.clone());
        if !generate_events {
            return;
        }
        let (gained_items, lost_items) = item_diff(&old_cache, &new_cache);
        if !gained_items.is_empty() && sale_expected {
            log::error!("Gained the following items when expecting to be losing items to selling... {}", repr_items(&gained_items));
        }
        if !held_item_changed {
            for (cur_gained_item, cur_gain_num) in &gained_items {
                let app_item_name = self.conv.item_name_convert(cur_gained_item.as_str()).unwrap_or_else(|| "None".into());
                log::info!("trying to gain item: {}, converted from {}", app_item_name, py_str(cur_gained_item));
                self.queue_new_event(EventDefinition::with_item(InventoryEventDefinition::new(&app_item_name, *cur_gain_num, true, purchase_expected, None)));
            }
        }
        if !lost_items.is_empty() {
            if purchase_expected {
                log::error!("Lost the following items when expecting to be gain items to purchasing... {}", repr_items(&lost_items));
            }
            if held_item_changed {
                log::error!("Lost multiple items when trying to change the held item... {}", repr_items(&lost_items));
            }
        }
        for (cur_lost_item, cur_lost_num) in &lost_items {
            let raw = cur_lost_item.as_str().unwrap_or("");
            let app_item_name = self.conv.item_name_convert(cur_lost_item.as_str()).unwrap_or_else(|| "None".into());
            if vitamin_flag && self.conv.is_game_vitamin(raw) {
                if sale_expected {
                    log::error!("Expected sale, but looks like vitamins were used too???");
                }
                self.queue_new_event(EventDefinition::with_vitamin(VitaminEventDefinition::new(&app_item_name, *cur_lost_num)));
            } else if candy_flag && self.conv.is_game_rare_candy(raw) {
                if sale_expected {
                    log::error!("Expected sale, but looks like rare candy was used too???");
                }
                self.queue_new_event(EventDefinition::with_rare_candy(*cur_lost_num));
            } else if tm_flag && self.conv.is_game_tm(raw) {
                if sale_expected {
                    log::error!("Expected sale, but looks like TM was used too???");
                }
                self.move_cache_update(store, true, Some(&app_item_name), false, false, false);
            } else if held_item_changed {
                if *cur_lost_num > 1 {
                    log::error!("Expected to lose multiple items while telling mon to hold item: {} x{}", app_item_name, cur_lost_num);
                }
                self.queue_new_event(EventDefinition::with_hold_item(HoldItemEventDefinition::new(Some(&app_item_name), false)));
            } else {
                self.queue_new_event(EventDefinition::with_item(InventoryEventDefinition::new(&app_item_name, *cur_lost_num, false, sale_expected, None)));
            }
        }
    }

    // ---- states -------------------------------------------------------------------

    fn watch_for_reset(&self, new: &GameHookProperty) -> Option<GameState> {
        if self.player_id.is_some() && new.path == self.keys.player_id && new.eq_i64(0) {
            return Some(GameState::Resetting);
        }
        None
    }

    fn return_custom_data(&self, friendship_values: &[i64]) -> Vec<CustomMoveData> {
        friendship_values
            .iter()
            .map(|f| {
                let mut c = CustomMoveData::default();
                c.player.insert(RETURN_MOVE_NAME.to_string(), ((*f as f64) / 2.5).trunc().to_string().trim_end_matches(".0").to_string());
                c
            })
            .collect()
    }

    fn has_return(&self) -> bool {
        self.cached_moves.iter().flatten().any(|m| m == RETURN_MOVE_NAME)
    }

    fn create_initial_trainer_event(&mut self, store: &PropertyStore) {
        if self.battle.is_trainer_battle {
            let total = store_i64(store, &self.keys.battle_trainer_total_pokemon).max(0) as usize;
            self.battle.exp_split = (0..total).map(|_| HashSet::from([0i64])).collect();
            self.battle.trainer_name = self.conv.trainer_name_convert(store.str_of(&self.keys.battle_trainer_class).as_deref(), &store.value(&self.keys.battle_trainer_number));
            let mut td = TrainerEventDefinition::new(&self.battle.trainer_name);
            if self.has_return() {
                let cur_friendship = store_i64(store, &self.keys.mon_friendship);
                td.custom_move_data = self.return_custom_data(&[cur_friendship; 6]);
            }
            self.queue_new_event(EventDefinition::with_trainer(td));
        }
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
                let is_trainer = store.get(&self.keys.battle_mode).map(|p| p.eq_str(TRAINER_BATTLE_TYPE)).unwrap_or(false);
                let money = store_i64(store, &self.keys.player_money);
                let b = &mut self.battle;
                b.defeated_trainer_mons.clear();
                b.waiting_for_moves = false;
                b.move_update_delay = 0;
                b.waiting_for_items = false;
                b.item_update_delay = 0;
                b.is_trainer_battle = is_trainer;
                b.loss_detected = false;
                b.evolution_detected = false;
                b.held_item_consumed = false;
                b.cached_mon_species.clear();
                b.cached_mon_level = 0;
                b.trainer_name.clear();
                b.exp_split = vec![HashSet::from([0i64])];
                b.enemy_mon_order.clear();
                b.friendship_data.clear();
                b.battle_started = false;
                b.initial_money = money;
                log::info!("Entered battle. With trainer initially? {}", is_trainer);
            }
            GameState::InventoryChange => {
                self.inventory.seconds_delay = BASE_DELAY;
                self.inventory.money_gained = self.money_cache_update(store).unwrap_or(false);
                self.inventory.money_lost = false;
                self.inventory.held_item_changed = false;
            }
            GameState::RareCandy => {
                self.candy.move_learned = false;
                self.candy.item_removal_detected = false;
                self.candy.cur_delay = BASE_DELAY;
            }
            GameState::Tm => {}
            GameState::MoveDelete => self.move_delete.cur_delay = BASE_DELAY,
            GameState::Vitamin => {
                self.vitamin.item_removal_detected = false;
                self.vitamin.cur_delay = BASE_DELAY;
            }
            GameState::Overworld => {
                self.money_cache_update(store);
                self.update_team_cache(store, true, false);
                let o = &mut self.overworld;
                o.waiting_for_registration = false;
                o.register_delay = BASE_DELAY;
                o.waiting_for_new_file = false;
                o.new_file_delay = BASE_DELAY;
                o.waiting_for_solo_mon_in_slot_1 = false;
                o.wrong_mon_delay = BASE_DELAY;
                o.wrong_mon_in_slot_1 = false;
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
                    if self.battle.is_trainer_battle {
                        let mut final_exp_split: Vec<i64> = self.battle.exp_split.iter().map(|x| x.len() as i64).collect();
                        if !final_exp_split.iter().any(|x| *x > 1) {
                            final_exp_split = Vec::new();
                        }
                        let mut sorted = self.battle.enemy_mon_order.clone();
                        sorted.sort();
                        let final_mon_order: Vec<i64> = sorted.iter().map(|x| self.battle.enemy_mon_order.iter().position(|y| y == x).unwrap_or(0) as i64).collect();
                        let mut td = TrainerEventDefinition::new(&self.battle.trainer_name);
                        td.exp_split = final_exp_split;
                        td.mon_order = final_mon_order;
                        if self.has_return() {
                            td.custom_move_data = self.return_custom_data(&self.battle.friendship_data);
                        }
                        td.pay_day_amount = Some(store_i64(store, &self.keys.player_money) - self.battle.initial_money);
                        let mut ev = EventDefinition::with_trainer(td);
                        ev.notes = roar_flag();
                        self.queue_new_event(ev);
                    }
                    if self.battle.loss_detected {
                        if self.battle.is_trainer_battle {
                            let mut ev = EventDefinition::with_trainer(TrainerEventDefinition::new(&self.battle.trainer_name));
                            ev.notes = trainer_loss_flag();
                            self.queue_new_event(ev);
                            let mons = std::mem::take(&mut self.battle.defeated_trainer_mons);
                            for m in mons {
                                self.queue_new_event(m);
                            }
                        }
                        self.queue_new_event(EventDefinition::with_blackout());
                    } else if self.battle.is_trainer_battle && self.battle.trainer_name.starts_with("Champion") {
                        self.queue_new_event(EventDefinition::with_save("Post-Champion Autosave"));
                    }
                    if self.battle.waiting_for_moves {
                        if self.battle.evolution_detected {
                            self.update_team_cache(store, true, false);
                        }
                        self.move_cache_update(store, true, None, false, false, true);
                    }
                    if self.battle.waiting_for_items {
                        self.item_cache_update(store, true, false, false, false, false, false, false);
                    }
                    if self.battle.held_item_consumed {
                        self.queue_new_event(EventDefinition::with_hold_item(HoldItemEventDefinition::new(None, true)));
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
                    self.item_cache_update(store, true, false, false, false, true, false, false);
                    let level = store_i64(store, &self.keys.mon_level);
                    self.solo_mon_levelup(level);
                    self.update_team_cache(store, true, false);
                    if self.candy.move_learned {
                        self.move_cache_update(store, true, None, false, false, true);
                    }
                }
            }
            GameState::Tm => {
                if next != GameState::Resetting {
                    self.item_cache_update(store, true, false, false, false, false, true, false);
                }
            }
            GameState::MoveDelete => {
                if next != GameState::Resetting {
                    self.move_cache_update(store, true, None, false, true, false);
                }
            }
            GameState::Vitamin => {
                if next != GameState::Resetting {
                    self.item_cache_update(store, true, false, false, true, false, false, false);
                }
            }
            GameState::Overworld => {}
        }
    }

    fn transition(&mut self, store: &PropertyStore, new: &GameHookProperty, prev: &GameHookProperty) -> GameState {
        let state = self.cur_state;
        if state != GameState::Resetting {
            if let Some(r) = self.watch_for_reset(new) {
                return r;
            }
        }
        match state {
            GameState::Uninitialized => {
                if new.path == self.keys.gametime_seconds {
                    if self.uninit.seconds_delay <= 0 {
                        // (Appendix B item 11 analogue: value comparison)
                        let mode = store.value(&self.keys.battle_mode);
                        if mode.as_str() == Some(WILD_BATTLE_TYPE) || mode.as_str() == Some(TRAINER_BATTLE_TYPE) {
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
                    if self.solo_mon_key.species == self.conv.pkmn_name_convert(store.str_of(&self.keys.mon_species).as_deref()) {
                        self.inventory.held_item_changed = true;
                        log::info!("gotem: {}", self.inventory.held_item_changed);
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
                } else if new.path == self.keys.item_count {
                    self.candy.item_removal_detected = true;
                } else if self.keys.item_quantity.contains(&new.path) {
                    if !self.candy.item_removal_detected {
                        return GameState::Overworld;
                    }
                } else if self.keys.item_type.contains(&new.path) {
                    if new.is_null() {
                        return GameState::Overworld;
                    }
                } else if new.path == self.keys.gametime_seconds {
                    if self.candy.item_removal_detected {
                        if self.candy.cur_delay <= 0 {
                            return GameState::Overworld;
                        } else {
                            self.candy.cur_delay -= 1;
                        }
                    }
                } else if new.path == self.keys.overworld_x || new.path == self.keys.overworld_y {
                    // fail-safe: the player is moving, the candy is definitely over
                    return GameState::Overworld;
                }
                state
            }
            GameState::Tm => {
                if self.keys.tm_keys.contains(&new.path) {
                    return GameState::Overworld;
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
                if new.path == self.keys.item_count {
                    self.vitamin.item_removal_detected = true;
                } else if self.keys.item_quantity.contains(&new.path) {
                    if !self.vitamin.item_removal_detected {
                        return GameState::Overworld;
                    }
                } else if self.keys.item_type.contains(&new.path) {
                    if new.is_null() {
                        return GameState::Overworld;
                    }
                } else if new.path == self.keys.gametime_seconds {
                    if self.vitamin.item_removal_detected {
                        if self.vitamin.cur_delay <= 0 {
                            return GameState::Overworld;
                        } else {
                            self.vitamin.cur_delay -= 1;
                        }
                    }
                }
                state
            }
            GameState::Overworld => self.overworld_transition(store, new, prev),
        }
    }

    fn battle_transition(&mut self, store: &PropertyStore, new: &GameHookProperty, prev: &GameHookProperty) -> GameState {
        let state = GameState::Battle;
        if new.path == self.keys.mon_exppoints {
            if self.battle.cached_mon_species.is_empty() || self.battle.cached_mon_level == 0 {
                log::error!("Solo mon gained experience, but we didn't properly cache which enemy mon was defeated...");
            } else {
                let (species, level) = (self.battle.cached_mon_species.clone(), self.battle.cached_mon_level);
                if self.battle.is_trainer_battle {
                    self.battle.defeated_trainer_mons.push(EventDefinition::with_wild(WildPkmnEventDefinition::new(&species, level, 1, true)));
                } else {
                    self.queue_new_event(EventDefinition::with_wild(WildPkmnEventDefinition::new(&species, level, 1, false)));
                }
                self.battle.cached_mon_species.clear();
                self.battle.cached_mon_level = 0;
            }
        } else if new.path == self.keys.battle_start {
            if prev.eq_i64(1) && new.eq_i64(0) {
                self.battle.battle_started = true;
            }
        } else if new.path == self.keys.battle_enemy_hp {
            if new.eq_i64(0) {
                self.battle.cached_mon_species = self.conv.pkmn_name_convert(store.str_of(&self.keys.battle_enemy_species).as_deref()).unwrap_or_default();
                self.battle.cached_mon_level = store_i64(store, &self.keys.battle_enemy_level);
                self.battle.friendship_data.push(store_i64(store, &self.keys.mon_friendship));
            }
        } else if new.path == self.keys.battle_text_buffer {
            if self.battle.trainer_name.is_empty() {
                self.battle.is_trainer_battle = store.get(&self.keys.battle_mode).map(|p| p.eq_str(TRAINER_BATTLE_TYPE)).unwrap_or(false);
                if self.battle.is_trainer_battle {
                    self.create_initial_trainer_event(store);
                } else {
                    // junk so we stop querying this
                    self.battle.trainer_name = "WildMon".into();
                }
            }
        } else if new.path == self.keys.battle_player_mon_hp {
            let player_mon_pos = store_i64(store, &self.keys.battle_player_mon_party_pos);
            let hp = new.as_i64().unwrap_or(0);
            if player_mon_pos == 0 && hp <= 0 {
                if self.battle.battle_started {
                    self.battle.loss_detected = true;
                }
            } else if hp <= 0 {
                let enemy_mon_pos = store_i64(store, &self.keys.battle_enemy_mon_party_pos);
                if let Some(set) = usize::try_from(enemy_mon_pos).ok().and_then(|i| self.battle.exp_split.get_mut(i)) {
                    set.remove(&player_mon_pos);
                }
            }
        } else if new.path == self.keys.mon_species {
            self.battle.evolution_detected = true;
        } else if self.keys.player_moves.contains(&new.path) {
            if !self.battle.waiting_for_moves {
                self.battle.move_update_delay = 2;
                self.battle.waiting_for_moves = true;
            }
        } else if self.keys.all_item_fields.contains(&new.path) {
            if !self.battle.waiting_for_items {
                self.battle.item_update_delay = 2;
                self.battle.waiting_for_items = true;
            }
        } else if new.path == self.keys.gametime_seconds {
            if self.battle.waiting_for_moves {
                if self.battle.move_update_delay <= 0 {
                    self.battle.waiting_for_moves = false;
                    if self.battle.evolution_detected {
                        self.update_team_cache(store, true, false);
                    }
                    self.move_cache_update(store, true, None, false, false, true);
                } else {
                    self.battle.move_update_delay -= 1;
                }
            }
            if self.battle.waiting_for_items {
                if self.battle.item_update_delay <= 0 {
                    self.battle.waiting_for_items = false;
                    self.item_cache_update(store, true, false, false, false, false, false, false);
                } else {
                    self.battle.item_update_delay -= 1;
                }
            }
        } else if new.path == self.keys.mon_level {
            self.solo_mon_levelup(new.as_i64().unwrap_or(0));
        } else if new.path == self.keys.mon_held_item {
            self.battle.held_item_consumed = true;
        } else if new.path == self.keys.battle_player_mon_party_pos {
            let enemy_mon_pos = store_i64(store, &self.keys.battle_enemy_mon_party_pos);
            let v = new.as_i64().unwrap_or(-1);
            if (0..6).contains(&v) && (0..6).contains(&enemy_mon_pos) {
                if let Some(set) = self.battle.exp_split.get_mut(enemy_mon_pos as usize) {
                    set.insert(v);
                }
            }
        } else if new.path == self.keys.battle_enemy_mon_party_pos {
            let v = new.as_i64().unwrap_or(-1);
            if v >= 0 && (v as usize) < self.battle.exp_split.len() {
                self.battle.exp_split[v as usize] = HashSet::from([store_i64(store, &self.keys.battle_player_mon_party_pos)]);
            }
            if !self.battle.enemy_mon_order.contains(&v) {
                self.battle.enemy_mon_order.push(v);
            }
        } else if new.path == self.keys.battle_mode {
            if new.is_null() {
                return GameState::Overworld;
            }
        }
        state
    }

    fn overworld_transition(&mut self, store: &PropertyStore, new: &GameHookProperty, prev: &GameHookProperty) -> GameState {
        let state = GameState::Overworld;
        if self.overworld.waiting_for_new_file || self.overworld.waiting_for_solo_mon_in_slot_1 {
            if new.path == self.keys.gametime_seconds {
                if self.overworld.wrong_mon_delay <= 0 {
                    self.overworld.waiting_for_solo_mon_in_slot_1 = false;
                    self.overworld.wrong_mon_in_slot_1 = false;
                }
                if self.overworld.new_file_delay <= 0 {
                    self.overworld.waiting_for_new_file = false;
                    self.controller.route_restarted();
                    self.update_all_cached_info(store);
                }
                self.overworld.new_file_delay -= 1;
                self.overworld.wrong_mon_delay -= 1;
            }
            return state;
        }
        if new.path == self.keys.battle_mode {
            return GameState::Battle;
        } else if new.path == self.keys.overworld_map || new.path == self.keys.overworld_map_num {
            let area = self.area_name(store);
            self.controller.entered_new_area(&area);
        } else if new.path == self.keys.player_id {
            if prev.truthy() && self.player_id.as_ref() != Some(&store.value(&self.keys.player_id)) {
                self.overworld.waiting_for_new_file = true;
            }
        } else if self.keys.all_item_fields.contains(&new.path) {
            return GameState::InventoryChange;
        } else if new.path == self.keys.mon_held_item {
            if !self.overworld.wrong_mon_in_slot_1 {
                let name = self.conv.item_name_convert(new.as_str());
                let mut ev = EventDefinition::with_hold_item(HoldItemEventDefinition::new(name.as_deref(), false));
                ev.notes = held_check_flag();
                self.queue_new_event(ev);
            }
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
                if new.as_i64() == prev.as_i64().map(|p| p + 1) {
                    return GameState::RareCandy;
                }
                log::warn!("Ignoring level change event that seems impossible, transitioning from: {} to {}", py_str(&prev.value), py_str(&new.value));
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
                } else if self.conv.get_hm_name(new.as_str().unwrap_or("")).is_some() {
                    self.move_cache_update(store, true, None, true, false, false);
                } else if self.conv.is_tutor_move(new.as_str().unwrap_or("")) {
                    self.move_cache_update(store, true, None, false, true, false);
                } else {
                    return GameState::Tm;
                }
            }
        } else if self.keys.stat_exp.contains(&new.path) {
            if !self.overworld.waiting_for_registration && !self.overworld.wrong_mon_in_slot_1 {
                return GameState::Vitamin;
            }
        } else if new.path == self.keys.audio_current_sound {
            // the location is the map *group* value as the mapper reports it (an int)
            if new.eq_i64(PKMN_CENTER_HEAL_SOUND_ID) {
                self.queue_new_event(EventDefinition::with_heal_value(&store.value(&self.keys.overworld_map)));
            } else if new.eq_i64(SAVE_HEAL_SOUND_ID) {
                self.queue_new_event(EventDefinition::with_save_value(&store.value(&self.keys.overworld_map)));
            }
        } else if new.path == self.keys.gametime_seconds {
            if self.overworld.waiting_for_registration {
                if self.overworld.register_delay <= 0 {
                    self.overworld.waiting_for_registration = false;
                    self.update_team_cache(store, true, true);
                }
                self.overworld.register_delay -= 1;
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
        let cached_lost_trainer = self.cached_lost_trainer.clone();
        std::thread::Builder::new()
            .name("gen2-recorder-events".into())
            .spawn(move || {
                let lost = cached_lost_trainer;
                ctx.run(|ctx, ev| process_one(ctx, ev, &lost));
            })
            .ok();
    }
}

/// `_get_trainer_obj`: coupled with `trainer_name_convert` (`class:id`).
fn get_trainer_obj(gen: &GenData, converted_trainer_name: &str) -> Option<Arc<Trainer>> {
    if !converted_trainer_name.contains(':') {
        return gen.trainer_db().get_trainer(converted_trainer_name).cloned();
    }
    let parts: Vec<&str> = converted_trainer_name.splitn(2, ':').collect();
    let trainer_class = parts[0];
    let trainer_id: i64 = parts.get(1).and_then(|s| s.trim().parse().ok())?;
    for name in gen.trainer_db().get_valid_trainers(Some(trainer_class), None, &[], true, false) {
        if let Some(t) = gen.trainer_db().get_trainer(&name) {
            if t.trainer_id == trainer_id {
                return Some(t.clone());
            }
        }
    }
    None
}

/// `_validate_trainer_pkmn`
fn validate_trainer_pkmn(gen: &GenData, mut event_def: EventDefinition, expected_trainer: Option<&str>) -> EventDefinition {
    let Some(w) = event_def.wild_pkmn_info.clone() else { return event_def };
    let trainer = expected_trainer.and_then(|t| gen.trainer_db().get_trainer(t));
    match trainer {
        None => {
            log::warn!("Couldn't validate wild mon {} from trainer {}: no such trainer", event_str(gen, &event_def), expected_trainer.unwrap_or("None"));
            event_def
        }
        Some(trainer_obj) => {
            for test_mon in &trainer_obj.pkmn {
                if test_mon.name == w.name && test_mon.level == w.level {
                    return event_def;
                }
            }
            for test_mon in &trainer_obj.pkmn {
                if test_mon.name == w.name {
                    log::info!("automatically correcting trainer mon's level from {} to {}", w.level, test_mon.level);
                    if let Some(wi) = event_def.wild_pkmn_info.as_mut() {
                        wi.level = test_mon.level;
                    }
                    return event_def;
                }
            }
            log::warn!("Couldn't validate wild mon {} from trainer {} due to: No matching species exists on source trainer", event_str(gen, &event_def), expected_trainer.unwrap_or("None"));
            event_def
        }
    }
}

fn process_one(ctx: &ProcessCtx, mut cur_event: EventDefinition, cached_lost_trainer: &std::sync::Mutex<Option<String>>) {
    let gen = &ctx.gen;
    // quick pre-check to handle the lost trainer cache
    if cur_event.wild_pkmn_info.is_none() {
        *cached_lost_trainer.lock().unwrap() = None;
    }
    if cur_event.notes == reset_flag() {
        log::info!("Resetting to last save...");
        ctx.controller.game_reset();
        return;
    } else if let Some(td) = cur_event.trainer_def.clone() {
        let Some(trainer) = get_trainer_obj(gen, &td.trainer_name) else {
            ctx.add_error(format!("Failed to find trainer from GameHook: {}", td.trainer_name));
            return;
        };
        if let Some(t) = cur_event.trainer_def.as_mut() {
            t.trainer_name = trainer.name.clone();
        }
        let trainer_name = trainer.name.clone();
        if cur_event.notes == trainer_loss_flag() {
            log::info!("Handling trainer loss: {}", trainer_name);
            ctx.controller.lost_trainer_battle(&trainer_name);
            *cached_lost_trainer.lock().unwrap() = Some(trainer_name.clone());
            log::info!("setting cached_lost_trainer: {}", trainer_name);
            return;
        } else if cur_event.notes == roar_flag() {
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
                    log::info!("held item: {:?}", obj.final_held_item);
                    if obj.final_held_item.as_deref() == Some(consts::AMULET_COIN_ITEM_NAME) {
                        expected_money *= 2;
                    }
                    if let Some(t) = cur_event.trainer_def.as_mut() {
                        t.pay_day_amount = Some((t.pay_day_amount.unwrap_or(0) - expected_money).max(0));
                        log::info!("Updating pay day for trainer {} to {:?}", t.trainer_name, t.pay_day_amount);
                    }
                    let id = obj.group_id;
                    let ev = cur_event.clone();
                    ctx.controller.host().call(move |h| h.update_existing_event(id, ev));
                }
            }
            // ROAR_FLAG is queued even for losses (a TRAINER_LOSS_FLAG follows)
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
        }
    } else if let Some(hold) = cur_event.hold_item.clone() {
        if ctx.controller.host().call(|h| h.get_previous_event(None)).is_none() {
            log::info!("Giving player initial berry that mon starts with");
            ctx.controller.add_event(EventDefinition::with_item(InventoryEventDefinition::new("Berry", 1, true, false, None)));
        }
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
        if w.trainer_pkmn {
            let lost = cached_lost_trainer.lock().unwrap().clone();
            log::info!("handling trainer mon event: {} with _cached_lost_trainer: {:?}", event_str(gen, &cur_event), lost);
            cur_event = validate_trainer_pkmn(gen, cur_event, lost.as_deref());
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

impl GameRecorder for Gen2Machine {
    fn on_mapper_loaded(&mut self, store: &PropertyStore) -> (Vec<String>, Vec<String>) {
        self.keys = Gen2Keys::configure_for_mapper(store.game_name());
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
        if self.debug_mode && new.path != self.keys.gametime_seconds {
            log::info!(
                "Change of {} from {} to {} for state {}, on frame {}",
                new.path,
                py_str(&prev.value),
                py_str(&new.value),
                self.cur_state,
                py_str(&store.value(&self.keys.gametime_frames))
            );
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
        log::info!("Shutting down Crystal recording FSM");
        crate::controller::deactivate(&self.active);
    }
}
