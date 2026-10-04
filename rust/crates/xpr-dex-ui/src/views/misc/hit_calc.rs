//! "Hit Probability" sub-tab (Solodex `HitProbabilityCalculator.tsx`): given a
//! per-use accuracy and N uses, the chance of exactly / at least / at most K
//! hits, plus the full distribution.

use egui::{Id, Ui};
use xpr_ui_kit::theme::Theme;

use super::blocks::{accuracy_block, n_k_block, page_header, MAX_USES};
use super::controls::{card, card_fill};
use super::hit_distribution::{hit_distribution, HitDistState};
use super::hit_probability::{clamp, js_round};
use crate::palette;

const QUICK_ACCURACIES: [i32; 8] = [50, 70, 75, 80, 85, 90, 95, 100];

pub struct HitProbView {
    /// percent
    accuracy: f64,
    n: f64,
    k: f64,
    dist: HitDistState,
}

impl Default for HitProbView {
    fn default() -> Self {
        HitProbView {
            accuracy: 70.0,
            n: 5.0,
            k: 3.0,
            dist: HitDistState::default(),
        }
    }
}

impl HitProbView {
    pub fn accuracy(&self) -> f64 {
        self.accuracy
    }

    pub fn ui(&mut self, ui: &mut Ui, theme: &Theme) {
        let id = Id::new("misc_hit_prob");
        page_header(ui, theme, "Hit Probability", "If a move has accuracy X% and you use it N times, how likely is it to hit exactly, at least, or at most K times?");
        ui.add_space(20.0);
        card(card_fill(), palette::GRAY_700, 16).show(ui, |ui| {
            ui.set_width(ui.available_width());
            accuracy_block(
                ui,
                theme,
                id.with("acc"),
                "Accuracy",
                &mut self.accuracy,
                &QUICK_ACCURACIES,
            );
            ui.add_space(16.0);
            n_k_block(ui, theme, id.with("nk"), &mut self.n, &mut self.k);
        });
        ui.add_space(20.0);
        // clamp everything used for computation so outputs can never be NaN
        let safe_n = clamp(js_round(self.n), 1.0, MAX_USES);
        let safe_k = clamp(js_round(self.k), 0.0, safe_n);
        let p = clamp(self.accuracy, 0.0, 100.0) / 100.0;
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
