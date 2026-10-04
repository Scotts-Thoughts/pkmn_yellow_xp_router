//! The base stat bars with their Total / WBST / UBST / bulk rows (Solodex
//! `BaseStats.tsx`). Hovering a row opens the ranking popover
//! ([`super::ranking`]); the popover's rows navigate and have the species
//! context menu.
//!
//! [`base_stats`] makes no assumption about the single-detail layout: it
//! draws into whatever `ui` it gets (the width of `ui` is the bars' width)
//! and keeps its popover state in egui memory under the caller's `id`, so
//! the comparison views can call it once per species column.

use std::collections::HashSet;
use std::time::Duration;

use egui::{Align2, Color32, CornerRadius, Id, Rect, Sense, Stroke, Ui, Vec2};
use xpr_dex::{BaseStats, RankKind, StatKey};
use xpr_ui_kit::theme::Theme;

use super::context_menu::MenuCtx;
use super::ranking::{self, RankingSpec};
use crate::palette;
use crate::widgets;
use crate::DexCx;

/// Solodex's `MAX_STAT`: the bar's full scale.
pub const MAX_STAT: f32 = 255.0;
const PHYSICAL_BULK_COLOR: &str = "#e86412";
const SPECIAL_BULK_COLOR: &str = "#4a6adf";
const TOTAL_COLOR: &str = "#94a3b8";
const ROW_H: f32 = 20.0;
/// How long the popover lingers after the pointer left (Solodex's 200 ms).
const LINGER: f64 = 0.2;

/// What the stat rows are drawn for.
pub struct BaseStatsArgs<'a> {
    /// stable id of this call site (the popover's state lives under it)
    pub id: Id,
    pub stats: &'a BaseStats,
    pub game: &'a str,
    /// the species (highlighted in the rankings; UBST is looked up for it)
    pub pokemon: &'a str,
    /// restrict the rankings to the species list's current filter
    /// (`cx.state.filtered_names`)
    pub use_filtered: bool,
    /// the page's selection, for the popover rows' context menu
    pub menu: MenuCtx,
}

#[derive(Clone)]
struct Open {
    key: String,
    spec: RankingSpec,
    anchor: Rect,
    leave_at: Option<f64>,
}

#[derive(Clone, Default)]
struct Hover {
    open: Option<Open>,
    session: u64,
    card: bool,
}

/// Which ranking a row opens.
#[derive(Clone, Copy)]
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

/// The ranking of `kind` as a popover spec (`{label} Ranking \u{2014} {game}`).
fn spec_for(
    kind: Kind,
    label: &str,
    color: Color32,
    a: &BaseStatsArgs,
    filter: Option<&HashSet<String>>,
) -> RankingSpec {
    RankingSpec {
        title: format!("{} Ranking \u{2014} {}", label, a.game),
        color,
        entries: xpr_dex::get_ranking(kind.rank_kind(), a.game, filter),
        current: a.pokemon.to_string(),
        game: a.game.to_string(),
    }
}

/// Draw the stat rows and their ranking popover. Returns the species the
/// user picked in a ranking (select it), if any.
pub fn base_stats(ui: &mut Ui, cx: &mut DexCx, a: &BaseStatsArgs) -> Option<String> {
    let theme = cx.theme;
    let ctx = ui.ctx().clone();
    let gen1 = xpr_dex::game_gen(a.game) == 1;
    let s = a.stats;
    let total = if gen1 {
        (s.hp + s.attack + s.defense + s.special_attack + s.speed) as i64
    } else {
        s.sum() as i64
    };
    let wbst = (s.hp + s.attack + s.defense + s.speed + s.special_attack * 2) as i64;
    let physical_bulk = s.hp as i64 * s.defense as i64;
    let special_bulk = s.hp as i64
        * if gen1 {
            s.special_attack
        } else {
            s.special_defense
        } as i64;
    let (show_bulk, show_wbst, show_ubst) = (
        cx.state.settings.show_bulk,
        cx.state.settings.show_wbst,
        cx.state.settings.show_ubst,
    );

    // UBST walks the learnsets: only when its row is shown
    let ubst = if gen1 && show_ubst {
        xpr_dex::get_pokemon_ubst(a.pokemon, a.game)
    } else {
        None
    };

    let mut hover: Hover = ctx.data(|d| d.get_temp(a.id)).unwrap_or_default();
    let open_key = hover.open.as_ref().map(|o| o.key.clone());
    let mut hovered: Option<(Kind, String, Color32, Rect)> = None;
    let w = ui.available_width();
    ui.spacing_mut().item_spacing = Vec2::ZERO;

    // the six (five) stat bars
    for (n, key) in palette::stat_keys(gen1).iter().enumerate() {
        if n > 0 {
            ui.add_space(4.0);
        }
        let color = palette::stat_color(*key, gen1);
        let label = palette::stat_label(*key, gen1);
        let value = s.get(*key);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, ROW_H), Sense::hover());
        let resp = ui
            .interact(rect, a.id.with(key.key()), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let is_open = open_key.as_deref() == Some(key.key());
        paint_stat_row(
            ui,
            theme,
            rect,
            label,
            &value.to_string(),
            color,
            Some(((value as f32 / MAX_STAT).clamp(0.0, 1.0), is_open)),
        );
        if resp.hovered() || resp.clicked() {
            hovered = Some((Kind::Stat(*key), label.to_string(), color, rect));
        }
    }

    // Total
    ui.add_space(4.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 5.0 + ROW_H), Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.min.y + 0.5,
        Stroke::new(1.0_f32, palette::GRAY_700),
    );
    let row = Rect::from_min_size(rect.min + Vec2::new(0.0, 5.0), Vec2::new(w, ROW_H));
    let resp = ui
        .interact(row, a.id.with("total"), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let is_open = open_key.as_deref() == Some("__total__");
    paint_stat_row(
        ui,
        theme,
        row,
        "Total",
        &total.to_string(),
        if is_open {
            palette::hex(TOTAL_COLOR)
        } else {
            Color32::WHITE
        },
        None,
    );
    if resp.hovered() || resp.clicked() {
        hovered = Some((
            Kind::Total,
            "Total".to_string(),
            palette::hex(TOTAL_COLOR),
            row,
        ));
    }

    // gen 1: weighted and useful totals
    if gen1 && show_wbst {
        ui.add_space(4.0);
        let (row, _) = ui.allocate_exact_size(Vec2::new(w, ROW_H), Sense::hover());
        let resp = ui
            .interact(row, a.id.with("wbst"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Weighted Base Stat Total \u{2014} Special counted twice");
        let is_open = open_key.as_deref() == Some("__wbst__");
        paint_stat_row(
            ui,
            theme,
            row,
            "WBST",
            &wbst.to_string(),
            if is_open {
                palette::WBST_COLOR
            } else {
                Color32::WHITE
            },
            None,
        );
        if resp.hovered() || resp.clicked() {
            hovered = Some((Kind::Wbst, "WBST".to_string(), palette::WBST_COLOR, row));
        }
    }
    if gen1 && show_ubst {
        if let Some(ubst) = ubst {
            ui.add_space(4.0);
            let (row, _) = ui.allocate_exact_size(Vec2::new(w, ROW_H), Sense::hover());
            let resp = ui.interact(row, a.id.with("ubst"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Useful Base Stat Total \u{2014} WBST minus an offensive stat the species can't use");
            let is_open = open_key.as_deref() == Some("__ubst__");
            paint_stat_row(
                ui,
                theme,
                row,
                "UBST",
                &ubst.to_string(),
                if is_open {
                    palette::UBST_COLOR
                } else {
                    Color32::WHITE
                },
                None,
            );
            if resp.hovered() || resp.clicked() {
                hovered = Some((Kind::Ubst, "UBST".to_string(), palette::UBST_COLOR, row));
            }
        }
    }

    // bulk
    if show_bulk {
        for (kind, label, name, value, color) in [
            (
                Kind::PhysicalBulk,
                "Phys Bulk",
                "Physical Bulk",
                physical_bulk,
                palette::hex(PHYSICAL_BULK_COLOR),
            ),
            (
                Kind::SpecialBulk,
                "Spec Bulk",
                "Special Bulk",
                special_bulk,
                palette::hex(SPECIAL_BULK_COLOR),
            ),
        ] {
            ui.add_space(4.0);
            let (row, _) = ui.allocate_exact_size(Vec2::new(w, ROW_H), Sense::hover());
            let resp = ui
                .interact(row, a.id.with(kind.key()), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            ui.painter().text(
                egui::Pos2::new(row.min.x + 56.0, row.center().y),
                Align2::RIGHT_CENTER,
                label,
                palette::px_bold(theme, 12.0),
                palette::GRAY_500,
            );
            ui.painter().text(
                egui::Pos2::new(row.max.x, row.center().y),
                Align2::RIGHT_CENTER,
                xpr_ui_kit::widgets::fmt_thousands(value),
                palette::px_bold(theme, 14.0),
                color,
            );
            if resp.hovered() || resp.clicked() {
                hovered = Some((kind, name.to_string(), color, row));
            }
        }
    }

    // ---- the popover ----------------------------------------------------------------------------
    let now = ctx.input(|i| i.time);
    let on_row = hovered.is_some();
    let filter: Option<HashSet<String>> = (a.use_filtered && !cx.state.filtered_names.is_empty())
        .then(|| cx.state.filtered_names.iter().cloned().collect());
    if let Some((kind, label, color, rect)) = hovered {
        let key = kind.key();
        match hover.open.as_mut() {
            Some(o) if o.key == key => {
                o.leave_at = None;
                o.anchor = rect;
            }
            _ => {
                hover.session += 1;
                hover.card = false;
                hover.open = Some(Open {
                    key,
                    spec: spec_for(kind, &label, color, a, filter.as_ref()),
                    anchor: rect,
                    leave_at: None,
                });
            }
        }
    }
    let mut navigate = None;
    if let Some(mut open) = hover.open.take() {
        let mut keep = on_row;
        let pop = ranking::ranking_popover(
            cx,
            &ctx,
            a.id,
            hover.session,
            open.anchor,
            &open.spec,
            &a.menu,
            &mut hover.card,
        );
        keep |= pop.pointer_inside || pop.hold;
        let mut close = false;
        if let Some(n) = pop.navigate {
            navigate = Some(n);
            close = true;
        }
        if hover.card {
            let card = ranking::ranking_card(cx, &ctx, a.id, &open.spec);
            keep = true;
            if let Some(n) = card.navigate {
                navigate = Some(n);
                close = true;
            }
            if card.card_closed {
                hover.card = false;
                ranking::reset_card(&ctx, a.id);
            }
        }
        if close {
            hover.card = false;
            ranking::reset_card(&ctx, a.id);
        } else if keep {
            open.leave_at = None;
            hover.open = Some(open);
        } else {
            let t = *open.leave_at.get_or_insert(now);
            if now - t < LINGER {
                ctx.request_repaint_after(Duration::from_secs_f64(LINGER - (now - t)));
                hover.open = Some(open);
            }
        }
    }
    ctx.data_mut(|d| d.insert_temp(a.id, hover));
    navigate
}

/// One bar row: right-aligned label, coloured value and (for the six
/// stats) the 14 px bar. `bar` is (fraction, popover-open).
fn paint_stat_row(
    ui: &Ui,
    theme: &Theme,
    rect: Rect,
    label: &str,
    value: &str,
    color: Color32,
    bar: Option<(f32, bool)>,
) {
    let cy = rect.center().y;
    ui.painter().text(
        egui::Pos2::new(rect.min.x + 32.0, cy),
        Align2::RIGHT_CENTER,
        label,
        palette::px_bold(theme, 12.0),
        palette::GRAY_500,
    );
    ui.painter().text(
        egui::Pos2::new(rect.min.x + 32.0 + 6.0 + 28.0, cy),
        Align2::RIGHT_CENTER,
        value,
        palette::px_bold(theme, 14.0),
        color,
    );
    if let Some((frac, open)) = bar {
        let track = Rect::from_min_max(
            egui::Pos2::new(rect.min.x + 72.0, cy - 7.0),
            egui::Pos2::new(rect.max.x, cy + 7.0),
        );
        ui.painter()
            .rect_filled(track, CornerRadius::same(2), palette::GRAY_700);
        let fill = Rect::from_min_size(track.min, Vec2::new(track.width() * frac, track.height()));
        if fill.width() > 0.5 {
            widgets::paint_glossy(
                ui,
                fill,
                2,
                if open {
                    color
                } else {
                    color.gamma_multiply(0.85)
                },
            );
        }
    }
}
