//! A texture atlas of overworld sprite frames (SPEC §3.4): every frame the
//! viewer needs is rendered once by `xpr_map::sprites`, packed into a
//! 1024×1024 page with a shelf packer and uploaded with `set_partial`, so
//! all sprite markers of a frame share one texture and draw as one mesh.

use std::collections::HashMap;

use egui::{Color32, ColorImage, Rect, TextureHandle, TextureId, TextureOptions};
use xpr_map::sprites::{frame_key, render_frame, FrameKey, SpriteCache};
use xpr_map::{MapObject, MapPack, Pixmap};

const PAGE: usize = 1024;
const PAD: usize = 1;

#[derive(Clone, Copy, Debug)]
pub struct AtlasFrame {
    pub tex: TextureId,
    pub uv: Rect,
    pub w: u32,
    pub h: u32,
}

struct Shelf {
    y: usize,
    h: usize,
    next_x: usize,
}

struct Page {
    tex: TextureHandle,
    shelves: Vec<Shelf>,
    next_y: usize,
}

#[derive(Default)]
pub struct SpriteAtlas {
    pages: Vec<Page>,
    frames: HashMap<FrameKey, Option<AtlasFrame>>,
    cache: SpriteCache,
}

fn atlas_options() -> TextureOptions {
    TextureOptions { magnification: egui::TextureFilter::Nearest, minification: egui::TextureFilter::Linear, wrap_mode: egui::TextureWrapMode::ClampToEdge, mipmap_mode: None }
}

impl SpriteAtlas {
    pub fn clear(&mut self) {
        self.pages.clear();
        self.frames.clear();
        self.cache.clear();
    }

    pub fn frame_count(&self) -> usize {
        self.frames.values().filter(|f| f.is_some()).count()
    }

    /// The atlas frame of an object (rendered and uploaded on first use).
    pub fn frame(&mut self, ctx: &egui::Context, pack: &MapPack, obj: &MapObject, night: bool) -> Option<AtlasFrame> {
        let key = frame_key(pack, obj, night)?;
        if let Some(f) = self.frames.get(&key) {
            return *f;
        }
        let entry = render_frame(pack, &mut self.cache, obj, night).and_then(|px| self.insert(ctx, &px));
        self.frames.insert(key, entry);
        entry
    }

    fn insert(&mut self, ctx: &egui::Context, px: &Pixmap) -> Option<AtlasFrame> {
        if px.w == 0 || px.h == 0 || px.w + 2 * PAD > PAGE || px.h + 2 * PAD > PAGE {
            return None;
        }
        let (w, h) = (px.w + PAD, px.h + PAD);
        // find a shelf of the right height with room, else open a shelf, else a page
        let mut place: Option<(usize, usize, usize)> = None; // (page, x, y)
        for (pi, page) in self.pages.iter_mut().enumerate() {
            if let Some(s) = page.shelves.iter_mut().find(|s| s.h == h && s.next_x + w <= PAGE) {
                place = Some((pi, s.next_x, s.y));
                s.next_x += w;
                break;
            }
            if page.next_y + h <= PAGE {
                let y = page.next_y;
                page.shelves.push(Shelf { y, h, next_x: w });
                page.next_y += h;
                place = Some((pi, 0, y));
                break;
            }
        }
        let (pi, x, y) = match place {
            Some(p) => p,
            None => {
                let tex = ctx.load_texture(format!("map-sprites-{}", self.pages.len()), ColorImage::filled([PAGE, PAGE], Color32::TRANSPARENT), atlas_options());
                self.pages.push(Page { tex, shelves: vec![Shelf { y: 0, h, next_x: w }], next_y: h });
                (self.pages.len() - 1, 0, 0)
            }
        };
        let page = &mut self.pages[pi];
        let image = ColorImage::from_rgba_unmultiplied([px.w, px.h], &px.data);
        page.tex.set_partial([x, y], image, atlas_options());
        let s = PAGE as f32;
        Some(AtlasFrame {
            tex: page.tex.id(),
            uv: Rect::from_min_max(egui::pos2(x as f32 / s, y as f32 / s), egui::pos2((x + px.w) as f32 / s, (y + px.h) as f32 / s)),
            w: px.w as u32,
            h: px.h as u32,
        })
    }
}
