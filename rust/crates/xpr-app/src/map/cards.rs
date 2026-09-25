//! The info card shown above a clicked object or tile (SPEC §3.6): trainer
//! (party from the router's own TrainerDB), item, hidden item, berry, sign,
//! warp, and the encounter table of a grass / water tile or a map.

use std::sync::Arc;

use egui::{Pos2, Rect, Ui, Vec2};
use xpr_data::GenData;
use xpr_map::{EncounterSlot, MapId, MapPack, ObjectKind, Payload};
use xpr_ui_kit::theme::Theme;
use xpr_ui_kit::widgets;

use crate::assets::Assets;
use crate::controller::MainController;

use super::state::RouteMapState;
use super::MapAction;

#[derive(Clone, Debug, PartialEq)]
pub enum Card {
    Object { idx: u32 },
    /// encounters for a clicked tile; `water` selects the water methods
    Tile { map: MapId, x: i32, y: i32, water: bool, grass: bool },
    Map { map: MapId },
}

impl Card {
    pub fn map(&self) -> MapId {
        match self {
            Card::Object { .. } => 0,
            Card::Tile { map, .. } | Card::Map { map } => *map,
        }
    }
}

pub struct CardCtx<'a> {
    pub theme: &'a Theme,
    pub pack: &'a MapPack,
    pub gen: Option<Arc<GenData>>,
    pub version: Option<String>,
    pub state: &'a RouteMapState,
    pub ctrl: &'a MainController,
    pub assets: &'a mut Assets,
    pub routed_event: Option<xpr_engine::NodeId>,
}

pub struct CardOut {
    pub close: bool,
    pub actions: Vec<MapAction>,
    pub navigate: Option<MapId>,
}

const CARD_W: f32 = 320.0;

/// Draw the card anchored above `anchor` (screen), clamped to `vp`.
pub fn draw_card(ui: &mut Ui, card: &Card, anchor: Pos2, vp: Rect, cx: &mut CardCtx) -> CardOut {
    let mut out = CardOut { close: false, actions: Vec::new(), navigate: None };
    let theme = cx.theme;
    let id = ui.id().with("map_card");
    // first pass: measure with a hidden area is overkill; place at anchor, then clamp using last size
    let last_size: Vec2 = ui.ctx().memory(|m| m.data.get_temp::<Vec2>(id)).unwrap_or(Vec2::new(CARD_W, 160.0));
    let mut pos = Pos2::new(anchor.x - last_size.x / 2.0, anchor.y - last_size.y - 14.0);
    if pos.y < vp.min.y + 4.0 {
        pos.y = anchor.y + 18.0;
    }
    pos.x = pos.x.clamp(vp.min.x + 4.0, (vp.max.x - last_size.x - 4.0).max(vp.min.x + 4.0));
    pos.y = pos.y.clamp(vp.min.y + 4.0, (vp.max.y - last_size.y - 4.0).max(vp.min.y + 4.0));
    let area = egui::Area::new(id).order(egui::Order::Foreground).fixed_pos(pos).interactable(true);
    let resp = area.show(ui.ctx(), |ui| {
        egui::Frame::new().fill(theme.card_bg()).stroke(theme.border_stroke()).corner_radius(8.0).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
            ui.set_width(CARD_W);
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
            match card {
                Card::Object { idx } => object_card(ui, *idx, cx, &mut out),
                Card::Tile { map, water, grass, .. } => tile_card(ui, *map, *water, *grass, cx, &mut out),
                Card::Map { map } => map_card(ui, *map, cx, &mut out),
            }
        });
    });
    let size = resp.response.rect.size();
    ui.ctx().memory_mut(|m| m.data.insert_temp(id, size));
    out
}

fn header(ui: &mut Ui, theme: &Theme, title: &str, subtitle: Option<&str>, out: &mut CardOut) {
    ui.horizontal(|ui| {
        widgets::label_font(ui, title.to_string(), theme.body_bold(), theme.text_strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::StyledButton::new(theme, "✕").min_size(Vec2::new(20.0, 20.0)).show(ui).clicked() {
                out.close = true;
            }
        });
    });
    if let Some(s) = subtitle {
        widgets::label_font(ui, s.to_string(), theme.caption_font(), theme.secondary);
    }
}

fn object_card(ui: &mut Ui, idx: u32, cx: &mut CardCtx, out: &mut CardOut) {
    let theme = cx.theme;
    let Some(o) = cx.pack.objects.get(idx as usize) else { return };
    let map_name = cx.pack.map(o.map).map(|m| m.display.clone()).unwrap_or_default();
    match (&o.kind, &o.payload) {
        (ObjectKind::Trainer, Payload::Trainer { variants, double, raw, .. }) | (ObjectKind::Npc, Payload::Trainer { variants, double, raw, .. }) => {
            if variants.is_empty() {
                header(ui, theme, "Trainer", Some(&format!("{} · no router trainer for {}", map_name, raw.clone().unwrap_or_default())), out);
                return;
            }
            trainer_card(ui, variants, *double, &map_name, cx, out);
        }
        (ObjectKind::Npc, Payload::Npc { battles, label }) => {
            if battles.is_empty() {
                header(ui, theme, label.as_deref().unwrap_or("NPC"), Some(&map_name), out);
            } else {
                trainer_card(ui, battles, false, &map_name, cx, out);
            }
        }
        (ObjectKind::Item, Payload::Item { item, raw, fake }) | (ObjectKind::HiddenItem, Payload::Item { item, raw, fake }) | (ObjectKind::Berry, Payload::Item { item, raw, fake }) => {
            let title = match o.kind {
                ObjectKind::HiddenItem => "Hidden item",
                ObjectKind::Berry => "Berry tree",
                _ => "Item",
            };
            match item {
                Some(name) => {
                    header(ui, theme, name, Some(&map_name), out);
                    let acquired = cx.state.is_acquired(name);
                    if acquired {
                        widgets::label_font(ui, "Already picked up in this route".to_string(), theme.caption_font(), theme.success);
                    }
                    if o.kind == ObjectKind::HiddenItem {
                        widgets::label_font(ui, "Use the Itemfinder to locate it in-game".to_string(), theme.caption_font(), theme.secondary);
                    }
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if widgets::StyledButton::new(theme, "Add to route").show(ui).clicked() {
                            out.actions.push(MapAction::AddItem { name: name.clone() });
                        }
                        if let Some(id) = cx.routed_event {
                            if widgets::StyledButton::new(theme, "Select in list").show(ui).clicked() {
                                out.actions.push(MapAction::SelectEvent(id));
                            }
                        }
                    });
                }
                None => {
                    let what = if *fake { "not a bag item (a Pokémon or a decoy)".to_string() } else { format!("unknown item {}", raw.clone().unwrap_or_default()) };
                    header(ui, theme, title, Some(&format!("{} · {}", map_name, what)), out);
                }
            }
        }
        (ObjectKind::Sign, Payload::Sign { text_key }) => {
            header(ui, theme, "Sign", Some(&map_name), out);
            let text = cx.pack.sign_text.get(text_key).cloned().unwrap_or_else(|| text_key.clone());
            ui.add(egui::Label::new(egui::RichText::new(text).font(theme.body()).color(theme.text)).wrap());
        }
        (ObjectKind::Warp, Payload::Warp { dest_map, dest_warp }) => {
            let dest = dest_map.as_ref().and_then(|c| cx.pack.map_by_const(c));
            let dest_name = dest.map(|m| m.display.clone()).or_else(|| dest_map.clone()).unwrap_or_else(|| "?".into());
            header(ui, theme, "Warp", Some(&format!("{} → {}{}", map_name, dest_name, dest_warp.map(|w| format!(" (warp {})", w)).unwrap_or_default())), out);
            if let Some(d) = dest {
                if widgets::StyledButton::new(theme, format!("Go to {}", d.display)).show(ui).clicked() {
                    out.navigate = Some(d.id);
                }
            }
        }
        _ => {
            header(ui, theme, "Object", Some(&map_name), out);
        }
    }
}

fn trainer_card(ui: &mut Ui, names: &[String], double: bool, map_name: &str, cx: &mut CardCtx, out: &mut CardOut) {
    let theme = cx.theme;
    let gen = cx.gen.clone();
    let primary = names[0].clone();
    let trainer = gen.as_ref().and_then(|g| g.trainer_db().get_trainer(&primary).cloned());
    let subtitle = match &trainer {
        Some(t) => format!("{}{} · ¥{}{}", t.trainer_class, if t.location.is_empty() { String::new() } else { format!(" · {}", t.location) }, t.money, if double || t.double_battle { " · Double" } else { "" }),
        None => map_name.to_string(),
    };
    header(ui, theme, &primary, Some(&subtitle), out);
    if let Some(t) = &trainer {
        ui.add_space(2.0);
        for p in &t.pkmn {
            ui.horizontal(|ui| {
                if let Some(tex) = cx.assets.pkmn_icon(ui.ctx(), &p.name) {
                    ui.add(egui::Image::new(&tex).fit_to_exact_size(Vec2::new(22.0, 22.0)));
                }
                widgets::label_font(ui, format!("{} Lv.{}", p.name, p.level), theme.body_bold(), theme.text);
                let moves: Vec<&str> = p.move_list.iter().filter_map(|m| m.as_deref()).collect();
                if !moves.is_empty() {
                    let s = widgets::elide(ui, &moves.join(" / "), &theme.caption_font(), 170.0);
                    widgets::label_font(ui, s, theme.caption_font(), theme.secondary);
                }
            });
        }
    }
    let defeated = cx.state.is_defeated(&primary);
    if defeated {
        widgets::label_font(ui, "Already fought in this route".to_string(), theme.caption_font(), theme.success);
    }
    ui.add_space(4.0);
    if names.len() > 1 {
        widgets::label_font(ui, "This fight depends on the run (starter / version):".to_string(), theme.caption_font(), theme.secondary);
        for n in names {
            ui.horizontal(|ui| {
                if widgets::StyledButton::new(theme, format!("Add {}", n)).show(ui).clicked() {
                    out.actions.push(MapAction::AddTrainer { name: n.clone() });
                }
            });
        }
    } else {
        ui.horizontal(|ui| {
            if widgets::StyledButton::new(theme, "Add to route").show(ui).clicked() {
                out.actions.push(MapAction::AddTrainer { name: primary.clone() });
            }
            if let Some(id) = cx.routed_event {
                if widgets::StyledButton::new(theme, "Select in list").show(ui).clicked() {
                    out.actions.push(MapAction::SelectEvent(id));
                }
            }
        });
    }
}

fn encounter_rows(ui: &mut Ui, theme: &Theme, slots: &[EncounterSlot], cx: &mut CardCtx, out: &mut CardOut) {
    for s in slots {
        ui.horizontal(|ui| {
            if let Some(tex) = cx.assets.pkmn_icon(ui.ctx(), &s.species) {
                ui.add(egui::Image::new(&tex).fit_to_exact_size(Vec2::new(18.0, 18.0)));
            }
            let lvl = if s.min_level == s.max_level { format!("Lv.{}", s.min_level) } else { format!("Lv.{}-{}", s.min_level, s.max_level) };
            widgets::label_font(ui, format!("{} {}", s.species, lvl), theme.body(), theme.text);
            let rate = if (s.rate - s.rate.round()).abs() < 0.05 { format!("{}%", s.rate.round() as i64) } else { format!("{:.1}%", s.rate) };
            widgets::label_font(ui, rate, theme.caption_font(), theme.secondary);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::StyledButton::new(theme, "+").min_size(Vec2::new(22.0, 20.0)).show(ui).clicked() {
                    out.actions.push(MapAction::AddWild { species: s.species.clone(), level: s.min_level });
                }
            });
        });
    }
}

fn encounter_section(ui: &mut Ui, map: MapId, water: bool, only_terrain: bool, cx: &mut CardCtx, out: &mut CardOut) {
    let theme = cx.theme;
    let Some(table) = cx.pack.encounters.get(&map) else {
        widgets::label_font(ui, "No wild Pokémon here".to_string(), theme.caption_font(), theme.secondary);
        return;
    };
    let version = cx.version.clone().unwrap_or_default();
    let water_methods = ["surf", "old_rod", "good_rod", "super_rod"];
    let mut any = false;
    for m in &table.methods {
        let base = m.name.split('_').next().unwrap_or("");
        let is_water = water_methods.iter().any(|w| m.name.starts_with(w)) || base == "surf";
        if only_terrain && is_water != water {
            continue;
        }
        let Some(slots) = table.slots_for(&m.name, &version) else { continue };
        if slots.is_empty() {
            continue;
        }
        any = true;
        ui.add_space(4.0);
        let title = m.name.replace('_', " ");
        let rate = m.base_rate.map(|r| format!("  ·  rate {}", r)).unwrap_or_default();
        widgets::label_font(ui, format!("{}{}", title.to_uppercase(), rate), theme.caption_font_bold(), theme.secondary);
        encounter_rows(ui, theme, slots, cx, out);
    }
    if !any {
        widgets::label_font(ui, "No wild Pokémon for this terrain".to_string(), theme.caption_font(), theme.secondary);
    }
}

fn tile_card(ui: &mut Ui, map: MapId, water: bool, grass: bool, cx: &mut CardCtx, out: &mut CardOut) {
    let theme = cx.theme;
    let name = cx.pack.map(map).map(|m| m.display.clone()).unwrap_or_default();
    let what = if water { "Water" } else if grass { "Tall grass" } else { "Wild Pokémon" };
    header(ui, theme, &format!("{} — {}", what, name), None, out);
    widgets::show_scroll(ui, egui::ScrollArea::vertical().id_salt("enc_scroll").max_height(260.0).auto_shrink([false, true]), |ui| {
        encounter_section(ui, map, water, water || grass, cx, out);
    });
}

fn map_card(ui: &mut Ui, map: MapId, cx: &mut CardCtx, out: &mut CardOut) {
    let theme = cx.theme;
    let Some(m) = cx.pack.map(map) else { return };
    let objs = cx.pack.objects_of(map);
    let trainers: Vec<&String> = objs.iter().filter(|o| o.effective_kind() == ObjectKind::Trainer).flat_map(|o| o.trainer_names().iter()).collect();
    let items = objs.iter().filter(|o| matches!(o.kind, ObjectKind::Item | ObjectKind::HiddenItem | ObjectKind::Berry) && o.item_name().is_some()).count();
    let remaining = trainers.iter().filter(|n| !cx.state.is_defeated(n)).count();
    header(ui, theme, &m.display, Some(&format!("{} trainers ({} not yet fought) · {} items", trainers.len(), remaining, items)), out);
    ui.horizontal(|ui| {
        if widgets::StyledButton::new(theme, "Add all trainers here").enabled(remaining > 0).show(ui).clicked() {
            out.actions.push(MapAction::AddAllTrainers { map });
        }
    });
    widgets::show_scroll(ui, egui::ScrollArea::vertical().id_salt("map_enc_scroll").max_height(240.0).auto_shrink([false, true]), |ui| {
        encounter_section(ui, map, false, false, cx, out);
    });
}

/// The route event id already covering an object, for "Select in list".
pub fn routed_event_for(ctrl: &MainController, pack: &MapPack, idx: u32) -> Option<xpr_engine::NodeId> {
    let o = pack.objects.get(idx as usize)?;
    for n in o.trainer_names() {
        if let Some(id) = ctrl.find_first_event_by_trainer_name(n) {
            return Some(id);
        }
    }
    None
}
