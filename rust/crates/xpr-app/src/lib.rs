//! The egui application: window, pages, route list, event details, battle
//! summary, dialogs, secondary windows, recorder wiring and the update flow.

pub mod app;
pub mod assets;
pub mod battle;
pub mod compare;
pub mod battle_ui;
pub mod controller;
pub mod custom_dvs;
pub mod dialogs;
pub mod editors;
pub mod event_details;
pub mod filter_bar;
pub mod inline_creator;
pub mod map;
pub mod pages;
pub mod quick_add;
pub mod recorder_glue;
pub mod route_index;
pub mod route_list;
pub mod screenshot;
pub mod state_views;
pub mod summaries;
#[cfg(windows)]
pub mod win_monitor;

/// A config path for tests and examples that no two processes or threads
/// share: `rust/target/xpr-scratch-config/<pid>-<n>.json`. `Config` writes
/// its file on every setter, so the harnesses that all loaded
/// `rust/target/no-such-config.json` leaked map view state into each other
/// whenever cargo ran their binaries in parallel (a test expecting the
/// world scope after loading found the map scope another test had saved).
pub fn scratch_config_path() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/xpr-scratch-config");
    let _ = std::fs::create_dir_all(&dir);
    dir.join(format!("{}-{}.json", std::process::id(), N.fetch_add(1, Ordering::Relaxed)))
}
