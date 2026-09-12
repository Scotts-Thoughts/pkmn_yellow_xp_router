//! Metadata index of the saved routes (`<config dir>/xpr_route_index.json`):
//! the game version, solo species and modification time of every route, so
//! the landing page never parses 5,000 route files on the UI thread. (It
//! lives next to the config rather than in `saved_routes/`, where the app
//! would list it as a route.)

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use xpr_core::consts;
use xpr_core::io_utils;
use xpr_core::Paths;

pub const INDEX_FILE_NAME: &str = "xpr_route_index.json";

#[derive(Clone, Debug, PartialEq)]
pub struct IndexEntry {
    pub name: String,
    pub version: String,
    pub species: String,
    /// seconds since the epoch (fractional), like `os.path.getmtime`
    pub mtime: f64,
}

#[derive(Clone, Debug, Default)]
pub struct RouteIndex {
    pub entries: HashMap<String, IndexEntry>,
    pub loaded: bool,
}

fn mtime_of(path: &Path) -> f64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

impl RouteIndex {
    fn index_path(paths: &Paths) -> PathBuf {
        paths.global_config_dir.join(INDEX_FILE_NAME)
    }

    /// Load the persisted index (missing or corrupt -> empty).
    pub fn load(paths: &Paths) -> RouteIndex {
        let mut idx = RouteIndex::default();
        let p = RouteIndex::index_path(paths);
        if let Ok(text) = std::fs::read_to_string(&p) {
            if let Ok(Value::Object(obj)) = serde_json::from_str::<Value>(&text) {
                for (name, v) in obj {
                    let version = v.get("version").and_then(|x| x.as_str()).unwrap_or("Unknown").to_string();
                    let species = v.get("species").and_then(|x| x.as_str()).unwrap_or("Unknown").to_string();
                    let mtime = v.get("mtime").and_then(|x| x.as_f64()).unwrap_or(0.0);
                    idx.entries.insert(name.clone(), IndexEntry { name, version, species, mtime });
                }
            }
        }
        idx
    }

    fn save(&self, paths: &Paths) {
        if !paths.global_config_dir.is_dir() {
            return;
        }
        let mut obj = serde_json::Map::new();
        let mut names: Vec<&String> = self.entries.keys().collect();
        names.sort();
        for n in names {
            let e = &self.entries[n];
            obj.insert(n.clone(), json!({"version": e.version, "species": e.species, "mtime": e.mtime}));
        }
        let text = serde_json::to_string_pretty(&Value::Object(obj)).unwrap_or_default();
        if let Err(e) = io_utils::write_atomic(&RouteIndex::index_path(paths), text.as_bytes()) {
            log::warn!("Could not write the route index: {}", e);
        }
    }

    /// Bring the index up to date with the saved-routes folder: re-read only
    /// the routes whose mtime changed, drop the ones that vanished.
    pub fn refresh(&mut self, paths: &Paths) -> bool {
        let names = io_utils::get_existing_route_names(paths, "", false);
        let mut changed = false;
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for name in names {
            let path = io_utils::get_existing_route_path(paths, &name);
            let mtime = mtime_of(&path);
            seen.insert(name.clone());
            let needs_read = match self.entries.get(&name) {
                Some(e) => e.mtime != mtime,
                None => true,
            };
            if !needs_read {
                continue;
            }
            let (version, species) = read_route_meta(&path);
            self.entries.insert(name.clone(), IndexEntry { name, version, species, mtime });
            changed = true;
        }
        let before = self.entries.len();
        self.entries.retain(|k, _| seen.contains(k));
        if self.entries.len() != before {
            changed = true;
        }
        self.loaded = true;
        if changed {
            self.save(paths);
        }
        changed
    }

    /// `_find_most_recent_route`
    pub fn most_recent(&self, paths: &Paths) -> Option<PathBuf> {
        let mut best: Option<&IndexEntry> = None;
        for e in self.entries.values() {
            if best.map(|b| e.mtime > b.mtime).unwrap_or(true) {
                best = Some(e);
            }
        }
        best.map(|e| io_utils::get_existing_route_path(paths, &e.name))
    }
}

/// The `Version` and `name` keys of a route file ("Unknown" when unreadable).
fn read_route_meta(path: &Path) -> (String, String) {
    let mut version = "Unknown".to_string();
    let mut species = "Unknown".to_string();
    if let Ok(bytes) = std::fs::read(path) {
        if let Ok(v) = serde_json::from_slice::<Value>(&bytes) {
            if let Some(s) = v.get(consts::PKMN_VERSION_KEY).and_then(|x| x.as_str()) {
                version = s.to_string();
            }
            if let Some(s) = v.get(consts::NAME_KEY).and_then(|x| x.as_str()) {
                species = s.to_string();
            }
        }
    }
    (version, species)
}
