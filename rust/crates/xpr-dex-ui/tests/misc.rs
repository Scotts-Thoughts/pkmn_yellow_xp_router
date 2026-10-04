#![cfg(feature = "misc")]
//! The Misc tab (hit probability, effective accuracy, multi-hit calculators)
//! and the Natures tab.

use egui::{Key, Modifiers, Pos2, Shape, Vec2};
use serde_json::json;
use xpr_dex_ui::testkit::DexDriver;
use xpr_dex_ui::views::misc::format::{format_number, format_percent, number, percent, to_fixed};
use xpr_dex_ui::views::misc::hit_probability::*;

fn driver(settings: serde_json::Value) -> DexDriver {
    let mut d = DexDriver::new(settings, Vec2::new(1600.0, 1000.0));
    d.frame(4);
    d
}

fn misc(tab: usize) -> DexDriver {
    driver(json!({ "tab": "Misc", "misc_tab": tab }))
}

fn centre_of(d: &DexDriver, pred: &dyn Fn(&str) -> bool) -> Option<Pos2> {
    fn walk(shape: &Shape, pred: &dyn Fn(&str) -> bool) -> Option<Pos2> {
        match shape {
            Shape::Text(t) if pred(t.galley.text()) => Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center()),
            Shape::Vec(v) => v.iter().find_map(|s| walk(s, pred)),
            _ => None,
        }
    }
    d.off.last_shapes().iter().find_map(|cs| walk(&cs.shape, pred))
}

/// The centre of the first drawn text that is exactly `text`.
fn find(d: &DexDriver, text: &str) -> Pos2 {
    centre_of(d, &|t| t == text).unwrap_or_else(|| panic!("no text {:?} in {:?}", text, d.texts))
}

/// The centre of the first drawn text that contains `text`.
fn find_in(d: &DexDriver, text: &str) -> Pos2 {
    centre_of(d, &|t| t.contains(text)).unwrap_or_else(|| panic!("no text containing {:?} in {:?}", text, d.texts))
}

fn set_field(d: &mut DexDriver, at: Pos2, value: &str) {
    d.click(at);
    d.key(Key::A, Modifiers::COMMAND);
    d.type_text(value);
}

// ---- the maths (also unit-tested next to it) --------------------------------------------------------------

#[test]
fn binomial_distribution_values() {
    // 5 uses at 70%: P(X=3) = 10 * 0.7^3 * 0.3^2
    assert!((binomial_pmf(3, 5, 0.7) - 0.3087).abs() < 1e-12);
    assert!((at_least(3, 5, 0.7) - 0.83692).abs() < 1e-12);
    assert!((at_most(3, 5, 0.7) - 0.47178).abs() < 1e-12);
    assert!((distribution(10, 0.3).iter().sum::<f64>() - 1.0).abs() < 1e-12);
    assert_eq!(expected_hits(10, 0.3), 3.0);
}

#[test]
fn formatting_matches_javascript() {
    assert_eq!(percent(0.3087), "30.87%");
    assert_eq!(percent(0.03125), "3.13%"); // JS toFixed rounds the exact tie up
    assert_eq!(to_fixed(3.125, 2), "3.13");
    assert_eq!(format_percent(0.00001, 2), "<0.01%");
    assert_eq!(format_percent(0.99999, 2), ">99.99%");
    assert_eq!(format_number(3.10, 2), "3.1");
    assert_eq!(number(3.0), "3");
}

#[test]
fn effective_accuracy_and_multi_hit_helpers() {
    let r = effective_accuracy(&EffectiveAccuracyInput { base_accuracy: 40.0, accuracy_stages: 2.0, evasion_stages: 0.0, factors: vec![1.3], always_hits: false });
    assert!((r.effective - 40.0 * (5.0 / 3.0) * 1.3).abs() < 1e-9);
    assert_eq!(r.combined_stage, 2.0);
    // 50% at +2 with Compound Eyes is past 100: capped, and flagged
    let r = effective_accuracy(&EffectiveAccuracyInput { base_accuracy: 50.0, accuracy_stages: 2.0, evasion_stages: 0.0, factors: vec![1.3], always_hits: false });
    assert_eq!((r.effective, r.capped_at_100), (100.0, true));
    let o = multi_hit_outcomes(90.0, MultiHitGen::Gen5Plus, MultiHitModifier::None);
    assert!((o.iter().map(|x| x.probability).sum::<f64>() - 1.0).abs() < 1e-12);
    assert!((expected_total_hits(100.0, MultiHitGen::Gen5Plus, MultiHitModifier::None) - 3.1).abs() < 1e-12);
}

// ---- the page --------------------------------------------------------------------------------------

#[test]
fn hit_probability_opens_first_with_the_default_inputs() {
    let d = misc(0);
    for t in ["Hit Probability", "Effective Accuracy", "Multi-Hit Moves", "Exactly 3", "At least 3", "At most 3", "30.87%", "83.69%", "47.18%", "Expected hits: 3.5 of 5", "\u{03C3} = 1.02", "70", "Show full table"] {
        assert!(d.shows(t), "missing {:?} in {:?}", t, d.texts);
    }
    // 0..=5 rows, the 3 highlighted: 0.24% 2.84% 13.23% 30.87% 36.02% 16.81%
    for p in ["0.24%", "2.84%", "13.23%", "36.02%", "16.81%"] {
        assert!(d.shows(p), "missing {} in {:?}", p, d.texts);
    }
}

#[test]
fn quick_accuracy_pills_set_the_accuracy() {
    let mut d = misc(0);
    let pill = find(&d, "50%");
    d.click(pill);
    // 5 uses at 50%: P(X=3) = 10/32, at least 3 = 16/32, at most 3 = 26/32
    assert!(d.shows("31.25%") && d.shows("50.00%") && d.shows("81.25%"), "{:?}", d.texts);
    assert!(d.shows("Expected hits: 2.5 of 5"), "{:?}", d.texts);
    // the exact tie 3.125% rounds up like JS
    assert!(d.shows("3.13%"), "{:?}", d.texts);
}

#[test]
fn the_number_field_clamps_and_feeds_the_results() {
    let mut d = misc(0);
    let field = find(&d, "70");
    set_field(&mut d, field, "90");
    // 5 uses at 90%: P(X=3) = 10 * 0.729 * 0.01
    assert!(d.shows("7.29%"), "{:?}", d.texts);
    // out of range is clamped to 100: certain hits
    d.key(Key::A, Modifiers::COMMAND);
    d.type_text("250");
    assert!(d.shows("Expected hits: 5 of 5"), "{:?}", d.texts);
    // leaving the field shows the clamped number it uses
    d.click(Pos2::new(40.0, 600.0));
    assert!(d.shows("100") && !d.shows("250"), "{:?}", d.texts);
}

#[test]
fn the_slider_moves_the_accuracy() {
    let mut d = misc(0);
    // the accuracy field is the text drawn right after the "ACCURACY" caption
    let value = |d: &DexDriver| -> f64 {
        let i = d.texts.iter().position(|t| t == "ACCURACY").unwrap();
        d.texts[i + 1].parse().unwrap()
    };
    let label = find(&d, "ACCURACY");
    // the far right of the track is (about) 100%, the far left (about) 0%
    d.click(Pos2::new(1040.0, label.y + 26.0));
    assert!(value(&d) >= 98.0, "{}", value(&d));
    d.click(Pos2::new(470.0, label.y + 26.0));
    assert!(value(&d) <= 2.0, "{}", value(&d));
    // in the middle, it is about half
    d.click(Pos2::new(750.0, label.y + 26.0));
    assert!((value(&d) - 50.0).abs() <= 3.0, "{}", value(&d));
}

#[test]
fn clicking_a_bar_picks_k() {
    let mut d = misc(0);
    let bar = find(&d, "13.23%");
    d.click(Pos2::new(700.0, bar.y));
    assert!(d.shows("Exactly 2") && d.shows("At least 2") && d.shows("96.92%"), "{:?}", d.texts);
}

#[test]
fn the_full_table_toggles() {
    let mut d = misc(0);
    assert!(!d.shows("Hits k"));
    let toggle = find(&d, "Show full table");
    d.click(toggle);
    assert!(d.shows("Hits k") && d.shows("P(X \u{2265} k)") && d.shows("Hide full table"), "{:?}", d.texts);
    // the cumulative columns: P(X >= 0) is 100%
    assert!(d.shows("100%"));
}

#[test]
fn the_sub_tab_is_remembered_in_the_settings() {
    let mut d = misc(0);
    let _ = d.view.take_settings_dirty();
    let tab = find(&d, "Effective Accuracy");
    d.click(tab);
    assert_eq!(d.view.state.settings.misc_tab, 1);
    assert!(d.view.take_settings_dirty());
    assert!(d.shows("EFFECTIVE ACCURACY"), "{:?}", d.texts);
    let tab = find(&d, "Multi-Hit Moves");
    d.click(tab);
    assert_eq!(d.view.state.settings.misc_tab, 2);
    assert!(d.shows("Expected total hits"));
    // a fresh page opens on the saved one
    let saved = d.view.settings_json();
    let d2 = driver(saved);
    assert!(d2.shows("Expected total hits"), "{:?}", d2.texts);
    // an index that no longer exists falls back to the first
    let d3 = misc(9);
    assert!(d3.shows("Exactly 3"), "{:?}", d3.texts);
}

#[test]
fn effective_accuracy_builds_the_per_use_accuracy() {
    let mut d = misc(1);
    assert!(d.shows("100% \u{00D7} stage +0 (\u{00D7}1)"), "{:?}", d.texts);
    assert!(d.shows("No Guard / Lock-On"));
    // 50% base, Compound Eyes x1.3 = 65%
    let base = find(&d, "50%");
    d.click(base);
    let ce = find_in(&d, "Compound Eyes");
    d.click(ce);
    assert!(d.shows("65.00%"), "{:?}", d.texts);
    assert!(d.shows("50% \u{00D7} stage +0 (\u{00D7}1) \u{00D7} Compound Eyes (\u{00D7}1.3)"), "{:?}", d.texts);
    // +2 accuracy stages: x(3+2)/3 more
    let plus = find(&d, "+");
    d.click(plus);
    d.click(plus);
    assert!(d.shows("+2"), "{:?}", d.texts);
    assert!(d.shows("50% \u{00D7} stage +2 (\u{00D7}1.667) \u{00D7} Compound Eyes (\u{00D7}1.3)"), "{:?}", d.texts);
    // 50 * 5/3 * 1.3 = 108.3 -> capped
    assert!(d.shows("capped at 100%"), "{:?}", d.texts);
    // the distribution below uses it: certain hits at 100%
    assert!(d.shows("Expected hits: 5 of 5"), "{:?}", d.texts);
}

#[test]
fn stages_stop_at_six() {
    let mut d = misc(1);
    let plus = find(&d, "+");
    for _ in 0..9 {
        d.click(plus);
    }
    assert!(d.shows("+6") && !d.shows("+7"), "{:?}", d.texts);
}

#[test]
fn no_guard_forces_a_certain_hit_and_inerts_the_rest() {
    let mut d = misc(1);
    let base = find(&d, "50%");
    d.click(base);
    assert!(d.shows("50.00%"), "{:?}", d.texts);
    let ng = find_in(&d, "No Guard / Lock-On");
    d.click(ng);
    assert!(d.shows("No Guard / Lock-On \u{2014} the move always hits."), "{:?}", d.texts);
    assert!(d.shows("Expected hits: 5 of 5"), "{:?}", d.texts);
    // the dimmed controls no longer react
    let pill = find(&d, "70%");
    d.click(pill);
    assert!(d.shows("Expected hits: 5 of 5"), "{:?}", d.texts);
    // and switching it off restores the base accuracy (the 50% chosen before)
    d.click(ng);
    assert!(d.shows("50.00%"), "{:?}", d.texts);
}

#[test]
fn multi_hit_defaults_and_modifiers() {
    let mut d = misc(2);
    for t in ["Expected total hits", "Avg hits if it connects", "P(pass) \u{00D7} E[rolled]", "E[rolled hit count]", "Miss", "2 hits", "5 hits", "35.00%", "15.00%"] {
        assert!(d.shows(t) || d.shows(&t.to_uppercase()), "missing {:?} in {:?}", t, d.texts);
    }
    assert!(d.texts.iter().filter(|t| *t == "3.1").count() == 2, "{:?}", d.texts);
    // Skill Link: always 5
    let sl = find_in(&d, "Skill Link");
    d.click(sl);
    assert!(d.texts.iter().filter(|t| *t == "5").count() == 2, "{:?}", d.texts);
    assert!(d.shows("100%"));
    // Loaded Dice: 4 or 5 evenly
    let ld = find_in(&d, "Loaded Dice");
    d.click(ld);
    assert!(d.shows("4.5") && d.shows("50.00%"), "{:?}", d.texts);
}

#[test]
fn multi_hit_gen_1_to_4_drops_loaded_dice() {
    let mut d = misc(2);
    let ld = find_in(&d, "Loaded Dice");
    d.click(ld);
    assert!(d.shows("50.00%"));
    // Gen 1-4 has no Loaded Dice: it falls back to the plain 2-5 distribution (3 / 8 / 3 / 8 ...)
    let g = find(&d, "Gen 1\u{2013}4");
    d.click(g);
    assert!(d.shows("37.50%") && d.shows("12.50%") && !d.shows("50.00%"), "{:?}", d.texts);
    assert!(d.texts.iter().filter(|t| *t == "3").count() == 2, "{:?}", d.texts);
    // and it cannot be picked there
    let ld = find_in(&d, "Loaded Dice");
    d.click(ld);
    assert!(d.shows("37.50%") && !d.shows("50.00%"), "{:?}", d.texts);
}

#[test]
fn multi_hit_accuracy_gates_every_outcome() {
    let mut d = misc(2);
    let p = find(&d, "90%");
    d.click(p);
    // Miss 10%, then 0.9 x (35 / 35 / 15 / 15)
    for t in ["10.00%", "31.50%", "13.50%"] {
        assert!(d.shows(t), "missing {} in {:?}", t, d.texts);
    }
    // 0.9 * 3.1 = 2.79
    assert!(d.shows("2.79"), "{:?}", d.texts);
}

// ---- Natures --------------------------------------------------------------------------------------------

fn natures() -> DexDriver {
    driver(json!({ "tab": "Natures" }))
}

#[test]
fn natures_table_lists_all_25() {
    let d = natures();
    for t in ["Hardy", "Adamant", "Jolly", "Modest", "Quirky", "Likes", "Dislikes", "Increased", "Decreased", "Special Attack", "Special Defense", "Spicy", "Bitter", "Increase:", "Decrease:"] {
        assert!(d.shows(t), "missing {:?} in {:?}", t, d.texts);
    }
    // 25 rows, indexed 0-24
    for i in 0..25 {
        assert!(d.texts.iter().any(|t| *t == i.to_string()), "row {} missing", i);
    }
}

#[test]
fn natures_neutral_rows_use_the_diagonal_stat_and_flavour() {
    let d = natures();
    // Hardy: Attack / Attack / Spicy / Spicy; Quirky: Special Defense / Special Defense / Bitter / Bitter
    assert!(d.texts.iter().filter(|t| *t == "Bitter").count() >= 5);
    assert!(d.texts.iter().filter(|t| *t == "Spicy").count() >= 5);
}

#[test]
fn natures_pickers_highlight_the_matching_row() {
    use xpr_dex_ui::views::natures::matched_nature;
    assert_eq!(matched_nature(Some("attack"), Some("specialAttack")), Some("Adamant"));
    assert_eq!(matched_nature(Some("speed"), Some("attack")), Some("Timid"));
    assert_eq!(matched_nature(Some("defense"), Some("defense")), Some("Docile"));
    assert_eq!(matched_nature(Some("attack"), None), None);
    assert_eq!(matched_nature(None, None), None);
}

#[test]
fn natures_selectors_toggle_and_paint_the_highlight() {
    let mut d = natures();
    assert_eq!(highlights(&d), 0, "no pair picked, no row highlighted");
    // the selectors' buttons: the first row is Increase, the second Decrease
    let inc_speed = find_all(&d, "Speed")[0];
    let dec_attack = find_all(&d, "Attack")[1];
    d.click(inc_speed);
    assert_eq!(highlights(&d), 0, "one stat alone makes no nature");
    d.click(dec_attack);
    // Speed up, Attack down is Timid: its row gets the amber outline
    assert_eq!(highlights(&d), 1);
    // clicking the picked stat again deselects it
    d.click(inc_speed);
    assert_eq!(highlights(&d), 0);
    // the same stat twice is that stat's neutral nature
    let dec_speed = find_all(&d, "Speed")[1];
    d.click(inc_speed);
    d.click(dec_speed);
    assert_eq!(highlights(&d), 1, "Serious is highlighted");
}

/// How many 2 px amber (#fbbf24) outlines are drawn: the matched nature's row.
fn highlights(d: &DexDriver) -> usize {
    fn walk(shape: &Shape) -> usize {
        match shape {
            Shape::Rect(r) if r.stroke.width == 2.0 && r.stroke.color == egui::Color32::from_rgb(0xfb, 0xbf, 0x24) => 1,
            Shape::Vec(v) => v.iter().map(walk).sum(),
            _ => 0,
        }
    }
    d.off.last_shapes().iter().map(|cs| walk(&cs.shape)).sum()
}

fn find_all(d: &DexDriver, text: &str) -> Vec<Pos2> {
    fn walk(shape: &Shape, text: &str, out: &mut Vec<Pos2>) {
        match shape {
            Shape::Text(t) if t.galley.text() == text => out.push(egui::Rect::from_min_size(t.pos, t.galley.size()).center()),
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, text, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for cs in d.off.last_shapes() {
        walk(&cs.shape, text, &mut out);
    }
    out
}
