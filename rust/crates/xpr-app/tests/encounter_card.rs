//! Encounter tables on the world map: the EV-yield column exists for gens
//! 3+ and reads straight from the router's own species data, the card that
//! opens on a gen 3 grass tile shows it, and a gen 1 card does not.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape, Vec2};

use xpr_app::assets::Assets;
use xpr_app::controller::MainController;
use xpr_app::map::cards::{ev_yield_text, has_ev_column};
use xpr_app::map::{MapAction, MapView};
use xpr_core::{Config, Paths};
use xpr_data::Registry;
use xpr_map::MapPack;
use xpr_ui_kit::theme::Theme;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn registry() -> Registry {
    Registry::new(repo_root().join("raw_pkmn_data"), PathBuf::new())
}

#[test]
fn ev_yields_come_from_the_species_data_in_gens_3_and_up() {
    let reg = registry();
    let emerald = reg.get_version("Emerald").unwrap();
    assert!(has_ev_column(Some(&emerald)));
    assert_eq!(ev_yield_text(&emerald, "Zigzagoon").as_deref(), Some("1 Spe"));
    assert_eq!(ev_yield_text(&emerald, "Wurmple").as_deref(), Some("1 HP"));
    assert_eq!(ev_yield_text(&emerald, "Poochyena").as_deref(), Some("1 Atk"));
    assert_eq!(ev_yield_text(&emerald, "Ralts").as_deref(), Some("1 SpA"));
    assert_eq!(ev_yield_text(&emerald, "Lotad").as_deref(), Some("1 SpD"));
    assert_eq!(ev_yield_text(&emerald, "Chansey").as_deref(), Some("2 HP"));
    // several stats are listed in HP, Atk, Def, SpA, SpD, Spe order
    assert_eq!(ev_yield_text(&emerald, "Wartortle").as_deref(), Some("1 Def, 1 SpD"));
    assert_eq!(ev_yield_text(&emerald, "Venusaur").as_deref(), Some("2 SpA, 1 SpD"));
    assert_eq!(ev_yield_text(&emerald, "Missingno"), None, "unknown species");

    let firered = reg.get_version("FireRed").unwrap();
    assert_eq!(ev_yield_text(&firered, "Pidgey").as_deref(), Some("1 Spe"));
    assert_eq!(ev_yield_text(&firered, "Rattata").as_deref(), Some("1 Spe"));

    // gens 1/2 have stat exp, not EV yields: no column
    let yellow = reg.get_version("Yellow").unwrap();
    assert!(!has_ev_column(Some(&yellow)));
    assert_eq!(ev_yield_text(&yellow, "Pidgey"), None);
    let crystal = reg.get_version("Crystal").unwrap();
    assert!(!has_ev_column(Some(&crystal)));
    assert_eq!(ev_yield_text(&crystal, "Pidgey"), None);
    assert!(!has_ev_column(None));

    // gen 4 carries EV yields too, so the column is ready for its maps
    let platinum = reg.get_version("Platinum").unwrap();
    assert!(has_ev_column(Some(&platinum)));
    assert_eq!(ev_yield_text(&platinum, "Chimchar").as_deref(), Some("1 Spe"));
}

// ---------------------------------------------------------------------------
// Headless map view: click a grass tile and read the card's text
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
}

fn collect_text(shape: &Shape, out: &mut Vec<String>) {
    match shape {
        Shape::Text(t) => out.push(t.galley.text().to_string()),
        Shape::Vec(v) => v.iter().for_each(|s| collect_text(s, out)),
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
        Harness { ctx, theme, cfg, ctrl, assets: Assets::new(), view, size: Vec2::new(1000.0, 700.0), texts: Vec::new() }
    }

    fn frame(&mut self, events: Vec<Event>) -> Vec<MapAction> {
        let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)), events, ..Default::default() };
        let mut actions = Vec::new();
        let Harness { ctx, theme, cfg, ctrl, assets, view, texts, .. } = self;
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                actions = view.ui(ui, theme, cfg, ctrl, assets);
            });
        });
        texts.clear();
        for cs in &out.shapes {
            collect_text(&cs.shape, texts);
        }
        actions
    }

    fn wait_ready(&mut self) {
        let t = Instant::now();
        while !self.view.is_ready() {
            assert!(t.elapsed() < Duration::from_secs(20), "map pack did not load");
            self.frame(vec![]);
            std::thread::sleep(Duration::from_millis(20));
        }
        self.frame(vec![]);
    }

    fn click(&mut self, pos: Pos2) -> Vec<MapAction> {
        let button = |pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        self.frame(vec![Event::PointerMoved(pos)]);
        self.frame(vec![button(true)]);
        let mut a = self.frame(vec![button(false)]);
        a.extend(self.frame(vec![]));
        a.extend(self.frame(vec![]));
        a
    }

    /// Navigate to `map_const` and return the screen position of a tall-grass
    /// step of it that is on screen and clear of every object marker.
    fn grass_step(&mut self, pack: &MapPack, map_const: &str) -> Pos2 {
        let map = pack.map_by_const(map_const).unwrap_or_else(|| panic!("{} in the pack", map_const));
        let (id, w, h) = (map.id, map.w as i32, map.h as i32);
        self.view.navigate_to(id, false);
        self.frame(vec![]);
        self.frame(vec![]);
        let objects: Vec<Pos2> = pack.object_range(id).filter_map(|i| self.view.object_screen_pos(i)).collect();
        let spb = pack.geom.steps_per_block() as i32;
        for by in 0..h {
            for bx in 0..w {
                let (grass, _water) = pack.terrain_at(id, bx as u32, by as u32);
                if !grass {
                    continue;
                }
                let Some(p) = self.view.step_screen_pos(id, bx * spb, by * spb) else { continue };
                let on_screen = p.x > 40.0 && p.x < 960.0 && p.y > 80.0 && p.y < 640.0;
                let clear = objects.iter().all(|o| (o.x - p.x).abs() > 48.0 || (o.y - p.y).abs() > 48.0);
                if on_screen && clear {
                    return p;
                }
            }
        }
        panic!("no free grass step of {} on screen", map_const);
    }
}

#[test]
fn a_gen_3_grass_tile_card_lists_the_ev_yield_of_every_slot() {
    let mut h = Harness::new("f-ditto-tests.json");
    h.frame(vec![]);
    h.wait_ready();
    let pack = h.view.pack().unwrap().clone();
    assert_eq!(pack.game, "firered_leafgreen");
    let grass = h.grass_step(&pack, "MAP_ROUTE1");
    let actions = h.click(grass);
    assert!(actions.is_empty(), "{:?}", actions);
    assert!(h.view.card_is_tile(), "the grass click opened the tile card");
    let texts = h.texts.clone();
    assert!(texts.iter().any(|t| t.starts_with("Tall grass — Route 1")), "card title: {:?}", texts);
    for head in ["Pokémon", "Level", "Rate", "EV yield"] {
        assert!(texts.iter().any(|t| t == head), "column head {:?} drawn: {:?}", head, texts);
    }
    assert!(texts.iter().any(|t| t == "WALK  ·  rate 21"), "the walk method heading: {:?}", texts);
    // Route 1 is Pidgey and Rattata, both 1 Speed EV
    let ev_cells = texts.iter().filter(|t| t.as_str() == "1 Spe").count();
    assert!(ev_cells >= 2, "one EV cell per slot ({} found): {:?}", ev_cells, texts);
    assert!(texts.iter().any(|t| t == "35%"), "slot rates: {:?}", texts);
}

#[test]
fn a_gen_1_grass_tile_card_has_no_ev_column() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    let pack = h.view.pack().unwrap().clone();
    assert_eq!(pack.game, "yellow");
    let grass = h.grass_step(&pack, "ROUTE_1");
    h.click(grass);
    assert!(h.view.card_is_tile(), "the grass click opened the tile card");
    let texts = h.texts.clone();
    assert!(texts.iter().any(|t| t.starts_with("Tall grass — Route 1")), "card title: {:?}", texts);
    for head in ["Pokémon", "Level", "Rate"] {
        assert!(texts.iter().any(|t| t == head), "column head {:?} drawn: {:?}", head, texts);
    }
    assert!(!texts.iter().any(|t| t == "EV yield"), "no EV column in gen 1: {:?}", texts);
    assert!(texts.iter().any(|t| t == "Pidgey") && texts.iter().any(|t| t == "Rattata"), "Route 1's species: {:?}", texts);
}
