//! The ranking popovers of the comparison views' stat rows and the machinery
//! that opens them (Solodex `ComparisonRankingPopover`, `SelfRankingPopover`,
//! `TripleRankingPopover`).
//!
//! The list itself is the Pokédex popover (`pokedex::ranking::ranking_popover`)
//! adapted: the comparison popovers highlight *several* species, sit on a
//! chosen side of the row, and (triple view) have no expanded card. The
//! expanded card is the Pokédex's [`ranking::ranking_card`].
//!
//! [`drive`] is the open / linger / dismiss logic shared by the three views:
//! the pair and triple views open on hover and close 200 ms after the pointer
//! left; the self view opens on a click and closes on a click elsewhere.

use std::sync::Arc;
use std::time::Duration;

use egui::{Align2, Color32, CornerRadius, Id, Order, Pos2, Rect, Sense, Stroke, Vec2};
use xpr_dex::{RankKind, StatRankEntry};

use super::draw::{paint_small_sprite, spaced_galley};
use crate::palette;
use crate::views::pokedex::ranking::{self, RankingOut, RankingSpec};
use crate::views::pokedex::{pokemon_context_menu, MenuCtx};
use crate::widgets;
use crate::DexCx;

const POPOVER_WIDTH: f32 = 260.0;
const POPOVER_HEIGHT: f32 = 400.0;
/// The title block: `px-3 py-2` around a 16 px `<p>` (the triple view), or
/// around a `<button>` that sits in a 24 px line box (the pair and self views).
const HEADER_H: f32 = 33.0;
const HEADER_BUTTON_H: f32 = 41.0;
const COLS_H: f32 = 24.0;
/// A ranking row (`py-0.5` around the 20 px of sprite and text).
const ROW_H: f32 = 24.0;
/// How long a hover popover lingers after the pointer left (Solodex's 200 ms).
const LINGER: f64 = 0.2;

/// One ranking to show.
#[derive(Clone)]
pub struct PopSpec {
    /// "HP Ranking \u{2014} Emerald"
    pub title: String,
    /// the stat's colour (title, highlighted rows)
    pub color: Color32,
    pub entries: Arc<Vec<StatRankEntry>>,
    pub game: String,
}

impl PopSpec {
    pub fn new(kind: RankKind, label: &str, color: Color32, game: &str, with_game: bool) -> PopSpec {
        PopSpec {
            title: if with_game {
                format!("{} Ranking \u{2014} {}", label, game)
            } else {
                format!("{} Ranking", label)
            },
            color,
            entries: xpr_dex::get_ranking(kind, game, None),
            game: game.to_string(),
        }
    }
}

/// Which side of the stat row a popover goes on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// left of the row (kept on screen)
    Left,
    /// right of the row (kept on screen)
    Right,
    /// right when it fits, else left
    Auto,
}

/// One popover of a stat row.
#[derive(Clone)]
pub struct PopLook {
    pub side: Side,
    /// the species the list centres on (None: the first highlighted row)
    pub scroll_to: Option<String>,
    /// the title opens the expanded card
    pub card: bool,
}

/// Where the popover goes: beside the row, vertically centred on it and kept
/// on screen (Solodex's `left` / `top` of each popover).
pub fn pos_for(side: Side, anchor: Rect, screen: Rect) -> Pos2 {
    let w = POPOVER_WIDTH;
    let left = match side {
        Side::Left => (anchor.min.x - w - 6.0).max(6.0),
        Side::Right => (anchor.max.x + 6.0).min(screen.max.x - w - 6.0),
        Side::Auto => {
            if anchor.max.x + 6.0 + w <= screen.max.x {
                anchor.max.x + 6.0
            } else {
                (anchor.min.x - w - 6.0).max(6.0)
            }
        }
    };
    let top = (anchor.min.y - POPOVER_HEIGHT / 2.0)
        .max(6.0)
        .min(screen.max.y - POPOVER_HEIGHT - 6.0);
    Pos2::new(left, top)
}

/// True for the first few frames of something that scrolls to a row when it
/// opens: a freshly created `Area` lays itself out in a sizing pass whose
/// scroll position is thrown away, so a single frame is not enough.
fn still_settling(ctx: &egui::Context, id: Id) -> bool {
    let n = ctx.data(|d| d.get_temp::<u8>(id).unwrap_or(0));
    ctx.data_mut(|d| d.insert_temp(id, n.saturating_add(1)));
    n < 3
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

/// The popover for `spec` beside `anchor`. `id` is the caller's stable id
/// for this popover and `session` changes whenever it is opened anew (so
/// the list scrolls again).
#[allow(clippy::too_many_arguments)]
pub fn popover(
    cx: &mut DexCx,
    ctx: &egui::Context,
    id: Id,
    session: u64,
    anchor: Rect,
    spec: &PopSpec,
    highlight: &[String],
    look: &PopLook,
    mc: &MenuCtx,
    card_open: &mut bool,
) -> RankingOut {
    let theme = cx.theme;
    let mut out = RankingOut::default();
    let pos = pos_for(look.side, anchor, ctx.content_rect());
    let is_hl = |name: &str| highlight.iter().any(|h| h == name);
    let target_idx = match &look.scroll_to {
        Some(name) => spec.entries.iter().position(|e| e.name == *name),
        None => spec.entries.iter().position(|e| is_hl(&e.name)),
    };
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
                // title (a button that expands the card in the pair and self views)
                let head_h = if look.card { HEADER_BUTTON_H } else { HEADER_H };
                let (head, _) = ui
                    .allocate_exact_size(Vec2::new(POPOVER_WIDTH - 2.0, head_h), Sense::hover());
                let g = spaced_galley(
                    ui,
                    &spec.title.to_uppercase(),
                    palette::px_bold(theme, 12.0),
                    spec.color,
                    1.2,
                    f32::INFINITY,
                );
                let text_y = head.min.y + if look.card { 12.0 } else { 8.0 };
                let trect = Rect::from_min_size(Pos2::new(head.min.x + 12.0, text_y), g.size());
                let mut color = spec.color;
                super::probe::record("poptitle", &spec.title, 0, &spec.title, trect);
                if look.card {
                    let tresp = ui
                        .interact(trect, id.with("title"), Sense::click())
                        .on_hover_text("Click to expand")
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if tresp.hovered() {
                        color = brighten(spec.color);
                    }
                    if tresp.clicked() {
                        *card_open = true;
                    }
                }
                ui.painter().galley(trect.min, g, color);
                ui.painter().hline(
                    head.x_range(),
                    head.max.y - 0.5,
                    Stroke::new(1.0_f32, palette::GRAY_700),
                );
                // the rank column is as wide as its widest rank (at least `w-8`)
                let rank_w = spec
                    .entries
                    .last()
                    .map(|e| widgets::text_w(ui, &e.rank.to_string(), &palette::px(theme, 12.0)) + 16.0)
                    .unwrap_or(0.0)
                    .max(32.0);
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
                    Pos2::new(cols.min.x + rank_w + 8.0, cols.center().y),
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
                    if let Some(i) = target_idx {
                        scroll = scroll.vertical_scroll_offset(
                            // Solodex centres the row in the 400 px box, header row included
                            (i as f32 * ROW_H + COLS_H - POPOVER_HEIGHT / 2.0 + ROW_H / 2.0)
                                .max(0.0),
                        );
                    }
                }
                scroll.show_rows(ui, ROW_H, spec.entries.len(), |ui, range| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    for i in range {
                        let e = &spec.entries[i];
                        let is_current = is_hl(&e.name);
                        let (rect, _) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), ROW_H),
                            Sense::hover(),
                        );
                        let resp =
                            ui.interact(rect, id.with(("rank_row", &e.name)), Sense::click());
                        super::probe::record("pop", &spec.title, i, &e.name, rect);
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
                            Pos2::new(rect.min.x + rank_w + 8.0, cy - 8.0),
                            Vec2::splat(16.0),
                        );
                        paint_small_sprite(ui, cx.images, &e.name, e.dex, sprite);
                        let name_color = if is_current {
                            spec.color
                        } else {
                            palette::hex("#a8b6c2")
                        };
                        let vw = widgets::text_w(ui, &e.value.to_string(), &small) + 16.0;
                        let name = xpr_ui_kit::widgets::elide(
                            ui,
                            xpr_dex::display_name(&e.name),
                            &palette::px(theme, 12.0),
                            (rect.max.x - rect.min.x - rank_w - 28.0 - vw).max(10.0),
                        );
                        ui.painter().text(
                            Pos2::new(rect.min.x + rank_w + 28.0, cy),
                            Align2::LEFT_CENTER,
                            name,
                            palette::px(theme, 12.0),
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

// ---------------------------------------------------------------------------
// opening and closing
// ---------------------------------------------------------------------------

/// How a stat row's popover is opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activation {
    /// pair and triple views: while the pointer is over the row (or the
    /// popover), and 200 ms after
    Hover,
    /// self view: a click on the row; a click elsewhere closes
    Click,
}

#[derive(Clone)]
struct Open {
    key: String,
    spec: PopSpec,
    anchor: Rect,
    leave_at: Option<f64>,
}

/// The state of one stat block's popovers, kept in egui memory.
#[derive(Clone, Default)]
struct State {
    open: Option<Open>,
    session: u64,
    /// the expanded card of each popover
    cards: [bool; 2],
    /// a row's context menu was open last frame
    menu_open: bool,
    /// what the popovers were opened for ([`Cfg::epoch`])
    epoch: u64,
}

/// What [`drive`] needs besides the open request.
pub struct Cfg<'a> {
    /// stable id of the stat block
    pub id: Id,
    /// changes when the block is about something else (other species, game,
    /// or the view was mounted anew): an open popover is closed
    pub epoch: u64,
    pub activation: Activation,
    /// species highlighted in the lists
    pub highlight: &'a [String],
    /// the popovers to show (one, or the pair view's left and right)
    pub looks: &'a [PopLook],
    /// the page's selection, for the rows' context menu
    pub menu: &'a MenuCtx,
    /// every stat row's rect (a press on one does not dismiss in click mode)
    pub rows: &'a [Rect],
}

/// Show the popovers of a stat block. `request` is the row the pointer is on
/// (hover mode) or the row that was clicked (click mode): its key and rect;
/// `make` builds the ranking of a key when a popover opens. Returns the
/// species a row of a popover picked.
pub fn drive(
    cx: &mut DexCx,
    ui: &egui::Ui,
    cfg: &Cfg,
    request: Option<(String, Rect)>,
    make: &dyn Fn(&str) -> PopSpec,
) -> Option<String> {
    let ctx = ui.ctx().clone();
    let mut st: State = ctx.data(|d| d.get_temp(cfg.id)).unwrap_or_default();
    if st.epoch != cfg.epoch {
        for n in 0..2 {
            ranking::reset_card(&ctx, cfg.id.with(("pop", n)).with("card"));
        }
        st = State {
            epoch: cfg.epoch,
            ..State::default()
        };
    }
    let now = ctx.input(|i| i.time);
    let on_row = request.is_some() && cfg.activation == Activation::Hover;
    if let Some((key, anchor)) = request {
        match st.open.as_mut() {
            Some(o) if o.key == key => {
                o.leave_at = None;
                o.anchor = anchor;
            }
            _ => {
                st.session += 1;
                st.cards = [false; 2];
                st.open = Some(Open {
                    spec: make(&key),
                    key,
                    anchor,
                    leave_at: None,
                });
            }
        }
    }
    // a click elsewhere dismisses (Solodex's `usePopoverDismiss`); a raw read,
    // so it stands down under a dialog or an open menu
    let pressed = cfg.activation == Activation::Click
        && st.open.is_some()
        && ctx.input(|i| i.pointer.any_pressed())
        && !xpr_ui_kit::modal::pointer_blocked(ui);
    let press_pos = ctx.input(|i| i.pointer.interact_pos());
    let mut navigate = None;
    let mut menu_open = false;
    if let Some(mut open) = st.open.take() {
        let mut keep = on_row;
        let mut close = false;
        let mut inside = false;
        let mut hold = false;
        for (n, look) in cfg.looks.iter().enumerate() {
            let pid = cfg.id.with(("pop", n));
            let mut card = st.cards[n];
            let pop = popover(
                cx,
                &ctx,
                pid,
                st.session,
                open.anchor,
                &open.spec,
                cfg.highlight,
                look,
                cfg.menu,
                &mut card,
            );
            inside |= pop.pointer_inside;
            hold |= pop.hold;
            menu_open |= pop.hold;
            if let Some(name) = pop.navigate {
                navigate = Some(name);
                close = true;
            }
            if card {
                let spec = RankingSpec {
                    title: open.spec.title.clone(),
                    color: open.spec.color,
                    entries: open.spec.entries.clone(),
                    current: look
                        .scroll_to
                        .clone()
                        .or_else(|| cfg.highlight.first().cloned())
                        .unwrap_or_default(),
                    game: open.spec.game.clone(),
                };
                let out = ranking::ranking_card(cx, &ctx, pid.with("card"), &spec);
                hold = true;
                if let Some(name) = out.navigate {
                    navigate = Some(name);
                    close = true;
                }
                if out.card_closed {
                    card = false;
                    ranking::reset_card(&ctx, pid.with("card"));
                }
            }
            st.cards[n] = card;
        }
        keep |= inside || hold;
        if cfg.activation == Activation::Click {
            // a click outside the popover and the rows closes it; the click
            // that closes a row's menu closes the popover too
            let on_a_row = press_pos
                .map(|p| cfg.rows.iter().any(|r| r.contains(p)))
                .unwrap_or(false);
            if (pressed && !inside && !hold && !on_a_row) || (st.menu_open && !menu_open) {
                close = true;
            }
            keep = true;
        }
        if close {
            for n in 0..2 {
                ranking::reset_card(&ctx, cfg.id.with(("pop", n)).with("card"));
            }
            st.cards = [false; 2];
        } else if keep {
            open.leave_at = None;
            st.open = Some(open);
        } else {
            let t = *open.leave_at.get_or_insert(now);
            if now - t < LINGER {
                ctx.request_repaint_after(Duration::from_secs_f64(LINGER - (now - t)));
                st.open = Some(open);
            }
        }
    }
    st.menu_open = menu_open;
    ctx.data_mut(|d| d.insert_temp(cfg.id, st));
    navigate
}
