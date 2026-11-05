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
    /// `dir` (or `XPR_REPLAY_TOOLS`) when given. Otherwise `replay_tools`
    /// beside the executable, then `tools/replay_to_route/out` in a folder
    /// above the executable or the working directory (a checkout), then
    /// `bundled`, which gets the released tools (`None` when there are none
    /// for this platform). It runs last because it may download them.
    ///
    /// The mappers are `mappers` in the tools' folder when it has one (the
    /// bundle's, which the recorders were tested against), else Poke-A-Byte's
    /// own. `XPR_MAPPERS_DIR` overrides both.
    pub fn locate(dir: Option<&Path>, bundled: &mut dyn FnMut() -> Result<Option<PathBuf>, String>) -> Result<Tools, String> {
        let explicit = dir.map(Path::to_path_buf).or_else(|| std::env::var_os("XPR_REPLAY_TOOLS").map(PathBuf::from));
        let found = match explicit {
            Some(d) => tools_in(&d),
            None => {
                let exe_dir = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf));
                let beside = exe_dir.as_ref().and_then(|d| tools_in(&d.join("replay_tools")));
                let cwd = std::env::current_dir().ok();
                let checkout = || exe_dir.iter().chain(cwd.iter()).find_map(|s| s.ancestors().find_map(|a| tools_in(&a.join("tools").join("replay_to_route").join("out"))));
                match beside.or_else(checkout) {
                    Some(t) => Some(t),
                    None => bundled()?.and_then(|d| tools_in(&d)),
                }
            }
        };
        let (worker, pab_host) = found.ok_or_else(tools_missing_message)?;
        let bundled_mappers = worker.parent().map(|d| d.join("mappers")).filter(|d| d.is_dir());
        let mappers = match std::env::var_os("XPR_MAPPERS_DIR") {
            Some(d) => PathBuf::from(d),
            None => bundled_mappers.or_else(default_mappers_dir).ok_or("Poke-A-Byte's mapper folder was not found (set XPR_MAPPERS_DIR)")?,
        };
        if !mappers.is_dir() {
            return Err(format!("The mapper folder {} does not exist", mappers.display()));
        }
        Ok(Tools { worker, pab_host, mappers })
    }
}

fn tools_missing_message() -> String {
    if cfg!(windows) {
        "The downloaded replay tools are incomplete.".to_string()
    } else {
        "Route from Replay is only available in the Windows version of the router for now.".to_string()
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
