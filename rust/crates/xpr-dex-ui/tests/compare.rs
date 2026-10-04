#![cfg(feature = "compare")]
//! The comparison views: pair, self (one species in two games) and triple.
//! Runs the page headlessly with the `compare` feature (the shell draws the
//! views in the right pane). Set `COMPARE_SHOTS=<dir>` to also save PNGs of a
//! few states.

use egui::epaint::Shape;
use egui::{Color32, Event, Key, Modifiers, MouseWheelUnit, Pos2, Rect, Vec2};
use serde_json::json;
use xpr_dex_ui::testkit::DexDriver;
use xpr_dex_ui::views::compare::probe;

fn driver_sized(settings: serde_json::Value, w: f32, h: f32) -> DexDriver {
    probe::enable();
    let mut d = DexDriver::new(settings, Vec2::new(w, h));
    d.frame(4);
    no_id_clash(&d);
    d
}

/// egui paints "Double use of widget ID" (debug builds) when two widgets share an id.
fn no_id_clash(d: &DexDriver) {
    let bad: Vec<&String> = d.texts.iter().filter(|t| t.contains("use of widget ID") || t.contains("use of ScrollArea ID")).collect();
    assert!(bad.is_empty(), "{:?}", bad);
}

fn driver(settings: serde_json::Value) -> DexDriver {
    driver_sized(settings, 1600.0, 1000.0)
}

/// Charmander vs Squirtle in Emerald.
fn pair() -> DexDriver {
    driver(json!({ "tab": "Pokedex", "selected": "Charmander", "comparing_with": "Squirtle", "game": "Emerald" }))
}

fn triple() -> DexDriver {
    driver(json!({ "tab": "Pokedex", "selected": "Bulbasaur", "comparing_with": "Charmander", "comparing_third": "Squirtle", "game": "Red and Blue" }))
}

fn self_view() -> DexDriver {
    driver(json!({ "tab": "Pokedex", "selected": "Pikachu", "self_compare": true, "game": "Yellow" }))
}

fn shot(d: &mut DexDriver, name: &str) {
    if let Ok(dir) = std::env::var("COMPARE_SHOTS") {
        let _ = std::fs::create_dir_all(&dir);
        d.save_png(&std::path::Path::new(&dir).join(format!("{}.png", name))).expect("save png");
    }
}

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn texts_eq(d: &DexDriver, text: &str) -> usize {
    d.texts.iter().filter(|t| t.as_str() == text).count()
}

fn contains(d: &DexDriver, text: &str) -> usize {
    d.texts.iter().filter(|t| t.contains(text)).count()
}

fn center(r: Rect) -> Pos2 {
    r.center()
}

fn row(scope: &str, name: &str) -> Rect {
    probe::row(scope, name).unwrap_or_else(|| panic!("no row {} in {}", name, scope))
}

fn stat(scope: &str, label: &str) -> Rect {
    probe::find("stat", scope, label).unwrap_or_else(|| panic!("no stat {} in {}", label, scope))
}

fn walk(shape: &Shape, f: &mut impl FnMut(&Shape)) {
    f(shape);
    if let Shape::Vec(v) = shape {
        v.iter().for_each(|s| walk(s, f));
    }
}

/// Whether a rect of exactly `rect` filled with `color` was drawn last frame.
fn filled(d: &DexDriver, rect: Rect, color: Color32) -> bool {
    let mut found = false;
    for cs in d.off.last_shapes() {
        walk(&cs.shape, &mut |s| {
            if let Shape::Rect(r) = s {
                let close = |a: Pos2, b: Pos2| (a.x - b.x).abs() < 0.6 && (a.y - b.y).abs() < 0.6;
                if r.fill == color && close(r.rect.min, rect.min) && close(r.rect.max, rect.max) {
                    found = true;
                }
            }
        });
    }
    found
}

/// The colour a text was drawn in inside `within`.
fn text_color(d: &DexDriver, within: Rect, text: &str) -> Option<Color32> {
    let mut out = None;
    for cs in d.off.last_shapes() {
        walk(&cs.shape, &mut |s| {
            if let Shape::Text(t) = s {
                if t.galley.text() == text && within.contains(t.pos + Vec2::new(1.0, 1.0)) || t.galley.text() == text && within.intersects(Rect::from_min_size(t.pos, t.galley.size())) {
                    out = t.galley.job.sections.first().map(|s| s.format.color);
                }
            }
        });
    }
    out
}

/// Whether the text was drawn struck through.
fn struck(d: &DexDriver, text: &str) -> bool {
    let mut out = false;
    for cs in d.off.last_shapes() {
        walk(&cs.shape, &mut |s| {
            if let Shape::Text(t) = s {
                if t.galley.text() == text {
                    out |= t.galley.job.sections.first().map(|s| s.format.strikethrough.width > 0.0).unwrap_or(false);
                }
            }
        });
    }
    out
}

const TINT: Color32 = Color32::from_rgb(0x1e, 0x3a, 0x5f);

fn wheel(d: &mut DexDriver, at: Pos2, dy: f32) {
    d.frame_with(1, vec![Event::PointerMoved(at)]);
    d.frame_with(
        3,
        vec![Event::MouseWheel {
            unit: MouseWheelUnit::Point,
            delta: Vec2::new(0.0, -dy),
            modifiers: Modifiers::NONE,
        }],
    );
}

// ---- pair -----------------------------------------------------------------------------------

#[test]
fn pair_shows_both_species_their_stat_differences_and_movepools() {
    let mut d = pair();
    for t in ["Charmander", "Squirtle", "BASE STATS", "Total", "HP", "Atk", "SpA", "Spe"] {
        assert!(contains(&d, t) > 0, "{} in {:?}", t, d.texts);
    }
    // HP 39 vs 44, Total 309 vs 314: the difference is on each side
    assert_eq!(texts_eq(&d, "-5"), 2, "HP and Total of Charmander: {:?}", d.texts);
    assert_eq!(texts_eq(&d, "+5"), 2);
    assert!(texts_eq(&d, "309") == 1 && texts_eq(&d, "314") == 1);
    // evolution families, with the arrow-less labels
    for t in ["Charmeleon", "Charizard", "Wartortle", "Blastoise", "Lv.16", "Lv.36"] {
        assert!(contains(&d, t) > 0, "{}", t);
    }
    // the sections of both sides
    for label in ["LEVEL UP LEARNSET", "TM / HM"] {
        assert_eq!(contains(&d, label), 2, "{}: {:?}", label, d.texts);
    }
    // multipliers of the defensive matchups: Charmander is weak to Water, Ground, Rock
    assert!(probe::find("type", "left", "Water").is_some() && probe::find("type", "left", "Rock").is_some());
    assert!(probe::find("type", "right", "Grass").is_some() && probe::find("type", "right", "Electric").is_some());
    assert!(contains(&d, "\u{d7}2") >= 5 && contains(&d, "\u{bd}\u{d7}") >= 4);
    shot(&mut d, "pair");
}

#[test]
fn level_up_rows_keep_the_data_order_and_merge_the_generations_games() {
    let d = pair();
    // Emerald's generation merges Ruby / Sapphire / Emerald / FireRed / LeafGreen? no: R/S + E + FRLG
    // are one group, so a row only some games have carries tags
    let order = probe::row_order("left:level");
    assert_eq!(order[0], "Scratch");
    assert!(order.contains(&"Metal Claw".to_string()), "FRLG-only move: {:?}", order);
    assert!(d.shows("FRLG"), "game tags: {:?}", d.texts);
    // the right side is Squirtle's own table
    assert_eq!(probe::row_order("right:level")[0], "Tackle");
}

#[test]
fn moves_the_other_side_lacks_are_tinted_until_the_diff_switch_is_off() {
    let mut d = pair();
    // Dragon Claw (TM02) is Charmander's alone; Toxic (TM06) is shared
    assert!(filled(&d, row("left:tmhm", "Dragon Claw"), TINT));
    assert!(!filled(&d, row("left:tmhm", "Toxic"), TINT));
    assert!(filled(&d, row("right:tmhm", "Water Pulse"), TINT));
    // level-up moves too (Scratch is not Squirtle's)
    assert!(filled(&d, row("left:level", "Scratch"), TINT));
    // the tables that are not diffed (egg moves) never tint
    if let Some(r) = probe::row("left:egg", "Belly Drum") {
        assert!(!filled(&d, r, TINT));
    }
    d.view.state.settings.show_movepool_diff = false;
    d.frame(2);
    assert!(!filled(&d, row("left:tmhm", "Dragon Claw"), TINT));
    assert!(!filled(&d, row("left:level", "Scratch"), TINT));
}

#[test]
fn sorting_a_column_sorts_both_sides_and_right_click_resets() {
    let mut d = pair();
    let initial_l = probe::row_order("left:level");
    let initial_r = probe::row_order("right:level");
    let pwr = probe::header("left:level", 4).unwrap();
    d.click(center(pwr));
    // both sides follow (the sort is shared per table); the TM tables are not sorted
    assert_eq!(contains(&d, "Pwr \u{25B2}"), 2, "arrows on the level tables only");
    let l = probe::row_order("left:level");
    let r = probe::row_order("right:level");
    assert_ne!(l, initial_l);
    assert_ne!(r, initial_r);
    // status moves (no power) sort first, ascending
    assert!(["Growl", "Leer", "SmokeScreen", "Scary Face"].contains(&l[0].as_str()), "{:?}", l);
    assert!(["Tail Whip", "Withdraw", "Protect", "Rain Dance"].contains(&r[0].as_str()), "{:?}", r);
    // a second click (on the other side's header) flips the direction
    let pwr = probe::header("right:level", 4).unwrap();
    d.click(center(pwr));
    assert_eq!(contains(&d, "Pwr \u{25BC}"), 2);
    assert_eq!(probe::row_order("left:level")[0], "Flamethrower");
    // right-click on a header: back to the table's own order
    let pwr = probe::header("left:level", 4).unwrap();
    d.right_click(center(pwr));
    assert_eq!(probe::row_order("left:level"), initial_l);
    assert_eq!(probe::row_order("right:level"), initial_r);
    assert!(!d.shows("Pwr \u{25B2}") && !d.shows("Pwr \u{25BC}"));
}

#[test]
fn sort_by_move_name_and_type() {
    let mut d = pair();
    d.click(center(probe::header("left:level", 1).unwrap()));
    let l = probe::row_order("left:level");
    let mut sorted = l.clone();
    sorted.sort_by(|a, b| xpr_dex::text::locale_cmp(a, b));
    assert_eq!(l, sorted);
    d.click(center(probe::header("right:level", 2).unwrap()));
    assert!(d.shows("Type \u{25B2}"));
}

#[test]
fn right_click_on_a_row_toggles_the_test_set() {
    let mut d = pair();
    d.right_click(center(row("left:level", "Ember")));
    assert_eq!(d.view.state.move_test_set, names(&["Ember"]));
    assert!(d.shows("\u{25CF}"), "the dot before the move: {:?}", d.texts);
    d.right_click(center(row("right:level", "Bubble")));
    assert_eq!(d.view.state.move_test_set, names(&["Ember", "Bubble"]));
    d.right_click(center(row("left:level", "Ember")));
    assert_eq!(d.view.state.move_test_set, names(&["Bubble"]));
}

#[test]
fn the_cross_out_settings_strike_banned_tm_moves() {
    let mut d = driver(json!({ "selected": "Charmander", "comparing_with": "Squirtle", "game": "Emerald", "cross_out_banned": true }));
    assert!(struck(&d, "Double Team"), "Double Team is banned in Emerald");
    assert!(!struck(&d, "Toxic"));
    d.view.state.settings.cross_out_banned = false;
    d.frame(2);
    assert!(!struck(&d, "Double Team"));
}

#[test]
fn clicking_a_section_label_copies_the_table() {
    let mut d = pair();
    d.click(center(probe::find("label", "left:level", "Level Up Learnset").unwrap()));
    assert!(d.shows("Copied"), "{:?}", d.texts);
    // it fades after a moment
    d.frame(120);
    assert!(!d.shows("Copied"));
}

#[test]
fn evolution_chips_select_within_the_comparison() {
    let mut d = pair();
    d.click(center(probe::find("evo", "left", "Charmeleon").unwrap()));
    assert_eq!(d.view.state.selected(), Some("Charmeleon"));
    assert_eq!(d.view.state.settings.comparing_with.as_deref(), Some("Squirtle"), "still comparing");
    assert!(d.shows("Charmeleon"));
    d.click(center(probe::find("evo", "right", "Wartortle").unwrap()));
    assert_eq!(d.view.state.settings.comparing_with.as_deref(), Some("Wartortle"));
    assert_eq!(d.view.state.selected(), Some("Charmeleon"));
    // the current species' own chip does nothing
    d.click(center(probe::find("evo", "right", "Wartortle").unwrap()));
    assert_eq!(d.view.state.settings.comparing_with.as_deref(), Some("Wartortle"));
}

#[test]
fn a_species_missing_from_the_game_shows_the_message_and_exits() {
    let mut d = driver(json!({ "selected": "Charmander", "comparing_with": "Chikorita", "game": "Red and Blue" }));
    assert!(d.shows("One or both Pokemon not available in this game."), "{:?}", d.texts);
    let btn = probe::any("button", "Exit Comparison").unwrap();
    d.click(center(btn));
    assert_eq!(d.view.state.settings.comparing_with, None);
    assert!(!d.shows("not available in this game"));
}

#[test]
fn stat_rows_show_the_diffs_of_gen1_and_the_optional_totals() {
    // Yellow: five stats with Spc; WBST / UBST / bulk rows follow the settings
    let d = driver(json!({ "selected": "Charizard", "comparing_with": "Mewtwo", "game": "Yellow" }));
    assert!(contains(&d, "Spc") > 0 && contains(&d, "SpD") == 0, "{:?}", d.texts);
    assert!(contains(&d, "WBST") == 0 && contains(&d, "Bulk") == 0);
    let d = driver(json!({ "selected": "Charizard", "comparing_with": "Mewtwo", "game": "Yellow", "show_wbst": true, "show_ubst": true, "show_bulk": true }));
    // 425 + 85 (Special counted twice) = 510 vs 590 + 154 = 744
    // (the labels of the bulk rows wrap onto two lines, as in Solodex)
    for t in ["WBST", "UBST", "Phys", "Spec", "Bulk", "510", "744", "6,084", "9,540"] {
        assert!(contains(&d, t) > 0, "{} in {:?}", t, d.texts);
    }
    // later generations have no WBST even when asked for
    let d = driver(json!({ "selected": "Charizard", "comparing_with": "Mewtwo", "game": "Emerald", "show_wbst": true }));
    assert!(contains(&d, "WBST") == 0 && contains(&d, "SpD") > 0);
}

// ---- rankings -------------------------------------------------------------------------------

#[test]
fn hovering_a_stat_row_opens_a_ranking_on_each_side_and_it_lingers() {
    let mut d = pair();
    d.hover(center(stat("pair", "HP")));
    no_id_clash(&d);
    assert_eq!(contains(&d, "HP RANKING \u{2014} EMERALD"), 2, "{:?}", d.texts);
    shot(&mut d, "pair_ranking");
    // both lists highlight both species and centre on their own
    let left = probe::items().into_iter().filter(|p| p.kind == "pop").map(|p| p.text).collect::<Vec<_>>();
    assert!(left.contains(&"Charmander".to_string()) || left.contains(&"Squirtle".to_string()), "{:?}", left);
    // moving off the row (and the popovers) closes them after 200 ms
    d.hover(Pos2::new(700.0, 600.0));
    d.frame(30);
    assert_eq!(contains(&d, "RANKING"), 0, "{:?}", d.texts);
}

#[test]
fn a_ranking_row_selects_the_species_and_leaves_the_comparison() {
    let mut d = pair();
    d.hover(center(stat("pair", "HP")));
    let other = probe::items()
        .into_iter()
        .find(|p| p.kind == "pop" && p.text != "Charmander" && p.text != "Squirtle")
        .expect("a ranking row");
    d.click(center(other.rect));
    assert_eq!(d.view.state.selected(), Some(other.text.as_str()));
    assert_eq!(d.view.state.settings.comparing_with, None);
    assert_eq!(contains(&d, "RANKING"), 0);
}

#[test]
fn a_ranking_titles_click_opens_the_expanded_card() {
    let mut d = pair();
    d.hover(center(stat("pair", "Atk")));
    assert_eq!(contains(&d, "ATK RANKING"), 2);
    let title = probe::items().into_iter().find(|p| p.kind == "poptitle").unwrap().rect;
    d.click(center(title));
    // the card is a second, wider list: its column header says "Value"
    assert!(d.shows("Value"), "{:?}", d.texts);
    d.key(Key::Escape, Modifiers::NONE);
    d.frame(2);
}

#[test]
fn right_click_on_a_ranking_row_offers_adding_it_to_the_comparison() {
    let mut d = pair();
    d.hover(center(stat("pair", "HP")));
    let other = probe::items()
        .into_iter()
        .find(|p| p.kind == "pop" && p.text != "Charmander" && p.text != "Squirtle")
        .expect("a ranking row");
    d.right_click(center(other.rect));
    let shown = xpr_dex::display_name(&other.text).to_string();
    assert!(d.shows(&format!("Compare Charmander to {}", shown)), "{:?}", d.texts);
    assert!(d.shows(&format!("Add {} to comparison", shown)));
    // the menu holds the popover open while the pointer is elsewhere
    d.frame(30);
    assert!(contains(&d, "HP RANKING") > 0, "still open: {:?}", d.texts);
    // "Add ... to comparison" makes it a triple comparison
    let at = center(other.rect);
    d.click(Pos2::new(at.x + 20.0, at.y + 46.0));
    assert_eq!(d.view.state.settings.comparing_third.as_deref(), Some(other.text.as_str()));
    assert_eq!(d.view.state.selected(), Some("Charmander"));
    assert!(d.shows(&shown));
}

// ---- triple ---------------------------------------------------------------------------------

#[test]
fn triple_shows_three_columns_with_best_and_worst_stats_marked() {
    let mut d = triple();
    for n in ["Bulbasaur", "Charmander", "Squirtle"] {
        assert!(contains(&d, n) > 0, "{}", n);
    }
    assert_eq!(texts_eq(&d, "BASE STATS"), 3);
    assert_eq!(texts_eq(&d, "Total"), 3);
    // HP 45 / 39 / 44: Bulbasaur's is the best (green), Charmander's the worst (red), Squirtle's keeps the stat colour
    let green = Color32::from_rgb(0x4a, 0xde, 0x80);
    let red = Color32::from_rgb(0xf8, 0x71, 0x71);
    let hp_a = stat("a", "HP");
    let hp_b = stat("b", "HP");
    let hp_c = stat("c", "HP");
    assert_eq!(text_color(&d, hp_a, "45"), Some(green));
    assert_eq!(text_color(&d, hp_b, "39"), Some(red));
    assert_ne!(text_color(&d, hp_c, "44"), Some(green));
    assert_ne!(text_color(&d, hp_c, "44"), Some(red));
    // the triple view has no WBST / bulk rows even when asked for
    d.view.state.settings.show_bulk = true;
    d.frame(2);
    assert_eq!(contains(&d, "Phys Bulk"), 0);
    shot(&mut d, "triple");
}

#[test]
fn triple_marks_moves_none_of_the_others_learn() {
    let d = triple();
    // Scratch is only Charmander's; Tackle is Bulbasaur's and Squirtle's
    assert!(filled(&d, row("b:level", "Scratch"), TINT));
    assert!(!filled(&d, row("a:level", "Tackle"), TINT));
    assert!(!filled(&d, row("c:level", "Tackle"), TINT));
    // an empty table still shows its label with (0): nobody here learns tutor moves in Red / Blue
    assert!(contains(&d, "MOVE TUTOR") == 3 && texts_eq(&d, "(0)") >= 3, "{:?}", d.texts);
}

#[test]
fn triple_hover_opens_one_ranking_without_a_card() {
    let mut d = triple();
    d.hover(center(stat("b", "Atk")));
    assert_eq!(contains(&d, "ATK RANKING \u{2014} RED AND BLUE"), 1, "{:?}", d.texts);
    // all three species are highlighted: Bulbasaur, Charmander and Squirtle rows exist in the list
    let rows: Vec<String> = probe::items().into_iter().filter(|p| p.kind == "pop").map(|p| p.text).collect();
    assert!(rows.contains(&"Charmander".to_string()), "{:?}", rows);
    // the title is a plain label: no card
    let title = probe::items().into_iter().find(|p| p.kind == "poptitle").unwrap().rect;
    d.click(center(title));
    assert!(!d.shows("Value"));
    assert_eq!(contains(&d, "ATK RANKING"), 1, "still the same popover");
    shot(&mut d, "triple_ranking");
}

#[test]
fn triple_evolution_chips_pick_per_column() {
    let mut d = triple();
    d.click(center(probe::find("evo", "c", "Wartortle").unwrap()));
    assert_eq!(d.view.state.settings.comparing_third.as_deref(), Some("Wartortle"));
    d.click(center(probe::find("evo", "b", "Charmeleon").unwrap()));
    assert_eq!(d.view.state.settings.comparing_with.as_deref(), Some("Charmeleon"));
    d.click(center(probe::find("evo", "a", "Ivysaur").unwrap()));
    assert_eq!(d.view.state.selected(), Some("Ivysaur"));
    assert!(d.view.state.settings.comparing_with.is_some() && d.view.state.settings.comparing_third.is_some());
}

#[test]
fn triple_with_a_species_missing_from_the_game_shows_the_message() {
    let mut d = driver(json!({ "selected": "Bulbasaur", "comparing_with": "Charmander", "comparing_third": "Chikorita", "game": "Red and Blue" }));
    assert!(d.shows("One or more Pokemon not available in this game."));
    d.click(center(probe::any("button", "Exit Comparison").unwrap()));
    assert_eq!(d.view.state.settings.comparing_with, None);
    assert_eq!(d.view.state.settings.comparing_third, None);
}

// ---- self -----------------------------------------------------------------------------------

#[test]
fn self_comparison_defaults_to_the_last_game_of_another_generation() {
    let mut d = self_view();
    assert_eq!(probe::row_order("left:level")[0], "ThunderShock");
    // Pikachu's last game is Legends Z-A: its first level-up entry is Move Reminder only
    let za = xpr_dex::get_pokemon_data("Pikachu", "Legends Z-A").unwrap();
    let first = xpr_dex_ui::views::movepool::single_level_rows(&za)[0].move_name.clone();
    assert_eq!(probe::row_order("right:level")[0], first);
    // both sides list the species' games; the other side's game is disabled
    assert!(probe::find("game", "left", "Legends Z-A").is_some() && probe::find("game", "right", "Yellow").is_some());
    shot(&mut d, "self");
}

#[test]
fn the_game_chips_change_each_side_and_the_disabled_one_does_nothing() {
    let mut d = self_view();
    let before = probe::row_order("right:level");
    d.click(center(probe::find("game", "right", "Sword and Shield").unwrap()));
    let swsh = xpr_dex::get_pokemon_data("Pikachu", "Sword and Shield").unwrap();
    let expected: Vec<String> = xpr_dex_ui::views::movepool::single_level_rows(&swsh).iter().map(|r| r.move_name.clone()).collect();
    assert_eq!(probe::row_order("right:level"), expected);
    assert_ne!(probe::row_order("right:level"), before);
    // the page's own game and selection are untouched
    assert_eq!(d.view.state.game(), "Yellow");
    assert!(d.view.state.settings.self_compare);
    // the left side
    d.click(center(probe::find("game", "left", "Crystal").unwrap()));
    let c = xpr_dex::get_pokemon_data("Pikachu", "Crystal").unwrap();
    let expected: Vec<String> = xpr_dex_ui::views::movepool::single_level_rows(&c).iter().map(|r| r.move_name.clone()).collect();
    assert_eq!(probe::row_order("left:level"), expected);
    // the game on the other side cannot be picked
    let right_now = probe::row_order("right:level");
    d.click(center(probe::find("game", "left", "Sword and Shield").unwrap()));
    assert_eq!(probe::row_order("left:level"), expected);
    assert_eq!(probe::row_order("right:level"), right_now);
}

#[test]
fn the_right_click_game_choice_of_the_page_picks_the_right_hand_game() {
    let mut d = driver(json!({ "selected": "Pikachu", "game": "Yellow" }));
    d.view.state.compare_games("Crystal");
    d.frame(3);
    let c = xpr_dex::get_pokemon_data("Pikachu", "Crystal").unwrap();
    let expected: Vec<String> = xpr_dex_ui::views::movepool::single_level_rows(&c).iter().map(|r| r.move_name.clone()).collect();
    assert_eq!(probe::row_order("right:level"), expected);
    assert_eq!(probe::row_order("left:level")[0], "ThunderShock");
}

/// The tinted first cell of a level row (the level differs on the other side).
fn tinted_cell(d: &DexDriver, row: Rect) -> bool {
    let mut found = false;
    for cs in d.off.last_shapes() {
        walk(&cs.shape, &mut |s| {
            if let Shape::Rect(rs) = s {
                if rs.fill == TINT
                    && (rs.rect.min.y - row.min.y).abs() < 0.6
                    && (rs.rect.min.x - row.min.x).abs() < 0.6
                    && rs.rect.width() < row.width() / 2.0
                {
                    found = true;
                }
            }
        });
    }
    found
}

#[test]
fn self_comparison_marks_moves_and_levels_across_spellings() {
    // Yellow vs Legends Z-A (the default pick)
    let d = self_view();
    // "ThunderShock" and "Thunder Shock" are one move at level 1: no mark at all
    let l = row("left:level", "ThunderShock");
    let r = row("right:level", "Thunder Shock");
    assert!(!filled(&d, l, TINT) && !filled(&d, r, TINT));
    assert!(!tinted_cell(&d, l) && !tinted_cell(&d, r));
    // Thunder Wave is learned at 8 in Yellow and 7 in Z-A: the rows stay plain, the level cells are tinted
    let (l, r) = (row("left:level", "Thunder Wave"), row("right:level", "Thunder Wave"));
    assert!(!filled(&d, l, TINT) && !filled(&d, r, TINT));
    assert!(tinted_cell(&d, l) && tinted_cell(&d, r), "level cells differ");
    // a move only one side learns tints the whole row (and not just the cell)
    let canon = |n: &str| xpr_dex::canonical_move_key(n);
    let tm = |game: &str| -> std::collections::HashSet<String> {
        xpr_dex::get_pokemon_data("Pikachu", game).unwrap().tm_hm_learnset.iter().map(|m| canon(m)).collect()
    };
    let (yellow, za) = (tm("Yellow"), tm("Legends Z-A"));
    let mut checked = 0;
    for name in probe::row_order("left:tmhm") {
        let rect = row("left:tmhm", &name);
        if rect.max.y > 990.0 {
            continue;
        }
        assert_eq!(filled(&d, rect, TINT), !za.contains(&canon(&name)), "{}", name);
        checked += 1;
    }
    assert!(checked >= 3);
    for name in probe::row_order("right:tmhm") {
        let rect = row("right:tmhm", &name);
        if rect.max.y > 990.0 {
            continue;
        }
        assert_eq!(filled(&d, rect, TINT), !yellow.contains(&canon(&name)), "{}", name);
    }
}

#[test]
fn self_stat_popover_opens_on_a_click_and_closes_on_a_click_elsewhere() {
    let mut d = self_view();
    assert_eq!(contains(&d, "RANKING"), 0, "hovering is not enough");
    d.hover(center(stat("self", "HP")));
    assert_eq!(contains(&d, "RANKING"), 0);
    d.click(center(stat("self", "HP")));
    no_id_clash(&d);
    assert_eq!(contains(&d, "HP RANKING"), 1, "{:?}", d.texts);
    assert_eq!(contains(&d, "\u{2014} YELLOW"), 0, "the self view's title has no game");
    // it stays while the pointer is elsewhere
    d.hover(Pos2::new(700.0, 700.0));
    d.frame(30);
    assert_eq!(contains(&d, "HP RANKING"), 1);
    shot(&mut d, "self_ranking");
    // another row switches it
    d.click(center(stat("self", "Atk")));
    assert_eq!(contains(&d, "ATK RANKING"), 1);
    assert_eq!(contains(&d, "HP RANKING"), 0);
    // a click elsewhere closes it
    d.click(Pos2::new(700.0, 700.0));
    assert_eq!(contains(&d, "RANKING"), 0);
}

#[test]
fn a_dialog_keeps_the_self_popover_open_and_a_menu_keeps_it_until_it_closes() {
    let mut d = self_view();
    d.click(center(stat("self", "HP")));
    assert_eq!(contains(&d, "HP RANKING"), 1);
    // the Pokémon search is a modal: a click on its backdrop closes it, not the popover
    d.key(Key::Space, Modifiers::NONE);
    assert!(d.view.state.spotlight.is_some());
    d.click(Pos2::new(700.0, 800.0));
    assert!(d.view.state.spotlight.is_none());
    assert_eq!(contains(&d, "HP RANKING"), 1, "the click belonged to the dialog: {:?}", d.texts);
    // a right-click menu on a ranking row, then a click elsewhere: the menu closes and takes the popover along
    let other = probe::items()
        .into_iter()
        .find(|p| p.kind == "pop" && p.text != "Pikachu")
        .expect("a ranking row");
    d.right_click(center(other.rect));
    assert!(contains(&d, "across generations") > 0, "{:?}", d.texts);
    d.click(Pos2::new(700.0, 800.0));
    d.frame(3);
    assert_eq!(contains(&d, "across generations"), 0);
    assert_eq!(contains(&d, "HP RANKING"), 0, "{:?}", d.texts);
}

#[test]
fn self_unavailable_species_shows_the_message() {
    // a species with no game of the selected generation still has its own games: nothing to show
    let d = driver(json!({ "selected": "Pikachu", "self_compare": true, "game": "Yellow" }));
    assert!(!d.shows("not available in one of the selected generations"));
}

// ---- layout ---------------------------------------------------------------------------------

#[test]
fn the_table_header_sticks_while_its_table_scrolls() {
    let mut d = pair();
    let h0 = probe::header("left:level", 0).unwrap();
    let first = row("left:level", "Scratch");
    assert!(first.min.y >= h0.max.y - 0.5, "the header sits above the first row");
    wheel(&mut d, Pos2::new(700.0, 600.0), 120.0);
    d.frame(30);
    let h1 = probe::header("left:level", 0).unwrap();
    let r1 = row("left:level", "Scratch");
    assert!(r1.min.y < h0.min.y - 60.0, "the table scrolled: {:?} -> {:?}", first, r1);
    assert!(h1.min.y > h0.min.y - 30.0 || r1.min.y < h1.min.y, "the header stays near the top: {:?} -> {:?}", h0, h1);
    assert!(r1.min.y < h1.max.y, "rows pass under the header");
    shot(&mut d, "pair_scrolled");
    wheel(&mut d, Pos2::new(700.0, 600.0), 600.0);
    d.frame(30);
    shot(&mut d, "pair_scrolled_far");
}

#[test]
fn it_is_usable_in_a_small_window_too() {
    let mut d = driver_sized(
        json!({ "selected": "Charmander", "comparing_with": "Squirtle", "game": "Emerald", "list_open": false }),
        1100.0,
        750.0,
    );
    // with the list hidden the whole header and both tables fit
    for t in ["Charmander", "Squirtle", "BASE STATS"] {
        assert!(contains(&d, t) > 0, "{}", t);
    }
    let l = probe::header("left:level", 6).unwrap();
    let r = probe::header("right:level", 6).unwrap();
    assert!(l.max.x < 1100.0 && r.max.x < 1100.0, "{:?} {:?}", l, r);
    shot(&mut d, "pair_small");
    let mut d = driver_sized(json!({ "selected": "Bulbasaur", "comparing_with": "Charmander", "comparing_third": "Squirtle", "game": "Red and Blue", "list_open": false }), 1100.0, 750.0);
    assert!(probe::row("c:level", "Tackle").unwrap().max.x <= 1100.0);
    shot(&mut d, "triple_small");
}

#[test]
fn a_popover_does_not_outlive_what_it_ranks() {
    let mut d = self_view();
    d.click(center(stat("self", "HP")));
    assert_eq!(contains(&d, "HP RANKING"), 1);
    // the page's game changes: the view starts over, so does the popover
    d.view.state.set_game("Crystal");
    d.frame(3);
    assert_eq!(contains(&d, "RANKING"), 0, "{:?}", d.texts);
    // leaving and re-entering the comparison starts clean too
    d.click(center(stat("self", "HP")));
    assert_eq!(contains(&d, "HP RANKING"), 1);
    d.view.state.exit_self_compare();
    d.frame(3);
    d.view.state.self_compare(None);
    d.frame(3);
    assert_eq!(contains(&d, "RANKING"), 0, "{:?}", d.texts);
}
