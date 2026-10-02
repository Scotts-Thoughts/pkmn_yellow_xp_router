//! The pictures of the gen 4/5 maps (GEN45_REQUIREMENTS §4.12): one zip per
//! game, `map_data/<game>/imagery.zip`, built into the executable like the
//! gen 1–3 packs (`build.rs`: `write_map_imagery`; `main` registers them).
//!
//! Where the pictures come from, in order:
//! 1. the zip embedded in the executable;
//! 2. the pack folder on disk (`map_data/<game>/` when run from source, or
//!    `XPR_MAP_DATA_DIR`): its `imagery.zip`, or `world/` + `interiors/`
//!    folders laid out like pokemap's `public/data/<game>/`.
//!
//! Without them the map still works, drawn as a terrain sketch.

use std::path::Path;
use std::sync::Arc;

use xpr_map::imagery::{embedded, DirFiles, ZipFiles};
use xpr_map::{Imagery, MapPack};

/// The pictures for a pack, if this build or the pack folder has them.
pub fn find_local(pack: &MapPack, source_dir: Option<&Path>) -> Option<Arc<Imagery>> {
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
    let dir = source_dir?.join(&pack.game);
    let zip = dir.join("imagery.zip");
    if std::fs::metadata(&zip).map(|m| fits(m.len())).unwrap_or(false) {
        match ZipFiles::open(&zip) {
            Ok(z) => return Some(Arc::new(Imagery::new(Box::new(z)))),
            Err(e) => log::warn!("map pictures {}: {}", zip.display(), e),
        }
    }
    DirFiles::open(&dir).map(|f| Arc::new(Imagery::new(Box::new(f))))
}
