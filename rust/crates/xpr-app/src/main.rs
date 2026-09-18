//! `main.pyw`: logging, config, data directories, the window, and the
//! restart-after-update handshake.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Arc;

use xpr_app::app::{initial_viewport, ExitState, XprApp};
use xpr_core::{Config, Paths};
use xpr_data::Registry;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let debug = args.iter().any(|a| a == "--debug");
    let source_root = xpr_core::consts::find_source_root().unwrap_or_else(|| {
        std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())).unwrap_or_else(|| PathBuf::from("."))
    });
    let mut paths = Paths::new(source_root);
    xpr_core::logging::config_logging(&paths.global_config_dir);
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
    let registry = Arc::new(Registry::new(paths.pokemon_raw_data.clone(), paths.custom_gens_dir.clone()));
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
        Box::new(move |cc| Ok(Box::new(XprApp::new(cc, cfg, paths, registry, exit_for_app)))),
    );
    if let Err(e) = result {
        log::error!("eframe error: {}", e);
    }
    let exit_state = exit.lock().unwrap().clone();
    log::info!("App closed, autoupdate requested? {}", exit_state.update_requested);
    if exit_state.update_requested {
        log::info!("Beginning cleanup of old version");
        xpr_update::auto_cleanup_old_version();
        let temp_dir = std::env::temp_dir().join("pkmn_xp_router_update");
        let _ = std::fs::create_dir_all(&temp_dir);
        let mut display = |msg: &str| {
            log::info!("{}", msg);
            eprintln!("{}", msg);
        };
        let ok = xpr_update::update(exit_state.update_version.as_deref(), exit_state.update_url.as_deref(), &temp_dir, &mut display);
        log::info!("Update result: {}", ok);
        if let Err(e) = xpr_update::restart(&args) {
            log::error!("Failed to restart: {}", e);
        }
    }
}
