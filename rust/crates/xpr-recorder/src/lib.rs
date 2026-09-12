//! Route recording: GameHook client, Super Shuckie poller, the recorder
//! controller and the per-game FSMs (`route_recording/**`).

pub mod controller;
pub mod gamehook;
pub mod games;
pub mod host;
pub mod shuckie;

pub use controller::{GameState, RecorderController, RecorderStatus};
pub use gamehook::{GameHookClient, GameHookProperty, PropertyStore};
pub use host::{host_channel, HostHandle, HostQueue, PrevEvent, RecorderHost};
pub use shuckie::{format_time_ms, supershuckie, SuperShuckieClient};
