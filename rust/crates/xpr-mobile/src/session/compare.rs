//! Route comparison (the desktop Compare page): load two routes (saved,
//! the open one from memory, or a file), run the engine's `compare`, and
//! hand back the Overview / Checkpoints / Event diff data.

use std::path::PathBuf;

use serde_json::{json, Value};

use xpr_core::io_utils;
use xpr_engine::compare::{format_delta, format_time, text_summary, DiffFilter, DiffRow, Entry, ItemStatus, RouteDigest, Snapshot};
use xpr_engine::{RouteOrigin, Router};

use super::{opt_s, Session};

fn snap(s: &Snapshot, generation: u8) -> Value {
    json!({"level": s.level, "xp": s.xp, "stats": s.stats, "evs": s.evs, "ev_total": s.ev_total(generation), "money": s.money,
           "moves": s.moves, "held_item": s.held_item, "badges": s.badge_count})
}

fn entry(e: &Entry, generation: u8) -> Value {
    json!({"kind": e.kind.caption(), "label": e.label, "qty": e.qty, "folder": e.folder, "location": e.location,
           "before": snap(&e.before, generation), "xp_gain": e.xp_gain, "money_delta": e.money_delta,
           "time": e.recorded_secs.map(format_time), "secs": e.recorded_secs, "major": e.is_major, "category": e.fight_category, "error": e.has_error})
}

fn digest_json(d: &RouteDigest) -> Value {
    let t = &d.totals;
    let origin = match &d.origin {
        RouteOrigin::CurrentRoute => "current route".to_string(),
        RouteOrigin::External => "external file".to_string(),
        RouteOrigin::Saved { mtime } => chrono::DateTime::from_timestamp(*mtime as i64, 0)
            .map(|x| format!("saved {}", x.with_timezone(&chrono::Local).format("%Y-%m-%d")))
            .unwrap_or_else(|| "saved".into()),
    };
    let vit_text = t.vitamins_by_name.iter().map(|(n, c)| format!("{} ×{}", n, c)).collect::<Vec<_>>().join(", ");
    json!({
        "label": d.label, "origin": origin, "version": d.version, "generation": d.generation, "species": d.species,
        "dvs": d.dvs, "dv_total": d.dv_total(), "dv_text": d.dv_text(), "ev_text": d.ev_text(),
        "nature": d.nature, "nature_up": d.nature_up, "nature_down": d.nature_down, "ability": d.ability,
        "hidden_power": d.hidden_power.as_ref().map(|(t, p)| json!({"type": t, "power": p})),
        "start": snap(&d.start, d.generation), "end": snap(&d.end, d.generation),
        "event_count": d.event_count(), "folder_count": d.folder_count, "disabled_events": d.disabled_events,
        "final_time": d.final_time.map(format_time), "final_secs": d.final_time,
        "xp": {"trainers": t.xp_from_trainers, "wild": t.xp_from_wild, "candies": t.xp_from_candies, "other": t.xp_from_other, "total": t.xp_total()},
        "vitamins_text": vit_text,
        "moves_learned": d.moves_learned.iter().map(|m| json!({"name": m.name, "source": m.source.short_label(), "level": m.level})).collect::<Vec<_>>(),
        "totals": [
            ["Battles", "Trainers fought", t.trainers], ["Battles", "of which major fights", t.major_fights],
            ["Battles", "Wild Pokémon fought", t.wild_pokemon], ["Battles", "different species", t.wild_species],
            ["Battles", "trainer-owned singles", t.trainer_owned_singles],
            ["Levelling", "Final level", d.end.level], ["Levelling", "Rare Candies used", t.rare_candies],
            ["Levelling", "Vitamins used", t.vitamins], ["Levelling", "EV berries used", t.ev_berries],
            ["Moves", "Moves learned", t.moves_learned()], ["Moves", "by level-up", t.moves_level_up],
            ["Moves", "by TM / HM", t.moves_tm_hm], ["Moves", "by tutor or deleter", t.moves_tutor],
            ["Items", "Items picked up", t.items_picked_up], ["Items", "Purchases", t.purchases], ["Items", "Sales", t.sales],
            ["Items", "Items used or dropped", t.items_used], ["Items", "Held item changes", t.held_item_changes],
            ["Other", "Pokémon Center heals", t.heals], ["Other", "Saves", t.saves], ["Other", "Blackouts", t.blackouts],
            ["Other", "Evolutions", t.evolutions], ["Other", "Events with errors", t.events_with_errors],
        ],
        "money": [
            ["Won from trainers", t.money_from_trainers], ["Sales", t.money_from_sales], ["Purchases", t.money_spent],
            ["Lost to blackouts", t.money_lost_blackouts], ["Other", t.money_other],
        ],
    })
}

fn status_json(s: &ItemStatus) -> Value {
    match s {
        ItemStatus::InBoth => json!({"status": "both"}),
        ItemStatus::QtyDiffers { other_qty } => json!({"status": "qty", "other_qty": other_qty}),
        ItemStatus::OnlyHere => json!({"status": "only"}),
        ItemStatus::TrainerMoved { other_after } => json!({"status": "moved", "other_after": other_after}),
        ItemStatus::TrainerRepeat => json!({"status": "repeat"}),
    }
}

impl Session {
    fn load_digest(&self, side: &Value) -> Result<RouteDigest, String> {
        let mut router = Router::new(self.registry.clone());
        if side.get("current").and_then(|v| v.as_bool()).unwrap_or(false) {
            let v = self.ctrl.router.serialize()?;
            router.load_value(&v, false)?;
            let label = if self.ctrl.get_current_route_name().is_empty() { "Open route".to_string() } else { self.ctrl.get_current_route_name().to_string() };
            return Ok(xpr_engine::digest(&router, &label, RouteOrigin::CurrentRoute));
        }
        let path: PathBuf = match opt_s(side, "path") {
            Some(p) => PathBuf::from(p),
            None => io_utils::get_existing_route_path(&self.paths, &opt_s(side, "name").ok_or("pick a route")?),
        };
        router.load(&path, false)?;
        let label = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let origin = if path.starts_with(&self.paths.saved_routes_dir) {
            std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| RouteOrigin::Saved { mtime: d.as_secs_f64() })
                .unwrap_or(RouteOrigin::External)
        } else {
            RouteOrigin::External
        };
        Ok(xpr_engine::digest(&router, &label, origin))
    }

    pub(super) fn compare_call(&mut self, method: &str, p: &Value) -> Result<Value, String> {
        match method {
            "compare.run" => {
                let a = self.load_digest(p.get("a").ok_or("route A")?).map_err(|e| format!("Could not load route A: {}", e))?;
                let b = self.load_digest(p.get("b").ok_or("route B")?).map_err(|e| format!("Could not load route B: {}", e))?;
                let same = p["a"] == p["b"];
                let mut cmp = xpr_engine::compare(a, b);
                cmp.compat.same_file = same;
                let v = self.comparison_json(&cmp);
                self.comparison = Some(cmp);
                Ok(v)
            }
            "compare.summary_text" => Ok(json!(self.comparison.as_ref().map(text_summary).unwrap_or_default())),
            _ => Err(format!("unknown method {}", method)),
        }
    }

    fn comparison_json(&self, c: &xpr_engine::RouteComparison) -> Value {
        let (ga, gb) = (c.a.generation, c.b.generation);
        let rows: Vec<Value> = c
            .rows
            .iter()
            .map(|r| match r {
                DiffRow::Anchor { a, b } => {
                    let (ea, eb) = (&c.a.entries[*a], &c.b.entries[*b]);
                    let delta = match (ea.recorded_secs, eb.recorded_secs) {
                        (Some(x), Some(y)) if (y - x).abs() >= 0.05 => Some(format_delta(y - x)),
                        _ => None,
                    };
                    json!({"anchor": true, "a": entry(ea, ga), "b": entry(eb, gb), "time_delta": delta})
                }
                DiffRow::Block { a, b } => {
                    let lane = |items: &Vec<xpr_engine::compare::BlockItem>| -> Vec<Value> {
                        items
                            .iter()
                            .map(|it| {
                                let mut v = status_json(&it.status);
                                v["label"] = json!(it.merged_label);
                                v["qty"] = json!(it.qty);
                                v["kind"] = json!(it.kind.caption());
                                v["filter"] = json!(it.kind.filter().label());
                                v
                            })
                            .collect()
                    };
                    json!({"anchor": false, "a": lane(a), "b": lane(b)})
                }
            })
            .collect();
        let checkpoints: Vec<Value> = c
            .checkpoints
            .iter()
            .map(|cp| {
                let (ea, eb) = (&c.a.entries[cp.a], &c.b.entries[cp.b]);
                let delta = match (ea.recorded_secs, eb.recorded_secs) {
                    (Some(x), Some(y)) => Some(format_delta(x - y)),
                    _ => None,
                };
                json!({"a": entry(ea, ga), "b": entry(eb, gb), "same_order": cp.same_order, "moves_differing": cp.moves_differing, "time_delta": delta, "b_ahead": ea.recorded_secs.zip(eb.recorded_secs).map(|(x, y)| y < x)})
            })
            .collect();
        let ts = &c.trainer_sets;
        let time_delta = match (c.a.final_time, c.b.final_time) {
            (Some(x), Some(y)) if (y - x).abs() >= 0.05 => Some(json!({"text": format_time((y - x).abs()), "faster": y < x})),
            _ => None,
        };
        json!({
            "a": digest_json(&c.a),
            "b": digest_json(&c.b),
            "compat": {"same_version": c.compat.same_version, "same_generation": c.compat.same_generation, "same_species": c.compat.same_species, "same_file": c.compat.same_file},
            "rows": rows,
            "checkpoints": checkpoints,
            "trainer_sets": {"same_order": ts.same_order, "different_order": ts.different_order, "only_a": ts.only_a, "only_b": ts.only_b, "repeat_a": ts.repeat_a, "repeat_b": ts.repeat_b},
            "both_have_times": c.both_have_times,
            "final_time_delta": time_delta,
            "filters": DiffFilter::ALL.iter().map(|f| json!({"label": f.label(), "on": f.default_on()})).collect::<Vec<_>>(),
            "shared_fights": c.shared_fight_count(),
            "blocks_with_differences": c.blocks_with_differences(),
            "errors": [c.a.totals.events_with_errors, c.b.totals.events_with_errors],
        })
    }
}
