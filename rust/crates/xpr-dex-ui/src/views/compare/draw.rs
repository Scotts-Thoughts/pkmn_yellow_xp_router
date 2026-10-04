//! Small drawing helpers of the comparison views. Several are copies of
//! private helpers of the Pokédex module (`pokedex::common`), which the
//! comparison views cannot reach.

use egui::{Align, Color32, FontId, Painter, Pos2, Rect, TextureHandle, Ui, UiBuilder, Vec2};
use xpr_ui_kit::theme::Theme;

use crate::images::{DexImages, SpriteSize};
use crate::palette;
use crate::widgets::text_w;

pub const GREEN_400: Color32 = palette::GREEN_400;
pub const RED_400: Color32 = palette::RED_400;

/// Run `f` in a child `Ui` confined to `rect` (left-to-right, centred)
/// without moving the parent's cursor.
pub fn in_rect<R>(
    ui: &mut Ui,
    rect: Rect,
    salt: impl std::hash::Hash,
    f: impl FnOnce(&mut Ui) -> R,
) -> R {
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .id_salt(salt)
            .layout(egui::Layout::left_to_right(Align::Center)),
    );
    f(&mut child)
}

/// A text job with extra letter spacing (Tailwind's `tracking-*`).
pub fn spaced_galley(
    ui: &Ui,
    text: &str,
    font: FontId,
    color: Color32,
    spacing: f32,
    wrap_width: f32,
) -> std::sync::Arc<egui::Galley> {
    let fmt = egui::TextFormat {
        font_id: font,
        extra_letter_spacing: spacing,
        color,
        ..Default::default()
    };
    let mut job = egui::text::LayoutJob::single_section(text.to_string(), fmt);
    job.wrap.max_width = wrap_width;
    ui.fonts_mut(|f| f.layout_job(job))
}

/// Solodex's bold gray uppercase section heading (`text-xs font-bold
/// text-gray-600 uppercase tracking-widest`), centred on `cx`, its top at `y`
/// (the line is 16 px tall).
pub fn centered_heading(ui: &Ui, theme: &Theme, cx: f32, y: f32, text: &str) {
    let g = spaced_galley(
        ui,
        &text.to_uppercase(),
        palette::px_bold(theme, 12.0),
        palette::GRAY_600,
        1.2,
        f32::INFINITY,
    );
    // the last letter's trailing spacing is part of the box, as in CSS
    let pos = Pos2::new(cx - g.size().x / 2.0, y);
    ui.painter().galley(pos, g, palette::GRAY_600);
}

/// Centred, wrapped (possibly multi-line) text whose block starts at `y`,
/// centred on `cx`; returns the block's height.
pub fn centered_text(
    ui: &Ui,
    cx: f32,
    y: f32,
    text: &str,
    font: FontId,
    color: Color32,
    wrap_w: f32,
) -> f32 {
    let mut job = egui::text::LayoutJob::single_section(
        text.to_string(),
        egui::TextFormat {
            font_id: font,
            color,
            ..Default::default()
        },
    );
    job.wrap.max_width = wrap_w;
    job.halign = Align::Center;
    let g = ui.fonts_mut(|f| f.layout_job(job));
    let origin = Pos2::new(cx - g.rect.center().x, y);
    let h = g.size().y;
    ui.painter().galley(origin, g, color);
    h
}

/// Height of [`centered_text`] without drawing.
pub fn centered_text_height(ui: &Ui, text: &str, font: FontId, wrap_w: f32) -> f32 {
    let mut job = egui::text::LayoutJob::single_section(
        text.to_string(),
        egui::TextFormat {
            font_id: font,
            color: Color32::WHITE,
            ..Default::default()
        },
    );
    job.wrap.max_width = wrap_w;
    job.halign = Align::Center;
    ui.fonts_mut(|f| f.layout_job(job)).size().y
}

/// Width of the widest line of `text` (no wrapping).
pub fn text_width(ui: &Ui, text: &str, font: &FontId) -> f32 {
    text_w(ui, text, font)
}

/// Paint a sprite fitted into `rect` with Solodex's `pokemon-icon-stroke`
/// (a black 1 px outline made of four offset black copies).
pub fn paint_outlined(painter: &Painter, tex: &TextureHandle, rect: Rect, tint: Color32) {
    let [w, h] = tex.size();
    let scale = (rect.width() / w as f32).min(rect.height() / h as f32);
    let r = Rect::from_center_size(rect.center(), Vec2::new(w as f32 * scale, h as f32 * scale));
    let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
    let black = Color32::from_black_alpha(tint.a());
    for d in [
        Vec2::new(1.0, 0.0),
        Vec2::new(-1.0, 0.0),
        Vec2::new(0.0, 1.0),
        Vec2::new(0.0, -1.0),
    ] {
        painter.image(tex.id(), r.translate(d), uv, black);
    }
    painter.image(tex.id(), r, uv, tint);
}

/// A species' small sprite painted (outlined) into `rect`.
pub fn paint_small_sprite(ui: &Ui, images: &mut DexImages, species: &str, dex: i32, rect: Rect) {
    let Some(tex) = images.sprite(ui.ctx(), species, dex, SpriteSize::Small) else {
        return;
    };
    paint_outlined(ui.painter(), &tex, rect, Color32::WHITE);
}

/// `+n` / an em dash / `-n` of a difference (Solodex's diff cells).
pub fn diff_text(diff: i64) -> String {
    if diff > 0 {
        format!("+{}", diff)
    } else if diff == 0 {
        "\u{2014}".to_string()
    } else {
        diff.to_string()
    }
}

/// Colour of a difference: green better, red worse, gray equal.
pub fn diff_color(diff: i64) -> Color32 {
    if diff > 0 {
        GREEN_400
    } else if diff < 0 {
        RED_400
    } else {
        palette::GRAY_600
    }
}
