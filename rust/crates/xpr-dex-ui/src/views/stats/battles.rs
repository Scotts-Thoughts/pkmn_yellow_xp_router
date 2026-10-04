//! The major battles of a game, from the router's trainer tables (Solodex
//! `getMajorBattles` + `autoGroupTrainers`).
//!
//! Solodex took its major battles from its own trainer files; here a major
//! battle is a trainer the router gives a fight category (gym leader, Elite
//! Four, champion, rival, boss, team leader, post-game). Trainers that are the
//! same fight (a rival's three starter variants, rematches) are grouped as
//! Solodex does, and the group's "fastest Pokemon" is taken over every
//! trainer in it.

use std::collections::HashSet;

use indexmap::IndexMap;
use xpr_data::{GenData, Trainer};

/// The fastest enemy Pokemon of a battle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fastest {
    /// the router's species name
    pub species: String,
    pub level: i64,
    pub speed: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MajorBattle {
    /// the first trainer's router name (unique per battle)
    pub id: String,
    pub name: String,
    pub trainers: Vec<String>,
    pub max_level: i64,
    /// the class of the group's first trainer
    pub trainer_class: String,
    /// the router's fight category of the first trainer (`gym_leader`, ...)
    pub category: String,
    pub fastest: Option<Fastest>,
}

fn max_level(t: &Trainer) -> i64 {
    t.pkmn.iter().map(|p| p.level).max().unwrap_or(0)
}

/// A group of trainers shown as one battle.
struct Group<'a> {
    name: String,
    members: Vec<&'a Trainer>,
}

/// The router appends " (123)" to the names of trainers that share a name.
pub fn strip_id_suffix(name: &str) -> &str {
    if let Some(open) = name.rfind(" (") {
        let tail = &name[open + 2..];
        if let Some(num) = tail.strip_suffix(')') {
            if !num.is_empty() && num.bytes().all(|b| b.is_ascii_digit()) {
                return &name[..open];
            }
        }
    }
    name
}

/// " Rematch" / " Rematch 2" at the end of a name.
fn strip_rematch(name: &str) -> Option<&str> {
    let mut parts = name.rsplitn(3, ' ');
    let last = parts.next()?;
    let prev = parts.next();
    if last == "Rematch" && name.len() > last.len() {
        return Some(&name[..name.len() - last.len() - 1]);
    }
    if !last.is_empty() && last.bytes().all(|b| b.is_ascii_digit()) && prev == Some("Rematch") && name.len() > last.len() + "Rematch".len() + 2 {
        return Some(&name[..name.len() - last.len() - 1 - "Rematch".len() - 1]);
    }
    None
}

/// Solodex strips a rival's trailing " 4 Treecko" so the starter variants of
/// one fight share a bucket. The router names rivals "Rival Brendan 4 Treecko"
/// (gen 3), "Rival1 Squirtle 1" (gen 1), "Rival3 Charmander"; here: a trailing
/// number, a trailing species (the starter) and a trailing number again are
/// dropped, in that order.
pub fn strip_rival_suffix(name: &str, is_species: &dyn Fn(&str) -> bool) -> String {
    let mut tokens: Vec<&str> = name.split(' ').collect();
    let is_number = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
    if tokens.len() > 1 && tokens.last().map(|t| is_number(t)).unwrap_or(false) {
        tokens.pop();
    }
    if tokens.len() > 1 && tokens.last().map(|t| is_species(t)).unwrap_or(false) {
        tokens.pop();
    }
    if tokens.len() > 1 && tokens.last().map(|t| is_number(t)).unwrap_or(false) {
        tokens.pop();
    }
    tokens.join(" ")
}

fn is_rival_like(t: &Trainer, category: &str) -> bool {
    category == "rival" || t.name.contains("Rival") || t.trainer_class.starts_with("Rival")
}

/// Distinct trainers that Solodex lists as one entry.
fn manual_groups(game: &str) -> Vec<(&'static str, Vec<&'static str>)> {
    match game {
        "Black" => vec![("Chili / Cilan / Cress", vec!["Leader Chili", "Leader Cilan", "Leader Cress"])],
        _ => Vec::new(),
    }
}

/// `autoGroupTrainers`: major trainers bucketed by name; at most three of a
/// name (or any with rematches) form one group, more are clustered by level.
fn auto_groups<'a>(majors: &[(&'a Trainer, String)], skip: &HashSet<String>, is_species: &dyn Fn(&str) -> bool) -> Vec<Group<'a>> {
    let mut by_name: IndexMap<String, Vec<&'a Trainer>> = IndexMap::new();
    let mut has_rematches: HashSet<String> = HashSet::new();
    for (t, category) in majors {
        if skip.contains(&t.name) {
            continue;
        }
        let mut key = strip_id_suffix(&t.name).to_string();
        if let Some(stripped) = strip_rematch(&key) {
            let stripped = stripped.to_string();
            has_rematches.insert(stripped.clone());
            key = stripped;
        }
        if is_rival_like(t, category) {
            key = strip_rival_suffix(&key, is_species);
        }
        by_name.entry(key).or_default().push(t);
    }
    let mut groups = Vec::new();
    for (name, mut list) in by_name {
        if list.len() <= 1 {
            continue;
        }
        // lowest level first, so the original comes before rematches
        list.sort_by_key(|t| max_level(t));
        if list.len() <= 3 || has_rematches.contains(&name) {
            groups.push(Group { name, members: list });
            continue;
        }
        // cluster by max party level
        const LEVEL_GAP_THRESHOLD: i64 = 1;
        let emit = |groups: &mut Vec<Group<'a>>, cluster: Vec<&'a Trainer>| {
            let lvl = max_level(cluster[0]);
            let name = if cluster.len() > 1 { format!("{} (Lv{})", name, lvl) } else { name.clone() };
            groups.push(Group { name, members: cluster });
        };
        let mut cluster: Vec<&'a Trainer> = vec![list[0]];
        let mut cluster_max = max_level(list[0]);
        for t in list.into_iter().skip(1) {
            let lvl = max_level(t);
            if lvl - cluster_max <= LEVEL_GAP_THRESHOLD {
                cluster.push(t);
                cluster_max = lvl;
            } else {
                emit(&mut groups, std::mem::take(&mut cluster));
                cluster = vec![t];
                cluster_max = lvl;
            }
        }
        emit(&mut groups, cluster);
    }
    groups
}

/// The major battles of `gen` (the game is only used for Solodex's manual
/// groups), ordered by level.
pub fn major_battles(gen: &GenData, game: &str) -> Vec<MajorBattle> {
    let trainers: Vec<&Trainer> = gen.trainer_db().iter().map(|t| &**t).collect();
    let is_species = |name: &str| gen.pkmn_db().get_pkmn(name).is_some();
    let majors: Vec<(&Trainer, String)> = trainers.iter().filter_map(|t| gen.get_fight_category(&t.name).map(|c| (*t, c.to_string()))).collect();
    let category_of = |t: &Trainer| gen.get_fight_category(&t.name).map(str::to_string);

    let mut groups: Vec<Group> = Vec::new();
    let mut skip: HashSet<String> = HashSet::new();
    for (name, members) in manual_groups(game) {
        let found: Vec<&Trainer> = members.iter().filter_map(|m| trainers.iter().copied().find(|t| t.name == *m)).collect();
        if found.len() == members.len() {
            skip.extend(members.iter().map(|m| m.to_string()));
            groups.push(Group { name: name.to_string(), members: found });
        }
    }
    groups.extend(auto_groups(&majors, &skip, &is_species));

    // trainer name -> group index
    let mut group_of: IndexMap<&str, usize> = IndexMap::new();
    for (i, g) in groups.iter().enumerate() {
        for m in &g.members {
            group_of.insert(m.name.as_str(), i);
        }
    }

    let mut battles: Vec<MajorBattle> = Vec::new();
    let mut seen: HashSet<usize> = HashSet::new();
    for t in &trainers {
        if let Some(&gi) = group_of.get(t.name.as_str()) {
            if !seen.insert(gi) {
                continue;
            }
            let g = &groups[gi];
            let first = g.members[0];
            let Some(category) = category_of(first) else { continue };
            battles.push(build(&g.name, &g.members, first, category));
        } else if let Some(category) = category_of(t) {
            battles.push(build(strip_id_suffix(&t.name), &[*t], t, category));
        }
    }
    // stable, so equal levels keep the trainer table's order
    battles.sort_by_key(|b| b.max_level);
    battles
}

fn build(name: &str, members: &[&Trainer], first: &Trainer, category: String) -> MajorBattle {
    let mut fastest: Option<Fastest> = None;
    for t in members {
        for mon in &t.pkmn {
            // strictly faster only: on a tie the first Pokemon found stays
            if fastest.as_ref().map(|f| mon.cur_stats.speed > f.speed).unwrap_or(true) {
                fastest = Some(Fastest { species: mon.name.clone(), level: mon.level, speed: mon.cur_stats.speed });
            }
        }
    }
    MajorBattle {
        id: first.name.clone(),
        name: name.to_string(),
        trainers: members.iter().map(|t| t.name.clone()).collect(),
        max_level: members.iter().map(|t| max_level(t)).max().unwrap_or(0),
        trainer_class: first.trainer_class.clone(),
        category,
        fastest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn species(n: &str) -> bool {
        matches!(n, "Treecko" | "Torchic" | "Mudkip" | "Squirtle" | "Bulbasaur" | "Charmander")
    }

    #[test]
    fn id_suffix() {
        assert_eq!(strip_id_suffix("Pokemon Trainer Cheren (53)"), "Pokemon Trainer Cheren");
        assert_eq!(strip_id_suffix("Brock 1"), "Brock 1");
        assert_eq!(strip_id_suffix("Leader (Lv5)"), "Leader (Lv5)");
        assert_eq!(strip_id_suffix("Team Plasma N (586)"), "Team Plasma N");
        assert_eq!(strip_id_suffix("X ()"), "X ()");
    }

    #[test]
    fn rematch_suffix() {
        assert_eq!(strip_rematch("Leader Roxanne Rematch"), Some("Leader Roxanne"));
        assert_eq!(strip_rematch("Leader Roxanne Rematch 2"), Some("Leader Roxanne"));
        assert_eq!(strip_rematch("Leader Roxanne 2"), None);
        assert_eq!(strip_rematch("Rematch"), None);
    }

    #[test]
    fn rival_suffix() {
        assert_eq!(strip_rival_suffix("Rival Brendan 4 Treecko", &species), "Rival Brendan");
        assert_eq!(strip_rival_suffix("Rival May 1 Mudkip", &species), "Rival May");
        assert_eq!(strip_rival_suffix("Rival1 Squirtle 1", &species), "Rival1");
        assert_eq!(strip_rival_suffix("Rival3 Charmander", &species), "Rival3");
        assert_eq!(strip_rival_suffix("Rival Steven", &species), "Rival Steven");
        assert_eq!(strip_rival_suffix("Rival", &species), "Rival");
        assert_eq!(strip_rival_suffix("Rival Brendan 5", &species), "Rival Brendan");
    }
}
