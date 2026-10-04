//! Feature `embed-dex-data`: deflate every `dex_data/**/*.json` into
//! `OUT_DIR` and list the HOME sprites (already WebP-compressed, included
//! as-is), generating the tables `embedded.rs` includes.

use std::io::Write;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var_os("CARGO_FEATURE_EMBED_DEX_DATA").is_none() {
        return;
    }
    let data_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../dex_data");
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed={}", data_dir.display());

    let mut files = Vec::new();
    collect(&data_dir, &data_dir, "json", &mut files);
    files.sort();
    let mut table = String::from("static FILES: &[(&str, &[u8])] = &[\n");
    for rel in &files {
        let src = data_dir.join(rel);
        let dst = out_dir.join("dex").join(format!("{}.z", rel));
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

    let mut sprites = Vec::new();
    let sprite_dir = data_dir.join("sprites/home");
    if sprite_dir.is_dir() {
        collect(&sprite_dir, &sprite_dir, "webp", &mut sprites);
    }
    let mut ids: Vec<(u32, String)> = sprites.iter().filter_map(|rel| Some((rel.strip_suffix(".webp")?.parse().ok()?, rel.clone()))).collect();
    ids.sort();
    table.push_str("static SPRITES: &[(u32, &[u8])] = &[\n");
    for (id, rel) in &ids {
        table.push_str(&format!("    ({}, include_bytes!({:?})),\n", id, sprite_dir.join(rel).to_string_lossy().replace('\\', "/")));
    }
    table.push_str("];\n");
    std::fs::write(out_dir.join("embedded_dex.rs"), table).unwrap();
}

/// Relative (forward-slash) paths of every `.<ext>` under `dir`.
fn collect(root: &Path, dir: &Path, ext: &str, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let p = entry.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n != "sprites" || ext != "json") {
                collect(root, &p, ext, out);
            }
        } else if p.extension().is_some_and(|e| e == ext) {
            out.push(p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/"));
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
