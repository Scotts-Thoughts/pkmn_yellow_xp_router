//! The toolbar's "Layers" popup (`docs/rust_port/design/world_map/
//! GRAPHICS_TOOLS_PLAN.md` WP-C "Toolbar consolidation"): the 8 object
//! chips plus the 5 WP-C overlay / panel chips (grid, labels, route path,
//! navigator, follow), replaced by one button and a checkbox list grouped
//! "Objects" / "Overlays" / "Panels", because the docked pane's toolbar had
//! run out of room for one-letter chips per toggle.
//!
//! `MapView` gets no extra field for the popup's open/closed state (the
//! `mod.rs` scaffold's rule: new `MapView` fields don't come from this
//! package) — it lives in `egui`'s own per-`Id` temp storage instead, the
//! same place a `CollapsingHeader` or `ComboBox` keeps that kind of bit.
//! Everything else follows `map/list.rs::MapList::popup`: an `egui::Area`
//! under the button, closed by a click outside / Escape, both guarded by
//! `xpr_ui_kit::modal::behind_modal` (CLAUDE.md "raw input must respect
//! modal dialogs" — `ui.input(|i| i.pointer…)` / `key_pressed` are raw
//! reads, not blocked by a modal on their own).

use egui::{Pos2, Ui};
use xpr_core::Config;
use xpr_ui_kit::modal::behind_modal;
use xpr_ui_kit::theme::Theme;
use xpr_ui_kit::widgets;

use super::Toggles;

fn popup_open_id(ui: &Ui) -> egui::Id {
    ui.id().with("xpr_map_layers_menu_open")
}

/// The "Layers" button and, while open, its popup. Persists through `cfg`
/// on any change, exactly like the chip row it replaces did.
pub fn show(ui: &mut Ui, theme: &Theme, toggles: &mut Toggles, cfg: &mut Config) {
    let open_id = popup_open_id(ui);
    let mut open = ui.ctx().data_mut(|d| *d.get_temp_mut_or_default::<bool>(open_id));
    // the label reserves room for a painted chevron: the "▾" / "▼" glyphs are
    // missing from the fallback fonts on macOS (they draw as boxes)
    let r = widgets::StyledButton::new(theme, "Layers    ").checked(open).show(ui).on_hover_text("Object, overlay and panel visibility");
    let chevron = egui::Rect::from_center_size(egui::Pos2::new(r.rect.max.x - 12.0, r.rect.center().y + 1.0), egui::Vec2::new(8.0, 5.0));
    widgets::paint_chevron(ui, chevron, false, if open { theme.text_strong() } else { theme.text });
    if r.clicked() {
        open = !open;
    }
    ui.ctx().data_mut(|d| *d.get_temp_mut_or_default::<bool>(open_id) = open);
    if !open {
        return;
    }

    let anchor = r.rect;
    let pos = Pos2::new(anchor.min.x, anchor.max.y + 2.0);
    let mut changed = false;
    let area = egui::Area::new(ui.id().with("xpr_map_layers_popup")).order(egui::Order::Foreground).fixed_pos(pos);
    let inner = area.show(ui.ctx(), |ui| {
        egui::Frame::new().fill(theme.bg_darker).stroke(theme.border_stroke()).corner_radius(6.0).inner_margin(8.0).show(ui, |ui| {
            ui.set_width(210.0);
            section(ui, theme, "Objects", &mut changed, |ui, changed| {
                row(ui, theme, &mut toggles.trainers, "Trainers", changed);
                row(ui, theme, &mut toggles.items, "Items", changed);
                row(ui, theme, &mut toggles.hidden, "Hidden items", changed);
                row(ui, theme, &mut toggles.warps, "Warps", changed);
                row(ui, theme, &mut toggles.signs, "Signs", changed);
                row(ui, theme, &mut toggles.berries, "Berry trees", changed);
                row(ui, theme, &mut toggles.npcs, "Other NPCs", changed);
                row(ui, theme, &mut toggles.sprites, "Overworld sprites", changed);
            });
            section(ui, theme, "Overlays", &mut changed, |ui, changed| {
                row(ui, theme, &mut toggles.grid, "Grid", changed);
                row(ui, theme, &mut toggles.labels, "Map names", changed);
                row(ui, theme, &mut toggles.path, "Route path", changed);
            });
            section(ui, theme, "Panels", &mut changed, |ui, changed| {
                row(ui, theme, &mut toggles.navigator, "Navigator", changed);
                row(ui, theme, &mut toggles.follow, "Follow selection", changed);
            });
        });
    });
    if changed {
        cfg.set_map_toggles(toggles.to_json());
    }

    if behind_modal(ui) {
        return;
    }
    // click elsewhere closes the popup (not a click on a dialog above the
    // page — `behind_modal` above already stood down for that)
    let clicked_outside = ui.input(|i| i.pointer.any_pressed())
        && !inner.response.rect.contains(ui.input(|i| i.pointer.interact_pos().unwrap_or(Pos2::ZERO)))
        && !anchor.contains(ui.input(|i| i.pointer.interact_pos().unwrap_or(Pos2::ZERO)));
    if clicked_outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        ui.ctx().data_mut(|d| *d.get_temp_mut_or_default::<bool>(open_id) = false);
    }
}

fn section(ui: &mut Ui, theme: &Theme, title: &str, changed: &mut bool, add: impl FnOnce(&mut Ui, &mut bool)) {
    ui.add_space(4.0);
    widgets::label_font(ui, title, theme.caption_font_bold(), theme.secondary);
    ui.add_space(2.0);
    add(ui, changed);
}

fn row(ui: &mut Ui, theme: &Theme, value: &mut bool, label: &str, changed: &mut bool) {
    if widgets::checkbox_label(ui, theme, value, label, false, true) {
        *changed = true;
    }
}
