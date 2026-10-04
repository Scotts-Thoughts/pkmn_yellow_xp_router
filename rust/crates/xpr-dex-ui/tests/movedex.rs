#![cfg(feature = "movedex")]
//! The Movedex tab: the move table and its filters / sorting, the learner
//! panel, the Space jump overlay, the move detail, and the shell's move search.

use egui::{Key, Modifiers, Pos2, Shape, Vec2};
use serde_json::json;
use xpr_dex_ui::testkit::DexDriver;
use xpr_dex_ui::views::movedex::data::{self, Col, Filters, Sort};
use xpr_dex_ui::views::movedex::spotlight_rows;
use xpr_dex_ui::widgets::SortDir;
use xpr_dex_ui::{DexAction, DexTab};

fn driver(settings: serde_json::Value) -> DexDriver {
    let mut d = DexDriver::new(settings, Vec2::new(1600.0, 1000.0));
    d.frame(4);
    d
}

fn movedex(game: &str) -> DexDriver {
    driver(json!({ "tab": "Movedex", "game": game }))
}

/// Centre of the first drawn text equal to `needle`.
fn find(d: &DexDriver, needle: &str) -> Pos2 {
    fn walk(s: &Shape, needle: &str, out: &mut Option<Pos2>) {
        match s {
            Shape::Text(t) if t.galley.text() == needle => {
                if out.is_none() {
                    *out = Some(t.galley.rect.translate(t.pos.to_vec2()).center());
                }
            }
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, needle, out)),
            _ => {}
        }
    }
    let mut out = None;
    for cs in d.off.last_shapes() {
        walk(&cs.shape, needle, &mut out);
    }
    out.unwrap_or_else(|| panic!("no text {:?} drawn; texts: {:?}", needle, d.texts))
}

fn type_in_search(d: &mut DexDriver, text: &str) {
    let p = find(d, "Search moves\u{2026}");
    d.click(p);
    d.type_text(text);
    d.frame(2);
}

// ---- data -----------------------------------------------------------------------------------

#[test]
fn a_games_table_uses_its_spellings_and_marks() {
    let t = data::moves_for_game("Platinum");
    assert!(t.len() > 450, "{}", t.len());
    let ap = t.iter().find(|m| m.name == "AncientPower").expect("gen 4 spelling");
    assert_eq!(ap.data.move_type, "Rock");
    let tb = t.iter().find(|m| m.name == "Thunderbolt").unwrap();
    assert_eq!(tb.tm.as_deref(), Some("TM24"));
    assert!(!tb.new_in_gen);
    assert!(t.iter().find(|m| m.name == "Thunder Fang").unwrap().new_in_gen);
    // names are in order
    assert!(t.windows(2).all(|w| xpr_dex::text::locale_cmp(&w[0].name, &w[1].name) != std::cmp::Ordering::Greater));
    // gen 1: everything is new
    let rb = data::moves_for_game("Red and Blue");
    assert!(rb.iter().all(|m| m.new_in_gen));
    assert!(data::moves_for_game("Nonsense").is_empty());
}

#[test]
fn filters_follow_solodex() {
    let t = data::moves_for_game("Platinum");
    let names = |f: &Filters| -> Vec<String> { t.iter().filter(|m| f.matches(m)).map(|m| m.name.clone()).collect() };
    let mut f = Filters { search: "  THUNDER ".into(), ..Default::default() };
    assert_eq!(names(&f), vec!["Thunder", "Thunder Fang", "Thunder Wave", "Thunderbolt", "ThunderPunch", "ThunderShock"]);
    f.type_filter = "Electric".into();
    f.category = "Special".into();
    assert_eq!(names(&f), vec!["Thunder", "Thunderbolt", "ThunderShock"]);
    f.min_power = "100".into();
    assert_eq!(names(&f), vec!["Thunder"]);
    // a move with no power fails any power bound (Me First is the one status move with a recorded 0)
    let g = Filters { category: "Status".into(), max_power: "500".into(), ..Default::default() };
    assert_eq!(names(&g), vec!["Me First"]);
    // no accuracy (Swift never misses) fails an accuracy bound but passes without one
    let swift = Filters { search: "swift".into(), ..Default::default() };
    assert_eq!(names(&swift), vec!["Swift"]);
    let swift_acc = Filters { search: "swift".into(), min_accuracy: "0".into(), ..Default::default() };
    assert!(names(&swift_acc).is_empty());
    // unreadable numbers mean no bound
    let junk = Filters { search: "swift".into(), min_power: "-".into(), max_pp: "abc".into(), ..Default::default() };
    assert_eq!(names(&junk), vec!["Swift"]);
    // PP range and the new-in-gen switch
    let pp = Filters { min_pp: "40".into(), ..Default::default() };
    assert!(names(&pp).iter().all(|n| t.iter().find(|m| &m.name == n).unwrap().data.pp.unwrap() >= 40));
    let new = Filters { only_new: true, ..Default::default() };
    assert!(names(&new).contains(&"Aqua Jet".to_string()) && !names(&new).contains(&"Tackle".to_string()));
    assert!(!Filters::default().any() && new.any());
}

#[test]
fn sorting_puts_missing_values_last_and_ties_by_name() {
    let t = data::moves_for_game("Platinum");
    let order = |sort: Sort| -> Vec<usize> {
        let mut idx: Vec<usize> = (0..t.len()).collect();
        data::sort_rows(&t, &mut idx, sort);
        idx
    };
    let by_power = order(Sort { col: Col::Power, dir: SortDir::Desc });
    assert_eq!(t[by_power[0]].name, "Explosion");
    assert!(t[*by_power.last().unwrap()].data.power.is_none());
    let powers: Vec<i32> = by_power.iter().filter_map(|i| t[*i].data.power).collect();
    assert!(powers.windows(2).all(|w| w[0] >= w[1]));
    let asc = order(Sort { col: Col::Power, dir: SortDir::Asc });
    assert!(t[*asc.last().unwrap()].data.power.is_none(), "absent power last whichever way");
    // ties by name
    let pp = order(Sort { col: Col::Pp, dir: SortDir::Asc });
    let five: Vec<&str> = pp.iter().filter(|i| t[**i].data.pp == Some(5)).map(|i| t[*i].name.as_str()).collect();
    let mut sorted = five.clone();
    sorted.sort_by(|a, b| xpr_dex::text::locale_cmp(a, b));
    assert_eq!(five, sorted);
    // TM column: TMs before HMs, then by number; no TM last
    let tm = order(Sort { col: Col::Tm, dir: SortDir::Asc });
    assert_eq!(t[tm[0]].tm.as_deref(), Some("TM01"));
    assert!(t[*tm.last().unwrap()].tm.is_none());
    // a click flips, another column starts ascending
    let s = Sort::default().clicked(Col::Power);
    assert_eq!((s.col, s.dir), (Col::Power, SortDir::Asc));
    assert_eq!(s.clicked(Col::Power).dir, SortDir::Desc);
    assert_eq!(s.clicked(Col::Pp).dir, SortDir::Asc);
}

#[test]
fn learners_list_every_method_like_solodex() {
    let l = data::learners_of("Thunderbolt", "Platinum");
    let pika = l.iter().find(|l| l.species == "Pikachu").expect("Pikachu learns Thunderbolt");
    assert_eq!(pika.methods, vec!["Lv 26".to_string(), "TM24".to_string()]);
    assert!(l.windows(2).all(|w| w[0].dex <= w[1].dex), "by national dex number");
    // evolution / reminder / egg / tutor labels
    let raichu = l.iter().find(|l| l.species == "Raichu").unwrap();
    assert!(raichu.methods.contains(&"Lv 1".to_string()) || raichu.methods.contains(&"Evo".to_string()));
    let egg = data::learners_of("Hidden Power", "Platinum");
    assert!(egg.iter().any(|l| l.methods.iter().any(|m| m.starts_with("TM"))));
    assert_eq!(data::method_kind("Lv 4"), data::MethodKind::Level);
    assert_eq!(data::method_kind("Evo"), data::MethodKind::Level);
    assert_eq!(data::method_kind("HM03"), data::MethodKind::Tm);
    assert_eq!(data::method_kind("Egg"), data::MethodKind::Egg);
    assert_eq!(data::method_kind("Tutor"), data::MethodKind::Other);
    assert_eq!(data::method_kind("TR12"), data::MethodKind::Other);
}

#[test]
fn generation_history_collapses_identical_runs() {
    let h = xpr_dex::move_across_gens("Thunderbolt");
    let groups = data::collapse_gens(&h);
    assert_eq!(groups.len(), 2);
    assert_eq!((groups[0].start, groups[0].end), (1, 5));
    assert_eq!((groups[1].start, groups[1].end), (6, 9));
    assert!(groups[0].changes.is_empty());
    assert_eq!(groups[1].changes.len(), 1);
    let c = &groups[1].changes[0];
    assert_eq!(c.old.shown(), "95");
    assert_eq!(c.new.shown(), "90");
    // an unknown move has no history
    assert!(data::collapse_gens(&xpr_dex::move_across_gens("Nope")).is_empty());
    // the older spelling finds the same history
    assert!(!xpr_dex::move_across_gens("Faint Attack").is_empty());
}

#[test]
fn the_move_search_matches_names_of_every_generation() {
    let rows = spotlight_rows("surf");
    let keys: Vec<&str> = rows.iter().map(|r| r.key.as_str()).collect();
    assert!(keys.contains(&"Surf"));
    assert!(keys.iter().all(|k| k.to_lowercase().contains("surf")));
    assert!(rows.iter().all(|r| r.label == r.key));
    assert!(spotlight_rows("  ").len() > 800, "empty query lists everything");
    assert!(spotlight_rows("zzzz").is_empty());
    // names of every generation are listed, by name
    assert!(spotlight_rows("Thunder Fang").len() == 1);
}

// ---- the list -------------------------------------------------------------------------------

#[test]
fn the_list_shows_the_games_moves_and_columns() {
    let d = movedex("Platinum");
    for t in ["Move \u{25B2}", "Type", "Cat", "Pwr", "Acc", "PP", "TM", "Absorb", "Acid Armor", "Newly introduced in this generation", "All Types", "All Categories", "Power", "Accuracy"] {
        assert!(d.shows(t), "{} missing: {:?}", t, d.texts);
    }
    // gen 4 spelling, TM numbers and the new-in-gen mark
    assert!(d.shows("AncientPower"));
    assert!(d.shows("TM40"));
    assert!(d.shows("NEW"));
    // a gen 5 move is not in Platinum's table
    assert!(!d.shows("Hone Claws"));
}

#[test]
fn gen_one_has_no_new_marks_and_follows_the_game() {
    let mut d = movedex("Red and Blue");
    assert!(d.shows("Absorb") && !d.shows("NEW"));
    d.view.state.set_game("Gold and Silver");
    d.frame(3);
    assert!(d.shows("NEW"), "gen 2 marks the moves it added");
}

#[test]
fn searching_filters_the_rows() {
    let mut d = movedex("Platinum");
    type_in_search(&mut d, "thunder");
    for m in ["Thunderbolt", "Thunder Fang", "ThunderPunch"] {
        assert!(d.shows(m), "{}", m);
    }
    assert!(!d.shows("Absorb"));
    // nothing matches
    let mut d = movedex("Platinum");
    type_in_search(&mut d, "zzzz");
    assert!(d.shows("No moves match the current filters."));
}

#[test]
fn clicking_a_header_sorts_and_right_click_resets() {
    let mut d = movedex("Platinum");
    let pwr = find(&d, "Pwr");
    d.click(pwr);
    d.click(pwr);
    assert!(d.shows("Pwr \u{25BC}"));
    assert!(d.shows("Explosion") && !d.shows("Absorb"), "highest power first");
    d.right_click(pwr);
    assert!(d.shows("Move \u{25B2}") && d.shows("Absorb"));
}

#[test]
fn the_new_switch_filters_to_new_moves() {
    let mut d = movedex("Platinum");
    d.click(find(&d, "Newly introduced in this generation"));
    assert!(d.shows("Acupressure") && d.shows("Aqua Jet"));
    assert!(!d.shows("Absorb"));
}

#[test]
fn power_range_filters_numbers_only() {
    let mut d = movedex("Platinum");
    // the first Min box (Power)
    let min = find(&d, "Min");
    d.click(min);
    d.type_text("2a5b0");
    d.frame(2);
    assert!(d.shows("250"), "letters are dropped: {:?}", d.texts);
    assert!(d.shows("Explosion"));
    assert!(!d.shows("Selfdestruct"), "200 is below the minimum of 250");
}

#[test]
fn right_click_shows_who_learns_the_move() {
    let mut d = movedex("Platinum");
    type_in_search(&mut d, "thunderbolt");
    d.right_click(Pos2::new(600.0, 252.0));
    for t in ["Pokemon", "Method", "Pikachu", "Lv 26", "TM24"] {
        assert!(d.shows(t), "{} missing: {:?}", t, d.texts);
    }
    // right click again closes it
    d.right_click(Pos2::new(600.0, 252.0));
    assert!(!d.shows("Method"));
}

#[test]
fn closing_the_panel_with_the_cross() {
    let mut d = movedex("Platinum");
    type_in_search(&mut d, "thunderbolt");
    d.right_click(Pos2::new(600.0, 252.0));
    assert!(d.shows("Method"));
    d.click(find(&d, "\u{d7}"));
    assert!(!d.shows("Method"));
}

#[test]
fn a_move_with_no_learners_says_so() {
    let mut d = movedex("Platinum");
    type_in_search(&mut d, "struggle");
    d.right_click(Pos2::new(600.0, 252.0));
    assert!(d.shows("No Pokemon learn this move in Platinum."), "{:?}", d.texts);
}

#[test]
fn clicking_a_learner_opens_it_in_the_pokedex() {
    let mut d = movedex("Platinum");
    type_in_search(&mut d, "thunderbolt");
    d.right_click(Pos2::new(600.0, 252.0));
    d.click(find(&d, "Pikachu"));
    assert_eq!(d.view.tab(), DexTab::Pokedex);
    assert_eq!(d.view.state.selected(), Some("Pikachu"));
    assert!(d.view.state.focused_move.is_none());
}

#[test]
fn the_name_opens_a_summary_with_links() {
    let mut d = movedex("Platinum");
    d.click(find(&d, "Absorb"));
    for t in ["Move details", "Bulbapedia", "Power 20   Accuracy 100   PP 25   Priority 0"] {
        assert!(d.shows(t), "{} missing: {:?}", t, d.texts);
    }
    d.click(find(&d, "Bulbapedia"));
    assert!(d.take_actions().contains(&DexAction::OpenUrl("https://bulbapedia.bulbagarden.net/wiki/Absorb_(move)".to_string())));
}

#[test]
fn the_summary_opens_the_detail() {
    let mut d = movedex("Platinum");
    d.click(find(&d, "Absorb"));
    d.click(find(&d, "Move details"));
    assert_eq!(d.view.state.focused_move.as_deref(), Some("Absorb"));
    assert!(d.shows("\u{2190} Back") && d.shows("Learned in Platinum"));
}

#[test]
fn a_type_badge_opens_only_its_own_matchups() {
    let mut d = movedex("Platinum");
    d.click(find(&d, "Grass"));
    let n = d.texts.iter().filter(|t| t.as_str() == "SUPER EFFECTIVE VS").count();
    assert_eq!(n, 1, "{:?}", d.texts);
    assert!(d.shows("GRASS"));
    // the click did not also select the row or open its name popover
    assert!(!d.shows("Move details"));
}

// ---- jump overlay ---------------------------------------------------------------------------

#[test]
fn space_jumps_to_a_move_and_highlights_it() {
    let mut d = movedex("Platinum");
    assert!(!d.shows("Thunderbolt"), "not in the first rows");
    d.key(Key::Space, Modifiers::NONE);
    assert!(d.shows("Jump to move"));
    d.type_text("thunderbolt");
    d.frame(2);
    assert!(d.shows("Electric \u{b7} 95"), "type and power: {:?}", d.texts);
    d.click(find(&d, "Thunderbolt"));
    assert!(!d.shows("Jump to move"));
    d.frame(3);
    assert!(d.shows("Thunderbolt"), "scrolled into view: {:?}", d.texts);
    assert!(d.shows("Thunder Wave"));
}

#[test]
fn escape_closes_the_jump_overlay() {
    let mut d = movedex("Platinum");
    d.key(Key::Space, Modifiers::NONE);
    assert!(d.shows("Jump to move"));
    d.key(Key::Escape, Modifiers::NONE);
    assert!(!d.shows("Jump to move"));
}

#[test]
fn space_in_a_text_field_does_not_jump() {
    let mut d = movedex("Platinum");
    let p = find(&d, "Search moves\u{2026}");
    d.click(p);
    d.key(Key::Space, Modifiers::NONE);
    assert!(!d.shows("Jump to move"));
}

#[test]
fn the_jump_list_follows_the_filters() {
    let mut d = movedex("Platinum");
    type_in_search(&mut d, "thunder");
    d.click(Pos2::new(1200.0, 600.0));
    d.key(Key::Space, Modifiers::NONE);
    assert!(d.shows("Jump to move") && d.shows("Thunder Wave"));
    assert!(!d.shows("Absorb"));
}

#[test]
fn space_stands_down_under_the_shells_move_search() {
    let mut d = movedex("Platinum");
    d.key(Key::Space, Modifiers::COMMAND | Modifiers::SHIFT);
    assert!(d.shows("Moves"), "the shell's search is open");
    d.key(Key::Space, Modifiers::NONE);
    assert!(!d.shows("Jump to move"));
}

// ---- the detail -----------------------------------------------------------------------------

#[test]
fn the_shells_move_search_opens_the_detail() {
    let mut d = movedex("Platinum");
    d.key(Key::Space, Modifiers::COMMAND | Modifiers::SHIFT);
    d.type_text("surf");
    d.frame(2);
    assert!(d.shows("Surf") && d.shows("Stoked Sparksurfer"));
    d.click(find(&d, "Surf"));
    assert_eq!(d.view.tab(), DexTab::Movedex);
    assert_eq!(d.view.state.focused_move.as_deref(), Some("Surf"));
    d.frame(3);
    assert!(d.shows("Generations 1\u{2013}5") && d.shows("Gen 1\u{2013}9"), "{:?}", d.texts);
}

#[test]
fn the_detail_shows_cards_changes_and_flags() {
    let mut d = movedex("Platinum");
    d.view.state.focused_move = Some("Thunderbolt".to_string());
    d.frame(3);
    for t in [
        "Thunderbolt", "Gen 1\u{2013}9", "Generations 1\u{2013}5", "Generations 6\u{2013}9", "Gen 5 \u{2192} 6", "Power", "Accuracy", "PP", "Priority", "Effect %", "Effect", "May Paralyze", "Target", "Foe Or Ally", "Contact", "Protect",
        "Magic Coat", "Snatch", "Mirror Move", "King's Rock", "Special", "Bulbapedia",
    ] {
        assert!(d.shows(t), "{} missing: {:?}", t, d.texts);
    }
    // the callout: Power 95 -> 90
    assert!(d.shows("Power") && d.shows("95") && d.shows("90"));
    assert!(d.shows("The user attacks the target with a strong electric blast. This may also leave the target with paralysis."));
}

#[test]
fn the_detail_lists_learners_for_the_game() {
    let mut d = movedex("Platinum");
    d.view.state.focused_move = Some("Thunderbolt".to_string());
    d.frame(3);
    assert!(d.shows("Learned in Platinum"));
    assert!(d.shows("158 Pok\u{e9}mon"), "{:?}", d.texts);
    assert!(d.shows("Rattata") && d.shows("TM24"));
}

#[test]
fn a_move_missing_from_the_game_says_so() {
    let mut d = movedex("Red and Blue");
    d.view.state.focused_move = Some("Thunder Fang".to_string());
    d.frame(3);
    assert!(d.shows("This move does not exist in Red and Blue."), "{:?}", d.texts);
    assert!(d.shows("Gen 4\u{2013}9"));
}

#[test]
fn an_unknown_move_has_no_data() {
    let mut d = movedex("Platinum");
    d.view.state.focused_move = Some("Nope".to_string());
    d.frame(3);
    assert!(d.shows("No data found for this move."));
}

#[test]
fn back_returns_to_the_list() {
    let mut d = movedex("Platinum");
    d.view.state.focused_move = Some("Thunderbolt".to_string());
    d.frame(3);
    d.click(find(&d, "\u{2190} Back"));
    assert!(d.view.state.focused_move.is_none());
    assert!(d.shows("Absorb") && !d.shows("Generations 1\u{2013}5"));
}

#[test]
fn the_detail_links_to_bulbapedia_and_the_pokedex() {
    let mut d = movedex("Platinum");
    d.view.state.focused_move = Some("Double-Edge".to_string());
    d.frame(3);
    d.click(find(&d, "Bulbapedia"));
    assert!(d.take_actions().contains(&DexAction::OpenUrl("https://bulbapedia.bulbagarden.net/wiki/Double-Edge_(move)".to_string())));
    d.view.state.focused_move = Some("Thunderbolt".to_string());
    d.frame(3);
    d.click(find(&d, "Pikachu"));
    assert_eq!(d.view.tab(), DexTab::Pokedex);
    assert_eq!(d.view.state.selected(), Some("Pikachu"));
    assert!(d.view.state.focused_move.is_none(), "leaving for the Pokédex consumes the focused move");
}

#[test]
fn leaving_the_tab_clears_the_focused_move() {
    let mut d = movedex("Platinum");
    d.view.state.focused_move = Some("Thunderbolt".to_string());
    d.frame(3);
    d.key(Key::F1, Modifiers::NONE);
    assert_eq!(d.view.tab(), DexTab::Pokedex);
    assert!(d.view.state.focused_move.is_none());
}

#[test]
fn it_fits_a_small_window() {
    let mut d = DexDriver::new(json!({ "tab": "Movedex", "game": "Emerald" }), Vec2::new(1100.0, 750.0));
    d.frame(4);
    assert!(d.shows("Absorb") && d.shows("Pwr"));
    d.view.state.focused_move = Some("Hyper Beam".to_string());
    d.frame(3);
    assert!(d.shows("Hyper Beam") && d.shows("Learned in Emerald"));
}
