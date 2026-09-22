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

    /// Load the persisted index (missing or corrupt -> empty, not loaded).
    /// A persisted index is `loaded`: it is shown right away and `refresh`
    /// corrects it afterwards.
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
                idx.loaded = true;
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
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::with_capacity(names.len());
        let mut stale: Vec<(String, PathBuf, f64)> = Vec::new();
        for name in names {
            // the names were just listed from `saved_routes_dir`, so this is
            // the path `get_existing_route_path` resolves to, minus its stat
            let path = paths.saved_routes_dir.join(format!("{}.json", name));
            let mtime = mtime_of(&path);
            // a tolerance, not equality: the index stores mtimes as JSON
            // floats and a parse can be off by 1 ulp, which re-read (and
            // re-saved) dozens of untouched routes on every start
            let needs_read = match self.entries.get(&name) {
                Some(e) => (e.mtime - mtime).abs() > 1e-3,
                None => true,
            };
            seen.insert(name.clone());
            if needs_read {
                stale.push((name, path, mtime));
            }
        }
        for ((name, _, mtime), (version, species)) in stale.iter().zip(read_route_metas(&stale)) {
            self.entries.insert(name.clone(), IndexEntry { name: name.clone(), version, species, mtime: *mtime });
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

/// `read_route_meta` for every stale route, in order. A first run (or a
/// re-synced folder) has thousands of files to read, so those are spread
/// over a few threads; the usual one or two are read in place.
fn read_route_metas(stale: &[(String, PathBuf, f64)]) -> Vec<(String, String)> {
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(8);
    if stale.len() < 16 || threads < 2 {
        return stale.iter().map(|(_, p, _)| read_route_meta(p)).collect();
    }
    let chunk = stale.len().div_ceil(threads);
    std::thread::scope(|s| {
        let handles: Vec<_> = stale.chunks(chunk).map(|c| s.spawn(move || c.iter().map(|(_, p, _)| read_route_meta(p)).collect::<Vec<_>>())).collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    })
}

/// The top-level `Version` and `name` strings of a route file, read without
/// building the whole JSON tree (a route is up to a megabyte of events).
/// Matches indexing a parsed `Value`: the last duplicate of a key wins, and
/// a non-string value counts as missing.
struct RouteMeta {
    version: Option<String>,
    species: Option<String>,
}

impl<'de> serde::Deserialize<'de> for RouteMeta {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<RouteMeta, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = RouteMeta;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a route object")
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<RouteMeta, A::Error> {
                let mut meta = RouteMeta { version: None, species: None };
                while let Some(key) = map.next_key::<std::borrow::Cow<'de, str>>()? {
                    if key == consts::PKMN_VERSION_KEY {
                        meta.version = map.next_value::<Value>()?.as_str().map(str::to_string);
                    } else if key == consts::NAME_KEY {
                        meta.species = map.next_value::<Value>()?.as_str().map(str::to_string);
                    } else {
                        map.next_value::<serde::de::IgnoredAny>()?;
                    }
                }
                Ok(meta)
            }
        }
        d.deserialize_map(V)
    }
}

/// The `Version` and `name` keys of a route file ("Unknown" when unreadable).
fn read_route_meta(path: &Path) -> (String, String) {
    let meta = std::fs::read(path).ok().and_then(|bytes| serde_json::from_slice::<RouteMeta>(&bytes).ok());
    let unknown = || "Unknown".to_string();
    match meta {
        Some(m) => (m.version.unwrap_or_else(unknown), m.species.unwrap_or_else(unknown)),
        None => (unknown(), unknown()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta_of(text: &str) -> (String, String) {
        let dir = std::env::temp_dir().join(format!("xpr_route_meta_{}_{:?}", std::process::id(), std::thread::current().id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("r.json");
        std::fs::write(&p, text).unwrap();
        let r = read_route_meta(&p);
        let _ = std::fs::remove_dir_all(&dir);
        r
    }

    /// The skipping parser answers exactly like indexing the parsed `Value`.
    #[test]
    fn route_meta_matches_a_full_parse() {
        let s = |a: &str, b: &str| (a.to_string(), b.to_string());
        assert_eq!(meta_of(r#"{"name": "Abra", "events": [{"name": "inner", "Version": "x"}], "Version": "Yellow"}"#), s("Yellow", "Abra"));
        // only the top level counts, the last duplicate wins, non-strings are missing
        assert_eq!(meta_of(r#"{"events": {"Version": "Red"}, "name": "A", "name": "B", "Version": 3}"#), s("Unknown", "B"));
        assert_eq!(meta_of(r#"{"name": null}"#), s("Unknown", "Unknown"));
        assert_eq!(meta_of(r#"[1, 2]"#), s("Unknown", "Unknown"));
        assert_eq!(meta_of(r#"{"name": "Abra", "Version": "Yellow""#), s("Unknown", "Unknown"));
        assert_eq!(meta_of(r#"{"name": "Abra"} trailing"#), s("Unknown", "Unknown"));
    }
}
