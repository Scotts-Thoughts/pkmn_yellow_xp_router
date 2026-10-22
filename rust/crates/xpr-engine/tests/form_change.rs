//! Form changes (Shaymin (Land) <-> Shaymin (Sky)): stored as evolutions into
//! the other form, labelled "Change Form into: ...".

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use xpr_data::Registry;
use xpr_engine::{NodeId, Router};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn registry() -> Arc<Registry> {
    Arc::new(Registry::new(repo_root().join("raw_pkmn_data"), PathBuf::new()))
}

fn route(version: &str, mon: &str, evolutions: &[&str]) -> Value {
    let mut events = vec![json!({"Enabled": true, "Tags": [], "Fight Wild Pkmn": ["Bidoof", 5, 3, false]})];
    for species in evolutions {
        events.push(json!({"Enabled": true, "Tags": [], "Evolution": {"evolved_species": species, "by_stone": null}}));
    }
    json!({
        "name": mon,
        "Version": version,
        "events": [{"Event Folder Name": "ROOT", "Just Notes": "", "events": [
            {"Event Folder Name": "Main", "Just Notes": "", "Expanded": true, "Enabled": true, "events": events}
        ]}]
    })
}

fn mon_after(router: &Router, id: NodeId) -> &xpr_engine::SoloPokemon {
    &router.group(id).unwrap().final_state.as_ref().unwrap().solo_pkmn
}

#[test]
fn forms_share_a_dex_number() {
    let reg = registry();
    let plat = reg.get_version("Platinum").unwrap();
    assert_eq!(plat.pkmn_db().get_forms("Shaymin (Land)"), vec!["Shaymin (Sky)".to_string()]);
    assert_eq!(plat.pkmn_db().get_forms("Shaymin (Sky)"), vec!["Shaymin (Land)".to_string()]);
    assert!(plat.pkmn_db().get_forms("Bidoof").is_empty());
    assert!(plat.pkmn_db().is_form_change("Shaymin (Land)", "Shaymin (Sky)"));
    assert!(!plat.pkmn_db().is_form_change("Shaymin (Land)", "Shaymin (Land)"));
    assert!(!plat.pkmn_db().is_form_change("Bidoof", "Bibarel"));
    // gen 5 data names the Land Forme plainly
    let black = reg.get_version("Black").unwrap();
    assert_eq!(black.pkmn_db().get_forms("Shaymin"), vec!["Shaymin (Sky)".to_string()]);
    // gens 1-3 have no dex numbers, so no forms
    let emerald = reg.get_version("Emerald").unwrap();
    assert!(emerald.pkmn_db().get_forms("Deoxys").is_empty());
}

#[test]
fn shaymin_changes_form_and_back() {
    let mut router = Router::new(registry());
    router.load_value(&route("Platinum", "Shaymin (Land)", &["Shaymin (Sky)", "Shaymin (Land)"]), false).expect("route loads");
    let groups = router.all_groups();
    assert_eq!(groups.len(), 3);
    let land = mon_after(&router, groups[0]).clone();

    let sky_group = router.group(groups[1]).unwrap();
    assert!(sky_group.error_messages.is_empty(), "{:?}", sky_group.error_messages);
    assert_eq!(sky_group.name, "Change Form into: Shaymin (Sky)");
    let sky = mon_after(&router, groups[1]);
    assert_eq!(sky.name, "Shaymin (Sky)");
    assert_eq!(sky.cur_xp, land.cur_xp);
    assert_eq!(sky.cur_level, land.cur_level);
    assert_eq!(sky.move_list, land.move_list);
    assert_eq!(sky.ability_idx, land.ability_idx);
    assert!(sky.cur_stats.speed > land.cur_stats.speed, "Sky Forme: base 127 Spe vs 100");
    assert_eq!(sky.species_def.second_type, "Flying");

    assert_eq!(router.group(groups[2]).unwrap().name, "Change Form into: Shaymin (Land)");
    assert_eq!(mon_after(&router, groups[2]).cur_stats, land.cur_stats);
}

#[test]
fn a_real_evolution_keeps_its_label() {
    let mut router = Router::new(registry());
    router.load_value(&route("Platinum", "Turtwig", &["Grotle"]), false).expect("route loads");
    let groups = router.all_groups();
    assert_eq!(router.group(groups[1]).unwrap().name, "Evolve into: Grotle");
}
