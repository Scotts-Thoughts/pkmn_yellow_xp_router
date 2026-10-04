//! Stats tab: speed / stat planning of a chosen species at a level with
//! DVs + Stat Exp (gens 1-2) or IVs + EVs + nature (gen 3+), against the major
//! battles of the game. Port of Solodex `StatsView.tsx`.
//!
//! Solodex took the major battles from its own trainer files; here they come
//! from the router's trainer tables (see [`battles`]). Games without router
//! trainer data (gens 6-9) show "No trainer data available".

use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{Align, Align2, Color32, CornerRadius, Id, Margin, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use xpr_dex::model::{BaseStats, PokemonData};
use xpr_dex::stats::{calc_gen12_stats, calc_gen3_stats, derive_hp_dv, nature_mods_by_name, CalcStats, Gen12Dvs, Gen12StatExps, Gen3Spread, NatureMods, NEUTRAL_NATURE};
use xpr_ui_kit::theme::Theme;

use crate::images::SpriteSize;
use crate::palette::{self, px, px_bold};
use crate::widgets;
use crate::DexCx;

pub mod battles;
mod combobox;
mod inputs;
pub mod nature_selector;
pub mod scout;

pub use battles::{major_battles, MajorBattle};
pub use nature_selector::nature_selector;

use combobox::{combobox, ComboOption, ComboState};
use inputs::{int_field, Range};
use scout::{max_invest, min_level_to_outspeed, user_invest, Spread};

/// Width of the calculator column (`style={{ width: 420 }}`).
const LEFT_W: f32 = 420.0;

pub struct StatsView {
    /// the picked species ("" = none); follows the Pokedex selection
    species: String,
    last_selected: Option<String>,
    last_game: String,
    level: i32,
    dvs: Gen12Dvs,
    stat_exps: Gen12StatExps,
    ivs: Gen3Spread,
    evs: Gen3Spread,
    nature: String,
    combo: ComboState,
    options: Option<(String, Vec<ComboOption>)>,
    battles: Option<(String, Vec<MajorBattle>)>,
    scouting: Option<(ScoutKey, Vec<ScoutRow>)>,
}

#[derive(Clone, PartialEq)]
struct ScoutKey {
    game: String,
    species: String,
    spread: Spread,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScoutRow {
    /// min level to outspeed with max speed investment
    pub level_max: Option<i32>,
    /// min level with the current speed investment
    pub level_user: Option<i32>,
}

impl Default for StatsView {
    fn default() -> Self {
        StatsView {
            species: String::new(),
            last_selected: None,
            last_game: String::new(),
            level: 50,
            dvs: Gen12Dvs::default(),
            stat_exps: Gen12StatExps::default(),
            ivs: Gen3Spread::MAX_IVS,
            evs: Gen3Spread::ZERO,
            nature: "Hardy".to_string(),
            combo: ComboState::default(),
            options: None,
            battles: None,
            scouting: None,
        }
    }
}

impl StatsView {
    /// The species being planned ("" when none).
    pub fn species(&self) -> &str {
        &self.species
    }

    pub fn level(&self) -> i32 {
        self.level
    }

    pub fn nature(&self) -> &str {
        &self.nature
    }

    fn spread(&self, gen: u8) -> Spread {
        let nature = if gen < 3 { NEUTRAL_NATURE } else { nature_mods_by_name(&self.nature) };
        Spread { dvs: self.dvs, stat_exps: self.stat_exps, ivs: self.ivs, evs: self.evs, nature }
    }

    /// Follow the Pokedex selection and drop a species the game lacks.
    fn sync(&mut self, cx: &DexCx, game: &str) {
        let selected = cx.state.selected().map(str::to_string);
        if selected != self.last_selected {
            self.last_selected = selected.clone();
            if let Some(s) = selected {
                self.species = s;
            }
        }
        // inputs are sticky across games, but a species missing from the new game resets the picker
        if !self.species.is_empty() && xpr_dex::get_pokemon_data(&self.species, game).is_none() {
            self.species.clear();
        }
        // back in a game that has the Pokedex's species: pick it up again
        if self.species.is_empty() && self.last_game != game {
            if let Some(s) = self.last_selected.as_deref() {
                if xpr_dex::get_pokemon_data(s, game).is_some() {
                    self.species = s.to_string();
                }
            }
        }
        self.last_game = game.to_string();
    }

    fn options_for(&mut self, game: &str) -> &[ComboOption] {
        if self.options.as_ref().map(|(g, _)| g.as_str()) != Some(game) {
            let list = xpr_dex::get_all_pokemon_for_game(game)
                .iter()
                .map(|p| {
                    let types = if p.type_1 != p.type_2 { format!("{}/{}", p.type_1, p.type_2) } else { p.type_1.clone() };
                    ComboOption::new(p.species.clone(), xpr_dex::display_name(&p.species), format!("#{:04} \u{00B7} {}", p.national_dex_number, types), Some(palette::type_color(&p.type_1)))
                })
                .collect();
            self.options = Some((game.to_string(), list));
        }
        &self.options.as_ref().unwrap().1
    }

    /// The major battles of `game` from the router's trainer data (cached per game).
    fn battles_for(&mut self, cx: &DexCx, game: &str) -> &[MajorBattle] {
        if self.battles.as_ref().map(|(g, _)| g.as_str()) != Some(game) {
            let list = xpr_dex::games::router_versions(game)
                .first()
                .and_then(|v| cx.registry.get_version(v).ok())
                .map(|gen| major_battles(&gen, game))
                .unwrap_or_default();
            self.battles = Some((game.to_string(), list));
        }
        &self.battles.as_ref().unwrap().1
    }

    /// The outspeed levels of every battle for the current inputs (cached).
    fn scouting_for(&mut self, game: &str, gen: u8, data: &PokemonData) -> Vec<ScoutRow> {
        let spread = self.spread(gen);
        let key = ScoutKey { game: game.to_string(), species: self.species.clone(), spread };
        if let Some((k, rows)) = &self.scouting {
            if *k == key {
                return rows.clone();
            }
        }
        let battles: &[MajorBattle] = self.battles.as_ref().map(|(_, b)| b.as_slice()).unwrap_or(&[]);
        let rows: Vec<ScoutRow> = battles
            .iter()
            .map(|b| {
                let target = b.fastest.as_ref().map(|f| f.speed).unwrap_or(0);
                ScoutRow { level_max: min_level_to_outspeed(&data.base_stats, gen, target, max_invest(gen), &spread), level_user: min_level_to_outspeed(&data.base_stats, gen, target, user_invest(gen, &spread), &spread) }
            })
            .collect();
        self.scouting = Some((key, rows.clone()));
        rows
    }

    /// The calculated stats of the picked species at the current inputs.
    pub fn calc(&self, gen: u8, data: &PokemonData) -> CalcStats {
        calc_stats(&data.base_stats, gen, self.level, &self.spread(gen), &self.species)
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut DexCx) {
        let theme = cx.theme;
        let game = cx.state.game().to_string();
        let gen = xpr_dex::game_gen(&game).max(1);
        self.sync(cx, &game);
        let data: Option<Arc<PokemonData>> = if self.species.is_empty() { None } else { xpr_dex::get_pokemon_data(&self.species, &game) };
        self.battles_for(cx, &game);

        let full = ui.available_rect_before_wrap();
        let left_w = LEFT_W.min(full.width() * 0.55);
        let left = Rect::from_min_size(full.min, Vec2::new(left_w, full.height()));
        let right = Rect::from_min_max(Pos2::new(left.max.x, full.min.y), full.max);

        // ---- left: calculator ----
        ui.painter().rect_filled(left, CornerRadius::ZERO, palette::panel_bg(theme));
        ui.painter().rect_filled(Rect::from_min_max(Pos2::new(left.max.x - 1.0, left.min.y), left.max), CornerRadius::ZERO, palette::GRAY_700);
        let inner = Rect::from_min_max(left.min, Pos2::new(left.max.x - 1.0, left.max.y));
        let mut lui = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(Align::Min)));
        lui.set_clip_rect(inner.intersect(ui.clip_rect()));
        let area = egui::ScrollArea::vertical().id_salt("stats_left").auto_shrink([false, false]);
        xpr_ui_kit::widgets::show_scroll(&mut lui, area, |ui| {
            egui::Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                self.calculator(ui, cx, &game, gen, data.as_deref());
            });
        });

        // ---- right: scouting ----
        let mut rui = ui.new_child(egui::UiBuilder::new().max_rect(right).layout(egui::Layout::top_down(Align::Min)));
        rui.set_clip_rect(right.intersect(ui.clip_rect()));
        self.scouting_panel(&mut rui, cx, &game, gen, data.as_deref());
        ui.allocate_rect(full, Sense::hover());
    }

    fn calculator(&mut self, ui: &mut Ui, cx: &mut DexCx, game: &str, gen: u8, data: Option<&PokemonData>) {
        let theme = cx.theme;
        ui.spacing_mut().item_spacing = Vec2::ZERO;

        // Pokemon + Level
        section_heading(ui, theme, "Pokemon", None);
        ui.add_space(8.0);
        let value = if self.species.is_empty() { String::new() } else { xpr_dex::display_name(&self.species).to_string() };
        let width = ui.available_width();
        let combo_id = Id::new("stats_species");
        self.options_for(game);
        let options = &self.options.as_ref().unwrap().1;
        if let Some(id) = combobox(ui, theme, combo_id, &mut self.combo, &value, options, "Select a Pokemon\u{2026}", width) {
            self.species = id;
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.label(egui::RichText::new("Level").font(px(theme, 12.0)).color(palette::GRAY_500));
            let (rect, _) = ui.allocate_exact_size(Vec2::new(64.0, 28.0), Sense::hover());
            int_field(ui, Id::new("stats_level"), rect, &mut self.level, 1, 100, px(theme, 14.0), Range::Reject, true);
        });
        ui.add_space(20.0);

        // Nature (gen 3+)
        if gen >= 3 {
            section_heading(ui, theme, "Nature", Some(" \u{2014} \u{00B1}10% to two stats"));
            ui.add_space(8.0);
            if let Some(n) = nature_selector(ui, theme, "stats", &self.nature) {
                self.nature = n;
            }
            ui.add_space(20.0);
        }

        // IV / EV (or DV / StatExp) grid
        section_heading(ui, theme, if gen <= 2 { "DVs & Stat Exp" } else { "IVs & EVs" }, None);
        ui.add_space(8.0);
        self.spread_grid(ui, theme, gen);
        ui.add_space(20.0);

        // result card
        section_heading(ui, theme, "Calculated Stats", None);
        ui.add_space(8.0);
        match data {
            None => {
                ui.label(egui::RichText::new("Select a Pokemon above").font(px(theme, 12.0)).italics().color(palette::GRAY_600));
            }
            Some(d) => self.result_card(ui, cx, game, gen, d),
        }
    }

    /// The DV / Stat Exp or IV / EV input grid.
    fn spread_grid(&mut self, ui: &mut Ui, theme: &Theme, gen: u8) {
        let small = px(theme, 10.0);
        let head = px(theme, 9.0);
        let (heads, n): (&[&str], usize) = if gen <= 2 { (&["HP", "Atk", "Def", "Spe", "Spc"], 5) } else { (&["HP", "Atk", "Def", "SpA", "SpD", "Spe"], 6) };
        let w = ui.available_width();
        let (col_gap, row_gap, label_w) = (4.0, 3.0, 28.0);
        let col_w = (w - label_w - col_gap * n as f32) / n as f32;
        let (top, _) = ui.allocate_exact_size(Vec2::new(w, 14.0), Sense::hover());
        let col_x = |i: usize| top.min.x + label_w + col_gap + i as f32 * (col_w + col_gap);
        for (i, h) in heads.iter().enumerate() {
            ui.painter().text(Pos2::new(col_x(i) + col_w / 2.0, top.center().y), Align2::CENTER_CENTER, h, head.clone(), palette::GRAY_500);
        }
        let row = |ui: &mut Ui, label: &str| -> Rect {
            ui.add_space(row_gap);
            let (r, _) = ui.allocate_exact_size(Vec2::new(w, 23.0), Sense::hover());
            ui.painter().text(Pos2::new(r.min.x, r.center().y), Align2::LEFT_CENTER, label, head.clone(), palette::GRAY_500);
            r
        };
        let cell = |r: Rect, i: usize| Rect::from_min_size(Pos2::new(col_x(i), r.min.y), Vec2::new(col_w, 23.0));
        if gen <= 2 {
            let r = row(ui, "DV");
            // HP's DV is derived from the others' parity bits
            let mut hp = derive_hp_dv(&self.dvs);
            let hp_cell = cell(r, 0);
            int_field(ui, Id::new("stats_dv_hp"), hp_cell, &mut hp, 0, 15, small.clone(), Range::Clamp, false);
            ui.interact(hp_cell, Id::new("stats_dv_hp_tip"), Sense::hover()).on_hover_text("Derived from ATK/DEF/SPE/SPC parity bits");
            let d = &mut self.dvs;
            for (i, (key, v)) in [("attack", &mut d.attack), ("defense", &mut d.defense), ("speed", &mut d.speed), ("special", &mut d.special)].into_iter().enumerate() {
                int_field(ui, Id::new(("stats_dv", key)), cell(r, i + 1), v, 0, 15, small.clone(), Range::Clamp, true);
            }
            let r = row(ui, "Exp");
            let e = &mut self.stat_exps;
            for (i, (key, v)) in [("hp", &mut e.hp), ("attack", &mut e.attack), ("defense", &mut e.defense), ("speed", &mut e.speed), ("special", &mut e.special)].into_iter().enumerate() {
                int_field(ui, Id::new(("stats_exp", key)), cell(r, i), v, 0, 65535, small.clone(), Range::Clamp, true);
            }
        } else {
            let r = row(ui, "IV");
            let v = &mut self.ivs;
            for (i, (key, x)) in [("hp", &mut v.hp), ("attack", &mut v.attack), ("defense", &mut v.defense), ("spattack", &mut v.spattack), ("spdefense", &mut v.spdefense), ("speed", &mut v.speed)].into_iter().enumerate() {
                int_field(ui, Id::new(("stats_iv", key)), cell(r, i), x, 0, 31, small.clone(), Range::Clamp, true);
            }
            let r = row(ui, "EV");
            let v = &mut self.evs;
            for (i, (key, x)) in [("hp", &mut v.hp), ("attack", &mut v.attack), ("defense", &mut v.defense), ("spattack", &mut v.spattack), ("spdefense", &mut v.spdefense), ("speed", &mut v.speed)].into_iter().enumerate() {
                int_field(ui, Id::new(("stats_ev", key)), cell(r, i), x, 0, 252, small.clone(), Range::Clamp, true);
            }
        }
    }

    /// The species header (sprite, name, types) and its calculated stats.
    fn result_card(&mut self, ui: &mut Ui, cx: &mut DexCx, game: &str, gen: u8, data: &PokemonData) {
        let theme = cx.theme;
        let stats = self.calc(gen, data);
        let mods: NatureMods = if gen >= 3 { nature_mods_by_name(&self.nature) } else { NEUTRAL_NATURE };
        let w = ui.available_width();
        let head_h = 56.0;
        let cell_h = 55.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, head_h + 1.0 + cell_h), Sense::hover());
        let p = ui.painter();
        p.rect(rect, CornerRadius::same(8), palette::GRAY_800, Stroke::new(1.0_f32, palette::GRAY_700), StrokeKind::Inside);
        let sep_y = rect.min.y + head_h;
        p.hline(rect.x_range(), sep_y + 0.5, Stroke::new(1.0_f32, palette::GRAY_700));
        // header: sprite, NAME, types at the right
        let sprite_rect = Rect::from_min_size(Pos2::new(rect.min.x + 12.0, rect.min.y + 8.0), Vec2::splat(40.0));
        if let Some(tex) = cx.images.sprite(ui.ctx(), &self.species, data.national_dex_number, SpriteSize::Small) {
            crate::images::paint_fit(ui, &tex, sprite_rect, Color32::WHITE);
        }
        let types = data.types();
        let badges_w = types.len() as f32 * 68.0 + (types.len().saturating_sub(1)) as f32 * 4.0;
        let name_x = rect.min.x + 12.0 + 40.0 + 8.0;
        let name = xpr_ui_kit::widgets::elide(ui, &xpr_dex::display_name(&self.species).to_uppercase(), &px_bold(theme, 14.0), (rect.max.x - 12.0 - badges_w - 8.0 - name_x).max(20.0));
        ui.painter().text(Pos2::new(name_x, rect.min.y + head_h / 2.0), Align2::LEFT_CENTER, name, px_bold(theme, 14.0), Color32::WHITE);
        let brect = Rect::from_min_size(Pos2::new(rect.max.x - 12.0 - badges_w, rect.min.y + head_h / 2.0 - 9.0), Vec2::new(badges_w, 18.0));
        let mut bui = ui.new_child(egui::UiBuilder::new().max_rect(brect).layout(egui::Layout::left_to_right(Align::Center)));
        bui.spacing_mut().item_spacing.x = 4.0;
        for t in &types {
            widgets::type_badge(&mut bui, theme, t, true, Some(game));
        }
        // stat cells
        let cells: Vec<(&str, i32, f64)> = if gen <= 1 {
            vec![("HP", stats.hp, 1.0), ("Atk", stats.attack, mods.attack), ("Def", stats.defense, mods.defense), ("Spc", stats.spattack, mods.spattack), ("Spe", stats.speed, mods.speed)]
        } else {
            vec![("HP", stats.hp, 1.0), ("Atk", stats.attack, mods.attack), ("Def", stats.defense, mods.defense), ("SpA", stats.spattack, mods.spattack), ("SpD", stats.spdefense, mods.spdefense), ("Spe", stats.speed, mods.speed)]
        };
        let cw = (w - 2.0) / cells.len() as f32;
        for (i, (label, value, m)) in cells.iter().enumerate() {
            let x0 = rect.min.x + 1.0 + i as f32 * cw;
            let cx_mid = x0 + cw / 2.0;
            if i > 0 {
                ui.painter().vline(x0, egui::Rangef::new(sep_y + 1.0, rect.max.y - 1.0), Stroke::new(1.0_f32, palette::GRAY_700));
            }
            let tint = if gen >= 3 && *label != "HP" {
                if *m > 1.0 {
                    palette::GREEN_400
                } else if *m < 1.0 {
                    palette::RED_400
                } else {
                    Color32::WHITE
                }
            } else {
                Color32::WHITE
            };
            ui.painter().text(Pos2::new(cx_mid, sep_y + 1.0 + 8.0 + 7.5), Align2::CENTER_CENTER, label, px_bold(theme, 10.0), palette::GRAY_400);
            ui.painter().text(Pos2::new(cx_mid, sep_y + 1.0 + 8.0 + 15.0 + 12.0), Align2::CENTER_CENTER, value.to_string(), px_bold(theme, 16.0), tint);
        }
    }

    fn scouting_panel(&mut self, ui: &mut Ui, cx: &mut DexCx, game: &str, gen: u8, data: Option<&PokemonData>) {
        let theme = cx.theme;
        let full = ui.available_rect_before_wrap();
        let user_invest_value = user_invest(gen, &self.spread(gen));
        let (invest_max, invest_user) = if gen <= 2 { ("Stat Exp", "Stat Exp") } else { ("EVs", "EV") };

        // header: title and description on a panel with a bottom border
        let reg = px(theme, 11.0);
        let who = if self.species.is_empty() { "your Pokemon".to_string() } else { xpr_dex::display_name(&self.species).to_string() };
        let mut job = LayoutJob::default();
        job.wrap.max_width = (full.width() - 32.0).max(100.0);
        let mut add = |text: &str, color: Color32| job.append(text, 0.0, TextFormat { font_id: reg.clone(), color, ..Default::default() });
        add(&format!("Minimum level for {} to outspeed each major battle's fastest Pokemon, with ", who), palette::GRAY_500);
        add(&format!("max Speed {}", invest_max), palette::GRAY_300);
        add(&format!(" vs your current Speed {} ({})", invest_user, user_invest_value), palette::GRAY_500);
        if gen >= 3 {
            add(&format!(", {} nature", self.nature), palette::GRAY_500);
        }
        add(".", palette::GRAY_500);
        let desc = ui.fonts_mut(|f| f.layout_job(job));
        let head_h = 12.0 + 20.0 + 2.0 + desc.size().y + 12.0 + 1.0;
        let head = Rect::from_min_size(full.min, Vec2::new(full.width(), head_h));
        ui.painter().rect_filled(head, CornerRadius::ZERO, palette::panel_bg(theme));
        ui.painter().rect_filled(Rect::from_min_max(Pos2::new(head.min.x, head.max.y - 1.0), head.max), CornerRadius::ZERO, palette::GRAY_700);
        ui.painter().text(Pos2::new(head.min.x + 16.0, head.min.y + 12.0 + 10.0), Align2::LEFT_CENTER, "Scouting \u{2014} outspeed levels", px_bold(theme, 14.0), Color32::WHITE);
        ui.painter().galley(Pos2::new(head.min.x + 16.0, head.min.y + 12.0 + 20.0 + 2.0), desc, palette::GRAY_500);

        let body = Rect::from_min_max(Pos2::new(full.min.x, head.max.y), full.max);
        let empty = |ui: &Ui, text: &str| {
            ui.painter().text(body.center(), Align2::CENTER_CENTER, text, px(theme, 14.0), palette::GRAY_600);
        };
        let Some(data) = data else {
            empty(ui, "Select a Pokemon to scout");
            return;
        };
        let battle_count = self.battles.as_ref().map(|(_, b)| b.len()).unwrap_or(0);
        if battle_count == 0 {
            empty(ui, &format!("No trainer data available for {}", game));
            return;
        }
        let rows = self.scouting_for(game, gen, data);
        let battles: Vec<MajorBattle> = self.battles.as_ref().map(|(_, b)| b.clone()).unwrap_or_default();
        let mut bui = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(Align::Min)));
        bui.set_clip_rect(body.intersect(ui.clip_rect()));
        let area = egui::ScrollArea::vertical().id_salt("stats_right").auto_shrink([false, false]);
        xpr_ui_kit::widgets::show_scroll(&mut bui, area, |ui| {
            egui::Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                scouting_header(ui, theme, if gen <= 2 { "Exp" } else { "EVs" }, if gen <= 2 { "Exp" } else { "EV" });
                for (b, r) in battles.iter().zip(rows.iter()) {
                    ui.add_space(6.0);
                    battle_row(ui, cx, game, b, r);
                }
            });
        });
    }
}

/// The calculated stats (`calcGen12Stats` / `calcGen3PlusStats`).
pub fn calc_stats(base: &BaseStats, gen: u8, level: i32, s: &Spread, species: &str) -> CalcStats {
    if gen <= 2 {
        calc_gen12_stats(base, level, &s.dvs, &s.stat_exps)
    } else {
        calc_gen3_stats(base, level, &s.ivs, &s.evs, &s.nature, Some(species))
    }
}

/// `text-[10px] font-semibold text-gray-500 uppercase tracking-wider`, with an
/// optional lowercase gray-600 aside.
fn section_heading(ui: &mut Ui, theme: &Theme, text: &str, aside: Option<&str>) {
    let mut job = LayoutJob::default();
    job.append(&text.to_uppercase(), 0.0, TextFormat { font_id: px_bold(theme, 10.0), color: palette::GRAY_500, ..Default::default() });
    if let Some(a) = aside {
        job.append(a, 0.0, TextFormat { font_id: px(theme, 10.0), color: palette::GRAY_600, ..Default::default() });
    }
    ui.label(job);
}

// ---- scouting rows --------------------------------------------------------------------------------

const COL_FASTEST: f32 = 160.0;
const COL_LEVEL: f32 = 80.0;
const COL_GAP: f32 = 8.0;
const ROW_PAD: f32 = 8.0;
const ROW_H: f32 = 46.0;

/// The column positions of a row `rect` wide: (fastest x, max x, your x), as
/// offsets from the row's left edge.
fn columns(width: f32) -> (f32, f32, f32) {
    let your = width - ROW_PAD - COL_LEVEL;
    let max = your - COL_GAP - COL_LEVEL;
    let fastest = max - COL_GAP - COL_FASTEST;
    (fastest, max, your)
}

fn scouting_header(ui: &mut Ui, theme: &Theme, max_label: &str, your_label: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 14.0), Sense::hover());
    let (fx, mx, yx) = columns(rect.width());
    let font = px_bold(theme, 9.0);
    let c = palette::GRAY_500;
    let y = rect.center().y;
    ui.painter().text(Pos2::new(rect.min.x + ROW_PAD, y), Align2::LEFT_CENTER, "BATTLE", font.clone(), c);
    ui.painter().text(Pos2::new(rect.min.x + fx, y), Align2::LEFT_CENTER, "FASTEST POKEMON", font.clone(), c);
    ui.painter().text(Pos2::new(rect.min.x + mx + COL_LEVEL / 2.0, y), Align2::CENTER_CENTER, format!("MAX {}", max_label.to_uppercase()), font.clone(), c);
    ui.painter().text(Pos2::new(rect.min.x + yx + COL_LEVEL / 2.0, y), Align2::CENTER_CENTER, format!("YOUR {}", your_label.to_uppercase()), font, c);
}

fn battle_row(ui: &mut Ui, cx: &mut DexCx, game: &str, b: &MajorBattle, r: &ScoutRow) {
    let theme = cx.theme;
    let _ = game;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_H), Sense::hover());
    ui.painter().rect(rect, CornerRadius::same(4), palette::GRAY_800, Stroke::new(1.0_f32, palette::GRAY_700), StrokeKind::Inside);
    let (fx, mx, yx) = columns(rect.width());
    // name and class
    let name_w = (fx - COL_GAP - ROW_PAD).max(20.0);
    let nx = rect.min.x + ROW_PAD;
    let name_font = px_bold(theme, 12.0);
    let name = xpr_ui_kit::widgets::elide(ui, &b.name, &name_font, name_w);
    ui.painter().text(Pos2::new(nx, rect.min.y + 15.0), Align2::LEFT_CENTER, name, name_font, palette::trainer_category_color(Some(&b.category)));
    let sub_font = px(theme, 10.0);
    let sub = xpr_ui_kit::widgets::elide(ui, &format!("{} \u{00B7} max Lv{}", b.trainer_class, b.max_level), &sub_font, name_w);
    ui.painter().text(Pos2::new(nx, rect.min.y + 31.0), Align2::LEFT_CENTER, sub, sub_font, palette::GRAY_500);
    // the fastest enemy Pokemon
    if let Some(f) = &b.fastest {
        let sx = rect.min.x + fx;
        if let Some(key) = xpr_dex::resolve_species(&f.species) {
            if let Some(tex) = cx.images.sprite_by_name(ui.ctx(), key, SpriteSize::Small) {
                crate::images::paint_fit(ui, &tex, Rect::from_min_size(Pos2::new(sx, rect.center().y - 12.0), Vec2::splat(24.0)), Color32::WHITE);
            }
        }
        let tx = sx + 24.0 + 6.0;
        let f1 = px(theme, 11.0);
        let shown = xpr_dex::resolve_species(&f.species).map(xpr_dex::display_name).unwrap_or(&f.species);
        let shown = xpr_ui_kit::widgets::elide(ui, shown, &f1, (COL_FASTEST - 30.0).max(20.0));
        ui.painter().text(Pos2::new(tx, rect.min.y + 15.0), Align2::LEFT_CENTER, shown, f1, palette::GRAY_200);
        ui.painter().text(Pos2::new(tx, rect.min.y + 31.0), Align2::LEFT_CENTER, format!("Lv{} \u{00B7} {} Spe", f.level, f.speed), px(theme, 10.0), palette::GRAY_500);
    }
    level_pill(ui, theme, Rect::from_min_size(Pos2::new(rect.min.x + mx, rect.min.y), Vec2::new(COL_LEVEL, ROW_H)), r.level_max);
    level_pill(ui, theme, Rect::from_min_size(Pos2::new(rect.min.x + yx, rect.min.y), Vec2::new(COL_LEVEL, ROW_H)), r.level_user);
}

/// "Lv45" in emerald, or a red dash when even Lv100 is too slow.
fn level_pill(ui: &mut Ui, theme: &Theme, rect: Rect, level: Option<i32>) {
    match level {
        None => {
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, "\u{2014}", px_bold(theme, 11.0), palette::RED_400);
            let tip = Rect::from_center_size(rect.center(), Vec2::new(24.0, 20.0));
            ui.interact(tip, ui.id().with(("stats_pill", rect.min.x as i32, rect.min.y as i32)), Sense::hover()).on_hover_text("Can't outspeed even at Lv100");
        }
        Some(l) => {
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, format!("Lv{}", l), px_bold(theme, 14.0), palette::hex("#34d399"));
        }
    }
}
