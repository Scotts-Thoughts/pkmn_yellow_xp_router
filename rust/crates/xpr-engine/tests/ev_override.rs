//! The "EV Override" event: a testing aid that pins the solo mon's EVs
//! (stat exp in gens 1-2) to fixed values from that point of the route on.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use xpr_core::consts;
use xpr_data::{Registry, StatBlock};
use xpr_engine::{EvOverrideEventDefinition, EventDefinition, NodeId, Router};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn registry() -> Arc<Registry> {
    Arc::new(Registry::new(repo_root().join("raw_pkmn_data"), PathBuf::new()))
}

fn evs(s: &StatBlock) -> [i64; 6] {
    [s.hp, s.attack, s.defense, s.special_attack, s.special_defense, s.speed]
}

fn route(version: &str, mon: &str, override_entry: Value) -> Value {
    json!({
        "name": mon,
        "Version": version,
        "events": [{"Event Folder Name": "ROOT", "Just Notes": "", "events": [
            {"Event Folder Name": "Main", "Just Notes": "", "Expanded": true, "Enabled": true, "events": [
                {"Enabled": true, "Tags": [], "Fight Wild Pkmn": ["Pidgey", 5, 3, false]},
                {"Enabled": true, "Tags": [], "EV Override": override_entry},
                {"Enabled": true, "Tags": [], "Fight Wild Pkmn": ["Pidgey", 5, 1, false]}
            ]}
        ]}]
    })
}

fn mon_after(router: &Router, id: NodeId) -> &xpr_engine::SoloPokemon {
    &router.group(id).unwrap().final_state.as_ref().unwrap().solo_pkmn
}

// ---- definition ----------------------------------------------------------------

#[test]
fn definition_round_trips_and_labels() {
    let reg = registry();
    let gen = reg.get_version("Emerald").unwrap();
    let def = EventDefinition::with_ev_override(EvOverrideEventDefinition::new(1, 2, 3, 4, 5, 6));
    assert_eq!(def.get_event_type(), consts::TASK_EV_OVERRIDE);
    assert_eq!(def.get_label(&gen).unwrap(), "EV Override: 1 HP, 2 Atk, 3 Def, 4 SpA, 5 SpD, 6 Spe");
    // gens 1-2 have one Special stat exp
    let yellow = reg.get_version("Yellow").unwrap();
    assert_eq!(def.get_label(&yellow).unwrap(), "Stat Exp Override: 1 HP, 2 Atk, 3 Def, 4 Spc, 6 Spe");
    assert_eq!(def.get_item_label(&gen).unwrap(), def.get_label(&gen).unwrap());

    let raw = def.serialize();
    assert_eq!(
        raw[consts::TASK_EV_OVERRIDE],
        json!({"hp": 1, "attack": 2, "defense": 3, "speed": 6, "special_attack": 4, "special_defense": 5})
    );
    let back = EventDefinition::deserialize(&raw).unwrap();
    assert_eq!(back, def);

    // a missing stat is 0; a malformed entry fails the load loudly; null is no override
    let partial = json!({"Enabled": true, "Tags": [], "EV Override": {"attack": 40}});
    let back = EventDefinition::deserialize(&partial).unwrap();
    assert_eq!(back.ev_override, Some(EvOverrideEventDefinition::new(0, 40, 0, 0, 0, 0)));
    let bad = json!({"Enabled": true, "Tags": [], "EV Override": [1, 2, 3]});
    assert!(EventDefinition::deserialize(&bad).unwrap_err().contains("EV Override"));
    let bad = json!({"Enabled": true, "Tags": [], "EV Override": {"hp": "lots"}});
    assert!(EventDefinition::deserialize(&bad).is_err());
    let none = json!({"Enabled": true, "Tags": [], "EV Override": null});
    assert_eq!(EventDefinition::deserialize(&none).unwrap().get_event_type(), consts::TASK_NOTES_ONLY);
    assert!(consts::ROUTE_EVENT_TYPES.contains(&consts::TASK_EV_OVERRIDE));
}

// ---- route level ------------------------------------------------------------------

#[test]
fn the_override_replaces_earned_evs_and_later_yields_stack_on_it() {
    let mut router = Router::new(registry());
    router
        .load_value(&route("Emerald", "Torchic", json!({"hp": 4, "attack": 100, "defense": 0, "special_attack": 252, "special_defense": 0, "speed": 20})), false)
        .expect("route loads");
    let groups = router.all_groups();
    assert_eq!(groups.len(), 3);
    let gen = router.gen().unwrap().clone();
    let before = mon_after(&router, groups[0]);
    assert_eq!(evs(&before.unrealized_stat_xp), [0, 0, 0, 0, 0, 3], "three Pidgey = 3 Spe EVs");

    let ov = router.group(groups[1]).unwrap();
    assert_eq!(ov.name, "EV Override: 4 HP, 100 Atk, 0 Def, 252 SpA, 0 SpD, 20 Spe");
    assert!(ov.error_messages.is_empty(), "{:?}", ov.error_messages);
    assert!(ov.warning_messages.is_empty(), "{:?}", ov.warning_messages);
    let mon = mon_after(&router, groups[1]);
    assert_eq!(evs(&mon.unrealized_stat_xp), [4, 100, 0, 252, 0, 20]);
    assert_eq!(evs(&mon.realized_stat_xp), [4, 100, 0, 252, 0, 20], "the stats reflect the override right away");
    assert_eq!(mon.cur_level, before.cur_level);
    assert_eq!(mon.cur_xp, before.cur_xp);
    assert!(mon.cur_stats.special_attack > before.cur_stats.special_attack, "252 SpA EVs at level 5 are worth at least a point");

    // the next yield adds to the override, not to the old total
    assert_eq!(evs(&mon_after(&router, groups[2]).unrealized_stat_xp), [4, 100, 0, 252, 0, 21]);

    assert!(router.do_render(groups[1], &gen, None, Some(&[consts::TASK_EV_OVERRIDE.to_string()])));
    assert!(!router.do_render(groups[1], &gen, None, Some(&[consts::ERROR_SEARCH.to_string()])));
    assert!(router.do_render(groups[1], &gen, Some("252 spa"), None));

    // save / reload keeps the event and its outcome
    let saved = router.serialize().unwrap();
    let mut reloaded = Router::new(router.registry().clone());
    reloaded.load_value(&saved, false).unwrap();
    let groups = reloaded.all_groups();
    assert_eq!(evs(&mon_after(&reloaded, groups[2]).unrealized_stat_xp), [4, 100, 0, 252, 0, 21]);
    assert!(reloaded.export_notes_text().unwrap().contains("EV Override: 4 HP"));
}

#[test]
fn over_cap_values_are_capped_with_a_warning() {
    let mut router = Router::new(registry());
    router
        .load_value(&route("Emerald", "Torchic", json!({"hp": 300, "attack": 252, "defense": 252, "special_attack": 0, "special_defense": 0, "speed": 0})), false)
        .expect("route loads");
    let groups = router.all_groups();
    let ov = router.group(groups[1]).unwrap();
    assert!(ov.error_messages.is_empty(), "{:?}", ov.error_messages);
    assert_eq!(ov.warning_messages, vec!["HP 300 capped to 255, Def 252 capped to 3".to_string()]);
    assert!(!ov.has_errors());
    assert_eq!(evs(&mon_after(&router, groups[1]).unrealized_stat_xp), [255, 252, 3, 0, 0, 0]);
}

#[test]
fn gen1_stat_exp_uses_the_16_bit_range() {
    let mut router = Router::new(registry());
    router
        .load_value(&route("Yellow", "Pikachu", json!({"hp": 65535, "attack": 70000, "defense": 0, "special_attack": 12345, "special_defense": 12345, "speed": 0})), false)
        .expect("route loads");
    let groups = router.all_groups();
    let ov = router.group(groups[1]).unwrap();
    assert_eq!(ov.warning_messages, vec!["Atk 70000 capped to 65535".to_string()]);
    // the label keeps the requested value; the warning reports the cap
    assert_eq!(ov.name, "Stat Exp Override: 65535 HP, 70000 Atk, 0 Def, 12345 Spc, 0 Spe");
    let mon = mon_after(&router, groups[1]);
    assert_eq!(evs(&mon.unrealized_stat_xp), [65535, 65535, 0, 12345, 12345, 0]);
    // max stat exp at level 5: HP gains floor(sqrt(65535)/4 * 5 / 100)... just check it moved
    assert!(mon.cur_stats.hp > mon_after(&router, groups[0]).cur_stats.hp);
}
