//! Render the world map with an info card open to a PNG, headless, through
//! the app's software rasteriser — for checking card layouts without a
//! window.
//!
//! ```text
//! cargo run -p xpr-app --example map_card_png -- <route.json> <MAP_CONST> <x,y | grass | water | map> <out.png> [width height]
//! ```
//!
//! `x,y` opens the card of that step (the encounter card on tall grass or
//! water), `grass` / `water` pick such a step near the map's centre, and
//! `map` opens the map card. The camera is centred on the map first.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Pos2, Rect, Vec2};
use xpr_app::assets::Assets;
use xpr_app::controller::MainController;
use xpr_app::map::MapView;
use xpr_app::screenshot::Offscreen;
use xpr_core::{Config, Paths};
use xpr_data::Registry;
use xpr_ui_kit::theme::Theme;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 4 {
        eprintln!("usage: map_card_png <route.json> <MAP_CONST> <x,y | grass | water | map> <out.png> [width height]");
        std::process::exit(2);
    }
    let route = PathBuf::from(&args[0]);
    let map_const = &args[1];
    let what = &args[2];
    let out = PathBuf::from(&args[3]);
    let size = Vec2::new(args.get(4).and_then(|s| s.parse().ok()).unwrap_or(1000.0), args.get(5).and_then(|s| s.parse().ok()).unwrap_or(700.0));

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

    // 2 px per point: the picture is as crisp as on a HiDPI screen
    let mut off = Offscreen::new(&theme, 2.0, 8192);
    let mut draw = |off: &mut Offscreen, view: &mut MapView, passes: usize| {
        off.render(size, passes, |ctx| {
            egui::CentralPanel::default().frame(egui::Frame::new().fill(theme.bg)).show(ctx, |ui| {
                view.ui(ui, &theme, &mut cfg, &ctrl, &mut assets);
            });
        })
    };

    let t = Instant::now();
    while !view.is_ready() {
        assert!(t.elapsed() < Duration::from_secs(30), "map pack did not load");
        draw(&mut off, &mut view, 1);
        std::thread::sleep(Duration::from_millis(20));
    }
    draw(&mut off, &mut view, 2);
    let pack = view.pack().unwrap().clone();
    let map = pack.map_by_const(map_const).unwrap_or_else(|| panic!("{} is not in the {} pack", map_const, pack.game));
    view.navigate_to(map.id, false);
    draw(&mut off, &mut view, 2);
    let spb = pack.geom.steps_per_block().max(1) as i32;
    let (x, y) = match what.as_str() {
        "map" => (0, 0),
        // the tall-grass / water block nearest the map's centre
        "grass" | "water" => {
            let (cx, cy) = (map.w as i32 / 2, map.h as i32 / 2);
            let mut best: Option<(i32, i32, i32)> = None;
            for by in 0..map.h as i32 {
                for bx in 0..map.w as i32 {
                    let (grass, water) = pack.terrain_at(map.id, bx as u32, by as u32);
                    if (what == "grass" && grass) || (what == "water" && water) {
                        let d = (bx - cx).abs() + (by - cy).abs();
                        if best.map(|b| d < b.0).unwrap_or(true) {
                            best = Some((d, bx, by));
                        }
                    }
                }
            }
            let (_, bx, by) = best.unwrap_or_else(|| panic!("no {} block in {}", what, map_const));
            (bx * spb, by * spb)
        }
        _ => {
            let (x, y) = what.split_once(',').expect("x,y | grass | water | map");
            (x.trim().parse().unwrap(), y.trim().parse().unwrap())
        }
    };
    let opened = view.open_card_at_step(map.id, x, y);
    assert!(opened, "the card did not open");
    // let the composited map chunks arrive from the background thread
    let mut prims = Vec::new();
    for _ in 0..40 {
        prims = draw(&mut off, &mut view, 1);
        std::thread::sleep(Duration::from_millis(15));
    }
    let canvas = off.rasterize(&prims, Rect::from_min_size(Pos2::ZERO, size)).expect("rasterise");
    canvas.save(&out).expect("write png");
    println!("wrote {} ({} x {} px), card open: tile={} map={}", out.display(), canvas.width, canvas.height, view.card_is_tile(), view.card_is_map());
}
