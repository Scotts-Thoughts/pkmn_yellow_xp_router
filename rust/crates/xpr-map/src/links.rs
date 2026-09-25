//! Route identity -> map anchors (SPEC §3.7). Keys are the router's own
//! trainer / item names, compared through `sanitize_string` like every DB
//! lookup in the router.

use std::collections::HashMap;

use xpr_core::io_utils::sanitize_string;

use crate::model::{Anchor, MapId};
use crate::pack::MapPack;

#[derive(Debug, Default, Clone)]
pub struct Links {
    pub trainers: HashMap<String, Vec<Anchor>>,
    pub items: HashMap<String, Vec<Anchor>>,
    pub stats: serde_json::Value,
}

impl Links {
    pub fn trainer(&self, router_name: &str) -> &[Anchor] {
        self.trainers.get(&sanitize_string(router_name)).map(|v| v.as_slice()).unwrap_or(&[])
    }
    pub fn item(&self, router_name: &str) -> &[Anchor] {
        self.items.get(&sanitize_string(router_name)).map(|v| v.as_slice()).unwrap_or(&[])
    }
    pub fn has_trainer(&self, router_name: &str) -> bool {
        !self.trainer(router_name).is_empty()
    }
}

/// What a route event asks the map for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkQuery {
    Trainer(String),
    Item(String),
    Species(String),
}

impl MapPack {
    /// Anchors for a query. Species queries list every map whose encounter
    /// table contains the species (map-level anchors).
    pub fn resolve(&self, q: &LinkQuery) -> Vec<Anchor> {
        match q {
            LinkQuery::Trainer(n) => self.links.trainer(n).to_vec(),
            LinkQuery::Item(n) => self.links.item(n).to_vec(),
            LinkQuery::Species(n) => {
                let key = sanitize_string(n);
                let mut out: Vec<Anchor> = Vec::new();
                let mut maps: Vec<MapId> = self
                    .encounters
                    .iter()
                    .filter(|(_, t)| t.methods.iter().any(|m| m.slots.iter().any(|(_, s)| s.iter().any(|x| sanitize_string(&x.species) == key))))
                    .map(|(id, _)| *id)
                    .collect();
                maps.sort();
                for id in maps {
                    if let Some(m) = self.map(id) {
                        out.push(Anchor { map: id, x: (m.w * self.geom.steps_per_block() / 2) as u16, y: (m.h * self.geom.steps_per_block() / 2) as u16, precision: crate::model::Precision::Map, object: None, source: "encounters".into() });
                    }
                }
                out
            }
        }
    }
}
