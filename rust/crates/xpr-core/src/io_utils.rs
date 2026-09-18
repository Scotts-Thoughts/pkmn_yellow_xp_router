//! Port of `utils/io_utils.py`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use crate::consts;
use crate::pyjson;

/// `sanitize_string`: keep alphanumerics, lowercase.
pub fn sanitize_string(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// `get_path_safe_string`
pub fn get_path_safe_string(raw: &str) -> String {
    let kept: String = raw
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || c.is_whitespace() || *c == '-')
        .collect();
    let value = kept.trim().to_lowercase();
    let mut out = String::new();
    let mut in_run = false;
    for c in value.chars() {
        if c == '-' || c.is_whitespace() {
            if !in_run {
                out.push('-');
                in_run = true;
            }
        } else {
            out.push(c);
            in_run = false;
        }
    }
    out
}

/// `get_safe_path_no_collision`
pub fn get_safe_path_no_collision(base_folder: &Path, name: &str, ext: &str) -> PathBuf {
    let name = get_path_safe_string(name);
    let mut result = base_folder.join(format!("{}{}", name, ext));
    if result.exists() {
        let mut counter = 0;
        while result.exists() {
            counter += 1;
            result = base_folder.join(format!("{}_{}{}", name, counter, ext));
        }
    }
    result
}

/// `get_existing_route_path`
pub fn get_existing_route_path(paths: &consts::Paths, route_name: &str) -> PathBuf {
    let result = paths.saved_routes_dir.join(format!("{}.json", route_name));
    if !result.exists() {
        return paths.outdated_routes_dir.join(format!("{}.json", route_name));
    }
    result
}

/// `get_existing_route_names`
pub fn get_existing_route_names(paths: &consts::Paths, filter_text: &str, load_backups: bool) -> Vec<String> {
    let filter_text = filter_text.to_lowercase();
    let mut loaded: Vec<String> = Vec::new();
    let mut scan = |dir: &Path| {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for entry in rd.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                let (name, ext) = split_ext(&fname);
                if !name.to_lowercase().contains(&filter_text) {
                    continue;
                }
                if ext != ".json" {
                    continue;
                }
                loaded.push(name.to_string());
            }
        }
    };
    if paths.saved_routes_dir.exists() {
        scan(&paths.saved_routes_dir);
    }
    if load_backups && paths.outdated_routes_dir.exists() {
        scan(&paths.outdated_routes_dir);
    }
    loaded.sort_by_key(|s| s.to_lowercase());
    loaded
}

/// `os.path.splitext`
pub fn split_ext(fname: &str) -> (&str, &str) {
    match fname.rfind('.') {
        Some(idx) if idx > 0 && !fname[..idx].chars().all(|c| c == '.') => (&fname[..idx], &fname[idx..]),
        _ => (fname, ""),
    }
}

/// `change_user_data_location`
pub fn change_user_data_location(paths: &consts::Paths, orig_dir: Option<&Path>, new_dir: &Path) -> bool {
    let inner = || -> std::io::Result<()> {
        match orig_dir {
            Some(o) if o.exists() => {}
            _ => {
                if !new_dir.exists() {
                    std::fs::create_dir_all(new_dir)?;
                }
                return Ok(());
            }
        }
        let orig_dir = orig_dir.unwrap();
        if !new_dir.exists() {
            std::fs::create_dir_all(new_dir)?;
        }
        for (orig_inner, new_inner) in paths.potential_user_data_dirs(new_dir) {
            if orig_inner.exists() {
                copy_tree(&orig_inner, &new_inner)?;
            }
        }
        for (orig_inner, _) in paths.potential_user_data_dirs(new_dir) {
            if orig_inner.exists() {
                std::fs::remove_dir_all(&orig_inner)?;
            }
        }
        if std::fs::read_dir(orig_dir)?.next().is_none() {
            std::fs::remove_dir_all(orig_dir)?;
        }
        Ok(())
    };
    match inner() {
        Ok(()) => true,
        Err(e) => {
            log::error!("Failed to change data location to: {}: {}", new_dir.display(), e);
            false
        }
    }
}

/// `shutil.copytree`
pub fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// `migrate_dir` (`shutil.move`)
pub fn migrate_dir(orig_dir: &Path, new_dir: &Path) -> bool {
    match std::fs::rename(orig_dir, new_dir) {
        Ok(()) => true,
        Err(_) => match copy_tree(orig_dir, new_dir).and_then(|_| std::fs::remove_dir_all(orig_dir)) {
            Ok(()) => true,
            Err(e) => {
                log::error!(
                    "Failed to change migrate dir from: {} to: {}: {}",
                    orig_dir.display(),
                    new_dir.display(),
                    e
                );
                false
            }
        },
    }
}

/// `open_explorer`
pub fn open_explorer(path: &Path) -> bool {
    let result = {
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer").arg(path).spawn().map(|_| ())
        }
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open").arg(path).spawn().map(|_| ())
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            std::process::Command::new("xdg-open").arg(path).spawn().map(|_| ())
        }
    };
    match result {
        Ok(()) => true,
        Err(e) => {
            log::error!("Failed to open explorer to location: {}: {}", path.display(), e);
            false
        }
    }
}

/// `get_default_user_data_dir`
pub fn get_default_user_data_dir() -> PathBuf {
    let mut result = consts::home_dir();
    let test = result.join("Documents");
    if test.exists() {
        result = test;
    }
    result.join(consts::APP_DATA_FOLDER_DEFAULT_NAME)
}

/// `backup_file_if_exists`
pub fn backup_file_if_exists(paths: &consts::Paths, orig_path: &Path) -> std::io::Result<()> {
    if orig_path.exists() && orig_path.is_file() {
        let new_backup_loc = get_safe_backup_path(paths, orig_path)?;
        std::fs::rename(orig_path, &new_backup_loc).or_else(|_| {
            std::fs::copy(orig_path, &new_backup_loc)?;
            std::fs::remove_file(orig_path)
        })?;
    }
    Ok(())
}

/// `get_safe_backup_path`
pub fn get_safe_backup_path(paths: &consts::Paths, orig_path: &Path) -> std::io::Result<PathBuf> {
    let orig_name = orig_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    if !paths.outdated_routes_dir.exists() {
        std::fs::create_dir_all(&paths.outdated_routes_dir)?;
    }
    let (base, ext) = split_ext(&orig_name);
    let mut counter = 1;
    loop {
        let result = paths.outdated_routes_dir.join(format!("{}_{}{}", base, counter, ext));
        if !result.exists() {
            return Ok(result);
        }
        counter += 1;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ReadJsonError {
    #[error("File not found: {0}")]
    NotFound(String),
    #[error("File appears to be a cloud storage placeholder that hasn't been downloaded: {0}\nIf using Dropbox/iCloud/OneDrive, please ensure the file is set to 'Available offline' or has finished syncing.")]
    CloudPlaceholder(String),
    #[error("File is empty: {0}")]
    Empty(String),
    #[error("{0}")]
    Io(String),
    #[error("{0}")]
    Json(String),
}

/// `read_json_file_safe`: retries for up to `max_wait` for cloud placeholder
/// files that report a size but read back empty.
pub fn read_json_file_safe(file_path: &Path, max_wait: Duration) -> Result<Value, ReadJsonError> {
    let display = file_path.display().to_string();
    if !file_path.exists() {
        return Err(ReadJsonError::NotFound(display));
    }
    let meta = std::fs::metadata(file_path).map_err(|e| ReadJsonError::Io(e.to_string()))?;
    let reported_size = meta.len();
    let read = || -> Result<String, ReadJsonError> {
        let bytes = std::fs::read(file_path).map_err(|e| ReadJsonError::Io(e.to_string()))?;
        Ok(pyjson::decode_text(&bytes))
    };
    let content = read()?;
    if !content.is_empty() {
        return pyjson::loads(&content).map_err(|e| ReadJsonError::Json(e.to_string()));
    }
    if reported_size > 0 {
        let interval = Duration::from_millis(250);
        let mut waited = Duration::ZERO;
        while waited < max_wait {
            std::thread::sleep(interval);
            waited += interval;
            let content = read()?;
            if !content.is_empty() {
                return pyjson::loads(&content).map_err(|e| ReadJsonError::Json(e.to_string()));
            }
        }
        return Err(ReadJsonError::CloudPlaceholder(display));
    }
    Err(ReadJsonError::Empty(display))
}

/// `is_likely_cloud_placeholder`
pub fn is_likely_cloud_placeholder(file_path: &Path) -> bool {
    if !file_path.exists() {
        return false;
    }
    let Ok(meta) = std::fs::metadata(file_path) else { return false };
    if meta.len() == 0 {
        return false;
    }
    match std::fs::read(file_path) {
        Ok(bytes) => bytes.is_empty(),
        Err(_) => false,
    }
}

/// Write bytes atomically (temp file + rename).
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let tmp = path.with_extension(format!(
        "{}.tmp{}",
        path.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default(),
        std::process::id()
    ));
    std::fs::write(&tmp, bytes)?;
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(_) => {
            // Windows refuses to rename over an existing file that another
            // process has open; fall back to a plain overwrite.
            let r = std::fs::write(path, bytes);
            let _ = std::fs::remove_file(&tmp);
            r
        }
    }
}

/// Write `text` the way Python's text-mode `open(path, 'w')` would on this
/// platform: newlines become the platform newline.
pub fn write_text_platform(path: &Path, text: &str) -> std::io::Result<()> {
    let text = if pyjson::PLATFORM_NEWLINE == "\n" {
        text.to_string()
    } else {
        text.replace('\n', pyjson::PLATFORM_NEWLINE)
    };
    std::fs::write(path, encode_locale_text(&text))
}

/// Python's default text encoding on this platform for written files:
/// cp1252 on Windows, UTF-8 elsewhere. Characters outside cp1252 are
/// written as UTF-8 (Python would raise instead; that failure mode is not
/// worth reproducing).
pub fn encode_locale_text(text: &str) -> Vec<u8> {
    #[cfg(windows)]
    {
        let (cow, _, had_errors) = encoding_rs::WINDOWS_1252.encode(text);
        if had_errors {
            return text.as_bytes().to_vec();
        }
        cow.into_owned()
    }
    #[cfg(not(windows))]
    {
        text.as_bytes().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize() {
        assert_eq!(sanitize_string("Farfetch'd"), "farfetchd");
        assert_eq!(sanitize_string("Mr. Mime"), "mrmime");
        assert_eq!(sanitize_string("Nidoran\u{2640}"), "nidoran");
    }

    #[test]
    fn path_safe() {
        assert_eq!(get_path_safe_string("My Route: v2!"), "my-route-v2");
        assert_eq!(get_path_safe_string("  a - b  "), "a-b");
    }
}
