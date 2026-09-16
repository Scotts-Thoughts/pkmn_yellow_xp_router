//! The display accessors of the tree classes (`pkmn_level`, `xp_gain`,
//! `get_tags`, `do_render`, ...), as router methods keyed by node id.

use xpr_core::consts;
use xpr_data::exp;

use crate::events::EventDefinition;
use crate::router::Router;
use crate::tree::{Node, NodeId, ObjKind};

/// The per-row values of the route list. `None` reproduces Python's `""`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RowValues {
    pub name: String,
    pub pkmn_after_levelups: String,
    pub pkmn_level: Option<i64>,
    pub xp_to_next_level: Option<i64>,
    pub percent_xp_to_next_level: String,
    pub xp_gain: Option<i64>,
    pub total_xp: Option<i64>,
    pub level_gain: String,
    pub experience_per_second: String,
    pub has_errors: bool,
    pub is_enabled: bool,
    pub is_major_fight: bool,
}

impl Router {
    /// `pkmn_level()` etc. for a node or item.
    pub fn row_values(&self, id: NodeId, gen: &xpr_data::GenData) -> RowValues {
        let kind = match self.obj_kind(id) {
            Some(k) => k,
            None => return RowValues::default(),
        };
        let enabled = self.is_enabled(id);
        let (name, has_errors, pkmn_after_levelups, eps) = match kind {
            ObjKind::Folder => {
                let f = self.folder(id).unwrap();
                (f.name.clone(), f.has_errors(), String::new(), String::new())
            }
            ObjKind::Group => {
                let g = self.group(id).unwrap();
                let eps = if enabled {
                    g.event_definition.experience_per_second(gen).unwrap_or_default()
                } else {
                    String::new()
                };
                (g.name.clone(), g.has_errors(), g.get_pkmn_after_levelups(), eps)
            }
            ObjKind::Item => {
                let i = self.item(id).unwrap();
                (i.name.clone(), i.has_errors(), String::new(), String::new())
            }
        };
        if kind == ObjKind::Folder || !enabled {
            return RowValues {
                name,
                pkmn_after_levelups,
                has_errors,
                is_enabled: enabled,
                is_major_fight: self.is_major_fight(id),
                ..Default::default()
            };
        }
        let init = self.init_state_of(id);
        let fin = self.final_state_of(id);
        let (Some(init), Some(fin)) = (init, fin) else {
            return RowValues {
                name,
                has_errors,
                is_enabled: enabled,
                ..Default::default()
            };
        };
        let xp_gain = fin.solo_pkmn.cur_xp - init.solo_pkmn.cur_xp;
        let level_gain = exp::calc_level_gain(
            init.solo_pkmn.cur_level,
            init.solo_pkmn.percent_xp_to_next_level,
            fin.solo_pkmn.cur_level,
            fin.solo_pkmn.percent_xp_to_next_level,
        );
        RowValues {
            name,
            pkmn_after_levelups,
            pkmn_level: Some(fin.solo_pkmn.cur_level),
            xp_to_next_level: Some(fin.solo_pkmn.xp_to_next_level),
            percent_xp_to_next_level: fin.solo_pkmn.percent_xp_to_next_level_str.clone(),
            xp_gain: if xp_gain != 0 { Some(xp_gain) } else { None },
            total_xp: Some(fin.solo_pkmn.cur_xp),
            level_gain,
            experience_per_second: eps,
            has_errors,
            is_enabled: enabled,
            is_major_fight: self.is_major_fight(id),
        }
    }

    /// `get_tags()` for any object.
    pub fn get_tags(&self, id: NodeId, gen: &xpr_data::GenData, color_major_battles: bool) -> Vec<&'static str> {
        match self.obj_kind(id) {
            Some(ObjKind::Item) => self.item(id).unwrap().get_tags(),
            Some(ObjKind::Group) => {
                let g = self.group(id).unwrap();
                if g.has_errors() {
                    return vec![consts::EVENT_TAG_ERRORS];
                }
                if let Some(ht) = g.event_definition.get_highlight_type() {
                    if (1..=9).contains(&ht) {
                        return vec![consts::ALL_HIGHLIGHT_LABELS[(ht - 1) as usize]];
                    }
                    return vec![consts::HIGHLIGHT_LABEL];
                }
                // a warning is not an error: the user's highlight still wins
                if g.has_warnings() {
                    return vec![consts::EVENT_TAG_WARNINGS];
                }
                if self.is_major_fight(id) {
                    if color_major_battles {
                        if let Some(td) = &g.event_definition.trainer_def {
                            if let Some(cat) = gen.get_fight_category(&td.trainer_name) {
                                if let Some(tag) = consts::fight_category_to_tag(cat) {
                                    return vec![tag];
                                }
                            }
                        }
                    }
                    return vec![consts::EVENT_TAG_IMPORTANT];
                }
                Vec::new()
            }
            Some(ObjKind::Folder) => {
                let f = self.folder(id).unwrap();
                if f.has_errors() {
                    return vec![consts::EVENT_TAG_ERRORS];
                }
                if f.is_expanded() {
                    return Vec::new();
                }
                for child in &f.children {
                    let child_tags = self.get_tags(*child, gen, color_major_battles);
                    for label in consts::ALL_HIGHLIGHT_LABELS {
                        if child_tags.contains(&label) {
                            return vec![label];
                        }
                    }
                    if child_tags.contains(&consts::HIGHLIGHT_LABEL) {
                        return vec![consts::HIGHLIGHT_LABEL];
                    }
                    for cat in consts::ALL_FIGHT_CATEGORY_TAGS {
                        if child_tags.contains(&cat) {
                            return vec![cat];
                        }
                    }
                    if child_tags.contains(&consts::EVENT_TAG_IMPORTANT) {
                        return vec![consts::EVENT_TAG_IMPORTANT];
                    }
                }
                Vec::new()
            }
            None => Vec::new(),
        }
    }

    /// `do_render(search, filter_types)` for a folder or group.
    pub fn do_render(&self, id: NodeId, gen: &xpr_data::GenData, search: Option<&str>, filter_types: Option<&[String]>) -> bool {
        match self.nodes.get(&id) {
            Some(Node::Folder(f)) => {
                if f.children.is_empty() && search.is_none() && filter_types.is_none() {
                    return true;
                }
                f.children.iter().any(|c| self.do_render(*c, gen, search, filter_types))
            }
            Some(Node::Group(g)) => {
                if let Some(types) = filter_types {
                    if g.has_errors() && types.iter().any(|t| t == consts::ERROR_SEARCH) {
                        return g.event_definition.do_render(gen, search, None).unwrap_or(false);
                    }
                    if self.is_major_fight(id) && types.iter().any(|t| t == consts::MAJOR_BATTLE_FILTER) {
                        return g.event_definition.do_render(gen, search, None).unwrap_or(false);
                    }
                }
                for lm in &g.level_up_learn_event_defs {
                    if EventDefinition::with_learn_move(lm.clone())
                        .do_render(gen, search, filter_types)
                        .unwrap_or(false)
                    {
                        return true;
                    }
                }
                g.event_definition.do_render(gen, search, filter_types).unwrap_or(false)
            }
            None => false,
        }
    }
}
