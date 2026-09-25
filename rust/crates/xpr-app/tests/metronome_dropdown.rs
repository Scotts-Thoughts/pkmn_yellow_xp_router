//! The Metronome move selector in the battle summary, driven through a
//! headless `egui::Context` with synthetic input. Its list used to open only
//! from a click on the text: the arrow's click took the entry's focus, and
//! the dropdown then closed the list again in the same frame (committing
//! the best match of any typed text) before it was ever shown. And with
//! hundreds of callable moves, the arrow keys walked the highlight out of
//! the list's view.

use std::path::PathBuf;
use std::sync::Arc;

use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2};

use xpr_app::assets::Assets;
use xpr_app::battle_ui::BattleUiActions;
use xpr_app::controller::MainController;
use xpr_app::event_details::{DetailsActions, EventDetails};
use xpr_core::{consts, Config, Paths};
use xpr_data::Registry;
use xpr_engine::events::{EventDefinition, LearnMoveEventDefinition, LevelVal};
use xpr_engine::NodeId;
use xpr_ui_kit::theme::Theme;
use xpr_ui_kit::widgets;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

/// A text drawn in the last frame, and the clip rect it was drawn under.
struct DrawnText {
    text: String,
    rect: Rect,
    clip: Rect,
}

impl DrawnText {
    fn visible(&self) -> bool {
        self.clip.contains_rect(self.rect)
    }
}

struct Harness {
    ctx: egui::Context,
    theme: Theme,
    cfg: Config,
    ctrl: MainController,
    details: EventDetails,
    assets: Assets,
    fight: NodeId,
    texts: Vec<DrawnText>,
    /// Vertical line segments drawn in the last frame, as (x, top, bottom).
    vlines: Vec<(f32, f32, f32)>,
    /// Where the selector's text and arrow are (the selector stays put).
    entry: Rect,
    arrow: Pos2,
}

impl Harness {
    /// The route's first trainer fight, with the player's mon taught
    /// Metronome (slot 1) by a tutor event right before it, on the battle
    /// summary.
    fn new(route: &str) -> Harness {
        let root = repo_root();
        let cfg = Config::load(&xpr_app::scratch_config_path());
        let ctx = egui::Context::default();
        let mut theme = Theme::from_config(&cfg);
        theme.install_fonts(&ctx);
        theme.apply(&ctx);
        let registry = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
        let mut ctrl = MainController::new(registry, Paths::new(root.clone()));
        ctrl.load_route(&root.join("tests/test_data").join(route));
        let fight = ctrl
            .router
            .all_groups()
            .into_iter()
            .find(|g| ctrl.router.group(*g).unwrap().event_definition.trainer_def.is_some())
            .expect("a trainer fight");
        let tutor = LearnMoveEventDefinition::new(Some(consts::METRONOME_MOVE_NAME), Some(0), consts::MOVE_SOURCE_TUTOR, LevelVal::any(), None, true);
        ctrl.new_event(EventDefinition::with_learn_move(tutor), None, Some(fight), None, false);
        ctrl.select_new_events(vec![fight]);
        let _ = ctrl.take_signals();
        let mut details = EventDetails::new(&cfg);
        details.handle_selection(&cfg, &mut ctrl, &mut DetailsActions::default());
        let mut h = Harness { ctx, theme, cfg, ctrl, details, assets: Assets::new(), fight, texts: Vec::new(), vlines: Vec::new(), entry: Rect::NOTHING, arrow: Pos2::ZERO };
        h.signals();
        h.details.battle_ui.show_contents();
        for _ in 0..3 {
            h.frame(vec![]);
        }
        assert_eq!(h.pick(), "", "Metronome starts with no pick");
        h.locate_selector();
        h
    }

    /// The frame's signal handling as `App` runs it for the details panel.
    fn signals(&mut self) {
        let sig = self.ctrl.take_signals();
        if sig.route_changed {
            self.details.handle_route_change(&self.cfg, &mut self.ctrl);
        }
        self.details.drain_battle_signals(&self.cfg, &mut self.ctrl);
    }

    /// One frame: the battle summary in its scroll area, as on the battle tab.
    fn frame(&mut self, events: Vec<Event>) {
        let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 900.0))), events, ..Default::default() };
        let Harness { ctx, theme, cfg, ctrl, details, assets, .. } = self;
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                widgets::show_scroll(ui, egui::ScrollArea::vertical().id_salt("battle_scroll").auto_shrink([false, false]), |ui| {
                    let mut actions = BattleUiActions::default();
                    details.battle_ui.ui(ui, theme, cfg, &mut details.bc, ctrl, assets, &mut actions);
                });
            });
        });
        self.texts.clear();
        self.vlines.clear();
        for cs in &out.shapes {
            self.collect(&cs.shape, cs.clip_rect);
        }
        self.signals();
    }

    fn collect(&mut self, shape: &egui::Shape, clip: Rect) {
        match shape {
            egui::Shape::Text(t) => self.texts.push(DrawnText { text: t.galley.text().to_string(), rect: t.galley.rect.translate(t.pos.to_vec2()), clip }),
            egui::Shape::LineSegment { points: [a, b], .. } if a.x == b.x => self.vlines.push((a.x, a.y.min(b.y), a.y.max(b.y))),
            egui::Shape::Vec(v) => v.iter().for_each(|s| self.collect(s, clip)),
            _ => {}
        }
    }

    fn click(&mut self, pos: Pos2) {
        let button = |pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        self.frame(vec![Event::PointerMoved(pos), button(true)]);
        self.frame(vec![button(false)]);
        self.frame(vec![]);
    }

    fn type_text(&mut self, t: &str) {
        self.frame(vec![Event::Text(t.to_string())]);
        self.frame(vec![]);
    }

    fn press(&mut self, key: Key) {
        let ev = |pressed| Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE };
        self.frame(vec![ev(true)]);
        self.frame(vec![ev(false)]);
    }

    /// Find the unpicked selector by its "Metronome" placeholder, and its
    /// arrow right of the separator line drawn beside the text.
    fn locate_selector(&mut self) {
        let found: Vec<Rect> = self.texts.iter().filter(|t| t.text == consts::METRONOME_MOVE_NAME && t.visible()).map(|t| t.rect).collect();
        assert_eq!(found.len(), 1, "one Metronome placeholder on screen");
        let entry = found[0];
        let y = entry.center().y;
        let x = self
            .vlines
            .iter()
            .filter(|(x, top, bottom)| *x > entry.min.x && *top < y && *bottom > y)
            .map(|(x, _, _)| *x)
            .fold(f32::INFINITY, f32::min);
        assert!(x.is_finite(), "the selector's arrow");
        self.entry = entry;
        self.arrow = Pos2::new(x + 10.0, y);
    }

    /// A row of the open list, if it is in view.
    fn row(&self, name: &str) -> Option<Rect> {
        self.texts.iter().find(|t| t.text == name && t.rect.min.y > self.entry.max.y && t.visible()).map(|t| t.rect)
    }

    fn list_open(&self) -> bool {
        // gen 1's first callable move, right under the blank row
        self.row("Pound").is_some()
    }

    fn player_metronome(&self) -> xpr_calc::battle_summary::MoveRenderInfo {
        self.details.bc.summary.player_move_data[0][0].clone().expect("the Metronome slot")
    }

    fn pick(&self) -> String {
        self.player_metronome().metronome_selection.expect("a Metronome slot")
    }

    /// The pick as saved with the fight (after the delayed save).
    fn saved_pick(&mut self) -> Option<String> {
        self.details.force_and_clear_event_update(&self.cfg, &mut self.ctrl);
        self.signals();
        let td = self.ctrl.router.group(self.fight).unwrap().event_definition.trainer_def.clone().unwrap();
        td.custom_move_data.first().and_then(|c| c.player.get(consts::METRONOME_MOVE_NAME).cloned())
    }
}

#[test]
fn the_arrow_opens_and_closes_the_list() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    assert!(!h.list_open());

    // unfocused selector: one click on the arrow shows the list...
    h.click(h.arrow);
    assert!(h.list_open(), "the arrow opened the list");
    // ... and the next closes it again, picking nothing
    h.click(h.arrow);
    assert!(!h.list_open(), "the arrow closed the list");
    assert_eq!(h.pick(), "");
    // the entry kept the focus, and the arrow reopens the list
    h.click(h.arrow);
    assert!(h.list_open(), "the arrow reopened the list");

    // a click on a row picks that move
    let row = h.row("Karate Chop").expect("Karate Chop in view");
    h.click(row.center());
    assert!(!h.list_open());
    assert_eq!(h.pick(), "Karate Chop");
    assert!(h.player_metronome().min_damage > 0, "the slot shows Karate Chop's damage");
    assert_eq!(h.saved_pick().as_deref(), Some("Karate Chop"));
    h.frame(vec![]);

    // with a pick, the arrow lists everything again, and closing it keeps the pick
    h.click(h.arrow);
    assert!(h.list_open());
    h.click(h.arrow);
    assert!(!h.list_open());
    assert_eq!(h.pick(), "Karate Chop");
}

#[test]
fn the_arrow_does_not_commit_typed_text() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.click(h.entry.center());
    assert!(h.list_open(), "a click on the text opens the list");
    h.type_text("thunder");
    assert!(h.row("Thunderbolt").is_some(), "the list is filtered to the typed text");
    let arrow = h.arrow;

    // closing the filtered list with the arrow is not a pick of its best match
    h.click(arrow);
    assert!(h.row("Thunderbolt").is_none(), "the arrow closed the list");
    assert_eq!(h.pick(), "", "closing the list picked nothing");

    // reopening shows the same filtered rows; a click picks one
    h.click(arrow);
    let row = h.row("Thunderbolt").expect("the filtered list reopened");
    assert!(h.row("Pound").is_none(), "still filtered");
    h.click(row.center());
    assert_eq!(h.pick(), "Thunderbolt");
    assert!(h.player_metronome().max_damage > 0);
}

#[test]
fn arrow_keys_keep_the_highlighted_row_in_view() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    let options = h.player_metronome().metronome_options.expect("the selector's rows");
    h.click(h.entry.center());
    // well past the ten rows the list shows at once
    let steps = 30;
    for _ in 0..steps {
        h.press(Key::ArrowDown);
    }
    for _ in 0..20 {
        h.frame(vec![]); // let the scroll animation settle
    }
    let target = &options[steps];
    assert!(h.row(target).is_some(), "the highlighted row ({}) is in view", target);
    h.press(Key::Enter);
    assert_eq!(&h.pick(), target);
}

