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
use xpr_app::dialogs::{block_secondary_window, AssignMoveDialog, DialogCtx};
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
        let mut h = Harness { ctx, theme, cfg, paths, registry, ctrl, list, quick_add: QuickAddPopover::new(), assets: Assets::new(), dialog: None, menu_bar: false, menu_button: None, size: Vec2::new(1000.0, 700.0) };
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
        let Harness { ctx, theme, cfg, paths, registry, ctrl, list, quick_add, assets, dialog, menu_bar, menu_button, .. } = self;
        let _ = ctx.run(input, |ctx| {
            if *menu_bar {
                egui::TopBottomPanel::bottom("menu_bar").show(ctx, |ui| {
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
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut actions = ListActions::default();
                list.ui(ui, theme, cfg, ctrl, &mut actions, true);
            });
            quick_add.ui(ctx, theme, ctrl, assets);
            if let Some(d) = dialog.as_mut() {
                let mut dctx = DialogCtx { theme, cfg, ctrl, registry, paths };
                let (close, _) = d.ui(ctx, &mut dctx);
                if close {
                    *dialog = None;
                }
            }
        });
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
        self.ctx.memory(|m| m.area_rect(egui::Id::new("xpr_assign_move"))).expect("the dialog is on screen")
    }

    fn click(&mut self, pos: Pos2) {
        let button = |pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        self.frame(vec![Event::PointerMoved(pos), button(true)]);
        self.frame(vec![button(false)]);
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
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    if ui.button("Add trainer").clicked() {
                        clicks += 1;
                    }
                });
                if blocked {
                    block_secondary_window(ctx, &theme);
                }
            });
        }
        clicks
    };
    assert_eq!(clicks(true), 0, "the window's own widgets are covered");
    assert_eq!(clicks(false), 1, "and work again once the dialog is gone");
}
