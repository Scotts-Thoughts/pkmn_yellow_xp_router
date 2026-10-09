//! Which game a ROM is, and the mapper the router's recorder runs against.

use xpr_core::consts;

/// The mapper (path under the mapper folder) and the router version for a ROM.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameChoice {
    pub mapper: &'static str,
    pub version: &'static str,
}

/// The ROM's header title (GB/GBC) or game code (GBA/NDS).
pub fn rom_code(rom: &[u8]) -> Option<String> {
    // up to the first byte that is not printable ASCII (a GBC title runs into its CGB flag)
    let text = |r: std::ops::Range<usize>| -> Option<String> {
        let b = rom.get(r)?;
        let end = b.iter().position(|&c| !(0x20..0x7F).contains(&c)).unwrap_or(b.len());
        Some(String::from_utf8_lossy(&b[..end]).trim().to_string())
    };
    // NDS: game code at 0x0C; GBA: at 0xAC (with the fixed 0x96 byte at 0xB2)
    if rom.len() > 0x200 && rom.get(0xB2) == Some(&0x96) {
        return text(0xAC..0xB0);
    }
    if rom.len() > 0x4000 && rom.get(0x104..0x108) == Some(&[0xCE, 0xED, 0x66, 0x66]) {
        // the GB logo: a Game Boy (Color) ROM
        return text(0x134..0x143);
    }
    text(0x0C..0x10)
}

/// The mapper and version for the ROM. The mappers are the ones the recorders
/// were built against: the stock ones for gens 1, 2 and 5 (gen 5 with the
/// save-counter patch), the project's own `ST` forks for gens 3 and 4 (their
/// saves and heals are signals the STP ROM patch writes).
pub fn choose(rom: &[u8]) -> Result<GameChoice, String> {
    let code = rom_code(rom).ok_or("the ROM is too short to have a header")?;
    let c = |mapper, version| Ok(GameChoice { mapper, version });
    let upper = code.to_uppercase();
    match upper.as_str() {
        "POKEMON RED" => c("STANDARD/gen1/pokemon_red_blue.xml", consts::RED_VERSION),
        "POKEMON BLUE" => c("STANDARD/gen1/pokemon_red_blue.xml", consts::BLUE_VERSION),
        "POKEMON YELLOW" => c("STANDARD/gen1/pokemon_yellow.xml", consts::YELLOW_VERSION),
        "PM_CRYSTAL" => c("STANDARD/gen2/pokemon_crystal.xml", consts::CRYSTAL_VERSION),
        _ => match upper.get(0..3) {
            // the deprecated Emerald mapper reads the STP patch's save marker at its 1.5 address;
            // this one finds the patch version (1.6 moved it), with the same paths otherwise
            Some("BPE") => c("ST/gen3/pokemon_emerald_ne.xml", consts::EMERALD_VERSION),
            Some("BPR") => c("ST/gen3/pokemon_firered_leafgreen_deprecated_ne.xml", consts::FIRE_RED_VERSION),
            Some("BPG") => c("ST/gen3/pokemon_firered_leafgreen_deprecated_ne.xml", consts::LEAF_GREEN_VERSION),
            Some("CPU") => c("ST/gen4/pokemon_platinum_ne.xml", consts::PLATINUM_VERSION),
            Some("IPK") => c("ST/gen4/pokemon_heartgold_soulsilver_ne.xml", consts::HEART_GOLD_VERSION),
            Some("IPG") => c("ST/gen4/pokemon_heartgold_soulsilver_ne.xml", consts::SOUL_SILVER_VERSION),
            Some("IRB") => c("STANDARD/gen5/pokemon_black.xml", consts::BLACK_VERSION),
            Some("IRA") => c("STANDARD/gen5/pokemon_white.xml", consts::WHITE_VERSION),
            Some("IRE") => c("STANDARD/gen5/pokemon_black_2.xml", consts::BLACK_2_VERSION),
            Some("IRD") => c("STANDARD/gen5/pokemon_white_2.xml", consts::WHITE_2_VERSION),
            _ => Err(format!(
                "'{code}' is not a game the recorder supports (Red, Blue, Yellow, Crystal, Emerald, FireRed, LeafGreen, Platinum, HeartGold, SoulSilver, Black, White, Black 2, White 2)"
            )),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gb(title: &str) -> Vec<u8> {
        let mut rom = vec![0u8; 0x8000];
        rom[0x104..0x108].copy_from_slice(&[0xCE, 0xED, 0x66, 0x66]);
        rom[0x134..0x134 + title.len()].copy_from_slice(title.as_bytes());
        rom
    }

    #[test]
    fn headers_pick_the_mapper() {
        assert_eq!(choose(&gb("POKEMON YELLOW")).unwrap().version, consts::YELLOW_VERSION);
        assert_eq!(choose(&gb("PM_CRYSTAL")).unwrap().mapper, "STANDARD/gen2/pokemon_crystal.xml");
        let mut gba = vec![0u8; 0x400];
        gba[0xAC..0xB0].copy_from_slice(b"BPEE");
        gba[0xB2] = 0x96;
        assert_eq!(choose(&gba).unwrap().version, consts::EMERALD_VERSION);
        let mut nds = vec![0u8; 0x400];
        nds[0x0C..0x10].copy_from_slice(b"IRDO");
        assert_eq!(choose(&nds).unwrap().mapper, "STANDARD/gen5/pokemon_white_2.xml");
        assert!(choose(&gb("POKEMON_GLD")).is_err());
    }
}
