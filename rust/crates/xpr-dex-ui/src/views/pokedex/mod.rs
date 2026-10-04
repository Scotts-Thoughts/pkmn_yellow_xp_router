//! Pokédex tab: the species list (left) and the species detail (right).
//! Port of Solodex `PokemonList.tsx`, `PokemonDetail.tsx` and the cards it uses
//! (`BaseStats.tsx`, `RankingCard.tsx`, `TypeEffectivenessPanel.tsx`,
//! `GrowthRatePopover.tsx`, `PokemonContextMenu.tsx`).
//!
//! Reusable by the comparison views: [`stats::base_stats`] (stat bars with
//! their ranking popover), [`ranking`] (the popover and the expanded card),
//! [`effectiveness::type_effectiveness`] and [`pokemon_context_menu`].

use egui::Ui;

use crate::views::MovepoolPane;
use crate::DexCx;

mod common;
pub mod context_menu;
mod detail;
pub mod effectiveness;
mod growth;
mod list;
pub mod ranking;
pub mod stats;

pub use context_menu::{enabled_entries as context_menu_entries, pokemon_context_menu, MenuCtx};
pub use detail::{evo_label, sprite_scale, weight_power};
pub use growth::{calc_exp, growth_popover};
pub use list::{filter_names, Filters};

#[derive(Default)]
pub struct PokedexView {
    list: list::ListState,
    detail: detail::DetailState,
}

impl PokedexView {
    /// The searchable, filterable species list. Writes the filtered names
    /// to `cx.state.filtered_names` (Up / Down navigation).
    pub fn list_ui(&mut self, ui: &mut Ui, cx: &mut DexCx) {
        self.list.ui(ui, cx);
    }

    /// The selected species in `cx.state.game()`; the movepool column is
    /// drawn by `movepool` when the movepool feature is built.
    pub fn detail_ui(
        &mut self,
        ui: &mut Ui,
        cx: &mut DexCx,
        movepool: Option<&mut dyn MovepoolPane>,
    ) {
        self.detail.ui(ui, cx, movepool);
    }

    /// The list's current filters.
    pub fn filters(&self) -> &Filters {
        &self.list.filters
    }

    /// Replace the list's filters (they are not persisted by the page).
    pub fn set_filters(&mut self, filters: Filters) {
        self.list.filters = filters;
    }
}
