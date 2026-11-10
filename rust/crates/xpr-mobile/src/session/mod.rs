//! The session the iOS app drives: one `MainController` + `BattleController`
//! plus the bits of desktop UI state that carry logic (the loaded battle,
//! map packs, the last comparison). `dispatch` maps a method name to one of
//! the handlers in the submodules; every handler returns plain JSON view
//! models so the Swift side stays a thin renderer.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};

use xpr_core::{Config, Paths};
use xpr_data::{GenData, Registry};
use xpr_engine::{EventDefinition, NodeId, ObjKind};

use crate::battle::BattleController;
use crate::controller::MainController;
use crate::route_index::RouteIndex;

mod battle_view;
mod compare;
mod drafts;
mod events;
mod library;
mod map;
mod options;
mod settings;
mod summaries;
mod tree;

pub(crate) use map::MapCache;

pub struct Session {
    pub cfg: Config,
    pub paths: Paths,
    pub registry: Arc<Registry>,
    pub ctrl: MainController,
    pub bc: BattleController,
    pub index: RouteIndex,
    /// the event whose battle summary is loaded (`None` = none / empty)
    battle_for: Option<NodeId>,
    /// toasts / errors raised since the Swift side last asked
    messages: Vec<Value>,
    /// folder expand state by folder name (the desktop keeps it per name path)
    expand: std::collections::HashMap<String, bool>,
    /// groups whose extra items (multi-fight trainers etc.) are shown
    group_expanded: std::collections::HashSet<NodeId>,
    map: MapCache,
    comparison: Option<xpr_engine::RouteComparison>,
}

pub(crate) fn s<'a>(p: &'a Value, k: &str) -> &'a str {
    p.get(k).and_then(|v| v.as_str()).unwrap_or("")
}
pub(crate) fn opt_s(p: &Value, k: &str) -> Option<String> {
    p.get(k).and_then(|v| v.as_str()).map(|s| s.to_string())
}
pub(crate) fn i(p: &Value, k: &str) -> Option<i64> {
    p.get(k).and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)).or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())))
}
pub(crate) fn b(p: &Value, k: &str) -> bool {
    p.get(k).and_then(|v| v.as_bool()).unwrap_or(false)
}
pub(crate) fn ids(p: &Value, k: &str) -> Vec<NodeId> {
    p.get(k).and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or_default()
}
pub(crate) fn strs(p: &Value, k: &str) -> Vec<String> {
    p.get(k).and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default()
}

impl Session {
    pub fn new(config_dir: PathBuf, data_dir: PathBuf) -> Result<Session, String> {
        std::fs::create_dir_all(&config_dir).map_err(|e| format!("config dir: {}", e))?;
        std::fs::create_dir_all(&data_dir).map_err(|e| format!("data dir: {}", e))?;
        xpr_core::logging::config_logging(&config_dir);
        let mut paths = Paths {
            source_root: PathBuf::from("."),
            global_config_file: config_dir.join("config.json"),
            global_config_dir: config_dir,
            pokemon_raw_data: PathBuf::from("raw_pkmn_data"),
            assets_path: PathBuf::from("assets"),
            saved_routes_dir: PathBuf::new(),
            outdated_routes_dir: PathBuf::new(),
            custom_gens_dir: PathBuf::new(),
        };
        let mut cfg = Config::load(&paths.global_config_file);
        // the iOS sandbox moves between installs: always keep data in the
        // app's Documents folder (visible in Files), never a stale path
        cfg.set_user_data_dir(&data_dir.to_string_lossy());
        cfg.set_images_dir(&data_dir.join("images").to_string_lossy());
        paths.config_user_data_dir(&data_dir);
        for d in [&paths.saved_routes_dir, &paths.outdated_routes_dir, &paths.custom_gens_dir] {
            let _ = std::fs::create_dir_all(d);
        }
        let registry = Arc::new(Registry::new(paths.pokemon_raw_data.clone(), paths.custom_gens_dir.clone()));
        if let Err(e) = registry.reload_all_custom_gens() {
            log::warn!("custom gens: {}", e);
        }
        let mut index = RouteIndex::load(&paths);
        index.refresh(&paths);
        let ctrl = MainController::new(registry.clone(), paths.clone());
        Ok(Session {
            cfg,
            paths,
            registry,
            ctrl,
            bc: BattleController::new(),
            index,
            battle_for: None,
            messages: Vec::new(),
            expand: Default::default(),
            group_expanded: Default::default(),
            map: MapCache::default(),
            comparison: None,
        })
    }

    pub fn gen(&self) -> Result<Arc<GenData>, String> {
        self.ctrl.gen().ok_or_else(|| "no route is open".to_string())
    }

    pub fn dispatch(&mut self, method: &str, p: &Value) -> Result<Value, String> {
        let r = self.route_call(method, p);
        self.pump();
        r
    }

    fn route_call(&mut self, method: &str, p: &Value) -> Result<Value, String> {
        let (group, _) = method.split_once('.').unwrap_or((method, ""));
        match group {
            "app" => self.app_call(method, p),
            "library" | "route" | "newroute" => self.library_call(method, p),
            "tree" | "edit" | "folder" => self.tree_call(method, p),
            "event" => self.event_call(method, p),
            "opts" => self.options_call(method, p),
            "battle" => self.battle_call(method, p),
            "summary" => self.summaries_call(method, p),
            "compare" => self.compare_call(method, p),
            "map" => self.map_call(method, p),
            "settings" | "customgen" => self.settings_call(method, p),
            _ => Err(format!("unknown method {}", method)),
        }
    }

    pub fn png(&mut self, method: &str, p: &Value) -> Result<Vec<u8>, String> {
        match method {
            "map.chunk" | "map.map_image" | "map.sprite" | "map.export" => self.map_png(method, p),
            _ => Err(format!("unknown image method {}", method)),
        }
    }

    // ---- shared state ---------------------------------------------------------

    fn app_call(&mut self, method: &str, _p: &Value) -> Result<Value, String> {
        match method {
            "app.state" => Ok(self.app_state()),
            "app.messages" => Ok(Value::Array(std::mem::take(&mut self.messages))),
            "app.undo" => {
                self.ctrl.undo();
                Ok(self.app_state())
            }
            _ => Err(format!("unknown method {}", method)),
        }
    }

    /// What every screen needs to know about the open route.
    pub fn app_state(&self) -> Value {
        let gen = self.ctrl.gen();
        let open = gen.is_some() && self.ctrl.router.init_route_state.is_some();
        let species = self.ctrl.get_init_state().map(|s| s.solo_pkmn.name.clone());
        let final_level = self.ctrl.get_final_state().map(|s| s.solo_pkmn.cur_level);
        json!({
            "open": open,
            "name": self.ctrl.get_current_route_name(),
            "version": self.ctrl.get_version(),
            "generation": gen.as_ref().map(|g| g.get_generation()),
            "species": species,
            "final_level": final_level,
            "unsaved": self.ctrl.has_unsaved_changes(),
            "can_undo": self.ctrl.can_undo(),
            "valid": !self.ctrl.has_errors(),
            "invalid_ids": self.ctrl.get_all_invalid_event_ids(),
            "event_count": self.ctrl.router.item_count(),
            "has_map": gen.as_ref().map(|g| map::game_of_gen(g).is_some()).unwrap_or(false),
            "supports_bag_reorder": gen.as_ref().map(|g| g.supports_bag_reorder()).unwrap_or(false),
            "has_branched_mandatory": gen.as_ref().map(|g| g.has_branched_mandatory_fights()).unwrap_or(false),
            "selected": self.ctrl.get_all_selected_ids(true),
            "app_version": xpr_core::consts::APP_VERSION,
        })
    }

    pub(crate) fn toast(&mut self, text: impl Into<String>) {
        self.messages.push(json!({"kind": "info", "text": text.into()}));
    }

    /// Drain the controllers' signals: persist battle-summary choices into
    /// the route, reload the battle after route changes and collect the
    /// controller's messages (the desktop does this once per frame).
    fn pump(&mut self) {
        for _ in 0..4 {
            let bs = self.bc.take_signals();
            if bs.nonload_change {
                self.persist_battle();
            }
            let sig = self.ctrl.take_signals();
            if sig.route_changed || sig.version_changed {
                if sig.version_changed {
                    self.map.on_version_change();
                }
                if let Some(id) = self.battle_for {
                    if !self.bc.take_handled_route_change(id) {
                        self.reload_battle();
                    }
                }
            }
            while let Some(m) = self.ctrl.get_next_message_info() {
                if let Some(name) = m.strip_prefix("Successfully saved route: ") {
                    self.index.refresh(&self.paths);
                    self.messages.push(json!({"kind": "saved", "text": format!("Route saved: {}", name)}));
                } else {
                    self.messages.push(json!({"kind": "info", "text": m}));
                }
            }
            while let Some(e) = self.ctrl.get_next_exception_info() {
                let e = e.replace("<class 'Exception'>: ", "");
                self.messages.push(json!({"kind": "error", "text": e}));
            }
            if !bs.nonload_change && !sig.any() {
                break;
            }
        }
    }

    pub(crate) fn def_of(&self, id: NodeId) -> Option<EventDefinition> {
        match self.ctrl.router.obj_kind(id)? {
            ObjKind::Group => self.ctrl.router.group(id).map(|g| g.event_definition.clone()),
            ObjKind::Item => self.ctrl.router.item(id).map(|i| i.event_definition.clone()),
            ObjKind::Folder => self.ctrl.router.folder(id).map(|f| f.event_definition.clone()),
        }
    }
}
