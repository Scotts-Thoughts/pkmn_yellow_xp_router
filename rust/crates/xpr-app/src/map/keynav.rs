//! Keyboard marker navigation and follow-selection
//! (`docs/rust_port/design/world_map/GRAPHICS_TOOLS_PLAN.md` WP-E, gaps
//! G11 / G12): Tab / Shift+Tab step through the visible markers of the map
//! under the viewport, Enter opens the card; with `toggles.follow` the
//! camera recentres on the selected route event whenever the selection
//! changes.

use egui::{Rect, Ui, Vec2};
use xpr_map::geom;

use super::cards::Card;
use super::MapView;

/// `egui::Memory`'s own focus system treats Tab as "give focus to the first
/// widget that wants it" whenever nothing is focused yet
/// (`Focus::begin_pass`/`interested_in_focus`, evaluated from the raw frame
/// input before any of this module's code runs). Since the toolbar's
/// search box is drawn before the viewport, an ordinary Tab press with
/// nothing focused hands it keyboard focus before `handle_keys` ever gets
/// called — `consume_key` inside `handle_keys` cannot prevent that grab
/// (egui's focus system does not consume the event to do it; see
/// `filtered_events` in egui's `TextEdit`, which clones rather than
/// removes). Called once per frame from `mod.rs::viewport`, right before it
/// computes `text_focused`, so Tab still reaches keyboard marker navigation
/// (gap G11) while the pointer is over the map: give any focus a bare Tab
/// just grabbed straight back. `behind_modal` guarded like every other raw
/// input read here, so a dialog's own Tab-focus traversal is untouched.
pub fn reclaim_hover_tab_focus(ui: &Ui, hovering: bool) {
    if !hovering || xpr_ui_kit::modal::behind_modal(ui) || !ui.input(|i| i.key_pressed(egui::Key::Tab)) {
        return;
    }
    if let Some(id) = ui.ctx().memory(|m| m.focused()) {
        ui.ctx().memory_mut(|m| m.surrender_focus(id));
    }
}

/// Called each frame while no text field has focus and no modal is up
/// (the caller already checked both; `behind_modal` is re-checked here
/// per the "every raw input read" rule).
pub fn handle_keys(view: &mut MapView, ui: &Ui, vp: Rect) {
    if xpr_ui_kit::modal::behind_modal(ui) {
        return;
    }
    // Shift+Tab must be consumed before plain Tab: `consume_key` matches
    // modifiers with `matches_logically`, which ignores an *extra* Shift,
    // so checking plain Tab first would also eat Shift+Tab events (egui's
    // own advice: check the more specific shortcut first). Tab is consumed
    // either way so egui's own widget-focus traversal never sees it.
    let (back, fwd) = ui.input_mut(|i| (i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab), i.consume_key(egui::Modifiers::NONE, egui::Key::Tab)));
    if back || fwd {
        step_selection(view, vp, fwd);
    }
    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        open_selected_card(view);
    }
}

/// Move `view.selected` to the next (or, `forward == false`, previous)
/// object among the current scope's visible, toggled-on objects, ordered
/// by distance from the viewport centre then by object index, wrapping.
fn step_selection(view: &mut MapView, vp: Rect, forward: bool) {
    let Some(pack) = view.pack().cloned() else { return };
    let scope = view.scope();
    let toggles = view.toggles;
    let vis = view.camera.visible_world(vp);
    let cx = (vis.x0 + vis.x1) / 2;
    let cy = (vis.y0 + vis.y1) / 2;

    let mut ordered: Vec<(u32, i64)> = Vec::new();
    for idx in 0..pack.objects.len() as u32 {
        let o = &pack.objects[idx as usize];
        if !toggles.visible(o.effective_kind()) {
            continue;
        }
        // `object_center_px` is `None` when the object's map is not part of
        // `scope` (an indoor object in `Scope::World`, or an object of a
        // different map in `Scope::Map`), so this doubles as the scope filter.
        let Some((wx, wy)) = geom::object_center_px(&pack, scope, idx) else { continue };
        if !vis.contains(wx, wy) {
            continue;
        }
        let (dx, dy) = ((wx - cx) as i64, (wy - cy) as i64);
        ordered.push((idx, dx * dx + dy * dy));
    }
    if ordered.is_empty() {
        return;
    }
    ordered.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));

    let len = ordered.len();
    let next = match view.selected.and_then(|cur| ordered.iter().position(|&(i, _)| i == cur)) {
        Some(pos) if forward => (pos + 1) % len,
        Some(pos) => (pos + len - 1) % len,
        // nothing selected, or the old selection fell out of view: start
        // from the nearest object to the viewport centre either way
        None => 0,
    };
    view.selected = Some(ordered[next].0);
    // the newly selected object is always inside `vis` by construction, so
    // there is nothing to pan to (plan WP-E: "it will not be, given the filter")
}

/// Enter: open the selected object's card and clear any focus banner (the
/// selection ring keeps drawing for `view.selected` either way).
pub(super) fn open_selected_card(view: &mut MapView) {
    let Some(idx) = view.selected else { return };
    let Some(pack) = view.pack().cloned() else { return };
    let scope = view.scope();
    let Some((wx, wy)) = geom::object_center_px(&pack, scope, idx) else { return };
    view.card = Some((Card::Object { idx }, Vec2::new(wx as f32, wy as f32)));
    view.focus = None;
}

/// Called after `RouteMapState::sync` (route or selection changed): with
/// `toggles.follow` on, recentre the camera on the newly selected route
/// event's first anchor, without a focus banner.
pub fn on_route_synced(view: &mut MapView) {
    if !view.toggles.follow || view.state.selected_anchors.is_empty() {
        return;
    }
    if view.state.selected_id.is_none() || view.state.selected_id == view.state.last_followed {
        return;
    }
    let Some(pack) = view.pack().cloned() else { return };
    let anchor = view.state.selected_anchors[0].clone();
    let scope = MapView::scope_of_map(&pack, anchor.map);
    view.set_scope(scope);
    if let Some((wx, wy)) = geom::step_center_px(&pack, scope, anchor.map, anchor.x as i32, anchor.y as i32) {
        let center = Vec2::new(wx as f32, wy as f32);
        if view.camera.zoom < 1.5 {
            // bring a too-far-out view to a readable zoom; otherwise leave it alone
            view.camera.go_to(center, 1.5, true);
        } else {
            view.camera.center_on(center, true);
        }
    }
    view.state.last_followed = view.state.selected_id;
    view.mark_view_dirty();
}

impl MapView {
    /// The object whose marker is ringed as "selected" (Tab/Shift+Tab move
    /// this; Enter opens its card).
    pub fn selected_object(&self) -> Option<u32> {
        self.selected
    }
}
