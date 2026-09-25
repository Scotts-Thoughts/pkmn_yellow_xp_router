//! Serde shapes of the pack files (format v1, see
//! `docs/rust_port/design/world_map/SPEC.md` §3.2) and the processed
//! runtime types built from them.

use std::collections::HashMap;
use std::ops::Range;

use serde::Deserialize;

pub type MapId = u16;

// ---------------------------------------------------------------------------
// raw pack files
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub format: u32,
    pub game: String,
    pub gen: u8,
    pub versions: Vec<String>,
    pub block_px: u32,
    pub step_px: u32,
    pub world: WorldSize,
    #[serde(default)]
    pub gen3: Option<Gen3Consts>,
    pub blockdata: Vec<BlobEntry>,
    pub blocksets: Vec<BlocksetEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorldSize {
    pub w_blocks: u32,
    pub h_blocks: u32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Gen3Consts {
    pub num_metatiles_in_primary: u32,
    pub num_tiles_in_primary: u32,
    pub num_pals_in_primary: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlobEntry {
    pub map: String,
    pub offset: usize,
    pub len: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlocksetEntry {
    pub tileset: String,
    pub offset: usize,
    pub count: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawMap {
    pub id: u32,
    #[serde(rename = "const")]
    pub const_name: String,
    pub name: String,
    pub w: u32,
    pub h: u32,
    pub kind: String,
    pub tileset: RawTilesetRef,
    #[serde(default)]
    pub palette: Option<RawPaletteRef>,
    #[serde(default)]
    pub connections: Vec<RawConnection>,
    #[serde(default)]
    pub env: Option<String>,
    #[serde(default)]
    pub has_blockdata: bool,
    #[serde(default)]
    pub display: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawTilesetRef {
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub primary: Option<String>,
    #[serde(default)]
    pub secondary: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawPaletteRef {
    #[serde(default)]
    pub index: Option<usize>,
    #[serde(default)]
    pub tod: Option<String>,
    #[serde(default)]
    pub group: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawConnection {
    pub dir: String,
    pub map: String,
    pub offset: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawLayout {
    pub positions: HashMap<String, [i32; 2]>,
    pub draw_order: Vec<String>,
    #[serde(default)]
    pub outdoor: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawObject {
    pub map: String,
    pub x: i32,
    pub y: i32,
    pub kind: String,
    #[serde(default)]
    pub sprite: Option<String>,
    #[serde(default)]
    pub facing: Option<String>,
    /// gen 2: the object's `PAL_NPC_*` palette
    #[serde(default)]
    pub pal: Option<String>,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawAnchor {
    pub map: String,
    pub x: i32,
    pub y: i32,
    pub precision: String,
    #[serde(default)]
    pub object: Option<u32>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawLinks {
    #[serde(default)]
    pub trainers: HashMap<String, Vec<RawAnchor>>,
    #[serde(default)]
    pub items: HashMap<String, Vec<RawAnchor>>,
    #[serde(default)]
    pub stats: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawMethod {
    #[serde(default)]
    pub base_rate: Option<i64>,
    pub slots: HashMap<String, Vec<EncounterSlot>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EncounterSlot {
    pub species: String,
    pub min_level: i64,
    pub max_level: i64,
    /// slot rate in percent (gen 2 tables carry fractional rates)
    pub rate: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawTerrain {
    #[serde(default)]
    pub grass: HashMap<String, Vec<u16>>,
    #[serde(default)]
    pub water: HashMap<String, Vec<u16>>,
}

pub type Rgb = [u8; 3];
pub type Pal4 = [Rgb; 4];

#[derive(Debug, Clone, Deserialize)]
pub struct Gen1Palettes {
    #[serde(rename = "cgbPalettes")]
    pub cgb_palettes: Vec<Pal4>,
    #[serde(rename = "mapAssignments", default)]
    pub map_assignments: HashMap<String, Gen1MapPalette>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Gen1MapPalette {
    #[serde(rename = "paletteIndex", default)]
    pub palette_index: usize,
    pub colors: Pal4,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Gen2Palettes {
    #[serde(rename = "bgPalettes")]
    pub bg_palettes: HashMap<String, Vec<Pal4>>,
    #[serde(rename = "paletteMaps", default)]
    pub palette_maps: HashMap<String, Vec<u8>>,
    #[serde(rename = "mapAssignments", default)]
    pub map_assignments: HashMap<String, Gen2MapPalette>,
    #[serde(rename = "npcPalettes", default)]
    pub npc_palettes: HashMap<String, Vec<Pal4>>,
    #[serde(rename = "roofPalettes", default)]
    pub roof_palettes: Vec<Option<RoofPalette>>,
    #[serde(rename = "tilesetPalettes", default)]
    pub tileset_palettes: HashMap<String, Vec<Pal4>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Gen2MapPalette {
    #[serde(rename = "timeOfDay", default)]
    pub time_of_day: Option<String>,
    #[serde(rename = "tilesetFileName", default)]
    pub tileset_file_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RoofPalette {
    #[serde(default)]
    pub day: Option<[Rgb; 2]>,
    #[serde(default)]
    pub nite: Option<[Rgb; 2]>,
}

/// gen 3: tileset name -> 16 palettes x 16 colours (a palette may be empty).
pub type Gen3Palettes = HashMap<String, Vec<Vec<Rgb>>>;

// ---------------------------------------------------------------------------
// processed runtime types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapGeom {
    pub block_px: u32,
    pub step_px: u32,
}

impl MapGeom {
    pub fn steps_per_block(&self) -> u32 {
        (self.block_px / self.step_px).max(1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapKind {
    Outdoor,
    Indoor,
}

#[derive(Debug, Clone)]
pub enum TilesetRef {
    Gen12 { file: String },
    Gen3 { primary: String, secondary: Option<String> },
}

#[derive(Debug, Clone)]
pub enum PaletteRef {
    None,
    Gen1 { index: usize },
    Gen2 { tod: String, group: usize },
}

#[derive(Debug, Clone)]
pub struct Connection {
    pub dir: String,
    pub map: Option<MapId>,
    pub offset: i32,
}

#[derive(Debug, Clone)]
pub struct MapDef {
    pub id: MapId,
    pub const_name: String,
    pub name: String,
    pub display: String,
    /// size in blocks (gen 1/2: 32 px; gen 3: 16 px metatiles)
    pub w: u32,
    pub h: u32,
    pub kind: MapKind,
    pub tileset: TilesetRef,
    pub palette: PaletteRef,
    pub connections: Vec<Connection>,
    /// range into `MapPack::blockdata`
    pub blocks: Option<Range<usize>>,
    /// position in the world, in blocks (outdoor maps only)
    pub world_pos: Option<(i32, i32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Trainer,
    Item,
    HiddenItem,
    Berry,
    Sign,
    Warp,
    Npc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Facing {
    Down,
    Up,
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub enum Payload {
    Trainer { trainer: Option<String>, variants: Vec<String>, double: bool, raw: Option<String> },
    Item { item: Option<String>, raw: Option<String>, fake: bool },
    Sign { text_key: String },
    Warp { dest_map: Option<String>, dest_warp: Option<i64> },
    Npc { label: Option<String>, battles: Vec<String> },
    None,
}

#[derive(Debug, Clone)]
pub struct MapObject {
    pub map: MapId,
    /// position in 16 px steps, relative to the map
    pub x: u16,
    pub y: u16,
    pub kind: ObjectKind,
    pub sprite: Option<String>,
    pub facing: Option<Facing>,
    /// gen 2: the object's `PAL_NPC_*` palette
    pub pal: Option<String>,
    pub payload: Payload,
}

impl MapObject {
    /// The kind for display and toggles: an NPC whose script starts a
    /// battle (gym leaders, the Elite Four, scripted rivals) is a trainer.
    pub fn effective_kind(&self) -> ObjectKind {
        match (&self.kind, &self.payload) {
            (ObjectKind::Npc, Payload::Npc { battles, .. }) if !battles.is_empty() => ObjectKind::Trainer,
            (k, _) => *k,
        }
    }

    /// The router trainer name(s) this object stands for.
    pub fn trainer_names(&self) -> &[String] {
        match &self.payload {
            Payload::Trainer { variants, .. } => variants,
            Payload::Npc { battles, .. } => battles,
            _ => &[],
        }
    }
    pub fn item_name(&self) -> Option<&str> {
        match &self.payload {
            Payload::Item { item, .. } => item.as_deref(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    Object,
    Script,
    Map,
}

#[derive(Debug, Clone)]
pub struct Anchor {
    pub map: MapId,
    pub x: u16,
    pub y: u16,
    pub precision: Precision,
    pub object: Option<u32>,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct EncounterMethod {
    pub name: String,
    pub base_rate: Option<i64>,
    /// ("*" or a version name, slots)
    pub slots: Vec<(String, Vec<EncounterSlot>)>,
}

#[derive(Debug, Clone, Default)]
pub struct EncounterTable {
    pub methods: Vec<EncounterMethod>,
}

impl EncounterTable {
    /// The slots of `method` for `version` (falls back to the shared column).
    pub fn slots_for(&self, method: &str, version: &str) -> Option<&[EncounterSlot]> {
        let m = self.methods.iter().find(|m| m.name == method)?;
        m.slots.iter().find(|(v, _)| v == version).or_else(|| m.slots.iter().find(|(v, _)| v == "*")).map(|(_, s)| s.as_slice())
    }
}

#[derive(Debug, Clone)]
pub struct SpriteMeta {
    pub file: String,
    pub w: u32,
    pub h: u32,
}

/// A decoded tileset sheet: one palette/shade index per pixel.
#[derive(Debug, Clone)]
pub struct Tileset {
    pub w: usize,
    pub h: usize,
    pub idx: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum Blockset {
    /// gen 1/2: 4x4 tile indices per block
    Gen12(Vec<[u8; 16]>),
    /// gen 3: 8 packed tile refs per metatile (bottom 0-3, top 4-7)
    Gen3(Vec<[u16; 8]>),
}

impl Blockset {
    pub fn len(&self) -> usize {
        match self {
            Blockset::Gen12(v) => v.len(),
            Blockset::Gen3(v) => v.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug, Clone)]
pub enum Palettes {
    Gen1(Gen1Palettes),
    Gen2(Gen2Palettes),
    Gen3(Gen3Palettes),
}
