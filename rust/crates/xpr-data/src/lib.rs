//! Game data: typed schemas, loaders for the five generation formats,
//! lookup databases, per-gen stat arithmetic and the version registry.

pub mod backport;
pub mod badges;
pub mod db;
pub mod embedded;
pub mod exp;
pub mod gen_consts;
pub mod gen_data;
pub mod loaders;
pub mod model;
pub mod registry;
pub mod stats;

pub use badges::BadgeList;
pub use db::{ItemDB, MinBattlesDB, MoveDB, PkmnDB, TrainerDB};
pub use gen_data::{E4Entry, GenData};
pub use model::{
    BaseItem, CustomMoveData, EnemyPkmn, FieldStatus, Gen, Move, MoveEffect, Nature, PokemonSpecies, StageModifiers,
    StatBlock, Trainer, TrainerTimingStats,
};
pub use registry::Registry;
