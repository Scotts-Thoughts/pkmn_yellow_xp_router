//! `xpr-golden verify <golden_dir> [--limit N] [--filter SUBSTR]`
//! `xpr-golden dump <out_dir> [--battles] <route.json>...`
//! `xpr-golden bench <route.json>`

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rayon::prelude::*;
use serde_json::Value;

use xpr_data::Registry;
use xpr_engine::Router;
use xpr_golden::record;

fn usage() -> ! {
    eprintln!("usage:");
    eprintln!("  xpr-golden verify <golden_dir> [--limit N] [--filter SUBSTR] [--max-diffs N] [--no-battles]");
    eprintln!("  xpr-golden dump <out_dir> [--battles] <route.json>...");
    eprintln!("  xpr-golden bench <route.json>");
    std::process::exit(2);
}

fn source_root() -> PathBuf {
    xpr_core::consts::find_source_root().unwrap_or_else(|| {
        eprintln!("could not locate the repository root (raw_pkmn_data/)");
        std::process::exit(1)
    })
}

fn registry(custom_gens_dir: Option<PathBuf>) -> Arc<Registry> {
    let root = source_root();
    let custom = custom_gens_dir.unwrap_or_else(|| {
        let cfg = xpr_core::Config::load(&xpr_core::Paths::global_config_dir().join("config.json"));
        cfg.get_user_data_dir().join(xpr_core::consts::CUSTOM_GENS_FOLDER_NAME)
    });
    let reg = Arc::new(Registry::new(root.join("raw_pkmn_data"), custom));
    if let Err(e) = reg.reload_all_custom_gens() {
        eprintln!("warning: custom gens: {}", e);
    }
    reg
}

fn verify(args: &[String]) {
    let dir = PathBuf::from(&args[0]);
    let mut limit: Option<usize> = None;
    let mut filter: Option<String> = None;
    let mut max_diffs = 20usize;
    let mut with_battles = true;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--limit" => {
                limit = args.get(i + 1).and_then(|s| s.parse().ok());
                i += 2;
            }
            "--filter" => {
                filter = args.get(i + 1).cloned();
                i += 2;
            }
            "--max-diffs" => {
                max_diffs = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(20);
                i += 2;
            }
            "--no-battles" => {
                with_battles = false;
                i += 1;
            }
            _ => i += 1,
        }
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("golden dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".golden.json"))
        .collect();
    files.sort();
    if let Some(f) = &filter {
        files.retain(|p| p.to_string_lossy().contains(f.as_str()));
    }
    if let Some(l) = limit {
        files.truncate(l);
    }
    let reg = registry(None);
    let cfg = xpr_core::Config::load(&xpr_core::Paths::global_config_dir().join("config.json"));
    let color_major = cfg.get_color_major_battles();
    let start = std::time::Instant::now();
    let results: Vec<(PathBuf, Vec<String>, bool)> = files
        .par_iter()
        .map(|golden_path| {
            let text = std::fs::read(golden_path)
                .map(|b| String::from_utf8_lossy(&b).to_string())
                .unwrap_or_default();
            let golden: Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(e) => return (golden_path.clone(), vec![format!("unreadable golden file: {}", e)], false),
            };
            let source = PathBuf::from(golden.get("source").and_then(|s| s.as_str()).unwrap_or(""));
            let mut router = Router::new(reg.clone());
            let mut ours = record::build_record(&mut router, &source, color_major);
            if golden.get("load_error").is_some() || ours.get("load_error").is_some() {
                let a = ours.get("load_error").is_some();
                let b = golden.get("load_error").is_some();
                if a == b {
                    return (golden_path.clone(), Vec::new(), true);
                }
                return (
                    golden_path.clone(),
                    vec![format!(
                        "load outcome differs: rust={:?} python={:?}",
                        ours.get("load_error"),
                        golden.get("load_error")
                    )],
                    false,
                );
            }
            if with_battles {
                if let Some(Value::Array(_)) = golden.get("battles") {
                    let summary_cfg = xpr_golden::battles::summary_config(&cfg, &router);
                    let battles = xpr_golden::battles::dump_battles(&router, &summary_cfg);
                    if let Value::Object(o) = &mut ours {
                        o.insert("battles".into(), battles);
                    }
                }
            }
            let mut diffs = Vec::new();
            let mut a = record::canonical(&ours);
            let mut b = record::canonical(&golden);
            for v in [&mut a, &mut b] {
                if let Value::Object(o) = v {
                    o.remove("source");
                }
            }
            record::diff("", &a, &b, &mut diffs, max_diffs);
            let ok = diffs.is_empty();
            (golden_path.clone(), diffs, ok)
        })
        .collect();
    let mut passed = 0;
    let mut failed = 0;
    for (path, diffs, ok) in &results {
        if *ok {
            passed += 1;
        } else {
            failed += 1;
            println!("FAIL {}", path.file_name().unwrap().to_string_lossy());
            for d in diffs {
                println!("    {}", d);
            }
        }
    }
    println!(
        "{} passed, {} failed, {} total in {:.2}s",
        passed,
        failed,
        results.len(),
        start.elapsed().as_secs_f64()
    );
    if failed > 0 {
        std::process::exit(1);
    }
}

fn dump(args: &[String]) {
    let out_dir = PathBuf::from(&args[0]);
    std::fs::create_dir_all(&out_dir).expect("out dir");
    let reg = registry(None);
    // `--battles`: also record every fight's battle summary, as `verify` does
    let with_battles = args[1..].iter().any(|a| a == "--battles");
    let cfg = xpr_core::Config::load(&xpr_core::Paths::global_config_dir().join("config.json"));
    for route in args[1..].iter().filter(|a| *a != "--battles") {
        let mut router = Router::new(reg.clone());
        let mut rec = record::build_record(&mut router, Path::new(route), true);
        if with_battles && rec.get("load_error").is_none() {
            let summary_cfg = xpr_golden::battles::summary_config(&cfg, &router);
            if let serde_json::Value::Object(o) = &mut rec {
                o.insert("battles".into(), xpr_golden::battles::dump_battles(&router, &summary_cfg));
            }
        }
        let name = Path::new(route).file_stem().unwrap().to_string_lossy().to_string();
        let text = serde_json::to_string_pretty(&rec).unwrap();
        std::fs::write(out_dir.join(format!("{}.rust.json", name)), text).unwrap();
    }
}

fn bench(args: &[String]) {
    let reg = registry(None);
    let path = Path::new(&args[0]);
    let t0 = std::time::Instant::now();
    let mut router = Router::new(reg.clone());
    router.load(path, false).expect("load");
    let t_load = t0.elapsed();
    let t1 = std::time::Instant::now();
    for _ in 0..10 {
        router.recalc().expect("recalc");
    }
    let t_recalc = t1.elapsed() / 10;
    let t2 = std::time::Instant::now();
    let bytes = router.save_bytes().unwrap();
    let t_save = t2.elapsed();
    println!(
        "load {:?}, recalc {:?} (avg of 10), serialize {:?} ({} bytes), {} groups",
        t_load,
        t_recalc,
        t_save,
        bytes.len(),
        router.all_groups().len()
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
    }
    match args[0].as_str() {
        "verify" if args.len() >= 2 => verify(&args[1..]),
        "dump" if args.len() >= 3 => dump(&args[1..]),
        "bench" if args.len() >= 2 => bench(&args[1..]),
        _ => usage(),
    }
}
