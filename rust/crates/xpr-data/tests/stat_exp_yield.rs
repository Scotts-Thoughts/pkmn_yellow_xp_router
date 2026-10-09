//! Gens 1-2 stat exp: one Special word. Gen 2 battles add the enemy's base
//! HP/Atk/Def/Spe/SpA (pokecrystal GiveExperiencePoints, divided among the
//! participants by .EvenlyDivideExpAmongParticipants), and both special
//! stats read that word (CalcMonStatC), so the Sp. Def slot mirrors Sp. Atk.

use std::path::PathBuf;
use xpr_data::Registry;

fn registry() -> Registry {
    let raw = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../raw_pkmn_data").canonicalize().unwrap();
    Registry::new(raw, PathBuf::new())
}

#[test]
fn gen2_special_stat_exp_is_base_sp_atk_in_both_slots() {
    let reg = registry();
    let crystal = reg.get_version("Crystal").unwrap();
    // Tentacool: base Sp. Atk 50, Sp. Def 100
    let base = crystal.pkmn_db().get_pkmn("Tentacool").unwrap().stats;
    assert_ne!(base.special_attack, base.special_defense);
    let y = crystal.get_stat_xp_yield("Tentacool", 1, None).unwrap();
    assert_eq!(y.special_attack, base.special_attack);
    assert_eq!(y.special_defense, base.special_attack);
    assert_eq!((y.hp, y.attack, y.defense, y.speed), (base.hp, base.attack, base.defense, base.speed));
    // split between two participants, floored
    let half = crystal.get_stat_xp_yield("Tentacool", 2, None).unwrap();
    assert_eq!(half.special_attack, base.special_attack / 2);
    assert_eq!(half.special_defense, base.special_attack / 2);
}

#[test]
fn gen1_special_stat_exp_fills_both_slots() {
    let reg = registry();
    let yellow = reg.get_version("Yellow").unwrap();
    let base = yellow.pkmn_db().get_pkmn("Tentacool").unwrap().stats;
    let y = yellow.get_stat_xp_yield("Tentacool", 1, None).unwrap();
    assert_eq!(y.special_attack, base.special_attack);
    assert_eq!(y.special_defense, base.special_attack);
}
