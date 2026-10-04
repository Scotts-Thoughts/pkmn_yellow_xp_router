//! The Pokédex reference data behind the Dex page: a port of Solodex's data
//! layer (`src/renderer/src/data/`).
//!
//! Tables load lazily from `dex_data/` (embedded with feature
//! `embed-dex-data`) and stay in memory; every lookup is a free function
//! mirroring the Solodex function of the same name in snake_case
//! (`getPokemonData` -> [`get_pokemon_data`], ...).
//!
//! Trainer data is not here: the Dex's Trainers and Damage tabs use the
//! router's own trainer tables (`xpr-data`), so their names and numbers
//! match the route editor's.

pub mod forms;
pub mod games;
pub mod model;
pub mod moves;
pub mod natures;
pub mod pokedex;
pub mod source;
pub mod species;
pub mod sprites;
pub mod stats;
pub mod store;
pub mod text;
pub mod tmhm;
pub mod types;

pub use forms::{classify_form, is_mega_form, split_form_name, FormInfo, Region};
pub use games::{game_abbrev, game_color, game_gen, GEN_GROUPS, GAMES};
pub use model::{BaseStats, EncounterEntry, EvoParam, EvolutionEntry, EvolutionStage, MoveData, NatureData, PokemonData, PokemonListEntry, StatKey};
pub use moves::{all_move_names, canonical_move_key, get_move_data, get_moves_for_gen, move_across_gens, move_introduction_gen, move_name_for_game, move_name_for_gen};
pub use natures::{unobtainable_move_sets, UnobtainableMoveSets, UserBans};
pub use pokedex::{
    base_stat_total, default_moves_at_level, get_all_pokemon, get_all_pokemon_for_game, get_encounters_for_pokemon, get_games_for_pokemon, get_learnset, get_pokemon_data, get_pokemon_types,
    get_pokemon_ubst, get_ranking, resolve_species, species_entry, LearnsetEntry, MoveSource, RankKind, StatRankEntry,
};
pub use species::display_name;
pub use store::{preload_all, store};
pub use tmhm::{compare_tmhm, tmhm_code};
pub use types::{defense_matchups, offensive_multiplier, type_matchups, types_for_game, TypeMatchups};
