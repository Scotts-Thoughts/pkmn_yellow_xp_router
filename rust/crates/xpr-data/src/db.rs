//! Port of `pkmn/pkmn_db.py`: the lookup databases built on top of the loaded
//! JSON records. Insertion order is data (it drives every dropdown), so every
//! table is an `IndexMap`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use indexmap::IndexMap;

use xpr_core::consts;
use xpr_core::io_utils::sanitize_string;

use crate::loaders::DataReader;
use crate::model::{BaseItem, Gen, Move, PokemonSpecies, Trainer};

#[derive(Clone, Debug, Default)]
pub struct MinBattlesDB {
    path: PathBuf,
    pub data: Vec<String>,
}

impl MinBattlesDB {
    pub fn new(path: &Path, reader: &dyn DataReader) -> MinBattlesDB {
        let mut data = Vec::new();
        if !path.as_os_str().is_empty() {
            for fragment in reader.list_dir(path) {
                let (name, ext) = xpr_core::io_utils::split_ext(&fragment);
                if ext != ".json" {
                    continue;
                }
                data.push(name.to_string());
            }
        }
        MinBattlesDB {
            path: path.to_path_buf(),
            data,
        }
    }

    pub fn get_dir(&self) -> &Path {
        &self.path
    }
}

#[derive(Clone, Debug, Default)]
pub struct PkmnDB {
    data: IndexMap<String, Arc<PokemonSpecies>>,
}

impl PkmnDB {
    pub fn new(records: Vec<PokemonSpecies>) -> PkmnDB {
        let mut data = IndexMap::new();
        for rec in records {
            data.insert(sanitize_string(&rec.name), Arc::new(rec));
        }
        PkmnDB { data }
    }

    pub fn validate_moves(&self, move_db: &MoveDB) -> Result<(), String> {
        let mut invalid: Vec<(String, String)> = Vec::new();
        for mon in self.data.values() {
            for mv in mon.initial_moves.iter().chain(mon.tmhm_moves.iter()) {
                if move_db.get_move(mv).is_none() {
                    invalid.push((mon.name.clone(), mv.clone()));
                }
            }
            for (_, mv) in &mon.levelup_moves {
                if move_db.get_move(mv).is_none() {
                    invalid.push((mon.name.clone(), mv.clone()));
                }
            }
        }
        if !invalid.is_empty() {
            return Err(format!(
                "Invalid mons detected with unsupported moves: {}",
                py_tuple_list(&invalid)
            ));
        }
        Ok(())
    }

    pub fn validate_types(&self, supported_types: &[String]) -> Result<(), String> {
        let mut invalid: Vec<(String, String)> = Vec::new();
        for mon in self.data.values() {
            if !supported_types.contains(&mon.first_type) {
                invalid.push((mon.name.clone(), mon.first_type.clone()));
            }
            if !supported_types.contains(&mon.second_type) {
                invalid.push((mon.name.clone(), mon.second_type.clone()));
            }
        }
        if !invalid.is_empty() {
            return Err(format!(
                "Invalid mons detected with unsupported types: {}",
                py_tuple_list(&invalid)
            ));
        }
        Ok(())
    }

    pub fn get_all_names(&self, growth_rate: Option<&str>) -> Vec<String> {
        match growth_rate {
            None => self.data.values().map(|x| x.name.clone()).collect(),
            Some(g) => self
                .data
                .values()
                .filter(|x| x.growth_rate == g)
                .map(|x| x.name.clone())
                .collect(),
        }
    }

    pub fn get_pkmn(&self, name: &str) -> Option<&Arc<PokemonSpecies>> {
        self.data.get(&sanitize_string(name))
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<PokemonSpecies>> {
        self.data.values()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn get_filtered_names(&self, filter_val: Option<&str>, growth_rate: Option<&str>) -> Vec<String> {
        let Some(orig_filter) = filter_val else {
            return self.get_all_names(growth_rate);
        };
        let filter_val = sanitize_string(orig_filter);
        let result: Vec<String> = self
            .data
            .iter()
            .filter(|(k, v)| k.contains(&filter_val) && growth_rate.map(|g| v.growth_rate == g).unwrap_or(true))
            .map(|(_, v)| v.name.clone())
            .collect();
        if result.is_empty() {
            return vec![format!("No Match: '{}'", orig_filter)];
        }
        result
    }
}

#[derive(Clone, Debug, Default)]
pub struct TrainerDB {
    data: IndexMap<String, Arc<Trainer>>,
    id_lookup: IndexMap<i64, Arc<Trainer>>,
    pub loc_oriented_trainers: IndexMap<String, Vec<String>>,
    pub class_oriented_trainers: IndexMap<String, Vec<String>>,
}

impl TrainerDB {
    pub fn new(records: Vec<Trainer>) -> TrainerDB {
        let mut data: IndexMap<String, Arc<Trainer>> = IndexMap::new();
        let mut id_lookup = IndexMap::new();
        for rec in records {
            let arc = Arc::new(rec);
            id_lookup.insert(arc.trainer_id, arc.clone());
            data.insert(sanitize_string(&arc.name), arc);
        }
        let mut loc_oriented: IndexMap<String, Vec<String>> = IndexMap::new();
        let mut class_oriented: IndexMap<String, Vec<String>> = IndexMap::new();
        for t in data.values() {
            if !t.location.is_empty() {
                loc_oriented.entry(t.location.clone()).or_default().push(t.name.clone());
            }
            class_oriented.entry(t.trainer_class.clone()).or_default().push(t.name.clone());
        }
        TrainerDB {
            data,
            id_lookup,
            loc_oriented_trainers: loc_oriented,
            class_oriented_trainers: class_oriented,
        }
    }

    pub fn validate_trainers(&self, pkmn_db: &PkmnDB, move_db: &MoveDB) -> Result<(), String> {
        let mut invalid: Vec<String> = Vec::new();
        for t in self.data.values() {
            for mon in &t.pkmn {
                if pkmn_db.get_pkmn(&mon.name).is_none() {
                    invalid.push(format!("({}, {})", py_repr(&t.name), py_repr(&mon.name)));
                }
                for mv in mon.move_list.iter().flatten() {
                    if move_db.get_move(mv).is_none() {
                        invalid.push(format!(
                            "({}, {}, {})",
                            py_repr(&t.name),
                            py_repr(&mon.name),
                            py_repr(mv)
                        ));
                    }
                }
            }
        }
        if !invalid.is_empty() {
            return Err(format!(
                "Invalid trainers found with invalid mons/moves: [{}]",
                invalid.join(", ")
            ));
        }
        Ok(())
    }

    pub fn get_trainer(&self, trainer_name: &str) -> Option<&Arc<Trainer>> {
        self.data.get(&sanitize_string(trainer_name))
    }

    pub fn get_trainer_by_id(&self, trainer_id: i64) -> Option<&Arc<Trainer>> {
        self.id_lookup.get(&trainer_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<Trainer>> {
        self.data.values()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn get_all_locations(&self) -> Vec<String> {
        self.loc_oriented_trainers.keys().cloned().collect()
    }

    pub fn get_all_classes(&self) -> Vec<String> {
        self.class_oriented_trainers.keys().cloned().collect()
    }

    pub fn can_trainer_multi_battle(&self, trainer_name: &str) -> bool {
        match self.get_trainer(trainer_name) {
            None => false,
            Some(t) => t.pkmn.len() <= 3,
        }
    }

    /// `get_valid_trainers` with the default name function.
    pub fn get_valid_trainers(
        &self,
        trainer_class: Option<&str>,
        trainer_loc: Option<&str>,
        defeated_trainers: &[String],
        show_rematches: bool,
        multi_only: bool,
    ) -> Vec<String> {
        self.get_valid_trainers_with(trainer_class, trainer_loc, defeated_trainers, show_rematches, multi_only, |t| t.name.clone())
    }

    pub fn get_valid_trainers_with<F: Fn(&Trainer) -> String>(
        &self,
        trainer_class: Option<&str>,
        trainer_loc: Option<&str>,
        defeated_trainers: &[String],
        show_rematches: bool,
        multi_only: bool,
        name_fn: F,
    ) -> Vec<String> {
        let trainer_class = trainer_class.filter(|c| *c != consts::ALL_TRAINERS);
        let trainer_loc = trainer_loc.filter(|c| *c != consts::ALL_TRAINERS);
        let mut result = Vec::new();
        for t in self.data.values() {
            // Python: an if/elif chain of `continue`s == skip when any holds.
            if trainer_class.map(|c| t.trainer_class != c).unwrap_or(false) {
                continue;
            }
            if trainer_loc.map(|l| t.location != l).unwrap_or(false) {
                continue;
            }
            if defeated_trainers.iter().any(|d| *d == t.name) {
                continue;
            }
            if !show_rematches && t.rematch {
                continue;
            }
            if multi_only && t.pkmn.len() > 3 {
                continue;
            }
            result.push(name_fn(t));
        }
        result
    }
}

#[derive(Clone, Debug, Default)]
pub struct ItemDB {
    data: IndexMap<String, Arc<BaseItem>>,
    pub mart_items: IndexMap<String, Vec<String>>,
    pub key_items: Vec<String>,
    pub tms: Vec<String>,
    pub other_items: Vec<String>,
}

impl ItemDB {
    pub fn new(records: Vec<BaseItem>) -> ItemDB {
        let mut data: IndexMap<String, Arc<BaseItem>> = IndexMap::new();
        for rec in records {
            data.insert(sanitize_string(&rec.name), Arc::new(rec));
        }
        let mut mart_items: IndexMap<String, Vec<String>> = IndexMap::new();
        let mut key_items = Vec::new();
        let mut tms = Vec::new();
        let mut other_items = Vec::new();
        for item in data.values() {
            let mut other = true;
            if item.is_key_item {
                key_items.push(item.name.clone());
                other = false;
            }
            if item.name.starts_with("TM") || item.name.starts_with("HM") {
                tms.push(item.name.clone());
                other = false;
            }
            if other {
                other_items.push(item.name.clone());
            }
            for mart in &item.marts {
                mart_items.entry(mart.clone()).or_default().push(item.name.clone());
            }
        }
        ItemDB {
            data,
            mart_items,
            key_items,
            tms,
            other_items,
        }
    }

    pub fn validate_tms_hms(&self, move_db: &MoveDB) -> Result<(), String> {
        let mut invalid: Vec<(String, String)> = Vec::new();
        for item_name in &self.tms {
            let move_name = self
                .get_item(item_name)
                .and_then(|i| i.move_name.clone())
                .unwrap_or_default();
            if move_db.get_move(&move_name).is_none() {
                invalid.push((item_name.clone(), move_name));
            }
        }
        if !invalid.is_empty() {
            return Err(format!("Found TM/HM(s) with invalid moves: {}", py_tuple_list(&invalid)));
        }
        Ok(())
    }

    /// `get_item`: sanitized lookup, then the (redundant) case-insensitive scan.
    pub fn get_item(&self, item_name: &str) -> Option<&Arc<BaseItem>> {
        let key = sanitize_string(item_name);
        if let Some(i) = self.data.get(&key) {
            return Some(i);
        }
        let lower = key.to_lowercase();
        for (test_name, item) in &self.data {
            if lower == test_name.to_lowercase() {
                return Some(item);
            }
        }
        None
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<BaseItem>> {
        self.data.values()
    }

    pub fn get_filtered_names(&self, item_type: &str, source_mart: &str, name_filter: Option<&str>) -> Vec<String> {
        let base_list: Vec<String> = if item_type == consts::ITEM_TYPE_ALL_ITEMS {
            self.data.values().map(|x| x.name.clone()).collect()
        } else if item_type == consts::ITEM_TYPE_KEY_ITEMS {
            self.key_items.clone()
        } else if item_type == consts::ITEM_TYPE_TM {
            self.tms.clone()
        } else {
            self.other_items.clone()
        };
        let filter = name_filter.map(sanitize_string);
        let mut result = Vec::new();
        for cur in base_list {
            let mart_ok = source_mart == consts::ITEM_TYPE_ALL_ITEMS
                || self.mart_items.get(source_mart).map(|v| v.contains(&cur)).unwrap_or(false);
            let filter_ok = match &filter {
                None => true,
                Some(f) => sanitize_string(&cur).contains(f.as_str()),
            };
            if mart_ok && filter_ok {
                result.push(cur);
            }
        }
        result
    }
}

const OMNI_STATS: [&str; 5] = [consts::ATK, consts::DEF, consts::SPA, consts::SPD, consts::SPE];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StatStageInfo {
    pub has_stat_effect: bool,
    pub is_guaranteed: bool,
    pub targets_self: bool,
    pub is_damaging: bool,
    pub max_applications: i64,
    pub stage_change: i64,
    pub is_belly_drum: bool,
}

#[derive(Clone, Debug, Default)]
pub struct MoveDB {
    data: IndexMap<String, Arc<Move>>,
    /// sanitized move name -> stat mods; boosting moves first, then reductions
    pub stat_mod_moves: IndexMap<String, Vec<(String, i64)>>,
    pub field_moves: IndexMap<String, Arc<Move>>,
    pub old_hacky_field_moves: IndexMap<String, Arc<Move>>,
}

impl MoveDB {
    pub fn new(records: Vec<Move>) -> MoveDB {
        let mut data: IndexMap<String, Arc<Move>> = IndexMap::new();
        for rec in records {
            data.insert(sanitize_string(&rec.name), Arc::new(rec));
        }
        let mut stat_mod_moves: IndexMap<String, Vec<(String, i64)>> = IndexMap::new();
        let mut stat_reduction_moves: IndexMap<String, Vec<(String, i64)>> = IndexMap::new();
        for mv in data.values() {
            let mut cur_mods: Vec<(String, i64)> = Vec::new();
            let mut is_reduction = false;
            for effect in &mv.effects {
                if let (Some(stat), Some(modifier)) = (&effect.stat, effect.modifier) {
                    if modifier > 0 {
                        is_reduction = true;
                    }
                    if stat == "omni" {
                        for s in OMNI_STATS {
                            cur_mods.push((s.to_string(), modifier));
                        }
                    } else {
                        cur_mods.push((stat.clone(), modifier));
                    }
                }
            }
            if !cur_mods.is_empty() {
                // NOTE: variable naming is inverted in the Python source; the
                // observable behaviour is "positive modifiers first".
                if is_reduction {
                    stat_mod_moves.insert(sanitize_string(&mv.name), cur_mods);
                } else {
                    stat_reduction_moves.insert(sanitize_string(&mv.name), cur_mods);
                }
            }
        }
        for (k, v) in stat_reduction_moves {
            stat_mod_moves.insert(k, v);
        }

        let mut field_moves: IndexMap<String, Arc<Move>> = IndexMap::new();
        let mut old_hacky_field_moves: IndexMap<String, Arc<Move>> = IndexMap::new();
        if let Some(ls) = data.get(consts::LIGHTSCREEN_SANITIZED_MOVE_NAME) {
            field_moves.insert(sanitize_string(&ls.name), ls.clone());
            old_hacky_field_moves.insert(sanitize_string(&ls.name), ls.clone());
        }
        if let Some(r) = data.get(consts::REFLECT_SANITIZED_MOVE_NAME) {
            field_moves.insert(sanitize_string(&r.name), r.clone());
            old_hacky_field_moves.insert(sanitize_string(&r.name), r.clone());
        }
        for mv in data.values() {
            if mv.has_field_effect {
                field_moves.insert(sanitize_string(&mv.name), mv.clone());
            }
        }
        MoveDB {
            data,
            stat_mod_moves,
            field_moves,
            old_hacky_field_moves,
        }
    }

    pub fn validate_move_types(&self, supported_types: &[String]) -> Result<(), String> {
        let mut invalid: Vec<(String, String)> = Vec::new();
        for mv in self.data.values() {
            if !supported_types.contains(&mv.move_type) {
                invalid.push((mv.name.clone(), mv.move_type.clone()));
            }
        }
        if !invalid.is_empty() {
            return Err(format!("Detected moves with invalid types: {}", py_tuple_list(&invalid)));
        }
        Ok(())
    }

    pub fn get_move(&self, move_name: &str) -> Option<&Arc<Move>> {
        let key = sanitize_string(move_name);
        if let Some(m) = self.data.get(&key) {
            return Some(m);
        }
        let lower = key.to_lowercase();
        for (test_name, mv) in &self.data {
            if lower == test_name.to_lowercase() {
                return Some(mv);
            }
        }
        None
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<Move>> {
        self.data.values()
    }

    pub fn get_filtered_names(&self, filter: Option<&str>, include_delete_move: bool) -> Vec<String> {
        let mut result: Vec<String> = Vec::new();
        let filter_s = filter.map(sanitize_string);
        match &filter_s {
            None => result = self.data.values().map(|m| m.name.clone()).collect(),
            Some(f) => {
                for (test_name, mv) in &self.data {
                    if test_name.to_lowercase().contains(f.as_str()) {
                        result.push(mv.name.clone());
                    }
                }
            }
        }
        if result.is_empty() {
            if include_delete_move {
                result.push(consts::DELETE_MOVE.to_string());
            } else {
                result.push(consts::NO_MOVE.to_string());
            }
        } else if include_delete_move {
            // Python: `filter in sanitize_string(DELETE_MOVE)` with filter None
            // raises; with a string it's a substring test.
            match &filter_s {
                Some(f) => {
                    if sanitize_string(consts::DELETE_MOVE).contains(f.as_str()) {
                        result.push(consts::DELETE_MOVE.to_string());
                    }
                }
                None => {}
            }
        }
        result
    }

    pub fn get_stat_mod(&self, move_name: &str) -> Vec<(String, i64)> {
        self.stat_mod_moves
            .get(&sanitize_string(move_name))
            .cloned()
            .unwrap_or_default()
    }

    pub fn get_stat_mod_for_target(&self, move_name: &str, target_self: bool) -> Vec<(String, i64)> {
        let Some(mv) = self.get_move(move_name) else {
            return Vec::new();
        };
        let mut result = Vec::new();
        for effect in &mv.effects {
            if let (Some(stat), Some(modifier)) = (&effect.stat, effect.modifier) {
                let effect_target = effect.target.as_deref().unwrap_or("self");
                let targets_self = effect_target == "self" || effect_target == "target_self";
                if targets_self != target_self {
                    continue;
                }
                if stat == "omni" {
                    for s in OMNI_STATS {
                        result.push((s.to_string(), modifier));
                    }
                } else {
                    result.push((stat.clone(), modifier));
                }
            }
        }
        result
    }

    pub fn get_stat_stage_info(&self, move_name: &str) -> StatStageInfo {
        let Some(mv) = self.get_move(move_name) else {
            return StatStageInfo::default();
        };
        let is_belly_drum = mv.has_flavor(consts::FLAVOR_BELLY_DRUM);
        let stat_effects: Vec<&crate::model::MoveEffect> = mv.effects.iter().filter(|e| e.has_stat_mod()).collect();
        if stat_effects.is_empty() {
            return StatStageInfo::default();
        }
        let first = stat_effects[0];
        let effect_target = first.target.as_deref().unwrap_or(consts::EFFECT_TARGET_SELF);
        let targets_self = effect_target == consts::EFFECT_TARGET_SELF || effect_target == "target_self";
        let modifier = first.modifier.unwrap_or(0);
        let chance = first.chance.unwrap_or(100);
        let is_guaranteed = chance >= 100;
        let is_damaging = mv.base_power.map(|p| p > 0).unwrap_or(false);
        let max_applications = if is_belly_drum {
            1
        } else if modifier == 0 {
            0
        } else {
            6 / modifier.abs()
        };
        StatStageInfo {
            has_stat_effect: true,
            is_guaranteed,
            targets_self,
            is_damaging,
            max_applications,
            stage_change: modifier,
            is_belly_drum,
        }
    }

    pub fn get_stat_stage_dropdown_options(&self, move_name: &str, gen: Gen) -> Option<Vec<String>> {
        let info = self.get_stat_stage_info(move_name);
        if !info.has_stat_effect {
            return None;
        }
        if !info.is_guaranteed && !info.is_damaging {
            return None;
        }
        if info.is_belly_drum {
            if gen == Gen::Two {
                return Some(vec!["0".into(), "2".into(), "6".into()]);
            }
            return Some(vec!["0".into(), "6".into()]);
        }
        if info.max_applications <= 0 {
            return None;
        }
        Some((0..=info.max_applications).map(|i| i.to_string()).collect())
    }
}

/// `repr(str)` for error strings.
fn py_repr(s: &str) -> String {
    xpr_core::pyjson::python_repr_str(s)
}

/// `str([(a, b), ...])`
fn py_tuple_list(items: &[(String, String)]) -> String {
    let inner: Vec<String> = items
        .iter()
        .map(|(a, b)| format!("({}, {})", py_repr(a), py_repr(b)))
        .collect();
    format!("[{}]", inner.join(", "))
}
