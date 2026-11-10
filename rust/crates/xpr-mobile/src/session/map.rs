//! The world map: packs per game (embedded), the map list, 512 px chunks
//! rendered to PNG, markers with their cards, encounter tables, search,
//! "show on map" anchors, the trip path, and "add to route".

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use xpr_core::io_utils::sanitize_string;
use xpr_data::GenData;
use xpr_engine::NodeId;
use xpr_map::geom::{fit_rect, map_world_rect, object_center_px, pick_step, scope_rect, step_center_px};
use xpr_map::{game_for_version, Anchor, Compositor, LinkQuery, MapKind, MapPack, ObjectGrid, ObjectKind, PackSource, Payload, Precision, RenderOpts, Scope, SpriteCache};

use crate::controller::MainController;
use crate::map_state::RouteMapState;

use super::{b, i, s, strs, Session};

pub(crate) fn game_of_gen(gen: &GenData) -> Option<&'static str> {
    game_for_version(gen.base_version_name().unwrap_or(gen.version_name()))
}

pub(crate) fn query_for(ctrl: &MainController, id: NodeId) -> Option<LinkQuery> {
    crate::map_state::query_for_event(ctrl, id)
}

struct Loaded {
    has_imagery: bool,
    pack: Arc<MapPack>,
    comp: Compositor,
    grid: ObjectGrid,
    sprites: Mutex<SpriteCache>,
}

#[derive(Default)]
pub(crate) struct MapCache {
    games: HashMap<String, Arc<Loaded>>,
}

impl MapCache {
    pub(crate) fn on_version_change(&mut self) {
        // packs are per game, not per route: keep them (they're expensive)
        if self.games.len() > 2 {
            self.games.clear();
        }
    }
}

fn imagery_asset_name(game: &str, sha256: &str) -> String {
    format!("imagery_{}_{}.zip", game, &sha256[..sha256.len().min(16)])
}

fn kind_name(k: ObjectKind) -> &'static str {
    match k {
        ObjectKind::Trainer => "trainer",
        ObjectKind::Item => "item",
        ObjectKind::HiddenItem => "hidden",
        ObjectKind::Berry => "berry",
        ObjectKind::Sign => "sign",
        ObjectKind::Warp => "warp",
        ObjectKind::Npc => "npc",
        ObjectKind::Obstacle => "obstacle",
    }
}

fn scope_of(p: &Value) -> Scope {
    match i(p, "scope") {
        Some(m) if m >= 0 => Scope::Map(m as u16),
        _ => Scope::World,
    }
}

fn scope_json(s: Scope) -> i64 {
    match s {
        Scope::World => -1,
        Scope::Map(m) => m as i64,
    }
}

fn group_of(pack: &MapPack, id: u16) -> &'static str {
    let m = &pack.maps[id as usize];
    if let Some(cat) = m.category.as_deref() {
        return match (cat, m.kind) {
            ("city", MapKind::Outdoor) => "Cities",
            ("route", _) | (_, MapKind::Outdoor) => "Routes",
            _ => "Indoors & Dungeons",
        };
    }
    let c = m.const_name.as_str();
    let lower = m.display.to_lowercase();
    if c.contains("ROUTE") || lower.contains("route ") {
        return "Routes";
    }
    if m.kind == MapKind::Outdoor && (c.contains("_TOWN") || c.contains("_CITY") || lower.contains(" town") || lower.contains(" city") || lower.contains("island")) {
        return "Cities";
    }
    if m.kind == MapKind::Outdoor && pack.gen == 1 && (id as usize) < 11 {
        return "Cities";
    }
    "Indoors & Dungeons"
}

fn world_title(pack: &MapPack) -> String {
    match (pack.gen, pack.game.as_str()) {
        (1, _) => "Kanto",
        (2, _) | (_, "heartgold_soulsilver") => "Johto & Kanto",
        (_, "firered_leafgreen") => "Kanto & Sevii Islands",
        (4, _) => "Sinnoh",
        (5, _) => "Unova",
        _ => "Hoenn",
    }
    .to_string()
}

fn is_water_method(name: &str) -> bool {
    ["surf", "old_rod", "good_rod", "super_rod"].iter().any(|w| name.starts_with(w))
}

fn ev_yield_text(gen: &GenData, species: &str) -> Option<String> {
    if gen.get_generation() < 3 {
        return None;
    }
    let y = gen.pkmn_db().get_pkmn(species)?.stat_xp_yield.clone();
    let v = [y.hp, y.attack, y.defense, y.special_attack, y.special_defense, y.speed];
    let names = ["HP", "Atk", "Def", "SpA", "SpD", "Spe"];
    let parts: Vec<String> = (0..6).filter(|&k| v[k] != 0).map(|k| format!("{} {}", v[k], names[k])).collect();
    Some(if parts.is_empty() { "—".into() } else { parts.join(", ") })
}

fn party_ev_text(gen: &GenData, party: &[xpr_data::model::EnemyPkmn]) -> Option<String> {
    let mut sum = [0i64; 6];
    let mut known = false;
    for p in party {
        let Some(sp) = gen.pkmn_db().get_pkmn(&p.name) else { continue };
        let y = &sp.stat_xp_yield;
        for (acc, v) in sum.iter_mut().zip([y.hp, y.attack, y.defense, y.special_attack, y.special_defense, y.speed]) {
            *acc += v;
        }
        known = true;
    }
    if !known {
        return None;
    }
    let names = ["HP", "Atk", "Def", "SpA", "SpD", "Spe"];
    if gen.get_generation() < 3 {
        let parts: Vec<String> = [0usize, 1, 2, 3, 5].iter().map(|&k| format!("{} {}", sum[k], if k == 3 { "Spc" } else { names[k] })).collect();
        return Some(format!("Stat exp: {}", parts.join(", ")));
    }
    let parts: Vec<String> = (0..6).filter(|&k| sum[k] != 0).map(|k| format!("{} {}", sum[k], names[k])).collect();
    Some(if parts.is_empty() { "EVs: none".into() } else { format!("EVs: {}", parts.join(", ")) })
}

impl Session {
    fn map_game(&self, p: &Value) -> Result<String, String> {
        if let Some(v) = p.get("version").and_then(|v| v.as_str()) {
            let gen = self.registry.get_version(v)?;
            return game_of_gen(&gen).map(|g| g.to_string()).ok_or_else(|| "No map for this game".into());
        }
        let gen = self.gen()?;
        game_of_gen(&gen).map(|g| g.to_string()).ok_or_else(|| "No map for this game".into())
    }

    /// The gen data the map's cards use: the route's, or the browsed version's.
    fn map_gen(&self, p: &Value) -> Option<Arc<GenData>> {
        if let Some(v) = p.get("version").and_then(|v| v.as_str()) {
            return self.registry.get_version(v).ok();
        }
        self.ctrl.gen()
    }

    fn loaded(&mut self, game: &str) -> Result<Arc<Loaded>, String> {
        if let Some(l) = self.map.games.get(game) {
            return Ok(l.clone());
        }
        let pack = Arc::new(MapPack::load(game, &PackSource::new(None)).map_err(|e| format!("Could not load the {} map: {}", game, e))?);
        // gens 4/5: the downloaded pictures, when present (else a sketch)
        let imagery = self.imagery_file(&pack).filter(|p| p.is_file()).and_then(|p| xpr_map::imagery::ZipFiles::open(&p).ok()).map(|z| Arc::new(xpr_map::Imagery::new(Box::new(z))));
        let has_imagery = imagery.is_some();
        let comp = Compositor::with_imagery(pack.clone(), imagery);
        let grid = ObjectGrid::build(&pack);
        let l = Arc::new(Loaded { has_imagery, pack, comp, grid, sprites: Mutex::new(SpriteCache::default()) });
        self.map.games.insert(game.to_string(), l.clone());
        Ok(l)
    }

    /// Where a game's downloaded pictures live: the desktop's cache layout.
    fn imagery_file(&self, pack: &MapPack) -> Option<std::path::PathBuf> {
        let info = pack.image.as_ref()?.imagery.as_ref()?;
        Some(self.paths.global_config_dir.join("map_imagery").join(imagery_asset_name(&pack.game, &info.sha256)))
    }

    fn route_map_state(&self, pack: &MapPack, p: &Value) -> RouteMapState {
        let mut st = RouteMapState::default();
        if p.get("version").is_none() && self.ctrl.gen().is_some() {
            st.sync(&self.ctrl, Some(pack));
        } else if let Some(v) = p.get("version").and_then(|v| v.as_str()) {
            let g = self.registry.get_version(v).ok();
            st.version = g.map(|g| g.base_version_name().unwrap_or(g.version_name()).to_string());
        }
        st
    }

    pub(super) fn map_png(&mut self, method: &str, p: &Value) -> Result<Vec<u8>, String> {
        let game = self.map_game(p)?;
        let l = self.loaded(&game)?;
        let opts = RenderOpts { night: b(p, "night") && l.pack.gen == 2, mask: !p.get("mask").map(|v| v == &json!(false)).unwrap_or(false) };
        match method {
            "map.chunk" => {
                let scope = scope_of(p);
                let px = l.comp.render_chunk(scope, i(p, "level").unwrap_or(0).clamp(0, 12) as u8, i(p, "cx").unwrap_or(0) as u32, i(p, "cy").unwrap_or(0) as u32, opts);
                px.to_png()
            }
            "map.map_image" => {
                let id = i(p, "map").ok_or("map")? as u16;
                l.comp.render_scope(Scope::Map(id), opts).to_png()
            }
            "map.sprite" => {
                let idx = i(p, "object").ok_or("object")? as usize;
                let obj = l.pack.objects.get(idx).ok_or("object")?;
                let mut cache = l.sprites.lock().map_err(|_| "sprite cache")?;
                xpr_map::sprites::render_frame(&l.pack, &mut cache, obj, opts.night).ok_or("no sprite")?.to_png()
            }
            _ => Err("unknown".into()),
        }
    }

    pub(super) fn map_call(&mut self, method: &str, p: &Value) -> Result<Value, String> {
        match method {
            "map.games" => {
                // versions with a map, for the landing page's browser
                let names: Vec<Value> = self
                    .registry
                    .get_gen_names(true, false)
                    .into_iter()
                    .filter(|v| game_for_version(v).is_some())
                    .map(|v| json!({"version": v, "game": game_for_version(&v)}))
                    .collect();
                return Ok(json!(names));
            }
            "map.add_trainer" => {
                let id = self.ctrl.add_trainer_fight_from_map(s(p, "name"));
                if id.is_none() {
                    return Err(format!("Could not add {}", s(p, "name")));
                }
                self.reveal_opt(id);
                return Ok(json!({"id": id}));
            }
            "map.add_item" => {
                let id = self.ctrl.add_item_pickup_from_map(s(p, "name"));
                if id.is_none() {
                    return Err(format!("Could not add {}", s(p, "name")));
                }
                self.reveal_opt(id);
                return Ok(json!({"id": id}));
            }
            "map.add_wild" => {
                let id = self.ctrl.add_wild_from_map(s(p, "species"), i(p, "level").unwrap_or(5));
                if id.is_none() {
                    return Err(format!("Could not add {}", s(p, "species")));
                }
                self.reveal_opt(id);
                return Ok(json!({"id": id}));
            }
            "map.add_trainers" => {
                let names = strs(p, "names");
                let id = self.ctrl.add_trainers_in_new_folder(s(p, "folder"), &names);
                if id.is_none() {
                    return Err("Nothing left to add on this map".into());
                }
                self.reveal_opt(id);
                self.toast(format!("Added {} trainers from {}", names.len(), s(p, "folder")));
                return Ok(json!({"id": id}));
            }
            _ => {}
        }
        let game = self.map_game(p)?;
        let l = self.loaded(&game)?;
        let pack = &l.pack;
        let gen = self.map_gen(p);
        let st = self.route_map_state(pack, p);
        let version = st.version.clone();
        match method {
            "map.info" => {
                let world = scope_rect(pack, Scope::World);
                let maps: Vec<Value> = pack
                    .maps
                    .iter()
                    .filter(|m| pack.has_view(m.id))
                    .map(|m| {
                        let wr = map_world_rect(pack, m.id);
                        let sr = scope_rect(pack, Scope::Map(m.id));
                        let fr = fit_rect(pack, Scope::Map(m.id));
                        json!({
                            "id": m.id, "name": m.display, "const": m.const_name, "group": group_of(pack, m.id),
                            "outdoor": m.kind == MapKind::Outdoor,
                            "world": wr.map(|r| [r.x0, r.y0, r.x1, r.y1]),
                            "size": [sr.width(), sr.height()],
                            "fit": [fr.x0, fr.y0, fr.x1, fr.y1],
                        })
                    })
                    .collect();
                let max_side = world.width().max(world.height()).max(1) as f64;
                let wf = fit_rect(pack, Scope::World);
                let world_fit = [wf.x0, wf.y0, wf.x1, wf.y1];
                Ok(json!({
                    "game": game,
                    "title": world_title(pack),
                    "generation": pack.gen,
                    "world": [world.width(), world.height()],
                    "world_fit": world_fit,
                    "chunk": xpr_map::CHUNK_PX,
                    "max_level": (max_side / xpr_map::CHUNK_PX as f64).log2().ceil().max(0.0) as i64,
                    "night": pack.gen == 2,
                    "image_world": pack.is_image(),
                    "maps": maps,
                    "can_add": p.get("version").is_none() && self.ctrl.gen().is_some(),
                    "imagery": pack.image.as_ref().and_then(|img| img.imagery.as_ref()).map(|info| json!({
                        "installed": l.has_imagery,
                        "asset": imagery_asset_name(&pack.game, &info.sha256),
                        "bytes": info.bytes,
                    })),
                }))
            }
            "map.scope_size" => {
                let sc = scope_of(p);
                let r = scope_rect(pack, sc);
                let max_side = r.width().max(r.height()).max(1) as f64;
                let f = fit_rect(pack, sc);
                Ok(json!({"size": [r.width(), r.height()], "fit": [f.x0, f.y0, f.x1, f.y1], "max_level": (max_side / xpr_map::CHUNK_PX as f64).log2().ceil().max(0.0) as i64}))
            }
            "map.objects" => {
                let sc = scope_of(p);
                let kinds = strs(p, "kinds");
                let mut out = Vec::new();
                for (idx, o) in pack.objects.iter().enumerate() {
                    if !o.in_version(version.as_deref()) {
                        continue;
                    }
                    let k = kind_name(o.effective_kind());
                    if !kinds.is_empty() && !kinds.iter().any(|x| x == k) {
                        continue;
                    }
                    let Some((x, y)) = object_center_px(pack, sc, idx as u32) else { continue };
                    let routed = match o.effective_kind() {
                        ObjectKind::Trainer => o.trainer_names().iter().any(|n| st.is_defeated(n)),
                        ObjectKind::Item | ObjectKind::HiddenItem | ObjectKind::Berry => o.item_name().map(|n| st.is_acquired(n)).unwrap_or(false),
                        _ => false,
                    };
                    out.push(json!([idx, k, x, y, routed, o.sprite.is_some()]));
                }
                Ok(json!(out))
            }
            "map.hit" => {
                // what a tap at a world point lands on: a marker, a grass /
                // water tile, or the map itself
                let sc = scope_of(p);
                let (wx, wy) = (i(p, "x").unwrap_or(0) as i32, i(p, "y").unwrap_or(0) as i32);
                let kinds = strs(p, "kinds");
                if let Some(idx) = l.grid.hit(pack, sc, wx, wy, |o| o.in_version(version.as_deref()) && (kinds.is_empty() || kinds.iter().any(|x| x == kind_name(o.effective_kind())))) {
                    return Ok(json!({"object": idx}));
                }
                let picked = if pack.is_image() {
                    pick_step(pack, sc, wx, wy).map(|(map, x, y)| {
                        let (g, w) = pack.terrain_at(map, x.max(0) as u32, y.max(0) as u32);
                        (map, x, y, g, w)
                    })
                } else {
                    xpr_map::geom::map_at_world_px(pack, sc, wx, wy).map(|(map, lx, ly)| {
                        let bp = pack.geom.block_px as i32;
                        let (g, w) = pack.terrain_at(map, (lx / bp).max(0) as u32, (ly / bp).max(0) as u32);
                        let s = pack.geom.step_px as i32;
                        (map, lx / s, ly / s, g, w)
                    })
                };
                if let Some((map, x, y, grass, water)) = picked {
                    return Ok(json!({"map": map, "x": x, "y": y, "grass": grass, "water": water}));
                }
                Ok(Value::Null)
            }
            "map.object" => {
                let idx = i(p, "object").ok_or("object")? as usize;
                let o = pack.objects.get(idx).ok_or("no such object")?;
                let map_name = pack.map(o.map).map(|m| m.display.clone()).unwrap_or_default();
                let mut v = json!({"object": idx, "kind": kind_name(o.effective_kind()), "map": o.map, "map_name": map_name});
                match &o.payload {
                    Payload::Trainer { .. } | Payload::Npc { .. } if o.effective_kind() == ObjectKind::Trainer => {
                        let names = o.trainer_names();
                        let trainers: Vec<Value> = names
                            .iter()
                            .map(|n| match gen.as_ref() {
                                Some(g) if g.trainer_db().get_trainer(n).is_some() => {
                                    let mut t = self.trainer_json(g, n);
                                    t["defeated"] = json!(st.is_defeated(n));
                                    t["ev_text"] = json!(g.trainer_db().get_trainer(n).and_then(|t| party_ev_text(g, &t.pkmn)));
                                    t
                                }
                                _ => json!({"name": n, "unknown": true}),
                            })
                            .collect();
                        v["trainers"] = json!(trainers);
                        if let Payload::Trainer { raw, double, .. } = &o.payload {
                            v["raw"] = json!(raw);
                            v["double"] = json!(double);
                        }
                    }
                    Payload::Item { item, raw, fake, count } => {
                        v["item"] = json!(item);
                        v["raw"] = json!(raw);
                        v["fake"] = json!(fake);
                        v["count"] = json!(count);
                        v["acquired"] = json!(item.as_deref().map(|n| st.is_acquired(n)).unwrap_or(false));
                        v["known"] = json!(item.as_deref().and_then(|n| gen.as_ref().and_then(|g| g.item_db().get_item(n))).is_some());
                    }
                    Payload::Sign { text_key } => {
                        v["text"] = json!(pack.sign_text.get(text_key));
                    }
                    Payload::Warp { dest_map, dynamic, candidates, dest_warp } => {
                        let resolve = |c: &str| pack.map_by_const(c).map(|m| json!({"id": m.id, "name": m.display}));
                        v["dynamic"] = json!(dynamic);
                        v["dest_warp"] = json!(dest_warp);
                        v["destinations"] = if *dynamic { json!(candidates.iter().filter_map(|c| resolve(c)).collect::<Vec<_>>()) } else { json!(dest_map.as_deref().and_then(resolve).into_iter().collect::<Vec<_>>()) };
                    }
                    Payload::Npc { label, text_key, .. } => {
                        v["label"] = json!(label);
                        v["text"] = json!(text_key.as_ref().and_then(|k| pack.sign_text.get(k)));
                    }
                    Payload::Obstacle { obstacle, label } => {
                        v["label"] = json!(label);
                        v["needs"] = json!(match obstacle.as_str() { "cut" => "Cut", "rock_smash" => "Rock Smash", "strength" => "Strength", o => o });
                    }
                    _ => {}
                }
                Ok(v)
            }
            "map.install_imagery" => {
                // a downloaded zip: check it, keep it, reload the pictures
                let info = pack.image.as_ref().and_then(|img| img.imagery.clone()).ok_or("this map has no pictures to install")?;
                let src = std::path::PathBuf::from(s(p, "path"));
                xpr_map::imagery::verify_zip(&src, &info)?;
                let dest = self.imagery_file(pack).ok_or("no imagery")?;
                std::fs::create_dir_all(dest.parent().unwrap()).map_err(|e| e.to_string())?;
                if let Ok(rd) = std::fs::read_dir(dest.parent().unwrap()) {
                    for e in rd.flatten() {
                        let n = e.file_name().to_string_lossy().to_string();
                        if n.starts_with(&format!("imagery_{}_", pack.game)) {
                            let _ = std::fs::remove_file(e.path());
                        }
                    }
                }
                if std::fs::rename(&src, &dest).is_err() {
                    std::fs::copy(&src, &dest).map_err(|e| e.to_string())?;
                    let _ = std::fs::remove_file(&src);
                }
                self.map.games.remove(&game);
                Ok(json!(true))
            }
            "map.encounters" => {
                let map = i(p, "map").ok_or("map")? as u16;
                let terrain = s(p, "terrain"); // "", "grass", "water"
                let Some(table) = pack.encounters.get(&map) else { return Ok(json!({"sections": []})) };
                let ver = version.clone().unwrap_or_default();
                let mut titles: Vec<(String, String, Option<i64>, Option<String>)> = Vec::new();
                let relevant: Vec<&xpr_map::EncounterMethod> = table.methods.iter().filter(|m| terrain.is_empty() || is_water_method(&m.name) == (terrain == "water")).collect();
                let mut conditions: Vec<(String, String)> = Vec::new();
                match &pack.image {
                    Some(img) if !img.encounter_methods.is_empty() => {
                        for m in &relevant {
                            let base = img.encounter_methods.iter().filter(|(b, _)| m.name == *b || m.name.starts_with(&format!("{}_", b))).max_by_key(|(b, _)| b.len());
                            let Some((base, label)) = base else { continue };
                            let cond = (m.name.len() > base.len()).then(|| m.name[base.len() + 1..].to_string());
                            let cond_label = cond.as_ref().and_then(|c| img.encounter_conditions.iter().find(|(k, _)| k == c).map(|(_, l)| l.clone()));
                            if let (Some(c), Some(cl)) = (&cond, &cond_label) {
                                if !conditions.iter().any(|(k, _)| k == c) {
                                    conditions.push((c.clone(), cl.clone()));
                                }
                            }
                            titles.push((m.name.clone(), label.clone(), m.base_rate, cond));
                        }
                    }
                    _ => {
                        for m in &relevant {
                            titles.push((m.name.clone(), m.name.replace('_', " ").to_uppercase(), m.base_rate, None));
                        }
                    }
                }
                let sections: Vec<Value> = titles
                    .into_iter()
                    .filter_map(|(name, title, rate, cond)| {
                        let slots = table.slots_for(&name, &ver).filter(|x| !x.is_empty())?;
                        let rows: Vec<Value> = slots
                            .iter()
                            .map(|sl| {
                                json!({
                                    "species": sl.species,
                                    "level": if sl.min_level == sl.max_level { sl.min_level.to_string() } else { format!("{}-{}", sl.min_level, sl.max_level) },
                                    "min_level": sl.min_level,
                                    "rate": if (sl.rate - sl.rate.round()).abs() < 0.05 { format!("{}%", sl.rate.round() as i64) } else { format!("{:.1}%", sl.rate) },
                                    "ev": gen.as_ref().and_then(|g| ev_yield_text(g, &sl.species)),
                                })
                            })
                            .collect();
                        Some(json!({"method": name, "title": title, "rate": rate.filter(|r| *r > 0), "condition": cond, "rows": rows}))
                    })
                    .collect();
                Ok(json!({"sections": sections, "conditions": conditions, "map_name": pack.map(map).map(|m| m.display.clone())}))
            }
            "map.map_card" => {
                let map = i(p, "map").ok_or("map")? as u16;
                let m = pack.map(map).ok_or("map")?;
                let base = pack.object_range(map).start;
                let mut trainers: Vec<Value> = Vec::new();
                let mut to_add: Vec<String> = Vec::new();
                let mut items = 0;
                for (k, o) in pack.objects_of(map).iter().enumerate() {
                    if !o.in_version(version.as_deref()) {
                        continue;
                    }
                    match o.effective_kind() {
                        ObjectKind::Trainer => {
                            let names = o.trainer_names();
                            let Some(first) = names.first() else { continue };
                            let defeated = names.iter().any(|n| st.is_defeated(n));
                            if !defeated && !to_add.contains(first) {
                                to_add.push(first.clone());
                            }
                            let t = gen.as_ref().and_then(|g| g.trainer_db().get_trainer(first).cloned());
                            trainers.push(json!({
                                "object": base as usize + k, "name": first, "variants": names.len(), "defeated": defeated,
                                "money": t.as_ref().map(|t| t.money), "exp": t.as_ref().map(|t| t.pkmn.iter().map(|m| m.xp).sum::<i64>()),
                                "double": t.as_ref().map(|t| t.double_battle),
                                "party": t.as_ref().map(|t| t.pkmn.iter().map(|m| json!({"name": m.name, "level": m.level})).collect::<Vec<_>>()),
                                "ev_text": t.as_ref().and_then(|t| gen.as_ref().and_then(|g| party_ev_text(g, &t.pkmn))),
                            }));
                        }
                        ObjectKind::Item | ObjectKind::HiddenItem | ObjectKind::Berry if o.item_name().is_some() => items += 1,
                        _ => {}
                    }
                }
                let remaining = to_add.len();
                Ok(json!({"map": map, "name": m.display, "trainers": trainers, "items": items, "remaining": remaining, "to_add": to_add, "has_encounters": pack.encounters.contains_key(&map)}))
            }
            "map.focus" => {
                // "show on map": where the selected event happens
                let id = i(p, "id").ok_or("id")?;
                let q = query_for(&self.ctrl, id).ok_or("This event has no map location")?;
                let anchors = pack.resolve(&q);
                if anchors.is_empty() {
                    return Err("The map has no position for this event".into());
                }
                Ok(json!(anchors.iter().map(|a| self.anchor_json(pack, a)).collect::<Vec<_>>()))
            }
            "map.trip" => {
                let Some(trip) = st.trip.as_ref() else {
                    return Ok(json!({"folder": st.trip_folder_name, "nodes": []}));
                };
                let nodes: Vec<Value> = trip
                    .nodes
                    .iter()
                    .map(|n| {
                        let mut a = self.anchor_json(pack, &n.anchor);
                        a["ids"] = json!(n.ids);
                        a["label"] = json!(if n.first == n.last { n.first.to_string() } else { format!("{}–{}", n.first, n.last) });
                        a
                    })
                    .collect();
                Ok(json!({"folder": trip.name, "nodes": nodes}))
            }
            "map.search" => {
                let q = s(p, "query").to_lowercase();
                if q.is_empty() {
                    return Ok(json!({"maps": [], "trainers": [], "items": []}));
                }
                let maps: Vec<Value> = pack
                    .maps
                    .iter()
                    .filter(|m| pack.has_view(m.id) && (m.display.to_lowercase().contains(&q) || m.const_name.to_lowercase().contains(&q)))
                    .take(40)
                    .map(|m| json!({"id": m.id, "name": m.display, "group": group_of(pack, m.id)}))
                    .collect();
                let mut trainers = Vec::new();
                let mut items = Vec::new();
                if q.len() >= 2 {
                    if let Some(g) = gen.as_ref() {
                        for t in g.trainer_db().iter() {
                            if trainers.len() >= 25 {
                                break;
                            }
                            if t.name.to_lowercase().contains(&q) {
                                let anchors = pack.resolve(&LinkQuery::Trainer(t.name.clone()));
                                if !anchors.is_empty() {
                                    trainers.push(json!({"name": t.name, "anchors": anchors.iter().map(|a| self.anchor_json(pack, a)).collect::<Vec<_>>()}));
                                }
                            }
                        }
                    }
                    // items straight from the pack's objects
                    let mut by_name: Vec<(String, Vec<Value>, usize, usize)> = Vec::new();
                    for (idx, o) in pack.objects.iter().enumerate() {
                        let Payload::Item { item: Some(name), fake: false, .. } = &o.payload else { continue };
                        if !name.to_lowercase().contains(&q) || !o.in_version(version.as_deref()) {
                            continue;
                        }
                        let a = Anchor { map: o.map, x: o.x, y: o.y, precision: Precision::Object, object: Some(idx as u32), source: String::new() };
                        let hidden = o.kind == ObjectKind::HiddenItem;
                        match by_name.iter_mut().find(|e| e.0 == *name) {
                            Some(e) => {
                                e.1.push(self.anchor_json(pack, &a));
                                if hidden { e.3 += 1 } else { e.2 += 1 }
                            }
                            None => by_name.push((name.clone(), vec![self.anchor_json(pack, &a)], usize::from(!hidden), usize::from(hidden))),
                        }
                    }
                    by_name.sort_by_key(|e| e.0.to_lowercase());
                    for (name, anchors, balls, hidden) in by_name.into_iter().take(25) {
                        items.push(json!({"name": name, "anchors": anchors, "balls": balls, "hidden": hidden}));
                    }
                }
                Ok(json!({"maps": maps, "trainers": trainers, "items": items}))
            }
            "map.anchor_of_map" => {
                let map = i(p, "map").ok_or("map")? as u16;
                let m = pack.map(map).ok_or("map")?;
                let a = Anchor { map, x: (m.w * (pack.geom.block_px / pack.geom.step_px).max(1)) as u16 / 2, y: (m.h * (pack.geom.block_px / pack.geom.step_px).max(1)) as u16 / 2, precision: Precision::Map, object: None, source: String::new() };
                Ok(self.anchor_json(pack, &a))
            }
            _ => Err(format!("unknown method {}", method)),
        }
    }

    /// An anchor in the scope it should be viewed in (the world for outdoor
    /// maps, the map itself for indoors) with its pixel position there.
    fn anchor_json(&self, pack: &MapPack, a: &Anchor) -> Value {
        let outdoor = pack.map(a.map).map(|m| m.kind == MapKind::Outdoor && m.world_pos.is_some()).unwrap_or(false);
        let scope = if outdoor { Scope::World } else { Scope::Map(a.map) };
        let pos = match a.object {
            Some(o) => object_center_px(pack, scope, o),
            None => step_center_px(pack, scope, a.map, a.x as i32, a.y as i32),
        };
        let map_rect = if outdoor { map_world_rect(pack, a.map) } else { Some(scope_rect(pack, scope)) };
        json!({
            "scope": scope_json(scope), "map": a.map, "map_name": pack.map(a.map).map(|m| m.display.clone()),
            "x": pos.map(|p| p.0), "y": pos.map(|p| p.1), "object": a.object,
            "precision": match a.precision { Precision::Object => "object", Precision::Script => "script", Precision::Map => "map" },
            "rect": map_rect.map(|r| [r.x0, r.y0, r.x1, r.y1]),
        })
    }

    fn reveal_opt(&mut self, id: Option<NodeId>) {
        if let Some(id) = id {
            self.reveal(id);
        }
    }
}

#[allow(dead_code)]
fn _sanitize(s: &str) -> String {
    sanitize_string(s)
}
