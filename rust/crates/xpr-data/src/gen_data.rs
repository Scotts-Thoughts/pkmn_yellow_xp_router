//! The port of the `GenOne`..`GenFive` classes (`pkmn/gen_X/gen_X_object.py`)
//! and the `CurrentGen` interface, as one struct whose behaviour branches on
//! [`Gen`]. Damage calculation lives in `xpr-calc` and takes a `&GenData`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::Value;

use xpr_core::consts;
use xpr_core::floor_div;

use crate::badges::BadgeList;
use crate::db::{ItemDB, MinBattlesDB, MoveDB, PkmnDB, TrainerDB};
use crate::gen_consts;
use crate::loaders::DataReader;
use crate::model::{EnemyPkmn, Gen, Nature, StatBlock, TrainerTimingStats};
use crate::stats;

pub type TypeChart = IndexMap<String, IndexMap<String, String>>;

#[derive(Debug)]
pub struct GenData {
    pub gen: Gen,
    pub version_name: String,
    pub base_version_name: Option<String>,
    pub all_flat_files: Vec<PathBuf>,

    pub pkmn_db: PkmnDB,
    pub trainer_db: TrainerDB,
    pub item_db: ItemDB,
    pub move_db: MoveDB,
    pub min_battles_db: MinBattlesDB,

    pub special_types: Vec<String>,
    pub type_chart: TypeChart,
    pub held_item_boosts: IndexMap<String, String>,

    pub badge_rewards: Arc<IndexMap<String, String>>,
    pub fight_categories: IndexMap<String, Vec<String>>,
    pub major_fights: HashSet<String>,
    pub trainer_to_category: IndexMap<String, String>,
    pub fight_rewards: IndexMap<String, String>,
    pub branched_mandatory_fights: Vec<String>,
    pub trainer_timing_info: TrainerTimingStats,
}

/// An entry of `get_elite_four_and_champion_names`: the Python list mixes
/// plain names with nested lists (starter-dependent champions).
#[derive(Clone, Debug, PartialEq)]
pub enum E4Entry {
    Single(String),
    Alternatives(Vec<String>),
}

impl E4Entry {
    pub fn names(&self) -> Vec<String> {
        match self {
            E4Entry::Single(s) => vec![s.clone()],
            E4Entry::Alternatives(v) => v.clone(),
        }
    }
}

impl GenData {
    pub fn version_name(&self) -> &str {
        &self.version_name
    }

    pub fn base_version_name(&self) -> Option<&str> {
        self.base_version_name.as_deref()
    }

    /// The base game this data set follows (the version itself for built-in
    /// games, the base version for custom gens).
    pub fn effective_version(&self) -> &str {
        self.base_version_name.as_deref().unwrap_or(&self.version_name)
    }

    pub fn get_generation(&self) -> u8 {
        self.gen.number()
    }

    /// Whether the route can hold bag-reorder events (the gen 1 SELECT swap).
    /// The engine applies such events whatever the version; this gates the
    /// recorder and the UI's ways of creating and filtering them.
    pub fn supports_bag_reorder(&self) -> bool {
        self.get_generation() == 1
    }

    pub fn pkmn_db(&self) -> &PkmnDB {
        &self.pkmn_db
    }
    pub fn item_db(&self) -> &ItemDB {
        &self.item_db
    }
    pub fn trainer_db(&self) -> &TrainerDB {
        &self.trainer_db
    }
    pub fn move_db(&self) -> &MoveDB {
        &self.move_db
    }
    pub fn min_battles_db(&self) -> &MinBattlesDB {
        &self.min_battles_db
    }

    pub fn supported_types(&self) -> Vec<String> {
        self.type_chart.keys().cloned().collect()
    }

    /// `type_chart.get(move_type, {}).get(defending_type)`
    pub fn effectiveness(&self, move_type: &str, defending_type: &str) -> Option<&str> {
        self.type_chart
            .get(move_type)
            .and_then(|row| row.get(defending_type))
            .map(|s| s.as_str())
    }

    pub fn create_trainer_pkmn(&self, pkmn_name: &str, level: i64) -> Option<EnemyPkmn> {
        let species = self.pkmn_db.get_pkmn(pkmn_name)?;
        Some(stats::instantiate_trainer_pokemon(self.gen, species, level, None, Nature::HARDY))
    }

    pub fn create_wild_pkmn(&self, pkmn_name: &str, level: i64, dv: i64) -> Option<EnemyPkmn> {
        let species = self.pkmn_db.get_pkmn(pkmn_name)?;
        Some(stats::instantiate_wild_pokemon(self.gen, species, level, dv, Nature::HARDY))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn make_stat_block(&self, hp: i64, attack: i64, defense: i64, special_attack: i64, special_defense: i64, speed: i64, is_stat_xp: bool) -> StatBlock {
        StatBlock::new(self.gen, hp, attack, defense, special_attack, special_defense, speed, is_stat_xp)
    }

    pub fn make_badge_list(&self) -> BadgeList {
        BadgeList::new(self.gen, self.badge_rewards.clone())
    }

    pub fn bag_limit(&self) -> Option<usize> {
        gen_consts::bag_limit(self.gen)
    }

    pub fn get_stat_modifer_moves(&self) -> Vec<String> {
        let mut result: Vec<String> = self
            .move_db
            .stat_mod_moves
            .keys()
            .filter_map(|k| self.move_db.get_move(k).map(|m| m.name.clone()))
            .collect();
        result.extend(
            self.move_db
                .old_hacky_field_moves
                .keys()
                .filter_map(|k| self.move_db.get_move(k).map(|m| m.name.clone())),
        );
        result.sort();
        result
    }

    pub fn get_field_moves(&self) -> Vec<String> {
        match self.gen {
            Gen::One | Gen::Two | Gen::Three => Vec::new(),
            Gen::Four | Gen::Five => {
                let mut result: Vec<String> = self
                    .move_db
                    .field_moves
                    .keys()
                    .filter_map(|k| self.move_db.get_move(k).map(|m| m.name.clone()))
                    .collect();
                result.push(gen_consts::SLOW_START.to_string());
                result.sort();
                result
            }
        }
    }

    pub fn get_fight_reward(&self, trainer_name: &str) -> Option<&str> {
        self.fight_rewards.get(trainer_name).map(|s| s.as_str())
    }

    pub fn is_major_fight(&self, trainer_name: &str) -> bool {
        self.major_fights.contains(trainer_name)
    }

    pub fn get_fight_category(&self, trainer_name: &str) -> Option<&str> {
        self.trainer_to_category.get(trainer_name).map(|s| s.as_str())
    }

    pub fn is_branched_mandatory_fight(&self, trainer_name: &str) -> bool {
        self.branched_mandatory_fights.iter().any(|t| t == trainer_name)
    }

    pub fn has_branched_mandatory_fights(&self) -> bool {
        !self.branched_mandatory_fights.is_empty()
    }

    fn category(&self, cat: &str) -> Vec<String> {
        self.fight_categories.get(cat).cloned().unwrap_or_default()
    }

    pub fn get_gym_leader_names(&self) -> Vec<String> {
        let all_leaders = self.category(consts::FIGHT_CATEGORY_GYM_LEADER);
        let filter_by = |keys: &[&str]| -> Vec<String> {
            all_leaders
                .iter()
                .filter(|name| keys.iter().any(|k| name.contains(k)))
                .cloned()
                .collect()
        };
        let sort_by_keys = |mut leaders: Vec<String>, keys: &[&str]| -> Vec<String> {
            leaders.sort_by_key(|n| keys.iter().position(|k| n.contains(k)).unwrap_or(99));
            leaders
        };
        let v = self.version_name.as_str();
        match self.gen {
            Gen::One | Gen::Two => all_leaders,
            Gen::Three => {
                if v == consts::FIRE_RED_VERSION || v == consts::LEAF_GREEN_VERSION {
                    filter_by(&["Brock", "Misty", "Lt. surge", "Erika", "Koga", "Sabrina", "Blaine", "Giovanni"])
                } else if v == consts::EMERALD_VERSION {
                    filter_by(&["Roxanne", "Brawly", "Wattson", "Flannery", "Norman", "Winona", "Tate & Liza", "Juan"])
                } else {
                    filter_by(&["Roxanne", "Brawly", "Wattson", "Flannery", "Norman", "Winona", "Tate & Liza", "Wallace"])
                }
            }
            Gen::Four => {
                if v == consts::HEART_GOLD_VERSION || v == consts::SOUL_SILVER_VERSION {
                    filter_by(&["Falkner", "Bugsy", "Whitney", "Morty", "Chuck", "Jasmine", "Pryce", "Clair"])
                } else if v == consts::PLATINUM_VERSION {
                    let sinnoh = ["Roark", "Gardenia", "Fantina", "Maylene", "Wake", "Byron", "Candice", "Volkner"];
                    sort_by_keys(filter_by(&sinnoh), &sinnoh)
                } else {
                    filter_by(&["Roark", "Gardenia", "Maylene", "Wake", "Fantina", "Byron", "Candice", "Volkner"])
                }
            }
            Gen::Five => {
                let unova: Vec<&str> = if self.is_bw2() {
                    vec!["Cheren", "Roxie", "Burgh", "Elesa", "Clay", "Skyla", "Drayden", "Marlon"]
                } else {
                    vec!["Chili", "Cilan", "Cress", "Lenora", "Burgh", "Elesa", "Clay", "Skyla", "Brycen", "Drayden", "Iris"]
                };
                sort_by_keys(filter_by(&unova), &unova)
            }
        }
    }

    fn is_bw2(&self) -> bool {
        let name = self.effective_version();
        name == consts::BLACK_2_VERSION || name == consts::WHITE_2_VERSION
    }

    pub fn get_elite_four_and_champion_names(&self) -> Vec<E4Entry> {
        let all_e4 = self.category(consts::FIGHT_CATEGORY_ELITE_FOUR);
        let all_champ = self.category(consts::FIGHT_CATEGORY_CHAMPION);
        let post = self.category(consts::FIGHT_CATEGORY_POST_GAME);
        let singles = |v: &[String]| -> Vec<E4Entry> { v.iter().map(|s| E4Entry::Single(s.clone())).collect() };
        let filter = |src: &[String], keys: &[&str]| -> Vec<String> {
            src.iter().filter(|n| keys.iter().any(|k| n.contains(k))).cloned().collect()
        };
        let v = self.version_name.as_str();
        match self.gen {
            Gen::One => {
                let mut r = singles(&all_e4);
                r.push(E4Entry::Alternatives(all_champ));
                r
            }
            Gen::Two => {
                let red: Vec<String> = post.iter().filter(|n| n.contains("Red")).cloned().collect();
                let mut r = singles(&all_e4);
                r.extend(singles(&all_champ));
                r.extend(singles(&red));
                r
            }
            Gen::Three => {
                if v == consts::FIRE_RED_VERSION || v == consts::LEAF_GREEN_VERSION {
                    let e4 = filter(&all_e4, &["Lorelei", "Bruno", "Agatha", "Lance"]);
                    let champ = filter(&all_champ, &["Squirtle", "Bulbasaur", "Charmander", "Terry"]);
                    let mut r = singles(&e4);
                    r.push(E4Entry::Alternatives(champ));
                    r
                } else {
                    let e4 = filter(&all_e4, &["Sidney", "Phoebe", "Glacia", "Drake"]);
                    let champ: Vec<String> = all_champ.iter().filter(|n| n.contains("Wallace")).cloned().collect();
                    let steven: Vec<String> = post.iter().filter(|n| n.contains("Steven")).cloned().collect();
                    let mut r = singles(&e4);
                    r.extend(singles(&champ));
                    r.extend(singles(&steven));
                    r
                }
            }
            Gen::Four => {
                if v == consts::HEART_GOLD_VERSION || v == consts::SOUL_SILVER_VERSION {
                    let e4: Vec<String> = all_e4
                        .iter()
                        .filter(|n| ["Will", "Koga", "Bruno", "Karen"].iter().any(|k| n.contains(k)) && !n.contains("Rematch"))
                        .cloned()
                        .collect();
                    let lance_first: Vec<String> = all_champ.iter().filter(|n| n.contains("Lance") && !n.contains("Rematch")).cloned().collect();
                    let lance_rematch: Vec<String> = all_champ.iter().filter(|n| n.contains("Lance") && n.contains("Rematch 2")).cloned().collect();
                    let red: Vec<String> = post.iter().filter(|n| n.contains("Red")).cloned().collect();
                    let mut r = singles(&e4);
                    r.extend(singles(&lance_first));
                    r.extend(singles(&lance_rematch));
                    r.extend(singles(&red));
                    r
                } else {
                    let e4 = filter(&all_e4, &["Aaron", "Bertha", "Flint", "Lucian"]);
                    let champ: Vec<String> = all_champ.iter().filter(|n| n.contains("Cynthia")).cloned().collect();
                    let mut r = singles(&e4);
                    r.extend(singles(&champ));
                    r
                }
            }
            Gen::Five => {
                let order = ["Shauntal", "Marshal", "Grimsley", "Caitlin"];
                let mut e4 = filter(&all_e4, &order);
                e4.sort_by_key(|n| order.iter().position(|k| n.contains(k)).unwrap_or(99));
                let mut r = singles(&e4);
                r.extend(singles(&all_champ));
                r
            }
        }
    }

    pub fn get_move_custom_data(&self, move_name: &str) -> Option<&'static Vec<String>> {
        gen_consts::custom_move_data_table(self.gen).get(move_name)
    }

    /// `get_hidden_power(dvs) -> (type, base_power)`
    pub fn get_hidden_power(&self, dvs: &StatBlock) -> (String, i64) {
        match self.gen {
            Gen::One => (String::new(), 0),
            Gen::Two => (
                stats::hidden_power_type_gen2(dvs).to_string(),
                stats::hidden_power_base_power_gen2(dvs),
            ),
            _ => (
                stats::hidden_power_type_gen345(dvs).to_string(),
                stats::hidden_power_base_power_gen345(dvs),
            ),
        }
    }

    /// `get_natural_gift(held_item) -> (type, base_power)`
    pub fn get_natural_gift(&self, held_item: Option<&str>) -> Option<(String, i64)> {
        gen_consts::natural_gift(self.gen, held_item).map(|(p, t)| (t.to_string(), p))
    }

    pub fn get_valid_weather(&self) -> Vec<&'static str> {
        match self.gen {
            Gen::One => vec![consts::WEATHER_NONE],
            Gen::Two => vec![consts::WEATHER_NONE, consts::WEATHER_SUN, consts::WEATHER_RAIN, consts::WEATHER_SANDSTORM],
            Gen::Three | Gen::Five => vec![
                consts::WEATHER_NONE,
                consts::WEATHER_SUN,
                consts::WEATHER_RAIN,
                consts::WEATHER_SANDSTORM,
                consts::WEATHER_HAIL,
            ],
            Gen::Four => vec![
                consts::WEATHER_NONE,
                consts::WEATHER_SUN,
                consts::WEATHER_RAIN,
                consts::WEATHER_SANDSTORM,
                consts::WEATHER_HAIL,
                consts::WEATHER_FOG,
            ],
        }
    }

    pub fn get_stats_boosted_by_vitamin(&self, vit_name: &str) -> Result<Vec<&'static str>, String> {
        let gen12 = matches!(self.gen, Gen::One | Gen::Two);
        match vit_name {
            consts::HP_UP => Ok(vec![consts::HP]),
            consts::PROTEIN => Ok(vec![consts::ATK]),
            consts::IRON => Ok(vec![consts::DEF]),
            consts::CALCIUM => Ok(if gen12 { vec![consts::SPA, consts::SPD] } else { vec![consts::SPA] }),
            consts::ZINC if !gen12 => Ok(vec![consts::SPD]),
            consts::CARBOS => Ok(vec![consts::SPE]),
            other => Err(format!("Unknown vitamin: {}", other)),
        }
    }

    pub fn get_valid_vitamins(&self) -> Vec<&'static str> {
        match self.gen {
            Gen::One | Gen::Two => vec![consts::HP_UP, consts::CARBOS, consts::IRON, consts::CALCIUM, consts::PROTEIN],
            _ => vec![consts::HP_UP, consts::CARBOS, consts::IRON, consts::CALCIUM, consts::ZINC, consts::PROTEIN],
        }
    }

    pub fn get_vitamin_amount(&self) -> i64 {
        stats::vitamin_consts(self.gen).vit_amt
    }
    pub fn get_vitamin_use_cap(&self) -> i64 {
        stats::vitamin_consts(self.gen).vit_cap
    }
    pub fn get_vitamin_value_cap(&self) -> i64 {
        stats::vitamin_consts(self.gen).value_cap
    }

    pub fn get_valid_ev_berries(&self) -> Vec<&'static str> {
        match self.gen {
            Gen::One | Gen::Two => Vec::new(),
            _ => vec![
                consts::POMEG_BERRY,
                consts::KELPSY_BERRY,
                consts::QUALOT_BERRY,
                consts::HONDEW_BERRY,
                consts::GREPA_BERRY,
                consts::TAMATO_BERRY,
            ],
        }
    }

    pub fn is_ev_berry(&self, name: &str) -> bool {
        self.get_valid_ev_berries().contains(&name)
    }

    pub fn get_stats_lowered_by_ev_berry(&self, berry_name: &str) -> Result<Vec<&'static str>, String> {
        if matches!(self.gen, Gen::One | Gen::Two) {
            return Err(format!("Unknown EV berry: {}", berry_name));
        }
        match berry_name {
            consts::POMEG_BERRY => Ok(vec![consts::HP]),
            consts::KELPSY_BERRY => Ok(vec![consts::ATK]),
            consts::QUALOT_BERRY => Ok(vec![consts::DEF]),
            consts::HONDEW_BERRY => Ok(vec![consts::SPA]),
            consts::GREPA_BERRY => Ok(vec![consts::SPD]),
            consts::TAMATO_BERRY => Ok(vec![consts::SPE]),
            other => Err(format!("Unknown EV berry: {}", other)),
        }
    }

    pub fn get_ev_berry_reduced_value(&self, cur_stat_xp: i64) -> i64 {
        match self.gen {
            // subtract first, then clamp to 100 (pokeplatinum CalculateEVUpdate,
            // pokeheartgold TryModEV): 255 -> 100 and 110 -> 100, but 105 -> 95
            Gen::Four => (cur_stat_xp - stats::EV_BERRY_AMT).min(stats::EV_BERRY_DROP_TARGET_GEN4).max(0),
            _ => (cur_stat_xp - stats::EV_BERRY_AMT).max(0),
        }
    }

    pub fn get_trainer_timing_info(&self) -> &TrainerTimingStats {
        &self.trainer_timing_info
    }

    /// `get_stat_xp_yield`; `None` when the species is unknown (Python raises).
    pub fn get_stat_xp_yield(&self, pkmn_name: &str, exp_split: i64, held_item: Option<&str>) -> Option<StatBlock> {
        let species = self.pkmn_db.get_pkmn(pkmn_name)?;
        let y = species.stat_xp_yield;
        match self.gen {
            Gen::One | Gen::Two => {
                let d = if exp_split == 0 { 1 } else { exp_split };
                Some(StatBlock::new(
                    self.gen,
                    floor_div(y.hp, d),
                    floor_div(y.attack, d),
                    floor_div(y.defense, d),
                    floor_div(y.special_attack, d),
                    floor_div(y.special_defense, d),
                    floor_div(y.speed, d),
                    true,
                ))
            }
            Gen::Three => {
                if held_item == Some(consts::MACHO_BRACE_ITEM_NAME) {
                    return Some(y.add(&y));
                }
                Some(y)
            }
            Gen::Four | Gen::Five => {
                if held_item == Some(consts::MACHO_BRACE_ITEM_NAME) {
                    return Some(y.add(&y));
                }
                let bonus = |hp, atk, def, spa, spd, spe| StatBlock::new(self.gen, hp, atk, def, spa, spd, spe, true);
                Some(match held_item {
                    Some(consts::POWER_WEIGHT_ITEM_NAME) => y.add(&bonus(4, 0, 0, 0, 0, 0)),
                    Some(consts::POWER_BRACER_ITEM_NAME) => y.add(&bonus(0, 4, 0, 0, 0, 0)),
                    Some(consts::POWER_BELT_ITEM_NAME) => y.add(&bonus(0, 0, 4, 0, 0, 0)),
                    Some(consts::POWER_LENS_ITEM_NAME) => y.add(&bonus(0, 0, 0, 4, 0, 0)),
                    Some(consts::POWER_BAND_ITEM_NAME) => y.add(&bonus(0, 0, 0, 0, 4, 0)),
                    Some(consts::POWER_ANKLET_ITEM_NAME) => y.add(&bonus(0, 0, 0, 0, 0, 4)),
                    _ => y,
                })
            }
        }
    }

    pub fn get_money_after_blackout(&self, cur_money: i64, mon_level: i64, badges: &BadgeList) -> i64 {
        match self.gen {
            Gen::One | Gen::Two => cur_money.div_euclid(2),
            Gen::Three => {
                let is_frlg = self
                    .base_version_name
                    .as_deref()
                    .map(|b| consts::FRLG_VERSIONS.contains(&b))
                    .unwrap_or(false)
                    || consts::FRLG_VERSIONS.contains(&self.version_name.as_str());
                if is_frlg {
                    let base_val = stats::blackout_base_val(badges.num_badges());
                    (cur_money - (base_val * mon_level)).max(0)
                } else {
                    cur_money.div_euclid(2)
                }
            }
            Gen::Four | Gen::Five => {
                let base_val = stats::blackout_base_val(badges.num_badges());
                (cur_money - (base_val * mon_level)).max(0)
            }
        }
    }

    /// `create_new_custom_gen`: copy the flat files into a new folder under
    /// the custom gens directory and write the metadata file.
    pub fn create_new_custom_gen(&self, custom_gens_dir: &Path, new_version_name: &str, reader: &dyn DataReader) -> Result<PathBuf, String> {
        let folder = xpr_core::io_utils::get_safe_path_no_collision(custom_gens_dir, new_version_name, "");
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        for cur_file in &self.all_flat_files {
            let Some(fname) = cur_file.file_name() else { continue };
            std::fs::write(folder.join(fname), reader.read_bytes(cur_file)?).map_err(|e| e.to_string())?;
        }
        let meta = xpr_core::pyjson::object(vec![
            (consts::CUSTOM_GEN_NAME_KEY, Value::String(new_version_name.to_string())),
            (consts::BASE_GEN_NAME_KEY, Value::String(self.version_name.clone())),
        ]);
        std::fs::write(
            folder.join(consts::CUSTOM_GEN_META_FILE_NAME),
            xpr_core::pyjson::dump_indent4_platform_bytes(&meta),
        )
        .map_err(|e| e.to_string())?;
        Ok(folder)
    }
}
