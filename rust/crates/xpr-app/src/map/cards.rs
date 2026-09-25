//! The info card shown above a clicked object or tile (SPEC §3.6): trainer
//! (party from the router's own TrainerDB), item, hidden item, berry, sign,
//! warp, and the encounter table of a grass / water tile or a map.

use std::sync::Arc;

use egui::{Align, Layout, Pos2, Rect, Ui, UiBuilder, Vec2};
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

/// Width of the object cards (trainer, item, sign, warp).
const CARD_W: f32 = 320.0;
/// Width of the encounter cards (a grass / water tile, a map). The table
/// has an icon, species, level, rate, the EV yield in gens 3+, and an add
/// button, so it needs more room than an object card.
const ENC_CARD_W: f32 = 480.0;
/// The encounter card without the EV column (gens 1/2).
const ENC_CARD_W_NO_EV: f32 = 380.0;
const ENC_ROW_H: f32 = 28.0;
const ENC_HEAD_H: f32 = 20.0;
const ENC_ICON: f32 = 24.0;
/// Encounter table column widths, right to left: add button, EV yield, rate, level.
const ENC_COL_ADD: f32 = 34.0;
const ENC_COL_EV: f32 = 126.0;
const ENC_COL_RATE: f32 = 58.0;
const ENC_COL_LEVEL: f32 = 64.0;
/// The encounter table scrolls once taller than this share of the map viewport.
const ENC_MAX_H_FRACTION: f32 = 0.6;

/// Draw the card anchored above `anchor` (screen), clamped to `vp`.
pub fn draw_card(ui: &mut Ui, card: &Card, anchor: Pos2, vp: Rect, cx: &mut CardCtx) -> CardOut {
    let mut out = CardOut { close: false, actions: Vec::new(), navigate: None };
    let theme = cx.theme;
    let id = ui.id().with("map_card");
    let width = match card {
        Card::Object { .. } => CARD_W,
        Card::Tile { .. } | Card::Map { .. } => {
            if has_ev_column(cx.gen.as_deref()) {
                ENC_CARD_W
            } else {
                ENC_CARD_W_NO_EV
            }
        }
    };
    // never wider than the viewport allows
    let width = width.min((vp.width() - 16.0).max(200.0));
    let table_max_h = (vp.height() * ENC_MAX_H_FRACTION).max(160.0);
    // place at the anchor, then clamp using the size measured last frame
    let last_size: Vec2 = ui.ctx().memory(|m| m.data.get_temp::<Vec2>(id)).unwrap_or(Vec2::new(width, 160.0));
    let mut pos = Pos2::new(anchor.x - last_size.x / 2.0, anchor.y - last_size.y - 14.0);
    if pos.y < vp.min.y + 4.0 {
        pos.y = anchor.y + 18.0;
    }
    pos.x = pos.x.clamp(vp.min.x + 4.0, (vp.max.x - last_size.x - 4.0).max(vp.min.x + 4.0));
    pos.y = pos.y.clamp(vp.min.y + 4.0, (vp.max.y - last_size.y - 4.0).max(vp.min.y + 4.0));
    let area = egui::Area::new(id).order(egui::Order::Foreground).fixed_pos(pos).interactable(true);
    let resp = area.show(ui.ctx(), |ui| {
        egui::Frame::new().fill(theme.card_bg()).stroke(theme.border_stroke()).corner_radius(8.0).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
            match card {
                Card::Object { idx } => object_card(ui, *idx, cx, &mut out),
                Card::Tile { map, water, grass, .. } => tile_card(ui, *map, *water, *grass, table_max_h, cx, &mut out),
                Card::Map { map } => map_card(ui, *map, table_max_h, cx, &mut out),
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

// ---------------------------------------------------------------------------
// Encounter tables
// ---------------------------------------------------------------------------

/// Whether the game's encounter tables carry an EV yield column: gens 3+.
/// Gens 1/2 hand out stat exp equal to the base stats, nothing to tabulate.
pub fn has_ev_column(gen: Option<&GenData>) -> bool {
    gen.map(|g| g.get_generation() >= 3).unwrap_or(false)
}

/// The EV yield of a species as "1 SpA" / "2 HP, 1 Def" (stats in HP, Atk,
/// Def, SpA, SpD, Spe order), "—" for a species that yields nothing, and
/// `None` when the game has no EV yields (gens 1/2) or the species is unknown.
pub fn ev_yield_text(gen: &GenData, species: &str) -> Option<String> {
    if gen.get_generation() < 3 {
        return None;
    }
    let y = gen.pkmn_db().get_pkmn(species)?.stat_xp_yield;
    let parts: Vec<String> = [(y.hp, "HP"), (y.attack, "Atk"), (y.defense, "Def"), (y.special_attack, "SpA"), (y.special_defense, "SpD"), (y.speed, "Spe")]
        .into_iter()
        .filter(|(v, _)| *v != 0)
        .map(|(v, name)| format!("{} {}", v, name))
        .collect();
    Some(if parts.is_empty() { "—".to_string() } else { parts.join(", ") })
}

fn is_water_method(name: &str) -> bool {
    ["surf", "old_rod", "good_rod", "super_rod"].iter().any(|w| name.starts_with(w))
}

fn rate_text(rate: f64) -> String {
    if (rate - rate.round()).abs() < 0.05 {
        format!("{}%", rate.round() as i64)
    } else {
        format!("{:.1}%", rate)
    }
}

fn level_text(s: &EncounterSlot) -> String {
    if s.min_level == s.max_level {
        s.min_level.to_string()
    } else {
        format!("{}-{}", s.min_level, s.max_level)
    }
}

/// The cells of one encounter row, laid out right to left from the row rect.
struct EncCols {
    species: Rect,
    level: Rect,
    rate: Rect,
    ev: Option<Rect>,
    add: Rect,
}

fn enc_cols(r: Rect, show_ev: bool) -> EncCols {
    let cell = |right: f32, w: f32| Rect::from_min_max(Pos2::new(right - w, r.min.y), Pos2::new(right, r.max.y));
    let mut x = r.max.x;
    let add = cell(x, ENC_COL_ADD);
    x -= ENC_COL_ADD;
    let ev = show_ev.then(|| {
        let c = cell(x, ENC_COL_EV);
        x -= ENC_COL_EV;
        c
    });
    let rate = cell(x, ENC_COL_RATE);
    x -= ENC_COL_RATE;
    let level = cell(x, ENC_COL_LEVEL);
    x -= ENC_COL_LEVEL;
    let species = Rect::from_min_max(r.min, Pos2::new(x, r.max.y));
    EncCols { species, level, rate, ev, add }
}

/// Horizontal padding of the rate cell, and the wider left inset of the EV
/// cell that keeps it clear of the right-aligned rates.
const ENC_CELL_PAD: f32 = 6.0;
const ENC_EV_INSET: f32 = 16.0;

fn ev_cell(ev: Rect) -> Rect {
    Rect::from_min_max(Pos2::new(ev.min.x + ENC_EV_INSET, ev.min.y), Pos2::new(ev.max.x - ENC_CELL_PAD, ev.max.y))
}

fn encounter_header(ui: &mut Ui, theme: &Theme, show_ev: bool) {
    let r = widgets::table_row(ui, ENC_HEAD_H, None);
    let c = enc_cols(r, show_ev);
    let font = theme.caption_font_bold();
    let color = theme.secondary;
    let name_cell = Rect::from_min_max(Pos2::new(c.species.min.x + ENC_ICON + 6.0, r.min.y), Pos2::new(c.species.max.x, r.max.y));
    widgets::col_text(ui, name_cell, "Pokémon", font.clone(), color, Align::Min);
    widgets::col_text(ui, c.level, "Level", font.clone(), color, Align::Min);
    widgets::col_text(ui, c.rate.shrink2(Vec2::new(ENC_CELL_PAD, 0.0)), "Rate", font.clone(), color, Align::Max);
    if let Some(ev) = c.ev {
        widgets::col_text(ui, ev_cell(ev), "EV yield", font, color, Align::Min);
    }
    widgets::hairline(ui, theme.border);
}

fn encounter_rows(ui: &mut Ui, method: &str, slots: &[EncounterSlot], show_ev: bool, cx: &mut CardCtx, out: &mut CardOut) {
    let theme = cx.theme;
    let name_font = theme.font_bold(10.0);
    let cell_font = theme.font(10.0);
    let divider = theme.row_divider();
    for (i, s) in slots.iter().enumerate() {
        let r = widgets::table_row(ui, ENC_ROW_H, if i > 0 { Some(divider) } else { None });
        if !ui.is_rect_visible(r) {
            continue;
        }
        if ui.rect_contains_pointer(r) {
            ui.painter().rect_filled(r, 3.0, theme.hover_bg);
        }
        let c = enc_cols(r, show_ev);
        // icon + species
        let mut x = c.species.min.x;
        if let Some(tex) = cx.assets.pkmn_icon(ui.ctx(), &s.species) {
            let icon = Rect::from_center_size(Pos2::new(x + ENC_ICON / 2.0, r.center().y), Vec2::splat(ENC_ICON));
            ui.painter().image(tex.id(), icon, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), egui::Color32::WHITE);
        }
        x += ENC_ICON + 6.0;
        let name_cell = Rect::from_min_max(Pos2::new(x, r.min.y), Pos2::new(c.species.max.x - 4.0, r.max.y));
        let name = widgets::elide(ui, &s.species, &name_font, name_cell.width());
        widgets::col_text(ui, name_cell, &name, name_font.clone(), theme.text_strong(), Align::Min);
        widgets::col_text(ui, c.level, &level_text(s), cell_font.clone(), theme.text, Align::Min);
        widgets::col_text(ui, c.rate.shrink2(Vec2::new(ENC_CELL_PAD, 0.0)), &rate_text(s.rate), cell_font.clone(), theme.text, Align::Max);
        if let Some(ev) = c.ev {
            let text = cx.gen.as_deref().and_then(|g| ev_yield_text(g, &s.species)).unwrap_or_else(|| "?".to_string());
            let cell = ev_cell(ev);
            let shown = widgets::elide(ui, &text, &cell_font, cell.width());
            widgets::col_text(ui, cell, &shown, cell_font.clone(), theme.text, Align::Min);
        }
        // add button
        let brect = Rect::from_center_size(c.add.center(), Vec2::new(26.0, 24.0));
        let mut child = ui.new_child(UiBuilder::new().max_rect(brect).layout(Layout::left_to_right(Align::Center)).id_salt(("enc_add", method, i)));
        let resp = widgets::StyledButton::new(theme, "+").min_size(Vec2::new(26.0, 24.0)).show(&mut child);
        if resp.on_hover_text(format!("Add a wild {} Lv.{} to the route", s.species, s.min_level)).clicked() {
            out.actions.push(MapAction::AddWild { species: s.species.clone(), level: s.min_level });
        }
    }
}

/// The encounter table of `map`: a column header, then one section per
/// method (all of them, or only the land / water ones when `only_terrain`),
/// in a scroll area capped at `max_h`.
fn encounter_table(ui: &mut Ui, map: MapId, water: bool, only_terrain: bool, max_h: f32, cx: &mut CardCtx, out: &mut CardOut) {
    let theme = cx.theme;
    let pack: &MapPack = cx.pack;
    let show_ev = has_ev_column(cx.gen.as_deref());
    let version = cx.version.clone().unwrap_or_default();
    let sections: Vec<(&str, Option<i64>, &[EncounterSlot])> = pack
        .encounters
        .get(&map)
        .map(|table| {
            table
                .methods
                .iter()
                .filter(|m| !only_terrain || is_water_method(&m.name) == water)
                .filter_map(|m| table.slots_for(&m.name, &version).filter(|s| !s.is_empty()).map(|s| (m.name.as_str(), m.base_rate, s)))
                .collect()
        })
        .unwrap_or_default();
    if sections.is_empty() {
        let msg = if pack.encounters.contains_key(&map) { "No wild Pokémon for this terrain" } else { "No wild Pokémon here" };
        widgets::label_font(ui, msg.to_string(), theme.caption_font(), theme.secondary);
        return;
    }
    encounter_header(ui, theme, show_ev);
    widgets::show_scroll(ui, egui::ScrollArea::vertical().id_salt("enc_scroll").max_height(max_h).auto_shrink([false, true]), |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (name, base_rate, slots) in sections {
            ui.add_space(6.0);
            let title = name.replace('_', " ").to_uppercase();
            let rate = base_rate.map(|r| format!("  ·  rate {}", r)).unwrap_or_default();
            widgets::label_font(ui, format!("{}{}", title, rate), theme.body_bold(), theme.secondary);
            ui.add_space(2.0);
            encounter_rows(ui, name, slots, show_ev, cx, out);
        }
    });
}

fn tile_card(ui: &mut Ui, map: MapId, water: bool, grass: bool, max_h: f32, cx: &mut CardCtx, out: &mut CardOut) {
    let theme = cx.theme;
    let name = cx.pack.map(map).map(|m| m.display.clone()).unwrap_or_default();
    let what = if water { "Water" } else if grass { "Tall grass" } else { "Wild Pokémon" };
    header(ui, theme, &format!("{} — {}", what, name), None, out);
    ui.add_space(2.0);
    encounter_table(ui, map, water, water || grass, max_h, cx, out);
}

fn map_card(ui: &mut Ui, map: MapId, max_h: f32, cx: &mut CardCtx, out: &mut CardOut) {
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
    ui.add_space(4.0);
    if cx.pack.encounters.contains_key(&map) {
        widgets::label_font(ui, "Wild Pokémon".to_string(), theme.body_bold(), theme.text_strong());
    }
    encounter_table(ui, map, false, false, max_h, cx, out);
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
