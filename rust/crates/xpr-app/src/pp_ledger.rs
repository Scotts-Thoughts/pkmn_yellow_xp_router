//! The PP ledger (docs/rust_port/design/pp_tracking/PLAN.md §5.1): current
//! PP of the solo mon's four moves before every event.
//!
//! PP never feeds back into the route (it may go negative, and the best move
//! is chosen without looking at it), so it is derived after recalculation:
//! the ledger walks the route in order, applying fight spends
//! (`xpr_calc::pp`), refills, PP items and the learn-move rules. It rebuilds
//! only when asked while the route revision or the calc settings changed,
//! and the fight spends, the expensive part, are cached per fight.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use xpr_calc::battle_summary::SummaryConfig;
use xpr_calc::pp::{FightInput, FightSpendCache, MatchupSpend, SlotSpend, SpendKind};
use xpr_core::{consts, CalcConfig, Config};
use xpr_data::{GenData, PpAmount, PpItemEffect};
use xpr_engine::{InventoryEventDefinition, Node, NodeId, RouteState, Router};

/// One move slot's PP at a point of the route.
#[derive(Clone, Debug, PartialEq)]
pub struct SlotPp {
    pub move_name: String,
    /// Current PP; negative when the route overdraws the move.
    pub cur: i64,
    pub max: i64,
    pub pp_ups: u8,
}

/// The four slots' PP before an event.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PpSnapshot {
    pub slots: [Option<SlotPp>; 4],
}

/// One line of a slot's history since it was last full: a spend or restore
/// (`amount`), or the refill / learn that started the history (`None`).
#[derive(Clone, Debug, PartialEq)]
pub struct PpChange {
    pub amount: Option<i64>,
    pub text: String,
}

/// The `SummaryConfig` PP uses: the calc settings and the player highlight
/// strategy (test moves never count).
pub fn pp_summary_config(cfg: &Config) -> SummaryConfig {
    SummaryConfig {
        calc: cfg.calc_config(),
        player_strategy: cfg.get_player_highlight_strategy().to_string(),
        enemy_strategy: cfg.get_enemy_highlight_strategy().to_string(),
        test_moves_enabled: false,
        test_moves: vec![String::new(); 4],
    }
}

#[derive(Clone, Debug, PartialEq)]
struct LedgerKey {
    revision: u64,
    calc: CalcConfig,
    strategy: String,
    version: String,
}

#[derive(Default)]
pub struct PpLedger {
    key: Option<LedgerKey>,
    before: HashMap<NodeId, PpSnapshot>,
    /// Per id and slot: the `logs[slot]` range that is the slot's history
    /// before that event.
    windows: HashMap<NodeId, [(u32, u32); 4]>,
    logs: [Vec<PpChange>; 4],
    /// PP a fight group (all its matchups) or fight item (one matchup)
    /// spends, per slot.
    spend: HashMap<NodeId, [i64; 4]>,
    notes: HashMap<NodeId, Vec<String>>,
    cache: FightSpendCache,
    /// Fights computed by the last rebuild (the rest came from the cache).
    pub last_computed: usize,
    /// How long the last rebuild took.
    pub last_rebuild: Option<Duration>,
}

impl PpLedger {
    pub fn new() -> PpLedger {
        PpLedger::default()
    }

    /// Forget everything (route closed or replaced).
    pub fn clear(&mut self) {
        self.key = None;
        self.before.clear();
        self.windows.clear();
        self.logs = Default::default();
        self.spend.clear();
        self.notes.clear();
        self.cache.clear();
    }

    /// Bring the ledger up to date with `router`; free when nothing changed
    /// since the last call. Returns whether it rebuilt.
    pub fn ensure(&mut self, router: &Router, revision: u64, cfg: &SummaryConfig) -> bool {
        let (Some(gen), Some(init)) = (router.gen().cloned(), router.init_route_state.clone()) else {
            self.clear();
            return false;
        };
        let key = LedgerKey { revision, calc: cfg.calc, strategy: cfg.player_strategy.clone(), version: gen.version_name().to_string() };
        if self.key.as_ref() == Some(&key) {
            return false;
        }
        if self.key.as_ref().map(|k| k.version != key.version).unwrap_or(false) {
            self.cache.clear();
        }
        let start = Instant::now();
        let inputs: Vec<FightInput> = router.all_groups().into_iter().filter_map(|g| FightInput::from_router(router, g)).collect();
        let spends = self.cache.resolve(&gen, inputs, cfg);
        self.last_computed = self.cache.last_computed;

        self.before.clear();
        self.windows.clear();
        self.logs = Default::default();
        self.spend.clear();
        self.notes.clear();
        let mut walk = Walk { gen: &gen, router, spends: &spends, slots: Default::default(), ledger: self };
        walk.start(&init);
        walk.folder(router.root_id);
        self.key = Some(key);
        self.last_rebuild = Some(start.elapsed());
        true
    }

    /// The PP before an event, group item or folder.
    pub fn before(&self, id: NodeId) -> Option<&PpSnapshot> {
        self.before.get(&id)
    }

    /// A slot's history before `id`: the refill or learn that last filled it,
    /// then every change since.
    pub fn history(&self, id: NodeId, slot: usize) -> &[PpChange] {
        match (self.windows.get(&id), self.logs.get(slot)) {
            (Some(w), Some(log)) => {
                let (a, b) = w[slot];
                &log[a as usize..b as usize]
            }
            _ => &[],
        }
    }

    /// PP a fight group (all matchups) or fight item (its matchup) spends.
    pub fn spend(&self, id: NodeId) -> Option<[i64; 4]> {
        self.spend.get(&id).copied()
    }

    /// Notes on an event: a fight that spends nothing, an Ether the game
    /// would refuse, ...
    pub fn notes(&self, id: NodeId) -> &[String] {
        self.notes.get(&id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// The Pre-Event State banner lines for `id` (§5.3).
    pub fn messages(&self, router: &Router, id: NodeId) -> Vec<String> {
        let mut out = Vec::new();
        let Some(snap) = self.before(id) else { return out };
        for s in snap.slots.iter().flatten() {
            if s.cur <= 0 {
                out.push(format!("{} is at {} PP before this event: heal first", s.move_name, fmt_pp(s.cur)));
            }
        }
        if let Some(spend) = self.spend(id) {
            let what = if router.item(id).is_some() { "matchup" } else { "fight" };
            for (i, s) in snap.slots.iter().enumerate() {
                let (Some(s), Some(n)) = (s, spend.get(i)) else { continue };
                if *n > 0 && s.cur > 0 && *n > s.cur {
                    out.push(format!("This {} needs {} ×{}, but it has {} PP left", what, s.move_name, n, s.cur));
                }
            }
        }
        out.extend(self.notes(id).iter().cloned());
        out
    }
}

/// `-3` with a real minus sign.
pub fn fmt_pp(n: i64) -> String {
    if n < 0 {
        format!("\u{2212}{}", -n)
    } else {
        n.to_string()
    }
}

struct Slot {
    move_name: String,
    cur: i64,
    log_start: usize,
}

struct Walk<'a> {
    gen: &'a GenData,
    router: &'a Router,
    spends: &'a HashMap<NodeId, Arc<Vec<MatchupSpend>>>,
    slots: [Option<Slot>; 4],
    ledger: &'a mut PpLedger,
}

impl<'a> Walk<'a> {
    fn max_pp(&self, st: &RouteState, slot: usize) -> i64 {
        let mon = &st.solo_pkmn;
        match mon.move_list.get(slot).cloned().flatten() {
            Some(m) => self.gen.max_pp(&m, mon.pp_ups.get(slot).copied().unwrap_or(0)).unwrap_or(0),
            None => 0,
        }
    }

    /// Start a new history for `slot` with `text` (a refill or a learn).
    fn restart_log(&mut self, slot: usize, text: String) -> usize {
        let log = &mut self.ledger.logs[slot];
        log.push(PpChange { amount: None, text });
        log.len() - 1
    }

    fn log(&mut self, slot: usize, amount: i64, text: String) {
        self.ledger.logs[slot].push(PpChange { amount: Some(amount), text });
    }

    /// The route's starting moves, all full.
    fn start(&mut self, init: &RouteState) {
        for slot in 0..4 {
            self.slots[slot] = match init.solo_pkmn.move_list.get(slot).cloned().flatten() {
                Some(m) => {
                    let log_start = self.restart_log(slot, "Full at the start of the route".to_string());
                    Some(Slot { move_name: m, cur: self.max_pp(init, slot), log_start })
                }
                None => None,
            };
        }
    }

    fn record(&mut self, id: NodeId, st: &RouteState) {
        let mut snap = PpSnapshot::default();
        let mut window = [(0u32, 0u32); 4];
        for slot in 0..4 {
            if let Some(s) = &self.slots[slot] {
                snap.slots[slot] = Some(SlotPp {
                    move_name: s.move_name.clone(),
                    cur: s.cur,
                    max: self.max_pp(st, slot),
                    pp_ups: st.solo_pkmn.pp_ups.get(slot).copied().unwrap_or(0),
                });
                window[slot] = (s.log_start as u32, self.ledger.logs[slot].len() as u32);
            }
        }
        self.ledger.before.insert(id, snap);
        self.ledger.windows.insert(id, window);
    }

    /// Match the slots to `st`'s move list. A slot whose move changed took
    /// a new move: full PP, except a gen 5 TM/HM over a move, which keeps the
    /// old move's PP up to the new max (`learn_source` is the learn event's
    /// source; `None` for a change no event explains).
    fn reconcile(&mut self, st: &RouteState, learn_source: Option<&str>) {
        for slot in 0..4 {
            let want = st.solo_pkmn.move_list.get(slot).cloned().flatten();
            let same = matches!((&self.slots[slot], &want), (Some(s), Some(m)) if s.move_name == *m);
            if same {
                continue;
            }
            let Some(m) = want else {
                self.slots[slot] = None;
                continue;
            };
            let max = self.max_pp(st, slot);
            let is_machine = learn_source.map(|src| src != consts::MOVE_SOURCE_LEVELUP && src != consts::MOVE_SOURCE_TUTOR).unwrap_or(false);
            let old = self.slots[slot].take();
            let (cur, text) = match (&old, is_machine && self.gen.get_generation() == 5) {
                (Some(o), true) => {
                    let kept = o.cur.min(max);
                    (kept, format!("Learned {} over {} (kept {} PP)", m, o.move_name, fmt_pp(kept)))
                }
                _ => (max, format!("Learned {}", m)),
            };
            if learn_source.is_none() {
                log::debug!("PP ledger: {} appeared in slot {} without a learn event", m, slot + 1);
            }
            let log_start = self.restart_log(slot, text);
            self.slots[slot] = Some(Slot { move_name: m, cur, log_start });
        }
    }

    fn refill(&mut self, st: &RouteState, label: &str) {
        for slot in 0..4 {
            if self.slots[slot].is_some() {
                let max = self.max_pp(st, slot);
                let log_start = self.restart_log(slot, format!("Full at {}", label));
                if let Some(s) = self.slots[slot].as_mut() {
                    s.cur = max;
                    s.log_start = log_start;
                }
            }
        }
    }

    fn folder(&mut self, id: NodeId) {
        let router = self.router;
        let Some(Node::Folder(f)) = router.node(id) else { return };
        if let Some(st) = f.init_state.clone() {
            self.reconcile(&st, None);
            self.record(id, &st);
        }
        for child in f.children.clone() {
            match router.node(child) {
                Some(Node::Folder(_)) => self.folder(child),
                Some(Node::Group(_)) => self.group(child),
                None => {}
            }
        }
    }

    fn group(&mut self, gid: NodeId) {
        let router = self.router;
        let Some(g) = router.group(gid) else { return };
        if let Some(st) = g.init_state.clone() {
            self.reconcile(&st, None);
            self.record(gid, &st);
        }
        let spends = self.spends.get(&gid).cloned();
        let mut fight_idx = 0usize;
        let mut group_spend: Option<[i64; 4]> = None;
        for item_id in g.event_items.clone() {
            let Some(item) = router.item(item_id) else { continue };
            let Some(init) = item.init_state.clone() else { continue };
            self.reconcile(&init, None);
            self.record(item_id, &init);
            if !item.enabled.unwrap_or(false) {
                continue;
            }
            let def = &item.event_definition;
            if item.to_defeat_mon.is_some() {
                let matchup = spends.as_ref().and_then(|s| s.get(fight_idx)).cloned();
                fight_idx += 1;
                let total = group_spend.get_or_insert([0; 4]);
                match matchup {
                    Some(m) => {
                        let per_slot = m.per_slot();
                        for (t, p) in total.iter_mut().zip(per_slot.iter()) {
                            *t += *p;
                        }
                        self.ledger.spend.insert(item_id, per_slot);
                        for s in &m.spends {
                            self.pay(s, &item.name);
                        }
                        if let Some(note) = &m.note {
                            self.ledger.notes.entry(item_id).or_default().push(format!("PP: {} ({})", note, m.enemy));
                        }
                    }
                    None => {
                        self.ledger.notes.entry(item_id).or_default().push("PP: this matchup's spend could not be worked out".to_string());
                    }
                }
            } else if def.heal.is_some() || def.blackout.is_some() {
                let label = item.name.clone();
                self.refill(&init, &label);
            } else if let Some(i) = &def.item_event_def {
                if i.is_use() && !i.no_effect {
                    self.use_item(item_id, i, &init);
                }
            }
            if let Some(fin) = item.final_state.clone() {
                // PP Up / PP Max: the current PP rises with the max
                for slot in 0..4 {
                    let before = init.solo_pkmn.pp_ups.get(slot).copied().unwrap_or(0);
                    let after = fin.solo_pkmn.pp_ups.get(slot).copied().unwrap_or(0);
                    let same_move = init.solo_pkmn.move_list.get(slot) == fin.solo_pkmn.move_list.get(slot);
                    if after > before && same_move {
                        let delta = self.max_pp(&fin, slot) - self.max_pp(&init, slot);
                        if let Some(s) = self.slots[slot].as_mut() {
                            s.cur += delta;
                        }
                        self.log(slot, delta, format!("{} (max +{})", item.name, delta));
                    }
                }
                let source = def.learn_move.as_ref().map(|lm| lm.source.clone());
                self.reconcile(&fin, source.as_deref());
            }
        }
        if let Some(total) = group_spend {
            self.ledger.spend.insert(gid, total);
        }
        // a group's notes are its items' (a single-item group is selected
        // as the group)
        let mut group_notes: Vec<String> = Vec::new();
        for item_id in &g.event_items {
            for n in self.ledger.notes.get(item_id).into_iter().flatten() {
                if !group_notes.contains(n) {
                    group_notes.push(n.clone());
                }
            }
        }
        if !group_notes.is_empty() {
            self.ledger.notes.insert(gid, group_notes);
        }
    }

    fn pay(&mut self, s: &SlotSpend, event: &str) {
        if s.pp == 0 {
            return;
        }
        let Some(slot) = self.slots.get_mut(s.slot).and_then(|x| x.as_mut()) else { return };
        slot.cur -= s.pp;
        let mut extras: Vec<&str> = Vec::new();
        let per_turn = if s.pressure { 2 * s.turns } else { s.turns };
        if s.kind == SpendKind::Ko && s.pp != per_turn {
            extras.push("locked");
        }
        if s.pressure {
            extras.push("Pressure");
        }
        let extra = if extras.is_empty() { String::new() } else { format!(", {}", extras.join(", ")) };
        let what = match s.kind {
            SpendKind::Ko => format!("{}, {} {}", s.move_name, s.turns, if s.turns == 1 { "hit" } else { "hits" }),
            SpendKind::Setup => format!("{} ×{}", s.move_name, s.turns),
            SpendKind::Mimic => format!("Mimic, {} {}", s.turns, if s.turns == 1 { "hit" } else { "hits" }),
            SpendKind::Transform => "Transform".to_string(),
        };
        self.log(s.slot, -s.pp, format!("{} ({}{})", event, what, extra));
    }

    fn use_item(&mut self, item_id: NodeId, i: &InventoryEventDefinition, init: &RouteState) {
        let Some(effect) = self.gen.pp_item_effect(&i.item_name) else { return };
        let (targets, amount): (Vec<usize>, PpAmount) = match effect {
            PpItemEffect::RestoreOne(a) => {
                let Some(slot) = i.target_move.as_deref().and_then(|t| init.solo_pkmn.slot_of_move(t)) else { return };
                (vec![slot], a)
            }
            PpItemEffect::RestoreAll(a) => ((0..4).filter(|s| self.slots[*s].is_some()).collect(), a),
            // PP Up / PP Max: applied from the engine's PP Ups
            PpItemEffect::PpUp | PpItemEffect::PpMax => return,
        };
        let label = format!("{} x{}", i.item_name, i.item_amount);
        for slot in targets {
            let max = self.max_pp(init, slot);
            let Some(cur) = self.slots[slot].as_ref().map(|s| s.cur) else { continue };
            if cur >= max {
                let name = self.slots[slot].as_ref().map(|s| s.move_name.clone()).unwrap_or_default();
                self.ledger.notes.entry(item_id).or_default().push(format!("{} was already full; the game would refuse this {}", name, i.item_name));
                continue;
            }
            let mut new = cur;
            for _ in 0..i.item_amount.max(0) {
                new = match amount {
                    PpAmount::Points(n) => (new + n).min(max),
                    PpAmount::Full => max,
                };
            }
            if let Some(s) = self.slots[slot].as_mut() {
                s.cur = new;
            }
            self.log(slot, new - cur, label.clone());
        }
    }
}
