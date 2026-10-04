//! The right column: every trainer Pokémon ranked by speed (Solodex
//! `TrainerSpeedList.tsx`).

use egui::{Align2, Color32, CornerRadius, FontFamily, FontId, Id, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use xpr_ui_kit::theme::Theme;

use super::data::VersionIndex;
use crate::palette::{self, px, px_bold};
use crate::widgets;

const ROW_H: f32 = 29.0;

#[derive(Default)]
pub struct SpeedState {
    pub major_only: bool,
    pub r1_only: bool,
    /// speed entries after the filters, with the filter state they were built for
    filtered: Vec<usize>,
    built_for: Option<(usize, bool, bool)>,
}

fn tag_button(ui: &mut Ui, theme: &Theme, rect: Rect, id: Id, text: &str, on: bool, on_fg: Color32, on_bg: Color32) -> bool {
    let resp = ui.interact(rect, id, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    let (bg, fg) = if on {
        (on_bg, on_fg)
    } else if resp.hovered() {
        (palette::GRAY_800, palette::GRAY_400)
    } else {
        (palette::GRAY_800, palette::GRAY_500)
    };
    ui.painter().rect_filled(rect, CornerRadius::same(4), bg);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, px(theme, 12.0), fg);
    resp.clicked()
}

/// The column in `ui`; returns the trainer clicked (an index into the
/// version's trainers).
pub fn speed_ui(ui: &mut Ui, theme: &Theme, ix: &VersionIndex, st: &mut SpeedState) -> Option<usize> {
    let mut picked = None;
    let full = ui.available_rect_before_wrap();
    let w = full.width();
    ui.painter().rect_filled(full, CornerRadius::ZERO, palette::GRAY_900);

    // ---- title bar: "Speed Rankings" | Major Battles | R1, spread like justify-between
    let bar_h = 36.0;
    let title_font = px_bold(theme, 12.0);
    let galley = xpr_ui_kit::widgets::caption_galley(ui, "Speed Rankings", title_font, palette::GRAY_400);
    let tw = galley.size().x;
    let f12 = px(theme, 12.0);
    let b1w = widgets::text_w(ui, "Major Battles", &f12) + 12.0;
    let b2w = widgets::text_w(ui, "R1", &f12) + 12.0;
    let free = (w - 24.0 - tw - b1w - b2w).max(0.0);
    let gap = free / 2.0;
    let cy = full.min.y + bar_h / 2.0;
    ui.painter().galley(Pos2::new(full.min.x + 12.0, cy - galley.size().y / 2.0), galley, palette::GRAY_400);
    let b1 = Rect::from_min_size(Pos2::new(full.min.x + 12.0 + tw + gap, cy - 10.0), Vec2::new(b1w, 20.0));
    let b2 = Rect::from_min_size(Pos2::new(b1.max.x + gap, cy - 10.0), Vec2::new(b2w, 20.0));
    if tag_button(ui, theme, b1, ui.id().with("speed_major"), "Major Battles", st.major_only, palette::YELLOW_400, Color32::from_rgba_unmultiplied(0xea, 0xb3, 0x08, 51)) {
        st.major_only = !st.major_only;
    }
    if tag_button(ui, theme, b2, ui.id().with("speed_r1"), "R1", st.r1_only, palette::BLUE_400, Color32::from_rgba_unmultiplied(0x3b, 0x82, 0xf6, 51)) {
        st.r1_only = !st.r1_only;
    }
    ui.painter().hline(full.x_range(), full.min.y + bar_h + 0.5, Stroke::new(1.0_f32, palette::GRAY_700));

    // ---- column heads
    let head_y = full.min.y + bar_h + 1.0;
    let head_h = 28.0;
    let col_w = ((w - 24.0 - 32.0) / 2.0).max(10.0);
    let c1 = full.min.x + 12.0;
    let c2 = c1 + col_w;
    let c3 = full.max.x - 12.0;
    let hc = head_y + head_h / 2.0;
    let hf = px_bold(theme, 12.0);
    ui.painter().text(Pos2::new(c1, hc), Align2::LEFT_CENTER, "TRAINER", hf.clone(), palette::GRAY_500);
    ui.painter().text(Pos2::new(c2, hc), Align2::LEFT_CENTER, "POKEMON", hf.clone(), palette::GRAY_500);
    ui.painter().text(Pos2::new(c3, hc), Align2::RIGHT_CENTER, "SPD", hf, palette::GRAY_500);
    ui.painter().hline(full.x_range(), head_y + head_h + 0.5, Stroke::new(1.0_f32, palette::GRAY_700));

    // ---- rows
    let list_top = head_y + head_h + 1.0;
    let list = Rect::from_min_max(Pos2::new(full.min.x, list_top), full.max);
    let key = (ix as *const VersionIndex as usize, st.major_only, st.r1_only);
    if st.built_for != Some(key) {
        st.built_for = Some(key);
        st.filtered = ix
            .speed
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                let t = &ix.trainers[e.trainer];
                (!st.major_only || t.major) && (!st.r1_only || !t.trainer.rematch)
            })
            .map(|(i, _)| i)
            .collect();
    }
    let mut lui = ui.new_child(egui::UiBuilder::new().max_rect(list).layout(egui::Layout::top_down(egui::Align::Min)));
    lui.set_clip_rect(list.intersect(ui.clip_rect()));
    let mono = FontId::new(px(theme, 14.0).size, FontFamily::Monospace);
    let f14 = px(theme, 14.0);
    // show_rows adds the *outer* ui's item spacing to every row height
    lui.spacing_mut().item_spacing = Vec2::ZERO;
    egui::ScrollArea::vertical().id_salt("trainer_speed_rows").auto_shrink([false, false]).show_rows(&mut lui, ROW_H, st.filtered.len(), |ui, range| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        for r in range {
            let e = &ix.speed[st.filtered[r]];
            let t = &ix.trainers[e.trainer];
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_H), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(rect, CornerRadius::ZERO, Color32::from_rgba_unmultiplied(0x1f, 0x29, 0x37, 128));
            }
            ui.painter().hline(rect.x_range(), rect.max.y - 0.5, Stroke::new(1.0_f32, palette::GRAY_800));
            let cy = rect.min.y + (ROW_H - 1.0) / 2.0;
            let color = if t.major { t.name_color() } else { palette::GRAY_400 };
            let cw = ((rect.width() - 24.0 - 32.0) / 2.0).max(10.0);
            let x1 = rect.min.x + 12.0;
            let x2 = x1 + cw;
            let name = xpr_ui_kit::widgets::elide(ui, &t.trainer.name, &f14, (cw - 8.0).max(10.0));
            ui.painter().text(Pos2::new(x1, cy), Align2::LEFT_CENTER, name, f14.clone(), color);
            let sp = xpr_ui_kit::widgets::elide(ui, &e.species, &f14, (cw - 8.0).max(10.0));
            ui.painter().text(Pos2::new(x2, cy), Align2::LEFT_CENTER, sp, f14.clone(), palette::GRAY_200);
            ui.painter().text(Pos2::new(rect.max.x - 12.0, cy), Align2::RIGHT_CENTER, e.speed.to_string(), mono.clone(), palette::GRAY_300);
            let resp = resp.on_hover_text(format!("{} {}", t.trainer.trainer_class, t.trainer.name));
            if resp.clicked() {
                picked = Some(e.trainer);
            }
        }
    });
    picked
}
