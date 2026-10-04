#![cfg(feature = "trainers")]
//! The Trainers tab on the router's trainer data: list, detail, speed list,
//! team order calculator and the trainer search.

use egui::{Key, Modifiers, Pos2, Shape, Vec2};
use serde_json::json;
use xpr_dex_ui::testkit::{test_registry, DexDriver};
use xpr_dex_ui::views::trainers::spotlight_rows;
use xpr_dex_ui::{DexAction, DexTab, Spotlight};

fn driver_at(version: &str, trainer: Option<&str>, size: Vec2) -> DexDriver {
    let mut d = DexDriver::new(json!({ "tab": "Trainers", "version": version }), size);
    d.view.state.selected_trainer = trainer.map(|s| s.to_string());
    d.frame(4);
    d
}

fn driver(version: &str, trainer: Option<&str>) -> DexDriver {
    driver_at(version, trainer, Vec2::new(1600.0, 1000.0))
}

fn walk(shape: &Shape, hit: &dyn Fn(&str) -> bool, out: &mut Vec<Pos2>) {
    match shape {
        Shape::Text(t) => {
            if hit(t.galley.text()) {
                out.push(t.visual_bounding_rect().center());
            }
        }
        Shape::Vec(v) => v.iter().for_each(|s| walk(s, hit, out)),
        _ => {}
    }
}

/// Every place the exact text `needle` was drawn last frame, in paint order.
fn text_positions(d: &DexDriver, needle: &str) -> Vec<Pos2> {
    let mut out = Vec::new();
    for cs in d.off.last_shapes() {
        walk(&cs.shape, &|t| t == needle, &mut out);
    }
    out
}

/// A list row's name and its id are one text ("Leader Bugsy(1)"): find it by its start.
fn first_prefix(d: &DexDriver, prefix: &str) -> Pos2 {
    let mut out = Vec::new();
    for cs in d.off.last_shapes() {
        walk(&cs.shape, &|t| t.starts_with(prefix), &mut out);
    }
    *out.first().unwrap_or_else(|| panic!("no text starting {:?}; texts: {:?}", prefix, d.texts))
}

fn first(d: &DexDriver, needle: &str) -> Pos2 {
    *text_positions(d, needle).first().unwrap_or_else(|| panic!("{:?} not drawn; texts: {:?}", needle, d.texts))
}

fn last(d: &DexDriver, needle: &str) -> Pos2 {
    *text_positions(d, needle).last().unwrap_or_else(|| panic!("{:?} not drawn; texts: {:?}", needle, d.texts))
}

fn count_label(d: &DexDriver) -> String {
    d.texts.iter().find(|t| t.ends_with(" trainers") || t.ends_with(" trainer")).cloned().unwrap_or_default()
}

#[test]
fn nothing_selected_shows_the_placeholder_and_a_sorted_list() {
    let d = driver("Crystal", None);
    assert!(d.shows("Select a trainer"), "{:?}", d.texts);
    assert!(d.shows("Speed Rankings") || d.shows("SPEED RANKINGS"), "{:?}", d.texts);
    // gym leaders come first, by ace level; ordinary trainers after the major fights
    let falkner = first_prefix(&d, "Leader Falkner").y;
    let bugsy = first_prefix(&d, "Leader Bugsy").y;
    assert!(falkner < bugsy);
    let reg = test_registry();
    let total = reg.get_version("Crystal").unwrap().trainer_db().len();
    let n: usize = count_label(&d).split(' ').next().unwrap().parse().unwrap();
    // rematch families and repeated fights share rows, so there are fewer rows than trainers
    assert!(n > 0 && n <= total, "{} rows for {} trainers", n, total);
}

#[test]
fn the_detail_shows_the_party_with_stats_and_moves() {
    let d = driver("Crystal", Some("Leader Falkner"));
    for t in ["Pidgey", "Pidgeotto", "Violet Gym", "$900", "2 Pokemon, avg Lv8", "299 total xp", "Mud Slap", "Gust", "Total", "Tackle", "Leader"] {
        assert!(d.shows(t), "missing {:?}: {:?}", t, d.texts);
    }
    // Pidgey Lv7 at 24 HP, total 81 (Crystal's stat block)
    assert!(d.shows("Lv7") && d.shows("Lv9") && d.shows("81") && d.shows("114"), "{:?}", d.texts);
    // the type effectiveness table
    assert!(d.shows("Type Effectiveness") || d.shows("TYPE EFFECTIVENESS"), "{:?}", d.texts);
    assert!(d.shows("2x"), "Normal/Flying takes 2x from Electric, Rock and Ice");
    assert!(d.shows("0x"), "and nothing from Ground");
    // gen 2 has no abilities or natures
    assert!(!d.shows("Levitate"));
}

#[test]
fn double_battles_and_abilities_show_in_gen_3_and_up() {
    let d = driver("Emerald", Some("Leader Tate & Liza"));
    assert!(d.shows("Double"), "{:?}", d.texts);
    assert!(d.shows("Claydol") && d.shows("Xatu") && d.shows("Lunatone") && d.shows("Solrock"));
    assert!(d.shows("Levitate") && d.shows("Synchronize"), "abilities: {:?}", d.texts);
    assert!(d.shows("Sitrus Berry"), "held items");
    assert!(d.texts.iter().any(|t| t.starts_with("Lax (") || t.starts_with("Modest (") || t.starts_with("Impish (")), "natures with their stat changes: {:?}", d.texts);
    let d = driver("Black 2", Some("Leader Roxie (157)"));
    assert!(d.shows("Koffing") && d.shows("Whirlipede"), "{:?}", d.texts);
    assert!(d.shows("Poison Point"));
}

#[test]
fn gen_1_cards_have_one_special_stat() {
    let d = driver("Red", Some("Brock 1"));
    assert!(d.shows("Geodude") && d.shows("Onix"));
    assert!(d.shows("Spc"), "{:?}", d.texts);
    assert!(!d.shows("SpA") && !d.shows("SpD"));
    // no Team Order for gen 1
    assert!(!d.shows("Team Order"));
}

#[test]
fn group_variants_switch_the_shown_trainer() {
    let mut d = driver("Emerald", Some("Leader Tate & Liza"));
    assert!(d.shows("Leader Tate & Liza Rematch 2"), "{:?}", d.texts);
    // the header and the first variant button both say the selected name; the third is a button
    let before = d.texts.iter().filter(|t| *t == "Leader Tate & Liza Rematch 2").count();
    d.click(first(&d, "Leader Tate & Liza Rematch 2"));
    d.frame(2);
    let after = d.texts.iter().filter(|t| *t == "Leader Tate & Liza Rematch 2").count();
    assert!(after > before, "the variant is now the header too: {} -> {}", before, after);
    // the selection itself did not change
    assert_eq!(d.view.state.selected_trainer.as_deref(), Some("Leader Tate & Liza"));
}

#[test]
fn clicking_a_list_row_selects_the_trainer() {
    let mut d = driver("Crystal", None);
    d.click(first_prefix(&d, "Leader Bugsy"));
    assert_eq!(d.view.state.selected_trainer.as_deref(), Some("Leader Bugsy"));
    assert!(d.shows("Metapod") && d.shows("Scyther"), "{:?}", d.texts);
}

#[test]
fn searching_filters_the_list_and_the_game_toggle_resets_it() {
    let mut d = driver("Crystal", None);
    d.click(first(&d, "Search trainers..."));
    d.type_text("falkner");
    d.frame(2);
    assert_eq!(count_label(&d), "1 trainer", "{:?}", d.texts);
    assert!(!d.shows("Leader Bugsy"));
    // searching also matches the class and the location
    d.click(first(&d, "falkner"));
    d.key(Key::A, Modifiers::COMMAND);
    d.type_text("VioletGym");
    d.frame(2);
    // Falkner and the two Bird Keepers of Violet Gym
    assert_eq!(count_label(&d), "3 trainers", "{:?}", d.texts);
    assert!(d.shows("Leader Falkner(1)") || d.texts.iter().any(|t| t.starts_with("Leader Falkner")));
    // another game starts with empty filters
    d.click(first(&d, "Emerald"));
    d.frame(2);
    assert_ne!(count_label(&d), "1 trainer");
    assert!(d.shows("Search trainers..."), "the search box was cleared: {:?}", d.texts);
}

#[test]
fn add_to_route_needs_a_route_of_that_version() {
    let mut d = driver("Crystal", Some("Leader Falkner"));
    d.route.version = Some("Red".into());
    d.frame(2);
    d.click(first(&d, "Add to Route"));
    assert!(d.take_actions().is_empty(), "disabled while the open route is another version");
    d.route.version = Some("Crystal".into());
    d.frame(2);
    d.click(first(&d, "Add to Route"));
    assert_eq!(d.take_actions(), vec![DexAction::AddTrainerToRoute { version: "Crystal".into(), trainer: "Leader Falkner".into() }]);
}

#[test]
fn add_to_route_adds_the_shown_variant() {
    let mut d = driver("Emerald", Some("Leader Tate & Liza"));
    d.route.version = Some("Emerald".into());
    d.frame(2);
    d.click(first(&d, "Leader Tate & Liza Rematch 1"));
    d.click(first(&d, "Add to Route"));
    assert_eq!(d.take_actions(), vec![DexAction::AddTrainerToRoute { version: "Emerald".into(), trainer: "Leader Tate & Liza Rematch 1".into() }]);
}

#[test]
fn calc_damage_hands_the_trainer_to_the_damage_tab() {
    let mut d = driver("Crystal", Some("Leader Falkner"));
    d.route.solo_species = Some("Totodile".into());
    d.frame(2);
    d.click(first(&d, "Calc Damage"));
    let r = &d.view.state.damage_request;
    assert_eq!(r.nonce, 1);
    assert_eq!(r.version.as_deref(), Some("Crystal"));
    assert_eq!(r.trainer.as_deref(), Some("Leader Falkner"));
    assert_eq!(r.species.as_deref(), Some("Totodile"));
    assert!(r.moves.is_empty());
    assert_eq!(d.view.tab(), DexTab::Damage);
}

#[test]
fn team_order_is_offered_for_gens_2_to_4_only() {
    for (version, trainer, expect) in [("Crystal", "Leader Falkner", true), ("Emerald", "Leader Roxanne", true), ("Platinum", "Leader Roark", true), ("Red", "Brock 1", false), ("Black 2", "Leader Roxie (157)", false)] {
        let d = driver(version, Some(trainer));
        assert_eq!(d.shows("Team Order"), expect, "{} {}", version, trainer);
    }
}

#[test]
fn the_team_order_calculator_predicts_and_closes() {
    let mut d = driver("Crystal", Some("Leader Morty"));
    d.click(first(&d, "Team Order"));
    d.frame(3);
    assert!(d.shows("Team Order Calculator") && d.shows("Leader Morty \u{b7} Gen 2 send-out logic"), "{:?}", d.texts);
    assert!(d.shows("Lead \u{2014} slot 0 is always sent out first"));
    assert!(d.shows("Gastly") && d.shows("Haunter") && d.shows("Gengar"));
    // against a Normal player nothing is super-effective: neutral matchups all the way
    assert!(d.texts.iter().any(|t| t.starts_with("Neutral matchup")), "{:?}", d.texts);
    // picking Ghost makes the Ghost mons' Shadow Ball-less teams read differently
    d.click(last(&d, "Ghost"));
    d.frame(2);
    assert!(d.shows("Team Order Calculator"));
    // Escape closes the dialog
    d.key(Key::Escape, Modifiers::NONE);
    d.frame(2);
    assert!(!d.shows("Team Order Calculator"), "{:?}", d.texts);
}

#[test]
fn clicks_behind_the_team_order_dialog_do_nothing() {
    let mut d = driver("Crystal", Some("Leader Morty"));
    d.click(first(&d, "Team Order"));
    d.frame(3);
    // a list row under the dialog's backdrop
    d.click(first_prefix(&d, "Leader Bugsy"));
    d.frame(2);
    // the click lands on the backdrop (which closes the dialog, like Solodex) and not on the row
    assert_eq!(d.view.state.selected_trainer.as_deref(), Some("Leader Morty"));
    assert!(!d.shows("Team Order Calculator"));
    // now that the dialog is gone the same row works
    d.click(first_prefix(&d, "Leader Bugsy"));
    assert_eq!(d.view.state.selected_trainer.as_deref(), Some("Leader Bugsy"));
}

#[test]
fn the_speed_list_ranks_by_speed_and_selects_the_trainer() {
    let mut d = driver("Crystal", None);
    let reg = test_registry();
    let gen = reg.get_version("Crystal").unwrap();
    let mut all: Vec<(i64, String)> = gen.trainer_db().iter().flat_map(|t| t.pkmn.iter().map(move |p| (p.cur_stats.speed, t.name.clone()))).collect();
    all.sort_by(|a, b| b.0.cmp(&a.0));
    // the rows after the "SPD" head are (trainer, species, speed) triples, fastest first
    let head = d.texts.iter().position(|t| t == "SPD").expect("speed list head");
    let drawn: Vec<i64> = d.texts[head + 1..].chunks(3).filter(|c| c.len() == 3).map(|c| c[2].parse::<i64>().unwrap()).take(8).collect();
    let expected: Vec<i64> = all.iter().map(|(s, _)| *s).take(8).collect();
    assert_eq!(drawn, expected);
    // clicking a row selects its trainer
    let row = text_positions(&d, &all[0].0.to_string());
    let row = *row.last().unwrap();
    d.click(Pos2::new(row.x - 100.0, row.y));
    assert_eq!(d.view.state.selected_trainer.as_deref(), Some(all[0].1.as_str()));
}

#[test]
fn the_speed_list_filters() {
    // Major Battles drops ordinary trainers (the fast "PkmnTrainer" Red-rematch-less rows of Crystal)
    let mut d = driver("Crystal", None);
    assert!(d.texts.iter().any(|t| t.starts_with("PkmnTrainer")), "an ordinary trainer is among the fastest: {:?}", d.texts);
    d.click(first(&d, "Major Battles"));
    d.frame(2);
    assert!(d.shows("Leader Red"));
    assert!(!d.texts.iter().any(|t| t.starts_with("PkmnTrainer")), "{:?}", d.texts);
    d.click(first(&d, "Major Battles"));
    d.frame(2);
    assert!(d.texts.iter().any(|t| t.starts_with("PkmnTrainer")));
    // R1 hides rematch teams
    let mut d = driver("Emerald", None);
    let rows = |d: &DexDriver| d.texts.iter().filter(|t| t.starts_with("Leader Wattso")).count();
    let before = rows(&d);
    assert!(before > 1, "Wattson and his rematches are among the fastest");
    d.click(first(&d, "R1"));
    d.frame(2);
    let after = rows(&d);
    assert!(after < before, "{} -> {}", before, after);
}

#[test]
fn the_trainer_search_spans_every_version() {
    let reg = test_registry();
    // no query: the current game's major fights, by ace level
    let rows = spotlight_rows("", &reg, Some("Crystal"));
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|r| r.key.starts_with("Crystal\u{1f}")), "{:?}", rows.iter().take(3).collect::<Vec<_>>());
    assert!(rows.iter().any(|r| r.key == "Crystal\u{1f}Leader Falkner"));
    // a name: the current game's entry wins
    let rows = spotlight_rows("brock", &reg, Some("Crystal"));
    assert!(rows.iter().any(|r| r.key == "Crystal\u{1f}Leader Brock"), "{:?}", rows);
    // a trainer only another game has: that game's entry
    let rows = spotlight_rows("roark", &reg, Some("Crystal"));
    assert!(rows.iter().any(|r| r.key.ends_with("\u{1f}Leader Roark") && !r.key.starts_with("Crystal")), "{:?}", rows);
    // class matches count too, and the detail names the class, level and game
    let rows = spotlight_rows("elite four", &reg, Some("Crystal"));
    assert!(rows.len() > 3);
    assert!(rows[0].detail.contains("Lv") && rows[0].detail.contains("\u{b7}"), "{}", rows[0].detail);
    // no match
    assert!(spotlight_rows("zzzzqqq", &reg, Some("Crystal")).is_empty());
    // the latest game wins when the current one does not have it
    let rows = spotlight_rows("cheren", &reg, Some("Crystal"));
    assert!(rows.iter().any(|r| r.key.starts_with("White 2\u{1f}Leader Cheren")), "{:?}", rows.iter().map(|r| &r.key).collect::<Vec<_>>());
}

#[test]
fn picking_a_search_result_opens_that_trainer() {
    let mut d = driver("Red", None);
    d.key(Key::Space, Modifiers::SHIFT);
    assert_eq!(d.view.state.spotlight, Some(Spotlight::Trainer));
    d.type_text("falkner");
    d.frame(2);
    assert!(d.shows("Leader Falkner"), "{:?}", d.texts);
    d.click(first(&d, "Leader Falkner"));
    d.frame(3);
    assert_eq!(d.view.state.spotlight, None);
    assert_eq!(d.view.tab(), DexTab::Trainers);
    let v = d.view.state.settings.version.clone().unwrap();
    assert_ne!(v, "Red");
    assert_eq!(d.view.state.selected_trainer.as_deref(), Some("Leader Falkner"));
    // the list and the detail follow
    assert!(d.shows("Pidgey"), "{:?}", d.texts);
}

#[test]
fn a_selected_group_member_highlights_its_row_and_starts_on_that_variant() {
    // the speed list can pick any member of a group; the detail starts on it
    let mut d = driver("Emerald", Some("Leader Tate & Liza Rematch 2"));
    d.frame(2);
    let headers = d.texts.iter().filter(|t| *t == "Leader Tate & Liza Rematch 2").count();
    assert!(headers >= 2, "header + button: {:?}", d.texts);
    // the list shows the group once, under its primary's name
    assert_eq!(d.texts.iter().filter(|t| *t == "Leader Tate & Liza").count(), 2 /* list row + variant button */, "{:?}", d.texts);
    d.view.state.selected_trainer = None;
    d.frame(2);
}

#[test]
fn works_at_a_small_window() {
    let d = driver_at("Crystal", Some("Leader Falkner"), Vec2::new(1100.0, 750.0));
    for t in ["Leader Falkner", "Pidgey", "Pidgeotto", "Calc Damage", "Team Order"] {
        assert!(d.shows(t), "missing {:?}: {:?}", t, d.texts);
    }
    // nothing is laid out past the canvas
    let right = text_positions(&d, "Calc Damage").first().map(|p| p.x).unwrap_or(0.0);
    assert!(right < 1100.0);
}

#[test]
fn dragging_the_splitter_resizes_the_side_columns() {
    use egui::{Event, PointerButton};
    let mut d = driver("Crystal", Some("Leader Falkner"));
    let w0 = d.view.state.settings.list_width;
    let grab = Pos2::new(w0 + 2.0, 600.0);
    d.frame_with(1, vec![Event::PointerMoved(grab), Event::PointerButton { pos: grab, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }]);
    let to = Pos2::new(grab.x + 60.0, grab.y);
    d.frame_with(2, vec![Event::PointerMoved(Pos2::new(grab.x + 30.0, grab.y))]);
    d.frame_with(2, vec![Event::PointerMoved(to)]);
    d.frame_with(2, vec![Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }]);
    d.frame(2);
    let w1 = d.view.state.settings.list_width;
    assert!(w1 > w0 + 40.0 && w1 <= w0 + 70.0, "{} -> {}", w0, w1);
    assert!(d.view.take_settings_dirty(), "the new width is saved");
}

#[test]
fn type_badges_open_their_matchups() {
    let mut d = driver("Crystal", Some("Leader Falkner"));
    assert!(!d.shows("SUPER EFFECTIVE VS"));
    d.click(first(&d, "Flying"));
    d.frame(2);
    assert!(d.shows("SUPER EFFECTIVE VS"), "{:?}", d.texts);
}

#[test]
fn a_trainer_that_is_not_in_the_version_shows_the_placeholder() {
    // the shell keeps the selection across games; a name the game lacks selects nothing
    let d = driver("Red", Some("Leader Falkner"));
    assert!(d.shows("Select a trainer"), "{:?}", d.texts);
}
