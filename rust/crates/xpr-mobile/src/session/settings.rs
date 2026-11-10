//! Settings (the desktop's App Config, Battle Config, highlight colours and
//! Battle Summary menu toggles) and custom gens.

use serde_json::{json, Value};

use xpr_core::consts;

use super::{b, i, s, Session};

const CATEGORY_LABELS: [(&str, &str); 7] = [
    ("rival", "Rival"),
    ("gym_leader", "Gym Leader"),
    ("elite_four", "Elite Four"),
    ("champion", "Champion"),
    ("post_game", "Post-Game"),
    ("boss", "Boss"),
    ("team_leader", "Team Leader"),
];

impl Session {
    fn settings_json(&self) -> Value {
        let c = &self.cfg;
        json!({
            "damage_search_depth": c.get_damage_search_depth(),
            "ignore_accuracy": c.do_ignore_accuracy(),
            "force_full_search": c.do_force_full_search(),
            "player_strategy": c.get_player_highlight_strategy(),
            "enemy_strategy": c.get_enemy_highlight_strategy(),
            "consistent_threshold": c.get_consistent_threshold(),
            "strategies": consts::ALL_HIGHLIGHT_STRATS,
            "show_move_highlights": c.get_show_move_highlights(),
            "fade_moves": c.get_fade_moves_without_highlight(),
            "test_moves": c.get_test_moves_enabled(),
            "show_legacy": c.get_show_legacy_controls(),
            "notes_visibility": c.get_notes_visibility_mode(),
            "auto_switch": c.do_auto_switch(),
            "auto_load": c.get_auto_load_most_recent_route(),
            "color_major_battles": c.get_color_major_battles(),
            "fade_folder_text": c.get_fade_folder_text(),
            "highlight_colors": (1..=9).map(|n| c.get_highlight_color(n)).collect::<Vec<_>>(),
            "category_colors": CATEGORY_LABELS.iter().map(|(k, l)| json!({"key": k, "label": l, "color": c.get_fight_category_color(k)})).collect::<Vec<_>>(),
            "app_version": consts::APP_VERSION,
            "data_dir": self.paths.saved_routes_dir.parent().map(|p| p.to_string_lossy().to_string()),
        })
    }

    pub(super) fn settings_call(&mut self, method: &str, p: &Value) -> Result<Value, String> {
        match method {
            "settings.get" => Ok(self.settings_json()),
            "settings.set" => {
                let key = s(p, "key");
                let c = &mut self.cfg;
                match key {
                    "damage_search_depth" => c.set_damage_search_depth(i(p, "value").unwrap_or(20).clamp(1, 99)),
                    "ignore_accuracy" => c.set_ignore_accuracy(b(p, "value")),
                    "force_full_search" => c.set_force_full_search(b(p, "value")),
                    "player_strategy" => c.set_player_highlight_strategy(s(p, "value")),
                    "enemy_strategy" => c.set_enemy_highlight_strategy(s(p, "value")),
                    "consistent_threshold" => c.set_consistent_threshold(i(p, "value").unwrap_or(90).clamp(1, 99)),
                    "show_move_highlights" => c.set_show_move_highlights(b(p, "value")),
                    "fade_moves" => c.set_fade_moves_without_highlight(b(p, "value")),
                    "test_moves" => c.set_test_moves_enabled(b(p, "value")),
                    "show_legacy" => c.set_show_legacy_controls(b(p, "value")),
                    "notes_visibility" => c.set_notes_visibility_mode(s(p, "value")),
                    "auto_switch" => c.set_auto_switch(b(p, "value")),
                    "auto_load" => c.set_auto_load_most_recent_route(b(p, "value")),
                    "color_major_battles" => c.set_color_major_battles(b(p, "value")),
                    "fade_folder_text" => c.set_fade_folder_text(b(p, "value")),
                    "highlight_color" => c.set_highlight_color(i(p, "index").unwrap_or(1), s(p, "value")),
                    "category_color" => c.set_fight_category_color(s(p, "category"), s(p, "value")),
                    "reset_highlight_colors" => {
                        c.reset_highlight_colors_to_defaults();
                        c.reset_fight_category_colors();
                    }
                    other => return Err(format!("unknown setting {}", other)),
                }
                // battle output depends on most of these
                if self.battle_for.is_some() {
                    self.bc.full_refresh(&self.cfg, &self.ctrl);
                    let _ = self.bc.take_signals();
                }
                Ok(self.settings_json())
            }
            "customgen.list" => {
                let rows: Vec<Value> = self
                    .registry
                    .get_all_custom_gen_info()
                    .iter()
                    .map(|c| json!({"name": c.name, "base": c.base_version, "path": c.path}))
                    .collect();
                Ok(json!({"customs": rows, "bases": self.registry.get_gen_names(true, false)}))
            }
            "customgen.create" => {
                let name = s(p, "name").trim().to_string();
                if name.is_empty() {
                    return Err("Enter a name for the custom game".into());
                }
                if self.registry.get_gen_names(true, true).contains(&name) {
                    return Err("A game with that name already exists".into());
                }
                self.ctrl.create_custom_version(s(p, "base"), &name);
                self.ctrl.load_all_custom_versions();
                Ok(json!(true))
            }
            "customgen.import_backport" => {
                // backports.asm (+ optional backport_moves.asm) into a custom game
                let info = self.registry.get_all_custom_gen_info().into_iter().find(|c| c.name == s(p, "name")).ok_or("Pick a custom game")?;
                let asm = std::fs::read_to_string(s(p, "asm")).map_err(|e| format!("Could not read backports.asm: {}", e))?;
                let moves = match s(p, "moves") {
                    "" => None,
                    path => Some(std::fs::read_to_string(path).map_err(|e| format!("Could not read the moves file: {}", e))?),
                };
                let species = s(p, "species").trim().to_string();
                if species.is_empty() {
                    return Err("Enter the species the backport adds".into());
                }
                let mut text = xpr_data::backport::import_backport(&info.path, &asm, moves.as_deref(), Some(&species))?;
                if let Err(e) = self.registry.reload_all_custom_gens() {
                    text.push_str(&format!("\n\nWarning during reload:\n{}", e));
                }
                Ok(json!({"text": text, "species": species, "version": info.name}))
            }
            "customgen.reload" => {
                self.ctrl.load_all_custom_versions();
                Ok(json!(true))
            }
            _ => Err(format!("unknown method {}", method)),
        }
    }
}
