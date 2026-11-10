//! Landing page (route library), loading / saving / sharing routes, and the
//! new-route form (game, solo Pokémon, base route, DVs/IVs, ability, nature).

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use xpr_core::consts;
use xpr_core::io_utils;
use xpr_data::model::{Nature, StatBlock};
use xpr_data::GenData;

use super::{b, i, opt_s, s, Session};

const GAME_ORDER: [&str; 20] = [
    consts::RED_VERSION,
    consts::BLUE_VERSION,
    consts::YELLOW_VERSION,
    consts::GOLD_VERSION,
    consts::SILVER_VERSION,
    consts::CRYSTAL_VERSION,
    consts::RUBY_VERSION,
    consts::SAPPHIRE_VERSION,
    consts::EMERALD_VERSION,
    consts::FIRE_RED_VERSION,
    consts::LEAF_GREEN_VERSION,
    consts::DIAMOND_VERSION,
    consts::PEARL_VERSION,
    consts::PLATINUM_VERSION,
    consts::HEART_GOLD_VERSION,
    consts::SOUL_SILVER_VERSION,
    consts::BLACK_VERSION,
    consts::WHITE_VERSION,
    consts::BLACK_2_VERSION,
    consts::WHITE_2_VERSION,
];

pub(crate) fn gen_num(g: xpr_data::model::Gen) -> u8 {
    use xpr_data::model::Gen;
    match g { Gen::One => 1, Gen::Two => 2, Gen::Three => 3, Gen::Four => 4, Gen::Five => 5 }
}

fn platform(version: &str) -> &'static str {
    match xpr_data::registry::gen_of_builtin(version).map(gen_num) {
        Some(1) => "Game Boy",
        Some(2) => "Game Boy Color",
        Some(3) => "Game Boy Advance",
        Some(_) => "Nintendo DS",
        None => "Custom",
    }
}

/// A route name usable as a file name (the desktop trusts the text field).
fn clean_route_name(name: &str) -> Result<String, String> {
    let n = name.trim().replace(['/', '\\', ':'], "-");
    if n.is_empty() {
        return Err("Please enter a route name".into());
    }
    if n.starts_with('.') {
        return Err("Route names can't start with a dot".into());
    }
    Ok(n)
}


/// `{hp, attack, defense, special_attack, special_defense, speed}` → StatBlock,
/// applying the gen 1-2 rules (HP DV from the others, one Special).
pub(crate) fn dvs_from_json(gen: &GenData, v: &Value) -> StatBlock {
    let g = |k: &str| v.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
    let (atk, def, spa, spe) = (g("attack"), g("defense"), g("special_attack"), g("speed"));
    if gen.get_generation() <= 2 {
        let hp = (atk % 2) * 8 + (def % 2) * 4 + (spe % 2) * 2 + (spa % 2);
        gen.make_stat_block(hp, atk, def, spa, spa, spe, false)
    } else {
        gen.make_stat_block(g("hp"), atk, def, spa, g("special_defense"), spe, false)
    }
}

pub(crate) fn dvs_to_json(sb: &StatBlock) -> Value {
    json!({"hp": sb.hp, "attack": sb.attack, "defense": sb.defense, "special_attack": sb.special_attack, "special_defense": sb.special_defense, "speed": sb.speed})
}

/// Everything the DV/IV form needs for one game and species.
pub(crate) fn dv_form(gen: &GenData, species: Option<&str>, dvs: Option<&StatBlock>, ability_idx: i64, nature: Nature) -> Value {
    let g = gen.get_generation();
    let max = if g <= 2 { 15 } else { 31 };
    let dvs = dvs.cloned().unwrap_or_else(|| gen.make_stat_block(max, max, max, max, max, max, false));
    let abilities: Vec<String> = species.and_then(|sp| gen.pkmn_db().get_pkmn(sp)).map(|m| m.abilities.clone()).unwrap_or_default();
    let natures: Vec<Value> = (0..25)
        .filter_map(Nature::from_index)
        .map(|n| {
            let mut up = Value::Null;
            let mut down = Value::Null;
            for st in ["attack", "defense", "speed", "special_attack", "special_defense"] {
                if n.is_stat_raised(st) {
                    up = json!(st);
                }
                if n.is_stat_lowered(st) {
                    down = json!(st);
                }
            }
            json!({"index": n.value(), "name": n.display_name(), "up": up, "down": down})
        })
        .collect();
    json!({
        "generation": g,
        "unit": if g <= 2 { "DV" } else { "IV" },
        "max": max,
        "derived_hp": g <= 2,
        "single_special": g <= 2,
        "dvs": dvs_to_json(&dvs),
        "abilities": abilities,
        "ability_idx": ability_idx,
        "show_ability": g >= 3,
        "show_nature": g >= 3,
        "nature": nature.value(),
        "natures": natures,
        "hidden_power": hidden_power(gen, &dvs),
    })
}

pub(crate) fn hidden_power(gen: &GenData, dvs: &StatBlock) -> Value {
    if gen.get_generation() == 1 {
        return Value::Null;
    }
    let (t, p) = gen.get_hidden_power(dvs);
    if t.is_empty() {
        Value::Null
    } else {
        json!({"type": t, "power": p})
    }
}

impl Session {
    pub(super) fn library_call(&mut self, method: &str, p: &Value) -> Result<Value, String> {
        match method {
            "library.list" => Ok(self.library_list(p)),
            "library.refresh" => {
                self.index.refresh(&self.paths);
                Ok(self.library_list(p))
            }
            "library.set_prefs" => {
                if let Some(v) = opt_s(p, "sort") {
                    self.cfg.set_landing_page_sort(&v);
                }
                if let Some(v) = opt_s(p, "game") {
                    self.cfg.set_landing_page_game_filter(&v);
                }
                if p.get("auto_load").is_some() {
                    self.cfg.set_auto_load_most_recent_route(b(p, "auto_load"));
                }
                Ok(json!(true))
            }
            "route.load" => {
                let path = self.route_path_param(p)?;
                if !path.exists() {
                    return Err(format!("No route file at {}", path.display()));
                }
                self.battle_for = None;
                self.expand.clear();
                self.ctrl.load_route(&path);
                self.ctrl.set_route_filter_types(Vec::new());
                self.ctrl.set_route_search("");
                Ok(self.app_state())
            }
            "route.load_most_recent" => {
                let Some(path) = self.index.most_recent(&self.paths) else { return Ok(json!(false)) };
                self.battle_for = None;
                self.ctrl.load_route(&path);
                Ok(json!(true))
            }
            "route.import" => {
                // a route file from Files / Mail / AirDrop: copy it into the
                // library (never overwriting) so it shows on the landing page
                let src = PathBuf::from(s(p, "path"));
                let raw = std::fs::read(&src).map_err(|e| format!("Could not read {}: {}", src.display(), e))?;
                let v: Value = serde_json::from_slice(&raw).map_err(|e| format!("Not a route file: {}", e))?;
                if v.get(consts::PKMN_VERSION_KEY).is_none() || v.get("events").is_none() {
                    return Err("That file isn't a route (no Version / events)".into());
                }
                let stem = src.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "imported".into());
                let dest = io_utils::get_safe_path_no_collision(&self.paths.saved_routes_dir, &stem, ".json");
                std::fs::write(&dest, &raw).map_err(|e| e.to_string())?;
                self.index.refresh(&self.paths);
                let name = dest.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                Ok(json!({"name": name}))
            }
            "route.save" => {
                let name = clean_route_name(s(p, "name"))?;
                self.ctrl.set_current_route_name(&name);
                self.ctrl.save_route(&name);
                Ok(self.app_state())
            }
            "route.set_name" => {
                self.ctrl.set_current_route_name(s(p, "name"));
                Ok(json!(true))
            }
            "route.close" => {
                self.battle_for = None;
                self.ctrl.close_route();
                Ok(self.app_state())
            }
            "route.new_from_current" => {
                self.battle_for = None;
                self.ctrl.create_new_route_from_current();
                Ok(self.app_state())
            }
            "route.delete" => {
                // moved to the backups folder rather than destroyed
                let path = self.paths.saved_routes_dir.join(format!("{}.json", s(p, "name")));
                if !path.exists() {
                    return Err("Route not found".into());
                }
                let dest = io_utils::get_safe_backup_path(&self.paths, &path).map_err(|e| e.to_string())?;
                std::fs::rename(&path, &dest).map_err(|e| e.to_string())?;
                self.index.refresh(&self.paths);
                Ok(json!(true))
            }
            "route.rename" => {
                let old = s(p, "name");
                let new = clean_route_name(s(p, "new_name"))?;
                let src = self.paths.saved_routes_dir.join(format!("{}.json", old));
                let dst = self.paths.saved_routes_dir.join(format!("{}.json", new));
                if dst.exists() {
                    return Err(format!("A route called {} already exists", new));
                }
                std::fs::rename(&src, &dst).map_err(|e| e.to_string())?;
                if self.ctrl.get_current_route_name() == old {
                    self.ctrl.set_current_route_name(&new);
                }
                self.index.refresh(&self.paths);
                Ok(json!(true))
            }
            "route.duplicate" => {
                let src = self.paths.saved_routes_dir.join(format!("{}.json", s(p, "name")));
                let dest = io_utils::get_safe_path_no_collision(&self.paths.saved_routes_dir, &format!("{} copy", s(p, "name")), ".json");
                std::fs::copy(&src, &dest).map_err(|e| e.to_string())?;
                self.index.refresh(&self.paths);
                Ok(json!(true))
            }
            "route.file" => {
                // a file to hand to the share sheet: the saved file, or the
                // open route (unsaved edits included) written to a temp file
                if let Some(name) = opt_s(p, "name") {
                    let path = self.paths.saved_routes_dir.join(format!("{}.json", name));
                    return Ok(json!({"path": path}));
                }
                let bytes = self.ctrl.router.save_bytes()?;
                let name = clean_route_name(self.ctrl.get_current_route_name()).unwrap_or_else(|_| "route".into());
                let dir = self.paths.global_config_dir.join("share");
                let _ = std::fs::remove_dir_all(&dir);
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let path = dir.join(format!("{}.json", name));
                std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
                Ok(json!({"path": path}))
            }
            "route.export_notes" => {
                let name = clean_route_name(self.ctrl.get_current_route_name())?;
                let text = self.ctrl.router.export_notes_text()?;
                self.ctrl.export_notes(&name);
                let path = self.paths.saved_routes_dir.join(format!("{}_notes.txt", name));
                Ok(json!({"path": path, "text": text}))
            }
            "route.dvs" => {
                let gen = self.gen()?;
                let init = self.ctrl.get_init_state().ok_or("no route")?;
                Ok(dv_form(&gen, Some(&init.solo_pkmn.name), self.ctrl.get_dvs().as_ref(), self.ctrl.get_ability_idx(), self.ctrl.get_nature()))
            }
            "route.set_dvs" => {
                let gen = self.gen()?;
                let dvs = dvs_from_json(&gen, p.get("dvs").unwrap_or(&Value::Null));
                let nature = Nature::from_index(i(p, "nature").unwrap_or(0)).unwrap_or_default();
                self.ctrl.customize_innate_stats(dvs, i(p, "ability_idx").unwrap_or(0), nature);
                Ok(self.app_state())
            }
            "newroute.games" => Ok(self.new_route_games()),
            "newroute.species" => {
                let gen = self.registry.get_version(s(p, "version"))?;
                let f = s(p, "filter");
                let names = if f.trim().is_empty() { gen.pkmn_db().get_all_names(None) } else { gen.pkmn_db().get_filtered_names(Some(f), None) };
                Ok(json!(names))
            }
            "newroute.bases" => {
                let version = s(p, "version");
                let gen = self.registry.get_version(version)?;
                let f = s(p, "filter").to_lowercase();
                let mut out = vec![consts::EMPTY_ROUTE_NAME.to_string()];
                for pr in &gen.min_battles_db().data {
                    out.push(format!("{}{}", consts::PRESET_ROUTE_PREFIX, pr));
                }
                let mut saved: Vec<String> = self.index.entries.values().filter(|e| e.version == version).map(|e| e.name.clone()).collect();
                saved.sort_by_key(|n| n.to_lowercase());
                out.extend(saved);
                if !f.is_empty() {
                    out.retain(|n| n == consts::EMPTY_ROUTE_NAME || n.to_lowercase().contains(&f));
                }
                Ok(json!(out))
            }
            "newroute.dv_form" => {
                let gen = self.registry.get_version(s(p, "version"))?;
                let dvs = p.get("dvs").filter(|v| v.is_object()).map(|v| dvs_from_json(&gen, v));
                let nature = Nature::from_index(i(p, "nature").unwrap_or(0)).unwrap_or_default();
                Ok(dv_form(&gen, opt_s(p, "species").as_deref(), dvs.as_ref(), i(p, "ability_idx").unwrap_or(0), nature))
            }
            "newroute.create" => {
                let version = s(p, "version").to_string();
                let gen = self.registry.get_version(&version)?;
                let species = s(p, "species").to_string();
                if gen.pkmn_db().get_pkmn(&species).is_none() {
                    return Err("Please pick a valid solo Pokémon".into());
                }
                let base = s(p, "base");
                let base_path: Option<PathBuf> = if base.is_empty() || base == consts::EMPTY_ROUTE_NAME {
                    None
                } else if let Some(preset) = base.strip_prefix(consts::PRESET_ROUTE_PREFIX) {
                    Some(gen.min_battles_db().get_dir().join(format!("{}.json", preset)))
                } else {
                    Some(io_utils::get_existing_route_path(&self.paths, base))
                };
                let dvs = dvs_from_json(&gen, p.get("dvs").unwrap_or(&Value::Null));
                let nature = Nature::from_index(i(p, "nature").unwrap_or(0)).unwrap_or_default();
                self.battle_for = None;
                self.expand.clear();
                self.ctrl.create_new_route(&species, base_path.as_deref(), &version, Some(dvs), Some(i(p, "ability_idx").unwrap_or(0)), Some(nature));
                if let Some(name) = opt_s(p, "name").filter(|n| !n.trim().is_empty()) {
                    self.ctrl.set_current_route_name(name.trim());
                }
                Ok(self.app_state())
            }
            _ => Err(format!("unknown method {}", method)),
        }
    }

    fn route_path_param(&self, p: &Value) -> Result<PathBuf, String> {
        if let Some(path) = opt_s(p, "path") {
            return Ok(PathBuf::from(path));
        }
        let name = s(p, "name");
        if name.is_empty() {
            return Err("No route given".into());
        }
        Ok(io_utils::get_existing_route_path(&self.paths, name))
    }

    fn library_list(&self, p: &Value) -> Value {
        let sort = opt_s(p, "sort").unwrap_or_else(|| self.cfg.get_landing_page_sort());
        let sort = if sort == "recent" { "most_recent".to_string() } else { sort };
        let game = opt_s(p, "game").unwrap_or_else(|| self.cfg.get_landing_page_game_filter());
        let search = s(p, "search").to_lowercase();
        let mut rows: Vec<&crate::route_index::IndexEntry> = self
            .index
            .entries
            .values()
            .filter(|e| search.is_empty() || e.name.to_lowercase().contains(&search) || e.version.to_lowercase().contains(&search) || e.species.to_lowercase().contains(&search))
            .filter(|e| sort != "game" || game == "All" || game == "All Games" || game.is_empty() || e.version == game)
            .collect();
        match sort.as_str() {
            "alphabetical" => rows.sort_by_key(|e| e.name.to_lowercase()),
            "game" => rows.sort_by(|a, b| a.version.cmp(&b.version).then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))),
            _ => rows.sort_by(|a, b| b.mtime.partial_cmp(&a.mtime).unwrap_or(std::cmp::Ordering::Equal)),
        }
        let routes: Vec<Value> = rows
            .iter()
            .map(|e| {
                let date = chrono::DateTime::from_timestamp(e.mtime as i64, 0)
                    .map(|d| d.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_default();
                json!({
                    "name": e.name, "version": e.version, "species": e.species, "mtime": e.mtime, "date": date,
                    "generation": self.registry.get_version(&e.version).ok().map(|g| g.get_generation()),
                })
            })
            .collect();
        let mut games = vec!["All Games".to_string()];
        games.extend(self.registry.get_gen_names(true, true));
        json!({
            "routes": routes,
            "sort": sort,
            "game": if game == "All" || game.is_empty() { "All Games".to_string() } else { game },
            "games": games,
            "auto_load": self.cfg.get_auto_load_most_recent_route(),
            "current": if self.ctrl.gen().is_some() { Some(self.ctrl.get_current_route_name()) } else { None },
        })
    }

    fn new_route_games(&self) -> Value {
        let customs = self.registry.get_all_custom_gen_info();
        let mut out: Vec<Value> = GAME_ORDER
            .iter()
            .map(|v| {
                let g = xpr_data::registry::gen_of_builtin(v).map(gen_num).unwrap_or(0);
                json!({"version": v, "generation": g, "platform": platform(v), "custom": false, "art": v})
            })
            .collect();
        let mut custom_rows: Vec<Value> = customs
            .iter()
            .map(|c| {
                let base = self.registry.get_version(&c.name).ok().and_then(|g| g.base_version_name().map(|s| s.to_string())).unwrap_or_else(|| c.base_version.clone());
                let g = self.registry.get_version(&c.name).map(|g| g.get_generation()).unwrap_or(0);
                json!({"version": c.name, "generation": g, "platform": "Custom", "custom": true, "art": base})
            })
            .collect();
        custom_rows.sort_by_key(|v| v["version"].as_str().unwrap_or("").to_lowercase());
        out.extend(custom_rows);
        Value::Array(out)
    }
}

#[allow(dead_code)]
fn _unused(_: &Path) {}
