//! Port of `pkmn/damage_calc.py`: `DamageRange`, the kill-percentage
//! search and the generation-agnostic helpers.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

use indexmap::IndexMap;

use xpr_core::consts;
use xpr_data::model::{EnemyPkmn, Move};
use xpr_data::gen_data::TypeChart;

/// Python's `int(s)`: optional surrounding whitespace, optional sign,
/// digits (underscores allowed between digits).
pub fn py_int(s: &str) -> Option<i64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let cleaned: String = t.chars().filter(|c| *c != '_').collect();
    if t.contains("__") || t.starts_with('_') || t.ends_with('_') {
        return None;
    }
    cleaned.parse::<i64>().ok()
}

/// `str.isdigit()` on a Python str: all characters are digits and non-empty.
pub fn py_isdigit(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

#[derive(Clone, Debug, PartialEq)]
pub struct DamageRange {
    /// damage value -> number of rolls, in insertion order
    pub damage_vals: IndexMap<i64, i64>,
    pub min_damage: i64,
    pub max_damage: i64,
    pub size: i64,
    pub num_attacks: i64,
}

impl DamageRange {
    /// `DamageRange(damage_vals, num_attacks)`; `None` for an empty table
    /// (Python raises).
    pub fn new(damage_vals: &IndexMap<i64, i64>, num_attacks: i64) -> Option<DamageRange> {
        let mut result = DamageRange {
            damage_vals: IndexMap::new(),
            min_damage: 0,
            max_damage: 0,
            size: 0,
            num_attacks,
        };
        let mut first = true;
        for (dmg, count) in damage_vals {
            *result.damage_vals.entry(*dmg).or_insert(0) += count;
            result.size += count;
            if first || *dmg < result.min_damage {
                result.min_damage = *dmg;
            }
            if first || *dmg > result.max_damage {
                result.max_damage = *dmg;
            }
            first = false;
        }
        if first {
            return None;
        }
        Some(result)
    }

    /// `DamageRange({value: 1})`
    pub fn single(value: i64) -> DamageRange {
        let mut m = IndexMap::new();
        m.insert(value, 1);
        DamageRange::new(&m, 1).unwrap()
    }

    /// Build from an ordered list of (damage, count) pairs.
    pub fn from_pairs(pairs: &[(i64, i64)], num_attacks: i64) -> Option<DamageRange> {
        let mut m: IndexMap<i64, i64> = IndexMap::new();
        for (d, c) in pairs {
            *m.entry(*d).or_insert(0) += c;
        }
        DamageRange::new(&m, num_attacks)
    }

    /// The standard random-roll spread: `floor(temp * n / max_range)` with a
    /// minimum of 1 for every numerator in `min_range..=max_range`.
    pub fn from_rolls(temp: i64, min_range: i64, max_range: i64) -> DamageRange {
        let mut m: IndexMap<i64, i64> = IndexMap::new();
        for numerator in min_range..=max_range {
            let dmg = xpr_core::floor_div(temp * numerator, max_range).max(1);
            *m.entry(dmg).or_insert(0) += 1;
        }
        DamageRange::new(&m, 1).unwrap()
    }

    pub fn len(&self) -> i64 {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// `merge_Pokemon_min_max`
    pub fn merge_min_max(range_for_min: Option<&DamageRange>, range_for_max: Option<&DamageRange>) -> Option<DamageRange> {
        match (range_for_min, range_for_max) {
            (None, None) => None,
            (Some(a), None) => Some(a.clone()),
            (None, Some(b)) => Some(b.clone()),
            (Some(a), Some(b)) => {
                let mut m = IndexMap::new();
                m.insert(a.min_damage, 1);
                m.insert(b.max_damage, 1);
                DamageRange::new(&m, 1)
            }
        }
    }

    /// `add`: the joint distribution of two independent hits.
    pub fn add(&self, other: &DamageRange) -> DamageRange {
        let mut m: IndexMap<i64, i64> = IndexMap::new();
        for (my_dmg, my_count) in &self.damage_vals {
            for (your_dmg, your_count) in &other.damage_vals {
                *m.entry(my_dmg + your_dmg).or_insert(0) += my_count * your_count;
            }
        }
        DamageRange::new(&m, self.num_attacks + other.num_attacks).unwrap()
    }

    /// `split_kills(hp_threshold) -> (kills, non_kills)`
    pub fn split_kills(&self, hp_threshold: i64) -> (Option<DamageRange>, Option<DamageRange>) {
        if hp_threshold > self.max_damage {
            return (None, Some(self.clone()));
        } else if hp_threshold <= self.min_damage {
            return (Some(self.clone()), None);
        }
        let mut kill = IndexMap::new();
        let mut non_kill = IndexMap::new();
        for (d, c) in &self.damage_vals {
            if *d >= hp_threshold {
                kill.insert(*d, *c);
            } else {
                non_kill.insert(*d, *c);
            }
        }
        (
            DamageRange::new(&kill, self.num_attacks),
            DamageRange::new(&non_kill, self.num_attacks),
        )
    }

    /// `to_string(max_num, percent_of)`
    pub fn to_string(&self, max_num: Option<usize>, percent_of: Option<i64>) -> String {
        let mut keys: Vec<i64> = self.damage_vals.keys().copied().collect();
        keys.sort();
        let mut result: Vec<String> = keys
            .iter()
            .map(|d| match percent_of {
                Some(p) => {
                    let pct = ((*d as f64) / (p as f64)) * 100.0;
                    format!("{:.1} x{}", pct, self.damage_vals[d])
                }
                None => format!("{} x{}", d, self.damage_vals[d]),
            })
            .collect();
        if let Some(max) = max_num {
            if max > 1 && result.len() > max {
                let parts = max / 2;
                let mut trimmed: Vec<String> = result[..parts].to_vec();
                trimmed.push("...".to_string());
                trimmed.extend(result[result.len() - parts..].iter().cloned());
                result = trimmed;
            }
        }
        result.join(", ")
    }
}

/// `math.pow(a, b)` for the integer operands the search uses.
fn fpow(base: i64, exp: i64) -> f64 {
    (base as f64).powf(exp as f64)
}

/// `math.comb(n, k)` converted to float the way Python's `float * int` does.
fn comb_f64(n: i64, k: i64) -> f64 {
    if k < 0 || k > n {
        return 0.0;
    }
    let k = k.min(n - k);
    let mut acc: u128 = 1;
    let mut overflow = false;
    for i in 0..k {
        match acc.checked_mul((n - i) as u128) {
            Some(v) => acc = v / (i as u128 + 1),
            None => {
                overflow = true;
                break;
            }
        }
    }
    if !overflow {
        return acc as f64;
    }
    let mut f = 1.0f64;
    for i in 0..k {
        f = f * ((n - i) as f64) / ((i + 1) as f64);
    }
    f
}

/// Multiplicative hashing for the packed memo keys: the search does tens of
/// thousands of lookups per fight, which made SipHash the dominant cost.
#[derive(Default, Clone, Copy)]
struct KeyHasher(u64);

impl Hasher for KeyHasher {
    fn finish(&self) -> u64 {
        // fold the well-mixed high half into the bucket-index bits
        self.0 ^ (self.0 >> 32)
    }

    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.write_u64(*b as u64);
        }
    }

    fn write_u64(&mut self, v: u64) {
        self.0 = (self.0.rotate_left(5) ^ v).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
}

/// The kill-roll counts of `percent_rolls_kill_recursive`, memoized per
/// `(non-crits left, crits left, damage dealt so far)`.
///
/// Python keys its memo by the roll multiplier as well, but the multiplier
/// only scales the result (`f(n, c, m, t) = m * h(n, c, t)`), so it is
/// applied by the caller instead. The float operations are the same ones in
/// the same order, so the numbers are bit-identical.
pub struct KillSearch<'a> {
    damage_range: &'a DamageRange,
    crit_damage_range: &'a DamageRange,
    target_hp: i64,
    /// `len(damage_range) ** k`, computed the way Python does, once per k
    len_pow: Vec<f64>,
    memo: HashMap<u64, f64, BuildHasherDefault<KeyHasher>>,
}

impl<'a> KillSearch<'a> {
    pub fn new(damage_range: &'a DamageRange, crit_damage_range: &'a DamageRange, target_hp: i64, attack_depth: i64) -> KillSearch<'a> {
        let len_pow = (0..=attack_depth.max(0)).map(|k| fpow(damage_range.len(), k)).collect();
        KillSearch { damage_range, crit_damage_range, target_hp, len_pow, memo: HashMap::default() }
    }

    fn len_pow(&self, k: i64) -> f64 {
        match self.len_pow.get(k as usize) {
            Some(v) => *v,
            None => fpow(self.damage_range.len(), k),
        }
    }

    /// `percent_rolls_kill`
    pub fn percent_rolls_kill(&mut self, num_non_crits: i64, num_crits: i64) -> f64 {
        // Python: `1 * h` for the top-level multiplier of 1 (exact)
        let num_kill_rolls = 1.0 * self.kill_rolls(num_non_crits, num_crits, 0);
        100.0 * num_kill_rolls / self.len_pow(num_non_crits + num_crits)
    }

    /// The (weighted) number of roll combinations of the remaining hits
    /// that bring `total_damage` to the target HP.
    fn kill_rolls(&mut self, num_non_crits: i64, num_crits: i64, total_damage: i64) -> f64 {
        let key = ((num_non_crits as u64) << 48) ^ ((num_crits as u64) << 32) ^ (total_damage as u64 & 0xFFFF_FFFF);
        if let Some(v) = self.memo.get(&key) {
            return *v;
        }
        let min_damage_left = num_non_crits * self.damage_range.min_damage + num_crits * self.crit_damage_range.min_damage;
        let max_damage_left = num_non_crits * self.damage_range.max_damage + num_crits * self.crit_damage_range.max_damage;

        let result = if total_damage + min_damage_left >= self.target_hp {
            self.len_pow(num_non_crits + num_crits)
        } else if (num_crits == 0 && num_non_crits == 0) || total_damage + max_damage_left < self.target_hp {
            0.0
        } else {
            let (next_range, next_non_crits, next_crits) = if num_crits > 0 {
                (self.crit_damage_range, num_non_crits, num_crits - 1)
            } else {
                (self.damage_range, num_non_crits - 1, num_crits)
            };
            let mut result = 0.0f64;
            for (next_damage, count) in &next_range.damage_vals {
                // the child's own multiplier, applied where Python applies it
                result += (*count as f64) * self.kill_rolls(next_non_crits, next_crits, total_damage + next_damage);
            }
            result
        };
        self.memo.insert(key, result);
        result
    }
}

/// `percent_rolls_kill` on a fresh search (one call; see [`KillSearch`]).
pub fn percent_rolls_kill(num_non_crits: i64, damage_range: &DamageRange, num_crits: i64, crit_damage_range: &DamageRange, target_hp: i64) -> f64 {
    KillSearch::new(damage_range, crit_damage_range, target_hp, num_non_crits + num_crits).percent_rolls_kill(num_non_crits, num_crits)
}

/// One entry of the kill-range list: `(number of attacks, percent)`, where
/// a percent of `-1.0` marks the guaranteed-but-unquantified kill.
pub type KillRange = (i64, f64);

/// `find_kill`
pub fn find_kill(
    damage_range: &DamageRange,
    crit_damage_range: &DamageRange,
    crit_chance: f64,
    accuracy: f64,
    target_hp: i64,
    attack_depth: i64,
    percent_cutoff: f64,
    force_full_search: bool,
) -> Vec<KillRange> {
    let mut result: Vec<KillRange> = Vec::new();
    let min_possible = damage_range.min_damage.min(crit_damage_range.min_damage);
    let max_possible = damage_range.max_damage.max(crit_damage_range.max_damage);
    let mut highest_found_kill_pct = 0.0f64;
    let mut search = KillSearch::new(damage_range, crit_damage_range, target_hp, attack_depth);
    // hits_to_kill_table[n] for n in 1..=attack_depth (index 0 unused)
    let mut hits_to_kill_table: Vec<f64> = vec![0.0; (attack_depth.max(0) as usize) + 1];

    if (damage_range.len() <= 200 && (min_possible * attack_depth) > target_hp) || force_full_search {
        for cur_num_attacks in 1..=attack_depth {
            if (max_possible * cur_num_attacks) < target_hp {
                continue;
            }
            let mut all_hits_kill_pct = 0.0f64;
            for cur_num_crits in 0..=cur_num_attacks {
                let kill_percent = search.percent_rolls_kill(cur_num_attacks - cur_num_crits, cur_num_crits);
                all_hits_kill_pct += kill_percent
                    * comb_f64(cur_num_attacks, cur_num_crits)
                    * crit_chance.powf(cur_num_crits as f64)
                    * (1.0 - crit_chance).powf((cur_num_attacks - cur_num_crits) as f64);
            }
            hits_to_kill_table[cur_num_attacks as usize] = all_hits_kill_pct;

            let mut cur_total_kill_pct = 0.0f64;
            for cur_num_hits in 1..=cur_num_attacks {
                cur_total_kill_pct += hits_to_kill_table[cur_num_hits as usize]
                    * comb_f64(cur_num_attacks, cur_num_hits)
                    * accuracy.powf(cur_num_hits as f64)
                    * (1.0 - accuracy).powf((cur_num_attacks - cur_num_hits) as f64);
            }
            highest_found_kill_pct = cur_total_kill_pct;
            if cur_total_kill_pct > percent_cutoff {
                result.push((cur_num_attacks, cur_total_kill_pct));
            }
            if cur_total_kill_pct > 99.0 {
                break;
            }
        }
    }

    let rounded_acc = (accuracy * 100.0).round_ties_even();
    if highest_found_kill_pct < 99.0 && highest_found_kill_pct < rounded_acc && damage_range.min_damage != 0 {
        let guaranteed = ((target_hp as f64) / (damage_range.min_damage as f64)).ceil() as i64;
        result.push((guaranteed, -1.0));
    }
    result
}

/// The per-hit structure of one use of a move whose hits roll damage (and,
/// from gen 2 on, critical hits) independently: multi-hit moves, two-hit
/// moves, Beat Up and Triple Kick. Gen 1 rolls once for every hit, so it
/// never produces one of these.
#[derive(Clone, Debug, PartialEq)]
pub struct HitModel {
    /// `(non-crit range, crit range)` of every hit slot, in order.
    pub hits: Vec<(DamageRange, DamageRange)>,
    /// Accuracy is rolled before every hit and a miss ends the sequence
    /// (gen 3/4 Triple Kick); otherwise one roll covers the whole use.
    pub accuracy_per_hit: bool,
}

impl HitModel {
    /// Sum of the smallest non-crit values of every hit.
    pub fn min_damage(&self) -> i64 {
        self.hits.iter().map(|(n, _)| n.min_damage).sum()
    }
}

/// Dense probability mass of a `DamageRange` over `0..=cap`, with every
/// value of `cap` or more collapsed into the `cap` bucket.
fn capped_pmf(range: &DamageRange, cap: usize) -> Vec<f64> {
    let mut out = vec![0.0f64; cap + 1];
    let total = range.size as f64;
    for (dmg, count) in &range.damage_vals {
        let idx = (*dmg).clamp(0, cap as i64) as usize;
        out[idx] += (*count as f64) / total;
    }
    out
}

/// `a ⊛ b` with every sum of `cap` or more collapsed into the `cap` bucket.
fn capped_convolve(a: &[f64], b: &[f64], cap: usize) -> Vec<f64> {
    let mut out = vec![0.0f64; cap + 1];
    for (i, pa) in a.iter().enumerate() {
        if *pa == 0.0 {
            continue;
        }
        for (j, pb) in b.iter().enumerate() {
            if *pb == 0.0 {
                continue;
            }
            let k = (i + j).min(cap);
            out[k] += pa * pb;
        }
    }
    out
}

/// `find_kill` for a move whose hits roll crits and damage independently
/// (see [`HitModel`]): every hit crits with `crit_chance` on its own, and
/// the damage of one use is the convolution of its hits. The result has the
/// same shape and semantics as [`find_kill`] (per-use kill percentages, the
/// `-1.0` guaranteed-kill entry).
pub fn find_kill_hits(
    model: &HitModel,
    crit_chance: f64,
    accuracy: f64,
    target_hp: i64,
    attack_depth: i64,
    percent_cutoff: f64,
) -> Vec<KillRange> {
    let mut result: Vec<KillRange> = Vec::new();
    if target_hp <= 0 || model.hits.is_empty() {
        return result;
    }
    let cap = target_hp as usize;
    let mut delta0 = vec![0.0f64; cap + 1];
    delta0[0] = 1.0;

    // one use
    let mut landed = delta0.clone();
    let mut ended = vec![0.0f64; cap + 1];
    for (normal, crit) in &model.hits {
        let pn = capped_pmf(normal, cap);
        let pc = capped_pmf(crit, cap);
        let mix: Vec<f64> = pn.iter().zip(pc.iter()).map(|(n, c)| n * (1.0 - crit_chance) + c * crit_chance).collect();
        if model.accuracy_per_hit {
            for (e, l) in ended.iter_mut().zip(landed.iter()) {
                *e += l * (1.0 - accuracy);
            }
            let hitting: Vec<f64> = landed.iter().map(|l| l * accuracy).collect();
            landed = capped_convolve(&hitting, &mix, cap);
        } else {
            landed = capped_convolve(&landed, &mix, cap);
        }
    }
    let use_dist: Vec<f64> = if model.accuracy_per_hit {
        ended.iter().zip(landed.iter()).map(|(e, l)| e + l).collect()
    } else {
        let mut d: Vec<f64> = landed.iter().map(|l| l * accuracy).collect();
        d[0] += 1.0 - accuracy;
        d
    };

    let mut total = delta0;
    let mut highest_found_kill_pct = 0.0f64;
    for cur_num_attacks in 1..=attack_depth.max(0) {
        total = capped_convolve(&total, &use_dist, cap);
        let pct = 100.0 * total[cap];
        highest_found_kill_pct = pct;
        if pct > percent_cutoff {
            result.push((cur_num_attacks, pct));
        }
        if pct > 99.0 {
            break;
        }
    }
    let rounded_acc = (accuracy * 100.0).round_ties_even();
    let min_damage = model.min_damage();
    if highest_found_kill_pct < 99.0 && highest_found_kill_pct < rounded_acc && min_damage != 0 {
        let guaranteed = ((target_hp as f64) / (min_damage as f64)).ceil() as i64;
        result.push((guaranteed, -1.0));
    }
    result
}

/// `get_special_damage_override`: Dragon Rage / Sonic Boom / Seismic Toss /
/// Night Shade. `defender_types` enables the gen 2+ immunity check.
pub fn get_special_damage_override(
    mv: &Move,
    attacking_pkmn: &EnemyPkmn,
    defender_types: Option<(&str, &str)>,
    type_chart: Option<&TypeChart>,
) -> Option<DamageRange> {
    let amount = match mv.name.as_str() {
        consts::DRAGON_RAGE_MOVE_NAME => Some(40),
        "SonicBoom" | "Sonic Boom" => Some(20),
        "Seismic Toss" | "Night Shade" => Some(attacking_pkmn.level),
        _ => None,
    }?;
    if let (Some((t1, t2)), Some(chart)) = (defender_types, type_chart) {
        let row = chart.get(&mv.move_type);
        let eff = |t: &str| row.and_then(|r| r.get(t)).map(|s| s.as_str());
        if eff(t1) == Some(consts::IMMUNE) || eff(t2) == Some(consts::IMMUNE) {
            return None;
        }
    }
    Some(DamageRange::single(amount))
}

/// `is_weather_active`
pub fn is_weather_active(attacking_ability: &str, defending_ability: &str, weather: &str) -> bool {
    if weather == consts::WEATHER_NONE {
        return false;
    }
    !consts::WEATHER_SUPPRESSING_ABILITIES.contains(&attacking_ability)
        && !consts::WEATHER_SUPPRESSING_ABILITIES.contains(&defending_ability)
}

/// `get_weather_ball_type`
pub fn get_weather_ball_type<'a>(weather: &str, weather_is_active: bool, default_type: &'a str) -> &'a str {
    if !weather_is_active {
        return default_type;
    }
    match consts::weather_ball_type_for(weather) {
        Some(t) => {
            // the returned type is a static string; hand back the caller's
            // lifetime through a static lookup
            static_type(t).unwrap_or(default_type)
        }
        None => default_type,
    }
}

fn static_type(t: &str) -> Option<&'static str> {
    for candidate in [
        consts::TYPE_FIRE,
        consts::TYPE_WATER,
        consts::TYPE_ICE,
        consts::TYPE_ROCK,
        consts::TYPE_NORMAL,
    ] {
        if candidate == t {
            return Some(candidate);
        }
    }
    None
}

/// `apply_forecast`: the (first, second) types of a species after Castform's
/// Forecast retyping.
pub fn apply_forecast(first_type: &str, second_type: &str, ability: &str, weather: &str, weather_is_active: bool) -> (String, String) {
    if !weather_is_active || ability != consts::FORECAST_ABILITY {
        return (first_type.to_string(), second_type.to_string());
    }
    match consts::forecast_type(weather) {
        Some(t) => (t.to_string(), t.to_string()),
        None => (first_type.to_string(), second_type.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_basics() {
        let r = DamageRange::from_rolls(100, 217, 255);
        assert_eq!(r.min_damage, 85);
        assert_eq!(r.max_damage, 100);
        assert_eq!(r.len(), 39);
        let s = r.add(&r);
        assert_eq!(s.min_damage, 170);
        assert_eq!(s.num_attacks, 2);
        assert_eq!(s.len(), 39 * 39);
    }

    #[test]
    fn find_kill_basic() {
        let r = DamageRange::from_rolls(20, 217, 255);
        let c = DamageRange::from_rolls(40, 217, 255);
        let kills = find_kill(&r, &c, 17.0 / 256.0, 1.0, 50, 20, 0.1, false);
        assert!(!kills.is_empty());
        assert_eq!(kills[0].0, 2);
        assert!(kills.last().unwrap().1 > 99.0 || kills.last().unwrap().1 == -1.0);
    }

    #[test]
    fn hit_model_matches_single_hit_search() {
        // one hit per use with no crit: identical to find_kill on the same range
        let r = DamageRange::from_rolls(20, 217, 255);
        let model = HitModel { hits: vec![(r.clone(), r.clone())], accuracy_per_hit: false };
        let a = find_kill(&r, &r, 0.0, 0.9, 50, 20, 0.1, true);
        let b = find_kill_hits(&model, 0.0, 0.9, 50, 20, 0.1);
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.0, y.0);
            assert!((x.1 - y.1).abs() < 1e-9, "{:?} vs {:?}", x, y);
        }
    }

    #[test]
    fn hit_model_two_hits_crit_independently() {
        // two hits of exactly 10 (crit 20) vs 30 HP: a kill needs at least one crit
        let n = DamageRange::single(10);
        let c = DamageRange::single(20);
        let model = HitModel { hits: vec![(n.clone(), c.clone()), (n, c)], accuracy_per_hit: false };
        let kills = find_kill_hits(&model, 0.25, 1.0, 30, 5, 0.0);
        assert_eq!(kills[0].0, 1);
        assert!((kills[0].1 - 43.75).abs() < 1e-9, "{:?}", kills);
    }

    #[test]
    fn pyint() {
        assert_eq!(py_int(" 12 "), Some(12));
        assert_eq!(py_int("5 + DefenseCurl"), None);
        assert_eq!(py_int(""), None);
    }
}
