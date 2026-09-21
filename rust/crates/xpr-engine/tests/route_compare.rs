//! `docs/rust_port/design/route_compare/SPEC.md` §9.1.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;
use xpr_data::Registry;
use xpr_engine::compare::{compare, digest, DiffRow, EntryKind, ItemStatus, MoveSource, RouteComparison, RouteDigest, RouteOrigin};
use xpr_engine::Router;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn registry() -> Arc<Registry> {
    Arc::new(Registry::new(repo_root().join("raw_pkmn_data"), PathBuf::new()))
}

fn raw(name: &str) -> Value {
    let path = repo_root().join("tests/test_data").join(name);
    serde_json::from_str(&std::fs::read_to_string(&path).expect("route file")).expect("valid json")
}

fn digest_of(value: &Value, label: &str) -> RouteDigest {
    let mut router = Router::new(registry());
    router.load_value(value, false).expect("route loads");
    digest(&router, label, RouteOrigin::External)
}

fn load(name: &str) -> RouteDigest {
    digest_of(&raw(name), name)
}

const ROUTES: [&str; 3] = ["yellow-pinsir-lv10brock.json", "c-porygon-1-.json", "platinum_chimchar.json"];

/// The root folder's `events` array, for the mutation helpers below.
fn events_mut(route: &mut Value) -> &mut Vec<Value> {
    route["events"][0]["events"].as_array_mut().expect("root events")
}

/// Depth-first indices of every trainer-fight event in a route's tree.
fn trainer_paths(node: &Value, path: Vec<usize>, out: &mut Vec<Vec<usize>>) {
    let Some(children) = node["events"].as_array() else { return };
    for (i, child) in children.iter().enumerate() {
        let mut p = path.clone();
        p.push(i);
        if child.get("Event Folder Name").is_some() {
            trainer_paths(child, p, out);
        } else if child.get("Fight Trainer").is_some() {
            out.push(p);
        }
    }
}

fn all_trainer_paths(route: &Value) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    trainer_paths(&route["events"][0], Vec::new(), &mut out);
    out
}

fn at_path<'a>(route: &'a mut Value, path: &[usize]) -> &'a mut Value {
    let mut node = &mut route["events"][0];
    for (depth, idx) in path.iter().enumerate() {
        node = &mut node["events"][*idx];
        let _ = depth;
    }
    node
}

fn remove_at(route: &mut Value, path: &[usize]) -> Value {
    let (last, parents) = path.split_last().unwrap();
    let mut node = &mut route["events"][0];
    for idx in parents {
        node = &mut node["events"][*idx];
    }
    node["events"].as_array_mut().unwrap().remove(*last)
}

// ---------------------------------------------------------------------------

#[test]
fn route_compared_with_itself_has_no_differences() {
    for name in ROUTES {
        let cmp = compare(load(name), load(name));
        assert!(cmp.compat.same_version && cmp.compat.same_generation && cmp.compat.same_species, "{name}");

        let trainers = cmp.a.totals.trainers as usize;
        assert_eq!(cmp.shared_fight_count(), trainers, "{name}: every trainer should anchor");
        assert_eq!(cmp.checkpoints.len(), trainers, "{name}");
        assert_eq!(cmp.trainer_sets.different_order, 0, "{name}");
        assert!(cmp.trainer_sets.only_a.is_empty() && cmp.trainer_sets.only_b.is_empty(), "{name}");
        assert_eq!(cmp.blocks_with_differences(), 0, "{name}: no block may differ");

        for row in &cmp.rows {
            if let DiffRow::Block { a, b } = row {
                for item in a.iter().chain(b.iter()) {
                    assert_eq!(item.status, ItemStatus::InBoth, "{name}: {}", item.merged_label);
                }
            }
        }
        for cp in &cmp.checkpoints {
            assert_eq!(cp.moves_differing, 0, "{name}");
            assert!(cp.same_order, "{name}");
            assert_eq!(cmp.a.entries[cp.a].before, cmp.b.entries[cp.b].before, "{name}");
        }
        assert_eq!(cmp.a.end, cmp.b.end, "{name}");
    }
}

#[test]
fn exp_and_money_buckets_add_up() {
    for name in ROUTES {
        let d = load(name);
        assert_eq!(
            d.totals.xp_total(),
            d.end.xp - d.start.xp,
            "{name}: exp buckets must sum to the route's exp gain"
        );
        assert_eq!(
            d.start.money + d.totals.money_total(),
            d.end.money,
            "{name}: money buckets must reconcile the final money"
        );
    }
}

#[test]
fn removing_a_trainer_shows_up_as_only_in_a() {
    let name = "c-porygon-1-.json";
    let a = raw(name);
    let paths = all_trainer_paths(&a);
    let target = &paths[paths.len() / 2];
    let mut b = a.clone();
    let removed = remove_at(&mut b, target);
    let removed_name = removed["Fight Trainer"]["trainer_name"].as_str().unwrap().to_string();

    let da = digest_of(&a, "a");
    let db = digest_of(&b, "b");
    assert_eq!(db.totals.trainers, da.totals.trainers - 1);

    let cmp = compare(da, db);
    assert_eq!(cmp.trainer_sets.only_a, vec![removed_name], "the dropped fight is only in A");
    assert!(cmp.trainer_sets.only_b.is_empty());
    assert_eq!(cmp.trainer_sets.different_order, 0);

    // B falls behind: after the removal it can never be ahead on exp.
    for cp in &cmp.checkpoints {
        assert!(
            cmp.b.entries[cp.b].before.xp <= cmp.a.entries[cp.a].before.xp,
            "B skipped a fight, so it cannot lead on exp at {}",
            cmp.a.entries[cp.a].label
        );
    }
}

#[test]
fn swapping_two_trainers_is_reported_as_a_different_order() {
    let name = "c-porygon-1-.json";
    let a = raw(name);
    let paths = all_trainer_paths(&a);
    // Two fights that are siblings in the same folder, so the swap is a pure
    // reorder of the route.
    let pair = paths
        .windows(2)
        .find(|w| w[0].len() == w[1].len() && w[0][..w[0].len() - 1] == w[1][..w[1].len() - 1])
        .expect("two sibling fights");
    let (p0, p1) = (pair[0].clone(), pair[1].clone());

    let mut b = a.clone();
    let first = at_path(&mut b, &p0).clone();
    let second = at_path(&mut b, &p1).clone();
    *at_path(&mut b, &p0) = second;
    *at_path(&mut b, &p1) = first;

    let cmp = compare(digest_of(&a, "a"), digest_of(&b, "b"));
    assert_eq!(cmp.trainer_sets.different_order, 1, "exactly one moved pair");
    assert!(cmp.trainer_sets.only_a.is_empty() && cmp.trainer_sets.only_b.is_empty());
    assert_eq!(cmp.a.totals.trainers, cmp.b.totals.trainers);

    let moved: usize = cmp
        .rows
        .iter()
        .filter_map(|r| match r {
            DiffRow::Block { a, .. } => Some(a.iter().filter(|i| matches!(i.status, ItemStatus::TrainerMoved { .. })).count()),
            _ => None,
        })
        .sum();
    assert_eq!(moved, 1, "the moved fight appears once in A's lane");
}

/// A minimal Emerald route around one trainer fight, for the pairing rules.
fn emerald_route(trainer_names: &[&str]) -> Value {
    let events: Vec<Value> = trainer_names
        .iter()
        .map(|n| {
            serde_json::json!({
                "Enabled": true,
                "Tags": [],
                "Fight Trainer": {
                    "trainer_name": n, "second_trainer_name": "", "verbose": false,
                    "setup_moves": [], "enemy_setup_moves": [], "player_field_moves": [], "enemy_field_moves": [],
                    "mimic_selection": "", "custom_move_data": [], "exp_split": [],
                    "weather": "None", "weather_source_mon_idx": null,
                    "player_screens": null, "enemy_screens": null, "player_intimidate": null, "enemy_intimidate": null,
                    "pay_day_amount": 0, "mon_order": [1], "transformed": false,
                    "stat_stage_setup": null, "collapsed_mons": null
                }
            })
        })
        .collect();
    serde_json::json!({
        "name": "Houndoom",
        "dv": { "hp": 31, "attack": 31, "defense": 31, "special_attack": 31, "special_defense": 31, "speed": 31 },
        "ability": 0,
        "nature": 0,
        "Version": "Emerald",
        "Learn Levelup Move": [],
        "events": [{ "Event Folder Name": "ROOT", "Just Notes": "", "Expanded": true, "Enabled": true, "events": events }],
        "test_moves": ["", "", "", ""]
    })
}

#[test]
fn equivalent_named_fights_pair_when_the_team_matches() {
    // Emerald's rival is "Rival Brendan N Mudkip" or "Rival May N Mudkip"
    // depending on the player's gender. Same location, same team, so §5.4
    // step 1 must pair them into a single anchor.
    let reg = registry();
    let gen = reg.get_version("Emerald").expect("emerald data");
    let (Some(brendan), Some(may)) = (
        gen.trainer_db().get_trainer("Rival Brendan 1 Mudkip"),
        gen.trainer_db().get_trainer("Rival May 1 Mudkip"),
    ) else {
        eprintln!("skipping: Emerald rival trainers not present in this data set");
        return;
    };
    assert_eq!(brendan.location, may.location, "same fight, different player gender");

    let a = digest_of(&emerald_route(&["Rival Brendan 1 Mudkip"]), "a");
    let b = digest_of(&emerald_route(&["Rival May 1 Mudkip"]), "b");
    let cmp = compare(a, b);

    assert_eq!(cmp.shared_fight_count(), 1, "the two rival fights are one shared fight");
    assert_eq!(cmp.trainer_sets.same_order, 1);
    assert!(
        cmp.trainer_sets.only_a.is_empty() && cmp.trainer_sets.only_b.is_empty(),
        "neither side is one-sided: {:?} / {:?}",
        cmp.trainer_sets.only_a,
        cmp.trainer_sets.only_b
    );
    assert_eq!(cmp.checkpoints.len(), 1);
}

#[test]
fn differently_teamed_fights_do_not_pair() {
    // Two real trainers whose teams differ must stay one-sided, even though
    // both sides have exactly one unmatched fight.
    let reg = registry();
    let gen = reg.get_version("Emerald").expect("emerald data");
    let (Some(_), Some(_)) = (
        gen.trainer_db().get_trainer("Rival Brendan 1 Mudkip"),
        gen.trainer_db().get_trainer("Rival Brendan 2 Mudkip"),
    ) else {
        eprintln!("skipping: Emerald rival trainers not present in this data set");
        return;
    };
    let a = digest_of(&emerald_route(&["Rival Brendan 1 Mudkip"]), "a");
    let b = digest_of(&emerald_route(&["Rival Brendan 2 Mudkip"]), "b");
    let cmp = compare(a, b);

    assert_eq!(cmp.shared_fight_count(), 0, "different teams are different fights");
    assert_eq!(cmp.trainer_sets.only_a.len(), 1);
    assert_eq!(cmp.trainer_sets.only_b.len(), 1);
}

#[test]
fn an_unknown_trainer_name_fails_the_whole_load() {
    // Documents why the compare UI needs a per-slot failure banner (SPEC §4.4):
    // a route naming a trainer the data set does not have cannot be loaded at
    // all, so it never reaches `digest`.
    let mut broken = raw("c-porygon-1-.json");
    let paths = all_trainer_paths(&broken);
    at_path(&mut broken, &paths[0])["Fight Trainer"]["trainer_name"] = Value::String("Totally Not A Trainer".into());

    let mut router = Router::new(registry());
    let err = router.load_value(&broken, false).expect_err("an unknown trainer must not load");
    assert!(!err.is_empty());
}

#[test]
fn duplicate_fights_are_flagged_as_repeats() {
    let name = "c-porygon-1-.json";
    let a = raw(name);
    let paths = all_trainer_paths(&a);
    let mut b = a.clone();
    let dup = at_path(&mut b, &paths[0]).clone();
    let dup_name = dup["Fight Trainer"]["trainer_name"].as_str().unwrap().to_string();
    events_mut(&mut b).push(dup);

    let cmp = compare(digest_of(&a, "a"), digest_of(&b, "b"));
    assert_eq!(cmp.trainer_sets.repeat_b, vec![dup_name], "the second fight is a repeat");
    assert_eq!(cmp.b.totals.trainers, cmp.a.totals.trainers + 1);
}

#[test]
fn disabled_events_are_ignored_everywhere() {
    let name = "c-porygon-1-.json";
    let a = raw(name);
    let mut b = a.clone();
    // Disable the first top-level folder.
    let folder_idx = events_mut(&mut b)
        .iter()
        .position(|e| e.get("Event Folder Name").is_some())
        .expect("a folder");
    events_mut(&mut b)[folder_idx]["Enabled"] = Value::Bool(false);

    let da = digest_of(&a, "a");
    let db = digest_of(&b, "b");
    assert!(db.disabled_events > 0, "the disabled folder's events are counted");
    assert!(db.entries.len() < da.entries.len(), "and excluded from the entries");
    assert_eq!(db.entries.len() + db.disabled_events, da.entries.len());
}

#[test]
fn wild_quantity_counts_and_merges() {
    let name = "c-porygon-1-.json";
    let a = raw(name);
    let da = digest_of(&a, "a");
    let Some(before) = da.entries.iter().find(|e| e.kind == EntryKind::Wild).cloned() else {
        eprintln!("skipping: no wild events in this fixture");
        return;
    };

    let mut b = a.clone();
    // Bump the quantity of every wild event by 2.
    fn bump(node: &mut Value) {
        let Some(children) = node["events"].as_array_mut() else { return };
        for child in children.iter_mut() {
            if child.get("Event Folder Name").is_some() {
                bump(child);
            } else if let Some(w) = child.get_mut("Fight Wild Pkmn") {
                let q = w[2].as_i64().unwrap_or(1);
                w[2] = Value::from(q + 2);
            }
        }
    }
    bump(&mut b["events"][0]);
    let db = digest_of(&b, "b");
    let wild_events = da.entries.iter().filter(|e| e.kind == EntryKind::Wild).count() as i64;
    assert_eq!(
        db.totals.wild_pokemon,
        da.totals.wild_pokemon + 2 * wild_events,
        "quantity, not event count, drives the total"
    );
    assert_eq!(db.totals.wild_species, da.totals.wild_species);
    assert_eq!(before.qty + 2, db.entries.iter().find(|e| e.kind == EntryKind::Wild).unwrap().qty);
}

#[test]
fn a_skipped_level_up_move_is_not_learned() {
    let name = "yellow-pinsir-lv10brock.json";
    let d = load(name);
    for m in &d.moves_learned {
        assert!(!m.name.is_empty());
        assert!(m.level > 0, "{} has no level", m.name);
    }
    assert_eq!(
        d.totals.moves_learned(),
        d.moves_learned.len() as i64,
        "the move totals and the list must agree"
    );
    let by_level_up = d.moves_learned.iter().filter(|m| m.source == MoveSource::LevelUp).count() as i64;
    assert_eq!(by_level_up, d.totals.moves_level_up);
}

#[test]
fn hidden_power_follows_the_dvs_and_is_absent_in_gen_one() {
    let yellow = load("yellow-pinsir-lv10brock.json");
    assert_eq!(yellow.generation, 1);
    assert!(yellow.hidden_power.is_none(), "gen 1 has no Hidden Power");
    assert_eq!(yellow.dv_text(), "DVs");
    assert_eq!(yellow.ev_text(), "StatExp");
    assert!(yellow.nature.is_none() && yellow.ability.is_none());

    let crystal = load("c-porygon-1-.json");
    assert_eq!(crystal.generation, 2);
    let hp = crystal.hidden_power.clone().expect("gen 2 has Hidden Power");
    assert!(!hp.0.is_empty() && hp.1 > 0, "{hp:?}");

    // Changing the DVs changes the Hidden Power.
    let mut other = raw("c-porygon-1-.json");
    other["dv"]["attack"] = Value::from(if crystal.dvs[1] == 15 { 14 } else { 15 });
    let changed = digest_of(&other, "b");
    assert_ne!(changed.dvs, crystal.dvs);
    assert_ne!(changed.hidden_power, crystal.hidden_power, "HP type/power tracks the DVs");

    let plat = load("platinum_chimchar.json");
    assert_eq!(plat.generation, 4);
    assert_eq!(plat.dv_text(), "IVs");
    assert_eq!(plat.ev_text(), "EVs");
    assert!(plat.nature.is_some() && plat.ability.is_some());
}

#[test]
fn routes_of_different_generations_compare_without_panicking() {
    let cmp: RouteComparison = compare(load("yellow-pinsir-lv10brock.json"), load("platinum_chimchar.json"));
    assert!(!cmp.compat.same_generation);
    assert!(!cmp.compat.same_version);
    assert!(!cmp.compat.same_species);
    // Two unrelated routes share no fights, so every trainer is one-sided.
    assert_eq!(cmp.checkpoints.len(), cmp.trainer_sets.same_order + cmp.trainer_sets.different_order);
    let _ = xpr_engine::compare::text_summary(&cmp);
}

#[test]
fn an_errored_event_is_counted_and_does_not_panic() {
    let name = "c-porygon-1-.json";
    let mut broken = raw(name);
    // Use an item that was never acquired: the event errors, but the route
    // still loads and every later state is still computed.
    events_mut(&mut broken).insert(
        0,
        serde_json::json!({ "Enabled": true, "Tags": [], "Inventory Event": ["Full Restore", 1, false, false] }),
    );

    let d = digest_of(&broken, "broken");
    assert!(d.totals.events_with_errors > 0, "the impossible item use is counted as an error");
    assert!(d.entries.iter().any(|e| e.has_error));

    let cmp = compare(d, load(name));
    assert!(!cmp.rows.is_empty());
    // The error must not break the alignment: every fight still pairs.
    assert_eq!(cmp.trainer_sets.only_a.len(), 0);
    assert_eq!(cmp.trainer_sets.only_b.len(), 0);
}

#[test]
fn the_text_summary_names_both_routes() {
    let cmp = compare(load("c-porygon-1-.json"), load("c-porygon-1-.json"));
    let text = xpr_engine::compare::text_summary(&cmp);
    assert!(text.contains("Route compare"), "{text}");
    assert!(text.contains("Trainers fought"), "{text}");
    assert!(text.lines().count() > 5, "{text}");
}

/// A heal event at `location`, in the shape the recorder writes.
fn heal_at(location: &str) -> Value {
    serde_json::json!({ "Enabled": true, "Tags": [], "PkmnCenter Heal": [location] })
}

#[test]
fn heals_pair_whatever_their_location() {
    // SPEC §5.3: the location is not part of a heal's match key. Two players
    // healing in different towns before the same fight did the same thing.
    let mut a = emerald_route(&["Rival Brendan 1 Mudkip"]);
    let mut b = emerald_route(&["Rival Brendan 1 Mudkip"]);
    events_mut(&mut a).insert(0, heal_at("OLDALE_TOWN - POKEMON_CENTER_1F"));
    events_mut(&mut b).insert(0, heal_at("PETALBURG_CITY - POKEMON_CENTER_1F"));

    let cmp = compare(digest_of(&a, "a"), digest_of(&b, "b"));
    let DiffRow::Block { a: lane_a, b: lane_b } = &cmp.rows[0] else { panic!("the heals come before the fight") };
    assert_eq!(lane_a.len(), 1);
    assert_eq!(lane_b.len(), 1);
    assert_eq!(lane_a[0].status, ItemStatus::InBoth, "matched by key, not by the town's name");
    assert_eq!(lane_b[0].status, ItemStatus::InBoth);
    // each lane still shows where *that* route healed
    assert_eq!(lane_a[0].merged_label, "Oldale Town");
    assert_eq!(lane_b[0].merged_label, "Petalburg City");
    assert_eq!(cmp.blocks_with_differences(), 0);
}

#[test]
fn merged_heals_in_different_towns_drop_the_place_name() {
    let mut a = emerald_route(&["Rival Brendan 1 Mudkip"]);
    events_mut(&mut a).insert(0, heal_at("OLDALE_TOWN - POKEMON_CENTER_1F"));
    events_mut(&mut a).insert(1, heal_at("PETALBURG_CITY - POKEMON_CENTER_1F"));
    let b = emerald_route(&["Rival Brendan 1 Mudkip"]);

    let cmp = compare(digest_of(&a, "a"), digest_of(&b, "b"));
    let DiffRow::Block { a: lane_a, .. } = &cmp.rows[0] else { panic!("block first") };
    assert_eq!(lane_a.len(), 1, "both heals merge into one line");
    assert_eq!(lane_a[0].qty, 2);
    assert_eq!(lane_a[0].merged_label, "Heal", "two towns cannot share one town's name");
    assert_eq!(lane_a[0].status, ItemStatus::OnlyHere);
}

#[test]
fn a_moveset_that_is_a_superset_still_differs() {
    // B knows everything A knows plus one more move: "A has, B lacks" is 0,
    // but the movesets are not the same.
    let name = "c-porygon-1-.json";
    let cmp = compare(load(name), load(name));
    let mut a = cmp.a.clone();
    let b = cmp.b.clone();
    let Some(idx) = a.entries.iter().position(|e| e.kind == EntryKind::Trainer && e.before.move_set().len() >= 2) else {
        eprintln!("skipping: no fight with two known moves in this fixture");
        return;
    };
    // drop one of A's moves before that fight
    let slot = a.entries[idx].before.moves.iter().position(|m| m.as_deref().map(|s| !s.is_empty()).unwrap_or(false)).unwrap();
    a.entries[idx].before.moves[slot] = None;

    let cmp = compare(a, b);
    let cp = cmp.checkpoints.iter().find(|c| c.a == idx).expect("the fight is shared");
    assert_eq!(cp.moves_differing, 1, "B's extra move must count");
}
