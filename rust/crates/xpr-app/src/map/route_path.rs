//! The route path overlay (`docs/rust_port/design/world_map/
//! GRAPHICS_TOOLS_PLAN.md` WP-E, gap G9). Superseded mid-implementation by
//! `docs/rust_port/design/world_map/SPEC.md` decision D15 and §13 "Trip
//! path overlay" (decided 2026-09-25): draws **one folder ("trip") at a
//! time**, never the whole route — the route format does not say how the
//! runner travelled between events (Fly / Dig / Teleport / Escape Rope /
//! boats / trains are not events, §13.1), so a line spanning two trips
//! would be wrong at almost every join. `state.rs::RouteMapState::sync`
//! resolves the trip (the folder selected in the route list, or the
//! nearest enclosing folder of the selected event) into `state.trip`; this
//! file only draws it and handles clicks on it.
//!
//! Phase 1 (§13.2, "straight segments first"): numbered discs on each
//! node's anchor, straight lines between consecutive discs that are in the
//! current scope, the selected event's disc brighter. No warp projection
//! (an indoor node is simply absent from a world-scope drawing until it
//! resolves; §13.2 "Scopes") and no least-steps pathfinding (§13.3) yet.
//!
//! Drawn by the viewer and by the export with an export camera — `draw`
//! only reads `oc.cam` / `oc.vp` / `oc.ppp` / `oc.pack` / `oc.scope` (plus
//! `oc.theme` for styling, which the export also supplies). `handle_click`
//! is viewer-only (a `&mut MapView`), called from `mod.rs::viewport`.

use egui::{Align2, Pos2, Rect, Stroke, Ui, Vec2};
use xpr_map::geom;
use xpr_ui_kit::theme;

use super::overlay::OverlayCtx;
use super::state::{RouteMapState, TripNode};
use super::{MapAction, MapView, Toggles};

/// Disc radius in screen px: bigger at high zoom, but clamped so it never
/// gets tiny far out or huge up close (plan WP-E: "radius clamp 7–12").
fn disc_radius(zoom: f32) -> f32 {
    (8.0 * zoom).clamp(7.0, 12.0)
}

/// The trip's nodes projected to screen space, keeping only the ones whose
/// anchor is in `oc.scope` (`geom::step_center_px` returns `None`
/// otherwise — an indoor node while looking at the world, or vice versa).
fn screen_nodes<'a>(oc: &OverlayCtx, nodes: &'a [TripNode]) -> Vec<(&'a TripNode, Pos2)> {
    nodes
        .iter()
        .filter_map(|n| {
            let (wx, wy) = geom::step_center_px(oc.pack, oc.scope, n.anchor.map, n.anchor.x as i32, n.anchor.y as i32)?;
            Some((n, oc.cam.world_to_screen(oc.vp, Vec2::new(wx as f32, wy as f32))))
        })
        .collect()
}

pub fn draw(painter: &egui::Painter, oc: &OverlayCtx, state: &RouteMapState, toggles: &Toggles) {
    if !toggles.path {
        return;
    }
    let Some(trip) = &state.trip else {
        // "no folder selected" and "this folder has nothing to show" both
        // leave `trip` at `None`; `trip_folder_name` tells them apart.
        let hint = if state.trip_folder_name.is_some() { "No map positions in this folder" } else { "Select a folder to see its path" };
        let pos = Pos2::new(oc.vp.min.x + 8.0, oc.vp.max.y - 8.0);
        painter.text(pos, Align2::LEFT_BOTTOM, hint, egui::FontId::proportional(12.0), theme::with_alpha(oc.theme.secondary, 220));
        return;
    };

    let points = screen_nodes(oc, &trip.nodes);
    if points.is_empty() {
        return;
    }

    // segments: straight lines between consecutive on-screen nodes only
    // (phase 1, §13.2/§13.3: no warp projection, no pathfinding)
    let line_stroke = Stroke::new(2.0, theme::with_alpha(oc.theme.accent, 153)); // ~60% alpha
    for pair in points.windows(2) {
        painter.line_segment([pair[0].1, pair[1].1], line_stroke);
    }

    let radius = disc_radius(oc.cam.zoom);
    let font = egui::FontId::monospace((radius * 0.9).clamp(8.0, 11.0));
    for (node, center) in &points {
        let selected = state.selected_id.map(|id| node.ids.contains(&id)).unwrap_or(false);
        let fill = if selected { oc.theme.accent } else { theme::with_alpha(oc.theme.bg_darker, 235) };
        let stroke_c = if selected { oc.theme.text_strong() } else { oc.theme.accent };
        painter.circle(*center, radius, fill, Stroke::new(if selected { 2.5 } else { 1.5 }, stroke_c));
        let label = if node.first == node.last { node.first.to_string() } else { format!("{}\u{2013}{}", node.first, node.last) };
        let text_c = if selected { oc.theme.bg } else { oc.theme.text_strong() };
        painter.text(*center, Align2::CENTER_CENTER, label, font.clone(), text_c);
    }
}

/// A click on a disc selects that node's first event (SPEC §13.2
/// "Interaction"). Called from `mod.rs::viewport` right before the
/// ordinary click block so a disc hit takes priority over opening a
/// map/tile card underneath it; returns true when a disc was hit.
pub fn handle_click(view: &mut MapView, ui: &Ui, resp: &egui::Response, vp: Rect, actions: &mut Vec<MapAction>) -> bool {
    if xpr_ui_kit::modal::behind_modal(ui) {
        return false;
    }
    if !view.toggles.path || !resp.clicked() {
        return false;
    }
    let Some(pos) = resp.interact_pointer_pos() else { return false };
    let Some(pack) = view.pack().cloned() else { return false };
    let Some(trip) = view.state.trip.clone() else { return false };
    let scope = view.scope();
    let radius = disc_radius(view.zoom());
    for node in &trip.nodes {
        let Some((wx, wy)) = geom::step_center_px(&pack, scope, node.anchor.map, node.anchor.x as i32, node.anchor.y as i32) else { continue };
        let center = view.camera.world_to_screen(vp, Vec2::new(wx as f32, wy as f32));
        if center.distance(pos) <= radius + 2.0 {
            if let Some(&id) = node.ids.first() {
                actions.push(MapAction::SelectEvent(id));
            }
            return true;
        }
    }
    false
}
