//! The info card shown above a clicked object or tile (SPEC §3.6): trainer
//! (party from the router's own TrainerDB), item, hidden item, berry, sign,
//! warp, the encounter table of a grass / water tile, and a map's trainers
//! and items for a click on plain ground.

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

    /// Whether `other` would show the same table as `self`. A tile card
    /// shows its map's grass or water encounters, whichever step was clicked.
    pub fn same_table(&self, other: &Card) -> bool {
        match (self, other) {
            (Card::Object { idx: a }, Card::Object { idx: b }) => a == b,
            (Card::Tile { map: a, water: wa, grass: ga, .. }, Card::Tile { map: b, water: wb, grass: gb, .. }) => a == b && wa == wb && ga == gb,
            (Card::Map { map: a }, Card::Map { map: b }) => a == b,
            _ => false,
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
    /// open this object's card instead (a trainer row of the map card)
    pub open_object: Option<u32>,
    /// height of the card's scrolling table this frame (0 when it has none),
    /// so the next frame knows how tall the rest of the card is
    table_h: f32,
    /// where the card was drawn this frame
    pub rect: Rect,
}

/// Width of the object cards (trainer, item, sign, warp) and the map card.
const CARD_W: f32 = 320.0;
/// Width of the encounter card (a grass / water tile). The table
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
/// Guess at the card's height outside its table (header, buttons, margins)
/// for the first frame, before it has been measured.
const CARD_CHROME_H: f32 = 100.0;

/// Draw the card anchored above `anchor` (screen), clamped to `vp` and kept
/// off `avoid` (the navigator minimap's rect, when it is shown).
pub fn draw_card(ui: &mut Ui, card: &Card, anchor: Pos2, vp: Rect, avoid: Option<Rect>, cx: &mut CardCtx) -> CardOut {
    let mut out = CardOut { close: false, actions: Vec::new(), navigate: None, open_object: None, table_h: 0.0, rect: Rect::NOTHING };
    let theme = cx.theme;
    let id = ui.id().with("map_card");
    let width = match card {
        Card::Object { .. } | Card::Map { .. } => CARD_W,
        Card::Tile { .. } => {
            if has_ev_column(cx.gen.as_deref()) {
                ENC_CARD_W
            } else {
                ENC_CARD_W_NO_EV
            }
        }
    };
    // never wider than the viewport allows
    let width = width.min((vp.width() - 16.0).max(200.0));
    // place at the anchor, then clamp using the size measured last frame
    let last_size: Vec2 = ui.ctx().memory(|m| m.data.get_temp::<Vec2>(id)).unwrap_or(Vec2::new(width, 160.0));
    // The table only scrolls when the whole card can't fit in the viewport:
    // it gets all the height the rest of the card (measured last frame)
    // leaves, less the 4px clamp margin at each edge. Measured per card
    // kind, since a map card has more around its table than a tile card.
    // The tables also pass it as `min_scrolled_height`: an Area's contents
    // are capped at last frame's size, so without it a growing table would
    // scroll while the card creeps taller a frame at a time.
    let chrome_id = id.with(("chrome", std::mem::discriminant(card)));
    let chrome_h = ui.ctx().memory(|m| m.data.get_temp::<f32>(chrome_id)).unwrap_or(CARD_CHROME_H);
    let table_max_h = (vp.height() - 8.0 - chrome_h).max(120.0);
    let mut pos = Pos2::new(anchor.x - last_size.x / 2.0, anchor.y - last_size.y - 14.0);
    if pos.y < vp.min.y + 4.0 {
        pos.y = anchor.y + 18.0;
    }
    pos.x = pos.x.clamp(vp.min.x + 4.0, (vp.max.x - last_size.x - 4.0).max(vp.min.x + 4.0));
    pos.y = pos.y.clamp(vp.min.y + 4.0, (vp.max.y - last_size.y - 4.0).max(vp.min.y + 4.0));
    if let Some(r) = avoid {
        pos = avoid_rect(pos, last_size, r, vp);
    }
    // The card rides along with its anchor while the map pans, but when its
    // spot relative to the anchor changes (flipping below it at the top
    // edge, stepping around the navigator) it glides there instead of
    // jumping. The viewport edges stay a hard wall.
    let glide_id = id.with("glide");
    let target = pos - anchor;
    let offset = match ui.ctx().memory(|m| m.data.get_temp::<(Card, Vec2, Vec2)>(glide_id)) {
        Some((prev, offset, size)) if prev == *card && size == last_size => {
            let dt = ui.input(|i| i.stable_dt).min(0.1);
            let next = glide(offset, target, dt);
            if next != target {
                ui.ctx().request_repaint();
            }
            next
        }
        // a newly opened card appears in place, and a card that just
        // measured a new size (opening, a table filling in) settles at once
        _ => target,
    };
    ui.ctx().memory_mut(|m| m.data.insert_temp(glide_id, (card.clone(), offset, last_size)));
    let mut pos = anchor + offset;
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
    out.rect = resp.response.rect;
    let size = resp.response.rect.size();
    let chrome_h = (size.y - out.table_h).max(0.0);
    ui.ctx().memory_mut(|m| {
        m.data.insert_temp(id, size);
        m.data.insert_temp(chrome_id, chrome_h);
    });
    out
}

/// One frame of a card's glide from `offset` toward `target`: an
/// exponential ease that covers ~90 % of the distance in 0.15 s, landing
/// exactly on `target` once it is within half a pixel.
pub fn glide(offset: Vec2, target: Vec2, dt: f32) -> Vec2 {
    let next = offset + (target - offset) * (1.0 - (-dt * 15.0).exp());
    if (target - next).length() < 0.5 {
        target
    } else {
        next
    }
}

/// Moves a card at `pos` (size `size`) off `avoid` if it overlaps it: to
/// the left of it or below it, whichever is the shorter move that still fits
/// in `vp`. The navigator minimap paints over the cards and reads the raw
/// pointer, so a card under it would have its buttons' clicks also move the
/// camera.
pub fn avoid_rect(pos: Pos2, size: Vec2, avoid: Rect, vp: Rect) -> Pos2 {
    let gap = 4.0;
    if !Rect::from_min_size(pos, size).intersects(avoid.expand(gap)) {
        return pos;
    }
    let left = Pos2::new(avoid.min.x - gap - size.x, pos.y);
    let below = Pos2::new(pos.x, avoid.max.y + gap);
    let fits = |p: Pos2| p.x >= vp.min.x + gap && p.y + size.y <= vp.max.y - gap;
    match (fits(left), fits(below)) {
        (true, true) => {
            if pos.x - left.x <= below.y - pos.y {
                left
            } else {
                below
            }
        }
        (true, false) => left,
        (false, true) => below,
        // no room either way (a tiny viewport): below still keeps the
        // card's top rows, header and close button, clear of the panel
        (false, false) => below,
    }
}

fn header(ui: &mut Ui, theme: &Theme, title: &str, subtitle: Option<&str>, out: &mut CardOut) {
    ui.horizontal(|ui| {
        widgets::label_font(ui, title.to_string(), theme.body_bold(), theme.text_strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::StyledButton::new(theme, "×").min_size(Vec2::new(20.0, 20.0)).show(ui).clicked() {
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
        Some(t) => format!("{}{} · ¥{} · {} exp{}", t.trainer_class, if t.location.is_empty() { String::new() } else { format!(" · {}", t.location) }, t.money, t.pkmn.iter().map(|p| p.xp).sum::<i64>(), if double || t.double_battle { " · Double" } else { "" }),
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
                if widgets::StyledButton::new(theme, format!("Add {}", n)).enabled(!cx.state.is_defeated(n)).show(ui).clicked() {
                    out.actions.push(MapAction::AddTrainer { name: n.clone() });
                }
            });
        }
    } else {
        ui.horizontal(|ui| {
            if widgets::StyledButton::new(theme, "Add to route").enabled(!defeated).show(ui).clicked() {
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
    let parts: Vec<String> = ev_yield_parts(gen, species)?.iter().map(|(_, v, name)| format!("{} {}", v, name)).collect();
    Some(if parts.is_empty() { "—".to_string() } else { parts.join(", ") })
}

/// The non-zero stats of a species' EV yield as `(stat index, amount,
/// label)`, like `party_ev_parts`; `None` as for `ev_yield_text`.
fn ev_yield_parts(gen: &GenData, species: &str) -> Option<Vec<(usize, i64, &'static str)>> {
    if gen.get_generation() < 3 {
        return None;
    }
    let y = gen.pkmn_db().get_pkmn(species)?.stat_xp_yield;
    let values = [y.hp, y.attack, y.defense, y.special_attack, y.special_defense, y.speed];
    Some((0..6).filter(|&i| values[i] != 0).map(|i| (i, values[i], STAT_NAMES[i])).collect())
}

/// The stats of a party's EV / stat exp total, in HP, Atk, Def, SpA, SpD,
/// Spe order; gens 1/2 fold the Specials into one (index 3, "Spc").
pub const STAT_NAMES: [&str; 6] = ["HP", "Atk", "Def", "SpA", "SpD", "Spe"];

/// What beating a whole party gives: the summed EV yields in gens 3+, or in
/// gens 1/2 the stat exp, which is the summed base stats with one Special.
/// Returns the prefix ("EVs" / "Stat exp") and `(stat index, amount, label)`
/// for each stat shown (every stat in gens 1/2, the non-zero ones in gens 3+).
/// Held items (Macho Brace, Power items) and Exp. Share splits are not
/// counted. `None` when no species is known.
pub fn party_ev_parts(gen: &GenData, party: &[xpr_data::EnemyPkmn]) -> Option<(&'static str, Vec<(usize, i64, &'static str)>)> {
    let mut sum = [0i64; 6];
    let mut known = false;
    for p in party {
        let Some(sp) = gen.pkmn_db().get_pkmn(&p.name) else { continue };
        let y = sp.stat_xp_yield;
        for (acc, v) in sum.iter_mut().zip([y.hp, y.attack, y.defense, y.special_attack, y.special_defense, y.speed]) {
            *acc += v;
        }
        known = true;
    }
    if !known {
        return None;
    }
    if gen.get_generation() < 3 {
        let parts = [0usize, 1, 2, 3, 5].into_iter().map(|i| (i, sum[i], if i == 3 { "Spc" } else { STAT_NAMES[i] })).collect();
        return Some(("Stat exp", parts));
    }
    Some(("EVs", (0..6).filter(|&i| sum[i] != 0).map(|i| (i, sum[i], STAT_NAMES[i])).collect()))
}

/// `party_ev_parts` as plain text: "EVs: 2 Atk, 1 Spe", "EVs: none",
/// "Stat exp: 130 HP, 95 Atk, 100 Def, 60 Spc, 140 Spe".
pub fn party_ev_text(gen: &GenData, party: &[xpr_data::EnemyPkmn]) -> Option<String> {
    let (prefix, parts) = party_ev_parts(gen, party)?;
    if parts.is_empty() {
        return Some(format!("{}: none", prefix));
    }
    let list: Vec<String> = parts.iter().map(|(_, v, name)| format!("{} {}", v, name)).collect();
    Some(format!("{}: {}", prefix, list.join(", ")))
}

/// The Bulbapedia colour of a stat, the same table pokemap uses
/// (`src/data/trainer-match.js` STAT_COLORS): exact on a dark card, the same
/// hues darkened on a light card so they stay readable on the configurable
/// background.
pub fn stat_color(stat: usize, bg: egui::Color32) -> egui::Color32 {
    use egui::Color32 as C;
    const STAT_COLORS: [C; 6] = [
        C::from_rgb(0xFF, 0x00, 0x00), // HP
        C::from_rgb(0xF0, 0x80, 0x30), // Atk
        C::from_rgb(0xF8, 0xD0, 0x30), // Def
        C::from_rgb(0x68, 0x90, 0xF0), // SpA
        C::from_rgb(0x78, 0xC8, 0x50), // SpD
        C::from_rgb(0xF8, 0x58, 0x88), // Spe
    ];
    let c = STAT_COLORS[stat.min(5)];
    let luma = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if luma < 140.0 {
        c
    } else {
        let f = |v: u8| (v as f32 * 0.6) as u8;
        C::from_rgb(f(c.r()), f(c.g()), f(c.b()))
    }
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
        // EV yield, each stat (amount and label) in its own colour
        if let Some(ev) = c.ev {
            let cell = ev_cell(ev);
            let plain = egui::TextFormat::simple(cell_font.clone(), theme.text);
            let mut job = egui::text::LayoutJob::default();
            match cx.gen.as_deref().and_then(|g| ev_yield_parts(g, &s.species)) {
                None => job.append("?", 0.0, plain.clone()),
                Some(parts) if parts.is_empty() => job.append("—", 0.0, plain.clone()),
                Some(parts) => {
                    for (n, (stat, v, name)) in parts.iter().enumerate() {
                        if n > 0 {
                            job.append(", ", 0.0, plain.clone());
                        }
                        job.append(&format!("{} {}", v, name), 0.0, egui::TextFormat::simple(name_font.clone(), stat_color(*stat, theme.card_bg())));
                    }
                }
            }
            let galley = ui.fonts_mut(|f| f.layout_job(job));
            let pos = Pos2::new(cell.min.x, cell.center().y - galley.size().y / 2.0);
            ui.painter().with_clip_rect(cell).galley(pos, galley, theme.text);
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
/// in a scroll area capped at `max_h` (the room the viewport leaves).
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
    let scroll = widgets::show_scroll(ui, egui::ScrollArea::vertical().id_salt("enc_scroll").max_height(max_h).min_scrolled_height(max_h).auto_shrink([false, true]), |ui| {
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
    out.table_h = scroll.inner_rect.height();
}

fn tile_card(ui: &mut Ui, map: MapId, water: bool, grass: bool, max_h: f32, cx: &mut CardCtx, out: &mut CardOut) {
    let theme = cx.theme;
    let name = cx.pack.map(map).map(|m| m.display.clone()).unwrap_or_default();
    let what = if water { "Water" } else if grass { "Tall grass" } else { "Wild Pokémon" };
    header(ui, theme, &format!("{} — {}", what, name), None, out);
    ui.add_space(2.0);
    encounter_table(ui, map, water, water || grass, max_h, cx, out);
}

/// The card for a click on plain ground: the map's trainers, each with a
/// summary of its team, and its item count. As in pokemap, the encounter
/// table only shows on a grass / water tile.
fn map_card(ui: &mut Ui, map: MapId, max_h: f32, cx: &mut CardCtx, out: &mut CardOut) {
    let theme = cx.theme;
    let Some(m) = cx.pack.map(map) else { return };
    let base = cx.pack.object_range(map).start;
    let objs = cx.pack.objects_of(map);
    let trainer_objs: Vec<(u32, &[String])> = objs
        .iter()
        .enumerate()
        .filter(|(_, o)| o.effective_kind() == ObjectKind::Trainer && !o.trainer_names().is_empty())
        .map(|(i, o)| (base + i as u32, o.trainer_names()))
        .collect();
    let trainers: Vec<&String> = objs.iter().filter(|o| o.effective_kind() == ObjectKind::Trainer).flat_map(|o| o.trainer_names().iter()).collect();
    let items = objs.iter().filter(|o| matches!(o.kind, ObjectKind::Item | ObjectKind::HiddenItem | ObjectKind::Berry) && o.item_name().is_some()).count();
    let remaining = trainers.iter().filter(|n| !cx.state.is_defeated(n)).count();
    header(ui, theme, &m.display, Some(&format!("{} trainers ({} not yet fought) · {} items", trainers.len(), remaining, items)), out);
    if trainer_objs.is_empty() {
        return;
    }
    ui.add_space(2.0);
    let scroll = widgets::show_scroll(ui, egui::ScrollArea::vertical().id_salt("map_card_trainers").max_height(max_h).min_scrolled_height(max_h).auto_shrink([false, true]), |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (i, (idx, names)) in trainer_objs.iter().enumerate() {
            map_trainer_row(ui, i, *idx, names, cx, out);
        }
    });
    out.table_h = scroll.inner_rect.height();
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if widgets::StyledButton::new(theme, "Add all trainers here").enabled(remaining > 0).show(ui).clicked() {
            out.actions.push(MapAction::AddAllTrainers { map });
        }
    });
}

const MAP_ROW_H: f32 = 62.0;
const MAP_ROW_ICON: f32 = 20.0;
/// Width of one party member (icon + level) in a map card row.
const MAP_ROW_MON_W: f32 = 50.0;

/// One trainer of the map card: name, prize money and a fought marker on the
/// first line, the party as icons + levels on the second, and the EVs (stat
/// exp in gens 1/2) the whole fight gives on the third. Clicking the row
/// opens the trainer's own card; "+" adds it (single-variant trainers only,
/// since a variant fight needs its card to pick the right one).
fn map_trainer_row(ui: &mut Ui, i: usize, idx: u32, names: &[String], cx: &mut CardCtx, out: &mut CardOut) {
    let theme = cx.theme;
    let primary = &names[0];
    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), MAP_ROW_H), egui::Sense::click());
    if !ui.is_rect_visible(r) {
        return;
    }
    if i > 0 {
        ui.painter().hline(r.x_range(), r.min.y, egui::Stroke::new(1.0_f32, theme.row_divider()));
    }
    if resp.hovered() {
        ui.painter().rect_filled(r, 3.0, theme.hover_bg);
    }
    let gen = cx.gen.clone();
    let trainer = gen.as_ref().and_then(|g| g.trainer_db().get_trainer(primary).cloned());
    let defeated = cx.state.is_defeated(primary);
    let name_color = if defeated { theme.secondary } else { theme.text_strong() };
    let add_w = if names.len() == 1 { ENC_COL_ADD } else { 0.0 };
    let text_right = r.max.x - add_w - 4.0;

    // line 1: name ... [fought] ¥money · total exp
    let top = Rect::from_min_max(Pos2::new(r.min.x + 4.0, r.min.y + 3.0), Pos2::new(text_right, r.min.y + 21.0));
    let mut right = String::new();
    if let Some(t) = &trainer {
        let exp: i64 = t.pkmn.iter().map(|p| p.xp).sum();
        right = format!("¥{} · {} exp", t.money, exp);
        if t.double_battle {
            right = format!("Double · {}", right);
        }
    }
    if names.len() > 1 {
        right = format!("{} variants · {}", names.len(), right);
    }
    let caption = theme.caption_font();
    let right_w = ui.fonts_mut(|f| f.layout_no_wrap(right.clone(), caption.clone(), theme.secondary).size().x);
    let fought_w = if defeated { ui.fonts_mut(|f| f.layout_no_wrap("fought".to_string(), caption.clone(), theme.success).size().x) + 8.0 } else { 0.0 };
    let name_font = theme.body_bold();
    let name = widgets::elide(ui, primary, &name_font, (top.width() - right_w - fought_w - 8.0).max(40.0));
    widgets::col_text(ui, top, &name, name_font, name_color, Align::Min);
    widgets::col_text(ui, top, &right, caption.clone(), theme.secondary, Align::Max);
    if defeated {
        let cell = Rect::from_min_max(top.min, Pos2::new(top.max.x - right_w - 8.0, top.max.y));
        widgets::col_text(ui, cell, "fought", caption, theme.success, Align::Max);
    }

    // line 2: the party
    let bottom_y = r.min.y + 33.0;
    match &trainer {
        Some(t) => {
            let level_font = theme.font(10.0);
            let mut x = r.min.x + 4.0;
            let per_mon = MAP_ROW_MON_W.min((text_right - x) / t.pkmn.len().max(1) as f32);
            for p in &t.pkmn {
                if let Some(tex) = cx.assets.pkmn_icon(ui.ctx(), &p.name) {
                    let icon = Rect::from_center_size(Pos2::new(x + MAP_ROW_ICON / 2.0, bottom_y), Vec2::splat(MAP_ROW_ICON));
                    ui.painter().image(tex.id(), icon, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), egui::Color32::WHITE);
                }
                let cell = Rect::from_min_max(Pos2::new(x + MAP_ROW_ICON + 1.0, bottom_y - 8.0), Pos2::new(x + per_mon, bottom_y + 8.0));
                widgets::col_text(ui, cell, &p.level.to_string(), level_font.clone(), theme.text, Align::Min);
                x += per_mon;
            }
        }
        None => {
            let cell = Rect::from_min_max(Pos2::new(r.min.x + 4.0, bottom_y - 8.0), Pos2::new(text_right, bottom_y + 8.0));
            widgets::col_text(ui, cell, "No router data for this trainer", theme.caption_font(), theme.secondary, Align::Min);
        }
    }
    // line 3: the EVs / stat exp of beating the whole party, each stat
    // (amount and label) in its own colour
    if let Some((prefix, parts)) = gen.as_deref().zip(trainer.as_ref()).and_then(|(g, t)| party_ev_parts(g, &t.pkmn)) {
        let cell = Rect::from_min_max(Pos2::new(r.min.x + 4.0, r.min.y + 44.0), Pos2::new(text_right, r.min.y + 58.0));
        let plain = egui::TextFormat::simple(theme.caption_font(), theme.secondary);
        let mut job = egui::text::LayoutJob::default();
        job.append(&format!("{}: ", prefix), 0.0, plain.clone());
        if parts.is_empty() {
            job.append("none", 0.0, plain.clone());
        }
        for (n, (stat, v, name)) in parts.iter().enumerate() {
            if n > 0 {
                job.append(", ", 0.0, plain.clone());
            }
            job.append(&format!("{} {}", v, name), 0.0, egui::TextFormat::simple(theme.caption_font_bold(), stat_color(*stat, theme.card_bg())));
        }
        let galley = ui.fonts_mut(|f| f.layout_job(job));
        let pos = Pos2::new(cell.min.x, cell.center().y - galley.size().y / 2.0);
        ui.painter().with_clip_rect(cell).galley(pos, galley, theme.secondary);
    }
    let party = trainer.as_ref().map(|t| t.pkmn.iter().map(|p| format!("{} Lv.{}", p.name, p.level)).collect::<Vec<_>>().join(", ")).unwrap_or_default();

    // the add button draws after the row, so it takes the click over it
    let mut added = false;
    if names.len() == 1 {
        let brect = Rect::from_center_size(Pos2::new(r.max.x - ENC_COL_ADD / 2.0, r.center().y), Vec2::new(26.0, 24.0));
        let mut child = ui.new_child(UiBuilder::new().max_rect(brect).layout(Layout::left_to_right(Align::Center)).id_salt(("map_trainer_add", idx)));
        // greyed out once fought: a beaten trainer can't be fought again
        let b = widgets::StyledButton::new(theme, "+").min_size(Vec2::new(26.0, 24.0)).enabled(!defeated).show(&mut child);
        let b = b.on_hover_text(format!("Add {} to the route", primary)).on_disabled_hover_text(format!("{} is already fought in this route", primary));
        if b.clicked() {
            out.actions.push(MapAction::AddTrainer { name: primary.clone() });
            added = true;
        }
    }
    let resp = if party.is_empty() { resp } else { resp.on_hover_text(party) };
    if resp.clicked() && !added {
        out.open_object = Some(idx);
    }
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
