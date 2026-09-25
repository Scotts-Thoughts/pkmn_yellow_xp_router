//! The navigator minimap and the zoom presets menu
//! (`docs/rust_port/design/world_map/GRAPHICS_TOOLS_PLAN.md` WP-D, gaps
//! G7 / G8): a small panel in the viewport's top-right corner showing the
//! whole scope through its coarsest LOD chunk with the visible rectangle
//! outlined; click or drag moves the camera. The zoom readout of the toolbar
//! opens a menu of presets.

use egui::{Color32, LayerId, Pos2, Rect, Response, Stroke, Ui, Vec2};
use xpr_map::{geom, CHUNK_PX};
use xpr_ui_kit::modal::behind_modal;
use xpr_ui_kit::theme::{self, Theme};
use xpr_ui_kit::widgets;

use super::camera::Camera;
use super::chunks::ChunkKey;
use super::layers;
use super::MapView;

/// Panel width (px); height comes from the scope's aspect ratio, clamped
/// (§3.4 "D"). Inset from the viewport's top-right corner; `vp` already
/// starts below the toolbar, so "8 px, below the toolbar" is just an inset
/// from `vp`'s own top-right corner.
const PANEL_W: f32 = 200.0;
const PANEL_MIN_H: f32 = 60.0;
const PANEL_MAX_H: f32 = 160.0;
const PANEL_INSET: f32 = 8.0;
/// Gap between the panel's border and the chunk image / visible-rect outline.
const CONTENT_INSET: f32 = 2.0;

#[derive(Default)]
pub struct Navigator {
    /// The panel's screen rect from the last frame it was drawn (`None`
    /// when off or no pack is loaded). `handle_input` runs before `draw`
    /// each frame, so it reads last frame's geometry, same as the rest of
    /// the viewport's one-frame-stale layout (`MapView::last_vp`).
    rect: Option<Rect>,
    /// The panel's own `Area` layer, set by `draw`.
    layer: Option<LayerId>,
    /// A drag that began on the panel is still in progress, kept true even
    /// if the pointer strays outside the panel mid-drag (a minimap should
    /// keep tracking the drag, not drop it at the edge).
    dragging: bool,
}

impl Navigator {
    /// Pointer input. Returns true while the pointer is over the panel or a
    /// drag started on it: the map then ignores the pointer this frame.
    pub fn handle_input(view: &mut MapView, ui: &Ui, resp: &Response, _vp: Rect) -> bool {
        let _ = resp; // the navigator's hit region is smaller than the viewport `resp`; it reads the raw pointer itself (see below)
        if !view.toggles.navigator {
            view.navigator.dragging = false;
            return false;
        }
        // every raw read on the viewport stands down behind a modal dialog
        // (CLAUDE.md "raw input must respect modal dialogs"); `tests/map_modal_input.rs` covers this
        if behind_modal(ui) {
            return false;
        }
        let Some(rect) = view.navigator.rect else { return false };
        let Some(pack) = view.pack().cloned() else { return false };
        let scope = view.scope;
        let scope_r = geom::scope_rect(&pack, scope);
        if scope_r.is_empty() {
            return false;
        }

        let ctx = ui.ctx();
        let (pos, down, pressed, released) = ctx.input(|i| (i.pointer.interact_pos(), i.pointer.primary_down(), i.pointer.primary_pressed(), i.pointer.primary_released()));
        // a popup over the panel (a card, the zoom presets or layers menu)
        // gets the pointer, so a click on one of its buttons can't also move
        // the camera. Any other visible layer above the page that contains
        // the pointer counts, whatever its stacking order: `ctx.layer_id_at`
        // can't be used, since the panel's own area is paint-only and has no
        // size, so egui never reports it as the top layer
        let nav_layer = view.navigator.layer;
        let under_popup = |p: Pos2| {
            ctx.memory(|m| m.areas().visible_layer_ids().into_iter().any(|l| l.order > egui::Order::Background && Some(l) != nav_layer && m.area_rect(l.id).is_some_and(|r| r.contains(p))))
        };
        let over = pos.map(|p| rect.contains(p) && !under_popup(p)).unwrap_or(false);
        if released {
            view.navigator.dragging = false;
        }
        if pressed && over {
            view.navigator.dragging = true;
        }
        // while the button is down only a drag that began on the panel is
        // ours: a pan or marquee drag that merely crosses the panel keeps
        // going (review finding 1, 2026-09-25); hovering alone is ours so
        // the map does not hit-test markers under the panel
        let active = if down { view.navigator.dragging } else { over };
        if active && down {
            if let Some(p) = pos {
                let content = rect.shrink(CONTENT_INSET);
                let fx = ((p.x - content.min.x) / content.width().max(1.0)).clamp(0.0, 1.0);
                let fy = ((p.y - content.min.y) / content.height().max(1.0)).clamp(0.0, 1.0);
                let world = Vec2::new(scope_r.x0 as f32 + fx * scope_r.width() as f32, scope_r.y0 as f32 + fy * scope_r.height() as f32);
                view.camera.center_on(world, false);
                view.mark_view_dirty();
            }
        }
        active
    }

    /// Draw the panel (when `toggles.navigator` is on) after the map's own
    /// layers, cards and banner: its own `Order::Foreground` area, drawn
    /// after theirs in the same frame (`cards.rs`, the focus banner in
    /// `mod.rs::viewport`), so it paints on top of them, same as an open
    /// dialog paints on top of all three (it is drawn later still, as a
    /// wholly separate call after the map view; see CLAUDE.md).
    pub fn draw(view: &mut MapView, ui: &mut Ui, vp: Rect, theme: &Theme) {
        if !view.toggles.navigator {
            view.navigator.rect = None;
            return;
        }
        let Some(loaded) = view.loaded.as_ref() else {
            view.navigator.rect = None;
            return;
        };
        let pack = loaded.pack.clone();
        let scope = view.scope;
        let scope_r = geom::scope_rect(&pack, scope);
        if scope_r.is_empty() {
            view.navigator.rect = None;
            return;
        }
        let aspect = scope_r.height() as f32 / scope_r.width().max(1) as f32;
        let height = (PANEL_W * aspect).clamp(PANEL_MIN_H, PANEL_MAX_H);
        let rect = Rect::from_min_size(Pos2::new(vp.max.x - PANEL_INSET - PANEL_W, vp.min.y + PANEL_INSET), Vec2::new(PANEL_W, height));
        view.navigator.rect = Some(rect);

        // the coarsest chunk: one texture covers the whole scope (§3.4 "D")
        let lvl = layers::max_level(&pack, scope);
        let key = ChunkKey { scope, level: lvl, cx: 0, cy: 0, night: view.night };
        let span = (CHUNK_PX << lvl) as f32;
        // the chunk spans `span` world px from the origin; the scope may be smaller
        let uv_max = Pos2::new((scope_r.width() as f32 / span).min(1.0), (scope_r.height() as f32 / span).min(1.0));
        let tex_id = view.chunks.get(&key).map(|t| t.id());
        if tex_id.is_none() && !view.chunks.is_pending(&key) {
            // `get` returning None already means `!has(&key)`; only the
            // pending check is left to make before requesting
            view.chunks.request(vec![key]);
        }
        let visible = view.camera.visible_world(vp).intersect(&scope_r);

        let area = egui::Area::new(ui.id().with("map_navigator_panel")).order(egui::Order::Foreground).fixed_pos(rect.min);
        view.navigator.layer = Some(area.layer());
        area.show(ui.ctx(), |ui| {
            let painter = ui.painter();
            painter.rect(rect, 4.0, theme.card_bg(), theme.border_stroke(), egui::StrokeKind::Inside);
            let content = rect.shrink(CONTENT_INSET);
            if let Some(id) = tex_id {
                let mut mesh = egui::Mesh::with_texture(id);
                mesh.add_rect_with_uv(content, Rect::from_min_max(Pos2::ZERO, uv_max), Color32::WHITE);
                painter.add(egui::Shape::mesh(mesh));
            } else {
                painter.rect_filled(content, 2.0, Color32::from_rgb(10, 10, 24));
            }
            if !visible.is_empty() {
                let to_panel = |wx: i32, wy: i32| {
                    Pos2::new(
                        content.min.x + (wx - scope_r.x0) as f32 / scope_r.width() as f32 * content.width(),
                        content.min.y + (wy - scope_r.y0) as f32 / scope_r.height() as f32 * content.height(),
                    )
                };
                let outline = Rect::from_min_max(to_panel(visible.x0, visible.y0), to_panel(visible.x1, visible.y1)).intersect(content);
                painter.rect_filled(outline, 0.0, theme::with_alpha(theme.accent, 40));
                painter.rect_stroke(outline, 0.0, Stroke::new(1.5, theme.accent), egui::StrokeKind::Inside);
            }
        });
    }

    /// Zoom-preset keyboard shortcuts (G8): `1` = 100 %. Called each frame
    /// while no text field has focus and no modal is up (`mod.rs`'s key
    /// block, right after `keynav::handle_keys`, same guard).
    pub fn handle_keys(view: &mut MapView, ui: &Ui) {
        if ui.ctx().input(|i| i.key_pressed(egui::Key::Num1)) {
            let vp = view.last_vp;
            view.camera.set_zoom_centered(vp, 1.0);
            view.mark_view_dirty();
        }
    }
}

impl MapView {
    /// The navigator panel's screen rect from the last frame it was drawn
    /// (`None` when off, or no pack loaded, or the scope is empty). Test
    /// accessor (`tests/map_navigator.rs`).
    pub fn navigator_rect(&self) -> Option<Rect> {
        self.navigator.rect
    }

    /// The camera's world-space centre. Test accessor.
    pub fn camera_center(&self) -> Vec2 {
        self.camera.center
    }
}

/// Zoom presets offered by the readout's popup, in menu order (G8): 25 …
/// 800 %; "1:1" and "Fit" are drawn as their own rows below a separator.
const ZOOM_PRESETS: [f32; 6] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0];

/// A popup row: hover highlight + left-aligned text, `MapList::popup`'s
/// per-item look (`list.rs`) rather than `widgets::menu_item`, which calls
/// `Ui::close()` expecting a real `menu_button`/`Popup` parent (this popup
/// is a plain `egui::Area`, like `MapList::popup`) and would just log a
/// "no closable parent" warning here.
fn popup_row(ui: &mut Ui, theme: &Theme, text: &str) -> bool {
    let r = ui.allocate_response(Vec2::new(ui.available_width(), 20.0), egui::Sense::click());
    if r.hovered() {
        ui.painter().rect_filled(r.rect, 3.0, theme.hover_bg);
    }
    ui.painter().text(Pos2::new(r.rect.min.x + 8.0, r.rect.center().y), egui::Align2::LEFT_CENTER, text, theme.body(), theme.text);
    r.clicked()
}

/// The zoom readout of the toolbar; a menu of presets (25 … 800 %, 1:1,
/// Fit). Returns true when the zoom changed.
///
/// Design note (G8): the WP-D contract fixes this function's signature to
/// `(ui, theme, cam, vp)` with no `MapView` / pack / scope, so "Fit" can't
/// compute the scope rect here the way `viewport()` does. `Camera` now
/// records the rect it was last asked to fit-zoom against
/// (`Camera::last_scope`, set by `set_min_zoom_for`, which `viewport()`
/// calls every frame); "Fit" reuses that rather than widening this
/// function's signature or threading a new field through `MapView`. The
/// popup's own open/closed state (this function gets no `&mut Navigator`
/// either) lives in egui's per-`Id` memory, the same mechanism `ComboBox`
/// and friends use for exactly this.
pub fn zoom_control(ui: &mut Ui, theme: &Theme, cam: &mut Camera, vp: Rect) -> bool {
    let open_id = egui::Id::new("xpr_map_zoom_menu_open");
    let mut open = ui.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false);
    let btn = widgets::StyledButton::new(theme, format!("{:.0}%", cam.zoom * 100.0)).min_size(Vec2::new(46.0, 22.0)).checked(open).show(ui);
    if btn.clicked() {
        open = !open;
    }

    let mut changed = false;
    if open {
        let pos = Pos2::new(btn.rect.min.x, btn.rect.max.y + 2.0);
        let area = egui::Area::new(ui.id().with("xpr_map_zoom_menu")).order(egui::Order::Foreground).fixed_pos(pos);
        let mut chosen: Option<f32> = None;
        let mut fit = false;
        let inner = area.show(ui.ctx(), |ui| {
            egui::Frame::new().fill(theme.card_bg()).stroke(theme.border_stroke()).corner_radius(6.0).inner_margin(6.0).show(ui, |ui| {
                ui.set_width(64.0);
                for z in ZOOM_PRESETS {
                    if popup_row(ui, theme, &format!("{:.0}%", z * 100.0)) {
                        chosen = Some(z);
                    }
                }
                widgets::menu_separator(ui, theme);
                if popup_row(ui, theme, "1:1") {
                    chosen = Some(1.0);
                }
                if popup_row(ui, theme, "Fit") {
                    fit = true;
                }
            });
        });
        if let Some(z) = chosen {
            cam.set_zoom_centered(vp, z);
            changed = true;
        } else if fit {
            cam.fit(vp, cam.last_scope, true);
            changed = true;
        }
        // click elsewhere closes the popup (not a click on a dialog above the page); `list.rs::MapList::popup`'s pattern
        if !behind_modal(ui) {
            let clicked_outside = ui.input(|i| i.pointer.any_pressed())
                && ui.input(|i| i.pointer.interact_pos()).map(|p| !inner.response.rect.contains(p) && !btn.rect.contains(p)).unwrap_or(false);
            if clicked_outside || chosen.is_some() || fit || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                open = false;
            }
        }
    }
    ui.data_mut(|d| d.insert_temp(open_id, open));
    changed
}
