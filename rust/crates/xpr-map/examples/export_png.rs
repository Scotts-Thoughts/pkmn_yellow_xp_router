//! CLI around `xpr_map::export::render` for ad hoc PNG exports and timing
//! (WP-A, `docs/rust_port/design/world_map/GRAPHICS_TOOLS_PLAN.md` §3.4 "A",
//! optional deliverable):
//!
//!     cargo run --release -p xpr-map --example export_png -- \
//!         <game> <out.png> [MAP_CONST] [--scale N] [--transparent] [--night]
//!
//! Reads `map_data/<game>`. With no MAP_CONST, exports the whole world
//! (subject to the same `MAX_PIXELS` bound the app's export dialog uses).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use xpr_map::export::{self, ExportRequest};
use xpr_map::{Compositor, MapPack, PackSource, RenderOpts, Scope};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: export_png <game> <out.png> [MAP_CONST] [--scale N] [--transparent] [--night]");
        std::process::exit(2);
    }
    let night = args.iter().any(|a| a == "--night");
    let transparent = args.iter().any(|a| a == "--transparent");
    let scale = args
        .iter()
        .position(|a| a == "--scale")
        .and_then(|i| args.get(i + 1))
        .map(|s| {
            s.parse::<u32>().unwrap_or_else(|_| {
                eprintln!("--scale needs a number, got {:?}", s);
                std::process::exit(2);
            })
        })
        .unwrap_or(1);
    let map_const = args.get(2).filter(|a| !a.starts_with("--")).cloned();

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../map_data");
    let source = PackSource::new(Some(root));

    let t = Instant::now();
    let pack = MapPack::load(&args[0], &source).unwrap_or_else(|e| {
        eprintln!("{}", e);
        std::process::exit(1);
    });
    eprintln!("loaded {} in {:.1} ms: {} maps, {} objects, {} outdoor", pack.game, t.elapsed().as_secs_f64() * 1000.0, pack.maps.len(), pack.objects.len(), pack.layout.draw_order.len());

    let scope = match &map_const {
        Some(c) => Scope::Map(
            pack.map_by_const(c)
                .unwrap_or_else(|| {
                    eprintln!("unknown map {}", c);
                    std::process::exit(1);
                })
                .id,
        ),
        None => Scope::World,
    };

    let pack = Arc::new(pack);
    let comp = Compositor::new(pack.clone());
    let rect = xpr_map::geom::scope_rect(&pack, scope);

    if let Err(e) = export::check(rect, scale) {
        eprintln!("refused: {}", e);
        std::process::exit(1);
    }
    let (w, h) = export::output_size(rect, scale);
    eprintln!(
        "exporting {}x{} world px -> {}x{} px at {}x ({:.1} Mpx){}",
        rect.width(),
        rect.height(),
        w,
        h,
        scale,
        export::output_pixels(rect, scale) as f64 / 1e6,
        if transparent { ", transparent" } else { "" }
    );

    let req = ExportRequest { scope, rect, scale, opts: RenderOpts { night }, transparent };
    let t = Instant::now();
    let mut last_pct = -1i32;
    let img = export::render(&comp, &req, &mut |f| {
        let pct = (f * 100.0).round() as i32;
        if pct != last_pct {
            eprintln!("  {}%  ({:.1} ms)", pct, t.elapsed().as_secs_f64() * 1000.0);
            last_pct = pct;
        }
        true
    })
    .unwrap_or_else(|e| {
        eprintln!("export failed: {}", e);
        std::process::exit(1);
    });
    eprintln!("rendered {}x{} in {:.1} ms", img.w, img.h, t.elapsed().as_secs_f64() * 1000.0);

    let t = Instant::now();
    let png = img.to_png().unwrap_or_else(|e| {
        eprintln!("png encode failed: {}", e);
        std::process::exit(1);
    });
    std::fs::write(&args[1], &png).unwrap_or_else(|e| {
        eprintln!("write failed: {}", e);
        std::process::exit(1);
    });
    eprintln!("encoded + wrote {} ({} bytes) in {:.1} ms", args[1], png.len(), t.elapsed().as_secs_f64() * 1000.0);
}
