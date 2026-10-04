//! Two species in one game (Solodex `ComparisonView.tsx`). The shared
//! layout is in [`super::two`].

use egui::Ui;

use super::tables::Column;
use super::two::{self, Kind, Two};
use super::{unavailable, CompareView, Mode};
use crate::DexCx;

/// The two species' tables for one game.
pub struct Model {
    key: (String, String, String),
    left: Option<Column>,
    right: Option<Column>,
}

pub fn ui(v: &mut CompareView, ui: &mut Ui, cx: &mut DexCx) {
    v.begin(Mode::Pair, ui.ctx());
    let (Some(left_name), Some(right_name)) = (
        cx.state.selected().map(str::to_string),
        cx.state.settings.comparing_with.clone(),
    ) else {
        return;
    };
    let game = cx.state.game().to_string();
    let key = (left_name.clone(), right_name.clone(), game.clone());
    if v.pair.as_ref().map(|m| m.key != key).unwrap_or(true) {
        v.pair = Some(Model {
            left: Column::merged(&left_name, &game, false),
            right: Column::merged(&right_name, &game, false),
            key,
        });
    }
    let model = v.pair.take().expect("model");
    let (Some(left), Some(right)) = (&model.left, &model.right) else {
        if unavailable(
            ui,
            cx,
            "One or both Pokemon not available in this game.",
            "Exit Comparison",
            Some("[Esc]"),
        ) {
            cx.state.exit_compare();
        }
        v.pair = Some(model);
        return;
    };
    let out = two::ui(
        v,
        ui,
        cx,
        &Two {
            kind: Kind::Pair,
            left,
            right,
            left_name: &left_name,
            right_name: &right_name,
            left_game: &game,
            right_game: &game,
        },
    );
    // Solodex's `onSelectLeft` / `onSelectRight` / `onNavigate`
    if let Some(name) = out.select_left {
        cx.state.set_selected_keep_compare(&name);
    }
    if let Some(name) = out.select_right {
        cx.state.compare_with(&name);
    }
    if let Some(name) = out.navigate {
        cx.state.select_species(&name);
    }
    v.pair = Some(model);
}
