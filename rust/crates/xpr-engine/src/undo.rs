//! Port of `controllers/undo_manager.py` plus
//! `Router.restore_events_from_state`.

use std::sync::Arc;

use serde_json::Value;

use xpr_core::consts;
use xpr_core::pyjson::{self, get};

use crate::events::{EventDefinition, LearnMoveEventDefinition, LevelUpKey};
use crate::router::Router;
use crate::tree::{Node, NodeId};

/// A copy of the event tree holding exactly what `Router::serialize_folder`
/// writes. Python snapshots the serialized JSON; taking a structural copy is
/// several times cheaper (the app snapshots on every edit, undo is rare) and
/// [`SnapNode::to_json`] produces the identical JSON when it is restored.
#[derive(Clone, Debug)]
pub enum SnapNode {
    Folder {
        name: String,
        notes: String,
        expanded: Option<bool>,
        enabled: Option<bool>,
        children: Vec<SnapNode>,
    },
    Group(EventDefinition),
    /// A child id with no node behind it (`serialize_folder` writes `null`).
    Missing,
}

impl SnapNode {
    fn capture(router: &Router, id: NodeId) -> SnapNode {
        match router.node(id) {
            Some(Node::Folder(f)) => SnapNode::Folder {
                name: f.name.clone(),
                notes: f.event_definition.notes.clone(),
                expanded: f.expanded,
                enabled: f.enabled,
                children: f.children.iter().map(|c| SnapNode::capture(router, *c)).collect(),
            },
            Some(Node::Group(g)) => SnapNode::Group(g.event_definition.clone()),
            None => SnapNode::Missing,
        }
    }

    /// The JSON `Router::serialize_folder` / `EventGroup::serialize` produce.
    pub fn to_json(&self) -> Value {
        match self {
            SnapNode::Missing => Value::Null,
            SnapNode::Group(def) => def.serialize(),
            SnapNode::Folder { name, notes, expanded, enabled, children } => pyjson::object(vec![
                (consts::EVENT_FOLDER_NAME, Value::String(name.clone())),
                (consts::TASK_NOTES_ONLY, Value::String(notes.clone())),
                (consts::EVENTS, Value::Array(children.iter().map(|c| c.to_json()).collect())),
                (consts::EXPANDED_KEY, expanded.map(Value::Bool).unwrap_or(Value::Null)),
                (consts::ENABLED_KEY, enabled.map(Value::Bool).unwrap_or(Value::Null)),
            ]),
        }
    }
}

/// One snapshot: the event tree, the defeated-trainer set and the level-up
/// move definitions (keyed the way Python's `str(tuple)` keys are parsed
/// back: only the first element, the sanitized mon name, is used).
#[derive(Clone, Debug)]
pub struct UndoState {
    pub events: SnapNode,
    pub defeated_trainers: Vec<String>,
    pub level_up_move_defs: Vec<(LevelUpKey, Value)>,
}

pub struct UndoManager {
    max_steps: usize,
    undo_stack: Vec<Arc<UndoState>>,
    current_state: Option<Arc<UndoState>>,
}

impl UndoManager {
    pub fn new(max_steps: usize) -> UndoManager {
        UndoManager {
            max_steps,
            undo_stack: Vec::new(),
            current_state: None,
        }
    }

    fn snapshot(router: &Router) -> UndoState {
        UndoState {
            events: SnapNode::capture(router, router.root_id),
            defeated_trainers: router.defeated_trainers.iter().cloned().collect(),
            level_up_move_defs: router
                .level_up_move_defs
                .iter()
                .map(|(k, v)| (k.clone(), v.serialize()))
                .collect(),
        }
    }

    /// `save_state(router, is_post_operation)`
    ///
    /// Before an operation the previous post-operation state goes on the
    /// stack. Python then also snapshots the router into `current_state`,
    /// but that copy is always replaced by the post-operation snapshot, so
    /// it is only taken here when there is no current state to push (the
    /// first operation after a load).
    pub fn save_state(&mut self, router: &Router, is_post_operation: bool) {
        if router.init_route_state.is_none() {
            return;
        }
        if is_post_operation {
            self.current_state = Some(Arc::new(UndoManager::snapshot(router)));
        } else {
            match self.current_state.take() {
                Some(cur) => {
                    self.undo_stack.push(cur.clone());
                    if self.undo_stack.len() > self.max_steps {
                        self.undo_stack.remove(0);
                    }
                    self.current_state = Some(cur);
                }
                None => self.current_state = Some(Arc::new(UndoManager::snapshot(router))),
            }
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn get_undo_state(&mut self) -> Option<Arc<UndoState>> {
        self.undo_stack.pop()
    }

    pub fn set_current_state(&mut self, state: Arc<UndoState>) {
        self.current_state = Some(state);
    }

    /// The snapshot taken after the last operation (or at load).
    pub fn get_current_state(&self) -> Option<Arc<UndoState>> {
        self.current_state.clone()
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.current_state = None;
    }
}

impl Router {
    /// `restore_events_from_state`
    pub fn restore_events_from_state(&mut self, state: &UndoState) -> Result<(), String> {
        self.reset_events();
        self.defeated_trainers = state.defeated_trainers.iter().cloned().collect();
        self.level_up_move_defs.clear();
        for (key, serialized) in &state.level_up_move_defs {
            let mon_name = key.0.clone();
            if mon_name.is_empty() {
                continue;
            }
            match LearnMoveEventDefinition::deserialize(Some(serialized), Some(&mon_name)) {
                Some(def) => {
                    if let Some(k) = def.get_level_up_key() {
                        self.level_up_move_defs.insert(k, def);
                    }
                }
                None => log::warn!("Failed to restore level up move {:?}", key),
            }
        }
        let events = state.events.to_json();
        if get(&events, consts::EVENTS).is_some() {
            let root = self.root_id;
            self.load_events_recursive_pub(root, &events)?;
        }
        self.recalc()
    }
}
