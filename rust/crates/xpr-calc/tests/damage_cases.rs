//! Replays `docs/rust_port/golden/damage_cases.json`: every damage-calc call
//! the Python damage tests make (recorded by
//! `docs/rust_port/golden/record_damage_cases.py`) must give the identical
//! result here. Regenerate the corpus with
//! `py -3.14 docs/rust_port/golden/record_damage_cases.py` after changing
//! the Python tests.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;

use xpr_calc::{calculate_damage, get_crit_rate, get_move_accuracy, DamageArgs};
use xpr_data::model::{CustomMoveData, EnemyPkmn, FieldStatus, Gen, Move, Nature, StageModifiers, StatBlock};
use xpr_data::{GenData, Registry};

fn corpus_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../docs/rust_port/golden/damage_cases.json")
}

fn stat_block(gen: Gen, v: &Value) -> StatBlock {
    let g = |k: &str| v.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
    StatBlock {
        gen,
        is_stat_xp: v.get("is_stat_xp").and_then(|x| x.as_bool()).unwrap_or(false),
        hp: g("hp"),
        attack: g("attack"),
        defense: g("defense"),
        special_attack: g("special_attack"),
        special_defense: g("special_defense"),
        speed: g("speed"),
    }
}

fn opt_stat_block(gen: Gen, v: Option<&Value>) -> Option<StatBlock> {
    match v {
        Some(Value::Object(_)) => Some(stat_block(gen, v.unwrap())),
        _ => None,
    }
}

fn mon(gen: &GenData, v: &Value) -> EnemyPkmn {
    let badges = match v.get("badges") {
        Some(Value::Object(o)) => {
            let set: Vec<&str> = o.iter().filter(|(_, b)| b.as_bool() == Some(true)).map(|(k, _)| k.as_str()).collect();
            Some(gen.make_badge_list().with_slots(&set))
        }
        _ => None,
    };
    let custom_move_data = match v.get("custom_move_data") {
        Some(Value::Object(_)) => Some(CustomMoveData::from_json(v.get("custom_move_data"))),
        _ => None,
    };
    EnemyPkmn {
        name: v["name"].as_str().unwrap_or("").to_string(),
        level: v["level"].as_i64().unwrap_or(0),
        xp: v["xp"].as_i64().unwrap_or(0),
        move_list: v["move_list"]
            .as_array()
            .map(|a| a.iter().map(|m| m.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default(),
        cur_stats: stat_block(gen.gen, &v["cur_stats"]),
        base_stats: stat_block(gen.gen, &v["base_stats"]),
        dvs: stat_block(gen.gen, &v["dvs"]),
        stat_xp: stat_block(gen.gen, &v["stat_xp"]),
        badges,
        held_item: v["held_item"].as_str().map(|s| s.to_string()),
        custom_move_data,
        is_trainer_mon: v["is_trainer_mon"].as_bool().unwrap_or(false),
        exp_split: v["exp_split"].as_i64().unwrap_or(1),
        mon_order: v["mon_order"].as_i64().unwrap_or(1),
        definition_order: v["definition_order"].as_i64().unwrap_or(1),
        ability: v["ability"].as_str().unwrap_or("").to_string(),
        nature: v["nature"].as_str().and_then(Nature::from_name).unwrap_or_default(),
    }
}

fn mv(gen: &GenData, v: &Value) -> Move {
    let name = v["name"].as_str().unwrap_or("");
    let mut m: Move = gen.move_db().get_move(name).map(|m| (**m).clone()).unwrap_or_else(|| panic!("unknown move {}", name));
    m.accuracy = v["accuracy"].as_i64();
    m.pp = v["pp"].as_i64();
    m.base_power = v["base_power"].as_i64();
    if let Some(t) = v["move_type"].as_str() {
        m.move_type = t.to_string();
    }
    if let Some(f) = v["attack_flavor"].as_array() {
        m.attack_flavor = f.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect();
    }
    m
}

fn stages(v: Option<&Value>) -> Option<StageModifiers> {
    let v = v?;
    let o = v.as_object()?;
    let g = |k: &str| o.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
    Some(StageModifiers {
        attack_stage: g("attack_stage"),
        defense_stage: g("defense_stage"),
        speed_stage: g("speed_stage"),
        special_attack_stage: g("special_attack_stage"),
        special_defense_stage: g("special_defense_stage"),
        accuracy_stage: g("accuracy_stage"),
        evasion_stage: g("evasion_stage"),
        attack_badge_boosts: g("attack_badge_boosts"),
        defense_badge_boosts: g("defense_badge_boosts"),
        speed_badge_boosts: g("speed_badge_boosts"),
        special_badge_boosts: g("special_badge_boosts"),
    })
}

fn field(v: Option<&Value>) -> Option<FieldStatus> {
    let v = v?;
    let o = v.as_object()?;
    let g = |k: &str| o.get(k).and_then(|x| x.as_bool()).unwrap_or(false);
    Some(FieldStatus {
        light_screen: g("light_screen"),
        reflect: g("reflect"),
        gravity: g("gravity"),
        magnet_rise: g("magnet_rise"),
        miracle_eye: g("miracle_eye"),
        power_trick: g("power_trick"),
        roost: g("roost"),
        tailwind: g("tailwind"),
        trick_room: g("trick_room"),
        worry_seed: g("worry_seed"),
        gastro_acid: g("gastro_acid"),
        slow_start: g("slow_start"),
    })
}

fn describe(case: &Value) -> String {
    format!(
        "{} [{}] {} {} -> {} (test {})",
        case["kind"].as_str().unwrap_or(""),
        case["version"].as_str().unwrap_or(""),
        case["attacking"]["name"].as_str().unwrap_or(""),
        case["move"]["name"].as_str().unwrap_or(""),
        case.get("defending").and_then(|d| d["name"].as_str()).unwrap_or("-"),
        case["test"].as_str().unwrap_or("")
    )
}

#[test]
fn replay_python_damage_cases() {
    let path = corpus_path();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e));
    let corpus: Value = serde_json::from_str(&text).expect("corpus json");
    assert_eq!(corpus["pytest_exit_code"].as_i64(), Some(0), "the Python tests failed when the corpus was recorded");
    let cases = corpus["cases"].as_array().expect("cases");
    assert!(cases.len() > 400, "corpus looks truncated: {} cases", cases.len());

    let root = xpr_core::consts::find_source_root().expect("source root");
    let reg = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));

    let mut failures: Vec<String> = Vec::new();
    let mut n_damage = 0;
    let mut n_crit = 0;
    let mut n_acc = 0;
    for case in cases {
        let version = case["version"].as_str().unwrap_or("");
        let gen = reg.get_version(version).unwrap_or_else(|e| panic!("load {}: {}", version, e));
        let attacking = mon(&gen, &case["attacking"]);
        let m = mv(&gen, &case["move"]);
        let custom = case["custom_move_data"].as_str().map(|s| s.to_string());
        match case["kind"].as_str().unwrap_or("") {
            "calculate_damage" => {
                n_damage += 1;
                let defending = mon(&gen, &case["defending"]);
                let a_stages = stages(case.get("attacking_stages"));
                let d_stages = stages(case.get("defending_stages"));
                let a_field = field(case.get("attacking_field"));
                let d_field = field(case.get("defending_field"));
                let a_stats = opt_stat_block(gen.gen, case.get("attacking_battle_stats"));
                let d_stats = opt_stat_block(gen.gen, case.get("defending_battle_stats"));
                let custom_str = custom.clone().unwrap_or_default();
                let weather = case["weather"].as_str().unwrap_or(xpr_core::consts::WEATHER_NONE).to_string();
                let args = DamageArgs {
                    attacking: &attacking,
                    mv: &m,
                    defending: &defending,
                    attacking_stages: a_stages.as_ref(),
                    defending_stages: d_stages.as_ref(),
                    attacking_field: a_field.as_ref(),
                    defending_field: d_field.as_ref(),
                    is_crit: case["is_crit"].as_bool().unwrap_or(false),
                    custom_move_data: &custom_str,
                    weather: &weather,
                    is_double_battle: case["is_double_battle"].as_bool().unwrap_or(false),
                    attacking_battle_stats: a_stats.as_ref(),
                    defending_battle_stats: d_stats.as_ref(),
                    attacker_is_enemy: case["attacker_is_enemy"].as_bool().unwrap_or(false),
                };
                let ours = calculate_damage(&gen, &args);
                let expected = &case["result"];
                match (ours, expected) {
                    (None, Value::Null) => {}
                    (None, e) => failures.push(format!("{}: rust None, python {}", describe(case), e)),
                    (Some(r), Value::Null) => failures.push(format!("{}: rust {:?}, python None", describe(case), (r.min_damage, r.max_damage))),
                    (Some(r), e) => {
                        let exp_vals: Vec<(i64, i64)> = e["damage_vals"]
                            .as_array()
                            .map(|a| a.iter().map(|p| (p[0].as_i64().unwrap_or(-1), p[1].as_i64().unwrap_or(-1))).collect())
                            .unwrap_or_default();
                        let our_vals: Vec<(i64, i64)> = r.damage_vals.iter().map(|(k, v)| (*k, *v)).collect();
                        if r.min_damage != e["min_damage"].as_i64().unwrap_or(-1)
                            || r.max_damage != e["max_damage"].as_i64().unwrap_or(-1)
                            || r.size != e["size"].as_i64().unwrap_or(-1)
                            || our_vals != exp_vals
                        {
                            failures.push(format!(
                                "{}: rust min/max/size {}/{}/{} vals {:?}; python {}/{}/{} vals {:?}",
                                describe(case),
                                r.min_damage,
                                r.max_damage,
                                r.size,
                                our_vals,
                                e["min_damage"],
                                e["max_damage"],
                                e["size"],
                                exp_vals
                            ));
                        }
                    }
                }
            }
            "get_crit_rate" => {
                n_crit += 1;
                let ours = get_crit_rate(&gen, &attacking, &m, custom.as_deref());
                let expected = case["result"].as_f64().unwrap_or(f64::NAN);
                if (ours - expected).abs() > 1e-12 {
                    failures.push(format!("{}: rust {} python {}", describe(case), ours, expected));
                }
            }
            "get_move_accuracy" => {
                n_acc += 1;
                let defending = mon(&gen, &case["defending"]);
                let weather = case["weather"].as_str().unwrap_or(xpr_core::consts::WEATHER_NONE);
                let ours = get_move_accuracy(&gen, &attacking, &m, custom.as_deref(), &defending, weather);
                let expected = case["result"].as_f64();
                let same = match (ours, expected) {
                    (None, None) => true,
                    (Some(a), Some(b)) => (a - b).abs() < 1e-9,
                    _ => false,
                };
                if !same {
                    failures.push(format!("{}: rust {:?} python {:?}", describe(case), ours, expected));
                }
            }
            other => failures.push(format!("unknown case kind {}", other)),
        }
    }
    eprintln!("replayed {} damage, {} crit-rate, {} accuracy cases", n_damage, n_crit, n_acc);
    if !failures.is_empty() {
        for f in failures.iter().take(60) {
            eprintln!("MISMATCH {}", f);
        }
        panic!("{} of {} cases differ from Python", failures.len(), cases.len());
    }
}
