//! Optional embedded copy of `map_data/` (feature `embed-map-data`), so a
//! packaged binary carries the map pack. `build.rs` deflates every file and
//! generates the table; files are inflated on demand.

#[cfg(feature = "embed-map-data")]
include!(concat!(env!("OUT_DIR"), "/embedded_files.rs"));

#[cfg(not(feature = "embed-map-data"))]
static FILES: &[(&str, &[u8])] = &[];

/// Bytes of an embedded file by its path relative to `map_data/` (forward slashes).
pub fn lookup(rel_path: &str) -> Option<Vec<u8>> {
    let (_, deflated) = FILES.iter().find(|(p, _)| *p == rel_path)?;
    let mut out = Vec::new();
    std::io::Read::read_to_end(&mut flate2::read::DeflateDecoder::new(*deflated), &mut out).ok()?;
    Some(out)
}

pub fn contains(rel_path: &str) -> bool {
    FILES.iter().any(|(p, _)| *p == rel_path)
}

/// File names directly inside the embedded directory `rel_dir` (sorted).
pub fn list_dir(rel_dir: &str) -> Vec<String> {
    let prefix = if rel_dir.is_empty() { String::new() } else { format!("{}/", rel_dir.trim_end_matches('/')) };
    let mut names: Vec<String> = FILES
        .iter()
        .filter_map(|(p, _)| p.strip_prefix(prefix.as_str()))
        .filter(|rest| !rest.contains('/'))
        .map(|s| s.to_string())
        .collect();
    names.sort();
    names
}

pub fn is_embedded() -> bool {
    cfg!(feature = "embed-map-data")
}

/// The games with an embedded pack (top-level directories of the table).
pub fn embedded_games() -> Vec<String> {
    let mut games: Vec<String> = FILES.iter().filter_map(|(p, _)| p.split('/').next().map(|s| s.to_string())).collect();
    games.sort();
    games.dedup();
    games
}
