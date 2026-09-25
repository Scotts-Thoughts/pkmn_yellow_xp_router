//! The map image export dialog and pipeline (`docs/rust_port/design/
//! world_map/GRAPHICS_TOOLS_PLAN.md` WP-B): the blocking path's pixels
//! match the compositor directly and a marker changes the pixels near a
//! trainer, a transparent world export is empty at a point no map covers,
//! the dialog flow (`request_open` -> the modal on screen -> a job ->
//! `MapAction::Exported`) delivers a real file, and `smoke_export_kind
//! ("map")` parses. Follows `tests/map_view.rs`'s headless harness.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Event, Pos2, RawInput, Rect, Shape, Vec2};

use xpr_app::app::smoke_export_kind;
use xpr_app::assets::Assets;
use xpr_app::controller::MainController;
use xpr_app::map::export::{run_export_blocking, spec_for, start_export_test, ExportOutcome, ExportRegion, ExportSpec};
use xpr_app::map::{layers, MapAction, MapView, Toggles};
use xpr_app::screenshot::ShotKind;
use xpr_core::{Config, Paths};
use xpr_data::Registry;
use xpr_map::{geom, Compositor, IRect, MapId, MapPack, ObjectKind, PackSource, RenderOpts, Scope};
use xpr_ui_kit::theme::Theme;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

/// A scratch output directory under `rust/target/` (never `/tmp`), unique
/// per test by name so parallel `#[test]` functions don't collide.
fn scratch_dir(name: &str) -> PathBuf {
    let dir = repo_root().join("rust/target/test_scratch/map_export").join(name);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// A second, independent `Compositor` loaded straight from `map_data/`
/// (`xpr-map/tests/export.rs`'s own convention), so the base-pixel and
/// transparency checks below verify the export against the compositor
/// directly rather than against the harness's own `MapView`.
fn reference_compositor(game: &str) -> (Arc<MapPack>, Compositor) {
    let pack = Arc::new(MapPack::load(game, &PackSource::new(Some(repo_root().join("map_data")))).expect("reference pack loads"));
    let comp = Compositor::new(pack.clone());
    (pack, comp)
}

/// A point (map-local world px) at least 40 world px from every object of
/// `map_id`, so an export's marker overlay never reaches it.
fn floor_point(pack: &MapPack, map_id: MapId) -> (i32, i32) {
    let rect = geom::scope_rect(pack, Scope::Map(map_id));
    let objects: Vec<(i32, i32)> = pack.object_range(map_id).filter_map(|i| geom::object_center_px(pack, Scope::Map(map_id), i)).collect();
    let mut y = 8;
    while y < rect.height() - 8 {
        let mut x = 8;
        while x < rect.width() - 8 {
            let clear = objects.iter().all(|&(ox, oy)| (ox - x).abs() > 40 || (oy - y).abs() > 40);
            if clear {
                return (x, y);
            }
            x += 8;
        }
        y += 8;
    }
    (2, 2)
}

fn collect_text(shape: &Shape, out: &mut Vec<String>) {
    match shape {
        Shape::Text(t) => out.push(t.galley.text().to_string()),
        Shape::Vec(v) => v.iter().for_each(|s| collect_text(s, out)),
        _ => {}
    }
}

struct Harness {
    ctx: egui::Context,
    theme: Theme,
    cfg: Config,
    ctrl: MainController,
    assets: Assets,
    view: MapView,
    size: Vec2,
    /// every text drawn in the last frame (`encounter_card.rs`'s pattern):
    /// used to notice the export modal on screen.
    texts: Vec<String>,
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
}

// ---------------------------------------------------------------------------
// the blocking path's pixels
// ---------------------------------------------------------------------------

#[test]
fn blocking_export_matches_the_compositor_and_a_marker_changes_brocks_pixels() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    let pack = h.view.pack().unwrap().clone();
    let gym = pack.map_by_const("PEWTER_GYM").expect("PEWTER_GYM is in the yellow pack");
    h.view.navigate_to(gym.id, false);
    h.frame(vec![]);
    h.frame(vec![]);
    assert_eq!(h.view.scope(), Scope::Map(gym.id), "the gym is a Scope::Map");

    let scale = 2u32;
    let rect = geom::scope_rect(&pack, Scope::Map(gym.id));
    let toggles = Toggles::default(); // markers + sprites on, matching the live viewer
    let spec = ExportSpec { scope: Scope::Map(gym.id), rect, scale, night: false, toggles, selection: None, transparent: false };

    let out_path = scratch_dir("blocking").join("pewter_gym.png");
    let outcome = run_export_blocking(&h.view, &h.theme, &spec, Some(&out_path)).expect("export succeeds");
    let path = match outcome {
        ExportOutcome::Exported(p) => p,
        other => panic!("expected a file export, got {:?}", other),
    };
    assert_eq!(path, out_path);
    assert!(path.exists(), "the PNG file was written");

    let img = image::open(&path).expect("a valid PNG").to_rgba8();
    assert_eq!((img.width(), img.height()), (rect.width() as u32 * scale, rect.height() as u32 * scale), "dims = map px size x scale");

    // --- a base pixel, well away from every marker, equals the compositor's own render_scope ---
    let (_ref_pack, ref_comp) = reference_compositor("yellow");
    let reference = ref_comp.render_scope(Scope::Map(gym.id), RenderOpts::default());
    let (fx, fy) = floor_point(&pack, gym.id);
    let exported = img.get_pixel(fx as u32 * scale, fy as u32 * scale).0;
    let expected = reference.get(fx as usize, fy as usize);
    assert_eq!(exported, expected, "a plain-floor base pixel at ({}, {}) matches Compositor::render_scope directly", fx, fy);

    // --- a marker is visible near Brock's step: an export with every marker
    // kind off changes the pixels there (robust to sprite-vs-circle and to
    // whether this route has already fought Brock, both of which shade the
    // marker differently; `layers::COLOR_TRAINER` is what circle mode uses) ---
    let brock = pack.objects.iter().position(|o| o.kind == ObjectKind::Trainer && o.trainer_names().iter().any(|n| n == "Brock 1")).expect("Brock 1 is in the pack") as u32;
    let (ox, oy) = geom::object_center_px(&pack, Scope::Map(gym.id), brock).expect("Brock has a position");
    let mut bare = toggles;
    bare.trainers = false;
    bare.items = false;
    bare.hidden = false;
    bare.warps = false;
    bare.signs = false;
    bare.berries = false;
    bare.npcs = false;
    let bare_spec = ExportSpec { toggles: bare, ..spec };
    let bare_outcome = run_export_blocking(&h.view, &h.theme, &bare_spec, None).expect("bare export succeeds");
    let ExportOutcome::Copied { width, height, rgba } = bare_outcome else { panic!("expected pixel data back, not a file") };
    assert_eq!((width as u32, height as u32), (img.width(), img.height()));
    let bare_img = image::RgbaImage::from_raw(width as u32, height as u32, rgba).expect("valid pixel buffer");

    let (tx, ty) = (ox * scale as i32, oy * scale as i32);
    let mut differs = false;
    'search: for dy in -12i32..=12 {
        for dx in -12i32..=12 {
            let (px, py) = (tx + dx, ty + dy);
            if px < 0 || py < 0 || px as u32 >= img.width() || py as u32 >= img.height() {
                continue;
            }
            if img.get_pixel(px as u32, py as u32) != bare_img.get_pixel(px as u32, py as u32) {
                differs = true;
                break 'search;
            }
        }
    }
    assert!(differs, "a marker near Brock's step ({}, {}) changes the exported pixels there (sprite or {:?})", tx, ty, layers::COLOR_TRAINER);
}

// ---------------------------------------------------------------------------
// transparency
// ---------------------------------------------------------------------------

#[test]
fn a_transparent_world_export_is_empty_where_no_map_reaches() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    let pack = h.view.pack().unwrap().clone();
    let rect = geom::scope_rect(&pack, Scope::World);
    let spec = ExportSpec { scope: Scope::World, rect, scale: 1, night: false, toggles: Toggles::default(), selection: None, transparent: true };

    let out_path = scratch_dir("world_transparent").join("world.png");
    let outcome = run_export_blocking(&h.view, &h.theme, &spec, Some(&out_path)).expect("export succeeds");
    let path = match outcome {
        ExportOutcome::Exported(p) => p,
        other => panic!("expected a file export, got {:?}", other),
    };
    let img = image::open(&path).expect("a valid PNG").to_rgba8();

    let (_ref_pack, ref_comp) = reference_compositor("yellow");
    let mut corner = None;
    'find: for y in (0..rect.height()).step_by(16) {
        for x in (0..rect.width()).step_by(16) {
            if !ref_comp.covers(Scope::World, &IRect::from_size(x, y, 1, 1)) {
                corner = Some((x, y));
                break 'find;
            }
        }
    }
    let (cx, cy) = corner.expect("the world layout has a point outside every map");
    let p = img.get_pixel(cx as u32, cy as u32);
    assert_eq!(p[3], 0, "({}, {}), which Compositor::covers says no map reaches, is transparent, got {:?}", cx, cy, p.0);
}

// ---------------------------------------------------------------------------
// the dialog flow
// ---------------------------------------------------------------------------

#[test]
fn the_dialog_flow_delivers_exported_and_the_file_exists() {
    let mut h = Harness::new("yellow-pinsir-lv10brock.json");
    h.frame(vec![]);
    h.wait_ready();
    let pack = h.view.pack().unwrap().clone();
    let gym = pack.map_by_const("PEWTER_GYM").unwrap();
    h.view.navigate_to(gym.id, false);
    h.frame(vec![]);
    h.frame(vec![]);

    assert!(!h.view.export.is_open());
    h.view.export.request_open();

    // frames until the modal is genuinely on screen (its title, not the
    // toolbar's own plain "Export" button, which is also drawn every frame)
    let t = Instant::now();
    loop {
        h.frame(vec![]);
        if h.texts.iter().any(|s| s == "Export Map Image") {
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(5), "the export dialog never appeared; texts: {:?}", h.texts);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(h.view.export.is_open());

    // drive the job as the dialog's own Export button would
    // (`start_export_test`), rather than locating that button by its drawn
    // text, which the toolbar's "Export" button would also match
    let spec = spec_for(&h.view, ExportRegion::Map, 1, false).expect("a spec resolves for the gym");
    let out_path = scratch_dir("dialog").join("dialog_export.png");
    assert!(start_export_test(&mut h.view, &h.theme, spec, Some(out_path.clone())), "the job started");

    let t = Instant::now();
    let mut exported = None;
    while exported.is_none() {
        for a in h.frame(vec![]) {
            if let MapAction::Exported(p) = a {
                exported = Some(p);
            }
        }
        assert!(t.elapsed() < Duration::from_secs(20), "MapAction::Exported did not arrive in time");
        std::thread::sleep(Duration::from_millis(20));
    }
    let exported = exported.unwrap();
    assert_eq!(exported, out_path);
    assert!(exported.exists(), "the exported file exists");
    assert!(!h.view.export.is_open(), "the dialog closes once the export lands");
}

// ---------------------------------------------------------------------------
// the smoke run
// ---------------------------------------------------------------------------

#[test]
fn xpr_smoke_export_parses_map() {
    assert_eq!(smoke_export_kind("map"), Some(ShotKind::Map));
}
