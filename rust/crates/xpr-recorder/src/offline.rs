//! Driving a recorder session without GameHook. `replay_to_route` reads the
//! property changes out of a replay and hands them over here batch by batch,
//! on the replay's clock (see `crate::clock`); everything after that is what
//! the GameHook client does with a `PropertiesChanged` message.

use std::collections::HashMap;

use serde_json::Value;

use crate::gamehook::{apply_property_change, positional_change, register_watched, PropertyStore, SessionEvents};

pub struct OfflineClient<S: SessionEvents> {
    store: PropertyStore,
    watched: HashMap<String, usize>,
    session: S,
}

impl<S: SessionEvents> OfflineClient<S> {
    /// Connect `session` to a mapper (`GET /mapper`'s JSON, with the values
    /// the game has at this point).
    pub fn new(session: S, mapper: &Value) -> Result<Self, String> {
        Ok(Self::from_store(session, PropertyStore::from_mapper(mapper)?))
    }

    /// Connect `session` to a mapper whose current values `store` holds (a
    /// new client loading the mapper while the game runs).
    pub fn from_store(mut session: S, store: PropertyStore) -> Self {
        session.on_connected();
        log::info!("[Offline Client] Mapper loaded: {}", store.game_name().unwrap_or("?"));
        let watched = register_watched(&mut session, &store);
        OfflineClient { store, watched, session }
    }

    /// One `PropertiesChanged` batch (Poke-A-Byte's items, in mapper order),
    /// followed by `on_idle` as the client's read loop does.
    pub fn apply_batch(&mut self, changes: &[Value]) {
        for change in changes {
            let positional = positional_change(change);
            if let Err(e) = apply_property_change(&mut self.store, &self.watched, &mut self.session, &positional) {
                log::error!("Exception generated handling property change: {}", e);
                self.session.on_game_hook_error(&e);
            }
        }
        self.idle();
    }

    /// A pass of the read loop with nothing new (the hub was quiet).
    pub fn idle(&mut self) {
        if self.store.is_loaded() {
            self.session.on_idle(&self.store);
        }
    }

    pub fn store(&self) -> &PropertyStore {
        &self.store
    }

    pub fn session(&self) -> &S {
        &self.session
    }

    pub fn session_mut(&mut self) -> &mut S {
        &mut self.session
    }

    /// Disconnect: the session shuts down; the values stay readable.
    pub fn shutdown(mut self) -> PropertyStore {
        self.session.on_shutdown();
        self.store
    }
}

/// A host with no route: answers every question with "nothing" (for asking a
/// recorder what it watches without recording anything).
struct NullHost;

impl crate::host::RecorderHost for NullHost {
    fn is_record_mode_active(&self) -> bool {
        false
    }
    fn set_record_mode(&mut self, _active: bool) {}
    fn is_empty(&self) -> bool {
        true
    }
    fn get_all_folder_names(&self) -> Vec<String> {
        Vec::new()
    }
    fn get_previous_event(&self, _cur_event_id: Option<xpr_engine::NodeId>) -> Option<crate::host::PrevEvent> {
        None
    }
    fn delete_events(&mut self, _ids: &[xpr_engine::NodeId]) {}
    fn purge_empty_folders(&mut self) {}
    fn new_event(&mut self, _def: xpr_engine::EventDefinition, _dest_folder_name: &str) {}
    fn finalize_new_folder(&mut self, _name: &str) {}
    fn get_defeated_trainers(&self) -> Vec<String> {
        Vec::new()
    }
    fn get_move_idx(&self, _move_name: &str) -> Option<i64> {
        None
    }
    fn update_levelup_move(&mut self, _def: xpr_engine::LearnMoveEventDefinition) {}
    fn update_existing_event(&mut self, _id: xpr_engine::NodeId, _def: xpr_engine::EventDefinition) {}
    fn get_dvs(&self) -> Option<xpr_data::model::StatBlock> {
        None
    }
    fn final_solo_species(&self) -> Option<String> {
        None
    }
    fn final_held_item(&self) -> Option<String> {
        None
    }
    fn final_inventory_has(&self, _item_name: &str) -> bool {
        false
    }
    fn can_evolve_into(&self, _species: &str) -> bool {
        false
    }
    fn is_valid_levelup_move(&self, _def: &xpr_engine::LearnMoveEventDefinition) -> bool {
        false
    }
    fn get_version(&self) -> Option<String> {
        None
    }
    fn gen(&self) -> Option<std::sync::Arc<xpr_data::GenData>> {
        None
    }
    fn trigger_exception(&mut self, _msg: &str) {}
    fn send_message(&mut self, _msg: &str) {}
    fn final_trainers(&self, _version: &str) -> Vec<String> {
        Vec::new()
    }
    fn recording_auto_stop_enabled(&self) -> bool {
        false
    }
    fn is_debug_mode(&self) -> bool {
        false
    }
    fn gamehook_url(&self) -> String {
        String::new()
    }
}

/// Every path a replay recording reacts to on this mapper: Quick Start's and
/// those of the recorder for `info`'s version. `xpr_replay` emulates every
/// frame of the stretches where one of them changes.
pub fn watched_paths(info: &crate::host::StartInfo, store: &PropertyStore) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    let mut watch = crate::starter::StarterWatch::offline(false);
    paths.extend(watch.on_mapper_loaded(store));
    let controller = crate::RecorderController::new(crate::host::HostHandle::direct(Box::new(NullHost)), std::sync::Arc::new(|| {}));
    if let Some((_, mut game)) = crate::games::create_recorder(info, controller) {
        let (watch, _invalid) = game.on_mapper_loaded(store);
        paths.extend(watch);
    }
    paths.retain(|p| store.contains(p));
    paths.sort();
    paths.dedup();
    paths
}
