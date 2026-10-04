#![cfg(feature = "stats")]
//! The Stats tab: the stat calculator, the nature selector and the scouting
//! list built from the router's trainer data.

use egui::{Key, Modifiers, Pos2, Shape, Vec2};
use serde_json::json;
use xpr_dex_ui::testkit::{test_registry, DexDriver};
use xpr_dex_ui::views::stats::{battles, major_battles, scout};

fn driver(settings: serde_json::Value) -> DexDriver {
    let mut d = DexDriver::new(settings, Vec2::new(1600.0, 1000.0));
    d.frame(4);
    d
}

fn stats(game: &str, species: &str) -> DexDriver {
    driver(json!({ "tab": "Stats", "game": game, "selected": species }))
}

/// The centre of the first drawn text that is exactly `text`.
fn find(d: &DexDriver, text: &str) -> Pos2 {
    fn walk(shape: &Shape, text: &str) -> Option<Pos2> {
        match shape {
            Shape::Text(t) if t.galley.text() == text => Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center()),
            Shape::Vec(v) => v.iter().find_map(|s| walk(s, text)),
            _ => None,
        }
    }
    d.off.last_shapes().iter().find_map(|cs| walk(&cs.shape, text)).unwrap_or_else(|| panic!("no text {:?} in {:?}", text, d.texts))
}

// ---- the calculator ----------------------------------------------------------------------------

#[test]
fn gen3_calculator_shows_the_species_stats_and_nature_section() {
    let d = stats("Emerald", "Mudkip");
    // Mudkip L50, 31 IVs, 0 EVs, Hardy: HP 125, Atk 90, Def 70, SpA 70, SpD 70, Spe 60
    for t in ["MUDKIP", "125", "90", "70", "60", "IVs & EVs", "Water", "Hardy", "neutral"] {
        assert!(d.shows(t) || d.shows(&t.to_uppercase()), "missing {:?} in {:?}", t, d.texts);
    }
    assert!(d.shows("NATURE"), "{:?}", d.texts);
    assert!(d.shows("SpD") && d.shows("Spe"));
    assert!(!d.shows("Spc"));
}

#[test]
fn gen1_calculator_has_dvs_stat_exp_and_one_special() {
    let d = stats("Yellow", "Pikachu");
    // Pikachu L50, max DVs, no stat exp: HP 110, Atk 75, Def 50, Spc 70, Spe 110
    assert!(d.shows("PIKACHU") && d.shows("Spc") && d.shows("110") && d.shows("75"), "{:?}", d.texts);
    assert!(d.shows("DV") && d.shows("Exp"));
    assert!(d.shows("DVS & STAT EXP"), "{:?}", d.texts);
    // no nature in gens 1-2, no SpD for gen 1
    assert!(!d.shows("NATURE"));
    assert!(!d.shows("SpD"));
}

#[test]
fn gen2_has_split_special_stats_but_no_nature() {
    let d = stats("Crystal", "Pikachu");
    // (the Special DV / Stat Exp column is still one: "Spc")
    assert!(d.shows("SpA") && d.shows("SpD"), "{:?}", d.texts);
    assert!(!d.shows("NATURE"));
}

#[test]
fn nature_buttons_change_the_nature_and_tint_the_stats() {
    let mut d = stats("Emerald", "Mudkip");
    // + Spe (the first row, last button), - Atk (the second row, first button) = Timid
    // the first "Atk" drawn is the "+" row's button; the "-" row is 29 px below
    let atk_first = find(&d, "Atk");
    d.click(Pos2::new(367.0, atk_first.y));
    d.click(Pos2::new(70.0, atk_first.y + 29.0));
    assert!(d.shows("Timid"), "{:?}", d.texts);
    // Timid: Atk 90 x 0.9 = 81, Spe 60 x 1.1 = 66
    assert!(d.shows("81"), "Atk lowered: {:?}", d.texts);
    assert!(d.shows("66"), "Spe raised: {:?}", d.texts);
    // picking the same stat for both is neutral
    d.click(Pos2::new(70.0, atk_first.y));
    assert!(d.shows("neutral"), "{:?}", d.texts);
}

#[test]
fn evs_change_the_calculated_speed_and_the_your_ev_column() {
    let mut d = stats("Emerald", "Mudkip");
    assert!(d.shows("Your EV") || d.shows("YOUR EV"), "{:?}", d.texts);
    assert!(d.shows("Speed EV (0)"), "{:?}", d.texts);
    // the EV row: the last field
    let ev_y = find(&d, "EV").y;
    let spe_x = 375.0;
    d.click(Pos2::new(spe_x, ev_y));
    d.key(Key::A, Modifiers::COMMAND);
    d.type_text("252");
    assert!(d.shows("Speed EV (252)"), "{:?}", d.texts);
    // Mudkip L50 base 40 speed with 252 EVs: (80 + 31 + 63) * 50 / 100 + 5 = 92
    assert!(d.shows("92"), "{:?}", d.texts);
}

#[test]
fn level_only_takes_1_to_100() {
    let mut d = stats("Emerald", "Mudkip");
    let level = find(&d, "50");
    d.click(Pos2::new(level.x, level.y));
    d.key(Key::A, Modifiers::COMMAND);
    d.type_text("100");
    // Mudkip L100 HP: (2*50 + 31) * 100 / 100 + 110 = 241
    assert!(d.shows("241"), "{:?}", d.texts);
    // 150 is refused: the field snaps back to what was in use
    d.key(Key::A, Modifiers::COMMAND);
    d.type_text("150");
    assert!(d.shows("241") || d.shows("100"), "{:?}", d.texts);
    assert!(!d.shows("150"), "{:?}", d.texts);
}

#[test]
fn the_species_picker_filters_and_enter_picks() {
    let mut d = stats("Emerald", "Mudkip");
    let field = Pos2::new(200.0, 136.0);
    d.click(field);
    d.type_text("torch");
    assert!(d.shows("Torchic") && d.shows("#0255"), "{:?}", d.texts);
    assert!(!d.shows("Wartortle"), "the list is filtered by the query");
    d.key(Key::Enter, Modifiers::NONE);
    d.frame(2);
    assert!(d.shows("TORCHIC"), "{:?}", d.texts);
    // the Pokedex's selection is not changed by this tab's own picker
    assert_eq!(d.view.state.selected(), Some("Mudkip"));
}

#[test]
fn the_picker_follows_the_pokedex_selection() {
    let mut d = stats("Emerald", "Mudkip");
    assert!(d.shows("MUDKIP"));
    d.view.state.select_species("Torchic");
    d.frame(3);
    assert!(d.shows("TORCHIC"), "{:?}", d.texts);
}

#[test]
fn a_species_missing_from_the_game_clears_the_picker() {
    // Mudkip is not in the Red and Blue Pokedex
    let d = driver(json!({ "tab": "Stats", "game": "Red and Blue", "selected": "Mudkip" }));
    assert!(d.shows("Select a Pokemon above"), "{:?}", d.texts);
    assert!(d.shows("Select a Pokemon to scout"));
}

// ---- scouting ----------------------------------------------------------------------------------------

#[test]
fn scouting_lists_the_routers_major_battles_with_outspeed_levels() {
    let d = stats("Emerald", "Mudkip");
    for t in ["Scouting", "outspeed levels", "Rival Brendan (Lv5)", "Leader Roxanne", "Champion Wallace", "Nosepass", "Lv15 \u{00B7} 17 Spe", "Rival \u{00B7} max Lv5", "Elite Four Sidney"] {
        assert!(d.shows(t) || d.shows(&t.to_uppercase()), "missing {:?} in {:?}", t, d.texts);
    }
    // Mudkip (base 40) needs L5 to pass the L5 Treecko (12 Spe) with max EVs, L8 with none
    assert!(d.shows("Lv8"), "{:?}", d.texts);
}

#[test]
fn games_without_router_trainers_say_so() {
    let d = driver(json!({ "tab": "Stats", "game": "X and Y", "selected": "Mudkip" }));
    assert!(d.shows("No trainer data available for X and Y"), "{:?}", d.texts);
    // the calculator itself still works
    assert!(d.shows("MUDKIP"));
}

#[test]
fn a_species_that_cannot_catch_up_shows_a_dash() {
    // Slowpoke (base speed 15) never outspeeds Lance's Aerodactyl, even at Lv100 with max Stat Exp
    let d = driver(json!({ "tab": "Stats", "game": "Red and Blue", "selected": "Slowpoke" }));
    assert!(d.texts.iter().any(|t| t == "\u{2014}"), "{:?}", d.texts);
}

#[test]
fn battles_of_red_group_the_rival_starter_variants() {
    let reg = test_registry();
    let gen = reg.get_version("Red").unwrap();
    let b = major_battles(&gen, "Red and Blue");
    assert!(b.windows(2).all(|w| w[0].max_level <= w[1].max_level), "ordered by level");
    let first = b.iter().find(|x| x.name == "Rival1 (Lv5)").expect("rival group");
    assert_eq!(first.trainers.len(), 3);
    assert_eq!(first.category, "rival");
    // the fastest Pokemon is taken over every trainer in the group
    let f = first.fastest.as_ref().unwrap();
    assert_eq!((f.species.as_str(), f.level, f.speed), ("Charmander", 5, 12));
    // single fights keep the router's name
    let brock = b.iter().find(|x| x.name == "Brock 1").unwrap();
    assert_eq!((brock.max_level, brock.trainer_class.as_str(), brock.category.as_str()), (14, "Brock", "gym_leader"));
    assert_eq!(brock.fastest.as_ref().map(|f| (f.species.as_str(), f.speed)), Some(("Onix", 26)));
    // the champion rival is grouped too
    assert!(b.iter().any(|x| x.name == "Rival3" && x.trainers.len() == 3 && x.category == "champion"));
}

#[test]
fn black_lists_the_three_gym_leaders_as_one_battle() {
    let reg = test_registry();
    let gen = reg.get_version("Black").unwrap();
    let b = major_battles(&gen, "Black");
    let trio = b.iter().find(|x| x.name == "Chili / Cilan / Cress").expect("manual group");
    assert_eq!(trio.trainers.len(), 3);
    assert!(!b.iter().any(|x| x.name == "Leader Chili"));
    // the router's "(123)" id suffixes never reach the list
    assert!(b.iter().all(|x| !x.name.contains(" (") || x.name.contains("(Lv")), "{:?}", b.iter().map(|x| &x.name).collect::<Vec<_>>());
}

#[test]
fn every_router_game_has_battles_and_later_games_have_none() {
    let reg = test_registry();
    for g in xpr_dex::GAMES {
        let versions = xpr_dex::games::router_versions(g);
        match versions.first() {
            Some(v) => {
                let b = major_battles(&reg.get_version(v).unwrap(), g);
                assert!(b.len() >= 15, "{}: {} battles", g, b.len());
                assert!(b.iter().all(|x| x.fastest.is_some() && x.max_level > 0), "{}", g);
            }
            None => assert!(xpr_dex::game_gen(g) >= 6, "{} has no router version", g),
        }
    }
}

#[test]
fn grouping_helpers() {
    assert_eq!(battles::strip_id_suffix("Pokemon Trainer Cheren (53)"), "Pokemon Trainer Cheren");
    assert_eq!(battles::strip_rival_suffix("Rival Brendan 4 Treecko", &|n| n == "Treecko"), "Rival Brendan");
}

#[test]
fn outspeed_levels_are_strict_in_both_gens() {
    use xpr_dex::model::BaseStats;
    use xpr_dex::stats::{Gen12Dvs, Gen12StatExps, Gen3Spread, NEUTRAL_NATURE};
    let base = BaseStats { hp: 50, attack: 50, defense: 50, speed: 60, special_attack: 50, special_defense: 50 };
    let s = scout::Spread { dvs: Gen12Dvs::default(), stat_exps: Gen12StatExps::default(), ivs: Gen3Spread::MAX_IVS, evs: Gen3Spread::ZERO, nature: NEUTRAL_NATURE };
    for gen in [1_u8, 3] {
        let at = |l| scout::speed_at_level(&base, gen, l, 0, &s) as i64;
        let l = scout::min_level_to_outspeed(&base, gen, at(30), 0, &s).unwrap();
        assert!(at(l) > at(30) && at(l - 1) <= at(30));
    }
}

#[test]
fn spaces_typed_in_the_picker_do_not_open_the_search_overlay() {
    let mut d = stats("Emerald", "Mudkip");
    d.click(Pos2::new(200.0, 136.0));
    d.type_text("mr. m");
    d.key(Key::Space, Modifiers::NONE);
    assert!(d.view.state.spotlight.is_none(), "Space belongs to the text field while it has focus");
    assert!(d.shows("Mr. Mime"), "{:?}", d.texts);
}

#[test]
fn arrow_keys_move_the_highlight_and_escape_closes_the_list() {
    let mut d = stats("Emerald", "Mudkip");
    d.click(Pos2::new(200.0, 136.0));
    d.type_text("tor");
    assert!(d.shows("Wartortle") && d.shows("Torkoal"));
    // Down twice, then Enter picks the third match (Wartortle, Voltorb, Exeggutor ...)
    d.key(Key::ArrowDown, Modifiers::NONE);
    d.key(Key::ArrowDown, Modifiers::NONE);
    d.key(Key::Enter, Modifiers::NONE);
    d.frame(2);
    assert!(d.shows("EXEGGUTOR"), "{:?}", d.texts);
    // Escape closes the list and brings back the current value
    d.click(Pos2::new(200.0, 136.0));
    d.type_text("pika");
    assert!(d.shows("Pikachu"));
    d.key(Key::Escape, Modifiers::NONE);
    assert!(!d.shows("#0025"), "the list is closed: {:?}", d.texts);
    assert!(d.shows("EXEGGUTOR"));
}
