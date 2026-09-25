//! Keeping input away from whatever a modal dialog covers.
//!
//! `egui::Modal` blocks *widget* interaction below it (hit-testing, so
//! hover / click / drag responses, and keyboard focus) and nothing else.
//! Code that reads raw input (`ui.input(|i| i.pointer…)`, `key_pressed`,
//! `consume_key`, `i.events`) sees every event on whatever layer it draws,
//! and because the pages draw before the dialogs each frame it even runs
//! first: a click on a dialog selects the route-list row under it, an Enter
//! meant for the dialog's OK also fires the page's own Enter action. Every
//! raw read on a layer a dialog can cover asks [`behind_modal`] first and
//! stands down.

use egui::{Context, Id, LayerId, Ui};

/// Whether a modal dialog sits above `ui`'s layer.
///
/// This is the previous frame's modal (the one egui itself blocks widgets
/// with), so the frame whose input opens a dialog is not blocked yet and
/// the frame after one closes still is.
pub fn behind_modal(ui: &Ui) -> bool {
    layer_behind_modal(ui.ctx(), ui.layer_id())
}

/// [`behind_modal`] for code holding a context and the layer it draws on
/// (an `Area` it shows itself) rather than a `Ui`.
pub fn layer_behind_modal(ctx: &Context, layer: LayerId) -> bool {
    !ctx.memory(|m| m.is_above_modal_layer(layer))
}

/// Whether a popup (a menu-bar menu, a submenu, a combo box's list) was open
/// when this frame began.
///
/// Raw pointer reads need the same care under an open menu as under a
/// modal: the click that dismisses a menu (or picks one of its items over
/// the page) is still in the raw input, so a page that hit-tests it by hand
/// acts on it as well, e.g. selecting the route-list row under the menu.
/// The state is taken at the start of the frame because the menu bar draws
/// before the pages and may already have closed the menu on this very click
/// by the time a page asks.
///
/// The first call installs the frame-start hook, so a context reports
/// `false` until the frame after it first asks (nothing can be open yet on
/// a context's first frame anyway).
pub fn popup_open(ctx: &Context) -> bool {
    ctx.add_plugin(PopupTracker);
    ctx.data(|d| d.get_temp::<bool>(popup_open_id())).unwrap_or(false)
}

/// [`behind_modal`] or [`popup_open`]: whether a click on `ui`'s layer may
/// really be meant for a dialog or a menu above it.
pub fn pointer_blocked(ui: &Ui) -> bool {
    behind_modal(ui) || popup_open(ui.ctx())
}

fn popup_open_id() -> Id {
    Id::new("xpr_ui_kit_popup_open_at_frame_start")
}

/// Records [`egui::Popup::is_any_open`] before anything draws each pass.
/// `add_plugin` registers a given plugin type only once.
struct PopupTracker;

impl egui::Plugin for PopupTracker {
    fn debug_name(&self) -> &'static str {
        "xpr_popup_tracker"
    }

    fn on_begin_pass(&mut self, ctx: &Context) {
        let open = egui::Popup::is_any_open(ctx);
        ctx.data_mut(|d| d.insert_temp(popup_open_id(), open));
    }
}
