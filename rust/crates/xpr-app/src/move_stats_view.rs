//! The "Move Stats" window (spec §2.5): the selected trainer battle's move
//! stats and the run total, for checking what the recorder counted.

use egui::{RichText, Ui};

use xpr_engine::move_stats::{MoveCounts, SideStats, INTEREST_CRIT_ONLY, INTEREST_NO_LUCK};
use xpr_engine::SidePair;
use xpr_ui_kit::theme::Theme;

use crate::controller::MainController;

/// "5/6 (exp 5.4)"
fn ratio(n: i64, of: i64, expected: f64) -> String {
    if of == 0 {
        return "-".into();
    }
    format!("{}/{} (exp {:.1})", n, of, expected)
}

fn effect_text(c: &MoveCounts) -> String {
    let mut s = if c.effect_rolls > 0 { ratio(c.effects, c.effect_rolls, c.expected_effects) } else { "-".into() };
    if c.effect_blocked > 0 {
        s.push_str(&format!(", {} blocked", c.effect_blocked));
    }
    s
}

fn side_table(ui: &mut Ui, theme: &Theme, id: &str, title: &str, side: &SideStats) {
    ui.label(RichText::new(title).font(theme.body_bold()).color(theme.text));
    if side.moves.is_empty() {
        ui.label(RichText::new("No moves recorded.").font(theme.body()).color(theme.secondary));
        return;
    }
    egui::Grid::new(id).striped(true).spacing([14.0, 3.0]).show(ui, |ui| {
        for h in ["Move", "Uses", "Hits", "Crits", "Effect", "Other"] {
            ui.label(RichText::new(h).font(theme.body_bold()).color(theme.secondary));
        }
        ui.end_row();
        for name in side.sorted_names() {
            let c = &side.moves[&name];
            let dim = c.interest == INTEREST_NO_LUCK || c.interest == INTEREST_CRIT_ONLY;
            let color = if dim { theme.secondary } else { theme.text };
            let cell = |ui: &mut Ui, t: String| {
                ui.label(RichText::new(t).font(theme.body()).color(color));
            };
            cell(ui, name.clone());
            cell(ui, c.uses.to_string());
            cell(ui, if c.interest == INTEREST_NO_LUCK { "-".into() } else { ratio(c.hits, c.acc_rolls, c.expected_hits) });
            cell(ui, ratio(c.crits, c.crit_rolls, c.expected_crits));
            cell(ui, effect_text(c));
            let mut other = Vec::new();
            for (n, label) in [(c.super_effective, "SE"), (c.not_very_effective, "NVE"), (c.no_effect, "immune"), (c.failed, "failed"), (c.protected, "protected"), (c.kos, "KO")] {
                if n > 0 {
                    other.push(format!("{} {}", n, label));
                }
            }
            if !c.multi_hit.is_empty() {
                other.push(format!("hits {}", c.multi_hit.iter().map(|(k, v)| format!("{}x{}", k, v)).collect::<Vec<_>>().join(" ")));
            }
            for (by, n) in &c.called_by {
                other.push(format!("{} via {}", n, by));
            }
            cell(ui, other.join(", "));
            ui.end_row();
        }
    });
}

fn pair(ui: &mut Ui, theme: &Theme, id: &str, p: &SidePair) {
    side_table(ui, theme, &format!("{}_player", id), "Player", &p.player);
    ui.add_space(8.0);
    side_table(ui, theme, &format!("{}_enemy", id), "Enemy", &p.enemy);
}

pub fn ui(ui: &mut Ui, theme: &Theme, ctrl: &MainController) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        let selected = ctrl
            .get_single_selected_event_id(false)
            .and_then(|id| ctrl.router.group(id))
            .and_then(|g| g.event_definition.trainer_def.as_ref())
            .and_then(|t| t.move_stats.clone().map(|m| (t.trainer_name.clone(), m)));
        match selected {
            Some((name, ms)) => {
                ui.label(RichText::new(format!("This battle: {} ({})", name, ms.outcome)).font(theme.body_bold()).color(theme.text));
                pair(ui, theme, "battle", &ms.stats);
            }
            None => {
                ui.label(RichText::new("Select a recorded trainer battle to see its numbers.").font(theme.body()).color(theme.secondary));
            }
        }
        ui.add_space(16.0);
        ui.separator();
        match &ctrl.router.move_stats {
            None if !ctrl.router.has_move_stats() => {
                ui.label(RichText::new("No move stats in this route.").font(theme.body()).color(theme.secondary));
            }
            run => {
                let total = ctrl.router.move_stats_total();
                let title = match run {
                    Some(r) => format!("Run total: {} with {}", r.game, r.solo_species),
                    None => "Run total".to_string(),
                };
                ui.label(RichText::new(title).font(theme.body_bold()).color(theme.text));
                if let Some(r) = run {
                    ui.label(RichText::new(format!("{} reset or lost battle(s) are kept apart and not in this total.", r.reset_attempts.len())).font(theme.body()).color(theme.secondary));
                }
                pair(ui, theme, "run", &total);
            }
        }
    });
}
