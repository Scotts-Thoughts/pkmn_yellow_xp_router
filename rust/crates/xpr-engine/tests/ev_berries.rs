//! Port of `tests/test_ev_berries.py`: EV-lowering berries subtract EVs by the game's rule.
//!
//! Emerald and gen 5: EV - 10, floored at 0. Platinum / HGSS subtract first and then
//! clamp anything still over 100 down to 100 (`CalculateEVUpdate` / `TryModEV`), so
//! 255 -> 100 and 110 -> 100, but 105 -> 95.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::json;
use xpr_core::consts;
use xpr_data::{Registry, StatBlock};
use xpr_engine::Router;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn registry() -> Arc<Registry> {
    Arc::new(Registry::new(repo_root().join("raw_pkmn_data"), PathBuf::new()))
}

fn is_gen4(version: &str) -> bool {
    matches!(version, "Platinum" | "HeartGold")
}

fn expected_after_berry(version: &str, ev: i64) -> i64 {
    if is_gen4(version) {
        (ev - 10).min(100).max(0)
    } else {
        (ev - 10).max(0)
    }
}

fn evs(s: &StatBlock) -> [i64; 6] {
    [s.hp, s.attack, s.defense, s.special_attack, s.special_defense, s.speed]
}

const VERSIONS: [&str; 4] = ["Emerald", "Platinum", "HeartGold", "Black"];

#[test]
fn reduction_matches_the_game() {
    let reg = registry();
    for version in VERSIONS {
        let gen = reg.get_version(version).unwrap();
        for before in [255, 252, 111, 110, 109, 105, 101, 100, 50, 10, 5, 1] {
            let after = gen.get_ev_berry_reduced_value(before);
            assert_eq!(after, expected_after_berry(version, before), "{version}: {before} EVs");
            assert!(after < before, "{version}: {before} EVs did not go down");
        }
    }
}

#[test]
fn each_berry_names_its_own_stat() {
    let reg = registry();
    let want = [
        ("Pomeg Berry", consts::HP),
        ("Kelpsy Berry", consts::ATK),
        ("Qualot Berry", consts::DEF),
        ("Hondew Berry", consts::SPA),
        ("Grepa Berry", consts::SPD),
        ("Tamato Berry", consts::SPE),
    ];
    for version in VERSIONS {
        let gen = reg.get_version(version).unwrap();
        for (berry, stat) in want {
            assert!(gen.is_ev_berry(berry), "{version}: {berry}");
            assert_eq!(gen.get_stats_lowered_by_ev_berry(berry).unwrap(), vec![stat], "{version}: {berry}");
        }
    }
    let crystal = reg.get_version("Crystal").unwrap();
    assert!(!crystal.is_ev_berry("Pomeg Berry"));
}

#[test]
fn route_events_subtract_evs_end_to_end() {
    let wild = [
        ("Emerald", "Zigzagoon", "Wurmple"),
        ("Platinum", "Starly", "Bidoof"),
        ("HeartGold", "Pidgey", "Hoothoot"),
        ("Black", "Purrloin", "Audino"),
    ];
    let dir = std::env::temp_dir().join(format!("xpr-ev-berries-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    for (version, speed_mon, hp_mon) in wild {
        let route = json!({
            "name": "Geodude",
            "Version": version,
            "ability": 0,
            "nature": 0,
            "events": [{"Event Folder Name": "ROOT", "Just Notes": "", "events": [
                {"Event Folder Name": "Main", "Just Notes": "", "Expanded": true, "Enabled": true, "events": [
                    {"Enabled": true, "Tags": [], "Inventory Event": ["Pomeg Berry", 10, true, false]},
                    {"Enabled": true, "Tags": [], "Inventory Event": ["Tamato Berry", 10, true, false]},
                    {"Enabled": true, "Tags": [], "Fight Wild Pkmn": [speed_mon, 3, 25, false]},
                    {"Enabled": true, "Tags": [], "Fight Wild Pkmn": [hp_mon, 3, 15, false]},
                    {"Enabled": true, "Tags": [], "Use Vitamin": ["Tamato Berry", 3]},
                    {"Enabled": true, "Tags": [], "Use Vitamin": ["Pomeg Berry", 1]}
                ]}
            ]}]
        });
        let path = dir.join(format!("{version}.json"));
        std::fs::write(&path, serde_json::to_vec(&route).unwrap()).unwrap();

        let mut router = Router::new(registry());
        router.load(&path, false).expect("route loads");
        let groups = router.all_groups();
        let (fight, tamato, pomeg) = (
            router.group(groups[3]).unwrap(),
            router.group(groups[4]).unwrap(),
            router.group(groups[5]).unwrap(),
        );

        let before = fight.final_state.as_ref().unwrap().solo_pkmn.unrealized_stat_xp;
        assert!(before.speed > 0 && before.hp > 0, "{version}: fights gave no speed/HP EVs");

        let mut spe = before.speed;
        for _ in 0..3 {
            spe = expected_after_berry(version, spe);
        }
        let after_tamato = &tamato.final_state.as_ref().unwrap().solo_pkmn;
        assert_eq!(after_tamato.realized_stat_xp.speed, spe, "{version}: Tamato x3");
        assert_eq!(after_tamato.unrealized_stat_xp.speed, spe, "{version}: Tamato x3");
        assert_eq!(evs(&after_tamato.realized_stat_xp)[..5], evs(&before)[..5], "{version}: Tamato touched another stat");
        assert_eq!(tamato.event_items.len(), 3);
        assert!(tamato.error_messages.is_empty(), "{version}: {:?}", tamato.error_messages);
        let tamato_left: i64 = tamato.final_state.as_ref().unwrap().inventory.cur_items.iter()
            .filter(|x| x.base_item.name == "Tamato Berry")
            .map(|x| x.num)
            .sum();
        assert_eq!(tamato_left, 7, "{version}: berries not taken from the bag");

        let after_pomeg = &pomeg.final_state.as_ref().unwrap().solo_pkmn;
        assert_eq!(after_pomeg.realized_stat_xp.hp, expected_after_berry(version, before.hp), "{version}: Pomeg");
        assert_eq!(after_pomeg.realized_stat_xp.speed, spe);
        assert!(pomeg.error_messages.is_empty(), "{version}: {:?}", pomeg.error_messages);

        let final_evs = evs(&router.get_final_state().unwrap().solo_pkmn.realized_stat_xp);
        let saved = dir.join(format!("{version}-saved.json"));
        std::fs::write(&saved, serde_json::to_vec(&router.serialize().unwrap()).unwrap()).unwrap();
        let mut reloaded = Router::new(registry());
        reloaded.load(&saved, false).expect("saved route loads");
        assert_eq!(evs(&reloaded.get_final_state().unwrap().solo_pkmn.realized_stat_xp), final_evs, "{version}: save/reload");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
