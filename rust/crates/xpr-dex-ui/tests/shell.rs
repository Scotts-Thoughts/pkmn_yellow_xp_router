//! The page frame: tab bar, game toggle, keys, the Pokémon search overlay,
//! settings round trip. Runs with any feature set.

use egui::{Key, Modifiers, Pos2, Vec2};
use serde_json::json;
use xpr_dex_ui::testkit::DexDriver;
use xpr_dex_ui::{DexAction, DexTab, Spotlight};

fn driver(settings: serde_json::Value) -> DexDriver {
    let mut d = DexDriver::new(settings, Vec2::new(1400.0, 900.0));
    d.frame(3);
    d
}

#[test]
fn opens_on_the_saved_tab_and_species() {
    let d = driver(json!({ "tab": "Pokedex", "selected": "Pikachu", "game": "Emerald" }));
    assert_eq!(d.view.tab(), DexTab::Pokedex);
    assert_eq!(d.view.state.selected(), Some("Pikachu"));
    assert_eq!(d.view.state.game(), "Emerald");
    // the game toggle lists Pikachu's games by abbreviation
    assert!(d.shows("FRLG") && d.shows("SV"), "{:?}", d.texts);
    assert!(d.shows("Trainers"));
}

#[test]
fn bad_settings_fall_back() {
    let d = driver(json!({ "game": "Pokemon Purple", "selected": "Missingno", "list_width": 5 }));
    assert_eq!(d.view.state.selected(), Some("Bulbasaur"));
    assert_eq!(d.view.state.game(), "Red and Blue");
    assert_eq!(d.view.state.settings.list_width, 180.0);
}

#[test]
fn function_keys_switch_tabs_and_trainers_use_router_versions() {
    let mut d = driver(json!({ "game": "Crystal", "selected": "Totodile" }));
    d.key(Key::F3, Modifiers::NONE);
    assert_eq!(d.view.tab(), DexTab::Trainers);
    // Crystal's router version is picked
    assert_eq!(d.view.state.settings.version.as_deref(), Some("Crystal"));
    assert!(
        d.shows("Black 2") && d.shows("FireRed"),
        "router versions in the toggle"
    );
    d.key(Key::F9, Modifiers::NONE);
    assert_eq!(d.view.tab(), DexTab::Misc);
    d.key(Key::F1, Modifiers::NONE);
    assert_eq!(d.view.tab(), DexTab::Pokedex);
    assert!(d.view.take_settings_dirty());
}

#[test]
fn space_opens_the_pokemon_search_and_an_exact_name_picks() {
    let mut d = driver(json!({ "selected": "Bulbasaur", "game": "Yellow" }));
    d.key(Key::Space, Modifiers::NONE);
    assert_eq!(d.view.state.spotlight, Some(Spotlight::Pokemon));
    d.type_text("pikachu");
    d.frame(2);
    assert_eq!(d.view.state.spotlight, None);
    assert_eq!(d.view.state.selected(), Some("Pikachu"));
    // Escape closes without picking
    d.key(Key::Space, Modifiers::NONE);
    d.type_text("char");
    d.frame(1);
    assert!(d.shows("Charmander"));
    d.key(Key::Escape, Modifiers::NONE);
    assert_eq!(d.view.state.spotlight, None);
    assert_eq!(d.view.state.selected(), Some("Pikachu"));
}

#[test]
fn back_closes_and_settings_round_trip() {
    let mut d = driver(json!({ "selected": "Eevee", "game": "Yellow", "show_bulk": true }));
    d.click(Pos2::new(40.0, 20.0));
    assert!(d.take_actions().contains(&DexAction::Close));
    let saved = d.view.settings_json();
    let d2 = driver(saved);
    assert_eq!(d2.view.state.selected(), Some("Eevee"));
    assert!(d2.view.state.settings.show_bulk);
}

#[test]
fn ctrl_number_cycles_the_games_of_a_generation() {
    let mut d = driver(json!({ "selected": "Pikachu", "game": "Red and Blue" }));
    d.key(Key::Num3, Modifiers::COMMAND);
    assert_eq!(d.view.state.game(), "Ruby and Sapphire");
    d.key(Key::Num3, Modifiers::COMMAND);
    assert_eq!(d.view.state.game(), "Emerald");
    d.key(Key::Num3, Modifiers::COMMAND);
    assert_eq!(d.view.state.game(), "FireRed and LeafGreen");
    d.key(Key::Num3, Modifiers::COMMAND);
    assert_eq!(d.view.state.game(), "Ruby and Sapphire");
}

#[test]
fn the_search_overlay_walks_with_the_arrows_and_picks_with_enter() {
    let mut d = driver(json!({ "selected": "Bulbasaur", "game": "Yellow" }));
    d.key(Key::Space, Modifiers::NONE);
    d.type_text("pidg");
    d.frame(1);
    assert!(d.shows("Pidgey") && d.shows("Pidgeotto"));
    d.key(Key::ArrowDown, Modifiers::NONE);
    d.key(Key::Enter, Modifiers::NONE);
    d.frame(1);
    assert_eq!(d.view.state.spotlight, None);
    assert_eq!(d.view.state.selected(), Some("Pidgeotto"));
}

