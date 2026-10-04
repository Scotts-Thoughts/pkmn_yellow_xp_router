//! Trainer grouping: rematches and repeated fights shown as one list row
//! with variants (Solodex `data/index.ts` `autoGroupTrainers`), adapted to
//! the router's names.
//!
//! Solodex buckets major trainers by name: a bucket of up to three, or one
//! that holds rematches, is one group; a bigger bucket is clustered by the
//! ace's level (consecutive levels within 1 of each other share a group,
//! named `"<name> (Lv<n>)"`). Router names differ from Solodex's, so the
//! bucket key is derived differently:
//!
//! * a trailing ` Rematch` / ` Rematch N` is stripped (and `Trainer::rematch`
//!   counts), for every trainer, major or not, since the router's gens 3 and 4
//!   hold several rematch teams for ordinary trainers;
//! * major fights (those with a fight category) also lose the ROM-id suffix
//!   gen 4 / 5 names carry (`"Elite Four Shauntal (228)"`);
//! * rival fights lose the starter species the name carries and their trailing
//!   fight number (`"Rival1 Squirtle 1"`, `"Rival1 2 Chikorita"` and
//!   `"Rival Brendan 1 Treecko"` -> the rival), so the starter variants of one
//!   fight cluster by level;
//! * other major fights lose a trailing ` N` when a trainer without it exists
//!   (`"Aqua Leader Archie 2"`, FireRed's `"Elite Four Lorelei 2"`).

use std::collections::{HashMap, HashSet};

/// Consecutive ace levels this close share a cluster.
const LEVEL_GAP_THRESHOLD: i64 = 1;

/// What the grouping needs to know about one trainer.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub name: String,
    /// `Trainer::rematch`
    pub rematch: bool,
    /// has a fight category (gym leader, rival, ...)
    pub major: bool,
    /// the fight category is `rival`
    pub rival: bool,
    pub max_level: i64,
}

/// A group of trainers shown as one row: `members` index into the
/// candidates, sorted by ace level (the first is the primary).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub name: String,
    pub members: Vec<usize>,
}

/// ` Rematch` or ` Rematch N` at the end of `name`, stripped.
pub fn strip_rematch(name: &str) -> Option<&str> {
    let (head, tail) = name.rsplit_once(" Rematch").map_or((None, ""), |(h, t)| (Some(h), t));
    let head = head?;
    let ok = tail.is_empty() || (tail.starts_with(' ') && tail.len() > 1 && tail[1..].chars().all(|c| c.is_ascii_digit()));
    ok.then_some(head)
}

/// A trailing ROM id ` (123)` stripped.
pub fn strip_rom_suffix(name: &str) -> &str {
    if let Some(body) = name.strip_suffix(')') {
        if let Some((head, digits)) = body.rsplit_once(" (") {
            if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
                return head;
            }
        }
    }
    name
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// JS `name.replace(/ \d+(?: \w+)?$/, '')`: the leftmost ` <digits>` that is
/// followed by at most one more word and the end of the name.
pub fn strip_fight_number(name: &str) -> &str {
    for (i, _) in name.match_indices(' ') {
        let rest = &name[i + 1..];
        let digits = rest.bytes().take_while(|b| b.is_ascii_digit()).count();
        if digits == 0 {
            continue;
        }
        let after = &rest[digits..];
        let ok = after.is_empty() || (after.starts_with(' ') && after.len() > 1 && after[1..].chars().all(is_word));
        if ok {
            return &name[..i];
        }
    }
    name
}

/// `name` without the words that are Pokemon species (the starter a rival's
/// name carries); the name itself when nothing else would be left.
pub fn strip_species_words(name: &str, is_species: &dyn Fn(&str) -> bool) -> String {
    let kept: Vec<&str> = name.split(' ').filter(|w| !is_species(w)).collect();
    if kept.is_empty() || kept.len() == name.split(' ').count() {
        name.to_string()
    } else {
        kept.join(" ")
    }
}

/// A trailing ` N` stripped when what is left is a trainer of its own.
fn strip_variant_number<'a>(name: &'a str, exists: &dyn Fn(&str) -> bool) -> &'a str {
    if let Some((head, tail)) = name.rsplit_once(' ') {
        if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) && exists(head) {
            return head;
        }
    }
    name
}

/// The bucket a trainer belongs to, and whether it is a rematch.
fn bucket_key(c: &Candidate, exists: &dyn Fn(&str) -> bool, is_species: &dyn Fn(&str) -> bool) -> (String, bool) {
    let mut key: &str = &c.name;
    if c.major {
        key = strip_rom_suffix(key);
    }
    let mut rematch = c.rematch;
    if let Some(stripped) = strip_rematch(key) {
        key = stripped;
        rematch = true;
    }
    if c.major {
        if c.rival {
            let bare = strip_species_words(key, is_species);
            return (strip_fight_number(&bare).to_string(), rematch);
        }
        key = strip_variant_number(key, exists);
    }
    (key.to_string(), rematch)
}

/// Group `cands` (in the order the router lists them). `exists` tells
/// whether a trainer of that exact name is in the table, `is_species` whether
/// a word is a Pokemon species.
pub fn group_trainers(cands: &[Candidate], exists: &dyn Fn(&str) -> bool, is_species: &dyn Fn(&str) -> bool) -> Vec<Group> {
    let keys: Vec<(String, bool)> = cands.iter().map(|c| bucket_key(c, exists, is_species)).collect();
    let has_rematches: HashSet<&str> = keys.iter().filter(|(_, r)| *r).map(|(k, _)| k.as_str()).collect();
    // buckets in first-seen order; ordinary trainers are only grouped with
    // their own rematches
    let mut order: Vec<&str> = Vec::new();
    let mut buckets: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, c) in cands.iter().enumerate() {
        let key = keys[i].0.as_str();
        if !c.major && !has_rematches.contains(key) {
            continue;
        }
        if !buckets.contains_key(key) {
            order.push(key);
        }
        buckets.entry(key).or_default().push(i);
    }
    let mut groups = Vec::new();
    for key in order {
        let list = &buckets[key];
        if list.len() <= 1 {
            continue;
        }
        let mut sorted = list.clone();
        sorted.sort_by_key(|i| cands[*i].max_level);
        if list.len() <= 3 || has_rematches.contains(key) {
            groups.push(Group { name: key.to_string(), members: sorted });
            continue;
        }
        // cluster by the ace's level
        let mut cluster: Vec<usize> = vec![sorted[0]];
        let mut cluster_max = cands[sorted[0]].max_level;
        let flush = |cluster: &mut Vec<usize>, groups: &mut Vec<Group>| {
            if cluster.len() > 1 {
                let lvl = cands[cluster[0]].max_level;
                groups.push(Group { name: format!("{} (Lv{})", key, lvl), members: std::mem::take(cluster) });
            } else {
                // a lone fight keeps its own name and row
                cluster.clear();
            }
        };
        for &i in &sorted[1..] {
            let lvl = cands[i].max_level;
            if lvl - cluster_max <= LEVEL_GAP_THRESHOLD {
                cluster.push(i);
                cluster_max = lvl;
            } else {
                flush(&mut cluster, &mut groups);
                cluster.push(i);
                cluster_max = lvl;
            }
        }
        flush(&mut cluster, &mut groups);
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(name: &str, major: bool, rival: bool, level: i64) -> Candidate {
        Candidate { name: name.to_string(), rematch: name.contains("Rematch"), major, rival, max_level: level }
    }

    fn run(cands: &[Candidate]) -> Vec<Group> {
        let names: HashSet<String> = cands.iter().map(|c| c.name.clone()).collect();
        group_trainers(cands, &|n| names.contains(n), &|w| ["Squirtle", "Bulbasaur", "Charmander", "Treecko", "Mudkip", "Chikorita"].contains(&w))
    }

    #[test]
    fn strips_rematch_suffixes() {
        assert_eq!(strip_rematch("Camper Mickey Rematch 2"), Some("Camper Mickey"));
        assert_eq!(strip_rematch("Champion Squirtle Rematch"), Some("Champion Squirtle"));
        assert_eq!(strip_rematch("Camper Mickey"), None);
        assert_eq!(strip_rematch("Rematcher Bob"), None);
        assert_eq!(strip_rematch("Aroma Lady Rose Rematch 1x"), None);
    }

    #[test]
    fn strips_rom_ids() {
        assert_eq!(strip_rom_suffix("Elite Four Shauntal (228)"), "Elite Four Shauntal");
        assert_eq!(strip_rom_suffix("Leader Falkner"), "Leader Falkner");
        assert_eq!(strip_rom_suffix("Weird (name)"), "Weird (name)");
    }

    #[test]
    fn fight_number_regex_matches_solodex() {
        // / \d+(?: \w+)?$/
        assert_eq!(strip_fight_number("Rival Brendan 4"), "Rival Brendan");
        assert_eq!(strip_fight_number("Rival Brendan 4 Treecko"), "Rival Brendan");
        assert_eq!(strip_fight_number("Rival1 Squirtle 1"), "Rival1 Squirtle");
        assert_eq!(strip_fight_number("Rival2 2 Jolteon"), "Rival2");
        assert_eq!(strip_fight_number("Rival1 1 Chikorita"), "Rival1");
        assert_eq!(strip_fight_number("Rival Steven"), "Rival Steven");
        assert_eq!(strip_fight_number("Rival"), "Rival");
        // the leftmost match wins even when a later one would also fit
        assert_eq!(strip_fight_number("Rival 1 2"), "Rival");
        // three words after the number do not match
        assert_eq!(strip_fight_number("Rival 1 a b"), "Rival 1 a b");
        // the number must be preceded by a space
        assert_eq!(strip_fight_number("Rival2"), "Rival2");
    }

    #[test]
    fn small_buckets_become_one_group_sorted_by_level() {
        let cands = vec![c("Rival Brendan 1", true, true, 12), c("Rival Brendan 2", true, true, 30), c("Rival Brendan 3", true, true, 20), c("Youngster 1", false, false, 5)];
        let g = run(&cands);
        assert_eq!(g, vec![Group { name: "Rival Brendan".into(), members: vec![0, 2, 1] }]);
    }

    #[test]
    fn big_buckets_cluster_by_level_and_lone_fights_stay_ungrouped() {
        // five fights of one rival: levels 10, 11 (cluster), 13 (alone), 20, 21 (cluster)
        let cands = vec![
            c("Rival May 3", true, true, 20),
            c("Rival May 1", true, true, 10),
            c("Rival May 1b", true, true, 11),
            c("Rival May 2", true, true, 13),
            c("Rival May 4", true, true, 21),
        ];
        // "Rival May 1b" is not a trainer number form; give it the right name
        let mut cands = cands;
        cands[2].name = "Rival May 1 Mudkip".to_string();
        let g = run(&cands);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].name, "Rival May (Lv10)");
        assert_eq!(g[0].members, vec![1, 2]);
        assert_eq!(g[1].name, "Rival May (Lv20)");
        assert_eq!(g[1].members, vec![0, 4]);
        // the level-13 fight is in no group
        assert!(g.iter().all(|g| !g.members.contains(&3)));
    }

    #[test]
    fn rematches_group_with_their_original_even_for_ordinary_trainers() {
        let cands = vec![
            c("Camper Mickey", false, false, 10),
            c("Camper Mickey Rematch 2", false, false, 30),
            c("Camper Mickey Rematch 3", false, false, 40),
            c("Camper Mickey Rematch 4", false, false, 50),
            c("Camper Mickey Rematch 5", false, false, 60),
            c("Picnicker Lone", false, false, 12),
        ];
        let g = run(&cands);
        // a bucket holding rematches is one group however big, never clustered by level
        assert_eq!(g, vec![Group { name: "Camper Mickey".into(), members: vec![0, 1, 2, 3, 4] }]);
    }

    #[test]
    fn rom_id_variants_cluster_and_numbered_second_teams_group() {
        let cands = vec![
            c("Elite Four Shauntal (228)", true, false, 50),
            c("Elite Four Shauntal (563)", true, false, 51),
            c("Elite Four Shauntal (38)", true, false, 52),
            c("Elite Four Shauntal (143)", true, false, 70),
            c("Aqua Leader Archie", true, false, 40),
            c("Aqua Leader Archie 2", true, false, 50),
            c("Elite Four Lorelei", true, false, 50),
            c("Elite Four Lorelei 2", true, false, 60),
            c("Rival Chikorita 1", true, true, 5),
        ];
        let g = run(&cands);
        let names: Vec<&str> = g.iter().map(|g| g.name.as_str()).collect();
        assert!(names.contains(&"Elite Four Shauntal (Lv50)"), "{:?}", names);
        assert!(names.contains(&"Aqua Leader Archie"));
        assert!(names.contains(&"Elite Four Lorelei"));
        // the lone Lv70 Shauntal and the lone rival fight are not grouped
        assert!(g.iter().all(|g| !g.members.contains(&3) && !g.members.contains(&8)));
        let shauntal = g.iter().find(|g| g.name.starts_with("Elite Four Shauntal")).unwrap();
        assert_eq!(shauntal.members, vec![0, 1, 2]);
    }

    #[test]
    fn starters_in_rival_names_are_dropped_so_variants_cluster_together() {
        // gen 1 puts the starter before the fight number: three fights x three starters
        let mut cands = Vec::new();
        for (fight, level) in [(1, 5), (2, 9), (3, 18)] {
            for starter in ["Squirtle", "Bulbasaur", "Charmander"] {
                cands.push(c(&format!("Rival1 {} {}", starter, fight), true, true, level));
            }
        }
        let g = run(&cands);
        assert_eq!(g.len(), 3);
        assert_eq!(g[0].name, "Rival1 (Lv5)");
        assert_eq!(g[0].members, vec![0, 1, 2]);
        assert_eq!(g[1].name, "Rival1 (Lv9)");
        assert_eq!(g[2].name, "Rival1 (Lv18)");
        // gen 2 puts it after the number
        let cands: Vec<Candidate> = ["Chikorita", "Mudkip", "Treecko"].iter().enumerate().map(|(i, s)| c(&format!("Rival1 2 {}", s), true, true, 14 + (i as i64 % 2))).collect();
        assert_eq!(run(&cands)[0].name, "Rival1");
        // a name that is nothing but species words is left alone
        assert_eq!(strip_species_words("Squirtle", &|w| w == "Squirtle"), "Squirtle");
        assert_eq!(strip_species_words("Rival Brendan 1 Treecko", &|w| w == "Treecko"), "Rival Brendan 1");
    }

    #[test]
    fn numbered_trainers_are_not_merged_without_a_base() {
        let cands = vec![c("ExecutiveM 1", true, false, 30), c("ExecutiveM 2", true, false, 31), c("Brock 1", true, false, 12)];
        assert!(run(&cands).is_empty());
    }
}
