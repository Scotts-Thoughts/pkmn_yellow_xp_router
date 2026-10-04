//! The landing page's "View Map" (`pages::MapViewerPage`): a game's map with
//! no route open, so the map browses (no dock toggle, no "add" buttons).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Pos2, RawInput, Rect, Shape, Vec2};

use xpr_app::assets::Assets;
use xpr_app::map::MapView;
use xpr_app::pages::{MapViewerActions, MapViewerPage};
use xpr_core::{consts, Config, Paths};
use xpr_data::Registry;
use xpr_ui_kit::theme::Theme;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

struct Harness {
    ctx: egui::Context,
    theme: Theme,
    cfg: Config,
    assets: Assets,
    map: MapView,
    viewer: MapViewerPage,
    texts: Vec<String>,
}

impl Harness {
    fn new(preferred: Option<&str>) -> Harness {
        let root = repo_root();
        let cfg = Config::load(&xpr_app::scratch_config_path());
        let ctx = egui::Context::default();
        let mut theme = Theme::from_config(&cfg);
        theme.install_fonts(&ctx);
        theme.apply(&ctx);
        let paths = Paths::new(root.clone());
        let registry = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
        let mut map = MapView::new(&cfg, &root.join("raw_pkmn_data"));
        map.browse = true;
        let viewer = MapViewerPage::new(&registry, &paths, &map, preferred).expect("some game has map data");
        Harness { ctx, theme, cfg, assets: Assets::new(), map, viewer, texts: Vec::new() }
    }

    fn frame(&mut self) -> MapViewerActions {
        let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0))), ..Default::default() };
        let mut actions = MapViewerActions::default();
        let Harness { ctx, theme, cfg, assets, map, viewer, texts } = self;
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                actions = viewer.ui(ui, theme, cfg, map, assets);
            });
        });
        texts.clear();
        for cs in &out.shapes {
            if let Shape::Text(t) = &cs.shape {
                texts.push(t.galley.text().to_string());
            }
        }
        actions
    }

    fn wait_ready(&mut self) {
        let t = Instant::now();
        while !self.map.is_ready() {
            self.frame();
            assert!(t.elapsed() < Duration::from_secs(30), "map pack never loaded");
            std::thread::sleep(Duration::from_millis(10));
        }
        self.frame();
    }
}

#[test]
fn opens_the_preferred_game_without_a_route() {
    let mut h = Harness::new(Some(consts::YELLOW_VERSION));
    assert_eq!(h.viewer.ctrl.get_version(), Some(consts::YELLOW_VERSION));
    assert!(h.viewer.ctrl.gen().is_some());
    assert!(h.viewer.ctrl.router.init_route_state.is_none());
    h.wait_ready();
    assert_eq!(h.map.pack().map(|p| p.game.as_str()), Some("yellow"));
    assert!(h.texts.iter().any(|t| t.contains("Back")), "back button drawn: {:?}", h.texts);
    assert!(!h.texts.iter().any(|t| t == "⤢" || t == "⤡"), "no dock toggle while browsing");
}

#[test]
fn unknown_preference_falls_back_to_a_game_with_map_data() {
    let h = Harness::new(Some("Not A Game"));
    let v = h.viewer.ctrl.get_version().expect("a version");
    let game = xpr_map::game_for_version(v).expect("a mapped version");
    assert!(h.map.has_pack(game));
}
