//! PP spent per fight (docs/rust_port/design/pp_tracking/PLAN.md §1.2-1.4):
//! the best move's guaranteed KO turns, the multi-hit assumption, locked
//! moves, Pressure, setup uses, Mimic and Transform.

use std::path::PathBuf;
use std::sync::Arc;

use indexmap::IndexMap;
use xpr_calc::battle_summary::{BattleSummary, SummaryConfig};
use xpr_calc::pp::{fight_pp_spend, FightInput, FightSpendCache, MatchupSpend, SpendKind};
use xpr_core::consts;
use xpr_data::model::{CustomMoveData, Nature};
use xpr_data::{exp, GenData, Registry};
use xpr_engine::state::SoloPokemonArgs;
use xpr_engine::{EventDefinition, Inventory, RouteState, SoloPokemon, TrainerEventDefinition, WildPkmnEventDefinition};

fn registry() -> Arc<Registry> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()))
}

/// A solo mon at `level` knowing exactly `moves` (max DVs, no EVs, no badges).
fn solo(gen: &GenData, species: &str, level: i64, moves: &[&str]) -> Arc<RouteState> {
    let def = gen.pkmn_db().get_pkmn(species).unwrap().clone();
    let xp = exp::level_lookup(&def.growth_rate).unwrap().get_xp_for_level(level).unwrap();
    let dv = if gen.get_generation() <= 2 { 15 } else { 31 };
    let mut move_list: Vec<Option<String>> = moves.iter().map(|m| Some(m.to_string())).collect();
    while move_list.len() < 4 {
        move_list.push(None);
    }
    let mon = SoloPokemon::new(
        species,
        def,
        gen.make_stat_block(dv, dv, dv, dv, dv, dv, false),
        gen.make_badge_list(),
        gen.make_stat_block(0, 0, 0, 0, 0, 0, true),
        0,
        Nature::default(),
        SoloPokemonArgs { move_list: Some(move_list), cur_xp: xp, ..Default::default() },
    )
    .unwrap();
    Arc::new(RouteState::new(mon, gen.make_badge_list(), Inventory::new(None, Vec::new(), gen.bag_limit())))
}

fn trainer_fight(gen: &GenData, def: EventDefinition, state: &Arc<RouteState>) -> FightInput {
    let enemies = def.pokemon_list(gen).unwrap();
    FightInput { group_id: 1, def, is_wild: false, matchups: enemies.into_iter().map(|e| (e, state.clone())).collect() }
}

fn trainer(gen: &GenData, name: &str, edit: impl FnOnce(&mut TrainerEventDefinition), state: &Arc<RouteState>) -> FightInput {
    let mut td = TrainerEventDefinition::new(name);
    edit(&mut td);
    trainer_fight(gen, EventDefinition::with_trainer(td), state)
}

fn wild(gen: &GenData, name: &str, level: i64, state: &Arc<RouteState>) -> FightInput {
    let def = EventDefinition::with_wild(WildPkmnEventDefinition::new(name, level, 1, false));
    let mon = gen.create_wild_pkmn(name, level, 15).unwrap();
    FightInput { group_id: 2, def, is_wild: true, matchups: vec![(mon, state.clone())] }
}

fn cfg(strategy: &str) -> SummaryConfig {
    SummaryConfig { player_strategy: strategy.to_string(), ..Default::default() }
}

/// Per-matchup player side maps, one per enemy mon.
fn side_maps(n: usize, entries: &[(&str, &str)]) -> Vec<CustomMoveData> {
    let player: IndexMap<String, String> = entries.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    (0..n).map(|_| CustomMoveData { player: player.clone(), enemy: IndexMap::new() }).collect()
}

fn ko(spend: &MatchupSpend) -> &xpr_calc::pp::SlotSpend {
    spend.spends.iter().find(|s| matches!(s.kind, SpendKind::Ko | SpendKind::Mimic)).expect("a KO spend")
}

// ---- KO turns ------------------------------------------------------------------

#[test]
fn pp_turns_match_the_guaranteed_kill_line() {
    let reg = registry();
    for (version, species, level, moves, fight) in [
        (consts::YELLOW_VERSION, "Squirtle", 14, vec!["Tackle", "Water Gun", "Bubble"], "Brock 1"),
        (consts::CRYSTAL_VERSION, "Cyndaquil", 12, vec!["Tackle", "Ember", "Quick Attack"], "Leader Falkner"),
        (consts::EMERALD_VERSION, "Mudkip", 14, vec!["Tackle", "Water Gun", "Mud-Slap"], "Leader Roxanne"),
    ] {
        let gen = reg.get_version(version).unwrap();
        let state = solo(&gen, species, level, &moves);
        let input = trainer(&gen, fight, |_| {}, &state);
        let mut summary = BattleSummary::new();
        summary.load_from_parts(&gen, 1, &input.def, &input.matchups, &cfg(consts::HIGHLIGHT_FASTEST_KILL)).unwrap();
        let mut checked = 0;
        for (mon_idx, row) in summary.player_move_data.iter().enumerate() {
            for m in row.iter().take(4).flatten() {
                if m.min_damage <= 0 {
                    continue;
                }
                let hp = summary.player_pkmn_matchup_data[mon_idx].defending_mon_hp;
                assert_eq!(m.pp_turns, Some((hp + m.min_damage - 1) / m.min_damage), "{} {} vs matchup {}", version, m.name, mon_idx);
                if let Some(&(n, pct)) = m.kill_ranges.last() {
                    if pct == -1.0 {
                        assert_eq!(m.pp_turns, Some(n), "the IGNORING ACC line");
                    }
                }
                checked += 1;
            }
        }
        assert!(checked >= 4, "{}: {} moves checked", version, checked);
    }
}

#[test]
fn the_highlighted_move_pays_its_guaranteed_turns() {
    let reg = registry();
    let gen = reg.get_version(consts::YELLOW_VERSION).unwrap();
    let state = solo(&gen, "Squirtle", 14, &["Tackle", "Tail Whip", "Water Gun"]);
    let input = trainer(&gen, "Brock 1", |_| {}, &state);
    let spends = fight_pp_spend(&gen, &input, &cfg(consts::HIGHLIGHT_FASTEST_KILL));
    assert_eq!(spends.len(), 2);
    for s in &spends {
        let k = ko(s);
        assert_eq!(k.move_name, "Water Gun", "{:?}", s);
        assert_eq!(k.slot, 2);
        assert_eq!(k.pp, k.turns);
        assert!(!k.pressure);
    }
    // strategy None: PP picks the fewest guaranteed turns itself
    let none = fight_pp_spend(&gen, &input, &cfg(consts::HIGHLIGHT_NONE));
    assert_eq!(ko(&none[0]).move_name, "Water Gun");
}

#[test]
fn no_damaging_move_spends_nothing() {
    let reg = registry();
    let gen = reg.get_version(consts::YELLOW_VERSION).unwrap();
    // Thundershock cannot touch Geodude / Onix
    let state = solo(&gen, "Pikachu", 6, &["Thundershock", "Growl"]);
    let spends = fight_pp_spend(&gen, &trainer(&gen, "Brock 1", |_| {}, &state), &cfg(consts::HIGHLIGHT_FASTEST_KILL));
    for s in &spends {
        assert!(s.spends.is_empty(), "{:?}", s);
        assert_eq!(s.note.as_deref(), Some("no damaging move"));
        assert_eq!(s.per_slot(), [0; 4]);
    }
}

// ---- multi-hit -----------------------------------------------------------------------

#[test]
fn variable_multi_hit_moves_count_two_hits_whatever_the_pick() {
    let reg = registry();
    let gen = reg.get_version(consts::EMERALD_VERSION).unwrap();
    let state = solo(&gen, "Beedrill", 20, &["Fury Attack"]);
    let pick = |hits: &str| {
        let input = trainer(&gen, "Leader Brawly", |td| td.custom_move_data = side_maps(3, &[("Fury Attack", hits)]), &state);
        let mut summary = BattleSummary::new();
        summary.load_from_parts(&gen, 1, &input.def, &input.matchups, &cfg(consts::HIGHLIGHT_FASTEST_KILL)).unwrap();
        let m = summary.player_move_data[0][0].clone().unwrap();
        (m.min_damage, m.pp_turns)
    };
    let (two_min, two_turns) = pick(consts::MULTI_HIT_2);
    let (five_min, five_turns) = pick(consts::MULTI_HIT_5);
    assert!(five_min > two_min, "the page shows the picked hit count");
    assert_eq!(two_turns, five_turns, "PP always assumes 2 hits");
}

#[test]
fn fixed_count_moves_keep_their_count() {
    let reg = registry();
    let gen = reg.get_version(consts::EMERALD_VERSION).unwrap();
    // Triple Kick counts all three kicks even when the page picks one
    let state = solo(&gen, "Hitmontop", 25, &["Triple Kick"]);
    let turns_with = |kicks: &str| {
        let input = trainer(&gen, "Leader Brawly", |td| td.custom_move_data = side_maps(3, &[("Triple Kick", kicks)]), &state);
        let mut summary = BattleSummary::new();
        summary.load_from_parts(&gen, 1, &input.def, &input.matchups, &cfg(consts::HIGHLIGHT_FASTEST_KILL)).unwrap();
        let m = summary.player_move_data[0][0].clone().unwrap();
        (m.min_damage, m.pp_turns)
    };
    let (three_min, three_turns) = turns_with("3");
    let (one_min, one_turns) = turns_with("1");
    assert!(three_min > one_min);
    assert_eq!(one_turns, three_turns);

    // Double Kick: the page and PP agree on two hits
    let state = solo(&gen, "Mudkip", 20, &["Double Kick"]);
    let input = trainer(&gen, "Leader Roxanne", |_| {}, &state);
    let mut summary = BattleSummary::new();
    summary.load_from_parts(&gen, 1, &input.def, &input.matchups, &cfg(consts::HIGHLIGHT_FASTEST_KILL)).unwrap();
    let m = summary.player_move_data[0][0].clone().unwrap();
    let hp = summary.player_pkmn_matchup_data[0].defending_mon_hp;
    assert_eq!(m.pp_turns, Some((hp + m.min_damage - 1) / m.min_damage));
}

#[test]
fn gen1_trapping_moves_count_their_shortest_trap() {
    let reg = registry();
    let gen = reg.get_version(consts::YELLOW_VERSION).unwrap();
    let state = solo(&gen, "Ekans", 12, &["Wrap"]);
    let pick = |turns: &str| {
        let input = trainer(&gen, "Brock 1", |td| td.custom_move_data = side_maps(2, &[("Wrap", turns)]), &state);
        fight_pp_spend(&gen, &input, &cfg(consts::HIGHLIGHT_FASTEST_KILL))
    };
    let two = pick(xpr_data::gen_consts::PARTIAL_TRAP_2_TURNS);
    let five = pick(xpr_data::gen_consts::PARTIAL_TRAP_5_TURNS);
    // one use is the whole trap: PP per use, never divided again
    assert_eq!(ko(&two[0]).pp, ko(&two[0]).turns);
    assert_eq!(ko(&two[0]).pp, ko(&five[0]).pp);
}

// ---- locks -----------------------------------------------------------------------------

#[test]
fn locked_moves_pay_once_per_lock() {
    let reg = registry();
    let fastest = cfg(consts::HIGHLIGHT_FASTEST_KILL);

    let yellow = reg.get_version(consts::YELLOW_VERSION).unwrap();
    let state = solo(&yellow, "Tauros", 10, &["Thrash", "Tail Whip"]);
    for s in fight_pp_spend(&yellow, &trainer(&yellow, "Brock 1", |_| {}, &state), &fastest) {
        let k = ko(&s);
        assert_eq!(k.pp, (k.turns + 2) / 3, "gen 1 Thrash locks for 3+ turns: {:?}", k);
    }

    // gen 1 Rage: one PP for the whole fight
    let state = solo(&yellow, "Mankey", 14, &["Rage", "Leer"]);
    let rage = fight_pp_spend(&yellow, &trainer(&yellow, "Brock 1", |_| {}, &state), &fastest);
    assert_eq!(rage.len(), 2);
    let total: i64 = rage.iter().map(|s| s.per_slot()[0]).sum();
    assert_eq!(total, 1);

    let crystal = reg.get_version(consts::CRYSTAL_VERSION).unwrap();
    let state = solo(&crystal, "Dratini", 15, &["Outrage", "Leer"]);
    for s in fight_pp_spend(&crystal, &trainer(&crystal, "Leader Falkner", |_| {}, &state), &fastest) {
        let k = ko(&s);
        assert_eq!(k.pp, (k.turns + 1) / 2, "gen 2 rampage: {:?}", k);
    }
    // gen 2 Rage and Rollout are per turn
    let state = solo(&crystal, "Geodude", 15, &["Rollout"]);
    for s in fight_pp_spend(&crystal, &trainer(&crystal, "Leader Falkner", |_| {}, &state), &fastest) {
        assert_eq!(ko(&s).pp, ko(&s).turns);
    }

    let emerald = reg.get_version(consts::EMERALD_VERSION).unwrap();
    let state = solo(&emerald, "Geodude", 15, &["Rollout"]);
    for s in fight_pp_spend(&emerald, &trainer(&emerald, "Leader Brawly", |_| {}, &state), &fastest) {
        let k = ko(&s);
        assert_eq!(k.pp, (k.turns + 4) / 5, "gen 3 Rollout locks for 5 turns: {:?}", k);
    }
}

// ---- Pressure --------------------------------------------------------------------------

#[test]
fn pressure_doubles_moves_that_target_it() {
    let reg = registry();
    let gen = reg.get_version(consts::EMERALD_VERSION).unwrap();
    let state = solo(&gen, "Mudkip", 30, &["Tackle", "Growl", "Swords Dance"]);
    let input = trainer(
        &gen,
        "Cooltrainer 2 Jazmyn",
        |td| td.stat_stage_setup = side_maps(1, &[("Growl", "1"), ("Swords Dance", "2")]),
        &state,
    );
    assert_eq!(input.matchups[0].0.ability, consts::PRESSURE_ABILITY);
    let spends = fight_pp_spend(&gen, &input, &cfg(consts::HIGHLIGHT_FASTEST_KILL));
    let s = &spends[0];
    let k = ko(s);
    assert!(k.pressure);
    assert_eq!(k.pp, 2 * k.turns);
    let growl = s.spends.iter().find(|x| x.move_name == "Growl").unwrap();
    assert_eq!((growl.pp, growl.pressure), (2, true));
    let sd = s.spends.iter().find(|x| x.move_name == "Swords Dance").unwrap();
    assert_eq!((sd.pp, sd.pressure), (2, false), "self-targeting setup is not doubled");

    // a wild Pressure mon too
    let absol = fight_pp_spend(&gen, &wild(&gen, "Absol", 25, &state), &cfg(consts::HIGHLIGHT_FASTEST_KILL));
    assert_eq!(ko(&absol[0]).pp, 2 * ko(&absol[0]).turns);
}

// ---- setup -----------------------------------------------------------------------------

#[test]
fn setup_pays_its_uses_and_lowers_the_ko_count() {
    let reg = registry();
    let gen = reg.get_version(consts::EMERALD_VERSION).unwrap();
    let state = solo(&gen, "Scyther", 20, &["Quick Attack", "Swords Dance", "Leer"]);
    let fastest = cfg(consts::HIGHLIGHT_FASTEST_KILL);
    let plain = fight_pp_spend(&gen, &trainer(&gen, "Leader Roxanne", |_| {}, &state), &fastest);
    let mut setup = side_maps(3, &[]);
    setup[0].player.insert("Swords Dance".into(), "2".into());
    setup[1].player.insert("Leer".into(), "1".into());
    let boosted = fight_pp_spend(&gen, &trainer(&gen, "Leader Roxanne", |td| td.stat_stage_setup = setup, &state), &fastest);

    // matchup 1: Swords Dance x2 costs 2 and the KO needs fewer turns
    assert_eq!(boosted[0].per_slot()[1], 2);
    assert!(ko(&boosted[0]).turns < ko(&plain[0]).turns, "{:?} vs {:?}", boosted[0], plain[0]);
    // matchup 2: the boost carries over for free; Leer x1 costs 1
    assert_eq!(boosted[1].per_slot()[1], 0);
    assert_eq!(boosted[1].per_slot()[2], 1);
    assert!(ko(&boosted[1]).turns <= ko(&plain[1]).turns);
    // matchup 3: nothing entered, nothing spent on setup
    assert_eq!(boosted[2].per_slot()[1] + boosted[2].per_slot()[2], 0);
}

#[test]
fn belly_drum_is_one_use_and_the_global_list_is_charged_once() {
    let reg = registry();
    let gen = reg.get_version(consts::CRYSTAL_VERSION).unwrap();
    let state = solo(&gen, "Snorlax", 30, &["Headbutt", "Belly Drum"]);
    let fastest = cfg(consts::HIGHLIGHT_FASTEST_KILL);
    let drum = fight_pp_spend(&gen, &trainer(&gen, "Leader Falkner", |td| td.stat_stage_setup = side_maps(2, &[("Belly Drum", "6")]), &state), &fastest);
    assert_eq!(drum[0].per_slot()[1], 1, "the stepper holds the stage, not a count");

    let state = solo(&gen, "Scyther", 20, &["Quick Attack", "Swords Dance"]);
    let global = fight_pp_spend(&gen, &trainer(&gen, "Leader Falkner", |td| td.setup_moves = vec!["Swords Dance".into(), "Swords Dance".into()], &state), &fastest);
    assert_eq!(global[0].per_slot()[1], 2);
    assert_eq!(global[1].per_slot()[1], 0, "the legacy list is charged once per fight");
}

#[test]
fn setup_for_moves_the_mon_no_longer_knows_is_ignored() {
    let reg = registry();
    let gen = reg.get_version(consts::EMERALD_VERSION).unwrap();
    let state = solo(&gen, "Scyther", 20, &["Quick Attack"]);
    let spends = fight_pp_spend(&gen, &trainer(&gen, "Leader Roxanne", |td| td.stat_stage_setup = side_maps(3, &[("Swords Dance", "2")]), &state), &cfg(consts::HIGHLIGHT_FASTEST_KILL));
    assert!(spends[0].spends.iter().all(|s| s.kind != SpendKind::Setup));
}

// ---- Mimic / Transform ---------------------------------------------------------------------

#[test]
fn mimic_pays_by_generation() {
    let reg = registry();
    let fastest = cfg(consts::HIGHLIGHT_FASTEST_KILL);

    // gen 1: the mimicked move uses Mimic's PP, plus the Mimic use itself
    let yellow = reg.get_version(consts::YELLOW_VERSION).unwrap();
    let state = solo(&yellow, "Clefairy", 20, &["Mimic", "Growl"]);
    let spends = fight_pp_spend(&yellow, &trainer(&yellow, "Brock 1", |td| td.mimic_selection = "Tackle".into(), &state), &fastest);
    let first = ko(&spends[0]);
    assert_eq!(first.kind, SpendKind::Mimic);
    assert_eq!(first.pp, first.turns + 1);
    let second = ko(&spends[1]);
    assert_eq!(second.pp, second.turns);

    // gen 2: the copied move has its own 5 PP; only Mimic's use costs
    let crystal = reg.get_version(consts::CRYSTAL_VERSION).unwrap();
    let state = solo(&crystal, "Clefairy", 20, &["Mimic", "Growl"]);
    let spends = fight_pp_spend(&crystal, &trainer(&crystal, "Leader Falkner", |td| td.mimic_selection = "Tackle".into(), &state), &fastest);
    let total: i64 = spends.iter().map(|s| s.per_slot()[0]).sum();
    assert_eq!(total, 1, "{:?}", spends);
}

#[test]
fn transform_pays_once_per_fight() {
    let reg = registry();
    let gen = reg.get_version(consts::YELLOW_VERSION).unwrap();
    let state = solo(&gen, "Ditto", 20, &["Transform"]);
    let spends = fight_pp_spend(&gen, &trainer(&gen, "Brock 1", |td| td.set_transformed(true), &state), &cfg(consts::HIGHLIGHT_FASTEST_KILL));
    let total: i64 = spends.iter().map(|s| s.per_slot()[0]).sum();
    assert_eq!(total, 1);
    assert_eq!(spends[0].spends[0].kind, SpendKind::Transform);
}

// ---- wild and cache ----------------------------------------------------------------------------

#[test]
fn wild_fights_use_each_items_own_state() {
    let reg = registry();
    let gen = reg.get_version(consts::EMERALD_VERSION).unwrap();
    let weak = solo(&gen, "Mudkip", 8, &["Tackle"]);
    let strong = solo(&gen, "Mudkip", 8, &["Tackle", "Water Gun"]);
    let def = EventDefinition::with_wild(WildPkmnEventDefinition::new("Geodude", 8, 2, false));
    let mon = gen.create_wild_pkmn("Geodude", 8, 15).unwrap();
    let input = FightInput { group_id: 3, def, is_wild: true, matchups: vec![(mon.clone(), weak), (mon, strong)] };
    let spends = fight_pp_spend(&gen, &input, &cfg(consts::HIGHLIGHT_FASTEST_KILL));
    assert_eq!(ko(&spends[0]).move_name, "Tackle");
    assert_eq!(ko(&spends[1]).move_name, "Water Gun", "a move learned between the two mons counts");
}

#[test]
fn the_cache_reuses_unchanged_fights() {
    let reg = registry();
    let gen = reg.get_version(consts::EMERALD_VERSION).unwrap();
    let state = solo(&gen, "Mudkip", 14, &["Tackle", "Water Gun"]);
    let fastest = cfg(consts::HIGHLIGHT_FASTEST_KILL);
    let mut a = trainer(&gen, "Leader Roxanne", |_| {}, &state);
    a.group_id = 10;
    let mut b = trainer(&gen, "Leader Brawly", |_| {}, &state);
    b.group_id = 11;
    let mut cache = FightSpendCache::new();
    let first = cache.resolve(&gen, vec![a.clone(), b.clone()], &fastest);
    assert_eq!(cache.last_computed, 2);
    assert_eq!(first.len(), 2);
    let again = cache.resolve(&gen, vec![a.clone(), b.clone()], &fastest);
    assert_eq!(cache.last_computed, 0);
    assert!(Arc::ptr_eq(&first[&10], &again[&10]));

    // a stronger mon for fight b only recomputes b
    let stronger = solo(&gen, "Mudkip", 20, &["Tackle", "Water Gun"]);
    let b2 = FightInput { matchups: b.matchups.iter().map(|(m, _)| (m.clone(), stronger.clone())).collect(), ..b.clone() };
    cache.resolve(&gen, vec![a.clone(), b2], &fastest);
    assert_eq!(cache.last_computed, 1);
    // a different strategy recomputes everything
    cache.resolve(&gen, vec![a, b], &cfg(consts::HIGHLIGHT_GUARANTEED_KILL));
    assert_eq!(cache.last_computed, 2);
}
