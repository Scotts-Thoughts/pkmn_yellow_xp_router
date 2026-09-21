//! Comparing two routes (`docs/rust_port/design/route_compare/SPEC.md` §5).
//!
//! Pure engine code: no UI types, everything returned is owned and `Send` so
//! it can cross the loader thread's channel. Two routes are reduced to a
//! [`RouteDigest`] each, then aligned by [`compare`].
//!
//! Route files carry no event ids, so events are matched *structurally*:
//! trainer fights are the anchors (deterministic and near-unique) and
//! everything between two shared fights is compared as an unordered set.

use std::collections::{HashMap, HashSet};

use xpr_core::consts;
use xpr_core::io_utils::sanitize_string;
use xpr_data::model::{Nature, StatBlock, Trainer};

use crate::events::LevelVal;
use crate::router::Router;
use crate::state::RouteState;
use crate::tree::NodeId;

// ---------------------------------------------------------------------------
// Digest types
// ---------------------------------------------------------------------------

/// Where a compared route came from (display only).
#[derive(Clone, Debug, PartialEq)]
pub enum RouteOrigin {
    /// The route open in the editor, compared from memory (D9).
    CurrentRoute,
    /// A file in the saved-routes folder; `mtime` is seconds since the epoch.
    Saved { mtime: f64 },
    /// Any other file on disk.
    External,
}

/// How a move entered the moveset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MoveSource {
    LevelUp,
    /// The TM/HM item name as stored in the route (`"TM46 Thief"`).
    TmHm(String),
    Tutor,
}

impl MoveSource {
    /// Short display string: `Level-up`, `TM46`, `Tutor`.
    pub fn short_label(&self) -> String {
        match self {
            MoveSource::LevelUp => "Level-up".to_string(),
            MoveSource::Tutor => "Tutor".to_string(),
            MoveSource::TmHm(item) => item.split_whitespace().next().unwrap_or(item).to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LearnedMove {
    pub name: String,
    pub source: MoveSource,
    /// The solo mon's level when the move was learned.
    pub level: i64,
}

/// What kind of thing one [`Entry`] is. The string is the kind caption drawn
/// in the event diff (SPEC §6.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntryKind {
    Trainer,
    Wild,
    PickUp,
    Buy,
    Sell,
    UseItem,
    HoldItem,
    LearnMove,
    RareCandy,
    Vitamin,
    Heal,
    Save,
    Blackout,
    Evolution,
    BagReorder,
    EvOverride,
}

impl EntryKind {
    pub fn caption(self) -> &'static str {
        match self {
            EntryKind::Trainer => "TRAINER",
            EntryKind::Wild => "WILD",
            EntryKind::PickUp => "ITEM",
            EntryKind::Buy => "BUY",
            EntryKind::Sell => "SELL",
            EntryKind::UseItem => "USE",
            EntryKind::HoldItem => "HOLD",
            EntryKind::LearnMove => "MOVE",
            EntryKind::RareCandy => "CANDY",
            EntryKind::Vitamin => "VITAMIN",
            EntryKind::Heal => "HEAL",
            EntryKind::Save => "SAVE",
            EntryKind::Blackout => "BLACKOUT",
            EntryKind::Evolution => "EVOLVE",
            EntryKind::BagReorder => "BAG",
            EntryKind::EvOverride => "EV",
        }
    }

    /// Which filter toggle in the event diff owns this kind (SPEC §6.3).
    pub fn filter(self) -> DiffFilter {
        match self {
            EntryKind::Trainer => DiffFilter::Trainers,
            EntryKind::Wild => DiffFilter::Wild,
            EntryKind::PickUp | EntryKind::Buy | EntryKind::Sell | EntryKind::UseItem | EntryKind::HoldItem | EntryKind::BagReorder => DiffFilter::Items,
            EntryKind::LearnMove => DiffFilter::Moves,
            EntryKind::RareCandy | EntryKind::Vitamin | EntryKind::EvOverride => DiffFilter::CandyVitamins,
            EntryKind::Heal | EntryKind::Save | EntryKind::Blackout | EntryKind::Evolution => DiffFilter::HealsSaves,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DiffFilter {
    Trainers,
    Wild,
    Items,
    Moves,
    CandyVitamins,
    HealsSaves,
}

impl DiffFilter {
    pub const ALL: [DiffFilter; 6] = [
        DiffFilter::Trainers,
        DiffFilter::Wild,
        DiffFilter::Items,
        DiffFilter::Moves,
        DiffFilter::CandyVitamins,
        DiffFilter::HealsSaves,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DiffFilter::Trainers => "Trainers",
            DiffFilter::Wild => "Wild",
            DiffFilter::Items => "Items",
            DiffFilter::Moves => "Moves",
            DiffFilter::CandyVitamins => "Candy & vitamins",
            DiffFilter::HealsSaves => "Heals & saves",
        }
    }

    /// Everything but `HealsSaves` starts on.
    pub fn default_on(self) -> bool {
        self != DiffFilter::HealsSaves
    }
}

/// A plain copy of the parts of a [`RouteState`] this feature compares.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub level: i64,
    pub xp: i64,
    /// hp, attack, defense, special_attack, special_defense, speed
    pub stats: [i64; 6],
    /// The running EV / stat-XP total (`unrealized_stat_xp`).
    pub evs: [i64; 6],
    pub money: i64,
    pub moves: [Option<String>; 4],
    pub held_item: Option<String>,
    pub badge_count: i64,
}

impl Snapshot {
    fn of(state: &RouteState) -> Snapshot {
        let mon = &state.solo_pkmn;
        let mut moves: [Option<String>; 4] = Default::default();
        for (slot, m) in moves.iter_mut().zip(mon.move_list.iter()) {
            slot.clone_from(m);
        }
        Snapshot {
            level: mon.cur_level,
            xp: mon.cur_xp,
            stats: flatten(&mon.cur_stats),
            evs: flatten(&mon.unrealized_stat_xp),
            money: state.inventory.cur_money,
            moves,
            held_item: mon.held_item.clone(),
            badge_count: state.badges.num_badges(),
        }
    }

    pub fn ev_total(&self) -> i64 {
        self.evs.iter().sum()
    }

    /// The non-empty moves, for set comparison (slot order ignored).
    pub fn move_set(&self) -> HashSet<&str> {
        self.moves.iter().filter_map(|m| m.as_deref()).filter(|m| !m.is_empty()).collect()
    }
}

fn flatten(sb: &StatBlock) -> [i64; 6] {
    [sb.hp, sb.attack, sb.defense, sb.special_attack, sb.special_defense, sb.speed]
}

/// One enabled, non-notes event of a route, in route order.
#[derive(Clone, Debug)]
pub struct Entry {
    pub kind: EntryKind,
    /// The structural match key (SPEC §5.3).
    pub key: String,
    /// Display text without the kind caption (`"Poochyena L3"`, `"Poké Ball"`).
    pub label: String,
    /// 1 unless the event carries an amount (wild quantity, item amount,
    /// candies, vitamins).
    pub qty: i64,
    /// The enclosing folder's name (display only).
    pub folder: String,
    /// `trainer.location` for fights.
    pub location: Option<String>,
    pub before: Snapshot,
    pub xp_gain: i64,
    pub money_delta: i64,
    /// Recorded time in seconds, parsed from `"H:MM:SS.ss"`.
    pub recorded_secs: Option<f64>,
    pub is_major: bool,
    pub fight_category: Option<&'static str>,
    /// Trainers only: location + team, used to pair differently-named but
    /// identical fights (SPEC §5.4 step 1).
    pub team_sig: Option<String>,
    /// Trainers only: how many Pokémon the trainer has.
    pub enemy_count: usize,
    pub has_error: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Totals {
    pub trainers: i64,
    pub major_fights: i64,
    pub trainer_pokemon: i64,
    pub wild_pokemon: i64,
    pub wild_species: i64,
    pub trainer_owned_singles: i64,
    pub rare_candies: i64,
    pub vitamins: i64,
    pub vitamins_by_name: Vec<(String, i64)>,
    pub ev_berries: i64,
    pub moves_level_up: i64,
    pub moves_tm_hm: i64,
    pub moves_tutor: i64,
    pub items_picked_up: i64,
    pub purchases: i64,
    pub sales: i64,
    pub items_used: i64,
    pub held_item_changes: i64,
    pub heals: i64,
    pub saves: i64,
    pub blackouts: i64,
    pub evolutions: i64,
    pub events_with_errors: i64,
    pub xp_from_trainers: i64,
    pub xp_from_wild: i64,
    pub xp_from_candies: i64,
    pub xp_from_other: i64,
    pub money_from_trainers: i64,
    pub money_from_sales: i64,
    /// Negative.
    pub money_spent: i64,
    /// Negative.
    pub money_lost_blackouts: i64,
    pub money_other: i64,
}

impl Totals {
    pub fn moves_learned(&self) -> i64 {
        self.moves_level_up + self.moves_tm_hm + self.moves_tutor
    }

    pub fn xp_total(&self) -> i64 {
        self.xp_from_trainers + self.xp_from_wild + self.xp_from_candies + self.xp_from_other
    }

    pub fn money_total(&self) -> i64 {
        self.money_from_trainers + self.money_from_sales + self.money_spent + self.money_lost_blackouts + self.money_other
    }
}

/// Everything the compare screen needs about one route.
#[derive(Clone, Debug)]
pub struct RouteDigest {
    pub label: String,
    pub origin: RouteOrigin,
    pub version: String,
    pub generation: u8,
    pub species: String,
    /// hp, attack, defense, special_attack, special_defense, speed.
    /// In gen 1 `special_attack == special_defense == Special`.
    pub dvs: [i64; 6],
    pub nature: Option<String>,
    /// Short stat name of the nature's raised stat (`"SpA"`), `None` when neutral.
    pub nature_up: Option<&'static str>,
    pub nature_down: Option<&'static str>,
    pub ability: Option<String>,
    /// `(type, base power)`; `None` in gen 1.
    pub hidden_power: Option<(String, i64)>,
    pub start: Snapshot,
    pub end: Snapshot,
    pub totals: Totals,
    pub entries: Vec<Entry>,
    pub moves_learned: Vec<LearnedMove>,
    pub folder_count: usize,
    pub disabled_events: usize,
    /// The last parseable recorded time of the route, in seconds.
    pub final_time: Option<f64>,
}

impl RouteDigest {
    pub fn event_count(&self) -> usize {
        self.entries.len()
    }

    /// The `DVs` / `IVs` wording for this route's generation, as
    /// `custom_dvs.rs` picks it.
    pub fn dv_text(&self) -> &'static str {
        if self.generation <= 2 {
            "DVs"
        } else {
            "IVs"
        }
    }

    /// The `StatExp` / `EVs` wording for this route's generation.
    pub fn ev_text(&self) -> &'static str {
        if self.generation <= 2 {
            "StatExp"
        } else {
            "EVs"
        }
    }

    pub fn dv_total(&self) -> i64 {
        if self.generation == 1 {
            // Special is one stat in gen 1; do not count it twice.
            self.dvs[0] + self.dvs[1] + self.dvs[2] + self.dvs[3] + self.dvs[5]
        } else {
            self.dvs.iter().sum()
        }
    }
}

// ---------------------------------------------------------------------------
// Comparison types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct Compat {
    pub same_version: bool,
    pub same_generation: bool,
    pub same_species: bool,
    /// Both digests carry the same label. Only a hint: two different files
    /// can share a name, so a caller that knows where the routes came from
    /// should overwrite this (the compare page does).
    pub same_file: bool,
}

/// How one merged item in a block relates to the other lane.
#[derive(Clone, Debug, PartialEq)]
pub enum ItemStatus {
    /// Same key and same quantity in the other lane of this block.
    InBoth,
    QtyDiffers {
        other_qty: i64,
    },
    /// Not in the other lane of this block.
    OnlyHere,
    /// Fought in both routes, but in a different order. `other_after` is the
    /// anchor it follows on the other side (`None` = at the start).
    TrainerMoved {
        other_after: Option<String>,
    },
    /// A second fight with a trainer already matched earlier.
    TrainerRepeat,
}

#[derive(Clone, Debug)]
pub struct BlockItem {
    /// Index into the owning digest's `entries`.
    pub entry: usize,
    pub qty: i64,
    pub status: ItemStatus,
    /// The label after merging (`"Poochyena L5–6"`).
    pub merged_label: String,
    pub kind: EntryKind,
    /// The structural match key (§5.3). Items are paired across lanes by this,
    /// never by their display label: two heals match whatever their location.
    pub key: String,
}

#[derive(Clone, Debug)]
pub enum DiffRow {
    /// The same fight in the same place: indices into `a.entries` / `b.entries`.
    Anchor { a: usize, b: usize },
    /// Everything between two anchors.
    Block { a: Vec<BlockItem>, b: Vec<BlockItem> },
}

#[derive(Clone, Debug)]
pub struct Checkpoint {
    pub a: usize,
    pub b: usize,
    pub same_order: bool,
    /// How many moves differ between the two pre-fight movesets: the larger
    /// of "A has, B lacks" and "B has, A lacks", so a superset still counts.
    pub moves_differing: usize,
}

#[derive(Clone, Debug, Default)]
pub struct TrainerSets {
    pub same_order: usize,
    pub different_order: usize,
    pub only_a: Vec<String>,
    pub only_b: Vec<String>,
    pub repeat_a: Vec<String>,
    pub repeat_b: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct RouteComparison {
    pub a: RouteDigest,
    pub b: RouteDigest,
    pub compat: Compat,
    pub rows: Vec<DiffRow>,
    pub checkpoints: Vec<Checkpoint>,
    pub trainer_sets: TrainerSets,
    pub both_have_times: bool,
}

impl RouteComparison {
    /// How many blocks contain at least one difference.
    pub fn blocks_with_differences(&self) -> usize {
        self.rows
            .iter()
            .filter(|r| match r {
                DiffRow::Block { a, b } => a.iter().chain(b.iter()).any(|i| i.status != ItemStatus::InBoth),
                _ => false,
            })
            .count()
    }

    pub fn shared_fight_count(&self) -> usize {
        self.rows.iter().filter(|r| matches!(r, DiffRow::Anchor { .. })).count()
    }
}

// ---------------------------------------------------------------------------
// Step 1: digest
// ---------------------------------------------------------------------------

/// Reduce one loaded, recalculated router to a [`RouteDigest`].
///
/// Disabled events (D5) and notes-only events are skipped entirely.
pub fn digest(router: &Router, label: &str, origin: RouteOrigin) -> RouteDigest {
    let gen = router.gen();
    let generation = gen.map(|g| g.get_generation()).unwrap_or(0);
    let start = router.init_route_state.as_ref().map(|s| Snapshot::of(s)).unwrap_or_default();
    let end = router.get_final_state().map(|s| Snapshot::of(s)).unwrap_or_else(|| start.clone());

    let mut totals = Totals::default();
    let mut entries: Vec<Entry> = Vec::new();
    let mut moves_learned: Vec<LearnedMove> = Vec::new();
    let mut disabled_events = 0usize;
    let mut wild_species: HashSet<String> = HashSet::new();
    let mut vitamin_counts: Vec<(String, i64)> = Vec::new();
    let mut final_time: Option<f64> = None;

    for gid in router.all_groups() {
        let Some(group) = router.group(gid) else { continue };
        let def = &group.event_definition;
        if def.get_event_type() == consts::TASK_NOTES_ONLY {
            continue;
        }
        if !router.is_enabled(gid) {
            disabled_events += 1;
            continue;
        }

        let before = group.init_state.as_ref().map(|s| Snapshot::of(s));
        let after = group.final_state.as_ref().map(|s| Snapshot::of(s));
        let (xp_gain, money_delta) = match (&before, &after) {
            (Some(i), Some(f)) => (f.xp - i.xp, f.money - i.money),
            _ => (0, 0),
        };
        let has_error = group.has_errors();
        if has_error {
            totals.events_with_errors += 1;
        }
        if let Some(secs) = parse_time(def.recorded_time_str().as_deref()) {
            final_time = Some(secs);
        }

        // Moves learned by the level-up items the engine injects under this
        // group, plus this group itself when it is a LearnMove event.
        collect_learned_moves(router, gid, &mut moves_learned, &mut totals);

        let folder = router.folder(group.parent).map(|f| f.name.clone()).unwrap_or_default();
        let mut entry = Entry {
            kind: EntryKind::Trainer,
            key: String::new(),
            label: String::new(),
            qty: 1,
            folder,
            location: None,
            before: before.unwrap_or_default(),
            xp_gain,
            money_delta,
            recorded_secs: parse_time(def.recorded_time_str().as_deref()),
            is_major: false,
            fight_category: None,
            team_sig: None,
            enemy_count: 0,
            has_error,
        };

        if let Some(td) = &def.trainer_def {
            let trainer = gen.and_then(|g| g.trainer_db().get_trainer(&td.trainer_name));
            entry.kind = EntryKind::Trainer;
            entry.key = trainer_key(&td.trainer_name, &td.second_trainer_name);
            entry.label = td.trainer_name.clone();
            entry.location = trainer.map(|t| t.location.clone()).filter(|l| !l.is_empty());
            entry.is_major = gen.map(|g| g.is_major_fight(&td.trainer_name)).unwrap_or(false);
            entry.fight_category = gen
                .and_then(|g| g.get_fight_category(&td.trainer_name))
                .and_then(consts::fight_category_to_tag);
            entry.team_sig = trainer.map(|t| team_signature(t));
            entry.enemy_count = gen
                .and_then(|g| def.get_pokemon_list(g, false).ok())
                .map(|l| l.len())
                .unwrap_or(0);
            totals.trainers += 1;
            totals.trainer_pokemon += entry.enemy_count as i64;
            if entry.is_major {
                totals.major_fights += 1;
            }
            totals.xp_from_trainers += xp_gain;
            // Pay Day and other odd money goes to "other"; the prize is the rest.
            if money_delta != 0 {
                totals.money_from_trainers += money_delta;
            }
        } else if let Some(w) = &def.wild_pkmn_info {
            entry.kind = EntryKind::Wild;
            entry.key = format!("w:{}", sanitize_string(&w.name));
            entry.label = format!("{} L{}", w.name, w.level);
            entry.qty = w.quantity.max(1);
            if w.trainer_pkmn {
                totals.trainer_owned_singles += entry.qty;
            } else {
                totals.wild_pokemon += entry.qty;
            }
            wild_species.insert(sanitize_string(&w.name));
            totals.xp_from_wild += xp_gain;
        } else if let Some(rc) = &def.rare_candy {
            entry.kind = EntryKind::RareCandy;
            entry.key = "candy".to_string();
            entry.label = "Rare Candy".to_string();
            entry.qty = rc.amount.max(1);
            totals.rare_candies += rc.amount;
            totals.xp_from_candies += xp_gain;
        } else if let Some(v) = &def.vitamin {
            entry.kind = EntryKind::Vitamin;
            entry.key = format!("v:{}", sanitize_string(&v.vitamin));
            entry.label = v.vitamin.clone();
            entry.qty = v.amount.max(1);
            let is_berry = gen.map(|g| g.is_ev_berry(&v.vitamin)).unwrap_or(false);
            if is_berry {
                totals.ev_berries += v.amount;
            } else {
                totals.vitamins += v.amount;
            }
            match vitamin_counts.iter_mut().find(|(n, _)| n == &v.vitamin) {
                Some(e) => e.1 += v.amount,
                None => vitamin_counts.push((v.vitamin.clone(), v.amount)),
            }
        } else if let Some(i) = &def.item_event_def {
            let kind = match (i.is_acquire, i.with_money) {
                (true, true) => EntryKind::Buy,
                (true, false) => EntryKind::PickUp,
                (false, true) => EntryKind::Sell,
                (false, false) => EntryKind::UseItem,
            };
            entry.kind = kind;
            entry.key = format!("i:{:?}:{}", kind, sanitize_string(&i.item_name));
            entry.label = i.item_name.clone();
            entry.qty = i.item_amount.max(1);
            match kind {
                EntryKind::Buy => {
                    totals.purchases += 1;
                    totals.money_spent += money_delta.min(0);
                }
                EntryKind::PickUp => totals.items_picked_up += 1,
                EntryKind::Sell => {
                    totals.sales += 1;
                    totals.money_from_sales += money_delta.max(0);
                }
                _ => totals.items_used += 1,
            }
        } else if let Some(h) = &def.hold_item {
            entry.kind = EntryKind::HoldItem;
            let name = h.item_name.clone().unwrap_or_default();
            entry.key = if name.is_empty() {
                "h:none".to_string()
            } else {
                format!("h:{}", sanitize_string(&name))
            };
            entry.label = if name.is_empty() { "Remove held item".to_string() } else { name };
            totals.held_item_changes += 1;
        } else if let Some(lm) = &def.learn_move {
            entry.kind = EntryKind::LearnMove;
            entry.key = match &lm.move_to_learn {
                Some(m) => format!("m:{}", sanitize_string(m)),
                None => format!("m:delete:{}", lm.destination.unwrap_or(-1)),
            };
            entry.label = lm.move_to_learn.clone().unwrap_or_else(|| "Delete move".to_string());
        } else if def.heal.is_some() {
            entry.kind = EntryKind::Heal;
            entry.key = "heal".to_string();
            entry.label = location_label(def.heal.as_ref().and_then(|l| l.location.as_deref()), "Heal");
            totals.heals += 1;
        } else if def.save.is_some() {
            entry.kind = EntryKind::Save;
            entry.key = "save".to_string();
            entry.label = location_label(def.save.as_ref().and_then(|l| l.location.as_deref()), "Save");
            totals.saves += 1;
        } else if def.blackout.is_some() {
            entry.kind = EntryKind::Blackout;
            entry.key = "blackout".to_string();
            entry.label = location_label(def.blackout.as_ref().and_then(|l| l.location.as_deref()), "Blackout");
            totals.blackouts += 1;
            totals.money_lost_blackouts += money_delta.min(0);
        } else if let Some(e) = &def.evolution {
            entry.kind = EntryKind::Evolution;
            let species = e.evolved_species.clone().unwrap_or_default();
            entry.key = format!("e:{}", sanitize_string(&species));
            entry.label = species;
            totals.evolutions += 1;
        } else if def.bag_reorder.is_some() {
            entry.kind = EntryKind::BagReorder;
            entry.key = "bag".to_string();
            entry.label = "Reorder Bag".to_string();
        } else if def.ev_override.is_some() {
            entry.kind = EntryKind::EvOverride;
            entry.key = "evo".to_string();
            entry.label = "EV Override".to_string();
        } else {
            continue;
        }

        // Money and exp that no specific bucket claimed.
        if !matches!(entry.kind, EntryKind::Trainer | EntryKind::Wild | EntryKind::RareCandy) {
            totals.xp_from_other += xp_gain;
        }
        if !matches!(
            entry.kind,
            EntryKind::Trainer | EntryKind::Buy | EntryKind::Sell | EntryKind::Blackout
        ) && money_delta != 0
        {
            totals.money_other += money_delta;
        }
        // A purchase that somehow gained money, or a sale that lost it.
        if entry.kind == EntryKind::Buy && money_delta > 0 {
            totals.money_other += money_delta;
        }
        if entry.kind == EntryKind::Sell && money_delta < 0 {
            totals.money_other += money_delta;
        }
        if entry.kind == EntryKind::Blackout && money_delta > 0 {
            totals.money_other += money_delta;
        }

        entries.push(entry);
    }

    totals.wild_species = wild_species.len() as i64;
    vitamin_counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    totals.vitamins_by_name = vitamin_counts;

    let dvs = router
        .init_route_state
        .as_ref()
        .map(|s| flatten(&s.solo_pkmn.dvs))
        .unwrap_or([0; 6]);
    let nature = router.init_route_state.as_ref().map(|s| s.solo_pkmn.nature);
    let hidden_power = match (gen, generation) {
        (Some(g), gn) if gn >= 2 => router
            .init_route_state
            .as_ref()
            .map(|s| g.get_hidden_power(&s.solo_pkmn.dvs)),
        _ => None,
    };

    RouteDigest {
        label: label.to_string(),
        origin,
        version: router.pkmn_version.clone().unwrap_or_default(),
        generation,
        species: router
            .init_route_state
            .as_ref()
            .map(|s| s.solo_pkmn.name.clone())
            .unwrap_or_default(),
        dvs,
        nature: if generation >= 3 { nature.map(|n| n.display_name()) } else { None },
        nature_up: if generation >= 3 { nature.and_then(nature_raised) } else { None },
        nature_down: if generation >= 3 { nature.and_then(nature_lowered) } else { None },
        ability: if generation >= 3 {
            router
                .init_route_state
                .as_ref()
                .map(|s| s.solo_pkmn.ability.clone())
                .filter(|a| !a.is_empty())
        } else {
            None
        },
        hidden_power,
        start,
        end,
        totals,
        entries,
        moves_learned,
        folder_count: count_folders(router),
        disabled_events,
        final_time,
    }
}

/// A move counts as learned only when the moveset actually changed: a
/// level-up move that was skipped is not "learned" (SPEC §5.1).
fn collect_learned_moves(router: &Router, gid: NodeId, out: &mut Vec<LearnedMove>, totals: &mut Totals) {
    let Some(group) = router.group(gid) else { return };
    for item_id in &group.event_items {
        let Some(item) = router.item(*item_id) else { continue };
        let Some(lm) = &item.event_definition.learn_move else { continue };
        let Some(name) = &lm.move_to_learn else { continue };
        let (Some(before), Some(after)) = (&item.init_state, &item.final_state) else { continue };
        if before.solo_pkmn.move_list == after.solo_pkmn.move_list {
            continue;
        }
        let source = if lm.source == consts::MOVE_SOURCE_LEVELUP {
            totals.moves_level_up += 1;
            MoveSource::LevelUp
        } else if lm.source == consts::MOVE_SOURCE_TUTOR {
            totals.moves_tutor += 1;
            MoveSource::Tutor
        } else {
            totals.moves_tm_hm += 1;
            MoveSource::TmHm(lm.source.clone())
        };
        let level = match &lm.level {
            LevelVal::Int(n) => *n,
            _ => before.solo_pkmn.cur_level,
        };
        out.push(LearnedMove {
            name: name.clone(),
            source,
            level: if level > 0 { level } else { before.solo_pkmn.cur_level },
        });
    }
}

fn count_folders(router: &Router) -> usize {
    fn walk(router: &Router, id: NodeId, n: &mut usize) {
        *n += 1;
        for child in router.children_of(id) {
            if router.folder(child).is_some() {
                walk(router, child, n);
            }
        }
    }
    let mut n = 0;
    walk(router, router.root_id, &mut n);
    n
}

fn trainer_key(name: &str, second: &serde_json::Value) -> String {
    let mut key = format!("t:{}", sanitize_string(name));
    if let serde_json::Value::String(s) = second {
        if !s.is_empty() {
            key.push('+');
            key.push_str(&sanitize_string(s));
        }
    }
    key
}

/// Location + the trainer's team in definition order, used to pair fights
/// that differ only in the trainer's name (rival name variants).
fn team_signature(t: &Trainer) -> String {
    let mons: Vec<String> = t
        .pkmn
        .iter()
        .map(|p| format!("{}:{}", sanitize_string(&p.name), p.level))
        .collect();
    format!("{}|{}", sanitize_string(&t.location), mons.join(","))
}

/// Recorded routes store map ids such as `OLDALE_TOWN - POKEMON_CENTER_1F`;
/// show the town part, title-cased (SPEC §5.1).
fn location_label(location: Option<&str>, fallback: &str) -> String {
    let Some(raw) = location.map(str::trim).filter(|s| !s.is_empty()) else {
        return fallback.to_string();
    };
    let head = raw.split(" - ").next().unwrap_or(raw);
    if !head.contains('_') && head.chars().any(|c| c.is_lowercase()) {
        // Hand-typed, pass through unchanged.
        return head.to_string();
    }
    head.split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(f) => f.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `"H:MM:SS.ss"` -> seconds.
pub fn parse_time(raw: Option<&str>) -> Option<f64> {
    let raw = raw?.trim();
    if raw.is_empty() {
        return None;
    }
    let mut total = 0.0_f64;
    let parts: Vec<&str> = raw.split(':').collect();
    if parts.len() > 3 {
        return None;
    }
    for part in &parts {
        let v: f64 = part.trim().parse().ok()?;
        total = total * 60.0 + v;
    }
    Some(total)
}

/// Seconds -> `"H:MM:SS.s"`, dropping the hour when it is zero.
pub fn format_time(secs: f64) -> String {
    let neg = secs < 0.0;
    let s = secs.abs();
    let h = (s / 3600.0).floor() as i64;
    let m = ((s % 3600.0) / 60.0).floor() as i64;
    let rem = s % 60.0;
    let body = if h > 0 {
        format!("{}:{:02}:{:04.1}", h, m, rem)
    } else {
        format!("{}:{:04.1}", m, rem)
    };
    if neg {
        format!("\u{2212}{}", body)
    } else {
        body
    }
}

/// Seconds -> a signed delta string (`"+2:58.6"`).
pub fn format_delta(secs: f64) -> String {
    if secs.abs() < 0.05 {
        return "\u{2014}".to_string();
    }
    let body = format_time(secs.abs());
    if secs < 0.0 {
        format!("\u{2212}{}", body)
    } else {
        format!("+{}", body)
    }
}

fn nature_raised(n: Nature) -> Option<&'static str> {
    STAT_SHORT.iter().find(|(full, _)| n.is_stat_raised(full)).map(|(_, short)| *short)
}

fn nature_lowered(n: Nature) -> Option<&'static str> {
    STAT_SHORT.iter().find(|(full, _)| n.is_stat_lowered(full)).map(|(_, short)| *short)
}

const STAT_SHORT: [(&str, &str); 5] = [
    (consts::ATTACK, "Atk"),
    (consts::DEFENSE, "Def"),
    (consts::SPECIAL_ATTACK, "SpA"),
    (consts::SPECIAL_DEFENSE, "SpD"),
    (consts::SPEED, "Spe"),
];

// ---------------------------------------------------------------------------
// Step 2: compare
// ---------------------------------------------------------------------------

/// Align two digests (SPEC §5.4). Deltas elsewhere are always `B − A`.
pub fn compare(a: RouteDigest, b: RouteDigest) -> RouteComparison {
    let compat = Compat {
        same_version: a.version == b.version,
        same_generation: a.generation == b.generation,
        same_species: sanitize_string(&a.species) == sanitize_string(&b.species),
        same_file: a.label == b.label,
    };

    let ta: Vec<usize> = a.entries.iter().enumerate().filter(|(_, e)| e.kind == EntryKind::Trainer).map(|(i, _)| i).collect();
    let tb: Vec<usize> = b.entries.iter().enumerate().filter(|(_, e)| e.kind == EntryKind::Trainer).map(|(i, _)| i).collect();

    // Step 1: pair differently-named trainers that have an identical team.
    let (keys_a, keys_b, names) = equivalent_keys(&a, &b, &ta, &tb);

    // Step 2: LCS over the key sequences.
    let matched = lcs(&keys_a, &keys_b);

    // Step 3: classify the trainers the LCS did not pair.
    let mut in_lcs_a: HashSet<usize> = HashSet::new();
    let mut in_lcs_b: HashSet<usize> = HashSet::new();
    for (i, j) in &matched {
        in_lcs_a.insert(*i);
        in_lcs_b.insert(*j);
    }
    let lcs_keys: HashSet<&String> = matched.iter().map(|(i, _)| &keys_a[*i]).collect();

    let mut status_a: HashMap<usize, ItemStatus> = HashMap::new();
    let mut status_b: HashMap<usize, ItemStatus> = HashMap::new();
    let mut moved_pairs: Vec<(usize, usize)> = Vec::new();
    let mut sets = TrainerSets {
        same_order: matched.len(),
        ..Default::default()
    };

    // Unmatched trainers, per side, grouped by key so the k-th occurrence on
    // one side pairs with the k-th on the other.
    let rest_a: Vec<usize> = (0..keys_a.len()).filter(|i| !in_lcs_a.contains(i)).collect();
    let rest_b: Vec<usize> = (0..keys_b.len()).filter(|j| !in_lcs_b.contains(j)).collect();
    let mut pool_b: HashMap<&str, Vec<usize>> = HashMap::new();
    for j in &rest_b {
        pool_b.entry(keys_b[*j].as_str()).or_default().push(*j);
    }
    let mut used_b: HashSet<usize> = HashSet::new();
    let mut seen_a: HashSet<&str> = HashSet::new();

    for i in &rest_a {
        let key = keys_a[*i].as_str();
        if lcs_keys.contains(&keys_a[*i]) || !seen_a.insert(key) {
            status_a.insert(*i, ItemStatus::TrainerRepeat);
            sets.repeat_a.push(names.a[*i].clone());
            continue;
        }
        match pool_b.get_mut(key).and_then(|v| v.iter().position(|j| !used_b.contains(j)).map(|p| v[p])) {
            Some(j) => {
                used_b.insert(j);
                moved_pairs.push((*i, j));
                sets.different_order += 1;
            }
            None => {
                status_a.insert(*i, ItemStatus::OnlyHere);
                sets.only_a.push(names.a[*i].clone());
            }
        }
    }
    let mut seen_b: HashSet<&str> = HashSet::new();
    for j in &rest_b {
        if used_b.contains(j) {
            continue;
        }
        let key = keys_b[*j].as_str();
        if lcs_keys.contains(&keys_b[*j]) || !seen_b.insert(key) {
            status_b.insert(*j, ItemStatus::TrainerRepeat);
            sets.repeat_b.push(names.b[*j].clone());
        } else {
            status_b.insert(*j, ItemStatus::OnlyHere);
            sets.only_b.push(names.b[*j].clone());
        }
    }

    // A moved trainer names the anchor it follows on the *other* side.
    for (i, j) in &moved_pairs {
        let after_a = preceding_anchor(&matched, *i, true).map(|k| names.a[k].clone());
        let after_b = preceding_anchor(&matched, *j, false).map(|k| names.b[k].clone());
        status_a.insert(*i, ItemStatus::TrainerMoved { other_after: after_b });
        status_b.insert(*j, ItemStatus::TrainerMoved { other_after: after_a });
    }

    // Step 4: walk both routes, emitting anchors and the blocks between them.
    let mut rows: Vec<DiffRow> = Vec::new();
    let mut pending_a: Vec<usize> = Vec::new();
    let mut pending_b: Vec<usize> = Vec::new();
    let mut ia = 0usize;
    let mut ib = 0usize;
    let mut next_anchor = 0usize;

    while next_anchor <= matched.len() {
        let (stop_a, stop_b) = if next_anchor < matched.len() {
            let (ti, tj) = matched[next_anchor];
            (ta[ti], tb[tj])
        } else {
            (a.entries.len(), b.entries.len())
        };
        while ia < stop_a {
            pending_a.push(ia);
            ia += 1;
        }
        while ib < stop_b {
            pending_b.push(ib);
            ib += 1;
        }
        let block_a = build_block(&a, &pending_a, &ta, &status_a);
        let block_b = build_block(&b, &pending_b, &tb, &status_b);
        let (block_a, block_b) = cross_mark(block_a, block_b);
        if !block_a.is_empty() || !block_b.is_empty() {
            rows.push(DiffRow::Block { a: block_a, b: block_b });
        }
        pending_a.clear();
        pending_b.clear();
        if next_anchor < matched.len() {
            rows.push(DiffRow::Anchor { a: stop_a, b: stop_b });
            ia = stop_a + 1;
            ib = stop_b + 1;
        }
        next_anchor += 1;
    }

    // Step 7: checkpoints = anchors + moved pairs, in A's order.
    let mut checkpoints: Vec<Checkpoint> = matched
        .iter()
        .map(|(i, j)| make_checkpoint(&a, &b, ta[*i], tb[*j], true))
        .chain(moved_pairs.iter().map(|(i, j)| make_checkpoint(&a, &b, ta[*i], tb[*j], false)))
        .collect();
    checkpoints.sort_by_key(|c| c.a);

    let both_have_times = a.final_time.is_some() && b.final_time.is_some();

    RouteComparison {
        a,
        b,
        compat,
        rows,
        checkpoints,
        trainer_sets: sets,
        both_have_times,
    }
}

struct DisplayNames {
    a: Vec<String>,
    b: Vec<String>,
}

/// Step 1 of §5.4: a trainer key present on only one side is paired with the
/// other side's unmatched trainer that has the same team signature, but only
/// when there is exactly one candidate.
fn equivalent_keys(a: &RouteDigest, b: &RouteDigest, ta: &[usize], tb: &[usize]) -> (Vec<String>, Vec<String>, DisplayNames) {
    let mut keys_a: Vec<String> = ta.iter().map(|i| a.entries[*i].key.clone()).collect();
    let mut keys_b: Vec<String> = tb.iter().map(|j| b.entries[*j].key.clone()).collect();
    let mut names_a: Vec<String> = ta.iter().map(|i| a.entries[*i].label.clone()).collect();
    let mut names_b: Vec<String> = tb.iter().map(|j| b.entries[*j].label.clone()).collect();

    let set_a: HashSet<&String> = keys_a.iter().collect();
    let set_b: HashSet<&String> = keys_b.iter().collect();
    let only_a: Vec<usize> = (0..keys_a.len()).filter(|i| !set_b.contains(&keys_a[*i])).collect();
    let only_b: Vec<usize> = (0..keys_b.len()).filter(|j| !set_a.contains(&keys_b[*j])).collect();

    let mut pairs: Vec<(usize, usize, String)> = Vec::new();
    let mut claimed_b: HashSet<usize> = HashSet::new();
    for i in &only_a {
        let Some(sig) = &a.entries[ta[*i]].team_sig else { continue };
        if sig.ends_with('|') {
            continue; // no team data: never pair on an empty signature
        }
        let candidates: Vec<usize> = only_b
            .iter()
            .copied()
            .filter(|j| !claimed_b.contains(j) && b.entries[tb[*j]].team_sig.as_deref() == Some(sig.as_str()))
            .collect();
        // Ambiguous matches are left alone.
        if candidates.len() == 1 {
            let same_sig_a = only_a
                .iter()
                .filter(|k| a.entries[ta[**k]].team_sig.as_deref() == Some(sig.as_str()))
                .count();
            if same_sig_a == 1 {
                claimed_b.insert(candidates[0]);
                pairs.push((*i, candidates[0], format!("t:eq:{}", sig)));
            }
        }
    }
    for (i, j, key) in pairs {
        let combined = if names_a[i] == names_b[j] {
            names_a[i].clone()
        } else {
            format!("{} / {}", names_a[i], names_b[j])
        };
        keys_a[i] = key.clone();
        keys_b[j] = key;
        names_a[i] = combined.clone();
        names_b[j] = combined;
    }
    (keys_a, keys_b, DisplayNames { a: names_a, b: names_b })
}

/// Longest common subsequence over the trainer keys. When the backtrack has no
/// preference, advance A first so the output is deterministic.
fn lcs(a: &[String], b: &[String]) -> Vec<(usize, usize)> {
    let (n, m) = (a.len(), b.len());
    if n == 0 || m == 0 {
        return Vec::new();
    }
    let mut table = vec![0u32; (n + 1) * (m + 1)];
    let at = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[at(i, j)] = if a[i] == b[j] {
                table[at(i + 1, j + 1)] + 1
            } else {
                table[at(i + 1, j)].max(table[at(i, j + 1)])
            };
        }
    }
    let mut out = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if a[i] == b[j] {
            out.push((i, j));
            i += 1;
            j += 1;
        } else if table[at(i + 1, j)] >= table[at(i, j + 1)] {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

/// The LCS pair that precedes trainer index `idx` on its own side.
fn preceding_anchor(matched: &[(usize, usize)], idx: usize, side_a: bool) -> Option<usize> {
    matched
        .iter()
        .rfind(|(i, j)| if side_a { *i < idx } else { *j < idx })
        .map(|(i, j)| if side_a { *i } else { *j })
}

/// Merge one lane's pending entries into [`BlockItem`]s (§5.4 steps 4-5).
fn build_block(d: &RouteDigest, pending: &[usize], trainers: &[usize], status: &HashMap<usize, ItemStatus>) -> Vec<BlockItem> {
    let mut out: Vec<BlockItem> = Vec::new();
    for idx in pending {
        let entry = &d.entries[*idx];
        if entry.kind == EntryKind::Trainer {
            // A trainer inside a block is one the LCS did not pair.
            let t_pos = trainers.iter().position(|t| t == idx);
            let st = t_pos.and_then(|p| status.get(&p).cloned()).unwrap_or(ItemStatus::OnlyHere);
            out.push(BlockItem {
                entry: *idx,
                qty: 1,
                status: st,
                merged_label: entry.label.clone(),
                kind: EntryKind::Trainer,
                key: entry.key.clone(),
            });
            continue;
        }
        match out.iter_mut().find(|i| i.kind != EntryKind::Trainer && i.key == entry.key) {
            Some(existing) => {
                existing.qty += entry.qty;
                existing.merged_label = merge_label(&existing.merged_label, entry);
            }
            None => out.push(BlockItem {
                entry: *idx,
                qty: entry.qty,
                status: ItemStatus::OnlyHere,
                merged_label: entry.label.clone(),
                kind: entry.kind,
                key: entry.key.clone(),
            }),
        }
    }
    out
}

/// Wild encounters of one species merge into a level range.
fn merge_label(existing: &str, entry: &Entry) -> String {
    // Heals / saves / blackouts merge regardless of location, so a merged
    // line can only keep a place name when every merged event shares it.
    if matches!(entry.kind, EntryKind::Heal | EntryKind::Save | EntryKind::Blackout) && existing != entry.label {
        return match entry.kind {
            EntryKind::Heal => "Heal",
            EntryKind::Save => "Save",
            _ => "Blackout",
        }
        .to_string();
    }
    if entry.kind != EntryKind::Wild {
        return existing.to_string();
    }
    let Some((species, rest)) = existing.rsplit_once(" L") else {
        return existing.to_string();
    };
    let new_level = entry.label.rsplit_once(" L").and_then(|(_, l)| l.parse::<i64>().ok());
    let Some(new_level) = new_level else { return existing.to_string() };
    let (lo, hi) = match rest.split_once('\u{2013}') {
        Some((a, b)) => (a.parse::<i64>().unwrap_or(new_level), b.parse::<i64>().unwrap_or(new_level)),
        None => {
            let v = rest.parse::<i64>().unwrap_or(new_level);
            (v, v)
        }
    };
    let (lo, hi) = (lo.min(new_level), hi.max(new_level));
    if lo == hi {
        format!("{} L{}", species, lo)
    } else {
        format!("{} L{}\u{2013}{}", species, lo, hi)
    }
}

/// Step 6: decide `InBoth` / `QtyDiffers` / `OnlyHere` for the non-trainer
/// items of a block by looking at the other lane.
fn cross_mark(mut a: Vec<BlockItem>, mut b: Vec<BlockItem>) -> (Vec<BlockItem>, Vec<BlockItem>) {
    let keys_b: HashMap<String, i64> = b
        .iter()
        .filter(|i| i.kind != EntryKind::Trainer)
        .map(|i| (i.key.clone(), i.qty))
        .collect();
    let keys_a: HashMap<String, i64> = a
        .iter()
        .filter(|i| i.kind != EntryKind::Trainer)
        .map(|i| (i.key.clone(), i.qty))
        .collect();
    for item in a.iter_mut().filter(|i| i.kind != EntryKind::Trainer) {
        item.status = match keys_b.get(&item.key) {
            Some(q) if *q == item.qty => ItemStatus::InBoth,
            Some(q) => ItemStatus::QtyDiffers { other_qty: *q },
            None => ItemStatus::OnlyHere,
        };
    }
    for item in b.iter_mut().filter(|i| i.kind != EntryKind::Trainer) {
        item.status = match keys_a.get(&item.key) {
            Some(q) if *q == item.qty => ItemStatus::InBoth,
            Some(q) => ItemStatus::QtyDiffers { other_qty: *q },
            None => ItemStatus::OnlyHere,
        };
    }
    (a, b)
}


fn make_checkpoint(a: &RouteDigest, b: &RouteDigest, ia: usize, ib: usize, same_order: bool) -> Checkpoint {
    let set_a = a.entries[ia].before.move_set();
    let set_b = b.entries[ib].before.move_set();
    Checkpoint {
        a: ia,
        b: ib,
        same_order,
        moves_differing: set_a.difference(&set_b).count().max(set_b.difference(&set_a).count()),
    }
}

// ---------------------------------------------------------------------------
// Plain-text summary (SPEC §8.2)
// ---------------------------------------------------------------------------

/// The clipboard summary behind the "Copy summary" button.
pub fn text_summary(cmp: &RouteComparison) -> String {
    let (a, b) = (&cmp.a, &cmp.b);
    let mut out = String::new();
    let head = if cmp.compat.same_version && cmp.compat.same_species {
        format!("Route compare \u{2014} {} \u{b7} {}", a.version, a.species)
    } else {
        format!("Route compare \u{2014} {} {} vs {} {}", a.version, a.species, b.version, b.species)
    };
    out.push_str(&head);
    out.push('\n');
    out.push_str(&format!("A: {}\nB: {}\n\n", a.label, b.label));

    // Wide enough for a six-stat DV/IV string plus a space.
    let mut row = |name: &str, va: String, vb: String, delta: String| {
        out.push_str(&format!("{:<24}{:>20}{:>20}{:>12}\n", name, va, vb, delta));
    };
    row("", "A".into(), "B".into(), "B-A".into());

    let stat_str = |d: &RouteDigest| {
        if d.generation == 1 {
            format!("{}/{}/{}/{}/{}", d.dvs[0], d.dvs[1], d.dvs[2], d.dvs[3], d.dvs[5])
        } else {
            format!("{}/{}/{}/{}/{}/{}", d.dvs[0], d.dvs[1], d.dvs[2], d.dvs[3], d.dvs[4], d.dvs[5])
        }
    };
    row(a.dv_text(), stat_str(a), stat_str(b), String::new());
    if let (Some(na), Some(nb)) = (&a.nature, &b.nature) {
        row("Nature", na.clone(), nb.clone(), String::new());
    }
    if let (Some(aa), Some(ab)) = (&a.ability, &b.ability) {
        row("Ability", aa.clone(), ab.clone(), String::new());
    }
    if let (Some((ta, pa)), Some((tb, pb))) = (&a.hidden_power, &b.hidden_power) {
        row("Hidden Power", format!("{} {}", ta, pa), format!("{} {}", tb, pb), String::new());
    }

    let num = |name: &str, va: i64, vb: i64, out: &mut dyn FnMut(&str, String, String, String)| {
        if va != 0 || vb != 0 {
            let delta = if va == vb { String::new() } else { format!("{:+}", vb - va) };
            out(name, va.to_string(), vb.to_string(), delta);
        }
    };
    num("Final level", a.end.level, b.end.level, &mut row);
    if let (Some(fa), Some(fb)) = (a.final_time, b.final_time) {
        row("Final time", format_time(fa), format_time(fb), format_delta(fb - fa));
    }
    let (x, y) = (&a.totals, &b.totals);
    for (name, va, vb) in [
        ("Trainers fought", x.trainers, y.trainers),
        ("Major fights", x.major_fights, y.major_fights),
        ("Trainer Pokemon", x.trainer_pokemon, y.trainer_pokemon),
        ("Wild Pokemon fought", x.wild_pokemon, y.wild_pokemon),
        ("Rare Candies used", x.rare_candies, y.rare_candies),
        ("Vitamins used", x.vitamins, y.vitamins),
        ("Moves learned", x.moves_learned(), y.moves_learned()),
        ("Items picked up", x.items_picked_up, y.items_picked_up),
        ("Purchases", x.purchases, y.purchases),
        ("Sales", x.sales, y.sales),
        ("Pokemon Center heals", x.heals, y.heals),
        ("Saves", x.saves, y.saves),
        ("Blackouts", x.blackouts, y.blackouts),
        ("Final money", a.end.money, b.end.money),
    ] {
        num(name, va, vb, &mut row);
    }
    out
}

// ---------------------------------------------------------------------------
// Tests that need no route file
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_formats_times() {
        // The accumulating parse carries the usual float error, so compare
        // to the millisecond rather than exactly.
        let near = |got: Option<f64>, want: f64| assert!(matches!(got, Some(v) if (v - want).abs() < 1e-3), "{got:?} vs {want}");
        near(parse_time(Some("0:01:16.79")), 76.79);
        near(parse_time(Some("1:34:54.30")), 5694.30);
        near(parse_time(Some("12.5")), 12.5);
        assert_eq!(parse_time(Some("")), None);
        assert_eq!(parse_time(None), None);
        assert_eq!(parse_time(Some("nonsense")), None);
        assert_eq!(format_time(76.79), "1:16.8");
        assert_eq!(format_time(5694.3), "1:34:54.3");
    }

    #[test]
    fn titlecases_recorded_locations() {
        assert_eq!(location_label(Some("OLDALE_TOWN - POKEMON_CENTER_1F"), "Heal"), "Oldale Town");
        assert_eq!(location_label(Some("MT_CHIMNEY"), "Save"), "Mt Chimney");
        assert_eq!(location_label(Some("Route 4 shop"), "Heal"), "Route 4 shop");
        assert_eq!(location_label(Some(""), "Blackout"), "Blackout");
        assert_eq!(location_label(None, "Save"), "Save");
    }

    #[test]
    fn lcs_prefers_advancing_a() {
        let a: Vec<String> = ["x", "b", "c"].iter().map(|s| s.to_string()).collect();
        let b: Vec<String> = ["b", "y", "c"].iter().map(|s| s.to_string()).collect();
        assert_eq!(lcs(&a, &b), vec![(1, 0), (2, 2)]);
        assert!(lcs(&[], &b).is_empty());
    }

    #[test]
    fn merges_wild_levels_into_a_range() {
        let entry = |label: &str| Entry {
            kind: EntryKind::Wild,
            key: "w:poochyena".into(),
            label: label.into(),
            qty: 1,
            folder: String::new(),
            location: None,
            before: Snapshot::default(),
            xp_gain: 0,
            money_delta: 0,
            recorded_secs: None,
            is_major: false,
            fight_category: None,
            team_sig: None,
            enemy_count: 0,
            has_error: false,
        };
        assert_eq!(merge_label("Poochyena L5", &entry("Poochyena L6")), "Poochyena L5\u{2013}6");
        assert_eq!(merge_label("Poochyena L5\u{2013}6", &entry("Poochyena L3")), "Poochyena L3\u{2013}6");
        assert_eq!(merge_label("Poochyena L5", &entry("Poochyena L5")), "Poochyena L5");
    }
}
