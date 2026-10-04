//! TM / HM numbers (Solodex `getTmHmCode`, `utils/tmhmSort.ts`).

use std::cmp::Ordering;
use std::collections::HashMap;

use indexmap::IndexMap;

use crate::games;
use crate::moves::move_alias;
use crate::store::store;

pub struct TmHmTables {
    /// key -> code -> move (as stored)
    pub by_key: IndexMap<String, IndexMap<String, String>>,
    /// key -> move -> code
    reverse: HashMap<String, HashMap<String, String>>,
}

impl TmHmTables {
    pub(crate) fn new(raw: IndexMap<String, IndexMap<String, String>>) -> TmHmTables {
        let mut reverse: HashMap<String, HashMap<String, String>> = HashMap::new();
        for (key, entries) in &raw {
            let r = reverse.entry(key.clone()).or_default();
            for (code, mv) in entries {
                r.insert(mv.clone(), code.clone());
            }
        }
        // XY TM94 is Rock Smash, but Secret Power also appears in XY learnsets as TM94 (shared with ORAS)
        if let Some(xy) = reverse.get_mut("6xy") {
            xy.insert("Secret Power".to_string(), "TM94".to_string());
        }
        // HGSS has Whirlpool as HM05 instead of Defog
        if let Some(gen4) = reverse.get("4").cloned() {
            let mut hgss = gen4;
            hgss.remove("Defog");
            hgss.insert("Whirlpool".to_string(), "HM05".to_string());
            reverse.insert("4hgss".to_string(), hgss);
        }
        TmHmTables { by_key: raw, reverse }
    }
}

/// The table key of a game (`GAME_TO_TMHM_KEY`, else its generation).
pub fn tmhm_key(game: &str) -> String {
    match game {
        "X and Y" => "6xy".to_string(),
        "HeartGold and SoulSilver" => "4hgss".to_string(),
        "Legends Z-A" => "9za".to_string(),
        "Brilliant Diamond and Shining Pearl" => "8bdsp".to_string(),
        _ => games::game_gen(game).to_string(),
    }
}

/// "TM24" / "HM03" for a move in a game; `None` when it is no TM there.
pub fn tmhm_code(move_name: &str, game: &str) -> Option<String> {
    let t = store().tmhm();
    let table = t.reverse.get(&tmhm_key(game))?;
    table.get(move_name).or_else(|| move_alias(move_name).and_then(|a| table.get(a))).cloned()
}

/// Every (code, move) of a game's TM/HM list, in table order.
pub fn tmhm_list(game: &str) -> Vec<(String, String)> {
    let t = store().tmhm();
    let key = tmhm_key(game);
    let base = if key == "4hgss" { "4".to_string() } else { key.clone() };
    let Some(table) = t.by_key.get(&base) else { return Vec::new() };
    let mut out: Vec<(String, String)> = table.iter().map(|(c, m)| (c.clone(), m.clone())).collect();
    if key == "4hgss" {
        for (c, m) in out.iter_mut() {
            if c == "HM05" {
                *m = "Whirlpool".to_string();
            }
        }
    }
    out
}

fn prefix_order(code: &str) -> u8 {
    if code.starts_with("TM") {
        0
    } else if code.starts_with("TR") {
        1
    } else if code.starts_with("HM") {
        2
    } else {
        3
    }
}

/// TM before TR before HM, then by number.
pub fn compare_tmhm(a: &str, b: &str) -> Ordering {
    let num = |s: &str| s.get(2..).and_then(|n| n.parse::<i64>().ok()).unwrap_or(0);
    prefix_order(a).cmp(&prefix_order(b)).then_with(|| num(a).cmp(&num(b)))
}
