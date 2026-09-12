//! The application-side interface the recorder talks to (the subset of
//! `MainController` used by `recorder.py` and the game FSMs), plus the
//! thread-safe dispatch that hands each call to the UI thread and blocks
//! until it has been applied (the plan's "channel + reply" design).

use std::any::Any;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};

use xpr_data::model::StatBlock;
use xpr_data::GenData;
use xpr_engine::{EventDefinition, LearnMoveEventDefinition, NodeId};

/// What the UI thread hands over when recording starts (everything the
/// machines would otherwise have to ask the host for synchronously while
/// still on the UI thread, which would dead-lock the channel dispatch).
#[derive(Clone)]
pub struct StartInfo {
    pub version: String,
    pub gen: Arc<GenData>,
    pub url: String,
    pub debug_mode: bool,
    pub dvs: Option<StatBlock>,
    pub solo_species: Option<String>,
}

/// The bits of an `EventGroup` the recorder inspects.
#[derive(Clone, Debug)]
pub struct PrevEvent {
    pub group_id: NodeId,
    pub parent_name: String,
    pub definition: EventDefinition,
    /// `final_state.solo_pkmn.held_item`
    pub final_held_item: Option<String>,
    /// the name of the first trainer (for `get_fight_reward`)
    pub first_trainer_name: Option<String>,
}

/// What the recorder needs from the application. Implemented by the app's
/// controller; every method runs on the UI thread.
pub trait RecorderHost {
    fn is_record_mode_active(&self) -> bool;
    fn set_record_mode(&mut self, active: bool);
    fn is_empty(&self) -> bool;
    fn get_all_folder_names(&self) -> Vec<String>;
    /// `MainController.get_previous_event(cur_event_id)` (enabled groups only)
    fn get_previous_event(&self, cur_event_id: Option<NodeId>) -> Option<PrevEvent>;
    fn delete_events(&mut self, ids: &[NodeId]);
    fn purge_empty_folders(&mut self);
    fn new_event(&mut self, def: EventDefinition, dest_folder_name: &str);
    fn finalize_new_folder(&mut self, name: &str);
    fn get_defeated_trainers(&self) -> Vec<String>;
    fn get_move_idx(&self, move_name: &str) -> Option<i64>;
    fn update_levelup_move(&mut self, def: LearnMoveEventDefinition);
    fn update_existing_event(&mut self, id: NodeId, def: EventDefinition);
    /// `init_route_state.solo_pkmn.dvs`
    fn get_dvs(&self) -> Option<StatBlock>;
    /// `get_final_state().solo_pkmn.species_def.name`
    fn final_solo_species(&self) -> Option<String>;
    /// `get_final_state().solo_pkmn.held_item`
    fn final_held_item(&self) -> Option<String>;
    /// `get_final_state().inventory._item_lookup.get(name) is not None`
    fn final_inventory_has(&self, item_name: &str) -> bool;
    fn can_evolve_into(&self, species: &str) -> bool;
    fn is_valid_levelup_move(&self, def: &LearnMoveEventDefinition) -> bool;
    fn get_version(&self) -> Option<String>;
    fn gen(&self) -> Option<Arc<GenData>>;
    fn trigger_exception(&mut self, msg: &str);
    fn send_message(&mut self, msg: &str);
    /// `config.get_final_trainers(version)`
    fn final_trainers(&self, version: &str) -> Vec<String>;
    fn recording_auto_stop_enabled(&self) -> bool;
    fn is_debug_mode(&self) -> bool;
    /// the GameHook base URL (default `http://localhost:8085`)
    fn gamehook_url(&self) -> String;
}

type HostFn = Box<dyn FnOnce(&mut dyn RecorderHost) -> Box<dyn Any + Send> + Send>;

pub struct HostCall {
    f: HostFn,
    reply: SyncSender<Box<dyn Any + Send>>,
}

/// The recorder side of the dispatch.
#[derive(Clone)]
pub enum HostHandle {
    /// calls are queued for the UI thread and awaited
    Channel {
        tx: Sender<HostCall>,
        wake: Arc<dyn Fn() + Send + Sync>,
    },
    /// calls run inline against a shared host (tests, headless tools)
    Direct(Arc<Mutex<Box<dyn RecorderHost + Send>>>),
}

impl HostHandle {
    pub fn direct(host: Box<dyn RecorderHost + Send>) -> HostHandle {
        HostHandle::Direct(Arc::new(Mutex::new(host)))
    }

    /// Run `f` on the host and return its result, blocking until the UI
    /// thread has applied it. If the application has gone away the default
    /// value is returned.
    pub fn call<R: Send + Default + 'static>(&self, f: impl FnOnce(&mut dyn RecorderHost) -> R + Send + 'static) -> R {
        match self {
            HostHandle::Direct(host) => {
                let mut guard = host.lock().unwrap();
                f(guard.as_mut())
            }
            HostHandle::Channel { tx, wake } => {
                let (reply_tx, reply_rx) = mpsc::sync_channel::<Box<dyn Any + Send>>(1);
                let call = HostCall {
                    f: Box::new(move |h| Box::new(f(h)) as Box<dyn Any + Send>),
                    reply: reply_tx,
                };
                if tx.send(call).is_err() {
                    return R::default();
                }
                wake();
                match reply_rx.recv() {
                    Ok(boxed) => *boxed.downcast::<R>().unwrap_or_else(|_| Box::new(R::default())),
                    Err(_) => R::default(),
                }
            }
        }
    }

    /// Fire-and-forget variant (no reply awaited).
    pub fn post(&self, f: impl FnOnce(&mut dyn RecorderHost) + Send + 'static) {
        match self {
            HostHandle::Direct(host) => {
                let mut guard = host.lock().unwrap();
                f(guard.as_mut());
            }
            HostHandle::Channel { tx, wake } => {
                let (reply_tx, _reply_rx) = mpsc::sync_channel::<Box<dyn Any + Send>>(1);
                let call = HostCall {
                    f: Box::new(move |h| {
                        f(h);
                        Box::new(()) as Box<dyn Any + Send>
                    }),
                    reply: reply_tx,
                };
                let _ = tx.send(call);
                wake();
            }
        }
    }
}

/// The application side: drain pending calls once per frame.
pub struct HostQueue {
    rx: Receiver<HostCall>,
}

impl HostQueue {
    /// Apply every queued call against `host`; returns how many ran.
    pub fn pump(&self, host: &mut dyn RecorderHost) -> usize {
        let mut n = 0;
        while let Ok(call) = self.rx.try_recv() {
            let result = (call.f)(host);
            let _ = call.reply.send(result);
            n += 1;
        }
        n
    }
}

/// Create the dispatch pair. `wake` is invoked whenever a call is queued
/// (the egui app uses it to request a repaint so the queue gets pumped).
pub fn host_channel(wake: Arc<dyn Fn() + Send + Sync>) -> (HostHandle, HostQueue) {
    let (tx, rx) = mpsc::channel();
    (HostHandle::Channel { tx, wake }, HostQueue { rx })
}
