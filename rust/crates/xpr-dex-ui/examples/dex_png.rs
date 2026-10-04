//! Render the Dex page to a PNG, headless (for checking a tab's layout
//! without the app):
//!
//! ```text
//! cargo run -p xpr-dex-ui --example dex_png [--no-default-features --features <tab>] -- \
//!     --out shot.png [--size 1600x1000] [--tab trainers] [--game Emerald] [--version Crystal] \
//!     [--species Pikachu] [--compare Raichu] [--third Pichu] [--self-compare] [--trainer "Leader Falkner"] \
//!     [--move Thunderbolt] [--settings '{"show_bulk":true}'] [--click x,y]... [--right-click x,y]... \
//!     [--hover x,y] [--key F3] [--type text] [--dump-text]
//! ```
//!
//! Actions run in the order given, each followed by a few frames. Points are
//! logical pixels of the `--size` canvas (1 px per point).

use std::path::PathBuf;

use egui::{Key, Modifiers, Pos2, Vec2};
use serde_json::{json, Value};
use xpr_dex_ui::testkit::DexDriver;

enum Step {
    Click(Pos2),
    RightClick(Pos2),
    Hover(Pos2),
    Key(Key, Modifiers),
    Type(String),
}

fn pos(s: &str) -> Pos2 {
    let (x, y) = s.split_once(',').expect("x,y");
    Pos2::new(x.trim().parse().unwrap(), y.trim().parse().unwrap())
}

fn key(s: &str) -> (Key, Modifiers) {
    let mut mods = Modifiers::NONE;
    let mut name = s;
    for (p, m) in [
        ("Ctrl+", Modifiers::COMMAND),
        ("Shift+", Modifiers::SHIFT),
        ("Alt+", Modifiers::ALT),
    ] {
        while let Some(rest) = name.strip_prefix(p) {
            mods = mods | m;
            name = rest;
        }
    }
    (
        Key::from_name(name).unwrap_or_else(|| panic!("unknown key {}", name)),
        mods,
    )
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut settings = json!({});
    let mut out = PathBuf::from("dex.png");
    let mut size = Vec2::new(1600.0, 1000.0);
    let mut steps = Vec::new();
    let mut trainer: Option<String> = None;
    let mut focus_move: Option<String> = None;
    let mut dump = false;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let v = args.get(i + 1).cloned().unwrap_or_default();
        let mut take = true;
        match a {
            "--out" => out = PathBuf::from(&v),
            "--size" => {
                let (w, h) = v.split_once('x').expect("WxH");
                size = Vec2::new(w.parse().unwrap(), h.parse().unwrap());
            }
            "--tab" => {
                settings["tab"] = Value::String(
                    match v.to_lowercase().as_str() {
                        "pokedex" => "Pokedex",
                        "evs" => "Evs",
                        "trainers" => "Trainers",
                        "stats" => "Stats",
                        "damage" => "Damage",
                        "movedex" => "Movedex",
                        "natures" => "Natures",
                        "misc" => "Misc",
                        other => panic!("unknown tab {}", other),
                    }
                    .into(),
                )
            }
            "--game" => settings["game"] = Value::String(v.clone()),
            "--version" => settings["version"] = Value::String(v.clone()),
            "--species" => settings["selected"] = Value::String(v.clone()),
            "--compare" => settings["comparing_with"] = Value::String(v.clone()),
            "--third" => settings["comparing_third"] = Value::String(v.clone()),
            "--self-compare" => {
                settings["self_compare"] = Value::Bool(true);
                take = false;
            }
            "--trainer" => trainer = Some(v.clone()),
            "--move" => focus_move = Some(v.clone()),
            "--settings" => {
                let extra: Value = serde_json::from_str(&v).expect("--settings JSON");
                for (k, x) in extra.as_object().expect("--settings object") {
                    settings[k] = x.clone();
                }
            }
            "--click" => steps.push(Step::Click(pos(&v))),
            "--right-click" => steps.push(Step::RightClick(pos(&v))),
            "--hover" => steps.push(Step::Hover(pos(&v))),
            "--key" => {
                let (k, m) = key(&v);
                steps.push(Step::Key(k, m));
            }
            "--type" => steps.push(Step::Type(v.clone())),
            "--dump-text" => {
                dump = true;
                take = false;
            }
            other => panic!("unknown argument {}", other),
        }
        i += if take { 2 } else { 1 };
    }
    let mut d = DexDriver::new(settings, size);
    d.view.state.selected_trainer = trainer;
    d.view.state.focused_move = focus_move;
    d.frame(4);
    for s in steps {
        match s {
            Step::Click(p) => d.click(p),
            Step::RightClick(p) => d.right_click(p),
            Step::Hover(p) => d.hover(p),
            Step::Key(k, m) => d.key(k, m),
            Step::Type(t) => d.type_text(&t),
        }
        d.frame(2);
    }
    d.frame(3);
    d.save_png(&out).expect("save png");
    if dump {
        for t in &d.texts {
            println!("{}", t);
        }
    }
    eprintln!("wrote {}", out.display());
}
