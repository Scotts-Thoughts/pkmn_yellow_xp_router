//! Gen 1 bag reordering: the swap helper, the name-first application with
//! warnings, the definition's (de)serialization and the route-level
//! behaviour (warnings never make the run invalid).

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use xpr_core::consts;
use xpr_data::Registry;
use xpr_engine::{swaps_between, BagReorderEventDefinition, BagSwap, EventDefinition, InsertSpec, InventoryEventDefinition, NodeId, Router};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn registry() -> Arc<Registry> {
    Arc::new(Registry::new(repo_root().join("raw_pkmn_data"), PathBuf::new()))
}

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn apply(old: &[String], swaps: &[BagSwap]) -> Vec<String> {
    let mut cur = old.to_vec();
    for s in swaps {
        cur.swap(s.slot_a - 1, s.slot_b - 1);
    }
    cur
}

// ---- swaps_between ----------------------------------------------------------

#[test]
fn identity_needs_no_swaps() {
    let bag = names(&["Potion", "Poke Ball", "Antidote"]);
    assert_eq!(swaps_between(&bag, &bag).unwrap(), Vec::<BagSwap>::new());
}

#[test]
fn one_transposition_is_one_swap() {
    let old = names(&["Potion", "Poke Ball", "Antidote"]);
    let new = names(&["Antidote", "Poke Ball", "Potion"]);
    let swaps = swaps_between(&old, &new).unwrap();
    assert_eq!(swaps, vec![BagSwap::new("Potion", 1, "Antidote", 3)]);
    assert_eq!(swaps[0].to_string(), "Potion (1) <-> Antidote (3)");
}

#[test]
fn a_three_cycle_is_two_swaps() {
    let old = names(&["A", "B", "C", "D"]);
    let new = names(&["B", "C", "A", "D"]);
    let swaps = swaps_between(&old, &new).unwrap();
    assert_eq!(swaps.len(), 2);
    assert_eq!(apply(&old, &swaps), new);
}

#[test]
fn swaps_reproduce_any_permutation() {
    let old = names(&["A", "B", "C", "D", "E", "F"]);
    let perms = [
        ["F", "E", "D", "C", "B", "A"],
        ["B", "A", "D", "C", "F", "E"],
        ["C", "A", "B", "F", "D", "E"],
        ["A", "F", "B", "E", "C", "D"],
    ];
    for p in perms {
        let new = names(&p);
        let swaps = swaps_between(&old, &new).unwrap();
        assert_eq!(apply(&old, &swaps), new, "{:?}", p);
        assert!(swaps.len() < old.len());
    }
}

#[test]
fn different_contents_are_rejected() {
    let old = names(&["Potion", "Poke Ball"]);
    assert!(swaps_between(&old, &names(&["Potion", "Antidote"])).is_err());
    assert!(swaps_between(&old, &names(&["Potion"])).is_err());
    assert!(swaps_between(&old, &names(&["Potion", "Poke Ball", "Antidote"])).is_err());
}

// ---- Inventory::swap_items ------------------------------------------------------

fn yellow_bag(items: &[(&str, i64)]) -> xpr_engine::Inventory {
    let gen = registry().get_version("Yellow").unwrap();
    let mut inv = xpr_engine::Inventory::new(None, Vec::new(), gen.bag_limit());
    for (name, num) in items {
        let item = gen.item_db().get_item(name).unwrap();
        inv = inv.add_item(item, *num, false, false, None).unwrap();
    }
    inv
}

#[test]
fn exact_slots_swap_silently() {
    let inv = yellow_bag(&[("Potion", 3), ("Poke Ball", 5), ("Antidote", 1)]);
    let (after, warnings) = inv.swap_items(&[BagSwap::new("Potion", 1, "Antidote", 3)]);
    assert!(warnings.is_empty(), "{:?}", warnings);
    assert_eq!(after.item_names(), names(&["Antidote", "Poke Ball", "Potion"]));
    assert_eq!(after.cur_items[2].num, 3);
    // the original is untouched
    assert_eq!(inv.item_names(), names(&["Potion", "Poke Ball", "Antidote"]));
}

#[test]
fn names_win_over_slots_with_a_warning() {
    // the bag gained an item at the front, so every recorded slot is off by one
    let inv = yellow_bag(&[("Escape Rope", 1), ("Potion", 3), ("Poke Ball", 5), ("Antidote", 1)]);
    let (after, warnings) = inv.swap_items(&[BagSwap::new("Potion", 1, "Antidote", 3)]);
    assert_eq!(after.item_names(), names(&["Escape Rope", "Antidote", "Poke Ball", "Potion"]));
    assert_eq!(warnings, vec!["Potion was at slot 2, not slot 1".to_string(), "Antidote was at slot 4, not slot 3".to_string()]);
}

#[test]
fn a_missing_item_skips_only_its_swap() {
    let inv = yellow_bag(&[("Potion", 3), ("Poke Ball", 5), ("Antidote", 1)]);
    let swaps = vec![BagSwap::new("Potion", 1, "Master Ball", 4), BagSwap::new("Poke Ball", 2, "Antidote", 3)];
    let (after, warnings) = inv.swap_items(&swaps);
    assert_eq!(after.item_names(), names(&["Potion", "Antidote", "Poke Ball"]));
    assert_eq!(warnings, vec!["Cannot swap Potion (1) <-> Master Ball (4): no Master Ball in bag".to_string()]);
}

#[test]
fn a_slot_past_the_end_is_just_a_wrong_slot() {
    let inv = yellow_bag(&[("Potion", 3), ("Poke Ball", 5)]);
    let (after, warnings) = inv.swap_items(&[BagSwap::new("Potion", 1, "Poke Ball", 9)]);
    assert_eq!(after.item_names(), names(&["Poke Ball", "Potion"]));
    assert_eq!(warnings, vec!["Poke Ball was at slot 2, not slot 9".to_string()]);
}

// ---- definition ----------------------------------------------------------------

#[test]
fn definition_round_trips_and_labels() {
    let gen = registry().get_version("Yellow").unwrap();
    let def = EventDefinition::with_bag_reorder(vec![BagSwap::new("Potion", 3, "Master Ball", 7), BagSwap::new("Antidote", 1, "Potion", 7)]);
    assert_eq!(def.get_event_type(), consts::TASK_REORDER_BAG);
    assert_eq!(def.get_label(&gen).unwrap(), "Reorder Bag: Potion (3) <-> Master Ball (7), Antidote (1) <-> Potion (7)");
    assert_eq!(def.get_item_label(&gen).unwrap(), def.get_label(&gen).unwrap());

    let raw = def.serialize();
    assert_eq!(raw[consts::TASK_REORDER_BAG], json!([["Potion", 3, "Master Ball", 7], ["Antidote", 1, "Potion", 7]]));
    let back = EventDefinition::deserialize(&raw).unwrap();
    assert_eq!(back, def);

    let empty = EventDefinition::with_bag_reorder(Vec::new());
    assert_eq!(empty.get_label(&gen).unwrap(), "Reorder Bag: (no swaps)");
    let back = EventDefinition::deserialize(&empty.serialize()).unwrap();
    assert_eq!(back.get_event_type(), consts::TASK_REORDER_BAG, "an empty reorder survives a reload");
    assert_eq!(back.bag_reorder, Some(BagReorderEventDefinition::default()));

    // a malformed entry fails the load loudly (silently degrading to notes
    // would drop the swaps on the next save); an explicit null is no reorder
    let bad = json!({"Enabled": true, "Tags": [], "Reorder Bag": [["Potion", 0, "Antidote", 2]]});
    assert!(EventDefinition::deserialize(&bad).unwrap_err().contains("Reorder Bag"));
    let bad = json!({"Enabled": true, "Tags": [], "Reorder Bag": [["Potion", 1]]});
    assert!(EventDefinition::deserialize(&bad).is_err());
    let bad = json!({"Enabled": true, "Tags": [], "Reorder Bag": "Potion"});
    assert!(EventDefinition::deserialize(&bad).is_err());
    let none = json!({"Enabled": true, "Tags": [], "Reorder Bag": null});
    assert_eq!(EventDefinition::deserialize(&none).unwrap().get_event_type(), consts::TASK_NOTES_ONLY);
    assert!(consts::ROUTE_EVENT_TYPES.contains(&consts::TASK_REORDER_BAG));
}

// ---- route level ------------------------------------------------------------------

fn yellow_route() -> Value {
    json!({
        "name": "Pikachu",
        "Version": "Yellow",
        "events": [{"Event Folder Name": "ROOT", "Just Notes": "", "events": [
            {"Event Folder Name": "Main", "Just Notes": "", "Expanded": true, "Enabled": true, "events": [
                {"Enabled": true, "Tags": [], "Inventory Event": ["Potion", 3, true, true]},
                {"Enabled": true, "Tags": [], "Inventory Event": ["Poke Ball", 5, true, true]},
                {"Enabled": true, "Tags": [], "Inventory Event": ["Antidote", 2, true, true]},
                {"Enabled": true, "Tags": [], "Reorder Bag": [["Potion", 1, "Antidote", 3]]}
            ]}
        ]}]
    })
}

fn bag_after(router: &Router, id: NodeId) -> Vec<String> {
    router.group(id).unwrap().final_state.as_ref().unwrap().inventory.item_names()
}

#[test]
fn a_recorded_reorder_applies_cleanly() {
    let mut router = Router::new(registry());
    router.load_value(&yellow_route(), false).expect("route loads");
    let groups = router.all_groups();
    assert_eq!(groups.len(), 4);
    let gen = router.gen().unwrap().clone();
    assert_eq!(bag_after(&router, groups[2]), names(&["Potion", "Poke Ball", "Antidote"]));
    let reorder = router.group(groups[3]).unwrap();
    assert_eq!(reorder.name, "Reorder Bag: Potion (1) <-> Antidote (3)");
    assert!(reorder.error_messages.is_empty());
    assert!(reorder.warning_messages.is_empty());
    assert_eq!(bag_after(&router, groups[3]), names(&["Antidote", "Poke Ball", "Potion"]));
    let after = router.group(groups[3]).unwrap().final_state.as_ref().unwrap();
    assert_eq!(after.inventory.cur_items.iter().map(|x| x.num).collect::<Vec<_>>(), vec![2, 5, 3], "counts travel with the items");
    assert_eq!(after.inventory.cur_money, router.group(groups[2]).unwrap().final_state.as_ref().unwrap().inventory.cur_money);
    assert!(router.get_tags(groups[3], &gen, false).is_empty());
    assert!(!router.root().has_errors());
    // the reorder is not an "invalid event"
    assert!(!router.do_render(groups[3], &gen, None, Some(&[consts::ERROR_SEARCH.to_string()])));
    assert!(router.do_render(groups[3], &gen, None, Some(&[consts::TASK_REORDER_BAG.to_string()])));
    assert!(!router.do_render(groups[3], &gen, None, Some(&[consts::TASK_USE_ITEM.to_string()])));
    assert!(router.do_render(groups[3], &gen, Some("antidote"), None));

    // save / reload keeps the event and its outcome
    let saved = router.serialize().unwrap();
    let mut reloaded = Router::new(router.registry().clone());
    reloaded.load_value(&saved, false).unwrap();
    let groups = reloaded.all_groups();
    assert_eq!(bag_after(&reloaded, groups[3]), names(&["Antidote", "Poke Ball", "Potion"]));
    assert!(reloaded.export_notes_text().unwrap().contains("Reorder Bag: Potion (1) <-> Antidote (3)"));
}

#[test]
fn an_edit_before_the_reorder_warns_but_still_swaps() {
    let mut router = Router::new(registry());
    router.load_value(&yellow_route(), false).expect("route loads");
    let groups = router.all_groups();
    let gen = router.gen().unwrap().clone();

    // an extra purchase at the front of the folder shifts every slot
    router
        .add_event_object(
            Some(EventDefinition::with_item(InventoryEventDefinition::new("Escape Rope", 1, true, true, None))),
            None,
            InsertSpec { insert_before: Some(groups[0]), insert_after: None, dest_folder_name: None },
            true,
            None,
            None,
        )
        .unwrap();
    let reorder_id = groups[3];
    let reorder = router.group(reorder_id).unwrap();
    assert!(reorder.error_messages.is_empty(), "{:?}", reorder.error_messages);
    // one entry per item; the item joins its own warnings
    assert_eq!(reorder.warning_messages, vec!["Potion was at slot 2, not slot 1, Antidote was at slot 4, not slot 3".to_string()]);
    assert_eq!(reorder.name, "Reorder Bag: Potion (1) <-> Antidote (3)", "warnings keep the label");
    assert_eq!(bag_after(&router, reorder_id), names(&["Escape Rope", "Antidote", "Poke Ball", "Potion"]));
    assert!(!reorder.has_errors());
    assert!(!router.root().has_errors(), "warnings never make the run invalid");
    assert_eq!(router.get_tags(reorder_id, &gen, false), vec![consts::EVENT_TAG_WARNINGS]);
    assert!(!router.do_render(reorder_id, &gen, None, Some(&[consts::ERROR_SEARCH.to_string()])));
    let item_id = router.group(reorder_id).unwrap().event_items[0];
    assert_eq!(router.item(item_id).unwrap().get_tags(), vec![consts::EVENT_TAG_WARNINGS]);

    // removing one of the swapped items: that swap is skipped, still no error
    router.remove_event_object(groups[2], true).unwrap();
    let reorder = router.group(reorder_id).unwrap();
    assert!(reorder.error_messages.is_empty());
    assert_eq!(reorder.warning_messages, vec!["Cannot swap Potion (1) <-> Antidote (3): no Antidote in bag".to_string()]);
    assert_eq!(bag_after(&router, reorder_id), names(&["Escape Rope", "Potion", "Poke Ball"]));

    // disabling the event clears the warning
    let mut disabled = router.group(reorder_id).unwrap().event_definition.clone();
    disabled.enabled = Some(false);
    router.replace_event_group(reorder_id, disabled).unwrap();
    let reorder = router.group(reorder_id).unwrap();
    assert!(reorder.warning_messages.is_empty());
    assert!(router.get_tags(reorder_id, &gen, false).is_empty());
    assert_eq!(bag_after(&router, reorder_id), names(&["Escape Rope", "Potion", "Poke Ball"]));
}

#[test]
fn several_swaps_apply_in_order() {
    let mut router = Router::new(registry());
    router.load_value(&yellow_route(), false).expect("route loads");
    let groups = router.all_groups();
    // Potion, Poke Ball, Antidote -> Poke Ball, Antidote, Potion (a 3-cycle)
    let old = names(&["Potion", "Poke Ball", "Antidote"]);
    let new = names(&["Poke Ball", "Antidote", "Potion"]);
    let swaps = swaps_between(&old, &new).unwrap();
    router.replace_event_group(groups[3], EventDefinition::with_bag_reorder(swaps)).unwrap();
    let reorder = router.group(groups[3]).unwrap();
    assert!(reorder.warning_messages.is_empty(), "{:?}", reorder.warning_messages);
    assert_eq!(bag_after(&router, groups[3]), new);
}

#[test]
fn a_highlight_outranks_a_warning() {
    let mut router = Router::new(registry());
    router.load_value(&yellow_route(), false).expect("route loads");
    let groups = router.all_groups();
    let gen = router.gen().unwrap().clone();
    let reorder_id = groups[3];
    router.set_event_highlight(reorder_id, Some(3)).unwrap();
    router.remove_event_object(groups[0], true).unwrap();
    assert!(router.group(reorder_id).unwrap().has_warnings());
    assert_eq!(router.get_tags(reorder_id, &gen, false), vec![consts::ALL_HIGHLIGHT_LABELS[2]], "the user's highlight still shows");
    router.set_event_highlight(reorder_id, None).unwrap();
    assert_eq!(router.get_tags(reorder_id, &gen, false), vec![consts::EVENT_TAG_WARNINGS]);
}
