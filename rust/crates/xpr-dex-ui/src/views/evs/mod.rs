//! EVs tab: EV yields of every species of a game with filters and the
//! encounter popover, plus the spread card. Port of Solodex
//! `EVComparisonView.tsx`, `SpreadCard.tsx`.
//!
//! The table is Solodex's: a 356 px column of `# / sprite / Pokémon / one
//! column per stat`, centred under a row of four filters (generation, type,
//! stat, EV yield). Clicking a header sorts, clicking a row opens the species
//! in the Pokédex, and hovering a name shows where the species is found in
//! the game. [`spread_card`] is the damage view's spread graphic; it is a
//! plain widget the Damage tab can draw.

pub mod spread_card;

use std::collections::HashSet;
use std::sync::Arc;

use egui::{Align2, Color32, CornerRadius, Id, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use xpr_dex::text::locale_cmp;
use xpr_dex::{EncounterEntry, PokemonData, StatKey};
use xpr_ui_kit::modal::pointer_blocked;
use xpr_ui_kit::theme::{self, Theme};

pub use spread_card::{show_spread_card, spread_card_height, SpreadCardProps, SPREAD_CARD_WIDTH};

use crate::widgets::SortDir;
use crate::{palette, DexCx, DexTab};

/// National dex ranges by generation (inclusive).
const GEN_RANGES: [(u8, i32, i32); 9] = [
    (1, 1, 151),
    (2, 152, 251),
    (3, 252, 386),
    (4, 387, 493),
    (5, 494, 649),
    (6, 650, 721),
    (7, 722, 809),
    (8, 810, 905),
    (9, 906, 1025),
];

const TYPES: [&str; 18] = [
    "Normal", "Fire", "Water", "Electric", "Grass", "Ice", "Fighting", "Poison", "Ground",
    "Flying", "Psychic", "Bug", "Rock", "Ghost", "Dragon", "Dark", "Steel", "Fairy",
];

/// Column widths (Solodex: `#` 28, sprite 36, Pokémon 100, stats 32 each;
/// the `#` column gives 2 px to its number and takes them from the name;
/// the table is 356 wide, so a gen 1 table with five stats stretches its
/// columns proportionally like `table-fixed` does).
const TABLE_W: f32 = 356.0;
const COL_NUM: f32 = 30.0;
const COL_SPRITE: f32 = 36.0;
const COL_NAME: f32 = 98.0;
const COL_STAT: f32 = 32.0;
const ROW_H: f32 = 40.0;
const HEAD_H: f32 = 29.0;
const SCROLL_BAR_W: f32 = 10.0;

/// The popover box (Solodex: `width: 360`, `maxHeight: 320`, list `max-h-64`).
const POPOVER_W: f32 = 360.0;
const POPOVER_MAX_H: f32 = 320.0;
const POPOVER_LIST_H: f32 = 256.0;
/// How long the popover outlives the pointer (Solodex `scheduleClose`).
const CLOSE_DELAY: f64 = 0.12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Amount {
    All,
    AtLeast(i32),
    Exactly(i32),
}

impl Amount {
    const OPTIONS: [Amount; 7] = [
        Amount::All,
        Amount::AtLeast(1),
        Amount::AtLeast(2),
        Amount::AtLeast(3),
        Amount::Exactly(1),
        Amount::Exactly(2),
        Amount::Exactly(3),
    ];

    fn label(self) -> String {
        match self {
            Amount::All => "All".to_string(),
            Amount::AtLeast(n) => format!("\u{2265} {}", n),
            Amount::Exactly(n) => format!("= {}", n),
        }
    }
}

/// Sort state: `by == None` sorts by Pokédex number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Sort {
    by: Option<StatKey>,
    dir: SortDir,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Filters {
    gen: Option<u8>,
    ty: Option<String>,
    stat: Option<StatKey>,
    amount: Amount,
}

impl Default for Filters {
    fn default() -> Self {
        Filters {
            gen: None,
            ty: None,
            stat: None,
            amount: Amount::All,
        }
    }
}

/// What a game's Pokédex offers the filters.
struct GameList {
    game: String,
    list: Vec<Arc<PokemonData>>,
    gens: HashSet<u8>,
    types: HashSet<String>,
}

impl GameList {
    fn load(game: &str) -> GameList {
        let list = xpr_dex::get_all_pokemon_for_game(game);
        let mut gens = HashSet::new();
        let mut types = HashSet::new();
        for p in &list {
            for (gen, min, max) in GEN_RANGES {
                if p.national_dex_number >= min && p.national_dex_number <= max {
                    gens.insert(gen);
                }
            }
            if !p.type_1.is_empty() {
                types.insert(p.type_1.clone());
            }
            if !p.type_2.is_empty() {
                types.insert(p.type_2.clone());
            }
        }
        GameList {
            game: game.to_string(),
            list,
            gens,
            types,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct CacheKey {
    game: String,
    filters: Filters,
    sort: Sort,
}

/// An encounter location with its method and level range (Solodex groups the
/// entries by location + method).
#[derive(Clone, Debug, PartialEq)]
struct EncounterGroup {
    location: String,
    method: String,
    min_level: i32,
    max_level: i32,
}

/// Group by location + method, sorted by location (`localeCompare`).
fn group_encounters(encounters: &[EncounterEntry]) -> Vec<EncounterGroup> {
    let mut groups: Vec<EncounterGroup> = Vec::new();
    for e in encounters {
        match groups
            .iter_mut()
            .find(|g| g.location == e.location && g.method == e.method)
        {
            Some(g) => {
                g.min_level = g.min_level.min(e.min_level);
                g.max_level = g.max_level.max(e.max_level);
            }
            None => groups.push(EncounterGroup {
                location: e.location.clone(),
                method: e.method.clone(),
                min_level: e.min_level,
                max_level: e.max_level,
            }),
        }
    }
    groups.sort_by(|a, b| locale_cmp(&a.location, &b.location));
    groups
}

/// "Lv 5" / "Lv 5–7".
fn level_text(g: &EncounterGroup) -> String {
    if g.min_level == g.max_level {
        format!("Lv {}", g.min_level)
    } else {
        format!("Lv {}\u{2013}{}", g.min_level, g.max_level)
    }
}

/// The encounter popover that is open (or lingering for [`CLOSE_DELAY`]).
struct Hover {
    species: String,
    anchor: Rect,
    groups: Vec<EncounterGroup>,
    /// when the pointer left the name and the popover
    left_at: Option<f64>,
}

#[derive(Default)]
pub struct EvsView {
    filters: Filters,
    sort: Sort,
    list: Option<GameList>,
    sorted: Option<(CacheKey, Vec<Arc<PokemonData>>)>,
    hover: Option<Hover>,
    /// last frame's popover rectangle (the wheel must not also scroll the table)
    popover_rect: Option<Rect>,
}

impl Default for Sort {
    fn default() -> Self {
        Sort {
            by: None,
            dir: SortDir::Asc,
        }
    }
}

impl EvsView {
    pub fn ui(&mut self, ui: &mut Ui, cx: &mut DexCx) {
        let game = cx.state.game().to_string();
        let gen1 = xpr_dex::game_gen(&game) == 1;
        let columns = palette::stat_keys(gen1);
        self.ensure_list(&game, columns);
        self.filter_bar(ui, cx.theme, columns, gen1);
        self.ensure_sorted(&game, columns);
        let selected = self.table(ui, cx, &game, columns, gen1);
        if let Some(name) = selected {
            // Solodex `onSelectPokemon`: select, then switch to the Pokédex.
            cx.state.select_species(&name);
            cx.state.settings.tab = DexTab::Pokedex;
            cx.state.touch();
        }
    }

    // ---- data ---------------------------------------------------------------------------

    /// Load the game's species and drop filters the game cannot satisfy.
    fn ensure_list(&mut self, game: &str, columns: &[StatKey]) {
        if self.list.as_ref().map(|l| l.game != game).unwrap_or(true) {
            self.list = Some(GameList::load(game));
            self.hover = None;
        }
        let Some(gl) = &self.list else { return };
        if let Some(g) = self.filters.gen {
            if !gl.gens.contains(&g) {
                self.filters.gen = None;
            }
        }
        if let Some(t) = &self.filters.ty {
            if !gl.types.contains(t) {
                self.filters.ty = None;
            }
        }
        // a stat the game lacks (gen 1 has no Special Defense column)
        if let Some(s) = self.filters.stat {
            if !columns.contains(&s) {
                self.filters.stat = None;
            }
        }
    }

    fn passes(&self, p: &PokemonData, columns: &[StatKey]) -> bool {
        let f = &self.filters;
        if let Some(g) = f.gen {
            let Some((_, min, max)) = GEN_RANGES.iter().find(|(gen, _, _)| *gen == g) else {
                return false;
            };
            if p.national_dex_number < *min || p.national_dex_number > *max {
                return false;
            }
        }
        if let Some(t) = &f.ty {
            if p.type_1 != *t && p.type_2 != *t {
                return false;
            }
        }
        let values: Vec<i32> = match f.stat {
            Some(s) => vec![p.ev_yield.get(s)],
            None => columns.iter().map(|k| p.ev_yield.get(*k)).collect(),
        };
        match f.amount {
            Amount::AtLeast(n) => {
                if values.iter().copied().max().unwrap_or(0) < n {
                    return false;
                }
            }
            Amount::Exactly(n) => {
                if !values.iter().any(|v| *v == n) {
                    return false;
                }
            }
            Amount::All => {
                // a stat chosen without an amount: just require some yield
                if f.stat.is_some() && values[0] == 0 {
                    return false;
                }
            }
        }
        true
    }

    fn ensure_sorted(&mut self, game: &str, columns: &[StatKey]) {
        let key = CacheKey {
            game: game.to_string(),
            filters: self.filters.clone(),
            sort: self.sort,
        };
        if self
            .sorted
            .as_ref()
            .map(|(k, _)| *k == key)
            .unwrap_or(false)
        {
            return;
        }
        let Some(gl) = &self.list else { return };
        let mut out: Vec<Arc<PokemonData>> = gl
            .list
            .iter()
            .filter(|p| self.passes(p, columns))
            .cloned()
            .collect();
        let desc = self.sort.dir == SortDir::Desc;
        match self.sort.by {
            None => out.sort_by(|a, b| {
                if desc {
                    b.national_dex_number.cmp(&a.national_dex_number)
                } else {
                    a.national_dex_number.cmp(&b.national_dex_number)
                }
            }),
            Some(k) => out.sort_by(|a, b| {
                let (va, vb) = (a.ev_yield.get(k), b.ev_yield.get(k));
                if va != vb {
                    if desc {
                        vb.cmp(&va)
                    } else {
                        va.cmp(&vb)
                    }
                } else {
                    a.national_dex_number.cmp(&b.national_dex_number)
                }
            }),
        }
        self.sorted = Some((key, out));
    }

    /// Solodex `handleStatHeaderClick`: a new stat sorts high first, the same
    /// stat again flips.
    fn click_stat_header(&mut self, key: StatKey) {
        self.sort = if self.sort.by == Some(key) {
            Sort {
                by: Some(key),
                dir: self.sort.dir.flip(),
            }
        } else {
            Sort {
                by: Some(key),
                dir: SortDir::Desc,
            }
        };
    }

    /// Solodex `handleSortByDex`: flips the dex order, or returns to it.
    fn click_dex_header(&mut self) {
        self.sort = if self.sort.by.is_none() {
            Sort {
                by: None,
                dir: self.sort.dir.flip(),
            }
        } else {
            Sort {
                by: None,
                dir: SortDir::Asc,
            }
        };
    }

    // ---- filters ------------------------------------------------------------------------

    fn filter_bar(&mut self, ui: &mut Ui, theme: &Theme, columns: &[StatKey], gen1: bool) {
        let Some(gl) = &self.list else { return };
        let gen_opts: Vec<SelectOption> = std::iter::once(SelectOption::new("All", true))
            .chain(
                GEN_RANGES
                    .iter()
                    .map(|(g, _, _)| SelectOption::new(format!("Gen {}", g), gl.gens.contains(g))),
            )
            .collect();
        let type_opts: Vec<SelectOption> = std::iter::once(SelectOption::new("All", true))
            .chain(
                TYPES
                    .iter()
                    .map(|t| SelectOption::new(*t, gl.types.contains(*t))),
            )
            .collect();
        let stat_opts: Vec<SelectOption> = std::iter::once(SelectOption::new("All", true))
            .chain(
                columns
                    .iter()
                    .map(|k| SelectOption::new(palette::stat_label(*k, gen1), true)),
            )
            .collect();
        let amount_opts: Vec<SelectOption> = Amount::OPTIONS
            .iter()
            .map(|a| SelectOption::new(a.label(), true))
            .collect();

        let gen_sel = self
            .filters
            .gen
            .and_then(|g| GEN_RANGES.iter().position(|(x, _, _)| *x == g))
            .map(|i| i + 1)
            .unwrap_or(0);
        let type_sel = self
            .filters
            .ty
            .as_deref()
            .and_then(|t| TYPES.iter().position(|x| *x == t))
            .map(|i| i + 1)
            .unwrap_or(0);
        let stat_sel = self
            .filters
            .stat
            .and_then(|s| columns.iter().position(|k| *k == s))
            .map(|i| i + 1)
            .unwrap_or(0);
        let amount_sel = Amount::OPTIONS
            .iter()
            .position(|a| *a == self.filters.amount)
            .unwrap_or(0);

        let controls = [
            ("Generation", "evs_filter_gen", &gen_opts, gen_sel, 0.0_f32),
            ("Type", "evs_filter_type", &type_opts, type_sel, 96.0),
            ("Stat", "evs_filter_stat", &stat_opts, stat_sel, 0.0),
            (
                "EV yield",
                "evs_filter_amount",
                &amount_opts,
                amount_sel,
                0.0,
            ),
        ];
        let widths: Vec<f32> = controls
            .iter()
            .map(|(_, _, opts, _, min_w)| select_width(ui, theme, opts).max(*min_w))
            .collect();

        let full = ui.available_width();
        let gap = 12.0;
        let avail = (full - 32.0).max(80.0);
        // wrap into centred rows like `flex-wrap justify-center`
        let mut rows: Vec<Vec<usize>> = vec![Vec::new()];
        let mut row_w = 0.0;
        for (i, w) in widths.iter().enumerate() {
            if row_w > 0.0 && row_w + gap + w > avail {
                rows.push(Vec::new());
                row_w = 0.0;
            }
            row_w += if row_w > 0.0 { gap + w } else { *w };
            rows.last_mut().unwrap().push(i);
        }
        let label_h = 16.0;
        let ctl_h = SELECT_H;
        let row_h = label_h + 4.0 + ctl_h;
        let total_h = 12.0 + rows.len() as f32 * row_h + (rows.len() - 1) as f32 * gap + 12.0;
        let (bar, _) = ui.allocate_exact_size(Vec2::new(full, total_h + 1.0), Sense::hover());
        ui.painter().hline(
            bar.x_range(),
            bar.max.y - 0.5,
            Stroke::new(1.0_f32, palette::GRAY_800),
        );

        let mut new_gen = None;
        let mut new_type = None;
        let mut new_stat = None;
        let mut new_amount = None;
        let mut y = bar.min.y + 12.0;
        for row in rows {
            let total: f32 =
                row.iter().map(|i| widths[*i]).sum::<f32>() + gap * (row.len() - 1) as f32;
            let mut x = bar.center().x - total / 2.0;
            for i in row {
                let (label, id, opts, sel, _) = &controls[i];
                ui.painter().text(
                    Pos2::new(x, y),
                    Align2::LEFT_TOP,
                    *label,
                    palette::px_bold(theme, 12.0),
                    palette::GRAY_500,
                );
                let rect = Rect::from_min_size(
                    Pos2::new(x, y + label_h + 4.0),
                    Vec2::new(widths[i], ctl_h),
                );
                if let Some(picked) = select(ui, theme, Id::new(*id), rect, opts, *sel) {
                    match i {
                        0 => new_gen = Some(picked),
                        1 => new_type = Some(picked),
                        2 => new_stat = Some(picked),
                        _ => new_amount = Some(picked),
                    }
                }
                x += widths[i] + gap;
            }
            y += row_h + gap;
        }
        if let Some(p) = new_gen {
            self.filters.gen = if p == 0 {
                None
            } else {
                Some(GEN_RANGES[p - 1].0)
            };
        }
        if let Some(p) = new_type {
            self.filters.ty = if p == 0 {
                None
            } else {
                Some(TYPES[p - 1].to_string())
            };
        }
        if let Some(p) = new_stat {
            self.filters.stat = if p == 0 { None } else { Some(columns[p - 1]) };
        }
        if let Some(p) = new_amount {
            self.filters.amount = Amount::OPTIONS[p];
        }
    }

    // ---- table --------------------------------------------------------------------------

    /// Draw the table; returns the species a row click picked.
    fn table(
        &mut self,
        ui: &mut Ui,
        cx: &mut DexCx,
        game: &str,
        columns: &[StatKey],
        gen1: bool,
    ) -> Option<String> {
        let theme = cx.theme;
        let body = ui.available_rect_before_wrap();
        ui.allocate_rect(body, Sense::hover());
        if body.height() < 10.0 {
            return None;
        }
        // columns scale up when there are fewer stats, so the table keeps its width
        let natural = COL_NUM + COL_SPRITE + COL_NAME + COL_STAT * columns.len() as f32;
        let k = TABLE_W / natural;
        let (w_num, w_sprite, w_name, w_stat) =
            (COL_NUM * k, COL_SPRITE * k, COL_NAME * k, COL_STAT * k);
        let table_w = TABLE_W.min((body.width() - SCROLL_BAR_W).max(160.0));
        let area_w = table_w + SCROLL_BAR_W;
        let left = (body.center().x - area_w / 2.0).max(body.min.x);
        let area = Rect::from_min_size(
            Pos2::new(left, body.min.y),
            Vec2::new(area_w, body.height()),
        );

        let mut picked: Option<String> = None;
        let mut hovered_name: Option<(String, Rect)> = None;

        let mut tui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(area)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        tui.set_clip_rect(area.intersect(ui.clip_rect()));
        tui.spacing_mut().item_spacing = Vec2::ZERO;

        // ---- sticky header
        let (head, _) = tui.allocate_exact_size(Vec2::new(table_w, HEAD_H), Sense::hover());
        tui.painter()
            .rect_filled(head, CornerRadius::ZERO, palette::page_bg(theme));
        let mut x = head.min.x;
        let dex_cells = [(w_num, "#"), (w_sprite, ""), (w_name, "Pok\u{e9}mon")];
        for (i, (w, label)) in dex_cells.iter().enumerate() {
            let r = Rect::from_min_size(Pos2::new(x, head.min.y), Vec2::new(*w, HEAD_H));
            x += w;
            if i == 1 {
                continue;
            }
            let resp = tui
                .interact(r, Id::new(("evs_head", i)), Sense::click())
                .on_hover_text("Sort by Pokedex number (click again to reverse)")
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.hovered() {
                tui.painter()
                    .rect_filled(r, CornerRadius::ZERO, palette::GRAY_800);
            }
            let arrow = if self.sort.by.is_none() {
                Some(self.sort.dir)
            } else {
                None
            };
            head_label(&tui, theme, r, label, arrow, Color32::WHITE, false);
            if resp.clicked() {
                self.click_dex_header();
            }
        }
        for key in columns {
            let r = Rect::from_min_size(Pos2::new(x, head.min.y), Vec2::new(w_stat, HEAD_H));
            x += w_stat;
            let label = palette::stat_label(*key, gen1);
            let resp = tui
                .interact(r, Id::new(("evs_head_stat", key.key())), Sense::click())
                .on_hover_text(format!(
                    "Sort by {} EV (click: high first, again: low first)",
                    label
                ))
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.hovered() {
                tui.painter()
                    .rect_filled(r, CornerRadius::ZERO, palette::GRAY_800);
            }
            let arrow = if self.sort.by == Some(*key) {
                Some(self.sort.dir)
            } else {
                None
            };
            head_label(
                &tui,
                theme,
                r,
                label,
                arrow,
                palette::stat_color(*key, gen1),
                true,
            );
            if resp.clicked() {
                self.click_stat_header(*key);
            }
        }
        tui.painter().hline(
            head.x_range(),
            head.max.y - 0.5,
            Stroke::new(1.0_f32, palette::GRAY_700),
        );

        // ---- rows
        let rows: &[Arc<PokemonData>] = self
            .sorted
            .as_ref()
            .map(|(_, v)| v.as_slice())
            .unwrap_or(&[]);
        let scroll_rect = Rect::from_min_max(Pos2::new(area.min.x, head.max.y), area.max);
        // Solodex forwards the wheel from the whole page body to the table.
        // That is a raw read, so it stands down under a dialog or an open menu.
        let pointer = tui.input(|i| i.pointer.hover_pos());
        let on_popover =
            matches!((self.popover_rect, pointer), (Some(r), Some(p)) if r.contains(p));
        let forward = match pointer {
            Some(p)
                if !pointer_blocked(&tui)
                    && !on_popover
                    && body.contains(p)
                    && !scroll_rect.contains(p) =>
            {
                Some(tui.input(|i| i.smooth_scroll_delta.y)).filter(|d| *d != 0.0)
            }
            _ => None,
        };
        if rows.is_empty() {
            let r = tui.available_rect_before_wrap();
            tui.painter().text(
                Pos2::new(r.center().x, r.min.y + 60.0),
                Align2::CENTER_CENTER,
                "No Pok\u{e9}mon match these filters",
                palette::px(theme, 14.0),
                palette::GRAY_600,
            );
        } else {
            let saved = tui.style().clone();
            {
                let handle = palette::GRAY_600;
                let w = &mut tui.style_mut().visuals.widgets;
                w.inactive.bg_fill = handle;
                w.hovered.bg_fill = theme::lighten(handle, 0.15);
            }
            egui::ScrollArea::vertical()
                .id_salt(("evs_rows", game))
                .auto_shrink([false, false])
                .max_height(scroll_rect.height())
                .show_rows(&mut tui, ROW_H, rows.len(), |ui, range| {
                    ui.set_style(saved.clone());
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    if let Some(dy) = forward {
                        ui.scroll_with_delta(Vec2::new(0.0, dy));
                    }
                    for i in range {
                        let p = &rows[i];
                        let (rect, resp) =
                            ui.allocate_exact_size(Vec2::new(table_w, ROW_H), Sense::click());
                        if resp.hovered() {
                            ui.painter().rect_filled(
                                rect,
                                CornerRadius::ZERO,
                                theme::with_alpha(palette::GRAY_800, 179),
                            );
                        }
                        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                        let name_rect = self.draw_row(
                            ui,
                            cx,
                            p,
                            rect,
                            (w_num, w_sprite, w_name, w_stat),
                            columns,
                            gen1,
                        );
                        if resp.hovered()
                            && resp
                                .hover_pos()
                                .map(|pos| name_rect.contains(pos))
                                .unwrap_or(false)
                        {
                            hovered_name = Some((p.species.clone(), name_rect));
                        }
                        if resp.clicked() {
                            picked = Some(p.species.clone());
                        }
                    }
                });
        }
        self.encounter_popover(ui, theme, game, hovered_name);
        picked
    }

    /// One row; returns the rectangle of the name text (the popover's anchor).
    #[allow(clippy::too_many_arguments)]
    fn draw_row(
        &self,
        ui: &mut Ui,
        cx: &mut DexCx,
        p: &PokemonData,
        rect: Rect,
        widths: (f32, f32, f32, f32),
        columns: &[StatKey],
        gen1: bool,
    ) -> Rect {
        let theme = cx.theme;
        let (w_num, w_sprite, w_name, w_stat) = widths;
        let cy = rect.center().y;
        // dex number
        ui.painter().text(
            Pos2::new(rect.min.x + 4.0, cy),
            Align2::LEFT_CENTER,
            format!("{:04}", p.national_dex_number),
            egui::FontId::monospace(11.0),
            palette::GRAY_500,
        );
        // sprite with Solodex's 1 px black outline
        let sprite_box = Rect::from_center_size(
            Pos2::new(rect.min.x + w_num + w_sprite / 2.0, cy),
            Vec2::splat(32.0),
        );
        // a form without its own sprite falls back to the base species' (Solodex `onError`)
        let sprite = cx
            .images
            .sprite(
                ui.ctx(),
                &p.species,
                p.national_dex_number,
                crate::SpriteSize::Small,
            )
            .or_else(|| {
                cx.images.sprite(
                    ui.ctx(),
                    "",
                    p.national_dex_number,
                    crate::SpriteSize::Small,
                )
            });
        if let Some(tex) = sprite {
            if ui.is_rect_visible(sprite_box) {
                for d in [
                    Vec2::new(1.0, 0.0),
                    Vec2::new(-1.0, 0.0),
                    Vec2::new(0.0, 1.0),
                    Vec2::new(0.0, -1.0),
                ] {
                    crate::images::paint_fit(ui, &tex, sprite_box.translate(d), Color32::BLACK);
                }
                crate::images::paint_fit(ui, &tex, sprite_box, Color32::WHITE);
            }
        }
        // name, clipped to its cell
        let name = xpr_dex::display_name(&p.species);
        let name_x = rect.min.x + w_num + w_sprite + 4.0;
        let cell_w = w_name - 8.0;
        let font = palette::px(theme, 14.0);
        let full_w = crate::widgets::text_w(ui, name, &font);
        let shown = if full_w > cell_w {
            xpr_ui_kit::widgets::elide(ui, name, &font, cell_w)
        } else {
            name.to_string()
        };
        ui.painter().text(
            Pos2::new(name_x, cy),
            Align2::LEFT_CENTER,
            &shown,
            font,
            Color32::WHITE,
        );
        let name_rect = Rect::from_min_size(
            Pos2::new(name_x, cy - 10.0),
            Vec2::new(full_w.min(cell_w), 20.0),
        );
        // EV yields, tinted when there is a yield
        let mut x = rect.min.x + w_num + w_sprite + w_name;
        for key in columns {
            let v = p.ev_yield.get(*key);
            let color = if v > 0 {
                palette::stat_color(*key, gen1)
            } else {
                palette::GRAY_300
            };
            ui.painter().text(
                Pos2::new(x + w_stat - 4.0, cy),
                Align2::RIGHT_CENTER,
                v.to_string(),
                palette::px(theme, 14.0),
                color,
            );
            x += w_stat;
        }
        name_rect
    }

    // ---- encounter popover --------------------------------------------------------------

    /// Show / keep / close the popover. It opens when the pointer is on a
    /// name that has encounters, and closes [`CLOSE_DELAY`] after the pointer
    /// leaves both the name and the popover.
    fn encounter_popover(
        &mut self,
        ui: &mut Ui,
        theme: &Theme,
        game: &str,
        hovered_name: Option<(String, Rect)>,
    ) {
        let now = ui.input(|i| i.time);
        let on_name = hovered_name.is_some();
        if let Some((species, anchor)) = hovered_name {
            if self
                .hover
                .as_ref()
                .map(|h| h.species == species)
                .unwrap_or(false)
            {
                let h = self.hover.as_mut().unwrap();
                h.anchor = anchor;
            } else {
                let groups = group_encounters(&xpr_dex::get_encounters_for_pokemon(game, &species));
                // Solodex only shows it with encounter data
                self.hover = if groups.is_empty() {
                    None
                } else {
                    Some(Hover {
                        species,
                        anchor,
                        groups,
                        left_at: None,
                    })
                };
            }
        }
        self.popover_rect = None;
        let Some(h) = &self.hover else { return };
        let screen = ui.ctx().content_rect();
        let left = if h.anchor.max.x + 6.0 + POPOVER_W <= screen.max.x {
            h.anchor.max.x + 6.0
        } else {
            (h.anchor.min.x - POPOVER_W - 6.0).max(6.0)
        };
        let top = h
            .anchor
            .min
            .y
            .max(6.0)
            .min(screen.max.y - POPOVER_MAX_H - 6.0);
        let area = egui::Area::new(Id::new("evs_encounter_popover"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::new(left, top))
            .constrain(false)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(palette::GRAY_900)
                    .stroke(Stroke::new(1.0_f32, palette::GRAY_700))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(egui::Margin::same(12))
                    .shadow(egui::Shadow {
                        offset: [0, 25],
                        blur: 50,
                        spread: 0,
                        color: Color32::from_black_alpha(100),
                    })
                    .show(ui, |ui| popover_contents(ui, theme, &h.species, &h.groups));
            });
        let rect = area.response.rect;
        self.popover_rect = Some(rect);
        let pointer_on_popover = area.response.contains_pointer();
        let h = self.hover.as_mut().unwrap();
        if pointer_on_popover || on_name {
            h.left_at = None;
        } else {
            let left_at = *h.left_at.get_or_insert(now);
            if now - left_at >= CLOSE_DELAY {
                self.hover = None;
                self.popover_rect = None;
            } else {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(30));
            }
        }
    }
}

fn popover_contents(ui: &mut Ui, theme: &Theme, species: &str, groups: &[EncounterGroup]) {
    // 360 box - 2 x 12 padding - 2 x 1 border
    ui.set_width(POPOVER_W - 26.0);
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    let line = |ui: &mut Ui, text: &str, font: egui::FontId, color: Color32, h: f32| {
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
        let shown = xpr_ui_kit::widgets::elide(ui, text, &font, r.width());
        ui.painter().text(
            Pos2::new(r.min.x, r.center().y),
            Align2::LEFT_CENTER,
            shown,
            font,
            color,
        );
    };
    line(
        ui,
        xpr_dex::display_name(species),
        palette::px_bold(theme, 14.0),
        Color32::WHITE,
        20.0,
    );
    line(
        ui,
        "Encounter locations in this game",
        palette::px(theme, 11.0),
        palette::GRAY_400,
        16.0,
    );
    ui.add_space(8.0);
    egui::ScrollArea::vertical()
        .id_salt("evs_encounter_list")
        .max_height(POPOVER_LIST_H)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            ui.set_width(ui.available_width() - 4.0);
            for (i, g) in groups.iter().enumerate() {
                if i > 0 {
                    ui.add_space(4.0);
                }
                line(
                    ui,
                    &g.location,
                    palette::px(theme, 11.0),
                    palette::GRAY_100,
                    16.0,
                );
                line(
                    ui,
                    &format!("{} \u{b7} {}", g.method, level_text(g)),
                    palette::px(theme, 11.0),
                    palette::GRAY_400,
                    16.0,
                );
            }
        });
}

/// A header cell's text, with the sort arrow in gray after it.
fn head_label(
    ui: &Ui,
    theme: &Theme,
    rect: Rect,
    label: &str,
    arrow: Option<SortDir>,
    color: Color32,
    right: bool,
) {
    let font = palette::px_bold(theme, 12.0);
    let arrow_font = palette::px(theme, 12.0);
    let arrow_text = arrow.map(|d| {
        if d == SortDir::Desc {
            "\u{2193}"
        } else {
            "\u{2191}"
        }
    });
    let lw = crate::widgets::text_w(ui, label, &font);
    let aw = arrow_text
        .map(|t| 2.0 + crate::widgets::text_w(ui, t, &arrow_font))
        .unwrap_or(0.0);
    let total = lw + aw;
    let x0 = if right {
        rect.max.x - 4.0 - total
    } else {
        rect.min.x + 4.0
    };
    let cy = rect.center().y;
    ui.painter()
        .text(Pos2::new(x0, cy), Align2::LEFT_CENTER, label, font, color);
    if let Some(t) = arrow_text {
        ui.painter().text(
            Pos2::new(x0 + lw + 2.0, cy),
            Align2::LEFT_CENTER,
            t,
            arrow_font,
            palette::GRAY_500,
        );
    }
}

// ---------------------------------------------------------------------------
// a `<select>`: gray-800 box, gray-700 border, optional disabled entries
// ---------------------------------------------------------------------------

const SELECT_H: f32 = 30.0;
const OPTION_H: f32 = 24.0;

struct SelectOption {
    label: String,
    enabled: bool,
}

impl SelectOption {
    fn new(label: impl Into<String>, enabled: bool) -> SelectOption {
        SelectOption {
            label: label.into(),
            enabled,
        }
    }
}

fn select_width(ui: &Ui, theme: &Theme, options: &[SelectOption]) -> f32 {
    let font = palette::px(theme, 14.0);
    let widest = options
        .iter()
        .map(|o| crate::widgets::text_w(ui, &o.label, &font))
        .fold(0.0_f32, f32::max);
    // 8 px padding each side, 1 px border, the native arrow
    (widest + 16.0 + 2.0 + 22.0).ceil()
}

/// A drop-down in `rect`; returns the index picked this frame.
fn select(
    ui: &mut Ui,
    theme: &Theme,
    id: Id,
    rect: Rect,
    options: &[SelectOption],
    selected: usize,
) -> Option<usize> {
    let resp = ui
        .interact(rect, id, Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp));
    let border = if open || resp.hovered() {
        palette::GRAY_500
    } else {
        palette::GRAY_700
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(4),
        palette::GRAY_800,
        Stroke::new(1.0_f32, border),
        egui::StrokeKind::Inside,
    );
    let font = palette::px(theme, 14.0);
    let label = options
        .get(selected)
        .map(|o| o.label.as_str())
        .unwrap_or("");
    ui.painter().text(
        Pos2::new(rect.min.x + 9.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font.clone(),
        palette::GRAY_200,
    );
    // chevron
    let c = Pos2::new(rect.max.x - 13.0, rect.center().y);
    let s = Stroke::new(1.5_f32, palette::GRAY_400);
    ui.painter()
        .line_segment([c + Vec2::new(-4.0, -2.0), c + Vec2::new(0.0, 2.0)], s);
    ui.painter()
        .line_segment([c + Vec2::new(0.0, 2.0), c + Vec2::new(4.0, -2.0)], s);

    let mut picked = None;
    let min_w = rect.width();
    egui::Popup::from_toggle_button_response(&resp)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .frame(
            egui::Frame::new()
                .fill(palette::GRAY_800)
                .stroke(Stroke::new(1.0_f32, palette::GRAY_600))
                .corner_radius(CornerRadius::same(4))
                .inner_margin(egui::Margin::same(2))
                .shadow(egui::Shadow {
                    offset: [0, 6],
                    blur: 16,
                    spread: 0,
                    color: Color32::from_black_alpha(120),
                }),
        )
        .show(|ui| {
            ui.set_width(min_w - 6.0);
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            for (i, o) in options.iter().enumerate() {
                let (r, rr) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), OPTION_H),
                    if o.enabled {
                        Sense::click()
                    } else {
                        Sense::hover()
                    },
                );
                if i == selected {
                    ui.painter()
                        .rect_filled(r, CornerRadius::same(3), palette::GRAY_600);
                } else if o.enabled && rr.hovered() {
                    ui.painter()
                        .rect_filled(r, CornerRadius::same(3), palette::GRAY_700);
                }
                let color = if !o.enabled {
                    palette::GRAY_600
                } else if i == selected || rr.hovered() {
                    Color32::WHITE
                } else {
                    palette::GRAY_200
                };
                ui.painter().text(
                    Pos2::new(r.min.x + 7.0, r.center().y),
                    Align2::LEFT_CENTER,
                    &o.label,
                    font.clone(),
                    color,
                );
                if o.enabled && rr.clicked() {
                    picked = Some(i);
                    ui.close();
                }
            }
        });
    picked
}
