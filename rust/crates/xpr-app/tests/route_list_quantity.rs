//! Clicking the quantity on an event's row opens an editor over it, driven
//! through a headless `egui::Context` with synthetic input. The editor gained
//! focus on the frame of the click that opened it, and egui takes focus back
//! on a click's frame from any widget the pointer wasn't already over, so it
//! closed itself on the next frame.

use std::path::PathBuf;
use std::sync::Arc;

use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2};

use xpr_app::controller::MainController;
use xpr_app::route_list::{ListActions, RouteList};
use xpr_core::{Config, Paths};
use xpr_data::Registry;
use xpr_engine::{EventDefinition, NodeId};
use xpr_ui_kit::theme::Theme;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

struct Harness {
    ctx: egui::Context,
    theme: Theme,
    cfg: Config,
    ctrl: MainController,
    list: RouteList,
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
        let list = RouteList::new(&cfg);
        let mut h = Harness { ctx, theme, cfg, ctrl, list };
        h.settle();
        h
    }

    fn frame(&mut self, events: Vec<Event>) {
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 800.0));
        let input = RawInput { screen_rect: Some(screen), events, ..Default::default() };
        let Harness { ctx, theme, cfg, ctrl, list } = self;
        ctx.run_ui(input, |root| {
            egui::CentralPanel::default().show(root, |ui| {
                let mut actions = ListActions::default();
                list.ui(ui, theme, cfg, ctrl, &mut actions, true);
            });
        }).drop_without_applying_deltas();
        // the app rebuilds the list's rows when the route changes
        if self.ctrl.take_signals().route_changed {
            self.list.mark_dirty();
        }
    }

    fn settle(&mut self) {
        for _ in 0..4 {
            self.frame(vec![]);
        }
    }

    fn click(&mut self, pos: Pos2) {
        let button = |pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        self.frame(vec![Event::PointerMoved(pos)]);
        self.frame(vec![button(true)]);
        self.frame(vec![button(false)]);
        self.settle();
    }

    fn type_text(&mut self, text: &str) {
        self.frame(vec![Event::Text(text.to_string())]);
    }

    fn press(&mut self, key: Key) {
        self.frame(vec![Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
        self.settle();
    }

    /// The first event `pick` accepts, scrolled onto the screen and left
    /// unselected, with the place to click its quantity.
    fn find(&mut self, pick: impl Fn(&EventDefinition) -> bool) -> (NodeId, Pos2) {
        let id = self.ctrl.router.all_groups().into_iter().find(|id| self.ctrl.router.group(*id).map(|g| pick(&g.event_definition)).unwrap_or(false)).expect("the route has the event");
        self.ctrl.select_new_events(vec![id]);
        self.list.set_all_selected_event_ids(&[id]);
        self.list.scroll_to_selected_events(&mut self.ctrl);
        self.settle();
        self.ctrl.select_new_events(vec![]);
        self.list.set_all_selected_event_ids(&[]);
        self.settle();
        let hit = self.list.quantity_hit_rect(id).expect("the quantity is on screen");
        (id, hit.center())
    }

    fn candies(&self, id: NodeId) -> i64 {
        self.ctrl.router.group(id).unwrap().event_definition.rare_candy.as_ref().unwrap().amount
    }
}

fn is_candy(d: &EventDefinition) -> bool {
    d.rare_candy.is_some()
}

fn is_item(d: &EventDefinition) -> bool {
    d.item_event_def.is_some()
}

#[test]
fn clicking_a_rare_candy_quantity_edits_it() {
    let mut h = Harness::new("f-ditto-tests.json");
    let (id, pos) = h.find(is_candy);
    h.click(pos);
    assert!(h.list.quantity_editor_text().is_some(), "the editor stays open after the click");
    assert_eq!(h.ctrl.get_all_selected_ids(true), vec![id], "the click selects the row");

    h.type_text("7");
    h.press(Key::Enter);
    assert!(h.list.quantity_editor_text().is_none(), "Enter closes the editor");
    assert_eq!(h.candies(id), 7, "the opening text was selected, so typing replaced it");
}

#[test]
fn clicking_an_item_quantity_edits_it() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    let (id, pos) = h.find(is_item);
    h.click(pos);
    h.type_text("7");
    h.press(Key::Enter);
    let amount = h.ctrl.router.group(id).unwrap().event_definition.item_event_def.as_ref().unwrap().item_amount;
    assert_eq!(amount, 7);
}

#[test]
fn clicking_inside_the_open_editor_keeps_what_was_typed() {
    let mut h = Harness::new("f-ditto-tests.json");
    let (id, pos) = h.find(is_candy);
    h.click(pos);
    h.type_text("7");
    h.click(pos);
    assert_eq!(h.list.quantity_editor_text(), Some("7"), "the click only moves the caret");
    h.press(Key::Enter);
    assert_eq!(h.candies(id), 7);
}

#[test]
fn escape_leaves_the_quantity_alone() {
    let mut h = Harness::new("f-ditto-tests.json");
    let (id, pos) = h.find(is_candy);
    let before = h.candies(id);
    h.click(pos);
    h.type_text("7");
    h.press(Key::Escape);
    assert!(h.list.quantity_editor_text().is_none());
    assert_eq!(h.candies(id), before);
}
