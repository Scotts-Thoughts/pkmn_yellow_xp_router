//! The layout the pair and self comparisons share (Solodex
//! `ComparisonView.tsx` / `SelfComparisonView.tsx`): a header of
//! [matchups | identity | stat rows | identity | matchups] over two
//! side-by-side movepools split by a divider. The pair view puts the
//! evolution families above the tables, the self view the game chips.

use std::collections::HashSet;

use egui::{Pos2, ScrollArea, Sense, Stroke, Ui, Vec2};
use xpr_dex::EvolutionEntry;

use super::header::{
    evo_chips, evo_chips_height, game_chips, identity_size, paint_identity, Eff, PAIR_EFF_LEFT,
    PAIR_EFF_RIGHT, PAIR_IDENTITY,
};
use super::stats::{diff_height, paint_diff, DiffArgs};
use super::tables::{
    draw_side, level_diff_marks, measure, side_height, unique_marks, Column, Env, SectionDef,
    SideSpec, Table, SECTIONS,
};
use super::{dedup, CompareView, NO_DRAG};
use crate::palette;
use crate::views::movepool::{toggle_test_set, HeaderEvent, SortState};
use crate::DexCx;

const EFF_W: f32 = 144.0;
const GAP: f32 = 12.0;
const PAD_X: f32 = 12.0;
const PAD_Y: f32 = 12.0;
const CENTER_MIN: f32 = 280.0;
/// Room the page's list toggle takes at the pane's top left (22 px at 8 px).
const TOGGLE_CLEARANCE: f32 = 30.0;
/// Half the divider's slot: the table sits `mx-2` plus half the 1 px rule from the middle.
const HALF_DIVIDER: f32 = 8.5;

pub enum Kind<'a> {
    Pair,
    /// all games of the species, for the chips
    SelfCmp { games: &'a [String] },
}

pub struct Two<'a> {
    pub kind: Kind<'a>,
    pub left: &'a Column,
    pub right: &'a Column,
    pub left_name: &'a str,
    pub right_name: &'a str,
    pub left_game: &'a str,
    pub right_game: &'a str,
}

/// What a frame asked the page to do.
#[derive(Default)]
pub struct Out {
    /// a ranking popover picked this species: select it, leaving the comparison
    pub navigate: Option<String>,
    pub select_left: Option<String>,
    pub select_right: Option<String>,
    pub pick_left_game: Option<String>,
    pub pick_right_game: Option<String>,
}

/// Draw the whole view into the space `ui` has.
pub fn ui(v: &mut CompareView, ui: &mut Ui, cx: &mut DexCx, t: &Two) -> Out {
    let mut out = Out::default();
    // the header (at most 55vh, scrolling on its own)
    let max_head = (ui.ctx().content_rect().height() * 0.55)
        .min(ui.available_height() - 100.0)
        .max(120.0);
    let avail_w = ui.available_width();
    let mount = v.mount;
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    ScrollArea::both()
        .id_salt(("cmp_two_head", mount))
        .max_height(max_head)
        .auto_shrink([false, true])
        .scroll_source(NO_DRAG)
        .show(ui, |ui| {
            out.navigate = header(ui, cx, t, avail_w, mount);
        });
    // border-b border-gray-700
    let (line, _) = ui.allocate_exact_size(Vec2::new(avail_w, 1.0), Sense::hover());
    ui.painter().hline(
        line.x_range(),
        line.center().y,
        Stroke::new(1.0_f32, palette::GRAY_700),
    );
    body(v, ui, cx, t, &mut out);
    out
}

fn header(ui: &mut Ui, cx: &mut DexCx, t: &Two, avail_w: f32, mount: u64) -> Option<String> {
    let theme = cx.theme;
    let epoch = egui::Id::new((mount, t.left_name, t.right_name, t.left_game, t.right_game)).value();
    let self_mode = matches!(t.kind, Kind::SelfCmp { .. });
    let id_w = if self_mode { 192.0 } else { 160.0 };
    let (lp, rp) = (&t.left.pokemon, &t.right.pokemon);
    let (l_ab, r_ab) = (dedup(&lp.abilities), dedup(&rp.abilities));
    // the self view orders the types shared with the other game first
    let (l_other, r_other) = if self_mode {
        (
            Some(xpr_dex::defense_matchups(&rp.type_1, &rp.type_2, t.right_game)),
            Some(xpr_dex::defense_matchups(&lp.type_1, &lp.type_2, t.left_game)),
        )
    } else {
        (None, None)
    };
    let l_eff = Eff::new(&lp.type_1, &lp.type_2, t.left_game, &l_ab, l_other.as_ref());
    let r_eff = Eff::new(&rp.type_1, &rp.type_2, t.right_game, &r_ab, r_other.as_ref());
    let (l_eff_sz, r_eff_sz) = (
        l_eff.size(ui, theme, &PAIR_EFF_LEFT),
        r_eff.size(ui, theme, &PAIR_EFF_RIGHT),
    );
    let (l_eff_h, r_eff_h) = (l_eff_sz.y, r_eff_sz.y);
    // a column grows when an ability note makes its block wider (Solodex lets
    // it overflow)
    let (l_eff_w, r_eff_w) = (EFF_W.max(l_eff_sz.x), EFF_W.max(r_eff_sz.x));
    // ... and then clears the list toggle that floats over the pane's top left
    let pad_left = if l_eff_w > EFF_W { PAD_X + TOGGLE_CLEARANCE } else { PAD_X };
    let needed = pad_left + PAD_X + l_eff_w + r_eff_w + 2.0 * id_w + CENTER_MIN + 4.0 * GAP;
    let content_w = avail_w.max(needed);
    let center_w = content_w - pad_left - PAD_X - l_eff_w - r_eff_w - 2.0 * id_w - 4.0 * GAP;
    let l_id_h = identity_size(ui, theme, lp, &PAIR_IDENTITY, id_w).y;
    let r_id_h = identity_size(ui, theme, rp, &PAIR_IDENTITY, id_w).y;
    let args = DiffArgs {
        id: egui::Id::new(("cmp_two_stats", self_mode)),
        epoch,
        scope: if self_mode { "self" } else { "pair" },
        left: &lp.base_stats,
        right: &rp.base_stats,
        left_game: t.left_game,
        right_game: t.right_game,
        left_name: t.left_name,
        right_name: t.right_name,
        self_mode,
    };
    let stats_h = diff_height(ui, theme, &args, &cx.state.settings);
    let h = l_eff_h.max(l_id_h).max(stats_h).max(r_id_h).max(r_eff_h);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(content_w, 2.0 * PAD_Y + h), Sense::hover());
    let y0 = rect.min.y + PAD_Y;
    let top = |col_h: f32| y0 + (h - col_h) / 2.0;
    let mut x = rect.min.x + pad_left;
    l_eff.paint(ui, theme, &PAIR_EFF_LEFT, x + l_eff_w, top(l_eff_h), "left");
    x += l_eff_w + GAP;
    paint_identity(
        ui,
        cx,
        lp,
        t.left_game,
        &PAIR_IDENTITY,
        x + id_w / 2.0,
        top(l_id_h),
        id_w,
        "left",
    );
    x += id_w + GAP;
    let nav = paint_diff(ui, cx, &args, x, center_w, top(stats_h));
    x += center_w + GAP;
    paint_identity(
        ui,
        cx,
        rp,
        t.right_game,
        &PAIR_IDENTITY,
        x + id_w / 2.0,
        top(r_id_h),
        id_w,
        "right",
    );
    x += id_w + GAP;
    r_eff.paint(ui, theme, &PAIR_EFF_RIGHT, x, top(r_eff_h), "right");
    nav
}

fn body(v: &mut CompareView, ui: &mut Ui, cx: &mut DexCx, t: &Two, out: &mut Out) {
    let theme = cx.theme;
    let self_mode = matches!(t.kind, Kind::SelfCmp { .. });
    let show_diff = cx.state.settings.show_movepool_diff;
    let test: Vec<String> = cx.state.move_test_set.clone();
    let cross_l = v.cross_set(&cx.state.settings, t.left_game);
    let cross_r = v.cross_set(&cx.state.settings, t.right_game);
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
    let cols = measure(
        ui,
        theme,
        &test,
        SECTIONS.iter().enumerate().flat_map(|(si, def)| {
            [
                (def, &t.left.tables[si], sorts[si]),
                (def, &t.right.tables[si], sorts[si]),
            ]
        }),
    );
    let tw = cols.total();
    let bg = palette::page_bg(theme);
    let mut new_sorts = sorts;
    let mut toggled: Option<String> = None;
    let dash = if self_mode { "\u{2014}" } else { "" };
    let l_family = &t.left.pokemon.evolution_family;
    let r_family = &t.right.pokemon.evolution_family;
    ScrollArea::both()
        .id_salt(("cmp_two_body", v.mount))
        .auto_shrink([false, false])
        .scroll_source(NO_DRAG)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let origin = ui.cursor().min;
            let avail_w = ui.available_width();
            let content_w = avail_w.max(2.0 * (tw + HALF_DIVIDER));
            let mid = origin.x + content_w / 2.0;
            let left_x = mid - HALF_DIVIDER - tw;
            let right_x = mid + HALF_DIVIDER;
            let mut y = origin.y;

            // evolution families (pair) or game chips (self)
            match &t.kind {
                Kind::Pair => {
                    if l_family.len() > 1 || r_family.len() > 1 {
                        let lh = family_height(ui, theme, l_family, &t.left.species, tw);
                        let rh = family_height(ui, theme, r_family, &t.right.species, tw);
                        if l_family.len() > 1 {
                            out.select_left = evo_chips(
                                ui,
                                theme,
                                l_family,
                                &t.left.species,
                                left_x,
                                tw,
                                y + 8.0,
                                "left",
                            );
                        }
                        if r_family.len() > 1 {
                            out.select_right = evo_chips(
                                ui,
                                theme,
                                r_family,
                                &t.right.species,
                                right_x,
                                tw,
                                y + 8.0,
                                "right",
                            );
                        }
                        y += 16.0 + lh.max(rh);
                    }
                }
                Kind::SelfCmp { games } => {
                    let (lh, _) = game_chips(
                        ui,
                        theme,
                        games,
                        t.left_game,
                        t.right_game,
                        left_x,
                        tw,
                        y + 8.0,
                        "left",
                        false,
                    );
                    out.pick_left_game = game_chips(
                        ui,
                        theme,
                        games,
                        t.left_game,
                        t.right_game,
                        left_x,
                        tw,
                        y + 8.0,
                        "left",
                        true,
                    )
                    .1;
                    out.pick_right_game = game_chips(
                        ui,
                        theme,
                        games,
                        t.right_game,
                        t.left_game,
                        right_x,
                        tw,
                        y + 8.0,
                        "right",
                        true,
                    )
                    .1;
                    y += 16.0 + lh;
                }
            }

            // the tables, side by side
            let mut env = Env {
                theme,
                cols: &cols,
                test: &test,
                bg,
                actions: &mut *cx.actions,
                now,
                copied: &mut v.copied,
                toggled: None,
            };
            for (si, def) in SECTIONS.iter().enumerate() {
                let (lt, rt) = (&t.left.tables[si], &t.right.tables[si]);
                if lt.rows.is_empty() && rt.rows.is_empty() {
                    continue;
                }
                let marks = show_diff && def.diff;
                let (lu, ru) = if marks {
                    (
                        Some(unique_marks(lt, &[rt], self_mode)),
                        Some(unique_marks(rt, &[lt], self_mode)),
                    )
                } else {
                    (None, None)
                };
                let (ld, rd) = if marks && self_mode && def.key == "level" {
                    (
                        Some(level_diff_marks(lt, rt)),
                        Some(level_diff_marks(rt, lt)),
                    )
                } else {
                    (None, None)
                };
                let sort = sorts[si];
                let le = draw_side(
                    ui,
                    &mut env,
                    &side_spec("left", def, lt, t.left_game, sort, lu.as_deref(), ld.as_deref(), &cross_l, tw, dash),
                    Pos2::new(left_x, y),
                );
                let re = draw_side(
                    ui,
                    &mut env,
                    &side_spec("right", def, rt, t.right_game, sort, ru.as_deref(), rd.as_deref(), &cross_r, tw, dash),
                    Pos2::new(right_x, y),
                );
                match le.or(re) {
                    Some(HeaderEvent::Clicked(col)) => new_sorts[si] = sorts[si].clicked(col),
                    Some(HeaderEvent::Reset) => new_sorts[si] = Default::default(),
                    None => {}
                }
                y += side_height(lt.rows.len()).max(side_height(rt.rows.len()));
            }
            toggled = env.toggled.take();
            // the divider
            ui.painter().vline(
                mid,
                egui::Rangef::new(origin.y, y),
                Stroke::new(1.0_f32, palette::GRAY_700),
            );
            ui.allocate_space(Vec2::new(content_w, y - origin.y + 16.0));
        });
    v.sorts = new_sorts;
    if let Some(name) = toggled {
        toggle_test_set(&mut cx.state.move_test_set, &name);
    }
}

/// One table of one side of the two-column layout. The left side's empty
/// label sticks to its right edge, the right side's to its left.
#[allow(clippy::too_many_arguments)]
fn side_spec<'a>(
    side: &str,
    def: &'static SectionDef,
    table: &'a Table,
    game: &'a str,
    sort: SortState,
    unique: Option<&'a [bool]>,
    level_diff: Option<&'a [bool]>,
    cross: &'a HashSet<String>,
    head_w: f32,
    dash: &'a str,
) -> SideSpec<'a> {
    SideSpec {
        scope: format!("{}:{}", side, def.key),
        def,
        table,
        game,
        sort,
        unique,
        level_diff,
        cross,
        head_w,
        collapse_empty: true,
        align_right: side == "left",
        dash,
    }
}

/// The family's chip rows (nothing when it is a single species).
fn family_height(
    ui: &Ui,
    theme: &xpr_ui_kit::theme::Theme,
    family: &[EvolutionEntry],
    current: &str,
    w: f32,
) -> f32 {
    if family.len() > 1 {
        evo_chips_height(ui, theme, family, current, w)
    } else {
        0.0
    }
}
