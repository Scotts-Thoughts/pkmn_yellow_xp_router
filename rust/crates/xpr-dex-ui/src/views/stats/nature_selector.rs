//! Solodex's `NatureSelector`: pick a nature with two rows of stat buttons
//! (boost / lower), mirroring the Natures tab. Choosing the same stat for both
//! gives that stat's neutral nature. Shared by the Stats tab and the Damage
//! tab.

use std::hash::Hash;

use egui::text::{LayoutJob, TextFormat};
use egui::{Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use xpr_ui_kit::theme::Theme;

use crate::palette::{self, hex, px_bold};

/// The stats a nature can change, ordered for display (Atk Def SpA SpD Spe),
/// in natures.json's spelling.
pub const STAT_KEYS: [&str; 5] = ["attack", "defense", "specialAttack", "specialDefense", "speed"];

fn abbr(key: &str) -> &'static str {
    match key {
        "attack" => "Atk",
        "defense" => "Def",
        "specialAttack" => "SpA",
        "specialDefense" => "SpD",
        "speed" => "Spe",
        _ => "",
    }
}

/// Stat colours / cell backgrounds, matching the Natures tab.
fn stat_color(key: &str) -> Color32 {
    hex(match key {
        "attack" => "#F8D030",
        "defense" => "#F08030",
        "speed" => "#F85888",
        "specialAttack" => "#6890F0",
        "specialDefense" => "#7038F8",
        _ => "#9ca3af",
    })
}

fn stat_bg(key: &str) -> Color32 {
    hex(match key {
        "attack" => "#3d3312",
        "defense" => "#3d2412",
        "speed" => "#3d1522",
        "specialAttack" => "#1a243d",
        "specialDefense" => "#1e103d",
        _ => "#111827",
    })
}

/// Neutral natures sit on the diagonal: index 0 -> Atk, 6 -> Def, 12 -> Spe,
/// 18 -> SpA, 24 -> SpD.
fn stat_by_index(index: i32) -> Option<&'static str> {
    match index {
        0 => Some("attack"),
        6 => Some("defense"),
        12 => Some("speed"),
        18 => Some("specialAttack"),
        24 => Some("specialDefense"),
        _ => None,
    }
}

/// A nature's (raised, lowered) stat; a neutral nature names its diagonal stat twice.
pub fn nature_pair(name: &str) -> Option<(&'static str, &'static str)> {
    let d = xpr_dex::natures::nature(name)?;
    let as_key = |k: &str| STAT_KEYS.iter().copied().find(|s| *s == k);
    match (d.increased.as_deref(), d.decreased.as_deref()) {
        (Some(i), Some(dec)) => Some((as_key(i)?, as_key(dec)?)),
        _ => {
            let s = stat_by_index(d.index)?;
            Some((s, s))
        }
    }
}

/// The nature a (raised, lowered) pair makes.
pub fn nature_for_pair(inc: &str, dec: &str) -> Option<&'static str> {
    let want = (as_static(inc)?, as_static(dec)?);
    xpr_dex::natures::natures().into_iter().map(|(name, _)| name).find(|name| nature_pair(name) == Some(want))
}

fn as_static(k: &str) -> Option<&'static str> {
    STAT_KEYS.iter().copied().find(|s| *s == k)
}

/// Draw the selector (two button rows and a caption) for `value`; returns the
/// new nature name when a button was clicked.
pub fn nature_selector(ui: &mut Ui, theme: &Theme, id_salt: impl Hash, value: &str) -> Option<String> {
    let id = ui.id().with(("nature_selector", id_salt));
    let (inc, dec) = nature_pair(value).unwrap_or(("attack", "attack"));
    let neutral = inc == dec;
    let mut result = None;
    ui.spacing_mut().item_spacing.y = 4.0;
    for (label, accent, which_inc) in [("+", hex("#22c55e"), true), ("\u{2212}", hex("#ef4444"), false)] {
        let selected = if neutral {
            None
        } else if which_inc {
            Some(inc)
        } else {
            Some(dec)
        };
        let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 25.0), Sense::hover());
        // the +/- sign: w-3, then gap-1.5, buttons flex-1 with gap-1
        ui.painter().text(Pos2::new(row.min.x + 6.0, row.center().y), Align2::CENTER_CENTER, label, px_bold(theme, 10.0), accent);
        let x0 = row.min.x + 12.0 + 6.0;
        let gap = 4.0;
        let bw = (row.max.x - x0 - gap * 4.0) / 5.0;
        for (i, stat) in STAT_KEYS.iter().enumerate() {
            let r = Rect::from_min_size(Pos2::new(x0 + i as f32 * (bw + gap), row.min.y), Vec2::new(bw, row.height()));
            let resp = ui.interact(r, id.with((which_inc, *stat)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            let is_sel = selected == Some(*stat);
            let (fill, line, text) = if is_sel { (stat_bg(stat), stat_color(stat), stat_color(stat)) } else { (hex("#111827"), hex("#374151"), hex("#9ca3af")) };
            ui.painter().rect(r, CornerRadius::same(4), fill, Stroke::new(1.0_f32, line), StrokeKind::Inside);
            ui.painter().text(r.center(), Align2::CENTER_CENTER, abbr(stat), px_bold(theme, 10.0), text);
            if resp.clicked() {
                let (ni, nd) = if which_inc { (*stat, dec) } else { (inc, *stat) };
                if let Some(n) = nature_for_pair(ni, nd) {
                    result = Some(n.to_string());
                }
            }
        }
    }
    // caption: the nature's name (and "neutral")
    let mut job = LayoutJob::default();
    job.append(value, 16.0, TextFormat { font_id: px_bold(theme, 10.0), color: palette::GRAY_200, ..Default::default() });
    if neutral {
        job.append(" \u{00B7} neutral", 0.0, TextFormat { font_id: crate::palette::px(theme, 10.0), color: palette::GRAY_600, ..Default::default() });
    }
    ui.label(job);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_nature_has_a_unique_pair_that_maps_back() {
        let mut seen = std::collections::HashSet::new();
        for (name, _) in xpr_dex::natures::natures() {
            let (i, d) = nature_pair(name).unwrap_or_else(|| panic!("{} has no pair", name));
            assert!(seen.insert((i, d)), "{} repeats a pair", name);
            assert_eq!(nature_for_pair(i, d), Some(name));
        }
        assert_eq!(seen.len(), 25);
    }

    #[test]
    fn neutral_natures_are_on_the_diagonal() {
        assert_eq!(nature_pair("Hardy"), Some(("attack", "attack")));
        assert_eq!(nature_pair("Docile"), Some(("defense", "defense")));
        assert_eq!(nature_pair("Serious"), Some(("speed", "speed")));
        assert_eq!(nature_pair("Bashful"), Some(("specialAttack", "specialAttack")));
        assert_eq!(nature_pair("Quirky"), Some(("specialDefense", "specialDefense")));
        assert_eq!(nature_for_pair("speed", "speed"), Some("Serious"));
    }

    #[test]
    fn pairs() {
        assert_eq!(nature_pair("Jolly"), Some(("speed", "specialAttack")));
        assert_eq!(nature_for_pair("attack", "specialAttack"), Some("Adamant"));
        assert_eq!(nature_for_pair("specialAttack", "attack"), Some("Modest"));
        assert_eq!(nature_pair("Nonsense"), None);
    }
}
