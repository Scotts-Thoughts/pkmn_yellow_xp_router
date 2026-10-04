//! Row data of the movepool tables and their sorting (Solodex `Movepool.tsx`
//! row builders, `hooks/useMoveSort.ts`). No egui painting here, so the
//! comparison views can build and sort rows the same way.

use std::cmp::Ordering;
use std::sync::Arc;

use egui::Color32;
use indexmap::IndexMap;
use xpr_dex::{compare_tmhm, get_move_data, tmhm_code, PokemonData};

use crate::widgets::SortDir;

/// A small game badge after a move name (multi-game rows: "RS", "E", ...).
#[derive(Clone, Debug, PartialEq)]
pub struct GameTag {
    pub abbrev: String,
    pub color: Color32,
}

/// One row of a movepool table (Solodex `RowData`).
#[derive(Clone, Debug, PartialEq)]
pub struct RowData {
    pub move_name: String,
    /// Sort key of level-up rows, twice Solodex's value so it stays an
    /// integer (`Rem` 0.5 -> 1, level `n` -> `2n`, `Evo` 1.5 -> 3). Other
    /// tables use 0.
    pub sort_key: i32,
    /// First column: "12", "Evo", "Rem", "TM24", "Tutor", or empty.
    pub prefix: String,
    pub game_tags: Vec<GameTag>,
}

impl RowData {
    pub fn simple(move_name: &str, prefix: &str) -> RowData {
        RowData {
            move_name: move_name.to_string(),
            sort_key: 0,
            prefix: prefix.to_string(),
            game_tags: Vec::new(),
        }
    }

    /// A TM / HM row (the prefix is a "TM24" / "HM03" code); those get the
    /// TM popover.
    pub fn is_tm(&self) -> bool {
        self.prefix.starts_with("TM") || self.prefix.starts_with("HM")
    }

    pub fn is_tutor(&self) -> bool {
        self.prefix == "Tutor"
    }
}

/// First-column label of a level-up entry. Level-up entries use two sentinel
/// levels: 0 = learned on evolution ("Evo"), -1 = only available from the
/// Move Reminder ("Rem", Legends: Z-A tables).
pub fn level_prefix(level: i32) -> String {
    if level == 0 {
        "Evo".to_string()
    } else if level < 0 {
        "Rem".to_string()
    } else {
        level.to_string()
    }
}

/// Sort key of a level-up entry, doubled so it is an integer: Solodex sorts
/// Move Reminder entries (0.5) before level 1 and Evo entries (1.5) between
/// level 1 and level 2; here that is 1, 3 and `2 * level`.
pub fn level_sort_key(level: i32) -> i32 {
    if level == 0 {
        3
    } else if level < 0 {
        1
    } else {
        level * 2
    }
}

/// A Pokémon only starts with the last four moves at its level, so when
/// five or more level-1 moves are listed the earlier ones are only
/// obtainable from the Move Reminder: relabel them "Rem" and sort them to
/// the front.
pub fn apply_remind_labels(mut rows: Vec<RowData>) -> Vec<RowData> {
    let level1: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.prefix == "1")
        .map(|(i, _)| i)
        .collect();
    if level1.len() >= 5 {
        let remind = level1.len() - 4;
        for &i in &level1[..remind] {
            rows[i].prefix = "Rem".to_string();
            rows[i].sort_key = level_sort_key(-1);
        }
        rows.sort_by_key(|r| r.sort_key);
    }
    rows
}

/// Level-up rows of one game: sorted by level only (a stable sort, so moves
/// at the same level keep the data's order), Move Reminder labels applied.
pub fn single_level_rows(pokemon: &PokemonData) -> Vec<RowData> {
    let mut rows: Vec<RowData> = pokemon
        .level_up_learnset
        .iter()
        .map(|(level, name)| RowData {
            move_name: name.clone(),
            sort_key: level_sort_key(*level),
            prefix: level_prefix(*level),
            game_tags: Vec::new(),
        })
        .collect();
    rows.sort_by_key(|r| r.sort_key);
    apply_remind_labels(rows)
}

/// Rows of a plain move list (egg, transfer, prior evolution, ...): no
/// prefix, in the data's order.
pub fn single_simple_rows(moves: &[String]) -> Vec<RowData> {
    moves.iter().map(|m| RowData::simple(m, "")).collect()
}

/// TM / HM rows: each move's code in `game` (empty when the move has none
/// there), sorted TM before TR before HM, then by number.
pub fn tm_hm_rows(moves: &[String], game: &str) -> Vec<RowData> {
    let mut rows: Vec<RowData> = moves
        .iter()
        .map(|m| RowData::simple(m, &tmhm_code(m, game).unwrap_or_default()))
        .collect();
    rows.sort_by(|a, b| compare_tmhm(&a.prefix, &b.prefix));
    rows
}

/// Move tutor rows (every prefix is "Tutor").
pub fn tutor_rows(moves: &[String]) -> Vec<RowData> {
    moves.iter().map(|m| RowData::simple(m, "Tutor")).collect()
}

// ---------------------------------------------------------------------------
// multi-game rows (a generation's games merged into one table)
// ---------------------------------------------------------------------------

/// One game of a generation group (Solodex `GenGameData`).
#[derive(Clone, Debug)]
pub struct GenGameData {
    pub game: String,
    pub abbrev: String,
    pub color: Color32,
    pub pokemon: Arc<PokemonData>,
}

fn tags_for(gen_data: &[GenGameData], games: &[&str]) -> Vec<GameTag> {
    if games.len() == gen_data.len() {
        return Vec::new();
    }
    gen_data
        .iter()
        .filter(|gd| games.contains(&gd.game.as_str()))
        .map(|gd| GameTag {
            abbrev: gd.abbrev.clone(),
            color: gd.color,
        })
        .collect()
}

/// Level-up rows of several games merged: keyed by (level, move) so the same
/// move at two levels stays two rows; a row carries game tags unless every
/// game has it.
pub fn build_level_up_rows(gen_data: &[GenGameData]) -> Vec<RowData> {
    let mut map: IndexMap<(i32, String), Vec<&str>> = IndexMap::new();
    for gd in gen_data {
        for (level, name) in &gd.pokemon.level_up_learnset {
            let games = map.entry((*level, name.clone())).or_default();
            if !games.contains(&gd.game.as_str()) {
                games.push(gd.game.as_str());
            }
        }
    }
    let mut rows: Vec<RowData> = map
        .into_iter()
        .map(|((level, name), games)| RowData {
            move_name: name,
            sort_key: level_sort_key(level),
            prefix: level_prefix(level),
            game_tags: tags_for(gen_data, &games),
        })
        .collect();
    rows.sort_by_key(|r| r.sort_key);
    apply_remind_labels(rows)
}

/// Rows of a plain move list across several games, tagged like
/// [`build_level_up_rows`].
pub fn build_simple_rows(
    gen_data: &[GenGameData],
    get_list: impl Fn(&PokemonData) -> &[String],
) -> Vec<RowData> {
    let mut map: IndexMap<String, Vec<&str>> = IndexMap::new();
    for gd in gen_data {
        for name in get_list(&gd.pokemon) {
            map.entry(name.clone()).or_default().push(gd.game.as_str());
        }
    }
    map.into_iter()
        .map(|(name, games)| RowData {
            move_name: name,
            sort_key: 0,
            prefix: String::new(),
            game_tags: tags_for(gen_data, &games),
        })
        .collect()
}

/// The comparison views' "different move" marks (`DexSettings::show_movepool_diff`,
/// level-up and TM/HM tables only): `marks[i]` is true when `rows[i]`'s move is
/// not in `other` (the other Pokémon's table). Pass it to
/// [`super::table::RowStyle::highlight`].
pub fn diff_marks(rows: &[RowData], other: &[RowData]) -> Vec<bool> {
    let other: std::collections::HashSet<&str> = other.iter().map(|r| r.move_name.as_str()).collect();
    rows.iter().map(|r| !other.contains(r.move_name.as_str())).collect()
}

// ---------------------------------------------------------------------------
// move stats of a row
// ---------------------------------------------------------------------------

/// What the table shows of a move (type, category, power, accuracy, PP).
#[derive(Clone, Debug, PartialEq)]
pub struct MoveInfo {
    pub move_type: String,
    pub category: String,
    pub power: Option<i32>,
    pub accuracy: Option<i32>,
    pub pp: Option<i32>,
}

/// The move's stats in `game` (alias-aware); `None` for an unknown move.
pub fn move_info(name: &str, game: &str) -> Option<MoveInfo> {
    get_move_data(name, game).map(|m| MoveInfo {
        move_type: m.move_type,
        category: m.category,
        power: m.power,
        accuracy: m.accuracy,
        pp: m.pp,
    })
}

// ---------------------------------------------------------------------------
// sorting
// ---------------------------------------------------------------------------

/// A sortable column (Solodex `SortColumn`); `Default` is the table's own order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortColumn {
    Default,
    Move,
    Type,
    Cat,
    Pwr,
    Acc,
    Pp,
}

impl SortColumn {
    /// The header cells, left to right (the first is the level / TM column).
    pub const ALL: [SortColumn; 7] = [
        SortColumn::Default,
        SortColumn::Move,
        SortColumn::Type,
        SortColumn::Cat,
        SortColumn::Pwr,
        SortColumn::Acc,
        SortColumn::Pp,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SortColumn::Default => "Lv",
            SortColumn::Move => "Move",
            SortColumn::Type => "Type",
            SortColumn::Cat => "Cat",
            SortColumn::Pwr => "Pwr",
            SortColumn::Acc => "Acc",
            SortColumn::Pp => "PP",
        }
    }

    /// Pwr / Acc / PP are right-aligned.
    pub fn right_aligned(self) -> bool {
        matches!(self, SortColumn::Pwr | SortColumn::Acc | SortColumn::Pp)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SortState {
    pub column: SortColumn,
    pub dir: SortDir,
}

impl Default for SortState {
    fn default() -> Self {
        SortState {
            column: SortColumn::Default,
            dir: SortDir::Asc,
        }
    }
}

impl SortState {
    /// A click on a header: the same column flips the direction, a new
    /// column sorts ascending, and the first column (or a right-click,
    /// `column = Default`) resets (`useMultiMoveSort.handleSort`).
    pub fn clicked(self, column: SortColumn) -> SortState {
        if column == SortColumn::Default {
            SortState::default()
        } else if column == self.column {
            SortState {
                column,
                dir: self.dir.flip(),
            }
        } else {
            SortState {
                column,
                dir: SortDir::Asc,
            }
        }
    }

    /// The arrow state of a header cell (`None` unless it is the active sort).
    pub fn active(self, column: SortColumn) -> Option<SortDir> {
        (self.column != SortColumn::Default && self.column == column).then_some(self.dir)
    }
}

fn category_order(c: Option<&str>) -> i32 {
    match c {
        Some("Physical") => 0,
        Some("Special") => 1,
        Some("Status") => 2,
        _ => 3,
    }
}

fn nullable(v: Option<i32>, null_value: i32) -> i32 {
    v.unwrap_or(null_value)
}

/// The display order of `rows` under `sort` as indices into `rows`
/// (`sortMoveRows`): a stable sort, descending only negates the comparison
/// so ties keep the table's order. `infos[i]` is `rows[i]`'s move stats.
pub fn sort_order(rows: &[RowData], infos: &[Option<MoveInfo>], sort: SortState) -> Vec<usize> {
    let mut order: Vec<usize> = (0..rows.len()).collect();
    if sort.column == SortColumn::Default {
        return order;
    }
    order.sort_by(|&a, &b| {
        let (ma, mb) = (infos[a].as_ref(), infos[b].as_ref());
        let cmp = match sort.column {
            SortColumn::Default => Ordering::Equal,
            SortColumn::Move => xpr_dex::text::locale_cmp(&rows[a].move_name, &rows[b].move_name),
            SortColumn::Type => xpr_dex::text::locale_cmp(
                ma.map(|m| m.move_type.as_str()).unwrap_or(""),
                mb.map(|m| m.move_type.as_str()).unwrap_or(""),
            ),
            SortColumn::Cat => category_order(ma.map(|m| m.category.as_str()))
                .cmp(&category_order(mb.map(|m| m.category.as_str()))),
            SortColumn::Pwr => {
                nullable(ma.and_then(|m| m.power), -1).cmp(&nullable(mb.and_then(|m| m.power), -1))
            }
            SortColumn::Acc => nullable(ma.and_then(|m| m.accuracy), 101)
                .cmp(&nullable(mb.and_then(|m| m.accuracy), 101)),
            SortColumn::Pp => {
                nullable(ma.and_then(|m| m.pp), 0).cmp(&nullable(mb.and_then(|m| m.pp), 0))
            }
        };
        if sort.dir == SortDir::Desc {
            cmp.reverse()
        } else {
            cmp
        }
    });
    order
}

/// [`sort_order`] looking the moves up itself, returning the sorted rows.
pub fn sort_rows(rows: &[RowData], sort: SortState, game: &str) -> Vec<RowData> {
    if sort.column == SortColumn::Default {
        return rows.to_vec();
    }
    let infos: Vec<Option<MoveInfo>> = rows.iter().map(|r| move_info(&r.move_name, game)).collect();
    sort_order(rows, &infos, sort)
        .into_iter()
        .map(|i| rows[i].clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, prefix: &str, key: i32) -> RowData {
        RowData {
            move_name: name.to_string(),
            sort_key: key,
            prefix: prefix.to_string(),
            game_tags: Vec::new(),
        }
    }

    #[test]
    fn level_labels_and_keys() {
        assert_eq!(level_prefix(0), "Evo");
        assert_eq!(level_prefix(-1), "Rem");
        assert_eq!(level_prefix(7), "7");
        // Rem < 1 < Evo < 2
        assert!(level_sort_key(-1) < level_sort_key(1));
        assert!(level_sort_key(1) < level_sort_key(0));
        assert!(level_sort_key(0) < level_sort_key(2));
    }

    #[test]
    fn extra_level_one_moves_become_reminder_only() {
        let rows: Vec<RowData> = ["A", "B", "C", "D", "E", "F"]
            .iter()
            .map(|n| row(n, "1", level_sort_key(1)))
            .chain([row("G", "5", level_sort_key(5))])
            .collect();
        let rows = apply_remind_labels(rows);
        let labels: Vec<(&str, &str)> = rows
            .iter()
            .map(|r| (r.move_name.as_str(), r.prefix.as_str()))
            .collect();
        assert_eq!(
            labels,
            vec![
                ("A", "Rem"),
                ("B", "Rem"),
                ("C", "1"),
                ("D", "1"),
                ("E", "1"),
                ("F", "1"),
                ("G", "5")
            ]
        );
        // four level-1 moves are untouched
        let four: Vec<RowData> = ["A", "B", "C", "D"]
            .iter()
            .map(|n| row(n, "1", level_sort_key(1)))
            .collect();
        assert!(apply_remind_labels(four).iter().all(|r| r.prefix == "1"));
    }

    #[test]
    fn sort_clicks_follow_solodex() {
        let s = SortState::default();
        let s = s.clicked(SortColumn::Pwr);
        assert_eq!((s.column, s.dir), (SortColumn::Pwr, SortDir::Asc));
        let s = s.clicked(SortColumn::Pwr);
        assert_eq!(s.dir, SortDir::Desc);
        let s = s.clicked(SortColumn::Move);
        assert_eq!((s.column, s.dir), (SortColumn::Move, SortDir::Asc));
        assert_eq!(s.clicked(SortColumn::Default), SortState::default());
        assert_eq!(s.active(SortColumn::Move), Some(SortDir::Asc));
        assert_eq!(s.active(SortColumn::Type), None);
        assert_eq!(SortState::default().active(SortColumn::Default), None);
    }

    #[test]
    fn desc_keeps_ties_in_table_order() {
        let rows = vec![row("A", "", 0), row("B", "", 0), row("C", "", 0)];
        let info = |p: Option<i32>| {
            Some(MoveInfo {
                move_type: "Normal".into(),
                category: "Physical".into(),
                power: p,
                accuracy: None,
                pp: None,
            })
        };
        let infos = vec![info(Some(50)), info(Some(50)), info(None)];
        assert_eq!(
            sort_order(
                &rows,
                &infos,
                SortState {
                    column: SortColumn::Pwr,
                    dir: SortDir::Asc
                }
            ),
            vec![2, 0, 1]
        );
        assert_eq!(
            sort_order(
                &rows,
                &infos,
                SortState {
                    column: SortColumn::Pwr,
                    dir: SortDir::Desc
                }
            ),
            vec![0, 1, 2]
        );
    }
}

#[cfg(test)]
mod diff_tests {
    use super::*;

    #[test]
    fn diff_marks_flag_moves_the_other_side_lacks() {
        let a = vec![RowData::simple("Surf", ""), RowData::simple("Dig", ""), RowData::simple("Cut", "")];
        let b = vec![RowData::simple("Dig", ""), RowData::simple("Fly", "")];
        assert_eq!(diff_marks(&a, &b), vec![true, false, true]);
        assert_eq!(diff_marks(&b, &a), vec![false, true]);
    }

    #[test]
    fn single_and_multi_game_rows() {
        let pikachu = xpr_dex::get_pokemon_data("Pikachu", "Emerald").unwrap();
        let rows = single_level_rows(&pikachu);
        assert_eq!(rows[0].move_name, "ThunderShock");
        assert_eq!(rows[0].prefix, "1");
        // the same moves in two games of one generation: no tags on a shared row, tags on a row only one game has
        let rs = xpr_dex::get_pokemon_data("Pikachu", "Ruby and Sapphire").unwrap();
        let games = vec![
            GenGameData { game: "Ruby and Sapphire".into(), abbrev: "RS".into(), color: Color32::RED, pokemon: rs },
            GenGameData { game: "Emerald".into(), abbrev: "E".into(), color: Color32::GREEN, pokemon: pikachu },
        ];
        let merged = build_level_up_rows(&games);
        let thunder_shock = merged.iter().find(|r| r.move_name == "ThunderShock").unwrap();
        assert!(thunder_shock.game_tags.is_empty());
        let eggs = build_simple_rows(&games, |p| &p.egg_moves);
        assert!(!eggs.is_empty());
        assert!(eggs.iter().all(|r| r.prefix.is_empty() && r.sort_key == 0));
    }
}
