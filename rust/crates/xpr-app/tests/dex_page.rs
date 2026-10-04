//! The Dex page as the app hosts it: the Damage tab (the router's battle
//! summary for a free-form player), requests from other tabs, copying the
//! open route's Pokémon, and the name bridges between the Dex's data and
//! the router's.

use std::path::PathBuf;
use std::sync::Arc;

use egui::{Event, Pos2, RawInput, Rect, Shape, Vec2};
use serde_json::json;
use xpr_app::assets::Assets;
use xpr_app::controller::MainController;
use xpr_app::dex_damage::{resolve_router_move, resolve_router_species, DexDamage};
use xpr_core::{Config, Paths};
use xpr_data::Registry;
use xpr_dex_ui::{DexCx, DexHost, DexTab, DexView, RouteContext};
use xpr_ui_kit::theme::Theme;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

struct Host<'a> {
    damage: &'a mut DexDamage,
    cfg: &'a mut Config,
    assets: &'a mut Assets,
    route: &'a MainController,
}

impl DexHost for Host<'_> {
    fn damage_ui(&mut self, ui: &mut egui::Ui, cx: &mut DexCx) {
        self.damage.ui(ui, cx, self.cfg, self.assets, self.route);
    }
    fn can_add_to_route(&self, version: &str) -> bool {
        self.route.get_version() == Some(version)
    }
}

struct Harness {
    ctx: egui::Context,
    theme: Theme,
    cfg: Config,
    registry: Arc<Registry>,
    route: MainController,
    damage: DexDamage,
    assets: Assets,
    dex: DexView,
    texts: Vec<String>,
}

impl Harness {
    fn new(settings: serde_json::Value, route_file: Option<&str>) -> Harness {
        let root = repo_root();
        let cfg = Config::load(&xpr_app::scratch_config_path());
        let ctx = egui::Context::default();
        let mut theme = Theme::from_config(&cfg);
        theme.install_fonts(&ctx);
        theme.apply(&ctx);
        let paths = Paths::new(root.clone());
        let registry = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
        let mut route = MainController::new(registry.clone(), paths.clone());
        if let Some(f) = route_file {
            route.load_route(&root.join("tests/test_data").join(f));
            let _ = route.take_signals();
        }
        let damage = DexDamage::new(registry.clone(), paths);
        Harness { ctx, theme, cfg, registry, route, damage, assets: Assets::new(), dex: DexView::new(settings), texts: Vec::new() }
    }

    fn route_context(&self) -> RouteContext {
        RouteContext { version: self.route.get_version().map(str::to_string), solo_species: self.route.get_init_state().map(|s| s.solo_pkmn.name.clone()) }
    }

    fn frame(&mut self, events: Vec<Event>) {
        let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 1000.0))), events, ..Default::default() };
        let rc = self.route_context();
        let Harness { ctx, theme, cfg, registry, route, damage, assets, dex, .. } = self;
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut host = Host { damage, cfg, assets, route };
                let _ = dex.ui(ui, theme, registry, &rc, &mut host);
            });
        });
        self.texts.clear();
        for cs in &out.shapes {
            collect(&cs.shape, &mut self.texts);
        }
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.frame(vec![]);
        }
    }

    fn shows(&self, needle: &str) -> bool {
        self.texts.iter().any(|t| t.contains(needle))
    }
}

fn collect(shape: &Shape, out: &mut Vec<String>) {
    match shape {
        Shape::Text(t) => out.push(t.galley.text().to_string()),
        Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
        _ => {}
    }
}

#[test]
fn damage_tab_runs_the_battle_summary_for_a_free_form_player() {
    let mut h = Harness::new(json!({ "tab": "Damage", "version": "Crystal" }), None);
    h.dex.state.request_damage(Some("Crystal".into()), Some("Totodile".into()), Some("Leader Falkner".into()), vec![]);
    h.frames(4);
    assert!(h.shows("Totodile Lv50 Damage Ranges"), "{:?}", h.texts);
    assert!(h.shows("Pidgeotto Lv9 Damage Ranges"));
    assert!(h.shows("Pidgey Lv7, Pidgeotto Lv9"));
    assert!(h.shows("Hidden Power:"));
}

#[test]
fn a_request_from_another_tab_switches_game_species_trainer_and_moves() {
    let mut h = Harness::new(json!({ "tab": "Pokedex", "version": "Red" }), None);
    h.frames(2);
    // the Pokedex's test set uses the Dex's spellings
    h.dex.state.request_damage(Some("Emerald".into()), Some("Mudkip".into()), Some("Leader Roxanne".into()), vec!["Water Gun".into(), "Mud-Slap".into()]);
    assert_eq!(h.dex.tab(), DexTab::Damage);
    h.frames(4);
    assert_eq!(h.dex.state.settings.version.as_deref(), Some("Emerald"));
    assert!(h.shows("Mudkip Lv50 Damage Ranges"), "{:?}", h.texts);
    assert!(h.shows("Water Gun"));
    assert!(h.texts.iter().any(|t| t.starts_with("Mud") && t.contains("Slap")), "Mud-Slap resolved to the router's spelling");
    assert!(h.shows("Nosepass Lv15 Damage Ranges"));
}

#[test]
fn router_names_resolve_from_dex_names() {
    let reg = Registry::new(repo_root().join("raw_pkmn_data"), PathBuf::new());
    let red = reg.get_version("Red").unwrap();
    assert!(resolve_router_move(&red, "Double-Edge").is_some());
    assert!(resolve_router_move(&red, "Bubble Beam").is_some(), "modern spelling of BubbleBeam");
    assert!(resolve_router_move(&red, "Thunder Shock").is_some());
    assert!(resolve_router_species(&red, "Nidoran_F").is_some(), "the Dex's Nidoran_F names a router species");
    assert!(resolve_router_species(&red, "Mr. Mime").is_some());
    let em = reg.get_version("Emerald").unwrap();
    assert!(resolve_router_move(&em, "Feint Attack").is_some());
    assert!(resolve_router_move(&em, "Not A Move").is_none());
}

#[test]
fn opening_from_a_route_starts_on_its_game_and_pokemon() {
    let mut h = Harness::new(json!({ "tab": "Pokedex", "game": "Sun and Moon", "selected": "Rowlet" }), Some("yellow-pinsir-lv10brock.json"));
    let rc = h.route_context();
    assert_eq!(rc.version.as_deref(), Some("Yellow"));
    let registry = h.registry.clone();
    h.dex.sync_to_route(&rc, &registry);
    assert_eq!(h.dex.state.game(), "Yellow");
    assert_eq!(h.dex.state.selected(), Some("Pinsir"));
    assert_eq!(h.dex.state.settings.version.as_deref(), Some("Yellow"));
}

#[test]
fn damage_tab_starts_on_the_routes_pokemon() {
    let mut h = Harness::new(json!({ "tab": "Damage", "version": "Yellow" }), Some("yellow-pinsir-lv10brock.json"));
    h.frames(4);
    assert!(h.texts.iter().any(|t| t.starts_with("Pinsir Lv") && t.ends_with("Damage Ranges")), "{:?}", h.texts);
    // a game the route is not of starts on the Pokedex's selection instead
    let mut h = Harness::new(json!({ "tab": "Damage", "version": "Crystal", "selected": "Cyndaquil" }), Some("yellow-pinsir-lv10brock.json"));
    h.frames(4);
    assert!(h.shows("Cyndaquil Lv50 Damage Ranges"), "{:?}", h.texts);
}
