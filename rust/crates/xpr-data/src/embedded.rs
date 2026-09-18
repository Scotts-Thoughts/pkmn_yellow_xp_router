//! Optional embedded copy of `raw_pkmn_data/` (feature `embed-data`) so a
//! packaged binary needs no data files next to it. `build.rs` deflates every
//! JSON file and generates the table; files are inflated on demand. Custom
//! gens are always read from disk.

#[cfg(feature = "embed-data")]
include!(concat!(env!("OUT_DIR"), "/embedded_files.rs"));

#[cfg(not(feature = "embed-data"))]
static FILES: &[(&str, &[u8])] = &[];

/// Bytes of an embedded data file by its path relative to `raw_pkmn_data/`
/// (forward slashes).
pub fn lookup(rel_path: &str) -> Option<Vec<u8>> {
    let (_, deflated) = FILES.iter().find(|(p, _)| *p == rel_path)?;
    let mut out = Vec::new();
    std::io::Read::read_to_end(&mut flate2::read::DeflateDecoder::new(*deflated), &mut out).ok()?;
    Some(out)
}

/// Whether `rel_path` names an embedded file.
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
    cfg!(feature = "embed-data")
}
