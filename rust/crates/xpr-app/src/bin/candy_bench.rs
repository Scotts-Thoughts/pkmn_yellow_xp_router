//! `candy_bench <route.json> [trainer substring] [iterations]`
//!
//! Times the pre-fight candy flow the way the app runs it: the route
//! mutation (undo snapshots + insert/replace + full recalc), the battle
//! summary reload with the transient restore, and the `route_changed`
//! cascade the frame then dispatches.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use xpr_app::battle::BattleController;
use xpr_app::controller::MainController;
use xpr_core::{Config, Paths};
use xpr_data::Registry;

fn ms(d: Duration) -> String {
    format!("{:.3} ms", d.as_secs_f64() * 1000.0)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: candy_bench <route.json> [trainer substring] [iterations]");
        std::process::exit(2);
    }
    if args[0] == "--pieces" {
        pieces(std::path::Path::new(&args[1]));
        return;
    }
    let route = PathBuf::from(&args[0]);
    let filter = args.get(1).cloned().unwrap_or_default();
    let iters: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(20);

    let source_root = xpr_core::consts::find_source_root().expect("source root");
    let mut paths = Paths::new(source_root);
    let cfg = Config::load(&paths.global_config_file);
    paths.config_user_data_dir(&cfg.get_user_data_dir());
    let registry = Arc::new(Registry::new(paths.pokemon_raw_data.clone(), paths.custom_gens_dir.clone()));
    let _ = registry.reload_all_custom_gens();

    let mut ctrl = MainController::new(registry, paths);
    let t = Instant::now();
    ctrl.load_route(&route);
    println!("load_route: {}", ms(t.elapsed()));
    let _ = ctrl.take_signals();

    // pick the trainer fight: the named one, else the one with the most mons
    let gen = ctrl.gen().expect("gen");
    let mut fights: Vec<(usize, String, xpr_engine::NodeId)> = Vec::new();
    for gid in ctrl.router.all_groups() {
        let Some(g) = ctrl.router.group(gid) else { continue };
        if g.event_definition.trainer_def.is_none() {
            continue;
        }
        let n = g.event_definition.pokemon_list(&gen).map(|l| l.len()).unwrap_or(0);
        fights.push((n, g.name.clone(), gid));
    }
    let pick = if filter.is_empty() {
        fights.iter().max_by_key(|f| f.0).cloned()
    } else {
        fights.iter().find(|f| f.1.contains(&filter)).cloned()
    };
    let Some((n_mons, name, gid)) = pick else {
        eprintln!("no trainer fight found");
        std::process::exit(1);
    };
    println!("fight: {} ({} mons, id {}), {} groups in route", name, n_mons, gid, ctrl.router.all_groups().len());

    ctrl.select_new_events(vec![gid]);
    let _ = ctrl.take_signals();
    let mut bc = BattleController::new();
    let t = Instant::now();
    bc.load_from_event(&cfg, &mut ctrl, gid);
    println!("initial load_from_event: {}", ms(t.elapsed()));
    let _ = bc.take_signals();

    // the summary refresh on its own
    let sc = BattleController::summary_config(&cfg, &ctrl);
    let t = Instant::now();
    for _ in 0..iters {
        bc.summary.full_refresh(Some(&gen), &sc);
    }
    println!("full_refresh alone: {} (avg of {})", ms(t.elapsed() / iters as u32), iters);
    {
        // how much of it is the kill search: depth 1 skips the search
        let mut shallow = sc.clone();
        shallow.calc.damage_search_depth = 1;
        let t = Instant::now();
        for _ in 0..iters {
            bc.summary.full_refresh(Some(&gen), &shallow);
        }
        println!("full_refresh with search depth 1: {} (avg of {})", ms(t.elapsed() / iters as u32), iters);
        bc.summary.full_refresh(Some(&gen), &sc);
        let mut n_moves = 0;
        for list in bc.summary.player_move_data.iter().chain(bc.summary.enemy_move_data.iter()) {
            n_moves += list.iter().flatten().count();
        }
        println!("moves computed per refresh: {}", n_moves);
    }

    // route-only pieces
    let t = Instant::now();
    for _ in 0..iters {
        ctrl.router.recalc().unwrap();
    }
    println!("router.recalc alone: {} (avg of {})", ms(t.elapsed() / iters as u32), iters);
    let t = Instant::now();
    for _ in 0..iters {
        let _ = ctrl.router.serialize_folder(ctrl.router.root_id);
    }
    println!("serialize_folder (one undo snapshot): {} (avg of {})", ms(t.elapsed() / iters as u32), iters);

    // the real flow: candy count 1, 2, 3, ... then back down, each followed
    // by the route_changed cascade (handle_route_change -> load_from_event)
    let mut total_mutation = Duration::ZERO;
    let mut total_cascade = Duration::ZERO;
    let mut worst = Duration::ZERO;
    let mut count = 0usize;
    let seq: Vec<i64> = (1..=iters as i64).chain((0..iters as i64).rev()).collect();
    for target in seq {
        let t0 = Instant::now();
        bc.update_prefight_candies(&cfg, &mut ctrl, target, false);
        let t1 = t0.elapsed();
        let sig = ctrl.take_signals();
        let t2 = Instant::now();
        if sig.route_changed && !bc.take_handled_route_change(gid) {
            // what EventDetails::handle_route_change does for the selected trainer group
            bc.load_from_event(&cfg, &mut ctrl, gid);
        }
        let t3 = t2.elapsed();
        let _ = bc.take_signals();
        total_mutation += t1;
        total_cascade += t3;
        worst = worst.max(t1 + t3);
        count += 1;
    }
    println!(
        "candy step: mutation+reload {} + cascade {} = {} avg over {} steps (worst {})",
        ms(total_mutation / count as u32),
        ms(total_cascade / count as u32),
        ms((total_mutation + total_cascade) / count as u32),
        count,
        ms(worst)
    );
    println!("candies now: {}", bc.get_prefight_candy_count(&ctrl));
}

/// Micro-benchmarks of the pieces (`candy_bench --pieces <route.json>`).
pub fn pieces(route: &std::path::Path) {
    let source_root = xpr_core::consts::find_source_root().expect("source root");
    let mut paths = Paths::new(source_root);
    let cfg = Config::load(&paths.global_config_file);
    paths.config_user_data_dir(&cfg.get_user_data_dir());
    let registry = Arc::new(Registry::new(paths.pokemon_raw_data.clone(), paths.custom_gens_dir.clone()));
    let _ = registry.reload_all_custom_gens();
    let mut ctrl = MainController::new(registry, paths);
    ctrl.load_route(route);
    let gen = ctrl.gen().expect("gen");

    // count group kinds and items
    let mut n_trainer = 0;
    let mut n_wild = 0;
    let mut n_other = 0;
    let mut n_items = 0;
    for gid in ctrl.router.all_groups() {
        let g = ctrl.router.group(gid).unwrap();
        if g.event_definition.trainer_def.is_some() {
            n_trainer += 1;
        } else if g.event_definition.wild_pkmn_info.is_some() {
            n_wild += 1;
        } else {
            n_other += 1;
        }
        n_items += g.event_items.len();
    }
    println!("groups: {} trainer, {} wild, {} other; {} items", n_trainer, n_wild, n_other, n_items);

    // get_pokemon_list over every trainer
    let t = Instant::now();
    let mut total_mons = 0;
    for gid in ctrl.router.all_groups() {
        let g = ctrl.router.group(gid).unwrap();
        if g.event_definition.trainer_def.is_some() {
            total_mons += g.event_definition.get_pokemon_list(&gen, false).map(|l| l.len()).unwrap_or(0);
        }
    }
    println!("get_pokemon_list over all trainers: {} ({} mons)", ms(t.elapsed()), total_mons);

    // get_label over every group
    let t = Instant::now();
    for gid in ctrl.router.all_groups() {
        let g = ctrl.router.group(gid).unwrap();
        let _ = g.event_definition.get_label(&gen);
    }
    println!("get_label over all groups: {}", ms(t.elapsed()));

    // clone every group's definition
    let t = Instant::now();
    let mut n = 0;
    for gid in ctrl.router.all_groups() {
        let g = ctrl.router.group(gid).unwrap();
        let c = g.event_definition.clone();
        n += c.notes.len();
    }
    println!("clone every group definition: {} ({})", ms(t.elapsed()), n);

    // clone every item's final state
    let t = Instant::now();
    let mut n = 0;
    for gid in ctrl.router.all_groups() {
        let g = ctrl.router.group(gid).unwrap();
        for it in &g.event_items {
            if let Some(item) = ctrl.router.item(*it) {
                if let Some(fs) = &item.final_state {
                    let c: xpr_engine::RouteState = (**fs).clone();
                    n += c.solo_pkmn.cur_level;
                }
            }
        }
    }
    println!("clone every item RouteState: {} ({})", ms(t.elapsed()), n);

    // defeat_pkmn over all trainer mons from their init states
    let t = Instant::now();
    let mut n = 0;
    for gid in ctrl.router.all_groups() {
        let g = ctrl.router.group(gid).unwrap();
        if g.event_definition.trainer_def.is_none() {
            continue;
        }
        let Some(init) = &g.init_state else { continue };
        let mons = g.event_definition.get_pokemon_list(&gen, false).unwrap_or_default();
        let mut st = init.clone();
        for (_, m) in &mons {
            if let Ok((next, _)) = st.defeat_pkmn(&gen, m, None, 1, 0) {
                st = Arc::new(next);
                n += 1;
            }
        }
    }
    println!("defeat_pkmn over all trainer mons: {} ({} mons)", ms(t.elapsed()), n);

    // inventory events from their init states
    let t = Instant::now();
    let mut n = 0;
    for gid in ctrl.router.all_groups() {
        let g = ctrl.router.group(gid).unwrap();
        let Some(i) = &g.event_definition.item_event_def else { continue };
        let Some(init) = &g.init_state else { continue };
        let (st, _) = if i.is_acquire {
            init.add_item(&gen, &i.item_name, i.item_amount, i.with_money, i.custom_price)
        } else {
            init.remove_item(&gen, &i.item_name, i.item_amount, i.with_money, i.custom_price)
        };
        n += st.inventory.cur_items.len();
    }
    println!("add/remove_item over all inventory events: {} ({})", ms(t.elapsed()), n);

    // get_item_label over every group
    let t = Instant::now();
    for gid in ctrl.router.all_groups() {
        let g = ctrl.router.group(gid).unwrap();
        let _ = g.event_definition.get_item_label(&gen);
    }
    println!("get_item_label over all groups: {}", ms(t.elapsed()));

    // SoloPokemon clone per item
    let t = Instant::now();
    let mut n = 0;
    for gid in ctrl.router.all_groups() {
        let g = ctrl.router.group(gid).unwrap();
        if let Some(fs) = &g.final_state {
            let c = fs.solo_pkmn.clone();
            n += c.cur_level;
        }
    }
    println!("clone SoloPokemon per group: {} ({})", ms(t.elapsed()), n);
}
