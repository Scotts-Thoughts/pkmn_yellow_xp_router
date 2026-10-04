//! The Dex page's Damage tab: Solodex's free-form "my Pokémon against a
//! trainer" calculator, computed and drawn by the router's own battle
//! summary (xpr-calc + `BattleSummaryUi`), so its numbers and its controls
//! match the route editor's.
//!
//! The left panel builds the player's Pokémon (species, level, DVs / IVs,
//! nature, ability, stat exp / EVs, held item, badges, moves) or copies it
//! from the open route; the right side is the battle summary against the
//! chosen trainer's team. It runs on a route-less controller of its own (like
//! the map viewer), so nothing here touches the open route.

use std::collections::BTreeSet;
use std::sync::Arc;

use egui::{Align2, Color32, CornerRadius, Rect, Sense, Ui, Vec2};
use xpr_calc::battle_summary::BattleSummary;
use xpr_core::{consts, Config, Paths};
use xpr_data::{exp, GenData, Nature, Registry, StatBlock};
use xpr_dex_ui::{palette, DexCx};
use xpr_engine::state::SoloPokemonArgs;
use xpr_engine::{Inventory, RouteState, SoloPokemon};
use xpr_ui_kit::theme::Theme;
use xpr_ui_kit::widgets::{self, AmountEntry, SearchableDropdown, StyledButton};

use crate::assets::Assets;
use crate::battle::BattleController;
use crate::battle_ui::{BattleSummaryUi, BattleUiActions};
use crate::controller::MainController;

/// Width of the player panel.
const PANEL_W: f32 = 360.0;
/// Width of the controls in the panel's second column.
const FIELD_W: f32 = 250.0;

/// Stat order of the DV / IV and stat exp / EV fields.
const HP: usize = 0;
const ATK: usize = 1;
const DEF: usize = 2;
const SPA: usize = 3;
const SPD: usize = 4;
const SPE: usize = 5;

pub struct DexDamage {
    ctrl: MainController,
    bc: BattleController,
    bui: BattleSummaryUi,
    version: Option<String>,
    gen: Option<Arc<GenData>>,
    species: String,
    level: String,
    /// DVs (gens 1-2: SpA holds Special, HP is derived) or IVs, by stat
    ivs: [String; 6],
    /// stat exp (gens 1-2: SpA holds Special) or EVs, by stat
    stat_xp: [String; 6],
    nature: String,
    ability: String,
    held_item: String,
    badges: BTreeSet<String>,
    moves: [String; 4],
    trainer: String,
    last_nonce: u64,
    dirty: bool,
    error: Option<String>,
    panel_open: bool,
    /// the first-show defaults were applied
    seeded: bool,
    /// the battle summary's "Configure/Help" was clicked (the app opens the dialog)
    pub open_battle_config: bool,
    species_options: Vec<String>,
    move_options: Vec<String>,
    trainer_options: Vec<String>,
    item_options: Vec<String>,
    nature_options: Vec<String>,
}

impl DexDamage {
    pub fn new(registry: Arc<Registry>, paths: Paths) -> DexDamage {
        DexDamage {
            ctrl: MainController::new(registry, paths),
            bc: BattleController::new(),
            bui: BattleSummaryUi::new(),
            version: None,
            gen: None,
            species: String::new(),
            level: "50".to_string(),
            ivs: Default::default(),
            stat_xp: ["0", "0", "0", "0", "0", "0"].map(String::from),
            nature: nature_label(Nature::HARDY),
            ability: String::new(),
            held_item: String::new(),
            badges: BTreeSet::new(),
            moves: Default::default(),
            trainer: String::new(),
            last_nonce: 0,
            dirty: true,
            error: None,
            panel_open: true,
            seeded: false,
            open_battle_config: false,
            species_options: Vec::new(),
            move_options: Vec::new(),
            trainer_options: Vec::new(),
            item_options: Vec::new(),
            nature_options: (0..25).filter_map(Nature::from_index).map(nature_label).collect(),
        }
    }

    /// Switch the calculator to a router version, keeping what still exists there.
    fn set_version(&mut self, version: &str) {
        if self.version.as_deref() == Some(version) {
            return;
        }
        if let Err(e) = self.ctrl.router.change_version(version) {
            self.error = Some(format!("Could not load {}: {}", version, e));
            return;
        }
        let old_gen = self.gen.as_ref().map(|g| g.get_generation());
        self.version = Some(version.to_string());
        self.gen = self.ctrl.gen();
        let Some(gen) = self.gen.clone() else { return };
        self.bui.configure_for_version(&self.ctrl);
        self.bui.show_contents();
        self.bui.free_form = true;
        self.species_options = gen.pkmn_db().get_all_names(None);
        let mut moves = gen.move_db().get_filtered_names(None, false);
        moves.insert(0, String::new());
        self.move_options = moves;
        self.trainer_options = gen.trainer_db().iter().filter(|t| !t.pkmn.is_empty()).map(|t| t.name.clone()).collect();
        let mut items = BattleSummary::get_held_item_options(&gen);
        if !items.iter().any(|i| i.is_empty()) {
            items.insert(0, String::new());
        }
        self.item_options = items;
        if !self.species_options.contains(&self.species) {
            self.species = self.species_options.first().cloned().unwrap_or_default();
        }
        if !self.trainer_options.contains(&self.trainer) {
            self.trainer = gen.get_gym_leader_names().into_iter().find(|t| self.trainer_options.contains(t)).or_else(|| self.trainer_options.first().cloned()).unwrap_or_default();
        }
        for m in self.moves.iter_mut() {
            if !m.is_empty() && gen.move_db().get_move(m).is_none() {
                m.clear();
            }
        }
        self.badges.retain(|b| badge_slots(&gen).iter().any(|(slot, _)| slot == b));
        // DVs and IVs have different ranges: start over at the maximum
        let g = gen.get_generation();
        if old_gen.map(|o| (o <= 2) != (g <= 2)).unwrap_or(true) {
            let max = if g <= 2 { 15 } else { 31 };
            self.ivs = [max; 6].map(|v: i32| v.to_string());
            self.stat_xp = ["0", "0", "0", "0", "0", "0"].map(String::from);
        }
        self.fix_ability();
        if self.moves.iter().all(|m| m.is_empty()) {
            self.default_moves();
        }
        self.dirty = true;
    }

    fn abilities(&self) -> Vec<String> {
        self.gen.as_ref().and_then(|g| g.pkmn_db().get_pkmn(&self.species).map(|p| p.abilities.clone())).unwrap_or_default()
    }

    fn fix_ability(&mut self) {
        let abilities = self.abilities();
        if !abilities.contains(&self.ability) {
            self.ability = abilities.first().cloned().unwrap_or_default();
        }
    }

    /// The level-up moves the species knows at the current level.
    fn default_moves(&mut self) {
        if let Some(solo) = self.build_solo(true) {
            for (slot, m) in self.moves.iter_mut().zip(solo.move_list.iter()) {
                *slot = m.clone().unwrap_or_default();
            }
            self.dirty = true;
        }
    }

    fn level_value(&self) -> i64 {
        self.level.trim().parse::<i64>().unwrap_or(50).clamp(1, 100)
    }

    fn num(s: &str) -> i64 {
        s.trim().parse::<i64>().unwrap_or(0).max(0)
    }

    /// DVs / IVs as a stat block (gens 1-2: HP from the other DVs' low bits).
    fn dv_block(&self, gen: &GenData) -> StatBlock {
        let v: Vec<i64> = self.ivs.iter().map(|s| Self::num(s)).collect();
        if gen.get_generation() <= 2 {
            let c = |x: i64| x.min(15);
            let (atk, def, spe, spc) = (c(v[ATK]), c(v[DEF]), c(v[SPE]), c(v[SPA]));
            let hp = ((atk & 1) << 3) | ((def & 1) << 2) | ((spe & 1) << 1) | (spc & 1);
            StatBlock::new(gen.gen, hp, atk, def, spc, spc, spe, false)
        } else {
            let c = |x: i64| x.min(31);
            StatBlock::new(gen.gen, c(v[HP]), c(v[ATK]), c(v[DEF]), c(v[SPA]), c(v[SPD]), c(v[SPE]), false)
        }
    }

    fn stat_xp_block(&self, gen: &GenData) -> StatBlock {
        let v: Vec<i64> = self.stat_xp.iter().map(|s| Self::num(s)).collect();
        if gen.get_generation() <= 2 {
            let c = |x: i64| x.min(65535);
            gen.make_stat_block(c(v[HP]), c(v[ATK]), c(v[DEF]), c(v[SPA]), c(v[SPA]), c(v[SPE]), true)
        } else {
            let c = |x: i64| x.min(255);
            gen.make_stat_block(c(v[HP]), c(v[ATK]), c(v[DEF]), c(v[SPA]), c(v[SPD]), c(v[SPE]), true)
        }
    }

    /// The player's Pokémon as configured (`default_moves`: let the species'
    /// level-up moves fill the move list).
    fn build_solo(&self, default_moves: bool) -> Option<SoloPokemon> {
        let gen = self.gen.clone()?;
        let species = gen.pkmn_db().get_pkmn(&self.species)?.clone();
        let lookup = exp::level_lookup(&species.growth_rate)?;
        let cur_xp = lookup.get_xp_for_level(self.level_value()).ok()?;
        let stat_xp = self.stat_xp_block(&gen);
        let badges = self.badge_list(&gen);
        let move_list = if default_moves { None } else { Some(self.moves.iter().map(|m| if m.is_empty() { None } else { Some(m.clone()) }).collect()) };
        let held_item = Some(self.held_item.trim().to_string()).filter(|s| !s.is_empty());
        let args = SoloPokemonArgs { move_list, cur_xp, realized_stat_xp: Some(stat_xp), unrealized_stat_xp: Some(stat_xp), held_item, ..Default::default() };
        let ability_idx = species.abilities.iter().position(|a| *a == self.ability).unwrap_or(0) as i64;
        let nature = if gen.get_generation() >= 3 { nature_from_label(&self.nature) } else { Nature::HARDY };
        SoloPokemon::new(&self.species, species, self.dv_block(&gen), badges, gen.make_stat_block(0, 0, 0, 0, 0, 0, true), ability_idx, nature, args).ok()
    }

    fn badge_list(&self, gen: &GenData) -> xpr_data::BadgeList {
        let mut badges = gen.make_badge_list();
        for (slot, trainer) in badge_slots(gen) {
            if self.badges.contains(&slot) {
                badges = badges.award_badge(&trainer);
            }
        }
        badges
    }

    /// Rebuild the player and reload the summary against the trainer.
    fn reload(&mut self, cfg: &Config) {
        self.dirty = false;
        self.error = None;
        let Some(gen) = self.gen.clone() else { return };
        let Some(solo) = self.build_solo(false) else {
            self.error = Some("Pick a Pok\u{e9}mon".to_string());
            return;
        };
        let badges = solo.badges.clone();
        let state = RouteState::new(solo, badges, Inventory::new(None, Vec::new(), gen.bag_limit()));
        let Some(trainer) = gen.trainer_db().get_trainer(&self.trainer).cloned() else {
            self.error = Some("Pick a trainer".to_string());
            self.bc.load_empty(cfg, &self.ctrl);
            return;
        };
        self.bc.load_from_state_named(cfg, &mut self.ctrl, &state, &trainer.pkmn, Some(&trainer.name), false);
    }

    /// Copy the Pokémon of the open route (the state entering the selected
    /// event, else the end of the route).
    fn copy_from_route(&mut self, route: &MainController) {
        let state = route.get_single_selected_event_id(true).and_then(|id| route.router.init_state_of(id).cloned()).or_else(|| route.get_final_state());
        let Some(state) = state else { return };
        let Some(gen) = self.gen.clone() else { return };
        let solo = &state.solo_pkmn;
        self.species = solo.name.clone();
        self.level = solo.cur_level.to_string();
        let d = solo.dvs;
        self.ivs = [d.hp, d.attack, d.defense, d.special_attack, d.special_defense, d.speed].map(|v| v.to_string());
        let sx = solo.realized_stat_xp;
        self.stat_xp = [sx.hp, sx.attack, sx.defense, sx.special_attack, sx.special_defense, sx.speed].map(|v| v.to_string());
        self.nature = nature_label(solo.nature);
        self.ability = solo.ability.clone();
        self.held_item = solo.held_item.clone().unwrap_or_default();
        self.moves = [0, 1, 2, 3].map(|i| solo.move_list.get(i).cloned().flatten().unwrap_or_default());
        self.badges = badge_slots(&gen).into_iter().filter(|(slot, _)| state.badges.has(slot)).map(|(slot, _)| slot).collect();
        self.fix_ability();
        self.dirty = true;
    }

    /// The first time the tab shows: the open route's Pokémon when it is of
    /// this game (as the battle summary would show it), else the Pokédex's
    /// selected species (Solodex opened the calculator on it).
    fn seed(&mut self, cx: &mut DexCx, route: &MainController) {
        let Some(gen) = self.gen.clone() else { return };
        if route.get_version().is_some() && route.get_version() == self.version.as_deref() {
            self.copy_from_route(route);
        } else if let Some(s) = cx.state.selected().and_then(|s| resolve_router_species(&gen, s)) {
            self.species = s;
            self.fix_ability();
            self.default_moves();
            // the Pokedex's right-clicked test set fills the move slots
            let test: Vec<String> = cx.state.move_test_set.iter().filter_map(|m| resolve_router_move(&gen, m)).take(4).collect();
            if !test.is_empty() {
                self.moves = Default::default();
                for (slot, m) in self.moves.iter_mut().zip(test) {
                    *slot = m;
                }
            }
            self.dirty = true;
        }
    }

    /// Apply what another Dex tab asked for (`DamageRequest`).
    fn apply_request(&mut self, cx: &mut DexCx) {
        let req = cx.state.damage_request.clone();
        if req.nonce == self.last_nonce {
            return;
        }
        self.last_nonce = req.nonce;
        if let Some(v) = &req.version {
            cx.state.set_version(v, cx.registry);
            self.set_version(v);
        }
        let Some(gen) = self.gen.clone() else { return };
        if let Some(s) = req.species.as_deref().and_then(|s| resolve_router_species(&gen, s)) {
            if s != self.species {
                self.species = s;
                self.fix_ability();
                if req.moves.is_empty() {
                    self.default_moves();
                }
            }
        }
        if let Some(t) = req.trainer.as_deref().filter(|t| gen.trainer_db().get_trainer(t).is_some()) {
            self.trainer = t.to_string();
        }
        if !req.moves.is_empty() {
            let resolved: Vec<String> = req.moves.iter().filter_map(|m| resolve_router_move(&gen, m)).take(4).collect();
            self.moves = Default::default();
            for (slot, m) in self.moves.iter_mut().zip(resolved) {
                *slot = m;
            }
        }
        self.dirty = true;
    }

    /// The tab body.
    pub fn ui(&mut self, ui: &mut Ui, cx: &mut DexCx, cfg: &mut Config, assets: &mut Assets, route: &MainController) {
        let theme = cx.theme.clone();
        if let Some(v) = cx.state.settings.version.clone() {
            self.set_version(&v);
        }
        // first show: start on the route's / Pokédex's Pokémon, then let a
        // request from another tab override it
        if !self.seeded {
            self.seeded = true;
            self.seed(cx, route);
        }
        self.apply_request(cx);
        if self.gen.is_none() {
            xpr_dex_ui::widgets::placeholder(ui, &theme, self.error.as_deref().unwrap_or("Pick a game above"));
            return;
        }
        let route_matches = route.get_version().is_some() && route.get_version() == self.version.as_deref();
        let full = ui.available_rect_before_wrap();
        let pw = if self.panel_open { PANEL_W } else { 0.0 };
        let left = Rect::from_min_size(full.min, Vec2::new(pw, full.height()));
        let right = Rect::from_min_max(egui::pos2(left.max.x + 1.0, full.min.y), full.max);
        if self.panel_open {
            let mut lui = ui.new_child(egui::UiBuilder::new().max_rect(left.shrink2(Vec2::new(8.0, 6.0))).layout(egui::Layout::top_down(egui::Align::Min)));
            lui.set_clip_rect(left.intersect(ui.clip_rect()));
            widgets::show_scroll(&mut lui, egui::ScrollArea::vertical().id_salt("dex_damage_panel").auto_shrink([false, false]), |ui| {
                self.player_panel(ui, &theme, route, route_matches);
            });
            ui.painter().vline(left.max.x, full.y_range(), egui::Stroke::new(1.0_f32, palette::GRAY_700));
        }
        if self.dirty {
            self.reload(cfg);
        }
        self.bui.tick(ui.ctx(), &mut self.bc, &mut self.ctrl, cfg);
        if self.bc.take_signals().refresh {
            self.bui.sync_from_controller(&self.bc, &self.ctrl);
        }
        {
            let mut rui = ui.new_child(egui::UiBuilder::new().max_rect(right.shrink(4.0)).layout(egui::Layout::top_down(egui::Align::Min)));
            rui.set_clip_rect(right.intersect(ui.clip_rect()));
            // the panel's show / hide toggle
            let (tb, _) = rui.allocate_exact_size(Vec2::new(22.0, 22.0), Sense::hover());
            let tr = ui.interact(tb, ui.id().with("dex_damage_panel_toggle"), Sense::click()).on_hover_text(if self.panel_open { "Hide the player panel" } else { "Show the player panel" });
            ui.painter().rect_filled(tb, CornerRadius::same(4), if tr.hovered() { palette::GRAY_700 } else { palette::GRAY_800 });
            ui.painter().text(tb.center(), Align2::CENTER_CENTER, if self.panel_open { "\u{25C0}" } else { "\u{25B6}" }, palette::px(&theme, 12.0), if tr.hovered() { Color32::WHITE } else { palette::GRAY_400 });
            if tr.clicked() {
                self.panel_open = !self.panel_open;
            }
            if let Some(e) = &self.error {
                xpr_dex_ui::widgets::placeholder(&mut rui, &theme, e);
            } else {
                let mut actions = BattleUiActions::default();
                widgets::show_scroll(&mut rui, egui::ScrollArea::vertical().id_salt("dex_damage_summary").auto_shrink([false, false]), |ui| {
                    self.bui.ui(ui, &theme, cfg, &mut self.bc, &mut self.ctrl, assets, &mut actions);
                });
                if actions.open_config {
                    self.open_battle_config = true;
                }
            }
        }
        ui.allocate_rect(full, Sense::hover());
        if self.bc.take_signals().refresh {
            self.bui.sync_from_controller(&self.bc, &self.ctrl);
        }
    }

    fn player_panel(&mut self, ui: &mut Ui, theme: &Theme, route: &MainController, route_matches: bool) {
        let Some(gen) = self.gen.clone() else { return };
        let g = gen.get_generation();
        let pw = ui.available_width();
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
        xpr_dex_ui::widgets::section_label(ui, theme, "Your Pok\u{e9}mon");
        let copy = StyledButton::new(theme, "Use the Route's Pok\u{e9}mon").enabled(route_matches).fixed_width(pw - 4.0).show(ui).on_hover_text(if route_matches {
            "Copy the solo Pok\u{e9}mon of the open route as it enters the selected event (or at the end of the route)"
        } else {
            "Open a route of this game to copy its Pok\u{e9}mon"
        });
        if copy.clicked() {
            self.copy_from_route(route);
        }
        row(ui, theme, "Species", |ui| {
            let r = SearchableDropdown::new(theme, ui.id().with("dex_dmg_species"), &mut self.species, &self.species_options).widths(FIELD_W, FIELD_W).show(ui);
            if r.changed && self.species_options.contains(&self.species) {
                self.fix_ability();
                self.default_moves();
                self.dirty = true;
            }
        });
        row(ui, theme, "Level", |ui| {
            let r = AmountEntry::new(theme, ui.id().with("dex_dmg_level"), &mut self.level).min(Some(1)).max(Some(100)).width(Some(6)).show(ui);
            if r.changed {
                self.dirty = true;
            }
        });
        if g >= 3 {
            row(ui, theme, "Nature", |ui| {
                if widgets::option_menu(ui, theme, ui.id().with("dex_dmg_nature"), &mut self.nature, &self.nature_options, Some(FIELD_W), true) {
                    self.dirty = true;
                }
            });
            let abilities = self.abilities();
            if !abilities.is_empty() {
                row(ui, theme, "Ability", |ui| {
                    if widgets::option_menu(ui, theme, ui.id().with("dex_dmg_ability"), &mut self.ability, &abilities, Some(FIELD_W), true) {
                        self.dirty = true;
                    }
                });
            }
        }
        // DVs / IVs and stat exp / EVs
        let fields: &[(usize, &str)] = if g <= 2 { &[(HP, "HP"), (ATK, "Atk"), (DEF, "Def"), (SPA, "Spc"), (SPE, "Spe")] } else { &[(HP, "HP"), (ATK, "Atk"), (DEF, "Def"), (SPA, "SpA"), (SPD, "SpD"), (SPE, "Spe")] };
        let dv_max = if g <= 2 { 15 } else { 31 };
        let xp_max = if g <= 2 { 65535 } else { 252 };
        let derived_hp = (g <= 2).then(|| self.dv_block(&gen).hp);
        egui::Grid::new(ui.id().with("dex_dmg_spread")).num_columns(3).spacing(Vec2::new(8.0, 4.0)).show(ui, |ui| {
            ui.label("");
            widgets::label_colored(ui, theme, if g <= 2 { "DV" } else { "IV" }, palette::GRAY_500);
            widgets::label_colored(ui, theme, if g <= 2 { "Stat Exp" } else { "EV" }, palette::GRAY_500);
            ui.end_row();
            for (i, label) in fields {
                widgets::label(ui, theme, *label);
                match derived_hp.filter(|_| *i == HP) {
                    Some(hp) => {
                        widgets::label_colored(ui, theme, format!("{} (from the others)", hp), palette::GRAY_500).on_hover_text("The HP DV is made of the low bits of the Attack, Defense, Speed and Special DVs");
                    }
                    None => {
                        let r = AmountEntry::new(theme, ui.id().with(("dex_dmg_iv", *i)), &mut self.ivs[*i]).min(Some(0)).max(Some(dv_max)).width(Some(5)).show(ui);
                        if r.changed {
                            self.dirty = true;
                        }
                    }
                }
                let r = AmountEntry::new(theme, ui.id().with(("dex_dmg_sx", *i)), &mut self.stat_xp[*i]).min(Some(0)).max(Some(xp_max)).width(Some(8)).show(ui);
                if r.changed {
                    self.dirty = true;
                }
                ui.end_row();
            }
        });
        if g >= 2 {
            let (hp_type, hp_power) = gen.get_hidden_power(&self.dv_block(&gen));
            if !hp_type.is_empty() {
                widgets::label_colored(ui, theme, format!("Hidden Power: {} {}", hp_type, hp_power), palette::GRAY_400);
            }
            row(ui, theme, "Held item", |ui| {
                let r = SearchableDropdown::new(theme, ui.id().with("dex_dmg_item"), &mut self.held_item, &self.item_options).widths(FIELD_W, FIELD_W).show(ui);
                if r.changed {
                    self.dirty = true;
                }
            });
        }
        let slots = badge_slots(&gen);
        if g <= 3 && !slots.is_empty() {
            xpr_dex_ui::widgets::section_label(ui, theme, "Badges");
            ui.horizontal_wrapped(|ui| {
                for (slot, _) in &slots {
                    let mut on = self.badges.contains(slot);
                    if widgets::checkbox(ui, theme, &mut on, &capitalize(slot), true).changed() {
                        if on {
                            self.badges.insert(slot.clone());
                        } else {
                            self.badges.remove(slot);
                        }
                        self.dirty = true;
                    }
                }
            });
        }
        xpr_dex_ui::widgets::section_label(ui, theme, "Moves");
        for i in 0..4 {
            row(ui, theme, &format!("{}.", i + 1), |ui| {
                let r = SearchableDropdown::new(theme, ui.id().with(("dex_dmg_move", i)), &mut self.moves[i], &self.move_options).widths(FIELD_W, FIELD_W).placeholder("\u{2014} empty \u{2014}").show(ui);
                if r.changed {
                    self.dirty = true;
                }
            });
        }
        if StyledButton::new(theme, "Level-Up Moves at This Level").fixed_width(pw - 4.0).show(ui).clicked() {
            self.default_moves();
        }
        if let Some(solo) = self.build_solo(false) {
            let s = solo.cur_stats;
            let text = if g <= 2 {
                format!("Lv {}   HP {}  Atk {}  Def {}  Spc {}  Spe {}", solo.cur_level, s.hp, s.attack, s.defense, s.special_attack, s.speed)
            } else {
                format!("Lv {}   HP {}  Atk {}  Def {}  SpA {}  SpD {}  Spe {}", solo.cur_level, s.hp, s.attack, s.defense, s.special_attack, s.special_defense, s.speed)
            };
            widgets::label_colored(ui, theme, text, palette::GRAY_400);
        }
        ui.add_space(6.0);
        xpr_dex_ui::widgets::section_label(ui, theme, "Opponent");
        row(ui, theme, "Trainer", |ui| {
            let r = SearchableDropdown::new(theme, ui.id().with("dex_dmg_trainer"), &mut self.trainer, &self.trainer_options).widths(FIELD_W, 300.0).show(ui);
            if r.changed {
                self.dirty = true;
            }
        });
        if let Some(t) = gen.trainer_db().get_trainer(&self.trainer) {
            let team: Vec<String> = t.pkmn.iter().map(|p| format!("{} Lv{}", p.name, p.level)).collect();
            ui.add(egui::Label::new(egui::RichText::new(team.join(", ")).font(palette::px(theme, 12.0)).color(palette::trainer_category_color(gen.get_fight_category(&t.name)))).wrap());
            if !t.location.is_empty() {
                widgets::label_colored(ui, theme, &t.location, palette::GRAY_500);
            }
        }
        ui.add_space(4.0);
        ui.add(egui::Label::new(egui::RichText::new("As in the route editor, the player gains experience from each Pok\u{e9}mon it beats before the next.").font(palette::px(theme, 11.0)).color(palette::GRAY_500)).wrap());
    }
}

/// `label` in a fixed column, then the control.
fn row(ui: &mut Ui, theme: &Theme, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::new(70.0, 20.0), Sense::hover());
        ui.painter().text(egui::pos2(r.min.x, r.center().y), Align2::LEFT_CENTER, label, palette::px(theme, 12.0), palette::GRAY_400);
        add(ui);
    });
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// "Adamant (+Atk, -SpA)"
fn nature_label(n: Nature) -> String {
    let stats = [(consts::ATTACK, "Atk"), (consts::DEFENSE, "Def"), (consts::SPECIAL_ATTACK, "SpA"), (consts::SPECIAL_DEFENSE, "SpD"), (consts::SPEED, "Spe")];
    let up = stats.iter().find(|(k, _)| n.is_stat_raised(k)).map(|(_, l)| *l);
    let down = stats.iter().find(|(k, _)| n.is_stat_lowered(k)).map(|(_, l)| *l);
    match (up, down) {
        (Some(u), Some(d)) => format!("{} (+{}, -{})", n.display_name(), u, d),
        _ => n.display_name(),
    }
}

fn nature_from_label(label: &str) -> Nature {
    Nature::from_name(label.split(' ').next().unwrap_or("")).unwrap_or(Nature::HARDY)
}

/// (badge slot, a trainer that awards it), in the game's badge order: the
/// badges of every region (block of 8 slots) whose leaders this version has.
/// The rewards table is per generation, so Emerald's also lists FireRed's
/// leaders (and gives Rain to "Leader Wallace", whom Emerald calls Juan).
fn badge_slots(gen: &GenData) -> Vec<(String, String)> {
    let badges = gen.make_badge_list();
    let slots = badges.slot_names();
    let awarder = |slot: &str, own: bool| badges.rewards().iter().find(|(t, _)| (!own || gen.trainer_db().get_trainer(t).is_some()) && badges.award_badge(t).has(slot)).map(|(t, _)| t.clone());
    let blocks: BTreeSet<usize> = slots.iter().enumerate().filter(|(_, s)| awarder(s, true).is_some()).map(|(i, _)| i / 8).collect();
    slots.iter().enumerate().filter(|(i, _)| blocks.contains(&(i / 8))).filter_map(|(_, s)| awarder(s, false).map(|t| (s.to_string(), t))).collect()
}

/// The router's name of a species named by the Dex (or anyone else).
pub fn resolve_router_species(gen: &GenData, name: &str) -> Option<String> {
    if gen.pkmn_db().get_pkmn(name).is_some() {
        return Some(name.to_string());
    }
    let key = xpr_dex::species::loose_key(xpr_dex::display_name(name));
    gen.pkmn_db().iter().find(|p| xpr_dex::species::loose_key(&p.name) == key).map(|p| p.name.clone())
}

/// The router's name of a move named by the Dex (spellings differ: "Double-Edge"
/// / "Double Edge", "Feint Attack" / "Faint Attack").
pub fn resolve_router_move(gen: &GenData, name: &str) -> Option<String> {
    if gen.move_db().get_move(name).is_some() {
        return Some(name.to_string());
    }
    let key = xpr_dex::canonical_move_key(name);
    gen.move_db().iter().find(|m| xpr_dex::canonical_move_key(&m.name) == key).map(|m| m.name.clone())
}
