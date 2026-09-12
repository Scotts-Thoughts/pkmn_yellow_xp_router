//! The per-game FSMs (`route_recording/game_recorders/**`).

pub mod common;
pub mod gen1;
pub mod gen2;
pub mod gen3;
pub mod gen45;

use std::sync::Arc;

use xpr_core::consts;

use crate::controller::{GameRecorder, RecorderController};
use crate::host::StartInfo;

/// `CurrentGen.get_recorder_client`: the recorder for a version, with the
/// mapper game names it accepts. `None` when no recorder exists for it.
pub fn create_recorder(info: &StartInfo, controller: Arc<RecorderController>) -> Option<(Vec<String>, Box<dyn GameRecorder>)> {
    let base = info.gen.effective_version().to_string();
    let names = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    match info.gen.get_generation() {
        1 => {
            if base == consts::YELLOW_VERSION {
                Some((names(&["Pokemon Yellow"]), Box::new(gen1::Gen1Machine::new(controller, info, false))))
            } else {
                Some((names(&["Pokemon Red and Blue", "Pokemon Red/Blue"]), Box::new(gen1::Gen1Machine::new(controller, info, true))))
            }
        }
        2 => {
            if base == consts::CRYSTAL_VERSION {
                Some((names(&["Pokemon Crystal"]), Box::new(gen2::Gen2Machine::new(controller, info))))
            } else {
                None
            }
        }
        3 => {
            if base == consts::EMERALD_VERSION {
                Some((names(&["Pokemon Emerald"]), Box::new(gen3::Gen3Machine::new(controller, info, false))))
            } else if base == consts::FIRE_RED_VERSION || base == consts::LEAF_GREEN_VERSION {
                Some((names(&["Pokemon FireRed & LeafGreen"]), Box::new(gen3::Gen3Machine::new(controller, info, true))))
            } else {
                None
            }
        }
        4 => {
            use gen45::Flavor;
            if base == consts::PLATINUM_VERSION {
                Some((names(&["Pokemon Platinum"]), Box::new(gen45::Gen45Machine::new(controller, info, Flavor::Platinum))))
            } else if base == consts::HEART_GOLD_VERSION {
                Some((names(&["Pokemon HeartGold"]), Box::new(gen45::Gen45Machine::new(controller, info, Flavor::HeartGoldSoulSilver))))
            } else if base == consts::SOUL_SILVER_VERSION {
                Some((names(&["Pokemon SoulSilver"]), Box::new(gen45::Gen45Machine::new(controller, info, Flavor::HeartGoldSoulSilver))))
            } else {
                None
            }
        }
        5 => {
            use gen45::Flavor;
            if base == consts::BLACK_VERSION {
                Some((names(&["Pokemon Black"]), Box::new(gen45::Gen45Machine::new(controller, info, Flavor::BlackWhite))))
            } else if base == consts::WHITE_VERSION {
                Some((names(&["Pokemon White"]), Box::new(gen45::Gen45Machine::new(controller, info, Flavor::BlackWhite))))
            } else if base == consts::BLACK_2_VERSION {
                Some((names(&["Pokemon Black 2"]), Box::new(gen45::Gen45Machine::new(controller, info, Flavor::Black2White2))))
            } else if base == consts::WHITE_2_VERSION {
                Some((names(&["Pokemon White 2"]), Box::new(gen45::Gen45Machine::new(controller, info, Flavor::Black2White2))))
            } else {
                None
            }
        }
        _ => None,
    }
}
