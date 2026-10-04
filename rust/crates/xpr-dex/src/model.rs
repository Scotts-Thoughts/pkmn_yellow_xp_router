//! The data types (Solodex `types/pokemon.ts`).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaseStats {
    pub hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub speed: i32,
    pub special_attack: i32,
    pub special_defense: i32,
}

/// The six stats, for code that walks them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StatKey {
    Hp,
    Attack,
    Defense,
    SpecialAttack,
    SpecialDefense,
    Speed,
}

impl StatKey {
    pub const ALL: [StatKey; 6] = [StatKey::Hp, StatKey::Attack, StatKey::Defense, StatKey::SpecialAttack, StatKey::SpecialDefense, StatKey::Speed];

    /// The data key (`special_attack`, ...).
    pub fn key(self) -> &'static str {
        match self {
            StatKey::Hp => "hp",
            StatKey::Attack => "attack",
            StatKey::Defense => "defense",
            StatKey::SpecialAttack => "special_attack",
            StatKey::SpecialDefense => "special_defense",
            StatKey::Speed => "speed",
        }
    }
}

impl BaseStats {
    pub fn get(&self, k: StatKey) -> i32 {
        match k {
            StatKey::Hp => self.hp,
            StatKey::Attack => self.attack,
            StatKey::Defense => self.defense,
            StatKey::SpecialAttack => self.special_attack,
            StatKey::SpecialDefense => self.special_defense,
            StatKey::Speed => self.speed,
        }
    }

    /// All six stats summed (gen 1 callers use [`crate::pokedex::base_stat_total`]).
    pub fn sum(&self) -> i32 {
        self.hp + self.attack + self.defense + self.special_attack + self.special_defense + self.speed
    }
}

/// An evolution parameter: a level, an item / move / location name, or none.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EvoParam {
    Num(f64),
    Text(String),
}

impl std::fmt::Display for EvoParam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvoParam::Num(n) if n.fract() == 0.0 => write!(f, "{}", *n as i64),
            EvoParam::Num(n) => write!(f, "{}", n),
            EvoParam::Text(s) => f.write_str(s),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvolutionEntry {
    pub species: String,
    /// `None` for family members this entry does not evolve into
    pub method: Option<String>,
    #[serde(default)]
    pub parameter: Option<EvoParam>,
}

/// One species in one game.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PokemonData {
    pub species: String,
    #[serde(default)]
    pub rom_id: i64,
    pub national_dex_number: i32,
    pub base_stats: BaseStats,
    #[serde(default)]
    pub ev_yield: BaseStats,
    pub type_1: String,
    pub type_2: String,
    #[serde(default)]
    pub catch_rate: Option<i32>,
    #[serde(default)]
    pub base_experience: Option<i32>,
    #[serde(default)]
    pub common_item: Option<String>,
    #[serde(default)]
    pub rare_item: Option<String>,
    #[serde(default)]
    pub gender_ratio: Option<f64>,
    #[serde(default)]
    pub egg_cycles: Option<i32>,
    #[serde(default)]
    pub base_friendship: Option<i32>,
    pub growth_rate: String,
    #[serde(default)]
    pub egg_group_1: Option<String>,
    #[serde(default)]
    pub egg_group_2: Option<String>,
    #[serde(default)]
    pub abilities: Vec<String>,
    #[serde(default)]
    pub hidden_ability: Option<String>,
    /// (level, move); level 0 = learned on evolution, -1 = Move Reminder only
    #[serde(default)]
    pub level_up_learnset: Vec<(i32, String)>,
    #[serde(default)]
    pub tm_hm_learnset: Vec<String>,
    #[serde(default)]
    pub tutor_learnset: Vec<String>,
    #[serde(default)]
    pub egg_moves: Vec<String>,
    #[serde(default)]
    pub transfer_learnset: Vec<String>,
    /// moves only a pre-evolution can learn in this game
    #[serde(default)]
    pub prior_evolution_learnset: Vec<String>,
    #[serde(default)]
    pub form_change_learnset: Vec<String>,
    #[serde(default)]
    pub zygarde_cube_learnset: Vec<String>,
    #[serde(default)]
    pub light_ball_egg_learnset: Vec<String>,
    #[serde(default)]
    pub weight: Option<f64>,
    #[serde(default)]
    pub evolution_family: Vec<EvolutionEntry>,
}

impl PokemonData {
    /// The second type, or `None` for a single-typed species (the data
    /// repeats the first type).
    pub fn second_type(&self) -> Option<&str> {
        (self.type_2 != self.type_1 && !self.type_2.is_empty()).then_some(self.type_2.as_str())
    }

    pub fn types(&self) -> Vec<&str> {
        let mut v = vec![self.type_1.as_str()];
        if let Some(t) = self.second_type() {
            v.push(t);
        }
        v
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MoveData {
    #[serde(default)]
    pub rom_id: i64,
    #[serde(rename = "move")]
    pub name: String,
    #[serde(rename = "type")]
    pub move_type: String,
    pub category: String,
    #[serde(default)]
    pub pp: Option<i32>,
    #[serde(default)]
    pub power: Option<i32>,
    #[serde(default)]
    pub accuracy: Option<i32>,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub effect: String,
    #[serde(default)]
    pub effect_chance: Option<i32>,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub makes_contact: bool,
    #[serde(default)]
    pub affected_by_protect: bool,
    #[serde(default)]
    pub affected_by_magic_coat: bool,
    #[serde(default)]
    pub affected_by_snatch: bool,
    #[serde(default)]
    pub affected_by_mirror_move: bool,
    #[serde(default)]
    pub affected_by_kings_rock: bool,
    #[serde(default)]
    pub description: String,
}

impl MoveData {
    /// A damaging move: not Status and with a positive power.
    pub fn is_damaging(&self) -> bool {
        self.category != "Status" && self.power.map(|p| p > 0).unwrap_or(false)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvolutionStage {
    Single,
    First,
    Middle,
    Final,
    Mega,
}

impl EvolutionStage {
    pub fn label(self) -> &'static str {
        match self {
            EvolutionStage::Single => "Single",
            EvolutionStage::First => "First",
            EvolutionStage::Middle => "Middle",
            EvolutionStage::Final => "Final",
            EvolutionStage::Mega => "Mega",
        }
    }
}

/// A row of the cross-game species index.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PokemonListEntry {
    pub name: String,
    pub national_dex_number: i32,
    pub type_1: String,
    pub type_2: String,
    pub growth_rate: String,
    pub evolution_stage: EvolutionStage,
    /// games (in `GAMES` order) whose Pokédex has this species
    pub games: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EncounterEntry {
    pub location: String,
    pub method: String,
    pub min_level: i32,
    pub max_level: i32,
    pub chance: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NatureData {
    /// lower-case name ("lonely")
    pub nature: String,
    pub index: i32,
    /// `attack` | `defense` | `speed` | `specialAttack` | `specialDefense`
    pub increased: Option<String>,
    pub decreased: Option<String>,
    #[serde(default)]
    pub favorite_flavor: Option<String>,
    #[serde(default)]
    pub disliked_flavor: Option<String>,
}
