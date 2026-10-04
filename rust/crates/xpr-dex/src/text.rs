//! String helpers shared by the data layer and the views.

use std::cmp::Ordering;

/// An approximation of JavaScript's `a.localeCompare(b)` (ICU root
/// collation) good enough for species, move and trainer names: letters
/// compare case-insensitively, spaces and punctuation sort before digits and
/// digits before letters; exact ties fall back to lowercase-first, then bytes.
pub fn locale_cmp(a: &str, b: &str) -> Ordering {
    let ka = a.chars().map(collation_key);
    let kb = b.chars().map(collation_key);
    match ka.cmp(kb) {
        Ordering::Equal => {}
        o => return o,
    }
    // tertiary: lowercase before uppercase (ICU default)
    let ta = a.chars().map(|c| c.is_uppercase());
    let tb = b.chars().map(|c| c.is_uppercase());
    match ta.cmp(tb) {
        Ordering::Equal => a.cmp(b),
        o => o,
    }
}

/// (class, folded char): 0 = whitespace, 1 = punctuation / symbols,
/// 2 = digits, 3 = letters.
fn collation_key(c: char) -> (u8, char) {
    let folded = fold(c);
    let class = if c.is_whitespace() {
        0
    } else if c.is_ascii_digit() {
        2
    } else if c.is_alphabetic() {
        3
    } else {
        1
    };
    (class, folded)
}

fn fold(c: char) -> char {
    match c {
        'é' | 'É' | 'è' | 'ê' => 'e',
        '\u{2019}' => '\'',
        c => c.to_lowercase().next().unwrap_or(c),
    }
}

/// Case-insensitive substring test (`haystack.toLowerCase().includes(needle.toLowerCase())`).
pub fn contains_ci(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_like_locale_compare() {
        let mut v = vec!["Thunder Wave", "Thunderbolt", "thunder", "Thunder", "Double-Edge", "Double Team", "Acid", "10,000,000 Volt Thunderbolt"];
        v.sort_by(|a, b| locale_cmp(a, b));
        assert_eq!(v, vec!["10,000,000 Volt Thunderbolt", "Acid", "Double Team", "Double-Edge", "thunder", "Thunder", "Thunder Wave", "Thunderbolt"]);
    }
}
