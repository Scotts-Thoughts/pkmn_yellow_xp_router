//! Misc tab: hit probability, effective accuracy and multi-hit calculators.
//! Port of Solodex `components/misc/` (`MiscView.tsx` and its sub-tabs) and
//! `utils/hitProbability.ts` (the maths, in [`hit_probability`]).

use egui::{Align2, Pos2, Rect, Sense, Ui, Vec2};

use crate::palette::{self, px_bold};
use crate::widgets::text_w;
use crate::DexCx;

mod blocks;
pub mod controls;
mod effective_accuracy;
pub mod format;
mod hit_calc;
mod hit_distribution;
pub mod hit_probability;
mod multi_hit;

pub use controls::MISC_ACCENT;
pub use effective_accuracy::EffectiveAccuracyView;
pub use hit_calc::HitProbView;
pub use multi_hit::MultiHitView;

/// The sub-tabs, in order (`DexSettings::misc_tab` indexes this).
pub const TABS: [&str; 3] = ["Hit Probability", "Effective Accuracy", "Multi-Hit Moves"];

#[derive(Default)]
pub struct MiscView {
    hit: HitProbView,
    eff: EffectiveAccuracyView,
    multi: MultiHitView,
}

impl MiscView {
    /// The active sub-tab (a saved index out of range falls back to the first).
    pub fn active_tab(cx: &DexCx) -> usize {
        let t = cx.state.settings.misc_tab;
        if t < TABS.len() {
            t
        } else {
            0
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut DexCx) {
        let theme = cx.theme;
        let active = Self::active_tab(cx);
        let full = ui.available_rect_before_wrap();

        // sub-tab bar: gap-1, px-4, pt-2, border-b gray-700
        let bar_h = 8.0 + 36.0 + 1.0;
        let bar = Rect::from_min_size(full.min, Vec2::new(full.width(), bar_h));
        // the border line under the bar (the active tab's underline overlaps it)
        ui.painter().rect_filled(
            Rect::from_min_max(Pos2::new(bar.min.x, bar.max.y - 1.0), bar.max),
            0.0,
            palette::GRAY_700,
        );
        let mut x = bar.min.x + 16.0;
        let tab_font = px_bold(theme, 14.0);
        let mut clicked = None;
        for (i, label) in TABS.iter().enumerate() {
            let w = text_w(ui, label, &tab_font) + 32.0;
            let r = Rect::from_min_size(Pos2::new(x, bar.min.y + 8.0), Vec2::new(w, 36.0));
            let resp = ui
                .interact(r, ui.id().with(("misc_tab", i)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            let color = if i == active {
                egui::Color32::WHITE
            } else if resp.hovered() {
                palette::GRAY_200
            } else {
                palette::GRAY_400
            };
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                label,
                tab_font.clone(),
                color,
            );
            if resp.clicked() {
                clicked = Some(i);
            }
            if i == active {
                ui.painter().rect_filled(
                    Rect::from_min_max(
                        Pos2::new(r.min.x, r.max.y - 1.0),
                        Pos2::new(r.max.x, r.max.y + 1.0),
                    ),
                    0.0,
                    MISC_ACCENT,
                );
            }
            x += w + 4.0;
        }
        if let Some(i) = clicked {
            if cx.state.settings.misc_tab != i {
                cx.state.settings.misc_tab = i;
                cx.state.touch();
            }
        }
        let active = clicked.unwrap_or(active);

        // active tab, in a centred column (mx-auto max-w-3xl p-6)
        let body = Rect::from_min_max(Pos2::new(full.min.x, bar.max.y), full.max);
        let mut bui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(body)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        bui.set_clip_rect(body.intersect(ui.clip_rect()));
        let area = egui::ScrollArea::vertical()
            .id_salt("misc_scroll")
            .auto_shrink([false, false]);
        xpr_ui_kit::widgets::show_scroll(&mut bui, area, |ui| {
            column(ui, |ui| match active {
                0 => self.hit.ui(ui, theme),
                1 => self.eff.ui(ui, theme),
                _ => self.multi.ui(ui, theme),
            });
        });
        ui.allocate_rect(full, Sense::hover());
    }
}

/// `mx-auto max-w-3xl p-6`: a centred column, 768 px wide at most, 24 px padding.
fn column(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let avail = ui.available_rect_before_wrap();
    let w = avail.width().min(controls::COLUMN_MAX_W);
    let x = avail.center().x - w / 2.0;
    let pad = controls::COLUMN_PAD;
    let inner = Rect::from_min_max(
        Pos2::new(x + pad, avail.min.y + pad),
        Pos2::new(x + w - pad, avail.min.y + 100_000.0),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
    add(&mut child);
    let used = child.min_rect();
    ui.allocate_rect(
        Rect::from_min_max(avail.min, Pos2::new(avail.max.x, used.max.y + pad)),
        Sense::hover(),
    );
}
