//! Message boxes driven through a headless `egui::Context` with synthetic
//! input: each button of the unsaved-changes prompts (quit, close route, new
//! route from current) answers what its label says, the buttons fit on one
//! row, and Enter / Escape stay on the safe side.


use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape, Vec2};

use xpr_app::dialogs::{MessageBox, MsgButtons, MsgChoice, MsgTag};
use xpr_core::Config;
use xpr_ui_kit::theme::Theme;


struct Harness {
    ctx: egui::Context,
    theme: Theme,
    mb: MessageBox,
    /// Every text drawn last frame, with its rect.
    texts: Vec<(String, Rect)>,
}

/// The three unsaved-changes prompts, as `app.rs` raises them:
/// (text, "Save, then …" label, "… without saving" label, tag).
const PROMPTS: [(&str, &str, &str, MsgTag); 3] = [
    ("Route has unsaved changes. Save before quitting?", "Save, then quit", "Quit without saving", MsgTag::QuitUnsaved),
    ("Route has unsaved changes. Save before closing?", "Save, then close", "Close without saving", MsgTag::CloseRouteUnsaved),
    ("Route has unsaved changes. Save before starting a new route?", "Save, then start new route", "Start without saving", MsgTag::NewRouteFromCurrentUnsaved),
];

impl Harness {
    fn close_route_prompt() -> Harness {
        Harness::prompt(1)
    }

    fn prompt(idx: usize) -> Harness {
        let (text, save, discard, tag) = PROMPTS[idx].clone();
        let cfg = Config::load(&xpr_app::scratch_config_path());
        let ctx = egui::Context::default();
        let mut theme = Theme::from_config(&cfg);
        theme.install_fonts(&ctx);
        theme.apply(&ctx);
        let mb = MessageBox::new("Unsaved Changes", text, MsgButtons::SaveDiscardCancel { save, discard }, tag);
        let mut h = Harness { ctx, theme, mb, texts: Vec::new() };
        for _ in 0..3 {
            assert_eq!(h.frame(vec![]), None, "nothing is answered on its own");
        }
        h
    }

    fn frame(&mut self, events: Vec<Event>) -> Option<MsgChoice> {
        let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0))), events, ..Default::default() };
        let mut choice = None;
        let out = self.ctx.run(input, |ctx| choice = self.mb.ui(ctx, &self.theme));
        self.texts.clear();
        for clipped in &out.shapes {
            collect_texts(&clipped.shape, &mut self.texts);
        }
        choice
    }

    fn button(&self, label: &str) -> Rect {
        self.texts.iter().find(|(t, _)| t == label).map(|(_, r)| *r).unwrap_or_else(|| panic!("no {label:?} button"))
    }

    /// Press and release on `label`; the answer comes with the release.
    fn click(&mut self, label: &str) -> Option<MsgChoice> {
        let pos = self.button(label).center();
        let button = |pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        let pressed = self.frame(vec![Event::PointerMoved(pos), button(true)]);
        pressed.or(self.frame(vec![button(false)]))
    }

    fn press(&mut self, key: Key) -> Option<MsgChoice> {
        self.frame(vec![Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }])
    }
}

fn collect_texts(shape: &Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Text(t) => out.push((t.galley.text().to_string(), t.visual_bounding_rect())),
        Shape::Vec(v) => v.iter().for_each(|s| collect_texts(s, out)),
        _ => {}
    }
}

#[test]
fn close_without_saving_answers_no() {
    let mut h = Harness::close_route_prompt();
    assert_eq!(h.click("Close without saving"), Some(MsgChoice::No));
}

#[test]
fn save_then_close_answers_yes() {
    let mut h = Harness::close_route_prompt();
    assert_eq!(h.click("Save, then close"), Some(MsgChoice::Yes));
}

#[test]
fn cancel_answers_cancel() {
    let mut h = Harness::close_route_prompt();
    assert_eq!(h.click("Cancel"), Some(MsgChoice::Cancel));
}

#[test]
fn enter_and_escape_cancel() {
    let mut h = Harness::close_route_prompt();
    assert_eq!(h.press(Key::Enter), Some(MsgChoice::Cancel), "Enter never discards or saves by accident");
    let mut h = Harness::close_route_prompt();
    assert_eq!(h.press(Key::Escape), Some(MsgChoice::Cancel));
}

#[test]
fn the_labels_replace_yes_and_no() {
    let h = Harness::close_route_prompt();
    for gone in ["Yes", "No"] {
        assert!(h.texts.iter().all(|(t, _)| t != gone), "no bare {gone:?} button");
    }
    // the discard button sits apart, at the left of the footer
    let discard = h.button("Close without saving");
    let save = h.button("Save, then close");
    let cancel = h.button("Cancel");
    assert!(discard.max.x < cancel.min.x && cancel.max.x < save.min.x, "discard | … | cancel, save");
}

#[test]
fn every_unsaved_prompt_names_its_buttons_and_fits_them() {
    for (idx, (_, save, discard, tag)) in PROMPTS.iter().enumerate() {
        let h = Harness::prompt(idx);
        let dialog = h.ctx.memory(|m| m.area_rect(egui::Id::new("xpr_message_box"))).expect("the prompt is on screen");
        let (d, c, s) = (h.button(discard), h.button("Cancel"), h.button(save));
        assert!(d.max.x < c.min.x && c.max.x < s.min.x, "{tag:?}: discard | … | cancel, save on one row");
        assert!((d.center().y - s.center().y).abs() < 1.0, "{tag:?}: one row");
        assert!(dialog.contains_rect(d) && dialog.contains_rect(s), "{tag:?}: the buttons fit inside the dialog");
        for (label, want) in [(*save, MsgChoice::Yes), (*discard, MsgChoice::No), ("Cancel", MsgChoice::Cancel)] {
            let mut h = Harness::prompt(idx);
            assert_eq!(h.click(label), Some(want), "{tag:?}: {label:?}");
        }
    }
}
