//! `find_kill` was restructured for speed (memo without the roll
//! multiplier, cheaper hashing, a power table). Its floating-point results
//! must stay bit-identical to the straight port of the Python search, which
//! is kept here as the reference.

use std::collections::HashMap;

use xpr_calc::damage::{find_kill, DamageRange};

mod reference {
    use super::*;

    fn fpow(base: i64, exp: i64) -> f64 {
        (base as f64).powf(exp as f64)
    }

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

    type Memo = HashMap<(i64, i64, i64, i64), f64>;

    fn percent_rolls_kill(num_non_crits: i64, damage_range: &DamageRange, num_crits: i64, crit_damage_range: &DamageRange, target_hp: i64, memo: &mut Memo) -> f64 {
        let num_kill_rolls = percent_rolls_kill_recursive(num_non_crits, damage_range, num_crits, crit_damage_range, target_hp, 1, 0, memo);
        100.0 * num_kill_rolls / fpow(damage_range.len(), num_non_crits + num_crits)
    }

    #[allow(clippy::too_many_arguments)]
    fn percent_rolls_kill_recursive(
        num_non_crits: i64,
        damage_range: &DamageRange,
        num_crits: i64,
        crit_damage_range: &DamageRange,
        target_hp: i64,
        num_roll_multiplier: i64,
        total_damage: i64,
        memo: &mut Memo,
    ) -> f64 {
        let key = (num_non_crits, num_crits, num_roll_multiplier, total_damage);
        if let Some(v) = memo.get(&key) {
            return *v;
        }
        let min_damage_left = num_non_crits * damage_range.min_damage + num_crits * crit_damage_range.min_damage;
        let max_damage_left = num_non_crits * damage_range.max_damage + num_crits * crit_damage_range.max_damage;

        if total_damage + min_damage_left >= target_hp {
            let result = (num_roll_multiplier as f64) * fpow(damage_range.len(), num_non_crits + num_crits);
            memo.insert(key, result);
            return result;
        } else if num_crits == 0 && num_non_crits == 0 {
            memo.insert(key, 0.0);
            return 0.0;
        } else if total_damage + max_damage_left < target_hp {
            memo.insert(key, 0.0);
            return 0.0;
        }

        let (next_range, next_non_crits, next_crits) = if num_crits > 0 {
            (crit_damage_range, num_non_crits, num_crits - 1)
        } else {
            (damage_range, num_non_crits - 1, num_crits)
        };
        let mut result = 0.0f64;
        for (next_damage, count) in &next_range.damage_vals {
            result += percent_rolls_kill_recursive(next_non_crits, damage_range, next_crits, crit_damage_range, target_hp, *count, total_damage + next_damage, memo);
        }
        result *= num_roll_multiplier as f64;
        memo.insert(key, result);
        result
    }

    #[allow(clippy::too_many_arguments)]
    pub fn find_kill(damage_range: &DamageRange, crit_damage_range: &DamageRange, crit_chance: f64, accuracy: f64, target_hp: i64, attack_depth: i64, percent_cutoff: f64, force_full_search: bool) -> Vec<(i64, f64)> {
        let mut result: Vec<(i64, f64)> = Vec::new();
        let min_possible = damage_range.min_damage.min(crit_damage_range.min_damage);
        let max_possible = damage_range.max_damage.max(crit_damage_range.max_damage);
        let mut highest_found_kill_pct = 0.0f64;
        let mut memo: Memo = HashMap::new();
        let mut hits_to_kill_table: HashMap<i64, f64> = HashMap::new();

        if (damage_range.len() <= 200 && (min_possible * attack_depth) > target_hp) || force_full_search {
            for cur_num_attacks in 1..=attack_depth {
                if (max_possible * cur_num_attacks) < target_hp {
                    continue;
                }
                let mut all_hits_kill_pct = 0.0f64;
                for cur_num_crits in 0..=cur_num_attacks {
                    let kill_percent = percent_rolls_kill(cur_num_attacks - cur_num_crits, damage_range, cur_num_crits, crit_damage_range, target_hp, &mut memo);
                    all_hits_kill_pct += kill_percent * comb_f64(cur_num_attacks, cur_num_crits) * crit_chance.powf(cur_num_crits as f64) * (1.0 - crit_chance).powf((cur_num_attacks - cur_num_crits) as f64);
                }
                hits_to_kill_table.insert(cur_num_attacks, all_hits_kill_pct);

                let mut cur_total_kill_pct = 0.0f64;
                for cur_num_hits in 1..=cur_num_attacks {
                    cur_total_kill_pct += hits_to_kill_table.get(&cur_num_hits).copied().unwrap_or(0.0) * comb_f64(cur_num_attacks, cur_num_hits) * accuracy.powf(cur_num_hits as f64) * (1.0 - accuracy).powf((cur_num_attacks - cur_num_hits) as f64);
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
}

/// xorshift64*: deterministic inputs without a dependency
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn same_bits(a: &[(i64, f64)], b: &[(i64, f64)]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.0 == y.0 && x.1.to_bits() == y.1.to_bits())
}

#[test]
fn restructured_search_is_bit_identical_to_the_reference() {
    let mut rng = Rng(0x1234_5678_9ABC_DEF1);
    let crit_chances = [1.0 / 16.0, 17.0 / 256.0, 1.0 / 8.0, 0.25, 0.5, 0.0];
    let accuracies = [1.0, 0.95, 0.9, 0.85, 0.7, 0.55, 0.3];
    let mut checked = 0;
    for case in 0..6000 {
        // roll spreads of gens 1/2 (39 rolls) and 3+ (16 rolls), plus the
        // fixed-damage tables and the min/max merge of wild fights
        let (min_r, max_r) = if case % 2 == 0 { (217, 255) } else { (85, 100) };
        let base = 1 + rng.below(if case % 7 == 0 { 40 } else { 400 }) as i64;
        let normal = match case % 5 {
            0 => DamageRange::single(base),
            1 => DamageRange::merge_min_max(Some(&DamageRange::from_rolls(base, min_r, max_r)), Some(&DamageRange::from_rolls(base + 20, min_r, max_r))).unwrap(),
            _ => DamageRange::from_rolls(base, min_r, max_r),
        };
        let crit_base = if case % 3 == 0 { base * 2 } else { (base * 3) / 2 };
        let crit = match case % 5 {
            0 => DamageRange::single(crit_base),
            _ => DamageRange::from_rolls(crit_base.max(1), min_r, max_r),
        };
        let hp = 1 + rng.below(if case % 11 == 0 { 40 } else { 800 }) as i64;
        let crit_chance = crit_chances[rng.below(crit_chances.len() as u64) as usize];
        let accuracy = accuracies[rng.below(accuracies.len() as u64) as usize];
        let depth = match case % 13 {
            0 => 5,
            1 => 30,
            _ => 20,
        };
        let force = case % 17 == 0;
        let expected = reference::find_kill(&normal, &crit, crit_chance, accuracy, hp, depth, 0.1, force);
        let actual = find_kill(&normal, &crit, crit_chance, accuracy, hp, depth, 0.1, force);
        assert!(
            same_bits(&expected, &actual),
            "case {}: hp {} depth {} force {} normal {:?} crit {:?}\n expected {:?}\n actual   {:?}",
            case,
            hp,
            depth,
            force,
            normal.damage_vals,
            crit.damage_vals,
            expected,
            actual
        );
        checked += 1;
    }
    assert_eq!(checked, 6000);
}
