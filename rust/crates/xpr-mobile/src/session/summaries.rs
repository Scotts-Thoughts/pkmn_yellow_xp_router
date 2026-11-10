//! Run summary (major fights and rare candies with the moveset at each),
//! setup summary and the notes export text.

use std::collections::HashSet;

use serde_json::{json, Value};

use xpr_core::consts;
use xpr_engine::NodeId;

use super::Session;

impl Session {
    pub(super) fn summaries_call(&mut self, method: &str, _p: &Value) -> Result<Value, String> {
        match method {
            "summary.run" => Ok(self.run_summary()),
            "summary.setup" => Ok(json!(self.setup_summary())),
            "summary.notes" => Ok(json!(self.ctrl.router.export_notes_text()?)),
            _ => Err(format!("unknown method {}", method)),
        }
    }

    /// `RunSummary::refresh` as columns; Swift merges equal neighbours.
    fn run_summary(&self) -> Value {
        let Some(gen) = self.ctrl.gen() else { return json!({"columns": []}) };
        let crystal_excluded = ["Leader Brock", "Leader Misty", "Leader Lt.Surge", "Leader Erika", "Leader Sabrina", "Leader Blaine", "Leader Janine"];
        let heartgold_excluded = [
            "Leader Brock", "Leader Misty", "Leader Lt.Surge", "Leader Erika", "Leader Sabrina", "Leader Blaine", "Leader Janine",
            "Elite Four Will Rematch 2", "Elite Four Koga Rematch 2", "Elite Four Bruno Rematch 2", "Elite Four Karen Rematch 2",
        ];
        let mut cols: Vec<Value> = Vec::new();
        let mut e4_seen: HashSet<String> = HashSet::new();
        let hp_type = self.ctrl.get_dvs().map(|d| gen.get_hidden_power(&d).0).unwrap_or_default();
        let moves_of = |list: &Vec<Option<String>>| -> Vec<Value> {
            (0..4)
                .map(|k| {
                    let name = list.get(k).cloned().flatten().unwrap_or_default();
                    if name.is_empty() {
                        json!({"name": "", "type": ""})
                    } else if name == consts::HIDDEN_POWER_MOVE_NAME {
                        json!({"name": format!("{} ({})", name, hp_type), "type": hp_type})
                    } else {
                        json!({"name": name, "type": gen.move_db().get_move(&name).map(|m| m.move_type.clone()).unwrap_or_default()})
                    }
                })
                .collect()
        };
        let mut cur: Option<NodeId> = self.ctrl.get_next_event(None);
        while let Some(gid) = cur {
            if let Some(g) = self.ctrl.router.group(gid) {
                let ed = &g.event_definition;
                if let (Some(td), true) = (&ed.trainer_def, ed.is_enabled_flag()) {
                    let name = td.trainer_name.clone();
                    let is_e4 = name.starts_with("Elite Four ");
                    let skip = ed.is_highlighted()
                        || (gen.version_name() == consts::CRYSTAL_VERSION && crystal_excluded.contains(&name.as_str()))
                        || (gen.version_name() == consts::HEART_GOLD_VERSION && heartgold_excluded.contains(&name.as_str()))
                        || (is_e4 && e4_seen.contains(&name));
                    if !skip {
                        if is_e4 {
                            e4_seen.insert(name.clone());
                        }
                        if gen.is_major_fight(&name) {
                            if let Some(init) = &g.init_state {
                                cols.push(json!({
                                    "id": gid, "title": name, "candy": false, "level": format!("Lv {}", init.solo_pkmn.cur_level),
                                    "held": init.solo_pkmn.held_item.clone().unwrap_or_else(|| "None".into()),
                                    "moves": moves_of(&init.solo_pkmn.move_list),
                                    "category": gen.get_fight_category(&name),
                                }));
                            }
                        }
                    }
                } else if let (Some(rc), true) = (&ed.rare_candy, ed.is_enabled_flag()) {
                    if rc.amount > 0 {
                        if let Some(fin) = &g.final_state {
                            cols.push(json!({
                                "id": gid, "title": format!("Rare Candy ×{}", rc.amount), "candy": true,
                                "level": format!("Lv {}→{}", fin.solo_pkmn.cur_level - rc.amount, fin.solo_pkmn.cur_level),
                                "held": fin.solo_pkmn.held_item.clone().unwrap_or_else(|| "None".into()),
                                "moves": moves_of(&fin.solo_pkmn.move_list),
                            }));
                        }
                    }
                }
            }
            cur = self.ctrl.get_next_event(Some(gid));
        }
        json!({"columns": cols, "show_held": gen.get_generation() != 1})
    }

    fn setup_summary(&self) -> String {
        let Some(gen) = self.ctrl.gen() else { return "No setup moves used".into() };
        let mut used: Vec<String> = Vec::new();
        let mut cur = self.ctrl.get_next_event(None);
        while let Some(gid) = cur {
            if let Some(g) = self.ctrl.router.group(gid) {
                let ed = &g.event_definition;
                if let Some(td) = &ed.trainer_def {
                    if ed.is_enabled_flag() && (ed.is_highlighted() || gen.is_major_fight(&td.trainer_name)) && !td.setup_moves.is_empty() {
                        let mut counts: Vec<(String, i64)> = Vec::new();
                        for sm in &td.setup_moves {
                            match counts.iter_mut().find(|(m, _)| m == sm) {
                                Some(e) => e.1 += 1,
                                None => counts.push((sm.clone(), 1)),
                            }
                        }
                        let text: Vec<String> = counts.iter().map(|(m, c)| format!("x{} {}", c, m)).collect();
                        used.push(format!("{}: {}", ed.get_label(&gen).unwrap_or_default(), text.join(",")));
                    }
                }
            }
            cur = self.ctrl.get_next_event(Some(gid));
        }
        if used.is_empty() {
            "No setup moves used".into()
        } else {
            let mut lines = vec!["Setup Moves:".to_string()];
            lines.extend(used);
            lines.join("\n")
        }
    }
}
