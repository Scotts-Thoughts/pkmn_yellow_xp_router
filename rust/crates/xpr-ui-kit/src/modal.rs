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

/// Horizontal padding of a dialog's title strip and body.
pub const DIALOG_PAD_X: f32 = 20.0;

/// Draw a modal dialog `width` points wide with `title` in its title strip;
/// `content` draws the body. Every dialog of the app (`xpr-app`'s
/// `dialogs::modal`) and of the Dex page goes through this, so they share
/// one look and egui's modal input blocking.
pub fn modal<R>(ctx: &Context, theme: &crate::theme::Theme, id: &str, title: &str, width: f32, content: impl FnOnce(&mut Ui) -> R) -> R {
    use egui::{Color32, CornerRadius, Stroke, Vec2};
    let frame = egui::Frame::new()
        .fill(theme.section_bg())
        .stroke(Stroke::new(1.0_f32, crate::theme::lighten(theme.bg, 0.14)))
        .corner_radius(CornerRadius::same(10))
        .shadow(egui::Shadow { offset: [0, 12], blur: 40, spread: 0, color: Color32::from_black_alpha(150) });
    let m = egui::Modal::new(Id::new(id)).frame(frame).backdrop_color(Color32::from_black_alpha(150));
    m.show(ctx, |ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let pad = DIALOG_PAD_X as i8;
        egui::Frame::new().inner_margin(egui::Margin { left: pad, right: pad, top: 15, bottom: 13 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add(egui::Label::new(egui::RichText::new(title).font(theme.font_bold(12.0)).color(theme.text_strong())).wrap());
        });
        let r = ui.max_rect();
        let y = ui.cursor().min.y;
        ui.painter().hline(r.x_range(), y + 0.5, Stroke::new(1.0_f32, theme.pane_divider()));
        ui.add_space(1.0);
        egui::Frame::new()
            .inner_margin(egui::Margin { left: pad, right: pad, top: 16, bottom: 16 })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                content(ui)
            })
            .inner
    })
    .inner
}
