//! The defensive type matchups of a (dual) typing (Solodex
//! `TypeEffectivenessPanel.tsx`): one block per multiplier (x4, x2, 1/2,
//! 1/4, immune), two type badges per row, with the abilities that grant an
//! immunity noted in italics.

use std::collections::HashMap;

use egui::{Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use xpr_dex::types::{ability_immunity_type, EFF_GROUPS};

use super::common;
use crate::palette;
use crate::widgets;
use crate::DexCx;

const ROW_H: f32 = 20.0;

/// Draw the panel `ui.available_width()` wide. `abilities` are the species'
/// distinct abilities (those that grant an immunity are shown beside the
/// type they make immune).
pub fn type_effectiveness(
    ui: &mut Ui,
    cx: &mut DexCx,
    type1: &str,
    type2: &str,
    game: &str,
    abilities: &[String],
) {
    let theme = cx.theme;
    let matchups = xpr_dex::defense_matchups(type1, type2, game);
    let mut immune_by: HashMap<&str, &str> = HashMap::new();
    for ability in abilities {
        if let Some(t) = ability_immunity_type(ability, game) {
            immune_by.insert(t, ability.as_str());
        }
    }
    let groups: Vec<_> = EFF_GROUPS
        .iter()
        .filter_map(|g| {
            let types: Vec<&str> = matchups
                .iter()
                .filter(|(_, v)| **v == g.value)
                .map(|(t, _)| t.as_str())
                .collect();
            (!types.is_empty()).then_some((g, types))
        })
        .collect();
    let w = ui.available_width();
    let col_w = (w - 8.0) / 2.0;
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    for (gi, (group, types)) in groups.iter().enumerate() {
        if gi > 0 {
            // mt-2 pt-2 border-t border-gray-800
            ui.add_space(8.0);
            let (r, _) = ui.allocate_exact_size(Vec2::new(w, 1.0), Sense::hover());
            ui.painter().hline(
                r.x_range(),
                r.center().y,
                Stroke::new(1.0_f32, palette::GRAY_800),
            );
            ui.add_space(8.0);
        }
        for (ri, chunk) in types.chunks(2).enumerate() {
            if ri > 0 {
                ui.add_space(2.0);
            }
            let (row, _) = ui.allocate_exact_size(Vec2::new(w, ROW_H), Sense::hover());
            for (ci, t) in chunk.iter().enumerate() {
                let x = row.min.x + ci as f32 * (col_w + 8.0);
                let badge =
                    Rect::from_min_size(Pos2::new(x, row.min.y + 1.0), Vec2::new(68.0, 18.0));
                common::in_rect(ui, badge, ("dex_eff_badge", gi, *t), |ui| {
                    widgets::type_badge(ui, theme, t, true, Some(game));
                });
                let mult =
                    Rect::from_min_size(Pos2::new(x + 72.0, row.min.y), Vec2::new(28.0, ROW_H));
                ui.painter()
                    .rect_filled(mult, CornerRadius::same(4), palette::hex(group.bg));
                ui.painter().text(
                    mult.center(),
                    Align2::CENTER_CENTER,
                    group.multiplier_label,
                    palette::px_bold(theme, 12.0),
                    palette::hex(group.text),
                );
                if let Some(ability) = immune_by.get(t) {
                    let fmt = egui::TextFormat {
                        font_id: palette::px(theme, 12.0),
                        color: palette::GRAY_500,
                        italics: true,
                        ..Default::default()
                    };
                    let job = egui::text::LayoutJob::single_section(format!("({})", ability), fmt);
                    let g = ui.fonts_mut(|f| f.layout_job(job));
                    let pos = Pos2::new(mult.max.x + 4.0, row.center().y - g.size().y / 2.0);
                    ui.painter().galley(pos, g, Color32::PLACEHOLDER);
                }
            }
        }
    }
}
