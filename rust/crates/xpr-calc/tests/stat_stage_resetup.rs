//! Setting a stage up again after the enemy lowers it: 6 Withdraws, the
//! enemy's 3 Leers, then 3 more Withdraws ends at +6, and in gen 1 every use
//! re-applies the badge boosts.

use std::path::PathBuf;
use std::sync::Arc;

use xpr_calc::battle_summary::{BattleSummary, SummaryConfig};
use xpr_core::consts;
use xpr_data::model::{EnemyPkmn, Nature};
use xpr_data::{exp, GenData, Registry};
use xpr_engine::state::SoloPokemonArgs;
use xpr_engine::{Inventory, RouteState, SoloPokemon};

fn registry() -> Arc<Registry> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()))
}

/// A solo mon at `level` knowing exactly `moves` (max DVs, no EVs, no badges).
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

fn enemy(gen: &GenData, species: &str, level: i64, moves: &[&str]) -> EnemyPkmn {
    let mut mon = gen.create_wild_pkmn(species, level, 8).unwrap();
    mon.move_list = moves.iter().map(|m| Some(m.to_string())).collect();
    mon
}

/// A two-mon fight against Squirtle with Withdraw and Tail Whip, where both
/// enemies know Leer and Harden.
fn summary(gen: &GenData) -> BattleSummary {
    let state = solo(gen, "Squirtle", 20, &["Tackle", "Withdraw", "Tail Whip"]);
    let enemies = vec![enemy(gen, "Ekans", 12, &["Leer", "Harden"]), enemy(gen, "Ekans", 12, &["Leer", "Harden"])];
    let mut s = BattleSummary::new();
    s.load_from_state(gen, Some(&state), &enemies, None, false, &SummaryConfig::default()).unwrap();
    s
}

fn set(s: &mut BattleSummary, mon_idx: usize, is_player: bool, move_name: &str, count: i64) {
    s.stat_stage_setup[mon_idx].side_mut(is_player).insert(move_name.to_string(), count.to_string());
}

fn stepper_max(s: &BattleSummary, mon_idx: usize, is_player: bool, move_name: &str) -> String {
    let data = if is_player { &s.player_move_data } else { &s.enemy_move_data };
    let mv = data[mon_idx].iter().flatten().find(|m| m.name == move_name).unwrap();
    mv.stat_stage_options.as_ref().unwrap().last().unwrap().clone()
}

#[test]
fn withdraw_after_the_enemys_leers_maxes_defense_again() {
    let reg = registry();
    let gen = reg.get_version(consts::YELLOW_VERSION).unwrap();
    let cfg = SummaryConfig::default();
    let mut s = summary(&gen);

    // nothing can lower defense: the stepper stops at 6 as always
    s.original_enemy_mon_list[0].move_list = vec![Some("Harden".to_string())];
    s.full_refresh(Some(&gen), &cfg);
    assert_eq!(stepper_max(&s, 0, true, "Withdraw"), "6");
    // Leer could: one more, which brings a Leer with it
    s.original_enemy_mon_list[0].move_list = vec![Some("Leer".to_string()), Some("Harden".to_string())];
    s.full_refresh(Some(&gen), &cfg);
    assert_eq!(stepper_max(&s, 0, true, "Withdraw"), "7");

    // 6 Withdraws, 3 Leers, 3 more Withdraws
    set(&mut s, 0, true, "Withdraw", 9);
    set(&mut s, 0, false, "Leer", 3);
    s.full_refresh(Some(&gen), &cfg);
    let p = s.per_matchup_player_modifiers[0];
    assert_eq!(p.defense_stage, 6);
    assert_eq!(stepper_max(&s, 0, true, "Withdraw"), "10");
    // gen 1: each of the 12 stat changes re-applied the other stats' badge
    // boosts; defense's own boost is dropped by the last Withdraw
    assert_eq!((p.attack_badge_boosts, p.speed_badge_boosts, p.special_badge_boosts, p.defense_badge_boosts), (12, 12, 12, 0));

    // the 3 extra Withdraws cost PP like any other use
    let withdraw = s.player_setup_uses(&gen, 0).into_iter().find(|u| u.move_name == "Withdraw").unwrap();
    assert_eq!(withdraw.uses, 9);
}

/// Presses the player's stepper for `move_name` to `count`, as the UI does.
fn press(s: &mut BattleSummary, gen: &GenData, mon_idx: usize, move_name: &str, count: i64) {
    let move_idx = s.player_move_data[mon_idx].iter().position(|m| m.as_ref().is_some_and(|m| m.name == move_name)).unwrap();
    s.update_stat_stage_setup(gen, &SummaryConfig::default(), mon_idx, move_idx, true, &count.to_string());
}

fn enemy_count(s: &BattleSummary, mon_idx: usize, move_name: &str) -> String {
    s.stat_stage_setup[mon_idx].enemy.get(move_name).cloned().unwrap_or_default()
}

#[test]
fn plus_at_max_adds_an_enemy_leer_and_a_withdraw() {
    let reg = registry();
    let gen = reg.get_version(consts::YELLOW_VERSION).unwrap();
    let mut s = summary(&gen);

    // up to +6 the Withdraws are plain uses
    for n in 1..=6 {
        press(&mut s, &gen, 0, "Withdraw", n);
    }
    assert_eq!(enemy_count(&s, 0, "Leer"), "");
    let p = s.per_matchup_player_modifiers[0];
    assert_eq!((p.defense_stage, p.attack_badge_boosts), (6, 6));

    // past it, each one comes with a Leer: still +6, two more badge boosts
    press(&mut s, &gen, 0, "Withdraw", 7);
    assert_eq!(enemy_count(&s, 0, "Leer"), "1");
    let p = s.per_matchup_player_modifiers[0];
    assert_eq!((p.defense_stage, p.attack_badge_boosts, p.defense_badge_boosts), (6, 8, 0));
    assert_eq!(stepper_max(&s, 0, true, "Withdraw"), "8");
    press(&mut s, &gen, 0, "Withdraw", 8);
    assert_eq!(enemy_count(&s, 0, "Leer"), "2");
    assert_eq!(s.per_matchup_player_modifiers[0].attack_badge_boosts, 10);

    // minus only takes back the Withdraw
    press(&mut s, &gen, 0, "Withdraw", 7);
    assert_eq!(enemy_count(&s, 0, "Leer"), "2");
    assert_eq!(s.per_matchup_player_modifiers[0].defense_stage, 5);

    // +5 carries into the next matchup, where one Withdraw is a plain use...
    press(&mut s, &gen, 1, "Withdraw", 1);
    assert_eq!(enemy_count(&s, 1, "Leer"), "");
    assert_eq!(s.per_matchup_player_modifiers[1].defense_stage, 6);
    // ...and the next one pairs
    press(&mut s, &gen, 1, "Withdraw", 2);
    assert_eq!(enemy_count(&s, 1, "Leer"), "1");
    assert_eq!(s.per_matchup_player_modifiers[1].defense_stage, 6);
}

#[test]
fn a_maxed_stage_carried_into_the_next_matchup_can_be_set_up_again() {
    let reg = registry();
    let gen = reg.get_version(consts::YELLOW_VERSION).unwrap();
    let cfg = SummaryConfig::default();
    let mut s = summary(&gen);

    set(&mut s, 0, true, "Withdraw", 6);
    set(&mut s, 1, false, "Leer", 3);
    set(&mut s, 1, true, "Withdraw", 3);
    s.full_refresh(Some(&gen), &cfg);
    assert_eq!(s.per_matchup_player_modifiers[0].defense_stage, 6);
    assert_eq!(s.per_matchup_player_modifiers[1].defense_stage, 6, "the Withdraws wait for the Leers");

    // without them the Leers stick
    set(&mut s, 1, true, "Withdraw", 0);
    s.full_refresh(Some(&gen), &cfg);
    assert_eq!(s.per_matchup_player_modifiers[1].defense_stage, 3);
}

#[test]
fn uses_with_room_still_come_before_the_enemys() {
    let reg = registry();
    let gen = reg.get_version(consts::YELLOW_VERSION).unwrap();
    let cfg = SummaryConfig::default();
    let mut s = summary(&gen);

    // 4 Withdraws all have room, so the Leers come after them
    set(&mut s, 0, true, "Withdraw", 4);
    set(&mut s, 0, false, "Leer", 3);
    s.full_refresh(Some(&gen), &cfg);
    assert_eq!(s.per_matchup_player_modifiers[0].defense_stage, 1);
}

#[test]
fn the_enemy_can_set_up_again_after_the_players_moves() {
    let reg = registry();
    let gen = reg.get_version(consts::YELLOW_VERSION).unwrap();
    let cfg = SummaryConfig::default();
    let mut s = summary(&gen);

    set(&mut s, 0, true, "Tail Whip", 3);
    s.full_refresh(Some(&gen), &cfg);
    assert_eq!(stepper_max(&s, 0, false, "Harden"), "9");
    set(&mut s, 0, false, "Harden", 9);
    s.full_refresh(Some(&gen), &cfg);
    assert_eq!(s.per_matchup_enemy_modifiers[0].defense_stage, 6);
}
