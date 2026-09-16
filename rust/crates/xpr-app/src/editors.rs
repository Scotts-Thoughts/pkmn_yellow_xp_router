//! Port of `gui_qt/event_editors.py`: one editor per event type, each holding
//! its own widget state, loading from an `EventDefinition` and producing one.

use egui::{Color32, Id, Pos2, Sense, Ui, Vec2};
use serde_json::Value;

use xpr_core::consts;
use xpr_core::Config;
use xpr_data::model::EnemyPkmn;
use xpr_data::GenData;
use xpr_engine::{
    swaps_between, BagSwap, EventDefinition, EvolutionEventDefinition, HoldItemEventDefinition, InventoryEventDefinition,
    LearnMoveEventDefinition, LevelVal, LocationEventDefinition, RouteState,
    TrainerEventDefinition, VitaminEventDefinition, WildPkmnEventDefinition,
};
use xpr_ui_kit::theme::Theme;
use xpr_ui_kit::widgets::{self, AmountEntry, Entry};

/// `SimpleOptionMenu`'s option list + selection semantics.
#[derive(Clone, Debug, Default)]
pub struct OptionMenu {
    pub options: Vec<String>,
    pub current: String,
}

impl OptionMenu {
    pub fn new(options: Vec<String>, default_val: Option<&str>) -> OptionMenu {
        let mut m = OptionMenu::default();
        m.new_values(options, default_val);
        m
    }

    /// `new_values(option_list, default_val)`
    pub fn new_values(&mut self, options: Vec<String>, default_val: Option<&str>) {
        if options == self.options {
            if let Some(d) = default_val {
                // setCurrentText on a non-editable combo only changes to a known value
                if self.options.iter().any(|o| o == d) {
                    self.current = d.to_string();
                }
            }
            return;
        }
        self.options = options;
        match default_val {
            Some(d) if self.options.iter().any(|o| o == d) => self.current = d.to_string(),
            _ => self.current = self.options.first().cloned().unwrap_or_default(),
        }
    }

    /// `set(val)`: only known values are accepted.
    pub fn set(&mut self, val: &str) {
        if self.options.iter().any(|o| o == val) {
            self.current = val.to_string();
        }
    }

    pub fn get(&self) -> &str {
        &self.current
    }

    pub fn index(&self) -> Option<usize> {
        self.options.iter().position(|o| *o == self.current)
    }

    /// Draw the combo; returns true on change.
    pub fn ui(&mut self, ui: &mut Ui, theme: &Theme, id: Id, width: Option<f32>, enabled: bool) -> bool {
        widgets::option_menu(ui, theme, id, &mut self.current, &self.options, width, enabled)
    }
}

/// What an editor's `ui` reported.
#[derive(Clone, Copy, Debug, Default)]
pub struct EditorOutput {
    pub save: bool,
    pub delayed_save: bool,
}

impl EditorOutput {
    fn save() -> EditorOutput {
        EditorOutput { save: true, delayed_save: false }
    }
    fn delayed() -> EditorOutput {
        EditorOutput { save: false, delayed_save: true }
    }
    fn merge(&mut self, o: EditorOutput) {
        self.save |= o.save;
        self.delayed_save |= o.delayed_save;
    }
}

/// `EditorParams`
pub struct EditorCtx<'a> {
    pub theme: &'a Theme,
    pub gen: &'a GenData,
    pub cfg: &'a Config,
    pub cur_state: Option<&'a RouteState>,
    pub event_type: &'a str,
    pub enabled: bool,
}

// ---------------------------------------------------------------------------
// Trainer fight editor
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct TrainerCard {
    html_lines: Vec<(String, String, String)>,
    name: String,
    ability_line: Option<String>,
    item_line: Option<String>,
    spe_color: Color32,
}

#[derive(Default)]
pub struct TrainerFightEditor {
    cur_trainer: String,
    second_trainer: Value,
    num_pkmn: usize,
    pay_day: String,
    exp_per_sec: String,
    cards: Vec<TrainerCard>,
    order_menus: Vec<OptionMenu>,
    exp_splits: Vec<OptionMenu>,
    cached_definition_order: Vec<usize>,
}

const EXP_PER_SEC_TEXT: &str = "Optimal exp per second (4x speed): ";

impl TrainerFightEditor {
    pub fn load_event(&mut self, ctx: &EditorCtx, def: &EventDefinition) {
        let Some(td) = &def.trainer_def else { return };
        self.cur_trainer = td.trainer_name.clone();
        self.second_trainer = td.second_trainer_name.clone();
        let pay_day_val = td.pay_day_amount_int().unwrap_or(0);
        self.exp_per_sec = format!("{} {}", EXP_PER_SEC_TEXT, def.experience_per_second(ctx.gen).unwrap_or_default());
        self.pay_day = pay_day_val.to_string();
        let ordered: Vec<EnemyPkmn> = def.pokemon_list(ctx.gen).unwrap_or_default();
        self.num_pkmn = ordered.len();
        let order_values: Vec<String> = (1..=ordered.len()).map(|v| v.to_string()).collect();
        self.cached_definition_order = def
            .get_pokemon_list(ctx.gen, true)
            .unwrap_or_default()
            .iter()
            .map(|(_, p)| (p.mon_order - 1).max(0) as usize)
            .collect();
        let mut cur_state: Option<RouteState> = ctx.cur_state.cloned();
        self.cards.clear();
        while self.order_menus.len() < 6 {
            self.order_menus.push(OptionMenu::new((1..=6).map(|v| v.to_string()).collect(), None));
            self.exp_splits.push(OptionMenu::new((1..=6).map(|v| v.to_string()).collect(), None));
        }
        for (idx, cur_pkmn) in ordered.iter().enumerate() {
            let speed_class = match &cur_state {
                Some(st) => {
                    let c = if st.solo_pkmn.cur_stats.speed > cur_pkmn.cur_stats.speed {
                        "success"
                    } else if st.solo_pkmn.cur_stats.speed == cur_pkmn.cur_stats.speed {
                        "warning"
                    } else {
                        "failure"
                    };
                    cur_state = st.defeat_pkmn(ctx.gen, cur_pkmn, None, 1, 0).ok().map(|r| r.0);
                    c
                }
                None => "contrast",
            };
            let spe_color = match speed_class {
                "success" => Color32::from_rgb(0x4e, 0xc9, 0x7a),
                "warning" => Color32::from_rgb(0xe8, 0xb7, 0x30),
                "failure" => Color32::from_rgb(0xe0, 0x55, 0x55),
                _ => Color32::from_rgb(0xd4, 0xd4, 0xd4),
            };
            let stats = &cur_pkmn.cur_stats;
            let xp_val = if cur_pkmn.xp != 0 { cur_pkmn.xp.to_string() } else { String::new() };
            let moves: Vec<String> = cur_pkmn.move_list.iter().filter_map(|m| m.clone()).filter(|m| !m.is_empty()).collect();
            let right = |i: usize, prefix: &str| if moves.len() > i { format!("{}{}", prefix, moves[i]) } else { String::new() };
            let rows = vec![
                ("HP:".to_string(), stats.hp.to_string(), format!("Lv: {}", cur_pkmn.level)),
                ("Atk:".to_string(), stats.attack.to_string(), if xp_val.is_empty() { String::new() } else { format!("Exp: {}", xp_val) }),
                ("Def:".to_string(), stats.defense.to_string(), right(0, "Move 1: ")),
                ("SpA:".to_string(), stats.special_attack.to_string(), right(1, "Move 2: ")),
                ("SpD:".to_string(), stats.special_defense.to_string(), right(2, "Move 3: ")),
                ("Spe:".to_string(), stats.speed.to_string(), right(3, "Move 4: ")),
            ];
            let ability_line = if !cur_pkmn.ability.is_empty() {
                let nature_str = if cur_pkmn.nature.display_name() != "Hardy" { format!(" ({})", cur_pkmn.nature.display_name()) } else { String::new() };
                Some(format!("{}{}", cur_pkmn.ability, nature_str))
            } else {
                None
            };
            let item_line = cur_pkmn.held_item.as_ref().filter(|h| !h.is_empty()).map(|h| format!("Item: {}", h));
            self.cards.push(TrainerCard { html_lines: rows, name: cur_pkmn.name.clone(), ability_line, item_line, spe_color });
            self.order_menus[idx].new_values(order_values.clone(), None);
            self.order_menus[idx].set(&cur_pkmn.mon_order.to_string());
            self.exp_splits[idx].set(&cur_pkmn.exp_split.to_string());
        }
        for idx in ordered.len()..6 {
            self.order_menus[idx].current = "-1".to_string();
        }
    }

    /// `_reorder_mons(updated_idx)`
    fn reorder_mons(&mut self, updated_idx: usize) {
        let adjusted_val: i64 = self.order_menus[updated_idx].get().parse().unwrap_or(-1);
        let mut ordered_indices: Vec<(i64, usize)> = Vec::new();
        for i in 0..self.num_pkmn {
            if i == updated_idx {
                continue;
            }
            let val: i64 = self.order_menus[i].get().parse().unwrap_or(-1);
            if val == -1 {
                continue;
            }
            ordered_indices.push((val, i));
        }
        ordered_indices.sort();
        let mut new_order_idx: i64 = 1;
        let mut oi_pos = 0usize;
        while new_order_idx <= self.num_pkmn as i64 {
            if new_order_idx == adjusted_val {
                new_order_idx += 1;
                continue;
            }
            if oi_pos < ordered_indices.len() {
                let target = ordered_indices[oi_pos].1;
                self.order_menus[target].set(&new_order_idx.to_string());
                oi_pos += 1;
            }
            new_order_idx += 1;
        }
    }

    pub fn get_event(&self) -> Result<EventDefinition, String> {
        let exp_split: Vec<i64> = self
            .cached_definition_order
            .iter()
            .map(|x| self.exp_splits.get(*x).and_then(|m| m.get().parse::<i64>().ok()).ok_or_else(|| "invalid exp split".to_string()))
            .collect::<Result<_, _>>()?;
        let mon_order: Vec<i64> = self
            .cached_definition_order
            .iter()
            .map(|x| self.order_menus.get(*x).and_then(|m| m.get().parse::<i64>().ok()).ok_or_else(|| "invalid mon order".to_string()))
            .collect::<Result<_, _>>()?;
        let pay_day_amount: i64 = self.pay_day.trim().parse().unwrap_or(0);
        let mut td = TrainerEventDefinition::new(&self.cur_trainer);
        td.second_trainer_name = self.second_trainer.clone();
        td.exp_split = exp_split;
        td.pay_day_amount = Some(pay_day_amount);
        td.mon_order = mon_order;
        Ok(EventDefinition::with_trainer(td))
    }

    pub fn ui(&mut self, ui: &mut Ui, ctx: &EditorCtx) -> (EditorOutput, bool) {
        let theme = ctx.theme;
        let mut out = EditorOutput::default();
        let mut reload = false;
        ui.horizontal(|ui| {
            ui.add_space(ui.available_width() * 0.05);
            widgets::label(ui, theme, "Pay Day Amount: ");
            let r = Entry::new(theme, &mut self.pay_day).width(60.0).enabled(ctx.enabled).id(ui.id().with("pay_day")).show(ui);
            if r.changed {
                out.merge(EditorOutput::save());
            }
            ui.add_space(20.0);
            widgets::label(ui, theme, self.exp_per_sec.clone());
        });
        ui.add_space(4.0);
        let card_bg = theme.section_bg();
        let n = self.cards.len();
        let cols = 3usize;
        let rows = (n + cols - 1) / cols;
        let card_w = ((ui.available_width() - 6.0 * (cols as f32 - 1.0)) / cols as f32).max(150.0);
        let mut reorder_idx: Option<usize> = None;
        for row in 0..rows {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for col in 0..cols {
                    let idx = row * cols + col;
                    if idx >= n {
                        break;
                    }
                    ui.allocate_ui_with_layout(Vec2::new(card_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.set_width(card_w);
                        ui.spacing_mut().item_spacing.y = 2.0;
                        let card = &self.cards[idx];
                        widgets::rounded_section(ui, card_bg, 6, egui::Margin { left: 8, right: 8, top: 6, bottom: 6 }, |ui| {
                            ui.set_width(card_w - 16.0);
                            let font = theme.body();
                            let bold = theme.body_bold();
                            ui.label(egui::RichText::new(&card.name).font(bold.clone()).color(theme.text));
                            if let Some(a) = &card.ability_line {
                                ui.label(egui::RichText::new(a).font(font.clone()).color(theme.text));
                            }
                            if let Some(i) = &card.item_line {
                                ui.label(egui::RichText::new(i).font(font.clone()).color(theme.text));
                            }
                            let label_w = 30.0;
                            let val_w = widgets::text_width(ui, "999", &font) + 12.0;
                            for (sn, sv, right) in &card.html_lines {
                                let color = if sn == "Spe:" { card.spe_color } else { theme.text };
                                let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), font.size * 1.3), Sense::hover());
                                ui.painter().text(Pos2::new(r.min.x, r.center().y), egui::Align2::LEFT_CENTER, sn, font.clone(), color);
                                ui.painter().text(Pos2::new(r.min.x + label_w + val_w, r.center().y), egui::Align2::RIGHT_CENTER, sv, font.clone(), color);
                                ui.painter().text(Pos2::new(r.min.x + label_w + val_w + 8.0, r.center().y), egui::Align2::LEFT_CENTER, right, font.clone(), theme.text);
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            widgets::label(ui, theme, "Mon Order");
                            if self.order_menus[idx].ui(ui, theme, ui.id().with(("order", idx)), Some(55.0), ctx.enabled) {
                                reorder_idx = Some(idx);
                            }
                            widgets::label(ui, theme, "Exp Split:");
                            if self.exp_splits[idx].ui(ui, theme, ui.id().with(("split", idx)), Some(55.0), ctx.enabled) {
                                out.merge(EditorOutput::save());
                            }
                        });
                    });
                }
            });
        }
        if let Some(idx) = reorder_idx {
            self.reorder_mons(idx);
            out.merge(EditorOutput::save());
            reload = true;
        }
        (out, reload)
    }
}

// ---------------------------------------------------------------------------
// Vitamin / rare candy
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct VitaminEditor {
    vitamin_types: OptionMenu,
    amount: String,
}

impl VitaminEditor {
    pub fn configure(&mut self, ctx: &EditorCtx) {
        let mut vals: Vec<String> = ctx.gen.get_valid_vitamins().iter().map(|s| s.to_string()).collect();
        vals.extend(ctx.gen.get_valid_ev_berries().iter().map(|s| s.to_string()));
        self.vitamin_types.new_values(vals, None);
    }

    pub fn load_event(&mut self, def: &EventDefinition) {
        if let Some(v) = &def.vitamin {
            self.vitamin_types.set(&v.vitamin);
            self.amount = v.amount.to_string();
        }
    }

    pub fn get_event(&self) -> Result<EventDefinition, String> {
        let amt: i64 = self.amount.trim().parse().map_err(|_| format!("invalid literal for int() with base 10: '{}'", self.amount))?;
        Ok(EventDefinition::with_vitamin(VitaminEventDefinition::new(self.vitamin_types.get(), amt)))
    }

    pub fn ui(&mut self, ui: &mut Ui, ctx: &EditorCtx) -> EditorOutput {
        let theme = ctx.theme;
        let mut out = EditorOutput::default();
        egui::Grid::new(ui.id().with("vitamin_grid")).spacing(Vec2::new(4.0, 4.0)).show(ui, |ui| {
            widgets::label(ui, theme, "Vitamin Type:");
            if self.vitamin_types.ui(ui, theme, ui.id().with("vit_type"), None, ctx.enabled) {
                out.merge(EditorOutput::save());
            }
            ui.end_row();
            widgets::label(ui, theme, "Num Vitamins:");
            let r = AmountEntry::new(theme, ui.id().with("vit_amt"), &mut self.amount).min(Some(1)).width(Some(5)).enabled(ctx.enabled).show(ui);
            if r.changed {
                if let Ok(v) = self.amount.trim().parse::<i64>() {
                    if v > 0 {
                        out.merge(EditorOutput::save());
                    }
                }
            }
            ui.end_row();
        });
        out
    }
}

#[derive(Default)]
pub struct RareCandyEditor {
    amount: String,
}

impl RareCandyEditor {
    pub fn load_event(&mut self, def: &EventDefinition) {
        if let Some(rc) = &def.rare_candy {
            self.amount = rc.amount.to_string();
        }
    }

    pub fn get_event(&self) -> Result<EventDefinition, String> {
        let amt: i64 = self.amount.trim().parse().map_err(|_| format!("invalid literal for int() with base 10: '{}'", self.amount))?;
        Ok(EventDefinition::with_rare_candy(amt))
    }

    pub fn ui(&mut self, ui: &mut Ui, ctx: &EditorCtx) -> EditorOutput {
        let theme = ctx.theme;
        let mut out = EditorOutput::default();
        egui::Grid::new(ui.id().with("candy_grid")).spacing(Vec2::new(4.0, 4.0)).show(ui, |ui| {
            widgets::label(ui, theme, "Num Rare Candies:");
            let r = AmountEntry::new(theme, ui.id().with("candy_amt"), &mut self.amount).min(Some(1)).width(Some(5)).enabled(ctx.enabled).show(ui);
            if r.changed {
                if let Ok(v) = self.amount.trim().parse::<i64>() {
                    if v > 0 {
                        out.merge(EditorOutput::save());
                    }
                }
            }
            ui.end_row();
        });
        out
    }
}

// ---------------------------------------------------------------------------
// Learn move editor
// ---------------------------------------------------------------------------

pub struct LearnMoveEditor {
    move_name_label: String,
    destination: OptionMenu,
    destination_enabled: bool,
    mv: Option<String>,
    level: LevelVal,
    mon: Option<String>,
    source: OptionMenu,
    source_enabled: bool,
    item_filter: String,
    item_selector: OptionMenu,
    move_filter: String,
    move_selector: OptionMenu,
    show_tm: bool,
    show_tutor: bool,
}

impl Default for LearnMoveEditor {
    fn default() -> Self {
        LearnMoveEditor {
            move_name_label: String::new(),
            destination: OptionMenu::default(),
            destination_enabled: true,
            mv: None,
            level: LevelVal::any(),
            mon: None,
            source: OptionMenu::default(),
            source_enabled: true,
            item_filter: String::new(),
            item_selector: OptionMenu::default(),
            move_filter: String::new(),
            move_selector: OptionMenu::default(),
            show_tm: false,
            show_tutor: false,
        }
    }
}

impl LearnMoveEditor {
    fn item_filter_callback(&mut self, ctx: &EditorCtx) {
        let mut new_vals = ctx.gen.item_db().get_filtered_names(consts::ITEM_TYPE_TM, consts::ITEM_TYPE_ALL_ITEMS, None);
        let f = self.item_filter.trim().to_lowercase();
        if !f.is_empty() {
            new_vals.retain(|x| x.to_lowercase().contains(&f));
        }
        if new_vals.is_empty() {
            new_vals = vec![consts::NO_ITEM.to_string()];
        }
        self.item_selector.new_values(new_vals, None);
    }

    fn move_filter_callback(&mut self, ctx: &EditorCtx) {
        let filter = self.move_filter.clone();
        self.move_selector.new_values(ctx.gen.move_db().get_filtered_names(Some(&filter), true), None);
    }

    /// `_move_source_callback` (without the save trigger)
    fn move_source_callback(&mut self, ctx: &EditorCtx) {
        let new_source = self.source.get().to_string();
        if new_source == consts::MOVE_SOURCE_LEVELUP {
            return;
        }
        if new_source == consts::MOVE_SOURCE_TM_HM {
            self.show_tm = true;
            self.show_tutor = false;
            self.item_filter_callback(ctx);
        } else {
            self.show_tm = false;
            self.show_tutor = true;
            self.move_filter_callback(ctx);
        }
    }

    /// `_move_selected_callback` (without the save trigger)
    fn move_selected_callback(&mut self, ctx: &EditorCtx) {
        if self.source.get() == consts::MOVE_SOURCE_TM_HM {
            self.mv = ctx.gen.item_db().get_item(self.item_selector.get()).and_then(|i| i.move_name.clone());
            self.move_name_label = format!("Move: {}", self.mv.as_deref().unwrap_or("None"));
        } else if self.source.get() == consts::MOVE_SOURCE_TUTOR {
            let m = self.move_selector.get().to_string();
            self.mv = if m == consts::DELETE_MOVE { None } else { Some(m) };
            self.move_name_label = format!("Move: {}", self.mv.as_deref().unwrap_or("None"));
        }
        if let Some(st) = ctx.cur_state {
            let (dest, allowed) = st.solo_pkmn.get_move_destination(self.mv.as_deref(), None, false);
            if !allowed {
                match dest {
                    None => self.destination.set(consts::MOVE_DONT_LEARN),
                    Some(d) => {
                        let text = slot_template(d + 1, "None");
                        self.destination.set(&text);
                    }
                }
                self.destination_enabled = false;
            } else {
                self.destination_enabled = true;
            }
        }
    }

    pub fn configure(&mut self, ctx: &EditorCtx) {
        let mut opts = vec![consts::MOVE_DONT_LEARN.to_string()];
        if let Some(st) = ctx.cur_state {
            for (idx, m) in st.solo_pkmn.move_list.iter().enumerate() {
                opts.push(slot_template(idx as i64 + 1, m.as_deref().unwrap_or("None")));
            }
        }
        self.destination.new_values(opts, None);
        self.show_tm = false;
        self.show_tutor = false;
        if ctx.event_type == consts::TASK_LEARN_MOVE_LEVELUP {
            self.source.new_values(vec![consts::MOVE_SOURCE_LEVELUP.to_string()], None);
            self.source_enabled = false;
        } else {
            self.source.new_values(vec![consts::MOVE_SOURCE_TUTOR.to_string(), consts::MOVE_SOURCE_TM_HM.to_string()], None);
            self.source_enabled = true;
        }
        self.item_filter_callback(ctx);
        self.move_selected_callback(ctx);
    }

    pub fn load_event(&mut self, ctx: &EditorCtx, def: &EventDefinition) {
        let Some(lm) = &def.learn_move else { return };
        if ctx.event_type == consts::TASK_LEARN_MOVE_LEVELUP {
            self.mv = lm.move_to_learn.clone();
            self.move_name_label = format!("Move: {}", self.mv.as_deref().unwrap_or("None"));
            self.level = lm.level.clone();
            self.mon = lm.mon.clone();
        } else if lm.source == consts::MOVE_SOURCE_TUTOR {
            self.source.set(consts::MOVE_SOURCE_TUTOR);
            self.move_filter.clear();
            self.move_filter_callback(ctx);
            let m = lm.move_to_learn.clone().unwrap_or_else(|| consts::DELETE_MOVE.to_string());
            self.move_selector.set(&m);
            self.level = LevelVal::any();
            self.mon = None;
        } else {
            self.source.set(consts::MOVE_SOURCE_TM_HM);
            self.item_filter.clear();
            self.item_filter_callback(ctx);
            self.item_selector.set(&lm.source);
            self.level = LevelVal::any();
            self.mon = None;
        }
        self.move_source_callback(ctx);
        self.move_selected_callback(ctx);
        match lm.destination {
            None => self.destination.set(consts::MOVE_DONT_LEARN),
            Some(d) => {
                if let Some(opt) = self.destination.options.get((d + 1) as usize).cloned() {
                    self.destination.set(&opt);
                }
            }
        }
    }

    pub fn get_event(&self, ctx: &EditorCtx) -> Result<EventDefinition, String> {
        let dest_text = self.destination.get();
        let dest = if dest_text == consts::MOVE_DONT_LEARN {
            None
        } else {
            let after = dest_text.split('#').nth(1).and_then(|s| s.chars().next()).and_then(|c| c.to_digit(10));
            match after {
                Some(d) => Some(d as i64 - 1),
                None => return Err(format!("Failed to extract slot destination from string '{}'", dest_text)),
            }
        };
        let source = if ctx.event_type == consts::TASK_LEARN_MOVE_LEVELUP {
            consts::MOVE_SOURCE_LEVELUP.to_string()
        } else if self.source.get() == consts::MOVE_SOURCE_TUTOR {
            consts::MOVE_SOURCE_TUTOR.to_string()
        } else {
            self.item_selector.get().to_string()
        };
        Ok(EventDefinition::with_learn_move(LearnMoveEventDefinition::new(
            self.mv.as_deref(),
            dest,
            &source,
            self.level.clone(),
            self.mon.as_deref(),
            false,
        )))
    }

    pub fn ui(&mut self, ui: &mut Ui, ctx: &EditorCtx) -> EditorOutput {
        let theme = ctx.theme;
        let mut out = EditorOutput::default();
        let val_width = 180.0;
        let mut source_changed = false;
        let mut item_filter_changed = false;
        let mut move_filter_changed = false;
        let mut move_changed = false;
        egui::Grid::new(ui.id().with("learn_grid")).spacing(Vec2::new(4.0, 4.0)).show(ui, |ui| {
            widgets::label(ui, theme, self.move_name_label.clone());
            ui.end_row();
            widgets::label(ui, theme, "Move Destination:");
            if self.destination.ui(ui, theme, ui.id().with("dest"), Some(val_width), ctx.enabled && self.destination_enabled) {
                out.merge(EditorOutput::save());
            }
            ui.end_row();
            widgets::label(ui, theme, "Move Source");
            if self.source.ui(ui, theme, ui.id().with("source"), Some(val_width), ctx.enabled && self.source_enabled) {
                source_changed = true;
            }
            ui.end_row();
            if self.show_tm {
                widgets::label(ui, theme, "Item Name Filter:");
                if Entry::new(theme, &mut self.item_filter).width(val_width).enabled(ctx.enabled).id(ui.id().with("item_filter")).show(ui).changed {
                    item_filter_changed = true;
                }
                ui.end_row();
                widgets::label(ui, theme, "Move:");
                if self.item_selector.ui(ui, theme, ui.id().with("item_sel"), Some(val_width), ctx.enabled) {
                    move_changed = true;
                }
                ui.end_row();
            }
            if self.show_tutor {
                widgets::label(ui, theme, "Move Filter:");
                if Entry::new(theme, &mut self.move_filter).width(val_width).enabled(ctx.enabled).id(ui.id().with("move_filter")).show(ui).changed {
                    move_filter_changed = true;
                }
                ui.end_row();
                widgets::label(ui, theme, "Move:");
                if self.move_selector.ui(ui, theme, ui.id().with("move_sel"), Some(val_width), ctx.enabled) {
                    move_changed = true;
                }
                ui.end_row();
            }
        });
        if source_changed {
            self.move_source_callback(ctx);
            out.merge(EditorOutput::save());
        }
        if item_filter_changed {
            self.item_filter_callback(ctx);
        }
        if move_filter_changed {
            self.move_filter_callback(ctx);
        }
        if move_changed {
            self.move_selected_callback(ctx);
            out.merge(EditorOutput::save());
        }
        out
    }
}

/// `const.MOVE_SLOT_TEMPLATE.format(idx, move)` -> "Move #N (Over X)"
pub fn slot_template(idx: i64, over: &str) -> String {
    format!("Move #{} (Over {})", idx, over)
}

// ---------------------------------------------------------------------------
// Wild pokemon editor
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct WildPkmnEditor {
    pkmn_types: OptionMenu,
    pkmn_filter: String,
    level: String,
    quantity: String,
    trainer_flag: bool,
}

impl WildPkmnEditor {
    pub fn configure(&mut self, ctx: &EditorCtx) {
        if self.pkmn_types.options.is_empty() {
            self.pkmn_types.new_values(ctx.gen.pkmn_db().get_all_names(None), None);
        }
        self.pkmn_filter.clear();
        self.level = "1".to_string();
        self.quantity = "1".to_string();
        self.trainer_flag = false;
    }

    pub fn load_event(&mut self, ctx: &EditorCtx, def: &EventDefinition) {
        let Some(w) = &def.wild_pkmn_info else { return };
        self.pkmn_filter.clear();
        self.pkmn_types.new_values(ctx.gen.pkmn_db().get_all_names(None), None);
        self.level = w.level.to_string();
        self.pkmn_types.set(&w.name);
        self.quantity = w.quantity.to_string();
        self.trainer_flag = w.trainer_pkmn;
    }

    fn valid(&self) -> bool {
        let level_ok = self.level.trim().parse::<i64>().map(|l| (2..=100).contains(&l)).unwrap_or(false);
        let qty_ok = self.quantity.trim().parse::<i64>().map(|q| q >= 1).unwrap_or(false);
        level_ok && qty_ok && !self.pkmn_types.get().trim().starts_with(consts::NO_POKEMON)
    }

    pub fn get_event(&self) -> Result<EventDefinition, String> {
        let level: i64 = self.level.trim().parse().map_err(|_| format!("invalid literal for int() with base 10: '{}'", self.level))?;
        let qty: i64 = self.quantity.trim().parse().map_err(|_| format!("invalid literal for int() with base 10: '{}'", self.quantity))?;
        Ok(EventDefinition::with_wild(WildPkmnEventDefinition::new(self.pkmn_types.get(), level, qty, self.trainer_flag)))
    }

    pub fn ui(&mut self, ui: &mut Ui, ctx: &EditorCtx) -> EditorOutput {
        let theme = ctx.theme;
        let mut out = EditorOutput::default();
        let mut filter_changed = false;
        let mut check_valid = false;
        egui::Grid::new(ui.id().with("wild_grid")).spacing(Vec2::new(4.0, 4.0)).show(ui, |ui| {
            widgets::label(ui, theme, "Wild Pokemon Type:");
            if self.pkmn_types.ui(ui, theme, ui.id().with("types"), Some(140.0), ctx.enabled) {
                out.merge(EditorOutput::save());
            }
            ui.end_row();
            widgets::label(ui, theme, "Wild Pokemon Type Filter:");
            if Entry::new(theme, &mut self.pkmn_filter).width(140.0).enabled(ctx.enabled).id(ui.id().with("filter")).show(ui).changed {
                filter_changed = true;
            }
            ui.end_row();
            widgets::label(ui, theme, "Wild Pokemon Level:");
            if AmountEntry::new(theme, ui.id().with("level"), &mut self.level).min(Some(2)).max(Some(100)).width(Some(5)).enabled(ctx.enabled).show(ui).changed {
                check_valid = true;
            }
            ui.end_row();
            widgets::label(ui, theme, "Num Pkmn:");
            if AmountEntry::new(theme, ui.id().with("qty"), &mut self.quantity).min(Some(1)).width(Some(5)).enabled(ctx.enabled).show(ui).changed {
                check_valid = true;
            }
            ui.end_row();
            if widgets::checkbox_label(ui, theme, &mut self.trainer_flag, "Is Trainer Pkmn?", true, ctx.enabled) {
                out.merge(EditorOutput::save());
            }
            ui.end_row();
        });
        if filter_changed {
            let f = self.pkmn_filter.trim().to_string();
            self.pkmn_types.new_values(ctx.gen.pkmn_db().get_filtered_names(Some(&f), None), None);
            check_valid = true;
        }
        if check_valid && self.valid() {
            out.merge(EditorOutput::save());
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Inventory editor (acquire / purchase / use / sell / hold)
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct InventoryEventEditor {
    pub event_type: String,
    allow_none_item: bool,
    item_type: OptionMenu,
    mart: OptionMenu,
    item_filter: String,
    item_selector: OptionMenu,
    amount: String,
    cost_label: String,
    consume_held: bool,
    hold_nothing: bool,
}

impl InventoryEventEditor {
    fn init_menus(&mut self, ctx: &EditorCtx) {
        if self.item_type.options.is_empty() {
            self.item_type.new_values(consts::ITEM_TYPES.iter().map(|s| s.to_string()).collect(), None);
        }
        let mut marts: Vec<String> = ctx.gen.item_db().mart_items.keys().cloned().collect();
        marts.sort();
        let mut opts = vec![consts::ITEM_TYPE_ALL_ITEMS.to_string()];
        opts.extend(marts);
        self.mart.new_values(opts, None);
        if self.item_selector.options.is_empty() {
            self.item_selector.new_values(ctx.gen.item_db().get_filtered_names(consts::ITEM_TYPE_ALL_ITEMS, consts::ITEM_TYPE_ALL_ITEMS, None), None);
        }
    }

    fn set_event_type(&mut self, event_type: &str) -> bool {
        match event_type {
            consts::TASK_GET_FREE_ITEM | consts::TASK_PURCHASE_ITEM | consts::TASK_USE_ITEM | consts::TASK_SELL_ITEM => {
                self.event_type = event_type.to_string();
                self.allow_none_item = false;
                true
            }
            consts::TASK_HOLD_ITEM => {
                self.event_type = event_type.to_string();
                self.allow_none_item = true;
                true
            }
            _ => false,
        }
    }

    /// `_item_filter_callback`
    fn item_filter_callback(&mut self, ctx: &EditorCtx) {
        let mut item_type = self.item_type.get().to_string();
        let mut backpack_filter = false;
        if item_type == consts::ITEM_TYPE_BACKPACK_ITEMS {
            item_type = consts::ITEM_TYPE_ALL_ITEMS.to_string();
            backpack_filter = true;
        }
        let mut new_vals = ctx.gen.item_db().get_filtered_names(&item_type, self.mart.get(), None);
        if backpack_filter {
            if let Some(st) = ctx.cur_state {
                let backpack: Vec<String> = st.inventory.cur_items.iter().map(|x| x.base_item.name.clone()).collect();
                new_vals.retain(|x| backpack.contains(x));
            } else {
                new_vals.clear();
            }
        }
        let f = self.item_filter.trim().to_lowercase();
        if !f.is_empty() {
            new_vals.retain(|x| x.to_lowercase().contains(&f));
        }
        if new_vals.is_empty() {
            new_vals = vec![if self.allow_none_item { "None".to_string() } else { consts::NO_ITEM.to_string() }];
        }
        self.item_selector.new_values(new_vals, None);
    }

    /// `_item_selector_callback`: refresh the cost label; true when a save should fire.
    fn item_selector_callback(&mut self, ctx: &EditorCtx) -> bool {
        let Ok(amt) = self.amount.trim().parse::<i64>() else { return false };
        let Some(item) = ctx.gen.item_db().get_item(self.item_selector.get()) else { return false };
        if self.event_type == consts::TASK_PURCHASE_ITEM {
            self.cost_label = format!("Total Cost: {}", item.purchase_price * amt);
        } else if self.event_type == consts::TASK_SELL_ITEM {
            self.cost_label = format!("Total Profit: {}", item.sell_price * amt);
        }
        true
    }

    pub fn configure(&mut self, ctx: &EditorCtx) {
        self.init_menus(ctx);
        self.item_filter.clear();
        self.mart.set(consts::ITEM_TYPE_ALL_ITEMS);
        self.item_type.set(consts::ITEM_TYPE_ALL_ITEMS);
        self.amount = "1".to_string();
        self.set_event_type(ctx.event_type);
    }

    pub fn load_event(&mut self, ctx: &EditorCtx, def: &EventDefinition) {
        self.init_menus(ctx);
        self.item_filter.clear();
        self.mart.set(consts::ITEM_TYPE_ALL_ITEMS);
        self.item_type.set(consts::ITEM_TYPE_ALL_ITEMS);
        self.set_event_type(def.get_event_type());
        if self.event_type != consts::TASK_HOLD_ITEM {
            if let Some(ie) = &def.item_event_def {
                self.item_selector.set(&ie.item_name);
                self.amount = ie.item_amount.to_string();
            }
        } else if let Some(h) = &def.hold_item {
            if let Some(n) = &h.item_name {
                self.item_selector.set(n);
            }
            self.consume_held = h.consumed;
            self.hold_nothing = h.item_name.is_none();
        }
        self.item_selector_callback(ctx);
    }

    pub fn get_event(&self) -> Result<EventDefinition, String> {
        let amt = || -> Result<i64, String> { self.amount.trim().parse().map_err(|_| format!("invalid literal for int() with base 10: '{}'", self.amount)) };
        let name = self.item_selector.get();
        match self.event_type.as_str() {
            consts::TASK_GET_FREE_ITEM => Ok(EventDefinition::with_item(InventoryEventDefinition::new(name, amt()?, true, false, None))),
            consts::TASK_PURCHASE_ITEM => Ok(EventDefinition::with_item(InventoryEventDefinition::new(name, amt()?, true, true, None))),
            consts::TASK_USE_ITEM => Ok(EventDefinition::with_item(InventoryEventDefinition::new(name, amt()?, false, false, None))),
            consts::TASK_SELL_ITEM => Ok(EventDefinition::with_item(InventoryEventDefinition::new(name, amt()?, false, true, None))),
            consts::TASK_HOLD_ITEM => {
                let held = if self.hold_nothing { None } else { Some(name) };
                Ok(EventDefinition::with_hold_item(HoldItemEventDefinition::new(held, self.consume_held)))
            }
            other => Err(format!("Cannot generate inventory event for event type: {}", other)),
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, ctx: &EditorCtx) -> EditorOutput {
        let theme = ctx.theme;
        let mut out = EditorOutput::default();
        let val_width = 180.0;
        let et = self.event_type.clone();
        let show_mart = et == consts::TASK_PURCHASE_ITEM;
        let show_amount = et != consts::TASK_HOLD_ITEM;
        let show_cost = et == consts::TASK_PURCHASE_ITEM || et == consts::TASK_SELL_ITEM;
        let show_hold = et == consts::TASK_HOLD_ITEM;
        let mut filter_changed = false;
        let mut selector_changed = false;
        egui::Grid::new(ui.id().with("inv_grid")).spacing(Vec2::new(4.0, 4.0)).show(ui, |ui| {
            widgets::label(ui, theme, "Item Type:");
            if self.item_type.ui(ui, theme, ui.id().with("type"), Some(val_width), ctx.enabled) {
                filter_changed = true;
            }
            ui.end_row();
            if show_mart {
                widgets::label(ui, theme, "Mart:");
                if self.mart.ui(ui, theme, ui.id().with("mart"), Some(val_width), ctx.enabled) {
                    filter_changed = true;
                }
                ui.end_row();
            }
            widgets::label(ui, theme, "Item Name Filter:");
            if Entry::new(theme, &mut self.item_filter).width(val_width).enabled(ctx.enabled).id(ui.id().with("filter")).show(ui).changed {
                filter_changed = true;
            }
            ui.end_row();
            widgets::label(ui, theme, "Item:");
            if self.item_selector.ui(ui, theme, ui.id().with("item"), Some(val_width), ctx.enabled) {
                selector_changed = true;
            }
            ui.end_row();
            if show_amount {
                widgets::label(ui, theme, "Num Items:");
                if AmountEntry::new(theme, ui.id().with("amt"), &mut self.amount).min(Some(1)).width(Some(5)).enabled(ctx.enabled).show(ui).changed {
                    selector_changed = true;
                }
                ui.end_row();
            }
            if show_cost {
                widgets::label(ui, theme, self.cost_label.clone());
                ui.end_row();
            }
            if show_hold {
                if widgets::checkbox_label(ui, theme, &mut self.consume_held, "Consume previously held item?", true, ctx.enabled) {
                    out.merge(EditorOutput::save());
                }
                ui.end_row();
                if widgets::checkbox_label(ui, theme, &mut self.hold_nothing, "Hold nothing?", true, ctx.enabled) {
                    out.merge(EditorOutput::save());
                }
                ui.end_row();
            }
        });
        if filter_changed {
            self.item_filter_callback(ctx);
        }
        if selector_changed && self.item_selector_callback(ctx) {
            out.merge(EditorOutput::save());
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Save / heal / blackout
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct LocationEditor {
    location: String,
}

impl LocationEditor {
    pub fn load_event(&mut self, def: &EventDefinition, event_type: &str) {
        let loc = match event_type {
            consts::TASK_SAVE => def.save.as_ref(),
            consts::TASK_HEAL => def.heal.as_ref(),
            _ => def.blackout.as_ref(),
        };
        self.location = loc.map(|l| l.location_str().to_string()).unwrap_or_default();
    }

    pub fn get_event(&self, event_type: &str) -> EventDefinition {
        let loc = LocationEventDefinition::new(&self.location);
        match event_type {
            consts::TASK_SAVE => EventDefinition { save: Some(loc), ..Default::default() },
            consts::TASK_HEAL => EventDefinition { heal: Some(loc), ..Default::default() },
            _ => EventDefinition { blackout: Some(loc), ..Default::default() },
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, ctx: &EditorCtx) -> EditorOutput {
        let theme = ctx.theme;
        let label = match ctx.event_type {
            consts::TASK_SAVE => "Save Location",
            consts::TASK_HEAL => "Heal Location",
            _ => "Black Out back to:",
        };
        let mut out = EditorOutput::default();
        egui::Grid::new(ui.id().with("loc_grid")).spacing(Vec2::new(4.0, 4.0)).show(ui, |ui| {
            widgets::label(ui, theme, label);
            if Entry::new(theme, &mut self.location).width(160.0).enabled(ctx.enabled).id(ui.id().with("loc")).show(ui).changed {
                out.merge(EditorOutput::delayed());
            }
            ui.end_row();
        });
        out
    }
}

// ---------------------------------------------------------------------------
// Evolution editor
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct EvolutionEditor {
    growth_rate: String,
    pkmn_types: OptionMenu,
    pkmn_filter: String,
    item_selector: OptionMenu,
}

impl EvolutionEditor {
    pub fn configure(&mut self, ctx: &EditorCtx) {
        let mut opts = vec![consts::NO_ITEM.to_string()];
        opts.extend(ctx.gen.item_db().get_filtered_names(consts::ITEM_TYPE_ALL_ITEMS, consts::ITEM_TYPE_ALL_ITEMS, Some("stone")));
        self.item_selector.new_values(opts, None);
    }

    pub fn load_event(&mut self, ctx: &EditorCtx, def: &EventDefinition) {
        let Some(ev) = &def.evolution else { return };
        let species = ev.evolved_species.clone().unwrap_or_default();
        self.growth_rate = ctx.gen.pkmn_db().get_pkmn(&species).map(|p| p.growth_rate.clone()).unwrap_or_default();
        self.pkmn_types.new_values(ctx.gen.pkmn_db().get_all_names(Some(&self.growth_rate)), Some(&species));
        self.pkmn_filter.clear();
        match &ev.by_stone {
            Some(s) if !s.is_empty() => self.item_selector.set(s),
            _ => self.item_selector.set(consts::NO_ITEM),
        }
    }

    pub fn get_event(&self) -> EventDefinition {
        let by_stone = if self.item_selector.get() == consts::NO_ITEM { None } else { Some(self.item_selector.get()) };
        EventDefinition { evolution: Some(EvolutionEventDefinition::new(self.pkmn_types.get(), by_stone)), ..Default::default() }
    }

    pub fn ui(&mut self, ui: &mut Ui, ctx: &EditorCtx) -> EditorOutput {
        let theme = ctx.theme;
        let mut out = EditorOutput::default();
        let mut filter_changed = false;
        egui::Grid::new(ui.id().with("evo_grid")).spacing(Vec2::new(4.0, 4.0)).show(ui, |ui| {
            widgets::label(ui, theme, "Pokemon Species:");
            if self.pkmn_types.ui(ui, theme, ui.id().with("species"), Some(140.0), ctx.enabled) {
                out.merge(EditorOutput::save());
            }
            ui.end_row();
            widgets::label(ui, theme, "Pokemon Species Filter:");
            if Entry::new(theme, &mut self.pkmn_filter).width(140.0).enabled(ctx.enabled).id(ui.id().with("filter")).show(ui).changed {
                filter_changed = true;
            }
            ui.end_row();
            widgets::label(ui, theme, "By Stone:");
            if self.item_selector.ui(ui, theme, ui.id().with("stone"), Some(140.0), ctx.enabled) {
                out.merge(EditorOutput::save());
            }
            ui.end_row();
        });
        if filter_changed {
            let f = self.pkmn_filter.trim().to_string();
            self.pkmn_types.new_values(ctx.gen.pkmn_db().get_filtered_names(Some(&f), Some(&self.growth_rate)), None);
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Bag reorder editor (gen 1)
// ---------------------------------------------------------------------------

/// The bag as it is right before the event, rearranged by dragging rows (or
/// with the arrow buttons). Saving stores the swaps that turn the pre-event
/// order into the displayed one, so a re-save also refreshes stale slots.
#[derive(Default)]
pub struct BagReorderEditor {
    /// item names in the pre-event order
    base: Vec<String>,
    /// `(name, count)` as displayed
    order: Vec<(String, i64)>,
    /// the swaps as loaded; handed back untouched until the user rearranges
    /// the list (every save, a notes edit included, goes through `get_event`)
    loaded: Vec<BagSwap>,
    modified: bool,
    /// the stored swaps did not apply exactly to `base` (the details panel
    /// shows the engine's warning text above the editor)
    stale: bool,
}

impl BagReorderEditor {
    fn rows_of(inv: &xpr_engine::Inventory) -> Vec<(String, i64)> {
        inv.cur_items.iter().map(|x| (x.base_item.name.clone(), x.num)).collect()
    }

    pub fn load_event(&mut self, ctx: &EditorCtx, def: &EventDefinition) {
        self.base = ctx.cur_state.map(|s| s.inventory.item_names()).unwrap_or_default();
        self.order = ctx.cur_state.map(|s| Self::rows_of(&s.inventory)).unwrap_or_default();
        self.loaded = def.bag_reorder.as_ref().map(|r| r.swaps.clone()).unwrap_or_default();
        self.modified = false;
        self.stale = false;
        if let (Some(r), Some(st)) = (&def.bag_reorder, ctx.cur_state) {
            let (inv, warnings) = st.inventory.swap_items(&r.swaps);
            self.order = Self::rows_of(&inv);
            self.stale = !warnings.is_empty();
        }
    }

    /// The loaded swaps unless the user rearranged the list, in which case
    /// the swaps that turn the pre-event bag into the displayed order (which
    /// is also how stale slots get refreshed).
    pub fn get_event(&self) -> Result<EventDefinition, String> {
        if !self.modified {
            return Ok(EventDefinition::with_bag_reorder(self.loaded.clone()));
        }
        let order: Vec<String> = self.order.iter().map(|x| x.0.clone()).collect();
        Ok(EventDefinition::with_bag_reorder(swaps_between(&self.base, &order)?))
    }

    /// Move the item at `from` so that it lands before the item currently at
    /// `to` (`to == len` appends).
    fn move_item(&mut self, from: usize, to: usize) -> bool {
        if from >= self.order.len() || to > self.order.len() {
            return false;
        }
        let dest = if to > from { to - 1 } else { to };
        if dest == from {
            return false;
        }
        let item = self.order.remove(from);
        self.order.insert(dest, item);
        true
    }

    pub fn ui(&mut self, ui: &mut Ui, ctx: &EditorCtx) -> EditorOutput {
        let theme = ctx.theme;
        let mut out = EditorOutput::default();
        if self.order.is_empty() {
            widgets::label(ui, theme, "The bag is empty before this event.");
            return out;
        }
        widgets::label(ui, theme, "Drag a row (or use the arrows) to rearrange the bag:");
        let font = theme.body();
        let n = self.order.len();
        let mut drop: Option<(usize, usize)> = None;
        let mut nudge: Option<(usize, bool)> = None;
        let mut row_rects: Vec<egui::Rect> = Vec::with_capacity(n);
        let zone = egui::Frame::new().inner_margin(egui::Margin::same(2));
        let (_, dropped_on_zone) = ui.dnd_drop_zone::<usize, ()>(zone, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 2.0);
            for idx in 0..n {
                let (name, count) = self.order[idx].clone();
                let row_id = ui.id().with(("bag_row", idx));
                let row = ui.horizontal(|ui| {
                    let handle = |ui: &mut Ui| {
                        let (r, _) = ui.allocate_exact_size(Vec2::new(16.0, font.size * 1.4), Sense::hover());
                        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "\u{2630}", font.clone(), theme.secondary);
                    };
                    if ctx.enabled {
                        ui.dnd_drag_source(row_id, idx, handle);
                    } else {
                        handle(ui);
                    }
                    let label = format!("{:>2}. {}x {}", idx + 1, count, name);
                    let (r, _) = ui.allocate_exact_size(Vec2::new(220.0_f32.max(widgets::text_width(ui, &label, &font) + 4.0), font.size * 1.4), Sense::hover());
                    ui.painter().text(Pos2::new(r.min.x + 2.0, r.center().y), egui::Align2::LEFT_CENTER, &label, font.clone(), theme.text);
                    if widgets::amount_button(ui, theme, "\u{25B2}", ctx.enabled && idx > 0).clicked() {
                        nudge = Some((idx, true));
                    }
                    if widgets::amount_button(ui, theme, "\u{25BC}", ctx.enabled && idx + 1 < n).clicked() {
                        nudge = Some((idx, false));
                    }
                });
                row_rects.push(row.response.rect);
                if !ctx.enabled {
                    continue;
                }
                let response = &row.response;
                if let (Some(pointer), Some(hovered)) = (ui.input(|i| i.pointer.interact_pos()), response.dnd_hover_payload::<usize>()) {
                    let rect = response.rect;
                    let stroke = egui::Stroke::new(2.0_f32, theme.accent);
                    let insert_at = if *hovered == idx {
                        idx
                    } else if pointer.y < rect.center().y {
                        ui.painter().hline(rect.x_range(), rect.top(), stroke);
                        idx
                    } else {
                        ui.painter().hline(rect.x_range(), rect.bottom(), stroke);
                        idx + 1
                    };
                    if let Some(from) = response.dnd_release_payload::<usize>() {
                        drop = Some((*from, insert_at));
                    }
                }
            }
        });
        if let Some(from) = dropped_on_zone {
            // released on the list but between rows (or on its padding): insert
            // before the first row below the pointer, else append
            let pointer_y = ui.input(|i| i.pointer.interact_pos()).map(|p| p.y);
            let insert_at = pointer_y.and_then(|y| row_rects.iter().position(|r| y < r.center().y)).unwrap_or(n);
            drop = Some((*from, insert_at));
        }
        let mut changed = false;
        if let Some((from, to)) = drop {
            changed |= self.move_item(from, to);
        }
        if let Some((idx, up)) = nudge {
            // the buttons are only enabled where the neighbour exists
            let other = if up { idx - 1 } else { idx + 1 };
            self.order.swap(idx, other);
            changed = true;
        }
        if changed {
            self.modified = true;
            self.stale = false;
            out.merge(EditorOutput::delayed());
        }
        if self.stale {
            widgets::label_colored(ui, theme, "The bag before this event no longer matches what was recorded; rearranging it here saves the order as shown.", theme.warning);
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Notes editor (the footer)
// ---------------------------------------------------------------------------

pub const NOTES_OPTIONS: [&str; 3] = [
    "Show notes in battle summary when space allows",
    "Show notes in battle summary at all times",
    "Never show notes in battle summary",
];

fn notes_mode_for_option(opt: &str) -> &'static str {
    match opt {
        "Show notes in battle summary at all times" => "always",
        "Never show notes in battle summary" => "never",
        _ => "when_space_allows",
    }
}

fn notes_option_for_mode(mode: &str) -> &'static str {
    match mode {
        "always" => NOTES_OPTIONS[1],
        "never" => NOTES_OPTIONS[2],
        _ => NOTES_OPTIONS[0],
    }
}

pub struct NotesEditor {
    pub text: String,
    collapsed: bool,
    force_collapsed: bool,
    visibility: OptionMenu,
    enabled: bool,
}

impl NotesEditor {
    pub fn new(cfg: &Config) -> NotesEditor {
        NotesEditor {
            text: String::new(),
            collapsed: cfg.get_notes_collapsed(),
            force_collapsed: false,
            visibility: OptionMenu::new(NOTES_OPTIONS.iter().map(|s| s.to_string()).collect(), Some(notes_option_for_mode(cfg.get_notes_visibility_mode()))),
            enabled: true,
        }
    }

    pub fn force_collapsed(&mut self, collapsed: bool) {
        self.force_collapsed = collapsed;
    }

    pub fn load_event(&mut self, cfg: &Config, def: Option<&EventDefinition>) {
        self.text = def.map(|d| d.notes.clone()).unwrap_or_default();
        self.visibility.set(notes_option_for_mode(cfg.get_notes_visibility_mode()));
    }

    pub fn get_event(&self) -> EventDefinition {
        EventDefinition::notes_only(self.text.trim())
    }

    pub fn set_enabled(&mut self, e: bool) {
        self.enabled = e;
    }

    /// Returns (output, visibility mode changed)
    pub fn ui(&mut self, ui: &mut Ui, theme: &Theme, cfg: &mut Config) -> (EditorOutput, bool) {
        let mut out = EditorOutput::default();
        let mut mode_changed = false;
        let expanded = !(self.collapsed || self.force_collapsed);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let tri = widgets::disclosure_triangle(ui, expanded, 14.0, Color32::from_rgb(0xcc, 0xcc, 0xcc));
            let lbl = ui.add(egui::Label::new(egui::RichText::new("Notes:").font(theme.body()).color(theme.text)).sense(Sense::click()));
            if tri.clicked() || lbl.clicked() {
                self.collapsed = !self.collapsed;
                self.force_collapsed = false;
                cfg.set_notes_collapsed(self.collapsed);
            }
            if expanded {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.visibility.ui(ui, theme, ui.id().with("notes_vis"), None, true) {
                        cfg.set_notes_visibility_mode(notes_mode_for_option(self.visibility.get()));
                        mode_changed = true;
                    }
                });
            }
        });
        if expanded {
            let r = widgets::text_area(ui, theme, &mut self.text, ui.id().with("notes_text"), 120.0, self.enabled);
            if r.changed() {
                out.merge(EditorOutput::delayed());
            }
        }
        (out, mode_changed)
    }
}

// ---------------------------------------------------------------------------
// The editor set (`EventEditorFactory`)
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct EventEditors {
    pub trainer: TrainerFightEditor,
    pub vitamin: VitaminEditor,
    pub rare_candy: RareCandyEditor,
    pub learn_move: LearnMoveEditor,
    pub wild: WildPkmnEditor,
    pub inventory: InventoryEventEditor,
    pub location: LocationEditor,
    pub evolution: EvolutionEditor,
    pub bag_reorder: BagReorderEditor,
}

impl EventEditors {
    /// `get_editor(...).configure(...)` then `load_event(def)`.
    pub fn load(&mut self, ctx: &EditorCtx, def: &EventDefinition) {
        match ctx.event_type {
            consts::TASK_TRAINER_BATTLE => self.trainer.load_event(ctx, def),
            consts::TASK_VITAMIN => {
                self.vitamin.configure(ctx);
                self.vitamin.load_event(def);
            }
            consts::TASK_RARE_CANDY => self.rare_candy.load_event(def),
            consts::TASK_LEARN_MOVE_LEVELUP | consts::TASK_LEARN_MOVE_TM => {
                self.learn_move.configure(ctx);
                self.learn_move.load_event(ctx, def);
            }
            consts::TASK_FIGHT_WILD_PKMN => {
                self.wild.configure(ctx);
                self.wild.load_event(ctx, def);
            }
            consts::TASK_GET_FREE_ITEM | consts::TASK_PURCHASE_ITEM | consts::TASK_USE_ITEM | consts::TASK_SELL_ITEM | consts::TASK_HOLD_ITEM => {
                self.inventory.configure(ctx);
                self.inventory.load_event(ctx, def);
            }
            consts::TASK_SAVE | consts::TASK_HEAL | consts::TASK_BLACKOUT => self.location.load_event(def, ctx.event_type),
            consts::TASK_EVOLUTION => {
                self.evolution.configure(ctx);
                self.evolution.load_event(ctx, def);
            }
            consts::TASK_REORDER_BAG => self.bag_reorder.load_event(ctx, def),
            _ => {}
        }
    }

    pub fn get_event(&self, ctx: &EditorCtx) -> Result<EventDefinition, String> {
        match ctx.event_type {
            consts::TASK_TRAINER_BATTLE => self.trainer.get_event(),
            consts::TASK_VITAMIN => self.vitamin.get_event(),
            consts::TASK_RARE_CANDY => self.rare_candy.get_event(),
            consts::TASK_LEARN_MOVE_LEVELUP | consts::TASK_LEARN_MOVE_TM => self.learn_move.get_event(ctx),
            consts::TASK_FIGHT_WILD_PKMN => self.wild.get_event(),
            consts::TASK_GET_FREE_ITEM | consts::TASK_PURCHASE_ITEM | consts::TASK_USE_ITEM | consts::TASK_SELL_ITEM | consts::TASK_HOLD_ITEM => self.inventory.get_event(),
            consts::TASK_SAVE | consts::TASK_HEAL | consts::TASK_BLACKOUT => Ok(self.location.get_event(ctx.event_type)),
            consts::TASK_EVOLUTION => Ok(self.evolution.get_event()),
            consts::TASK_REORDER_BAG => self.bag_reorder.get_event(),
            _ => Ok(EventDefinition::default()),
        }
    }

    /// Draw the editor for `ctx.event_type`. Returns (output, reload requested).
    pub fn ui(&mut self, ui: &mut Ui, ctx: &EditorCtx) -> (EditorOutput, bool) {
        egui::Frame::new().inner_margin(egui::Margin::same(4)).show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
            match ctx.event_type {
                consts::TASK_TRAINER_BATTLE => self.trainer.ui(ui, ctx),
                consts::TASK_VITAMIN => (self.vitamin.ui(ui, ctx), false),
                consts::TASK_RARE_CANDY => (self.rare_candy.ui(ui, ctx), false),
                consts::TASK_LEARN_MOVE_LEVELUP | consts::TASK_LEARN_MOVE_TM => (self.learn_move.ui(ui, ctx), false),
                consts::TASK_FIGHT_WILD_PKMN => (self.wild.ui(ui, ctx), false),
                consts::TASK_GET_FREE_ITEM | consts::TASK_PURCHASE_ITEM | consts::TASK_USE_ITEM | consts::TASK_SELL_ITEM | consts::TASK_HOLD_ITEM => {
                    (self.inventory.ui(ui, ctx), false)
                }
                consts::TASK_SAVE | consts::TASK_HEAL | consts::TASK_BLACKOUT => (self.location.ui(ui, ctx), false),
                consts::TASK_EVOLUTION => (self.evolution.ui(ui, ctx), false),
                consts::TASK_REORDER_BAG => (self.bag_reorder.ui(ui, ctx), false),
                _ => (EditorOutput::default(), false),
            }
        })
        .inner
    }
}
