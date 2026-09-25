//! Input under a modal dialog on the map view — the map-side counterpart of
//! `modal_input.rs` (CLAUDE.md "raw input must respect modal dialogs").
//! `egui::Modal` blocks widgets only; the map's raw reads (its keys, Escape,
//! the map list's click-outside, and every graphics tool that reads the
//! pointer or keys directly: marquee, ruler, navigator, keyboard marker
//! navigation, the export dialog's own keys) must stand down through
//! `xpr_ui_kit::modal::behind_modal`. Every map feature that reads raw
//! input adds a case to this file, on this harness.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Event, Key, Modifiers, MouseWheelUnit, PointerButton, Pos2, RawInput, Rect, Vec2};

use xpr_app::assets::Assets;
use xpr_app::controller::MainController;
use xpr_app::dialogs::{AssignMoveDialog, DialogCtx};
use xpr_app::map::{MapAction, MapView};
use xpr_core::{Config, Paths};
use xpr_data::Registry;
use xpr_map::{LinkQuery, ObjectKind};
use xpr_ui_kit::theme::Theme;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

pub struct Harness {
    pub ctx: egui::Context,
    pub theme: Theme,
    pub cfg: Config,
    pub paths: Paths,
    pub registry: Arc<Registry>,
    pub ctrl: MainController,
    pub assets: Assets,
    pub view: MapView,
    pub dialog: Option<AssignMoveDialog>,
    pub size: Vec2,
}

impl Harness {
    pub fn new(route: &str) -> Harness {
        let root = repo_root();
        let cfg = Config::load(&xpr_app::scratch_config_path());
        let ctx = egui::Context::default();
        let mut theme = Theme::from_config(&cfg);
        theme.install_fonts(&ctx);
        theme.apply(&ctx);
        let paths = Paths::new(root.clone());
        let registry = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
        let mut ctrl = MainController::new(registry.clone(), paths.clone());
        ctrl.load_route(&root.join("tests/test_data").join(route));
        let _ = ctrl.take_signals();
        let view = MapView::new(&cfg, &root.join("raw_pkmn_data"));
        Harness { ctx, theme, cfg, paths, registry, ctrl, assets: Assets::new(), view, dialog: None, size: Vec2::new(1000.0, 700.0) }
    }

    /// One frame in the app's order: the map, then the dialog on top.
    pub fn frame(&mut self, events: Vec<Event>) -> Vec<MapAction> {
        let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)), events, ..Default::default() };
        let mut actions = Vec::new();
        let Harness { ctx, theme, cfg, paths, registry, ctrl, assets, view, dialog, .. } = self;
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                actions = view.ui(ui, theme, cfg, ctrl, assets);
            });
            if let Some(d) = dialog.as_mut() {
                let mut dctx = DialogCtx { theme, cfg, ctrl, registry, paths };
                let (close, _) = d.ui(ctx, &mut dctx);
                if close {
                    *dialog = None;
                }
            }
        });
        actions
    }

    /// Frames until the pack is loaded (it loads on a background thread).
    pub fn wait_ready(&mut self) {
        let t = Instant::now();
        while !self.view.is_ready() {
            assert!(t.elapsed() < Duration::from_secs(20), "map pack did not load");
            self.frame(vec![]);
            std::thread::sleep(Duration::from_millis(20));
        }
        self.frame(vec![]);
    }

    /// Frames until the camera animation is over.
    pub fn settle(&mut self) {
        std::thread::sleep(Duration::from_millis(320));
        self.frame(vec![]);
        self.frame(vec![]);
    }

    /// Open the "Assign Move to Slot 1" dialog over the map and let it settle.
    pub fn open_dialog(&mut self) {
        let gen = self.ctrl.gen().expect("the route is loaded");
        self.dialog = Some(AssignMoveDialog::new(&gen, 0, false));
        for _ in 0..3 {
            self.frame(vec![]);
        }
    }

    /// Close the dialog; egui keeps blocking for the frame after.
    pub fn close_dialog(&mut self) {
        self.dialog = None;
        self.frame(vec![]);
        self.frame(vec![]);
    }

    pub fn click(&mut self, pos: Pos2) -> Vec<MapAction> {
        let button = |pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        self.frame(vec![Event::PointerMoved(pos)]);
        self.frame(vec![button(true)]);
        let mut a = self.frame(vec![button(false)]);
        a.extend(self.frame(vec![]));
        a
    }

    /// A primary-button drag from `from` to `to` over a few frames.
    pub fn drag(&mut self, from: Pos2, to: Pos2, modifiers: Modifiers) -> Vec<MapAction> {
        let mut a = Vec::new();
        a.extend(self.frame(vec![Event::PointerMoved(from)]));
        a.extend(self.frame(vec![Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers }]));
        let mid = Pos2::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
        a.extend(self.frame(vec![Event::PointerMoved(mid)]));
        a.extend(self.frame(vec![Event::PointerMoved(to)]));
        a.extend(self.frame(vec![Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers }]));
        a.extend(self.frame(vec![]));
        a
    }

    pub fn press(&mut self, key: Key) -> Vec<MapAction> {
        self.press_with(key, Modifiers::NONE)
    }

    pub fn press_with(&mut self, key: Key, modifiers: Modifiers) -> Vec<MapAction> {
        let mut a = self.frame(vec![Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }]);
        a.extend(self.frame(vec![Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers }]));
        a
    }

    pub fn wheel(&mut self, pos: Pos2, dy: f32) {
        self.frame(vec![Event::PointerMoved(pos)]);
        self.frame(vec![Event::MouseWheel { unit: MouseWheelUnit::Point, delta: Vec2::new(0.0, dy), modifiers: Modifiers::NONE }]);
        self.frame(vec![]);
    }

    /// The object index of a trainer by router name.
    pub fn trainer_object(&self, name: &str) -> u32 {
        let pack = self.view.pack().expect("pack loaded");
        pack.objects.iter().position(|o| o.kind == ObjectKind::Trainer && o.trainer_names().iter().any(|n| n == name)).unwrap_or_else(|| panic!("{} has no object", name)) as u32
    }
}

/// Focus Brock's gym, then: a click through the dialog must not open his
/// card, the map's zoom key must not zoom, Escape closes the dialog without
/// also clearing the focus banner underneath, and once the dialog is gone
/// the map takes clicks again.
#[test]
fn clicks_and_keys_under_a_dialog_do_not_reach_the_map() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    assert_eq!(h.view.request_focus(LinkQuery::Trainer("Brock 1".into()), "Brock 1".into()), Some(true));
    h.settle();
    let brock = h.trainer_object("Brock 1");
    let pos = h.view.object_screen_pos(brock).expect("Brock is on screen");

    h.open_dialog();
    let zoom = h.view.zoom();
    h.click(pos);
    assert!(h.dialog.is_some(), "the dialog is still open");
    assert_eq!(h.view.card_object(), None, "a click through the dialog must not open the card under it");
    assert_eq!(h.view.focus_label(), Some("Brock 1"), "nor clear the focus banner");
    h.press(Key::Plus);
    assert_eq!(h.view.zoom(), zoom, "the map's zoom key stands down under a dialog");
    h.press(Key::Escape);
    assert!(h.dialog.is_none(), "Escape closed the dialog");
    assert_eq!(h.view.focus_label(), Some("Brock 1"), "the map did not see that Escape");

    h.close_dialog();
    h.click(pos);
    assert_eq!(h.view.card_object(), Some(brock), "with the dialog gone the map takes clicks again");
}

/// The navigator minimap and its zoom-preset key stand down the same way
/// (WP-D, `navigator.rs`): a click on the panel through the dialog must not
/// move the camera, and `1` must not change the zoom, while the dialog is
/// open; both work again once it closes.
#[test]
fn navigator_click_and_zoom_key_do_not_reach_the_map_under_a_dialog() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    h.settle();
    let rect = h.view.navigator_rect().expect("the navigator panel is drawn (toggles.navigator is on by default)");

    h.open_dialog();
    let center = h.view.camera_center();
    let zoom = h.view.zoom();
    h.click(rect.center());
    assert!(h.dialog.is_some(), "the dialog is still open");
    assert_eq!(h.view.camera_center(), center, "a click on the navigator through the dialog must not move the camera");
    h.press(Key::Num1);
    assert_eq!(h.view.zoom(), zoom, "the zoom-preset key stands down under a dialog");

    h.close_dialog();
    h.click(rect.center());
    assert_ne!(h.view.camera_center(), center, "with the dialog gone the navigator takes clicks again");
}

/// Keyboard marker navigation stands down the same way (WP-E, `keynav.rs`,
/// gaps G11/G12): Tab must not move the selection ring and Enter must not
/// open a card while a dialog sits over the map.
#[test]
fn tab_and_enter_do_not_reach_the_map_under_a_dialog() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    assert_eq!(h.view.request_focus(LinkQuery::Trainer("Brock 1".into()), "Brock 1".into()), Some(true));
    h.settle();

    h.open_dialog();
    // `keynav::handle_keys` only runs while the viewport is hovered, like
    // every other map shortcut key; put the pointer over the map first.
    h.frame(vec![Event::PointerMoved(Pos2::new(500.0, 350.0))]);
    let before = h.view.selected_object();
    h.press(Key::Tab);
    assert_eq!(h.view.selected_object(), before, "Tab does not move the selection under a dialog");
    h.press(Key::Enter);
    assert_eq!(h.view.card_object(), None, "Enter opens no card under a dialog");

    h.close_dialog();
}

/// The map's own export dialog stands down the same way (WP-B,
/// `export.rs`, gaps G1/G2) -- it is an `egui::Modal` the map draws itself
/// (`view.export`, not `Harness::dialog`), so this covers it directly on
/// `h.view` rather than through `open_dialog`/`close_dialog`: while it is
/// open, the map's zoom key must not zoom and a click on a marker behind it
/// must not open its card; both work again once Escape dismisses it.
#[test]
fn export_dialog_keys_and_clicks_do_not_reach_the_map() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    assert_eq!(h.view.request_focus(LinkQuery::Trainer("Brock 1".into()), "Brock 1".into()), Some(true));
    h.settle();
    let brock = h.trainer_object("Brock 1");
    let pos = h.view.object_screen_pos(brock).expect("Brock is on screen");

    h.view.export.request_open();
    for _ in 0..3 {
        h.frame(vec![]);
    }
    assert!(h.view.export.is_open(), "the export dialog is open");
    let zoom = h.view.zoom();
    h.click(pos);
    assert!(h.view.export.is_open(), "the export dialog is still open");
    assert_eq!(h.view.card_object(), None, "a click through the export dialog must not open the card under it");
    h.press(Key::Plus);
    assert_eq!(h.view.zoom(), zoom, "the map's zoom key stands down under the export dialog");

    h.press(Key::Escape);
    assert!(!h.view.export.is_open(), "Escape closed the export dialog");

    h.click(pos);
    assert_eq!(h.view.card_object(), Some(brock), "with the export dialog gone the map takes clicks again");
}

/// The marquee / ruler tools stand down the same way (WP-C, `tools.rs`,
/// gaps G3/G13): with a dialog open, the M key must not switch the active
/// tool (`handle_keys` is only called when `!behind_modal`, same as every
/// other single-letter shortcut here) and a Shift+drag -- the
/// photoshop-style "marquee from any tool" shortcut, so it doesn't even
/// need `Tool::Select` active -- must create no selection; both work again
/// once the dialog closes.
#[test]
fn tool_key_and_shift_drag_do_not_reach_the_map_under_a_dialog() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    assert_eq!(h.view.request_focus(LinkQuery::Trainer("Brock 1".into()), "Brock 1".into()), Some(true));
    h.settle();

    h.open_dialog();
    let tool_before = h.view.tools.tool;
    h.press(Key::M);
    assert_eq!(h.view.tools.tool, tool_before, "M does not switch the tool under a dialog");
    let center = Pos2::new(500.0, 400.0);
    h.drag(Pos2::new(center.x - 60.0, center.y - 60.0), Pos2::new(center.x + 60.0, center.y + 60.0), Modifiers::SHIFT);
    assert!(h.view.tools.selection.is_none(), "a Shift+drag creates no selection under a dialog");

    h.close_dialog();
    h.press(Key::M);
    assert_ne!(h.view.tools.tool, tool_before, "with the dialog gone M switches the tool again");
    h.drag(Pos2::new(center.x - 60.0, center.y - 60.0), Pos2::new(center.x + 60.0, center.y + 60.0), Modifiers::NONE);
    assert!(h.view.tools.selection.is_some(), "and a drag in Select mode makes a selection again");
}
