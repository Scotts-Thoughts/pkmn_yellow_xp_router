//! Gen 5 experience scales the species' base yield by level, so a wrong yield
//! shows up in every fight against that species. These are the values in the
//! ROMs' personal data (a/0/1/6, offset 0x22; a form has its own entry) wherever
//! the data files used to hold a later generation's number. Black and White
//! agree with each other, as do Black 2 and White 2, patched and unpatched.

use std::path::PathBuf;

use xpr_data::exp::calc_xp_yield_gen5;
use xpr_data::Registry;

fn registry() -> Registry {
    let raw = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../raw_pkmn_data").canonicalize().unwrap();
    Registry::new(raw, PathBuf::new())
}

const BLACK_WHITE: &[(&str, i64)] = &[
    ("Butterfree", 173), ("Beedrill", 173), ("Pidgeot", 211), ("Arbok", 153), ("Pikachu", 105),
    ("Raichu", 214), ("Nidoqueen", 223), ("Nidoking", 223), ("Clefable", 213), ("Wigglytuff", 191),
    ("Vileplume", 216), ("Dugtrio", 142), ("Poliwrath", 225), ("Alakazam", 221),
    ("Victreebel", 216), ("Golem", 218), ("Farfetch’d", 123), ("Dodrio", 161), ("Electrode", 168),
    ("Exeggutor", 182), ("Noctowl", 155), ("Ariados", 137), ("Ampharos", 225), ("Bellossom", 216),
    ("Azumarill", 185), ("Jumpluff", 203), ("Qwilfish", 86), ("Magcargo", 144), ("Corsola", 133),
    ("Mantine", 163), ("Poochyena", 44), ("Zigzagoon", 48), ("Wurmple", 39), ("Beautifly", 173),
    ("Cascoon", 41), ("Dustox", 135), ("Swellow", 151), ("Pelipper", 151), ("Masquerain", 145),
    ("Exploud", 216), ("Delcatty", 133), ("Volbeat", 140), ("Illumise", 140), ("Lunatone", 154),
    ("Solrock", 154), ("Chimecho", 149), ("Staraptor", 214), ("Roserade", 227),
    ("Giratina (Origin)", 306), ("Snivy", 28), ("Tepig", 28), ("Oshawott", 28), ("Stoutland", 221),
    ("Unfezant", 215), ("Gigalith", 227), ("Woobat", 63), ("Seismitoad", 225), ("Leavanny", 221),
    ("Scolipede", 214), ("Krookodile", 229), ("Crustle", 166), ("Beartic", 170), ("Cryogonal", 170),
];

const BLACK2_WHITE2: &[(&str, i64)] = &[
    ("Butterfree", 173), ("Beedrill", 173), ("Pidgeot", 211), ("Arbok", 153), ("Pikachu", 105),
    ("Raichu", 214), ("Nidoqueen", 223), ("Nidoking", 223), ("Clefable", 213), ("Wigglytuff", 191),
    ("Vileplume", 216), ("Dugtrio", 142), ("Poliwrath", 225), ("Alakazam", 221),
    ("Victreebel", 216), ("Golem", 218), ("Farfetch’d", 123), ("Dodrio", 161), ("Electrode", 168),
    ("Exeggutor", 182), ("Noctowl", 155), ("Ariados", 137), ("Ampharos", 225), ("Bellossom", 216),
    ("Azumarill", 185), ("Jumpluff", 203), ("Qwilfish", 86), ("Magcargo", 144), ("Corsola", 133),
    ("Mantine", 163), ("Poochyena", 44), ("Zigzagoon", 48), ("Wurmple", 39), ("Beautifly", 173),
    ("Cascoon", 41), ("Dustox", 135), ("Swellow", 151), ("Pelipper", 151), ("Masquerain", 145),
    ("Exploud", 216), ("Delcatty", 133), ("Volbeat", 140), ("Illumise", 140), ("Lunatone", 154),
    ("Solrock", 154), ("Chimecho", 149), ("Staraptor", 214), ("Roserade", 227),
    ("Giratina (Origin)", 306), ("Stoutland", 221), ("Unfezant", 215), ("Gigalith", 227),
    ("Woobat", 63), ("Seismitoad", 225), ("Leavanny", 221), ("Scolipede", 214), ("Krookodile", 229),
    ("Crustle", 166), ("Beartic", 170), ("Cryogonal", 170), ("Tornadus (Therian)", 261),
    ("Thundurus (Therian)", 261), ("Landorus (Therian)", 270), ("Kyurem (Black)", 315),
    ("Kyurem (White)", 315), ("Keldeo (Resolute)", 261),
];

#[test]
fn base_yields_match_the_roms() {
    let reg = registry();
    for (version, table) in [("Black", BLACK_WHITE), ("White", BLACK_WHITE), ("Black 2", BLACK2_WHITE2), ("White 2", BLACK2_WHITE2)] {
        let gen = reg.get_version(version).unwrap();
        for (name, base_xp) in table {
            let species = gen.pkmn_db().get_pkmn(name).unwrap_or_else(|| panic!("{} has no {}", version, name));
            assert_eq!(species.base_xp, *base_xp, "{} {}", version, name);
        }
    }
}

#[test]
fn black_starters_give_what_the_game_gave() {
    // b-victini-1-12015: Lv5 Victini beats Bianca's Snivy Lv5 and goes from 156 to
    // 199 exp, then Cheren's Oshawott Lv5 and goes from 199 to 242
    let reg = registry();
    let gen = reg.get_version("Black").unwrap();
    for name in ["Snivy", "Oshawott"] {
        let base = gen.pkmn_db().get_pkmn(name).unwrap().base_xp;
        assert_eq!(calc_xp_yield_gen5(base, 5, 5, true, 1, false), 43, "{}", name);
    }
    // Black 2 and White 2 did raise the starters' yield
    let b2 = reg.get_version("Black 2").unwrap();
    assert_eq!(b2.pkmn_db().get_pkmn("Snivy").unwrap().base_xp, 62);
}
