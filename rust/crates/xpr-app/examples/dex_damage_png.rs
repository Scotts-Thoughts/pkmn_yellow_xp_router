//! Render the Dex page's Damage tab to a PNG, headless:
//!
//! ```text
//! cargo run -p xpr-app --example dex_damage_png -- <out.png> [version] [species] [trainer] [WxH]
//! ```

use std::path::PathBuf;
use std::sync::Arc;

use egui::{Pos2, Rect, Vec2};
use serde_json::json;
use xpr_app::assets::Assets;
use xpr_app::controller::MainController;
use xpr_app::dex_damage::DexDamage;
use xpr_app::screenshot::Offscreen;
use xpr_core::{Config, Paths};
use xpr_data::Registry;
use xpr_dex_ui::{DexCx, DexHost, DexView, RouteContext};
use xpr_ui_kit::theme::Theme;

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
    fn can_add_to_route(&self, _version: &str) -> bool {
        false
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = PathBuf::from(args.first().cloned().unwrap_or_else(|| "dex_damage.png".into()));
    let version = args.get(1).cloned().unwrap_or_else(|| "Crystal".into());
    let species = args.get(2).cloned();
    let trainer = args.get(3).cloned();
    let size = args.get(4).and_then(|s| s.split_once('x')).map(|(w, h)| Vec2::new(w.parse().unwrap(), h.parse().unwrap())).unwrap_or(Vec2::new(1600.0, 1000.0));

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap();
    let mut cfg = Config::load(&xpr_app::scratch_config_path());
    let theme = Theme::from_config(&cfg);
    let paths = Paths::new(root.clone());
    let registry = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    let route_ctrl = MainController::new(registry.clone(), paths.clone());
    let mut assets = Assets::new();
    let mut damage = DexDamage::new(registry.clone(), paths);
    let mut dex = DexView::new(json!({ "tab": "Damage", "version": version }));
    if species.is_some() || trainer.is_some() {
        dex.state.request_damage(Some(version.clone()), species, trainer, Vec::new());
    }
    let route = RouteContext::default();
    let mut off = Offscreen::new(&theme, 1.0, 8192);
    let prims = off.render(size, 6, |ctx| {
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme.bg)).show(ctx, |ui| {
            let mut host = Host { damage: &mut damage, cfg: &mut cfg, assets: &mut assets, route: &route_ctrl };
            let _ = dex.ui(ui, &theme, &registry, &route, &mut host);
        });
    });
    let canvas = off.rasterize(&prims, Rect::from_min_size(Pos2::ZERO, size)).unwrap();
    canvas.save(&out).unwrap();
    eprintln!("wrote {}", out.display());
}
