//! The Event diff tab (`docs/rust_port/design/route_compare/SPEC.md` §6.3):
//! two aligned lanes with the shared trainer fights as the fixed points.
//!
//! Rows are virtualised the way `route_list.rs` does it: every row is
//! allocated so the scrollbar is right, but only the visible ones are painted.

use egui::{Align, Rect, Sense, Ui, Vec2};

use xpr_engine::compare::{format_delta, format_time, BlockItem, DiffFilter, DiffRow, EntryKind, ItemStatus, RouteComparison};
use xpr_ui_kit::theme::Theme;
use xpr_ui_kit::widgets::{self, Side};

use super::shared::{self, Cmp};

const ANCHOR_H: f32 = 30.0;
const ITEM_H: f32 = 24.0;
const BLOCK_PAD: f32 = 6.0;
const GUTTER_W: f32 = 44.0;
const HEAD_H: f32 = 30.0;
const KIND_W: f32 = 52.0;

/// One laid-out row: its height and what it draws.
struct Row<'a> {
    height: f32,
    kind: RowKind<'a>,
}

enum RowKind<'a> {
    Anchor { a: usize, b: usize },
    Block { a: Vec<&'a BlockItem>, b: Vec<&'a BlockItem> },
}

pub fn ui(
    ui: &mut Ui,
    theme: &Theme,
    cmp: &RouteComparison,
    highlight: bool,
    filters: &mut Vec<DiffFilter>,
    scroll_to: &mut Option<String>,
    scroll_y: &mut f32,
) {
    let c = Cmp { theme, highlight };

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for f in DiffFilter::ALL {
            let on = filters.contains(&f);
            if widgets::seg_toggle(ui, theme, f.label(), on).clicked() {
                if on {
                    filters.retain(|x| *x != f);
                } else {
                    filters.push(f);
                }
            }
        }
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            legend(ui, theme);
        });
    });

    // Lay the rows out once, applying the filters.
    let rows = layout(cmp, filters);
    let total_h: f32 = rows.iter().map(|r| r.height).sum();

    // Keep the footer caption on screen: the lanes take what is left.
    const FOOTER_H: f32 = 22.0;
    let lanes_h = (ui.available_height() - FOOTER_H).max(120.0);
    egui::Frame::new()
        .fill(theme.card_bg())
        .stroke(egui::Stroke::new(1.0_f32, theme.card_border()))
        .corner_radius(egui::CornerRadius::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            head(ui, theme, cmp);
            let mut scroll = egui::ScrollArea::vertical()
                .id_salt("compare_diff")
                .auto_shrink([false, false])
                .max_height(lanes_h - HEAD_H - 2.0);
            // Jump to a fight the Trainers card asked for.
            if let Some(name) = scroll_to.take() {
                if let Some(y) = offset_of_trainer(cmp, &rows, &name) {
                    scroll = scroll.vertical_scroll_offset((y - 60.0).max(0.0));
                }
            }
            widgets::show_scroll(ui, scroll, |ui| {
                let (area, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), total_h.max(1.0)), Sense::hover());
                let clip = ui.clip_rect();
                // how far the rows are scrolled, for a capped export
                *scroll_y = (clip.min.y - area.min.y).max(0.0);
                let mut y = area.min.y;
                for row in &rows {
                    let rect = Rect::from_min_size(egui::Pos2::new(area.min.x, y), Vec2::new(area.width(), row.height));
                    y += row.height;
                    // Virtualisation: skip painting rows that are off screen.
                    if rect.max.y < clip.min.y || rect.min.y > clip.max.y {
                        continue;
                    }
                    match &row.kind {
                        RowKind::Anchor { a, b } => anchor_row(ui, &c, cmp, rect, *a, *b),
                        RowKind::Block { a, b } => block_row(ui, &c, rect, a, b),
                    }
                }
            });
        });

    ui.label(
        egui::RichText::new(format!(
            "{} shared fights \u{b7} {} blocks with differences",
            cmp.shared_fight_count(),
            cmp.blocks_with_differences()
        ))
        .font(theme.caption_font())
        .color(theme.secondary),
    );
}

/// Where the fight called `name` is drawn, measured from the top of the rows.
/// The Trainers card lists one-sided fights, and those sit *inside* blocks
/// (only shared fights are anchor rows), so both are searched, in either lane.
fn offset_of_trainer(cmp: &RouteComparison, rows: &[Row], name: &str) -> Option<f32> {
    let mut y = 0.0_f32;
    for row in rows {
        match &row.kind {
            RowKind::Anchor { a, b } => {
                if cmp.a.entries[*a].label == name || cmp.b.entries[*b].label == name {
                    return Some(y);
                }
            }
            RowKind::Block { a, b } => {
                for lane in [a, b] {
                    if let Some(i) = lane.iter().position(|item| item.kind == EntryKind::Trainer && item.merged_label == name) {
                        return Some(y + BLOCK_PAD / 2.0 + i as f32 * ITEM_H);
                    }
                }
            }
        }
        y += row.height;
    }
    None
}

/// Tallest diff an export will draw. A long route's diff runs to tens of
/// thousands of px; at 2 px per point that is a PNG of several hundred MB.
pub const MAX_EXPORT_H: f32 = 6_000.0;

/// The diff without a scroll viewport, for the screenshot export (SPEC §8.1):
/// everything when it fits in [`MAX_EXPORT_H`], otherwise that much of it
/// starting at the row the page is scrolled to.
pub fn export_ui(ui: &mut Ui, theme: &Theme, cmp: &RouteComparison, highlight: bool, filters: &[DiffFilter], scroll_y: f32) {
    let c = Cmp { theme, highlight };
    let mut rows = layout(cmp, filters);
    let full_h: f32 = rows.iter().map(|r| r.height).sum();
    if full_h > MAX_EXPORT_H {
        // drop the rows above the scroll position, then the ones past the cap
        let mut y = 0.0_f32;
        rows.retain(|r| {
            let keep = y + r.height > scroll_y;
            y += r.height;
            keep
        });
        let mut h = 0.0_f32;
        rows.retain(|r| {
            h += r.height;
            h <= MAX_EXPORT_H
        });
    }
    let total_h: f32 = rows.iter().map(|r| r.height).sum();
    egui::Frame::new()
        .fill(theme.card_bg())
        .stroke(egui::Stroke::new(1.0_f32, theme.card_border()))
        .corner_radius(egui::CornerRadius::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            head(ui, theme, cmp);
            let (area, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), total_h.max(1.0)), Sense::hover());
            let mut y = area.min.y;
            for row in &rows {
                let rect = Rect::from_min_size(egui::Pos2::new(area.min.x, y), Vec2::new(area.width(), row.height));
                y += row.height;
                match &row.kind {
                    RowKind::Anchor { a, b } => anchor_row(ui, &c, cmp, rect, *a, *b),
                    RowKind::Block { a, b } => block_row(ui, &c, rect, a, b),
                }
            }
        });
}

/// Filter the comparison's rows and measure each one.
fn layout<'a>(cmp: &'a RouteComparison, filters: &[DiffFilter]) -> Vec<Row<'a>> {
    let keep = |item: &&BlockItem| filters.contains(&item.kind.filter());
    let mut out = Vec::new();
    for row in &cmp.rows {
        match row {
            DiffRow::Anchor { a, b } => out.push(Row {
                height: ANCHOR_H,
                kind: RowKind::Anchor { a: *a, b: *b },
            }),
            DiffRow::Block { a, b } => {
                let a: Vec<&BlockItem> = a.iter().filter(keep).collect();
                let b: Vec<&BlockItem> = b.iter().filter(keep).collect();
                if a.is_empty() && b.is_empty() {
                    continue;
                }
                let lines = a.len().max(b.len()).max(1) as f32;
                out.push(Row {
                    height: BLOCK_PAD + lines * ITEM_H,
                    kind: RowKind::Block { a, b },
                });
            }
        }
    }
    out
}

fn legend(ui: &mut Ui, theme: &Theme) {
    let items: [(&str, Option<Side>); 4] = [
        ("only in A", Some(Side::A)),
        ("only in B", Some(Side::B)),
        ("grey = in both", None),
        ("a fight both routes have", None),
    ];
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 14.0;
        // The caller places this in a right-to-left layout, so the items are
        // added back to front to read naturally.
        for (label, side) in items.into_iter().rev() {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if let Some(side) = side {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(3.0, 12.0), Sense::hover());
                    ui.painter().rect_filled(rect, egui::CornerRadius::same(1), side.color(theme));
                }
                ui.label(egui::RichText::new(label).font(theme.caption_font()).color(theme.secondary));
            });
        }
    });
}

fn head(ui: &mut Ui, theme: &Theme, cmp: &RouteComparison) {
    let rect = widgets::table_row(ui, HEAD_H, None);
    ui.painter().rect_filled(
        Rect::from_min_max(rect.min, egui::Pos2::new(rect.max.x, rect.max.y)),
        egui::CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 },
        theme.strip_bg(),
    );
    ui.painter()
        .hline(rect.x_range(), rect.max.y - 0.5, egui::Stroke::new(1.0_f32, theme.pane_divider()));
    for (side, d, lane) in [(Side::A, &cmp.a, lane_rect(rect, true)), (Side::B, &cmp.b, lane_rect(rect, false))] {
        let badge = Rect::from_min_size(egui::Pos2::new(lane.min.x + 12.0, lane.center().y - 10.0), Vec2::splat(20.0));
        widgets::paint_route_badge(ui, theme, badge, side);
        let cell = Rect::from_min_max(egui::Pos2::new(badge.max.x + 8.0, lane.min.y), lane.max);
        let name = widgets::elide(ui, &d.label, &theme.body_bold(), cell.width() - 12.0);
        widgets::col_text(ui, cell, &name, theme.body_bold(), theme.text_strong(), Align::Min);
    }
}

/// The left or right lane of a row (the gutter sits between them).
fn lane_rect(row: Rect, left: bool) -> Rect {
    let lane_w = (row.width() - GUTTER_W) / 2.0;
    if left {
        Rect::from_min_size(row.min, Vec2::new(lane_w, row.height()))
    } else {
        Rect::from_min_size(egui::Pos2::new(row.max.x - lane_w, row.min.y), Vec2::new(lane_w, row.height()))
    }
}

fn gutter_rect(row: Rect) -> Rect {
    let lane_w = (row.width() - GUTTER_W) / 2.0;
    Rect::from_min_size(egui::Pos2::new(row.min.x + lane_w, row.min.y), Vec2::new(GUTTER_W, row.height()))
}

fn paint_gutter(ui: &Ui, theme: &Theme, row: Rect, mark: bool) {
    let g = gutter_rect(row);
    ui.painter()
        .vline(g.min.x, row.y_range(), egui::Stroke::new(1.0_f32, theme.row_divider()));
    ui.painter()
        .vline(g.max.x, row.y_range(), egui::Stroke::new(1.0_f32, theme.row_divider()));
    if mark {
        widgets::paint_equals_mark(ui, g, theme.secondary);
    }
}

fn anchor_row(ui: &mut Ui, c: &Cmp, cmp: &RouteComparison, rect: Rect, ia: usize, ib: usize) {
    let theme = c.theme;
    ui.painter().rect_filled(rect, egui::CornerRadius::ZERO, xpr_ui_kit::theme::lighten(theme.card_bg(), 0.02));
    ui.painter()
        .hline(rect.x_range(), rect.min.y + 0.5, egui::Stroke::new(1.0_f32, theme.pane_divider()));
    paint_gutter(ui, theme, rect, true);

    let (ea, eb) = (&cmp.a.entries[ia], &cmp.b.entries[ib]);
    // A lane: name, location, level, time.
    let lane = lane_rect(rect, true).shrink2(Vec2::new(12.0, 0.0));
    let right = anchor_right(ui, theme, lane, Side::A, ea.before.level, ea.recorded_secs, None);
    let mut left = lane.min.x;
    let name = widgets::elide(ui, &ea.label, &theme.body_bold(), (right - left - 8.0).max(40.0));
    let nw = widgets::text_width(ui, &name, &theme.body_bold());
    widgets::col_text(
        ui,
        Rect::from_min_max(egui::Pos2::new(left, lane.min.y), egui::Pos2::new(left + nw, lane.max.y)),
        &name,
        theme.body_bold(),
        theme.text_strong(),
        Align::Min,
    );
    left += nw + 8.0;
    if let Some(loc) = &ea.location {
        let avail = right - left - 8.0;
        if avail > 30.0 {
            let loc = widgets::elide(ui, loc, &theme.body(), avail);
            widgets::col_text(
                ui,
                Rect::from_min_max(egui::Pos2::new(left, lane.min.y), egui::Pos2::new(right - 8.0, lane.max.y)),
                &loc,
                theme.body(),
                theme.secondary,
                Align::Min,
            );
        }
    }

    // B lane: name plus the time delta.
    let lane = lane_rect(rect, false).shrink2(Vec2::new(12.0, 0.0));
    let delta = match (ea.recorded_secs, eb.recorded_secs) {
        (Some(ta), Some(tb)) => Some(tb - ta),
        _ => None,
    };
    let right = anchor_right(ui, theme, lane, Side::B, eb.before.level, eb.recorded_secs, delta);
    let name = widgets::elide(ui, &eb.label, &theme.body_bold(), (right - lane.min.x - 8.0).max(40.0));
    widgets::col_text(
        ui,
        Rect::from_min_max(lane.min, egui::Pos2::new(right - 8.0, lane.max.y)),
        &name,
        theme.body_bold(),
        theme.text_strong(),
        Align::Min,
    );
}

/// The right-aligned `[delta] Lv N  time` block of an anchor lane. Returns
/// the x where it starts, so the name can be elided to fit.
fn anchor_right(ui: &mut Ui, theme: &Theme, lane: Rect, side: Side, level: i64, time: Option<f64>, delta: Option<f64>) -> f32 {
    let time_text = time.map(format_time).unwrap_or_default();
    let lv_text = format!("Lv {}", level);
    let tw = widgets::text_width(ui, &time_text, &theme.body());
    let lw = widgets::text_width(ui, &lv_text, &theme.body_bold());
    let mut x = lane.max.x;
    if !time_text.is_empty() {
        widgets::col_text(ui, lane, &time_text, theme.body(), theme.secondary, Align::Max);
        x -= tw + 10.0;
    }
    let lv_cell = Rect::from_min_max(egui::Pos2::new(x - lw, lane.min.y), egui::Pos2::new(x, lane.max.y));
    widgets::col_text(ui, lv_cell, &lv_text, theme.body_bold(), side.color(theme), Align::Max);
    x -= lw + 10.0;
    if let Some(delta) = delta {
        if delta.abs() >= 0.05 {
            let text = format_delta(delta);
            let dw = widgets::text_width(ui, &text, &theme.caption_font());
            let cell = Rect::from_min_max(egui::Pos2::new(x - dw, lane.min.y), egui::Pos2::new(x, lane.max.y));
            widgets::col_text(ui, cell, &text, theme.caption_font(), shared::time_delta_color(theme, delta), Align::Max);
            x -= dw + 10.0;
        }
    }
    x
}

fn block_row(ui: &mut Ui, c: &Cmp, rect: Rect, a: &[&BlockItem], b: &[&BlockItem]) {
    let theme = c.theme;
    ui.painter()
        .hline(rect.x_range(), rect.min.y + 0.5, egui::Stroke::new(1.0_f32, theme.row_divider()));
    paint_gutter(ui, theme, rect, false);
    for (items, side, left) in [(a, Side::A, true), (b, Side::B, false)] {
        let lane = lane_rect(rect, left).shrink2(Vec2::new(12.0, 0.0));
        if items.is_empty() {
            let cell = Rect::from_min_size(egui::Pos2::new(lane.min.x, lane.min.y + BLOCK_PAD / 2.0), Vec2::new(lane.width(), ITEM_H));
            widgets::col_text(ui, cell, "\u{2014}", theme.body(), theme.disabled_text, Align::Min);
            continue;
        }
        for (i, item) in items.iter().enumerate() {
            let cell = Rect::from_min_size(
                egui::Pos2::new(lane.min.x, lane.min.y + BLOCK_PAD / 2.0 + i as f32 * ITEM_H),
                Vec2::new(lane.width(), ITEM_H),
            );
            item_line(ui, c, cell, item, side);
        }
    }
}

fn item_line(ui: &mut Ui, c: &Cmp, cell: Rect, item: &BlockItem, side: Side) {
    let theme = c.theme;
    let one_sided = !matches!(item.status, ItemStatus::InBoth | ItemStatus::QtyDiffers { .. });
    let emphasise = one_sided && c.highlight;

    // A 3 px marker in the route colour for anything only this route has.
    if emphasise {
        let marker = Rect::from_min_size(egui::Pos2::new(cell.min.x - 8.0, cell.center().y - 6.0), Vec2::new(3.0, 12.0));
        ui.painter().rect_filled(marker, egui::CornerRadius::same(1), side.color(theme));
    }

    let kind_color = if emphasise { side.color(theme) } else { theme.disabled_text };
    widgets::col_text(
        ui,
        Rect::from_min_size(cell.min, Vec2::new(KIND_W, cell.height())),
        item.kind.caption(),
        theme.caption_font(),
        kind_color,
        Align::Min,
    );

    let (text_color, font) = if emphasise {
        (theme.text_strong(), theme.body_bold())
    } else if one_sided {
        (theme.text, theme.body())
    } else {
        (theme.secondary, theme.body())
    };

    // Trailing note: quantity, or why a trainer is out of place.
    let trailing = match &item.status {
        ItemStatus::QtyDiffers { other_qty } => format!("\u{d7}{} vs \u{d7}{}", item.qty, other_qty),
        _ if item.qty > 1 => format!("\u{d7}{}", item.qty),
        _ => String::new(),
    };
    let tw = if trailing.is_empty() { 0.0 } else { widgets::text_width(ui, &trailing, &theme.caption_font()) + 8.0 };
    if !trailing.is_empty() {
        widgets::col_text(ui, cell, &trailing, theme.caption_font(), theme.secondary, Align::Max);
    }

    let note = match &item.status {
        ItemStatus::TrainerMoved { other_after } => {
            let other = if side == Side::A { "B" } else { "A" };
            match other_after {
                Some(name) => format!("  moved: {} fights them after {}", other, name),
                None => format!("  moved: {} fights them at the start", other),
            }
        }
        ItemStatus::TrainerRepeat => "  rematch".to_string(),
        _ => String::new(),
    };

    let name_left = cell.min.x + KIND_W;
    let avail = (cell.max.x - tw - name_left).max(20.0);
    let nw = widgets::text_width(ui, &item.merged_label, &font).min(avail);
    let label = widgets::elide(ui, &item.merged_label, &font, avail);
    widgets::col_text(
        ui,
        Rect::from_min_max(egui::Pos2::new(name_left, cell.min.y), egui::Pos2::new(name_left + nw, cell.max.y)),
        &label,
        font,
        text_color,
        Align::Min,
    );
    if !note.is_empty() {
        let left = name_left + nw;
        let note_avail = cell.max.x - tw - left;
        if note_avail > 30.0 {
            let note = widgets::elide(ui, &note, &theme.caption_font(), note_avail);
            widgets::col_text(
                ui,
                Rect::from_min_max(egui::Pos2::new(left, cell.min.y), egui::Pos2::new(cell.max.x - tw, cell.max.y)),
                &note,
                theme.caption_font(),
                theme.secondary,
                Align::Min,
            );
        }
    }
}
