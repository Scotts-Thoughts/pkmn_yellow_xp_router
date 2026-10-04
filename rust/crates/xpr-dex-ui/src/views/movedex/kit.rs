//! Small drawing helpers private to the Movedex: the on/off switch, method
//! chips, wrapped chip layout and Solodex's translucent Tailwind colours.

use egui::{Align, Align2, Color32, CornerRadius, Layout, Pos2, Rect, Sense, Stroke, Ui, UiBuilder, Vec2};
use xpr_ui_kit::theme::Theme;

use super::data::{method_kind, MethodKind};
use crate::palette::{self, hex};
use crate::widgets::text_w;

/// A Tailwind colour at `alpha` (0-1), e.g. `bg-green-900/50`.
pub fn tw(hex_color: &str, alpha: f32) -> Color32 {
    let c = hex(hex_color);
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (alpha * 255.0).round() as u8)
}

/// Table / panel rule colour (`border-gray-800`).
pub const RULE: Color32 = palette::GRAY_800;

/// A horizontal rule at the top edge of `rect`.
pub fn rule_top(ui: &Ui, rect: Rect, color: Color32) {
    ui.painter().hline(rect.x_range(), rect.min.y + 0.5, Stroke::new(1.0_f32, color));
}

/// A horizontal rule at the bottom edge of `rect`.
pub fn rule_bottom(ui: &Ui, rect: Rect, color: Color32) {
    ui.painter().hline(rect.x_range(), rect.max.y - 0.5, Stroke::new(1.0_f32, color));
}

/// Solodex's toggle switch (`w-7 h-4`, blue when on, gray-600 when off) with
/// its label to the right; both are clickable. Returns true when toggled.
pub fn switch(ui: &mut Ui, theme: &Theme, on: &mut bool, label: &str) -> bool {
    let font = palette::px(theme, 12.0);
    let label_w = text_w(ui, label, &font);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(28.0 + 8.0 + label_w, 16.0), Sense::click());
    let track = Rect::from_min_size(rect.min, Vec2::new(28.0, 16.0));
    ui.painter().rect_filled(track, CornerRadius::same(8), if *on { palette::BLUE_500 } else { palette::GRAY_600 });
    let knob_x = if *on { track.min.x + 14.0 } else { track.min.x + 2.0 };
    ui.painter().rect_filled(Rect::from_min_size(Pos2::new(knob_x, track.min.y + 2.0), Vec2::splat(12.0)), CornerRadius::same(6), Color32::WHITE);
    ui.painter().text(Pos2::new(track.max.x + 8.0, rect.center().y), Align2::LEFT_CENTER, label, font, if resp.hovered() { Color32::WHITE } else { palette::GRAY_300 });
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if resp.clicked() {
        *on = !*on;
        return true;
    }
    false
}

/// A child `Ui` over `rect` whose layout runs left to right, with its
/// widgets aligned to `align` vertically.
pub fn row_ui(ui: &mut Ui, rect: Rect, spacing: f32, align: Align) -> Ui {
    let mut c = ui.new_child(UiBuilder::new().id_salt(("movedex_row_ui", rect.min.x as i32, rect.min.y as i32)).max_rect(rect).layout(Layout::left_to_right(align)));
    c.spacing_mut().item_spacing = Vec2::new(spacing, 0.0);
    c
}

/// A full-width strip `h` high whose content (`w` wide) is centred in it.
pub fn centered_strip(ui: &mut Ui, w: f32, h: f32, spacing: f32, align: Align, add: impl FnOnce(&mut Ui)) {
    let (strip, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
    let w = w.min(strip.width());
    let inner = Rect::from_center_size(strip.center(), Vec2::new(w, h));
    let mut c = row_ui(ui, inner, spacing, align);
    add(&mut c);
}

// ---------------------------------------------------------------------------
// method chips
// ---------------------------------------------------------------------------

pub const CHIP_H: f32 = 20.0;
pub const CHIP_GAP: f32 = 4.0;

/// (fill, text) of a method chip.
pub fn chip_colors(label: &str) -> (Color32, Color32) {
    match method_kind(label) {
        MethodKind::Level => (tw("#14532d", 0.5), hex("#86efac")),
        MethodKind::Tm => (tw("#1e3a8a", 0.5), hex("#93c5fd")),
        MethodKind::Egg => (tw("#713f12", 0.5), hex("#fde047")),
        MethodKind::Other => (tw("#581c87", 0.5), hex("#d8b4fe")),
    }
}

pub fn chip_width(ui: &Ui, theme: &Theme, label: &str) -> f32 {
    text_w(ui, label, &palette::px(theme, 12.0)) + 12.0
}

/// Paint a method chip with its top-left at `pos`.
pub fn paint_chip(ui: &Ui, theme: &Theme, pos: Pos2, w: f32, label: &str) {
    let (fill, fg) = chip_colors(label);
    let r = Rect::from_min_size(pos, Vec2::new(w, CHIP_H));
    ui.painter().rect_filled(r, CornerRadius::same(4), fill);
    ui.painter().text(r.center(), Align2::CENTER_CENTER, label, palette::px(theme, 12.0), fg);
}

/// Where wrapped chips of the given widths land in a box `avail` wide
/// (`flex flex-wrap gap-1`): offsets from the top-left, and the total height.
pub fn flow(widths: &[f32], avail: f32) -> (Vec<Vec2>, f32) {
    let mut out = Vec::with_capacity(widths.len());
    let (mut x, mut y) = (0.0_f32, 0.0_f32);
    for w in widths {
        if x > 0.0 && x + w > avail {
            x = 0.0;
            y += CHIP_H + CHIP_GAP;
        }
        out.push(Vec2::new(x, y));
        x += w + CHIP_GAP;
    }
    (out, if widths.is_empty() { 0.0 } else { y + CHIP_H })
}

/// Index range of the rows of a table (`offsets[i]` = top of row `i`,
/// `offsets[n]` = total height) that intersect `[top, bottom]`.
pub fn visible_rows(offsets: &[f32], top: f32, bottom: f32) -> std::ops::Range<usize> {
    let n = offsets.len().saturating_sub(1);
    let first = offsets.partition_point(|o| *o <= top).saturating_sub(1).min(n);
    let last = offsets.partition_point(|o| *o < bottom).min(n);
    first..last.max(first)
}
