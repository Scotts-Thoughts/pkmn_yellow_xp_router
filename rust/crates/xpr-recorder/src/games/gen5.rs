//! Black/White and Black 2/White 2, read through the Poke-A-Byte gen 5 mappers.
//!
//! Gen 5 memory does not behave like gen 4's, so this machine has its own
//! logic (it shares gen 4's name conversion and event processing). What it is
//! built around, measured on recorded runs (`docs/rust_port/recording/gen5.md`):
//!
//! - A battle is on while `battle.other.battle` holds [`BATTLE_WORD`]; the
//!   outcome is decided when `battle.other.battle_end` becomes 1. (The
//!   mappers' `meta.state` is built from the same values, but in White 2 it
//!   can stay at `To Battle` for a whole battle.)
//! - The party (`player.team.N`) only changes when a battle ends: the
//!   experience, levels, moves, EVs and held item of a whole battle land in one
//!   update, together with the prize money, after `battle_end` is set and
//!   before the battle word is cleared.
//! - In battle the progress lives in one battle struct per party slot
//!   (`battle.{player,opponent}.pkmn_ram.pokemon_N_ram`, N = party slot, not
//!   field position). An enemy has fainted when its struct's HP drops to 0.
//! - `battle.opponent.id` holds an unrelated value in the overworld, is 0 when
//!   a battle starts and gets the trainer id a few frames later, so a battle
//!   whose id is still 0 is not yet known to be a wild one.
//! - The game decrypts a party Pokémon in place while it edits it (menus,
//!   items, the move order) and the mapper reads garbage for a few frames, so
//!   party and bag changes are only taken from a snapshot that has stayed the
//!   same for [`SETTLE`] and passes sanity checks. Every decision is made in
//!   `on_idle`, after a whole batch of changes has been applied.
//! - TMs are not used up when taught.
//! - After a Rare Candy or an evolution the level (species) changes first and
//!   the forget-a-move choice arrives later, after the dialogue.
//! - A save shows as `flags.new_game` flipping when it is bit 0 of the save
//!   counter in the footer of the trainer-info save block. The game only
//!   rewrites the blocks that changed, and that one holds the play time, so it
//!   is rewritten by every save.
//! - A reset empties the party. Black/White and White 2 clear the player id
//!   with it, Black 2 does not; Black 2 / White 2 then load the save while the
//!   title screen menus are up and read garbage as the player id until the
//!   game goes on. So after a reset the save file is recognised by its
//!   Pokémon, and what the route keeps is decided by comparing the loaded save
//!   with the last saved and the pre-reset state.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use serde_json::Value;

use xpr_core::consts;
use xpr_data::model::CustomMoveData;
use xpr_data::GenData;
use xpr_engine::{EventDefinition, HoldItemEventDefinition, InventoryEventDefinition, LearnMoveEventDefinition, LevelVal, TrainerEventDefinition, VitaminEventDefinition, WildPkmnEventDefinition};

use super::common::*;
use super::gen45::{process_one, reset_flag, roar_flag, trainer_loss_flag, Flavor, Gen45Converter, RETURN_MOVE_NAME};
use crate::controller::{is_set, new_active_flag, ActiveFlag, EventQueue, GameRecorder, GameState, RecorderController};
use crate::gamehook::{value_as_i64, GameHookProperty, PropertyStore};
use crate::host::StartInfo;

/// Where `flags.new_game` must be read from for every save to be seen: the save
/// counter of the trainer-info block (Black, White, Black 2, White 2). The
/// Black 2 and White 2 mappers read an unrelated byte (0x221DA14 - 0x40 and
/// 0x221DA14), which never changes.
const SAVE_COUNTER_ADDRESSES: [i64; 4] = [0x2235014, 0x2235034, 0x221EA94, 0x221EAD4];
/// The party block's save counter, which the Black and White mappers read: a
/// save made with the party exactly as it was at the previous save does not
/// rewrite that block, so it goes unseen.
const PARTY_BLOCK_COUNTERS: [i64; 2] = [0x2234EE0, 0x2234F00];
/// `battle.other.battle` while a battle is on (the same in both games).
const BATTLE_WORD: i64 = 21828;
/// How long the party and bag must stay unchanged before a change is read.
const SETTLE: Duration = Duration::from_millis(400);
/// How long an overworld snapshot may stay unreadable before it is logged.
const INVALID_WARN: Duration = Duration::from_secs(10);

const VITAMINS: [&str; 6] = ["HP Up", "Protein", "Iron", "Calcium", "Zinc", "Carbos"];
const EV_BERRIES: [&str; 6] = ["Pomeg Berry", "Kelpsy Berry", "Qualot Berry", "Hondew Berry", "Grepa Berry", "Tamato Berry"];
/// The mappers' `ivs.*` / `evs.*` names, in the order `Mon` keeps them.
const STAT_NAMES: [&str; 6] = ["hp", "attack", "defense", "speed", "special_attack", "special_defense"];

/// The property paths this machine reads. Party and battle-struct paths are
/// per slot; the bag pockets are discovered from the loaded mapper.
#[derive(Clone, Debug, Default)]
pub struct Gen5Keys {
    pub meta_state: String,
    pub player_id: String,
    pub team_count: String,
    pub money: String,
    pub map_name: String,
    pub map_index: String,
    pub gametime_seconds: String,
    /// bit 0 of a save block's save counter
    pub save_flag: String,
    pub species: Vec<String>,
    pub level: Vec<String>,
    pub exp: Vec<String>,
    pub held: Vec<String>,
    pub friendship: Vec<String>,
    pub pid: Vec<String>,
    /// `flags.skip_checksum`: 3 while the game has the Pokémon decrypted in place
    pub decrypted: Vec<String>,
    pub hp: Vec<String>,
    pub max_hp: Vec<String>,
    pub moves: Vec<[String; 4]>,
    pub ivs: Vec<[String; 6]>,
    pub evs: Vec<[String; 6]>,
    /// `(item, quantity)` per bag slot, every pocket the mapper maps
    pub bag: Vec<(String, String)>,
    pub battle_word: String,
    pub battle_end: String,
    pub trainer_a: String,
    pub trainer_b: String,
    pub opp_count: String,
    pub opp_pos: String,
    pub player_pos: String,
    pub opp_struct_species: Vec<String>,
    pub opp_struct_level: Vec<String>,
    pub opp_struct_hp: Vec<String>,
    pub player_struct_hp: Vec<String>,
    pub opp_held: Vec<String>,
}

impl Gen5Keys {
    pub fn configure(store: &PropertyStore) -> Gen5Keys {
        let team = |suffix: &str| -> Vec<String> { (0..6).map(|i| format!("player.team.{}.{}", i, suffix)).collect() };
        let mut bag = Vec::new();
        for pocket in ["items", "medicine", "berries", "tmhm"] {
            let mut i = 0;
            loop {
                let item = format!("bag.{}.{}.item", pocket, i);
                if !store.contains(&item) {
                    break;
                }
                bag.push((item, format!("bag.{}.{}.quantity", pocket, i)));
                i += 1;
            }
        }
        Gen5Keys {
            meta_state: "meta.state".into(),
            player_id: "player.player_id".into(),
            team_count: "player.team_count".into(),
            money: "bag.money".into(),
            map_name: "overworld.map_name".into(),
            map_index: "overworld.map_index".into(),
            gametime_seconds: "game_time.seconds".into(),
            save_flag: "flags.new_game".into(),
            species: team("species"),
            level: team("level"),
            exp: team("exp"),
            held: team("held_item"),
            friendship: team("friendship"),
            pid: team("internals.personality_value"),
            decrypted: team("flags.skip_checksum"),
            hp: team("stats.hp"),
            max_hp: team("stats.hp_max"),
            moves: (0..6).map(|i| std::array::from_fn(|m| format!("player.team.{}.moves.{}.move", i, m))).collect(),
            ivs: (0..6).map(|i| std::array::from_fn(|s| format!("player.team.{}.ivs.{}", i, STAT_NAMES[s]))).collect(),
            evs: (0..6).map(|i| std::array::from_fn(|s| format!("player.team.{}.evs.{}", i, STAT_NAMES[s]))).collect(),
            bag,
            battle_word: "battle.other.battle".into(),
            battle_end: "battle.other.battle_end".into(),
            trainer_a: "battle.opponent.id".into(),
            trainer_b: "battle.opponent_2.id".into(),
            opp_count: "battle.opponent.team_count".into(),
            opp_pos: "battle.opponent.party_position".into(),
            player_pos: "battle.player.party_position".into(),
            opp_struct_species: (0..6).map(|i| format!("battle.opponent.pkmn_ram.pokemon_{}_ram.species", i)).collect(),
            opp_struct_level: (0..6).map(|i| format!("battle.opponent.pkmn_ram.pokemon_{}_ram.level", i)).collect(),
            opp_struct_hp: (0..6).map(|i| format!("battle.opponent.pkmn_ram.pokemon_{}_ram.stats.hp", i)).collect(),
            player_struct_hp: (0..6).map(|i| format!("battle.player.pkmn_ram.pokemon_{}_ram.stats.hp", i)).collect(),
            opp_held: (0..6).map(|i| format!("battle.opponent.team.{}.held_item", i)).collect(),
        }
    }

    /// Every path to register.
    pub fn all(&self) -> Vec<String> {
        let mut v = vec![
            self.meta_state.clone(),
            self.player_id.clone(),
            self.team_count.clone(),
            self.money.clone(),
            self.map_name.clone(),
            self.map_index.clone(),
            self.gametime_seconds.clone(),
            self.save_flag.clone(),
            self.battle_word.clone(),
            self.battle_end.clone(),
            self.trainer_a.clone(),
            self.trainer_b.clone(),
            self.opp_count.clone(),
            self.opp_pos.clone(),
            self.player_pos.clone(),
        ];
        for list in [&self.species, &self.level, &self.exp, &self.held, &self.friendship, &self.pid, &self.decrypted, &self.hp, &self.max_hp] {
            v.extend(list.iter().cloned());
        }
        for slot in &self.moves {
            v.extend(slot.iter().cloned());
        }
        for slot in self.ivs.iter().chain(self.evs.iter()) {
            v.extend(slot.iter().cloned());
        }
        for (i, q) in &self.bag {
            v.push(i.clone());
            v.push(q.clone());
        }
        for list in [&self.opp_struct_species, &self.opp_struct_level, &self.opp_struct_hp, &self.player_struct_hp, &self.opp_held] {
            v.extend(list.iter().cloned());
        }
        v
    }

    /// The party / bag / money paths: a change to one restarts the settle timer.
    fn settle_paths(&self) -> HashSet<String> {
        let mut s: HashSet<String> = HashSet::new();
        s.insert(self.team_count.clone());
        s.insert(self.money.clone());
        s.insert(self.player_id.clone());
        // HP too: a Pokémon Center heal changes nothing else
        for list in [&self.species, &self.level, &self.exp, &self.held, &self.pid, &self.decrypted, &self.hp, &self.max_hp] {
            s.extend(list.iter().cloned());
        }
        for slot in &self.moves {
            s.extend(slot.iter().cloned());
        }
        for slot in &self.evs {
            s.extend(slot.iter().cloned());
        }
        for (i, q) in &self.bag {
            s.insert(i.clone());
            s.insert(q.clone());
        }
        s
    }
}

/// One party Pokémon as the router names things.
#[derive(Clone, Debug, PartialEq)]
struct Mon {
    slot: usize,
    pid: i64,
    species: String,
    level: i64,
    exp: i64,
    moves: [Option<String>; 4],
    held: Option<String>,
    friendship: i64,
    /// hp, attack, defense, speed, special attack, special defense
    ivs: [i64; 6],
    evs: [i64; 6],
    hp: i64,
    max_hp: i64,
}

/// The party, bag and money at one moment, all read from one settled store.
#[derive(Clone, Debug)]
struct Snap {
    team: Vec<Mon>,
    /// router item name -> count
    items: IndexMap<String, i64>,
    money: i64,
    player_id: i64,
}

impl Snap {
    fn by_pid(&self, pid: i64) -> Option<&Mon> {
        self.team.iter().find(|m| m.pid == pid)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// waiting for a loaded save with a readable party
    Uninitialized,
    Overworld,
    Battle,
    /// the game went back to the title screen
    Resetting,
}

/// Everything followed while a battle is on.
#[derive(Debug)]
struct BattleTrack {
    /// the settled overworld state the battle started from
    pre: Snap,
    trainer_a: i64,
    trainer_b: i64,
    trainer_event_queued: bool,
    /// `None` until the battle is known to be against a trainer or not
    is_trainer: Option<bool>,
    /// HP per opponent party slot at the last look, and whether it was ever above 0
    opp_hp: [i64; 6],
    opp_seen_alive: [bool; 6],
    opp_fainted: [bool; 6],
    /// (slot, species, level) in the order they fainted
    faints: Vec<(usize, String, i64)>,
    /// opponent party slots in the order they were first sent out
    appear_order: Vec<usize>,
    /// player party slots that fought each opponent party slot
    participants: [HashSet<usize>; 6],
    player_hp: [i64; 6],
    player_seen_alive: [bool; 6],
    player_fainted: [bool; 6],
    all_player_fainted: bool,
    /// opponent party slot whose held item went missing (Thief / Covet)
    opp_held: [Option<String>; 6],
    opp_lost_item: Vec<usize>,
    /// `meta.state` left `Battle`
    over: bool,
}

impl BattleTrack {
    fn new(pre: Snap) -> BattleTrack {
        BattleTrack {
            pre,
            trainer_a: 0,
            trainer_b: 0,
            trainer_event_queued: false,
            is_trainer: None,
            opp_hp: [0; 6],
            opp_seen_alive: [false; 6],
            opp_fainted: [false; 6],
            faints: Vec::new(),
            appear_order: Vec::new(),
            participants: Default::default(),
            player_hp: [0; 6],
            player_seen_alive: [false; 6],
            player_fainted: [false; 6],
            all_player_fainted: false,
            opp_held: Default::default(),
            opp_lost_item: Vec::new(),
            over: false,
        }
    }
}

pub struct Gen5Machine {
    controller: Arc<RecorderController>,
    gen: Arc<GenData>,
    conv: Gen45Converter,
    b2w2: bool,
    debug_mode: bool,
    keys: Gen5Keys,
    settle_paths: HashSet<String>,
    active: ActiveFlag,
    queue: Arc<EventQueue>,

    phase: Phase,
    player_id: Option<i64>,
    /// the route's solo mon (species and IVs), to find it in the party
    route_species: Option<String>,
    route_ivs: Option<[i64; 6]>,
    /// the solo mon's personality value once it has been found
    solo_pid: Option<i64>,
    /// the last overworld state events were generated up to
    baseline: Option<Snap>,
    /// party/bag data changed since the baseline, and when it last changed
    dirty: bool,
    last_change: Instant,
    invalid_since: Option<Instant>,
    battle: Option<BattleTrack>,
    area: Option<String>,
    level_up_moves: HashMap<(String, i64), Vec<String>>,
    /// a blackout just happened: the heal that follows it is part of it
    blackout_heal_pending: bool,
    /// the item the last beaten trainer hands over after the battle (a gym
    /// leader's TM): it is part of the fight, not an item event
    pending_reward: Option<String>,
    /// "TM83" -> "TM83 Work Up": the bag names TMs by number only
    tm_names: HashMap<String, String>,
    /// the game was saved: record it once the changes before it are in
    save_pending: bool,
    /// level-up moves offered in the overworld (a Rare Candy, an evolution)
    /// and not in the move list yet: the game asks which move to forget after
    /// the level has already changed, so the choice can arrive in a later diff
    pending_levelup: Vec<(String, i64, String)>,
    /// `flags.new_game` is read from a save counter (see [`SAVE_COUNTER_ADDRESSES`]);
    /// without it a reset only rolls events back when the loaded save matches a
    /// state the recorder knows
    saves_detectable: bool,
    warned_saves: bool,
    /// the overworld state when the last save was recorded
    saved: Option<Snap>,
    /// the overworld state when the game was reset (before a battle, for a
    /// reset during one)
    pre_reset: Option<Snap>,
    /// the game was reset: what the route keeps is decided once the save loads
    reset_pending: bool,
}

fn opt_str(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.trim().is_empty() => Some(s.clone()),
        _ => None,
    }
}

impl Gen5Machine {
    pub fn new(controller: Arc<RecorderController>, info: &StartInfo, flavor: Flavor) -> Gen5Machine {
        let route_ivs = info.dvs.as_ref().map(|d| [d.hp, d.attack, d.defense, d.speed, d.special_attack, d.special_defense]);
        Gen5Machine {
            controller,
            gen: info.gen.clone(),
            conv: Gen45Converter { flavor },
            b2w2: flavor == Flavor::Black2White2,
            debug_mode: info.debug_mode,
            keys: Gen5Keys::default(),
            settle_paths: HashSet::new(),
            active: new_active_flag(),
            queue: EventQueue::new(),
            phase: Phase::Uninitialized,
            player_id: None,
            route_species: info.solo_species.clone(),
            route_ivs,
            solo_pid: None,
            baseline: None,
            dirty: false,
            last_change: Instant::now(),
            invalid_since: None,
            battle: None,
            area: None,
            level_up_moves: HashMap::new(),
            blackout_heal_pending: false,
            pending_reward: None,
            tm_names: info
                .gen
                .item_db()
                .get_filtered_names(consts::ITEM_TYPE_TM, consts::ITEM_TYPE_ALL_ITEMS, None)
                .into_iter()
                .filter_map(|full| full.split(' ').next().map(|short| (short.to_string(), full.clone())))
                .collect(),
            save_pending: false,
            pending_levelup: Vec::new(),
            saves_detectable: false,
            warned_saves: false,
            saved: None,
            pre_reset: None,
            reset_pending: false,
        }
    }

    fn queue_event(&self, event: EventDefinition) {
        if self.debug_mode {
            log::info!("queueing: {}", event_str(&self.gen, &event));
        }
        self.queue.push(event);
    }

    fn set_phase(&mut self, phase: Phase) {
        if phase != self.phase {
            log::info!("[gen5] {:?} -> {:?}", self.phase, phase);
            self.phase = phase;
        }
        self.controller.set_game_state(match phase {
            Phase::Uninitialized => GameState::Uninitialized,
            Phase::Overworld => GameState::Overworld,
            Phase::Battle => GameState::Battle,
            Phase::Resetting => GameState::Resetting,
        });
    }

    // ---- reading the store --------------------------------------------------------

    fn meta_state(&self, store: &PropertyStore) -> String {
        store.str_of(&self.keys.meta_state).unwrap_or_default()
    }

    fn in_battle(&self, store: &PropertyStore) -> bool {
        Self::i64_of(store, &self.keys.battle_word) == BATTLE_WORD
    }

    fn i64_of(store: &PropertyStore, key: &str) -> i64 {
        store_i64(store, key)
    }

    fn species_name(&self, raw: &Value) -> Option<String> {
        let name = opt_str(raw)?;
        let converted = self.conv.pkmn_name_convert(Some(&name), None)?;
        self.gen.pkmn_db().get_pkmn(&converted).map(|p| p.name.clone())
    }

    /// The router's name for a mapper item name: TMs get their move, and the
    /// item data's own spelling is used (the converter title-cases, so "PP UP"
    /// comes out as "Pp Up").
    fn item_name(&self, raw: &str) -> Option<String> {
        let name = self.conv.item_name_convert(Some(raw))?;
        if let Some(full) = self.tm_names.get(&name) {
            return Some(full.clone());
        }
        Some(self.gen.item_db().get_item(&name).map(|i| i.name.clone()).unwrap_or(name))
    }

    /// One party slot, or why it cannot be trusted right now.
    fn read_mon(&self, store: &PropertyStore, slot: usize) -> Result<Mon, String> {
        let k = &self.keys;
        let raw_species = store.get_value(Some(&k.species[slot]));
        let species = self.species_name(&raw_species).ok_or_else(|| format!("slot {} species {}", slot, py_str(&raw_species)))?;
        let level = Self::i64_of(store, &k.level[slot]);
        if !(1..=100).contains(&level) {
            return Err(format!("slot {} level {}", slot, level));
        }
        if Self::i64_of(store, &k.decrypted[slot]) != 0 {
            return Err(format!("slot {} is decrypted in place", slot));
        }
        let pid = Self::i64_of(store, &k.pid[slot]);
        if pid == 0 {
            return Err(format!("slot {} has no personality value", slot));
        }
        let hp = Self::i64_of(store, &k.hp[slot]);
        let max_hp = Self::i64_of(store, &k.max_hp[slot]);
        if !(1..=999).contains(&max_hp) || !(0..=max_hp).contains(&hp) {
            return Err(format!("slot {} HP {}/{}", slot, hp, max_hp));
        }
        let moves: [Option<String>; 4] = std::array::from_fn(|m| {
            let raw = store.str_of(&k.moves[slot][m]);
            raw.and_then(|r| self.conv.move_name_convert(Some(&r)))
        });
        let held = store.str_of(&k.held[slot]).and_then(|h| self.item_name(&h));
        Ok(Mon {
            slot,
            pid,
            species,
            level,
            exp: Self::i64_of(store, &k.exp[slot]),
            moves,
            held,
            friendship: Self::i64_of(store, &k.friendship[slot]),
            ivs: std::array::from_fn(|s| Self::i64_of(store, &k.ivs[slot][s])),
            evs: std::array::from_fn(|s| Self::i64_of(store, &k.evs[slot][s])),
            hp,
            max_hp,
        })
    }

    fn read_snap(&self, store: &PropertyStore) -> Result<Snap, String> {
        let count = Self::i64_of(store, &self.keys.team_count);
        if !(1..=6).contains(&count) {
            return Err(format!("team count {}", count));
        }
        let mut team = Vec::new();
        for slot in 0..count as usize {
            team.push(self.read_mon(store, slot)?);
        }
        let mut items: IndexMap<String, i64> = IndexMap::new();
        for (item_key, qty_key) in &self.keys.bag {
            let Some(raw) = store.str_of(item_key) else { continue };
            let Some(name) = self.item_name(&raw) else { continue };
            let qty = Self::i64_of(store, qty_key);
            if qty <= 0 {
                continue;
            }
            if qty > 999 {
                return Err(format!("{} x{}", name, qty));
            }
            *items.entry(name).or_insert(0) += qty;
        }
        Ok(Snap { team, items, money: Self::i64_of(store, &self.keys.money), player_id: Self::i64_of(store, &self.keys.player_id) })
    }

    // ---- the solo mon -------------------------------------------------------------

    fn load_level_up_moves(&mut self, species: &str) {
        let Some(mon) = self.gen.pkmn_db().get_pkmn(species) else { return };
        for (level, move_name) in &mon.levelup_moves {
            let entry = self.level_up_moves.entry((mon.name.clone(), *level)).or_default();
            if !entry.contains(move_name) {
                entry.push(move_name.clone());
            }
        }
    }

    fn can_evolve_into(&self, species: &str) -> bool {
        let s = species.to_string();
        self.controller.host().call(move |h| h.can_evolve_into(&s))
    }

    /// The solo mon in `snap`: by personality value once known, else by the
    /// route's species (or an evolution of it) and IVs.
    fn find_solo<'a>(&mut self, snap: &'a Snap) -> Option<&'a Mon> {
        if let Some(pid) = self.solo_pid {
            return snap.by_pid(pid);
        }
        if self.route_species.is_none() {
            self.route_species = self.controller.host().call(|h| h.final_solo_species());
            if let Some(d) = self.controller.host().call(|h| h.get_dvs()) {
                self.route_ivs = Some([d.hp, d.attack, d.defense, d.speed, d.special_attack, d.special_defense]);
            }
        }
        let species = self.route_species.clone()?;
        let ivs_match = |m: &Mon| self.route_ivs.map(|ivs| ivs == m.ivs).unwrap_or(true);
        let found = snap
            .team
            .iter()
            .find(|m| m.species == species && ivs_match(m))
            .or_else(|| snap.team.iter().find(|m| ivs_match(m) && self.can_evolve_into(&m.species)))
            .or_else(|| {
                // a route made for a different spread still records the right species
                let m = snap.team.iter().find(|m| m.species == species)?;
                log::error!("Expected the route's IVs {:?}, but found {} with {:?}", self.route_ivs, m.species, m.ivs);
                Some(m)
            })?;
        log::info!("[gen5] solo mon found: {} Lv{} in slot {} (pid {})", found.species, found.level, found.slot + 1, found.pid);
        self.solo_pid = Some(found.pid);
        self.load_level_up_moves(&found.species.clone());
        if found.species != species {
            // the route's species is an earlier stage: the evolution is part of the route
            self.queue_event(EventDefinition::with_evolution(&found.species));
        }
        Some(found)
    }

    // ---- event helpers ------------------------------------------------------------

    fn tm_for_move(&self, move_name: &str, items: &IndexMap<String, i64>) -> Option<String> {
        items
            .keys()
            .filter(|name| name.starts_with("TM") || name.starts_with("HM"))
            .find(|name| self.gen.item_db().get_item(name).and_then(|i| i.move_name.as_deref()).map(|m| m.eq_ignore_ascii_case(move_name)).unwrap_or(false))
            .cloned()
    }

    /// Learn/ignore events for the level-up moves of `species` at every level
    /// in `from..=to`, judged against the moves before and after. With
    /// `prompt_may_follow` (a level gained in the overworld) a move not learned
    /// yet is remembered: the forget-a-move choice may only come later.
    #[allow(clippy::too_many_arguments)]
    fn level_up_events(&mut self, species: &str, from: i64, to: i64, before: &[Option<String>; 4], after: &[Option<String>; 4], handled: &mut HashSet<String>, prompt_may_follow: bool) {
        self.load_level_up_moves(species);
        let before_set: HashSet<String> = before.iter().flatten().cloned().collect();
        let after_set: HashSet<String> = after.iter().flatten().cloned().collect();
        for level in from..=to {
            let moves = self.level_up_moves.get(&(species.to_string(), level)).cloned().unwrap_or_default();
            for move_name in moves {
                let learned = after_set.contains(&move_name) && !before_set.contains(&move_name);
                let mut lm = LearnMoveEventDefinition::new(Some(&move_name), None, consts::MOVE_SOURCE_LEVELUP, LevelVal::Int(level), Some(species), false);
                if learned {
                    lm.destination_name = replaced_move(before, after, &move_name);
                    handled.insert(move_name.clone());
                    log::info!("level {} move {} learned over {:?}", level, move_name, lm.destination_name);
                } else if before_set.contains(&move_name) {
                    // already known (relearned or learned earlier): nothing changes
                    continue;
                } else {
                    log::info!("level {} move {} not learned", level, move_name);
                    if prompt_may_follow {
                        self.pending_levelup.push((species.to_string(), level, move_name.clone()));
                    }
                }
                self.queue_event(EventDefinition::with_learn_move(lm));
            }
        }
    }

    /// The solo mon evolved (same personality value, new species): the
    /// evolution, then what the new species learns at its current level.
    fn evolution_events(&mut self, o: &Mon, n: &Mon, handled: &mut HashSet<String>) {
        if n.species == o.species {
            return;
        }
        log::info!("[gen5] evolution: {} -> {}", o.species, n.species);
        self.queue_event(EventDefinition::with_evolution(&n.species));
        self.level_up_events(&n.species.clone(), n.level, n.level, &o.moves, &n.moves, handled, true);
    }

    /// Moves that changed without a level-up to explain them: TMs/HMs (TMs are
    /// not used up in gen 5), the move reminder, tutors, Sketch, the deleter.
    fn other_move_events(&mut self, before: &[Option<String>; 4], after: &[Option<String>; 4], handled: &HashSet<String>, items: &IndexMap<String, i64>, heart_scale_used: bool) {
        let before_set: HashSet<String> = before.iter().flatten().cloned().collect();
        let after_set: HashSet<String> = after.iter().flatten().cloned().collect();
        let learned: Vec<String> = after.iter().flatten().filter(|m| !before_set.contains(*m) && !handled.contains(*m)).cloned().collect();
        let forgotten: Vec<String> = before.iter().flatten().filter(|m| !after_set.contains(*m)).cloned().collect();
        for move_name in &learned {
            // the answer to a forget-a-move prompt after a Rare Candy / evolution:
            // it replaces the "not learned" event of that level
            if let Some(pos) = self.pending_levelup.iter().position(|(_, _, m)| m == move_name) {
                let (species, level, _) = self.pending_levelup.remove(pos);
                let mut lm = LearnMoveEventDefinition::new(Some(move_name), None, consts::MOVE_SOURCE_LEVELUP, LevelVal::Int(level), Some(&species), false);
                lm.destination_name = replaced_move(before, after, move_name);
                log::info!("level {} move {} learned over {:?} after the prompt", level, move_name, lm.destination_name);
                self.queue_event(EventDefinition::with_learn_move(lm));
                continue;
            }
            let source = if let Some(hm) = self.conv.get_hm_name(move_name) {
                hm.to_string()
            } else if heart_scale_used {
                consts::MOVE_SOURCE_TUTOR.to_string()
            } else if let Some(tm) = self.tm_for_move(move_name, items) {
                tm
            } else {
                consts::MOVE_SOURCE_TUTOR.to_string()
            };
            let mut lm = LearnMoveEventDefinition::new(Some(move_name), None, &source, LevelVal::any(), None, false);
            lm.destination_name = replaced_move(before, after, move_name);
            log::info!("move {} learned from {} over {:?}", move_name, source, lm.destination_name);
            self.queue_event(EventDefinition::with_learn_move(lm));
        }
        // forgotten without a replacement: the move deleter
        let replaced: HashSet<String> = learned.iter().chain(handled.iter()).filter_map(|m| replaced_move(before, after, m)).collect();
        for gone in forgotten.iter().filter(|m| !replaced.contains(*m)) {
            let mut lm = LearnMoveEventDefinition::new(None, None, consts::MOVE_SOURCE_TUTOR, LevelVal::any(), None, false);
            lm.destination_name = Some(gone.clone());
            log::info!("move {} deleted", gone);
            self.queue_event(EventDefinition::with_learn_move(lm));
        }
    }

    // ---- the overworld ------------------------------------------------------------

    /// Turn the difference between two settled overworld states into events.
    fn overworld_diff(&mut self, old: &Snap, new: &Snap) {
        let (mut gained, mut lost) = diff_items(&old.items, &new.items);
        if let Some(reward) = self.pending_reward.clone() {
            if take(&mut gained, &reward) {
                log::info!("[gen5] {} is the battle reward of the last trainer", reward);
                self.pending_reward = None;
            }
        }
        let money_change = new.money - old.money;
        if !gained.is_empty() || !lost.is_empty() || money_change != 0 {
            log::info!("[gen5] bag +{:?} -{:?}, money {:+}", gained, lost, money_change);
        }

        let old_solo = self.solo_pid.and_then(|p| old.by_pid(p)).cloned();
        let new_solo = match self.solo_pid {
            Some(p) => new.by_pid(p).cloned(),
            None => self.find_solo(new).cloned(),
        };
        if let (Some(o), Some(n)) = (&old_solo, &new_solo) {
            self.solo_changes(o, n, new, &mut gained, &mut lost);
        } else if old_solo.is_some() && new_solo.is_none() {
            log::warn!("[gen5] the solo mon left the party");
        }

        // what is left is bought, sold, found or used
        let purchase = money_change < 0 && !gained.is_empty();
        let mut sale = money_change > 0 && !lost.is_empty();
        if sale {
            let expected: i64 = lost.iter().map(|(name, n)| self.gen.item_db().get_item(name).map(|i| i.sell_price * n).unwrap_or(0)).sum();
            if expected != money_change {
                log::warn!("Money change ({}) does not match the sell price ({}) of the lost items: treating as use/drop", money_change, expected);
                sale = false;
            }
        }
        let premier_bonus = purchase && gained.iter().any(|(name, n)| *n >= 10 && ["Poke Ball", "Great Ball", "Ultra Ball"].contains(&name.as_str()));
        for (name, n) in &gained {
            if purchase && premier_bonus && name == "Premier Ball" {
                // the free Premier Ball for 10+ balls: bought balls add it
                continue;
            }
            self.queue_event(EventDefinition::with_item(InventoryEventDefinition::new(name, *n, true, purchase, None)));
            if purchase && *n >= 10 && ["Poke Ball", "Great Ball", "Ultra Ball"].contains(&name.as_str()) {
                self.queue_event(EventDefinition::with_item(InventoryEventDefinition::new("Premier Ball", 1, true, false, None)));
            }
        }
        for (name, n) in &lost {
            self.queue_event(EventDefinition::with_item(InventoryEventDefinition::new(name, *n, false, sale, None)));
        }
        if healed(old, new) && lost_nothing(old, new) {
            if self.blackout_heal_pending {
                log::info!("[gen5] the party was healed after the blackout");
            } else {
                let area = self.area.clone().unwrap_or_default();
                log::info!("[gen5] the party was healed in {}", area);
                self.queue_event(EventDefinition::with_heal(&area));
            }
            self.blackout_heal_pending = false;
        }
    }

    /// Events for what happened to the solo mon between two overworld states.
    /// Consumes the bag changes they explain from `gained` / `lost`.
    fn solo_changes(&mut self, o: &Mon, n: &Mon, new: &Snap, gained: &mut IndexMap<String, i64>, lost: &mut IndexMap<String, i64>) {
        if o.slot != n.slot {
            log::info!("[gen5] solo mon moved from slot {} to {}", o.slot + 1, n.slot + 1);
        }
        // held item: give / take / swap with the bag
        if o.held != n.held {
            match (&o.held, &n.held) {
                (_, Some(y)) if take(lost, y) => {
                    if let Some(x) = &o.held {
                        take(gained, x);
                    }
                    self.queue_event(EventDefinition::with_hold_item(HoldItemEventDefinition::new(Some(y), false)));
                }
                (Some(x), None) if take(gained, x) => {
                    self.queue_event(EventDefinition::with_hold_item(HoldItemEventDefinition::new(None, false)));
                }
                (Some(_), None) => {
                    // moved to another party member
                    self.queue_event(EventDefinition::with_hold_item(HoldItemEventDefinition::new(None, true)));
                }
                (_, Some(y)) => {
                    // moved from another party member: it was never in the bag
                    if let Some(x) = &o.held {
                        self.queue_event(EventDefinition::with_hold_item(HoldItemEventDefinition::new(None, true)));
                        log::info!("held {} replaced by {} from outside the bag", x, y);
                    }
                    self.queue_event(EventDefinition::with_item(InventoryEventDefinition::new(y, 1, true, false, None)));
                    self.queue_event(EventDefinition::with_hold_item(HoldItemEventDefinition::new(Some(y), false)));
                }
                (None, None) => {}
            }
        }
        // vitamins and EV-lowering berries
        let ev_up = (0..6).any(|s| n.evs[s] > o.evs[s]);
        let ev_down = (0..6).any(|s| n.evs[s] < o.evs[s]) || n.friendship > o.friendship;
        for vit in VITAMINS {
            if ev_up {
                if let Some(count) = lost.shift_remove(vit) {
                    self.queue_event(EventDefinition::with_vitamin(VitaminEventDefinition::new(vit, count)));
                }
            }
        }
        for berry in EV_BERRIES {
            if ev_down {
                if let Some(count) = lost.shift_remove(berry) {
                    self.queue_event(EventDefinition::with_vitamin(VitaminEventDefinition::new(berry, count)));
                }
            }
        }
        // evolution (level-up, trade, stone): the stone goes first
        if n.species != o.species {
            let stone_names: Vec<String> = lost.keys().filter(|name| name.ends_with("Stone")).cloned().collect();
            for stone in stone_names {
                if let Some(count) = lost.shift_remove(&stone) {
                    self.queue_event(EventDefinition::with_item(InventoryEventDefinition::new(&stone, count, false, false, None)));
                }
            }
        }
        let mut handled: HashSet<String> = HashSet::new();
        // rare candies
        if n.level > o.level {
            let candies = lost.get("Rare Candy").copied().unwrap_or(0).min(n.level - o.level);
            if candies > 0 {
                take_n(lost, "Rare Candy", candies);
                self.queue_event(EventDefinition::with_rare_candy(candies));
            } else {
                log::warn!("[gen5] level {} -> {} in the overworld without a Rare Candy", o.level, n.level);
            }
            self.level_up_events(&o.species.clone(), o.level + 1, n.level, &o.moves, &n.moves, &mut handled, true);
        }
        self.evolution_events(o, n, &mut handled);
        if o.moves != n.moves {
            let heart_scale_used = take(lost, "Heart Scale");
            self.other_move_events(&o.moves, &n.moves, &handled, &new.items, heart_scale_used);
        }
    }

    // ---- battles ------------------------------------------------------------------

    fn start_battle(&mut self, store: &PropertyStore, pre: Snap) {
        log::info!("[gen5] battle started (meta.state {})", self.meta_state(store));
        self.pending_reward = None;
        self.pending_levelup.clear();
        let mut b = BattleTrack::new(pre);
        for s in 0..6 {
            b.opp_held[s] = store.str_of(&self.keys.opp_held[s]);
        }
        self.battle = Some(b);
        self.set_phase(Phase::Battle);
        self.update_battle(store);
    }

    /// Follow the battle structs; called after every batch while in battle.
    fn update_battle(&mut self, store: &PropertyStore) {
        let k = self.keys.clone();
        let decided = !self.in_battle(store) || Self::i64_of(store, &k.battle_end) == 1;
        let opp_count = (Self::i64_of(store, &k.opp_count).clamp(0, 6)) as usize;
        let player_count = (Self::i64_of(store, &k.team_count).clamp(0, 6)) as usize;
        let player_pos = store.i64_of(&k.player_pos).filter(|p| (0..player_count as i64).contains(p)).map(|p| p as usize);
        let opp_pos = store.i64_of(&k.opp_pos).filter(|p| (0..opp_count as i64).contains(p)).map(|p| p as usize);
        let trainer_a = Self::i64_of(store, &k.trainer_a);
        let trainer_b = Self::i64_of(store, &k.trainer_b);
        let mut to_queue: Vec<EventDefinition> = Vec::new();
        let conv = self.conv.clone();
        let gen = self.gen.clone();
        let Some(b) = self.battle.as_mut() else { return };
        if decided {
            if !b.over {
                log::info!("[gen5] the battle is decided");
            }
            b.over = true;
            return;
        }
        // the trainer id arrives a few frames into the battle
        if trainer_a != 0 && b.trainer_a == 0 {
            b.trainer_a = trainer_a;
            b.is_trainer = Some(true);
            log::info!("[gen5] trainer battle: id {}", trainer_a);
        }
        if trainer_b != 0 && b.trainer_b == 0 {
            b.trainer_b = trainer_b;
            log::info!("[gen5] second trainer: id {}", trainer_b);
        }
        if b.is_trainer == Some(true) && !b.trainer_event_queued {
            b.trainer_event_queued = true;
            let mut td = TrainerEventDefinition::new(&b.trainer_a.to_string());
            if b.trainer_b != 0 {
                td.second_trainer_name = Value::from(b.trainer_b);
            }
            to_queue.push(EventDefinition::with_trainer(td));
        }
        // who is out
        if let Some(p) = opp_pos {
            if !b.appear_order.contains(&p) {
                b.appear_order.push(p);
            }
        }
        if let (Some(e), Some(p)) = (opp_pos, player_pos) {
            if !b.opp_fainted[e] && !b.player_fainted[p] {
                b.participants[e].insert(p);
            }
        }
        // player mons fainting (a fainted mon gets no experience)
        for s in 0..player_count {
            let hp = Self::i64_of(store, &k.player_struct_hp[s]);
            if hp > 0 {
                b.player_seen_alive[s] = true;
                b.player_fainted[s] = false;
            } else if b.player_seen_alive[s] && b.player_hp[s] > 0 && !b.player_fainted[s] {
                b.player_fainted[s] = true;
                log::info!("[gen5] player slot {} fainted", s + 1);
                for e in 0..6 {
                    if !b.opp_fainted[e] {
                        b.participants[e].remove(&s);
                    }
                }
            }
            b.player_hp[s] = hp;
        }
        if player_count > 0 && (0..player_count).all(|s| b.player_fainted[s] || (b.player_seen_alive[s] && b.player_hp[s] <= 0)) && (0..player_count).any(|s| b.player_seen_alive[s]) {
            if !b.all_player_fainted {
                log::info!("[gen5] every party member fainted");
            }
            b.all_player_fainted = true;
        }
        // opponents fainting
        for s in 0..opp_count {
            let hp = Self::i64_of(store, &k.opp_struct_hp[s]);
            if hp > 0 {
                b.opp_seen_alive[s] = true;
            } else if b.opp_seen_alive[s] && b.opp_hp[s] > 0 && !b.opp_fainted[s] {
                b.opp_fainted[s] = true;
                let raw = store.get_value(Some(&k.opp_struct_species[s]));
                let species = opt_str(&raw).and_then(|r| conv.pkmn_name_convert(Some(&r), None)).unwrap_or_default();
                let level = Self::i64_of(store, &k.opp_struct_level[s]);
                log::info!("[gen5] opponent slot {} fainted: {} Lv{}", s + 1, species, level);
                if !b.appear_order.contains(&s) {
                    b.appear_order.push(s);
                }
                if b.participants[s].is_empty() {
                    if let Some(p) = player_pos {
                        b.participants[s].insert(p);
                    }
                }
                b.faints.push((s, species.clone(), level));
                if b.trainer_a == 0 {
                    // still no trainer id: it is a wild battle
                    b.is_trainer = Some(false);
                    if gen.pkmn_db().get_pkmn(&species).is_some() {
                        to_queue.push(EventDefinition::with_wild(WildPkmnEventDefinition::new(&species, level, 1, false)));
                    } else {
                        log::error!("[gen5] unknown wild Pokémon {:?}", species);
                    }
                }
            }
            b.opp_hp[s] = hp;
            // Thief / Covet: the item leaves the enemy's party data
            let held = store.str_of(&k.opp_held[s]);
            if b.opp_held[s].is_some() && held.is_none() && !b.opp_lost_item.contains(&s) {
                b.opp_lost_item.push(s);
            }
            b.opp_held[s] = held;
        }
        for ev in to_queue {
            self.queue_event(ev);
        }
    }

    /// The battle is over and the party holds its outcome.
    fn finish_battle(&mut self, post: Snap) {
        let Some(b) = self.battle.take() else { return };
        let pre = b.pre.clone();
        log::info!(
            "[gen5] battle over: trainer {} / {}, fainted {:?}, participants {:?}, lost {}",
            b.trainer_a,
            b.trainer_b,
            b.faints.iter().map(|(s, sp, l)| format!("{}:{} Lv{}", s + 1, sp, l)).collect::<Vec<_>>(),
            b.participants.iter().map(|p| p.len()).collect::<Vec<_>>(),
            b.all_player_fainted
        );
        let (mut gained, lost) = diff_items(&pre.items, &post.items);
        let o = self.solo_pid.and_then(|p| pre.by_pid(p)).cloned();
        let n = self.solo_pid.and_then(|p| post.by_pid(p)).cloned();
        let mut thief_mons: Vec<i64> = Vec::new();
        if let (Some(o), Some(n)) = (&o, &n) {
            // held item
            if o.held != n.held {
                // what it held is gone (eaten, flung, knocked off, swapped away)
                if let Some(x) = &o.held {
                    log::info!("held {} was used up", x);
                    self.queue_event(EventDefinition::with_hold_item(HoldItemEventDefinition::new(None, true)));
                }
                // and what it holds now never came from the bag (Thief / Covet)
                if let Some(y) = &n.held {
                    if b.is_trainer == Some(true) {
                        let slot = b.opp_lost_item.first().copied().or_else(|| b.appear_order.last().copied()).unwrap_or(0);
                        log::info!("stole {} from the enemy mon at party position {}", y, slot);
                        thief_mons.push(slot as i64);
                    } else {
                        self.queue_event(EventDefinition::with_item(InventoryEventDefinition::new(y, 1, true, false, None)));
                        self.queue_event(EventDefinition::with_hold_item(HoldItemEventDefinition::new(Some(y), false)));
                    }
                }
            }
            // level-ups and the moves they taught
            let mut handled: HashSet<String> = HashSet::new();
            if n.level > o.level {
                // (a forget-a-move choice in battle is made before the party update)
                self.level_up_events(&o.species.clone(), o.level + 1, n.level, &o.moves, &n.moves, &mut handled, false);
            }
            // the evolution scene can finish before the battle word is cleared
            self.evolution_events(o, n, &mut handled);
            if o.moves != n.moves {
                // Sketch, or a level-up move this species does not list
                self.other_move_events(&o.moves, &n.moves, &handled, &IndexMap::new(), false);
            }
        }
        // the bag in battle: items used (and balls thrown)
        for (name, count) in &lost {
            self.queue_event(EventDefinition::with_item(InventoryEventDefinition::new(name, *count, false, false, None)));
        }
        for (name, count) in gained.drain(..) {
            self.queue_event(EventDefinition::with_item(InventoryEventDefinition::new(&name, count, true, false, None)));
        }
        let trainer = b.is_trainer == Some(true);
        // the party update at the end of a lost battle has every HP at 0
        let blacked_out = b.all_player_fainted || (!post.team.is_empty() && post.team.iter().all(|m| m.hp == 0));
        if blacked_out {
            self.blackout_heal_pending = true;
            // lost: the trainer is still to be beaten, and the player blacks out
            if trainer {
                let mut ev = EventDefinition::with_trainer(TrainerEventDefinition::new(&b.trainer_a.to_string()));
                ev.notes = trainer_loss_flag();
                self.queue_event(ev);
                for (_, species, level) in &b.faints {
                    self.queue_event(EventDefinition::with_wild(WildPkmnEventDefinition::new(species, *level, 1, true)));
                }
            }
            self.queue_event(EventDefinition::with_blackout());
        } else if trainer {
            // the order they were beaten in (in a double battle both leads are out
            // from the start), then any still standing in the order they came out
            let opp_slots: Vec<usize> = {
                let mut v: Vec<usize> = b.faints.iter().map(|(s, _, _)| *s).collect();
                for s in &b.appear_order {
                    if !v.contains(s) {
                        v.push(*s);
                    }
                }
                v
            };
            let mut td = TrainerEventDefinition::new(&b.trainer_a.to_string());
            if b.trainer_b != 0 {
                td.second_trainer_name = Value::from(b.trainer_b);
            }
            let known = opp_slots.iter().copied().max().map(|m| m + 1).unwrap_or(0);
            // only the first field position is followed: in a double battle the
            // two leads share every enemy's experience (as gen 4 records it)
            let double = self.gen.trainer_db().get_trainer_by_id(b.trainer_a).map(|t| t.double_battle).unwrap_or(false);
            let least = if double && pre.team.iter().filter(|m| m.hp > 0).count() >= 2 { 2 } else { 1 };
            let split: Vec<i64> = (0..known).map(|s| b.participants[s].len().max(least) as i64).collect();
            td.exp_split = if split.iter().any(|x| *x > 1) { split } else { Vec::new() };
            // two trainers interleave their mons: a custom order would break
            td.mon_order = if b.trainer_b != 0 {
                Vec::new()
            } else {
                let mut sorted = opp_slots.clone();
                sorted.sort();
                sorted.iter().map(|x| opp_slots.iter().position(|y| y == x).unwrap_or(0) as i64 + 1).collect()
            };
            td.thief_mons = thief_mons;
            if let Some(o) = &o {
                if o.moves.iter().flatten().any(|m| m == RETURN_MOVE_NAME) {
                    td.custom_move_data = (0..known.max(1))
                        .map(|_| {
                            let mut c = CustomMoveData::default();
                            c.player.insert(RETURN_MOVE_NAME.to_string(), (((o.friendship as f64) / 2.5).trunc() as i64).to_string());
                            c
                        })
                        .collect();
                }
            }
            td.pay_day_amount = Some(post.money - pre.money);
            let mut ev = EventDefinition::with_trainer(td);
            ev.notes = roar_flag();
            self.queue_event(ev);
            self.pending_reward = self
                .gen
                .trainer_db()
                .get_trainer_by_id(b.trainer_a)
                .and_then(|t| self.gen.get_fight_reward(&t.name))
                .map(|r| r.to_string());
        }
        self.baseline = Some(post);
        self.dirty = false;
    }

    // ---- lifecycle ----------------------------------------------------------------

    /// Take the current state as the starting point, without events.
    fn initialize(&mut self, snap: Snap) {
        self.player_id = Some(snap.player_id);
        let solo = self.find_solo(&snap).cloned();
        match &solo {
            Some(m) => log::info!("[gen5] recording from {} Lv{}", m.species, m.level),
            None => log::warn!("[gen5] the route's solo mon is not in the party yet"),
        }
        self.baseline = Some(snap);
        self.dirty = false;
    }

    fn enter_area(&mut self, store: &PropertyStore) {
        let zone = store.i64_of(&self.keys.map_index);
        let area = match zone.and_then(|z| super::gen5_places::place_name(self.b2w2, z)) {
            Some(name) => name.to_string(),
            // a zone without a name of its own: stay in the current area
            None if self.area.is_some() => return,
            None => py_str(&store.get_value(Some(&self.keys.map_name))),
        };
        if self.area.as_deref() != Some(area.as_str()) {
            self.area = Some(area.clone());
            self.controller.entered_new_area(&area);
        }
    }

    fn tick(&mut self, store: &PropertyStore) {
        let now = Instant::now();
        let in_battle = self.in_battle(store);
        let player_id = Self::i64_of(store, &self.keys.player_id);
        // a soft reset empties the party (the recorder only follows a game with
        // one). The player id is no signal: Black 2 keeps it through the reset,
        // and both B2W2 games read it as 0 now and then while a save loads.
        let reset = Self::i64_of(store, &self.keys.team_count) == 0;
        match self.phase {
            Phase::Uninitialized => {
                if player_id == 0 || in_battle || now.duration_since(self.last_change) < SETTLE {
                    return;
                }
                let Ok(snap) = self.read_snap(store) else { return };
                if self.player_id.is_some() && self.player_id != Some(snap.player_id) {
                    self.controller.route_restarted();
                }
                self.initialize(snap);
                self.enter_area(store);
                self.set_phase(Phase::Overworld);
            }
            Phase::Resetting => {
                if player_id == 0 || in_battle || now.duration_since(self.last_change) < SETTLE {
                    return;
                }
                let Ok(snap) = self.read_snap(store) else { return };
                if self.same_save_file(&snap) {
                    // the save is loaded while the title screen menus are still up;
                    // Black 2 / White 2 read garbage as the player id until the
                    // game goes on, so wait for the real one
                    if self.player_id.is_some_and(|id| id != snap.player_id) {
                        return;
                    }
                    if std::mem::take(&mut self.reset_pending) {
                        self.settle_reset(&snap);
                    }
                } else {
                    log::info!("[gen5] a different save file was loaded");
                    self.controller.route_restarted();
                    self.solo_pid = None;
                    self.saved = None;
                    self.reset_pending = false;
                }
                self.pre_reset = None;
                self.initialize(snap);
                self.enter_area(store);
                self.set_phase(Phase::Overworld);
            }
            Phase::Overworld => {
                if reset {
                    self.begin_reset();
                    return;
                }
                if player_id != 0 && self.player_id.is_some() && self.player_id != Some(player_id) && now.duration_since(self.last_change) >= SETTLE {
                    if let Ok(snap) = self.read_snap(store) {
                        if self.same_save_file(&snap) {
                            log::info!("[gen5] the player id reads {} now (was {:?})", player_id, self.player_id);
                            self.player_id = Some(player_id);
                        } else {
                            log::info!("[gen5] player id changed: a new save file");
                            self.controller.route_restarted();
                            self.solo_pid = None;
                            self.saved = None;
                            self.player_id = None;
                            self.set_phase(Phase::Uninitialized);
                            return;
                        }
                    }
                }
                let battle_starting = in_battle;
                if self.dirty && (battle_starting || self.save_pending || now.duration_since(self.last_change) >= SETTLE) {
                    match self.read_snap(store) {
                        Ok(snap) => {
                            self.invalid_since = None;
                            if let Some(old) = self.baseline.clone() {
                                self.overworld_diff(&old, &snap);
                            }
                            self.baseline = Some(snap);
                            self.dirty = false;
                        }
                        Err(why) => {
                            let since = *self.invalid_since.get_or_insert(now);
                            if now.duration_since(since) >= INVALID_WARN {
                                log::warn!("[gen5] the party has been unreadable for {:?}: {}", now.duration_since(since), why);
                                self.invalid_since = Some(now);
                            }
                        }
                    }
                }
                if self.save_pending && !self.dirty {
                    self.save_pending = false;
                    let area = self.area.clone().unwrap_or_default();
                    log::info!("[gen5] the game was saved in {}", area);
                    self.queue_event(EventDefinition::with_save(&area));
                    self.saved = self.baseline.clone();
                }
                if battle_starting {
                    if let Some(pre) = self.baseline.clone() {
                        self.start_battle(store, pre);
                    }
                }
            }
            Phase::Battle => {
                if reset {
                    self.battle = None;
                    self.begin_reset();
                    return;
                }
                self.update_battle(store);
                let over = self.battle.as_ref().map(|b| b.over).unwrap_or(true);
                // the party holds the outcome once the battle word is cleared
                if over && !in_battle && now.duration_since(self.last_change) >= SETTLE {
                    match self.read_snap(store) {
                        Ok(post) => {
                            self.finish_battle(post);
                            self.enter_area(store);
                            self.set_phase(Phase::Overworld);
                        }
                        Err(why) => {
                            let since = *self.invalid_since.get_or_insert(now);
                            if now.duration_since(since) >= INVALID_WARN {
                                log::warn!("[gen5] the party after the battle is unreadable: {}", why);
                                self.invalid_since = Some(now);
                            }
                        }
                    }
                }
            }
        }
    }

    fn begin_reset(&mut self) {
        log::info!("[gen5] the game was reset");
        self.save_pending = false;
        self.pending_reward = None;
        self.pending_levelup.clear();
        self.blackout_heal_pending = false;
        // (a second reset before the save loaded keeps the first one's state)
        if !self.reset_pending {
            self.pre_reset = self.baseline.take();
        }
        self.reset_pending = true;
        self.baseline = None;
        self.dirty = false;
        self.set_phase(Phase::Resetting);
    }

    /// Whether `snap`, read after a reset, is from the save file being followed:
    /// its party shares a Pokémon with the last known one. (The player id cannot
    /// tell while Black 2 / White 2 read garbage for it.)
    fn same_save_file(&self, snap: &Snap) -> bool {
        let known: HashSet<i64> = [&self.pre_reset, &self.baseline, &self.saved]
            .into_iter()
            .flatten()
            .flat_map(|s| s.team.iter().map(|m| m.pid))
            .chain(self.solo_pid)
            .collect();
        if known.is_empty() {
            return self.player_id.is_none_or(|id| id == snap.player_id);
        }
        snap.team.iter().any(|m| known.contains(&m.pid))
    }

    /// The save the game loaded after a reset decides how much of the route stays.
    fn settle_reset(&mut self, loaded: &Snap) {
        let at_save = self.saved.as_ref().map(|s| same_state(s, loaded));
        let at_reset = self.pre_reset.as_ref().is_some_and(|s| same_state(s, loaded));
        if at_save == Some(true) {
            log::info!("[gen5] the last save was loaded");
            self.queue_event(EventDefinition::notes_only(&reset_flag()));
        } else if at_reset {
            // everything recorded before the reset is in the save: it was made
            // after the last save that was seen, with nothing changed since
            let area = self.area.clone().unwrap_or_default();
            log::info!("[gen5] the loaded save is the state before the reset: saved in {}", area);
            self.queue_event(EventDefinition::with_save(&area));
            self.saved = Some(loaded.clone());
        } else if self.saves_detectable {
            if at_save == Some(false) {
                let msg = "The game loaded a save that does not match the last save the recorder saw, so the route was rolled back to that one: check the events after it.";
                log::warn!("[gen5] {}", msg);
                self.controller.host().post(move |h| h.send_message(msg));
            } else {
                log::info!("[gen5] no save seen since recording started: back to the route's last save");
            }
            self.queue_event(EventDefinition::notes_only(&reset_flag()));
        } else {
            self.queue_event(EventDefinition::notes_only(&format!(
                "{}The game was reset. This mapper does not report saves, so no events were removed: delete the ones after your last save by hand.",
                consts::RECORDING_ERROR_FRAGMENT
            )));
        }
    }

    fn spawn_processing_thread(&self) {
        let ctx = ProcessCtx { controller: self.controller.clone(), gen: self.gen.clone(), queue: self.queue.clone(), active: self.active.clone() };
        let conv = self.conv.clone();
        std::thread::Builder::new()
            .name("gen5-recorder-events".into())
            .spawn(move || {
                ctx.run(|ctx, ev| process_one(ctx, ev, &conv));
            })
            .ok();
    }
}

/// Every party member back at full HP, when one was not before (same party).
fn healed(old: &Snap, new: &Snap) -> bool {
    old.team.len() == new.team.len()
        && old.team.iter().zip(new.team.iter()).all(|(a, b)| a.pid == b.pid && a.max_hp == b.max_hp && a.level == b.level)
        && new.team.iter().all(|m| m.max_hp > 0 && m.hp == m.max_hp)
        && old.team.iter().any(|m| m.hp < m.max_hp)
}

/// The same party, bag and money. Friendship is left out: walking raises it
/// without any other change, so a snapshot can be behind on it.
fn same_state(a: &Snap, b: &Snap) -> bool {
    let key = |m: &Mon| Mon { friendship: 0, ..m.clone() };
    a.money == b.money && a.items == b.items && a.team.len() == b.team.len() && a.team.iter().zip(&b.team).all(|(x, y)| key(x) == key(y))
}

/// No item left the bag (a potion is not a heal).
fn lost_nothing(old: &Snap, new: &Snap) -> bool {
    old.items.iter().all(|(name, count)| new.items.get(name).copied().unwrap_or(0) >= *count)
}

/// The move `learned` replaced: the move that was in its slot before.
fn replaced_move(before: &[Option<String>; 4], after: &[Option<String>; 4], learned: &str) -> Option<String> {
    for i in 0..4 {
        if after[i].as_deref() == Some(learned) && before[i].as_deref() != Some(learned) {
            return before[i].clone();
        }
    }
    None
}

fn diff_items(old: &IndexMap<String, i64>, new: &IndexMap<String, i64>) -> (IndexMap<String, i64>, IndexMap<String, i64>) {
    let mut gained = IndexMap::new();
    let mut lost = IndexMap::new();
    for (name, count) in old {
        let now = new.get(name).copied().unwrap_or(0);
        if now < *count {
            lost.insert(name.clone(), count - now);
        } else if now > *count {
            gained.insert(name.clone(), now - count);
        }
    }
    for (name, count) in new {
        if !old.contains_key(name) {
            gained.insert(name.clone(), *count);
        }
    }
    (gained, lost)
}

/// Take one `name` out of `map`; `false` when there is none.
fn take(map: &mut IndexMap<String, i64>, name: &str) -> bool {
    take_n(map, name, 1)
}

fn take_n(map: &mut IndexMap<String, i64>, name: &str, n: i64) -> bool {
    match map.get_mut(name) {
        Some(c) if *c >= n => {
            *c -= n;
            if *c == 0 {
                map.shift_remove(name);
            }
            true
        }
        _ => false,
    }
}

impl GameRecorder for Gen5Machine {
    fn on_mapper_loaded(&mut self, store: &PropertyStore) -> (Vec<String>, Vec<String>) {
        self.keys = Gen5Keys::configure(store);
        self.settle_paths = self.keys.settle_paths();
        let save_address = store.get(&self.keys.save_flag).and_then(|p| value_as_i64(&p.address));
        let detectable = save_address.is_some_and(|a| SAVE_COUNTER_ADDRESSES.contains(&a) || PARTY_BLOCK_COUNTERS.contains(&a));
        if save_address.is_some_and(|a| PARTY_BLOCK_COUNTERS.contains(&a)) {
            log::info!("[gen5] saves are read from the party's save block: a save with the party unchanged since the previous one goes unseen");
        }
        if !detectable && !self.warned_saves {
            self.warned_saves = true;
            let msg = format!(
                "This Poke-A-Byte mapper cannot tell the recorder when the game saves ({} is read from {}). After a reset the recorder can only tell what to keep when nothing changed between your last save and the reset; otherwise delete the events after your last save by hand.",
                self.keys.save_flag,
                save_address.map(|a| format!("0x{:X}", a)).unwrap_or_else(|| "nowhere".into())
            );
            log::warn!("[gen5] {}", msg);
            self.controller.host().post(move |h| h.send_message(&msg));
        }
        self.saves_detectable = detectable;
        let all = self.keys.all();
        let invalid: Vec<String> = all.iter().filter(|k| !store.contains(k)).cloned().collect();
        (all, invalid)
    }

    fn is_active(&self) -> bool {
        is_set(&self.active)
    }

    fn startup(&mut self, _store: &PropertyStore) {
        self.active.store(true, Ordering::SeqCst);
        self.phase = Phase::Uninitialized;
        self.last_change = Instant::now();
        self.controller.set_game_state(GameState::Uninitialized);
        crate::shuckie::supershuckie().start();
        self.spawn_processing_thread();
    }

    fn handle_event(&mut self, store: &PropertyStore, new: &GameHookProperty, prev: &GameHookProperty) {
        if self.debug_mode && new.path != self.keys.gametime_seconds {
            log::info!("Change of {} from {} to {} ({:?})", new.path, py_str(&prev.value), py_str(&new.value), self.phase);
        }
        if self.settle_paths.contains(&new.path) {
            self.last_change = Instant::now();
            self.dirty = true;
        }
        if self.saves_detectable && new.path == self.keys.save_flag && self.phase == Phase::Overworld && prev.value.is_boolean() && new.value.is_boolean() {
            // (loading a save after a reset flips it too: that is the Resetting phase)
            self.save_pending = true;
        }
        if new.path == self.keys.map_index && self.phase == Phase::Overworld {
            self.enter_area(store);
        }
    }

    fn on_idle(&mut self, store: &PropertyStore) {
        self.tick(store);
    }

    fn active_flag(&self) -> ActiveFlag {
        self.active.clone()
    }

    fn shutdown(&mut self) {
        log::info!("Shutting down the gen 5 recording FSM");
        crate::controller::deactivate(&self.active);
    }
}
