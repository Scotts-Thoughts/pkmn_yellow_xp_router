//! The route-compare page driven through a headless `egui::Context` with
//! synthetic input: the interactive paths a screenshot run cannot reach.
//! (`docs/rust_port/design/route_compare/SPEC.md` §4.3, §9.)

use std::path::PathBuf;
use std::sync::Arc;

use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2};

use xpr_app::assets::Assets;
use xpr_app::compare::{CompareActions, CompareEnv, CompareView, RouteSource, TAB_CHECKPOINTS, TAB_DIFF, TAB_OVERVIEW};
use xpr_app::route_index::RouteIndex;
use xpr_core::{Config, Paths};
use xpr_data::Registry;
use xpr_ui_kit::theme::Theme;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

struct Harness {
    ctx: egui::Context,
    theme: Theme,
    cfg: Config,
    paths: Paths,
    registry: Arc<Registry>,
    index: RouteIndex,
    assets: Assets,
    view: CompareView,
    size: Vec2,
    /// Whether Esc was still unconsumed after the page drew (the app reads
    /// that as "leave the page").
    esc_reached_the_app: bool,
}

impl Harness {
    fn new(size: Vec2) -> Harness {
        let root = repo_root();
        let cfg = Config::load(&xpr_app::scratch_config_path());
        let ctx = egui::Context::default();
        let mut theme = Theme::from_config(&cfg);
        theme.install_fonts(&ctx);
        theme.apply(&ctx);
        Harness {
            ctx,
            theme,
            cfg,
            paths: Paths::new(root.clone()),
            registry: Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new())),
            index: RouteIndex::default(),
            assets: Assets::new(),
            view: CompareView::new(),
            size,
            esc_reached_the_app: false,
        }
    }

    /// One frame with `events`, exactly as `XprApp::compare_page` runs it.
    fn frame(&mut self, events: Vec<Event>) -> CompareActions {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            events,
            ..Default::default()
        };
        let mut actions = CompareActions::default();
        let mut esc = false;
        let Harness { ctx, theme, cfg, paths, registry, index, assets, view, .. } = self;
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                view.poll(ctx);
                let env = CompareEnv { registry: registry.clone(), paths, index, current_route_name: None };
                actions = view.ui(ui, theme, cfg, assets, &env);
                esc = ctx.input(|i| i.key_pressed(Key::Escape));
            });
        });
        self.esc_reached_the_app = esc;
        actions
    }

    fn click(&mut self, pos: Pos2) {
        let button = |pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        self.frame(vec![Event::PointerMoved(pos), button(true)]);
        self.frame(vec![button(false)]);
    }

    fn press_escape(&mut self) {
        self.frame(vec![Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
    }

    fn load_pair(&mut self, a: &str, b: &str) {
        let dir = repo_root().join("tests/test_data");
        {
            let Harness { view, registry, paths, index, .. } = self;
            let env = CompareEnv { registry: registry.clone(), paths, index, current_route_name: None };
            view.set_source(true, RouteSource::Path(dir.join(a)), &env);
            view.set_source(false, RouteSource::Path(dir.join(b)), &env);
        }
        for _ in 0..400 {
            self.frame(Vec::new());
            if !self.view.is_loading() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        self.frame(Vec::new());
        assert!(self.view.has_comparison(), "both routes load and compare");
    }
}

#[test]
fn the_picker_survives_the_click_that_opens_it() {
    let mut h = Harness::new(Vec2::new(1280.0, 800.0));
    h.frame(Vec::new());
    let button_a = h.view.picker_button_rect(true).expect("the A button was drawn");
    assert!(!h.view.picker_open(true));

    h.click(button_a.center());
    assert!(h.view.picker_open(true), "the opening click must not count as a click outside the popup");
    for _ in 0..5 {
        h.frame(Vec::new());
    }
    assert!(h.view.picker_open(true), "and it stays open while nothing happens");
}

#[test]
fn opening_the_page_with_no_route_b_shows_its_picker() {
    let mut h = Harness::new(Vec2::new(1280.0, 800.0));
    let env = CompareEnv { registry: h.registry.clone(), paths: &h.paths, index: &h.index, current_route_name: None };
    h.view.open(None, &env);
    // The menu click that opened the page is still this frame's click.
    let away = Pos2::new(640.0, 600.0);
    h.frame(vec![
        Event::PointerMoved(away),
        Event::PointerButton { pos: away, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE },
        Event::PointerButton { pos: away, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE },
    ]);
    assert!(h.view.picker_open(false), "B's picker is open on arrival");
    h.frame(Vec::new());
    assert!(h.view.picker_open(false));
}

#[test]
fn a_click_on_the_page_closes_the_picker_and_the_button_toggles_it() {
    let mut h = Harness::new(Vec2::new(1280.0, 800.0));
    h.frame(Vec::new());
    let (button_a, button_b) = (h.view.picker_button_rect(true).unwrap(), h.view.picker_button_rect(false).unwrap());

    h.click(button_a.center());
    assert!(h.view.picker_open(true));
    h.click(Pos2::new(1100.0, 700.0));
    assert!(!h.view.picker_open(true), "a click on the page closes it");

    h.click(button_a.center());
    assert!(h.view.picker_open(true));
    h.click(button_a.center());
    assert!(!h.view.picker_open(true), "the button toggles its own picker");

    h.click(button_a.center());
    h.click(button_b.center());
    assert!(h.view.picker_open(false), "opening B's picker");
    assert!(!h.view.picker_open(true), "closes A's");
}

#[test]
fn a_click_inside_the_popup_keeps_it_open() {
    let mut h = Harness::new(Vec2::new(1280.0, 800.0));
    h.frame(Vec::new());
    let button_a = h.view.picker_button_rect(true).unwrap();
    h.click(button_a.center());
    // the popup hangs under the button; its empty list area is inert
    h.click(Pos2::new(button_a.min.x + 200.0, button_a.max.y + 120.0));
    assert!(h.view.picker_open(true), "clicking inside the popup is not clicking outside it");
}

#[test]
fn escape_closes_the_picker_before_it_leaves_the_page() {
    let mut h = Harness::new(Vec2::new(1280.0, 800.0));
    h.frame(Vec::new());
    let button_a = h.view.picker_button_rect(true).unwrap();
    h.click(button_a.center());
    assert!(h.view.picker_open(true));

    h.press_escape();
    assert!(!h.view.picker_open(true), "Esc closes the popup");
    assert!(!h.esc_reached_the_app, "and is consumed, so the page stays");

    h.press_escape();
    assert!(h.esc_reached_the_app, "with nothing open, Esc reaches the app and means Back");
}

#[test]
fn every_tab_draws_at_every_supported_width() {
    // Layout arithmetic that goes negative panics inside egui; the spec
    // supports 1000 px and up, and the narrow (< 1100) Overview stacks.
    for width in [1000.0, 1099.0, 1100.0, 1280.0, 1920.0] {
        let mut h = Harness::new(Vec2::new(width, 800.0));
        h.load_pair("c-porygon-1-.json", "c-porygon-1-.json");
        for tab in [TAB_OVERVIEW, TAB_CHECKPOINTS, TAB_DIFF] {
            h.view.tab = tab;
            for _ in 0..3 {
                h.frame(Vec::new());
            }
        }
        h.view.expand_checkpoint_for_smoke(0);
        h.view.tab = TAB_CHECKPOINTS;
        h.frame(Vec::new());
    }
}

#[test]
fn routes_of_different_generations_draw_too() {
    let mut h = Harness::new(Vec2::new(1280.0, 800.0));
    h.load_pair("yellow-pinsir-lv10brock.json", "platinum_chimchar.json");
    for tab in [TAB_OVERVIEW, TAB_CHECKPOINTS, TAB_DIFF] {
        h.view.tab = tab;
        h.frame(Vec::new());
    }
}

#[test]
fn an_unloadable_route_reports_and_keeps_the_other_slot() {
    let mut h = Harness::new(Vec2::new(1280.0, 800.0));
    let good = RouteSource::Path(repo_root().join("tests/test_data/c-porygon-1-.json"));
    let bad = RouteSource::Path(repo_root().join("tests/test_data/no-such-route.json"));
    let env = CompareEnv { registry: h.registry.clone(), paths: &h.paths, index: &h.index, current_route_name: None };
    h.view.set_source(true, good, &env);
    h.view.set_source(false, bad, &env);
    for _ in 0..400 {
        h.frame(Vec::new());
        if !h.view.is_loading() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    h.frame(Vec::new());
    assert!(!h.view.has_comparison(), "one route cannot be compared with nothing");
    assert!(!h.view.is_loading(), "and the page is not stuck loading");
}
