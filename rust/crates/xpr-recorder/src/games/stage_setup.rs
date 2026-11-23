//! The stat stages the player sets up in a trainer battle, for the battle
//! event's per-matchup stat-stage steppers (`stat_stage_setup`): how many
//! times each of the solo mon's moves changed a stage, per enemy mon. Three
//! Swords Dances against the leader's first mon come out as Swords Dance 3 on
//! that matchup, two Charge Beams that raised Special Attack as Charge Beam 2.
//!
//! The mappers show the stages of the two active mons, not who changed them,
//! so a change counts only when one of the solo mon's moves explains it:
//! raising its own stages (or lowering them: Superpower, Overheat) or
//! lowering the enemy's (Growl, an Acid that dropped Defense). Whatever the
//! enemy does (its Growl, its own Swords Dance) is left out, as are abilities:
//! the battle page already applies Intimidate from the mons' abilities.
//! A move whose stages change one after another (Dragon Dance: Attack, then
//! Speed, a message apart) is put together over [`MOVE_WINDOW`].

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use serde_json::Value;

use xpr_core::consts;
use xpr_data::model::CustomMoveData;
use xpr_data::{GenData, MoveDB};
use xpr_engine::EventDefinition;

use crate::gamehook::PropertyStore;

/// The stats in the order the stages are kept in here.
const STATS: [&str; 7] = [consts::ATK, consts::DEF, consts::SPE, consts::SPA, consts::SPD, consts::ACC, consts::EV];

/// How long the stage changes of one move take to all show up.
pub const MOVE_WINDOW: Duration = Duration::from_secs(4);

/// After an enemy comes out, its stages are its own (they start over, or an
/// Intimidate lowers them) for this long, so a change to them is no move of
/// the player's.
pub const SEND_OUT_SETTLE: Duration = Duration::from_secs(3);

type Stages = [i64; 7];

/// The mapper paths of the active mons' stages (index as [`STATS`]), or
/// `None` where the mapper has none.
#[derive(Clone, Debug, Default)]
pub struct StageKeys {
    player: [Option<String>; 7],
    enemy: [Option<String>; 7],
}

impl StageKeys {
    /// The standard mappers keep the stages under
    /// `battle.{player,opponent}.active_pokemon.modifiers` (gen 1 has one
    /// `special` for both special stats), the deprecated ones as
    /// `battle.{yourPokemon,enemyPokemon}.modStage*` (gen 1's enemy:
    /// `modEnemyStage*`).
    pub fn resolve(store: &PropertyStore) -> StageKeys {
        Self::with_prefixes(store, "battle.player.active_pokemon", "battle.opponent.active_pokemon")
    }

    /// As [`Self::resolve`], with the standard mappers' stages read from
    /// `{player_prefix}.modifiers` and `{enemy_prefix}.modifiers` (gen 5 keeps
    /// a battle struct per party slot, and its `active_pokemon` is slot 0's).
    pub fn with_prefixes(store: &PropertyStore, player_prefix: &str, enemy_prefix: &str) -> StageKeys {
        let names: [(&[&str], &[&str]); 7] = [
            (&["attack"], &["Attack"]),
            (&["defense"], &["Defense"]),
            (&["speed"], &["Speed"]),
            (&["special_attack", "special"], &["SpecialAttack", "Special"]),
            (&["special_defense", "special"], &["SpecialDefense", "Special"]),
            (&["accuracy"], &["Accuracy"]),
            (&["evasion"], &["Evasion"]),
        ];
        let find = |candidates: Vec<String>| candidates.iter().find_map(|c| store.resolve_case(c));
        let mut keys = StageKeys::default();
        for (i, (std_names, dep_names)) in names.iter().enumerate() {
            keys.player[i] = find(
                std_names
                    .iter()
                    .map(|n| format!("{}.modifiers.{}", player_prefix, n))
                    .chain(dep_names.iter().map(|n| format!("battle.yourPokemon.modStage{}", n)))
                    .collect(),
            );
            keys.enemy[i] = find(
                std_names
                    .iter()
                    .map(|n| format!("{}.modifiers.{}", enemy_prefix, n))
                    .chain(dep_names.iter().map(|n| format!("battle.enemyPokemon.modStage{}", n)))
                    .chain(dep_names.iter().map(|n| format!("battle.enemyPokemon.modEnemyStage{}", n)))
                    .collect(),
            );
        }
        keys
    }

    /// Whether the mapper shows the stages the steppers use (attack to
    /// special defense) for both mons.
    pub fn is_usable(&self) -> bool {
        self.player[..5].iter().chain(self.enemy[..5].iter()).all(|k| k.is_some())
    }

    fn read_side(store: &PropertyStore, keys: &[Option<String>; 7]) -> Option<Stages> {
        let mut out = [0; 7];
        for (i, key) in keys.iter().enumerate() {
            match key {
                // the mappers give accuracy and evasion too, but nothing needs them
                None if i >= 5 => {}
                None => return None,
                Some(k) => out[i] = stage_value(&store.get_value(Some(k)))?,
            }
        }
        Some(out)
    }
}

/// A stage as the mappers give it: a number from -6 to 6, or the same as a
/// string. Anything else (no battle: a byte the glossary has no value for)
/// is no stage.
fn stage_value(v: &Value) -> Option<i64> {
    let n = match v {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }?;
    (-6..=6).contains(&n).then_some(n)
}

fn stat_index(stat: &str) -> Option<usize> {
    STATS.iter().position(|s| *s == stat)
}

/// One of the solo mon's moves that a stat-stage stepper can count (as the
/// battle page offers one): a stat move, or a damaging move with a stat
/// side effect.
#[derive(Clone, Debug)]
struct SetupMove {
    name: String,
    targets_self: bool,
    is_guaranteed: bool,
    is_belly_drum: bool,
    /// (stat index, stages)
    mods: Vec<(usize, i64)>,
}

impl SetupMove {
    fn from_db(move_db: &MoveDB, name: &str) -> Option<SetupMove> {
        let info = move_db.get_stat_stage_info(name);
        if !info.has_stat_effect || !(info.is_guaranteed || info.is_damaging) {
            return None;
        }
        let mods: Vec<(usize, i64)> = move_db
            .get_stat_mod_for_target(name, info.targets_self)
            .into_iter()
            .filter_map(|(stat, d)| stat_index(&stat).map(|i| (i, d)))
            .filter(|(_, d)| *d != 0)
            .collect();
        if mods.is_empty() {
            return None;
        }
        Some(SetupMove {
            name: name.to_string(),
            targets_self: info.targets_self,
            is_guaranteed: info.is_guaranteed,
            is_belly_drum: info.is_belly_drum,
            mods,
        })
    }

    /// The stage changes one use makes from `player` / `enemy` (each stage
    /// moves on its own and stops at -6 / 6).
    fn deltas(&self, player: &Stages, enemy: &Stages) -> (Stages, Stages) {
        let mut dp = [0; 7];
        let mut de = [0; 7];
        let (base, out) = if self.targets_self { (player, &mut dp) } else { (enemy, &mut de) };
        for (i, d) in &self.mods {
            out[*i] = (base[*i] + d).clamp(-6, 6) - base[*i];
        }
        (dp, de)
    }

    /// Whether the move can move this stage of this side in this direction.
    fn can_change(&self, on_player: bool, stat: usize, sign: i64) -> bool {
        self.targets_self == on_player && self.mods.iter().any(|(i, d)| *i == stat && d.signum() == sign)
    }
}

/// The moves a stepper can count, guaranteed ones first: when Howl and a
/// Meteor Mash that boosted would both explain +1 Attack, it was Howl.
fn setup_moves(move_db: &MoveDB, moves: &[Option<String>]) -> Vec<SetupMove> {
    let mut out: Vec<SetupMove> = Vec::new();
    for name in moves.iter().flatten() {
        if name.is_empty() || name == consts::MIMIC_MOVE_NAME || out.iter().any(|m| &m.name == name) {
            continue;
        }
        if let Some(m) = SetupMove::from_db(move_db, name) {
            out.push(m);
        }
    }
    out.sort_by_key(|m| !m.is_guaranteed);
    out
}

#[derive(Clone, Copy, Debug)]
struct Snapshot {
    player: Stages,
    enemy: Stages,
    slot: Option<usize>,
}

/// Stage changes a move of the player's could have made, waiting for the rest
/// of that move's changes.
#[derive(Clone, Debug)]
struct Pending {
    slot: usize,
    start_player: Stages,
    start_enemy: Stages,
    dp: Stages,
    de: Stages,
    since: Instant,
}

enum Fit {
    /// a move makes exactly these changes; `more` when another move would
    /// make these and more (so they may not be finished yet)
    Exact { idx: usize, more: bool },
    /// part of what a move makes
    Partial,
    None,
}

fn fit(moves: &[SetupMove], p: &Pending) -> Fit {
    let mut exact = None;
    let mut more = false;
    for (idx, m) in moves.iter().enumerate() {
        let (mp, me) = m.deltas(&p.start_player, &p.start_enemy);
        if mp == p.dp && me == p.de {
            exact.get_or_insert(idx);
            continue;
        }
        let covers = |mine: &Stages, seen: &Stages| (0..7).all(|i| seen[i] == 0 || seen[i] == mine[i]);
        if covers(&mp, &p.dp) && covers(&me, &p.de) {
            more = true;
        }
    }
    match exact {
        Some(idx) => Fit::Exact { idx, more },
        None if more => Fit::Partial,
        None => Fit::None,
    }
}

/// Follows the solo mon's stat-stage setup through one battle.
#[derive(Clone, Debug, Default)]
pub struct StageSetupTracker {
    keys: Option<StageKeys>,
    prev: Option<Snapshot>,
    pending: Option<Pending>,
    settle_until: Option<Instant>,
    /// enemy party slot -> move -> uses (Belly Drum: the Attack stage it left)
    counts: BTreeMap<usize, IndexMap<String, i64>>,
}

impl StageSetupTracker {
    /// The mapper was (re)loaded.
    pub fn configure(&mut self, store: &PropertyStore) {
        let keys = StageKeys::resolve(store);
        if !keys.is_usable() {
            log::info!("[stat stages] the mapper doesn't show the battle stat stages: setup is not recorded");
        }
        self.keys = Some(keys).filter(|k| k.is_usable());
        self.reset();
    }

    /// Read the stages from other paths from now on (the stages seen so far
    /// stay the ones to compare with).
    pub fn set_keys(&mut self, keys: StageKeys) {
        self.keys = Some(keys).filter(|k| k.is_usable());
    }

    /// A new battle.
    pub fn reset(&mut self) {
        self.prev = None;
        self.pending = None;
        self.settle_until = None;
        self.counts.clear();
    }

    /// Look at the stages after a batch of changes. `slot` is the enemy's
    /// party slot (in the trainer event's definition order), `solo_active`
    /// whether the solo mon is the one on the field, `moves` its moves (route
    /// names).
    pub fn observe(&mut self, gen: &GenData, store: &PropertyStore, slot: Option<usize>, solo_active: bool, moves: &[Option<String>]) {
        let Some(keys) = &self.keys else { return };
        let (Some(player), Some(enemy)) = (StageKeys::read_side(store, &keys.player), StageKeys::read_side(store, &keys.enemy)) else {
            return;
        };
        let now = crate::clock::now();
        let cur = Snapshot { player, enemy, slot };
        let Some(prev) = self.prev.replace(cur) else {
            self.settle_until = Some(now + SEND_OUT_SETTLE);
            return;
        };
        let setup = setup_moves(gen.move_db(), moves);
        // a move's changes are all in, or the matchup it belonged to is over
        if let Some(p) = &self.pending {
            if now.duration_since(p.since) >= MOVE_WINDOW || Some(p.slot) != slot {
                self.settle_pending(&setup);
            }
        }
        if prev.slot != slot {
            self.settle_until = Some(now + SEND_OUT_SETTLE);
        }
        let (Some(slot), true) = (slot, solo_active) else {
            self.pending = None;
            return;
        };
        if setup.is_empty() {
            return;
        }
        let enemy_settled = prev.slot == Some(slot) && self.settle_until.is_none_or(|t| now >= t);
        let mut dp = [0; 7];
        let mut de = [0; 7];
        for i in 0..7 {
            let d = cur.player[i] - prev.player[i];
            if d != 0 && setup.iter().any(|m| m.can_change(true, i, d.signum())) {
                dp[i] = d;
            }
            let d = cur.enemy[i] - prev.enemy[i];
            if enemy_settled && d != 0 && setup.iter().any(|m| m.can_change(false, i, d.signum())) {
                de[i] = d;
            }
        }
        if dp == [0; 7] && de == [0; 7] {
            return;
        }
        // with what came before (a Dragon Dance's Attack, now its Speed)
        if let Some(p) = &self.pending {
            let mut joined = p.clone();
            for i in 0..7 {
                joined.dp[i] += dp[i];
                joined.de[i] += de[i];
            }
            if !matches!(fit(&setup, &joined), Fit::None) {
                self.pending = Some(joined);
                self.try_settle(&setup);
                return;
            }
            self.settle_pending(&setup);
        }
        self.pending = Some(Pending { slot, start_player: prev.player, start_enemy: prev.enemy, dp, de, since: now });
        self.try_settle(&setup);
    }

    /// Count the pending changes if they are a whole move that can't be part
    /// of a bigger one.
    fn try_settle(&mut self, setup: &[SetupMove]) {
        let Some(p) = &self.pending else { return };
        match fit(setup, p) {
            Fit::Exact { idx, more: false } => {
                let p = self.pending.take().unwrap();
                self.count(&setup[idx], &p);
            }
            Fit::Exact { .. } | Fit::Partial => {}
            Fit::None => {
                log::info!("[stat stages] changes no move of the solo mon's explains: {:?} / {:?}", p.dp, p.de);
                self.pending = None;
            }
        }
    }

    /// Count the pending changes if they are a whole move, and drop them.
    fn settle_pending(&mut self, setup: &[SetupMove]) {
        let Some(p) = self.pending.take() else { return };
        if let Fit::Exact { idx, .. } = fit(setup, &p) {
            self.count(&setup[idx], &p);
        } else {
            log::info!("[stat stages] changes no move of the solo mon's explains: {:?} / {:?}", p.dp, p.de);
        }
    }

    fn count(&mut self, m: &SetupMove, p: &Pending) {
        let entry = self.counts.entry(p.slot).or_default();
        if m.is_belly_drum {
            // its stepper holds the Attack stage it ends at
            entry.insert(m.name.clone(), p.start_player[0] + p.dp[0]);
        } else {
            *entry.entry(m.name.clone()).or_insert(0) += 1;
        }
        log::info!("[stat stages] {} vs enemy slot {}: {}", m.name, p.slot + 1, entry[&m.name]);
    }

    /// The battle is over: the setup, by enemy party slot (empty when there
    /// was none), for [`remap_stat_stage_setup`] once the mon order is known.
    pub fn finish(&mut self, gen: &GenData, moves: &[Option<String>]) -> Vec<CustomMoveData> {
        let setup = setup_moves(gen.move_db(), moves);
        self.settle_pending(&setup);
        let counts = std::mem::take(&mut self.counts);
        self.reset();
        let Some(last) = counts.keys().next_back().copied() else { return Vec::new() };
        let mut out = vec![CustomMoveData::default(); last + 1];
        for (slot, moves) in counts {
            for (name, n) in moves {
                if n != 0 {
                    out[slot].player.insert(name, n.to_string());
                }
            }
        }
        if out.iter().all(|c| c.is_empty()) {
            return Vec::new();
        }
        out
    }
}

/// `stat_stage_setup` as the recorder leaves it is by enemy party slot (the
/// trainer data's order); the event keeps it in the battle page's order, which
/// depends on the final `mon_order`. Call once that is set.
///
/// The battle page puts entry `mon_order[i] - 1` on the `i`-th matchup (the
/// `i`-th mon fought), where `mon_order` is read in party order: so the setup
/// of the slot fought `i`-th goes there.
pub fn remap_stat_stage_setup(gen: &GenData, ev: &mut EventDefinition) {
    let by_slot = match ev.trainer_def.as_ref() {
        Some(td) if !td.stat_stage_setup.is_empty() => td.stat_stage_setup.clone(),
        _ => return,
    };
    let (Ok(fought), Ok(party)) = (ev.get_pokemon_list(gen, false), ev.get_pokemon_list(gen, true)) else {
        return;
    };
    let mut out = vec![CustomMoveData::default(); party.len()];
    for (i, (slot, _)) in fought.iter().enumerate() {
        let Some(target) = party.get(i).and_then(|(_, m)| usize::try_from(m.mon_order - 1).ok()) else { continue };
        if let (Some(dest), Some(data)) = (out.get_mut(target), usize::try_from(*slot).ok().and_then(|s| by_slot.get(s))) {
            *dest = data.clone();
        }
    }
    if let Some(td) = ev.trainer_def.as_mut() {
        td.stat_stage_setup = if out.iter().all(|c| c.is_empty()) { Vec::new() } else { out };
    }
}

/// A party position as the mappers give it (0-5), as a slot.
pub fn party_slot(v: Value) -> Option<usize> {
    v.as_i64().and_then(|p| usize::try_from(p).ok()).filter(|p| *p < 6)
}
