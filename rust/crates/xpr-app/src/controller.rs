//! Port of `controllers/main_controller.py`: the application state the views
//! read (route, selection, preview, filters, messages) and every operation
//! the views invoke. The Python callback bus becomes a set of flags the UI
//! drains once per frame ([`Signals`]).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use xpr_core::consts;
use xpr_core::io_utils::sanitize_string;
use xpr_core::{Config, Paths};
use xpr_data::model::{Nature, StatBlock};
use xpr_data::{GenData, Registry};
use xpr_engine::{
    EvOverrideEventDefinition, EventDefinition, InsertSpec, InventoryEventDefinition, LearnMoveEventDefinition, LevelVal, NodeId, ObjKind, Router,
    RouteState, TrainerEventDefinition, UndoManager, WildPkmnEventDefinition,
};

/// The callbacks of the Python controller, as "fired this frame" flags.
#[derive(Clone, Debug, Default)]
pub struct Signals {
    pub name_changed: bool,
    pub version_changed: bool,
    pub route_changed: bool,
    pub filter_changed: bool,
    pub event_changed: bool,
    pub selection_changed: bool,
    pub preview_changed: bool,
    pub record_mode_changed: bool,
    pub message: bool,
    pub exception: bool,
}

impl Signals {
    pub fn any(&self) -> bool {
        self.name_changed
            || self.version_changed
            || self.route_changed
            || self.filter_changed
            || self.event_changed
            || self.selection_changed
            || self.preview_changed
            || self.record_mode_changed
            || self.message
            || self.exception
    }
}

pub struct MainController {
    pub router: Router,
    pub paths: Paths,
    undo: UndoManager,
    preview_event: Option<EventDefinition>,
    route_name: String,
    selected_ids: Vec<NodeId>,
    record_mode_active: bool,
    exception_info: VecDeque<String>,
    message_info: VecDeque<String>,
    route_filter_types: Vec<String>,
    route_search: String,
    unsaved_changes: bool,
    custom_image_path: Option<String>,
    signals: Signals,
    /// The next route mutation joins the previous one as a single undo step
    /// (a burst of candy / vitamin clicks).
    coalesce_next_undo: bool,
    /// Bumped on every route or version change; the PP ledger rebuilds when
    /// it moved.
    route_revision: u64,
    /// Current PP before every event (see [`MainController::ensure_pp`]).
    pub pp: crate::pp_ledger::PpLedger,
}

impl MainController {
    pub fn new(registry: Arc<Registry>, paths: Paths) -> MainController {
        MainController {
            router: Router::new(registry),
            paths,
            undo: UndoManager::new(15),
            preview_event: None,
            route_name: String::new(),
            selected_ids: Vec::new(),
            record_mode_active: false,
            exception_info: VecDeque::new(),
            message_info: VecDeque::new(),
            route_filter_types: Vec::new(),
            route_search: String::new(),
            unsaved_changes: false,
            custom_image_path: None,
            signals: Signals::default(),
            coalesce_next_undo: false,
            route_revision: 0,
            pp: crate::pp_ledger::PpLedger::new(),
        }
    }

    /// Make the next mutation part of the previous undo step. Python
    /// batches rapid stepper clicks behind a debounce timer, so a burst was
    /// one route mutation; applying every click immediately keeps that undo
    /// granularity by not pushing a new pre-operation state for the burst.
    pub fn coalesce_next_undo_step(&mut self) {
        self.coalesce_next_undo = true;
    }

    /// The pre-operation half of the undo bookkeeping.
    fn undo_before(&mut self) {
        if self.coalesce_next_undo {
            self.coalesce_next_undo = false;
            return;
        }
        self.undo.save_state(&self.router, false);
    }

    /// The flags raised since the last call (the UI drains them each frame).
    pub fn take_signals(&mut self) -> Signals {
        std::mem::take(&mut self.signals)
    }

    pub fn get_next_exception_info(&mut self) -> Option<String> {
        self.exception_info.pop_front()
    }

    pub fn get_next_message_info(&mut self) -> Option<String> {
        self.message_info.pop_front()
    }

    // ---- event "callbacks" ----------------------------------------------------

    fn on_name_change(&mut self) {
        self.signals.name_changed = true;
    }

    fn on_version_change(&mut self) {
        self.route_revision += 1;
        self.signals.version_changed = true;
    }

    fn on_route_change(&mut self) {
        self.route_revision += 1;
        self.unsaved_changes = true;
        self.signals.route_changed = true;
    }

    /// Bumped on every route or version change.
    pub fn route_revision(&self) -> u64 {
        self.route_revision
    }

    /// Bring the PP ledger (`self.pp`) up to date; free when neither the
    /// route nor the calc settings changed since the last call. Everything
    /// that shows PP calls this first.
    pub fn ensure_pp(&mut self, cfg: &Config) {
        let sc = crate::pp_ledger::pp_summary_config(cfg);
        self.pp.ensure(&self.router, self.route_revision, &sc);
    }

    fn on_filter_change(&mut self) {
        self.signals.filter_changed = true;
    }

    fn on_event_change(&mut self) {
        self.signals.event_changed = true;
        self.on_route_change();
    }

    fn on_event_selection(&mut self) {
        self.signals.selection_changed = true;
    }

    fn on_event_preview(&mut self) {
        self.signals.preview_changed = true;
    }

    fn on_record_mode_change(&mut self) {
        self.signals.record_mode_changed = true;
    }

    fn on_info_message(&mut self, msg: String) {
        self.message_info.push_back(msg);
        self.signals.message = true;
    }

    fn on_exception(&mut self, msg: String) {
        log::error!("Application error: {}", msg);
        self.exception_info.push_back(msg);
        self.signals.exception = true;
    }

    /// `@handle_exceptions`: log and surface an error instead of raising.
    fn report<T>(&mut self, result: Result<T, String>, what: &str) -> Option<T> {
        match result {
            Ok(v) => Some(v),
            Err(e) => {
                log::error!("Trying to run function: {}, got error: {}", what, e);
                self.on_exception(format!("<class 'Exception'>: {}", e));
                None
            }
        }
    }

    // ---- data access ------------------------------------------------------------

    pub fn gen(&self) -> Option<Arc<GenData>> {
        self.router.gen().cloned()
    }

    pub fn get_raw_route(&self) -> &Router {
        &self.router
    }

    pub fn get_raw_route_mut(&mut self) -> &mut Router {
        &mut self.router
    }

    pub fn get_current_route_name(&self) -> &str {
        &self.route_name
    }

    pub fn set_custom_image_path(&mut self, path: &str) {
        let cleaned = path.trim().trim_matches('"').trim_matches('\'').trim_matches('"').trim_matches('\'');
        self.custom_image_path = if cleaned.is_empty() { None } else { Some(cleaned.to_string()) };
    }

    pub fn get_custom_image_path(&self) -> Option<&str> {
        self.custom_image_path.as_deref()
    }

    pub fn get_preview_event(&self) -> Option<&EventDefinition> {
        self.preview_event.as_ref()
    }

    pub fn obj_kind(&self, id: NodeId) -> Option<ObjKind> {
        self.router.obj_kind(id)
    }

    pub fn has_errors(&self) -> bool {
        self.router.root().has_errors()
    }

    pub fn get_version(&self) -> Option<&str> {
        self.router.pkmn_version.as_deref()
    }

    pub fn get_state_after(&self, previous_event_id: Option<NodeId>) -> Option<Arc<RouteState>> {
        match previous_event_id {
            None => self.router.init_route_state.clone(),
            Some(id) => match self.router.init_state_of(id) {
                Some(s) => Some(s.clone()),
                None => self.router.init_route_state.clone(),
            },
        }
    }

    pub fn get_init_state(&self) -> Option<Arc<RouteState>> {
        self.router.init_route_state.clone()
    }

    pub fn get_final_state(&self) -> Option<Arc<RouteState>> {
        self.router.get_final_state().cloned()
    }

    pub fn get_all_folder_names(&self) -> Vec<String> {
        self.router.folder_lookup.keys().cloned().collect()
    }

    pub fn get_invalid_folders(&self, event_id: NodeId) -> Vec<String> {
        self.router.get_invalid_folder_transfers(event_id)
    }

    pub fn get_dvs(&self) -> Option<StatBlock> {
        self.router.init_route_state.as_ref().map(|s| s.solo_pkmn.dvs.clone())
    }

    pub fn get_ability_idx(&self) -> i64 {
        self.router.init_route_state.as_ref().map(|s| s.solo_pkmn.ability_idx).unwrap_or(0)
    }

    pub fn get_nature(&self) -> Nature {
        self.router.init_route_state.as_ref().map(|s| s.solo_pkmn.nature).unwrap_or(Nature::HARDY)
    }

    pub fn get_defeated_trainers(&self) -> Vec<String> {
        self.router.get_effective_defeated_trainers()
    }

    pub fn get_route_search_string(&self) -> Option<&str> {
        if self.route_search.is_empty() {
            None
        } else {
            Some(&self.route_search)
        }
    }

    pub fn get_route_filter_types(&self) -> Option<&[String]> {
        if self.route_filter_types.is_empty() {
            None
        } else {
            Some(&self.route_filter_types)
        }
    }

    pub fn is_empty(&self) -> bool {
        self.router.root().children.is_empty()
    }

    pub fn is_valid_levelup_move(&self, def: &LearnMoveEventDefinition) -> bool {
        self.router.is_valid_levelup_move(def)
    }

    pub fn can_evolve_into(&self, species_name: &str) -> bool {
        let Some(gen) = self.gen() else { return false };
        let Some(target) = gen.pkmn_db().get_pkmn(species_name) else { return false };
        match self.get_final_state() {
            Some(fs) => target.growth_rate == fs.solo_pkmn.species_def.growth_rate,
            None => false,
        }
    }

    /// `find_first_event_by_trainer_name`
    pub fn find_first_event_by_trainer_name(&self, trainer_name: &str) -> Option<NodeId> {
        for gid in self.router.all_groups() {
            if let Some(g) = self.router.group(gid) {
                if let Some(td) = &g.event_definition.trainer_def {
                    if td.trainer_name == trainer_name {
                        return Some(gid);
                    }
                }
            }
        }
        None
    }

    /// `get_all_invalid_event_ids`
    pub fn get_all_invalid_event_ids(&self) -> Vec<NodeId> {
        self.router
            .all_groups()
            .into_iter()
            .filter(|gid| self.router.group(*gid).map(|g| g.has_errors()).unwrap_or(false))
            .collect()
    }

    pub fn has_unsaved_changes(&self) -> bool {
        self.unsaved_changes
    }

    pub fn can_undo(&self) -> bool {
        self.undo.can_undo()
    }

    pub fn get_all_selected_ids(&self, allow_event_items: bool) -> Vec<NodeId> {
        if allow_event_items {
            return self.selected_ids.clone();
        }
        self.selected_ids
            .iter()
            .copied()
            .filter(|id| self.router.obj_kind(*id) != Some(ObjKind::Item))
            .collect()
    }

    pub fn get_single_selected_event_id(&self, allow_event_items: bool) -> Option<NodeId> {
        if self.selected_ids.len() != 1 {
            return None;
        }
        let id = self.selected_ids[0];
        if !allow_event_items && self.router.obj_kind(id) == Some(ObjKind::Item) {
            return None;
        }
        Some(id)
    }

    /// `get_active_state`: the state to operate on (after the selection).
    pub fn get_active_state(&self) -> Option<Arc<RouteState>> {
        if let Some(mut id) = self.get_single_selected_event_id(true) {
            if self.router.obj_kind(id) == Some(ObjKind::Item) {
                id = self.router.item(id).map(|i| i.parent).unwrap_or(id);
            }
            return self.router.final_state_of(id).cloned();
        }
        if self.is_empty() {
            return self.router.init_route_state.clone();
        }
        self.get_final_state()
    }

    pub fn can_insert_after_current_selection(&self) -> bool {
        if self.is_empty() {
            return true;
        }
        self.get_single_selected_event_id(true).is_some()
    }

    pub fn is_record_mode_active(&self) -> bool {
        self.record_mode_active
    }

    /// `get_move_idx(move_name, state)`
    pub fn get_move_idx(&self, move_name: &str, state: Option<&RouteState>) -> Option<i64> {
        let owned;
        let state = match state {
            Some(s) => s,
            None => {
                owned = self.get_final_state()?;
                &owned
            }
        };
        let target = sanitize_string(move_name);
        for (idx, m) in state.solo_pkmn.move_list.iter().enumerate() {
            let cur = m.as_deref().unwrap_or("None");
            if sanitize_string(cur) == target {
                return Some(idx as i64);
            }
        }
        None
    }

    fn walk_events_helper(&self, folder_id: NodeId, cur_event_id: Option<NodeId>, mut found: bool, forward: bool) -> (bool, Option<NodeId>) {
        let mut children = self.router.children_of(folder_id);
        if !forward {
            children.reverse();
        }
        for child in children {
            match self.router.obj_kind(child) {
                Some(ObjKind::Group) => {
                    if found && self.router.is_enabled(child) {
                        return (found, Some(child));
                    } else if Some(child) == cur_event_id {
                        found = true;
                    }
                }
                Some(ObjKind::Folder) => {
                    let (f, r) = self.walk_events_helper(child, cur_event_id, found, forward);
                    found = f;
                    if found && r.is_some() {
                        return (found, r);
                    }
                }
                _ => log::error!("Encountered unexpected types walking events"),
            }
        }
        (found, None)
    }

    /// `get_next_event(cur_event_id)`: the next enabled group after `cur`
    /// (the first one when `cur` is `None`).
    pub fn get_next_event(&self, cur_event_id: Option<NodeId>) -> Option<NodeId> {
        self.walk_events_helper(self.router.root_id, cur_event_id, cur_event_id.is_none(), true).1
    }

    /// `get_previous_event(cur_event_id)`
    pub fn get_previous_event(&self, cur_event_id: Option<NodeId>) -> Option<NodeId> {
        self.walk_events_helper(self.router.root_id, cur_event_id, cur_event_id.is_none(), false).1
    }

    // ---- state changes ------------------------------------------------------------

    pub fn select_new_events(&mut self, all_event_ids: Vec<NodeId>) {
        self.selected_ids = all_event_ids;
        if !self.selected_ids.is_empty() {
            self.preview_event = None;
        }
        self.on_event_selection();
        if !self.selected_ids.is_empty() {
            self.on_event_preview();
        }
    }

    pub fn set_preview_trainer(&mut self, trainer_name: &str) {
        if let Some(p) = &self.preview_event {
            if p.trainer_def.as_ref().map(|t| t.trainer_name.as_str()) == Some(trainer_name) {
                return;
            }
        }
        let exists = self.gen().map(|g| g.trainer_db().get_trainer(trainer_name).is_some()).unwrap_or(false);
        self.preview_event = if exists {
            Some(EventDefinition::with_trainer(TrainerEventDefinition::new(trainer_name)))
        } else {
            None
        };
        self.on_event_preview();
    }

    pub fn update_existing_event(&mut self, event_group_id: NodeId, new_event: EventDefinition) {
        if let Some(lm) = &new_event.learn_move {
            if lm.source == consts::MOVE_SOURCE_LEVELUP {
                let lm = lm.clone();
                self.update_levelup_move(lm);
                return;
            }
        }
        self.undo_before();
        let r = self.router.replace_event_group(event_group_id, new_event);
        self.undo.save_state(&self.router, true);
        if self.report(r, "update_existing_event").is_some() {
            self.on_event_change();
        }
    }

    pub fn update_levelup_move(&mut self, new_def: LearnMoveEventDefinition) {
        self.undo_before();
        let r = self.router.replace_levelup_move_event(new_def);
        self.undo.save_state(&self.router, true);
        if self.report(r, "update_levelup_move").is_some() {
            self.on_event_change();
        }
    }

    pub fn add_area(&mut self, area_name: &str, include_rematches: bool, insert_after_id: Option<NodeId>) {
        let r = self.router.add_area(area_name, insert_after_id, consts::ROOT_FOLDER_NAME, include_rematches);
        if self.report(r, "add_area").is_some() {
            self.on_route_change();
        }
    }

    fn after_route_reset(&mut self) {
        self.undo.clear();
        self.coalesce_next_undo = false;
        if self.router.init_route_state.is_some() {
            self.undo.save_state(&self.router, false);
        }
        self.on_name_change();
        self.on_version_change();
        self.on_event_selection();
        self.on_route_change();
    }

    pub fn create_new_route_from_current(&mut self) {
        let Some(init) = self.router.init_route_state.clone() else { return };
        let solo_mon = init.solo_pkmn.name.clone();
        let dvs = init.solo_pkmn.dvs.clone();
        let ability = init.solo_pkmn.ability_idx;
        let nature = init.solo_pkmn.nature;
        let Some(version) = self.router.pkmn_version.clone() else { return };
        self.route_name.clear();
        self.selected_ids.clear();
        let r = self.router.new_route(&solo_mon, None, &version, Some(dvs), Some(ability), Some(nature));
        self.report(r, "create_new_route_from_current");
        self.after_route_reset();
    }

    pub fn create_new_route(
        &mut self,
        solo_mon: &str,
        base_route_path: Option<&Path>,
        pkmn_version: &str,
        custom_dvs: Option<StatBlock>,
        custom_ability_idx: Option<i64>,
        custom_nature: Option<Nature>,
    ) {
        let base = base_route_path.filter(|p| p.to_string_lossy() != consts::EMPTY_ROUTE_NAME);
        self.route_name.clear();
        self.selected_ids.clear();
        let r = self.router.new_route(solo_mon, base, pkmn_version, custom_dvs, custom_ability_idx, custom_nature);
        if let Err(e) = &r {
            log::error!("Exception ocurred trying to copy route: {:?}: {}", base_route_path, e);
            let fallback = self.router.new_route("Abra", None, pkmn_version, None, None, None);
            if let Err(e2) = fallback {
                log::error!("Could not even load the fallback route: {}", e2);
            }
        }
        self.report(r, "create_new_route");
        self.after_route_reset();
    }

    pub fn load_route(&mut self, full_path: &Path) {
        let route_name = full_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        self.route_name = route_name;
        let r = self.router.load(full_path, false);
        self.selected_ids.clear();
        if let Err(e) = &r {
            log::error!("Exception ocurred trying to load route: {}: {}", full_path.display(), e);
            self.route_name.clear();
            let version = self.router.pkmn_version.clone().unwrap_or_else(|| consts::YELLOW_VERSION.to_string());
            if let Err(e2) = self.router.new_route("Abra", None, &version, None, None, None) {
                log::error!("Could not load the fallback route: {}", e2);
            }
        }
        self.report(r, "load_route");
        self.after_route_reset();
        self.unsaved_changes = false;
    }

    pub fn customize_innate_stats(&mut self, new_dvs: StatBlock, new_ability: i64, new_nature: Nature) {
        let r = self.router.change_current_innate_stats(new_dvs, new_ability, new_nature);
        if self.report(r, "customize_innate_stats").is_some() {
            self.on_route_change();
        }
    }

    fn undoable<F: FnOnce(&mut Router) -> Result<(), String>>(&mut self, what: &str, f: F) -> bool {
        self.undo_before();
        let r = f(&mut self.router);
        self.undo.save_state(&self.router, true);
        self.report(r, what).is_some()
    }

    pub fn move_groups_up(&mut self, event_ids: &[NodeId]) {
        let ids = event_ids.to_vec();
        if self.undoable("move_groups_up", |r| {
            for id in ids {
                r.move_event_object(id, true)?;
            }
            Ok(())
        }) {
            self.on_route_change();
        }
    }

    pub fn move_groups_down(&mut self, event_ids: &[NodeId]) {
        let ids = event_ids.to_vec();
        if self.undoable("move_groups_down", |r| {
            for id in ids {
                r.move_event_object(id, false)?;
            }
            Ok(())
        }) {
            self.on_route_change();
        }
    }

    pub fn move_event_to_position(&mut self, event_id: NodeId, dest_folder_id: NodeId, after: Option<NodeId>, before: Option<NodeId>) {
        if self.undoable("move_event_to_position", |r| r.move_event_to_position(event_id, dest_folder_id, after, before)) {
            self.on_route_change();
        }
    }

    /// `move_events_to_position` (drag-and-drop): keeps the relative order.
    pub fn move_events_to_position(&mut self, event_ids: &[NodeId], dest_folder_id: NodeId, after: Option<NodeId>, before: Option<NodeId>) {
        let ids = event_ids.to_vec();
        if self.undoable("move_events_to_position", |r| {
            let mut cur_after = after;
            let mut cur_before = before;
            for id in ids {
                r.move_event_to_position(id, dest_folder_id, cur_after, cur_before)?;
                cur_after = Some(id);
                cur_before = None;
            }
            Ok(())
        }) {
            self.on_route_change();
        }
    }

    pub fn delete_events(&mut self, event_ids: &[NodeId]) {
        let ids = event_ids.to_vec();
        let ok = self.undoable("delete_events", |r| r.batch_remove_events(&ids));
        let mut selection_changed = false;
        for id in event_ids {
            if let Some(pos) = self.selected_ids.iter().position(|x| x == id) {
                self.selected_ids.remove(pos);
                selection_changed = true;
            }
        }
        if ok {
            self.on_route_change();
            if selection_changed {
                self.on_event_selection();
            }
        }
    }

    pub fn purge_empty_folders(&mut self) {
        loop {
            let mut deleted: Vec<NodeId> = Vec::new();
            for (name, id) in self.router.folder_lookup.iter() {
                if name == consts::ROOT_FOLDER_NAME {
                    continue;
                }
                if self.router.children_of(*id).is_empty() {
                    deleted.push(*id);
                }
            }
            if deleted.is_empty() {
                break;
            }
            self.delete_events(&deleted);
        }
    }

    pub fn transfer_to_folder(&mut self, event_ids: &[NodeId], new_folder_name: &str) {
        let ids = event_ids.to_vec();
        let name = new_folder_name.to_string();
        if self.undoable("transfer_to_folder", |r| r.transfer_events(&ids, &name)) {
            self.on_route_change();
        }
    }

    /// `split_folder_at_current_event`
    pub fn split_folder_at_current_event(&mut self, event_id: NodeId) {
        let kind = match self.router.obj_kind(event_id) {
            Some(k) => k,
            None => return,
        };
        if kind == ObjKind::Folder {
            return;
        }
        let Some(parent_id) = self.router.parent_of(event_id) else { return };
        let Some(parent) = self.router.folder(parent_id) else { return };
        if parent.name == consts::ROOT_FOLDER_NAME {
            return;
        }
        let Some(index) = parent.children.iter().position(|c| *c == event_id) else { return };
        let events_to_move: Vec<NodeId> = parent.children[index..].to_vec();
        if events_to_move.is_empty() {
            return;
        }
        let old_folder_name = parent.name.clone();
        let grand_parent_name = parent
            .parent
            .and_then(|gp| self.router.folder(gp))
            .map(|f| f.name.clone())
            .unwrap_or_else(|| consts::ROOT_FOLDER_NAME.to_string());
        let new_folder_name = self.generate_unique_folder_name(&old_folder_name);
        self.undo_before();
        let spec = InsertSpec { insert_after: Some(parent_id), dest_folder_name: Some(grand_parent_name), ..Default::default() };
        // `add_event_object(new_folder_name=...)` builds an `EventFolder` with its
        // defaults (`expanded=True, enabled=True`); `None` here would be a *disabled*
        // (and collapsed) folder, which also hides every event in it from
        // `get_previous_event` and the route state
        let r = self
            .router
            .add_event_object(None, Some(&new_folder_name), spec, false, Some(true), Some(true))
            .and_then(|new_id| self.router.transfer_events(&events_to_move, &new_folder_name).map(|_| new_id));
        self.undo.save_state(&self.router, true);
        if let Some(new_folder_id) = self.report(r, "split_folder_at_current_event") {
            self.on_route_change();
            self.select_new_events(vec![new_folder_id]);
        }
    }

    fn generate_unique_folder_name(&self, base_name: &str) -> String {
        let all: std::collections::HashSet<String> = self.get_all_folder_names().into_iter().collect();
        for v in [format!("{} (Part 2)", base_name), format!("{} (2)", base_name), format!("{} - Split", base_name)] {
            if !all.contains(&v) {
                return v;
            }
        }
        let mut counter = 2;
        loop {
            let candidate = format!("{} (Part {})", base_name, counter);
            if !all.contains(&candidate) {
                return candidate;
            }
            counter += 1;
        }
    }

    /// `new_event(event_def, insert_after, insert_before, dest_folder_name, do_select)`
    pub fn new_event(&mut self, event_def: EventDefinition, insert_after: Option<NodeId>, insert_before: Option<NodeId>, dest_folder_name: Option<&str>, do_select: bool) -> Option<NodeId> {
        let spec = InsertSpec {
            insert_after,
            insert_before,
            dest_folder_name: Some(dest_folder_name.unwrap_or(consts::ROOT_FOLDER_NAME).to_string()),
        };
        self.undo_before();
        let r = self.router.add_event_object(Some(event_def), None, spec, true, None, None);
        self.undo.save_state(&self.router, true);
        let result = self.report(r, "new_event")?;
        self.on_route_change();
        if do_select {
            self.select_new_events(vec![result]);
        }
        Some(result)
    }

    // ---- "add to route" from the world map (docs/rust_port/design/world_map/SPEC.md §3.7) ----

    /// A folder name that does not exist yet ("Route 3", "Route 3 Trip:2", ...).
    pub fn unique_folder_name(&self, base: &str) -> String {
        let all: std::collections::HashSet<String> = self.get_all_folder_names().into_iter().collect();
        let base = if base.trim().is_empty() { "New Folder" } else { base };
        let mut name = base.to_string();
        let mut count = 1;
        while all.contains(&name) {
            count += 1;
            name = format!("{} Trip:{}", base, count);
        }
        name
    }

    fn trainer_fight_def(&self, gen: &Arc<GenData>, name: &str) -> Option<EventDefinition> {
        let trainer = gen.trainer_db().get_trainer(name)?.clone();
        let mut temp = EventDefinition::with_trainer(TrainerEventDefinition::new(&trainer.name));
        if trainer.double_battle {
            let n = temp.pokemon_list(gen).map(|l| l.len()).unwrap_or(0);
            if let Some(t) = temp.trainer_def.as_mut() {
                t.exp_split = vec![2; n];
            }
        }
        Some(temp)
    }

    /// A trainer fight for a map marker, inserted after the selection — or,
    /// when a folder is selected, into a new folder named after the
    /// trainer's location (the quick-add popover's rule).
    pub fn add_trainer_fight_from_map(&mut self, name: &str) -> Option<NodeId> {
        let gen = self.gen()?;
        let def = self.trainer_fight_def(&gen, name)?;
        let location = gen.trainer_db().get_trainer(name).map(|t| t.location.clone()).unwrap_or_default();
        let selected_id = self.get_single_selected_event_id(true);
        let is_folder = selected_id.map(|id| self.router.obj_kind(id) == Some(ObjKind::Folder)).unwrap_or(false);
        if is_folder {
            let folder_name = self.unique_folder_name(&location);
            self.finalize_new_folder(&folder_name, None, selected_id);
            self.new_event(def, None, None, Some(&folder_name), true)
        } else {
            self.new_event(def, selected_id, None, None, true)
        }
    }

    /// An item pickup ("Get Free Item") after the selection.
    pub fn add_item_pickup_from_map(&mut self, name: &str) -> Option<NodeId> {
        let gen = self.gen()?;
        let item = gen.item_db().get_item(name)?.clone();
        let after = self.get_single_selected_event_id(true);
        self.new_event(EventDefinition::with_item(InventoryEventDefinition::new(&item.name, 1, true, false, None)), after, None, None, true)
    }

    /// A wild encounter after the selection.
    pub fn add_wild_from_map(&mut self, species: &str, level: i64) -> Option<NodeId> {
        let gen = self.gen()?;
        let mon = gen.pkmn_db().get_pkmn(species)?.clone();
        let after = self.get_single_selected_event_id(true);
        self.new_event(EventDefinition::with_wild(WildPkmnEventDefinition::new(&mon.name, level, 1, false)), after, None, None, true)
    }

    /// Every trainer of a map, in order, into a new folder after the
    /// selection. Returns the last event added.
    pub fn add_trainers_in_new_folder(&mut self, folder_base: &str, names: &[String]) -> Option<NodeId> {
        let gen = self.gen()?;
        let defs: Vec<EventDefinition> = names.iter().filter_map(|n| self.trainer_fight_def(&gen, n)).collect();
        if defs.is_empty() {
            return None;
        }
        let folder = self.unique_folder_name(folder_base);
        let after = self.get_single_selected_event_id(true);
        self.finalize_new_folder(&folder, None, after);
        let mut last = None;
        for def in defs {
            last = self.new_event(def, None, None, Some(&folder), false);
        }
        if let Some(id) = last {
            self.select_new_events(vec![id]);
        }
        last
    }

    /// Pre-Event State's "click a move to replace it": insert a Tutor move
    /// event for slot `slot_idx` immediately before the currently selected
    /// event, so that event's moveset changes without touching the event
    /// itself.
    pub fn assign_move_via_tutor_before_selected(&mut self, slot_idx: i64, move_name: &str) -> Option<NodeId> {
        if move_name.is_empty() || !(0..=3).contains(&slot_idx) {
            return None;
        }
        let id = self.get_single_selected_event_id(true)?;
        if !matches!(self.router.obj_kind(id), Some(ObjKind::Group) | Some(ObjKind::Item)) {
            return None;
        }
        self.new_event(
            EventDefinition::with_learn_move(LearnMoveEventDefinition::new(Some(move_name), Some(slot_idx), consts::MOVE_SOURCE_TUTOR, LevelVal::any(), None, true)),
            None,
            Some(id),
            None,
            false,
        )
    }

    /// Insert a use of the PP item `item_name` (on `target_move` when it
    /// acts on one move) before the selected event (the Pre-Event State
    /// moves card's menu).
    pub fn use_pp_item_before_selected(&mut self, item_name: &str, target_move: Option<&str>) -> Option<NodeId> {
        let id = self.get_single_selected_event_id(true)?;
        if !matches!(self.router.obj_kind(id), Some(ObjKind::Group) | Some(ObjKind::Item)) {
            return None;
        }
        self.new_event(EventDefinition::with_item(InventoryEventDefinition::use_on(item_name, 1, target_move)), None, Some(id), None, false)
    }

    /// Insert an EV Override event (`[hp, atk, def, spa, spd, spe]`) before
    /// the selected event (the Pre-Event State stats card's EV column).
    pub fn override_evs_before_selected(&mut self, values: [i64; 6]) -> Option<NodeId> {
        let id = self.get_single_selected_event_id(true)?;
        if !matches!(self.router.obj_kind(id), Some(ObjKind::Group) | Some(ObjKind::Item)) {
            return None;
        }
        let [hp, atk, def, spa, spd, spe] = values;
        self.new_event(EventDefinition::with_ev_override(EvOverrideEventDefinition::new(hp, atk, def, spa, spd, spe)), None, Some(id), None, false)
    }

    /// `finalize_new_folder(new_folder_name, prev_folder_name, insert_after)`
    pub fn finalize_new_folder(&mut self, new_folder_name: &str, prev_folder_name: Option<&str>, insert_after: Option<NodeId>) {
        let name = new_folder_name.to_string();
        let prev = prev_folder_name.map(|s| s.to_string());
        // new folders are enabled and expanded (the `EventFolder` defaults)
        if self.undoable("finalize_new_folder", |r| match (prev, insert_after) {
            (None, None) => r.add_event_object(None, Some(&name), InsertSpec::default(), true, Some(true), Some(true)).map(|_| ()),
            (None, Some(after)) => r
                .add_event_object(None, Some(&name), InsertSpec { insert_after: Some(after), ..Default::default() }, true, Some(true), Some(true))
                .map(|_| ()),
            (Some(p), _) => r.rename_event_folder(&p, &name),
        }) {
            self.on_route_change();
        }
    }

    pub fn toggle_event_highlight(&mut self, event_ids: &[NodeId]) {
        let mut ok = true;
        for id in event_ids {
            let r = self.router.toggle_event_highlight(*id);
            if self.report(r, "toggle_event_highlight").is_none() {
                ok = false;
                break;
            }
        }
        if ok {
            self.on_route_change();
        }
    }

    pub fn set_event_highlight(&mut self, event_ids: &[NodeId], highlight_num: Option<i64>) {
        let mut ok = true;
        for id in event_ids {
            let r = self.router.set_event_highlight(*id, highlight_num);
            if self.report(r, "set_event_highlight").is_none() {
                ok = false;
                break;
            }
        }
        if ok {
            self.on_route_change();
        }
    }

    /// Enable / disable several events as a single undo step.
    pub fn set_events_enabled(&mut self, event_ids: &[NodeId], enabled: bool) {
        let ids = event_ids.to_vec();
        if self.undoable("set_events_enabled", |r| r.set_events_enabled(&ids, enabled)) {
            self.on_event_change();
        }
    }

    pub fn set_record_mode(&mut self, new_record_mode: bool) {
        self.record_mode_active = new_record_mode;
        self.on_record_mode_change();
    }

    pub fn move_groups_to_adjacent_folder_up(&mut self, event_ids: &[NodeId]) {
        let ids = event_ids.to_vec();
        if self.undoable("move_groups_to_adjacent_folder_up", |r| {
            for id in ids {
                r.move_event_to_adjacent_folder(id, true)?;
            }
            Ok(())
        }) {
            self.on_route_change();
        }
    }

    pub fn move_groups_to_adjacent_folder_down(&mut self, event_ids: &[NodeId]) {
        let ids = event_ids.to_vec();
        if self.undoable("move_groups_to_adjacent_folder_down", |r| {
            for id in ids {
                r.move_event_to_adjacent_folder(id, false)?;
            }
            Ok(())
        }) {
            self.on_route_change();
        }
    }

    pub fn set_route_filter_types(&mut self, filter_options: Vec<String>) {
        self.route_filter_types = filter_options;
        self.on_filter_change();
    }

    pub fn set_route_search(&mut self, search: &str) {
        self.route_search = search.to_string();
        self.on_filter_change();
    }

    pub fn load_all_custom_versions(&mut self) {
        let r = self.router.registry().reload_all_custom_gens();
        self.on_custom_versions_loaded(r);
    }

    /// The outcome of a `reload_all_custom_gens` run elsewhere (the start-up
    /// background load).
    pub fn on_custom_versions_loaded(&mut self, r: Result<(), String>) {
        self.report(r, "load_all_custom_versions");
    }

    pub fn create_custom_version(&mut self, base_version: &str, custom_version: &str) {
        let r = self.router.registry().create_custom_version(base_version, custom_version).map(|_| ());
        self.report(r, "create_custom_version");
    }

    pub fn send_message(&mut self, message: impl Into<String>) {
        self.on_info_message(message.into());
    }

    pub fn trigger_exception(&mut self, message: impl Into<String>) {
        self.on_exception(message.into());
    }

    pub fn set_current_route_name(&mut self, new_name: &str) {
        self.route_name = new_name.to_string();
        self.on_name_change();
    }

    /// `MainWindow.close_route`'s reset (the confirmation lives in the UI).
    pub fn close_route(&mut self) {
        self.router.close_route();
        self.route_name.clear();
        self.selected_ids.clear();
        self.unsaved_changes = false;
        self.on_name_change();
        self.on_version_change();
        self.on_event_selection();
        self.on_route_change();
    }

    pub fn undo(&mut self) {
        if !self.can_undo() {
            return;
        }
        let Some(previous) = self.undo.get_undo_state() else { return };
        let r = self.router.restore_events_from_state(&previous);
        self.undo.set_current_state(previous);
        self.report(r, "undo");
        self.selected_ids.clear();
        self.on_event_selection();
        self.on_route_change();
    }

    pub fn save_route(&mut self, route_name: &str) {
        match self.router.save(&self.paths, route_name) {
            Ok(_) => {
                self.send_message(format!("Successfully saved route: {}", route_name));
                self.unsaved_changes = false;
            }
            Err(e) => self.trigger_exception(format!("Couldn't save route due to exception! <class 'Exception'>: {}", e)),
        }
    }

    pub fn export_notes(&mut self, route_name: &str) {
        match self.router.export_notes(&self.paths, route_name) {
            Ok(p) => self.send_message(format!("Exported notes to: {}", p.display())),
            Err(e) => self.trigger_exception(format!("Couldn't export notes: {}", e)),
        }
    }

    /// The directory screenshots go to: the custom image path when it is (or
    /// can be made) a directory, else the configured images dir.
    pub fn screenshot_dir(&self, cfg: &Config, custom_path: Option<&str>) -> PathBuf {
        let path_to_use = custom_path.map(|s| s.to_string()).or_else(|| self.custom_image_path.clone());
        if let Some(p) = path_to_use {
            let cleaned = p.trim().trim_matches('"').trim_matches('\'');
            if !cleaned.is_empty() {
                let pb = PathBuf::from(cleaned);
                if pb.is_dir() {
                    return pb;
                }
                if std::fs::create_dir_all(&pb).is_ok() && pb.is_dir() {
                    return pb;
                }
            }
        }
        cfg.get_images_dir()
    }

    /// `take_screenshot`'s naming: `<timestamp>-<route>_<image_name>.png`
    /// (no collision) in `dir`.
    pub fn screenshot_path(&self, dir: &Path, image_name: &str) -> PathBuf {
        let date_prefix = chrono::Local::now().format("%Y%m%d%H%M%S").to_string();
        xpr_core::io_utils::get_safe_path_no_collision(dir, &format!("{}-{}_{}", date_prefix, self.route_name, image_name), ".png")
    }
}
