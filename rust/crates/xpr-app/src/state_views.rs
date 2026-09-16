//! The pre-event state panel: `StateViewer` (Pokémon section, stat-EXP
//! section, inventory section), built from `StatColumn` / `PkmnViewer` /
//! `StatExpViewer` / `InventoryViewer`.

use egui::{Color32, CornerRadius, FontId, Pos2, Sense, Ui, Vec2};

use xpr_data::model::{EnemyPkmn, StatBlock};
use xpr_data::{BadgeList, GenData};
use xpr_engine::{Inventory, RouteState};
use xpr_ui_kit::theme::Theme;
use xpr_ui_kit::widgets;

/// One `StatColumn`: rounded section, optional centred header, label/value rows.
pub struct StatColumnSpec<'a> {
    pub header: &'a str,
    pub rows: Vec<(String, String, Option<&'static str>)>,
    /// Qt `label_width * 8` / `val_width * 8` minimums
    pub label_width: Option<f32>,
    pub val_width: Option<f32>,
    pub style_prefix: &'a str,
    pub font: FontId,
}

pub fn stat_column(ui: &mut Ui, theme: &Theme, spec: &StatColumnSpec) {
    let bg = theme.tinted_bg(spec.style_prefix, 0.15);
    let font = spec.font.clone();
    widgets::rounded_section(ui, bg, 6, egui::Margin { left: 8, right: 8, top: 6, bottom: 6 }, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let label_w = spec.label_width.unwrap_or(0.0);
        let val_w = spec.val_width.unwrap_or(0.0);
        // widest label/value so the rows line up
        let max_label = spec.rows.iter().map(|(l, _, _)| widgets::text_width(ui, l, &font)).fold(0.0f32, f32::max).max(label_w);
        let max_val = spec.rows.iter().map(|(_, v, _)| widgets::text_width(ui, v, &font)).fold(0.0f32, f32::max).max(val_w);
        let row_w = (max_label + 2.0 + max_val).max(ui.available_width().min(max_label + max_val + 2.0));
        let row_w = row_w.max(if spec.header.is_empty() { 0.0 } else { widgets::text_width(ui, spec.header, &font) });
        if !spec.header.is_empty() {
            let lines: Vec<&str> = spec.header.split('\n').collect();
            for line in lines {
                let (r, _) = ui.allocate_exact_size(Vec2::new(row_w, font.size * 1.3), Sense::hover());
                ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, line, font.clone(), theme.text);
            }
        }
        for (label, value, style) in &spec.rows {
            let color = match style {
                Some(s) => theme.style_color(s),
                None => theme.text,
            };
            let (r, _) = ui.allocate_exact_size(Vec2::new(row_w, font.size * 1.3 + 2.0), Sense::hover());
            let text_rect = r.shrink2(Vec2::new(0.0, 1.0));
            ui.painter().text(Pos2::new(text_rect.min.x, text_rect.center().y), egui::Align2::LEFT_CENTER, label, font.clone(), color);
            ui.painter().text(Pos2::new(text_rect.max.x, text_rect.center().y), egui::Align2::RIGHT_CENTER, value, font.clone(), color);
        }
    });
}

/// `PkmnViewer.set_pkmn` layout data.
pub struct PkmnViewerArgs<'a> {
    pub pkmn: &'a EnemyPkmn,
    pub badges: Option<&'a BadgeList>,
    pub speed_style: Option<&'static str>,
    pub stats_only: bool,
    pub font_size: Option<f32>,
}

/// The `PkmnViewer` (name / ability / held item tinted labels, stat and move columns).
pub fn pkmn_viewer(ui: &mut Ui, theme: &Theme, gen: &GenData, args: &PkmnViewerArgs) {
    let font = match args.font_size {
        Some(s) => theme.font(s),
        None => theme.body(),
    };
    let header_bg = theme.tinted_bg("Header", 0.25);
    let header_color = theme.header;
    let pkmn = args.pkmn;
    ui.spacing_mut().item_spacing = Vec2::new(2.0, 2.0);
    let tinted_label = |ui: &mut Ui, text: &str| {
        let galley = ui.fonts_mut(|f| f.layout_no_wrap(text.to_string(), font.clone(), Color32::WHITE));
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().max(galley.size().x + 8.0), galley.size().y + 4.0), Sense::hover());
        ui.painter().rect_filled(rect, CornerRadius::same(2), header_bg);
        ui.painter().galley(Pos2::new(rect.min.x + 4.0, rect.center().y - galley.size().y / 2.0), galley, header_color);
    };
    tinted_label(ui, &pkmn.name);
    if gen.get_generation() >= 3 {
        tinted_label(ui, &format!("{} ({})", pkmn.ability, pkmn.nature.display_name()));
    }
    if gen.get_generation() >= 2 {
        tinted_label(ui, &format!("Held Item: {}", pkmn.held_item.as_deref().unwrap_or("None")));
    }

    let mut attack_val = pkmn.cur_stats.attack.to_string();
    let mut defense_val = pkmn.cur_stats.defense.to_string();
    let mut spa_val = pkmn.cur_stats.special_attack.to_string();
    let mut spd_val = pkmn.cur_stats.special_defense.to_string();
    let mut speed_val = pkmn.cur_stats.speed.to_string();
    if let Some(b) = args.badges {
        if b.is_attack_boosted() {
            attack_val = format!("*{}", attack_val);
        }
        if b.is_defense_boosted() {
            defense_val = format!("*{}", defense_val);
        }
        if b.is_special_attack_boosted() {
            spa_val = format!("*{}", spa_val);
        }
        if b.is_special_defense_boosted() {
            if gen.get_generation() == 2 {
                let unboosted = pkmn
                    .base_stats
                    .calc_level_stats(pkmn.level, &pkmn.dvs, &pkmn.stat_xp, &gen.make_badge_list(), pkmn.nature, None)
                    .special_attack;
                if !xpr_data::stats::should_ignore_spd_badge_boost(unboosted) {
                    spd_val = format!("*{}", spd_val);
                }
            } else {
                spd_val = format!("*{}", spd_val);
            }
        }
        if b.is_speed_boosted() {
            speed_val = format!("*{}", speed_val);
        }
    }
    let stat_rows = vec![
        ("HP:".to_string(), pkmn.cur_stats.hp.to_string(), None),
        ("Attack:".to_string(), attack_val, None),
        ("Defense:".to_string(), defense_val, None),
        ("Spc Atk:".to_string(), spa_val, None),
        ("Spc Def:".to_string(), spd_val, None),
        ("Speed:".to_string(), speed_val, args.speed_style),
    ];
    let mut moves: Vec<String> = pkmn.move_list.iter().map(|m| m.clone().unwrap_or_default()).collect();
    while moves.len() < 4 {
        moves.push(String::new());
    }
    let move_rows = vec![
        ("Lv:".to_string(), pkmn.level.to_string(), None),
        ("Exp:".to_string(), pkmn.xp.to_string(), None),
        ("Move 1:".to_string(), moves[0].clone(), None),
        ("Move 2:".to_string(), moves[1].clone(), None),
        ("Move 3:".to_string(), moves[2].clone(), None),
        ("Move 4:".to_string(), moves[3].clone(), None),
    ];
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let total = ui.available_width();
        let half = if args.stats_only { total } else { (total - 2.0) / 2.0 };
        ui.allocate_ui_with_layout(Vec2::new(half, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(half);
            stat_column(
                ui,
                theme,
                &StatColumnSpec { header: "", rows: stat_rows, label_width: None, val_width: Some(4.0 * 8.0), style_prefix: "Secondary", font: font.clone() },
            );
        });
        if !args.stats_only {
            ui.allocate_ui_with_layout(Vec2::new(half, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_width(half);
                stat_column(
                    ui,
                    theme,
                    &StatColumnSpec { header: "", rows: move_rows, label_width: None, val_width: Some(11.0 * 8.0), style_prefix: "Primary", font: font.clone() },
                );
            });
        }
    });
}

fn vals_from_stat_block(gen: &GenData, sb: &StatBlock) -> Vec<i64> {
    if gen.get_generation() >= 2 {
        vec![sb.hp, sb.attack, sb.defense, sb.special_attack, sb.special_defense, sb.speed]
    } else {
        vec![sb.hp, sb.attack, sb.defense, sb.special_attack, sb.speed]
    }
}

/// `StatExpViewer`: three columns (net gain / realized / total).
pub fn stat_exp_viewer(ui: &mut Ui, theme: &Theme, gen: &GenData, state: Option<&RouteState>) {
    let cur_gen = gen.get_generation();
    let labels: Vec<&str> = if cur_gen >= 2 {
        vec!["HP:", "Attack:", "Defense:", "Spc Atk:", "Spc Def:", "Speed:"]
    } else {
        vec!["HP:", "Attack:", "Defense:", "Special:", "Speed:"]
    };
    let (gain_header, realized_header, total_header) = if cur_gen >= 3 {
        ("Net Stats\nFrom EVs", "Realized\nEVs", "Total\nEVs")
    } else {
        ("Net Stats\nFrom StatExp", "Realized\nStatExp", "Total\nStatExp")
    };
    let (net, realized, total): (Vec<i64>, Vec<i64>, Vec<i64>) = match state {
        Some(s) => (
            vals_from_stat_block(gen, &s.solo_pkmn.get_net_gain_from_stat_xp(&s.badges)),
            vals_from_stat_block(gen, &s.solo_pkmn.realized_stat_xp),
            vals_from_stat_block(gen, &s.solo_pkmn.unrealized_stat_xp),
        ),
        None => (vec![0; labels.len()], vec![0; labels.len()], vec![0; labels.len()]),
    };
    let rows = |vals: &Vec<i64>| -> Vec<(String, String, Option<&'static str>)> {
        labels.iter().zip(vals.iter()).map(|(l, v)| (l.to_string(), v.to_string(), None)).collect()
    };
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let third = (ui.available_width() - 8.0) / 3.0;
        let specs = [
            (gain_header, rows(&net), 3.0, "Header"),
            (realized_header, rows(&realized), 5.0, "Secondary"),
            (total_header, rows(&total), 5.0, "Primary"),
        ];
        for (header, rows, vw, prefix) in specs {
            ui.allocate_ui_with_layout(Vec2::new(third, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_width(third);
                stat_column(ui, theme, &StatColumnSpec { header, rows, label_width: None, val_width: Some(vw * 8.0), style_prefix: prefix, font: theme.body() });
            });
        }
    });
}

/// `InventoryViewer`: money line + 20 numbered item slots in two columns.
/// Slots are numbered from 1, matching the bag-reorder event text.
pub fn inventory_viewer(ui: &mut Ui, theme: &Theme, inventory: Option<&Inventory>) {
    let max_render = 20usize;
    let split = max_render / 2;
    let header_bg = theme.tinted_bg("Header", 0.25);
    let money = inventory.map(|i| i.cur_money.to_string()).unwrap_or_else(|| "0".to_string());
    let font = theme.body();
    ui.spacing_mut().item_spacing = Vec2::new(2.0, 2.0);
    {
        let text = format!("Current Money: {}", money);
        let galley = ui.fonts_mut(|f| f.layout_no_wrap(text, font.clone(), Color32::WHITE));
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().max(galley.size().x + 8.0), galley.size().y + 4.0), Sense::hover());
        ui.painter().rect_filled(rect, CornerRadius::same(2), header_bg);
        ui.painter().galley(Pos2::new(rect.min.x + 4.0, rect.center().y - galley.size().y / 2.0), galley, theme.header);
    }
    let items: Vec<String> = match inventory {
        Some(inv) => {
            let too_many = inv.cur_items.len() > max_render;
            let mut out = Vec::new();
            for idx in 0..max_render {
                if idx < inv.cur_items.len() {
                    if too_many && idx == max_render - 1 {
                        out.push(format!("# {:0>2}+: More items...", idx + 1));
                    } else {
                        let it = &inv.cur_items[idx];
                        out.push(format!("# {:0>2}: {}x {}", idx + 1, it.num, it.base_item.name));
                    }
                } else {
                    out.push(format!("# {:0>2}:", idx + 1));
                }
            }
            out
        }
        None => (0..max_render).map(|i| format!("# {:0>2}:", i + 1)).collect(),
    };
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        for col in 0..2 {
            ui.vertical(|ui| {
                ui.set_min_width(160.0);
                for row in 0..split {
                    let idx = col * split + row;
                    ui.label(egui::RichText::new(&items[idx]).font(font.clone()).color(theme.secondary));
                }
            });
        }
    });
}

/// `StateViewer.set_state`: the three rounded sections (Pokémon + stat EXP on
/// the left in a 3:2 split with the inventory on the right).
pub fn state_viewer(ui: &mut Ui, theme: &Theme, gen: Option<&GenData>, state: Option<&RouteState>, last_pkmn: &mut Option<(EnemyPkmn, BadgeList)>) {
    let section_bg = theme.section_bg();
    let margin = egui::Margin { left: 8, right: 8, top: 6, bottom: 6 };
    ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
    // Python keeps the last mon shown when the state is None
    if let Some(s) = state {
        *last_pkmn = Some((s.solo_pkmn.get_pkmn_obj(&s.badges, None), s.badges.clone()));
    }
    ui.horizontal_top(|ui| {
        let total = ui.available_width();
        let left_w = ((total - 6.0) * 3.0 / 5.0).max(200.0);
        let right_w = (total - 6.0 - left_w).max(250.0);
        ui.allocate_ui_with_layout(Vec2::new(left_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(left_w);
            widgets::rounded_section(ui, section_bg, 6, margin, |ui| {
                ui.set_min_height(150.0);
                match (gen, last_pkmn.as_ref()) {
                    (Some(g), Some((pkmn, badges))) => {
                        pkmn_viewer(ui, theme, g, &PkmnViewerArgs { pkmn, badges: Some(badges), speed_style: None, stats_only: false, font_size: Some(12.0) });
                    }
                    _ => {
                        ui.label(egui::RichText::new("").font(theme.body()));
                    }
                }
            });
            widgets::rounded_section(ui, section_bg, 6, margin, |ui| {
                ui.set_min_height(150.0);
                if let Some(g) = gen {
                    stat_exp_viewer(ui, theme, g, state);
                }
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Stats with * are calculated with a badge boost").font(theme.body()).italics().color(theme.contrast));
            });
        });
        ui.allocate_ui_with_layout(Vec2::new(right_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(right_w);
            widgets::rounded_section(ui, section_bg, 6, margin, |ui| {
                ui.set_min_height(150.0);
                inventory_viewer(ui, theme, state.map(|s| &s.inventory));
            });
        });
    });
}
