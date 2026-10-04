//! The middle column: the trainer's header, group variants, party cards and
//! the type effectiveness summary (Solodex `TrainerDetail.tsx`).

use egui::{Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Ui, UiBuilder, Vec2};
use xpr_data::GenData;
use xpr_ui_kit::theme::{self, Theme};
use xpr_ui_kit::widgets::fmt_thousands;

use super::card::{self, effect_label, CardEnv, CardLayout, MIN_CARD_W};
use super::data::{TrainerInfo, VersionIndex};
use super::order::supported_gen;
use super::order_ui::OrderUi;
use crate::palette::{self, px, px_bold};
use crate::widgets;
use crate::{DexAction, DexCx};

#[derive(Default)]
pub struct DetailState {
    /// the shown variant of the selected group
    pub group_index: usize,
    last_selected: Option<usize>,
    /// the team order calculator, while open
    pub order: Option<OrderUi>,
}

/// `VioletGym` -> `Violet Gym`, `Route_1` -> `Route 1` (Solodex `formatLocation`).
pub fn format_location(loc: &str) -> String {
    let mut out = String::with_capacity(loc.len() + 4);
    let mut prev_lower = false;
    for c in loc.chars() {
        if prev_lower && c.is_ascii_uppercase() {
            out.push(' ');
        }
        prev_lower = c.is_ascii_lowercase();
        out.push(if c == '_' { ' ' } else { c });
    }
    out
}

/// Cards per row: Solodex's (one per Pokémon up to 3, 3 from four on), fewer
/// when the column is too narrow for a readable card.
pub fn columns_for(party: usize, width: f32) -> usize {
    let wanted = if party >= 4 { 3 } else { party.max(1) };
    let fit = ((width + 12.0) / (MIN_CARD_W + 12.0)).floor().max(1.0) as usize;
    wanted.min(fit)
}

/// The type effectiveness table: for every attacking type (in the Dex
/// chart's order for the version's game), the multiplier against each party
/// member and the total, sorted by total descending (Solodex `TeamTypeSummary`).
pub fn type_summary(gen: &GenData, dex_game: Option<&str>, party: &[String]) -> Vec<(String, Vec<f64>, f64)> {
    let mult = |atk: &str, def: &str| match gen.effectiveness(atk, def) {
        Some(xpr_core::consts::SUPER_EFFECTIVE) => 2.0,
        Some(xpr_core::consts::NOT_VERY_EFFECTIVE) => 0.5,
        Some(xpr_core::consts::IMMUNE) => 0.0,
        _ => 1.0,
    };
    let typings: Vec<Option<(String, Option<String>)>> = party
        .iter()
        .map(|name| {
            gen.pkmn_db().get_pkmn(name).map(|s| {
                let t2 = (s.second_type != s.first_type && !s.second_type.is_empty()).then(|| s.second_type.clone());
                (s.first_type.clone(), t2)
            })
        })
        .collect();
    if typings.iter().all(|t| t.is_none()) {
        return Vec::new();
    }
    // attacking types: the Dex chart's order, then any the router has beyond it
    let router: Vec<String> = gen.supported_types().into_iter().filter(|t| t != "none").collect();
    let mut attack: Vec<String> = Vec::new();
    if let Some(g) = dex_game {
        for t in xpr_dex::types_for_game(g) {
            if router.contains(&t) {
                attack.push(t);
            }
        }
    }
    for t in &router {
        if !attack.contains(t) {
            attack.push(t.clone());
        }
    }
    let mut rows: Vec<(String, Vec<f64>, f64)> = attack
        .into_iter()
        .map(|atk| {
            let ms: Vec<f64> = typings
                .iter()
                .map(|t| match t {
                    Some((t1, t2)) => mult(&atk, t1) * t2.as_deref().map(|t2| mult(&atk, t2)).unwrap_or(1.0),
                    None => 1.0,
                })
                .collect();
            let total = ms.iter().sum();
            (atk, ms, total)
        })
        .collect();
    rows.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    rows
}

fn eff_colors(m: f64) -> (Color32, Color32) {
    // (text, background) from the attacker's perspective
    if m >= 2.0 {
        (palette::hex("#86efac"), palette::hex("#14532d"))
    } else if m == 1.0 {
        (palette::hex("#9ca3af"), palette::hex("#374151"))
    } else if m > 0.0 {
        (palette::hex("#fca5a5"), palette::hex("#7f1d1d"))
    } else {
        (palette::hex("#6b7280"), palette::hex("#1f2937"))
    }
}

/// A gray button of the detail header's button row.
fn header_button(ui: &mut Ui, theme: &Theme, pos: Pos2, text: &str, enabled: bool, tip: &str) -> (Rect, bool) {
    let font = px_bold(theme, 12.0);
    let w = widgets::text_w(ui, text, &font) + 16.0;
    let rect = Rect::from_min_size(pos, Vec2::new(w, 24.0));
    let resp = ui.interact(rect, ui.id().with(("trainer_btn", text)), if enabled { Sense::click() } else { Sense::hover() });
    let (bg, fg) = if !enabled {
        (palette::GRAY_800, palette::GRAY_600)
    } else if resp.hovered() {
        (palette::GRAY_600, Color32::WHITE)
    } else {
        (palette::GRAY_700, palette::GRAY_300)
    };
    ui.painter().rect_filled(rect, CornerRadius::same(4), bg);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, font, fg);
    let resp = if tip.is_empty() { resp } else { resp.on_hover_text(tip) };
    let clicked = enabled && resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked();
    (rect, clicked)
}

fn pill(ui: &mut Ui, pos: Pos2, text: &str, font: egui::FontId, fg: Color32, bg: Color32, border: Color32, pad_x: f32) -> f32 {
    let w = widgets::text_w(ui, text, &font) + pad_x * 2.0 + 2.0;
    let rect = Rect::from_min_size(pos, Vec2::new(w, 22.0));
    ui.painter().rect(rect, CornerRadius::same(11), bg, Stroke::new(1.0_f32, border), egui::StrokeKind::Inside);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, font, fg);
    w
}

/// Draw the detail of the selected trainer in `ui`'s rect.
pub fn detail_ui(ui: &mut Ui, cx: &mut DexCx, ix: &VersionIndex, selected: Option<usize>, st: &mut DetailState) {
    let theme = cx.theme;
    let full = ui.available_rect_before_wrap();
    let Some(sel) = selected else {
        widgets::placeholder(ui, theme, "Select a trainer");
        return;
    };
    // the shown variant starts at the selected member and resets when the selection changes
    let group = ix.group_of(sel);
    if st.last_selected != Some(sel) {
        st.last_selected = Some(sel);
        st.group_index = group.and_then(|g| g.members.iter().position(|m| *m == sel)).unwrap_or(0);
    }
    let members: Vec<usize> = group.map(|g| g.members.clone()).unwrap_or_default();
    let active = if members.is_empty() { sel } else { members[st.group_index.min(members.len() - 1)] };
    let t: &TrainerInfo = &ix.trainers[active];
    let w = full.width();
    let mut y = full.min.y;

    // ---- header: name, class, double battle | location, money, party, xp
    let pad = 24.0;
    let f20 = px_bold(theme, 20.0);
    let f14 = px(theme, 14.0);
    let name = &t.trainer.name;
    let name_w = widgets::text_w(ui, name, &f20);
    let pill_color = t.pill_color();
    let class_font = px_bold(theme, 12.0);
    let class_w = widgets::text_w(ui, &t.trainer.trainer_class, &class_font) + 22.0;
    let double_w = if t.trainer.double_battle { 12.0 + widgets::text_w(ui, "Double", &class_font) + 18.0 } else { 0.0 };
    let left_w = name_w + 12.0 + class_w + double_w;
    let avg = if t.party.is_empty() { 0 } else { (t.party.iter().map(|p| p.level).sum::<i64>() as f64 / t.party.len() as f64).round() as i64 };
    let total_xp: i64 = t.trainer.pkmn.iter().map(|p| p.xp).sum();
    let mut info: Vec<(String, Color32)> = Vec::new();
    if !t.trainer.location.is_empty() {
        info.push((format_location(&t.trainer.location), palette::GRAY_500));
    }
    if t.trainer.money > 0 {
        info.push((format!("${}", fmt_thousands(t.trainer.money)), Color32::from_rgba_unmultiplied(0xfa, 0xcc, 0x15, 178)));
    }
    info.push((format!("{} Pokemon, avg Lv{}", t.party.len(), avg), palette::GRAY_500));
    if total_xp > 0 {
        info.push((format!("{} total xp", fmt_thousands(total_xp)), palette::GRAY_500));
    }
    let info_w: f32 = info.iter().map(|(s, _)| widgets::text_w(ui, s, &f14)).sum::<f32>() + 20.0 * (info.len() - 1) as f32;
    let one_row = pad + left_w + 16.0 + info_w + pad <= w;
    let head_h = if one_row { 12.0 + 28.0 + 12.0 } else { 12.0 + 28.0 + 4.0 + 20.0 + 12.0 };
    let cy = y + 12.0 + 14.0;
    ui.painter().text(Pos2::new(full.min.x + pad, cy), Align2::LEFT_CENTER, name, f20, Color32::WHITE);
    let mut pill_x = full.min.x + pad + name_w + 12.0;
    let pw = pill(ui, Pos2::new(pill_x, cy - 11.0), &t.trainer.trainer_class, class_font.clone(), pill_color, theme::with_alpha(pill_color, 32), theme::with_alpha(pill_color, 64), 10.0);
    pill_x += pw;
    if t.trainer.double_battle {
        pill_x += 12.0;
        pill(ui, Pos2::new(pill_x, cy - 11.0), "Double", class_font, palette::PURPLE_400, Color32::from_rgba_unmultiplied(0x4c, 0x1d, 0x95, 77), Color32::from_rgba_unmultiplied(0x7e, 0x22, 0xce, 128), 8.0);
    }
    let (mut info_x, info_y) = if one_row { (full.max.x - pad - info_w, cy) } else { (full.min.x + pad, y + 12.0 + 28.0 + 4.0 + 10.0) };
    for (s, c) in &info {
        let r = ui.painter().text(Pos2::new(info_x, info_y), Align2::LEFT_CENTER, s, f14.clone(), *c);
        info_x = r.max.x + 20.0;
    }
    y += head_h;
    ui.painter().hline(full.x_range(), y + 0.5, Stroke::new(1.0_f32, palette::GRAY_800));
    y += 1.0;

    // ---- variants + buttons
    let version = ix.version.clone();
    let can_add = cx.route.version.as_deref() == Some(version.as_str());
    let order_gen = supported_gen(ix.generation);
    let mut btns: Vec<(&str, bool, &str)> = vec![("Add to Route", can_add, if can_add { "Insert this fight into the open route" } else { "Open a route of this version to add the fight" }), ("Calc Damage", true, "Open the Damage tab against this trainer")];
    if order_gen.is_some() {
        btns.push(("Team Order", true, "Predict the order this trainer will send out their Pok\u{e9}mon"));
    }
    let f12b = px_bold(theme, 12.0);
    let btn_ws: Vec<f32> = btns.iter().map(|(s, _, _)| widgets::text_w(ui, s, &f12b) + 16.0).collect();
    let btns_w: f32 = btn_ws.iter().sum::<f32>() + 8.0 * (btns.len() - 1) as f32;
    let avail = w - 2.0 * pad;
    let variant_font = px(theme, 12.0);
    let variant_ws: Vec<f32> = members.iter().map(|m| widgets::text_w(ui, &ix.trainers[*m].trainer.name, &variant_font) + 24.0).collect();
    let show_variants = members.len() > 1;
    let beside = !show_variants || avail - btns_w - 12.0 >= 260.0;
    let var_avail = if beside { avail - btns_w - 12.0 } else { avail };
    // wrap the variant buttons
    let mut var_rows: Vec<Vec<usize>> = vec![Vec::new()];
    if show_variants {
        let mut x = 0.0;
        for (i, vw) in variant_ws.iter().enumerate() {
            if x > 0.0 && x + 4.0 + vw > var_avail {
                var_rows.push(Vec::new());
                x = 0.0;
            }
            x += if x > 0.0 { 4.0 + vw } else { *vw };
            var_rows.last_mut().unwrap().push(i);
        }
    } else {
        var_rows.clear();
    }
    y += 12.0;
    let btn_y = y;
    let mut bx = full.max.x - pad - btns_w;
    let mut clicked_btn: Option<usize> = None;
    for (i, (label, enabled, tip)) in btns.iter().enumerate() {
        let (r, clicked) = header_button(ui, theme, Pos2::new(bx, btn_y), label, *enabled, tip);
        bx = r.max.x + 8.0;
        if clicked {
            clicked_btn = Some(i);
        }
    }
    let mut vy = if beside { y } else { y + 24.0 + 6.0 };
    for row in &var_rows {
        let mut x = full.min.x + pad;
        for &i in row {
            let r = Rect::from_min_size(Pos2::new(x, vy), Vec2::new(variant_ws[i], 24.0));
            let resp = ui.interact(r, ui.id().with(("trainer_variant", i)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            let on = i == st.group_index.min(members.len() - 1);
            let (bg, fg) = if on {
                (palette::BLUE_600, Color32::WHITE)
            } else if resp.hovered() {
                (palette::GRAY_800, Color32::WHITE)
            } else {
                (palette::GRAY_800, palette::GRAY_400)
            };
            ui.painter().rect_filled(r, CornerRadius::same(4), bg);
            ui.painter().text(r.center(), Align2::CENTER_CENTER, &ix.trainers[members[i]].trainer.name, variant_font.clone(), fg);
            if resp.clicked() {
                st.group_index = i;
            }
            x += variant_ws[i] + 4.0;
        }
        vy += 24.0 + 4.0;
    }
    let rows_h = if show_variants { var_rows.len() as f32 * 28.0 - 4.0 } else { 0.0 };
    y += if beside { rows_h.max(24.0) } else { 24.0 + 6.0 + rows_h };
    if let Some(i) = clicked_btn {
        match btns[i].0 {
            "Add to Route" => cx.actions.push(DexAction::AddTrainerToRoute { version: version.clone(), trainer: t.trainer.name.clone() }),
            "Calc Damage" => {
                let species = cx.route.solo_species.clone();
                cx.state.request_damage(Some(version.clone()), species, Some(t.trainer.name.clone()), Vec::new());
            }
            "Team Order" => st.order = Some(OrderUi::default()),
            _ => {}
        }
    }

    // ---- party + summary
    let summary = type_summary(&ix.gen, ix.dex_game, &t.party.iter().map(|p| p.name.clone()).collect::<Vec<_>>());
    let party_n = t.party.len();
    let sum_layout = SummaryLayout::compute(&summary, party_n, w - 2.0 * pad, ix.generation);
    let sum_h = if summary.is_empty() { 0.0 } else { 1.0 + 12.0 + 24.0 + sum_layout.height + 12.0 };
    let body_top = y + 12.0;
    let remaining = full.max.y - body_top;
    // pin the summary under the cards unless it would take over the column
    let pinned = !summary.is_empty() && sum_h <= remaining * 0.30;
    let scroll_rect = Rect::from_min_max(Pos2::new(full.min.x, y), Pos2::new(full.max.x, if pinned { full.max.y - sum_h } else { full.max.y }));
    let env = CardEnv { theme, ix };
    let cols = columns_for(party_n, w - 2.0 * pad);
    let card_w = ((w - 2.0 * pad) - 12.0 * (cols.saturating_sub(1)) as f32) / cols as f32;
    let mut sui = ui.new_child(UiBuilder::new().max_rect(scroll_rect).layout(egui::Layout::top_down(egui::Align::Min)));
    sui.set_clip_rect(scroll_rect.intersect(ui.clip_rect()));
    let images = &mut *cx.images;
    egui::ScrollArea::vertical().id_salt("trainer_party_scroll").auto_shrink([false, false]).show(&mut sui, |ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        ui.add_space(16.0);
        let layouts: Vec<CardLayout> = t.trainer.pkmn.iter().zip(t.party.iter()).map(|(e, m)| card::measure(ui, &env, e, m, card_w)).collect();
        let mut index = 0;
        while index < party_n {
            let end = (index + cols).min(party_n);
            let row_h = layouts[index..end].iter().map(|l| l.height).fold(0.0, f32::max);
            let (row, _) = ui.allocate_exact_size(Vec2::new(w, row_h), Sense::hover());
            for (k, i) in (index..end).enumerate() {
                let r = Rect::from_min_size(Pos2::new(row.min.x + pad + k as f32 * (card_w + 12.0), row.min.y), Vec2::new(card_w, row_h));
                card::draw(ui, &env, images, &layouts[i], &t.trainer.pkmn[i], &t.party[i], i, r);
            }
            ui.add_space(12.0);
            index = end;
        }
        ui.add_space(4.0);
        if !pinned && !summary.is_empty() {
            summary_ui(ui, theme, &env, &summary, &sum_layout, w);
        }
    });
    if pinned {
        let r = Rect::from_min_max(Pos2::new(full.min.x, full.max.y - sum_h), full.max);
        let mut bui = ui.new_child(UiBuilder::new().max_rect(r).layout(egui::Layout::top_down(egui::Align::Min)));
        summary_ui(&mut bui, theme, &env, &summary, &sum_layout, w);
    }

    // ---- the team order calculator
    if st.order.is_some() {
        if let Some(gen_n) = order_gen {
            let party: Vec<super::order::OrderMon> = t.trainer.pkmn.iter().map(|p| super::order::OrderMon { species: p.name.clone(), moves: p.move_list.iter().flatten().cloned().collect() }).collect();
            let close = super::order_ui::show(ui.ctx(), theme, &mut *cx.images, ix, t, &party, gen_n, st.order.get_or_insert_with(OrderUi::default));
            if close {
                st.order = None;
            }
        } else {
            st.order = None;
        }
    }
}

/// Where the summary's columns and cells go.
pub struct SummaryLayout {
    cols: usize,
    cell_w: f32,
    gap: f32,
    per_col: usize,
    pub height: f32,
}

impl SummaryLayout {
    pub fn compute(rows: &[(String, Vec<f64>, f64)], party: usize, avail: f32, generation: u8) -> SummaryLayout {
        let n = rows.len();
        let wanted = if generation <= 5 { 3 } else { 2 };
        let mut best = (1usize, 24.0_f32, 2.0_f32);
        'outer: for (cell_w, gap) in [(28.0_f32, 4.0_f32), (24.0, 2.0)] {
            for cols in (1..=wanted).rev() {
                let col_w = 68.0 + gap + party as f32 * (cell_w + gap);
                if cols as f32 * col_w + (cols - 1) as f32 * 24.0 <= avail {
                    best = (cols, cell_w, gap);
                    break 'outer;
                }
            }
        }
        let cols = best.0.min(wanted);
        let per_col = if n == 0 { 0 } else { n.div_ceil(cols) };
        let height = if per_col == 0 { 0.0 } else { per_col as f32 * 22.0 + (per_col - 1) as f32 * 4.0 };
        SummaryLayout { cols, cell_w: best.1, gap: best.2, per_col, height }
    }
}

fn summary_ui(ui: &mut Ui, theme: &Theme, env: &CardEnv, rows: &[(String, Vec<f64>, f64)], lay: &SummaryLayout, w: f32) {
    let h = 1.0 + 12.0 + 24.0 + lay.height + 12.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    ui.painter().hline(rect.x_range(), rect.min.y + 0.5, Stroke::new(1.0_f32, palette::GRAY_800));
    let title_y = rect.min.y + 1.0 + 12.0 + 8.0;
    let g = xpr_ui_kit::widgets::caption_galley(ui, "Type Effectiveness", px_bold(theme, 12.0), palette::GRAY_500);
    ui.painter().galley(Pos2::new(rect.center().x - g.size().x / 2.0, title_y - g.size().y / 2.0), g, palette::GRAY_500);
    let party = rows.first().map(|r| r.1.len()).unwrap_or(0);
    let col_w = 68.0 + lay.gap + party as f32 * (lay.cell_w + lay.gap);
    let total_w = lay.cols as f32 * col_w + (lay.cols - 1) as f32 * 24.0;
    let x0 = rect.center().x - total_w / 2.0;
    let top = rect.min.y + 1.0 + 12.0 + 24.0;
    let f12b = px_bold(theme, 12.0);
    for c in 0..lay.cols {
        let cx_ = x0 + c as f32 * (col_w + 24.0);
        for (k, row) in rows.iter().skip(c * lay.per_col).take(lay.per_col).enumerate() {
            let ry = top + k as f32 * 26.0;
            let badge = Rect::from_min_size(Pos2::new(cx_, ry + 2.0), Vec2::new(68.0, 18.0));
            let mut child = ui.new_child(UiBuilder::new().id_salt(("trainer_sum_type", c, k)).max_rect(badge));
            widgets::type_badge(&mut child, theme, &row.0, true, env.ix.dex_game);
            for (i, m) in row.1.iter().enumerate() {
                let (fg, bg) = eff_colors(*m);
                let cell = Rect::from_min_size(Pos2::new(cx_ + 68.0 + lay.gap + i as f32 * (lay.cell_w + lay.gap), ry), Vec2::new(lay.cell_w, 22.0));
                ui.painter().rect_filled(cell, CornerRadius::same(4), bg);
                ui.painter().text(cell.center(), Align2::CENTER_CENTER, effect_label(*m), f12b.clone(), fg);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locations_get_spaces() {
        assert_eq!(format_location("VioletGym"), "Violet Gym");
        assert_eq!(format_location("Route_3"), "Route 3");
        assert_eq!(format_location("Route 03"), "Route 03");
    }

    #[test]
    fn card_columns() {
        assert_eq!(columns_for(1, 1000.0), 1);
        assert_eq!(columns_for(2, 1000.0), 2);
        assert_eq!(columns_for(3, 1000.0), 3);
        assert_eq!(columns_for(6, 1000.0), 3);
        // narrow columns drop to what fits
        assert_eq!(columns_for(6, 470.0), 1);
        assert_eq!(columns_for(6, 700.0), 2);
        assert_eq!(columns_for(0, 700.0), 1);
    }
}

#[cfg(test)]
mod summary_tests {
    use super::*;
    use crate::testkit::test_registry;

    #[test]
    fn type_summary_multiplies_both_types_and_sorts_by_total() {
        let reg = test_registry();
        let gen = reg.get_version("Crystal").unwrap();
        // Pidgey and Pidgeotto: Normal / Flying
        let rows = type_summary(&gen, Some("Crystal"), &["Pidgey".to_string(), "Pidgeotto".to_string()]);
        let get = |t: &str| rows.iter().find(|r| r.0 == t).unwrap_or_else(|| panic!("{} missing", t)).clone();
        assert_eq!(get("Electric").1, vec![2.0, 2.0]);
        assert_eq!(get("Rock").1, vec![2.0, 2.0]);
        assert_eq!(get("Ice").1, vec![2.0, 2.0]);
        assert_eq!(get("Ground").1, vec![0.0, 0.0], "Flying is immune to Ground");
        assert_eq!(get("Ghost").1, vec![0.0, 0.0], "Normal is immune to Ghost");
        assert_eq!(get("Fighting").1, vec![1.0, 1.0], "Normal weak, Flying resistant");
        assert_eq!(get("Bug").1, vec![0.5, 0.5]);
        assert_eq!(get("Grass").1, vec![0.5, 0.5]);
        // sorted by total, highest first
        assert!(rows.windows(2).all(|w| w[0].2 >= w[1].2));
        assert_eq!(rows.first().unwrap().2, 4.0);
        assert_eq!(rows.last().unwrap().2, 0.0);
        // 17 attacking types in gen 2 (no "none")
        assert_eq!(rows.len(), 17);
        // gen 1 has 15
        let red = reg.get_version("Red").unwrap();
        assert_eq!(type_summary(&red, Some("Red and Blue"), &["Geodude".to_string()]).len(), 15);
        // a species the table does not know counts as neutral; none known: no table
        let rows = type_summary(&gen, Some("Crystal"), &["Pidgey".to_string(), "Nope".to_string()]);
        assert!(rows.iter().all(|r| r.1[1] == 1.0));
        assert!(type_summary(&gen, Some("Crystal"), &["Nope".to_string()]).is_empty());
    }

    #[test]
    fn summary_layout_shrinks_to_fit() {
        let rows: Vec<(String, Vec<f64>, f64)> = (0..17).map(|i| (format!("T{}", i), vec![1.0; 6], 6.0)).collect();
        // wide: three columns, six rows
        let l = SummaryLayout::compute(&rows, 6, 1000.0, 4);
        assert_eq!((l.cols, l.per_col), (3, 6));
        assert_eq!(l.height, 6.0 * 22.0 + 5.0 * 4.0);
        // narrow: narrower cells, then fewer columns
        let l = SummaryLayout::compute(&rows, 6, 470.0, 4);
        assert!(l.cols < 3 && l.cols >= 1);
        // a small party fits three columns even in a narrow column
        let l = SummaryLayout::compute(&rows, 2, 470.0, 4);
        assert_eq!(l.cols, 3);
    }
}
