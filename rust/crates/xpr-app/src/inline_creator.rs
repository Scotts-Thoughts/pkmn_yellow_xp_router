//! Port of `gui_qt/components/inline_event_creator.py`: the compact
//! create/edit strip overlaid on the route list. Create mode inserts via the
//! controller; edit mode hands the built definition back to the list.

use egui::{Color32, CornerRadius, Id, Stroke, Ui};

use xpr_core::consts;
use xpr_data::GenData;
use xpr_engine::{
    BagSwap, EvOverrideEventDefinition, EventDefinition, HoldItemEventDefinition, InventoryEventDefinition, LearnMoveEventDefinition, LevelVal,
    NodeId, RouteState, TrainerEventDefinition, VitaminEventDefinition, WildPkmnEventDefinition,
};
use xpr_ui_kit::modal::behind_modal;
use xpr_ui_kit::theme::Theme;
use xpr_ui_kit::widgets::{self, AmountEntry, Entry, SearchableDropdown, StyledButton};

use crate::controller::MainController;
use crate::editors::slot_template;

pub const INLINE_ROW_HEIGHT: f32 = 34.0;

/// (display name, internal key)
pub const EVENT_TYPES: [(&str, &str); 18] = [
    ("Fight Trainer", "trainer"),
    ("Get Item", "get_item"),
    ("Buy Item", "buy_item"),
    ("Sell Item", "sell_item"),
    ("Use/Drop Item", "use_item"),
    ("Hold Item", "hold_item"),
    ("Reorder Bag", "reorder_bag"),
    ("Wild Pkmn", "wild_pkmn"),
    ("Rare Candy", "rare_candy"),
    ("Vitamin", "vitamin"),
    ("TM/HM Move", "tm_move"),
    ("Tutor Move", "tutor_move"),
    ("Save", "save"),
    ("Heal", "heal"),
    ("Blackout", "blackout"),
    ("Evolve", "evolve"),
    ("EV Override", "ev_override"),
    ("Notes", "notes"),
];

/// event-type constant -> inline key (`_EVENT_TYPE_TO_KEY`)
pub fn key_for_event_type(event_type: &str) -> Option<&'static str> {
    Some(match event_type {
        consts::TASK_TRAINER_BATTLE => "trainer",
        consts::TASK_GET_FREE_ITEM => "get_item",
        consts::TASK_PURCHASE_ITEM => "buy_item",
        consts::TASK_SELL_ITEM => "sell_item",
        consts::TASK_USE_ITEM => "use_item",
        consts::TASK_HOLD_ITEM => "hold_item",
        consts::TASK_REORDER_BAG => "reorder_bag",
        consts::TASK_FIGHT_WILD_PKMN => "wild_pkmn",
        consts::TASK_RARE_CANDY => "rare_candy",
        consts::TASK_VITAMIN => "vitamin",
        consts::TASK_LEARN_MOVE_TM => "tm_move",
        consts::TASK_SAVE => "save",
        consts::TASK_HEAL => "heal",
        consts::TASK_BLACKOUT => "blackout",
        consts::TASK_EVOLUTION => "evolve",
        consts::TASK_EV_OVERRIDE => "ev_override",
        consts::TASK_NOTES_ONLY => "notes",
        _ => return None,
    })
}

fn display_for_key(key: &str) -> Option<&'static str> {
    EVENT_TYPES.iter().find(|(_, k)| *k == key).map(|(d, _)| *d)
}

/// The per-type configuration widgets' state.
#[derive(Clone, Debug, Default)]
struct ConfigState {
    loc: String,
    cls: String,
    trainer: String,
    trainers: Vec<String>,
    item: String,
    items: Vec<String>,
    qty: String,
    pkmn: String,
    level: String,
    vitamin: String,
    tm: String,
    tms: Vec<String>,
    mv: String,
    dest: String,
    dest_options: Vec<String>,
    note: String,
    /// the swaps of a reorder event being edited (the strip has no field for
    /// them; the details panel is where the order is arranged)
    swaps: Vec<BagSwap>,
    /// a use of a PP item that acts on one move: the move menu shows
    pp_needs_target: bool,
    pp_move: String,
    pp_moves: Vec<String>,
    /// an edited use was marked tossed (the details panel sets it; the
    /// strip keeps it)
    pp_tossed: bool,
}

/// Focus targets in layout order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    Type,
    Loc,
    Cls,
    Trainer,
    Item,
    Qty,
    /// the move a single-target PP item (Ether, PP Up, ...) is used on
    PpMove,
    Pkmn,
    Level,
    Vitamin,
    Tm,
    Move,
    Dest,
    Note,
}

/// What the strip reported this frame.
#[derive(Clone, Debug, Default)]
pub struct InlineOutcome {
    /// create mode: the event was inserted; edit mode: `result` holds the definition
    pub created: bool,
    pub discarded: bool,
    pub result: Option<EventDefinition>,
}

pub struct InlineEventCreator {
    pub insert_after_id: Option<NodeId>,
    pub dest_folder_name: Option<String>,
    pub editing_group_id: Option<NodeId>,
    type_text: String,
    type_key: Option<&'static str>,
    cfg: ConfigState,
    focus_request: Option<Field>,
    id: Id,
    locations: Vec<String>,
    classes: Vec<String>,
    vitamins: Vec<String>,
    all_pkmn: Vec<String>,
    all_moves: Vec<String>,
    all_items: Vec<String>,
}

impl InlineEventCreator {
    pub fn new(id: Id, insert_after_id: Option<NodeId>, dest_folder_name: Option<String>, editing: Option<(NodeId, &EventDefinition)>, ctrl: &MainController) -> InlineEventCreator {
        let gen = ctrl.gen();
        let mut c = InlineEventCreator {
            insert_after_id,
            dest_folder_name,
            editing_group_id: editing.map(|(id, _)| id),
            type_text: String::new(),
            type_key: None,
            cfg: ConfigState::default(),
            focus_request: Some(Field::Type),
            id,
            locations: Vec::new(),
            classes: Vec::new(),
            vitamins: Vec::new(),
            all_pkmn: Vec::new(),
            all_moves: Vec::new(),
            all_items: Vec::new(),
        };
        if let Some(g) = &gen {
            let mut locs = g.trainer_db().get_all_locations();
            locs.sort();
            c.locations = std::iter::once(consts::ALL_TRAINERS.to_string()).chain(locs).collect();
            let mut classes = g.trainer_db().get_all_classes();
            classes.sort();
            c.classes = std::iter::once(consts::ALL_TRAINERS.to_string()).chain(classes).collect();
            c.vitamins = g.get_valid_vitamins().iter().chain(g.get_valid_ev_berries().iter()).map(|s| s.to_string()).collect();
            c.all_pkmn = g.pkmn_db().get_all_names(None);
            c.all_moves = g.move_db().get_filtered_names(Some(""), true);
            c.all_items = g.item_db().get_filtered_names(consts::ITEM_TYPE_ALL_ITEMS, consts::ITEM_TYPE_ALL_ITEMS, None);
            if c.all_items.is_empty() {
                c.all_items = vec![consts::NO_ITEM.to_string()];
            }
        }
        if let Some((_, def)) = editing {
            c.init_from_event(def, ctrl);
        }
        c
    }

    fn state_at_insertion_point(&self, ctrl: &MainController) -> Option<std::sync::Arc<RouteState>> {
        match self.insert_after_id {
            None => ctrl.get_init_state(),
            Some(id) => ctrl.router.final_state_of(id).cloned().or_else(|| ctrl.get_init_state()),
        }
    }

    /// `_on_type_changed` + `_build_config`
    fn set_type(&mut self, key: &'static str, ctrl: &MainController) {
        if self.type_key == Some(key) {
            return;
        }
        self.type_key = Some(key);
        self.type_text = display_for_key(key).unwrap_or("").to_string();
        self.cfg = ConfigState::default();
        match key {
            "trainer" => {
                self.cfg.loc = consts::ALL_TRAINERS.to_string();
                self.cfg.cls = consts::ALL_TRAINERS.to_string();
                self.refresh_trainers(ctrl);
            }
            "get_item" | "buy_item" | "sell_item" | "use_item" | "hold_item" => {
                self.cfg.items = self.all_items.clone();
                self.cfg.item = self.cfg.items.first().cloned().unwrap_or_default();
                self.cfg.qty = "1".to_string();
                self.refresh_pp(ctrl, None);
            }
            "wild_pkmn" => {
                self.cfg.pkmn = self.all_pkmn.first().cloned().unwrap_or_default();
                self.cfg.level = "5".to_string();
                self.cfg.qty = "1".to_string();
            }
            "rare_candy" => {
                self.cfg.qty = "1".to_string();
            }
            "vitamin" => {
                self.cfg.vitamin = self.vitamins.first().cloned().unwrap_or_default();
                self.cfg.qty = "1".to_string();
            }
            "tm_move" => {
                let mut tms = ctrl.gen().map(|g| g.item_db().get_filtered_names(consts::ITEM_TYPE_TM, consts::ITEM_TYPE_ALL_ITEMS, None)).unwrap_or_default();
                if tms.is_empty() || tms[0] == consts::NO_ITEM {
                    tms = vec![consts::NO_ITEM.to_string()];
                }
                self.cfg.tms = tms;
                self.cfg.tm = self.cfg.tms.first().cloned().unwrap_or_default();
                self.refresh_dest(ctrl);
            }
            "tutor_move" => {
                self.cfg.mv = self.all_moves.first().cloned().unwrap_or_default();
                self.refresh_dest(ctrl);
            }
            _ => {}
        }
    }

    fn refresh_trainers(&mut self, ctrl: &MainController) {
        let defeated = ctrl.get_defeated_trainers();
        let mut trainers = ctrl
            .gen()
            .map(|g| g.trainer_db().get_valid_trainers(Some(&self.cfg.cls), Some(&self.cfg.loc), &defeated, false, false))
            .unwrap_or_default();
        if trainers.is_empty() {
            trainers = vec![consts::NO_TRAINERS.to_string()];
        }
        self.cfg.trainer = trainers[0].clone();
        self.cfg.trainers = trainers;
    }

    /// The PP-item move menu of a use: shown when the item acts on one move,
    /// listing the moves known at the insertion point; `keep` is the move to
    /// select.
    fn refresh_pp(&mut self, ctrl: &MainController, keep: Option<String>) {
        let needs = self.type_key == Some("use_item")
            && ctrl.gen().and_then(|g| g.pp_item_effect(&self.cfg.item)).map(|e| e.needs_target()).unwrap_or(false);
        self.cfg.pp_needs_target = needs;
        let mut moves = vec![crate::editors::PP_TARGET_NONE.to_string()];
        if let Some(st) = self.state_at_insertion_point(ctrl) {
            moves.extend(st.solo_pkmn.move_list.iter().flatten().filter(|m| !m.is_empty()).cloned());
        }
        self.cfg.pp_move = match keep {
            Some(k) if moves.contains(&k) => k,
            _ => moves[0].clone(),
        };
        self.cfg.pp_moves = moves;
    }

    /// `_refresh_dest` (TM and tutor variants)
    fn refresh_dest(&mut self, ctrl: &MainController) {
        let st = self.state_at_insertion_point(ctrl);
        let move_list: Vec<Option<String>> = st.as_ref().map(|s| s.solo_pkmn.move_list.clone()).unwrap_or_default();
        let mut options = vec![consts::MOVE_DONT_LEARN.to_string()];
        for (idx, m) in move_list.iter().enumerate() {
            options.push(slot_template(idx as i64 + 1, m.as_deref().unwrap_or("None")));
        }
        self.cfg.dest_options = options.clone();
        self.cfg.dest = options[0].clone();
        let move_name: Option<String> = match self.type_key {
            Some("tm_move") => {
                if self.cfg.tm != consts::NO_ITEM {
                    ctrl.gen().and_then(|g| g.item_db().get_item(&self.cfg.tm).and_then(|i| i.move_name.clone()))
                } else {
                    None
                }
            }
            Some("tutor_move") => {
                if !self.cfg.mv.is_empty() && self.cfg.mv != consts::DELETE_MOVE {
                    Some(self.cfg.mv.clone())
                } else {
                    None
                }
            }
            _ => None,
        };
        if let (Some(m), Some(s)) = (move_name, st) {
            let (dest, _) = s.solo_pkmn.get_move_destination(Some(&m), None, false);
            if let Some(d) = dest {
                if let Some(opt) = options.get((d + 1) as usize) {
                    self.cfg.dest = opt.clone();
                }
            }
        }
    }

    /// `_init_from_event` + `_populate_config`
    fn init_from_event(&mut self, def: &EventDefinition, ctrl: &MainController) {
        let mut key = key_for_event_type(def.get_event_type());
        if key == Some("tm_move") {
            if let Some(lm) = &def.learn_move {
                if lm.source == consts::MOVE_SOURCE_TUTOR {
                    key = Some("tutor_move");
                }
            }
        }
        let Some(key) = key else { return };
        self.set_type(key, ctrl);
        match key {
            "trainer" => {
                if let Some(td) = &def.trainer_def {
                    if let Some(t) = ctrl.gen().and_then(|g| g.trainer_db().get_trainer(&td.trainer_name).cloned()) {
                        if !t.location.is_empty() && self.locations.contains(&t.location) {
                            self.cfg.loc = t.location.clone();
                        }
                    }
                    self.refresh_trainers(ctrl);
                    if self.cfg.trainers.contains(&td.trainer_name) {
                        self.cfg.trainer = td.trainer_name.clone();
                    }
                }
            }
            "get_item" | "buy_item" | "sell_item" | "use_item" => {
                if let Some(ie) = &def.item_event_def {
                    if self.cfg.items.contains(&ie.item_name) {
                        self.cfg.item = ie.item_name.clone();
                    }
                    self.cfg.qty = ie.item_amount.to_string();
                    self.cfg.pp_tossed = ie.no_effect;
                    self.refresh_pp(ctrl, ie.target_move.clone());
                }
            }
            "hold_item" => {
                if let Some(h) = &def.hold_item {
                    if let Some(n) = &h.item_name {
                        if self.cfg.items.contains(n) {
                            self.cfg.item = n.clone();
                        }
                    }
                }
            }
            "wild_pkmn" => {
                if let Some(w) = &def.wild_pkmn_info {
                    if self.all_pkmn.contains(&w.name) {
                        self.cfg.pkmn = w.name.clone();
                    }
                    self.cfg.level = w.level.to_string();
                    self.cfg.qty = w.quantity.to_string();
                }
            }
            "rare_candy" => {
                if let Some(rc) = &def.rare_candy {
                    self.cfg.qty = rc.amount.to_string();
                }
            }
            "vitamin" => {
                if let Some(v) = &def.vitamin {
                    if self.vitamins.contains(&v.vitamin) {
                        self.cfg.vitamin = v.vitamin.clone();
                    }
                    self.cfg.qty = v.amount.to_string();
                }
            }
            "tm_move" => {
                if let Some(lm) = &def.learn_move {
                    if self.cfg.tms.contains(&lm.source) {
                        self.cfg.tm = lm.source.clone();
                    }
                    self.refresh_dest(ctrl);
                    if let Some(d) = lm.destination {
                        if let Some(opt) = self.cfg.dest_options.get((d + 1) as usize) {
                            self.cfg.dest = opt.clone();
                        }
                    }
                }
            }
            "tutor_move" => {
                if let Some(lm) = &def.learn_move {
                    if let Some(m) = &lm.move_to_learn {
                        if self.all_moves.contains(m) {
                            self.cfg.mv = m.clone();
                        }
                    }
                    self.refresh_dest(ctrl);
                    if let Some(d) = lm.destination {
                        if let Some(opt) = self.cfg.dest_options.get((d + 1) as usize) {
                            self.cfg.dest = opt.clone();
                        }
                    }
                }
            }
            "notes" => self.cfg.note = def.notes.clone(),
            "reorder_bag" => {
                if let Some(r) = &def.bag_reorder {
                    self.cfg.swaps = r.swaps.clone();
                }
            }
            _ => {}
        }
    }

    /// `focus_primary_field`
    pub fn focus_primary_field(&mut self) {
        self.focus_request = Some(match self.type_key {
            Some("trainer") => Field::Trainer,
            Some("get_item") | Some("buy_item") | Some("sell_item") | Some("use_item") | Some("hold_item") => Field::Item,
            Some("wild_pkmn") => Field::Pkmn,
            Some("vitamin") => Field::Vitamin,
            Some("rare_candy") => Field::Qty,
            Some("tm_move") => Field::Tm,
            Some("tutor_move") => Field::Move,
            Some("notes") => Field::Note,
            _ => Field::Type,
        });
    }

    fn fields(&self) -> Vec<Field> {
        let mut f = vec![Field::Type];
        match self.type_key {
            Some("trainer") => f.extend([Field::Loc, Field::Cls, Field::Trainer]),
            Some("use_item") if self.cfg.pp_needs_target => f.extend([Field::Item, Field::Qty, Field::PpMove]),
            Some("get_item") | Some("buy_item") | Some("sell_item") | Some("use_item") => f.extend([Field::Item, Field::Qty]),
            Some("hold_item") => f.push(Field::Item),
            Some("wild_pkmn") => f.extend([Field::Pkmn, Field::Level, Field::Qty]),
            Some("rare_candy") => f.push(Field::Qty),
            Some("vitamin") => f.extend([Field::Vitamin, Field::Qty]),
            Some("tm_move") => f.extend([Field::Tm, Field::Dest]),
            Some("tutor_move") => f.extend([Field::Move, Field::Dest]),
            Some("notes") => f.push(Field::Note),
            _ => {}
        }
        f
    }

    /// `_advance_focus`: Enter on the last field creates; Tab wraps.
    fn advance(&mut self, from: Field, forward: bool, trigger_create_at_end: bool) -> bool {
        let fields = self.fields();
        let Some(cur) = fields.iter().position(|f| *f == from) else { return false };
        if forward && trigger_create_at_end && cur == fields.len() - 1 {
            return true;
        }
        if fields.len() < 2 {
            return false;
        }
        let next = if forward { (cur + 1) % fields.len() } else { (cur + fields.len() - 1) % fields.len() };
        self.focus_request = Some(fields[next]);
        false
    }

    fn can_create(&self) -> bool {
        match self.type_key {
            Some("trainer") => self.cfg.trainer != consts::NO_TRAINERS,
            Some("get_item") | Some("buy_item") | Some("sell_item") | Some("use_item") | Some("hold_item") => self.cfg.item != consts::NO_ITEM,
            Some("tm_move") => self.cfg.tm != consts::NO_ITEM,
            Some("tutor_move") => !self.cfg.mv.is_empty(),
            Some(_) => true,
            None => false,
        }
    }

    /// The `_event_builder` of the current type.
    fn build(&self, ctrl: &MainController) -> Result<Option<EventDefinition>, String> {
        let gen: Option<std::sync::Arc<GenData>> = ctrl.gen();
        let qty = || -> Result<i64, String> { self.cfg.qty.trim().parse::<i64>().map_err(|_| format!("invalid literal for int() with base 10: '{}'", self.cfg.qty)) };
        Ok(match self.type_key {
            Some("trainer") => {
                if self.cfg.trainer == consts::NO_TRAINERS {
                    None
                } else {
                    Some(EventDefinition::with_trainer(TrainerEventDefinition::new(&self.cfg.trainer)))
                }
            }
            Some("get_item") => {
                if self.cfg.item == consts::NO_ITEM {
                    None
                } else {
                    Some(EventDefinition::with_item(InventoryEventDefinition::new(&self.cfg.item, qty()?, true, false, None)))
                }
            }
            Some("buy_item") => {
                if self.cfg.item == consts::NO_ITEM {
                    None
                } else {
                    Some(EventDefinition::with_item(InventoryEventDefinition::new(&self.cfg.item, qty()?, true, true, None)))
                }
            }
            Some("sell_item") => {
                if self.cfg.item == consts::NO_ITEM {
                    None
                } else {
                    Some(EventDefinition::with_item(InventoryEventDefinition::new(&self.cfg.item, qty()?, false, true, None)))
                }
            }
            Some("use_item") => {
                if self.cfg.item == consts::NO_ITEM {
                    None
                } else {
                    let n = self.cfg.item.as_str();
                    let q = qty()?;
                    let is_vit = gen.as_ref().map(|g| g.get_valid_vitamins().contains(&n) || g.get_valid_ev_berries().contains(&n)).unwrap_or(false);
                    if is_vit {
                        Some(EventDefinition::with_vitamin(VitaminEventDefinition::new(n, q)))
                    } else if n == consts::RARE_CANDY {
                        Some(EventDefinition::with_rare_candy(q))
                    } else {
                        let target = Some(self.cfg.pp_move.as_str()).filter(|t| self.cfg.pp_needs_target && *t != crate::editors::PP_TARGET_NONE);
                        let mut ie = InventoryEventDefinition::use_on(n, q, target);
                        ie.no_effect = self.cfg.pp_tossed;
                        Some(EventDefinition::with_item(ie))
                    }
                }
            }
            Some("hold_item") => {
                if self.cfg.item == consts::NO_ITEM {
                    None
                } else {
                    Some(EventDefinition::with_hold_item(HoldItemEventDefinition::new(Some(&self.cfg.item), false)))
                }
            }
            Some("wild_pkmn") => {
                let level: i64 = self.cfg.level.trim().parse().map_err(|_| format!("invalid literal for int() with base 10: '{}'", self.cfg.level))?;
                Some(EventDefinition::with_wild(WildPkmnEventDefinition::new(&self.cfg.pkmn, level, qty()?, false)))
            }
            Some("rare_candy") => Some(EventDefinition::with_rare_candy(qty()?)),
            Some("vitamin") => Some(EventDefinition::with_vitamin(VitaminEventDefinition::new(&self.cfg.vitamin, qty()?))),
            Some("tm_move") => {
                if self.cfg.tm == consts::NO_ITEM {
                    None
                } else {
                    let Some(item) = gen.as_ref().and_then(|g| g.item_db().get_item(&self.cfg.tm).cloned()) else { return Ok(None) };
                    let dest = parse_dest(&self.cfg.dest);
                    Some(EventDefinition::with_learn_move(LearnMoveEventDefinition::new(item.move_name.as_deref(), dest, &self.cfg.tm, LevelVal::any(), None, false)))
                }
            }
            Some("tutor_move") => {
                if self.cfg.mv.is_empty() {
                    None
                } else {
                    let mv = if self.cfg.mv == consts::DELETE_MOVE { None } else { Some(self.cfg.mv.as_str()) };
                    let dest = parse_dest(&self.cfg.dest);
                    Some(EventDefinition::with_learn_move(LearnMoveEventDefinition::new(mv, dest, consts::MOVE_SOURCE_TUTOR, LevelVal::any(), None, false)))
                }
            }
            Some("notes") => Some(EventDefinition::notes_only(&self.cfg.note)),
            // a new reorder starts empty: the order is arranged in the details panel
            Some("reorder_bag") => Some(EventDefinition::with_bag_reorder(self.cfg.swaps.clone())),
            Some("save") => Some(EventDefinition::with_save("")),
            Some("heal") => Some(EventDefinition::with_heal("")),
            Some("blackout") => Some(EventDefinition::with_blackout()),
            Some("evolve") => {
                let species = self.state_at_insertion_point(ctrl).map(|s| s.solo_pkmn.name.clone()).unwrap_or_default();
                Some(EventDefinition::with_evolution(&species))
            }
            // a new override starts at the mon's current values: the numbers
            // are set in the details panel
            Some("ev_override") => {
                let sx = self.state_at_insertion_point(ctrl).map(|s| s.solo_pkmn.unrealized_stat_xp);
                let e = match sx {
                    Some(sx) => EvOverrideEventDefinition::new(sx.hp, sx.attack, sx.defense, sx.special_attack, sx.special_defense, sx.speed),
                    None => EvOverrideEventDefinition::default(),
                };
                Some(EventDefinition::with_ev_override(e))
            }
            _ => None,
        })
    }

    /// `_on_create`
    fn on_create(&mut self, ctrl: &mut MainController, outcome: &mut InlineOutcome) {
        match self.build(ctrl) {
            Ok(Some(ev)) => {
                if self.editing_group_id.is_some() {
                    outcome.result = Some(ev);
                } else {
                    ctrl.new_event(ev, self.insert_after_id, None, self.dest_folder_name.as_deref(), true);
                }
                outcome.created = true;
            }
            Ok(None) => {}
            Err(e) => log::error!("Inline event creation failed: {}", e),
        }
    }

    /// Draw the strip inside `rect`. Returns what happened.
    pub fn ui(&mut self, ui: &mut Ui, theme: &Theme, ctrl: &mut MainController) -> InlineOutcome {
        let mut outcome = InlineOutcome::default();
        let frame = egui::Frame::new()
            .fill(Color32::from_rgb(0x1e, 0x2d, 0x3d))
            .stroke(Stroke::new(1.0_f32, theme.accent))
            .corner_radius(CornerRadius::same(3))
            .inner_margin(egui::Margin { left: 6, right: 6, top: 3, bottom: 3 });
        let focus = self.focus_request.take();
        let mut create_now = false;
        let mut add_all = false;
        let mut discard = false;
        frame.show(ui, |ui| {
            ui.set_min_height(INLINE_ROW_HEIGHT - 6.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                // ---- type combo ----
                // the bag can only be reordered in gen 1
                // (still offered while editing an existing one, whatever the route's gen)
                let gen1 = ctrl.gen().map(|g| g.supports_bag_reorder()).unwrap_or(false) || self.type_key == Some("reorder_bag");
                let names: Vec<String> = EVENT_TYPES.iter().filter(|(_, k)| gen1 || *k != "reorder_bag").map(|(d, _)| d.to_string()).collect();
                let before = self.type_text.clone();
                let r = SearchableDropdown::new(theme, self.id.with("type"), &mut self.type_text, &names)
                    .widths(110.0, 140.0)
                    .placeholder("Event type...")
                    .request_focus(focus == Some(Field::Type))
                    .show(ui);
                if r.changed || self.type_text != before {
                    if let Some((_, key)) = EVENT_TYPES.iter().find(|(d, _)| *d == self.type_text) {
                        let key: &'static str = key;
                        self.set_type(key, ctrl);
                    }
                }
                if r.enter_pressed || r.tab_pressed {
                    if self.advance(Field::Type, true, r.enter_pressed) {
                        create_now = true;
                    }
                } else if r.backtab_pressed {
                    self.advance(Field::Type, false, false);
                }
                // ---- config ----
                let key = self.type_key;
                match key {
                    Some("trainer") => {
                        widgets::label(ui, theme, "Loc:");
                        let locs = self.locations.clone();
                        let r = SearchableDropdown::new(theme, self.id.with("loc"), &mut self.cfg.loc, &locs).widths(110.0, 200.0).request_focus(focus == Some(Field::Loc)).show(ui);
                        if r.changed {
                            self.refresh_trainers(ctrl);
                        }
                        self.handle_nav(&r, Field::Loc, &mut create_now);
                        widgets::label(ui, theme, "Class:");
                        let classes = self.classes.clone();
                        let r = SearchableDropdown::new(theme, self.id.with("cls"), &mut self.cfg.cls, &classes).widths(100.0, 200.0).request_focus(focus == Some(Field::Cls)).show(ui);
                        if r.changed {
                            self.refresh_trainers(ctrl);
                        }
                        self.handle_nav(&r, Field::Cls, &mut create_now);
                        widgets::label(ui, theme, "Trainer:");
                        let trainers = self.cfg.trainers.clone();
                        let r = SearchableDropdown::new(theme, self.id.with("trainer"), &mut self.cfg.trainer, &trainers).widths(160.0, 200.0).request_focus(focus == Some(Field::Trainer)).show(ui);
                        self.handle_nav(&r, Field::Trainer, &mut create_now);
                    }
                    Some("get_item") | Some("buy_item") | Some("sell_item") | Some("use_item") => {
                        widgets::label(ui, theme, "Item:");
                        let items = self.cfg.items.clone();
                        let r = SearchableDropdown::new(theme, self.id.with("item"), &mut self.cfg.item, &items).widths(130.0, 200.0).request_focus(focus == Some(Field::Item)).show(ui);
                        if r.changed {
                            let keep = Some(self.cfg.pp_move.clone());
                            self.refresh_pp(ctrl, keep);
                        }
                        self.handle_nav(&r, Field::Item, &mut create_now);
                        widgets::label(ui, theme, "Qty:");
                        let r = AmountEntry::new(theme, self.id.with("qty"), &mut self.cfg.qty).min(Some(1)).max(Some(999)).request_focus(focus == Some(Field::Qty)).show(ui);
                        self.handle_amount_nav(r.enter_pressed, r.tab_pressed, r.backtab_pressed, Field::Qty, &mut create_now);
                        if self.cfg.pp_needs_target {
                            widgets::label(ui, theme, "On:");
                            let moves = self.cfg.pp_moves.clone();
                            let r = SearchableDropdown::new(theme, self.id.with("pp_move"), &mut self.cfg.pp_move, &moves).widths(130.0, 200.0).request_focus(focus == Some(Field::PpMove)).show(ui);
                            self.handle_nav(&r, Field::PpMove, &mut create_now);
                        }
                    }
                    Some("hold_item") => {
                        widgets::label(ui, theme, "Item:");
                        let items = self.cfg.items.clone();
                        let r = SearchableDropdown::new(theme, self.id.with("item"), &mut self.cfg.item, &items).widths(130.0, 200.0).request_focus(focus == Some(Field::Item)).show(ui);
                        self.handle_nav(&r, Field::Item, &mut create_now);
                    }
                    Some("wild_pkmn") => {
                        widgets::label(ui, theme, "Pkmn:");
                        let names = self.all_pkmn.clone();
                        let r = SearchableDropdown::new(theme, self.id.with("pkmn"), &mut self.cfg.pkmn, &names).widths(120.0, 200.0).request_focus(focus == Some(Field::Pkmn)).show(ui);
                        self.handle_nav(&r, Field::Pkmn, &mut create_now);
                        widgets::label(ui, theme, "Lv:");
                        let r = AmountEntry::new(theme, self.id.with("level"), &mut self.cfg.level).min(Some(2)).max(Some(100)).request_focus(focus == Some(Field::Level)).show(ui);
                        self.handle_amount_nav(r.enter_pressed, r.tab_pressed, r.backtab_pressed, Field::Level, &mut create_now);
                        widgets::label(ui, theme, "Qty:");
                        let r = AmountEntry::new(theme, self.id.with("qty"), &mut self.cfg.qty).min(Some(1)).max(Some(999)).request_focus(focus == Some(Field::Qty)).show(ui);
                        self.handle_amount_nav(r.enter_pressed, r.tab_pressed, r.backtab_pressed, Field::Qty, &mut create_now);
                    }
                    Some("rare_candy") => {
                        widgets::label(ui, theme, "Qty:");
                        let r = AmountEntry::new(theme, self.id.with("qty"), &mut self.cfg.qty).min(Some(1)).max(Some(999)).request_focus(focus == Some(Field::Qty)).show(ui);
                        self.handle_amount_nav(r.enter_pressed, r.tab_pressed, r.backtab_pressed, Field::Qty, &mut create_now);
                    }
                    Some("vitamin") => {
                        widgets::label(ui, theme, "Vitamin:");
                        let vits = self.vitamins.clone();
                        let r = SearchableDropdown::new(theme, self.id.with("vitamin"), &mut self.cfg.vitamin, &vits).widths(100.0, 200.0).request_focus(focus == Some(Field::Vitamin)).show(ui);
                        self.handle_nav(&r, Field::Vitamin, &mut create_now);
                        widgets::label(ui, theme, "Qty:");
                        let r = AmountEntry::new(theme, self.id.with("qty"), &mut self.cfg.qty).min(Some(1)).max(Some(999)).request_focus(focus == Some(Field::Qty)).show(ui);
                        self.handle_amount_nav(r.enter_pressed, r.tab_pressed, r.backtab_pressed, Field::Qty, &mut create_now);
                    }
                    Some("tm_move") => {
                        widgets::label(ui, theme, "TM/HM:");
                        let tms = self.cfg.tms.clone();
                        let r = SearchableDropdown::new(theme, self.id.with("tm"), &mut self.cfg.tm, &tms).widths(130.0, 200.0).request_focus(focus == Some(Field::Tm)).show(ui);
                        if r.changed {
                            self.refresh_dest(ctrl);
                        }
                        self.handle_nav(&r, Field::Tm, &mut create_now);
                        widgets::label(ui, theme, "Over:");
                        let opts = self.cfg.dest_options.clone();
                        let r = SearchableDropdown::new(theme, self.id.with("dest"), &mut self.cfg.dest, &opts).widths(140.0, 200.0).request_focus(focus == Some(Field::Dest)).show(ui);
                        self.handle_nav(&r, Field::Dest, &mut create_now);
                    }
                    Some("tutor_move") => {
                        widgets::label(ui, theme, "Move:");
                        let moves = self.all_moves.clone();
                        let r = SearchableDropdown::new(theme, self.id.with("move"), &mut self.cfg.mv, &moves).widths(140.0, 200.0).request_focus(focus == Some(Field::Move)).show(ui);
                        if r.changed {
                            self.refresh_dest(ctrl);
                        }
                        self.handle_nav(&r, Field::Move, &mut create_now);
                        widgets::label(ui, theme, "Over:");
                        let opts = self.cfg.dest_options.clone();
                        let r = SearchableDropdown::new(theme, self.id.with("dest"), &mut self.cfg.dest, &opts).widths(140.0, 200.0).request_focus(focus == Some(Field::Dest)).show(ui);
                        self.handle_nav(&r, Field::Dest, &mut create_now);
                    }
                    Some("notes") => {
                        widgets::label(ui, theme, "Note:");
                        let id = self.id.with("note");
                        if focus == Some(Field::Note) {
                            ui.memory_mut(|m| m.request_focus(id));
                        }
                        let r = Entry::new(theme, &mut self.cfg.note).width(200.0).hint("Enter note...").id(id).show(ui);
                        self.handle_amount_nav(r.enter_pressed, r.tab_pressed, r.backtab_pressed, Field::Note, &mut create_now);
                    }
                    _ => {}
                }
                // ---- buttons ----
                let label = if self.editing_group_id.is_some() { "Confirm" } else { "Create" };
                if StyledButton::new(theme, label).fixed_width(60.0).enabled(self.can_create()).show(ui).clicked() {
                    create_now = true;
                }
                if self.type_key == Some("trainer") && self.editing_group_id.is_none() && self.cfg.loc != consts::ALL_TRAINERS {
                    if StyledButton::new(theme, "Add all trainers").fixed_width(110.0).show(ui).clicked() {
                        add_all = true;
                    }
                }
                if StyledButton::new(theme, "Discard").fixed_width(60.0).show(ui).clicked() {
                    discard = true;
                }
            });
        });
        // Escape discards
        if !behind_modal(ui) && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            discard = true;
        }
        if create_now && self.can_create() {
            self.on_create(ctrl, &mut outcome);
        }
        if add_all {
            let loc = self.cfg.loc.clone();
            if !loc.is_empty() && loc != consts::ALL_TRAINERS {
                ctrl.add_area(&loc, false, self.insert_after_id);
                outcome.discarded = true;
            }
        }
        if discard {
            outcome.discarded = true;
        }
        outcome
    }

    fn handle_nav(&mut self, r: &widgets::SearchableResponse, field: Field, create_now: &mut bool) {
        if r.enter_pressed || r.tab_pressed {
            if self.advance(field, true, r.enter_pressed) {
                *create_now = true;
            }
        } else if r.backtab_pressed {
            self.advance(field, false, false);
        }
    }

    fn handle_amount_nav(&mut self, enter: bool, tab: bool, backtab: bool, field: Field, create_now: &mut bool) {
        if enter || tab {
            if self.advance(field, true, enter) {
                *create_now = true;
            }
        } else if backtab {
            self.advance(field, false, false);
        }
    }
}

/// `int(dest_text.split("#")[1][0]) - 1`, `None` for "Don't Learn", 0 on failure.
fn parse_dest(dest_text: &str) -> Option<i64> {
    if dest_text == consts::MOVE_DONT_LEARN {
        return None;
    }
    let d = dest_text.split('#').nth(1).and_then(|s| s.chars().next()).and_then(|c| c.to_digit(10)).map(|d| d as i64 - 1).unwrap_or(0);
    Some(d)
}
