//! Builds the same JSON record as `docs/rust_port/golden/dump.py` from the
//! Rust engine so the two can be diffed.

use std::path::Path;

use serde_json::{json, Map, Value};

use xpr_core::consts;
use xpr_core::pyjson;
use xpr_engine::{NodeId, Router, RouteState};

pub const FILTER_TYPES_EXTRA: [&str; 1] = [consts::MAJOR_BATTLE_FILTER];
pub const SEARCH_STRINGS: [&str; 6] = ["", "a", "rare", "trainer", "potion", "z"];

fn opt_i(v: Option<i64>) -> Value {
    // Python returns "" for a missing value
    match v {
        Some(i) => Value::from(i),
        None => Value::String(String::new()),
    }
}

fn state(s: Option<&std::sync::Arc<RouteState>>) -> Value {
    let Some(s) = s else { return Value::Null };
    let m = &s.solo_pkmn;
    json!({
        "solo_mon": m.serialize(),
        "badges_verbose": s.badges.to_verbose_string(),
        "badges_short": s.badges.to_short_string(),
        "inventory": s.inventory.serialize(),
        "move_list": m.move_list.iter().map(|x| match x { Some(v) => Value::String(v.clone()), None => Value::Null }).collect::<Vec<_>>(),
        "xp_to_next_level": m.xp_to_next_level,
        "percent_xp_to_next_level": m.percent_xp_to_next_level,
        "percent_xp_to_next_level_str": m.percent_xp_to_next_level_str,
        "unrealized_stat_xp": m.unrealized_stat_xp.serialize(),
        "realized_stat_xp": m.realized_stat_xp.serialize(),
        "held_item": m.held_item,
        "ability": m.ability,
        "nature": m.nature.display_name(),
        "cur_stats": m.cur_stats.serialize(),
        "level": m.cur_level,
        "xp": m.cur_xp,
        "name": m.name,
    })
}

fn filter_types() -> Vec<String> {
    // the Python reference has no bag-reorder or EV-override event type, so
    // its records carry no such filter keys
    let mut v: Vec<String> = consts::ROUTE_EVENT_TYPES
        .iter()
        .filter(|s| **s != consts::TASK_REORDER_BAG && **s != consts::TASK_EV_OVERRIDE)
        .map(|s| s.to_string())
        .collect();
    v.extend(FILTER_TYPES_EXTRA.iter().map(|s| s.to_string()));
    v
}

fn render_maps(router: &Router, id: NodeId, gen: &xpr_data::GenData) -> (Value, Value) {
    let mut filters = Map::new();
    for ft in filter_types() {
        filters.insert(ft.clone(), Value::Bool(router.do_render(id, gen, None, Some(std::slice::from_ref(&ft)))));
    }
    let mut searches = Map::new();
    for s in SEARCH_STRINGS {
        searches.insert(s.to_string(), Value::Bool(router.do_render(id, gen, Some(s), None)));
    }
    (Value::Object(filters), Value::Object(searches))
}

fn group_record(router: &Router, id: NodeId, gen: &xpr_data::GenData, color_major: bool) -> Value {
    let g = router.group(id).unwrap();
    let rv = router.row_values(id, gen);
    let (filters, searches) = render_maps(router, id, gen);
    let items: Vec<Value> = g
        .event_items
        .iter()
        .map(|iid| {
            let it = router.item(*iid).unwrap();
            let irv = router.row_values(*iid, gen);
            json!({
                "name": it.name,
                "error_message": it.error_message,
                "is_enabled": irv.is_enabled,
                "tags": it.get_tags(),
                "pkmn_level": opt_i(irv.pkmn_level),
                "xp_to_next_level": opt_i(irv.xp_to_next_level),
                "percent_xp_to_next_level": irv.percent_xp_to_next_level,
                "xp_gain": opt_i(irv.xp_gain),
                "total_xp": opt_i(irv.total_xp),
                "level_gain": irv.level_gain,
                "definition": it.event_definition.serialize(),
                "shares_group_definition": it.shares_group_definition,
                "to_defeat_mon": it.to_defeat_mon.as_ref().map(|m| m.to_string(true)),
                "exp_split_num": it.exp_split_num,
                "pay_day_amount": it.pay_day_amount,
                "defeating_trainer": it.defeating_trainer,
                "final_state": state(it.final_state.as_ref()),
            })
        })
        .collect();
    json!({
        "kind": "group",
        "name": g.name,
        "label": g.event_definition.get_label(gen).unwrap_or_default(),
        "item_label": g.event_definition.get_item_label(gen).unwrap_or_default(),
        "event_type": g.event_definition.get_event_type(),
        "is_enabled": rv.is_enabled,
        "has_errors": g.has_errors(),
        "error_messages": g.error_messages,
        "tags": router.get_tags(id, gen, color_major),
        "highlight_type": g.event_definition.get_highlight_type(),
        "pkmn_after_levelups": g.get_pkmn_after_levelups(),
        "pkmn_level": opt_i(rv.pkmn_level),
        "xp_to_next_level": opt_i(rv.xp_to_next_level),
        "percent_xp_to_next_level": rv.percent_xp_to_next_level,
        "xp_gain": opt_i(rv.xp_gain),
        "total_xp": opt_i(rv.total_xp),
        "level_gain": rv.level_gain,
        "experience_per_second": rv.experience_per_second,
        "is_major_fight": rv.is_major_fight,
        "definition": g.event_definition.serialize(),
        "init_state": state(g.init_state.as_ref()),
        "final_state": state(g.final_state.as_ref()),
        "level_up_learn_event_defs": g.level_up_learn_event_defs.iter().map(|x| x.serialize()).collect::<Vec<_>>(),
        "do_render_filters": filters,
        "do_render_search": searches,
        "items": items,
    })
}

fn folder_record(router: &Router, id: NodeId, gen: &xpr_data::GenData, color_major: bool) -> Value {
    let f = router.folder(id).unwrap();
    let (filters, searches) = render_maps(router, id, gen);
    let children: Vec<Value> = f
        .children
        .iter()
        .map(|c| match router.node(*c) {
            Some(xpr_engine::Node::Folder(_)) => folder_record(router, *c, gen, color_major),
            Some(xpr_engine::Node::Group(_)) => group_record(router, *c, gen, color_major),
            None => Value::Null,
        })
        .collect();
    json!({
        "kind": "folder",
        "name": f.name,
        "notes": f.event_definition.notes,
        "expanded": f.expanded,
        "enabled": f.enabled,
        "is_enabled": router.is_enabled(id),
        "has_errors": f.has_errors(),
        "tags": router.get_tags(id, gen, color_major),
        "init_state": state(f.init_state.as_ref()),
        "final_state": state(f.final_state.as_ref()),
        "do_render_filters": filters,
        "do_render_search": searches,
        "children": children,
    })
}

/// Build the record for one route file. `color_major` mirrors the config
/// value the Python dump ran with.
pub fn build_record(router: &mut Router, source: &Path, color_major: bool) -> Value {
    let name = source
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut record = Map::new();
    record.insert("source".into(), Value::String(source.display().to_string()));
    record.insert("name".into(), Value::String(name));
    if let Err(e) = router.load(source, false) {
        record.insert("load_error".into(), Value::String(e));
        return Value::Object(record);
    }
    let gen = router.gen().unwrap().clone();
    let saved_text = String::from_utf8_lossy(&router.save_bytes().unwrap_or_default()).replace("\r\n", "\n");
    let mut level_up = Map::new();
    for (k, v) in &router.level_up_move_defs {
        level_up.insert(xpr_engine::router::py_key(k), v.serialize());
    }
    let mut defeated: Vec<String> = router.defeated_trainers.iter().cloned().collect();
    defeated.sort();
    let mut effective = router.get_effective_defeated_trainers();
    effective.sort();
    record.insert(
        "route".into(),
        json!({
            "version": router.pkmn_version,
            "generation": gen.get_generation(),
            "saved_text": saved_text,
            "notes_text": router.export_notes_text().unwrap_or_default(),
            "init_state": state(router.init_route_state.as_ref()),
            "final_state": state(router.get_final_state()),
            "defeated_trainers": defeated,
            "effective_defeated_trainers": effective,
            "folder_names": router.folder_lookup.keys().cloned().collect::<Vec<_>>(),
            "level_up_move_defs": Value::Object(level_up),
            "test_moves": router.test_moves,
        }),
    );
    let root = router.root_id;
    record.insert("tree".into(), folder_record(router, root, &gen, color_major));
    Value::Object(record)
}

/// Sort keys recursively so that Python's sorted `defeated_trainers` and
/// insertion-ordered dicts compare structurally.
pub fn canonical(v: &Value) -> Value {
    match v {
        Value::Object(o) => {
            let mut m = Map::new();
            let mut keys: Vec<&String> = o.keys().collect();
            keys.sort();
            for k in keys {
                m.insert(k.clone(), canonical(&o[k]));
            }
            Value::Object(m)
        }
        Value::Array(a) => Value::Array(a.iter().map(canonical).collect()),
        other => other.clone(),
    }
}

/// Human-readable list of differences between two records (path -> values).
pub fn diff(path: &str, a: &Value, b: &Value, out: &mut Vec<String>, limit: usize) {
    if out.len() >= limit {
        return;
    }
    match (a, b) {
        (Value::Object(oa), Value::Object(ob)) => {
            let mut keys: Vec<&String> = oa.keys().chain(ob.keys()).collect();
            keys.sort();
            keys.dedup();
            for k in keys {
                let p = format!("{}.{}", path, k);
                match (oa.get(k), ob.get(k)) {
                    (Some(x), Some(y)) => diff(&p, x, y, out, limit),
                    (Some(_), None) => out.push(format!("{}: missing in python", p)),
                    (None, Some(_)) => out.push(format!("{}: missing in rust", p)),
                    (None, None) => {}
                }
            }
        }
        (Value::Array(aa), Value::Array(ab)) => {
            if aa.len() != ab.len() {
                out.push(format!("{}: length {} (rust) vs {} (python)", path, aa.len(), ab.len()));
            }
            for (i, (x, y)) in aa.iter().zip(ab.iter()).enumerate() {
                diff(&format!("{}[{}]", path, i), x, y, out, limit);
            }
        }
        (Value::String(sa), Value::String(sb)) if sa != sb => {
            if sa.len() > 200 || sb.len() > 200 {
                // report the first differing line for long texts
                let la: Vec<&str> = sa.lines().collect();
                let lb: Vec<&str> = sb.lines().collect();
                for (i, (x, y)) in la.iter().zip(lb.iter()).enumerate() {
                    if x != y {
                        out.push(format!("{}: line {} differs:\n    rust:   {}\n    python: {}", path, i + 1, x, y));
                        return;
                    }
                }
                out.push(format!("{}: length {} vs {} lines", path, la.len(), lb.len()));
            } else {
                out.push(format!("{}: {:?} (rust) vs {:?} (python)", path, sa, sb));
            }
        }
        (Value::Number(na), Value::Number(nb)) => {
            let fa = na.as_f64().unwrap_or(0.0);
            let fb = nb.as_f64().unwrap_or(0.0);
            let same = na == nb || (fa - fb).abs() <= 1e-9 * fa.abs().max(fb.abs()).max(1.0);
            if !same {
                out.push(format!("{}: {} (rust) vs {} (python)", path, na, nb));
            }
        }
        _ => {
            if a != b {
                out.push(format!(
                    "{}: {} (rust) vs {} (python)",
                    path,
                    pyjson::dumps_compact(a),
                    pyjson::dumps_compact(b)
                ));
            }
        }
    }
}
