//! "Multi-Hit Moves" sub-tab (Solodex `MultiHitCalculator.tsx`): a standard 2-5
//! hit move makes ONE accuracy check for the whole move; if it passes, the hit
//! count is rolled from a fixed distribution. A different model from the
//! binomial calculator.

use egui::{Align2, CornerRadius, Id, Pos2, Rect, Sense, Ui, Vec2};
use xpr_ui_kit::theme::Theme;

use super::blocks::{accuracy_block, page_header};
use super::controls::{caps_label, caps_label_gray500, card, card_fill, pill, MISC_ACCENT};
use super::format::{number, percent};
use super::hit_distribution::stat_card;
use super::hit_probability::{
    expected_rolled_hits, expected_total_hits, multi_hit_outcomes, MultiHitGen, MultiHitModifier,
};
use crate::palette::{self, px, px_bold};

const QUICK_ACCURACIES: [i32; 5] = [80, 85, 90, 95, 100];

pub struct MultiHitView {
    accuracy: f64,
    gen: MultiHitGen,
    modifier: MultiHitModifier,
}

impl Default for MultiHitView {
    fn default() -> Self {
        MultiHitView {
            accuracy: 100.0,
            gen: MultiHitGen::Gen5Plus,
            modifier: MultiHitModifier::None,
        }
    }
}

impl MultiHitView {
    pub fn modifier(&self) -> MultiHitModifier {
        self.modifier
    }

    pub fn gen(&self) -> MultiHitGen {
        self.gen
    }

    /// `setGenSafe`: Loaded Dice is a Gen 9 item, so it is dropped when Gen 1-4
    /// is chosen.
    fn set_gen(&mut self, g: MultiHitGen) {
        self.gen = g;
        if g == MultiHitGen::Gen1To4 && self.modifier == MultiHitModifier::LoadedDice {
            self.modifier = MultiHitModifier::None;
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, theme: &Theme) {
        let id = Id::new("misc_multi_hit");
        page_header(ui, theme, "Multi-Hit Moves", "Bullet Seed, Rock Blast, Pin Missile, Icicle Spear\u{2026} make one accuracy check for the whole move; if it passes, the number of hits (2\u{2013}5) is rolled from a fixed distribution.");
        ui.add_space(20.0);

        card(card_fill(), palette::GRAY_700, 16).show(ui, |ui| {
            ui.set_width(ui.available_width());
            accuracy_block(
                ui,
                theme,
                id.with("acc"),
                "Move accuracy",
                &mut self.accuracy,
                &QUICK_ACCURACIES,
            );
            ui.add_space(16.0);
            caps_label(ui, theme, "Generation");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if pill(
                    ui,
                    theme,
                    "Gen 1\u{2013}4",
                    None,
                    self.gen == MultiHitGen::Gen1To4,
                    false,
                )
                .clicked()
                {
                    self.set_gen(MultiHitGen::Gen1To4);
                }
                if pill(
                    ui,
                    theme,
                    "Gen 5+",
                    None,
                    self.gen == MultiHitGen::Gen5Plus,
                    false,
                )
                .clicked()
                {
                    self.set_gen(MultiHitGen::Gen5Plus);
                }
            });
            ui.add_space(16.0);
            caps_label(ui, theme, "Modifier");
            ui.add_space(8.0);
            // Loaded Dice is a Gen 9 item: only offered in Gen 5+ mode
            let loaded_dice_available = self.gen == MultiHitGen::Gen5Plus;
            let modifiers = [
                (MultiHitModifier::None, "None", "2\u{2013}5 hits", false),
                (MultiHitModifier::SkillLink, "Skill Link", "always 5", false),
                (
                    MultiHitModifier::LoadedDice,
                    "Loaded Dice",
                    "\u{2265}4 hits (Gen 9)",
                    !loaded_dice_available,
                ),
            ];
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                for (m, label, note, disabled) in modifiers {
                    let tip = if disabled {
                        "Loaded Dice is a Gen 9 item".to_string()
                    } else {
                        format!("{} \u{2014} {}", label, note)
                    };
                    let r = pill(ui, theme, label, Some(note), self.modifier == m, disabled)
                        .on_hover_text(tip);
                    if r.clicked() && !disabled {
                        self.modifier = m;
                    }
                }
            });
        });
        ui.add_space(20.0);

        // summary
        let outcomes = multi_hit_outcomes(self.accuracy, self.gen, self.modifier);
        let exp_total = expected_total_hits(self.accuracy, self.gen, self.modifier);
        let exp_rolled = expected_rolled_hits(self.gen, self.modifier);
        let max_p = outcomes.iter().fold(0.0_f64, |m, o| m.max(o.probability));
        let (row, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 91.0), Sense::hover());
        let cw = (row.width() - 12.0) / 2.0;
        let r1 = Rect::from_min_size(row.min, Vec2::new(cw, 91.0));
        let r2 = Rect::from_min_size(
            Pos2::new(row.min.x + cw + 12.0, row.min.y),
            Vec2::new(cw, 91.0),
        );
        stat_card(
            ui,
            theme,
            r1,
            "Expected total hits",
            &number(exp_total),
            "P(pass) \u{00D7} E[rolled]",
            true,
            px_bold(theme, 24.0),
        );
        stat_card(
            ui,
            theme,
            r2,
            "Avg hits if it connects",
            &number(exp_rolled),
            "E[rolled hit count]",
            false,
            px_bold(theme, 24.0),
        );
        ui.add_space(20.0);

        // outcome distribution
        card(card_fill(), palette::GRAY_700, 12).show(ui, |ui| {
            ui.set_width(ui.available_width());
            caps_label_gray500(
                ui,
                theme,
                "Outcome probability \u{2014} 0 hits means the accuracy check failed",
            );
            ui.add_space(8.0);
            ui.spacing_mut().item_spacing.y = 6.0;
            for o in &outcomes {
                let is_miss = o.hits == 0;
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
                let pct = percent(o.probability);
                resp.on_hover_text(format!("{} hits: {}", o.hits, pct));
                let p = ui.painter();
                let label = if is_miss {
                    "Miss".to_string()
                } else {
                    format!("{} hits", o.hits)
                };
                p.text(
                    Pos2::new(rect.min.x + 56.0, rect.center().y),
                    Align2::RIGHT_CENTER,
                    label,
                    px(theme, 12.0),
                    if is_miss {
                        palette::GRAY_500
                    } else {
                        palette::GRAY_300
                    },
                );
                let bar = Rect::from_min_max(
                    Pos2::new(rect.min.x + 64.0, rect.min.y),
                    Pos2::new((rect.max.x - 88.0).max(rect.min.x + 65.0), rect.max.y),
                );
                p.rect_filled(bar, CornerRadius::same(4), palette::GRAY_800);
                let frac = if max_p > 0.0 {
                    (o.probability / max_p) as f32
                } else {
                    0.0
                };
                let mut fw = bar.width() * frac;
                if o.probability > 0.0 {
                    fw = fw.max(2.0);
                }
                if fw > 0.0 {
                    p.rect_filled(
                        Rect::from_min_size(bar.min, Vec2::new(fw.min(bar.width()), bar.height())),
                        CornerRadius::same(4),
                        if is_miss {
                            palette::GRAY_500
                        } else {
                            MISC_ACCENT
                        },
                    );
                }
                p.text(
                    Pos2::new(rect.max.x, rect.center().y),
                    Align2::RIGHT_CENTER,
                    pct,
                    px(theme, 12.0),
                    palette::GRAY_300,
                );
            }
        });
    }
}
