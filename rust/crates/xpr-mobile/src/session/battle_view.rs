//! The battle summary: loading it for an event, the matchup / move view
//! models (formatted exactly like the desktop columns) and every control.

use serde_json::{json, Value};

use xpr_calc::battle_summary::{BattleSummary, MoveRenderInfo};
use xpr_core::consts;
use xpr_engine::{NodeId, ObjKind};

use super::{b, i, s, strs, Session};

/// Python `round(x)` (ties to even).
fn py_round(x: f64) -> i64 {
    if (x - x.trunc()).abs() == 0.5 {
        let t = x.trunc() as i64;
        if t % 2 == 0 {
            t
        } else {
            t + x.signum() as i64
        }
    } else {
        x.round() as i64
    }
}

fn shown_kill_ranges(m: &MoveRenderInfo) -> Vec<(i64, f64)> {
    let mut k: Vec<(i64, f64)> = m.kill_ranges.iter().map(|r| (r.0, r.1)).collect();
    if k.len() > 3 {
        let last = *k.last().unwrap();
        k.truncate(2);
        k.push(last);
    }
    k
}

fn format_kill(ignore_acc: bool, (n, pct): (i64, f64)) -> (String, String) {
    if pct == -1.0 {
        if ignore_acc {
            return (format!("{}-hit kill", n), "100 %".into());
        }
        return (format!("{}-hit kill, IGNORING ACC", n), String::new());
    }
    let r1 = (pct * 10.0).round() / 10.0;
    let rendered = if r1 == (pct as i64) as f64 { format!("{}", pct as i64) } else { format!("{:.1}", pct) };
    if ignore_acc {
        (format!("{}-hit kill", n), format!("{} %", rendered))
    } else {
        (format!("{}-turn kill", n), format!("{} %", rendered))
    }
}

fn recoil_line(m: &MoveRenderInfo) -> Option<(String, String)> {
    if m.min_damage == -1 {
        return None;
    }
    let attacker_hp = m.attacking_mon_hp.max(0);
    let mut max_hp_divisor = None;
    let mut damage_divisor = None;
    for flavor in &m.attack_flavor {
        if let Some((_, d)) = consts::RECOIL_MAX_HP_FLAVOR_DIVISORS.iter().find(|(f, _)| f == flavor) {
            max_hp_divisor = Some(*d);
            break;
        }
        if let Some((_, d)) = consts::RECOIL_FLAVOR_DIVISORS.iter().find(|(f, _)| f == flavor) {
            damage_divisor = Some(*d);
            break;
        }
    }
    let (lo, hi) = if let (Some(d), true) = (max_hp_divisor, attacker_hp > 0) {
        let v = (attacker_hp / d).max(1);
        (v, v)
    } else if let Some(d) = damage_divisor {
        let hp = m.defending_mon_hp;
        ((m.min_damage.min(hp) / d).max(1), (m.max_damage.min(hp) / d).max(1))
    } else {
        return None;
    };
    let pct = |v: i64| py_round(v as f64 / attacker_hp as f64 * 100.0);
    if lo == hi {
        Some((format!("Recoil: {}", lo), if attacker_hp > 0 { format!("{} %", pct(lo)) } else { String::new() }))
    } else {
        Some((format!("Recoil: {} - {}", lo, hi), if attacker_hp > 0 { format!("{} - {} %", pct(lo), pct(hi)) } else { String::new() }))
    }
}

fn span(lo: i64, hi: i64, suffix: &str) -> String {
    if lo == hi {
        format!("{}{}", lo, suffix)
    } else {
        format!("{} - {}{}", lo, hi, suffix)
    }
}

const VITAMIN_STATS: [(&str, &str); 6] = [(consts::HP, "HP"), (consts::ATK, "Atk"), (consts::DEF, "Def"), (consts::SPA, "SpA"), (consts::SPD, "SpD"), (consts::SPE, "Spe")];

impl Session {
    /// The event whose battle shows for `id` (a fight item shows its group's).
    fn battle_target(&self, id: NodeId) -> NodeId {
        if self.ctrl.router.obj_kind(id) == Some(ObjKind::Item) {
            if let Some(it) = self.ctrl.router.item(id) {
                if it.event_definition.learn_move.is_none() {
                    return it.parent;
                }
            }
        }
        id
    }

    /// `load_team_for`: a trainer group loads from the event (editable), a
    /// trainer item or wild encounter from its state (preview).
    pub(crate) fn reload_battle(&mut self) {
        let Some(id) = self.battle_for else { return };
        if !self.ctrl.router.contains_id(id) {
            self.battle_for = None;
            self.bc.load_empty(&self.cfg, &self.ctrl);
            return;
        }
        let kind = self.ctrl.router.obj_kind(id);
        let Some(def) = self.def_of(id) else { return };
        let init = self.ctrl.router.init_state_of(id).cloned();
        let gen = self.ctrl.gen();
        if def.trainer_def.is_some() {
            if kind == Some(ObjKind::Group) {
                self.bc.load_from_event(&self.cfg, &mut self.ctrl, id);
            } else if let (Some(st), Some(g)) = (init, gen) {
                let mons = def.pokemon_list(&g).unwrap_or_default();
                self.bc.load_from_state(&self.cfg, &mut self.ctrl, &st, &mons, false);
            }
        } else if def.wild_pkmn_info.is_some() {
            let wild = gen.and_then(|g| def.pokemon_list(&g).ok()).unwrap_or_default();
            match init {
                Some(st) if !wild.is_empty() => self.bc.load_from_state(&self.cfg, &mut self.ctrl, &st, &wild, true),
                _ => self.bc.load_empty(&self.cfg, &self.ctrl),
            }
        } else {
            self.bc.load_empty(&self.cfg, &self.ctrl);
        }
        let _ = self.bc.take_signals();
    }

    /// Write the summary's setup choices into the trainer event (the
    /// desktop's delayed save), keeping the editor-owned fields.
    pub(crate) fn persist_battle(&mut self) {
        let Some(gid) = self.bc.summary.event_group_id else { return };
        let Some(base) = self.ctrl.router.group(gid).map(|g| g.event_definition.clone()) else { return };
        let Some(old_td) = base.trainer_def.clone() else { return };
        let Some(mut td) = self.bc.summary.get_partial_trainer_definition() else { return };
        if td.trainer_name != old_td.trainer_name {
            return;
        }
        td.exp_split = old_td.exp_split.clone();
        td.pay_day_amount = old_td.pay_day_amount;
        td.mon_order = old_td.mon_order.clone();
        td.thief_mons = old_td.thief_mons.clone();
        td.second_trainer_name = old_td.second_trainer_name.clone();
        td.verbose_export = old_td.verbose_export.clone();
        let mut def = base.clone();
        def.trainer_def = Some(td);
        if def.serialize() == base.serialize() {
            return;
        }
        self.ctrl.update_existing_event(gid, def);
        // the summary already shows this state: the reload this route change
        // would trigger is a no-op, so mark it handled
        self.bc.mark_handled_route_change(gid);
    }

    pub(super) fn battle_call(&mut self, method: &str, p: &Value) -> Result<Value, String> {
        match method {
            "battle.load" => {
                let id = i(p, "id").ok_or("id")?;
                let target = self.battle_target(id);
                self.battle_for = Some(target);
                self.reload_battle();
                self.battle_view()
            }
            "battle.view" => self.battle_view(),
            "battle.unload" => {
                self.battle_for = None;
                Ok(json!(true))
            }
            "battle.action" => {
                self.battle_action(p)?;
                // drain now so the view reflects persisted / reloaded state
                self.pump_battle();
                self.battle_view()
            }
            "battle.held_items" => {
                let gen = self.gen()?;
                Ok(json!(BattleSummary::get_held_item_options(&gen)))
            }
            _ => Err(format!("unknown method {}", method)),
        }
    }

    fn pump_battle(&mut self) {
        let bs = self.bc.take_signals();
        if bs.nonload_change {
            self.persist_battle();
        }
        let sig = self.ctrl.take_signals();
        if sig.route_changed {
            if let Some(id) = self.battle_for {
                if !self.bc.take_handled_route_change(id) {
                    self.reload_battle();
                }
            }
        }
        // keep the other signals for the general pump
        if sig.message || sig.exception {
            while let Some(m) = self.ctrl.get_next_message_info() {
                self.toast(m);
            }
            while let Some(e) = self.ctrl.get_next_exception_info() {
                self.messages.push(json!({"kind": "error", "text": e.replace("<class 'Exception'>: ", "")}));
            }
        }
    }

    fn battle_action(&mut self, p: &Value) -> Result<(), String> {
        let mon = i(p, "mon").unwrap_or(0);
        let mv = i(p, "move").unwrap_or(0);
        let player = b(p, "player");
        let cfg = &self.cfg;
        match s(p, "op") {
            "collapse" => {
                self.bc.update_mon_collapsed(mon, b(p, "collapsed"));
            }
            "highlight" => self.bc.update_move_highlight(mon, mv, true, b(p, "reset")),
            "intimidate" => self.bc.toggle_intimidate(cfg, &self.ctrl, mon, player, b(p, "enabled")),
            "weather_move" => self.bc.toggle_weather_from_move(cfg, &self.ctrl, s(p, "name"), b(p, "enabled"), Some(mon)),
            "screen_move" => self.bc.toggle_screen_from_move(cfg, &self.ctrl, s(p, "name"), b(p, "enabled"), mon, player),
            "custom_data" => self.bc.update_custom_move_data(cfg, &self.ctrl, mon as usize, mv as usize, player, s(p, "value")),
            "mimic" => self.bc.update_mimic_selection(cfg, &self.ctrl, s(p, "value")),
            "stat_stage" => self.bc.update_stat_stage_setup(cfg, &self.ctrl, mon as usize, mv as usize, player, s(p, "value")),
            "test_move" => self.bc.update_test_move(cfg, &mut self.ctrl, i(p, "slot").unwrap_or(0) as usize, s(p, "value")),
            "candies" => {
                let n = i(p, "value").unwrap_or(0).max(0);
                self.bc.update_prefight_candies(cfg, &mut self.ctrl, n, b(p, "coalesce"));
            }
            "vitamin" => self.bc.adjust_vitamin_for_stat(cfg, &mut self.ctrl, s(p, "stat"), i(p, "delta").unwrap_or(0), false),
            "held_item" => self.bc.update_player_held_item(cfg, &mut self.ctrl, s(p, "value")),
            "assign_move" => self.bc.assign_player_move_via_tutor(cfg, &mut self.ctrl, i(p, "slot").unwrap_or(0), s(p, "value")),
            "reorder" => {
                let (from, to) = (i(p, "from").unwrap_or(0) as usize, i(p, "to").unwrap_or(0) as usize);
                if let (Some(gid), Ok(Some(def))) = (self.bc.summary.event_group_id, self.bc.summary.reorder_matchup(&self.ctrl.router, from, to)) {
                    self.ctrl.update_existing_event(gid, def);
                }
            }
            "player_setup" => self.bc.update_player_setup_moves(cfg, &self.ctrl, strs(p, "moves")),
            "enemy_setup" => self.bc.update_enemy_setup_moves(cfg, &self.ctrl, strs(p, "moves")),
            "transform" => self.bc.update_player_transform(cfg, &self.ctrl, b(p, "enabled")),
            "weather" => self.bc.update_weather(cfg, &self.ctrl, s(p, "value"), None),
            "refresh" => self.bc.full_refresh(cfg, &self.ctrl),
            other => return Err(format!("unknown battle op {}", other)),
        }
        Ok(())
    }

    fn move_json(&self, m: Option<&MoveRenderInfo>, mon: usize, move_idx: usize, is_player: bool, is_test: bool) -> Value {
        let Some(m) = m else { return Value::Null };
        let summary = &self.bc.summary;
        let gen = self.ctrl.gen();
        let show_hl = self.cfg.get_show_move_highlights();
        let fade_enabled = self.cfg.get_fade_moves_without_highlight() && show_hl;
        let highlight = if is_player && show_hl && !is_test { summary.get_move_highlight_state(mon as i64, move_idx as i64, true) } else { 0 };
        let faded = (is_player && !is_test && fade_enabled && highlight == 0) || (!is_player && fade_enabled && !m.is_best_move);
        let is_mimic = m.name == consts::MIMIC_MOVE_NAME;
        let is_metronome = m.metronome_selection.is_some();
        let (custom_kind, options, selection): (&str, Option<Vec<String>>, Option<String>) = if is_mimic {
            ("mimic", Some(m.mimic_options.clone()), Some(m.mimic_data.clone()))
        } else if is_metronome {
            ("metronome", m.metronome_options.as_ref().map(|o| o.as_ref().clone()), m.metronome_selection.clone())
        } else {
            ("custom", m.custom_data_options.clone(), m.custom_data_selection.clone().or_else(|| m.custom_data_options.as_ref().and_then(|o| o.first().cloned())))
        };
        let show_custom = !is_test && (is_metronome || options.as_ref().map(|o| !o.is_empty()).unwrap_or(false)) && !(faded && is_player);
        let has_global = if is_player { !summary.player_setup_move_list.is_empty() } else { !summary.enemy_setup_move_list.is_empty() };
        let stage = if !has_global && !is_test { m.stat_stage_options.clone().filter(|o| !o.is_empty()) } else { None };
        let resolved = if is_mimic { Some(m.mimic_data.clone()) } else if let Some(c) = m.metronome_selection.as_ref().filter(|x| !x.is_empty()) { Some(c.clone()) } else { Some(m.name.clone()) };
        let weather = resolved.as_deref().and_then(|n| gen.as_ref().and_then(|g| BattleSummary::get_weather_for_move(g, n)));
        let screen = resolved.as_deref().and_then(BattleSummary::get_screen_for_move);
        let weather_active = weather.map(|w| summary.get_weather() == w && summary.weather_source_mon_idx == Some(mon as i64)).unwrap_or(false);
        let screen_active = screen.map(|sc| summary.get_screen_source_mon_idx(is_player, sc) == Some(mon as i64)).unwrap_or(false);
        let ignore_acc = self.cfg.do_ignore_accuracy();
        let damaging = m.min_damage != -1;
        let hp = m.defending_mon_hp.max(1) as f64;
        let pct = |v: i64| py_round(v as f64 / hp * 100.0);
        let kills: Vec<Value> = if damaging {
            shown_kill_ranges(m).into_iter().map(|k| {
                let (t, p) = format_kill(ignore_acc, k);
                json!({"text": t, "pct": p, "turns": k.0, "chance": k.1})
            }).collect()
        } else {
            vec![]
        };
        let recoil = recoil_line(m).map(|(t, p)| json!({"text": t, "pct": p}));
        json!({
            "slot": move_idx,
            "name": m.name,
            "type": gen.as_ref().and_then(|g| g.move_db().get_move(resolved.as_deref().unwrap_or(&m.name)).or_else(|| g.move_db().get_move(&m.name)).map(|x| x.move_type.clone())),
            "damaging": damaging,
            "range": if damaging { Some(span(m.min_damage, m.max_damage, "")) } else { None },
            "range_pct": if damaging { Some(span(pct(m.min_damage), pct(m.max_damage), "%")) } else { None },
            "crit": if damaging { Some(span(m.crit_min_damage, m.crit_max_damage, "")) } else { None },
            "crit_pct": if damaging { Some(span(pct(m.crit_min_damage), pct(m.crit_max_damage), "%")) } else { None },
            "kills": kills,
            "recoil": recoil,
            "best": m.is_best_move && !(fade_enabled && is_player),
            "highlight": highlight,
            "faded": faded,
            "custom": if show_custom { json!({"kind": custom_kind, "options": options, "selection": selection}) } else { Value::Null },
            "stage": stage.map(|o| json!({"options": o, "selection": if m.stat_stage_selection.is_empty() { "0".to_string() } else { m.stat_stage_selection.clone() }})),
            "weather": weather.map(|w| json!({"name": w, "move": resolved, "active": weather_active})),
            "screen": screen.map(|sc| json!({"name": sc, "move": resolved, "active": screen_active})),
            "test": is_test,
        })
    }

    fn battle_view(&mut self) -> Result<Value, String> {
        let gen = self.gen()?;
        let summary = &self.bc.summary;
        let n = summary.num_matchups().min(6);
        let test_enabled = self.cfg.get_test_moves_enabled();
        let mut matchups = Vec::new();
        for mon in 0..n {
            let pl = self.bc.get_pkmn_info(mon, true).cloned();
            let en = self.bc.get_pkmn_info(mon, false).cloned();
            if pl.is_none() && en.is_none() {
                continue;
            }
            let player_moves: Vec<Value> = (0..4).map(|k| self.move_json(self.bc.get_move_info(&self.cfg, &self.ctrl, mon, k, true).as_ref(), mon, k, true, false)).collect();
            let enemy_moves: Vec<Value> = (0..4).map(|k| self.move_json(self.bc.get_move_info(&self.cfg, &self.ctrl, mon, k, false).as_ref(), mon, k, false, false)).collect();
            let test_moves: Vec<Value> = if test_enabled {
                (4..8).map(|k| self.move_json(self.bc.get_move_info(&self.cfg, &self.ctrl, mon, k, true).as_ref(), mon, k, true, true)).collect()
            } else {
                vec![]
            };
            let speed_cmp = pl.as_ref().map(|p| p.attacking_mon_speed.cmp(&p.defending_mon_speed) as i8);
            let s = &self.bc.summary;
            let intim = |player: bool| {
                if s.pokemon_has_intimidate(mon as i64, player) {
                    json!({"active": s.is_intimidate_active(mon as i64, player), "blocked": s.is_intimidate_blocked(mon as i64, player)})
                } else {
                    Value::Null
                }
            };
            let enemy_mon = s.original_enemy_mon_list.get(mon).or_else(|| s.wild_min_dv_mons.get(mon));
            matchups.push(json!({
                "idx": mon,
                "collapsed": s.is_mon_collapsed(mon as i64),
                "player": pl.as_ref().map(|p| json!({"name": p.attacking_mon_name, "level": p.attacking_mon_level, "speed": p.attacking_mon_speed})),
                "enemy": en.as_ref().map(|e| json!({"name": e.attacking_mon_name, "level": e.attacking_mon_level, "speed": e.attacking_mon_speed, "hp": pl.as_ref().map(|p| p.defending_mon_hp)})),
                "enemy_detail": enemy_mon.map(|m| super::options::mon_json(&gen, m)),
                "speed_cmp": speed_cmp,
                "player_intimidate": intim(true),
                "enemy_intimidate": intim(false),
                "download": if mon == 0 { s.player_download_stat() } else { None },
                "player_moves": player_moves,
                "enemy_moves": enemy_moves,
                "test_moves": test_moves,
            }));
        }
        let can_edit = self.bc.can_support_prefight_candies();
        let vits = self.bc.get_vitamins_used_per_stat(&self.ctrl);
        let vit_rows: Vec<Value> = VITAMIN_STATS
            .iter()
            .filter(|(st, _)| gen.get_valid_vitamins().iter().any(|v| gen.get_stats_boosted_by_vitamin(v).map(|b| b.contains(st)).unwrap_or(false)))
            .map(|(st, label)| json!({"stat": st, "label": label, "count": vits.get(st).copied().unwrap_or(0)}))
            .collect();
        let s = &self.bc.summary;
        Ok(json!({
            "loaded": n > 0,
            "event_id": self.battle_for,
            "can_edit": can_edit,
            "wild": s.is_wild_battle,
            "double": s.is_double_battle(),
            "trainer": s.trainer_name,
            "hp": s.get_player_battle_hp(),
            "speed": s.get_player_battle_speed(),
            "candies": self.bc.get_prefight_candy_count(&self.ctrl),
            "vitamins": vit_rows,
            "held_item": self.bc.get_player_held_item(),
            "has_held_items": gen.get_generation() >= 2,
            "weather": s.get_weather(),
            "weather_options": gen.get_valid_weather(),
            "player_setup": s.player_setup_move_list,
            "enemy_setup": s.enemy_setup_move_list,
            "setup_options": gen.get_stat_modifer_moves(),
            "transformed": s.is_player_transformed,
            "test_moves_enabled": test_enabled,
            "test_moves": self.bc.get_test_moves(&self.ctrl),
            "show_legacy": self.cfg.get_show_legacy_controls(),
            "show_highlights": self.cfg.get_show_move_highlights(),
            "fade": self.cfg.get_fade_moves_without_highlight(),
            "player_strategy": self.cfg.get_player_highlight_strategy(),
            "enemy_strategy": self.cfg.get_enemy_highlight_strategy(),
            "ignore_accuracy": self.cfg.do_ignore_accuracy(),
            "matchups": matchups,
        }))
    }
}
