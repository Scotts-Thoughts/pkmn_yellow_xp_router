//! Download: the player's ability raises Attack or Sp. Atk once on entering
//! the fight, chosen from the opposing lead's Defense and Sp. Def.

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

fn solo(gen: &GenData) -> RouteState {
    let def = gen.pkmn_db().get_pkmn("Porygon").unwrap().clone();
    let xp = exp::level_lookup(&def.growth_rate).unwrap().get_xp_for_level(30).unwrap();
    let mon = SoloPokemon::new(
        "Porygon",
        def,
        gen.make_stat_block(15, 15, 15, 15, 15, 15, false),
        gen.make_badge_list(),
        gen.make_stat_block(0, 0, 0, 0, 0, 0, true),
        0,
        Nature::default(),
        SoloPokemonArgs { move_list: Some(vec![Some("Tackle".to_string()), Some("Psybeam".to_string()), None, None]), cur_xp: xp, ..Default::default() },
    )
    .unwrap();
    RouteState::new(mon, gen.make_badge_list(), Inventory::new(None, Vec::new(), gen.bag_limit()))
}

fn enemy(gen: &GenData, defense: i64, special_defense: i64, slot: i64) -> EnemyPkmn {
    let mut mon = gen.create_wild_pkmn("Bidoof", 20, 8).unwrap();
    mon.cur_stats.defense = defense;
    mon.cur_stats.special_defense = special_defense;
    mon.definition_order = slot;
    mon
}

/// A fight against `enemies` where the player has `ability`.
fn summary(gen: &GenData, enemies: &[EnemyPkmn], ability: &str) -> BattleSummary {
    let mut s = BattleSummary::new();
    s.load_from_state(gen, Some(&solo(gen)), enemies, None, false, &SummaryConfig::default()).unwrap();
    for mon in &mut s.original_player_mon_list {
        mon.ability = ability.to_string();
    }
    s.full_refresh(Some(gen), &SummaryConfig::default());
    s
}

fn stages(s: &BattleSummary, mon_idx: usize) -> (i64, i64) {
    let p = s.per_matchup_player_modifiers[mon_idx];
    (p.attack_stage, p.special_attack_stage)
}

fn tackle_max(s: &BattleSummary, mon_idx: usize) -> i64 {
    s.player_move_data[mon_idx].iter().flatten().find(|m| m.name == "Tackle").unwrap().max_damage
}

#[test]
fn lower_defense_raises_attack_otherwise_sp_atk() {
    let reg = registry();
    let gen = reg.get_version(consts::PLATINUM_VERSION).unwrap();

    let s = summary(&gen, &[enemy(&gen, 30, 40, 0)], consts::DOWNLOAD_ABILITY);
    assert_eq!(s.player_download_stat(), Some(consts::ATK));
    assert_eq!(stages(&s, 0), (1, 0));

    let s = summary(&gen, &[enemy(&gen, 40, 30, 0)], consts::DOWNLOAD_ABILITY);
    assert_eq!(s.player_download_stat(), Some(consts::SPA));
    assert_eq!(stages(&s, 0), (0, 1));

    // a tie goes to Sp. Atk (`sumDef >= sumSpDef`)
    let s = summary(&gen, &[enemy(&gen, 40, 40, 0)], consts::DOWNLOAD_ABILITY);
    assert_eq!(s.player_download_stat(), Some(consts::SPA));
}

#[test]
fn other_abilities_get_no_boost() {
    let reg = registry();
    let gen = reg.get_version(consts::PLATINUM_VERSION).unwrap();
    let s = summary(&gen, &[enemy(&gen, 30, 40, 0)], "Trace");
    assert_eq!(s.player_download_stat(), None);
    assert_eq!(stages(&s, 0), (0, 0));
}

#[test]
fn the_boost_comes_from_the_lead_and_lasts_the_whole_fight() {
    let reg = registry();
    let gen = reg.get_version(consts::PLATINUM_VERSION).unwrap();
    // the second mon would pick the other stat, and must not stack or change it
    let s = summary(&gen, &[enemy(&gen, 30, 40, 0), enemy(&gen, 40, 30, 1), enemy(&gen, 30, 40, 2)], consts::DOWNLOAD_ABILITY);
    assert_eq!(stages(&s, 0), (1, 0));
    assert_eq!(stages(&s, 1), (1, 0));
    assert_eq!(stages(&s, 2), (1, 0));
}

#[test]
fn a_reordered_fight_still_uses_party_slot_0() {
    let reg = registry();
    let gen = reg.get_version(consts::PLATINUM_VERSION).unwrap();
    // slot 1 dragged to the top of the summary; slot 0 still leads in game
    let s = summary(&gen, &[enemy(&gen, 40, 30, 1), enemy(&gen, 30, 40, 0)], consts::DOWNLOAD_ABILITY);
    assert_eq!(s.player_download_stat(), Some(consts::ATK));
}

#[test]
fn a_double_battle_totals_both_leads() {
    let reg = registry();
    let gen = reg.get_version(consts::PLATINUM_VERSION).unwrap();
    // the first lead alone says Attack, both together say Sp. Atk; the third
    // isn't on the field yet
    let mut s = summary(&gen, &[enemy(&gen, 30, 40, 0), enemy(&gen, 60, 30, 1), enemy(&gen, 1, 500, 2)], consts::DOWNLOAD_ABILITY);
    assert_eq!(s.player_download_stat(), Some(consts::ATK));
    s.double_battle_flag = true;
    assert_eq!(s.player_download_stat(), Some(consts::SPA));
}

#[test]
fn the_boost_reaches_the_damage_ranges() {
    let reg = registry();
    let gen = reg.get_version(consts::PLATINUM_VERSION).unwrap();
    let enemies = [enemy(&gen, 30, 40, 0)];
    let plain = summary(&gen, &enemies, "Trace");
    let boosted = summary(&gen, &enemies, consts::DOWNLOAD_ABILITY);
    assert!(tackle_max(&boosted, 0) > tackle_max(&plain, 0));
}

#[test]
fn global_setup_moves_keep_the_boost() {
    let reg = registry();
    let gen = reg.get_version(consts::PLATINUM_VERSION).unwrap();
    let enemies = [enemy(&gen, 30, 40, 0)];
    let cfg = SummaryConfig::default();
    let mut s = summary(&gen, &enemies, consts::DOWNLOAD_ABILITY);
    s.player_setup_move_list = vec!["Agility".to_string()];
    s.full_refresh(Some(&gen), &cfg);
    assert!(s.per_matchup_player_modifiers.is_empty());
    assert_eq!(s.player_stage_modifier.attack_stage, 1);
    assert_eq!(s.player_stage_modifier.speed_stage, 2);
    // a second refresh must not stack it
    s.full_refresh(Some(&gen), &cfg);
    assert_eq!(s.player_stage_modifier.attack_stage, 1);
}
