//! The toolbar's "Find an item…" box: a popup of the item names placed on
//! the map (item balls and hidden items, straight from the pack's objects,
//! so it also finds items the router's link table doesn't know). Choosing
//! one focuses every instance through the focus banner, whose ◀ ▶ (or
//! Enter in the box) loop through them.

use egui::{Pos2, Rect, Ui, Vec2};
use xpr_map::{Anchor, MapPack, ObjectKind, Payload, Precision};
use xpr_ui_kit::modal::behind_modal;
use xpr_ui_kit::theme::Theme;
use xpr_ui_kit::widgets;

#[derive(Default)]
pub struct ItemSearch {
    pub query: String,
    pub open: bool,
    /// the item whose instances the focus banner is looping through
    pub active: Option<String>,
}

/// One item name on the map and how many of each kind there are.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemHit {
    pub name: String,
    pub balls: usize,
    pub hidden: usize,
}

impl ItemHit {
    pub fn counts(&self) -> String {
        match (self.balls, self.hidden) {
            (b, 0) => format!("{} ball{}", b, if b == 1 { "" } else { "s" }),
            (0, h) => format!("{} hidden", h),
            (b, h) => format!("{} ball{} · {} hidden", b, if b == 1 { "" } else { "s" }, h),
        }
    }
}

/// The item a map object stands for, if it is a real item ball or hidden
/// item (not a berry tree, not a "fake" ball like the Voltorb traps).
fn item_of(o: &xpr_map::MapObject) -> Option<&str> {
    if !matches!(o.kind, ObjectKind::Item | ObjectKind::HiddenItem) {
        return None;
    }
    match &o.payload {
        Payload::Item { item: Some(name), fake: false, .. } => Some(name.as_str()),
        _ => None,
    }
}

/// Distinct item names on the map containing `query` (case-insensitive;
/// an empty query lists them all), alphabetically.
pub fn find(pack: &MapPack, query: &str) -> Vec<ItemHit> {
    let q = query.trim().to_lowercase();
    let mut out: Vec<ItemHit> = Vec::new();
    for o in &pack.objects {
        let Some(name) = item_of(o) else { continue };
        if !q.is_empty() && !name.to_lowercase().contains(&q) {
            continue;
        }
        let i = match out.iter().position(|h| h.name == name) {
            Some(i) => i,
            None => {
                out.push(ItemHit { name: name.to_string(), balls: 0, hidden: 0 });
                out.len() - 1
            }
        };
        if o.kind == ObjectKind::HiddenItem {
            out[i].hidden += 1;
        } else {
            out[i].balls += 1;
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Every placement of `name` (balls and hidden items), in the pack's map order.
pub fn instances(pack: &MapPack, name: &str) -> Vec<Anchor> {
    pack.objects
        .iter()
        .enumerate()
        .filter(|(_, o)| item_of(o) == Some(name))
        .map(|(i, o)| Anchor { map: o.map, x: o.x, y: o.y, precision: Precision::Object, object: Some(i as u32), source: "item_search".into() })
        .collect()
}

impl ItemSearch {
    /// Draw the popup below `anchor`; returns the chosen item name, if any.
    pub fn popup(&mut self, ui: &mut Ui, theme: &Theme, pack: &MapPack, anchor: Rect, max_h: f32) -> Option<String> {
        if !self.open {
            return None;
        }
        let hits = find(pack, &self.query);
        let mut chosen: Option<String> = None;
        let width = anchor.width().max(260.0);
        let pos = Pos2::new(anchor.min.x, anchor.max.y + 2.0);
        let area = egui::Area::new(ui.id().with("map_item_popup")).order(egui::Order::Foreground).fixed_pos(pos);
        let inner = area.show(ui.ctx(), |ui| {
            egui::Frame::new().fill(theme.bg_darker).stroke(theme.border_stroke()).corner_radius(6.0).inner_margin(6.0).show(ui, |ui| {
                ui.set_width(width);
                let body = theme.body();
                let caption = theme.caption_font();
                if hits.is_empty() {
                    widgets::label_font(ui, "No items on the map match", body.clone(), theme.secondary);
                    return;
                }
                widgets::show_scroll(ui, egui::ScrollArea::vertical().id_salt("map_item_scroll").max_height(max_h).auto_shrink([false, true]), |ui| {
                    for hit in &hits {
                        let r = ui.allocate_response(Vec2::new(ui.available_width(), 20.0), egui::Sense::click());
                        if r.hovered() {
                            ui.painter().rect_filled(r.rect, 3.0, theme.hover_bg);
                        }
                        let counts = hit.counts();
                        let counts_w = ui.painter().layout_no_wrap(counts.clone(), caption.clone(), theme.secondary).size().x;
                        let text = widgets::elide(ui, &hit.name, &body, r.rect.width() - counts_w - 20.0);
                        ui.painter().text(Pos2::new(r.rect.min.x + 6.0, r.rect.center().y), egui::Align2::LEFT_CENTER, text, body.clone(), theme.text);
                        ui.painter().text(Pos2::new(r.rect.max.x - 6.0, r.rect.center().y), egui::Align2::RIGHT_CENTER, counts, caption.clone(), theme.secondary);
                        if r.clicked() {
                            chosen = Some(hit.name.clone());
                        }
                    }
                });
            });
        });
        // click elsewhere closes the popup (not a click on a dialog above the page)
        if behind_modal(ui) {
            return chosen;
        }
        let at = ui.input(|i| i.pointer.interact_pos().unwrap_or(Pos2::ZERO));
        let clicked_outside = ui.input(|i| i.pointer.any_pressed()) && !inner.response.rect.contains(at) && !anchor.contains(at);
        if clicked_outside || chosen.is_some() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.open = false;
        }
        chosen
    }
}
