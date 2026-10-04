//! Control groups the Misc calculators share: the page header, the accuracy
//! slider with its quick pills, the N / K pair and the stage stepper.

use egui::{Align2, Color32, CornerRadius, Id, Sense, Stroke, StrokeKind, Ui, Vec2};
use xpr_ui_kit::theme::Theme;

use super::controls::{caps_label, number_field, pill, range_slider};
use super::format::js_string;
use super::hit_probability::{clamp, js_round};
use crate::palette::{self, px, px_bold};
use crate::widgets::text_w;

/// The most uses N can be set to.
pub const MAX_USES: f64 = 50.0;

/// `<h2>` (18 px bold white) and the gray description under it.
pub fn page_header(ui: &mut Ui, theme: &Theme, title: &str, description: &str) {
    ui.label(
        egui::RichText::new(title)
            .font(px_bold(theme, 18.0))
            .color(Color32::WHITE),
    );
    ui.add(
        egui::Label::new(
            egui::RichText::new(description)
                .font(px(theme, 14.0))
                .color(palette::GRAY_400),
        )
        .wrap(),
    );
}

/// A labelled accuracy slider + number field + percent sign, then the quick
/// pills that set it.
pub fn accuracy_block(
    ui: &mut Ui,
    theme: &Theme,
    id: Id,
    label: &str,
    accuracy: &mut f64,
    quick: &[i32],
) {
    caps_label(ui, theme, label);
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        let pct_font = px(theme, 14.0);
        let pct_w = text_w(ui, "%", &pct_font);
        let slider_w = ui.available_width() - (64.0 + 4.0 + pct_w) - 12.0;
        if let Some(v) = range_slider(
            ui,
            id.with("slider"),
            slider_w,
            clamp(*accuracy, 0.0, 100.0),
            0.0,
            100.0,
        ) {
            *accuracy = v;
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            number_field(
                ui,
                theme,
                id.with("number"),
                accuracy,
                0.0,
                100.0,
                false,
                1.0,
            );
            ui.label(
                egui::RichText::new("%")
                    .font(pct_font)
                    .color(palette::GRAY_400),
            );
        });
    });
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
        for a in quick {
            if pill(
                ui,
                theme,
                &format!("{}%", a),
                None,
                *accuracy == *a as f64,
                false,
            )
            .clicked()
            {
                *accuracy = *a as f64;
            }
        }
    });
}

/// `setNClamped`: N stays in 1..=50 and K follows it down.
fn set_n(n: &mut f64, k: &mut f64, v: f64) {
    let nn = clamp(js_round(v), 1.0, MAX_USES);
    *n = nn;
    if *k > nn {
        *k = nn;
    }
}

/// The "Number of uses (N)" and "Target hits (K)" sliders side by side.
pub fn n_k_block(ui: &mut Ui, theme: &Theme, id: Id, n: &mut f64, k: &mut f64) {
    let safe_n = clamp(js_round(*n), 1.0, MAX_USES);
    let safe_k = clamp(js_round(*k), 0.0, safe_n);
    ui.spacing_mut().item_spacing.x = 16.0;
    ui.columns(2, |cols| {
        {
            let ui = &mut cols[0];
            caps_label(ui, theme, "Number of uses (N)");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                let w = ui.available_width() - 64.0 - 12.0;
                if let Some(v) = range_slider(ui, id.with("n_slider"), w, safe_n, 1.0, MAX_USES) {
                    set_n(n, k, v);
                }
                let mut nv = safe_n;
                if number_field(
                    ui,
                    theme,
                    id.with("n_number"),
                    &mut nv,
                    1.0,
                    MAX_USES,
                    true,
                    1.0,
                ) {
                    set_n(n, k, nv);
                }
            });
        }
        {
            let ui = &mut cols[1];
            caps_label(ui, theme, "Target hits (K)");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                let w = ui.available_width() - 64.0 - 12.0;
                if let Some(v) = range_slider(ui, id.with("k_slider"), w, safe_k, 0.0, safe_n) {
                    *k = v;
                }
                let mut kv = safe_k;
                if number_field(
                    ui,
                    theme,
                    id.with("k_number"),
                    &mut kv,
                    0.0,
                    safe_n,
                    true,
                    1.0,
                ) {
                    *k = kv;
                }
            });
        }
    });
}

/// A -6..=+6 stage stepper: [\u{2212}] +N [+].
pub fn stage_stepper(ui: &mut Ui, theme: &Theme, id: Id, label: &str, value: &mut f64) {
    caps_label(ui, theme, label);
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if step_button(ui, theme, id.with("minus"), "\u{2212}", *value > -6.0).clicked() {
            *value = clamp(*value - 1.0, -6.0, 6.0);
        }
        let (rect, _) = ui.allocate_exact_size(Vec2::new(48.0, 32.0), Sense::hover());
        let text = if *value > 0.0 {
            format!("+{}", js_string(*value))
        } else {
            js_string(*value)
        };
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            text,
            px_bold(theme, 18.0),
            Color32::WHITE,
        );
        if step_button(ui, theme, id.with("plus"), "+", *value < 6.0).clicked() {
            *value = clamp(*value + 1.0, -6.0, 6.0);
        }
    });
}

fn step_button(ui: &mut Ui, theme: &Theme, id: Id, text: &str, enabled: bool) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
    let resp = ui.interact(
        rect,
        id,
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let alpha = if enabled { 255 } else { 102 };
    let border = if enabled && resp.hovered() {
        palette::GRAY_400
    } else {
        palette::GRAY_600
    };
    let faded = |c: Color32| Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha);
    ui.painter().rect(
        rect,
        CornerRadius::same(4),
        faded(palette::GRAY_800),
        Stroke::new(1.0_f32, faded(border)),
        StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        px(theme, 18.0),
        faded(palette::GRAY_200),
    );
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}
