//! Port of `routing/router.py`: the route tree, its operations, load/save
//! and notes export. Recalculation lives in `recalc.rs`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use indexmap::{IndexMap, IndexSet};
use serde_json::Value;

use xpr_core::consts;
use xpr_core::pyjson::{self, get, truthy};
use xpr_data::model::{Nature, StatBlock};
use xpr_data::{GenData, Registry};

use crate::events::{EventDefinition, LearnMoveEventDefinition, LevelUpKey, LevelVal};
use crate::state::{Inventory, RouteState, SoloPokemon, SoloPokemonArgs};
use crate::tree::{EventFolder, EventGroup, EventItem, Node, NodeId, ObjKind};

/// Where a new node goes (`insert_after` / `insert_before` / `dest_folder_name`).
#[derive(Clone, Debug, Default)]
pub struct InsertSpec {
    pub insert_before: Option<NodeId>,
    pub insert_after: Option<NodeId>,
    pub dest_folder_name: Option<String>,
}

pub struct Router {
    registry: Arc<Registry>,
    gen: Option<Arc<GenData>>,
    pub pkmn_version: Option<String>,
    pub init_route_state: Option<Arc<RouteState>>,
    pub(crate) nodes: HashMap<NodeId, Node>,
    pub(crate) items: HashMap<NodeId, EventItem>,
    pub root_id: NodeId,
    pub folder_lookup: IndexMap<String, NodeId>,
    pub level_up_move_defs: IndexMap<LevelUpKey, LearnMoveEventDefinition>,
    pub defeated_trainers: IndexSet<String>,
    pub test_moves: Vec<String>,
    pub(crate) next_id: NodeId,
    /// Set while the computed states are not a valid prefix for an
    /// incremental recalculation (before the first full pass, or after a
    /// failed one).
    pub(crate) needs_full_recalc: bool,
}

impl Router {
    pub fn new(registry: Arc<Registry>) -> Router {
        let mut r = Router {
            registry,
            gen: None,
            pkmn_version: None,
            init_route_state: None,
            nodes: HashMap::new(),
            items: HashMap::new(),
            root_id: 0,
            folder_lookup: IndexMap::new(),
            level_up_move_defs: IndexMap::new(),
            defeated_trainers: IndexSet::new(),
            test_moves: vec![String::new(); 4],
            next_id: 0,
            needs_full_recalc: true,
        };
        r.reset_events();
        r
    }

    pub fn registry(&self) -> &Arc<Registry> {
        &self.registry
    }

    /// The current game data (`current_gen_info()`), if a version is loaded.
    pub fn gen(&self) -> Option<&Arc<GenData>> {
        self.gen.as_ref()
    }

    pub fn gen_data(&self) -> Result<&Arc<GenData>, String> {
        self.gen.as_ref().ok_or_else(|| "No game version loaded".to_string())
    }

    pub(crate) fn mint_id(&mut self) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// `_reset_events`
    pub(crate) fn reset_events(&mut self) {
        self.nodes.clear();
        self.items.clear();
        let root_id = self.mint_id();
        self.nodes.insert(
            root_id,
            Node::Folder(EventFolder {
                id: root_id,
                parent: None,
                name: consts::ROOT_FOLDER_NAME.to_string(),
                enabled: Some(true),
                expanded: Some(true),
                event_definition: EventDefinition::notes_only(""),
                init_state: None,
                final_state: None,
                child_errors: false,
                children: Vec::new(),
            }),
        );
        self.root_id = root_id;
        self.folder_lookup = IndexMap::new();
        self.folder_lookup.insert(consts::ROOT_FOLDER_NAME.to_string(), root_id);
        self.defeated_trainers = IndexSet::new();
        self.test_moves = vec![String::new(); 4];
        self.needs_full_recalc = true;
    }

    /// `MainWindow.close_route`'s reset of the router: no solo mon, no
    /// version, an empty tree, no level-up moves and no defeated trainers.
    /// (The loaded game data is kept, as `current_gen_info()` is in Python.)
    pub fn close_route(&mut self) {
        self.init_route_state = None;
        self.pkmn_version = None;
        self.reset_events();
        self.level_up_move_defs = IndexMap::new();
        self.defeated_trainers = IndexSet::new();
    }

    /// `_change_version`
    pub fn change_version(&mut self, new_version: &str) -> Result<(), String> {
        self.pkmn_version = Some(new_version.to_string());
        let g = self.registry.get_version(new_version)?;
        self.gen = Some(g);
        Ok(())
    }

    // ---- lookups ------------------------------------------------------------

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(&id)
    }

    pub fn folder(&self, id: NodeId) -> Option<&EventFolder> {
        self.nodes.get(&id).and_then(|n| n.as_folder())
    }

    pub fn group(&self, id: NodeId) -> Option<&EventGroup> {
        self.nodes.get(&id).and_then(|n| n.as_group())
    }

    pub fn item(&self, id: NodeId) -> Option<&EventItem> {
        self.items.get(&id)
    }

    pub fn root(&self) -> &EventFolder {
        self.folder(self.root_id).expect("root folder")
    }

    /// `get_event_obj`: what kind of object an id names, if any.
    pub fn obj_kind(&self, id: NodeId) -> Option<ObjKind> {
        match self.nodes.get(&id) {
            Some(Node::Folder(_)) => Some(ObjKind::Folder),
            Some(Node::Group(_)) => Some(ObjKind::Group),
            None => self.items.get(&id).map(|_| ObjKind::Item),
        }
    }

    /// Number of event items currently held (every one belongs to a group).
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    pub fn contains_id(&self, id: NodeId) -> bool {
        self.nodes.contains_key(&id) || self.items.contains_key(&id)
    }

    /// Parent id of a node or item (`obj.parent`).
    pub fn parent_of(&self, id: NodeId) -> Option<NodeId> {
        if let Some(n) = self.nodes.get(&id) {
            return n.parent();
        }
        self.items.get(&id).map(|i| i.parent)
    }

    /// `is_enabled()` following the parent chain for any object.
    pub fn is_enabled(&self, id: NodeId) -> bool {
        if let Some(item) = self.items.get(&id) {
            return item.enabled.unwrap_or(false) && self.is_enabled(item.parent);
        }
        match self.nodes.get(&id) {
            Some(Node::Folder(f)) => {
                let parent_enabled = match f.parent {
                    None => true,
                    Some(p) => self.is_enabled(p),
                };
                f.enabled.unwrap_or(false) && parent_enabled
            }
            Some(Node::Group(g)) => g.enabled.unwrap_or(false) && self.is_enabled(g.parent),
            None => false,
        }
    }

    pub fn init_state_of(&self, id: NodeId) -> Option<&Arc<RouteState>> {
        if let Some(item) = self.items.get(&id) {
            return item.init_state.as_ref();
        }
        self.nodes.get(&id).and_then(|n| n.init_state())
    }

    pub fn final_state_of(&self, id: NodeId) -> Option<&Arc<RouteState>> {
        if let Some(item) = self.items.get(&id) {
            return item.final_state.as_ref();
        }
        self.nodes.get(&id).and_then(|n| n.final_state())
    }

    /// `get_final_state`
    pub fn get_final_state(&self) -> Option<&Arc<RouteState>> {
        let root = self.root();
        if !root.children.is_empty() {
            return root.final_state.as_ref();
        }
        self.init_route_state.as_ref()
    }

    /// Children of a folder in order.
    pub fn children_of(&self, folder_id: NodeId) -> Vec<NodeId> {
        self.folder(folder_id).map(|f| f.children.clone()).unwrap_or_default()
    }

    /// Depth-first list of all groups in route order.
    pub fn all_groups(&self) -> Vec<NodeId> {
        let mut out = Vec::new();
        self.collect_groups(self.root_id, &mut out);
        out
    }

    fn collect_groups(&self, folder_id: NodeId, out: &mut Vec<NodeId>) {
        for child in self.children_of(folder_id) {
            match self.nodes.get(&child) {
                Some(Node::Folder(_)) => self.collect_groups(child, out),
                Some(Node::Group(_)) => out.push(child),
                None => {}
            }
        }
    }

    // ---- solo pokemon ---------------------------------------------------------

    /// `set_solo_pkmn`
    pub fn set_solo_pkmn(
        &mut self,
        pkmn_name: &str,
        level_up_moves: Option<Vec<LearnMoveEventDefinition>>,
        custom_dvs: Option<StatBlock>,
        custom_ability_idx: Option<i64>,
        custom_nature: Option<Nature>,
    ) -> Result<(), String> {
        let gen = self.gen_data()?.clone();
        let pkmn_base = gen
            .pkmn_db()
            .get_pkmn(pkmn_name)
            .cloned()
            .ok_or_else(|| format!("Could not find base stats for Pokemon: {}", pkmn_name))?;
        let ability_idx = custom_ability_idx.unwrap_or(0);
        let nature = custom_nature.unwrap_or(Nature::HARDY);
        let dvs = match custom_dvs {
            Some(d) => d,
            None => {
                if gen.get_generation() <= 2 {
                    gen.make_stat_block(15, 15, 15, 15, 15, 15, false)
                } else {
                    gen.make_stat_block(31, 31, 31, 31, 31, 31, false)
                }
            }
        };
        let badges = gen.make_badge_list();
        let solo = SoloPokemon::new(
            pkmn_name,
            pkmn_base.clone(),
            dvs,
            badges.clone(),
            gen.make_stat_block(0, 0, 0, 0, 0, 0, true),
            ability_idx,
            nature,
            SoloPokemonArgs::default(),
        )?;
        self.init_route_state = Some(Arc::new(RouteState::new(
            solo,
            badges,
            Inventory::new(None, Vec::new(), gen.bag_limit()),
        )));

        match level_up_moves {
            None => {
                self.level_up_move_defs = IndexMap::new();
                self.add_level_up_moves_for_mon(&pkmn_base);
            }
            Some(moves) => {
                self.level_up_move_defs = IndexMap::new();
                for m in moves {
                    let key = m
                        .get_level_up_key()
                        .ok_or_else(|| format!("invalid literal for int() with base 10: {}", pyjson::python_repr_str(&m.level.display())))?;
                    self.level_up_move_defs.insert(key, m);
                }
            }
        }
        self.recalc()
    }

    /// Parse the `dv` block of a route file (`custom_dvs` dict) into a stat block.
    pub fn dvs_from_json(gen: &GenData, raw: Option<&Value>) -> Option<StatBlock> {
        let raw = raw?;
        let g = |k: &str| get(raw, k).and_then(pyjson::value_as_i64);
        if let (Some(hp), Some(atk), Some(def), Some(spa), Some(spd), Some(spe)) = (
            g(consts::HP),
            g(consts::ATTACK),
            g(consts::DEFENSE),
            g(consts::SPECIAL_ATTACK),
            g(consts::SPECIAL_DEFENSE),
            g(consts::SPEED),
        ) {
            return Some(gen.make_stat_block(hp, atk, def, spa, spd, spe, false));
        }
        if let (Some(hp), Some(atk), Some(def), Some(spc), Some(spd)) =
            (g(consts::HP), g(consts::ATK), g(consts::DEF), g(consts::SPC), g(consts::SPD))
        {
            return Some(gen.make_stat_block(hp, atk, def, spc, spc, spd, false));
        }
        None
    }

    /// `_add_level_up_moves_for_mon`
    pub(crate) fn add_level_up_moves_for_mon(&mut self, pkmn_base: &xpr_data::model::PokemonSpecies) {
        for (lvl, mv) in &pkmn_base.levelup_moves {
            let def = LearnMoveEventDefinition::new(
                Some(mv),
                None,
                consts::MOVE_SOURCE_LEVELUP,
                LevelVal::Int(*lvl),
                Some(&pkmn_base.name),
                false,
            );
            if let Some(key) = def.get_level_up_key() {
                if !self.level_up_move_defs.contains_key(&key) {
                    self.level_up_move_defs.insert(key, def);
                }
            }
        }
    }

    /// `change_current_innate_stats`
    pub fn change_current_innate_stats(&mut self, new_dvs: StatBlock, new_ability_idx: i64, new_nature: Nature) -> Result<(), String> {
        let gen = self.gen_data()?.clone();
        let init = self.init_route_state.clone().ok_or("no solo pokemon")?;
        let cur = &init.solo_pkmn;
        let solo = SoloPokemon::new(
            &cur.name,
            cur.species_def.clone(),
            new_dvs,
            init.badges.clone(),
            gen.make_stat_block(0, 0, 0, 0, 0, 0, true),
            new_ability_idx,
            new_nature,
            SoloPokemonArgs::default(),
        )?;
        self.init_route_state = Some(Arc::new(RouteState::new(solo, init.badges.clone(), init.inventory.clone())));
        self.recalc()
    }

    /// `get_effective_defeated_trainers`
    pub fn get_effective_defeated_trainers(&self) -> Vec<String> {
        match &self.gen {
            Some(g) if g.get_generation() >= 3 => Vec::new(),
            _ => self.defeated_trainers.iter().cloned().collect(),
        }
    }

    // ---- tree operations -------------------------------------------------------

    /// `add_area`
    pub fn add_area(&mut self, area_name: &str, insert_after: Option<NodeId>, dest_folder_name: &str, include_rematches: bool) -> Result<(), String> {
        let gen = self.gen_data()?.clone();
        let defeated = self.get_effective_defeated_trainers();
        let trainers = gen
            .trainer_db()
            .get_valid_trainers(None, Some(area_name), &defeated, include_rematches, false);
        if trainers.is_empty() {
            return Ok(());
        }
        let mut folder_name = area_name.to_string();
        let mut count = 1;
        while self.folder_lookup.contains_key(&folder_name) {
            count += 1;
            folder_name = format!("{} Trip:{}", area_name, count);
        }
        self.add_event_object(
            None,
            Some(&folder_name),
            InsertSpec {
                insert_after,
                dest_folder_name: Some(dest_folder_name.to_string()),
                ..Default::default()
            },
            false,
            Some(true),
            Some(true),
        )?;
        for t in trainers {
            self.add_event_object(
                Some(EventDefinition::with_trainer(crate::events::TrainerEventDefinition::new(&t))),
                None,
                InsertSpec {
                    dest_folder_name: Some(folder_name.clone()),
                    ..Default::default()
                },
                false,
                Some(true),
                Some(true),
            )?;
        }
        self.recalc()
    }

    /// `add_event_object`
    pub fn add_event_object(
        &mut self,
        event_def: Option<EventDefinition>,
        new_folder_name: Option<&str>,
        spec: InsertSpec,
        recalc: bool,
        folder_expanded: Option<bool>,
        folder_enabled: Option<bool>,
    ) -> Result<NodeId, String> {
        if self.init_route_state.is_none() {
            return Err("Cannot add an event when solo pokmn is not yet selected".to_string());
        }
        if event_def.is_none() && new_folder_name.is_none() {
            return Err("Must define either folder name or event definition".to_string());
        }
        let mut insert_after = spec.insert_after;
        let mut insert_before = spec.insert_before;
        let parent_id: NodeId = if let Some(after) = insert_after {
            let mut after_obj = after;
            if self.items.contains_key(&after_obj) {
                after_obj = self.items[&after_obj].parent;
                insert_after = Some(after_obj);
            }
            self.parent_of(after_obj)
                .ok_or_else(|| format!("Cannot find object to insert after: {}", after))?
        } else if let Some(before) = insert_before {
            let mut before_obj = before;
            if self.items.contains_key(&before_obj) {
                before_obj = self.items[&before_obj].parent;
                insert_before = Some(before_obj);
            }
            self.parent_of(before_obj)
                .ok_or_else(|| format!("Cannot find object to insert before: {}", before))?
        } else {
            let name = spec.dest_folder_name.as_deref().unwrap_or(consts::ROOT_FOLDER_NAME);
            *self
                .folder_lookup
                .get(name)
                .ok_or_else(|| format!("Cannot find folder with name: {}", name))?
        };

        let new_id = self.mint_id();
        if let Some(folder_name) = new_folder_name {
            let node = Node::Folder(EventFolder {
                id: new_id,
                parent: Some(parent_id),
                name: folder_name.to_string(),
                enabled: folder_enabled,
                expanded: folder_expanded,
                event_definition: event_def.unwrap_or_else(|| EventDefinition::notes_only("")),
                init_state: None,
                final_state: None,
                child_errors: false,
                children: Vec::new(),
            });
            self.nodes.insert(new_id, node);
            self.folder_lookup.insert(folder_name.to_string(), new_id);
        } else if let Some(def) = event_def {
            if let Some(td) = &def.trainer_def {
                let gen = self.gen_data()?.clone();
                let first = gen.trainer_db().get_trainer(&td.trainer_name);
                let first_refightable = first
                    .map(|t| t.refightable)
                    .ok_or_else(|| "'NoneType' object has no attribute 'refightable'".to_string())?;
                if !first_refightable {
                    self.defeated_trainers.insert(td.trainer_name.clone());
                    // NOTE: Python checks the FIRST trainer's flag here (Appendix B item 4)
                    if let Some(second) = td.second_trainer_name_str() {
                        if !first_refightable {
                            self.defeated_trainers.insert(second.to_string());
                        }
                    }
                }
            }
            let node = Node::Group(EventGroup {
                id: new_id,
                parent: parent_id,
                enabled: Some(true),
                name: String::new(),
                init_state: None,
                final_state: None,
                event_items: Vec::new(),
                event_definition: def,
                pkmn_after_levelups: Vec::new(),
                error_messages: Vec::new(),
                warning_messages: Vec::new(),
                level_up_learn_event_defs: Vec::new(),
            });
            self.nodes.insert(new_id, node);
        }

        self.insert_child_after(parent_id, new_id, insert_after, insert_before)?;
        if recalc {
            self.recalc_from_node(new_id)?;
        }
        Ok(new_id)
    }

    /// `EventFolder.insert_child_after`
    pub(crate) fn insert_child_after(&mut self, parent_id: NodeId, child_id: NodeId, after: Option<NodeId>, before: Option<NodeId>) -> Result<(), String> {
        let after = after.filter(|a| self.contains_id(*a));
        let before = before.filter(|b| self.contains_id(*b));
        let folder = self
            .nodes
            .get_mut(&parent_id)
            .and_then(|n| n.as_folder_mut())
            .ok_or_else(|| format!("Cannot find folder with id: {}", parent_id))?;
        if after.is_none() && before.is_none() {
            folder.children.push(child_id);
        } else if let Some(a) = after {
            let idx = folder
                .children
                .iter()
                .position(|c| *c == a)
                .ok_or_else(|| format!("Could not find object to insert after: {}", a))?;
            folder.children.insert(idx + 1, child_id);
        } else if let Some(b) = before {
            let idx = folder
                .children
                .iter()
                .position(|c| *c == b)
                .ok_or_else(|| format!("Could not find object to insert before: {}", b))?;
            folder.children.insert(idx, child_id);
        }
        if let Some(n) = self.nodes.get_mut(&child_id) {
            n.set_parent(Some(parent_id));
        }
        Ok(())
    }

    /// `EventFolder.remove_child`
    pub(crate) fn remove_child(&mut self, parent_id: NodeId, child_id: NodeId) -> Result<(), String> {
        let folder = self
            .nodes
            .get_mut(&parent_id)
            .and_then(|n| n.as_folder_mut())
            .ok_or_else(|| format!("Cannot find folder with id: {}", parent_id))?;
        let idx = folder
            .children
            .iter()
            .position(|c| *c == child_id)
            .ok_or_else(|| format!("EventFolder {} does not have child object: {}", folder.name, child_id))?;
        folder.children.remove(idx);
        Ok(())
    }

    /// `batch_remove_events`
    pub fn batch_remove_events(&mut self, ids: &[NodeId]) -> Result<(), String> {
        if let [single] = ids {
            return self.remove_event_object(*single, true);
        }
        for id in ids {
            self.remove_event_object(*id, false)?;
        }
        self.recalc()
    }

    /// `remove_event_object`
    pub fn remove_event_object(&mut self, event_id: NodeId, recalc: bool) -> Result<(), String> {
        let node = self
            .nodes
            .get(&event_id)
            .ok_or_else(|| format!("Cannot remove event for unknown id: {}", event_id))?;
        if let Node::Group(g) = node {
            if let Some(td) = &g.event_definition.trainer_def {
                self.defeated_trainers.shift_remove(&td.trainer_name);
                if let Some(second) = td.second_trainer_name_str() {
                    self.defeated_trainers.shift_remove(second);
                }
            }
        }
        let parent = node.parent().ok_or_else(|| "Cannot remove the root folder".to_string())?;
        let folder_info = node.as_folder().map(|f| (f.name.clone(), f.children.clone()));
        let group_items: Vec<NodeId> = node.as_group().map(|g| g.event_items.clone()).unwrap_or_default();
        if let Some((name, children)) = folder_info {
            // Python detaches the folder first and then removes its children
            // from the detached object; removing the children first is
            // equivalent because the intermediate recalcs are not observable.
            self.folder_lookup.shift_remove(&name);
            for child in &children {
                self.remove_event_object(*child, false)?;
            }
        }
        for item in group_items {
            self.items.remove(&item);
        }
        let position = self.position_of(event_id);
        self.remove_child(parent, event_id)?;
        self.nodes.remove(&event_id);
        if recalc {
            match position {
                Some((p, idx)) => self.recalc_from(p, idx)?,
                None => self.recalc()?,
            }
        }
        Ok(())
    }

    /// `move_event_object`
    pub fn move_event_object(&mut self, event_id: NodeId, move_up: bool) -> Result<(), String> {
        let parent = self
            .nodes
            .get(&event_id)
            .and_then(|n| n.parent())
            .ok_or_else(|| format!("Failed to find event object with id: {}", event_id))?;
        let folder = self.nodes.get_mut(&parent).and_then(|n| n.as_folder_mut()).unwrap();
        let idx = folder
            .children
            .iter()
            .position(|c| *c == event_id)
            .ok_or_else(|| format!("Failed to find event object with id: {}", event_id))?;
        let insert_idx = if move_up {
            idx.saturating_sub(1)
        } else {
            (idx + 1).min(folder.children.len() - 1)
        };
        folder.children.remove(idx);
        folder.children.insert(insert_idx, event_id);
        self.recalc()
    }

    /// `move_event_to_position`
    pub fn move_event_to_position(&mut self, event_id: NodeId, dest_folder_id: NodeId, after: Option<NodeId>, before: Option<NodeId>) -> Result<(), String> {
        if !self.nodes.contains_key(&event_id) {
            return Err(format!("Cannot find event with id: {}", event_id));
        }
        match self.nodes.get(&dest_folder_id) {
            None => return Err(format!("Cannot find destination folder with id: {}", dest_folder_id)),
            Some(Node::Group(_)) => return Err(format!("Destination {} is not a folder", dest_folder_id)),
            Some(Node::Folder(_)) => {}
        }
        if self.nodes[&event_id].is_folder() {
            let mut check = Some(dest_folder_id);
            while let Some(c) = check {
                if c == event_id {
                    return Err("Cannot move a folder into itself or a descendant".to_string());
                }
                check = self.parent_of(c);
            }
        }
        let after = after.filter(|a| self.contains_id(*a));
        let before = before.filter(|b| self.contains_id(*b));
        if after == Some(event_id) || before == Some(event_id) {
            return Ok(());
        }
        let parent = self.parent_of(event_id).unwrap();
        self.remove_child(parent, event_id)?;
        self.insert_child_after(dest_folder_id, event_id, after, before)?;
        self.recalc()
    }

    /// `toggle_event_highlight`
    pub fn toggle_event_highlight(&mut self, event_id: NodeId) -> Result<(), String> {
        match self.nodes.get_mut(&event_id) {
            Some(Node::Group(g)) => {
                g.event_definition.toggle_highlight();
                Ok(())
            }
            Some(_) => Ok(()),
            None => {
                if self.items.contains_key(&event_id) {
                    Ok(())
                } else {
                    Err(format!("Failed to find event object with id: {}", event_id))
                }
            }
        }
    }

    /// `set_event_highlight`
    pub fn set_event_highlight(&mut self, event_id: NodeId, highlight_num: Option<i64>) -> Result<(), String> {
        match self.nodes.get_mut(&event_id) {
            Some(Node::Group(g)) => {
                g.event_definition.set_highlight(highlight_num);
                Ok(())
            }
            Some(_) => Ok(()),
            None => {
                if self.items.contains_key(&event_id) {
                    Ok(())
                } else {
                    Err(format!("Failed to find event object with id: {}", event_id))
                }
            }
        }
    }

    /// `move_event_to_adjacent_folder`
    pub fn move_event_to_adjacent_folder(&mut self, event_id: NodeId, move_up: bool) -> Result<(), String> {
        if !self.nodes.contains_key(&event_id) {
            return Err(format!("Cannot find event object with id: {}", event_id));
        }
        let current_folder = self
            .parent_of(event_id)
            .ok_or_else(|| format!("Event {} has no parent folder", event_id))?;
        let grandparent = self
            .parent_of(current_folder)
            .ok_or_else(|| format!("Event {} is in root folder, cannot move to adjacent folder", event_id))?;
        let siblings = self.children_of(grandparent);
        let cur_idx = siblings
            .iter()
            .position(|c| *c == current_folder)
            .ok_or_else(|| "Current folder not found in grandparent's children".to_string())?;
        if move_up {
            let target = (0..cur_idx)
                .rev()
                .map(|i| siblings[i])
                .find(|s| self.nodes.get(s).map(|n| n.is_folder()).unwrap_or(false))
                .ok_or_else(|| format!("No previous folder found for event {}", event_id))?;
            self.remove_child(current_folder, event_id)?;
            self.insert_child_after(target, event_id, None, None)?;
        } else {
            let target = ((cur_idx + 1)..siblings.len())
                .map(|i| siblings[i])
                .find(|s| self.nodes.get(s).map(|n| n.is_folder()).unwrap_or(false))
                .ok_or_else(|| format!("No next folder found for event {}", event_id))?;
            self.remove_child(current_folder, event_id)?;
            let first_child = self.children_of(target).first().copied();
            self.insert_child_after(target, event_id, None, first_child)?;
        }
        self.recalc()
    }

    /// `get_invalid_folder_transfers`
    pub fn get_invalid_folder_transfers(&self, event_id: NodeId) -> Vec<String> {
        let mut result = Vec::new();
        self.child_folder_names_recursive(event_id, &mut result);
        result
    }

    fn child_folder_names_recursive(&self, id: NodeId, result: &mut Vec<String>) {
        if let Some(Node::Folder(f)) = self.nodes.get(&id) {
            result.push(f.name.clone());
            for c in f.children.clone() {
                self.child_folder_names_recursive(c, result);
            }
        }
    }

    /// `transfer_events`
    pub fn transfer_events(&mut self, event_ids: &[NodeId], dest_folder_name: &str) -> Result<(), String> {
        if !self.folder_lookup.contains_key(dest_folder_name) {
            self.add_event_object(None, Some(dest_folder_name), InsertSpec::default(), true, Some(true), Some(true))?;
        }
        for id in event_ids {
            if !self.nodes.contains_key(id) {
                return Err(format!("Cannot find group for id: {}", id));
            }
            if self.get_invalid_folder_transfers(*id).iter().any(|n| n == dest_folder_name) {
                return Err("Cannot transfer a folder into itself or a child folder".to_string());
            }
        }
        for id in event_ids {
            let dest = *self.folder_lookup.get(dest_folder_name).unwrap();
            let parent = self.parent_of(*id).unwrap();
            self.remove_child(parent, *id)?;
            self.insert_child_after(dest, *id, None, None)?;
        }
        self.recalc()
    }

    /// `replace_event_group`
    pub fn replace_event_group(&mut self, event_group_id: NodeId, mut new_event_def: EventDefinition) -> Result<(), String> {
        let kind = self
            .obj_kind(event_group_id)
            .ok_or_else(|| format!("Cannot find any event with id: {}", event_group_id))?;
        let old_def = match kind {
            ObjKind::Item => self.items[&event_group_id].event_definition.clone(),
            _ => self.nodes[&event_group_id].event_definition().clone(),
        };
        if !old_def.tags.is_empty() && new_event_def.tags.is_empty() {
            new_event_def.tags = old_def.tags.clone();
        }
        if new_event_def.recorded_time.is_null() {
            new_event_def.recorded_time = old_def.recorded_time.clone();
        }
        if new_event_def.split_time.is_null() {
            new_event_def.split_time = old_def.split_time.clone();
        }
        match kind {
            ObjKind::Folder => {
                if new_event_def.get_event_type() != consts::TASK_NOTES_ONLY {
                    return Err("Can only assign notes to EventFolders".to_string());
                }
                *self.nodes.get_mut(&event_group_id).unwrap().event_definition_mut() = new_event_def;
            }
            ObjKind::Item => {
                if old_def.get_event_type() != consts::TASK_LEARN_MOVE_LEVELUP {
                    return Err("Can only update event items for level up moves, currentlty".to_string());
                }
                let lm = new_event_def
                    .learn_move
                    .clone()
                    .ok_or_else(|| "'NoneType' object has no attribute 'get_level_up_key'".to_string())?;
                let key = lm.get_level_up_key().ok_or_else(|| "invalid level up key".to_string())?;
                if self.level_up_move_defs.contains_key(&key) {
                    return Err(format!("Invalid level up move: {}", py_key(&key)));
                }
                self.level_up_move_defs.insert(key, lm);
            }
            ObjKind::Group => {
                if let Some(td) = &old_def.trainer_def {
                    self.defeated_trainers.shift_remove(&td.trainer_name);
                }
                if let Some(td) = &new_event_def.trainer_def {
                    let gen = self.gen_data()?.clone();
                    let refightable = gen
                        .trainer_db()
                        .get_trainer(&td.trainer_name)
                        .map(|t| t.refightable)
                        .ok_or_else(|| "'NoneType' object has no attribute 'refightable'".to_string())?;
                    if !refightable {
                        self.defeated_trainers.insert(td.trainer_name.clone());
                    }
                }
                *self.nodes.get_mut(&event_group_id).unwrap().event_definition_mut() = new_event_def;
            }
        }
        match kind {
            ObjKind::Group => self.recalc_from_node(event_group_id),
            // a folder's own definition is notes only: its subtree is
            // enough (the folder's leaving state will not change)
            ObjKind::Folder => self.recalc_from(event_group_id, 0),
            ObjKind::Item => self.recalc(),
        }
    }

    /// `replace_levelup_move_event`
    pub fn replace_levelup_move_event(&mut self, new_def: LearnMoveEventDefinition) -> Result<(), String> {
        let key = new_def
            .get_level_up_key()
            .ok_or_else(|| "invalid level up key".to_string())?;
        self.level_up_move_defs.insert(key, new_def);
        self.recalc()
    }

    /// `is_valid_levelup_move`
    pub fn is_valid_levelup_move(&self, new_def: &LearnMoveEventDefinition) -> bool {
        match new_def.get_level_up_key() {
            Some(k) => self.level_up_move_defs.contains_key(&k),
            None => false,
        }
    }

    /// `rename_event_folder`
    pub fn rename_event_folder(&mut self, cur_name: &str, new_name: &str) -> Result<(), String> {
        let id = *self
            .folder_lookup
            .get(cur_name)
            .ok_or_else(|| pyjson::python_repr_str(cur_name))?;
        if let Some(Node::Folder(f)) = self.nodes.get_mut(&id) {
            f.name = new_name.to_string();
        }
        self.folder_lookup.shift_remove(cur_name);
        self.folder_lookup.insert(new_name.to_string(), id);
        Ok(())
    }

    // ---- serialization -----------------------------------------------------------

    /// `EventFolder.serialize`
    pub fn serialize_folder(&self, folder_id: NodeId) -> Value {
        let f = self.folder(folder_id).expect("folder");
        let events: Vec<Value> = f
            .children
            .iter()
            .map(|c| match self.nodes.get(c) {
                Some(Node::Folder(_)) => self.serialize_folder(*c),
                Some(Node::Group(g)) => g.serialize(),
                None => Value::Null,
            })
            .collect();
        pyjson::object(vec![
            (consts::EVENT_FOLDER_NAME, Value::String(f.name.clone())),
            (consts::TASK_NOTES_ONLY, Value::String(f.event_definition.notes.clone())),
            (consts::EVENTS, Value::Array(events)),
            (
                consts::EXPANDED_KEY,
                match f.expanded {
                    Some(b) => Value::Bool(b),
                    None => Value::Null,
                },
            ),
            (
                consts::ENABLED_KEY,
                match f.enabled {
                    Some(b) => Value::Bool(b),
                    None => Value::Null,
                },
            ),
        ])
    }

    /// `Router.serialize`
    pub fn serialize(&self) -> Result<Value, String> {
        let init = self.init_route_state.as_ref().ok_or("no solo pokemon")?;
        Ok(pyjson::object(vec![
            (consts::NAME_KEY, Value::String(init.solo_pkmn.name.clone())),
            (consts::DVS_KEY, init.solo_pkmn.dvs.serialize()),
            (consts::ABILITY_KEY, Value::from(init.solo_pkmn.ability_idx)),
            (consts::NATURE_KEY, Value::from(init.solo_pkmn.nature.value())),
            (
                consts::PKMN_VERSION_KEY,
                self.pkmn_version.clone().map(Value::String).unwrap_or(Value::Null),
            ),
            (
                consts::TASK_LEARN_MOVE_LEVELUP,
                Value::Array(self.level_up_move_defs.values().map(|x| x.serialize()).collect()),
            ),
            (consts::EVENTS, Value::Array(vec![self.serialize_folder(self.root_id)])),
        ]))
    }

    /// The bytes `save` writes (`serialize()` + `test_moves`, `indent=4`,
    /// platform newlines).
    pub fn save_bytes(&self) -> Result<Vec<u8>, String> {
        let mut out = self.serialize()?;
        if let Value::Object(o) = &mut out {
            o.insert(consts::TEST_MOVES_KEY.to_string(), pyjson::str_array(&self.test_moves));
        }
        Ok(pyjson::dump_indent4_platform_bytes(&out))
    }

    /// `save(name)`
    pub fn save(&self, paths: &xpr_core::Paths, name: &str) -> Result<PathBuf, String> {
        if !paths.saved_routes_dir.exists() {
            std::fs::create_dir(&paths.saved_routes_dir).map_err(|e| e.to_string())?;
        }
        let final_path = paths.saved_routes_dir.join(format!("{}.json", name));
        xpr_core::io_utils::backup_file_if_exists(paths, &final_path).map_err(|e| e.to_string())?;
        let bytes = self.save_bytes()?;
        std::fs::write(&final_path, bytes).map_err(|e| e.to_string())?;
        Ok(final_path)
    }

    /// `new_route`
    pub fn new_route(
        &mut self,
        solo_mon: &str,
        base_route_path: Option<&Path>,
        pkmn_version: &str,
        custom_dvs: Option<StatBlock>,
        custom_ability_idx: Option<i64>,
        custom_nature: Option<Nature>,
    ) -> Result<(), String> {
        self.change_version(pkmn_version)?;
        self.reset_events();
        self.set_solo_pkmn(solo_mon, None, custom_dvs, custom_ability_idx, custom_nature)?;
        if let Some(p) = base_route_path {
            self.load(p, true)?;
        }
        Ok(())
    }

    /// `load(route_path, load_events_only)`
    pub fn load(&mut self, route_path: &Path, load_events_only: bool) -> Result<(), String> {
        let result = if self.registry.is_embedded_file(route_path) {
            self.registry.read_data_json(route_path)
        } else {
            xpr_core::io_utils::read_json_file_safe(route_path, std::time::Duration::from_secs(2)).map_err(|e| e.to_string())
        }
        .map_err(|e| format!("Could not load route file: {}", e))?;
        self.load_value(&result, load_events_only)
    }

    /// `load` on an already parsed JSON document.
    pub fn load_value(&mut self, result: &Value, load_events_only: bool) -> Result<(), String> {
        self.reset_events();
        if !load_events_only {
            let version = get(result, consts::PKMN_VERSION_KEY)
                .and_then(|v| v.as_str())
                .unwrap_or(consts::YELLOW_VERSION)
                .to_string();
            self.change_version(&version)?;
            let gen = self.gen_data()?.clone();
            let name = get(result, consts::NAME_KEY)
                .and_then(|v| v.as_str())
                .ok_or_else(|| pyjson::python_repr_str(consts::NAME_KEY))?
                .to_string();
            let level_up_moves = match get(result, consts::TASK_LEARN_MOVE_LEVELUP) {
                Some(Value::Array(items)) => {
                    let mut moves = Vec::new();
                    for x in items {
                        if let Some(m) = LearnMoveEventDefinition::deserialize(Some(x), Some(&name)) {
                            moves.push(m);
                        }
                    }
                    Some(moves)
                }
                _ => None,
            };
            let ability_idx = match get(result, consts::ABILITY_KEY) {
                Some(Value::String(s)) => {
                    let species = gen
                        .pkmn_db()
                        .get_pkmn(&name)
                        .ok_or_else(|| "'NoneType' object has no attribute 'abilities'".to_string())?;
                    Some(
                        species
                            .abilities
                            .iter()
                            .position(|a| a == s)
                            .map(|p| p as i64)
                            .ok_or_else(|| format!("{} is not in list", pyjson::python_repr_str(s)))?,
                    )
                }
                Some(Value::Null) | None => None,
                Some(other) => pyjson::value_as_i64(other),
            };
            let nature = match get(result, consts::NATURE_KEY) {
                Some(Value::Null) | None => None,
                Some(other) => {
                    let idx = pyjson::value_as_i64(other).ok_or_else(|| format!("{} is not a valid Nature", other))?;
                    Some(Nature::from_index(idx).ok_or_else(|| format!("{} is not a valid Nature", idx))?)
                }
            };
            let dvs = Router::dvs_from_json(&gen, get(result, consts::DVS_KEY));
            self.set_solo_pkmn(&name, level_up_moves, dvs, ability_idx, nature)?;
        }

        let mut test_moves: Vec<String> = match get(result, consts::TEST_MOVES_KEY) {
            Some(Value::Array(items)) => items
                .iter()
                .map(|x| match x {
                    Value::String(s) => s.clone(),
                    Value::Null => String::new(),
                    other => pyjson::python_str_number(other),
                })
                .collect(),
            _ => vec![String::new(); 4],
        };
        while test_moves.len() < 4 {
            test_moves.push(String::new());
        }
        test_moves.truncate(4);
        self.test_moves = test_moves;

        let events = get(result, consts::EVENTS).ok_or_else(|| pyjson::python_repr_str(consts::EVENTS))?;
        if let Value::Array(items) = events {
            if let Some(first) = items.first() {
                let root = self.root_id;
                self.load_events_recursive(root, first)?;
            }
        }
        self.recalc()
    }

    /// `_load_events_recursive`
    pub(crate) fn load_events_recursive_pub(&mut self, parent_folder: NodeId, json_obj: &Value) -> Result<(), String> {
        self.load_events_recursive(parent_folder, json_obj)
    }

    fn load_events_recursive(&mut self, parent_folder: NodeId, json_obj: &Value) -> Result<(), String> {
        let parent_name = self.folder(parent_folder).map(|f| f.name.clone()).unwrap_or_default();
        let events = get(json_obj, consts::EVENTS).ok_or_else(|| pyjson::python_repr_str(consts::EVENTS))?;
        let Value::Array(items) = events else {
            return Ok(());
        };
        for event_json in items {
            if let Some(folder_name) = get(event_json, consts::EVENT_FOLDER_NAME) {
                let folder_name = match folder_name {
                    Value::String(s) => s.clone(),
                    other => pyjson::python_str_number(other),
                };
                let def = EventDefinition::deserialize(event_json)?;
                let expanded = match get(event_json, consts::EXPANDED_KEY) {
                    None => Some(true),
                    Some(Value::Null) => None,
                    Some(v) => Some(truthy(v)),
                };
                let enabled = match get(event_json, consts::ENABLED_KEY) {
                    None => Some(true),
                    Some(Value::Null) => None,
                    Some(v) => Some(truthy(v)),
                };
                self.add_event_object(
                    Some(def),
                    Some(&folder_name),
                    InsertSpec {
                        dest_folder_name: Some(parent_name.clone()),
                        ..Default::default()
                    },
                    false,
                    expanded,
                    enabled,
                )?;
                let inner = *self.folder_lookup.get(&folder_name).unwrap();
                self.load_events_recursive(inner, event_json)?;
            } else {
                let def = EventDefinition::deserialize(event_json)?;
                self.add_event_object(
                    Some(def),
                    None,
                    InsertSpec {
                        dest_folder_name: Some(parent_name.clone()),
                        ..Default::default()
                    },
                    false,
                    Some(true),
                    Some(true),
                )?;
            }
        }
        Ok(())
    }

    // ---- notes export ---------------------------------------------------------------

    /// `export_notes`: the text written to `<name>_notes.txt` (with `\n`).
    pub fn export_notes_text(&self) -> Result<String, String> {
        let gen = self.gen_data()?.clone();
        let mut output: Vec<String> = Vec::new();
        self.export_recursive(&gen, self.root_id, 0, &mut output)?;
        Ok(output.join("\n"))
    }

    pub fn export_notes(&self, paths: &xpr_core::Paths, name: &str) -> Result<PathBuf, String> {
        let dest = paths.saved_routes_dir.join(format!("{}_notes.txt", name));
        let text = self.export_notes_text()?;
        xpr_core::io_utils::write_text_platform(&dest, &text).map_err(|e| e.to_string())?;
        Ok(dest)
    }

    fn export_single_entry_def(&self, gen: &GenData, def: &EventDefinition, depth: usize, output: &mut Vec<String>) -> Result<(), String> {
        let indent = "\t".repeat(depth);
        if def.get_event_type() == consts::TASK_NOTES_ONLY {
            output.push(format!("{}Notes:", indent));
        } else {
            output.push(format!("{}{}", indent, def.get_label(gen)?));
            if def.get_event_type() == consts::TASK_TRAINER_BATTLE {
                let td = def.trainer_def.as_ref().unwrap();
                if !td.setup_moves.is_empty() {
                    let reprs: Vec<String> = td.setup_moves.iter().map(|s| pyjson::python_repr_str(s)).collect();
                    output.push(format!("{}Setup Moves: [{}]", indent, reprs.join(", ")));
                }
                if !td.mimic_selection.is_empty() {
                    output.push(format!("{}Mimic: {}", indent, td.mimic_selection));
                }
            }
        }
        if !def.notes.is_empty() {
            let notes_val = format!("{}{}", indent, def.notes.replace('\n', &format!("\n{}", indent)));
            output.push(notes_val);
        }
        output.push(indent);
        Ok(())
    }

    fn export_recursive(&self, gen: &GenData, folder_id: NodeId, depth: usize, output: &mut Vec<String>) -> Result<(), String> {
        for child in self.children_of(folder_id) {
            if !self.is_enabled(child) {
                continue;
            }
            match self.nodes.get(&child) {
                Some(Node::Folder(f)) => {
                    let indent = "\t".repeat(depth);
                    output.push(format!("{}Folder: {}", indent, f.name));
                    if !f.event_definition.notes.is_empty() {
                        output.push(format!("{}{}", indent, f.event_definition.notes.replace('\n', &format!("\n{}", indent))));
                    }
                    output.push(indent);
                    self.export_recursive(gen, child, depth + 1, output)?;
                }
                Some(Node::Group(g)) => {
                    self.export_single_entry_def(gen, &g.event_definition, depth, output)?;
                    for item_id in &g.event_items {
                        if let Some(item) = self.items.get(item_id) {
                            if !item.shares_group_definition && item.event_definition.learn_move.is_some() {
                                self.export_single_entry_def(gen, &item.event_definition, depth, output)?;
                            }
                        }
                    }
                }
                None => {}
            }
        }
        Ok(())
    }
}

/// `str(tuple)` for level-up keys in error text.
pub fn py_key(key: &LevelUpKey) -> String {
    format!(
        "({}, {}, {})",
        pyjson::python_repr_str(&key.0),
        key.1,
        pyjson::python_repr_str(&key.2)
    )
}
