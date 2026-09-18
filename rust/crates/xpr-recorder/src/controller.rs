//! Port of `route_recording/recorder.py`: `RecorderController` (the
//! translator between the game clients and the route) and the
//! `RecorderGameHookClient` session behaviour (mapper validation, status
//! strings, constant validation).

use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use serde_json::Value;

use xpr_core::consts;
use xpr_engine::{EventDefinition, NodeId};

use crate::gamehook::{GameHookClient, GameHookProperty, PropertyStore, SessionEvents};
use crate::host::{HostHandle, PrevEvent, StartInfo};
use crate::shuckie::supershuckie;

/// The FSM state of every game machine (`StateType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GameState {
    Uninitialized,
    Resetting,
    Overworld,
    Battle,
    InventoryChange,
    RareCandy,
    Tm,
    MoveDelete,
    Vitamin,
}

impl fmt::Display for GameState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            GameState::Uninitialized => "UNINITIALIZED",
            GameState::Resetting => "RESETTING",
            GameState::Overworld => "OVERWORLD",
            GameState::Battle => "BATTLE",
            GameState::InventoryChange => "INVENTORY_CHANGE",
            GameState::RareCandy => "RARE_CANDY",
            GameState::Tm => "TM",
            GameState::MoveDelete => "MOVE_DELETE",
            GameState::Vitamin => "VITAMIN",
        };
        write!(f, "StateType.{}", name)
    }
}

/// What the status bar shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecorderStatus {
    pub status: Option<String>,
    pub ready: Option<bool>,
    pub game_state: Option<GameState>,
}

#[derive(Default)]
struct FolderMeta {
    potential_new_area_name: Option<String>,
    potential_new_folder_name: Option<String>,
    active_area_name: Option<String>,
    active_folder_name: String,
}

/// The queue of events the FSM generated, drained by the processing thread
/// (`Machine._events_to_generate`).
pub struct EventQueue {
    items: Mutex<VecDeque<EventDefinition>>,
    cv: Condvar,
}

impl EventQueue {
    pub fn new() -> Arc<EventQueue> {
        Arc::new(EventQueue { items: Mutex::new(VecDeque::new()), cv: Condvar::new() })
    }

    /// `_queue_new_event`: stamped with the Super Shuckie time now, as the
    /// processing thread can lag behind.
    pub fn push(&self, mut event: EventDefinition) {
        event.recorded_time = match supershuckie().get_current_time() {
            Some(t) => Value::String(t),
            None => Value::Null,
        };
        self.items.lock().unwrap().push_back(event);
        self.cv.notify_all();
    }

    pub fn pop(&self, timeout: Duration) -> Option<EventDefinition> {
        let guard = self.items.lock().unwrap();
        let (mut guard, _) = self.cv.wait_timeout_while(guard, timeout, |q| q.is_empty()).unwrap();
        guard.pop_front()
    }

    /// `_events_to_generate.pop(0)` when the list is not empty.
    pub fn try_pop(&self) -> Option<EventDefinition> {
        self.items.lock().unwrap().pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.items.lock().unwrap().is_empty()
    }

    pub fn any(&self, pred: impl Fn(&EventDefinition) -> bool) -> bool {
        self.items.lock().unwrap().iter().any(pred)
    }
}

impl Default for EventQueue {
    fn default() -> Self {
        EventQueue { items: Mutex::new(VecDeque::new()), cv: Condvar::new() }
    }
}

pub struct RecorderController {
    host: HostHandle,
    status: Mutex<RecorderStatus>,
    folders: Mutex<FolderMeta>,
    client: Mutex<Option<Arc<GameHookClient>>>,
    /// the running machine's `_active` flag (see `deactivate`)
    machine_active: Mutex<Option<ActiveFlag>>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl RecorderController {
    pub fn new(host: HostHandle, wake: Arc<dyn Fn() + Send + Sync>) -> Arc<RecorderController> {
        Arc::new(RecorderController {
            host,
            status: Mutex::new(RecorderStatus::default()),
            folders: Mutex::new(FolderMeta {
                active_folder_name: consts::ROOT_FOLDER_NAME.to_string(),
                ..Default::default()
            }),
            client: Mutex::new(None),
            machine_active: Mutex::new(None),
            wake,
        })
    }

    pub fn host(&self) -> &HostHandle {
        &self.host
    }

    pub fn snapshot(&self) -> RecorderStatus {
        self.status.lock().unwrap().clone()
    }

    pub fn set_status(&self, new_val: &str) {
        self.status.lock().unwrap().status = Some(new_val.to_string());
        (self.wake)();
    }

    pub fn get_status(&self) -> Option<String> {
        self.status.lock().unwrap().status.clone()
    }

    pub fn set_ready(&self, new_val: bool) {
        self.status.lock().unwrap().ready = Some(new_val);
        (self.wake)();
    }

    pub fn is_ready(&self) -> bool {
        self.status.lock().unwrap().ready.unwrap_or(false)
    }

    pub fn set_game_state(&self, new_val: GameState) {
        self.status.lock().unwrap().game_state = Some(new_val);
        (self.wake)();
    }

    pub fn get_game_state(&self) -> Option<GameState> {
        self.status.lock().unwrap().game_state
    }

    fn is_active(&self) -> bool {
        self.host.call(|h| h.is_record_mode_active())
    }

    /// `on_recording_mode_changed`. Called on the UI thread, so everything
    /// needed to build the recorder arrives in `start` instead of being
    /// fetched through the (UI-thread-pumped) host channel.
    pub fn on_recording_mode_changed(self: &Arc<Self>, is_record_mode_active: bool, start: Option<StartInfo>) {
        if is_record_mode_active {
            if self.client.lock().unwrap().is_some() {
                log::warn!("Recording mode set to active, but gamehook client was already active");
                return;
            }
            match start {
                Some(info) => match crate::games::create_recorder(&info, self.clone()) {
                    Some((expected_names, game)) => {
                        *self.machine_active.lock().unwrap() = Some(game.active_flag());
                        let session = Session { controller: self.clone(), expected_names, game };
                        let client = GameHookClient::connect(&info.url, Box::new(session));
                        *self.client.lock().unwrap() = Some(client);
                    }
                    None => {
                        // The Qt `RecorderStatus` only shows the message in the status bar
                        // and leaves record mode on (the tkinter-era controller raised and
                        // turned recording off; that handler is not registered in the Qt app).
                        log::warn!("No recorder has been created yet for the current version");
                    }
                },
                None => {
                    self.host.post(|h| {
                        h.trigger_exception("Exception encountered trying to connect to gamehook: no route loaded. Check logs for more details");
                        h.set_record_mode(false);
                    });
                }
            }
        } else {
            let client = self.client.lock().unwrap().take();
            if let Some(c) = client {
                c.disconnect();
            }
            // `disconnect()` → `machine.shutdown()` in Python: the machine stops now,
            // not when the connection thread gets around to it
            if let Some(flag) = self.machine_active.lock().unwrap().take() {
                deactivate(&flag);
            }
        }
    }

    /// The status bar's reconnect button.
    pub fn reconnect(&self) {
        if let Some(c) = self.client.lock().unwrap().as_ref() {
            c.reconnect();
        }
    }

    pub fn has_client(&self) -> bool {
        self.client.lock().unwrap().is_some()
    }

    /// `route_restarted`: a new game file was started.
    pub fn route_restarted(&self) {
        if !self.host.call(|h| h.is_empty()) {
            // TODO(py): need to complain to the user somehow...
            self.set_ready(false);
        }
    }

    /// `entered_new_area`
    pub fn entered_new_area(&self, new_area_name: &str) {
        if !self.is_active() {
            log::warn!("Ignoring recorder function call due to recorder being inactive: entered_new_area({})", new_area_name);
            return;
        }
        let existing = self.host.call(|h| h.get_all_folder_names());
        let mut folder_name = new_area_name.to_string();
        let mut counter = 1;
        while existing.contains(&folder_name) {
            counter += 1;
            folder_name = format!("{}: Trip {}", new_area_name, counter);
        }
        let mut f = self.folders.lock().unwrap();
        f.potential_new_area_name = Some(new_area_name.to_string());
        f.potential_new_folder_name = Some(folder_name);
    }

    fn extract_area_name_from_folder_name(folder_name: &str) -> String {
        let test: Vec<&str> = folder_name.split(':').collect();
        if test.len() == 2 && test[1].contains("Trip") {
            return test[0].to_string();
        }
        folder_name.to_string()
    }

    /// `game_reset`: roll the route back to the last save.
    pub fn game_reset(&self) {
        if !self.is_active() {
            log::warn!("Ignoring recorder function call due to recorder being inactive: game_reset");
            return;
        }
        let mut to_delete: Vec<NodeId> = Vec::new();
        let mut test_obj = self.host.call(|h| h.get_previous_event(None));
        while let Some(obj) = &test_obj {
            if obj.definition.save.is_some() {
                break;
            }
            to_delete.push(obj.group_id);
            let id = obj.group_id;
            test_obj = self.host.call(move |h| h.get_previous_event(Some(id)));
        }
        log::info!("Cleaning up {} events for reset", to_delete.len());
        let ids = to_delete.clone();
        self.host.call(move |h| {
            h.delete_events(&ids);
            h.purge_empty_folders();
        });
        let mut f = self.folders.lock().unwrap();
        match test_obj {
            Some(obj) => {
                f.active_folder_name = obj.parent_name.clone();
                f.active_area_name = Some(Self::extract_area_name_from_folder_name(&obj.parent_name));
            }
            None => {
                f.active_folder_name = consts::ROOT_FOLDER_NAME.to_string();
                f.active_area_name = None;
            }
        }
        f.potential_new_area_name = None;
        f.potential_new_folder_name = None;
    }

    pub fn is_trainer_event(event_obj: Option<&PrevEvent>, trainer_name: &str) -> bool {
        match event_obj {
            Some(e) => e.definition.trainer_def.as_ref().map(|t| t.trainer_name == trainer_name).unwrap_or(false),
            None => false,
        }
    }

    /// `lost_trainer_battle`: remove the event of the trainer we lost to.
    pub fn lost_trainer_battle(&self, trainer_name: &str) {
        if !self.is_active() {
            log::warn!("Ignoring recorder function call due to recorder being inactive: lost_trainer_battle({})", trainer_name);
            return;
        }
        log::info!("[BLACKOUT DEBUG] lost_trainer_battle called for trainer: {}", trainer_name);
        let last_obj = self.host.call(|h| h.get_previous_event(None));
        let last_id = last_obj.as_ref().map(|o| o.group_id);
        log::info!("[BLACKOUT DEBUG] Last event: {:?}", last_id);
        let mut test_obj = last_obj;
        let mut search_count = 0;
        while !Self::is_trainer_event(test_obj.as_ref(), trainer_name) {
            let Some(obj) = &test_obj else {
                log::info!("[BLACKOUT DEBUG] Reached None while searching for trainer {} (searched {} events)", trainer_name, search_count);
                break;
            };
            let id = obj.group_id;
            test_obj = self.host.call(move |h| h.get_previous_event(Some(id)));
            search_count += 1;
            if search_count > 20 {
                log::error!("[BLACKOUT DEBUG] Search limit reached while looking for trainer {}", trainer_name);
                break;
            }
        }
        match test_obj {
            None => {
                let msg = format!("{} Could not find trainer event {} to remove after losing to them", consts::RECORDING_ERROR_FRAGMENT, trainer_name);
                log::error!("[BLACKOUT DEBUG] {}", msg);
                let folder = self.folders.lock().unwrap().active_folder_name.clone();
                self.host.call(move |h| h.new_event(EventDefinition::notes_only(&msg), &folder));
            }
            Some(obj) => {
                if Some(obj.group_id) != last_id {
                    log::info!("[BLACKOUT DEBUG] Found trainer event at position {} from last event", search_count);
                    log::error!("{} When removing trainer event {}, it was not the last event... odd", consts::RECORDING_ERROR_FRAGMENT, trainer_name);
                }
                log::info!("[BLACKOUT DEBUG] Deleting trainer event with group_id: {}", obj.group_id);
                let id = obj.group_id;
                self.host.call(move |h| h.delete_events(&[id]));
            }
        }
    }

    /// `check_final_trainer`: auto-stop recording after a configured final trainer.
    pub fn check_final_trainer(&self, trainer_name: &str) {
        if !self.is_active() {
            log::warn!("Ignoring recorder function call due to recorder being inactive: check_final_trainer({})", trainer_name);
            return;
        }
        let enabled = self.host.call(|h| h.recording_auto_stop_enabled());
        if !enabled {
            return;
        }
        let Some(version) = self.host.call(|h| h.get_version()) else { return };
        if version.is_empty() {
            return;
        }
        let v = version.clone();
        let final_trainers = self.host.call(move |h| h.final_trainers(&v));
        if !final_trainers.iter().any(|t| t == trainer_name) {
            return;
        }
        log::info!("Final trainer '{}' defeated for {} - scheduling recording stop", trainer_name, version);
        // dispatched (not awaited) so the FSM thread never waits on its own shutdown
        self.host.post(|h| {
            if h.is_record_mode_active() {
                h.set_record_mode(false);
            }
        });
    }

    /// `add_event`: folder bookkeeping, duplicate-trainer guard, level-up
    /// move overrides and item/vitamin/candy coalescing.
    pub fn add_event(&self, mut event_def: EventDefinition) {
        if !self.is_active() {
            log::warn!("Ignoring recorder function call due to recorder being inactive: add_event");
            return;
        }
        if event_def.recorded_time.is_null() {
            event_def.recorded_time = match supershuckie().get_current_time() {
                Some(t) => Value::String(t),
                None => Value::Null,
            };
        }
        let active_folder = {
            let mut f = self.folders.lock().unwrap();
            if let Some(potential_area) = f.potential_new_area_name.clone() {
                if f.active_area_name.as_deref() == Some(potential_area.as_str()) {
                    f.potential_new_area_name = None;
                    f.potential_new_folder_name = None;
                } else {
                    f.active_area_name = Some(potential_area);
                    f.active_folder_name = f.potential_new_folder_name.clone().unwrap_or_default();
                    f.potential_new_area_name = None;
                    f.potential_new_folder_name = None;
                    let name = f.active_folder_name.clone();
                    drop(f);
                    self.host.call(move |h| h.finalize_new_folder(&name));
                    f = self.folders.lock().unwrap();
                }
            }
            f.active_folder_name.clone()
        };
        let existing = self.host.call(|h| h.get_all_folder_names());
        if !existing.contains(&active_folder) {
            let name = active_folder.clone();
            self.host.call(move |h| h.finalize_new_folder(&name));
        }

        if let Some(td) = &event_def.trainer_def {
            let defeated = self.host.call(|h| h.get_defeated_trainers());
            if defeated.contains(&td.trainer_name) {
                let msg = format!("{} Tried to fight trainer that has already been defeated: {}", consts::RECORDING_ERROR_FRAGMENT, td.trainer_name);
                log::error!("{}", msg);
                let folder = active_folder.clone();
                self.host.call(move |h| h.new_event(EventDefinition::notes_only(&msg), &folder));
                return;
            }
        } else if let Some(lm) = event_def.learn_move.as_mut() {
            if let Some(dest_name) = lm.destination_name.take() {
                let name = dest_name.clone();
                let delete_idx = self.host.call(move |h| h.get_move_idx(&name));
                match delete_idx {
                    None => {
                        let gen = self.host.call(|h| h.gen());
                        let label = gen.as_ref().map(|g| crate::games::common::event_str(g, &event_def)).unwrap_or_default();
                        let msg = format!(
                            "{} When teaching level-up move {} over {}, Mon didn't have {}",
                            consts::RECORDING_ERROR_FRAGMENT,
                            label,
                            dest_name,
                            dest_name
                        );
                        log::error!("{}", msg);
                        let folder = active_folder.clone();
                        self.host.call(move |h| h.new_event(EventDefinition::notes_only(&msg), &folder));
                        return;
                    }
                    Some(idx) => lm.destination = Some(idx),
                }
            }
            if lm.source == consts::MOVE_SOURCE_LEVELUP {
                let def = lm.clone();
                self.host.call(move |h| h.update_levelup_move(def));
                return;
            }
        } else if let Some(item) = event_def.item_event_def.as_mut() {
            let last_event = self.host.call(|h| h.get_previous_event(None));
            if let Some(last) = last_event {
                if let Some(last_item) = &last.definition.item_event_def {
                    if last_item.item_name == item.item_name
                        && last_item.is_acquire == item.is_acquire
                        && last_item.with_money == item.with_money
                        && last.parent_name == active_folder
                    {
                        item.item_amount += last_item.item_amount;
                        // the combined event still starts where the original one did
                        event_def.recorded_time = last.definition.recorded_time.clone();
                        let id = last.group_id;
                        self.host.call(move |h| h.update_existing_event(id, event_def));
                        return;
                    }
                }
            }
        } else if let Some(vit) = event_def.vitamin.as_mut() {
            let last_event = self.host.call(|h| h.get_previous_event(None));
            if let Some(last) = last_event {
                if let Some(last_vit) = &last.definition.vitamin {
                    if last_vit.vitamin == vit.vitamin && last.parent_name == active_folder {
                        vit.amount += last_vit.amount;
                        event_def.recorded_time = last.definition.recorded_time.clone();
                        let id = last.group_id;
                        self.host.call(move |h| h.update_existing_event(id, event_def));
                        return;
                    }
                }
            }
        } else if let Some(candy) = event_def.rare_candy.as_mut() {
            let last_event = self.host.call(|h| h.get_previous_event(None));
            if let Some(last) = last_event {
                if let Some(last_candy) = &last.definition.rare_candy {
                    if last.parent_name == active_folder {
                        candy.amount += last_candy.amount;
                        event_def.recorded_time = last.definition.recorded_time.clone();
                        let id = last.group_id;
                        self.host.call(move |h| h.update_existing_event(id, event_def));
                        return;
                    }
                }
            }
        }

        let folder = active_folder;
        self.host.call(move |h| h.new_event(event_def, &folder));
    }
}

/// A game-specific recorder (the FSM `Machine` of one generation).
pub trait GameRecorder: Send {
    /// The mapper loaded with a matching game name: configure the keys for
    /// this mapper and case-correct them. Returns the paths to watch and the
    /// invalid (unmapped) constants.
    fn on_mapper_loaded(&mut self, store: &PropertyStore) -> (Vec<String>, Vec<String>);
    fn is_active(&self) -> bool;
    /// The machine's `_active` flag, shared with its processing thread (and
    /// with the controller, which clears it when recording stops).
    fn active_flag(&self) -> ActiveFlag;
    fn startup(&mut self, store: &PropertyStore);
    fn handle_event(&mut self, store: &PropertyStore, new: &GameHookProperty, prev: &GameHookProperty);
    fn shutdown(&mut self);
}

/// `Machine.shutdown()`: deactivate the machine and stop the Super Shuckie
/// poller — once. Python does this synchronously from `disconnect()`; here
/// it can run from the UI thread (recording turned off) and again from the
/// connection thread when it winds down, and only the first call may stop the
/// poller, or it would stop the poller of a session started in between.
pub fn deactivate(flag: &ActiveFlag) -> bool {
    let was_active = flag.swap(false, Ordering::SeqCst);
    if was_active {
        crate::shuckie::supershuckie().stop();
    }
    was_active
}

/// `RecorderGameHookClient`: statuses and mapper validation around a game recorder.
pub struct Session {
    controller: Arc<RecorderController>,
    expected_names: Vec<String>,
    game: Box<dyn GameRecorder>,
}

impl SessionEvents for Session {
    fn on_connected(&mut self) {
        self.controller.set_ready(false);
        self.controller.set_status(consts::RECORDING_STATUS_CONNECTED);
    }

    fn on_connection_error(&mut self) {
        self.controller.set_ready(false);
        self.controller.set_status(consts::RECORDING_STATUS_FAILED_CONNECTION);
    }

    fn on_disconnected(&mut self) {
        self.controller.set_ready(false);
        self.controller.set_status(consts::RECORDING_STATUS_DISCONNECTED);
    }

    fn on_game_hook_error(&mut self, err: &str) {
        self.controller.set_ready(false);
        log::error!("GameHook error: {}", err);
        let msg = format!("GameHook error: {}", err);
        self.controller.host.post(move |h| h.send_message(&msg));
        self.controller.set_status(consts::RECORDING_STATUS_GAMEHOOK_FAILED);
    }

    fn on_driver_error(&mut self, err: &str) {
        log::error!("[GameHook Client] Driver error ocurred: {}", err);
    }

    fn on_mapper_loaded(&mut self, store: &PropertyStore) -> Vec<String> {
        let game_name = store.game_name().unwrap_or("");
        let correct_mapper_loaded = self.expected_names.iter().any(|t| game_name.contains(t.as_str()));
        log::info!(
            "Successfully loaded mapper. Got gameName: {}, to be validated against: {:?} (result: {})",
            game_name,
            self.expected_names,
            correct_mapper_loaded
        );
        if !correct_mapper_loaded {
            self.controller.set_ready(false);
            self.controller.set_status(consts::RECORDING_STATUS_WRONG_MAPPER);
            return Vec::new();
        }
        self.controller.set_ready(true);
        self.controller.set_status(consts::RECORDING_STATUS_READY);
        let (watch, invalid) = self.game.on_mapper_loaded(store);
        if !invalid.is_empty() {
            let msg = format!("Likely due to mismatching GameHook version, invalid GameHook properties: {:?}", invalid);
            log::error!("{}", msg);
            self.controller.host.post(move |h| h.send_message(&msg));
        }
        log::info!("Validated GameHook constants successfully");
        // Appendix B item 13: every gen skips (with a warning) paths the mapper does not serve
        let mut result = Vec::new();
        for key in watch {
            if store.contains(&key) {
                result.push(key);
            } else {
                log::warn!("Skipping registration of unmapped path: {}", key);
            }
        }
        if !self.game.is_active() {
            self.game.startup(store);
        }
        result
    }

    fn on_mapper_load_error(&mut self, _err: &str) {
        self.controller.set_ready(false);
        self.controller.set_status(consts::RECORDING_STATUS_NO_MAPPER);
    }

    fn on_property_changed(&mut self, store: &PropertyStore, new: &GameHookProperty, old: &GameHookProperty) {
        self.game.handle_event(store, new, old);
    }

    fn on_shutdown(&mut self) {
        self.game.shutdown();
    }
}

/// `validate_constants` for one key: case-correct it against the mapper or
/// record it as invalid.
pub fn fix_key(store: &PropertyStore, key: &mut String, invalid: &mut Vec<String>) {
    if store.contains(key) {
        return;
    }
    match store.resolve_case(key) {
        Some(real) => *key = real,
        None => invalid.push(key.clone()),
    }
}

pub fn fix_opt_key(store: &PropertyStore, key: &mut Option<String>, invalid: &mut Vec<String>) {
    if let Some(k) = key {
        fix_key(store, k, invalid);
    }
}

pub fn fix_keys(store: &PropertyStore, keys: &mut [String], invalid: &mut Vec<String>) {
    for k in keys.iter_mut() {
        fix_key(store, k, invalid);
    }
}

/// Dedupe while keeping order (Python's `set` only affects the message).
pub fn dedupe(mut invalid: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    invalid.retain(|x| seen.insert(x.clone()));
    invalid
}

/// Shared processing-thread lifetime flag.
pub type ActiveFlag = Arc<AtomicBool>;

pub fn new_active_flag() -> ActiveFlag {
    Arc::new(AtomicBool::new(false))
}

pub fn is_set(flag: &ActiveFlag) -> bool {
    flag.load(Ordering::SeqCst)
}
