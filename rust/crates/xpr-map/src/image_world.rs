//! The runtime side of a format 2 ("image world") pack: gen 4/5 maps are a
//! pre-rendered, tilted 3D picture instead of tilesets, so the pack carries
//! what the picture cannot tell the viewer by itself. That is: which map
//! owns which part of the world (the maps' rectangles overlap), every tile's
//! terrain class and how far the tilted render lifted it, and the masks that
//! dim the scenery the player can never reach.
//!
//! Tiles are 16 px and are also the step grid, so a map's `w`/`h` are in
//! tiles, `geom.block_px == geom.step_px == 16`, and world px = tile · 16.

use std::ops::Range;

use crate::model::{ImageryInfo, MapId, RawMaskSurface};

/// Tile size of the image-world packs, in px.
pub const TILE_PX: i32 = 16;

/// How far below / above a tile's own row a lifted tile can be found
/// (pokemap's `pickTileLocal`: rows `base - 4 ..= base + 16`).
const PICK_ROWS_ABOVE: i32 = 4;
const PICK_ROWS_BELOW: i32 = 16;

#[derive(Debug, Clone)]
pub struct Ownership {
    /// world tile of cell (0, 0)
    pub origin: (i32, i32),
    /// tiles per cell side
    pub cell: i32,
    pub cols: i32,
    pub rows: i32,
    /// row-major map id or -1
    pub cells: Vec<i32>,
}

impl Ownership {
    /// The map that owns a world tile.
    pub fn owner(&self, tx: i32, ty: i32) -> Option<MapId> {
        let c = (tx - self.origin.0).div_euclid(self.cell);
        let r = (ty - self.origin.1).div_euclid(self.cell);
        if c < 0 || r < 0 || c >= self.cols || r >= self.rows {
            return None;
        }
        let v = *self.cells.get((r * self.cols + c) as usize)?;
        (v >= 0).then_some(v as MapId)
    }
}

/// A decoded visibility mask: one byte per 16 px cell, 1 = visible. Cells
/// past `w × h` are hidden.
#[derive(Debug, Clone)]
pub struct Mask {
    pub w: usize,
    pub h: usize,
    pub cells: Vec<u8>,
}

impl Mask {
    pub fn decode(raw: &RawMaskSurface) -> Option<Mask> {
        let n = raw.w * raw.h;
        let mut cells = vec![0u8; n];
        let mut i = 0usize;
        for pair in raw.rle.chunks_exact(2) {
            let (v, run) = (pair[0], pair[1] as usize);
            if i + run > n {
                return None;
            }
            if v != 0 {
                cells[i..i + run].fill(1);
            }
            i += run;
        }
        (i == n).then_some(Mask { w: raw.w, h: raw.h, cells })
    }

    pub fn visible(&self, cx: i32, cy: i32) -> bool {
        cx >= 0 && cy >= 0 && (cx as usize) < self.w && (cy as usize) < self.h && self.cells[cy as usize * self.w + cx as usize] != 0
    }

    /// Visibility at a pixel (in the mask's surface px), smoothed across
    /// cell edges like a bilinear-filtered mask texture: 0.0 hidden, 1.0
    /// visible.
    pub fn coverage(&self, px: f32, py: f32) -> f32 {
        let fx = px / TILE_PX as f32 - 0.5;
        let fy = py / TILE_PX as f32 - 0.5;
        let x0 = fx.floor();
        let y0 = fy.floor();
        let tx = fx - x0;
        let ty = fy - y0;
        let (x0, y0) = (x0 as i32, y0 as i32);
        let v = |x: i32, y: i32| if self.visible(x, y) { 1.0 } else { 0.0 };
        let top = v(x0, y0) * (1.0 - tx) + v(x0 + 1, y0) * tx;
        let bottom = v(x0, y0 + 1) * (1.0 - tx) + v(x0 + 1, y0 + 1) * tx;
        top * (1.0 - ty) + bottom * ty
    }
}

/// Everything format 2 adds to a `MapPack`.
#[derive(Debug, Clone)]
pub struct ImageWorld {
    pub w_px: u32,
    pub h_px: u32,
    pub top_px: u32,
    /// pyramid levels of the world imagery (level `L` tiles cover 512·2^L px)
    pub levels: u8,
    pub tile_size: u32,
    pub default_map: Option<MapId>,
    pub ownership: Ownership,
    /// per map (by id): range into `classes` / `lift` (w · h tiles)
    pub tiles: Vec<Option<Range<usize>>>,
    /// `.` walkable, `#` blocked, `g` land encounters, `w` water, ` ` no data
    pub classes: Vec<u8>,
    /// px the tilted render lifted each tile
    pub lift: Vec<i16>,
    /// per map (by id): its largest lift, for hit-test slack
    pub max_lift: Vec<i16>,
    pub world_mask: Option<Mask>,
    /// interior image path -> mask
    pub interior_masks: std::collections::HashMap<String, Mask>,
    pub imagery: Option<ImageryInfo>,
    /// (method id, label) in the manifest's order
    pub encounter_methods: Vec<(String, String)>,
    /// (condition id, label)
    pub encounter_conditions: Vec<(String, String)>,
}

impl ImageWorld {
    fn tile_index(&self, map: MapId, w: u32, h: u32, tx: i32, ty: i32) -> Option<usize> {
        if tx < 0 || ty < 0 || tx >= w as i32 || ty >= h as i32 {
            return None;
        }
        let r = self.tiles.get(map as usize)?.as_ref()?;
        let i = r.start + (ty as usize) * (w as usize) + tx as usize;
        (i < r.end).then_some(i)
    }

    pub fn class_at(&self, map: MapId, w: u32, h: u32, tx: i32, ty: i32) -> Option<u8> {
        self.tile_index(map, w, h, tx, ty).map(|i| self.classes[i])
    }

    pub fn lift_at(&self, map: MapId, w: u32, h: u32, tx: i32, ty: i32) -> i16 {
        self.tile_index(map, w, h, tx, ty).map(|i| self.lift[i]).unwrap_or(0)
    }

    /// The tile under a map-local pixel, taking terrain lift into account:
    /// the front-most tile whose lifted rectangle contains the point (a port
    /// of pokemap's `pickTileLocal`, GEN45_REQUIREMENTS §4.5).
    pub fn pick_local(&self, map: MapId, w: u32, h: u32, lx: i32, ly: i32) -> Option<(i32, i32)> {
        let tx = lx.div_euclid(TILE_PX);
        if tx < 0 || tx >= w as i32 {
            return None;
        }
        let base = ly.div_euclid(TILE_PX);
        let has_lift = self.tiles.get(map as usize).map(|r| r.is_some()).unwrap_or(false);
        if !has_lift {
            return (base >= 0 && base < h as i32).then_some((tx, base));
        }
        let lo = (base - PICK_ROWS_ABOVE).max(0);
        let hi = (base + PICK_ROWS_BELOW).min(h as i32 - 1);
        let mut ty = hi;
        while ty >= lo {
            let top = ty * TILE_PX - self.lift_at(map, w, h, tx, ty) as i32;
            if ly >= top && ly < top + TILE_PX {
                return Some((tx, ty));
            }
            ty -= 1;
        }
        None
    }
}
