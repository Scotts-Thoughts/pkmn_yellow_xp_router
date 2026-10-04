//! Textures: HOME sprites (decoded from WebP on first use) and the move
//! category icons.

use std::collections::HashMap;

use egui::{Color32, ColorImage, Rect, TextureHandle, TextureOptions, Ui, Vec2};

static PHYSICAL: &[u8] = include_bytes!("../assets/move-physical.png");
static SPECIAL: &[u8] = include_bytes!("../assets/move-special.png");
static STATUS: &[u8] = include_bytes!("../assets/move-status.png");

/// Sprite texture sizes: lists draw at <= 32 px, cards at <= 128 px.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpriteSize {
    /// downscaled to 48 px (list rows, chips)
    Small,
    /// the source 128 px
    Full,
}

#[derive(Default)]
pub struct DexImages {
    sprites: HashMap<(u32, SpriteSize), Option<TextureHandle>>,
    icons: HashMap<&'static str, Option<TextureHandle>>,
}

impl DexImages {
    pub fn new() -> DexImages {
        DexImages::default()
    }

    /// A species' HOME sprite (`national_dex_number` for base forms; alternate
    /// forms are looked up by name).
    pub fn sprite(
        &mut self,
        ctx: &egui::Context,
        species: &str,
        national_dex_number: i32,
        size: SpriteSize,
    ) -> Option<TextureHandle> {
        let id = xpr_dex::sprites::sprite_id(species, national_dex_number);
        if let Some(t) = self.sprites.get(&(id, size)) {
            return t.clone();
        }
        let tex = xpr_dex::source::sprite(id)
            .and_then(|bytes| {
                decode(
                    &bytes,
                    if size == SpriteSize::Small {
                        Some(48)
                    } else {
                        None
                    },
                )
            })
            .map(|img| {
                ctx.load_texture(
                    format!("dex-sprite-{}-{:?}", id, size),
                    img,
                    TextureOptions::LINEAR,
                )
            });
        self.sprites.insert((id, size), tex.clone());
        tex
    }

    /// Like [`DexImages::sprite`] with the dex number taken from the species index.
    pub fn sprite_by_name(
        &mut self,
        ctx: &egui::Context,
        species: &str,
        size: SpriteSize,
    ) -> Option<TextureHandle> {
        let dex = xpr_dex::species_entry(species)
            .map(|e| e.national_dex_number)
            .unwrap_or(0);
        self.sprite(ctx, species, dex, size)
    }

    /// The Physical / Special / Status icon (60x40).
    pub fn category_icon(&mut self, ctx: &egui::Context, category: &str) -> Option<TextureHandle> {
        let (key, bytes): (&'static str, &[u8]) = match category {
            "Physical" => ("physical", PHYSICAL),
            "Special" => ("special", SPECIAL),
            "Status" => ("status", STATUS),
            _ => return None,
        };
        if let Some(t) = self.icons.get(key) {
            return t.clone();
        }
        let tex = decode(bytes, None)
            .map(|img| ctx.load_texture(format!("dex-cat-{}", key), img, TextureOptions::LINEAR));
        self.icons.insert(key, tex.clone());
        tex
    }
}

fn decode(bytes: &[u8], max_side: Option<u32>) -> Option<ColorImage> {
    let img = image::load_from_memory(bytes).ok()?;
    let img = match max_side {
        Some(m) if img.width().max(img.height()) > m => {
            img.resize(m, m, image::imageops::FilterType::Triangle)
        }
        _ => img,
    };
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Some(ColorImage::from_rgba_unmultiplied(
        [w as usize, h as usize],
        rgba.as_raw(),
    ))
}

/// Paint a texture fitted (aspect kept) and centred in `rect`.
pub fn paint_fit(ui: &Ui, tex: &TextureHandle, rect: Rect, tint: Color32) {
    let [w, h] = tex.size();
    let scale = (rect.width() / w as f32).min(rect.height() / h as f32);
    let r = Rect::from_center_size(rect.center(), Vec2::new(w as f32 * scale, h as f32 * scale));
    ui.painter().image(
        tex.id(),
        r,
        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        tint,
    );
}

/// Allocate a `size` square and draw the species sprite in it; returns the
/// response (hover / click) of the square.
pub fn sprite_widget(
    ui: &mut Ui,
    images: &mut DexImages,
    species: &str,
    national_dex_number: i32,
    size: f32,
    sense: egui::Sense,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), sense);
    let kind = if size <= 48.0 {
        SpriteSize::Small
    } else {
        SpriteSize::Full
    };
    if let Some(tex) = images.sprite(ui.ctx(), species, national_dex_number, kind) {
        paint_fit(ui, &tex, rect, Color32::WHITE);
    }
    resp
}

/// The category icon at `height` (width follows the 3:2 source).
pub fn category_icon_widget(
    ui: &mut Ui,
    images: &mut DexImages,
    category: &str,
    height: f32,
) -> egui::Response {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(height * 1.5, height), egui::Sense::hover());
    if let Some(tex) = images.category_icon(ui.ctx(), category) {
        // Solodex brightens the icons (filter: brightness(1.35))
        paint_fit(ui, &tex, rect, Color32::WHITE);
    }
    resp.on_hover_text(category)
}
