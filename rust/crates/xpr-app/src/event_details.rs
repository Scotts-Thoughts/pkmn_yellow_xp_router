//! Port of `gui_qt/event_details.py`: the right-hand panel with the
//! Pre-Event State / Battle Summary tabs, the auto-switch checkbox, the
//! event editor, the notes footer and the delayed-save machinery.

use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Ui, Vec2};

use xpr_core::consts;
use xpr_core::Config;
use xpr_engine::{EventDefinition, NodeId, ObjKind, RouteState};
use xpr_ui_kit::theme::Theme;
use xpr_ui_kit::widgets;

use crate::assets::Assets;
use crate::battle::BattleController;
use crate::battle_ui::{BattleSummaryUi, BattleUiActions};
use crate::controller::MainController;
use crate::editors::{EditorCtx, EventEditors, NotesEditor};
use crate::state_views::{self, BeforeInfo, LastPkmn};

/// Side gutter of the pre-event state pane.
const PANE_PAD: f32 = 16.0;
/// Reserved height of the pinned notes footer (collapsed / expanded).
const NOTES_H_COLLAPSED: f32 = 40.0;
const NOTES_H_EXPANDED: f32 = 168.0;

pub const SAVE_DELAY_MS: u64 = 2000;
pub const PRE_STATE_TAB: usize = 0;
pub const BATTLE_SUMMARY_TAB: usize = 1;
pub const MAP_TAB: usize = 2;

/// What the panel asks the window to do this frame.
#[derive(Clone, Debug, Default)]
pub struct DetailsActions {
    pub battle: BattleUiActions,
    /// the tab changed (the splitter re-proportions)
    pub tab_changed: Option<bool>,
    /// a move slot was clicked in the Pre-Event State moves card
    pub pre_state_assign_move_slot: Option<i64>,
    /// the EV column was clicked in the Pre-Event State stats card
    pub pre_state_override_evs: bool,
    /// a PP item was picked from a move row's menu in the Pre-Event State
    /// moves card: (item, target move), to use before the selected event
    pub pre_state_use_pp_item: Option<(String, Option<String>)>,
    /// the Map tab is active: the window draws the map view into this rect
    pub map_body: Option<egui::Rect>,
    /// the editor's "Map" button: show the selected event on the map
    pub show_on_map: bool,
}

pub struct EventDetails {
    pub tab: usize,
    pub auto_switch: bool,
    pub bc: BattleController,
    pub battle_ui: BattleSummaryUi,
    editors: EventEditors,
    pub notes: NotesEditor,
    current_event_type: Option<String>,
    current_init_state: Option<Arc<RouteState>>,
    allow_updates: bool,
    ignore_tab_switching: bool,
    suppress_battle_summary: bool,
    cur_delayed_event_id: Option<NodeId>,
    cur_delayed_deadline: Option<Instant>,
    deferred_show_at: Option<Instant>,
    last_pkmn: Option<LastPkmn>,
    editor_reload_pending: bool,
}

impl EventDetails {
    pub fn new(cfg: &Config) -> EventDetails {
        EventDetails {
            tab: PRE_STATE_TAB,
            auto_switch: cfg.do_auto_switch(),
            bc: BattleController::new(),
            battle_ui: BattleSummaryUi::new(),
            editors: EventEditors::default(),
            notes: NotesEditor::new(cfg),
            current_event_type: None,
            current_init_state: None,
            allow_updates: false,
            ignore_tab_switching: false,
            suppress_battle_summary: false,
            cur_delayed_event_id: None,
            cur_delayed_deadline: None,
            deferred_show_at: None,
            last_pkmn: None,
            editor_reload_pending: false,
        }
    }

    pub fn is_battle_tab(&self) -> bool {
        self.tab == BATTLE_SUMMARY_TAB
    }

    pub fn is_map_tab(&self) -> bool {
        self.tab == MAP_TAB
    }

    /// `_tab_changed_callback`
    fn tab_changed(&mut self, cfg: &Config, actions: &mut DetailsActions) {
        let is_battle = self.is_battle_tab();
        actions.tab_changed = Some(is_battle);
        if is_battle {
            self.deferred_show_at = Some(Instant::now() + Duration::from_millis(300));
        } else {
            self.battle_ui.hide_contents();
        }
        self.update_notes_visibility(cfg);
    }

    pub fn change_tabs(&mut self, cfg: &Config, actions: &mut DetailsActions) {
        self.tab = if self.tab == BATTLE_SUMMARY_TAB { PRE_STATE_TAB } else { BATTLE_SUMMARY_TAB };
        self.tab_changed(cfg, actions);
    }

    pub fn set_tab(&mut self, cfg: &Config, tab: usize, actions: &mut DetailsActions) {
        if self.tab != tab {
            self.tab = tab;
            self.tab_changed(cfg, actions);
        }
    }

    pub fn set_suppress_battle_summary(&mut self, cfg: &Config, suppress: bool, actions: &mut DetailsActions) {
        self.suppress_battle_summary = suppress;
        if suppress {
            self.set_tab(cfg, PRE_STATE_TAB, actions);
        }
    }

    /// `_update_notes_visibility`
    pub fn update_notes_visibility(&mut self, cfg: &Config) {
        let collapse = self.is_battle_tab() && !cfg.are_notes_visible_in_battle_summary();
        self.notes.force_collapsed(collapse);
    }

    // ---- controller event handlers ---------------------------------------------------

    pub fn handle_version_change(&mut self, cfg: &Config, ctrl: &mut MainController) {
        self.bc.load_empty(cfg, ctrl);
        self.battle_ui.configure_for_version(ctrl);
    }

    /// `_handle_route_change`
    pub fn handle_route_change(&mut self, cfg: &Config, ctrl: &mut MainController) {
        let sel = ctrl.get_single_selected_event_id(true);
        match sel {
            None => {
                self.current_init_state = ctrl.get_init_state();
                self.set_team_none(cfg, ctrl);
            }
            Some(id) => {
                self.current_init_state = ctrl.router.init_state_of(id).cloned();
                // a battle-summary operation (candies, vitamins, held item,
                // tutor move) already reloaded this fight and restored the
                // unsaved selections; reloading again would discard them
                if self.bc.take_handled_route_change(id) {
                    return;
                }
                self.load_team_for(cfg, ctrl, id);
            }
        }
    }

    fn set_team_none(&mut self, cfg: &Config, ctrl: &mut MainController) {
        self.bc.load_empty(cfg, ctrl);
    }

    /// The `set_team` calls of `_handle_route_change` / `show_event_details`.
    fn load_team_for(&mut self, cfg: &Config, ctrl: &mut MainController, id: NodeId) {
        let kind = ctrl.router.obj_kind(id);
        let def: Option<EventDefinition> = match kind {
            Some(ObjKind::Group) => ctrl.router.group(id).map(|g| g.event_definition.clone()),
            Some(ObjKind::Item) => ctrl.router.item(id).map(|i| i.event_definition.clone()),
            Some(ObjKind::Folder) => ctrl.router.folder(id).map(|f| f.event_definition.clone()),
            None => None,
        };
        let Some(def) = def else {
            self.set_team_none(cfg, ctrl);
            return;
        };
        let init_state = ctrl.router.init_state_of(id).cloned();
        if def.trainer_def.is_some() {
            if kind == Some(ObjKind::Group) {
                self.bc.load_from_event(cfg, ctrl, id);
            } else if let Some(st) = init_state {
                // Python: set_team(pokemon list, cur_state, event_group=None) -> load_from_state
                let mons = ctrl.gen().and_then(|g| def.pokemon_list(&g).ok()).unwrap_or_default();
                self.bc.load_from_state(cfg, ctrl, &st, &mons, false);
            } else {
                self.set_team_none(cfg, ctrl);
            }
        } else if def.wild_pkmn_info.is_some() {
            let wild = ctrl.gen().and_then(|g| def.pokemon_list(&g).ok()).unwrap_or_default();
            match init_state {
                Some(st) if !wild.is_empty() => self.bc.load_from_state(cfg, ctrl, &st, &wild, true),
                _ => self.set_team_none(cfg, ctrl),
            }
        } else {
            self.set_team_none(cfg, ctrl);
        }
    }

    /// `_handle_selection` (also fired on record-mode changes)
    pub fn handle_selection(&mut self, cfg: &Config, ctrl: &mut MainController, actions: &mut DetailsActions) {
        let force_pre = self.suppress_battle_summary;
        // the Map tab stays put: the map is a working surface, not a per-event view
        let on_map = self.tab == MAP_TAB;
        if ctrl.is_record_mode_active() && !on_map {
            self.set_tab(cfg, BATTLE_SUMMARY_TAB, actions);
        }
        let sel = ctrl.get_single_selected_event_id(true);
        match sel {
            None => {
                let init = ctrl.get_init_state();
                self.show_event_details(cfg, ctrl, None, init, false, None);
            }
            Some(id) => match ctrl.router.obj_kind(id) {
                Some(ObjKind::Folder) => {
                    let def = ctrl.router.folder(id).map(|f| f.event_definition.clone());
                    let init = ctrl.router.init_state_of(id).cloned();
                    self.show_event_details(cfg, ctrl, def, init, true, Some(id));
                }
                Some(kind) => {
                    let def = match kind {
                        ObjKind::Group => ctrl.router.group(id).map(|g| g.event_definition.clone()),
                        _ => ctrl.router.item(id).map(|i| i.event_definition.clone()),
                    };
                    let Some(def) = def else { return };
                    let do_allow_updates = kind == ObjKind::Group || def.get_event_type() == consts::TASK_LEARN_MOVE_LEVELUP;
                    // an item without a learn move resolves to its parent group for the battle
                    let mut trainer_group_id = id;
                    if kind == ObjKind::Item && def.learn_move.is_none() {
                        trainer_group_id = ctrl.router.item(id).map(|i| i.parent).unwrap_or(id);
                    }
                    let trainer_def = match ctrl.router.obj_kind(trainer_group_id) {
                        Some(ObjKind::Group) => ctrl.router.group(trainer_group_id).map(|g| g.event_definition.clone()),
                        _ => ctrl.router.item(trainer_group_id).map(|i| i.event_definition.clone()),
                    }
                    .unwrap_or_default();
                    let has_battle = trainer_def.trainer_def.is_some() || trainer_def.wild_pkmn_info.is_some();
                    if on_map {
                        // keep the map
                    } else if force_pre {
                        self.set_tab(cfg, PRE_STATE_TAB, actions);
                    } else if self.ignore_tab_switching || self.auto_switch {
                        if ctrl.is_record_mode_active() {
                            if has_battle {
                                self.set_tab(cfg, BATTLE_SUMMARY_TAB, actions);
                            }
                        } else if has_battle {
                            self.set_tab(cfg, BATTLE_SUMMARY_TAB, actions);
                        } else {
                            self.set_tab(cfg, PRE_STATE_TAB, actions);
                        }
                    }
                    let init = ctrl.router.init_state_of(id).cloned();
                    self.show_event_details(cfg, ctrl, Some(def), init, do_allow_updates, Some(trainer_group_id));
                }
                None => {}
            },
        }
    }

    /// `show_event_details(event_def, init_state, final_state, allow_updates, event_group)`
    fn show_event_details(&mut self, cfg: &Config, ctrl: &mut MainController, def: Option<EventDefinition>, init_state: Option<Arc<RouteState>>, allow_updates: bool, event_group: Option<NodeId>) {
        self.force_and_clear_event_update(cfg, ctrl);
        let allow_updates = allow_updates && !ctrl.is_record_mode_active();
        self.allow_updates = allow_updates;
        self.current_init_state = init_state.clone();
        self.current_event_type = None;
        match &def {
            None => {
                self.notes.load_event(cfg, None);
                self.set_team_none(cfg, ctrl);
            }
            Some(d) => {
                self.notes.load_event(cfg, Some(d));
                if d.trainer_def.is_some() {
                    self.battle_ui.should_render = true;
                    match event_group {
                        Some(gid) if ctrl.router.obj_kind(gid) == Some(ObjKind::Group) => self.bc.load_from_event(cfg, ctrl, gid),
                        _ => {
                            let mons = ctrl.gen().and_then(|g| d.pokemon_list(&g).ok()).unwrap_or_default();
                            match &init_state {
                                Some(st) => self.bc.load_from_state(cfg, ctrl, st, &mons, false),
                                None => self.set_team_none(cfg, ctrl),
                            }
                        }
                    }
                } else if d.wild_pkmn_info.is_some() {
                    self.battle_ui.should_render = true;
                    let wild = ctrl.gen().and_then(|g| d.pokemon_list(&g).ok()).unwrap_or_default();
                    match &init_state {
                        Some(st) if !wild.is_empty() => self.bc.load_from_state(cfg, ctrl, st, &wild, true),
                        _ => self.set_team_none(cfg, ctrl),
                    }
                } else {
                    self.set_team_none(cfg, ctrl);
                }
                let et = d.get_event_type();
                if et != consts::TASK_NOTES_ONLY {
                    if let Some(gen) = ctrl.gen() {
                        let ctx = EditorCtx { theme: &Theme::from_config(cfg), gen: &gen, cfg, cur_state: init_state.as_deref(), event_type: et, enabled: allow_updates };
                        self.editors.load(&ctx, d);
                        self.current_event_type = Some(et.to_string());
                    }
                }
            }
        }
        self.notes.set_enabled(true);
    }

    // ---- saving -------------------------------------------------------------------------

    pub fn update_existing_event(&mut self, cfg: &Config, ctrl: &mut MainController) {
        let id = ctrl.get_single_selected_event_id(true);
        self.event_update_helper(cfg, ctrl, id);
    }

    pub fn update_existing_event_after_delay(&mut self, ctrl: &MainController) {
        let to_save = ctrl.get_single_selected_event_id(true);
        if let Some(cur) = self.cur_delayed_event_id {
            if Some(cur) != to_save {
                log::error!("Unexpected switch of event id from {:?} to {:?}", cur, to_save);
            }
        }
        self.cur_delayed_event_id = to_save;
        self.cur_delayed_deadline = Some(Instant::now() + Duration::from_millis(SAVE_DELAY_MS));
    }

    /// `force_and_clear_event_update` (the pre-save hook too)
    pub fn force_and_clear_event_update(&mut self, cfg: &Config, ctrl: &mut MainController) {
        let Some(id) = self.cur_delayed_event_id else { return };
        self.event_update_helper(cfg, ctrl, Some(id));
    }

    fn delayed_event_update(&mut self, cfg: &Config, ctrl: &mut MainController) {
        let (Some(id), Some(deadline)) = (self.cur_delayed_event_id, self.cur_delayed_deadline) else { return };
        if Instant::now() < deadline {
            return;
        }
        self.event_update_helper(cfg, ctrl, Some(id));
    }

    fn event_update_helper(&mut self, cfg: &Config, ctrl: &mut MainController, event_to_update: Option<NodeId>) {
        let Some(id) = event_to_update else { return };
        if let Some(cur) = self.cur_delayed_event_id {
            if cur != id {
                log::error!("Found delayed update for: {} which is different from the current update for {}", cur, id);
            }
        }
        self.cur_delayed_event_id = None;
        self.cur_delayed_deadline = None;
        let new_event: Result<EventDefinition, String> = match (&self.current_event_type, ctrl.gen()) {
            (Some(et), Some(gen)) => {
                let ctx = EditorCtx { theme: &Theme::from_config(cfg), gen: &gen, cfg, cur_state: self.current_init_state.as_deref(), event_type: et, enabled: self.allow_updates };
                self.editors.get_event(&ctx)
            }
            _ => Ok(EventDefinition::default()),
        };
        let mut new_event = match new_event {
            Ok(e) => e,
            Err(e) => {
                log::error!("Exception occurred trying to update current event: {}", e);
                ctrl.trigger_exception("Exception occurred trying to update current event");
                return;
            }
        };
        if new_event.get_event_type() == consts::TASK_TRAINER_BATTLE {
            match self.bc.summary.get_partial_trainer_definition() {
                None => log::error!("Expected to get updated trainer def from battle summary controller, but got None"),
                Some(mut td) => {
                    let editor_td = new_event.trainer_def.clone().unwrap_or_else(|| td.clone());
                    td.exp_split = editor_td.exp_split;
                    td.pay_day_amount = editor_td.pay_day_amount;
                    td.mon_order = editor_td.mon_order;
                    td.thief_mons = editor_td.thief_mons;
                    new_event.trainer_def = Some(td);
                }
            }
        }
        new_event.notes = self.notes.get_event().notes;
        ctrl.update_existing_event(id, new_event);
    }

    /// Timers: the delayed save and the 300 ms battle-summary reveal.
    pub fn tick(&mut self, ctx: &egui::Context, cfg: &Config, ctrl: &mut MainController) {
        if let Some(d) = self.cur_delayed_deadline {
            let now = Instant::now();
            if now >= d {
                self.delayed_event_update(cfg, ctrl);
            } else {
                ctx.request_repaint_after(d - now);
            }
        }
        if let Some(d) = self.deferred_show_at {
            let now = Instant::now();
            if now >= d {
                self.deferred_show_at = None;
                self.battle_ui.show_contents();
                self.update_notes_visibility(cfg);
            } else {
                ctx.request_repaint_after(d - now);
            }
        }
        self.battle_ui.tick(ctx, &mut self.bc, ctrl, cfg);
    }

    /// Drain the battle controller's signals (refresh -> resync the widgets,
    /// non-load change -> schedule the delayed save).
    pub fn drain_battle_signals(&mut self, cfg: &Config, ctrl: &mut MainController) {
        let sig = self.bc.take_signals();
        if sig.refresh {
            self.battle_ui.sync_from_controller(&self.bc, ctrl);
            self.update_notes_visibility(cfg);
        }
        if sig.nonload_change {
            self.update_existing_event_after_delay(ctrl);
        }
    }

    // ---- drawing -----------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    pub fn ui(&mut self, ui: &mut Ui, theme: &Theme, cfg: &mut Config, ctrl: &mut MainController, assets: &mut Assets, actions: &mut DetailsActions) {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);
        let total_h = ui.available_height();
        // egui lays out top-down, so the pinned notes footer's height is
        // reserved up front and the tab body gets the rest.
        let notes_h = if self.tab == MAP_TAB { 0.0 } else if self.notes_expanded() { NOTES_H_EXPANDED } else { NOTES_H_COLLAPSED };
        let body_h = (total_h - notes_h).max(100.0);
        let width = ui.available_width();
        ui.allocate_ui_with_layout(Vec2::new(width, body_h), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(width);
            ui.set_height(body_h);
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);
            let mut tab = self.tab;
            let mut auto = self.auto_switch;
            let mut auto_toggled = false;
            // no Map tab for games without a map pack (gens 1-3 only)
            let tabs: &[&str] = if crate::map::MapView::game_of(ctrl).is_some() { &["Pre-Event State", "Battle Summary", "Map"] } else { &["Pre-Event State", "Battle Summary"] };
            if widgets::tab_bar(ui, theme, tabs, &mut tab, |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let r = ui.add(egui::Label::new(egui::RichText::new("Auto-switch tabs").font(theme.body()).color(theme.secondary)).sense(egui::Sense::click()));
                if r.clicked() {
                    auto = !auto;
                    auto_toggled = true;
                }
                if widgets::checkbox(ui, theme, &mut auto, "", true).changed() {
                    auto_toggled = true;
                }
            }) {
                self.tab = tab;
                self.tab_changed(cfg, actions);
            }
            if auto_toggled {
                self.auto_switch = auto;
                cfg.set_auto_switch(auto);
            }
            let content_h = (body_h - widgets::TAB_STRIP_H).max(50.0);
            if self.tab == MAP_TAB {
                let r = ui.available_rect_before_wrap();
                actions.map_body = Some(r);
                ui.allocate_rect(r, egui::Sense::hover());
                return;
            }
            if self.tab == PRE_STATE_TAB {
                self.pre_state_tab(ui, theme, cfg, ctrl, assets, content_h, actions);
            } else {
                let pane = egui::Frame::new().stroke(theme.border_stroke()).inner_margin(egui::Margin::same(0));
                pane.show(ui, |ui| {
                    ui.set_min_height(content_h);
                    ui.set_width(ui.available_width());
                    self.battle_tab(ui, theme, cfg, ctrl, assets, actions);
                });
            }
        });
        if self.tab == MAP_TAB {
            return;
        }
        let (out, mode_changed) = self.notes.ui(ui, theme, cfg);
        if mode_changed {
            self.update_notes_visibility(cfg);
        }
        if out.delayed_save {
            self.update_existing_event_after_delay(ctrl);
        }
    }

    fn notes_expanded(&self) -> bool {
        self.notes.is_expanded()
    }

    /// The header's "Before" block: what the selected node is and where.
    fn before_info(ctrl: &MainController) -> BeforeInfo {
        let Some(id) = ctrl.get_single_selected_event_id(true) else {
            return BeforeInfo { label: "Route start".to_string(), location: None };
        };
        // the invisible root folder is nobody's location
        let folder_name = |fid: Option<NodeId>| fid.filter(|f| *f != ctrl.router.root_id).and_then(|f| ctrl.router.folder(f)).map(|f| f.name.clone());
        // the nearest folder above `id`
        let enclosing_folder = |mut cur: NodeId| -> Option<String> {
            loop {
                let parent = ctrl.router.parent_of(cur)?;
                if ctrl.router.obj_kind(parent) == Some(ObjKind::Folder) {
                    return folder_name(Some(parent));
                }
                cur = parent;
            }
        };
        match ctrl.router.obj_kind(id) {
            Some(ObjKind::Folder) => {
                let f = ctrl.router.folder(id);
                BeforeInfo { label: f.map(|f| f.name.clone()).unwrap_or_default(), location: folder_name(f.and_then(|f| f.parent)) }
            }
            Some(kind) => {
                let (def, name) = match kind {
                    ObjKind::Group => ctrl.router.group(id).map(|g| (g.event_definition.clone(), g.name.clone())),
                    _ => ctrl.router.item(id).map(|i| (i.event_definition.clone(), i.name.clone())),
                }
                .unwrap_or_default();
                let gen = ctrl.gen();
                let mut label = gen.as_ref().and_then(|g| def.get_label(g).ok()).unwrap_or(name);
                let mut location = None;
                if def.trainer_def.is_some() {
                    let trainer = gen.as_ref().and_then(|g| def.get_first_trainer_obj(g).ok().flatten().cloned());
                    // "Trainer: Name (Location)" / "Multi: A, B (Location)" -> "Name" / "A, B"
                    for prefix in ["Trainer: ", "Multi: "] {
                        if let Some(rest) = label.strip_prefix(prefix) {
                            label = rest.to_string();
                            break;
                        }
                    }
                    if let Some(t) = &trainer {
                        if !t.location.is_empty() {
                            let suffix = format!(" ({})", t.location);
                            if let Some(rest) = label.strip_suffix(&suffix) {
                                label = rest.to_string();
                            }
                            location = Some(t.location.clone());
                        }
                    }
                }
                if location.is_none() {
                    location = enclosing_folder(id);
                }
                BeforeInfo { label, location }
            }
            None => BeforeInfo { label: "Route start".to_string(), location: None },
        }
    }

    /// The warnings of the selected event (an event that applied, but not
    /// exactly as written); shown above its editor.
    fn selected_warnings(ctrl: &MainController) -> Vec<String> {
        let Some(id) = ctrl.get_single_selected_event_id(true) else { return Vec::new() };
        match ctrl.router.obj_kind(id) {
            Some(ObjKind::Group) => ctrl.router.group(id).map(|g| g.warning_messages.clone()).unwrap_or_default(),
            Some(ObjKind::Item) => ctrl
                .router
                .item(id)
                .filter(|i| i.has_warnings())
                .map(|i| vec![i.warning_message.clone()])
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    /// The Moves card's PP view of the selected event (and the PP banner
    /// lines), bringing the PP ledger up to date first. `None` with no
    /// event selected.
    fn moves_pp(cfg: &Config, ctrl: &mut MainController) -> Option<(state_views::MovesPp, Vec<String>)> {
        let id = ctrl.get_single_selected_event_id(true)?;
        ctrl.ensure_pp(cfg);
        let snapshot = ctrl.pp.before(id)?.clone();
        let gen = ctrl.gen()?;
        let bag_items: Vec<(String, bool)> = ctrl
            .router
            .init_state_of(id)
            .map(|st| {
                st.inventory
                    .cur_items
                    .iter()
                    .filter_map(|b| gen.pp_item_effect(&b.base_item.name).map(|e| (b.base_item.name.clone(), e.needs_target())))
                    .collect()
            })
            .unwrap_or_default();
        let view = state_views::MovesPp { snapshot, spend: ctrl.pp.spend(id), history: std::array::from_fn(|i| ctrl.pp.history(id, i).to_vec()), bag_items };
        Some((view, ctrl.pp.messages(&ctrl.router, id)))
    }

    #[allow(clippy::too_many_arguments)]
    fn pre_state_tab(&mut self, ui: &mut Ui, theme: &Theme, cfg: &Config, ctrl: &mut MainController, assets: &mut Assets, height: f32, actions: &mut DetailsActions) {
        let (moves_pp, pp_messages) = match Self::moves_pp(cfg, ctrl) {
            Some((v, m)) => (Some(v), m),
            None => (None, Vec::new()),
        };
        widgets::show_scroll(ui, egui::ScrollArea::vertical().id_salt("pre_state_scroll").max_height(height).auto_shrink([false, false]), |ui| {
            let pad = PANE_PAD as i8;
            egui::Frame::new().inner_margin(egui::Margin { left: pad, right: pad, top: pad, bottom: pad }).show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);
                ui.set_width(ui.available_width());
                let gen = ctrl.gen();
                let state = self.current_init_state.clone();
                let before = Self::before_info(ctrl);
                let clicks = state_views::state_viewer(ui, theme, gen.as_deref(), state.as_deref(), moves_pp.as_ref(), &mut self.last_pkmn, assets, &before);
                if clicks.move_slot.is_some() {
                    actions.pre_state_assign_move_slot = clicks.move_slot;
                }
                if clicks.use_pp_item.is_some() {
                    actions.pre_state_use_pp_item = clicks.use_pp_item.clone();
                }
                if clicks.evs {
                    actions.pre_state_override_evs = true;
                }
                let mut warnings = Self::selected_warnings(ctrl);
                warnings.extend(pp_messages.iter().cloned());
                if !warnings.is_empty() {
                    ui.add_space(state_views::CARD_GAP);
                    for (i, w) in warnings.iter().enumerate() {
                        if i > 0 {
                            ui.add_space(6.0);
                        }
                        widgets::warning_banner(ui, theme, w);
                    }
                }
                if let (Some(et), Some(gen)) = (self.current_event_type.clone(), gen) {
                    ui.add_space(state_views::CARD_GAP);
                    let init = self.current_init_state.clone();
                    let ctx = EditorCtx { theme, gen: &gen, cfg, cur_state: init.as_deref(), event_type: &et, enabled: self.allow_updates };
                    let (out, reload) = self.editors.ui(ui, &ctx, assets, &before);
                    if out.show_on_map {
                        actions.show_on_map = true;
                    }
                    if out.save {
                        self.update_existing_event(cfg, ctrl);
                    }
                    if out.delayed_save {
                        self.update_existing_event_after_delay(ctrl);
                    }
                    if reload {
                        self.editor_reload_pending = true;
                    }
                }
            });
        });
        if self.editor_reload_pending {
            // TrainerFightEditor._reorder_mons: `self.load_event(self.get_event())`
            self.editor_reload_pending = false;
            if let (Some(et), Some(gen)) = (self.current_event_type.clone(), ctrl.gen()) {
                let init = self.current_init_state.clone();
                let ctx = EditorCtx { theme, gen: &gen, cfg, cur_state: init.as_deref(), event_type: &et, enabled: self.allow_updates };
                if let Ok(def) = self.editors.get_event(&ctx) {
                    self.editors.load(&ctx, &def);
                }
            }
        }
    }

    fn battle_tab(&mut self, ui: &mut Ui, theme: &Theme, cfg: &mut Config, ctrl: &mut MainController, assets: &mut Assets, actions: &mut DetailsActions) {
        if !self.battle_ui.should_render {
            return;
        }
        widgets::show_scroll(ui, egui::ScrollArea::vertical().id_salt("battle_scroll").auto_shrink([false, false]), |ui| {
            self.battle_ui.ui(ui, theme, cfg, &mut self.bc, ctrl, assets, &mut actions.battle);
        });
    }

    // ---- delegated helpers ------------------------------------------------------------------

    pub fn increment_prefight_candies(&mut self, cfg: &Config, ctrl: &mut MainController) {
        self.battle_ui.increment_prefight_candies(cfg, &mut self.bc, ctrl);
    }

    pub fn decrement_prefight_candies(&mut self, cfg: &Config, ctrl: &mut MainController) {
        self.battle_ui.decrement_prefight_candies(cfg, &mut self.bc, ctrl);
    }

    /// `_handle_matchup_reorder`
    pub fn handle_matchup_reorder(&mut self, cfg: &Config, ctrl: &mut MainController, from_idx: usize, to_idx: usize) {
        self.force_and_clear_event_update(cfg, ctrl);
        if self.bc.reorder_matchup(cfg, ctrl, from_idx, to_idx) {
            self.reload_trainer_editor(cfg, ctrl);
        }
    }

    /// Resync the trainer editor with the saved definition after the battle
    /// summary rewrote it. Every later save (stage modifiers, notes, ...)
    /// takes `mon_order` from the editor's order menus, so a stale editor
    /// would write the old order back over the reorder.
    fn reload_trainer_editor(&mut self, cfg: &Config, ctrl: &MainController) {
        if self.current_event_type.as_deref() != Some(consts::TASK_TRAINER_BATTLE) {
            return;
        }
        let Some(id) = ctrl.get_single_selected_event_id(true) else { return };
        let (Some(def), Some(gen)) = (ctrl.router.group(id).map(|g| g.event_definition.clone()), ctrl.gen()) else { return };
        let init = self.current_init_state.clone();
        let ctx = EditorCtx { theme: &Theme::from_config(cfg), gen: &gen, cfg, cur_state: init.as_deref(), event_type: consts::TASK_TRAINER_BATTLE, enabled: self.allow_updates };
        self.editors.load(&ctx, &def);
    }

    pub fn refresh_after_config_change(&mut self, cfg: &Config, ctrl: &MainController) {
        self.bc.full_refresh(cfg, ctrl);
    }
}
