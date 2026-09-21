//! The Checkpoints tab (`docs/rust_port/design/route_compare/SPEC.md` §6.2):
//! where each run stood before every fight the two routes share.

use std::collections::HashMap;

use egui::{Align, Color32, Rect, Sense, Ui, Vec2};

use xpr_engine::compare::{format_delta, format_time, Checkpoint, RouteComparison};
use xpr_ui_kit::theme::Theme;
use xpr_ui_kit::widgets::{self, Side};

use super::shared::{self, Cmp, ROW_H};

/// Column widths, right to left.
const W_LV: f32 = 46.0;
const W_DELTA: f32 = 52.0;
const W_TIME: f32 = 74.0;
const W_VS: f32 = 82.0;
const W_MONEY: f32 = 78.0;
const W_EV: f32 = 52.0;
const W_MOVES: f32 = 76.0;

struct Cols {
    lv_a: f32,
    lv_b: f32,
    lv_d: f32,
    time_a: f32,
    time_b: f32,
    vs: f32,
    money_a: f32,
    money_b: f32,
    ev_a: f32,
    ev_b: f32,
    moves: f32,
    show_times: bool,
}

impl Cols {
    /// Lay the columns out from the right edge; the fight name takes the rest.
    fn new(right: f32, show_times: bool) -> Cols {
        let mut x = right;
        let moves = x;
        x -= W_MOVES;
        let ev_b = x;
        x -= W_EV;
        let ev_a = x;
        x -= W_EV;
        let money_b = x;
        x -= W_MONEY;
        let money_a = x;
        x -= W_MONEY;
        let (time_a, time_b, vs) = if show_times {
            let vs = x;
            x -= W_VS;
            let tb = x;
            x -= W_TIME;
            let ta = x;
            x -= W_TIME;
            (ta, tb, vs)
        } else {
            (x, x, x)
        };
        let lv_d = x;
        x -= W_DELTA;
        let lv_b = x;
        x -= W_LV;
        let lv_a = x;
        Cols {
            lv_a,
            lv_b,
            lv_d,
            time_a,
            time_b,
            vs,
            money_a,
            money_b,
            ev_a,
            ev_b,
            moves,
            show_times,
        }
    }

    fn cell(right: f32, width: f32, row: Rect) -> Rect {
        Rect::from_min_max(egui::Pos2::new(right - width, row.min.y), egui::Pos2::new(right, row.max.y))
    }

    fn name(&self, row: Rect) -> Rect {
        Rect::from_min_max(row.min, egui::Pos2::new(self.lv_a - W_LV - 8.0, row.max.y))
    }
}

pub fn ui(
    ui: &mut Ui,
    theme: &Theme,
    cfg: &xpr_core::Config,
    cmp: &RouteComparison,
    highlight: bool,
    major_only: &mut bool,
    expanded: &mut Option<usize>,
) {
    let c = Cmp { theme, highlight };
    let shared_count = cmp.checkpoints.len();
    let shown: Vec<&Checkpoint> = cmp
        .checkpoints
        .iter()
        .filter(|cp| !*major_only || cmp.a.entries[cp.a].is_major || cmp.b.entries[cp.b].is_major)
        .collect();

    // toolbar
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        if widgets::seg_toggle(ui, theme, "Major fights", *major_only).clicked() {
            *major_only = true;
            *expanded = None;
        }
        if widgets::seg_toggle(ui, theme, &format!("All shared trainers ({})", shared_count), !*major_only).clicked() {
            *major_only = false;
            *expanded = None;
        }
        ui.label(
            egui::RichText::new("State before each fight both routes have. Click a row for stats and moves.")
                .font(theme.caption_font())
                .color(theme.secondary),
        );
    });

    if shown.is_empty() {
        widgets::card(ui, theme, None, |ui| {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                let text = if shared_count == 0 {
                    "These routes have no trainer fights in common."
                } else {
                    "Neither route has a major fight in common. Switch to all shared trainers."
                };
                ui.label(egui::RichText::new(text).font(theme.body()).color(theme.secondary));
            });
            ui.add_space(40.0);
        });
        return;
    }

    let colors = shared::fight_category_colors(cfg);
    widgets::card(ui, theme, Some(egui::Margin { left: 14, right: 14, top: 10, bottom: 10 }), |ui| {
        // A few px short of the edge so the last column clears the card's
        // rounding and the body scrollbar.
        let width = ui.available_width() - 6.0;
        let cols = Cols::new(ui.min_rect().min.x + width, cmp.both_have_times);
        head_row(ui, theme, &cols, cmp);
        for cp in shown {
            row(ui, &c, &cols, cmp, cp, expanded, &colors);
        }
    });
}

fn head_row(ui: &mut Ui, theme: &Theme, cols: &Cols, cmp: &RouteComparison) {
    let rect = widgets::table_row(ui, 18.0, None);
    let cap = theme.caption_font();
    let a_col = Side::A.color(theme);
    let b_col = Side::B.color(theme);
    widgets::col_text(ui, cols.name(rect), "Fight", cap.clone(), theme.secondary, Align::Min);
    widgets::col_text(ui, Cols::cell(cols.lv_a, W_LV, rect), "Lv A", theme.caption_font_bold(), a_col, Align::Max);
    widgets::col_text(ui, Cols::cell(cols.lv_b, W_LV, rect), "Lv B", theme.caption_font_bold(), b_col, Align::Max);
    widgets::col_text(ui, Cols::cell(cols.lv_d, W_DELTA, rect), "B \u{2212} A", cap.clone(), theme.secondary, Align::Max);
    if cols.show_times {
        widgets::col_text(ui, Cols::cell(cols.time_a, W_TIME, rect), "Time A", theme.caption_font_bold(), a_col, Align::Max);
        widgets::col_text(ui, Cols::cell(cols.time_b, W_TIME, rect), "Time B", theme.caption_font_bold(), b_col, Align::Max);
        widgets::col_text(ui, Cols::cell(cols.vs, W_VS, rect), "A vs B", cap.clone(), theme.secondary, Align::Max);
    }
    widgets::col_text(ui, Cols::cell(cols.money_a, W_MONEY, rect), "Money A", theme.caption_font_bold(), a_col, Align::Max);
    widgets::col_text(ui, Cols::cell(cols.money_b, W_MONEY, rect), "Money B", theme.caption_font_bold(), b_col, Align::Max);
    let ev = cmp.a.ev_text();
    widgets::col_text(ui, Cols::cell(cols.ev_a, W_EV, rect), &format!("{} A", ev), theme.caption_font_bold(), a_col, Align::Max);
    widgets::col_text(ui, Cols::cell(cols.ev_b, W_EV, rect), &format!("{} B", ev), theme.caption_font_bold(), b_col, Align::Max);
    widgets::col_text(ui, Cols::cell(cols.moves, W_MOVES, rect), "Moves", cap, theme.secondary, Align::Max);
}

fn row(
    ui: &mut Ui,
    c: &Cmp,
    cols: &Cols,
    cmp: &RouteComparison,
    cp: &Checkpoint,
    expanded: &mut Option<usize>,
    colors: &HashMap<&'static str, Color32>,
) {
    let theme = c.theme;
    let (ea, eb) = (&cmp.a.entries[cp.a], &cmp.b.entries[cp.b]);
    let rect = widgets::table_row(ui, ROW_H, Some(theme.row_divider()));
    let is_open = *expanded == Some(cp.a);
    let resp = ui.interact(rect, ui.id().with(("cp", cp.a)), Sense::click());
    if resp.hovered() || is_open {
        ui.painter().rect_filled(rect, egui::CornerRadius::ZERO, theme.hover_bg);
    }
    if resp.clicked() {
        *expanded = if is_open { None } else { Some(cp.a) };
    }

    // name + category swatch
    let name_cell = cols.name(rect);
    shared::category_swatch(
        ui,
        theme,
        egui::Pos2::new(name_cell.min.x + 4.0, rect.center().y),
        ea.fight_category.and_then(|t| colors.get(t).copied()),
    );
    let text_left = name_cell.min.x + 16.0;
    let name = ea.label.clone();
    let avail = name_cell.max.x - text_left - if cp.same_order { 0.0 } else { 18.0 };
    let name = widgets::elide(ui, &name, &theme.body_bold(), avail);
    widgets::col_text(
        ui,
        Rect::from_min_max(egui::Pos2::new(text_left, rect.min.y), name_cell.max),
        &name,
        theme.body_bold(),
        theme.text_strong(),
        Align::Min,
    );
    if !cp.same_order {
        // A painted marker: the up/down arrow glyph is not in every font.
        let nw = widgets::text_width(ui, &name, &theme.body_bold());
        let mark = Rect::from_center_size(egui::Pos2::new(text_left + nw + 9.0, rect.center().y), Vec2::splat(12.0));
        widgets::paint_moved_mark(ui, mark, theme.secondary);
        resp.clone().on_hover_text("Fought in a different order in B");
    }

    // levels
    let (la, lb) = (ea.before.level, eb.before.level);
    let differs = c.diff(la != lb);
    let (font, color) = if differs {
        (theme.font_bold(10.5), theme.text_strong())
    } else {
        (theme.body(), theme.secondary)
    };
    widgets::col_text(ui, Cols::cell(cols.lv_a, W_LV, rect), &la.to_string(), font.clone(), color, Align::Max);
    widgets::col_text(ui, Cols::cell(cols.lv_b, W_LV, rect), &lb.to_string(), font, color, Align::Max);
    shared::delta_cell(ui, c, Cols::cell(cols.lv_d, W_DELTA, rect), lb - la);

    // times
    if cols.show_times {
        let ta = ea.recorded_secs;
        let tb = eb.recorded_secs;
        widgets::col_text(
            ui,
            Cols::cell(cols.time_a, W_TIME, rect),
            &ta.map(format_time).unwrap_or_else(|| "\u{2014}".into()),
            theme.body(),
            theme.text,
            Align::Max,
        );
        widgets::col_text(
            ui,
            Cols::cell(cols.time_b, W_TIME, rect),
            &tb.map(format_time).unwrap_or_else(|| "\u{2014}".into()),
            theme.body(),
            theme.text,
            Align::Max,
        );
        if let (Some(ta), Some(tb)) = (ta, tb) {
            // A vs B: positive means A took longer, so B is ahead.
            let delta = ta - tb;
            let cell = Cols::cell(cols.vs, W_VS, rect);
            widgets::col_text(ui, cell, &format_delta(delta), theme.body_bold(), shared::time_delta_color(theme, delta), Align::Max);
            let word = if delta > 0.0 { "later" } else { "earlier" };
            ui.interact(cell, ui.id().with(("cp_time", cp.a)), Sense::hover())
                .on_hover_text(format!("A reached this fight {} {} than B", format_time(delta.abs()), word));
        }
    }

    widgets::col_text(ui, Cols::cell(cols.money_a, W_MONEY, rect), &shared::money(ea.before.money), theme.body(), theme.text, Align::Max);
    widgets::col_text(ui, Cols::cell(cols.money_b, W_MONEY, rect), &shared::money(eb.before.money), theme.body(), theme.text, Align::Max);

    let (va, vb) = (ea.before.ev_total(), eb.before.ev_total());
    let ev_color = if va == vb { theme.secondary } else { theme.text };
    widgets::col_text(ui, Cols::cell(cols.ev_a, W_EV, rect), &va.to_string(), theme.body(), ev_color, Align::Max);
    widgets::col_text(ui, Cols::cell(cols.ev_b, W_EV, rect), &vb.to_string(), theme.body(), ev_color, Align::Max);

    let (moves_text, moves_color) = if cp.moves_differing == 0 {
        ("same".to_string(), theme.secondary)
    } else {
        (format!("{} differ", cp.moves_differing), theme.text_strong())
    };
    widgets::col_text(ui, Cols::cell(cols.moves, W_MOVES, rect), &moves_text, theme.body(), moves_color, Align::Max);

    if is_open {
        detail(ui, c, cmp, cp);
    }
}

/// The expanded panel: a mini stats table plus each route's moves.
fn detail(ui: &mut Ui, c: &Cmp, cmp: &RouteComparison, cp: &Checkpoint) {
    let theme = c.theme;
    let (ea, eb) = (&cmp.a.entries[cp.a], &cmp.b.entries[cp.b]);
    egui::Frame::new()
        .fill(theme.well_bg())
        .inner_margin(egui::Margin { left: 14, right: 14, top: 12, bottom: 12 })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 20.0;
                let total = ui.available_width();
                let stats_w = (total * 0.46).max(260.0);
                let moves_w = ((total - stats_w - 40.0) / 2.0).max(120.0);

                ui.allocate_ui_with_layout(Vec2::new(stats_w, 0.0), egui::Layout::top_down(Align::Min), |ui| {
                    ui.set_width(stats_w);
                    let title = widgets::table_row(ui, 18.0, None);
                    let label = format!("Before {}", ea.label);
                    let label = widgets::elide(ui, &label, &theme.caption_font(), title.width() - 200.0);
                    widgets::col_text(ui, title, &label, theme.caption_font(), theme.secondary, Align::Min);
                    let trio = shared::mini_trio(title.max.x - 4.0);
                    shared::mini_heads(ui, theme, &trio, cmp.a.ev_text());
                    for (name, idx) in super::overview::stat_rows(cmp.a.generation) {
                        let rect = widgets::table_row(ui, 24.0, Some(theme.row_divider()));
                        shared::name_cell(ui, theme, rect, name, false);
                        let (sa, sb) = (ea.before.stats[idx], eb.before.stats[idx]);
                        shared::value_pair(ui, c, rect, &trio.stats, sa, sb);
                        shared::value_pair_plain(ui, c, rect, &trio.evs, ea.before.evs[idx], eb.before.evs[idx]);
                    }
                });

                let sets = (ea.before.move_set(), eb.before.move_set());
                for (side, snap, other) in [(Side::A, &ea.before, &sets.1), (Side::B, &eb.before, &sets.0)] {
                    ui.allocate_ui_with_layout(Vec2::new(moves_w, 0.0), egui::Layout::top_down(Align::Min), |ui| {
                        ui.set_width(moves_w);
                        let rect = widgets::table_row(ui, 18.0, None);
                        let badge = Rect::from_min_size(egui::Pos2::new(rect.min.x, rect.center().y - 9.0), Vec2::splat(18.0));
                        widgets::paint_route_badge(ui, theme, badge, side);
                        let held = format!("Held: {}", snap.held_item.clone().unwrap_or_else(|| "none".into()));
                        widgets::col_text(
                            ui,
                            Rect::from_min_max(egui::Pos2::new(badge.max.x + 8.0, rect.min.y), rect.max),
                            &held,
                            theme.caption_font(),
                            theme.secondary,
                            Align::Min,
                        );
                        for (i, m) in snap.moves.iter().enumerate() {
                            let rect = widgets::table_row(ui, 24.0, Some(theme.row_divider()));
                            widgets::col_text(
                                ui,
                                Rect::from_min_size(rect.min, Vec2::new(14.0, rect.height())),
                                &format!("{}", i + 1),
                                theme.caption_font(),
                                theme.secondary,
                                Align::Min,
                            );
                            let cell = Rect::from_min_max(egui::Pos2::new(rect.min.x + 20.0, rect.min.y), rect.max);
                            match m.as_deref().filter(|s| !s.is_empty()) {
                                Some(name) => {
                                    let shared_move = other.contains(name);
                                    let (font, color) = if shared_move || !c.highlight {
                                        (theme.body(), theme.text)
                                    } else {
                                        (theme.body_bold(), theme.text_strong())
                                    };
                                    widgets::col_text(ui, cell, name, font, color, Align::Min);
                                }
                                None => widgets::col_text(ui, cell, "\u{2014}", theme.body(), theme.secondary, Align::Min),
                            }
                        }
                    });
                }
            });
        });
}
