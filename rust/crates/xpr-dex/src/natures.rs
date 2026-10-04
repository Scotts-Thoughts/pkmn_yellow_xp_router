//! Natures and the user's unobtainable-move lists (Solodex `natures.js`,
//! `unobtainable_moves.js`, `getUnobtainableMoveSets`).

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::model::NatureData;
use crate::store::store;

/// All natures in table (index) order: (Display name, data).
pub fn natures() -> Vec<(&'static str, &'static NatureData)> {
    store().natures().iter().map(|(k, v)| (k.as_str(), v)).collect()
}

pub fn nature(name: &str) -> Option<&'static NatureData> {
    store().natures().get(name)
}

pub fn nature_by_index(index: i64) -> Option<&'static str> {
    store().natures().iter().find(|(_, v)| v.index as i64 == index).map(|(k, _)| k.as_str())
}

/// Display label of a nature stat key (`specialAttack` -> "Sp. Atk").
pub fn nature_stat_label(key: &str) -> &str {
    match key {
        "attack" => "Attack",
        "defense" => "Defense",
        "speed" => "Speed",
        "specialAttack" => "Sp. Atk",
        "specialDefense" => "Sp. Def",
        other => other,
    }
}

/// Moves the user has marked as banned, conditional, or unobtainable in one game.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserBans {
    pub banned: Vec<String>,
    pub conditional: Vec<String>,
    #[serde(rename = "byGame")]
    pub by_game: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Debug, Default)]
pub struct UnobtainableMoveSets {
    pub banned: HashSet<String>,
    pub postgame: HashSet<String>,
    pub conditional: HashSet<String>,
}

/// unobtainable_moves.js keys that name a version rather than a Dex game
fn map_unobtainable_key(key: &str) -> &str {
    match key {
        "White" => "Black",
        "Black 2" | "White 2" => "Black 2 and White 2",
        other => other,
    }
}

pub fn static_banned_moves() -> Vec<String> {
    store().unobtainable_raw().get("Banned").cloned().unwrap_or_default()
}

pub fn static_conditional_moves() -> Vec<String> {
    store().unobtainable_raw().get("Conditional").cloned().unwrap_or_default()
}

/// The built-in postgame / unobtainable moves of a game.
pub fn static_postgame_moves(game: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (k, moves) in store().unobtainable_raw() {
        if k == "Banned" || k == "Conditional" {
            continue;
        }
        if map_unobtainable_key(k) == game {
            for m in moves {
                if !out.contains(m) {
                    out.push(m.clone());
                }
            }
        }
    }
    out
}

/// Built-in lists merged with the user's additions.
pub fn unobtainable_move_sets(game: &str, user: &UserBans) -> UnobtainableMoveSets {
    let mut banned: HashSet<String> = static_banned_moves().into_iter().collect();
    banned.extend(user.banned.iter().cloned());
    let mut postgame: HashSet<String> = static_postgame_moves(game).into_iter().collect();
    if let Some(v) = user.by_game.get(game) {
        postgame.extend(v.iter().cloned());
    }
    let mut conditional: HashSet<String> = static_conditional_moves().into_iter().collect();
    conditional.extend(user.conditional.iter().cloned());
    UnobtainableMoveSets { banned, postgame, conditional }
}
