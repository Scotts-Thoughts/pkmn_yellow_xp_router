//! Port of `gui_qt/pkmn_components/custom_dvs.py`: the DV/IV grid, the
//! nature selector (dropdown + increase/decrease stat buttons) and the
//! ability dropdown, with the hidden-power readout.

use std::sync::Arc;

use egui::{Color32, CornerRadius, Stroke, Ui, Vec2};

use xpr_core::consts;
use xpr_data::model::{Nature, PokemonSpecies, StatBlock};
use xpr_data::GenData;
use xpr_ui_kit::theme::Theme;
use xpr_ui_kit::widgets::{self, AmountEntry, StyledButton};

use crate::editors::OptionMenu;

const STAT_ORDER: [(&str, &str); 5] = [
    (consts::ATTACK, "Attack"),
    (consts::DEFENSE, "Defense"),
    (consts::SPEED, "Speed"),
    (consts::SPECIAL_ATTACK, "Sp. Atk"),
    (consts::SPECIAL_DEFENSE, "Sp. Def"),
];

const INCREASE_ACCENT: &str = "#ef4444";
const DECREASE_ACCENT: &str = "#3b82f6";

fn neutral_nature_for_stat(stat: &str) -> Option<Nature> {
    let name = match stat {
        consts::ATTACK => "HARDY",
        consts::DEFENSE => "DOCILE",
        consts::SPEED => "SERIOUS",
        consts::SPECIAL_ATTACK => "BASHFUL",
        consts::SPECIAL_DEFENSE => "QUIRKY",
        _ => return None,
    };
    Nature::from_name(name)
}

fn stat_for_neutral_nature(nat: Nature) -> Option<&'static str> {
    match nat.enum_name() {
        "HARDY" => Some(consts::ATTACK),
        "DOCILE" => Some(consts::DEFENSE),
        "SERIOUS" => Some(consts::SPEED),
        "BASHFUL" => Some(consts::SPECIAL_ATTACK),
        "QUIRKY" => Some(consts::SPECIAL_DEFENSE),
        _ => None,
    }
}

fn all_natures() -> Vec<Nature> {
    (0..25).filter_map(Nature::from_index).collect()
}

fn nature_for_stats(inc: Option<&str>, dec: Option<&str>) -> Option<Nature> {
    let (inc, dec) = (inc?, dec?);
    if inc == dec {
        return neutral_nature_for_stat(inc);
    }
    all_natures().into_iter().find(|n| n.is_stat_raised(inc) && n.is_stat_lowered(dec))
}

fn stats_for_nature(nat: Nature) -> (Option<&'static str>, Option<&'static str>) {
    if let Some(s) = stat_for_neutral_nature(nat) {
        return (Some(s), Some(s));
    }
    let mut inc = None;
    let mut dec = None;
    for (stat, _) in STAT_ORDER {
        if nat.is_stat_raised(stat) {
            inc = Some(stat);
        }
        if nat.is_stat_lowered(stat) {
            dec = Some(stat);
        }
    }
    (inc, dec)
}

/// `NatureSelector`
pub struct NatureSelector {
    pub dropdown: OptionMenu,
    increase: Option<&'static str>,
    decrease: Option<&'static str>,
}

impl NatureSelector {
    pub fn new(init: Nature) -> NatureSelector {
        let names: Vec<String> = all_natures().into_iter().map(|n| n.display_name()).collect();
        let mut s = NatureSelector { dropdown: OptionMenu::new(names, Some(&init.display_name())), increase: None, decrease: None };
        s.sync_buttons_from_dropdown();
        s
    }

    pub fn get_nature(&self) -> Nature {
        Nature::from_name(&self.dropdown.get().to_uppercase()).unwrap_or(Nature::HARDY)
    }

    fn sync_buttons_from_dropdown(&mut self) {
        let (i, d) = stats_for_nature(self.get_nature());
        self.increase = i;
        self.decrease = d;
    }

    fn sync_dropdown_from_buttons(&mut self) {
        if let Some(n) = nature_for_stats(self.increase, self.decrease) {
            self.dropdown.set(&n.display_name());
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, theme: &Theme) {
        egui::Grid::new(ui.id().with("nature_grid")).spacing(Vec2::new(5.0, 5.0)).show(ui, |ui| {
            widgets::label(ui, theme, "Nature:");
            if self.dropdown.ui(ui, theme, ui.id().with("nature_dd"), Some(120.0), true) {
                self.sync_buttons_from_dropdown();
            }
            ui.end_row();
            widgets::label(ui, theme, "Increase:");
            let mut inc_click: Option<&'static str> = None;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                for (stat, short) in STAT_ORDER {
                    if stat_button(ui, theme, short, self.increase == Some(stat), INCREASE_ACCENT).clicked() {
                        inc_click = Some(stat);
                    }
                }
            });
            ui.end_row();
            widgets::label(ui, theme, "Decrease:");
            let mut dec_click: Option<&'static str> = None;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                for (stat, short) in STAT_ORDER {
                    if stat_button(ui, theme, short, self.decrease == Some(stat), DECREASE_ACCENT).clicked() {
                        dec_click = Some(stat);
                    }
                }
            });
            ui.end_row();
            if let Some(s) = inc_click {
                self.increase = if self.increase == Some(s) { None } else { Some(s) };
                self.sync_dropdown_from_buttons();
            }
            if let Some(s) = dec_click {
                self.decrease = if self.decrease == Some(s) { None } else { Some(s) };
                self.sync_dropdown_from_buttons();
            }
        });
    }
}

fn stat_button(ui: &mut Ui, theme: &Theme, text: &str, checked: bool, accent: &str) -> egui::Response {
    let accent_c = xpr_ui_kit::theme::parse_hex(accent);
    let bold_w = widgets::text_width(ui, text, &theme.body_bold()) + 24.0;
    let mut b = StyledButton::new(theme, if checked { egui::RichText::new(text).font(theme.body_bold()) } else { egui::RichText::new(text).font(theme.body()) })
        .padding(Vec2::new(10.0, 2.0))
        .min_size(Vec2::new(bold_w, 22.0));
    if checked {
        b = b.fill(accent_c).hover_fill(accent_c).text_color(Color32::WHITE).stroke(Stroke::new(1.0_f32, accent_c));
    } else {
        b = b.stroke(Stroke::new(1.0_f32, Color32::TRANSPARENT));
    }
    let _ = CornerRadius::same(3);
    b.show(ui)
}

/// `CustomDVsFrame`
pub struct CustomDvsFrame {
    gen: Option<Arc<GenData>>,
    hp: String,
    atk: String,
    def: String,
    spd: String,
    spa: String,
    spc_def: Option<String>,
    nature: Option<NatureSelector>,
    ability: Option<OptionMenu>,
    hidden_power: Option<String>,
    dv_max: i64,
    dv_text: &'static str,
}

impl Default for CustomDvsFrame {
    fn default() -> Self {
        CustomDvsFrame::new()
    }
}

impl CustomDvsFrame {
    pub fn new() -> CustomDvsFrame {
        CustomDvsFrame {
            gen: None,
            hp: "15".into(),
            atk: "15".into(),
            def: "15".into(),
            spd: "15".into(),
            spa: "15".into(),
            spc_def: None,
            nature: None,
            ability: None,
            hidden_power: None,
            dv_max: 15,
            dv_text: "DV",
        }
    }

    /// `config_for_target_game_and_mon`
    pub fn config_for_target_game_and_mon(&mut self, gen: &Arc<GenData>, mon: Option<&PokemonSpecies>, init_dvs: Option<StatBlock>, init_ability_idx: Option<i64>, init_nature: Option<Nature>) {
        self.gen = Some(gen.clone());
        let cur_gen = gen.get_generation();
        let init_dvs = init_dvs.unwrap_or_else(|| {
            let max_dv = if cur_gen <= 2 { 15 } else { 31 };
            gen.make_stat_block(max_dv, max_dv, max_dv, max_dv, max_dv, max_dv, false)
        });
        let ability_idx = init_ability_idx.unwrap_or(0);
        let nature = init_nature.unwrap_or(Nature::HARDY);
        if cur_gen > 2 {
            self.dv_max = 31;
            self.dv_text = "IV";
            let ability_list: Vec<String> = mon.map(|m| m.abilities.clone()).unwrap_or_else(|| vec![String::new()]);
            let default_ability = ability_list.get(ability_idx as usize).cloned().unwrap_or_default();
            match self.ability.as_mut() {
                Some(a) => {
                    a.new_values(ability_list, None);
                    a.set(&default_ability);
                }
                None => self.ability = Some(OptionMenu::new(ability_list, Some(&default_ability))),
            }
            if self.nature.is_none() {
                self.nature = Some(NatureSelector::new(nature));
            } else if init_nature.is_some() {
                self.nature = Some(NatureSelector::new(nature));
            }
            self.hp = init_dvs.hp.to_string();
            self.spc_def = Some(init_dvs.special_defense.to_string());
            if self.hidden_power.is_none() {
                self.hidden_power = Some(String::new());
            }
        } else {
            self.dv_max = 15;
            self.dv_text = "DV";
            self.spc_def = None;
            self.nature = None;
            self.ability = None;
            self.hidden_power = if cur_gen == 2 { Some(String::new()) } else { None };
        }
        self.atk = init_dvs.attack.to_string();
        self.def = init_dvs.defense.to_string();
        self.spd = init_dvs.speed.to_string();
        self.spa = init_dvs.special_attack.to_string();
        if cur_gen <= 2 {
            self.recalc_hp_dv();
        }
        self.recalc_hidden_power();
    }

    fn recalc_hp_dv(&mut self) {
        if let (Ok(atk), Ok(def), Ok(spd), Ok(spc)) = (self.atk.trim().parse::<i64>(), self.def.trim().parse::<i64>(), self.spd.trim().parse::<i64>(), self.spa.trim().parse::<i64>()) {
            let hp = (atk % 2) * 8 + (def % 2) * 4 + (spd % 2) * 2 + (spc % 2);
            self.hp = hp.to_string();
        }
    }

    /// `recalc_hidden_power`
    pub fn recalc_hidden_power(&mut self) {
        if let Some(g) = &self.gen {
            if g.get_generation() <= 2 {
                self.recalc_hp_dv();
            }
        }
        if self.hidden_power.is_none() {
            return;
        }
        let Some(g) = self.gen.clone() else { return };
        let spc_def = self.spc_def.clone().unwrap_or_else(|| self.spa.clone());
        let parsed = (
            self.hp.trim().parse::<i64>(),
            self.atk.trim().parse::<i64>(),
            self.def.trim().parse::<i64>(),
            self.spa.trim().parse::<i64>(),
            spc_def.trim().parse::<i64>(),
            self.spd.trim().parse::<i64>(),
        );
        match parsed {
            (Ok(hp), Ok(atk), Ok(def), Ok(spa), Ok(spd_), Ok(spe)) => {
                let block = StatBlock::new(g.gen, hp, atk, def, spa, spd_, spe, false);
                let (hp_type, hp_power) = g.get_hidden_power(&block);
                self.hidden_power = Some(if hp_type.is_empty() { "Not supported in gen 1".to_string() } else { format!("{}: {}", hp_type, hp_power) });
            }
            _ => self.hidden_power = Some("Failed to calculate, invalid DVs".to_string()),
        }
    }

    /// `get_dvs()` -> (stat block, ability idx, nature)
    pub fn get_dvs(&self) -> Option<(StatBlock, i64, Nature)> {
        let g = self.gen.as_ref()?;
        let nature = self.nature.as_ref().map(|n| n.get_nature()).unwrap_or(Nature::HARDY);
        let ability = self.ability.as_ref().and_then(|a| a.index()).unwrap_or(0) as i64;
        let spc_def = self.spc_def.clone().unwrap_or_else(|| self.spa.clone());
        let parse = |s: &str| s.trim().parse::<i64>().ok();
        let block = StatBlock::new(g.gen, parse(&self.hp)?, parse(&self.atk)?, parse(&self.def)?, parse(&self.spa)?, parse(&spc_def)?, parse(&self.spd)?, false);
        Some((block, ability, nature))
    }

    pub fn ui(&mut self, ui: &mut Ui, theme: &Theme) {
        let Some(g) = self.gen.clone() else { return };
        let cur_gen = g.get_generation();
        let dv_text = self.dv_text;
        let dv_max = self.dv_max;
        let mut changed = false;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(5.0, 5.0);
            egui::Grid::new(ui.id().with("dv_grid")).spacing(Vec2::new(5.0, 5.0)).show(ui, |ui| {
                let hp_enabled = cur_gen > 2;
                widgets::label(ui, theme, format!("HP {}:", dv_text));
                changed |= AmountEntry::new(theme, ui.id().with("hp"), &mut self.hp).min(Some(0)).max(Some(dv_max)).enabled(hp_enabled).show(ui).changed;
                ui.end_row();
                widgets::label(ui, theme, format!("Attack {}:", dv_text));
                changed |= AmountEntry::new(theme, ui.id().with("atk"), &mut self.atk).min(Some(0)).max(Some(dv_max)).show(ui).changed;
                ui.end_row();
                widgets::label(ui, theme, format!("Defense {}:", dv_text));
                changed |= AmountEntry::new(theme, ui.id().with("def"), &mut self.def).min(Some(0)).max(Some(dv_max)).show(ui).changed;
                ui.end_row();
                widgets::label(ui, theme, format!("Speed {}:", dv_text));
                changed |= AmountEntry::new(theme, ui.id().with("spd"), &mut self.spd).min(Some(0)).max(Some(dv_max)).show(ui).changed;
                ui.end_row();
                let spa_label = if cur_gen > 2 { format!("Special Attack {}:", dv_text) } else { format!("Special {}:", dv_text) };
                widgets::label(ui, theme, spa_label);
                changed |= AmountEntry::new(theme, ui.id().with("spa"), &mut self.spa).min(Some(0)).max(Some(dv_max)).show(ui).changed;
                ui.end_row();
                if let Some(sd) = self.spc_def.as_mut() {
                    widgets::label(ui, theme, format!("Special Defense {}:", dv_text));
                    changed |= AmountEntry::new(theme, ui.id().with("spc_def"), sd).min(Some(0)).max(Some(dv_max)).show(ui).changed;
                    ui.end_row();
                }
                if let Some(hp) = &self.hidden_power {
                    widgets::label(ui, theme, "Hidden Power:");
                    widgets::label(ui, theme, hp.clone());
                    ui.end_row();
                }
            });
            if self.nature.is_some() || self.ability.is_some() {
                ui.add_space(20.0);
                ui.vertical(|ui| {
                    if let Some(n) = self.nature.as_mut() {
                        n.ui(ui, theme);
                    }
                    if let Some(a) = self.ability.as_mut() {
                        ui.horizontal(|ui| {
                            widgets::label(ui, theme, "Ability:");
                            a.ui(ui, theme, ui.id().with("ability"), Some(120.0), true);
                        });
                    }
                });
            }
        });
        if changed {
            self.recalc_hidden_power();
        }
    }
}
