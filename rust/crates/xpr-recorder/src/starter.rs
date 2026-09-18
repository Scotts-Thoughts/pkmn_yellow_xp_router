//! "Start Recording" from the landing page: connect to GameHook before any
//! route exists, work out which game is running from the loaded mapper, wait
//! for the player's first Pokémon to land in party slot 1, let its fields
//! settle for a second and hand the application everything the new route
//! needs (version, species, DVs/IVs, nature, ability). The app then creates
//! the route and starts the ordinary recorder.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;

use xpr_core::consts;
use xpr_data::model::{Gen, Nature, StatBlock};

use crate::gamehook::{GameHookClient, GameHookProperty, PropertyStore, SessionEvents};

/// How long slot 1 must have held the new Pokémon before its fields are
/// read: GameHook delivers a party slot's fields over several batches.
pub const SETTLE_TIME: Duration = Duration::from_secs(1);
/// A slot-1 field changing while settling restarts the wait, but never past
/// this long after the Pokémon first appeared.
const SETTLE_CAP: Duration = Duration::from_secs(5);

/// The game a mapper name identifies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetectedGame {
    /// the mapper's `gameName`
    pub mapper_name: String,
    /// the router version the route is created for
    pub version: String,
    pub generation: u8,
    /// the other game of the pair when the mapper serves two ("Pokemon Red
    /// and Blue" -> Red, with Blue here); the pair shares one data set
    pub sibling: Option<String>,
}

/// `gameName` -> router version. A mapper that serves two games ("Pokemon
/// Red and Blue", "Pokemon FireRed and LeafGreen", ...) yields the first of
/// the pair: those pairs share one data set in the router.
pub fn detect_version(mapper_name: &str) -> Option<&'static str> {
    let n = mapper_name.to_lowercase();
    let has = |s: &str| n.contains(s);
    let no_space = n.replace(' ', "");
    // order matters: "firered" before "red", "heartgold" before "gold", "black 2" before "black"
    let version = if has("yellow") {
        consts::YELLOW_VERSION
    } else if no_space.contains("firered") {
        consts::FIRE_RED_VERSION
    } else if no_space.contains("leafgreen") {
        consts::LEAF_GREEN_VERSION
    } else if has("red") {
        consts::RED_VERSION
    } else if has("blue") {
        consts::BLUE_VERSION
    } else if has("crystal") {
        consts::CRYSTAL_VERSION
    } else if no_space.contains("heartgold") {
        consts::HEART_GOLD_VERSION
    } else if no_space.contains("soulsilver") {
        consts::SOUL_SILVER_VERSION
    } else if has("gold") {
        consts::GOLD_VERSION
    } else if has("silver") {
        consts::SILVER_VERSION
    } else if has("emerald") {
        consts::EMERALD_VERSION
    } else if has("ruby") {
        consts::RUBY_VERSION
    } else if has("sapphire") {
        consts::SAPPHIRE_VERSION
    } else if has("platinum") {
        consts::PLATINUM_VERSION
    } else if has("diamond") {
        consts::DIAMOND_VERSION
    } else if has("pearl") {
        consts::PEARL_VERSION
    } else if no_space.contains("black2") {
        consts::BLACK_2_VERSION
    } else if no_space.contains("white2") {
        consts::WHITE_2_VERSION
    } else if has("black") {
        consts::BLACK_VERSION
    } else if has("white") {
        consts::WHITE_VERSION
    } else {
        return None;
    };
    Some(version)
}

/// The second game of a two-game mapper, if `mapper_name` names both.
pub fn detect_sibling(mapper_name: &str, version: &str) -> Option<&'static str> {
    let n = mapper_name.to_lowercase().replace(' ', "");
    let pairs: [(&str, &str, &str); 6] = [
        (consts::RED_VERSION, consts::BLUE_VERSION, "blue"),
        (consts::GOLD_VERSION, consts::SILVER_VERSION, "silver"),
        (consts::RUBY_VERSION, consts::SAPPHIRE_VERSION, "sapphire"),
        (consts::FIRE_RED_VERSION, consts::LEAF_GREEN_VERSION, "leafgreen"),
        (consts::DIAMOND_VERSION, consts::PEARL_VERSION, "pearl"),
        (consts::HEART_GOLD_VERSION, consts::SOUL_SILVER_VERSION, "soulsilver"),
    ];
    pairs.iter().find(|(first, _, needle)| *first == version && n.contains(needle)).map(|(_, second, _)| *second)
}

pub fn detect_game(mapper_name: &str) -> Option<DetectedGame> {
    let version = detect_version(mapper_name)?;
    let generation = xpr_data::registry::gen_of_builtin(version)?.number();
    Some(DetectedGame {
        mapper_name: mapper_name.to_string(),
        version: version.to_string(),
        generation,
        sibling: detect_sibling(mapper_name, version).map(|s| s.to_string()),
    })
}

/// What is in party slot 1 right now (for the status text).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartyMon {
    pub species: String,
    pub level: i64,
}

/// Everything the new route is created from.
#[derive(Clone, Debug)]
pub struct StarterInfo {
    pub game: DetectedGame,
    /// the species as the mapper names it (the app resolves it in the DB)
    pub species: String,
    pub level: i64,
    /// DVs (gens 1-2, HP derived) or IVs (gens 3+)
    pub dvs: StatBlock,
    /// gens 3+ only
    pub nature: Option<Nature>,
    /// the mapper's ability value: a bool (gen 3: second-ability bit), a
    /// name (gens 4-5) or nothing; see `resolve_ability_idx`
    pub ability: Value,
}

impl StarterInfo {
    /// One line for the status bar / log.
    pub fn summary(&self) -> String {
        let d = &self.dvs;
        let stats = if self.game.generation <= 2 {
            format!("DVs {}/{}/{}/{}/{}", d.hp, d.attack, d.defense, d.speed, d.special_attack)
        } else {
            format!("IVs {}/{}/{}/{}/{}/{}", d.hp, d.attack, d.defense, d.special_attack, d.special_defense, d.speed)
        };
        let mut s = format!("{} {} Lv{} {}", self.game.version, self.species, self.level, stats);
        if let Some(n) = self.nature {
            s.push_str(&format!(" {}", n.display_name()));
        }
        s
    }
}

/// The ability index the route stores, from the mapper's ability value and
/// the species' ability list: gen 3 mappers give the second-ability bit,
/// gens 4-5 the ability's name.
pub fn resolve_ability_idx(raw: &Value, abilities: &[String]) -> i64 {
    let sanitize = xpr_core::io_utils::sanitize_string;
    match raw {
        Value::Bool(second) => {
            if *second && abilities.len() > 1 && !abilities[1].trim().is_empty() {
                1
            } else {
                0
            }
        }
        Value::String(name) => abilities.iter().position(|a| sanitize(a) == sanitize(name)).unwrap_or(0) as i64,
        Value::Number(n) => {
            let idx = n.as_i64().unwrap_or(0);
            if idx >= 0 && (idx as usize) < abilities.len() {
                idx
            } else {
                0
            }
        }
        _ => 0,
    }
}

/// Where the quick start is, as the UI shows it.
#[derive(Clone, Debug)]
pub enum QuickStartPhase {
    /// no hub connection yet (or it dropped); the text says what is happening
    Connecting(String),
    /// connected, but GameHook has no mapper loaded
    NoMapper,
    /// the mapper's game is not one the router knows
    UnsupportedGame(String),
    /// slot 1 already holds a Pokémon: waiting for a new game to clear the party
    WaitingForNewGame { game: DetectedGame, current: PartyMon },
    /// the party is empty: waiting for the first Pokémon
    WaitingForStarter(DetectedGame),
    /// the Pokémon arrived; its fields are settling
    Settling { game: DetectedGame, current: PartyMon },
    Done(StarterInfo),
    /// the mapper is missing something essential
    Failed(String),
}

impl QuickStartPhase {
    pub fn game(&self) -> Option<&DetectedGame> {
        match self {
            QuickStartPhase::WaitingForNewGame { game, .. } | QuickStartPhase::Settling { game, .. } | QuickStartPhase::WaitingForStarter(game) => Some(game),
            QuickStartPhase::Done(info) => Some(&info.game),
            _ => None,
        }
    }
}

/// The party-slot-1 paths of the loaded mapper (every mapper flavour names
/// them a little differently).
#[derive(Clone, Debug)]
struct SlotKeys {
    species: String,
    level: Option<String>,
    team_count: Option<String>,
    hp: Option<String>,
    attack: Option<String>,
    defense: Option<String>,
    speed: Option<String>,
    special_attack: Option<String>,
    special_defense: Option<String>,
    nature: Option<String>,
    ability: Option<String>,
}

impl SlotKeys {
    fn resolve(store: &PropertyStore) -> Option<SlotKeys> {
        let pick = |cands: &[&str]| cands.iter().find_map(|c| store.resolve_case(c));
        Some(SlotKeys {
            species: pick(&["player.team.0.species"])?,
            level: pick(&["player.team.0.level"]),
            team_count: pick(&["player.team_count", "player.teamCount"]),
            hp: pick(&["player.team.0.ivHp", "player.team.0.ivs.hp"]),
            attack: pick(&["player.team.0.dvAttack", "player.team.0.ivAttack", "player.team.0.ivs.attack"]),
            defense: pick(&["player.team.0.dvDefense", "player.team.0.ivDefense", "player.team.0.ivs.defense"]),
            speed: pick(&["player.team.0.dvSpeed", "player.team.0.ivSpeed", "player.team.0.ivs.speed"]),
            special_attack: pick(&["player.team.0.dvSpecial", "player.team.0.ivSpecialAttack", "player.team.0.ivs.special_attack", "player.team.0.ivs.special"]),
            special_defense: pick(&["player.team.0.ivSpecialDefense", "player.team.0.ivs.special_defense"]),
            nature: pick(&["player.team.0.nature"]),
            ability: pick(&["player.team.0.ability"]),
        })
    }

    fn all(&self) -> Vec<String> {
        let mut v = vec![self.species.clone()];
        for k in [&self.level, &self.team_count, &self.hp, &self.attack, &self.defense, &self.speed, &self.special_attack, &self.special_defense, &self.nature, &self.ability].into_iter().flatten() {
            v.push(k.clone());
        }
        v
    }

    /// Slot 1 holds a Pokémon: a named species, and a non-empty party when
    /// the mapper reports the count (a title screen can still show the old
    /// save's slot data).
    fn filled(&self, store: &PropertyStore) -> bool {
        let species = store.get_value(Some(&self.species));
        let named = species.as_str().map(|s| !s.trim().is_empty()).unwrap_or(false);
        if !named {
            return false;
        }
        match self.team_count.as_deref().map(|k| store.get_value(Some(k))) {
            Some(Value::Number(n)) => n.as_i64().unwrap_or(0) > 0,
            _ => true,
        }
    }

    fn current(&self, store: &PropertyStore) -> PartyMon {
        PartyMon {
            species: store.get_value(Some(&self.species)).as_str().unwrap_or("").to_string(),
            level: self.level.as_deref().and_then(|k| crate::gamehook::value_as_i64(&store.get_value(Some(k)))).unwrap_or(0),
        }
    }

    fn stat(&self, store: &PropertyStore, key: &Option<String>, what: &str) -> i64 {
        match key.as_deref() {
            Some(k) => match crate::gamehook::value_as_i64(&store.get_value(Some(k))) {
                Some(v) => v,
                None => {
                    log::warn!("[Quick Start] {} ({}) has no numeric value; using 0", what, k);
                    0
                }
            },
            None => {
                log::warn!("[Quick Start] the mapper has no {} for party slot 1; using 0", what);
                0
            }
        }
    }

    fn read(&self, store: &PropertyStore, game: &DetectedGame) -> StarterInfo {
        let gen = Gen::from_number(game.generation).unwrap_or(Gen::One);
        let current = self.current(store);
        let attack = self.stat(store, &self.attack, "attack DV/IV");
        let defense = self.stat(store, &self.defense, "defense DV/IV");
        let speed = self.stat(store, &self.speed, "speed DV/IV");
        let special_attack = self.stat(store, &self.special_attack, "special DV/IV");
        let (hp, special_defense) = if game.generation <= 2 {
            // gens 1-2: the HP DV is the low bit of each of the other four
            ((attack % 2) * 8 + (defense % 2) * 4 + (speed % 2) * 2 + (special_attack % 2), special_attack)
        } else {
            (self.stat(store, &self.hp, "HP IV"), self.stat(store, &self.special_defense, "special defense IV"))
        };
        let dvs = StatBlock::new(gen, hp, attack, defense, special_attack, special_defense, speed, false);
        let nature = if game.generation >= 3 {
            let raw = self.nature.as_deref().map(|k| store.get_value(Some(k))).unwrap_or(Value::Null);
            let parsed = match &raw {
                Value::String(s) => Nature::from_name(s),
                Value::Number(n) => n.as_i64().and_then(Nature::from_index),
                _ => None,
            };
            if parsed.is_none() {
                log::warn!("[Quick Start] could not read the nature of party slot 1 ({:?}); the route keeps the default", raw);
            }
            parsed
        } else {
            None
        };
        let ability = if game.generation >= 3 { self.ability.as_deref().map(|k| store.get_value(Some(k))).unwrap_or(Value::Null) } else { Value::Null };
        StarterInfo { game: game.clone(), species: current.species, level: current.level, dvs, nature, ability }
    }
}

/// The `SessionEvents` side: runs on the GameHook connection thread.
struct StarterWatch {
    phase: Arc<Mutex<QuickStartPhase>>,
    accept_current: Arc<AtomicBool>,
    /// set by `QuickStart::stop` (cancel or hand-off): the shutdown that
    /// follows is expected
    stopped: Arc<AtomicBool>,
    wake: Arc<dyn Fn() + Send + Sync>,
    url: String,
    game: Option<DetectedGame>,
    keys: Option<SlotKeys>,
    /// when slot 1 first showed the new Pokémon, and when to read it
    settle: Option<(Instant, Instant)>,
    done: bool,
}

impl StarterWatch {
    fn set_phase(&self, phase: QuickStartPhase) {
        *self.phase.lock().unwrap() = phase;
        (self.wake)();
    }

    fn phase(&self) -> QuickStartPhase {
        self.phase.lock().unwrap().clone()
    }

    /// Re-derive the phase from the slot's current contents.
    fn evaluate(&mut self, store: &PropertyStore, now: Instant) {
        if self.done {
            return;
        }
        let (Some(game), Some(keys)) = (self.game.clone(), self.keys.clone()) else { return };
        let filled = keys.filled(store);
        match self.phase() {
            QuickStartPhase::WaitingForNewGame { current, .. } => {
                if !filled {
                    log::info!("[Quick Start] party cleared; waiting for the first Pokémon");
                    self.set_phase(QuickStartPhase::WaitingForStarter(game));
                } else {
                    let now_mon = keys.current(store);
                    if now_mon != current {
                        self.set_phase(QuickStartPhase::WaitingForNewGame { game, current: now_mon });
                    }
                }
            }
            QuickStartPhase::WaitingForStarter(_) => {
                if filled {
                    let current = keys.current(store);
                    log::info!("[Quick Start] first Pokémon received: {} Lv{}; settling for {:?}", current.species, current.level, SETTLE_TIME);
                    self.settle = Some((now, now + SETTLE_TIME));
                    self.set_phase(QuickStartPhase::Settling { game, current });
                }
            }
            QuickStartPhase::Settling { current, .. } => {
                if !filled {
                    log::info!("[Quick Start] the Pokémon left slot 1 while settling; waiting again");
                    self.settle = None;
                    self.set_phase(QuickStartPhase::WaitingForStarter(game));
                } else if let Some((first, deadline)) = self.settle {
                    // a field still moving: give it another second (within the cap)
                    let extended = (now + SETTLE_TIME).min(first + SETTLE_CAP).max(deadline);
                    self.settle = Some((first, extended));
                    let now_mon = keys.current(store);
                    if now_mon != current {
                        self.set_phase(QuickStartPhase::Settling { game, current: now_mon });
                    }
                }
            }
            _ => {}
        }
    }

    fn finish(&mut self, store: &PropertyStore) {
        let (Some(game), Some(keys)) = (self.game.clone(), self.keys.clone()) else { return };
        let info = keys.read(store, &game);
        log::info!("[Quick Start] route parameters read from the game: {}", info.summary());
        self.done = true;
        self.settle = None;
        self.set_phase(QuickStartPhase::Done(info));
    }
}

impl SessionEvents for StarterWatch {
    fn on_connected(&mut self) {
        self.set_phase(QuickStartPhase::Connecting("Connected to GameHook; loading the mapper...".to_string()));
    }

    fn on_connection_error(&mut self) {
        if self.done || matches!(self.phase(), QuickStartPhase::Failed(_)) {
            return;
        }
        self.game = None;
        self.keys = None;
        self.settle = None;
        self.set_phase(QuickStartPhase::Connecting(format!("Could not connect to GameHook at {}. Is it running? Retrying...", self.url)));
    }

    fn on_disconnected(&mut self) {
        // a GameHook error stops the client for good: keep showing it
        if self.done || matches!(self.phase(), QuickStartPhase::Failed(_)) {
            return;
        }
        self.game = None;
        self.keys = None;
        self.settle = None;
        self.set_phase(QuickStartPhase::Connecting("Lost the connection to GameHook; reconnecting...".to_string()));
    }

    fn on_game_hook_error(&mut self, err: &str) {
        log::error!("[Quick Start] GameHook error: {}", err);
        // while a game is being watched the error is usually transient (the
        // recorder's own session also just logs it); the client tells us via
        // `on_shutdown` when it gave up
        if self.done || self.phase().game().is_some() {
            return;
        }
        self.set_phase(QuickStartPhase::Failed(format!("GameHook error: {}", err)));
    }

    fn on_driver_error(&mut self, err: &str) {
        log::error!("[Quick Start] driver error: {}", err);
    }

    fn on_mapper_loaded(&mut self, store: &PropertyStore) -> Vec<String> {
        if self.done {
            return Vec::new();
        }
        let name = store.game_name().unwrap_or("").to_string();
        let Some(game) = detect_game(&name) else {
            log::warn!("[Quick Start] mapper '{}' is not a game the router knows", name);
            self.game = None;
            self.keys = None;
            self.set_phase(QuickStartPhase::UnsupportedGame(name));
            return Vec::new();
        };
        let Some(keys) = SlotKeys::resolve(store) else {
            self.set_phase(QuickStartPhase::Failed(format!("The '{}' mapper has no player.team.0.species property, so the first Pokémon cannot be detected.", name)));
            return Vec::new();
        };
        log::info!("[Quick Start] mapper '{}' -> {} (gen {}); slot keys: {:?}", name, game.version, game.generation, keys);
        let watched = keys.all();
        self.settle = None;
        if keys.filled(store) {
            let current = keys.current(store);
            log::info!("[Quick Start] slot 1 already holds {} Lv{}; waiting for a new game (or the user's say-so)", current.species, current.level);
            self.set_phase(QuickStartPhase::WaitingForNewGame { game: game.clone(), current });
        } else {
            self.set_phase(QuickStartPhase::WaitingForStarter(game.clone()));
        }
        self.game = Some(game);
        self.keys = Some(keys);
        watched
    }

    fn on_mapper_load_error(&mut self, _err: &str) {
        if self.done {
            return;
        }
        self.game = None;
        self.keys = None;
        self.settle = None;
        self.set_phase(QuickStartPhase::NoMapper);
    }

    fn on_property_changed(&mut self, store: &PropertyStore, _new: &GameHookProperty, _old: &GameHookProperty) {
        self.evaluate(store, Instant::now());
    }

    fn on_shutdown(&mut self) {
        // the client only shuts down on its own after a fatal error (a
        // cancel or hand-off has `stop` called first, and `done` set)
        if self.done || self.stopped.load(Ordering::SeqCst) || matches!(self.phase(), QuickStartPhase::Failed(_)) {
            return;
        }
        log::error!("[Quick Start] the GameHook client shut down before the first Pokémon was read");
        self.set_phase(QuickStartPhase::Failed("The GameHook connection shut down after an error (see the log). Cancel and press Start Recording again.".to_string()));
    }

    fn on_idle(&mut self, store: &PropertyStore) {
        if self.done {
            return;
        }
        let now = Instant::now();
        if self.accept_current.swap(false, Ordering::SeqCst) && matches!(self.phase(), QuickStartPhase::WaitingForNewGame { .. }) && self.keys.as_ref().map(|k| k.filled(store)).unwrap_or(false) {
            log::info!("[Quick Start] using the Pokémon already in slot 1");
            self.finish(store);
            return;
        }
        if let Some((_, deadline)) = self.settle {
            if now >= deadline {
                if self.keys.as_ref().map(|k| k.filled(store)).unwrap_or(false) {
                    self.finish(store);
                } else {
                    self.evaluate(store, now);
                }
            }
        }
    }
}

/// The application's handle: owns the GameHook connection of a quick start.
pub struct QuickStart {
    client: Arc<GameHookClient>,
    phase: Arc<Mutex<QuickStartPhase>>,
    accept_current: Arc<AtomicBool>,
    url: String,
    stopped: Arc<AtomicBool>,
}

impl QuickStart {
    /// Connect to GameHook at `url` and start watching. `wake` is invoked
    /// whenever the phase changes (the app requests a repaint).
    pub fn start(url: &str, wake: Arc<dyn Fn() + Send + Sync>) -> QuickStart {
        let phase = Arc::new(Mutex::new(QuickStartPhase::Connecting(format!("Connecting to GameHook at {}...", url))));
        let accept_current = Arc::new(AtomicBool::new(false));
        let stopped = Arc::new(AtomicBool::new(false));
        let watch = StarterWatch {
            phase: phase.clone(),
            accept_current: accept_current.clone(),
            stopped: stopped.clone(),
            wake,
            url: url.to_string(),
            game: None,
            keys: None,
            settle: None,
            done: false,
        };
        log::info!("[Quick Start] connecting to {}", url);
        let client = GameHookClient::connect(url, Box::new(watch));
        QuickStart { client, phase, accept_current, url: url.to_string(), stopped }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn phase(&self) -> QuickStartPhase {
        self.phase.lock().unwrap().clone()
    }

    /// The user wants the Pokémon already in slot 1 (no new game).
    pub fn accept_current_pokemon(&self) {
        self.accept_current.store(true, Ordering::SeqCst);
    }

    /// Retry now / reload the mapper (the status panel's button).
    pub fn reconnect(&self) {
        self.client.reconnect();
    }

    /// Drop the GameHook connection (the app does this before the real
    /// recorder connects, and on cancel).
    pub fn stop(&self) {
        if !self.stopped.swap(true, Ordering::SeqCst) {
            self.client.disconnect();
        }
    }
}

impl Drop for QuickStart {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mapper_names_map_to_versions() {
        let cases = [
            ("Pokemon Yellow", consts::YELLOW_VERSION),
            ("Pokemon Yellow - Deprecated Mapper", consts::YELLOW_VERSION),
            ("Pokemon Yellow Legacy - Deprecated Mapper", consts::YELLOW_VERSION),
            ("Pokemon Red and Blue", consts::RED_VERSION),
            ("Pokemon Red/Blue", consts::RED_VERSION),
            ("Pokemon Crystal", consts::CRYSTAL_VERSION),
            ("Pokemon Gold and Silver", consts::GOLD_VERSION),
            ("Pokemon Emerald", consts::EMERALD_VERSION),
            ("STP Pokemon Emerald", consts::EMERALD_VERSION),
            ("Pokemon FireRed and LeafGreen", consts::FIRE_RED_VERSION),
            ("Pokemon FireRed & LeafGreen - Deprecated Mapper", consts::FIRE_RED_VERSION),
            ("Pokemon Ruby and Sapphire", consts::RUBY_VERSION),
            ("Pokemon Platinum", consts::PLATINUM_VERSION),
            ("Pokemon Diamond and Pearl - Beta", consts::DIAMOND_VERSION),
            ("Pokemon HeartGold and SoulSilver - Beta", consts::HEART_GOLD_VERSION),
            ("Pokemon HeartGold", consts::HEART_GOLD_VERSION),
            ("Pokemon SoulSilver", consts::SOUL_SILVER_VERSION),
            ("Pokemon Black - Beta", consts::BLACK_VERSION),
            ("Pokemon White - Beta", consts::WHITE_VERSION),
            ("Pokemon Black 2 - Beta", consts::BLACK_2_VERSION),
            ("Pokemon White 2 - Beta", consts::WHITE_2_VERSION),
        ];
        for (name, version) in cases {
            assert_eq!(detect_version(name), Some(version), "{}", name);
            let game = detect_game(name).unwrap();
            assert_eq!(game.version, version);
        }
        assert_eq!(detect_version("Pokémon Trading Card Game"), None);
        assert_eq!(detect_game("Pokemon Black 2 - Beta").unwrap().generation, 5);
        assert_eq!(detect_game("Pokemon Yellow").unwrap().generation, 1);
        assert_eq!(detect_game("Pokemon Yellow").unwrap().sibling, None);
        assert_eq!(detect_game("Pokemon Red and Blue").unwrap().sibling.as_deref(), Some(consts::BLUE_VERSION));
        assert_eq!(detect_game("Pokemon Red/Blue").unwrap().sibling.as_deref(), Some(consts::BLUE_VERSION));
        assert_eq!(detect_game("Pokemon FireRed and LeafGreen").unwrap().sibling.as_deref(), Some(consts::LEAF_GREEN_VERSION));
        assert_eq!(detect_game("Pokemon HeartGold and SoulSilver - Beta").unwrap().sibling.as_deref(), Some(consts::SOUL_SILVER_VERSION));
        assert_eq!(detect_game("Pokemon HeartGold").unwrap().sibling, None);
        assert_eq!(detect_game("Pokemon Platinum").unwrap().sibling, None);
    }

    #[test]
    fn ability_index_from_each_mapper_flavour() {
        let two = vec!["Blaze".to_string(), "Solar Power".to_string()];
        let one = vec!["Blaze".to_string(), String::new()];
        assert_eq!(resolve_ability_idx(&json!(false), &two), 0);
        assert_eq!(resolve_ability_idx(&json!(true), &two), 1);
        assert_eq!(resolve_ability_idx(&json!(true), &one), 0);
        assert_eq!(resolve_ability_idx(&json!("Solar Power"), &two), 1);
        assert_eq!(resolve_ability_idx(&json!("solarpower"), &two), 1);
        assert_eq!(resolve_ability_idx(&json!("Torrent"), &two), 0);
        assert_eq!(resolve_ability_idx(&json!(1), &two), 1);
        assert_eq!(resolve_ability_idx(&json!(7), &two), 0);
        assert_eq!(resolve_ability_idx(&Value::Null, &two), 0);
    }

    fn store(props: Value) -> PropertyStore {
        let mut list = Vec::new();
        for (k, v) in props.as_object().unwrap() {
            list.push(json!({"path": k, "value": v, "bytes": null}));
        }
        PropertyStore::from_mapper(&json!({"meta": {"gameName": "Pokemon Yellow"}, "properties": list})).unwrap()
    }

    #[test]
    fn gen1_deprecated_slot_read_derives_hp_dv() {
        let s = store(json!({
            "player.teamCount": 1,
            "player.team.0.species": "Charmander",
            "player.team.0.level": 5,
            "player.team.0.dvAttack": 15,
            "player.team.0.dvDefense": 14,
            "player.team.0.dvSpeed": 13,
            "player.team.0.dvSpecial": 12,
        }));
        let keys = SlotKeys::resolve(&s).unwrap();
        assert!(keys.filled(&s));
        let game = detect_game("Pokemon Yellow - Deprecated Mapper").unwrap();
        let info = keys.read(&s, &game);
        assert_eq!(info.species, "Charmander");
        assert_eq!(info.level, 5);
        // HP DV = odd(atk)*8 + odd(def)*4 + odd(spd)*2 + odd(spc) = 8 + 0 + 2 + 0
        assert_eq!((info.dvs.hp, info.dvs.attack, info.dvs.defense, info.dvs.speed, info.dvs.special_attack, info.dvs.special_defense), (10, 15, 14, 13, 12, 12));
        assert!(info.nature.is_none());
    }

    #[test]
    fn gen1_standard_slot_uses_ivs_paths_and_empty_party() {
        let mut s = store(json!({
            "player.team_count": 0,
            "player.team.0.species": null,
            "player.team.0.level": 0,
            "player.team.0.ivs.attack": 0,
            "player.team.0.ivs.defense": 0,
            "player.team.0.ivs.speed": 0,
            "player.team.0.ivs.special": 0,
        }));
        let keys = SlotKeys::resolve(&s).unwrap();
        assert!(!keys.filled(&s));
        s.set_value("player.team.0.species", json!("Pikachu"));
        // the count still says empty: a title screen showing stale slot data
        assert!(!keys.filled(&s));
        s.set_value("player.team_count", json!(1));
        assert!(keys.filled(&s));
        s.set_value("player.team.0.ivs.attack", json!(9));
        s.set_value("player.team.0.ivs.special", json!(7));
        let info = keys.read(&s, &detect_game("Pokemon Yellow").unwrap());
        assert_eq!(info.dvs.hp, 9);
        assert_eq!(info.dvs.attack, 9);
        assert_eq!(info.dvs.special_attack, 7);
    }

    #[test]
    fn gen3_slot_reads_nature_and_ability_bit() {
        let s = store(json!({
            "player.teamCount": 1,
            "player.team.0.species": "Torchic",
            "player.team.0.level": 5,
            "player.team.0.ivHp": 31,
            "player.team.0.ivAttack": 30,
            "player.team.0.ivDefense": 29,
            "player.team.0.ivSpeed": 28,
            "player.team.0.ivSpecialAttack": 27,
            "player.team.0.ivSpecialDefense": 26,
            "player.team.0.nature": "Adamant",
            "player.team.0.ability": true,
        }));
        let keys = SlotKeys::resolve(&s).unwrap();
        let info = keys.read(&s, &detect_game("Pokemon Emerald").unwrap());
        assert_eq!((info.dvs.hp, info.dvs.attack, info.dvs.defense, info.dvs.speed, info.dvs.special_attack, info.dvs.special_defense), (31, 30, 29, 28, 27, 26));
        assert_eq!(info.nature, Nature::from_name("Adamant"));
        assert_eq!(info.ability, json!(true));
    }

    #[test]
    fn gen4_slot_reads_numeric_nature_and_ability_name() {
        let s = store(json!({
            "player.team_count": 1,
            "player.team.0.species": "Chimchar",
            "player.team.0.level": 5,
            "player.team.0.ivs.hp": 1,
            "player.team.0.ivs.attack": 2,
            "player.team.0.ivs.defense": 3,
            "player.team.0.ivs.speed": 4,
            "player.team.0.ivs.special_attack": 5,
            "player.team.0.ivs.special_defense": 6,
            "player.team.0.nature": 13,
            "player.team.0.ability": "Blaze",
        }));
        let keys = SlotKeys::resolve(&s).unwrap();
        let info = keys.read(&s, &detect_game("Pokemon Platinum").unwrap());
        assert_eq!((info.dvs.hp, info.dvs.attack, info.dvs.defense, info.dvs.speed, info.dvs.special_attack, info.dvs.special_defense), (1, 2, 3, 4, 5, 6));
        assert_eq!(info.nature, Nature::from_index(13));
        assert_eq!(info.ability, json!("Blaze"));
        assert_eq!(resolve_ability_idx(&info.ability, &["Blaze".to_string(), "Iron Fist".to_string()]), 0);
    }

    #[test]
    fn watch_waits_for_an_empty_party_then_settles() {
        let phase = Arc::new(Mutex::new(QuickStartPhase::Connecting(String::new())));
        let accept = Arc::new(AtomicBool::new(false));
        let mut w = StarterWatch {
            phase: phase.clone(),
            accept_current: accept.clone(),
            stopped: Arc::new(AtomicBool::new(false)),
            wake: Arc::new(|| {}),
            url: String::new(),
            game: None,
            keys: None,
            settle: None,
            done: false,
        };
        let mut s = store(json!({
            "player.teamCount": 1,
            "player.team.0.species": "Squirtle",
            "player.team.0.level": 20,
            "player.team.0.dvAttack": 1,
            "player.team.0.dvDefense": 1,
            "player.team.0.dvSpeed": 1,
            "player.team.0.dvSpecial": 1,
        }));
        let watched = w.on_mapper_loaded(&s);
        assert!(watched.contains(&"player.team.0.species".to_string()));
        assert!(matches!(w.phase(), QuickStartPhase::WaitingForNewGame { ref current, .. } if current.species == "Squirtle" && current.level == 20));
        // the player starts a new game: the party empties
        let (n, o) = s.set_value("player.teamCount", json!(0)).unwrap();
        w.on_property_changed(&s, &n, &o);
        assert!(matches!(w.phase(), QuickStartPhase::WaitingForStarter(_)));
        // the starter arrives, species first, DVs a moment later
        let t0 = Instant::now();
        s.set_value("player.team.0.species", json!("Charmander"));
        s.set_value("player.team.0.level", json!(5));
        let (n, o) = s.set_value("player.teamCount", json!(1)).unwrap();
        w.on_property_changed(&s, &n, &o);
        assert!(matches!(w.phase(), QuickStartPhase::Settling { ref current, .. } if current.species == "Charmander"));
        let (first, deadline) = w.settle.unwrap();
        assert!(first >= t0 && deadline >= first + SETTLE_TIME);
        // not yet: idle before the deadline reads nothing
        w.on_idle(&s);
        assert!(matches!(w.phase(), QuickStartPhase::Settling { .. }));
        let (n, o) = s.set_value("player.team.0.dvAttack", json!(11)).unwrap();
        w.on_property_changed(&s, &n, &o);
        // force the deadline to now and let idle finish the job
        w.settle = Some((first, Instant::now()));
        w.on_idle(&s);
        match w.phase() {
            QuickStartPhase::Done(info) => {
                assert_eq!(info.species, "Charmander");
                assert_eq!(info.level, 5);
                assert_eq!(info.dvs.attack, 11);
                assert_eq!(info.game.version, consts::YELLOW_VERSION);
            }
            other => panic!("expected Done, got {:?}", other),
        }
        // done stays done
        let (n, o) = s.set_value("player.team.0.species", json!(null)).unwrap();
        w.on_property_changed(&s, &n, &o);
        assert!(matches!(w.phase(), QuickStartPhase::Done(_)));
    }

    #[test]
    fn watch_accepts_the_current_pokemon_on_request() {
        let phase = Arc::new(Mutex::new(QuickStartPhase::Connecting(String::new())));
        let accept = Arc::new(AtomicBool::new(false));
        let mut w = StarterWatch {
            phase: phase.clone(),
            accept_current: accept.clone(),
            stopped: Arc::new(AtomicBool::new(false)),
            wake: Arc::new(|| {}),
            url: String::new(),
            game: None,
            keys: None,
            settle: None,
            done: false,
        };
        let s = store(json!({
            "player.team_count": 1,
            "player.team.0.species": "Totodile",
            "player.team.0.level": 5,
            "player.team.0.ivs.attack": 15,
            "player.team.0.ivs.defense": 15,
            "player.team.0.ivs.speed": 15,
            "player.team.0.ivs.special": 15,
        }));
        let s = PropertyStore::from_mapper(&json!({
            "meta": {"gameName": "Pokemon Crystal"},
            "properties": s.paths().iter().map(|p| json!({"path": p, "value": s.get_value(Some(p)), "bytes": null})).collect::<Vec<_>>()
        })).unwrap();
        w.on_mapper_loaded(&s);
        assert!(matches!(w.phase(), QuickStartPhase::WaitingForNewGame { .. }));
        w.on_idle(&s);
        assert!(matches!(w.phase(), QuickStartPhase::WaitingForNewGame { .. }));
        accept.store(true, Ordering::SeqCst);
        w.on_idle(&s);
        match w.phase() {
            QuickStartPhase::Done(info) => {
                assert_eq!(info.game.version, consts::CRYSTAL_VERSION);
                assert_eq!(info.species, "Totodile");
                assert_eq!(info.dvs.hp, 15);
            }
            other => panic!("expected Done, got {:?}", other),
        }
    }

    #[test]
    fn unknown_mapper_and_missing_species_key() {
        let phase = Arc::new(Mutex::new(QuickStartPhase::Connecting(String::new())));
        let mut w = StarterWatch {
            phase: phase.clone(),
            accept_current: Arc::new(AtomicBool::new(false)),
            stopped: Arc::new(AtomicBool::new(false)),
            wake: Arc::new(|| {}),
            url: String::new(),
            game: None,
            keys: None,
            settle: None,
            done: false,
        };
        let s = PropertyStore::from_mapper(&json!({"meta": {"gameName": "Pokémon Trading Card Game"}, "properties": [{"path": "x", "value": 1}]})).unwrap();
        assert!(w.on_mapper_loaded(&s).is_empty());
        assert!(matches!(w.phase(), QuickStartPhase::UnsupportedGame(_)));
        let s = PropertyStore::from_mapper(&json!({"meta": {"gameName": "Pokemon Yellow"}, "properties": [{"path": "x", "value": 1}]})).unwrap();
        assert!(w.on_mapper_loaded(&s).is_empty());
        assert!(matches!(w.phase(), QuickStartPhase::Failed(_)));
    }
}
