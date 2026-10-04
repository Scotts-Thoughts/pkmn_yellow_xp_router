//! Move data and spellings (Solodex `data/index.ts` move section and
//! `utils/moveNameCanonical.ts`).
//!
//! `moves.json` is keyed by generation and uses the modern spelling; trainer
//! data and `tmhm.json` use the period spelling and the Pokédex learnsets are
//! already respelled per game. Always look moves up through
//! [`get_move_data`] (alias-aware), never by exact key.

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use indexmap::IndexMap;

use crate::games;
use crate::model::MoveData;
use crate::store::store;

/// (modern, legacy, last gen that used the legacy spelling)
pub const MOVE_RENAMES: [(&str, &str, u8); 23] = [
    ("Ancient Power", "AncientPower", 5),
    ("Bubble Beam", "BubbleBeam", 5),
    ("Conversion 2", "Conversion2", 2),
    ("Double Slap", "DoubleSlap", 5),
    ("Dragon Breath", "DragonBreath", 5),
    ("Dynamic Punch", "DynamicPunch", 5),
    ("Extreme Speed", "ExtremeSpeed", 5),
    ("Feather Dance", "FeatherDance", 5),
    ("Feint Attack", "Faint Attack", 5),
    ("Grass Whistle", "GrassWhistle", 5),
    ("High Jump Kick", "Hi Jump Kick", 5),
    ("Poison Powder", "PoisonPowder", 5),
    ("Sand Attack", "Sand-Attack", 5),
    ("Self-Destruct", "Selfdestruct", 5),
    ("Smelling Salts", "SmellingSalt", 5),
    ("Smokescreen", "SmokeScreen", 5),
    ("Soft-Boiled", "Softboiled", 5),
    ("Solar Beam", "SolarBeam", 5),
    ("Sonic Boom", "SonicBoom", 5),
    ("Thunder Punch", "ThunderPunch", 5),
    ("Thunder Shock", "ThunderShock", 5),
    ("Vise Grip", "ViceGrip", 5),
    ("Vise Grip", "Vice Grip", 7),
];

/// `MOVE_NAME_ALIASES`: modern -> first legacy spelling, legacy -> modern.
fn aliases() -> &'static HashMap<&'static str, &'static str> {
    static A: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    A.get_or_init(|| {
        let mut m = HashMap::new();
        for (modern, legacy, _) in MOVE_RENAMES {
            m.entry(modern).or_insert(legacy);
            m.insert(legacy, modern);
        }
        m
    })
}

/// The other spelling of a renamed move, if any.
pub fn move_alias(name: &str) -> Option<&'static str> {
    aliases().get(name).copied()
}

/// The spelling a move had in `gen` (Feint Attack -> "Faint Attack" in gens
/// 1-5). Accepts any spelling.
pub fn move_name_for_gen(name: &str, gen: u8) -> String {
    let is_modern = MOVE_RENAMES.iter().any(|(m, _, _)| *m == name);
    let modern = if is_modern { Some(name) } else { aliases().get(name).copied() };
    let Some(modern) = modern else { return name.to_string() };
    let mut spellings: Vec<(u8, &str)> = MOVE_RENAMES.iter().filter(|(m, _, _)| *m == modern).map(|(_, l, g)| (*g, *l)).collect();
    if spellings.is_empty() {
        return name.to_string();
    }
    spellings.sort();
    spellings.iter().find(|(last, _)| gen <= *last).map(|(_, l)| l.to_string()).unwrap_or_else(|| modern.to_string())
}

pub fn move_name_for_game(name: &str, game: &str) -> String {
    match games::game_gen(game) {
        0 => name.to_string(),
        gen => move_name_for_gen(name, gen),
    }
}

/// Collapse spelling variants ("SolarBeam" / "Solar Beam", "Vice Grip" /
/// "Vise Grip") to one key for cross-generation comparison.
pub fn canonical_move_key(name: &str) -> String {
    let mut spaced = String::with_capacity(name.len() + 4);
    let mut prev_lower = false;
    for c in name.chars() {
        if prev_lower && c.is_ascii_uppercase() {
            spaced.push(' ');
        }
        prev_lower = c.is_ascii_lowercase();
        spaced.push(c);
    }
    let key: String = spaced.to_lowercase().chars().filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit()).collect();
    match key.as_str() {
        "vicegrip" => "visegrip".to_string(),
        "faintattack" => "feintattack".to_string(),
        "hijumpkick" => "highjumpkick".to_string(),
        _ => key,
    }
}

/// `MOVE_GAME_OVERRIDES`: per-game patches on top of the generation record.
fn apply_game_override(game: &str, m: &mut MoveData) {
    // Hypnosis was 70% accurate in Diamond/Pearl; Platinum and HGSS lowered it to 60%.
    if game == "Diamond and Pearl" && m.name == "Hypnosis" {
        m.accuracy = Some(70);
    }
}

fn has_game_override(game: &str, name: &str) -> bool {
    game == "Diamond and Pearl" && name == "Hypnosis"
}

pub struct MoveTables {
    pub by_gen: BTreeMap<u8, IndexMap<String, MoveData>>,
    intro_gen: HashMap<String, u8>,
    all_names: Vec<String>,
}

impl MoveTables {
    pub(crate) fn new(raw: IndexMap<String, IndexMap<String, MoveData>>) -> MoveTables {
        let mut by_gen = BTreeMap::new();
        for (g, table) in raw {
            if let Ok(n) = g.parse::<u8>() {
                by_gen.insert(n, table);
            }
        }
        let mut intro_gen = HashMap::new();
        for (g, table) in &by_gen {
            for name in table.keys() {
                intro_gen.entry(name.clone()).or_insert(*g);
            }
        }
        let mut all: Vec<String> = by_gen.values().flat_map(|t| t.keys().cloned()).collect();
        all.sort_by(|a, b| crate::text::locale_cmp(a, b));
        all.dedup();
        MoveTables { by_gen, intro_gen, all_names: all }
    }

    fn find_in_gen(&self, gen: u8, name: &str) -> Option<&MoveData> {
        let t = self.by_gen.get(&gen)?;
        t.get(name).or_else(|| move_alias(name).and_then(|a| t.get(a)))
    }

    pub fn max_gen(&self) -> u8 {
        self.by_gen.keys().copied().max().unwrap_or(0)
    }
}

/// Move details for a game: the game's generation first, then earlier
/// generations, then later ones (moves only documented in a later file).
pub fn get_move_data(name: &str, game: &str) -> Option<MoveData> {
    let gen = games::game_gen(game);
    if gen == 0 {
        return None;
    }
    let mut m = get_gen_move_data(name, gen)?.clone();
    apply_game_override(game, &mut m);
    Some(m)
}

/// Like [`get_move_data`] for a bare generation (no per-game overrides).
pub fn get_gen_move_data(name: &str, gen: u8) -> Option<&'static MoveData> {
    let t = store().moves();
    let max = t.max_gen();
    for g in (1..=gen.min(max)).rev() {
        if let Some(m) = t.find_in_gen(g, name) {
            return Some(m);
        }
    }
    for g in gen + 1..=max {
        if let Some(m) = t.find_in_gen(g, name) {
            return Some(m);
        }
    }
    None
}

/// Every move in a generation's table, sorted by name. With `game`, names
/// are respelled for it and its per-game overrides applied. Shadow moves
/// (Colosseum / XD) are left out.
pub fn get_moves_for_gen(gen: u8, game: Option<&str>) -> Vec<(String, MoveData)> {
    let t = store().moves();
    let Some(table) = t.by_gen.get(&gen) else { return Vec::new() };
    let mut out: Vec<(String, MoveData)> = table
        .iter()
        .filter(|(_, m)| m.move_type != "Shadow")
        .map(|(name, m)| {
            let mut data = m.clone();
            let shown = match game {
                Some(g) => {
                    if has_game_override(g, name) {
                        apply_game_override(g, &mut data);
                    }
                    move_name_for_game(name, g)
                }
                None => name.clone(),
            };
            (shown, data)
        })
        .collect();
    out.sort_by(|a, b| crate::text::locale_cmp(&a.0, &b.0));
    out
}

/// The generation a move first appears in.
pub fn move_introduction_gen(name: &str) -> Option<u8> {
    let t = store().moves();
    t.intro_gen.get(name).or_else(|| move_alias(name).and_then(|a| t.intro_gen.get(a))).copied()
}

/// True when the move is in `gen`'s table but not the previous gen's.
pub fn is_move_new_in_gen(name: &str, gen: u8) -> bool {
    if gen < 1 {
        return false;
    }
    let t = store().moves();
    if t.find_in_gen(gen, name).is_none() {
        return false;
    }
    gen == 1 || t.find_in_gen(gen - 1, name).is_none()
}

/// Every move name of every generation (modern spelling), sorted.
pub fn all_move_names() -> &'static [String] {
    &store().moves().all_names
}

/// The move's record in every generation that has it.
pub fn move_across_gens(name: &str) -> Vec<(u8, &'static MoveData)> {
    let t = store().moves();
    t.by_gen.keys().filter_map(|g| t.find_in_gen(*g, name).map(|m| (*g, m))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spellings() {
        assert_eq!(move_name_for_gen("Feint Attack", 3), "Faint Attack");
        assert_eq!(move_name_for_gen("Faint Attack", 6), "Feint Attack");
        assert_eq!(move_name_for_gen("Vise Grip", 4), "ViceGrip");
        assert_eq!(move_name_for_gen("Vise Grip", 7), "Vice Grip");
        assert_eq!(move_name_for_gen("ViceGrip", 8), "Vise Grip");
        assert_eq!(move_name_for_gen("Conversion 2", 3), "Conversion 2");
        assert_eq!(move_name_for_gen("Tackle", 1), "Tackle");
        assert_eq!(canonical_move_key("SolarBeam"), canonical_move_key("Solar Beam"));
        assert_eq!(canonical_move_key("Vice Grip"), canonical_move_key("Vise Grip"));
        assert_eq!(canonical_move_key("Hi Jump Kick"), "highjumpkick");
        assert_eq!(canonical_move_key("Self-Destruct"), canonical_move_key("Selfdestruct"));
    }
}
