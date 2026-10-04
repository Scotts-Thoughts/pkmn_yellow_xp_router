//! The stat rows of the comparison views: the pair and self views' mirrored
//! rows with their differences (Solodex `StatComparison`,
//! `SelfStatComparison`) and the triple view's per-column bars
//! (`TripleColumnStats`). Each row opens a ranking popover ([`super::rank`]).

use egui::{Align2, Color32, CornerRadius, Id, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use xpr_dex::{BaseStats, RankKind, StatKey};

use super::draw::{centered_heading, diff_color, diff_text, GREEN_400, RED_400};
use super::probe;
use super::rank::{self, Activation, Cfg, PopLook, PopSpec, Side};
use crate::palette;
use xpr_ui_kit::theme::Theme;

use crate::state::DexSettings;
use crate::views::pokedex::MenuCtx;
use crate::widgets;
use crate::DexCx;

/// Solodex's `MAX_STAT`: the bar's full scale.
const MAX_STAT: f32 = 255.0;
const PHYSICAL_BULK_COLOR: &str = "#e86412";
const SPECIAL_BULK_COLOR: &str = "#4a6adf";
const TOTAL_COLOR: &str = "#94a3b8";
const ROW_H: f32 = 20.0;
const GAP: f32 = 6.0;

// ---------------------------------------------------------------------------
// pair and self rows
// ---------------------------------------------------------------------------

/// What the mirrored rows are drawn for.
pub struct DiffArgs<'a> {
    /// stable id of the block (the popovers' state lives under it)
    pub id: Id,
    /// what the block is about (species, games, mount): see [`Cfg::epoch`]
    pub epoch: u64,
    pub scope: &'a str,
    pub left: &'a BaseStats,
    pub right: &'a BaseStats,
    pub left_game: &'a str,
    pub right_game: &'a str,
    pub left_name: &'a str,
    pub right_name: &'a str,
    /// the self comparison: one species, two games; popovers open on a click
    /// and rank in the left game
    pub self_mode: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Stat(StatKey),
    Total,
    Wbst,
    Ubst,
    PhysicalBulk,
    SpecialBulk,
}

impl Kind {
    fn key(self) -> String {
        match self {
            Kind::Stat(k) => k.key().to_string(),
            Kind::Total => "__total__".into(),
            Kind::Wbst => "__wbst__".into(),
            Kind::Ubst => "__ubst__".into(),
            Kind::PhysicalBulk => "__physical_bulk__".into(),
            Kind::SpecialBulk => "__special_bulk__".into(),
        }
    }

    fn rank_kind(self) -> RankKind {
        match self {
            Kind::Stat(k) => RankKind::Stat(k),
            Kind::Total => RankKind::Total,
            Kind::Wbst => RankKind::Wbst,
            Kind::Ubst => RankKind::Ubst,
            Kind::PhysicalBulk => RankKind::PhysicalBulk,
            Kind::SpecialBulk => RankKind::SpecialBulk,
        }
    }
}

struct DiffRow {
    kind: Kind,
    label: String,
    /// "Physical Bulk" for the popover title
    long_label: String,
    color: Color32,
    lv: i64,
    rv: i64,
    /// the value colour when the sides are equal
    equal: Color32,
    /// the bar fractions (stat rows only)
    bars: Option<(f32, f32)>,
    /// a thousands-separated value (bulk)
    thousands: bool,
    tip: Option<&'static str>,
    /// the `border-t` row
    border: bool,
}

fn gen1(game: &str) -> bool {
    xpr_dex::game_gen(game) == 1
}

fn total(s: &BaseStats, gen1: bool) -> i64 {
    (if gen1 {
        s.hp + s.attack + s.defense + s.special_attack + s.speed
    } else {
        s.sum()
    }) as i64
}

fn wbst(s: &BaseStats) -> i64 {
    (s.hp + s.attack + s.defense + s.speed + s.special_attack * 2) as i64
}

fn diff_rows(a: &DiffArgs, st: &DexSettings) -> Vec<DiffRow> {
    let (g1l, g1r) = (gen1(a.left_game), gen1(a.right_game));
    // the pair view's sides share a game; the self view uses the five-stat
    // layout only when both games are gen 1
    let config_gen1 = g1l && g1r;
    let (l, r) = (a.left, a.right);
    let mut rows = Vec::new();
    for key in palette::stat_keys(config_gen1) {
        let (lv, rv) = (l.get(*key), r.get(*key));
        rows.push(DiffRow {
            kind: Kind::Stat(*key),
            label: palette::stat_label(*key, config_gen1).to_string(),
            long_label: palette::stat_label(*key, config_gen1).to_string(),
            color: palette::stat_color(*key, config_gen1),
            lv: lv as i64,
            rv: rv as i64,
            equal: palette::stat_color(*key, config_gen1),
            bars: Some((
                (lv as f32 / MAX_STAT).clamp(0.0, 1.0),
                (rv as f32 / MAX_STAT).clamp(0.0, 1.0),
            )),
            thousands: false,
            tip: None,
            border: false,
        });
    }
    let plain = |kind, label: &str, color, lv, rv, tip| DiffRow {
        kind,
        label: label.to_string(),
        long_label: label.to_string(),
        color,
        lv,
        rv,
        equal: Color32::WHITE,
        bars: None,
        thousands: false,
        tip,
        border: false,
    };
    let mut t = plain(
        Kind::Total,
        "Total",
        palette::hex(TOTAL_COLOR),
        total(l, g1l),
        total(r, g1r),
        None,
    );
    t.border = true;
    rows.push(t);
    if config_gen1 && st.show_wbst {
        rows.push(plain(
            Kind::Wbst,
            "WBST",
            palette::WBST_COLOR,
            wbst(l),
            wbst(r),
            Some("Weighted Base Stat Total \u{2014} Special counted twice"),
        ));
    }
    if config_gen1 && st.show_ubst {
        let u = (
            xpr_dex::get_pokemon_ubst(a.left_name, a.left_game),
            xpr_dex::get_pokemon_ubst(a.right_name, a.right_game),
        );
        if let (Some(lu), Some(ru)) = u {
            rows.push(plain(
                Kind::Ubst,
                "UBST",
                palette::UBST_COLOR,
                lu,
                ru,
                Some("Useful Base Stat Total \u{2014} WBST minus an offensive stat the species can't use"),
            ));
        }
    }
    if st.show_bulk {
        let spec = |s: &BaseStats, g1: bool| {
            s.hp as i64 * if g1 { s.special_attack } else { s.special_defense } as i64
        };
        for (kind, label, long, color, lv, rv) in [
            (
                Kind::PhysicalBulk,
                "Phys Bulk",
                "Physical Bulk",
                PHYSICAL_BULK_COLOR,
                l.hp as i64 * l.defense as i64,
                r.hp as i64 * r.defense as i64,
            ),
            (
                Kind::SpecialBulk,
                "Spec Bulk",
                "Special Bulk",
                SPECIAL_BULK_COLOR,
                spec(l, g1l),
                spec(r, g1r),
            ),
        ] {
            let color = palette::hex(color);
            rows.push(DiffRow {
                kind,
                label: label.to_string(),
                long_label: long.to_string(),
                color,
                lv,
                rv,
                equal: color,
                bars: None,
                thousands: true,
                tip: None,
                border: false,
            });
        }
    }
    rows
}

/// The label `w-12` (48 px) is too narrow for "Phys Bulk" and the like: it
/// wraps at its space, onto two 16 px lines.
fn label_wraps(ui: &Ui, theme: &Theme, label: &str) -> bool {
    label.contains(' ') && widgets::text_w(ui, label, &palette::px_bold(theme, 12.0)) > 48.0
}

/// The height of a row's content. The bar rows are 20 px (their values are
/// `text-sm` lines); the rows without bars end in a block holding an inline
/// span, whose line box takes the page's 24 px line height, and a wrapped
/// label makes it two 16 px lines.
fn content_h(ui: &Ui, theme: &Theme, r: &DiffRow) -> f32 {
    if r.bars.is_some() {
        ROW_H
    } else if label_wraps(ui, theme, &r.label) {
        32.0
    } else {
        24.0
    }
}

/// Height of the block ("Base Stats" heading and the rows).
pub fn diff_height(ui: &Ui, theme: &Theme, a: &DiffArgs, st: &DexSettings) -> f32 {
    let rows = diff_rows(a, st);
    // the heading (16) and its `mb-2`
    let mut h = 16.0 + 8.0;
    for (i, r) in rows.iter().enumerate() {
        if i > 0 {
            h += 4.0;
        }
        h += content_h(ui, theme, r) + if r.border { 5.0 } else { 0.0 };
    }
    h
}

/// The row's pieces inside `rect` (Solodex's `flex items-center gap-1.5`:
/// a 36 px difference, a flexible value + bar, a 48 px label, the mirror).
struct Geo {
    diff_l: Rect,
    left: Rect,
    label: Rect,
    right: Rect,
    diff_r: Rect,
}

fn geo(rect: Rect) -> Geo {
    let side = ((rect.width() - 36.0 * 2.0 - 48.0 - GAP * 4.0) / 2.0).max(40.0);
    let (y0, y1) = (rect.min.y, rect.max.y);
    let mut x = rect.min.x;
    let mut next = |w: f32| {
        let r = Rect::from_min_max(Pos2::new(x, y0), Pos2::new(x + w, y1));
        x += w + GAP;
        r
    };
    let diff_l = next(36.0);
    let left = next(side);
    let label = next(48.0);
    let right = next(side);
    let diff_r = next(36.0);
    Geo {
        diff_l,
        left,
        label,
        right,
        diff_r,
    }
}

fn bar(ui: &Ui, track: Rect, frac: f32, color: Color32, from_right: bool) {
    ui.painter()
        .rect_filled(track, CornerRadius::same(2), palette::GRAY_700);
    let w = track.width() * frac;
    if w > 0.5 {
        let fill = if from_right {
            Rect::from_min_max(Pos2::new(track.max.x - w, track.min.y), track.max)
        } else {
            Rect::from_min_size(track.min, Vec2::new(w, track.height()))
        };
        widgets::paint_glossy(ui, fill, 2, color.gamma_multiply(0.85));
    }
}

/// Draw the heading and rows into `[x0, x0 + width]` from `top`. Returns
/// the species a ranking popover picked (select it).
pub fn paint_diff(
    ui: &mut Ui,
    cx: &mut DexCx,
    a: &DiffArgs,
    x0: f32,
    width: f32,
    top: f32,
) -> Option<String> {
    let theme = cx.theme;
    let rows = diff_rows(a, &cx.state.settings);
    let mut y = top;
    // the heading is a 16 px line with `mb-2`
    centered_heading(ui, theme, x0 + width / 2.0, y, "Base Stats");
    y += 16.0 + 8.0;
    let mut request: Option<(String, Rect)> = None;
    let mut rects: Vec<Rect> = Vec::new();
    let val = palette::px_bold(theme, 14.0);
    for (i, r) in rows.iter().enumerate() {
        if i > 0 {
            y += 4.0;
        }
        let h = content_h(ui, theme, r);
        let (row, content) = if r.border {
            let full = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(width, 5.0 + h));
            ui.painter().hline(
                full.x_range(),
                full.min.y + 0.5,
                Stroke::new(1.0_f32, palette::GRAY_700),
            );
            let content = Rect::from_min_size(Pos2::new(x0, y + 5.0), Vec2::new(width, h));
            (full, content)
        } else {
            let content = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(width, h));
            (content, content)
        };
        y += row.height();
        let key = r.kind.key();
        let mut resp = ui
            .interact(row, a.id.with(&key), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        if let Some(tip) = r.tip {
            resp = resp.on_hover_text(tip);
        }
        rects.push(row);
        probe::record("stat", a.scope, i, &r.label, row);
        if a.self_mode {
            if resp.clicked() {
                request = Some((key.clone(), row));
            }
        } else if resp.hovered() || resp.clicked() {
            request = Some((key.clone(), row));
        }
        // `rounded hover:bg-gray-800/50`
        if resp.hovered() {
            ui.painter().rect_filled(
                row,
                CornerRadius::same(4),
                palette::GRAY_800.gamma_multiply(0.5),
            );
        }
        let g = geo(content);
        let cy = content.center().y;
        let diff = r.lv - r.rv;
        // differences (in the colour of the side's advantage)
        ui.painter().text(
            Pos2::new(g.diff_l.max.x, cy),
            Align2::RIGHT_CENTER,
            diff_text(diff),
            val.clone(),
            diff_color(diff),
        );
        ui.painter().text(
            Pos2::new(g.diff_r.min.x, cy),
            Align2::LEFT_CENTER,
            diff_text(-diff),
            val.clone(),
            diff_color(-diff),
        );
        // label (two lines when it does not fit)
        let label_font = palette::px_bold(theme, 12.0);
        match r.label.split_once(' ').filter(|_| label_wraps(ui, theme, &r.label)) {
            Some((a, b)) => {
                for (line, dy) in [(a, -8.0), (b, 8.0)] {
                    ui.painter().text(
                        g.label.center() + Vec2::new(0.0, dy),
                        Align2::CENTER_CENTER,
                        line,
                        label_font.clone(),
                        palette::GRAY_500,
                    );
                }
            }
            None => {
                ui.painter().text(
                    g.label.center(),
                    Align2::CENTER_CENTER,
                    &r.label,
                    label_font,
                    palette::GRAY_500,
                );
            }
        }
        // values
        let fmt = |v: i64| {
            if r.thousands {
                xpr_ui_kit::widgets::fmt_thousands(v)
            } else {
                v.to_string()
            }
        };
        let pick = |d: i64| {
            if d > 0 {
                GREEN_400
            } else if d < 0 {
                RED_400
            } else {
                r.equal
            }
        };
        ui.painter().text(
            Pos2::new(g.left.max.x, cy),
            Align2::RIGHT_CENTER,
            fmt(r.lv),
            val.clone(),
            pick(diff),
        );
        ui.painter().text(
            Pos2::new(g.right.min.x, cy),
            Align2::LEFT_CENTER,
            fmt(r.rv),
            val.clone(),
            pick(-diff),
        );
        if let Some((lp, rp)) = r.bars {
            let lt = Rect::from_min_max(
                Pos2::new(g.left.min.x, cy - 6.0),
                Pos2::new(g.left.max.x - 28.0 - GAP, cy + 6.0),
            );
            let rt = Rect::from_min_max(
                Pos2::new(g.right.min.x + 28.0 + GAP, cy - 6.0),
                Pos2::new(g.right.max.x, cy + 6.0),
            );
            bar(ui, lt, lp, r.color, true);
            bar(ui, rt, rp, r.color, false);
        }
    }

    // popovers
    let (highlight, looks, menu): (Vec<String>, Vec<PopLook>, MenuCtx) = if a.self_mode {
        (
            vec![a.left_name.to_string()],
            vec![PopLook {
                side: Side::Auto,
                scroll_to: None,
                card: true,
            }],
            MenuCtx {
                selected: Some(a.left_name.to_string()),
                game: a.left_game.to_string(),
                comparing_with: None,
            },
        )
    } else {
        (
            vec![a.left_name.to_string(), a.right_name.to_string()],
            vec![
                PopLook {
                    side: Side::Left,
                    scroll_to: Some(a.left_name.to_string()),
                    card: true,
                },
                PopLook {
                    side: Side::Right,
                    scroll_to: Some(a.right_name.to_string()),
                    card: true,
                },
            ],
            MenuCtx {
                selected: Some(a.left_name.to_string()),
                game: a.left_game.to_string(),
                comparing_with: Some(a.right_name.to_string()),
            },
        )
    };
    let cfg = Cfg {
        id: a.id.with("pop"),
        epoch: a.epoch,
        activation: if a.self_mode {
            Activation::Click
        } else {
            Activation::Hover
        },
        highlight: &highlight,
        looks: &looks,
        menu: &menu,
        rows: &rects,
    };
    let game = a.left_game;
    let with_game = !a.self_mode;
    let make = |key: &str| {
        let r = rows
            .iter()
            .find(|r| r.kind.key() == key)
            .expect("the requested row exists");
        PopSpec::new(r.kind.rank_kind(), &r.long_label, r.color, game, with_game)
    };
    rank::drive(cx, ui, &cfg, request, &make)
}

// ---------------------------------------------------------------------------
// triple column
// ---------------------------------------------------------------------------

/// What one column of the triple view's stat bars needs.
pub struct ColumnArgs<'a> {
    pub id: Id,
    /// see [`Cfg::epoch`]
    pub epoch: u64,
    pub scope: &'a str,
    pub game: &'a str,
    /// the column's species and its stats
    pub name: &'a str,
    pub all_names: [&'a str; 3],
    pub all: [&'a BaseStats; 3],
    pub column: usize,
}

/// Height of a column's stat block (heading, bars, total).
pub fn column_height(game: &str) -> f32 {
    let n = palette::stat_keys(gen1(game)).len() as f32;
    // heading + mb-1, the bars at `space-y-0.5`, the total row
    16.0 + 4.0 + n * ROW_H + (n - 1.0) * 2.0 + 2.0 + 5.0 + ROW_H
}

/// Draw one triple column's stat bars into `[x0, x0 + width]` from `top`.
/// Returns the species a popover picked.
pub fn paint_column(
    ui: &mut Ui,
    cx: &mut DexCx,
    a: &ColumnArgs,
    x0: f32,
    width: f32,
    top: f32,
) -> Option<String> {
    let theme = cx.theme;
    let g1 = gen1(a.game);
    let stats = a.all[a.column];
    let mut y = top;
    // the heading is a 16 px line with `mb-1`
    centered_heading(ui, theme, x0 + width / 2.0, y, "Base Stats");
    y += 16.0 + 4.0;
    let mut request: Option<(String, Rect)> = None;
    let mut specs: Vec<(String, RankKind, String, Color32)> = Vec::new();
    let val = palette::px_bold(theme, 14.0);
    let label_font = palette::px_bold(theme, 12.0);
    let pick = |v: i64, vals: &[i64]| -> Option<Color32> {
        let max = *vals.iter().max().unwrap();
        let min = *vals.iter().min().unwrap();
        if v == max && max != min {
            Some(GREEN_400)
        } else if v == min && max != min {
            Some(RED_400)
        } else {
            None
        }
    };
    let keys = palette::stat_keys(g1);
    for (i, key) in keys.iter().enumerate() {
        if i > 0 {
            y += 2.0;
        }
        let row = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(width, ROW_H));
        y += ROW_H;
        let color = palette::stat_color(*key, g1);
        let label = palette::stat_label(*key, g1);
        let vals: Vec<i64> = a.all.iter().map(|s| s.get(*key) as i64).collect();
        let v = stats.get(*key) as i64;
        let id = a.id.with(key.key());
        let resp = ui
            .interact(row, id, Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        probe::record("stat", a.scope, i, label, row);
        if resp.hovered() || resp.clicked() {
            request = Some((key.key().to_string(), row));
        }
        if resp.hovered() {
            ui.painter().rect_filled(
                row,
                CornerRadius::same(4),
                palette::GRAY_800.gamma_multiply(0.5),
            );
        }
        specs.push((
            key.key().to_string(),
            RankKind::Stat(*key),
            label.to_string(),
            color,
        ));
        let cy = row.center().y;
        ui.painter().text(
            Pos2::new(row.min.x + 32.0, cy),
            Align2::RIGHT_CENTER,
            label,
            label_font.clone(),
            palette::GRAY_500,
        );
        ui.painter().text(
            Pos2::new(row.min.x + 32.0 + 4.0 + 28.0, cy),
            Align2::RIGHT_CENTER,
            v.to_string(),
            val.clone(),
            pick(v, &vals).unwrap_or(color),
        );
        let track = Rect::from_min_max(
            Pos2::new(row.min.x + 32.0 + 4.0 + 28.0 + 4.0, cy - 6.0),
            Pos2::new(row.max.x, cy + 6.0),
        );
        bar(ui, track, (v as f32 / MAX_STAT).clamp(0.0, 1.0), color, false);
    }
    // total
    y += 2.0;
    let full = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(width, 5.0 + ROW_H));
    ui.painter().hline(
        full.x_range(),
        full.min.y + 0.5,
        Stroke::new(1.0_f32, palette::GRAY_700),
    );
    let row = Rect::from_min_size(Pos2::new(x0, y + 5.0), Vec2::new(width, ROW_H));
    let totals: Vec<i64> = a.all.iter().map(|s| total(s, g1)).collect();
    let t = totals[a.column];
    let resp = ui
        .interact(row, a.id.with("__total__"), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    probe::record("stat", a.scope, keys.len(), "Total", row);
    if resp.hovered() || resp.clicked() {
        request = Some(("__total__".to_string(), row));
    }
    if resp.hovered() {
        ui.painter().rect_filled(
            row,
            CornerRadius::same(4),
            palette::GRAY_800.gamma_multiply(0.5),
        );
    }
    specs.push((
        "__total__".to_string(),
        RankKind::Total,
        "Total".to_string(),
        palette::hex(TOTAL_COLOR),
    ));
    let cy = row.center().y;
    ui.painter().text(
        Pos2::new(row.min.x + 32.0, cy),
        Align2::RIGHT_CENTER,
        "Total",
        label_font,
        palette::GRAY_500,
    );
    ui.painter().text(
        Pos2::new(row.min.x + 32.0 + 4.0 + 28.0, cy),
        Align2::RIGHT_CENTER,
        t.to_string(),
        val,
        pick(t, &totals).unwrap_or(Color32::WHITE),
    );

    // the ranking popover (hover; no expanded card)
    let highlight: Vec<String> = a.all_names.iter().map(|s| s.to_string()).collect();
    let looks = [PopLook {
        side: Side::Auto,
        scroll_to: None,
        card: false,
    }];
    let menu = MenuCtx {
        selected: Some(a.name.to_string()),
        game: a.game.to_string(),
        comparing_with: a
            .all_names
            .iter()
            .find(|n| **n != a.name)
            .map(|n| n.to_string()),
    };
    let cfg = Cfg {
        id: a.id.with("pop"),
        epoch: a.epoch,
        activation: Activation::Hover,
        highlight: &highlight,
        looks: &looks,
        menu: &menu,
        rows: &[],
    };
    let game = a.game;
    let make = |key: &str| {
        let (_, kind, label, color) = specs
            .iter()
            .find(|(k, ..)| k == key)
            .expect("the requested row exists");
        PopSpec::new(*kind, label, *color, game, true)
    };
    rank::drive(cx, ui, &cfg, request, &make)
}
