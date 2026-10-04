//! The Shift+Space trainer search across every router version (Solodex
//! `TrainerSpotlightSearch.tsx`; the overlay itself is `widgets::spotlight`).

use std::sync::{Arc, Mutex};

use xpr_data::Registry;

use super::data::{index_for, VersionIndex};
use crate::state::router_versions;
use crate::widgets::SpotlightRow;

struct Entry {
    version: String,
    /// position in the version list (the latest game has the highest)
    rank: usize,
    /// the shown name (a group's name for a group)
    name: String,
    name_lc: String,
    class: String,
    class_lc: String,
    /// the router trainer the row opens (a group's primary)
    trainer: String,
    max_level: i64,
    major: bool,
}

#[derive(Default)]
struct Cache {
    /// (version, index identity) of every version the entries were built from
    sig: Vec<(String, usize)>,
    entries: Arc<Vec<Entry>>,
    last: Option<(String, Option<String>, Vec<SpotlightRow>)>,
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);

fn identity(ix: &Arc<VersionIndex>) -> usize {
    Arc::as_ptr(ix) as usize
}

fn build_entries(indexes: &[(String, Arc<VersionIndex>)]) -> Vec<Entry> {
    let mut out = Vec::new();
    for (rank, (version, ix)) in indexes.iter().enumerate() {
        for row in &ix.rows {
            let t = &ix.trainers[row.primary];
            out.push(Entry {
                version: version.clone(),
                rank,
                name: row.name.clone(),
                name_lc: row.name.to_lowercase(),
                class: t.trainer.trainer_class.clone(),
                class_lc: t.trainer.trainer_class.to_lowercase(),
                trainer: t.trainer.name.clone(),
                max_level: t.max_level,
                major: t.major,
            });
        }
    }
    out
}

/// The matches of `query` among `entries` (Solodex's filter, dedupe and sort).
fn search<'a>(entries: &'a [Entry], query: &str, current: Option<&str>, versions: &[String]) -> Vec<&'a Entry> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        // no query: the major trainers of the current game (or the latest)
        let game = current.filter(|c| versions.iter().any(|v| v == c)).or_else(|| versions.last().map(|s| s.as_str()));
        let Some(game) = game else { return Vec::new() };
        let mut list: Vec<&Entry> = entries.iter().filter(|e| e.version == game && e.major).collect();
        list.sort_by_key(|e| e.max_level);
        return list;
    }
    // dedupe by name: the current game's entry, else the latest game's
    let mut order: Vec<String> = Vec::new();
    let mut by_name: std::collections::HashMap<&str, Vec<&Entry>> = std::collections::HashMap::new();
    for e in entries.iter().filter(|e| e.name_lc.contains(&q) || e.class_lc.contains(&q)) {
        let list = by_name.entry(e.name_lc.as_str()).or_default();
        if list.is_empty() {
            order.push(e.name_lc.clone());
        }
        list.push(e);
    }
    let mut deduped: Vec<&Entry> = Vec::new();
    for key in &order {
        let list = &by_name[key.as_str()];
        match list.iter().find(|e| Some(e.version.as_str()) == current) {
            Some(e) => deduped.push(e),
            None => deduped.push(list.iter().max_by_key(|e| e.rank).unwrap()),
        }
    }
    // major trainers first, then by name
    deduped.sort_by(|a, b| (!a.major).cmp(&!b.major).then_with(|| xpr_dex::text::locale_cmp(&a.name, &b.name)));
    deduped
}

/// Trainer search results across every version. Row keys are
/// `"<version>\u{1f}<trainer name>"`.
pub fn spotlight_rows(query: &str, registry: &Arc<Registry>, current_version: Option<&str>) -> Vec<SpotlightRow> {
    let versions = router_versions(registry);
    let mut indexes: Vec<(String, Arc<VersionIndex>)> = Vec::new();
    for v in &versions {
        if let Some(ix) = index_for(registry, v) {
            indexes.push((v.clone(), ix));
        }
    }
    let sig: Vec<(String, usize)> = indexes.iter().map(|(v, ix)| (v.clone(), identity(ix))).collect();
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = guard.get_or_insert_with(Cache::default);
    if cache.sig != sig {
        cache.sig = sig;
        cache.entries = Arc::new(build_entries(&indexes));
        cache.last = None;
    }
    if let Some((q, c, rows)) = &cache.last {
        if q == query && c.as_deref() == current_version {
            return rows.clone();
        }
    }
    let have: Vec<String> = indexes.iter().map(|(v, _)| v.clone()).collect();
    let rows: Vec<SpotlightRow> = search(&cache.entries, query, current_version, &have)
        .into_iter()
        .map(|e| SpotlightRow {
            key: format!("{}\u{1f}{}", e.version, e.trainer),
            label: e.name.clone(),
            detail: format!("{} \u{b7} Lv{} \u{b7} {}", e.class, e.max_level, e.version),
            sprite: None,
            color: None,
        })
        .collect();
    cache.last = Some((query.to_string(), current_version.map(|s| s.to_string()), rows.clone()));
    rows
}
