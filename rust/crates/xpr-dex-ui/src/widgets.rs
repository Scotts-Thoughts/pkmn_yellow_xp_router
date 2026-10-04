//! Widgets shared by the Dex's tabs: type badges, the game toggle, section
//! labels, popovers, dialogs, sort headers, search fields and the search
//! overlay.

use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{
    Align2, Color32, CornerRadius, FontId, Id, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2,
};
use xpr_ui_kit::theme::{self, Theme};
use xpr_ui_kit::widgets::{Entry, EntryResponse};

use crate::palette::{self, hex};

// ---------------------------------------------------------------------------
// painting helpers
// ---------------------------------------------------------------------------

/// A rounded rect in `color` with Solodex's top gloss
/// (`linear-gradient(180deg, rgba(255,255,255,.18), transparent)`).
pub fn paint_glossy(ui: &Ui, rect: Rect, radius: u8, color: Color32) {
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(radius), color);
    let inner = rect.shrink2(Vec2::new(radius as f32 * 0.3, 0.0));
    let top = Color32::from_white_alpha(46);
    let bottom = Color32::from_white_alpha(0);
    let mut mesh = Mesh::default();
    let i = mesh.vertices.len() as u32;
    for (pos, c) in [
        (inner.left_top(), top),
        (inner.right_top(), top),
        (inner.right_bottom(), bottom),
        (inner.left_bottom(), bottom),
    ] {
        mesh.vertices.push(Vertex {
            pos,
            uv: WHITE_UV,
            color: c,
        });
    }
    mesh.indices
        .extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
    p.add(egui::Shape::mesh(mesh));
}

/// White text with a soft drop shadow, centred in `rect`.
pub fn paint_shadowed_text(ui: &Ui, rect: Rect, text: &str, font: FontId, color: Color32) {
    let p = ui.painter();
    p.text(
        rect.center() + Vec2::new(0.0, 1.0),
        Align2::CENTER_CENTER,
        text,
        font.clone(),
        Color32::from_black_alpha(100),
    );
    p.text(rect.center(), Align2::CENTER_CENTER, text, font, color);
}

/// Width of `text` in `font`.
pub fn text_w(ui: &Ui, text: &str, font: &FontId) -> f32 {
    xpr_ui_kit::widgets::text_width(ui, text, font)
}

// ---------------------------------------------------------------------------
// type badges
// ---------------------------------------------------------------------------

/// A non-interactive type chip `width` wide (68 for Solodex's small badges).
pub fn type_chip(ui: &mut Ui, theme: &Theme, t: &str, width: f32) -> Response {
    let h = 18.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, h), Sense::hover());
    paint_glossy(ui, rect, 4, palette::type_color(t));
    paint_shadowed_text(
        ui,
        rect,
        palette::type_label(t),
        palette::px_bold(theme, 12.0),
        Color32::WHITE,
    );
    resp
}

/// A type badge (80 wide, or 68 when `small`); a click opens the type's
/// matchups for `game`'s chart.
pub fn type_badge(
    ui: &mut Ui,
    theme: &Theme,
    t: &str,
    small: bool,
    game: Option<&str>,
) -> Response {
    let (w, h) = if small { (68.0, 18.0) } else { (80.0, 24.0) };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    let base = palette::type_color(t);
    let color = if resp.hovered() {
        theme::with_alpha(base, 204)
    } else {
        base
    };
    paint_glossy(ui, rect, 4, color);
    paint_shadowed_text(
        ui,
        rect,
        palette::type_label(t),
        palette::px_bold(theme, 12.0),
        Color32::WHITE,
    );
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    egui::Popup::from_toggle_button_response(&resp)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .frame(popover_frame(theme))
        .show(|ui| type_popover(ui, theme, t, game));
    resp
}

fn type_popover(ui: &mut Ui, theme: &Theme, t: &str, game: Option<&str>) {
    ui.set_min_width(220.0);
    ui.set_max_width(260.0);
    ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
    ui.label(
        egui::RichText::new(t.to_uppercase())
            .font(palette::px_bold(theme, 14.0))
            .color(palette::type_color(t)),
    );
    ui.add_space(4.0);
    let m = xpr_dex::type_matchups(t, game);
    let row = |ui: &mut Ui, label: &str, types: &[String]| {
        if types.is_empty() {
            return;
        }
        section_label(ui, theme, label);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
            for t in types {
                type_chip(ui, theme, t, 68.0);
            }
        });
        ui.add_space(4.0);
    };
    row(ui, "Super effective vs", &m.super_eff_vs);
    row(ui, "Not very effective vs", &m.not_eff_vs);
    row(ui, "No effect vs", &m.no_eff_vs);
    let offense = !m.super_eff_vs.is_empty() || !m.not_eff_vs.is_empty() || !m.no_eff_vs.is_empty();
    let defense = !m.weak_to.is_empty() || !m.resists.is_empty() || !m.immune_to.is_empty();
    if offense && defense {
        hairline(ui, palette::GRAY_700);
    }
    row(ui, "Weak to", &m.weak_to);
    row(ui, "Resists", &m.resists);
    row(ui, "Immune to", &m.immune_to);
}

// ---------------------------------------------------------------------------
// text
// ---------------------------------------------------------------------------

/// Solodex's small uppercase gray section heading.
pub fn section_label(ui: &mut Ui, theme: &Theme, text: &str) -> Response {
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .font(palette::px(theme, 12.0))
            .color(palette::GRAY_500),
    )
}

/// A one-pixel horizontal rule across the available width.
pub fn hairline(ui: &mut Ui, color: Color32) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter()
        .hline(r.x_range(), r.center().y, Stroke::new(1.0_f32, color));
}

/// A centred gray placeholder filling the available space ("Select a Pokémon").
pub fn placeholder(ui: &mut Ui, theme: &Theme, text: &str) {
    let rect = ui.available_rect_before_wrap();
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        palette::px(theme, 16.0),
        palette::GRAY_600,
    );
    ui.allocate_rect(rect, Sense::hover());
}

// ---------------------------------------------------------------------------
// buttons
// ---------------------------------------------------------------------------

/// Solodex's small gray button (bg gray-800, hover gray-700, border gray-700).
pub fn small_button(ui: &mut Ui, theme: &Theme, text: &str) -> Response {
    small_button_ex(ui, theme, text, false, true)
}

/// [`small_button`] with an active (highlighted) state and enablement.
pub fn small_button_ex(
    ui: &mut Ui,
    theme: &Theme,
    text: &str,
    active: bool,
    enabled: bool,
) -> Response {
    let font = palette::px(theme, 12.0);
    let w = text_w(ui, text, &font) + 16.0;
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(w, 22.0),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let fill = if active {
        palette::GRAY_600
    } else if enabled && resp.hovered() {
        palette::GRAY_700
    } else {
        palette::GRAY_800
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(4),
        fill,
        Stroke::new(1.0_f32, palette::GRAY_700),
        egui::StrokeKind::Inside,
    );
    let color = if !enabled {
        palette::GRAY_600
    } else if active || resp.hovered() {
        Color32::WHITE
    } else {
        palette::GRAY_300
    };
    ui.painter()
        .text(rect.center(), Align2::CENTER_CENTER, text, font, color);
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// One entry of a context / popup menu; returns true when clicked.
pub fn menu_entry(ui: &mut Ui, theme: &Theme, text: &str, enabled: bool) -> bool {
    let font = palette::px(theme, 14.0);
    // not `available_width()`: inside a popup that is the screen's width
    let w = (text_w(ui, text, &font) + 24.0).max(ui.min_rect().width()).max(220.0);
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(w, 26.0),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    if enabled && resp.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(3), palette::GRAY_700);
    }
    ui.painter().text(
        Pos2::new(rect.min.x + 12.0, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        font,
        if enabled {
            palette::GRAY_200
        } else {
            palette::GRAY_600
        },
    );
    enabled && resp.clicked()
}

// ---------------------------------------------------------------------------
// popovers and dialogs
// ---------------------------------------------------------------------------

/// The frame of every popover (gray-800, gray-600 border, rounded, shadow).
pub fn popover_frame(_theme: &Theme) -> egui::Frame {
    egui::Frame::new()
        .fill(palette::GRAY_800)
        .stroke(Stroke::new(1.0_f32, palette::GRAY_600))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .shadow(egui::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(140),
        })
}

/// A popover under `anchor` toggled by clicks on it; closes on a click
/// outside or Escape. Returns the content's value while open.
pub fn click_popover<R>(
    theme: &Theme,
    anchor: &Response,
    width: Option<f32>,
    content: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    let mut p = egui::Popup::from_toggle_button_response(anchor)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .frame(popover_frame(theme));
    if let Some(w) = width {
        p = p.width(w);
    }
    p.show(content).map(|r| r.inner)
}

/// A right-click menu on `resp` (Solodex's context menus).
pub fn context_menu(theme: &Theme, resp: &Response, content: impl FnOnce(&mut Ui)) {
    egui::Popup::context_menu(resp)
        .frame(
            egui::Frame::new()
                .fill(palette::GRAY_800)
                .stroke(Stroke::new(1.0_f32, palette::GRAY_600))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(4, 4)),
        )
        .show(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let _ = theme;
            content(ui)
        });
}

/// A modal dialog in the router's style (see `xpr_ui_kit::modal::modal`).
pub fn modal<R>(
    ctx: &egui::Context,
    theme: &Theme,
    id: &str,
    title: &str,
    width: f32,
    content: impl FnOnce(&mut Ui) -> R,
) -> R {
    xpr_ui_kit::modal::modal(ctx, theme, id, title, width, content)
}

// ---------------------------------------------------------------------------
// game toggle
// ---------------------------------------------------------------------------

/// One button of the game toggle.
#[derive(Clone, Debug, PartialEq)]
pub struct ToggleItem {
    /// the value handed back when clicked (a Dex game or a router version)
    pub key: String,
    pub label: String,
    pub color: Color32,
    pub tooltip: String,
}

#[derive(Clone, Debug, Default)]
pub struct ToggleResponse {
    pub clicked: Option<String>,
    pub right_clicked: Option<String>,
}

/// Solodex's `GameToggle`: a centred, wrapping row of game buttons; the
/// selected one filled with its colour, the others outlined in it.
pub fn game_toggle(
    ui: &mut Ui,
    theme: &Theme,
    items: &[ToggleItem],
    selected: &str,
) -> ToggleResponse {
    // rows stack whatever the caller's layout is
    ui.vertical(|ui| game_toggle_rows(ui, theme, items, selected))
        .inner
}

fn game_toggle_rows(
    ui: &mut Ui,
    theme: &Theme,
    items: &[ToggleItem],
    selected: &str,
) -> ToggleResponse {
    let mut out = ToggleResponse::default();
    let font = palette::px_bold(theme, 12.0);
    let sizes: Vec<f32> = items
        .iter()
        .map(|it| text_w(ui, &it.label, &font) + 24.0)
        .collect();
    let gap = 6.0;
    let avail = ui.available_width();
    // wrap into centred rows
    let mut rows: Vec<Vec<usize>> = vec![Vec::new()];
    let mut row_w = 0.0;
    for (i, w) in sizes.iter().enumerate() {
        if row_w > 0.0 && row_w + gap + w > avail - 16.0 {
            rows.push(Vec::new());
            row_w = 0.0;
        }
        row_w += if row_w > 0.0 { gap + w } else { *w };
        rows.last_mut().unwrap().push(i);
    }
    for row in rows {
        let total: f32 =
            row.iter().map(|i| sizes[*i]).sum::<f32>() + gap * (row.len().saturating_sub(1)) as f32;
        let (line, _) = ui.allocate_exact_size(Vec2::new(avail, 28.0), Sense::hover());
        let mut x = line.center().x - total / 2.0;
        for i in row {
            let it = &items[i];
            let r = Rect::from_min_size(Pos2::new(x, line.min.y + 2.0), Vec2::new(sizes[i], 24.0));
            let resp = ui
                .interact(
                    r,
                    ui.id().with(("dex_game_toggle", &it.key)),
                    Sense::click(),
                )
                .on_hover_text(&it.tooltip);
            let active = it.key == selected;
            if active {
                ui.painter().rect_filled(r, CornerRadius::same(4), it.color);
                ui.painter().text(
                    r.center(),
                    Align2::CENTER_CENTER,
                    &it.label,
                    font.clone(),
                    Color32::WHITE,
                );
            } else {
                let fill = if resp.hovered() {
                    hex("#232a36")
                } else {
                    hex("#1a1f29")
                };
                ui.painter().rect(
                    r,
                    CornerRadius::same(4),
                    fill,
                    Stroke::new(1.0_f32, theme::with_alpha(it.color, 64)),
                    egui::StrokeKind::Inside,
                );
                ui.painter().text(
                    r.center(),
                    Align2::CENTER_CENTER,
                    &it.label,
                    font.clone(),
                    if resp.hovered() {
                        palette::GRAY_200
                    } else {
                        hex("#8b96a5")
                    },
                );
            }
            if resp.clicked() {
                out.clicked = Some(it.key.clone());
            }
            if resp.secondary_clicked() {
                out.right_clicked = Some(it.key.clone());
            }
            x += sizes[i] + gap;
        }
    }
    out
}

// ---------------------------------------------------------------------------
// tables
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDir {
    Asc,
    Desc,
}

impl SortDir {
    pub fn flip(self) -> SortDir {
        match self {
            SortDir::Asc => SortDir::Desc,
            SortDir::Desc => SortDir::Asc,
        }
    }
}

/// A sortable column header cell in `rect`: bold gray, brighter and with an
/// arrow while it is the active sort. Click = sort, right-click = reset.
pub fn sort_header_cell(
    ui: &mut Ui,
    theme: &Theme,
    rect: Rect,
    id: Id,
    label: &str,
    active: Option<SortDir>,
    right_align: bool,
) -> Response {
    let resp = ui.interact(rect, id, Sense::click());
    let color = if active.is_some() {
        palette::GRAY_300
    } else if resp.hovered() {
        palette::GRAY_400
    } else {
        palette::GRAY_600
    };
    let text = match active {
        Some(SortDir::Asc) => format!("{} \u{25B2}", label),
        Some(SortDir::Desc) => format!("{} \u{25BC}", label),
        None => label.to_string(),
    };
    let (pos, align) = if right_align {
        (
            Pos2::new(rect.max.x - 4.0, rect.center().y),
            Align2::RIGHT_CENTER,
        )
    } else {
        (
            Pos2::new(rect.min.x + 4.0, rect.center().y),
            Align2::LEFT_CENTER,
        )
    };
    ui.painter()
        .text(pos, align, text, palette::px_bold(theme, 14.0), color);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

// ---------------------------------------------------------------------------
// inputs
// ---------------------------------------------------------------------------

/// A clearable search field `width` wide.
pub fn search_field(
    ui: &mut Ui,
    theme: &Theme,
    text: &mut String,
    hint: &str,
    id: Id,
    width: f32,
) -> EntryResponse {
    Entry::new(theme, text)
        .width(width)
        .hint(hint)
        .id(id)
        .clearable()
        .show(ui)
}

// ---------------------------------------------------------------------------
// search overlay
// ---------------------------------------------------------------------------

/// One result row of a search overlay.
#[derive(Clone, Debug, PartialEq)]
pub struct SpotlightRow {
    pub key: String,
    pub label: String,
    /// right-aligned gray detail ("#025", "Lv 14", ...)
    pub detail: String,
    /// optional sprite: (species, national dex number)
    pub sprite: Option<(String, i32)>,
    /// optional label colour
    pub color: Option<Color32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpotlightOutcome {
    Open,
    Picked(String),
    Closed,
}

/// The overlay's own state: the query and the highlighted row.
#[derive(Clone, Debug, Default)]
pub struct SpotlightState {
    pub query: String,
    pub highlighted: usize,
    focus_requested: bool,
}

impl SpotlightState {
    pub fn reset(&mut self) {
        *self = SpotlightState::default();
    }
}

/// Solodex's spotlight search: a modal panel near the top of the window
/// with a query field (focused) and a result list. Up / Down move the
/// highlight, Enter picks, Escape or a click outside closes. `results` is
/// recomputed by the caller from `state.query` each frame.
pub fn spotlight(
    ctx: &egui::Context,
    theme: &Theme,
    images: &mut crate::images::DexImages,
    id: &str,
    title: &str,
    accent: Color32,
    state: &mut SpotlightState,
    results: &[SpotlightRow],
) -> SpotlightOutcome {
    let mut outcome = SpotlightOutcome::Open;
    let modal = egui::Modal::new(Id::new(id))
        .frame(
            egui::Frame::new()
                .fill(palette::GRAY_900)
                .stroke(Stroke::new(1.0_f32, palette::GRAY_700))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(egui::Margin::same(10)),
        )
        .backdrop_color(Color32::from_black_alpha(120))
        .area(
            egui::Modal::default_area(Id::new(id).with("area"))
                .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 80.0)),
        );
    let resp = modal.show(ctx, |ui| {
        ui.set_width(520.0);
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(title)
                    .font(palette::px_bold(theme, 12.0))
                    .color(accent),
            );
        });
        let field_id = Id::new(id).with("query");
        let r = Entry::new(theme, &mut state.query)
            .width(520.0)
            .hint("Type to search...")
            .id(field_id)
            .show(ui);
        if !state.focus_requested {
            ui.memory_mut(|m| m.request_focus(field_id));
            state.focus_requested = true;
        }
        if r.changed {
            state.highlighted = 0;
        }
        let (up, down, enter, esc) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Escape),
            )
        });
        // the query field takes Enter itself (and reports it)
        let enter = enter || r.enter_pressed;
        if !results.is_empty() {
            if down {
                state.highlighted = (state.highlighted + 1).min(results.len() - 1);
            }
            if up {
                state.highlighted = state.highlighted.saturating_sub(1);
            }
            state.highlighted = state.highlighted.min(results.len() - 1);
            if enter {
                outcome = SpotlightOutcome::Picked(results[state.highlighted].key.clone());
            }
        }
        if esc {
            outcome = SpotlightOutcome::Closed;
        }
        let row_h = 28.0;
        // `show_rows` adds the item spacing to every row's height
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::ScrollArea::vertical()
            .id_salt(Id::new(id).with("results"))
            .max_height(420.0)
            .auto_shrink([false, true])
            .show_rows(ui, row_h, results.len(), |ui, range| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                for i in range {
                    let row = &results[i];
                    let (rect, resp) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), row_h),
                        Sense::click(),
                    );
                    let hl = i == state.highlighted;
                    if hl {
                        ui.painter().rect_filled(
                            rect,
                            CornerRadius::same(4),
                            theme::with_alpha(accent, 70),
                        );
                        if up || down {
                            ui.scroll_to_rect(rect, None);
                        }
                    } else if resp.hovered() {
                        ui.painter()
                            .rect_filled(rect, CornerRadius::same(4), palette::GRAY_800);
                    }
                    let mut x = rect.min.x + 8.0;
                    if let Some((species, dex)) = &row.sprite {
                        if let Some(tex) =
                            images.sprite(ui.ctx(), species, *dex, crate::images::SpriteSize::Small)
                        {
                            crate::images::paint_fit(
                                ui,
                                &tex,
                                Rect::from_min_size(
                                    Pos2::new(x, rect.min.y + 2.0),
                                    Vec2::splat(24.0),
                                ),
                                Color32::WHITE,
                            );
                        }
                        x += 30.0;
                    }
                    ui.painter().text(
                        Pos2::new(x, rect.center().y),
                        Align2::LEFT_CENTER,
                        &row.label,
                        palette::px(theme, 14.0),
                        row.color.unwrap_or(palette::GRAY_200),
                    );
                    if !row.detail.is_empty() {
                        ui.painter().text(
                            Pos2::new(rect.max.x - 8.0, rect.center().y),
                            Align2::RIGHT_CENTER,
                            &row.detail,
                            palette::px(theme, 12.0),
                            palette::GRAY_500,
                        );
                    }
                    if resp.clicked() {
                        outcome = SpotlightOutcome::Picked(row.key.clone());
                    }
                }
            });
        if results.is_empty() && !state.query.trim().is_empty() {
            ui.label(
                egui::RichText::new("No results")
                    .font(palette::px(theme, 14.0))
                    .color(palette::GRAY_500),
            );
        }
    });
    if resp.should_close() && outcome == SpotlightOutcome::Open {
        outcome = SpotlightOutcome::Closed;
    }
    outcome
}
