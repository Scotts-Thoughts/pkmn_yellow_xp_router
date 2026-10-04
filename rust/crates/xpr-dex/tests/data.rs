//! The data layer against facts Solodex shows for the same data.

use xpr_dex::*;

#[test]
fn every_game_loads_and_the_index_covers_it() {
    for g in GAMES {
        let all = get_all_pokemon_for_game(g);
        assert!(all.len() > 100, "{} has only {} species", g, all.len());
        assert!(all.windows(2).all(|w| w[0].national_dex_number <= w[1].national_dex_number), "{} not sorted by dex", g);
        for p in all.iter() {
            assert!(get_games_for_pokemon(&p.species).iter().any(|x| x == g), "{} / {} missing from the species index", g, p.species);
        }
    }
    assert!(get_all_pokemon().len() > 1200);
    assert_eq!(get_all_pokemon()[0].name, "Bulbasaur");
}

#[test]
fn families_follow_solodex_rules() {
    let fam = |name: &str, game: &str| -> Vec<String> { get_pokemon_data(name, game).unwrap().evolution_family.iter().map(|e| e.species.clone()).collect() };
    // Galarian Meowth evolves into Perrserker, not Persian
    let gm = fam("Galarian Meowth", "Sword and Shield");
    assert!(gm.contains(&"Perrserker".to_string()), "{:?}", gm);
    assert!(!gm.contains(&"Persian".to_string()), "{:?}", gm);
    // base Linoone does not show Obstagoon
    let lin = fam("Linoone", "Sword and Shield");
    assert!(!lin.contains(&"Obstagoon".to_string()), "{:?}", lin);
    // Megas are appended to the family in X and Y
    let ch = get_pokemon_data("Charizard", "X and Y").unwrap();
    let megas: Vec<&EvolutionEntry> = ch.evolution_family.iter().filter(|e| e.method.as_deref() == Some("mega")).collect();
    assert_eq!(megas.len(), 2, "{:?}", ch.evolution_family);
    // no megas in gen 3
    assert!(get_pokemon_data("Charizard", "Emerald").unwrap().evolution_family.iter().all(|e| e.method.as_deref() != Some("mega")));
}

#[test]
fn learnsets_use_the_games_spelling() {
    let p = get_pokemon_data("Pikachu", "Emerald").unwrap();
    assert!(p.level_up_learnset.iter().any(|(_, m)| m == "ThunderShock"));
    let p = get_pokemon_data("Pikachu", "Scarlet and Violet").unwrap();
    assert!(p.level_up_learnset.iter().any(|(_, m)| m == "Thunder Shock"));
    // transfer learnsets only exist in gen 1
    assert!(get_pokemon_data("Pikachu", "Crystal").unwrap().transfer_learnset.is_empty());
    let md = get_move_data("Faint Attack", "Emerald").unwrap();
    assert_eq!(md.move_type, "Dark");
    assert_eq!(get_move_data("Hypnosis", "Diamond and Pearl").unwrap().accuracy, Some(70));
    assert_eq!(get_move_data("Hypnosis", "Platinum").unwrap().accuracy, Some(60));
}

#[test]
fn tm_codes_and_type_charts() {
    assert_eq!(tmhm_code("Whirlpool", "HeartGold and SoulSilver").as_deref(), Some("HM05"));
    assert_eq!(tmhm_code("Defog", "HeartGold and SoulSilver"), None);
    assert_eq!(tmhm_code("Defog", "Platinum").as_deref(), Some("HM05"));
    assert_eq!(tmhm_code("Solar Beam", "Red and Blue").as_deref(), Some("TM22"));
    let ghost = type_matchups("Ghost", Some("Red and Blue"));
    assert!(ghost.no_eff_vs.contains(&"Psychic".to_string()), "{:?}", ghost);
    assert!(!types_for_game("Red and Blue").contains(&"Steel".to_string()));
    assert!(types_for_game("X and Y").contains(&"Fairy".to_string()));
    assert_eq!(offensive_multiplier("Dragon", "Fairy", "X and Y"), 0.0);
    assert_eq!(offensive_multiplier("Ghost", "Steel", "Black"), 0.5);
    assert_eq!(offensive_multiplier("Ghost", "Steel", "X and Y"), 1.0);
    let d = defense_matchups("Water", "Ground", "Emerald");
    assert_eq!(d.get("Grass"), Some(&4.0));
    assert_eq!(d.get("Electric"), Some(&0.0));
}

#[test]
fn rankings_and_lookups() {
    let speed = get_ranking(RankKind::Stat(StatKey::Speed), "Red and Blue", None);
    assert_eq!(speed[0].name, "Electrode");
    assert_eq!(speed[0].rank, 1);
    // ties share a rank
    for w in speed.windows(2) {
        if w[0].value == w[1].value {
            assert_eq!(w[0].rank, w[1].rank);
        }
    }
    let total = get_ranking(RankKind::Total, "Red and Blue", None);
    assert_eq!(total[0].name, "Mewtwo");
    assert_eq!(resolve_species("Nidoran♀"), Some("Nidoran_F"));
    assert_eq!(resolve_species("NIDORAN_M"), Some("Nidoran_M"));
    assert_eq!(resolve_species("Farfetch\u{2019}d"), Some("Farfetch'd"));
    assert_eq!(resolve_species("MrMime"), Some("Mr. Mime"));
    assert!(!get_encounters_for_pokemon("Emerald", "Abra").is_empty());
    assert!(get_encounters_for_pokemon("Scarlet and Violet", "Abra").is_empty());
    assert!(xpr_dex::sprites::sprite_bytes("Pikachu", 25).is_some());
    assert!(xpr_dex::sprites::sprite_bytes("Mega Charizard X", 6).is_some());
    let ubst = get_pokemon_ubst("Chansey", "Red and Blue").unwrap();
    assert!(ubst > 0);
    let defaults = default_moves_at_level(&get_pokemon_data("Bulbasaur", "Red and Blue").unwrap(), 20);
    assert_eq!(defaults.len(), 4);
}
