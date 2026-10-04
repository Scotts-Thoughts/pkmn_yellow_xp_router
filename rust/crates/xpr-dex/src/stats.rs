//! Stat formulas (Solodex `utils/damage/stats.ts`, verified there against the
//! decomps by `verify:stats`).

use serde::{Deserialize, Serialize};

use crate::model::BaseStats;

/// Gen 1-2 DVs (0-15). The HP DV is derived from the others' parity bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gen12Dvs {
    pub attack: i32,
    pub defense: i32,
    pub speed: i32,
    pub special: i32,
}

impl Default for Gen12Dvs {
    fn default() -> Self {
        Gen12Dvs { attack: 15, defense: 15, speed: 15, special: 15 }
    }
}

/// Gen 1-2 Stat Experience (0-65535); `special` covers SpA and SpD.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gen12StatExps {
    pub hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub speed: i32,
    pub special: i32,
}

/// Gen 3+ IVs (0-31) or EVs (0-252), one per stat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gen3Spread {
    pub hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub spattack: i32,
    pub spdefense: i32,
    pub speed: i32,
}

impl Gen3Spread {
    pub const MAX_IVS: Gen3Spread = Gen3Spread { hp: 31, attack: 31, defense: 31, spattack: 31, spdefense: 31, speed: 31 };
    pub const ZERO: Gen3Spread = Gen3Spread { hp: 0, attack: 0, defense: 0, spattack: 0, spdefense: 0, speed: 0 };
}

/// Per-stat nature multipliers (1.1 / 0.9 / 1.0; HP is never affected).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct NatureMods {
    pub attack: f64,
    pub defense: f64,
    pub spattack: f64,
    pub spdefense: f64,
    pub speed: f64,
}

pub const NEUTRAL_NATURE: NatureMods = NatureMods { attack: 1.0, defense: 1.0, spattack: 1.0, spdefense: 1.0, speed: 1.0 };

/// Multipliers from a nature's increased / decreased stat keys as stored in
/// natures.json (`attack`, `defense`, `speed`, `specialAttack`, `specialDefense`).
pub fn nature_mods(increased: Option<&str>, decreased: Option<&str>) -> NatureMods {
    let mut m = NEUTRAL_NATURE;
    let (Some(inc), Some(dec)) = (increased, decreased) else { return m };
    if inc == dec {
        return m;
    }
    let slot = |m: &mut NatureMods, k: &str, v: f64| match k {
        "attack" => m.attack = v,
        "defense" => m.defense = v,
        "speed" => m.speed = v,
        "specialAttack" => m.spattack = v,
        "specialDefense" => m.spdefense = v,
        _ => {}
    };
    slot(&mut m, inc, 1.1);
    slot(&mut m, dec, 0.9);
    m
}

/// The multipliers of a nature by name (neutral when unknown).
pub fn nature_mods_by_name(name: &str) -> NatureMods {
    match crate::natures::nature(name) {
        Some(n) => nature_mods(n.increased.as_deref(), n.decreased.as_deref()),
        None => NEUTRAL_NATURE,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalcStats {
    pub hp: i32,
    pub attack: i32,
    pub defense: i32,
    /// "Special" in gen 1 (same value as `spdefense`)
    pub spattack: i32,
    pub spdefense: i32,
    pub speed: i32,
}

/// HP_DV = (ATK & 1)<<3 | (DEF & 1)<<2 | (SPE & 1)<<1 | (SPC & 1)
pub fn derive_hp_dv(dvs: &Gen12Dvs) -> i32 {
    ((dvs.attack & 1) << 3) | ((dvs.defense & 1) << 2) | ((dvs.speed & 1) << 1) | (dvs.special & 1)
}

/// Gen 1-2: floor(((base + DV) x 2 + floor(min(255, ceil(sqrt(StatExp))) / 4)) x L / 100) + 5;
/// HP adds L + 10 instead of 5.
pub fn calc_gen12_stats(base: &BaseStats, level: i32, dvs: &Gen12Dvs, stat_exps: &Gen12StatExps) -> CalcStats {
    let hp_dv = derive_hp_dv(dvs);
    let bonus = |exp: i32| ((exp.max(0) as f64).sqrt().ceil().min(255.0) as i32) / 4;
    let core = |b: i32, dv: i32, exp: i32| (((b + dv) * 2 + bonus(exp)) * level).div_euclid(100);
    CalcStats {
        hp: core(base.hp, hp_dv, stat_exps.hp) + level + 10,
        attack: core(base.attack, dvs.attack, stat_exps.attack) + 5,
        defense: core(base.defense, dvs.defense, stat_exps.defense) + 5,
        spattack: core(base.special_attack, dvs.special, stat_exps.special) + 5,
        spdefense: core(base.special_defense, dvs.special, stat_exps.special) + 5,
        speed: core(base.speed, dvs.speed, stat_exps.speed) + 5,
    }
}

/// Gen 3+: floor((floor((2 x base + IV + floor(EV/4)) x L / 100) + 5) x nature);
/// HP = floor((2 x base + IV + floor(EV/4)) x L / 100) + L + 10 (Shedinja: 1).
pub fn calc_gen3_stats(base: &BaseStats, level: i32, ivs: &Gen3Spread, evs: &Gen3Spread, nature: &NatureMods, species: Option<&str>) -> CalcStats {
    let core = |b: i32, iv: i32, ev: i32| ((2 * b + iv + ev.div_euclid(4)) * level).div_euclid(100);
    let nat = |v: i32, m: f64| ((v as f64) * m).floor() as i32;
    CalcStats {
        hp: if species == Some("Shedinja") { 1 } else { core(base.hp, ivs.hp, evs.hp) + level + 10 },
        attack: nat(core(base.attack, ivs.attack, evs.attack) + 5, nature.attack),
        defense: nat(core(base.defense, ivs.defense, evs.defense) + 5, nature.defense),
        spattack: nat(core(base.special_attack, ivs.spattack, evs.spattack) + 5, nature.spattack),
        spdefense: nat(core(base.special_defense, ivs.spdefense, evs.spdefense) + 5, nature.spdefense),
        speed: nat(core(base.speed, ivs.speed, evs.speed) + 5, nature.speed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formulas() {
        // Pikachu, gen 1, L5, max DVs, no stat exp
        let base = BaseStats { hp: 35, attack: 55, defense: 30, speed: 90, special_attack: 50, special_defense: 50 };
        let s = calc_gen12_stats(&base, 5, &Gen12Dvs::default(), &Gen12StatExps::default());
        assert_eq!((s.hp, s.attack, s.defense, s.spattack, s.speed), (20, 12, 9, 11, 15));
        // 65535 stat exp caps the bonus at 63, not 64
        let max = Gen12StatExps { hp: 65535, attack: 65535, defense: 65535, speed: 65535, special: 65535 };
        let s = calc_gen12_stats(&base, 100, &Gen12Dvs::default(), &max);
        assert_eq!(s.attack, ((55 + 15) * 2 + 63) + 5);
        // Garchomp L100 252 Spe Jolly: 333
        let g = BaseStats { hp: 108, attack: 130, defense: 95, speed: 102, special_attack: 80, special_defense: 85 };
        let mut evs = Gen3Spread::ZERO;
        evs.speed = 252;
        let s = calc_gen3_stats(&g, 100, &Gen3Spread::MAX_IVS, &evs, &nature_mods(Some("speed"), Some("specialAttack")), None);
        assert_eq!(s.speed, 333);
        assert_eq!(s.hp, 357);
    }
}
