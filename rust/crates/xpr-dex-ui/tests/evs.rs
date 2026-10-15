#![cfg(feature = "evs")]
//! The EVs tab: the table, its filters and sorting, the encounter popover,
//! and the wheel forwarding (which must respect dialogs).

use egui::{Event, Key, Modifiers, Pos2, Vec2};
use serde_json::json;
use xpr_dex_ui::testkit::DexDriver;
use xpr_dex_ui::DexTab;

const W: f32 = 1400.0;
/// Canvas positions of the 1400 px wide page (see `dex_png` PNGs).
const HEAD_Y: f32 = 176.0;
const ROW0_Y: f32 = 211.0;
const ROW_H: f32 = 40.0;
const FILTER_Y: f32 = 128.0;

fn driver(game: &str, size: Vec2) -> DexDriver {
    let mut d = DexDriver::new(
        json!({ "tab": "Evs", "game": game, "selected": "Pikachu" }),
        size,
    );
    d.frame(4);
    d
}

fn table_left(w: f32) -> f32 {
    w / 2.0 - 183.0
}

/// Centre x of stat column `i` (0 = HP) of a six-stat table.
fn stat_x(w: f32, i: usize) -> f32 {
    table_left(w) + 30.0 + 36.0 + 98.0 + 32.0 * i as f32 + 16.0
}

fn name_pos(w: f32, row: usize) -> Pos2 {
    Pos2::new(
        table_left(w) + 30.0 + 36.0 + 20.0,
        ROW0_Y + ROW_H * row as f32,
    )
}

/// Pick option `n` (0 = All) of the filter whose box is centred at `x`.
fn pick_filter(d: &mut DexDriver, x: f32, n: usize) {
    d.click(Pos2::new(x, FILTER_Y));
    d.click(Pos2::new(x, 160.0 + 24.0 * n as f32));
}

#[test]
fn lists_every_species_with_its_yields() {
    let d = driver("Emerald", Vec2::new(W, 900.0));
    for t in [
        "Generation",
        "Type",
        "Stat",
        "EV yield",
        "Bulbasaur",
        "Pok\u{e9}mon",
        "HP",
        "SpA",
        "SpD",
        "0001",
    ] {
        assert!(d.shows(t), "{} missing in {:?}", t, d.texts);
    }
    assert!(!d.shows("Spc"));
    assert_eq!(d.view.tab(), DexTab::Evs);
}

#[test]
fn gen_1_games_have_one_special_column() {
    let d = driver("Yellow", Vec2::new(W, 900.0));
    assert!(d.shows("Spc"), "{:?}", d.texts);
    assert!(!d.shows("SpA") && !d.shows("SpD"));
    // gen 1 yields are the species' base stats (Bulbasaur: 45 / 49 / 49 / 65 / 45)
    assert!(d.shows("65"));
}

#[test]
fn stat_header_sorts_high_first_then_flips() {
    let mut d = driver("Emerald", Vec2::new(W, 900.0));
    // SpA header: highest first, ties by dex number (Charizard #6 is the first 3)
    d.click(Pos2::new(stat_x(W, 3), HEAD_Y));
    assert!(
        d.shows("Charizard") && !d.shows("Bulbasaur"),
        "{:?}",
        d.texts
    );
    assert!(d.shows("\u{2193}"), "descending arrow");
    // again: low first, so the zero-yield species lead (Metapod is the first dex-wise)
    d.click(Pos2::new(stat_x(W, 3), HEAD_Y));
    assert!(
        d.shows("\u{2191}") && d.shows("Charmander") && !d.shows("Charizard"),
        "{:?}",
        d.texts
    );
    // the dex header goes back to dex order, ascending
    d.click(Pos2::new(table_left(W) + 15.0, HEAD_Y));
    assert!(d.shows("Bulbasaur"), "{:?}", d.texts);
    // and flips it
    d.click(Pos2::new(table_left(W) + 15.0, HEAD_Y));
    assert!(
        !d.shows("Bulbasaur"),
        "descending dex order starts at the last species"
    );
    assert!(d.shows("\u{2193}"));
}

#[test]
fn filters_narrow_the_table() {
    let mut d = driver("Emerald", Vec2::new(W, 900.0));
    let gen_x = W / 2.0 - 800.0 + 671.0;
    // Generation 2: Chikorita is the first
    pick_filter(&mut d, gen_x, 2);
    assert!(
        d.shows("Chikorita") && !d.shows("Bulbasaur"),
        "{:?}",
        d.texts
    );
    // Gen 4+ do not exist in Emerald; their options are disabled and a click does nothing
    pick_filter(&mut d, gen_x, 4);
    assert!(d.shows("Chikorita"), "still Gen 2");
    d.key(Key::Escape, Modifiers::NONE);

    // Stat = Spe with EV yield = 3: only species yielding 3 Speed
    let mut d = driver("Emerald", Vec2::new(W, 900.0));
    let stat_f = W / 2.0 - 800.0 + 861.0;
    let amount_f = W / 2.0 - 800.0 + 936.0;
    pick_filter(&mut d, stat_f, 6);
    assert!(d.shows("Rattata"), "any Speed yield: {:?}", d.texts);
    pick_filter(&mut d, amount_f, 6);
    assert!(
        d.shows("Pidgeot") && !d.shows("Rattata"),
        "exactly 3: {:?}",
        d.texts
    );
}

#[test]
fn row_click_opens_the_species_in_the_pokedex() {
    let mut d = driver("Emerald", Vec2::new(W, 900.0));
    d.click(name_pos(W, 3));
    assert_eq!(d.view.tab(), DexTab::Pokedex);
    assert_eq!(d.view.state.selected(), Some("Charmander"));
    assert_eq!(d.view.state.game(), "Emerald");
    assert!(d.view.take_settings_dirty());
}

/// The first row (within the first `max` of the Gen 3 table) with encounters.
fn first_row_with_encounters(game: &str, max: usize) -> (usize, String) {
    let list: Vec<_> = xpr_dex::get_all_pokemon_for_game(game)
        .into_iter()
        .filter(|p| (252..=386).contains(&p.national_dex_number))
        .collect();
    list.iter()
        .take(max)
        .enumerate()
        .find(|(_, p)| !xpr_dex::get_encounters_for_pokemon(game, &p.species).is_empty())
        .map(|(i, p)| (i, p.species.clone()))
        .expect("a Gen 3 species with Emerald encounters in the first rows")
}

#[test]
fn hovering_a_name_lists_where_the_species_is_found() {
    let mut d = driver("Emerald", Vec2::new(W, 1100.0));
    let gen_x = W / 2.0 - 800.0 + 671.0;
    pick_filter(&mut d, gen_x, 3);
    let (row, species) = first_row_with_encounters("Emerald", 14);
    let groups = xpr_dex::get_encounters_for_pokemon("Emerald", &species);
    d.hover(name_pos(W, row));
    assert!(d.shows("Encounter locations in this game"), "{:?}", d.texts);
    assert!(
        d.shows(&groups[0].location),
        "{} not in {:?}",
        groups[0].location,
        d.texts
    );
    assert!(d.shows(&groups[0].method), "{:?}", d.texts);
    assert!(d.shows("Lv "));
    // the pointer can travel onto the popover without it closing
    let p = name_pos(W, row);
    d.hover(Pos2::new(p.x + 260.0, p.y + 30.0));
    d.frame(12);
    assert!(
        d.shows("Encounter locations in this game"),
        "stays while hovered"
    );
    // and it goes away shortly after the pointer leaves
    d.hover(Pos2::new(120.0, 600.0));
    d.frame(12);
    assert!(
        !d.shows("Encounter locations in this game"),
        "{:?}",
        d.texts
    );
}

#[test]
fn species_without_encounters_and_games_without_tables_show_no_popover() {
    // Bulbasaur is not found in the wild in Emerald
    let mut d = driver("Emerald", Vec2::new(W, 900.0));
    d.hover(name_pos(W, 0));
    assert!(!d.shows("Encounter locations in this game"));
    // Scarlet and Violet have no encounter tables
    let mut d = driver("Scarlet and Violet", Vec2::new(W, 900.0));
    d.hover(name_pos(W, 0));
    assert!(!d.shows("Encounter locations in this game"));
    d.hover(name_pos(W, 1));
    assert!(!d.shows("Encounter locations in this game"));
}

fn wheel(d: &mut DexDriver, at: Pos2, dy: f32) {
    d.frame_with(
        2,
        vec![
            Event::PointerMoved(at),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, dy),
                modifiers: Modifiers::NONE,
            },
        ],
    );
    d.frame(20);
}

#[test]
fn the_wheel_scrolls_the_table_from_anywhere_in_the_body() {
    let mut d = driver("Emerald", Vec2::new(W, 900.0));
    assert!(d.shows("Bulbasaur"));
    // left of the table, over the empty page
    wheel(&mut d, Pos2::new(150.0, 500.0), -900.0);
    assert!(!d.shows("Bulbasaur"), "scrolled: {:?}", d.texts);
}

#[test]
fn the_wheel_stands_down_under_a_dialog() {
    let mut d = driver("Emerald", Vec2::new(W, 900.0));
    // Shift+Space opens the trainer search, a modal
    d.key(Key::Space, Modifiers::SHIFT);
    d.frame(3);
    assert!(d.view.state.spotlight.is_some());
    wheel(&mut d, Pos2::new(150.0, 500.0), -900.0);
    assert!(
        d.shows("Bulbasaur"),
        "the table must not scroll behind the dialog: {:?}",
        d.texts
    );
    // closing it restores the wheel
    d.key(Key::Escape, Modifiers::NONE);
    d.frame(3);
    wheel(&mut d, Pos2::new(150.0, 500.0), -900.0);
    assert!(!d.shows("Bulbasaur"));
}

#[test]
fn filters_reset_when_the_game_cannot_satisfy_them() {
    let mut d = driver("Emerald", Vec2::new(W, 900.0));
    let gen_x = W / 2.0 - 800.0 + 671.0;
    pick_filter(&mut d, gen_x, 3);
    assert!(d.shows("Treecko"));
    // Red and Blue have no Gen 3 species: the filter falls back to All
    d.view.state.set_game("Red and Blue");
    d.frame(3);
    assert!(d.shows("Bulbasaur"), "{:?}", d.texts);
}
