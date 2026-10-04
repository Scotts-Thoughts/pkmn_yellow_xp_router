//! The outspeed maths of the Stats tab (Solodex `StatsView.tsx`'s
//! `speedAtLevel` / `minLevelToOutspeed`).

use xpr_dex::model::BaseStats;
use xpr_dex::stats::{calc_gen12_stats, calc_gen3_stats, Gen12Dvs, Gen12StatExps, Gen3Spread, NatureMods};

/// Max trainable speed investment: Stat Exp in Gen 1-2, EVs in Gen 3+.
pub const MAX_STATEXP: i32 = 65535;
pub const MAX_EV: i32 = 252;

/// Everything the stat formulas take besides the base stats and level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spread {
    pub dvs: Gen12Dvs,
    pub stat_exps: Gen12StatExps,
    pub ivs: Gen3Spread,
    pub evs: Gen3Spread,
    pub nature: NatureMods,
}

/// The maximum speed investment of a generation.
pub fn max_invest(gen: u8) -> i32 {
    if gen <= 2 {
        MAX_STATEXP
    } else {
        MAX_EV
    }
}

/// The player's own speed investment (Stat Exp or EVs).
pub fn user_invest(gen: u8, s: &Spread) -> i32 {
    if gen <= 2 {
        s.stat_exps.speed
    } else {
        s.evs.speed
    }
}

/// A Pokemon's Speed at `level`, overriding only the speed EV / Stat Exp
/// (the other stats are irrelevant). Generation-aware.
pub fn speed_at_level(base: &BaseStats, gen: u8, level: i32, speed_investment: i32, s: &Spread) -> i32 {
    if gen <= 2 {
        calc_gen12_stats(base, level, &s.dvs, &Gen12StatExps { speed: speed_investment, ..s.stat_exps }).speed
    } else {
        calc_gen3_stats(base, level, &s.ivs, &Gen3Spread { speed: speed_investment, ..s.evs }, &s.nature, None).speed
    }
}

/// The lowest level (1-100) at which the player strictly outspeeds
/// `target_speed`, or `None` if Lv100 still is not enough. A tie does not count
/// as outspeeding.
pub fn min_level_to_outspeed(base: &BaseStats, gen: u8, target_speed: i64, speed_investment: i32, s: &Spread) -> Option<i32> {
    (1..=100).find(|&lvl| speed_at_level(base, gen, lvl, speed_investment, s) as i64 > target_speed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use xpr_dex::stats::{nature_mods, NEUTRAL_NATURE};

    fn spread(nature: NatureMods) -> Spread {
        Spread { dvs: Gen12Dvs::default(), stat_exps: Gen12StatExps::default(), ivs: Gen3Spread::MAX_IVS, evs: Gen3Spread::ZERO, nature }
    }

    const GARCHOMP: BaseStats = BaseStats { hp: 108, attack: 130, defense: 95, speed: 102, special_attack: 80, special_defense: 85 };
    const PIKACHU: BaseStats = BaseStats { hp: 35, attack: 55, defense: 30, speed: 90, special_attack: 50, special_defense: 50 };

    #[test]
    fn gen3_speed_uses_the_speed_investment_and_nature() {
        let jolly = nature_mods(Some("speed"), Some("specialAttack"));
        assert_eq!(speed_at_level(&GARCHOMP, 4, 100, 252, &spread(jolly)), 333);
        // no EVs, neutral nature: floor((2*102 + 31) * 100 / 100) + 5 = 240
        assert_eq!(speed_at_level(&GARCHOMP, 4, 100, 0, &spread(NEUTRAL_NATURE)), 240);
        // only the speed EV is overridden: the other EVs in the spread do not matter
        let mut s = spread(NEUTRAL_NATURE);
        s.evs = Gen3Spread { speed: 4, attack: 252, ..Gen3Spread::ZERO };
        assert_eq!(speed_at_level(&GARCHOMP, 4, 100, 252, &s), 240 + 63);
    }

    #[test]
    fn gen12_speed_uses_the_speed_stat_exp() {
        let s = spread(NEUTRAL_NATURE);
        // L50, max DVs, no stat exp: floor((90+15)*2*50/100) + 5 = 110
        assert_eq!(speed_at_level(&PIKACHU, 1, 50, 0, &s), 110);
        // 65535 stat exp adds 63 to the inner sum: floor((210 + 63) * 50 / 100) + 5 = 141
        assert_eq!(speed_at_level(&PIKACHU, 1, 50, 65535, &s), 141);
        // the 'spe' DV (not the others) matters
        let mut low = s;
        low.dvs.speed = 0;
        assert_eq!(speed_at_level(&PIKACHU, 1, 50, 0, &low), 95);
    }

    #[test]
    fn outspeed_is_strict_and_none_when_out_of_reach() {
        let s = spread(NEUTRAL_NATURE);
        let at = |l| speed_at_level(&PIKACHU, 1, l, 0, &s) as i64;
        // a tie does not count
        let l = min_level_to_outspeed(&PIKACHU, 1, at(40), 0, &s).unwrap();
        assert!(at(l) > at(40) && at(l - 1) <= at(40), "level {}", l);
        // a target below the level 1 speed is beaten at level 1
        assert_eq!(min_level_to_outspeed(&PIKACHU, 1, 0, 0, &s), Some(1));
        // nothing at level 100 beats a speed above its own
        assert_eq!(min_level_to_outspeed(&PIKACHU, 1, at(100), 0, &s), None);
        assert_eq!(min_level_to_outspeed(&PIKACHU, 1, 10_000, 65535, &s), None);
        // more investment never needs a higher level
        for target in [30, 80, 150, 200] {
            let hi = min_level_to_outspeed(&GARCHOMP, 4, target, 252, &s);
            let lo = min_level_to_outspeed(&GARCHOMP, 4, target, 0, &s);
            match (hi, lo) {
                (Some(h), Some(l)) => assert!(h <= l),
                (None, Some(_)) => panic!("more investment must not be worse"),
                _ => {}
            }
        }
    }

    #[test]
    fn investment_caps_per_generation() {
        let s = spread(NEUTRAL_NATURE);
        assert_eq!(max_invest(1), 65535);
        assert_eq!(max_invest(2), 65535);
        assert_eq!(max_invest(3), 252);
        assert_eq!(max_invest(9), 252);
        let mut s2 = s;
        s2.stat_exps.speed = 1234;
        s2.evs.speed = 56;
        assert_eq!(user_invest(1, &s2), 1234);
        assert_eq!(user_invest(5, &s2), 56);
    }
}
