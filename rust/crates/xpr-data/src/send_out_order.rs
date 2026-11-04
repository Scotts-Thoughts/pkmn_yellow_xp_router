//! The order a trainer's AI sends out its Pokémon against the player's one
//! Pokémon (a solo run): a port of the unified frontend's
//! `core/logic/sendOutOrder.ts` and `typeMatchups.ts`. The rules, with the
//! decomp citations, are written up in that repo's `docs/send-out-order.md`.
//!
//! Every generation leads with party slot 0.
//!
//! * Gen 1 (pokered `EnemySendOut`): the first living Pokémon, so party order.
//!   Not modelled here.
//! * Gen 2 (pokecrystal `FindMonInOTPartyToSwitchIntoBattle`): [`next_gen2`].
//! * Gen 3 (pokeemerald `GetMostSuitableMonToSwitchInto`): [`next_gen3`].
//! * Gen 4 (pokeplatinum `BattleAI_PostKOSwitchIn`, HG/SS identical): [`next_gen4`].
//! * Gen 5 (pokebw2 `PickBestMonToSwitchInto`): [`predict_gen5`]. Black/White 1
//!   and Diamond/Pearl are inferred from their sequels.
//!
//! A prediction is a chain: the lead faints, the rule picks the next Pokémon
//! with the lead as the one that fainted, and so on. It assumes a neutral field
//! (no stat stages, screens or weather, nobody statused) and no voluntary
//! switch; those can change the real order, and voluntary switches are random.
//! Where a rule is itself random (gen 2's "every candidate is bad"), the rest
//! stay in party order. The recorder overwrites the prediction with what it saw
//! when the battle ends, so a wrong guess only lasts for the fight.
//!
//! The Dex page's calculator (`xpr-dex-ui`, Solodex's) is a different thing: it
//! explains each step of gens 2-4 and stops where the damage passes begin.

use crate::gen_data::GenData;
use crate::model::EnemyPkmn;

const FORESIGHT: &str = "(foresight)";

/// The games' own type chart, gens 2-5, as the battle engines walk it: one
/// entry per (attacking type, defending type) that is not neutral, in the ROM's
/// order. pokeemerald `gTypeEffectiveness` and pokeplatinum
/// `sTypeMatchupMultipliers` list the same entries in the same order. The order
/// matters where the engines multiply integers entry by entry (each step
/// truncates), so `GenData::effectiveness` can't stand in for it.
///
/// Multipliers are tenths, as in the ROM: 0 no effect, 5 not very effective,
/// 20 super effective. The Ghost immunities to Normal and Fighting come after
/// the [`FORESIGHT`] marker, so Foresight / Scrappy can skip them as a batch.
#[rustfmt::skip]
const TYPE_MATCHUPS: &[(&str, &str, i64)] = &[
    ("Normal", "Rock", 5), ("Normal", "Steel", 5),
    ("Fire", "Fire", 5), ("Fire", "Water", 5), ("Fire", "Grass", 20), ("Fire", "Ice", 20), ("Fire", "Bug", 20),
    ("Fire", "Rock", 5), ("Fire", "Dragon", 5), ("Fire", "Steel", 20),
    ("Water", "Fire", 20), ("Water", "Water", 5), ("Water", "Grass", 5), ("Water", "Ground", 20), ("Water", "Rock", 20),
    ("Water", "Dragon", 5),
    ("Electric", "Water", 20), ("Electric", "Electric", 5), ("Electric", "Grass", 5), ("Electric", "Ground", 0),
    ("Electric", "Flying", 20), ("Electric", "Dragon", 5),
    ("Grass", "Fire", 5), ("Grass", "Water", 20), ("Grass", "Grass", 5), ("Grass", "Poison", 5), ("Grass", "Ground", 20),
    ("Grass", "Flying", 5), ("Grass", "Bug", 5), ("Grass", "Rock", 20), ("Grass", "Dragon", 5), ("Grass", "Steel", 5),
    ("Ice", "Water", 5), ("Ice", "Grass", 20), ("Ice", "Ice", 5), ("Ice", "Ground", 20), ("Ice", "Flying", 20),
    ("Ice", "Dragon", 20), ("Ice", "Steel", 5), ("Ice", "Fire", 5),
    ("Fighting", "Normal", 20), ("Fighting", "Ice", 20), ("Fighting", "Poison", 5), ("Fighting", "Flying", 5),
    ("Fighting", "Psychic", 5), ("Fighting", "Bug", 5), ("Fighting", "Rock", 20), ("Fighting", "Dark", 20),
    ("Fighting", "Steel", 20),
    ("Poison", "Grass", 20), ("Poison", "Poison", 5), ("Poison", "Ground", 5), ("Poison", "Rock", 5), ("Poison", "Ghost", 5),
    ("Poison", "Steel", 0),
    ("Ground", "Fire", 20), ("Ground", "Electric", 20), ("Ground", "Grass", 5), ("Ground", "Poison", 20),
    ("Ground", "Flying", 0), ("Ground", "Bug", 5), ("Ground", "Rock", 20), ("Ground", "Steel", 20),
    ("Flying", "Electric", 5), ("Flying", "Grass", 20), ("Flying", "Fighting", 20), ("Flying", "Bug", 20),
    ("Flying", "Rock", 5), ("Flying", "Steel", 5),
    ("Psychic", "Fighting", 20), ("Psychic", "Poison", 20), ("Psychic", "Psychic", 5), ("Psychic", "Dark", 0),
    ("Psychic", "Steel", 5),
    ("Bug", "Fire", 5), ("Bug", "Grass", 20), ("Bug", "Fighting", 5), ("Bug", "Poison", 5), ("Bug", "Flying", 5),
    ("Bug", "Psychic", 20), ("Bug", "Ghost", 5), ("Bug", "Dark", 20), ("Bug", "Steel", 5),
    ("Rock", "Fire", 20), ("Rock", "Ice", 20), ("Rock", "Fighting", 5), ("Rock", "Ground", 5), ("Rock", "Flying", 20),
    ("Rock", "Bug", 20), ("Rock", "Steel", 5),
    ("Ghost", "Normal", 0), ("Ghost", "Psychic", 20), ("Ghost", "Dark", 5), ("Ghost", "Steel", 5), ("Ghost", "Ghost", 20),
    ("Dragon", "Dragon", 20), ("Dragon", "Steel", 5),
    ("Dark", "Fighting", 5), ("Dark", "Psychic", 20), ("Dark", "Ghost", 20), ("Dark", "Dark", 5), ("Dark", "Steel", 5),
    ("Steel", "Fire", 5), ("Steel", "Water", 5), ("Steel", "Electric", 5), ("Steel", "Ice", 20), ("Steel", "Rock", 20),
    ("Steel", "Steel", 5),
    (FORESIGHT, FORESIGHT, 0),
    ("Normal", "Ghost", 0), ("Fighting", "Ghost", 0),
];

/// The plain product of the chart against a defender's two types (the second
/// skipped when it repeats the first): 4, 2, 1, 0.5, 0.25 or 0. No integer
/// steps, so only for rules that compare the result (gens 2 and 5).
fn type_product(attack: &str, defend1: &str, defend2: &str) -> f64 {
    let mut product = 1.0;
    for &(a, d, m) in TYPE_MATCHUPS {
        if a != attack {
            continue;
        }
        if d == defend1 {
            product *= m as f64 / 10.0;
        }
        if d == defend2 && defend1 != defend2 {
            product *= m as f64 / 10.0;
        }
    }
    product
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveCategory {
    Physical,
    Special,
    Status,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SendOutMove {
    pub name: String,
    pub move_type: String,
    /// The ROM's base power: 0 for status moves, 1 for the moves whose power
    /// the engine works out (Seismic Toss, Return in gen 3...). Gen 5's data
    /// has no 1s; its rule counts anything under 10 as 60.
    pub power: i64,
    pub category: MoveCategory,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SendOutMon {
    pub species: String,
    /// A single-typed mon repeats its type.
    pub types: [String; 2],
    pub moves: Vec<SendOutMove>,
    /// Gens 3-4: what the damage pass reads from the Pokémon that just fainted.
    pub level: i64,
    pub attack: i64,
    pub special_attack: i64,
    pub ability: Option<String>,
    pub item: Option<String>,
    /// Gen 4: Hidden Power's type (from the IVs), if known; otherwise Normal.
    pub hidden_power_type: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SendOutPlayer {
    pub species: String,
    /// A single-typed mon repeats its type.
    pub types: [String; 2],
    /// Gens 3-4. Gen 3's badge boosts belong in these values or in the flags
    /// below, not both.
    pub defense: i64,
    pub special_defense: i64,
    pub ability: Option<String>,
    pub item: Option<String>,
    /// Gen 3: the move that knocks the enemy out (`gCurrentMove`), most often
    /// the player's strongest ([`strongest_move`]).
    pub ko_move: Option<SendOutMove>,
    /// Gen 3: badge stat boosts on the player's side (badges 5 and 7).
    pub defense_badge: bool,
    pub special_defense_badge: bool,
}

/// One rule: the slot sent out after `fainted` faints, given who is still to
/// come (`left`, ascending); `None` when the game picks at random.
type NextRule = fn(&[SendOutMon], usize, &[usize], &SendOutPlayer) -> Option<usize>;

fn key(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_lowercase()).collect()
}

fn opt_key(s: &Option<String>) -> String {
    key(s.as_deref().unwrap_or(""))
}

fn u8_of(n: i64) -> i64 {
    n & 0xff
}

/// A type name as the chart keys it: "FIRE", "fire" and "Fire" are all "Fire".
fn normal_type(s: &str) -> String {
    let s = s.trim().to_lowercase();
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn normal_types(types: &[String; 2]) -> (String, String) {
    (normal_type(&types[0]), normal_type(&types[1]))
}

fn chain(party: &[SendOutMon], player: &SendOutPlayer, next: NextRule) -> Vec<usize> {
    if party.is_empty() {
        return Vec::new();
    }
    let mut order = vec![0];
    let mut left: Vec<usize> = (1..party.len()).collect();
    while !left.is_empty() {
        let fainted = *order.last().unwrap();
        match next(party, fainted, &left, player) {
            Some(pick) if left.contains(&pick) => {
                order.push(pick);
                left.retain(|s| *s != pick);
            }
            _ => {
                order.extend(left);
                return order;
            }
        }
    }
    order
}

// ---- gen 2 --------------------------------------------------------------------------------------

/// pokecrystal `FindMonInOTPartyToSwitchIntoBattle` / `ScoreMonTypeMatchups`: a
/// candidate is good when one of its moves' types (power ignored: Sunny Day is
/// Fire) is super effective on the player's types, and bad when the player's
/// type 1 (or, failing that, type 2) is super effective on its species types;
/// both cancel. The lowest good slot, else the lowest not-bad slot, else random.
pub fn next_gen2(party: &[SendOutMon], _fainted: usize, left: &[usize], player: &SendOutPlayer) -> Option<usize> {
    let (p1, p2) = normal_types(&player.types);
    let score: Vec<(usize, bool, bool)> = left
        .iter()
        .map(|&i| {
            let mon = &party[i];
            let (t1, t2) = normal_types(&mon.types);
            let mut good = mon.moves.iter().any(|m| type_product(&normal_type(&m.move_type), &p1, &p2) > 1.0);
            let mut bad = false;
            if type_product(&p1, &t1, &t2) > 1.0 || type_product(&p2, &t1, &t2) > 1.0 {
                if good {
                    good = false;
                } else {
                    bad = true;
                }
            }
            (i, good, bad)
        })
        .collect();
    score.iter().find(|s| s.1).or_else(|| score.iter().find(|s| !s.2)).map(|s| s.0)
}

pub fn predict_gen2(party: &[SendOutMon], player: &SendOutPlayer) -> Vec<usize> {
    chain(party, player, next_gen2)
}

// ---- gen 3 --------------------------------------------------------------------------------------

const GEN3_PHYSICAL: &[&str] = &["Normal", "Fighting", "Flying", "Poison", "Ground", "Rock", "Bug", "Ghost", "Steel"];

fn gen3_physical(t: &str) -> bool {
    GEN3_PHYSICAL.contains(&t)
}

/// pokeemerald `sHoldEffectToType` (param 10 for each, Sea Incense 5).
fn gen3_type_item(item: &str) -> Option<(&'static str, i64)> {
    Some(match item {
        "silverpowder" => ("Bug", 10),
        "metalcoat" => ("Steel", 10),
        "softsand" => ("Ground", 10),
        "hardstone" => ("Rock", 10),
        "miracleseed" => ("Grass", 10),
        "blackglasses" => ("Dark", 10),
        "blackbelt" => ("Fighting", 10),
        "magnet" => ("Electric", 10),
        "mysticwater" => ("Water", 10),
        "sharpbeak" => ("Flying", 10),
        "poisonbarb" => ("Poison", 10),
        "nevermeltice" => ("Ice", 10),
        "spelltag" => ("Ghost", 10),
        "twistedspoon" => ("Psychic", 10),
        "charcoal" => ("Fire", 10),
        "dragonfang" => ("Dragon", 10),
        "silkscarf" => ("Normal", 10),
        "seaincense" => ("Water", 5),
        _ => return None,
    })
}

struct Gen3TypeCalc {
    damage: i64,
    super_effective: bool,
}

/// `ModulateDmgByType2` over the chart, in table order (pokeemerald `TypeCalc`).
fn gen3_type_calc(mut damage: i64, move_type: &str, power: i64, player: &SendOutPlayer) -> Gen3TypeCalc {
    let (d1, d2) = normal_types(&player.types);
    if opt_key(&player.ability) == "levitate" && move_type == "Ground" {
        return Gen3TypeCalc { damage, super_effective: false };
    }
    let (mut se, mut nve, mut no_effect) = (false, false, false);
    let mut apply = |m: i64| {
        damage = damage * m / 10;
        if damage == 0 && m != 0 {
            damage = 1;
        }
        if m == 0 {
            no_effect = true;
            se = false;
            nve = false;
        } else if m == 5 && power != 0 && !no_effect {
            if se {
                se = false;
            } else {
                nve = true;
            }
        } else if m == 20 && power != 0 && !no_effect {
            if nve {
                nve = false;
            } else {
                se = true;
            }
        }
    };
    for &(a, d, m) in TYPE_MATCHUPS {
        if a == FORESIGHT || a != move_type {
            continue;
        }
        if d == d1 {
            apply(m);
        }
        if d == d2 && d1 != d2 {
            apply(m);
        }
    }
    Gen3TypeCalc { damage, super_effective: se }
}

/// pokeemerald `CalculateBaseDamage`, the fainted enemy attacking the player
/// with `mv`, as `AI_CalcDmg` calls it here: no crit, no random roll, stat stages
/// neutral (`FaintClearSetData` reset the attacker's), no status, no weather or
/// screens. Its HP is 0, so Overgrow/Blaze/Torrent/Swarm apply. Returns damage + 2.
pub fn gen3_base_damage(attacker: &SendOutMon, player: &SendOutPlayer, mv: Option<&SendOutMove>) -> i64 {
    let mut power = mv.map(|m| m.power).unwrap_or(0);
    let move_type = normal_type(mv.map(|m| m.move_type.as_str()).unwrap_or("Normal"));
    let t = move_type.as_str();
    let (mut attack, mut sp_attack) = (attacker.attack, attacker.special_attack);
    let (mut defense, mut sp_defense) = (player.defense, player.special_defense);
    let (atk_ability, def_ability) = (opt_key(&attacker.ability), opt_key(&player.ability));
    let (atk_item, def_item) = (opt_key(&attacker.item), opt_key(&player.item));
    let (atk_species, def_species) = (key(&attacker.species), key(&player.species));
    if atk_ability == "hugepower" || atk_ability == "purepower" {
        attack *= 2;
    }
    if player.defense_badge {
        defense = 110 * defense / 100;
    }
    if player.special_defense_badge {
        sp_defense = 110 * sp_defense / 100;
    }
    if let Some((item_type, param)) = gen3_type_item(&atk_item) {
        if item_type == t {
            if gen3_physical(t) {
                attack = attack * (param + 100) / 100;
            } else {
                sp_attack = sp_attack * (param + 100) / 100;
            }
        }
    }
    if atk_item == "choiceband" {
        attack = 150 * attack / 100;
    }
    if atk_item == "souldew" && (atk_species == "latias" || atk_species == "latios") {
        sp_attack = 150 * sp_attack / 100;
    }
    if def_item == "souldew" && (def_species == "latias" || def_species == "latios") {
        sp_defense = 150 * sp_defense / 100;
    }
    if atk_item == "deepseatooth" && atk_species == "clamperl" {
        sp_attack *= 2;
    }
    if def_item == "deepseascale" && def_species == "clamperl" {
        sp_defense *= 2;
    }
    if atk_item == "lightball" && atk_species == "pikachu" {
        sp_attack *= 2;
    }
    if def_item == "metalpowder" && def_species == "ditto" {
        defense *= 2;
    }
    if atk_item == "thickclub" && (atk_species == "cubone" || atk_species == "marowak") {
        attack *= 2;
    }
    if def_ability == "thickfat" && (t == "Fire" || t == "Ice") {
        sp_attack /= 2;
    }
    if atk_ability == "hustle" {
        attack = 150 * attack / 100;
    }
    // hp 0 <= maxHP / 3: the pinch abilities always apply to a fainted attacker
    if (t == "Grass" && atk_ability == "overgrow") || (t == "Fire" && atk_ability == "blaze") || (t == "Water" && atk_ability == "torrent") || (t == "Bug" && atk_ability == "swarm") {
        power = 150 * power / 100;
    }
    let move_key = key(mv.map(|m| m.name.as_str()).unwrap_or(""));
    if move_key == "explosion" || move_key == "selfdestruct" {
        defense /= 2;
    }
    let level_factor = 2 * attacker.level / 5 + 2;
    let mut damage = 0;
    let mystery = t == "???" || t == "Mystery";
    if gen3_physical(t) {
        damage = attack * power * level_factor / defense.max(1) / 50;
        if damage == 0 {
            damage = 1;
        }
    }
    if !gen3_physical(t) && !mystery {
        damage = sp_attack * power * level_factor / sp_defense.max(1) / 50;
    }
    damage + 2
}

/// pokeemerald `GetMostSuitableMonToSwitchInto` (Ruby and FR/LG the same here).
/// Pass 1: candidates by how hard the PLAYER's types hit their species types
/// (most first, a game quirk the source flags), the first with a super-effective
/// damaging move. Pass 2: the (candidate, move) with the most damage, where the
/// damage is `gCurrentMove`'s from the fainted Pokémon, times STAB on the fainted
/// Pokémon's types and the candidate move's type chart, kept in a byte.
pub fn next_gen3(party: &[SendOutMon], fainted: usize, left: &[usize], player: &SendOutPlayer) -> Option<usize> {
    let (p1, p2) = normal_types(&player.types);
    let mut invalid: Vec<usize> = Vec::new();
    loop {
        let (mut best, mut best_mon) = (0, None);
        for &i in left {
            if invalid.contains(&i) {
                continue;
            }
            let (t1, t2) = normal_types(&party[i].types);
            let mut t = 10;
            for atk in [&p1, &p2] {
                for &(a, d, m) in TYPE_MATCHUPS {
                    if a == FORESIGHT || a != atk.as_str() {
                        continue;
                    }
                    if d == t1 {
                        t = u8_of(t * m / 10);
                    }
                    if d == t2 && t1 != t2 {
                        t = u8_of(t * m / 10);
                    }
                }
            }
            if best < t {
                best = t;
                best_mon = Some(i);
            }
        }
        let Some(mon) = best_mon else { break };
        if party[mon].moves.iter().any(|m| gen3_type_calc(0, &normal_type(&m.move_type), m.power, player).super_effective) {
            return Some(mon);
        }
        invalid.push(mon);
    }
    let attacker = &party[fainted];
    let base = gen3_base_damage(attacker, player, player.ko_move.as_ref());
    let (a1, a2) = normal_types(&attacker.types);
    let (mut best_dmg, mut best_mon) = (0, None);
    for &i in left {
        for j in 0..4 {
            let mut damage = 0;
            if let Some(mv) = party[i].moves.get(j).filter(|m| m.power != 1) {
                let t = normal_type(&mv.move_type);
                damage = base;
                if a1 == t || a2 == t {
                    damage = damage * 15 / 10;
                }
                damage = gen3_type_calc(damage, &t, mv.power, player).damage;
            }
            if best_dmg < damage {
                best_dmg = u8_of(damage);
                best_mon = Some(i);
            }
        }
    }
    best_mon.or(left.first().copied())
}

pub fn predict_gen3(party: &[SendOutMon], player: &SendOutPlayer) -> Vec<usize> {
    chain(party, player, next_gen3)
}

// ---- gen 4 --------------------------------------------------------------------------------------

/// pokeplatinum `sTypeBoostingItems`: the type items and plates.
fn gen4_type_item(item: &str) -> Option<&'static str> {
    Some(match item {
        "silverpowder" | "insectplate" => "Bug",
        "metalcoat" | "ironplate" => "Steel",
        "softsand" | "earthplate" => "Ground",
        "hardstone" | "rockincense" | "stoneplate" => "Rock",
        "miracleseed" | "roseincense" | "meadowplate" => "Grass",
        "blackglasses" | "dreadplate" => "Dark",
        "blackbelt" | "fistplate" => "Fighting",
        "magnet" | "zapplate" => "Electric",
        "mysticwater" | "seaincense" | "waveincense" | "splashplate" => "Water",
        "sharpbeak" | "skyplate" => "Flying",
        "poisonbarb" | "toxicplate" => "Poison",
        "nevermeltice" | "icicleplate" => "Ice",
        "spelltag" | "spookyplate" => "Ghost",
        "twistedspoon" | "oddincense" | "mindplate" => "Psychic",
        "charcoal" | "flameplate" => "Fire",
        "dragonfang" | "dracoplate" => "Dragon",
        "silkscarf" => "Normal",
        _ => return None,
    })
}

const GEN4_PUNCHES: &[&str] = &[
    "icepunch", "firepunch", "thunderpunch", "machpunch", "focuspunch", "dizzypunch", "dynamicpunch", "hammerarm", "megapunch", "cometpunch", "meteormash", "shadowpunch",
    "drainpunch", "bulletpunch", "skyuppercut",
];

/// `Move_CalcVariableType`: Hidden Power from the IVs, Judgment from the plate;
/// everything else is the move's own type here (Natural Gift's berry type is not
/// modelled; Weather Ball has no weather in a neutral field).
fn gen4_move_type(mon: &SendOutMon, mv: &SendOutMove) -> String {
    let move_key = key(&mv.name);
    if move_key == "hiddenpower" {
        if let Some(t) = &mon.hidden_power_type {
            return normal_type(t);
        }
    }
    if move_key == "judgment" {
        let item = opt_key(&mon.item);
        return if item.ends_with("plate") { gen4_type_item(&item).unwrap_or("Normal").to_string() } else { "Normal".to_string() };
    }
    normal_type(&mv.move_type)
}

/// `BattleSystem_TypeMatchupMultiplier`: 40, times each chart entry against the
/// two types (the second once).
fn gen4_type_matchup_multiplier(attack: &str, defend1: &str, defend2: &str) -> i64 {
    let mut mul = 40;
    for &(a, d, m) in TYPE_MATCHUPS {
        if a != attack {
            continue;
        }
        if d == defend1 {
            mul = mul * m / 10;
        }
        if d == defend2 && defend1 != defend2 {
            mul = mul * m / 10;
        }
    }
    mul
}

/// `BattleSystem_CalcEffectiveness`: does this move come out super effective on
/// the player (status moves too)?
fn gen4_super_effective(mon: &SendOutMon, mv: &SendOutMove, player: &SendOutPlayer) -> bool {
    let ability = opt_key(&mon.ability);
    let move_type = if ability == "normalize" { "Normal".to_string() } else { gen4_move_type(mon, mv) };
    let (d1, d2) = normal_types(&player.types);
    if ability != "moldbreaker" && opt_key(&player.ability) == "levitate" && move_type == "Ground" && opt_key(&player.item) != "ironball" {
        return false;
    }
    let (mut se, mut nve) = (false, false);
    // UpateMoveStatusForTypeMul (no power check: status moves count)
    let mut update = |m: i64| {
        if m == 0 {
            se = false;
            nve = false;
        } else if m == 5 {
            if se {
                se = false;
            } else {
                nve = true;
            }
        } else if nve {
            nve = false;
        } else {
            se = true;
        }
    };
    for &(a, d, m) in TYPE_MATCHUPS {
        if a == FORESIGHT {
            if ability == "scrappy" {
                break;
            }
            continue;
        }
        if a != move_type {
            continue;
        }
        if d == d1 {
            update(m);
        }
        if d == d2 && d1 != d2 {
            update(m);
        }
    }
    se
}

fn divide(dividend: i64, divisor: i64) -> i64 {
    if dividend == 0 {
        return 0;
    }
    let q = dividend / divisor;
    if q == 0 {
        dividend.signum()
    } else {
        q
    }
}

/// pokeplatinum `BattleSystem_CalcMoveDamage` then `BattleSystem_ApplyTypeChart`,
/// as `BattleAI_PostKOSwitchIn` calls them: the FAINTED enemy attacks the player
/// with a candidate's move, crit multiplier 1, power multiplier 10, no random
/// roll, neutral stages, no status, weather or screens. Its HP is 0, so the pinch
/// abilities apply and Slow Start always halves. Returns the damage and whether
/// the player is immune. The caller keeps the byte after each step.
pub fn gen4_ai_damage(attacker: &SendOutMon, player: &SendOutPlayer, mv: &SendOutMove) -> (i64, bool) {
    let (atk_ability, def_ability) = (opt_key(&attacker.ability), opt_key(&player.ability));
    let mold_breaker = atk_ability == "moldbreaker";
    let def_has = |ability: &str| !mold_breaker && def_ability == ability;
    let (atk_item, def_item) = (opt_key(&attacker.item), opt_key(&player.item));
    let (atk_species, def_species) = (key(&attacker.species), key(&player.species));
    let move_type = if atk_ability == "normalize" { "Normal".to_string() } else { normal_type(&mv.move_type) };
    let t = move_type.as_str();
    let move_key = key(&mv.name);
    let mut power = mv.power;
    let (mut attack, mut sp_attack) = (attacker.attack, attacker.special_attack);
    let (mut defense, mut sp_defense) = (player.defense, player.special_defense);
    if atk_ability == "technician" && move_key != "struggle" && power <= 60 {
        power = power * 15 / 10;
    }
    if atk_ability == "hugepower" || atk_ability == "purepower" {
        attack *= 2;
    }
    if atk_ability == "slowstart" {
        attack /= 2;
    }
    if gen4_type_item(&atk_item) == Some(t) {
        power = power * 120 / 100;
    }
    if atk_item == "choiceband" {
        attack = attack * 150 / 100;
    }
    if atk_item == "choicespecs" {
        sp_attack = sp_attack * 150 / 100;
    }
    if atk_item == "souldew" && (atk_species == "latios" || atk_species == "latias") {
        sp_attack = sp_attack * 150 / 100;
    }
    if def_item == "souldew" && (def_species == "latios" || def_species == "latias") {
        sp_defense = sp_defense * 150 / 100;
    }
    if atk_item == "deepseatooth" && atk_species == "clamperl" {
        sp_attack *= 2;
    }
    if def_item == "deepseascale" && def_species == "clamperl" {
        sp_defense *= 2;
    }
    if atk_item == "lightball" && atk_species == "pikachu" {
        power *= 2;
    }
    if def_item == "metalpowder" && def_species == "ditto" {
        defense *= 2;
    }
    if atk_item == "thickclub" && (atk_species == "cubone" || atk_species == "marowak") {
        attack *= 2;
    }
    if (atk_item == "adamantorb" && atk_species == "dialga" && (t == "Dragon" || t == "Steel"))
        || (atk_item == "lustrousorb" && atk_species == "palkia" && (t == "Dragon" || t == "Water"))
        || (atk_item == "griseousorb" && atk_species == "giratina" && (t == "Dragon" || t == "Ghost"))
    {
        power = power * 120 / 100;
    }
    if atk_item == "muscleband" && mv.category == MoveCategory::Physical {
        power = power * 110 / 100;
    }
    if atk_item == "wiseglasses" && mv.category == MoveCategory::Special {
        power = power * 110 / 100;
    }
    if def_has("thickfat") && (t == "Fire" || t == "Ice") {
        power /= 2;
    }
    if atk_ability == "hustle" {
        attack = attack * 150 / 100;
    }
    if (t == "Grass" && atk_ability == "overgrow") || (t == "Fire" && atk_ability == "blaze") || (t == "Water" && atk_ability == "torrent") || (t == "Bug" && atk_ability == "swarm") {
        power = power * 150 / 100;
    }
    if t == "Fire" && def_has("heatproof") {
        power /= 2;
    }
    if t == "Fire" && def_has("dryskin") {
        power = power * 125 / 100;
    }
    if atk_ability == "ironfist" && GEN4_PUNCHES.contains(&move_key.as_str()) {
        power = power * 12 / 10;
    }
    if move_key == "explosion" || move_key == "selfdestruct" {
        defense /= 2;
    }
    // u16 movePower
    power &= 0xffff;
    let level_factor = attacker.level * 2 / 5 + 2;
    let mut damage = match mv.category {
        MoveCategory::Physical => attack * power * level_factor / defense.max(1) / 50,
        MoveCategory::Special => sp_attack * power * level_factor / sp_defense.max(1) / 50,
        MoveCategory::Status => 0,
    };
    damage = u8_of(damage + 2);

    // ApplyTypeChart
    if move_key == "struggle" {
        return (damage, false);
    }
    let chart_type = if atk_ability == "normalize" { "Normal".to_string() } else { gen4_move_type(attacker, mv) };
    let (a1, a2) = normal_types(&attacker.types);
    if a1 == chart_type || a2 == chart_type {
        damage = if atk_ability == "adaptability" { damage * 2 } else { damage * 15 / 10 };
    }
    let (d1, d2) = normal_types(&player.types);
    let (mut se, mut nve, mut immune) = (false, false, false);
    if def_has("levitate") && chart_type == "Ground" && def_item != "ironball" {
        immune = true;
    } else {
        let mut apply = |m: i64| {
            if damage != 0 {
                damage = divide(damage * m, 10);
            }
            if m == 0 {
                immune = true;
                se = false;
                nve = false;
            } else if m == 5 && mv.power != 0 {
                if se {
                    se = false;
                } else {
                    nve = true;
                }
            } else if m == 20 && mv.power != 0 {
                if nve {
                    nve = false;
                } else {
                    se = true;
                }
            }
        };
        for &(a, d, m) in TYPE_MATCHUPS {
            if a == FORESIGHT {
                if atk_ability == "scrappy" {
                    break;
                }
                continue;
            }
            if a != chart_type {
                continue;
            }
            let iron_ball_ground = def_item == "ironball" && d == "Flying" && m == 0;
            if d == d1 && !iron_ball_ground {
                apply(m);
            }
            if d == d2 && d1 != d2 && !iron_ball_ground {
                apply(m);
            }
        }
    }
    if def_has("wonderguard") && (!se || nve) && mv.power != 0 {
        immune = true;
    } else {
        if se && mv.power != 0 {
            if def_has("filter") || def_has("solidrock") {
                damage = divide(damage * 3, 4);
            }
            if atk_item == "expertbelt" {
                damage = damage * 120 / 100;
            }
        }
        if nve && mv.power != 0 && atk_ability == "tintedlens" {
            damage *= 2;
        }
    }
    (damage, immune)
}

/// pokeplatinum `BattleAI_PostKOSwitchIn` (HG/SS `ov12_02258800` is the same
/// code). Stage 1: candidates by their own species types attacking the player's
/// types (summed in a byte), best first, the first with a super-effective move
/// (status moves count). Stage 2: the (candidate, move) with the most damage from
/// the fainted Pokémon's stats, kept in a byte. The score variable is not reset
/// between moves or between the stages (a skipped move compares the previous
/// value again): reproduced on purpose.
pub fn next_gen4(party: &[SendOutMon], fainted: usize, left: &[usize], player: &SendOutPlayer) -> Option<usize> {
    let (p1, p2) = normal_types(&player.types);
    let mut disregarded: Vec<usize> = Vec::new();
    let mut score = 0;
    loop {
        let (mut max_score, mut picked) = (0, None);
        for &i in left {
            if disregarded.contains(&i) {
                continue;
            }
            let (t1, t2) = normal_types(&party[i].types);
            score = u8_of(gen4_type_matchup_multiplier(&t1, &p1, &p2) + gen4_type_matchup_multiplier(&t2, &p1, &p2));
            if max_score < score {
                max_score = score;
                picked = Some(i);
            }
        }
        let Some(pick) = picked else { break };
        let mon = &party[pick];
        if mon.moves.iter().any(|m| gen4_super_effective(mon, m, player)) {
            return Some(pick);
        }
        disregarded.push(pick);
    }
    let attacker = &party[fainted];
    let (mut max_score, mut picked) = (0, None);
    for &i in left {
        for j in 0..4 {
            if let Some(mv) = party[i].moves.get(j).filter(|m| m.power != 1) {
                let (damage, immune) = gen4_ai_damage(attacker, player, mv);
                score = if immune { 0 } else { u8_of(damage) };
            }
            if max_score < score {
                max_score = score;
                picked = Some(i);
            }
        }
    }
    picked.or(left.first().copied())
}

pub fn predict_gen4(party: &[SendOutMon], player: &SendOutPlayer) -> Vec<usize> {
    chain(party, player, next_gen4)
}

// ---- gen 5 --------------------------------------------------------------------------------------

/// pokebw2 `PickBestMonToSwitchInto`: each candidate's best damaging move, base
/// power (under 10 counts as 60) times the type chart against the player (integer
/// halving), no STAB, stats or abilities. The battle keeps its own party order:
/// candidates are positions 1 on, sorted by `if (p[i] < p[j]) swap` for every
/// i < j (not a stable sort), and a switch-in swaps position 0 with the
/// newcomer's position.
pub fn predict_gen5(party: &[SendOutMon], player: &SendOutPlayer) -> Vec<usize> {
    if party.is_empty() {
        return Vec::new();
    }
    let (p1, p2) = normal_types(&player.types);
    let score_of = |mon: &SendOutMon| -> i64 {
        let mut best = 0;
        for mv in &mon.moves {
            if mv.category == MoveCategory::Status {
                continue;
            }
            let mut power = if mv.power < 10 { 60 } else { mv.power };
            let product = type_product(&normal_type(&mv.move_type), &p1, &p2);
            if product == 4.0 {
                power *= 4;
            } else if product == 2.0 {
                power *= 2;
            } else if product == 0.5 {
                power /= 2;
            } else if product == 0.25 {
                power /= 4;
            } else if product == 0.0 {
                power = 0;
            }
            if best < power {
                best = power;
            }
        }
        best
    };
    // positions[p] = party slot at battle position p
    let mut positions: Vec<usize> = (0..party.len()).collect();
    let mut fainted: Vec<usize> = Vec::new();
    let mut order = vec![positions[0]];
    while order.len() < party.len() {
        fainted.push(positions[0]);
        let mut list: Vec<usize> = (1..positions.len()).filter(|p| !fainted.contains(&positions[*p])).collect();
        if list.is_empty() {
            break;
        }
        let mut power: Vec<i64> = list.iter().map(|p| score_of(&party[positions[*p]])).collect();
        for i in 0..list.len() {
            for j in (i + 1)..list.len() {
                if power[i] < power[j] {
                    power.swap(i, j);
                    list.swap(i, j);
                }
            }
        }
        positions.swap(0, list[0]);
        order.push(positions[0]);
    }
    order
}

// ---- from the router's data --------------------------------------------------------------------

/// The player's strongest damaging move: the stand-in for gen 3's `gCurrentMove`
/// (the move that made the KO).
pub fn strongest_move(moves: &[SendOutMove]) -> Option<SendOutMove> {
    let mut best: Option<&SendOutMove> = None;
    for mv in moves {
        if mv.category == MoveCategory::Status || mv.power <= 1 {
            continue;
        }
        if best.map(|b| mv.power > b.power).unwrap_or(true) {
            best = Some(mv);
        }
    }
    best.cloned()
}

/// A species' types, a single type repeated (the games store them that way).
pub fn species_types(gen: &GenData, species: &str) -> [String; 2] {
    match gen.pkmn_db().get_pkmn(species) {
        Some(s) => {
            let single = s.second_type.is_empty() || s.second_type.eq_ignore_ascii_case("none");
            let second = if single { s.first_type.clone() } else { s.second_type.clone() };
            [s.first_type.clone(), second]
        }
        None => ["Normal".to_string(), "Normal".to_string()],
    }
}

/// A move as the rules read it. Gens 2-3 have no categories in the data, so
/// theirs comes from the type.
pub fn send_out_move(gen: &GenData, name: &str) -> SendOutMove {
    let data = gen.move_db().get_move(name);
    let power = data.map(|m| m.base_power_or_zero()).unwrap_or(0);
    let mut move_type = data.map(|m| m.move_type.clone()).unwrap_or_else(|| "Normal".to_string());
    // the ROMs of gens 2-4 have Curse as its own type, which the chart never
    // mentions; the router's data says Ghost, as later games do
    if key(name) == "curse" && gen.get_generation() <= 4 {
        move_type = "???".to_string();
    }
    let category = match key(data.map(|m| m.category.as_str()).unwrap_or("")).as_str() {
        "physical" => MoveCategory::Physical,
        "special" => MoveCategory::Special,
        "status" => MoveCategory::Status,
        _ if power == 0 => MoveCategory::Status,
        _ if gen3_physical(&normal_type(&move_type)) => MoveCategory::Physical,
        _ => MoveCategory::Special,
    };
    SendOutMove { name: name.to_string(), move_type, power, category }
}

/// A trainer's Pokémon as the rules read it.
pub fn send_out_mon(gen: &GenData, mon: &EnemyPkmn) -> SendOutMon {
    let moves: Vec<SendOutMove> = mon.move_list.iter().flatten().filter(|m| !m.is_empty()).map(|m| send_out_move(gen, m)).collect();
    SendOutMon {
        species: mon.name.clone(),
        types: species_types(gen, &mon.name),
        moves,
        level: mon.level,
        attack: mon.cur_stats.attack,
        special_attack: mon.cur_stats.special_attack,
        ability: (!mon.ability.is_empty()).then(|| mon.ability.clone()),
        item: mon.held_item.clone().filter(|i| !i.is_empty()),
        hidden_power_type: (gen.get_generation() == 4).then(|| gen.get_hidden_power(&mon.dvs).0),
    }
}

/// The predicted send-out order of `party` (party slots, the lead first) in a
/// generation that has a rule: gens 2-5.
pub fn predict(generation: u8, party: &[SendOutMon], player: &SendOutPlayer) -> Option<Vec<usize>> {
    match generation {
        2 => Some(predict_gen2(party, player)),
        3 => Some(predict_gen3(party, player)),
        4 => Some(predict_gen4(party, player)),
        5 => Some(predict_gen5(party, player)),
        _ => None,
    }
}

/// `order` (party slots, sent out first to last) as a trainer event's
/// `mon_order`: `mon_order[party slot]` is the 1-based position it comes out
/// at. Empty when that is just party order, which is what an empty one means.
pub fn mon_order_of(order: &[usize]) -> Vec<i64> {
    if order.iter().enumerate().all(|(i, s)| i == *s) {
        return Vec::new();
    }
    let mut result = vec![0; order.len()];
    for (position, slot) in order.iter().enumerate() {
        result[*slot] = position as i64 + 1;
    }
    result
}

#[cfg(test)]
mod tests {
    //! The frontend's self-test (`tools/parity/send-out-order-selftest.ts`),
    //! each case worked out by hand from the decompilations, plus real teams.
    use super::*;
    use crate::Registry;
    use std::path::PathBuf;

    fn mv(name: &str, t: &str, power: i64) -> SendOutMove {
        let category = if power != 0 { MoveCategory::Physical } else { MoveCategory::Status };
        SendOutMove { name: name.into(), move_type: t.into(), power, category }
    }

    fn sp(name: &str, t: &str, power: i64) -> SendOutMove {
        SendOutMove { category: MoveCategory::Special, ..mv(name, t, power) }
    }

    fn mon(species: &str, t1: &str, t2: &str, moves: Vec<SendOutMove>) -> SendOutMon {
        SendOutMon {
            species: species.into(),
            types: [t1.into(), t2.into()],
            moves,
            level: 1,
            attack: 0,
            special_attack: 0,
            ability: None,
            item: None,
            hidden_power_type: None,
        }
    }

    fn stats(m: SendOutMon, level: i64, stat: i64) -> SendOutMon {
        SendOutMon { level, attack: stat, special_attack: stat, ..m }
    }

    fn player(species: &str, t1: &str, t2: &str, defense: i64, special_defense: i64) -> SendOutPlayer {
        SendOutPlayer {
            species: species.into(),
            types: [t1.into(), t2.into()],
            defense,
            special_defense,
            ability: None,
            item: None,
            ko_move: None,
            defense_badge: false,
            special_defense_badge: false,
        }
    }

    fn names(party: &[SendOutMon], order: Vec<usize>) -> Vec<String> {
        order.into_iter().map(|i| party[i].species.clone()).collect()
    }

    #[test]
    fn gen2_crystal_red_vs_sunflora() {
        let red = vec![
            mon("Pikachu", "Electric", "Electric", vec![mv("Charm", "Normal", 0), mv("Quick Attack", "Normal", 40), mv("Thunderbolt", "Electric", 95), mv("Thunder", "Electric", 120)]),
            mon("Espeon", "Psychic", "Psychic", vec![mv("Mud-Slap", "Ground", 20), mv("Reflect", "Psychic", 0), mv("Swift", "Normal", 60), mv("Psychic", "Psychic", 90)]),
            mon("Snorlax", "Normal", "Normal", vec![mv("Amnesia", "Psychic", 0), mv("Snore", "Normal", 40), mv("Rest", "Psychic", 0), mv("Body Slam", "Normal", 85)]),
            mon("Venusaur", "Grass", "Poison", vec![mv("Sunny Day", "Fire", 0), mv("Giga Drain", "Grass", 60), mv("Synthesis", "Grass", 0), mv("Solarbeam", "Grass", 120)]),
            mon("Charizard", "Fire", "Flying", vec![mv("Flamethrower", "Fire", 95), mv("Wing Attack", "Flying", 60), mv("Slash", "Normal", 70), mv("Fire Spin", "Fire", 15)]),
            mon("Blastoise", "Water", "Water", vec![mv("Rain Dance", "Water", 0), mv("Surf", "Water", 95), mv("Blizzard", "Ice", 120), mv("Whirlpool", "Water", 15)]),
        ];
        let sunflora = player("Sunflora", "Grass", "Grass", 0, 0);
        assert_eq!(names(&red, predict_gen2(&red, &sunflora)), ["Pikachu", "Venusaur", "Charizard", "Espeon", "Snorlax", "Blastoise"]);
    }

    #[test]
    fn gen2_all_bad_keeps_party_order() {
        let party = [mon("A", "Normal", "Normal", vec![]), mon("B", "Fire", "Fire", vec![mv("Ember", "Fire", 40)]), mon("C", "Fire", "Fire", vec![mv("Ember", "Fire", 40)])];
        assert_eq!(predict_gen2(&party, &player("Squirtle", "Water", "Water", 0, 0)), [0, 1, 2]);
    }

    fn gen3_party() -> Vec<SendOutMon> {
        vec![
            stats(mon("Lead", "Normal", "Normal", vec![mv("Tackle", "Normal", 35)]), 20, 40),
            stats(mon("Neutral", "Normal", "Normal", vec![mv("Tackle", "Normal", 35)]), 20, 40),
            // Water and Ground both hit Rock/Fire; has Grass (SE on Ground/Water x4)
            stats(mon("Weak+SE", "Rock", "Fire", vec![sp("Absorb", "Grass", 20)]), 20, 40),
            // weak too but only a status Grass move: power 0 never counts as SE
            stats(mon("Weak+status", "Fire", "Fire", vec![mv("Stun Spore", "Grass", 0)]), 20, 40),
        ]
    }

    #[test]
    fn gen3_pass_1_picks_the_weakest_with_a_damaging_se_move() {
        // then nobody qualifies: pass 2 with D = 3, where a status move still
        // scores (Stun Spore: 3 x2 x2 = 12 beats Tackle 3)
        assert_eq!(predict_gen3(&gen3_party(), &player("Marshtomp", "Water", "Ground", 60, 50)), [0, 2, 3, 1]);
    }

    #[test]
    fn gen3_levitate_blocks_ground_in_pass_1() {
        let party = [
            stats(mon("Lead", "Normal", "Normal", vec![mv("Tackle", "Normal", 35)]), 20, 30),
            stats(mon("Digger", "Ground", "Ground", vec![mv("Dig", "Ground", 60)]), 20, 30),
            stats(mon("Biter", "Dark", "Dark", vec![mv("Bite", "Dark", 60)]), 20, 30),
        ];
        let mut lunatone = player("Lunatone", "Rock", "Psychic", 50, 50);
        lunatone.ability = Some("Levitate".into());
        assert_eq!(predict_gen3(&party, &lunatone), [0, 2, 1]);
    }

    #[test]
    fn gen3_pass_2_takes_stab_from_the_fainted_mon() {
        // D = 3 with no KO move; the lead is Water: Water Gun gets STAB (3 -> 4), Tackle stays 3
        let party = [
            stats(mon("Lead", "Water", "Water", vec![mv("Water Gun", "Water", 40)]), 30, 50),
            stats(mon("Tackler", "Normal", "Normal", vec![mv("Tackle", "Normal", 35)]), 30, 50),
            stats(mon("Gunner", "Normal", "Normal", vec![mv("Water Gun", "Water", 40)]), 30, 50),
        ];
        assert_eq!(predict_gen3(&party, &player("Snorlax", "Normal", "Normal", 65, 110)), [0, 2, 1]);
    }

    #[test]
    fn gen4_type_matchups() {
        assert_eq!(gen4_type_matchup_multiplier("Normal", "Water", "Water"), 40);
        assert_eq!(gen4_type_matchup_multiplier("Electric", "Water", "Flying"), 160);
        assert_eq!(gen4_type_matchup_multiplier("Normal", "Ghost", "Ghost"), 0);
    }

    #[test]
    fn gen4_stage_1_wraps_in_a_byte() {
        let party = [
            stats(mon("Lead", "Normal", "Normal", vec![mv("Tackle", "Normal", 35)]), 30, 50),
            // Electric: 160 + 160 = 320 -> byte 64. Thunder Wave: status counts in stage 1
            stats(mon("Pikachu", "Electric", "Electric", vec![mv("Thunder Wave", "Electric", 0)]), 30, 50),
            // Rock 80 + Ground 0 = 80, Rock Throw SE
            stats(mon("Geodude", "Rock", "Ground", vec![mv("Rock Throw", "Rock", 50)]), 30, 50),
            // 40 + 40 = 80 but later, and Vine Whip is not SE
            stats(mon("Grass", "Grass", "Grass", vec![mv("Vine Whip", "Grass", 35)]), 30, 50),
        ];
        assert_eq!(predict_gen4(&party, &player("Gyarados", "Water", "Flying", 79, 100)), [0, 2, 1, 3]);
    }

    #[test]
    fn gen4_stage_2_wraps_in_a_byte() {
        // Lv50 Atk 400 vs Def 100. Mega Kick: 211 + 2 = 213, STAB 319 -> 63. Tackle: 61 + 2, STAB 94.
        // The weaker move's Pokémon wins.
        let party = [
            stats(mon("Lead", "Normal", "Normal", vec![mv("Tackle", "Normal", 35)]), 50, 400),
            stats(mon("Kicker", "Normal", "Normal", vec![mv("Mega Kick", "Normal", 120)]), 50, 400),
            stats(mon("Tackler", "Normal", "Normal", vec![mv("Tackle", "Normal", 35)]), 50, 400),
        ];
        assert_eq!(predict_gen4(&party, &player("Snorlax", "Normal", "Normal", 100, 100)), [0, 2, 1]);
    }

    #[test]
    fn gen5_swap_sort_and_battle_positions() {
        let party = [
            mon("Lead", "Normal", "Normal", vec![mv("Tackle", "Normal", 50)]),
            mon("A", "Normal", "Normal", vec![mv("Pound", "Normal", 50)]),
            mon("B", "Normal", "Normal", vec![mv("Scratch", "Normal", 60)]),
            mon("C", "Normal", "Normal", vec![mv("Cut", "Normal", 60)]),
            mon("D", "Water", "Water", vec![sp("Water Gun", "Water", 40)]),
        ];
        // round 1: A50 B60 C60 D80 -> D, swap: [D, A, B, C, Lead]
        // round 2: A50 B60 C60 -> (B,A,C), (B,C,A) -> B, swap: [B, A, D, C, Lead]
        // round 3: A50 C60 -> C. Round 4: A.
        assert_eq!(names(&party, predict_gen5(&party, &player("Tepig", "Fire", "Fire", 0, 0))), ["Lead", "D", "B", "C", "A"]);
    }

    #[test]
    fn gen5_ignores_status_and_low_power_counts_as_60() {
        let party = [
            mon("Lead", "Normal", "Normal", vec![]),
            mon("Status", "Water", "Water", vec![mv("Rain Dance", "Water", 0)]),
            mon("Toss", "Fighting", "Fighting", vec![SendOutMove { category: MoveCategory::Physical, ..mv("Seismic Toss", "Fighting", 0) }]),
        ];
        assert_eq!(predict_gen5(&party, &player("Tepig", "Fire", "Fire", 0, 0)), [0, 2, 1]);
    }

    #[test]
    fn mon_order_is_indexed_by_party_slot() {
        // 0 comes out first, then 2, then 3, then 1
        assert_eq!(mon_order_of(&[0, 2, 3, 1]), [1, 4, 2, 3]);
        assert!(mon_order_of(&[0, 1, 2]).is_empty());
        assert!(mon_order_of(&[]).is_empty());
    }

    fn registry() -> Registry {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        Registry::new(root.join("raw_pkmn_data"), PathBuf::new())
    }

    #[test]
    fn curse_is_not_ghost_in_gens_2_to_4() {
        let reg = registry();
        for version in ["Crystal", "Emerald", "Platinum"] {
            assert_eq!(send_out_move(&reg.get_version(version).unwrap(), "Curse").move_type, "???", "{}", version);
        }
    }

    #[test]
    fn real_data_reads_as_the_rules_expect() {
        let reg = registry();
        let crystal = reg.get_version("Crystal").unwrap();
        assert_eq!(send_out_move(&crystal, "Sunny Day").move_type, "Fire");
        assert_eq!(send_out_move(&crystal, "Sunny Day").category, MoveCategory::Status);
        assert_eq!(species_types(&crystal, "Pikachu"), ["Electric", "Electric"]);
        assert_eq!(species_types(&crystal, "Venusaur"), ["Grass", "Poison"]);
        let emerald = reg.get_version("Emerald").unwrap();
        assert_eq!(send_out_move(&emerald, "Return").power, 1);
        assert_eq!(send_out_move(&emerald, "Thunderbolt").category, MoveCategory::Special);
        assert_eq!(send_out_move(&emerald, "Earthquake").category, MoveCategory::Physical);
        let platinum = reg.get_version("Platinum").unwrap();
        assert_eq!(send_out_move(&platinum, "Shadow Ball").category, MoveCategory::Special);
    }

    /// Crystal's Red against the frontend's example, from the router's own tables.
    #[test]
    fn crystal_red_from_the_router_data() {
        let reg = registry();
        let gen = reg.get_version("Crystal").unwrap();
        let red = gen.trainer_db().iter().find(|t| t.pkmn.len() == 6 && t.pkmn[0].name == "Pikachu" && t.pkmn[0].level == 81).expect("Red");
        let party: Vec<SendOutMon> = red.pkmn.iter().map(|m| send_out_mon(&gen, m)).collect();
        let sunflora = SendOutPlayer { types: species_types(&gen, "Sunflora"), ..player("Sunflora", "", "", 0, 0) };
        let order = predict(gen.get_generation(), &party, &sunflora).unwrap();
        assert_eq!(names(&party, order), ["Pikachu", "Venusaur", "Charizard", "Espeon", "Snorlax", "Blastoise"]);
    }

    /// Over every trainer in the data: each order is a permutation led by slot 0.
    #[test]
    fn every_real_team_gets_a_full_order() {
        let reg = registry();
        for version in ["Crystal", "Gold", "Emerald", "FireRed", "Platinum", "HeartGold", "Diamond", "Black", "White 2"] {
            let gen = reg.get_version(version).unwrap();
            let types = species_types(&gen, "Gyarados");
            let you = SendOutPlayer { ko_move: Some(send_out_move(&gen, "Surf")), ..player("Gyarados", &types[0], &types[1], 120, 130) };
            for t in gen.trainer_db().iter() {
                let party: Vec<SendOutMon> = t.pkmn.iter().map(|m| send_out_mon(&gen, m)).collect();
                let order = predict(gen.get_generation(), &party, &you).unwrap();
                let mut sorted = order.clone();
                sorted.sort();
                assert_eq!(sorted, (0..party.len()).collect::<Vec<_>>(), "{} {}", version, t.name);
                if !order.is_empty() {
                    assert_eq!(order[0], 0, "{} {}", version, t.name);
                }
            }
        }
    }
}
