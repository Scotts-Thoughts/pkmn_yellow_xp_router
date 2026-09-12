//! Feature `embed-data`: deflate every `raw_pkmn_data/**/*.json` into
//! `OUT_DIR` and generate the lookup table `embedded.rs` includes, so a
//! packaged binary carries the built-in game data.

use std::io::Write;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var_os("CARGO_FEATURE_EMBED_DATA").is_none() {
        return;
    }
    let raw_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../raw_pkmn_data");
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed={}", raw_dir.display());

    let mut files = Vec::new();
    collect_json(&raw_dir, &raw_dir, &mut files);
    files.sort();

    let mut table = String::from("static FILES: &[(&str, &[u8])] = &[\n");
    for rel in &files {
        let src = raw_dir.join(rel);
        let dst = out_dir.join("data").join(format!("{}.z", rel));
        if needs_refresh(&src, &dst) {
            std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
            let bytes = std::fs::read(&src).unwrap();
            let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
            enc.write_all(&bytes).unwrap();
            std::fs::write(&dst, enc.finish().unwrap()).unwrap();
        }
        table.push_str(&format!("    ({:?}, include_bytes!({:?})),\n", rel, dst.to_string_lossy().replace('\\', "/")));
    }
    table.push_str("];\n");
    std::fs::write(out_dir.join("embedded_files.rs"), table).unwrap();
}

/// Relative (forward-slash) paths of every `.json` under `dir`.
fn collect_json(root: &Path, dir: &Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_json(root, &p, out);
        } else if p.extension().is_some_and(|e| e == "json") {
            let rel = p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            out.push(rel);
        }
    }
}

fn needs_refresh(src: &Path, dst: &Path) -> bool {
    let (Ok(s), Ok(d)) = (std::fs::metadata(src), std::fs::metadata(dst)) else { return true };
    match (s.modified(), d.modified()) {
        (Ok(sm), Ok(dm)) => sm > dm,
        _ => true,
    }
}
