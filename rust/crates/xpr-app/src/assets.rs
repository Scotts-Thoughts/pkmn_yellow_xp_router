//! Image assets: Pokémon HOME icons (`pkmn_icon.py`), box art (`box_art.py`),
//! filter-bar icons (greyscale, alpha preserved) and the quick-add category
//! icons. All of them are compiled into the binary by `build.rs` (the
//! Pokémon icons and box art prescaled) and decoded lazily into egui
//! textures.

use std::collections::HashMap;

use egui::{ColorImage, TextureHandle, TextureOptions};
use serde_json::Value;

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded_assets.rs"));
}

#[derive(Default)]
pub struct Assets {
    dex_lookup: Option<HashMap<String, i64>>,
    textures: HashMap<String, Option<TextureHandle>>,
}

impl Assets {
    pub fn new() -> Assets {
        Assets::default()
    }

    fn ensure_lookup(&mut self) {
        if self.dex_lookup.is_some() {
            return;
        }
        let mut map = HashMap::new();
        match serde_json::from_str::<Value>(embedded::DEX_LOOKUP) {
            Ok(Value::Object(obj)) => {
                for (k, v) in obj {
                    if let Some(n) = v.as_i64() {
                        map.insert(k, n);
                    }
                }
            }
            _ => log::warn!("Could not parse dex_lookup.json"),
        }
        self.dex_lookup = Some(map);
    }

    fn load_texture(&mut self, ctx: &egui::Context, key: &str, bytes: &[u8], grayscale: bool) -> Option<TextureHandle> {
        if let Some(t) = self.textures.get(key) {
            return t.clone();
        }
        let tex = load_png(bytes, grayscale).map(|img| ctx.load_texture(key.to_string(), img, TextureOptions::LINEAR));
        self.textures.insert(key.to_string(), tex.clone());
        tex
    }

    fn icon_bytes(rel: &str) -> Option<&'static [u8]> {
        embedded::ICONS.iter().find(|(p, _)| *p == rel).map(|(_, b)| *b)
    }

    /// `pkmn_icon.get_icon(species)`: the HOME icon (scaled at draw time).
    pub fn pkmn_icon(&mut self, ctx: &egui::Context, species: &str) -> Option<TextureHandle> {
        self.ensure_lookup();
        let dex = *self.dex_lookup.as_ref()?.get(species)? as u32;
        let bytes = embedded::PKMN_ICONS.iter().find(|(n, _)| *n == dex).map(|(_, b)| *b)?;
        self.load_texture(ctx, &format!("pkmn:{}", dex), bytes, false)
    }

    /// `box_art.get_box_art(version)`
    pub fn box_art(&mut self, ctx: &egui::Context, version: &str) -> Option<TextureHandle> {
        let bytes = embedded::BOX_ART.iter().find(|(v, _)| *v == version).map(|(_, b)| *b)?;
        self.load_texture(ctx, &format!("boxart:{}", version), bytes, false)
    }

    /// The greyscale filter-bar icon for an event-type constant name.
    pub fn filter_icon(&mut self, ctx: &egui::Context, file_stem: &str) -> Option<TextureHandle> {
        let bytes = Assets::icon_bytes(&format!("filter icons/{}.png", file_stem))?;
        self.load_texture(ctx, &format!("filter:{}", file_stem), bytes, true)
    }

    /// The colour rare-candy icon of the controls bar.
    pub fn candy_icon(&mut self, ctx: &egui::Context) -> Option<TextureHandle> {
        let bytes = Assets::icon_bytes("filter icons/TASK_RARE_CANDY.png")?;
        self.load_texture(ctx, "candy", bytes, false)
    }

    /// Quick-add category icon (`trainer.png`, `item.png`, ...).
    pub fn category_icon(&mut self, ctx: &egui::Context, file_name: &str) -> Option<TextureHandle> {
        let bytes = Assets::icon_bytes(file_name)?;
        self.load_texture(ctx, &format!("cat:{}", file_name), bytes, false)
    }
}

/// Decode a PNG into an egui image; `grayscale` reproduces the filter bar's
/// `Format_Grayscale8` conversion with the original alpha kept.
fn load_png(bytes: &[u8], grayscale: bool) -> Option<ColorImage> {
    let img = image::load_from_memory(bytes).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut pixels: Vec<egui::Color32> = Vec::with_capacity((w * h) as usize);
    for p in rgba.pixels() {
        let [r, g, b, a] = p.0;
        if grayscale {
            // Qt's Grayscale8 uses the luma weights 0.299/0.587/0.114.
            let y = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32).round().clamp(0.0, 255.0) as u8;
            pixels.push(egui::Color32::from_rgba_unmultiplied(y, y, y, a));
        } else {
            pixels.push(egui::Color32::from_rgba_unmultiplied(r, g, b, a));
        }
    }
    Some(ColorImage { size: [w as usize, h as usize], pixels, source_size: egui::Vec2::new(w as f32, h as f32) })
}

/// Draw a texture scaled to fit `size` (keeping aspect ratio), centred.
pub fn draw_fit(ui: &mut egui::Ui, tex: &TextureHandle, size: f32) -> egui::Response {
    let [w, h] = tex.size();
    let (w, h) = (w as f32, h as f32);
    let scale = (size / w).min(size / h);
    let draw_size = egui::Vec2::new(w * scale, h * scale);
    let (rect, resp) = ui.allocate_exact_size(egui::Vec2::splat(size), egui::Sense::hover());
    let img_rect = egui::Rect::from_center_size(rect.center(), draw_size);
    ui.painter().image(tex.id(), img_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_tables_are_complete() {
        assert_eq!(embedded::PKMN_ICONS.len(), 649);
        let lookup: Value = serde_json::from_str(embedded::DEX_LOOKUP).unwrap();
        assert!(lookup.as_object().unwrap().len() > 600);
        for version in xpr_core::consts::VERSION_LIST {
            assert!(embedded::BOX_ART.iter().any(|(v, _)| *v == version), "no box art for {}", version);
        }
        for icon in ["trainer.png", "item.png", "misc.png", "moves.png", "wild.png", "filter icons/TASK_RARE_CANDY.png", "filter icons/ERROR_SEARCH.png"] {
            assert!(Assets::icon_bytes(icon).is_some(), "missing {}", icon);
        }
        assert!(load_png(embedded::PKMN_ICONS[0].1, false).is_some());
        assert!(load_png(Assets::icon_bytes("filter icons/TASK_HEAL.png").unwrap(), true).is_some());
    }
}
