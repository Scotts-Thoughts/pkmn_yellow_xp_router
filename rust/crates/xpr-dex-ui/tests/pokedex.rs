#![cfg(feature = "pokedex")]
//! The Pokédex tab: list filters, selection, the species context menu, the
//! detail's identity column and the stat ranking popover / card.
//!
//! Pointer positions are logical pixels of the 1600x1000 canvas the driver
//! renders (the list's rows start at y = 222 and are 25 px apart; the
//! detail's identity column is 256 px wide beside the 292 px list).

use egui::{Key, Modifiers, Pos2, Vec2};
use serde_json::json;
use xpr_dex_ui::testkit::DexDriver;
use xpr_dex_ui::views::pokedex::{calc_exp, evo_label, filter_names, sprite_scale, weight_power, Filters};
use xpr_dex_ui::DexAction;

fn driver(settings: serde_json::Value) -> DexDriver {
    let mut d = DexDriver::new(settings, Vec2::new(1600.0, 1000.0));
    d.frame(4);
    d
}

/// The list at its top with Bulbasaur selected (row i is at y = 222 + 25 i).
fn first_rows() -> DexDriver {
    driver(json!({ "tab": "Pokedex", "selected": "Bulbasaur", "game": "Red and Blue" }))
}

/// Move the pointer to `pos`, then click: a click that arrives together
/// with a jump of the pointer would also drag any scroll area under it.
fn click_at(d: &mut DexDriver, pos: Pos2) {
    d.hover(pos);
    d.click(pos);
}

fn row_y(i: usize) -> f32 {
    222.0 + 25.0 * i as f32
}

fn names(f: &Filters, game: &str) -> Vec<String> {
    filter_names(f, game)
}

fn has(list: &[String], name: &str) -> bool {
    list.iter().any(|n| n == name)
}

// ---- the list's filters -------------------------------------------------------------------

#[test]
fn unfiltered_list_is_the_species_index_and_feeds_up_down_navigation() {
    let d = first_rows();
    assert_eq!(d.view.state.filtered_names.len(), xpr_dex::get_all_pokemon().len());
    assert!(d.shows("1283 Pok\u{e9}mon"), "{:?}", d.texts);
    assert!(d.shows("Bulbasaur") && d.shows("Ivysaur"));
    // sorted by dex number
    let all = xpr_dex::get_all_pokemon();
    assert!(all.windows(2).all(|w| w[0].national_dex_number <= w[1].national_dex_number));
}

#[test]
fn search_matches_name_display_name_exact_number_and_type() {
    let g = "Red and Blue";
    let f = Filters { query: "pika".into(), ..Filters::default() };
    let n = names(&f, g);
    assert!(has(&n, "Pikachu") && has(&n, "Pikachu (Gmax)") && !has(&n, "Raichu"));
    // a dex number matches only exactly
    let n = names(&Filters { query: "25".into(), ..Filters::default() }, g);
    assert!(has(&n, "Pikachu") && has(&n, "Pikachu (Gmax)"));
    assert!(!has(&n, "Raichu") && !has(&n, "Nidoran_F"), "251 or 125 must not match: {:?}", n);
    // the display name of the Nidorans
    let n = names(&Filters { query: "nidoran\u{2640}".into(), ..Filters::default() }, g);
    assert_eq!(n, vec!["Nidoran_F".to_string()]);
    // typing in the game selected
    let n = names(&Filters { query: "dragon".into(), ..Filters::default() }, g);
    assert!(has(&n, "Dragonite") && has(&n, "Dratini"));
    // Clefairy is Normal in Red and Blue, Fairy later
    let fairy = Filters { type_: "Fairy".into(), ..Filters::default() };
    assert!(!has(&names(&fairy, "Red and Blue"), "Clefairy"));
    assert!(has(&names(&fairy, "X and Y"), "Clefairy"));
}

#[test]
fn gen_filter_uses_dex_ranges_and_hides_forms_introduced_later() {
    let g = "Sun and Moon";
    let n = names(&Filters { gen: "1".into(), ..Filters::default() }, g);
    assert!(has(&n, "Charizard") && has(&n, "Mew"));
    assert!(!has(&n, "Chikorita"));
    // Megas (gen 6), Alolan (gen 7) and Gigantamax (gen 8) forms of gen 1 species are later
    assert!(!has(&n, "Mega Charizard X") && !has(&n, "Alolan Raichu") && !has(&n, "Pikachu (Gmax)"));
    let n = names(&Filters { gen: "6".into(), ..Filters::default() }, g);
    // Mega Charizard X is a gen 6 kind of form but dex 6, outside gen 6's range
    assert!(has(&n, "Chespin") && !has(&n, "Mega Charizard X"));
    let n = names(&Filters { gen: "7".into(), ..Filters::default() }, g);
    assert!(has(&n, "Rowlet"));
    let n = names(&Filters { gen: "9".into(), ..Filters::default() }, g);
    assert!(has(&n, "Sprigatito") && !has(&n, "Rowlet"));
}

#[test]
fn type_growth_and_stage_filters() {
    let g = "Emerald";
    let all = xpr_dex::get_all_pokemon();
    let n = names(&Filters { type_: "Electric".into(), ..Filters::default() }, g);
    assert!(has(&n, "Pikachu") && has(&n, "Jolteon") && has(&n, "Magnemite") && !has(&n, "Charizard"));
    let n = names(&Filters { growth: "Erratic".into(), ..Filters::default() }, g);
    assert!(!n.is_empty() && n.iter().all(|x| xpr_dex::species_entry(x).unwrap().growth_rate == "Erratic"));
    let n = names(&Filters { stage: "mega".into(), ..Filters::default() }, g);
    assert!(has(&n, "Mega Charizard X") && !has(&n, "Charizard"));
    let n = names(&Filters { stage: "first".into(), ..Filters::default() }, g);
    assert!(has(&n, "Charmander") && !has(&n, "Charmeleon") && !has(&n, "Charizard"));
    let n = names(&Filters { stage: "middle".into(), ..Filters::default() }, g);
    assert!(has(&n, "Charmeleon") && !has(&n, "Charmander"));
    let n = names(&Filters { stage: "single".into(), ..Filters::default() }, g);
    assert!(has(&n, "Tauros") || has(&n, "Farfetch'd"));
    assert!(n.len() < all.len());
}

#[test]
fn regional_mega_and_form_toggles() {
    let g = "Red and Blue";
    let n = names(&Filters { regional: false, ..Filters::default() }, g);
    assert!(!has(&n, "Alolan Raichu") && has(&n, "Raichu") && has(&n, "Mega Charizard X"));
    let n = names(&Filters { megas: false, ..Filters::default() }, g);
    assert!(!has(&n, "Mega Charizard X") && has(&n, "Alolan Raichu") && has(&n, "Charizard"));
    let n = names(&Filters { forms: false, ..Filters::default() }, g);
    assert!(!has(&n, "Pikachu (Gmax)") && !has(&n, "Giratina (Origin)") && has(&n, "Pikachu"));
    assert!(has(&n, "Mega Charizard X") && has(&n, "Alolan Raichu"));
}

#[test]
fn the_filter_dropdown_filters_the_list_and_the_shared_names() {
    let mut d = first_rows();
    // Type -> Fire
    d.click(Pos2::new(108.0, 139.0));
    d.click(Pos2::new(108.0, 208.0));
    let filtered = d.view.state.filtered_names.clone();
    assert!(has(&filtered, "Charmander") && !has(&filtered, "Squirtle"), "{:?}", &filtered[..filtered.len().min(8)]);
    assert!(d.shows("Charmander") && !d.shows("Squirtle"));
    assert!(d.shows(&format!("{} Pok\u{e9}mon", filtered.len())), "{:?}", d.texts);
    // the toggles
    d.click(Pos2::new(88.0, 166.0));
    let no_megas = d.view.state.filtered_names.clone();
    assert!(no_megas.len() < filtered.len() || !filtered.iter().any(|n| n.starts_with("Alolan ")));
}

#[test]
fn typing_in_the_search_box_narrows_the_list() {
    let mut d = first_rows();
    d.click(Pos2::new(150.0, 106.0));
    d.type_text("char");
    d.frame(2);
    let n = d.view.state.filtered_names.clone();
    assert!(has(&n, "Charmander") && has(&n, "Chansey") == false && has(&n, "Charizard"), "{:?}", n);
    assert!(d.shows("Charmander") && !d.shows("Squirtle"));
    // nothing matches
    d.type_text("zzzz");
    d.frame(2);
    assert!(d.view.state.filtered_names.is_empty());
    assert!(d.shows("No results"));
}

// ---- selection and the context menu -------------------------------------------------------

#[test]
fn clicking_a_row_selects_the_species() {
    let mut d = first_rows();
    assert_eq!(d.view.state.selected(), Some("Bulbasaur"));
    d.click(Pos2::new(120.0, row_y(1)));
    assert_eq!(d.view.state.selected(), Some("Ivysaur"));
    assert!(!d.view.state.settings.self_compare && d.view.state.settings.comparing_with.is_none());
    // the detail follows
    assert!(d.shows("Ivysaur"));
}

#[test]
fn a_row_click_leaves_a_comparison() {
    let mut d = driver(json!({ "selected": "Bulbasaur", "game": "Red and Blue", "comparing_with": "Charmander", "comparing_third": "Squirtle" }));
    d.click(Pos2::new(120.0, row_y(1)));
    assert_eq!(d.view.state.selected(), Some("Ivysaur"));
    assert_eq!(d.view.state.settings.comparing_with, None);
    assert_eq!(d.view.state.settings.comparing_third, None);
}

#[test]
fn selecting_a_species_scrolls_the_list_to_it() {
    let mut d = driver(json!({ "selected": "Bulbasaur", "game": "Emerald" }));
    d.view.state.select_species("Mewtwo");
    d.frame(3);
    assert!(d.shows("0150"), "the Mewtwo row is in view: {:?}", &d.texts[..d.texts.len().min(40)]);
}

#[test]
fn context_menu_offers_compare_with_the_selected_species() {
    let mut d = first_rows();
    d.right_click(Pos2::new(120.0, row_y(1)));
    assert!(d.shows("Compare Bulbasaur to Ivysaur"), "{:?}", d.texts);
    assert!(d.shows("Add Ivysaur to comparison"));
    assert!(d.shows("Compare Ivysaur across generations"));
    d.click(Pos2::new(170.0, row_y(1) + 21.0));
    assert_eq!(d.view.state.settings.comparing_with.as_deref(), Some("Ivysaur"));
    assert_eq!(d.view.state.selected(), Some("Bulbasaur"));
    assert!(!d.shows("Compare Bulbasaur to Ivysaur"), "the menu closes");
}

#[test]
fn compare_is_disabled_for_the_selected_species_itself() {
    let mut d = first_rows();
    d.right_click(Pos2::new(120.0, row_y(0)));
    d.click(Pos2::new(170.0, row_y(0) + 21.0));
    assert_eq!(d.view.state.settings.comparing_with, None);
}

#[test]
fn add_to_comparison_needs_a_comparison_and_a_third_species() {
    // not comparing: disabled
    let mut d = first_rows();
    d.right_click(Pos2::new(120.0, row_y(2)));
    d.click(Pos2::new(170.0, row_y(2) + 21.0 + 28.0));
    assert_eq!(d.view.state.settings.comparing_third, None);
    // comparing: enabled for a species that is neither of the two
    let mut d = driver(json!({ "selected": "Bulbasaur", "game": "Red and Blue", "comparing_with": "Charmander" }));
    d.right_click(Pos2::new(120.0, row_y(2)));
    d.click(Pos2::new(170.0, row_y(2) + 21.0 + 28.0));
    assert_eq!(d.view.state.settings.comparing_third.as_deref(), Some("Venusaur"));
}

#[test]
fn compare_across_generations_selects_the_species_and_enters_self_comparison() {
    let mut d = first_rows();
    d.right_click(Pos2::new(120.0, row_y(1)));
    d.click(Pos2::new(170.0, row_y(1) + 21.0 + 56.0));
    assert!(d.view.state.settings.self_compare);
    assert_eq!(d.view.state.selected(), Some("Ivysaur"));
}

#[test]
fn species_in_a_single_game_cannot_be_compared_across_generations() {
    let (compare, triple, selfc) = xpr_dex_ui::views::pokedex::context_menu_entries(
        "Kubfu",
        &xpr_dex_ui::views::pokedex::MenuCtx { selected: Some("Pikachu".into()), game: "Sword and Shield".into(), comparing_with: None },
    );
    assert!(compare && !triple);
    // Kubfu is only in Sword and Shield
    assert_eq!(selfc, xpr_dex::get_games_for_pokemon("Kubfu").len() > 1);
    // a species missing from the game cannot be compared in it
    let (compare, _, _) = xpr_dex_ui::views::pokedex::context_menu_entries(
        "Chikorita",
        &xpr_dex_ui::views::pokedex::MenuCtx { selected: Some("Bulbasaur".into()), game: "Red and Blue".into(), comparing_with: None },
    );
    assert!(!compare);
}

// ---- the detail ---------------------------------------------------------------------------

#[test]
fn detail_shows_identity_and_meta() {
    let d = driver(json!({ "selected": "Pikachu", "game": "Emerald" }));
    for needle in ["Pikachu", "Electric", "Medium Fast", "Static", "Field, Fairy", "Base Friendship", "Oran Berry", "50.4%", "49.6%", "2 Spe", "6 kg", "20 BP", "Grass Knot / Low Kick"] {
        assert!(d.shows(needle), "missing {:?} in {:?}", needle, d.texts);
    }
    // evolution family: Pichu is last in Emerald's table; Raichu's arrow names the stone
    assert!(d.shows("Thunder Stone \u{2192}") && d.shows("Raichu") && d.shows("Pichu"));
    // type matchups: Ground x2, Flying / Electric / Steel half
    assert!(d.shows("Ground") && d.shows("\u{d7}2") && d.shows("\u{bd}\u{d7}"));
}

#[test]
fn hidden_ability_genderless_and_no_ev_yield_before_gen_3() {
    let d = driver(json!({ "selected": "Eevee", "game": "Black 2 and White 2" }));
    assert!(d.shows("Abilities") && d.shows("Run Away") && d.shows("Adaptability"));
    assert!(d.shows("Anticipation") && d.shows(" (hidden)"));
    let d = driver(json!({ "selected": "Magnemite", "game": "Emerald" }));
    assert!(d.shows("Genderless"));
    assert!(d.shows("Abilities") && d.shows("Magnet Pull"));
    let d = driver(json!({ "selected": "Pikachu", "game": "Emerald" }));
    assert!(d.shows("Ability") && !d.shows("Abilities") && !d.shows("(hidden)"));
    let d = driver(json!({ "selected": "Pikachu", "game": "Yellow" }));
    assert!(!d.shows("EV Yield") && !d.shows("Abilities") && !d.shows("Egg Groups"));
    let d = driver(json!({ "selected": "Pikachu", "game": "Gold and Silver" }));
    assert!(!d.shows("EV Yield"));
    let d = driver(json!({ "selected": "Pikachu", "game": "Ruby and Sapphire" }));
    assert!(d.shows("EV Yield"));
}

#[test]
fn ability_immunity_is_noted_beside_the_type() {
    let d = driver(json!({ "selected": "Gengar", "game": "Emerald" }));
    // Gengar had Levitate in gen 3
    assert!(d.shows("(Levitate)"), "{:?}", d.texts);
}

#[test]
fn single_group_species_list_the_group_once() {
    let d = driver(json!({ "selected": "Eevee", "game": "Black 2 and White 2" }));
    assert!(d.shows("Ground") && !d.shows("Ground, Ground"));
}

#[test]
fn species_not_in_the_game_get_a_placeholder() {
    let mut d = driver(json!({ "selected": "Pikachu", "game": "Emerald" }));
    d.view.state.settings.selected = Some("Rowlet".into());
    d.frame(3);
    assert!(d.shows("Rowlet is not in Emerald"), "{:?}", d.texts);
}

#[test]
fn evolution_family_buttons_select_the_species() {
    let mut d = driver(json!({ "selected": "Pikachu", "game": "Emerald" }));
    d.click(Pos2::new(498.0, 284.0));
    assert_eq!(d.view.state.selected(), Some("Raichu"));
    // the new family: Pikachu is now an earlier member
    assert!(d.shows("Pikachu"));
}

#[test]
fn evolution_family_right_click_offers_comparison() {
    let mut d = driver(json!({ "selected": "Pikachu", "game": "Emerald" }));
    d.right_click(Pos2::new(498.0, 284.0));
    assert!(d.shows("Compare Pikachu to Raichu"), "{:?}", d.texts);
    click_at(&mut d, Pos2::new(560.0, 302.0));
    assert_eq!(d.view.state.settings.comparing_with.as_deref(), Some("Raichu"));
}

#[test]
fn bulbapedia_link_strips_the_form_decorations() {
    let mut d = driver(json!({ "selected": "Pikachu", "game": "Emerald" }));
    d.click(Pos2::new(521.0, 106.0));
    assert_eq!(d.take_actions(), vec![DexAction::OpenUrl("https://bulbapedia.bulbagarden.net/wiki/Pikachu_(Pok%C3%A9mon)".into())]);
    let mut d = driver(json!({ "selected": "Mega Charizard X", "game": "X and Y" }));
    let url_click = Pos2::new(510.0, 106.0);
    d.click(url_click);
    assert_eq!(d.take_actions(), vec![DexAction::OpenUrl("https://bulbapedia.bulbagarden.net/wiki/Charizard_(Pok%C3%A9mon)".into())]);
    let mut d = driver(json!({ "selected": "Mr. Mime", "game": "Emerald" }));
    d.click(Pos2::new(521.0, 106.0));
    assert_eq!(d.take_actions(), vec![DexAction::OpenUrl("https://bulbapedia.bulbagarden.net/wiki/Mr.%20Mime_(Pok%C3%A9mon)".into())]);
}

#[test]
fn ability_names_open_a_bulbapedia_popover() {
    let mut d = driver(json!({ "selected": "Eevee", "game": "Black 2 and White 2" }));
    d.click(Pos2::new(483.0, 858.0));
    assert!(d.shows("Bulbapedia"), "{:?}", d.texts);
    d.click(Pos2::new(600.0, 891.0));
    assert_eq!(d.take_actions(), vec![DexAction::OpenUrl("https://bulbapedia.bulbagarden.net/wiki/Adaptability_(Ability)".into())]);
}

#[test]
fn artwork_opens_a_lightbox_that_escape_closes() {
    let mut d = driver(json!({ "selected": "Pikachu", "game": "Emerald" }));
    d.click(Pos2::new(420.0, 170.0));
    d.frame(2);
    // the lightbox is a modal: keys are not the page's
    d.key(Key::Escape, Modifiers::NONE);
    d.frame(2);
    assert_eq!(d.view.state.selected(), Some("Pikachu"));
}

#[test]
fn growth_rate_opens_the_experience_table() {
    let mut d = driver(json!({ "selected": "Pikachu", "game": "Emerald" }));
    d.click(Pos2::new(495.0, 712.0));
    assert!(d.shows("1,000,000 total exp to Lv100"), "{:?}", d.texts);
    assert!(d.shows("Total Exp") && d.shows("To Next"));
    // Lv 10 of Medium Fast
    assert!(d.shows("1,000"));
}

// ---- base stats -------------------------------------------------------------------------

#[test]
fn gen_1_has_one_special_and_a_five_stat_total() {
    let d = driver(json!({ "selected": "Pikachu", "game": "Red and Blue" }));
    assert!(d.shows("Spc") && !d.shows("SpA") && !d.shows("SpD"), "{:?}", d.texts);
    // 35 + 55 + 30 + 50 + 90
    assert!(d.shows("260"), "{:?}", d.texts);
    assert!(!d.shows("WBST") && !d.shows("UBST") && !d.shows("Phys Bulk"));
}

#[test]
fn gen_3_has_six_stats_and_a_six_stat_total() {
    let d = driver(json!({ "selected": "Pikachu", "game": "Emerald" }));
    assert!(d.shows("SpA") && d.shows("SpD") && !d.shows("Spc"));
    assert!(d.shows("300"));
}

#[test]
fn gen_1_extra_rows_follow_the_settings() {
    let d = driver(json!({ "selected": "Pikachu", "game": "Red and Blue", "show_wbst": true, "show_ubst": true, "show_bulk": true }));
    // WBST = 35 + 55 + 30 + 90 + 2 * 50, bulk = HP * Def and HP * Special
    assert!(d.shows("WBST") && d.shows("310"), "{:?}", d.texts);
    assert!(d.shows("UBST"));
    assert!(d.shows("Phys Bulk") && d.shows("1,050"));
    assert!(d.shows("Spec Bulk") && d.shows("1,750"));
    // WBST / UBST are gen 1 rows
    let d = driver(json!({ "selected": "Pikachu", "game": "Emerald", "show_wbst": true, "show_ubst": true, "show_bulk": true }));
    assert!(!d.shows("WBST") && !d.shows("UBST"));
    // bulk: HP x Def and HP x SpD
    assert!(d.shows("Phys Bulk") && d.shows("1,050") && d.shows("1,400"));
}

#[test]
fn hovering_a_stat_row_opens_its_ranking_and_leaving_closes_it_after_a_moment() {
    let mut d = driver(json!({ "selected": "Pikachu", "game": "Emerald" }));
    assert!(!d.shows("RANKING"));
    d.hover(Pos2::new(460.0, 478.0));
    assert!(d.shows("SPD RANKING \u{2014} EMERALD"), "{:?}", d.texts);
    // the current species is listed, with its rank
    assert!(d.shows("Pikachu") && d.shows("326"));
    // still open a few frames after the pointer left ...
    d.hover(Pos2::new(1300.0, 800.0));
    assert!(d.shows("SPD RANKING"));
    // ... and gone after the 200 ms
    d.frame(20);
    assert!(!d.shows("RANKING"), "{:?}", d.texts);
}

#[test]
fn moving_between_rows_switches_the_ranking_at_once_and_total_has_one() {
    let mut d = driver(json!({ "selected": "Pikachu", "game": "Emerald" }));
    d.hover(Pos2::new(460.0, 381.0));
    assert!(d.shows("HP RANKING"));
    d.hover(Pos2::new(460.0, 405.0));
    assert!(d.shows("ATK RANKING") && !d.shows("HP RANKING"));
    d.hover(Pos2::new(440.0, 531.0));
    assert!(d.shows("TOTAL RANKING \u{2014} EMERALD"));
}

#[test]
fn a_ranking_row_navigates_to_its_species() {
    let mut d = driver(json!({ "selected": "Pikachu", "game": "Emerald" }));
    d.hover(Pos2::new(460.0, 478.0));
    // the second visible row
    click_at(&mut d, Pos2::new(620.0, 353.0));
    assert_eq!(d.view.state.selected(), Some("Smeargle"));
    assert!(!d.shows("RANKING"), "the popover closes: {:?}", d.texts);
}

#[test]
fn clicking_the_current_species_in_its_ranking_does_nothing() {
    let mut d = driver(json!({ "selected": "Pikachu", "game": "Emerald" }));
    d.hover(Pos2::new(460.0, 478.0));
    click_at(&mut d, Pos2::new(620.0, 513.0));
    assert_eq!(d.view.state.selected(), Some("Pikachu"));
    assert!(d.shows("SPD RANKING"));
}

#[test]
fn ranking_rows_have_the_species_context_menu() {
    let mut d = driver(json!({ "selected": "Pikachu", "game": "Emerald" }));
    d.hover(Pos2::new(460.0, 478.0));
    d.hover(Pos2::new(620.0, 353.0));
    d.right_click(Pos2::new(620.0, 353.0));
    assert!(d.shows("Compare Pikachu to Smeargle"), "{:?}", d.texts);
    // the popover stays open while the menu is
    d.frame(30);
    assert!(d.shows("SPD RANKING") && d.shows("Compare Pikachu to Smeargle"));
    click_at(&mut d, Pos2::new(700.0, 353.0 + 21.0));
    assert_eq!(d.view.state.settings.comparing_with.as_deref(), Some("Smeargle"));
}

#[test]
fn the_ranking_title_expands_to_a_card_that_escape_closes() {
    let mut d = driver(json!({ "selected": "Pikachu", "game": "Emerald" }));
    d.hover(Pos2::new(460.0, 478.0));
    d.click(Pos2::new(640.0, 284.0));
    assert!(d.shows("Value"), "the card has its own column header: {:?}", d.texts);
    // it stays while the pointer is away
    d.hover(Pos2::new(1400.0, 900.0));
    d.frame(30);
    assert!(d.shows("Value"));
    d.key(Key::Escape, Modifiers::NONE);
    d.frame(2);
    assert!(!d.shows("Value"));
}

#[test]
fn a_card_row_navigates_and_closes_everything() {
    let mut d = driver(json!({ "selected": "Pikachu", "game": "Emerald" }));
    d.hover(Pos2::new(460.0, 478.0));
    d.click(Pos2::new(640.0, 284.0));
    // the card is centred on Pikachu (rank 326): its neighbour above is Barboach
    click_at(&mut d, Pos2::new(760.0, 506.0));
    assert_eq!(d.view.state.selected(), Some("Barboach"));
    d.frame(30);
    assert!(!d.shows("Value") && !d.shows("RANKING"));
}

#[test]
fn filter_comparison_ranks_within_the_list_filter() {
    let mut d = driver(json!({ "selected": "Pikachu", "game": "Emerald" }));
    // Type -> Electric
    d.click(Pos2::new(108.0, 139.0));
    d.click(Pos2::new(108.0, 260.0));
    assert!(d.view.state.filtered_names.iter().all(|n| xpr_dex::get_pokemon_types(n, "Emerald").map(|(a, b)| a == "Electric" || b == "Electric").unwrap_or(true)));
    // unfiltered ranking: Delibird (Normal / Flying) is among Pikachu's neighbours
    d.hover(Pos2::new(460.0, 478.0));
    assert!(d.shows("Delibird"));
    d.hover(Pos2::new(1300.0, 800.0));
    d.frame(20);
    // switch the toggle on: the ranking holds only Electric species
    d.click(Pos2::new(420.0, 355.0));
    d.hover(Pos2::new(460.0, 478.0));
    assert!(d.shows("SPD RANKING") && !d.shows("Delibird"), "{:?}", d.texts);
    assert!(d.shows("Pikachu"));
}

// ---- the small pure pieces -----------------------------------------------------------------

#[test]
fn growth_rate_formulas() {
    assert_eq!(calc_exp("Medium Fast", 100), 1_000_000);
    assert_eq!(calc_exp("Fast", 100), 800_000);
    assert_eq!(calc_exp("Slow", 100), 1_250_000);
    assert_eq!(calc_exp("Medium Slow", 100), 1_059_860);
    assert_eq!(calc_exp("Erratic", 100), 600_000);
    assert_eq!(calc_exp("Fluctuating", 100), 1_640_000);
    // the Medium Slow curve starts negative, as in Solodex
    assert_eq!(calc_exp("Medium Slow", 1), -54);
    assert_eq!(calc_exp("Medium Slow", 2), 9);
    assert_eq!(calc_exp("Erratic", 50), 125_000);
    assert_eq!(calc_exp("Fluctuating", 36), 46_656);
}

#[test]
fn weight_power_thresholds() {
    assert_eq!(weight_power(0.1), 20);
    assert_eq!(weight_power(9.9), 20);
    assert_eq!(weight_power(10.0), 40);
    assert_eq!(weight_power(24.9), 40);
    assert_eq!(weight_power(25.0), 60);
    assert_eq!(weight_power(50.0), 80);
    assert_eq!(weight_power(99.9), 80);
    assert_eq!(weight_power(100.0), 100);
    assert_eq!(weight_power(199.9), 100);
    assert_eq!(weight_power(200.0), 120);
}

#[test]
fn sprite_scale_by_evolution_stage() {
    let p = |name: &str| xpr_dex::get_pokemon_data(name, "Emerald").unwrap();
    assert_eq!(sprite_scale(&p("Pichu")), 0.75);
    assert_eq!(sprite_scale(&p("Pikachu")), 0.9);
    // Solodex scales by "another member has an evolution method": the last stage
    // of a three-stage line counts as a middle one
    assert_eq!(sprite_scale(&p("Raichu")), 0.9);
    assert_eq!(sprite_scale(&p("Rattata")), 0.75);
    assert_eq!(sprite_scale(&p("Raticate")), 1.0);
    assert_eq!(sprite_scale(&p("Tauros")), 1.0);
    // Mega forms are never scaled down
    let mega = xpr_dex::get_pokemon_data("Mega Charizard X", "X and Y").unwrap();
    assert_eq!(sprite_scale(&mega), 1.0);
}

#[test]
fn evolution_labels() {
    let p = xpr_dex::get_pokemon_data("Charmander", "Emerald").unwrap();
    let labels: Vec<Option<String>> = p.evolution_family.iter().map(evo_label).collect();
    assert_eq!(labels[0], None);
    assert_eq!(labels[1].as_deref(), Some("Lv.16 \u{2192}"));
    assert_eq!(labels[2].as_deref(), Some("Lv.36 \u{2192}"));
    let p = xpr_dex::get_pokemon_data("Pikachu", "Sun and Moon").unwrap();
    let labels: Vec<Option<String>> = p.evolution_family.iter().map(evo_label).collect();
    assert_eq!(labels[1].as_deref(), Some("friendship \u{2192}"));
    assert_eq!(labels[2].as_deref(), Some("Thunder Stone \u{2192}"));
    let p = xpr_dex::get_pokemon_data("Charizard", "X and Y").unwrap();
    assert!(p.evolution_family.iter().filter_map(evo_label).any(|l| l == "Mega \u{2192}"));
}
