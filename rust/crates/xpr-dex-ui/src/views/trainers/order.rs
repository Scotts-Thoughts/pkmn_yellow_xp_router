//! Team order calculator core: predicts the order a trainer sends out their
//! party for generations 2-4 (Solodex `utils/teamOrderCalculator.ts`, which
//! documents the decomp sources in `docs/team_order_calculator.md`).
//!
//! Everything here is pure: the type chart, species typings and move types
//! come through [`OrderEnv`], so the algorithm is unit-tested without game
//! data. [`GenEnv`] feeds it from the router's own tables.
//!
//! All generations: the lead is always slot 0. The differences are in the
//! replacement logic after each faint.
//!
//! * Gen 2: every alive slot scores +1 (has a super-effective move and the
//!   player is not super-effective against it), 0 (mutual or neither) or -1.
//!   Lowest-indexed +1, else lowest-indexed 0, else a random pick.
//! * Gen 3: phase 1 picks the alive slot with the highest `typeDmg` (the
//!   damage it takes from the player's STAB, an engine quirk) that also has a
//!   super-effective move; otherwise a damage-based phase 2 that is not
//!   predicted.
//! * Gen 4: stage 1 picks the highest 8-bit offensive type score (the `u8`
//!   wraparound is reproduced) with a super-effective move; otherwise a
//!   damage-based stage 2 that is not predicted.
//!
//! Move power is never consulted: status moves with a super-effective type
//! still count (that matches the games).

use xpr_data::GenData;

/// The result of one chart lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Eff {
    Super,
    NotVery,
    Immune,
    Neutral,
}

/// What the algorithm needs to know about the game.
pub trait OrderEnv {
    /// The chart entry of `atk` against the single type `def`.
    fn eff(&self, atk: &str, def: &str) -> Eff;
    /// A species' typing: the first type and the second when it differs.
    fn species_types(&self, species: &str) -> Option<(String, Option<String>)>;
    /// The type of a move by its name in this game.
    fn move_type(&self, mv: &str) -> Option<String>;
}

/// The router's tables as an [`OrderEnv`].
pub struct GenEnv<'a>(pub &'a GenData);

impl OrderEnv for GenEnv<'_> {
    fn eff(&self, atk: &str, def: &str) -> Eff {
        match self.0.effectiveness(atk, def) {
            Some(xpr_core::consts::SUPER_EFFECTIVE) => Eff::Super,
            Some(xpr_core::consts::NOT_VERY_EFFECTIVE) => Eff::NotVery,
            Some(xpr_core::consts::IMMUNE) => Eff::Immune,
            _ => Eff::Neutral,
        }
    }

    fn species_types(&self, species: &str) -> Option<(String, Option<String>)> {
        let s = self.0.pkmn_db().get_pkmn(species)?;
        let t2 = (s.second_type != s.first_type && !s.second_type.is_empty()).then(|| s.second_type.clone());
        Some((s.first_type.clone(), t2))
    }

    fn move_type(&self, mv: &str) -> Option<String> {
        self.0.move_db().get_move(mv).map(|m| m.move_type.clone())
    }
}

/// One party member as the calculator sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct OrderMon {
    pub species: String,
    pub moves: Vec<String>,
}

/// Gen 2's per-slot score.
pub type SlotScore = i8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepReason {
    Lead,
    Gen2SePick,
    Gen2NeutralPick,
    Gen2ForcedRandom,
    Gen2Random,
    Gen3Phase1,
    Gen3ForcedLinear,
    Gen3Phase2,
    Gen4Stage1,
    Gen4ForcedLinear,
    Gen4Stage2,
}

impl StepReason {
    /// A step the simulation could not decide (the game rolls a random
    /// number or runs the damage-based fallback).
    pub fn is_fallback(self) -> bool {
        matches!(self, StepReason::Gen2Random | StepReason::Gen3Phase2 | StepReason::Gen4Stage2)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SlotEvaluation {
    pub slot: usize,
    pub is_fainted: bool,
    pub has_se_move: bool,
    pub se_move: Option<String>,
    /// gen 2 only
    pub gen2_score: Option<SlotScore>,
    pub player_se_vs_me: Option<bool>,
    /// gen 3 only: engine units (10 = 1x, 20 = 2x, 5 = 0.5x, 0 = 0x)
    pub type_dmg: Option<i64>,
    /// gen 4 only: the true sum of the two type-matchup calls
    pub gen4_score: Option<i64>,
    /// gen 4 only: `gen4_score & 0xFF`, the value the engine compares
    pub gen4_score_8bit: Option<i64>,
}

impl SlotEvaluation {
    fn blank(slot: usize, is_fainted: bool) -> SlotEvaluation {
        SlotEvaluation { slot, is_fainted, has_se_move: false, se_move: None, gen2_score: None, player_se_vs_me: None, type_dmg: None, gen4_score: None, gen4_score_8bit: None }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SimStep {
    /// `None` when the pick is not deterministic (the candidates are in
    /// `fallback_candidates` and the simulation stops)
    pub sent_out: Option<usize>,
    pub reason: StepReason,
    pub evaluations: Vec<SlotEvaluation>,
    pub fallback_candidates: Vec<usize>,
}

/// The generations the calculator knows (`getSupportedGen`).
pub fn supported_gen(generation: u8) -> Option<u8> {
    matches!(generation, 2..=4).then_some(generation)
}

// ---- shared type-chart helpers ----------------------------------------------------------------

fn single_type_multiplier(env: &dyn OrderEnv, atk: &str, def: &str) -> f64 {
    match env.eff(atk, def) {
        Eff::Super => 2.0,
        Eff::NotVery => 0.5,
        Eff::Immune => 0.0,
        Eff::Neutral => 1.0,
    }
}

/// Multiplier of one attacking type against a (possibly dual) defender, with
/// the engines' row-iteration model: each chart row that matches fires once.
fn combined_multiplier(env: &dyn OrderEnv, atk: &str, def1: &str, def2: Option<&str>) -> f64 {
    let mut m = single_type_multiplier(env, atk, def1);
    if let Some(d2) = def2 {
        if d2 != def1 {
            m *= single_type_multiplier(env, atk, d2);
        }
    }
    m
}

/// The first of `moves` that is super-effective against the defender.
fn first_se_move(env: &dyn OrderEnv, moves: &[String], def1: &str, def2: Option<&str>) -> Option<String> {
    for mv in moves {
        if mv.is_empty() {
            continue;
        }
        let Some(t) = env.move_type(mv) else { continue };
        if combined_multiplier(env, &t, def1, def2) > 1.0 {
            return Some(mv.clone());
        }
    }
    None
}

// ---- gen 2 --------------------------------------------------------------------------------------

fn evaluate_slot_gen2(env: &dyn OrderEnv, mon: &OrderMon, slot: usize, p1: &str, p2: Option<&str>, is_fainted: bool) -> SlotEvaluation {
    let mut ev = SlotEvaluation::blank(slot, is_fainted);
    if is_fainted {
        ev.gen2_score = Some(-1);
        ev.player_se_vs_me = Some(false);
        return ev;
    }
    let Some((t1, t2)) = env.species_types(&mon.species) else {
        ev.gen2_score = Some(0);
        ev.player_se_vs_me = Some(false);
        return ev;
    };
    let se_move = first_se_move(env, &mon.moves, p1, p2);
    let has_se = se_move.is_some();
    let mut player_se = combined_multiplier(env, p1, &t1, t2.as_deref()) > 1.0;
    if !player_se {
        if let Some(p2) = p2 {
            if p2 != p1 {
                player_se = combined_multiplier(env, p2, &t1, t2.as_deref()) > 1.0;
            }
        }
    }
    let score: SlotScore = if has_se && !player_se {
        1
    } else if !has_se && player_se {
        -1
    } else {
        0
    };
    ev.has_se_move = has_se;
    ev.se_move = se_move;
    ev.gen2_score = Some(score);
    ev.player_se_vs_me = Some(player_se);
    ev
}

struct Pick {
    slot: Option<usize>,
    reason: StepReason,
    fallback: Vec<usize>,
}

fn pick_replacement_gen2(evals: &[SlotEvaluation]) -> Pick {
    if let Some(s) = evals.iter().find(|s| s.gen2_score == Some(1)) {
        return Pick { slot: Some(s.slot), reason: StepReason::Gen2SePick, fallback: Vec::new() };
    }
    if let Some(s) = evals.iter().find(|s| s.gen2_score == Some(0) && !s.is_fainted) {
        return Pick { slot: Some(s.slot), reason: StepReason::Gen2NeutralPick, fallback: Vec::new() };
    }
    let candidates: Vec<usize> = evals.iter().filter(|s| !s.is_fainted).map(|s| s.slot).collect();
    if candidates.len() == 1 {
        return Pick { slot: Some(candidates[0]), reason: StepReason::Gen2ForcedRandom, fallback: Vec::new() };
    }
    Pick { slot: None, reason: StepReason::Gen2Random, fallback: candidates }
}

// ---- gen 3 --------------------------------------------------------------------------------------

/// One call of `ModulateByTypeEffectiveness`: every chart row matching the
/// attacker and one of the (deduplicated) candidate types multiplies the
/// running `type_dmg` by `row / 10` with integer division.
fn apply_modulate(env: &dyn OrderEnv, atk: &str, def1: &str, def2: &str, mut type_dmg: i64) -> i64 {
    let defs: Vec<&str> = if def1 == def2 { vec![def1] } else { vec![def1, def2] };
    for dt in defs {
        let chart_mult = match env.eff(atk, dt) {
            Eff::Super => 20,
            Eff::NotVery => 5,
            Eff::Immune => 0,
            Eff::Neutral => -1,
        };
        if chart_mult >= 0 {
            type_dmg = type_dmg * chart_mult / 10;
        }
    }
    type_dmg
}

fn evaluate_slot_gen3(env: &dyn OrderEnv, mon: &OrderMon, slot: usize, opp1: &str, opp2: &str, is_fainted: bool) -> SlotEvaluation {
    let mut ev = SlotEvaluation::blank(slot, is_fainted);
    if is_fainted {
        ev.type_dmg = Some(0);
        return ev;
    }
    // gSpeciesInfo gives single-typed species type1 == type2
    let (t1, t2) = match env.species_types(&mon.species) {
        Some((a, b)) => {
            let b = b.unwrap_or_else(|| a.clone());
            (a, b)
        }
        None => (String::new(), String::new()),
    };
    let mut type_dmg = 10;
    type_dmg = apply_modulate(env, opp1, &t1, &t2, type_dmg);
    type_dmg = apply_modulate(env, opp2, &t1, &t2, type_dmg);
    // the super-effective move check is the ordinary type calculation: a
    // single-typed opponent has no second type
    let opp2_for_move = if opp2 == opp1 { None } else { Some(opp2) };
    let se_move = first_se_move(env, &mon.moves, opp1, opp2_for_move);
    ev.has_se_move = se_move.is_some();
    ev.se_move = se_move;
    ev.type_dmg = Some(type_dmg);
    ev
}

fn pick_replacement_gen3(evals: &[SlotEvaluation]) -> Pick {
    let mut pool: Vec<&SlotEvaluation> = evals.iter().filter(|e| !e.is_fainted && e.type_dmg.unwrap_or(0) > 0).collect();
    pool.sort_by(|a, b| b.type_dmg.unwrap_or(0).cmp(&a.type_dmg.unwrap_or(0)).then(a.slot.cmp(&b.slot)));
    if let Some(ev) = pool.iter().find(|e| e.has_se_move) {
        return Pick { slot: Some(ev.slot), reason: StepReason::Gen3Phase1, fallback: Vec::new() };
    }
    let remaining: Vec<usize> = evals.iter().filter(|e| !e.is_fainted).map(|e| e.slot).collect();
    if remaining.len() == 1 {
        return Pick { slot: Some(remaining[0]), reason: StepReason::Gen3ForcedLinear, fallback: Vec::new() };
    }
    Pick { slot: None, reason: StepReason::Gen3Phase2, fallback: remaining }
}

// ---- gen 4 --------------------------------------------------------------------------------------

/// `BattleSystem_TypeMatchupMultiplier`: starts at 40 (1x) and applies the
/// super-effective / not-very-effective chart rows. The immunity rows are
/// not in this routine's chart (a Ground attacker against a Flying defender
/// contributes x1, not x0).
fn gen4_type_matchup_multiplier(env: &dyn OrderEnv, atk: &str, def1: &str, def2: &str) -> i64 {
    let mut mul = 40;
    let defs: Vec<&str> = if def1 == def2 { vec![def1] } else { vec![def1, def2] };
    for dt in defs {
        let chart_mult = match env.eff(atk, dt) {
            Eff::Super => 20,
            Eff::NotVery => 5,
            Eff::Immune | Eff::Neutral => -1,
        };
        if chart_mult >= 0 {
            mul = mul * chart_mult / 10;
        }
    }
    mul
}

fn evaluate_slot_gen4(env: &dyn OrderEnv, mon: &OrderMon, slot: usize, def1: &str, def2: &str, is_fainted: bool) -> SlotEvaluation {
    let mut ev = SlotEvaluation::blank(slot, is_fainted);
    if is_fainted {
        ev.gen4_score = Some(0);
        ev.gen4_score_8bit = Some(0);
        return ev;
    }
    // the engine treats a single type as type1 == type2: both passes use it
    let (m1, m2) = match env.species_types(&mon.species) {
        Some((a, b)) => {
            let b = b.unwrap_or_else(|| a.clone());
            (a, b)
        }
        None => (String::new(), String::new()),
    };
    let score = gen4_type_matchup_multiplier(env, &m1, def1, def2) + gen4_type_matchup_multiplier(env, &m2, def1, def2);
    let def2_for_move = if def2 == def1 { None } else { Some(def2) };
    let se_move = first_se_move(env, &mon.moves, def1, def2_for_move);
    ev.has_se_move = se_move.is_some();
    ev.se_move = se_move;
    ev.gen4_score = Some(score);
    ev.gen4_score_8bit = Some(score & 0xFF);
    ev
}

fn pick_replacement_gen4(evals: &[SlotEvaluation]) -> Pick {
    // strict `<` against a starting maximum of 0: a candidate whose 8-bit
    // score is 0 (a true score of 256 wraps) is never picked
    let mut pool: Vec<&SlotEvaluation> = evals.iter().filter(|e| !e.is_fainted && e.gen4_score_8bit.unwrap_or(0) > 0).collect();
    pool.sort_by(|a, b| b.gen4_score_8bit.unwrap_or(0).cmp(&a.gen4_score_8bit.unwrap_or(0)).then(a.slot.cmp(&b.slot)));
    if let Some(ev) = pool.iter().find(|e| e.has_se_move) {
        return Pick { slot: Some(ev.slot), reason: StepReason::Gen4Stage1, fallback: Vec::new() };
    }
    let remaining: Vec<usize> = evals.iter().filter(|e| !e.is_fainted).map(|e| e.slot).collect();
    if remaining.len() == 1 {
        return Pick { slot: Some(remaining[0]), reason: StepReason::Gen4ForcedLinear, fallback: Vec::new() };
    }
    Pick { slot: None, reason: StepReason::Gen4Stage2, fallback: remaining }
}

// ---- simulation ---------------------------------------------------------------------------------

/// Run the send-out order: the lead, then a replacement after each faint,
/// stopping at the first step the game decides at random.
fn simulate(party: &[OrderMon], evaluate: &dyn Fn(&OrderMon, usize, bool) -> SlotEvaluation, pick: &dyn Fn(&[SlotEvaluation]) -> Pick) -> Vec<SimStep> {
    if party.is_empty() {
        return Vec::new();
    }
    let mut steps = vec![SimStep { sent_out: Some(0), reason: StepReason::Lead, evaluations: Vec::new(), fallback_candidates: Vec::new() }];
    let mut fainted: Vec<bool> = vec![false; party.len()];
    let mut current = 0usize;
    loop {
        fainted[current] = true;
        if fainted.iter().filter(|f| **f).count() >= party.len() {
            break;
        }
        let evals: Vec<SlotEvaluation> = party.iter().enumerate().map(|(i, p)| evaluate(p, i, fainted[i])).collect();
        let p = pick(&evals);
        steps.push(SimStep { sent_out: p.slot, reason: p.reason, evaluations: evals, fallback_candidates: p.fallback });
        match p.slot {
            Some(s) => current = s,
            None => break,
        }
    }
    steps
}

/// The predicted send-out order of `party` against a player Pokémon of
/// `player_type1` / `player_type2` in a generation 2-4 game; empty for any
/// other generation.
pub fn simulate_order(env: &dyn OrderEnv, party: &[OrderMon], generation: u8, player_type1: &str, player_type2: Option<&str>) -> Vec<SimStep> {
    match supported_gen(generation) {
        Some(4) => {
            // a single-typed player has both type slots the same
            let eff2 = player_type2.unwrap_or(player_type1);
            simulate(party, &|p, i, f| evaluate_slot_gen4(env, p, i, player_type1, eff2, f), &pick_replacement_gen4)
        }
        Some(3) => {
            // single-typed players have type1 == type2, so the modulate call
            // runs twice with the same attacker and the multiplier doubles
            let eff2 = player_type2.unwrap_or(player_type1);
            simulate(party, &|p, i, f| evaluate_slot_gen3(env, p, i, player_type1, eff2, f), &pick_replacement_gen3)
        }
        Some(2) => simulate(party, &|p, i, f| evaluate_slot_gen2(env, p, i, player_type1, player_type2, f), &pick_replacement_gen2),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A tiny chart, species table and move table.
    #[derive(Default)]
    struct Mini {
        chart: HashMap<(&'static str, &'static str), Eff>,
        species: HashMap<&'static str, (&'static str, Option<&'static str>)>,
        moves: HashMap<&'static str, &'static str>,
    }

    impl OrderEnv for Mini {
        fn eff(&self, atk: &str, def: &str) -> Eff {
            self.chart.iter().find(|((a, d), _)| *a == atk && *d == def).map(|(_, e)| *e).unwrap_or(Eff::Neutral)
        }
        fn species_types(&self, species: &str) -> Option<(String, Option<String>)> {
            self.species.get(species).map(|(a, b)| (a.to_string(), b.map(|s| s.to_string())))
        }
        fn move_type(&self, mv: &str) -> Option<String> {
            self.moves.get(mv).map(|s| s.to_string())
        }
    }

    fn mini() -> Mini {
        let mut m = Mini::default();
        use Eff::*;
        for (a, d, e) in [
            ("Water", "Fire", Super),
            ("Water", "Rock", Super),
            ("Water", "Grass", NotVery),
            ("Fire", "Grass", Super),
            ("Fire", "Water", NotVery),
            ("Grass", "Water", Super),
            ("Grass", "Fire", NotVery),
            ("Electric", "Water", Super),
            ("Electric", "Flying", Super),
            ("Electric", "Ground", Immune),
            ("Ground", "Electric", Super),
            ("Ground", "Fire", Super),
            ("Ground", "Flying", Immune),
            ("Rock", "Fire", Super),
            ("Rock", "Flying", Super),
            ("Rock", "Ground", NotVery),
            ("Normal", "Rock", NotVery),
        ] {
            m.chart.insert((a, d), e);
        }
        for (s, a, b) in [
            ("Charmander", "Fire", None),
            ("Squirtle", "Water", None),
            ("Bulbasaur", "Grass", Some("Poison")),
            ("Pikachu", "Electric", None),
            ("Geodude", "Rock", Some("Ground")),
            ("Pidgey", "Normal", Some("Flying")),
            ("Gyarados", "Water", Some("Flying")),
            ("Onix", "Rock", Some("Ground")),
        ] {
            m.species.insert(s, (a, b));
        }
        for (mv, t) in [("Ember", "Fire"), ("Water Gun", "Water"), ("Vine Whip", "Grass"), ("Thunderbolt", "Electric"), ("Tackle", "Normal"), ("Earthquake", "Ground"), ("Rock Throw", "Rock"), ("Growl", "Normal")] {
            m.moves.insert(mv, t);
        }
        m
    }

    fn mon(species: &str, moves: &[&str]) -> OrderMon {
        OrderMon { species: species.to_string(), moves: moves.iter().map(|s| s.to_string()).collect() }
    }

    fn order(steps: &[SimStep]) -> Vec<Option<usize>> {
        steps.iter().map(|s| s.sent_out).collect()
    }

    #[test]
    fn supported_gens_are_two_to_four() {
        assert_eq!(supported_gen(1), None);
        assert_eq!(supported_gen(2), Some(2));
        assert_eq!(supported_gen(4), Some(4));
        assert_eq!(supported_gen(5), None);
        assert!(simulate_order(&mini(), &[mon("Pidgey", &[])], 5, "Water", None).is_empty());
        assert!(simulate_order(&mini(), &[], 3, "Water", None).is_empty());
    }

    #[test]
    fn single_mon_party_only_leads() {
        let steps = simulate_order(&mini(), &[mon("Pidgey", &["Tackle"])], 2, "Water", None);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].reason, StepReason::Lead);
        assert_eq!(steps[0].sent_out, Some(0));
    }

    #[test]
    fn gen2_prefers_super_effective_without_a_counter() {
        let m = mini();
        // player is Water: Charmander (Fire) is SE'd by Water; Pikachu has Thunderbolt (SE vs Water)
        // and Water is neutral vs Electric -> +1; Bulbasaur has Vine Whip (SE vs Water) but Water
        // is NVE vs Grass, so the player is not SE against it -> +1 too, but slot 1 comes first
        let party = [mon("Squirtle", &["Water Gun"]), mon("Pikachu", &["Thunderbolt"]), mon("Bulbasaur", &["Vine Whip"]), mon("Charmander", &["Ember"])];
        let steps = simulate_order(&m, &party, 2, "Water", None);
        assert_eq!(steps[0].reason, StepReason::Lead);
        assert_eq!(steps[1].sent_out, Some(1));
        assert_eq!(steps[1].reason, StepReason::Gen2SePick);
        assert_eq!(steps[1].evaluations[1].gen2_score, Some(1));
        assert_eq!(steps[1].evaluations[1].se_move.as_deref(), Some("Thunderbolt"));
        // the fainted lead scores -1
        assert_eq!(steps[1].evaluations[0].gen2_score, Some(-1));
        // next: Bulbasaur (+1)
        assert_eq!(steps[2].sent_out, Some(2));
        // Charmander: no SE move vs Water (Ember is NVE), and Water is SE vs Fire -> -1, the last
        // one standing is picked by elimination
        assert_eq!(steps[3].sent_out, Some(3));
        assert_eq!(steps[3].reason, StepReason::Gen2ForcedRandom);
        assert_eq!(steps.len(), 4);
    }

    #[test]
    fn gen2_mutual_super_effective_collapses_to_neutral() {
        let m = mini();
        // player Water. Charmander has Thunderbolt (SE vs Water) but Water is SE vs Fire: mutual -> 0.
        // Pikachu has no SE move and Water is neutral vs Electric: neither -> 0.
        // Geodude (Rock/Ground) has no SE move and Water is SE vs Rock: -1.
        let party = [mon("Squirtle", &[]), mon("Charmander", &["Thunderbolt"]), mon("Pikachu", &["Tackle"]), mon("Geodude", &["Tackle"])];
        let steps = simulate_order(&m, &party, 2, "Water", None);
        let ev = &steps[1].evaluations;
        assert_eq!((ev[1].has_se_move, ev[1].player_se_vs_me, ev[1].gen2_score), (true, Some(true), Some(0)));
        assert_eq!((ev[2].has_se_move, ev[2].player_se_vs_me, ev[2].gen2_score), (false, Some(false), Some(0)));
        assert_eq!(ev[3].gen2_score, Some(-1));
        // no +1: the lowest-indexed 0 wins, then the next 0, then the last one by elimination
        assert_eq!(order(&steps), vec![Some(0), Some(1), Some(2), Some(3)]);
        assert_eq!(steps[1].reason, StepReason::Gen2NeutralPick);
        assert_eq!(steps[2].reason, StepReason::Gen2NeutralPick);
        assert_eq!(steps[3].reason, StepReason::Gen2ForcedRandom);
    }

    #[test]
    fn gen2_all_negative_with_several_left_is_a_random_step() {
        let m = mini();
        // Water vs Rock/Ground: SE, and neither has an SE move: both -1 -> the game rolls a random number
        let party = [mon("Squirtle", &[]), mon("Geodude", &["Tackle"]), mon("Onix", &["Tackle"])];
        let steps = simulate_order(&m, &party, 2, "Water", None);
        assert_eq!(steps.len(), 2, "the simulation stops at a random step");
        assert_eq!(steps[1].sent_out, None);
        assert_eq!(steps[1].reason, StepReason::Gen2Random);
        assert_eq!(steps[1].fallback_candidates, vec![1, 2]);
        // a dual-typed player is checked with both of its types
        let party = [mon("Squirtle", &[]), mon("Pikachu", &["Tackle"]), mon("Charmander", &["Tackle"])];
        // Water/Ground: Ground is SE vs Fire (Charmander) and Electric (Pikachu): both -1
        let steps = simulate_order(&m, &party, 2, "Water", Some("Ground"));
        assert_eq!(steps[1].evaluations[1].player_se_vs_me, Some(true));
        assert_eq!(steps[1].evaluations[2].player_se_vs_me, Some(true));
        assert_eq!(steps[1].reason, StepReason::Gen2Random);
    }

    #[test]
    fn gen2_mutual_se_scores_zero_and_is_picked_before_negative() {
        let m = mini();
        // player Grass: Charmander's Ember is SE vs Grass, and Grass is NVE vs Fire -> +1.
        // Squirtle with Tackle: no SE move; Grass is SE vs Water -> -1.
        // Bulbasaur (Grass/Poison) with Vine Whip: not SE vs Grass; Grass vs Grass neutral -> 0.
        let party = [mon("Pidgey", &["Tackle"]), mon("Squirtle", &["Tackle"]), mon("Bulbasaur", &["Vine Whip"])];
        let steps = simulate_order(&m, &party, 2, "Grass", None);
        assert_eq!(steps[1].evaluations[1].gen2_score, Some(-1));
        assert_eq!(steps[1].evaluations[2].gen2_score, Some(0));
        assert_eq!(steps[1].sent_out, Some(2));
        assert_eq!(steps[1].reason, StepReason::Gen2NeutralPick);
        // then Squirtle is the only one left
        assert_eq!(steps[2].sent_out, Some(1));
        assert_eq!(steps[2].reason, StepReason::Gen2ForcedRandom);
    }

    #[test]
    fn gen3_single_typed_player_double_applies_the_modifier() {
        let m = mini();
        // Water player: Charmander (Fire) takes 2x from the first call and 2x again from the
        // second (single type duplicated) -> 40
        let party = [mon("Squirtle", &[]), mon("Charmander", &["Ember"])];
        let steps = simulate_order(&m, &party, 3, "Water", None);
        // Charmander has no SE move vs Water (Ember is NVE): phase 1 finds nothing; one left ->
        // forced
        assert_eq!(steps[1].reason, StepReason::Gen3ForcedLinear);
        assert_eq!(steps[1].evaluations[1].type_dmg, Some(40));
        // dual-typed player Water/Flying: two different attackers, each applied once: Fire takes 2x
        // from Water, neutral from Flying -> 20
        let steps = simulate_order(&m, &party, 3, "Water", Some("Flying"));
        assert_eq!(steps[1].evaluations[1].type_dmg, Some(20));
    }

    #[test]
    fn gen3_picks_the_highest_type_damage_with_a_se_move_and_ties_go_to_the_lowest_slot() {
        let m = mini();
        // Water player. Candidates: slot 1 Bulbasaur (Grass/Poison, Water NVE: 10*5/10=5, then 2 -> floor(25/10)=2),
        // slot 2 Pikachu (neutral: 10 -> 10), slot 3 Geodude (Rock/Ground: Water SE vs Rock -> 20 -> 40),
        // slot 4 Charmander (Fire: 40).
        let party = [
            mon("Squirtle", &[]),
            mon("Bulbasaur", &["Vine Whip"]),
            mon("Pikachu", &["Thunderbolt"]),
            mon("Geodude", &["Tackle"]),
            mon("Charmander", &["Ember", "Rock Throw"]),
        ];
        let steps = simulate_order(&m, &party, 3, "Water", None);
        let ev = &steps[1].evaluations;
        assert_eq!(ev[1].type_dmg, Some(2));
        assert_eq!(ev[2].type_dmg, Some(10));
        assert_eq!(ev[3].type_dmg, Some(40));
        assert_eq!(ev[4].type_dmg, Some(40));
        // Geodude and Charmander tie at 40 but neither has an SE move vs Water (Rock Throw is
        // neutral vs Water here); Pikachu (10) has Thunderbolt: it is picked
        assert_eq!(steps[1].sent_out, Some(2));
        assert_eq!(steps[1].reason, StepReason::Gen3Phase1);
        assert_eq!(ev[2].se_move.as_deref(), Some("Thunderbolt"));
    }

    #[test]
    fn gen3_immune_candidates_are_never_phase_one_picks() {
        let m = mini();
        // Electric player vs Geodude (Rock/Ground): Ground is immune -> typeDmg 0
        let party = [mon("Squirtle", &[]), mon("Geodude", &["Water Gun", "Rock Throw"]), mon("Charmander", &["Rock Throw"])];
        let steps = simulate_order(&m, &party, 3, "Electric", None);
        assert_eq!(steps[1].evaluations[1].type_dmg, Some(0));
        // phase 1: Charmander is eligible but has no SE move vs Electric; Geodude's Water Gun is
        // SE but its typeDmg is 0 -> phase 2 fallback with both candidates
        assert_eq!(steps[1].reason, StepReason::Gen3Phase2);
        assert_eq!(steps[1].sent_out, None);
        assert_eq!(steps[1].fallback_candidates, vec![1, 2]);
    }

    #[test]
    fn gen4_scores_wrap_to_eight_bits() {
        let m = mini();
        // defender (the player) is Water/Flying. Candidate Pikachu (Electric, mono): both passes
        // compute Electric vs Water (x2) and vs Flying (x2): 40*2*2 = 160 each -> 320; 320 & 255 = 64.
        let party = [mon("Squirtle", &[]), mon("Pikachu", &["Thunderbolt"]), mon("Charmander", &["Ember"])];
        let steps = simulate_order(&m, &party, 4, "Water", Some("Flying"));
        let p = &steps[1].evaluations[1];
        assert_eq!(p.gen4_score, Some(320));
        assert_eq!(p.gen4_score_8bit, Some(64));
        // Charmander: Fire vs Water NVE (x0.5 -> 20), vs Flying neutral: 20 per pass -> 40
        let c = &steps[1].evaluations[2];
        assert_eq!(c.gen4_score, Some(40));
        // the wrapped Pikachu (64) still beats Charmander (40), and has an SE move
        assert_eq!(steps[1].sent_out, Some(1));
        assert_eq!(steps[1].reason, StepReason::Gen4Stage1);
    }

    #[test]
    fn gen4_ignores_immunity_rows_in_the_score() {
        let m = mini();
        // Ground attacker vs Flying defender contributes x1 (no x0): Onix (Rock/Ground) vs a
        // Flying/Water player: Rock is SE vs Flying (x2 -> 80), Ground vs Flying immune (skipped)
        // vs Water neutral -> 40; so the two passes give 80 and 40
        let party = [mon("Squirtle", &[]), mon("Onix", &["Rock Throw"])];
        let steps = simulate_order(&m, &party, 4, "Flying", Some("Water"));
        let o = &steps[1].evaluations[1];
        assert_eq!(o.gen4_score, Some(120));
        assert_eq!(o.gen4_score_8bit, Some(120));
        assert!(o.has_se_move);
        // forced by elimination? no: stage 1 picks it first (it is the only candidate with a score)
        assert_eq!(steps[1].sent_out, Some(1));
    }

    #[test]
    fn gen4_stage_two_fallback_stops_the_simulation() {
        let m = mini();
        let party = [mon("Squirtle", &[]), mon("Pidgey", &["Tackle"]), mon("Pikachu", &["Growl"])];
        let steps = simulate_order(&m, &party, 4, "Grass", None);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[1].reason, StepReason::Gen4Stage2);
        assert_eq!(steps[1].fallback_candidates, vec![1, 2]);
        assert!(steps[1].reason.is_fallback());
    }

    #[test]
    fn full_orders_run_to_the_last_mon() {
        let m = mini();
        let party = [mon("Squirtle", &["Water Gun"]), mon("Pikachu", &["Thunderbolt"]), mon("Bulbasaur", &["Vine Whip"])];
        let steps = simulate_order(&m, &party, 2, "Water", None);
        assert_eq!(order(&steps), vec![Some(0), Some(1), Some(2)]);
    }
}

#[cfg(test)]
mod data_tests {
    use super::*;
    use crate::testkit::test_registry;

    fn mon_of(gen: &GenData, trainer: &str, slot: usize) -> OrderMon {
        let t = gen.trainer_db().get_trainer(trainer).unwrap();
        let p = &t.pkmn[slot];
        OrderMon { species: p.name.clone(), moves: p.move_list.iter().flatten().cloned().collect() }
    }

    /// The router's type charts match the Dex's (Solodex's) for generations 2-5. Gen 1 differs in one
    /// entry, where the router follows the game (Fighting does nothing to Ghost in Red / Blue; the
    /// calculator is not offered for gen 1).
    #[test]
    fn router_charts_match_the_dex_charts() {
        let reg = test_registry();
        let red = reg.get_version("Red").unwrap();
        assert_eq!(GenEnv(&red).eff("Fighting", "Ghost"), Eff::Immune);
        assert_eq!(xpr_dex::offensive_multiplier("Fighting", "Ghost", "Red and Blue"), 1.0);
        for (version, dex_game) in [("Crystal", "Crystal"), ("Emerald", "Emerald"), ("Platinum", "Platinum"), ("Black", "Black")] {
            let gen = reg.get_version(version).unwrap();
            let env = GenEnv(&gen);
            let types: Vec<String> = gen.supported_types().into_iter().filter(|t| t != "none").collect();
            let mut diffs = Vec::new();
            for atk in &types {
                for def in &types {
                    let router = match env.eff(atk, def) {
                        Eff::Super => 2.0,
                        Eff::NotVery => 0.5,
                        Eff::Immune => 0.0,
                        Eff::Neutral => 1.0,
                    };
                    let dex = xpr_dex::offensive_multiplier(atk, def, dex_game);
                    if router != dex {
                        diffs.push(format!("{} {}->{}: router {} dex {}", version, atk, def, router, dex));
                    }
                }
            }
            assert!(diffs.is_empty(), "{:?}", diffs);
        }
    }

    #[test]
    fn crystal_falkner_against_an_electric_player() {
        let reg = test_registry();
        let gen = reg.get_version("Crystal").unwrap();
        let party: Vec<OrderMon> = (0..2).map(|i| mon_of(&gen, "Leader Falkner", i)).collect();
        let steps = simulate_order(&GenEnv(&gen), &party, gen.get_generation(), "Electric", None);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[1].sent_out, Some(1));
        // Pidgeotto's Mud Slap (Ground) is super-effective against Electric, and Electric is super-effective
        // against its Flying type: a mutual matchup
        let ev = &steps[1].evaluations[1];
        assert_eq!(ev.se_move.as_deref(), Some("Mud Slap"));
        assert_eq!(ev.player_se_vs_me, Some(true));
        assert_eq!(ev.gen2_score, Some(0));
        // a mutual matchup scores 0, which the neutral rule picks (before the by-elimination rule)
        assert_eq!(steps[1].reason, StepReason::Gen2NeutralPick);
    }

    #[test]
    fn every_gen_2_to_4_gym_runs_through_every_player_type() {
        // smoke test over the real data: no panic, the lead is slot 0 and no slot is sent out twice
        let reg = test_registry();
        for version in ["Crystal", "Emerald", "FireRed", "Platinum", "HeartGold"] {
            let gen = reg.get_version(version).unwrap();
            let env = GenEnv(&gen);
            for t in gen.trainer_db().iter().filter(|t| gen.get_fight_category(&t.name) == Some("gym_leader")) {
                let party: Vec<OrderMon> = (0..t.pkmn.len()).map(|i| mon_of(&gen, &t.name, i)).collect();
                for p1 in ["Normal", "Water", "Ground", "Dark"] {
                    for p2 in [None, Some("Flying"), Some("Steel")] {
                        let steps = simulate_order(&env, &party, gen.get_generation(), p1, p2);
                        assert_eq!(steps[0].sent_out, Some(0));
                        let mut seen = std::collections::HashSet::new();
                        for s in &steps {
                            if let Some(slot) = s.sent_out {
                                assert!(seen.insert(slot), "{} {} sent slot {} twice", version, t.name, slot);
                            }
                        }
                    }
                }
            }
        }
    }
}
