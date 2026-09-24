//! Damage-calc sweep harness (verification tooling, not part of the app).
//!
//! `cargo run --release -p xpr-calc --example sweep -- <cases.json> <out.json>`
//! runs every case through the real calculator and writes the resolved
//! inputs (battle stats, species types, move data) next to the results, so
//! an external reference implementation of the game's formula can be
//! compared against it without re-deriving stats.
//!
//! `cargo run --release -p xpr-calc --example sweep -- --options <version>`
//! prints the custom-move-data dropdown table of that version.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Map, Value};

use xpr_calc::{calculate_damage, get_crit_rate, get_move_accuracy, DamageArgs, DamageRange};
use xpr_data::model::{EnemyPkmn, FieldStatus, StageModifiers, StatBlock};
use xpr_data::{stats, GenData, Registry};

fn stages(v: Option<&Value>) -> StageModifiers {
    let mut s = StageModifiers::default();
    if let Some(o) = v.and_then(|v| v.as_object()) {
        let g = |k: &str| o.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
        s.attack_stage = g("atk");
        s.defense_stage = g("def");
        s.special_attack_stage = g("spa");
        s.special_defense_stage = g("spd");
        s.speed_stage = g("spe");
        s.accuracy_stage = g("acc");
        s.evasion_stage = g("eva");
        s.attack_badge_boosts = g("atk_bb");
        s.defense_badge_boosts = g("def_bb");
        s.speed_badge_boosts = g("spe_bb");
        s.special_badge_boosts = g("spc_bb");
    }
    s
}

fn field(v: Option<&Value>) -> FieldStatus {
    let mut f = FieldStatus::default();
    if let Some(o) = v.and_then(|v| v.as_object()) {
        let g = |k: &str| o.get(k).and_then(|x| x.as_bool()).unwrap_or(false);
        f.light_screen = g("light_screen");
        f.reflect = g("reflect");
        f.gravity = g("gravity");
        f.magnet_rise = g("magnet_rise");
        f.miracle_eye = g("miracle_eye");
        f.power_trick = g("power_trick");
        f.roost = g("roost");
        f.tailwind = g("tailwind");
        f.trick_room = g("trick_room");
        f.worry_seed = g("worry_seed");
        f.gastro_acid = g("gastro_acid");
        f.slow_start = g("slow_start");
    }
    f
}

fn mon(gen: &GenData, v: &Value) -> EnemyPkmn {
    let name = v["name"].as_str().expect("mon name");
    let level = v["level"].as_i64().expect("mon level");
    let mut m = match v["wild_dv"].as_i64() {
        Some(dv) => gen.create_wild_pkmn(name, level, dv),
        None => gen.create_trainer_pkmn(name, level),
    }
    .unwrap_or_else(|| panic!("unknown species {} in {}", name, gen.version_name()));
    if let Some(item) = v["held_item"].as_str() {
        m.held_item = Some(item.to_string());
    }
    if let Some(ab) = v["ability"].as_str() {
        m.ability = ab.to_string();
    }
    if let Some(b) = v["badges"].as_array() {
        let slots: Vec<&str> = b.iter().filter_map(|x| x.as_str()).collect();
        m.badges = Some(gen.make_badge_list().with_slots(&slots));
    }
    if let Some(hp) = v["cur_hp"].as_i64() {
        m.cur_stats.hp = hp;
    }
    m
}

fn stat_json(s: &StatBlock) -> Value {
    json!({"hp": s.hp, "atk": s.attack, "def": s.defense, "spa": s.special_attack, "spd": s.special_defense, "spe": s.speed})
}

fn range_json(r: &Option<DamageRange>) -> Value {
    match r {
        None => Value::Null,
        Some(r) => Value::Array(r.damage_vals.iter().map(|(d, c)| json!([d, c])).collect()),
    }
}

fn mon_json(gen: &GenData, m: &EnemyPkmn, st: &StageModifiers, f: &FieldStatus) -> Value {
    let species = gen.pkmn_db().get_pkmn(&m.name).expect("species");
    let raw = stats::calc_battle_stats(&m.base_stats, m.level, &m.dvs, &m.stat_xp, &StageModifiers::default(), None, m.nature, m.held_item.as_deref(), false, None);
    let staged = m.get_battle_stats(st, false, Some(f));
    // the gen 1/2 "unboosted" reload: no stages, no badges
    let crit = m.get_battle_stats(&StageModifiers::default(), true, Some(f));
    let badge_flags = m.badges.as_ref().map(|b| {
        let slots: Vec<&str> = b.slots_with_state().into_iter().filter(|(_, on)| *on).map(|(s, _)| s).collect();
        json!({"atk": b.is_attack_boosted(), "def": b.is_defense_boosted(), "spa": b.is_special_attack_boosted(), "spd": b.is_special_defense_boosted(), "spe": b.is_speed_boosted(), "slots": slots})
    });
    json!({
        "name": m.name,
        "level": m.level,
        "types": [species.first_type, species.second_type],
        "base": stat_json(&species.stats),
        "weight": species.weight,
        "dvs": stat_json(&m.dvs),
        "cur_hp": m.cur_stats.hp,
        "raw": stat_json(&raw),
        "staged": stat_json(&staged),
        "crit": stat_json(&crit),
        "badges": badge_flags,
        "held_item": m.held_item,
        "ability": m.ability,
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let root = xpr_core::consts::find_source_root().expect("source root");
    let reg = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));

    if args.get(1).map(|s| s.as_str()) == Some("--options") {
        let gen = reg.get_version(&args[2]).expect("version");
        let mut out = Map::new();
        for m in gen.move_db().iter() {
            if let Some(opts) = gen.get_move_custom_data(&m.name) {
                out.insert(m.name.clone(), json!(opts));
            }
        }
        println!("{}", serde_json::to_string_pretty(&Value::Object(out)).unwrap());
        return;
    }

    let cases_path = &args[1];
    let out_path = &args[2];
    let text = std::fs::read_to_string(cases_path).expect("read cases");
    let cases: Vec<Value> = serde_json::from_str(&text).expect("cases json");
    let mut results: Vec<Value> = Vec::with_capacity(cases.len());
    for case in &cases {
        let version = case["version"].as_str().expect("version");
        let gen = reg.get_version(version).unwrap_or_else(|e| panic!("load {}: {}", version, e));
        let a = mon(&gen, &case["attacker"]);
        let d = mon(&gen, &case["defender"]);
        let move_name = case["move"].as_str().expect("move");
        let mv = match gen.move_db().get_move(move_name) {
            Some(m) => (**m).clone(),
            None => {
                results.push(json!({"id": case["id"], "error": format!("unknown move {}", move_name)}));
                continue;
            }
        };
        let a_stages = stages(case.get("attacking_stages"));
        let d_stages = stages(case.get("defending_stages"));
        let a_field = field(case.get("attacking_field"));
        let d_field = field(case.get("defending_field"));
        let custom = case["custom"].as_str().unwrap_or("").to_string();
        let weather = case["weather"].as_str().unwrap_or(xpr_core::consts::WEATHER_NONE).to_string();
        let doubles = case["doubles"].as_bool().unwrap_or(false);
        let attacker_is_enemy = case["attacker_is_enemy"].as_bool().unwrap_or(false);

        let mut dargs = DamageArgs {
            attacking: &a,
            mv: &mv,
            defending: &d,
            attacking_stages: Some(&a_stages),
            defending_stages: Some(&d_stages),
            attacking_field: Some(&a_field),
            defending_field: Some(&d_field),
            is_crit: false,
            custom_move_data: &custom,
            weather: &weather,
            is_double_battle: doubles,
            attacking_battle_stats: None,
            defending_battle_stats: None,
            attacker_is_enemy,
            is_wild_battle: case["wild"].as_bool().unwrap_or(false),
        };
        let normal = calculate_damage(&gen, &dargs);
        dargs.is_crit = true;
        let crit = calculate_damage(&gen, &dargs);
        dargs.is_crit = false;
        let crit_rate = get_crit_rate(&gen, &dargs);
        let accuracy = get_move_accuracy(&gen, &dargs);

        results.push(json!({
            "id": case["id"],
            "version": version,
            "gen": gen.gen.number(),
            "attacker": mon_json(&gen, &a, &a_stages, &a_field),
            "defender": mon_json(&gen, &d, &d_stages, &d_field),
            "move": {
                "name": mv.name,
                "type": mv.move_type,
                "power": mv.base_power,
                "accuracy": mv.accuracy,
                "flavors": mv.attack_flavor,
                "category": mv.category,
                "targeting": mv.targeting,
            },
            "custom": custom,
            "weather": weather,
            "doubles": doubles,
            "attacking_stages": case.get("attacking_stages").cloned().unwrap_or(Value::Null),
            "defending_stages": case.get("defending_stages").cloned().unwrap_or(Value::Null),
            "attacking_field": case.get("attacking_field").cloned().unwrap_or(Value::Null),
            "defending_field": case.get("defending_field").cloned().unwrap_or(Value::Null),
            "attacker_is_enemy": attacker_is_enemy,
            "wild": case["wild"].as_bool().unwrap_or(false),
            "damage": range_json(&normal),
            "crit_damage": range_json(&crit),
            "crit_rate": crit_rate,
            "accuracy": accuracy,
        }));
    }
    std::fs::write(out_path, serde_json::to_string(&results).unwrap()).expect("write out");
    eprintln!("wrote {} results", results.len());
}
