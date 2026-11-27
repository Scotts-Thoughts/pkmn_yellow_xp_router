//! Persistent area weather: a fight in a weathered area (Red on Mt. Silver's
//! summit in HG/SS) starts with that weather in the calc, Blizzard then
//! skips its accuracy check in hail (pokeheartgold: Blizzard always hits
//! under hail), and a weather move layers on top of it from its matchup on.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::json;
use xpr_calc::battle_summary::{BattleSummary, SummaryConfig};
use xpr_core::consts;
use xpr_data::model::Nature;
use xpr_data::{exp, GenData, Registry};
use xpr_engine::state::SoloPokemonArgs;
use xpr_engine::{Inventory, RouteState, SoloPokemon, TrainerEventDefinition};

fn registry() -> Arc<Registry> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()))
}

fn solo(gen: &GenData, species: &str, level: i64, moves: &[&str]) -> RouteState {
    let def = gen.pkmn_db().get_pkmn(species).unwrap().clone();
    let xp = exp::level_lookup(&def.growth_rate).unwrap().get_xp_for_level(level).unwrap();
    let mut move_list: Vec<Option<String>> = moves.iter().map(|m| Some(m.to_string())).collect();
    while move_list.len() < 4 {
        move_list.push(None);
    }
    let mon = SoloPokemon::new(
        species,
        def,
        gen.make_stat_block(15, 15, 15, 15, 15, 15, false),
        gen.make_badge_list(),
        gen.make_stat_block(0, 0, 0, 0, 0, 0, true),
        0,
        Nature::default(),
        SoloPokemonArgs { move_list: Some(move_list), cur_xp: xp, ..Default::default() },
    )
    .unwrap();
    RouteState::new(mon, gen.make_badge_list(), Inventory::new(None, Vec::new(), gen.bag_limit()))
}

const RED: &str = "Pokemon Trainer Red";

fn red_summary(gen: &GenData) -> BattleSummary {
    let state = solo(gen, "Articuno", 80, &["Blizzard", "Ice Beam"]);
    let red = gen.trainer_db().get_trainer(RED).unwrap().clone();
    let mut s = BattleSummary::new();
    s.load_from_state(gen, Some(&state), &red.pkmn, Some(RED), false, &SummaryConfig::default()).unwrap();
    s
}

fn blizzard_kill_ranges(s: &BattleSummary, mon_idx: usize) -> Vec<(i64, f64)> {
    s.player_move_data[mon_idx].iter().flatten().find(|m| m.name == "Blizzard").unwrap().kill_ranges.clone()
}

#[test]
fn red_defaults_to_hail_and_blizzard_always_hits() {
    let reg = registry();
    for version in [consts::HEART_GOLD_VERSION, consts::SOUL_SILVER_VERSION] {
        let gen = reg.get_version(version).unwrap();
        assert_eq!(gen.get_area_weather(RED), consts::WEATHER_HAIL);
        let cfg = SummaryConfig::default();
        let mut s = red_summary(&gen);
        assert_eq!(s.get_field_weather(), consts::WEATHER_HAIL);
        assert_eq!(s.get_weather_for_mon_idx(0), consts::WEATHER_HAIL);
        let hail = blizzard_kill_ranges(&s, 0);
        s.update_field_weather(&gen, &cfg, consts::WEATHER_NONE);
        let clear = blizzard_kill_ranges(&s, 0);
        // Blizzard is 70% accurate: in hail the 1HKO chance is the damage
        // rolls alone, without it they're scaled by the 70%
        assert_eq!(hail[0].0, 1, "{version}");
        assert!((clear[0].1 - hail[0].1 * 0.7).abs() < 1e-6, "{version}: {hail:?} vs {clear:?}");
        assert_eq!(hail.last().unwrap().1, 100.0, "{version}: {hail:?}");
        assert!(clear.last().unwrap().1 < 100.0, "{version}: {clear:?}");
    }
}

#[test]
fn a_weather_move_overrides_the_area_weather_from_its_matchup_on() {
    let reg = registry();
    let gen = reg.get_version(consts::HEART_GOLD_VERSION).unwrap();
    let cfg = SummaryConfig::default();
    let mut s = red_summary(&gen);
    s.update_weather(&gen, &cfg, consts::WEATHER_SUN, Some(2));
    assert_eq!(s.get_weather_for_mon_idx(1), consts::WEATHER_HAIL);
    assert_eq!(s.get_weather_for_mon_idx(2), consts::WEATHER_SUN);
    // turning the move's weather off falls back to the area's
    s.toggle_weather_from_move(&gen, &cfg, "Sunny Day", false, Some(2));
    assert_eq!(s.get_weather_for_mon_idx(2), consts::WEATHER_HAIL);
}

#[test]
fn other_areas_and_gens_have_no_weather() {
    let reg = registry();
    let gen = reg.get_version(consts::HEART_GOLD_VERSION).unwrap();
    assert_eq!(gen.get_area_weather("Leader Falkner"), consts::WEATHER_NONE);
    assert_eq!(gen.get_area_weather("no such trainer"), consts::WEATHER_NONE);
    let gen = reg.get_version(consts::CRYSTAL_VERSION).unwrap();
    assert_eq!(gen.get_area_weather("Red"), consts::WEATHER_NONE);
}

#[test]
fn field_weather_is_saved_only_when_it_differs_from_the_area_default() {
    let reg = registry();
    let gen = reg.get_version(consts::HEART_GOLD_VERSION).unwrap();
    let cfg = SummaryConfig::default();
    let mut s = red_summary(&gen);
    assert_eq!(s.get_partial_trainer_definition().unwrap().field_weather, None);
    s.update_field_weather(&gen, &cfg, consts::WEATHER_NONE);
    let td = s.get_partial_trainer_definition().unwrap();
    assert_eq!(td.field_weather.as_deref(), Some(consts::WEATHER_NONE));
    let raw = td.serialize();
    assert_eq!(raw.get(consts::FIELD_WEATHER_KEY), Some(&json!("None")));
    let back = TrainerEventDefinition::deserialize(Some(&raw)).unwrap().unwrap();
    assert_eq!(back.field_weather.as_deref(), Some(consts::WEATHER_NONE));
    assert!(TrainerEventDefinition::new(RED).serialize().get(consts::FIELD_WEATHER_KEY).is_none());
}

#[test]
fn spot_checks_across_the_games() {
    let reg = registry();
    let cases = [
        (consts::PLATINUM_VERSION, "Skier Bjorn", consts::WEATHER_HAIL),
        (consts::PLATINUM_VERSION, "Ace Trainer Blake", consts::WEATHER_HAIL),
        (consts::DIAMOND_VERSION, "Skier Bjorn", consts::WEATHER_HAIL),
        (consts::HEART_GOLD_VERSION, "Pokemon Trainer Red", consts::WEATHER_HAIL),
        (consts::FIRE_RED_VERSION, "Leader Brock", consts::WEATHER_NONE),
    ];
    for (version, trainer, expected) in cases {
        let gen = reg.get_version(version).unwrap();
        assert_eq!(gen.get_area_weather(trainer), expected, "{version} {trainer}");
    }
    // every Route 119 trainer outdoors in Emerald fights in rain
    let gen = reg.get_version(consts::EMERALD_VERSION).unwrap();
    let rainy: Vec<_> = gen.trainer_db().iter().filter(|t| t.location == "Route119").map(|t| gen.get_area_weather(&t.name)).collect();
    assert!(rainy.iter().any(|w| *w == consts::WEATHER_RAIN));
    // the Weather Institute is indoors
    assert_eq!(gen.get_area_weather("Aqua Admin Shelly"), consts::WEATHER_NONE);
}
