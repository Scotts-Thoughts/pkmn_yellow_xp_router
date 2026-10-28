//! A gen 4 route on the world map (GEN45_REQUIREMENTS §8): the Platinum
//! route opens Sinnoh as an image-world pack drawn from the pack's pictures
//! (`map_data/platinum/imagery.zip`, which releases ship as a separate
//! download); without them the map is a terrain sketch and says so; a
//! downloaded copy in the cache is used when it matches; a tall-grass tile opens its
//! encounter card, whose condition chips switch the tables.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape, Vec2};

use xpr_app::assets::Assets;
use xpr_app::controller::MainController;
use xpr_app::map::{MapAction, MapView};
use xpr_core::{Config, Paths};
use xpr_data::Registry;
use xpr_map::{MapPack, Scope};
use xpr_ui_kit::theme::Theme;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

struct Harness {
    ctx: egui::Context,
    theme: Theme,
    cfg: Config,
    ctrl: MainController,
    assets: Assets,
    view: MapView,
    size: Vec2,
    /// every text drawn in the last frame, with where it was drawn
    texts: Vec<(String, Rect)>,
}

fn collect_text(shape: &Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Text(t) => out.push((t.galley.text().to_string(), Rect::from_min_size(t.pos, t.galley.size()))),
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

    fn has_text(&self, needle: &str) -> bool {
        self.texts.iter().any(|(t, _)| t.contains(needle))
    }

    fn text_rect(&self, exact: &str) -> Option<Rect> {
        self.texts.iter().find(|(t, _)| t == exact).map(|(_, r)| *r)
    }

    /// A tall-grass step of `map_const` that is on screen and clear of markers.
    fn grass_step(&mut self, pack: &MapPack, map_const: &str) -> (u16, i32, i32, Pos2) {
        let map = pack.map_by_const(map_const).unwrap_or_else(|| panic!("{} in the pack", map_const));
        let (id, w, h) = (map.id, map.w as i32, map.h as i32);
        self.view.navigate_to(id, false);
        self.frame(vec![]);
        self.frame(vec![]);
        let objects: Vec<Pos2> = pack.object_range(id).filter_map(|i| self.view.object_screen_pos(i)).collect();
        for y in 0..h {
            for x in 0..w {
                if !pack.terrain_at(id, x as u32, y as u32).0 {
                    continue;
                }
                let Some(p) = self.view.step_screen_pos(id, x, y) else { continue };
                // clear of the navigator (top right)
                let on_screen = p.x > 60.0 && p.x < 700.0 && p.y > 220.0 && p.y < 620.0;
                let clear = objects.iter().all(|o| (o.x - p.x).abs() > 40.0 || (o.y - p.y).abs() > 40.0);
                if on_screen && clear {
                    return (id, x, y, p);
                }
            }
        }
        panic!("no clear grass step on {}", map_const);
    }
}

/// A stand-in for the GitHub releases API with `zip` as Platinum's pictures
/// asset; the file itself is held back until `gate` opens.
fn serve_release(zip: &std::path::Path, sha256: &str, gate: Arc<AtomicBool>) -> String {
    use std::io::{BufRead, BufReader, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let name = xpr_update::assets::imagery_asset_name("platinum", sha256);
    let (zip, thread_base) = (zip.to_path_buf(), base.clone());
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            let _ = reader.read_line(&mut line);
            let mut h = String::new();
            while reader.read_line(&mut h).map(|n| n > 2).unwrap_or(false) {
                h.clear();
            }
            let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
            let mut out = &stream;
            if path.starts_with("/releases") {
                let size = std::fs::metadata(&zip).unwrap().len();
                let body = if path.ends_with("page=1") {
                    serde_json::json!([{"tag_name": "v6.3a", "assets": [{"name": name, "size": size, "browser_download_url": format!("{}/files/{}", thread_base, name)}]}])
                } else {
                    serde_json::json!([])
                }
                .to_string();
                let _ = write!(out, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
            } else {
                while !gate.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(10));
                }
                let mut f = std::fs::File::open(&zip).unwrap();
                let _ = write!(out, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", f.metadata().unwrap().len());
                let _ = std::io::copy(&mut f, &mut out);
            }
        }
    });
    base
}

#[test]
fn a_platinum_route_opens_sinnoh_and_its_encounter_cards_switch_conditions() {
    let scratch = std::env::temp_dir().join(format!("xpr_map_image_world_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();

    // run from source, the pack folder's imagery.zip has the pictures
    std::env::remove_var("XPR_MAP_DATA_DIR");
    let mut h = Harness::new("platinum_chimchar.json");
    h.wait_ready();
    let pack = h.view.pack().unwrap().clone();
    assert_eq!(pack.game, "platinum");
    assert!(pack.is_image());
    assert!(h.has_text("Sinnoh"), "the world's title");
    assert!(h.view.has_imagery(), "map_data/platinum/imagery.zip");
    assert!(!h.has_text("map pictures missing"));

    // a tall-grass tile of Route 201 opens its encounter card
    let (map, x, y, p) = h.grass_step(&pack, "MAP_HEADER_ROUTE_201");
    assert_eq!(xpr_map::geom::pick_step(&pack, Scope::World, h.view.camera_world(p).0, h.view.camera_world(p).1), Some((map, x, y)));
    h.click(p);
    assert!(h.view.card_is_tile(), "a grass click opens the encounter card");
    assert!(h.has_text("Starly"), "Route 201's wild Pokémon");
    assert!(h.has_text("Walk"), "the method's label from the manifest");
    // the condition chips: Day is the plain table; Night switches to the night table
    let night = h.text_rect("Night").expect("a Night chip");
    assert!(h.text_rect("Day").is_some(), "a Day chip");
    h.click(night.center());
    assert!(h.view.card_is_tile(), "the chip keeps the card open");
    assert!(h.has_text("Walk · Night"), "the night table is shown");

    // no pictures built in (this test binary embeds none), no pack folder and
    // nothing downloaded (downloads are off here): the sketch, and a note
    let empty = scratch.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    std::env::set_var("XPR_MAP_DATA_DIR", &empty);
    std::env::set_var("XPR_MAP_IMAGERY_DIR", &empty);
    std::env::set_var("XPR_NO_MAP_DOWNLOAD", "1");
    let mut h2 = Harness::new("platinum_chimchar.json");
    h2.wait_ready();
    h2.frame(Vec::new());
    assert!(!h2.view.has_imagery());
    assert!(!h2.view.is_fetching_imagery());
    assert!(h2.has_text("map pictures not downloaded"));

    // a downloaded copy in the cache, named after its content (`xpr_update::assets`)
    let cache = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("xpr_map_imagery_cache_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cache);
    std::fs::create_dir_all(&cache).unwrap();
    let info = pack.image.as_ref().unwrap().imagery.clone().unwrap();
    let cached = cache.join(xpr_update::assets::imagery_asset_name("platinum", &info.sha256));
    let src = repo_root().join("map_data/platinum/imagery.zip");
    if std::fs::hard_link(&src, &cached).is_err() {
        std::fs::copy(&src, &cached).unwrap();
    }
    std::env::set_var("XPR_MAP_IMAGERY_DIR", &cache);
    let mut h4 = Harness::new("platinum_chimchar.json");
    h4.wait_ready();
    assert!(h4.view.has_imagery(), "the cached download");
    // a copy of another version (a stale cache) is not used
    std::fs::remove_file(&cached).unwrap();
    std::fs::write(cache.join(xpr_update::assets::imagery_asset_name("platinum", &"0".repeat(64))), b"old").unwrap();
    let mut h5 = Harness::new("platinum_chimchar.json");
    h5.wait_ready();
    assert!(!h5.view.has_imagery());

    // with downloads on, the map fetches its pictures from the release
    // assets (a local stand-in for GitHub), shows the progress, then draws them
    let gate = Arc::new(AtomicBool::new(false));
    let base = serve_release(&src, &info.sha256, gate.clone());
    std::env::set_var("XPR_RELEASES_URL", format!("{}/releases", base));
    std::env::remove_var("XPR_NO_MAP_DOWNLOAD");
    let mut h6 = Harness::new("platinum_chimchar.json");
    h6.wait_ready();
    assert!(h6.view.is_fetching_imagery(), "the download starts by itself");
    assert!(h6.has_text("Downloading this game's map pictures"));
    assert!(h6.has_text("downloading map pictures"), "the status strip");
    gate.store(true, Ordering::SeqCst);
    let t = Instant::now();
    while h6.view.is_fetching_imagery() {
        assert!(t.elapsed() < Duration::from_secs(60), "the download did not finish");
        h6.frame(vec![]);
        std::thread::sleep(Duration::from_millis(20));
    }
    h6.frame(vec![]);
    assert!(h6.view.has_imagery(), "the downloaded pictures are drawn");
    assert!(!h6.has_text("Downloading this game's map pictures"));
    assert!(cached.is_file(), "kept in the cache under its asset name");
    assert!(!cache.join(xpr_update::assets::imagery_asset_name("platinum", &"0".repeat(64))).exists(), "the stale copy is dropped");
    std::env::remove_var("XPR_RELEASES_URL");
    std::env::set_var("XPR_NO_MAP_DOWNLOAD", "1");
    std::env::remove_var("XPR_MAP_IMAGERY_DIR");
    let _ = std::fs::remove_dir_all(&cache);

    // pictures as loose folders beside the pack (a pokemap checkout's layout)
    let data = scratch.join("map_data");
    std::fs::create_dir_all(data.join("platinum/world/0")).unwrap();
    let tile = xpr_map::Pixmap::filled(512, 512, [200, 30, 30, 255]).to_png().unwrap();
    std::fs::write(data.join("platinum/world/0/0_0.webp"), tile).unwrap();
    std::env::set_var("XPR_MAP_DATA_DIR", &data);
    let mut h3 = Harness::new("platinum_chimchar.json");
    h3.wait_ready();
    assert!(h3.view.has_imagery());
    std::env::remove_var("XPR_MAP_DATA_DIR");
    let _ = std::fs::remove_dir_all(&scratch);
}
