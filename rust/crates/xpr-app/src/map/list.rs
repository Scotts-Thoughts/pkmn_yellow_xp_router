//! The searchable map list (pokemap's sidebar as a popup under the toolbar's
//! search box): Cities & Towns / Routes / Indoors & Dungeons, plus, once the
//! query is specific enough, Trainers and Items (WP-E, gap G10 — pokemap's
//! item finder, SPEC Phase 5 step 3).

use egui::{Pos2, Rect, Ui, Vec2};
use xpr_map::{Anchor, MapId, MapKind, MapPack};
use xpr_ui_kit::modal::behind_modal;
use xpr_ui_kit::theme::Theme;
use xpr_ui_kit::widgets;

use crate::controller::MainController;

use super::finder;

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

/// What choosing a row in the popup asks the viewer to do.
pub enum Chosen {
    /// Jump to a map: centre the world camera on it, or switch scope.
    Map(MapId),
    /// Focus a trainer's / item's anchors through the focus banner.
    Focus { label: String, anchors: Vec<Anchor> },
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
    /// Draw the popup below `anchor`; returns what was chosen, if anything.
    pub fn popup(&mut self, ui: &mut Ui, theme: &Theme, pack: &MapPack, ctrl: &MainController, anchor: Rect, max_h: f32) -> Option<Chosen> {
        if !self.open {
            return None;
        }
        let filter = self.search.trim().to_lowercase();
        let mut chosen: Option<Chosen> = None;
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
                                chosen = Some(Chosen::Map(id));
                            }
                        }
                    }
                    // Trainers / Items (WP-E, gap G10): only once the query is
                    // specific enough to keep the popup from listing everyone
                    if filter.chars().count() >= 2 {
                        let groups: [(&str, Vec<finder::Hit>); 2] = [("Trainers", finder::find_trainers(pack, ctrl, &filter)), ("Items", finder::find_items(pack, ctrl, &filter))];
                        for (title, hits) in groups {
                            if hits.is_empty() {
                                continue;
                            }
                            ui.add_space(4.0);
                            widgets::label_font(ui, title, caption.clone(), theme.secondary);
                            ui.add_space(2.0);
                            for hit in hits {
                                let r = ui.allocate_response(Vec2::new(ui.available_width(), 20.0), egui::Sense::click());
                                if r.hovered() {
                                    ui.painter().rect_filled(r.rect, 3.0, theme.hover_bg);
                                }
                                let text = widgets::elide(ui, &hit.label, &body, r.rect.width() - 8.0);
                                ui.painter().text(Pos2::new(r.rect.min.x + 6.0, r.rect.center().y), egui::Align2::LEFT_CENTER, text, body.clone(), theme.text);
                                if r.clicked() {
                                    chosen = Some(Chosen::Focus { label: hit.label.clone(), anchors: hit.anchors.clone() });
                                }
                            }
                        }
                    }
                });
            });
        });
        // click elsewhere closes the popup (not a click on a dialog above the page)
        if behind_modal(ui) {
            return chosen;
        }
        let clicked_outside = ui.input(|i| i.pointer.any_pressed()) && !inner.response.rect.contains(ui.input(|i| i.pointer.interact_pos().unwrap_or(Pos2::ZERO))) && !anchor.contains(ui.input(|i| i.pointer.interact_pos().unwrap_or(Pos2::ZERO)));
        if clicked_outside || chosen.is_some() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.open = false;
        }
        chosen
    }
}
