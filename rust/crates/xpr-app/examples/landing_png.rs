//! Render the landing page to PNGs, headless, through the app's software
//! rasteriser, with made-up routes (one per game, plus a game and a species
//! with no art) so the picture doesn't depend on this machine's saved routes.
//!
//! ```text
//! cargo run -p xpr-app --example landing_png -- <out_dir> [width height]
//! ```
//!
//! Writes `landing.png`, `landing_game_menu.png` with the Game sort's
//! drop-down open, and `landing_game_picked.png` with a game picked from it.

use std::path::PathBuf;

use egui::{Event, PointerButton, Pos2, Rect, Shape, Vec2};
use xpr_app::assets::Assets;
use xpr_app::pages::{LandingActions, LandingPage, SORT_GAME, SORT_MOST_RECENT};
use xpr_app::route_index::{IndexEntry, RouteIndex};
use xpr_core::{consts, Config, Paths};
use xpr_data::Registry;
use xpr_ui_kit::offscreen::Offscreen;
use xpr_ui_kit::theme::Theme;

const SPECIES: &[&str] = &["Pinsir", "Nidoran_M", "Mr. Mime", "Farfetch'd", "Ho-Oh", "Porygon-Z", "Mewtwo", "Bulbasaur", "Unknown Thing"];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(out_dir) = args.first().map(PathBuf::from) else {
        eprintln!("usage: landing_png <out_dir> [width height]");
        std::process::exit(2);
    };
    std::fs::create_dir_all(&out_dir).expect("create the output directory");
    let w: f32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1100.0);
    let h: f32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1000.0);
    let size = Vec2::new(w, h);
    let full = Rect::from_min_size(Pos2::ZERO, size);

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap();
    let mut cfg = Config::load(&xpr_app::scratch_config_path());
    cfg.set_landing_page_game_filter("All Games");
    cfg.set_landing_page_search_filter("");
    let theme = Theme::from_config(&cfg);
    let paths = Paths::new(root.clone());
    let registry = Registry::new(root.join("raw_pkmn_data"), PathBuf::new());
    let mut index = RouteIndex { loaded: true, ..Default::default() };
    let versions = consts::VERSION_LIST.iter().map(|v| v.to_string()).chain(["My Custom Game".to_string()]);
    for (i, version) in versions.enumerate() {
        let species = SPECIES[i % SPECIES.len()].to_string();
        let name = format!("{} {} route", version, species);
        index.entries.insert(name.clone(), IndexEntry { name, version, species, mtime: 1_790_000_000.0 + i as f64 * 3600.0 });
    }

    let mut assets = Assets::new();
    let mut off = Offscreen::new(&theme, 2.0, 8192);
    cfg.set_landing_page_sort(SORT_MOST_RECENT);
    let mut recent = LandingPage::new(&cfg);
    cfg.set_landing_page_sort(SORT_GAME);
    let mut by_game = LandingPage::new(&cfg);
    let mut draw = |off: &mut Offscreen, page: &mut LandingPage, passes: usize, events: Vec<Event>| {
        off.render_with_events(size, passes, events, |root| {
            egui::CentralPanel::default().frame(egui::Frame::new().fill(theme.bg).inner_margin(8)).show(root, |ui| {
                let mut actions = LandingActions::default();
                page.ui(ui, &theme, &mut cfg, &paths, &registry, &index, &mut assets, &mut actions);
            });
        })
    };

    let prims = draw(&mut off, &mut recent, 6, Vec::new());
    off.rasterize(&prims, full).expect("rasterise").save(&out_dir.join("landing.png")).expect("write png");

    draw(&mut off, &mut by_game, 3, Vec::new());
    // the combo's text: the filter is the only place "All Games" is drawn
    let at = text_center(&off, "All Games");
    let click = click_at(at);
    draw(&mut off, &mut by_game, 1, click);
    // long enough for the popup to finish fading in
    let prims = draw(&mut off, &mut by_game, 30, Vec::new());
    off.rasterize(&prims, full).expect("rasterise").save(&out_dir.join("landing_game_menu.png")).expect("write png");

    // pick a game: the closed button shows its art, the list only its routes
    let at = text_center(&off, consts::CRYSTAL_VERSION);
    draw(&mut off, &mut by_game, 1, click_at(at));
    let prims = draw(&mut off, &mut by_game, 30, Vec::new());
    off.rasterize(&prims, full).expect("rasterise").save(&out_dir.join("landing_game_picked.png")).expect("write png");
    println!("wrote {}", out_dir.display());
}

/// Where `text` was last drawn; the topmost when it appears twice.
fn text_center(off: &Offscreen, text: &str) -> Pos2 {
    off.last_shapes()
        .iter()
        .rev()
        .find_map(|s| match &s.shape {
            Shape::Text(t) if t.galley.text() == text => Some(t.pos + t.galley.rect.center().to_vec2()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{} is not drawn", text))
}

fn click_at(at: Pos2) -> Vec<Event> {
    vec![
        Event::PointerMoved(at),
        Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: Default::default() },
        Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: false, modifiers: Default::default() },
    ]
}
