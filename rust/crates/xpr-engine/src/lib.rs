//! The route engine: state objects, event definitions, the route tree and
//! router, recalculation and undo.

pub mod compare;
pub mod events;
pub mod recalc;
pub mod router;
pub mod state;
pub mod tree;
pub mod undo;
pub mod view;

pub use compare::{compare, digest, RouteComparison, RouteDigest, RouteOrigin};
pub use events::{
    swaps_between, BagReorderEventDefinition, BagSwap, EvOverrideEventDefinition, EvolutionEventDefinition, EventDefinition,
    HoldItemEventDefinition, InventoryEventDefinition, LearnMoveEventDefinition, LevelUpKey, LevelVal,
    LocationEventDefinition, RareCandyEventDefinition, TrainerEventDefinition, VitaminEventDefinition,
    WildPkmnEventDefinition,
};
pub use router::{InsertSpec, Router};
pub use state::{BagItem, Inventory, RouteState, SoloPokemon};
pub use tree::{EventFolder, EventGroup, EventItem, Node, NodeId, ObjKind};
pub use undo::{SnapNode, UndoManager, UndoState};
