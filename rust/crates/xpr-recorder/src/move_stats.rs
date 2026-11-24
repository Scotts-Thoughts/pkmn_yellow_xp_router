//! Move stats while recording (spec "player profiles and move stats" §2):
//! a per-generation detector that turns the mapper's `battle.move_tracking.*`
//! properties into one [`MoveOutcome`] per executed move, and the collector
//! that adds them up per battle, files each finished battle (trainer event,
//! wild total or reset attempt) and keeps the run object for the route.
//!
//! How a move is found, per generation (see `docs/move_stats.md`):
//! - gen 1: `hWhoseTurn` changes, the player's PP drops, or the miss flags are
//!   cleared (`ExecutePlayerMove` / `ExecuteEnemyMove` start).
//! - gen 2: `wPlayerTurnsTaken` / `wEnemyTurnsTaken` go up (`BattleCommand_DoTurn`,
//!   which only runs once a move really executes).
//! - gen 3: `gCurrentTurnActionNumber` changes while the action is a move.
//! - gen 4: the turn order counter (or turn number) changes while the battler
//!   whose turn it is picked FIGHT.
//! Everything seen until the next move starts belongs to that move. The flags
//! are collected from the change events (they are cleared when a move starts,
//! so a stale value never arrives as a change), and the secondary effect is
//! worked out by comparing the user and target before and after.

use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use xpr_data::gen_consts as gc;
use xpr_data::model::{EnemyPkmn, Move, StageModifiers};
use xpr_data::GenData;
use xpr_engine::move_stats::{effect_info, interest_of, new_id, EffectInfo, MoveCounts};
use xpr_engine::{BattleMoveStats, RunMoveStats, SidePair};

use crate::gamehook::{GameHookProperty, PropertyStore};

pub const PREFIX: &str = "battle.move_tracking.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Player,
    Enemy,
}

/// One executed move (spec §2.1). Crit counts are per hit.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MoveOutcome {
    pub side: Option<Side>,
    pub battler: u8,
    pub move_name: String,
    pub called_by: Option<String>,
    pub accuracy_rolled: bool,
    pub hit_chance: Option<f64>,
    pub missed: bool,
    pub crit_rolls: u32,
    pub crit_chance: Option<f64>,
    pub crits: u32,
    pub effect_rolled: bool,
    pub effect_chance: Option<f64>,
    pub effect_applied: bool,
    pub effect_blocked: bool,
    pub failed: bool,
    pub no_effect: bool,
    pub protected: bool,
    pub super_effective: bool,
    pub not_very_effective: bool,
    pub ohko: bool,
    pub ko: bool,
    /// hits landed, for multi-hit moves (0 when the move is not one)
    pub hits: u32,
}

impl MoveOutcome {
    /// The counts this outcome adds to its move's entry.
    pub fn counts(&self, gen: Option<&GenData>) -> MoveCounts {
        let mut c = MoveCounts { uses: 1, ..Default::default() };
        let mv = gen.and_then(|g| g.move_db().get_move(&self.move_name).cloned());
        if let Some(mv) = &mv {
            c.listed_accuracy = mv.accuracy;
            c.move_type = Some(mv.move_type.clone());
            c.interest = interest_of(mv);
            if let Some(info) = effect_info(mv) {
                c.effect_kind = Some(info.kind.clone());
                c.listed_effect_chance = Some(info.chance);
            }
        }
        if self.accuracy_rolled {
            if let Some(p) = self.hit_chance {
                c.acc_rolls = 1;
                if self.missed {
                    c.misses = 1;
                } else {
                    c.hits = 1;
                }
                c.expected_hits = p;
                c.hit_variance = p * (1.0 - p);
            }
        }
        if self.crit_rolls > 0 {
            if let Some(p) = self.crit_chance {
                let n = self.crit_rolls as f64;
                c.crit_rolls = self.crit_rolls as i64;
                c.crits = self.crits.min(self.crit_rolls) as i64;
                c.expected_crits = n * p;
                c.crit_variance = n * p * (1.0 - p);
            }
        }
        if self.effect_blocked {
            c.effect_blocked = 1;
        } else if self.effect_rolled {
            if let Some(p) = self.effect_chance {
                c.effect_rolls = 1;
                c.effects = self.effect_applied as i64;
                c.expected_effects = p;
                c.effect_variance = p * (1.0 - p);
            }
        }
        c.failed = self.failed as i64;
        c.no_effect = self.no_effect as i64;
        c.protected = self.protected as i64;
        c.super_effective = self.super_effective as i64;
        c.not_very_effective = self.not_very_effective as i64;
        c.ohko = self.ohko as i64;
        c.kos = self.ko as i64;
        if self.hits > 1 || (self.hits > 0 && mv.as_ref().map(|m| m.has_flavor("multi_hit")).unwrap_or(false)) {
            c.multi_hit.insert(self.hits.to_string(), 1);
        }
        if let Some(by) = &self.called_by {
            c.called_by.insert(by.clone(), 1);
        }
        c
    }
}

// ---------------------------------------------------------------------------
// Collector
// ---------------------------------------------------------------------------

/// The numbers for this recording session. Lives in the controller behind a
/// mutex, so the app's local server can read it while the recorder writes.
#[derive(Default)]
pub struct MoveStatsCollector {
    gen: Option<Arc<GenData>>,
    recording: bool,
    run: Option<RunMoveStats>,
    /// the run total (battles still in the route, plus wild)
    total: SidePair,
    current: Option<BattleMoveStats>,
    current_lost: bool,
    /// a trainer battle event was queued before the battle state was entered
    trainer_hint: bool,
    /// every battle finished this session, in the order fought
    finished: Vec<BattleMoveStats>,
    /// finished trainer battles not yet stored on their event
    to_attach: Vec<BattleMoveStats>,
    /// battle ids whose numbers are in `total`
    counted: HashSet<String>,
    /// wild battles since the last save (a reset takes them back out)
    wild_since_save: Vec<BattleMoveStats>,
    /// the run object changed and the route needs the new copy
    dirty: bool,
}

pub type MoveStatsHandle = Arc<Mutex<MoveStatsCollector>>;

fn now_iso() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // civil-from-days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, rem / 3600, (rem % 3600) / 60, rem % 60)
}

impl MoveStatsCollector {
    pub fn new_handle() -> MoveStatsHandle {
        Arc::new(Mutex::new(MoveStatsCollector::default()))
    }

    /// Recording started. `run` is the route's run object (kept, so a route
    /// recorded over several sessions keeps one `run_id`); `route_total` the
    /// route's total at this moment.
    pub fn start(&mut self, gen: Arc<GenData>, run: Option<RunMoveStats>, route_total: SidePair, game: &str, solo_species: &str) {
        let mut run = run.unwrap_or_else(|| RunMoveStats::new(game, solo_species));
        if run.game.is_empty() {
            run.game = game.to_string();
        }
        if run.solo_species.is_empty() {
            run.solo_species = solo_species.to_string();
        }
        run.recorded_with = env!("CARGO_PKG_VERSION").to_string();
        *self = MoveStatsCollector { gen: Some(gen), recording: true, run: Some(run), total: route_total, dirty: true, ..Default::default() };
    }

    pub fn stop(&mut self) {
        self.recording = false;
        self.current = None;
        self.trainer_hint = false;
    }

    pub fn is_recording(&self) -> bool {
        self.recording
    }

    pub fn in_battle(&self) -> bool {
        self.current.is_some()
    }

    pub fn run(&self) -> Option<&RunMoveStats> {
        self.run.as_ref()
    }

    pub fn total(&self) -> &SidePair {
        &self.total
    }

    pub fn finished(&self) -> &[BattleMoveStats] {
        &self.finished
    }

    pub fn set_solo_species(&mut self, species: &str) {
        if let Some(run) = self.run.as_mut() {
            if run.solo_species != species && !species.is_empty() {
                run.solo_species = species.to_string();
                self.dirty = true;
            }
        }
    }

    /// The run object, when it changed since the last call.
    pub fn take_dirty_run(&mut self) -> Option<RunMoveStats> {
        if !self.dirty {
            return None;
        }
        self.dirty = false;
        self.run.clone()
    }

    pub fn begin_battle(&mut self) {
        if !self.recording {
            return;
        }
        let kind = if std::mem::take(&mut self.trainer_hint) { "trainer" } else { "wild" };
        self.current = Some(BattleMoveStats {
            battle_id: new_id(),
            started_at: now_iso(),
            kind: kind.to_string(),
            ..Default::default()
        });
        self.current_lost = false;
    }

    /// A trainer battle's opening event was queued (it can come just before
    /// or just after the battle state is entered).
    pub fn mark_trainer(&mut self) {
        match self.current.as_mut() {
            Some(b) => b.kind = "trainer".to_string(),
            None => self.trainer_hint = true,
        }
    }

    /// The trainer's name as the route has it (the opening event reached the route).
    pub fn set_trainer_name(&mut self, name: &str) {
        if let Some(b) = self.current.as_mut() {
            if b.kind == "trainer" && b.trainer.is_none() {
                b.trainer = Some(name.to_string());
                return;
            }
        }
        if let Some(b) = self.to_attach.iter_mut().rev().find(|b| b.trainer.is_none()) {
            b.trainer = Some(name.to_string());
            let id = b.battle_id.clone();
            if let Some(f) = self.finished.iter_mut().find(|f| f.battle_id == id) {
                f.trainer = Some(name.to_string());
            }
        }
    }

    /// The player lost the battle in progress (or the one that just ended).
    pub fn mark_lost(&mut self) {
        if self.current.is_some() {
            self.current_lost = true;
        } else if let Some(b) = self.to_attach.last_mut() {
            b.outcome = "lost".to_string();
            let id = b.battle_id.clone();
            if let Some(f) = self.finished.iter_mut().find(|f| f.battle_id == id) {
                f.outcome = "lost".to_string();
            }
        }
    }

    pub fn record(&mut self, outcome: &MoveOutcome) {
        let Some(b) = self.current.as_mut() else { return };
        let side = match outcome.side {
            Some(Side::Player) => &mut b.stats.player,
            Some(Side::Enemy) => &mut b.stats.enemy,
            None => return,
        };
        let counts = outcome.counts(self.gen.as_deref());
        side.entry(&outcome.move_name).add(&counts);
        if let Some(by) = &outcome.called_by {
            // the calling move is a use of its own as well
            let mut caller = MoveCounts { uses: 1, ..Default::default() };
            if let Some(mv) = self.gen.as_ref().and_then(|g| g.move_db().get_move(by).cloned()) {
                caller.interest = interest_of(&mv);
                caller.listed_accuracy = mv.accuracy;
                caller.move_type = Some(mv.move_type.clone());
            }
            side.entry(by).add(&caller);
        }
        let prefix = if outcome.side == Some(Side::Player) { "player" } else { "enemy" };
        if outcome.crits > 0 {
            *b.stats.other.entry(format!("{}_crits", prefix)).or_insert(0) += outcome.crits as i64;
        }
        if outcome.accuracy_rolled && outcome.missed {
            *b.stats.other.entry(format!("{}_misses", prefix)).or_insert(0) += 1;
        }
    }

    pub fn record_other(&mut self, key: &str, n: i64) {
        if let Some(b) = self.current.as_mut() {
            *b.stats.other.entry(key.to_string()).or_insert(0) += n;
        }
    }

    /// The battle state was left. `reset`: the game was reset mid-battle.
    pub fn end_battle(&mut self, reset: bool) {
        let Some(mut b) = self.current.take() else { return };
        b.finished_at = now_iso();
        b.outcome = if reset {
            "reset"
        } else if self.current_lost {
            "lost"
        } else {
            "won"
        }
        .to_string();
        self.current_lost = false;
        if b.stats.is_empty() {
            return;
        }
        self.finished.push(b.clone());
        if reset || b.outcome == "lost" && b.kind == "trainer" {
            // a lost trainer battle's event is taken out of the route
            self.push_attempt(b);
        } else if b.kind == "trainer" {
            self.total.add_signed(&b.stats, 1);
            self.counted.insert(b.battle_id.clone());
            self.to_attach.push(b);
        } else {
            self.total.add_signed(&b.stats, 1);
            self.counted.insert(b.battle_id.clone());
            if let Some(run) = self.run.as_mut() {
                run.wild.add_signed(&b.stats, 1);
            }
            self.wild_since_save.push(b);
            self.dirty = true;
        }
    }

    fn push_attempt(&mut self, b: BattleMoveStats) {
        if let Some(f) = self.finished.iter_mut().find(|f| f.battle_id == b.battle_id) {
            f.outcome = b.outcome.clone();
        }
        if let Some(run) = self.run.as_mut() {
            if !run.reset_attempts.iter().any(|r| r.battle_id == b.battle_id) {
                run.reset_attempts.push(b);
            }
        }
        self.dirty = true;
    }

    /// A save reached the route: the wild battles before it can no longer be reset away.
    pub fn on_save(&mut self) {
        self.wild_since_save.clear();
    }

    /// Trainer battles ready to be stored on their event.
    pub fn take_attachable(&mut self) -> Vec<BattleMoveStats> {
        let (ready, wait): (Vec<_>, Vec<_>) = std::mem::take(&mut self.to_attach).into_iter().partition(|b| b.trainer.is_some());
        self.to_attach = wait;
        let mut out = Vec::new();
        for b in ready {
            if b.outcome == "lost" {
                if self.counted.remove(&b.battle_id) {
                    self.total.add_signed(&b.stats, -1);
                }
                self.push_attempt(b);
            } else {
                out.push(b);
            }
        }
        out
    }

    /// The route took out the event of a trainer battle the player lost: a
    /// battle still waiting for that event becomes an attempt.
    pub fn lost_battle(&mut self, trainer: &str) {
        if let Some(i) = self.to_attach.iter().rposition(|b| b.trainer.as_deref() == Some(trainer)) {
            let b = self.to_attach.remove(i);
            self.event_removed(b, "lost");
        }
    }

    /// Put back battles whose event wasn't found yet.
    pub fn return_unattached(&mut self, mut battles: Vec<BattleMoveStats>) {
        battles.append(&mut self.to_attach);
        self.to_attach = battles;
    }

    /// A battle's event is being taken out of the route (a lost trainer battle,
    /// or everything since the last save on a reset): keep its numbers as an attempt.
    pub fn event_removed(&mut self, mut b: BattleMoveStats, outcome: &str) {
        if self.counted.remove(&b.battle_id) || !self.finished.iter().any(|f| f.battle_id == b.battle_id) {
            // in the total, from this session or an earlier one
            self.total.add_signed(&b.stats, -1);
        }
        b.outcome = outcome.to_string();
        self.push_attempt(b);
    }

    /// The game was reset to the last save. `removed` are the move stats of the
    /// trainer events the reset takes out of the route.
    pub fn on_reset(&mut self, removed: Vec<BattleMoveStats>) {
        for b in removed {
            self.event_removed(b, "reset");
        }
        for b in std::mem::take(&mut self.to_attach) {
            self.event_removed(b, "reset");
        }
        for b in std::mem::take(&mut self.wild_since_save) {
            if let Some(run) = self.run.as_mut() {
                run.wild.add_signed(&b.stats, -1);
            }
            self.event_removed(b, "reset");
        }
        if self.current.is_some() {
            self.end_battle(true);
        }
    }

    /// `GET /move-stats`: the run object plus every battle finished this session.
    pub fn server_json(&self) -> Value {
        let mut v = match &self.run {
            Some(run) => run.to_json(&self.total),
            None => json!({}),
        };
        if let Value::Object(m) = &mut v {
            m.insert("battles".into(), Value::Array(self.finished.iter().map(|b| b.to_json()).collect()));
            m.insert("in_battle".into(), Value::Bool(self.current.is_some()));
        }
        v
    }

    /// `GET /status`
    pub fn status_json(&self) -> Value {
        let run = self.run.as_ref();
        json!({
            "recording": self.recording,
            "game": run.map(|r| r.game.clone()),
            "run_id": run.map(|r| r.run_id.clone()),
            "solo_species": run.map(|r| r.solo_species.clone()),
        })
    }
}

// ---------------------------------------------------------------------------
// Detector
// ---------------------------------------------------------------------------

/// Which bits of a battler's status words mean what.
#[derive(Clone, Copy, Default)]
struct Bits {
    flinch: (usize, i64),
    confuse: (usize, i64),
    focus: (usize, i64),
    charging: (usize, i64),
    /// badly poisoned, when it is a volatile bit (gens 1–2)
    toxic_vol: Option<(usize, i64)>,
    /// badly poisoned, when it is a status bit (gens 3–4)
    toxic_status: i64,
}

const SLP: i64 = 0b111;
const PSN: i64 = 1 << 3;
const BRN: i64 = 1 << 4;
const FRZ: i64 = 1 << 5;
const PAR: i64 = 1 << 6;

#[derive(Clone, Default)]
struct BattlerPaths {
    species: String,
    level: String,
    hp: String,
    status: String,
    vol: Vec<String>,
    /// raw stage paths, in the game's order
    stages: Vec<String>,
    pp: Vec<String>,
    ability: String,
    item: String,
}

/// A battler at one moment.
#[derive(Clone, Debug, Default)]
struct Snap {
    species: Option<String>,
    level: i64,
    hp: i64,
    status: i64,
    vol: Vec<i64>,
    /// atk, def, spe, spa, spd, acc, eva as -6..=6
    stages: [i64; 7],
    ability: Option<String>,
    item: Option<String>,
}

const ST_ATK: usize = 0;
const ST_DEF: usize = 1;
const ST_SPE: usize = 2;
const ST_SPA: usize = 3;
const ST_SPD: usize = 4;
const ST_ACC: usize = 5;
const ST_EVA: usize = 6;

fn stage_index(stat: &str) -> Option<usize> {
    Some(match stat {
        "atk" => ST_ATK,
        "def" => ST_DEF,
        "spe" => ST_SPE,
        "spa" | "special" => ST_SPA,
        "spd" => ST_SPD,
        "acc" => ST_ACC,
        "ev" | "eva" => ST_EVA,
        _ => return None,
    })
}

/// The move in progress.
#[derive(Clone, Debug, Default)]
struct Window {
    battler: usize,
    target: Option<usize>,
    snaps: Vec<Snap>,
    /// the move as first seen, and as last seen (they differ for called moves)
    first_move: Option<String>,
    move_name: Option<String>,
    evidence: bool,
    /// gens 3–4: the battler's own move really ran (result flags, its PP, or
    /// its move showing while it is the attacker). HP and stat changes alone
    /// can be end-of-turn effects there.
    core: bool,
    pp_drop: bool,
    missed: bool,
    crit_edges: u32,
    ohko: bool,
    /// gens 1–2 effectiveness byte (10 = normal) when it wasn't normal
    effectiveness: Option<i64>,
    /// gens 3–4 result flags, or-ed together
    flags: i64,
    /// gen 4: the game said an indirect secondary effect fired
    effect_flag: bool,
    /// gen 2: `wEffectFailed` was set
    effect_failed: bool,
    hp_drops: [u32; 4],
    flinch_seen: [bool; 4],
    charging_rose: bool,
    num_hits: i64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Gen1,
    Gen2,
    Gen3,
    Gen4,
}

pub struct MoveDetector {
    gen: Arc<GenData>,
    kind: Kind,
    bits: Bits,
    battlers: Vec<BattlerPaths>,
    window: Option<Window>,
    /// gens 3–4: the action key the window was opened for
    last_key: Option<(i64, i64)>,
    /// every path this detector reads (fixed to the mapper's case)
    paths: Vec<String>,
}

fn p(name: &str) -> String {
    format!("{}{}", PREFIX, name)
}

fn p_(name: &str) -> String {
    p(name)
}

const G3_SYS: &str = "battle.flags.system.";

impl MoveDetector {
    /// The detector for a generation, or `None` where move stats aren't
    /// supported yet (gen 5).
    pub fn for_gen(gen: Arc<GenData>) -> Option<MoveDetector> {
        let kind = match gen.get_generation() {
            1 => Kind::Gen1,
            2 => Kind::Gen2,
            3 => Kind::Gen3,
            4 => Kind::Gen4,
            _ => return None,
        };
        let mut battlers = Vec::new();
        let bits;
        match kind {
            Kind::Gen1 => {
                for side in ["player", "enemy"] {
                    battlers.push(BattlerPaths {
                        species: p(&format!("{}_species", side)),
                        level: p(&format!("{}_level", side)),
                        hp: p(&format!("{}_hp", side)),
                        status: p(&format!("{}_status", side)),
                        vol: (1..=3).map(|i| p(&format!("{}_battle_status_{}", side, i))).collect(),
                        stages: (0..6).map(|i| p(&format!("{}_stat_mod_{}", side, i))).collect(),
                        pp: if side == "player" { (0..4).map(|i| p(&format!("player_pp_{}", i))).collect() } else { Vec::new() },
                        ..Default::default()
                    });
                }
                bits = Bits { flinch: (0, 1 << 3), confuse: (0, 1 << 7), focus: (1, 1 << 2), charging: (0, 1 << 4 | 1 << 6), toxic_vol: Some((2, 1)), toxic_status: 0 };
            }
            Kind::Gen2 => {
                for side in ["player", "enemy"] {
                    battlers.push(BattlerPaths {
                        species: p(&format!("{}_species", side)),
                        level: p(&format!("{}_level", side)),
                        hp: p(&format!("{}_hp", side)),
                        status: p(&format!("{}_status", side)),
                        vol: (1..=5).map(|i| p(&format!("{}_substatus_{}", side, i))).collect(),
                        stages: (0..7).map(|i| p(&format!("{}_stat_level_{}", side, i))).collect(),
                        pp: (0..4).map(|i| p(&format!("{}_pp_{}", side, i))).collect(),
                        ..Default::default()
                    });
                }
                bits = Bits { flinch: (2, 1 << 3), confuse: (2, 1 << 7), focus: (3, 1 << 2), charging: (2, 1 << 4 | 1 << 5 | 1 << 6), toxic_vol: Some((4, 1)), toxic_status: 0 };
            }
            Kind::Gen3 | Kind::Gen4 => {
                for i in 0..4 {
                    let b = |s: &str| p(&format!("battler_{}_{}", i, s));
                    battlers.push(BattlerPaths {
                        species: b("species"),
                        level: b("level"),
                        hp: b("hp"),
                        status: b("status"),
                        vol: vec![b("volatile")],
                        stages: ["atk", "def", "spe", "spa", "spd", "acc", "eva"].iter().map(|s| b(&format!("stage_{}", s))).collect(),
                        pp: (0..4).map(|j| b(&format!("pp_{}", j))).collect(),
                        ability: b("ability"),
                        item: b("item"),
                    });
                }
                bits = Bits { flinch: (0, 1 << 3), confuse: (0, 0b111), focus: (0, 1 << 20), charging: (0, 1 << 12), toxic_vol: None, toxic_status: 1 << 7 };
            }
        }
        let mut d = MoveDetector { gen, kind, bits, battlers, window: None, last_key: None, paths: Vec::new() };
        d.paths = d.all_paths();
        Some(d)
    }

    fn trigger_paths(&self) -> Vec<String> {
        let names: &[&str] = match self.kind {
            Kind::Gen1 => &[
                "move_missed",
                "critical_hit_or_ohko",
                "damage_multipliers",
                "move_didnt_miss",
                "whose_turn",
                "player_move_num",
                "enemy_move_num",
                "player_num_hits",
                "enemy_num_hits",
            ],
            Kind::Gen2 => &[
                "attack_missed",
                "critical_hit",
                "effect_failed",
                "type_modifier",
                "cur_player_move",
                "cur_enemy_move",
                "player_move_chance",
                "enemy_move_chance",
                "player_turns_taken",
                "enemy_turns_taken",
            ],
            Kind::Gen3 => &[
                "turn_action_number",
                "actions_by_turn_order_0",
                "actions_by_turn_order_1",
                "actions_by_turn_order_2",
                "actions_by_turn_order_3",
                "battler_by_turn_order_0",
                "battler_by_turn_order_1",
                "battler_by_turn_order_2",
                "battler_by_turn_order_3",
                "move_result_flags",
            ],
            Kind::Gen4 => &[
                "attacker",
                "defender",
                "turn_order_counter",
                "total_turns",
                "battle_status_mask",
                "move_status_flags",
                "multi_hit_num_hits",
                "action_command_0",
                "action_command_1",
                "action_command_2",
                "action_command_3",
                "action_order_0",
                "action_order_1",
                "action_order_2",
                "action_order_3",
                "move_current",
            ],
        };
        let mut out: Vec<String> = names.iter().map(|n| p(n)).collect();
        if self.kind == Kind::Gen3 {
            for n in ["current_move", "chosen_move", "battler_attacker", "battler_target", "crit_multiplier", "battlers_count"] {
                out.push(format!("{}{}", G3_SYS, n));
            }
        }
        out
    }

    fn all_paths(&self) -> Vec<String> {
        let mut out = self.trigger_paths();
        for b in &self.battlers {
            out.extend([b.species.clone(), b.level.clone(), b.hp.clone(), b.status.clone(), b.ability.clone(), b.item.clone()]);
            out.extend(b.vol.iter().cloned());
            out.extend(b.stages.iter().cloned());
            out.extend(b.pp.iter().cloned());
        }
        out.retain(|s| !s.is_empty());
        out
    }

    /// The paths to watch, case-corrected against the mapper; the ones the
    /// mapper lacks are left out (an old mapper simply records no move stats).
    pub fn on_mapper_loaded(&mut self, store: &PropertyStore) -> Vec<String> {
        let fix = |s: &mut String| {
            if s.is_empty() || store.contains(s) {
                return;
            }
            match store.resolve_case(s) {
                Some(real) => *s = real,
                None => s.clear(),
            }
        };
        for b in &mut self.battlers {
            for s in [&mut b.species, &mut b.level, &mut b.hp, &mut b.status, &mut b.ability, &mut b.item] {
                fix(s);
            }
            b.vol.iter_mut().for_each(fix);
            b.stages.iter_mut().for_each(fix);
            b.pp.iter_mut().for_each(fix);
        }
        let mut paths = self.all_paths();
        paths.retain(|s| store.contains(s));
        if !store.contains(&p(match self.kind {
            Kind::Gen1 => "whose_turn",
            Kind::Gen2 => "player_turns_taken",
            Kind::Gen3 => "turn_action_number",
            Kind::Gen4 => "turn_order_counter",
        })) {
            log::info!("Mapper has no move tracking properties: no move stats this session");
        }
        self.paths = paths.clone();
        paths
    }

    fn is_tracked(&self, path: &str) -> bool {
        self.paths.iter().any(|p| p == path)
    }

    // ---- reading ----------------------------------------------------------

    fn snap(&self, store: &PropertyStore, i: usize) -> Snap {
        let Some(b) = self.battlers.get(i) else { return Snap::default() };
        let int = |s: &str| if s.is_empty() { 0 } else { store.i64_of(s).unwrap_or(0) };
        let text = |s: &str| if s.is_empty() { None } else { store.str_of(s).filter(|x| !x.is_empty()) };
        let raw: Vec<i64> = b.stages.iter().map(|s| int(s)).collect();
        let mut stages = [0; 7];
        match self.kind {
            Kind::Gen1 if raw.len() == 6 => {
                let m = |v: i64| if v == 0 { 0 } else { v - 7 };
                stages = [m(raw[0]), m(raw[1]), m(raw[2]), m(raw[3]), m(raw[3]), m(raw[4]), m(raw[5])];
            }
            Kind::Gen2 if raw.len() == 7 => {
                for (k, v) in raw.iter().enumerate() {
                    stages[k] = if *v == 0 { 0 } else { v - 7 };
                }
            }
            Kind::Gen3 | Kind::Gen4 if raw.len() == 7 => {
                for (k, v) in raw.iter().enumerate() {
                    stages[k] = if b.stages[k].is_empty() { 0 } else { v - 6 };
                }
            }
            _ => {}
        }
        Snap {
            species: text(&b.species),
            level: int(&b.level),
            hp: int(&b.hp),
            status: int(&b.status),
            vol: b.vol.iter().map(|s| int(s)).collect(),
            stages,
            ability: text(&b.ability),
            item: text(&b.item),
        }
    }

    fn snaps(&self, store: &PropertyStore) -> Vec<Snap> {
        (0..self.battlers.len()).map(|i| self.snap(store, i)).collect()
    }

    fn vol_bit(&self, s: &Snap, (w, mask): (usize, i64)) -> bool {
        s.vol.get(w).map(|v| v & mask != 0).unwrap_or(false)
    }

    fn opponent(&self, battler: usize) -> usize {
        match self.kind {
            Kind::Gen1 | Kind::Gen2 => 1 - battler.min(1),
            _ => battler ^ 1,
        }
    }

    fn side_of(&self, battler: usize) -> Option<Side> {
        match self.kind {
            Kind::Gen1 | Kind::Gen2 => Some(if battler == 0 { Side::Player } else { Side::Enemy }),
            // battler 2 is a multi battle's ally: not the player's
            _ => match battler {
                0 => Some(Side::Player),
                1 | 3 => Some(Side::Enemy),
                _ => None,
            },
        }
    }

    // ---- windows ------------------------------------------------------------

    fn open(&mut self, store: &PropertyStore, battler: usize, out: &mut Vec<MoveOutcome>) {
        if let Some(w) = &self.window {
            if w.battler == battler && !w.evidence {
                return;
            }
        }
        if let Some(o) = self.close(store) {
            out.push(o);
        }
        let snaps = self.snaps(store);
        self.window = Some(Window { battler, snaps, ..Default::default() });
    }

    /// The battle started: nothing is in progress.
    pub fn begin(&mut self) {
        self.window = None;
        self.last_key = None;
    }

    /// The battle ended (or the state was left): finish the move in progress.
    pub fn end(&mut self, store: &PropertyStore) -> Vec<MoveOutcome> {
        let out = self.close(store).into_iter().collect();
        self.window = None;
        self.last_key = None;
        out
    }

    /// One property changed while in battle. Returns the moves that finished.
    pub fn on_change(&mut self, store: &PropertyStore, new: &GameHookProperty, prev: &GameHookProperty) -> Vec<MoveOutcome> {
        let mut out = Vec::new();
        if !self.is_tracked(&new.path) {
            return out;
        }
        let path = new.path.as_str();
        let name = path.strip_prefix(PREFIX).unwrap_or(path);
        let nv = new.as_i64();
        let pv = prev.as_i64();
        let rose = |mask: i64| nv.map(|v| v & mask != 0).unwrap_or(false) && !pv.map(|v| v & mask != 0).unwrap_or(false);
        match self.kind {
            Kind::Gen1 => match name {
                "whose_turn" => {
                    if let Some(v) = nv {
                        self.open(store, v.clamp(0, 1) as usize, &mut out);
                    }
                }
                "move_missed" | "move_didnt_miss" => {
                    if pv.unwrap_or(0) != 0 && nv == Some(0) {
                        // `ExecutePlayerMove` / `ExecuteEnemyMove` clear both flags as a move starts
                        let side = store.i64_of(&p("whose_turn")).unwrap_or(0).clamp(0, 1) as usize;
                        self.open(store, side, &mut out);
                    } else if nv.unwrap_or(0) != 0 {
                        if let Some(w) = self.window.as_mut() {
                            w.evidence = true;
                            if name == "move_missed" {
                                w.missed = true;
                            }
                        }
                    }
                }
                "critical_hit_or_ohko" => {
                    if let Some(w) = self.window.as_mut() {
                        match nv {
                            Some(1) => w.crit_edges += 1,
                            Some(2) => w.ohko = true,
                            _ => {}
                        }
                    }
                }
                "damage_multipliers" => {
                    if let (Some(w), Some(v)) = (self.window.as_mut(), nv) {
                        let e = v & 0x7F;
                        if e != 10 {
                            w.effectiveness = Some(e);
                        }
                    }
                }
                "player_num_hits" | "enemy_num_hits" => {
                    if let (Some(w), Some(v)) = (self.window.as_mut(), nv) {
                        w.num_hits = w.num_hits.max(v);
                    }
                }
                "player_move_num" | "enemy_move_num" => {
                    let b = if name.starts_with("player") { 0 } else { 1 };
                    self.note_move(b, new);
                }
                _ => self.on_battler_change(store, path, nv, pv, &mut out),
            },
            Kind::Gen2 => match name {
                "player_turns_taken" | "enemy_turns_taken" => {
                    if nv.unwrap_or(0) > pv.unwrap_or(0) {
                        let b = if name.starts_with("player") { 0 } else { 1 };
                        // flags were cleared by `CheckTurn` just before; this is a real use
                        self.force_open(store, b, &mut out);
                        if let Some(w) = self.window.as_mut() {
                            w.evidence = true;
                        }
                    }
                }
                "attack_missed" => {
                    if rose(0xFF) {
                        if let Some(w) = self.window.as_mut() {
                            w.missed = true;
                        }
                    }
                }
                "critical_hit" => {
                    if let Some(w) = self.window.as_mut() {
                        match nv {
                            Some(1) => w.crit_edges += 1,
                            Some(2) => w.ohko = true,
                            _ => {}
                        }
                    }
                }
                "effect_failed" => {
                    if rose(0xFF) {
                        if let Some(w) = self.window.as_mut() {
                            w.effect_failed = true;
                        }
                    }
                }
                "type_modifier" => {
                    if let (Some(w), Some(v)) = (self.window.as_mut(), nv) {
                        let e = v & 0x7F;
                        if e != 10 {
                            w.effectiveness = Some(e);
                        }
                    }
                }
                "cur_player_move" | "cur_enemy_move" => {
                    let b = if name.contains("player") { 0 } else { 1 };
                    self.note_move(b, new);
                }
                _ => self.on_battler_change(store, path, nv, pv, &mut out),
            },
            Kind::Gen3 => {
                if name == "turn_action_number" {
                    if let Some(n) = nv {
                        self.gen3_action(store, n, &mut out);
                    }
                } else if name == "move_result_flags" {
                    if let (Some(w), Some(v)) = (self.window.as_mut(), nv) {
                        if v != 0 {
                            w.flags |= v;
                            w.evidence = true;
                            w.core = true;
                        }
                    }
                } else if path == format!("{}crit_multiplier", G3_SYS) {
                    if nv == Some(2) && pv != Some(2) {
                        if let Some(w) = self.window.as_mut() {
                            w.crit_edges += 1;
                        }
                    }
                } else if path == format!("{}current_move", G3_SYS) {
                    let attacker = store.i64_of(&format!("{}battler_attacker", G3_SYS)).unwrap_or(-1);
                    if let Some(w) = &self.window {
                        if attacker == w.battler as i64 || attacker < 0 {
                            let b = w.battler;
                            self.note_move(b, new);
                        }
                    }
                } else if path == format!("{}battler_target", G3_SYS) {
                    if let (Some(w), Some(v)) = (self.window.as_mut(), nv) {
                        if (0..4).contains(&v) && v as usize != w.battler {
                            w.target = Some(v as usize);
                        }
                    }
                } else {
                    self.on_battler_change(store, path, nv, pv, &mut out);
                }
            }
            Kind::Gen4 => match name {
                "turn_order_counter" | "total_turns" => {
                    let key = (store.i64_of(&p("total_turns")).unwrap_or(0), store.i64_of(&p("turn_order_counter")).unwrap_or(0));
                    self.gen4_action(store, key, &mut out);
                }
                "move_status_flags" => {
                    if let (Some(w), Some(v)) = (self.window.as_mut(), nv) {
                        if v != 0 {
                            w.flags |= v;
                            w.evidence = true;
                            w.core = true;
                        }
                        // flags are cleared between the hits of a multi-hit move
                        if rose(1 << 4) {
                            w.crit_edges += 1;
                        }
                    }
                }
                "battle_status_mask" => {
                    // BATTLE_STATUS_APPLY_SECONDARY_EFFECT (indirect secondary effects)
                    if rose(1 << 22) {
                        if let Some(w) = self.window.as_mut() {
                            w.effect_flag = true;
                        }
                    }
                }
                "multi_hit_num_hits" => {
                    if let (Some(w), Some(v)) = (self.window.as_mut(), nv) {
                        w.num_hits = w.num_hits.max(v);
                    }
                }
                "defender" => {
                    if let (Some(w), Some(v)) = (self.window.as_mut(), nv) {
                        let attacker = store.i64_of(&p("attacker")).unwrap_or(-1);
                        if attacker == w.battler as i64 && (0..4).contains(&v) && v as usize != w.battler {
                            w.target = Some(v as usize);
                        }
                    }
                }
                "move_current" => {
                    let attacker = store.i64_of(&p("attacker")).unwrap_or(-1);
                    if let Some(w) = &self.window {
                        if attacker == w.battler as i64 {
                            let b = w.battler;
                            self.note_move(b, new);
                        }
                    }
                }
                "attacker" => {
                    // the move's own values arrive with or after the attacker
                    if let (Some(w), Some(v)) = (self.window.as_mut(), nv) {
                        if v as usize == w.battler && w.move_name.is_none() {
                            if let Some(m) = store.str_of(&p("move_current")).filter(|m| !m.is_empty()) {
                                w.first_move = Some(m.clone());
                                w.move_name = Some(m);
                                w.core = true;
                            }
                            if let Some(t) = store.i64_of(&p("defender")) {
                                if (0..4).contains(&t) && t as usize != w.battler {
                                    w.target = Some(t as usize);
                                }
                            }
                        }
                    }
                }
                _ => self.on_battler_change(store, path, nv, pv, &mut out),
            },
        }
        out
    }

    /// Open a window even when one for the same battler has no evidence yet.
    fn force_open(&mut self, store: &PropertyStore, battler: usize, out: &mut Vec<MoveOutcome>) {
        if let Some(o) = self.close(store) {
            out.push(o);
        }
        let snaps = self.snaps(store);
        self.window = Some(Window { battler, snaps, ..Default::default() });
    }

    fn note_move(&mut self, battler: usize, new: &GameHookProperty) {
        let Some(w) = self.window.as_mut() else { return };
        if w.battler != battler {
            return;
        }
        let Some(name) = new.as_str().map(|s| s.to_string()).filter(|s| !s.is_empty() && s != "-" && s != "--") else { return };
        if w.first_move.is_none() {
            w.first_move = Some(name.clone());
        }
        w.move_name = Some(name);
        w.core = true;
    }

    fn gen3_action(&mut self, store: &PropertyStore, n: i64, out: &mut Vec<MoveOutcome>) {
        let key = (0, n);
        if self.last_key == Some(key) {
            return;
        }
        self.last_key = Some(key);
        if let Some(o) = self.close(store) {
            out.push(o);
        }
        self.window = None;
        let count = store.i64_of(&format!("{}battlers_count", G3_SYS)).filter(|c| (1..=4).contains(c)).unwrap_or(4);
        if !(0..count).contains(&n) {
            return;
        }
        let action = store.i64_of(&p(&format!("actions_by_turn_order_{}", n))).unwrap_or(-1);
        let battler = store.i64_of(&p(&format!("battler_by_turn_order_{}", n))).unwrap_or(-1);
        // B_ACTION_USE_MOVE
        if action == 0 && (0..4).contains(&battler) {
            let snaps = self.snaps(store);
            self.window = Some(Window { battler: battler as usize, snaps, ..Default::default() });
        }
    }

    fn gen4_action(&mut self, store: &PropertyStore, key: (i64, i64), out: &mut Vec<MoveOutcome>) {
        if self.last_key == Some(key) {
            return;
        }
        self.last_key = Some(key);
        if let Some(o) = self.close(store) {
            out.push(o);
        }
        self.window = None;
        let n = key.1;
        if !(0..4).contains(&n) {
            return;
        }
        let battler = store.i64_of(&p(&format!("action_order_{}", n))).unwrap_or(-1);
        if !(0..4).contains(&battler) {
            return;
        }
        // BATTLE_CONTROL_FIGHT
        let action = store.i64_of(&p(&format!("action_command_{}", battler))).unwrap_or(-1);
        if action == 13 {
            let snaps = self.snaps(store);
            self.window = Some(Window { battler: battler as usize, snaps, ..Default::default() });
        }
    }

    /// A battler value changed: evidence that a move did something, hit counts,
    /// flinches, charging turns, and (gen 1) the player's PP as a move-start signal.
    fn on_battler_change(&mut self, store: &PropertyStore, path: &str, nv: Option<i64>, pv: Option<i64>, out: &mut Vec<MoveOutcome>) {
        let Some((i, field)) = self.battlers.iter().enumerate().find_map(|(i, b)| {
            if b.hp == path {
                Some((i, "hp"))
            } else if b.pp.iter().any(|s| s == path) {
                Some((i, "pp"))
            } else if b.vol.iter().any(|s| s == path) {
                Some((i, "vol"))
            } else if b.stages.iter().any(|s| s == path) || b.status == path {
                Some((i, "state"))
            } else {
                None
            }
        }) else {
            return;
        };
        let (nv, pv) = (nv.unwrap_or(0), pv.unwrap_or(0));
        if field == "pp" && nv < pv {
            if self.kind == Kind::Gen1 {
                // only the player's PP: a second drop means a second move
                let again = self.window.as_ref().map(|w| w.battler != i || w.pp_drop).unwrap_or(true);
                if again {
                    self.force_open(store, i, out);
                }
            }
            if let Some(w) = self.window.as_mut() {
                if w.battler == i {
                    w.pp_drop = true;
                    w.evidence = true;
                    w.core = true;
                }
            }
            return;
        }
        let bits = self.bits;
        let Some(w) = self.window.as_mut() else { return };
        match field {
            "hp" => {
                if nv < pv && i != w.battler {
                    w.hp_drops[i.min(3)] += 1;
                    w.evidence = true;
                } else if i == w.battler || w.target == Some(i) {
                    w.evidence = true;
                }
            }
            "vol" => {
                let word = self.battlers[i].vol.iter().position(|s| s == path).unwrap_or(0);
                let rose = |(bw, mask): (usize, i64)| bw == word && nv & mask != 0 && pv & mask == 0;
                if rose(bits.flinch) {
                    w.flinch_seen[i.min(3)] = true;
                }
                if i == w.battler && rose(bits.charging) {
                    w.charging_rose = true;
                }
                if nv != pv {
                    w.evidence = true;
                }
            }
            _ => {
                if nv != pv {
                    w.evidence = true;
                }
            }
        }
    }

    // ---- finishing a move ---------------------------------------------------------

    fn close(&mut self, store: &PropertyStore) -> Option<MoveOutcome> {
        let w = self.window.take()?;
        let ran = match self.kind {
            Kind::Gen3 | Kind::Gen4 => w.core,
            _ => w.evidence,
        };
        if !ran {
            return None;
        }
        let side = self.side_of(w.battler)?;
        let target = w.target.unwrap_or_else(|| self.opponent(w.battler));
        if side == Side::Enemy && self.side_of(target) != Some(Side::Player) {
            // an enemy's move at the multi battle ally (or itself)
            if target == 2 {
                return None;
            }
        }
        let move_name = w.move_name.clone().or_else(|| self.current_move(store, w.battler))?;
        let mv = self.gen.move_db().get_move(&move_name).cloned();
        let move_name = mv.as_ref().map(|m| m.name.clone()).unwrap_or(move_name);
        let user0 = w.snaps.get(w.battler).cloned().unwrap_or_default();
        let target0 = w.snaps.get(target).cloned().unwrap_or_default();
        let user1 = self.snap(store, w.battler);
        let target1 = self.snap(store, target);

        let mut o = MoveOutcome { side: Some(side), battler: w.battler as u8, move_name, ..Default::default() };
        if let (Some(first), Some(last)) = (&w.first_move, &w.move_name) {
            if first != last && is_calling_move(first) {
                o.called_by = Some(self.gen.move_db().get_move(first).map(|m| m.name.clone()).unwrap_or_else(|| first.clone()));
            }
        }

        // what the game said
        let (mut missed, mut no_effect, mut failed, mut protected, mut se, mut nve, mut ohko) = (w.missed, false, false, false, false, false, w.ohko);
        match self.kind {
            Kind::Gen1 | Kind::Gen2 => {
                if let Some(e) = w.effectiveness {
                    if e == 0 {
                        no_effect = true;
                    } else if e > 10 {
                        se = true;
                    } else if e < 10 {
                        nve = true;
                    }
                }
                if no_effect {
                    // gens 1–2 report an immunity through the miss flag too
                    missed = false;
                }
            }
            Kind::Gen3 => {
                let f = w.flags;
                missed = f & 1 != 0;
                se = f & 2 != 0;
                nve = f & 4 != 0;
                no_effect = f & 8 != 0;
                ohko = ohko || f & 16 != 0;
                failed = f & 32 != 0;
            }
            Kind::Gen4 => {
                let f = w.flags;
                missed = f & 1 != 0;
                se = f & 2 != 0;
                nve = f & 4 != 0;
                no_effect = f & (1 << 3 | 1 << 11 | 1 << 18 | 1 << 20) != 0;
                ohko = ohko || f & (1 << 5) != 0;
                failed = f & (1 << 6 | 1 << 12) != 0;
                protected = f & (1 << 15) != 0;
                let invulnerable = f & (1 << 16) != 0;
                if invulnerable {
                    missed = false;
                    protected = true;
                }
            }
        }
        let charging = w.charging_rose && w.hp_drops[target.min(3)] == 0 && !missed;
        let landed = !missed && !no_effect && !protected && !failed && !charging;
        let hp_hits = w.hp_drops[target.min(3)];
        let damaging = mv.as_ref().map(|m| rolls_crit(m)).unwrap_or(false);
        o.missed = missed;
        o.no_effect = no_effect;
        o.failed = failed;
        o.protected = protected;
        o.super_effective = se && landed;
        o.not_very_effective = nve && landed;
        o.ohko = ohko;
        o.ko = target0.hp > 0 && target1.hp == 0 && landed;
        o.hits = if landed && damaging { (hp_hits as i64).max(w.num_hits).max(1) as u32 } else { 0 };

        let Some(mv) = mv else { return Some(o) };
        let focus = self.vol_bit(&user0, self.bits.focus);
        let (acc, crit) = self.chances(&mv, &user0, &target0, side == Side::Enemy, focus);
        if !charging && !no_effect && !protected && !(failed && !missed) {
            if let Some(p) = acc {
                o.accuracy_rolled = true;
                o.hit_chance = Some(p);
            }
        }
        if landed && damaging && crit > 0.0 {
            // gen 1 works out a multi-hit move's damage (and crit) once
            o.crit_rolls = if self.kind == Kind::Gen1 { 1 } else { o.hits.max(1) };
            o.crit_chance = Some(crit);
            o.crits = w.crit_edges.min(o.crit_rolls);
        }
        if let Some(info) = effect_info(&mv) {
            if landed {
                let ctx = EffectCtx { w: &w, target, user0: &user0, user1: &user1, target0: &target0, target1: &target1 };
                match self.effect_result(&info, &ctx) {
                    Some(EffectResult::Blocked) => o.effect_blocked = true,
                    Some(EffectResult::Rolled(applied)) => {
                        o.effect_rolled = true;
                        o.effect_applied = applied;
                        o.effect_chance = Some(self.effect_chance(&info, side, &user0, store));
                    }
                    None => {}
                }
            }
        }
        Some(o)
    }

    fn current_move(&self, store: &PropertyStore, battler: usize) -> Option<String> {
        let path = match self.kind {
            Kind::Gen1 => p(if battler == 0 { "player_move_num" } else { "enemy_move_num" }),
            Kind::Gen2 => p(if battler == 0 { "cur_player_move" } else { "cur_enemy_move" }),
            Kind::Gen3 => format!("{}current_move", G3_SYS),
            Kind::Gen4 => p("move_current"),
        };
        store.str_of(&path).filter(|s| !s.is_empty())
    }

    fn effect_chance(&self, info: &EffectInfo, side: Side, user: &Snap, store: &PropertyStore) -> f64 {
        let mut p = info.chance / 100.0;
        match self.kind {
            Kind::Gen2 => {
                // `BattleCommand_EffectChance`: the move struct's own chance byte, out of 256
                let path = p_(if side == Side::Player { "player_move_chance" } else { "enemy_move_chance" });
                if let Some(byte) = store.i64_of(&path).filter(|b| (1..=255).contains(b)) {
                    if info.kind != "tri_attack" {
                        p = byte as f64 / 256.0;
                    }
                }
            }
            Kind::Gen3 | Kind::Gen4 => {
                if user.ability.as_deref() == Some("Serene Grace") {
                    p *= 2.0;
                }
            }
            Kind::Gen1 => {}
        }
        p.clamp(0.0, 1.0)
    }

    /// The hit chance (None = can't miss) and the chance of a crit per hit,
    /// from the router's own damage calculator with the stages at the start of
    /// the move.
    fn chances(&self, mv: &Move, user: &Snap, target: &Snap, attacker_is_enemy: bool, focus: bool) -> (Option<f64>, f64) {
        let fallback_acc = mv.accuracy.map(|a| (a as f64 / 100.0).min(1.0));
        let make = |s: &Snap| -> Option<EnemyPkmn> {
            let mut m = self.gen.create_trainer_pkmn(s.species.as_deref()?, s.level.max(1))?;
            if let Some(a) = &s.ability {
                m.ability = a.clone();
            }
            m.held_item = s.item.clone();
            Some(m)
        };
        let (Some(a), Some(d)) = (make(user), make(target)) else {
            return (fallback_acc, base_crit(self.kind));
        };
        let atk_stages = StageModifiers { accuracy_stage: user.stages[ST_ACC], evasion_stage: user.stages[ST_EVA], ..Default::default() };
        let def_stages = StageModifiers { accuracy_stage: target.stages[ST_ACC], evasion_stage: target.stages[ST_EVA], ..Default::default() };
        let mut args = xpr_calc::DamageArgs::new(&a, mv, &d);
        args.attacking_stages = Some(&atk_stages);
        args.defending_stages = Some(&def_stages);
        args.attacker_is_enemy = attacker_is_enemy;
        if self.kind == Kind::Gen1 && focus {
            args.custom_move_data = gc::FLAVOR_FOCUS_ENERGY;
        }
        let acc = xpr_calc::get_move_accuracy(&self.gen, &args).map(|p| (p / 100.0).clamp(0.0, 1.0));
        let mut crit = xpr_calc::get_crit_rate(&self.gen, &args);
        if focus && self.kind != Kind::Gen1 {
            crit = bump_crit(self.kind, crit);
        }
        (acc, crit)
    }

    fn effect_result(&self, info: &EffectInfo, c: &EffectCtx) -> Option<EffectResult> {
        let ko = c.target0.hp > 0 && c.target1.hp == 0;
        let status_bit = |kind: &str| -> i64 {
            match kind {
                "paralyze" => PAR,
                "burn" => BRN,
                "freeze" => FRZ,
                "poison" | "toxic" => PSN | self.bits.toxic_status,
                "sleep" => SLP,
                _ => 0,
            }
        };
        let types = c.target0.species.as_deref().and_then(|s| self.gen.pkmn_db().get_pkmn(s)).map(|s| (s.first_type.clone(), s.second_type.clone()));
        let has_type = |t: &str| types.as_ref().map(|(a, b)| a == t || b == t).unwrap_or(false);
        match info.kind.as_str() {
            "paralyze" | "burn" | "freeze" | "poison" | "toxic" | "sleep" | "tri_attack" => {
                if ko || c.target0.status != 0 {
                    return Some(EffectResult::Blocked);
                }
                let immune = match info.kind.as_str() {
                    "burn" => has_type("Fire"),
                    "freeze" => has_type("Ice"),
                    "poison" | "toxic" => has_type("Poison") || (self.kind != Kind::Gen1 && has_type("Steel")),
                    _ => false,
                };
                if immune {
                    return Some(EffectResult::Blocked);
                }
                let mask = if info.kind == "tri_attack" { PAR | BRN | FRZ } else { status_bit(&info.kind) };
                let mut applied = c.target1.status & mask != 0;
                if info.kind == "toxic" {
                    if let Some(tv) = self.bits.toxic_vol {
                        applied = applied || self.vol_bit(c.target1, tv);
                    }
                }
                Some(EffectResult::Rolled(applied || (self.kind == Kind::Gen4 && c.w.effect_flag)))
            }
            "confuse" => {
                if ko || self.vol_bit(c.target0, self.bits.confuse) {
                    return Some(EffectResult::Blocked);
                }
                Some(EffectResult::Rolled(self.vol_bit(c.target1, self.bits.confuse) || (self.kind == Kind::Gen4 && c.w.effect_flag)))
            }
            "flinch" => {
                if ko {
                    return Some(EffectResult::Blocked);
                }
                let seen = c.w.flinch_seen[c.target.min(3)] || self.vol_bit(c.target1, self.bits.flinch);
                let failed = self.kind == Kind::Gen2 && c.w.effect_failed;
                Some(EffectResult::Rolled(seen && !failed))
            }
            k if k.starts_with("raise_self_") || k.starts_with("lower_self_") || k.starts_with("raise_enemy_") || k.starts_with("lower_enemy_") => {
                let (before, after) = if info.on_self { (c.user0, c.user1) } else { (c.target0, c.target1) };
                if !info.on_self && ko {
                    return Some(EffectResult::Blocked);
                }
                let stat = info.stat.as_deref().unwrap_or("");
                let idxs: Vec<usize> = if stat == "all" { vec![ST_ATK, ST_DEF, ST_SPE, ST_SPA, ST_SPD] } else { stage_index(stat).into_iter().collect() };
                if idxs.is_empty() {
                    return None;
                }
                let up = info.modifier > 0;
                let capped = idxs.iter().all(|&i| if up { before.stages[i] >= 6 } else { before.stages[i] <= -6 });
                if capped {
                    return Some(EffectResult::Blocked);
                }
                let moved = idxs.iter().any(|&i| if up { after.stages[i] > before.stages[i] } else { after.stages[i] < before.stages[i] });
                Some(EffectResult::Rolled(moved || (self.kind == Kind::Gen4 && c.w.effect_flag)))
            }
            _ => None,
        }
    }
}

struct EffectCtx<'a> {
    w: &'a Window,
    target: usize,
    user0: &'a Snap,
    user1: &'a Snap,
    target0: &'a Snap,
    target1: &'a Snap,
}

enum EffectResult {
    Blocked,
    Rolled(bool),
}

fn is_calling_move(name: &str) -> bool {
    let n: String = name.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase();
    matches!(n.as_str(), "metronome" | "sleeptalk" | "mirrormove" | "assist" | "naturepower" | "copycat" | "mefirst")
}

/// Whether a hit of this move rolls for a critical hit.
fn rolls_crit(mv: &Move) -> bool {
    if mv.base_power.unwrap_or(0) <= 0 {
        return false;
    }
    if ["level_damage", "fixed_damage", "one_hit_ko"].iter().any(|f| mv.has_flavor(f)) {
        return false;
    }
    let n: String = mv.name.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase();
    !matches!(
        n.as_str(),
        "seismictoss" | "nightshade" | "dragonrage" | "sonicboom" | "superfang" | "psywave" | "counter" | "mirrorcoat" | "bide" | "endeavor" | "futuresight" | "doomdesire" | "metalburst"
    )
}

fn base_crit(kind: Kind) -> f64 {
    match kind {
        Kind::Gen1 => 0.0,
        Kind::Gen2 => 17.0 / 256.0,
        _ => 1.0 / 16.0,
    }
}

/// One more crit stage (Focus Energy), for gens 2–4.
fn bump_crit(kind: Kind, rate: f64) -> f64 {
    let table: &[f64] = match kind {
        Kind::Gen2 => &[17.0 / 256.0, 32.0 / 256.0, 64.0 / 256.0, 85.0 / 256.0, 128.0 / 256.0],
        _ => &[1.0 / 16.0, 1.0 / 8.0, 1.0 / 4.0, 1.0 / 3.0, 1.0 / 2.0],
    };
    if rate <= 0.0 {
        return rate;
    }
    let idx = table.iter().enumerate().min_by(|a, b| (a.1 - rate).abs().partial_cmp(&(b.1 - rate).abs()).unwrap()).map(|(i, _)| i).unwrap_or(0);
    // gen 2 Focus Energy is +1 stage, gens 3-4 +2
    let step = if kind == Kind::Gen2 { 1 } else { 2 };
    table[(idx + step).min(table.len() - 1)]
}

/// Sums battles' numbers into one pair (for tests and the app's run view).
pub fn sum_battles<'a>(battles: impl IntoIterator<Item = &'a BattleMoveStats>) -> SidePair {
    let mut out = SidePair::default();
    for b in battles {
        out.add_signed(&b.stats, 1);
    }
    out
}

/// `other` counters as JSON (kept here so the keys are in one place).
pub fn other_keys() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("player_crits", "crits the player landed"),
        ("enemy_crits", "crits taken"),
        ("player_misses", "the player's misses"),
        ("enemy_misses", "enemy misses against the player"),
    ])
}
