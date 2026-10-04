//! The movepool tables (level-up, TM/HM, tutor, egg, transfer, prior
//! evolution) with type coverage, TM / tutor popovers and the
//! banned / postgame move cross-outs. Port of Solodex `Movepool.tsx`,
//! `SortableTableHeader.tsx`, `useMoveSort.ts`, `TypeCoveragePanel.tsx`,
//! `TmPopover.tsx`, `TutorPopover.tsx`, `BannedMovesModal.tsx`.
//!
//! What the comparison views reuse: [`RowData`] with [`level_prefix`],
//! [`level_sort_key`], [`single_level_rows`] and the other row builders
//! ([`rows`]), [`sort_order`] / [`sort_rows`] with [`SortState`], the shared
//! column widths [`Columns`], the sortable header [`table_header`] and the
//! row painter [`move_row`] ([`table`]), and [`crossed_out_moves`].

pub mod banned;
pub mod coverage;
pub mod paint;
pub mod popovers;
pub mod probe;
pub mod rows;
pub mod table;

use std::collections::HashSet;
use std::sync::Arc;

use egui::{Align2, Color32, Pos2, Rect, ScrollArea, Sense, Ui, UiBuilder, Vec2};
use xpr_dex::{canonical_move_key, unobtainable_move_sets, PokemonData, UserBans};
use xpr_ui_kit::theme::Theme;

pub use rows::{
    apply_remind_labels, build_level_up_rows, build_simple_rows, diff_marks, level_prefix,
    level_sort_key, move_info, single_level_rows, single_simple_rows, sort_order, sort_rows, tm_hm_rows,
    tutor_rows, GameTag, GenGameData, MoveInfo, RowData, SortColumn, SortState,
};
pub use table::{
    move_row, table_header, Columns, HeaderEvent, MoveRowCx, RowResponse, RowStyle, TableMeasure,
    CELL_PAD, HEADER_H, ROW_H,
};

use self::banned::BannedState;
use self::coverage::Coverage;
use self::paint::text_at;
use crate::state::DexSettings;
use crate::views::MovepoolPane;
use crate::widgets::text_w;
use crate::{palette, DexAction, DexCx};

/// Gap between the two table columns (Tailwind `gap-12`).
const COLUMN_GAP: f32 = 48.0;
/// Gap between stacked tables (`gap-4`).
const SECTION_GAP: f32 = 16.0;
/// Width the left column never gets narrower than.
const LEFT_MIN_W: f32 = 360.0;
/// Scroll areas scroll by wheel and bar only: a drag would also scroll while
/// right-clicking a row.
const NO_DRAG: egui::scroll_area::ScrollSource = egui::scroll_area::ScrollSource {
    drag: false,
    ..egui::scroll_area::ScrollSource::ALL
};
/// The right-click test set holds at most this many moves.
pub const TEST_SET_MAX: usize = 4;

/// The canonical keys (`canonical_move_key`) of every move to cross out in
/// `game` under the cross-out settings (Solodex `useCrossedOutMoves`). Keys
/// rather than names so the built-in lists ("ThunderPunch", "AncientPower")
/// keep matching the modern spellings of the user's lists.
pub fn crossed_out_moves(settings: &DexSettings, game: &str) -> HashSet<String> {
    let sets = unobtainable_move_sets(game, &settings.user_bans);
    let mut out = HashSet::new();
    if settings.cross_out_banned {
        out.extend(sets.banned.iter().map(|m| canonical_move_key(m)));
    }
    if settings.cross_out_postgame {
        out.extend(sets.postgame.iter().map(|m| canonical_move_key(m)));
    }
    if settings.cross_out_conditional {
        out.extend(sets.conditional.iter().map(|m| canonical_move_key(m)));
    }
    out
}

/// Toggle `name` in the right-click test set: removes it, or adds it while
/// there is room for [`TEST_SET_MAX`] moves.
pub fn toggle_test_set(set: &mut Vec<String>, name: &str) {
    if let Some(i) = set.iter().position(|m| m == name) {
        set.remove(i);
    } else if set.len() < TEST_SET_MAX {
        set.push(name.to_string());
    }
}

// ---------------------------------------------------------------------------
// model
// ---------------------------------------------------------------------------

/// One table of the movepool.
struct Section {
    key: &'static str,
    label: &'static str,
    /// first header ("Lv", or empty)
    col1: &'static str,
    /// first column of the copied spreadsheet
    tsv_label: &'static str,
    /// banned / postgame / conditional cross-outs apply (TM/HM and tutor only, as in Solodex)
    cross: bool,
    rows: Vec<RowData>,
    infos: Vec<Option<MoveInfo>>,
    canon: Vec<String>,
    sort: SortState,
    order: Vec<usize>,
}

impl Section {
    fn new(
        key: &'static str,
        label: &'static str,
        col1: &'static str,
        tsv_label: &'static str,
        cross: bool,
        rows: Vec<RowData>,
        game: &str,
    ) -> Section {
        let infos: Vec<Option<MoveInfo>> =
            rows.iter().map(|r| move_info(&r.move_name, game)).collect();
        let canon = rows
            .iter()
            .map(|r| canonical_move_key(&r.move_name))
            .collect();
        let order = (0..rows.len()).collect();
        Section {
            key,
            label,
            col1,
            tsv_label,
            cross,
            rows,
            infos,
            canon,
            sort: SortState::default(),
            order,
        }
    }

    fn set_sort(&mut self, sort: SortState) {
        self.sort = sort;
        self.order = sort_order(&self.rows, &self.infos, sort);
    }

    /// The copied spreadsheet: the table in its own order (not the sorted one).
    fn tsv(&self) -> String {
        let dash = "\u{2014}".to_string();
        let mut lines = vec![[
            self.tsv_label,
            "Move",
            "Type",
            "Category",
            "Power",
            "Accuracy",
            "PP",
        ]
        .join("\t")];
        for (row, info) in self.rows.iter().zip(&self.infos) {
            let num = |v: Option<i32>| v.map(|n| n.to_string()).unwrap_or_else(|| dash.clone());
            lines.push(
                [
                    row.prefix.clone(),
                    row.move_name.clone(),
                    info.as_ref()
                        .map(|m| m.move_type.clone())
                        .unwrap_or_else(|| dash.clone()),
                    info.as_ref()
                        .map(|m| m.category.clone())
                        .unwrap_or_else(|| dash.clone()),
                    num(info.as_ref().and_then(|m| m.power)),
                    num(info.as_ref().and_then(|m| m.accuracy)),
                    num(info.as_ref().and_then(|m| m.pp)),
                ]
                .join("\t"),
            );
        }
        lines.join("\n")
    }
}

/// The tables of one species in one game.
struct Model {
    species: String,
    game: String,
    /// level up, tutor, egg, transfer, prior evolution, ...
    left: Vec<Section>,
    /// TM / HM
    right: Option<Section>,
}

impl Model {
    fn build(data: &PokemonData, game: &str) -> Model {
        let mut left = Vec::new();
        let mut push = |key: &'static str,
                        label: &'static str,
                        col1: &'static str,
                        tsv: &'static str,
                        cross: bool,
                        rows: Vec<RowData>| {
            if !rows.is_empty() {
                left.push(Section::new(key, label, col1, tsv, cross, rows, game));
            }
        };
        push(
            "level",
            "Level Up Learnset",
            "Lv",
            "Lv",
            false,
            single_level_rows(data),
        );
        push(
            "tutor",
            "Move Tutor",
            "",
            "Tutor",
            true,
            tutor_rows(&data.tutor_learnset),
        );
        push(
            "egg",
            "Egg Moves",
            "",
            "Egg",
            false,
            single_simple_rows(&data.egg_moves),
        );
        push(
            "transfer",
            "Transfer Moves",
            "",
            "Transfer",
            false,
            single_simple_rows(&data.transfer_learnset),
        );
        push(
            "priorevo",
            "Prior Evolution Only",
            "",
            "Pre-evo",
            false,
            single_simple_rows(&data.prior_evolution_learnset),
        );
        // in the data but not shown by Solodex
        push(
            "formchange",
            "Form Change Moves",
            "",
            "Form",
            false,
            single_simple_rows(&data.form_change_learnset),
        );
        push(
            "zygarde",
            "Zygarde Cube Moves",
            "",
            "Cube",
            false,
            single_simple_rows(&data.zygarde_cube_learnset),
        );
        push(
            "lightball",
            "Light Ball Egg Move",
            "",
            "Light Ball",
            false,
            single_simple_rows(&data.light_ball_egg_learnset),
        );
        let tm = tm_hm_rows(&data.tm_hm_learnset, game);
        let right = (!tm.is_empty())
            .then(|| Section::new("tmhm", "TM / HM Learnset", "", "TM/HM", true, tm, game));
        Model {
            species: data.species.clone(),
            game: game.to_string(),
            left,
            right,
        }
    }

    fn is_empty(&self) -> bool {
        self.left.is_empty() && self.right.is_none()
    }

    fn sections(&self) -> impl Iterator<Item = &Section> {
        self.left.iter().chain(self.right.iter())
    }
}

// ---------------------------------------------------------------------------
// the view
// ---------------------------------------------------------------------------

struct CrossCache {
    game: String,
    flags: (bool, bool, bool),
    bans: UserBans,
    set: Arc<HashSet<String>>,
}

struct CoverageCache {
    test_set: Vec<String>,
    game: String,
    cov: Arc<Coverage>,
}

#[derive(Default)]
pub struct MovepoolView {
    model: Option<Model>,
    cross: Option<CrossCache>,
    coverage: Option<CoverageCache>,
    /// height the coverage panel needed last frame
    coverage_h: f32,
    banned: BannedState,
    /// section whose spreadsheet was just copied, and until when the hint says so
    copied: Option<(&'static str, f64)>,
}

/// What the table drawing needs besides the section itself.
struct Env<'a> {
    theme: &'a Theme,
    game: &'a str,
    cols: &'a Columns,
    test: &'a [String],
    cross: &'a HashSet<String>,
    bg: Color32,
    actions: &'a mut Vec<DexAction>,
    now: f64,
    /// the move whose row was right-clicked
    toggled: Option<String>,
    copied: &'a mut Option<(&'static str, f64)>,
}

impl MovepoolView {
    /// The movepool of `data` (already resolved for `game`).
    pub fn ui(&mut self, ui: &mut Ui, cx: &mut DexCx, data: &PokemonData, game: &str) {
        probe::begin_frame();
        let theme: &Theme = cx.theme;
        if self
            .model
            .as_ref()
            .map(|m| m.species != data.species || m.game != game)
            .unwrap_or(true)
        {
            // a new species or game starts with every table in its own order
            self.model = Some(Model::build(data, game));
        }
        let mut model = self.model.take().expect("model");
        let pane = ui.available_rect_before_wrap();
        if model.is_empty() {
            text_at(
                ui,
                Pos2::new(pane.min.x + 16.0, pane.min.y + 16.0),
                Align2::LEFT_CENTER,
                "No move data available.",
                palette::px(theme, 12.0),
                palette::GRAY_600,
                false,
            );
            ui.allocate_rect(pane, Sense::hover());
            self.model = Some(model);
            return;
        }

        let cross = self.cross_set(&cx.state.settings, game);
        let test: Vec<String> = cx.state.move_test_set.clone();
        let max_cov = (ui.ctx().content_rect().height() * 0.4).max(120.0);
        let cov_h = if test.is_empty() {
            0.0
        } else if self.coverage_h <= 0.0 {
            max_cov.min(160.0)
        } else {
            self.coverage_h.min(max_cov)
        }
        .min(pane.height() * 0.8);
        let scroll_rect = Rect::from_min_max(pane.min, Pos2::new(pane.max.x, pane.max.y - cov_h));
        let cov_rect = Rect::from_min_max(Pos2::new(pane.min.x, pane.max.y - cov_h), pane.max);

        // the tables: column widths are shared by every table (Solodex syncColumnWidths)
        let mut cols = Columns::new();
        {
            let in_test = |n: &str| test.iter().any(|t| t == n);
            for s in model.sections() {
                cols.include(
                    ui,
                    theme,
                    &TableMeasure {
                        col1: s.col1,
                        sort: s.sort,
                        rows: &s.rows,
                        infos: &s.infos,
                        in_test_set: &in_test,
                    },
                );
            }
        }
        let table_w = cols.total();
        let left_w = table_w.max(LEFT_MIN_W);
        let avail_w = scroll_rect.width() - 32.0;
        let two_col = model.right.is_some()
            && !model.left.is_empty()
            && avail_w >= left_w + COLUMN_GAP + table_w;
        let now = ui.input(|i| i.time);
        if let Some((_, until)) = self.copied {
            if now >= until {
                self.copied = None;
            } else {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(200));
            }
        }

        let toggled: Option<String>;
        {
            let mut sui = ui.new_child(
                UiBuilder::new()
                    .max_rect(scroll_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            sui.set_clip_rect(scroll_rect.intersect(ui.clip_rect()));
            let mut env = Env {
                theme,
                game,
                cols: &cols,
                test: &test,
                cross: &cross,
                bg: palette::page_bg(theme),
                actions: &mut *cx.actions,
                now,
                toggled: None,
                copied: &mut self.copied,
            };
            ScrollArea::both()
                .id_salt("movepool_scroll")
                .auto_shrink([false, false])
                .scroll_source(NO_DRAG)
                .show(&mut sui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    let origin = ui.cursor().min + Vec2::new(16.0, 4.0);
                    let tall = Vec2::new(0.0, 1.0e6);
                    let mut bottom = origin.y;
                    let mut right_edge = origin.x;
                    let left_rect = Rect::from_min_size(origin, Vec2::new(left_w, 0.0) + tall);
                    if !model.left.is_empty() {
                        let r = ui.scope_builder(
                            UiBuilder::new().max_rect(left_rect).id_salt("mp_left"),
                            |ui| {
                                ui.spacing_mut().item_spacing = Vec2::ZERO;
                                for (i, s) in model.left.iter_mut().enumerate() {
                                    if i > 0 {
                                        ui.add_space(SECTION_GAP);
                                    }
                                    draw_section(ui, &mut env, s);
                                }
                            },
                        );
                        bottom = bottom.max(r.response.rect.max.y);
                        right_edge = right_edge.max(r.response.rect.min.x + left_w);
                    }
                    if let Some(tm) = model.right.as_mut() {
                        let pos = if two_col || model.left.is_empty() {
                            Pos2::new(
                                origin.x
                                    + if model.left.is_empty() {
                                        0.0
                                    } else {
                                        left_w + COLUMN_GAP
                                    },
                                origin.y,
                            )
                        } else {
                            Pos2::new(origin.x, bottom + SECTION_GAP)
                        };
                        let r = ui.scope_builder(
                            UiBuilder::new()
                                .max_rect(Rect::from_min_size(pos, Vec2::new(table_w, 0.0) + tall))
                                .id_salt("mp_right"),
                            |ui| {
                                ui.spacing_mut().item_spacing = Vec2::ZERO;
                                draw_section(ui, &mut env, tm);
                            },
                        );
                        bottom = bottom.max(r.response.rect.max.y);
                        right_edge = right_edge.max(pos.x + table_w);
                    }
                    // padding right and below (px-4, pb-4)
                    ui.allocate_rect(
                        Rect::from_min_max(
                            origin - Vec2::new(16.0, 4.0),
                            Pos2::new(right_edge + 16.0, bottom + 16.0),
                        ),
                        Sense::hover(),
                    );
                });
            toggled = env.toggled.take();
        }
        if let Some(name) = toggled {
            toggle_test_set(&mut cx.state.move_test_set, &name);
        }

        // the coverage panel, fixed below the scroll area
        if !test.is_empty() {
            let cov = self.coverage_for(&test, game);
            let mut pui = ui.new_child(
                UiBuilder::new()
                    .max_rect(cov_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            pui.set_clip_rect(cov_rect.intersect(ui.clip_rect()));
            coverage::frame_fill(&pui, cov_rect);
            let inner = cov_rect.shrink2(Vec2::new(12.0, 8.0));
            let mut cui = pui.new_child(
                UiBuilder::new()
                    .max_rect(inner)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            cui.set_clip_rect(inner.intersect(pui.clip_rect()));
            let out = ScrollArea::vertical()
                .id_salt("movepool_coverage")
                .auto_shrink([false, true])
                .scroll_source(NO_DRAG)
                .show(&mut cui, |ui| {
                    coverage::panel_ui(ui, theme, cx.images, &cov, game)
                });
            let needed = (out.content_size.y + 16.0).min(max_cov);
            if (needed - self.coverage_h).abs() > 0.5 {
                self.coverage_h = needed;
                ui.ctx().request_repaint();
            }
            if let Some(name) = out.inner.remove {
                cx.state.move_test_set.retain(|m| *m != name);
            }
            if out.inner.clear {
                cx.state.move_test_set.clear();
            }
        } else {
            self.coverage_h = 0.0;
        }
        ui.allocate_rect(pane, Sense::hover());
        self.model = Some(model);
    }

    /// The banned / postgame moves editor while `cx.state.banned_editor_open`.
    pub fn banned_moves_modal(&mut self, ctx: &egui::Context, cx: &mut DexCx) {
        if !cx.state.banned_editor_open {
            self.banned.opened = false;
            return;
        }
        banned::show(ctx, cx, &mut self.banned);
    }

    fn cross_set(&mut self, settings: &DexSettings, game: &str) -> Arc<HashSet<String>> {
        let flags = (
            settings.cross_out_banned,
            settings.cross_out_postgame,
            settings.cross_out_conditional,
        );
        if let Some(c) = &self.cross {
            if c.game == game && c.flags == flags && c.bans == settings.user_bans {
                return c.set.clone();
            }
        }
        let set = Arc::new(crossed_out_moves(settings, game));
        self.cross = Some(CrossCache {
            game: game.to_string(),
            flags,
            bans: settings.user_bans.clone(),
            set: set.clone(),
        });
        set
    }

    fn coverage_for(&mut self, test: &[String], game: &str) -> Arc<Coverage> {
        if let Some(c) = &self.coverage {
            if c.game == game && c.test_set == test {
                return c.cov.clone();
            }
        }
        let cov = Arc::new(Coverage::compute(test, game));
        self.coverage = Some(CoverageCache {
            test_set: test.to_vec(),
            game: game.to_string(),
            cov: cov.clone(),
        });
        cov
    }
}

impl MovepoolPane for MovepoolView {
    fn movepool_ui(&mut self, ui: &mut Ui, cx: &mut DexCx, data: &PokemonData, game: &str) {
        self.ui(ui, cx, data, game);
    }
}

// ---------------------------------------------------------------------------
// one table
// ---------------------------------------------------------------------------

/// The section label above a table (Solodex `CopyableHeader`): a click
/// copies the table as a spreadsheet.
fn section_label(ui: &mut Ui, env: &mut Env, s: &Section) {
    let theme = env.theme;
    let label = s.label.to_uppercase();
    let label_font = palette::px_bold(theme, 12.0);
    let hint_font = palette::px(theme, 12.0);
    let copied = env
        .copied
        .map(|(k, until)| k == s.key && env.now < until)
        .unwrap_or(false);
    let hint = if copied { "Copied" } else { "Copy" };
    let lw = text_w(ui, &label, &label_font);
    let hw = text_w(ui, hint, &hint_font);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(lw + 8.0 + hw, 16.0), Sense::click());
    let (lc, hc) = if resp.hovered() {
        (palette::GRAY_300, palette::GRAY_400)
    } else {
        (palette::GRAY_500, palette::GRAY_600)
    };
    ui.painter().text(
        rect.left_center(),
        Align2::LEFT_CENTER,
        &label,
        label_font,
        lc,
    );
    ui.painter().text(
        Pos2::new(rect.min.x + lw + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        hint,
        hint_font,
        hc,
    );
    probe::record("label", s.key, 0, s.label, rect);
    if resp
        .on_hover_text("Click to copy as spreadsheet")
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        ui.ctx().copy_text(s.tsv());
        *env.copied = Some((s.key, env.now + 1.5));
    }
    ui.add_space(4.0);
}

/// A section's label, sortable header (sticky while the table is in view)
/// and rows.
fn draw_section(ui: &mut Ui, env: &mut Env, s: &mut Section) {
    section_label(ui, env, s);
    let cols = env.cols;
    let top = ui.cursor().min;
    let table_h = HEADER_H + s.rows.len() as f32 * ROW_H;
    ui.allocate_exact_size(Vec2::new(cols.total(), HEADER_H), Sense::hover());
    let id_base = ui.id().with(s.key);
    {
        let mut mx = MoveRowCx {
            theme: env.theme,
            game: env.game,
            cols,
            actions: &mut *env.actions,
        };
        for (pos, &i) in s.order.iter().enumerate() {
            let row = &s.rows[i];
            let style = RowStyle {
                in_test_set: env.test.iter().any(|t| *t == row.move_name),
                crossed_out: s.cross && env.cross.contains(&s.canon[i]),
                highlight: false,
                can_toggle: true,
            };
            let r = move_row(
                ui,
                &mut mx,
                id_base.with(pos),
                row,
                s.infos[i].as_ref(),
                style,
            );
            probe::record("row", s.key, pos, &row.move_name, r.rect);
            if style.crossed_out {
                probe::record("crossed", s.key, pos, &row.move_name, r.rect);
            }
            if r.toggle_test_set {
                env.toggled = Some(row.move_name.clone());
            }
        }
    }
    // header: sticks to the top of the scroll area while the table is in view
    let view_top = ui.clip_rect().min.y;
    let y = view_top.max(top.y).min(top.y + table_h - HEADER_H);
    let (hrect, event) = table_header(
        ui,
        env.theme,
        cols,
        Pos2::new(top.x, y),
        id_base.with("header"),
        s.col1,
        s.sort,
        env.bg,
    );
    for (i, col) in SortColumn::ALL.iter().enumerate() {
        let label = if i == 0 { s.col1 } else { col.label() };
        let cell = Rect::from_min_size(
            Pos2::new(hrect.min.x + cols.x(i), hrect.min.y),
            Vec2::new(cols.width(i), HEADER_H),
        );
        probe::record("th", s.key, i, label, cell);
    }
    match event {
        Some(HeaderEvent::Clicked(col)) => s.set_sort(s.sort.clicked(col)),
        Some(HeaderEvent::Reset) => s.set_sort(SortState::default()),
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_toggles_and_stops_at_four() {
        let mut set: Vec<String> = Vec::new();
        for m in ["A", "B", "C", "D", "E"] {
            toggle_test_set(&mut set, m);
        }
        assert_eq!(set, vec!["A", "B", "C", "D"]);
        toggle_test_set(&mut set, "B");
        assert_eq!(set, vec!["A", "C", "D"]);
        toggle_test_set(&mut set, "E");
        assert_eq!(set, vec!["A", "C", "D", "E"]);
    }

    #[test]
    fn crossed_out_set_follows_the_flags_and_canonical_names() {
        let mut s = DexSettings::default();
        assert!(crossed_out_moves(&s, "Emerald").is_empty());
        s.cross_out_postgame = true;
        let set = crossed_out_moves(&s, "Emerald");
        // the built-in list spells it "ThunderPunch"; the key is spelling-independent
        assert!(set.contains(&canonical_move_key("Thunder Punch")));
        assert!(!set.contains(&canonical_move_key("Double Team")));
        s.cross_out_banned = true;
        assert!(crossed_out_moves(&s, "Emerald").contains(&canonical_move_key("Double Team")));
        s.cross_out_postgame = false;
        s.user_bans.by_game.insert("Emerald".into(), vec!["Surf".into()]);
        assert!(!crossed_out_moves(&s, "Emerald").contains(&canonical_move_key("Surf")), "per-game lists only apply with the postgame flag");
        s.cross_out_postgame = true;
        assert!(crossed_out_moves(&s, "Emerald").contains(&canonical_move_key("Surf")));
        assert!(!crossed_out_moves(&s, "Crystal").contains(&canonical_move_key("Surf")));
    }

    #[test]
    fn copied_spreadsheet_is_the_table_in_its_own_order() {
        let data = xpr_dex::get_pokemon_data("Pikachu", "Emerald").unwrap();
        let model = Model::build(&data, "Emerald");
        let level = &model.left[0];
        let tsv = level.tsv();
        let mut lines = tsv.lines();
        assert_eq!(lines.next(), Some("Lv\tMove\tType\tCategory\tPower\tAccuracy\tPP"));
        assert_eq!(lines.next(), Some("1\tThunderShock\tElectric\tSpecial\t40\t100\t30"));
        // a status move without power or accuracy shows dashes
        assert!(tsv.contains("1\tGrowl\tNormal\tStatus\t\u{2014}\t100\t40"));
        let tm = model.right.as_ref().unwrap().tsv();
        assert!(tm.starts_with("TM/HM\tMove\t"));
        assert!(tm.contains("TM24\tThunderbolt\tElectric\tSpecial\t95\t100\t15"));
    }
}
