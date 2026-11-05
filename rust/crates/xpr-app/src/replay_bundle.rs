//! The replay tools a released router fetches for itself, so a download can
//! make a route from a replay with nothing else installed: the replay worker,
//! the mapper host (a self-contained .NET program) and the mappers, as one zip
//! (`tools/replay_to_route/bundle.py`).
//!
//! They are not in the executable, so an update that leaves them alone stays
//! small. The zip is a release asset named after its content, and
//! `rust/replay_tools.json` is built in to say which one this version needs.
//! The first import downloads it and unpacks it into
//! `<config dir>/replay_tools/<bundle name>/`. Later imports, and updates that
//! name the same bundle, reuse that folder. An update that names a new bundle
//! fetches it at the next import, which removes the old one.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Deserialize;
use xpr_update::assets;

/// Which bundle this version uses.
const LOCK: &str = include_str!("../../../replay_tools.json");

/// Written last: a folder with this file in it was unpacked completely.
const COMPLETE: &str = ".complete";

#[derive(Deserialize)]
struct Bundle {
    asset: String,
    sha256: String,
    bytes: u64,
}

/// The unpacked tools, downloading them if this version's aren't here yet, or
/// `None` on a platform that has none. `progress(phase, done, total)`.
pub fn ensure(config_dir: &Path, cancel: &AtomicBool, progress: &mut dyn FnMut(&str, u64, u64)) -> Result<Option<PathBuf>, String> {
    if !cfg!(windows) {
        return Ok(None);
    }
    let bundle: Bundle = serde_json::from_str(LOCK).map_err(|e| format!("replay_tools.json: {e}"))?;
    ensure_in(&bundle, &config_dir.join("replay_tools"), cancel, progress).map(Some)
}

fn ensure_in(bundle: &Bundle, root: &Path, cancel: &AtomicBool, progress: &mut dyn FnMut(&str, u64, u64)) -> Result<PathBuf, String> {
    let name = bundle.asset.strip_suffix(".zip").unwrap_or(&bundle.asset);
    let dest = root.join(name);
    if dest.join(COMPLETE).is_file() {
        return Ok(dest);
    }
    let failed = |e: String| format!("Route from Replay downloads its tools ({} MB) the first time it runs, and that failed: {e}", bundle.bytes / 1_000_000);
    if !assets::downloads_enabled() {
        return Err(failed("downloads are turned off".to_string()));
    }
    let bytes = bundle.bytes;
    let zip = assets::fetch_asset(&bundle.asset, &bundle.sha256, bytes, root, cancel, &mut |done, total| progress("download", done, total.unwrap_or(bytes))).map_err(|e| {
        if cancel.load(Ordering::Relaxed) {
            xpr_replay::pipeline::CANCELLED.to_string()
        } else {
            failed(e)
        }
    })?;
    progress("unpack", 0, 0);
    log::info!("[Replay Import] unpacking the replay tools into {}", dest.display());
    unpack(&zip, name, root).map_err(|e| format!("could not unpack the replay tools into {}: {e}", root.display()))?;
    let _ = std::fs::remove_file(&zip);
    prune(root, name);
    Ok(dest)
}

fn unpack(zip: &Path, name: &str, root: &Path) -> Result<(), String> {
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let tmp = root.join(format!("{name}.{nonce}.tmp"));
    let result = extract(zip, &tmp).and_then(|()| place(&tmp, &root.join(name)));
    if tmp.exists() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    result
}

fn extract(zip: &Path, dest: &Path) -> Result<(), String> {
    let file = std::fs::File::open(zip).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    archive.extract(dest).map_err(|e| e.to_string())?;
    std::fs::write(dest.join(COMPLETE), b"").map_err(|e| e.to_string())
}

/// Move the finished folder into place. An antivirus scanning the new programs
/// can hold them open for a moment, which fails a rename on Windows. Another
/// copy of the app may also have got there first, which is just as good. A
/// folder without the marker was left by something else and is replaced.
fn place(tmp: &Path, dest: &Path) -> Result<(), String> {
    let mut last = String::new();
    for attempt in 0..20 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        if dest.join(COMPLETE).is_file() {
            return Ok(());
        }
        if dest.exists() {
            let _ = std::fs::remove_dir_all(dest);
        }
        match std::fs::rename(tmp, dest) {
            Ok(()) => return Ok(()),
            Err(e) => last = e.to_string(),
        }
    }
    Err(last)
}

/// Remove other bundles and anything an interrupted download or unpack left.
/// A folder still in use (another copy of the app is importing) stays until a
/// later import.
fn prune(root: &Path, keep: &str) {
    let Ok(rd) = std::fs::read_dir(root) else { return };
    for entry in rd.flatten() {
        if entry.file_name().to_string_lossy() == keep {
            continue;
        }
        let path = entry.path();
        let removed = if path.is_dir() { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
        if let Err(e) = removed {
            log::info!("[Replay Import] left {}: {}", path.display(), e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::io::{Cursor, Write};

    fn zip_bytes() -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default();
        w.start_file("xpr-replay-worker.exe", opts).unwrap();
        w.write_all(b"worker").unwrap();
        w.start_file("mappers/STANDARD/gen1/pokemon_yellow.xml", opts).unwrap();
        w.write_all(b"<mapper/>").unwrap();
        w.finish().unwrap().into_inner()
    }

    /// A bundle already downloaded into `root`, so the tests need no server
    /// (`xpr_update::assets` tests the download itself).
    fn downloaded(root: &Path, asset: &str, body: &[u8]) -> Bundle {
        std::fs::write(root.join(asset), body).unwrap();
        let sha256 = Sha256::digest(body).iter().map(|b| format!("{:02x}", b)).collect();
        Bundle { asset: asset.to_string(), sha256, bytes: body.len() as u64 }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xpr_replay_bundle_{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn names(root: &Path) -> Vec<String> {
        std::fs::read_dir(root).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect()
    }

    #[test]
    fn the_shipped_lock_file_names_a_bundle() {
        let bundle: Bundle = serde_json::from_str(LOCK).unwrap();
        assert!(bundle.asset.starts_with("replay_tools_windows_") && bundle.asset.ends_with(".zip"), "{}", bundle.asset);
        assert_eq!(bundle.sha256.len(), 64);
        assert!(bundle.asset.contains(&bundle.sha256[..16]), "the name carries the content's hash");
    }

    #[test]
    fn unpacks_once_and_drops_the_previous_bundle() {
        let root = scratch("unpack");
        let cancel = AtomicBool::new(false);
        std::fs::create_dir_all(root.join("replay_tools_windows_old")).unwrap();
        std::fs::create_dir_all(root.join("replay_tools_windows_new.1.tmp")).unwrap();
        std::fs::write(root.join("replay_tools_windows_new.zip.1.part"), b"partial").unwrap();
        let bundle = downloaded(&root, "replay_tools_windows_new.zip", &zip_bytes());

        let mut phases = Vec::new();
        let dest = ensure_in(&bundle, &root, &cancel, &mut |p, _, _| phases.push(p.to_string())).unwrap();
        assert_eq!(phases, ["unpack"]);
        assert_eq!(std::fs::read(dest.join("xpr-replay-worker.exe")).unwrap(), b"worker");
        assert!(dest.join("mappers/STANDARD/gen1/pokemon_yellow.xml").is_file());
        assert_eq!(names(&root), ["replay_tools_windows_new"], "only the unpacked bundle stays");

        // the next import uses the folder as it is, and fetches nothing
        std::fs::write(dest.join("xpr-replay-worker.exe"), b"kept").unwrap();
        let mut called = false;
        ensure_in(&bundle, &root, &cancel, &mut |_, _, _| called = true).unwrap();
        assert!(!called);
        assert_eq!(std::fs::read(dest.join("xpr-replay-worker.exe")).unwrap(), b"kept");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_folder_left_half_unpacked_is_replaced() {
        let root = scratch("partial");
        let cancel = AtomicBool::new(false);
        // no COMPLETE marker: this is not a finished folder
        std::fs::create_dir_all(root.join("replay_tools_windows_a")).unwrap();
        std::fs::write(root.join("replay_tools_windows_a").join("stray"), b"x").unwrap();
        let bundle = downloaded(&root, "replay_tools_windows_a.zip", &zip_bytes());
        let dest = ensure_in(&bundle, &root, &cancel, &mut |_, _, _| {}).unwrap();
        assert!(dest.join(COMPLETE).is_file());
        assert!(!dest.join("stray").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_bad_zip_is_reported_and_not_used() {
        let root = scratch("bad");
        let cancel = AtomicBool::new(false);
        let bundle = downloaded(&root, "replay_tools_windows_b.zip", b"not a zip");
        let err = ensure_in(&bundle, &root, &cancel, &mut |_, _, _| {}).unwrap_err();
        assert!(err.contains("could not unpack"), "{err}");
        assert!(!root.join("replay_tools_windows_b").join(COMPLETE).exists());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
