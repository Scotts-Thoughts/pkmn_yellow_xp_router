//! JSON loaders for the five generation schemas (`_load_pkmn_db`,
//! `_load_trainer_db`, `_load_item_db`, `_load_move_db` and the type/fight
//! info blocks of each `gen_X_object.py`), with each gen's quirks.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::Value;

use xpr_core::consts;
use xpr_core::pyjson;

use crate::db::{ItemDB, MinBattlesDB, MoveDB, PkmnDB, TrainerDB};
use crate::gen_data::{GenData, TypeChart};
use crate::model::{BaseItem, EnemyPkmn, Gen, Move, MoveEffect, Nature, PokemonSpecies, StatBlock, Trainer, TrainerTimingStats};

/// Where a generation's files live.
#[derive(Clone, Debug)]
pub struct GenSources {
    pub pkmn_db: PathBuf,
    pub trainer_db: PathBuf,
    pub items: PathBuf,
    pub moves: PathBuf,
    pub type_info: PathBuf,
    pub fights_info: PathBuf,
    /// Empty path for gens without min-battle presets.
    pub min_battles: PathBuf,
}

/// Reads data files; lets the registry serve embedded copies of the
/// built-in data without touching the disk.
pub trait DataReader {
    fn read_bytes(&self, path: &Path) -> Result<Vec<u8>, String>;

    /// Names of the files directly inside `dir` (sorted); empty when the
    /// directory does not exist.
    fn list_dir(&self, dir: &Path) -> Vec<String>;

    fn read_text(&self, path: &Path) -> Result<String, String> {
        self.read_bytes(path).map(|bytes| pyjson::decode_text(&bytes))
    }
}

pub struct DiskReader;

impl DataReader for DiskReader {
    fn read_bytes(&self, path: &Path) -> Result<Vec<u8>, String> {
        std::fs::read(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                format!("[Errno 2] No such file or directory: {}", pyjson::python_repr_str(&path.display().to_string()))
            } else {
                e.to_string()
            }
        })
    }

    fn list_dir(&self, dir: &Path) -> Vec<String> {
        let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
        let mut names: Vec<String> = rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        // os.listdir order is arbitrary; Windows returns sorted names
        names.sort();
        names
    }
}

fn load_json(reader: &dyn DataReader, path: &Path) -> Result<Value, String> {
    let text = reader.read_text(path)?;
    pyjson::loads(&text).map_err(|e| e.to_string())
}

/// `raw[key]` with Python's KeyError text (`'key'`).
fn req<'a>(v: &'a Value, key: &str) -> Result<&'a Value, String> {
    pyjson::get(v, key).ok_or_else(|| pyjson::python_repr_str(key))
}

fn req_str(v: &Value, key: &str) -> Result<String, String> {
    let x = req(v, key)?;
    match x {
        Value::String(s) => Ok(s.clone()),
        Value::Null => Ok(String::new()),
        other => Ok(pyjson::python_str_number(other)),
    }
}

fn req_i64(v: &Value, key: &str) -> Result<i64, String> {
    let x = req(v, key)?;
    pyjson::value_as_i64(x).ok_or_else(|| format!("invalid integer for {}: {}", key, x))
}

/// Like `req_i64` but tolerates `null` (Python stores `None` and only fails
/// later, when the value is used in arithmetic; e.g. a backport species
/// with no base experience that is never fought).
fn req_i64_lenient(v: &Value, key: &str) -> Result<i64, String> {
    let x = req(v, key)?;
    match x {
        Value::Null => {
            log::warn!("{} is null; treating as 0", key);
            Ok(0)
        }
        other => pyjson::value_as_i64(other).ok_or_else(|| format!("invalid integer for {}: {}", key, other)),
    }
}

fn opt_i64(v: &Value, key: &str) -> Result<Option<i64>, String> {
    let x = req(v, key)?;
    match x {
        Value::Null => Ok(None),
        other => Ok(pyjson::value_as_i64(other)),
    }
}

fn req_bool(v: &Value, key: &str) -> Result<bool, String> {
    Ok(pyjson::truthy(req(v, key)?))
}

fn str_list(v: &Value) -> Vec<String> {
    match v {
        Value::Array(items) => items
            .iter()
            .map(|x| match x {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                other => pyjson::python_str_number(other),
            })
            .collect(),
        Value::String(s) => vec![s.clone()],
        _ => Vec::new(),
    }
}

fn opt_str_list(v: &Value) -> Vec<Option<String>> {
    match v {
        Value::Array(items) => items
            .iter()
            .map(|x| match x {
                Value::String(s) => Some(s.clone()),
                Value::Null => None,
                other => Some(pyjson::python_str_number(other)),
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn levelup_list(v: &Value) -> Vec<(i64, String)> {
    let mut out = Vec::new();
    if let Value::Array(items) = v {
        for entry in items {
            if let Value::Array(pair) = entry {
                if pair.len() >= 2 {
                    let lvl = pyjson::value_as_i64(&pair[0]).unwrap_or(0);
                    let mv = match &pair[1] {
                        Value::String(s) => s.clone(),
                        other => pyjson::python_str_number(other),
                    };
                    out.push((lvl, mv));
                }
            }
        }
    }
    out
}

/// `raw_db.get("pokemon", raw_db.values())`: a list under a key, or the
/// values of a top-level dict.
fn records<'a>(raw: &'a Value, key: &str) -> Vec<&'a Value> {
    if let Some(Value::Array(items)) = pyjson::get(raw, key) {
        return items.iter().collect();
    }
    match raw {
        Value::Object(o) => o.values().collect(),
        Value::Array(items) => items.iter().collect(),
        _ => Vec::new(),
    }
}

fn effects_list(v: Option<&Value>) -> Vec<MoveEffect> {
    match v {
        Some(Value::Array(items)) => items.iter().filter(|x| x.is_object()).map(MoveEffect::from_json).collect(),
        _ => Vec::new(),
    }
}

fn move_name_of(raw_item_name: &str) -> Option<String> {
    if raw_item_name.starts_with("TM") || raw_item_name.starts_with("HM") {
        raw_item_name.split_once(' ').map(|(_, rest)| rest.to_string())
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// species
// ---------------------------------------------------------------------------

fn load_pkmn_db(gen: Gen, raw: &Value) -> Result<Vec<PokemonSpecies>, String> {
    let mut result: IndexMap<String, PokemonSpecies> = IndexMap::new();
    for cur in records(raw, "pokemon") {
        let species = match gen {
            Gen::One => {
                let base = StatBlock::new(
                    gen,
                    req_i64(cur, consts::BASE_HP_KEY)?,
                    req_i64(cur, consts::BASE_ATK_KEY)?,
                    req_i64(cur, consts::BASE_DEF_KEY)?,
                    req_i64(cur, consts::BASE_SPC_KEY)?,
                    req_i64(cur, consts::BASE_SPC_KEY)?,
                    req_i64(cur, consts::OLD_BASE_SPD_KEY)?,
                    false,
                );
                PokemonSpecies {
                    name: req_str(cur, consts::NAME_KEY)?,
                    growth_rate: req_str(cur, consts::GROWTH_RATE_KEY)?,
                    base_xp: req_i64_lenient(cur, consts::BASE_XP_KEY)?,
                    first_type: req_str(cur, consts::FIRST_TYPE_KEY)?,
                    second_type: req_str(cur, consts::SECOND_TYPE_KEY)?,
                    stats: base,
                    initial_moves: str_list(req(cur, consts::INITIAL_MOVESET_KEY)?),
                    levelup_moves: levelup_list(req(cur, consts::LEARNED_MOVESET_KEY)?),
                    tmhm_moves: str_list(req(cur, consts::TM_HM_LEARNSET_KEY)?),
                    stat_xp_yield: base,
                    abilities: vec![String::new()],
                    weight: None,
                }
            }
            Gen::Two => {
                let base = StatBlock::new(
                    gen,
                    req_i64(cur, consts::BASE_HP_KEY)?,
                    req_i64(cur, consts::BASE_ATK_KEY)?,
                    req_i64(cur, consts::BASE_DEF_KEY)?,
                    req_i64(cur, consts::BASE_SPA_KEY)?,
                    req_i64(cur, consts::BASE_SPD_KEY)?,
                    req_i64(cur, consts::OLD_BASE_SPD_KEY)?,
                    false,
                );
                PokemonSpecies {
                    name: req_str(cur, consts::NAME_KEY)?,
                    growth_rate: req_str(cur, consts::GROWTH_RATE_KEY)?,
                    base_xp: req_i64_lenient(cur, consts::BASE_XP_KEY)?,
                    first_type: req_str(cur, consts::FIRST_TYPE_KEY)?,
                    second_type: req_str(cur, consts::SECOND_TYPE_KEY)?,
                    stats: base,
                    initial_moves: Vec::new(),
                    levelup_moves: levelup_list(req(cur, consts::LEARNED_MOVESET_KEY)?),
                    tmhm_moves: str_list(req(cur, consts::TM_HM_LEARNSET_KEY)?),
                    stat_xp_yield: base,
                    abilities: vec![String::new()],
                    weight: None,
                }
            }
            Gen::Three => PokemonSpecies {
                name: req_str(cur, consts::NAME_KEY)?,
                growth_rate: req_str(cur, consts::GROWTH_RATE_KEY)?,
                base_xp: req_i64_lenient(cur, consts::BASE_XP_KEY)?,
                first_type: req_str(cur, consts::FIRST_TYPE_KEY)?,
                second_type: req_str(cur, consts::SECOND_TYPE_KEY)?,
                stats: StatBlock::new(
                    gen,
                    req_i64(cur, consts::BASE_HP_KEY)?,
                    req_i64(cur, consts::BASE_ATK_KEY)?,
                    req_i64(cur, consts::BASE_DEF_KEY)?,
                    req_i64(cur, consts::BASE_SPA_KEY)?,
                    req_i64(cur, consts::BASE_SPD_KEY)?,
                    req_i64(cur, consts::OLD_BASE_SPD_KEY)?,
                    false,
                ),
                initial_moves: Vec::new(),
                levelup_moves: levelup_list(req(cur, consts::LEARNED_MOVESET_KEY)?),
                tmhm_moves: str_list(req(cur, consts::TM_HM_LEARNSET_KEY)?),
                stat_xp_yield: StatBlock::new(
                    gen,
                    req_i64(cur, consts::EV_YIELD_HP_KEY)?,
                    req_i64(cur, consts::EV_YIELD_ATK_KEY)?,
                    req_i64(cur, consts::EV_YIELD_DEF_KEY)?,
                    req_i64(cur, consts::EV_YIELD_SPC_ATK_KEY)?,
                    req_i64(cur, consts::EV_YIELD_SPC_DEF_KEY)?,
                    req_i64(cur, consts::EV_YIELD_SPD_KEY)?,
                    false,
                ),
                abilities: str_list(req(cur, consts::ABILITY_LIST_KEY)?),
                weight: None,
            },
            Gen::Four | Gen::Five => {
                let bs = req(cur, consts::BASE_STATS_KEY)?;
                let ev = req(cur, consts::EV_YIELD_KEY)?;
                PokemonSpecies {
                    name: req_str(cur, consts::SPECIES_KEY)?,
                    growth_rate: req_str(cur, consts::GROWTH_RATE_KEY)?,
                    base_xp: req_i64_lenient(cur, consts::BASE_EXPERIENCE_KEY)?,
                    first_type: req_str(cur, consts::FIRST_TYPE_KEY)?,
                    second_type: req_str(cur, consts::SECOND_TYPE_KEY)?,
                    stats: StatBlock::new(
                        gen,
                        req_i64(bs, consts::HP)?,
                        req_i64(bs, consts::ATTACK)?,
                        req_i64(bs, consts::DEFENSE)?,
                        req_i64(bs, consts::SPECIAL_ATTACK)?,
                        req_i64(bs, consts::SPECIAL_DEFENSE)?,
                        req_i64(bs, consts::SPEED)?,
                        false,
                    ),
                    initial_moves: Vec::new(),
                    levelup_moves: levelup_list(req(cur, consts::LEVEL_UP_MOVESET_KEY)?),
                    tmhm_moves: str_list(req(cur, consts::TM_HM_LEARNSET_KEY)?),
                    stat_xp_yield: StatBlock::new(
                        gen,
                        req_i64(ev, consts::HP)?,
                        req_i64(ev, consts::ATTACK)?,
                        req_i64(ev, consts::DEFENSE)?,
                        req_i64(ev, consts::SPECIAL_ATTACK)?,
                        req_i64(ev, consts::SPECIAL_DEFENSE)?,
                        req_i64(ev, consts::SPEED)?,
                        false,
                    ),
                    abilities: str_list(req(cur, consts::ABILITY_LIST_KEY)?),
                    weight: match req(cur, consts::WEIGHT_KEY)? {
                        Value::Null => None,
                        other => pyjson::value_as_f64(other),
                    },
                }
            }
        };
        result.insert(species.name.clone(), species);
    }
    Ok(result.into_values().collect())
}

// ---------------------------------------------------------------------------
// trainers
// ---------------------------------------------------------------------------

const GEN5_TRAINER_MOVE_ALIASES: [(&str, &str); 19] = [
    ("AncientPower", "Ancient Power"),
    ("BubbleBeam", "Bubble Beam"),
    ("DoubleSlap", "Double Slap"),
    ("DragonBreath", "Dragon Breath"),
    ("DynamicPunch", "Dynamic Punch"),
    ("ExtremeSpeed", "Extreme Speed"),
    ("Faint Attack", "Feint Attack"),
    ("FeatherDance", "Feather Dance"),
    ("GrassWhistle", "Grass Whistle"),
    ("Hi Jump Kick", "High Jump Kick"),
    ("PoisonPowder", "Poison Powder"),
    ("Sand-Attack", "Sand Attack"),
    ("Softboiled", "Soft-Boiled"),
    ("SolarBeam", "Solar Beam"),
    ("ThunderPunch", "Thunder Punch"),
    ("ThunderShock", "Thunder Shock"),
    ("Selfdestruct", "Self-Destruct"),
    ("SmokeScreen", "Smokescreen"),
    ("SonicBoom", "Sonic Boom"),
];

fn gen5_species_alias(name: &str) -> String {
    match name {
        "Nidoran\u{246e}" => "Nidoran\u{2640}".to_string(),
        "Nidoran\u{246d}" => "Nidoran\u{2642}".to_string(),
        "Farfetch'd" => "Farfetch\u{2019}d".to_string(),
        other => other.to_string(),
    }
}

fn gen5_move_alias(m: &str) -> String {
    GEN5_TRAINER_MOVE_ALIASES
        .iter()
        .find(|(a, _)| *a == m)
        .map(|(_, b)| b.to_string())
        .unwrap_or_else(|| m.to_string())
}

fn create_trainer(gen: Gen, t: &Value, pkmn_db: &PkmnDB, extract_trainer_id: bool) -> Result<Trainer, String> {
    let mut enemy_pkmn: Vec<EnemyPkmn> = Vec::new();
    let trainer_name = match gen {
        Gen::Four | Gen::Five => req_str(t, consts::TRAINER_NAME)?,
        _ => req_str(t, consts::TRAINER_NAME)?,
    };
    let mons = match req(t, consts::TRAINER_POKEMON)? {
        Value::Array(items) => items.clone(),
        _ => Vec::new(),
    };
    for cur in &mons {
        let mon = match gen {
            Gen::One => {
                let name = req_str(cur, consts::NAME_KEY)?;
                let species = pkmn_db.get_pkmn(&name).ok_or_else(|| {
                    format!(
                        "Invalid mon found when creating trainer: ({}, {})",
                        pyjson::python_repr_str(&trainer_name),
                        pyjson::python_repr_str(&name)
                    )
                })?;
                EnemyPkmn {
                    name: name.clone(),
                    level: req_i64(cur, consts::LEVEL)?,
                    xp: req_i64(cur, consts::XP)?,
                    move_list: opt_str_list(req(cur, consts::MOVES)?),
                    cur_stats: StatBlock::new(
                        gen,
                        req_i64(cur, consts::HP)?,
                        req_i64(cur, consts::ATK)?,
                        req_i64(cur, consts::DEF)?,
                        req_i64(cur, consts::SPC)?,
                        req_i64(cur, consts::SPC)?,
                        req_i64(cur, consts::SPD)?,
                        false,
                    ),
                    base_stats: species.stats,
                    dvs: StatBlock::new(gen, 8, 9, 8, 8, 8, 8, false),
                    stat_xp: StatBlock::zero(gen, false),
                    badges: None,
                    held_item: None,
                    custom_move_data: None,
                    is_trainer_mon: true,
                    exp_split: 1,
                    mon_order: 1,
                    definition_order: 1,
                    ability: String::new(),
                    nature: Nature::HARDY,
                }
            }
            Gen::Two => {
                let name = req_str(cur, consts::NAME_KEY)?;
                let species = pkmn_db
                    .get_pkmn(&name)
                    .ok_or_else(|| "'NoneType' object has no attribute 'stats'".to_string())?;
                let dv = req(t, consts::DVS_KEY)?;
                EnemyPkmn {
                    name: name.clone(),
                    level: req_i64(cur, consts::LEVEL)?,
                    xp: req_i64(cur, consts::XP)?,
                    move_list: opt_str_list(req(cur, consts::MOVES)?),
                    cur_stats: StatBlock::new(
                        gen,
                        req_i64(cur, consts::HP)?,
                        req_i64(cur, consts::ATK)?,
                        req_i64(cur, consts::DEF)?,
                        req_i64(cur, consts::SPA)?,
                        req_i64(cur, consts::SPD)?,
                        req_i64(cur, consts::SPE)?,
                        false,
                    ),
                    base_stats: species.stats,
                    dvs: StatBlock::new(
                        gen,
                        req_i64(dv, consts::HP)?,
                        req_i64(dv, consts::ATK)?,
                        req_i64(dv, consts::DEF)?,
                        req_i64(dv, consts::SPA)?,
                        req_i64(dv, consts::SPA)?,
                        req_i64(dv, consts::SPE)?,
                        false,
                    ),
                    stat_xp: StatBlock::zero(gen, false),
                    badges: None,
                    held_item: match req(cur, consts::HELD_ITEM_KEY)? {
                        Value::String(s) => Some(s.clone()),
                        Value::Null => None,
                        other => Some(pyjson::python_str_number(other)),
                    },
                    custom_move_data: None,
                    is_trainer_mon: true,
                    exp_split: 1,
                    mon_order: 1,
                    definition_order: 1,
                    ability: String::new(),
                    nature: Nature::HARDY,
                }
            }
            Gen::Three => {
                let name = req_str(cur, consts::NAME_KEY)?;
                let species = pkmn_db
                    .get_pkmn(&name)
                    .ok_or_else(|| "'NoneType' object has no attribute 'stats'".to_string())?;
                let iv = req_i64(cur, consts::IVS_KEY)?;
                let nature_idx = req_i64(cur, consts::NATURE_KEY)?;
                EnemyPkmn {
                    name: name.clone(),
                    level: req_i64(cur, consts::LEVEL)?,
                    xp: req_i64(cur, consts::XP)?,
                    move_list: opt_str_list(req(cur, consts::MOVES)?),
                    cur_stats: StatBlock::new(
                        gen,
                        req_i64(cur, consts::HP)?,
                        req_i64(cur, consts::ATK)?,
                        req_i64(cur, consts::DEF)?,
                        req_i64(cur, consts::SPA)?,
                        req_i64(cur, consts::SPD)?,
                        req_i64(cur, consts::SPE)?,
                        false,
                    ),
                    base_stats: species.stats,
                    dvs: StatBlock::new(gen, iv, iv, iv, iv, iv, iv, false),
                    stat_xp: StatBlock::zero(gen, false),
                    badges: None,
                    held_item: match req(cur, consts::HELD_ITEM_KEY)? {
                        Value::String(s) => Some(s.clone()),
                        Value::Null => None,
                        other => Some(pyjson::python_str_number(other)),
                    },
                    custom_move_data: None,
                    is_trainer_mon: true,
                    exp_split: 1,
                    mon_order: 1,
                    definition_order: 1,
                    ability: req_str(cur, consts::ABILITY_KEY)?,
                    nature: Nature::from_index(nature_idx).ok_or_else(|| format!("{} is not a valid Nature", nature_idx))?,
                }
            }
            Gen::Four | Gen::Five => {
                let raw_species = req_str(cur, consts::SPECIES_KEY)?;
                let name = if gen == Gen::Five { gen5_species_alias(&raw_species) } else { raw_species };
                let species = pkmn_db
                    .get_pkmn(&name)
                    .ok_or_else(|| format!("Failed to get mon: {}", name))?;
                let st = req(cur, consts::STATS_KEY)?;
                let ivs = req(cur, consts::IVS_PLURAL_KEY)?;
                let nature_idx = req_i64(cur, consts::NATURE_KEY)?;
                let mut moves = opt_str_list(req(cur, consts::MOVES)?);
                if gen == Gen::Five {
                    moves = moves.into_iter().map(|m| m.map(|s| gen5_move_alias(&s))).collect();
                }
                EnemyPkmn {
                    name: name.clone(),
                    level: req_i64(cur, consts::LEVEL)?,
                    xp: req_i64(cur, consts::EXPERIENCE_YIELD_KEY)?,
                    move_list: moves,
                    cur_stats: StatBlock::new(
                        gen,
                        req_i64(st, consts::HP)?,
                        req_i64(st, consts::ATTACK)?,
                        req_i64(st, consts::DEFENSE)?,
                        req_i64(st, consts::SPECIAL_ATTACK)?,
                        req_i64(st, consts::SPECIAL_DEFENSE)?,
                        req_i64(st, consts::SPEED)?,
                        false,
                    ),
                    base_stats: species.stats,
                    dvs: StatBlock::new(
                        gen,
                        req_i64(ivs, consts::HP)?,
                        req_i64(ivs, consts::ATTACK)?,
                        req_i64(ivs, consts::DEFENSE)?,
                        req_i64(ivs, consts::SPECIAL_ATTACK)?,
                        req_i64(ivs, consts::SPECIAL_DEFENSE)?,
                        req_i64(ivs, consts::SPEED)?,
                        false,
                    ),
                    stat_xp: StatBlock::zero(gen, false),
                    badges: None,
                    held_item: match req(cur, consts::HELD_ITEM_KEY)? {
                        Value::String(s) => Some(s.clone()),
                        Value::Null => None,
                        other => Some(pyjson::python_str_number(other)),
                    },
                    custom_move_data: None,
                    is_trainer_mon: true,
                    exp_split: 1,
                    mon_order: 1,
                    definition_order: 1,
                    ability: req_str(cur, consts::ABILITY_KEY)?,
                    nature: Nature::from_index(nature_idx).ok_or_else(|| format!("{} is not a valid Nature", nature_idx))?,
                }
            }
        };
        enemy_pkmn.push(mon);
    }

    let (trainer_id, location, money, rematch, double_battle) = match gen {
        Gen::One => (
            -1,
            req_str(t, consts::TRAINER_LOC)?,
            req_i64(t, consts::MONEY)?,
            false,
            false,
        ),
        Gen::Two => {
            let id = if extract_trainer_id {
                pyjson::get_i64(t, consts::TRAINER_ID).ok_or_else(|| format!("Issue with {}", trainer_name))?
            } else {
                -1
            };
            (
                id,
                req_str(t, consts::TRAINER_LOC)?,
                req_i64(t, consts::MONEY)?,
                trainer_name.contains("Rematch"),
                false,
            )
        }
        Gen::Three => {
            let id = pyjson::get_i64(t, consts::TRAINER_ID).ok_or_else(|| format!("Issue with {}", trainer_name))?;
            let loc = match req(t, consts::TRAINER_LOC)? {
                v if pyjson::truthy(v) => match v {
                    Value::String(s) => s.clone(),
                    other => pyjson::python_str_number(other),
                },
                _ => String::new(),
            };
            (
                id,
                loc,
                req_i64(t, consts::MONEY)?,
                trainer_name.contains("Rematch"),
                req_bool(t, consts::TRAINER_DOUBLE_BATTLE)?,
            )
        }
        Gen::Four => {
            let id = pyjson::get_i64(t, consts::ROM_ID).ok_or_else(|| format!("Issue with {}", pyjson::get_str(t, consts::NAME_KEY).unwrap_or("")))?;
            (
                id,
                String::new(),
                req_i64(t, consts::MONEY)? * 4,
                trainer_name.contains("Rematch"),
                req_bool(t, consts::TRAINER_DOUBLE_BATTLE)?,
            )
        }
        Gen::Five => {
            let id = pyjson::get_i64(t, consts::ROM_ID).ok_or_else(|| format!("Issue with {}", pyjson::get_str(t, consts::NAME_KEY).unwrap_or("")))?;
            let last_level = enemy_pkmn.last().map(|m| m.level).unwrap_or(0);
            (
                id,
                String::new(),
                req_i64(t, consts::MONEY)? * 4 * last_level,
                trainer_name.contains("Rematch"),
                req_bool(t, consts::TRAINER_DOUBLE_BATTLE)?,
            )
        }
    };

    Ok(Trainer {
        trainer_class: req_str(t, consts::TRAINER_CLASS)?,
        name: trainer_name,
        location,
        money,
        pkmn: enemy_pkmn,
        rematch,
        trainer_id,
        refightable: pyjson::get(t, consts::TRAINER_REFIGHTABLE).map(pyjson::truthy).unwrap_or(false),
        double_battle,
    })
}

fn load_trainer_db(gen: Gen, raw: &Value, pkmn_db: &PkmnDB, extract_trainer_id: bool, path: &Path) -> Result<Vec<Trainer>, String> {
    let mut result: IndexMap<String, Trainer> = IndexMap::new();
    let all = records(raw, "trainers");
    let mut unused_count = 0usize;
    for raw_trainer in &all {
        if req_str(raw_trainer, consts::TRAINER_LOC)? == consts::UNUSED_TRAINER_LOC {
            unused_count += 1;
            continue;
        }
        let name = req_str(raw_trainer, consts::TRAINER_NAME)?;
        if matches!(gen, Gen::Three | Gen::Four | Gen::Five) && result.contains_key(&name) {
            return Err(format!(
                "Multiple trainers with the same name ({}) from trainer file: {}",
                name,
                path.display()
            ));
        }
        let trainer = create_trainer(gen, raw_trainer, pkmn_db, extract_trainer_id)?;
        result.insert(name, trainer);
    }
    if matches!(gen, Gen::Three | Gen::Four | Gen::Five) && all.len() != result.len() + unused_count {
        return Err("Incorrect number of trainers. Some name collisions must exist".to_string());
    }
    Ok(result.into_values().collect())
}

// ---------------------------------------------------------------------------
// items / moves
// ---------------------------------------------------------------------------

fn load_item_db(gen: Gen, raw: &Value) -> Result<Vec<BaseItem>, String> {
    let mut result: IndexMap<String, BaseItem> = IndexMap::new();
    for cur in records(raw, "items") {
        let name = req_str(cur, consts::NAME_KEY)?;
        let marts = match gen {
            Gen::One | Gen::Two | Gen::Three => str_list(req(cur, consts::MARTS)?),
            _ => Vec::new(),
        };
        let item = BaseItem::new(
            name.clone(),
            req_bool(cur, consts::IS_KEY_ITEM)?,
            req_i64(cur, consts::PURCHASE_PRICE)?,
            marts,
            move_name_of(&name),
        );
        result.insert(name, item);
    }
    Ok(result.into_values().collect())
}

fn gen5_effect_to_flavor(effect: Option<&str>) -> Vec<String> {
    match effect {
        Some("high_crit_rate") | Some("may_burn_and_high_crit_rate") | Some("may_poison_and_high_crit_rate") => {
            vec![consts::FLAVOR_HIGH_CRIT.to_string()]
        }
        Some("two_to_five_hits") => vec![consts::FLAVOR_MULTI_HIT.to_string()],
        Some("two_hits") | Some("two_hits_and_may_poison") => vec![consts::DOUBLE_HIT_FLAVOR.to_string()],
        Some("psywave") => vec![consts::FLAVOR_PSYWAVE.to_string()],
        Some("fixed_damage") => vec![consts::FLAVOR_FIXED_DAMAGE.to_string()],
        Some("level_based_damage") => vec![consts::FLAVOR_LEVEL_DAMAGE.to_string()],
        _ => Vec::new(),
    }
}

fn load_move_db(gen: Gen, raw: &Value) -> Result<Vec<Move>, String> {
    let mut result: IndexMap<String, Move> = IndexMap::new();
    for cur in records(raw, "moves") {
        let mv = match gen {
            Gen::One => {
                let flavor_raw = req(cur, consts::MOVE_FLAVOR)?;
                let flavor = match flavor_raw {
                    Value::Array(_) => str_list(flavor_raw),
                    Value::Null => Vec::new(),
                    other => vec![match other {
                        Value::String(s) => s.clone(),
                        o => pyjson::python_str_number(o),
                    }],
                };
                Move {
                    name: req_str(cur, consts::NAME_KEY)?,
                    accuracy: opt_i64(cur, consts::MOVE_ACCURACY)?,
                    pp: opt_i64(cur, consts::MOVE_PP)?,
                    base_power: opt_i64(cur, consts::BASE_POWER)?,
                    move_type: req_str(cur, consts::MOVE_TYPE)?,
                    effects: effects_list(pyjson::get(cur, consts::MOVE_EFFECTS)),
                    attack_flavor: flavor,
                    targeting: String::new(),
                    category: String::new(),
                    has_field_effect: false,
                }
            }
            Gen::Two => Move {
                name: req_str(cur, consts::NAME_KEY)?,
                accuracy: opt_i64(cur, consts::MOVE_ACCURACY)?,
                pp: opt_i64(cur, consts::MOVE_PP)?,
                base_power: opt_i64(cur, consts::BASE_POWER)?,
                move_type: req_str(cur, consts::MOVE_TYPE)?,
                effects: effects_list(pyjson::get(cur, consts::MOVE_EFFECTS)),
                attack_flavor: str_list(req(cur, consts::MOVE_FLAVOR)?),
                targeting: String::new(),
                category: String::new(),
                has_field_effect: false,
            },
            Gen::Three => Move {
                name: req_str(cur, consts::NAME_KEY)?,
                accuracy: opt_i64(cur, consts::MOVE_ACCURACY)?,
                pp: opt_i64(cur, consts::MOVE_PP)?,
                base_power: opt_i64(cur, consts::BASE_POWER)?,
                move_type: req_str(cur, consts::MOVE_TYPE)?,
                effects: effects_list(pyjson::get(cur, consts::MOVE_EFFECTS)),
                attack_flavor: str_list(req(cur, consts::MOVE_FLAVOR)?),
                targeting: req_str(cur, consts::MOVE_TARGET)?,
                category: String::new(),
                has_field_effect: false,
            },
            Gen::Four => Move {
                name: req_str(cur, consts::MOVE_KEY)?,
                accuracy: opt_i64(cur, consts::MOVE_ACCURACY)?,
                pp: opt_i64(cur, consts::MOVE_PP)?,
                base_power: opt_i64(cur, consts::POWER)?,
                move_type: req_str(cur, consts::MOVE_TYPE)?,
                effects: effects_list(pyjson::get(cur, consts::MOVE_EFFECTS)),
                attack_flavor: str_list(req(cur, consts::MOVE_FLAVOR)?),
                targeting: req_str(cur, consts::MOVE_TARGET)?,
                category: req_str(cur, consts::MOVE_CATEGORY)?,
                has_field_effect: req_bool(cur, consts::MOVE_HAS_FIELD_EFFECT)?,
            },
            Gen::Five => {
                if pyjson::get_str(cur, consts::MOVE_TYPE) == Some("Shadow") {
                    continue;
                }
                let mut flavor = match pyjson::get(cur, consts::MOVE_FLAVOR) {
                    Some(v) if pyjson::truthy(v) => str_list(v),
                    _ => Vec::new(),
                };
                if flavor.is_empty() {
                    flavor = gen5_effect_to_flavor(pyjson::get_str(cur, "effect"));
                }
                Move {
                    name: req_str(cur, consts::MOVE_KEY)?,
                    accuracy: opt_i64(cur, consts::MOVE_ACCURACY)?,
                    pp: opt_i64(cur, consts::MOVE_PP)?,
                    base_power: opt_i64(cur, consts::POWER)?,
                    move_type: req_str(cur, consts::MOVE_TYPE)?,
                    effects: effects_list(pyjson::get(cur, consts::MOVE_EFFECTS)),
                    attack_flavor: flavor,
                    targeting: req_str(cur, consts::MOVE_TARGET)?,
                    category: req_str(cur, consts::MOVE_CATEGORY)?,
                    has_field_effect: pyjson::get(cur, consts::MOVE_HAS_FIELD_EFFECT).map(pyjson::truthy).unwrap_or(false),
                }
            }
        };
        result.insert(mv.name.clone(), mv);
    }
    Ok(result.into_values().collect())
}

// ---------------------------------------------------------------------------
// type info / fight info
// ---------------------------------------------------------------------------

fn str_map(v: &Value) -> IndexMap<String, String> {
    let mut m = IndexMap::new();
    if let Value::Object(o) = v {
        for (k, val) in o {
            m.insert(k.clone(), match val {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                other => pyjson::python_str_number(other),
            });
        }
    }
    m
}

struct TypeInfo {
    special_types: Vec<String>,
    type_chart: TypeChart,
    held_item_boosts: IndexMap<String, String>,
}

fn load_type_info(gen: Gen, raw: &Value) -> Result<TypeInfo, String> {
    let special_types = match gen {
        Gen::One | Gen::Two | Gen::Three => str_list(req(raw, consts::SPECIAL_TYPES_KEY)?),
        _ => Vec::new(),
    };
    let mut type_chart: TypeChart = IndexMap::new();
    if let Value::Object(o) = req(raw, consts::TYPE_CHART_KEY)? {
        for (k, v) in o {
            type_chart.insert(k.clone(), str_map(v));
        }
    }
    let held_item_boosts = match gen {
        Gen::One => IndexMap::new(),
        _ => str_map(req(raw, consts::HELD_ITEM_BOOSTS_KEY)?),
    };
    Ok(TypeInfo {
        special_types,
        type_chart,
        held_item_boosts,
    })
}

struct FightInfo {
    badge_rewards: IndexMap<String, String>,
    fight_categories: IndexMap<String, Vec<String>>,
    major_fights: HashSet<String>,
    trainer_to_category: IndexMap<String, String>,
    fight_rewards: IndexMap<String, String>,
    branched_mandatory_fights: Vec<String>,
    timing: TrainerTimingStats,
}

fn load_fight_info(raw: &Value) -> Result<FightInfo, String> {
    let badge_rewards = str_map(req(raw, consts::BADGE_REWARDS_KEY)?);
    let raw_major = req(raw, consts::MAJOR_FIGHTS_KEY)?;
    let mut fight_categories = IndexMap::new();
    let mut major_fights = HashSet::new();
    let mut trainer_to_category = IndexMap::new();
    match raw_major {
        Value::Array(_) => {
            for n in str_list(raw_major) {
                major_fights.insert(n);
            }
        }
        Value::Object(o) => {
            for (cat, names) in o {
                let list = str_list(names);
                for n in &list {
                    major_fights.insert(n.clone());
                    trainer_to_category.insert(n.clone(), cat.clone());
                }
                fight_categories.insert(cat.clone(), list);
            }
        }
        _ => {}
    }
    let fight_rewards = str_map(req(raw, consts::FIGHT_REWARDS_KEY)?);
    let branched = pyjson::get(raw, consts::BRANCHED_MANDATORY_FIGHTS_KEY).map(str_list).unwrap_or_default();
    let timing_raw = pyjson::get(raw, consts::TRAINER_TIMING_INFO_KEY);
    let tf = |key: &str, default: f64| -> f64 {
        timing_raw
            .and_then(|t| pyjson::get(t, key))
            .and_then(pyjson::value_as_f64)
            .unwrap_or(default)
    };
    Ok(FightInfo {
        badge_rewards,
        fight_categories,
        major_fights,
        trainer_to_category,
        fight_rewards,
        branched_mandatory_fights: branched,
        timing: TrainerTimingStats {
            intro_time: tf(consts::INTRO_TIME_KEY, consts::DEFAULT_INTRO_TIME),
            outro_time: tf(consts::OUTRO_TIME_KEY, consts::DEFAULT_OUTRO_TIME),
            ko_time: tf(consts::KO_TIME_KEY, consts::DEFAULT_KO_TIME),
            send_out_time: tf(consts::SEND_OUT_TIME_KEY, consts::DEFAULT_SEND_OUT_TIME),
        },
    })
}

// ---------------------------------------------------------------------------
// the GenX constructor
// ---------------------------------------------------------------------------

fn wrap_err(gen: Gen, what: &str, path: &Path, e: String) -> String {
    match gen {
        Gen::One | Gen::Two | Gen::Three => format!("Failed to load {}: {}", what, e),
        Gen::Four | Gen::Five => format!("Error loading {}: {}", what, path.display()),
    }
}

/// `GenX(...)`: load every file, then run the validation passes.
pub fn load_gen(
    gen: Gen,
    sources: &GenSources,
    version_name: &str,
    base_version_name: Option<&str>,
    reader: &dyn DataReader,
) -> Result<GenData, String> {
    let pkmn_raw = load_json(reader, &sources.pkmn_db).map_err(|e| wrap_err(gen, "pokemon DB", &sources.pkmn_db, e))?;
    let pkmn_db = PkmnDB::new(load_pkmn_db(gen, &pkmn_raw).map_err(|e| wrap_err(gen, "pokemon DB", &sources.pkmn_db, e))?);

    let extract_trainer_id = gen == Gen::Two
        && (version_name == consts::CRYSTAL_VERSION || base_version_name == Some(consts::CRYSTAL_VERSION));
    let trainer_raw = load_json(reader, &sources.trainer_db).map_err(|e| wrap_err(gen, "trainer DB", &sources.trainer_db, e))?;
    let trainer_db = TrainerDB::new(
        load_trainer_db(gen, &trainer_raw, &pkmn_db, extract_trainer_id, &sources.trainer_db)
            .map_err(|e| wrap_err(gen, "trainer DB", &sources.trainer_db, e))?,
    );

    let item_what = if gen == Gen::One { "trainer DB" } else { "item DB" };
    let item_raw = load_json(reader, &sources.items).map_err(|e| wrap_err(gen, item_what, &sources.items, e))?;
    let item_db = ItemDB::new(load_item_db(gen, &item_raw).map_err(|e| wrap_err(gen, item_what, &sources.items, e))?);

    let move_raw = load_json(reader, &sources.moves).map_err(|e| wrap_err(gen, "move DB", &sources.moves, e))?;
    let move_db = MoveDB::new(load_move_db(gen, &move_raw).map_err(|e| wrap_err(gen, "move DB", &sources.moves, e))?);

    let min_battles_db = MinBattlesDB::new(&sources.min_battles, reader);

    let type_raw = load_json(reader, &sources.type_info).map_err(|e| wrap_err(gen, "type info", &sources.type_info, e))?;
    let type_info = load_type_info(gen, &type_raw).map_err(|e| wrap_err(gen, "type info", &sources.type_info, e))?;

    let fight_raw = load_json(reader, &sources.fights_info).map_err(|e| format!("Failed to load fight info: {}", e))?;
    let fight_info = load_fight_info(&fight_raw).map_err(|e| format!("Failed to load fight info: {}", e))?;

    let data = GenData {
        gen,
        version_name: version_name.to_string(),
        base_version_name: base_version_name.map(|s| s.to_string()),
        all_flat_files: vec![
            sources.pkmn_db.clone(),
            sources.trainer_db.clone(),
            sources.items.clone(),
            sources.moves.clone(),
            sources.type_info.clone(),
            sources.fights_info.clone(),
        ],
        pkmn_db,
        trainer_db,
        item_db,
        move_db,
        min_battles_db,
        special_types: type_info.special_types,
        type_chart: type_info.type_chart,
        held_item_boosts: type_info.held_item_boosts,
        badge_rewards: Arc::new(fight_info.badge_rewards),
        fight_categories: fight_info.fight_categories,
        major_fights: fight_info.major_fights,
        trainer_to_category: fight_info.trainer_to_category,
        fight_rewards: fight_info.fight_rewards,
        branched_mandatory_fights: fight_info.branched_mandatory_fights,
        trainer_timing_info: fight_info.timing,
    };

    validate(&data)?;
    Ok(data)
}

fn validate(data: &GenData) -> Result<(), String> {
    let supported = data.supported_types();
    match data.gen {
        Gen::One => {
            validate_special_types(data, &supported)?;
            data.move_db.validate_move_types(&supported)?;
            data.item_db.validate_tms_hms(&data.move_db)?;
            validate_fight_rewards(data)?;
            data.pkmn_db.validate_types(&supported)?;
            data.pkmn_db.validate_moves(&data.move_db)?;
            data.trainer_db.validate_trainers(&data.pkmn_db, &data.move_db)?;
        }
        Gen::Two | Gen::Three => {
            validate_special_types(data, &supported)?;
            data.move_db.validate_move_types(&supported)?;
            data.item_db.validate_tms_hms(&data.move_db)?;
            validate_fight_rewards(data)?;
            validate_held_item_boosts(data, &supported)?;
            data.pkmn_db.validate_types(&supported)?;
            data.pkmn_db.validate_moves(&data.move_db)?;
            data.trainer_db.validate_trainers(&data.pkmn_db, &data.move_db)?;
        }
        Gen::Four | Gen::Five => {
            data.move_db.validate_move_types(&supported)?;
            data.item_db.validate_tms_hms(&data.move_db)?;
            validate_fight_rewards(data)?;
            validate_held_item_boosts(data, &supported)?;
            data.pkmn_db.validate_types(&supported)?;
            data.pkmn_db.validate_moves(&data.move_db)?;
            data.trainer_db.validate_trainers(&data.pkmn_db, &data.move_db)?;
        }
    }
    Ok(())
}

fn validate_special_types(data: &GenData, supported: &[String]) -> Result<(), String> {
    let invalid: Vec<String> = data
        .special_types
        .iter()
        .filter(|t| !supported.contains(t))
        .map(|t| pyjson::python_repr_str(t))
        .collect();
    if !invalid.is_empty() {
        return Err(format!("Detected invalid special type(s): [{}]", invalid.join(", ")));
    }
    Ok(())
}

fn validate_held_item_boosts(data: &GenData, supported: &[String]) -> Result<(), String> {
    let mut invalid: Vec<String> = Vec::new();
    for (item, ty) in &data.held_item_boosts {
        if !supported.contains(ty) || data.item_db.get_item(item).is_none() {
            invalid.push(format!("({}, {})", pyjson::python_repr_str(item), pyjson::python_repr_str(ty)));
        }
    }
    if !invalid.is_empty() {
        return Err(format!("Detected invalid item boosts: [{}]", invalid.join(", ")));
    }
    Ok(())
}

fn validate_fight_rewards(data: &GenData) -> Result<(), String> {
    let mut invalid: Vec<String> = Vec::new();
    for (fight, reward) in &data.fight_rewards {
        if data.item_db.get_item(reward).is_none() {
            invalid.push(format!("({}, {})", pyjson::python_repr_str(fight), pyjson::python_repr_str(reward)));
        }
    }
    if !invalid.is_empty() {
        return Err(format!("Invalid Fight Rewards: [{}]", invalid.join(", ")));
    }
    Ok(())
}
