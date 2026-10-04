//! Species-name normalisation (Solodex `data/speciesIndex.ts`) and lookups
//! that bridge the router's species names to the Dex's.

/// Names that differ across gens, normalised to one key (applied to the
/// data by `tools/dex_data/sync.mjs`; kept here to resolve outside names).
pub const SPECIES_ALIASES: [(&str, &str); 12] = [
    ("Nidoran♀", "Nidoran_F"),
    ("Nidoran♂", "Nidoran_M"),
    ("Farfetch\u{2019}d", "Farfetch'd"),
    ("Galarian Farfetch\u{2019}d", "Galarian Farfetch'd"),
    ("Sirfetch\u{2019}d", "Sirfetch'd"),
    ("Wormadam (Plant Cloak)", "Wormadam"),
    ("Wormadam (Sandy Cloak)", "Wormadam (Sandy)"),
    ("Wormadam (Trash Cloak)", "Wormadam (Trash)"),
    ("Giratina", "Giratina (Altered)"),
    ("Shaymin", "Shaymin (Land)"),
    ("Deoxys", "Deoxys (Normal)"),
    ("Meloetta", "Meloetta (Aria)"),
];

pub fn species_alias(name: &str) -> Option<&'static str> {
    SPECIES_ALIASES.iter().find(|(from, _)| *from == name).map(|(_, to)| *to)
}

/// The name shown for an internal key (`Nidoran_F` -> `Nidoran♀`).
pub fn display_name(name: &str) -> &str {
    match name {
        "Nidoran_F" => "Nidoran♀",
        "Nidoran_M" => "Nidoran♂",
        other => other,
    }
}

/// Inverse of [`display_name`] for names coming from elsewhere.
pub fn internal_name(name: &str) -> &str {
    match name {
        "Nidoran♀" => "Nidoran_F",
        "Nidoran♂" => "Nidoran_M",
        other => other,
    }
}

/// A lowercase alphanumeric key that ignores punctuation, spacing, the
/// gender symbols and apostrophe styles: "Nidoran♀", "Nidoran F",
/// "NIDORAN_F" and "nidoranf" all reduce to "nidoranf".
pub fn loose_key(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        match c {
            '♀' => out.push('f'),
            '♂' => out.push('m'),
            'é' | 'É' => out.push('e'),
            c if c.is_alphanumeric() => out.extend(c.to_lowercase()),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys() {
        assert_eq!(loose_key("Nidoran♀"), loose_key("NIDORAN_F"));
        assert_eq!(loose_key("Farfetch\u{2019}d"), loose_key("Farfetch'd"));
        assert_eq!(loose_key("Mr. Mime"), "mrmime");
        assert_eq!(display_name("Nidoran_M"), "Nidoran♂");
        assert_eq!(species_alias("Giratina"), Some("Giratina (Altered)"));
    }
}
