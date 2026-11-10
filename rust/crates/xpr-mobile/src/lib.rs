#![recursion_limit = "512"]
//! The router's business logic behind a JSON command interface, for the
//! native iOS app under `ios/`. The desktop app's UI-free controllers
//! (`MainController`, `BattleController`, the PP ledger and the route
//! index) are compiled in from `xpr-app/src` by path, so both front ends
//! share one implementation.

#[path = "../../xpr-app/src/controller.rs"]
pub mod controller;
#[path = "../../xpr-app/src/battle.rs"]
pub mod battle;
#[path = "../../xpr-app/src/pp_ledger.rs"]
pub mod pp_ledger;
#[path = "../../xpr-app/src/route_index.rs"]
pub mod route_index;

#[path = "../../xpr-app/src/map/state.rs"]
pub mod map_state;

pub mod ffi;
pub mod session;
