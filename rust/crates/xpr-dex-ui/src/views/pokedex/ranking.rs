//! The stat ranking popover (Solodex `BaseStats.tsx`'s `RankingPopover`) and
//! its expanded card (`RankingCard.tsx`).
//!
//! Both are free functions over an [`egui::Context`] and a [`RankingSpec`],
//! so the comparison views can show them for their own stat rows:
//! [`ranking_popover`] draws the 260 px list beside `anchor`,
//! [`ranking_card`] the 300 px modal card that its title opens. Neither
//! keeps open-state of its own besides the scroll position: whoever shows
//! them decides when (hover, click) and for how long.

use std::sync::Arc;

use egui::{Align2, Color32, CornerRadius, Id, Order, Pos2, Rect, Sense, Stroke, Vec2};
use xpr_dex::StatRankEntry;

use super::common;
use super::context_menu::{pokemon_context_menu, MenuCtx};
use crate::palette;
use crate::widgets;
use crate::DexCx;

const POPOVER_WIDTH: f32 = 260.0;
const POPOVER_HEIGHT: f32 = 400.0;
const HEADER_H: f32 = 33.0;
const COLS_H: f32 = 24.0;
const ROW_H: f32 = 20.0;
const CARD_ROW_H: f32 = 28.0;

/// One ranking to show.
#[derive(Clone)]
pub struct RankingSpec {
    /// "HP Ranking \u{2014} Emerald"
    pub title: String,
    /// the stat's colour (title, highlighted row)
    pub color: Color32,
    pub entries: Arc<Vec<StatRankEntry>>,
    /// the species to highlight and centre on
    pub current: String,
    pub game: String,
}

/// What a frame of the popover (or card) reported.
#[derive(Clone, Debug, Default)]
pub struct RankingOut {
    /// a species row was clicked (the caller selects it and closes)
    pub navigate: Option<String>,
    /// the pointer is over the popover
    pub pointer_inside: bool,
    /// something belonging to the popover is open (a row's context menu,
    /// the expanded card), so it must not auto-close
    pub hold: bool,
    /// the card was closed this frame
    pub card_closed: bool,
}

/// True for the first few frames of something that scrolls to a row when it
/// opens: a freshly created `Area` lays itself out in a sizing pass whose
/// scroll position is thrown away, so a single frame is not enough.
fn still_settling(ctx: &egui::Context, id: Id) -> bool {
    let n = ctx.data(|d| d.get_temp::<u8>(id).unwrap_or(0));
    ctx.data_mut(|d| d.insert_temp(id, n.saturating_add(1)));
    n < 3
}

/// Forget the settle counter of a card that was closed.
pub fn reset_card(ctx: &egui::Context, id: Id) {
    ctx.data_mut(|d| d.remove::<u8>(id.with("card_init")));
}

fn tint_bg(color: Color32) -> Color32 {
    // `${statColor}22`
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 0x22)
}

fn brighten(c: Color32) -> Color32 {
    // `hover:brightness-125`
    let f = |v: u8| ((v as f32 * 1.25).min(255.0)) as u8;
    Color32::from_rgb(f(c.r()), f(c.g()), f(c.b()))
}

/// Where the popover goes for a stat row: beside it, on the right when it
/// fits, else on the left, vertically centred on the row and kept on screen.
pub fn popover_pos(anchor: Rect, screen: Rect) -> Pos2 {
    let left = if anchor.max.x + 6.0 + POPOVER_WIDTH <= screen.max.x {
        anchor.max.x + 6.0
    } else {
        anchor.min.x - POPOVER_WIDTH - 6.0
    };
    let top = (anchor.min.y - POPOVER_HEIGHT / 2.0)
        .max(6.0)
        .min(screen.max.y - POPOVER_HEIGHT - 6.0);
    Pos2::new(left, top)
}

/// The ranking popover for `spec` beside `anchor`. `id` is the caller's
/// stable id for this popover and `session` changes whenever it is opened
/// anew (so the list scrolls to the current species again).
#[allow(clippy::too_many_arguments)]
pub fn ranking_popover(
    cx: &mut DexCx,
    ctx: &egui::Context,
    id: Id,
    session: u64,
    anchor: Rect,
    spec: &RankingSpec,
    mc: &MenuCtx,
    card_open: &mut bool,
) -> RankingOut {
    let theme = cx.theme;
    let mut out = RankingOut::default();
    let pos = popover_pos(anchor, ctx.content_rect());
    let current_idx = spec.entries.iter().position(|e| e.name == spec.current);
    let first = still_settling(ctx, id.with(("rank_init", session)));
    let area = egui::Area::new(id.with("area"))
        .order(Order::Foreground)
        .fixed_pos(pos)
        .constrain(false);
    let resp = area.show(ctx, |ui| {
        egui::Frame::new()
            .fill(palette::GRAY_800)
            .stroke(Stroke::new(1.0_f32, palette::GRAY_600))
            .corner_radius(CornerRadius::same(8))
            .shadow(egui::Shadow {
                offset: [0, 12],
                blur: 32,
                spread: 0,
                color: Color32::from_black_alpha(150),
            })
            .show(ui, |ui| {
                ui.set_width(POPOVER_WIDTH - 2.0);
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                // title (click to expand)
                let (head, _) = ui
                    .allocate_exact_size(Vec2::new(POPOVER_WIDTH - 2.0, HEADER_H), Sense::hover());
                let g = common::spaced_galley(
                    ui,
                    &spec.title.to_uppercase(),
                    palette::px_bold(theme, 12.0),
                    spec.color,
                    1.2,
                    f32::INFINITY,
                );
                let trect =
                    Rect::from_min_size(Pos2::new(head.min.x + 12.0, head.min.y + 8.0), g.size());
                let tresp = ui
                    .interact(trect, id.with("title"), Sense::click())
                    .on_hover_text("Click to expand")
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                let color = if tresp.hovered() {
                    brighten(spec.color)
                } else {
                    spec.color
                };
                ui.painter().galley(trect.min, g, color);
                ui.painter().hline(
                    head.x_range(),
                    head.max.y - 0.5,
                    Stroke::new(1.0_f32, palette::GRAY_700),
                );
                if tresp.clicked() {
                    *card_open = true;
                }
                // column header
                let (cols, _) =
                    ui.allocate_exact_size(Vec2::new(POPOVER_WIDTH - 2.0, COLS_H), Sense::hover());
                let f = palette::px_bold(theme, 12.0);
                ui.painter().text(
                    Pos2::new(cols.min.x + 8.0, cols.center().y),
                    Align2::LEFT_CENTER,
                    "#",
                    f.clone(),
                    palette::GRAY_500,
                );
                ui.painter().text(
                    Pos2::new(cols.min.x + 32.0 + 8.0, cols.center().y),
                    Align2::LEFT_CENTER,
                    "Pok\u{e9}mon",
                    f.clone(),
                    palette::GRAY_500,
                );
                ui.painter().text(
                    Pos2::new(cols.max.x - 8.0, cols.center().y),
                    Align2::RIGHT_CENTER,
                    "Val",
                    f,
                    palette::GRAY_500,
                );
                // rows
                let view_h = POPOVER_HEIGHT - COLS_H;
                let mut scroll = egui::ScrollArea::vertical()
                    .id_salt(id.with(("rank_rows", session)))
                    .min_scrolled_height(view_h)
                    .max_height(view_h)
                    .auto_shrink([false, false]);
                if first {
                    if let Some(i) = current_idx {
                        scroll = scroll.vertical_scroll_offset(
                            (i as f32 * ROW_H - view_h / 2.0 + ROW_H / 2.0).max(0.0),
                        );
                    }
                }
                scroll.show_rows(ui, ROW_H, spec.entries.len(), |ui, range| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    for i in range {
                        let e = &spec.entries[i];
                        let is_current = e.name == spec.current;
                        let (rect, _) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), ROW_H),
                            Sense::hover(),
                        );
                        let resp =
                            ui.interact(rect, id.with(("rank_row", &e.name)), Sense::click());
                        if is_current {
                            ui.painter()
                                .rect_filled(rect, CornerRadius::ZERO, tint_bg(spec.color));
                        } else if resp.hovered() {
                            ui.painter().rect_filled(
                                rect,
                                CornerRadius::ZERO,
                                palette::GRAY_700.gamma_multiply(0.5),
                            );
                        }
                        let cy = rect.center().y;
                        let small = palette::px(theme, 12.0);
                        ui.painter().text(
                            Pos2::new(rect.min.x + 8.0, cy),
                            Align2::LEFT_CENTER,
                            e.rank.to_string(),
                            small.clone(),
                            palette::GRAY_500,
                        );
                        let sprite = Rect::from_min_size(
                            Pos2::new(rect.min.x + 40.0, cy - 8.0),
                            Vec2::splat(16.0),
                        );
                        common::paint_sprite(ui, cx.images, &e.name, e.dex, sprite, true);
                        let name_color = if is_current {
                            spec.color
                        } else {
                            palette::hex("#a8b6c2")
                        };
                        let vw = widgets::text_w(ui, &e.value.to_string(), &small) + 16.0;
                        let name = xpr_ui_kit::widgets::elide(
                            ui,
                            xpr_dex::display_name(&e.name),
                            &palette::px_bold(theme, 12.0),
                            (rect.max.x - rect.min.x - 60.0 - vw).max(10.0),
                        );
                        ui.painter().text(
                            Pos2::new(rect.min.x + 60.0, cy),
                            Align2::LEFT_CENTER,
                            name,
                            palette::px_bold(theme, 12.0),
                            name_color,
                        );
                        ui.painter().text(
                            Pos2::new(rect.max.x - 8.0, cy),
                            Align2::RIGHT_CENTER,
                            xpr_ui_kit::widgets::fmt_thousands(e.value),
                            small,
                            palette::GRAY_300,
                        );
                        if resp.clicked() && !is_current {
                            out.navigate = Some(e.name.clone());
                        }
                        pokemon_context_menu(cx, &resp, &e.name, mc);
                        if egui::Popup::is_id_open(
                            ui.ctx(),
                            egui::Popup::default_response_id(&resp),
                        ) {
                            out.hold = true;
                        }
                    }
                });
            });
    });
    out.pointer_inside = ctx.rect_contains_pointer(resp.response.layer_id, resp.response.rect);
    out
}

/// The expanded card (`RankingCard.tsx`): a 300 px column of every ranked
/// species on a dimmed backdrop. Click outside or Escape closes it; a row
/// navigates to its species.
pub fn ranking_card(cx: &mut DexCx, ctx: &egui::Context, id: Id, spec: &RankingSpec) -> RankingOut {
    let theme = cx.theme;
    let mut out = RankingOut {
        hold: true,
        ..Default::default()
    };
    let screen = ctx.content_rect();
    let max_h = screen.height() * 0.8;
    let modal = egui::Modal::new(id.with("card"))
        .backdrop_color(Color32::from_black_alpha(178))
        .frame(
            egui::Frame::new()
                .fill(palette::GRAY_900)
                .stroke(Stroke::new(1.0_f32, palette::GRAY_700))
                .corner_radius(CornerRadius::same(16))
                .inner_margin(egui::Margin::ZERO),
        );
    let current_idx = spec.entries.iter().position(|e| e.name == spec.current);
    let first = still_settling(ctx, id.with("card_init"));
    let resp = modal.show(ctx, |ui| {
        ui.set_width(300.0);
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let (head, _) = ui.allocate_exact_size(Vec2::new(300.0, 37.0), Sense::hover());
        let g = common::spaced_galley(
            ui,
            &spec.title.to_uppercase(),
            palette::px_bold(theme, 14.0),
            spec.color,
            1.4,
            276.0,
        );
        ui.painter().galley(
            Pos2::new(head.min.x + 12.0, head.min.y + 8.0),
            g,
            spec.color,
        );
        ui.painter().hline(
            head.x_range(),
            head.max.y - 0.5,
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(75, 85, 99, 153)),
        );
        let (cols, _) = ui.allocate_exact_size(Vec2::new(300.0, 28.0), Sense::hover());
        ui.painter()
            .rect_filled(cols, CornerRadius::ZERO, palette::GRAY_800);
        let f = palette::px_bold(theme, 14.0);
        ui.painter().text(
            Pos2::new(cols.min.x + 8.0, cols.center().y),
            Align2::LEFT_CENTER,
            "#",
            f.clone(),
            palette::GRAY_500,
        );
        ui.painter().text(
            Pos2::new(cols.min.x + 40.0 + 8.0, cols.center().y),
            Align2::LEFT_CENTER,
            "Pok\u{e9}mon",
            f.clone(),
            palette::GRAY_500,
        );
        ui.painter().text(
            Pos2::new(cols.max.x - 8.0, cols.center().y),
            Align2::RIGHT_CENTER,
            "Value",
            f,
            palette::GRAY_500,
        );
        let view_h = (max_h - 65.0).max(120.0);
        let n = spec.entries.len();
        let shown_h = (n as f32 * CARD_ROW_H).min(view_h);
        let mut scroll = egui::ScrollArea::vertical()
            .id_salt(id.with("card_rows"))
            .max_height(shown_h)
            .auto_shrink([false, false]);
        if first {
            if let Some(i) = current_idx {
                scroll = scroll.vertical_scroll_offset(
                    (i as f32 * CARD_ROW_H - shown_h / 2.0 + CARD_ROW_H / 2.0).max(0.0),
                );
            }
        }
        // a modal's area offers only part of the screen's height; ask for the rows' own
        ui.allocate_ui(Vec2::new(300.0, shown_h), |ui| {
            scroll.show_rows(ui, CARD_ROW_H, n, |ui, range| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                for i in range {
                    let e = &spec.entries[i];
                    let is_current = e.name == spec.current;
                    let (rect, _) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), CARD_ROW_H),
                        Sense::hover(),
                    );
                    let resp = ui.interact(rect, id.with(("card_row", &e.name)), Sense::click());
                    if is_current {
                        ui.painter()
                            .rect_filled(rect, CornerRadius::ZERO, tint_bg(spec.color));
                    } else if resp.hovered() {
                        ui.painter().rect_filled(
                            rect,
                            CornerRadius::ZERO,
                            palette::GRAY_700.gamma_multiply(0.5),
                        );
                    }
                    let cy = rect.center().y;
                    let body = palette::px(theme, 14.0);
                    ui.painter().text(
                        Pos2::new(rect.min.x + 8.0, cy),
                        Align2::LEFT_CENTER,
                        e.rank.to_string(),
                        body.clone(),
                        palette::GRAY_500,
                    );
                    let sprite = Rect::from_min_size(
                        Pos2::new(rect.min.x + 48.0, cy - 12.0),
                        Vec2::splat(24.0),
                    );
                    common::paint_sprite(ui, cx.images, &e.name, e.dex, sprite, true);
                    let name_color = if is_current {
                        spec.color
                    } else {
                        palette::hex("#a8b6c2")
                    };
                    let name = xpr_ui_kit::widgets::elide(
                        ui,
                        xpr_dex::display_name(&e.name),
                        &palette::px_bold(theme, 14.0),
                        rect.width() - 80.0 - 64.0,
                    );
                    ui.painter().text(
                        Pos2::new(rect.min.x + 78.0, cy),
                        Align2::LEFT_CENTER,
                        name,
                        palette::px_bold(theme, 14.0),
                        name_color,
                    );
                    ui.painter().text(
                        Pos2::new(rect.max.x - 8.0, cy),
                        Align2::RIGHT_CENTER,
                        e.value.to_string(),
                        palette::px_bold(theme, 14.0),
                        if is_current {
                            spec.color
                        } else {
                            palette::GRAY_300
                        },
                    );
                    if resp.clicked() && !is_current {
                        out.navigate = Some(e.name.clone());
                    }
                }
            });
        });
    });
    if resp.should_close() || out.navigate.is_some() {
        out.card_closed = true;
        out.hold = false;
    }
    out
}
