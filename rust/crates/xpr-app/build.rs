//! Embed the image assets so the binary needs nothing next to it:
//! `icons/**` as-is, `assets/pkmn_icons/dex_lookup.json` as-is, and the
//! Pokémon HOME icons / box art prescaled (they are only ever drawn at 28 px
//! and 72 px logical, so 512 px sources would be ~100 MB of dead weight).
//! Outputs are cached in `OUT_DIR` and refreshed only when the source is
//! newer; `embedded_assets.rs` is the generated table.

use std::path::{Path, PathBuf};

use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::imageops::FilterType;
use image::{ImageBuffer, Rgba};

/// Longest side of the prescaled Pokémon icons (drawn at 28 px; stays crisp
/// up to ~225 % display scaling).  96 px would double the pack to ~8 MB.
const PKMN_ICON_SIZE: u32 = 64;
/// Longest side of the prescaled box art (drawn at 72 px).
const BOX_ART_SIZE: u32 = 160;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let assets = root.join("assets");
    let icons = root.join("icons");
    for d in ["assets/pkmn_icons", "assets/box_art", "icons"] {
        println!("cargo:rerun-if-changed={}", root.join(d).display());
    }

    let mut src = String::new();

    // dex_lookup.json
    let dex = assets.join("pkmn_icons").join("dex_lookup.json");
    src.push_str(&format!("pub static DEX_LOOKUP: &str = include_str!({:?});\n", lit(&dex)));

    // Pokémon icons: `<dex>.png` -> (dex, bytes)
    let mut pkmn: Vec<(u32, PathBuf)> = png_files(&assets.join("pkmn_icons"))
        .into_iter()
        .filter_map(|p| p.file_stem()?.to_str()?.parse::<u32>().ok().map(|n| (n, p)))
        .collect();
    pkmn.sort();
    let pkmn_out: Vec<(u32, PathBuf)> = pkmn.iter().map(|(n, p)| (*n, out_dir.join("pkmn_icons").join(p.file_name().unwrap()))).collect();
    prescale_all(&pkmn.iter().map(|(_, p)| p.clone()).collect::<Vec<_>>(), &pkmn_out.iter().map(|(_, p)| p.clone()).collect::<Vec<_>>(), PKMN_ICON_SIZE);
    src.push_str("pub static PKMN_ICONS: &[(u32, &[u8])] = &[\n");
    for (n, p) in &pkmn_out {
        src.push_str(&format!("    ({}, include_bytes!({:?})),\n", n, lit(p)));
    }
    src.push_str("];\n");

    // Box art: `<gen>-<Version>.png` -> (version, bytes)
    let mut art: Vec<(String, PathBuf)> = png_files(&assets.join("box_art"))
        .into_iter()
        .filter_map(|p| {
            let stem = p.file_stem()?.to_str()?.to_string();
            let idx = stem.find('-')?;
            Some((stem[idx + 1..].to_string(), p))
        })
        .collect();
    art.sort();
    let art_out: Vec<(String, PathBuf)> = art.iter().map(|(v, p)| (v.clone(), out_dir.join("box_art").join(p.file_name().unwrap()))).collect();
    prescale_all(&art.iter().map(|(_, p)| p.clone()).collect::<Vec<_>>(), &art_out.iter().map(|(_, p)| p.clone()).collect::<Vec<_>>(), BOX_ART_SIZE);
    src.push_str("pub static BOX_ART: &[(&str, &[u8])] = &[\n");
    for (v, p) in &art_out {
        src.push_str(&format!("    ({:?}, include_bytes!({:?})),\n", v, lit(p)));
    }
    src.push_str("];\n");

    // icons/**: keyed by the path relative to `icons/` (forward slashes)
    let mut icon_files = Vec::new();
    collect_pngs(&icons, &icons, &mut icon_files);
    icon_files.sort();
    src.push_str("pub static ICONS: &[(&str, &[u8])] = &[\n");
    for (rel, p) in &icon_files {
        src.push_str(&format!("    ({:?}, include_bytes!({:?})),\n", rel, lit(p)));
    }
    src.push_str("];\n");

    std::fs::write(out_dir.join("embedded_assets.rs"), src).unwrap();

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let ico_path = out_dir.join("app_icon.ico");
        write_ico(&icons.join("app_icon.png"), &ico_path);
        winresource::WindowsResource::new().set_icon(ico_path.to_str().unwrap()).compile().unwrap_or_else(|e| {
            println!("cargo:warning=failed to embed exe icon: {}", e);
        });
    }
}

/// Build a multi-resolution `.ico` from the (small, pixel-art) source PNG:
/// integer nearest-neighbor upscales stay crisp, the one downscale to 16px
/// uses Lanczos since there's no clean integer ratio down.
fn write_ico(src: &Path, dst: &Path) {
    let img = image::open(src).unwrap_or_else(|e| panic!("{}: {}", src.display(), e)).to_rgba8();
    let sizes = [16u32, 32, 48, 64, 128, 256];
    let native = img.width().max(img.height());
    let frames: Vec<image::codecs::ico::IcoFrame> = sizes
        .iter()
        .map(|&size| {
            let filter = if size <= native { FilterType::Lanczos3 } else { FilterType::Nearest };
            let resized = image::imageops::resize(&img, size, size, filter);
            image::codecs::ico::IcoFrame::as_png(resized.as_raw(), size, size, image::ExtendedColorType::Rgba8)
                .unwrap_or_else(|e| panic!("ico frame {}px: {}", size, e))
        })
        .collect();
    let file = std::io::BufWriter::new(std::fs::File::create(dst).unwrap_or_else(|e| panic!("{}: {}", dst.display(), e)));
    image::codecs::ico::IcoEncoder::new(file).encode_images(&frames).unwrap_or_else(|e| panic!("{}: {}", dst.display(), e));
}

fn lit(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

fn png_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        println!("cargo:warning=asset directory not found: {}", dir.display());
        return Vec::new();
    };
    rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("png"))).collect()
}

fn collect_pngs(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_pngs(root, &p, out);
        } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("png")) {
            out.push((lit(p.strip_prefix(root).unwrap()), p));
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

/// Prescale every stale `srcs[i]` into `dsts[i]`, spread over the cores.
fn prescale_all(srcs: &[PathBuf], dsts: &[PathBuf], max_side: u32) {
    let jobs: Vec<(&PathBuf, &PathBuf)> = srcs.iter().zip(dsts).filter(|(s, d)| needs_refresh(s, d)).collect();
    if jobs.is_empty() {
        return;
    }
    if let Some(parent) = dsts[0].parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(jobs.len());
    let chunk = jobs.len().div_ceil(threads);
    std::thread::scope(|s| {
        for part in jobs.chunks(chunk) {
            s.spawn(move || {
                for (src, dst) in part {
                    prescale(src, dst, max_side);
                }
            });
        }
    });
}

/// Downscale so the longest side is `max_side` (never upscale), resampling
/// premultiplied alpha so transparent edges do not pick up dark fringes.
fn prescale(src: &Path, dst: &Path, max_side: u32) {
    let img = image::open(src).unwrap_or_else(|e| panic!("{}: {}", src.display(), e)).to_rgba32f();
    let (w, h) = img.dimensions();
    let scale = (max_side as f32 / w as f32).min(max_side as f32 / h as f32).min(1.0);
    let (nw, nh) = (((w as f32 * scale).round() as u32).max(1), ((h as f32 * scale).round() as u32).max(1));
    let mut pre: ImageBuffer<Rgba<f32>, Vec<f32>> = img;
    for p in pre.pixels_mut() {
        let a = p.0[3];
        p.0[0] *= a;
        p.0[1] *= a;
        p.0[2] *= a;
    }
    let small = image::imageops::resize(&pre, nw, nh, FilterType::Lanczos3);
    let mut out: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(nw, nh);
    for (o, p) in out.pixels_mut().zip(small.pixels()) {
        let a = p.0[3].clamp(0.0, 1.0);
        let un = |c: f32| if a > 0.0 { (c / a).clamp(0.0, 1.0) } else { 0.0 };
        o.0 = [to_u8(un(p.0[0])), to_u8(un(p.0[1])), to_u8(un(p.0[2])), to_u8(a)];
    }
    let file = std::io::BufWriter::new(std::fs::File::create(dst).unwrap_or_else(|e| panic!("{}: {}", dst.display(), e)));
    let enc = PngEncoder::new_with_quality(file, CompressionType::Best, PngFilter::Adaptive);
    out.write_with_encoder(enc).unwrap_or_else(|e| panic!("{}: {}", dst.display(), e));
}

fn to_u8(v: f32) -> u8 {
    (v * 255.0).round().clamp(0.0, 255.0) as u8
}
