//! WP-E verification only (`docs/rust_port/design/world_map/
//! GRAPHICS_TOOLS_PLAN.md`): render the world with the route-path overlay
//! (gap G9 / SPEC D15 §13) turned on, headless, through the app's software
//! rasteriser, so the discs and connecting line can be eyeballed without a
//! window. Temporary — not part of the app; safe to delete after review.
//! Modelled on `examples/map_card_png.rs`.
//!
//! ```text
//! cargo run -p xpr-app --example map_route_path_png -- <route.json> <out.png>
//! ```
//!
//! Builds a throwaway folder ("WP-E Route 3 Trip") of several Route 3
//! trainers (Yellow) after the route's first trainer fight, selects it, and
//! turns `toggles.path` on — the same construction `tests/map_navigation.rs`
//! uses for the route-path tests, just with more nodes for a clearer picture.

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
use xpr_engine::{EventDefinition, TrainerEventDefinition};
use xpr_ui_kit::theme::Theme;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: map_route_path_png <route.json> <out.png>");
        std::process::exit(2);
    }
    let route = PathBuf::from(&args[0]);
    let out = PathBuf::from(&args[1]);
    let size = Vec2::new(1200.0, 800.0);

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap();
    let mut cfg = Config::load(&xpr_app::scratch_config_path());
    let theme = Theme::from_config(&cfg);
    let paths = Paths::new(root.clone());
    let registry = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    let mut ctrl = MainController::new(registry, paths);
    ctrl.load_route(&route);
    let _ = ctrl.take_signals();

    // build a throwaway trip: six Route 3 trainers (Yellow), in object
    // order — pure `Router` operations, done before `ctrl` is borrowed by
    // the `draw` closure below (a route event does not need the map pack)
    let anchor = ctrl
        .router
        .all_groups()
        .into_iter()
        .find(|g| ctrl.router.group(*g).unwrap().event_definition.trainer_def.is_some())
        .expect("the route has a trainer fight to anchor the new folder after");
    let folder_name = "WP-E Route 3 Trip";
    ctrl.finalize_new_folder(folder_name, None, Some(anchor));
    let names = ["BugCatcher 4", "Youngster 1", "Lass 1", "BugCatcher 5", "Lass 2", "Youngster 2"];
    let mut last = None;
    for name in names {
        last = ctrl.new_event(EventDefinition::with_trainer(TrainerEventDefinition::new(name)), None, None, Some(folder_name), false);
    }
    let folder_id = ctrl.router.parent_of(last.expect("at least one trainer added")).expect("the new event has a parent folder");
    ctrl.select_new_events(vec![folder_id]);

    let mut assets = Assets::new();
    let mut view = MapView::new(&cfg, &root.join("raw_pkmn_data"));
    view.toggles.path = true;

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
    view.sync_route(&ctrl); // `poll_load` already did this once the pack arrived; explicit for clarity

    // `Offscreen::render` builds its own empty `RawInput` each pass (no way
    // to inject a wheel event to zoom out), so centre on the first trainer
    // with `request_focus` instead of `navigate_to`-ing the whole (wider
    // than the canvas) map: the six trainers are a tight cluster around it.
    let ok = view.request_focus(xpr_map::LinkQuery::Trainer("BugCatcher 4".into()), "BugCatcher 4".into());
    assert_eq!(ok, Some(true), "BugCatcher 4 has a map position");
    draw(&mut off, &mut view, 2);
    view.clear_focus(); // the banner is not what this picture is about

    // let the composited map chunks arrive from the background thread
    let mut prims = Vec::new();
    for _ in 0..60 {
        prims = draw(&mut off, &mut view, 1);
        std::thread::sleep(Duration::from_millis(15));
    }
    let canvas = off.rasterize(&prims, Rect::from_min_size(Pos2::ZERO, size)).expect("rasterise");
    canvas.save(&out).expect("write png");
    let trip_nodes = view.state.trip.as_ref().map(|t| t.nodes.len()).unwrap_or(0);
    println!("wrote {} ({} x {} px), scope={:?} zoom={:.2} trip.nodes={}", out.display(), canvas.width, canvas.height, view.scope(), view.zoom(), trip_nodes);
}
