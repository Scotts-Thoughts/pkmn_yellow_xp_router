//! The tile compositor (SPEC §3.4): gen 1 (one 4-colour palette per map),
//! gen 2 (per-tile palette slots, time of day, roof and tileset overrides),
//! gen 3 (primary + secondary tileset, 2x2 metatiles x 2 layers, 16-colour
//! palettes, flips). A port of pokemap's `src/render/tile-renderer.js`;
//! pokemap's PNG export at 1x is the golden reference.
//!
//! Output is a `Pixmap` for a region of a map, or a 512x512 world/indoor
//! chunk at a LOD level (level L covers 512·2^L world px).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::geom::{map_px_size, scope_rect, IRect, Scope};
use crate::lod::Pixmap;
use crate::model::*;
use crate::pack::MapPack;

pub const CHUNK_PX: i32 = 512;
pub const TILE: usize = 8;
/// pokemap's world background (#0a0a18).
pub const BACKGROUND: [u8; 4] = [10, 10, 24, 255];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct RenderOpts {
    /// gen 2: use the night palettes for outdoor maps
    pub night: bool,
}

/// A tileset recoloured with a concrete palette set (gen 1/2), RGBA.
struct RgbaSheet {
    w: usize,
    data: Vec<u8>,
}

/// Every metatile of a gen 3 tileset pair composited to 16x16 RGBA.
struct MetatileCache {
    /// metatile id -> 16*16*4 bytes
    tiles: Vec<[u8; 16 * 16 * 4]>,
}

pub struct Compositor {
    pack: Arc<MapPack>,
    sheets: Mutex<HashMap<String, Arc<RgbaSheet>>>,
    metatiles: Mutex<HashMap<String, Arc<MetatileCache>>>,
}

impl Compositor {
    pub fn new(pack: Arc<MapPack>) -> Compositor {
        Compositor { pack, sheets: Mutex::new(HashMap::new()), metatiles: Mutex::new(HashMap::new()) }
    }

    pub fn pack(&self) -> &Arc<MapPack> {
        &self.pack
    }

    // ---- gen 1 / 2 sheets ----------------------------------------------------------

    /// The palette set (8 x 4 colours) and cache key for a gen 2 map.
    fn gen2_palettes(&self, m: &MapDef, tileset: &str, opts: RenderOpts) -> (Vec<Pal4>, String) {
        let Palettes::Gen2(p) = &self.pack.palettes else { return (Vec::new(), String::new()) };
        let (base_tod, group) = match &m.palette {
            PaletteRef::Gen2 { tod, group } => (tod.as_str(), *group),
            _ => ("day", 0),
        };
        let tod = if opts.night && base_tod != "indoor" { "nite" } else { base_tod };
        if let Some(ov) = p.tileset_palettes.get(tileset) {
            return (ov.clone(), format!("ts_{}", tileset));
        }
        let bg = p.bg_palettes.get(tod).or_else(|| p.bg_palettes.get("day")).cloned().unwrap_or_default();
        let roof_tod = if tod == "indoor" { "day" } else { tod };
        let roof = p.roof_palettes.get(group).and_then(|r| r.as_ref()).and_then(|r| if roof_tod == "nite" { r.nite.or(r.day) } else { r.day });
        if let (Some(rc), true) = (roof, bg.len() > 6) {
            let mut pals = bg.clone();
            pals[6][1] = rc[0];
            pals[6][2] = rc[1];
            return (pals, format!("{}_g{}", tod, group));
        }
        (bg, format!("{}_none", tod))
    }

    fn sheet_for(&self, m: &MapDef, opts: RenderOpts) -> Option<Arc<RgbaSheet>> {
        let TilesetRef::Gen12 { file } = &m.tileset else { return None };
        let ts = self.pack.tilesets.get(file)?;
        let (palettes, key): (Vec<Pal4>, String) = match &self.pack.palettes {
            Palettes::Gen1(p) => {
                let (idx, colors) = match p.map_assignments.get(&m.const_name) {
                    Some(a) => (a.palette_index, a.colors),
                    None => (0, p.cgb_palettes.first().copied().unwrap_or([[255, 255, 255], [170, 170, 170], [85, 85, 85], [0, 0, 0]])),
                };
                (vec![colors], format!("{}:g1:{}", file, idx))
            }
            Palettes::Gen2(_) => {
                let (pals, k) = self.gen2_palettes(m, file, opts);
                (pals, format!("{}:g2:{}", file, k))
            }
            Palettes::Gen3(_) => return None,
        };
        if let Some(s) = self.sheets.lock().unwrap().get(&key) {
            return Some(s.clone());
        }
        let mut data = vec![0u8; ts.w * ts.h * 4];
        let per_tile = match &self.pack.palettes {
            Palettes::Gen2(p) => p.palette_maps.get(file).cloned(),
            _ => None,
        };
        let tiles_per_row = ts.w / TILE;
        for (i, &shade) in ts.idx.iter().enumerate() {
            let pal = match &per_tile {
                Some(map) => {
                    let x = i % ts.w;
                    let y = i / ts.w;
                    let tile_idx = (y / TILE) * tiles_per_row + x / TILE;
                    let slot = map.get(tile_idx).copied().unwrap_or(0) as usize;
                    palettes.get(slot).or_else(|| palettes.first())
                }
                None => palettes.first(),
            };
            let c = pal.map(|p| p[(shade & 3) as usize]).unwrap_or([255, 0, 255]);
            data[i * 4..i * 4 + 3].copy_from_slice(&c);
            data[i * 4 + 3] = 255;
        }
        let sheet = Arc::new(RgbaSheet { w: ts.w, data });
        self.sheets.lock().unwrap().insert(key, sheet.clone());
        Some(sheet)
    }

    // ---- gen 3 metatiles -----------------------------------------------------------

    fn metatiles_for(&self, m: &MapDef) -> Option<Arc<MetatileCache>> {
        let TilesetRef::Gen3 { primary, secondary } = &m.tileset else { return None };
        let key = format!("{}+{}", primary, secondary.as_deref().unwrap_or(""));
        if let Some(c) = self.metatiles.lock().unwrap().get(&key) {
            return Some(c.clone());
        }
        let consts = self.pack.gen3.unwrap_or(Gen3Consts { num_metatiles_in_primary: 512, num_tiles_in_primary: 512, num_pals_in_primary: 6 });
        let Palettes::Gen3(pals) = &self.pack.palettes else { return None };
        let pri_ts = self.pack.tilesets.get(primary)?;
        let sec_ts = secondary.as_ref().and_then(|s| self.pack.tilesets.get(s));
        let empty_meta: Vec<[u16; 8]> = Vec::new();
        let pri_meta = match self.pack.blocksets.get(primary) {
            Some(Blockset::Gen3(v)) => v,
            _ => &empty_meta,
        };
        let sec_meta = match secondary.as_ref().and_then(|s| self.pack.blocksets.get(s)) {
            Some(Blockset::Gen3(v)) => v,
            _ => &empty_meta,
        };
        let pri_pals = pals.get(primary).cloned().unwrap_or_default();
        let sec_pals = secondary.as_ref().and_then(|s| pals.get(s)).cloned().unwrap_or_default();
        let mut combined: Vec<Vec<Rgb>> = Vec::with_capacity(16);
        for i in 0..16 {
            if (i as u32) < consts.num_pals_in_primary {
                combined.push(pri_pals.get(i).cloned().unwrap_or_default());
            } else {
                combined.push(sec_pals.get(i).cloned().unwrap_or_default());
            }
        }
        let total = consts.num_metatiles_in_primary as usize + sec_meta.len();
        let mut tiles: Vec<[u8; 16 * 16 * 4]> = vec![[0u8; 16 * 16 * 4]; total.max(consts.num_metatiles_in_primary as usize)];
        for (id, out) in tiles.iter_mut().enumerate() {
            let entries = if id < consts.num_metatiles_in_primary as usize { pri_meta.get(id) } else { sec_meta.get(id - consts.num_metatiles_in_primary as usize) };
            let Some(entries) = entries else { continue };
            for layer in 0..2usize {
                for ty in 0..2usize {
                    for tx in 0..2usize {
                        let e = entries[layer * 4 + ty * 2 + tx];
                        let tile_num = (e & 0x3FF) as usize;
                        let xflip = (e >> 10) & 1 == 1;
                        let yflip = (e >> 11) & 1 == 1;
                        let pal = &combined[((e >> 12) & 0xF) as usize];
                        let (src, local) = if tile_num < consts.num_tiles_in_primary as usize {
                            (Some(pri_ts), tile_num)
                        } else {
                            (sec_ts, tile_num - consts.num_tiles_in_primary as usize)
                        };
                        let Some(src) = src else { continue };
                        let tiles_per_row = src.w / TILE;
                        let sx0 = (local % tiles_per_row) * TILE;
                        let sy0 = (local / tiles_per_row) * TILE;
                        if sy0 + TILE > src.h {
                            continue;
                        }
                        for py in 0..TILE {
                            for px in 0..TILE {
                                let spx = if xflip { TILE - 1 - px } else { px };
                                let spy = if yflip { TILE - 1 - py } else { py };
                                let ci = src.idx[(sy0 + spy) * src.w + sx0 + spx] as usize;
                                if layer == 1 && ci == 0 {
                                    continue;
                                }
                                let Some(c) = pal.get(ci) else { continue };
                                let o = ((ty * TILE + py) * 16 + tx * TILE + px) * 4;
                                out[o] = c[0];
                                out[o + 1] = c[1];
                                out[o + 2] = c[2];
                                out[o + 3] = 255;
                            }
                        }
                    }
                }
            }
        }
        let cache = Arc::new(MetatileCache { tiles });
        self.metatiles.lock().unwrap().insert(key, cache.clone());
        Some(cache)
    }

    // ---- regions and chunks --------------------------------------------------------

    /// Composite the map-local pixel rectangle `region` of map `id` into
    /// `dst` at (dst_x, dst_y). Pixels outside the map are left untouched.
    pub fn render_map_region(&self, id: MapId, region: IRect, opts: RenderOpts, dst: &mut Pixmap, dst_x: i32, dst_y: i32) {
        let Some(m) = self.pack.map(id) else { return };
        let Some(blocks) = self.pack.blocks_of(id) else { return };
        let (mw, mh) = map_px_size(&self.pack.geom, m);
        let region = region.intersect(&IRect::from_size(0, 0, mw, mh));
        if region.is_empty() {
            return;
        }
        let bp = self.pack.geom.block_px as i32;
        let bx0 = region.x0.div_euclid(bp);
        let by0 = region.y0.div_euclid(bp);
        let bx1 = (region.x1 - 1).div_euclid(bp);
        let by1 = (region.y1 - 1).div_euclid(bp);
        match self.pack.gen {
            3 => {
                let Some(cache) = self.metatiles_for(m) else { return };
                for by in by0..=by1 {
                    for bx in bx0..=bx1 {
                        let Some(&v) = blocks.get((by as u32 * m.w + bx as u32) as usize) else { continue };
                        let mt = (v & 0x3FF) as usize;
                        let Some(px) = cache.tiles.get(mt) else { continue };
                        let ox = bx * bp - region.x0 + dst_x;
                        let oy = by * bp - region.y0 + dst_y;
                        blit_block(dst, ox, oy, px, 16, dst_x, dst_y, region.width(), region.height());
                    }
                }
            }
            _ => {
                let Some(sheet) = self.sheet_for(m, opts) else { return };
                let TilesetRef::Gen12 { file } = &m.tileset else { return };
                let Some(Blockset::Gen12(blockset)) = self.pack.blocksets.get(file) else { return };
                let tiles_per_row = sheet.w / TILE;
                for by in by0..=by1 {
                    for bx in bx0..=bx1 {
                        let Some(&v) = blocks.get((by as u32 * m.w + bx as u32) as usize) else { continue };
                        let Some(block) = blockset.get(v as usize) else { continue };
                        for ty in 0..4usize {
                            for tx in 0..4usize {
                                let t = block[ty * 4 + tx] as usize;
                                let sx0 = (t % tiles_per_row) * TILE;
                                let sy0 = (t / tiles_per_row) * TILE;
                                let ox = bx * bp + (tx * TILE) as i32 - region.x0 + dst_x;
                                let oy = by * bp + (ty * TILE) as i32 - region.y0 + dst_y;
                                blit_sheet_tile(dst, ox, oy, &sheet, sx0, sy0, dst_x, dst_y, region.width(), region.height());
                            }
                        }
                    }
                }
            }
        }
    }

    /// Composite a world-pixel rectangle of a scope at native scale.
    pub fn render_region(&self, scope: Scope, rect: IRect, opts: RenderOpts) -> Pixmap {
        let mut dst = Pixmap::filled(rect.width().max(0) as usize, rect.height().max(0) as usize, BACKGROUND);
        self.render_region_into(scope, rect, opts, &mut dst, 0, 0);
        dst
    }

    fn render_region_into(&self, scope: Scope, rect: IRect, opts: RenderOpts, dst: &mut Pixmap, dst_x: i32, dst_y: i32) {
        match scope {
            Scope::World => {
                for (mrect, id) in &self.pack.layout.placed_rects {
                    let inter = mrect.intersect(&rect);
                    if inter.is_empty() {
                        continue;
                    }
                    let local = inter.translate(-mrect.x0, -mrect.y0);
                    self.render_map_region(*id, local, opts, dst, inter.x0 - rect.x0 + dst_x, inter.y0 - rect.y0 + dst_y);
                }
            }
            Scope::Map(id) => {
                let sr = scope_rect(&self.pack, scope);
                let inter = sr.intersect(&rect);
                if !inter.is_empty() {
                    self.render_map_region(id, inter, opts, dst, inter.x0 - rect.x0 + dst_x, inter.y0 - rect.y0 + dst_y);
                }
            }
        }
    }

    /// Does any map of the scope intersect the world rectangle?
    pub fn covers(&self, scope: Scope, rect: &IRect) -> bool {
        match scope {
            Scope::World => self.pack.layout.placed_rects.iter().any(|(r, _)| r.intersects(rect)),
            Scope::Map(_) => scope_rect(&self.pack, scope).intersects(rect),
        }
    }

    /// A 512x512 chunk at LOD `level` (chunk (cx, cy) covers
    /// 512·2^level world px starting at (cx, cy) × that size).
    pub fn render_chunk(&self, scope: Scope, level: u8, cx: u32, cy: u32, opts: RenderOpts) -> Pixmap {
        let f = 1i32 << level;
        let span = CHUNK_PX * f;
        let origin = (cx as i32 * span, cy as i32 * span);
        let mut out = Pixmap::filled(CHUNK_PX as usize, CHUNK_PX as usize, BACKGROUND);
        if level == 0 {
            self.render_region_into(scope, IRect::from_size(origin.0, origin.1, CHUNK_PX, CHUNK_PX), opts, &mut out, 0, 0);
            return out;
        }
        let sub = (CHUNK_PX / f) as usize;
        let mut scratch = Pixmap::new(CHUNK_PX as usize, CHUNK_PX as usize);
        for sy in 0..f {
            for sx in 0..f {
                let r = IRect::from_size(origin.0 + sx * CHUNK_PX, origin.1 + sy * CHUNK_PX, CHUNK_PX, CHUNK_PX);
                if !self.covers(scope, &r) {
                    continue;
                }
                scratch.fill(BACKGROUND);
                self.render_region_into(scope, r, opts, &mut scratch, 0, 0);
                out.downsample_into(sx as usize * sub, sy as usize * sub, &scratch, f as usize);
            }
        }
        out
    }

    /// The whole scope at native scale (goldens, export).
    pub fn render_scope(&self, scope: Scope, opts: RenderOpts) -> Pixmap {
        self.render_region(scope, scope_rect(&self.pack, scope), opts)
    }
}

/// Copy a 16x16 RGBA block with clipping to the destination window
/// (win_x, win_y, win_w, win_h) and the image bounds.
#[allow(clippy::too_many_arguments)]
fn blit_block(dst: &mut Pixmap, ox: i32, oy: i32, px: &[u8], size: i32, win_x: i32, win_y: i32, win_w: i32, win_h: i32) {
    for y in 0..size {
        let dy = oy + y;
        if dy < win_y || dy >= win_y + win_h || dy < 0 || dy >= dst.h as i32 {
            continue;
        }
        let x_start = (win_x - ox).max(0).max(-ox);
        let x_end = size.min(win_x + win_w - ox).min(dst.w as i32 - ox);
        if x_end <= x_start {
            continue;
        }
        let si = (y * size + x_start) as usize * 4;
        let di = ((dy as usize) * dst.w + (ox + x_start) as usize) * 4;
        let n = (x_end - x_start) as usize * 4;
        dst.data[di..di + n].copy_from_slice(&px[si..si + n]);
    }
}

/// Copy one 8x8 tile from a recoloured sheet with clipping.
#[allow(clippy::too_many_arguments)]
fn blit_sheet_tile(dst: &mut Pixmap, ox: i32, oy: i32, sheet: &RgbaSheet, sx0: usize, sy0: usize, win_x: i32, win_y: i32, win_w: i32, win_h: i32) {
    let size = TILE as i32;
    for y in 0..size {
        let dy = oy + y;
        if dy < win_y || dy >= win_y + win_h || dy < 0 || dy >= dst.h as i32 {
            continue;
        }
        let x_start = (win_x - ox).max(0).max(-ox);
        let x_end = size.min(win_x + win_w - ox).min(dst.w as i32 - ox);
        if x_end <= x_start {
            continue;
        }
        let si = ((sy0 + y as usize) * sheet.w + sx0 + x_start as usize) * 4;
        let di = ((dy as usize) * dst.w + (ox + x_start) as usize) * 4;
        let n = (x_end - x_start) as usize * 4;
        if si + n > sheet.data.len() {
            continue;
        }
        dst.data[di..di + n].copy_from_slice(&sheet.data[si..si + n]);
    }
}
