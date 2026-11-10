//! The route list: visible rows (folders, events, level-up moves and the
//! items of multi-part events), filters / search, selection and every
//! structural edit (move, transfer, delete, enable, highlight, folders).

use std::collections::HashSet;

use serde_json::{json, Value};

use xpr_core::consts;
use xpr_engine::{NodeId, ObjKind};

use super::{b, i, ids, s, strs, Session};

/// Every filter of the desktop filter bar: (filter type, label, SF symbol).
pub(crate) const FILTERS: [(&str, &str, &str); 20] = [
    (consts::MAJOR_BATTLE_FILTER, "Major Battles", "star.fill"),
    (consts::TASK_TRAINER_BATTLE, "Trainer Battles", "person.2.fill"),
    (consts::TASK_RARE_CANDY, "Rare Candy", "birthday.cake.fill"),
    (consts::TASK_VITAMIN, "Vitamins", "pill.fill"),
    (consts::TASK_LEARN_MOVE_LEVELUP, "Level-up Moves", "arrow.up.circle.fill"),
    (consts::TASK_LEARN_MOVE_TM, "TM / HM / Tutor", "opticaldisc.fill"),
    (consts::TASK_FIGHT_WILD_PKMN, "Wild Pokémon", "leaf.fill"),
    (consts::TASK_HOLD_ITEM, "Hold Item", "hand.raised.fill"),
    (consts::TASK_GET_FREE_ITEM, "Get Free Item", "gift.fill"),
    (consts::TASK_PURCHASE_ITEM, "Purchase Item", "cart.fill"),
    (consts::TASK_SELL_ITEM, "Sell Item", "dollarsign.circle.fill"),
    (consts::TASK_USE_ITEM, "Use / Drop Item", "trash.fill"),
    (consts::TASK_REORDER_BAG, "Reorder Bag", "arrow.up.arrow.down"),
    (consts::TASK_BLACKOUT, "Blackout", "moon.zzz.fill"),
    (consts::TASK_HEAL, "Heal", "cross.case.fill"),
    (consts::TASK_EVOLUTION, "Evolution", "sparkles"),
    (consts::TASK_EV_OVERRIDE, "EV Override", "slider.horizontal.3"),
    (consts::TASK_DAY_CARE, "Day Care", "house.fill"),
    (consts::TASK_NOTES_ONLY, "Notes Only", "note.text"),
    (consts::ERROR_SEARCH, "Invalid Events", "exclamationmark.triangle.fill"),
];

/// The SF symbol for an event type (rows and pickers).
pub(crate) fn type_symbol(event_type: &str) -> &'static str {
    match event_type {
        consts::TASK_SAVE => "square.and.arrow.down.fill",
        _ => FILTERS.iter().find(|f| f.0 == event_type).map(|f| f.2).unwrap_or("circle.fill"),
    }
}

impl Session {
    pub(super) fn tree_call(&mut self, method: &str, p: &Value) -> Result<Value, String> {
        let sel = ids(p, "ids");
        match method {
            "tree.rows" => self.tree_rows(),
            "tree.filters" => Ok(self.filters_view()),
            "tree.set_filters" => {
                self.ctrl.set_route_filter_types(strs(p, "types"));
                self.ctrl.set_route_search(s(p, "search"));
                self.tree_rows()
            }
            "tree.toggle" => {
                let id = i(p, "id").ok_or("id")?;
                match self.ctrl.router.obj_kind(id) {
                    Some(ObjKind::Folder) => {
                        let f = self.ctrl.router.folder(id).ok_or("folder")?;
                        let cur = self.expand.get(&f.name).copied().unwrap_or_else(|| f.is_expanded());
                        self.expand.insert(f.name.clone(), !cur);
                    }
                    Some(ObjKind::Group) => {
                        if !self.group_expanded.remove(&id) {
                            self.group_expanded.insert(id);
                        }
                    }
                    _ => {}
                }
                self.tree_rows()
            }
            "tree.expand_all" => {
                let expanded = b(p, "expanded");
                let names: Vec<String> = self.ctrl.get_all_folder_names();
                for n in names {
                    self.expand.insert(n, expanded);
                }
                self.tree_rows()
            }
            "tree.select" => {
                self.ctrl.select_new_events(sel);
                Ok(json!(true))
            }
            "tree.next_invalid" => {
                // the status chip's "cycle through invalid events"
                let invalid = self.ctrl.get_all_invalid_event_ids();
                if invalid.is_empty() {
                    return Ok(Value::Null);
                }
                let cur = self.ctrl.get_single_selected_event_id(true);
                let next = cur.and_then(|c| invalid.iter().position(|x| *x == c)).map(|p| invalid[(p + 1) % invalid.len()]).unwrap_or(invalid[0]);
                self.reveal(next);
                self.ctrl.select_new_events(vec![next]);
                Ok(json!(next))
            }
            "tree.reveal" => {
                let id = i(p, "id").ok_or("id")?;
                self.reveal(id);
                Ok(json!(true))
            }
            "tree.find_trainer" => {
                Ok(json!(self.ctrl.find_first_event_by_trainer_name(s(p, "name"))))
            }
            "tree.jump_targets" => Ok(self.jump_targets()),
            "edit.move_up" => {
                self.ctrl.move_groups_up(&sel);
                self.tree_rows()
            }
            "edit.move_down" => {
                let mut r = sel.clone();
                r.reverse();
                self.ctrl.move_groups_down(&r);
                self.tree_rows()
            }
            "edit.move_folder_up" => {
                self.ctrl.move_groups_to_adjacent_folder_up(&sel);
                self.tree_rows()
            }
            "edit.move_folder_down" => {
                let mut r = sel.clone();
                r.reverse();
                self.ctrl.move_groups_to_adjacent_folder_down(&r);
                self.tree_rows()
            }
            "edit.move_to" => {
                // drag and drop: into a folder, before or after a row
                let dest = i(p, "folder").unwrap_or(self.ctrl.router.root_id);
                self.ctrl.move_events_to_position(&sel, dest, i(p, "after"), i(p, "before"));
                self.tree_rows()
            }
            "edit.delete" => {
                if self.battle_for.map(|b| sel.contains(&b)).unwrap_or(false) {
                    self.battle_for = None;
                }
                self.ctrl.delete_events(&sel);
                self.tree_rows()
            }
            "edit.set_enabled" => {
                self.ctrl.set_events_enabled(&sel, b(p, "enabled"));
                self.tree_rows()
            }
            "edit.toggle_highlight" => {
                self.ctrl.toggle_event_highlight(&sel);
                self.tree_rows()
            }
            "edit.highlight" => {
                // the desktop's Highlight N: clears it when every selected
                // event already has N
                let n = i(p, "n");
                let all_have = n.is_some()
                    && sel.iter().all(|id| self.def_of(*id).and_then(|d| d.get_highlight_type()) == n);
                self.ctrl.set_event_highlight(&sel, if all_have { None } else { n });
                self.tree_rows()
            }
            "edit.transfer" => {
                let folder = s(p, "folder").to_string();
                if b(p, "create") {
                    if folder.trim().is_empty() || self.ctrl.get_all_folder_names().contains(&folder) {
                        return Err("Pick a new, unused folder name".into());
                    }
                    let after = sel.first().copied();
                    self.ctrl.finalize_new_folder(&folder, None, after);
                }
                self.ctrl.transfer_to_folder(&sel, &folder);
                self.tree_rows()
            }
            "edit.set_quantity" => {
                let id = i(p, "id").ok_or("id")?;
                let n = i(p, "quantity").filter(|n| *n >= 1).ok_or("Quantity must be at least 1")?;
                let mut def = self.def_of(id).ok_or("event")?;
                if let Some(it) = def.item_event_def.as_mut() {
                    it.item_amount = n;
                } else if let Some(v) = def.vitamin.as_mut() {
                    v.amount = n;
                } else if let Some(rc) = def.rare_candy.as_mut() {
                    rc.amount = n;
                } else if let Some(w) = def.wild_pkmn_info.as_mut() {
                    w.quantity = n;
                } else {
                    return Err("This event has no quantity".into());
                }
                self.ctrl.update_existing_event(id, def);
                self.tree_rows()
            }
            "folder.names" => Ok(json!(self.ctrl.get_all_folder_names().into_iter().filter(|n| n != consts::ROOT_FOLDER_NAME).collect::<Vec<_>>())),
            "folder.destinations" => {
                let mut invalid: HashSet<String> = HashSet::new();
                for id in &sel {
                    invalid.extend(self.ctrl.get_invalid_folders(*id));
                }
                let all: Vec<String> = self.ctrl.get_all_folder_names().into_iter().filter(|n| !invalid.contains(n)).collect();
                Ok(json!(all))
            }
            "folder.new" => {
                let name = s(p, "name").trim().to_string();
                if name.is_empty() || self.ctrl.get_all_folder_names().contains(&name) {
                    return Err("Folder names must be unique and not empty".into());
                }
                let after = i(p, "after").filter(|x| self.ctrl.router.contains_id(*x)).or_else(|| self.ctrl.get_single_selected_event_id(false));
                self.ctrl.finalize_new_folder(&name, None, after);
                self.tree_rows()
            }
            "folder.rename" => {
                let old = s(p, "old").to_string();
                let name = s(p, "name").trim().to_string();
                if old == consts::ROOT_FOLDER_NAME {
                    return Err("Can't rename that folder".into());
                }
                if name.is_empty() || self.ctrl.get_all_folder_names().contains(&name) {
                    return Err("Folder names must be unique and not empty".into());
                }
                if let Some(e) = self.expand.remove(&old) {
                    self.expand.insert(name.clone(), e);
                }
                self.ctrl.finalize_new_folder(&name, Some(&old), None);
                self.tree_rows()
            }
            "folder.split" => {
                let id = i(p, "id").ok_or("id")?;
                self.ctrl.split_folder_at_current_event(id);
                self.tree_rows()
            }
            "folder.purge" => {
                self.ctrl.purge_empty_folders();
                self.tree_rows()
            }
            "folder.unique_name" => Ok(json!(self.ctrl.unique_folder_name(s(p, "base")))),
            _ => Err(format!("unknown method {}", method)),
        }
    }

    /// Expand every folder above `id` so the row is visible.
    pub(crate) fn reveal(&mut self, id: NodeId) {
        let mut cur = match self.ctrl.router.obj_kind(id) {
            Some(ObjKind::Item) => self.ctrl.router.item(id).map(|i| i.parent),
            _ => Some(id),
        };
        if let Some(c) = cur {
            if self.ctrl.router.obj_kind(id) == Some(ObjKind::Item) {
                self.group_expanded.insert(c);
            }
        }
        while let Some(c) = cur {
            let parent = self.ctrl.router.parent_of(c);
            if let Some(pid) = parent {
                if let Some(f) = self.ctrl.router.folder(pid) {
                    if pid != self.ctrl.router.root_id {
                        self.expand.insert(f.name.clone(), true);
                    }
                }
            }
            cur = parent;
        }
    }

    fn filters_view(&self) -> Value {
        let gen = self.ctrl.gen();
        let active: Vec<String> = self.ctrl.get_route_filter_types().map(|f| f.to_vec()).unwrap_or_default();
        let list: Vec<Value> = FILTERS
            .iter()
            .filter(|f| f.0 != consts::TASK_REORDER_BAG || gen.as_ref().map(|g| g.supports_bag_reorder()).unwrap_or(false) || active.iter().any(|a| a == f.0))
            .map(|f| json!({"type": f.0, "label": f.1, "symbol": f.2, "active": active.iter().any(|a| a == f.0)}))
            .collect();
        json!({"filters": list, "search": self.ctrl.get_route_search_string().unwrap_or("")})
    }

    /// Gym leaders and the Elite Four for the "jump to" menu (the desktop's
    /// number-key shortcuts).
    fn jump_targets(&self) -> Value {
        let Some(gen) = self.ctrl.gen() else { return json!([]) };
        let mut out = Vec::new();
        for (idx, e) in gen.get_gym_leader_entries().iter().enumerate() {
            if let Some(id) = e.names().iter().find_map(|n| self.ctrl.find_first_event_by_trainer_name(n)) {
                out.push(json!({"label": format!("Gym {}: {}", idx + 1, e.names()[0]), "id": id, "group": "Gym Leaders"}));
            }
        }
        for e in gen.get_elite_four_and_champion_names() {
            if let Some(id) = e.names().iter().find_map(|n| self.ctrl.find_first_event_by_trainer_name(n)) {
                out.push(json!({"label": e.names()[0].clone(), "id": id, "group": "Elite Four & Champion"}));
            }
        }
        Value::Array(out)
    }

    pub(crate) fn tag_color(&self, tag: &str) -> Option<String> {
        if let Some(idx) = consts::ALL_HIGHLIGHT_LABELS.iter().position(|l| *l == tag) {
            return Some(self.cfg.get_highlight_color(idx as i64 + 1));
        }
        for (cat, t) in consts::FIGHT_CATEGORY_TO_TAG {
            if *t == *tag {
                return Some(self.cfg.get_fight_category_color(cat));
            }
        }
        Some(match tag {
            t if t == consts::EVENT_TAG_ERRORS => "#e05555".to_string(),
            t if t == consts::EVENT_TAG_WARNINGS => self.cfg.get_warning_color().to_string(),
            t if t == consts::EVENT_TAG_IMPORTANT => "#8e8e93".to_string(),
            t if t == consts::HIGHLIGHT_LABEL => "#156152".to_string(),
            _ => return None,
        })
    }

    pub(crate) fn tree_rows(&mut self) -> Result<Value, String> {
        let Some(gen) = self.ctrl.gen() else { return Ok(json!({"rows": []})) };
        let filter: Option<Vec<String>> = self.ctrl.get_route_filter_types().map(|f| f.to_vec());
        let search: Option<String> = self.ctrl.get_route_search_string().map(|s| s.to_string());
        let flatten = filter.as_ref().map(|f| f.iter().any(|t| t == consts::MAJOR_BATTLE_FILTER)).unwrap_or(false);
        let filtering = filter.is_some() || search.is_some();
        let mut rows: Vec<Value> = Vec::new();
        let color_major = self.cfg.get_color_major_battles();
        let fade_folders = self.cfg.get_fade_folder_text();
        let router = &self.ctrl.router;
        // (id, depth, parent) to visit in order
        let push_group = |rows: &mut Vec<Value>, gid: NodeId, depth: usize, this: &Session| {
            if !router.do_render(gid, &gen, search.as_deref(), filter.as_deref()) {
                return;
            }
            let Some(g) = router.group(gid) else { return };
            let mut level_up: Vec<NodeId> = Vec::new();
            let mut other: Vec<NodeId> = Vec::new();
            if g.event_items.len() > 1 {
                for item_id in &g.event_items {
                    let lu = router.item(*item_id).and_then(|i| i.event_definition.learn_move.as_ref()).map(|lm| lm.source == consts::MOVE_SOURCE_LEVELUP).unwrap_or(false);
                    if lu {
                        level_up.push(*item_id);
                    } else {
                        other.push(*item_id);
                    }
                }
            }
            let expandable = !other.is_empty();
            let expanded = expandable && this.group_expanded.contains(&gid);
            rows.push(this.row_json(gid, "group", depth, expandable, expanded, &gen, color_major, false));
            for lu in level_up {
                rows.push(this.row_json(lu, "levelup", depth, false, false, &gen, color_major, false));
            }
            if expanded {
                for o in other {
                    rows.push(this.row_json(o, "item", depth + 1, false, false, &gen, color_major, false));
                }
            }
        };
        if flatten {
            for gid in router.all_groups() {
                push_group(&mut rows, gid, 0, self);
            }
        } else {
            let mut stack: Vec<(NodeId, usize)> = router.children_of(router.root_id).into_iter().rev().map(|c| (c, 0)).collect();
            while let Some((id, depth)) = stack.pop() {
                if !router.do_render(id, &gen, search.as_deref(), filter.as_deref()) {
                    continue;
                }
                match router.obj_kind(id) {
                    Some(ObjKind::Folder) => {
                        let f = router.folder(id).unwrap();
                        // a search/filter always shows the matching contents
                        let expanded = filtering || self.expand.get(&f.name).copied().unwrap_or_else(|| f.is_expanded());
                        rows.push(self.row_json(id, "folder", depth, true, expanded, &gen, color_major, fade_folders));
                        if expanded {
                            for c in f.children.iter().rev() {
                                stack.push((*c, depth + 1));
                            }
                        }
                    }
                    Some(ObjKind::Group) => push_group(&mut rows, id, depth, self),
                    _ => {}
                }
            }
        }
        Ok(json!({"rows": rows, "filtering": filtering, "flattened": flatten}))
    }

    #[allow(clippy::too_many_arguments)]
    fn row_json(&self, id: NodeId, kind: &str, depth: usize, expandable: bool, expanded: bool, gen: &xpr_data::GenData, color_major: bool, faded: bool) -> Value {
        let router = &self.ctrl.router;
        let v = router.row_values(id, gen);
        let tags = router.get_tags(id, gen, color_major);
        let tag = tags.first().copied();
        let def = self.def_of(id);
        let event_type = match kind {
            "folder" => "Folder",
            _ => def.as_ref().map(|d| d.get_event_type()).unwrap_or(""),
        };
        let qty = def.as_ref().and_then(|d| {
            d.item_event_def.as_ref().map(|x| x.item_amount)
                .or(d.vitamin.as_ref().map(|x| x.amount))
                .or(d.rare_candy.as_ref().map(|x| x.amount))
                .or(d.wild_pkmn_info.as_ref().map(|x| x.quantity))
        });
        let child_count = if kind == "folder" { router.folder(id).map(|f| f.children.len()).unwrap_or(0) } else { 0 };
        let warnings = match kind {
            "group" => router.group(id).map(|g| g.has_warnings()).unwrap_or(false),
            "item" | "levelup" => router.item(id).map(|i| i.has_warnings()).unwrap_or(false),
            _ => false,
        };
        let has_notes = def.as_ref().map(|d| !d.notes.trim().is_empty()).unwrap_or(false);
        let species = if kind == "group" {
            def.as_ref().and_then(|d| {
                if d.wild_pkmn_info.is_some() {
                    d.wild_pkmn_info.as_ref().map(|w| w.name.clone())
                } else if d.trainer_def.is_some() {
                    d.pokemon_list(gen).ok().and_then(|l| l.last().map(|m| m.name.clone()))
                } else {
                    None
                }
            })
        } else {
            None
        };
        let parent = match kind {
            "item" | "levelup" => router.item(id).map(|i| i.parent),
            _ => router.parent_of(id),
        };
        json!({
            "id": id,
            "kind": kind,
            "parent": parent,
            "depth": depth,
            "name": v.name,
            "type": event_type,
            "symbol": if kind == "folder" { "folder.fill" } else { type_symbol(event_type) },
            "species": species,
            "expandable": expandable,
            "expanded": expanded,
            "enabled": v.is_enabled,
            "errors": v.has_errors,
            "warnings": warnings,
            "major": v.is_major_fight,
            "faded": faded,
            "notes": has_notes,
            "tag": tag,
            "color": tag.and_then(|t| self.tag_color(t)),
            "qty": qty,
            "child_count": child_count,
            "level": v.pkmn_level,
            "levels_up": v.pkmn_after_levelups,
            "xp_gain": v.xp_gain,
            "total_xp": v.total_xp,
            "to_next": v.xp_to_next_level,
            "pct_tnl": v.percent_xp_to_next_level,
            "eps": v.experience_per_second,
            "level_gain": v.level_gain,
        })
    }
}
