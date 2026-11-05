//! Input under a modal dialog, driven through a headless `egui::Context`
//! with synthetic input. `egui::Modal` only blocks widgets; the pieces that
//! read raw input (the route list's rows and keys, the inline creator's
//! Escape, the quick-add popover's keys and click-outside) must stand down
//! on their own (`xpr_ui_kit::modal`). A click "through" the "Assign Move
//! to Slot N" dialog used to select a route-list row, which moved where the
//! dialog's tutor event was inserted. The map view's cases (its keys, the
//! graphics tools, the navigator, the export dialog) live in
//! `map_modal_input.rs` on a map harness.

use std::path::PathBuf;
use std::sync::Arc;

use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2};

use xpr_app::assets::Assets;
use xpr_app::controller::MainController;
use xpr_app::dialogs::{block_secondary_window, AssignMoveDialog, DialogCtx, DialogOutcome};
use xpr_app::quick_add::QuickAddPopover;
use xpr_app::route_list::{ListActions, RouteList, ROW_HEIGHT};
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
    ctrl: MainController,
    list: RouteList,
    quick_add: QuickAddPopover,
    assets: Assets,
    dialog: Option<AssignMoveDialog>,
    /// What the dialog returned when it last closed.
    outcome: Option<DialogOutcome>,
    /// Draw a menu bar (before the page, as the app's own bar is) and
    /// where its "File" button landed.
    menu_bar: bool,
    menu_button: Option<Rect>,
    size: Vec2,
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
        let mut ctrl = MainController::new(registry.clone(), paths.clone());
        ctrl.load_route(&root.join("tests/test_data").join(route));
        let _ = ctrl.take_signals();
        let list = RouteList::new(&cfg);
        let mut h = Harness { ctx, theme, cfg, paths, registry, ctrl, list, quick_add: QuickAddPopover::new(), assets: Assets::new(), dialog: None, outcome: None, menu_bar: false, menu_button: None, size: Vec2::new(1000.0, 700.0) };
        h.frame(vec![]);
        h.frame(vec![]);
        h
    }

    /// One frame in the app's order: the menu bar (if any; at the bottom
    /// here so the list's rows stay where `row_pos` puts them), the page
    /// (here the route list), the quick-add popover, then the dialog on top
    /// of everything.
    fn frame(&mut self, events: Vec<Event>) {
        let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)), events, ..Default::default() };
        let Harness { ctx, theme, cfg, paths, registry, ctrl, list, quick_add, assets, dialog, outcome, menu_bar, menu_button, .. } = self;
        ctx.run_ui(input, |root| {
            let ctx = &root.ctx().clone();
            if *menu_bar {
                egui::Panel::bottom("menu_bar").show(root, |ui| {
                    egui::MenuBar::new().ui(ui, |ui| {
                        let r = ui.menu_button("File", |ui| {
                            for i in 0..8 {
                                let _ = ui.button(format!("Menu item {i}"));
                            }
                        });
                        *menu_button = Some(r.response.rect);
                    });
                });
            }
            egui::CentralPanel::default().show(root, |ui| {
                let mut actions = ListActions::default();
                list.ui(ui, theme, cfg, ctrl, &mut actions, true);
            });
            quick_add.ui(ctx, theme, ctrl, assets);
            if let Some(d) = dialog.as_mut() {
                let mut dctx = DialogCtx { theme, cfg, ctrl, registry, paths };
                let (close, out) = d.ui(ctx, &mut dctx);
                if close {
                    *outcome = out;
                    *dialog = None;
                }
            }
        }).drop_without_applying_deltas();
    }

    /// Open the "Assign Move to Slot 1" dialog and let it settle.
    fn open_dialog(&mut self) {
        let gen = self.ctrl.gen().expect("the route is loaded");
        self.dialog = Some(AssignMoveDialog::new(&gen, 0, false));
        for _ in 0..3 {
            self.frame(vec![]);
        }
    }

    /// Close the dialog; egui keeps blocking for the frame after.
    fn close_dialog(&mut self) {
        self.dialog = None;
        self.frame(vec![]);
        self.frame(vec![]);
    }

    fn dialog_rect(&self) -> Rect {
        self.ctx.memory(|m| m.area_rect(egui::Id::unique("xpr_assign_move"))).expect("the dialog is on screen")
    }

    fn click(&mut self, pos: Pos2) {
        let button = |pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        self.frame(vec![Event::PointerMoved(pos), button(true)]);
        self.frame(vec![button(false)]);
    }

    /// Press at `from`, move to `to` over a few frames, release there.
    fn drag(&mut self, from: Pos2, to: Pos2) {
        let button = |pos, pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        self.frame(vec![Event::PointerMoved(from), button(from, true)]);
        for i in 1..=8 {
            self.frame(vec![Event::PointerMoved(from + (to - from) * (i as f32 / 8.0))]);
        }
        self.frame(vec![button(to, false)]);
    }

    /// The events in the same folder as `id`, in order.
    fn siblings(&self, id: i64) -> Vec<i64> {
        let parent = self.ctrl.router.parent_of(id).expect("the event has a folder");
        self.ctrl.router.children_of(parent)
    }

    fn press(&mut self, key: Key) {
        self.frame(vec![Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
    }

    /// A point on the `idx`-th route-list row, at `x`.
    fn row_pos(idx: usize, x: f32) -> Pos2 {
        // panel margin + list frame stroke + header, then the rows
        Pos2::new(x, 8.0 + 1.0 + 22.0 + ROW_HEIGHT * (idx as f32 + 0.5))
    }

    fn selected(&self) -> Vec<i64> {
        self.ctrl.get_all_selected_ids(true)
    }
}

#[test]
fn clicks_and_keys_on_a_dialog_do_not_reach_the_route_list() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.click(Harness::row_pos(1, 300.0));
    let before = h.selected();
    assert!(!before.is_empty(), "a click on a row selects it");

    h.open_dialog();
    // on the backdrop, left of the dialog
    let dialog = h.dialog_rect();
    let backdrop = Harness::row_pos(3, 100.0);
    assert!(!dialog.contains(backdrop), "{dialog:?} leaves room for a backdrop click beside it");
    h.click(backdrop);
    assert_eq!(h.selected(), before, "a click on the backdrop must not select the row under it");
    // on the dialog itself, over a row
    let idx = ((dialog.center().y - 31.0) / ROW_HEIGHT) as usize;
    h.click(Harness::row_pos(idx, dialog.min.x + 4.0));
    assert!(h.dialog.is_some(), "the dialog is still open");
    assert_eq!(h.selected(), before, "nor a click on the dialog");
    h.press(Key::ArrowDown);
    assert_eq!(h.selected(), before, "the list's arrow keys stand down too");

    // with the dialog gone the list takes clicks again (and the row geometry is right)
    h.close_dialog();
    h.click(Harness::row_pos(3, 100.0));
    let after = h.selected();
    assert!(!after.is_empty() && after != before, "with the dialog gone a click selects again");
}

/// The dialog opens with the top match selected and Enter assigns it, so it
/// works from the keyboard alone: straight away, and after typing a filter.
#[test]
fn enter_assigns_the_top_match_in_the_assign_move_dialog() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    let first = h.ctrl.gen().unwrap().move_db().get_filtered_names(None, false)[0].clone();
    h.open_dialog();
    h.press(Key::Enter);
    assert!(h.dialog.is_none(), "Enter closes the dialog");
    assert!(matches!(&h.outcome, Some(DialogOutcome::AssignMove { slot: 0, mv, .. }) if *mv == first), "Enter assigns the top move");

    h.open_dialog();
    h.frame(vec![Event::Text("thunderb".into())]);
    h.frame(vec![]);
    h.press(Key::Enter);
    assert!(matches!(&h.outcome, Some(DialogOutcome::AssignMove { mv, .. }) if mv.to_lowercase().contains("thunderb")), "Enter assigns the top filtered match");
}

#[test]
fn a_drag_on_a_dialog_does_not_move_route_list_rows() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    // rows 3-6 are the four events of the Viridian City folder
    h.click(Harness::row_pos(3, 300.0));
    let event = h.selected()[0];
    let before = h.siblings(event);

    h.open_dialog();
    let dialog = h.dialog_rect();
    // across the dialog, over the rows under it
    let top = ((dialog.min.y + 10.0 - 31.0) / ROW_HEIGHT).ceil() as usize;
    let (from, to) = (Harness::row_pos(top, dialog.min.x + 4.0), Harness::row_pos(top + 3, dialog.min.x + 4.0));
    assert!(dialog.contains(from) && dialog.contains(to), "{dialog:?} covers the drag");
    h.drag(from, to);
    // and across the backdrop beside it
    h.drag(Harness::row_pos(3, 100.0), Harness::row_pos(6, 100.0));
    assert_eq!(h.siblings(event), before, "a drag that starts under a dialog moves nothing");

    // a press on the backdrop, then Escape with the button still down: the
    // rest of the drag is no longer under the dialog, but its press was
    let (from, to) = (Harness::row_pos(3, 100.0), Harness::row_pos(5, 100.0));
    let button = |pos, pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    h.frame(vec![Event::PointerMoved(from), button(from, true)]);
    h.press(Key::Escape);
    assert!(h.dialog.is_none(), "Escape cancels the dialog");
    for i in 1..=8 {
        h.frame(vec![Event::PointerMoved(from + (to - from) * (i as f32 / 8.0))]);
    }
    h.frame(vec![button(to, false)]);
    assert_eq!(h.siblings(event), before, "a press under a dialog does not become a drag once it closes");

    // with the dialog gone the same drag moves the event
    h.close_dialog();
    h.drag(Harness::row_pos(3, 100.0), Harness::row_pos(5, 100.0));
    assert_ne!(h.siblings(event), before, "with the dialog gone a drag moves the event");
}

#[test]
fn clicks_while_a_menu_is_open_do_not_reach_the_route_list() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.menu_bar = true;
    h.frame(vec![]);
    h.click(Harness::row_pos(1, 300.0));
    let before = h.selected();
    assert!(!before.is_empty(), "a click on a row selects it");

    // a click outside the menu only closes it
    let button = h.menu_button.expect("the menu bar is drawn");
    h.click(button.center());
    assert!(egui::Popup::is_any_open(&h.ctx), "the menu opened");
    h.click(Harness::row_pos(3, 600.0));
    assert!(!egui::Popup::is_any_open(&h.ctx), "the click closed the menu");
    assert_eq!(h.selected(), before, "the click that closes a menu must not select the row under it");

    // nor does a click on one of its items over the list
    h.click(button.center());
    assert!(egui::Popup::is_any_open(&h.ctx), "the menu opened again");
    let item = Pos2::new(button.min.x + 20.0, button.min.y - 40.0);
    h.click(item);
    assert!(!egui::Popup::is_any_open(&h.ctx), "picking an item closes the menu");
    assert_eq!(h.selected(), before, "a click on a menu item must not select the row under it");

    // with the menu closed the list takes clicks again
    h.click(Harness::row_pos(3, 600.0));
    let after = h.selected();
    assert!(!after.is_empty() && after != before, "with the menu closed a click selects again");
}

#[test]
fn a_drag_while_a_menu_is_open_does_not_move_route_list_rows() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.menu_bar = true;
    h.frame(vec![]);
    // rows 3-6 are the four events of the Viridian City folder
    h.click(Harness::row_pos(3, 600.0));
    let event = h.selected()[0];
    let before = h.siblings(event);

    let button = h.menu_button.expect("the menu bar is drawn");
    h.click(button.center());
    assert!(egui::Popup::is_any_open(&h.ctx), "the menu opened");
    h.drag(Harness::row_pos(3, 600.0), Harness::row_pos(5, 600.0));
    assert!(egui::Popup::is_any_open(&h.ctx), "a drag is not the click that closes a menu");
    assert_eq!(h.siblings(event), before, "a drag under an open menu moves nothing");

    // with the menu closed the same drag moves the event
    h.click(Harness::row_pos(8, 600.0));
    assert!(!egui::Popup::is_any_open(&h.ctx), "the click closed the menu");
    h.drag(Harness::row_pos(3, 600.0), Harness::row_pos(5, 600.0));
    assert_ne!(h.siblings(event), before, "with the menu closed a drag moves the event");
}

#[test]
fn escape_on_a_dialog_does_not_discard_the_inline_creator() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.click(Harness::row_pos(1, 300.0));
    let mut actions = ListActions::default();
    h.list.start_add_new_event(&mut h.ctrl, &mut actions);
    h.frame(vec![]);
    h.frame(vec![]);
    assert!(h.list.has_inline_creator());

    h.open_dialog();
    h.press(Key::Escape);
    assert!(h.dialog.is_none(), "Escape cancels the dialog");
    assert!(h.list.has_inline_creator(), "and only the dialog");

    h.close_dialog();
    h.press(Key::Escape);
    assert!(!h.list.has_inline_creator(), "with the dialog gone Escape discards the creator");
}

#[test]
fn the_quick_add_popover_stands_down_under_a_dialog() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.quick_add.show_above(&h.ctrl, Pos2::new(500.0, 650.0), None);
    h.frame(vec![]);
    h.frame(vec![]);
    assert!(h.quick_add.open);

    h.open_dialog();
    let dialog = h.dialog_rect();
    h.click(dialog.center());
    assert!(h.quick_add.open, "a click on the dialog is not a click outside the popover");
    h.press(Key::Space);
    assert!(h.quick_add.open, "Space belongs to the dialog");

    h.close_dialog();
    h.press(Key::Space);
    assert!(!h.quick_add.open, "with the dialog gone Space closes the popover");
}

#[test]
fn a_secondary_window_is_covered_while_a_dialog_is_up() {
    let cfg = Config::load(&xpr_app::scratch_config_path());
    let ctx = egui::Context::default();
    // a native window (its own viewport), as eframe gives the app
    ctx.set_embed_viewports(false);
    let mut theme = Theme::from_config(&cfg);
    theme.install_fonts(&ctx);
    theme.apply(&ctx);
    let clicks = |blocked: bool| -> usize {
        let mut clicks = 0;
        let pos = Pos2::new(40.0, 20.0);
        let button = |pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        for events in [vec![], vec![], vec![], vec![Event::PointerMoved(pos), button(true)], vec![button(false)]] {
            let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0))), events, ..Default::default() };
            ctx.run_ui(input, |root| {
                let ctx = &root.ctx().clone();
                egui::CentralPanel::default().show(root, |ui| {
                    if ui.button("Add trainer").clicked() {
                        clicks += 1;
                    }
                });
                if blocked {
                    block_secondary_window(ctx, &theme);
                }
            }).drop_without_applying_deltas();
        }
        clicks
    };
    assert_eq!(clicks(true), 0, "the window's own widgets are covered");
    assert_eq!(clicks(false), 1, "and work again once the dialog is gone");
}

/// The landing page's "Route from Replay" panel cancels on Escape, but not
/// when Escape belongs to a dialog drawn over it.
#[test]
fn escape_under_a_dialog_does_not_cancel_the_replay_import() {
    let cfg = Config::load(&xpr_app::scratch_config_path());
    let ctx = egui::Context::default();
    let mut theme = Theme::from_config(&cfg);
    theme.install_fonts(&ctx);
    theme.apply(&ctx);
    let status = xpr_app::replay_import::ImportStatus { phase: "record".into(), frame: 50, total: 100, ..Default::default() };
    let run = |with_dialog: bool, events: Vec<Event>| -> bool {
        let mut cancel = false;
        let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0))), events, ..Default::default() };
        ctx.run_ui(input, |root| {
            let ctx = &root.ctx().clone();
            egui::CentralPanel::default().show(root, |ui| {
                xpr_app::pages::replay_import_ui(ui, &theme, &status, &mut cancel);
            });
            if with_dialog {
                xpr_ui_kit::modal::modal(ctx, &theme, "xpr_test_dialog", "A dialog", 300.0, |ui| {
                    ui.label("on top");
                });
            }
        }).drop_without_applying_deltas();
        cancel
    };
    let escape = || vec![Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }];
    for _ in 0..3 {
        run(true, vec![]);
    }
    assert!(!run(true, escape()), "Escape belongs to the dialog");
    for _ in 0..3 {
        run(false, vec![]);
    }
    assert!(run(false, escape()), "without a dialog Escape cancels the import");
}
/// Clicking a folder's chevron collapses it and clicking again expands it.
/// The rebuild used to restore the expand state from the old rows, so the
/// folder stayed open on screen whatever the chevron did.
#[test]
fn a_chevron_click_collapses_and_expands_a_folder() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    let folders = h.ctrl.router.root().children.clone();
    let expanded = |h: &Harness, i: usize| h.ctrl.router.folder(folders[i]).unwrap().is_expanded();
    assert!(expanded(&h, 0) && expanded(&h, 1));
    h.click(Harness::row_pos(0, 19.0));
    h.frame(vec![]);
    assert!(!expanded(&h, 0), "the first click collapses the folder");
    // with its children hidden the next folder is the second row
    h.click(Harness::row_pos(1, 19.0));
    h.frame(vec![]);
    assert!(!expanded(&h, 1), "the folder's children are hidden from the list");
    h.click(Harness::row_pos(0, 19.0));
    h.frame(vec![]);
    assert!(expanded(&h, 0), "a second click expands it again");
}
