//! Selection and measurement tools — the marquee and the ruler
//! (`docs/rust_port/design/world_map/GRAPHICS_TOOLS_PLAN.md` WP-C, gaps
//! G3 / G4 / G13). `Tool::Pan` is the hand tool the map always had;
//! `Select` drags a step-aligned rectangle in world pixels; `Measure` drags
//! a line and reports the step distance.
//!
//! `draw` only gets `&self`, a `Painter` and an `OverlayCtx` — no
//! `&mut MapView`, because the export path (`map/export.rs`) calls the
//! exact same function with an export camera to bake the selection outline
//! into a rendered image, and must never pick up interactive chrome. So
//! `draw` stays a pure renderer (the selection / measurement shapes only);
//! the floating toolbar under a selection is drawn by `handle_input`
//! instead, which *is* export-exclusive (export never calls it) and
//! *does* get `&mut MapView`, so its buttons apply immediately, no queuing
//! needed. "Add trainers" is the one button whose result other code needs
//! to see: it goes through `pending_actions`, drained by `mod.rs` via
//! `take_actions`.

use egui::{Pos2, Rect, Response, Ui, Vec2};
use xpr_map::{geom, IRect, MapPack, ObjectKind};
use xpr_ui_kit::modal::behind_modal;
use xpr_ui_kit::theme::{self, Theme};
use xpr_ui_kit::widgets;

use super::export::ExportRegion;
use super::overlay::OverlayCtx;
use super::{MapAction, MapView};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Pan,
    Select,
    Measure,
}

/// A marquee selection: world pixels of the current scope, aligned to steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub rect: IRect,
}

/// The last ruler measurement: two step-centre world points (`Tool::Measure`
/// endpoints are snapped to centres, not grown outward like `Selection`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Measurement {
    a: (i32, i32),
    b: (i32, i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DragMode {
    Select,
    Measure,
}

/// A press-to-release drag `handle_input` owns: world px, unsnapped (snapped
/// only when drawn / finalized, so the maths lives in one place).
#[derive(Clone, Copy, Debug)]
struct ActiveDrag {
    mode: DragMode,
    start: (i32, i32),
    current: (i32, i32),
}

#[derive(Default)]
pub struct ToolState {
    pub tool: Tool,
    pub selection: Option<Selection>,
    measurement: Option<Measurement>,
    active: Option<ActiveDrag>,
    /// Actions the floating toolbar queued ("Add trainers"); drained once a
    /// frame by `mod.rs`'s `actions.extend(self.tools.take_actions())`.
    pending_actions: Vec<MapAction>,
}

impl ToolState {
    /// The tool buttons of the map toolbar: H / M / R, the active one
    /// pressed-looking (`StyledButton::checked`).
    pub fn toolbar(&mut self, ui: &mut Ui, theme: &Theme) {
        for (tool, label, tip) in [
            (Tool::Pan, "H", "Hand — pan (H)"),
            (Tool::Select, "M", "Marquee — select a region (M)"),
            (Tool::Measure, "R", "Ruler — measure steps (R)"),
        ] {
            let r = widgets::StyledButton::new(theme, label).checked(self.tool == tool).min_size(Vec2::new(24.0, 22.0)).show(ui).on_hover_text(tip);
            if r.clicked() {
                self.tool = tool;
            }
        }
    }

    /// Single-letter tool keys (H / M / R). Called only while no text field
    /// has focus and no modal dialog is up (the caller in `mod.rs`
    /// guarantees both; see CLAUDE.md "raw input must respect modal
    /// dialogs").
    pub fn handle_keys(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            if i.key_pressed(egui::Key::H) {
                self.tool = Tool::Pan;
            } else if i.key_pressed(egui::Key::M) {
                self.tool = Tool::Select;
            } else if i.key_pressed(egui::Key::R) {
                self.tool = Tool::Measure;
            }
        });
    }

    /// Pointer input on the viewport. Returns true when the active tool used
    /// the press / drag / release, in which case the map neither pans nor
    /// hit-tests markers this frame.
    ///
    /// `resp` is a widget `Response` (from `ui.allocate_rect`), so its
    /// `dragged` / `drag_started` / `drag_stopped` / `clicked` are already
    /// blocked by an open `egui::Modal` the way `mod.rs`'s own pan / click
    /// handling is (CLAUDE.md "raw input must respect modal dialogs": a
    /// modal blocks *widgets* automatically). Only the raw pointer / key
    /// reads below it (`press_origin`, the Shift / Space modifiers) need the
    /// explicit `behind_modal` guard, and only for *starting* something new
    /// — a drag already owned (`active.is_some()`) keeps running so it can
    /// be released cleanly even if a modal opens mid-drag.
    pub fn handle_input(view: &mut MapView, ui: &Ui, resp: &Response, vp: Rect) -> bool {
        let already_ours = view.tools.active.is_some();
        if behind_modal(ui) && !already_ours {
            return false;
        }

        let Some(pack) = view.pack().cloned() else { return false };
        let step = pack.geom.step_px.max(1) as i32;
        let scope_rect = geom::scope_rect(&pack, view.scope);
        let mut consumed = false;

        if resp.drag_started() {
            let space = ui.input(|i| i.key_down(egui::Key::Space));
            let shift = ui.input(|i| i.modifiers.shift);
            // Space (temporary hand tool) always wins; Shift is the
            // photoshop-style "marquee from any tool" shortcut; otherwise
            // the current tool decides. Pan (no modifier) isn't ours: fall
            // through so the map's own drag-to-pan handles it.
            let mode = if space {
                None
            } else if shift {
                Some(DragMode::Select)
            } else {
                match view.tools.tool {
                    Tool::Pan => None,
                    Tool::Select => Some(DragMode::Select),
                    Tool::Measure => Some(DragMode::Measure),
                }
            };
            if let (Some(mode), Some(start_screen)) = (mode, ui.input(|i| i.pointer.press_origin())) {
                let w = view.camera.screen_to_world(vp, start_screen);
                let start = (w.x.round() as i32, w.y.round() as i32);
                view.tools.active = Some(ActiveDrag { mode, start, current: start });
            }
        }

        if view.tools.active.is_some() {
            if resp.dragged() || resp.drag_stopped() {
                let prev = view.tools.active.unwrap();
                let fallback = view.camera.world_to_screen(vp, Vec2::new(prev.current.0 as f32, prev.current.1 as f32));
                let cur_screen = resp.interact_pointer_pos().unwrap_or(fallback);
                let w = view.camera.screen_to_world(vp, cur_screen);
                let current = (w.x.round() as i32, w.y.round() as i32);
                if let Some(a) = view.tools.active.as_mut() {
                    a.current = current;
                }
                consumed = true;
            }
            if resp.drag_stopped() {
                if let Some(active) = view.tools.active.take() {
                    match active.mode {
                        DragMode::Select => {
                            let rect = snap_rect(step, scope_rect, active.start, active.current);
                            if !rect.is_empty() {
                                view.tools.selection = Some(Selection { rect });
                            }
                        }
                        DragMode::Measure => {
                            view.tools.measurement = Some(Measurement { a: snap_point(step, active.start), b: snap_point(step, active.current) });
                        }
                    }
                }
                consumed = true;
            }
        } else if resp.clicked() && view.tools.tool == Tool::Select {
            // a plain click (no drag) in Select clears the selection
            view.tools.selection = None;
            consumed = true;
        }

        // the floating toolbar under a finalized selection: not while
        // still dragging (the rect keeps changing) and never behind a
        // modal (real widgets, so egui already blocks their clicks there,
        // but there's no reason to lay the popup out at all)
        if view.tools.active.is_none() && !behind_modal(ui) {
            if let Some(sel) = view.tools.selection {
                floating_toolbar(view, ui, vp, sel);
            }
        }

        consumed
    }

    /// The selection rectangle / measurement line over the map (also drawn
    /// into exports through the export camera's `OverlayCtx` — see the
    /// module doc on why this stays non-interactive). While a drag is in
    /// progress its live shape replaces whatever was finalized before (SPEC
    /// WP-C: "the last measurement stays until Escape or a new drag").
    pub fn draw(&self, painter: &egui::Painter, oc: &OverlayCtx) {
        let step = oc.pack.geom.step_px.max(1) as i32;
        match self.active {
            Some(ActiveDrag { mode: DragMode::Select, start, current }) => {
                let scope_rect = geom::scope_rect(oc.pack, oc.scope);
                let rect = snap_rect(step, scope_rect, start, current);
                if !rect.is_empty() {
                    paint_selection(painter, oc, rect, true);
                }
            }
            Some(ActiveDrag { mode: DragMode::Measure, start, current }) => {
                paint_measurement(painter, oc, snap_point(step, start), snap_point(step, current));
            }
            None => {
                if let Some(sel) = self.selection {
                    paint_selection(painter, oc, sel.rect, false);
                }
                if let Some(m) = self.measurement {
                    paint_measurement(painter, oc, m.a, m.b);
                }
            }
        }
    }

    /// What the status strip shows while a selection or measurement exists.
    pub fn status_text(&self, pack: &MapPack) -> Option<String> {
        let step = pack.geom.step_px.max(1) as i32;
        if let Some(sel) = self.selection {
            return Some(selection_text(sel.rect, step));
        }
        if let Some(m) = self.measurement {
            return Some(measurement_text(m.a, m.b, step));
        }
        None
    }

    /// Escape: clear the selection / measurement. Returns true when there
    /// was something to clear (the card and the focus banner then stay).
    /// An in-progress drag is cancelled first, then the measurement, then
    /// the selection — one thing per press.
    pub fn on_escape(&mut self) -> bool {
        if self.active.take().is_some() {
            return true;
        }
        if self.measurement.take().is_some() {
            return true;
        }
        self.selection.take().is_some()
    }

    /// Drain the actions the floating toolbar queued this frame (`mod.rs`
    /// calls this right after the draw block: `actions.extend(self.tools.take_actions())`).
    pub fn take_actions(&mut self) -> Vec<MapAction> {
        std::mem::take(&mut self.pending_actions)
    }

    /// What the floating toolbar's "Add trainers" button pushes (also
    /// called directly by `tests/map_tools.rs`, "call the same function the
    /// button calls"): the selection's linked, not-yet-fought trainers, in
    /// object order, in a folder named after the map under the selection's
    /// centre (`MapView::trainers_in_rect`). `None` when there is nothing
    /// to add.
    pub fn add_trainers_action(view: &MapView, sel: Selection) -> Option<MapAction> {
        let (folder, names) = view.trainers_in_rect(sel.rect);
        if names.is_empty() {
            None
        } else {
            Some(MapAction::AddTrainersNamed { folder, names })
        }
    }
}

/// Zoom / Export / Copy / Add trainers / ✕, below the selection and clamped
/// inside the viewport (`egui::Area`, `Order::Foreground`, the pattern of
/// the focus banner in `mod.rs`). Lives here rather than in `draw` because
/// it needs `&mut MapView` (Zoom drives the camera directly; Export / Copy
/// call `view.export`) and `Theme`-free styling: there is no `Theme` to
/// hand it (the `handle_input` contract has none), so it uses plain egui
/// widgets, which already pick up the app's theme because `Theme::apply`
/// configures it globally on `egui::Context` (`xpr-ui-kit/src/theme.rs`:
/// button fills/strokes per widget state, `hyperlink_color` = the accent),
/// not through per-widget calls the way `StyledButton` does.
fn floating_toolbar(view: &mut MapView, ui: &Ui, vp: Rect, sel: Selection) {
    let ctx = ui.ctx().clone();
    let a = view.camera.world_to_screen(vp, Vec2::new(sel.rect.x0 as f32, sel.rect.y0 as f32));
    let b = view.camera.world_to_screen(vp, Vec2::new(sel.rect.x1 as f32, sel.rect.y1 as f32));
    let screen = Rect::from_two_pos(a, b);
    let est = Vec2::new(320.0, 30.0);
    let x = screen.min.x.clamp(vp.min.x + 4.0, (vp.max.x - est.x - 4.0).max(vp.min.x + 4.0));
    let below = screen.max.y + 6.0;
    let y = if below + est.y <= vp.max.y { below } else { (screen.min.y - est.y - 6.0).max(vp.min.y + 4.0) };

    let (mut zoom, mut export, mut copy, mut add, mut clear) = (false, false, false, false, false);
    egui::Area::new(egui::Id::new("xpr_map_tools_selection_bar")).order(egui::Order::Foreground).fixed_pos(Pos2::new(x, y)).show(&ctx, |ui| {
        egui::Frame::popup(ui.style()).corner_radius(6.0).inner_margin(egui::Margin::symmetric(8, 4)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                zoom = ui.button("Zoom").on_hover_text("Zoom to the selection").clicked();
                export = ui.button("Export").on_hover_text("Export the selection as a PNG").clicked();
                copy = ui.button("Copy").on_hover_text("Copy the selection to the clipboard").clicked();
                add = ui.button("Add trainers").on_hover_text("Add the selection's undefeated trainers to the route").clicked();
                clear = ui.button("✕").on_hover_text("Clear the selection").clicked();
            });
        });
    });

    if zoom {
        view.camera.fit(vp, sel.rect, true);
        view.mark_view_dirty();
    }
    if export {
        view.export.request_open_with(ExportRegion::Selection);
    }
    if copy {
        view.export.request_copy(ExportRegion::Selection);
    }
    if add {
        if let Some(action) = ToolState::add_trainers_action(view, sel) {
            view.tools.pending_actions.push(action);
        }
    }
    if clear {
        view.tools.selection = None;
    }
}

impl MapView {
    /// The map under `rect`'s centre (its display name, or "Selection" when
    /// none is found) and the linked, not-yet-fought trainers whose object
    /// step lies inside `rect` (world px), in object order — the marquee
    /// selection's "Add trainers" (GRAPHICS_TOOLS_PLAN.md WP-C, gap G3).
    /// Scans every object rather than just one map's so a selection
    /// spanning more than one placed map (world scope) still works; objects
    /// of a different map than the current scope simply have no world
    /// position (`geom::object_center_px` returns `None`) and are skipped.
    fn trainers_in_rect(&self, rect: IRect) -> (String, Vec<String>) {
        let mut names: Vec<String> = Vec::new();
        if let Some(pack) = self.pack() {
            for (idx, o) in pack.objects.iter().enumerate() {
                if o.effective_kind() != ObjectKind::Trainer {
                    continue;
                }
                let Some((wx, wy)) = geom::object_center_px(pack, self.scope, idx as u32) else { continue };
                if !rect.contains(wx, wy) {
                    continue;
                }
                if let Some(n) = o.trainer_names().first() {
                    if !self.state.is_defeated(n) && !names.contains(n) {
                        names.push(n.clone());
                    }
                }
            }
        }
        let cx = (rect.x0 + rect.x1) / 2;
        let cy = (rect.y0 + rect.y1) / 2;
        let folder = self
            .pack()
            .and_then(|p| geom::map_at_world_px(p, self.scope, cx, cy))
            .map(|(m, _, _)| self.map_display_name(m))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Selection".to_string());
        (folder, names)
    }
}

// ---------------------------------------------------------------------------
// geometry: step-outward selection rects, step-centre measurement points
// ---------------------------------------------------------------------------

fn floor_to(v: i32, step: i32) -> i32 {
    v.div_euclid(step) * step
}

fn ceil_to(v: i32, step: i32) -> i32 {
    -((-v).div_euclid(step) * step)
}

/// The drag from `a` to `b` (world px) as a step-aligned rect, grown
/// outward to the nearest step boundary on each side, clipped to `scope`.
fn snap_rect(step: i32, scope: IRect, a: (i32, i32), b: (i32, i32)) -> IRect {
    let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
    let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
    let rect = IRect::new(floor_to(x0, step), floor_to(y0, step), ceil_to(x1, step), ceil_to(y1, step));
    rect.intersect(&scope)
}

/// A world-px point snapped to the centre of its step cell (ruler endpoints:
/// "step-snapped centres", not grown outward like a selection).
fn snap_point(step: i32, p: (i32, i32)) -> (i32, i32) {
    let c = |v: i32| v.div_euclid(step) * step + step / 2;
    (c(p.0), c(p.1))
}

// ---------------------------------------------------------------------------
// text shared by the status strip and the on-map badges
// ---------------------------------------------------------------------------

fn selection_text(rect: IRect, step: i32) -> String {
    let sw = (rect.width() / step).max(0);
    let sh = (rect.height() / step).max(0);
    format!("Selection: {} × {} steps ({} × {} px)", sw, sh, rect.width(), rect.height())
}

fn measurement_text(a: (i32, i32), b: (i32, i32), step: i32) -> String {
    let dx = (b.0 - a.0).abs() / step.max(1);
    let dy = (b.1 - a.1).abs() / step.max(1);
    format!("Δ {} × {} · {} steps", dx, dy, dx + dy)
}

// ---------------------------------------------------------------------------
// drawing: marching ants + fill, the measurement line + badge
// ---------------------------------------------------------------------------

/// The selection as marching ants (`Shape::dashed_line`) plus a subtle fill;
/// `live` (still being dragged) dims the fill a little more.
fn paint_selection(painter: &egui::Painter, oc: &OverlayCtx, rect: IRect, live: bool) {
    if rect.is_empty() {
        return;
    }
    let a = oc.cam.world_to_screen(oc.vp, Vec2::new(rect.x0 as f32, rect.y0 as f32));
    let b = oc.cam.world_to_screen(oc.vp, Vec2::new(rect.x1 as f32, rect.y1 as f32));
    let r = Rect::from_two_pos(a, b);
    let accent = oc.theme.accent;
    painter.rect_filled(r, 0.0, theme::with_alpha(accent, if live { 0x14 } else { 0x26 }));
    let pts = vec![r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    painter.extend(egui::Shape::dashed_line(&pts, egui::Stroke::new(1.5, accent), 6.0, 4.0));
}

/// The ruler as a line with end caps and a "Δ 12 × 5 · 17 steps" badge at
/// its midpoint.
fn paint_measurement(painter: &egui::Painter, oc: &OverlayCtx, a: (i32, i32), b: (i32, i32)) {
    let pa = oc.cam.world_to_screen(oc.vp, Vec2::new(a.0 as f32, a.1 as f32));
    let pb = oc.cam.world_to_screen(oc.vp, Vec2::new(b.0 as f32, b.1 as f32));
    let accent = oc.theme.accent;
    painter.line_segment([pa, pb], egui::Stroke::new(2.0, accent));
    for p in [pa, pb] {
        painter.circle_filled(p, 3.5, accent);
        painter.circle_stroke(p, 3.5, egui::Stroke::new(1.0, egui::Color32::WHITE));
    }
    let step = oc.pack.geom.step_px.max(1) as i32;
    let label = measurement_text(a, b, step);
    let mid = Pos2::new((pa.x + pb.x) / 2.0, (pa.y + pb.y) / 2.0);
    let galley = painter.layout_no_wrap(label, oc.theme.caption_font_bold(), egui::Color32::WHITE);
    let pad = Vec2::new(6.0, 3.0);
    let badge = Rect::from_center_size(mid, galley.size() + pad * 2.0);
    painter.rect_filled(badge, 4.0, theme::with_alpha(egui::Color32::BLACK, 0xC0));
    painter.rect_stroke(badge, 4.0, egui::Stroke::new(1.0, accent), egui::StrokeKind::Outside);
    painter.galley(badge.min + pad, galley, egui::Color32::WHITE);
}
