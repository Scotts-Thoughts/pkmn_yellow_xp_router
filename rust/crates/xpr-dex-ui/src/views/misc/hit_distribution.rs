//! The shared results display of a binomial hit distribution (Solodex
//! `misc/HitDistribution.tsx`): the three headline framings (exactly / at least
//! / at most K), expected hits, a bar chart over 0..=n and an optional
//! cumulative table. Presentation only; the numbers come from
//! [`super::hit_probability`].

use egui::{
    Align2, Color32, CornerRadius, FontId, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2,
};
use xpr_ui_kit::theme::Theme;

use super::controls::{
    accent_row_tint, accent_tint, caps_label_gray500, card, card_fill, rich_line, stat_card_fill,
    MISC_ACCENT,
};
use super::format::{number, percent};
use super::hit_probability::{at_least, at_most, distribution, expected_hits, std_deviation};
use crate::palette::{self, px, px_bold};

#[derive(Default)]
pub struct HitDistState {
    pub show_table: bool,
}

/// One headline card (`StatCard`): label, big value, mono formula.
pub fn stat_card(
    ui: &mut Ui,
    theme: &Theme,
    rect: Rect,
    label: &str,
    value: &str,
    sub: &str,
    highlight: bool,
    value_font: FontId,
) {
    let (border, fill) = if highlight {
        (MISC_ACCENT, accent_tint())
    } else {
        (palette::GRAY_700, stat_card_fill())
    };
    let p = ui.painter();
    p.rect(
        rect,
        CornerRadius::same(8),
        fill,
        Stroke::new(1.0_f32, border),
        StrokeKind::Inside,
    );
    let x = rect.min.x + 13.0;
    let mut y = rect.min.y + 13.0;
    p.text(
        Pos2::new(x, y),
        Align2::LEFT_TOP,
        label,
        px(theme, 12.0),
        palette::GRAY_400,
    );
    y += 18.0;
    p.text(
        Pos2::new(x, y),
        Align2::LEFT_TOP,
        value,
        value_font,
        Color32::WHITE,
    );
    y += 34.0;
    p.text(
        Pos2::new(x, y),
        Align2::LEFT_TOP,
        sub,
        FontId::monospace(10.0),
        palette::GRAY_500,
    );
}

/// Draws the results for `n` uses at hit probability `p` with target `k`;
/// returns a `k` the user picked by clicking a bar.
pub fn hit_distribution(
    ui: &mut Ui,
    theme: &Theme,
    id: Id,
    st: &mut HitDistState,
    n: i64,
    p: f64,
    k: i64,
) -> Option<i64> {
    let dist = distribution(n, p);
    let max_p = dist.iter().cloned().fold(0.0_f64, f64::max);
    let least = at_least(k, n, p);
    let most = at_most(k, n, p);
    let exactly = dist.get(k as usize).copied().unwrap_or(0.0);
    let ev = expected_hits(n, p);
    let sd = std_deviation(n, p);
    let mut picked = None;
    let width = ui.available_width();

    // headline framings: grid-cols-3 gap-3
    let (row, _) = ui.allocate_exact_size(Vec2::new(width, 91.0), Sense::hover());
    let cw = (width - 24.0) / 3.0;
    let cards = [
        (
            format!("Exactly {}", k),
            percent(exactly),
            format!("P(X = {})", k),
            true,
        ),
        (
            format!("At least {}", k),
            percent(least),
            format!("P(X \u{2265} {})", k),
            false,
        ),
        (
            format!("At most {}", k),
            percent(most),
            format!("P(X \u{2264} {})", k),
            false,
        ),
    ];
    for (i, (label, value, sub, hl)) in cards.iter().enumerate() {
        let r = Rect::from_min_size(
            Pos2::new(row.min.x + i as f32 * (cw + 12.0), row.min.y),
            Vec2::new(cw, 91.0),
        );
        stat_card(ui, theme, r, label, value, sub, *hl, px_bold(theme, 24.0));
    }
    ui.add_space(16.0);

    // summary stats
    let bold = px_bold(theme, 14.0);
    let reg = px(theme, 14.0);
    let g1 = rich_line(
        ui,
        &[
            ("Expected hits: ", reg.clone(), palette::GRAY_400),
            (&number(ev), bold, Color32::WHITE),
            (&format!(" of {}", n), reg.clone(), palette::GRAY_500),
        ],
    );
    let g2 = rich_line(
        ui,
        &[
            ("\u{03C3} = ", reg.clone(), palette::GRAY_500),
            (&number(sd), reg, palette::GRAY_500),
        ],
    );
    let (line, _) = ui.allocate_exact_size(Vec2::new(width, 20.0), Sense::hover());
    let x2 = line.min.x + g1.size().x + 20.0;
    ui.painter().galley(
        Pos2::new(line.min.x, line.center().y - g1.size().y / 2.0),
        g1,
        Color32::WHITE,
    );
    ui.painter().galley(
        Pos2::new(x2, line.center().y - g2.size().y / 2.0),
        g2,
        Color32::WHITE,
    );
    ui.add_space(16.0);

    // distribution bar chart
    card(card_fill(), palette::GRAY_700, 12).show(ui, |ui| {
        ui.set_width(ui.available_width());
        caps_label_gray500(ui, theme, "Distribution \u{2014} P(X = i)");
        ui.add_space(8.0);
        let area = egui::ScrollArea::vertical()
            .id_salt(id.with("bars"))
            .max_height(360.0)
            .auto_shrink([false, true]);
        xpr_ui_kit::widgets::show_scroll(ui, area, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            let w = ui.available_width() - 4.0;
            for (i, prob) in dist.iter().enumerate() {
                let selected = i as i64 == k;
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 20.0), Sense::click());
                let pct = percent(*prob);
                let resp = resp
                    .on_hover_text(format!("P(X = {}) = {}", i, pct))
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                let p = ui.painter();
                let idx_font = if selected {
                    px_bold(theme, 12.0)
                } else {
                    px(theme, 12.0)
                };
                p.text(
                    Pos2::new(rect.min.x + 24.0, rect.center().y),
                    Align2::RIGHT_CENTER,
                    i.to_string(),
                    idx_font,
                    if selected {
                        MISC_ACCENT
                    } else {
                        palette::GRAY_500
                    },
                );
                let bar = Rect::from_min_max(
                    Pos2::new(rect.min.x + 32.0, rect.min.y),
                    Pos2::new((rect.max.x - 88.0).max(rect.min.x + 33.0), rect.max.y),
                );
                p.rect_filled(bar, CornerRadius::same(4), palette::GRAY_800);
                let frac = if max_p > 0.0 {
                    (prob / max_p) as f32
                } else {
                    0.0
                };
                let mut fw = bar.width() * frac;
                if *prob > 0.0 {
                    fw = fw.max(2.0);
                }
                if fw > 0.0 {
                    let inner =
                        Rect::from_min_size(bar.min, Vec2::new(fw.min(bar.width()), bar.height()));
                    p.rect_filled(
                        inner,
                        CornerRadius::same(4),
                        if selected {
                            MISC_ACCENT
                        } else {
                            palette::GRAY_600
                        },
                    );
                }
                let pct_font = if selected {
                    px_bold(theme, 12.0)
                } else {
                    px(theme, 12.0)
                };
                p.text(
                    Pos2::new(rect.max.x, rect.center().y),
                    Align2::RIGHT_CENTER,
                    pct,
                    pct_font,
                    if selected {
                        Color32::WHITE
                    } else {
                        palette::GRAY_400
                    },
                );
                if resp.clicked() {
                    picked = Some(i as i64);
                }
            }
        });
    });
    ui.add_space(16.0);

    // optional cumulative table
    let toggle_text = if st.show_table {
        "Hide full table"
    } else {
        "Show full table"
    };
    let tfont = px(theme, 12.0);
    let tw = rich_line(ui, &[(toggle_text, tfont.clone(), palette::GRAY_400)])
        .size()
        .x;
    let (trect, tresp) = ui.allocate_exact_size(Vec2::new(tw + 14.0, 16.0), Sense::click());
    let tcolor = if tresp.hovered() {
        Color32::WHITE
    } else {
        palette::GRAY_400
    };
    // the disclosure triangle (the font has no small triangles)
    let c = Pos2::new(trect.min.x + 4.0, trect.center().y);
    let tri = if st.show_table {
        vec![
            Pos2::new(c.x - 3.5, c.y - 2.0),
            Pos2::new(c.x + 3.5, c.y - 2.0),
            Pos2::new(c.x, c.y + 3.0),
        ]
    } else {
        vec![
            Pos2::new(c.x - 2.5, c.y - 3.5),
            Pos2::new(c.x - 2.5, c.y + 3.5),
            Pos2::new(c.x + 2.5, c.y),
        ]
    };
    ui.painter()
        .add(egui::Shape::convex_polygon(tri, tcolor, Stroke::NONE));
    ui.painter().text(
        Pos2::new(trect.min.x + 14.0, trect.center().y),
        Align2::LEFT_CENTER,
        toggle_text,
        tfont,
        tcolor,
    );
    if tresp
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        st.show_table = !st.show_table;
    }
    if st.show_table {
        ui.add_space(8.0);
        let heads = ["Hits k", "P(X = k)", "P(X \u{2265} k)", "P(X \u{2264} k)"];
        let cw = ui.available_width() / 4.0;
        let cell = |ui: &Ui,
                    rect: Rect,
                    text: &str,
                    font: FontId,
                    color: Color32,
                    fill: Option<Color32>| {
            if let Some(f) = fill {
                ui.painter().rect_filled(rect, CornerRadius::ZERO, f);
            }
            ui.painter().rect_stroke(
                rect,
                CornerRadius::ZERO,
                Stroke::new(1.0_f32, palette::GRAY_700),
                StrokeKind::Inside,
            );
            ui.painter().text(
                Pos2::new(rect.max.x - 9.0, rect.center().y),
                Align2::RIGHT_CENTER,
                text,
                font,
                color,
            );
        };
        ui.spacing_mut().item_spacing.y = 0.0;
        let (hrow, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 25.0), Sense::hover());
        for (c, h) in heads.iter().enumerate() {
            let r = Rect::from_min_size(
                Pos2::new(hrow.min.x + c as f32 * cw, hrow.min.y),
                Vec2::new(cw, 25.0),
            );
            cell(ui, r, h, px_bold(theme, 12.0), palette::GRAY_500, None);
        }
        for (i, prob) in dist.iter().enumerate() {
            let (rrow, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 25.0), Sense::hover());
            let fill = if i as i64 == k {
                Some(accent_row_tint())
            } else {
                None
            };
            let vals = [
                (i.to_string(), palette::GRAY_300),
                (percent(*prob), palette::GRAY_200),
                (percent(at_least(i as i64, n, p)), palette::GRAY_400),
                (percent(at_most(i as i64, n, p)), palette::GRAY_400),
            ];
            for (c, (v, color)) in vals.iter().enumerate() {
                let r = Rect::from_min_size(
                    Pos2::new(rrow.min.x + c as f32 * cw, rrow.min.y),
                    Vec2::new(cw, 25.0),
                );
                cell(ui, r, v, px(theme, 12.0), *color, fill);
            }
        }
    }
    picked
}
