//! WP-E: finding trainers/items from the search box (G10), keyboard marker
//! navigation and follow-selection (G11/G12), and the route-path overlay
//! (G9; `docs/rust_port/design/world_map/SPEC.md` D15 / §13 "Trip path
//! overlay" — one folder ("trip") at a time, never the whole route).
//! Harness copied from `map_view.rs` / `map_modal_input.rs`, with the
//! `encounter_card.rs` shape-collecting trick added so a test can see the
//! path overlay's discs and lines, not just call its Rust API.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape, Vec2};

use xpr_app::assets::Assets;
use xpr_app::controller::MainController;
use xpr_app::map::finder::{find_items, find_trainers};
use xpr_app::map::item_search;
use xpr_app::map::{MapAction, MapView};
use xpr_core::{Config, Paths};
use xpr_data::Registry;
use xpr_engine::{EventDefinition, NodeId, TrainerEventDefinition, WildPkmnEventDefinition};
use xpr_map::Scope;
use xpr_ui_kit::theme::Theme;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

struct Harness {
    ctx: egui::Context,
    theme: Theme,
    cfg: Config,
    ctrl: MainController,
    assets: Assets,
    view: MapView,
    size: Vec2,
    /// every text drawn in the last frame
    texts: Vec<String>,
    /// text drawn in the last frame, with its screen position (`TextShape::pos`,
    /// the top-left of the glyphs — see `Painter::text`), so a test can click
    /// a popup row or a path disc it found by its label.
    text_positions: Vec<(String, Pos2)>,
    /// `Shape::Circle` count in the last frame
    circles: usize,
    /// `Shape::LineSegment` / `Shape::Path` count in the last frame
    line_likes: usize,
}

fn collect_shapes(shape: &Shape, texts: &mut Vec<String>, positions: &mut Vec<(String, Pos2)>, circles: &mut usize, line_likes: &mut usize) {
    match shape {
        Shape::Text(t) => {
            let s = t.galley.text().to_string();
            positions.push((s.clone(), t.pos));
            texts.push(s);
        }
        Shape::Circle(_) => *circles += 1,
        Shape::LineSegment { .. } | Shape::Path(_) => *line_likes += 1,
        Shape::Vec(v) => v.iter().for_each(|s| collect_shapes(s, texts, positions, circles, line_likes)),
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
        Harness { ctx, theme, cfg, ctrl, assets: Assets::new(), view, size: Vec2::new(1000.0, 700.0), texts: Vec::new(), text_positions: Vec::new(), circles: 0, line_likes: 0 }
    }

    fn frame(&mut self, events: Vec<Event>) -> Vec<MapAction> {
        let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)), events, ..Default::default() };
        let mut actions = Vec::new();
        let Harness { ctx, theme, cfg, ctrl, assets, view, texts, text_positions, circles, line_likes, .. } = self;
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                actions = view.ui(ui, theme, cfg, ctrl, assets);
            });
        });
        texts.clear();
        text_positions.clear();
        *circles = 0;
        *line_likes = 0;
        for cs in &out.shapes {
            collect_shapes(&cs.shape, texts, text_positions, circles, line_likes);
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

    fn click(&mut self, pos: Pos2) -> Vec<MapAction> {
        let button = |pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        self.frame(vec![Event::PointerMoved(pos)]);
        self.frame(vec![button(true)]);
        let mut a = self.frame(vec![button(false)]);
        a.extend(self.frame(vec![]));
        a
    }

    fn press(&mut self, key: Key) -> Vec<MapAction> {
        self.press_with(key, Modifiers::NONE)
    }

    fn press_with(&mut self, key: Key, modifiers: Modifiers) -> Vec<MapAction> {
        let mut a = self.frame(vec![Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }]);
        a.extend(self.frame(vec![Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers }]));
        a
    }
}

/// The `n`-th trainer-fight group in route order (0-based), as in
/// `tests/map_actions.rs`.
fn nth_trainer(ctrl: &MainController, n: usize) -> NodeId {
    ctrl.router.all_groups().into_iter().filter(|g| ctrl.router.group(*g).unwrap().event_definition.trainer_def.is_some()).nth(n).expect("trainer fight")
}

fn trainer_def(name: &str) -> EventDefinition {
    EventDefinition::with_trainer(TrainerEventDefinition::new(name))
}

fn wild_def(species: &str, level: i64) -> EventDefinition {
    EventDefinition::with_wild(WildPkmnEventDefinition::new(species, level, 1, false))
}

/// Build a fresh folder named `folder`, placed after `after`, holding one
/// event per definition in `defs`, in order (mirrors
/// `MainController::add_trainers_in_new_folder`, generalised to any
/// definition so a test route path trip can be built directly). Returns
/// the folder's id and the events' ids, insertion order preserved.
fn build_folder(ctrl: &mut MainController, folder: &str, after: NodeId, defs: Vec<EventDefinition>) -> (NodeId, Vec<NodeId>) {
    ctrl.finalize_new_folder(folder, None, Some(after));
    let mut ids = Vec::new();
    for def in defs {
        let id = ctrl.new_event(def, None, None, Some(folder), false).expect("event added to the new folder");
        ids.push(id);
    }
    let folder_id = ctrl.router.parent_of(*ids.last().expect("at least one definition")).expect("the new event has a parent folder");
    (folder_id, ids)
}

// ---------------------------------------------------------------------------
// Finder (G10)
// ---------------------------------------------------------------------------

#[test]
fn find_trainers_and_items_match_by_substring_and_carry_anchors() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    let pack = h.view.pack().unwrap().clone();

    let trainer_hits = find_trainers(&pack, &h.ctrl, "youngster");
    let hit = trainer_hits.iter().find(|h| h.label == "Youngster 1").expect("Youngster 1 among the hits");
    assert!(!hit.anchors.is_empty(), "Youngster 1 has at least one anchor");
    assert!(trainer_hits.len() <= 25, "capped at 25");

    // "Potion", "Super Potion", "Max Potion", "Hyper Potion" are all linked
    // in map_data/yellow/links.json's `items` table (checked directly).
    let item_hits = find_items(&pack, &h.ctrl, "potion");
    assert!(!item_hits.is_empty(), "the potion search returns hits: {:?}", item_hits.iter().map(|h| &h.label).collect::<Vec<_>>());
    assert!(item_hits.iter().any(|h| h.label == "Potion"));
    assert!(item_hits.iter().all(|h| !h.anchors.is_empty()), "every hit carries at least one anchor");

    // an empty or too-short query finds nothing (the popup only shows these
    // groups once the query has >= 2 characters)
    assert!(find_trainers(&pack, &h.ctrl, "").is_empty());
    assert!(find_items(&pack, &h.ctrl, "").is_empty());
}

#[test]
fn the_search_popup_lists_a_trainers_group_and_choosing_the_hit_focuses_it() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();

    h.view.list.search = "brock".to_string();
    h.view.list.open = true;
    // a freshly-opened popup's content needs a second frame before it shows
    // up in `ctx.run()`'s output (egui settles a new floating Area's size
    // over two passes; the first paints into a placeholder), matching the
    // "click, then one more frame" pattern the rest of this harness uses.
    h.frame(vec![]);
    h.frame(vec![]);
    assert!(h.texts.iter().any(|t| t == "Trainers"), "a Trainers group heading is drawn: {:?}", h.texts);
    let (_, row_pos) = h.text_positions.iter().find(|(t, _)| t == "Brock 1").cloned().expect("a \"Brock 1\" row is drawn in the popup");

    // click inside the row (`Painter::text`'s `pos` is the glyphs' top-left
    // for a left-aligned anchor; nudge into the 20 px row band under it)
    let click_pos = row_pos + Vec2::new(15.0, 8.0);
    h.click(click_pos);
    assert_eq!(h.view.focus_label(), Some("Brock 1"), "choosing the hit focuses it through the banner");
    h.settle();
    let gym = h.view.pack().unwrap().map_by_const("PEWTER_GYM").unwrap().id;
    assert_eq!(h.view.scope(), Scope::Map(gym), "Brock's gym, an indoor map, switched the scope");
}

#[test]
fn item_search_lists_balls_and_hidden_items_and_loops_through_them() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    let pack = h.view.pack().unwrap().clone();

    // Yellow's Nuggets: 3 item balls and 5 hidden (map_data/yellow/objects.json)
    let hits = item_search::find(&pack, "nugg");
    assert_eq!(hits, vec![item_search::ItemHit { name: "Nugget".into(), balls: 3, hidden: 5 }]);
    assert_eq!(item_search::instances(&pack, "Nugget").len(), 8);
    assert!(item_search::find(&pack, "").len() > 20, "an empty query lists every item on the map");

    h.view.item_search.query = "nugget".to_string();
    h.view.item_search.open = true;
    h.frame(vec![]);
    h.frame(vec![]);
    let (_, row_pos) = h.text_positions.iter().find(|(t, _)| t == "Nugget").cloned().expect("a \"Nugget\" row is drawn in the popup");
    assert!(h.texts.iter().any(|t| t == "3 balls · 5 hidden"), "the row shows its counts: {:?}", h.texts);
    h.click(row_pos + Vec2::new(15.0, 8.0));
    assert_eq!(h.view.focus_label(), Some("Nugget"));
    assert!(!h.view.item_search.open, "choosing closes the popup");
    h.settle();
    assert!(h.texts.iter().any(|t| t.contains("(1 of 8)")), "the banner counts the instances: {:?}", h.texts);

    // the banner loops through all eight, naming each one's kind, and wraps
    let mut kinds = (0, 0);
    for _ in 0..8 {
        let line = h.texts.iter().find(|t| t.contains(" of 8)")).cloned().unwrap();
        if line.contains("· item ball") {
            kinds.0 += 1;
        } else if line.contains("· hidden item") {
            kinds.1 += 1;
        }
        h.view.step_focus(true);
        h.settle();
    }
    assert_eq!(kinds, (3, 5));
    assert!(h.texts.iter().any(|t| t.contains("(1 of 8)")), "wrapped back to the first");
}

#[test]
fn the_map_card_lists_the_maps_trainers_and_their_teams() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    let pack = h.view.pack().unwrap().clone();
    let route3 = pack.map_by_const("ROUTE_3").unwrap().id;
    h.view.navigate_to(route3, false);
    h.settle();
    assert!(h.view.open_card_at_step(route3, 0, 0));
    h.frame(vec![]);
    h.frame(vec![]);
    assert!(h.view.card_is_map());
    // each trainer gets a row with its prize money, total exp and party levels (the
    // last rows are scrolled out of this 700px window)
    for name in ["BugCatcher 4", "Youngster 1", "Lass 1", "BugCatcher 5"] {
        assert!(h.texts.iter().any(|t| t == name), "{} has a row: {:?}", name, h.texts);
    }
    assert!(h.texts.iter().any(|t| t == "¥165 · 278 exp"), "Youngster 1's prize money and exp: {:?}", h.texts);
    assert!(h.texts.iter().any(|t| t == "14"), "a party level: {:?}", h.texts);
    assert!(h.texts.iter().any(|t| t == "Stat exp: 130 HP, 95 Atk, 100 Def, 60 Spc, 140 Spe"), "BugCatcher 4's stat exp: {:?}", h.texts);

    // "+" on a row adds that trainer
    let (_, bug) = h.text_positions.iter().find(|(t, _)| t == "BugCatcher 4").cloned().unwrap();
    let (_, plus) = h.text_positions.iter().filter(|(t, p)| t == "+" && p.x > bug.x && (p.y - bug.y).abs() < 40.0).min_by(|a, b| (a.1.y - bug.y).abs().total_cmp(&(b.1.y - bug.y).abs())).cloned().expect("BugCatcher 4's add button");
    let actions = h.click(plus + Vec2::new(3.0, 5.0));
    assert!(actions.iter().any(|a| matches!(a, MapAction::AddTrainer { name } if name == "BugCatcher 4")), "{:?}", actions);
    assert!(h.view.card_is_map(), "adding keeps the map card open");

    // clicking a row opens that trainer's own card
    let (_, lass) = h.text_positions.iter().find(|(t, _)| t == "Lass 1").cloned().unwrap();
    h.click(lass + Vec2::new(10.0, 5.0));
    let lass_obj = pack.object_range(route3).find(|&i| pack.objects[i as usize].trainer_names().iter().any(|n| n == "Lass 1")).unwrap();
    assert_eq!(h.view.card_object(), Some(lass_obj));
}

// ---------------------------------------------------------------------------
// Keyboard marker navigation (G11)
// ---------------------------------------------------------------------------

#[test]
fn tab_cycles_the_selection_enter_opens_its_card_and_shift_tab_reverses() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    // `navigate_to` fits the whole gym in view (unlike `request_focus`,
    // which zooms in tight on one trainer) so every one of its markers —
    // Brock plus at least the exit warp — is inside `cam.visible_world(vp)`.
    let gym = h.view.pack().unwrap().map_by_const("PEWTER_GYM").unwrap().id;
    h.view.navigate_to(gym, false);
    h.frame(vec![]);
    h.frame(vec![]);
    assert_eq!(h.view.scope(), Scope::Map(gym));
    // `keynav::handle_keys` only runs while the viewport is hovered
    // (`mod.rs::viewport`'s `if resp.hovered() { ... }`), like every other
    // map shortcut key; put the pointer over the map first.
    h.frame(vec![Event::PointerMoved(Pos2::new(500.0, 350.0))]);

    h.press(Key::Tab);
    let first = h.view.selected_object().expect("Tab selects a visible, toggled-on object");

    h.press(Key::Enter);
    assert_eq!(h.view.card_object(), Some(first), "Enter opens the selected object's card");

    h.press(Key::Tab);
    let second = h.view.selected_object().expect("Tab moves to the next object");
    assert_ne!(first, second, "the gym has more than one visible marker to Tab through (an object and at least a warp)");

    h.press_with(Key::Tab, Modifiers::SHIFT);
    assert_eq!(h.view.selected_object(), Some(first), "Shift+Tab moves back to the previous object");
}

// ---------------------------------------------------------------------------
// Follow selection (G12)
// ---------------------------------------------------------------------------

#[test]
fn follow_selection_switches_scope_and_centres_without_a_focus_banner() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    h.view.toggles.follow = true;

    // the test route's title fight, already in the file (`tests/test_data/
    // yellow-pinsir-lv10brock.json`, "Fight Trainer": {"trainer_name": "Brock 1"})
    let brock_id = h.ctrl.router.all_groups().into_iter().find(|g| h.ctrl.router.group(*g).and_then(|grp| grp.event_definition.trainer_def.as_ref()).map(|t| t.trainer_name == "Brock 1").unwrap_or(false)).expect("Brock 1 is in the test route");
    h.ctrl.select_new_events(vec![brock_id]);
    h.view.sync_route(&h.ctrl);
    h.frame(vec![]);

    let gym = h.view.pack().unwrap().map_by_const("PEWTER_GYM").unwrap().id;
    assert_eq!(h.view.scope(), Scope::Map(gym), "follow switched the scope to Brock's gym");
    assert!(h.view.focus_label().is_none(), "follow does not raise a focus banner");
}

// ---------------------------------------------------------------------------
// Route path overlay (G9): one folder ("trip") at a time (SPEC D15 / §13)
// ---------------------------------------------------------------------------

#[test]
fn a_folder_of_linked_trainers_gives_nodes_in_route_order() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    let anchor = nth_trainer(&h.ctrl, 0);
    let (folder_id, ids) = build_folder(&mut h.ctrl, "Route 3 Trip", anchor, vec![trainer_def("BugCatcher 4"), trainer_def("Youngster 1"), trainer_def("Lass 1")]);
    h.ctrl.select_new_events(vec![folder_id]);
    h.view.sync_route(&h.ctrl);

    let trip = h.view.state.trip.clone().expect("the folder has a trip");
    assert_eq!(trip.folder, folder_id);
    assert_eq!(trip.nodes.len(), 3);
    for (i, node) in trip.nodes.iter().enumerate() {
        assert_eq!(node.ids, vec![ids[i]], "node {} is event {}", i, i);
        assert_eq!((node.first, node.last), (i + 1, i + 1));
    }
}

#[test]
fn a_disabled_event_is_skipped_and_the_rest_renumber() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    let anchor = nth_trainer(&h.ctrl, 0);
    let (folder_id, ids) = build_folder(&mut h.ctrl, "Route 3 Disabled", anchor, vec![trainer_def("BugCatcher 4"), trainer_def("Youngster 1"), trainer_def("Lass 1")]);
    h.ctrl.set_events_enabled(&[ids[1]], false);
    h.ctrl.select_new_events(vec![folder_id]);
    h.view.sync_route(&h.ctrl);

    let trip = h.view.state.trip.clone().expect("the folder still has a trip");
    assert_eq!(trip.nodes.len(), 2, "the disabled middle fight is skipped, not just hidden");
    assert_eq!(trip.nodes[0].ids, vec![ids[0]]);
    assert_eq!(trip.nodes[1].ids, vec![ids[2]]);
    assert_eq!((trip.nodes[0].first, trip.nodes[0].last), (1, 1));
    assert_eq!((trip.nodes[1].first, trip.nodes[1].last), (2, 2), "the survivors renumber, they do not keep gap 3");
}

#[test]
fn consecutive_events_on_the_same_step_collapse_into_one_node() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    let anchor = nth_trainer(&h.ctrl, 0);
    // two fights against the same trainer resolve to the same first anchor
    let (folder_id, ids) = build_folder(&mut h.ctrl, "Route 3 Collapse", anchor, vec![trainer_def("Youngster 1"), trainer_def("Youngster 1"), trainer_def("Lass 1")]);
    h.ctrl.select_new_events(vec![folder_id]);
    h.view.sync_route(&h.ctrl);

    let trip = h.view.state.trip.clone().expect("the folder has a trip");
    assert_eq!(trip.nodes.len(), 2, "the two Youngster 1 fights collapse into one disc");
    assert_eq!(trip.nodes[0].ids, vec![ids[0], ids[1]]);
    assert_eq!((trip.nodes[0].first, trip.nodes[0].last), (1, 2), "labelled as a range (\"1–2\")");
    assert_eq!(trip.nodes[1].ids, vec![ids[2]]);
    assert_eq!((trip.nodes[1].first, trip.nodes[1].last), (3, 3));
}

#[test]
fn a_folder_without_anchored_events_gives_none() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    let anchor = nth_trainer(&h.ctrl, 0);
    // a wild encounter resolves to map-level anchors only, so it never
    // contributes a node (SPEC §13.2 "Anchors of a trip")
    let (folder_id, _ids) = build_folder(&mut h.ctrl, "Wild Only", anchor, vec![wild_def("Pidgey", 3)]);
    h.ctrl.select_new_events(vec![folder_id]);
    h.view.sync_route(&h.ctrl);

    assert!(h.view.state.trip.is_none());
    assert_eq!(h.view.state.trip_folder_name.as_deref(), Some("Wild Only"), "the folder was resolved, it is just empty");
}

#[test]
fn an_event_directly_under_the_root_has_no_trip() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    let id = h.ctrl.new_event(trainer_def("Youngster 1"), None, None, None, true).expect("a root-level event was added");
    assert_eq!(h.ctrl.router.parent_of(id), Some(h.ctrl.router.root_id), "sanity: it landed directly under the root");
    h.ctrl.select_new_events(vec![id]);
    h.view.sync_route(&h.ctrl);

    assert!(h.view.state.trip.is_none());
    assert!(h.view.state.trip_folder_name.is_none(), "the root is not a folder to show a path for");
}

#[test]
fn clicking_a_disc_selects_its_event() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    let pack = h.view.pack().unwrap().clone();
    let route3 = pack.map_by_const("ROUTE_3").unwrap().id;
    h.view.navigate_to(route3, false);
    h.frame(vec![]);
    h.frame(vec![]);
    // Route 3 (1,120 world px wide) is wider than the fixed test viewport at
    // the initial zoom (2x, inherited from the "first town" fit): zoom out
    // so both trainers used below land on screen, the same wheel path
    // `map_view.rs`'s zoom test drives.
    let center = Pos2::new(500.0, 350.0);
    h.frame(vec![Event::PointerMoved(center)]);
    for _ in 0..4 {
        h.frame(vec![Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: Vec2::new(0.0, -80.0), modifiers: Modifiers::NONE }]);
    }

    let anchor = nth_trainer(&h.ctrl, 0);
    let (folder_id, ids) = build_folder(&mut h.ctrl, "Route 3 Click", anchor, vec![trainer_def("BugCatcher 4"), trainer_def("Youngster 1")]);
    h.ctrl.select_new_events(vec![folder_id]);
    h.view.toggles.path = true;
    h.view.sync_route(&h.ctrl);
    h.frame(vec![]);

    let trip = h.view.state.trip.clone().expect("a trip");
    let node = &trip.nodes[1];
    let pos = h.view.step_screen_pos(node.anchor.map, node.anchor.x as i32, node.anchor.y as i32).expect("the second disc's step resolves on Route 3");
    assert!(pos.x >= 0.0 && pos.x <= 1000.0 && pos.y >= 30.0 && pos.y <= 682.0, "the disc is on screen: {:?}", pos);
    let actions = h.click(pos);
    assert!(actions.contains(&MapAction::SelectEvent(ids[1])), "clicking the disc selects its event: {:?}", actions);
}

#[test]
fn the_path_overlay_draws_a_numbered_disc_in_a_map_scope_and_a_line_in_the_world() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    let pack = h.view.pack().unwrap().clone();
    // both captured before any route mutation, so `all_groups()` ordering
    // cannot shift under the second one because of the first folder's insert
    let anchor0 = nth_trainer(&h.ctrl, 0);
    let anchor1 = nth_trainer(&h.ctrl, 1);

    // map scope (Brock's gym): one node, one disc labelled "1"
    let gym = pack.map_by_const("PEWTER_GYM").unwrap().id;
    let (folder1, _ids1) = build_folder(&mut h.ctrl, "Brock Trip", anchor0, vec![trainer_def("Brock 1")]);
    h.ctrl.select_new_events(vec![folder1]);
    h.view.sync_route(&h.ctrl);
    h.view.navigate_to(gym, false);
    h.frame(vec![]);
    h.view.toggles.path = false;
    h.frame(vec![]);
    let (circles_off, ones_off) = (h.circles, h.texts.iter().filter(|t| t.as_str() == "1").count());
    h.view.toggles.path = true;
    h.frame(vec![]);
    assert!(h.circles > circles_off, "the path disc adds a circle ({} -> {})", circles_off, h.circles);
    assert!(h.texts.iter().filter(|t| t.as_str() == "1").count() > ones_off, "the disc is labelled \"1\": {:?}", h.texts);

    // world scope: a two-node trip on Route 3 draws a connecting line
    let route3 = pack.map_by_const("ROUTE_3").unwrap().id;
    let (folder2, _ids2) = build_folder(&mut h.ctrl, "Route 3 Lines", anchor1, vec![trainer_def("BugCatcher 4"), trainer_def("Youngster 1")]);
    h.ctrl.select_new_events(vec![folder2]);
    h.view.sync_route(&h.ctrl);
    h.view.navigate_to(route3, false);
    h.frame(vec![]);
    h.view.toggles.path = false;
    h.frame(vec![]);
    let lines_off = h.line_likes;
    h.view.toggles.path = true;
    h.frame(vec![]);
    assert!(h.line_likes > lines_off, "the path overlay draws a connecting line in the world scope ({} -> {})", lines_off, h.line_likes);
}
