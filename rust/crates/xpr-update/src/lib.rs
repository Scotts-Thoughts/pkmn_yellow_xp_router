//! Port of `utils/auto_update.py`, grown into the full updater: GitHub
//! release checks, the in-app download, the swap of the running executable
//! (Windows) or `.app` bundle (macOS) once the window is gone, the one saved
//! previous version the Update menu can roll back to, and the result note
//! the next launch reports.
//!
//! The flow: the app downloads and extracts a release into [`staging_dir`]
//! while it runs ([`download_release`]), then quits with a [`PendingSwap`];
//! `main` calls [`apply_swap`] and [`restart`]. A swap first copies the
//! running version into `<config dir>/previous_version/` (replacing whatever
//! was there, so only one old version is ever kept), and a roll-back is the
//! same swap with that saved copy as the new app ([`stage_previous`]).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{json, Value};

use xpr_core::consts;
pub use xpr_core::version::{extract_version_info, get_newer_version, is_upgrade_needed, same_version};

const GITHUB_API_URL: &str = "https://api.github.com/repos/Scotts-Thoughts/pkmn_yellow_xp_router/releases";
const USER_AGENT: &str = concat!("pkmn_xp_router/", env!("CARGO_PKG_VERSION"));
/// `<config dir>/previous_version/`: the saved copy and its `version.txt`.
const PREVIOUS_DIR: &str = "previous_version";
const PREVIOUS_VERSION_FILE: &str = "version.txt";
/// `<config dir>/update_result.json`: written by the swap, shown and removed
/// by the next launch.
const RESULT_FILE: &str = "update_result.json";

/// `tag_name` and the download URL of the asset for this platform.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReleaseInfo {
    pub tag_name: Option<String>,
    pub asset_url: Option<String>,
}

/// One installable GitHub release (it has an asset for this platform).
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub tag: String,
    /// `YYYY-MM-DD`
    pub published: String,
    pub asset_url: String,
    pub asset_size: u64,
}

/// The name prefix of the release asset for the running platform.
pub fn asset_prefix() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else {
        "windows"
    }
}

/// `get_new_version_info`: `(None, None)` on any failure, as in Python.
pub fn get_new_version_info() -> ReleaseInfo {
    match fetch_json(&format!("{}/latest", GITHUB_API_URL)) {
        Ok(data) => parse_release(&data, asset_prefix()),
        Err(e) => {
            log::error!("Exception during new version info extraction: {}", e);
            ReleaseInfo::default()
        }
    }
}

/// Every published release with an asset for this platform, newest first.
pub fn list_releases() -> Result<Vec<Release>, String> {
    let data = fetch_json(&format!("{}?per_page=100", GITHUB_API_URL))?;
    Ok(parse_release_list(&data, asset_prefix()))
}

fn client(timeout: Option<std::time::Duration>) -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(std::time::Duration::from_secs(20))
        .timeout(timeout)
        .build()
        .map_err(|e| e.to_string())
}

fn fetch_json(url: &str) -> Result<Value, String> {
    let resp = client(Some(std::time::Duration::from_secs(20)))?.get(url).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<Value>().map_err(|e| e.to_string())
}

/// The asset-selection logic of `get_new_version_info`, separated for tests.
pub fn parse_release(data: &Value, prefix: &str) -> ReleaseInfo {
    let tag_name = data.get("tag_name").and_then(|t| t.as_str()).map(|s| s.to_string());
    let asset_url = platform_asset(data, prefix).and_then(|a| a.get("browser_download_url")).and_then(|u| u.as_str()).map(|s| s.to_string());
    ReleaseInfo { tag_name, asset_url }
}

fn platform_asset<'a>(release: &'a Value, prefix: &str) -> Option<&'a Value> {
    release.get("assets")?.as_array()?.iter().find(|a| a.get("name").and_then(|n| n.as_str()).unwrap_or("").starts_with(prefix))
}

/// [`list_releases`] without the request: drafts, tags that are not a
/// version and releases without a `prefix` asset are left out.
pub fn parse_release_list(data: &Value, prefix: &str) -> Vec<Release> {
    let mut out: Vec<Release> = data
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .filter(|r| !r.get("draft").and_then(|d| d.as_bool()).unwrap_or(false))
        .filter_map(|r| {
            let tag = r.get("tag_name")?.as_str()?.to_string();
            extract_version_info(&tag)?;
            let asset = platform_asset(r, prefix)?;
            Some(Release {
                tag,
                published: r.get("published_at").and_then(|p| p.as_str()).map(|p| p.chars().take(10).collect()).unwrap_or_default(),
                asset_url: asset.get("browser_download_url")?.as_str()?.to_string(),
                asset_size: asset.get("size").and_then(|s| s.as_u64()).unwrap_or(0),
            })
        })
        .collect();
    out.sort_by(|a, b| {
        if same_version(&a.tag, &b.tag) {
            std::cmp::Ordering::Equal
        } else if get_newer_version(&a.tag, &b.tag) == a.tag {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
    });
    out
}

/// `is_upgrade_possible`: a real installed binary on Windows or macOS (not a
/// `cargo run` build, which lives under a `target/` directory).
pub fn is_upgrade_possible() -> bool {
    if !(cfg!(windows) || cfg!(target_os = "macos")) {
        return false;
    }
    if std::env::var_os("XPR_DISABLE_AUTO_UPDATE").is_some() {
        return false;
    }
    match std::env::current_exe() {
        Ok(p) => !p.components().any(|c| c.as_os_str() == "target"),
        Err(_) => false,
    }
}

/// The installed program: the executable (Windows) or the `.app` bundle
/// around it (macOS).
fn current_app() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    if cfg!(target_os = "macos") {
        return exe
            .ancestors()
            .find(|p| p.extension().map(|e| e == "app").unwrap_or(false))
            .map(|p| p.to_path_buf())
            .ok_or_else(|| "not running from an .app bundle".to_string());
    }
    Ok(exe)
}

/// Where the running app is moved aside during a swap: `.<exe>.old` next to
/// the executable (Windows, which cannot delete a running exe but can rename
/// it), `<bundle>.app.old` next to the bundle (macOS).
pub fn backup_loc() -> Option<PathBuf> {
    let app = current_app().ok()?;
    if cfg!(target_os = "macos") {
        return Some(app.with_extension("app.old"));
    }
    let file_name = app.file_name()?.to_string_lossy().to_string();
    Some(app.parent()?.join(format!(".{}.old", file_name)))
}

/// Start-up cleanup: the moved-aside copy of the version that was replaced
/// and any download left behind. (The saved previous version is kept.)
pub fn auto_cleanup_old_version() {
    if !is_upgrade_possible() {
        return;
    }
    if let Some(backup) = backup_loc() {
        if backup.exists() {
            log::info!("trying to cleanup: {}", backup.display());
            // the replaced process may still be exiting; the next launch retries
            if let Err(e) = remove_path(&backup) {
                log::warn!("Failed to cleanup old version: {}", e);
            }
        }
    }
    let staging = staging_dir();
    if staging.exists() {
        let _ = remove_path(&staging);
    }
}

fn remove_path(p: &Path) -> std::io::Result<()> {
    if p.is_dir() {
        std::fs::remove_dir_all(p)
    } else {
        std::fs::remove_file(p)
    }
}

/// The scratch folder for downloads and staged roll-backs.
pub fn staging_dir() -> PathBuf {
    std::env::temp_dir().join("pkmn_xp_router_update")
}

// ---- download -------------------------------------------------------------------------------

/// Download the release zip at `zip_url` into `staging`, extract it, and
/// return the app inside (the top-level `.exe`, or `.app` on macOS).
/// `progress` gets (bytes so far, total if known); setting `cancel` stops
/// the download with an error.
pub fn download_release(zip_url: &str, staging: &Path, cancel: &AtomicBool, progress: &mut dyn FnMut(u64, Option<u64>)) -> Result<PathBuf, String> {
    if staging.exists() {
        remove_path(staging).map_err(|e| format!("could not clear {}: {}", staging.display(), e))?;
    }
    std::fs::create_dir_all(staging).map_err(|e| e.to_string())?;
    let zip_path = staging.join("download.zip");
    let extract_dir = staging.join("extracted");
    download(zip_url, &zip_path, cancel, progress)?;
    log::info!("New version downloaded");
    std::fs::create_dir_all(&extract_dir).map_err(|e| e.to_string())?;
    extract_zip(&zip_path, &extract_dir)?;
    let _ = std::fs::remove_file(&zip_path);
    log::info!("New version zip file extracted");
    find_app_in(&extract_dir).ok_or_else(|| "the downloaded zip did not contain the program".to_string())
}

fn download(url: &str, dest: &Path, cancel: &AtomicBool, progress: &mut dyn FnMut(u64, Option<u64>)) -> Result<(), String> {
    // no overall timeout: the zip is a few hundred MB
    let mut resp = client(None)?.get(url).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let total = resp.content_length();
    let mut file = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 256 * 1024];
    let mut done: u64 = 0;
    progress(0, total);
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".to_string());
        }
        let n = resp.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        progress(done, total);
    }
    if let Some(t) = total {
        if done != t {
            return Err(format!("download incomplete ({} of {} bytes)", done, t));
        }
    }
    Ok(())
}

fn extract_zip(zip_path: &Path, dest: &Path) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    archive.extract(dest).map_err(|e| e.to_string())
}

/// The top-level executable (`.exe`) or bundle (`.app`) in `dir`.
pub fn find_app_in(dir: &Path) -> Option<PathBuf> {
    let wanted = if cfg!(target_os = "macos") { "app" } else { "exe" };
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).collect();
    entries.sort();
    entries.into_iter().find(|entry| entry.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default() == wanted)
}

// ---- the saved previous version -------------------------------------------------------------

/// The version saved for roll-back, if there is one.
pub fn previous_version(config_dir: &Path) -> Option<String> {
    let dir = config_dir.join(PREVIOUS_DIR);
    find_app_in(&dir)?;
    let v = std::fs::read_to_string(dir.join(PREVIOUS_VERSION_FILE)).ok()?;
    let v = v.trim();
    (!v.is_empty()).then(|| v.to_string())
}

/// What `main` does once the window is closed: put `new_app` in place of the
/// running app, saving the running one as the previous version.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingSwap {
    pub new_app: PathBuf,
    pub to_version: String,
    /// a roll-back to the saved previous version (only changes the wording
    /// of the result)
    pub revert: bool,
}

/// Copy the saved previous version into `staging` (the swap then replaces
/// the saved copy with the running version) and describe the roll-back.
pub fn stage_previous(config_dir: &Path, staging: &Path) -> Result<PendingSwap, String> {
    let dir = config_dir.join(PREVIOUS_DIR);
    let version = previous_version(config_dir).ok_or("no previous version is saved")?;
    let app = find_app_in(&dir).ok_or("no previous version is saved")?;
    if staging.exists() {
        remove_path(staging).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(staging).map_err(|e| e.to_string())?;
    let staged = staging.join(app.file_name().ok_or("bad saved app path")?);
    copy_app(&app, &staged)?;
    Ok(PendingSwap { new_app: staged, to_version: version, revert: true })
}

/// Swap `swap.new_app` in for the running app. The running version is first
/// copied to `previous_version.tmp`, which replaces `previous_version` only
/// once the new app is in place, so a failed swap keeps the old saved copy.
/// Writes the result note either way and removes the staging folder.
pub fn apply_swap(swap: &PendingSwap, config_dir: &Path) -> Result<(), String> {
    let result = (|| -> Result<(), String> {
        if !is_upgrade_possible() {
            return Err("automatic updates are not supported for this installation".into());
        }
        let app = current_app()?;
        let tmp = config_dir.join(format!("{}.tmp", PREVIOUS_DIR));
        if tmp.exists() {
            remove_path(&tmp).map_err(|e| e.to_string())?;
        }
        std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
        let saved = (|| {
            copy_app(&app, &tmp.join(app.file_name().ok_or("bad app path")?))?;
            std::fs::write(tmp.join(PREVIOUS_VERSION_FILE), consts::APP_VERSION).map_err(|e| e.to_string())
        })();
        if let Err(e) = saved {
            let _ = remove_path(&tmp);
            return Err(format!("could not save the current version: {}", e));
        }
        if let Err(e) = swap_in(&app, &swap.new_app) {
            let _ = remove_path(&tmp);
            return Err(e);
        }
        // the new app is in place: a problem saving the old one is not a failed update
        let dir = config_dir.join(PREVIOUS_DIR);
        if dir.exists() {
            if let Err(e) = remove_path(&dir) {
                log::error!("could not replace the saved previous version: {}", e);
            }
        }
        if let Err(e) = std::fs::rename(&tmp, &dir) {
            log::error!("could not save the previous version: {}", e);
        }
        Ok(())
    })();
    match &result {
        Ok(()) => log::info!("Swapped in {} (was {})", swap.to_version, consts::APP_VERSION),
        Err(e) => log::error!("Automatic update failed: {}", e),
    }
    write_result(config_dir, &UpdateResult { ok: result.is_ok(), from: consts::APP_VERSION.to_string(), to: swap.to_version.clone(), revert: swap.revert, error: result.clone().err() });
    let _ = remove_path(&staging_dir());
    result
}

/// Move the running app aside and copy `new_app` into its place; puts the
/// running app back if the copy fails.
fn swap_in(app: &Path, new_app: &Path) -> Result<(), String> {
    let backup = backup_loc().ok_or("no backup location")?;
    if backup.exists() {
        remove_path(&backup).map_err(|e| format!("could not remove {}: {}", backup.display(), e))?;
    }
    std::fs::rename(app, &backup).map_err(|e| e.to_string())?;
    if let Err(e) = copy_app(new_app, app) {
        let _ = remove_path(app);
        let _ = std::fs::rename(&backup, app);
        return Err(format!("could not install the new version: {}", e));
    }
    Ok(())
}

/// Copy an executable, or a whole bundle directory (symlinks kept on Unix).
fn copy_app(src: &Path, dst: &Path) -> Result<(), String> {
    if src.is_dir() {
        copy_dir_all(src, dst)
    } else {
        std::fs::copy(src, dst).map(|_| ()).map_err(|e| e.to_string())
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(src).map_err(|e| e.to_string())?.flatten() {
        let ty = entry.file_type().map_err(|e| e.to_string())?;
        let target = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else if ty.is_symlink() {
            #[cfg(unix)]
            {
                let link = std::fs::read_link(entry.path()).map_err(|e| e.to_string())?;
                std::os::unix::fs::symlink(link, &target).map_err(|e| e.to_string())?;
            }
            #[cfg(not(unix))]
            std::fs::copy(entry.path(), &target).map_err(|e| e.to_string())?;
        } else {
            std::fs::copy(entry.path(), &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

// ---- result note ----------------------------------------------------------------------------

/// How the last swap went, for the next launch to report.
#[derive(Clone, Debug, PartialEq)]
pub struct UpdateResult {
    pub ok: bool,
    pub from: String,
    pub to: String,
    pub revert: bool,
    pub error: Option<String>,
}

fn write_result(config_dir: &Path, r: &UpdateResult) {
    let v = json!({"ok": r.ok, "from": r.from, "to": r.to, "revert": r.revert, "error": r.error});
    if let Err(e) = std::fs::write(config_dir.join(RESULT_FILE), v.to_string()) {
        log::error!("could not write the update result: {}", e);
    }
}

/// Read and remove the note the last swap left.
pub fn take_result(config_dir: &Path) -> Option<UpdateResult> {
    let path = config_dir.join(RESULT_FILE);
    let text = std::fs::read_to_string(&path).ok()?;
    let _ = std::fs::remove_file(&path);
    let v: Value = serde_json::from_str(&text).ok()?;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    Some(UpdateResult {
        ok: v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false),
        from: s("from"),
        to: s("to"),
        revert: v.get("revert").and_then(|x| x.as_bool()).unwrap_or(false),
        error: v.get("error").and_then(|x| x.as_str()).map(|x| x.to_string()),
    })
}

/// Relaunch the application (after a swap), detached.
pub fn restart(args: &[String]) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    log::info!("About to restart: {:?} {:?}", exe, args);
    #[cfg(target_os = "macos")]
    {
        if let Some(bundle) = exe.ancestors().find(|p| p.extension().map(|e| e == "app").unwrap_or(false)) {
            return std::process::Command::new("open")
                .arg("-n")
                .arg(bundle)
                .spawn()
                .map(|_| ())
                .map_err(|e| e.to_string());
        }
    }
    let mut cmd = std::process::Command::new(&exe);
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn picks_platform_asset() {
        let data = json!({
            "tag_name": "v6.1a",
            "assets": [
                {"name": "macos_pkmn_xp_router.zip", "browser_download_url": "https://x/mac.zip"},
                {"name": "windows_pkmn_xp_router.zip", "browser_download_url": "https://x/win.zip"},
            ]
        });
        assert_eq!(
            parse_release(&data, "windows"),
            ReleaseInfo { tag_name: Some("v6.1a".into()), asset_url: Some("https://x/win.zip".into()) }
        );
        assert_eq!(parse_release(&data, "macos").asset_url.as_deref(), Some("https://x/mac.zip"));
        assert_eq!(parse_release(&json!({}), "windows"), ReleaseInfo::default());
    }

    #[test]
    fn release_list_is_filtered_and_newest_first() {
        let win = |v: &str| json!({"name": format!("windows_pkmn_xp_router_{}.zip", v), "browser_download_url": format!("https://x/{}.zip", v), "size": 10});
        let data = json!([
            {"tag_name": "v6.1c", "published_at": "2026-10-22T18:51:00Z", "assets": [win("v6.1c")]},
            {"tag_name": "v6.10a", "published_at": "2026-12-01T00:00:00Z", "assets": [win("v6.10a")]},
            {"tag_name": "v6.2a", "published_at": "2026-11-01T00:00:00Z", "assets": [{"name": "macos_x.zip", "browser_download_url": "https://x/m.zip"}]},
            {"tag_name": "v6.3a", "draft": true, "assets": [win("v6.3a")]},
            {"tag_name": "nightly", "assets": [win("n")]},
            {"tag_name": "v6.1D", "published_at": "2026-10-30T00:00:00Z", "assets": [win("v6.1D")]},
        ]);
        let list = parse_release_list(&data, "windows");
        let tags: Vec<&str> = list.iter().map(|r| r.tag.as_str()).collect();
        assert_eq!(tags, ["v6.10a", "v6.1D", "v6.1c"]);
        assert_eq!(list[2].published, "2026-10-22");
        assert_eq!(list[2].asset_size, 10);
    }

    #[test]
    fn version_comparison() {
        assert!(is_upgrade_needed(Some("v99.0a"), consts::APP_VERSION));
        assert!(!is_upgrade_needed(Some(consts::APP_VERSION), consts::APP_VERSION));
        assert!(!is_upgrade_needed(Some(&consts::APP_VERSION.to_uppercase()), consts::APP_VERSION));
        assert!(!is_upgrade_needed(None, consts::APP_VERSION));
        assert_eq!(get_newer_version("v6.0a", "v6.0b"), "v6.0b");
        assert_eq!(get_newer_version("v6.1a", "v6.0b"), "v6.1a");
    }

    #[test]
    fn previous_version_and_result_note() {
        let dir = std::env::temp_dir().join(format!("xpr_update_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(PREVIOUS_DIR)).unwrap();
        assert_eq!(previous_version(&dir), None);
        let app_name = if cfg!(target_os = "macos") { "pkmn_xp_router.app" } else { "pkmn_xp_router.exe" };
        if cfg!(target_os = "macos") {
            std::fs::create_dir_all(dir.join(PREVIOUS_DIR).join(app_name).join("Contents")).unwrap();
        } else {
            std::fs::write(dir.join(PREVIOUS_DIR).join(app_name), b"old exe").unwrap();
        }
        assert_eq!(previous_version(&dir), None, "no version.txt yet");
        std::fs::write(dir.join(PREVIOUS_DIR).join(PREVIOUS_VERSION_FILE), "v6.0a\n").unwrap();
        assert_eq!(previous_version(&dir).as_deref(), Some("v6.0a"));

        let staging = dir.join("staging");
        let swap = stage_previous(&dir, &staging).unwrap();
        assert_eq!(swap.to_version, "v6.0a");
        assert!(swap.revert);
        assert!(swap.new_app.exists());
        // staging is a copy: the saved version is still there
        assert!(previous_version(&dir).is_some());

        let r = UpdateResult { ok: false, from: "v6.1c".into(), to: "v6.2a".into(), revert: false, error: Some("boom".into()) };
        write_result(&dir, &r);
        assert_eq!(take_result(&dir), Some(r));
        assert_eq!(take_result(&dir), None, "the note is removed once read");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
