//! Temporary WP-C verification example (not part of the deliverable, but
//! kept simple enough to leave behind): render the world map with a
//! marquee selection, a ruler measurement, the grid and map-name labels
//! all visible at once, headless, through `xpr_app::screenshot::Offscreen`
//! -- the same rasteriser `examples/map_card_png.rs` uses.
//!
//! `Offscreen::render` builds its own empty `RawInput` each call (no way to
//! feed it pointer events), so the marquee / ruler drags are simulated by
//! cloning `Offscreen`'s own internal `egui::Context` (captured on its
//! first render) and calling `ctx.run` on that clone directly with real
//! `Event::PointerMoved` / `PointerButton` events -- `MapView`'s tool state
//! is plain data, indifferent to which `egui::Context` last drew it, so the
//! final `Offscreen::render` pass (proper texture-delta handling, so newly
//! needed glyphs like "Δ" / "×" actually reach its software texture cache)
//! picks the resulting selection / measurement straight up.
//!
//! `RawInput::time` has to stay monotonic on that shared context
//! (`emath::History` panics otherwise) but `Offscreen`'s own frame counter
//! (private, and the only thing driving its `render()` calls' time) cannot
//! be advanced except by calling `render()` itself -- so once a directly
//! injected frame's time gets ahead of it, `render()` can never safely run
//! again (even a "catch-up" pass would start from its old, now-behind,
//! value and immediately violate the invariant). The fix: freeze every
//! directly injected frame at the exact `RawInput::time` the loading-phase
//! `render()` calls left off on (equal, not behind, is fine), so resuming
//! `render()` afterwards continues moving forward from that same point.
//!
//! ```text
//! cargo run -p xpr-app --example map_tools_png -- <out.png>
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Event, Key, Modifiers, PointerButton, Pos2, Rect, Vec2};
use xpr_app::assets::Assets;
use xpr_app::controller::MainController;
use xpr_app::map::{tools, MapView};
use xpr_app::screenshot::Offscreen;
use xpr_core::{Config, Paths};
use xpr_data::Registry;
use xpr_ui_kit::theme::Theme;

#[allow(clippy::too_many_arguments)]
fn run_frame(ctx: &egui::Context, size: Vec2, time: f64, events: Vec<Event>, modifiers: Modifiers, view: &mut MapView, theme: &Theme, cfg: &mut Config, ctrl: &MainController, assets: &mut Assets) {
    let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), time: Some(time), events, modifiers, ..Default::default() };
    let _ = ctx.run(input, |ctx| {
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme.bg)).show(ctx, |ui| {
            let _ = view.ui(ui, theme, cfg, ctrl, assets);
        });
    });
}

#[allow(clippy::too_many_arguments)]
fn drag(ctx: &egui::Context, size: Vec2, time: f64, from: Pos2, to: Pos2, modifiers: Modifiers, view: &mut MapView, theme: &Theme, cfg: &mut Config, ctrl: &MainController, assets: &mut Assets) {
    run_frame(ctx, size, time, vec![Event::PointerMoved(from)], modifiers, view, theme, cfg, ctrl, assets);
    run_frame(ctx, size, time, vec![Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers }], modifiers, view, theme, cfg, ctrl, assets);
    let mid = Pos2::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
    run_frame(ctx, size, time, vec![Event::PointerMoved(mid)], modifiers, view, theme, cfg, ctrl, assets);
    run_frame(ctx, size, time, vec![Event::PointerMoved(to)], modifiers, view, theme, cfg, ctrl, assets);
    run_frame(ctx, size, time, vec![Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers }], modifiers, view, theme, cfg, ctrl, assets);
    run_frame(ctx, size, time, vec![], Modifiers::NONE, view, theme, cfg, ctrl, assets);
}

#[allow(clippy::too_many_arguments)]
fn press(ctx: &egui::Context, size: Vec2, time: f64, key: Key, view: &mut MapView, theme: &Theme, cfg: &mut Config, ctrl: &MainController, assets: &mut Assets) {
    run_frame(ctx, size, time, vec![Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], Modifiers::NONE, view, theme, cfg, ctrl, assets);
    run_frame(ctx, size, time, vec![Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE }], Modifiers::NONE, view, theme, cfg, ctrl, assets);
}

/// A free function (not a closure bound for the whole of `main`, which
/// would hold `cfg` / `assets` borrowed for its entire lifetime and
/// conflict with the `run_frame` / `drag` / `press` calls in between).
/// `frame_n` mirrors `Offscreen`'s own private frame counter 1:1 (only
/// touched here, by exactly the number of passes run) so the caller can
/// read off the exact `RawInput::time` its *last* pass used.
#[allow(clippy::too_many_arguments)]
fn draw_via_offscreen(off: &mut Offscreen, size: Vec2, view: &mut MapView, captured: &mut Option<egui::Context>, frame_n: &mut u64, theme: &Theme, cfg: &mut Config, ctrl: &MainController, assets: &mut Assets, passes: usize) -> Vec<egui::epaint::ClippedPrimitive> {
    *frame_n += passes as u64;
    off.render(size, passes, |ctx| {
        if captured.is_none() {
            *captured = Some(ctx.clone());
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme.bg)).show(ctx, |ui| {
            let _ = view.ui(ui, theme, cfg, ctrl, assets);
        });
    })
}

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "/tmp/map_tools.png".to_string()));
    let size = Vec2::new(1100.0, 750.0);
    let mut frame_n: u64 = 0;

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap();
    let mut cfg = Config::load(&xpr_app::scratch_config_path());
    let theme = Theme::from_config(&cfg);
    let paths = Paths::new(root.clone());
    let registry = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    let mut ctrl = MainController::new(registry, paths);
    ctrl.load_route(&root.join("tests/test_data/yellow-pinsir-lv10brock.json"));
    let _ = ctrl.take_signals();
    let mut assets = Assets::new();
    let mut view = MapView::new(&cfg, &root.join("raw_pkmn_data"));

    let mut off = Offscreen::new(&theme, 2.0, 8192);
    let mut captured: Option<egui::Context> = None;

    let t = Instant::now();
    while !view.is_ready() {
        assert!(t.elapsed() < Duration::from_secs(30), "map pack did not load");
        draw_via_offscreen(&mut off, size, &mut view, &mut captured, &mut frame_n, &theme, &mut cfg, &ctrl, &mut assets, 1);
        std::thread::sleep(Duration::from_millis(20));
    }
    draw_via_offscreen(&mut off, size, &mut view, &mut captured, &mut frame_n, &theme, &mut cfg, &ctrl, &mut assets, 2);

    let pack = view.pack().unwrap().clone();
    let pewter = pack.map_by_const("PEWTER_CITY").unwrap_or_else(|| panic!("PEWTER_CITY in the {} pack", pack.game));
    view.navigate_to(pewter.id, false);
    draw_via_offscreen(&mut off, size, &mut view, &mut captured, &mut frame_n, &theme, &mut cfg, &ctrl, &mut assets, 2);
    // let chunks / sprites for this area arrive and get uploaded while
    // still going through `Offscreen::render` (its own texture-delta
    // tracking), before switching to raw event injection below
    // frames until the composited chunks have arrived; only the last frame is rasterised
    for _ in 0..30 {
        let _ = draw_via_offscreen(&mut off, size, &mut view, &mut captured, &mut frame_n, &theme, &mut cfg, &ctrl, &mut assets, 1);
        std::thread::sleep(Duration::from_millis(15));
    }

    let ctx = captured.clone().expect("Offscreen's context was captured on its first render");
    // the exact time `Offscreen::render`'s last pass used (its loop reads
    // `self.frame` *then* increments it -- see the module doc)
    let frozen_time = frame_n.saturating_sub(1) as f64 / 60.0;

    // hover the viewport first: the zoom keys and `handle_keys` (H/M/R)
    // only run while `resp.hovered()`, same as on the real map
    run_frame(&ctx, size, frozen_time, vec![Event::PointerMoved(Pos2::new(550.0, 400.0))], Modifiers::NONE, &mut view, &theme, &mut cfg, &ctrl, &mut assets);

    // zoom in past 3x so the grid's step lines (not just block lines) show
    press(&ctx, size, frozen_time, Key::Plus, &mut view, &theme, &mut cfg, &ctrl, &mut assets);
    press(&ctx, size, frozen_time, Key::Plus, &mut view, &theme, &mut cfg, &ctrl, &mut assets);
    println!("zoom after +/+: {}", view.zoom());

    // re-centre on the town now that it's zoomed in more than the viewport
    // is wide, so the town's own label (drawn at its centre) stays on screen
    view.navigate_to(pewter.id, false);
    run_frame(&ctx, size, frozen_time, vec![], Modifiers::NONE, &mut view, &theme, &mut cfg, &ctrl, &mut assets);

    // the marquee: drag a selection over part of the town
    view.tools.tool = tools::Tool::Select;
    drag(&ctx, size, frozen_time, Pos2::new(560.0, 320.0), Pos2::new(820.0, 520.0), Modifiers::NONE, &mut view, &theme, &mut cfg, &ctrl, &mut assets);
    println!("selection: {:?}", view.tools.selection);

    // the ruler: drag a measurement elsewhere in view
    view.tools.tool = tools::Tool::Measure;
    drag(&ctx, size, frozen_time, Pos2::new(200.0, 150.0), Pos2::new(480.0, 260.0), Modifiers::NONE, &mut view, &theme, &mut cfg, &ctrl, &mut assets);

    view.toggles.grid = true;
    view.toggles.labels = true;
    run_frame(&ctx, size, frozen_time, vec![], Modifiers::NONE, &mut view, &theme, &mut cfg, &ctrl, &mut assets);
    let pack2 = view.pack().unwrap().clone();
    println!("status text (selection wins the status strip while both exist): {:?}", view.tools.status_text(&pack2));


    // quick visual check of the Layers popup: click the toolbar button and
    // take one more screenshot before the real final one
    run_frame(&ctx, size, frozen_time, vec![Event::PointerMoved(Pos2::new(401.0, 14.0))], Modifiers::NONE, &mut view, &theme, &mut cfg, &ctrl, &mut assets);
    drag(&ctx, size, frozen_time, Pos2::new(401.0, 14.0), Pos2::new(401.0, 14.0), Modifiers::NONE, &mut view, &theme, &mut cfg, &ctrl, &mut assets);
    let layers_prims = draw_via_offscreen(&mut off, size, &mut view, &mut captured, &mut frame_n, &theme, &mut cfg, &ctrl, &mut assets, 2);
    let layers_canvas = off.rasterize(&layers_prims, Rect::from_min_size(Pos2::ZERO, size)).expect("rasterise");
    layers_canvas.save(&out.with_file_name("layers_popup.png")).expect("write layers png");

    // resume `Offscreen::render` proper for the final passes, continuing
    // forward from exactly `frozen_time` -- so both the time invariant and
    // the texture-delta tracking (new glyphs included) hold
    let prims = draw_via_offscreen(&mut off, size, &mut view, &mut captured, &mut frame_n, &theme, &mut cfg, &ctrl, &mut assets, 2);

    let canvas = off.rasterize(&prims, Rect::from_min_size(Pos2::ZERO, size)).expect("rasterise");
    canvas.save(&out).expect("write png");
    println!("wrote {} ({} x {} px)", out.display(), canvas.width, canvas.height);
}
