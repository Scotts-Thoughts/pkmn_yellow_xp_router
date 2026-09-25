//! The map view driven through a headless `egui::Context` with synthetic
//! input: the pack loads for the route's game, "show on map" focuses the
//! trainer's map, a click on a marker opens its card, a click on plain
//! ground opens the map card, Escape closes it, and the wheel zooms.
//! (`docs/rust_port/design/world_map/SPEC.md` §5 Phase 2 step 7.)

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Event, Key, Modifiers, MouseWheelUnit, PointerButton, Pos2, RawInput, Rect, Vec2};

use xpr_app::assets::Assets;
use xpr_app::controller::MainController;
use xpr_app::map::{MapAction, MapView};
use xpr_core::{Config, Paths};
use xpr_data::Registry;
use xpr_map::{LinkQuery, ObjectKind, Scope};
use xpr_ui_kit::theme::Theme;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

struct Harness {
    ctx: egui::Context,
    theme: Theme,
    cfg: Config,
    ctrl: MainController,
    assets: Assets,
    view: MapView,
    size: Vec2,
}

impl Harness {
    fn new(route: &str) -> Harness {
        let root = repo_root();
        let cfg = Config::load(&root.join("rust/target/no-such-config.json"));
        let ctx = egui::Context::default();
        let mut theme = Theme::from_config(&cfg);
        theme.install_fonts(&ctx);
        theme.apply(&ctx);
        let paths = Paths::new(root.clone());
        let registry = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
        let mut ctrl = MainController::new(registry, paths);
        ctrl.load_route(&root.join("tests/test_data").join(route));
        let _ = ctrl.take_signals();
        let view = MapView::new(&cfg, &root.join("raw_pkmn_data"));
        Harness { ctx, theme, cfg, ctrl, assets: Assets::new(), view, size: Vec2::new(1000.0, 700.0) }
    }

    fn frame(&mut self, events: Vec<Event>) -> Vec<MapAction> {
        let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)), events, ..Default::default() };
        let mut actions = Vec::new();
        let Harness { ctx, theme, cfg, ctrl, assets, view, .. } = self;
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                actions = view.ui(ui, theme, cfg, ctrl, assets);
            });
        });
        actions
    }

    /// Frames until the pack is loaded (it loads on a background thread).
    fn wait_ready(&mut self) {
        let t = Instant::now();
        while !self.view.is_ready() {
            assert!(t.elapsed() < Duration::from_secs(20), "map pack did not load");
            self.frame(vec![]);
            std::thread::sleep(Duration::from_millis(20));
        }
        self.frame(vec![]);
    }

    /// Frames until the camera animation is over.
    fn settle(&mut self) {
        std::thread::sleep(Duration::from_millis(320));
        self.frame(vec![]);
        self.frame(vec![]);
    }

    fn click(&mut self, pos: Pos2) -> Vec<MapAction> {
        let button = |pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        self.frame(vec![Event::PointerMoved(pos)]);
        self.frame(vec![button(true)]);
        let mut a = self.frame(vec![button(false)]);
        a.extend(self.frame(vec![]));
        a
    }
}

#[test]
fn the_pack_loads_for_the_routes_game_and_focus_moves_to_the_trainers_map() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    let pack = h.view.pack().unwrap().clone();
    assert_eq!(pack.game, "yellow");
    assert_eq!(h.view.scope(), Scope::World);

    // "show on map" for Brock: an indoor map, so the scope switches to it
    assert_eq!(h.view.request_focus(LinkQuery::Trainer("Brock 1".into()), "Brock 1".into()), Some(true));
    h.settle();
    let gym = pack.map_by_const("PEWTER_GYM").unwrap().id;
    assert_eq!(h.view.scope(), Scope::Map(gym));
    assert_eq!(h.view.focus_label(), Some("Brock 1"));
    // an unknown trainer reports "no location"
    assert_eq!(h.view.request_focus(LinkQuery::Trainer("Nobody 7".into()), "Nobody 7".into()), Some(false));
}

#[test]
fn clicking_a_marker_opens_its_card_and_plain_ground_opens_the_map_card() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    let pack = h.view.pack().unwrap().clone();
    assert_eq!(h.view.request_focus(LinkQuery::Trainer("Brock 1".into()), "Brock 1".into()), Some(true));
    h.settle();
    let brock = pack.objects.iter().position(|o| o.kind == ObjectKind::Trainer && o.trainer_names().iter().any(|n| n == "Brock 1")).unwrap() as u32;
    let pos = h.view.object_screen_pos(brock).expect("Brock is on screen");
    let actions = h.click(pos);
    assert!(actions.is_empty());
    assert_eq!(h.view.card_object(), Some(brock), "the trainer card opened");
    assert!(h.view.focus_label().is_none(), "a click clears the focus banner");

    // Escape closes the card
    h.frame(vec![Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
    h.frame(vec![]);
    assert!(!h.view.has_card());

    // a floor step of the gym near Brock (on screen, no object within reach) opens the map card
    let gym = pack.map_by_const("PEWTER_GYM").unwrap().id;
    let b = &pack.objects[brock as usize];
    let objects: Vec<Pos2> = pack.object_range(gym).filter_map(|i| h.view.object_screen_pos(i)).collect();
    let mut floor = None;
    'search: for dy in 1..8i32 {
        for dx in -3..4i32 {
            let (x, y) = (b.x as i32 + dx, b.y as i32 + dy);
            let Some(p) = h.view.step_screen_pos(gym, x, y) else { continue };
            let on_screen = p.x > 40.0 && p.x < 960.0 && p.y > 80.0 && p.y < 640.0;
            // the hit tolerance is 12 world px (~40 screen px at this zoom): stay well clear of every object
            let clear = objects.iter().all(|o| (o.x - p.x).abs() > 56.0 || (o.y - p.y).abs() > 56.0);
            if on_screen && clear {
                floor = Some(p);
                break 'search;
            }
        }
    }
    let floor = floor.expect("a free floor step on screen");
    h.click(floor);
    assert!(h.view.card_is_map() || h.view.card_is_tile(), "a ground click opens the map / tile card (card object: {:?})", h.view.card_object());
}

#[test]
fn the_wheel_zooms_about_the_cursor_and_the_zoom_stays_in_range() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    h.settle();
    let z0 = h.view.zoom();
    let center = Pos2::new(500.0, 350.0);
    h.frame(vec![Event::PointerMoved(center)]);
    for _ in 0..3 {
        h.frame(vec![Event::MouseWheel { unit: MouseWheelUnit::Point, delta: Vec2::new(0.0, 40.0), modifiers: Modifiers::NONE }]);
    }
    assert!(h.view.zoom() > z0, "wheel up zooms in ({} -> {})", z0, h.view.zoom());
    for _ in 0..40 {
        h.frame(vec![Event::MouseWheel { unit: MouseWheelUnit::Point, delta: Vec2::new(0.0, -80.0), modifiers: Modifiers::NONE }]);
    }
    assert!(h.view.zoom() >= 0.02 && h.view.zoom() < z0, "zoom out is clamped ({})", h.view.zoom());
}

#[test]
fn sprites_fill_the_atlas_when_zoomed_in_and_circles_take_over_when_zoomed_out() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    assert_eq!(h.view.request_focus(LinkQuery::Trainer("Brock 1".into()), "Brock 1".into()), Some(true));
    h.settle();
    assert!(h.view.zoom() >= 1.0);
    let n = h.view.sprite_frames();
    assert!(n >= 2, "the gym's trainers and NPCs got sprite frames ({})", n);
    // zooming far out (in the world, whose fit zoom is small) draws circles instead
    h.view.back_to_world();
    h.frame(vec![]);
    let center = Pos2::new(500.0, 350.0);
    h.frame(vec![Event::PointerMoved(center)]);
    for _ in 0..40 {
        h.frame(vec![Event::MouseWheel { unit: MouseWheelUnit::Point, delta: Vec2::new(0.0, -80.0), modifiers: Modifiers::NONE }]);
    }
    assert!(h.view.zoom() < 0.75);
    h.view.request_focus(LinkQuery::Trainer("Youngster 1".into()), "Youngster 1".into());
    h.settle();
    assert!(h.view.sprite_frames() > n, "Route 3's trainers add frames once zoomed back in ({} -> {})", n, h.view.sprite_frames());
}
