//! Port of the state and computation of
//! `controllers/battle_summary_controller.py`. Route-mutating operations
//! (prefight candies, held item, vitamin shadows, tutor moves, matchup
//! reorder) are orchestrated by the application controller on top of the
//! `save_transient_state` / `restore_transient_state` helpers here.

use std::collections::{BTreeSet, HashMap};

use indexmap::IndexMap;
use serde_json::{json, Value};

use xpr_core::consts;
use xpr_core::CalcConfig;
use xpr_data::db::StatStageInfo;
use xpr_data::model::{CustomMoveData, EnemyPkmn, FieldStatus, Gen, StageModifiers, StatBlock};
use xpr_data::GenData;
use xpr_engine::{EventDefinition, NodeId, RouteState, Router, TrainerEventDefinition};

use crate::damage::{self, find_kill, DamageRange, KillRange};
use crate::{calculate_damage, get_crit_rate, get_move_accuracy, DamageArgs};

#[derive(Clone, Debug, PartialEq)]
pub struct MoveRenderInfo {
    pub name: String,
    pub attack_flavor: Vec<String>,
    pub min_damage: i64,
    pub max_damage: i64,
    pub crit_min_damage: i64,
    pub crit_max_damage: i64,
    pub defending_mon_hp: i64,
    pub kill_ranges: Vec<KillRange>,
    pub mimic_data: String,
    pub mimic_options: Vec<String>,
    pub custom_data_options: Option<Vec<String>>,
    pub custom_data_selection: Option<String>,
    pub attacking_mon_hp: i64,
    pub is_best_move: bool,
    pub stat_stage_options: Option<Vec<String>>,
    pub stat_stage_selection: String,
    pub stat_stage_info: StatStageInfo,
}

impl MoveRenderInfo {
    /// `asdict()`-style JSON for the golden corpus.
    pub fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "attack_flavor": self.attack_flavor,
            "min_damage": self.min_damage,
            "max_damage": self.max_damage,
            "crit_min_damage": self.crit_min_damage,
            "crit_max_damage": self.crit_max_damage,
            "defending_mon_hp": self.defending_mon_hp,
            "kill_ranges": self.kill_ranges.iter().map(|(n, p)| json!([n, p])).collect::<Vec<_>>(),
            "mimic_data": self.mimic_data,
            "mimic_options": self.mimic_options,
            "custom_data_options": self.custom_data_options,
            "custom_data_selection": self.custom_data_selection,
            "attacking_mon_hp": self.attacking_mon_hp,
            "is_best_move": self.is_best_move,
            "stat_stage_options": self.stat_stage_options,
            "stat_stage_selection": self.stat_stage_selection,
            "stat_stage_info": stat_stage_info_json(&self.stat_stage_info),
        })
    }
}

fn stat_stage_info_json(i: &StatStageInfo) -> Value {
    if !i.has_stat_effect {
        return json!({"has_stat_effect": false});
    }
    json!({
        "has_stat_effect": true,
        "is_guaranteed": i.is_guaranteed,
        "targets_self": i.targets_self,
        "is_damaging": i.is_damaging,
        "max_applications": i.max_applications,
        "stage_change": i.stage_change,
        "is_belly_drum": i.is_belly_drum,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct PkmnRenderInfo {
    pub attacking_mon_name: String,
    pub attacking_mon_level: i64,
    pub attacking_mon_speed: i64,
    pub defending_mon_name: String,
    pub defending_mon_level: i64,
    pub defending_mon_speed: i64,
    pub defending_mon_hp: i64,
}

impl PkmnRenderInfo {
    /// `__str__`
    pub fn to_string(&self) -> String {
        let verb = if self.attacking_mon_speed > self.defending_mon_speed {
            "outspeeds"
        } else if self.defending_mon_speed > self.attacking_mon_speed {
            "underspeeds"
        } else {
            "speed-ties"
        };
        format!(
            "Lv {}: {} {} Lv {}: {} ({} HP)",
            self.attacking_mon_level, self.attacking_mon_name, verb, self.defending_mon_level, self.defending_mon_name, self.defending_mon_hp
        )
    }

    pub fn to_json(&self) -> Value {
        json!({
            "attacking_mon_name": self.attacking_mon_name,
            "attacking_mon_level": self.attacking_mon_level,
            "attacking_mon_speed": self.attacking_mon_speed,
            "defending_mon_name": self.defending_mon_name,
            "defending_mon_level": self.defending_mon_level,
            "defending_mon_speed": self.defending_mon_speed,
            "defending_mon_hp": self.defending_mon_hp,
        })
    }
}

/// Everything the refresh reads from the config and the route.
#[derive(Clone, Debug)]
pub struct SummaryConfig {
    pub calc: CalcConfig,
    pub player_strategy: String,
    pub enemy_strategy: String,
    pub test_moves_enabled: bool,
    pub test_moves: Vec<String>,
}

impl Default for SummaryConfig {
    fn default() -> Self {
        SummaryConfig {
            calc: CalcConfig::default(),
            player_strategy: consts::HIGHLIGHT_FASTEST_KILL.to_string(),
            enemy_strategy: consts::HIGHLIGHT_FASTEST_KILL.to_string(),
            test_moves_enabled: false,
            test_moves: vec![String::new(); 4],
        }
    }
}

/// The transient state the route-change cascade would otherwise clobber
/// (`_save_transient_state`).
#[derive(Clone, Debug)]
pub struct TransientState {
    player_setup: Vec<String>,
    player_field: Vec<String>,
    enemy_setup: Vec<String>,
    enemy_field: Vec<String>,
    stat_stage: Vec<CustomMoveData>,
    custom_data: Vec<CustomMoveData>,
    weather: Option<String>,
    weather_source_mon_idx: Option<i64>,
    player_screens: IndexMap<String, i64>,
    enemy_screens: IndexMap<String, i64>,
    player_intimidate: Option<BTreeSet<i64>>,
    enemy_intimidate: Option<BTreeSet<i64>>,
    mimic: String,
    transformed: bool,
    double: bool,
}

#[derive(Clone, Debug)]
pub struct BattleSummary {
    pub trainer_name: String,
    pub second_trainer_name: String,
    /// the loose JSON value of the definition (old routes store e.g. `0`)
    pub second_trainer_name_raw: Value,
    pub event_group_id: Option<NodeId>,

    pub original_player_mon_list: Vec<EnemyPkmn>,
    pub player_setup_move_list: Vec<String>,
    pub player_field_move_list: Vec<String>,
    pub is_player_transformed: bool,
    pub transformed_mon_list: Vec<EnemyPkmn>,
    pub original_enemy_mon_list: Vec<EnemyPkmn>,
    pub enemy_setup_move_list: Vec<String>,
    pub enemy_field_move_list: Vec<String>,

    pub mimic_options: Vec<String>,
    pub mimic_selection: String,
    pub custom_move_data: Vec<CustomMoveData>,
    pub cached_definition_order: Vec<i64>,
    pub weather: Option<String>,
    pub weather_source_mon_idx: Option<i64>,
    pub player_screens: IndexMap<String, i64>,
    pub enemy_screens: IndexMap<String, i64>,
    pub player_intimidate: Option<BTreeSet<i64>>,
    pub enemy_intimidate: Option<BTreeSet<i64>>,
    pub double_battle_flag: bool,
    pub stat_stage_setup: Vec<CustomMoveData>,
    pub collapsed_mons: BTreeSet<i64>,
    pub is_wild_battle: bool,
    pub wild_min_dv_mons: Vec<EnemyPkmn>,
    pub wild_max_dv_mons: Vec<EnemyPkmn>,

    pub using_global_setup: bool,
    pub player_stage_modifier: StageModifiers,
    pub enemy_stage_modifier: StageModifiers,
    pub player_field_statuses: Vec<FieldStatus>,
    pub enemy_field_statuses: Vec<FieldStatus>,
    pub per_matchup_player_modifiers: Vec<StageModifiers>,
    pub per_matchup_enemy_modifiers: Vec<StageModifiers>,

    pub player_move_data: Vec<Vec<Option<MoveRenderInfo>>>,
    pub enemy_move_data: Vec<Vec<Option<MoveRenderInfo>>>,
    pub player_pkmn_matchup_data: Vec<PkmnRenderInfo>,
    pub enemy_pkmn_matchup_data: Vec<PkmnRenderInfo>,

    /// `{stat: (shadow_id, original_id)}`
    pub vitamin_shadows: HashMap<String, (Option<NodeId>, Option<NodeId>)>,
    /// `_move_highlights[event_group_id][(mon_idx, move_idx, is_player)]`
    pub move_highlights: HashMap<Option<NodeId>, HashMap<(i64, i64, bool), i64>>,
    pub suppress_refresh: bool,
    pub held_item_update_in_flight: bool,
}

impl Default for BattleSummary {
    fn default() -> Self {
        BattleSummary {
            trainer_name: String::new(),
            second_trainer_name: String::new(),
            second_trainer_name_raw: Value::Null,
            event_group_id: None,
            original_player_mon_list: Vec::new(),
            player_setup_move_list: Vec::new(),
            player_field_move_list: Vec::new(),
            is_player_transformed: false,
            transformed_mon_list: Vec::new(),
            original_enemy_mon_list: Vec::new(),
            enemy_setup_move_list: Vec::new(),
            enemy_field_move_list: Vec::new(),
            mimic_options: Vec::new(),
            mimic_selection: String::new(),
            custom_move_data: Vec::new(),
            cached_definition_order: Vec::new(),
            weather: Some(consts::WEATHER_NONE.to_string()),
            weather_source_mon_idx: None,
            player_screens: IndexMap::new(),
            enemy_screens: IndexMap::new(),
            player_intimidate: None,
            enemy_intimidate: None,
            double_battle_flag: false,
            stat_stage_setup: Vec::new(),
            collapsed_mons: BTreeSet::new(),
            is_wild_battle: false,
            wild_min_dv_mons: Vec::new(),
            wild_max_dv_mons: Vec::new(),
            using_global_setup: false,
            player_stage_modifier: StageModifiers::default(),
            enemy_stage_modifier: StageModifiers::default(),
            player_field_statuses: Vec::new(),
            enemy_field_statuses: Vec::new(),
            per_matchup_player_modifiers: Vec::new(),
            per_matchup_enemy_modifiers: Vec::new(),
            player_move_data: Vec::new(),
            enemy_move_data: Vec::new(),
            player_pkmn_matchup_data: Vec::new(),
            enemy_pkmn_matchup_data: Vec::new(),
            vitamin_shadows: HashMap::new(),
            move_highlights: HashMap::new(),
            suppress_refresh: false,
            held_item_update_in_flight: false,
        }
    }
}

/// Python list indexing (`list[idx]` with negative indices counting from the
/// end); `None` where Python would raise `IndexError`.
fn py_get<T>(list: &[T], idx: i64) -> Option<&T> {
    let n = list.len() as i64;
    let real = if idx < 0 { idx + n } else { idx };
    if real < 0 || real >= n {
        None
    } else {
        list.get(real as usize)
    }
}

fn empty_side_maps(n: usize) -> Vec<CustomMoveData> {
    (0..n).map(|_| CustomMoveData::default()).collect()
}

impl BattleSummary {
    pub fn new() -> BattleSummary {
        BattleSummary::default()
    }

    // ---- loading -------------------------------------------------------------

    /// `load_empty`
    pub fn load_empty(&mut self, gen: Option<&GenData>, cfg: &SummaryConfig) {
        self.vitamin_shadows.clear();
        self.event_group_id = None;
        self.trainer_name = String::new();
        self.second_trainer_name = String::new();
        self.second_trainer_name_raw = Value::String(String::new());
        self.weather = Some(consts::WEATHER_NONE.to_string());
        self.weather_source_mon_idx = None;
        self.player_screens.clear();
        self.enemy_screens.clear();
        self.player_intimidate = None;
        self.enemy_intimidate = None;
        self.double_battle_flag = false;
        self.mimic_selection = String::new();
        self.is_player_transformed = false;
        self.is_wild_battle = false;
        self.wild_min_dv_mons.clear();
        self.wild_max_dv_mons.clear();
        self.player_setup_move_list.clear();
        self.player_field_move_list.clear();
        self.enemy_setup_move_list.clear();
        self.enemy_field_move_list.clear();
        self.custom_move_data.clear();
        self.stat_stage_setup.clear();
        self.collapsed_mons.clear();
        self.original_player_mon_list.clear();
        self.transformed_mon_list.clear();
        self.original_enemy_mon_list.clear();
        self.cached_definition_order.clear();
        self.full_refresh(gen, cfg);
    }

    /// `load_from_event`
    pub fn load_from_event(&mut self, router: &Router, group_id: Option<NodeId>, cfg: &SummaryConfig) -> Result<(), String> {
        let gen = router.gen().cloned();
        let Some(gid) = group_id else {
            self.load_empty(gen.as_deref(), cfg);
            return Ok(());
        };
        let Some(group) = router.group(gid) else {
            self.load_empty(gen.as_deref(), cfg);
            return Ok(());
        };
        let Some(td) = group.event_definition.trainer_def.clone() else {
            self.load_empty(gen.as_deref(), cfg);
            return Ok(());
        };
        let gen = gen.ok_or("no gen")?;
        let ed = &group.event_definition;
        let trainer_obj = ed.get_first_trainer_obj(&gen)?.ok_or("no trainer")?.clone();
        let second_trainer_obj = ed.get_second_trainer_obj(&gen)?.cloned();

        if Some(gid) != self.event_group_id {
            self.vitamin_shadows.clear();
        }
        self.event_group_id = Some(gid);
        self.trainer_name = td.trainer_name.clone();
        self.second_trainer_name = td.second_trainer_name_str().unwrap_or("").to_string();
        self.second_trainer_name_raw = td.second_trainer_name.clone();
        self.weather = td.weather.clone();
        self.weather_source_mon_idx = td.weather_source_mon_idx;
        self.player_screens = td.player_screens.clone();
        self.enemy_screens = td.enemy_screens.clone();
        self.double_battle_flag = trainer_obj.double_battle || second_trainer_obj.is_some();
        self.mimic_selection = td.mimic_selection.clone();
        self.is_player_transformed = td.is_transformed();
        self.is_wild_battle = false;
        self.wild_min_dv_mons.clear();
        self.wild_max_dv_mons.clear();
        self.player_setup_move_list = td.setup_moves.clone();
        self.player_field_move_list = td.player_field_moves.clone();
        self.player_stage_modifier = calc_stage_modifier(&gen, &self.player_setup_move_list);
        self.enemy_setup_move_list = td.enemy_setup_moves.clone();
        self.enemy_field_move_list = td.enemy_field_moves.clone();
        self.enemy_stage_modifier = calc_stage_modifier(&gen, &self.enemy_setup_move_list);
        self.cached_definition_order = ed.get_pokemon_list(&gen, true)?.iter().map(|(_, m)| m.mon_order - 1).collect();
        // intimidate sets (definition order -> display order) need the cached order
        self.player_intimidate = self.intimidate_from_def_order(td.player_intimidate.as_deref());
        self.enemy_intimidate = self.intimidate_from_def_order(td.enemy_intimidate.as_deref());

        let pokemon_list = ed.pokemon_list(&gen)?;
        if td.custom_move_data.is_empty() {
            self.custom_move_data = empty_side_maps(pokemon_list.len());
        } else {
            self.custom_move_data = pokemon_list
                .iter()
                .map(|m| m.custom_move_data.clone().unwrap_or_default())
                .collect();
        }
        if !td.stat_stage_setup.is_empty() {
            self.stat_stage_setup = Vec::new();
            for display_idx in 0..pokemon_list.len() {
                // Python: `if def_idx < len(list): list[def_idx]` -- a mon with
                // mon_order 0 gives def_idx -1, which indexes the *last* entry
                let entry = self
                    .cached_definition_order
                    .get(display_idx)
                    .and_then(|def_idx| py_get(&td.stat_stage_setup, *def_idx).cloned())
                    .unwrap_or_default();
                self.stat_stage_setup.push(entry);
            }
        } else {
            self.stat_stage_setup = empty_side_maps(pokemon_list.len());
        }

        self.collapsed_mons.clear();
        if !td.collapsed_mons.is_empty() {
            for (display_idx, def_idx) in self.cached_definition_order.iter().enumerate() {
                if td.collapsed_mons.contains(def_idx) {
                    self.collapsed_mons.insert(display_idx as i64);
                }
            }
        }

        self.original_player_mon_list.clear();
        self.transformed_mon_list.clear();
        self.original_enemy_mon_list.clear();

        let mut cur_item_idx = 0usize;
        let items: Vec<NodeId> = group.event_items.clone();
        for cur_pkmn in &pokemon_list {
            while cur_item_idx < items.len() {
                let item = router.item(items[cur_item_idx]).ok_or("missing item")?;
                cur_item_idx += 1;
                if item.event_definition.trainer_def.is_none() {
                    continue;
                }
                if item.to_defeat_mon.as_ref().map(|m| m.py_eq(cur_pkmn)).unwrap_or(false) {
                    self.original_enemy_mon_list.push(cur_pkmn.clone());
                    let mut transformed = pokemon_list[0].clone();
                    let init = item.init_state.as_ref().ok_or("no init state")?;
                    let player = init.solo_pkmn.get_pkmn_obj(&init.badges, Some(&self.player_stage_modifier));
                    transformed.level = player.level;
                    transformed.cur_stats.hp = player.cur_stats.hp;
                    self.transformed_mon_list.push(transformed);
                    self.original_player_mon_list.push(player);
                    break;
                }
            }
        }
        self.full_refresh(Some(&gen), cfg);
        Ok(())
    }

    /// `load_from_state`
    pub fn load_from_state(&mut self, gen: &GenData, init_state: Option<&RouteState>, enemy_mons: &[EnemyPkmn], trainer_name: Option<&str>, is_wild: bool, cfg: &SummaryConfig) -> Result<(), String> {
        let Some(init_state) = init_state else {
            self.load_empty(Some(gen), cfg);
            return Ok(());
        };
        if enemy_mons.is_empty() {
            self.load_empty(Some(gen), cfg);
            return Ok(());
        }
        self.event_group_id = None;
        self.trainer_name = trainer_name.unwrap_or("").to_string();
        self.second_trainer_name = String::new();
        self.second_trainer_name_raw = Value::String(String::new());
        self.weather = Some(consts::WEATHER_NONE.to_string());
        self.weather_source_mon_idx = None;
        self.player_screens.clear();
        self.enemy_screens.clear();
        self.player_intimidate = None;
        self.enemy_intimidate = None;
        self.mimic_selection = String::new();
        self.is_player_transformed = false;
        self.player_setup_move_list.clear();
        self.player_field_move_list.clear();
        self.enemy_setup_move_list.clear();
        self.enemy_field_move_list.clear();
        self.custom_move_data = empty_side_maps(enemy_mons.len());
        self.stat_stage_setup = empty_side_maps(enemy_mons.len());
        self.collapsed_mons.clear();
        self.is_wild_battle = is_wild;
        self.wild_min_dv_mons.clear();
        self.wild_max_dv_mons.clear();
        self.cached_definition_order = (0..enemy_mons.len() as i64).collect();
        self.original_player_mon_list.clear();
        self.original_enemy_mon_list.clear();
        // NOTE: Python does not clear _transformed_mon_list here

        let mut cur_state = init_state.clone();
        for cur_enemy in enemy_mons {
            self.original_enemy_mon_list.push(cur_enemy.clone());
            let mut transformed = enemy_mons[0].clone();
            let player = cur_state.solo_pkmn.get_pkmn_obj(&cur_state.badges, None);
            transformed.level = player.level;
            transformed.cur_stats.hp = player.cur_stats.hp;
            self.transformed_mon_list.push(transformed);
            self.original_player_mon_list.push(player);
            if is_wild {
                if let Some(m) = gen.create_wild_pkmn(&cur_enemy.name, cur_enemy.level, 0) {
                    self.wild_min_dv_mons.push(m);
                }
                if let Some(m) = gen.create_wild_pkmn(&cur_enemy.name, cur_enemy.level, 15) {
                    self.wild_max_dv_mons.push(m);
                }
            }
            cur_state = cur_state.defeat_pkmn(gen, cur_enemy, None, 1, 0)?.0;
        }
        self.double_battle_flag = trainer_name
            .and_then(|t| gen.trainer_db().get_trainer(t))
            .map(|t| t.double_battle)
            .unwrap_or(false);
        self.full_refresh(Some(gen), cfg);
        Ok(())
    }

    // ---- refresh -------------------------------------------------------------

    /// `_full_refresh`
    pub fn full_refresh(&mut self, gen: Option<&GenData>, cfg: &SummaryConfig) {
        if self.suppress_refresh {
            return;
        }
        let Some(gen) = gen else {
            self.player_pkmn_matchup_data.clear();
            self.enemy_pkmn_matchup_data.clear();
            self.player_move_data.clear();
            self.enemy_move_data.clear();
            self.mimic_options.clear();
            return;
        };
        let using_player = !self.player_setup_move_list.is_empty() && self.player_setup_move_list.iter().any(|m| !m.is_empty());
        let using_enemy = !self.enemy_setup_move_list.is_empty() && self.enemy_setup_move_list.iter().any(|m| !m.is_empty());
        self.using_global_setup = using_player || using_enemy;

        self.player_stage_modifier = calc_stage_modifier(gen, &self.player_setup_move_list);
        self.enemy_stage_modifier = calc_stage_modifier(gen, &self.enemy_setup_move_list);
        let num_matchups = self.original_player_mon_list.len();
        self.player_field_statuses = (0..num_matchups).map(|i| self.calc_field_status(true, i as i64)).collect();
        self.enemy_field_statuses = (0..num_matchups).map(|i| self.calc_field_status(false, i as i64)).collect();

        self.per_matchup_player_modifiers = Vec::new();
        self.per_matchup_enemy_modifiers = Vec::new();
        if !self.using_global_setup {
            let (p, e) = self.calc_per_matchup_stage_modifiers(gen);
            self.per_matchup_player_modifiers = p;
            self.per_matchup_enemy_modifiers = e;
        }

        self.player_pkmn_matchup_data.clear();
        self.enemy_pkmn_matchup_data.clear();
        self.player_move_data.clear();
        self.enemy_move_data.clear();
        self.mimic_options.clear();

        let mut can_mimic_yet = false;
        for mon_idx in 0..num_matchups {
            let cur_player_stage = self.per_matchup_player_modifiers.get(mon_idx).copied().unwrap_or(self.player_stage_modifier);
            let cur_enemy_stage = self.per_matchup_enemy_modifiers.get(mon_idx).copied().unwrap_or(self.enemy_stage_modifier);
            let (player_mon, player_stats): (EnemyPkmn, StatBlock) = if self.is_player_transformed {
                let m = self.transformed_mon_list[mon_idx].clone();
                let s = m.cur_stats;
                (m, s)
            } else {
                let m = self.original_player_mon_list[mon_idx].clone();
                let s = m.get_battle_stats(&cur_player_stage, false, self.player_field_statuses.get(mon_idx));
                (m, s)
            };
            let enemy_mon = self.original_enemy_mon_list[mon_idx].clone();
            let enemy_stats = enemy_mon.get_battle_stats(&cur_enemy_stage, false, self.enemy_field_statuses.get(mon_idx));

            self.player_pkmn_matchup_data.push(PkmnRenderInfo {
                attacking_mon_name: player_mon.name.clone(),
                attacking_mon_level: player_mon.level,
                attacking_mon_speed: player_stats.speed,
                defending_mon_name: enemy_mon.name.clone(),
                defending_mon_level: enemy_mon.level,
                defending_mon_speed: enemy_stats.speed,
                defending_mon_hp: enemy_mon.cur_stats.hp,
            });
            self.enemy_pkmn_matchup_data.push(PkmnRenderInfo {
                attacking_mon_name: enemy_mon.name.clone(),
                attacking_mon_level: enemy_mon.level,
                attacking_mon_speed: enemy_stats.speed,
                defending_mon_name: player_mon.name.clone(),
                defending_mon_level: player_mon.level,
                defending_mon_speed: player_stats.speed,
                defending_mon_hp: player_mon.cur_stats.hp,
            });
            self.player_move_data.push(Vec::new());
            self.enemy_move_data.push(Vec::new());

            let mut struggle_set = false;
            for move_idx in 0..4 {
                let cur_player = if move_idx < player_mon.move_list.len() {
                    // an empty slot is `None` in Python, which later defaults
                    // the display name to the real (Struggle) move name
                    let move_display_name: Option<String> = player_mon.move_list[move_idx].clone();
                    let mut move_name = move_display_name.clone().unwrap_or_default();
                    if move_name == consts::MIMIC_MOVE_NAME {
                        if can_mimic_yet {
                            move_name = self.mimic_selection.clone();
                        } else if !self.mimic_selection.is_empty() && enemy_mon.has_move(&self.mimic_selection) {
                            move_name = self.mimic_selection.clone();
                            can_mimic_yet = true;
                        } else {
                            move_name = "Leer".to_string();
                        }
                    }
                    if move_name.is_empty() && !struggle_set {
                        struggle_set = true;
                        move_name = consts::STRUGGLE_MOVE_NAME.to_string();
                    }
                    self.recalculate_single_move(gen, cfg, mon_idx, true, &move_name, move_display_name.as_deref())
                } else {
                    None
                };
                let cur_enemy = if move_idx < enemy_mon.move_list.len() {
                    let move_name = enemy_mon.move_list[move_idx].clone().unwrap_or_default();
                    if !move_name.is_empty() && !self.mimic_options.contains(&move_name) {
                        self.mimic_options.push(move_name.clone());
                    }
                    self.recalculate_single_move(gen, cfg, mon_idx, false, &move_name, None)
                } else {
                    None
                };
                self.player_move_data[mon_idx].push(cur_player);
                self.enemy_move_data[mon_idx].push(cur_enemy);
            }

            let has_test_moves = cfg.test_moves.iter().take(4).any(|m| !m.trim().is_empty());
            if has_test_moves || cfg.test_moves_enabled {
                for test_idx in 0..4 {
                    let entry = cfg.test_moves.get(test_idx).map(|s| s.trim()).unwrap_or("");
                    if !entry.is_empty() {
                        let data = self.recalculate_single_move(gen, cfg, mon_idx, true, entry, None);
                        self.player_move_data[mon_idx].push(data);
                    } else {
                        self.player_move_data[mon_idx].push(None);
                    }
                }
            }

            self.update_best_move_inplace(cfg, mon_idx, true);
            self.update_best_move_inplace(cfg, mon_idx, false);
        }
        // Python hands every MoveRenderInfo the *same* list object, which keeps
        // growing during the refresh; at rest they all show the final list.
        let final_options = self.mimic_options.clone();
        for list in self.player_move_data.iter_mut().chain(self.enemy_move_data.iter_mut()) {
            for m in list.iter_mut().flatten() {
                m.mimic_options = final_options.clone();
            }
        }
    }

    /// `_update_best_move_inplace`
    pub fn update_best_move_inplace(&mut self, cfg: &SummaryConfig, pkmn_idx: usize, is_player: bool) {
        let strat = if is_player { cfg.player_strategy.as_str() } else { cfg.enemy_strategy.as_str() };
        let mon_data = if is_player {
            self.player_pkmn_matchup_data.get(pkmn_idx).cloned()
        } else {
            self.enemy_pkmn_matchup_data.get(pkmn_idx).cloned()
        };
        let Some(mon_data) = mon_data else { return };
        let moves = if is_player { &mut self.player_move_data } else { &mut self.enemy_move_data };
        let Some(list) = moves.get_mut(pkmn_idx) else { return };
        let mut best: Option<MoveRenderInfo> = None;
        let mut best_idx: Option<usize> = None;
        for (idx, cur) in list.iter().enumerate() {
            let Some(cur) = cur else { continue };
            if cur.name == consts::STRUGGLE_MOVE_NAME {
                continue;
            }
            if is_move_better(cur, best.as_ref(), strat, &mon_data, cfg) {
                best = Some(cur.clone());
                best_idx = Some(idx);
            }
        }
        for (idx, cur) in list.iter_mut().enumerate() {
            if let Some(c) = cur {
                c.is_best_move = Some(idx) == best_idx;
            }
        }
    }

    /// `_get_weather_for_mon_idx`
    pub fn get_weather_for_mon_idx(&self, mon_idx: i64) -> &str {
        let Some(w) = &self.weather else { return consts::WEATHER_NONE };
        match self.weather_source_mon_idx {
            None => w,
            Some(src) if mon_idx < src => consts::WEATHER_NONE,
            Some(_) => w,
        }
    }

    /// `_recalculate_single_move`
    pub fn recalculate_single_move(&self, gen: &GenData, cfg: &SummaryConfig, mon_idx: usize, is_player: bool, move_name: &str, move_display_name: Option<&str>) -> Option<MoveRenderInfo> {
        let current_weather = self.get_weather_for_mon_idx(mon_idx as i64).to_string();
        let default_field = FieldStatus::default();

        // attacker / defender selection
        let (mut attacking_mon, attacking_mon_stats, mut crit_mon, crit_mon_stats): (EnemyPkmn, Option<StatBlock>, EnemyPkmn, Option<StatBlock>);
        let (attacking_stages, defending_stages): (StageModifiers, StageModifiers);
        let (attacking_field, defending_field): (FieldStatus, FieldStatus);
        let mut defending_mon: EnemyPkmn;
        let defending_mon_stats: Option<StatBlock>;
        if is_player {
            if self.is_player_transformed {
                let am = self.transformed_mon_list[mon_idx].clone();
                let mut am_stats = am.cur_stats;
                let mut am_mut = am.clone();
                if gen.get_generation() == 1 {
                    if am.level > self.transformed_mon_list[0].level {
                        am_mut.badges = self.original_player_mon_list[0].badges.clone();
                        let stage = self.per_matchup_player_modifiers.get(mon_idx).copied().unwrap_or(self.player_stage_modifier);
                        am_stats = am_mut.get_battle_stats(&stage, false, None);
                    }
                    let orig = &self.original_player_mon_list[mon_idx];
                    let mut cm = am_mut.clone();
                    cm.level = orig.level;
                    cm.base_stats = orig.base_stats;
                    cm.stat_xp = orig.stat_xp;
                    cm.dvs = orig.dvs;
                    cm.badges = None;
                    attacking_mon = am_mut;
                    attacking_mon_stats = Some(am_stats);
                    crit_mon = cm;
                    crit_mon_stats = None;
                } else if gen.get_generation() == 2 {
                    let mut a_stats = Some(am_stats);
                    let mut c_stats = Some(am_stats);
                    if am.level > self.transformed_mon_list[0].level {
                        let stage = self.per_matchup_player_modifiers.get(mon_idx).copied().unwrap_or(self.player_stage_modifier);
                        a_stats = Some(self.original_player_mon_list[mon_idx].get_battle_stats(&stage, false, None));
                        c_stats = Some(self.original_player_mon_list[mon_idx].get_battle_stats(&stage, true, None));
                    }
                    attacking_mon = am.clone();
                    attacking_mon_stats = a_stats;
                    crit_mon = am;
                    crit_mon_stats = c_stats;
                } else {
                    attacking_mon = am.clone();
                    attacking_mon_stats = Some(am_stats);
                    crit_mon = am;
                    crit_mon_stats = Some(am_stats);
                }
            } else {
                let am = self.original_player_mon_list[mon_idx].clone();
                attacking_mon = am.clone();
                attacking_mon_stats = None;
                crit_mon = am;
                crit_mon_stats = None;
            }
            attacking_stages = self.per_matchup_player_modifiers.get(mon_idx).copied().unwrap_or(self.player_stage_modifier);
            attacking_field = self.player_field_statuses.get(mon_idx).copied().unwrap_or(default_field);
            defending_mon = self.original_enemy_mon_list[mon_idx].clone();
            defending_mon_stats = None;
            defending_stages = self.per_matchup_enemy_modifiers.get(mon_idx).copied().unwrap_or(self.enemy_stage_modifier);
            defending_field = self.enemy_field_statuses.get(mon_idx).copied().unwrap_or(default_field);
        } else {
            let am = self.original_enemy_mon_list[mon_idx].clone();
            attacking_mon = am.clone();
            attacking_mon_stats = None;
            crit_mon = am;
            crit_mon_stats = None;
            attacking_stages = self.per_matchup_enemy_modifiers.get(mon_idx).copied().unwrap_or(self.enemy_stage_modifier);
            attacking_field = self.enemy_field_statuses.get(mon_idx).copied().unwrap_or(default_field);
            if self.is_player_transformed {
                defending_mon = self.transformed_mon_list[mon_idx].clone();
                defending_mon_stats = Some(defending_mon.cur_stats);
            } else {
                defending_mon = self.original_player_mon_list[mon_idx].clone();
                defending_mon_stats = None;
            }
            defending_stages = self.per_matchup_player_modifiers.get(mon_idx).copied().unwrap_or(self.player_stage_modifier);
            defending_field = self.player_field_statuses.get(mon_idx).copied().unwrap_or(default_field);
        }

        if move_name.is_empty() {
            return None;
        }
        let mv = gen.move_db().get_move(move_name)?.clone();
        let mut display_name: Option<String> = move_display_name.map(|s| s.to_string());
        if mv.name == consts::HIDDEN_POWER_MOVE_NAME {
            let (t, p) = gen.get_hidden_power(&attacking_mon.dvs);
            display_name = Some(format!("{} ({}: {})", mv.name, t, p));
        } else if mv.name == consts::NATURAL_GIFT_MOVE_NAME {
            if let Some((t, p)) = gen.get_natural_gift(attacking_mon.held_item.as_deref()) {
                display_name = Some(format!("{} ({}: {})", mv.name, t, p));
            }
        } else if mv.name == consts::WEATHER_BALL_MOVE_NAME && mv.base_power_or_zero() != 0 {
            let active = damage::is_weather_active(&attacking_mon.ability, &defending_mon.ability, &current_weather);
            if active {
                let t = damage::get_weather_ball_type(&current_weather, active, &mv.move_type);
                display_name = Some(format!("{} ({}: {})", mv.name, t, mv.base_power_or_zero() * 2));
            }
        }
        let display_name = display_name.unwrap_or_else(|| mv.name.clone());

        let custom_lookup = self.custom_move_data.get(mon_idx).map(|c| c.side(is_player));
        let mut custom_data_selection: Option<String> = custom_lookup.and_then(|m| m.get(move_name).cloned());
        let mut custom_data_options: Option<Vec<String>> = gen.get_move_custom_data(&mv.name).cloned();
        if custom_data_options.is_none() && mv.has_flavor(consts::FLAVOR_MULTI_HIT) {
            custom_data_options = Some(consts::MULTI_HIT_CUSTOM_DATA.iter().map(|s| s.to_string()).collect());
        }
        match &custom_data_options {
            None => custom_data_selection = None,
            Some(opts) => {
                if !custom_data_selection.as_ref().map(|s| opts.contains(s)).unwrap_or(false) {
                    custom_data_selection = opts.first().cloned();
                }
            }
        }
        let custom_str = custom_data_selection.clone().unwrap_or_default();

        let (normal_ranges, crit_ranges): (Option<DamageRange>, Option<DamageRange>);
        if self.is_wild_battle && mon_idx < self.wild_min_dv_mons.len() {
            let wild_min = &self.wild_min_dv_mons[mon_idx];
            let wild_max = &self.wild_max_dv_mons[mon_idx];
            let common = CommonArgs {
                gen,
                mv: &mv,
                attacking_stages: &attacking_stages,
                defending_stages: &defending_stages,
                attacking_field: &attacking_field,
                defending_field: &defending_field,
                custom: &custom_str,
                weather: &current_weather,
                is_double_battle: self.double_battle_flag,
            };
            if is_player {
                let tanky = common.calc(&attacking_mon, wild_max, false, attacking_mon_stats.as_ref(), None);
                let squishy = common.calc(&attacking_mon, wild_min, false, attacking_mon_stats.as_ref(), None);
                let crit_tanky = common.calc(&crit_mon, wild_max, true, crit_mon_stats.as_ref(), None);
                let crit_squishy = common.calc(&crit_mon, wild_min, true, crit_mon_stats.as_ref(), None);
                normal_ranges = DamageRange::merge_min_max(tanky.as_ref(), squishy.as_ref());
                crit_ranges = DamageRange::merge_min_max(crit_tanky.as_ref(), crit_squishy.as_ref());
                defending_mon = wild_min.clone();
            } else {
                let weak = common.calc(wild_min, &defending_mon, false, None, defending_mon_stats.as_ref());
                let strong = common.calc(wild_max, &defending_mon, false, None, defending_mon_stats.as_ref());
                let crit_weak = common.calc(wild_min, &defending_mon, true, None, defending_mon_stats.as_ref());
                let crit_strong = common.calc(wild_max, &defending_mon, true, None, defending_mon_stats.as_ref());
                normal_ranges = DamageRange::merge_min_max(weak.as_ref(), strong.as_ref());
                crit_ranges = DamageRange::merge_min_max(crit_weak.as_ref(), crit_strong.as_ref());
                attacking_mon = wild_max.clone();
                crit_mon = attacking_mon.clone();
            }
        } else {
            let mut args = DamageArgs {
                attacking: &attacking_mon,
                mv: &mv,
                defending: &defending_mon,
                attacking_stages: Some(&attacking_stages),
                defending_stages: Some(&defending_stages),
                attacking_field: Some(&attacking_field),
                defending_field: Some(&defending_field),
                is_crit: false,
                custom_move_data: &custom_str,
                weather: &current_weather,
                is_double_battle: self.double_battle_flag,
                attacking_battle_stats: attacking_mon_stats.as_ref(),
                defending_battle_stats: defending_mon_stats.as_ref(),
                // Python's summary never passes attacker_is_enemy (always False)
                attacker_is_enemy: false,
            };
            normal_ranges = calculate_damage(gen, &args);
            args.attacking = &crit_mon;
            args.is_crit = true;
            args.attacking_battle_stats = crit_mon_stats.as_ref();
            crit_ranges = calculate_damage(gen, &args);
        }
        let _ = &crit_mon;

        let kill_ranges: Vec<KillRange> = match (&normal_ranges, &crit_ranges) {
            (Some(n), Some(c)) => {
                let accuracy = if cfg.calc.ignore_accuracy {
                    100.0
                } else {
                    get_move_accuracy(gen, &attacking_mon, &mv, custom_data_selection.as_deref(), &defending_mon, &current_weather).unwrap_or(100.0)
                };
                let accuracy = accuracy / 100.0;
                find_kill(
                    n,
                    c,
                    get_crit_rate(gen, &attacking_mon, &mv, custom_data_selection.as_deref()),
                    accuracy,
                    defending_mon.cur_stats.hp,
                    cfg.calc.damage_search_depth,
                    0.1,
                    cfg.calc.force_full_search,
                )
            }
            _ => Vec::new(),
        };

        let is_mimic_placeholder = move_display_name == Some(consts::MIMIC_MOVE_NAME) && mv.name != self.mimic_selection;
        let (stat_stage_options, stat_stage_info, stat_stage_selection) = if self.using_global_setup || is_mimic_placeholder {
            (None, StatStageInfo::default(), "0".to_string())
        } else {
            let opts = gen.move_db().get_stat_stage_dropdown_options(&mv.name, gen.gen);
            let info = gen.move_db().get_stat_stage_info(&mv.name);
            let sel = if opts.is_some() {
                self.get_stat_stage_selection(mon_idx, is_player, &mv.name)
            } else {
                "0".to_string()
            };
            (opts, info, sel)
        };

        Some(MoveRenderInfo {
            name: display_name,
            attack_flavor: mv.attack_flavor.clone(),
            min_damage: normal_ranges.as_ref().map(|r| r.min_damage).unwrap_or(-1),
            max_damage: normal_ranges.as_ref().map(|r| r.max_damage).unwrap_or(-1),
            crit_min_damage: crit_ranges.as_ref().map(|r| r.min_damage).unwrap_or(-1),
            crit_max_damage: crit_ranges.as_ref().map(|r| r.max_damage).unwrap_or(-1),
            defending_mon_hp: defending_mon.cur_stats.hp,
            kill_ranges,
            mimic_data: self.mimic_selection.clone(),
            mimic_options: self.mimic_options.clone(),
            custom_data_options,
            custom_data_selection,
            attacking_mon_hp: attacking_mon.cur_stats.hp,
            is_best_move: false,
            stat_stage_options,
            stat_stage_selection,
            stat_stage_info,
        })
    }

    // ---- state changes --------------------------------------------------------

    /// `update_mimic_selection`
    pub fn update_mimic_selection(&mut self, gen: &GenData, cfg: &SummaryConfig, new_value: &str) {
        self.mimic_selection = new_value.to_string();
        let mut target_found = false;
        for mon_idx in 0..self.original_enemy_mon_list.len() {
            if !target_found && self.original_enemy_mon_list[mon_idx].has_move(new_value) {
                target_found = true;
            }
            let move_name = if target_found { self.mimic_selection.clone() } else { "Leer".to_string() };
            if let Some(mimic_idx) = self.original_player_mon_list.get(mon_idx).and_then(|m| m.move_index(consts::MIMIC_MOVE_NAME)) {
                let data = self.recalculate_single_move(gen, cfg, mon_idx, true, &move_name, Some(consts::MIMIC_MOVE_NAME));
                if let Some(list) = self.player_move_data.get_mut(mon_idx) {
                    if mimic_idx < list.len() {
                        list[mimic_idx] = data;
                    }
                }
                self.update_best_move_inplace(cfg, mon_idx, true);
            }
        }
    }

    /// `update_custom_move_data`
    pub fn update_custom_move_data(&mut self, gen: &GenData, cfg: &SummaryConfig, pkmn_idx: usize, move_idx: usize, is_player: bool, new_value: &str) {
        let move_name = {
            let data = if is_player { &self.player_move_data } else { &self.enemy_move_data };
            match data.get(pkmn_idx).and_then(|l| l.get(move_idx)).and_then(|m| m.as_ref()) {
                Some(m) => m.name.clone(),
                None => return,
            }
        };
        if pkmn_idx >= self.custom_move_data.len() {
            return;
        }
        self.custom_move_data[pkmn_idx].side_mut(is_player).insert(move_name.clone(), new_value.to_string());
        let recalculated = self.recalculate_single_move(gen, cfg, pkmn_idx, is_player, &move_name, None);
        let data = if is_player { &mut self.player_move_data } else { &mut self.enemy_move_data };
        data[pkmn_idx][move_idx] = recalculated;
        self.update_best_move_inplace(cfg, pkmn_idx, is_player);
    }

    pub fn update_weather(&mut self, gen: &GenData, cfg: &SummaryConfig, new_weather: &str, source_mon_idx: Option<i64>) {
        self.weather = Some(new_weather.to_string());
        self.weather_source_mon_idx = source_mon_idx;
        self.full_refresh(Some(gen), cfg);
    }

    pub fn update_enemy_setup_moves(&mut self, gen: &GenData, cfg: &SummaryConfig, moves: Vec<String>) {
        self.enemy_setup_move_list = moves;
        self.full_refresh(Some(gen), cfg);
    }

    pub fn update_player_setup_moves(&mut self, gen: &GenData, cfg: &SummaryConfig, moves: Vec<String>) {
        self.player_setup_move_list = moves;
        self.full_refresh(Some(gen), cfg);
    }

    pub fn update_enemy_field_moves(&mut self, gen: &GenData, cfg: &SummaryConfig, moves: Vec<String>) {
        self.enemy_field_move_list = moves;
        self.full_refresh(Some(gen), cfg);
    }

    pub fn update_player_field_moves(&mut self, gen: &GenData, cfg: &SummaryConfig, moves: Vec<String>) {
        self.player_field_move_list = moves;
        self.full_refresh(Some(gen), cfg);
    }

    pub fn update_player_transform(&mut self, gen: &GenData, cfg: &SummaryConfig, is_transformed: bool) {
        self.is_player_transformed = is_transformed;
        self.full_refresh(Some(gen), cfg);
    }

    /// `_save_transient_state`
    pub fn save_transient_state(&self) -> TransientState {
        TransientState {
            player_setup: self.player_setup_move_list.clone(),
            player_field: self.player_field_move_list.clone(),
            enemy_setup: self.enemy_setup_move_list.clone(),
            enemy_field: self.enemy_field_move_list.clone(),
            stat_stage: self.stat_stage_setup.clone(),
            custom_data: self.custom_move_data.clone(),
            weather: self.weather.clone(),
            weather_source_mon_idx: self.weather_source_mon_idx,
            player_screens: self.player_screens.clone(),
            enemy_screens: self.enemy_screens.clone(),
            player_intimidate: self.player_intimidate.clone(),
            enemy_intimidate: self.enemy_intimidate.clone(),
            mimic: self.mimic_selection.clone(),
            transformed: self.is_player_transformed,
            double: self.double_battle_flag,
        }
    }

    /// `_restore_transient_state`
    pub fn restore_transient_state(&mut self, saved: TransientState) {
        self.player_setup_move_list = saved.player_setup;
        self.player_field_move_list = saved.player_field;
        self.enemy_setup_move_list = saved.enemy_setup;
        self.enemy_field_move_list = saved.enemy_field;
        self.stat_stage_setup = saved.stat_stage;
        self.custom_move_data = saved.custom_data;
        self.weather = saved.weather;
        self.weather_source_mon_idx = saved.weather_source_mon_idx;
        self.player_screens = saved.player_screens;
        self.enemy_screens = saved.enemy_screens;
        self.player_intimidate = saved.player_intimidate;
        self.enemy_intimidate = saved.enemy_intimidate;
        self.mimic_selection = saved.mimic;
        self.is_player_transformed = saved.transformed;
        self.double_battle_flag = saved.double;
    }

    // ---- queries ------------------------------------------------------------------

    pub fn get_player_battle_hp(&self) -> i64 {
        self.original_player_mon_list.first().map(|m| m.cur_stats.hp).unwrap_or(0)
    }

    pub fn get_player_battle_speed(&self) -> i64 {
        self.original_player_mon_list.first().map(|m| m.cur_stats.speed).unwrap_or(0)
    }

    pub fn get_player_held_item(&self) -> String {
        self.original_player_mon_list
            .first()
            .and_then(|m| m.held_item.clone())
            .unwrap_or_default()
    }

    /// `get_held_item_options`: `[""] + sorted(all item names)`
    pub fn get_held_item_options(gen: &GenData) -> Vec<String> {
        let mut items = gen.item_db().get_filtered_names(consts::ITEM_TYPE_ALL_ITEMS, consts::ITEM_TYPE_ALL_ITEMS, None);
        items.sort();
        let mut result = vec![String::new()];
        result.extend(items);
        result
    }

    /// `get_partial_trainer_definition`
    pub fn get_partial_trainer_definition(&self) -> Option<TrainerEventDefinition> {
        if self.trainer_name.is_empty() {
            return None;
        }
        let custom_present = self.custom_move_data.iter().any(|c| !c.player.is_empty() || !c.enemy.is_empty());
        let final_custom: Vec<CustomMoveData> = if custom_present {
            // (Python raises IndexError for an out-of-range index; we substitute an empty entry)
            self.cached_definition_order
                .iter()
                .map(|x| py_get(&self.custom_move_data, *x).cloned().unwrap_or_default())
                .collect()
        } else {
            Vec::new()
        };

        let mut stat_stage_present = false;
        for cur in &self.stat_stage_setup {
            if !cur.player.is_empty() || !cur.enemy.is_empty() {
                for side in [&cur.player, &cur.enemy] {
                    for (_, stage) in side {
                        if stage != "0" {
                            stat_stage_present = true;
                            break;
                        }
                    }
                    if stat_stage_present {
                        break;
                    }
                }
                if stat_stage_present {
                    break;
                }
            }
        }
        let final_stat_stage: Vec<CustomMoveData> = if stat_stage_present {
            let mut def_to_display: Vec<(i64, usize)> = self
                .cached_definition_order
                .iter()
                .enumerate()
                .map(|(display_idx, def_idx)| (*def_idx, display_idx))
                .collect();
            // dict semantics: a repeated def index keeps the last display index
            let mut dedup: IndexMap<i64, usize> = IndexMap::new();
            for (d, disp) in def_to_display.drain(..) {
                dedup.insert(d, disp);
            }
            let mut keys: Vec<i64> = dedup.keys().copied().collect();
            keys.sort();
            keys.iter()
                .map(|def_idx| {
                    let display_idx = dedup[def_idx];
                    self.stat_stage_setup.get(display_idx).cloned().unwrap_or_default()
                })
                .collect()
        } else {
            Vec::new()
        };

        let mut collapsed: Vec<i64> = self
            .collapsed_mons
            .iter()
            .filter(|d| **d >= 0 && (**d as usize) < self.cached_definition_order.len())
            .map(|d| self.cached_definition_order[*d as usize])
            .collect();
        collapsed.sort();
        collapsed.dedup();

        let mut td = TrainerEventDefinition::new(&self.trainer_name);
        td.second_trainer_name = self.second_trainer_name_raw.clone();
        td.setup_moves = self.player_setup_move_list.clone();
        td.player_field_moves = self.player_field_move_list.clone();
        td.enemy_setup_moves = self.enemy_setup_move_list.clone();
        td.enemy_field_moves = self.enemy_field_move_list.clone();
        td.mimic_selection = self.mimic_selection.clone();
        td.custom_move_data = final_custom;
        td.weather = self.weather.clone();
        td.weather_source_mon_idx = self.weather_source_mon_idx;
        td.player_screens = self.player_screens.clone();
        td.enemy_screens = self.enemy_screens.clone();
        td.player_intimidate = self.intimidate_to_def_order(self.player_intimidate.as_ref());
        td.enemy_intimidate = self.intimidate_to_def_order(self.enemy_intimidate.as_ref());
        td.set_transformed(self.is_player_transformed);
        td.stat_stage_setup = final_stat_stage;
        td.collapsed_mons = collapsed;
        Some(td)
    }

    pub fn get_pkmn_info(&self, pkmn_idx: usize, is_player: bool) -> Option<&PkmnRenderInfo> {
        if is_player {
            self.player_pkmn_matchup_data.get(pkmn_idx)
        } else {
            self.enemy_pkmn_matchup_data.get(pkmn_idx)
        }
    }

    pub fn num_matchups(&self) -> usize {
        self.player_pkmn_matchup_data.len()
    }

    /// `get_move_info`
    pub fn get_move_info(&self, cfg: &SummaryConfig, pkmn_idx: usize, move_idx: usize, is_player: bool) -> Option<&MoveRenderInfo> {
        let data = if is_player { &self.player_move_data } else { &self.enemy_move_data };
        let max_idx = if is_player {
            let has_test = cfg.test_moves.iter().take(4).any(|m| !m.trim().is_empty());
            if has_test || cfg.test_moves_enabled {
                7
            } else {
                3
            }
        } else {
            3
        };
        if move_idx > max_idx {
            return None;
        }
        data.get(pkmn_idx)?.get(move_idx)?.as_ref()
    }

    pub fn get_weather(&self) -> &str {
        self.weather.as_deref().unwrap_or(consts::WEATHER_NONE)
    }

    /// `get_weather_for_move`
    pub fn get_weather_for_move(gen: &GenData, move_name: &str) -> Option<&'static str> {
        if move_name.is_empty() {
            return None;
        }
        let weather = consts::weather_for_move(move_name)?;
        if !gen.get_valid_weather().contains(&weather) {
            return None;
        }
        Some(weather)
    }

    /// `toggle_weather_from_move`
    pub fn toggle_weather_from_move(&mut self, gen: &GenData, cfg: &SummaryConfig, move_name: &str, enabled: bool, mon_idx: Option<i64>) {
        let Some(weather) = BattleSummary::get_weather_for_move(gen, move_name) else { return };
        if enabled {
            self.update_weather(gen, cfg, weather, mon_idx);
        } else if self.weather.as_deref() == Some(weather) {
            self.update_weather(gen, cfg, consts::WEATHER_NONE, None);
        }
    }

    pub fn get_screen_for_move(move_name: &str) -> Option<&'static str> {
        if move_name.is_empty() {
            return None;
        }
        consts::screen_for_move(move_name)
    }

    pub fn get_screen_source_mon_idx(&self, is_player: bool, screen_id: &str) -> Option<i64> {
        let screens = if is_player { &self.player_screens } else { &self.enemy_screens };
        screens.get(screen_id).copied()
    }

    fn intimidate_from_def_order(&self, raw: Option<&[i64]>) -> Option<BTreeSet<i64>> {
        let raw = raw?;
        let mut result = BTreeSet::new();
        for def_idx in raw {
            if let Some(display_idx) = self.cached_definition_order.iter().rposition(|d| d == def_idx) {
                result.insert(display_idx as i64);
            }
        }
        Some(result)
    }

    fn intimidate_to_def_order(&self, display_set: Option<&BTreeSet<i64>>) -> Option<Vec<i64>> {
        let set = display_set?;
        let mut result: Vec<i64> = set
            .iter()
            .filter(|d| **d >= 0 && (**d as usize) < self.cached_definition_order.len())
            .map(|d| self.cached_definition_order[*d as usize])
            .collect();
        result.sort();
        Some(result)
    }

    fn player_has_intimidate(&self, mon_idx: i64) -> bool {
        mon_idx >= 0
            && self
                .original_player_mon_list
                .get(mon_idx as usize)
                .map(|m| m.ability == consts::INTIMIDATE_ABILITY)
                .unwrap_or(false)
    }

    fn enemy_has_intimidate(&self, mon_idx: i64) -> bool {
        mon_idx >= 0
            && self
                .original_enemy_mon_list
                .get(mon_idx as usize)
                .map(|m| m.ability == consts::INTIMIDATE_ABILITY)
                .unwrap_or(false)
    }

    pub fn pokemon_has_intimidate(&self, mon_idx: i64, is_player: bool) -> bool {
        if is_player {
            self.player_has_intimidate(mon_idx)
        } else {
            self.enemy_has_intimidate(mon_idx)
        }
    }

    pub fn is_intimidate_blocked(&self, mon_idx: i64, is_player: bool) -> bool {
        let opposing = if is_player { &self.original_enemy_mon_list } else { &self.original_player_mon_list };
        if mon_idx < 0 {
            return false;
        }
        opposing
            .get(mon_idx as usize)
            .map(|m| consts::INTIMIDATE_BLOCKING_ABILITIES.contains(&m.ability.as_str()))
            .unwrap_or(false)
    }

    fn resolve_intimidate(&self, is_player: bool) -> BTreeSet<i64> {
        let stored = if is_player { &self.player_intimidate } else { &self.enemy_intimidate };
        if let Some(s) = stored {
            return s.iter().copied().filter(|idx| !self.is_intimidate_blocked(*idx, is_player)).collect();
        }
        let mut result = BTreeSet::new();
        if is_player {
            if !self.original_player_mon_list.is_empty() && self.player_has_intimidate(0) && !self.is_intimidate_blocked(0, true) {
                result.insert(0);
            }
        } else {
            for idx in 0..self.original_enemy_mon_list.len() as i64 {
                if self.enemy_has_intimidate(idx) && !self.is_intimidate_blocked(idx, false) {
                    result.insert(idx);
                }
            }
        }
        result
    }

    pub fn is_intimidate_active(&self, mon_idx: i64, is_player: bool) -> bool {
        self.resolve_intimidate(is_player).contains(&mon_idx)
    }

    pub fn toggle_intimidate(&mut self, gen: &GenData, cfg: &SummaryConfig, mon_idx: i64, is_player: bool, enabled: bool) {
        let mut new_set = self.resolve_intimidate(is_player);
        if enabled {
            new_set.insert(mon_idx);
        } else {
            new_set.remove(&mon_idx);
        }
        if is_player {
            self.player_intimidate = Some(new_set);
        } else {
            self.enemy_intimidate = Some(new_set);
        }
        self.full_refresh(Some(gen), cfg);
    }

    pub fn toggle_screen_from_move(&mut self, gen: &GenData, cfg: &SummaryConfig, move_name: &str, enabled: bool, mon_idx: i64, is_player: bool) {
        let Some(screen_id) = BattleSummary::get_screen_for_move(move_name) else { return };
        let screens = if is_player { &mut self.player_screens } else { &mut self.enemy_screens };
        if enabled {
            screens.insert(screen_id.to_string(), mon_idx);
        } else if screens.get(screen_id) == Some(&mon_idx) {
            screens.shift_remove(screen_id);
        }
        self.full_refresh(Some(gen), cfg);
    }

    pub fn is_double_battle(&self) -> bool {
        self.double_battle_flag
    }

    pub fn is_mon_collapsed(&self, mon_idx: i64) -> bool {
        self.collapsed_mons.contains(&mon_idx)
    }

    /// `update_mon_collapsed`; returns true when the state changed.
    pub fn update_mon_collapsed(&mut self, mon_idx: i64, collapsed: bool) -> bool {
        let already = self.collapsed_mons.contains(&mon_idx);
        if collapsed == already {
            return false;
        }
        if collapsed {
            self.collapsed_mons.insert(mon_idx);
        } else {
            self.collapsed_mons.remove(&mon_idx);
        }
        true
    }

    /// `reorder_matchup`: computes the new event definition; the caller applies it.
    pub fn reorder_matchup(&self, router: &Router, from_disp_idx: usize, to_disp_idx: usize) -> Result<Option<EventDefinition>, String> {
        if from_disp_idx == to_disp_idx {
            return Ok(None);
        }
        let Some(gid) = self.event_group_id else { return Ok(None) };
        let Some(group) = router.group(gid) else { return Ok(None) };
        let Some(orig_td) = &group.event_definition.trainer_def else { return Ok(None) };
        let gen = router.gen().ok_or("no gen")?;
        let num_mons = group.event_definition.get_pokemon_list(gen, true)?.len();
        if num_mons < 2 || from_disp_idx >= num_mons || to_disp_idx >= num_mons {
            return Ok(None);
        }
        let cur_mon_order: Vec<i64> = if !orig_td.mon_order.is_empty() && orig_td.mon_order.len() == num_mons {
            orig_td.mon_order.clone()
        } else {
            (1..=num_mons as i64).collect()
        };
        let mut display_to_def: Vec<Option<usize>> = vec![None; num_mons];
        for (def_idx, disp_pos) in cur_mon_order.iter().enumerate() {
            let slot = disp_pos - 1;
            if slot >= 0 && (slot as usize) < num_mons && display_to_def[slot as usize].is_none() {
                display_to_def[slot as usize] = Some(def_idx);
            }
        }
        let used: BTreeSet<usize> = display_to_def.iter().flatten().copied().collect();
        let missing_slots: Vec<usize> = display_to_def.iter().enumerate().filter(|(_, x)| x.is_none()).map(|(i, _)| i).collect();
        let missing_defs: Vec<usize> = (0..num_mons).filter(|i| !used.contains(i)).collect();
        for (slot, def_idx) in missing_slots.iter().zip(missing_defs.iter()) {
            display_to_def[*slot] = Some(*def_idx);
        }
        let mut order: Vec<usize> = display_to_def.into_iter().map(|x| x.unwrap_or(0)).collect();
        let moved = order.remove(from_disp_idx);
        order.insert(to_disp_idx, moved);
        let mut new_mon_order = vec![0i64; num_mons];
        for (new_disp_idx, def_idx) in order.iter().enumerate() {
            new_mon_order[*def_idx] = new_disp_idx as i64 + 1;
        }
        let mut new_td = match self.get_partial_trainer_definition() {
            Some(t) => t,
            None => return Ok(None),
        };
        new_td.exp_split = orig_td.exp_split.clone();
        new_td.pay_day_amount = orig_td.pay_day_amount;
        new_td.mon_order = new_mon_order;
        let mut new_event = EventDefinition::with_trainer(new_td);
        new_event.notes = group.event_definition.notes.clone();
        Ok(Some(new_event))
    }

    /// `_calc_field_status`
    pub fn calc_field_status(&self, is_player: bool, mon_idx: i64) -> FieldStatus {
        let mut result = FieldStatus::default();
        let (field, setup, screens) = if is_player {
            (&self.player_field_move_list, &self.player_setup_move_list, &self.player_screens)
        } else {
            (&self.enemy_field_move_list, &self.enemy_setup_move_list, &self.enemy_screens)
        };
        for m in field.iter().chain(setup.iter()) {
            result = result.apply_move(m);
        }
        for (toggle_id, source_idx) in screens {
            if mon_idx >= *source_idx {
                result.set_by_name(toggle_id);
            }
        }
        result
    }

    /// `_calc_per_matchup_stage_modifiers`
    pub fn calc_per_matchup_stage_modifiers(&self, gen: &GenData) -> (Vec<StageModifiers>, Vec<StageModifiers>) {
        let num_matchups = self.original_player_mon_list.len();
        let mut player_modifiers = Vec::new();
        let mut enemy_modifiers = Vec::new();
        let mut persistent_player = StageModifiers::default();
        let player_intimidate_active = self.resolve_intimidate(true);
        let enemy_intimidate_active = self.resolve_intimidate(false);
        let intimidate_drop = vec![(consts::ATK.to_string(), -1i64)];
        let move_db = gen.move_db();
        let is_gen1 = gen.gen == Gen::One;
        let mut prev_player_level: Option<i64> = None;

        for mon_idx in 0..num_matchups {
            if is_gen1 {
                let cur_level = self.original_player_mon_list[mon_idx].level;
                if let Some(prev) = prev_player_level {
                    if cur_level > prev {
                        persistent_player = persistent_player.clear_badge_boosts();
                    }
                }
                prev_player_level = Some(cur_level);
            }
            let mut cur_player = persistent_player;
            let mut cur_enemy = StageModifiers::default();

            if player_intimidate_active.contains(&(mon_idx as i64)) {
                cur_enemy = cur_enemy.apply_stat_mod(&intimidate_drop);
            }
            if enemy_intimidate_active.contains(&(mon_idx as i64)) {
                persistent_player = persistent_player.apply_stat_mod(&intimidate_drop);
                cur_player = cur_player.apply_stat_mod(&intimidate_drop);
            }

            let empty = CustomMoveData::default();
            let matchup_setup = self.stat_stage_setup.get(mon_idx).unwrap_or(&empty);

            // Step 1: the player's moves
            if let Some(player_mon) = self.original_player_mon_list.get(mon_idx) {
                for move_name in player_mon.move_list.iter().flatten() {
                    if move_name.is_empty() {
                        continue;
                    }
                    let count = matchup_setup
                        .player
                        .get(move_name)
                        .and_then(|s| damage::py_int(s))
                        .unwrap_or(0);
                    if count <= 0 {
                        continue;
                    }
                    let info = move_db.get_stat_stage_info(move_name);
                    if !info.has_stat_effect {
                        continue;
                    }
                    if !info.is_guaranteed && !info.is_damaging {
                        continue;
                    }
                    if info.targets_self && !info.is_damaging {
                        if info.is_belly_drum {
                            persistent_player = persistent_player.set_attack_stage(count);
                            cur_player = cur_player.set_attack_stage(count);
                        } else {
                            let mods = move_db.get_stat_mod_for_target(move_name, true);
                            for _ in 0..count {
                                persistent_player = persistent_player.apply_stat_mod(&mods);
                                cur_player = cur_player.apply_stat_mod(&mods);
                            }
                        }
                    } else if !info.targets_self && !info.is_damaging {
                        let mods = move_db.get_stat_mod_for_target(move_name, false);
                        for _ in 0..count {
                            cur_enemy = cur_enemy.apply_stat_mod(&mods);
                        }
                    } else if info.is_damaging && info.targets_self {
                        let mods = move_db.get_stat_mod_for_target(move_name, true);
                        for _ in 0..count {
                            persistent_player = persistent_player.apply_stat_mod(&mods);
                            cur_player = cur_player.apply_stat_mod(&mods);
                        }
                    } else if info.is_damaging {
                        let mods = move_db.get_stat_mod_for_target(move_name, false);
                        for _ in 0..count {
                            cur_enemy = cur_enemy.apply_stat_mod(&mods);
                        }
                    }
                }
            }

            // Step 2: the enemy's moves
            if let Some(enemy_mon) = self.original_enemy_mon_list.get(mon_idx) {
                for move_name in enemy_mon.move_list.iter().flatten() {
                    if move_name.is_empty() {
                        continue;
                    }
                    let count = matchup_setup
                        .enemy
                        .get(move_name)
                        .and_then(|s| damage::py_int(s))
                        .unwrap_or(0);
                    if count <= 0 {
                        continue;
                    }
                    let info = move_db.get_stat_stage_info(move_name);
                    if !info.has_stat_effect {
                        continue;
                    }
                    if !info.is_guaranteed && !info.is_damaging {
                        continue;
                    }
                    if info.targets_self && !info.is_damaging {
                        if info.is_belly_drum {
                            cur_enemy = cur_enemy.set_attack_stage(count);
                        } else {
                            let mods = move_db.get_stat_mod_for_target(move_name, true);
                            for _ in 0..count {
                                cur_enemy = cur_enemy.apply_stat_mod(&mods);
                            }
                        }
                    } else if !info.targets_self && !info.is_damaging {
                        let mods = move_db.get_stat_mod_for_target(move_name, false);
                        for _ in 0..count {
                            persistent_player = persistent_player.apply_stat_mod(&mods);
                            cur_player = cur_player.apply_stat_mod(&mods);
                        }
                    } else if info.is_damaging && info.targets_self {
                        let mods = move_db.get_stat_mod_for_target(move_name, true);
                        for _ in 0..count {
                            cur_enemy = cur_enemy.apply_stat_mod(&mods);
                        }
                    } else if info.is_damaging {
                        let mods = move_db.get_stat_mod_for_target(move_name, false);
                        for _ in 0..count {
                            persistent_player = persistent_player.apply_stat_mod(&mods);
                            cur_player = cur_player.apply_stat_mod(&mods);
                        }
                    }
                }
            }
            player_modifiers.push(cur_player);
            enemy_modifiers.push(cur_enemy);
        }
        (player_modifiers, enemy_modifiers)
    }

    pub fn can_support_prefight_candies(&self) -> bool {
        self.event_group_id.is_some()
    }

    // ---- move highlights ---------------------------------------------------------

    pub fn get_move_highlight_state(&self, mon_idx: i64, move_idx: i64, is_player: bool) -> i64 {
        self.move_highlights
            .get(&self.event_group_id)
            .and_then(|m| m.get(&(mon_idx, move_idx, is_player)))
            .copied()
            .unwrap_or(0)
    }

    pub fn set_move_highlight_state(&mut self, mon_idx: i64, move_idx: i64, is_player: bool, state: i64) {
        self.move_highlights
            .entry(self.event_group_id)
            .or_default()
            .insert((mon_idx, move_idx, is_player), state);
    }

    /// `update_move_highlight`: 0 -> 1 -> 2 -> 3 -> 0
    pub fn update_move_highlight(&mut self, mon_idx: i64, move_idx: i64, is_player: bool, reset: bool) {
        let cur = self.get_move_highlight_state(mon_idx, move_idx, is_player);
        let new_state = if reset { 0 } else { (cur + 1) % 4 };
        self.set_move_highlight_state(mon_idx, move_idx, is_player, new_state);
    }

    // ---- stat stages -------------------------------------------------------------------

    /// `_get_stat_stage_selection`
    pub fn get_stat_stage_selection(&self, pkmn_idx: usize, is_player: bool, move_name: &str) -> String {
        self.stat_stage_setup
            .get(pkmn_idx)
            .and_then(|s| s.side(is_player).get(move_name).cloned())
            .unwrap_or_else(|| "0".to_string())
    }

    /// `update_stat_stage_setup`
    pub fn update_stat_stage_setup(&mut self, gen: &GenData, cfg: &SummaryConfig, pkmn_idx: usize, move_idx: usize, is_player: bool, new_value: &str) {
        let move_name = {
            let data = if is_player { &self.player_move_data } else { &self.enemy_move_data };
            match data.get(pkmn_idx).and_then(|l| l.get(move_idx)).and_then(|m| m.as_ref()) {
                Some(m) => m.name.clone(),
                None => return,
            }
        };
        while self.stat_stage_setup.len() <= pkmn_idx {
            self.stat_stage_setup.push(CustomMoveData::default());
        }
        self.stat_stage_setup[pkmn_idx].side_mut(is_player).insert(move_name, new_value.to_string());
        self.full_refresh(Some(gen), cfg);
    }

    // ---- golden dump --------------------------------------------------------------------

    /// All computed output, as JSON, for the golden corpus.
    pub fn to_json(&self) -> Value {
        let moves = |data: &Vec<Vec<Option<MoveRenderInfo>>>| -> Value {
            Value::Array(
                data.iter()
                    .map(|list| Value::Array(list.iter().map(|m| m.as_ref().map(|x| x.to_json()).unwrap_or(Value::Null)).collect()))
                    .collect(),
            )
        };
        json!({
            "trainer_name": self.trainer_name,
            "second_trainer_name": self.second_trainer_name,
            "double_battle": self.double_battle_flag,
            "using_global_setup": self.using_global_setup,
            "mimic_options": self.mimic_options,
            "mimic_selection": self.mimic_selection,
            "player_matchups": self.player_pkmn_matchup_data.iter().map(|p| p.to_json()).collect::<Vec<_>>(),
            "enemy_matchups": self.enemy_pkmn_matchup_data.iter().map(|p| p.to_json()).collect::<Vec<_>>(),
            "player_matchup_strings": self.player_pkmn_matchup_data.iter().map(|p| p.to_string()).collect::<Vec<_>>(),
            "enemy_matchup_strings": self.enemy_pkmn_matchup_data.iter().map(|p| p.to_string()).collect::<Vec<_>>(),
            "player_moves": moves(&self.player_move_data),
            "enemy_moves": moves(&self.enemy_move_data),
            "partial_trainer_definition": self.get_partial_trainer_definition().map(|t| t.serialize()),
            "player_field_statuses": self.player_field_statuses.iter().map(field_json).collect::<Vec<_>>(),
            "enemy_field_statuses": self.enemy_field_statuses.iter().map(field_json).collect::<Vec<_>>(),
            "per_matchup_player_modifiers": self.per_matchup_player_modifiers.iter().map(stage_json).collect::<Vec<_>>(),
            "per_matchup_enemy_modifiers": self.per_matchup_enemy_modifiers.iter().map(stage_json).collect::<Vec<_>>(),
        })
    }
}

/// The per-call-invariant arguments of the wild min/max DV damage calls.
struct CommonArgs<'a> {
    gen: &'a GenData,
    mv: &'a xpr_data::model::Move,
    attacking_stages: &'a StageModifiers,
    defending_stages: &'a StageModifiers,
    attacking_field: &'a FieldStatus,
    defending_field: &'a FieldStatus,
    custom: &'a str,
    weather: &'a str,
    is_double_battle: bool,
}

impl<'a> CommonArgs<'a> {
    fn calc(&self, attacking: &EnemyPkmn, defending: &EnemyPkmn, is_crit: bool, a_stats: Option<&StatBlock>, d_stats: Option<&StatBlock>) -> Option<DamageRange> {
        let args = DamageArgs {
            attacking,
            mv: self.mv,
            defending,
            attacking_stages: Some(self.attacking_stages),
            defending_stages: Some(self.defending_stages),
            attacking_field: Some(self.attacking_field),
            defending_field: Some(self.defending_field),
            is_crit,
            custom_move_data: self.custom,
            weather: self.weather,
            is_double_battle: self.is_double_battle,
            attacking_battle_stats: a_stats,
            defending_battle_stats: d_stats,
            attacker_is_enemy: false,
        };
        calculate_damage(self.gen, &args)
    }
}

fn field_json(f: &FieldStatus) -> Value {
    json!({
        "light_screen": f.light_screen, "reflect": f.reflect, "gravity": f.gravity, "magnet_rise": f.magnet_rise,
        "miracle_eye": f.miracle_eye, "power_trick": f.power_trick, "roost": f.roost, "tailwind": f.tailwind,
        "trick_room": f.trick_room, "worry_seed": f.worry_seed, "gastro_acid": f.gastro_acid, "slow_start": f.slow_start,
    })
}

fn stage_json(s: &StageModifiers) -> Value {
    json!({
        "attack_stage": s.attack_stage, "defense_stage": s.defense_stage, "speed_stage": s.speed_stage,
        "special_attack_stage": s.special_attack_stage, "special_defense_stage": s.special_defense_stage,
        "accuracy_stage": s.accuracy_stage, "evasion_stage": s.evasion_stage,
        "attack_badge_boosts": s.attack_badge_boosts, "defense_badge_boosts": s.defense_badge_boosts,
        "speed_badge_boosts": s.speed_badge_boosts, "special_badge_boosts": s.special_badge_boosts,
    })
}

/// `_calc_stage_modifier`
pub fn calc_stage_modifier(gen: &GenData, move_list: &[String]) -> StageModifiers {
    let mut result = StageModifiers::default();
    for m in move_list {
        result = result.apply_stat_mod(&gen.move_db().get_stat_mod(m));
    }
    result
}

/// `_is_move_better`
pub fn is_move_better(new_move: &MoveRenderInfo, prev_move: Option<&MoveRenderInfo>, strat: &str, other_mon: &PkmnRenderInfo, cfg: &SummaryConfig) -> bool {
    if strat == consts::HIGHLIGHT_NONE || !consts::ALL_HIGHLIGHT_STRATS.contains(&strat) {
        return false;
    }
    if new_move.min_damage == -1 {
        return false;
    }
    let Some(prev_move) = prev_move else { return true };
    if prev_move.min_damage == -1 {
        return true;
    }
    let recharge = |m: &MoveRenderInfo| m.attack_flavor.iter().any(|f| f == consts::FLAVOR_RECHARGE);
    if recharge(new_move) && new_move.max_damage < other_mon.defending_mon_hp {
        return false;
    } else if recharge(prev_move) && prev_move.max_damage < other_mon.defending_mon_hp {
        return true;
    }

    let threshold = cfg.calc.consistent_threshold as f64;
    let mut new_fastest_kill: i64 = 1_000_000;
    let mut new_accuracy: f64 = -2.0;
    if !new_move.kill_ranges.is_empty() {
        if strat == consts::HIGHLIGHT_GUARANTEED_KILL {
            let last = *new_move.kill_ranges.last().unwrap();
            if cfg.calc.ignore_accuracy || last.1 != -1.0 {
                new_fastest_kill = last.0;
                new_accuracy = last.1;
            }
        } else if strat == consts::HIGHLIGHT_FASTEST_KILL {
            let first = new_move.kill_ranges[0];
            new_fastest_kill = first.0;
            new_accuracy = first.1;
        } else if strat == consts::HIGHLIGHT_CONSISTENT_KILL {
            for k in &new_move.kill_ranges {
                if k.1 >= threshold {
                    new_fastest_kill = k.0;
                    new_accuracy = k.1;
                    break;
                }
            }
        }
    }
    let mut prev_fastest_kill: i64 = new_fastest_kill + 1;
    let mut prev_accuracy: f64 = -2.0;
    if !prev_move.kill_ranges.is_empty() {
        if strat == consts::HIGHLIGHT_GUARANTEED_KILL {
            let last = *prev_move.kill_ranges.last().unwrap();
            prev_fastest_kill = last.0;
            prev_accuracy = last.1;
        } else if strat == consts::HIGHLIGHT_FASTEST_KILL {
            let first = prev_move.kill_ranges[0];
            prev_fastest_kill = first.0;
            prev_accuracy = first.1;
        } else if strat == consts::HIGHLIGHT_CONSISTENT_KILL {
            for k in &prev_move.kill_ranges {
                if k.1 >= threshold {
                    prev_fastest_kill = k.0;
                    prev_accuracy = k.1;
                    break;
                }
            }
        }
    }
    if new_fastest_kill < prev_fastest_kill {
        return true;
    } else if prev_fastest_kill < new_fastest_kill {
        return false;
    }
    if new_accuracy > prev_accuracy {
        return true;
    } else if prev_accuracy > new_accuracy {
        return false;
    }
    let two_turn = |m: &MoveRenderInfo| m.attack_flavor.iter().any(|f| f == consts::FLAVOR_TWO_TURN || f == consts::FLAVOR_TWO_TURN_INVULN);
    if two_turn(prev_move) {
        return true;
    } else if two_turn(new_move) {
        return false;
    }
    new_move.max_damage > prev_move.max_damage
}
