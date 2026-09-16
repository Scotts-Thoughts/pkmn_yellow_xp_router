//! Port of `routing/state_objects.py` and `routing/full_route_state.py`:
//! the inventory, the solo Pokémon snapshot and the `RouteState` mutators
//! with their "try strictly, record the error, retry forced" protocol.

use std::sync::Arc;

use serde_json::Value;

use xpr_core::consts;
use xpr_core::floor_div;
use xpr_data::badges::BadgeList;
use xpr_data::exp;
use xpr_data::model::{BaseItem, EnemyPkmn, Nature, PokemonSpecies, StageModifiers, StatBlock};
use xpr_data::GenData;

use crate::events::BagSwap;

// ---------------------------------------------------------------------------
// Inventory
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct BagItem {
    pub base_item: Arc<BaseItem>,
    pub num: i64,
}

impl PartialEq for BagItem {
    fn eq(&self, other: &Self) -> bool {
        self.base_item.name == other.base_item.name && self.num == other.num
    }
}

#[derive(Clone, Debug)]
pub struct Inventory {
    pub cur_money: i64,
    pub cur_items: Vec<BagItem>,
    bag_limit: Option<usize>,
}

impl PartialEq for Inventory {
    fn eq(&self, other: &Self) -> bool {
        self.cur_money == other.cur_money && self.cur_items == other.cur_items
    }
}

impl Inventory {
    pub fn new(cur_money: Option<i64>, cur_items: Vec<BagItem>, bag_limit: Option<usize>) -> Inventory {
        Inventory {
            cur_money: cur_money.unwrap_or(3000),
            cur_items,
            bag_limit,
        }
    }

    pub fn bag_limit(&self) -> Option<usize> {
        self.bag_limit
    }

    pub fn serialize(&self) -> Value {
        let items: Vec<Value> = self
            .cur_items
            .iter()
            .map(|x| {
                xpr_core::pyjson::object(vec![
                    (consts::COUNT_KEY, Value::from(x.num)),
                    (consts::NAME_KEY, Value::String(x.base_item.name.clone())),
                ])
            })
            .collect();
        xpr_core::pyjson::object(vec![(consts::MONEY, Value::from(self.cur_money)), (consts::ITEMS_KEY, Value::Array(items))])
    }

    fn index_of(&self, name: &str) -> Option<usize> {
        // Python: a dict from name -> idx, last write wins on duplicate names
        self.cur_items.iter().rposition(|x| x.base_item.name == name)
    }

    pub fn add_item(&self, base_item: &Arc<BaseItem>, num: i64, is_purchase: bool, force: bool, custom_price: Option<i64>) -> Result<Inventory, String> {
        let mut result = self.clone();
        if is_purchase {
            let total_cost = match custom_price {
                Some(p) => num * p,
                None => num * base_item.purchase_price,
            };
            if total_cost > result.cur_money && !force {
                return Err(format!(
                    "Cannot purchase {} {} for {} with only {} money",
                    num, base_item.name, total_cost, result.cur_money
                ));
            }
            result.cur_money -= total_cost;
        }
        if let Some(idx) = result.index_of(&base_item.name) {
            if base_item.is_key_item && !force {
                return Err(format!("Cannot have multiple of the same key item: {}", base_item.name));
            }
            result.cur_items[idx].num += num;
        } else if let Some(limit) = self.bag_limit {
            if result.cur_items.len() >= limit && !force {
                return Err(format!("Cannot add more than {} items to bag", limit));
            } else if result.cur_items.len() < limit {
                result.cur_items.push(BagItem {
                    base_item: base_item.clone(),
                    num,
                });
            }
            // forced with no room: the item is silently dropped
        } else {
            result.cur_items.push(BagItem {
                base_item: base_item.clone(),
                num,
            });
        }
        Ok(result)
    }

    pub fn remove_item(&self, base_item: &Arc<BaseItem>, num: i64, is_sale: bool, force: bool, custom_price: Option<i64>) -> Result<Inventory, String> {
        let Some(idx) = self.index_of(&base_item.name) else {
            if force {
                if is_sale {
                    let mut result = self.clone();
                    let sell_price = custom_price.unwrap_or(base_item.sell_price);
                    result.cur_money += sell_price * num;
                    return Ok(result);
                }
                return Ok(self.clone());
            }
            return Err(format!("Cannot sell/use item that you do not have: {}", base_item.name));
        };
        if base_item.is_key_item && is_sale && !force {
            return Err(format!("Cannot sell key item: {}", base_item.name));
        }
        let mut result = self.clone();
        let have = result.cur_items[idx].num;
        if have < num && !force {
            return Err(format!(
                "Cannot sell/use {} {} when you only have {}",
                num, base_item.name, have
            ));
        }
        result.cur_items[idx].num -= num;
        if result.cur_items[idx].num <= 0 {
            result.cur_items.remove(idx);
        }
        if is_sale {
            let sell_price = custom_price.unwrap_or(base_item.sell_price);
            result.cur_money += sell_price * num;
        }
        Ok(result)
    }

    pub fn has_item(&self, name: &str) -> bool {
        self.cur_items.iter().any(|x| x.base_item.name == name)
    }

    /// The bag order as item names (slot 1 first).
    pub fn item_names(&self) -> Vec<String> {
        self.cur_items.iter().map(|x| x.base_item.name.clone()).collect()
    }

    /// Apply the swaps of a bag-reorder event, in order. Items are matched by
    /// name; the recorded slots only decide whether a swap is reported. The
    /// returned messages are warnings, not errors: a swap whose items sit at
    /// other slots is still applied, a swap naming an item that is not in the
    /// bag is skipped, and the bag always ends up in the best order that can
    /// be made from what is there.
    pub fn swap_items(&self, swaps: &[BagSwap]) -> (Inventory, Vec<String>) {
        let mut result = self.clone();
        let mut warnings = Vec::new();
        for swap in swaps {
            let idx_a = result.index_of(&swap.item_a);
            let idx_b = result.index_of(&swap.item_b);
            let (Some(a), Some(b)) = (idx_a, idx_b) else {
                for (name, idx) in [(&swap.item_a, idx_a), (&swap.item_b, idx_b)] {
                    if idx.is_none() {
                        warnings.push(format!("Cannot swap {}: no {} in bag", swap.to_string(), name));
                    }
                }
                continue;
            };
            if a + 1 != swap.slot_a {
                warnings.push(format!("{} was at slot {}, not slot {}", swap.item_a, a + 1, swap.slot_a));
            }
            if b + 1 != swap.slot_b {
                warnings.push(format!("{} was at slot {}, not slot {}", swap.item_b, b + 1, swap.slot_b));
            }
            result.cur_items.swap(a, b);
        }
        (result, warnings)
    }
}

// ---------------------------------------------------------------------------
// SoloPokemon
// ---------------------------------------------------------------------------

/// An immutable snapshot of the solo Pokémon.
#[derive(Clone, Debug)]
pub struct SoloPokemon {
    pub name: String,
    pub species_def: Arc<PokemonSpecies>,
    pub dvs: StatBlock,
    pub badges: BadgeList,
    pub held_item: Option<String>,
    pub ability_idx: i64,
    pub ability: String,
    pub nature: Nature,
    pub empty_stat_block: StatBlock,
    pub cur_xp: i64,
    pub cur_level: i64,
    pub xp_to_next_level: i64,
    pub move_list: Vec<Option<String>>,
    pub realized_stat_xp: StatBlock,
    pub unrealized_stat_xp: StatBlock,
    pub percent_xp_to_next_level: i64,
    pub percent_xp_to_next_level_str: String,
    pub cur_stats: StatBlock,
}

/// Arguments of the Python constructor that have defaults.
#[derive(Clone, Debug, Default)]
pub struct SoloPokemonArgs {
    pub move_list: Option<Vec<Option<String>>>,
    pub cur_xp: i64,
    pub realized_stat_xp: Option<StatBlock>,
    pub unrealized_stat_xp: Option<StatBlock>,
    pub gained_xp: i64,
    pub gained_stat_xp: Option<StatBlock>,
    pub held_item: Option<String>,
}

impl SoloPokemon {
    /// `SoloPokemon.__init__`
    pub fn new(
        name: &str,
        species_def: Arc<PokemonSpecies>,
        dvs: StatBlock,
        badges: BadgeList,
        empty_stat_block: StatBlock,
        ability_idx: i64,
        nature: Nature,
        args: SoloPokemonArgs,
    ) -> Result<SoloPokemon, String> {
        let ability = if species_def.abilities.is_empty() {
            String::new()
        } else {
            // Python: species_def.abilities[idx] (IndexError on a bad index)
            species_def
                .abilities
                .get(ability_idx.max(0) as usize)
                .cloned()
                .ok_or_else(|| "list index out of range".to_string())?
        };
        let lookup = exp::level_lookup(&species_def.growth_rate)
            .ok_or_else(|| format!("Invalid growth rate: {}", species_def.growth_rate))?;

        let mut cur_xp = if args.cur_xp == 0 { lookup.get_xp_for_level(5)? } else { args.cur_xp };
        let (mut cur_level, mut xp_to_next_level) = lookup.get_level_info(cur_xp);

        let move_list = match args.move_list {
            Some(m) => m,
            None => {
                let mut ml: Vec<Option<String>> = species_def.initial_moves.iter().cloned().map(Some).collect();
                for (lvl, mv) in &species_def.levelup_moves {
                    if *lvl <= cur_level && !ml.iter().any(|m| m.as_deref() == Some(mv.as_str())) {
                        ml.push(Some(mv.clone()));
                    }
                    if ml.len() > 4 {
                        ml = ml[ml.len() - 4..].to_vec();
                    }
                }
                while ml.len() < 4 {
                    ml.push(None);
                }
                ml
            }
        };

        let mut realized_stat_xp = match args.realized_stat_xp {
            Some(r) => r,
            None => empty_stat_block.as_stat_xp(),
        };
        let mut unrealized_stat_xp = args.unrealized_stat_xp.unwrap_or(realized_stat_xp);
        let gained_stat_xp = args.gained_stat_xp.unwrap_or(empty_stat_block);

        let gained_xp = args.gained_xp;
        cur_xp += gained_xp;

        if gained_xp < xp_to_next_level {
            xp_to_next_level -= gained_xp;
            unrealized_stat_xp = unrealized_stat_xp.add(&gained_stat_xp);
        } else {
            let (lvl, tnl) = lookup.get_level_info(cur_xp);
            cur_level = lvl;
            if cur_level == 100 {
                cur_xp = lookup.get_xp_for_level(100)?;
                xp_to_next_level = 0;
                unrealized_stat_xp = unrealized_stat_xp.add(&gained_stat_xp);
            } else {
                unrealized_stat_xp = unrealized_stat_xp.add(&gained_stat_xp);
                realized_stat_xp = unrealized_stat_xp;
                xp_to_next_level = tnl;
            }
        }

        let (percent, percent_str) = if xp_to_next_level <= 0 {
            (0, "N/A".to_string())
        } else {
            let last_level_xp = lookup.get_xp_for_level(cur_level)?;
            let denom = (cur_xp + xp_to_next_level - last_level_xp) as f64;
            let pct = (((xp_to_next_level as f64) / denom) * 100.0) as i64;
            (pct, format!("{} %", pct))
        };

        let cur_stats = species_def.stats.calc_level_stats(
            cur_level,
            &dvs,
            &realized_stat_xp,
            &badges,
            nature,
            args.held_item.as_deref(),
        );

        Ok(SoloPokemon {
            name: name.to_string(),
            species_def,
            dvs,
            badges,
            held_item: args.held_item,
            ability_idx,
            ability,
            nature,
            empty_stat_block,
            cur_xp,
            cur_level,
            xp_to_next_level,
            move_list,
            realized_stat_xp,
            unrealized_stat_xp,
            percent_xp_to_next_level: percent,
            percent_xp_to_next_level_str: percent_str,
            cur_stats,
        })
    }

    /// `serialize` (note the Python dict literal writes `xp` and
    /// `xp_to_next_level` twice; the second value wins: Appendix B item 2).
    pub fn serialize(&self) -> Value {
        xpr_core::pyjson::object(vec![
            (consts::SPECIES_KEY, Value::String(self.species_def.name.clone())),
            (consts::LEVEL, Value::from(self.cur_level)),
            (consts::XP, Value::from(self.cur_xp)),
            (consts::DVS_KEY, self.dvs.serialize()),
            (consts::REALIZED_STAT_XP_KEY, self.realized_stat_xp.serialize()),
            (consts::UNREALIZED_STAT_XP_KEY, self.unrealized_stat_xp.serialize()),
            (consts::STATS_KEY, self.cur_stats.serialize()),
            (
                consts::HELD_ITEM_KEY,
                self.held_item.clone().map(Value::String).unwrap_or(Value::Null),
            ),
            (consts::ABILITY_KEY, Value::String(self.ability.clone())),
            (consts::NATURE_KEY, Value::String(self.nature.display_name())),
            (consts::XP_TO_NEXT_LEVEL, Value::from(self.percent_xp_to_next_level)),
        ])
    }

    /// Python `__eq__`
    pub fn py_eq(&self, other: &SoloPokemon) -> bool {
        self.species_def.name == other.species_def.name
            && self.cur_level == other.cur_level
            && self.cur_xp == other.cur_xp
            && self.dvs == other.dvs
            && self.realized_stat_xp == other.realized_stat_xp
            && self.unrealized_stat_xp == other.unrealized_stat_xp
            && self.cur_stats == other.cur_stats
            && self.held_item == other.held_item
            && self.move_list == other.move_list
    }

    pub fn get_net_gain_from_stat_xp(&self, badges: &BadgeList) -> StatBlock {
        let temp = self.species_def.stats.calc_level_stats(
            self.cur_level,
            &self.dvs,
            &self.empty_stat_block,
            badges,
            self.nature,
            self.held_item.as_deref(),
        );
        self.cur_stats.subtract(&temp)
    }

    pub fn get_pkmn_obj(&self, badges: &BadgeList, stage_modifiers: Option<&StageModifiers>) -> EnemyPkmn {
        let default = StageModifiers::default();
        let stages = stage_modifiers.unwrap_or(&default);
        let battle_stats = self.species_def.stats.calc_battle_stats(
            self.cur_level,
            &self.dvs,
            &self.realized_stat_xp,
            stages,
            Some(badges),
            self.nature,
            self.held_item.as_deref(),
            false,
            None,
        );
        EnemyPkmn {
            name: self.name.clone(),
            level: self.cur_level,
            xp: -1,
            move_list: self.move_list.clone(),
            cur_stats: battle_stats,
            base_stats: self.species_def.stats,
            dvs: self.dvs,
            stat_xp: self.realized_stat_xp,
            badges: Some(badges.clone()),
            held_item: self.held_item.clone(),
            custom_move_data: None,
            is_trainer_mon: true,
            exp_split: 1,
            mon_order: 1,
            definition_order: 1,
            ability: self.ability.clone(),
            nature: self.nature,
        }
    }

    /// `get_move_destination(move_name, dest, force) -> (dest, bool)`
    pub fn get_move_destination(&self, move_name: Option<&str>, dest: Option<i64>, force: bool) -> (Option<i64>, bool) {
        let Some(move_name) = move_name else {
            return (dest, true);
        };
        if force && dest.is_some() {
            return (dest, true);
        }
        if self.move_list.iter().any(|m| m.as_deref() == Some(move_name)) {
            return (None, false);
        }
        for (idx, m) in self.move_list.iter().enumerate() {
            if m.is_none() {
                return (Some(idx as i64), false);
            }
        }
        if dest.is_none() {
            return (None, true);
        }
        (dest, true)
    }
}

// ---------------------------------------------------------------------------
// RouteState
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct RouteState {
    pub solo_pkmn: SoloPokemon,
    pub badges: BadgeList,
    pub inventory: Inventory,
}

impl RouteState {
    pub fn new(solo_pkmn: SoloPokemon, badges: BadgeList, inventory: Inventory) -> RouteState {
        RouteState {
            solo_pkmn,
            badges,
            inventory,
        }
    }

    pub fn py_eq(&self, other: &RouteState) -> bool {
        self.solo_pkmn.py_eq(&other.solo_pkmn) && self.badges == other.badges && self.inventory == other.inventory
    }

    pub fn serialize(&self) -> Value {
        xpr_core::pyjson::object(vec![
            (consts::SOLO_MON_KEY, self.solo_pkmn.serialize()),
            (consts::BADGES_KEY, Value::String(self.badges.to_verbose_string())),
            (consts::INVENTORY_KEY, self.inventory.serialize()),
        ])
    }

    fn rebuild(&self, args: SoloPokemonArgs, badges: BadgeList, name: Option<&str>, species: Option<Arc<PokemonSpecies>>) -> Result<SoloPokemon, String> {
        let cur = &self.solo_pkmn;
        SoloPokemon::new(
            name.unwrap_or(&cur.name),
            species.unwrap_or_else(|| cur.species_def.clone()),
            cur.dvs,
            badges,
            cur.empty_stat_block,
            cur.ability_idx,
            cur.nature,
            args,
        )
    }

    /// `learn_move(move_name, dest, source, force)`
    pub fn learn_move(&self, gen: &GenData, move_name: Option<&str>, dest: Option<i64>, source: &str, force: bool) -> Result<(RouteState, String), String> {
        let mut error_message = String::new();
        let inv = if source == consts::MOVE_SOURCE_LEVELUP || source == consts::MOVE_SOURCE_TUTOR {
            self.inventory.clone()
        } else {
            let item = gen.item_db().get_item(source);
            let consume_item = match item {
                Some(i) => !i.is_key_item,
                None => {
                    error_message = format!("Could not get valid item for move {} source: {}", py_opt(move_name), source);
                    false
                }
            };
            if consume_item {
                let item = item.unwrap();
                match self.inventory.remove_item(item, 1, false, false, None) {
                    Ok(inv) => inv,
                    Err(e) => {
                        error_message = e;
                        self.inventory.remove_item(item, 1, false, true, None)?
                    }
                }
            } else {
                self.inventory.clone()
            }
        };

        // _learn_move
        let mut new_movelist = self.solo_pkmn.move_list.clone();
        let (actual_dest, _) = self.solo_pkmn.get_move_destination(move_name, dest, force);
        if let Some(d) = actual_dest {
            if d < 0 || (d as usize) >= new_movelist.len() {
                return Err("list assignment index out of range".to_string());
            }
            new_movelist[d as usize] = move_name.map(|s| s.to_string());
        }
        let mon = self.rebuild(
            SoloPokemonArgs {
                move_list: Some(new_movelist),
                cur_xp: self.solo_pkmn.cur_xp,
                realized_stat_xp: Some(self.solo_pkmn.realized_stat_xp),
                unrealized_stat_xp: Some(self.solo_pkmn.unrealized_stat_xp),
                held_item: self.solo_pkmn.held_item.clone(),
                ..Default::default()
            },
            self.badges.clone(),
            None,
            None,
        )?;
        Ok((RouteState::new(mon, self.badges.clone(), inv), error_message))
    }

    fn take_vitamin(&self, gen: &GenData, vit_name: &str, badges: BadgeList, force: bool) -> Result<SoloPokemon, String> {
        if gen.is_ev_berry(vit_name) {
            return self.eat_ev_berry(gen, vit_name, badges, force);
        }
        let vit_use_cap = gen.get_vitamin_use_cap();
        let vit_result_cap = gen.get_vitamin_value_cap();
        let vit_boost = gen.get_vitamin_amount();
        let cur = &self.solo_pkmn;
        let mut final_realized = cur.unrealized_stat_xp;
        for stat in gen.get_stats_boosted_by_vitamin(vit_name)? {
            let cur_val = cur.unrealized_stat_xp.get_short(stat).unwrap_or(0);
            if cur_val >= vit_use_cap && !force {
                return Err(format!("Ineffective Vitamin: {} (Already above vitamin cap)", vit_name));
            }
            let cur_boost = vit_boost.min(vit_result_cap - cur_val);
            let block = match stat {
                consts::HP => gen.make_stat_block(cur_boost, 0, 0, 0, 0, 0, true),
                consts::ATK => gen.make_stat_block(0, cur_boost, 0, 0, 0, 0, true),
                consts::DEF => gen.make_stat_block(0, 0, cur_boost, 0, 0, 0, true),
                consts::SPA => gen.make_stat_block(0, 0, 0, cur_boost, 0, 0, true),
                consts::SPD => gen.make_stat_block(0, 0, 0, 0, cur_boost, 0, true),
                consts::SPE => gen.make_stat_block(0, 0, 0, 0, 0, cur_boost, true),
                _ => return Err(format!("Unknown vitamin: {}", vit_name)),
            };
            final_realized = final_realized.add(&block);
        }
        self.rebuild(
            SoloPokemonArgs {
                move_list: Some(cur.move_list.clone()),
                cur_xp: cur.cur_xp,
                realized_stat_xp: Some(final_realized),
                held_item: cur.held_item.clone(),
                ..Default::default()
            },
            badges,
            None,
            None,
        )
    }

    fn eat_ev_berry(&self, gen: &GenData, berry_name: &str, badges: BadgeList, force: bool) -> Result<SoloPokemon, String> {
        let cur = &self.solo_pkmn;
        let sx = cur.unrealized_stat_xp;
        let mut vals = [
            (consts::HP, sx.hp),
            (consts::ATK, sx.attack),
            (consts::DEF, sx.defense),
            (consts::SPA, sx.special_attack),
            (consts::SPD, sx.special_defense),
            (consts::SPE, sx.speed),
        ];
        for lowered in gen.get_stats_lowered_by_ev_berry(berry_name)? {
            let entry = vals.iter_mut().find(|(k, _)| *k == lowered).unwrap();
            if entry.1 <= 0 && !force {
                return Err(format!("Ineffective Berry: {} (Stat EV already at 0)", berry_name));
            }
            entry.1 = gen.get_ev_berry_reduced_value(entry.1);
        }
        let final_realized = gen.make_stat_block(vals[0].1, vals[1].1, vals[2].1, vals[3].1, vals[4].1, vals[5].1, true);
        self.rebuild(
            SoloPokemonArgs {
                move_list: Some(cur.move_list.clone()),
                cur_xp: cur.cur_xp,
                realized_stat_xp: Some(final_realized),
                held_item: cur.held_item.clone(),
                ..Default::default()
            },
            badges,
            None,
            None,
        )
    }

    /// `vitamin(vitamin_name)`
    pub fn vitamin(&self, gen: &GenData, vitamin_name: &str) -> Result<(RouteState, String), String> {
        let mut errors: Vec<String> = Vec::new();
        let new_mon = match self.take_vitamin(gen, vitamin_name, self.badges.clone(), false) {
            Ok(m) => m,
            Err(e) => {
                errors.push(e);
                self.take_vitamin(gen, vitamin_name, self.badges.clone(), true)?
            }
        };
        let item = gen
            .item_db()
            .get_item(vitamin_name)
            .ok_or_else(|| "'NoneType' object has no attribute 'name'".to_string())?;
        let inv = match self.inventory.remove_item(item, 1, false, false, None) {
            Ok(i) => i,
            Err(e) => {
                errors.push(e);
                self.inventory.remove_item(item, 1, false, true, None)?
            }
        };
        Ok((RouteState::new(new_mon, self.badges.clone(), inv), errors.join(", ")))
    }

    /// `rare_candy()`
    pub fn rare_candy(&self, gen: &GenData) -> Result<(RouteState, String), String> {
        let mut error_message = String::new();
        let item = gen
            .item_db()
            .get_item(consts::RARE_CANDY)
            .ok_or_else(|| "'NoneType' object has no attribute 'name'".to_string())?;
        let inv = match self.inventory.remove_item(item, 1, false, false, None) {
            Ok(i) => i,
            Err(e) => {
                error_message = e;
                self.inventory.remove_item(item, 1, false, true, None)?
            }
        };
        let cur = &self.solo_pkmn;
        let mon = self.rebuild(
            SoloPokemonArgs {
                move_list: Some(cur.move_list.clone()),
                cur_xp: cur.cur_xp,
                realized_stat_xp: Some(cur.realized_stat_xp),
                unrealized_stat_xp: Some(cur.unrealized_stat_xp),
                gained_xp: cur.xp_to_next_level,
                held_item: cur.held_item.clone(),
                ..Default::default()
            },
            self.badges.clone(),
            None,
            None,
        )?;
        Ok((RouteState::new(mon, self.badges.clone(), inv), error_message))
    }

    /// `defeat_pkmn(enemy_pkmn, trainer_name, exp_split, pay_day_amount)`
    pub fn defeat_pkmn(&self, gen: &GenData, enemy_pkmn: &EnemyPkmn, trainer_name: Option<&str>, exp_split: i64, pay_day_amount: i64) -> Result<(RouteState, String), String> {
        let new_badges = match trainer_name {
            Some(t) => self.badges.award_badge(t),
            None => self.badges.clone(),
        };
        // _defeat_pkmn
        let gained_xp = if exp_split != 1 {
            let species = gen
                .pkmn_db()
                .get_pkmn(&enemy_pkmn.name)
                .ok_or_else(|| "'NoneType' object has no attribute 'base_xp'".to_string())?;
            exp::calc_xp_yield(species.base_xp, enemy_pkmn.level, enemy_pkmn.is_trainer_mon, exp_split)
        } else {
            enemy_pkmn.xp
        };
        let cur = &self.solo_pkmn;
        let gained_stat_xp = gen
            .get_stat_xp_yield(&enemy_pkmn.name, exp_split, cur.held_item.as_deref())
            .ok_or_else(|| "'NoneType' object has no attribute 'stat_xp_yield'".to_string())?;
        let mon = self.rebuild(
            SoloPokemonArgs {
                move_list: Some(cur.move_list.clone()),
                cur_xp: cur.cur_xp,
                realized_stat_xp: Some(cur.realized_stat_xp),
                unrealized_stat_xp: Some(cur.unrealized_stat_xp),
                gained_xp,
                gained_stat_xp: Some(gained_stat_xp),
                held_item: cur.held_item.clone(),
            },
            new_badges.clone(),
            None,
            None,
        )?;
        // _defeat_trainer
        let trainer_obj = trainer_name.and_then(|t| gen.trainer_db().get_trainer(t));
        let inventory = match trainer_obj {
            None => self.inventory.clone(),
            Some(trainer) => {
                let mut result = self.inventory.clone();
                let mut reward_money = trainer.money;
                if cur.held_item.as_deref() == Some(consts::AMULET_COIN_ITEM_NAME) {
                    reward_money *= 2;
                }
                result.cur_money += reward_money + pay_day_amount;
                if let Some(fight_reward) = gen.get_fight_reward(&trainer.name) {
                    if let Some(item) = gen.item_db().get_item(fight_reward) {
                        if let Ok(r) = result.add_item(item, 1, false, false, None) {
                            result = r;
                        }
                    }
                }
                result
            }
        };
        Ok((RouteState::new(mon, new_badges, inventory), String::new()))
    }

    /// `add_item(item_name, amount, is_purchase, custom_price)`
    pub fn add_item(&self, gen: &GenData, item_name: &str, amount: i64, is_purchase: bool, custom_price: Option<i64>) -> (RouteState, String) {
        let Some(base_item) = gen.item_db().get_item(item_name) else {
            return (self.clone(), format!("Unknown item: {}", item_name));
        };
        match self.inventory.add_item(base_item, amount, is_purchase, false, custom_price) {
            Ok(inv) => (RouteState::new(self.solo_pkmn.clone(), self.badges.clone(), inv), String::new()),
            Err(e) => {
                let inv = self
                    .inventory
                    .add_item(base_item, amount, is_purchase, true, custom_price)
                    .unwrap_or_else(|_| self.inventory.clone());
                (RouteState::new(self.solo_pkmn.clone(), self.badges.clone(), inv), e)
            }
        }
    }

    /// `remove_item(item_name, amount, is_purchase, custom_price)`
    pub fn remove_item(&self, gen: &GenData, item_name: &str, amount: i64, is_sale: bool, custom_price: Option<i64>) -> (RouteState, String) {
        let Some(base_item) = gen.item_db().get_item(item_name) else {
            return (self.clone(), format!("Unknown item: {}", item_name));
        };
        match self.inventory.remove_item(base_item, amount, is_sale, false, custom_price) {
            Ok(inv) => (RouteState::new(self.solo_pkmn.clone(), self.badges.clone(), inv), String::new()),
            Err(e) => {
                let inv = self
                    .inventory
                    .remove_item(base_item, amount, is_sale, true, custom_price)
                    .unwrap_or_else(|_| self.inventory.clone());
                (RouteState::new(self.solo_pkmn.clone(), self.badges.clone(), inv), e)
            }
        }
    }

    /// `hold_item(item_name, consumed)`
    pub fn hold_item(&self, gen: &GenData, item_name: Option<&str>, consumed: bool) -> Result<(RouteState, String), String> {
        let mut error_message = String::new();
        let mut inv = self.inventory.clone();
        let existing_held = self.solo_pkmn.held_item.as_deref();
        if let Some(existing) = existing_held {
            if existing != "None" && existing != consts::NO_ITEM && !consumed {
                match gen.item_db().get_item(existing) {
                    Some(existing_item) => {
                        inv = inv.add_item(existing_item, 1, false, false, None)?;
                    }
                    None => error_message = format!("Unknown existing held item: {}", existing),
                }
            }
        }
        if let Some(name) = item_name {
            if name != "None" && name != consts::NO_ITEM {
                match gen.item_db().get_item(name) {
                    None => error_message = format!("Unknown item: {}", name),
                    Some(new_item) => match inv.remove_item(new_item, 1, false, false, None) {
                        Ok(i) => inv = i,
                        Err(e) => {
                            error_message = e;
                            inv = inv.remove_item(new_item, 1, false, true, None)?;
                        }
                    },
                }
            }
        }
        let cur = &self.solo_pkmn;
        let mon = self.rebuild(
            SoloPokemonArgs {
                move_list: Some(cur.move_list.clone()),
                cur_xp: cur.cur_xp,
                realized_stat_xp: Some(cur.realized_stat_xp),
                unrealized_stat_xp: Some(cur.unrealized_stat_xp),
                held_item: item_name.map(|s| s.to_string()),
                ..Default::default()
            },
            self.badges.clone(),
            None,
            None,
        )?;
        Ok((RouteState::new(mon, self.badges.clone(), inv), error_message))
    }

    /// Bag reorder: the state always advances to the swapped bag; the
    /// returned messages are warnings (see [`Inventory::swap_items`]).
    pub fn reorder_bag(&self, swaps: &[BagSwap]) -> (RouteState, Vec<String>) {
        let (inv, warnings) = self.inventory.swap_items(swaps);
        (RouteState::new(self.solo_pkmn.clone(), self.badges.clone(), inv), warnings)
    }

    /// `blackout()`
    pub fn blackout(&self, gen: &GenData) -> (RouteState, String) {
        let mut inv = self.inventory.clone();
        inv.cur_money = gen.get_money_after_blackout(inv.cur_money, self.solo_pkmn.cur_level, &self.badges);
        (RouteState::new(self.solo_pkmn.clone(), self.badges.clone(), inv), String::new())
    }

    /// `evolve(evolved_species, by_stone)`
    pub fn evolve(&self, gen: &GenData, evolved_species: Option<&str>, by_stone: Option<&str>) -> Result<(RouteState, String), String> {
        let mut error_message = String::new();
        let result_mon = match evolved_species {
            None | Some("") | Some(consts::NO_POKEMON) => self.solo_pkmn.clone(),
            Some(species_name) => match gen.pkmn_db().get_pkmn(species_name) {
                None => {
                    error_message = format!("Could not find dex entry for evolution: {}", species_name);
                    self.solo_pkmn.clone()
                }
                Some(new_species) => {
                    if new_species.growth_rate != self.solo_pkmn.species_def.growth_rate {
                        error_message = format!(
                            "Cannot evolve into species ({}) with different growth rate: {}",
                            new_species.name, new_species.growth_rate
                        );
                        self.solo_pkmn.clone()
                    } else {
                        let cur = &self.solo_pkmn;
                        self.rebuild(
                            SoloPokemonArgs {
                                move_list: Some(cur.move_list.clone()),
                                cur_xp: cur.cur_xp,
                                realized_stat_xp: Some(cur.realized_stat_xp),
                                unrealized_stat_xp: Some(cur.unrealized_stat_xp),
                                held_item: cur.held_item.clone(),
                                ..Default::default()
                            },
                            self.badges.clone(),
                            Some(&new_species.name),
                            Some(new_species.clone()),
                        )?
                    }
                }
            },
        };
        let inv = match by_stone {
            Some(stone) => {
                let item = gen
                    .item_db()
                    .get_item(stone)
                    .ok_or_else(|| "'NoneType' object has no attribute 'name'".to_string())?;
                match self.inventory.remove_item(item, 1, false, false, None) {
                    Ok(i) => i,
                    Err(e) => {
                        error_message.push_str(&e);
                        self.inventory.remove_item(item, 1, false, true, None)?
                    }
                }
            }
            None => self.inventory.clone(),
        };
        Ok((RouteState::new(result_mon, self.badges.clone(), inv), error_message))
    }
}

fn py_opt(s: Option<&str>) -> String {
    match s {
        Some(v) => v.to_string(),
        None => "None".to_string(),
    }
}

/// Python's `floor(x / n)` helper re-exported for the router.
pub fn floor_div_i64(a: i64, b: i64) -> i64 {
    floor_div(a, b)
}
