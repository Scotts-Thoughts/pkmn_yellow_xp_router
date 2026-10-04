//! The growth-rate experience table popover (Solodex
//! `GrowthRatePopover.tsx`).

use egui::{Align2, Color32, CornerRadius, Pos2, RectAlign, Response, Sense, Stroke, Vec2};
use xpr_ui_kit::theme::Theme;
use xpr_ui_kit::widgets::fmt_thousands;

use super::common;
use crate::palette;

const POPOVER_WIDTH: f32 = 240.0;
const POPOVER_HEIGHT: f32 = 400.0;
const ROW_H: f32 = 20.0;
const COLS_H: f32 = 24.0;

/// Total experience at level `n` for a growth rate (the standard formulas).
pub fn calc_exp(rate: &str, n: i64) -> i64 {
    let n_f = n as f64;
    let cube = n_f * n_f * n_f;
    match rate {
        "Erratic" => {
            if n <= 50 {
                (cube * (100.0 - n_f) / 50.0).floor() as i64
            } else if n <= 68 {
                (cube * (150.0 - n_f) / 100.0).floor() as i64
            } else if n <= 98 {
                (cube * ((1911.0 - 10.0 * n_f) / 3.0).floor() / 500.0).floor() as i64
            } else {
                (cube * (160.0 - n_f) / 100.0).floor() as i64
            }
        }
        "Fast" => (4.0 * cube / 5.0).floor() as i64,
        "Medium Fast" => n * n * n,
        "Medium Slow" => {
            ((6.0 / 5.0) * cube - 15.0 * n_f * n_f + 100.0 * n_f - 140.0).floor() as i64
        }
        "Slow" => (5.0 * cube / 4.0).floor() as i64,
        "Fluctuating" => {
            if n <= 15 {
                (cube * ((((n_f + 1.0) / 3.0).floor() + 24.0) / 50.0)).floor() as i64
            } else if n <= 36 {
                (cube * ((n_f + 14.0) / 50.0)).floor() as i64
            } else {
                (cube * (((n_f / 2.0).floor() + 32.0) / 50.0)).floor() as i64
            }
        }
        _ => n * n * n,
    }
}

/// The rate's accent colour.
pub fn rate_color(rate: &str) -> Color32 {
    palette::hex(match rate {
        "Erratic" => "#F59E0B",
        "Fast" => "#22C55E",
        "Medium Fast" => "#3B82F6",
        "Medium Slow" => "#8B5CF6",
        "Slow" => "#EF4444",
        "Fluctuating" => "#EC4899",
        _ => "#6B7280",
    })
}

/// The popover toggled by clicks on `anchor` (the growth rate's name): a
/// table of every level's total experience and the step to the next.
pub fn growth_popover(theme: &Theme, anchor: &Response, rate: &str) {
    let color = rate_color(rate);
    let frame = egui::Frame::new()
        .fill(palette::GRAY_800)
        .stroke(Stroke::new(1.0_f32, palette::GRAY_600))
        .corner_radius(CornerRadius::same(8))
        .shadow(egui::Shadow {
            offset: [0, 12],
            blur: 32,
            spread: 0,
            color: Color32::from_black_alpha(150),
        });
    egui::Popup::from_toggle_button_response(anchor)
        .align(RectAlign::RIGHT_START)
        .align_alternatives(&[RectAlign::LEFT_START])
        .gap(6.0)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .frame(frame)
        .width(POPOVER_WIDTH)
        .show(|ui| {
            ui.set_width(POPOVER_WIDTH - 2.0);
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let total_100 = calc_exp(rate, 100);
            let (head, _) = ui.allocate_exact_size(
                Vec2::new(POPOVER_WIDTH - 2.0, 8.0 + 16.0 + 2.0 + 15.0 + 8.0 + 1.0),
                Sense::hover(),
            );
            let g = common::spaced_galley(
                ui,
                &rate.to_uppercase(),
                palette::px_bold(theme, 12.0),
                color,
                1.2,
                f32::INFINITY,
            );
            ui.painter()
                .galley(Pos2::new(head.min.x + 12.0, head.min.y + 8.0), g, color);
            ui.painter().text(
                Pos2::new(head.min.x + 12.0, head.min.y + 8.0 + 16.0 + 2.0),
                Align2::LEFT_TOP,
                format!("{} total exp to Lv100", fmt_thousands(total_100)),
                palette::px(theme, 10.0),
                palette::GRAY_500,
            );
            ui.painter().hline(
                head.x_range(),
                head.max.y - 0.5,
                Stroke::new(1.0_f32, palette::GRAY_700),
            );
            let (cols, _) =
                ui.allocate_exact_size(Vec2::new(POPOVER_WIDTH - 2.0, COLS_H), Sense::hover());
            let f = palette::px_bold(theme, 12.0);
            ui.painter().text(
                Pos2::new(cols.min.x + 8.0, cols.center().y),
                Align2::LEFT_CENTER,
                "Lv",
                f.clone(),
                palette::GRAY_500,
            );
            ui.painter().text(
                Pos2::new(cols.min.x + 144.0, cols.center().y),
                Align2::RIGHT_CENTER,
                "Total Exp",
                f.clone(),
                palette::GRAY_500,
            );
            ui.painter().text(
                Pos2::new(cols.max.x - 8.0, cols.center().y),
                Align2::RIGHT_CENTER,
                "To Next",
                f,
                palette::GRAY_500,
            );
            let view_h = POPOVER_HEIGHT - COLS_H;
            egui::ScrollArea::vertical()
                .id_salt("dex_growth_rows")
                .min_scrolled_height(view_h)
                .max_height(view_h)
                .auto_shrink([false, false])
                .show_rows(ui, ROW_H, 100, |ui, range| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    for i in range {
                        let lv = i as i64 + 1;
                        let total = calc_exp(rate, lv);
                        let prev = if lv > 1 { calc_exp(rate, lv - 1) } else { 0 };
                        let (rect, _) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), ROW_H),
                            Sense::hover(),
                        );
                        if lv % 10 == 0 {
                            ui.painter().rect_filled(
                                rect,
                                CornerRadius::ZERO,
                                palette::GRAY_700.gamma_multiply(0.2),
                            );
                        }
                        let small = palette::px(theme, 12.0);
                        let cy = rect.center().y;
                        ui.painter().text(
                            Pos2::new(rect.min.x + 8.0, cy),
                            Align2::LEFT_CENTER,
                            lv.to_string(),
                            small.clone(),
                            palette::GRAY_400,
                        );
                        ui.painter().text(
                            Pos2::new(rect.min.x + 144.0, cy),
                            Align2::RIGHT_CENTER,
                            fmt_thousands(total),
                            small.clone(),
                            palette::GRAY_300,
                        );
                        ui.painter().text(
                            Pos2::new(rect.max.x - 8.0, cy),
                            Align2::RIGHT_CENTER,
                            fmt_thousands(total - prev),
                            small,
                            palette::GRAY_500,
                        );
                    }
                });
        });
}
