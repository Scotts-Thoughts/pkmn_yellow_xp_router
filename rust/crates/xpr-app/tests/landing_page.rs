//! The landing page: no title above the buttons, and the game drop-down with
//! box art still picks and filters like the plain one.

use std::path::PathBuf;

use egui::{Event, PointerButton, Pos2, Rect, Shape, Vec2};
use xpr_app::assets::Assets;
use xpr_app::pages::{LandingActions, LandingPage, SORT_GAME};
use xpr_app::route_index::{IndexEntry, RouteIndex};
use xpr_core::{consts, Config, Paths};
use xpr_data::Registry;
use xpr_ui_kit::offscreen::Offscreen;
use xpr_ui_kit::theme::Theme;

struct Harness {
    off: Offscreen,
    theme: Theme,
    cfg: Config,
    paths: Paths,
    registry: Registry,
    index: RouteIndex,
    assets: Assets,
    page: LandingPage,
}

impl Harness {
    fn new() -> Harness {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap();
        let mut cfg = Config::load(&xpr_app::scratch_config_path());
        cfg.set_landing_page_sort(SORT_GAME);
        cfg.set_landing_page_game_filter("All Games");
        cfg.set_landing_page_search_filter("");
        let theme = Theme::from_config(&cfg);
        let mut index = RouteIndex { loaded: true, ..Default::default() };
        for (name, version, species) in [("yellow run", consts::YELLOW_VERSION, "Pinsir"), ("crystal run", consts::CRYSTAL_VERSION, "Ho-Oh")] {
            index.entries.insert(name.to_string(), IndexEntry { name: name.to_string(), version: version.to_string(), species: species.to_string(), mtime: 1.0 });
        }
        let page = LandingPage::new(&cfg);
        Harness {
            off: Offscreen::new(&theme, 1.0, 8192),
            theme,
            cfg,
            paths: Paths::new(root.clone()),
            registry: Registry::new(root.join("raw_pkmn_data"), PathBuf::new()),
            index,
            assets: Assets::new(),
            page,
        }
    }

    fn frames(&mut self, passes: usize, events: Vec<Event>) {
        let Harness { off, theme, cfg, paths, registry, index, assets, page } = self;
        off.render_with_events(Vec2::new(1100.0, 1000.0), passes, events, |root| {
            egui::CentralPanel::default().frame(egui::Frame::new().fill(theme.bg).inner_margin(8)).show(root, |ui| {
                let mut actions = LandingActions::default();
                page.ui(ui, theme, cfg, paths, registry, index, assets, &mut actions);
            });
        });
    }

    /// Where `text` was last drawn; the topmost when it appears twice.
    fn text_at(&self, text: &str) -> Option<Rect> {
        self.off.last_shapes().iter().rev().find_map(|s| match &s.shape {
            Shape::Text(t) if t.galley.text() == text => Some(t.galley.rect.translate(t.pos.to_vec2())),
            _ => None,
        })
    }

    fn click(&mut self, at: Pos2) {
        let events = vec![
            Event::PointerMoved(at),
            Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: Default::default() },
            Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: false, modifiers: Default::default() },
        ];
        self.frames(1, events);
        self.frames(3, Vec::new());
    }
}

/// Left and right edge of everything drawn on the row around `y`, leaving out
/// the page background.
fn row_extent(h: &Harness, y: f32) -> (f32, f32) {
    let mut edges = (f32::MAX, f32::MIN);
    for s in h.off.last_shapes() {
        let r = s.shape.visual_bounding_rect();
        if r.is_finite() && r.width() < 900.0 && r.min.y <= y && y <= r.max.y {
            edges = (edges.0.min(r.min.x), edges.1.max(r.max.x));
        }
    }
    edges
}

/// The search row and the table share both edges, and the dates end at the
/// right of the rows: with a scrollbar (`routes` long) or without.
fn assert_table_lines_up(routes: usize) {
    let mut h = Harness::new();
    for i in 0..routes {
        let name = format!("extra route {}", i);
        h.index.entries.insert(name.clone(), IndexEntry { name, version: consts::RED_VERSION.to_string(), species: "Abra".to_string(), mtime: i as f64 });
    }
    h.frames(4, Vec::new());

    let search = row_extent(&h, h.text_at("Search:").expect("the search row is drawn").center().y);
    let table = row_extent(&h, h.text_at("Date Played").expect("the header is drawn").center().y);
    assert!((search.0 - table.0).abs() <= 1.0, "left edges: search {:?}, table {:?}", search, table);
    assert!((search.1 - table.1).abs() <= 1.0, "right edges: search {:?}, table {:?}", search, table);

    // every test route is dated at the epoch, so in 1969 or 1970 by time zone
    let date = h
        .off
        .last_shapes()
        .iter()
        .find_map(|s| match &s.shape {
            Shape::Text(t) if t.galley.text().len() == 16 && t.galley.text().starts_with("19") => Some(t.galley.rect.translate(t.pos.to_vec2())),
            _ => None,
        })
        .expect("a date is drawn");
    // Room for the scrollbar (~11 px) and the cell padding, no more.
    let gap = table.1 - date.max.x;
    assert!(gap < 20.0, "{} px of space beside the dates", gap);
}

#[test]
fn the_table_lines_up_with_the_search_row_without_a_scrollbar() {
    assert_table_lines_up(0);
}

#[test]
fn the_table_lines_up_with_the_search_row_with_a_scrollbar() {
    assert_table_lines_up(200);
}

#[test]
fn the_buttons_start_at_the_top_of_the_page() {
    let mut h = Harness::new();
    h.frames(3, Vec::new());
    assert!(h.text_at("Pokemon Solo Challenge Router").is_none());
    let record = h.text_at("Start Recording").expect("the record button is drawn");
    assert!(record.min.y < 60.0, "Start Recording starts at y = {}", record.min.y);
}

/// Start Recording and Route from Replay above a two-by-two grid of the same
/// width, in reading order, with Map last.
#[test]
fn the_small_buttons_fill_the_width_of_the_wide_ones() {
    let mut h = Harness::new();
    h.frames(3, Vec::new());
    let at = |h: &Harness, text: &str| h.text_at(text).unwrap_or_else(|| panic!("{} is not drawn", text)).center();
    let (create, load, compare, map) = (at(&h, "Create"), at(&h, "Load"), at(&h, "Compare"), at(&h, "Map"));
    let replay = at(&h, "Route from Replay");

    assert!(replay.y < create.y && create.y < compare.y, "wide buttons, then the Create row, then the Compare row");
    assert!((create.y - load.y).abs() <= 1.0 && (compare.y - map.y).abs() <= 1.0, "each row shares a line");
    assert!(create.x < load.x && compare.x < map.x, "Load and Map are on the right");
    assert!((create.x - compare.x).abs() <= 1.0 && (load.x - map.x).abs() <= 1.0, "the columns line up");

    let wide = row_extent(&h, replay.y);
    assert!(wide.1 - wide.0 >= 349.0, "the wide button is {:?}", wide);
    for y in [create.y, compare.y] {
        let row = row_extent(&h, y);
        assert!((row.0 - wide.0).abs() <= 1.0 && (row.1 - wide.1).abs() <= 1.0, "row at {}: {:?}, wide button {:?}", y, row, wide);
    }
}

#[test]
fn picking_a_game_from_the_menu_filters_the_list() {
    let mut h = Harness::new();
    h.frames(3, Vec::new());
    assert!(h.text_at("yellow run").is_some() && h.text_at("crystal run").is_some());

    let closed = h.text_at("All Games").expect("the filter is drawn").center();
    h.click(closed);
    // the popup's row, drawn over the table's Game cell
    let row = h.text_at(consts::CRYSTAL_VERSION).expect("the menu lists the game").center();
    h.click(row);

    assert_eq!(h.cfg.get_landing_page_game_filter(), consts::CRYSTAL_VERSION);
    assert!(h.text_at("crystal run").is_some());
    assert!(h.text_at("yellow run").is_none(), "other games are filtered out");
    assert!(h.text_at("Yellow").is_none(), "the menu closes after a pick");
}

#[test]
fn the_list_and_menu_cover_every_game() {
    let ctx = egui::Context::default();
    let mut assets = Assets::new();
    for version in consts::VERSION_LIST {
        assert!(assets.box_art(&ctx, version).is_some(), "no box art for {}", version);
    }
}
