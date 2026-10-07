//! The league in battle order: `order_area_trainers` (Add Area) and
//! `get_league_lineup` (the quick-add popover's Elite4 buttons).

use std::path::PathBuf;

use xpr_data::Registry;

fn registry() -> Registry {
    let raw = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../raw_pkmn_data").canonicalize().unwrap();
    let reg = Registry::new(raw, PathBuf::new());
    assert!(reg.load_all_builtin().is_empty());
    reg
}

fn area(reg: &Registry, version: &str, loc: &str, rematches: bool) -> Vec<String> {
    let g = reg.get_version(version).unwrap();
    g.order_area_trainers(g.trainer_db().get_valid_trainers(None, Some(loc), &[], rematches, false))
}

#[test]
fn crystal_indigo_plateau_is_in_battle_order() {
    let reg = registry();
    let expected = [
        "Rival2 2 Chikorita",
        "Rival2 2 Cyndaquil",
        "Rival2 2 Totodile",
        "Elite Four Will",
        "Elite Four Koga",
        "Elite Four Bruno",
        "Elite Four Karen",
        "Champion Lance",
    ];
    assert_eq!(area(&reg, "Crystal", "Indigo Plateau", false), expected);
    assert_eq!(area(&reg, "Gold", "Indigo Plateau", false), expected[3..]);
}

#[test]
fn rematches_follow_the_first_run() {
    let reg = registry();
    let plat = area(&reg, "Platinum", "Pokémon League", true);
    assert_eq!(&plat[3..8], ["Elite Four Aaron", "Elite Four Bertha", "Elite Four Flint", "Elite Four Lucian", "Champion Cynthia"]);
    assert_eq!(plat[8], "Elite Four Aaron Rematch 2");
    let hgss = area(&reg, "HeartGold", "Pokémon League", true);
    assert_eq!(&hgss[3..8], ["Elite Four Will", "Elite Four Koga", "Elite Four Bruno", "Elite Four Karen", "Champion Lance"]);
    assert_eq!(hgss[12], "Champion Lance Rematch 2");
    // the champion's alternatives after the Elite Four
    assert_eq!(area(&reg, "Yellow", "Indigo Plateau", false)[..5], ["Lorelei 1", "Bruno 1", "Agatha 1", "Lance 1", "Rival3 Jolteon"]);
}

#[test]
fn areas_outside_the_league_keep_their_order() {
    let reg = registry();
    for v in reg.get_gen_names(true, false) {
        let g = reg.get_version(&v).unwrap();
        for loc in g.trainer_db().get_all_locations() {
            let names = g.trainer_db().get_valid_trainers(None, Some(&loc), &[], true, false);
            let mut ordered = g.order_area_trainers(names.clone());
            let league = g.get_league_lineup().map(|(e4, _)| e4[0].names()).unwrap_or_default();
            if !names.iter().any(|n| league.contains(n)) {
                assert_eq!(ordered, names, "{v} {loc}");
            }
            ordered.sort();
            let mut sorted = names;
            sorted.sort();
            assert_eq!(ordered, sorted, "{v} {loc}: a permutation");
        }
    }
}

#[test]
fn every_game_has_a_full_lineup() {
    let reg = registry();
    for v in reg.get_gen_names(true, false) {
        let (e4, champ) = reg.get_version(&v).unwrap().get_league_lineup().unwrap_or_else(|| panic!("{v}"));
        assert_eq!(e4.len(), 4, "{v}");
        assert!(champ.is_some(), "{v}");
    }
    let (e4, champ) = reg.get_version("Crystal").unwrap().get_league_lineup().unwrap();
    let names: Vec<String> = e4.iter().chain(champ.as_ref()).flat_map(|e| e.names()).collect();
    assert_eq!(names, ["Elite Four Will", "Elite Four Koga", "Elite Four Bruno", "Elite Four Karen", "Champion Lance"]);
    let (_, champ) = reg.get_version("Ruby").unwrap().get_league_lineup().unwrap();
    assert_eq!(champ.unwrap().names(), ["Champion Steven"]);
}
