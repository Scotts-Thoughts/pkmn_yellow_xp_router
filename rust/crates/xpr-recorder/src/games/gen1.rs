//! Port of `route_recording/game_recorders/gen_one/*` (Yellow and Red/Blue).

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::Value;

use xpr_core::consts;
use xpr_data::GenData;
use xpr_engine::{EventDefinition, InventoryEventDefinition, LearnMoveEventDefinition, LevelVal, TrainerEventDefinition, VitaminEventDefinition, WildPkmnEventDefinition};

use super::common::*;
use crate::controller::{fix_key, fix_keys, dedupe, is_set, new_active_flag, ActiveFlag, EventQueue, GameRecorder, GameState, RecorderController};
use crate::gamehook::{GameHookProperty, PropertyStore};
use crate::host::StartInfo;

// ---------------------------------------------------------------------------
// constants
// ---------------------------------------------------------------------------

pub const OAKS_PARCEL: &str = "Oak's Parcel";
pub const VENDING_MACHINE_DRINKS: [&str; 3] = ["Fresh Water", "Soda Pop", "Lemonade"];
pub const NUGGET: &str = "Nugget";
pub const NUGGET_ROCKET: &str = "Rocket 6";
pub const RIVAL_LAB_FIGHTS: [&str; 4] = ["Rival1 1", "Rival1 Charmander 1", "Rival1 Squirtle 1", "Rival1 Bulbasaur 1"];
pub const TRAINER_BATTLE_TYPE: &str = "Trainer";
pub const WILD_BATTLE_TYPE: &str = "Wild";
pub const END_OF_ITEM_LIST: &str = "--End of list--";
pub const MAP_GAME_CORNER: &str = "Celadon City - Game Corner";

pub fn reset_flag() -> String {
    format!("{}FLAG TO SIGNAL GAME RESET. USER SHOULD NEVER SEE THIS", consts::RECORDING_ERROR_FRAGMENT)
}
pub fn trainer_loss_flag() -> String {
    format!("{}FLAG TO SIGNAL LOSING TO TRAINER. USER SHOULD NEVER SEE THIS", consts::RECORDING_ERROR_FRAGMENT)
}
pub fn pay_day_flag() -> String {
    format!("{}FLAG TO HANDLE POTENTIAL PAY DAY REWARDS. USER SHOULD NEVER SEE THIS", consts::RECORDING_ERROR_FRAGMENT)
}

/// `Gen1GameHookConstants` for one mapper flavour.
#[derive(Clone, Debug)]
pub struct Gen1Keys {
    pub deprecated: bool,
    pub overworld_map: String,
    pub audio_channel_4: String,
    pub audio_channel_5: String,
    pub audio_channel_7: String,
    pub player_id: String,
    pub player_money: String,
    pub mon_exppoints: String,
    pub mon_level: String,
    pub mon_species: String,
    pub team_species: Vec<String>,
    pub team_level: Vec<String>,
    pub team_dv_attack: Vec<String>,
    pub team_dv_defense: Vec<String>,
    pub team_dv_speed: Vec<String>,
    pub team_dv_special: Vec<String>,
    pub player_moves: Vec<String>,
    pub stat_exp: Vec<String>,
    pub gametime_seconds: String,
    pub battle_type: String,
    pub battle_trainer_class: String,
    pub battle_trainer_number: String,
    pub battle_player_mon_species: String,
    pub battle_player_mon_hp: String,
    pub battle_enemy_species: String,
    pub battle_enemy_level: String,
    pub item_count: String,
    pub item_type: Vec<String>,
    pub item_quantity: Vec<String>,
}

impl Gen1Keys {
    pub fn configure_for_mapper(game_name: Option<&str>) -> Gen1Keys {
        let deprecated = game_name.map(|g| g.contains("Deprecated")).unwrap_or(true);
        let d = |a: &str, b: &str| if deprecated { a.to_string() } else { b.to_string() };
        let team = |dep: &str, std: &str| -> Vec<String> { (0..6).map(|i| format!("player.team.{}.{}", i, if deprecated { dep } else { std })).collect() };
        let items = |dep: &str, std: &str| -> Vec<String> { (0..20).map(|i| if deprecated { format!("player.items.{}.{}", i, dep) } else { format!("bag.items.{}.{}", i, std) }).collect() };
        Gen1Keys {
            deprecated,
            overworld_map: d("overworld.map", "overworld.map_name"),
            audio_channel_4: d("audio.channel4", "audio.channels.3"),
            audio_channel_5: d("audio.channel5", "audio.channels.4"),
            audio_channel_7: d("audio.channel7", "audio.channels.6"),
            player_id: d("player.playerId", "player.player_id"),
            player_money: d("player.money", "bag.money"),
            mon_exppoints: d("player.team.0.expPoints", "player.team.0.exp"),
            mon_level: "player.team.0.level".into(),
            mon_species: "player.team.0.species".into(),
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
            battle_type: d("battle.type", "battle.mode"),
            battle_trainer_class: d("battle.trainer.class", "battle.opponent.trainer"),
            battle_trainer_number: d("battle.trainer.number", "battle.opponent.id"),
            battle_player_mon_species: d("battle.yourPokemon.species", "battle.player.active_pokemon.species"),
            battle_player_mon_hp: d("battle.yourPokemon.battleStatHp", "battle.player.active_pokemon.stats.hp"),
            battle_enemy_species: d("battle.enemyPokemon.species", "battle.opponent.active_pokemon.species"),
            battle_enemy_level: d("battle.enemyPokemon.level", "battle.opponent.active_pokemon.level"),
            item_count: d("player.itemCount", "bag.item_count"),
            item_type: items("item", "item"),
            item_quantity: items("quantity", "quantity"),
        }
    }

    /// `NONE_BATTLE_TYPE`: `"None"` on the deprecated mapper, JSON null otherwise.
    pub fn is_none_battle_type(&self, p: &GameHookProperty) -> bool {
        if self.deprecated {
            p.eq_str("None")
        } else {
            p.is_null()
        }
    }

    pub fn all_keys_to_register(&self) -> Vec<String> {
        let mut v = vec![
            self.overworld_map.clone(),
            self.audio_channel_4.clone(),
            self.audio_channel_5.clone(),
            self.player_id.clone(),
            self.player_money.clone(),
            self.mon_exppoints.clone(),
            self.mon_level.clone(),
            self.mon_species.clone(),
            self.gametime_seconds.clone(),
            self.battle_type.clone(),
            self.battle_player_mon_hp.clone(),
            self.battle_enemy_species.clone(),
            self.battle_enemy_level.clone(),
            self.item_count.clone(),
        ];
        v.extend(self.player_moves.iter().cloned());
        v.extend(self.stat_exp.iter().cloned());
        v.extend(self.item_type.iter().cloned());
        v.extend(self.item_quantity.iter().cloned());
        v.extend(self.team_species.iter().cloned());
        v
    }

    /// `validate_constants`: case-correct every key; returns the invalid ones.
    pub fn fix_case(&mut self, store: &PropertyStore) -> Vec<String> {
        let mut invalid = Vec::new();
        for k in [
            &mut self.overworld_map,
            &mut self.audio_channel_4,
            &mut self.audio_channel_5,
            &mut self.audio_channel_7,
            &mut self.player_id,
            &mut self.player_money,
            &mut self.mon_exppoints,
            &mut self.mon_level,
            &mut self.mon_species,
            &mut self.gametime_seconds,
            &mut self.battle_type,
            &mut self.battle_trainer_class,
            &mut self.battle_trainer_number,
            &mut self.battle_player_mon_species,
            &mut self.battle_player_mon_hp,
            &mut self.battle_enemy_species,
            &mut self.battle_enemy_level,
            &mut self.item_count,
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
        ] {
            fix_keys(store, list, &mut invalid);
        }
        dedupe(invalid)
    }
}

// ---------------------------------------------------------------------------
// converter
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Gen1Converter {
    pub red_blue: bool,
}

impl Gen1Converter {
    const GAME_VITAMINS: [&'static str; 5] = ["HP UP", "PROTEIN", "IRON", "CARBOS", "CALCIUM"];
    const GAME_RARE_CANDY: &'static str = "RARE CANDY";

    pub fn is_game_vitamin(&self, item_name: &str) -> bool {
        Self::GAME_VITAMINS.contains(&item_name)
    }

    pub fn is_game_rare_candy(&self, item_name: &str) -> bool {
        item_name == Self::GAME_RARE_CANDY
    }

    pub fn is_game_tm(&self, item_name: &str) -> bool {
        item_name.starts_with("TM")
    }

    pub fn get_hm_name(&self, gh_move_name: &str) -> Option<&'static str> {
        match name_prettify(gh_move_name).as_str() {
            "Cut" => Some("HM01 Cut"),
            "Fly" => Some("HM02 Fly"),
            "Surf" => Some("HM03 Surf"),
            "Strength" => Some("HM04 Strength"),
            "Flash" => Some("HM05 Flash"),
            _ => None,
        }
    }

    pub fn item_name_convert(&self, gh_item_name: Option<&str>) -> Option<String> {
        let gh_item_name = gh_item_name?;
        let mut converted = name_prettify(&gh_item_name.replace('é', "e"));
        if let Some(rest) = converted.strip_prefix("Tm") {
            converted = format!("TM{}", rest);
        }
        if let Some(rest) = converted.strip_prefix("Hm") {
            converted = format!("HM{}", rest);
        }
        if converted.starts_with("HM") || converted.starts_with("TM") {
            let parts: Vec<&str> = converted.splitn(2, ':').collect();
            if parts.len() == 2 {
                let move_name = self.move_name_convert(Some(parts[1].trim())).unwrap_or_default();
                converted = format!("{} {}", parts[0], move_name);
            }
            // (Python raises when there is no ':'; we keep the name as is)
        } else if converted == "Thunderstone" {
            converted = "Thunder Stone".into();
        } else if converted == "Hp Up" {
            converted = "HP Up".into();
        } else if converted == "Pp Up" {
            converted = "PP Up".into();
        } else if converted == "Guard Spec." {
            converted = "Guard Spec".into();
        } else if converted == "S.s.ticket" {
            converted = "SS Anne Ticket".into();
        }
        Some(converted)
    }

    pub fn move_name_convert(&self, gh_move_name: Option<&str>) -> Option<String> {
        let name = gh_move_name?;
        let converted = name_prettify(&name.replace('-', " "));
        Some(
            match converted.as_str() {
                "Thunderpunch" => "ThunderPunch",
                "Sonicboom" => "SonicBoom",
                "Bubblebeam" => "BubbleBeam",
                "Solarbeam" => "Solar Beam",
                "Poisonpowder" => "PoisonPowder",
                other => other,
            }
            .to_string(),
        )
    }

    pub fn pkmn_name_convert(&self, gh_pkmn_name: Option<&str>) -> Option<String> {
        let name = gh_pkmn_name?;
        Some(
            match name {
                "NidoranM" => "Nidoran_M",
                "NidoranF" => "Nidoran_F",
                "Mr. Mime" => "Mr_Mime",
                "Farfetch'd" => "Farfetchd",
                other => other,
            }
            .to_string(),
        )
    }

    fn trainer_class_convert(&self, gh_trainer_class: Option<&str>) -> Option<String> {
        let c = gh_trainer_class?;
        let converted = name_prettify(c).replace(' ', "");
        Some(if converted == "Lt.surge" { "LtSurge".to_string() } else { converted })
    }

    pub fn trainer_name_convert(&self, trainer_class: Option<&str>, trainer_num: &Value, overworld_map: &str) -> String {
        let trainer_class = self.trainer_class_convert(trainer_class).unwrap_or_else(|| "None".to_string());
        let num = py_str(trainer_num);
        let mut converted = format!("{} {}", trainer_class, num);
        if converted == "JrTrainerM 2" && overworld_map == "Route 25" {
            converted.push_str(" Duplicate");
        } else if converted == "Hiker 11" && overworld_map.starts_with("Rock Tunnel") {
            converted.push_str(" Duplicate");
        } else if converted == "Scientist 4" && overworld_map.starts_with("Cinnabar Mansion") {
            converted.push_str(" Duplicate");
        } else if converted == "Gentleman 3" && overworld_map == "Vermilion City - Gym" {
            converted.push_str(" Duplicate");
        } else if self.red_blue {
            if trainer_class.starts_with("Rival") {
                let n = crate::gamehook::value_as_i64(trainer_num).unwrap_or(0);
                let starter_selector = (n - 1).rem_euclid(3);
                let starter_mon = match starter_selector {
                    0 => "Squirtle",
                    1 => "Bulbasaur",
                    _ => "Charmander",
                };
                let trainer_num = (n - 1).div_euclid(3) + 1;
                converted = if trainer_class == "Rival3" {
                    format!("{} {}", trainer_class, starter_mon)
                } else {
                    format!("{} {} {}", trainer_class, starter_mon, trainer_num)
                };
            }
        } else {
            converted = match converted.as_str() {
                "Rival2 2" => "Rival2 2 Jolteon".into(),
                "Rival2 3" => "Rival2 2 Flareon".into(),
                "Rival2 4" => "Rival2 2 Vaporeon".into(),
                "Rival2 5" => "Rival2 3 Jolteon".into(),
                "Rival2 6" => "Rival2 3 Flareon".into(),
                "Rival2 7" => "Rival2 3 Vaporeon".into(),
                "Rival2 8" => "Rival2 4 Jolteon".into(),
                "Rival2 9" => "Rival2 4 Flareon".into(),
                "Rival2 10" => "Rival2 4 Vaporeon".into(),
                "Rival3 1" => "Rival3 Jolteon".into(),
                "Rival3 2" => "Rival3 Flareon".into(),
                "Rival3 3" => "Rival3 Vaporeon".into(),
                "Rocket 42" => "Jessie & James 1".into(),
                "Rocket 43" => "Jessie & James 2".into(),
                "Rocket 44" => "Jessie & James 3".into(),
                "Rocket 45" => "Jessie & James 4".into(),
                _ => converted,
            };
        }
        converted
    }

    pub fn area_name_convert(&self, area_name: &str) -> String {
        let area_name = area_name.split('-').next().unwrap_or("").trim();
        match area_name {
            "Vermilion Dock" => "Vermilion City".to_string(),
            "Bill's House" => "Route 25".to_string(),
            a if a.starts_with("Rock Tunnel") => "Rock Tunnel".to_string(),
            a if a.starts_with("Safari Zone") => "Safari Zone".to_string(),
            "Lorelei's Room" | "Bruno's Room" | "Agatha's Room" | "Lance's Room" | "Champions Room" => "Indigo Plateau".to_string(),
            other => other.to_string(),
        }
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

#[derive(Default)]
struct BattleData {
    is_trainer_battle: bool,
    waiting_for_init: bool,
    init_delay: i64,
    trainer_name: String,
    defeated_trainer_mons: Vec<EventDefinition>,
    waiting_for_moves: bool,
    move_update_delay: i64,
    waiting_for_items: bool,
    item_update_delay: i64,
    loss_detected: bool,
    evolution_detected: bool,
    initial_money: i64,
}

#[derive(Default)]
struct InventoryData {
    seconds_delay: i64,
    money_gained: bool,
    money_lost: bool,
}

#[derive(Default)]
struct CandyData {
    move_learned: bool,
    item_removal_detected: bool,
}

#[derive(Default)]
struct TmData {
    item_removal_detected: bool,
}

#[derive(Default)]
struct VitaminData {
    item_removal_detected: bool,
}

#[derive(Default)]
struct OverworldData {
    waiting_for_registration: bool,
    register_delay: i64,
    waiting_for_new_file: bool,
    new_file_delay: i64,
    waiting_for_heal_completion: bool,
    heal_delay: i64,
    waiting_for_solo_mon_in_slot_1: bool,
    wrong_mon_delay: i64,
    wrong_mon_in_slot_1: bool,
}

const BASE_DELAY: i64 = 2;

pub struct Gen1Machine {
    controller: Arc<RecorderController>,
    gen: Arc<GenData>,
    keys: Gen1Keys,
    conv: Gen1Converter,
    debug_mode: bool,

    player_id: Option<Value>,
    valid_solo_mon: bool,
    cached_team: Vec<MonKey>,
    cached_items: ItemCache,
    level_up_moves: HashMap<(String, i64), Vec<String>>,
    cached_moves: [Option<String>; 4],
    cached_money: i64,
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
    vitamin: VitaminData,
    overworld: OverworldData,
}

impl Gen1Machine {
    pub fn new(controller: Arc<RecorderController>, info: &StartInfo, red_blue: bool) -> Gen1Machine {
        let mut m = Gen1Machine {
            controller,
            gen: info.gen.clone(),
            keys: Gen1Keys::configure_for_mapper(None),
            conv: Gen1Converter { red_blue },
            debug_mode: info.debug_mode,
            player_id: None,
            valid_solo_mon: false,
            cached_team: Vec::new(),
            cached_items: IndexMap::new(),
            level_up_moves: HashMap::new(),
            cached_moves: [None, None, None, None],
            cached_money: 0,
            solo_mon_key: MonKey {
                species: None,
                attack: Value::Null,
                defense: Value::Null,
                speed: Value::Null,
                special_attack: Value::Null,
                special_defense: Value::Null,
                level: Value::Null,
            },
            cur_state: GameState::Uninitialized,
            active: new_active_flag(),
            queue: EventQueue::new(),
            uninit: UninitData::default(),
            resetting: ResettingData::default(),
            battle: BattleData::default(),
            inventory: InventoryData::default(),
            candy: CandyData::default(),
            tm: TmData::default(),
            vitamin: VitaminData::default(),
            overworld: OverworldData::default(),
        };
        m.solo_mon_key = mon_key_from(info.dvs.as_ref(), info.solo_species.clone());
        m
    }

    /// `_get_route_defined_mon_key` (from the reader thread: asks the host)
    fn route_defined_mon_key(&self) -> MonKey {
        let dvs = self.controller.host().call(|h| h.get_dvs());
        let species = self.controller.host().call(|h| h.final_solo_species());
        mon_key_from(dvs.as_ref(), species)
    }
}

fn mon_key_from(dvs: Option<&xpr_data::model::StatBlock>, species: Option<String>) -> MonKey {
    let v = |x: Option<i64>| x.map(Value::from).unwrap_or(Value::Null);
    MonKey {
            species,
            attack: v(dvs.as_ref().map(|d| d.attack)),
            defense: v(dvs.as_ref().map(|d| d.defense)),
            speed: v(dvs.as_ref().map(|d| d.speed)),
            special_attack: v(dvs.as_ref().map(|d| d.special_attack)),
            special_defense: Value::Null,
            level: Value::Null,
    }
}

impl Gen1Machine {
    fn mon_key(&self, store: &PropertyStore, mon_idx: usize) -> MonKey {
        let species = self.conv.pkmn_name_convert(store.str_of(&self.keys.team_species[mon_idx]).as_deref());
        MonKey {
            species,
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
            // Python checks `move_level not in dict` against tuple keys, so the
            // list is reset before every append: only the last move of a level survives
            self.level_up_moves.insert((new_mon.name.clone(), *move_level), Vec::new());
            self.level_up_moves.get_mut(&(new_mon.name.clone(), *move_level)).unwrap().push(move_name.clone());
        }
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
                self.move_cache_update(store, false, None, false, false);
            }
        }
        log::info!("after team cache update, valid_solo_mon: {}", self.valid_solo_mon);
        self.cached_team = new_cache;
    }

    fn can_evolve_into(&self, species: Option<&str>) -> bool {
        let Some(s) = species else { return false };
        let s = s.to_string();
        self.controller.host().call(move |h| h.can_evolve_into(&s))
    }

    fn update_all_cached_info(&mut self, store: &PropertyStore) {
        self.update_team_cache(store, false, true);
        self.item_cache_update(store, false, false, false, false, false, false);
        self.money_cache_update(store);
        let area = self.conv.area_name_convert(&store.str_of(&self.keys.overworld_map).unwrap_or_default());
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

    fn move_cache_update(&mut self, store: &PropertyStore, generate_events: bool, tm_name: Option<&str>, hm_expected: bool, levelup_source: bool) {
        let new_cache: [Option<String>; 4] = [
            self.conv.move_name_convert(store.str_of(&self.keys.player_moves[0]).as_deref()),
            self.conv.move_name_convert(store.str_of(&self.keys.player_moves[1]).as_deref()),
            self.conv.move_name_convert(store.str_of(&self.keys.player_moves[2]).as_deref()),
            self.conv.move_name_convert(store.str_of(&self.keys.player_moves[3]).as_deref()),
        ];
        if generate_events {
            let old_moves: Vec<&String> = self.cached_moves.iter().flatten().collect();
            let new_moves: Vec<&String> = new_cache.iter().flatten().collect();
            let deleted: Vec<String> = old_moves.iter().filter(|m| !new_moves.contains(m)).map(|m| (*m).clone()).collect();
            let learned: Vec<String> = new_moves.iter().filter(|m| !old_moves.contains(m)).map(|m| (*m).clone()).collect();
            let to_delete_move = if deleted.len() > 1 {
                log::error!("Got multiple deleted moves..? {:?}, from {:?} to {:?}", deleted, self.cached_moves, new_cache);
                deleted.first().cloned()
            } else {
                deleted.first().cloned()
            };
            let to_learn_move = if learned.len() > 1 {
                log::error!("Got multiple learned moves..? {:?}, from {:?} to {:?}", learned, self.cached_moves, new_cache);
                learned.first().cloned()
            } else {
                learned.first().cloned()
            };
            if to_learn_move.is_none() && to_delete_move.is_some() {
                log::error!("Move deleted but no move learned? That seems impossible... from {:?} to {:?}", self.cached_moves, new_cache);
            } else if let Some(learn) = to_learn_move {
                let (source, level) = if levelup_source {
                    (consts::MOVE_SOURCE_LEVELUP.to_string(), LevelVal::Int(store_i64(store, &self.keys.mon_level)))
                } else {
                    let src = if hm_expected {
                        self.conv.get_hm_name(&learn).map(|s| s.to_string()).unwrap_or_else(|| "None".to_string())
                    } else {
                        tm_name.map(|s| s.to_string()).unwrap_or_else(|| "None".to_string())
                    };
                    (src, LevelVal::any())
                };
                let mut lm = LearnMoveEventDefinition::new(
                    self.conv.move_name_convert(Some(&learn)).as_deref(),
                    None,
                    &source,
                    level,
                    self.solo_mon_key.species.as_deref(),
                    false,
                );
                lm.destination_name = to_delete_move.and_then(|d| self.conv.move_name_convert(Some(&d)));
                self.queue_new_event(EventDefinition::with_learn_move(lm));
            }
        }
        self.cached_moves = new_cache;
    }

    fn item_cache(&self, store: &PropertyStore) -> ItemCache {
        let mut new_cache: ItemCache = IndexMap::new();
        for i in 0..20 {
            let item_type = store.value(&self.keys.item_type[i]);
            if item_type.is_null() {
                break;
            }
            new_cache.insert(item_type, store_i64(store, &self.keys.item_quantity[i]));
        }
        new_cache
    }

    fn item_cache_update(&mut self, store: &PropertyStore, generate_events: bool, purchase_expected: bool, sale_expected: bool, vitamin_flag: bool, candy_flag: bool, tm_flag: bool) {
        let new_cache = self.item_cache(store);
        let (gained_items, lost_items) = item_diff(&self.cached_items, &new_cache);
        self.cached_items = new_cache;
        if !generate_events {
            return;
        }
        if !gained_items.is_empty() && sale_expected {
            log::error!("Gained the following items when expecting to be losing items to selling... {}", repr_items(&gained_items));
        }
        for (cur_gained_item, cur_gain_num) in &gained_items {
            let app_item_name = self.conv.item_name_convert(cur_gained_item.as_str()).unwrap_or_else(|| "None".into());
            if app_item_name == OAKS_PARCEL {
                continue;
            } else if VENDING_MACHINE_DRINKS.contains(&app_item_name.as_str()) {
                // vending machines give the item before the money is deducted: force a purchase
                self.queue_new_event(EventDefinition::with_item(InventoryEventDefinition::new(&app_item_name, *cur_gain_num, true, true, None)));
            } else {
                self.queue_new_event(EventDefinition::with_item(InventoryEventDefinition::new(&app_item_name, *cur_gain_num, true, purchase_expected, None)));
            }
        }
        if !lost_items.is_empty() && purchase_expected {
            log::error!("Lost the following items when expecting to be gain items to purchasing... {}", repr_items(&lost_items));
        }
        for (cur_lost_item, cur_lost_num) in &lost_items {
            let raw = cur_lost_item.as_str().unwrap_or("");
            let app_item_name = self.conv.item_name_convert(cur_lost_item.as_str()).unwrap_or_else(|| "None".into());
            if app_item_name == OAKS_PARCEL {
                continue;
            }
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
                self.move_cache_update(store, true, Some(&app_item_name), false, false);
            } else {
                self.queue_new_event(EventDefinition::with_item(InventoryEventDefinition::new(&app_item_name, *cur_lost_num, false, sale_expected, None)));
            }
        }
    }

    // ---- states -------------------------------------------------------------

    fn watch_for_reset(&self, new: &GameHookProperty) -> Option<GameState> {
        if self.player_id.is_some() && new.path == self.keys.player_id && new.eq_i64(0) {
            return Some(GameState::Resetting);
        }
        None
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
                let b = &mut self.battle;
                b.waiting_for_init = true;
                b.init_delay = 0;
                b.defeated_trainer_mons.clear();
                b.waiting_for_moves = false;
                b.move_update_delay = 0;
                b.waiting_for_items = false;
                b.item_update_delay = 0;
                b.trainer_name.clear();
                b.is_trainer_battle = false;
                b.loss_detected = false;
                b.evolution_detected = false;
                b.initial_money = store_i64(store, &self.keys.player_money);
            }
            GameState::InventoryChange => {
                self.inventory.seconds_delay = BASE_DELAY;
                self.inventory.money_gained = self.money_cache_update(store).unwrap_or(false);
                self.inventory.money_lost = false;
            }
            GameState::RareCandy => {
                self.candy.move_learned = false;
                self.candy.item_removal_detected = false;
            }
            GameState::Tm => self.tm.item_removal_detected = false,
            GameState::Vitamin => self.vitamin.item_removal_detected = false,
            GameState::Overworld => {
                self.money_cache_update(store);
                self.update_team_cache(store, true, false);
                let o = &mut self.overworld;
                o.waiting_for_registration = false;
                o.register_delay = 2;
                o.waiting_for_new_file = false;
                o.new_file_delay = 2;
                o.waiting_for_heal_completion = false;
                o.heal_delay = 2;
                o.waiting_for_solo_mon_in_slot_1 = false;
                o.wrong_mon_delay = 2;
                o.wrong_mon_in_slot_1 = false;
            }
            GameState::MoveDelete => {}
        }
    }

    fn on_exit(&mut self, state: GameState, next: GameState, store: &PropertyStore) {
        match state {
            GameState::Uninitialized => {
                let id = store.value(&self.keys.player_id);
                self.player_id = Some(id);
                // exiting into a reset while the id is 0: let ResettingState set it
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
                    if self.battle.loss_detected {
                        let name = self.battle.trainer_name.clone();
                        let mut ev = EventDefinition::with_trainer(TrainerEventDefinition::new(&name));
                        ev.notes = trainer_loss_flag();
                        self.queue_new_event(ev);
                        // the nugget bridge rocket gives the nugget before the fight
                        if name == NUGGET_ROCKET {
                            self.queue_new_event(EventDefinition::with_item(InventoryEventDefinition::new(NUGGET, 1, true, false, None)));
                        }
                        let mons = std::mem::take(&mut self.battle.defeated_trainer_mons);
                        for ev in mons {
                            self.queue_new_event(ev);
                        }
                        // losing the rival lab fight does not black out
                        if !RIVAL_LAB_FIGHTS.contains(&name.as_str()) {
                            self.queue_new_event(EventDefinition::with_blackout());
                        }
                    } else if !self.battle.trainer_name.is_empty() {
                        let mut td = TrainerEventDefinition::new(&self.battle.trainer_name);
                        td.pay_day_amount = Some(store_i64(store, &self.keys.player_money) - self.battle.initial_money);
                        let mut ev = EventDefinition::with_trainer(td);
                        ev.notes = pay_day_flag();
                        self.queue_new_event(ev);
                    }
                    if self.battle.waiting_for_moves {
                        if self.battle.evolution_detected {
                            self.update_team_cache(store, true, false);
                        }
                        self.move_cache_update(store, true, None, false, true);
                    }
                    if self.battle.waiting_for_items {
                        self.item_cache_update(store, true, false, false, false, false, false);
                    }
                }
            }
            GameState::InventoryChange => {
                if next != GameState::Resetting {
                    let (gained, lost) = (self.inventory.money_gained, self.inventory.money_lost);
                    self.item_cache_update(store, true, lost, gained, false, false, false);
                }
            }
            GameState::RareCandy => {
                if next != GameState::Resetting {
                    self.item_cache_update(store, true, false, false, false, true, false);
                    let level = store_i64(store, &self.keys.mon_level);
                    self.solo_mon_levelup(level);
                    self.update_team_cache(store, true, false);
                    if self.candy.move_learned {
                        self.move_cache_update(store, true, None, false, true);
                    }
                }
            }
            GameState::Tm => {
                if next != GameState::Resetting {
                    self.item_cache_update(store, true, false, false, false, false, true);
                }
            }
            GameState::Vitamin => {
                if next != GameState::Resetting {
                    self.item_cache_update(store, true, false, false, true, false, false);
                }
            }
            GameState::Overworld | GameState::MoveDelete => {}
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
                        // Appendix B item 11: Python compared the property *object* to a
                        // string (always false); the intended value comparison is used here
                        let mode = store.value(&self.keys.battle_type);
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
                } else if new.path == self.keys.item_count || self.keys.item_quantity.contains(&new.path) || self.keys.item_type.contains(&new.path) {
                    self.inventory.seconds_delay = BASE_DELAY;
                } else if new.path == self.keys.gametime_seconds {
                    // vending machines deduct money after the rumble sound effect: wait for it
                    if !store.get(&self.keys.audio_channel_7).map(|p| p.eq_i64(168)).unwrap_or(false) {
                        if self.inventory.seconds_delay <= 0 {
                            return GameState::Overworld;
                        } else {
                            self.inventory.seconds_delay -= 1;
                        }
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
                } else if self.keys.item_type.contains(&new.path) && (new.is_null() || new.eq_str(END_OF_ITEM_LIST)) {
                    return GameState::Overworld;
                }
                state
            }
            GameState::Tm => {
                if new.path == self.keys.item_count {
                    self.tm.item_removal_detected = true;
                } else if self.keys.item_quantity.contains(&new.path) {
                    if !self.tm.item_removal_detected {
                        return GameState::Overworld;
                    }
                } else if self.keys.item_type.contains(&new.path) && (new.is_null() || new.eq_str(END_OF_ITEM_LIST)) {
                    return GameState::Overworld;
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
                } else if self.keys.item_type.contains(&new.path) && (new.is_null() || new.eq_str(END_OF_ITEM_LIST)) {
                    return GameState::Overworld;
                }
                state
            }
            GameState::Overworld => self.overworld_transition(store, new, prev),
            GameState::MoveDelete => state,
        }
    }

    fn create_initial_trainer_event(&mut self, store: &PropertyStore) {
        if self.battle.is_trainer_battle {
            let name = self.conv.trainer_name_convert(
                store.str_of(&self.keys.battle_trainer_class).as_deref(),
                &store.value(&self.keys.battle_trainer_number),
                &store.str_of(&self.keys.overworld_map).unwrap_or_else(|| "None".into()),
            );
            self.battle.trainer_name = name.clone();
            self.queue_new_event(EventDefinition::with_trainer(TrainerEventDefinition::new(&name)));
        }
    }

    fn battle_transition(&mut self, store: &PropertyStore, new: &GameHookProperty, _prev: &GameHookProperty) -> GameState {
        let state = GameState::Battle;
        if new.path == self.keys.mon_exppoints {
            let species = self.conv.pkmn_name_convert(store.str_of(&self.keys.battle_enemy_species).as_deref()).unwrap_or_else(|| "None".into());
            let level = store_i64(store, &self.keys.battle_enemy_level);
            if self.battle.is_trainer_battle {
                self.battle.defeated_trainer_mons.push(EventDefinition::with_wild(WildPkmnEventDefinition::new(&species, level, 1, true)));
            } else {
                self.queue_new_event(EventDefinition::with_wild(WildPkmnEventDefinition::new(&species, level, 1, false)));
            }
        } else if new.path == self.keys.battle_player_mon_hp {
            if new.as_i64().map(|v| v <= 0).unwrap_or(false) {
                self.battle.loss_detected = true;
            }
        } else if new.path == self.keys.mon_species {
            self.battle.evolution_detected = true;
        } else if self.keys.player_moves.contains(&new.path) {
            if !self.battle.waiting_for_moves {
                self.battle.move_update_delay = 2;
                self.battle.waiting_for_moves = true;
            }
        } else if self.keys.item_quantity.contains(&new.path) || self.keys.item_type.contains(&new.path) || new.path == self.keys.item_count {
            if !self.battle.waiting_for_items {
                self.battle.item_update_delay = 2;
                self.battle.waiting_for_items = true;
            }
        } else if new.path == self.keys.gametime_seconds {
            if self.battle.waiting_for_init {
                if self.battle.init_delay <= 0 {
                    self.battle.waiting_for_init = false;
                    self.battle.is_trainer_battle = store.get(&self.keys.battle_type).map(|p| p.eq_str(TRAINER_BATTLE_TYPE)).unwrap_or(false);
                    if self.battle.is_trainer_battle {
                        self.create_initial_trainer_event(store);
                    }
                } else {
                    self.battle.init_delay -= 1;
                }
            }
            if self.battle.waiting_for_moves {
                if self.battle.move_update_delay <= 0 {
                    self.battle.waiting_for_moves = false;
                    if self.battle.evolution_detected {
                        self.update_team_cache(store, true, false);
                    }
                    self.move_cache_update(store, true, None, false, true);
                } else {
                    self.battle.move_update_delay -= 1;
                }
            }
            if self.battle.waiting_for_items {
                if self.battle.item_update_delay <= 0 {
                    self.battle.waiting_for_items = false;
                    self.item_cache_update(store, true, false, false, false, false, false);
                } else {
                    self.battle.item_update_delay -= 1;
                }
            }
        } else if new.path == self.keys.mon_level {
            self.solo_mon_levelup(new.as_i64().unwrap_or(0));
        } else if new.path == self.keys.battle_type {
            if self.keys.is_none_battle_type(new) {
                return GameState::Overworld;
            }
        }
        state
    }

    fn overworld_transition(&mut self, store: &PropertyStore, new: &GameHookProperty, prev: &GameHookProperty) -> GameState {
        let state = GameState::Overworld;
        // intentionally ignore all updates while waiting for a new file
        if self.overworld.waiting_for_new_file || self.overworld.waiting_for_solo_mon_in_slot_1 {
            if new.path == self.keys.gametime_seconds {
                if self.overworld.new_file_delay <= 0 {
                    self.overworld.waiting_for_new_file = false;
                    self.controller.route_restarted();
                    self.update_all_cached_info(store);
                }
                self.overworld.new_file_delay -= 1;
            }
            return state;
        }
        if new.path == self.keys.battle_type {
            return GameState::Battle;
        } else if new.path == self.keys.overworld_map {
            let area = self.conv.area_name_convert(new.as_str().unwrap_or(""));
            self.controller.entered_new_area(&area);
        } else if new.path == self.keys.player_id {
            if prev.truthy() && self.player_id.as_ref() != Some(&store.value(&self.keys.player_id)) {
                self.overworld.waiting_for_new_file = true;
            }
        } else if new.path == self.keys.item_count || self.keys.item_quantity.contains(&new.path) || self.keys.item_type.contains(&new.path) {
            return GameState::InventoryChange;
        } else if new.path == self.keys.player_money {
            let cur_location = store.str_of(&self.keys.overworld_map).unwrap_or_default();
            if new.as_i64().unwrap_or(0) > prev.as_i64().unwrap_or(0) && cur_location != MAP_GAME_CORNER {
                // configure the cached money so the sale check sees the gain
                self.cached_money = prev.as_i64().unwrap_or(0);
                return GameState::InventoryChange;
            }
        } else if new.path == self.keys.mon_species {
            if !prev.truthy() {
                self.overworld.waiting_for_registration = true;
            } else if self.solo_mon_key.species == self.conv.pkmn_name_convert(prev.as_str()) {
                self.overworld.wrong_mon_in_slot_1 = true;
            } else if self.solo_mon_key.species == self.conv.pkmn_name_convert(new.as_str()) {
                self.overworld.wrong_mon_delay = 2;
                self.overworld.waiting_for_solo_mon_in_slot_1 = true;
            }
        } else if new.path == self.keys.mon_level {
            if !self.overworld.waiting_for_registration && !self.overworld.wrong_mon_in_slot_1 {
                return GameState::RareCandy;
            }
        } else if self.keys.player_moves.contains(&new.path) {
            if !self.overworld.waiting_for_registration && !self.overworld.wrong_mon_in_slot_1 {
                if self.conv.get_hm_name(new.as_str().unwrap_or("")).is_some() {
                    self.move_cache_update(store, true, None, true, false);
                } else {
                    return GameState::Tm;
                }
            }
        } else if self.keys.stat_exp.contains(&new.path) {
            if !self.overworld.waiting_for_registration && !self.overworld.wrong_mon_in_slot_1 {
                return GameState::Vitamin;
            }
        } else if new.path == self.keys.audio_channel_5 {
            if new.eq_i64(158) && !self.overworld.waiting_for_heal_completion {
                let loc = store.str_of(&self.keys.overworld_map).unwrap_or_else(|| "None".into());
                self.queue_new_event(EventDefinition::with_heal(&loc));
                self.overworld.waiting_for_heal_completion = true;
                self.overworld.heal_delay = 2;
            } else if new.eq_i64(182) {
                let loc = store.str_of(&self.keys.overworld_map).unwrap_or_else(|| "None".into());
                self.queue_new_event(EventDefinition::with_save(&loc));
            }
        } else if new.path == self.keys.audio_channel_4 {
            // channel 4 for R/B pokecenters, channel 5 for Y
            if new.eq_i64(158) && !self.overworld.waiting_for_heal_completion {
                let loc = store.str_of(&self.keys.overworld_map).unwrap_or_else(|| "None".into());
                self.queue_new_event(EventDefinition::with_heal(&loc));
                self.overworld.waiting_for_heal_completion = true;
                self.overworld.heal_delay = 2;
            }
        } else if new.path == self.keys.gametime_seconds {
            if self.overworld.waiting_for_registration {
                if self.overworld.register_delay <= 0 {
                    self.overworld.waiting_for_registration = false;
                    self.update_team_cache(store, true, true);
                }
                self.overworld.register_delay -= 1;
            }
            if self.overworld.waiting_for_heal_completion {
                if self.overworld.heal_delay <= 0 {
                    self.overworld.waiting_for_heal_completion = false;
                }
                self.overworld.heal_delay -= 1;
            }
            if self.overworld.waiting_for_solo_mon_in_slot_1 {
                if self.overworld.wrong_mon_delay <= 0 {
                    self.overworld.waiting_for_solo_mon_in_slot_1 = false;
                    self.overworld.wrong_mon_in_slot_1 = false;
                }
                self.overworld.wrong_mon_delay -= 1;
            }
        }
        state
    }

    // ---- processing thread ------------------------------------------------------

    fn spawn_processing_thread(&self) {
        let ctx = ProcessCtx {
            controller: self.controller.clone(),
            gen: self.gen.clone(),
            queue: self.queue.clone(),
            active: self.active.clone(),
        };
        std::thread::Builder::new()
            .name("gen1-recorder-events".into())
            .spawn(move || ctx.run(process_one))
            .ok();
    }
}

/// `_process_events` body for one event.
fn process_one(ctx: &ProcessCtx, mut cur_event: EventDefinition) {
    let gen = &ctx.gen;
    if cur_event.notes == reset_flag() {
        log::info!("Resetting to last save...");
        ctx.controller.game_reset();
        return;
    } else if let Some(td) = cur_event.trainer_def.clone() {
        let Some(trainer) = gen.trainer_db().get_trainer(&td.trainer_name).cloned() else {
            ctx.add_error(format!("Failed to find trainer from GameHook: {}", td.trainer_name));
            return;
        };
        if cur_event.notes == trainer_loss_flag() {
            log::info!("Handling trainer loss: {}", td.trainer_name);
            ctx.controller.lost_trainer_battle(&td.trainer_name);
            return;
        } else if cur_event.notes == pay_day_flag() {
            let name = td.trainer_name.clone();
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
                    if let Some(t) = cur_event.trainer_def.as_mut() {
                        t.pay_day_amount = Some(t.pay_day_amount.unwrap_or(0) - trainer.money);
                        log::info!("Updating pay day for trainer {} to {:?}", t.trainer_name, t.pay_day_amount);
                    }
                    let id = obj.group_id;
                    let ev = cur_event.clone();
                    ctx.controller.host().call(move |h| h.update_existing_event(id, ev));
                }
            }
            // PAY_DAY_FLAG is only queued on a win in gen 1
            ctx.controller.check_final_trainer(&td.trainer_name);
            return;
        }
    } else if let Some(item_def) = cur_event.item_event_def.clone() {
        let Some(item) = gen.item_db().get_item(&item_def.item_name).cloned() else {
            ctx.add_error(format!("Failed to find item from GameHook: {} for event {}", item_def.item_name, event_str(gen, &cur_event)));
            return;
        };
        if item_def.is_acquire {
            let prev_event = ctx.controller.host().call(|h| h.get_previous_event(None));
            if let Some(prev) = &prev_event {
                if prev.definition.trainer_def.is_some() {
                    let reward = prev.first_trainer_name.as_deref().and_then(|n| gen.get_fight_reward(n));
                    if reward == Some(item_def.item_name.as_str()) {
                        log::info!("Intentionally ignoring item add for battle reward: {}", item_def.item_name);
                        return;
                    }
                }
            }
            let name = item.name.clone();
            if item.is_key_item && ctx.controller.host().call(move |h| h.final_inventory_has(&name)) {
                log::info!("Intentionally ignoring item add for duplicate key item: {}", item_def.item_name);
                return;
            }
        }
    } else if let Some(lm) = cur_event.learn_move.clone() {
        let to_learn = lm.move_to_learn.as_deref().and_then(|m| gen.move_db().get_move(m));
        let to_forget = lm.destination_name.as_deref().and_then(|m| gen.move_db().get_move(m));
        if to_learn.is_none() {
            ctx.add_error(format!(
                "Failed to find move from GameHook: {} for event {}",
                lm.move_to_learn.as_deref().unwrap_or("None"),
                event_str(gen, &cur_event)
            ));
            return;
        } else if lm.destination_name.is_some() && to_forget.is_none() {
            ctx.add_error(format!(
                "Failed to find move from GameHook: {} for event {}",
                lm.destination_name.as_deref().unwrap_or("None"),
                event_str(gen, &cur_event)
            ));
            return;
        }
    } else if let Some(w) = &cur_event.wild_pkmn_info {
        if gen.pkmn_db().get_pkmn(&w.name).is_none() {
            ctx.add_error(format!("Failed to find wild pokemon from GameHook: {} for event {}", w.name, event_str(gen, &cur_event)));
            return;
        }
    }
    log::info!("adding new event: {}", event_str(gen, &cur_event));
    ctx.controller.add_event(cur_event);
}

impl GameRecorder for Gen1Machine {
    fn on_mapper_loaded(&mut self, store: &PropertyStore) -> (Vec<String>, Vec<String>) {
        self.keys = Gen1Keys::configure_for_mapper(store.game_name());
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
        if self.debug_mode
            && new.path != self.keys.audio_channel_4
            && new.path != self.keys.audio_channel_5
            && new.path != self.keys.audio_channel_7
            && new.path != self.keys.gametime_seconds
        {
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

    fn shutdown(&mut self) {
        log::info!("Shutting down Yellow recording FSM");
        self.active.store(false, Ordering::SeqCst);
        crate::shuckie::supershuckie().stop();
    }
}
