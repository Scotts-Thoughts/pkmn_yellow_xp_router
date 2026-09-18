//! The application half of `controllers/battle_summary_controller.py`: wraps
//! the pure calculation state (`xpr_calc::BattleSummary`) with the
//! route-mutating operations (pre-fight candies, vitamins, held item, tutor
//! assignment, matchup reorder, test moves) and the refresh / delayed-save
//! notifications the Qt widgets subscribe to.

use std::collections::HashMap;

use xpr_calc::battle_summary::{BattleSummary, MoveRenderInfo, PkmnRenderInfo, SummaryConfig, TransientState};
use xpr_core::consts;
use xpr_core::Config;
use xpr_data::model::EnemyPkmn;
use xpr_engine::{
    EventDefinition, HoldItemEventDefinition, InventoryEventDefinition, LearnMoveEventDefinition, LevelVal, NodeId,
    ObjKind, RareCandyEventDefinition, RouteState, VitaminEventDefinition,
};

use crate::controller::MainController;

/// Raised by operations; drained by the UI each frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct BattleSignals {
    pub refresh: bool,
    pub nonload_change: bool,
}

pub struct BattleController {
    pub summary: BattleSummary,
    signals: BattleSignals,
    /// The group whose route mutation this controller already reloaded the
    /// summary for (with the transient state restored). The frame's
    /// `route_changed` cascade must not reload it again: Python runs the
    /// cascade synchronously inside the mutation, before the restore.
    handled_route_change: Option<NodeId>,
}

impl Default for BattleController {
    fn default() -> Self {
        BattleController::new()
    }
}

impl BattleController {
    pub fn new() -> BattleController {
        BattleController { summary: BattleSummary::new(), signals: BattleSignals::default(), handled_route_change: None }
    }

    /// Whether the pending `route_changed` cascade for `group_id` was already
    /// handled by a route-mutating operation of this controller (consumed).
    pub fn take_handled_route_change(&mut self, group_id: NodeId) -> bool {
        let handled = self.handled_route_change == Some(group_id);
        self.handled_route_change = None;
        handled
    }

    pub fn take_signals(&mut self) -> BattleSignals {
        std::mem::take(&mut self.signals)
    }

    fn on_refresh(&mut self) {
        self.signals.refresh = true;
    }

    fn on_nonload_change(&mut self) {
        self.signals.nonload_change = true;
    }

    /// `SummaryConfig` from the user config and the route's test moves.
    pub fn summary_config(cfg: &Config, ctrl: &MainController) -> SummaryConfig {
        SummaryConfig {
            calc: cfg.calc_config(),
            player_strategy: cfg.get_player_highlight_strategy().to_string(),
            enemy_strategy: cfg.get_enemy_highlight_strategy().to_string(),
            test_moves_enabled: cfg.get_test_moves_enabled(),
            test_moves: ctrl.router.test_moves.clone(),
        }
    }

    // ---- loading ----------------------------------------------------------------------

    pub fn load_empty(&mut self, cfg: &Config, ctrl: &MainController) {
        let sc = BattleController::summary_config(cfg, ctrl);
        let gen = ctrl.gen();
        self.summary.load_empty(gen.as_deref(), &sc);
        self.on_refresh();
    }

    pub fn load_from_event(&mut self, cfg: &Config, ctrl: &mut MainController, group_id: NodeId) {
        let sc = BattleController::summary_config(cfg, ctrl);
        let r = self.summary.load_from_event(&ctrl.router, Some(group_id), &sc);
        if let Err(e) = r {
            log::error!("Failed to load battle summary from event: {}", e);
            ctrl.trigger_exception(format!("Failed to load battle summary: {}", e));
        }
        self.on_refresh();
    }

    pub fn load_from_state(&mut self, cfg: &Config, ctrl: &mut MainController, init_state: &RouteState, enemy_mons: &[EnemyPkmn], is_wild: bool) {
        let sc = BattleController::summary_config(cfg, ctrl);
        let Some(gen) = ctrl.gen() else { return };
        let r = self.summary.load_from_state(&gen, Some(init_state), enemy_mons, None, is_wild, &sc);
        if let Err(e) = r {
            log::error!("Failed to load battle summary from state: {}", e);
            ctrl.trigger_exception(format!("Failed to load battle summary: {}", e));
        }
        self.on_refresh();
    }

    /// `_full_refresh(is_load=False)`
    pub fn full_refresh(&mut self, cfg: &Config, ctrl: &MainController) {
        if self.summary.suppress_refresh {
            return;
        }
        let sc = BattleController::summary_config(cfg, ctrl);
        let gen = ctrl.gen();
        self.summary.full_refresh(gen.as_deref(), &sc);
        self.on_refresh();
        self.on_nonload_change();
    }

    // ---- simple state changes (recalculate in place) -------------------------------

    pub fn update_mimic_selection(&mut self, cfg: &Config, ctrl: &MainController, new_value: &str) {
        let Some(gen) = ctrl.gen() else { return };
        let sc = BattleController::summary_config(cfg, ctrl);
        self.summary.update_mimic_selection(&gen, &sc, new_value);
        self.on_refresh();
        self.on_nonload_change();
    }

    pub fn update_custom_move_data(&mut self, cfg: &Config, ctrl: &MainController, pkmn_idx: usize, move_idx: usize, is_player: bool, new_value: &str) {
        let Some(gen) = ctrl.gen() else { return };
        let sc = BattleController::summary_config(cfg, ctrl);
        self.summary.update_custom_move_data(&gen, &sc, pkmn_idx, move_idx, is_player, new_value);
        self.on_refresh();
        self.on_nonload_change();
    }

    fn after_full(&mut self) {
        if !self.summary.suppress_refresh {
            self.on_refresh();
            self.on_nonload_change();
        }
    }

    pub fn update_weather(&mut self, cfg: &Config, ctrl: &MainController, new_weather: &str, source_mon_idx: Option<i64>) {
        let Some(gen) = ctrl.gen() else { return };
        let sc = BattleController::summary_config(cfg, ctrl);
        self.summary.update_weather(&gen, &sc, new_weather, source_mon_idx);
        self.after_full();
    }

    pub fn update_enemy_setup_moves(&mut self, cfg: &Config, ctrl: &MainController, moves: Vec<String>) {
        let Some(gen) = ctrl.gen() else { return };
        let sc = BattleController::summary_config(cfg, ctrl);
        self.summary.update_enemy_setup_moves(&gen, &sc, moves);
        self.after_full();
    }

    pub fn update_player_setup_moves(&mut self, cfg: &Config, ctrl: &MainController, moves: Vec<String>) {
        let Some(gen) = ctrl.gen() else { return };
        let sc = BattleController::summary_config(cfg, ctrl);
        self.summary.update_player_setup_moves(&gen, &sc, moves);
        self.after_full();
    }

    pub fn update_player_transform(&mut self, cfg: &Config, ctrl: &MainController, is_transformed: bool) {
        let Some(gen) = ctrl.gen() else { return };
        let sc = BattleController::summary_config(cfg, ctrl);
        self.summary.update_player_transform(&gen, &sc, is_transformed);
        self.after_full();
    }

    pub fn toggle_weather_from_move(&mut self, cfg: &Config, ctrl: &MainController, move_name: &str, enabled: bool, mon_idx: Option<i64>) {
        let Some(gen) = ctrl.gen() else { return };
        let sc = BattleController::summary_config(cfg, ctrl);
        let before = (self.summary.weather.clone(), self.summary.weather_source_mon_idx);
        self.summary.toggle_weather_from_move(&gen, &sc, move_name, enabled, mon_idx);
        if before != (self.summary.weather.clone(), self.summary.weather_source_mon_idx) || enabled {
            self.after_full();
        }
    }

    pub fn toggle_screen_from_move(&mut self, cfg: &Config, ctrl: &MainController, move_name: &str, enabled: bool, mon_idx: i64, is_player: bool) {
        let Some(gen) = ctrl.gen() else { return };
        let sc = BattleController::summary_config(cfg, ctrl);
        if BattleSummary::get_screen_for_move(move_name).is_none() {
            return;
        }
        self.summary.toggle_screen_from_move(&gen, &sc, move_name, enabled, mon_idx, is_player);
        self.after_full();
    }

    pub fn toggle_intimidate(&mut self, cfg: &Config, ctrl: &MainController, mon_idx: i64, is_player: bool, enabled: bool) {
        let Some(gen) = ctrl.gen() else { return };
        let sc = BattleController::summary_config(cfg, ctrl);
        self.summary.toggle_intimidate(&gen, &sc, mon_idx, is_player, enabled);
        self.after_full();
    }

    pub fn update_stat_stage_setup(&mut self, cfg: &Config, ctrl: &MainController, pkmn_idx: usize, move_idx: usize, is_player: bool, new_value: &str) {
        let Some(gen) = ctrl.gen() else { return };
        let sc = BattleController::summary_config(cfg, ctrl);
        self.summary.update_stat_stage_setup(&gen, &sc, pkmn_idx, move_idx, is_player, new_value);
        self.after_full();
    }

    pub fn update_mon_collapsed(&mut self, mon_idx: i64, collapsed: bool) {
        if self.summary.update_mon_collapsed(mon_idx, collapsed) {
            self.on_nonload_change();
        }
    }

    pub fn update_move_highlight(&mut self, mon_idx: i64, move_idx: i64, is_player: bool, reset: bool) {
        self.summary.update_move_highlight(mon_idx, move_idx, is_player, reset);
        self.on_refresh();
        self.on_nonload_change();
    }

    // ---- queries ------------------------------------------------------------------------

    pub fn get_pkmn_info(&self, pkmn_idx: usize, is_player: bool) -> Option<&PkmnRenderInfo> {
        self.summary.get_pkmn_info(pkmn_idx, is_player)
    }

    pub fn get_move_info(&self, cfg: &Config, ctrl: &MainController, pkmn_idx: usize, move_idx: usize, is_player: bool) -> Option<MoveRenderInfo> {
        let sc = BattleController::summary_config(cfg, ctrl);
        self.summary.get_move_info(&sc, pkmn_idx, move_idx, is_player).cloned()
    }

    pub fn get_test_moves(&self, ctrl: &MainController) -> Vec<String> {
        let mut t = ctrl.router.test_moves.clone();
        while t.len() < 4 {
            t.push(String::new());
        }
        t.truncate(4);
        t
    }

    pub fn update_test_move(&mut self, cfg: &Config, ctrl: &mut MainController, slot_idx: usize, move_name: &str) {
        if slot_idx >= 4 {
            return;
        }
        while ctrl.router.test_moves.len() < 4 {
            ctrl.router.test_moves.push(String::new());
        }
        ctrl.router.test_moves[slot_idx] = move_name.to_string();
        self.full_refresh(cfg, ctrl);
        self.on_nonload_change();
    }

    pub fn can_support_prefight_candies(&self) -> bool {
        self.summary.event_group_id.is_some()
    }

    /// `get_prefight_candy_count`
    pub fn get_prefight_candy_count(&self, ctrl: &MainController) -> i64 {
        let Some(gid) = self.summary.event_group_id else { return 0 };
        let Some(prev) = ctrl.get_previous_event(Some(gid)) else { return 0 };
        ctrl.router.group(prev).and_then(|g| g.event_definition.rare_candy.as_ref()).map(|rc| rc.amount).unwrap_or(0)
    }

    /// `get_vitamins_used_per_stat`
    pub fn get_vitamins_used_per_stat(&self, ctrl: &MainController) -> HashMap<&'static str, i64> {
        let mut counts: HashMap<&'static str, i64> = HashMap::new();
        for k in [consts::HP, consts::ATK, consts::DEF, consts::SPA, consts::SPD, consts::SPE] {
            counts.insert(k, 0);
        }
        let Some(gid) = self.summary.event_group_id else { return counts };
        let Some(gen) = ctrl.gen() else { return counts };
        for cur in ctrl.router.all_groups() {
            if cur == gid {
                break;
            }
            if !ctrl.router.is_enabled(cur) {
                continue;
            }
            let Some(g) = ctrl.router.group(cur) else { continue };
            let Some(vit) = &g.event_definition.vitamin else { continue };
            let Ok(boosted) = gen.get_stats_boosted_by_vitamin(&vit.vitamin) else { continue };
            let amount = if vit.amount == 0 { 1 } else { vit.amount };
            for stat in boosted {
                if let Some(c) = counts.get_mut(stat) {
                    *c += amount;
                }
            }
        }
        counts
    }

    pub fn get_player_held_item(&self) -> String {
        self.summary.get_player_held_item()
    }

    pub fn get_held_item_options(&self, ctrl: &MainController) -> Vec<String> {
        match ctrl.gen() {
            Some(g) => BattleSummary::get_held_item_options(&g),
            None => vec![String::new()],
        }
    }

    // ---- route-mutating operations ---------------------------------------------------

    fn with_transient<F: FnOnce(&mut Self, &mut MainController)>(&mut self, f: F, ctrl: &mut MainController) -> TransientState {
        let saved = self.summary.save_transient_state();
        self.summary.suppress_refresh = true;
        f(self, ctrl);
        self.summary.suppress_refresh = false;
        saved
    }

    /// `update_prefight_candies(num_candies)`. With `coalesce_undo` the
    /// mutation joins the previous undo step (a burst of stepper clicks).
    pub fn update_prefight_candies(&mut self, cfg: &Config, ctrl: &mut MainController, num_candies: i64, coalesce_undo: bool) {
        let Some(gid) = self.summary.event_group_id else { return };
        let prev = ctrl.get_previous_event(Some(gid));
        let prev_candy = prev.and_then(|p| ctrl.router.group(p).and_then(|g| g.event_definition.rare_candy.clone()).map(|_| p));
        if prev_candy.is_none() && num_candies <= 0 {
            return;
        }
        if coalesce_undo {
            ctrl.coalesce_next_undo_step();
        }
        let saved = self.with_transient(
            |_bc, ctrl| match prev_candy {
                None => {
                    ctrl.new_event(EventDefinition::with_rare_candy(num_candies), None, Some(gid), None, false);
                }
                Some(pid) => {
                    if num_candies <= 0 {
                        ctrl.delete_events(&[pid]);
                    } else {
                        ctrl.update_existing_event(pid, EventDefinition::with_rare_candy(num_candies));
                    }
                }
            },
            ctrl,
        );
        // The route change reloads the summary from the (not yet saved) event
        // definition; restore what the user had set transiently.
        self.reload_after_mutation(cfg, ctrl, gid, saved);
    }

    /// The route-change cascade's `load_from_event` followed by the transient
    /// restore and a single full refresh.
    fn reload_after_mutation(&mut self, cfg: &Config, ctrl: &mut MainController, gid: NodeId, saved: TransientState) {
        let sc = BattleController::summary_config(cfg, ctrl);
        self.summary.suppress_refresh = true;
        let _ = self.summary.load_from_event(&ctrl.router, Some(gid), &sc);
        self.summary.suppress_refresh = false;
        self.summary.restore_transient_state(saved);
        self.handled_route_change = Some(gid);
        self.full_refresh(cfg, ctrl);
    }

    /// `assign_player_move_via_tutor(slot_idx, move_name)`
    pub fn assign_player_move_via_tutor(&mut self, cfg: &Config, ctrl: &mut MainController, slot_idx: i64, move_name: &str) {
        let Some(trainer_id) = self.summary.event_group_id else { return };
        if move_name.is_empty() || !(0..=3).contains(&slot_idx) {
            return;
        }
        let saved = self.with_transient(
            |_bc, ctrl| {
                ctrl.new_event(
                    EventDefinition::with_learn_move(LearnMoveEventDefinition::new(Some(move_name), Some(slot_idx), consts::MOVE_SOURCE_TUTOR, LevelVal::any(), None, true)),
                    None,
                    Some(trainer_id),
                    None,
                    false,
                );
            },
            ctrl,
        );
        self.summary.suppress_refresh = true;
        let sc = BattleController::summary_config(cfg, ctrl);
        let _ = self.summary.load_from_event(&ctrl.router, Some(trainer_id), &sc);
        self.summary.suppress_refresh = false;
        self.summary.restore_transient_state(saved);
        self.handled_route_change = Some(trainer_id);
        ctrl.select_new_events(vec![trainer_id]);
        self.full_refresh(cfg, ctrl);
    }

    /// `_find_existing_prefight_hold_event`: (hold event, anchor candy event)
    fn find_existing_prefight_hold_event(&self, ctrl: &MainController) -> (Option<NodeId>, Option<NodeId>) {
        let Some(gid) = self.summary.event_group_id else { return (None, None) };
        let mut cursor = gid;
        let mut anchor: Option<NodeId> = None;
        loop {
            let Some(prev) = ctrl.get_previous_event(Some(cursor)) else { return (None, anchor) };
            let Some(g) = ctrl.router.group(prev) else { return (None, anchor) };
            if g.event_definition.rare_candy.is_some() {
                anchor = Some(prev);
                cursor = prev;
                continue;
            }
            if g.event_definition.hold_item.is_some() {
                return (Some(prev), anchor);
            }
            return (None, anchor);
        }
    }

    /// `_maybe_insert_find_for_held_item`
    fn maybe_insert_find_for_held_item(ctrl: &mut MainController, item_name: &str, init_state: Option<&RouteState>, insert_before_id: NodeId) {
        if item_name.is_empty() {
            return;
        }
        let Some(st) = init_state else { return };
        let already = st.inventory.cur_items.iter().any(|b| b.base_item.name == item_name);
        if already {
            return;
        }
        ctrl.new_event(EventDefinition::with_item(InventoryEventDefinition::new(item_name, 1, true, false, None)), None, Some(insert_before_id), None, false);
    }

    /// `update_player_held_item(new_item)`
    pub fn update_player_held_item(&mut self, cfg: &Config, ctrl: &mut MainController, new_item: &str) {
        let Some(gid) = self.summary.event_group_id else { return };
        if self.summary.held_item_update_in_flight {
            return;
        }
        let trimmed = new_item.trim();
        let normalized: Option<String> = if trimmed.is_empty() || trimmed == consts::NO_ITEM || trimmed == "None" { None } else { Some(trimmed.to_string()) };
        let current_held = self
            .summary
            .original_player_mon_list
            .first()
            .and_then(|m| m.held_item.clone())
            .filter(|c| !c.is_empty() && c != "None" && c != consts::NO_ITEM)
            .unwrap_or_default();
        if current_held == normalized.clone().unwrap_or_default() {
            return;
        }
        let (existing_hold, anchor) = self.find_existing_prefight_hold_event(ctrl);
        match existing_hold {
            Some(h) => {
                let cur = ctrl.router.group(h).and_then(|g| g.event_definition.hold_item.as_ref()).and_then(|hi| hi.item_name.clone());
                if cur == normalized {
                    return;
                }
            }
            None => {
                if normalized.is_none() {
                    return;
                }
            }
        }
        self.summary.held_item_update_in_flight = true;
        let saved = self.with_transient(
            |_bc, ctrl| match existing_hold {
                Some(h) => match &normalized {
                    None => ctrl.delete_events(&[h]),
                    Some(n) => {
                        let init = ctrl.router.init_state_of(h).cloned();
                        BattleController::maybe_insert_find_for_held_item(ctrl, n, init.as_deref(), h);
                        ctrl.update_existing_event(h, EventDefinition::with_hold_item(HoldItemEventDefinition::new(Some(n), false)));
                    }
                },
                None => {
                    let insert_target = anchor.unwrap_or(gid);
                    let init = ctrl.router.init_state_of(insert_target).cloned();
                    let n = normalized.clone().unwrap_or_default();
                    BattleController::maybe_insert_find_for_held_item(ctrl, &n, init.as_deref(), insert_target);
                    ctrl.new_event(EventDefinition::with_hold_item(HoldItemEventDefinition::new(Some(&n), false)), None, Some(insert_target), None, false);
                }
            },
            ctrl,
        );
        self.reload_after_mutation(cfg, ctrl, gid, saved);
        self.summary.held_item_update_in_flight = false;
    }

    /// `_stat_to_vitamin_name`
    fn stat_to_vitamin_name(ctrl: &MainController, stat: &str) -> Option<String> {
        let gen = ctrl.gen()?;
        for vit in gen.get_valid_vitamins() {
            if let Ok(boosted) = gen.get_stats_boosted_by_vitamin(vit) {
                if boosted.contains(&stat) {
                    return Some(vit.to_string());
                }
            }
        }
        None
    }

    /// `_find_last_vitamins(target_stat)`: (last enabled vitamin for stat, last vitamin of any type)
    fn find_last_vitamins(&self, ctrl: &MainController, target_stat: &str) -> (Option<NodeId>, Option<NodeId>) {
        let Some(gid) = self.summary.event_group_id else { return (None, None) };
        let Some(gen) = ctrl.gen() else { return (None, None) };
        let mut last_for_stat = None;
        let mut last_any = None;
        for cur in ctrl.router.all_groups() {
            if cur == gid {
                break;
            }
            let Some(g) = ctrl.router.group(cur) else { continue };
            if let Some(vit) = &g.event_definition.vitamin {
                last_any = Some(cur);
                if ctrl.router.is_enabled(cur) {
                    if let Ok(boosted) = gen.get_stats_boosted_by_vitamin(&vit.vitamin) {
                        if boosted.contains(&target_stat) {
                            last_for_stat = Some(cur);
                        }
                    }
                }
            }
        }
        (last_for_stat, last_any)
    }

    /// `adjust_vitamin_for_stat(stat, delta, _skip_refresh)`
    pub fn adjust_vitamin_for_stat(&mut self, cfg: &Config, ctrl: &mut MainController, stat: &str, delta: i64, skip_refresh: bool) {
        let Some(gid) = self.summary.event_group_id else { return };
        let Some(vit_name) = BattleController::stat_to_vitamin_name(ctrl, stat) else { return };
        let saved = self.summary.save_transient_state();
        self.summary.suppress_refresh = true;
        let shadow_info = self.summary.vitamin_shadows.get(stat).cloned();
        match shadow_info {
            Some((Some(shadow_id), original_id)) => {
                let shadow_amount = ctrl.router.group(shadow_id).and_then(|g| g.event_definition.vitamin.as_ref()).map(|v| v.amount);
                match shadow_amount {
                    Some(amount) => {
                        let new_amount = amount + delta;
                        if new_amount > 0 {
                            ctrl.update_existing_event(shadow_id, EventDefinition::with_vitamin(VitaminEventDefinition::new(&vit_name, new_amount)));
                        } else {
                            ctrl.delete_events(&[shadow_id]);
                            if let Some(orig_id) = original_id {
                                if ctrl.router.obj_kind(orig_id) == Some(ObjKind::Group) {
                                    let def = {
                                        let g = ctrl.router.node_mut(orig_id).and_then(|n| n.as_group_mut());
                                        g.map(|g| {
                                            g.set_enabled_status(true);
                                            g.event_definition.clone()
                                        })
                                    };
                                    if let Some(d) = def {
                                        ctrl.update_existing_event(orig_id, d);
                                    }
                                }
                            }
                            self.summary.vitamin_shadows.remove(stat);
                        }
                    }
                    None => {
                        self.summary.vitamin_shadows.remove(stat);
                    }
                }
            }
            _ => {
                let (last_for_stat, last_any) = self.find_last_vitamins(ctrl, stat);
                if let Some(orig) = last_for_stat {
                    let orig_amount = ctrl.router.group(orig).and_then(|g| g.event_definition.vitamin.as_ref()).map(|v| v.amount).unwrap_or(0);
                    let new_amount = orig_amount + delta;
                    let def = {
                        let g = ctrl.router.node_mut(orig).and_then(|n| n.as_group_mut());
                        g.map(|g| {
                            g.set_enabled_status(false);
                            g.event_definition.clone()
                        })
                    };
                    if let Some(d) = def {
                        ctrl.update_existing_event(orig, d);
                    }
                    if new_amount > 0 {
                        let shadow_id = ctrl.new_event(EventDefinition::with_vitamin(VitaminEventDefinition::new(&vit_name, new_amount)), Some(orig), None, None, false);
                        self.summary.vitamin_shadows.insert(stat.to_string(), (shadow_id, Some(orig)));
                    } else {
                        self.summary.vitamin_shadows.insert(stat.to_string(), (None, Some(orig)));
                    }
                } else if delta > 0 {
                    let shadow_id = match last_any {
                        Some(la) => ctrl.new_event(EventDefinition::with_vitamin(VitaminEventDefinition::new(&vit_name, delta)), Some(la), None, None, false),
                        None => ctrl.new_event(EventDefinition::with_vitamin(VitaminEventDefinition::new(&vit_name, delta)), None, Some(gid), None, false),
                    };
                    self.summary.vitamin_shadows.insert(stat.to_string(), (shadow_id, None));
                }
            }
        }
        self.summary.suppress_refresh = false;
        // the cascade's reload (keeps the shadows the Python code holds on the controller)
        let shadows = self.summary.vitamin_shadows.clone();
        let sc = BattleController::summary_config(cfg, ctrl);
        self.summary.suppress_refresh = true;
        let _ = self.summary.load_from_event(&ctrl.router, Some(gid), &sc);
        self.summary.suppress_refresh = false;
        self.summary.vitamin_shadows = shadows;
        self.summary.restore_transient_state(saved);
        self.handled_route_change = Some(gid);
        if !skip_refresh {
            self.full_refresh(cfg, ctrl);
        }
    }

    /// `reorder_matchup(from, to)`: rewrite `mon_order` through the router.
    pub fn reorder_matchup(&mut self, cfg: &Config, ctrl: &mut MainController, from_idx: usize, to_idx: usize) -> bool {
        match self.summary.reorder_matchup(&ctrl.router, from_idx, to_idx) {
            Ok(Some(new_event)) => {
                let gid = self.summary.event_group_id.unwrap_or(0);
                ctrl.update_existing_event(gid, new_event);
                let _ = cfg;
                true
            }
            Ok(None) => false,
            Err(e) => {
                ctrl.trigger_exception(format!("Failed to reorder matchup: {}", e));
                false
            }
        }
    }

    /// `take_screenshot` naming: the trainer name with spaces replaced.
    pub fn screenshot_name(&self) -> Option<String> {
        if self.summary.trainer_name.is_empty() {
            None
        } else {
            Some(self.summary.trainer_name.replace(' ', "_"))
        }
    }

    /// The vitamin shadow bookkeeping reset when a different battle is loaded.
    pub fn is_loaded_event(&self, gid: NodeId) -> bool {
        self.summary.event_group_id == Some(gid)
    }

    /// The `RareCandyEventDefinition` helper used by the legacy widget.
    pub fn candy_def(amount: i64) -> RareCandyEventDefinition {
        RareCandyEventDefinition::new(amount)
    }
}
