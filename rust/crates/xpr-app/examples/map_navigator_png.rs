//! TEMPORARY, WP-D verification only (not a plan deliverable): render the
//! world map with the navigator panel visible to a PNG, headless, through
//! the app's software rasteriser (`screenshot::Offscreen`, the pattern of
//! `map_card_png.rs`).
//!
//! ```text
//! cargo run -p xpr-app --example map_navigator_png -- <route.json> <out.png>
//! ```

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
use xpr_map::LinkQuery;
use xpr_ui_kit::theme::Theme;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap();
    let route = args.first().map(PathBuf::from).unwrap_or_else(|| root.join("tests/test_data/yellow-pinsir-lv10brock.json"));
    let out = args.get(1).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp/map_navigator.png"));
    let size = Vec2::new(1000.0, 700.0);

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
    // focus a trainer so the scope + camera settle somewhere with content
    // (an indoor map), then let the background chunk renders (including the
    // navigator's coarsest one) arrive
    let _ = view.request_focus(LinkQuery::Trainer("Brock 1".into()), "Brock 1".into());
    let mut prims = Vec::new();
    for _ in 0..80 {
        prims = draw(&mut off, &mut view, 1);
        std::thread::sleep(Duration::from_millis(15));
    }
    let canvas = off.rasterize(&prims, Rect::from_min_size(Pos2::ZERO, size)).expect("rasterise");
    canvas.save(&out).expect("write png");
    println!("wrote {} ({} x {} px), navigator rect: {:?}, zoom: {:.0}%", out.display(), canvas.width, canvas.height, view.navigator_rect(), view.zoom() * 100.0);
}
