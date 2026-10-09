//! A route from a Super Shuckie replay: the landing page's "Start Recording"
//! and the recorder, driven offline by the property changes `xpr_replay`
//! reads out of the replay, on the replay's clock (`xpr_recorder::clock`).
//!
//! The replay is replayed exactly as it was played: Quick Start waits for
//! the first Pokémon and creates the route, the generation's recorder adds
//! the events, and everything the recorder does with the route is what it
//! does live. Only the source of the property changes differs.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use xpr_core::{Config, Paths};
use xpr_data::Registry;
use xpr_recorder::clock;
use xpr_recorder::controller::Session;
use xpr_recorder::host::{HostHandle, RecorderHost, StartInfo};
use xpr_recorder::offline::OfflineClient;
use xpr_recorder::{starter, QuickStartPhase, RecorderController, StarterWatch};
use xpr_replay::{Item, ReplayInfo, Tools};

use crate::controller::MainController;
use crate::recorder_glue::OwnedHost;

/// How often the GameHook client's read loop runs `on_idle` while the hub is
/// quiet (its read timeout).
const IDLE_TICK_MS: u64 = 500;

#[derive(Clone, Debug, Default)]
pub struct ImportOptions {
    pub replay: PathBuf,
    /// the ROM (default: found by checksum, see `xpr_replay::rom_search_dirs`)
    pub rom: Option<PathBuf>,
    pub rom_dirs: Vec<PathBuf>,
    /// the helper programs (default: `Tools::locate`)
    pub tools_dir: Option<PathBuf>,
    /// mapper path under the mapper folder (default: from the ROM header)
    pub mapper: Option<String>,
    /// router version (default: from the ROM header)
    pub version: Option<String>,
    /// start from the Pokémon in slot 1 when the replay does not begin with a
    /// new game (default: wait for a new game, as Quick Start does)
    pub accept_current: bool,
    pub pipeline: xpr_replay::Options,
    /// set from another thread to stop the import
    pub cancel: Option<Arc<std::sync::atomic::AtomicBool>>,
}

/// What `import_replay` returns when `ImportOptions::cancel` stopped it.
pub use xpr_replay::pipeline::CANCELLED;

/// Puts the recorder clock of this thread back on the wall clock.
struct OfflineGuard;

impl Drop for OfflineGuard {
    fn drop(&mut self) {
        clock::set_offline(false);
    }
}

#[derive(Clone, Debug, Default)]
pub struct ImportReport {
    pub version: String,
    pub mapper: String,
    pub rom: PathBuf,
    pub starter: String,
    pub events: usize,
    pub frames: u64,
    pub summary: xpr_replay::Summary,
    pub messages: Vec<String>,
    pub stopped_at_final_trainer: bool,
    /// the route's solo Pokémon against the game's party where recording stopped
    pub check: Vec<String>,
    pub check_passed: bool,
}

enum Phase {
    Waiting,
    Starter(OfflineClient<StarterWatch>),
    Recording(OfflineClient<Session>),
    Done,
}

/// Record `opts.replay` into a new route. Returns the controller holding it
/// (unsaved) and a report. `progress(phase, frame, total)`.
pub fn import_replay(
    registry: Arc<Registry>,
    paths: Paths,
    cfg: Config,
    opts: &ImportOptions,
    progress: &mut dyn FnMut(&str, u64, u64),
) -> Result<(MainController, ImportReport), String> {
    let tools = Tools::locate(opts.tools_dir.as_deref())?;
    let info = ReplayInfo::from_json(&xpr_replay::procs::replay_info(&tools.worker, &opts.replay)?)?;
    let rom = match &opts.rom {
        Some(r) => r.clone(),
        None => xpr_replay::pipeline::locate_rom(&tools, &opts.replay, &xpr_replay::rom_search_dirs(&opts.replay, &opts.rom_dirs))?,
    };
    let rom_bytes = std::fs::read(&rom).map_err(|e| format!("cannot read {}: {e}", rom.display()))?;
    let choice = xpr_replay::choose_game(&rom_bytes);
    let mapper = match (&opts.mapper, &choice) {
        (Some(m), _) => m.clone(),
        (None, Ok(c)) => c.mapper.to_string(),
        (None, Err(e)) => return Err(e.clone()),
    };
    let version_hint = opts.version.clone().or_else(|| choice.as_ref().ok().map(|c| c.version.to_string()));
    log::info!("[Replay Import] {} ({} frames, ROM {}), mapper {}", opts.replay.display(), info.total_frames, rom.display(), mapper);

    clock::set_offline(true);
    clock::set_offline_ms(0);
    clock::set_offline_timer_ms(None);
    let _guard = OfflineGuard;
    let cancelled = || opts.cancel.as_ref().map(|c| c.load(std::sync::atomic::Ordering::Relaxed)).unwrap_or(false);

    let host: Arc<Mutex<Box<dyn RecorderHost + Send>>> = Arc::new(Mutex::new(Box::new(OwnedHost { ctrl: MainController::new(registry.clone(), paths.clone()), cfg: cfg.clone() })));
    let handle = HostHandle::Direct(host.clone());
    let controller = RecorderController::new(handle, Arc::new(|| {}));
    let with_ctrl = |f: &mut dyn FnMut(&mut MainController)| {
        let mut guard = host.lock().unwrap();
        let owned = guard.as_any_mut().and_then(|a| a.downcast_mut::<OwnedHost>()).expect("owned host");
        f(&mut owned.ctrl);
    };

    let mut report = ImportReport { mapper: mapper.clone(), rom: rom.clone(), ..Default::default() };
    let mut phase = Phase::Waiting;
    // (events last checked, the route-minus-game exp difference then)
    let mut exp_trace: (usize, i64) = (0, 0);
    let mut final_store: Option<xpr_recorder::PropertyStore> = None;
    let mut last_ms: Option<u64> = None;
    let total = info.total_frames;
    let mut fatal: Option<String> = None;
    const STOP: &str = "\u{0}stop";

    let mut sink = |item: Item| -> Result<(), String> {
        match item {
            Item::Mapper(m) => {
                let mut client = OfflineClient::new(StarterWatch::offline(opts.accept_current), &m)?;
                client.idle();
                phase = Phase::Starter(client);
            }
            Item::Step { frame, ms, changes } => {
                if cancelled() {
                    return Err(CANCELLED.to_string());
                }
                report.frames = frame;
                // the read loop's idle passes while nothing arrived
                if let Some(prev) = last_ms {
                    let mut t = prev + IDLE_TICK_MS;
                    while t < ms {
                        clock::set_offline_ms(t);
                        match &mut phase {
                            Phase::Starter(c) => c.idle(),
                            Phase::Recording(c) => {
                                c.idle();
                                c.session_mut().pump_events();
                            }
                            _ => {}
                        }
                        t += IDLE_TICK_MS;
                    }
                }
                last_ms = Some(ms);
                clock::set_offline_ms(ms);
                clock::set_offline_timer_ms(info.run_timer_ms(ms));
                let json: Vec<serde_json::Value> = changes.iter().map(|c| c.to_json()).collect();
                match &mut phase {
                    Phase::Starter(c) => {
                        c.apply_batch(&json);
                        match c.session().current_phase() {
                            QuickStartPhase::Done(start) => {
                                let Phase::Starter(c) = std::mem::replace(&mut phase, Phase::Waiting) else { unreachable!() };
                                c.session().stop();
                                let store = c.shutdown();
                                let version = version_hint.clone().filter(|v| same_game_pair(v, &start.game.version)).unwrap_or_else(|| start.game.version.clone());
                                let gen = registry.get_version(&version).map_err(|e| format!("version {version}: {e}"))?;
                                let Some(species) = gen.pkmn_db().get_pkmn(&start.species).cloned() else {
                                    return Err(format!("the first Pokémon '{}' is not in the {} data set", start.species, version));
                                };
                                let ability_idx = if start.game.generation >= 3 { starter::resolve_ability_idx(&start.ability, &species.abilities) } else { 0 };
                                log::info!("[Replay Import] frame {frame}: creating a {version} route for {} ({})", species.name, start.summary());
                                report.starter = format!("{} ({})", species.name, start.summary());
                                report.version = version.clone();
                                let mut created = false;
                                with_ctrl(&mut |ctrl| {
                                    ctrl.create_new_route(&species.name, None, &version, Some(start.dvs.clone()), Some(ability_idx), start.nature);
                                    created = ctrl.get_version().is_some();
                                    if created {
                                        ctrl.set_record_mode(true);
                                    }
                                });
                                if !created {
                                    return Err("the route could not be created; see the log".into());
                                }
                                let start_info = {
                                    let mut sinfo = None;
                                    with_ctrl(&mut |ctrl| {
                                        sinfo = match (ctrl.get_version(), ctrl.gen()) {
                                            (Some(v), Some(gen)) => Some(StartInfo {
                                                version: v.to_string(),
                                                gen,
                                                url: "offline".to_string(),
                                                debug_mode: cfg.is_debug_mode(),
                                                dvs: ctrl.get_dvs(),
                                                solo_species: ctrl.get_final_state().map(|s| s.solo_pkmn.species_def.name.clone()),
                                            }),
                                            _ => None,
                                        };
                                    });
                                    sinfo.ok_or("the new route has no version")?
                                };
                                let session = controller.create_session(&start_info).ok_or_else(|| format!("no recorder exists for {}", start_info.version))?;
                                let mut rec = OfflineClient::from_store(session, store);
                                rec.idle();
                                rec.session_mut().pump_events();
                                phase = Phase::Recording(rec);
                            }
                            QuickStartPhase::Failed(e) => return Err(format!("Quick Start failed: {e}")),
                            QuickStartPhase::UnsupportedGame(g) => return Err(format!("the mapper's game '{g}' is not supported")),
                            QuickStartPhase::NoMapper => return Err("the mapper could not be used for Quick Start".into()),
                            _ => {}
                        }
                    }
                    Phase::Recording(c) => {
                        c.apply_batch(&json);
                        c.session_mut().pump_events();
                        let mut active = true;
                        let mut trace: Option<(usize, i64, i64)> = None;
                        with_ctrl(&mut |ctrl| {
                            active = ctrl.is_record_mode_active();
                            // the route's solo exp against the game's, whenever the route changed
                            let events = ctrl.router.all_groups().len();
                            if events != exp_trace.0 {
                                exp_trace.0 = events;
                                if let (Some(state), Some(game)) = (ctrl.get_final_state(), solo_exp(c.store(), &ctrl.get_final_state().map(|s| s.solo_pkmn.species_def.name.clone()).unwrap_or_default())) {
                                    trace = Some((events, state.solo_pkmn.cur_xp, game));
                                }
                            }
                        });
                        if let Some((events, route_xp, game_xp)) = trace {
                            let diff = route_xp - game_xp;
                            if diff != exp_trace.1 {
                                log::info!("[Replay Import] exp check at frame {frame} ({events} events): route {route_xp}, game {game_xp}, difference {diff:+} (was {:+})", exp_trace.1);
                                exp_trace.1 = diff;
                            }
                        }
                        if !active {
                            log::info!("[Replay Import] recording stopped at frame {frame} (final trainer)");
                            report.stopped_at_final_trainer = true;
                            return Err(STOP.to_string());
                        }
                    }
                    Phase::Waiting | Phase::Done => {}
                }
            }
        }
        Ok(())
    };

    // what the recording reacts to: Quick Start's slot-1 fields and the recorder's paths
    let relevant = |mapper_json: &serde_json::Value| -> Option<Vec<String>> {
        let store = xpr_recorder::PropertyStore::from_mapper(mapper_json).ok()?;
        let version = version_hint.clone()?;
        let gen = registry.get_version(&version).ok()?;
        let info = StartInfo { version, gen, url: "offline".to_string(), debug_mode: false, dvs: None, solo_species: None };
        Some(xpr_recorder::offline::watched_paths(&info, &store))
    };
    let mut prog = |p: xpr_replay::Progress| progress(p.phase, p.frame, p.total.max(total));
    let mut pipeline_opts = opts.pipeline.clone();
    pipeline_opts.cancel = opts.cancel.clone();
    let mut summary = xpr_replay::Summary::default();
    let result = xpr_replay::run(&tools, &opts.replay, &rom, &mapper, &info, &pipeline_opts, &relevant, &mut prog, &mut sink, &mut summary);
    match result {
        Ok(()) => {}
        Err(e) if e == STOP => {}
        Err(e) => fatal = Some(e),
    }
    drop(sink);
    report.summary = summary;

    // wind down as turning record mode off does
    match std::mem::replace(&mut phase, Phase::Done) {
        Phase::Recording(c) => {
            let mut c = c;
            c.session_mut().pump_events();
            let store = c.shutdown();
            controller.stop_offline_session();
            final_store = Some(store);
        }
        Phase::Starter(_) | Phase::Waiting => {
            if fatal.is_none() {
                fatal = Some("the replay ended before the first Pokémon was received (pass --accept-current to start from the party it begins with)".into());
            }
        }
        Phase::Done => {}
    }
    clock::set_offline_timer_ms(None);
    if let Some(e) = fatal {
        return Err(e);
    }
    drop(controller);

    let owned = {
        let mut guard = host.lock().unwrap();
        let o = guard.as_any_mut().and_then(|a| a.downcast_mut::<OwnedHost>()).expect("owned host");
        std::mem::replace(&mut o.ctrl, MainController::new(registry.clone(), paths.clone()))
    };
    let mut ctrl = owned;
    ctrl.set_record_mode(false);
    report.events = ctrl.router.all_groups().len();
    if let (Some(store), Some(state)) = (&final_store, ctrl.get_final_state()) {
        let (passed, lines) = check_solo_mon(store, &state.solo_pkmn);
        report.check_passed = passed;
        report.check = lines;
    }
    while let Some(m) = ctrl.get_next_message_info() {
        report.messages.push(m);
    }
    while let Some(e) = ctrl.get_next_exception_info() {
        report.messages.push(format!("error: {e}"));
    }
    Ok((ctrl, report))
}

/// The game's exp of the party member of `species` (slot 1 first).
fn solo_exp(store: &xpr_recorder::PropertyStore, species: &str) -> Option<i64> {
    let want = norm(species);
    (0..6).find_map(|slot| {
        let sp = store.resolve_case(&format!("player.team.{slot}.species")).map(|p| store.get_value(Some(&p)))?;
        if sp.as_str().map(norm).as_deref() != Some(want.as_str()) {
            return None;
        }
        ["exp", "experience", "expPoints", "exp_points"].iter().find_map(|n| store.resolve_case(&format!("player.team.{slot}.{n}"))).and_then(|p| store.get_value(Some(&p)).as_i64())
    })
}

fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

/// Compare the route's solo Pokémon with the party the game has (the slot of
/// that species, slot 1 first): species, level, experience and moves.
pub fn check_solo_mon(store: &xpr_recorder::PropertyStore, solo: &xpr_engine::state::SoloPokemon) -> (bool, Vec<String>) {
    let mut lines = Vec::new();
    let mut ok = true;
    let get = |slot: usize, names: &[&str]| -> Option<serde_json::Value> {
        names.iter().find_map(|n| store.resolve_case(&format!("player.team.{slot}.{n}"))).map(|p| store.get_value(Some(&p)))
    };
    let want = norm(&solo.species_def.name);
    let slot = (0..6).find(|&i| get(i, &["species"]).and_then(|v| v.as_str().map(norm)).as_deref() == Some(want.as_str()));
    let Some(slot) = slot else {
        lines.push(format!("the game's party has no {} (slot 1: {:?})", solo.species_def.name, get(0, &["species"])));
        return (false, lines);
    };
    let level = get(slot, &["level"]).and_then(|v| v.as_i64());
    let exp = get(slot, &["exp", "experience", "expPoints", "exp_points"]).and_then(|v| v.as_i64());
    match level {
        Some(l) if l == solo.cur_level => lines.push(format!("level {l}: matches")),
        Some(l) => {
            ok = false;
            lines.push(format!("level: game {l}, route {}", solo.cur_level));
        }
        None => lines.push("level: not in the mapper".into()),
    }
    match exp {
        Some(x) if x == solo.cur_xp => lines.push(format!("exp {x}: matches")),
        Some(x) => {
            ok = false;
            lines.push(format!("exp: game {x}, route {} ({:+})", solo.cur_xp, solo.cur_xp - x));
        }
        None => lines.push("exp: not in the mapper".into()),
    }
    let mut game_moves = Vec::new();
    for i in 0..4 {
        let v = get(slot, &[&format!("moves.{i}.move"), &format!("move{}", i + 1), &format!("moves.{i}.name")]);
        game_moves.push(v.and_then(|v| v.as_str().map(|s| s.to_string())).filter(|s| !s.is_empty() && norm(s) != "none" && norm(s) != ""));
    }
    let route_moves: Vec<Option<String>> = solo.move_list.iter().cloned().chain(std::iter::repeat(None)).take(4).collect();
    let same_order = game_moves.iter().zip(&route_moves).all(|(g, r)| g.as_deref().map(norm) == r.as_deref().map(norm));
    let sorted = |v: &[Option<String>]| {
        let mut x: Vec<String> = v.iter().flatten().map(|m| norm(m)).collect();
        x.sort();
        x
    };
    if same_order {
        lines.push(format!("moves {:?}: match", route_moves.iter().flatten().collect::<Vec<_>>()));
    } else if sorted(&game_moves) == sorted(&route_moves) {
        // the game lets the player reorder moves; the route does not track that
        lines.push(format!("moves match, in another order: game {:?}, route {:?}", game_moves.iter().flatten().collect::<Vec<_>>(), route_moves.iter().flatten().collect::<Vec<_>>()));
    } else {
        ok = false;
        lines.push(format!("moves: game {:?}, route {:?}", game_moves, route_moves));
    }
    (ok, lines)
}

/// `a` and `b` are the same game or the two games of a pair (the mapper
/// names the pair; the ROM header says which one it is).
fn same_game_pair(a: &str, b: &str) -> bool {
    use xpr_core::consts::*;
    let pairs = [
        (RED_VERSION, BLUE_VERSION),
        (GOLD_VERSION, SILVER_VERSION),
        (RUBY_VERSION, SAPPHIRE_VERSION),
        (FIRE_RED_VERSION, LEAF_GREEN_VERSION),
        (DIAMOND_VERSION, PEARL_VERSION),
        (HEART_GOLD_VERSION, SOUL_SILVER_VERSION),
    ];
    a == b || pairs.iter().any(|&(x, y)| (a == x && b == y) || (a == y && b == x))
}

/// Save the imported route to `out` (a file path).
pub fn save_route_to(ctrl: &MainController, out: &Path) -> Result<(), String> {
    let bytes = ctrl.router.save_bytes()?;
    if let Some(dir) = out.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        }
    }
    std::fs::write(out, bytes).map_err(|e| format!("cannot write {}: {e}", out.display()))
}

/// What a background import is doing (for the app's panel).
#[derive(Clone, Debug, Default)]
pub struct ImportStatus {
    pub replay: PathBuf,
    /// "scan" (reading every keyframe) or "record"
    pub phase: String,
    pub frame: u64,
    pub total: u64,
    pub started: Option<std::time::Instant>,
    /// the saved route and the report, or what went wrong
    pub done: Option<Result<(PathBuf, ImportReport), String>>,
}

/// An import on a background thread that saves the route to `out` when it is
/// done (the app's "Route from Replay").
pub struct ImportJob {
    pub status: Arc<Mutex<ImportStatus>>,
    cancel: Arc<std::sync::atomic::AtomicBool>,
}

impl ImportJob {
    pub fn spawn(registry: Arc<Registry>, paths: Paths, cfg: Config, mut opts: ImportOptions, out: PathBuf, wake: Arc<dyn Fn() + Send + Sync>) -> ImportJob {
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        opts.cancel = Some(cancel.clone());
        let status = Arc::new(Mutex::new(ImportStatus { replay: opts.replay.clone(), phase: "start".into(), started: Some(std::time::Instant::now()), ..Default::default() }));
        let st = status.clone();
        std::thread::Builder::new()
            .name("replay-import".into())
            .spawn(move || {
                let mut last = std::time::Instant::now();
                let mut progress = |phase: &str, frame: u64, total: u64| {
                    let mut s = st.lock().unwrap();
                    s.phase = phase.to_string();
                    s.frame = frame;
                    s.total = total;
                    drop(s);
                    if last.elapsed().as_millis() >= 200 {
                        last = std::time::Instant::now();
                        wake();
                    }
                };
                let result = import_replay(registry, paths, cfg, &opts, &mut progress).and_then(|(ctrl, report)| {
                    save_route_to(&ctrl, &out)?;
                    Ok((out.clone(), report))
                });
                if let Err(e) = &result {
                    log::error!("[Replay Import] {}", e);
                }
                st.lock().unwrap().done = Some(result);
                wake();
            })
            .ok();
        ImportJob { status, cancel }
    }

    pub fn cancel(&self) {
        self.cancel.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn snapshot(&self) -> ImportStatus {
        self.status.lock().unwrap().clone()
    }
}

impl ImportReport {
    /// A few lines for the user.
    pub fn summary_text(&self) -> String {
        let s = &self.summary;
        let mut text = format!(
            "{} route for {}: {} events{}.\nRead in {:.0} s ({} of {} keyframe intervals emulated frame by frame).",
            self.version,
            self.starter,
            self.events,
            if self.stopped_at_final_trainer { ", up to the final trainer" } else { "" },
            s.coarse_secs + s.final_secs,
            s.active_intervals,
            s.intervals
        );
        text.push_str(if self.check_passed { "\n\nThe route matches the game where recording stopped:" } else { "\n\nThe route does NOT match the game where recording stopped:" });
        for line in &self.check {
            text.push_str("\n  ");
            text.push_str(line);
        }
        text
    }
}
