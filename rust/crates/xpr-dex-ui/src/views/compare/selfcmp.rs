//! One species across two games (Solodex `SelfComparisonView.tsx`). The
//! shared layout is in [`super::two`]; this picks the two games and keeps
//! them while the page's selection, game and right-hand game stay the same
//! (Solodex remounts the view when that key changes).

use egui::Ui;

use super::tables::Column;
use super::two::{self, Kind, Two};
use super::{unavailable, CompareView, Mode};
use crate::DexCx;

/// The species' two games and their tables.
pub struct Model {
    /// (species, the page's game, the right-hand game the page asked for)
    key: (String, String, Option<String>),
    games: Vec<String>,
    left_game: String,
    right_game: String,
    /// the games `left` and `right` were built for
    built: (String, String),
    left: Option<Column>,
    right: Option<Column>,
}

/// The right-hand game when none was asked for: the last game of another
/// generation, else any other game.
fn default_right(games: &[String], left: &str) -> String {
    let left_gen = xpr_dex::game_gen(left);
    games
        .iter()
        .rev()
        .find(|g| xpr_dex::game_gen(g) != left_gen)
        .or_else(|| games.iter().find(|g| g.as_str() != left))
        .cloned()
        .unwrap_or_else(|| left.to_string())
}

impl Model {
    fn new(key: (String, String, Option<String>)) -> Model {
        let games = xpr_dex::get_games_for_pokemon(&key.0);
        let left = if games.contains(&key.1) {
            key.1.clone()
        } else {
            games.first().cloned().unwrap_or_else(|| key.1.clone())
        };
        let right = match &key.2 {
            Some(r) if games.iter().any(|g| g == r) => r.clone(),
            _ => default_right(&games, &left),
        };
        Model {
            key,
            games,
            left_game: left,
            right_game: right,
            built: (String::new(), String::new()),
            left: None,
            right: None,
        }
    }

    fn load(&mut self) {
        if self.built != (self.left_game.clone(), self.right_game.clone()) {
            self.left = Column::single(&self.key.0, &self.left_game);
            self.right = Column::single(&self.key.0, &self.right_game);
            self.built = (self.left_game.clone(), self.right_game.clone());
        }
    }
}

pub fn ui(v: &mut CompareView, ui: &mut Ui, cx: &mut DexCx) {
    v.begin(Mode::SelfCmp, ui.ctx());
    let Some(name) = cx.state.selected().map(str::to_string) else {
        return;
    };
    let key = (
        name.clone(),
        cx.state.game().to_string(),
        cx.state.self_compare_right_game.clone(),
    );
    if v.selfcmp.as_ref().map(|m| m.key != key).unwrap_or(true) {
        v.selfcmp = Some(Model::new(key));
    }
    let mut model = v.selfcmp.take().expect("model");
    model.load();
    let out = {
        let (Some(left), Some(right)) = (&model.left, &model.right) else {
            if unavailable(
                ui,
                cx,
                "Pokemon not available in one of the selected generations.",
                "Exit",
                Some("[Esc]"),
            ) {
                cx.state.exit_self_compare();
            }
            v.selfcmp = Some(model);
            return;
        };
        two::ui(
            v,
            ui,
            cx,
            &Two {
                kind: Kind::SelfCmp {
                    games: &model.games,
                },
                left,
                right,
                left_name: &name,
                right_name: &name,
                left_game: &model.left_game,
                right_game: &model.right_game,
            },
        )
    };
    if let Some(g) = out.pick_left_game {
        model.left_game = g;
    }
    if let Some(g) = out.pick_right_game {
        model.right_game = g;
    }
    if let Some(n) = out.navigate {
        cx.state.select_species(&n);
    }
    v.selfcmp = Some(model);
}
