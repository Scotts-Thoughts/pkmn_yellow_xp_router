//! The marquee / ruler tools and the grid / map-name overlays, driven
//! headlessly the way `tests/map_modal_input.rs` and `tests/map_view.rs`
//! do (`docs/rust_port/design/world_map/GRAPHICS_TOOLS_PLAN.md` WP-C).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape, Vec2};

use xpr_app::assets::Assets;
use xpr_app::controller::MainController;
use xpr_app::map::tools;
use xpr_app::map::{MapAction, MapView};
use xpr_core::{Config, Paths};
use xpr_data::Registry;
use xpr_map::{geom, LinkQuery, Scope};
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
    /// every shape the last frame painted (`ctx.run`'s output), for the
    /// grid / map-label overlay tests
    shapes: Vec<Shape>,
}

fn collect_lines(shape: &Shape, n: &mut usize) {
    match shape {
        Shape::LineSegment { .. } => *n += 1,
        Shape::Path(_) => *n += 1,
        Shape::Vec(v) => v.iter().for_each(|s| collect_lines(s, n)),
        _ => {}
    }
}

fn collect_text(shape: &Shape, out: &mut Vec<String>) {
    match shape {
        Shape::Text(t) => out.push(t.galley.text().to_string()),
        Shape::Vec(v) => v.iter().for_each(|s| collect_text(s, out)),
        _ => {}
    }
}

impl Harness {
    fn new(route: &str) -> Harness {
        let root = repo_root();
        let cfg = Config::load(&xpr_app::scratch_config_path());
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
        Harness { ctx, theme, cfg, ctrl, assets: Assets::new(), view, size: Vec2::new(1000.0, 700.0), shapes: Vec::new() }
    }

    fn frame(&mut self, events: Vec<Event>) -> Vec<MapAction> {
        self.frame_with_mods(events, Modifiers::NONE)
    }

    /// Like `frame`, but also sets `RawInput::modifiers` (the ambient
    /// "which keys are held" state a real backend reports every frame,
    /// independent of any one event) -- needed for `drag`'s Shift+drag,
    /// since `ToolState::handle_input` reads `i.modifiers.shift`, not a
    /// per-event modifiers field.
    fn frame_with_mods(&mut self, events: Vec<Event>, modifiers: Modifiers) -> Vec<MapAction> {
        let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)), events, modifiers, ..Default::default() };
        let mut actions = Vec::new();
        let Harness { ctx, theme, cfg, ctrl, assets, view, shapes, .. } = self;
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                actions = view.ui(ui, theme, cfg, ctrl, assets);
            });
        });
        shapes.clear();
        for cs in &out.shapes {
            shapes.push(cs.shape.clone());
        }
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

    /// A primary-button drag from `from` to `to` over a few frames (the
    /// `tests/map_modal_input.rs` harness's `drag`).
    fn drag(&mut self, from: Pos2, to: Pos2, modifiers: Modifiers) -> Vec<MapAction> {
        let mut a = Vec::new();
        a.extend(self.frame_with_mods(vec![Event::PointerMoved(from)], modifiers));
        a.extend(self.frame_with_mods(vec![Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers }], modifiers));
        let mid = Pos2::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
        a.extend(self.frame_with_mods(vec![Event::PointerMoved(mid)], modifiers));
        a.extend(self.frame_with_mods(vec![Event::PointerMoved(to)], modifiers));
        a.extend(self.frame_with_mods(vec![Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers }], modifiers));
        a.extend(self.frame(vec![]));
        a
    }

    fn press(&mut self, key: Key) -> Vec<MapAction> {
        let mut a = self.frame(vec![Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
        a.extend(self.frame(vec![Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE }]));
        a
    }
}

/// M switches to the marquee, and a drag defines a step-aligned selection,
/// clipped to the map, whose status line names the step size.
#[test]
fn pressing_m_then_dragging_creates_a_step_aligned_selection() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    let pack = h.view.pack().unwrap().clone();
    assert_eq!(h.view.request_focus(LinkQuery::Trainer("Brock 1".into()), "Brock 1".into()), Some(true));
    h.settle();
    assert!(h.view.zoom() >= 2.0, "the gym is on screen at a readable zoom ({})", h.view.zoom());

    // hover the viewport first: `handle_keys` only runs while hovered (the
    // same gate the existing zoom/pan key tests rely on)
    h.frame(vec![Event::PointerMoved(Pos2::new(500.0, 350.0))]);
    h.press(Key::M);
    assert_eq!(h.view.tools.tool, tools::Tool::Select, "M switched to the marquee tool");

    // a generous drag that overshoots the gym on purpose, so the "clipped
    // to the map" behaviour is actually exercised
    h.drag(Pos2::new(200.0, 150.0), Pos2::new(800.0, 600.0), Modifiers::NONE);
    let sel = h.view.tools.selection.expect("a selection was created");
    assert!(!sel.rect.is_empty(), "the selection is non-empty");
    for v in [sel.rect.x0, sel.rect.y0, sel.rect.x1, sel.rect.y1] {
        assert_eq!(v % 16, 0, "step-aligned (multiples of 16): {:?}", sel.rect);
    }
    let scope_rect = geom::scope_rect(&pack, h.view.scope());
    assert_eq!(sel.rect, sel.rect.intersect(&scope_rect), "clipped to the map: {:?} vs {:?}", sel.rect, scope_rect);

    let status = h.view.tools.status_text(&pack).expect("a status line while a selection exists");
    assert!(status.contains("steps"), "status names the step size: {:?}", status);
}

/// The Shift+drag shortcut does the same thing from the default hand tool,
/// without switching the persistent tool.
#[test]
fn shift_drag_in_hand_mode_also_creates_a_selection() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    assert_eq!(h.view.request_focus(LinkQuery::Trainer("Brock 1".into()), "Brock 1".into()), Some(true));
    h.settle();
    assert_eq!(h.view.tools.tool, tools::Tool::Pan, "the hand tool is the default");

    h.drag(Pos2::new(200.0, 150.0), Pos2::new(800.0, 600.0), Modifiers::SHIFT);
    let sel = h.view.tools.selection.expect("shift+drag created a selection");
    assert!(!sel.rect.is_empty());
    assert_eq!(sel.rect.x0 % 16, 0);
    assert_eq!(sel.rect.y0 % 16, 0);
    assert_eq!(h.view.tools.tool, tools::Tool::Pan, "shift is a temporary override, not a tool switch");
}

/// Escape clears the selection; called directly, `on_escape` clears the
/// measurement first and the selection second, one thing per call, and
/// reports whether it cleared something.
#[test]
fn escape_clears_the_selection_and_on_escape_reports_it() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    let pack = h.view.pack().unwrap().clone();
    assert_eq!(h.view.request_focus(LinkQuery::Trainer("Brock 1".into()), "Brock 1".into()), Some(true));
    h.settle();

    h.view.tools.tool = tools::Tool::Select;
    h.drag(Pos2::new(200.0, 150.0), Pos2::new(800.0, 600.0), Modifiers::NONE);
    assert!(h.view.tools.selection.is_some(), "a selection exists");

    h.press(Key::Escape);
    assert!(h.view.tools.selection.is_none(), "Escape cleared the selection");

    // now build both a selection and a measurement, and drive `on_escape`
    // directly to check its order and return value
    h.view.tools.tool = tools::Tool::Select;
    h.drag(Pos2::new(200.0, 150.0), Pos2::new(500.0, 400.0), Modifiers::NONE);
    h.view.tools.tool = tools::Tool::Measure;
    h.drag(Pos2::new(300.0, 200.0), Pos2::new(600.0, 500.0), Modifiers::NONE);
    assert!(h.view.tools.selection.is_some(), "the selection survives switching to the ruler");
    assert!(h.view.tools.status_text(&pack).is_some(), "a measurement now exists too");

    assert!(h.view.tools.on_escape(), "first Escape clears something");
    let after_first = h.view.tools.status_text(&pack).expect("the selection's status line (the measurement was cleared first)");
    assert!(after_first.starts_with("Selection"), "{:?}", after_first);
    assert!(h.view.tools.on_escape(), "second Escape clears something");
    assert!(h.view.tools.status_text(&pack).is_none(), "the selection is gone too");
    assert!(!h.view.tools.on_escape(), "nothing left to clear the third time");
}

/// "Add trainers" (the floating toolbar's action, called directly here):
/// the selection's linked, undefeated trainers, in object order. Pewter
/// Gym's own two trainers (Brock, JrTrainerM 1) are both Fight Trainer
/// events in this fixture already, so they read as routed / defeated; Route
/// 3 has trainers this route never mentions, so it exercises the same
/// "undefeated" filter with a non-empty, checkable result.
#[test]
fn add_trainers_yields_the_undefeated_trainers_in_object_order() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    let pack = h.view.pack().unwrap().clone();
    assert_eq!(h.view.request_focus(LinkQuery::Trainer("Youngster 1".into()), "Youngster 1".into()), Some(true));
    h.settle();
    assert_eq!(h.view.scope(), Scope::World);

    let route3 = pack.map_by_const("ROUTE_3").expect("Route 3 in the pack").id;
    // a step-space box around Youngster 1 (14, 4) and BugCatcher 5 (19, 5)
    // only -- Lass 1 / Youngster 2 (y = 9) and BugCatcher 4 / Lass 2 (x = 10
    // / 23) sit just outside it
    let a = h.view.step_screen_pos(route3, 12, 2).expect("step position resolves");
    let b = h.view.step_screen_pos(route3, 21, 7).expect("step position resolves");

    h.view.tools.tool = tools::Tool::Select;
    h.drag(a, b, Modifiers::NONE);
    let sel = h.view.tools.selection.expect("a selection was created");

    match tools::ToolState::add_trainers_action(&h.view, sel) {
        Some(MapAction::AddTrainersNamed { folder, names }) => {
            assert_eq!(folder, "Route 3");
            assert_eq!(names, vec!["Youngster 1".to_string(), "BugCatcher 5".to_string()], "undefeated trainers inside the selection, in object order");
        }
        other => panic!("expected AddTrainersNamed, got {:?}", other),
    }
}

/// R switches to the ruler, and a drag between two known steps gives a
/// measurement with the expected Manhattan step count.
#[test]
fn pressing_r_then_dragging_gives_a_measurement_with_the_manhattan_step_count() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    let pack = h.view.pack().unwrap().clone();
    assert_eq!(h.view.request_focus(LinkQuery::Trainer("Brock 1".into()), "Brock 1".into()), Some(true));
    h.settle();
    let gym = pack.map_by_const("PEWTER_GYM").unwrap().id;

    h.frame(vec![Event::PointerMoved(Pos2::new(500.0, 350.0))]);
    h.press(Key::R);
    assert_eq!(h.view.tools.tool, tools::Tool::Measure, "R switched to the ruler");

    // step-centre screen positions, so the drag lands exactly on (1, 1) and
    // (4, 6) with no snapping ambiguity: Manhattan = |4-1| + |6-1| = 8
    let a = h.view.step_screen_pos(gym, 1, 1).expect("step position resolves");
    let b = h.view.step_screen_pos(gym, 4, 6).expect("step position resolves");
    h.drag(a, b, Modifiers::NONE);

    let status = h.view.tools.status_text(&pack).expect("a measurement status line");
    assert_eq!(status, "Δ 3 × 5 · 8 steps");
}

/// The grid toggle adds line-shaped drawing (step / block lines and map
/// outlines are `line_segment` calls, not `rect_stroke`, exactly so this is
/// countable).
#[test]
fn grid_toggle_adds_line_shapes() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();

    h.view.toggles.grid = false;
    h.frame(vec![]);
    let mut off = 0;
    h.shapes.iter().for_each(|s| collect_lines(s, &mut off));

    h.view.toggles.grid = true;
    h.frame(vec![]);
    let mut on = 0;
    h.shapes.iter().for_each(|s| collect_lines(s, &mut on));

    assert!(on > off, "grid on ({}) draws more line shapes than off ({})", on, off);
}

/// A placed map's display name is drawn as text in the world scope when
/// the labels toggle is on.
#[test]
fn map_labels_are_drawn_in_the_world_scope() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    let pack = h.view.pack().unwrap().clone();
    assert_eq!(h.view.scope(), Scope::World, "the default scope before any navigation");

    h.view.toggles.labels = true;
    h.frame(vec![]);
    let mut texts = Vec::new();
    h.shapes.iter().for_each(|s| collect_text(s, &mut texts));
    let any_label = pack.maps.iter().any(|m| !m.display.is_empty() && texts.contains(&m.display));
    assert!(any_label, "some placed map's name is drawn: {:?}", texts);
}
