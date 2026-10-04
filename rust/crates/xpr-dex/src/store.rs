//! The loaded tables. Every table is parsed on first use and kept for the
//! rest of the session (Solodex's per-game `loadGame` / `loadEncounters`
//! chunks become lazily parsed JSON here); [`preload_all`] warms them on a
//! background thread.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use indexmap::IndexMap;
use serde::de::DeserializeOwned;

use crate::games::{self, GAMES};
use crate::model::{EncounterEntry, MoveData, NatureData, PokemonData, PokemonListEntry};

pub type RawDex = IndexMap<String, PokemonData>;
pub type EncounterTable = IndexMap<String, Vec<EncounterEntry>>;

/// One game's Pokédex: the table as stored, plus what is derived from it.
pub struct GameDex {
    pub game: &'static str,
    pub gen: u8,
    pub raw: RawDex,
    /// `get_pokemon_data` for every species (families resolved), in table order
    pub(crate) resolved: OnceLock<IndexMap<String, Arc<PokemonData>>>,
    /// `get_all_pokemon_for_game`: resolved species sorted by national dex number
    pub(crate) by_dex: OnceLock<Vec<Arc<PokemonData>>>,
    /// Mega / Primal forms filed under every species they belong to
    pub(crate) mega_index: OnceLock<HashMap<String, Vec<String>>>,
    /// unfiltered rankings by kind
    pub(crate) rankings: Mutex<HashMap<crate::pokedex::RankKind, Arc<Vec<crate::pokedex::StatRankEntry>>>>,
}

pub struct SpeciesIndex {
    pub entries: Vec<PokemonListEntry>,
    pub(crate) by_name: HashMap<String, usize>,
}

pub struct DexStore {
    index: OnceLock<SpeciesIndex>,
    pub(crate) moves: OnceLock<crate::moves::MoveTables>,
    pub(crate) charts: OnceLock<crate::types::TypeCharts>,
    pub(crate) tmhm: OnceLock<crate::tmhm::TmHmTables>,
    natures: OnceLock<IndexMap<String, NatureData>>,
    unobtainable: OnceLock<IndexMap<String, Vec<String>>>,
    form_sprites: OnceLock<HashMap<String, u32>>,
    games: Vec<OnceLock<Option<Arc<GameDex>>>>,
    encounters: Vec<OnceLock<Option<Arc<EncounterTable>>>>,
}

static STORE: OnceLock<DexStore> = OnceLock::new();

/// The process-wide store.
pub fn store() -> &'static DexStore {
    STORE.get_or_init(|| DexStore {
        index: OnceLock::new(),
        moves: OnceLock::new(),
        charts: OnceLock::new(),
        tmhm: OnceLock::new(),
        natures: OnceLock::new(),
        unobtainable: OnceLock::new(),
        form_sprites: OnceLock::new(),
        games: GAMES.iter().map(|_| OnceLock::new()).collect(),
        encounters: GAMES.iter().map(|_| OnceLock::new()).collect(),
    })
}

pub(crate) fn parse<T: DeserializeOwned>(rel: &str) -> Option<T> {
    let bytes = crate::source::read(rel)?;
    match serde_json::from_slice(&bytes) {
        Ok(v) => Some(v),
        Err(e) => {
            log::error!("dex data file {} does not parse: {}", rel, e);
            None
        }
    }
}

fn game_idx(game: &str) -> Option<usize> {
    GAMES.iter().position(|g| *g == game)
}

impl DexStore {
    pub fn species_index(&self) -> &SpeciesIndex {
        self.index.get_or_init(|| {
            let entries: Vec<PokemonListEntry> = parse("species_index.json").unwrap_or_default();
            let by_name = entries.iter().enumerate().map(|(i, e)| (e.name.clone(), i)).collect();
            SpeciesIndex { entries, by_name }
        })
    }

    /// A game's Pokédex, parsed on first use; `None` for an unknown game or
    /// a missing / broken file.
    pub fn game(&self, game: &str) -> Option<Arc<GameDex>> {
        let idx = game_idx(game)?;
        self.games[idx]
            .get_or_init(|| {
                let name = GAMES[idx];
                let raw: RawDex = parse(&format!("pokedex/{}.json", games::slug(name)))?;
                Some(Arc::new(GameDex {
                    game: name,
                    gen: games::game_gen(name),
                    raw,
                    resolved: OnceLock::new(),
                    by_dex: OnceLock::new(),
                    mega_index: OnceLock::new(),
                    rankings: Mutex::new(HashMap::new()),
                }))
            })
            .clone()
    }

    /// Whether a game's table is already in memory (no load is started).
    pub fn is_game_loaded(&self, game: &str) -> bool {
        game_idx(game).map(|i| self.games[i].get().is_some()).unwrap_or(false)
    }

    /// A game's wild encounters by species; `None` when the game has none.
    pub fn encounters(&self, game: &str) -> Option<Arc<EncounterTable>> {
        let idx = game_idx(game)?;
        self.encounters[idx]
            .get_or_init(|| {
                let rel = format!("encounters/{}.json", games::slug(GAMES[idx]));
                if crate::source::is_embedded() || crate::source::data_dir().join(&rel).exists() {
                    parse::<EncounterTable>(&rel).map(Arc::new)
                } else {
                    None
                }
            })
            .clone()
    }

    pub fn natures(&self) -> &IndexMap<String, NatureData> {
        self.natures.get_or_init(|| parse("natures.json").unwrap_or_default())
    }

    pub fn unobtainable_raw(&self) -> &IndexMap<String, Vec<String>> {
        self.unobtainable.get_or_init(|| parse("unobtainable_moves.json").unwrap_or_default())
    }

    pub fn form_sprites(&self) -> &HashMap<String, u32> {
        self.form_sprites.get_or_init(|| parse("form_sprites.json").unwrap_or_default())
    }

    pub fn moves(&self) -> &crate::moves::MoveTables {
        self.moves.get_or_init(|| crate::moves::MoveTables::new(parse::<IndexMap<String, IndexMap<String, MoveData>>>("moves.json").unwrap_or_default()))
    }

    pub fn charts(&self) -> &crate::types::TypeCharts {
        self.charts.get_or_init(|| crate::types::TypeCharts::new(parse("effectiveness.json").unwrap_or_default()))
    }

    pub fn tmhm(&self) -> &crate::tmhm::TmHmTables {
        self.tmhm.get_or_init(|| crate::tmhm::TmHmTables::new(parse("tmhm.json").unwrap_or_default()))
    }
}

/// Parse every table on a background thread (Solodex's `preloadAllData`),
/// so switching games never waits on a parse. Safe to call repeatedly.
pub fn preload_all() {
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_err() {
        return;
    }
    std::thread::Builder::new()
        .name("dex-preload".into())
        .spawn(|| {
            let s = store();
            let _ = s.species_index();
            let _ = s.moves();
            let _ = s.charts();
            let _ = s.tmhm();
            let _ = s.natures();
            for g in GAMES {
                if let Some(dex) = s.game(g) {
                    let _ = crate::pokedex::resolved(&dex);
                }
            }
            for g in GAMES {
                let _ = s.encounters(g);
            }
        })
        .ok();
}
