//! Move stats: what happened each time a move ran in battle (hits, misses,
//! crits, secondary effects), recorded by the recorder and stored in the route.
//!
//! The route keeps one [`BattleMoveStats`] on each trainer battle event and a
//! [`RunMoveStats`] at the top level (wild battles, reset attempts and the run
//! total). The total is worked out again on every save from the battles still
//! in the route plus the wild battles, so it can't drift. All of it is a
//! record of what happened, not part of the plan: recalculation ignores it.
//!
//! The JSON shape is a contract with the STP overlay and Scott's Thoughts Data
//! (see `docs/move_stats.md`).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use indexmap::IndexMap;
use serde_json::{Map, Value};

use xpr_data::model::Move;
use xpr_data::GenData;

pub const MOVE_STATS_KEY: &str = "move_stats";
pub const MOVE_STATS_FORMAT: i64 = 1;

/// The three groups every list sorts by (spec §3).
pub const INTEREST_LUCK: i64 = 1;
pub const INTEREST_CRIT_ONLY: i64 = 2;
pub const INTEREST_NO_LUCK: i64 = 3;

/// The counts for one move on one side.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MoveCounts {
    pub uses: i64,
    pub acc_rolls: i64,
    pub hits: i64,
    pub misses: i64,
    pub expected_hits: f64,
    pub hit_variance: f64,
    pub listed_accuracy: Option<i64>,
    /// the move's type ("Fire"), so a reader can colour it without a move table
    pub move_type: Option<String>,
    pub crit_rolls: i64,
    pub crits: i64,
    pub expected_crits: f64,
    pub crit_variance: f64,
    pub effect_rolls: i64,
    pub effects: i64,
    pub expected_effects: f64,
    pub effect_variance: f64,
    pub effect_blocked: i64,
    pub effect_kind: Option<String>,
    pub listed_effect_chance: Option<f64>,
    pub failed: i64,
    pub no_effect: i64,
    pub protected: i64,
    pub super_effective: i64,
    pub not_very_effective: i64,
    pub ohko: i64,
    pub kos: i64,
    pub multi_hit: BTreeMap<String, i64>,
    pub called_by: BTreeMap<String, i64>,
    pub interest: i64,
}

fn add_map(a: &mut BTreeMap<String, i64>, b: &BTreeMap<String, i64>, sign: i64) {
    for (k, v) in b {
        let e = a.entry(k.clone()).or_insert(0);
        *e += sign * v;
        if *e == 0 {
            a.remove(k);
        }
    }
}

fn num(v: &Value, key: &str) -> i64 {
    v.get(key).and_then(|x| x.as_i64().or_else(|| x.as_f64().map(|f| f as i64))).unwrap_or(0)
}

fn fnum(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(|x| x.as_f64()).unwrap_or(0.0)
}

fn round4(f: f64) -> Value {
    let r = (f * 10000.0).round() / 10000.0;
    serde_json::Number::from_f64(r).map(Value::Number).unwrap_or(Value::from(0))
}

fn map_value(m: &BTreeMap<String, i64>) -> Value {
    let mut o = Map::new();
    for (k, v) in m {
        o.insert(k.clone(), Value::from(*v));
    }
    Value::Object(o)
}

fn map_from(v: Option<&Value>) -> BTreeMap<String, i64> {
    let mut m = BTreeMap::new();
    if let Some(Value::Object(o)) = v {
        for (k, x) in o {
            if let Some(i) = x.as_i64() {
                m.insert(k.clone(), i);
            }
        }
    }
    m
}

impl MoveCounts {
    /// Add (`sign` 1) or take away (`sign` -1) another set of counts.
    pub fn add_signed(&mut self, o: &MoveCounts, sign: i64) {
        let s = sign as f64;
        self.uses += sign * o.uses;
        self.acc_rolls += sign * o.acc_rolls;
        self.hits += sign * o.hits;
        self.misses += sign * o.misses;
        self.expected_hits += s * o.expected_hits;
        self.hit_variance += s * o.hit_variance;
        self.crit_rolls += sign * o.crit_rolls;
        self.crits += sign * o.crits;
        self.expected_crits += s * o.expected_crits;
        self.crit_variance += s * o.crit_variance;
        self.effect_rolls += sign * o.effect_rolls;
        self.effects += sign * o.effects;
        self.expected_effects += s * o.expected_effects;
        self.effect_variance += s * o.effect_variance;
        self.effect_blocked += sign * o.effect_blocked;
        self.failed += sign * o.failed;
        self.no_effect += sign * o.no_effect;
        self.protected += sign * o.protected;
        self.super_effective += sign * o.super_effective;
        self.not_very_effective += sign * o.not_very_effective;
        self.ohko += sign * o.ohko;
        self.kos += sign * o.kos;
        add_map(&mut self.multi_hit, &o.multi_hit, sign);
        add_map(&mut self.called_by, &o.called_by, sign);
        if self.listed_accuracy.is_none() {
            self.listed_accuracy = o.listed_accuracy;
        }
        if self.move_type.is_none() {
            self.move_type = o.move_type.clone();
        }
        if self.effect_kind.is_none() {
            self.effect_kind = o.effect_kind.clone();
        }
        if self.listed_effect_chance.is_none() {
            self.listed_effect_chance = o.listed_effect_chance;
        }
        if self.interest == 0 {
            self.interest = o.interest;
        }
    }

    pub fn add(&mut self, o: &MoveCounts) {
        self.add_signed(o, 1);
    }

    pub fn to_json(&self) -> Value {
        let mut o = Map::new();
        let mut put = |k: &str, v: Value| {
            o.insert(k.to_string(), v);
        };
        put("interest", Value::from(self.interest));
        put("uses", Value::from(self.uses));
        put("acc_rolls", Value::from(self.acc_rolls));
        put("hits", Value::from(self.hits));
        put("misses", Value::from(self.misses));
        put("expected_hits", round4(self.expected_hits));
        put("hit_variance", round4(self.hit_variance));
        put("listed_accuracy", self.listed_accuracy.map(Value::from).unwrap_or(Value::Null));
        put("type", self.move_type.clone().map(Value::String).unwrap_or(Value::Null));
        put("crit_rolls", Value::from(self.crit_rolls));
        put("crits", Value::from(self.crits));
        put("expected_crits", round4(self.expected_crits));
        put("crit_variance", round4(self.crit_variance));
        put("effect_rolls", Value::from(self.effect_rolls));
        put("effects", Value::from(self.effects));
        put("expected_effects", round4(self.expected_effects));
        put("effect_variance", round4(self.effect_variance));
        put("effect_blocked", Value::from(self.effect_blocked));
        put("effect_kind", self.effect_kind.clone().map(Value::String).unwrap_or(Value::Null));
        put("listed_effect_chance", self.listed_effect_chance.map(round4).unwrap_or(Value::Null));
        put("failed", Value::from(self.failed));
        put("no_effect", Value::from(self.no_effect));
        put("protected", Value::from(self.protected));
        put("super_effective", Value::from(self.super_effective));
        put("not_very_effective", Value::from(self.not_very_effective));
        put("ohko", Value::from(self.ohko));
        put("kos", Value::from(self.kos));
        put("multi_hit", map_value(&self.multi_hit));
        put("called_by", map_value(&self.called_by));
        Value::Object(o)
    }

    pub fn from_json(v: &Value) -> MoveCounts {
        MoveCounts {
            uses: num(v, "uses"),
            acc_rolls: num(v, "acc_rolls"),
            hits: num(v, "hits"),
            misses: num(v, "misses"),
            expected_hits: fnum(v, "expected_hits"),
            hit_variance: fnum(v, "hit_variance"),
            listed_accuracy: v.get("listed_accuracy").and_then(|x| x.as_i64()),
            move_type: v.get("type").and_then(|x| x.as_str()).map(|s| s.to_string()),
            crit_rolls: num(v, "crit_rolls"),
            crits: num(v, "crits"),
            expected_crits: fnum(v, "expected_crits"),
            crit_variance: fnum(v, "crit_variance"),
            effect_rolls: num(v, "effect_rolls"),
            effects: num(v, "effects"),
            expected_effects: fnum(v, "expected_effects"),
            effect_variance: fnum(v, "effect_variance"),
            effect_blocked: num(v, "effect_blocked"),
            effect_kind: v.get("effect_kind").and_then(|x| x.as_str()).map(|s| s.to_string()),
            listed_effect_chance: v.get("listed_effect_chance").and_then(|x| x.as_f64()),
            failed: num(v, "failed"),
            no_effect: num(v, "no_effect"),
            protected: num(v, "protected"),
            super_effective: num(v, "super_effective"),
            not_very_effective: num(v, "not_very_effective"),
            ohko: num(v, "ohko"),
            kos: num(v, "kos"),
            multi_hit: map_from(v.get("multi_hit")),
            called_by: map_from(v.get("called_by")),
            interest: num(v, "interest"),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.uses == 0 && self.acc_rolls == 0 && self.crit_rolls == 0 && self.effect_rolls == 0
    }
}

/// Moves by name for one side.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SideStats {
    pub moves: IndexMap<String, MoveCounts>,
}

impl SideStats {
    pub fn entry(&mut self, name: &str) -> &mut MoveCounts {
        self.moves.entry(name.to_string()).or_default()
    }

    pub fn add_signed(&mut self, o: &SideStats, sign: i64) {
        for (k, v) in &o.moves {
            self.entry(k).add_signed(v, sign);
        }
        self.moves.retain(|_, v| !v.is_empty());
    }

    /// Luck moves first, then crit-only, then no-luck; most used first in each.
    pub fn sorted_names(&self) -> Vec<String> {
        let mut names: Vec<&String> = self.moves.keys().collect();
        names.sort_by(|a, b| {
            let (ma, mb) = (&self.moves[*a], &self.moves[*b]);
            let ia = if ma.interest == 0 { 9 } else { ma.interest };
            let ib = if mb.interest == 0 { 9 } else { mb.interest };
            ia.cmp(&ib).then(mb.uses.cmp(&ma.uses)).then(a.cmp(b))
        });
        names.into_iter().cloned().collect()
    }

    pub fn to_json(&self) -> Value {
        let mut o = Map::new();
        for name in self.sorted_names() {
            o.insert(name.clone(), self.moves[&name].to_json());
        }
        Value::Object(o)
    }

    pub fn from_json(v: Option<&Value>) -> SideStats {
        let mut s = SideStats::default();
        if let Some(Value::Object(o)) = v {
            for (k, x) in o {
                s.moves.insert(k.clone(), MoveCounts::from_json(x));
            }
        }
        s
    }
}

/// Counts that belong to no one move (turns lost to full paralysis...).
pub type OtherStats = BTreeMap<String, i64>;

/// Both sides of a battle, or of a total.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SidePair {
    pub player: SideStats,
    pub enemy: SideStats,
    pub other: OtherStats,
}

impl SidePair {
    pub fn add_signed(&mut self, o: &SidePair, sign: i64) {
        self.player.add_signed(&o.player, sign);
        self.enemy.add_signed(&o.enemy, sign);
        add_map(&mut self.other, &o.other, sign);
    }

    pub fn is_empty(&self) -> bool {
        self.player.moves.is_empty() && self.enemy.moves.is_empty() && self.other.values().all(|v| *v == 0)
    }

    fn write(&self, o: &mut Map<String, Value>) {
        o.insert("player".into(), self.player.to_json());
        o.insert("enemy".into(), self.enemy.to_json());
        o.insert("other".into(), map_value(&self.other));
    }

    pub fn to_json(&self) -> Value {
        let mut o = Map::new();
        self.write(&mut o);
        Value::Object(o)
    }

    pub fn from_json(v: Option<&Value>) -> SidePair {
        SidePair {
            player: SideStats::from_json(v.and_then(|v| v.get("player"))),
            enemy: SideStats::from_json(v.and_then(|v| v.get("enemy"))),
            other: map_from(v.and_then(|v| v.get("other"))),
        }
    }
}

/// One battle's numbers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BattleMoveStats {
    /// unique and stable; the overlay de-duplicates on it
    pub battle_id: String,
    pub started_at: String,
    pub finished_at: String,
    /// `won`, `lost`, `fled`, `caught`, `reset` or `unknown`
    pub outcome: String,
    /// `trainer` or `wild`
    pub kind: String,
    pub trainer: Option<String>,
    pub stats: SidePair,
}

impl BattleMoveStats {
    pub fn to_json(&self) -> Value {
        let mut o = Map::new();
        o.insert("battle_id".into(), Value::String(self.battle_id.clone()));
        o.insert("kind".into(), Value::String(self.kind.clone()));
        o.insert("trainer".into(), self.trainer.clone().map(Value::String).unwrap_or(Value::Null));
        o.insert("started_at".into(), Value::String(self.started_at.clone()));
        o.insert("finished_at".into(), Value::String(self.finished_at.clone()));
        o.insert("outcome".into(), Value::String(self.outcome.clone()));
        self.stats.write(&mut o);
        Value::Object(o)
    }

    pub fn from_json(v: &Value) -> Option<BattleMoveStats> {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        if !v.is_object() {
            return None;
        }
        Some(BattleMoveStats {
            battle_id: s("battle_id"),
            started_at: s("started_at"),
            finished_at: s("finished_at"),
            outcome: s("outcome"),
            kind: {
                let k = s("kind");
                if k.is_empty() {
                    "trainer".into()
                } else {
                    k
                }
            },
            trainer: v.get("trainer").and_then(|x| x.as_str()).map(|x| x.to_string()),
            stats: SidePair::from_json(Some(v)),
        })
    }
}

/// The run-level object at the top of the route file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunMoveStats {
    pub run_id: String,
    pub game: String,
    pub solo_species: String,
    pub recorded_with: String,
    pub wild: SidePair,
    pub reset_attempts: Vec<BattleMoveStats>,
}

impl RunMoveStats {
    pub fn new(game: &str, solo_species: &str) -> RunMoveStats {
        RunMoveStats {
            run_id: new_id(),
            game: game.to_string(),
            solo_species: solo_species.to_string(),
            recorded_with: format!("router {}", env!("CARGO_PKG_VERSION")),
            ..Default::default()
        }
    }

    /// `total` is passed in: the router sums its trainer battles and adds `wild`.
    pub fn to_json(&self, total: &SidePair) -> Value {
        let mut o = Map::new();
        o.insert("format".into(), Value::from(MOVE_STATS_FORMAT));
        o.insert("run_id".into(), Value::String(self.run_id.clone()));
        o.insert("game".into(), Value::String(self.game.clone()));
        o.insert("solo_species".into(), Value::String(self.solo_species.clone()));
        o.insert("recorded_with".into(), Value::String(self.recorded_with.clone()));
        o.insert("total".into(), total.to_json());
        o.insert("wild".into(), self.wild.to_json());
        o.insert("reset_attempts".into(), Value::Array(self.reset_attempts.iter().map(|b| b.to_json()).collect()));
        Value::Object(o)
    }

    pub fn from_json(v: &Value) -> Option<RunMoveStats> {
        if !v.is_object() {
            return None;
        }
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        Some(RunMoveStats {
            run_id: s("run_id"),
            game: s("game"),
            solo_species: s("solo_species"),
            recorded_with: s("recorded_with"),
            wild: SidePair::from_json(v.get("wild")),
            reset_attempts: match v.get("reset_attempts") {
                Some(Value::Array(items)) => items.iter().filter_map(BattleMoveStats::from_json).collect(),
                _ => Vec::new(),
            },
        })
    }
}

/// A fresh id: 16 hex digits from the clock, the process and a counter.
pub fn new_id() -> String {
    use std::hash::{Hash, Hasher};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0).hash(&mut h);
    std::process::id().hash(&mut h);
    COUNTER.fetch_add(1, Ordering::SeqCst).hash(&mut h);
    let a = h.finish();
    COUNTER.fetch_add(1, Ordering::SeqCst).hash(&mut h);
    format!("{:016x}{:08x}", a, h.finish() as u32)
}

/// What a move's secondary effect is, for display and for working out
/// whether it fired.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectInfo {
    /// `paralyze`, `burn`, `freeze`, `poison`, `toxic`, `sleep`, `confuse`,
    /// `flinch`, `raise_self_<stat>`, `lower_enemy_<stat>`, `tri_attack`, ...
    pub kind: String,
    /// the listed chance in percent
    pub chance: f64,
    pub status: Option<String>,
    pub stat: Option<String>,
    pub modifier: i64,
    pub on_self: bool,
    pub flinch: bool,
}

/// The move's chance-based secondary effect, if it has one (an effect
/// listed at 100% is the move's main effect, not a secondary one).
pub fn effect_info(mv: &Move) -> Option<EffectInfo> {
    let chancy: Vec<_> = mv.effects.iter().filter(|e| e.chance_exact.map(|c| c > 0.0 && c < 100.0).unwrap_or(false)).collect();
    let first = chancy.first()?;
    if chancy.len() > 1 && chancy.iter().all(|e| e.status.is_some()) && chancy.iter().any(|e| e.status != first.status) {
        // Tri Attack: one roll, one of three statuses
        let total: f64 = chancy.iter().map(|e| e.chance_exact.unwrap_or(0.0)).sum();
        return Some(EffectInfo {
            kind: "tri_attack".into(),
            chance: (total * 100.0).round() / 100.0,
            status: None,
            stat: None,
            modifier: 0,
            on_self: false,
            flinch: false,
        });
    }
    let chance = first.chance_exact.unwrap_or(0.0);
    if first.flinch {
        return Some(EffectInfo { kind: "flinch".into(), chance, status: None, stat: None, modifier: 0, on_self: false, flinch: true });
    }
    if let Some(status) = &first.status {
        let kind = match status.as_str() {
            "Paralysis" => "paralyze",
            "Burn" => "burn",
            "Freeze" => "freeze",
            "Poison" => "poison",
            "Toxic" => "toxic",
            "Sleep" => "sleep",
            "Confusion" => "confuse",
            other => return Some(EffectInfo { kind: other.to_lowercase().replace(' ', "_"), chance, status: Some(other.to_string()), stat: None, modifier: 0, on_self: false, flinch: false }),
        };
        return Some(EffectInfo { kind: kind.into(), chance, status: Some(status.clone()), stat: None, modifier: 0, on_self: false, flinch: false });
    }
    if let (Some(stat), Some(m)) = (&first.stat, first.modifier) {
        let on_self = first.target.as_deref() == Some("self");
        let stats: Vec<&str> = chancy.iter().filter_map(|e| e.stat.as_deref()).collect();
        let stat_name = if stats.len() == 2 && stats.contains(&"spa") && stats.contains(&"spd") {
            // gen 1's single Special stat, listed as both halves
            "special".to_string()
        } else if stat == "omni" || (stats.len() > 1 && stats.len() == chancy.len()) {
            "all".to_string()
        } else {
            stat.clone()
        };
        let kind = format!("{}_{}_{}", if m > 0 { "raise" } else { "lower" }, if on_self { "self" } else { "enemy" }, stat_name);
        return Some(EffectInfo { kind, chance, status: None, stat: Some(stat_name), modifier: m, on_self, flinch: false });
    }
    None
}

/// The group a move sorts into (spec §3): 1 when anything about it is luck
/// beyond crits, 2 when only crits are random, 3 when nothing is.
pub fn interest_of(mv: &Move) -> i64 {
    if mv.accuracy.is_some() || effect_info(mv).is_some() {
        return INTEREST_LUCK;
    }
    if mv.base_power.unwrap_or(0) > 0 {
        return INTEREST_CRIT_ONLY;
    }
    INTEREST_NO_LUCK
}

pub fn interest_by_name(gen: &GenData, move_name: &str) -> i64 {
    gen.move_db().get_move(move_name).map(|m| interest_of(m)).unwrap_or(INTEREST_LUCK)
}
