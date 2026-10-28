//! Data too big to ship inside the executable, published as release assets
//! of their own so an update downloads them only when they changed: the
//! gen 4/5 map pictures (one zip per game, 50–95 MB each).
//!
//! An asset is named after its content, `imagery_<game>_<sha256 prefix>.zip`
//! (the sha256 is the one the game's map pack manifest records), so:
//! - the app looks for the exact file its packs expect on whichever release
//!   first shipped it, and a release only needs the pictures that changed;
//! - the downloaded copy in [`imagery_cache_dir`] keeps its asset name, and
//!   stays valid across every update that leaves that game's pictures alone.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{download, fetch_json, releases_url};

pub const IMAGERY_PREFIX: &str = "imagery_";
/// How much of the sha256 goes into the asset name.
const SHA_PREFIX_LEN: usize = 16;
/// Release pages (100 each) searched for an asset before giving up.
const MAX_RELEASE_PAGES: u32 = 10;

/// `imagery_<game>_<first 16 hex digits of sha256>.zip`
pub fn imagery_asset_name(game: &str, sha256: &str) -> String {
    format!("{}{}_{}.zip", IMAGERY_PREFIX, game, &sha256[..sha256.len().min(SHA_PREFIX_LEN)])
}

/// Where downloaded map pictures are kept: `<config dir>/map_imagery/`, or
/// `XPR_MAP_IMAGERY_DIR`.
pub fn imagery_cache_dir(config_dir: &Path) -> PathBuf {
    match std::env::var_os("XPR_MAP_IMAGERY_DIR") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => config_dir.join("map_imagery"),
    }
}

/// False when `XPR_NO_MAP_DOWNLOAD` is set (tests, offline smoke runs).
pub fn downloads_enabled() -> bool {
    std::env::var_os("XPR_NO_MAP_DOWNLOAD").is_none_or(|v| v.is_empty() || v == "0")
}

/// The download URL and size of the asset called `name`, on any release.
pub fn find_asset(name: &str) -> Result<(String, u64), String> {
    for page in 1..=MAX_RELEASE_PAGES {
        let data = fetch_json(&format!("{}?per_page=100&page={}", releases_url(), page))?;
        if data.as_array().is_none_or(|a| a.is_empty()) {
            break;
        }
        if let Some(found) = find_asset_in(&data, name) {
            return Ok(found);
        }
    }
    Err(format!("{} is not on any release", name))
}

/// [`find_asset`] over one page of the releases list, without the request.
pub fn find_asset_in(releases: &Value, name: &str) -> Option<(String, u64)> {
    releases
        .as_array()?
        .iter()
        .filter(|r| !r.get("draft").and_then(|d| d.as_bool()).unwrap_or(false))
        .filter_map(|r| r.get("assets")?.as_array())
        .flatten()
        .find(|a| a.get("name").and_then(|n| n.as_str()) == Some(name))
        .and_then(|a| Some((a.get("browser_download_url")?.as_str()?.to_string(), a.get("size").and_then(|s| s.as_u64()).unwrap_or(0))))
}

/// Download `game`'s map pictures (`bytes` long, `sha256`) into `cache_dir`
/// and return the zip. A copy already there that checks out is reused;
/// once the new one is in place, the game's other cached versions are removed.
pub fn download_imagery(game: &str, sha256: &str, bytes: u64, cache_dir: &Path, cancel: &AtomicBool, progress: &mut dyn FnMut(u64, Option<u64>)) -> Result<PathBuf, String> {
    let name = imagery_asset_name(game, sha256);
    let dest = cache_dir.join(&name);
    if dest.is_file() && check_file(&dest, sha256, bytes).is_ok() {
        return Ok(dest);
    }
    std::fs::create_dir_all(cache_dir).map_err(|e| format!("could not create {}: {}", cache_dir.display(), e))?;
    let (url, _) = find_asset(&name)?;
    log::info!("downloading the {} map pictures from {}", game, url);
    // a unique partial name, so a cancelled download still winding down
    // never shares a file with the retry
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let part = cache_dir.join(format!("{}.{}.part", name, nonce));
    let result = download(&url, &part, cancel, progress).and_then(|()| check_file(&part, sha256, bytes)).and_then(|()| {
        if dest.exists() {
            let _ = std::fs::remove_file(&dest);
        }
        std::fs::rename(&part, &dest).map_err(|e| e.to_string())
    });
    if let Err(e) = result {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    prune_imagery(cache_dir, game, &name);
    Ok(dest)
}

/// Remove `game`'s cached pictures other than `keep` (and stray partial downloads).
pub fn prune_imagery(cache_dir: &Path, game: &str, keep: &str) {
    let prefix = format!("{}{}_", IMAGERY_PREFIX, game);
    let Ok(rd) = std::fs::read_dir(cache_dir) else { return };
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name != keep && name.starts_with(&prefix) {
            // a file still open (Windows) stays until a later prune
            if let Err(e) = std::fs::remove_file(entry.path()) {
                log::warn!("could not remove old map pictures {}: {}", name, e);
            }
        }
    }
}

/// The file is `bytes` long and hashes to `sha256`.
pub fn check_file(path: &Path, sha256: &str, bytes: u64) -> Result<(), String> {
    let len = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if len != bytes {
        return Err(format!("the download is {} bytes; expected {}", len, bytes));
    }
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    let got: String = h.finalize().iter().map(|b| format!("{:02x}", b)).collect();
    if !got.eq_ignore_ascii_case(sha256) {
        return Err("the download's checksum does not match".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn asset_names_carry_the_content_hash() {
        assert_eq!(imagery_asset_name("platinum", "829136516a2ec3d610f8686f7a2244ca"), "imagery_platinum_829136516a2ec3d6.zip");
        assert_eq!(imagery_asset_name("x", "abc"), "imagery_x_abc.zip");
    }

    #[test]
    fn finds_an_asset_on_any_published_release() {
        let data = json!([
            {"tag_name": "v6.4a", "assets": [{"name": "windows_pkmn_xp_router_v6.4a.zip", "browser_download_url": "https://x/w.zip", "size": 1}]},
            {"tag_name": "v6.3a", "draft": true, "assets": [{"name": "imagery_platinum_aa.zip", "browser_download_url": "https://x/draft.zip", "size": 5}]},
            {"tag_name": "v6.3a", "assets": [{"name": "imagery_platinum_aa.zip", "browser_download_url": "https://x/p.zip", "size": 9}]},
        ]);
        assert_eq!(find_asset_in(&data, "imagery_platinum_aa.zip"), Some(("https://x/p.zip".to_string(), 9)));
        assert_eq!(find_asset_in(&data, "imagery_platinum_bb.zip"), None);
    }

    #[test]
    fn checks_and_prunes_cached_pictures() {
        let dir = std::env::temp_dir().join(format!("xpr_update_assets_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let body = b"pictures";
        let sha: String = Sha256::digest(body).iter().map(|b| format!("{:02x}", b)).collect();
        let keep = imagery_asset_name("platinum", &sha);
        std::fs::write(dir.join(&keep), body).unwrap();
        assert!(check_file(&dir.join(&keep), &sha, body.len() as u64).is_ok());
        assert!(check_file(&dir.join(&keep), &sha, 7).is_err(), "wrong size");
        assert!(check_file(&dir.join(&keep), &"0".repeat(64), body.len() as u64).is_err(), "wrong hash");
        for stale in ["imagery_platinum_0000.zip", "imagery_platinum_0000.zip.1.part", "imagery_diamond_pearl_0000.zip"] {
            std::fs::write(dir.join(stale), b"old").unwrap();
        }
        prune_imagery(&dir, "platinum", &keep);
        let mut left: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        left.sort();
        assert_eq!(left, ["imagery_diamond_pearl_0000.zip", keep.as_str()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A one-thread HTTP server: the releases list (one page) and the files
    /// it links to. Returns its address and a count of file requests.
    fn serve(files: Vec<(String, Vec<u8>)>) -> (String, Arc<AtomicUsize>) {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let (thread_base, thread_hits) = (base.clone(), hits.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut line = String::new();
                let mut reader = BufReader::new(&stream);
                if reader.read_line(&mut line).is_err() {
                    continue;
                }
                // the rest of the request head
                let mut h = String::new();
                while reader.read_line(&mut h).map(|n| n > 2).unwrap_or(false) {
                    h.clear();
                }
                let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
                let body: Vec<u8> = if path.starts_with("/releases") {
                    let assets: Vec<Value> = files.iter().map(|(n, b)| json!({"name": n, "size": b.len(), "browser_download_url": format!("{}/files/{}", thread_base, n)})).collect();
                    let page = if path.ends_with("page=1") { json!([{"tag_name": "v6.3a", "assets": assets}]) } else { json!([]) };
                    page.to_string().into_bytes()
                } else {
                    thread_hits.fetch_add(1, Ordering::SeqCst);
                    files.iter().find(|(n, _)| path == format!("/files/{}", n)).map(|(_, b)| b.clone()).unwrap_or_default()
                };
                let mut out = &stream;
                let _ = write!(out, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = out.write_all(&body);
            }
        });
        (base, hits)
    }

    #[test]
    fn downloads_the_pictures_once_and_drops_the_old_version() {
        let dir = std::env::temp_dir().join(format!("xpr_update_download_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let body = b"new pictures".to_vec();
        let sha: String = Sha256::digest(&body).iter().map(|b| format!("{:02x}", b)).collect();
        let name = imagery_asset_name("platinum", &sha);
        let bad_sha = "f".repeat(64);
        let bad_name = imagery_asset_name("platinum", &bad_sha);
        let (base, hits) = serve(vec![(name.clone(), body.clone()), (bad_name.clone(), b"not what the manifest says".to_vec())]);
        std::env::set_var("XPR_RELEASES_URL", format!("{}/releases", base));
        std::fs::write(dir.join(imagery_asset_name("platinum", &"0".repeat(64))), b"old").unwrap();

        let cancel = AtomicBool::new(false);
        let mut seen = 0u64;
        let got = download_imagery("platinum", &sha, body.len() as u64, &dir, &cancel, &mut |n, _| seen = n).unwrap();
        assert_eq!(got, dir.join(&name));
        assert_eq!(std::fs::read(&got).unwrap(), body);
        assert_eq!(seen, body.len() as u64);
        let names = |d: &Path| {
            let mut v: Vec<String> = std::fs::read_dir(d).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
            v.sort();
            v
        };
        assert_eq!(names(&dir), std::slice::from_ref(&name), "the old version is gone");
        assert_eq!(hits.load(Ordering::SeqCst), 1);

        // already there: nothing is fetched
        download_imagery("platinum", &sha, body.len() as u64, &dir, &cancel, &mut |_, _| {}).unwrap();
        assert_eq!(hits.load(Ordering::SeqCst), 1);

        // a file that does not match the manifest is refused and not kept
        let err = download_imagery("platinum", &bad_sha, 26, &dir, &cancel, &mut |_, _| {}).unwrap_err();
        assert!(err.contains("checksum"), "{}", err);
        assert_eq!(names(&dir), std::slice::from_ref(&name));

        // an asset no release has
        let err = download_imagery("platinum", &"1".repeat(64), 3, &dir, &cancel, &mut |_, _| {}).unwrap_err();
        assert!(err.contains("not on any release"), "{}", err);
        std::env::remove_var("XPR_RELEASES_URL");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
