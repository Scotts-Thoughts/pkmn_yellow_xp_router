//! The route tree: `EventFolder` / `EventGroup` / `EventItem` from
//! `routing/route_events.py`, stored in an id-keyed arena owned by the
//! [`crate::router::Router`].

use std::sync::Arc;

use serde_json::Value;

use xpr_core::consts;
use xpr_data::model::EnemyPkmn;

use crate::events::{EventDefinition, LearnMoveEventDefinition};
use crate::state::RouteState;

pub type NodeId = i64;

#[derive(Clone, Debug)]
pub struct EventItem {
    pub id: NodeId,
    pub parent: NodeId,
    /// `EventItem._enabled` (set from the definition on apply; tri-state).
    pub enabled: Option<bool>,
    pub name: String,
    pub to_defeat_mon: Option<EnemyPkmn>,
    pub exp_split_num: i64,
    pub pay_day_amount: i64,
    pub defeating_trainer: bool,
    /// The solo mon steals this enemy's held item (Thief / Covet) before
    /// the KO; see [`RouteState::steal_held_item`].
    pub thief: bool,
    /// The item's definition: a copy of the group's definition, or, for the
    /// level-up move items the engine injects, its own learn-move definition.
    pub event_definition: EventDefinition,
    /// True when `event_definition` is the group's own (shared) definition.
    pub shares_group_definition: bool,
    pub init_state: Option<Arc<RouteState>>,
    pub final_state: Option<Arc<RouteState>>,
    pub error_message: String,
    /// The event applied, but not exactly as written (see
    /// [`crate::Inventory::swap_items`]). Never counts as an error.
    pub warning_message: String,
}

impl EventItem {
    pub fn has_errors(&self) -> bool {
        !self.error_message.is_empty()
    }

    pub fn has_warnings(&self) -> bool {
        !self.warning_message.is_empty()
    }

    pub fn get_tags(&self) -> Vec<&'static str> {
        if self.has_errors() {
            vec![consts::EVENT_TAG_ERRORS]
        } else if self.has_warnings() {
            vec![consts::EVENT_TAG_WARNINGS]
        } else {
            Vec::new()
        }
    }
}

#[derive(Clone, Debug)]
pub struct EventGroup {
    pub id: NodeId,
    pub parent: NodeId,
    pub enabled: Option<bool>,
    pub name: String,
    pub init_state: Option<Arc<RouteState>>,
    pub final_state: Option<Arc<RouteState>>,
    pub event_items: Vec<NodeId>,
    pub event_definition: EventDefinition,
    pub pkmn_after_levelups: Vec<String>,
    pub error_messages: Vec<String>,
    /// Warnings collected from the items (the label is kept, unlike errors).
    pub warning_messages: Vec<String>,
    pub level_up_learn_event_defs: Vec<LearnMoveEventDefinition>,
}

impl EventGroup {
    pub fn has_errors(&self) -> bool {
        !self.error_messages.is_empty()
    }

    pub fn has_warnings(&self) -> bool {
        !self.warning_messages.is_empty()
    }

    pub fn get_pkmn_after_levelups(&self) -> String {
        self.pkmn_after_levelups.join(",")
    }

    pub fn serialize(&self) -> Value {
        self.event_definition.serialize()
    }

    /// `set_enabled_status`
    pub fn set_enabled_status(&mut self, is_enabled: bool) {
        self.enabled = Some(is_enabled);
        self.event_definition.enabled = Some(is_enabled);
    }

    /// `__repr__`
    pub fn py_repr(&self, label: &str) -> String {
        format!("EventGroup: {}", label)
    }
}

#[derive(Clone, Debug)]
pub struct EventFolder {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub name: String,
    pub enabled: Option<bool>,
    pub expanded: Option<bool>,
    pub event_definition: EventDefinition,
    pub init_state: Option<Arc<RouteState>>,
    pub final_state: Option<Arc<RouteState>>,
    pub child_errors: bool,
    pub children: Vec<NodeId>,
}

impl EventFolder {
    pub fn has_errors(&self) -> bool {
        self.child_errors
    }

    pub fn set_enabled_status(&mut self, is_enabled: bool) {
        self.enabled = Some(is_enabled);
    }

    pub fn is_expanded(&self) -> bool {
        self.expanded.unwrap_or(false)
    }

    pub fn py_repr(&self) -> String {
        format!("EventFolder: {}", self.name)
    }
}

#[derive(Clone, Debug)]
pub enum Node {
    Folder(EventFolder),
    Group(EventGroup),
}

impl Node {
    pub fn id(&self) -> NodeId {
        match self {
            Node::Folder(f) => f.id,
            Node::Group(g) => g.id,
        }
    }

    pub fn parent(&self) -> Option<NodeId> {
        match self {
            Node::Folder(f) => f.parent,
            Node::Group(g) => Some(g.parent),
        }
    }

    pub fn set_parent(&mut self, parent: Option<NodeId>) {
        match self {
            Node::Folder(f) => f.parent = parent,
            Node::Group(g) => {
                if let Some(p) = parent {
                    g.parent = p;
                }
            }
        }
    }

    pub fn is_folder(&self) -> bool {
        matches!(self, Node::Folder(_))
    }

    pub fn is_group(&self) -> bool {
        matches!(self, Node::Group(_))
    }

    pub fn as_folder(&self) -> Option<&EventFolder> {
        match self {
            Node::Folder(f) => Some(f),
            _ => None,
        }
    }

    pub fn as_folder_mut(&mut self) -> Option<&mut EventFolder> {
        match self {
            Node::Folder(f) => Some(f),
            _ => None,
        }
    }

    pub fn as_group(&self) -> Option<&EventGroup> {
        match self {
            Node::Group(g) => Some(g),
            _ => None,
        }
    }

    pub fn as_group_mut(&mut self) -> Option<&mut EventGroup> {
        match self {
            Node::Group(g) => Some(g),
            _ => None,
        }
    }

    pub fn event_definition(&self) -> &EventDefinition {
        match self {
            Node::Folder(f) => &f.event_definition,
            Node::Group(g) => &g.event_definition,
        }
    }

    pub fn event_definition_mut(&mut self) -> &mut EventDefinition {
        match self {
            Node::Folder(f) => &mut f.event_definition,
            Node::Group(g) => &mut g.event_definition,
        }
    }

    pub fn init_state(&self) -> Option<&Arc<RouteState>> {
        match self {
            Node::Folder(f) => f.init_state.as_ref(),
            Node::Group(g) => g.init_state.as_ref(),
        }
    }

    pub fn final_state(&self) -> Option<&Arc<RouteState>> {
        match self {
            Node::Folder(f) => f.final_state.as_ref(),
            Node::Group(g) => g.final_state.as_ref(),
        }
    }

    pub fn has_errors(&self) -> bool {
        match self {
            Node::Folder(f) => f.has_errors(),
            Node::Group(g) => g.has_errors(),
        }
    }

    /// The node's own enabled flag (before consulting the parent chain).
    pub fn own_enabled(&self) -> bool {
        match self {
            Node::Folder(f) => f.enabled.unwrap_or(false),
            Node::Group(g) => g.enabled.unwrap_or(false),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Node::Folder(f) => &f.name,
            Node::Group(g) => &g.name,
        }
    }
}

/// Either kind of addressable object in the route (`get_event_obj`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjKind {
    Folder,
    Group,
    Item,
}
