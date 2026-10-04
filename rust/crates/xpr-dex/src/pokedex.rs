//! Species lookups, evolution families and stat rankings (Solodex
//! `data/index.ts`: `getPokemonData`, `getAllPokemonForGame`, the ranking
//! functions, encounters).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use indexmap::IndexMap;

use crate::forms::{classify_form, is_mega_form};
use crate::model::{EncounterEntry, EvolutionEntry, PokemonData, PokemonListEntry, StatKey};
use crate::species::{display_name, internal_name, loose_key, species_alias};
use crate::store::{store, GameDex, RawDex};

/// Species that only evolve from a regional form: (species, region prefix,
/// base-form siblings of the other branch to drop).
const REGIONAL_EVO_LINEAGE: [(&str, &str, &[&str]); 9] = [
    ("Sirfetch'd", "Galarian", &[]),
    ("Perrserker", "Galarian", &["Persian"]),
    ("Obstagoon", "Galarian", &[]),
    ("Mr. Rime", "Galarian", &[]),
    ("Cursola", "Galarian", &[]),
    ("Runerigus", "Galarian", &["Cofagrigus"]),
    ("Sneasler", "Hisuian", &["Weavile"]),
    ("Overqwil", "Hisuian", &[]),
    ("Clodsire", "Paldean", &["Quagsire"]),
];

fn lineage(species: &str) -> Option<(&'static str, &'static [&'static str])> {
    REGIONAL_EVO_LINEAGE.iter().find(|(s, _, _)| *s == species).map(|(_, p, r)| (*p, *r))
}

fn regional_replaced(prefix: &str, species: &str) -> bool {
    REGIONAL_EVO_LINEAGE.iter().any(|(_, p, r)| *p == prefix && r.contains(&species))
}

fn remap_family_to_regional(family: &[EvolutionEntry], prefix: &str, dex: &RawDex) -> Vec<EvolutionEntry> {
    family
        .iter()
        .map(|evo| {
            let regional = format!("{} {}", prefix, evo.species);
            if dex.contains_key(&regional) {
                EvolutionEntry { species: regional, ..evo.clone() }
            } else {
                evo.clone()
            }
        })
        .collect()
}

/// Mega / Primal / (Mega Z) forms filed under the species they belong to; a
/// "Mega X ..." key under every word-prefix of X.
fn mega_index(gd: &GameDex) -> &HashMap<String, Vec<String>> {
    gd.mega_index.get_or_init(|| {
        let mut index: HashMap<String, Vec<String>> = HashMap::new();
        for key in gd.raw.keys() {
            if !is_mega_form(key) {
                continue;
            }
            let bases: Vec<String> = if let Some(rest) = key.strip_prefix("Mega ") {
                let words: Vec<&str> = rest.split(' ').collect();
                (0..words.len()).map(|i| words[..=i].join(" ")).collect()
            } else if let Some(rest) = key.strip_prefix("Primal ") {
                vec![rest.to_string()]
            } else if let Some(rest) = key.strip_suffix(" (Mega Z)") {
                vec![rest.to_string()]
            } else {
                continue;
            };
            for b in bases {
                index.entry(b).or_default().push(key.clone());
            }
        }
        index
    })
}

/// `getPokemonData` for one raw entry.
fn resolve(gd: &GameDex, name: &str, raw: &PokemonData) -> PokemonData {
    let dex = &gd.raw;
    let mut family = raw.evolution_family.clone();
    if !family.is_empty() {
        let form = classify_form(name);
        if let (true, Some(region)) = (form.is_regional, form.region) {
            let prefix = region.prefix();
            family = remap_family_to_regional(&family, prefix, dex);
            family.retain(|evo| {
                if let Some((p, _)) = lineage(&evo.species) {
                    if p != prefix {
                        return false;
                    }
                }
                if !evo.species.starts_with(&format!("{} ", prefix)) && dex.contains_key(&format!("{} {}", prefix, evo.species)) {
                    return false;
                }
                !regional_replaced(prefix, &evo.species)
            });
        } else if let Some((prefix, replaces)) = lineage(name) {
            family = remap_family_to_regional(&family, prefix, dex);
            family.retain(|evo| {
                evo.species == name || evo.species.starts_with(&format!("{} ", prefix)) || (!dex.contains_key(&format!("{} {}", prefix, evo.species)) && !replaces.contains(&evo.species.as_str()))
            });
        } else {
            family.retain(|evo| lineage(&evo.species).is_none() || evo.species == name);
        }
    }

    // fill in null evolution methods from other family members' data
    if family.len() > 1 {
        let snapshot = family.clone();
        for (i, evo) in family.iter_mut().enumerate() {
            if evo.method.is_some() {
                continue;
            }
            let base_species = snapshot
                .iter()
                .enumerate()
                .find(|(j, e)| e.method.is_none() && *j != i)
                .map(|(_, e)| e.species.clone())
                .unwrap_or_else(|| snapshot[0].species.clone());
            let found = dex
                .get(&base_species)
                .and_then(|b| b.evolution_family.iter().find(|e| e.species == evo.species && e.method.is_some()).cloned())
                .or_else(|| {
                    snapshot.iter().filter(|o| o.species != evo.species).find_map(|o| dex.get(&o.species).and_then(|d| d.evolution_family.iter().find(|e| e.species == evo.species && e.method.is_some()).cloned()))
                });
            if let Some(m) = found {
                evo.method = m.method;
                evo.parameter = m.parameter;
            }
        }
    }

    // append Mega / Primal forms of this game that are not in the family
    if !family.is_empty() {
        let names: Vec<String> = family.iter().map(|e| e.species.clone()).collect();
        let mut present: HashSet<String> = names.iter().cloned().collect();
        let index = mega_index(gd);
        let mut megas = Vec::new();
        for member in &names {
            if is_mega_form(member) {
                continue;
            }
            for key in index.get(member).map(|v| v.as_slice()).unwrap_or(&[]) {
                if !present.contains(key) {
                    present.insert(key.clone());
                    megas.push(EvolutionEntry { species: key.clone(), method: Some("mega".to_string()), parameter: None });
                }
            }
        }
        family.extend(megas);
    }

    let mut out = raw.clone();
    if gd.gen != 1 {
        out.transfer_learnset.clear();
    }
    out.evolution_family = family;
    out
}

pub(crate) fn resolved(gd: &GameDex) -> &IndexMap<String, Arc<PokemonData>> {
    gd.resolved.get_or_init(|| gd.raw.iter().map(|(k, v)| (k.clone(), Arc::new(resolve(gd, k, v)))).collect())
}

/// Full data for a species in a game (families remapped for regional forms,
/// missing evolution methods filled in, Mega forms appended).
pub fn get_pokemon_data(name: &str, game: &str) -> Option<Arc<PokemonData>> {
    let gd = store().game(game)?;
    resolved(&gd).get(name).cloned()
}

/// Cheap typing lookup (no family work).
pub fn get_pokemon_types(name: &str, game: &str) -> Option<(String, String)> {
    let gd = store().game(game)?;
    gd.raw.get(name).map(|d| (d.type_1.clone(), d.type_2.clone()))
}

/// Every species of a game, sorted by national dex number.
pub fn get_all_pokemon_for_game(game: &str) -> Vec<Arc<PokemonData>> {
    let Some(gd) = store().game(game) else { return Vec::new() };
    gd.by_dex
        .get_or_init(|| {
            let mut list: Vec<Arc<PokemonData>> = resolved(&gd).values().cloned().collect();
            list.sort_by_key(|p| p.national_dex_number);
            list
        })
        .clone()
}

/// The cross-game species list (available without loading any game).
pub fn get_all_pokemon() -> &'static [PokemonListEntry] {
    &store().species_index().entries
}

pub fn species_entry(name: &str) -> Option<&'static PokemonListEntry> {
    let idx = store().species_index();
    idx.by_name.get(name).map(|i| &idx.entries[*i])
}

/// Games (in `GAMES` order) whose Pokédex has the species.
pub fn get_games_for_pokemon(name: &str) -> Vec<String> {
    species_entry(name).map(|e| e.games.clone()).unwrap_or_default()
}

/// The Dex's key for a species name from elsewhere (the router's trainer
/// data, a typed search): exact, aliased, display-name and loose matches.
pub fn resolve_species(name: &str) -> Option<&'static str> {
    let idx = store().species_index();
    let try_exact = |n: &str| idx.by_name.get(n).map(|i| idx.entries[*i].name.as_str());
    if let Some(n) = try_exact(name).or_else(|| species_alias(name).and_then(try_exact)).or_else(|| try_exact(internal_name(name))) {
        return Some(n);
    }
    static LOOSE: std::sync::OnceLock<HashMap<String, usize>> = std::sync::OnceLock::new();
    let loose = LOOSE.get_or_init(|| {
        let mut m = HashMap::new();
        for (i, e) in idx.entries.iter().enumerate() {
            m.entry(loose_key(&e.name)).or_insert(i);
        }
        m
    });
    loose.get(&loose_key(name)).map(|i| idx.entries[*i].name.as_str())
}

/// Wild encounters of a species in a game (empty when the game has none).
pub fn get_encounters_for_pokemon(game: &str, species: &str) -> Vec<EncounterEntry> {
    let Some(table) = store().encounters(game) else { return Vec::new() };
    if let Some(v) = table.get(species) {
        return v.clone();
    }
    if let Some(v) = species_alias(species).and_then(|a| table.get(a)) {
        return v.clone();
    }
    let disp = display_name(species);
    if disp != species {
        if let Some(v) = table.get(disp) {
            return v.clone();
        }
    }
    Vec::new()
}

// ---- rankings -------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct StatRankEntry {
    pub name: String,
    pub dex: i32,
    pub value: i64,
    /// 1-based, ties share a rank ("competition" ranking)
    pub rank: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RankKind {
    Stat(StatKey),
    /// HP x Def
    PhysicalBulk,
    /// HP x SpD (HP x Special in gen 1)
    SpecialBulk,
    /// base stat total (gen 1: five stats)
    Total,
    /// gen 1 weighted total: Special counted twice
    Wbst,
    /// gen 1 useful total (WBST minus unusable offensive stats)
    Ubst,
}

fn build_ranking(mut entries: Vec<(String, i32, i64)>) -> Vec<StatRankEntry> {
    // stable: ties keep table order, like Array.prototype.sort
    entries.sort_by(|a, b| b.2.cmp(&a.2));
    let mut out = Vec::with_capacity(entries.len());
    let mut rank = 1;
    for (i, (name, dex, value)) in entries.into_iter().enumerate() {
        if i > 0 && value < out.last().map(|e: &StatRankEntry| e.value).unwrap_or(value) {
            rank = i + 1;
        }
        out.push(StatRankEntry { name, dex, value, rank });
    }
    out
}

/// BST as the Dex shows it: gen 1 counts Special once (five stats).
pub fn base_stat_total(data: &PokemonData, gen: u8) -> i64 {
    let s = &data.base_stats;
    if gen == 1 {
        (s.hp + s.attack + s.defense + s.special_attack + s.speed) as i64
    } else {
        s.sum() as i64
    }
}

fn rank_value(gd: &GameDex, kind: RankKind, data: &PokemonData) -> i64 {
    let s = &data.base_stats;
    let gen1 = gd.gen == 1;
    match kind {
        RankKind::Stat(k) => s.get(k) as i64,
        RankKind::PhysicalBulk => s.hp as i64 * s.defense as i64,
        RankKind::SpecialBulk => s.hp as i64 * if gen1 { s.special_attack } else { s.special_defense } as i64,
        RankKind::Total => base_stat_total(data, gd.gen),
        RankKind::Wbst => (s.hp + s.attack + s.defense + s.speed + s.special_attack * 2) as i64,
        RankKind::Ubst => ubst_of(data, gd.game),
    }
}

/// A ranking of every species of a game (or of `filter` only).
pub fn get_ranking(kind: RankKind, game: &str, filter: Option<&HashSet<String>>) -> Arc<Vec<StatRankEntry>> {
    let Some(gd) = store().game(game) else { return Arc::new(Vec::new()) };
    if filter.is_none() {
        if let Some(r) = gd.rankings.lock().unwrap().get(&kind) {
            return r.clone();
        }
    }
    let entries: Vec<(String, i32, i64)> =
        gd.raw.iter().filter(|(n, _)| filter.map(|f| f.contains(*n)).unwrap_or(true)).map(|(n, d)| (n.clone(), d.national_dex_number, rank_value(&gd, kind, d))).collect();
    let r = Arc::new(build_ranking(entries));
    if filter.is_none() {
        gd.rankings.lock().unwrap().insert(kind, r.clone());
    }
    r
}

/// Useful Base Stat Total (gen 1): WBST minus Attack when the species learns
/// no physical damaging move (level-up or TM/HM), minus one Special when it
/// learns no special one.
fn ubst_of(data: &PokemonData, game: &str) -> i64 {
    let s = &data.base_stats;
    let mut value = (s.hp + s.attack + s.defense + s.speed + s.special_attack * 2) as i64;
    let mut physical = false;
    let mut special = false;
    let mut seen = HashSet::new();
    for m in data.level_up_learnset.iter().map(|(_, m)| m).chain(data.tm_hm_learnset.iter()) {
        if !seen.insert(m.as_str()) {
            continue;
        }
        let Some(md) = crate::moves::get_move_data(m, game) else { continue };
        if md.power.map(|p| p <= 0).unwrap_or(true) {
            continue;
        }
        match md.category.as_str() {
            "Physical" => physical = true,
            "Special" => special = true,
            _ => {}
        }
        if physical && special {
            break;
        }
    }
    if !physical {
        value -= s.attack as i64;
    }
    if !special {
        value -= s.special_attack as i64;
    }
    value
}

pub fn get_pokemon_ubst(name: &str, game: &str) -> Option<i64> {
    let gd = store().game(game)?;
    gd.raw.get(name).map(|d| ubst_of(d, game))
}

// ---- learnsets --------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MoveSource {
    Level,
    Tm,
    Tutor,
    Egg,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LearnsetEntry {
    pub move_name: String,
    pub source: MoveSource,
    pub level: Option<i32>,
    pub tm_code: Option<String>,
}

/// A species' level-up, TM/HM, tutor and egg moves with their source.
pub fn get_learnset(species: &str, game: &str) -> Vec<LearnsetEntry> {
    let Some(data) = get_pokemon_data(species, game) else { return Vec::new() };
    let mut out = Vec::new();
    for (lv, m) in &data.level_up_learnset {
        out.push(LearnsetEntry { move_name: m.clone(), source: MoveSource::Level, level: Some(*lv), tm_code: None });
    }
    for m in &data.tm_hm_learnset {
        out.push(LearnsetEntry { move_name: m.clone(), source: MoveSource::Tm, level: None, tm_code: crate::tmhm::tmhm_code(m, game) });
    }
    for m in &data.tutor_learnset {
        out.push(LearnsetEntry { move_name: m.clone(), source: MoveSource::Tutor, level: None, tm_code: None });
    }
    for m in &data.egg_moves {
        out.push(LearnsetEntry { move_name: m.clone(), source: MoveSource::Egg, level: None, tm_code: None });
    }
    out
}

/// The moves a species knows at `level` from its level-up learnset alone
/// (each new move pushes the oldest out past four; -1 Move Reminder entries
/// never count).
pub fn default_moves_at_level(data: &PokemonData, level: i32) -> Vec<String> {
    let mut queue: Vec<String> = Vec::new();
    for (l, m) in &data.level_up_learnset {
        if *l > level || *l < 0 {
            continue;
        }
        if let Some(i) = queue.iter().position(|q| q == m) {
            queue.remove(i);
        }
        queue.push(m.clone());
        if queue.len() > 4 {
            queue.remove(0);
        }
    }
    queue
}
