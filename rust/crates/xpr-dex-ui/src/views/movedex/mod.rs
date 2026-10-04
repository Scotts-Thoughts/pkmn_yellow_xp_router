//! Movedex tab: every move of a generation with filters and sorting, and the
//! move detail (generation history, learners). Port of Solodex
//! `MovedexView.tsx`, `MoveDetailView.tsx`, `MoveSpotlightSearch.tsx`.
//!
//! * The list (`list.rs`): search / type / category / power / accuracy / PP /
//!   "newly introduced" filters, the table, a right-click learner panel and
//!   the Space "jump to move" overlay.
//! * The detail (`detail.rs`): one card per run of identical generations with
//!   change callouts between them, plus who learns the move in the current
//!   game. The shell's Ctrl+Shift+Space move search sets
//!   `DexState::focused_move`, which opens it; Back clears it.

use egui::Ui;

use crate::widgets::{SpotlightRow, SpotlightState};
use crate::DexCx;

pub mod data;
mod detail;
mod kit;
mod learners;
mod list;

use data::{Filters, MoveRow, Sort};

#[derive(Default)]
pub struct MovedexView {
    /// the game `table` was built for
    table_game: String,
    table: Vec<MoveRow>,
    /// types of the game, for the type filter
    types: Vec<String>,
    /// width of the Move column (the widest name, with room for the NEW mark)
    name_col: f32,
    filters: Filters,
    sort: Sort,
    /// table indices after filtering and sorting, with what they were made from
    view: Vec<usize>,
    view_sig: Option<(String, Filters, Sort)>,
    /// the move whose learners the right-hand panel shows (right click)
    selected: Option<String>,
    /// the row picked in the jump overlay
    highlighted: Option<String>,
    scroll_to: Option<String>,
    /// the Space overlay
    jump: Option<SpotlightState>,
    panel: learners::Learners,
    detail: detail::Detail,
}

impl MovedexView {
    pub fn ui(&mut self, ui: &mut Ui, cx: &mut DexCx) {
        if let Some(name) = cx.state.focused_move.clone() {
            self.detail_ui(ui, cx, &name);
        } else {
            self.list_ui(ui, cx);
        }
    }
}

/// Move search results (row key = move name): every move of every generation
/// whose name contains the query, by name.
pub fn spotlight_rows(query: &str) -> Vec<SpotlightRow> {
    data::search_names(query).into_iter().map(|name| SpotlightRow { key: name.to_string(), label: name.to_string(), detail: String::new(), sprite: None, color: None }).collect()
}
