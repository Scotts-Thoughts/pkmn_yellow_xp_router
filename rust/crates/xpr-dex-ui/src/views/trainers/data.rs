//! The Trainers tab's view of one router version: every trainer with its
//! party, fight category, list grouping, and the speed ranking. Built once
//! per version and shared (with the trainer search, which reads every
//! version) through a small cache.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use egui::Color32;
use xpr_data::{GenData, Registry, Trainer};

use super::grouping::{group_trainers, Candidate, Group};
use crate::palette;
use crate::state::dex_game_for_version;

/// One party member with its Dex identity resolved.
#[derive(Clone, Debug)]
pub struct PartyMon {
    /// the router's species name
    pub name: String,
    /// shown name ("Nidoran♀", "Mr. Mime")
    pub display: String,
    /// the Dex's key for the species (sprites, evolution data)
    pub dex_key: Option<&'static str>,
    pub dex: i32,
    pub level: i64,
}

pub struct TrainerInfo {
    pub trainer: Arc<Trainer>,
    /// the router's fight category (`gym_leader`, `rival`, ...)
    pub category: Option<String>,
    /// a major fight (has a category)
    pub major: bool,
    pub max_level: i64,
    pub party: Vec<PartyMon>,
}

impl TrainerInfo {
    /// The trainer's name colour in lists (Solodex `trainerNameColor`).
    pub fn name_color(&self) -> Color32 {
        if self.major {
            // a major fight the table has no category for is gold, like gym leaders
            palette::trainer_category_color(Some(self.category.as_deref().unwrap_or("major")))
        } else {
            palette::GRAY_200
        }
    }

    /// The class pill colour of the detail header (Solodex `CLASS_COLORS`).
    pub fn pill_color(&self) -> Color32 {
        match self.category.as_deref() {
            Some("gym_leader") => palette::hex("#FFD700"),
            Some(c) => palette::trainer_category_color(Some(c)),
            None if self.major => palette::YELLOW_400,
            None => palette::SLATE_400,
        }
    }

    /// `trainer_class (id)` the way the list shows the trainer.
    pub fn id_suffix(&self) -> Option<String> {
        let t = &self.trainer;
        (t.trainer_id >= 0 && !t.name.ends_with(')')).then(|| format!("({})", t.trainer_id))
    }
}

/// One row of the trainer list: a trainer, or a group shown by its primary.
#[derive(Clone, Debug)]
pub struct ListRow {
    /// the trainer whose party and level the row shows (a group's first)
    pub primary: usize,
    /// the shown name (the group's name for a group)
    pub name: String,
    pub group: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct SpeedEntry {
    pub trainer: usize,
    pub species: String,
    pub speed: i64,
}

pub struct VersionIndex {
    pub version: String,
    pub gen: Arc<GenData>,
    pub generation: u8,
    /// the Dex game of the version (sprites, evolution lines, type order)
    pub dex_game: Option<&'static str>,
    pub trainers: Vec<TrainerInfo>,
    by_name: HashMap<String, usize>,
    pub groups: Vec<Group>,
    /// per trainer: its group
    group_of: Vec<Option<usize>>,
    /// the list, major fights first, each class by ace level
    pub rows: Vec<ListRow>,
    pub classes: Vec<String>,
    pub locations: Vec<String>,
    /// every party member by speed, fastest first
    pub speed: Vec<SpeedEntry>,
}

impl VersionIndex {
    pub fn build(version: &str, gen: Arc<GenData>, dex_game: Option<&'static str>) -> VersionIndex {
        let db = gen.trainer_db();
        let mut trainers: Vec<TrainerInfo> = Vec::with_capacity(db.len());
        let mut species_cache: HashMap<String, (String, Option<&'static str>, i32)> = HashMap::new();
        for t in db.iter() {
            let category = gen.get_fight_category(&t.name).map(|s| s.to_string());
            let major = category.is_some() || gen.is_major_fight(&t.name);
            let party: Vec<PartyMon> = t
                .pkmn
                .iter()
                .map(|p| {
                    let (display, key, dex) = species_cache.entry(p.name.clone()).or_insert_with(|| resolve(&p.name)).clone();
                    PartyMon { name: p.name.clone(), display, dex_key: key, dex, level: p.level }
                })
                .collect();
            let max_level = party.iter().map(|p| p.level).max().unwrap_or(0);
            trainers.push(TrainerInfo { trainer: t.clone(), category, major, max_level, party });
        }
        let by_name: HashMap<String, usize> = trainers.iter().enumerate().map(|(i, t)| (t.trainer.name.clone(), i)).collect();

        let cands: Vec<Candidate> = trainers
            .iter()
            .map(|t| Candidate { name: t.trainer.name.clone(), rematch: t.trainer.rematch, major: t.major, rival: t.category.as_deref() == Some("rival"), max_level: t.max_level })
            .collect();
        let groups = group_trainers(&cands, &|n| db.get_trainer(n).is_some(), &|w| gen.pkmn_db().get_pkmn(w).is_some());
        let mut group_of: Vec<Option<usize>> = vec![None; trainers.len()];
        for (gi, g) in groups.iter().enumerate() {
            for m in &g.members {
                group_of[*m] = Some(gi);
            }
        }

        // the list: one row per trainer or group, at the position of its first member
        let mut rows: Vec<ListRow> = Vec::new();
        let mut emitted = vec![false; groups.len()];
        for i in 0..trainers.len() {
            match group_of[i] {
                Some(g) => {
                    if !emitted[g] {
                        emitted[g] = true;
                        rows.push(ListRow { primary: groups[g].members[0], name: groups[g].name.clone(), group: Some(g) });
                    }
                }
                None => rows.push(ListRow { primary: i, name: trainers[i].trainer.name.clone(), group: None }),
            }
        }
        // major battles first, then the rest; both by ace level (stable)
        rows.sort_by_key(|r| (!trainers[r.primary].major, trainers[r.primary].max_level));

        let mut classes: Vec<String> = trainers.iter().map(|t| t.trainer.trainer_class.clone()).collect();
        classes.sort();
        classes.dedup();
        let mut locations: Vec<String> = trainers.iter().filter(|t| !t.trainer.location.is_empty()).map(|t| t.trainer.location.clone()).collect();
        locations.sort();
        locations.dedup();

        let mut speed: Vec<SpeedEntry> = Vec::new();
        for (i, t) in trainers.iter().enumerate() {
            for (m, p) in t.party.iter().zip(t.trainer.pkmn.iter()) {
                speed.push(SpeedEntry { trainer: i, species: m.display.clone(), speed: p.cur_stats.speed });
            }
        }
        speed.sort_by(|a, b| b.speed.cmp(&a.speed));

        VersionIndex { version: version.to_string(), generation: gen.get_generation(), gen, dex_game, trainers, by_name, groups, group_of, rows, classes, locations, speed }
    }

    /// The index of a trainer by the router's name.
    pub fn find(&self, name: &str) -> Option<usize> {
        if let Some(i) = self.by_name.get(name) {
            return Some(*i);
        }
        let t = self.gen.trainer_db().get_trainer(name)?;
        self.by_name.get(&t.name).copied()
    }

    pub fn group_of(&self, trainer: usize) -> Option<&Group> {
        self.group_of.get(trainer).copied().flatten().map(|g| &self.groups[g])
    }

    /// Whether `row` is the selected trainer's (the trainer itself or its group).
    pub fn row_is_selected(&self, row: &ListRow, selected: Option<usize>) -> bool {
        let Some(sel) = selected else { return false };
        row.primary == sel || row.group.map(|g| self.groups[g].members.contains(&sel)).unwrap_or(false)
    }
}

/// (shown name, Dex key, national dex number) of a router species name.
fn resolve(name: &str) -> (String, Option<&'static str>, i32) {
    match xpr_dex::resolve_species(name) {
        Some(key) => {
            let dex = xpr_dex::species_entry(key).map(|e| e.national_dex_number).unwrap_or(0);
            (xpr_dex::display_name(key).to_string(), Some(key), dex)
        }
        None => (name.replace('_', " "), None, 0),
    }
}

// ---- cache ------------------------------------------------------------------------------------

static CACHE: Mutex<Option<HashMap<String, Arc<VersionIndex>>>> = Mutex::new(None);

/// The index of a router version, built on first use and rebuilt when the
/// registry hands out new data for it (a reloaded custom gen).
pub fn index_for(registry: &Arc<Registry>, version: &str) -> Option<Arc<VersionIndex>> {
    let gen = registry.get_version(version).ok()?;
    {
        let guard = CACHE.lock().ok()?;
        if let Some(ix) = guard.as_ref().and_then(|m| m.get(version)) {
            if Arc::ptr_eq(&ix.gen, &gen) {
                return Some(ix.clone());
            }
        }
    }
    let ix = Arc::new(VersionIndex::build(version, gen, dex_game_for_version(registry, version)));
    let mut guard = CACHE.lock().ok()?;
    guard.get_or_insert_with(HashMap::new).insert(version.to_string(), ix.clone());
    Some(ix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::test_registry;

    fn index(version: &str) -> Arc<VersionIndex> {
        index_for(&test_registry(), version).unwrap()
    }

    #[test]
    fn every_trainer_is_in_exactly_one_row() {
        for version in ["Red", "Yellow", "Crystal", "Emerald", "FireRed", "Platinum", "HeartGold", "Black", "Black 2"] {
            let ix = index(version);
            let mut seen = vec![0; ix.trainers.len()];
            for row in &ix.rows {
                match row.group {
                    Some(g) => {
                        assert_eq!(row.primary, ix.groups[g].members[0], "{}: a group row shows its first member", version);
                        for m in &ix.groups[g].members {
                            seen[*m] += 1;
                        }
                    }
                    None => seen[row.primary] += 1,
                }
            }
            assert!(seen.iter().all(|n| *n == 1), "{}: {:?}", version, seen.iter().enumerate().filter(|(_, n)| **n != 1).collect::<Vec<_>>());
            // major fights first, each part by ace level
            let majors: Vec<bool> = ix.rows.iter().map(|r| ix.trainers[r.primary].major).collect();
            let switch = majors.iter().position(|m| !m).unwrap_or(majors.len());
            assert!(majors[..switch].iter().all(|m| *m) && majors[switch..].iter().all(|m| !*m), "{}", version);
            for part in [&ix.rows[..switch], &ix.rows[switch..]] {
                assert!(part.windows(2).all(|w| ix.trainers[w[0].primary].max_level <= ix.trainers[w[1].primary].max_level), "{}", version);
            }
        }
    }

    #[test]
    fn rematches_and_repeated_fights_share_a_row() {
        let ix = index("Emerald");
        let i = ix.find("Leader Tate & Liza").unwrap();
        let g = ix.group_of(i).expect("Tate & Liza rematches are grouped");
        assert_eq!(g.name, "Leader Tate & Liza");
        assert_eq!(g.members.len(), 5, "the first fight and four rematches");
        assert_eq!(g.members[0], i, "the original has the lowest ace");
        let row = ix.rows.iter().find(|r| r.group.is_some() && r.name == g.name).unwrap();
        assert!(ix.row_is_selected(row, Some(g.members[3])));
        // ordinary trainers are grouped with their own rematches too
        let rose = ix.find("Aroma Lady Rose").unwrap();
        assert_eq!(ix.group_of(rose).unwrap().members.len(), 5);
        // gen 5 fights that carry ROM ids
        let ix = index("Black 2");
        let roxie = ix.find("Leader Roxie (157)").unwrap();
        let other = ix.find("Leader Roxie (765)").unwrap();
        assert_eq!(ix.group_of(roxie).map(|g| &g.members), ix.group_of(other).map(|g| &g.members));
        assert_eq!(ix.group_of(roxie).unwrap().name, "Leader Roxie");
    }

    #[test]
    fn rivals_cluster_by_ace_level() {
        // gen 1 names carry the starter before the fight number; the three starters of a fight share a row
        let red = index("Red");
        let g = red.group_of(red.find("Rival1 Squirtle 1").unwrap()).expect("starter variants are grouped");
        assert_eq!(g.members.len(), 3);
        assert!(g.name.starts_with("Rival1 (Lv"), "{}", g.name);
        let ix = index("Emerald");
        let groups: Vec<&str> = ix.groups.iter().map(|g| g.name.as_str()).filter(|n| n.starts_with("Rival Brendan")).collect();
        assert!(!groups.is_empty());
        assert!(groups.iter().all(|n| n.contains("(Lv")), "{:?}", groups);
        // the three starter variants of the first fight share a group
        let t = ix.find("Rival Brendan 1 Treecko").unwrap();
        let g = ix.group_of(t).unwrap();
        assert_eq!(g.members.len(), 3);
        assert!(g.name.starts_with("Rival Brendan (Lv"));
    }

    #[test]
    fn names_are_coloured_by_fight_category() {
        let ix = index("Crystal");
        let color = |n: &str| ix.trainers[ix.find(n).unwrap()].name_color();
        assert_eq!(color("Leader Falkner"), palette::YELLOW_400);
        assert_eq!(color("Elite Four Will"), palette::PURPLE_400);
        assert_eq!(color("Champion Lance"), palette::TEAL_400);
        assert_eq!(color("Rival1 1 Totodile"), palette::BLUE_400);
        assert_eq!(color("Youngster Joey"), palette::GRAY_200);
        let pill = |n: &str| ix.trainers[ix.find(n).unwrap()].pill_color();
        assert_eq!(pill("Leader Falkner"), palette::hex("#FFD700"));
        assert_eq!(pill("Youngster Joey"), palette::SLATE_400);
    }

    #[test]
    fn the_speed_ranking_is_sorted_and_complete() {
        let ix = index("Crystal");
        let total: usize = ix.trainers.iter().map(|t| t.party.len()).sum();
        assert_eq!(ix.speed.len(), total);
        assert!(ix.speed.windows(2).all(|w| w[0].speed >= w[1].speed));
        assert!(ix.classes.windows(2).all(|w| w[0] < w[1]) && ix.locations.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn species_resolve_to_dex_names_and_sprites() {
        let ix = index("Red");
        let t = &ix.trainers[ix.find("Brock 1").unwrap()];
        assert_eq!(t.party[0].display, "Geodude");
        assert_eq!(t.party[0].dex, 74);
        assert!(t.party[0].dex_key.is_some());
        // internal names read properly
        assert_eq!(resolve("Mr_Mime").0, "Mr. Mime");
        assert_eq!(resolve("Nidoran_F").0, "Nidoran\u{2640}");
        assert_eq!(resolve("NoSuchMon_X"), ("NoSuchMon X".to_string(), None, 0));
    }

    #[test]
    fn the_index_is_shared_until_the_data_changes() {
        let reg = test_registry();
        let a = index_for(&reg, "Crystal").unwrap();
        let b = index_for(&reg, "Crystal").unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert!(index_for(&reg, "No Such Version").is_none());
    }
}
