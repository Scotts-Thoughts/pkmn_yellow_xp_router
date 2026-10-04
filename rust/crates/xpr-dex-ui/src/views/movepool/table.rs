//! The movepool table widgets: shared column widths, the sortable header
//! and one move row (Solodex `Movepool.tsx` `MoveRow`, `SortableTableHeader.tsx`).
//!
//! Everything here is usable from other views (the comparison views draw the
//! same rows): measure the columns of every table that should line up with
//! [`Columns`], draw each table's header with [`table_header`] and its rows
//! with [`move_row`].

use egui::{
    Align2, Color32, CornerRadius, Id, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, UiBuilder,
    Vec2,
};
use xpr_ui_kit::theme::{self, Theme};

use super::paint::text_at;
use super::popovers;
use super::rows::{MoveInfo, RowData, SortColumn, SortState};
use crate::palette::{self, hex};
use crate::widgets::{self, text_w};
use crate::DexAction;

/// Height of a body row.
pub const ROW_H: f32 = 22.0;
/// Height of the header row.
pub const HEADER_H: f32 = 28.0;
/// Horizontal cell padding (Tailwind `px-1`).
pub const CELL_PAD: f32 = 4.0;
/// Width of the small type badge.
const BADGE_W: f32 = 68.0;
const BADGE_H: f32 = 18.0;

/// Move-name font of the rows.
fn body_font(theme: &Theme) -> egui::FontId {
    palette::px(theme, 14.0)
}

/// What a table row needs to know about the surroundings.
pub struct MoveRowCx<'a> {
    pub theme: &'a Theme,
    pub game: &'a str,
    pub cols: &'a Columns,
    /// receives `DexAction::OpenUrl` from the TM / tutor popovers
    pub actions: &'a mut Vec<DexAction>,
}

/// How a row is drawn.
#[derive(Clone, Copy, Debug, Default)]
pub struct RowStyle {
    /// the move is in the right-click test set (blue-gray row, a dot before the name)
    pub in_test_set: bool,
    /// banned / postgame / conditional: faded and struck through
    pub crossed_out: bool,
    /// the soft "different move" tint of the comparison views
    pub highlight: bool,
    /// right-click toggles the move in the test set
    pub can_toggle: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct RowResponse {
    pub rect: Rect,
    /// the row was right-clicked while `can_toggle` was set
    pub toggle_test_set: bool,
}

// ---------------------------------------------------------------------------
// column widths
// ---------------------------------------------------------------------------

/// The seven column widths shared by every table of a movepool (Solodex
/// `syncColumnWidths`: each column is as wide as its widest cell in any
/// table, so the level-up, tutor, egg ... tables line up).
#[derive(Clone, Debug)]
pub struct Columns {
    w: [f32; 7],
}

impl Default for Columns {
    fn default() -> Self {
        Columns::new()
    }
}

/// What [`Columns::include`] needs of a table.
pub struct TableMeasure<'a> {
    /// the first header ("Lv", or empty)
    pub col1: &'a str,
    pub sort: SortState,
    pub rows: &'a [RowData],
    /// `infos[i]` is the stats of `rows[i]`
    pub infos: &'a [Option<MoveInfo>],
    /// whether a move is in the test set (its dot widens the move column)
    pub in_test_set: &'a dyn Fn(&str) -> bool,
}

impl Columns {
    /// Minimum widths: the first column is `w-10` and the type column holds
    /// its badge.
    pub fn new() -> Columns {
        Columns {
            w: [40.0, 0.0, BADGE_W + 2.0 * CELL_PAD, 0.0, 0.0, 0.0, 0.0],
        }
    }

    pub fn width(&self, col: usize) -> f32 {
        self.w[col]
    }

    /// Left edge of a column inside the table.
    pub fn x(&self, col: usize) -> f32 {
        self.w[..col].iter().sum()
    }

    pub fn total(&self) -> f32 {
        self.w.iter().sum()
    }

    fn grow(&mut self, col: usize, w: f32) {
        if w > self.w[col] {
            self.w[col] = w;
        }
    }

    /// Widen the columns to fit `table`.
    pub fn include(&mut self, ui: &Ui, theme: &Theme, table: &TableMeasure) {
        let body = body_font(theme);
        let bold = palette::px_bold(theme, 14.0);
        let tag_font = palette::px_bold(theme, 11.0);
        let dot_w = text_w(ui, "\u{25CF}", &palette::px(theme, 12.0));
        let pad = 2.0 * CELL_PAD;
        for (i, col) in SortColumn::ALL.iter().enumerate() {
            let label = if i == 0 { table.col1 } else { col.label() };
            let text = match table.sort.active(*col) {
                Some(widgets::SortDir::Asc) => format!("{} \u{25B2}", label),
                Some(widgets::SortDir::Desc) => format!("{} \u{25BC}", label),
                None => label.to_string(),
            };
            self.grow(i, text_w(ui, &text, &bold) + pad);
        }
        for (row, info) in table.rows.iter().zip(table.infos) {
            if !row.prefix.is_empty() {
                self.grow(0, text_w(ui, &row.prefix, &body) + pad);
            }
            let mut w = text_w(ui, &row.move_name, &body);
            if (table.in_test_set)(&row.move_name) {
                w += dot_w + 4.0;
            }
            for tag in &row.game_tags {
                w += 4.0 + text_w(ui, &tag.abbrev, &tag_font) + 2.0 * CELL_PAD + 2.0;
            }
            self.grow(1, w + pad);
            let cat = info
                .as_ref()
                .map(|m| m.category.as_str())
                .unwrap_or("\u{2014}");
            self.grow(3, text_w(ui, cat, &body) + pad);
            let num = |v: Option<i32>| {
                v.map(|n| n.to_string())
                    .unwrap_or_else(|| "\u{2014}".to_string())
            };
            self.grow(
                4,
                text_w(ui, &num(info.as_ref().and_then(|m| m.power)), &body) + pad,
            );
            self.grow(
                5,
                text_w(ui, &num(info.as_ref().and_then(|m| m.accuracy)), &body) + pad,
            );
            self.grow(
                6,
                text_w(ui, &num(info.as_ref().and_then(|m| m.pp)), &body) + pad,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// header
// ---------------------------------------------------------------------------

/// What a click on a header did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderEvent {
    /// left click on a column (the first column resets)
    Clicked(SortColumn),
    /// right click: back to the table's own order
    Reset,
}

/// Draw a table header at `pos` (`bg` fills it so rows scrolling under a
/// sticky header are hidden). Returns the click, if any.
#[allow(clippy::too_many_arguments)]
pub fn table_header(
    ui: &mut Ui,
    theme: &Theme,
    cols: &Columns,
    pos: Pos2,
    salt: Id,
    col1: &str,
    sort: SortState,
    bg: Color32,
) -> (Rect, Option<HeaderEvent>) {
    let rect = Rect::from_min_size(pos, Vec2::new(cols.total(), HEADER_H));
    ui.painter().rect_filled(rect, CornerRadius::ZERO, bg);
    let mut event = None;
    for (i, col) in SortColumn::ALL.iter().enumerate() {
        let cell = Rect::from_min_size(
            Pos2::new(rect.min.x + cols.x(i), rect.min.y),
            Vec2::new(cols.width(i), HEADER_H),
        );
        let label = if i == 0 { col1 } else { col.label() };
        let resp = widgets::sort_header_cell(
            ui,
            theme,
            cell,
            salt.with(("th", i)),
            label,
            sort.active(*col),
            col.right_aligned(),
        );
        if resp.secondary_clicked() {
            event = Some(HeaderEvent::Reset);
        } else if resp.clicked() {
            event = Some(HeaderEvent::Clicked(*col));
        }
    }
    (rect, event)
}

// ---------------------------------------------------------------------------
// row
// ---------------------------------------------------------------------------

/// Draw one move row at the cursor. `salt` must be unique per row of a page
/// (it keys the row's popups). Rows outside the visible area are laid out
/// but not painted.
pub fn move_row(
    ui: &mut Ui,
    mx: &mut MoveRowCx,
    salt: Id,
    row: &RowData,
    info: Option<&MoveInfo>,
    style: RowStyle,
) -> RowResponse {
    let theme = mx.theme;
    let cols = mx.cols;
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(cols.total(), ROW_H),
        if style.can_toggle {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    if !ui.is_rect_visible(rect) {
        return RowResponse {
            rect,
            toggle_test_set: false,
        };
    }
    let cell = |i: usize| {
        Rect::from_min_size(
            Pos2::new(rect.min.x + cols.x(i), rect.min.y),
            Vec2::new(cols.width(i), ROW_H),
        )
    };
    let fade = if style.crossed_out { 0.4 } else { 1.0 };
    let tint = |c: Color32| c.gamma_multiply(fade);
    let strike = style.crossed_out;
    let font = body_font(theme);
    let mid = |c: Rect| c.center().y;
    // the background is painted last (it depends on the children's hover) but sits below them
    let bg_idx = ui.painter().add(Shape::Noop);
    let mut child_hover = false;
    let mut toggle = style.can_toggle && resp.secondary_clicked();

    // first column: level / TM code / "Tutor"
    let c0 = cell(0);
    if row.is_tm() || row.is_tutor() {
        let w = text_w(ui, &row.prefix, &font);
        let r = Rect::from_min_size(
            Pos2::new(c0.min.x + CELL_PAD, c0.min.y),
            Vec2::new(w, ROW_H),
        );
        let pr = ui
            .interact(r, salt.with("prefix"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let color = if pr.hovered() {
            palette::BLUE_400
        } else {
            palette::GRAY_500
        };
        text_at(
            ui,
            Pos2::new(r.min.x, mid(c0)),
            Align2::LEFT_CENTER,
            &row.prefix,
            font.clone(),
            tint(color),
            strike,
        );
        child_hover |= pr.hovered();
        toggle |= style.can_toggle && pr.secondary_clicked();
        if row.is_tm() {
            popovers::tm_popover(&pr, theme, &row.prefix, mx.game, mx.actions);
        } else {
            popovers::tutor_popover(&pr, theme, mx.game, mx.actions);
        }
    } else if !row.prefix.is_empty() {
        text_at(
            ui,
            Pos2::new(c0.min.x + CELL_PAD, mid(c0)),
            Align2::LEFT_CENTER,
            &row.prefix,
            font.clone(),
            tint(palette::GRAY_500),
            strike,
        );
    }

    // move name, test-set dot, game tags
    let c1 = cell(1);
    let mut x = c1.min.x + CELL_PAD;
    if style.in_test_set {
        let dot = palette::px(theme, 12.0);
        let r = text_at(
            ui,
            Pos2::new(x, mid(c1)),
            Align2::LEFT_CENTER,
            "\u{25CF}",
            dot,
            tint(palette::BLUE_400),
            false,
        );
        x = r.max.x + 4.0;
    }
    let r = text_at(
        ui,
        Pos2::new(x, mid(c1)),
        Align2::LEFT_CENTER,
        &row.move_name,
        font.clone(),
        tint(Color32::WHITE),
        strike,
    );
    x = r.max.x + 4.0;
    for tag in &row.game_tags {
        let tf = palette::px_bold(theme, 11.0);
        let w = text_w(ui, &tag.abbrev, &tf) + 2.0 * CELL_PAD + 2.0;
        let tr = Rect::from_center_size(Pos2::new(x + w / 2.0, mid(c1)), Vec2::new(w, 16.0));
        ui.painter().rect_stroke(
            tr,
            CornerRadius::same(3),
            Stroke::new(1.0_f32, tint(theme::with_alpha(tag.color, 96))),
            StrokeKind::Inside,
        );
        ui.painter().text(
            tr.center(),
            Align2::CENTER_CENTER,
            &tag.abbrev,
            tf,
            tint(tag.color),
        );
        x += w + 4.0;
    }

    // type badge
    let c2 = cell(2);
    match info {
        Some(m) => {
            let br = Rect::from_min_size(
                Pos2::new(c2.min.x + CELL_PAD, c2.center().y - BADGE_H / 2.0),
                Vec2::new(BADGE_W, BADGE_H),
            );
            // a child ui: a scope would move this ui's cursor to the badge's bottom
            let mut bui = ui.new_child(
                UiBuilder::new()
                    .max_rect(br)
                    .id_salt(salt.with("type"))
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            if style.crossed_out {
                bui.set_opacity(0.4);
            }
            let badge = widgets::type_badge(&mut bui, theme, &m.move_type, true, Some(mx.game));
            child_hover |= badge.hovered();
            toggle |= style.can_toggle && badge.secondary_clicked();
        }
        None => {
            text_at(
                ui,
                Pos2::new(c2.min.x + CELL_PAD, mid(c2)),
                Align2::LEFT_CENTER,
                "\u{2014}",
                palette::px(theme, 12.0),
                tint(palette::GRAY_600),
                strike,
            );
        }
    }

    // category, power, accuracy, PP
    let c3 = cell(3);
    let (cat_text, cat_color) = match info {
        Some(m) => (m.category.as_str(), palette::category_color(&m.category)),
        None => ("\u{2014}", palette::GRAY_400),
    };
    text_at(
        ui,
        Pos2::new(c3.min.x + CELL_PAD, mid(c3)),
        Align2::LEFT_CENTER,
        cat_text,
        font.clone(),
        tint(cat_color),
        strike,
    );
    let num = |v: Option<i32>| {
        v.map(|n| n.to_string())
            .unwrap_or_else(|| "\u{2014}".to_string())
    };
    for (i, (v, color)) in [
        (info.and_then(|m| m.power), palette::GRAY_100),
        (info.and_then(|m| m.accuracy), palette::GRAY_100),
        (info.and_then(|m| m.pp), palette::GRAY_400),
    ]
    .into_iter()
    .enumerate()
    {
        let c = cell(4 + i);
        text_at(
            ui,
            Pos2::new(c.max.x - CELL_PAD, mid(c)),
            Align2::RIGHT_CENTER,
            &num(v),
            font.clone(),
            tint(color),
            strike,
        );
    }

    let hovered = resp.hovered() || child_hover;
    let bg = if style.in_test_set {
        hex("#1e293b")
    } else if hovered {
        palette::hover_bg(theme)
    } else if style.highlight {
        theme::rgba(30, 58, 95, 0.4)
    } else {
        Color32::TRANSPARENT
    };
    ui.painter()
        .set(bg_idx, Shape::rect_filled(rect, CornerRadius::ZERO, bg));
    RowResponse {
        rect,
        toggle_test_set: toggle,
    }
}
