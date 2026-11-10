//! Event "drafts": the flat field set the iOS forms edit, converted to and
//! from `EventDefinition`. One schema serves creation (the desktop's inline
//! creator and quick-add) and editing (the details editors):
//!
//! `{ "kind": <kind>, ...fields }` with kinds
//! trainer, wild, rare_candy, vitamin, get_item, buy_item, sell_item,
//! use_item, hold_item, tm, tutor, levelup, save, heal, blackout, evolve,
//! bag_reorder, ev_override, day_care, notes.

use serde_json::{json, Value};

use xpr_core::consts;
use xpr_data::GenData;
use xpr_engine::{
    swaps_between, DayCareEventDefinition, EvOverrideEventDefinition, EventDefinition, EvolutionEventDefinition,
    HoldItemEventDefinition, InventoryEventDefinition, LearnMoveEventDefinition, LevelVal, RouteState,
    TrainerEventDefinition, VitaminEventDefinition, WildPkmnEventDefinition,
};

use super::{b, i, opt_s, s, strs};

pub(crate) fn kind_of(def: &EventDefinition) -> &'static str {
    match def.get_event_type() {
        consts::TASK_TRAINER_BATTLE => "trainer",
        consts::TASK_RARE_CANDY => "rare_candy",
        consts::TASK_VITAMIN => "vitamin",
        consts::TASK_FIGHT_WILD_PKMN => "wild",
        consts::TASK_GET_FREE_ITEM => "get_item",
        consts::TASK_PURCHASE_ITEM => "buy_item",
        consts::TASK_SELL_ITEM => "sell_item",
        consts::TASK_USE_ITEM => "use_item",
        consts::TASK_HOLD_ITEM => "hold_item",
        consts::TASK_LEARN_MOVE_LEVELUP => "levelup",
        consts::TASK_LEARN_MOVE_TM => {
            if def.learn_move.as_ref().map(|l| l.source == consts::MOVE_SOURCE_TUTOR).unwrap_or(false) {
                "tutor"
            } else {
                "tm"
            }
        }
        consts::TASK_SAVE => "save",
        consts::TASK_HEAL => "heal",
        consts::TASK_BLACKOUT => "blackout",
        consts::TASK_EVOLUTION => "evolve",
        consts::TASK_REORDER_BAG => "bag_reorder",
        consts::TASK_EV_OVERRIDE => "ev_override",
        consts::TASK_DAY_CARE => "day_care",
        _ => "notes",
    }
}

/// Display name and SF symbol of each creatable kind, in menu order.
pub(crate) const KINDS: [(&str, &str, &str, &str); 20] = [
    ("trainer", "Fight Trainer", "person.2.fill", "Battles"),
    ("wild", "Wild Pokémon", "leaf.fill", "Battles"),
    ("get_item", "Get Item", "gift.fill", "Items"),
    ("buy_item", "Buy Item", "cart.fill", "Items"),
    ("sell_item", "Sell Item", "dollarsign.circle.fill", "Items"),
    ("use_item", "Use / Drop Item", "trash.fill", "Items"),
    ("hold_item", "Hold Item", "hand.raised.fill", "Items"),
    ("bag_reorder", "Reorder Bag", "arrow.up.arrow.down", "Items"),
    ("rare_candy", "Rare Candy", "birthday.cake.fill", "Training"),
    ("vitamin", "Vitamin / EV Berry", "pill.fill", "Training"),
    ("ev_override", "EV Override", "slider.horizontal.3", "Training"),
    ("tm", "TM / HM Move", "opticaldisc.fill", "Moves"),
    ("tutor", "Tutor Move", "graduationcap.fill", "Moves"),
    ("evolve", "Evolve", "sparkles", "Pokémon"),
    ("change_form", "Change Form", "arrow.triangle.2.circlepath", "Pokémon"),
    ("save", "Save", "square.and.arrow.down.fill", "Other"),
    ("heal", "Heal", "cross.case.fill", "Other"),
    ("blackout", "Blackout", "moon.zzz.fill", "Other"),
    ("notes", "Notes", "note.text", "Other"),
    ("day_care", "Day Care", "house.fill", "Other"),
];

fn dest_json(d: Option<i64>) -> Value {
    d.map(|x| json!(x)).unwrap_or(Value::Null)
}

/// The draft of an existing event.
pub(crate) fn describe(def: &EventDefinition, gen: &GenData) -> Value {
    let kind = kind_of(def);
    let mut d = json!({"kind": kind});
    let o = d.as_object_mut().unwrap();
    if let Some(t) = &def.trainer_def {
        o.insert("trainer".into(), json!(t.trainer_name));
        o.insert("second_trainer".into(), json!(t.second_trainer_name_str()));
        o.insert("pay_day".into(), json!(t.pay_day_amount_int().unwrap_or(0)));
        o.insert("exp_split".into(), json!(t.exp_split));
        o.insert("mon_order".into(), json!(t.mon_order));
        o.insert("thief".into(), json!(t.thief_mons));
    }
    if let Some(r) = &def.rare_candy {
        o.insert("amount".into(), json!(r.amount));
    }
    if let Some(v) = &def.vitamin {
        o.insert("vitamin".into(), json!(v.vitamin));
        o.insert("amount".into(), json!(v.amount));
    }
    if let Some(w) = &def.wild_pkmn_info {
        o.insert("species".into(), json!(w.name));
        o.insert("level".into(), json!(w.level));
        o.insert("quantity".into(), json!(w.quantity));
        o.insert("trainer_pkmn".into(), json!(w.trainer_pkmn));
    }
    if let Some(it) = &def.item_event_def {
        o.insert("item".into(), json!(it.item_name));
        o.insert("amount".into(), json!(it.item_amount));
        o.insert("target_move".into(), json!(it.target_move));
        o.insert("tossed".into(), json!(it.no_effect));
        if it.with_money {
            let base = gen.item_db().get_item(&it.item_name);
            let price = if it.is_acquire { base.map(|b| b.purchase_price) } else { base.map(|b| b.sell_price) };
            o.insert("unit_price".into(), json!(it.custom_price.or(price)));
        }
    }
    if let Some(h) = &def.hold_item {
        o.insert("item".into(), json!(h.item_name));
        o.insert("consume_held".into(), json!(h.consumed));
        o.insert("hold_nothing".into(), json!(h.item_name.is_none()));
    }
    if let Some(l) = &def.learn_move {
        o.insert("move".into(), json!(l.move_to_learn));
        o.insert("dest".into(), dest_json(l.destination));
        o.insert("source".into(), json!(l.source));
        o.insert("level".into(), l.level.to_value());
        o.insert("mon".into(), json!(l.mon));
        o.insert("force_dest".into(), json!(l.force_destination));
        if kind == "tm" {
            o.insert("tm".into(), json!(l.source));
        }
    }
    for (k, loc) in [("save", &def.save), ("heal", &def.heal), ("blackout", &def.blackout)] {
        if let Some(l) = loc {
            let _ = k;
            o.insert("location".into(), json!(l.location_str()));
        }
    }
    if let Some(e) = &def.evolution {
        o.insert("species".into(), json!(e.evolved_species));
        o.insert("stone".into(), json!(e.by_stone));
    }
    if let Some(r) = &def.bag_reorder {
        o.insert("swaps".into(), json!(r.swaps.iter().map(|s| s.to_string()).collect::<Vec<_>>()));
    }
    if let Some(e) = &def.ev_override {
        o.insert("evs".into(), json!(e.values()));
    }
    if let Some(dc) = &def.day_care {
        o.insert("exp".into(), json!(dc.exp_gained));
    }
    o.insert("notes".into(), json!(def.notes));
    d
}

fn amount(p: &Value) -> Result<i64, String> {
    let a = i(p, "amount").unwrap_or(1);
    if a < 1 {
        return Err("The amount must be at least 1".into());
    }
    Ok(a)
}

fn valid_item(gen: &GenData, name: &str) -> Result<String, String> {
    gen.item_db().get_item(name).map(|i| i.name.clone()).ok_or_else(|| format!("Pick an item (\"{}\" is not one)", name))
}

fn valid_mon(gen: &GenData, name: &str) -> Result<String, String> {
    gen.pkmn_db().get_pkmn(name).map(|m| m.name.clone()).ok_or_else(|| format!("Pick a Pokémon (\"{}\" is not one)", name))
}

/// Build the definition for `draft`. `base` is the event being edited: the
/// fields a draft doesn't carry (battle-summary setup, recorded times,
/// tags) are kept from it. `state` is the route state at the insertion point
/// (bag reorder's base order, evolve's default species).
pub(crate) fn build(p: &Value, base: Option<&EventDefinition>, gen: &GenData, state: Option<&RouteState>) -> Result<EventDefinition, String> {
    let kind = s(p, "kind");
    let mut def = match kind {
        "trainer" => {
            let name = s(p, "trainer");
            let trainer = gen.trainer_db().get_trainer(name).ok_or("Pick a trainer")?.clone();
            // keep the battle-summary setup when the trainer is unchanged
            let mut td = match base.and_then(|b| b.trainer_def.clone()) {
                Some(t) if t.trainer_name == trainer.name => t,
                _ => {
                    let mut t = TrainerEventDefinition::new(&trainer.name);
                    if trainer.double_battle && p.get("exp_split").is_none() {
                        let n = EventDefinition::with_trainer(t.clone()).pokemon_list(gen).map(|l| l.len()).unwrap_or(0);
                        t.exp_split = vec![2; n];
                    }
                    t
                }
            };
            if let Some(second) = opt_s(p, "second_trainer").filter(|x| !x.is_empty()) {
                td.set_second_trainer_name(&second);
                if p.get("exp_split").is_none() {
                    let n = EventDefinition::with_trainer(td.clone()).pokemon_list(gen).map(|l| l.len()).unwrap_or(0);
                    td.exp_split = vec![2; n];
                }
            }
            if p.get("exp_split").map(|v| v.is_array()).unwrap_or(false) {
                td.exp_split = p["exp_split"].as_array().unwrap().iter().filter_map(|x| x.as_i64()).collect();
            }
            if p.get("mon_order").map(|v| v.is_array()).unwrap_or(false) {
                td.mon_order = p["mon_order"].as_array().unwrap().iter().filter_map(|x| x.as_i64()).collect();
            }
            if p.get("thief").map(|v| v.is_array()).unwrap_or(false) {
                td.thief_mons = p["thief"].as_array().unwrap().iter().filter_map(|x| x.as_i64()).collect();
            }
            if p.get("pay_day").is_some() {
                td.pay_day_amount = Some(i(p, "pay_day").unwrap_or(0).max(0));
            }
            EventDefinition::with_trainer(td)
        }
        "wild" => {
            let level = i(p, "level").unwrap_or(5);
            if !(2..=100).contains(&level) {
                return Err("Level must be between 2 and 100".into());
            }
            let q = i(p, "quantity").unwrap_or(1);
            if q < 1 {
                return Err("Quantity must be at least 1".into());
            }
            EventDefinition::with_wild(WildPkmnEventDefinition::new(&valid_mon(gen, s(p, "species"))?, level, q, b(p, "trainer_pkmn")))
        }
        "rare_candy" => EventDefinition::with_rare_candy(amount(p)?),
        "vitamin" => {
            let v = s(p, "vitamin");
            if !gen.get_valid_vitamins().contains(&v) && !gen.get_valid_ev_berries().contains(&v) {
                return Err("Pick a vitamin".into());
            }
            EventDefinition::with_vitamin(VitaminEventDefinition::new(v, amount(p)?))
        }
        "get_item" => EventDefinition::with_item(InventoryEventDefinition::new(&valid_item(gen, s(p, "item"))?, amount(p)?, true, false, None)),
        "buy_item" => EventDefinition::with_item(InventoryEventDefinition::new(&valid_item(gen, s(p, "item"))?, amount(p)?, true, true, None)),
        "sell_item" => EventDefinition::with_item(InventoryEventDefinition::new(&valid_item(gen, s(p, "item"))?, amount(p)?, false, true, None)),
        "use_item" => {
            let item = valid_item(gen, s(p, "item"))?;
            let amt = amount(p)?;
            // the inline creator turns a used vitamin / candy into its own event
            if b(p, "convert") && (gen.get_valid_vitamins().contains(&item.as_str()) || gen.get_valid_ev_berries().contains(&item.as_str())) {
                EventDefinition::with_vitamin(VitaminEventDefinition::new(&item, amt))
            } else if b(p, "convert") && item == consts::RARE_CANDY {
                EventDefinition::with_rare_candy(amt)
            } else {
                let effect = gen.pp_item_effect(&item);
                let tossed = b(p, "tossed") && effect.is_some();
                let target = if effect.map(|e| e.needs_target()).unwrap_or(false) && !tossed { opt_s(p, "target_move").filter(|m| !m.is_empty()) } else { None };
                let mut it = InventoryEventDefinition::use_on(&item, amt, target.as_deref());
                it.no_effect = tossed;
                EventDefinition::with_item(it)
            }
        }
        "drop_item" => {
            // quick-add "Drop": a PP item dropped restores nothing
            let item = valid_item(gen, s(p, "item"))?;
            let mut it = InventoryEventDefinition::new(&item, amount(p)?, false, false, None);
            it.no_effect = gen.pp_item_effect(&item).is_some();
            EventDefinition::with_item(it)
        }
        "hold_item" => {
            let item = if b(p, "hold_nothing") { None } else { Some(valid_item(gen, s(p, "item"))?) };
            EventDefinition::with_hold_item(HoldItemEventDefinition::new(item.as_deref(), b(p, "consume_held")))
        }
        "tm" => {
            let tm = s(p, "tm");
            let item = gen.item_db().get_item(tm).ok_or("Pick a TM / HM")?;
            let mv = item.move_name.clone().ok_or("That item doesn't teach a move")?;
            EventDefinition::with_learn_move(LearnMoveEventDefinition::new(Some(&mv), i(p, "dest"), &item.name, LevelVal::any(), None, b(p, "force_dest")))
        }
        "tutor" => {
            let mv = s(p, "move");
            let mv = if mv.is_empty() || mv == consts::DELETE_MOVE {
                None
            } else {
                Some(gen.move_db().get_move(mv).map(|m| m.name.clone()).ok_or("Pick a move")?)
            };
            EventDefinition::with_learn_move(LearnMoveEventDefinition::new(mv.as_deref(), i(p, "dest"), consts::MOVE_SOURCE_TUTOR, LevelVal::any(), None, b(p, "force_dest")))
        }
        "levelup" => {
            let base_lm = base.and_then(|b| b.learn_move.clone()).ok_or("Level-up moves can only be edited")?;
            EventDefinition::with_learn_move(LearnMoveEventDefinition::new(base_lm.move_to_learn.as_deref(), i(p, "dest"), consts::MOVE_SOURCE_LEVELUP, base_lm.level.clone(), base_lm.mon.as_deref(), false))
        }
        "save" => EventDefinition::with_save(s(p, "location")),
        "heal" => EventDefinition::with_heal(s(p, "location")),
        "blackout" => {
            let mut d = EventDefinition::with_blackout();
            if let Some(l) = d.blackout.as_mut() {
                *l = xpr_engine::LocationEventDefinition::new(s(p, "location"));
            }
            d
        }
        "evolve" | "change_form" => {
            let species = match opt_s(p, "species").filter(|x| !x.is_empty()) {
                Some(sp) => valid_mon(gen, &sp)?,
                None => {
                    let cur = state.map(|st| st.solo_pkmn.name.clone()).ok_or("Pick a species")?;
                    if kind == "change_form" {
                        gen.pkmn_db().get_forms(&cur).into_iter().find(|f| *f != cur).ok_or("This Pokémon has no other forms")?
                    } else {
                        cur
                    }
                }
            };
            let stone = opt_s(p, "stone").filter(|x| !x.is_empty() && x != consts::NO_ITEM);
            let mut d = EventDefinition::with_evolution(&species);
            d.evolution = Some(EvolutionEventDefinition::new(&species, stone.as_deref()));
            d
        }
        "bag_reorder" => {
            if p.get("bag").map(|v| v.is_array()).unwrap_or(false) {
                let base_names: Vec<String> = state.map(|st| st.inventory.cur_items.iter().map(|b| b.base_item.name.clone()).collect()).unwrap_or_default();
                let new = strs(p, "bag");
                EventDefinition::with_bag_reorder(swaps_between(&base_names, &new)?)
            } else {
                EventDefinition::with_bag_reorder(base.and_then(|b| b.bag_reorder.as_ref().map(|r| r.swaps.clone())).unwrap_or_default())
            }
        }
        "ev_override" => {
            let v: Vec<i64> = p.get("evs").and_then(|x| x.as_array()).map(|a| a.iter().map(|x| x.as_i64().unwrap_or(0)).collect()).unwrap_or_default();
            let vals: [i64; 6] = match v.len() {
                6 => [v[0], v[1], v[2], v[3], v[4], v[5]],
                _ => {
                    let u = state.map(|st| st.solo_pkmn.unrealized_stat_xp.clone());
                    u.map(|u| [u.hp, u.attack, u.defense, u.special_attack, u.special_defense, u.speed]).unwrap_or([0; 6])
                }
            };
            let cap = if gen.get_generation() <= 2 { xpr_data::model::STAT_XP_CAP_GEN12 } else { xpr_data::model::SINGLE_STAT_EV_CAP };
            if vals.iter().any(|x| *x < 0 || *x > cap) {
                return Err(format!("Each value must be between 0 and {}", cap));
            }
            let spd = if gen.get_generation() <= 2 { vals[3] } else { vals[4] };
            EventDefinition::with_ev_override(EvOverrideEventDefinition::new(vals[0], vals[1], vals[2], vals[3], spd, vals[5]))
        }
        "day_care" => {
            let exp = i(p, "exp").unwrap_or(1);
            if exp < 1 {
                return Err("Exp must be at least 1".into());
            }
            let mut d = EventDefinition::default();
            d.day_care = Some(DayCareEventDefinition::new(exp));
            d
        }
        "notes" => EventDefinition::default(),
        other => return Err(format!("unknown event kind {}", other)),
    };
    // notes: the draft's, else the edited event's
    def.notes = match p.get("notes").and_then(|n| n.as_str()) {
        Some(n) => n.trim().to_string(),
        None => base.map(|b| b.notes.clone()).unwrap_or_default(),
    };
    if let Some(b) = base {
        def.enabled = b.enabled;
        def.tags = b.tags.clone();
    }
    Ok(def)
}
