//! "Effective Accuracy" sub-tab (Solodex `EffectiveAccuracyCalculator.tsx`):
//! the real per-use accuracy from battle conditions (Gen 6+ accuracy / evasion
//! stages, abilities, items), then the binomial distribution on the result.

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, Id, Ui, Vec2};
use xpr_ui_kit::theme::Theme;

use super::blocks::{accuracy_block, n_k_block, page_header, stage_stepper, MAX_USES};
use super::controls::{
    accent_checkbox_row, accent_tint, caps_label, card, card_fill, pill, MISC_ACCENT,
};
use super::format::{format_number, js_string, percent};
use super::hit_distribution::{hit_distribution, HitDistState};
use super::hit_probability::{
    clamp, effective_accuracy, js_round, EffectiveAccuracyInput, ACCURACY_MODIFIERS,
};
use crate::palette::{self, px_bold};

const QUICK_ACCURACIES: [i32; 8] = [50, 70, 75, 80, 85, 90, 95, 100];

pub struct EffectiveAccuracyView {
    base_accuracy: f64,
    accuracy_stages: f64,
    evasion_stages: f64,
    active: [bool; ACCURACY_MODIFIERS.len()],
    always_hits: bool,
    n: f64,
    k: f64,
    dist: HitDistState,
}

impl Default for EffectiveAccuracyView {
    fn default() -> Self {
        EffectiveAccuracyView {
            base_accuracy: 100.0,
            accuracy_stages: 0.0,
            evasion_stages: 0.0,
            active: [false; ACCURACY_MODIFIERS.len()],
            always_hits: false,
            n: 5.0,
            k: 3.0,
            dist: HitDistState::default(),
        }
    }
}

impl EffectiveAccuracyView {
    /// The current result of the conditions (for tests).
    pub fn effective(&self) -> f64 {
        self.result().effective
    }

    fn result(&self) -> super::hit_probability::EffectiveAccuracyResult {
        let factors: Vec<f64> = ACCURACY_MODIFIERS
            .iter()
            .zip(self.active)
            .filter(|(_, on)| *on)
            .map(|(m, _)| m.factor)
            .collect();
        effective_accuracy(&EffectiveAccuracyInput {
            base_accuracy: clamp(self.base_accuracy, 0.0, 100.0),
            accuracy_stages: self.accuracy_stages,
            evasion_stages: self.evasion_stages,
            factors,
            always_hits: self.always_hits,
        })
    }

    pub fn ui(&mut self, ui: &mut Ui, theme: &Theme) {
        let id = Id::new("misc_eff_acc");
        page_header(ui, theme, "Effective Accuracy", "Build the real per-use accuracy from battle conditions, then see the hit distribution over N uses. Accuracy/evasion multipliers are Gen 6+.");
        ui.add_space(20.0);

        card(card_fill(), palette::GRAY_700, 16).show(ui, |ui| {
            ui.set_width(ui.available_width());
            // No Guard / always-hit override
            if accent_checkbox_row(
                ui,
                theme,
                self.always_hits,
                "No Guard / Lock-On",
                "\u{2014} move always hits (forces 100%)",
            ) {
                self.always_hits = !self.always_hits;
            }
            ui.add_space(16.0);
            // dimmed and inert while the override is on
            ui.add_enabled_ui(!self.always_hits, |ui| {
                accuracy_block(
                    ui,
                    theme,
                    id.with("base"),
                    "Base accuracy",
                    &mut self.base_accuracy,
                    &QUICK_ACCURACIES,
                );
                ui.add_space(16.0);
                ui.spacing_mut().item_spacing.x = 16.0;
                ui.columns(2, |cols| {
                    stage_stepper(
                        &mut cols[0],
                        theme,
                        id.with("acc_stage"),
                        "Your accuracy stages",
                        &mut self.accuracy_stages,
                    );
                    stage_stepper(
                        &mut cols[1],
                        theme,
                        id.with("eva_stage"),
                        "Target evasion stages",
                        &mut self.evasion_stages,
                    );
                });
                ui.add_space(16.0);
                caps_label(ui, theme, "Other modifiers");
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                    for (i, m) in ACCURACY_MODIFIERS.iter().enumerate() {
                        let r = pill(ui, theme, m.label, Some(m.note), self.active[i], false)
                            .on_hover_text(format!("{} {}", m.label, m.note));
                        if r.clicked() {
                            self.active[i] = !self.active[i];
                        }
                    }
                });
            });
        });
        ui.add_space(20.0);

        // effective accuracy readout + breakdown
        let result = self.result();
        let p = result.effective / 100.0;
        card(accent_tint(), MISC_ACCENT, 16).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let (row, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 38.0), egui::Sense::hover());
            ui.painter().text(
                egui::Pos2::new(row.min.x, row.max.y - 8.0),
                egui::Align2::LEFT_BOTTOM,
                "EFFECTIVE ACCURACY",
                px_bold(theme, 12.0),
                palette::GRAY_400,
            );
            ui.painter().text(
                egui::Pos2::new(row.max.x, row.center().y),
                egui::Align2::RIGHT_CENTER,
                percent(p),
                px_bold(theme, 30.0),
                Color32::WHITE,
            );
            ui.add_space(4.0);
            let mut job = LayoutJob::default();
            job.wrap.max_width = ui.available_width();
            let add = |job: &mut LayoutJob, text: &str, font: &FontId, color: Color32| {
                job.append(
                    text,
                    0.0,
                    TextFormat {
                        font_id: font.clone(),
                        color,
                        ..Default::default()
                    },
                );
            };
            if result.always_hits {
                // a plain sentence, not the formula
                add(
                    &mut job,
                    "No Guard / Lock-On \u{2014} the move always hits.",
                    &crate::palette::px(theme, 12.0),
                    palette::GRAY_400,
                );
            } else {
                let mono = FontId::monospace(12.0);
                add(
                    &mut job,
                    &format!(
                        "{}%",
                        format_number(clamp(self.base_accuracy, 0.0, 100.0), 2)
                    ),
                    &mono,
                    palette::GRAY_400,
                );
                let stage = if result.combined_stage >= 0.0 {
                    format!("+{}", js_string(result.combined_stage))
                } else {
                    js_string(result.combined_stage)
                };
                add(
                    &mut job,
                    &format!(
                        " \u{00D7} stage {} (\u{00D7}{})",
                        stage,
                        format_number(result.stage_multiplier, 3)
                    ),
                    &mono,
                    palette::GRAY_400,
                );
                for (m, on) in ACCURACY_MODIFIERS.iter().zip(self.active) {
                    if on {
                        add(
                            &mut job,
                            &format!(" \u{00D7} {} (\u{00D7}{})", m.label, js_string(m.factor)),
                            &mono,
                            palette::GRAY_400,
                        );
                    }
                }
                if result.capped_at_100 {
                    add(
                        &mut job,
                        " \u{2192} capped at 100%",
                        &mono,
                        palette::AMBER_400,
                    );
                }
            }
            ui.add(egui::Label::new(job).wrap());
        });
        ui.add_space(20.0);

        // distribution over N uses at the effective accuracy
        card(card_fill(), palette::GRAY_700, 16).show(ui, |ui| {
            ui.set_width(ui.available_width());
            n_k_block(ui, theme, id.with("nk"), &mut self.n, &mut self.k);
        });
        ui.add_space(20.0);
        let safe_n = clamp(js_round(self.n), 1.0, MAX_USES);
        let safe_k = clamp(js_round(self.k), 0.0, safe_n);
        if let Some(k) = hit_distribution(
            ui,
            theme,
            id.with("dist"),
            &mut self.dist,
            safe_n as i64,
            p,
            safe_k as i64,
        ) {
            self.k = k as f64;
        }
    }
}
