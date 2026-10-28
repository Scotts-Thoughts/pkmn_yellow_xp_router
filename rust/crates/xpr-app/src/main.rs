//! `main.pyw`: logging, config, data directories, the window, and the
//! restart-after-update handshake.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Arc;

use xpr_app::app::{initial_viewport, ExitState, StartupWork, XprApp};
use xpr_core::{Config, Paths};
use xpr_data::Registry;

// the gen 4/5 map pictures (`build.rs`: `write_map_imagery`)
include!(concat!(env!("OUT_DIR"), "/embedded_imagery.rs"));

fn main() {
    register_map_imagery();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let debug = args.iter().any(|a| a == "--debug");
    let source_root = xpr_core::consts::find_source_root().unwrap_or_else(|| {
        std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())).unwrap_or_else(|| PathBuf::from("."))
    });
    let mut paths = Paths::new(source_root);
    xpr_core::logging::config_logging(&paths.global_config_dir);
    #[cfg(windows)]
    xpr_app::crash_dump::install(&paths.global_config_dir);
    if debug {
        log::info!("Debug mode requested");
    }
    let cfg = Config::load(&paths.global_config_file);
    let data_dir = cfg.get_user_data_dir();
    if !data_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&data_dir) {
            log::error!("Could not create the user data dir {}: {}", data_dir.display(), e);
        }
    }
    paths.config_user_data_dir(&data_dir);
    // the copy a previous update moved aside, and any leftover download
    xpr_update::auto_cleanup_old_version();
    let config_dir = paths.global_config_dir.clone();
    let registry = Arc::new(Registry::new(paths.pokemon_raw_data.clone(), paths.custom_gens_dir.clone()));
    let background = StartupWork::spawn(&paths, &registry);
    let exit = Arc::new(std::sync::Mutex::new(ExitState::default()));
    let exit_for_app = exit.clone();
    let options = eframe::NativeOptions {
        viewport: initial_viewport(&cfg),
        persist_window: false,
        ..Default::default()
    };
    let result = eframe::run_native(
        "Pokemon Solo Challenge Router",
        options,
        Box::new(move |cc| Ok(Box::new(XprApp::new(cc, cfg, paths, registry, exit_for_app, background)))),
    );
    if let Err(e) = result {
        log::error!("eframe error: {}", e);
    }
    let exit_state = exit.lock().unwrap().clone();
    if let Some(swap) = exit_state.pending_swap {
        log::info!("App closed, switching to {}", swap.to_version);
        // on failure the old version stays in place; either way the restarted
        // app reports the result (`xpr_update::take_result`)
        let ok = xpr_update::apply_swap(&swap, &config_dir).is_ok();
        log::info!("Update result: {}", ok);
        if let Err(e) = xpr_update::restart(&args) {
            log::error!("Failed to restart: {}", e);
        }
    }
}
