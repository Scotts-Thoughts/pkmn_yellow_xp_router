//! Where the data files come from: compiled into the binary (feature
//! `embed-dex-data`) or read from `dex_data/` on disk (tests and dev builds;
//! `XPR_DEX_DATA_DIR` overrides the directory).

use std::borrow::Cow;
use std::path::PathBuf;

#[cfg(feature = "embed-dex-data")]
include!(concat!(env!("OUT_DIR"), "/embedded_dex.rs"));

#[cfg(not(feature = "embed-dex-data"))]
static FILES: &[(&str, &[u8])] = &[];
#[cfg(not(feature = "embed-dex-data"))]
static SPRITES: &[(u32, &[u8])] = &[];

/// The on-disk data directory used when nothing is embedded.
pub fn data_dir() -> PathBuf {
    match std::env::var_os("XPR_DEX_DATA_DIR") {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../dex_data"),
    }
}

pub fn is_embedded() -> bool {
    cfg!(feature = "embed-dex-data")
}

/// The bytes of a data file by its path relative to `dex_data/`.
pub fn read(rel_path: &str) -> Option<Vec<u8>> {
    if is_embedded() {
        let (_, deflated) = FILES.iter().find(|(p, _)| *p == rel_path)?;
        let mut out = Vec::new();
        std::io::Read::read_to_end(&mut flate2::read::DeflateDecoder::new(*deflated), &mut out).ok()?;
        return Some(out);
    }
    match std::fs::read(data_dir().join(rel_path)) {
        Ok(b) => Some(b),
        Err(e) => {
            log::warn!("dex data file {} unreadable: {}", rel_path, e);
            None
        }
    }
}

/// A HOME sprite (128 px WebP) by PokeAPI id.
pub fn sprite(id: u32) -> Option<Cow<'static, [u8]>> {
    if is_embedded() {
        return SPRITES.binary_search_by_key(&id, |(i, _)| *i).ok().map(|i| Cow::Borrowed(SPRITES[i].1));
    }
    std::fs::read(data_dir().join("sprites/home").join(format!("{}.webp", id))).ok().map(Cow::Owned)
}
