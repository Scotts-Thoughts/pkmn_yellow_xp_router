//! Replay -> the property changes Poke-A-Byte would have sent while the run
//! was played, as fast as the machine allows.
//!
//! A replay keeps a save state every 120 frames (its keyframes). Loading one
//! needs no emulation, so:
//!
//! 1. **Coarse pass.** Every keyframe's memory goes through the mapper. An
//!    interval between two keyframes is *active* when a property that is not
//!    noise changed across it (noise: a property that changes in most
//!    intervals, like the play-time clock).
//! 2. **Final pass.** Active intervals (and `margin` around them) are emulated
//!    frame by frame (every `stride` frames go through the mapper); quiet ones
//!    are read at their keyframe only.
//!
//! Both passes are cut into chunks run in parallel by `slots` pairs of
//! processes (a replay worker and a Poke-A-Byte host). A chunk starts at a
//! quiet keyframe outside battle, with a fresh mapper instance; where two
//! chunks meet, the second's values are compared with what the stream has
//! sent so far and any difference goes out as a change of its own, so the
//! consumer sees one continuous stream in frame order.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

use serde_json::{json, Value};

use crate::change::{Change, ADDRESS, VALUE};
use crate::procs::{PabProc, WorkerMsg, WorkerProc};
use crate::tools::Tools;

/// What `xpr-replay-worker --info` reports.
#[derive(Clone, Debug)]
pub struct ReplayInfo {
    pub console: String,
    pub rom_name: String,
    pub rom_filename: String,
    pub rom_checksum: String,
    pub total_frames: u64,
    pub total_ms: u64,
    /// `(frame, ms)` where the run's timer starts / stops
    pub crop_start: Option<(u64, u64)>,
    pub crop_end: Option<(u64, u64)>,
    pub timer_offset: Option<u64>,
    pub keyframes: Vec<u64>,
}

impl ReplayInfo {
    pub fn from_json(v: &Value) -> Result<ReplayInfo, String> {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let u = |k: &str| v.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
        let pair = |k: &str| {
            let a = v.get(k)?.as_array()?;
            Some((a.first()?.as_u64()?, a.get(1)?.as_u64()?))
        };
        let keyframes: Vec<u64> = v.get("keyframes").and_then(|k| k.as_array()).ok_or("the replay has no keyframe list")?.iter().filter_map(|k| k.as_u64()).collect();
        if keyframes.is_empty() {
            return Err("the replay has no keyframes".into());
        }
        Ok(ReplayInfo {
            console: s("console"),
            rom_name: s("rom_name"),
            rom_filename: s("rom_filename"),
            rom_checksum: s("rom_checksum"),
            total_frames: u("total_frames"),
            total_ms: u("total_ms"),
            crop_start: pair("crop_start"),
            crop_end: pair("crop_end"),
            timer_offset: v.get("timer_offset").and_then(|x| x.as_u64()),
            keyframes,
        })
    }

    /// Super Shuckie's run timer at replay time `ms` (`None` before the run
    /// was started with mark-start).
    pub fn run_timer_ms(&self, ms: u64) -> Option<i64> {
        let (_, start) = self.crop_start?;
        let mut t = ms.saturating_sub(start);
        if let Some((_, end)) = self.crop_end {
            t = t.min(end.saturating_sub(start));
        }
        Some((t + self.timer_offset.unwrap_or(0)) as i64)
    }
}

#[derive(Clone, Debug)]
pub struct Options {
    /// frames between two mapper reads in an active stretch
    pub stride: u64,
    /// process pairs run at once
    pub slots: usize,
    /// quiet intervals emulated before an active one
    pub margin_before: usize,
    /// quiet intervals emulated after an active one: the recorders of gens
    /// 1-4 count game-time seconds after an event (up to 5) and must see
    /// every second tick until they are done
    pub margin_after: usize,
    /// skip the coarse pass: emulate everything
    pub dense_all: bool,
    /// read keyframes only (no emulation at all)
    pub sparse_all: bool,
    /// a watched property changing in more than this share of intervals is a
    /// clock (the play time), not a sign of something happening. Sound
    /// properties the recorders listen to for heals and saves change in most
    /// intervals and must not count as noise.
    pub noise_share: f64,
    /// properties that never make an interval active: exact paths, or a
    /// prefix ending in `*`
    pub ignore: Vec<String>,
    /// a chunk may only start where each of these properties (when the
    /// mapper has it) holds one of the values (default: `meta.state` is
    /// "Overworld" or "No Pokemon")
    pub boundary_values: Vec<(String, Vec<Value>)>,
    /// only this part of the replay (frames)
    pub frames: Option<(u64, u64)>,
    /// one chunk: a single mapper instance reads the whole replay (slow; the
    /// reference the parallel run is checked against)
    pub single_chunk: bool,
    /// set from another thread to stop (`run` then fails with [`CANCELLED`])
    pub cancel: Option<Arc<AtomicBool>>,
}

/// The error `run` stops with when `Options::cancel` is set.
pub const CANCELLED: &str = "The import was cancelled.";

impl Options {
    fn check_cancel(&self) -> Result<(), String> {
        match &self.cancel {
            Some(c) if c.load(Ordering::Relaxed) => Err(CANCELLED.to_string()),
            _ => Ok(()),
        }
    }
}

impl Default for Options {
    fn default() -> Options {
        let slots = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        Options {
            stride: 1,
            // a quarter of the cores: an import runs while the machine is used for other things
            // (each slot is two processes; a DS slot holds ~0.7 GB)
            slots: (slots / 4).clamp(1, 6),
            margin_before: 1,
            margin_after: 4,
            dense_all: false,
            sparse_all: false,
            noise_share: 0.9,
            ignore: Vec::new(),
            boundary_values: vec![("meta.state".to_string(), vec![json!("Overworld"), json!("No Pokemon")])],
            frames: None,
            single_chunk: false,
            cancel: None,
        }
    }
}

/// What the stream hands the consumer, in frame order.
#[derive(Debug)]
pub enum Item {
    /// the mapper (`GET /mapper`) with its values at the first frame read
    Mapper(Value),
    /// one mapper read: what changed (Poke-A-Byte's `PropertiesChanged`
    /// items, in mapper order)
    Step { frame: u64, ms: u64, changes: Vec<Change> },
}

#[derive(Clone, Debug)]
pub struct Progress {
    pub phase: &'static str,
    pub frame: u64,
    pub total: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Summary {
    pub intervals: usize,
    pub active_intervals: usize,
    pub coarse_secs: f64,
    pub final_secs: f64,
    pub chunks: usize,
    pub steps: u64,
    pub boundary_fixups: u64,
    pub noise: Vec<String>,
}

// ---------------------------------------------------------------------------
// running chunks in parallel

struct Job {
    index: usize,
    pieces: String,
    /// snapshots at the start that only warm the mapper up
    skip: usize,
    /// script state to seed a fresh mapper instance with (see `PabProc::export_state`)
    seed: Option<Arc<Vec<u8>>>,
}

struct StepOut {
    frame: u64,
    ms: u64,
    changes: Vec<Change>,
}

struct ChunkOut {
    steps: Vec<StepOut>,
    /// the mapper's values after the last skipped snapshot (or the first one)
    model: Option<Value>,
}

struct Ctx<'a> {
    tools: &'a Tools,
    replay: &'a Path,
    rom: &'a Path,
    mapper: &'a str,
}

/// A replay worker and a Poke-A-Byte host reading one chunk at a time.
struct Slot {
    worker: WorkerProc,
    pab: PabProc,
    fresh: bool,
}

impl Slot {
    fn start(ctx: &Ctx) -> Result<Slot, String> {
        let pab = PabProc::start(&ctx.tools.pab_host, &ctx.tools.mappers, ctx.mapper)?;
        let mut worker = WorkerProc::start(&ctx.tools.worker, ctx.replay, ctx.rom)?;
        worker.set_blocks(&pab.blocks)?;
        Ok(Slot { worker, pab, fresh: true })
    }

    fn run(&mut self, job: &Job, stop: &AtomicBool) -> Result<ChunkOut, String> {
        if !self.fresh {
            self.pab.reset()?;
        }
        self.fresh = false;
        if let Some(seed) = &job.seed {
            self.pab.import_state(seed)?;
        }
        self.worker.start_job(job.index as u64, &job.pieces)?;
        let mut steps = Vec::new();
        let mut model = None;
        let mut seen = 0usize;
        loop {
            match self.worker.next()? {
                WorkerMsg::Snapshot { frame, ms, runs } => {
                    let changes = self.pab.step(&runs)?;
                    seen += 1;
                    if seen == job.skip.max(1) {
                        model = Some(self.pab.mapper()?);
                    }
                    if seen > job.skip {
                        steps.push(StepOut { frame, ms, changes });
                    }
                    if stop.load(Ordering::Relaxed) {
                        return Err("stopped".into());
                    }
                }
                WorkerMsg::End { .. } => break,
            }
        }
        Ok(ChunkOut { steps, model })
    }
}

/// A replay worker alone: the coarse pass's keyframe reader.
struct ReaderSlot {
    worker: WorkerProc,
}

/// Keyframe memory: (frame, ms, runs as pab-host's `S` takes them).
type Snaps = Vec<(u64, u64, Vec<u8>)>;

impl ReaderSlot {
    fn run(&mut self, job: &Job, stop: &AtomicBool) -> Result<Snaps, String> {
        self.worker.start_job(job.index as u64, &job.pieces)?;
        let mut out = Vec::new();
        loop {
            match self.worker.next()? {
                WorkerMsg::Snapshot { frame, ms, runs } => {
                    out.push((frame, ms, runs));
                    if stop.load(Ordering::Relaxed) {
                        return Err("stopped".into());
                    }
                }
                WorkerMsg::End { .. } => return Ok(out),
            }
        }
    }
}

/// Run `jobs` on `slots` threads (each with a `S` made by `start` when it
/// gets its first job), handing the results to `consume` in job order. At
/// most `lookahead` results wait to be consumed.
fn run_jobs<S, T: Send>(
    jobs: Vec<Job>,
    slots: usize,
    lookahead: usize,
    start: &(dyn Fn() -> Result<S, String> + Sync),
    exec: &(dyn Fn(&mut S, &Job, &AtomicBool) -> Result<T, String> + Sync),
    consume: &mut dyn FnMut(T) -> Result<(), String>,
) -> Result<(), String> {
    let count = jobs.len();
    if count == 0 {
        return Ok(());
    }
    let queue = Mutex::new((VecDeque::from(jobs), 0usize)); // (jobs, next index allowed to start below)
    let gate = Condvar::new();
    let stop = AtomicBool::new(false);
    let (tx, rx) = mpsc::channel::<(usize, Result<T, String>)>();
    let mut result = Ok(());
    std::thread::scope(|scope| {
        for _ in 0..slots.min(count) {
            let tx = tx.clone();
            let (queue, gate, stop) = (&queue, &gate, &stop);
            scope.spawn(move || {
                let mut slot: Option<S> = None;
                loop {
                    let job = {
                        let mut q = queue.lock().unwrap();
                        loop {
                            if stop.load(Ordering::SeqCst) {
                                return;
                            }
                            let allowed = q.1;
                            match q.0.front() {
                                None => return,
                                Some(j) if j.index < allowed => break q.0.pop_front().unwrap(),
                                Some(_) => q = gate.wait(q).unwrap(),
                            }
                        }
                    };
                    log::debug!("[replay] job {} starting", job.index);
                    // a panic must still answer for its job, or the consumer waits for it forever
                    let caught = |p: Box<dyn std::any::Any + Send>| -> String {
                        let msg = p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_else(|| "unknown".into());
                        format!("a replay reader failed: {msg}")
                    };
                    if slot.is_none() {
                        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(start)).unwrap_or_else(|p| Err(caught(p))) {
                            Ok(s) => slot = Some(s),
                            Err(e) => {
                                let _ = tx.send((job.index, Err(e)));
                                return;
                            }
                        }
                    }
                    let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| exec(slot.as_mut().unwrap(), &job, stop))).unwrap_or_else(|p| Err(caught(p)));
                    let failed = out.is_err();
                    let _ = tx.send((job.index, out));
                    if failed {
                        return;
                    }
                }
            });
        }
        drop(tx);
        {
            let mut q = queue.lock().unwrap();
            q.1 = lookahead.max(slots);
        }
        gate.notify_all();
        let mut pending: BTreeMap<usize, T> = BTreeMap::new();
        let mut next = 0usize;
        while next < count {
            match rx.recv() {
                Ok((index, Ok(out))) => {
                    pending.insert(index, out);
                    while let Some(out) = pending.remove(&next) {
                        if let Err(e) = consume(out) {
                            result = Err(e);
                            break;
                        }
                        next += 1;
                        let mut q = queue.lock().unwrap();
                        q.1 = next + lookahead.max(slots);
                        gate.notify_all();
                    }
                    if result.is_err() {
                        break;
                    }
                }
                Ok((_, Err(e))) => {
                    result = Err(e);
                    break;
                }
                Err(_) => {
                    result = Err("the replay readers stopped".into());
                    break;
                }
            }
        }
        stop.store(true, Ordering::SeqCst);
        gate.notify_all();
    });
    result
}

// ---------------------------------------------------------------------------
// the passes

/// The values the stream has sent so far, to compare a new chunk's start with.
#[derive(Default)]
struct Values {
    map: HashMap<String, (Value, Option<u64>)>,
}

impl Values {
    fn load(&mut self, mapper: &Value) {
        self.map.clear();
        for p in mapper.get("properties").and_then(|p| p.as_array()).into_iter().flatten() {
            if let Some(path) = p.get("path").and_then(|x| x.as_str()) {
                self.map.insert(path.to_string(), (p.get("value").cloned().unwrap_or(Value::Null), p.get("address").and_then(|a| a.as_u64())));
            }
        }
    }

    fn apply(&mut self, changes: &[Change]) {
        for c in changes {
            let e = self.map.entry(c.path.to_string()).or_insert((Value::Null, None));
            e.0 = c.value.clone();
            e.1 = c.address;
        }
    }

    fn get(&self, path: &str) -> Option<&Value> {
        self.map.get(path).map(|v| &v.0)
    }

    /// The changes that turn these values into `mapper`'s (in mapper order).
    fn diff(&self, mapper: &Value) -> Vec<Change> {
        let mut out = Vec::new();
        for p in mapper.get("properties").and_then(|p| p.as_array()).into_iter().flatten() {
            let Some(path) = p.get("path").and_then(|x| x.as_str()) else { continue };
            let value = p.get("value").cloned().unwrap_or(Value::Null);
            let address = p.get("address").and_then(|a| a.as_u64());
            let (old_v, old_a) = self.map.get(path).cloned().unwrap_or((Value::Null, None));
            let mut fields = 0;
            if old_v != value {
                fields |= VALUE;
            }
            if old_a != address {
                fields |= ADDRESS;
            }
            if fields != 0 {
                out.push(Change { path: path.into(), address, value, fields });
            }
        }
        out
    }
}

fn path_ignored(path: &str, ignore: &[String]) -> bool {
    ignore.iter().any(|i| match i.strip_suffix('*') {
        Some(prefix) => path.starts_with(prefix),
        None => path == i,
    })
}

/// Coarse pass result: for each keyframe interval (index i: between keyframe
/// i-1 and i) the paths that changed, and for each keyframe whether a chunk
/// may start there.
struct Coarse {
    changed: Vec<Vec<u32>>,
    boundary_ok: Vec<bool>,
    paths: Vec<String>,
    /// per path: whether a change of it can matter (all, without a list)
    relevant: Vec<bool>,
    /// the coarse instance's script state at some keyframes (where chunks may start)
    seeds: HashMap<usize, Arc<Vec<u8>>>,
}

/// Keyframes between two script-state exports of the coarse pass (where a
/// chunk of the final pass may start).
const SEED_EVERY: usize = 8;

pub fn run(
    tools: &Tools,
    replay: &Path,
    rom: &Path,
    mapper: &str,
    info: &ReplayInfo,
    opts: &Options,
    relevant: &dyn Fn(&Value) -> Option<Vec<String>>,
    progress: &mut dyn FnMut(Progress),
    sink: &mut dyn FnMut(Item) -> Result<(), String>,
    summary: &mut Summary,
) -> Result<(), String> {
    let ctx = Ctx { tools, replay, rom, mapper };
    let t_start = Instant::now();
    let (lo, hi) = opts.frames.unwrap_or((0, u64::MAX));
    // keyframes in range, plus the one at or before `lo` to start from
    let first = info.keyframes.partition_point(|&k| k <= lo).saturating_sub(1);
    let last = info.keyframes.partition_point(|&k| k <= hi);
    let kf: Vec<u64> = info.keyframes[first..last].to_vec();
    if kf.is_empty() {
        return Err("no keyframes in the requested range".into());
    }
    let n = kf.len();
    let total_frames = (*kf.last().unwrap()).max(1);
    summary.intervals = n.saturating_sub(1);

    // ---- coarse pass
    let t0 = Instant::now();
    // (dense_all still scans: the scan says where chunks can start)
    let coarse = if opts.sparse_all {
        None
    } else {
        Some(coarse_pass(&ctx, &kf, opts, relevant, progress, total_frames)?)
    };
    summary.coarse_secs = t0.elapsed().as_secs_f64();

    // ---- which intervals to emulate
    let mut active = vec![opts.dense_all; n];
    active[0] = false;
    let mut boundary_ok = vec![true; n];
    // a relevant change across the interval (before the margins)
    let mut raw = vec![false; n];
    if let Some(c) = &coarse {
        let intervals = (n - 1).max(1) as f64;
        let mut counts = vec![0usize; c.paths.len()];
        for ch in &c.changed {
            for &p in ch {
                counts[p as usize] += 1;
            }
        }
        let noise: Vec<bool> = counts
            .iter()
            .enumerate()
            .map(|(i, &k)| (k as f64) / intervals > opts.noise_share || path_ignored(&c.paths[i], &opts.ignore) || !c.relevant[i])
            .collect();
        summary.noise = c.paths.iter().enumerate().filter(|(i, _)| noise[*i] && counts[*i] > 0).map(|(_, p)| p.clone()).collect();
        {
            let mut top: Vec<(usize, &String)> = counts.iter().copied().zip(c.paths.iter()).enumerate().filter(|(i, (k, _))| *k > 0 && c.relevant[*i]).map(|(_, x)| x).collect();
            top.sort_by(|x, y| y.0.cmp(&x.0));
            let line: Vec<String> = top.iter().take(60).map(|(k, p)| format!("{p}={k}")).collect();
            log::info!("[replay] {} intervals; most changed of the relevant properties: {}", n - 1, line.join(", "));
        }
        raw = c.changed.iter().map(|ch| ch.iter().any(|&p| !noise[p as usize])).collect();
        for i in 1..n {
            if raw[i] {
                let a = i.saturating_sub(opts.margin_before).max(1);
                let b = (i + opts.margin_after).min(n - 1);
                for flag in &mut active[a..=b] {
                    *flag = true;
                }
            }
        }
        boundary_ok = c.boundary_ok.clone();
    }
    summary.active_intervals = active.iter().filter(|a| **a).count();

    // ---- final pass: chunks
    let stride = opts.stride.max(1);
    let cost = |i: usize| -> u64 {
        if active[i] {
            (kf[i] - kf[i - 1]).div_ceil(stride) * 3
        } else {
            10
        }
    };
    let total_cost: u64 = (1..n).map(cost).sum::<u64>().max(1);
    let target = (total_cost / (opts.slots as u64 * 6).max(1)).clamp(2_000, 30_000);
    let mut chunks: Vec<(usize, usize)> = Vec::new(); // keyframe index ranges [a, b]
    let mut a = 0usize;
    let mut acc = 0u64;
    for i in 1..n {
        acc += cost(i);
        // a chunk starts on the overworld, where nothing the recorder watches changes
        // around it: the fresh mapper instance has nothing to catch up on there
        let seeded = coarse.as_ref().map(|c| c.seeds.contains_key(&i)).unwrap_or(true);
        let quiet_here = !raw[i] && (i + 1 >= n || !raw[i + 1]) && boundary_ok[i] && seeded;
        if acc >= target && quiet_here && i + 1 < n && !opts.single_chunk {
            chunks.push((a, i));
            a = i;
            acc = 0;
        }
    }
    chunks.push((a, n - 1));
    summary.chunks = chunks.len();

    let jobs: Vec<Job> = chunks
        .iter()
        .enumerate()
        .map(|(ci, &(a, b))| {
            // the chunk's first keyframe is its baseline; a later chunk's mapper instance starts
            // from the coarse instance's script state there
            let mut pieces: Vec<String> = vec![format!("s {} {}", kf[a], kf[a])];
            let skip = 1;
            let seed = coarse.as_ref().and_then(|c| c.seeds.get(&a).cloned());
            let mut i = a + 1;
            while i <= b {
                if active[i] {
                    let start = i;
                    while i <= b && active[i] {
                        i += 1;
                    }
                    let from = kf[start - 1] + stride;
                    let to = kf[i - 1];
                    if from <= to {
                        pieces.push(format!("d {from} {to} {stride}"));
                        if (to - from) % stride != 0 {
                            pieces.push(format!("d {to} {to} 1"));
                        }
                    }
                } else {
                    let start = i;
                    while i <= b && !active[i] {
                        i += 1;
                    }
                    pieces.push(format!("s {} {}", kf[start], kf[i - 1]));
                }
            }
            Job { index: ci, pieces: pieces.join(";"), skip, seed }
        })
        .collect();

    let t1 = Instant::now();
    let mut values = Values::default();
    let mut started = false;
    let lookahead = opts.slots * 2;
    let mut chunk_index = 0usize;
    let start_slot = || Slot::start(&ctx);
    let exec_slot = |slot: &mut Slot, job: &Job, stop: &AtomicBool| slot.run(job, stop);
    let result = run_jobs(jobs, opts.slots, lookahead, &start_slot, &exec_slot, &mut |out: ChunkOut| {
        opts.check_cancel()?;
        let index = chunk_index;
        chunk_index += 1;
        let model = out.model.ok_or("a chunk did not report its mapper")?;
        if !started {
            values.load(&model);
            sink(Item::Mapper(model))?;
            started = true;
        } else {
            let fix = values.diff(&model);
            if !fix.is_empty() {
                summary.boundary_fixups += fix.len() as u64;
                log::info!(
                    "[replay] chunk {} starts with {} differences: {}",
                    index,
                    fix.len(),
                    fix.iter().take(12).map(|c| format!("{}={}", c.path, c.value)).collect::<Vec<_>>().join(", ")
                );
                values.apply(&fix);
                let frame = out.steps.first().map(|s| s.frame).unwrap_or(0);
                let ms = out.steps.first().map(|s| s.ms).unwrap_or(0);
                sink(Item::Step { frame, ms, changes: fix })?;
            }
        }
        let mut last = 0;
        for s in out.steps {
            values.apply(&s.changes);
            last = s.frame;
            summary.steps += 1;
            sink(Item::Step { frame: s.frame, ms: s.ms, changes: s.changes })?;
        }
        progress(Progress { phase: "record", frame: last, total: total_frames });
        Ok(())
    });
    summary.final_secs = t1.elapsed().as_secs_f64();
    let _ = t_start;
    result
}

/// Every keyframe through one mapper instance, in order (the keyframes are
/// read in parallel): which properties change between keyframes, where a
/// chunk may start, and the instance's script state there.
fn coarse_pass(ctx: &Ctx, kf: &[u64], opts: &Options, relevant_of: &dyn Fn(&Value) -> Option<Vec<String>>, progress: &mut dyn FnMut(Progress), total_frames: u64) -> Result<Coarse, String> {
    let n = kf.len();
    let group = 64usize;
    let mut jobs = Vec::new();
    let mut a = 0usize;
    while a < n {
        let b = (a + group - 1).min(n - 1);
        jobs.push(Job { index: jobs.len(), pieces: format!("s {} {}", kf[a], kf[b]), skip: 0, seed: None });
        a = b + 1;
    }
    let mut pab = PabProc::start(&ctx.tools.pab_host, &ctx.tools.mappers, ctx.mapper)?;
    let blocks = pab.blocks.clone();
    let mut paths: Vec<String> = Vec::new();
    let mut relevant: Vec<bool> = Vec::new();
    let mut index: HashMap<String, u32> = HashMap::new();
    let mut changed: Vec<Vec<u32>> = vec![Vec::new(); n];
    let mut boundary_ok = vec![true; n];
    let mut seeds: HashMap<usize, Arc<Vec<u8>>> = HashMap::new();
    let mut values = Values::default();
    let mut next_kf = 0usize;
    let check_boundary = |values: &Values| -> bool {
        // (a property the mapper declares but never sets stays null)
        opts.boundary_values.iter().all(|(path, allowed)| match values.get(path) {
            Some(v) => v.is_null() || allowed.contains(v),
            None => true,
        })
    };
    let start_reader = || -> Result<ReaderSlot, String> {
        let mut worker = WorkerProc::start(&ctx.tools.worker, ctx.replay, ctx.rom)?;
        worker.set_blocks(&blocks)?;
        Ok(ReaderSlot { worker })
    };
    let exec_reader = |slot: &mut ReaderSlot, job: &Job, stop: &AtomicBool| slot.run(job, stop);
    run_jobs(jobs, opts.slots, opts.slots * 2, &start_reader, &exec_reader, &mut |snaps: Snaps| {
        opts.check_cancel()?;
        for (frame, _ms, runs) in snaps {
            // the snapshots arrive in keyframe order
            while next_kf < n && kf[next_kf] < frame {
                next_kf += 1;
            }
            if next_kf >= n {
                break;
            }
            let i = next_kf;
            let changes = pab.step(&runs)?;
            if i == 0 {
                let model = pab.mapper()?;
                values.load(&model);
                if let Some(props) = model.get("properties").and_then(|p| p.as_array()) {
                    for p in props {
                        if let Some(path) = p.get("path").and_then(|x| x.as_str()) {
                            index.insert(path.to_string(), paths.len() as u32);
                            paths.push(path.to_string());
                        }
                    }
                }
                relevant = match relevant_of(&model) {
                    Some(list) => {
                        let set: std::collections::HashSet<&str> = list.iter().map(|s| s.as_str()).collect();
                        log::info!("[replay] {} of {} properties can make a stretch active", set.len(), paths.len());
                        paths.iter().map(|p| set.contains(p.as_str())).collect()
                    }
                    None => vec![true; paths.len()],
                };
            } else {
                changed[i] = changes.iter().filter(|c| c.fields & VALUE != 0).filter_map(|c| index.get(&*c.path).copied()).collect();
                values.apply(&changes);
            }
            boundary_ok[i] = check_boundary(&values);
            if i > 0 && i % SEED_EVERY == 0 && boundary_ok[i] {
                seeds.insert(i, Arc::new(pab.export_state()?));
            }
            next_kf += 1;
        }
        progress(Progress { phase: "scan", frame: kf[next_kf.min(n - 1)], total: total_frames });
        Ok(())
    })?;
    if next_kf == 0 {
        return Err("the replay worker read no keyframes".into());
    }
    let seed_bytes: usize = seeds.values().map(|s| s.len()).sum();
    log::info!("[replay] scan: {} keyframes, {} chunk starts with script state ({} KB)", n, seeds.len(), seed_bytes / 1024);
    Ok(Coarse { changed, boundary_ok, paths, relevant, seeds })
}

/// Find a replay's ROM: in `dirs` (see `tools::rom_search_dirs`).
pub fn locate_rom(tools: &Tools, replay: &Path, dirs: &[PathBuf]) -> Result<PathBuf, String> {
    crate::procs::find_rom(&tools.worker, replay, dirs)?.ok_or_else(|| {
        format!(
            "The ROM this replay was recorded with was not found (looked in: {}). Pass it with --rom.",
            dirs.iter().map(|d| d.display().to_string()).collect::<Vec<_>>().join(", ")
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(crop_start: Option<(u64, u64)>, crop_end: Option<(u64, u64)>, offset: Option<u64>) -> ReplayInfo {
        ReplayInfo {
            console: "NDS".into(),
            rom_name: String::new(),
            rom_filename: String::new(),
            rom_checksum: String::new(),
            total_frames: 0,
            total_ms: 0,
            crop_start,
            crop_end,
            timer_offset: offset,
            keyframes: vec![0],
        }
    }

    #[test]
    fn the_run_timer_is_super_shuckies() {
        assert_eq!(info(None, None, None).run_timer_ms(5000), None);
        let i = info(Some((203, 784)), Some((1000, 10_784)), Some(500));
        assert_eq!(i.run_timer_ms(500), Some(500)); // before the start: 0 + offset
        assert_eq!(i.run_timer_ms(1784), Some(1500));
        assert_eq!(i.run_timer_ms(50_000), Some(10_500)); // stops at the end
    }

    #[test]
    fn ignore_patterns() {
        let ignore = vec!["game_time.*".to_string(), "audio.channels.3".to_string()];
        assert!(path_ignored("game_time.seconds", &ignore));
        assert!(path_ignored("audio.channels.3", &ignore));
        assert!(!path_ignored("audio.channels.4", &ignore));
    }

    #[test]
    fn a_chunk_start_is_reconciled_with_what_was_sent() {
        let mapper = json!({"properties": [
            {"path": "a", "value": 1, "address": 10},
            {"path": "b", "value": "x", "address": 20},
            {"path": "c", "value": null, "address": null},
        ]});
        let mut v = Values::default();
        v.load(&mapper);
        assert!(v.diff(&mapper).is_empty());
        v.apply(&[Change { path: "a".into(), address: Some(10), value: json!(2), fields: VALUE }]);
        let next = json!({"properties": [
            {"path": "a", "value": 1, "address": 10},
            {"path": "b", "value": "x", "address": 24},
            {"path": "c", "value": null, "address": null},
        ]});
        let fix = v.diff(&next);
        assert_eq!(fix.len(), 2);
        assert_eq!((&*fix[0].path, fix[0].value.clone(), fix[0].fields), ("a", json!(1), VALUE));
        assert_eq!((&*fix[1].path, fix[1].address, fix[1].fields), ("b", Some(24), ADDRESS));
        assert_eq!(fix[0].to_json()["fieldsChanged"], json!(["value"]));
    }
}
