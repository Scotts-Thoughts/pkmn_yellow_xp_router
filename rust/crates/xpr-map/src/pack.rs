//! Loading a game's map pack (embedded or from `map_data/<game>/`) into the
//! processed `MapPack`.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::{Path, PathBuf};

use xpr_core::consts;
use xpr_core::io_utils::sanitize_string;

use crate::geom::IRect;
use crate::links::Links;
use crate::model::*;

pub const PACK_FORMAT: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum MapError {
    #[error("no map data for game {0}")]
    NoGame(String),
    #[error("map data file {0} is missing (this build has no embedded map data and no map_data directory)")]
    Missing(String),
    #[error("{0}: {1}")]
    Parse(String, String),
    #[error("map pack format {0} is not supported (expected {PACK_FORMAT})")]
    Format(u32),
    #[error("{0}")]
    Other(String),
}

/// Where packs come from: the embedded table first, then a directory.
#[derive(Debug, Clone, Default)]
pub struct PackSource {
    pub dir: Option<PathBuf>,
}

impl PackSource {
    pub fn new(dir: Option<PathBuf>) -> PackSource {
        let dir = std::env::var_os("XPR_MAP_DATA_DIR").map(PathBuf::from).or(dir);
        PackSource { dir }
    }

    pub fn read(&self, game: &str, rel: &str) -> Result<Vec<u8>, MapError> {
        let key = format!("{}/{}", game, rel);
        if let Some(b) = crate::embedded::lookup(&key) {
            return Ok(b);
        }
        if let Some(dir) = &self.dir {
            let p = dir.join(game).join(rel);
            if p.exists() {
                return std::fs::read(&p).map_err(|e| MapError::Other(format!("{}: {}", p.display(), e)));
            }
        }
        Err(MapError::Missing(key))
    }

    pub fn has_game(&self, game: &str) -> bool {
        crate::embedded::contains(&format!("{}/manifest.json", game))
            || self.dir.as_ref().map(|d| d.join(game).join("manifest.json").exists()).unwrap_or(false)
    }

    /// Files of `<game>/<sub>/` (embedded or on disk).
    pub fn list(&self, game: &str, sub: &str) -> Vec<String> {
        let mut names = crate::embedded::list_dir(&format!("{}/{}", game, sub));
        if names.is_empty() {
            if let Some(dir) = &self.dir {
                if let Ok(rd) = std::fs::read_dir(dir.join(game).join(sub)) {
                    names = rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
                    names.sort();
                }
            }
        }
        names
    }
}

/// The pokemap game id of a router version (Red and Blue share `red_blue`, ...).
pub fn game_for_version(version: &str) -> Option<&'static str> {
    Some(match version {
        consts::RED_VERSION | consts::BLUE_VERSION => "red_blue",
        consts::YELLOW_VERSION => "yellow",
        consts::GOLD_VERSION | consts::SILVER_VERSION => "gold_silver",
        consts::CRYSTAL_VERSION => "crystal",
        consts::RUBY_VERSION | consts::SAPPHIRE_VERSION => "ruby_sapphire",
        consts::EMERALD_VERSION => "emerald",
        consts::FIRE_RED_VERSION | consts::LEAF_GREEN_VERSION => "firered_leafgreen",
        _ => return None,
    })
}

pub struct MapPack {
    pub game: String,
    pub gen: u8,
    pub versions: Vec<String>,
    pub geom: MapGeom,
    pub gen3: Option<Gen3Consts>,
    pub maps: Vec<MapDef>,
    pub map_index: HashMap<String, MapId>,
    pub layout: WorldLayout,
    /// every map's block ids (gen 1/2 widened to u16; gen 3 raw map.bin values)
    pub blockdata: Vec<u16>,
    pub blocksets: HashMap<String, Blockset>,
    pub tilesets: HashMap<String, Tileset>,
    pub palettes: Palettes,
    pub grass: HashMap<String, HashSet<u16>>,
    pub water: HashMap<String, HashSet<u16>>,
    pub objects: Vec<MapObject>,
    /// object index range per map id
    pub objects_by_map: Vec<Range<u32>>,
    pub links: Links,
    pub encounters: HashMap<MapId, EncounterTable>,
    pub sign_text: HashMap<String, String>,
    pub sprites: HashMap<String, SpriteMeta>,
    /// raw PNG bytes by sprite file stem (decoded by the viewer on demand)
    pub sprite_png: HashMap<String, Vec<u8>>,
}

#[derive(Debug, Clone, Default)]
pub struct WorldLayout {
    pub w_blocks: u32,
    pub h_blocks: u32,
    /// outdoor maps in draw order (underlays, base, overlays)
    pub draw_order: Vec<MapId>,
    /// world-pixel rectangles of the placed maps, in draw order
    pub placed_rects: Vec<(IRect, MapId)>,
}

fn parse<T: serde::de::DeserializeOwned>(name: &str, bytes: &[u8]) -> Result<T, MapError> {
    serde_json::from_slice(bytes).map_err(|e| MapError::Parse(name.to_string(), e.to_string()))
}

fn decode_tileset(gen: u8, bytes: &[u8]) -> Result<Tileset, String> {
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?.to_rgba8();
    let (w, h) = img.dimensions();
    let mut idx = Vec::with_capacity((w * h) as usize);
    for p in img.pixels() {
        let r = p.0[0];
        if gen == 3 {
            // pokemap: index = round((255 - r) * 15 / 255)
            idx.push((((255 - r as u32) * 15 + 127) / 255) as u8);
        } else {
            idx.push(if r >= 192 { 0 } else if r >= 128 { 1 } else if r >= 64 { 2 } else { 3 });
        }
    }
    Ok(Tileset { w: w as usize, h: h as usize, idx })
}

fn kind_of(s: &str) -> Option<ObjectKind> {
    Some(match s {
        "trainer" => ObjectKind::Trainer,
        "item" => ObjectKind::Item,
        "hidden_item" => ObjectKind::HiddenItem,
        "berry" => ObjectKind::Berry,
        "sign" => ObjectKind::Sign,
        "warp" => ObjectKind::Warp,
        "npc" => ObjectKind::Npc,
        _ => return None,
    })
}

fn facing_of(s: Option<&str>) -> Option<Facing> {
    match s? {
        "down" => Some(Facing::Down),
        "up" => Some(Facing::Up),
        "left" => Some(Facing::Left),
        "right" => Some(Facing::Right),
        _ => None,
    }
}

fn str_list(v: Option<&serde_json::Value>) -> Vec<String> {
    v.and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default()
}

fn payload_of(kind: ObjectKind, v: &serde_json::Value) -> Payload {
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(|s| s.to_string());
    match kind {
        ObjectKind::Trainer => Payload::Trainer {
            trainer: s("trainer"),
            variants: str_list(v.get("variants")),
            double: v.get("double").and_then(|x| x.as_bool()).unwrap_or(false),
            raw: s("raw"),
        },
        ObjectKind::Item | ObjectKind::HiddenItem | ObjectKind::Berry => Payload::Item {
            item: s("item"),
            raw: s("raw"),
            fake: v.get("fake").and_then(|x| x.as_bool()).unwrap_or(false),
        },
        ObjectKind::Sign => Payload::Sign { text_key: s("text_key").unwrap_or_default() },
        ObjectKind::Warp => Payload::Warp { dest_map: s("dest_map"), dest_warp: v.get("dest_warp").and_then(|x| x.as_i64()) },
        ObjectKind::Npc => Payload::Npc { label: s("label"), battles: str_list(v.get("battles")) },
    }
}

impl MapPack {
    pub fn map(&self, id: MapId) -> Option<&MapDef> {
        self.maps.get(id as usize)
    }

    pub fn map_by_const(&self, c: &str) -> Option<&MapDef> {
        self.map_index.get(c).and_then(|id| self.map(*id))
    }

    pub fn objects_of(&self, id: MapId) -> &[MapObject] {
        match self.objects_by_map.get(id as usize) {
            Some(r) => &self.objects[r.start as usize..r.end as usize],
            None => &[],
        }
    }

    /// Global object indices of a map.
    pub fn object_range(&self, id: MapId) -> Range<u32> {
        self.objects_by_map.get(id as usize).cloned().unwrap_or(0..0)
    }

    pub fn blocks_of(&self, id: MapId) -> Option<&[u16]> {
        let r = self.map(id)?.blocks.clone()?;
        self.blockdata.get(r)
    }

    /// Block id at a block cell of a map (gen 3: metatile id, collision bits stripped).
    pub fn block_at(&self, id: MapId, bx: u32, by: u32) -> Option<u16> {
        let m = self.map(id)?;
        if bx >= m.w || by >= m.h {
            return None;
        }
        let blocks = self.blocks_of(id)?;
        let v = *blocks.get((by * m.w + bx) as usize)?;
        Some(if self.gen == 3 { v & 0x3FF } else { v })
    }

    /// Is the block cell tall grass / surfable water (for the encounter card).
    pub fn terrain_at(&self, id: MapId, bx: u32, by: u32) -> (bool, bool) {
        let Some(m) = self.map(id) else { return (false, false) };
        let Some(v) = self.block_at(id, bx, by) else { return (false, false) };
        match &m.tileset {
            TilesetRef::Gen12 { file } => (
                self.grass.get(file).map(|s| s.contains(&v)).unwrap_or(false),
                self.water.get(file).map(|s| s.contains(&v)).unwrap_or(false),
            ),
            TilesetRef::Gen3 { primary, secondary } => {
                let n = self.gen3.map(|g| g.num_metatiles_in_primary as u16).unwrap_or(512);
                let (ts, local) = if v < n { (Some(primary), v) } else { (secondary.as_ref(), v - n) };
                match ts {
                    Some(t) => (
                        self.grass.get(t).map(|s| s.contains(&local)).unwrap_or(false),
                        self.water.get(t).map(|s| s.contains(&local)).unwrap_or(false),
                    ),
                    None => (false, false),
                }
            }
        }
    }

    pub fn load(game: &str, source: &PackSource) -> Result<MapPack, MapError> {
        if !source.has_game(game) {
            return Err(MapError::NoGame(game.to_string()));
        }
        let manifest: Manifest = parse("manifest.json", &source.read(game, "manifest.json")?)?;
        if manifest.format != PACK_FORMAT {
            return Err(MapError::Format(manifest.format));
        }
        let gen = manifest.gen;
        let geom = MapGeom { block_px: manifest.block_px, step_px: manifest.step_px.max(1) };
        let raw_maps: Vec<RawMap> = parse("maps.json", &source.read(game, "maps.json")?)?;
        let layout_raw: RawLayout = parse("layout.json", &source.read(game, "layout.json")?)?;

        // --- blobs ---
        let bd_bytes = source.read(game, "blockdata.bin")?;
        let blockdata: Vec<u16> = if gen == 3 {
            bd_bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect()
        } else {
            bd_bytes.iter().map(|b| *b as u16).collect()
        };
        let mut block_ranges: HashMap<String, Range<usize>> = HashMap::new();
        for e in &manifest.blockdata {
            if e.offset + e.len <= blockdata.len() {
                block_ranges.insert(e.map.clone(), e.offset..e.offset + e.len);
            }
        }
        let bs_bytes = source.read(game, "blocksets.bin")?;
        let mut blocksets: HashMap<String, Blockset> = HashMap::new();
        for e in &manifest.blocksets {
            if gen == 3 {
                let start = e.offset * 16;
                let end = start + e.count * 16;
                if end > bs_bytes.len() {
                    return Err(MapError::Other(format!("blocksets.bin too short for {}", e.tileset)));
                }
                let mut v = Vec::with_capacity(e.count);
                for mt in bs_bytes[start..end].chunks_exact(16) {
                    let mut arr = [0u16; 8];
                    for (j, c) in mt.chunks_exact(2).enumerate() {
                        arr[j] = u16::from_le_bytes([c[0], c[1]]);
                    }
                    v.push(arr);
                }
                blocksets.insert(e.tileset.clone(), Blockset::Gen3(v));
            } else {
                let start = e.offset * 16;
                let end = start + e.count * 16;
                if end > bs_bytes.len() {
                    return Err(MapError::Other(format!("blocksets.bin too short for {}", e.tileset)));
                }
                let v = bs_bytes[start..end].chunks_exact(16).map(|b| { let mut a = [0u8; 16]; a.copy_from_slice(b); a }).collect();
                blocksets.insert(e.tileset.clone(), Blockset::Gen12(v));
            }
        }

        // --- maps ---
        let mut maps: Vec<MapDef> = Vec::with_capacity(raw_maps.len());
        let mut map_index: HashMap<String, MapId> = HashMap::new();
        for (i, r) in raw_maps.iter().enumerate() {
            map_index.insert(r.const_name.clone(), i as MapId);
        }
        for (i, r) in raw_maps.iter().enumerate() {
            let tileset = if gen == 3 {
                TilesetRef::Gen3 { primary: r.tileset.primary.clone().unwrap_or_default(), secondary: r.tileset.secondary.clone() }
            } else {
                TilesetRef::Gen12 { file: r.tileset.file.clone().unwrap_or_default() }
            };
            let palette = match (&r.palette, gen) {
                (Some(p), 1) => PaletteRef::Gen1 { index: p.index.unwrap_or(0) },
                (Some(p), 2) => PaletteRef::Gen2 { tod: p.tod.clone().unwrap_or_else(|| "day".into()), group: p.group.unwrap_or(0) },
                _ => PaletteRef::None,
            };
            let world_pos = layout_raw.positions.get(&r.const_name).map(|p| (p[0], p[1]));
            maps.push(MapDef {
                id: i as MapId,
                const_name: r.const_name.clone(),
                name: r.name.clone(),
                display: r.display.clone().unwrap_or_else(|| r.name.clone()),
                w: r.w,
                h: r.h,
                kind: if r.kind == "outdoor" && world_pos.is_some() { MapKind::Outdoor } else { MapKind::Indoor },
                tileset,
                palette,
                connections: r.connections.iter().map(|c| Connection { dir: c.dir.clone(), map: map_index.get(&c.map).copied(), offset: c.offset }).collect(),
                blocks: block_ranges.get(&r.const_name).cloned(),
                world_pos,
            });
        }
        let mut draw_order = Vec::new();
        let mut placed_rects = Vec::new();
        for c in &layout_raw.draw_order {
            if let Some(&id) = map_index.get(c) {
                let m = &maps[id as usize];
                if let Some((px, py)) = m.world_pos {
                    draw_order.push(id);
                    let bp = geom.block_px as i32;
                    placed_rects.push((IRect::from_size(px * bp, py * bp, (m.w * geom.block_px) as i32, (m.h * geom.block_px) as i32), id));
                }
            }
        }
        let layout = WorldLayout { w_blocks: manifest.world.w_blocks, h_blocks: manifest.world.h_blocks, draw_order, placed_rects };

        // --- tilesets / palettes / terrain ---
        let mut tilesets = HashMap::new();
        for f in source.list(game, "tilesets") {
            if !f.ends_with(".png") {
                continue;
            }
            let bytes = source.read(game, &format!("tilesets/{}", f))?;
            match decode_tileset(gen, &bytes) {
                Ok(t) => {
                    tilesets.insert(f.trim_end_matches(".png").to_string(), t);
                }
                Err(e) => log::warn!("tileset {} failed to decode: {}", f, e),
            }
        }
        let pal_bytes = source.read(game, "palettes.json")?;
        let palettes = match gen {
            1 => Palettes::Gen1(parse("palettes.json", &pal_bytes)?),
            2 => Palettes::Gen2(parse("palettes.json", &pal_bytes)?),
            _ => Palettes::Gen3(parse("palettes.json", &pal_bytes)?),
        };
        let terrain: RawTerrain = parse("terrain.json", &source.read(game, "terrain.json")?)?;
        let grass = terrain.grass.into_iter().map(|(k, v)| (k, v.into_iter().collect())).collect();
        let water = terrain.water.into_iter().map(|(k, v)| (k, v.into_iter().collect())).collect();

        // --- objects ---
        let raw_objects: Vec<RawObject> = parse("objects.json", &source.read(game, "objects.json")?)?;
        let mut objects: Vec<MapObject> = Vec::with_capacity(raw_objects.len());
        for o in &raw_objects {
            let (Some(&map), Some(kind)) = (map_index.get(&o.map), kind_of(&o.kind)) else { continue };
            objects.push(MapObject {
                map,
                x: o.x.max(0) as u16,
                y: o.y.max(0) as u16,
                kind,
                sprite: o.sprite.clone(),
                facing: facing_of(o.facing.as_deref()),
                pal: o.pal.clone(),
                payload: payload_of(kind, &o.payload),
            });
        }
        // objects.json is grouped by map in map order; index the ranges
        let mut objects_by_map: Vec<Range<u32>> = vec![0..0; maps.len()];
        let mut i = 0usize;
        while i < objects.len() {
            let m = objects[i].map;
            let mut j = i;
            while j < objects.len() && objects[j].map == m {
                j += 1;
            }
            let r = &mut objects_by_map[m as usize];
            if r.start == r.end {
                *r = i as u32..j as u32;
            } else {
                log::warn!("objects of map {} are not contiguous", m);
            }
            i = j;
        }

        // --- links / encounters / text / sprites ---
        let raw_links: RawLinks = parse("links.json", &source.read(game, "links.json")?)?;
        let conv = |a: &RawAnchor| -> Option<Anchor> {
            let map = *map_index.get(&a.map)?;
            Some(Anchor {
                map,
                x: a.x.max(0) as u16,
                y: a.y.max(0) as u16,
                precision: match a.precision.as_str() {
                    "object" => Precision::Object,
                    "script" => Precision::Script,
                    _ => Precision::Map,
                },
                object: a.object,
                source: a.source.clone().unwrap_or_default(),
            })
        };
        let mut links = Links { stats: raw_links.stats.clone(), ..Default::default() };
        for (k, v) in &raw_links.trainers {
            links.trainers.insert(sanitize_string(k), v.iter().filter_map(conv).collect());
        }
        for (k, v) in &raw_links.items {
            links.items.insert(sanitize_string(k), v.iter().filter_map(conv).collect());
        }
        let raw_enc: HashMap<String, HashMap<String, RawMethod>> = parse("encounters.json", &source.read(game, "encounters.json")?)?;
        let mut encounters: HashMap<MapId, EncounterTable> = HashMap::new();
        for (c, methods) in raw_enc {
            let Some(&id) = map_index.get(&c) else { continue };
            let mut table = EncounterTable::default();
            let mut names: Vec<&String> = methods.keys().collect();
            names.sort_by_key(|n| method_order(n));
            for n in names {
                let m = &methods[n];
                let mut slots: Vec<(String, Vec<EncounterSlot>)> = m.slots.iter().map(|(v, s)| (v.clone(), s.clone())).collect();
                slots.sort_by(|a, b| a.0.cmp(&b.0));
                table.methods.push(EncounterMethod { name: n.clone(), base_rate: m.base_rate, slots });
            }
            encounters.insert(id, table);
        }
        let sign_text: HashMap<String, String> = source.read(game, "sign_text.json").ok().and_then(|b| parse("sign_text.json", &b).ok()).unwrap_or_default();
        let sprites_raw: serde_json::Value = source.read(game, "sprites.json").ok().and_then(|b| parse("sprites.json", &b).ok()).unwrap_or(serde_json::Value::Null);
        let mut sprites = HashMap::new();
        if let Some(obj) = sprites_raw.as_object() {
            for (k, v) in obj {
                let meta = match v {
                    serde_json::Value::String(f) => SpriteMeta { file: f.clone(), w: 16, h: 16 },
                    serde_json::Value::Object(o) => SpriteMeta {
                        file: o.get("file").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                        w: o.get("width").and_then(|x| x.as_u64()).unwrap_or(16) as u32,
                        h: o.get("height").and_then(|x| x.as_u64()).unwrap_or(16) as u32,
                    },
                    _ => continue,
                };
                sprites.insert(k.clone(), meta);
            }
        }
        let mut sprite_png = HashMap::new();
        for f in source.list(game, "sprites") {
            if f.ends_with(".png") {
                if let Ok(b) = source.read(game, &format!("sprites/{}", f)) {
                    sprite_png.insert(f.trim_end_matches(".png").to_string(), b);
                }
            }
        }

        Ok(MapPack {
            game: game.to_string(),
            gen,
            versions: manifest.versions.clone(),
            geom,
            gen3: manifest.gen3,
            maps,
            map_index,
            layout,
            blockdata,
            blocksets,
            tilesets,
            palettes,
            grass,
            water,
            objects,
            objects_by_map,
            links,
            encounters,
            sign_text,
            sprites,
            sprite_png,
        })
    }

    /// Default on-disk location next to `raw_pkmn_data`.
    pub fn default_dir(raw_pkmn_data: &Path) -> Option<PathBuf> {
        raw_pkmn_data.parent().map(|p| p.join("map_data"))
    }
}

fn method_order(name: &str) -> (usize, String) {
    let base = name.split('_').next().unwrap_or(name);
    let rank = match base {
        "walk" => 0,
        "surf" => 1,
        "old" => 2,
        "good" => 3,
        "super" => 4,
        "rock" => 5,
        "headbutt" => 6,
        _ => 9,
    };
    (rank, name.to_string())
}
