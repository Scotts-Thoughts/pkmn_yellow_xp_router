#![cfg(feature = "evs")]
//! The EVs tab: the table, its filters and sorting, the encounter popover,
//! the wheel forwarding (which must respect dialogs) and the spread card.

use std::collections::HashSet;
use std::path::PathBuf;

use egui::{Event, Key, Modifiers, Pos2, Rect, Vec2};
use serde_json::json;
use xpr_dex::stats::{
    calc_gen12_stats, calc_gen3_stats, nature_mods_by_name, Gen12Dvs, Gen12StatExps, Gen3Spread,
};
use xpr_dex_ui::testkit::{test_theme, DexDriver};
use xpr_dex_ui::views::evs::spread_card::{
    all_badge_ids, show_spread_card, spread_card_height, SpreadCardProps,
};
use xpr_dex_ui::{DexImages, DexTab};
use xpr_ui_kit::offscreen::Offscreen;

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

// ---------------------------------------------------------------------------
// spread card
// ---------------------------------------------------------------------------

fn png_dir() -> PathBuf {
    let dir = std::env::var_os("XPR_EVS_PNG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| xpr_dex_ui::testkit::repo_root().join("rust/target/xpr-scratch/evs"));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn gen3_props(
    species: &str,
    game: &str,
    level: i32,
    nature: &str,
    evs: Gen3Spread,
    held: Option<&str>,
) -> SpreadCardProps {
    let data = xpr_dex::get_pokemon_data(species, game).unwrap();
    let mods = nature_mods_by_name(nature);
    let ivs = Gen3Spread {
        hp: 31,
        attack: 31,
        defense: 31,
        spattack: 31,
        spdefense: 31,
        speed: 31,
    };
    let stats = calc_gen3_stats(&data.base_stats, level, &ivs, &evs, &mods, Some(species));
    SpreadCardProps {
        species: species.to_string(),
        dex_number: data.national_dex_number,
        type1: data.type_1.clone(),
        type2: data.type_2.clone(),
        game: game.to_string(),
        gen: xpr_dex::game_gen(game),
        level,
        base_stats: data.base_stats,
        stats,
        ivs,
        evs,
        dvs: Gen12Dvs::default(),
        stat_exps: Gen12StatExps::default(),
        nature_name: nature.to_string(),
        nature_mods: mods,
        held_item_name: held.map(str::to_string),
        badges: all_badge_ids(game),
        stats_locked: false,
    }
}

fn gen1_props(species: &str, game: &str, level: i32, exps: Gen12StatExps) -> SpreadCardProps {
    let data = xpr_dex::get_pokemon_data(species, game).unwrap();
    let dvs = Gen12Dvs {
        attack: 15,
        defense: 14,
        speed: 15,
        special: 12,
    };
    let stats = calc_gen12_stats(&data.base_stats, level, &dvs, &exps);
    SpreadCardProps {
        species: species.to_string(),
        dex_number: data.national_dex_number,
        type1: data.type_1.clone(),
        type2: data.type_2.clone(),
        game: game.to_string(),
        gen: xpr_dex::game_gen(game),
        level,
        base_stats: data.base_stats,
        stats,
        ivs: Gen3Spread::MAX_IVS,
        evs: Gen3Spread::ZERO,
        dvs,
        stat_exps: exps,
        nature_name: "Hardy".into(),
        nature_mods: nature_mods_by_name("Hardy"),
        held_item_name: None,
        badges: HashSet::new(),
        stats_locked: false,
    }
}

/// Draw a card on a canvas and return the drawn texts; saves the PNG as `name`.
fn render_card(props: &SpreadCardProps, width: f32, name: &str) -> Vec<String> {
    let theme = test_theme();
    let mut off = Offscreen::new(&theme, 1.0, 8192);
    let mut images = DexImages::new();
    let h = spread_card_height(props, width);
    let size = Vec2::new(width + 40.0, h + 40.0);
    let prims = off.render(size, 3, |ctx| {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme.bg))
            .show(ctx, |ui| {
                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    show_spread_card(ui, &theme, &mut images, props, width);
                });
            });
    });
    off.rasterize(&prims, Rect::from_min_size(Pos2::ZERO, size))
        .unwrap()
        .save(&png_dir().join(name))
        .unwrap();
    let mut texts = Vec::new();
    for cs in off.last_shapes() {
        collect(&cs.shape, &mut texts);
    }
    texts
}

fn collect(shape: &egui::Shape, out: &mut Vec<String>) {
    match shape {
        egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
        egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
        _ => {}
    }
}

#[test]
fn spread_card_gen_3_plus_shows_ivs_evs_nature_and_the_footer() {
    let evs = Gen3Spread {
        hp: 252,
        attack: 0,
        defense: 4,
        spattack: 0,
        spdefense: 0,
        speed: 252,
    };
    let p = gen3_props("Swampert", "Emerald", 50, "Adamant", evs, Some("Leftovers"));
    let t = render_card(&p, 1120.0, "spread_swampert.png");
    for needle in [
        "Swampert",
        "#0260",
        "LEVEL",
        "NATURE",
        "Adamant",
        "+Atk",
        "\u{2212}SpA",
        "IV",
        "EV",
        "STAT",
        "EVs 508/510",
        "Leftovers",
        "EV gain",
        "Emerald",
        "Lv 50",
        "incl. badge boosts",
    ] {
        assert!(
            t.iter().any(|s| s.contains(needle)),
            "{} missing in {:?}",
            needle,
            t
        );
    }
    assert!(t.iter().any(|s| s == "252") && t.iter().any(|s| s == "31"));
    // the card height follows the width
    assert!((spread_card_height(&p, 560.0) * 2.0 - spread_card_height(&p, 1120.0)).abs() < 0.01);
}

#[test]
fn spread_card_form_names_split_onto_their_own_line() {
    let evs = Gen3Spread {
        hp: 0,
        attack: 252,
        defense: 0,
        spattack: 0,
        spdefense: 4,
        speed: 252,
    };
    let p = gen3_props("Giratina (Origin)", "Platinum", 78, "Jolly", evs, None);
    let t = render_card(&p, 1120.0, "spread_giratina_origin.png");
    assert!(
        t.iter().any(|s| s == "Giratina") && t.iter().any(|s| s == "(Origin)"),
        "{:?}",
        t
    );
    // no badge boosts outside gens 1-3
    assert!(!t.iter().any(|s| s.contains("badge")));
}

#[test]
fn spread_card_gens_1_and_2_use_dvs_and_stat_exp() {
    let exps = Gen12StatExps {
        hp: 65535,
        attack: 12000,
        defense: 0,
        speed: 65535,
        special: 40000,
    };
    let p = gen1_props("Alakazam", "Yellow", 55, exps);
    let t = render_card(&p, 1120.0, "spread_alakazam_gen1.png");
    for needle in [
        "HP DV",
        "/15",
        "DV",
        "EXP",
        "SPC",
        "Stat Exp gain",
        "Yellow",
        "DVs ",
    ] {
        assert!(
            t.iter().any(|s| s.contains(needle)),
            "{} missing in {:?}",
            needle,
            t
        );
    }
    // gen 1 has one Special row and no nature
    assert!(
        !t.iter().any(|s| s == "SPA" || s == "SPD" || s == "NATURE"),
        "{:?}",
        t
    );
    // five rows: HP, ATK, DEF, SPC, SPE
    let labels: Vec<&str> = t
        .iter()
        .map(String::as_str)
        .filter(|s| ["HP", "ATK", "DEF", "SPC", "SPE", "SPA", "SPD"].contains(s))
        .collect();
    assert_eq!(labels, ["HP", "ATK", "DEF", "SPC", "SPE"]);
}

#[test]
fn spread_card_without_investment_has_no_gain_legend_and_neutral_nature() {
    let p = gen3_props("Pikachu", "Emerald", 30, "Hardy", Gen3Spread::ZERO, None);
    let t = render_card(&p, 800.0, "spread_pikachu_neutral.png");
    assert!(!t.iter().any(|s| s.contains("EV gain")), "{:?}", t);
    assert!(t.iter().any(|s| s == "neutral"));
    // hand-edited stats drop the gain tail and say so
    let mut locked = gen3_props(
        "Pikachu",
        "Emerald",
        30,
        "Modest",
        Gen3Spread {
            hp: 0,
            attack: 0,
            defense: 0,
            spattack: 252,
            spdefense: 0,
            speed: 0,
        },
        None,
    );
    locked.stats_locked = true;
    let t = render_card(&locked, 800.0, "spread_pikachu_locked.png");
    assert!(
        t.iter().any(|s| s.contains("stats set manually"))
            && !t.iter().any(|s| s.contains("EV gain")),
        "{:?}",
        t
    );
}
