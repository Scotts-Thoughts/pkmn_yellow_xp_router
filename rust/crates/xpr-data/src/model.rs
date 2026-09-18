//! Port of `pkmn/universal_data_objects.py`: the shared value types.

use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::Value;

use xpr_core::consts;
use xpr_core::io_utils::sanitize_string;

use crate::badges::BadgeList;
use crate::stats;

/// Which game generation's arithmetic a stat block follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Gen {
    One,
    Two,
    Three,
    Four,
    Five,
}

impl Gen {
    pub fn number(self) -> u8 {
        match self {
            Gen::One => 1,
            Gen::Two => 2,
            Gen::Three => 3,
            Gen::Four => 4,
            Gen::Five => 5,
        }
    }

    pub fn from_number(n: u8) -> Option<Gen> {
        match n {
            1 => Some(Gen::One),
            2 => Some(Gen::Two),
            3 => Some(Gen::Three),
            4 => Some(Gen::Four),
            5 => Some(Gen::Five),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Nature
// ---------------------------------------------------------------------------

pub const NATURE_NAMES: [&str; 25] = [
    "HARDY", "LONELY", "BRAVE", "ADAMANT", "NAUGHTY", "BOLD", "DOCILE", "RELAXED", "IMPISH", "LAX",
    "TIMID", "HASTY", "SERIOUS", "JOLLY", "NAIVE", "MODEST", "MILD", "QUIET", "BASHFUL", "RASH",
    "CALM", "GENTLE", "SASSY", "CAREFUL", "QUIRKY",
];

const NEUTRAL_NATURES: [i64; 5] = [0, 6, 12, 18, 24];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Nature(pub i64);

impl Default for Nature {
    fn default() -> Self {
        Nature::HARDY
    }
}

impl Nature {
    pub const HARDY: Nature = Nature(0);

    /// `Nature(value)`; out-of-range values are an error in Python (ValueError).
    pub fn from_index(idx: i64) -> Option<Nature> {
        if (0..25).contains(&idx) {
            Some(Nature(idx))
        } else {
            None
        }
    }

    pub fn from_name(name: &str) -> Option<Nature> {
        let upper = name.to_uppercase();
        NATURE_NAMES.iter().position(|n| *n == upper).map(|i| Nature(i as i64))
    }

    pub fn value(self) -> i64 {
        self.0
    }

    /// `Nature.name` (upper case enum member name).
    pub fn enum_name(self) -> &'static str {
        NATURE_NAMES[self.0.clamp(0, 24) as usize]
    }

    /// `str(nature)` -> `name.capitalize()`.
    pub fn display_name(self) -> String {
        let n = self.enum_name();
        let mut c = n.chars();
        match c.next() {
            Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
            None => String::new(),
        }
    }

    pub fn is_stat_raised(self, stat_name: &str) -> bool {
        let v = self.0;
        if NEUTRAL_NATURES.contains(&v) {
            false
        } else if v <= 4 {
            stat_name == consts::ATTACK
        } else if v <= 9 {
            stat_name == consts::DEFENSE
        } else if v <= 14 {
            stat_name == consts::SPEED
        } else if v <= 19 {
            stat_name == consts::SPECIAL_ATTACK
        } else if v <= 23 {
            stat_name == consts::SPECIAL_DEFENSE
        } else {
            false
        }
    }

    pub fn is_stat_lowered(self, stat_name: &str) -> bool {
        let v = self.0;
        if NEUTRAL_NATURES.contains(&v) {
            false
        } else if v == 1 || v == 11 || v == 16 || v == 21 {
            stat_name == consts::DEFENSE
        } else if v == 2 || v == 7 || v == 17 || v == 22 {
            stat_name == consts::SPEED
        } else if v == 3 || v == 8 || v == 13 || v == 23 {
            stat_name == consts::SPECIAL_ATTACK
        } else if v == 4 || v == 9 || v == 14 || v == 19 {
            stat_name == consts::SPECIAL_DEFENSE
        } else if v == 5 || v == 10 || v == 15 || v == 20 {
            stat_name == consts::ATTACK
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// StageModifiers
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StageModifiers {
    pub attack_stage: i64,
    pub defense_stage: i64,
    pub speed_stage: i64,
    pub special_attack_stage: i64,
    pub special_defense_stage: i64,
    pub accuracy_stage: i64,
    pub evasion_stage: i64,
    // "theoretical" gen-1 badge boost counters, ignored by other gens
    pub attack_badge_boosts: i64,
    pub defense_badge_boosts: i64,
    pub speed_badge_boosts: i64,
    pub special_badge_boosts: i64,
}

fn clamp_stage(v: i64) -> i64 {
    v.clamp(-6, 6)
}

impl StageModifiers {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        attack: i64,
        defense: i64,
        speed: i64,
        special_attack: i64,
        special_defense: i64,
        accuracy: i64,
        evasion: i64,
    ) -> StageModifiers {
        StageModifiers {
            attack_stage: clamp_stage(attack),
            defense_stage: clamp_stage(defense),
            speed_stage: clamp_stage(speed),
            special_attack_stage: clamp_stage(special_attack),
            special_defense_stage: clamp_stage(special_defense),
            accuracy_stage: clamp_stage(accuracy),
            evasion_stage: clamp_stage(evasion),
            ..Default::default()
        }
    }

    /// `StageModifiers(accuracy=x, evasion=y)`
    pub fn only_accuracy_evasion(accuracy: i64, evasion: i64) -> StageModifiers {
        StageModifiers::new(0, 0, 0, 0, 0, accuracy, evasion)
    }

    pub fn set_attack_stage(&self, value: i64) -> StageModifiers {
        let mut result = *self;
        result.attack_stage = clamp_stage(value);
        result.attack_badge_boosts = 0;
        result
    }

    pub fn clear_badge_boosts(&self) -> StageModifiers {
        let mut result = *self;
        result.attack_badge_boosts = 0;
        result.defense_badge_boosts = 0;
        result.speed_badge_boosts = 0;
        result.special_badge_boosts = 0;
        result
    }

    /// Faithful port of `apply_stat_mod`, including the Python quirk where a
    /// no-op boost mid-list re-aliases `result` to `self` (after which the
    /// remaining boosts still change the stage but never reset a badge boost
    /// counter, because the "did it change" comparison is then self-vs-self).
    pub fn apply_stat_mod(&self, all_stat_mods: &[(String, i64)]) -> StageModifiers {
        if all_stat_mods.is_empty() {
            return *self;
        }
        let mut result = *self;
        result.attack_badge_boosts += 1;
        result.defense_badge_boosts += 1;
        result.speed_badge_boosts += 1;
        result.special_badge_boosts += 1;
        let mut aliased = false;

        macro_rules! apply_stage {
            ($stage:ident, $bb:ident, $delta:expr) => {{
                let base = if aliased { result.$stage } else { self.$stage };
                let new_val = clamp_stage(base + $delta);
                result.$stage = new_val;
                let unchanged = if aliased { true } else { new_val == self.$stage };
                if unchanged {
                    if !aliased {
                        result = *self;
                        aliased = true;
                    }
                    continue;
                }
                result.$bb = 0;
            }};
            ($stage:ident, $delta:expr) => {{
                let base = if aliased { result.$stage } else { self.$stage };
                let new_val = clamp_stage(base + $delta);
                result.$stage = new_val;
                let unchanged = if aliased { true } else { new_val == self.$stage };
                if unchanged {
                    if !aliased {
                        result = *self;
                        aliased = true;
                    }
                    continue;
                }
            }};
        }

        for (stat, delta) in all_stat_mods {
            match stat.as_str() {
                consts::ATK => apply_stage!(attack_stage, attack_badge_boosts, *delta),
                consts::DEF => apply_stage!(defense_stage, defense_badge_boosts, *delta),
                consts::SPE => apply_stage!(speed_stage, speed_badge_boosts, *delta),
                consts::SPA => apply_stage!(special_attack_stage, special_badge_boosts, *delta),
                consts::SPD => apply_stage!(special_defense_stage, special_badge_boosts, *delta),
                consts::ACC => apply_stage!(accuracy_stage, *delta),
                consts::EV => apply_stage!(evasion_stage, *delta),
                _ => {}
            }
        }
        result
    }

    /// Python `__eq__` (ignores evasion/accuracy badge boosts, which do not exist).
    pub fn py_eq(&self, other: &StageModifiers) -> bool {
        self == other
    }
}

// ---------------------------------------------------------------------------
// StatBlock
// ---------------------------------------------------------------------------

/// A block of six stats. `gen` selects the caps and formulas the Python
/// `GenXStatBlock` subclasses implement; `is_stat_xp` selects the stat-XP /
/// EV caps. Equality compares the six values only, like Python.
#[derive(Clone, Copy, Debug)]
pub struct StatBlock {
    pub gen: Gen,
    pub is_stat_xp: bool,
    pub hp: i64,
    pub attack: i64,
    pub defense: i64,
    pub special_attack: i64,
    pub special_defense: i64,
    pub speed: i64,
}

impl PartialEq for StatBlock {
    fn eq(&self, other: &Self) -> bool {
        self.hp == other.hp
            && self.attack == other.attack
            && self.defense == other.defense
            && self.speed == other.speed
            && self.special_attack == other.special_attack
            && self.special_defense == other.special_defense
    }
}
impl Eq for StatBlock {}

pub const STAT_XP_CAP_GEN12: i64 = 65535;
pub const SINGLE_STAT_EV_CAP: i64 = 255;
pub const TOTAL_EV_CAP: i64 = 510;

impl StatBlock {
    /// Constructor argument order matches Python: hp, attack, defense,
    /// special_attack, special_defense, speed.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        gen: Gen,
        hp: i64,
        attack: i64,
        defense: i64,
        special_attack: i64,
        special_defense: i64,
        speed: i64,
        is_stat_xp: bool,
    ) -> StatBlock {
        let mut sb = StatBlock {
            gen,
            is_stat_xp,
            hp,
            attack,
            defense,
            special_attack,
            special_defense,
            speed,
        };
        if is_stat_xp {
            match gen {
                Gen::One | Gen::Two => {
                    sb.hp = hp.min(STAT_XP_CAP_GEN12);
                    sb.attack = attack.min(STAT_XP_CAP_GEN12);
                    sb.defense = defense.min(STAT_XP_CAP_GEN12);
                    sb.speed = speed.min(STAT_XP_CAP_GEN12);
                    sb.special_attack = special_attack.min(STAT_XP_CAP_GEN12);
                    sb.special_defense = special_defense.min(STAT_XP_CAP_GEN12);
                }
                Gen::Three | Gen::Four | Gen::Five => {
                    let mut total = 0;
                    let (v, t) = actual_addable_evs(0, hp, total);
                    sb.hp = v;
                    total = t;
                    let (v, t) = actual_addable_evs(0, attack, total);
                    sb.attack = v;
                    total = t;
                    let (v, t) = actual_addable_evs(0, defense, total);
                    sb.defense = v;
                    total = t;
                    let (v, t) = actual_addable_evs(0, speed, total);
                    sb.speed = v;
                    total = t;
                    let (v, t) = actual_addable_evs(0, special_attack, total);
                    sb.special_attack = v;
                    total = t;
                    let (v, _t) = actual_addable_evs(0, special_defense, total);
                    sb.special_defense = v;
                }
            }
        }
        sb
    }

    pub fn zero(gen: Gen, is_stat_xp: bool) -> StatBlock {
        StatBlock::new(gen, 0, 0, 0, 0, 0, 0, is_stat_xp)
    }

    pub fn uniform(gen: Gen, v: i64) -> StatBlock {
        StatBlock::new(gen, v, v, v, v, v, v, false)
    }

    /// Python `copy(stat_block)` followed by `_is_stat_xp = True` (no caps
    /// re-applied, exactly like the Python shallow copy).
    pub fn as_stat_xp(&self) -> StatBlock {
        let mut c = *self;
        c.is_stat_xp = true;
        c
    }

    pub fn with_gen(&self, gen: Gen) -> StatBlock {
        let mut c = *self;
        c.gen = gen;
        c
    }

    pub fn add(&self, other: &StatBlock) -> StatBlock {
        if self.is_stat_xp && matches!(self.gen, Gen::Three | Gen::Four | Gen::Five) {
            let mut cur_total = self.hp + self.attack + self.defense + self.special_attack + self.special_defense + self.speed;
            cur_total = cur_total.min(TOTAL_EV_CAP);
            let (add_hp, t) = actual_addable_evs(self.hp, other.hp, cur_total);
            cur_total = t;
            let (add_atk, t) = actual_addable_evs(self.attack, other.attack, cur_total);
            cur_total = t;
            let (add_def, t) = actual_addable_evs(self.defense, other.defense, cur_total);
            cur_total = t;
            let (add_spe, t) = actual_addable_evs(self.speed, other.speed, cur_total);
            cur_total = t;
            let (add_spa, t) = actual_addable_evs(self.special_attack, other.special_attack, cur_total);
            cur_total = t;
            let (add_spd, _t) = actual_addable_evs(self.special_defense, other.special_defense, cur_total);
            return StatBlock::new(
                self.gen,
                self.hp + add_hp,
                self.attack + add_atk,
                self.defense + add_def,
                self.special_attack + add_spa,
                self.special_defense + add_spd,
                self.speed + add_spe,
                self.is_stat_xp,
            );
        }
        StatBlock::new(
            self.gen,
            self.hp + other.hp,
            self.attack + other.attack,
            self.defense + other.defense,
            self.special_attack + other.special_attack,
            self.special_defense + other.special_defense,
            self.speed + other.speed,
            self.is_stat_xp,
        )
    }

    pub fn subtract(&self, other: &StatBlock) -> StatBlock {
        StatBlock::new(
            self.gen,
            self.hp - other.hp,
            self.attack - other.attack,
            self.defense - other.defense,
            self.special_attack - other.special_attack,
            self.special_defense - other.special_defense,
            self.speed - other.speed,
            self.is_stat_xp,
        )
    }

    /// `StatBlock.serialize()`
    pub fn serialize(&self) -> Value {
        xpr_core::pyjson::object(vec![
            (consts::HP, Value::from(self.hp)),
            (consts::ATTACK, Value::from(self.attack)),
            (consts::DEFENSE, Value::from(self.defense)),
            (consts::SPEED, Value::from(self.speed)),
            (consts::SPECIAL_ATTACK, Value::from(self.special_attack)),
            (consts::SPECIAL_DEFENSE, Value::from(self.special_defense)),
        ])
    }

    /// `__repr__`
    pub fn py_repr(&self) -> String {
        format!(
            "HP: {}, attack: {}, defense: {}, special attack: {}, special_defense: {}, speed: {}, is_stat_xp: {}",
            self.hp,
            self.attack,
            self.defense,
            self.special_attack,
            self.special_defense,
            self.speed,
            if self.is_stat_xp { "True" } else { "False" }
        )
    }

    /// Stat value by the short stat key (`hp`, `atk`, ...).
    pub fn get_short(&self, key: &str) -> Option<i64> {
        match key {
            consts::HP => Some(self.hp),
            consts::ATK => Some(self.attack),
            consts::DEF => Some(self.defense),
            consts::SPA => Some(self.special_attack),
            consts::SPD => Some(self.special_defense),
            consts::SPE => Some(self.speed),
            _ => None,
        }
    }

    pub fn calc_level_stats(
        &self,
        level: i64,
        dvs: &StatBlock,
        stat_xp: &StatBlock,
        badges: &BadgeList,
        nature: Nature,
        held_item: Option<&str>,
    ) -> StatBlock {
        stats::calc_level_stats(self, level, dvs, stat_xp, badges, nature, held_item)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn calc_battle_stats(
        &self,
        level: i64,
        dvs: &StatBlock,
        stat_xp: &StatBlock,
        stage_modifiers: &StageModifiers,
        badges: Option<&BadgeList>,
        nature: Nature,
        held_item: Option<&str>,
        is_crit: bool,
        field_status: Option<&FieldStatus>,
    ) -> StatBlock {
        stats::calc_battle_stats(
            self,
            level,
            dvs,
            stat_xp,
            stage_modifiers,
            badges,
            nature,
            held_item,
            is_crit,
            field_status,
        )
    }
}

/// `_get_actual_addable_evs`
pub fn actual_addable_evs(cur_stat_ev: i64, new_stat_ev: i64, cur_total_ev: i64) -> (i64, i64) {
    let mut actual = (cur_stat_ev + new_stat_ev).min(SINGLE_STAT_EV_CAP) - cur_stat_ev;
    actual = actual.max(0);
    actual = (cur_total_ev + actual).min(TOTAL_EV_CAP) - cur_total_ev;
    actual = actual.max(0);
    (actual, cur_total_ev + actual)
}

// ---------------------------------------------------------------------------
// Species / trainers / items / moves
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct PokemonSpecies {
    pub name: String,
    pub growth_rate: String,
    pub base_xp: i64,
    pub first_type: String,
    pub second_type: String,
    pub stats: StatBlock,
    pub initial_moves: Vec<String>,
    pub levelup_moves: Vec<(i64, String)>,
    pub tmhm_moves: Vec<String>,
    pub stat_xp_yield: StatBlock,
    pub abilities: Vec<String>,
    pub weight: Option<f64>,
}

/// Per-Pokémon custom move data: `{ "player": {move: selection}, "enemy": {...} }`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CustomMoveData {
    pub player: IndexMap<String, String>,
    pub enemy: IndexMap<String, String>,
}

impl CustomMoveData {
    pub fn side(&self, is_player: bool) -> &IndexMap<String, String> {
        if is_player {
            &self.player
        } else {
            &self.enemy
        }
    }

    pub fn side_mut(&mut self, is_player: bool) -> &mut IndexMap<String, String> {
        if is_player {
            &mut self.player
        } else {
            &mut self.enemy
        }
    }

    pub fn is_empty(&self) -> bool {
        self.player.is_empty() && self.enemy.is_empty()
    }

    pub fn to_json(&self) -> Value {
        let side = |m: &IndexMap<String, String>| {
            let mut o = serde_json::Map::new();
            for (k, v) in m {
                o.insert(k.clone(), Value::String(v.clone()));
            }
            Value::Object(o)
        };
        xpr_core::pyjson::object(vec![
            (consts::PLAYER_KEY, side(&self.player)),
            (consts::ENEMY_KEY, side(&self.enemy)),
        ])
    }

    /// Python: `copy.deepcopy(x.custom_move_data) or {}` then `setdefault` both keys.
    pub fn from_json(v: Option<&Value>) -> CustomMoveData {
        let mut result = CustomMoveData::default();
        if let Some(Value::Object(o)) = v {
            for (k, side) in o {
                let target = match k.as_str() {
                    consts::PLAYER_KEY => &mut result.player,
                    consts::ENEMY_KEY => &mut result.enemy,
                    _ => continue,
                };
                if let Value::Object(entries) = side {
                    for (mv, sel) in entries {
                        target.insert(mv.clone(), match sel {
                            Value::String(s) => s.clone(),
                            other => xpr_core::pyjson::python_str_number(other),
                        });
                    }
                }
            }
        }
        result
    }
}

#[derive(Clone, Debug)]
pub struct EnemyPkmn {
    pub name: String,
    pub level: i64,
    pub xp: i64,
    pub move_list: Vec<Option<String>>,
    pub cur_stats: StatBlock,
    pub base_stats: StatBlock,
    pub dvs: StatBlock,
    pub stat_xp: StatBlock,
    pub badges: Option<BadgeList>,
    pub held_item: Option<String>,
    pub custom_move_data: Option<CustomMoveData>,
    pub is_trainer_mon: bool,
    pub exp_split: i64,
    pub mon_order: i64,
    pub definition_order: i64,
    pub ability: String,
    pub nature: Nature,
}

impl EnemyPkmn {
    /// Python `__eq__`
    pub fn py_eq(&self, other: &EnemyPkmn) -> bool {
        self.name == other.name
            && self.level == other.level
            && self.cur_stats == other.cur_stats
            && self.xp == other.xp
            && self.move_list == other.move_list
            && self.base_stats == other.base_stats
            && self.dvs == other.dvs
            && self.stat_xp == other.stat_xp
            && match (&self.badges, &other.badges) {
                (None, None) => true,
                (Some(a), Some(b)) => a == b,
                _ => false,
            }
            && self.held_item == other.held_item
            && self.nature == other.nature
            && self.ability == other.ability
    }

    pub fn held_item_str(&self) -> &str {
        self.held_item.as_deref().unwrap_or("")
    }

    /// Python's `move_list` as the list of strings it holds (None -> "").
    pub fn move_names(&self) -> Vec<String> {
        self.move_list.iter().map(|m| m.clone().unwrap_or_default()).collect()
    }

    pub fn has_move(&self, name: &str) -> bool {
        self.move_list.iter().any(|m| m.as_deref() == Some(name))
    }

    pub fn move_index(&self, name: &str) -> Option<usize> {
        self.move_list.iter().position(|m| m.as_deref() == Some(name))
    }

    pub fn to_string(&self, verbose: bool) -> String {
        if verbose {
            let moves: Vec<String> = self
                .move_list
                .iter()
                .map(|m| match m {
                    Some(s) => xpr_core::pyjson::python_repr_str(s),
                    None => "None".to_string(),
                })
                .collect();
            format!(
                "Lv {}: {} (Held: {}, Nature: {}) ({}, {}, {}, {}, {}, {}), ([{}])",
                self.level,
                self.name,
                self.held_item.as_deref().unwrap_or("None"),
                self.nature.display_name(),
                self.cur_stats.hp,
                self.cur_stats.attack,
                self.cur_stats.defense,
                self.cur_stats.special_attack,
                self.cur_stats.special_defense,
                self.cur_stats.speed,
                moves.join(", ")
            )
        } else {
            format!("Lv {}: {}", self.level, self.name)
        }
    }

    pub fn get_battle_stats(&self, stages: &StageModifiers, is_crit: bool, mon_field: Option<&FieldStatus>) -> StatBlock {
        self.base_stats.calc_battle_stats(
            self.level,
            &self.dvs,
            &self.stat_xp,
            stages,
            self.badges.as_ref(),
            self.nature,
            self.held_item.as_deref(),
            is_crit,
            mon_field,
        )
    }
}

#[derive(Clone, Debug)]
pub struct Trainer {
    pub trainer_class: String,
    pub name: String,
    pub location: String,
    pub money: i64,
    pub pkmn: Vec<EnemyPkmn>,
    pub rematch: bool,
    pub trainer_id: i64,
    pub refightable: bool,
    pub double_battle: bool,
}

#[derive(Clone, Debug)]
pub struct BaseItem {
    pub name: String,
    pub is_key_item: bool,
    pub purchase_price: i64,
    pub sell_price: i64,
    pub marts: Vec<String>,
    pub move_name: Option<String>,
}

impl BaseItem {
    pub fn new(name: String, is_key_item: bool, purchase_price: i64, marts: Vec<String>, move_name: Option<String>) -> BaseItem {
        BaseItem {
            sell_price: purchase_price.div_euclid(2),
            name,
            is_key_item,
            purchase_price,
            marts,
            move_name,
        }
    }
}

/// One entry of a move's `effects` list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MoveEffect {
    pub stat: Option<String>,
    pub modifier: Option<i64>,
    pub target: Option<String>,
    pub chance: Option<i64>,
    pub status: Option<String>,
}

impl MoveEffect {
    pub fn from_json(v: &Value) -> MoveEffect {
        MoveEffect {
            stat: xpr_core::pyjson::get_str(v, consts::STAT_KEY).map(|s| s.to_string()),
            modifier: xpr_core::pyjson::get_i64(v, consts::MODIFIER_KEY),
            target: xpr_core::pyjson::get_str(v, consts::TARGET_KEY).map(|s| s.to_string()),
            chance: xpr_core::pyjson::get_i64(v, "chance"),
            status: xpr_core::pyjson::get_str(v, "status").map(|s| s.to_string()),
        }
    }

    pub fn has_stat_mod(&self) -> bool {
        self.stat.is_some() && self.modifier.is_some()
    }
}

#[derive(Clone, Debug)]
pub struct Move {
    pub name: String,
    pub accuracy: Option<i64>,
    pub pp: Option<i64>,
    pub base_power: Option<i64>,
    pub move_type: String,
    pub effects: Vec<MoveEffect>,
    pub attack_flavor: Vec<String>,
    pub targeting: String,
    pub category: String,
    pub has_field_effect: bool,
}

impl Move {
    pub fn has_flavor(&self, flavor: &str) -> bool {
        self.attack_flavor.iter().any(|f| f == flavor)
    }

    /// `move.base_power` treated the way the Python damage code tests it
    /// (`is None or == 0`).
    pub fn base_power_or_zero(&self) -> i64 {
        self.base_power.unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrainerTimingStats {
    pub intro_time: f64,
    pub outro_time: f64,
    pub ko_time: f64,
    pub send_out_time: f64,
}

impl TrainerTimingStats {
    pub fn get_optimal_exp_per_second(&self, num_pokemon: i64, total_exp: i64) -> f64 {
        let n = num_pokemon as f64;
        (total_exp as f64) / (self.intro_time + self.outro_time + (self.ko_time * n) + (self.send_out_time * (n - 1.0)))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FieldStatus {
    pub light_screen: bool,
    pub reflect: bool,
    pub gravity: bool,
    pub magnet_rise: bool,
    pub miracle_eye: bool,
    pub power_trick: bool,
    pub roost: bool,
    pub tailwind: bool,
    pub trick_room: bool,
    pub worry_seed: bool,
    pub gastro_acid: bool,
    pub slow_start: bool,
}

impl FieldStatus {
    pub fn apply_move(&self, move_name: &str) -> FieldStatus {
        let mut result = *self;
        let s = sanitize_string(move_name);
        match s.as_str() {
            consts::LIGHTSCREEN_SANITIZED_MOVE_NAME => result.light_screen = true,
            consts::REFLECT_SANITIZED_MOVE_NAME => result.reflect = true,
            consts::GRAVITY_SANITIZED_MOVE_NAME => result.gravity = true,
            consts::MAGNET_RISE_SANITIZED_MOVE_NAME => result.magnet_rise = true,
            consts::MIRACLE_EYE_SANITIZED_MOVE_NAME => result.miracle_eye = true,
            consts::POWER_TRICK_SANITIZED_MOVE_NAME => result.power_trick = true,
            consts::ROOST_SANITIZED_MOVE_NAME => result.roost = true,
            consts::TAILWIND_SANITIZED_MOVE_NAME => result.tailwind = true,
            consts::TRICK_ROOM_SANITIZED_MOVE_NAME => result.trick_room = true,
            consts::WORRY_SEED_SANITIZED_MOVE_NAME => result.worry_seed = true,
            consts::GASTRO_ACID_SANITIZED_MOVE_NAME => result.gastro_acid = true,
            consts::SLOW_START_SANITIZED_NAME => result.slow_start = true,
            _ => {}
        }
        result
    }

    /// `setattr(result, toggle_id, True)` for the screen toggle ids.
    pub fn set_by_name(&mut self, name: &str) -> bool {
        match name {
            "light_screen" => self.light_screen = true,
            "reflect" => self.reflect = true,
            "gravity" => self.gravity = true,
            "magnet_rise" => self.magnet_rise = true,
            "miracle_eye" => self.miracle_eye = true,
            "power_trick" => self.power_trick = true,
            "roost" => self.roost = true,
            "tailwind" => self.tailwind = true,
            "trick_room" => self.trick_room = true,
            "worry_seed" => self.worry_seed = true,
            "gastro_acid" => self.gastro_acid = true,
            "slow_start" => self.slow_start = true,
            _ => return false,
        }
        true
    }
}

pub type SpeciesRef = Arc<PokemonSpecies>;
