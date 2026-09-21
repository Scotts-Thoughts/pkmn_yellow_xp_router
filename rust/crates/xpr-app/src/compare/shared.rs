//! Column geometry and cell painters shared by the compare tabs.
//!
//! Every table on the page follows SPEC §3.3: equal values are muted, values
//! that differ are bold `text_strong`, and the Δ column is always `B − A`.

use egui::{Align, Color32, Rect, Ui, Vec2};

use xpr_ui_kit::theme::Theme;
use xpr_ui_kit::widgets::{self, Side};

/// Height of an ordinary table row.
pub const ROW_H: f32 = 27.0;
/// Height of a group-heading row.
pub const GROUP_H: f32 = 30.0;

const DELTA_W: f32 = 64.0;
const BARS_W: f32 = 120.0;
const VALUE_W: f32 = 56.0;

/// Theme plus the "Highlight differences" switch.
#[derive(Clone, Copy)]
pub struct Cmp<'a> {
    pub theme: &'a Theme,
    pub highlight: bool,
}

impl Cmp<'_> {
    /// Whether a difference should be drawn as one.
    pub fn diff(&self, differs: bool) -> bool {
        differs && self.highlight
    }
}

/// An A / B / Δ column trio, measured from the right edge of a row.
pub struct Trio {
    pub a_right: f32,
    pub b_right: f32,
    pub delta_right: f32,
    pub width: f32,
    pub delta_width: f32,
}

impl Trio {
    fn new(row_right: f32, value_w: f32, delta_w: f32) -> Trio {
        let delta_right = row_right;
        let b_right = delta_right - delta_w;
        let a_right = b_right - value_w;
        Trio {
            a_right,
            b_right,
            delta_right,
            width: value_w,
            delta_width: delta_w,
        }
    }

    fn cell(right: f32, width: f32, row: Rect) -> Rect {
        Rect::from_min_max(egui::Pos2::new(right - width, row.min.y), egui::Pos2::new(right, row.max.y))
    }

    pub fn a(&self, row: Rect) -> Rect {
        Trio::cell(self.a_right, self.width, row)
    }

    pub fn b(&self, row: Rect) -> Rect {
        Trio::cell(self.b_right, self.width, row)
    }

    pub fn delta(&self, row: Rect) -> Rect {
        Trio::cell(self.delta_right, self.delta_width, row)
    }

    /// Where the name column ends.
    pub fn name_right(&self) -> f32 {
        self.a_right - self.width
    }
}

/// Paint the `A` / `B` / `B − A` heads over the title row, and return the
/// column geometry. `left_label` is drawn at the far left (e.g. `IVs`).
pub fn three_col_heads(ui: &mut Ui, theme: &Theme, title: Rect, left_label: &str) -> Trio {
    let trio = Trio::new(title.max.x, 72.0, DELTA_W);
    head_row(ui, theme, title, &trio, left_label);
    trio
}

fn head_row(ui: &mut Ui, theme: &Theme, title: Rect, trio: &Trio, left_label: &str) {
    // The heads sit on their own 16 px row under the card title.
    let rect = widgets::table_row(ui, 16.0, None);
    let _ = title;
    if !left_label.is_empty() {
        widgets::col_text(ui, rect, left_label, theme.caption_font(), theme.secondary, Align::Min);
    }
    side_head(ui, theme, trio.a(rect), Side::A, "A");
    side_head(ui, theme, trio.b(rect), Side::B, "B");
    widgets::col_text(ui, trio.delta(rect), "B \u{2212} A", theme.caption_font(), theme.secondary, Align::Max);
}

fn side_head(ui: &mut Ui, theme: &Theme, cell: Rect, side: Side, text: &str) {
    widgets::col_text(ui, cell, text, theme.caption_font_bold(), side.color(theme), Align::Max);
}

/// Wider A / B / Δ columns for the word-valued trait rows.
pub fn trait_cols(right: f32) -> Trio {
    Trio::new(right, 108.0, DELTA_W)
}

/// Columns of the "Final stats and EVs" table.
pub struct StatsCols {
    pub stats: Trio,
    pub evs: Trio,
    bars_right: f32,
}

impl StatsCols {
    pub fn bars(&self, row: Rect) -> Rect {
        Rect::from_min_max(
            egui::Pos2::new(self.bars_right - BARS_W, row.min.y),
            egui::Pos2::new(self.bars_right, row.max.y),
        )
    }
}

pub fn stats_col_heads(ui: &mut Ui, theme: &Theme, title: Rect, ev_word: &str) -> StatsCols {
    let evs = Trio::new(title.max.x, VALUE_W, DELTA_W);
    let bars_right = evs.name_right() - 14.0;
    let stats = Trio::new(bars_right - BARS_W - 14.0, VALUE_W, DELTA_W);
    let rect = widgets::table_row(ui, 16.0, None);
    widgets::col_text(ui, rect, "Stat", theme.caption_font(), theme.secondary, Align::Min);
    side_head(ui, theme, stats.a(rect), Side::A, "Stat A");
    side_head(ui, theme, stats.b(rect), Side::B, "Stat B");
    widgets::col_text(ui, stats.delta(rect), "B \u{2212} A", theme.caption_font(), theme.secondary, Align::Max);
    side_head(ui, theme, evs.a(rect), Side::A, &format!("{} A", ev_word));
    side_head(ui, theme, evs.b(rect), Side::B, &format!("{} B", ev_word));
    widgets::col_text(ui, evs.delta(rect), "B \u{2212} A", theme.caption_font(), theme.secondary, Align::Max);
    StatsCols { stats, evs, bars_right }
}

/// Columns of the route-totals table.
pub struct TotalsCols {
    pub values: Trio,
    bars_left: f32,
    detail_left: f32,
}

impl TotalsCols {
    pub fn bars(&self, row: Rect) -> Rect {
        Rect::from_min_max(
            egui::Pos2::new(self.bars_left, row.min.y),
            egui::Pos2::new(self.bars_left + BARS_W, row.max.y),
        )
    }

    pub fn detail(&self, row: Rect) -> Rect {
        Rect::from_min_max(egui::Pos2::new(self.detail_left, row.min.y), egui::Pos2::new(row.max.x, row.max.y))
    }
}

pub fn totals_col_heads(ui: &mut Ui, theme: &Theme, title: Rect) -> TotalsCols {
    // Detail takes what is left after the value trio and the bars.
    let detail_w = ((title.width() - 56.0 * 2.0 - DELTA_W - BARS_W - 160.0) * 0.5).clamp(0.0, 260.0);
    let values = Trio::new(title.max.x - detail_w - BARS_W - 28.0, VALUE_W, DELTA_W);
    let bars_left = values.delta_right + 14.0;
    let detail_left = bars_left + BARS_W + 14.0;
    let rect = widgets::table_row(ui, 16.0, None);
    widgets::col_text(ui, rect, "Metric", theme.caption_font(), theme.secondary, Align::Min);
    side_head(ui, theme, values.a(rect), Side::A, "A");
    side_head(ui, theme, values.b(rect), Side::B, "B");
    widgets::col_text(ui, values.delta(rect), "B \u{2212} A", theme.caption_font(), theme.secondary, Align::Max);
    if detail_w > 40.0 {
        widgets::col_text(
            ui,
            Rect::from_min_max(egui::Pos2::new(detail_left, rect.min.y), egui::Pos2::new(rect.max.x, rect.max.y)),
            "Detail",
            theme.caption_font(),
            theme.secondary,
            Align::Min,
        );
    }
    TotalsCols {
        values,
        bars_left,
        detail_left,
    }
}

/// Columns of the "Moves learned" table.
pub struct MovesCols {
    source_left: f32,
    pub trio: Trio,
}

impl MovesCols {
    pub fn name(&self, row: Rect) -> Rect {
        Rect::from_min_max(row.min, egui::Pos2::new(self.source_left - 8.0, row.max.y))
    }

    pub fn source(&self, row: Rect) -> Rect {
        Rect::from_min_max(
            egui::Pos2::new(self.source_left, row.min.y),
            egui::Pos2::new(self.trio.a_right - self.trio.width, row.max.y),
        )
    }

    pub fn a(&self, row: Rect) -> Rect {
        self.trio.a(row)
    }

    pub fn b(&self, row: Rect) -> Rect {
        self.trio.b(row)
    }
}

pub fn moves_col_heads(ui: &mut Ui, theme: &Theme, title: Rect) -> MovesCols {
    let trio = Trio::new(title.max.x, 220.0_f32.min(title.width() * 0.22), 0.0);
    let source_left = (title.min.x + 200.0).min(trio.name_right());
    let rect = widgets::table_row(ui, 16.0, None);
    widgets::col_text(ui, rect, "Move", theme.caption_font(), theme.secondary, Align::Min);
    widgets::col_text(
        ui,
        Rect::from_min_max(egui::Pos2::new(source_left, rect.min.y), rect.max),
        "Source",
        theme.caption_font(),
        theme.secondary,
        Align::Min,
    );
    side_head(ui, theme, trio.a(rect), Side::A, "A \u{b7} Lv");
    side_head(ui, theme, trio.b(rect), Side::B, "B \u{b7} Lv");
    MovesCols { source_left, trio }
}

// ---------------------------------------------------------------------------
// Cells
// ---------------------------------------------------------------------------

/// The left-hand name cell; `sub` indents and mutes it.
pub fn name_cell(ui: &mut Ui, theme: &Theme, row: Rect, text: &str, sub: bool) {
    let x = row.min.x + if sub { 14.0 } else { 0.0 };
    let color = if sub { theme.secondary } else { theme.text };
    widgets::col_text(
        ui,
        Rect::from_min_max(egui::Pos2::new(x, row.min.y), row.max),
        text,
        theme.body(),
        color,
        Align::Min,
    );
}

/// An uppercase group heading row.
pub fn group_heading(ui: &mut Ui, theme: &Theme, text: &str) {
    let rect = widgets::table_row(ui, GROUP_H, Some(theme.pane_divider()));
    let g = widgets::caption_galley(ui, text, theme.caption_font(), theme.secondary);
    ui.painter()
        .galley(egui::Pos2::new(rect.min.x, rect.center().y - g.size().y / 2.0), g, theme.secondary);
}

/// A / B values plus the Δ cell, emphasised when they differ (SPEC §3.3).
pub fn value_pair(ui: &mut Ui, c: &Cmp, row: Rect, trio: &Trio, a: i64, b: i64) {
    let differs = c.diff(a != b);
    let (font, color) = if differs {
        (c.theme.font_bold(10.5), c.theme.text_strong())
    } else {
        (c.theme.body(), c.theme.secondary)
    };
    widgets::col_text(ui, trio.a(row), &widgets::fmt_thousands(a), font.clone(), color, Align::Max);
    widgets::col_text(ui, trio.b(row), &widgets::fmt_thousands(b), font, color, Align::Max);
    delta_cell(ui, c, trio.delta(row), b - a);
}

/// Like [`value_pair`] but the values stay body-sized (the EV columns).
pub fn value_pair_plain(ui: &mut Ui, c: &Cmp, row: Rect, trio: &Trio, a: i64, b: i64) {
    let color = if a == b { c.theme.secondary } else { c.theme.text };
    widgets::col_text(ui, trio.a(row), &widgets::fmt_thousands(a), c.theme.body(), color, Align::Max);
    widgets::col_text(ui, trio.b(row), &widgets::fmt_thousands(b), c.theme.body(), color, Align::Max);
    delta_cell(ui, c, trio.delta(row), b - a);
}

/// The signed `B − A` cell. Zero is an em dash.
pub fn delta_cell(ui: &mut Ui, c: &Cmp, cell: Rect, delta: i64) {
    let (text, font, color) = if delta == 0 {
        ("\u{2014}".to_string(), c.theme.body(), c.theme.secondary)
    } else {
        let sign = if delta > 0 { "+" } else { "\u{2212}" };
        let text = format!("{}{}", sign, widgets::fmt_thousands(delta.abs()));
        let strong = c.highlight;
        (
            text,
            if strong { c.theme.body_bold() } else { c.theme.body() },
            if strong { c.theme.text_strong() } else { c.theme.text },
        )
    };
    widgets::col_text(ui, cell, &text, font, color, Align::Max);
}

/// A money delta (`+$11,976`).
pub fn money_delta_cell(ui: &mut Ui, c: &Cmp, cell: Rect, delta: i64) {
    let (text, font, color) = if delta == 0 {
        ("\u{2014}".to_string(), c.theme.body(), c.theme.secondary)
    } else {
        let sign = if delta > 0 { "+" } else { "\u{2212}" };
        (
            format!("{}${}", sign, widgets::fmt_thousands(delta.abs())),
            if c.highlight { c.theme.body_bold() } else { c.theme.body() },
            if c.highlight { c.theme.text_strong() } else { c.theme.text },
        )
    };
    widgets::col_text(ui, cell, &text, font, color, Align::Max);
}

/// The word `differs` (or an em dash) for non-numeric rows.
pub fn differs_cell(ui: &mut Ui, c: &Cmp, cell: Rect, differs: bool) {
    let (text, font, color) = if differs {
        (
            "differs",
            if c.highlight { c.theme.body_bold() } else { c.theme.body() },
            if c.highlight { c.theme.text_strong() } else { c.theme.text },
        )
    } else {
        ("\u{2014}", c.theme.body(), c.theme.secondary)
    };
    widgets::col_text(ui, cell, text, font, color, Align::Max);
}

/// `$3,000` / `−$96,015`.
pub fn money(v: i64) -> String {
    if v < 0 {
        format!("\u{2212}${}", widgets::fmt_thousands(v.abs()))
    } else {
        format!("${}", widgets::fmt_thousands(v))
    }
}

/// A coloured time delta: negative (B ahead) is success, positive is failure.
pub fn time_delta_color(theme: &Theme, delta: f64) -> Color32 {
    if delta < 0.0 {
        theme.success
    } else if delta > 0.0 {
        theme.failure
    } else {
        theme.secondary
    }
}

/// The configured colour of every fight category, keyed by its event tag —
/// the same colours the route list paints its fight rows with.
pub fn fight_category_colors(cfg: &xpr_core::Config) -> std::collections::HashMap<&'static str, Color32> {
    xpr_core::consts::FIGHT_CATEGORY_TO_TAG
        .iter()
        .map(|(cat, tag)| (*tag, xpr_ui_kit::theme::parse_hex(&cfg.get_fight_category_color(cat))))
        .collect()
}

/// A small square swatch in a fight-category colour, outlined so the nearly
/// black gym-leader colour stays visible.
pub fn category_swatch(ui: &Ui, theme: &Theme, center: egui::Pos2, color: Option<Color32>) {
    let Some(color) = color else { return };
    let rect = Rect::from_center_size(center, Vec2::splat(8.0));
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(2),
        color,
        egui::Stroke::new(1.0_f32, theme.divider),
        egui::StrokeKind::Outside,
    );
}

/// The narrow stat/EV column pair of the expanded checkpoint panel.
pub struct MiniTrio {
    pub stats: Trio,
    pub evs: Trio,
}

pub fn mini_trio(right: f32) -> MiniTrio {
    let evs = Trio::new(right, 40.0, 0.0);
    let stats = Trio::new(evs.name_right() - 12.0, 40.0, 48.0);
    MiniTrio { stats, evs }
}

pub fn mini_heads(ui: &mut Ui, theme: &Theme, trio: &MiniTrio, ev_word: &str) {
    let rect = widgets::table_row(ui, 16.0, None);
    let cap = theme.caption_font_bold();
    widgets::col_text(ui, trio.stats.a(rect), "Stat A", cap.clone(), Side::A.color(theme), Align::Max);
    widgets::col_text(ui, trio.stats.b(rect), "Stat B", cap.clone(), Side::B.color(theme), Align::Max);
    widgets::col_text(ui, trio.stats.delta(rect), "B \u{2212} A", theme.caption_font(), theme.secondary, Align::Max);
    widgets::col_text(ui, trio.evs.a(rect), &format!("{} A", ev_word), cap.clone(), Side::A.color(theme), Align::Max);
    widgets::col_text(ui, trio.evs.b(rect), &format!("{} B", ev_word), cap, Side::B.color(theme), Align::Max);
}
