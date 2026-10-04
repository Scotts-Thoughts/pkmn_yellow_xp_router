//! Form classification for species display names (Solodex `data/forms.ts`).
//!
//! The Pokédex files name alternate forms three ways:
//! - prefix: "Mega Venusaur", "Mega Charizard X", "Primal Kyogre",
//!   "Alolan Raichu", "Galarian Darmanitan", "Paldean Tauros"
//! - suffix: "Giratina (Origin)", "Pumpkaboo (Small)", "Venusaur (Gmax)",
//!   "Absol (Mega Z)"
//! - both:   "Galarian Darmanitan (Zen)", "Paldean Tauros (Combat Breed)"
//!
//! Everything that needs to know what kind of form a name is goes through
//! [`classify_form`].

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    Alolan,
    Galarian,
    Hisuian,
    Paldean,
}

impl Region {
    pub fn prefix(self) -> &'static str {
        match self {
            Region::Alolan => "Alolan",
            Region::Galarian => "Galarian",
            Region::Hisuian => "Hisuian",
            Region::Paldean => "Paldean",
        }
    }

    fn gen(self) -> u8 {
        match self {
            Region::Alolan => 7,
            Region::Galarian | Region::Hisuian => 8,
            Region::Paldean => 9,
        }
    }

    fn parse(s: &str) -> Option<Region> {
        Some(match s {
            "Alolan" => Region::Alolan,
            "Galarian" => Region::Galarian,
            "Hisuian" => Region::Hisuian,
            "Paldean" => Region::Paldean,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormInfo {
    pub name: String,
    /// species name with every form decoration removed ("Darmanitan")
    pub base: String,
    /// Mega Evolution, Primal Reversion or a Legends Z-A "(Mega Z)" form
    pub is_mega: bool,
    pub is_regional: bool,
    pub region: Option<Region>,
    pub is_gmax: bool,
    /// any other parenthesised variant: Origin, Zen, Small, Female, Core ...
    pub is_variant: bool,
    /// text inside the parentheses, if any
    pub variant: Option<String>,
    /// the undecorated species entry
    pub is_base: bool,
    /// earliest generation in which this kind of form can exist
    pub introduced_gen: u8,
}

/// `^(.+?) \((.+)\)$`
fn split_suffix(name: &str) -> Option<(&str, &str)> {
    let inner = name.strip_suffix(')')?;
    // lazy `.+?` then ` (`: the first " (" with at least one char before it
    let idx = inner.char_indices().skip(1).find(|(i, _)| inner[*i..].starts_with(" ("))?.0;
    let variant = &inner[idx + 2..];
    if variant.is_empty() {
        return None;
    }
    Some((&inner[..idx], variant))
}

/// `^(Alolan|Galarian|Hisuian|Paldean) (.+)$`
fn split_region(name: &str) -> Option<(Region, &str)> {
    let (head, rest) = name.split_once(' ')?;
    let region = Region::parse(head)?;
    (!rest.is_empty()).then_some((region, rest))
}

/// `^(Mega|Primal) (.+?)( X| Y| Z)?$` -> the species part
fn split_mega(name: &str) -> Option<&str> {
    let rest = name.strip_prefix("Mega ").or_else(|| name.strip_prefix("Primal "))?;
    if rest.is_empty() {
        return None;
    }
    for suf in [" X", " Y", " Z"] {
        if let Some(s) = rest.strip_suffix(suf) {
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    Some(rest)
}

pub fn classify_form(name: &str) -> FormInfo {
    let mut rest = name;
    let mut is_mega = false;
    let mut variant: Option<String> = None;
    let mut is_gmax = false;
    if let Some((r, v)) = split_suffix(rest) {
        rest = r;
        if v == "Gmax" || v.ends_with(" Gmax") {
            is_gmax = true;
        }
        if v == "Mega Z" {
            is_mega = true;
        }
        variant = Some(v.to_string());
    }
    let mut region = None;
    if let Some((reg, r)) = split_region(rest) {
        region = Some(reg);
        rest = r;
    }
    if let Some(r) = split_mega(rest) {
        is_mega = true;
        rest = r;
    }
    let is_variant = variant.as_deref().map(|v| !is_gmax && v != "Mega Z").unwrap_or(false);
    let is_regional = region.is_some();
    let mut introduced_gen = 1;
    if is_mega {
        introduced_gen = 6;
    }
    if is_gmax {
        introduced_gen = introduced_gen.max(8);
    }
    if let Some(r) = region {
        introduced_gen = introduced_gen.max(r.gen());
    }
    FormInfo {
        name: name.to_string(),
        base: rest.to_string(),
        is_mega,
        is_regional,
        region,
        is_gmax,
        is_variant,
        variant,
        is_base: !is_mega && !is_regional && !is_gmax && !is_variant,
        introduced_gen,
    }
}

/// Mega / Primal / Mega Z.
pub fn is_mega_form(name: &str) -> bool {
    classify_form(name).is_mega
}

/// "Meloetta (Aria)" -> ("Meloetta", Some("(Aria)")): the stat and
/// effectiveness cards draw the form on its own line.
pub fn split_form_name(name: &str) -> (&str, Option<&str>) {
    match split_suffix(name) {
        Some((base, _)) => {
            let form = name[base.len()..].trim_start();
            (base.trim_end(), Some(form))
        }
        None => (name, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_forms() {
        let f = classify_form("Galarian Darmanitan (Zen)");
        assert_eq!(f.base, "Darmanitan");
        assert_eq!(f.region, Some(Region::Galarian));
        assert!(f.is_variant && f.is_regional && !f.is_mega);
        assert_eq!(f.introduced_gen, 8);
        let m = classify_form("Mega Charizard X");
        assert!(m.is_mega);
        assert_eq!(m.base, "Charizard");
        assert!(classify_form("Absol (Mega Z)").is_mega);
        assert!(classify_form("Venusaur (Gmax)").is_gmax);
        assert!(classify_form("Primal Kyogre").is_mega);
        assert!(classify_form("Pikachu").is_base);
        assert!(!classify_form("Mr. Mime").is_mega);
        assert!(!classify_form("Megaplex").is_mega);
        assert_eq!(split_form_name("Meloetta (Aria)"), ("Meloetta", Some("(Aria)")));
        assert_eq!(split_form_name("Pikachu"), ("Pikachu", None));
        assert_eq!(classify_form("Paldean Tauros (Combat Breed)").variant.as_deref(), Some("Combat Breed"));
    }
}
