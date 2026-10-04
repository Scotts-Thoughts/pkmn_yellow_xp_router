//! The tabs. Each is its own feature (see Cargo.toml) and module.

#[cfg(feature = "compare")]
pub mod compare;
#[cfg(feature = "evs")]
pub mod evs;
#[cfg(feature = "misc")]
pub mod misc;
#[cfg(feature = "movedex")]
pub mod movedex;
#[cfg(feature = "movepool")]
pub mod movepool;
#[cfg(feature = "misc")]
pub mod natures;
#[cfg(feature = "pokedex")]
pub mod pokedex;
#[cfg(feature = "stats")]
pub mod stats;
#[cfg(feature = "trainers")]
pub mod trainers;

use crate::DexCx;

/// The movepool column of the Pokédex detail, without the detail having to
/// know the movepool module (it may be built without it).
pub trait MovepoolPane {
    fn movepool_ui(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut DexCx,
        data: &xpr_dex::PokemonData,
        game: &str,
    );
}
