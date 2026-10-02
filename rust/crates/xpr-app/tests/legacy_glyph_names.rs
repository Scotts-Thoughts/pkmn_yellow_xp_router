//! Gen 5 trainer names used to spell ♂/♀ and "PKMN" with the game's font
//! glyphs (⑭ ⑮ ⒆⒇), which no font draws. The data now spells them out
//! ("Pokemon Trainer Bianca (4)", "Clerk M Chaz (19)"). A route saved with
//! the old names still loads, its fights renamed, and a custom gen whose
//! data still has the glyphs reads under the new names.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::json;
use xpr_app::controller::MainController;
use xpr_core::io_utils::{fix_legacy_name_glyphs, fix_legacy_name_glyphs_in};
use xpr_core::Paths;
use xpr_data::Registry;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn fight(name: &str, second: serde_json::Value) -> serde_json::Value {
    json!({"Enabled": true, "Tags": [], "Fight Trainer": {"trainer_name": name, "second_trainer_name": second, "verbose": false, "setup_moves": [], "enemy_setup_moves": [], "player_field_moves": [], "enemy_field_moves": [], "mimic_selection": "", "custom_move_data": [], "exp_split": [], "weather": "None", "pay_day_amount": 0, "mon_order": [1], "transformed": false}})
}

#[test]
fn the_glyphs_are_spelled_out() {
    assert_eq!(fix_legacy_name_glyphs("\u{2486}\u{2487} Trainer Bianca (4)"), "Pokemon Trainer Bianca (4)");
    assert_eq!(fix_legacy_name_glyphs("Clerk \u{246d} Chaz (19)"), "Clerk M Chaz (19)");
    assert_eq!(fix_legacy_name_glyphs("Swimmer \u{246e} Ruth"), "Swimmer F Ruth");
    assert!(matches!(fix_legacy_name_glyphs("Leader Cheren (156)"), std::borrow::Cow::Borrowed(_)));
    let mut v = json!({"major_fights": {"rival": ["\u{2486}\u{2487} Trainer Rival (163)"]}, "\u{2486}\u{2487} Trainer N (5)": "x"});
    fix_legacy_name_glyphs_in(&mut v);
    assert_eq!(v, json!({"major_fights": {"rival": ["Pokemon Trainer Rival (163)"]}, "Pokemon Trainer N (5)": "x"}));
}

#[test]
fn the_renamed_gen5_data_has_no_glyphs_left() {
    let reg = Registry::new(repo_root().join("raw_pkmn_data"), PathBuf::new());
    for v in ["Black", "White", "Black 2", "White 2"] {
        let gen = reg.get_version(v).unwrap();
        assert!(gen.trainer_db().get_trainer("Pokemon Trainer Cheren (54)").is_some() || v.contains('2'), "{}", v);
        for t in gen.trainer_db().iter() {
            assert!(fix_legacy_name_glyphs(&t.name) == t.name.as_str() && fix_legacy_name_glyphs(&t.trainer_class) == t.trainer_class.as_str(), "{}: {}", v, t.name);
        }
    }
    let b2 = reg.get_version("Black 2").unwrap();
    assert!(b2.trainer_db().get_trainer("Pokemon Trainer Bianca (4)").is_some());
    assert!(b2.trainer_db().get_trainer("Clerk M Chaz (19)").is_some());
    // Lass Helia's Nidoran pair are the real species
    let helia = b2.trainer_db().iter().find(|t| t.name.starts_with("Lass Helia")).unwrap();
    assert!(helia.pkmn.iter().any(|p| p.name == "Nidoran\u{2642}") && helia.pkmn.iter().any(|p| p.name == "Nidoran\u{2640}"));
}

#[test]
fn a_route_saved_with_the_old_names_loads_under_the_new_ones() {
    let route = json!({
        "name": "Snivy",
        "dv": {"hp": 31, "attack": 31, "defense": 31, "speed": 31, "special_attack": 31, "special_defense": 31},
        "ability": 0,
        "nature": 0,
        "Version": "Black 2",
        "Learn Levelup Move": [],
        "events": [{"Event Folder Name": "Aspertia", "Just Notes": "", "events": [
            fight("\u{2486}\u{2487} Trainer Bianca (4)", json!(0)),
            fight("Clerk \u{246d} Chaz (19)", json!("Clerk \u{246e} Trisha")),
        ]}]
    });
    let dir = std::env::temp_dir().join(format!("xpr_legacy_glyph_route_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("old_b2.json");
    std::fs::write(&path, serde_json::to_string(&route).unwrap()).unwrap();

    let root = repo_root();
    let registry = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    let mut ctrl = MainController::new(registry, Paths::new(root));
    ctrl.load_route(&path);
    let _ = ctrl.take_signals();
    assert_eq!(ctrl.get_version(), Some("Black 2"), "the route loaded");
    assert!(ctrl.find_first_event_by_trainer_name("Pokemon Trainer Bianca (4)").is_some(), "Bianca's fight is renamed");
    assert!(ctrl.find_first_event_by_trainer_name("Clerk M Chaz (19)").is_some(), "Chaz's fight is renamed");
    assert!(ctrl.find_first_event_by_trainer_name("\u{2486}\u{2487} Trainer Bianca (4)").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
