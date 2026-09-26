//! The gen 5 fight lists name trainers exactly as the trainer data does, so
//! the major fights get their highlight, leaders award their badges and the
//! gym / Elite Four hotkeys find their fights in all four games.

use std::path::PathBuf;

use xpr_data::{E4Entry, Registry};

fn registry() -> Registry {
    let raw = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../raw_pkmn_data").canonicalize().unwrap();
    Registry::new(raw, PathBuf::new())
}

#[test]
fn every_listed_fight_is_a_real_trainer() {
    let reg = registry();
    let (bw, b2w2) = (reg.get_version("Black").unwrap(), reg.get_version("Black 2").unwrap());
    // one shared file lists both games' trainers
    for name in &bw.major_fights {
        assert!(bw.trainer_db().get_trainer(name).is_some() || b2w2.trainer_db().get_trainer(name).is_some(), "{} is no trainer", name);
    }
    for name in bw.badge_rewards.keys().chain(bw.fight_rewards.keys()) {
        assert!(bw.trainer_db().get_trainer(name).is_some() || b2w2.trainer_db().get_trainer(name).is_some(), "{} is no trainer", name);
    }
}

#[test]
fn story_fights_have_their_category() {
    let reg = registry();
    let cases: &[(&str, &str, &str)] = &[
        ("Black", "⒆⒇ Trainer Cheren (54)", "rival"),
        ("White", "⒆⒇ Trainer Bianca (499)", "rival"),
        ("Black", "Leader Cress", "gym_leader"),
        ("White", "Leader Iris", "gym_leader"),
        ("Black", "Elite Four Grimsley (230)", "elite_four"),
        ("Black", "Champion Alder", "champion"),
        ("Black", "Team Plasma N (587)", "team_leader"),
        ("Black", "Team Plasma Ghetsis", "boss"),
        ("White 2", "⒆⒇ Trainer Rival (163)", "rival"),
        ("White 2", "Leader Cheren (156)", "gym_leader"),
        ("Black 2", "Leader Marlon (771)", "gym_leader"),
        ("White 2", "Elite Four Caitlin (41)", "elite_four"),
        ("White 2", "Champion Iris (341)", "champion"),
        ("White 2", "Team Plasma Zinzolin (584)", "team_leader"),
        ("White 2", "Team Plasma Shadow (583)", "team_leader"),
        ("White 2", "⒆⒇ Trainer Colress (358)", "team_leader"),
        ("Black 2", "Team Plasma Ghetsis", "boss"),
        ("Black 2", "⒆⒇ Trainer N (5)", "post_game"),
    ];
    for (version, name, category) in cases {
        let gen = reg.get_version(version).unwrap();
        assert!(gen.trainer_db().get_trainer(name).is_some(), "{}: {} is no trainer", version, name);
        assert!(gen.is_major_fight(name), "{}: {} is not a major fight", version, name);
        assert_eq!(gen.get_fight_category(name), Some(*category), "{}: {}", version, name);
    }
}

#[test]
fn leaders_award_their_badges() {
    let reg = registry();
    let w2 = reg.get_version("White 2").unwrap();
    let badges = w2.make_badge_list().award_badge("Leader Cheren (156)").award_badge("Leader Marlon (771)");
    assert!(badges.has("basic") && badges.has("wave"));
    assert_eq!(badges.num_badges(), 2);
    let black = reg.get_version("Black").unwrap();
    assert!(black.make_badge_list().award_badge("Leader Chili").has("trio"));
}

#[test]
fn every_game_has_eight_gyms_and_five_league_fights() {
    let reg = registry();
    for version in ["Black", "White", "Black 2", "White 2"] {
        let gen = reg.get_version(version).unwrap();
        let gyms = gen.get_gym_leader_entries();
        let league = gen.get_elite_four_and_champion_names();
        assert_eq!(gyms.len(), 8, "{}: {:?}", version, gyms);
        assert_eq!(league.len(), 5, "{}: {:?}", version, league);
        for name in gyms.iter().chain(league.iter()).flat_map(E4Entry::names) {
            assert!(gen.trainer_db().get_trainer(&name).is_some(), "{}: {} is not in this game", version, name);
        }
    }
    let white2 = reg.get_version("White 2").unwrap();
    // the normal team first, then Challenge Mode's
    assert_eq!(white2.get_gym_leader_entries()[0].names(), vec!["Leader Cheren (156)".to_string(), "Leader Cheren (764)".to_string()]);
    assert_eq!(white2.get_elite_four_and_champion_names()[4].names()[0], "Champion Iris (341)");
    let black = reg.get_version("Black").unwrap();
    assert_eq!(black.get_gym_leader_entries()[0].names().len(), 3, "the Striaton trio");
}
