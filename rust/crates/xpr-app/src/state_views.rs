//! The read-only half of the Pre-Event State tab: the identity header card
//! and the Stats / Moves / Bag cards
//! (docs/rust_port/design/pre_event_state/SPEC.md §5.2–5.5).

use egui::{Align, Color32, FontId, Layout, Pos2, Rect, Sense, Ui, Vec2};

use xpr_data::model::{EnemyPkmn, StatBlock};
use xpr_data::{BadgeList, GenData};
use xpr_engine::{Inventory, RouteState};
use xpr_ui_kit::theme::Theme;
use xpr_ui_kit::widgets::{self, fmt_thousands, CARD_TITLE_H};

use crate::assets::Assets;

/// Gap between cards.
pub const CARD_GAP: f32 = 12.0;
const TITLE_GAP: f32 = 6.0;
const STAT_ROW_H: f32 = 27.0;
const CARD_FOOTER_H: f32 = 24.0;
const MOVE_ROW_H: f32 = 34.0;
const MONEY_ROW_H: f32 = 34.0;
const BAG_ROW_H: f32 = 25.0;
const EMPTY_BAG_H: f32 = 72.0;
/// Bag rows drawn before the overflow row takes the last slot.
const MAX_BAG_ROWS: usize = 20;
/// Column widths of the stats table: (Now, from EVs, real / total).
const STAT_COLS_EV: [f32; 3] = [56.0, 78.0, 108.0];
const STAT_COLS_STATEXP: [f32; 3] = [56.0, 86.0, 118.0];

/// The header's right-hand "Before" block: the selected event and where it is.
#[derive(Clone, Debug, Default)]
pub struct BeforeInfo {
    pub label: String,
    pub location: Option<String>,
}

/// The last Pokémon shown; kept (with its badges and exp position) while
/// the selected event has no state, as the Python app did.
#[derive(Clone, Debug)]
pub struct LastPkmn {
    pub pkmn: EnemyPkmn,
    pub badges: BadgeList,
    pub cur_xp: i64,
    pub xp_to_next_level: i64,
    pub percent_xp_to_next_level: i64,
}

/// One inline text segment: (text, font, colour).
type Seg<'a> = (&'a str, FontId, Color32);

fn seg_width(ui: &Ui, segs: &[Seg], gap: f32) -> f32 {
    let mut w = 0.0;
    for (i, (t, f, _)) in segs.iter().enumerate() {
        if i > 0 {
            w += gap;
        }
        w += widgets::text_width(ui, t, f);
    }
    w
}

/// Paint `segs` left to right from `x`, vertically centred on `cy`.
fn paint_run(ui: &Ui, x: f32, cy: f32, segs: &[Seg], gap: f32) -> f32 {
    let mut cx = x;
    for (i, (t, f, c)) in segs.iter().enumerate() {
        if i > 0 {
            cx += gap;
        }
        let g = ui.fonts_mut(|fonts| fonts.layout_no_wrap(t.to_string(), f.clone(), *c));
        let size = g.size();
        ui.painter().galley(Pos2::new(cx, cy - size.y / 2.0), g, *c);
        cx += size.x;
    }
    cx - x
}

fn row_h(ui: &Ui, font: &FontId) -> f32 {
    ui.fonts_mut(|f| f.row_height(font))
}

// ---------------------------------------------------------------------------
// Identity header
// ---------------------------------------------------------------------------

pub fn identity_header(ui: &mut Ui, theme: &Theme, gen: Option<&GenData>, last: Option<&LastPkmn>, has_state: bool, assets: &mut Assets, before: &BeforeInfo) {
    let tile = 68.0;
    let pad = egui::Margin { left: 16, right: 16, top: 14, bottom: 14 };
    widgets::card(ui, theme, Some(pad), |ui| {
        let width = ui.available_width();
        ui.allocate_ui_with_layout(Vec2::new(width, tile), Layout::left_to_right(Align::Center), |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing.x = 16.0;
            // 1. icon tile
            let (tile_rect, _) = ui.allocate_exact_size(Vec2::splat(tile), Sense::hover());
            ui.painter().rect(tile_rect, egui::CornerRadius::same(12), theme.well_bg(), egui::Stroke::new(1.0_f32, theme.card_border()), egui::StrokeKind::Inside);
            if let Some(l) = last {
                if let Some(tex) = assets.pkmn_icon(ui.ctx(), &l.pkmn.name) {
                    let [w, h] = tex.size();
                    let (w, h) = (w as f32, h as f32);
                    let scale = (60.0 / w).min(60.0 / h);
                    let img = Rect::from_center_size(tile_rect.center(), Vec2::new(w * scale, h * scale));
                    ui.painter().image(tex.id(), img, Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)), Color32::WHITE);
                }
            }
            // 3. "Before" block (measured first so the main column knows its width)
            let bold = theme.body_bold();
            let body = theme.body();
            let caption = widgets::caption_galley(ui, "Before", theme.caption_font(), theme.secondary);
            let label_w = widgets::text_width(ui, &before.label, &bold);
            let loc_w = before.location.as_deref().map(|l| widgets::text_width(ui, l, &body)).unwrap_or(0.0);
            let before_w = caption.size().x.max(label_w).max(loc_w).min((width - tile - 32.0) * 0.5);
            let main_w = (ui.available_width() - before_w - 16.0).max(80.0);
            // 2. main column
            let name_font = theme.font_bold(16.5);
            let name_h = row_h(ui, &name_font).max(row_h(ui, &bold) + 4.0);
            let line_h = row_h(ui, &body).max(16.0);
            let cur_gen = gen.map(|g| g.get_generation()).unwrap_or(1);
            let show_meta = cur_gen >= 2 && last.is_some();
            let col_h = name_h + 6.0 + line_h + if show_meta { 6.0 + line_h } else { 0.0 };
            ui.allocate_ui_with_layout(Vec2::new(main_w, col_h), Layout::top_down(Align::Min), |ui| {
                ui.set_width(main_w);
                ui.spacing_mut().item_spacing.y = 6.0;
                let Some(l) = last else {
                    let (r, _) = ui.allocate_exact_size(Vec2::new(main_w, name_h), Sense::hover());
                    widgets::col_text(ui, r, "No Pokémon", name_font.clone(), theme.secondary, Align::Min);
                    return;
                };
                let p = &l.pkmn;
                // row 1: name + level pill
                let (r, _) = ui.allocate_exact_size(Vec2::new(main_w, name_h), Sense::hover());
                let name_w = paint_run(ui, r.min.x, r.center().y, &[(&p.name, name_font.clone(), theme.text_strong())], 0.0);
                let pill_rect = Rect::from_min_size(Pos2::new(r.min.x + name_w + 10.0, r.min.y), Vec2::new(main_w - name_w - 10.0, name_h));
                let mut pill_ui = ui.new_child(egui::UiBuilder::new().max_rect(pill_rect).layout(Layout::left_to_right(Align::Center)));
                widgets::pill(&mut pill_ui, theme, &format!("Lv {}", p.level), theme.primary, theme.pill_bg(), theme.pill_border());
                // row 2: ability · nature · held item
                if show_meta {
                    let (r, _) = ui.allocate_exact_size(Vec2::new(main_w, line_h), Sense::hover());
                    let held = format!("Held item: {}", p.held_item.as_deref().filter(|h| !h.is_empty()).unwrap_or("none"));
                    let nature = p.nature.display_name();
                    let mut segs: Vec<Seg> = Vec::new();
                    if cur_gen >= 3 {
                        segs.push((&p.ability, body.clone(), theme.text));
                        segs.push(("·", body.clone(), theme.secondary));
                        segs.push((&nature, body.clone(), theme.text));
                        segs.push(("·", body.clone(), theme.secondary));
                    }
                    segs.push((&held, body.clone(), theme.secondary));
                    paint_run(ui, r.min.x, r.center().y, &segs, 8.0);
                }
                // row 3: exp line with the progress bar
                let (r, _) = ui.allocate_exact_size(Vec2::new(main_w, line_h), Sense::hover());
                let exp = fmt_thousands(l.cur_xp);
                let left: [Seg; 2] = [("Exp", body.clone(), theme.secondary), (&exp, bold.clone(), theme.text)];
                let left_w = paint_run(ui, r.min.x, r.center().y, &left, 4.0);
                if has_state {
                    let (right, fraction): (Vec<Seg>, f32) = if l.xp_to_next_level <= 0 {
                        (vec![("Max level", body.clone(), theme.secondary)], 1.0)
                    } else {
                        (Vec::new(), (1.0 - l.percent_xp_to_next_level as f32 / 100.0).clamp(0.0, 1.0))
                    };
                    let to_next = fmt_thousands(l.xp_to_next_level);
                    let to_lv = format!("to Lv {}", p.level + 1);
                    let right: Vec<Seg> = if right.is_empty() { vec![(&to_next, bold.clone(), theme.text), (&to_lv, body.clone(), theme.secondary)] } else { right };
                    let right_w = seg_width(ui, &right, 4.0);
                    paint_run(ui, r.max.x - right_w, r.center().y, &right, 4.0);
                    let bar = Rect::from_min_max(Pos2::new(r.min.x + left_w + 10.0, r.center().y - 3.0), Pos2::new(r.max.x - right_w - 10.0, r.center().y + 3.0));
                    if bar.width() > 0.0 {
                        widgets::paint_progress_bar(ui, bar, fraction, theme.primary, theme.subtle_border);
                    }
                }
            });
            // 3. "Before" block, right-aligned and vertically centred
            let cap_h = caption.size().y;
            let before_h = cap_h + 3.0 + row_h(ui, &bold) + if before.location.is_some() { 3.0 + row_h(ui, &body) } else { 0.0 };
            let x1 = ui.max_rect().max.x;
            let x0 = x1 - before_w;
            let (region, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().max(before_w), tile), Sense::hover());
            let _ = region;
            let cy0 = tile_rect.center().y - before_h / 2.0;
            ui.painter().galley(Pos2::new(x1 - caption.size().x, cy0), caption.clone(), theme.secondary);
            let mut y = cy0 + cap_h + 3.0;
            let label = widgets::elide(ui, &before.label, &bold, before_w);
            widgets::col_text(ui, Rect::from_min_size(Pos2::new(x0, y), Vec2::new(before_w, row_h(ui, &bold))), &label, bold.clone(), theme.header, Align::Max);
            if let Some(loc) = &before.location {
                y += row_h(ui, &bold) + 3.0;
                let loc = widgets::elide(ui, loc, &body, before_w);
                widgets::col_text(ui, Rect::from_min_size(Pos2::new(x0, y), Vec2::new(before_w, row_h(ui, &body))), &loc, body.clone(), theme.secondary, Align::Max);
            }
        });
    });
}

// ---------------------------------------------------------------------------
// Stats card
// ---------------------------------------------------------------------------

struct StatRow {
    name: &'static str,
    now: i64,
    boosted: bool,
    gain: Option<i64>,
    realized: Option<i64>,
    total: Option<i64>,
}

fn stat_vals(gen1: bool, sb: &StatBlock) -> Vec<i64> {
    if gen1 {
        vec![sb.hp, sb.attack, sb.defense, sb.special_attack, sb.speed]
    } else {
        vec![sb.hp, sb.attack, sb.defense, sb.special_attack, sb.special_defense, sb.speed]
    }
}

/// The badge-boost markers of `pkmn.cur_stats` (including the Gen 2 Sp. Def
/// rule: no marker when the boost is ignored for the mon's Sp. Atk).
fn boost_flags(gen: &GenData, pkmn: &EnemyPkmn, badges: &BadgeList) -> Vec<bool> {
    let cur_gen = gen.get_generation();
    let spd = if badges.is_special_defense_boosted() {
        if cur_gen == 2 {
            let unboosted = pkmn.base_stats.calc_level_stats(pkmn.level, &pkmn.dvs, &pkmn.stat_xp, &gen.make_badge_list(), pkmn.nature, None).special_attack;
            !xpr_data::stats::should_ignore_spd_badge_boost(unboosted)
        } else {
            true
        }
    } else {
        false
    };
    if cur_gen == 1 {
        vec![false, badges.is_attack_boosted(), badges.is_defense_boosted(), badges.is_special_attack_boosted(), badges.is_speed_boosted()]
    } else {
        vec![false, badges.is_attack_boosted(), badges.is_defense_boosted(), badges.is_special_attack_boosted(), spd, badges.is_speed_boosted()]
    }
}

fn stat_rows(gen: &GenData, last: &LastPkmn, state: Option<&RouteState>) -> Vec<StatRow> {
    let gen1 = gen.get_generation() == 1;
    let names: &[&'static str] = if gen1 { &["HP", "Attack", "Defense", "Special", "Speed"] } else { &["HP", "Attack", "Defense", "Sp. Atk", "Sp. Def", "Speed"] };
    let now = stat_vals(gen1, &last.pkmn.cur_stats);
    let boosted = boost_flags(gen, &last.pkmn, &last.badges);
    // EV columns are unknown (`—`) without a state
    let opt = |sb: Option<StatBlock>| -> Vec<Option<i64>> {
        match sb {
            Some(sb) => stat_vals(gen1, &sb).into_iter().map(Some).collect(),
            None => vec![None; names.len()],
        }
    };
    let gain = opt(state.map(|s| s.solo_pkmn.get_net_gain_from_stat_xp(&s.badges)));
    let realized = opt(state.map(|s| s.solo_pkmn.realized_stat_xp));
    let total = opt(state.map(|s| s.solo_pkmn.unrealized_stat_xp));
    names
        .iter()
        .enumerate()
        .map(|(i, n)| StatRow { name: n, now: now[i], boosted: boosted[i], gain: gain[i], realized: realized[i], total: total[i] })
        .collect()
}

fn stats_inner_height(rows: usize) -> f32 {
    CARD_TITLE_H + TITLE_GAP + rows as f32 * STAT_ROW_H + CARD_FOOTER_H
}

/// Draws the Stats card; returns true when the "EVs real / total" column was
/// clicked this frame, so the caller can offer an EV override at this point.
pub fn stats_card(ui: &mut Ui, theme: &Theme, gen: Option<&GenData>, last: Option<&LastPkmn>, state: Option<&RouteState>, min_inner_h: f32) -> bool {
    let mut clicked_evs = false;
    widgets::card(ui, theme, None, |ui| {
        ui.set_min_height(min_inner_h);
        ui.spacing_mut().item_spacing.y = 0.0;
        let cur_gen = gen.map(|g| g.get_generation()).unwrap_or(3);
        let (heads, cols): ([&str; 3], [f32; 3]) = if cur_gen >= 3 {
            (["Now", "from EVs", "EVs real / total"], STAT_COLS_EV)
        } else {
            (["Now", "from StatExp", "StatExp real / total"], STAT_COLS_STATEXP)
        };
        let title = widgets::card_title(ui, theme, "Stats", None);
        // column heads, right-aligned over their columns
        let mut x = title.max.x;
        for (head, w) in heads.iter().zip(cols.iter()).rev() {
            let cell = Rect::from_min_max(Pos2::new(x - w, title.min.y), Pos2::new(x, title.max.y));
            widgets::col_text(ui, cell, head, theme.caption_font(), theme.secondary, Align::Max);
            x -= w;
        }
        ui.add_space(TITLE_GAP);
        let rows: Vec<StatRow> = match (gen, last) {
            (Some(g), Some(l)) => stat_rows(g, l, state),
            _ => Vec::new(),
        };
        let now_font = theme.font_bold(10.5);
        let body = theme.body();
        let divider = theme.row_divider();
        let dash = "—";
        for row in &rows {
            let r = widgets::table_row(ui, STAT_ROW_H, Some(divider));
            let mut x = r.max.x;
            // real / total
            let cell = Rect::from_min_max(Pos2::new(x - cols[2], r.min.y), Pos2::new(x, r.max.y));
            let rt = match (row.realized, row.total) {
                (Some(a), Some(b)) => format!("{} / {}", fmt_thousands(a), fmt_thousands(b)),
                _ => dash.to_string(),
            };
            if state.is_some() {
                let resp = ui.interact(cell, ui.id().with(("ev_cell", row.name)), Sense::click());
                if resp.clicked() {
                    clicked_evs = true;
                }
                resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Override the EVs from this point on");
            }
            widgets::col_text(ui, cell, &rt, body.clone(), theme.secondary, Align::Max);
            x -= cols[2];
            // from EVs
            let cell = Rect::from_min_max(Pos2::new(x - cols[1], r.min.y), Pos2::new(x, r.max.y));
            let gain = match row.gain {
                Some(g) => format!("{:+}", g),
                None => dash.to_string(),
            };
            widgets::col_text(ui, cell, &gain, body.clone(), theme.secondary, Align::Max);
            x -= cols[1];
            // Now (+ badge-boost dot)
            let cell = Rect::from_min_max(Pos2::new(x - cols[0], r.min.y), Pos2::new(x, r.max.y));
            let now = row.now.to_string();
            widgets::col_text(ui, cell, &now, now_font.clone(), theme.text_strong(), Align::Max);
            if row.boosted {
                let vw = widgets::text_width(ui, &now, &now_font);
                ui.painter().circle_filled(Pos2::new(cell.max.x - vw - 5.0 - 3.0, cell.center().y), 3.0, theme.header);
            }
            x -= cols[0];
            // name
            let cell = Rect::from_min_max(Pos2::new(r.min.x, r.min.y), Pos2::new(x, r.max.y));
            widgets::col_text(ui, cell, row.name, body.clone(), theme.text, Align::Min);
        }
        // footer: left text (badge-boost / no-EVs note) and the right-aligned
        // badges list must never overlap, so the right side is elided to
        // whatever width the left side leaves it.
        let r = widgets::table_row(ui, CARD_FOOTER_H, Some(divider));
        let cap = theme.caption_font();
        let any_boost = rows.iter().any(|r| r.boosted);
        let no_evs = state.is_some() && !rows.is_empty() && rows.iter().all(|r| r.total == Some(0));
        let cell = Rect::from_min_max(Pos2::new(r.min.x, r.min.y + 4.0), r.max);
        let left_w = if any_boost {
            ui.painter().circle_filled(Pos2::new(cell.min.x + 3.0, cell.center().y), 3.0, theme.header);
            let text_cell = Rect::from_min_max(Pos2::new(cell.min.x + 11.0, cell.min.y), cell.max);
            widgets::col_text(ui, text_cell, "Badge boost applied", cap.clone(), theme.secondary, Align::Min);
            11.0 + widgets::text_width(ui, "Badge boost applied", &cap)
        } else if no_evs {
            widgets::col_text(ui, cell, "No EVs earned yet", cap.clone(), theme.secondary, Align::Min);
            widgets::text_width(ui, "No EVs earned yet", &cap)
        } else {
            0.0
        };
        let badges_text = match last {
            Some(l) if l.badges.num_badges() > 0 => l.badges.to_short_string(),
            _ => "Badges: none".to_string(),
        };
        let right_max_w = (cell.width() - left_w - 12.0).max(20.0);
        let shown = widgets::elide(ui, &badges_text, &cap, right_max_w);
        widgets::col_text(ui, cell, &shown, cap, theme.secondary, Align::Max);
    });
    clicked_evs
}

// ---------------------------------------------------------------------------
// Moves card
// ---------------------------------------------------------------------------

fn moves_inner_height() -> f32 {
    CARD_TITLE_H + TITLE_GAP + 4.0 * MOVE_ROW_H
}

/// Draws the Moves card; returns the slot (0-3) clicked this frame, if any,
/// so the caller can offer to replace it via a Tutor event.
pub fn moves_card(ui: &mut Ui, theme: &Theme, last: Option<&LastPkmn>, min_inner_h: f32) -> Option<i64> {
    let mut moves: Vec<String> = last.map(|l| l.pkmn.move_list.iter().map(|m| m.clone().unwrap_or_default()).collect()).unwrap_or_default();
    moves.truncate(4);
    while moves.len() < 4 {
        moves.push(String::new());
    }
    let n = moves.iter().filter(|m| !m.is_empty()).count();
    let can_click = last.is_some();
    let mut clicked_slot: Option<i64> = None;
    widgets::card(ui, theme, None, |ui| {
        ui.set_min_height(min_inner_h);
        ui.spacing_mut().item_spacing.y = 0.0;
        widgets::card_title(ui, theme, "Moves", Some(&format!("{} of 4", n)));
        ui.add_space(TITLE_GAP);
        let divider = theme.row_divider();
        for (i, m) in moves.iter().enumerate() {
            let r = widgets::table_row(ui, MOVE_ROW_H, Some(divider));
            if can_click {
                let resp = ui.interact(r, ui.id().with(("move_row", i)), Sense::click());
                if resp.clicked() {
                    clicked_slot = Some(i as i64);
                }
                resp.on_hover_cursor(egui::CursorIcon::PointingHand);
            }
            let slot = Rect::from_min_max(r.min, Pos2::new(r.min.x + 14.0, r.max.y));
            widgets::col_text(ui, slot, &(i + 1).to_string(), theme.caption_font(), theme.secondary, Align::Min);
            let cell = Rect::from_min_max(Pos2::new(r.min.x + 24.0, r.min.y), r.max);
            if m.is_empty() {
                widgets::col_text(ui, cell, "—", theme.body(), theme.secondary, Align::Min);
            } else {
                let shown = widgets::elide(ui, m, &theme.body_bold(), cell.width());
                widgets::col_text(ui, cell, &shown, theme.body_bold(), theme.text_strong(), Align::Min);
            }
        }
    });
    clicked_slot
}

// ---------------------------------------------------------------------------
// Bag card
// ---------------------------------------------------------------------------

/// The rows the bag card draws: `(slot label, quantity, name)`; the overflow
/// row has no quantity.
fn bag_rows(inv: &Inventory) -> Vec<(String, Option<i64>, String)> {
    let n = inv.cur_items.len();
    let too_many = n > MAX_BAG_ROWS;
    let mut out = Vec::new();
    for (idx, it) in inv.cur_items.iter().enumerate().take(MAX_BAG_ROWS) {
        if too_many && idx == MAX_BAG_ROWS - 1 {
            out.push((format!("{:0>2}+", idx + 1), None, "More items…".to_string()));
        } else {
            out.push((format!("{:0>2}", idx + 1), Some(it.num), it.base_item.name.clone()));
        }
    }
    out
}

fn bag_inner_height(items: usize) -> f32 {
    let body = if items == 0 { EMPTY_BAG_H } else { items.min(MAX_BAG_ROWS) as f32 * BAG_ROW_H };
    CARD_TITLE_H + TITLE_GAP + MONEY_ROW_H + body
}

pub fn bag_card(ui: &mut Ui, theme: &Theme, inventory: Option<&Inventory>, min_inner_h: f32) {
    let rows = inventory.map(bag_rows).unwrap_or_default();
    let n_items = inventory.map(|i| i.cur_items.len()).unwrap_or(0).min(MAX_BAG_ROWS);
    let money = inventory.map(|i| i.cur_money).unwrap_or(0);
    widgets::card(ui, theme, None, |ui| {
        ui.set_min_height(min_inner_h);
        ui.spacing_mut().item_spacing.y = 0.0;
        widgets::card_title(ui, theme, "Bag", Some(&format!("{} of {} slots", n_items, MAX_BAG_ROWS)));
        ui.add_space(TITLE_GAP);
        let divider = theme.row_divider();
        let r = widgets::table_row(ui, MONEY_ROW_H, Some(divider));
        widgets::col_text(ui, r, "Money", theme.body(), theme.secondary, Align::Min);
        widgets::col_text(ui, r, &format!("${}", fmt_thousands(money)), theme.font_bold(13.5), theme.text_strong(), Align::Max);
        if rows.is_empty() {
            // centred empty state filling the rest of the card
            let h = (min_inner_h - CARD_TITLE_H - TITLE_GAP - MONEY_ROW_H).max(EMPTY_BAG_H);
            let r = widgets::table_row(ui, h, Some(divider));
            let text = "Bag is empty before this event";
            let tw = row_h(ui, &theme.body());
            let block_h = 26.0 + 6.0 + tw;
            let top = r.center().y - block_h / 2.0;
            let icon = Rect::from_min_size(Pos2::new(r.center().x - 13.0, top), Vec2::splat(26.0));
            widgets::paint_bag_glyph(ui, icon, theme.icon_stroke());
            let cell = Rect::from_min_max(Pos2::new(r.min.x, top + 26.0 + 6.0), Pos2::new(r.max.x, top + block_h));
            widgets::col_text(ui, cell, text, theme.body(), theme.secondary, Align::Center);
            return;
        }
        for (slot, qty, name) in &rows {
            let r = widgets::table_row(ui, BAG_ROW_H, Some(divider));
            let slot_cell = Rect::from_min_max(r.min, Pos2::new(r.min.x + 18.0, r.max.y));
            widgets::col_text(ui, slot_cell, slot, theme.caption_font(), theme.secondary, Align::Min);
            let qty_cell = Rect::from_min_max(Pos2::new(r.min.x + 18.0 + 10.0, r.min.y), Pos2::new(r.min.x + 18.0 + 10.0 + 26.0, r.max.y));
            if let Some(q) = qty {
                widgets::col_text(ui, qty_cell, &format!("{}×", q), theme.body(), theme.secondary, Align::Max);
            }
            let name_cell = Rect::from_min_max(Pos2::new(qty_cell.max.x + 10.0, r.min.y), r.max);
            let shown = widgets::elide(ui, name, &theme.body(), name_cell.width());
            let color = if qty.is_some() { theme.contrast } else { theme.secondary };
            widgets::col_text(ui, name_cell, &shown, theme.body(), color, Align::Min);
        }
    });
}

// ---------------------------------------------------------------------------
// The whole read-only block
// ---------------------------------------------------------------------------

/// Header card + the cards row. Updates `last_pkmn` from `state` when there
/// is one, and otherwise keeps showing the last mon (with `—` in the EV
/// columns and an empty bag).
/// What the Pre-Event State cards were clicked on this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct StateViewerClicks {
    /// a move slot (0-3) of the Moves card
    pub move_slot: Option<i64>,
    /// the "EVs real / total" column of the Stats card
    pub evs: bool,
}

pub fn state_viewer(ui: &mut Ui, theme: &Theme, gen: Option<&GenData>, state: Option<&RouteState>, last_pkmn: &mut Option<LastPkmn>, assets: &mut Assets, before: &BeforeInfo) -> StateViewerClicks {
    if let Some(s) = state {
        *last_pkmn = Some(LastPkmn {
            pkmn: s.solo_pkmn.get_pkmn_obj(&s.badges, None),
            badges: s.badges.clone(),
            cur_xp: s.solo_pkmn.cur_xp,
            xp_to_next_level: s.solo_pkmn.xp_to_next_level,
            percent_xp_to_next_level: s.solo_pkmn.percent_xp_to_next_level,
        });
    }
    let last = last_pkmn.as_ref();
    identity_header(ui, theme, gen, last, state.is_some(), assets, before);
    ui.add_space(CARD_GAP);

    let n_stat_rows = if gen.map(|g| g.get_generation()).unwrap_or(3) == 1 { 5 } else { 6 };
    let n_items = state.map(|s| s.inventory.cur_items.len()).unwrap_or(0);
    let inventory = state.map(|s| &s.inventory);
    let content_w = ui.available_width();
    let two_rows = content_w - 2.0 * CARD_GAP < 360.0 + 180.0 + 240.0;
    let column = |ui: &mut Ui, w: f32, add: &mut dyn FnMut(&mut Ui)| {
        ui.allocate_ui_with_layout(Vec2::new(w, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(w);
            add(ui);
        });
    };
    let mut clicks = StateViewerClicks::default();
    if two_rows {
        let avail = content_w - CARD_GAP;
        let stats_w = (avail * 0.65).max(360.0);
        let moves_w = (avail - stats_w).max(180.0);
        let h = stats_inner_height(n_stat_rows).max(moves_inner_height());
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = CARD_GAP;
            column(ui, stats_w, &mut |ui| clicks.evs = stats_card(ui, theme, gen, last, state, h));
            column(ui, moves_w, &mut |ui| clicks.move_slot = moves_card(ui, theme, last, h));
        });
        ui.add_space(CARD_GAP);
        bag_card(ui, theme, inventory, bag_inner_height(n_items));
    } else {
        let avail = content_w - 2.0 * CARD_GAP;
        let stats_w = (avail * 0.443).max(360.0);
        let moves_w = (avail * 0.237).max(180.0);
        let bag_w = (avail - stats_w - moves_w).max(240.0);
        let h = stats_inner_height(n_stat_rows).max(moves_inner_height()).max(bag_inner_height(n_items));
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = CARD_GAP;
            column(ui, stats_w, &mut |ui| clicks.evs = stats_card(ui, theme, gen, last, state, h));
            column(ui, moves_w, &mut |ui| clicks.move_slot = moves_card(ui, theme, last, h));
            column(ui, bag_w, &mut |ui| bag_card(ui, theme, inventory, h));
        });
    }
    clicks
}
