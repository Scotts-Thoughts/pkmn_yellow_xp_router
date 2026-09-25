//! Finding trainers and items on the map from the toolbar's search box
//! (`docs/rust_port/design/world_map/GRAPHICS_TOOLS_PLAN.md` WP-E, gap
//! G10): the popup under the search box lists matching trainers and items
//! next to the maps; choosing one focuses its anchors through the focus
//! banner (pokemap's item finder, SPEC Phase 5 step 3).

use xpr_map::{Anchor, MapPack};

use crate::controller::MainController;

/// A search hit: what to show and where it is.
#[derive(Clone, Debug)]
pub struct Hit {
    pub label: String,
    pub anchors: Vec<Anchor>,
}

/// Trainers with a map position whose router name contains `query`
/// (case-insensitive), in the router's trainer order, capped at 25.
///
/// `pack.links.trainers` is keyed by `sanitize_string(name)`, which throws
/// away case and punctuation, so the search itself walks the trainer DB (in
/// its own, already-stable order) and asks the pack for each candidate's
/// anchors rather than searching the link table's keys directly — a linear
/// scan of the DB (at most ~850 records, Emerald) per query, cheap enough
/// that it is not worth caching for the MVP.
pub fn find_trainers(pack: &MapPack, ctrl: &MainController, query: &str) -> Vec<Hit> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let Some(gen) = ctrl.gen() else { return Vec::new() };
    let mut out = Vec::new();
    for t in gen.trainer_db().iter() {
        if out.len() >= 25 {
            break;
        }
        if !t.name.to_lowercase().contains(&q) {
            continue;
        }
        let anchors = pack.links.trainer(&t.name);
        if anchors.is_empty() {
            continue;
        }
        out.push(Hit { label: t.name.clone(), anchors: anchors.to_vec() });
    }
    out
}

/// Items with a pickup position whose router name contains `query`, same
/// rules as `find_trainers`.
pub fn find_items(pack: &MapPack, ctrl: &MainController, query: &str) -> Vec<Hit> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let Some(gen) = ctrl.gen() else { return Vec::new() };
    let mut out = Vec::new();
    for it in gen.item_db().iter() {
        if out.len() >= 25 {
            break;
        }
        if !it.name.to_lowercase().contains(&q) {
            continue;
        }
        let anchors = pack.links.item(&it.name);
        if anchors.is_empty() {
            continue;
        }
        out.push(Hit { label: it.name.clone(), anchors: anchors.to_vec() });
    }
    out
}
