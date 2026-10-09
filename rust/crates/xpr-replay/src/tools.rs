//! Where the helper programs, the mappers and the ROMs are.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// The helper programs and the mapper folder.
#[derive(Clone, Debug)]
pub struct Tools {
    /// `xpr-replay-worker.exe`
    pub worker: PathBuf,
    /// `pab-host.exe`
    pub pab_host: PathBuf,
    /// Poke-A-Byte's mapper folder (`%APPDATA%\PokeAByte\mappers`)
    pub mappers: PathBuf,
}

const WORKER_EXE: &str = if cfg!(windows) { "xpr-replay-worker.exe" } else { "xpr-replay-worker" };
const PAB_HOST_EXE: &str = if cfg!(windows) { "pab-host.exe" } else { "pab-host" };

fn tools_in(dir: &Path) -> Option<(PathBuf, PathBuf)> {
    let worker = dir.join(WORKER_EXE);
    let pab = dir.join("pab-host").join(PAB_HOST_EXE);
    (worker.is_file() && pab.is_file()).then_some((worker, pab))
}

impl Tools {
    /// `dir` (or `XPR_REPLAY_TOOLS`) when given; otherwise `replay_tools`
    /// beside the executable, or `tools/replay_to_route/out` in a folder above
    /// the executable or the working directory (a checkout).
    pub fn locate(dir: Option<&Path>) -> Result<Tools, String> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(d) = dir {
            candidates.push(d.to_path_buf());
        } else if let Some(d) = std::env::var_os("XPR_REPLAY_TOOLS") {
            candidates.push(PathBuf::from(d));
        } else {
            let mut starts = Vec::new();
            if let Ok(exe) = std::env::current_exe() {
                if let Some(p) = exe.parent() {
                    candidates.push(p.join("replay_tools"));
                    starts.push(p.to_path_buf());
                }
            }
            if let Ok(cwd) = std::env::current_dir() {
                starts.push(cwd);
            }
            for s in starts {
                for a in s.ancestors() {
                    candidates.push(a.join("tools").join("replay_to_route").join("out"));
                }
            }
        }
        let (worker, pab_host) = candidates
            .iter()
            .find_map(|d| tools_in(d))
            .ok_or_else(|| "The replay tools were not found: build them with tools/replay_to_route/build.sh (or set XPR_REPLAY_TOOLS).".to_string())?;
        let mappers = match std::env::var_os("XPR_MAPPERS_DIR") {
            Some(d) => PathBuf::from(d),
            None => default_mappers_dir().ok_or("Poke-A-Byte's mapper folder was not found (set XPR_MAPPERS_DIR)")?,
        };
        if !mappers.is_dir() {
            return Err(format!("The mapper folder {} does not exist", mappers.display()));
        }
        Ok(Tools { worker, pab_host, mappers })
    }
}

fn default_mappers_dir() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA").map(PathBuf::from).or_else(|| {
        // Poke-A-Byte uses .NET's ApplicationData folder on every platform
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config"))
    })?;
    let d = appdata.join("PokeAByte").join("mappers");
    d.is_dir().then_some(d)
}

/// Folders to look for a replay's ROM in: the ones given, the folders of the
/// ROMs Super Shuckie opened recently (from the `settings.json` of the user
/// folder the replay sits in), and the replay's own folder.
pub fn rom_search_dirs(replay: &Path, extra: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = extra.to_vec();
    // <user dir>/<rom>-data/replays/<name>.replay
    let user_dir = replay.parent().and_then(|p| p.parent()).and_then(|p| p.parent());
    if let Some(u) = user_dir {
        if let Ok(text) = std::fs::read_to_string(u.join("settings.json")) {
            if let Ok(v) = serde_json::from_str::<Value>(&text) {
                let recent = v.pointer("/recent_roms/recent_roms").and_then(|r| r.as_array()).cloned().unwrap_or_default();
                for r in recent.iter().filter_map(|r| r.as_str()) {
                    if let Some(parent) = Path::new(r).parent() {
                        dirs.push(parent.to_path_buf());
                    }
                }
            }
        }
    }
    if let Some(p) = replay.parent() {
        dirs.push(p.to_path_buf());
    }
    let mut seen = std::collections::HashSet::new();
    dirs.retain(|d| d.is_dir() && seen.insert(d.to_string_lossy().to_lowercase()));
    dirs
}
