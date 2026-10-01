//! Dragging route-list rows to move their events, driven through a headless
//! `egui::Context` with synthetic input. The drag used to start on the
//! click, which egui reports on release, so it ended in the frame it began.
//! On a route taller than the window, egui's drag-to-scroll also took the
//! drag and scrolled the list with the pointer instead.

use std::path::PathBuf;
use std::sync::Arc;

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2};

use xpr_app::controller::MainController;
use xpr_app::route_list::{ListActions, RouteList, ROW_HEIGHT};
use xpr_core::{Config, Paths};
use xpr_data::Registry;
use xpr_engine::NodeId;
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
    size: Vec2,
}

impl Harness {
    /// A window short enough that the test route scrolls.
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
        let mut h = Harness { ctx, theme, cfg, ctrl, list, size: Vec2::new(1000.0, 300.0) };
        h.frame(vec![]);
        h.frame(vec![]);
        h
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.frame_with(events, Modifiers::NONE);
    }

    /// A frame with `modifiers` held (egui reads them from the raw input,
    /// not from the pointer events).
    fn frame_with(&mut self, events: Vec<Event>, modifiers: Modifiers) {
        let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)), events, modifiers, ..Default::default() };
        let Harness { ctx, theme, cfg, ctrl, list, .. } = self;
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut actions = ListActions::default();
                list.ui(ui, theme, cfg, ctrl, &mut actions, true);
            });
        });
        // the app rebuilds the list's rows when the route changes
        if self.ctrl.take_signals().route_changed {
            self.list.mark_dirty();
        }
    }

    fn button(pos: Pos2, pressed: bool, modifiers: Modifiers) -> Event {
        Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers }
    }

    fn click_with(&mut self, pos: Pos2, modifiers: Modifiers) {
        self.frame_with(vec![Event::PointerMoved(pos), Harness::button(pos, true, modifiers)], modifiers);
        self.frame_with(vec![Harness::button(pos, false, modifiers)], modifiers);
    }

    fn click(&mut self, pos: Pos2) {
        self.click_with(pos, Modifiers::NONE);
    }

    /// Press at `from`, move to `to` over a few frames, release there.
    fn drag(&mut self, from: Pos2, to: Pos2) {
        self.frame(vec![Event::PointerMoved(from), Harness::button(from, true, Modifiers::NONE)]);
        for i in 1..=8 {
            self.frame(vec![Event::PointerMoved(from + (to - from) * (i as f32 / 8.0))]);
        }
        self.frame(vec![Harness::button(to, false, Modifiers::NONE)]);
        self.frame(vec![]);
    }

    /// A point on the `idx`-th route-list row, at `x`, `dy` below its centre.
    fn row_pos(idx: usize, dy: f32) -> Pos2 {
        // panel margin + list frame stroke + header, then the rows
        Pos2::new(300.0, 8.0 + 1.0 + 22.0 + ROW_HEIGHT * (idx as f32 + 0.5) + dy)
    }

    /// The event on the `idx`-th row (clicks it).
    fn id_at(&mut self, idx: usize) -> NodeId {
        self.click(Harness::row_pos(idx, 0.0));
        let ids = self.ctrl.get_all_selected_ids(true);
        assert_eq!(ids.len(), 1, "a click on row {idx} selects just it");
        ids[0]
    }

    fn siblings(&self, id: NodeId) -> Vec<NodeId> {
        let parent = self.ctrl.router.parent_of(id).expect("the event has a folder");
        self.ctrl.router.children_of(parent)
    }
}

/// Rows 0-6 of the test route: the Pallet Town folder and its trainer, then
/// the Viridian City folder and its four inventory events.
const FIRST_VIRIDIAN_EVENT: usize = 3;

#[test]
fn dragging_a_row_moves_its_event() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    let a = h.id_at(FIRST_VIRIDIAN_EVENT);
    let c = h.id_at(FIRST_VIRIDIAN_EVENT + 2);
    let before = h.siblings(a);
    assert_eq!(before.len(), 4, "the Viridian City folder holds four events");
    assert_eq!(before[0], a);
    assert_eq!(before[2], c);

    // the first event onto the lower half of the third: it lands below it
    h.drag(Harness::row_pos(FIRST_VIRIDIAN_EVENT, 0.0), Harness::row_pos(FIRST_VIRIDIAN_EVENT + 2, 5.0));
    let after = h.siblings(a);
    assert_eq!(after, vec![before[1], before[2], a, before[3]], "the dragged event moved below the drop row");
    assert_eq!(h.ctrl.get_all_selected_ids(true), vec![a], "the dragged event stays selected");

    // and back above the (now) first event
    h.drag(Harness::row_pos(FIRST_VIRIDIAN_EVENT + 2, 0.0), Harness::row_pos(FIRST_VIRIDIAN_EVENT, -5.0));
    assert_eq!(h.siblings(a), before, "the event moved back above the drop row");
}

#[test]
fn dragging_an_unselected_row_drags_just_that_row() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    let a = h.id_at(FIRST_VIRIDIAN_EVENT);
    let before = h.siblings(a);
    let b = before[1];
    let d = before[3];
    // `a` is selected; drag `b` instead
    h.drag(Harness::row_pos(FIRST_VIRIDIAN_EVENT + 1, 0.0), Harness::row_pos(FIRST_VIRIDIAN_EVENT + 3, 5.0));
    assert_eq!(h.siblings(a), vec![a, before[2], d, b], "only the pressed row moved");
    assert_eq!(h.ctrl.get_all_selected_ids(true), vec![b], "the dragged row is now the selection");
}

#[test]
fn dragging_a_selected_row_drags_the_whole_selection() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    let a = h.id_at(FIRST_VIRIDIAN_EVENT);
    let before = h.siblings(a);
    h.click_with(Harness::row_pos(FIRST_VIRIDIAN_EVENT + 1, 0.0), Modifiers::COMMAND);
    assert_eq!(h.ctrl.get_all_selected_ids(true).len(), 2, "two rows are selected");
    // drag the pair below the last event
    h.drag(Harness::row_pos(FIRST_VIRIDIAN_EVENT + 1, 0.0), Harness::row_pos(FIRST_VIRIDIAN_EVENT + 3, 5.0));
    assert_eq!(h.siblings(a), vec![before[2], before[3], before[0], before[1]], "both selected events moved");
}

#[test]
fn a_short_wiggle_is_still_a_click() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    let a = h.id_at(FIRST_VIRIDIAN_EVENT);
    let before = h.siblings(a);
    h.drag(Harness::row_pos(FIRST_VIRIDIAN_EVENT + 1, 0.0), Harness::row_pos(FIRST_VIRIDIAN_EVENT + 1, 3.0));
    assert_eq!(h.siblings(a), before, "nothing moved");
    assert_eq!(h.ctrl.get_all_selected_ids(true), vec![before[1]], "the press-and-release selected the row");
}

#[test]
fn a_drop_outside_the_rows_does_nothing() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    let a = h.id_at(FIRST_VIRIDIAN_EVENT);
    let before = h.siblings(a);
    // across the rows, then up onto the header
    h.frame(vec![Event::PointerMoved(Harness::row_pos(FIRST_VIRIDIAN_EVENT, 0.0)), Harness::button(Harness::row_pos(FIRST_VIRIDIAN_EVENT, 0.0), true, Modifiers::NONE)]);
    for p in [Harness::row_pos(FIRST_VIRIDIAN_EVENT + 2, 5.0), Harness::row_pos(FIRST_VIRIDIAN_EVENT + 2, 5.0), Pos2::new(300.0, 18.0)] {
        h.frame(vec![Event::PointerMoved(p)]);
    }
    h.frame(vec![Harness::button(Pos2::new(300.0, 18.0), false, Modifiers::NONE)]);
    h.frame(vec![]);
    assert_eq!(h.siblings(a), before, "a drop on the header moves nothing");
}

#[test]
fn dropping_onto_a_folder_moves_the_event_into_it() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    // row 0 is the Pallet Town folder, row 2 the Viridian City one
    let pallet_trainer = h.id_at(1);
    let pallet = h.ctrl.router.parent_of(pallet_trainer).unwrap();
    let a = h.id_at(FIRST_VIRIDIAN_EVENT);
    // onto the middle of the Pallet Town row
    h.drag(Harness::row_pos(FIRST_VIRIDIAN_EVENT, 0.0), Harness::row_pos(0, 0.0));
    assert_eq!(h.ctrl.router.parent_of(a), Some(pallet), "the event moved into the folder");
    assert_eq!(h.ctrl.router.children_of(pallet), vec![pallet_trainer, a], "at its end");
}

#[test]
fn holding_a_drag_at_the_bottom_edge_scrolls_the_list() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    let a = h.id_at(FIRST_VIRIDIAN_EVENT);
    // Pewter City, which starts below the window
    let last_folder = *h.ctrl.router.children_of(h.ctrl.router.root_id).last().unwrap();
    let from = Harness::row_pos(FIRST_VIRIDIAN_EVENT, 0.0);
    // the last visible row, deep in the scroll margin at the bottom of the list
    let edge = Pos2::new(300.0, h.size.y - 25.0);
    h.frame(vec![Event::PointerMoved(from), Harness::button(from, true, Modifiers::NONE)]);
    for _ in 0..60 {
        h.frame(vec![Event::PointerMoved(edge)]);
    }
    h.frame(vec![Harness::button(edge, false, Modifiers::NONE)]);
    h.frame(vec![]);
    assert_eq!(h.ctrl.router.parent_of(a), Some(last_folder), "the list scrolled to the end and the event moved there");
}
