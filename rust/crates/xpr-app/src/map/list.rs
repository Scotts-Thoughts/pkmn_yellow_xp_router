//! The searchable map list (pokemap's sidebar as a popup under the toolbar's
//! search box): Cities & Towns / Routes / Indoors & Dungeons.

use egui::{Pos2, Rect, Ui, Vec2};
use xpr_map::{MapId, MapKind, MapPack};
use xpr_ui_kit::theme::Theme;
use xpr_ui_kit::widgets;

#[derive(Default)]
pub struct MapList {
    pub search: String,
    pub open: bool,
    pub hovered: Option<MapId>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Group {
    Cities,
    Routes,
    Indoors,
}

fn group_of(pack: &MapPack, id: MapId) -> Group {
    let m = &pack.maps[id as usize];
    let c = m.const_name.as_str();
    let lower = m.display.to_lowercase();
    if c.contains("ROUTE") || lower.contains("route ") {
        return Group::Routes;
    }
    if m.kind == MapKind::Outdoor && (c.contains("_TOWN") || c.contains("_CITY") || lower.contains(" town") || lower.contains(" city") || lower.contains("island")) {
        return Group::Cities;
    }
    if m.kind == MapKind::Outdoor && (pack.gen == 1 && (id as usize) < 11) {
        return Group::Cities;
    }
    Group::Indoors
}

impl MapList {
    /// Draw the popup below `anchor`; returns the chosen map.
    pub fn popup(&mut self, ui: &mut Ui, theme: &Theme, pack: &MapPack, anchor: Rect, max_h: f32) -> Option<MapId> {
        if !self.open {
            return None;
        }
        let filter = self.search.trim().to_lowercase();
        let mut chosen = None;
        let width = anchor.width().max(260.0);
        let pos = Pos2::new(anchor.min.x, anchor.max.y + 2.0);
        let area = egui::Area::new(ui.id().with("map_list_popup")).order(egui::Order::Foreground).fixed_pos(pos);
        let inner = area.show(ui.ctx(), |ui| {
            egui::Frame::new().fill(theme.bg_darker).stroke(theme.border_stroke()).corner_radius(6.0).inner_margin(6.0).show(ui, |ui| {
                ui.set_width(width);
                let body = theme.body();
                let caption = theme.caption_font_bold();
                widgets::show_scroll(ui, egui::ScrollArea::vertical().id_salt("map_list_scroll").max_height(max_h).auto_shrink([false, true]), |ui| {
                    for (group, title) in [(Group::Cities, "Cities & Towns"), (Group::Routes, "Routes"), (Group::Indoors, "Indoors & Dungeons")] {
                        let mut items: Vec<MapId> = pack
                            .maps
                            .iter()
                            .filter(|m| m.blocks.is_some() || m.kind == MapKind::Outdoor)
                            .filter(|m| group_of(pack, m.id) == group)
                            .filter(|m| filter.is_empty() || m.display.to_lowercase().contains(&filter) || m.const_name.to_lowercase().contains(&filter))
                            .map(|m| m.id)
                            .collect();
                        if items.is_empty() {
                            continue;
                        }
                        if group != Group::Indoors {
                            items.sort_by_key(|id| id.to_string());
                        }
                        ui.add_space(4.0);
                        widgets::label_font(ui, title, caption.clone(), theme.secondary);
                        ui.add_space(2.0);
                        for id in items {
                            let m = &pack.maps[id as usize];
                            let r = ui.allocate_response(Vec2::new(ui.available_width(), 20.0), egui::Sense::click());
                            let hovered = r.hovered();
                            if hovered {
                                ui.painter().rect_filled(r.rect, 3.0, theme.hover_bg);
                                self.hovered = Some(id);
                            }
                            let text = widgets::elide(ui, &m.display, &body, r.rect.width() - 8.0);
                            ui.painter().text(Pos2::new(r.rect.min.x + 6.0, r.rect.center().y), egui::Align2::LEFT_CENTER, text, body.clone(), theme.text);
                            if r.clicked() {
                                chosen = Some(id);
                            }
                        }
                    }
                });
            });
        });
        // click elsewhere closes the popup
        let clicked_outside = ui.input(|i| i.pointer.any_pressed()) && !inner.response.rect.contains(ui.input(|i| i.pointer.interact_pos().unwrap_or(Pos2::ZERO))) && !anchor.contains(ui.input(|i| i.pointer.interact_pos().unwrap_or(Pos2::ZERO)));
        if clicked_outside || chosen.is_some() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.open = false;
        }
        chosen
    }
}
