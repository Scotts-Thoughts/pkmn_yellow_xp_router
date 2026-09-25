//! Render a map export to a PNG, headless, through `map::export::
//! run_export_blocking` (the synchronous path `ShotKind::Map` and the tests
//! also use) -- for checking that a real export's base tiles and markers
//! line up without opening the app or driving the dialog.
//!
//! ```text
//! cargo run -p xpr-app --example map_export_png -- <route.json> <MAP_CONST | world> <scale> <out.png>
//! ```
//!
//! `MAP_CONST` exports just that map at its own full extent (indoor or
//! outdoor -- `Scope::Map(id)` renders a map's own tiles regardless of
//! whether it is placed in the world layout); `world` exports the whole
//! `Scope::World` layout. `scale` is an integer 1-8. Markers, sprites and
//! map names are on; grid, route path and the selection outline are off
//! (`Toggles::default()`), matching the live viewer's own defaults.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::Vec2;
use xpr_app::assets::Assets;
use xpr_app::controller::MainController;
use xpr_app::map::export::{run_export_blocking, ExportOutcome, ExportSpec};
use xpr_app::map::{MapView, Toggles};
use xpr_app::screenshot::Offscreen;
use xpr_core::{Config, Paths};
use xpr_data::Registry;
use xpr_map::{geom, Scope};
use xpr_ui_kit::theme::Theme;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 4 {
        eprintln!("usage: map_export_png <route.json> <MAP_CONST | world> <scale> <out.png>");
        std::process::exit(2);
    }
    let route = PathBuf::from(&args[0]);
    let target = &args[1];
    let scale: u32 = args[2].parse().expect("scale must be an integer 1-8");
    let out = PathBuf::from(&args[3]);

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap();
    let mut cfg = Config::load(&xpr_app::scratch_config_path());
    let theme = Theme::from_config(&cfg);
    let paths = Paths::new(root.clone());
    let registry = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    let mut ctrl = MainController::new(registry, paths);
    ctrl.load_route(&route);
    let _ = ctrl.take_signals();
    let mut assets = Assets::new();
    let mut view = MapView::new(&cfg, &root.join("raw_pkmn_data"));

    // Drive the pack load (and the route-state sync it triggers) through a
    // small offscreen context. The export itself (below) renders through
    // its own, independent `Offscreen`, straight from the `Compositor` --
    // no chunk cache / camera navigation needed for a correct pixel export.
    let mut off = Offscreen::new(&theme, 1.0, 2048);
    let size = Vec2::new(800.0, 600.0);
    let mut draw = |off: &mut Offscreen, view: &mut MapView| {
        off.render(size, 1, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                view.ui(ui, &theme, &mut cfg, &ctrl, &mut assets);
            });
        });
    };
    let t = Instant::now();
    while !view.is_ready() {
        assert!(t.elapsed() < Duration::from_secs(30), "map pack did not load");
        draw(&mut off, &mut view);
        std::thread::sleep(Duration::from_millis(20));
    }
    draw(&mut off, &mut view);

    let pack = view.pack().expect("pack loaded").clone();
    let (scope, rect) = if target.eq_ignore_ascii_case("world") {
        (Scope::World, geom::scope_rect(&pack, Scope::World))
    } else {
        let map = pack.map_by_const(target).unwrap_or_else(|| panic!("{} is not in the {} pack", target, pack.game));
        (Scope::Map(map.id), geom::scope_rect(&pack, Scope::Map(map.id)))
    };
    let spec = ExportSpec { scope, rect, scale, night: false, toggles: Toggles::default(), selection: None, transparent: false };
    match run_export_blocking(&view, &theme, &spec, Some(&out)) {
        Ok(ExportOutcome::Exported(path)) => {
            println!("wrote {} ({:?} at {}x, {:?})", path.display(), scope, scale, rect);
        }
        Ok(ExportOutcome::Copied { width, height, .. }) => {
            println!("(unexpected) got pixel data {}x{} instead of a file", width, height);
        }
        Err(e) => {
            eprintln!("export failed: {}", e);
            std::process::exit(1);
        }
    }
}
