//! What the map knows about the open route: which trainers are already
//! fought, which items already picked up, and where the selected event is.

use std::collections::HashSet;

use xpr_core::io_utils::sanitize_string;
use xpr_engine::NodeId;
use xpr_map::{Anchor, LinkQuery, MapPack};

use crate::controller::MainController;

#[derive(Default, Clone, Debug)]
pub struct RouteMapState {
    /// sanitized trainer names fought anywhere in the route
    pub defeated: HashSet<String>,
    /// sanitized item names acquired without money (pickups)
    pub acquired: HashSet<String>,
    /// the selected event's link, if any
    pub selected_query: Option<LinkQuery>,
    pub selected_id: Option<NodeId>,
    pub selected_anchors: Vec<Anchor>,
}

impl RouteMapState {
    pub fn is_defeated(&self, trainer: &str) -> bool {
        self.defeated.contains(&sanitize_string(trainer))
    }
    pub fn is_acquired(&self, item: &str) -> bool {
        self.acquired.contains(&sanitize_string(item))
    }

    /// Rebuild from the controller (cheap: one walk over the groups).
    pub fn sync(&mut self, ctrl: &MainController, pack: Option<&MapPack>) {
        self.defeated = ctrl.router.defeated_trainers.iter().map(|n| sanitize_string(n)).collect();
        let mut acquired = HashSet::new();
        for g in ctrl.router.all_groups() {
            if let Some(grp) = ctrl.router.group(g) {
                if let Some(item) = &grp.event_definition.item_event_def {
                    if item.is_acquire && !item.with_money {
                        acquired.insert(sanitize_string(&item.item_name));
                    }
                }
            }
        }
        self.acquired = acquired;
        self.selected_id = ctrl.get_single_selected_event_id(true);
        self.selected_query = self.selected_id.and_then(|id| query_for_event(ctrl, id));
        self.selected_anchors = match (&self.selected_query, pack) {
            (Some(q), Some(p)) => p.resolve(q),
            _ => Vec::new(),
        };
    }
}

/// The map query for a route node (trainer fight, item pickup, wild encounter).
pub fn query_for_event(ctrl: &MainController, id: NodeId) -> Option<LinkQuery> {
    let def = match ctrl.router.obj_kind(id)? {
        xpr_engine::ObjKind::Group => ctrl.router.group(id)?.event_definition.clone(),
        xpr_engine::ObjKind::Item => {
            let it = ctrl.router.item(id)?;
            ctrl.router.group(it.parent).map(|g| g.event_definition.clone()).unwrap_or_else(|| it.event_definition.clone())
        }
        xpr_engine::ObjKind::Folder => return None,
    };
    if let Some(t) = &def.trainer_def {
        return Some(LinkQuery::Trainer(t.trainer_name.clone()));
    }
    if let Some(item) = &def.item_event_def {
        if item.is_acquire && !item.with_money {
            return Some(LinkQuery::Item(item.item_name.clone()));
        }
        return None;
    }
    if let Some(w) = &def.wild_pkmn_info {
        return Some(LinkQuery::Species(w.name.clone()));
    }
    None
}
