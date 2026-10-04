//! Pure, UI-free probability helpers for the Misc calculators (Solodex
//! `utils/hitProbability.ts`).
//!
//! The core model is the binomial distribution: N independent uses of a move,
//! each landing with probability p. Also here are the Gen 6+ accuracy / evasion
//! stage multipliers (effective-accuracy helper) and the 2-5 strike multi-hit
//! distributions (multi-hit move mode). Everything is a line-for-line port, so
//! the results match Solodex's.

use std::collections::BTreeMap;

/// `Math.max(min, Math.min(max, value))` (a NaN stays NaN, as in JS).
pub fn clamp(value: f64, min: f64, max: f64) -> f64 {
    js_max(min, js_min(max, value))
}

fn js_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.min(b)
    }
}

fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

/// `Math.round`: halves round towards +infinity (Rust's `round` goes away
/// from zero).
pub fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

// ---- binomial core ----------------------------------------------------------------------------

/// Numerically stable "n choose k": divides as it multiplies instead of using
/// factorials, and uses the symmetry C(n, k) = C(n, n - k).
pub fn combinations(n: i64, k: i64) -> f64 {
    if k < 0 || k > n {
        return 0.0;
    }
    if k == 0 || k == n {
        return 1.0;
    }
    let k = k.min(n - k);
    let mut result = 1.0_f64;
    for i in 0..k {
        result = (result * (n - i) as f64) / (i + 1) as f64;
    }
    result
}

/// P(X = k): exactly k hits out of n uses, each with hit probability p.
///
/// `powf(0, 0)` is 1, which is what is wanted at the extremes: with p = 1 the
/// (1 - p)^(n - k) term is 0^0 = 1 for k = n, and with p = 0 the p^k term is
/// 0^0 = 1 for k = 0. No special-casing needed.
pub fn binomial_pmf(k: i64, n: i64, p: f64) -> f64 {
    combinations(n, k) * p.powf(k as f64) * (1.0 - p).powf((n - k) as f64)
}

/// P(X >= k): at least k hits (summed directly to avoid off-by-one bugs).
pub fn at_least(k: i64, n: i64, p: f64) -> f64 {
    let mut sum = 0.0;
    for i in k..=n {
        sum += binomial_pmf(i, n, p);
    }
    sum
}

/// P(X <= k): at most k hits.
pub fn at_most(k: i64, n: i64, p: f64) -> f64 {
    let mut sum = 0.0;
    for i in 0..=k {
        sum += binomial_pmf(i, n, p);
    }
    sum
}

/// The full distribution: index i = P(X = i), for i from 0..=n.
pub fn distribution(n: i64, p: f64) -> Vec<f64> {
    (0..=n).map(|i| binomial_pmf(i, n, p)).collect()
}

pub fn expected_hits(n: i64, p: f64) -> f64 {
    n as f64 * p
}

pub fn variance(n: i64, p: f64) -> f64 {
    n as f64 * p * (1.0 - p)
}

pub fn std_deviation(n: i64, p: f64) -> f64 {
    variance(n, p).sqrt()
}

// ---- effective accuracy (Gen 6+) --------------------------------------------------------------

/// Gen 6+ accuracy / evasion combined stat-stage multiplier. The combined
/// stage is (your accuracy stages) - (target's evasion stages), clamped to
/// -6..=+6. The multiplier is (3 + s) / 3 for s >= 0 and 3 / (3 - s) for s < 0.
///
/// Gen 1-5 used a different evasion formula; this table is Gen 6+ only.
pub fn accuracy_stage_multiplier(stage: f64) -> f64 {
    let s = clamp(js_round(stage), -6.0, 6.0);
    if s >= 0.0 {
        (3.0 + s) / 3.0
    } else {
        3.0 / (3.0 - s)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AccuracyModifier {
    pub id: &'static str,
    pub label: &'static str,
    /// multiplicative factor applied to accuracy, e.g. 1.3 for Compound Eyes
    pub factor: f64,
    pub note: &'static str,
}

/// Common "other modifiers" (multiply in, then cap at 100%).
pub const ACCURACY_MODIFIERS: [AccuracyModifier; 5] = [
    AccuracyModifier {
        id: "compoundEyes",
        label: "Compound Eyes",
        factor: 1.3,
        note: "\u{00D7}1.3",
    },
    AccuracyModifier {
        id: "victoryStar",
        label: "Victory Star",
        factor: 1.1,
        note: "\u{00D7}1.1",
    },
    AccuracyModifier {
        id: "wideLens",
        label: "Wide Lens",
        factor: 1.1,
        note: "\u{00D7}1.1 held",
    },
    AccuracyModifier {
        id: "hustle",
        label: "Hustle",
        factor: 0.8,
        note: "\u{00D7}0.8 physical",
    },
    AccuracyModifier {
        id: "sandSnow",
        label: "Sand Veil / Snow Cloak",
        factor: 0.8,
        note: "\u{00D7}0.8 target in weather",
    },
];

#[derive(Clone, Debug, PartialEq)]
pub struct EffectiveAccuracyInput {
    /// 0-100
    pub base_accuracy: f64,
    /// -6..=+6 (attacker)
    pub accuracy_stages: f64,
    /// -6..=+6 (target)
    pub evasion_stages: f64,
    /// extra multiplicative modifiers
    pub factors: Vec<f64>,
    /// No Guard / Lock-On / Mind Reader: always hits
    pub always_hits: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EffectiveAccuracyResult {
    /// final accuracy %, capped at 100
    pub effective: f64,
    /// clamped -6..=+6
    pub combined_stage: f64,
    pub stage_multiplier: f64,
    /// true if the raw product exceeded 100
    pub capped_at_100: bool,
    pub always_hits: bool,
}

/// effective = base x stageMultiplier(accuracy - evasion) x product(modifiers),
/// capped at 100%. No Guard (`always_hits`) short-circuits to 100%.
pub fn effective_accuracy(input: &EffectiveAccuracyInput) -> EffectiveAccuracyResult {
    if input.always_hits {
        return EffectiveAccuracyResult {
            effective: 100.0,
            combined_stage: 0.0,
            stage_multiplier: 1.0,
            capped_at_100: false,
            always_hits: true,
        };
    }
    let combined_stage = clamp(
        js_round(input.accuracy_stages - input.evasion_stages),
        -6.0,
        6.0,
    );
    let stage_multiplier = accuracy_stage_multiplier(combined_stage);
    let mut acc = clamp(input.base_accuracy, 0.0, 100.0) * stage_multiplier;
    for f in &input.factors {
        acc *= f;
    }
    EffectiveAccuracyResult {
        effective: js_min(100.0, acc),
        combined_stage,
        stage_multiplier,
        capped_at_100: acc > 100.0,
        always_hits: false,
    }
}

// ---- multi-hit (2-5 strike) moves -------------------------------------------------------------
//
// A standard 2-5 hit move makes ONE accuracy check for the whole move; if it
// passes, the number of hits is rolled from a fixed distribution. This is a
// different probability model from the binomial one above.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MultiHitGen {
    Gen1To4,
    Gen5Plus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MultiHitModifier {
    None,
    SkillLink,
    LoadedDice,
}

/// Conditional per-roll hit-count distribution of a standard 2-5 hit move.
fn multi_hit_base(gen: MultiHitGen) -> [(u32, f64); 4] {
    match gen {
        MultiHitGen::Gen5Plus => [(2, 0.35), (3, 0.35), (4, 0.15), (5, 0.15)],
        MultiHitGen::Gen1To4 => [(2, 0.375), (3, 0.375), (4, 0.125), (5, 0.125)],
    }
}

/// The hit-count distribution GIVEN the single accuracy check passed.
///  - Skill Link: always the maximum (5 hits).
///  - Loaded Dice: guarantees >= 4 hits; rolls that would be 2 or 3 are
///    remapped to an even 4 / 5 split, so overall 50% / 50% for 4 / 5 (a Gen 9
///    item).
pub fn multi_hit_roll_distribution(
    gen: MultiHitGen,
    modifier: MultiHitModifier,
) -> BTreeMap<u32, f64> {
    match modifier {
        MultiHitModifier::SkillLink => BTreeMap::from([(5, 1.0)]),
        MultiHitModifier::LoadedDice => BTreeMap::from([(4, 0.5), (5, 0.5)]),
        MultiHitModifier::None => multi_hit_base(gen).into_iter().collect(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MultiHitOutcome {
    /// 0 (the accuracy check failed) or 2-5
    pub hits: u32,
    pub probability: f64,
}

/// The full outcome distribution including the single accuracy gate
/// (`accuracy` is 0-100), ordered by hit count.
pub fn multi_hit_outcomes(
    accuracy: f64,
    gen: MultiHitGen,
    modifier: MultiHitModifier,
) -> Vec<MultiHitOutcome> {
    let p = clamp(accuracy / 100.0, 0.0, 1.0);
    let roll = multi_hit_roll_distribution(gen, modifier);
    let mut outcomes = vec![MultiHitOutcome {
        hits: 0,
        probability: 1.0 - p,
    }];
    for (hits, pr) in roll {
        outcomes.push(MultiHitOutcome {
            hits,
            probability: p * pr,
        });
    }
    outcomes.sort_by_key(|o| o.hits);
    outcomes
}

/// E[hit count] given the move's single accuracy check passed.
pub fn expected_rolled_hits(gen: MultiHitGen, modifier: MultiHitModifier) -> f64 {
    multi_hit_roll_distribution(gen, modifier)
        .into_iter()
        .map(|(h, pr)| h as f64 * pr)
        .sum()
}

/// E[total hits] over the whole move = P(accuracy passes) x E[rolled hit count].
pub fn expected_total_hits(accuracy: f64, gen: MultiHitGen, modifier: MultiHitModifier) -> f64 {
    clamp(accuracy / 100.0, 0.0, 1.0) * expected_rolled_hits(gen, modifier)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    #[test]
    fn combinations_match_pascal() {
        assert_eq!(combinations(5, 0), 1.0);
        assert_eq!(combinations(5, 5), 1.0);
        assert_eq!(combinations(5, 2), 10.0);
        assert_eq!(combinations(5, 3), 10.0);
        assert_eq!(combinations(10, 4), 210.0);
        assert_eq!(combinations(50, 25), 126410606437752.0);
        assert_eq!(combinations(5, 6), 0.0);
        assert_eq!(combinations(5, -1), 0.0);
        assert_eq!(combinations(0, 0), 1.0);
    }

    #[test]
    fn pmf_of_a_fair_coin() {
        let d = distribution(5, 0.5);
        let expect = [1.0, 5.0, 10.0, 10.0, 5.0, 1.0];
        assert_eq!(d.len(), 6);
        for (got, e) in d.iter().zip(expect) {
            assert!(close(*got, e / 32.0), "{} vs {}", got, e / 32.0);
        }
    }

    #[test]
    fn distribution_sums_to_one() {
        for n in [1_i64, 2, 5, 17, 50] {
            for p in [0.0, 0.01, 0.3, 0.5, 0.7, 0.95, 1.0] {
                let s: f64 = distribution(n, p).iter().sum();
                assert!((s - 1.0).abs() < 1e-9, "n={} p={} sum={}", n, p, s);
            }
        }
    }

    #[test]
    fn extremes_are_exact() {
        // p = 1: always n hits; p = 0: always 0 hits (0^0 = 1)
        assert_eq!(binomial_pmf(5, 5, 1.0), 1.0);
        assert_eq!(binomial_pmf(4, 5, 1.0), 0.0);
        assert_eq!(binomial_pmf(0, 5, 0.0), 1.0);
        assert_eq!(binomial_pmf(1, 5, 0.0), 0.0);
        assert_eq!(at_least(5, 5, 1.0), 1.0);
        assert_eq!(at_most(0, 5, 0.0), 1.0);
    }

    #[test]
    fn cumulative_probabilities() {
        // 70% accuracy, 5 uses
        let (n, p) = (5, 0.7);
        assert!(close(binomial_pmf(3, n, p), 10.0 * 0.343 * 0.09));
        assert!(close(at_least(3, n, p), 0.83692));
        assert!(close(at_most(3, n, p), 1.0 - at_least(4, n, p)));
        // at least 0 is certain, at most n is certain
        assert!(close(at_least(0, n, p), 1.0));
        assert!(close(at_most(n, n, p), 1.0));
        // exactly k + at least k+1 = at least k
        for k in 0..n {
            assert!(close(
                binomial_pmf(k, n, p) + at_least(k + 1, n, p),
                at_least(k, n, p)
            ));
        }
    }

    #[test]
    fn moments() {
        assert_eq!(expected_hits(5, 0.7), 3.5);
        assert!(close(variance(5, 0.7), 5.0 * 0.7 * 0.3));
        assert!(close(std_deviation(10, 0.5), 2.5_f64.sqrt()));
        assert_eq!(std_deviation(5, 1.0), 0.0);
    }

    #[test]
    fn stage_multipliers() {
        assert_eq!(accuracy_stage_multiplier(0.0), 1.0);
        assert!(close(accuracy_stage_multiplier(1.0), 4.0 / 3.0));
        assert!(close(accuracy_stage_multiplier(6.0), 3.0));
        assert!(close(accuracy_stage_multiplier(-1.0), 3.0 / 4.0));
        assert!(close(accuracy_stage_multiplier(-6.0), 1.0 / 3.0));
        // clamped, and rounded like Math.round
        assert!(close(accuracy_stage_multiplier(9.0), 3.0));
        assert!(close(accuracy_stage_multiplier(-9.0), 1.0 / 3.0));
        assert!(close(accuracy_stage_multiplier(1.4), 4.0 / 3.0));
        assert_eq!(js_round(-2.5), -2.0);
        assert_eq!(js_round(2.5), 3.0);
        assert_eq!(js_round(-0.4), 0.0);
    }

    fn eff(
        base: f64,
        acc: f64,
        eva: f64,
        factors: &[f64],
        always: bool,
    ) -> EffectiveAccuracyResult {
        effective_accuracy(&EffectiveAccuracyInput {
            base_accuracy: base,
            accuracy_stages: acc,
            evasion_stages: eva,
            factors: factors.to_vec(),
            always_hits: always,
        })
    }

    #[test]
    fn effective_accuracy_combines_stage_and_modifiers() {
        let r = eff(100.0, 0.0, 0.0, &[], false);
        assert_eq!(
            (
                r.effective,
                r.combined_stage,
                r.stage_multiplier,
                r.capped_at_100
            ),
            (100.0, 0.0, 1.0, false)
        );
        // Thunder (70) at +1 accuracy: 70 x 4/3
        let r = eff(70.0, 1.0, 0.0, &[], false);
        assert!(close(r.effective, 70.0 * 4.0 / 3.0));
        // evasion cancels accuracy
        let r = eff(85.0, 2.0, 2.0, &[], false);
        assert!(close(r.effective, 85.0));
        // the combined stage is clamped to +-6
        let r = eff(50.0, 6.0, -6.0, &[], false);
        assert_eq!(r.combined_stage, 6.0);
        assert!(close(r.effective, 100.0) && r.capped_at_100);
        let r = eff(100.0, -6.0, 6.0, &[], false);
        assert_eq!(r.combined_stage, -6.0);
        assert!(close(r.effective, 100.0 / 3.0) && !r.capped_at_100);
        // Compound Eyes x Wide Lens on 60% (on 70% it would pass 100 and be capped)
        let r = eff(60.0, 0.0, 0.0, &[1.3, 1.1], false);
        assert!(close(r.effective, 60.0 * 1.3 * 1.1));
        // Hustle on a 100% move
        let r = eff(100.0, 0.0, 0.0, &[0.8], false);
        assert!(close(r.effective, 80.0));
        // capped at 100 but flagged
        let r = eff(90.0, 0.0, 0.0, &[1.3], false);
        assert_eq!(r.effective, 100.0);
        assert!(r.capped_at_100);
        // exactly 100 is not "capped"
        assert!(!eff(100.0, 0.0, 0.0, &[], false).capped_at_100);
        // an out-of-range base is clamped
        assert!(close(eff(150.0, 0.0, 0.0, &[], false).effective, 100.0));
        assert_eq!(eff(-5.0, 0.0, 0.0, &[], false).effective, 0.0);
    }

    #[test]
    fn no_guard_always_hits() {
        let r = eff(30.0, -6.0, 6.0, &[0.8], true);
        assert_eq!(
            (r.effective, r.combined_stage, r.stage_multiplier),
            (100.0, 0.0, 1.0)
        );
        assert!(r.always_hits && !r.capped_at_100);
    }

    #[test]
    fn modifier_table() {
        let ids: Vec<&str> = ACCURACY_MODIFIERS.iter().map(|m| m.id).collect();
        assert_eq!(
            ids,
            [
                "compoundEyes",
                "victoryStar",
                "wideLens",
                "hustle",
                "sandSnow"
            ]
        );
        assert_eq!(ACCURACY_MODIFIERS[0].factor, 1.3);
        assert_eq!(ACCURACY_MODIFIERS[3].factor, 0.8);
        assert_eq!(ACCURACY_MODIFIERS[2].note, "\u{00D7}1.1 held");
    }

    #[test]
    fn multi_hit_rolls() {
        use MultiHitGen::*;
        use MultiHitModifier::*;
        let hits = |g, m| {
            multi_hit_roll_distribution(g, m)
                .into_iter()
                .collect::<Vec<_>>()
        };
        assert_eq!(
            hits(Gen5Plus, None),
            vec![(2, 0.35), (3, 0.35), (4, 0.15), (5, 0.15)]
        );
        assert_eq!(
            hits(Gen1To4, None),
            vec![(2, 0.375), (3, 0.375), (4, 0.125), (5, 0.125)]
        );
        assert_eq!(hits(Gen5Plus, SkillLink), vec![(5, 1.0)]);
        assert_eq!(hits(Gen1To4, SkillLink), vec![(5, 1.0)]);
        assert_eq!(hits(Gen5Plus, LoadedDice), vec![(4, 0.5), (5, 0.5)]);
        // the expected hit counts people quote: 3.1 (Gen 5+), 3.0 (Gen 1-4), 5, 4.5
        assert!(close(expected_rolled_hits(Gen5Plus, None), 3.1));
        assert!(close(expected_rolled_hits(Gen1To4, None), 3.0));
        assert_eq!(expected_rolled_hits(Gen5Plus, SkillLink), 5.0);
        assert_eq!(expected_rolled_hits(Gen5Plus, LoadedDice), 4.5);
    }

    #[test]
    fn multi_hit_outcomes_include_the_accuracy_gate() {
        use MultiHitGen::*;
        use MultiHitModifier::*;
        let o = multi_hit_outcomes(90.0, Gen5Plus, None);
        assert_eq!(
            o.iter().map(|x| x.hits).collect::<Vec<_>>(),
            vec![0, 2, 3, 4, 5]
        );
        assert!(close(o[0].probability, 0.1));
        assert!(close(o[1].probability, 0.9 * 0.35));
        assert!(close(o[4].probability, 0.9 * 0.15));
        assert!(close(o.iter().map(|x| x.probability).sum::<f64>(), 1.0));
        // a 100% move never misses
        let o = multi_hit_outcomes(100.0, Gen1To4, None);
        assert_eq!(o[0].probability, 0.0);
        // a 0% move always does
        let o = multi_hit_outcomes(0.0, Gen5Plus, SkillLink);
        assert_eq!((o[0].hits, o[0].probability), (0, 1.0));
        assert_eq!(o.last().unwrap().probability, 0.0);
        // out-of-range accuracy is clamped to 0..=1
        assert_eq!(
            multi_hit_outcomes(250.0, Gen5Plus, None)[0].probability,
            0.0
        );
        assert_eq!(multi_hit_outcomes(-4.0, Gen5Plus, None)[0].probability, 1.0);
    }

    #[test]
    fn multi_hit_expected_total() {
        use MultiHitGen::*;
        use MultiHitModifier::*;
        assert!(close(expected_total_hits(90.0, Gen5Plus, None), 0.9 * 3.1));
        assert!(close(expected_total_hits(100.0, Gen5Plus, SkillLink), 5.0));
        assert_eq!(expected_total_hits(0.0, Gen5Plus, LoadedDice), 0.0);
        assert_eq!(expected_total_hits(300.0, Gen1To4, None), 3.0);
    }

    #[test]
    fn clamp_matches_js() {
        assert_eq!(clamp(5.0, 0.0, 3.0), 3.0);
        assert_eq!(clamp(-5.0, 0.0, 3.0), 0.0);
        assert_eq!(clamp(2.0, 0.0, 3.0), 2.0);
        assert!(clamp(f64::NAN, 0.0, 3.0).is_nan());
    }
}
