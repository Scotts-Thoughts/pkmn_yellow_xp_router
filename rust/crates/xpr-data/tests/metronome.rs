//! Which moves Metronome can call, per generation (tables and sources in
//! `gen_consts`).

use std::path::PathBuf;
use xpr_data::gen_consts::{metronome_uncallable_moves, METRONOME_GRAVITY_BLOCKED};
use xpr_data::model::Gen;
use xpr_data::Registry;

fn registry() -> Registry {
    let raw = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../raw_pkmn_data").canonicalize().unwrap();
    Registry::new(raw, PathBuf::new())
}

fn known(moves: &[&str]) -> Vec<Option<String>> {
    moves.iter().map(|m| Some(m.to_string())).collect()
}

#[test]
fn every_table_entry_is_a_move_of_its_generation() {
    let reg = registry();
    for (version, gen) in [("Yellow", Gen::One), ("Crystal", Gen::Two), ("Emerald", Gen::Three), ("Platinum", Gen::Four), ("Black", Gen::Five)] {
        let data = reg.get_version(version).unwrap();
        let mut names: Vec<&str> = metronome_uncallable_moves(gen).to_vec();
        if gen == Gen::Four {
            names.extend(METRONOME_GRAVITY_BLOCKED);
        }
        for name in names {
            let mv = data.move_db().get_move(name).unwrap_or_else(|| panic!("{}: no move {}", version, name));
            assert_eq!(mv.name, name, "{}: spelling", version);
        }
    }
}

#[test]
fn callable_counts_match_each_games_move_table() {
    let reg = registry();
    // every move of the game minus its uncallable table (gen 5: 559 moves,
    // the Shadow moves in the data are not loaded)
    for (version, count) in [
        ("Red", 165 - 2),
        ("Yellow", 165 - 2),
        ("Gold", 251 - 12),
        ("Crystal", 251 - 12),
        ("Ruby", 354 - 18),
        ("Emerald", 354 - 18),
        ("FireRed", 354 - 18),
        ("Diamond", 467 - 25),
        ("Platinum", 467 - 25),
        ("HeartGold", 467 - 25),
        ("Black", 559 - 41),
        ("Black 2", 559 - 41),
    ] {
        let data = reg.get_version(version).unwrap();
        assert_eq!(data.metronome_callable_moves(&[], false).len(), count, "{}", version);
    }
}

#[test]
fn gens_2_and_4_skip_the_users_own_moves() {
    let reg = registry();
    let user = known(&["Metronome", "Double Slap", "Sing", "Tackle"]);
    for (version, skips) in [("Yellow", false), ("Crystal", true), ("Emerald", false), ("Diamond", true), ("HeartGold", true), ("White", false)] {
        let data = reg.get_version(version).unwrap();
        assert_eq!(!data.metronome_can_call("Double Slap", &user, false), skips, "{}", version);
        assert!(data.metronome_can_call("Body Slam", &user, false), "{}", version);
    }
}

#[test]
fn gravity_blocks_fly_in_platinum_and_hgss_only() {
    let reg = registry();
    for (version, blocked) in [("Diamond", false), ("Pearl", false), ("Platinum", true), ("HeartGold", true), ("SoulSilver", true), ("Black", false)] {
        let data = reg.get_version(version).unwrap();
        assert!(data.metronome_can_call("Fly", &[], false), "{}", version);
        assert_eq!(!data.metronome_can_call("Fly", &[], true), blocked, "{}", version);
        assert!(data.metronome_can_call("Earthquake", &[], true), "{}", version);
    }
}

#[test]
fn uncallable_moves_are_rejected() {
    let reg = registry();
    let cases = [
        ("Yellow", &["Metronome", "Struggle"][..], &["Counter", "Mimic", "Transform"][..]),
        ("Crystal", &["Counter", "Thief", "Sleep Talk"][..], &["Mirror Move", "Transform"][..]),
        ("Emerald", &["Focus Punch", "Covet"][..], &["Assist", "Mirror Move"][..]),
        ("Platinum", &["Chatter", "Me First", "Assist"][..], &["Transform", "Snore"][..]),
        ("Black", &["V-create", "Snarl", "Transform", "Snore"][..], &["Fusion Flare", "Hidden Power"][..]),
    ];
    for (version, rejected, allowed) in cases {
        let data = reg.get_version(version).unwrap();
        for m in rejected {
            assert!(!data.metronome_can_call(m, &[], false), "{} {}", version, m);
        }
        for m in allowed {
            assert!(data.metronome_can_call(m, &[], false), "{} {}", version, m);
        }
    }
}
