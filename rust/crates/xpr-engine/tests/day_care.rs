//! The "Day Care" event: the exp a mon earned in the Day Care, added when it is
//! collected, as the game does.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use xpr_core::consts;
use xpr_data::Registry;
use xpr_engine::{EventDefinition, Router};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn registry() -> Arc<Registry> {
    Arc::new(Registry::new(repo_root().join("raw_pkmn_data"), PathBuf::new()))
}

fn route(exp_gained: i64) -> Value {
    json!({
        "name": "Torchic",
        "Version": "Emerald",
        "events": [{"Event Folder Name": "ROOT", "Just Notes": "", "events": [
            {"Event Folder Name": "Main", "Just Notes": "", "Expanded": true, "Enabled": true, "events": [
                {"Enabled": true, "Tags": [], "Fight Wild Pkmn": ["Pidgey", 5, 3, false]},
                {"Enabled": true, "Tags": [], "Day Care": exp_gained}
            ]}
        ]}]
    })
}

#[test]
fn definition_round_trips_and_labels() {
    let gen = registry().get_version("Emerald").unwrap();
    let def = EventDefinition::with_day_care(1326);
    assert_eq!(def.get_event_type(), consts::TASK_DAY_CARE);
    assert_eq!(def.get_label(&gen).unwrap(), "Day Care, 1326 exp");
    let raw = def.serialize();
    assert_eq!(raw[consts::TASK_DAY_CARE], json!(1326));
    assert_eq!(EventDefinition::deserialize(&raw).unwrap(), def);
    assert!(consts::ROUTE_EVENT_TYPES.contains(&consts::TASK_DAY_CARE));
}

#[test]
fn exp_levels_the_mon_and_the_fee_is_100_plus_100_a_level() {
    let mut router = Router::new(registry());
    router.load_value(&route(1326), false).expect("route loads");
    let groups = router.all_groups();
    let before = router.group(groups[0]).unwrap().final_state.clone().unwrap();
    let group = router.group(groups[1]).unwrap();
    let after = group.final_state.clone().unwrap();
    assert!(group.error_messages.is_empty(), "{:?}", group.error_messages);
    assert_eq!(after.solo_pkmn.cur_xp, before.solo_pkmn.cur_xp + 1326);
    let levels = after.solo_pkmn.cur_level - before.solo_pkmn.cur_level;
    assert!(levels >= 2, "this much exp is several levels for a level 5 mon, got {levels}");
    assert_eq!(after.inventory.cur_money, before.inventory.cur_money - (100 + 100 * levels));
    assert_eq!(after.solo_pkmn.realized_stat_xp, after.solo_pkmn.unrealized_stat_xp, "the game recalculates the stats on the way out");
}

#[test]
fn a_stay_too_short_to_level_still_realizes_stat_exp_and_costs_100() {
    let mut router = Router::new(registry());
    router.load_value(&route(1), false).expect("route loads");
    let groups = router.all_groups();
    let before = router.group(groups[0]).unwrap().final_state.clone().unwrap();
    let after = router.group(groups[1]).unwrap().final_state.clone().unwrap();
    assert_eq!(after.solo_pkmn.cur_level, before.solo_pkmn.cur_level);
    assert_eq!(after.solo_pkmn.realized_stat_xp, before.solo_pkmn.unrealized_stat_xp);
    assert_eq!(after.inventory.cur_money, before.inventory.cur_money - 100);
}
