//! Pieces shared by the per-generation machines.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use indexmap::IndexMap;
use serde_json::Value;

use xpr_core::consts;
use xpr_data::GenData;
use xpr_engine::EventDefinition;

use crate::controller::{ActiveFlag, EventQueue, RecorderController};
use crate::gamehook::{GameHookProperty, PropertyStore};

/// Python `str(value)` for the JSON values GameHook sends.
pub fn py_str(v: &Value) -> String {
    match v {
        Value::Null => "None".to_string(),
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

/// Python `str.capitalize()`.
pub fn py_capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let mut out: String = first.to_uppercase().collect();
            out.extend(chars.flat_map(|c| c.to_lowercase()));
            out
        }
    }
}

/// `_name_prettify`: `" ".join([x.capitalize() for x in name.lower().split(" ")])`
pub fn name_prettify(name: &str) -> String {
    name.to_lowercase().split(' ').map(py_capitalize).collect::<Vec<_>>().join(" ")
}

/// The team-slot identity used to follow the solo mon (`_MonKey`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MonKey {
    pub species: Option<String>,
    pub attack: Value,
    pub defense: Value,
    pub speed: Value,
    pub special_attack: Value,
    /// gens 3+ carry it but it is not part of the key
    pub special_defense: Value,
    pub level: Value,
}

impl MonKey {
    pub fn dvs(&self) -> (&Value, &Value, &Value, &Value) {
        (&self.attack, &self.defense, &self.speed, &self.special_attack)
    }

    pub fn key(&self) -> (&Option<String>, (&Value, &Value, &Value, &Value)) {
        (&self.species, self.dvs())
    }

    pub fn same_key(&self, other: &MonKey) -> bool {
        self.key() == other.key()
    }

    pub fn same_dvs(&self, other: &MonKey) -> bool {
        self.dvs() == other.dvs()
    }

    pub fn level_i64(&self) -> i64 {
        crate::gamehook::value_as_i64(&self.level).unwrap_or(0)
    }

    /// `__repr__`
    pub fn repr(&self) -> String {
        format!(
            "_MonKey: ({}, ({}, {}, {}, {}))",
            match &self.species {
                Some(s) => format!("'{}'", s),
                None => "None".to_string(),
            },
            py_str(&self.attack),
            py_str(&self.defense),
            py_str(&self.speed),
            py_str(&self.special_attack)
        )
    }
}

/// A hash key for the gained/lost dictionaries (`__hash__` uses `get_key`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MonKeyId(pub Option<String>, pub String, pub String, pub String, pub String);

impl MonKeyId {
    pub fn of(m: &MonKey) -> MonKeyId {
        MonKeyId(m.species.clone(), m.attack.to_string(), m.defense.to_string(), m.speed.to_string(), m.special_attack.to_string())
    }
}

/// `update_team_cache`'s gained/lost bookkeeping (identical in every gen).
pub struct TeamDiff {
    pub gained: IndexMap<MonKeyId, (MonKey, i64)>,
    pub lost: IndexMap<MonKeyId, (MonKey, i64)>,
}

pub fn team_diff(old: &[MonKey], new: &[MonKey]) -> TeamDiff {
    let mut gained: IndexMap<MonKeyId, (MonKey, i64)> = IndexMap::new();
    let mut lost: IndexMap<MonKeyId, (MonKey, i64)> = IndexMap::new();
    for (mon_idx, cur) in new.iter().enumerate() {
        if mon_idx >= old.len() {
            gained.entry(MonKeyId::of(cur)).or_insert((cur.clone(), 0)).1 += 1;
        } else if !cur.same_key(&old[mon_idx]) {
            gained.entry(MonKeyId::of(cur)).or_insert((cur.clone(), 0)).1 += 1;
            lost.entry(MonKeyId::of(&old[mon_idx])).or_insert((old[mon_idx].clone(), 0)).1 += 1;
        }
    }
    // account for re-orderings
    let keys: Vec<MonKeyId> = gained.keys().cloned().collect();
    for k in keys {
        if let Some(l) = lost.get_mut(&k) {
            let g = gained.get_mut(&k).unwrap();
            let match_count = g.1.min(l.1);
            g.1 -= match_count;
            l.1 -= match_count;
        }
    }
    gained.retain(|_, v| v.1 > 0);
    lost.retain(|_, v| v.1 > 0);
    TeamDiff { gained, lost }
}

pub fn repr_mons(map: &IndexMap<MonKeyId, (MonKey, i64)>) -> String {
    let parts: Vec<String> = map.values().map(|(m, n)| format!("{}: {}", m.repr(), n)).collect();
    format!("{{{}}}", parts.join(", "))
}

pub fn repr_team(team: &[MonKey]) -> String {
    format!("[{}]", team.iter().map(|m| m.repr()).collect::<Vec<_>>().join(", "))
}

/// Item counts keyed by the GameHook item name, in bag order (a Python dict).
pub type ItemCache = IndexMap<Value, i64>;

/// The gained/lost split of `_item_cache_update` (identical in every gen).
pub fn item_diff(old: &ItemCache, new: &ItemCache) -> (IndexMap<Value, i64>, IndexMap<Value, i64>) {
    let mut gained: IndexMap<Value, i64> = IndexMap::new();
    let mut lost: IndexMap<Value, i64> = IndexMap::new();
    let mut compared: HashSet<String> = HashSet::new();
    for (cur_item, cur_count) in old {
        let new_count = new.get(cur_item).copied().unwrap_or(0);
        if new_count > *cur_count {
            gained.insert(cur_item.clone(), new_count - cur_count);
        } else if *cur_count > new_count {
            lost.insert(cur_item.clone(), cur_count - new_count);
        }
        compared.insert(cur_item.to_string());
    }
    for (new_item, new_count) in new {
        if compared.contains(&new_item.to_string()) {
            continue;
        }
        if *new_count > 0 {
            gained.insert(new_item.clone(), *new_count);
        } else if 0 > *new_count {
            lost.insert(new_item.clone(), -*new_count);
        }
    }
    (gained, lost)
}

pub fn repr_items(map: &IndexMap<Value, i64>) -> String {
    let parts: Vec<String> = map
        .iter()
        .map(|(k, v)| {
            let key = match k {
                Value::String(s) => format!("'{}'", s),
                other => py_str(other),
            };
            format!("{}: {}", key, v)
        })
        .collect();
    format!("{{{}}}", parts.join(", "))
}

/// Emerald-and-later `DelayedUpdate`: the caller runs the update when
/// `tick`/`trigger` report `true`.
#[derive(Clone, Debug)]
pub struct DelayedUpdate {
    pub base_delay: i64,
    pub cur_delay: i64,
    pub is_active: bool,
}

impl DelayedUpdate {
    pub fn new(delay: i64) -> DelayedUpdate {
        DelayedUpdate { base_delay: delay, cur_delay: 0, is_active: false }
    }

    pub fn reset(&mut self) {
        self.cur_delay = 0;
        self.is_active = false;
    }

    /// One game second passed; `true` when the update should fire now.
    pub fn tick(&mut self) -> bool {
        if self.is_active {
            if self.cur_delay > 0 {
                self.cur_delay -= 1;
            } else {
                return self.trigger(false);
            }
        }
        false
    }

    pub fn begin_waiting(&mut self, force_reset: bool) {
        if self.is_active && !force_reset {
            return;
        }
        self.cur_delay = self.base_delay;
        self.is_active = true;
    }

    /// `true` when the update should run.
    pub fn trigger(&mut self, force: bool) -> bool {
        if self.is_active || force {
            self.cur_delay = 0;
            self.is_active = false;
            return true;
        }
        false
    }
}

/// `str(event_def)` (its label).
pub fn event_str(gen: &GenData, def: &EventDefinition) -> String {
    def.get_label(gen).unwrap_or_else(|e| format!("<{}>", e))
}

/// Everything the processing thread needs.
pub struct ProcessCtx {
    pub controller: Arc<RecorderController>,
    pub gen: Arc<GenData>,
    pub queue: Arc<EventQueue>,
    pub active: ActiveFlag,
}

impl ProcessCtx {
    /// `_process_events`: drain the queue while active (and then until empty).
    pub fn run(&self, mut process_one: impl FnMut(&ProcessCtx, EventDefinition)) {
        loop {
            let active = self.active.load(std::sync::atomic::Ordering::SeqCst);
            if !active && self.queue.is_empty() {
                break;
            }
            match self.queue.pop(Duration::from_millis(100)) {
                Some(event) => {
                    let label = event_str(&self.gen, &event);
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| process_one(self, event)));
                    if let Err(panic) = result {
                        let msg = if let Some(s) = panic.downcast_ref::<&str>() {
                            s.to_string()
                        } else if let Some(s) = panic.downcast_ref::<String>() {
                            s.clone()
                        } else {
                            "unknown".to_string()
                        };
                        log::error!("Exception occurred trying to process event: {}: {}", label, msg);
                        self.controller.add_event(EventDefinition::notes_only(&format!(
                            "{} Unexpected error: {}",
                            consts::RECORDING_ERROR_FRAGMENT,
                            msg
                        )));
                    }
                }
                None => {}
            }
        }
    }

    pub fn add_error(&self, msg: String) {
        log::error!("{}", msg);
        self.controller.add_event(EventDefinition::notes_only(&format!("{}{}", consts::RECORDING_ERROR_FRAGMENT, msg)));
    }
}

/// Prefix-resolve a TM/HM item name (`TM01`) to the DB's full name (`TM01 Mega Punch`).
pub fn resolve_tm_name(gen: &GenData, prefix: &str) -> Option<String> {
    gen.item_db()
        .get_filtered_names(consts::ITEM_TYPE_TM, consts::ITEM_TYPE_ALL_ITEMS, None)
        .into_iter()
        .find(|n| n.starts_with(prefix))
}

pub fn prop_i64(p: &GameHookProperty) -> Option<i64> {
    p.as_i64()
}

pub fn store_i64(store: &PropertyStore, key: &str) -> i64 {
    store.i64_of(key).unwrap_or(0)
}
