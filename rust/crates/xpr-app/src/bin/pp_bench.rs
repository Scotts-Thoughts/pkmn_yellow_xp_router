//! `pp_bench <route.json>...`
//!
//! Times the PP ledger (docs/rust_port/design/pp_tracking/PLAN.md §4.5) the
//! way the Pre-Event State tab drives it: a cold build (every fight's spend
//! computed), a no-op `ensure`, a change at the end of the route (the fight
//! spends come from the cache), and a rare candy before the first trainer
//! fight (every later fight's spend recomputed).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use xpr_app::controller::MainController;
use xpr_core::{Config, Paths};
use xpr_data::Registry;
use xpr_engine::EventDefinition;

fn ms(d: Duration) -> String {
    format!("{:.1} ms", d.as_secs_f64() * 1000.0)
}

fn main() {
    let routes: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    if routes.is_empty() {
        eprintln!("usage: pp_bench <route.json>...");
        std::process::exit(2);
    }
    let source_root = xpr_core::consts::find_source_root().expect("source root");
    let mut paths = Paths::new(source_root);
    let cfg = Config::load(&paths.global_config_file);
    paths.config_user_data_dir(&cfg.get_user_data_dir());
    let registry = Arc::new(Registry::new(paths.pokemon_raw_data.clone(), paths.custom_gens_dir.clone()));
    let _ = registry.reload_all_custom_gens();
    println!("rayon threads: {}", rayon_threads());

    for route in routes {
        let mut ctrl = MainController::new(registry.clone(), paths.clone());
        ctrl.load_route(&route);
        let _ = ctrl.take_signals();
        let groups = ctrl.router.all_groups();
        let fights = groups
            .iter()
            .filter(|g| ctrl.router.group(**g).map(|x| x.event_definition.trainer_def.is_some() || x.event_definition.wild_pkmn_info.is_some()).unwrap_or(false))
            .count();
        println!("{}: {} groups, {} fights", route.file_name().unwrap().to_string_lossy(), groups.len(), fights);

        let t = Instant::now();
        ctrl.ensure_pp(&cfg);
        println!("  cold build:        {} ({} fights computed)", ms(t.elapsed()), ctrl.pp.last_computed);

        let t = Instant::now();
        ctrl.ensure_pp(&cfg);
        println!("  no-op ensure:      {}", ms(t.elapsed()));

        // where the route runs lowest on PP
        let mut lowest: Vec<(i64, String, String)> = groups
            .iter()
            .filter_map(|g| {
                let snap = ctrl.pp.before(*g)?;
                let s = snap.slots.iter().flatten().min_by_key(|s| s.cur)?;
                Some((s.cur, ctrl.router.group(*g)?.name.clone(), format!("{} {}/{}", s.move_name, s.cur, s.max)))
            })
            .collect();
        lowest.sort();
        for (_, name, slot) in lowest.iter().take(3) {
            println!("  low PP before:     {} ({})", name, slot);
        }

        // a note at the end: the walk reruns, every fight from the cache
        let last = *groups.last().unwrap();
        ctrl.update_existing_event(last, EventDefinition::notes_only("pp_bench"));
        let t = Instant::now();
        ctrl.ensure_pp(&cfg);
        println!("  late edit:         {} ({} fights computed)", ms(t.elapsed()), ctrl.pp.last_computed);

        // a rare candy before the first trainer fight: every later fight changes
        let first_trainer = groups.iter().copied().find(|g| ctrl.router.group(*g).map(|x| x.event_definition.trainer_def.is_some()).unwrap_or(false));
        if let Some(gid) = first_trainer {
            ctrl.new_event(EventDefinition::with_rare_candy(1), None, Some(gid), None, false);
            let t = Instant::now();
            ctrl.ensure_pp(&cfg);
            println!("  candy before gym:  {} ({} fights computed)", ms(t.elapsed()), ctrl.pp.last_computed);
        }
    }
}

fn rayon_threads() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
}
