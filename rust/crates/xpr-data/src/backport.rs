//! Port of `utils/backport_parser.py`: the ASM backport importer for custom
//! gens (Scott's Thoughts backport format).

use std::collections::HashMap;
use std::path::Path;

use indexmap::IndexMap;
use regex::{Regex, RegexBuilder};
use serde_json::{json, Value};

use xpr_core::consts;
use xpr_core::pyjson;

pub const BACKPORT_FORMAT_ERROR: &str = "ASM file must be in Scott's Thoughts backport format in order to automatically import.";

/// Crystal move tutor moves (not TMs/HMs, but taught by NPC tutor)
const CRYSTAL_TUTOR_MOVES: [&str; 3] = ["flamethrower", "thunderbolt", "icebeam"];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParsedPokemon {
    pub name: String,
    pub base_hp: i64,
    pub base_atk: i64,
    pub base_def: i64,
    pub base_spc_atk: i64,
    pub base_spc_def: i64,
    pub base_spd: i64,
    pub type_1: Option<String>,
    pub type_2: Option<String>,
    pub catch_rate: Option<i64>,
    pub base_xp: Option<i64>,
    pub common_item: Option<String>,
    pub rare_item: Option<String>,
    pub gender_ratio: Option<String>,
    pub egg_cycles: Option<i64>,
    pub growth_rate: Option<String>,
    pub egg_group_1: Option<String>,
    pub egg_group_2: Option<String>,
    pub tmhm_asm_names: Vec<String>,
    pub levelup_asm: Vec<(i64, String)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParsedMove {
    pub asm_name: String,
    pub effect: String,
    pub base_power: i64,
    pub move_type: String,
    pub accuracy: i64,
    pub pp: i64,
    pub effect_chance: i64,
    pub comment_name: Option<String>,
    pub name: String,
}

/// Python `str.title()`.
pub fn py_title(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut previous_is_cased = false;
    for c in s.chars() {
        let cased = c.is_lowercase() || c.is_uppercase();
        if cased {
            if previous_is_cased {
                out.extend(c.to_lowercase());
            } else {
                out.extend(c.to_uppercase());
            }
        } else {
            out.push(c);
        }
        previous_is_cased = cased;
    }
    out
}

/// `_normalize_name`: remove non-alphanumeric chars and lowercase.
pub fn normalize_name(name: &str) -> String {
    name.to_lowercase().chars().filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit()).collect()
}

fn build_move_lookup(moves_json_path: &Path) -> Result<IndexMap<String, String>, String> {
    let bytes = std::fs::read(moves_json_path).map_err(|e| format!("{}: {}", moves_json_path.display(), e))?;
    let data: Value = pyjson::loads(&pyjson::decode_text(&bytes)).map_err(|e| e.to_string())?;
    let mut lookup = IndexMap::new();
    if let Some(moves) = data.get("moves").and_then(|m| m.as_array()) {
        for mv in moves {
            let name = mv.get("name").and_then(|n| n.as_str()).ok_or("move without a name")?;
            lookup.insert(normalize_name(name), name.to_string());
        }
    }
    Ok(lookup)
}

/// `_resolve_move_name`
pub fn resolve_move_name(asm_name: &str, move_lookup: &IndexMap<String, String>, new_move_names: Option<&HashMap<String, String>>) -> String {
    let asm_name = asm_name.trim();
    if let Some(map) = new_move_names {
        if let Some(n) = map.get(asm_name) {
            return n.clone();
        }
    }
    let normalized = normalize_name(asm_name);
    if let Some(n) = move_lookup.get(&normalized) {
        return n.clone();
    }
    if let Some(stripped) = asm_name.strip_suffix("_M") {
        let normalized_no_m = normalize_name(stripped);
        if let Some(n) = move_lookup.get(&normalized_no_m) {
            return n.clone();
        }
    }
    let keys: Vec<&str> = move_lookup.keys().map(|k| k.as_str()).collect();
    if let Some(close) = get_close_match(&normalized, &keys, 0.8) {
        log::info!("Fuzzy matched ASM move '{}' to '{}'", asm_name, move_lookup[close]);
        return move_lookup[close].clone();
    }
    py_title(&asm_name.replace('_', " "))
}

fn macro_regex(macro_name: &str) -> Regex {
    RegexBuilder::new(&format!(r"MACRO\s+{}\b[^\n]*\n(.*?)^\s*ENDM\b", regex::escape(macro_name)))
        .dot_matches_new_line(true)
        .multi_line(true)
        .build()
        .expect("macro regex")
}

/// `_extract_macro`
fn extract_macro<'a>(content: &'a str, macro_name: &str) -> Option<&'a str> {
    macro_regex(macro_name).captures(content).map(|c| c.get(1).unwrap().as_str())
}

fn strip_comment(line: &str) -> String {
    match line.find(';') {
        Some(idx) => line[..idx].trim().to_string(),
        None => line.trim().to_string(),
    }
}

fn get_comment(line: &str) -> String {
    match line.find(';') {
        Some(idx) => line[idx + 1..].trim().to_string(),
        None => String::new(),
    }
}

fn strip_directive(line: &str, directive: &str) -> String {
    let re = RegexBuilder::new(&format!(r"^{}\s+", directive)).case_insensitive(true).build().unwrap();
    re.replace(line, "").trim().to_string()
}

fn py_int(s: &str) -> Result<i64, String> {
    s.trim().parse::<i64>().map_err(|_| format!("invalid literal for int() with base 10: '{}'", s))
}

/// `parse_backports_asm`
pub fn parse_backports_asm(content: &str) -> Result<ParsedPokemon, String> {
    for macro_name in ["backport_name", "backport_base_stats", "backport_level_up_learnset"] {
        let re = Regex::new(&format!(r"MACRO\s+{}\b", macro_name)).unwrap();
        if !re.is_match(content) {
            return Err(BACKPORT_FORMAT_ERROR.to_string());
        }
    }
    let mut result = ParsedPokemon::default();

    let name_section = extract_macro(content, "backport_name").ok_or(BACKPORT_FORMAT_ERROR)?;
    let name_re = Regex::new(r#"db\s+"([^"]+)""#).unwrap();
    let mut found_name = false;
    for line in name_section.split('\n') {
        if let Some(m) = name_re.captures(line) {
            if !line.trim().starts_with(';') {
                let raw_name = m.get(1).unwrap().as_str().replace('@', "");
                result.name = py_title(raw_name.trim());
                found_name = true;
                break;
            }
        }
    }
    if !found_name {
        return Err(BACKPORT_FORMAT_ERROR.to_string());
    }

    let stat_re = Regex::new(r"def\s+backport_(\w+)\s+EQU\s+(\d+)").unwrap();
    let mut stats: HashMap<String, i64> = HashMap::new();
    for m in stat_re.captures_iter(content) {
        let stat_name = m.get(1).unwrap().as_str().to_lowercase();
        let stat_value = py_int(m.get(2).unwrap().as_str())?;
        stats.insert(stat_name, stat_value);
    }
    result.base_hp = *stats.get("hit").unwrap_or(&0);
    result.base_atk = *stats.get("atk").unwrap_or(&0);
    result.base_def = *stats.get("def").unwrap_or(&0);
    result.base_spc_atk = *stats.get("spa").unwrap_or(&0);
    result.base_spc_def = *stats.get("spd").unwrap_or(&0);
    result.base_spd = *stats.get("spe").unwrap_or(&0);

    let base_stats_section = extract_macro(content, "backport_base_stats").ok_or(BACKPORT_FORMAT_ERROR)?;
    let mut db_lines: Vec<String> = Vec::new();
    let mut dn_lines: Vec<String> = Vec::new();
    let mut tmhm_line: Option<String> = None;
    for line in base_stats_section.split('\n') {
        let stripped = line.trim();
        if stripped.is_empty() || stripped.starts_with(';') {
            continue;
        }
        let clean = strip_comment(stripped);
        if clean.is_empty() {
            continue;
        }
        let clean_lower = clean.to_lowercase();
        if clean_lower.starts_with("tmhm") {
            tmhm_line = Some(clean);
        } else if clean_lower.starts_with("db ") || clean_lower.starts_with("db\t") {
            db_lines.push(clean);
        } else if clean_lower.starts_with("dn ") || clean_lower.starts_with("dn\t") {
            dn_lines.push(clean);
        }
    }

    if db_lines.len() >= 2 {
        let types_str = strip_directive(&db_lines[1], "db");
        let types: Vec<String> = types_str.split(',').map(|t| py_title(t.trim())).collect();
        let t1 = types.first().cloned().unwrap_or_else(|| "Normal".to_string());
        let t2 = types.get(1).cloned().unwrap_or_else(|| t1.clone());
        result.type_1 = Some(t1);
        result.type_2 = Some(t2);
    }
    if db_lines.len() >= 3 {
        result.catch_rate = Some(py_int(&strip_directive(&db_lines[2], "db"))?);
    }
    if db_lines.len() >= 4 {
        result.base_xp = Some(py_int(&strip_directive(&db_lines[3], "db"))?);
    }
    if db_lines.len() >= 5 {
        let items_str = strip_directive(&db_lines[4], "db");
        let items: Vec<String> = items_str.split(',').map(|i| i.trim().to_string()).collect();
        result.common_item = Some(items.first().cloned().unwrap_or_else(|| "NO_ITEM".to_string()));
        result.rare_item = Some(items.get(1).cloned().unwrap_or_else(|| "NO_ITEM".to_string()));
    }
    if db_lines.len() >= 6 {
        result.gender_ratio = Some(strip_directive(&db_lines[5], "db"));
    }
    if db_lines.len() >= 8 {
        result.egg_cycles = Some(py_int(&strip_directive(&db_lines[7], "db"))?);
    }
    for clean in &db_lines {
        let val = strip_directive(clean, "db");
        if val.to_uppercase().starts_with("GROWTH_") {
            result.growth_rate = Some(val.to_lowercase());
            break;
        }
    }
    if let Some(first) = dn_lines.first() {
        let eggs_str = strip_directive(first, "dn");
        let eggs: Vec<String> = eggs_str.split(',').map(|e| e.trim().to_string()).collect();
        result.egg_group_1 = Some(eggs.first().cloned().unwrap_or_else(|| "EGG_NONE".to_string()));
        result.egg_group_2 = Some(eggs.get(1).cloned().unwrap_or_else(|| "EGG_NONE".to_string()));
    }
    if let Some(t) = tmhm_line {
        let tmhm_str = strip_directive(&t, "tmhm");
        result.tmhm_asm_names = tmhm_str.split(',').map(|m| m.trim().to_string()).filter(|m| !m.is_empty()).collect();
    }

    let learnset_section = extract_macro(content, "backport_level_up_learnset").ok_or(BACKPORT_FORMAT_ERROR)?;
    for line in learnset_section.split('\n') {
        let stripped = line.trim();
        if stripped.is_empty() || stripped.starts_with(';') {
            continue;
        }
        let clean = strip_comment(stripped);
        if !clean.to_lowercase().starts_with("db") {
            continue;
        }
        let parts_str = strip_directive(&clean, "db");
        let parts: Vec<&str> = parts_str.split(',').collect();
        if parts.len() >= 2 {
            let level = py_int(parts[0])?;
            result.levelup_asm.push((level, parts[1].trim().to_string()));
        }
    }
    Ok(result)
}

/// `parse_backport_moves_asm`
pub fn parse_backport_moves_asm(content: &str) -> Result<Vec<ParsedMove>, String> {
    if !Regex::new(r"MACRO\s+backport_moves\b").unwrap().is_match(content) {
        return Err(BACKPORT_FORMAT_ERROR.to_string());
    }
    let mut moves: Vec<ParsedMove> = Vec::new();
    let mut move_names: Vec<String> = Vec::new();
    let move_re = Regex::new(r"^move\s+(\w+)\s*,\s*(\w+)\s*,\s*(\d+)\s*,\s*(\w+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)").unwrap();
    if let Some(section) = extract_macro(content, "backport_moves") {
        for line in section.split('\n') {
            let stripped = line.trim();
            if stripped.is_empty() || stripped.starts_with(';') {
                continue;
            }
            if let Some(m) = move_re.captures(stripped) {
                let comment = get_comment(stripped);
                moves.push(ParsedMove {
                    asm_name: m.get(1).unwrap().as_str().to_string(),
                    effect: m.get(2).unwrap().as_str().to_string(),
                    base_power: py_int(m.get(3).unwrap().as_str())?,
                    move_type: py_title(m.get(4).unwrap().as_str()),
                    accuracy: py_int(m.get(5).unwrap().as_str())?,
                    pp: py_int(m.get(6).unwrap().as_str())?,
                    effect_chance: py_int(m.get(7).unwrap().as_str())?,
                    comment_name: if comment.is_empty() { None } else { Some(comment) },
                    name: String::new(),
                });
            }
        }
    }
    let li_re = Regex::new(r#"li\s+"([^"]+)""#).unwrap();
    if let Some(section) = extract_macro(content, "backport_move_names") {
        for line in section.split('\n') {
            if let Some(m) = li_re.captures(line.trim()) {
                move_names.push(m.get(1).unwrap().as_str().to_string());
            }
        }
    }
    for (i, mv) in moves.iter_mut().enumerate() {
        mv.name = if let Some(c) = &mv.comment_name {
            py_title(c)
        } else if i < move_names.len() {
            py_title(&move_names[i])
        } else {
            py_title(&mv.asm_name.replace('_', " "))
        };
    }
    Ok(moves)
}

/// `get_backport_name`
pub fn get_backport_name(backport_asm_content: &str) -> Result<String, String> {
    Ok(parse_backports_asm(backport_asm_content)?.name)
}

/// `import_backport`: append the parsed species (and new moves) to the custom
/// gen's `pokemon.json` / `moves.json`. Returns the success message.
pub fn import_backport(custom_gen_path: &Path, backport_asm_content: &str, backport_moves_asm_content: Option<&str>, species_name: Option<&str>) -> Result<String, String> {
    let parsed_pokemon = parse_backports_asm(backport_asm_content)?;

    let mut new_moves: Vec<ParsedMove> = Vec::new();
    let mut new_move_name_map: HashMap<String, String> = HashMap::new();
    if let Some(content) = backport_moves_asm_content {
        if !content.is_empty() {
            new_moves = parse_backport_moves_asm(content)?;
            for mv in &new_moves {
                new_move_name_map.insert(mv.asm_name.clone(), mv.name.clone());
            }
        }
    }

    let moves_json_path = custom_gen_path.join(consts::MOVE_DB_FILE_NAME);
    let move_lookup = build_move_lookup(&moves_json_path)?;

    let mut tm_hm_learnset: Vec<String> = Vec::new();
    let mut tutor_learnset: Vec<String> = Vec::new();
    for asm_name in &parsed_pokemon.tmhm_asm_names {
        let resolved = resolve_move_name(asm_name, &move_lookup, Some(&new_move_name_map));
        let normalized = normalize_name(&resolved);
        if CRYSTAL_TUTOR_MOVES.contains(&normalized.as_str()) {
            tutor_learnset.push(resolved);
        } else {
            tm_hm_learnset.push(resolved);
        }
    }
    let levelup_moveset: Vec<Value> = parsed_pokemon
        .levelup_asm
        .iter()
        .map(|(level, asm_name)| json!([level, resolve_move_name(asm_name, &move_lookup, Some(&new_move_name_map))]))
        .collect();

    let pokemon_name = species_name.filter(|s| !s.is_empty()).map(|s| s.to_string()).unwrap_or_else(|| parsed_pokemon.name.clone());
    let pokemon_entry = pyjson::object(vec![
        ("name", json!(pokemon_name)),
        ("base_hp", json!(parsed_pokemon.base_hp)),
        ("base_atk", json!(parsed_pokemon.base_atk)),
        ("base_def", json!(parsed_pokemon.base_def)),
        ("base_spc_atk", json!(parsed_pokemon.base_spc_atk)),
        ("base_spc_def", json!(parsed_pokemon.base_spc_def)),
        ("base_spd", json!(parsed_pokemon.base_spd)),
        ("type_1", json!(parsed_pokemon.type_1.clone().unwrap_or_else(|| "Normal".into()))),
        ("type_2", json!(parsed_pokemon.type_2.clone().unwrap_or_else(|| "Normal".into()))),
        ("catch_rate", json!(parsed_pokemon.catch_rate.unwrap_or(0))),
        ("base_xp", json!(parsed_pokemon.base_xp.unwrap_or(0))),
        ("common_item", json!(parsed_pokemon.common_item.clone().unwrap_or_else(|| "NO_ITEM".into()))),
        ("rare_item", json!(parsed_pokemon.rare_item.clone().unwrap_or_else(|| "NO_ITEM".into()))),
        ("gender_ratio", json!(parsed_pokemon.gender_ratio.clone().unwrap_or_else(|| "GENDER_UNKNOWN".into()))),
        ("egg_cycles", json!(parsed_pokemon.egg_cycles.unwrap_or(0))),
        ("growth_rate", json!(parsed_pokemon.growth_rate.clone().unwrap_or_else(|| "growth_medium_fast".into()))),
        ("egg_group_1", json!(parsed_pokemon.egg_group_1.clone().unwrap_or_else(|| "EGG_NONE".into()))),
        ("egg_group_2", json!(parsed_pokemon.egg_group_2.clone().unwrap_or_else(|| "EGG_NONE".into()))),
        ("tm_hm_learnset", json!(tm_hm_learnset)),
        ("tutor_learnset", json!(tutor_learnset)),
        ("levelup_moveset", Value::Array(levelup_moveset)),
        ("egg_moves", json!([])),
    ]);

    let pokemon_json_path = custom_gen_path.join(consts::POKEMON_DB_FILE_NAME);
    let bytes = std::fs::read(&pokemon_json_path).map_err(|e| format!("{}: {}", pokemon_json_path.display(), e))?;
    let mut pokemon_data: Value = pyjson::loads(&pyjson::decode_text(&bytes)).map_err(|e| e.to_string())?;
    let mut pokemon_list = pokemon_data.get("pokemon").and_then(|p| p.as_array()).cloned().unwrap_or_default();
    pokemon_list.push(pokemon_entry);
    if let Value::Object(o) = &mut pokemon_data {
        o.insert("pokemon".to_string(), Value::Array(pokemon_list));
    } else {
        return Err("pokemon.json is not an object".to_string());
    }
    std::fs::write(&pokemon_json_path, pyjson::dump_indent4_platform_bytes(&pokemon_data)).map_err(|e| e.to_string())?;

    if !new_moves.is_empty() {
        let bytes = std::fs::read(&moves_json_path).map_err(|e| format!("{}: {}", moves_json_path.display(), e))?;
        let mut moves_data: Value = pyjson::loads(&pyjson::decode_text(&bytes)).map_err(|e| e.to_string())?;
        let mut moves_list = moves_data.get("moves").and_then(|m| m.as_array()).cloned().unwrap_or_default();
        let max_rom_id = moves_list.iter().map(|m| m.get("rom_id").and_then(|r| r.as_i64()).unwrap_or(0)).max().unwrap_or(0);
        for (i, mv) in new_moves.iter().enumerate() {
            moves_list.push(pyjson::object(vec![
                ("name", json!(mv.name)),
                ("accuracy", json!(mv.accuracy)),
                ("pp", json!(mv.pp)),
                ("base_power", json!(mv.base_power)),
                ("type", json!(mv.move_type)),
                ("attack_flavor", json!([])),
                ("effects", json!([])),
                ("rom_id", json!(max_rom_id + 1 + i as i64)),
            ]));
        }
        if let Value::Object(o) = &mut moves_data {
            o.insert("moves".to_string(), Value::Array(moves_list));
        } else {
            return Err("moves.json is not an object".to_string());
        }
        std::fs::write(&moves_json_path, pyjson::dump_indent4_platform_bytes(&moves_data)).map_err(|e| e.to_string())?;
    }

    let mut msg = format!("Successfully imported '{}'", pokemon_name);
    if !new_moves.is_empty() {
        msg.push_str(&format!(" with {} new move(s)", new_moves.len()));
    }
    Ok(msg)
}

// ---- difflib.get_close_matches(word, possibilities, n=1, cutoff) -----------------

/// `difflib.SequenceMatcher(None, a, b).ratio()` for short strings (no junk
/// heuristic: autojunk only applies to sequences of 200+ elements).
pub fn sequence_ratio(a: &str, b: &str) -> f64 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let matches: usize = matching_blocks(&a, &b).iter().map(|(_, _, n)| *n).sum();
    let total = a.len() + b.len();
    if total == 0 {
        1.0
    } else {
        2.0 * matches as f64 / total as f64
    }
}

fn find_longest_match(a: &[char], b: &[char], alo: usize, ahi: usize, blo: usize, bhi: usize) -> (usize, usize, usize) {
    // b2j: char -> positions in b
    let mut b2j: HashMap<char, Vec<usize>> = HashMap::new();
    for (j, &c) in b.iter().enumerate() {
        b2j.entry(c).or_default().push(j);
    }
    let (mut besti, mut bestj, mut bestsize) = (alo, blo, 0usize);
    let mut j2len: HashMap<usize, usize> = HashMap::new();
    for i in alo..ahi {
        let mut newj2len: HashMap<usize, usize> = HashMap::new();
        if let Some(positions) = b2j.get(&a[i]) {
            for &j in positions {
                if j < blo {
                    continue;
                }
                if j >= bhi {
                    break;
                }
                let k = j2len.get(&(j.wrapping_sub(1))).copied().unwrap_or(0) + 1;
                newj2len.insert(j, k);
                if k > bestsize {
                    besti = i + 1 - k;
                    bestj = j + 1 - k;
                    bestsize = k;
                }
            }
        }
        j2len = newj2len;
    }
    // (no junk elements, so the extension loops are no-ops)
    (besti, bestj, bestsize)
}

fn matching_blocks(a: &[char], b: &[char]) -> Vec<(usize, usize, usize)> {
    let mut queue = vec![(0usize, a.len(), 0usize, b.len())];
    let mut blocks: Vec<(usize, usize, usize)> = Vec::new();
    while let Some((alo, ahi, blo, bhi)) = queue.pop() {
        let (i, j, k) = find_longest_match(a, b, alo, ahi, blo, bhi);
        if k > 0 {
            blocks.push((i, j, k));
            if alo < i && blo < j {
                queue.push((alo, i, blo, j));
            }
            if i + k < ahi && j + k < bhi {
                queue.push((i + k, ahi, j + k, bhi));
            }
        }
    }
    blocks.sort();
    // merge adjacent blocks
    let mut merged: Vec<(usize, usize, usize)> = Vec::new();
    for (i, j, k) in blocks {
        if let Some(last) = merged.last_mut() {
            if last.0 + last.2 == i && last.1 + last.2 == j {
                last.2 += k;
                continue;
            }
        }
        merged.push((i, j, k));
    }
    merged
}

/// `difflib.get_close_matches(word, possibilities, n=1, cutoff)[0]`
pub fn get_close_match<'a>(word: &str, possibilities: &[&'a str], cutoff: f64) -> Option<&'a str> {
    let mut best: Option<(f64, usize)> = None;
    for (idx, p) in possibilities.iter().enumerate() {
        let ratio = sequence_ratio(word, p);
        if ratio >= cutoff {
            // heapq.nlargest keeps the highest score; ties keep the earlier element
            match best {
                Some((s, _)) if s >= ratio => {}
                _ => best = Some((ratio, idx)),
            }
        }
    }
    best.map(|(_, idx)| possibilities[idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_case_matches_python() {
        assert_eq!(py_title("pin's missle"), "Pin'S Missle");
        assert_eq!(py_title("HELLO world_foo"), "Hello World_Foo");
        assert_eq!(py_title("x2y"), "X2Y");
    }

    #[test]
    fn ratio_matches_python() {
        // difflib.SequenceMatcher(None, "pinmissle", "pinmissile").ratio() == 0.9473684210526315
        assert!((sequence_ratio("pinmissle", "pinmissile") - 0.9473684210526315).abs() < 1e-12);
        assert_eq!(sequence_ratio("abc", "xyz"), 0.0);
        assert_eq!(get_close_match("pinmissle", &["tackle", "pinmissile", "growl"], 0.8), Some("pinmissile"));
        assert_eq!(get_close_match("zzzz", &["tackle"], 0.8), None);
    }

    #[test]
    fn parses_a_minimal_backport() {
        let asm = r#"
MACRO backport_name
	db "SNEASEL@@@"
ENDM

def backport_hit EQU 55
def backport_atk EQU 95
def backport_def EQU 55
def backport_spa EQU 35
def backport_spd EQU 75
def backport_spe EQU 115

MACRO backport_base_stats
	db backport_hit, backport_atk, backport_def, backport_spe, backport_spa, backport_spd
	db DARK, ICE ; type
	db 60 ; catch rate
	db 132 ; base exp
	db NO_ITEM, NO_ITEM ; items
	db GENDER_F50 ; gender ratio
	db 100 ; unknown 1
	db 20 ; step cycles to hatch
	db 5 ; unknown 2
	INCBIN "gfx/pokemon/sneasel/front.dimensions"
	dw NULL, NULL ; unused (beta front/back pics)
	db GROWTH_MEDIUM_SLOW ; growth rate
	dn EGG_GROUND, EGG_GROUND ; egg groups
	tmhm TOXIC, ICE_BEAM, PIN_MISSLE, FLAMETHROWER
ENDM

MACRO backport_level_up_learnset
	db 1, SCRATCH
	db 1, LEER
	db 10, QUICK_ATTACK ; comment
ENDM
"#;
        let p = parse_backports_asm(asm).unwrap();
        assert_eq!(p.name, "Sneasel");
        assert_eq!((p.base_hp, p.base_atk, p.base_spd), (55, 95, 115));
        assert_eq!(p.type_1.as_deref(), Some("Dark"));
        assert_eq!(p.type_2.as_deref(), Some("Ice"));
        assert_eq!(p.catch_rate, Some(60));
        assert_eq!(p.base_xp, Some(132));
        assert_eq!(p.egg_cycles, Some(20));
        assert_eq!(p.growth_rate.as_deref(), Some("growth_medium_slow"));
        assert_eq!(p.egg_group_1.as_deref(), Some("EGG_GROUND"));
        assert_eq!(p.tmhm_asm_names, vec!["TOXIC", "ICE_BEAM", "PIN_MISSLE", "FLAMETHROWER"]);
        assert_eq!(p.levelup_asm, vec![(1, "SCRATCH".to_string()), (1, "LEER".to_string()), (10, "QUICK_ATTACK".to_string())]);
        assert!(parse_backports_asm("nothing here").is_err());
    }
}
