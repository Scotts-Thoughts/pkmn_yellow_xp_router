//! Overworld sprites (a port of pokemap's `src/render/sprite-renderer.js`).
//!
//! Gen 1/2 sheets are 16 px wide grayscale strips: 16 px tall for a still
//! sprite, else 96 px with the standing frames at y = 0 (down), 16 (up),
//! 32 (left; right is the mirror). Pixels are 4 shades (from the red
//! channel); shade 0 is transparent and shades 1–3 take a palette: gen 1
//! uses the map's CGB palette through the OBP0 remap, gen 2 the
//! `PAL_NPC_*` palette of the map's time of day.
//!
//! Gen 3 sheets are full-colour rows of `w × h` frames (0 down, 1 up, 2
//! left, right mirrored; berry trees use the last, grown frame) with an
//! opaque key colour in Ruby/Sapphire/Emerald that is removed.

use std::collections::HashMap;
use std::sync::Arc;

use crate::lod::Pixmap;
use crate::model::*;
use crate::pack::MapPack;

pub enum SheetPixels {
    /// gen 1/2: shade 0..3 per pixel
    Shades(Vec<u8>),
    /// gen 3: RGBA
    Rgba(Vec<u8>),
}

pub struct SpriteSheet {
    pub w: usize,
    pub h: usize,
    pub px: SheetPixels,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dir {
    Down,
    Up,
    Left,
    Right,
}

impl Dir {
    pub fn from_facing(f: Option<Facing>) -> Dir {
        match f {
            Some(Facing::Up) => Dir::Up,
            Some(Facing::Left) => Dir::Left,
            Some(Facing::Right) => Dir::Right,
            _ => Dir::Down,
        }
    }
}

/// What identifies one rendered frame (for a texture atlas).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FrameKey {
    pub sprite: String,
    pub dir: Dir,
    /// palette variant ("" for gen 3)
    pub pal: String,
}

pub fn decode_sheet(gen: u8, bytes: &[u8]) -> Result<SpriteSheet, String> {
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?.to_rgba8();
    let (w, h) = img.dimensions();
    let (w, h) = (w as usize, h as usize);
    if gen == 3 {
        Ok(SpriteSheet { w, h, px: SheetPixels::Rgba(img.into_raw()) })
    } else {
        let shades = img.pixels().map(|p| { let r = p.0[0]; if r >= 192 { 0 } else if r >= 128 { 1 } else if r >= 64 { 2 } else { 3 } }).collect();
        Ok(SpriteSheet { w, h, px: SheetPixels::Shades(shades) })
    }
}

const GRAY_FALLBACK: Pal4 = [[255, 255, 255], [192, 192, 192], [96, 96, 96], [0, 0, 0]];

/// A frame with transparency (gen 1/2: 16x16; gen 3: the sprite's `w x h`).
pub fn extract_frame(_gen: u8, sheet: &SpriteSheet, meta: &SpriteMeta, dir: Dir, palette: Option<&Pal4>, berry: bool) -> Option<Pixmap> {
    match &sheet.px {
        SheetPixels::Shades(shades) => {
            let size = 16usize;
            if sheet.w < size || sheet.h < size {
                return None;
            }
            let still = sheet.h <= size;
            let src_y = if still { 0 } else { match dir { Dir::Down => 0, Dir::Up => 16, Dir::Left | Dir::Right => 32 } };
            if src_y + size > sheet.h {
                return None;
            }
            let flip = !still && dir == Dir::Right;
            let pal = palette.unwrap_or(&GRAY_FALLBACK);
            let mut out = Pixmap::new(size, size);
            for y in 0..size {
                for x in 0..size {
                    let sx = if flip { size - 1 - x } else { x };
                    let shade = shades[(src_y + y) * sheet.w + sx] as usize;
                    if shade == 0 {
                        continue;
                    }
                    let c = pal[shade.min(3)];
                    out.put(x, y, [c[0], c[1], c[2], 255]);
                }
            }
            Some(out)
        }
        SheetPixels::Rgba(rgba) => {
            let (fw, fh) = (meta.w.max(1) as usize, meta.h.max(1) as usize);
            if sheet.w < fw || sheet.h < fh {
                return None;
            }
            let frames = sheet.w / fw;
            let mut idx = if berry { frames.saturating_sub(1) } else { match dir { Dir::Down => 0, Dir::Up => 1, Dir::Left | Dir::Right => 2 } };
            if frames <= 1 || idx >= frames {
                idx = 0;
            }
            let flip = !berry && dir == Dir::Right;
            let sx0 = idx * fw;
            let mut out = Pixmap::new(fw, fh);
            for y in 0..fh {
                for x in 0..fw {
                    let sx = if flip { fw - 1 - x } else { x };
                    let i = (y * sheet.w + sx0 + sx) * 4;
                    out.put(x, y, [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]);
                }
            }
            // chroma key: an opaque top-left pixel of the frame (as extracted, i.e. before the flip
            // would matter — the key colour is uniform) is the background colour
            let key = {
                let i = (sx0) * 4;
                if rgba[i + 3] == 255 { Some([rgba[i], rgba[i + 1], rgba[i + 2]]) } else { None }
            };
            if let Some(k) = key {
                for px in out.data.chunks_exact_mut(4) {
                    if (px[0] as i32 - k[0] as i32).abs() <= 30 && (px[1] as i32 - k[1] as i32).abs() <= 30 && (px[2] as i32 - k[2] as i32).abs() <= 30 {
                        px[3] = 0;
                    }
                }
            }
            Some(out)
        }
    }
}

/// gen 1: the map's CGB palette through the OBP0 remap (shade 1 → colour 0,
/// 2 → colour 1, 3 → colour 3).
pub fn gen1_sprite_palette(pack: &MapPack, map: MapId) -> Option<(Pal4, usize)> {
    let Palettes::Gen1(p) = &pack.palettes else { return None };
    let m = pack.map(map)?;
    let (cgb, idx) = match p.map_assignments.get(&m.const_name) {
        Some(a) => (a.colors, a.palette_index),
        None => (*p.cgb_palettes.first()?, 0),
    };
    Some(([cgb[0], cgb[0], cgb[1], cgb[3]], idx))
}

const GEN2_PAL_INDEX: [(&str, usize); 8] = [
    ("PAL_NPC_RED", 0),
    ("PAL_NPC_BLUE", 1),
    ("PAL_NPC_GREEN", 2),
    ("PAL_NPC_BROWN", 3),
    ("PAL_NPC_PINK", 4),
    ("PAL_NPC_EMOTE", 5),
    ("PAL_NPC_TREE", 6),
    ("PAL_NPC_ROCK", 7),
];

/// gen 2: the NPC palette for a `PAL_NPC_*` name at the map's time of day
/// (indoor maps use the day set; `night` switches outdoor maps to `nite`).
pub fn gen2_sprite_palette(pack: &MapPack, map: MapId, pal: Option<&str>, night: bool) -> Option<(Pal4, String)> {
    let Palettes::Gen2(p) = &pack.palettes else { return None };
    let m = pack.map(map)?;
    let base = match &m.palette {
        PaletteRef::Gen2 { tod, .. } => tod.as_str(),
        _ => "day",
    };
    let mut tod = if base == "indoor" { "day" } else { base };
    if night && base != "indoor" {
        tod = "nite";
    }
    let pals = p.npc_palettes.get(tod).or_else(|| p.npc_palettes.get("day"))?;
    let idx = pal.and_then(|n| GEN2_PAL_INDEX.iter().find(|(k, _)| *k == n).map(|(_, i)| *i)).unwrap_or(1);
    let chosen = pals.get(idx).or_else(|| pals.first())?;
    Some((*chosen, format!("{}:{}", pal.unwrap_or("default"), tod)))
}

/// Decoded sheets by file stem.
#[derive(Default)]
pub struct SpriteCache {
    sheets: HashMap<String, Option<Arc<SpriteSheet>>>,
}

impl SpriteCache {
    pub fn sheet(&mut self, pack: &MapPack, file: &str) -> Option<Arc<SpriteSheet>> {
        if let Some(s) = self.sheets.get(file) {
            return s.clone();
        }
        let decoded = pack.sprite_png.get(file).and_then(|b| match decode_sheet(pack.gen, b) {
            Ok(s) => Some(Arc::new(s)),
            Err(e) => {
                log::warn!("sprite {} failed to decode: {}", file, e);
                None
            }
        });
        self.sheets.insert(file.to_string(), decoded.clone());
        decoded
    }

    pub fn clear(&mut self) {
        self.sheets.clear();
    }
}

fn is_berry(sprite: &str) -> bool {
    sprite.starts_with("OBJ_EVENT_GFX_BERRY_TREE_")
}

/// The atlas key of an object's frame, if it has a sprite the pack knows.
pub fn frame_key(pack: &MapPack, obj: &MapObject, night: bool) -> Option<FrameKey> {
    let sprite = obj.sprite.as_deref()?;
    pack.sprites.get(sprite)?;
    let dir = if is_berry(sprite) { Dir::Down } else { Dir::from_facing(obj.facing) };
    let pal = match pack.gen {
        1 => format!("g1:{}", gen1_sprite_palette(pack, obj.map).map(|(_, i)| i).unwrap_or(0)),
        2 => gen2_sprite_palette(pack, obj.map, obj.pal.as_deref(), night).map(|(_, k)| k).unwrap_or_default(),
        _ => String::new(),
    };
    Some(FrameKey { sprite: sprite.to_string(), dir, pal })
}

/// Render an object's frame (transparent RGBA).
pub fn render_frame(pack: &MapPack, cache: &mut SpriteCache, obj: &MapObject, night: bool) -> Option<Pixmap> {
    let sprite = obj.sprite.as_deref()?;
    let meta = pack.sprites.get(sprite)?.clone();
    let sheet = cache.sheet(pack, &meta.file)?;
    let berry = is_berry(sprite);
    let dir = if berry { Dir::Down } else { Dir::from_facing(obj.facing) };
    let palette = match pack.gen {
        1 => gen1_sprite_palette(pack, obj.map).map(|(p, _)| p),
        2 => gen2_sprite_palette(pack, obj.map, obj.pal.as_deref(), night).map(|(p, _)| p),
        _ => None,
    };
    extract_frame(pack.gen, &sheet, &meta, dir, palette.as_ref(), berry)
}
