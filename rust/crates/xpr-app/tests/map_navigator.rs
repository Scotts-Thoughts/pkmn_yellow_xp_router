//! The navigator minimap and the zoom presets menu, headless
//! (`docs/rust_port/design/world_map/GRAPHICS_TOOLS_PLAN.md` WP-D, gaps
//! G7 / G8). Harness copied from `tests/map_view.rs`; the zoom popup's rows
//! and the readout aren't real named widgets with a fixed id to poke, so
//! they're driven the way `tests/encounter_card.rs` reads a card's text,
//! extended here to also record each text shape's screen position as a
//! click target.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape, Vec2};

use xpr_app::assets::Assets;
use xpr_app::controller::MainController;
use xpr_app::map::{MapAction, MapView, STATUS_H, TOOLBAR_H};
use xpr_core::{Config, Paths};
use xpr_data::Registry;
use xpr_map::{LinkQuery, ObjectKind};
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
    /// (text, centre position) of every text shape drawn last frame
    texts: Vec<(String, Pos2)>,
}

fn collect_texts(shape: &Shape, out: &mut Vec<(String, Pos2)>) {
    match shape {
        Shape::Text(t) => out.push((t.galley.text().to_string(), t.pos + t.galley.size() / 2.0)),
        Shape::Vec(v) => v.iter().for_each(|s| collect_texts(s, out)),
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
        Harness { ctx, theme, cfg, ctrl, assets: Assets::new(), view, size: Vec2::new(1000.0, 700.0), texts: Vec::new() }
    }

    fn frame(&mut self, events: Vec<Event>) -> Vec<MapAction> {
        let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)), events, ..Default::default() };
        let mut actions = Vec::new();
        let Harness { ctx, theme, cfg, ctrl, assets, view, texts, .. } = self;
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                actions = view.ui(ui, theme, cfg, ctrl, assets);
            });
        });
        texts.clear();
        for cs in &out.shapes {
            collect_texts(&cs.shape, texts);
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

    /// Frames until nothing is pending in the chunk cache (`ChunkCache::poll`
    /// runs inside `MapView::ui`; the status strip reads "rendering N …"
    /// while anything, including the navigator's coarsest chunk, is in
    /// flight).
    fn wait_chunks_idle(&mut self) {
        let t = Instant::now();
        loop {
            self.frame(vec![]);
            if !self.texts.iter().any(|(s, _)| s.starts_with("rendering ")) {
                break;
            }
            assert!(t.elapsed() < Duration::from_secs(10), "chunks did not finish rendering");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn click(&mut self, pos: Pos2) -> Vec<MapAction> {
        let button = |pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        self.frame(vec![Event::PointerMoved(pos)]);
        self.frame(vec![button(true)]);
        let mut a = self.frame(vec![button(false)]);
        a.extend(self.frame(vec![]));
        a
    }

    /// A primary-button drag from `from` to `to` over a few frames.
    fn drag(&mut self, from: Pos2, to: Pos2) -> Vec<MapAction> {
        let mut a = Vec::new();
        a.extend(self.frame(vec![Event::PointerMoved(from)]));
        a.extend(self.frame(vec![Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }]));
        let mid = Pos2::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
        a.extend(self.frame(vec![Event::PointerMoved(mid)]));
        a.extend(self.frame(vec![Event::PointerMoved(to)]));
        a.extend(self.frame(vec![Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }]));
        a.extend(self.frame(vec![]));
        a
    }

    fn press(&mut self, key: Key) -> Vec<MapAction> {
        let mut a = self.frame(vec![Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
        a.extend(self.frame(vec![Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE }]));
        a
    }

    /// The screen position of a text shape matching `text` exactly.
    fn find_text(&self, text: &str) -> Pos2 {
        self.texts.iter().find(|(s, _)| s == text).map(|(_, p)| *p).unwrap_or_else(|| panic!("no text shape {:?} in the frame: {:?}", text, self.texts.iter().map(|(s, _)| s).collect::<Vec<_>>()))
    }
}

#[test]
fn navigator_panel_click_and_drag_move_the_camera_and_toggle_off_restores_map_clicks() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    let pack = h.view.pack().unwrap().clone();
    assert_eq!(h.view.request_focus(LinkQuery::Trainer("Brock 1".into()), "Brock 1".into()), Some(true));
    h.settle();
    h.wait_chunks_idle();

    // ---- with the toggle off, a click reaches the map as usual ----
    // (the free-floor-step search is `tests/map_view.rs`'s, run at the same
    // just-focused camera state it uses, so it is not a new assumption)
    h.view.toggles.navigator = false;
    h.frame(vec![]);
    assert!(h.view.navigator_rect().is_none(), "no panel is drawn while the toggle is off");
    let gym = pack.map_by_const("PEWTER_GYM").unwrap().id;
    let brock = pack.objects.iter().position(|o| o.kind == ObjectKind::Trainer && o.trainer_names().iter().any(|n| n == "Brock 1")).unwrap() as u32;
    let b = &pack.objects[brock as usize];
    let objects: Vec<Pos2> = pack.object_range(gym).filter_map(|i| h.view.object_screen_pos(i)).collect();
    let mut floor = None;
    'search: for dy in 1..8i32 {
        for dx in -3..4i32 {
            let (x, y) = (b.x as i32 + dx, b.y as i32 + dy);
            let Some(p) = h.view.step_screen_pos(gym, x, y) else { continue };
            let on_screen = p.x > 40.0 && p.x < 960.0 && p.y > 80.0 && p.y < 640.0;
            let clear = objects.iter().all(|o| (o.x - p.x).abs() > 56.0 || (o.y - p.y).abs() > 56.0);
            if on_screen && clear {
                floor = Some(p);
                break 'search;
            }
        }
    }
    let floor = floor.expect("a free floor step of the gym on screen");
    h.click(floor);
    assert!(h.view.card_is_map() || h.view.card_is_tile(), "toggling the navigator off lets the map take the pointer (card object: {:?})", h.view.card_object());
    h.frame(vec![Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
    h.frame(vec![]);
    assert!(!h.view.has_card());

    // ---- turn it back on: the panel redraws in the viewport's top-right ----
    h.view.toggles.navigator = true;
    h.frame(vec![]);
    let rect = h.view.navigator_rect().expect("the navigator panel is drawn (toggles.navigator is back on)");
    // `CentralPanel::default()` adds its own frame margin around `full` on
    // top of the panel's own 8px inset (`mod.rs::ui`'s `full` /
    // `TOOLBAR_H` / `STATUS_H` are all relative to that already-inset
    // rect), so the margin's exact value isn't asserted here, only that the
    // panel sits in the top-right corner within a generous slack for it.
    assert!((rect.width() - 200.0).abs() < 0.01, "panel width is 200px: {:?}", rect);
    assert!((60.0..=160.0).contains(&rect.height()), "panel height is clamped to [60, 160]: {:?}", rect);
    assert!(rect.max.x <= h.size.x, "panel fits within the window: {:?}", rect);
    assert!(rect.max.x > h.size.x - 40.0, "panel is near the viewport's right edge: {:?}", rect);
    assert!(rect.min.y >= TOOLBAR_H, "panel is below the toolbar: {:?}", rect);
    assert!(rect.min.y < TOOLBAR_H + 40.0, "panel is close under the toolbar: {:?}", rect);
    assert!(rect.max.y <= h.size.y - STATUS_H, "panel stays clear of the status strip: {:?}", rect);

    // ---- a click near the panel's right edge establishes a known camera position ----
    let right = Pos2::new(rect.max.x - 4.0, rect.center().y);
    let actions = h.click(right);
    assert!(actions.is_empty(), "a navigator click opens no route action: {:?}", actions);
    assert!(!h.view.has_card(), "a navigator click never opens a card");
    let c1 = h.view.camera_center();

    // ---- a click in the panel's left third moves the centre left of where it was ----
    let left_third = Pos2::new(rect.min.x + rect.width() / 6.0, rect.center().y);
    let actions = h.click(left_third);
    assert!(actions.is_empty());
    assert!(!h.view.has_card());
    let c2 = h.view.camera_center();
    assert!(c2.x < c1.x, "the left-third click moved the camera left ({} -> {})", c1.x, c2.x);

    // ---- a drag across the panel moves it further ----
    let actions = h.drag(left_third, right);
    assert!(actions.is_empty());
    assert!(!h.view.has_card());
    let c3 = h.view.camera_center();
    assert!(c3.x > c2.x, "the drag moved the camera further right ({} -> {})", c2.x, c3.x);
}

#[test]
fn zoom_key_and_presets_popup_set_the_camera_zoom() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    h.settle();

    // the `1` key (= 100%) only acts while the viewport is hovered, like +/-/0
    h.frame(vec![Event::PointerMoved(Pos2::new(500.0, 350.0))]);
    h.press(Key::Num1);
    assert!((h.view.zoom() - 1.0).abs() < 1e-3, "the `1` key set the zoom to 100% ({})", h.view.zoom());

    // the zoom readout opens a presets popup; choosing 400% sets the zoom
    let label = format!("{:.0}%", h.view.zoom() * 100.0);
    let readout = h.find_text(&label);
    h.click(readout);
    let row = h.find_text("400%");
    h.click(row);
    assert!((h.view.zoom() - 4.0).abs() < 1e-3, "choosing 400% in the popup set the zoom ({})", h.view.zoom());
}

/// Review finding (2026-09-25): a pan drag that begins outside the panel
/// and merely crosses it stays a pan. The navigator only owns a drag it
/// started, so the camera moves by the drag's delta (`Camera::pan`:
/// centre -= delta / zoom) instead of snapping to the panel's mapping of
/// wherever the pointer happens to be.
#[test]
fn a_drag_that_only_crosses_the_panel_is_not_captured() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    assert_eq!(h.view.request_focus(LinkQuery::Trainer("Brock 1".into()), "Brock 1".into()), Some(true));
    h.settle();
    h.wait_chunks_idle();
    h.frame(vec![]);
    let rect = h.view.navigator_rect().expect("the navigator panel is drawn");
    let zoom = h.view.zoom();
    let before = h.view.camera_center();
    // from the map well left of the panel to the panel's centre
    let from = Pos2::new(rect.min.x - 160.0, rect.center().y);
    let to = rect.center();
    let actions = h.drag(from, to);
    assert!(actions.is_empty(), "{:?}", actions);
    assert!(!h.view.has_card());
    let after = h.view.camera_center();
    let expected = before - Vec2::new((to.x - from.x) / zoom, (to.y - from.y) / zoom);
    assert!((after.x - expected.x).abs() < 2.0 && (after.y - expected.y).abs() < 2.0, "the drag must pan the map ({:?} -> {:?}, expected {:?}), not hand the camera to the navigator", before, after, expected);
}
