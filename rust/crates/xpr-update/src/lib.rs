//! Port of `utils/auto_update.py`: GitHub release check, download, in-place
//! swap of the running executable (Windows) or `.app` bundle (macOS), and
//! the restart/cleanup handshake used by `main.pyw`.

use std::path::{Path, PathBuf};

use serde_json::Value;

use xpr_core::consts;
pub use xpr_core::version::{extract_version_info, get_newer_version, is_upgrade_needed};

const GITHUB_API_URL: &str = "https://api.github.com/repos/Scotts-Thoughts/pkmn_yellow_xp_router/releases/latest";
const USER_AGENT: &str = concat!("pkmn_xp_router/", env!("CARGO_PKG_VERSION"));

/// `tag_name` and the download URL of the asset for this platform.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReleaseInfo {
    pub tag_name: Option<String>,
    pub asset_url: Option<String>,
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
    match fetch_release_json() {
        Ok(data) => parse_release(&data, asset_prefix()),
        Err(e) => {
            log::error!("Exception during new version info extraction: {}", e);
            ReleaseInfo::default()
        }
    }
}

fn fetch_release_json() -> Result<Value, String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(GITHUB_API_URL).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<Value>().map_err(|e| e.to_string())
}

/// The asset-selection logic of `get_new_version_info`, separated for tests.
pub fn parse_release(data: &Value, prefix: &str) -> ReleaseInfo {
    let tag_name = data.get("tag_name").and_then(|t| t.as_str()).map(|s| s.to_string());
    let mut asset_url = None;
    if let Some(assets) = data.get("assets").and_then(|a| a.as_array()) {
        for asset in assets {
            let name = asset.get("name").and_then(|n| n.as_str()).unwrap_or("");
            if name.starts_with(prefix) {
                asset_url = asset.get("browser_download_url").and_then(|u| u.as_str()).map(|s| s.to_string());
                break;
            }
        }
    }
    ReleaseInfo { tag_name, asset_url }
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

/// `_get_backup_loc`: `<dir>/.<exe name>.old` next to the running executable.
pub fn backup_loc() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let folder = exe.parent()?;
    let file_name = exe.file_name()?.to_string_lossy().to_string();
    Some(folder.join(format!(".{}.old", file_name)))
}

/// `auto_cleanup_old_version`
pub fn auto_cleanup_old_version() {
    if !is_upgrade_possible() {
        return;
    }
    if let Some(backup) = backup_loc() {
        if backup.exists() {
            log::info!("trying to cleanup: {}", backup.display());
            if let Err(e) = remove_path(&backup) {
                log::error!("Failed to cleanup old version: {}", e);
            }
        }
    }
}

fn remove_path(p: &Path) -> std::io::Result<()> {
    if p.is_dir() {
        std::fs::remove_dir_all(p)
    } else {
        std::fs::remove_file(p)
    }
}

/// `update`: check (unless the caller already did), then download and swap.
/// `display` receives the user-facing progress messages.
pub fn update(new_version: Option<&str>, asset_url: Option<&str>, temp_dir: &Path, display: &mut dyn FnMut(&str)) -> bool {
    if !is_upgrade_possible() {
        let message = "Rejecting automatic update due to unsupported platform or installation";
        display(message);
        log::error!("{}", message);
        return false;
    }
    let (new_version, asset_url) = match (new_version, asset_url) {
        (Some(v), Some(u)) => (Some(v.to_string()), Some(u.to_string())),
        _ => {
            let info = get_new_version_info();
            (info.tag_name, info.asset_url)
        }
    };
    if is_upgrade_needed(new_version.as_deref(), consts::APP_VERSION) {
        match asset_url {
            Some(url) => return extract_and_update_code(&url, temp_dir, display),
            None => {
                let message = "No downloadable asset found for this platform";
                display(message);
                log::error!("{}", message);
                return false;
            }
        }
    }
    let message = "No upgrade necessary, ignoring request for automatic update";
    display(message);
    log::error!("{}", message);
    false
}

/// `extract_and_update_code`: download the zip into `temp_dir`, extract it,
/// find the one executable (Windows) or `.app` bundle (macOS) at the top
/// level, move the running copy aside and copy the new one into place.
pub fn extract_and_update_code(zip_url: &str, temp_dir: &Path, display: &mut dyn FnMut(&str)) -> bool {
    let temp_zip_loc = temp_dir.join("temp.zip");
    let temp_extract_loc = temp_dir.join("extracted_zip");
    let result = (|| -> Result<(), String> {
        download(zip_url, &temp_zip_loc)?;
        display("New version downloaded");
        log::info!("New version downloaded");

        if !temp_extract_loc.exists() {
            std::fs::create_dir_all(&temp_extract_loc).map_err(|e| e.to_string())?;
        }
        extract_zip(&temp_zip_loc, &temp_extract_loc)?;
        display("New version zip file extracted");
        log::info!("New version zip file extracted");

        let extracted_app_loc = find_app_in(&temp_extract_loc).ok_or_else(|| {
            log::error!("Extracted zip did not have the right structure");
            "Extracted zip did not have the right structure".to_string()
        })?;
        swap_in(&extracted_app_loc)?;
        display("Update completed");
        log::info!("Update completed");
        Ok(())
    })();
    // `finally: _cleanup()`
    log::info!("Beginning cleanup");
    if temp_zip_loc.exists() {
        let _ = std::fs::remove_file(&temp_zip_loc);
    }
    if temp_extract_loc.exists() {
        let _ = std::fs::remove_dir_all(&temp_extract_loc);
    }
    match result {
        Ok(()) => true,
        Err(e) => {
            log::error!("Automatic update failed with exception: {}", e);
            false
        }
    }
}

fn download(url: &str, dest: &Path) -> Result<(), String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| e.to_string())?;
    let mut resp = client.get(url).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let mut file = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    std::io::copy(&mut resp, &mut file).map_err(|e| e.to_string())?;
    Ok(())
}

fn extract_zip(zip_path: &Path, dest: &Path) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    archive.extract(dest).map_err(|e| e.to_string())
}

/// The top-level executable (`.exe`) or bundle (`.app`) of the extracted zip.
pub fn find_app_in(extract_dir: &Path) -> Option<PathBuf> {
    let wanted = if cfg!(target_os = "macos") { "app" } else { "exe" };
    let mut entries: Vec<PathBuf> = std::fs::read_dir(extract_dir).ok()?.flatten().map(|e| e.path()).collect();
    entries.sort();
    for entry in entries {
        let ext = entry.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        log::info!("testing: .{}", ext);
        if ext == wanted {
            return Some(entry);
        }
    }
    None
}

#[cfg(not(target_os = "macos"))]
fn swap_in(extracted_app_loc: &Path) -> Result<(), String> {
    let update_loc = std::env::current_exe().map_err(|e| e.to_string())?;
    let temp_rename_loc = backup_loc().ok_or("no backup location")?;
    // move the running app out of the way to allow "in-place" replacement
    std::fs::rename(&update_loc, &temp_rename_loc).map_err(|e| e.to_string())?;
    std::fs::copy(extracted_app_loc, &update_loc).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn swap_in(extracted_app_loc: &Path) -> Result<(), String> {
    // <bundle>.app/Contents/MacOS/<exe>
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let bundle = exe
        .ancestors()
        .find(|p| p.extension().map(|e| e == "app").unwrap_or(false))
        .ok_or("not running from an .app bundle")?
        .to_path_buf();
    let backup = bundle.with_extension("app.old");
    if backup.exists() {
        std::fs::remove_dir_all(&backup).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&bundle, &backup).map_err(|e| e.to_string())?;
    copy_dir_all(extracted_app_loc, &bundle)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(src).map_err(|e| e.to_string())?.flatten() {
        let ty = entry.file_type().map_err(|e| e.to_string())?;
        let target = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else if ty.is_symlink() {
            let link = std::fs::read_link(entry.path()).map_err(|e| e.to_string())?;
            std::os::unix::fs::symlink(link, &target).map_err(|e| e.to_string())?;
        } else {
            std::fs::copy(entry.path(), &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Relaunch the application (after a successful update), detached.
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
    fn version_comparison() {
        assert!(is_upgrade_needed(Some("v99.0a"), consts::APP_VERSION));
        assert!(!is_upgrade_needed(Some(consts::APP_VERSION), consts::APP_VERSION));
        assert!(!is_upgrade_needed(None, consts::APP_VERSION));
        assert_eq!(get_newer_version("v6.0a", "v6.0b"), "v6.0b");
        assert_eq!(get_newer_version("v6.1a", "v6.0b"), "v6.1a");
    }
}
