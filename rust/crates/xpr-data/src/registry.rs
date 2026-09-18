//! Replacement for `gen_factory.py`: a registry of the 20 built-in versions
//! plus the custom gens found in the user's data directory. Versions are
//! parsed lazily, on first use, and cached.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use indexmap::IndexMap;
use serde_json::Value;

use xpr_core::consts;
use xpr_core::pyjson;

use crate::gen_data::GenData;
use crate::loaders::{load_gen, DataReader, DiskReader, GenSources};
use crate::model::Gen;

/// Built-in version -> (gen, dataset folder, trainer file override)
fn builtin_sources(raw_dir: &Path, version: &str) -> Option<(Gen, GenSources)> {
    let (gen, gen_dir, sub_dir, trainers, min_battles): (Gen, &str, &str, &str, bool) = match version {
        consts::YELLOW_VERSION => (Gen::One, "gen_one", "yellow", "trainers.json", true),
        consts::RED_VERSION | consts::BLUE_VERSION => (Gen::One, "gen_one", "red_blue", "trainers.json", true),
        consts::CRYSTAL_VERSION => (Gen::Two, "gen_two", "crystal", "trainers.json", true),
        consts::GOLD_VERSION | consts::SILVER_VERSION => (Gen::Two, "gen_two", "gold_silver", "trainers.json", true),
        consts::RUBY_VERSION => (Gen::Three, "gen_three", "ruby_sapphire", "ruby_trainers.json", false),
        consts::SAPPHIRE_VERSION => (Gen::Three, "gen_three", "ruby_sapphire", "sapphire_trainers.json", false),
        consts::EMERALD_VERSION => (Gen::Three, "gen_three", "emerald", "trainers.json", false),
        consts::FIRE_RED_VERSION | consts::LEAF_GREEN_VERSION => {
            (Gen::Three, "gen_three", "firered_leafgreen", "trainers.json", false)
        }
        consts::PLATINUM_VERSION => (Gen::Four, "gen_four", "platinum", "trainers.json", false),
        consts::DIAMOND_VERSION | consts::PEARL_VERSION => (Gen::Four, "gen_four", "diamond_pearl", "trainers.json", false),
        consts::HEART_GOLD_VERSION | consts::SOUL_SILVER_VERSION => {
            (Gen::Four, "gen_four", "heartgold_soulsilver", "trainers.json", false)
        }
        consts::BLACK_VERSION | consts::WHITE_VERSION => (Gen::Five, "gen_five", "black_white", "trainers.json", false),
        consts::BLACK_2_VERSION | consts::WHITE_2_VERSION => (Gen::Five, "gen_five", "black2_white2", "trainers.json", false),
        _ => return None,
    };
    let gen_path = raw_dir.join(gen_dir);
    let sub = gen_path.join(sub_dir);
    Some((
        gen,
        GenSources {
            pkmn_db: sub.join(consts::POKEMON_DB_FILE_NAME),
            trainer_db: sub.join(trainers),
            items: gen_path.join(consts::ITEM_DB_FILE_NAME),
            moves: gen_path.join(consts::MOVE_DB_FILE_NAME),
            type_info: gen_path.join(consts::TYPE_INFO_FILE_NAME),
            fights_info: gen_path.join(consts::FIGHTS_INFO_FILE_NAME),
            min_battles: if min_battles { sub.join("min_battles") } else { PathBuf::new() },
        },
    ))
}

pub fn gen_of_builtin(version: &str) -> Option<Gen> {
    builtin_sources(Path::new(""), version).map(|(g, _)| g)
}

/// Sources for a custom gen folder.
fn custom_sources(root: &Path) -> GenSources {
    GenSources {
        pkmn_db: root.join(consts::POKEMON_DB_FILE_NAME),
        trainer_db: root.join(consts::TRAINERS_DB_FILE_NAME),
        items: root.join(consts::ITEM_DB_FILE_NAME),
        moves: root.join(consts::MOVE_DB_FILE_NAME),
        type_info: root.join(consts::TYPE_INFO_FILE_NAME),
        fights_info: root.join(consts::FIGHTS_INFO_FILE_NAME),
        min_battles: PathBuf::new(),
    }
}

#[derive(Clone, Debug)]
pub struct CustomGenInfo {
    pub path: PathBuf,
    pub base_version: String,
    pub name: String,
}

/// Reads embedded copies of the built-in data (when compiled in) before
/// falling back to the disk.
pub struct EmbeddedOrDiskReader {
    raw_dir: PathBuf,
}

impl EmbeddedOrDiskReader {
    /// The embedded key of a path under `raw_dir`, if any.
    fn embedded_key(&self, path: &Path) -> Option<String> {
        let rel = path.strip_prefix(&self.raw_dir).ok()?;
        Some(rel.to_string_lossy().replace('\\', "/"))
    }
}

impl DataReader for EmbeddedOrDiskReader {
    fn read_bytes(&self, path: &Path) -> Result<Vec<u8>, String> {
        if let Some(bytes) = self.embedded_key(path).and_then(|k| crate::embedded::lookup(&k)) {
            return Ok(bytes);
        }
        DiskReader.read_bytes(path)
    }

    fn list_dir(&self, dir: &Path) -> Vec<String> {
        if let Some(key) = self.embedded_key(dir) {
            let names = crate::embedded::list_dir(&key);
            if !names.is_empty() {
                return names;
            }
        }
        DiskReader.list_dir(dir)
    }
}

pub struct Registry {
    raw_dir: PathBuf,
    custom_gens_dir: Mutex<PathBuf>,
    cache: Mutex<HashMap<String, Arc<GenData>>>,
    custom: Mutex<IndexMap<String, Arc<GenData>>>,
}

impl Registry {
    pub fn new(raw_dir: PathBuf, custom_gens_dir: PathBuf) -> Registry {
        Registry {
            raw_dir,
            custom_gens_dir: Mutex::new(custom_gens_dir),
            cache: Mutex::new(HashMap::new()),
            custom: Mutex::new(IndexMap::new()),
        }
    }

    pub fn set_custom_gens_dir(&self, dir: PathBuf) {
        *self.custom_gens_dir.lock().unwrap() = dir;
    }

    pub fn custom_gens_dir(&self) -> PathBuf {
        self.custom_gens_dir.lock().unwrap().clone()
    }

    pub fn raw_dir(&self) -> &Path {
        &self.raw_dir
    }

    fn reader(&self) -> EmbeddedOrDiskReader {
        EmbeddedOrDiskReader {
            raw_dir: self.raw_dir.clone(),
        }
    }

    /// Whether `path` is served from the embedded data pack.
    pub fn is_embedded_file(&self, path: &Path) -> bool {
        self.reader().embedded_key(path).is_some_and(|k| crate::embedded::contains(&k))
    }

    /// Parse a JSON file of the built-in data (embedded copy first, then
    /// disk); e.g. a min-battles preset route.
    pub fn read_data_json(&self, path: &Path) -> Result<Value, String> {
        let text = self.reader().read_text(path)?;
        pyjson::loads(&text).map_err(|e| e.to_string())
    }

    pub fn is_builtin(version: &str) -> bool {
        consts::VERSION_LIST.contains(&version)
    }

    /// `get_specific_version`: built-in (lazily loaded) or custom.
    pub fn get_version(&self, version: &str) -> Result<Arc<GenData>, String> {
        if let Some(g) = self.cache.lock().unwrap().get(version) {
            return Ok(g.clone());
        }
        if let Some((gen, sources)) = builtin_sources(&self.raw_dir, version) {
            let data = load_gen(gen, &sources, version, None, &self.reader())?;
            let arc = Arc::new(data);
            self.cache.lock().unwrap().insert(version.to_string(), arc.clone());
            return Ok(arc);
        }
        if let Some(g) = self.custom.lock().unwrap().get(version) {
            return Ok(g.clone());
        }
        Err(format!("Unknown version: {}", version))
    }

    pub fn is_loaded(&self, version: &str) -> bool {
        self.cache.lock().unwrap().contains_key(version) || self.custom.lock().unwrap().contains_key(version)
    }

    /// `get_gen_names`
    pub fn get_gen_names(&self, real_gens: bool, custom_gens: bool) -> Vec<String> {
        let mut result = Vec::new();
        if real_gens {
            result.extend(consts::VERSION_LIST.iter().map(|s| s.to_string()));
        }
        if custom_gens {
            result.extend(self.custom.lock().unwrap().keys().cloned());
        }
        result
    }

    /// `get_all_custom_gen_info`
    pub fn get_all_custom_gen_info(&self) -> Vec<CustomGenInfo> {
        let dir = self.custom_gens_dir();
        let mut result = Vec::new();
        if !dir.exists() {
            return result;
        }
        let Ok(rd) = std::fs::read_dir(&dir) else { return result };
        let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        entries.sort();
        for folder in entries {
            let meta_path = folder.join(consts::CUSTOM_GEN_META_FILE_NAME);
            let meta = std::fs::read(&meta_path)
                .ok()
                .and_then(|b| pyjson::loads(&pyjson::decode_text(&b)).ok());
            match meta {
                Some(m) => {
                    let name = pyjson::get_str(&m, consts::CUSTOM_GEN_NAME_KEY);
                    let base = pyjson::get_str(&m, consts::BASE_GEN_NAME_KEY);
                    match (name, base) {
                        (Some(n), Some(b)) => result.push(CustomGenInfo {
                            path: folder.clone(),
                            base_version: b.to_string(),
                            name: n.to_string(),
                        }),
                        _ => log::error!("Failed to load metadata for custom gen with path: {}", folder.display()),
                    }
                }
                None => log::error!("Failed to load metadata for custom gen with path: {}", folder.display()),
            }
        }
        result
    }

    /// `reload_all_custom_gens`: (re)loads every custom gen; the aggregated
    /// error text matches the Python message.
    pub fn reload_all_custom_gens(&self) -> Result<(), String> {
        let mut invalid: Vec<(String, String)> = Vec::new();
        let mut loaded: IndexMap<String, Arc<GenData>> = IndexMap::new();
        for info in self.get_all_custom_gen_info() {
            let result = (|| -> Result<Arc<GenData>, String> {
                let base_gen = gen_of_builtin(&info.base_version)
                    .ok_or_else(|| format!("Invalid base gen specified: {}", info.base_version))?;
                let sources = custom_sources(&info.path);
                let data = load_gen(base_gen, &sources, &info.name, Some(&info.base_version), &DiskReader)?;
                Ok(Arc::new(data))
            })();
            match result {
                Ok(data) => {
                    loaded.insert(info.name.clone(), data);
                }
                Err(e) => {
                    log::error!("Failed to load custom gen with path: {}: {}", info.path.display(), e);
                    invalid.push((info.name.clone(), e));
                }
            }
        }
        *self.custom.lock().unwrap() = loaded;
        if !invalid.is_empty() {
            let msg: Vec<String> = invalid
                .iter()
                .map(|(n, e)| format!("Custom gen: {}, error: {}", n, e))
                .collect();
            return Err(format!("Error(s) loading custom gens:\n\n{}", msg.join("\n")));
        }
        Ok(())
    }

    /// `create_custom_version`
    pub fn create_custom_version(&self, base_version: &str, custom_version: &str) -> Result<PathBuf, String> {
        let base = self.get_version(base_version)?;
        base.create_new_custom_gen(&self.custom_gens_dir(), custom_version, &self.reader())
    }

    /// Preload every built-in version (used by the golden tooling / tests).
    pub fn load_all_builtin(&self) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        for v in consts::VERSION_LIST {
            if let Err(e) = self.get_version(v) {
                errors.push((v.to_string(), e));
            }
        }
        errors
    }
}
