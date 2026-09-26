//! Port of `gui_qt/components/quick_add_popover.py` and the quick-add pages
//! (`quick_trainer_add.py`, `quick_item_add.py`, `quick_wild_pkmn.py`,
//! `quick_misc.py`): the two-phase popover opened from the route list with
//! Q/W/E/R/T.

use egui::{Color32, CornerRadius, Pos2, Sense, Stroke, Ui, Vec2};

use xpr_core::consts;
use xpr_data::model::Trainer;
use xpr_engine::{
    EventDefinition, HoldItemEventDefinition, InventoryEventDefinition, LearnMoveEventDefinition, LevelVal, ObjKind,
    RareCandyEventDefinition, TrainerEventDefinition, VitaminEventDefinition, WildPkmnEventDefinition,
};
use xpr_ui_kit::modal::layer_behind_modal;
use xpr_ui_kit::theme::{self, Theme};
use xpr_ui_kit::widgets::{self, AmountEntry, Entry, StyledButton};

use crate::assets::Assets;
use crate::controller::MainController;
use crate::editors::OptionMenu;

const CATEGORIES: [(&str, &str); 5] = [
    ("trainer.png", "Trainer (Q)"),
    ("item.png", "Item (W)"),
    ("moves.png", "Move (E)"),
    ("wild.png", "Wild Pkmn (R)"),
    ("misc.png", "Misc (T)"),
];

/// `QuickTrainerAdd`
pub struct QuickTrainerAdd {
    loc: OptionMenu,
    class: OptionMenu,
    trainers: OptionMenu,
    show_rematches: bool,
    multi_setup_mode: bool,
    saved_partner: Option<String>,
    ignore_preview: bool,
}

impl QuickTrainerAdd {
    fn new() -> QuickTrainerAdd {
        QuickTrainerAdd {
            loc: OptionMenu::new(vec![consts::ALL_TRAINERS.to_string()], None),
            class: OptionMenu::new(vec![consts::ALL_TRAINERS.to_string()], None),
            trainers: OptionMenu::new(vec![consts::NO_TRAINERS.to_string()], None),
            show_rematches: false,
            multi_setup_mode: false,
            saved_partner: None,
            ignore_preview: false,
        }
    }

    fn custom_trainer_name(ctrl: &MainController, t: &Trainer) -> String {
        let gen = ctrl.gen();
        let eps = gen.map(|g| xpr_data::exp::experience_per_second(g.get_trainer_timing_info(), &t.pkmn)).unwrap_or_default();
        format!("({}) {}", eps, t.name)
    }

    fn get_trainer_name(&self) -> String {
        let s = self.trainers.get();
        match s.find(')') {
            Some(i) => s[i + 1..].trim().to_string(),
            None => s.trim().to_string(),
        }
    }

    fn update_pkmn_version(&mut self, ctrl: &MainController) {
        if let Some(g) = ctrl.gen() {
            let mut locs = g.trainer_db().get_all_locations();
            locs.sort();
            self.loc.new_values(std::iter::once(consts::ALL_TRAINERS.to_string()).chain(locs).collect(), None);
            let mut classes = g.trainer_db().get_all_classes();
            classes.sort();
            self.class.new_values(std::iter::once(consts::ALL_TRAINERS.to_string()).chain(classes).collect(), None);
        }
        self.set_multi_setup_mode(false, ctrl);
        self.trainer_filter_callback(ctrl, false);
    }

    fn trainer_filter_callback(&mut self, ctrl: &MainController, ignore_preview: bool) {
        self.ignore_preview = ignore_preview;
        let mut defeated = ctrl.get_defeated_trainers();
        if let Some(p) = &self.saved_partner {
            defeated.push(p.clone());
        }
        let mut valid = ctrl
            .gen()
            .map(|g| {
                let gctrl = ctrl;
                g.trainer_db().get_valid_trainers_with(Some(self.class.get()), Some(self.loc.get()), &defeated, self.show_rematches, self.multi_setup_mode, |t| QuickTrainerAdd::custom_trainer_name(gctrl, t))
            })
            .unwrap_or_default();
        if valid.is_empty() {
            valid.push(consts::NO_TRAINERS.to_string());
        }
        self.trainers.new_values(valid, None);
        self.ignore_preview = false;
    }

    fn set_multi_setup_mode(&mut self, new_val: bool, ctrl: &MainController) {
        if new_val {
            let selected = self.get_trainer_name();
            if selected == consts::NO_TRAINERS {
                return;
            }
            let Some(g) = ctrl.gen() else { return };
            if !g.trainer_db().can_trainer_multi_battle(&selected) {
                return;
            }
            self.saved_partner = Some(selected.clone());
            self.multi_setup_mode = true;
            if let Some(t) = g.trainer_db().get_trainer(&selected) {
                self.loc.set(&t.location);
            }
        } else {
            self.multi_setup_mode = false;
            self.saved_partner = None;
        }
    }

    /// Returns a preview trainer request.
    fn ui(&mut self, ui: &mut Ui, theme: &Theme, ctrl: &mut MainController) -> Option<String> {
        let mut preview = None;
        let gen = ctrl.gen();
        let can_insert = ctrl.can_insert_after_current_selection();
        let selected = self.get_trainer_name();
        let (add_ok, area_ok, multi_ok) = if !can_insert {
            (false, false, false)
        } else if selected == consts::NO_TRAINERS {
            (false, false, self.multi_setup_mode)
        } else {
            let area = self.loc.get() != consts::ALL_TRAINERS && !self.multi_setup_mode;
            let multi = gen.as_ref().map(|g| g.get_generation() >= 3 && g.trainer_db().can_trainer_multi_battle(&selected)).unwrap_or(false);
            (true, area, multi)
        };
        let title = if self.multi_setup_mode { "Select Partner" } else { "Trainers" };
        widgets::group_box(ui, theme, title, |ui| {
            let mut filter_changed = false;
            let mut trainer_changed = false;
            egui::Grid::new("quick_trainer_grid").spacing(Vec2::new(5.0, 1.0)).show(ui, |ui| {
                widgets::label(ui, theme, "Location:");
                if self.loc.ui(ui, theme, ui.id().with("loc"), Some(200.0), !self.multi_setup_mode) {
                    filter_changed = true;
                }
                ui.end_row();
                widgets::label(ui, theme, "Trainer Class:");
                if self.class.ui(ui, theme, ui.id().with("cls"), Some(200.0), true) {
                    filter_changed = true;
                }
                ui.end_row();
                widgets::label(ui, theme, "Trainer:");
                if self.trainers.ui(ui, theme, ui.id().with("trainer"), Some(200.0), true) {
                    trainer_changed = true;
                }
                ui.end_row();
                if widgets::checkbox_label(ui, theme, &mut self.show_rematches, "Show Rematches:", true, true) {
                    filter_changed = true;
                }
                ui.end_row();
                if self.multi_setup_mode {
                    widgets::label(ui, theme, "Multi Partner: ");
                    widgets::label(ui, theme, self.saved_partner.clone().unwrap_or_default());
                    ui.end_row();
                }
            });
            if filter_changed {
                self.trainer_filter_callback(ctrl, false);
                trainer_changed = true;
            }
            if trainer_changed {
                let sel = self.get_trainer_name();
                if sel != consts::NO_TRAINERS && !self.ignore_preview {
                    preview = Some(sel);
                }
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                let add_label = if self.multi_setup_mode { "Add Multi" } else { "Add Trainer" };
                if widgets::button_enabled(ui, theme, add_label, add_ok).clicked() && add_ok {
                    self.add_trainer(ctrl);
                }
                if widgets::button_enabled(ui, theme, "Add Area", area_ok).clicked() && area_ok {
                    if let Some(after) = ctrl.get_single_selected_event_id(true) {
                        let loc = self.loc.get().to_string();
                        ctrl.add_area(&loc, self.show_rematches, Some(after));
                    }
                }
                if gen.as_ref().map(|g| g.get_generation() >= 3).unwrap_or(false) {
                    let multi_label = if self.multi_setup_mode { "Cancel Multi" } else { "Multi Partner" };
                    if widgets::button_enabled(ui, theme, multi_label, multi_ok).clicked() && multi_ok {
                        let new_mode = !self.multi_setup_mode;
                        self.set_multi_setup_mode(new_mode, ctrl);
                        self.trainer_filter_callback(ctrl, !self.multi_setup_mode);
                    }
                }
            });
        });
        preview
    }

    /// `add_trainer`
    fn add_trainer(&mut self, ctrl: &mut MainController) {
        let Some(gen) = ctrl.gen() else { return };
        if self.multi_setup_mode {
            let Some(partner) = self.saved_partner.clone() else { return };
            let mut td = TrainerEventDefinition::new(&partner);
            td.set_second_trainer_name(&self.get_trainer_name());
            let mut temp = EventDefinition::with_trainer(td);
            let n = temp.pokemon_list(&gen).map(|l| l.len()).unwrap_or(0);
            if let Some(t) = temp.trainer_def.as_mut() {
                t.exp_split = vec![2; n];
            }
            let after = ctrl.get_single_selected_event_id(true);
            ctrl.new_event(temp, after, None, None, true);
            self.set_multi_setup_mode(false, ctrl);
            self.trainer_filter_callback(ctrl, true);
        } else {
            let name = self.get_trainer_name();
            let mut temp = EventDefinition::with_trainer(TrainerEventDefinition::new(&name));
            let Some(trainer) = gen.trainer_db().get_trainer(&name).cloned() else { return };
            if trainer.double_battle {
                let n = temp.pokemon_list(&gen).map(|l| l.len()).unwrap_or(0);
                if let Some(t) = temp.trainer_def.as_mut() {
                    t.exp_split = vec![2; n];
                }
            }
            let selected_id = ctrl.get_single_selected_event_id(true);
            let is_folder = selected_id.map(|id| ctrl.router.obj_kind(id) == Some(ObjKind::Folder)).unwrap_or(false);
            if is_folder {
                let base = if trainer.location.trim().is_empty() { "New Folder".to_string() } else { trainer.location.clone() };
                let all: std::collections::HashSet<String> = ctrl.get_all_folder_names().into_iter().collect();
                let mut folder_name = base.clone();
                let mut count = 1;
                while all.contains(&folder_name) {
                    count += 1;
                    folder_name = format!("{} Trip:{}", base, count);
                }
                ctrl.finalize_new_folder(&folder_name, None, selected_id);
                ctrl.new_event(temp, None, None, Some(&folder_name), true);
            } else {
                ctrl.new_event(temp, selected_id, None, None, true);
            }
        }
    }
}

/// `QuickItemAdd`
pub struct QuickItemAdd {
    filter: String,
    item_type: OptionMenu,
    items: OptionMenu,
    mart: OptionMenu,
    amount: String,
    purchase_cost: String,
    sell_cost: String,
    /// The move a single-target PP item (Ether, PP Up, ...) is used on.
    pp_move: OptionMenu,
}

impl QuickItemAdd {
    fn new() -> QuickItemAdd {
        QuickItemAdd {
            filter: String::new(),
            item_type: OptionMenu::new(consts::ITEM_TYPES.iter().map(|s| s.to_string()).collect(), None),
            items: OptionMenu::new(vec![consts::NO_ITEM.to_string()], None),
            mart: OptionMenu::new(vec![consts::ITEM_TYPE_ALL_ITEMS.to_string()], None),
            pp_move: OptionMenu::default(),
            amount: "1".to_string(),
            purchase_cost: String::new(),
            sell_cost: String::new(),
        }
    }

    fn update_pkmn_version(&mut self, ctrl: &MainController) {
        if let Some(g) = ctrl.gen() {
            self.items.new_values(g.item_db().get_filtered_names(consts::ITEM_TYPE_ALL_ITEMS, consts::ITEM_TYPE_ALL_ITEMS, None), None);
            let mut marts: Vec<String> = g.item_db().mart_items.keys().cloned().collect();
            marts.sort();
            self.mart.new_values(std::iter::once(consts::ITEM_TYPE_ALL_ITEMS.to_string()).chain(marts).collect(), None);
        }
        self.item_selector_callback(ctrl);
    }

    fn item_filter_callback(&mut self, ctrl: &MainController) {
        let Some(g) = ctrl.gen() else { return };
        let mut item_type = self.item_type.get().to_string();
        let mut backpack = false;
        if item_type == consts::ITEM_TYPE_BACKPACK_ITEMS {
            item_type = consts::ITEM_TYPE_ALL_ITEMS.to_string();
            backpack = true;
        }
        let mut vals = g.item_db().get_filtered_names(&item_type, self.mart.get(), None);
        if backpack {
            match ctrl.get_active_state() {
                None => vals.clear(),
                Some(st) => {
                    let bag: Vec<String> = st.inventory.cur_items.iter().map(|x| x.base_item.name.clone()).collect();
                    vals.retain(|x| bag.contains(x));
                }
            }
        }
        let f = self.filter.trim().to_lowercase();
        if !f.is_empty() {
            vals.retain(|x| x.to_lowercase().contains(&f));
        }
        if vals.is_empty() {
            vals.push(consts::NO_ITEM.to_string());
        }
        self.items.new_values(vals, None);
    }

    fn item_selector_callback(&mut self, ctrl: &MainController) {
        let amt = self.amount.trim().parse::<i64>();
        let item = ctrl.gen().and_then(|g| g.item_db().get_item(self.items.get()).cloned());
        match (amt, item) {
            (Ok(a), Some(i)) => {
                self.purchase_cost = (i.purchase_price * a).to_string();
                self.sell_cost = (i.sell_price * a).to_string();
            }
            _ => {
                self.purchase_cost.clear();
                self.sell_cost.clear();
            }
        }
    }

    fn ui(&mut self, ui: &mut Ui, theme: &Theme, ctrl: &mut MainController) {
        let gen = ctrl.gen();
        let can_insert = ctrl.can_insert_after_current_selection();
        let item = gen.as_ref().and_then(|g| g.item_db().get_item(self.items.get()).cloned());
        let pp_effect = match (&item, &gen) {
            (Some(i), Some(g)) => g.pp_item_effect(&i.name),
            _ => None,
        };
        let (get_ok, use_ok, hold_ok, tm_ok) = match (&item, &gen) {
            (Some(i), Some(g)) if can_insert => {
                let is_use = g.get_valid_vitamins().contains(&i.name.as_str()) || g.get_valid_ev_berries().contains(&i.name.as_str()) || i.name == consts::RARE_CANDY || pp_effect.is_some();
                (true, is_use, g.get_generation() != 1, i.move_name.is_some())
            }
            _ => (false, false, false, false),
        };
        // the moves a single-target PP item can go on, at the insertion point
        let pp_needs_target = pp_effect.map(|e| e.needs_target()).unwrap_or(false);
        if pp_needs_target {
            let moves: Vec<String> = ctrl.get_active_state().map(|st| st.solo_pkmn.move_list.iter().flatten().filter(|m| !m.is_empty()).cloned().collect()).unwrap_or_default();
            self.pp_move.new_values(moves, None);
        }
        // the bag can only be rearranged in gen 1; the new event starts empty
        // and is arranged in the details panel
        let reorder_ok = can_insert && gen.as_ref().map(|g| g.supports_bag_reorder()).unwrap_or(false);
        widgets::group_box(ui, theme, "Items", |ui| {
            let mut filter_changed = false;
            let mut sel_changed = false;
            egui::Grid::new("quick_item_grid").spacing(Vec2::new(2.0, 1.0)).show(ui, |ui| {
                widgets::label(ui, theme, "Search:");
                if Entry::new(theme, &mut self.filter).width(120.0).id(ui.id().with("filter")).show(ui).changed {
                    filter_changed = true;
                }
                widgets::label(ui, theme, "Item Type:");
                if self.item_type.ui(ui, theme, ui.id().with("type"), Some(140.0), true) {
                    filter_changed = true;
                }
                ui.end_row();
                widgets::label(ui, theme, "Item:");
                if self.items.ui(ui, theme, ui.id().with("item"), Some(120.0), true) {
                    sel_changed = true;
                }
                widgets::label(ui, theme, "Mart:");
                if self.mart.ui(ui, theme, ui.id().with("mart"), Some(140.0), true) {
                    filter_changed = true;
                }
                ui.end_row();
                widgets::label(ui, theme, "Quantity:");
                if AmountEntry::new(theme, ui.id().with("amount"), &mut self.amount).min(Some(1)).show(ui).changed {
                    sel_changed = true;
                }
                ui.end_row();
                if pp_needs_target {
                    widgets::label(ui, theme, "On move:");
                    self.pp_move.ui(ui, theme, ui.id().with("pp_move"), Some(120.0), !self.pp_move.options.is_empty());
                    ui.end_row();
                }
                widgets::label(ui, theme, "Purchase:");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| widgets::label(ui, theme, self.purchase_cost.clone()));
                widgets::label(ui, theme, "Sell Price:");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| widgets::label(ui, theme, self.sell_cost.clone()));
                ui.end_row();
            });
            if filter_changed {
                self.item_filter_callback(ctrl);
                sel_changed = true;
            }
            if sel_changed {
                self.item_selector_callback(ctrl);
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let name = self.items.get().to_string();
                let amt = self.amount.trim().parse::<i64>().unwrap_or(1);
                let after = ctrl.get_single_selected_event_id(true);
                if StyledButton::new(theme, "Get").fixed_width(60.0).enabled(get_ok).show(ui).clicked() && get_ok {
                    ctrl.new_event(EventDefinition::with_item(InventoryEventDefinition::new(&name, amt, true, false, None)), after, None, None, true);
                }
                if StyledButton::new(theme, "Drop").fixed_width(60.0).enabled(get_ok).show(ui).clicked() && get_ok {
                    let mut ie = InventoryEventDefinition::new(&name, amt, false, false, None);
                    // a dropped PP item restores nothing
                    ie.no_effect = pp_effect.is_some();
                    ctrl.new_event(EventDefinition::with_item(ie), after, None, None, true);
                }
                ui.add_space(10.0);
                if StyledButton::new(theme, "Use").fixed_width(60.0).enabled(use_ok).show(ui).clicked() && use_ok {
                    if let Some(g) = &gen {
                        if g.get_valid_vitamins().contains(&name.as_str()) || g.get_valid_ev_berries().contains(&name.as_str()) {
                            ctrl.new_event(EventDefinition::with_vitamin(VitaminEventDefinition::new(&name, amt)), after, None, None, true);
                        } else if name == consts::RARE_CANDY {
                            ctrl.new_event(EventDefinition::with_rare_candy(amt), after, None, None, true);
                        } else if pp_effect.is_some() {
                            let target = Some(self.pp_move.get()).filter(|m| pp_needs_target && !m.is_empty());
                            ctrl.new_event(EventDefinition::with_item(InventoryEventDefinition::use_on(&name, amt, target)), after, None, None, true);
                        }
                    }
                }
                if StyledButton::new(theme, "Hold").fixed_width(60.0).enabled(hold_ok).show(ui).clicked() && hold_ok {
                    ctrl.new_event(EventDefinition::with_hold_item(HoldItemEventDefinition::new(Some(&name), false)), after, None, None, true);
                }
                if StyledButton::new(theme, "TM/HM").fixed_width(60.0).enabled(tm_ok).show(ui).clicked() && tm_ok {
                    if let (Some(i), Some(st)) = (&item, ctrl.get_active_state()) {
                        if let Some(mv) = &i.move_name {
                            let dest = st.solo_pkmn.get_move_destination(Some(mv), None, false).0;
                            ctrl.new_event(EventDefinition::with_learn_move(LearnMoveEventDefinition::new(Some(mv), dest, &name, LevelVal::any(), None, false)), after, None, None, true);
                        }
                    }
                }
                ui.add_space(10.0);
                if StyledButton::new(theme, "Buy").fixed_width(60.0).enabled(get_ok).show(ui).clicked() && get_ok {
                    ctrl.new_event(EventDefinition::with_item(InventoryEventDefinition::new(&name, amt, true, true, None)), after, None, None, true);
                }
                if StyledButton::new(theme, "Sell").fixed_width(60.0).enabled(get_ok).show(ui).clicked() && get_ok {
                    ctrl.new_event(EventDefinition::with_item(InventoryEventDefinition::new(&name, amt, false, true, None)), after, None, None, true);
                }
                ui.add_space(10.0);
                let reorder = StyledButton::new(theme, "Reorder").fixed_width(70.0).enabled(reorder_ok).show(ui);
                if reorder.clicked() && reorder_ok {
                    ctrl.new_event(EventDefinition::with_bag_reorder(Vec::new()), after, None, None, true);
                }
                reorder.on_hover_text("Insert a bag reorder (gen 1); arrange the bag in the event's details");
            });
        });
    }
}

/// `QuickWildPkmn`
pub struct QuickWildPkmn {
    filter: String,
    pkmn: OptionMenu,
    level: String,
    quantity: String,
}

impl QuickWildPkmn {
    fn new() -> QuickWildPkmn {
        QuickWildPkmn { filter: String::new(), pkmn: OptionMenu::new(vec![consts::NO_POKEMON.to_string()], None), level: "5".to_string(), quantity: "1".to_string() }
    }

    fn update_pkmn_version(&mut self, ctrl: &MainController) {
        if let Some(g) = ctrl.gen() {
            self.pkmn.new_values(g.pkmn_db().get_all_names(None), None);
        }
    }

    fn valid(&self, ctrl: &MainController) -> bool {
        if !ctrl.can_insert_after_current_selection() {
            return false;
        }
        if self.pkmn.get().trim().starts_with(consts::NO_POKEMON) {
            return false;
        }
        let level_ok = self.level.trim().parse::<i64>().map(|l| (2..=100).contains(&l)).unwrap_or(false);
        let qty_ok = self.quantity.trim().parse::<i64>().map(|q| q >= 1).unwrap_or(false);
        level_ok && qty_ok
    }

    fn ui(&mut self, ui: &mut Ui, theme: &Theme, ctrl: &mut MainController) {
        widgets::group_box(ui, theme, "Wild Pkmn", |ui| {
            let mut filter_changed = false;
            egui::Grid::new("quick_wild_grid").spacing(Vec2::new(5.0, 1.0)).show(ui, |ui| {
                widgets::label(ui, theme, "Filter:");
                if Entry::new(theme, &mut self.filter).width(160.0).id(ui.id().with("filter")).show(ui).changed {
                    filter_changed = true;
                }
                ui.end_row();
                widgets::label(ui, theme, "Wild Pkmn:");
                self.pkmn.ui(ui, theme, ui.id().with("pkmn"), Some(160.0), true);
                ui.end_row();
                widgets::label(ui, theme, "Pkmn Level:");
                AmountEntry::new(theme, ui.id().with("level"), &mut self.level).min(Some(2)).max(Some(100)).show(ui);
                ui.end_row();
                widgets::label(ui, theme, "Quantity:");
                AmountEntry::new(theme, ui.id().with("qty"), &mut self.quantity).min(Some(1)).show(ui);
                ui.end_row();
            });
            if filter_changed {
                if let Some(g) = ctrl.gen() {
                    let f = self.filter.trim().to_string();
                    self.pkmn.new_values(g.pkmn_db().get_filtered_names(Some(&f), None), None);
                }
            }
            let ok = self.valid(ctrl);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                let after = ctrl.get_single_selected_event_id(true);
                let level = self.level.trim().parse::<i64>().unwrap_or(5);
                let qty = self.quantity.trim().parse::<i64>().unwrap_or(1);
                if widgets::button_enabled(ui, theme, "Add Wild Pkmn", ok).clicked() && ok {
                    ctrl.new_event(EventDefinition::with_wild(WildPkmnEventDefinition::new(self.pkmn.get(), level, qty, false)), after, None, None, true);
                }
                if widgets::button_enabled(ui, theme, "Add Trainer Pkmn", ok).clicked() && ok {
                    ctrl.new_event(EventDefinition::with_wild(WildPkmnEventDefinition::new(self.pkmn.get(), level, qty, true)), after, None, None, true);
                }
            });
        });
    }
}

/// `QuickMiscEvents`
fn quick_misc_ui(ui: &mut Ui, theme: &Theme, ctrl: &mut MainController) {
    let ok = ctrl.can_insert_after_current_selection() && ctrl.gen().is_some();
    widgets::group_box(ui, theme, "Misc", |ui| {
        let after = ctrl.get_single_selected_event_id(true);
        egui::Grid::new("quick_misc_grid").spacing(Vec2::new(5.0, 2.0)).show(ui, |ui| {
            if widgets::button_enabled(ui, theme, "Tutor Move", ok).clicked() && ok {
                if let Some(st) = ctrl.get_active_state() {
                    let dest = st.solo_pkmn.get_move_destination(None, None, false).0;
                    ctrl.new_event(EventDefinition::with_learn_move(LearnMoveEventDefinition::new(None, dest, consts::MOVE_SOURCE_TUTOR, LevelVal::any(), None, false)), after, None, None, true);
                }
            }
            if widgets::button_enabled(ui, theme, "Evolve", ok).clicked() && ok {
                if let Some(st) = ctrl.get_active_state() {
                    let name = st.solo_pkmn.name.clone();
                    ctrl.new_event(EventDefinition::with_evolution(&name), after, None, None, true);
                }
            }
            ui.end_row();
            if widgets::button_enabled(ui, theme, "Add Save", ok).clicked() && ok {
                ctrl.new_event(EventDefinition::with_save(""), after, None, None, true);
            }
            if widgets::button_enabled(ui, theme, "Add 2 Candies", ok).clicked() && ok {
                ctrl.new_event(EventDefinition::with_rare_candy(2), after, None, None, true);
            }
            ui.end_row();
            if widgets::button_enabled(ui, theme, "Add Heal", ok).clicked() && ok {
                ctrl.new_event(EventDefinition::with_heal(""), after, None, None, true);
            }
            if widgets::button_enabled(ui, theme, "Add 3 Candies", ok).clicked() && ok {
                ctrl.new_event(EventDefinition::with_rare_candy(3), after, None, None, true);
            }
            ui.end_row();
            if widgets::button_enabled(ui, theme, "Add Black Out", ok).clicked() && ok {
                ctrl.new_event(EventDefinition::with_blackout(), after, None, None, true);
            }
            if widgets::button_enabled(ui, theme, "Add 5 Candies", ok).clicked() && ok {
                ctrl.new_event(EventDefinition::with_rare_candy(5), after, None, None, true);
            }
            ui.end_row();
            if widgets::button_enabled(ui, theme, "Add Notes", ok).clicked() && ok {
                ctrl.new_event(EventDefinition::default(), after, None, None, true);
            }
            if widgets::button_enabled(ui, theme, "Add 10 Candies", ok).clicked() && ok {
                ctrl.new_event(EventDefinition::with_rare_candy(10), after, None, None, true);
            }
            ui.end_row();
        });
    });
    let _ = RareCandyEventDefinition::new(0);
}

/// The popover.
pub struct QuickAddPopover {
    pub open: bool,
    anchor: Pos2,
    category: Option<usize>,
    trainer: QuickTrainerAdd,
    item: QuickItemAdd,
    wild: QuickWildPkmn,
    opened_frame: bool,
}

impl QuickAddPopover {
    pub fn new() -> QuickAddPopover {
        QuickAddPopover { open: false, anchor: Pos2::ZERO, category: None, trainer: QuickTrainerAdd::new(), item: QuickItemAdd::new(), wild: QuickWildPkmn::new(), opened_frame: false }
    }

    /// `show_above(global_pos, category_idx)`
    pub fn show_above(&mut self, ctrl: &MainController, anchor: Pos2, category: Option<usize>) {
        self.anchor = anchor;
        self.category = category;
        self.open = true;
        self.opened_frame = true;
        self.trainer.update_pkmn_version(ctrl);
        self.item.update_pkmn_version(ctrl);
        self.wild.update_pkmn_version(ctrl);
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// Called on route changes (the popover auto-closes).
    pub fn on_route_change(&mut self) {
        if self.open {
            self.open = false;
        }
    }

    pub fn ui(&mut self, ctx: &egui::Context, theme: &Theme, ctrl: &mut MainController, assets: &mut Assets) -> Option<String> {
        if !self.open {
            return None;
        }
        let mut preview = None;
        // a dialog above the page gets the keys and clicks (the popover is
        // part of the page, and it reads both raw)
        let blocked = layer_behind_modal(ctx, egui::LayerId::background());
        // keys: category shortcuts, Escape/Space close
        let mut close = false;
        ctx.input_mut(|i| {
            if blocked {
                return;
            }
            for (idx, key) in [egui::Key::Q, egui::Key::W, egui::Key::E, egui::Key::R, egui::Key::T].iter().enumerate() {
                if i.consume_key(egui::Modifiers::NONE, *key) {
                    self.category = Some(idx);
                }
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::Escape) || i.consume_key(egui::Modifiers::NONE, egui::Key::Space) {
                close = true;
            }
        });
        let screen = ctx.content_rect();
        let area = egui::Area::new(egui::Id::new("quick_add_popover")).order(egui::Order::Foreground).fixed_pos(self.anchor).pivot(egui::Align2::CENTER_BOTTOM).constrain_to(screen);
        let inner = area.show(ctx, |ui| {
            egui::Frame::new()
                .fill(theme.bg)
                .stroke(Stroke::new(1.0_f32, theme::rgba(255, 255, 255, 0.15)))
                .corner_radius(CornerRadius::same(4))
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                    ui.horizontal(|ui| {
                        for (idx, (icon, tip)) in CATEGORIES.iter().enumerate() {
                            let checked = self.category == Some(idx);
                            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::click());
                            let fill = if checked {
                                theme.accent
                            } else if resp.hovered() {
                                theme.hover_bg
                            } else {
                                theme.bg_lighter
                            };
                            ui.painter().rect(rect, CornerRadius::same(3), fill, Stroke::new(1.0_f32, if checked || resp.hovered() { theme.accent } else { theme.border }), egui::StrokeKind::Inside);
                            if let Some(tex) = assets.category_icon(ui.ctx(), icon) {
                                let r = egui::Rect::from_center_size(rect.center(), Vec2::splat(24.0));
                                ui.painter().image(tex.id(), r, egui::Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)), Color32::WHITE);
                            }
                            if resp.on_hover_text(*tip).clicked() {
                                self.category = Some(idx);
                            }
                        }
                    });
                    if let Some(cat) = self.category {
                        widgets::hline(ui, theme);
                        match cat {
                            0 => preview = self.trainer.ui(ui, theme, ctrl),
                            1 => self.item.ui(ui, theme, ctrl),
                            2 => {
                                widgets::group_box(ui, theme, "Moves", |ui| {
                                    if widgets::button(ui, theme, "Add TM/HM Move").clicked() {
                                        if let (Some(g), Some(st)) = (ctrl.gen(), ctrl.get_active_state()) {
                                            let tms = g.item_db().get_filtered_names(consts::ITEM_TYPE_TM, consts::ITEM_TYPE_ALL_ITEMS, None);
                                            if let Some(first) = tms.first() {
                                                if first != consts::NO_ITEM {
                                                    let mv = g.item_db().get_item(first).and_then(|i| i.move_name.clone());
                                                    let dest = st.solo_pkmn.get_move_destination(mv.as_deref(), None, false).0;
                                                    let after = ctrl.get_single_selected_event_id(true);
                                                    ctrl.new_event(EventDefinition::with_learn_move(LearnMoveEventDefinition::new(mv.as_deref(), dest, first, LevelVal::any(), None, false)), after, None, None, true);
                                                }
                                            }
                                        }
                                    }
                                    if widgets::button(ui, theme, "Add Tutor Move").clicked() {
                                        if let Some(st) = ctrl.get_active_state() {
                                            let dest = st.solo_pkmn.get_move_destination(None, None, false).0;
                                            let after = ctrl.get_single_selected_event_id(true);
                                            ctrl.new_event(EventDefinition::with_learn_move(LearnMoveEventDefinition::new(None, dest, consts::MOVE_SOURCE_TUTOR, LevelVal::any(), None, false)), after, None, None, true);
                                        }
                                    }
                                });
                            }
                            3 => self.wild.ui(ui, theme, ctrl),
                            _ => quick_misc_ui(ui, theme, ctrl),
                        }
                    }
                });
        });
        // click outside closes (Qt.Popup)
        if !self.opened_frame && !blocked && ctx.input(|i| i.pointer.any_pressed()) {
            if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
                if !inner.response.rect.contains(p) {
                    close = true;
                }
            }
        }
        self.opened_frame = false;
        if close {
            self.open = false;
        }
        preview
    }
}
