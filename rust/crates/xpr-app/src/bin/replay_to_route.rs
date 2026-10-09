//! `replay_to_route <file.replay> [options]`: record a route from a Super
//! Shuckie replay (see `xpr_app::replay_import`).
//!
//! ```text
//!   --out <route.json>     where to save it (default: the router's saved routes
//!                          folder, named after the replay)
//!   --rom <file>           the ROM (default: found by checksum)
//!   --rom-dir <dir>        another folder to look for the ROM in (repeatable)
//!   --mapper <path>        mapper under the mapper folder (default: from the ROM)
//!   --version <name>       router version (default: from the ROM)
//!   --accept-current       start from the Pokémon in slot 1 if the replay does
//!                          not begin with a new game
//!   --stride <n>           frames between mapper reads in active stretches (1)
//!   --slots <n>            parallel reader pairs
//!   --cpus <n>             run on n cores only (the helpers inherit it; default: all)
//!   --margin-before <n>    quiet keyframe intervals emulated before active ones (1)
//!   --margin-after <n>     ... and after them (4)
//!   --dense-all            emulate every frame (no coarse pass)
//!   --single-chunk         one mapper instance for the whole replay (no parallelism)
//!   --sparse-all           read keyframes only (no emulation)
//!   --frames <a>-<b>       only this part of the replay
//!   --tools <dir>          the helper programs (tools/replay_to_route/out)
//!   --log-dir <dir>        where the log goes (default: beside the route)
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use xpr_app::replay_import::{import_replay, save_route_to, ImportOptions};
use xpr_core::{Config, Paths};
use xpr_data::Registry;

fn usage() -> ! {
    eprintln!("usage: replay_to_route <file.replay> [--out <route.json>] [--rom <file>] [--rom-dir <dir>]... [--mapper <path>] [--version <name>] [--accept-current] [--stride <n>] [--slots <n>] [--margin-before <n>] [--margin-after <n>] [--dense-all] [--sparse-all] [--frames <a>-<b>] [--tools <dir>] [--log-dir <dir>]");
    std::process::exit(2);
}

fn args_has_slots() -> bool {
    std::env::args().any(|a| a == "--slots")
}

/// Keep this process, and the helpers it starts (they inherit it), on `n` cores.
#[cfg(windows)]
fn limit_cpus(n: usize) {
    unsafe extern "system" {
        fn GetCurrentProcess() -> isize;
        fn SetProcessAffinityMask(process: isize, mask: usize) -> i32;
    }
    let n = n.clamp(1, usize::BITS as usize - 1);
    let mask = (1usize << n) - 1;
    // SAFETY: plain Win32 calls on this process' own pseudo-handle
    if unsafe { SetProcessAffinityMask(GetCurrentProcess(), mask) } == 0 {
        eprintln!("warning: could not limit the import to {n} cores");
    }
}

#[cfg(not(windows))]
fn limit_cpus(_n: usize) {}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut opts = ImportOptions::default();
    let mut replay: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut log_dir: Option<PathBuf> = None;
    let mut cpus: Option<usize> = None;
    let num = |v: Option<String>| -> u64 { v.and_then(|s| s.parse().ok()).unwrap_or_else(|| usage()) };
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" => out = args.next().map(PathBuf::from),
            "--rom" => opts.rom = args.next().map(PathBuf::from),
            "--rom-dir" => opts.rom_dirs.extend(args.next().map(PathBuf::from)),
            "--mapper" => opts.mapper = args.next(),
            "--version" => opts.version = args.next(),
            "--accept-current" => opts.accept_current = true,
            "--stride" => opts.pipeline.stride = num(args.next()),
            "--slots" => opts.pipeline.slots = num(args.next()) as usize,
            "--cpus" => cpus = Some(num(args.next()) as usize),
            "--margin-before" => opts.pipeline.margin_before = num(args.next()) as usize,
            "--margin-after" => opts.pipeline.margin_after = num(args.next()) as usize,
            "--dense-all" => opts.pipeline.dense_all = true,
            "--single-chunk" => opts.pipeline.single_chunk = true,
            "--sparse-all" => opts.pipeline.sparse_all = true,
            "--frames" => {
                let v = args.next().unwrap_or_else(|| usage());
                let (a, b) = v.split_once('-').unwrap_or_else(|| usage());
                opts.pipeline.frames = Some((a.parse().unwrap_or_else(|_| usage()), b.parse().unwrap_or_else(|_| usage())));
            }
            "--tools" => opts.tools_dir = args.next().map(PathBuf::from),
            "--log-dir" => log_dir = args.next().map(PathBuf::from),
            s if s.starts_with("--") => usage(),
            _ => replay = Some(PathBuf::from(a)),
        }
    }
    let Some(replay) = replay else { usage() };
    if let Some(n) = cpus {
        limit_cpus(n);
        // a slot keeps about two cores busy
        if !args_has_slots() {
            opts.pipeline.slots = (n / 2).max(1);
        }
    }
    opts.replay = replay.clone();

    let source_root = xpr_core::consts::find_source_root().unwrap_or_else(|| {
        std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())).unwrap_or_else(|| PathBuf::from("."))
    });
    let mut paths = Paths::new(source_root);
    let cfg = Config::load(&paths.global_config_file);
    paths.config_user_data_dir(&cfg.get_user_data_dir());
    let stem = replay.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "replay".into());
    let out = out.unwrap_or_else(|| xpr_core::io_utils::get_safe_path_no_collision(&paths.saved_routes_dir, &stem, ".json"));
    let log_dir = log_dir.unwrap_or_else(|| out.parent().map(|p| p.join("replay_to_route_logs")).unwrap_or_else(|| PathBuf::from("replay_to_route_logs")));
    let _ = std::fs::create_dir_all(&log_dir);
    xpr_core::logging::config_logging(&log_dir);

    let registry = Arc::new(Registry::new(paths.pokemon_raw_data.clone(), paths.custom_gens_dir.clone()));
    let _ = registry.reload_all_custom_gens();

    let t0 = Instant::now();
    let mut last_report = Instant::now();
    let mut progress = |phase: &str, frame: u64, total: u64| {
        if last_report.elapsed().as_secs_f64() >= 2.0 {
            last_report = Instant::now();
            eprintln!("[replay_to_route] {phase}: {:.1}% ({frame}/{total}) after {:.0}s", frame as f64 * 100.0 / total.max(1) as f64, t0.elapsed().as_secs_f64());
        }
    };
    match import_replay(registry, paths, cfg, &opts, &mut progress) {
        Ok((ctrl, report)) => {
            if let Err(e) = save_route_to(&ctrl, &out) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
            let s = &report.summary;
            println!("route: {}", out.display());
            println!("version {} / mapper {} / ROM {}", report.version, report.mapper, report.rom.display());
            println!("starter: {}", report.starter);
            println!("events: {}{}", report.events, if report.stopped_at_final_trainer { " (stopped at the final trainer)" } else { "" });
            println!(
                "time: {:.1}s (scan {:.1}s, record {:.1}s); {} of {} keyframe intervals emulated, {} chunks, {} mapper reads, {} boundary fixes",
                t0.elapsed().as_secs_f64(),
                s.coarse_secs,
                s.final_secs,
                s.active_intervals,
                s.intervals,
                s.chunks,
                s.steps,
                s.boundary_fixups
            );
            println!("check against the game: {}", if report.check_passed { "passed" } else { "FAILED" });
            for line in &report.check {
                println!("  {line}");
            }
            if !s.noise.is_empty() {
                log::info!("[replay_to_route] noise properties: {:?}", s.noise);
            }
            for m in &report.messages {
                log::info!("[replay_to_route] {m}");
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
