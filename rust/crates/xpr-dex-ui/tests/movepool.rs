#![cfg(feature = "movepool")]
//! The movepool column: tables, sorting, test set and coverage, cross-outs,
//! popovers and the banned-moves editor. Runs the page headlessly with the
//! `movepool` feature alone (the shell draws the movepool of the selected
//! species in the right pane). Set `MOVEPOOL_SHOTS=<dir>` to also save PNGs
//! of a few states.

use egui::{Event, Key, Modifiers, MouseWheelUnit, Pos2, Rect, Vec2};
use serde_json::json;
use xpr_dex_ui::testkit::DexDriver;
use xpr_dex_ui::views::movepool::probe;
use xpr_dex_ui::DexAction;

fn driver_sized(settings: serde_json::Value, w: f32, h: f32) -> DexDriver {
    probe::enable();
    let mut d = DexDriver::new(settings, Vec2::new(w, h));
    d.frame(4);
    d
}

fn driver(settings: serde_json::Value) -> DexDriver {
    driver_sized(settings, 1600.0, 1000.0)
}

fn pikachu_emerald() -> DexDriver {
    driver(json!({ "tab": "Pokedex", "selected": "Pikachu", "game": "Emerald" }))
}

fn row(section: &str, name: &str) -> Rect {
    probe::row(section, name).unwrap_or_else(|| panic!("no row {} in {}", name, section))
}

fn shot(d: &mut DexDriver, name: &str) {
    if let Ok(dir) = std::env::var("MOVEPOOL_SHOTS") {
        let _ = std::fs::create_dir_all(&dir);
        d.save_png(&std::path::Path::new(&dir).join(format!("{}.png", name))).expect("save png");
    }
}

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

// ---- tables -------------------------------------------------------------------------------

#[test]
fn level_up_keeps_the_data_order_and_labels_the_tables() {
    let d = pikachu_emerald();
    assert_eq!(
        probe::row_order("level"),
        names(&["ThunderShock", "Growl", "Tail Whip", "Thunder Wave", "Quick Attack", "Double Team", "Slam", "Thunderbolt", "Agility", "Thunder", "Light Screen"])
    );
    for label in ["LEVEL UP LEARNSET", "MOVE TUTOR", "EGG MOVES", "PRIOR EVOLUTION ONLY", "TM / HM LEARNSET"] {
        assert!(d.shows(label), "{} in {:?}", label, d.texts);
    }
    // no transfer table outside generation 1
    assert!(!d.shows("TRANSFER MOVES"));
    for h in ["Move", "Type", "Cat", "Pwr", "Acc", "PP"] {
        assert!(d.shows(h), "{}", h);
    }
    assert!(d.shows("Special") && d.shows("Electric"));
}

#[test]
fn extra_level_one_moves_are_reminder_only_and_evolution_moves_sort_after_level_one() {
    // Charizard in X and Y lists nine level-1 moves: the first five are reminder-only
    let d = driver(json!({ "tab": "Pokedex", "selected": "Charizard", "game": "X and Y" }));
    let order = probe::row_order("level");
    assert_eq!(&order[..5], &names(&["Flare Blitz", "Heat Wave", "Dragon Claw", "Shadow Claw", "Air Slash"])[..]);
    assert!(d.shows("Rem"), "{:?}", d.texts);
    // Venusaur in Sword and Shield: "Evo" (level 0) goes between level 1 and level 9
    let d = driver(json!({ "tab": "Pokedex", "selected": "Venusaur", "game": "Sword and Shield" }));
    let order = probe::row_order("level");
    assert_eq!(&order[..8], &names(&["Petal Blizzard", "Petal Dance", "Tackle", "Growl", "Vine Whip", "Growth", "Petal Blizzard", "Leech Seed"])[..]);
    assert!(d.shows("Evo") && d.shows("Rem"));
    // Legends: Z-A's level -1 entries are Move Reminder moves
    let d = driver(json!({ "tab": "Pokedex", "selected": "Pikachu", "game": "Legends Z-A" }));
    assert_eq!(probe::row_order("level")[0], "Draining Kiss");
    assert!(d.shows("Rem"));
}

#[test]
fn tm_rows_carry_their_codes_in_tm_order() {
    let d = pikachu_emerald();
    let order = probe::row_order("tmhm");
    assert_eq!(order.first().map(|s| s.as_str()), Some("Focus Punch"), "TM01 first: {:?}", order);
    assert_eq!(order.last().map(|s| s.as_str()), Some("Rock Smash"), "HM06 last");
    assert!(d.shows("TM24") && d.shows("HM04"));
    // HMs sort after every TM
    let pos = |n: &str| order.iter().position(|m| m == n).unwrap();
    assert!(pos("Thunderbolt") < pos("Strength"));
}

#[test]
fn generation_one_has_transfer_moves() {
    let d = driver(json!({ "tab": "Pokedex", "selected": "Pikachu", "game": "Red and Blue" }));
    assert!(d.shows("TRANSFER MOVES"), "{:?}", d.texts);
    assert!(probe::row_order("transfer").contains(&"Defense Curl".to_string()));
    // Red and Blue have no tutors or egg moves for Pikachu
    assert!(!d.shows("MOVE TUTOR") && !d.shows("EGG MOVES"));
}

#[test]
fn light_ball_egg_move_table_is_shown_when_the_data_has_one() {
    let d = pikachu_emerald();
    assert!(d.shows("LIGHT BALL EGG MOVE"));
    assert_eq!(probe::row_order("lightball"), names(&["Volt Tackle"]));
}

// ---- sorting ------------------------------------------------------------------------------

fn power(name: &str) -> i32 {
    xpr_dex::get_move_data(name, "Emerald").and_then(|m| m.power).unwrap_or(-1)
}

#[test]
fn header_clicks_sort_flip_and_reset() {
    let mut d = pikachu_emerald();
    let natural = probe::row_order("level");
    // Pwr: ascending, nothing (-1) first, ties in table order
    let pwr = probe::header("level", 4).unwrap();
    d.click(pwr.center());
    let mut asc = natural.clone();
    asc.sort_by_key(|n| power(n));
    assert_eq!(probe::row_order("level"), asc);
    assert!(d.shows("Pwr \u{25B2}"), "{:?}", d.texts);
    // the other tables keep their own order
    assert_eq!(probe::row_order("tutor")[0], "Body Slam");
    // again: descending; ties keep the table's order (a stable sort of the negated comparison)
    let pwr = probe::header("level", 4).unwrap();
    d.click(pwr.center());
    let mut desc = natural.clone();
    desc.sort_by(|a, b| power(b).cmp(&power(a)));
    assert_eq!(probe::row_order("level"), desc);
    assert!(d.shows("Pwr \u{25BC}"));
    // right click resets
    let pwr = probe::header("level", 4).unwrap();
    d.right_click(pwr.center());
    assert_eq!(probe::row_order("level"), natural);
    assert!(!d.shows("\u{25BC}") && !d.shows("\u{25B2}"));
    // Move sorts by name; the first column's header resets
    let mv = probe::header("level", 1).unwrap();
    d.click(mv.center());
    let mut by_name = natural.clone();
    by_name.sort_by(|a, b| xpr_dex::text::locale_cmp(a, b));
    assert_eq!(probe::row_order("level"), by_name);
    let first = probe::header("level", 0).unwrap();
    d.click(first.center());
    assert_eq!(probe::row_order("level"), natural);
}

#[test]
fn category_and_type_sorting() {
    let mut d = pikachu_emerald();
    let cat = probe::header("tmhm", 3).unwrap();
    d.click(cat.center());
    let order = probe::row_order("tmhm");
    let cat_of = |n: &str| match xpr_dex::get_move_data(n, "Emerald").map(|m| m.category).as_deref() {
        Some("Physical") => 0,
        Some("Special") => 1,
        Some("Status") => 2,
        _ => 3,
    };
    assert!(order.windows(2).all(|w| cat_of(&w[0]) <= cat_of(&w[1])), "{:?}", order);
    let ty = probe::header("tmhm", 2).unwrap();
    d.click(ty.center());
    let order = probe::row_order("tmhm");
    let ty_of = |n: &str| xpr_dex::get_move_data(n, "Emerald").map(|m| m.move_type).unwrap_or_default();
    assert!(order.windows(2).all(|w| xpr_dex::text::locale_cmp(&ty_of(&w[0]), &ty_of(&w[1])).is_le()));
}

#[test]
fn a_new_species_starts_unsorted() {
    let mut d = pikachu_emerald();
    let pwr = probe::header("level", 4).unwrap();
    d.click(pwr.center());
    assert!(d.shows("\u{25B2}"));
    d.view.state.select_species("Raichu");
    d.frame(3);
    assert!(!d.shows("\u{25B2}"));
}

#[test]
fn headers_stick_to_the_top_while_their_table_is_in_view() {
    let mut d = pikachu_emerald();
    let label = probe::find("label", "TM / HM Learnset").unwrap();
    let head0 = probe::header("tmhm", 1).unwrap();
    d.hover(Pos2::new(900.0, 400.0));
    d.frame_with(3, vec![Event::MouseWheel { unit: MouseWheelUnit::Point, delta: Vec2::new(0.0, -300.0), modifiers: Modifiers::NONE }]);
    let label2 = probe::find("label", "TM / HM Learnset").unwrap();
    let head = probe::header("tmhm", 1).unwrap();
    assert!(label2.min.y < label.min.y - 100.0, "scrolled: {:?} -> {:?}", label, label2);
    assert!(head.min.y > head0.min.y - 300.0 && head.min.y < head0.min.y, "the header moved less than the content: {:?} -> {:?}", head0, head);
    // the level table's header is stuck at the same line
    let level_head = probe::header("level", 1).unwrap();
    assert!((level_head.min.y - head.min.y).abs() < 0.5, "{:?} vs {:?}", level_head, head);
    shot(&mut d, "scrolled_sticky_headers");
}

// ---- layout -------------------------------------------------------------------------------

#[test]
fn two_columns_when_wide_and_stacked_when_narrow() {
    let _wide = pikachu_emerald();
    let level = probe::find("label", "Level Up Learnset").unwrap();
    let tm = probe::find("label", "TM / HM Learnset").unwrap();
    assert!(tm.min.x > level.min.x + 300.0 && (tm.min.y - level.min.y).abs() < 1.0, "side by side: {:?} {:?}", level, tm);
    let mut d = driver_sized(json!({ "tab": "Pokedex", "selected": "Pikachu", "game": "Emerald" }), 900.0, 700.0);
    let level = probe::find("label", "Level Up Learnset").unwrap();
    let tm = probe::find("label", "TM / HM Learnset").unwrap();
    assert!((tm.min.x - level.min.x).abs() < 1.0 && tm.min.y > level.min.y + 300.0, "stacked: {:?} {:?}", level, tm);
    shot(&mut d, "narrow_stacked");
}

// ---- test set and coverage ------------------------------------------------------------------

#[test]
fn right_click_builds_the_test_set_up_to_four_moves() {
    let mut d = pikachu_emerald();
    assert!(!d.shows("COVERAGE"));
    for m in ["Thunderbolt", "Quick Attack", "Slam", "Agility"] {
        d.right_click(row("level", m).center());
    }
    assert_eq!(d.view.state.move_test_set, names(&["Thunderbolt", "Quick Attack", "Slam", "Agility"]));
    assert!(d.shows("COVERAGE"));
    // a fifth is ignored
    d.right_click(row("level", "Thunder").center());
    assert_eq!(d.view.state.move_test_set.len(), 4);
    // the same move in another table is the same move: right-click removes it
    d.right_click(row("tmhm", "Thunderbolt").center());
    assert_eq!(d.view.state.move_test_set, names(&["Quick Attack", "Slam", "Agility"]));
    // and the fifth fits now
    d.right_click(row("level", "Thunder").center());
    assert_eq!(d.view.state.move_test_set.len(), 4);
    shot(&mut d, "test_set_coverage");
}

#[test]
fn coverage_panel_removes_moves_and_clears() {
    let mut d = pikachu_emerald();
    d.right_click(row("level", "Thunderbolt").center());
    d.right_click(row("level", "Growl").center());
    assert_eq!(d.view.state.move_test_set.len(), 2);
    // a status move is marked
    assert!(d.shows("status"));
    // the chip removes its move
    let chip = probe::find("chip", "Growl").unwrap();
    d.click(Pos2::new(chip.max.x - 6.0, chip.center().y));
    assert_eq!(d.view.state.move_test_set, names(&["Thunderbolt"]));
    let clear = probe::find("clear", "Clear all").unwrap();
    d.click(clear.center());
    assert!(d.view.state.move_test_set.is_empty());
    d.frame(3);
    assert!(!d.shows("COVERAGE"));
}

#[test]
fn status_only_test_sets_have_no_coverage() {
    let mut d = pikachu_emerald();
    d.right_click(row("level", "Growl").center());
    assert!(d.shows("All moves are Status"), "{:?}", d.texts);
    assert!(!d.shows("Neutral"));
}

#[test]
fn the_test_set_belongs_to_the_species() {
    let mut d = pikachu_emerald();
    d.right_click(row("level", "Thunderbolt").center());
    d.view.state.select_species("Raichu");
    d.frame(3);
    assert!(d.view.state.move_test_set.is_empty());
    assert!(!d.shows("COVERAGE"));
}

// ---- cross-outs -----------------------------------------------------------------------------

fn crossed(section: &str) -> Vec<String> {
    probe::items().into_iter().filter(|p| p.kind == "crossed" && p.section == section).map(|p| p.text).collect()
}

#[test]
fn nothing_is_crossed_out_by_default() {
    let _d = pikachu_emerald();
    assert!(crossed("tmhm").is_empty() && crossed("tutor").is_empty());
}

#[test]
fn banned_moves_are_crossed_out_in_tm_and_tutor_tables_only() {
    let _d = driver(json!({ "tab": "Pokedex", "selected": "Pikachu", "game": "Emerald", "cross_out_banned": true }));
    // the global list is Double Team and Minimize; Emerald's TM32
    assert_eq!(crossed("tmhm"), names(&["Double Team"]));
    // Solodex crosses out only TM / HM and tutor rows, so the level-up Double Team stays
    assert!(crossed("level").is_empty());
    assert!(crossed("tutor").is_empty());
}

#[test]
fn postgame_moves_match_legacy_spellings_and_use_the_games_list() {
    let _d = driver(json!({ "tab": "Pokedex", "selected": "Pikachu", "game": "Emerald", "cross_out_postgame": true }));
    let tutor = crossed("tutor");
    // Emerald's built-in postgame list spells it "ThunderPunch"; the tutor data does too
    assert!(tutor.contains(&"ThunderPunch".to_string()) && tutor.contains(&"Body Slam".to_string()), "{:?}", tutor);
    assert!(!tutor.contains(&"Mimic".to_string()));
    // another game's list does not apply
    let _d = driver(json!({ "tab": "Pokedex", "selected": "Pikachu", "game": "Crystal", "cross_out_postgame": true }));
    assert!(!crossed("tmhm").contains(&"Thunderbolt".to_string()));
}

#[test]
fn the_users_lists_add_to_the_built_in_ones_by_canonical_name() {
    let _d = driver(json!({
        "tab": "Pokedex", "selected": "Pikachu", "game": "Emerald",
        "cross_out_banned": true, "cross_out_conditional": true,
        "user_bans": { "banned": ["Thunder Punch"], "conditional": ["Toxic"], "byGame": {} }
    }));
    // modern spelling in the list, legacy spelling in the gen 3 data
    assert!(crossed("tutor").contains(&"ThunderPunch".to_string()), "{:?}", crossed("tutor"));
    assert!(crossed("tmhm").contains(&"Toxic".to_string()));
    // conditional off: Toxic is not crossed
    let _d = driver(json!({
        "tab": "Pokedex", "selected": "Pikachu", "game": "Emerald", "cross_out_banned": true,
        "user_bans": { "banned": [], "conditional": ["Toxic"], "byGame": {} }
    }));
    assert!(!crossed("tmhm").contains(&"Toxic".to_string()));
}

// ---- popovers -------------------------------------------------------------------------------

#[test]
fn tm_code_opens_a_popover_with_a_bulbapedia_link() {
    let mut d = pikachu_emerald();
    let r = row("tmhm", "Thunderbolt");
    d.click(Pos2::new(r.min.x + 14.0, r.center().y));
    assert!(d.shows("TM24 \u{2014} Thunderbolt"), "{:?}", d.texts);
    assert!(d.shows("Bulbapedia"));
    // the TM's move stats come from the Dex data
    assert!(d.shows("Pwr 95"), "{:?}", d.texts);
    let link = probe::find("link", "Bulbapedia").unwrap();
    d.take_actions();
    d.click(link.center());
    assert_eq!(d.take_actions(), vec![DexAction::OpenUrl("https://bulbapedia.bulbagarden.net/wiki/TM024".to_string())]);
    shot(&mut d, "tm_popover");
}

#[test]
fn tutor_label_opens_the_version_picker_then_links() {
    let mut d = pikachu_emerald();
    let r = row("tutor", "Body Slam");
    d.click(Pos2::new(r.min.x + 14.0, r.center().y));
    assert!(d.shows("Select Version"), "{:?}", d.texts);
    let v = probe::find("version", "Emerald").unwrap();
    d.click(v.center());
    assert!(d.shows("Move Tutor \u{2014} Emerald"), "{:?}", d.texts);
    let link = probe::find("link", "Serebii").unwrap();
    d.take_actions();
    d.click(link.center());
    assert_eq!(d.take_actions(), vec![DexAction::OpenUrl("https://www.serebii.net/emerald/movetutor.shtml".to_string())]);
}

#[test]
fn crystal_has_a_single_tutor_version_picked_for_you() {
    let mut d = driver(json!({ "tab": "Pokedex", "selected": "Pikachu", "game": "Crystal" }));
    // Crystal's Pikachu may have no tutor moves in the data; use a species that does
    if probe::row_order("tutor").is_empty() {
        d.view.state.select_species("Mew");
        d.frame(3);
    }
    let order = probe::row_order("tutor");
    assert!(!order.is_empty(), "a Crystal species with tutor moves");
    let r = row("tutor", &order[0]);
    d.click(Pos2::new(r.min.x + 14.0, r.center().y));
    assert!(d.shows("Move Tutor \u{2014} Crystal"), "{:?}", d.texts);
}

// ---- banned moves editor ----------------------------------------------------------------------

#[test]
fn banned_editor_adds_and_removes_moves() {
    let mut d = pikachu_emerald();
    d.view.open_banned_moves_editor();
    d.frame(3);
    assert!(d.shows("Define Banned Moves") && d.shows("Banned globally") && d.shows("Conditional") && d.shows("Per-game"));
    assert!(d.shows("YOUR BANS (0)") && d.shows("No moves added yet."));
    // built-in globals are listed
    assert!(d.shows("BUILT-IN (2)") && d.shows("Minimize"), "{:?}", d.texts);
    d.type_text("thunder pu");
    d.frame(2);
    assert!(d.shows("Thunder Punch"), "{:?}", d.texts);
    shot(&mut d, "banned_modal_suggestions");
    d.key(Key::Enter, Modifiers::NONE);
    assert_eq!(d.view.state.settings.user_bans.banned, names(&["Thunder Punch"]));
    assert!(d.view.take_settings_dirty());
    d.frame(2);
    assert!(d.shows("YOUR BANS (1)"));
    shot(&mut d, "banned_modal_with_ban");
    // remove it again with the chip's x
    let x = probe::find("remove", "Thunder Punch").unwrap();
    d.click(x.center());
    assert!(d.view.state.settings.user_bans.banned.is_empty());
    shot(&mut d, "banned_modal");
}

#[test]
fn banned_editor_tabs_and_per_game_lists() {
    let mut d = pikachu_emerald();
    d.view.open_banned_moves_editor();
    d.frame(3);
    let tab = probe::find("tab", "Per-game").unwrap();
    d.click(tab.center());
    // the dialog is centred, so it moves when its height changes
    d.frame(3);
    // the dialog opens on the Dex game
    assert!(d.shows("Emerald (E)"), "{:?}", d.texts);
    assert!(d.shows("Crossed out only in Emerald"));
    shot(&mut d, "banned_modal_per_game");
    d.type_text("surf");
    d.frame(2);
    // the first suggestion is "Stoked Sparksurfer"; Down moves to "Surf"
    assert!(d.shows("Stoked Sparksurfer"), "{:?}", d.texts);
    d.key(Key::ArrowDown, Modifiers::NONE);
    d.key(Key::Enter, Modifiers::NONE);
    assert_eq!(d.view.state.settings.user_bans.by_game.get("Emerald"), Some(&names(&["Surf"])));
    assert!(d.view.state.settings.user_bans.banned.is_empty());
    // the game list opens inside the dialog; Escape closes the list first, the dialog next
    let combo = probe::find("game", "combo").unwrap();
    d.click(combo.center());
    assert!(d.shows("Gold and Silver (GS)"), "{:?}", d.texts);
    shot(&mut d, "banned_modal_game_list");
    d.key(Key::Escape, Modifiers::NONE);
    assert!(d.view.state.banned_editor_open, "Escape closed the open game list, not the dialog");
    d.frame(2);
    assert!(!d.shows("Gold and Silver (GS)"));
    d.key(Key::Escape, Modifiers::NONE);
    assert!(!d.view.state.banned_editor_open);
    d.frame(2);
    assert!(!d.shows("Define Banned Moves"));
}

#[test]
fn banned_editor_blocks_the_page_under_it() {
    let mut d = pikachu_emerald();
    d.view.open_banned_moves_editor();
    d.frame(3);
    d.right_click(row("level", "Thunderbolt").center());
    assert!(d.view.state.move_test_set.is_empty(), "a click on the dialog's backdrop must not reach the table");
    shot(&mut d, "banned_modal_over_page");
}

// ---- widgets inside a row -----------------------------------------------------------------------

#[test]
fn type_badges_open_the_matchups_and_right_clicks_on_them_still_toggle_the_test_set() {
    let mut d = pikachu_emerald();
    let head = probe::header("level", 2).unwrap();
    let r = row("level", "Thunderbolt");
    let badge = Pos2::new(head.min.x + 4.0 + 34.0, r.center().y);
    d.click(badge);
    assert!(d.shows("SUPER EFFECTIVE VS"), "{:?}", d.texts);
    assert!(d.view.state.move_test_set.is_empty(), "a left click on a badge only opens the popover");
    // close it (click elsewhere), then right-click the badge: Solodex's contextmenu bubbles up to the row
    d.click(Pos2::new(1400.0, 800.0));
    assert!(!d.shows("SUPER EFFECTIVE VS"));
    d.right_click(badge);
    assert_eq!(d.view.state.move_test_set, names(&["Thunderbolt"]));
    // and so does a right-click on a TM code
    let r = row("tmhm", "Dig");
    d.right_click(Pos2::new(r.min.x + 14.0, r.center().y));
    assert_eq!(d.view.state.move_test_set, names(&["Thunderbolt", "Dig"]));
}
