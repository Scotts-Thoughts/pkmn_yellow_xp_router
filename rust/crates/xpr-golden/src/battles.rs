//! Battle-summary records: the counterpart of `docs/rust_port/golden/battles.py`.

use serde_json::{json, Value};

use xpr_calc::battle_summary::{BattleSummary, SummaryConfig};
use xpr_core::Config;
use xpr_engine::Router;

/// The `SummaryConfig` the Python dumper implicitly uses (the user's config).
pub fn summary_config(cfg: &Config, router: &Router) -> SummaryConfig {
    SummaryConfig {
        calc: cfg.calc_config(),
        player_strategy: cfg.get_player_highlight_strategy().to_string(),
        enemy_strategy: cfg.get_enemy_highlight_strategy().to_string(),
        test_moves_enabled: cfg.get_test_moves_enabled(),
        test_moves: router.test_moves.clone(),
    }
}

pub fn dump_battles(router: &Router, cfg: &SummaryConfig) -> Value {
    let mut summary = BattleSummary::new();
    let mut result = Vec::new();
    let Some(gen) = router.gen().cloned() else {
        return Value::Array(result);
    };
    for (idx, gid) in router.all_groups().into_iter().enumerate() {
        let Some(group) = router.group(gid) else { continue };
        let ed = &group.event_definition;
        let mut rec = json!({"group_index": idx, "group_name": group.name});
        let outcome: Result<Option<&'static str>, String> = if ed.trainer_def.is_some() {
            summary.load_from_event(router, Some(gid), cfg).map(|_| Some("trainer"))
        } else if ed.wild_pkmn_info.is_some() {
            match ed.pokemon_list(&gen) {
                Ok(wild) if !wild.is_empty() && group.init_state.is_some() => summary
                    .load_from_state(&gen, group.init_state.as_deref(), &wild, None, true, cfg)
                    .map(|_| Some("wild")),
                Ok(_) => Ok(None),
                Err(e) => Err(e),
            }
        } else {
            Ok(None)
        };
        match outcome {
            Ok(None) => continue,
            Ok(Some(kind)) => {
                rec["kind"] = json!(kind);
                rec["summary"] = summary.to_json();
            }
            Err(e) => {
                rec["error"] = json!(e);
            }
        }
        result.push(rec);
    }
    Value::Array(result)
}
