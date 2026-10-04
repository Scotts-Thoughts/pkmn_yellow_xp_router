//! The Movedex list: filter block, move table, the right-click learner panel
//! and the Space "jump to move" overlay (Solodex `MovedexView.tsx`).

use egui::{Align, Align2, Color32, CornerRadius, Id, Key, Layout, Margin, Modifiers, Pos2, Rect, Sense, Shape, Stroke, Ui, UiBuilder, Vec2};
use xpr_ui_kit::modal::pointer_blocked;
use xpr_ui_kit::theme::Theme;
use xpr_ui_kit::widgets::{option_menu, option_menu_height, Entry};

use super::data::{self, Col, MoveRow, Sort, ALL, CATEGORIES};
use super::kit::{self, tw, RULE};
use super::learners::{self, Style};
use super::MovedexView;
use crate::palette::{self, hex};
use crate::widgets::{self, SortDir, SpotlightOutcome, SpotlightRow, SpotlightState};
use crate::DexCx;

pub(super) const ROW_H: f32 = 27.0;
const HEADER_H: f32 = 24.0;
const TYPE_W: f32 = 80.0;
const CAT_W: f32 = 80.0;
const NUM_W: f32 = 48.0;
const TM_W: f32 = 56.0;
pub(super) const PANEL_W: f32 = 280.0;
/// Width of one Min / Max pair of the filter block (`w-16` x 2 + `gap-1`).
const RANGE_W: f32 = 132.0;

fn num_filter_changed(s: &mut String) {
    // an HTML number input only takes digits, a sign and a point
    s.retain(|c| c.is_ascii_digit() || c == '-' || c == '.');
}

/// The columns' rects for a row (or the header) whose top-left is `(x, y)`.
fn col_rects(x: f32, y: f32, h: f32, name_col: f32) -> [Rect; 7] {
    let widths = [name_col, TYPE_W, CAT_W, NUM_W, NUM_W, NUM_W, TM_W];
    let mut out = [Rect::NOTHING; 7];
    let mut cx = x;
    for (i, w) in widths.iter().enumerate() {
        out[i] = Rect::from_min_size(Pos2::new(cx, y), Vec2::new(*w, h));
        cx += w;
    }
    out
}

impl MovedexView {
    pub(super) fn list_ui(&mut self, ui: &mut Ui, cx: &mut DexCx) {
        let full = ui.available_rect_before_wrap();
        let panel_w = if self.selected.is_some() { PANEL_W.min(full.width() * 0.6) } else { 0.0 };
        let left = Rect::from_min_max(full.min, Pos2::new(full.max.x - panel_w, full.max.y));
        self.sync(ui, cx);
        let mut lui = ui.new_child(UiBuilder::new().id_salt("movedex_left").max_rect(left).layout(Layout::top_down(Align::Min)));
        lui.set_clip_rect(left.intersect(ui.clip_rect()));
        lui.spacing_mut().item_spacing = Vec2::ZERO;
        self.filters_ui(&mut lui, cx);
        self.table_ui(&mut lui, cx);
        if panel_w > 0.0 {
            let right = Rect::from_min_max(Pos2::new(left.max.x, full.min.y), full.max);
            let mut rui = ui.new_child(UiBuilder::new().id_salt("movedex_right").max_rect(right).layout(Layout::top_down(Align::Min)));
            rui.set_clip_rect(right.intersect(ui.clip_rect()));
            rui.spacing_mut().item_spacing = Vec2::ZERO;
            self.panel_ui(&mut rui, cx, right);
        }
        ui.allocate_rect(full, Sense::hover());
        self.handle_keys(ui, cx);
        self.jump_overlay(ui.ctx(), cx);
    }

    /// Rebuild the move table when the game changed and the filtered / sorted
    /// view when anything it depends on did.
    fn sync(&mut self, ui: &Ui, cx: &DexCx) {
        let game = cx.state.game();
        if self.table_game != game {
            self.table_game = game.to_string();
            self.table = data::moves_for_game(game);
            self.types = xpr_dex::types_for_game(game);
            let font = palette::px(cx.theme, 14.0);
            let widest = self.table.iter().map(|m| widgets::text_w(ui, &m.name, &font)).fold(0.0_f32, f32::max);
            let any_new = xpr_dex::game_gen(game) > 1 && self.table.iter().any(|m| m.new_in_gen);
            self.name_col = widest + 12.0 + if any_new { 30.0 } else { 0.0 };
            if self.filters.type_filter != ALL && !self.types.contains(&self.filters.type_filter) {
                self.filters.type_filter = ALL.to_string();
            }
            self.view_sig = None;
            self.panel = Default::default();
        }
        let sig = (game.to_string(), self.filters.clone(), self.sort);
        if self.view_sig.as_ref() != Some(&sig) {
            let mut idx: Vec<usize> = self.table.iter().enumerate().filter(|(_, m)| self.filters.matches(m)).map(|(i, _)| i).collect();
            data::sort_rows(&self.table, &mut idx, self.sort);
            self.view = idx;
            self.view_sig = Some(sig);
        }
    }

    // ---- filters ------------------------------------------------------------------------------

    fn filters_ui(&mut self, ui: &mut Ui, cx: &mut DexCx) {
        let theme = cx.theme;
        let full_w = ui.available_width();
        let col_w = (full_w - 32.0).min(672.0);
        ui.add_space(12.0);

        // search, type, category
        let (type_w, cat_w, search_w) = (112.0, 140.0, 178.0);
        let types: Vec<String> = std::iter::once("All Types".to_string()).chain(self.types.iter().cloned()).collect();
        let cats: Vec<String> = std::iter::once("All Categories".to_string()).chain(CATEGORIES.iter().map(|c| c.to_string())).collect();
        // the combos' height with a 7 px vertical padding sets the row height
        let pad_y = 7.0;
        let strip_h = option_menu_height(ui, theme) - 2.0 * ui.spacing().button_padding.y + 2.0 * pad_y;
        kit::centered_strip(ui, search_w + type_w + cat_w + 16.0, strip_h, 8.0, Align::Min, |ui| {
            Entry::new(theme, &mut self.filters.search)
                .width(search_w)
                .hint("Search moves\u{2026}")
                .id(Id::new("movedex_search"))
                .clearable()
                .font(palette::px(theme, 14.0))
                .margin(Margin::symmetric(8, 6))
                .min_height(strip_h)
                .show(ui);
            ui.spacing_mut().button_padding.y = pad_y;
            let mut t = if self.filters.type_filter == ALL { "All Types".to_string() } else { self.filters.type_filter.clone() };
            if option_menu(ui, theme, Id::new("movedex_type"), &mut t, &types, Some(type_w), true) {
                self.filters.type_filter = if t == "All Types" { ALL.to_string() } else { t };
            }
            let mut c = if self.filters.category == ALL { "All Categories".to_string() } else { self.filters.category.clone() };
            if option_menu(ui, theme, Id::new("movedex_category"), &mut c, &cats, Some(cat_w), true) {
                self.filters.category = if c == "All Categories" { ALL.to_string() } else { c };
            }
        });
        ui.add_space(8.0);

        // Power / Accuracy / PP ranges (flex-wrap, centred)
        let per_row = (((col_w + 12.0) / (RANGE_W + 12.0)).floor() as usize).clamp(1, 3);
        let ranges: [(&str, usize); 3] = [("Power", 0), ("Accuracy", 1), ("PP", 2)];
        for chunk in ranges.chunks(per_row) {
            let total = chunk.len() as f32 * RANGE_W + (chunk.len() - 1) as f32 * 12.0;
            kit::centered_strip(ui, total, 46.0, 12.0, Align::Min, |ui| {
                for (label, which) in chunk {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(RANGE_W, 46.0), Sense::hover());
                    ui.painter().text(rect.min + Vec2::new(0.0, 8.0), Align2::LEFT_CENTER, *label, palette::px_bold(theme, 12.0), palette::GRAY_500);
                    let mut row = kit::row_ui(ui, Rect::from_min_size(rect.min + Vec2::new(0.0, 20.0), Vec2::new(RANGE_W, 26.0)), 4.0, Align::Min);
                    let f = &mut self.filters;
                    let (lo, hi) = match which {
                        0 => (&mut f.min_power, &mut f.max_power),
                        1 => (&mut f.min_accuracy, &mut f.max_accuracy),
                        _ => (&mut f.min_pp, &mut f.max_pp),
                    };
                    for (hint, text, key) in [("Min", lo, "min"), ("Max", hi, "max")] {
                        let r = Entry::new(theme, text).width(64.0).hint(hint).id(Id::new(("movedex_range", *which, key))).font(palette::px(theme, 12.0)).margin(Margin::symmetric(6, 4)).min_height(26.0).show(&mut row);
                        if r.changed {
                            num_filter_changed(text);
                        }
                    }
                }
            });
        }

        // newly introduced toggle
        ui.add_space(4.0);
        let label = "Newly introduced in this generation";
        let sw_w = 28.0 + 8.0 + widgets::text_w(ui, label, &palette::px(theme, 12.0));
        kit::centered_strip(ui, sw_w, 16.0, 0.0, Align::Center, |ui| {
            kit::switch(ui, theme, &mut self.filters.only_new, label);
        });
        ui.add_space(12.0);
        let (line, _) = ui.allocate_exact_size(Vec2::new(full_w, 1.0), Sense::hover());
        kit::rule_bottom(ui, line, RULE);
    }

    // ---- table --------------------------------------------------------------------------------

    fn table_ui(&mut self, ui: &mut Ui, cx: &mut DexCx) {
        let theme = cx.theme;
        let region = ui.available_rect_before_wrap();
        let name_col = self.name_col;
        let table_w = name_col + TYPE_W + CAT_W + 3.0 * NUM_W + TM_W;
        let left_x = (region.center().x - table_w / 2.0).max(region.min.x);
        self.header_ui(ui, theme, Pos2::new(left_x, region.min.y));

        let body = Rect::from_min_max(Pos2::new(region.min.x, region.min.y + HEADER_H), region.max);
        let mut sui = ui.new_child(UiBuilder::new().id_salt("movedex_table_body").max_rect(body).layout(Layout::top_down(Align::Min)));
        sui.set_clip_rect(body.intersect(ui.clip_rect()));
        let game = cx.state.game().to_string();
        let mut right_clicked: Option<String> = None;
        let mut open_detail: Option<String> = None;
        let mut open_url: Option<String> = None;
        let scroll_to = self.scroll_to.take();
        // `animated(false)`: Solodex jumps to the row at once
        egui::ScrollArea::vertical().id_salt("movedex_table").auto_shrink([false, false]).animated(false).show(&mut sui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let n = self.view.len();
            let (area, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), (n as f32 * ROW_H + 4.0).max(0.0)), Sense::hover());
            if n == 0 {
                ui.painter().text(Pos2::new(area.center().x, area.min.y + 32.0), Align2::CENTER_CENTER, "No moves match the current filters.", palette::px(theme, 14.0), palette::GRAY_500);
                return;
            }
            if let Some(name) = &scroll_to {
                if let Some(i) = self.view.iter().position(|&t| self.table[t].name == *name) {
                    ui.scroll_to_rect(Rect::from_min_size(Pos2::new(area.min.x, area.min.y + i as f32 * ROW_H), Vec2::new(area.width(), ROW_H)), Some(Align::Center));
                }
            }
            let clip = ui.clip_rect();
            let first = (((clip.min.y - area.min.y) / ROW_H).floor().max(0.0) as usize).min(n);
            let last = (((clip.max.y - area.min.y) / ROW_H).ceil().max(0.0) as usize + 1).min(n);
            for i in first..last {
                let row = &self.table[self.view[i]];
                let top = area.min.y + i as f32 * ROW_H;
                let rect = Rect::from_min_size(Pos2::new(left_x, top), Vec2::new(table_w, ROW_H));
                let cols = col_rects(left_x, top, ROW_H, name_col);
                let id = Id::new(("movedex_row", &row.name));
                let resp = ui.interact(rect, id, Sense::click()).on_hover_cursor(egui::CursorIcon::ContextMenu);
                let bg = ui.painter().add(Shape::Noop);
                kit::rule_top(ui, rect, RULE);
                let font = palette::px(theme, 14.0);
                let name_w = widgets::text_w(ui, &row.name, &font);
                let name_rect = Rect::from_min_size(Pos2::new(cols[0].min.x + 6.0, top + 1.0), Vec2::new(name_w, ROW_H - 1.0));
                let name_resp = ui.interact(name_rect, id.with("name"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                // the name opens a summary popover (Solodex's wiki popover)
                widgets::click_popover(theme, &name_resp, Some(300.0), |ui| move_popover(ui, theme, row, &mut open_detail, &mut open_url));
                // the name, the badge and the row are separate widgets; any of them counts
                let mut hovered = resp.hovered() || name_resp.hovered();
                let mut right = resp.secondary_clicked() || name_resp.secondary_clicked();

                ui.painter().text(Pos2::new(name_rect.min.x, rect.center().y + 0.5), Align2::LEFT_CENTER, &row.name, font, if hovered { Color32::WHITE } else { palette::GRAY_100 });
                if row.new_in_gen && xpr_dex::game_gen(&game) > 1 {
                    let r = ui.painter().text(Pos2::new(name_rect.max.x + 6.0, rect.center().y + 0.5), Align2::LEFT_CENTER, "NEW", palette::px_bold(theme, 10.0), palette::AMBER_500);
                    ui.interact(r.expand(2.0), id.with("new"), Sense::hover()).on_hover_text("Newly introduced in this generation");
                }
                // type badge (clickable: matchups)
                let badge_rect = Rect::from_min_size(Pos2::new(cols[1].min.x + 6.0, top + 5.0), Vec2::new(68.0, 18.0));
                let mut bui = ui.new_child(UiBuilder::new().id_salt(("movedex_badge", &row.name)).max_rect(badge_rect));
                let b = widgets::type_badge(&mut bui, theme, &row.data.move_type, true, Some(&game));
                hovered |= b.hovered();
                right |= b.secondary_clicked();
                // category, power, accuracy, PP, TM
                let ty = rect.center().y + 0.5;
                let cat = if row.data.category.is_empty() { "\u{2014}" } else { row.data.category.as_str() };
                ui.painter().text(Pos2::new(cols[2].min.x + 6.0, ty), Align2::LEFT_CENTER, cat, palette::px(theme, 12.0), palette::category_color(cat));
                let num = |v: Option<i32>| v.map(|v| v.to_string()).unwrap_or_else(|| "\u{2014}".to_string());
                ui.painter().text(Pos2::new(cols[3].max.x - 6.0, ty), Align2::RIGHT_CENTER, num(row.data.power), palette::px(theme, 14.0), palette::GRAY_100);
                ui.painter().text(Pos2::new(cols[4].max.x - 6.0, ty), Align2::RIGHT_CENTER, num(row.data.accuracy), palette::px(theme, 14.0), palette::GRAY_100);
                ui.painter().text(Pos2::new(cols[5].max.x - 6.0, ty), Align2::RIGHT_CENTER, num(row.data.pp), palette::px(theme, 14.0), palette::GRAY_400);
                if let Some(tm) = &row.tm {
                    ui.painter().text(Pos2::new(cols[6].min.x + 6.0, ty), Align2::LEFT_CENTER, tm, palette::px(theme, 12.0), palette::GRAY_400);
                }
                let selected = self.selected.as_deref() == Some(row.name.as_str());
                let highlighted = self.highlighted.as_deref() == Some(row.name.as_str());
                let fill = if highlighted {
                    Some(tw("#1e3a8a", 0.4))
                } else if selected {
                    Some(palette::GRAY_800)
                } else if hovered {
                    Some(tw("#1f2937", 0.6))
                } else {
                    None
                };
                if let Some(f) = fill {
                    ui.painter().set(bg, Shape::rect_filled(Rect::from_min_max(rect.min + Vec2::new(0.0, 1.0), rect.max), CornerRadius::ZERO, f));
                }
                if right {
                    right_clicked = Some(row.name.clone());
                }
            }
        });
        if let Some(name) = right_clicked {
            self.selected = if self.selected.as_deref() == Some(name.as_str()) { None } else { Some(name) };
            ui.ctx().request_repaint();
        }
        if let Some(name) = open_detail {
            cx.state.focused_move = Some(name);
            ui.ctx().request_repaint();
        }
        if let Some(url) = open_url {
            cx.actions.push(crate::DexAction::OpenUrl(url));
        }
    }

    fn header_ui(&mut self, ui: &mut Ui, theme: &Theme, origin: Pos2) {
        let cols = col_rects(origin.x, origin.y, HEADER_H, self.name_col);
        let font = palette::px_bold(theme, 12.0);
        for (col, rect) in Col::ALL.iter().zip(cols) {
            let resp = ui.interact(rect, Id::new(("movedex_header", col.label())), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            let active = self.sort.active(*col);
            let color = if active.is_some() {
                palette::GRAY_300
            } else if resp.hovered() {
                palette::GRAY_400
            } else {
                palette::GRAY_500
            };
            let text = match active {
                Some(SortDir::Asc) => format!("{} \u{25B2}", col.label()),
                Some(SortDir::Desc) => format!("{} \u{25BC}", col.label()),
                None => col.label().to_string(),
            };
            let (pos, align) = if col.right_aligned() { (Pos2::new(rect.max.x - 6.0, rect.center().y), Align2::RIGHT_CENTER) } else { (Pos2::new(rect.min.x + 6.0, rect.center().y), Align2::LEFT_CENTER) };
            ui.painter().text(pos, align, text, font.clone(), color);
            if resp.clicked() {
                self.sort = self.sort.clicked(*col);
            }
            if resp.secondary_clicked() {
                self.sort = Sort::default();
            }
        }
    }

    // ---- learner panel ------------------------------------------------------------------------

    fn panel_ui(&mut self, ui: &mut Ui, cx: &mut DexCx, rect: Rect) {
        let theme = cx.theme;
        let Some(name) = self.selected.clone() else { return };
        let game = cx.state.game().to_string();
        ui.painter().vline(rect.min.x + 0.5, rect.y_range(), Stroke::new(1.0_f32, palette::GRAY_700));
        let inner = Rect::from_min_max(Pos2::new(rect.min.x + 1.0, rect.min.y), rect.max);
        let count = self.panel.learners(&name, &game).len();

        // header: name, count, close
        let header = Rect::from_min_size(inner.min, Vec2::new(inner.width(), 36.0));
        let close = Rect::from_center_size(Pos2::new(header.max.x - 20.0, header.center().y), Vec2::new(24.0, 24.0));
        let close_resp = ui.interact(close, Id::new("movedex_panel_close"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        ui.painter().text(close.center(), Align2::CENTER_CENTER, "\u{d7}", palette::px(theme, 18.0), if close_resp.hovered() { palette::GRAY_300 } else { palette::GRAY_500 });
        let name_font = palette::px_bold(theme, 14.0);
        let shown = xpr_ui_kit::widgets::elide(ui, &name, &name_font, (close.min.x - header.min.x - 12.0 - 30.0).max(40.0));
        let r = ui.painter().text(Pos2::new(header.min.x + 12.0, header.center().y), Align2::LEFT_CENTER, shown, name_font, palette::GRAY_100);
        ui.painter().text(Pos2::new(r.max.x + 8.0, header.center().y + 1.0), Align2::LEFT_CENTER, count.to_string(), palette::px(theme, 12.0), palette::GRAY_500);
        kit::rule_bottom(ui, header, RULE);
        if close_resp.clicked() {
            self.selected = None;
            ui.ctx().request_repaint();
            return;
        }

        let body = Rect::from_min_max(Pos2::new(inner.min.x, header.max.y), inner.max);
        if count == 0 {
            let galley = ui.fonts_mut(|f| f.layout(format!("No Pokemon learn this move in {}.", game), palette::px(theme, 14.0), palette::GRAY_500, body.width() - 24.0));
            let pos = Pos2::new(body.center().x - galley.size().x / 2.0, body.min.y + 32.0);
            ui.painter().galley(pos, galley, palette::GRAY_500);
            return;
        }
        let style = Style { sprite: false, name_px: 12.0, width: body.width() - 8.0 };
        let total = self.panel.height(ui, theme, style);
        // sticky table header
        let th = Rect::from_min_size(Pos2::new(body.min.x + 4.0, body.min.y), Vec2::new(style.width, HEADER_H));
        let hfont = palette::px_bold(theme, 12.0);
        ui.painter().text(Pos2::new(th.min.x + 4.0, th.center().y), Align2::LEFT_CENTER, "Pokemon", hfont.clone(), palette::GRAY_500);
        ui.painter().text(Pos2::new(th.min.x + self.panel.name_col(), th.center().y), Align2::LEFT_CENTER, "Method", hfont, palette::GRAY_500);
        let scroll = Rect::from_min_max(Pos2::new(body.min.x, th.max.y), body.max);
        let mut sui = ui.new_child(UiBuilder::new().id_salt("movedex_panel_body").max_rect(scroll).layout(Layout::top_down(Align::Min)));
        sui.set_clip_rect(scroll.intersect(ui.clip_rect()));
        let mut clicked = None;
        egui::ScrollArea::vertical().id_salt("movedex_panel_scroll").auto_shrink([false, false]).show(&mut sui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let (area, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), total + 4.0), Sense::hover());
            let table = Rect::from_min_size(Pos2::new(area.min.x + 4.0, area.min.y), Vec2::new(style.width, total));
            clicked = self.panel.show(ui, cx, table, style, Id::new("movedex_panel_rows"));
        });
        if let Some(species) = clicked {
            learners::open_in_pokedex(cx, &species);
            ui.ctx().request_repaint();
        }
    }

    // ---- keys and the jump overlay --------------------------------------------------------------

    /// Space opens the jump overlay (Solodex ignores it in text fields). Raw
    /// key read, so it stands down under a dialog, an open menu / popover,
    /// the shell's search overlays and while a text field has focus.
    fn handle_keys(&mut self, ui: &Ui, cx: &DexCx) {
        if self.jump.is_some() || cx.state.spotlight.is_some() || cx.state.banned_editor_open {
            return;
        }
        if pointer_blocked(ui) || ui.ctx().wants_keyboard_input() {
            return;
        }
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Space)) {
            self.jump = Some(SpotlightState::default());
            ui.ctx().request_repaint();
        }
    }

    fn jump_overlay(&mut self, ctx: &egui::Context, cx: &mut DexCx) {
        let Some(state) = self.jump.as_mut() else { return };
        let q = state.query.trim().to_lowercase();
        let rows: Vec<SpotlightRow> = self
            .table
            .iter()
            .filter(|m| self.filters.matches(m))
            .filter(|m| q.is_empty() || m.lower.contains(&q))
            .map(|m| SpotlightRow { key: m.name.clone(), label: m.name.clone(), detail: jump_detail(m), sprite: None, color: None })
            .collect();
        match widgets::spotlight(ctx, cx.theme, cx.images, "movedex_jump", "Jump to move", hex("#1d9bf0"), state, &rows) {
            SpotlightOutcome::Open => {}
            SpotlightOutcome::Closed => self.jump = None,
            SpotlightOutcome::Picked(name) => {
                self.highlighted = Some(name.clone());
                self.scroll_to = Some(name);
                self.jump = None;
            }
        }
        ctx.request_repaint();
    }
}

/// "Electric · 95" (type and power, as Solodex's jump list shows).
fn jump_detail(m: &MoveRow) -> String {
    match m.data.power {
        Some(p) => format!("{} \u{b7} {}", m.data.move_type, p),
        None => m.data.move_type.clone(),
    }
}

/// The popover of a move's name: description, stats, and the links.
fn move_popover(ui: &mut Ui, theme: &Theme, row: &MoveRow, open_detail: &mut Option<String>, open_url: &mut Option<String>) {
    ui.set_min_width(300.0);
    ui.set_max_width(300.0);
    ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(&row.name).font(palette::px_bold(theme, 16.0)).color(Color32::WHITE));
        widgets::type_chip(ui, theme, &row.data.move_type, 68.0);
        ui.label(egui::RichText::new(&row.data.category).font(palette::px_bold(theme, 12.0)).color(palette::category_color(&row.data.category)));
    });
    let fmt = |v: Option<i32>| v.map(|v| v.to_string()).unwrap_or_else(|| "\u{2014}".to_string());
    let stats = format!("Power {}   Accuracy {}   PP {}   Priority {}", fmt(row.data.power), fmt(row.data.accuracy), fmt(row.data.pp), row.data.priority);
    ui.add(egui::Label::new(egui::RichText::new(stats).font(palette::px(theme, 12.0)).color(palette::GRAY_400)).wrap());
    if !row.data.description.is_empty() {
        ui.add(egui::Label::new(egui::RichText::new(&row.data.description).font(palette::px(theme, 12.0)).color(palette::GRAY_300)).wrap());
    }
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        if widgets::small_button(ui, theme, "Move details").clicked() {
            *open_detail = Some(row.name.clone());
            ui.close();
        }
        if widgets::small_button(ui, theme, "Bulbapedia").on_hover_text("Open the move's Bulbapedia article").clicked() {
            *open_url = Some(data::bulbapedia_url(&row.name));
            ui.close();
        }
    });
}
