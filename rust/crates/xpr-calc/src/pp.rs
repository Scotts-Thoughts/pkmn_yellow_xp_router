//! PP spent per fight: for each enemy mon a fight KOs, which of the solo
//! mon's move slots pay PP and how much
//! (docs/rust_port/design/pp_tracking/PLAN.md §1.2-1.4, §4).
//!
//! The spend is a pure function of the fight's saved definition, the solo
//! mon at each matchup and the calc config, so it is cached per fight
//! ([`FightSpendCache`]) and computed on rayon threads.

use std::collections::HashMap;
use std::sync::Arc;

use rayon::prelude::*;

use xpr_core::consts;
use xpr_core::CalcConfig;
use xpr_data::model::{EnemyPkmn, Gen};
use xpr_data::{GenData, PpLock};
use xpr_engine::{EventDefinition, NodeId, RouteState, Router};

use crate::battle_summary::{BattleSummary, MoveRenderInfo, SummaryConfig};

/// Everything a fight's spend depends on besides the config. Owned and
/// `Send`, so fights can be computed on other threads.
#[derive(Clone, Debug)]
pub struct FightInput {
    pub group_id: NodeId,
    pub def: EventDefinition,
    pub is_wild: bool,
    /// One per fight item, in battle order: the mon KO'd and the state the
    /// item starts from.
    pub matchups: Vec<(EnemyPkmn, Arc<RouteState>)>,
}

impl FightInput {
    /// The fight of a trainer or wild group; `None` for any other group, or
    /// a fight without fight items (disabled).
    pub fn from_router(router: &Router, group_id: NodeId) -> Option<FightInput> {
        let group = router.group(group_id)?;
        let def = &group.event_definition;
        let is_wild = def.wild_pkmn_info.is_some();
        if !is_wild && def.trainer_def.is_none() {
            return None;
        }
        let mut matchups = Vec::new();
        for id in &group.event_items {
            let item = router.item(*id)?;
            if let (Some(mon), Some(init)) = (&item.to_defeat_mon, &item.init_state) {
                matchups.push((mon.clone(), init.clone()));
            }
        }
        if matchups.is_empty() {
            return None;
        }
        Some(FightInput { group_id, def: def.clone(), is_wild, matchups })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpendKind {
    /// A stat-stage setup move entered on the battle page.
    Setup,
    /// The best move's guaranteed KO.
    Ko,
    /// The Mimic slot: gen 1 pays for the mimicked move's uses too.
    Mimic,
    /// Transform, once per fight (the copied moves have their own PP).
    Transform,
}

/// PP one move slot pays in a matchup.
#[derive(Clone, Debug, PartialEq)]
pub struct SlotSpend {
    pub slot: usize,
    /// The slot's move.
    pub move_name: String,
    /// PP spent, after the lock rule and Pressure.
    pub pp: i64,
    /// Turns (or setup uses) before the lock rule and Pressure.
    pub turns: i64,
    pub kind: SpendKind,
    /// The enemy has Pressure and doubled this spend.
    pub pressure: bool,
}

/// PP spent against one enemy mon.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MatchupSpend {
    /// The enemy mon's name.
    pub enemy: String,
    /// Setup first, then the KO move.
    pub spends: Vec<SlotSpend>,
    /// Why nothing (or only setup) was spent, e.g. `no damaging move`.
    pub note: Option<String>,
}

impl MatchupSpend {
    /// PP spent per move slot.
    pub fn per_slot(&self) -> [i64; 4] {
        let mut out = [0i64; 4];
        for s in &self.spends {
            if let Some(v) = out.get_mut(s.slot) {
                *v += s.pp;
            }
        }
        out
    }
}

/// The spend config: the calc settings and the player highlight strategy.
/// Test moves never count, so they are switched off.
fn spend_config(cfg: &SummaryConfig) -> SummaryConfig {
    SummaryConfig { test_moves_enabled: false, test_moves: vec![String::new(); 4], ..cfg.clone() }
}

/// Whether the page highlights no best move, so PP picks its own (§1.2).
fn uses_own_rule(strategy: &str) -> bool {
    strategy == consts::HIGHLIGHT_NONE || !consts::ALL_HIGHLIGHT_STRATS.contains(&strategy)
}

/// The best of the four move slots for PP when the page highlights none:
/// fewest guaranteed turns, then the higher max damage.
fn own_best(moves: &[Option<MoveRenderInfo>]) -> Option<usize> {
    let mut best: Option<(usize, i64, i64)> = None;
    for (idx, m) in moves.iter().enumerate().take(4) {
        let Some(m) = m else { continue };
        if m.name == consts::STRUGGLE_MOVE_NAME || m.min_damage == -1 {
            continue;
        }
        let Some(turns) = m.pp_turns else { continue };
        let better = match best {
            None => true,
            Some((_, t, dmg)) => turns < t || (turns == t && m.max_damage > dmg),
        };
        if better {
            best = Some((idx, turns, m.max_damage));
        }
    }
    best.map(|(idx, _, _)| idx)
}

/// PP spent against each enemy mon of a fight, in battle order (one entry
/// per `input.matchups` entry that the summary could pair up).
pub fn fight_pp_spend(gen: &GenData, input: &FightInput, cfg: &SummaryConfig) -> Vec<MatchupSpend> {
    let cfg = spend_config(cfg);
    let mut summary = BattleSummary::new();
    let loaded = if input.is_wild {
        summary.load_wild_from_parts(gen, &input.matchups, &cfg)
    } else {
        summary.load_from_parts(gen, input.group_id, &input.def, &input.matchups, &cfg)
    };
    if let Err(e) = loaded {
        log::warn!("PP: could not load fight {}: {}", input.group_id, e);
        return input
            .matchups
            .iter()
            .map(|(mon, _)| MatchupSpend { enemy: mon.name.clone(), spends: Vec::new(), note: Some(format!("could not work out this fight: {}", e)) })
            .collect();
    }
    let own_rule = uses_own_rule(&cfg.player_strategy);
    let pressure_gen = gen.has_pressure_pp_cost();
    let mut rage_paid = false;
    let mut mimic_paid = false;
    let mut transform_paid = false;
    let mut out = Vec::with_capacity(summary.num_matchups());
    for mon_idx in 0..summary.num_matchups() {
        let enemy = &summary.original_enemy_mon_list[mon_idx];
        let player = &summary.original_player_mon_list[mon_idx];
        // a wild mon's ability is unknown: assume the worst when its species
        // can have Pressure, as the damage assumes its worst DVs
        let pressure = pressure_gen
            && if enemy.ability.is_empty() && summary.is_wild_battle {
                gen.pkmn_db().get_pkmn(&enemy.name).map(|s| s.abilities.iter().any(|a| a == consts::PRESSURE_ABILITY)).unwrap_or(false)
            } else {
                enemy.ability == consts::PRESSURE_ABILITY
            };
        let doubled = |targets_enemy: bool| pressure && targets_enemy;
        let mut spend = MatchupSpend { enemy: enemy.name.clone(), ..Default::default() };

        for s in summary.player_setup_uses(gen, mon_idx) {
            let d = doubled(s.targets_enemy);
            spend.spends.push(SlotSpend { slot: s.slot, move_name: s.move_name, pp: s.uses * if d { 2 } else { 1 }, turns: s.uses, kind: SpendKind::Setup, pressure: d });
        }

        if summary.is_player_transformed {
            // the copied moves have their own PP and are gone after the fight
            // (gen 1 `DecrementPP` skips the party PP while TRANSFORMED)
            if !transform_paid {
                if let Some(slot) = player.move_list.iter().position(|m| m.as_deref() == Some(consts::TRANSFORM_MOVE_NAME)) {
                    transform_paid = true;
                    let d = doubled(true);
                    spend.spends.push(SlotSpend {
                        slot,
                        move_name: consts::TRANSFORM_MOVE_NAME.to_string(),
                        pp: if d { 2 } else { 1 },
                        turns: 1,
                        kind: SpendKind::Transform,
                        pressure: d,
                    });
                }
            }
            spend.note = Some("transformed: the copied moves have their own PP".to_string());
            out.push(spend);
            continue;
        }

        let moves = &summary.player_move_data[mon_idx];
        let best_idx = if own_rule {
            own_best(moves)
        } else {
            moves.iter().take(4).position(|m| m.as_ref().map(|m| m.is_best_move).unwrap_or(false))
        };
        let Some(best_idx) = best_idx else {
            spend.note = Some("no damaging move".to_string());
            out.push(spend);
            continue;
        };
        let best = moves[best_idx].as_ref().expect("best move exists");
        let Some(turns) = best.pp_turns else {
            spend.note = Some(format!("{} does no damage", best.name));
            out.push(spend);
            continue;
        };
        let slot_move = player.move_list.get(best_idx).cloned().flatten().unwrap_or_default();
        let is_mimic_slot = slot_move == consts::MIMIC_MOVE_NAME && !summary.mimic_selection.is_empty();
        // the move that actually runs decides whether it locks
        let executed = if is_mimic_slot {
            summary.mimic_selection.clone()
        } else {
            match &best.metronome_selection {
                Some(called) if !called.is_empty() => called.clone(),
                _ => slot_move.clone(),
            }
        };
        let ko_pp = match gen.pp_lock(&executed) {
            PpLock::PerTurn => turns,
            PpLock::Lock(k) => (turns + k - 1) / k,
            PpLock::OncePerFight => {
                if rage_paid {
                    0
                } else {
                    rage_paid = true;
                    1
                }
            }
        };
        let d = doubled(true);
        let factor = if d { 2 } else { 1 };
        if is_mimic_slot {
            // gen 1: the mimicked move sits in Mimic's slot and uses its PP;
            // gens 2+: it gets 5 PP of its own and only Mimic's use costs
            let first = !mimic_paid;
            mimic_paid = true;
            let pp = if gen.gen == Gen::One { ko_pp + i64::from(first) } else { i64::from(first) };
            if pp > 0 {
                spend.spends.push(SlotSpend { slot: best_idx, move_name: slot_move, pp: pp * factor, turns, kind: SpendKind::Mimic, pressure: d });
            }
        } else {
            spend.spends.push(SlotSpend { slot: best_idx, move_name: slot_move, pp: ko_pp * factor, turns, kind: SpendKind::Ko, pressure: d });
        }
        out.push(spend);
    }
    out
}

// ---------------------------------------------------------------------------
// Cache
// ---------------------------------------------------------------------------

/// What a cached spend was computed from. The enemies, the double-battle
/// flag and `is_wild` all follow from `def` and the version; the battle
/// page's vitamin "shadows" are ordinary vitamin events, so they reach the
/// player states like any other event.
#[derive(Clone, Debug)]
struct FightKey {
    def: EventDefinition,
    players: Vec<EnemyPkmn>,
    calc: CalcConfig,
    strategy: String,
    version: String,
}

impl FightKey {
    fn new(gen: &GenData, input: &FightInput, cfg: &SummaryConfig) -> FightKey {
        FightKey {
            def: input.def.clone(),
            players: input.matchups.iter().map(|(_, st)| st.solo_pkmn.get_pkmn_obj(&st.badges, None)).collect(),
            calc: cfg.calc,
            strategy: cfg.player_strategy.clone(),
            version: gen.version_name().to_string(),
        }
    }

    fn same(&self, other: &FightKey) -> bool {
        self.def == other.def
            && self.calc == other.calc
            && self.strategy == other.strategy
            && self.version == other.version
            && self.players.len() == other.players.len()
            && self.players.iter().zip(other.players.iter()).all(|(a, b)| a.py_eq(b))
    }
}

/// Fight spends by group id, reused while a fight's inputs stay the same.
/// Item ids cannot be keys: every recalc mints new ones.
#[derive(Default)]
pub struct FightSpendCache {
    entries: HashMap<NodeId, (FightKey, Arc<Vec<MatchupSpend>>)>,
    /// Fights computed by the last [`FightSpendCache::resolve`] (tests and
    /// the benchmark read it).
    pub last_computed: usize,
}

impl FightSpendCache {
    pub fn new() -> FightSpendCache {
        FightSpendCache::default()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// The spend of every fight in `inputs`: cached ones as they are, the
    /// rest computed in parallel. Fights no longer in `inputs` are dropped.
    pub fn resolve(&mut self, gen: &GenData, inputs: Vec<FightInput>, cfg: &SummaryConfig) -> HashMap<NodeId, Arc<Vec<MatchupSpend>>> {
        let keyed: Vec<(FightInput, FightKey)> = inputs
            .into_iter()
            .map(|i| {
                let k = FightKey::new(gen, &i, cfg);
                (i, k)
            })
            .collect();
        let mut result: HashMap<NodeId, Arc<Vec<MatchupSpend>>> = HashMap::with_capacity(keyed.len());
        let mut misses: Vec<(FightInput, FightKey)> = Vec::new();
        for (input, key) in keyed {
            match self.entries.get(&input.group_id) {
                Some((old_key, spends)) if old_key.same(&key) => {
                    result.insert(input.group_id, spends.clone());
                }
                _ => misses.push((input, key)),
            }
        }
        let computed: Vec<(NodeId, FightKey, Arc<Vec<MatchupSpend>>)> = misses
            .into_par_iter()
            .map(|(input, key)| {
                let spends = Arc::new(fight_pp_spend(gen, &input, cfg));
                (input.group_id, key, spends)
            })
            .collect();
        self.last_computed = computed.len();
        let mut entries: HashMap<NodeId, (FightKey, Arc<Vec<MatchupSpend>>)> = HashMap::with_capacity(result.len() + computed.len());
        for (id, spends) in &result {
            if let Some((key, _)) = self.entries.remove(id) {
                entries.insert(*id, (key, spends.clone()));
            }
        }
        for (id, key, spends) in computed {
            result.insert(id, spends.clone());
            entries.insert(id, (key, spends));
        }
        self.entries = entries;
        result
    }
}
