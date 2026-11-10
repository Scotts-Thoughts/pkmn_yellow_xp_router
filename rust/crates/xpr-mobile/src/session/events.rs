//! One event in detail (the desktop's Pre-Event State tab, warnings, the
//! editor's draft and the trainer team) and the create / update / notes
//! operations, including the "insert before this event" actions of the
//! state view and the quick-add extras (whole area, Elite Four, multi).

use serde_json::{json, Value};

use xpr_core::consts;
use xpr_data::badges::BadgeList;
use xpr_data::model::{EnemyPkmn, StatBlock};
use xpr_data::GenData;
use xpr_engine::{EventDefinition, NodeId, ObjKind, RouteState};

use super::drafts;
use super::options::mon_json;
use super::{b, i, opt_s, s, Session};

fn stat_vals(gen1: bool, sb: &StatBlock) -> Vec<i64> {
    if gen1 {
        vec![sb.hp, sb.attack, sb.defense, sb.special_attack, sb.speed]
    } else {
        vec![sb.hp, sb.attack, sb.defense, sb.special_attack, sb.special_defense, sb.speed]
    }
}

fn boost_flags(gen: &GenData, pkmn: &EnemyPkmn, badges: &BadgeList) -> Vec<bool> {
    let cur_gen = gen.get_generation();
    let spd = if badges.is_special_defense_boosted() {
        if cur_gen == 2 {
            let unboosted = pkmn.base_stats.calc_level_stats(pkmn.level, &pkmn.dvs, &pkmn.stat_xp, &gen.make_badge_list(), pkmn.nature, None).special_attack;
            !xpr_data::stats::should_ignore_spd_badge_boost(unboosted)
        } else {
            true
        }
    } else {
        false
    };
    if cur_gen == 1 {
        vec![false, badges.is_attack_boosted(), badges.is_defense_boosted(), badges.is_special_attack_boosted(), badges.is_speed_boosted()]
    } else {
        vec![false, badges.is_attack_boosted(), badges.is_defense_boosted(), badges.is_special_attack_boosted(), spd, badges.is_speed_boosted()]
    }
}

pub(crate) fn move_type(gen: &GenData, name: &str) -> Option<String> {
    gen.move_db().get_move(name).map(|m| m.move_type.clone())
}

impl Session {
    pub(super) fn event_call(&mut self, method: &str, p: &Value) -> Result<Value, String> {
        match method {
            "event.detail" => {
                let id = i(p, "id");
                self.event_detail(id)
            }
            "event.state" => {
                // the state view of an arbitrary point (route start / end)
                let gen = self.gen()?;
                let st = if b(p, "end") { self.ctrl.get_final_state() } else { self.ctrl.get_init_state() };
                Ok(self.state_json(&gen, st.as_deref(), None))
            }
            "event.create" => {
                let gen = self.gen()?;
                // a stale id (undo rebuilds the tree) means "at the end"
                let mut after = i(p, "after").filter(|x| self.ctrl.router.contains_id(*x));
                let before = i(p, "before").filter(|x| self.ctrl.router.contains_id(*x));
                let mut folder = opt_s(p, "folder");
                // an empty route has nowhere to put an event: start a folder
                if self.ctrl.is_empty() && folder.is_none() {
                    let name = self.ctrl.unique_folder_name("New Folder");
                    self.ctrl.finalize_new_folder(&name, None, None);
                    folder = Some(name);
                    after = None;
                }
                // a selected folder means "inside it, at the end"
                if let Some(a) = after {
                    if self.ctrl.router.obj_kind(a) == Some(ObjKind::Folder) && b(p, "into_folder") {
                        folder = self.ctrl.router.folder(a).map(|f| f.name.clone());
                        after = None;
                    }
                }
                let st = self.state_at(after, before);
                let draft = p.get("draft").ok_or("draft")?;
                let def = drafts::build(draft, None, &gen, st.as_deref())?;
                let id = self.ctrl.new_event(def, after, before, folder.as_deref(), b(p, "select") || p.get("select").is_none());
                if let Some(id) = id {
                    self.reveal(id);
                }
                Ok(json!({"id": id}))
            }
            "event.update" => {
                let id = i(p, "id").ok_or("id")?;
                let gen = self.gen()?;
                let base = self.def_of(id).ok_or("event not found")?;
                let st = self.ctrl.router.init_state_of(id).cloned();
                let draft = p.get("draft").ok_or("draft")?;
                let mut def = drafts::build(draft, Some(&base), &gen, st.as_deref())?;
                // the battle summary's own choices ride along with the trainer
                if def.trainer_def.is_some() && self.battle_for == Some(id) {
                    if let (Some(partial), Some(td)) = (self.bc.summary.get_partial_trainer_definition(), def.trainer_def.as_mut()) {
                        if partial.trainer_name == td.trainer_name {
                            let keep = (td.exp_split.clone(), td.pay_day_amount, td.mon_order.clone(), td.thief_mons.clone(), td.second_trainer_name.clone());
                            *td = partial;
                            td.exp_split = keep.0;
                            td.pay_day_amount = keep.1;
                            td.mon_order = keep.2;
                            td.thief_mons = keep.3;
                            td.second_trainer_name = keep.4;
                        }
                    }
                }
                if def.serialize() == base.serialize() {
                    return Ok(json!({"changed": false}));
                }
                self.ctrl.update_existing_event(id, def);
                Ok(json!({"changed": true}))
            }
            "event.set_notes" => {
                let id = i(p, "id").ok_or("id")?;
                let mut def = self.def_of(id).ok_or("event not found")?;
                let notes = s(p, "notes").trim().to_string();
                if def.notes == notes {
                    return Ok(json!(false));
                }
                def.notes = notes;
                self.ctrl.update_existing_event(id, def);
                Ok(json!(true))
            }
            "event.assign_move_before" => {
                self.ctrl.select_new_events(vec![i(p, "id").ok_or("id")?]);
                let r = self.ctrl.assign_move_via_tutor_before_selected(i(p, "slot").unwrap_or(0), s(p, "move"));
                Ok(json!({"id": r}))
            }
            "event.use_pp_item_before" => {
                self.ctrl.select_new_events(vec![i(p, "id").ok_or("id")?]);
                let r = self.ctrl.use_pp_item_before_selected(s(p, "item"), opt_s(p, "target").as_deref());
                Ok(json!({"id": r}))
            }
            "event.override_evs_before" => {
                self.ctrl.select_new_events(vec![i(p, "id").ok_or("id")?]);
                let v: Vec<i64> = p.get("evs").and_then(|x| x.as_array()).map(|a| a.iter().map(|x| x.as_i64().unwrap_or(0)).collect()).unwrap_or_default();
                if v.len() != 6 {
                    return Err("six values expected".into());
                }
                let gen = self.gen()?;
                let spd = if gen.get_generation() <= 2 { v[3] } else { v[4] };
                let r = self.ctrl.override_evs_before_selected([v[0], v[1], v[2], v[3], spd, v[5]]);
                Ok(json!({"id": r}))
            }
            "event.add_area" => {
                // every trainer of a location (quick-add "Add Area")
                let after = i(p, "after").filter(|x| self.ctrl.router.contains_id(*x));
                self.ctrl.add_area(s(p, "location"), b(p, "rematches"), after);
                Ok(json!(true))
            }
            "event.add_league" => Ok(json!({"id": self.ctrl.add_league_folder(b(p, "champion"))})),
            "event.add_trainer_quick" => {
                // quick-add "Add Trainer": a selected folder gets a new folder
                // named after the trainer's location
                let gen = self.gen()?;
                let name = s(p, "trainer");
                let t = gen.trainer_db().get_trainer(name).ok_or("Pick a trainer")?.clone();
                let mut draft = json!({"kind": "trainer", "trainer": t.name});
                if let Some(partner) = opt_s(p, "partner").filter(|x| !x.is_empty()) {
                    draft = json!({"kind": "trainer", "trainer": partner, "second_trainer": t.name});
                }
                let def = drafts::build(&draft, None, &gen, None)?;
                let sel = i(p, "after").filter(|x| self.ctrl.router.contains_id(*x));
                let is_folder = sel.map(|id| self.ctrl.router.obj_kind(id) == Some(ObjKind::Folder)).unwrap_or(false);
                let id = if is_folder && p.get("partner").is_none() {
                    let folder = self.ctrl.unique_folder_name(&t.location);
                    self.ctrl.finalize_new_folder(&folder, None, sel);
                    self.ctrl.new_event(def, None, None, Some(&folder), true)
                } else if self.ctrl.is_empty() {
                    let folder = self.ctrl.unique_folder_name(&t.location);
                    self.ctrl.finalize_new_folder(&folder, None, None);
                    self.ctrl.new_event(def, None, None, Some(&folder), true)
                } else {
                    self.ctrl.new_event(def, sel, None, None, true)
                };
                if let Some(id) = id {
                    self.reveal(id);
                }
                Ok(json!({"id": id}))
            }
            _ => Err(format!("unknown method {}", method)),
        }
    }

    fn before_info(&self, id: NodeId, gen: &GenData) -> (String, Option<String>) {
        let router = &self.ctrl.router;
        let folder_name = |fid: Option<NodeId>| fid.filter(|f| *f != router.root_id).and_then(|f| router.folder(f)).map(|f| f.name.clone());
        let enclosing_folder = |mut cur: NodeId| -> Option<String> {
            loop {
                let parent = router.parent_of(cur)?;
                if router.obj_kind(parent) == Some(ObjKind::Folder) {
                    return folder_name(Some(parent));
                }
                cur = parent;
            }
        };
        match router.obj_kind(id) {
            Some(ObjKind::Folder) => {
                let f = router.folder(id);
                (f.map(|f| f.name.clone()).unwrap_or_default(), folder_name(f.and_then(|f| f.parent)))
            }
            Some(kind) => {
                let (def, name) = match kind {
                    ObjKind::Group => router.group(id).map(|g| (g.event_definition.clone(), g.name.clone())),
                    _ => router.item(id).map(|i| (i.event_definition.clone(), i.name.clone())),
                }
                .unwrap_or_default();
                let mut label = def.get_label(gen).unwrap_or(name);
                let mut location = None;
                if def.trainer_def.is_some() {
                    let trainer = def.get_first_trainer_obj(gen).ok().flatten().cloned();
                    for prefix in ["Trainer: ", "Multi: "] {
                        if let Some(rest) = label.strip_prefix(prefix) {
                            label = rest.to_string();
                            break;
                        }
                    }
                    if let Some(t) = &trainer {
                        if !t.location.is_empty() {
                            let suffix = format!(" ({})", t.location);
                            if let Some(rest) = label.strip_suffix(&suffix) {
                                label = rest.to_string();
                            }
                            location = Some(t.location.clone());
                        }
                    }
                }
                if location.is_none() {
                    location = enclosing_folder(id);
                }
                (label, location)
            }
            None => ("Route start".into(), None),
        }
    }

    /// The Pre-Event State view of `st` (identity, stats, moves with PP, bag).
    pub(crate) fn state_json(&self, gen: &GenData, st: Option<&RouteState>, pp_for: Option<NodeId>) -> Value {
        let Some(st) = st else { return Value::Null };
        let solo = &st.solo_pkmn;
        let pkmn = solo.get_pkmn_obj(&st.badges, None);
        let g = gen.get_generation();
        let gen1 = g == 1;
        let names: &[&str] = if gen1 { &["HP", "Attack", "Defense", "Special", "Speed"] } else { &["HP", "Attack", "Defense", "Sp. Atk", "Sp. Def", "Speed"] };
        let now = stat_vals(gen1, &pkmn.cur_stats);
        let boosted = boost_flags(gen, &pkmn, &st.badges);
        let gain = stat_vals(gen1, &solo.get_net_gain_from_stat_xp(&st.badges));
        let realized = stat_vals(gen1, &solo.realized_stat_xp);
        let total = stat_vals(gen1, &solo.unrealized_stat_xp);
        let stats: Vec<Value> = names
            .iter()
            .enumerate()
            .map(|(k, n)| json!({"name": n, "now": now[k], "boosted": boosted[k], "gain": gain[k], "realized": realized[k], "total": total[k]}))
            .collect();
        let snapshot = pp_for.and_then(|id| self.ctrl.pp.before(id).cloned());
        let spend = pp_for.and_then(|id| self.ctrl.pp.spend(id));
        let moves: Vec<Value> = (0..4)
            .map(|slot| {
                let name = solo.move_list.get(slot).cloned().flatten();
                let pp = snapshot.as_ref().and_then(|s| s.slots[slot].clone());
                let history: Vec<Value> = pp_for
                    .map(|id| self.ctrl.pp.history(id, slot).iter().map(|h| json!({"amount": h.amount, "text": h.text})).collect())
                    .unwrap_or_default();
                json!({
                    "slot": slot,
                    "name": name,
                    "type": name.as_deref().and_then(|n| move_type(gen, n)),
                    "pp_cur": pp.as_ref().map(|p| p.cur),
                    "pp_max": pp.as_ref().map(|p| p.max),
                    "pp_ups": pp.as_ref().map(|p| p.pp_ups),
                    "spend": spend.map(|s| s[slot]),
                    "history": history,
                })
            })
            .collect();
        let bag: Vec<Value> = st.inventory.cur_items.iter().map(|b| json!({"name": b.base_item.name, "num": b.num})).collect();
        let pp_items: Vec<Value> = st
            .inventory
            .cur_items
            .iter()
            .filter_map(|b| gen.pp_item_effect(&b.base_item.name).map(|e| json!({"name": b.base_item.name, "needs_target": e.needs_target()})))
            .collect();
        let types = gen.pkmn_db().get_pkmn(&solo.name).map(|sp| {
            let mut t = vec![sp.first_type.clone()];
            if !sp.second_type.is_empty() && sp.second_type != sp.first_type {
                t.push(sp.second_type.clone());
            }
            t
        });
        json!({
            "species": solo.name,
            "types": types,
            "level": solo.cur_level,
            "xp": solo.cur_xp,
            "to_next": solo.xp_to_next_level,
            "pct_to_next": solo.percent_xp_to_next_level,
            "ability": if g >= 3 { Some(solo.ability.clone()) } else { None },
            "nature": if g >= 3 { Some(solo.nature.display_name()) } else { None },
            "held_item": if g >= 2 { Some(solo.held_item.clone().unwrap_or_else(|| "none".into())) } else { None },
            "stats": stats,
            "ev_label": if g <= 2 { "StatExp" } else { "EVs" },
            "badges": if st.badges.num_badges() > 0 { st.badges.to_short_string() } else { "none".into() },
            "badge_list": st.badges.slots_with_state().iter().map(|(n, has)| json!({"name": n, "has": has})).collect::<Vec<_>>(),
            "badge_boost": boosted.iter().any(|b| *b),
            "moves": moves,
            "bag": bag,
            "bag_limit": gen.bag_limit(),
            "money": st.inventory.cur_money,
            "pp_items": pp_items,
            "generation": g,
        })
    }

    fn event_detail(&mut self, id: Option<NodeId>) -> Result<Value, String> {
        let gen = self.gen()?;
        let Some(id) = id.filter(|id| self.ctrl.router.contains_id(*id)) else {
            let st = self.ctrl.get_init_state();
            return Ok(json!({"id": null, "label": "Route start", "state": self.state_json(&gen, st.as_deref(), None)}));
        };
        self.ctrl.ensure_pp(&self.cfg);
        let kind = self.ctrl.router.obj_kind(id).ok_or("event not found")?;
        let def = self.def_of(id).ok_or("event not found")?;
        let st = self.ctrl.router.init_state_of(id).cloned();
        let after = self.ctrl.router.final_state_of(id).cloned();
        let (label, location) = self.before_info(id, &gen);
        let (errors, mut warnings) = match kind {
            ObjKind::Group => self.ctrl.router.group(id).map(|g| (g.error_messages.clone(), g.warning_messages.clone())).unwrap_or_default(),
            ObjKind::Item => self
                .ctrl
                .router
                .item(id)
                .map(|i| (if i.has_errors() { vec![i.error_message.clone()] } else { vec![] }, if i.has_warnings() { vec![i.warning_message.clone()] } else { vec![] }))
                .unwrap_or_default(),
            ObjKind::Folder => (vec![], vec![]),
        };
        let pp_messages = self.ctrl.pp.messages(&self.ctrl.router, id);
        warnings.extend(pp_messages);
        let is_levelup_item = kind == ObjKind::Item && def.learn_move.as_ref().map(|l| l.source == consts::MOVE_SOURCE_LEVELUP).unwrap_or(false);
        let editable = matches!(kind, ObjKind::Group) || is_levelup_item;
        // a fight item resolves to its group for the battle
        let battle_id = if kind == ObjKind::Item && def.learn_move.is_none() { self.ctrl.router.item(id).map(|i| i.parent) } else { Some(id) };
        let battle_def = battle_id.and_then(|b| self.def_of(b));
        let has_battle = battle_def.as_ref().map(|d| d.trainer_def.is_some() || d.wild_pkmn_info.is_some()).unwrap_or(false);
        let team: Vec<Value> = if def.trainer_def.is_some() {
            def.get_pokemon_list(&gen, false).unwrap_or_default().iter().map(|(_, m)| {
                let mut v = mon_json(&gen, m);
                v["exp_split"] = json!(m.exp_split);
                v["mon_order"] = json!(m.mon_order);
                v["definition_order"] = json!(m.definition_order);
                v
            }).collect()
        } else {
            vec![]
        };
        // speed comparisons advance the solo mon through the fight
        let mut speeds: Vec<Value> = Vec::new();
        if let (Some(st), true) = (st.as_ref(), def.trainer_def.is_some()) {
            let mut cur: RouteState = (**st).clone();
            for (_, m) in def.get_pokemon_list(&gen, false).unwrap_or_default() {
                let mine = cur.solo_pkmn.get_pkmn_obj(&cur.badges, None).cur_stats.speed;
                speeds.push(json!(mine.cmp(&m.cur_stats.speed) as i8));
                if let Ok((next, _)) = cur.defeat_pkmn(&gen, &m, None, 1, 0) {
                    cur = next;
                }
            }
        }
        let trainer = def.trainer_def.as_ref().map(|t| self.trainer_json(&gen, &t.trainer_name));
        let second = def.trainer_def.as_ref().and_then(|t| t.second_trainer_name_str().map(|n| self.trainer_json(&gen, n)));
        let bag_order: Option<Vec<String>> = def.bag_reorder.as_ref().and_then(|r| st.as_ref().map(|s| s.inventory.swap_items(&r.swaps).0.cur_items.iter().map(|b| b.base_item.name.clone()).collect()));
        let bag_stale = def.bag_reorder.as_ref().and_then(|r| st.as_ref().map(|s| !s.inventory.swap_items(&r.swaps).1.is_empty())).unwrap_or(false);
        Ok(json!({
            "id": id,
            "kind": match kind { ObjKind::Folder => "folder", ObjKind::Group => "group", ObjKind::Item => if is_levelup_item { "levelup" } else { "item" } },
            "type": if kind == ObjKind::Folder { "Folder" } else { def.get_event_type() },
            "label": label,
            "location": location,
            "title": def.get_label(&gen).unwrap_or_default(),
            "enabled": self.ctrl.router.is_enabled(id),
            "editable": editable,
            "errors": errors,
            "warnings": warnings,
            "notes": def.notes,
            "draft": if kind == ObjKind::Folder { json!({"kind": "folder"}) } else { drafts::describe(&def, &gen) },
            "state": self.state_json(&gen, st.as_deref(), Some(id)),
            "context": self.context_json(&gen, st.as_deref()),
            "after": after.map(|a| json!({"level": a.solo_pkmn.cur_level, "xp": a.solo_pkmn.cur_xp, "money": a.inventory.cur_money})),
            "has_battle": has_battle,
            "battle_id": battle_id,
            "trainer": trainer,
            "second_trainer": second,
            "team": team,
            "speeds": speeds,
            "exp_per_sec": def.experience_per_second(&gen).unwrap_or_default(),
            "mappable": super::map::query_for(&self.ctrl, id).is_some() && super::map::game_of_gen(&gen).is_some(),
            "bag_order": bag_order,
            "bag_stale": bag_stale,
            "folder_name": if kind == ObjKind::Folder { self.ctrl.router.folder(id).map(|f| f.name.clone()) } else { None },
            "can_split": kind != ObjKind::Folder && self.ctrl.router.parent_of(id).map(|p| p != self.ctrl.router.root_id && self.ctrl.router.obj_kind(p) == Some(ObjKind::Folder)).unwrap_or(false),
        }))
    }

    #[allow(dead_code)]
    fn _def(&self, _d: &EventDefinition) {}
}
