//! The movepool tables of the comparison views: building a species' five
//! tables (level up, TM / HM, tutor, egg, transfer), the diff marks, and
//! drawing one table of one side (section label with copy, sortable header,
//! rows). The rows are the movepool module's [`move_row`].

use std::collections::HashSet;
use std::sync::Arc;

use egui::{
    Align2, Color32, CornerRadius, Id, Pos2, Rect, Sense, Stroke, Ui, UiBuilder, Vec2,
};
use indexmap::IndexMap;
use xpr_dex::{canonical_move_key, compare_tmhm, tmhm_code, PokemonData};
use xpr_ui_kit::theme::Theme;

use super::draw::spaced_galley;
use super::probe;
use crate::palette::{self, hex};
use crate::views::movepool::paint::text_at;
use crate::views::movepool::{
    apply_remind_labels, build_simple_rows, move_info, move_row, single_level_rows,
    single_simple_rows, sort_order, table_header, Columns, GameTag, GenGameData, HeaderEvent,
    MoveInfo, MoveRowCx, RowData, RowStyle, SortState, TableMeasure, CELL_PAD, HEADER_H, ROW_H,
};
use crate::widgets::text_w;
use crate::DexAction;

/// Height of a section label (`pt-3 pb-1` around a 20 px line).
pub const SECTION_HEAD_H: f32 = 36.0;
/// The tint of a move the other side lacks (`#1e3a5f`).
const UNIQUE_BG: &str = "#1e3a5f";

/// One of the five tables.
pub struct SectionDef {
    pub key: &'static str,
    pub label: &'static str,
    /// the first header ("Lv", or empty)
    pub col1: &'static str,
    /// the first column of the copied spreadsheet
    pub tsv_label: &'static str,
    /// different moves are marked (level up and TM / HM only)
    pub diff: bool,
    /// banned / postgame / conditional cross-outs apply (TM / HM and tutor, as in the Movepool)
    pub cross: bool,
}

pub const SECTIONS: [SectionDef; 5] = [
    SectionDef {
        key: "level",
        label: "Level Up Learnset",
        col1: "Lv",
        tsv_label: "Lv",
        diff: true,
        cross: false,
    },
    SectionDef {
        key: "tmhm",
        label: "TM / HM",
        col1: "",
        tsv_label: "TM/HM",
        diff: true,
        cross: true,
    },
    SectionDef {
        key: "tutor",
        label: "Move Tutor",
        col1: "",
        tsv_label: "Tutor",
        diff: false,
        cross: true,
    },
    SectionDef {
        key: "egg",
        label: "Egg Moves",
        col1: "",
        tsv_label: "",
        diff: false,
        cross: false,
    },
    SectionDef {
        key: "transfer",
        label: "Transfer Moves",
        col1: "",
        tsv_label: "",
        diff: false,
        cross: false,
    },
];

// ---------------------------------------------------------------------------
// model
// ---------------------------------------------------------------------------

/// One table: its rows in the table's own order and the stats of each move.
pub struct Table {
    pub rows: Vec<RowData>,
    pub infos: Vec<Option<MoveInfo>>,
    pub canon: Vec<String>,
}

impl Table {
    fn new(rows: Vec<RowData>, game: &str) -> Table {
        let infos = rows.iter().map(|r| move_info(&r.move_name, game)).collect();
        let canon = rows
            .iter()
            .map(|r| canonical_move_key(&r.move_name))
            .collect();
        Table { rows, infos, canon }
    }

    pub fn order(&self, sort: SortState) -> Vec<usize> {
        sort_order(&self.rows, &self.infos, sort)
    }

    /// The copied spreadsheet: the table in its own order (not the sorted
    /// one). `dash` is what a move without stats shows (the self comparison
    /// writes dashes, the others nothing).
    fn tsv(&self, tsv_label: &str, dash: &str) -> String {
        let mut lines = vec![[tsv_label, "Move", "Type", "Category", "Power", "Accuracy", "PP"].join("\t")];
        for (row, info) in self.rows.iter().zip(&self.infos) {
            let num = |v: Option<i32>| v.map(|n| n.to_string()).unwrap_or_else(|| dash.to_string());
            lines.push(
                [
                    row.prefix.clone(),
                    row.move_name.clone(),
                    info.as_ref().map(|m| m.move_type.clone()).unwrap_or_else(|| dash.to_string()),
                    info.as_ref().map(|m| m.category.clone()).unwrap_or_else(|| dash.to_string()),
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

/// A species' five tables in `game` (or, for the pair and triple views, in
/// every game of `game`'s generation, merged).
pub struct Column {
    pub species: String,
    pub pokemon: Arc<PokemonData>,
    pub tables: Vec<Table>,
}

/// Level-up rows of several games merged like [`crate::views::movepool::build_level_up_rows`],
/// with the triple view's tie-break: moves at the same level are ordered by name.
fn level_rows_by_name(gen_data: &[GenGameData]) -> Vec<RowData> {
    let mut map: IndexMap<(i32, String), Vec<&str>> = IndexMap::new();
    for gd in gen_data {
        for (level, name) in &gd.pokemon.level_up_learnset {
            let games = map.entry((*level, name.clone())).or_default();
            if !games.contains(&gd.game.as_str()) {
                games.push(gd.game.as_str());
            }
        }
    }
    let mut rows: Vec<RowData> = map
        .into_iter()
        .map(|((level, name), games)| RowData {
            move_name: name,
            sort_key: crate::views::movepool::level_sort_key(level),
            prefix: crate::views::movepool::level_prefix(level),
            game_tags: if games.len() == gen_data.len() {
                Vec::new()
            } else {
                gen_data
                    .iter()
                    .filter(|gd| games.contains(&gd.game.as_str()))
                    .map(|gd| GameTag {
                        abbrev: gd.abbrev.clone(),
                        color: gd.color,
                    })
                    .collect()
            },
        })
        .collect();
    rows.sort_by(|a, b| {
        a.sort_key
            .cmp(&b.sort_key)
            .then_with(|| xpr_dex::text::locale_cmp(&a.move_name, &b.move_name))
    });
    apply_remind_labels(rows)
}

fn with_tm_codes(rows: Vec<RowData>, game: &str, gen_data: &[GenGameData], multi: bool) -> Vec<RowData> {
    let mut rows: Vec<RowData> = rows
        .into_iter()
        .map(|mut r| {
            let mut code = tmhm_code(&r.move_name, game);
            if code.is_none() && multi {
                for gd in gen_data {
                    code = tmhm_code(&r.move_name, &gd.game);
                    if code.is_some() {
                        break;
                    }
                }
            }
            r.prefix = code.unwrap_or_default();
            r
        })
        .collect();
    rows.sort_by(|a, b| compare_tmhm(&a.prefix, &b.prefix));
    rows
}

fn tm_list(p: &PokemonData) -> &[String] {
    &p.tm_hm_learnset
}

fn tutor_list(p: &PokemonData) -> &[String] {
    &p.tutor_learnset
}

fn egg_list(p: &PokemonData) -> &[String] {
    &p.egg_moves
}

fn transfer_list(p: &PokemonData) -> &[String] {
    &p.transfer_learnset
}

fn tutor_prefixed(rows: Vec<RowData>) -> Vec<RowData> {
    rows.into_iter()
        .map(|mut r| {
            r.prefix = "Tutor".to_string();
            r
        })
        .collect()
}

impl Column {
    /// The pair and triple views' column: the tables merge every game of
    /// `game`'s generation the species is in (a row only some games have
    /// carries game tags). `by_name`: the triple view's level-up tie-break.
    pub fn merged(species: &str, game: &str, by_name: bool) -> Option<Column> {
        let pokemon = xpr_dex::get_pokemon_data(species, game)?;
        let available = xpr_dex::get_games_for_pokemon(species);
        let gen_data: Vec<GenGameData> = match xpr_dex::games::gen_group_of(game) {
            Some(group) => group
                .games
                .iter()
                .filter(|g| available.iter().any(|a| a == **g))
                .filter_map(|g| {
                    xpr_dex::get_pokemon_data(species, g).map(|p| GenGameData {
                        game: g.to_string(),
                        abbrev: xpr_dex::game_abbrev(g).to_string(),
                        color: palette::game_color(g),
                        pokemon: p,
                    })
                })
                .collect(),
            None => Vec::new(),
        };
        let multi = gen_data.len() > 1;
        let level = if multi {
            if by_name {
                level_rows_by_name(&gen_data)
            } else {
                crate::views::movepool::build_level_up_rows(&gen_data)
            }
        } else {
            single_level_rows(&pokemon)
        };
        let simple = |pick: fn(&PokemonData) -> &[String]| -> Vec<RowData> {
            if multi {
                build_simple_rows(&gen_data, pick)
            } else {
                single_simple_rows(pick(&pokemon))
            }
        };
        let tm = with_tm_codes(simple(tm_list), game, &gen_data, multi);
        let tutor = tutor_prefixed(simple(tutor_list));
        let egg = simple(egg_list);
        let transfer = simple(transfer_list);
        Some(Column {
            species: species.to_string(),
            tables: [level, tm, tutor, egg, transfer]
                .into_iter()
                .map(|rows| Table::new(rows, game))
                .collect(),
            pokemon,
        })
    }

    /// The self view's column: one species in one game.
    pub fn single(species: &str, game: &str) -> Option<Column> {
        let pokemon = xpr_dex::get_pokemon_data(species, game)?;
        let level = single_level_rows(&pokemon);
        let tm = with_tm_codes(single_simple_rows(&pokemon.tm_hm_learnset), game, &[], false);
        let tutor = tutor_prefixed(single_simple_rows(&pokemon.tutor_learnset));
        let egg = single_simple_rows(&pokemon.egg_moves);
        let transfer = single_simple_rows(&pokemon.transfer_learnset);
        Some(Column {
            species: species.to_string(),
            tables: [level, tm, tutor, egg, transfer]
                .into_iter()
                .map(|rows| Table::new(rows, game))
                .collect(),
            pokemon,
        })
    }
}

// ---------------------------------------------------------------------------
// diff marks
// ---------------------------------------------------------------------------

/// `marks[i]`: the move of `table.rows[i]` is in none of `others` (compared
/// by exact name, or by `canonical_move_key` so the spellings of different
/// games match).
pub fn unique_marks(table: &Table, others: &[&Table], canonical: bool) -> Vec<bool> {
    let mut set: HashSet<&str> = HashSet::new();
    for o in others {
        for (i, r) in o.rows.iter().enumerate() {
            set.insert(if canonical { &o.canon[i] } else { &r.move_name });
        }
    }
    table
        .rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let key: &str = if canonical { &table.canon[i] } else { &r.move_name };
            !set.contains(key)
        })
        .collect()
}

/// Self comparison, level up: `marks[i]` is true when the other side has
/// the move of `table.rows[i]` but at another level.
pub fn level_diff_marks(table: &Table, other: &Table) -> Vec<bool> {
    let names: HashSet<&str> = other.canon.iter().map(|s| s.as_str()).collect();
    let exact: HashSet<(&str, i32)> = other
        .canon
        .iter()
        .zip(&other.rows)
        .map(|(c, r)| (c.as_str(), r.sort_key))
        .collect();
    table
        .canon
        .iter()
        .zip(&table.rows)
        .map(|(c, r)| names.contains(c.as_str()) && !exact.contains(&(c.as_str(), r.sort_key)))
        .collect()
}

// ---------------------------------------------------------------------------
// drawing
// ---------------------------------------------------------------------------

/// What the table drawing needs besides the table itself.
pub struct Env<'a> {
    pub theme: &'a Theme,
    pub cols: &'a Columns,
    pub test: &'a [String],
    pub bg: Color32,
    pub actions: &'a mut Vec<DexAction>,
    pub now: f64,
    /// the section whose spreadsheet was just copied, and until when the hint says so
    pub copied: &'a mut Option<(String, f64)>,
    /// the move whose row was right-clicked
    pub toggled: Option<String>,
}

/// One table of one side.
pub struct SideSpec<'a> {
    /// where it is, for ids and tests: "left:level"
    pub scope: String,
    pub def: &'static SectionDef,
    pub table: &'a Table,
    pub game: &'a str,
    pub sort: SortState,
    /// rows marked as different (the whole row tinted)
    pub unique: Option<&'a [bool]>,
    /// rows whose level differs (the first cell tinted)
    pub level_diff: Option<&'a [bool]>,
    /// banned / postgame / conditional moves of `game` (canonical keys)
    pub cross: &'a HashSet<String>,
    /// width of the section label's row (the table's, or the whole column's)
    pub head_w: f32,
    /// an empty table's label shrinks to its text (pair and self views), else
    /// the lines span `head_w`
    pub collapse_empty: bool,
    /// a collapsed label sticks to its right edge (the left side)
    pub align_right: bool,
    /// what a move without stats shows in the copied spreadsheet
    pub dash: &'a str,
}

/// Height of a side's section: the label and, unless empty, the table.
pub fn side_height(rows: usize) -> f32 {
    if rows == 0 {
        SECTION_HEAD_H
    } else {
        SECTION_HEAD_H + HEADER_H + rows as f32 * ROW_H
    }
}

/// Make `cols` as wide as the widest cell of any of `tables`.
pub fn measure<'a>(
    ui: &Ui,
    theme: &Theme,
    test: &[String],
    tables: impl IntoIterator<Item = (&'static SectionDef, &'a Table, SortState)>,
) -> Columns {
    let mut cols = Columns::new();
    let in_test = |n: &str| test.iter().any(|t| t == n);
    for (def, t, sort) in tables {
        if t.rows.is_empty() {
            continue;
        }
        cols.include(
            ui,
            theme,
            &TableMeasure {
                col1: def.col1,
                sort,
                rows: &t.rows,
                infos: &t.infos,
                in_test_set: &in_test,
            },
        );
    }
    cols
}

/// Draw one side's section with its top-left corner at `pos`. Returns the
/// header click, if any.
pub fn draw_side(ui: &mut Ui, env: &mut Env, s: &SideSpec, pos: Pos2) -> Option<HeaderEvent> {
    let theme = env.theme;
    let w = env.cols.total();
    let n = s.table.rows.len();
    let head = Rect::from_min_size(pos, Vec2::new(s.head_w, SECTION_HEAD_H));
    section_head(ui, env, s, head);
    if n == 0 {
        return None;
    }
    let top = Pos2::new(pos.x, pos.y + SECTION_HEAD_H);
    let table_h = HEADER_H + n as f32 * ROW_H;
    let id_base = Id::new(("cmp_table", &s.scope));
    let mut rui = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_size(top, Vec2::new(w, table_h)))
            .id_salt(id_base)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    rui.spacing_mut().item_spacing = Vec2::ZERO;
    // the header's slot; the header itself is drawn last, over the rows
    rui.allocate_exact_size(Vec2::new(w, HEADER_H), Sense::hover());
    let order = s.table.order(s.sort);
    {
        let cols = env.cols;
        let mut mx = MoveRowCx {
            theme,
            game: s.game,
            cols,
            actions: &mut *env.actions,
        };
        for (p, &i) in order.iter().enumerate() {
            let row = &s.table.rows[i];
            let cell0 = Rect::from_min_size(
                Pos2::new(rui.cursor().min.x, rui.cursor().min.y),
                Vec2::new(cols.width(0), ROW_H),
            );
            let row_rect = Rect::from_min_size(rui.cursor().min, Vec2::new(cols.total(), ROW_H));
            if s.unique.map(|u| u[i]).unwrap_or(false) && rui.is_rect_visible(row_rect) {
                rui.painter()
                    .rect_filled(row_rect, CornerRadius::ZERO, hex(UNIQUE_BG));
            }
            let style = RowStyle {
                in_test_set: env.test.contains(&row.move_name),
                crossed_out: s.def.cross && s.cross.contains(&s.table.canon[i]),
                highlight: false,
                can_toggle: true,
            };
            let r = move_row(
                &mut rui,
                &mut mx,
                id_base.with(p),
                row,
                s.table.infos[i].as_ref(),
                style,
            );
            if s.level_diff.map(|d| d[i]).unwrap_or(false) && rui.is_rect_visible(r.rect) {
                // the level cell keeps its tint under the row's hover colour
                rui.painter()
                    .rect_filled(cell0, CornerRadius::ZERO, hex(UNIQUE_BG));
                text_at(
                    &rui,
                    Pos2::new(cell0.min.x + CELL_PAD, cell0.center().y),
                    Align2::LEFT_CENTER,
                    &row.prefix,
                    palette::px(theme, 14.0),
                    palette::GRAY_500,
                    false,
                );
            }
            probe::record("row", &s.scope, p, &row.move_name, r.rect);
            if r.toggle_test_set {
                env.toggled = Some(row.move_name.clone());
            }
        }
    }
    // the header sticks to the top of the scroll area while the table is in view
    let view_top = ui.clip_rect().min.y;
    let y = view_top.max(top.y).min(top.y + table_h - HEADER_H);
    let (hrect, event) = table_header(
        &mut rui,
        theme,
        env.cols,
        Pos2::new(top.x, y),
        id_base.with("header"),
        s.def.col1,
        s.sort,
        env.bg,
    );
    for (i, col) in crate::views::movepool::SortColumn::ALL.iter().enumerate() {
        let label = if i == 0 { s.def.col1 } else { col.label() };
        let cell = Rect::from_min_size(
            Pos2::new(hrect.min.x + env.cols.x(i), hrect.min.y),
            Vec2::new(env.cols.width(i), HEADER_H),
        );
        probe::record("th", &s.scope, i, label, cell);
    }
    event
}

/// The label above a table: `-- LEVEL UP LEARNSET (12) Copy --` (Solodex
/// `CopyableSectionHeader`); a click copies the table as a spreadsheet. An
/// empty table shows a dimmed label and `(0)`, its lines collapsed.
fn section_head(ui: &mut Ui, env: &mut Env, s: &SideSpec, head: Rect) {
    let theme = env.theme;
    let n = s.table.rows.len();
    let label = s.def.label.to_uppercase();
    let bold = palette::px_bold(theme, 14.0);
    let body = palette::px(theme, 14.0);
    let hint_font = palette::px(theme, 12.0);
    let copied = env
        .copied
        .as_ref()
        .map(|(k, until)| *k == s.scope && env.now < *until)
        .unwrap_or(false);
    let hint = if copied { "Copied" } else { "Copy" };
    let count = format!("({})", n);
    let lw = spaced_galley(ui, &label, bold.clone(), Color32::WHITE, 1.4, f32::INFINITY)
        .size()
        .x;
    let cw = text_w(ui, &count, &body);
    let hw = text_w(ui, hint, &hint_font);
    let bw = lw + 8.0 + cw + if n > 0 { 8.0 + hw } else { 0.0 };
    let cy = head.min.y + 12.0 + 10.0;
    let full = n > 0 || !s.collapse_empty;
    let (bx, line_w) = if full {
        let line_w = ((head.width() - 8.0) - 16.0 - bw).max(0.0) / 2.0;
        (head.min.x + 4.0 + line_w + 8.0, line_w)
    } else if s.align_right {
        (head.max.x - 4.0 - bw - 8.0, 0.0)
    } else {
        (head.min.x + 4.0 + 8.0, 0.0)
    };
    let btn = Rect::from_min_size(Pos2::new(bx, cy - 10.0), Vec2::new(bw, 20.0));
    let resp = if n > 0 {
        let r = ui
            .interact(btn, Id::new(("cmp_head", &s.scope)), Sense::click())
            .on_hover_text("Click to copy as spreadsheet")
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        Some(r)
    } else {
        None
    };
    let hot = resp.as_ref().map(|r| r.hovered()).unwrap_or(false);
    let (lc, cc, hc) = if n == 0 {
        (palette::GRAY_600, palette::GRAY_700, palette::GRAY_600)
    } else if hot {
        (palette::GRAY_300, palette::GRAY_600, palette::GRAY_400)
    } else {
        (palette::GRAY_400, palette::GRAY_600, palette::GRAY_600)
    };
    let g = spaced_galley(ui, &label, bold, lc, 1.4, f32::INFINITY);
    ui.painter()
        .galley(Pos2::new(bx, cy - g.size().y / 2.0), g, lc);
    ui.painter().text(
        Pos2::new(bx + lw + 8.0, cy),
        Align2::LEFT_CENTER,
        &count,
        body,
        cc,
    );
    if n > 0 {
        ui.painter().text(
            Pos2::new(bx + lw + 8.0 + cw + 8.0, cy),
            Align2::LEFT_CENTER,
            hint,
            hint_font,
            hc,
        );
    }
    if full {
        let stroke = Stroke::new(1.0_f32, palette::GRAY_700);
        ui.painter().hline(
            egui::Rangef::new(head.min.x + 4.0, head.min.x + 4.0 + line_w),
            cy,
            stroke,
        );
        ui.painter().hline(
            egui::Rangef::new(bx + bw + 8.0, bx + bw + 8.0 + line_w),
            cy,
            stroke,
        );
    }
    probe::record("label", &s.scope, 0, s.def.label, btn);
    if let Some(r) = resp {
        if r.clicked() {
            ui.ctx().copy_text(s.table.tsv(s.def.tsv_label, s.dash));
            *env.copied = Some((s.scope.clone(), env.now + 1.5));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(t: &Table) -> Vec<&str> {
        t.rows.iter().map(|r| r.move_name.as_str()).collect()
    }

    #[test]
    fn merged_tables_tag_moves_only_some_games_have() {
        // Emerald's generation: Ruby / Sapphire, Emerald, FireRed / LeafGreen
        let c = Column::merged("Charmander", "Emerald", false).unwrap();
        let level = &c.tables[0];
        assert_eq!(level.rows[0].move_name, "Scratch");
        // Metal Claw is FireRed / LeafGreen's alone
        let metal = level.rows.iter().find(|r| r.move_name == "Metal Claw").unwrap();
        assert_eq!(metal.game_tags.iter().map(|t| t.abbrev.as_str()).collect::<Vec<_>>(), vec!["FRLG"]);
        // Scratch is in every game: no tags
        assert!(level.rows[0].game_tags.is_empty());
        // TM codes come from the viewed game; tutor rows say "Tutor"; the egg table has no prefix
        let tm = &c.tables[1];
        assert!(tm.rows.iter().any(|r| r.prefix == "TM02" && r.move_name == "Dragon Claw"));
        assert!(c.tables[2].rows.iter().all(|r| r.prefix == "Tutor"));
        assert!(c.tables[3].rows.iter().all(|r| r.prefix.is_empty()));
        // Gen 1 has no tutors or egg moves
        let rb = Column::merged("Charmander", "Red and Blue", false).unwrap();
        assert!(rb.tables[2].rows.is_empty() && rb.tables[3].rows.is_empty());
    }

    #[test]
    fn the_triple_view_orders_moves_of_a_level_by_name() {
        let plain = Column::merged("Charmander", "Red and Blue", false).unwrap();
        let by_name = Column::merged("Charmander", "Red and Blue", true).unwrap();
        // red / blue and yellow share the table, so both start with the level-1 moves
        assert_eq!(&names(&plain.tables[0])[..2], &["Scratch", "Growl"]);
        assert_eq!(&names(&by_name.tables[0])[..2], &["Growl", "Scratch"]);
        // only the order within a level differs
        let mut a = names(&plain.tables[0]);
        let mut b = names(&by_name.tables[0]);
        a.sort();
        b.sort();
        assert_eq!(a, b);
    }

    #[test]
    fn unique_marks_compare_names_exactly_or_by_canonical_key() {
        let rows = |v: &[&str]| v.iter().map(|m| RowData::simple(m, "")).collect::<Vec<_>>();
        let a = Table::new(rows(&["Surf", "ThunderShock", "Dig"]), "Emerald");
        let b = Table::new(rows(&["Thunder Shock", "Dig", "Fly"]), "Emerald");
        let c = Table::new(rows(&["Surf"]), "Emerald");
        assert_eq!(unique_marks(&a, &[&b], false), vec![true, true, false]);
        // the other games' spelling of the same move is the same move
        assert_eq!(unique_marks(&a, &[&b], true), vec![true, false, false]);
        // the triple view: a move is marked when none of the others has it
        assert_eq!(unique_marks(&a, &[&b, &c], false), vec![false, true, false]);
        assert_eq!(unique_marks(&b, &[&a, &c], true), vec![false, false, true]);
    }

    #[test]
    fn level_marks_flag_a_move_the_other_side_learns_at_another_level() {
        let level = |v: &[(i32, &str)]| {
            Table::new(
                v.iter()
                    .map(|(l, m)| RowData {
                        move_name: m.to_string(),
                        sort_key: crate::views::movepool::level_sort_key(*l),
                        prefix: crate::views::movepool::level_prefix(*l),
                        game_tags: Vec::new(),
                    })
                    .collect(),
                "Emerald",
            )
        };
        let a = level(&[(1, "ThunderShock"), (8, "Thunder Wave"), (11, "Quick Attack"), (20, "Slam")]);
        let b = level(&[(1, "Thunder Shock"), (7, "Thunder Wave"), (11, "Quick Attack"), (25, "Slam"), (30, "Fly")]);
        // same move, same level: nothing; same move, other level: marked; only one side: not a level mark
        assert_eq!(level_diff_marks(&a, &b), vec![false, true, false, true]);
        assert_eq!(level_diff_marks(&b, &a), vec![false, true, false, true, false]);
        // a move learned twice only matches at the level it is learned at
        let twice = level(&[(5, "Surf"), (9, "Surf")]);
        let once = level(&[(5, "Surf")]);
        assert_eq!(level_diff_marks(&twice, &once), vec![false, true]);
    }

    #[test]
    fn the_copied_spreadsheet_is_the_table_in_its_own_order() {
        let c = Column::single("Pikachu", "Emerald").unwrap();
        let tsv = c.tables[0].tsv("Lv", "\u{2014}");
        let mut lines = tsv.lines();
        assert_eq!(lines.next(), Some("Lv\tMove\tType\tCategory\tPower\tAccuracy\tPP"));
        assert_eq!(lines.next(), Some("1\tThunderShock\tElectric\tSpecial\t40\t100\t30"));
        // the pair and triple views write nothing for a missing number, the self view a dash
        assert!(tsv.contains("1\tGrowl\tNormal\tStatus\t\u{2014}\t100\t40"));
        assert!(c.tables[0].tsv("Lv", "").contains("1\tGrowl\tNormal\tStatus\t\t100\t40"));
    }
}
