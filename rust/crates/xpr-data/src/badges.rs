//! Port of the five `GenXBadgeList` classes as one struct driven by per-gen
//! tables. Every observable string (including the "Zephr" typo in the gen 2
//! and gen 4 short forms) is reproduced.

use std::sync::Arc;

use indexmap::IndexMap;

use crate::model::Gen;

/// Badge slot ids in the order the Python `to_string` methods list them.
const GEN1_SLOTS: [&str; 8] = ["boulder", "cascade", "thunder", "rainbow", "soul", "marsh", "volcano", "earth"];
const GEN2_SLOTS: [&str; 16] = [
    "zephyr", "hive", "plain", "fog", "storm", "mineral", "glacier", "rising", "boulder", "cascade", "thunder",
    "rainbow", "soul", "marsh", "volcano", "earth",
];
const GEN3_SLOTS: [&str; 16] = [
    "stone", "knuckle", "dynamo", "heat", "balance", "feather", "mind", "rain", "boulder", "cascade", "thunder",
    "rainbow", "soul", "marsh", "volcano", "earth",
];
const GEN4_SLOTS: [&str; 24] = [
    "coal", "forest", "cobble", "fen", "relic", "mine", "icicle", "beacon", "zephyr", "hive", "plain", "fog",
    "storm", "mineral", "glacier", "rising", "boulder", "cascade", "thunder", "rainbow", "soul", "marsh",
    "volcano", "earth",
];
const GEN5_SLOTS: [&str; 10] = ["trio", "basic", "toxic", "insect", "bolt", "quake", "jet", "freeze", "legend", "wave"];

fn slots(gen: Gen) -> &'static [&'static str] {
    match gen {
        Gen::One => &GEN1_SLOTS,
        Gen::Two => &GEN2_SLOTS,
        Gen::Three => &GEN3_SLOTS,
        Gen::Four => &GEN4_SLOTS,
        Gen::Five => &GEN5_SLOTS,
    }
}

/// Display name for a slot in the short (non-verbose) form. Note "Zephr".
fn short_name(slot: &str) -> &'static str {
    match slot {
        "boulder" => "Boulder",
        "cascade" => "Cascade",
        "thunder" => "Thunder",
        "rainbow" => "Rainbow",
        "soul" => "Soul",
        "marsh" => "Marsh",
        "volcano" => "Volcano",
        "earth" => "Earth",
        "zephyr" => "Zephr",
        "hive" => "Hive",
        "plain" => "Plain",
        "fog" => "Fog",
        "storm" => "Storm",
        "mineral" => "Mineral",
        "glacier" => "Glacier",
        "rising" => "Rising",
        "stone" => "Stone",
        "knuckle" => "Knuckle",
        "dynamo" => "Dynamo",
        "heat" => "Heat",
        "balance" => "Balance",
        "feather" => "Feather",
        "mind" => "Mind",
        "rain" => "Rain",
        "coal" => "Coal",
        "forest" => "Forest",
        "cobble" => "Cobble",
        "fen" => "Fen",
        "relic" => "Relic",
        "mine" => "Mine",
        "icicle" => "Icicle",
        "beacon" => "Beacon",
        "trio" => "Trio",
        "basic" => "Basic",
        "toxic" => "Toxic",
        "insect" => "Insect",
        "bolt" => "Bolt",
        "quake" => "Quake",
        "jet" => "Jet",
        "freeze" => "Freeze",
        "legend" => "Legend",
        "wave" => "Wave",
        _ => "",
    }
}

/// Display name in the verbose form (`Zephyr: True`), i.e. `attr.capitalize()`
/// for gen 5 and the hand-written labels for the others.
fn verbose_name(slot: &str) -> String {
    match slot {
        "zephyr" => "Zephyr".to_string(),
        other => {
            let mut c = other.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct BadgeList {
    pub gen: Gen,
    flags: u32,
    rewards: Arc<IndexMap<String, String>>,
}

impl PartialEq for BadgeList {
    fn eq(&self, other: &Self) -> bool {
        self.gen == other.gen && self.flags == other.flags
    }
}
impl Eq for BadgeList {}

impl BadgeList {
    pub fn new(gen: Gen, rewards: Arc<IndexMap<String, String>>) -> BadgeList {
        BadgeList { gen, flags: 0, rewards }
    }

    /// The badge-less list the crit paths construct (`GenXBadgeList(rewards)`).
    pub fn empty_like(&self) -> BadgeList {
        BadgeList {
            gen: self.gen,
            flags: 0,
            rewards: self.rewards.clone(),
        }
    }

    pub fn rewards(&self) -> &Arc<IndexMap<String, String>> {
        &self.rewards
    }

    fn slot_index(&self, slot: &str) -> Option<usize> {
        slots(self.gen).iter().position(|s| *s == slot)
    }

    pub fn has(&self, slot: &str) -> bool {
        match self.slot_index(slot) {
            Some(i) => self.flags & (1 << i) != 0,
            None => false,
        }
    }

    fn set(&mut self, slot: &str) {
        if let Some(i) = self.slot_index(slot) {
            self.flags |= 1 << i;
        }
    }

    /// `award_badge(trainer_name)`; returns an unchanged copy when the trainer
    /// awards nothing (Python returns `self`).
    pub fn award_badge(&self, trainer_name: &str) -> BadgeList {
        let Some(reward) = self.rewards.get(trainer_name) else {
            return self.clone();
        };
        let reward = reward.as_str();
        let mut result = self.clone();
        match self.gen {
            Gen::One => {
                if GEN1_SLOTS.contains(&reward) {
                    result.set(reward);
                } else {
                    return self.clone();
                }
            }
            Gen::Two | Gen::Three => {
                if slots(self.gen).contains(&reward) {
                    result.set(reward);
                } else {
                    return self.clone();
                }
            }
            Gen::Four => {
                // Sinnoh badges are stored in the Johto slots (Python quirk).
                let mapped = match reward {
                    "coal" => "zephyr",
                    "forest" => "hive",
                    "cobble" => "plain",
                    "fen" => "fog",
                    "relic" => "storm",
                    "mine" => "mineral",
                    "icicle" => "glacier",
                    "beacon" => "rising",
                    other => other,
                };
                // Only the Johto and Kanto slots are ever set; the Sinnoh slots
                // exist for display symmetry but `award_badge` never sets them.
                if GEN4_SLOTS[8..].contains(&mapped) {
                    result.set(mapped);
                } else {
                    return self.clone();
                }
            }
            Gen::Five => {
                if GEN5_SLOTS.contains(&reward) {
                    result.set(reward);
                } else {
                    return self.clone();
                }
            }
        }
        result
    }

    pub fn is_attack_boosted(&self) -> bool {
        match self.gen {
            Gen::One => self.has("boulder"),
            Gen::Two => self.has("zephyr"),
            Gen::Three => self.has("stone") || self.has("boulder"),
            Gen::Four | Gen::Five => false,
        }
    }

    pub fn is_defense_boosted(&self) -> bool {
        match self.gen {
            Gen::One => self.has("thunder"),
            Gen::Two => self.has("mineral"),
            Gen::Three => self.has("balance") || self.has("soul"),
            Gen::Four | Gen::Five => false,
        }
    }

    pub fn is_speed_boosted(&self) -> bool {
        match self.gen {
            Gen::One => self.has("soul"),
            Gen::Two => self.has("plain"),
            Gen::Three => self.has("dynamo") || self.has("thunder"),
            Gen::Four | Gen::Five => false,
        }
    }

    pub fn is_special_attack_boosted(&self) -> bool {
        match self.gen {
            Gen::One => self.has("volcano"),
            Gen::Two => self.has("glacier"),
            Gen::Three => self.has("mind") || self.has("volcano"),
            Gen::Four | Gen::Five => false,
        }
    }

    pub fn is_special_defense_boosted(&self) -> bool {
        match self.gen {
            Gen::One => self.has("volcano"),
            Gen::Two => self.has("glacier"),
            Gen::Three => self.has("mind") || self.has("volcano"),
            Gen::Four | Gen::Five => false,
        }
    }

    pub fn num_badges(&self) -> i64 {
        self.flags.count_ones() as i64
    }

    /// `to_string(verbose=False)`: `"Badges: A, B"`.
    pub fn to_short_string(&self) -> String {
        let names: Vec<&str> = slots(self.gen)
            .iter()
            .filter(|s| self.has(s))
            .map(|s| short_name(s))
            .collect();
        format!("Badges: {}", names.join(", "))
    }

    /// `to_string(verbose=True)` / `__repr__` / `str(badges)`.
    pub fn to_verbose_string(&self) -> String {
        let parts: Vec<String> = slots(self.gen)
            .iter()
            .map(|s| format!("{}: {}", verbose_name(s), if self.has(s) { "True" } else { "False" }))
            .collect();
        parts.join(", ")
    }

    pub fn to_string(&self, verbose: bool) -> String {
        if verbose {
            self.to_verbose_string()
        } else {
            self.to_short_string()
        }
    }

    /// All slot ids for this gen (display order) with their current state.
    pub fn slots_with_state(&self) -> Vec<(&'static str, bool)> {
        slots(self.gen).iter().map(|s| (*s, self.has(s))).collect()
    }

    /// Build a list with the given slots set (for the badge boost calculator UI).
    pub fn with_slots(&self, set: &[&str]) -> BadgeList {
        let mut r = self.empty_like();
        for s in set {
            r.set(s);
        }
        r
    }
}
