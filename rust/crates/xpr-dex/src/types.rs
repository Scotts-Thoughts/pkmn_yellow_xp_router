//! Type charts and matchups (Solodex `data/index.ts` effectiveness section,
//! `constants/effectiveness.ts`).
//!
//! The raw data covers gens 1-4 (its gen 5 table is replaced, as Solodex
//! does): gen 5 copies gen 4; gens 6+ drop Steel's Ghost / Dark resistances
//! and add Fairy.

use std::collections::HashMap;

use indexmap::IndexMap;

use crate::games;
use crate::store::store;

/// attacking type -> defending type -> multiplier
pub type Chart = IndexMap<String, IndexMap<String, f64>>;

pub struct TypeCharts {
    by_gen: HashMap<u8, Chart>,
}

const GEN1_TYPES: [&str; 15] = ["Normal", "Fire", "Water", "Electric", "Grass", "Ice", "Fighting", "Poison", "Ground", "Flying", "Psychic", "Bug", "Rock", "Ghost", "Dragon"];

impl TypeCharts {
    pub(crate) fn new(raw: IndexMap<String, Chart>) -> TypeCharts {
        let mut by_gen: HashMap<u8, Chart> = HashMap::new();
        for (g, chart) in raw {
            if let Ok(n) = g.parse::<u8>() {
                if n <= 4 {
                    by_gen.insert(n, chart);
                }
            }
        }
        if let Some(gen4) = by_gen.get(&4).cloned() {
            let mut gen6 = gen4.clone();
            by_gen.insert(5, gen4);
            set(&mut gen6, "Ghost", "Steel", 1.0);
            set(&mut gen6, "Dark", "Steel", 1.0);
            let defenders: Vec<String> = gen6.get("Normal").map(|r| r.keys().cloned().collect()).unwrap_or_default();
            let mut fairy: IndexMap<String, f64> = defenders.iter().map(|t| (t.clone(), 1.0)).collect();
            for (t, v) in [("Dragon", 2.0), ("Dark", 2.0), ("Fighting", 2.0), ("Fire", 0.5), ("Poison", 0.5), ("Steel", 0.5)] {
                fairy.insert(t.to_string(), v);
            }
            gen6.insert("Fairy".to_string(), fairy);
            for row in gen6.values_mut() {
                row.insert("Fairy".to_string(), 1.0);
            }
            for (atk, v) in [("Poison", 2.0), ("Steel", 2.0), ("Bug", 0.5), ("Dark", 0.5), ("Fighting", 0.5), ("Dragon", 0.0)] {
                set(&mut gen6, atk, "Fairy", v);
            }
            for g in 6..=9 {
                by_gen.insert(g, gen6.clone());
            }
        }
        TypeCharts { by_gen }
    }

    /// The chart of a generation (gen 4's for anything unknown).
    pub fn chart(&self, gen: u8) -> Option<&Chart> {
        self.by_gen.get(&gen).or_else(|| self.by_gen.get(&4))
    }
}

fn set(chart: &mut Chart, atk: &str, def: &str, v: f64) {
    if let Some(row) = chart.get_mut(atk) {
        row.insert(def.to_string(), v);
    }
}

/// Solodex's `GAME_TO_GEN[game] ?? '4'` default.
fn gen_or_4(game: &str) -> u8 {
    match games::game_gen(game) {
        0 => 4,
        g => g,
    }
}

/// Types that exist in the chart of a gen (gen 1 has no Dark / Steel rows in game).
fn valid_in_gen(gen: u8, t: &str) -> bool {
    gen != 1 || GEN1_TYPES.contains(&t)
}

fn introduced_gen(t: &str) -> u8 {
    match t {
        "Dark" | "Steel" => 2,
        "Fairy" => 6,
        _ => 1,
    }
}

/// Every type that exists in a game's generation, in chart order.
pub fn types_for_game(game: &str) -> Vec<String> {
    let gen = gen_or_4(game);
    store().charts().chart(gen).map(|c| c.keys().filter(|t| gen >= introduced_gen(t)).cloned().collect()).unwrap_or_default()
}

/// Multiplier of `attack` against one defending type.
pub fn offensive_multiplier(attack: &str, defending: &str, game: &str) -> f64 {
    store().charts().chart(gen_or_4(game)).and_then(|c| c.get(attack)).and_then(|r| r.get(defending)).copied().unwrap_or(1.0)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TypeMatchups {
    pub super_eff_vs: Vec<String>,
    pub not_eff_vs: Vec<String>,
    pub no_eff_vs: Vec<String>,
    pub weak_to: Vec<String>,
    pub resists: Vec<String>,
    pub immune_to: Vec<String>,
}

/// One type's offensive and defensive matchups (gen 4 chart without a game).
pub fn type_matchups(t: &str, game: Option<&str>) -> TypeMatchups {
    let gen = game.map(gen_or_4).unwrap_or(4);
    let mut m = TypeMatchups::default();
    let Some(chart) = store().charts().chart(gen) else { return m };
    if let Some(row) = chart.get(t) {
        for (d, v) in row {
            if !valid_in_gen(gen, d) {
                continue;
            }
            if *v == 2.0 {
                m.super_eff_vs.push(d.clone());
            } else if *v == 0.5 {
                m.not_eff_vs.push(d.clone());
            } else if *v == 0.0 {
                m.no_eff_vs.push(d.clone());
            }
        }
    }
    for (atk, row) in chart {
        if !valid_in_gen(gen, atk) {
            continue;
        }
        match row.get(t) {
            Some(v) if *v == 2.0 => m.weak_to.push(atk.clone()),
            Some(v) if *v == 0.5 => m.resists.push(atk.clone()),
            Some(v) if *v == 0.0 => m.immune_to.push(atk.clone()),
            _ => {}
        }
    }
    m
}

/// Combined defensive multiplier of a (dual) typing against every attacking
/// type, in chart order.
pub fn defense_matchups(type_1: &str, type_2: &str, game: &str) -> IndexMap<String, f64> {
    let gen = gen_or_4(game);
    let mut out = IndexMap::new();
    let Some(chart) = store().charts().chart(gen) else { return out };
    for (atk, row) in chart {
        if !valid_in_gen(gen, atk) {
            continue;
        }
        let v1 = row.get(type_1).copied().unwrap_or(1.0);
        let v2 = if type_1 != type_2 { row.get(type_2).copied().unwrap_or(1.0) } else { 1.0 };
        out.insert(atk.clone(), v1 * v2);
    }
    out
}

/// `ABILITY_IMMUNITIES` with the gen an immunity started applying.
pub fn ability_immunity_type(ability: &str, game: &str) -> Option<&'static str> {
    let (t, min_gen) = match ability {
        "Levitate" => ("Ground", 0),
        "Flash Fire" => ("Fire", 0),
        // Water Absorb existed in gens 3-4 but did not grant immunity until gen 5
        "Water Absorb" => ("Water", 5),
        "Dry Skin" | "Storm Drain" => ("Water", 0),
        "Volt Absorb" | "Motor Drive" | "Lightning Rod" => ("Electric", 0),
        "Sap Sipper" => ("Grass", 0),
        "Earth Eater" => ("Ground", 0),
        "Well-Baked Body" => ("Fire", 0),
        "Wind Rider" => ("Flying", 0),
        _ => return None,
    };
    if min_gen > 0 && games::game_gen(game) < min_gen {
        return None;
    }
    Some(t)
}

/// One row of the weakness table: multiplier, label, colours (Solodex `EFF_GROUPS`).
pub struct EffGroup {
    pub label: &'static str,
    pub value: f64,
    pub multiplier_label: &'static str,
    pub bg: &'static str,
    pub text: &'static str,
}

pub const EFF_GROUPS: [EffGroup; 5] = [
    EffGroup { label: "Weak", value: 4.0, multiplier_label: "\u{d7}4", bg: "#7f1d1d", text: "#fca5a5" },
    EffGroup { label: "Weak", value: 2.0, multiplier_label: "\u{d7}2", bg: "#451a03", text: "#fdba74" },
    EffGroup { label: "Resists", value: 0.5, multiplier_label: "\u{bd}\u{d7}", bg: "#14532d", text: "#86efac" },
    EffGroup { label: "Resists", value: 0.25, multiplier_label: "\u{bc}\u{d7}", bg: "#1e3a5f", text: "#93c5fd" },
    EffGroup { label: "Immune", value: 0.0, multiplier_label: "0\u{d7}", bg: "#1f2937", text: "#9ca3af" },
];
