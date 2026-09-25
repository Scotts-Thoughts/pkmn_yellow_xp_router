//! Feature `embed-map-data`: deflate every file of `map_data/<game>/**`
//! (except the generated coverage report and the hand-curated overrides,
//! which are pipeline inputs/outputs, not runtime data) into `OUT_DIR` and
//! generate the lookup table `embedded.rs` includes. Mirrors
//! `xpr-data/build.rs`, but for arbitrary files (JSON, binary blobs, PNG).

use std::io::Write;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var_os("CARGO_FEATURE_EMBED_MAP_DATA").is_none() {
        return;
    }
    let data_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../map_data");
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed={}", data_dir.display());

    let mut files = Vec::new();
    if data_dir.is_dir() {
        collect(&data_dir, &data_dir, &mut files);
    }
    files.sort();

    let mut table = String::from("static FILES: &[(&str, &[u8])] = &[\n");
    for rel in &files {
        let src = data_dir.join(rel);
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

fn collect(root: &Path, dir: &Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let p = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if p.is_dir() {
            collect(root, &p, out);
        } else if name != "coverage.md" && name != "overrides.json" && !name.starts_with('.') {
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
