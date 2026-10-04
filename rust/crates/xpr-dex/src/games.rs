//! Static game tables (Solodex `data/games.ts`) and the mapping between the
//! Dex's games and the router's versions.

/// Every game with a Pokédex, in release order.
pub const GAMES: [&str; 21] = [
    "Red and Blue",
    "Yellow",
    "Gold and Silver",
    "Crystal",
    "Ruby and Sapphire",
    "Emerald",
    "FireRed and LeafGreen",
    "Diamond and Pearl",
    "Platinum",
    "HeartGold and SoulSilver",
    "Black",
    "Black 2 and White 2",
    "X and Y",
    "Omega Ruby and Alpha Sapphire",
    "Sun and Moon",
    "Ultra Sun and Ultra Moon",
    "Sword and Shield",
    "Brilliant Diamond and Shining Pearl",
    "Legends Arceus",
    "Scarlet and Violet",
    "Legends Z-A",
];

pub struct GenGroup {
    pub label: &'static str,
    pub gen: u8,
    pub games: &'static [&'static str],
    pub color: &'static str,
}

pub const GEN_GROUPS: [GenGroup; 9] = [
    GenGroup { label: "Gen 1", gen: 1, games: &["Red and Blue", "Yellow"], color: "#FFB300" },
    GenGroup { label: "Gen 2", gen: 2, games: &["Gold and Silver", "Crystal"], color: "#29B6F6" },
    GenGroup { label: "Gen 3", gen: 3, games: &["Ruby and Sapphire", "Emerald", "FireRed and LeafGreen"], color: "#2E7D32" },
    GenGroup { label: "Gen 4", gen: 4, games: &["Diamond and Pearl", "Platinum", "HeartGold and SoulSilver"], color: "#78909C" },
    GenGroup { label: "Gen 5", gen: 5, games: &["Black", "Black 2 and White 2"], color: "#616161" },
    GenGroup { label: "Gen 6", gen: 6, games: &["X and Y", "Omega Ruby and Alpha Sapphire"], color: "#1565C0" },
    GenGroup { label: "Gen 7", gen: 7, games: &["Sun and Moon", "Ultra Sun and Ultra Moon"], color: "#F57F17" },
    GenGroup { label: "Gen 8", gen: 8, games: &["Sword and Shield", "Brilliant Diamond and Shining Pearl", "Legends Arceus"], color: "#880E4F" },
    GenGroup { label: "Gen 9", gen: 9, games: &["Scarlet and Violet", "Legends Z-A"], color: "#6A1B9A" },
];

/// The game's accent colour (hex).
pub fn game_color(game: &str) -> &'static str {
    match game {
        "Red and Blue" => "#CC0000",
        "Yellow" => "#FFB300",
        "Gold and Silver" => "#B8860B",
        "Crystal" => "#29B6F6",
        "Ruby and Sapphire" => "#C62828",
        "Emerald" => "#2E7D32",
        "FireRed and LeafGreen" => "#E64A19",
        "Diamond and Pearl" => "#5C6BC0",
        "Platinum" => "#78909C",
        "HeartGold and SoulSilver" => "#F9A825",
        "Black" => "#616161",
        "Black 2 and White 2" => "#78909C",
        "X and Y" => "#1565C0",
        "Omega Ruby and Alpha Sapphire" => "#BF360C",
        "Sun and Moon" => "#F57F17",
        "Ultra Sun and Ultra Moon" => "#E65100",
        "Sword and Shield" => "#880E4F",
        "Brilliant Diamond and Shining Pearl" => "#5C6BC0",
        "Legends Arceus" => "#1B5E20",
        "Scarlet and Violet" => "#6A1B9A",
        "Legends Z-A" => "#4A148C",
        _ => "#6B7280",
    }
}

/// The short label of the game toggle ("RB", "HGSS", ...).
pub fn game_abbrev(game: &str) -> &str {
    match game {
        "Red and Blue" => "RB",
        "Yellow" => "Y",
        "Gold and Silver" => "GS",
        "Crystal" => "C",
        "Ruby and Sapphire" => "RS",
        "Emerald" => "E",
        "FireRed and LeafGreen" => "FRLG",
        "Diamond and Pearl" => "DP",
        "Platinum" => "Pt",
        "HeartGold and SoulSilver" => "HGSS",
        "Black" => "BW",
        "Black 2 and White 2" => "BW2",
        "X and Y" => "XY",
        "Omega Ruby and Alpha Sapphire" => "ORAS",
        "Sun and Moon" => "SM",
        "Ultra Sun and Ultra Moon" => "USUM",
        "Sword and Shield" => "SwSh",
        "Brilliant Diamond and Shining Pearl" => "BDSP",
        "Legends Arceus" => "PLA",
        "Scarlet and Violet" => "SV",
        "Legends Z-A" => "ZA",
        other => other,
    }
}

/// The game's generation (1-9); 0 for an unknown game.
pub fn game_gen(game: &str) -> u8 {
    GEN_GROUPS.iter().find(|g| g.games.contains(&game)).map(|g| g.gen).unwrap_or(0)
}

/// The gen group a game belongs to.
pub fn gen_group_of(game: &str) -> Option<&'static GenGroup> {
    GEN_GROUPS.iter().find(|g| g.games.contains(&game))
}

/// Canonical spelling of a game name (`GAMES` entry) from any case.
pub fn canonical_game(game: &str) -> Option<&'static str> {
    GAMES.iter().copied().find(|g| g.eq_ignore_ascii_case(game))
}

/// The file stem of a game's tables (`"Red and Blue"` -> `red_and_blue`).
pub fn slug(game: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in game.chars() {
        if c.is_ascii_alphanumeric() {
            if gap && !out.is_empty() {
                out.push('_');
            }
            gap = false;
            out.push(c.to_ascii_lowercase());
        } else {
            gap = true;
        }
    }
    out
}

// ---- router versions ------------------------------------------------------------------------

/// The router's built-in versions that share a game's Pokédex (and whose
/// trainer data the Trainers / Damage tabs use). Empty for gens 6-9.
pub fn router_versions(game: &str) -> &'static [&'static str] {
    match game {
        "Red and Blue" => &["Red", "Blue"],
        "Yellow" => &["Yellow"],
        "Gold and Silver" => &["Gold", "Silver"],
        "Crystal" => &["Crystal"],
        "Ruby and Sapphire" => &["Ruby", "Sapphire"],
        "Emerald" => &["Emerald"],
        "FireRed and LeafGreen" => &["FireRed", "LeafGreen"],
        "Diamond and Pearl" => &["Diamond", "Pearl"],
        "Platinum" => &["Platinum"],
        "HeartGold and SoulSilver" => &["HeartGold", "SoulSilver"],
        "Black" => &["Black", "White"],
        "Black 2 and White 2" => &["Black 2", "White 2"],
        _ => &[],
    }
}

/// The Dex game of a built-in router version (`"Blue"` -> `"Red and Blue"`).
/// For a custom gen pass its base version.
pub fn game_for_router_version(version: &str) -> Option<&'static str> {
    GAMES.iter().copied().find(|g| router_versions(g).contains(&version))
}

/// Games that have router trainer data (Solodex's `GAMES_WITH_TRAINERS`).
pub fn games_with_trainers() -> Vec<&'static str> {
    GAMES.iter().copied().filter(|g| !router_versions(g).is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_and_groups() {
        assert_eq!(slug("Red and Blue"), "red_and_blue");
        assert_eq!(slug("Legends Z-A"), "legends_z_a");
        assert_eq!(slug("Black 2 and White 2"), "black_2_and_white_2");
        for g in GAMES {
            assert!(game_gen(g) >= 1, "{} has no gen", g);
        }
        assert_eq!(GEN_GROUPS.iter().map(|g| g.games.len()).sum::<usize>(), GAMES.len());
        assert_eq!(game_for_router_version("White 2"), Some("Black 2 and White 2"));
        assert_eq!(games_with_trainers().len(), 12);
    }
}
