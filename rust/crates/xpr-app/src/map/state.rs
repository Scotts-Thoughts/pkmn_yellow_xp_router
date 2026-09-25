//! What the map knows about the open route: which trainers are already
//! fought, which items already picked up, where the selected event is, and
//! the route-path overlay's data for the trip currently in view.
//!
//! The route-path part (WP-E, gap G9) follows
//! `docs/rust_port/design/world_map/SPEC.md` decision D15 and §13 "Trip
//! path overlay" (decided 2026-09-25, superseding the plan's first,
//! whole-route sketch): the overlay draws **one folder ("trip") at a
//! time**, never the whole route, because nothing in a route file says how
//! the runner got from one trip to the next (Fly / Dig / Teleport / Escape
//! Rope / boats / trains are not events, §13.1) — a line spanning two trips
//! would be wrong at almost every join.

use std::collections::HashSet;

use xpr_core::io_utils::sanitize_string;
use xpr_engine::{NodeId, ObjKind};
use xpr_map::{Anchor, LinkQuery, MapPack, Precision};

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
    /// The last selected event id the camera followed (`keynav::
    /// on_route_synced`, gap G12), so follow only acts when the selection
    /// actually changed.
    pub last_followed: Option<NodeId>,
    /// The route-path overlay's data (SPEC §13.2 "Data flow"): the folder
    /// selected in the route list, or the nearest enclosing folder of the
    /// selected event. `None` when nothing is selected, the selection is
    /// directly under the root (§13 "Events directly under the root belong
    /// to no trip"), or the resolved folder has no anchored events.
    pub trip: Option<TripPath>,
    /// Set whenever a non-root folder was resolved, whether or not it
    /// produced a `trip` — lets `route_path::draw` tell "no folder
    /// selected" apart from "this folder has nothing to show", which both
    /// leave `trip` at `None`.
    pub trip_folder_name: Option<String>,
}

/// One folder's route-path: its anchored trainer fights / item pickups, in
/// route order, consecutive same-step events collapsed into one node
/// (SPEC §13.2 "Anchors of a trip" / "Nodes").
#[derive(Clone, Debug)]
pub struct TripPath {
    pub folder: NodeId,
    pub name: String,
    pub nodes: Vec<TripNode>,
}

/// One disc of the path: one or more route events collapsed onto the same
/// step, labelled with its 1-based order within the trip (`first == last`)
/// or the range it spans (`first < last`, drawn "3–5").
#[derive(Clone, Debug)]
pub struct TripNode {
    pub ids: Vec<NodeId>,
    pub first: usize,
    pub last: usize,
    pub anchor: Anchor,
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

        // the route-path overlay's trip (D15, §13): one folder at a time
        self.trip = None;
        self.trip_folder_name = None;
        if let (Some(sel), Some(p)) = (self.selected_id, pack) {
            if let Some(folder_id) = enclosing_folder(ctrl, sel) {
                // a folder id equal to the route's root means the selection
                // sits directly under the root: no trip, not even an empty one
                if folder_id != ctrl.router.root_id {
                    self.trip_folder_name = ctrl.router.folder(folder_id).map(|f| f.name.clone());
                    self.trip = build_trip(ctrl, p, folder_id);
                }
            }
        }
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

/// The nearest folder containing `id`: `id` itself when it is already a
/// folder (the route list can select a folder directly), else the first
/// folder found walking up the parent chain.
fn enclosing_folder(ctrl: &MainController, id: NodeId) -> Option<NodeId> {
    let mut cur = id;
    loop {
        if ctrl.router.obj_kind(cur) == Some(ObjKind::Folder) {
            return Some(cur);
        }
        cur = ctrl.router.parent_of(cur)?;
    }
}

/// Depth-first group ids under `folder_id`, sub-folders included, in the
/// same relative order `Router::all_groups` would list them (it is a
/// depth-first walk from the root; restricting the same walk to one
/// subtree preserves the order within it).
fn groups_in_folder(ctrl: &MainController, folder_id: NodeId, out: &mut Vec<NodeId>) {
    for child in ctrl.router.children_of(folder_id) {
        match ctrl.router.obj_kind(child) {
            Some(ObjKind::Folder) => groups_in_folder(ctrl, child, out),
            Some(ObjKind::Group) => out.push(child),
            _ => {}
        }
    }
}

/// Build one folder's trip (SPEC §13.2): enabled trainer fights / no-money
/// pickups whose first `Object`/`Script` anchor is kept, in folder order;
/// map-level anchors (wild encounters, trainers the game code cannot
/// place) and disabled events are skipped; consecutive events that land on
/// the same step collapse into one `TripNode`. `None` when the folder has
/// no such event.
fn build_trip(ctrl: &MainController, pack: &MapPack, folder_id: NodeId) -> Option<TripPath> {
    let name = ctrl.router.folder(folder_id)?.name.clone();
    let mut group_ids = Vec::new();
    groups_in_folder(ctrl, folder_id, &mut group_ids);

    let mut entries: Vec<(NodeId, Anchor)> = Vec::new();
    for id in group_ids {
        if !ctrl.router.is_enabled(id) {
            continue;
        }
        let Some(query) = query_for_event(ctrl, id) else { continue };
        // trainer fights and no-money pickups only; wild encounters resolve
        // to map-level (encounter-table) anchors, excluded below anyway
        if !matches!(query, LinkQuery::Trainer(_) | LinkQuery::Item(_)) {
            continue;
        }
        let Some(anchor) = pack.resolve(&query).into_iter().find(|a| matches!(a.precision, Precision::Object | Precision::Script)) else {
            continue;
        };
        entries.push((id, anchor));
    }
    if entries.is_empty() {
        return None;
    }

    let mut nodes: Vec<TripNode> = Vec::new();
    for (i, (id, anchor)) in entries.into_iter().enumerate() {
        let order = i + 1; // 1-based order within the trip
        if let Some(last) = nodes.last_mut() {
            if last.anchor.map == anchor.map && last.anchor.x == anchor.x && last.anchor.y == anchor.y {
                last.ids.push(id);
                last.last = order;
                continue;
            }
        }
        nodes.push(TripNode { ids: vec![id], first: order, last: order, anchor });
    }
    Some(TripPath { folder: folder_id, name, nodes })
}
