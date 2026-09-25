//! Composite a game's world (or one map) at native scale and write a PNG:
//!
//!     cargo run --release -p xpr-map --example render_world -- yellow out.png [MAP_CONST] [--night]
//!
//! Reads `map_data/<game>` (or `XPR_MAP_DATA_DIR`).

use std::path::PathBuf;
use std::time::Instant;

use xpr_map::{Compositor, MapPack, PackSource, RenderOpts, Scope};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: render_world <game> <out.png> [MAP_CONST] [--night]");
        std::process::exit(2);
    }
    let night = args.iter().any(|a| a == "--night");
    let map_const = args.get(2).filter(|a| !a.starts_with("--")).cloned();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../map_data");
    let source = PackSource::new(Some(root));
    let t = Instant::now();
    let pack = MapPack::load(&args[0], &source).unwrap_or_else(|e| {
        eprintln!("{}", e);
        std::process::exit(1)
    });
    eprintln!("loaded {} in {:.1} ms: {} maps, {} objects, {} outdoor", pack.game, t.elapsed().as_secs_f64() * 1000.0, pack.maps.len(), pack.objects.len(), pack.layout.draw_order.len());
    let scope = match map_const {
        Some(c) => Scope::Map(pack.map_by_const(&c).map(|m| m.id).unwrap_or_else(|| {
            eprintln!("unknown map {}", c);
            std::process::exit(1)
        })),
        None => Scope::World,
    };
    let comp = Compositor::new(std::sync::Arc::new(pack));
    let t = Instant::now();
    let img = comp.render_scope(scope, RenderOpts { night });
    eprintln!("composited {}x{} in {:.1} ms", img.w, img.h, t.elapsed().as_secs_f64() * 1000.0);
    let t = Instant::now();
    let chunk = comp.render_chunk(scope, 0, 0, 0, RenderOpts { night });
    let c5 = comp.render_chunk(scope, 4, 0, 0, RenderOpts { night });
    eprintln!("chunk L0 + L4 in {:.1} ms ({}x{}, {}x{})", t.elapsed().as_secs_f64() * 1000.0, chunk.w, chunk.h, c5.w, c5.h);
    std::fs::write(&args[1], img.to_png().expect("png")).expect("write");
    let overview = PathBuf::from(&args[1]).with_extension("l4.png");
    std::fs::write(&overview, c5.to_png().expect("png")).expect("write");
    eprintln!("wrote {} and {}", args[1], overview.display());
}
