//! The chunk texture cache (SPEC §3.4): 512x512 world-pixel chunks per LOD
//! level are composited on a worker thread, uploaded on the UI thread under
//! a per-frame budget, and kept in an LRU capped by a byte budget. Coarse
//! levels (>= 2) are kept outside the budget so zooming out is always
//! instant after the first visit.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Arc;

use egui::{ColorImage, TextureHandle, TextureOptions};
use xpr_map::{Compositor, Pixmap, RenderOpts, Scope, CHUNK_PX};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChunkKey {
    pub scope: Scope,
    pub level: u8,
    pub cx: u32,
    pub cy: u32,
    pub night: bool,
}

impl ChunkKey {
    pub fn span(&self) -> i32 {
        CHUNK_PX << self.level
    }
    pub fn opts(&self) -> RenderOpts {
        RenderOpts { night: self.night }
    }
}

enum Job {
    SetCompositor(Arc<Compositor>, u32),
    Render(Vec<ChunkKey>, u32),
    Quit,
}

struct Entry {
    tex: TextureHandle,
    last_used: u64,
}

pub struct ChunkCache {
    entries: HashMap<ChunkKey, Entry>,
    pending: HashSet<ChunkKey>,
    tx: Sender<Job>,
    rx: Receiver<(u32, ChunkKey, Pixmap)>,
    generation: u32,
    frame: u64,
    /// budget for level 0/1 chunks (bytes of RGBA)
    budget_bytes: usize,
    pub uploads_per_frame: usize,
    last_wanted: Vec<ChunkKey>,
    stats_drawn: usize,
}

const CHUNK_BYTES: usize = (CHUNK_PX as usize) * (CHUNK_PX as usize) * 4;

pub fn texture_options() -> TextureOptions {
    TextureOptions {
        magnification: egui::TextureFilter::Nearest,
        minification: egui::TextureFilter::Linear,
        wrap_mode: egui::TextureWrapMode::ClampToEdge,
        mipmap_mode: Some(egui::TextureFilter::Linear),
    }
}

impl ChunkCache {
    pub fn new(budget_mb: usize) -> ChunkCache {
        let (tx, job_rx) = channel::<Job>();
        let (res_tx, rx) = channel::<(u32, ChunkKey, Pixmap)>();
        std::thread::Builder::new()
            .name("map-compositor".into())
            .spawn(move || worker(job_rx, res_tx))
            .expect("spawn map worker");
        ChunkCache {
            entries: HashMap::new(),
            pending: HashSet::new(),
            tx,
            rx,
            generation: 0,
            frame: 0,
            budget_bytes: budget_mb.max(32) * 1024 * 1024,
            uploads_per_frame: 4,
            last_wanted: Vec::new(),
            stats_drawn: 0,
        }
    }

    /// Switch to a new game's compositor; everything cached is dropped.
    pub fn set_compositor(&mut self, comp: Arc<Compositor>) {
        self.generation = self.generation.wrapping_add(1);
        self.entries.clear();
        self.pending.clear();
        self.last_wanted.clear();
        let _ = self.tx.send(Job::SetCompositor(comp, self.generation));
    }

    pub fn clear(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.entries.clear();
        self.pending.clear();
        self.last_wanted.clear();
    }

    pub fn begin_frame(&mut self) {
        self.frame += 1;
        self.stats_drawn = 0;
    }

    pub fn has(&self, key: &ChunkKey) -> bool {
        self.entries.contains_key(key)
    }

    /// The texture of a ready chunk, marking it used this frame.
    pub fn get(&mut self, key: &ChunkKey) -> Option<&TextureHandle> {
        self.stats_drawn += 1;
        let frame = self.frame;
        self.entries.get_mut(key).map(|e| {
            e.last_used = frame;
            &e.tex
        })
    }

    pub fn drawn_this_frame(&self) -> usize {
        self.stats_drawn
    }

    pub fn is_pending(&self, key: &ChunkKey) -> bool {
        self.pending.contains(key)
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Ask the worker for these chunks (highest priority first). Sends only
    /// when the wanted set changed, so a static view costs nothing.
    pub fn request(&mut self, wanted: Vec<ChunkKey>) {
        let wanted: Vec<ChunkKey> = wanted.into_iter().filter(|k| !self.entries.contains_key(k)).collect();
        if wanted.is_empty() || wanted == self.last_wanted {
            return;
        }
        for k in &wanted {
            self.pending.insert(*k);
        }
        self.last_wanted = wanted.clone();
        let _ = self.tx.send(Job::Render(wanted, self.generation));
    }

    /// Upload finished chunks (at most `uploads_per_frame`). Returns true if
    /// anything is still pending (the caller keeps repainting).
    pub fn poll(&mut self, ctx: &egui::Context) -> bool {
        let mut uploaded = 0;
        while uploaded < self.uploads_per_frame {
            match self.rx.try_recv() {
                Ok((gen, key, px)) => {
                    self.pending.remove(&key);
                    if gen != self.generation {
                        continue;
                    }
                    let image = ColorImage::from_rgba_unmultiplied([px.w, px.h], &px.data);
                    let name = format!("map:{:?}:{}:{}:{}:{}", key.scope, key.level, key.cx, key.cy, key.night as u8);
                    let tex = ctx.load_texture(name, image, texture_options());
                    self.entries.insert(key, Entry { tex, last_used: self.frame });
                    uploaded += 1;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        self.evict();
        !self.pending.is_empty()
    }

    fn evict(&mut self) {
        let fine: Vec<(ChunkKey, u64)> = self.entries.iter().filter(|(k, _)| k.level <= 1).map(|(k, e)| (*k, e.last_used)).collect();
        let bytes = fine.len() * CHUNK_BYTES;
        if bytes <= self.budget_bytes {
            return;
        }
        let mut victims = fine;
        victims.sort_by_key(|(_, used)| *used);
        let to_free = (bytes - self.budget_bytes) / CHUNK_BYTES + 1;
        for (k, used) in victims.into_iter().take(to_free) {
            if used < self.frame {
                self.entries.remove(&k);
            }
        }
    }

    pub fn cached_count(&self) -> usize {
        self.entries.len()
    }
}

impl Drop for ChunkCache {
    fn drop(&mut self) {
        let _ = self.tx.send(Job::Quit);
    }
}

fn worker(rx: Receiver<Job>, tx: Sender<(u32, ChunkKey, Pixmap)>) {
    let mut comp: Option<(Arc<Compositor>, u32)> = None;
    let mut queue: VecDeque<ChunkKey> = VecDeque::new();
    let mut done: HashSet<(u32, ChunkKey)> = HashSet::new();
    loop {
        // take the newest instructions; a new render list replaces the queue
        let msg = if queue.is_empty() {
            match rx.recv() {
                Ok(m) => Some(m),
                Err(_) => return,
            }
        } else {
            match rx.try_recv() {
                Ok(m) => Some(m),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => return,
            }
        };
        match msg {
            Some(Job::Quit) => return,
            Some(Job::SetCompositor(c, gen)) => {
                comp = Some((c, gen));
                queue.clear();
                done.clear();
                continue;
            }
            Some(Job::Render(list, gen)) => {
                // the new list goes first (it is what is visible now); older
                // requests keep their place behind it so nothing is dropped
                let old: Vec<ChunkKey> = queue.drain(..).collect();
                for k in list.iter().chain(old.iter()) {
                    if !done.contains(&(gen, *k)) && !queue.contains(k) {
                        queue.push_back(*k);
                    }
                }
                continue;
            }
            None => {}
        }
        let Some(key) = queue.pop_front() else { continue };
        let Some((c, gen)) = &comp else { continue };
        if done.contains(&(*gen, key)) {
            continue;
        }
        let px = c.render_chunk(key.scope, key.level, key.cx, key.cy, key.opts());
        done.insert((*gen, key));
        if done.len() > 4096 {
            done.clear();
        }
        if tx.send((*gen, key, px)).is_err() {
            return;
        }
    }
}
