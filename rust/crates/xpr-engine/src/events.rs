//! Port of the definition classes in `routing/route_events.py`: the 11
//! per-type definitions, `EventDefinition`, and their (de)serialization
//! with every legacy shape the saved-route corpus contains.

use indexmap::IndexMap;
use serde_json::Value;

use xpr_core::consts;
use xpr_core::io_utils::sanitize_string;
use xpr_core::pyjson::{self, get, truthy};
use xpr_data::exp;
use xpr_data::model::{CustomMoveData, EnemyPkmn, Trainer};
use xpr_data::GenData;

fn s_or_none(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Null => None,
        other => Some(pyjson::python_str_number(other)),
    }
}

fn str_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => pyjson::python_str_number(other),
    }
}

fn opt_str_value(v: &Option<String>) -> Value {
    match v {
        Some(s) => Value::String(s.clone()),
        None => Value::Null,
    }
}

fn i64_of(v: &Value) -> i64 {
    pyjson::value_as_i64(v).unwrap_or(0)
}

fn str_list(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(items)) => items.iter().map(str_of).collect(),
        _ => Vec::new(),
    }
}

fn i64_list(v: Option<&Value>) -> Vec<i64> {
    match v {
        Some(Value::Array(items)) => items.iter().map(i64_of).collect(),
        _ => Vec::new(),
    }
}

fn i64_list_value(v: &[i64]) -> Value {
    Value::Array(v.iter().map(|x| Value::from(*x)).collect())
}

// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct InventoryEventDefinition {
    pub item_name: String,
    pub item_amount: i64,
    pub is_acquire: bool,
    pub with_money: bool,
    pub custom_price: Option<i64>,
}

impl InventoryEventDefinition {
    pub fn new(item_name: &str, item_amount: i64, is_acquire: bool, with_money: bool, custom_price: Option<i64>) -> Self {
        InventoryEventDefinition {
            item_name: item_name.to_string(),
            item_amount,
            is_acquire,
            with_money,
            custom_price,
        }
    }

    pub fn serialize(&self) -> Value {
        let mut result = vec![
            Value::String(self.item_name.clone()),
            Value::from(self.item_amount),
            Value::Bool(self.is_acquire),
            Value::Bool(self.with_money),
        ];
        if let Some(p) = self.custom_price {
            result.push(pyjson::object(vec![(consts::CUSTOM_PRICE_KEY, Value::from(p))]));
        }
        Value::Array(result)
    }

    pub fn deserialize(raw: Option<&Value>) -> Option<Self> {
        let raw = raw?;
        if !truthy(raw) {
            return None;
        }
        let Value::Array(items) = raw else { return None };
        if items.len() < 4 {
            return None;
        }
        let mut custom_price = None;
        if items.len() > 4 {
            custom_price = match &items[4] {
                Value::Object(_) => get(&items[4], consts::CUSTOM_PRICE_KEY).and_then(pyjson::value_as_i64),
                other => pyjson::value_as_i64(other),
            };
        }
        Some(InventoryEventDefinition {
            item_name: str_of(&items[0]),
            item_amount: i64_of(&items[1]),
            is_acquire: truthy(&items[2]),
            with_money: truthy(&items[3]),
            custom_price,
        })
    }

    pub fn to_string(&self) -> String {
        let action = if self.is_acquire && self.with_money {
            "Purchase"
        } else if self.is_acquire {
            "Find"
        } else if self.with_money {
            "Sell"
        } else {
            "Use/Drop"
        };
        format!("{} {} x{}", action, self.item_name, self.item_amount)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HoldItemEventDefinition {
    pub item_name: Option<String>,
    pub consumed: bool,
}

impl HoldItemEventDefinition {
    pub fn new(item_name: Option<&str>, consumed: bool) -> Self {
        HoldItemEventDefinition {
            item_name: item_name.map(|s| s.to_string()),
            consumed,
        }
    }

    pub fn serialize(&self) -> Value {
        Value::Array(vec![opt_str_value(&self.item_name), Value::Bool(self.consumed)])
    }

    pub fn deserialize(raw: Option<&Value>) -> Option<Self> {
        let raw = raw?;
        if !truthy(raw) {
            return None;
        }
        let Value::Array(items) = raw else { return None };
        if items.is_empty() {
            return None;
        }
        let consumed = if items.len() == 1 { false } else { truthy(&items[1]) };
        Some(HoldItemEventDefinition {
            item_name: s_or_none(&items[0]),
            consumed,
        })
    }

    pub fn to_string(&self) -> String {
        format!("Hold {}", self.item_name.as_deref().unwrap_or("None"))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VitaminEventDefinition {
    pub vitamin: String,
    pub amount: i64,
}

impl VitaminEventDefinition {
    pub fn new(vitamin: &str, amount: i64) -> Self {
        VitaminEventDefinition {
            vitamin: vitamin.to_string(),
            amount,
        }
    }

    pub fn serialize(&self) -> Value {
        Value::Array(vec![Value::String(self.vitamin.clone()), Value::from(self.amount)])
    }

    pub fn deserialize(raw: Option<&Value>) -> Option<Self> {
        let raw = raw?;
        if !truthy(raw) {
            return None;
        }
        match raw {
            Value::String(s) => Some(VitaminEventDefinition::new(s, 1)),
            Value::Array(items) if items.len() >= 2 => Some(VitaminEventDefinition {
                vitamin: str_of(&items[0]),
                amount: i64_of(&items[1]),
            }),
            _ => None,
        }
    }

    pub fn is_ev_berry(&self, gen: &GenData) -> bool {
        gen.is_ev_berry(&self.vitamin)
    }

    pub fn to_string(&self, gen: &GenData) -> String {
        if self.is_ev_berry(gen) {
            format!("Berry {}, x{}", self.vitamin, self.amount)
        } else {
            format!("Vitamin {}, x{}", self.vitamin, self.amount)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RareCandyEventDefinition {
    pub amount: i64,
}

impl RareCandyEventDefinition {
    pub fn new(amount: i64) -> Self {
        RareCandyEventDefinition { amount }
    }

    pub fn serialize(&self) -> Value {
        Value::from(self.amount)
    }

    pub fn deserialize(raw: Option<&Value>) -> Option<Self> {
        let raw = raw?;
        if !truthy(raw) {
            return None;
        }
        if raw == &Value::Bool(true) {
            return Some(RareCandyEventDefinition { amount: 1 });
        }
        Some(RareCandyEventDefinition { amount: i64_of(raw) })
    }

    pub fn to_string(&self) -> String {
        format!("Rare Candy, x{}", self.amount)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WildPkmnEventDefinition {
    pub name: String,
    pub level: i64,
    pub quantity: i64,
    pub trainer_pkmn: bool,
}

impl WildPkmnEventDefinition {
    pub fn new(name: &str, level: i64, quantity: i64, trainer_pkmn: bool) -> Self {
        WildPkmnEventDefinition {
            name: name.to_string(),
            level,
            quantity,
            trainer_pkmn,
        }
    }

    pub fn serialize(&self) -> Value {
        Value::Array(vec![
            Value::String(self.name.clone()),
            Value::from(self.level),
            Value::from(self.quantity),
            Value::Bool(self.trainer_pkmn),
        ])
    }

    pub fn deserialize(raw: Option<&Value>) -> Option<Self> {
        let raw = raw?;
        if !truthy(raw) {
            return None;
        }
        let Value::Array(items) = raw else { return None };
        if items.len() < 2 {
            return None;
        }
        if items.len() == 2 {
            return Some(WildPkmnEventDefinition {
                name: str_of(&items[0]),
                level: i64_of(&items[1]),
                quantity: 1,
                trainer_pkmn: false,
            });
        }
        Some(WildPkmnEventDefinition {
            name: str_of(&items[0]),
            level: i64_of(&items[1]),
            quantity: i64_of(&items[2]),
            trainer_pkmn: items.get(3).map(truthy).unwrap_or(false),
        })
    }

    pub fn to_string(&self) -> String {
        let prefix = if self.trainer_pkmn { "TrainerPkmn" } else { "WildPkmn" };
        format!("{} {}, LV: {}", prefix, self.name, self.level)
    }
}

/// The `level` field of a learn-move definition: an int for level-up moves,
/// `"AnyLevel"` (or another string) for TM/tutor moves.
#[derive(Clone, Debug, PartialEq)]
pub enum LevelVal {
    Int(i64),
    Str(String),
    Other(Value),
}

impl LevelVal {
    pub fn any() -> LevelVal {
        LevelVal::Str(consts::LEVEL_ANY.to_string())
    }

    pub fn from_value(v: &Value) -> LevelVal {
        match v {
            Value::String(s) => LevelVal::Str(s.clone()),
            Value::Number(n) if n.is_i64() || n.is_u64() => LevelVal::Int(n.as_i64().unwrap_or(0)),
            other => LevelVal::Other(other.clone()),
        }
    }

    pub fn to_value(&self) -> Value {
        match self {
            LevelVal::Int(i) => Value::from(*i),
            LevelVal::Str(s) => Value::String(s.clone()),
            LevelVal::Other(v) => v.clone(),
        }
    }

    pub fn is_any(&self) -> bool {
        matches!(self, LevelVal::Str(s) if s == consts::LEVEL_ANY)
    }

    /// Python `int(level)`
    pub fn as_int(&self) -> Option<i64> {
        match self {
            LevelVal::Int(i) => Some(*i),
            LevelVal::Str(s) => s.trim().parse::<i64>().ok(),
            LevelVal::Other(v) => pyjson::value_as_i64(v),
        }
    }

    /// `str(level)` for display
    pub fn display(&self) -> String {
        match self {
            LevelVal::Int(i) => i.to_string(),
            LevelVal::Str(s) => s.clone(),
            LevelVal::Other(v) => pyjson::python_str_number(v),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LearnMoveEventDefinition {
    pub move_to_learn: Option<String>,
    pub destination: Option<i64>,
    pub source: String,
    pub level: LevelVal,
    pub mon: Option<String>,
    pub force_destination: bool,
    /// Recorder-only: the destination as a move *name* before the recorder
    /// resolves it to a slot (Python stores the name in `destination`).
    /// Never serialized.
    pub destination_name: Option<String>,
}

/// `(sanitized mon, level, sanitized move)`
pub type LevelUpKey = (String, i64, String);

impl LearnMoveEventDefinition {
    pub fn new(move_to_learn: Option<&str>, destination: Option<i64>, source: &str, level: LevelVal, mon: Option<&str>, force_destination: bool) -> Self {
        LearnMoveEventDefinition {
            move_to_learn: move_to_learn.map(|s| s.to_string()),
            destination,
            source: source.to_string(),
            level,
            mon: mon.map(|s| s.to_string()),
            force_destination,
            destination_name: None,
        }
    }

    pub fn serialize(&self) -> Value {
        pyjson::object(vec![
            (consts::LEARN_MOVE_KEY, opt_str_value(&self.move_to_learn)),
            (
                consts::MOVE_DEST_KEY,
                self.destination.map(Value::from).unwrap_or(Value::Null),
            ),
            (consts::MOVE_SOURCE_KEY, Value::String(self.source.clone())),
            (consts::MOVE_LEVEL_KEY, self.level.to_value()),
            (consts::MOVE_MON_KEY, opt_str_value(&self.mon)),
            (consts::MOVE_FORCE_DEST_KEY, Value::Bool(self.force_destination)),
        ])
    }

    /// `get_level_up_key`; `None` when the level is not an int (Python raises).
    pub fn get_level_up_key(&self) -> Option<LevelUpKey> {
        Some((
            sanitize_string(self.mon.as_deref().unwrap_or("")),
            self.level.as_int()?,
            sanitize_string(self.move_to_learn.as_deref().unwrap_or("")),
        ))
    }

    pub fn matches_level_up_move(&self, level: i64, mon: &str) -> bool {
        let level_matches = matches!(self.level, LevelVal::Int(l) if l == level);
        let mon_matches = match &self.mon {
            Some(m) => sanitize_string(m) == sanitize_string(mon),
            None => false,
        };
        level_matches && mon_matches
    }

    pub fn deserialize(raw: Option<&Value>, mon_default: Option<&str>) -> Option<Self> {
        let raw = raw?;
        if !truthy(raw) {
            return None;
        }
        match raw {
            Value::Array(items) => {
                if items.len() < 4 {
                    return None;
                }
                let level = LevelVal::from_value(&items[3]);
                let mut result = LearnMoveEventDefinition {
                    move_to_learn: s_or_none(&items[0]),
                    destination: match &items[1] {
                        Value::Null => None,
                        other => pyjson::value_as_i64(other),
                    },
                    source: str_of(&items[2]),
                    level,
                    mon: None,
                    force_destination: false,
                    destination_name: None,
                };
                if !result.level.is_any() {
                    result.mon = mon_default.map(|s| s.to_string());
                }
                Some(result)
            }
            Value::Object(_) => {
                let mon_val = match get(raw, consts::MOVE_MON_KEY) {
                    Some(Value::Null) | None => mon_default.map(|s| s.to_string()),
                    Some(other) => s_or_none(other),
                };
                Some(LearnMoveEventDefinition {
                    move_to_learn: get(raw, consts::LEARN_MOVE_KEY).and_then(s_or_none),
                    destination: match get(raw, consts::MOVE_DEST_KEY) {
                        Some(Value::Null) | None => None,
                        Some(other) => pyjson::value_as_i64(other),
                    },
                    source: get(raw, consts::MOVE_SOURCE_KEY).map(str_of).unwrap_or_default(),
                    level: get(raw, consts::MOVE_LEVEL_KEY).map(LevelVal::from_value).unwrap_or_else(LevelVal::any),
                    mon: mon_val,
                    force_destination: get(raw, consts::MOVE_FORCE_DEST_KEY).map(truthy).unwrap_or(false),
                    destination_name: None,
                })
            }
            _ => None,
        }
    }

    pub fn to_string(&self) -> String {
        let mon = self.mon.as_deref().unwrap_or("None");
        match (&self.destination, &self.move_to_learn) {
            // the recorder's unresolved destination is a move *name* in Python
            (None, Some(m)) if self.destination_name.is_some() => {
                format!("Learning {} over: {}, from {} (mon: {})", m, self.destination_name.as_deref().unwrap_or(""), self.source, mon)
            }
            (None, None) if self.destination_name.is_some() => {
                // `move_to_learn is None` with a str destination: Python's "Deleting" branch raises on `str + 1`
                let level = match &self.level {
                    LevelVal::Str(l) => format!("'{}'", l),
                    other => other.display(),
                };
                format!("LearnMove: (None, '{}', '{}', {})", self.destination_name.as_deref().unwrap_or(""), self.source, level)
            }
            (None, _) => format!(
                "Ignoring {}, from {} (mon: {})",
                self.move_to_learn.as_deref().unwrap_or("None"),
                self.source,
                mon
            ),
            (Some(d), None) => format!("Deleting move in slot #: {}", d + 1),
            (Some(d), Some(m)) => format!("Learning {} in slot #: {}, from {} (mon: {})", m, d + 1, self.source, mon),
        }
    }
}

/// Loose JSON scalar kept verbatim for round-tripping (`second_trainer_name`
/// can be a string, `null` or even an int in old routes).
#[derive(Clone, Debug, PartialEq)]
pub struct TrainerEventDefinition {
    pub trainer_name: String,
    pub second_trainer_name: Value,
    pub verbose_export: Value,
    pub setup_moves: Vec<String>,
    pub mimic_selection: String,
    pub custom_move_data: Vec<CustomMoveData>,
    pub enemy_setup_moves: Vec<String>,
    pub player_field_moves: Vec<String>,
    pub enemy_field_moves: Vec<String>,
    pub exp_split: Vec<i64>,
    pub weather: Option<String>,
    pub weather_source_mon_idx: Option<i64>,
    pub player_screens: IndexMap<String, i64>,
    pub enemy_screens: IndexMap<String, i64>,
    pub player_intimidate: Option<Vec<i64>>,
    pub enemy_intimidate: Option<Vec<i64>>,
    pub pay_day_amount: Option<i64>,
    pub mon_order: Vec<i64>,
    pub transformed: Value,
    pub stat_stage_setup: Vec<CustomMoveData>,
    pub collapsed_mons: Vec<i64>,
}

impl TrainerEventDefinition {
    pub fn new(trainer_name: &str) -> TrainerEventDefinition {
        TrainerEventDefinition {
            trainer_name: trainer_name.to_string(),
            second_trainer_name: Value::String(String::new()),
            verbose_export: Value::Bool(false),
            setup_moves: Vec::new(),
            mimic_selection: String::new(),
            custom_move_data: Vec::new(),
            enemy_setup_moves: Vec::new(),
            player_field_moves: Vec::new(),
            enemy_field_moves: Vec::new(),
            exp_split: Vec::new(),
            weather: Some(consts::WEATHER_NONE.to_string()),
            weather_source_mon_idx: None,
            player_screens: IndexMap::new(),
            enemy_screens: IndexMap::new(),
            player_intimidate: None,
            enemy_intimidate: None,
            pay_day_amount: Some(0),
            mon_order: Vec::new(),
            transformed: Value::Bool(false),
            stat_stage_setup: Vec::new(),
            collapsed_mons: Vec::new(),
        }
    }

    /// The second trainer name when it is a non-empty string (Python truthiness
    /// of the stored value, then used as a trainer-db key).
    pub fn second_trainer_name_str(&self) -> Option<&str> {
        match &self.second_trainer_name {
            Value::String(s) if !s.is_empty() => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn set_second_trainer_name(&mut self, name: &str) {
        self.second_trainer_name = Value::String(name.to_string());
    }

    pub fn is_verbose(&self) -> bool {
        truthy(&self.verbose_export)
    }

    pub fn set_verbose(&mut self, v: bool) {
        self.verbose_export = Value::Bool(v);
    }

    pub fn is_transformed(&self) -> bool {
        truthy(&self.transformed)
    }

    pub fn set_transformed(&mut self, v: bool) {
        self.transformed = Value::Bool(v);
    }

    /// `trainer_def.weather` as the damage code sees it (`None` -> falsy;
    /// the battle summary treats a missing weather as `WEATHER_NONE`).
    pub fn weather_str(&self) -> &str {
        self.weather.as_deref().unwrap_or(consts::WEATHER_NONE)
    }

    /// Python `isinstance(pay_day_amount, int)` (bools count as ints there,
    /// but a bool never reaches this field).
    pub fn pay_day_amount_int(&self) -> Option<i64> {
        self.pay_day_amount
    }

    pub fn serialize(&self) -> Value {
        let screens = |m: &IndexMap<String, i64>| -> Value {
            if m.is_empty() {
                Value::Null
            } else {
                let mut o = serde_json::Map::new();
                for (k, v) in m {
                    o.insert(k.clone(), Value::from(*v));
                }
                Value::Object(o)
            }
        };
        let intimidate = |v: &Option<Vec<i64>>| -> Value {
            match v {
                Some(list) => i64_list_value(list),
                None => Value::Null,
            }
        };
        pyjson::object(vec![
            (consts::TRAINER_NAME, Value::String(self.trainer_name.clone())),
            (consts::SECOND_TRAINER_NAME, self.second_trainer_name.clone()),
            (consts::VERBOSE_KEY, self.verbose_export.clone()),
            (consts::SETUP_MOVES_KEY, pyjson::str_array(&self.setup_moves)),
            (consts::ENEMY_SETUP_MOVES_KEY, pyjson::str_array(&self.enemy_setup_moves)),
            (consts::PLAYER_FIELD_MOVES_KEY, pyjson::str_array(&self.player_field_moves)),
            (consts::ENEMY_FIELD_MOVES_KEY, pyjson::str_array(&self.enemy_field_moves)),
            (consts::MIMIC_SELECTION, Value::String(self.mimic_selection.clone())),
            (
                consts::CUSTOM_MOVE_DATA,
                Value::Array(self.custom_move_data.iter().map(|c| c.to_json()).collect()),
            ),
            (consts::EXP_SPLIT, i64_list_value(&self.exp_split)),
            (consts::WEATHER, opt_str_value(&self.weather)),
            (
                consts::WEATHER_SOURCE_MON_IDX,
                self.weather_source_mon_idx.map(Value::from).unwrap_or(Value::Null),
            ),
            (consts::PLAYER_SCREENS_KEY, screens(&self.player_screens)),
            (consts::ENEMY_SCREENS_KEY, screens(&self.enemy_screens)),
            (consts::PLAYER_INTIMIDATE_KEY, intimidate(&self.player_intimidate)),
            (consts::ENEMY_INTIMIDATE_KEY, intimidate(&self.enemy_intimidate)),
            (
                consts::PAY_DAY_AMOUNT,
                self.pay_day_amount.map(Value::from).unwrap_or(Value::Null),
            ),
            (consts::MON_ORDER, i64_list_value(&self.mon_order)),
            (consts::TRANSFORMED, self.transformed.clone()),
            (
                consts::STAT_STAGE_SETUP_KEY,
                if self.stat_stage_setup.is_empty() {
                    Value::Null
                } else {
                    Value::Array(self.stat_stage_setup.iter().map(|c| c.to_json()).collect())
                },
            ),
            (
                consts::COLLAPSED_MONS_KEY,
                if self.collapsed_mons.is_empty() {
                    Value::Null
                } else {
                    i64_list_value(&self.collapsed_mons)
                },
            ),
        ])
    }

    pub fn deserialize(raw: Option<&Value>) -> Result<Option<Self>, String> {
        let Some(raw) = raw else { return Ok(None) };
        if !truthy(raw) {
            return Ok(None);
        }
        match raw {
            Value::String(name) => Ok(Some(TrainerEventDefinition::new(name))),
            Value::Array(items) => {
                let mut result = TrainerEventDefinition::new(&str_of(&items[0]));
                if let Some(v) = items.get(1) {
                    result.verbose_export = v.clone();
                }
                if let Some(v) = items.get(2) {
                    result.setup_moves = str_list(Some(v));
                }
                Ok(Some(result))
            }
            Value::Object(_) => {
                let req = |key: &str| -> Result<&Value, String> {
                    get(raw, key).ok_or_else(|| pyjson::python_repr_str(key))
                };
                let mut result = TrainerEventDefinition::new(&str_of(req(consts::TRAINER_NAME)?));
                result.verbose_export = req(consts::VERBOSE_KEY)?.clone();
                result.setup_moves = str_list(Some(req(consts::SETUP_MOVES_KEY)?));
                result.mimic_selection = str_of(req(consts::MIMIC_SELECTION)?);
                result.custom_move_data = match get(raw, consts::CUSTOM_MOVE_DATA) {
                    Some(Value::Array(items)) => items.iter().map(|x| CustomMoveData::from_json(Some(x))).collect(),
                    _ => Vec::new(),
                };
                result.enemy_setup_moves = str_list(get(raw, consts::ENEMY_SETUP_MOVES_KEY));
                result.player_field_moves = str_list(get(raw, consts::PLAYER_FIELD_MOVES_KEY));
                result.enemy_field_moves = str_list(get(raw, consts::ENEMY_FIELD_MOVES_KEY));
                result.exp_split = i64_list(get(raw, consts::EXP_SPLIT));
                result.weather = match get(raw, consts::WEATHER) {
                    None => Some(consts::WEATHER_NONE.to_string()),
                    Some(Value::Null) => None,
                    Some(other) => Some(str_of(other)),
                };
                result.weather_source_mon_idx = get(raw, consts::WEATHER_SOURCE_MON_IDX).and_then(pyjson::value_as_i64);
                let screens = |key: &str| -> IndexMap<String, i64> {
                    let mut m = IndexMap::new();
                    if let Some(Value::Object(o)) = get(raw, key) {
                        for (k, v) in o {
                            if let Some(i) = pyjson::value_as_i64(v) {
                                m.insert(k.clone(), i);
                            }
                        }
                    }
                    m
                };
                result.player_screens = screens(consts::PLAYER_SCREENS_KEY);
                result.enemy_screens = screens(consts::ENEMY_SCREENS_KEY);
                let intimidate = |key: &str| -> Option<Vec<i64>> {
                    match get(raw, key) {
                        Some(Value::Array(_)) => Some(i64_list(get(raw, key))),
                        _ => None,
                    }
                };
                result.player_intimidate = intimidate(consts::PLAYER_INTIMIDATE_KEY);
                result.enemy_intimidate = intimidate(consts::ENEMY_INTIMIDATE_KEY);
                result.pay_day_amount = match get(raw, consts::PAY_DAY_AMOUNT) {
                    None => Some(0),
                    Some(Value::Null) => None,
                    Some(other) => pyjson::value_as_i64(other),
                };
                result.mon_order = i64_list(get(raw, consts::MON_ORDER));
                result.second_trainer_name = get(raw, consts::SECOND_TRAINER_NAME)
                    .cloned()
                    .unwrap_or(Value::String(String::new()));
                result.transformed = get(raw, consts::TRANSFORMED).cloned().unwrap_or(Value::Bool(false));
                result.stat_stage_setup = match get(raw, consts::STAT_STAGE_SETUP_KEY) {
                    Some(Value::Array(items)) => items.iter().map(|x| CustomMoveData::from_json(Some(x))).collect(),
                    _ => Vec::new(),
                };
                result.collapsed_mons = i64_list(get(raw, consts::COLLAPSED_MONS_KEY));
                Ok(Some(result))
            }
            _ => Ok(None),
        }
    }

    pub fn to_string(&self) -> String {
        format!("Trainer {}", self.trainer_name)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LocationEventDefinition {
    pub location: Option<String>,
    /// The location exactly as loaded / recorded when it is not a string
    /// (the gen 2 recorder stores the map group *number*); Python keeps the
    /// raw value, so it is written back verbatim. `None` for string locations.
    pub raw: Option<Value>,
}

impl LocationEventDefinition {
    pub fn new(location: &str) -> Self {
        LocationEventDefinition {
            location: Some(location.to_string()),
            raw: None,
        }
    }

    /// `SaveEventDefinition(location=<any JSON value>)`
    pub fn from_value(location: &Value) -> Self {
        match location {
            Value::String(s) => LocationEventDefinition::new(s),
            Value::Null => LocationEventDefinition { location: None, raw: None },
            other => LocationEventDefinition {
                location: Some(pyjson::python_str_number(other)),
                raw: Some(other.clone()),
            },
        }
    }

    pub fn serialize(&self) -> Value {
        match &self.raw {
            Some(v) => Value::Array(vec![v.clone()]),
            None => Value::Array(vec![opt_str_value(&self.location)]),
        }
    }

    pub fn deserialize(raw: Option<&Value>) -> Option<Self> {
        let raw = raw?;
        if !truthy(raw) {
            return None;
        }
        let Value::Array(items) = raw else { return None };
        Some(match items.first() {
            Some(v) => LocationEventDefinition::from_value(v),
            None => LocationEventDefinition { location: None, raw: None },
        })
    }

    pub fn location_str(&self) -> &str {
        self.location.as_deref().unwrap_or("")
    }

    fn to_string_with(&self, with: &str, without: &str) -> String {
        match &self.location {
            Some(l) if !l.is_empty() => format!("{}{}", with, l),
            _ => without.to_string(),
        }
    }

    pub fn save_string(&self) -> String {
        self.to_string_with("Game Saved at: ", "Game Saved")
    }

    pub fn heal_string(&self) -> String {
        self.to_string_with("PkmnCenter Heal at: ", "PkmnCenter Heal")
    }

    pub fn blackout_string(&self) -> String {
        self.to_string_with("Black Out back to: ", "Black Out")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EvolutionEventDefinition {
    pub evolved_species: Option<String>,
    pub by_stone: Option<String>,
}

impl EvolutionEventDefinition {
    pub fn new(evolved_species: &str, by_stone: Option<&str>) -> Self {
        EvolutionEventDefinition {
            evolved_species: Some(evolved_species.to_string()),
            by_stone: by_stone.map(|s| s.to_string()),
        }
    }

    pub fn serialize(&self) -> Value {
        pyjson::object(vec![
            (consts::EVOLVED_SPECIES, opt_str_value(&self.evolved_species)),
            (consts::BY_STONE_KEY, opt_str_value(&self.by_stone)),
        ])
    }

    pub fn deserialize(raw: Option<&Value>) -> Result<Option<Self>, String> {
        let Some(raw) = raw else { return Ok(None) };
        if !truthy(raw) {
            return Ok(None);
        }
        let species = get(raw, consts::EVOLVED_SPECIES).ok_or_else(|| pyjson::python_repr_str(consts::EVOLVED_SPECIES))?;
        let stone = get(raw, consts::BY_STONE_KEY).ok_or_else(|| pyjson::python_repr_str(consts::BY_STONE_KEY))?;
        Ok(Some(EvolutionEventDefinition {
            evolved_species: s_or_none(species),
            by_stone: s_or_none(stone),
        }))
    }

    pub fn to_string(&self) -> String {
        format!("Evolve into: {}", self.evolved_species.as_deref().unwrap_or("None"))
    }
}

// ---------------------------------------------------------------------------
// Bag reordering (gen 1)
// ---------------------------------------------------------------------------

/// One in-game SELECT swap: the two items exchanged and the (1-based) slots
/// they occupied when the swap was recorded. Slots are exact at recording
/// time; when the event is applied the items are matched by name and the
/// slots only decide whether the application warns (see
/// [`crate::Inventory::swap_items`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BagSwap {
    pub item_a: String,
    pub slot_a: usize,
    pub item_b: String,
    pub slot_b: usize,
}

impl BagSwap {
    pub fn new(item_a: &str, slot_a: usize, item_b: &str, slot_b: usize) -> BagSwap {
        BagSwap {
            item_a: item_a.to_string(),
            slot_a,
            item_b: item_b.to_string(),
            slot_b,
        }
    }

    /// `Potion (3) <-> Master Ball (7)`
    pub fn to_string(&self) -> String {
        format!("{} ({}) <-> {} ({})", self.item_a, self.slot_a, self.item_b, self.slot_b)
    }

    fn serialize(&self) -> Value {
        Value::Array(vec![
            Value::String(self.item_a.clone()),
            Value::from(self.slot_a as i64),
            Value::String(self.item_b.clone()),
            Value::from(self.slot_b as i64),
        ])
    }

    fn deserialize(raw: &Value) -> Option<BagSwap> {
        let Value::Array(parts) = raw else { return None };
        if parts.len() != 4 {
            return None;
        }
        let slot = |v: &Value| -> Option<usize> {
            let n = pyjson::value_as_i64(v)?;
            if n < 1 {
                None
            } else {
                Some(n as usize)
            }
        };
        Some(BagSwap {
            item_a: str_of(&parts[0]),
            slot_a: slot(&parts[1])?,
            item_b: str_of(&parts[2]),
            slot_b: slot(&parts[3])?,
        })
    }
}

/// The swaps of one bag-reorder event, applied in order; the slots of a later
/// swap refer to the bag after the earlier swaps of the same event.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BagReorderEventDefinition {
    pub swaps: Vec<BagSwap>,
}

impl BagReorderEventDefinition {
    pub fn new(swaps: Vec<BagSwap>) -> Self {
        BagReorderEventDefinition { swaps }
    }

    pub fn serialize(&self) -> Value {
        Value::Array(self.swaps.iter().map(|s| s.serialize()).collect())
    }

    /// Any array of `[name, slot, name, slot]` entries (an empty array is a
    /// valid, empty reorder). A malformed entry fails the load, like a
    /// malformed trainer or evolution does: degrading to notes would drop the
    /// swaps for good on the next save.
    pub fn deserialize(raw: Option<&Value>) -> Result<Option<Self>, String> {
        let raw = match raw {
            None | Some(Value::Null) => return Ok(None),
            Some(v) => v,
        };
        let Value::Array(items) = raw else {
            return Err(format!("Invalid {} entry: {}", consts::TASK_REORDER_BAG, raw));
        };
        let mut swaps = Vec::with_capacity(items.len());
        for item in items {
            swaps.push(BagSwap::deserialize(item).ok_or_else(|| format!("Invalid {} swap (expected [item, slot, item, slot] with slots from 1): {}", consts::TASK_REORDER_BAG, item))?);
        }
        Ok(Some(BagReorderEventDefinition { swaps }))
    }

    pub fn to_string(&self) -> String {
        if self.swaps.is_empty() {
            return "Reorder Bag: (no swaps)".to_string();
        }
        let parts: Vec<String> = self.swaps.iter().map(|s| s.to_string()).collect();
        format!("Reorder Bag: {}", parts.join(", "))
    }
}

/// The swaps that turn the bag order `old` into `new` (a selection-style
/// walk: one swap per misplaced slot, so a single transposition is exactly
/// one swap and a k-cycle is k-1). Both lists must hold the same names,
/// each once; otherwise `Err` names the first item that does not line up.
pub fn swaps_between(old: &[String], new: &[String]) -> Result<Vec<BagSwap>, String> {
    if old.len() != new.len() {
        return Err(format!("Bag has {} items before and {} after; a reorder cannot change the item count", old.len(), new.len()));
    }
    let mut cur: Vec<String> = old.to_vec();
    let mut swaps = Vec::new();
    for i in 0..new.len() {
        if cur[i] == new[i] {
            continue;
        }
        let Some(j) = (i + 1..cur.len()).find(|j| cur[*j] == new[i]) else {
            return Err(format!("Cannot reorder bag: {} is not in the bag", new[i]));
        };
        swaps.push(BagSwap::new(&cur[i], i + 1, &cur[j], j + 1));
        cur.swap(i, j);
    }
    Ok(swaps)
}

// ---------------------------------------------------------------------------
// EventDefinition
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct EventDefinition {
    /// `None` reproduces a stored `Enabled: null` (falsy, written back as null).
    pub enabled: Option<bool>,
    pub recorded_time: Value,
    pub split_time: Value,
    pub rare_candy: Option<RareCandyEventDefinition>,
    pub vitamin: Option<VitaminEventDefinition>,
    pub trainer_def: Option<TrainerEventDefinition>,
    pub wild_pkmn_info: Option<WildPkmnEventDefinition>,
    pub item_event_def: Option<InventoryEventDefinition>,
    pub learn_move: Option<LearnMoveEventDefinition>,
    pub hold_item: Option<HoldItemEventDefinition>,
    pub save: Option<LocationEventDefinition>,
    pub heal: Option<LocationEventDefinition>,
    pub blackout: Option<LocationEventDefinition>,
    pub evolution: Option<EvolutionEventDefinition>,
    pub bag_reorder: Option<BagReorderEventDefinition>,
    pub tags: Vec<String>,
    pub notes: String,
}

impl Default for EventDefinition {
    fn default() -> Self {
        EventDefinition {
            enabled: Some(true),
            recorded_time: Value::Null,
            split_time: Value::Null,
            rare_candy: None,
            vitamin: None,
            trainer_def: None,
            wild_pkmn_info: None,
            item_event_def: None,
            learn_move: None,
            hold_item: None,
            save: None,
            heal: None,
            blackout: None,
            evolution: None,
            bag_reorder: None,
            tags: Vec::new(),
            notes: String::new(),
        }
    }
}

impl EventDefinition {
    pub fn notes_only(notes: &str) -> EventDefinition {
        EventDefinition {
            notes: notes.to_string(),
            ..Default::default()
        }
    }

    pub fn with_trainer(t: TrainerEventDefinition) -> EventDefinition {
        EventDefinition {
            trainer_def: Some(t),
            ..Default::default()
        }
    }

    pub fn with_learn_move(l: LearnMoveEventDefinition) -> EventDefinition {
        EventDefinition {
            learn_move: Some(l),
            ..Default::default()
        }
    }

    pub fn with_rare_candy(amount: i64) -> EventDefinition {
        EventDefinition {
            rare_candy: Some(RareCandyEventDefinition::new(amount)),
            ..Default::default()
        }
    }

    pub fn with_vitamin(v: VitaminEventDefinition) -> EventDefinition {
        EventDefinition {
            vitamin: Some(v),
            ..Default::default()
        }
    }

    pub fn with_item(i: InventoryEventDefinition) -> EventDefinition {
        EventDefinition {
            item_event_def: Some(i),
            ..Default::default()
        }
    }

    pub fn with_hold_item(h: HoldItemEventDefinition) -> EventDefinition {
        EventDefinition {
            hold_item: Some(h),
            ..Default::default()
        }
    }

    pub fn with_wild(w: WildPkmnEventDefinition) -> EventDefinition {
        EventDefinition {
            wild_pkmn_info: Some(w),
            ..Default::default()
        }
    }

    pub fn with_save(location: &str) -> EventDefinition {
        EventDefinition {
            save: Some(LocationEventDefinition::new(location)),
            ..Default::default()
        }
    }

    pub fn with_heal(location: &str) -> EventDefinition {
        EventDefinition {
            heal: Some(LocationEventDefinition::new(location)),
            ..Default::default()
        }
    }

    /// `EventDefinition(save=SaveEventDefinition(location=value))` with the raw mapper value.
    pub fn with_save_value(location: &Value) -> EventDefinition {
        EventDefinition {
            save: Some(LocationEventDefinition::from_value(location)),
            ..Default::default()
        }
    }

    /// `EventDefinition(heal=HealEventDefinition(location=value))` with the raw mapper value.
    pub fn with_heal_value(location: &Value) -> EventDefinition {
        EventDefinition {
            heal: Some(LocationEventDefinition::from_value(location)),
            ..Default::default()
        }
    }

    /// `EventDefinition(blackout=BlackoutEventDefinition())`: the location
    /// defaults to `""` (saved as `[""]`), not `None`.
    pub fn with_blackout() -> EventDefinition {
        EventDefinition {
            blackout: Some(LocationEventDefinition::new("")),
            ..Default::default()
        }
    }

    pub fn with_evolution(species: &str) -> EventDefinition {
        EventDefinition {
            evolution: Some(EvolutionEventDefinition::new(species, None)),
            ..Default::default()
        }
    }

    pub fn with_bag_reorder(swaps: Vec<BagSwap>) -> EventDefinition {
        EventDefinition {
            bag_reorder: Some(BagReorderEventDefinition::new(swaps)),
            ..Default::default()
        }
    }

    pub fn is_enabled_flag(&self) -> bool {
        self.enabled.unwrap_or(false)
    }

    pub fn recorded_time_str(&self) -> Option<String> {
        match &self.recorded_time {
            Value::Null => None,
            Value::String(s) => Some(s.clone()),
            other => Some(pyjson::python_str_number(other)),
        }
    }

    pub fn split_time_str(&self) -> Option<String> {
        match &self.split_time {
            Value::Null => None,
            Value::String(s) => Some(s.clone()),
            other => Some(pyjson::python_str_number(other)),
        }
    }

    pub fn get_first_trainer_obj<'a>(&self, gen: &'a GenData) -> Result<Option<&'a std::sync::Arc<Trainer>>, String> {
        let Some(td) = &self.trainer_def else { return Ok(None) };
        match gen.trainer_db().get_trainer(&td.trainer_name) {
            Some(t) => Ok(Some(t)),
            None => Err(format!(
                "Could not find trainer object for trainer named: '{}', from trainer_db for version: {}",
                td.trainer_name,
                gen.version_name()
            )),
        }
    }

    pub fn get_second_trainer_obj<'a>(&self, gen: &'a GenData) -> Result<Option<&'a std::sync::Arc<Trainer>>, String> {
        let Some(td) = &self.trainer_def else { return Ok(None) };
        let Some(second) = td.second_trainer_name_str() else { return Ok(None) };
        match gen.trainer_db().get_trainer(second) {
            Some(t) => Ok(Some(t)),
            None => Err(format!(
                "Could not find trainer object for trainer named: '{}', from trainer_db for version: {}",
                second,
                gen.version_name()
            )),
        }
    }

    pub fn get_wild_pkmn(&self, gen: &GenData) -> Result<Option<EnemyPkmn>, String> {
        let Some(w) = &self.wild_pkmn_info else { return Ok(None) };
        let mon = if w.trainer_pkmn {
            gen.create_trainer_pkmn(&w.name, w.level)
        } else {
            gen.create_wild_pkmn(&w.name, w.level, 15)
        };
        mon.map(Some)
            .ok_or_else(|| "'NoneType' object has no attribute 'name'".to_string())
    }

    /// `get_pokemon_list(definition_order, include_definition_idx=True)`:
    /// returns `(definition_idx, mon)` pairs exactly as the Python code
    /// computes them (note the `mon_order[x] - 1` pairing quirk).
    pub fn get_pokemon_list(&self, gen: &GenData, definition_order: bool) -> Result<Vec<(i64, EnemyPkmn)>, String> {
        if let Some(wild) = self.get_wild_pkmn(gen)? {
            let q = self.wild_pkmn_info.as_ref().map(|w| w.quantity).unwrap_or(1);
            return Ok((0..q.max(0)).map(|x| (x, wild.clone())).collect());
        }
        let Some(trainer) = self.get_first_trainer_obj(gen)? else {
            return Ok(Vec::new());
        };
        let td = self.trainer_def.as_ref().unwrap();
        let pkmn_list: Vec<EnemyPkmn> = match self.get_second_trainer_obj(gen)? {
            None => trainer.pkmn.clone(),
            Some(second) => {
                let mut list = Vec::new();
                for idx in 0..3 {
                    if idx < trainer.pkmn.len() {
                        list.push(trainer.pkmn[idx].clone());
                    }
                    if idx < second.pkmn.len() {
                        list.push(second.pkmn[idx].clone());
                    }
                }
                list
            }
        };
        let mon_order: Vec<i64> = if td.mon_order.is_empty() || definition_order {
            (1..=(pkmn_list.len() as i64)).collect()
        } else {
            td.mon_order.clone()
        };
        let mut result: Vec<EnemyPkmn> = Vec::new();
        let mut order_idx: i64 = 1;
        while order_idx <= mon_order.len() as i64 {
            for (lookup_idx, test_idx) in mon_order.iter().enumerate() {
                if order_idx == *test_idx {
                    let Some(base) = pkmn_list.get(lookup_idx) else {
                        return Err("list index out of range".to_string());
                    };
                    let mut cur = base.clone();
                    if td.custom_move_data.len() > lookup_idx {
                        cur.custom_move_data = Some(td.custom_move_data[lookup_idx].clone());
                    }
                    if td.exp_split.len() > lookup_idx {
                        cur.exp_split = td.exp_split[lookup_idx];
                    }
                    if td.mon_order.len() > lookup_idx {
                        cur.mon_order = td.mon_order[lookup_idx];
                    } else {
                        cur.mon_order = order_idx;
                    }
                    cur.definition_order = lookup_idx as i64;
                    result.push(cur);
                    break;
                }
            }
            order_idx += 1;
        }
        Ok(result
            .into_iter()
            .enumerate()
            .map(|(x, mon)| (mon_order.get(x).copied().unwrap_or(1) - 1, mon))
            .collect())
    }

    /// `get_pokemon_list()` without definition indices.
    pub fn pokemon_list(&self, gen: &GenData) -> Result<Vec<EnemyPkmn>, String> {
        Ok(self.get_pokemon_list(gen, false)?.into_iter().map(|(_, m)| m).collect())
    }

    pub fn get_event_type(&self) -> &'static str {
        if self.rare_candy.is_some() {
            consts::TASK_RARE_CANDY
        } else if self.vitamin.is_some() {
            consts::TASK_VITAMIN
        } else if self.wild_pkmn_info.is_some() {
            consts::TASK_FIGHT_WILD_PKMN
        } else if self.trainer_def.is_some() {
            consts::TASK_TRAINER_BATTLE
        } else if let Some(i) = &self.item_event_def {
            if i.is_acquire && i.with_money {
                consts::TASK_PURCHASE_ITEM
            } else if i.is_acquire {
                consts::TASK_GET_FREE_ITEM
            } else if i.with_money {
                consts::TASK_SELL_ITEM
            } else {
                consts::TASK_USE_ITEM
            }
        } else if let Some(l) = &self.learn_move {
            if l.source == consts::MOVE_SOURCE_LEVELUP {
                consts::TASK_LEARN_MOVE_LEVELUP
            } else {
                consts::TASK_LEARN_MOVE_TM
            }
        } else if self.hold_item.is_some() {
            consts::TASK_HOLD_ITEM
        } else if self.save.is_some() {
            consts::TASK_SAVE
        } else if self.heal.is_some() {
            consts::TASK_HEAL
        } else if self.blackout.is_some() {
            consts::TASK_BLACKOUT
        } else if self.evolution.is_some() {
            consts::TASK_EVOLUTION
        } else if self.bag_reorder.is_some() {
            consts::TASK_REORDER_BAG
        } else {
            consts::TASK_NOTES_ONLY
        }
    }

    fn trainer_label(&self, gen: &GenData) -> Result<String, String> {
        let trainer = self.get_first_trainer_obj(gen)?.ok_or_else(|| "no trainer".to_string())?;
        let second = self.get_second_trainer_obj(gen)?;
        let loc = if trainer.location.is_empty() {
            String::new()
        } else {
            format!(" ({})", trainer.location)
        };
        Ok(match second {
            None => format!("Trainer: {}{}", trainer.name, loc),
            Some(s) => format!("Multi: {}, {}{}", trainer.name, s.name, loc),
        })
    }

    /// `get_item_label`
    pub fn get_item_label(&self, gen: &GenData) -> Result<String, String> {
        Ok(if self.rare_candy.is_some() {
            "Rare Candy x1".to_string()
        } else if let Some(v) = &self.vitamin {
            if v.is_ev_berry(gen) {
                format!("Berry: {} x1", v.vitamin)
            } else {
                format!("Vitamin: {} x1", v.vitamin)
            }
        } else if let Some(w) = &self.wild_pkmn_info {
            w.to_string()
        } else if self.trainer_def.is_some() {
            self.trainer_label(gen)?
        } else {
            self.non_battle_label(gen)
        })
    }

    fn non_battle_label(&self, gen: &GenData) -> String {
        if let Some(i) = &self.item_event_def {
            i.to_string()
        } else if let Some(l) = &self.learn_move {
            l.to_string()
        } else if let Some(h) = &self.hold_item {
            h.to_string()
        } else if let Some(s) = &self.save {
            s.save_string()
        } else if let Some(h) = &self.heal {
            h.heal_string()
        } else if let Some(b) = &self.blackout {
            b.blackout_string()
        } else if let Some(e) = &self.evolution {
            e.to_string()
        } else if let Some(r) = &self.bag_reorder {
            r.to_string()
        } else {
            let _ = gen;
            format!("Notes: {}", self.notes)
        }
    }

    /// `get_label` / `__str__`
    pub fn get_label(&self, gen: &GenData) -> Result<String, String> {
        Ok(if let Some(r) = &self.rare_candy {
            r.to_string()
        } else if let Some(v) = &self.vitamin {
            v.to_string(gen)
        } else if let Some(w) = &self.wild_pkmn_info {
            w.to_string()
        } else if self.trainer_def.is_some() {
            self.trainer_label(gen)?
        } else {
            self.non_battle_label(gen)
        })
    }

    pub fn is_highlighted(&self) -> bool {
        self.tags.iter().any(|t| t == consts::HIGHLIGHT_LABEL)
    }

    pub fn experience_per_second(&self, gen: &GenData) -> Result<String, String> {
        if self.trainer_def.is_none() {
            return Ok(String::new());
        }
        let mons = self.pokemon_list(gen)?;
        Ok(exp::experience_per_second(gen.get_trainer_timing_info(), &mons))
    }

    pub fn do_render(&self, gen: &GenData, search: Option<&str>, filter_types: Option<&[String]>) -> Result<bool, String> {
        if let Some(types) = filter_types {
            if !types.iter().any(|t| t == self.get_event_type()) {
                return Ok(false);
            }
        }
        if let Some(search) = search {
            let s = search.to_lowercase();
            if !self.get_item_label(gen)?.to_lowercase().contains(&s) && !self.notes.to_lowercase().contains(&s) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn toggle_highlight(&mut self) {
        if let Some(pos) = self.tags.iter().position(|t| t == consts::HIGHLIGHT_LABEL) {
            self.tags.remove(pos);
        } else {
            self.tags.push(consts::HIGHLIGHT_LABEL.to_string());
        }
    }

    pub fn get_highlight_type(&self) -> Option<i64> {
        for (idx, label) in consts::ALL_HIGHLIGHT_LABELS.iter().enumerate() {
            if self.tags.iter().any(|t| t == label) {
                return Some(idx as i64 + 1);
            }
        }
        if self.is_highlighted() {
            return Some(1);
        }
        None
    }

    pub fn set_highlight(&mut self, highlight_num: Option<i64>) {
        if let Some(pos) = self.tags.iter().position(|t| t == consts::HIGHLIGHT_LABEL) {
            self.tags.remove(pos);
        }
        for label in consts::ALL_HIGHLIGHT_LABELS {
            if let Some(pos) = self.tags.iter().position(|t| t == label) {
                self.tags.remove(pos);
            }
        }
        if let Some(n) = highlight_num {
            if (1..=9).contains(&n) {
                self.tags.push(consts::ALL_HIGHLIGHT_LABELS[(n - 1) as usize].to_string());
            }
        }
    }

    pub fn serialize(&self) -> Value {
        let mut pairs: Vec<(&str, Value)> = vec![
            (
                consts::ENABLED_KEY,
                match self.enabled {
                    Some(b) => Value::Bool(b),
                    None => Value::Null,
                },
            ),
            (consts::RECORDED_TIME_KEY, self.recorded_time.clone()),
            (consts::SPLIT_TIME_KEY, self.split_time.clone()),
            (consts::TAGS_KEY, pyjson::str_array(&self.tags)),
        ];
        if !self.notes.is_empty() {
            pairs.push((consts::TASK_NOTES_ONLY, Value::String(self.notes.clone())));
        }
        if let Some(r) = &self.rare_candy {
            pairs.push((consts::TASK_RARE_CANDY, r.serialize()));
        } else if let Some(v) = &self.vitamin {
            pairs.push((consts::TASK_VITAMIN, v.serialize()));
        } else if let Some(t) = &self.trainer_def {
            pairs.push((consts::TASK_TRAINER_BATTLE, t.serialize()));
        } else if let Some(w) = &self.wild_pkmn_info {
            pairs.push((consts::TASK_FIGHT_WILD_PKMN, w.serialize()));
        } else if let Some(i) = &self.item_event_def {
            pairs.push((consts::INVENTORY_EVENT_DEFINITON, i.serialize()));
        } else if let Some(l) = &self.learn_move {
            pairs.push((consts::LEARN_MOVE_KEY, l.serialize()));
        } else if let Some(h) = &self.hold_item {
            pairs.push((consts::TASK_HOLD_ITEM, h.serialize()));
        } else if let Some(s) = &self.save {
            pairs.push((consts::TASK_SAVE, s.serialize()));
        } else if let Some(h) = &self.heal {
            pairs.push((consts::TASK_HEAL, h.serialize()));
        } else if let Some(b) = &self.blackout {
            pairs.push((consts::TASK_BLACKOUT, b.serialize()));
        } else if let Some(e) = &self.evolution {
            pairs.push((consts::TASK_EVOLUTION, e.serialize()));
        } else if let Some(r) = &self.bag_reorder {
            pairs.push((consts::TASK_REORDER_BAG, r.serialize()));
        }
        pyjson::object(pairs)
    }

    pub fn deserialize(raw: &Value) -> Result<EventDefinition, String> {
        let enabled = match get(raw, consts::ENABLED_KEY) {
            None => Some(true),
            Some(Value::Null) => None,
            Some(other) => Some(truthy(other)),
        };
        let mut result = EventDefinition {
            enabled,
            notes: get(raw, consts::TASK_NOTES_ONLY).map(str_of).unwrap_or_default(),
            tags: str_list(get(raw, consts::TAGS_KEY)),
            recorded_time: get(raw, consts::RECORDED_TIME_KEY).cloned().unwrap_or(Value::Null),
            split_time: get(raw, consts::SPLIT_TIME_KEY).cloned().unwrap_or(Value::Null),
            rare_candy: RareCandyEventDefinition::deserialize(get(raw, consts::TASK_RARE_CANDY)),
            vitamin: VitaminEventDefinition::deserialize(get(raw, consts::TASK_VITAMIN)),
            trainer_def: TrainerEventDefinition::deserialize(get(raw, consts::TASK_TRAINER_BATTLE))?,
            wild_pkmn_info: WildPkmnEventDefinition::deserialize(get(raw, consts::TASK_FIGHT_WILD_PKMN)),
            item_event_def: InventoryEventDefinition::deserialize(get(raw, consts::INVENTORY_EVENT_DEFINITON)),
            learn_move: LearnMoveEventDefinition::deserialize(get(raw, consts::LEARN_MOVE_KEY), None),
            hold_item: HoldItemEventDefinition::deserialize(get(raw, consts::TASK_HOLD_ITEM)),
            save: LocationEventDefinition::deserialize(get(raw, consts::TASK_SAVE)),
            heal: LocationEventDefinition::deserialize(get(raw, consts::TASK_HEAL)),
            blackout: LocationEventDefinition::deserialize(get(raw, consts::TASK_BLACKOUT)),
            evolution: EvolutionEventDefinition::deserialize(get(raw, consts::TASK_EVOLUTION))?,
            bag_reorder: BagReorderEventDefinition::deserialize(get(raw, consts::TASK_REORDER_BAG))?,
        };
        if result.wild_pkmn_info.is_some() {
            result.trainer_def = None;
        }
        Ok(result)
    }
}
