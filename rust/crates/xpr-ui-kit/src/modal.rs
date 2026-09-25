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

use egui::{Context, LayerId, Ui};

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
