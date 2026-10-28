//! The pictures of the gen 4/5 maps (GEN45_REQUIREMENTS §4.12): one zip per
//! game, `map_data/<game>/imagery.zip`. Releases ship them as assets of
//! their own instead of inside the executable (`xpr_update::assets`), so an
//! update only downloads the ones that changed.
//!
//! Where the pictures come from, in order:
//! 1. the zip embedded in the executable (`XPR_EMBED_MAP_IMAGERY=1` builds);
//! 2. the pack folder on disk (`map_data/<game>/` when run from source, or
//!    `XPR_MAP_DATA_DIR`): its `imagery.zip`, or `world/` + `interiors/`
//!    folders laid out like pokemap's `public/data/<game>/`;
//! 3. the download cache (`<config dir>/map_imagery/`), which [`Fetch`]
//!    fills from the release assets when none of the above has them.
//!
//! Without them the map still works, drawn as a terrain sketch.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

use xpr_map::imagery::{embedded, DirFiles, ZipFiles};
use xpr_map::{Imagery, MapPack};
use xpr_update::assets;

/// The pictures for a pack, if this build, the pack folder or the download
/// cache has them.
pub fn find_local(pack: &MapPack, source_dir: Option<&Path>, cache_dir: Option<&Path>) -> Option<Arc<Imagery>> {
    let img = pack.image.as_ref()?;
    let expected = img.imagery.as_ref().map(|i| i.bytes);
    // a zip of another export (a stale build) would put pictures in the wrong places
    let fits = |len: u64| expected.map(|b| b == len).unwrap_or(true);
    if let Some(bytes) = embedded(&pack.game).filter(|b| fits(b.len() as u64)) {
        match ZipFiles::from_static(&format!("{} (built in)", pack.game), bytes) {
            Ok(z) => return Some(Arc::new(Imagery::new(Box::new(z)))),
            Err(e) => log::warn!("built-in map pictures of {}: {}", pack.game, e),
        }
    }
    let open_zip = |zip: &Path| -> Option<Arc<Imagery>> {
        if !std::fs::metadata(zip).map(|m| fits(m.len())).unwrap_or(false) {
            return None;
        }
        match ZipFiles::open(zip) {
            Ok(z) => Some(Arc::new(Imagery::new(Box::new(z)))),
            Err(e) => {
                log::warn!("map pictures {}: {}", zip.display(), e);
                None
            }
        }
    };
    if let Some(dir) = source_dir.map(|d| d.join(&pack.game)) {
        if let Some(found) = open_zip(&dir.join("imagery.zip")) {
            return Some(found);
        }
        if let Some(f) = DirFiles::open(&dir) {
            return Some(Arc::new(Imagery::new(Box::new(f))));
        }
    }
    // the download cache only holds exact copies (named by their sha256)
    let info = img.imagery.as_ref()?;
    open_zip(&cache_dir?.join(assets::imagery_asset_name(&pack.game, &info.sha256)))
}

/// What a pack without its pictures can download: (bytes, sha256).
pub fn downloadable(pack: &MapPack) -> Option<(u64, String)> {
    let info = pack.image.as_ref()?.imagery.as_ref()?;
    Some((info.bytes, info.sha256.clone()))
}

enum Msg {
    Progress(u64, Option<u64>),
    Done(Result<PathBuf, String>),
}

/// A background download of one game's pictures into the cache.
pub struct Fetch {
    pub game: String,
    rx: Receiver<Msg>,
    cancel: Arc<AtomicBool>,
    pub done: u64,
    pub total: Option<u64>,
}

impl Fetch {
    pub fn start(ctx: &egui::Context, game: &str, bytes: u64, sha256: String, cache_dir: PathBuf) -> Fetch {
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let thread_cancel = cancel.clone();
        let game_s = game.to_string();
        let wake = ctx.clone();
        std::thread::spawn(move || {
            let ptx = tx.clone();
            let pwake = wake.clone();
            let mut last = std::time::Instant::now();
            let mut progress = move |done: u64, total: Option<u64>| {
                let _ = ptx.send(Msg::Progress(done, total));
                if last.elapsed().as_millis() >= 100 {
                    last = std::time::Instant::now();
                    pwake.request_repaint();
                }
            };
            let result = assets::download_imagery(&game_s, &sha256, bytes, &cache_dir, &thread_cancel, &mut progress);
            if let Err(e) = &result {
                log::error!("downloading the {} map pictures failed: {}", game_s, e);
            }
            let _ = tx.send(Msg::Done(result));
            wake.request_repaint();
        });
        Fetch { game: game.to_string(), rx, cancel, done: 0, total: Some(bytes) }
    }

    /// The finished download, once there is one.
    pub fn poll(&mut self) -> Option<Result<PathBuf, String>> {
        let mut finished = None;
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Progress(n, t) => {
                    self.done = n;
                    self.total = t.or(self.total);
                }
                Msg::Done(r) => finished = Some(r),
            }
        }
        finished
    }

    pub fn fraction(&self) -> f32 {
        match self.total {
            Some(t) if t > 0 => (self.done as f32 / t as f32).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }
}

impl Drop for Fetch {
    /// Switching games (or closing) stops the download.
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// The pictures in a downloaded zip.
pub fn open_downloaded(zip: &Path) -> Result<Arc<Imagery>, String> {
    ZipFiles::open(zip).map(|z| Arc::new(Imagery::new(Box::new(z))))
}
