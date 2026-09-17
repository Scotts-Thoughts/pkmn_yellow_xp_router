//! Recalculation: `Router._recalc` / `_recursive_recalc` /
//! `_calc_single_event`, `EventGroup.apply`, `EventItem.apply`.

use std::sync::Arc;

use xpr_core::consts;
use xpr_data::model::EnemyPkmn;

use crate::events::{EventDefinition, LearnMoveEventDefinition};
use crate::router::Router;
use crate::state::RouteState;
use crate::tree::{EventItem, Node, NodeId};

impl Router {
    /// `_recalc`: recompute every event from the initial state.
    pub fn recalc(&mut self) -> Result<(), String> {
        self.items.clear();
        let init = self.init_route_state.clone().ok_or("no solo pokemon")?;
        let root = self.root_id;
        // until this succeeds the computed states cannot be trusted as a prefix
        self.needs_full_recalc = true;
        self.recursive_recalc(root, init)?;
        self.needs_full_recalc = false;
        Ok(())
    }

    /// Recompute the route from `parent.children[start]` to the end, reusing
    /// the states already computed for everything before that position.
    ///
    /// This gives the same result as [`Router::recalc`]: an event's outcome
    /// depends only on the state it starts from, its own definition and the
    /// level-up move table, and each level-up move is matched by exactly one
    /// event of the route (levels only go up), so the prefix would come out
    /// identical. When the prefix states are missing (a folder that was never
    /// calculated, or an earlier failed recalculation) this falls back to a
    /// full recalculation.
    pub fn recalc_from(&mut self, parent: NodeId, start: usize) -> Result<(), String> {
        if self.needs_full_recalc || self.init_route_state.is_none() {
            return self.recalc();
        }
        match self.recalc_suffix(parent, start) {
            Ok(true) => Ok(()),
            Ok(false) => self.recalc(),
            Err(e) => {
                self.needs_full_recalc = true;
                Err(e)
            }
        }
    }

    /// Recompute from the node's own position (its definition changed).
    pub fn recalc_from_node(&mut self, id: NodeId) -> Result<(), String> {
        match self.position_of(id) {
            Some((parent, idx)) => self.recalc_from(parent, idx),
            None => self.recalc(),
        }
    }

    /// `(parent folder, index among its children)` of a node.
    pub fn position_of(&self, id: NodeId) -> Option<(NodeId, usize)> {
        let parent = self.nodes.get(&id)?.parent()?;
        let idx = self.folder(parent)?.children.iter().position(|c| *c == id)?;
        Some((parent, idx))
    }

    /// The incremental pass; `Ok(false)` means "no prefix state available".
    fn recalc_suffix(&mut self, mut parent: NodeId, mut start: usize) -> Result<bool, String> {
        loop {
            let (children, parent_init, old_final, grandparent) = match self.nodes.get(&parent) {
                Some(Node::Folder(f)) => (f.children.clone(), f.init_state.clone(), f.final_state.clone(), f.parent),
                _ => return Err(format!("Cannot find folder with id: {}", parent)),
            };
            let first = start.min(children.len());
            let entering: Arc<RouteState> = if first == 0 {
                match parent_init {
                    Some(s) => s,
                    None => return Ok(false),
                }
            } else {
                match self.nodes.get(&children[first - 1]).and_then(|n| n.final_state().cloned()) {
                    Some(s) => s,
                    None => return Ok(false),
                }
            };
            let mut cur = entering;
            for child in &children[first..] {
                self.recursive_recalc(*child, cur.clone())?;
                if let Some(next) = self.nodes.get(child).and_then(|n| n.final_state().cloned()) {
                    cur = next;
                }
            }
            let child_errors = children.iter().any(|c| self.nodes.get(c).map(|n| n.has_errors()).unwrap_or(false));
            let unchanged = old_final.as_ref().map(|o| o.py_eq(&cur)).unwrap_or(false);
            if let Some(Node::Folder(f)) = self.nodes.get_mut(&parent) {
                f.final_state = Some(cur);
                f.child_errors = child_errors;
            }
            let Some(gp) = grandparent else { return Ok(true) };
            if unchanged {
                // the state leaving this folder did not change: nothing after
                // it needs recomputing, only the error flags go up
                self.refresh_child_errors_upwards(gp);
                return Ok(true);
            }
            let idx = self.folder(gp).and_then(|f| f.children.iter().position(|c| *c == parent)).ok_or_else(|| format!("Folder {} is not a child of {}", parent, gp))?;
            parent = gp;
            start = idx + 1;
        }
    }

    /// Recompute `child_errors` on a folder and each of its ancestors.
    fn refresh_child_errors_upwards(&mut self, mut folder: NodeId) {
        loop {
            let Some(f) = self.folder(folder) else { return };
            let errors = f.children.iter().any(|c| self.nodes.get(c).map(|n| n.has_errors()).unwrap_or(false));
            let parent = f.parent;
            if let Some(Node::Folder(f)) = self.nodes.get_mut(&folder) {
                f.child_errors = errors;
            }
            match parent {
                Some(p) => folder = p,
                None => return,
            }
        }
    }

    /// `_recursive_recalc`
    fn recursive_recalc(&mut self, id: NodeId, cur_state: Arc<RouteState>) -> Result<(), String> {
        match self.nodes.get_mut(&id) {
            Some(Node::Group(g)) => {
                g.init_state = Some(cur_state.clone());
                self.calc_single_event(id, cur_state)
            }
            Some(Node::Folder(f)) => {
                f.init_state = Some(cur_state.clone());
                f.child_errors = false;
                let children = f.children.clone();
                let mut cur = cur_state;
                let mut child_errors = false;
                for child in children {
                    self.recursive_recalc(child, cur.clone())?;
                    let node = self.nodes.get(&child).unwrap();
                    cur = node.final_state().cloned().unwrap_or(cur);
                    if node.has_errors() {
                        child_errors = true;
                    }
                }
                if let Some(Node::Folder(f)) = self.nodes.get_mut(&id) {
                    f.final_state = Some(cur);
                    f.child_errors = child_errors;
                }
                Ok(())
            }
            None => Ok(()),
        }
    }

    /// `_calc_single_event`: apply once, then again with any level-up moves
    /// the level gain unlocked.
    fn calc_single_event(&mut self, group_id: NodeId, prev_state: Arc<RouteState>) -> Result<(), String> {
        self.apply_group(group_id, prev_state.clone(), None)?;
        let post_state = self.group(group_id).and_then(|g| g.final_state.clone()).unwrap_or_else(|| prev_state.clone());

        let prev_level = prev_state.solo_pkmn.cur_level;
        let post_level = post_state.solo_pkmn.cur_level;
        let mut to_learn: Vec<LearnMoveEventDefinition> = Vec::new();
        for lvl in (prev_level + 1)..=post_level {
            for def in self.level_up_move_defs.values() {
                if def.matches_level_up_move(lvl, &prev_state.solo_pkmn.name) {
                    to_learn.push(def.clone());
                }
            }
        }
        if post_state.solo_pkmn.name != prev_state.solo_pkmn.name {
            let species = post_state.solo_pkmn.species_def.clone();
            self.add_level_up_moves_for_mon(&species);
            for def in self.level_up_move_defs.values() {
                if def.matches_level_up_move(post_level, &post_state.solo_pkmn.name) {
                    to_learn.push(def.clone());
                }
            }
        }
        if !to_learn.is_empty() {
            self.apply_group(group_id, prev_state, Some(to_learn))?;
        }
        Ok(())
    }

    fn new_item(&mut self, parent: NodeId, def: EventDefinition, shares_group_definition: bool, to_defeat_mon: Option<EnemyPkmn>, exp_split_num: i64, pay_day_amount: i64, defeating_trainer: bool, thief: bool) -> NodeId {
        let id = self.mint_id();
        self.items.insert(
            id,
            EventItem {
                id,
                parent,
                enabled: Some(true),
                name: String::new(),
                to_defeat_mon,
                exp_split_num,
                pay_day_amount,
                defeating_trainer,
                thief,
                event_definition: def,
                shares_group_definition,
                init_state: None,
                final_state: None,
                error_message: String::new(),
                warning_message: String::new(),
            },
        );
        id
    }

    /// `EventItem.apply`. Returns the resolved learn-move destination when
    /// the item is a learn-move (Python writes it back into the shared
    /// definition object).
    fn apply_item(&mut self, item_id: NodeId, cur_state: Arc<RouteState>) -> Result<(), String> {
        let gen = self.gen_data()?.clone();
        let parent = self.items[&item_id].parent;
        let parent_enabled = self.is_enabled(parent);
        let item = self.items.get_mut(&item_id).unwrap();
        item.init_state = Some(cur_state.clone());
        item.enabled = item.event_definition.enabled;
        if !(item.enabled.unwrap_or(false) && parent_enabled) {
            item.final_state = Some(cur_state);
            item.error_message = String::new();
            item.warning_message = String::new();
            item.name = format!("Disabled: {}", item.event_definition.get_label(&gen)?);
            return Ok(());
        }
        // fights and learn moves set their own name below
        if item.to_defeat_mon.is_none() && item.event_definition.learn_move.is_none() {
            item.name = item.event_definition.get_item_label(&gen)?;
        }

        let mut warning_message = String::new();
        let (final_state, error_message): (Arc<RouteState>, String) = if let Some(mon) = item.to_defeat_mon.clone() {
            let (defeated_trainer_name, render_trainer_name): (Option<String>, String) = if let Some(td) = &item.event_definition.trainer_def {
                (
                    if item.defeating_trainer { Some(td.trainer_name.clone()) } else { None },
                    td.trainer_name.clone(),
                )
            } else {
                let is_trainer_pkmn = item
                    .event_definition
                    .wild_pkmn_info
                    .as_ref()
                    .map(|w| w.trainer_pkmn)
                    .unwrap_or(false);
                (None, if is_trainer_pkmn { "TrainerPkmn".to_string() } else { "WildPkmn".to_string() })
            };
            item.name = format!("{}: {}", render_trainer_name, mon.name);
            // the steal lands before the KO, so a stolen Lucky Egg / Macho
            // Brace already counts for this mon's exp, as in the games
            let (pre_ko, steal_error) = if item.thief {
                let (st, e) = cur_state.steal_held_item(&gen, &mon)?;
                if e.is_empty() {
                    item.name.push_str(&format!(" (stole {})", mon.held_item_str()));
                }
                (Arc::new(st), e)
            } else {
                (cur_state, String::new())
            };
            let (st, e) = pre_ko.defeat_pkmn(&gen, &mon, defeated_trainer_name.as_deref(), item.exp_split_num, item.pay_day_amount)?;
            let e = match (steal_error.is_empty(), e.is_empty()) {
                (true, _) => e,
                (false, true) => steal_error,
                (false, false) => format!("{}; {}", steal_error, e),
            };
            (Arc::new(st), e)
        } else if item.event_definition.rare_candy.is_some() {
            let (st, e) = cur_state.rare_candy(&gen)?;
            (Arc::new(st), e)
        } else if let Some(v) = &item.event_definition.vitamin {
            let name = gen
                .item_db()
                .get_item(&v.vitamin)
                .map(|i| i.name.clone())
                .ok_or_else(|| "'NoneType' object has no attribute 'name'".to_string())?;
            let (st, e) = cur_state.vitamin(&gen, &name)?;
            (Arc::new(st), e)
        } else if let Some(i) = &item.event_definition.item_event_def {
            let (st, e) = if i.is_acquire {
                cur_state.add_item(&gen, &i.item_name, i.item_amount, i.with_money, i.custom_price)
            } else {
                cur_state.remove_item(&gen, &i.item_name, i.item_amount, i.with_money, i.custom_price)
            };
            (Arc::new(st), e)
        } else if let Some(lm) = item.event_definition.learn_move.clone() {
            let (dest, _) = cur_state
                .solo_pkmn
                .get_move_destination(lm.move_to_learn.as_deref(), lm.destination, lm.force_destination);
            item.event_definition.learn_move.as_mut().unwrap().destination = dest;
            item.name = item.event_definition.get_label(&gen)?;
            let (st, e) = cur_state.learn_move(&gen, lm.move_to_learn.as_deref(), dest, &lm.source, lm.force_destination)?;
            (Arc::new(st), e)
        } else if let Some(h) = &item.event_definition.hold_item {
            let (st, e) = cur_state.hold_item(&gen, h.item_name.as_deref(), h.consumed)?;
            (Arc::new(st), e)
        } else if item.event_definition.blackout.is_some() {
            let (st, e) = cur_state.blackout(&gen);
            (Arc::new(st), e)
        } else if let Some(e) = &item.event_definition.evolution {
            let (st, e) = cur_state.evolve(&gen, e.evolved_species.as_deref(), e.by_stone.as_deref())?;
            (Arc::new(st), e)
        } else if let Some(r) = &item.event_definition.bag_reorder {
            // a reorder never fails: items are matched by name and any
            // slot mismatch is a warning, not an error
            if r.swaps.is_empty() {
                (cur_state, String::new())
            } else {
                let (st, warnings) = cur_state.reorder_bag(&r.swaps);
                warning_message = warnings.join(", ");
                (Arc::new(st), String::new())
            }
        } else {
            // notes / save / heal: the state passes through unchanged
            (cur_state, String::new())
        };
        let item = self.items.get_mut(&item_id).unwrap();
        item.final_state = Some(final_state);
        item.error_message = error_message;
        item.warning_message = warning_message;
        if item.event_definition.notes.starts_with(consts::RECORDING_ERROR_FRAGMENT) {
            item.error_message = item.event_definition.notes.clone();
        }
        Ok(())
    }

    /// After a learn-move item resolved its destination, mirror Python's
    /// shared-object mutation into the group definition / level-up map.
    fn write_back_destination(&mut self, group_id: NodeId, item_id: NodeId) {
        let item = &self.items[&item_id];
        let Some(lm) = &item.event_definition.learn_move else { return };
        let dest = lm.destination;
        if item.shares_group_definition {
            if let Some(Node::Group(g)) = self.nodes.get_mut(&group_id) {
                if let Some(glm) = g.event_definition.learn_move.as_mut() {
                    glm.destination = dest;
                }
            }
        } else {
            let key = lm.get_level_up_key();
            if let Some(Node::Group(g)) = self.nodes.get_mut(&group_id) {
                for def in g.level_up_learn_event_defs.iter_mut() {
                    if def.get_level_up_key() == key {
                        def.destination = dest;
                    }
                }
            }
            if let Some(k) = key {
                if let Some(def) = self.level_up_move_defs.get_mut(&k) {
                    def.destination = dest;
                }
            }
        }
    }

    fn item_final(&self, item_id: NodeId) -> Arc<RouteState> {
        self.items[&item_id].final_state.clone().expect("applied item")
    }

    /// `EventGroup.apply`
    pub(crate) fn apply_group(&mut self, group_id: NodeId, cur_state: Arc<RouteState>, level_up_defs: Option<Vec<LearnMoveEventDefinition>>) -> Result<(), String> {
        let gen = self.gen_data()?.clone();
        let parent_enabled = {
            let g = self.group(group_id).unwrap();
            self.is_enabled(g.parent)
        };
        let old_items: Vec<NodeId> = self.group(group_id).map(|g| g.event_items.clone()).unwrap_or_default();
        for old in old_items {
            self.items.remove(&old);
        }
        let (def, label) = {
            let g = self.nodes.get_mut(&group_id).and_then(|n| n.as_group_mut()).unwrap();
            let label = g.event_definition.get_label(&gen)?;
            g.init_state = Some(cur_state.clone());
            g.pkmn_after_levelups.clear();
            g.event_items.clear();
            g.enabled = g.event_definition.enabled;
            if !(g.enabled.unwrap_or(false) && parent_enabled) {
                g.final_state = Some(cur_state);
                g.error_messages.clear();
                g.warning_messages.clear();
                g.name = format!("Disabled: {}", label);
                return Ok(());
            }
            g.name = label.clone();
            g.level_up_learn_event_defs = level_up_defs.unwrap_or_default();
            (g.event_definition.clone(), label)
        };
        let level_up_defs = self.group(group_id).unwrap().level_up_learn_event_defs.clone();

        let mut items: Vec<NodeId> = Vec::new();
        let mut state = cur_state;

        if def.trainer_def.is_some() || def.wild_pkmn_info.is_some() {
            let mut pkmn_counter: indexmap::IndexMap<String, i64> = indexmap::IndexMap::new();
            let pkmn_to_fight = def.get_pokemon_list(&gen, false)?;
            let n = pkmn_to_fight.len();
            for (order_idx, (definition_idx, cur_pkmn)) in pkmn_to_fight.iter().enumerate() {
                let exp_split = if def.wild_pkmn_info.is_some() || def.trainer_def.as_ref().map(|t| t.exp_split.is_empty()).unwrap_or(true) {
                    1
                } else {
                    let td = def.trainer_def.as_ref().unwrap();
                    *td.exp_split
                        .get(*definition_idx as usize)
                        .ok_or_else(|| "list index out of range".to_string())?
                };
                let pay_day_amount = if def.wild_pkmn_info.is_some() || order_idx != n - 1 {
                    0
                } else {
                    match def.trainer_def.as_ref().and_then(|t| t.pay_day_amount_int()) {
                        Some(v) => v,
                        None => 0,
                    }
                };
                let defeating_trainer = order_idx == n - 1;
                let thief = def.trainer_def.as_ref().map(|t| t.thief_mons.contains(definition_idx)).unwrap_or(false);
                let item_id = self.new_item(group_id, def.clone(), true, Some(cur_pkmn.clone()), exp_split, pay_day_amount, defeating_trainer, thief);
                self.apply_item(item_id, state.clone())?;
                items.push(item_id);
                *pkmn_counter.entry(cur_pkmn.name.clone()).or_insert(0) += 1;

                let mut next_state = self.item_final(item_id);
                if next_state.solo_pkmn.cur_level != state.solo_pkmn.cur_level {
                    for lm in level_up_defs.iter() {
                        if matches!(lm.level, crate::events::LevelVal::Int(l) if l == next_state.solo_pkmn.cur_level) {
                            let lm_id = self.new_item(group_id, EventDefinition::with_learn_move(lm.clone()), false, None, 1, 0, false, false);
                            self.apply_item(lm_id, next_state.clone())?;
                            self.write_back_destination(group_id, lm_id);
                            items.push(lm_id);
                            next_state = self.item_final(lm_id);
                        }
                    }
                    if order_idx + 1 < n {
                        let next_name = &pkmn_to_fight[order_idx + 1].1.name;
                        let count = pkmn_counter.get(next_name).copied().unwrap_or(0) + 1;
                        if let Some(Node::Group(g)) = self.nodes.get_mut(&group_id) {
                            g.pkmn_after_levelups.push(format!("{}{}", next_name, count));
                        }
                    } else if let Some(Node::Group(g)) = self.nodes.get_mut(&group_id) {
                        g.pkmn_after_levelups.push("end".to_string());
                    }
                }
                state = next_state;
            }
        } else if let Some(rc) = &def.rare_candy {
            if rc.amount <= 0 {
                let item_id = self.new_item(group_id, EventDefinition::default(), false, None, 1, 0, false, false);
                self.apply_item(item_id, state.clone())?;
                items.push(item_id);
            }
            for _ in 0..rc.amount.max(0) {
                let item_id = self.new_item(group_id, def.clone(), true, None, 1, 0, false, false);
                self.apply_item(item_id, state.clone())?;
                items.push(item_id);
                let mut next_state = self.item_final(item_id);
                if next_state.solo_pkmn.cur_level != state.solo_pkmn.cur_level {
                    for lm in level_up_defs.iter() {
                        if matches!(lm.level, crate::events::LevelVal::Int(l) if l == next_state.solo_pkmn.cur_level) {
                            let lm_id = self.new_item(group_id, EventDefinition::with_learn_move(lm.clone()), false, None, 1, 0, false, false);
                            self.apply_item(lm_id, next_state.clone())?;
                            self.write_back_destination(group_id, lm_id);
                            items.push(lm_id);
                            next_state = self.item_final(lm_id);
                        }
                    }
                }
                state = next_state;
            }
        } else if let Some(v) = &def.vitamin {
            for _ in 0..v.amount.max(0) {
                let item_id = self.new_item(group_id, def.clone(), true, None, 1, 0, false, false);
                self.apply_item(item_id, state.clone())?;
                items.push(item_id);
                state = self.item_final(item_id);
            }
        } else {
            let item_id = self.new_item(group_id, def.clone(), true, None, 1, 0, false, false);
            self.apply_item(item_id, state.clone())?;
            self.write_back_destination(group_id, item_id);
            items.push(item_id);
            if let Some(first) = level_up_defs.first() {
                let after = self.item_final(item_id);
                let lm_id = self.new_item(group_id, EventDefinition::with_learn_move(first.clone()), false, None, 1, 0, false, false);
                self.apply_item(lm_id, after)?;
                self.write_back_destination(group_id, lm_id);
                items.push(lm_id);
            }
        }

        if items.is_empty() {
            return Err(format!("Something went wrong generating event group: {}", label));
        }

        let collect = |field: fn(&EventItem) -> &String| -> Vec<String> { items.iter().map(|i| field(&self.items[i])).filter(|m| !m.is_empty()).cloned().collect() };
        let error_messages = collect(|i| &i.error_message);
        let warning_messages = collect(|i| &i.warning_message);
        let last_final = self.item_final(*items.last().unwrap());
        if let Some(Node::Group(g)) = self.nodes.get_mut(&group_id) {
            g.event_items = items;
            // errors replace the label; warnings keep it
            g.name = if error_messages.is_empty() { label } else { error_messages.join(", ") };
            g.error_messages = error_messages;
            g.warning_messages = warning_messages;
            g.final_state = Some(last_final);
        }
        Ok(())
    }

    /// `serialize_metadata`-style summary values used by the route list and
    /// tests: level, xp to next level, etc. for a node or item.
    pub fn is_major_fight(&self, id: NodeId) -> bool {
        let Some(gen) = self.gen() else { return false };
        match self.nodes.get(&id) {
            Some(Node::Group(g)) => match &g.event_definition.trainer_def {
                Some(td) => gen.is_major_fight(&td.trainer_name),
                None => false,
            },
            _ => false,
        }
    }
}
