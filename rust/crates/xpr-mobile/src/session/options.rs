//! Option lists for the pickers (trainers, items, Pokémon, moves, ...) and
//! the route state at an insertion point, which several forms depend on.

use std::sync::Arc;

use serde_json::{json, Value};

use xpr_core::consts;
use xpr_data::model::EnemyPkmn;
use xpr_data::GenData;
use xpr_engine::{NodeId, ObjKind, RouteState};

use super::{b, i, opt_s, s, Session};

pub(crate) fn mon_json(gen: &GenData, m: &EnemyPkmn) -> Value {
    let st = &m.cur_stats;
    let g1 = gen.get_generation() == 1;
    let stats = if g1 {
        json!([["HP", st.hp], ["Atk", st.attack], ["Def", st.defense], ["Spc", st.special_attack], ["Spe", st.speed]])
    } else {
        json!([["HP", st.hp], ["Atk", st.attack], ["Def", st.defense], ["SpA", st.special_attack], ["SpD", st.special_defense], ["Spe", st.speed]])
    };
    let types = gen.pkmn_db().get_pkmn(&m.name).map(|sp| {
        let mut t = vec![sp.first_type.clone()];
        if !sp.second_type.is_empty() && sp.second_type != sp.first_type {
            t.push(sp.second_type.clone());
        }
        t
    });
    json!({
        "name": m.name,
        "level": m.level,
        "xp": m.xp,
        "moves": m.move_list.iter().flatten().collect::<Vec<_>>(),
        "held_item": m.held_item,
        "ability": if gen.get_generation() >= 3 { Some(m.ability.clone()) } else { None },
        "nature": if gen.get_generation() >= 3 && m.nature.value() != 0 { Some(m.nature.display_name()) } else { None },
        "stats": stats,
        "speed": st.speed,
        "types": types,
    })
}

impl Session {
    /// The route state an event inserted after `after` (or before `before`)
    /// starts from: the inline creator's "insertion point".
    pub(crate) fn state_at(&self, after: Option<NodeId>, before: Option<NodeId>) -> Option<Arc<RouteState>> {
        let after = after.filter(|x| self.ctrl.router.contains_id(*x));
        let before = before.filter(|x| self.ctrl.router.contains_id(*x));
        if let Some(b) = before {
            return self.ctrl.router.init_state_of(b).cloned();
        }
        match after {
            Some(id) => {
                let id = match self.ctrl.router.obj_kind(id) {
                    Some(ObjKind::Item) => self.ctrl.router.item(id).map(|i| i.parent).unwrap_or(id),
                    _ => id,
                };
                self.ctrl.router.final_state_of(id).cloned().or_else(|| self.ctrl.get_init_state())
            }
            None => {
                if self.ctrl.is_empty() {
                    self.ctrl.get_init_state()
                } else {
                    self.ctrl.get_final_state()
                }
            }
        }
    }

    pub(crate) fn trainer_json(&self, gen: &GenData, name: &str) -> Value {
        let Some(t) = gen.trainer_db().get_trainer(name) else { return Value::Null };
        let defeated = self.ctrl.router.defeated_trainers.contains(&t.name);
        json!({
            "name": t.name,
            "class": t.trainer_class,
            "location": t.location,
            "money": t.money,
            "double": t.double_battle,
            "rematch": t.rematch,
            "refightable": t.refightable,
            "defeated": defeated,
            "exp_per_sec": xpr_data::exp::experience_per_second(gen.get_trainer_timing_info(), &t.pkmn),
            "total_exp": t.pkmn.iter().map(|m| m.xp).sum::<i64>(),
            "category": gen.get_fight_category(&t.name),
            "major": gen.is_major_fight(&t.name),
            "mons": t.pkmn.iter().map(|m| mon_json(gen, m)).collect::<Vec<_>>(),
            "event_id": self.ctrl.find_first_event_by_trainer_name(&t.name),
        })
    }

    pub(super) fn options_call(&mut self, method: &str, p: &Value) -> Result<Value, String> {
        let gen = self.gen()?;
        match method {
            "opts.kinds" => {
                let st = self.state_at(i(p, "after"), i(p, "before"));
                let has_forms = st.as_ref().map(|s| !gen.pkmn_db().get_forms(&s.solo_pkmn.name).is_empty()).unwrap_or(false);
                let kinds: Vec<Value> = super::drafts::KINDS
                    .iter()
                    .filter(|k| k.0 != "bag_reorder" || gen.supports_bag_reorder())
                    .filter(|k| k.0 != "change_form" || has_forms)
                    .filter(|k| k.0 != "day_care")
                    .map(|k| json!({"kind": k.0, "label": k.1, "symbol": k.2, "group": k.3}))
                    .collect();
                Ok(json!(kinds))
            }
            "opts.context" => {
                let st = self.state_at(i(p, "after"), i(p, "before"));
                Ok(self.context_json(&gen, st.as_deref()))
            }
            "opts.locations" => {
                let mut l = gen.trainer_db().get_all_locations();
                l.sort();
                Ok(json!(l))
            }
            "opts.classes" => {
                let mut l = gen.trainer_db().get_all_classes();
                l.sort();
                Ok(json!(l))
            }
            "opts.trainers" => {
                let loc = opt_s(p, "location").filter(|x| !x.is_empty());
                let cls = opt_s(p, "class").filter(|x| !x.is_empty());
                let mut defeated = if b(p, "include_defeated") { Vec::new() } else { self.ctrl.get_defeated_trainers() };
                if let Some(keep) = opt_s(p, "keep") {
                    defeated.retain(|d| *d != keep);
                }
                if let Some(partner) = opt_s(p, "partner") {
                    defeated.push(partner);
                }
                let names = gen.trainer_db().get_valid_trainers(cls.as_deref(), loc.as_deref(), &defeated, b(p, "rematches"), b(p, "multi_only"));
                let f = s(p, "filter").to_lowercase();
                let out: Vec<Value> = names
                    .iter()
                    .filter(|n| f.is_empty() || n.to_lowercase().contains(&f))
                    .filter_map(|n| gen.trainer_db().get_trainer(n))
                    .map(|t| {
                        json!({
                            "name": t.name, "class": t.trainer_class, "location": t.location,
                            "mons": t.pkmn.len(), "double": t.double_battle,
                            "can_multi": gen.get_generation() >= 3 && gen.trainer_db().can_trainer_multi_battle(&t.name),
                            "exp_per_sec": xpr_data::exp::experience_per_second(gen.get_trainer_timing_info(), &t.pkmn),
                            "lead": t.pkmn.last().map(|m| m.name.clone()),
                            "major": gen.is_major_fight(&t.name),
                        })
                    })
                    .collect();
                Ok(json!(out))
            }
            "opts.trainer" => Ok(self.trainer_json(&gen, s(p, "name"))),
            "opts.marts" => {
                let mut m: Vec<String> = gen.item_db().mart_items.keys().cloned().collect();
                m.sort();
                Ok(json!(m))
            }
            "opts.items" => {
                let ty = opt_s(p, "type").unwrap_or_else(|| consts::ITEM_TYPE_ALL_ITEMS.to_string());
                let mart = opt_s(p, "mart").unwrap_or_else(|| consts::ITEM_TYPE_ALL_ITEMS.to_string());
                let backpack = ty == consts::ITEM_TYPE_BACKPACK_ITEMS;
                let mut names = gen.item_db().get_filtered_names(if backpack { consts::ITEM_TYPE_ALL_ITEMS } else { &ty }, &mart, None);
                if backpack {
                    let st = self.state_at(i(p, "after"), i(p, "before"));
                    let bag: Vec<String> = st.map(|s| s.inventory.cur_items.iter().map(|b| b.base_item.name.clone()).collect()).unwrap_or_default();
                    names.retain(|n| bag.contains(n));
                }
                let f = s(p, "filter").to_lowercase();
                if !f.is_empty() {
                    names.retain(|n| n.to_lowercase().contains(&f));
                }
                let out: Vec<Value> = names
                    .iter()
                    .filter_map(|n| gen.item_db().get_item(n))
                    .map(|it| {
                        json!({"name": it.name, "buy": it.purchase_price, "sell": it.sell_price, "key": it.is_key_item, "move": it.move_name,
                               "pp": gen.pp_item_effect(&it.name).is_some(), "needs_target": gen.pp_item_effect(&it.name).map(|e| e.needs_target()).unwrap_or(false)})
                    })
                    .collect();
                Ok(json!(out))
            }
            "opts.item" => {
                let it = gen.item_db().get_item(s(p, "name")).ok_or("no such item")?;
                let is_vit = gen.get_valid_vitamins().contains(&it.name.as_str()) || gen.get_valid_ev_berries().contains(&it.name.as_str());
                Ok(json!({"name": it.name, "buy": it.purchase_price, "sell": it.sell_price, "key": it.is_key_item, "move": it.move_name,
                    "pp": gen.pp_item_effect(&it.name).is_some(), "needs_target": gen.pp_item_effect(&it.name).map(|e| e.needs_target()).unwrap_or(false),
                    "vitamin": is_vit, "candy": it.name == consts::RARE_CANDY}))
            }
            "opts.pokemon" => {
                let f = s(p, "filter");
                let growth = opt_s(p, "same_growth_as").and_then(|sp| gen.pkmn_db().get_pkmn(&sp).map(|m| m.growth_rate.clone()));
                let names = if f.trim().is_empty() { gen.pkmn_db().get_all_names(growth.as_deref()) } else { gen.pkmn_db().get_filtered_names(Some(f), growth.as_deref()) };
                Ok(json!(names))
            }
            "opts.moves" => {
                let f = opt_s(p, "filter").filter(|x| !x.is_empty());
                Ok(json!(gen.move_db().get_filtered_names(f.as_deref(), b(p, "include_delete"))))
            }
            "opts.tms" => {
                let names = gen.item_db().get_filtered_names(consts::ITEM_TYPE_TM, consts::ITEM_TYPE_ALL_ITEMS, None);
                let out: Vec<Value> = names.iter().filter_map(|n| gen.item_db().get_item(n)).map(|it| json!({"name": it.name, "move": it.move_name})).collect();
                Ok(json!(out))
            }
            "opts.vitamins" => {
                let mut v: Vec<&str> = gen.get_valid_vitamins();
                v.extend(gen.get_valid_ev_berries());
                Ok(json!(v))
            }
            "opts.stones" => Ok(json!(gen.item_db().get_filtered_names(consts::ITEM_TYPE_ALL_ITEMS, consts::ITEM_TYPE_ALL_ITEMS, Some("stone")))),
            "opts.forms" => Ok(json!(gen.pkmn_db().get_forms(s(p, "species")))),
            "opts.move_destination" => {
                // which slot a move goes to, and whether the user may choose
                let st = self.state_at(i(p, "after"), i(p, "before")).ok_or("no state")?;
                let mv = opt_s(p, "move").filter(|m| !m.is_empty() && m != consts::DELETE_MOVE);
                let (dest, free) = st.solo_pkmn.get_move_destination(mv.as_deref(), None, false);
                Ok(json!({"dest": dest, "free": free}))
            }
            "opts.league" => {
                let e4 = self.ctrl.league_lineup(false);
                let champ = self.ctrl.league_lineup(true);
                Ok(json!({"elite4": e4, "with_champion": champ}))
            }
            _ => Err(format!("unknown method {}", method)),
        }
    }

    /// The pieces of the state at an insertion point the forms use.
    pub(crate) fn context_json(&self, gen: &GenData, st: Option<&RouteState>) -> Value {
        let Some(st) = st else { return json!({"has_state": false}) };
        let moves: Vec<Value> = st.solo_pkmn.move_list.iter().map(|m| json!(m)).collect();
        let bag: Vec<Value> = st.inventory.cur_items.iter().map(|b| json!({"name": b.base_item.name, "num": b.num})).collect();
        let forms = gen.pkmn_db().get_forms(&st.solo_pkmn.name);
        let u = &st.solo_pkmn.unrealized_stat_xp;
        json!({
            "has_state": true,
            "species": st.solo_pkmn.name,
            "level": st.solo_pkmn.cur_level,
            "moves": moves,
            "bag": bag,
            "money": st.inventory.cur_money,
            "forms": forms,
            "held_item": st.solo_pkmn.held_item,
            "evs": [u.hp, u.attack, u.defense, u.special_attack, u.special_defense, u.speed],
            "generation": gen.get_generation(),
            "ev_cap": if gen.get_generation() <= 2 { xpr_data::model::STAT_XP_CAP_GEN12 } else { xpr_data::model::SINGLE_STAT_EV_CAP },
            "ev_total_cap": if gen.get_generation() <= 2 { Value::Null } else { json!(xpr_data::model::TOTAL_EV_CAP) },
        })
    }
}
