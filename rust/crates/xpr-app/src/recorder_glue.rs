//! The application side of the recorder: `RecorderHost` implemented over the
//! main controller (pumped on the UI thread every frame), the start-info
//! hand-off and the status-bar state (`recorder_status.py`).

use std::sync::Arc;

use xpr_core::consts;
use xpr_core::Config;
use xpr_data::model::StatBlock;
use xpr_data::GenData;
use xpr_engine::{EventDefinition, LearnMoveEventDefinition, NodeId};
use xpr_recorder::host::{HostHandle, HostQueue, PrevEvent, RecorderHost, StartInfo};
use xpr_recorder::{host_channel, RecorderController};

use crate::controller::MainController;

/// A per-frame view the recorder's queued calls run against.
pub struct HostBridge<'a> {
    pub ctrl: &'a mut MainController,
    pub cfg: &'a Config,
}

impl<'a> RecorderHost for HostBridge<'a> {
    fn is_record_mode_active(&self) -> bool {
        self.ctrl.is_record_mode_active()
    }

    fn set_record_mode(&mut self, active: bool) {
        self.ctrl.set_record_mode(active);
    }

    fn is_empty(&self) -> bool {
        self.ctrl.is_empty()
    }

    fn get_all_folder_names(&self) -> Vec<String> {
        self.ctrl.get_all_folder_names()
    }

    fn get_previous_event(&self, cur_event_id: Option<NodeId>) -> Option<PrevEvent> {
        let gid = self.ctrl.get_previous_event(cur_event_id)?;
        let g = self.ctrl.router.group(gid)?;
        let parent_name = self.ctrl.router.folder(g.parent).map(|f| f.name.clone()).unwrap_or_default();
        Some(PrevEvent {
            group_id: gid,
            parent_name,
            definition: g.event_definition.clone(),
            final_held_item: g.final_state.as_ref().and_then(|s| s.solo_pkmn.held_item.clone()),
            first_trainer_name: g.event_definition.trainer_def.as_ref().map(|t| t.trainer_name.clone()),
        })
    }

    fn delete_events(&mut self, ids: &[NodeId]) {
        self.ctrl.delete_events(ids);
    }

    fn purge_empty_folders(&mut self) {
        self.ctrl.purge_empty_folders();
    }

    fn new_event(&mut self, def: EventDefinition, dest_folder_name: &str) {
        self.ctrl.new_event(def, None, None, Some(dest_folder_name), true);
    }

    fn finalize_new_folder(&mut self, name: &str) {
        self.ctrl.finalize_new_folder(name, None, None);
    }

    fn get_defeated_trainers(&self) -> Vec<String> {
        self.ctrl.get_defeated_trainers()
    }

    fn get_move_idx(&self, move_name: &str) -> Option<i64> {
        self.ctrl.get_move_idx(move_name, None)
    }

    fn update_levelup_move(&mut self, def: LearnMoveEventDefinition) {
        self.ctrl.update_levelup_move(def);
    }

    fn update_existing_event(&mut self, id: NodeId, def: EventDefinition) {
        self.ctrl.update_existing_event(id, def);
    }

    fn get_dvs(&self) -> Option<StatBlock> {
        self.ctrl.get_dvs()
    }

    fn final_solo_species(&self) -> Option<String> {
        self.ctrl.get_final_state().map(|s| s.solo_pkmn.species_def.name.clone())
    }

    fn final_held_item(&self) -> Option<String> {
        self.ctrl.get_final_state().and_then(|s| s.solo_pkmn.held_item.clone())
    }

    fn final_inventory_has(&self, item_name: &str) -> bool {
        self.ctrl.get_final_state().map(|s| s.inventory.has_item(item_name)).unwrap_or(false)
    }

    fn can_evolve_into(&self, species: &str) -> bool {
        self.ctrl.can_evolve_into(species)
    }

    fn is_valid_levelup_move(&self, def: &LearnMoveEventDefinition) -> bool {
        self.ctrl.is_valid_levelup_move(def)
    }

    fn get_version(&self) -> Option<String> {
        self.ctrl.get_version().map(|s| s.to_string())
    }

    fn gen(&self) -> Option<Arc<GenData>> {
        self.ctrl.gen()
    }

    fn trigger_exception(&mut self, msg: &str) {
        self.ctrl.trigger_exception(msg);
    }

    fn send_message(&mut self, msg: &str) {
        self.ctrl.send_message(msg);
    }

    fn final_trainers(&self, version: &str) -> Vec<String> {
        self.cfg.get_final_trainers(version)
    }

    fn recording_auto_stop_enabled(&self) -> bool {
        self.cfg.get_recording_auto_stop_enabled()
    }

    fn is_debug_mode(&self) -> bool {
        self.cfg.is_debug_mode()
    }

    fn gamehook_url(&self) -> String {
        gamehook_url()
    }
}

/// The GameHook base URL (`XPR_GAMEHOOK_URL` overrides the default).
pub fn gamehook_url() -> String {
    std::env::var("XPR_GAMEHOOK_URL").unwrap_or_else(|_| "http://localhost:8085".to_string())
}

/// Owns the recorder controller and its UI-thread queue.
pub struct Recorder {
    pub controller: Arc<RecorderController>,
    pub queue: HostQueue,
    pub handle: HostHandle,
    /// status label text shown while recording
    pub client_status: String,
    pub reconnect_enabled: bool,
}

impl Recorder {
    pub fn new(ctx: egui::Context) -> Recorder {
        let wake_ctx = ctx.clone();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || wake_ctx.request_repaint());
        let (handle, queue) = host_channel(wake.clone());
        let controller = RecorderController::new(handle.clone(), wake);
        Recorder { controller, queue, handle, client_status: String::new(), reconnect_enabled: false }
    }

    /// `on_recording_mode_changed`: start or stop the GameHook session.
    pub fn on_recording_mode_changed(&mut self, ctrl: &MainController, cfg: &Config) {
        if ctrl.is_record_mode_active() {
            if self.controller.has_client() {
                log::warn!("Recording mode set to active, but gamehook client was already active");
                return;
            }
            self.client_status = "Client Status: Connecting...".to_string();
            let start = match (ctrl.get_version(), ctrl.gen()) {
                (Some(v), Some(gen)) => Some(StartInfo {
                    version: v.to_string(),
                    gen,
                    url: gamehook_url(),
                    debug_mode: cfg.is_debug_mode(),
                    dvs: ctrl.get_dvs(),
                    solo_species: ctrl.get_final_state().map(|s| s.solo_pkmn.species_def.name.clone()),
                }),
                _ => None,
            };
            self.controller.on_recording_mode_changed(true, start);
            if !self.controller.has_client() {
                self.client_status = "No recorder has been created yet for the current version".to_string();
                self.reconnect_enabled = false;
            }
        } else {
            self.controller.on_recording_mode_changed(false, None);
        }
    }

    /// Refresh the status-bar text from the controller snapshot.
    pub fn refresh_status(&mut self) {
        let snap = self.controller.snapshot();
        if let Some(s) = &snap.status {
            self.client_status = format!("Client Status: {}", s);
            if s == consts::RECORDING_STATUS_DISCONNECTED {
                self.reconnect_enabled = true;
            }
        }
        if let Some(ready) = snap.ready {
            self.reconnect_enabled = !ready;
        }
    }

    pub fn reconnect(&self) {
        self.controller.reconnect();
    }

    /// Run the queued host calls against the controller.
    pub fn pump(&self, ctrl: &mut MainController, cfg: &Config) -> usize {
        let mut bridge = HostBridge { ctrl, cfg };
        self.queue.pump(&mut bridge)
    }
}
