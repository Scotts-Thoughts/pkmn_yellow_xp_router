//! The battle page's "HP cur / max" chip, through a headless `egui::Context`:
//! clicking the current HP types a new one, clicking the max opens the
//! "% of max" popup, and the value reaches the battle summary (and its
//! trainer definition, which is what gets saved). With Blaze, a set HP at
//! or under a third boosts the Fire moves and drops their pinch option.

use std::path::PathBuf;
use std::sync::Arc;

use egui::{epaint, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2};

use xpr_app::assets::Assets;
use xpr_app::battle::BattleController;
use xpr_app::battle_ui::{BattleSummaryUi, BattleUiActions};
use xpr_app::controller::MainController;
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
    bc: BattleController,
    page: BattleSummaryUi,
    assets: Assets,
    /// (text, rect) of every text drawn last frame
    texts: Vec<(String, Rect)>,
}

fn collect(shape: &epaint::Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        epaint::Shape::Text(t) => out.push((t.galley.text().to_string(), Rect::from_min_size(t.pos, t.galley.size()))),
        epaint::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
        _ => {}
    }
}

impl Harness {
    fn new(route: &str, fight_idx: usize) -> (Harness, NodeId) {
        let root = repo_root();
        let cfg = Config::load(&xpr_app::scratch_config_path());
        let ctx = egui::Context::default();
        let mut theme = Theme::from_config(&cfg);
        theme.install_fonts(&ctx);
        theme.apply(&ctx);
        let registry = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
        let mut ctrl = MainController::new(registry, Paths::new(root.clone()));
        ctrl.load_route(&root.join("tests/test_data").join(route));
        let fight = ctrl.router.all_groups().into_iter().filter(|g| ctrl.router.group(*g).unwrap().event_definition.trainer_def.is_some()).nth(fight_idx).expect("trainer fight");
        ctrl.select_new_events(vec![fight]);
        let _ = ctrl.take_signals();
        let mut bc = BattleController::new();
        bc.load_from_event(&cfg, &mut ctrl, fight);
        let mut page = BattleSummaryUi::new();
        page.show_contents();
        let mut h = Harness { ctx, theme, cfg, ctrl, bc, page, assets: Assets::new(), texts: Vec::new() };
        h.frame(vec![]);
        h.frame(vec![]);
        (h, fight)
    }

    fn frame(&mut self, events: Vec<Event>) {
        let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1800.0, 900.0))), events, ..Default::default() };
        let Harness { ctx, theme, cfg, ctrl, bc, page, assets, .. } = self;
        let mut out = ctx.run_ui(input, |root| {
            egui::CentralPanel::default().show(root, |ui| {
                let mut actions = BattleUiActions::default();
                page.ui(ui, theme, cfg, bc, ctrl, assets, &mut actions);
            });
        });
        let shapes = std::mem::take(&mut out.shapes);
        out.drop_without_applying_deltas();
        self.texts.clear();
        for s in &shapes {
            collect(&s.shape, &mut self.texts);
        }
    }

    fn text_rect(&self, text: &str) -> Rect {
        self.texts.iter().find(|(t, _)| t == text).map(|(_, r)| *r).unwrap_or_else(|| panic!("{text:?} is drawn; drawn: {:?}", self.texts.iter().map(|t| &t.0).collect::<Vec<_>>()))
    }

    fn click(&mut self, pos: Pos2) {
        let button = |pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        self.frame(vec![Event::PointerMoved(pos), button(true)]);
        self.frame(vec![button(false)]);
        self.frame(vec![]);
    }

    fn key(&mut self, key: Key) {
        self.frame(vec![Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
    }

    /// Replace the focused entry's text and press Enter.
    fn type_and_enter(&mut self, text: &str) {
        for _ in 0..6 {
            self.key(Key::Backspace);
        }
        self.frame(vec![Event::Text(text.to_string())]);
        self.key(Key::Enter);
        self.frame(vec![]);
    }

    fn max(&self) -> i64 {
        self.bc.summary.get_player_battle_hp()
    }
}

#[test]
fn typing_the_current_hp_and_the_percent_popup_set_the_calc_hp() {
    let (mut h, _) = Harness::new("platinum_chimchar.json", 1);
    let max = h.max();
    assert!(max > 3);
    assert_eq!(h.bc.summary.player_cur_hp, None);
    let denom = format!("/ {max}");
    h.text_rect(&denom);

    // click the current HP, type 1/3 of max
    let cur = h.text_rect(&max.to_string());
    h.click(cur.center());
    h.type_and_enter(&(max / 3).to_string());
    assert_eq!(h.bc.summary.player_cur_hp, Some(max / 3));
    assert_eq!(h.bc.summary.get_partial_trainer_definition().unwrap().player_cur_hp, Some(max / 3), "it is saved with the fight");
    h.text_rect(&(max / 3).to_string());

    // Escape leaves it as it was
    let cur = h.text_rect(&(max / 3).to_string());
    h.click(cur.center());
    for _ in 0..6 {
        h.key(Key::Backspace);
    }
    h.frame(vec![Event::Text("5".into())]);
    h.key(Key::Escape);
    h.frame(vec![]);
    assert_eq!(h.bc.summary.player_cur_hp, Some(max / 3));

    // the max opens the % popup; 50 % is rounded down
    h.click(h.text_rect(&denom).center());
    h.text_rect("Set HP to a % of max HP");
    h.type_and_enter("50");
    assert_eq!(h.bc.summary.player_cur_hp, Some(max / 2));
    assert!(!h.texts.iter().any(|(t, _)| t == "Set HP to a % of max HP"), "Enter closes the popup");

    // "Full" clears it (the HP label opens the popup too)
    let d = h.text_rect(&denom);
    let label = h.texts.iter().filter(|(t, r)| t == "HP" && r.max.x <= d.min.x && (r.center().y - d.center().y).abs() < 2.0).map(|(_, r)| *r).max_by(|a, b| a.min.x.total_cmp(&b.min.x)).expect("the chip's HP label");
    h.click(label.center());
    h.click(h.text_rect("Full").center());
    assert_eq!(h.bc.summary.player_cur_hp, None);
    assert_eq!(h.bc.summary.get_partial_trainer_definition().unwrap().player_cur_hp, None);
    h.text_rect(&max.to_string());
}

#[test]
fn a_set_hp_turns_on_blaze_for_the_players_fire_moves() {
    let (mut h, _) = Harness::new("platinum_chimchar.json", 1);
    let player = h.bc.summary.original_player_mon_list[0].clone();
    assert_eq!(player.ability, "Blaze", "the test route's solo mon has Blaze");
    let fire_slot = |h: &Harness| -> Option<(usize, i64, Option<Vec<String>>)> {
        let row = h.bc.summary.player_move_data.first()?;
        row.iter().enumerate().find_map(|(i, m)| {
            let m = m.as_ref()?;
            let opts = m.custom_data_options.clone();
            opts.as_ref().filter(|o| o.iter().any(|x| x.contains("Blaze"))).map(|_| (i, m.max_damage, opts.clone()))
        })
    };
    let (slot, plain_max, _) = fire_slot(&h).expect("a Fire move with the Blaze option");
    let max = h.max();
    let cfg = h.cfg.clone();
    h.bc.update_player_cur_hp(&cfg, &h.ctrl, Some(max / 3));
    let m = h.bc.summary.player_move_data[0][slot].as_ref().unwrap();
    assert!(m.max_damage > plain_max, "Blaze applies at a third: {} -> {}", plain_max, m.max_damage);
    assert!(m.custom_data_options.as_ref().is_none_or(|o| !o.iter().any(|x| x.contains("Blaze"))), "the pinch option goes away");
    h.bc.update_player_cur_hp(&cfg, &h.ctrl, Some(max / 3 + 1));
    assert_eq!(h.bc.summary.player_move_data[0][slot].as_ref().unwrap().max_damage, plain_max, "and not above it");
}
