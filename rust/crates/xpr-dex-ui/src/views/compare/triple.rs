//! Three species in one game (Solodex `TripleComparisonView.tsx`): three
//! columns, each with the identity and matchups side by side, its stat bars
//! (best green, worst red), its evolution family and its movepool tables.

use egui::{Pos2, ScrollArea, Stroke, Ui, Vec2};

use super::header::{
    evo_chips, evo_chips_height, identity_size, paint_identity, Eff, TRIPLE_EFF, TRIPLE_IDENTITY,
};
use super::stats::{column_height, paint_column, ColumnArgs};
use super::tables::{
    draw_side, measure, side_height, unique_marks, Column, Env, SideSpec, Table, SECTIONS,
};
use super::{dedup, unavailable, CompareView, Mode, NO_DRAG};
use crate::palette;
use crate::views::movepool::{toggle_test_set, HeaderEvent};
use crate::DexCx;

const PAD_X: f32 = 16.0;
/// A column's rule: `mx-2` on both sides of a 1 px divider.
const DIVIDER: f32 = 17.0;
/// Wide enough that a name never wraps.
const NO_WRAP: f32 = 4000.0;

/// The three species' tables for one game.
pub struct Model {
    key: ([String; 3], String),
    cols: [Option<Column>; 3],
}

pub fn ui(v: &mut CompareView, ui: &mut Ui, cx: &mut DexCx) {
    v.begin(Mode::Triple, ui.ctx());
    let (Some(a), Some(b), Some(c)) = (
        cx.state.selected().map(str::to_string),
        cx.state.settings.comparing_with.clone(),
        cx.state.settings.comparing_third.clone(),
    ) else {
        return;
    };
    let names = [a, b, c];
    let game = cx.state.game().to_string();
    let key = (names.clone(), game.clone());
    if v.triple.as_ref().map(|m| m.key != key).unwrap_or(true) {
        v.triple = Some(Model {
            cols: [
                Column::merged(&names[0], &game, true),
                Column::merged(&names[1], &game, true),
                Column::merged(&names[2], &game, true),
            ],
            key,
        });
    }
    let model = v.triple.take().expect("model");
    let (Some(c0), Some(c1), Some(c2)) = (&model.cols[0], &model.cols[1], &model.cols[2]) else {
        if unavailable(
            ui,
            cx,
            "One or more Pokemon not available in this game.",
            "Exit Comparison",
            None,
        ) {
            cx.state.exit_compare();
        }
        v.triple = Some(model);
        return;
    };
    let cols = [c0, c1, c2];
    let mut picked: [Option<String>; 3] = [None, None, None];
    let mut navigate: Option<String> = None;
    let theme = cx.theme;
    let show_diff = cx.state.settings.show_movepool_diff;
    let test: Vec<String> = cx.state.move_test_set.clone();
    let cross = v.cross_set(&cx.state.settings, &game);
    let sorts = v.sorts;
    let now = ui.input(|i| i.time);
    if let Some((_, until)) = &v.copied {
        if now >= *until {
            v.copied = None;
        } else {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
    let widths = measure(
        ui,
        theme,
        &test,
        SECTIONS.iter().enumerate().flat_map(|(si, def)| {
            cols.iter()
                .map(move |c| (def, &c.tables[si], sorts[si]))
                .collect::<Vec<_>>()
        }),
    );
    let tw = widths.total();
    let epoch = egui::Id::new((v.mount, &names, &game)).value();
    let bg = palette::page_bg(theme);
    let mut new_sorts = sorts;
    let mut toggled: Option<String> = None;
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    ScrollArea::both()
        .id_salt(("cmp_triple_body", v.mount))
        .auto_shrink([false, false])
        .scroll_source(NO_DRAG)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let origin = ui.cursor().min;
            let avail_w = ui.available_width();
            // the margins (`px-4`, `mx-2` around each divider) give way before a
            // table does: a column is never narrower than its table
            let min_w = 3.0 * tw + 2.0;
            let full_w = 3.0 * tw + 2.0 * DIVIDER + 2.0 * PAD_X;
            let content_w = avail_w.max(min_w);
            let k = ((content_w - min_w) / (full_w - min_w)).clamp(0.0, 1.0);
            let pad = PAD_X * k;
            let divider = 1.0 + (DIVIDER - 1.0) * k;
            let col_w = (content_w - 2.0 * pad - 2.0 * divider) / 3.0;
            let col_x = |i: usize| origin.x + pad + i as f32 * (col_w + divider);

            // ---- the graphic block: identity + matchups, then the stat bars ----
            struct Top {
                eff: Eff,
                id_size: Vec2,
                eff_size: Vec2,
            }
            let tops: Vec<Top> = cols
                .iter()
                .map(|c| {
                    let p = &c.pokemon;
                    let eff = Eff::new(&p.type_1, &p.type_2, &game, &dedup(&p.abilities), None);
                    Top {
                        id_size: identity_size(ui, theme, p, &TRIPLE_IDENTITY, NO_WRAP),
                        eff_size: eff.size(ui, theme, &TRIPLE_EFF),
                        eff,
                    }
                })
                .collect();
            let stats_h = column_height(&game);
            let top_h = |t: &Top| 12.0 + t.id_size.y.max(t.eff_size.y) + 12.0;
            // the stats section: border-t, pt-2, the bars, pb-2
            let graphic_h = tops
                .iter()
                .map(|t| top_h(t) + 1.0 + 8.0 + stats_h + 8.0)
                .fold(0.0_f32, f32::max);
            let y0 = origin.y;
            let all_stats = [
                &cols[0].pokemon.base_stats,
                &cols[1].pokemon.base_stats,
                &cols[2].pokemon.base_stats,
            ];
            let scopes = ["a", "b", "c"];
            for (i, t) in tops.iter().enumerate() {
                let group_h = t.id_size.y.max(t.eff_size.y);
                let group_w = t.id_size.x + 8.0 + t.eff_size.x;
                let x = col_x(i) + (col_w - group_w) / 2.0;
                paint_identity(
                    ui,
                    cx,
                    &cols[i].pokemon,
                    &game,
                    &TRIPLE_IDENTITY,
                    x + t.id_size.x / 2.0,
                    y0 + 12.0 + (group_h - t.id_size.y) / 2.0,
                    NO_WRAP,
                    scopes[i],
                );
                t.eff.paint(
                    ui,
                    theme,
                    &TRIPLE_EFF,
                    x + t.id_size.x + 8.0,
                    y0 + 12.0 + (group_h - t.eff_size.y) / 2.0,
                    scopes[i],
                );
                let rule_y = y0 + top_h(t);
                ui.painter().hline(
                    egui::Rangef::new(col_x(i), col_x(i) + col_w),
                    rule_y + 0.5,
                    Stroke::new(1.0_f32, palette::GRAY_700),
                );
                let args = ColumnArgs {
                    id: egui::Id::new(("cmp_triple_stats", i)),
                    epoch,
                    scope: scopes[i],
                    game: &game,
                    name: &names[i],
                    all_names: [&names[0], &names[1], &names[2]],
                    all: all_stats,
                    column: i,
                };
                if let Some(n) = paint_column(ui, cx, &args, col_x(i), col_w, rule_y + 1.0 + 8.0) {
                    navigate = Some(n);
                }
            }

            // ---- evolution families and movepools ----
            let y1 = y0 + graphic_h;
            let mut env = Env {
                theme,
                cols: &widths,
                test: &test,
                bg,
                actions: &mut *cx.actions,
                now,
                copied: &mut v.copied,
                toggled: None,
            };
            let mut bottom = y1;
            for i in 0..3 {
                let mut y = y1;
                ui.painter().hline(
                    egui::Rangef::new(col_x(i), col_x(i) + col_w),
                    y + 0.5,
                    Stroke::new(1.0_f32, palette::GRAY_700),
                );
                y += 1.0;
                let family = &cols[i].pokemon.evolution_family;
                if family.len() > 1 {
                    picked[i] = evo_chips(
                        ui,
                        theme,
                        family,
                        &cols[i].species,
                        col_x(i),
                        col_w,
                        y + 8.0,
                        scopes[i],
                    );
                    y += 8.0 + evo_chips_height(ui, theme, family, &cols[i].species, col_w) + 8.0;
                }
                for (si, def) in SECTIONS.iter().enumerate() {
                    let table = &cols[i].tables[si];
                    // a move is marked when neither other Pokémon learns it
                    let others: Vec<&Table> = (0..3)
                        .filter(|j| *j != i)
                        .map(|j| &cols[j].tables[si])
                        .collect();
                    let unique = (show_diff && def.diff).then(|| unique_marks(table, &others, false));
                    let spec = SideSpec {
                        scope: format!("{}:{}", scopes[i], def.key),
                        def,
                        table,
                        game: &game,
                        sort: sorts[si],
                        unique: unique.as_deref(),
                        level_diff: None,
                        cross: &cross,
                        head_w: col_w,
                        collapse_empty: false,
                        align_right: false,
                        dash: "",
                    };
                    match draw_side(ui, &mut env, &spec, Pos2::new(col_x(i), y)) {
                        Some(HeaderEvent::Clicked(col)) => new_sorts[si] = sorts[si].clicked(col),
                        Some(HeaderEvent::Reset) => new_sorts[si] = Default::default(),
                        None => {}
                    }
                    y += side_height(table.rows.len());
                }
                bottom = bottom.max(y);
            }
            toggled = env.toggled.take();
            // the column dividers
            for i in 0..2 {
                let x = col_x(i) + col_w + divider / 2.0;
                ui.painter().vline(
                    x,
                    egui::Rangef::new(y0, bottom),
                    Stroke::new(1.0_f32, palette::GRAY_700),
                );
            }
            ui.allocate_space(Vec2::new(content_w, bottom - origin.y + 16.0));
        });
    v.sorts = new_sorts;
    if let Some(name) = toggled {
        toggle_test_set(&mut cx.state.move_test_set, &name);
    }
    // Solodex's `onSelect1` / `onSelect2` / `onSelect3` / `onNavigate`
    if let Some(sp) = picked[0].take() {
        cx.state.set_selected_keep_compare(&sp);
    }
    if let Some(sp) = picked[1].take() {
        cx.state.compare_with(&sp);
    }
    if let Some(sp) = picked[2].take() {
        cx.state.triple_compare(&sp);
    }
    if let Some(name) = navigate {
        cx.state.select_species(&name);
    }
    v.triple = Some(model);
}
