//! The packaged binary needs nothing next to it: with a `raw_pkmn_data`
//! directory that does not exist, every built-in version still loads from the
//! embedded pack, min-battles presets are listed and load as base routes,
//! and a custom gen is created from the embedded files.

use std::path::PathBuf;
use std::sync::Arc;

use xpr_core::consts;
use xpr_data::Registry;
use xpr_engine::Router;

fn missing_dir() -> PathBuf {
    std::env::temp_dir().join("xpr-embedded-test").join("no-such-raw_pkmn_data")
}

#[test]
fn all_versions_load_without_data_on_disk() {
    assert!(xpr_data::embedded::is_embedded());
    let registry = Registry::new(missing_dir(), PathBuf::new());
    let errors = registry.load_all_builtin();
    assert!(errors.is_empty(), "{:?}", errors);
}

#[test]
fn min_battles_presets_are_listed_and_loadable() {
    let registry = Arc::new(Registry::new(missing_dir(), PathBuf::new()));
    let expected = [
        (consts::YELLOW_VERSION, vec!["Flareon", "Jolteon", "Vaporeon"]),
        (consts::RED_VERSION, vec!["Bulbasaur", "Charmander", "Squirtle"]),
        (consts::CRYSTAL_VERSION, vec!["Chikorita", "Cyndaquil", "Totodile"]),
        (consts::GOLD_VERSION, vec!["Chikorita", "Cyndaquil", "Totodile"]),
        (consts::EMERALD_VERSION, vec![]),
    ];
    for (version, presets) in expected {
        let gen = registry.get_version(version).unwrap();
        assert_eq!(gen.min_battles_db().data, presets, "{}", version);
        for preset in presets {
            let preset_path = gen.min_battles_db().get_dir().join(format!("{}.json", preset));
            assert!(registry.is_embedded_file(&preset_path));
            let mut router = Router::new(registry.clone());
            router.new_route(preset, Some(&preset_path), version, None, None, None).unwrap();
            assert!(!router.all_groups().is_empty(), "{} preset {} has no events", version, preset);
        }
    }
}

#[test]
fn custom_gen_is_created_from_embedded_files() {
    let custom_dir = std::env::temp_dir().join("xpr-embedded-test").join(format!("custom_gens_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&custom_dir);
    std::fs::create_dir_all(&custom_dir).unwrap();
    let registry = Registry::new(missing_dir(), custom_dir.clone());
    let folder = registry.create_custom_version(consts::PLATINUM_VERSION, "Embedded Test").unwrap();
    let mut names: Vec<String> = std::fs::read_dir(&folder).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
    names.sort();
    assert_eq!(names, ["custom_gen.json", "fights_info.json", "items.json", "moves.json", "pokemon.json", "trainers.json", "type_info.json"]);
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../raw_pkmn_data/gen_four");
    assert_eq!(std::fs::read(folder.join("moves.json")).unwrap(), std::fs::read(repo.join("moves.json")).unwrap());
    assert_eq!(std::fs::read(folder.join("pokemon.json")).unwrap(), std::fs::read(repo.join("platinum/pokemon.json")).unwrap());
    registry.reload_all_custom_gens().unwrap();
    assert!(registry.get_version("Embedded Test").is_ok());
    let _ = std::fs::remove_dir_all(&custom_dir);
}
