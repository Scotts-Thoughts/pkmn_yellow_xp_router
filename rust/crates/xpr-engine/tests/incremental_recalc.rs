//! The incremental recalculation (`Router::recalc_from`) must leave the
//! route exactly as a full `recalc` would. Every structural operation the
//! app performs is applied to the test routes (and to any extra routes named
//! by `XPR_ROUTE_FILES`, `;`-separated), fingerprinting the whole route
//! before and after a full recalculation.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};

use xpr_data::Registry;
use xpr_engine::{EventDefinition, InsertSpec, Node, NodeId, Router, VitaminEventDefinition};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn registry() -> Arc<Registry> {
    let root = repo_root();
    let custom = std::env::var_os("XPR_CUSTOM_GENS_DIR").map(PathBuf::from).unwrap_or_default();
    let reg = Arc::new(Registry::new(root.join("raw_pkmn_data"), custom));
    let _ = reg.reload_all_custom_gens();
    reg
}

fn route_files() -> Vec<PathBuf> {
    let root = repo_root();
    let mut files: Vec<PathBuf> = ["yellow-pinsir-lv10brock.json", "c-porygon-1-.json", "f-ditto-tests.json", "platinum_chimchar.json"]
        .iter()
        .map(|n| root.join("tests/test_data").join(n))
        .collect();
    if let Ok(extra) = std::env::var("XPR_ROUTE_FILES") {
        files.extend(extra.split(';').filter(|s| !s.trim().is_empty()).map(PathBuf::from));
    }
    files
}

fn state_json(s: Option<&Arc<xpr_engine::RouteState>>) -> Value {
    match s {
        None => Value::Null,
        Some(s) => json!({
            "state": s.serialize(),
            "moves": s.solo_pkmn.move_list,
            "tnl": s.solo_pkmn.xp_to_next_level,
            "pct": s.solo_pkmn.percent_xp_to_next_level_str,
            "unrealized": s.solo_pkmn.unrealized_stat_xp.serialize(),
            "badges_short": s.badges.to_short_string(),
        }),
    }
}

fn node_json(router: &Router, id: NodeId) -> Value {
    match router.node(id) {
        Some(Node::Folder(f)) => json!({
            "folder": f.name,
            "enabled": f.enabled,
            "expanded": f.expanded,
            "is_enabled": router.is_enabled(id),
            "child_errors": f.child_errors,
            "notes": f.event_definition.notes,
            "init": state_json(f.init_state.as_ref()),
            "final": state_json(f.final_state.as_ref()),
            "children": f.children.iter().map(|c| node_json(router, *c)).collect::<Vec<_>>(),
        }),
        Some(Node::Group(g)) => json!({
            "group": g.name,
            "enabled": g.enabled,
            "is_enabled": router.is_enabled(id),
            "errors": g.error_messages,
            "definition": g.event_definition.serialize(),
            "after_levelups": g.pkmn_after_levelups,
            "level_up_defs": g.level_up_learn_event_defs.iter().map(|d| d.serialize()).collect::<Vec<_>>(),
            "init": state_json(g.init_state.as_ref()),
            "final": state_json(g.final_state.as_ref()),
            "items": g.event_items.iter().map(|i| {
                let it = router.item(*i).expect("item exists");
                json!({
                    "name": it.name,
                    "enabled": it.enabled,
                    "is_enabled": router.is_enabled(*i),
                    "error": it.error_message,
                    "definition": it.event_definition.serialize(),
                    "shares": it.shares_group_definition,
                    "mon": it.to_defeat_mon.as_ref().map(|m| m.to_string(true)),
                    "split": it.exp_split_num,
                    "pay_day": it.pay_day_amount,
                    "defeating": it.defeating_trainer,
                    "init": state_json(it.init_state.as_ref()),
                    "final": state_json(it.final_state.as_ref()),
                })
            }).collect::<Vec<_>>(),
        }),
        None => Value::Null,
    }
}

fn fingerprint(router: &Router) -> Value {
    let mut item_count = 0usize;
    for g in router.all_groups() {
        item_count += router.group(g).unwrap().event_items.len();
    }
    json!({
        "tree": node_json(router, router.root_id),
        "level_up_move_defs": router.level_up_move_defs.iter().map(|(k, v)| json!([k.0, k.1, k.2, v.serialize()])).collect::<Vec<_>>(),
        "defeated": router.defeated_trainers.iter().cloned().collect::<Vec<_>>(),
        "final": state_json(router.get_final_state()),
        // every item in the arena belongs to a live group (no orphans)
        "orphan_free": router.item_count() == item_count,
        "item_count": item_count,
    })
}

/// The route was just mutated with an incremental recalc: a full recalc
/// must change nothing.
fn check(router: &mut Router, what: &str) {
    let incremental = fingerprint(router);
    router.recalc().expect("full recalc");
    let full = fingerprint(router);
    if incremental != full {
        let a = serde_json::to_string_pretty(&incremental).unwrap();
        let b = serde_json::to_string_pretty(&full).unwrap();
        for (line, (la, lb)) in a.lines().zip(b.lines()).enumerate() {
            if la != lb {
                panic!("{}: incremental recalc differs from full recalc at line {}:\n  incremental: {}\n  full:        {}", what, line + 1, la, lb);
            }
        }
        panic!("{}: incremental recalc differs from full recalc (lengths {} vs {})", what, a.len(), b.len());
    }
}

fn trainer_groups(router: &Router) -> Vec<NodeId> {
    router.all_groups().into_iter().filter(|g| router.group(*g).unwrap().event_definition.trainer_def.is_some()).collect()
}

fn before(id: NodeId) -> InsertSpec {
    InsertSpec { insert_before: Some(id), ..Default::default() }
}

fn after(id: NodeId) -> InsertSpec {
    InsertSpec { insert_after: Some(id), ..Default::default() }
}

#[test]
fn incremental_recalc_matches_full_recalc() {
    let reg = registry();
    let mut checked = 0;
    for path in route_files() {
        let mut router = Router::new(reg.clone());
        if let Err(e) = router.load(&path, false) {
            eprintln!("skipping {}: {}", path.display(), e);
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let trainers = trainer_groups(&router);
        assert!(!trainers.is_empty(), "{} has trainer fights", name);
        // spread the probes over the route: first, a few in the middle, last
        let n = trainers.len();
        let probes: Vec<NodeId> = [0, n / 4, n / 2, (3 * n) / 4, n - 1].iter().map(|i| trainers[*i]).collect();
        for gid in probes {
            let label = format!("{} / {}", name, router.group(gid).unwrap().name);
            // pre-fight candies: insert, raise, lower, remove
            let candy = router.add_event_object(Some(EventDefinition::with_rare_candy(1)), None, before(gid), true, None, None).expect("insert candy");
            check(&mut router, &format!("{}: insert candy", label));
            for amount in [3, 7, 2] {
                router.replace_event_group(candy, EventDefinition::with_rare_candy(amount)).expect("update candy");
                check(&mut router, &format!("{}: candy x{}", label, amount));
            }
            // a vitamin after the candies
            let vit = router
                .add_event_object(Some(EventDefinition::with_vitamin(VitaminEventDefinition::new("Protein", 2))), None, after(candy), true, None, None)
                .expect("insert vitamin");
            check(&mut router, &format!("{}: insert vitamin", label));
            // disable / re-enable the fight through its definition
            let mut disabled = router.group(gid).unwrap().event_definition.clone();
            disabled.enabled = Some(false);
            router.replace_event_group(gid, disabled).expect("disable");
            check(&mut router, &format!("{}: disable fight", label));
            let mut enabled = router.group(gid).unwrap().event_definition.clone();
            enabled.enabled = Some(true);
            router.replace_event_group(gid, enabled).expect("enable");
            check(&mut router, &format!("{}: enable fight", label));
            // notes on the fight's folder (no state change: the suffix is skipped)
            let folder = router.parent_of(gid).unwrap();
            let mut fdef = router.folder(folder).unwrap().event_definition.clone();
            fdef.notes = format!("{} probe", fdef.notes);
            router.replace_event_group(folder, fdef).expect("folder notes");
            check(&mut router, &format!("{}: folder notes", label));
            // removals (a single id takes the incremental path)
            router.batch_remove_events(&[vit]).expect("remove vitamin");
            check(&mut router, &format!("{}: remove vitamin", label));
            router.batch_remove_events(&[candy]).expect("remove candy");
            check(&mut router, &format!("{}: remove candy", label));
            checked += 1;
        }
        // a new folder with an event inside it, then removing the folder
        let first = trainers[0];
        let fid = router.add_event_object(None, Some("incremental probe folder"), after(first), true, Some(true), Some(true)).expect("folder");
        check(&mut router, &format!("{}: new folder", name));
        let inner = router
            .add_event_object(Some(EventDefinition::with_rare_candy(4)), None, InsertSpec { dest_folder_name: Some("incremental probe folder".to_string()), ..Default::default() }, true, None, None)
            .expect("candy in folder");
        check(&mut router, &format!("{}: candy in new folder", name));
        router.replace_event_group(inner, EventDefinition::with_rare_candy(1)).expect("update");
        check(&mut router, &format!("{}: update candy in new folder", name));
        router.batch_remove_events(&[fid]).expect("remove folder");
        check(&mut router, &format!("{}: remove folder", name));
    }
    assert!(checked > 0, "at least one route was exercised");
}
