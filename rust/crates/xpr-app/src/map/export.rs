//! The map image export (`docs/rust_port/design/world_map/
//! GRAPHICS_TOOLS_PLAN.md` WP-B, gaps G1 / G2): a modal the map draws itself
//! (so it works docked and in the map's own window) that renders the view /
//! selection / map / world at 1-8x through `xpr_map::export`, draws the
//! toggled overlays on top through the app's software rasteriser, and
//! writes a PNG to the images dir or copies the image to the clipboard.
//! Runs on a worker thread with progress and cancel.
//!
//! The base image comes from `xpr_map::export::render` (WP-A: banded
//! compositing, transparency, the size bound). The overlays (markers, grid,
//! map names, route path, the selection outline) are the exact same free
//! functions the live viewer draws with (`layers::draw_markers`,
//! `overlay::draw_grid` / `draw_map_labels`, `route_path::draw`,
//! `tools::ToolState::draw`), called once with an *export* camera
//! (`Camera::default()` recentred on the region at `zoom = scale`) into an
//! offscreen egui context (`screenshot::Offscreen`) sized to the output,
//! then rasterised band by band and alpha-blended onto the base pixmap
//! (`Canvas::blend_over_rgba8`) so the float canvas never holds the whole
//! image at once (plan §3.4 "B").

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::Duration;

use egui::{Pos2, Rect, Ui, Vec2};
use xpr_core::Config;
use xpr_map::export as mexport;
use xpr_map::{geom, Compositor, IRect, MapPack, Pixmap, RenderOpts, Scope};
use xpr_ui_kit::theme::Theme;
use xpr_ui_kit::widgets;

use crate::controller::MainController;
use crate::dialogs;
use crate::screenshot::Offscreen;

use super::camera::{self, Camera};
use super::layers;
use super::overlay::{self, OverlayCtx};
use super::route_path;
use super::sprites::SpriteAtlas;
use super::state::RouteMapState;
use super::tools::{self, Selection};
use super::{MapAction, MapView, Toggles};

/// What an export covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ExportRegion {
    /// the viewport as it is on screen
    #[default]
    View,
    /// the marquee selection (`tools.selection`)
    Selection,
    /// the map of the current `Scope::Map`
    Map,
    /// the whole scope rectangle (the world, or the map on its own)
    World,
}

fn region_label(r: ExportRegion) -> &'static str {
    match r {
        ExportRegion::View => "View",
        ExportRegion::Selection => "Selection",
        ExportRegion::Map => "This Map",
        ExportRegion::World => "Whole World",
    }
}

/// Whether `r` is offered right now ("Selection when one exists", "This
/// map when in a map scope"; View and World are always available).
fn is_region_available(view: &MapView, r: ExportRegion) -> bool {
    match r {
        ExportRegion::View | ExportRegion::World => true,
        ExportRegion::Selection => view.tools.selection.is_some(),
        ExportRegion::Map => matches!(view.scope, Scope::Map(_)),
    }
}

/// `request_open`'s auto region: Selection when one exists, View otherwise.
fn default_region(view: &MapView) -> ExportRegion {
    if view.tools.selection.is_some() {
        ExportRegion::Selection
    } else {
        ExportRegion::View
    }
}

/// The scope and world-pixel rectangle a region resolves to right now.
/// `World` always means `Scope::World` (the whole game map), independent of
/// the viewer's current scope, so it stays useful while looking at a single
/// map; `Map`/`Selection` fall back to a sane rect if asked for when they
/// are not actually available (callers normally check `is_region_available`
/// first, but the export job itself must never panic on a stale region).
fn region_rect(view: &MapView, pack: &MapPack, r: ExportRegion) -> (Scope, IRect) {
    match r {
        ExportRegion::View => {
            let scope = view.scope;
            (scope, view.camera.visible_world(view.last_vp).intersect(&geom::scope_rect(pack, scope)))
        }
        ExportRegion::Selection => (view.scope, view.tools.selection.map(|s| s.rect).unwrap_or_else(|| geom::scope_rect(pack, view.scope))),
        ExportRegion::Map => (view.scope, geom::scope_rect(pack, view.scope)),
        ExportRegion::World => (Scope::World, geom::scope_rect(pack, Scope::World)),
    }
}

/// Everything a render needs, independent of the dialog: shared by the
/// async job, the blocking path (`run_export_blocking`) and the test hook
/// (`start_export_test`).
#[derive(Clone, Copy, Debug)]
pub struct ExportSpec {
    pub scope: Scope,
    /// world px of `scope`, clipped to it when rendered
    pub rect: IRect,
    /// output px per world px, `1..=xpr_map::export::MAX_SCALE`
    pub scale: u32,
    /// gen 2 only
    pub night: bool,
    /// the full toggle set (object kinds, sprites, grid, labels, path);
    /// `navigator` / `follow` are viewer-only and ignored here
    pub toggles: Toggles,
    /// drawn as the selection-outline overlay when `Some`
    pub selection: Option<Selection>,
    /// leave pixels no map covers transparent instead of the world background
    pub transparent: bool,
}

/// What a finished job produced.
#[derive(Debug)]
pub enum ExportOutcome {
    Exported(PathBuf),
    Copied { width: usize, height: usize, rgba: Vec<u8> },
}

enum JobTarget {
    File(PathBuf),
    Clipboard,
}

enum JobMsg {
    Progress(f32),
    Done(Result<ExportOutcome, String>),
}

struct Job {
    rx: Receiver<JobMsg>,
    cancel: Arc<AtomicBool>,
    progress: f32,
}

pub struct ExportUi {
    open: bool,
    /// tracks the open->closed edge so toggle-derived settings are
    /// re-primed only when the dialog is freshly opened, not every frame
    was_open: bool,
    /// the region the dialog opens with (`None`: Selection when one exists, else View)
    region: Option<ExportRegion>,
    /// a copy-to-clipboard request without the dialog (current layer toggles, 1x)
    copy: Option<ExportRegion>,
    // remembered settings for the session (scale / transparent / the
    // selection-outline flag have no view-state counterpart to prefill
    // from, so they simply persist as ExportUi's own fields; the layer
    // checkboxes and night are re-synced from `view.toggles` / `view.night`
    // each time the dialog is freshly opened, see `export_ui`)
    scale: u32,
    night: bool,
    markers: bool,
    sprites: bool,
    grid: bool,
    labels: bool,
    path: bool,
    selection_outline: bool,
    transparent: bool,
    job: Option<Job>,
    /// the last job's failure, shown in the dialog until the next attempt
    error: Option<String>,
}

impl Default for ExportUi {
    fn default() -> Self {
        ExportUi {
            open: false,
            was_open: false,
            region: None,
            copy: None,
            scale: 1,
            night: false,
            markers: true,
            sprites: true,
            grid: false,
            labels: true,
            path: false,
            selection_outline: true,
            transparent: false,
            job: None,
            error: None,
        }
    }
}

impl ExportUi {
    /// Open the dialog on the next frame (toolbar Export, the Map menu):
    /// the Selection region when a selection exists, the View otherwise.
    pub fn request_open(&mut self) {
        self.open = true;
        self.region = None;
        self.error = None;
    }

    /// Open the dialog preset to `region` (the selection's Export button).
    pub fn request_open_with(&mut self, region: ExportRegion) {
        self.open = true;
        self.region = Some(region);
        self.error = None;
    }

    /// Copy `region` to the clipboard straight away, with the current layer
    /// toggles at 1x (the selection's Copy button, the Map menu).
    pub fn request_copy(&mut self, region: ExportRegion) {
        self.copy = Some(region);
    }

    pub fn is_open(&self) -> bool {
        self.open
    }
}

// ---------------------------------------------------------------------------
// dialog + job polling (called every frame from `MapView::ui`)
// ---------------------------------------------------------------------------

/// Draw the dialog / the running job's progress and poll the worker. The
/// outcome goes to the window as `MapAction::Exported` / `ExportFailed` /
/// `CopiedImage`.
pub(super) fn export_ui(view: &mut MapView, ui: &mut Ui, theme: &Theme, cfg: &Config, ctrl: &MainController, actions: &mut Vec<MapAction>) {
    // prefill the toggle-derived settings the first time the dialog opens
    // this session (plan §3.4 "B": "prefilled from view.toggles" /
    // "prefilled from view.night", plus "remember the last settings ... for
    // the session" for scale / transparent / the selection-outline flag,
    // which simply keep whatever they were last set to)
    let just_opened = view.export.open && !view.export.was_open;
    view.export.was_open = view.export.open;
    if just_opened {
        view.export.night = view.night;
        let t = view.toggles;
        view.export.markers = t.trainers || t.items || t.hidden || t.warps || t.signs || t.berries || t.npcs;
        view.export.sprites = t.sprites;
        view.export.grid = t.grid;
        view.export.labels = t.labels;
        view.export.path = t.path;
        view.export.error = None;
    }

    // a quiet copy request (no dialog): start a job right away, 1x, the
    // current layer toggles (`request_copy`'s contract)
    if let Some(region) = view.export.copy.take() {
        if view.export.job.is_none() {
            if let Some(loaded) = view.loaded.as_ref() {
                let pack = loaded.pack.clone();
                let comp = loaded.comp.clone();
                let (scope, rect) = region_rect(view, &pack, region);
                let spec = ExportSpec { scope, rect, scale: 1, night: view.night, toggles: view.toggles, selection: view.tools.selection, transparent: false };
                if mexport::check(rect, spec.scale).is_ok() {
                    let job = spawn_job(pack, comp, theme.clone(), view.state.clone(), spec, JobTarget::Clipboard);
                    view.export.job = Some(job);
                } else {
                    actions.push(MapAction::ExportFailed("nothing to copy".to_string()));
                }
            } else {
                actions.push(MapAction::ExportFailed("no map data loaded".to_string()));
            }
        }
    }

    // poll a running job (base + overlay bands on a worker thread)
    let ctx = ui.ctx().clone();
    let poll = if let Some(job) = view.export.job.as_mut() {
        let mut outcome = None;
        loop {
            match job.rx.try_recv() {
                Ok(JobMsg::Progress(f)) => job.progress = f,
                Ok(JobMsg::Done(r)) => {
                    outcome = Some(r);
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    outcome = Some(Err("the export worker stopped unexpectedly".to_string()));
                    break;
                }
            }
        }
        outcome.map(|r| (r, job.cancel.load(Ordering::Relaxed)))
    } else {
        None
    };
    if let Some((result, cancelled)) = poll {
        view.export.job = None;
        match result {
            Ok(ExportOutcome::Exported(path)) => {
                view.export.open = false;
                view.export.error = None;
                actions.push(MapAction::Exported(path));
            }
            Ok(ExportOutcome::Copied { width, height, rgba }) => {
                view.export.open = false;
                view.export.error = None;
                if width > 0 && height > 0 && rgba.len() == width * height * 4 {
                    ctx.copy_image(egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba));
                    actions.push(MapAction::CopiedImage);
                } else {
                    actions.push(MapAction::ExportFailed("nothing to copy".to_string()));
                }
            }
            Err(_) if cancelled => {
                // the user asked to stop; nothing to report
            }
            Err(e) => {
                if view.export.open {
                    // the dialog is still up: show the error there and let
                    // the user adjust settings and retry
                    view.export.error = Some(e);
                } else {
                    // a quiet `request_copy` job: the only way to report it
                    actions.push(MapAction::ExportFailed(e));
                }
            }
        }
    }

    if !view.export.open {
        return;
    }
    draw_dialog(view, ui, theme, cfg, ctrl);
    if view.export.job.is_some() {
        ctx.request_repaint_after(Duration::from_millis(50));
    }
}

/// The modal body: settings while idle, a progress bar + Cancel while a job
/// runs. Reads/writes `view.export`'s remembered fields directly from the
/// widgets (so they persist across dialog opens without a separate "apply"
/// step) and starts a job on Export / Copy.
fn draw_dialog(view: &mut MapView, ui: &mut Ui, theme: &Theme, cfg: &Config, ctrl: &MainController) {
    let ctx = ui.ctx().clone();
    let Some(loaded) = view.loaded.as_ref() else {
        // the Export button is disabled until the pack loads, so this is
        // only reachable if the pack somehow unloads mid-dialog
        view.export.open = false;
        return;
    };
    let pack = loaded.pack.clone();
    let comp = loaded.comp.clone();

    if view.export.job.is_some() {
        let progress = view.export.job.as_ref().map(|j| j.progress).unwrap_or(0.0);
        let cancel = view.export.job.as_ref().map(|j| j.cancel.clone());
        let mut stop = false;
        dialogs::modal(&ctx, theme, "xpr_map_export", "Export Map Image", 340.0, |ui| {
            ui.vertical(|ui| {
                widgets::label(ui, theme, "Rendering…");
                ui.add_space(8.0);
                widgets::progress_bar(ui, 300.0, progress, theme.accent, theme.bg_darker);
                ui.add_space(10.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::StyledButton::new(theme, "Cancel").show(ui).clicked() {
                        stop = true;
                    }
                });
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                stop = true;
            }
        });
        if stop {
            if let Some(cancel) = cancel {
                cancel.store(true, Ordering::Relaxed);
            }
        }
        return;
    }

    let requested = view.export.region.unwrap_or_else(|| default_region(view));
    let region = if is_region_available(view, requested) { requested } else { default_region(view) };
    let (scope, rect) = region_rect(view, &pack, region);
    let night_capable = pack.gen == 2;
    let scale_max = mexport::MAX_SCALE;

    let mut region_pick: Option<ExportRegion> = None;
    let mut do_export = false;
    let mut do_copy = false;
    let mut do_cancel = false;

    dialogs::modal(&ctx, theme, "xpr_map_export", "Export Map Image", 380.0, |ui| {
        widgets::label_bold(ui, theme, "Region");
        ui.add_space(2.0);
        ui.horizontal_wrapped(|ui| {
            for r in [ExportRegion::View, ExportRegion::Selection, ExportRegion::Map, ExportRegion::World] {
                if !is_region_available(view, r) {
                    continue;
                }
                if widgets::StyledButton::new(theme, region_label(r)).checked(region == r).show(ui).clicked() {
                    region_pick = Some(r);
                }
            }
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            widgets::label_bold(ui, theme, "Scale");
            ui.add_space(6.0);
            let delta = widgets::stepper_group(ui, theme, ui.id().with("xpr_map_export_scale"), 22.0, view.export.scale > 1, view.export.scale < scale_max, ("Smaller", "Larger"), |ui| {
                widgets::label_font(ui, format!("{}×", view.export.scale), theme.body_bold(), theme.text_strong());
            });
            if delta != 0 {
                view.export.scale = (view.export.scale as i32 + delta).clamp(1, scale_max as i32) as u32;
            }
            if night_capable {
                ui.add_space(14.0);
                widgets::checkbox(ui, theme, &mut view.export.night, "Night", true);
            }
        });
        ui.add_space(8.0);
        widgets::label_bold(ui, theme, "Layers");
        ui.add_space(2.0);
        ui.horizontal_wrapped(|ui| {
            widgets::checkbox(ui, theme, &mut view.export.markers, "Markers", true);
            widgets::checkbox(ui, theme, &mut view.export.sprites, "Sprites", true);
            widgets::checkbox(ui, theme, &mut view.export.grid, "Grid", true);
            widgets::checkbox(ui, theme, &mut view.export.labels, "Map names", true);
            widgets::checkbox(ui, theme, &mut view.export.path, "Route path", true);
            let sel_available = view.tools.selection.is_some();
            widgets::checkbox(ui, theme, &mut view.export.selection_outline, "Selection outline", sel_available);
        });
        ui.add_space(8.0);
        widgets::checkbox(ui, theme, &mut view.export.transparent, "Transparent background", true);
        ui.add_space(10.0);

        let (w, h) = mexport::output_size(rect, view.export.scale);
        let pixels = mexport::output_pixels(rect, view.export.scale);
        let check_result = mexport::check(rect, view.export.scale);
        let ok = check_result.is_ok();
        let size_line = match check_result {
            Ok(()) => format!("{} × {} px · {:.1} MB", w, h, pixels as f64 * 4.0 / 1_048_576.0),
            Err(e) => e.to_string(),
        };
        widgets::label_colored(ui, theme, size_line, if ok { theme.secondary } else { theme.failure });
        if let Some(err) = view.export.error.clone() {
            ui.add_space(4.0);
            widgets::label_colored(ui, theme, err, theme.failure);
        }
        ui.add_space(10.0);

        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::StyledButton::new(theme, "Export").enabled(ok).show(ui).clicked() || (enter && ok) {
                do_export = true;
            }
            if widgets::StyledButton::new(theme, "Copy").enabled(ok).show(ui).clicked() {
                do_copy = true;
            }
            if widgets::StyledButton::new(theme, "Cancel").show(ui).clicked() || escape {
                do_cancel = true;
            }
        });
    });

    if let Some(r) = region_pick {
        view.export.region = Some(r);
    }
    if do_cancel {
        view.export.open = false;
        view.export.error = None;
        return;
    }
    if do_export || do_copy {
        let spec = ExportSpec {
            scope,
            rect,
            scale: view.export.scale,
            night: view.export.night,
            toggles: build_job_toggles(view.toggles, &view.export),
            selection: if view.export.selection_outline { view.tools.selection } else { None },
            transparent: view.export.transparent,
        };
        view.export.error = None;
        let target = if do_export { JobTarget::File(ctrl.screenshot_path(&cfg.get_images_dir(), "map")) } else { JobTarget::Clipboard };
        let job = spawn_job(pack, comp, theme.clone(), view.state.clone(), spec, target);
        view.export.job = Some(job);
    }
}

/// The dialog's "Markers" checkbox is a single on/off for every object kind
/// at once; when on it keeps whichever mix the toolbar's own T/I/?/W/S/B/N
/// chips are currently set to.
fn build_job_toggles(base: Toggles, ui: &ExportUi) -> Toggles {
    let mut t = base;
    if !ui.markers {
        t.trainers = false;
        t.items = false;
        t.hidden = false;
        t.warps = false;
        t.signs = false;
        t.berries = false;
        t.npcs = false;
    }
    t.sprites = ui.sprites;
    t.grid = ui.grid;
    t.labels = ui.labels;
    t.path = ui.path;
    t
}

// ---------------------------------------------------------------------------
// the worker job
// ---------------------------------------------------------------------------

fn spawn_job(pack: Arc<MapPack>, comp: Arc<Compositor>, theme: Theme, state: RouteMapState, spec: ExportSpec, target: JobTarget) -> Job {
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_bg = cancel.clone();
    let (tx, rx) = mpsc::channel::<JobMsg>();
    let tx_progress = tx.clone();
    std::thread::spawn(move || {
        let mut progress_fn = move |f: f32| -> bool {
            let _ = tx_progress.send(JobMsg::Progress(f));
            !cancel_bg.load(Ordering::Relaxed)
        };
        let result = render_full(&pack, &comp, &theme, &spec, &state, &mut progress_fn).and_then(|pixmap| finish(pixmap, &target));
        let _ = tx.send(JobMsg::Done(result));
    });
    Job { rx, cancel, progress: 0.0 }
}

/// Encode + write (Export) or hand back the pixel data (Copy).
fn finish(pixmap: Pixmap, target: &JobTarget) -> Result<ExportOutcome, String> {
    match target {
        JobTarget::File(path) => {
            let bytes = pixmap.to_png()?;
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(path, &bytes).map_err(|e| format!("couldn't write {}: {}", path.display(), e))?;
            Ok(ExportOutcome::Exported(path.clone()))
        }
        JobTarget::Clipboard => Ok(ExportOutcome::Copied { width: pixmap.w, height: pixmap.h, rgba: pixmap.data }),
    }
}

/// Output rows per overlay band (`Offscreen::rasterize` is called once per
/// band so the f32 canvas stays small regardless of the export's size).
const OVERLAY_BAND_PX: i32 = 512;

/// Render `spec` to a pixmap: the base through `xpr_map::export::render`
/// (already banded and size-checked by WP-A), then -- when any overlay
/// layer would actually draw something -- the toggled overlays through an
/// offscreen egui context with an export camera, rasterised and alpha-
/// blended onto the base band by band. `progress` is threaded through both
/// phases (0.0 before anything, 1.0 on success) and stops the render as
/// soon as it returns false.
fn render_full(pack: &MapPack, comp: &Compositor, theme: &Theme, spec: &ExportSpec, state: &RouteMapState, progress: &mut dyn FnMut(f32) -> bool) -> Result<Pixmap, String> {
    let needs_overlay = spec.selection.is_some()
        || spec.toggles.grid
        || spec.toggles.labels
        || spec.toggles.path
        || spec.toggles.trainers
        || spec.toggles.items
        || spec.toggles.hidden
        || spec.toggles.warps
        || spec.toggles.signs
        || spec.toggles.berries
        || spec.toggles.npcs;
    // the base render gets the whole progress range when there is nothing
    // else to do, else the first 60%; the overlay bands get the rest
    let base_weight = if needs_overlay { 0.6 } else { 1.0 };

    let req = mexport::ExportRequest { scope: spec.scope, rect: spec.rect, scale: spec.scale, opts: RenderOpts { night: spec.night }, transparent: spec.transparent };
    let mut out = mexport::render(comp, &req, &mut |f| progress(f * base_weight)).map_err(|e| e.to_string())?;
    if !needs_overlay {
        if !progress(1.0) {
            return Err(mexport::ExportError::Cancelled.to_string());
        }
        return Ok(out);
    }

    // the same clipping `render` applied internally, so the export camera
    // and the band loop below line up exactly with `out`'s dimensions
    let rect = req.clipped_rect(comp);
    let (out_w, out_h) = mexport::output_size(rect, spec.scale);
    if out_w == 0 || out_h == 0 {
        return Ok(out);
    }

    let mut cam = Camera::default();
    cam.center = Vec2::new(rect.x0 as f32 + rect.width() as f32 / 2.0, rect.y0 as f32 + rect.height() as f32 / 2.0);
    cam.zoom = spec.scale as f32;
    cam.min_zoom = camera::MIN_ZOOM_ABS;
    let vp = Rect::from_min_size(Pos2::ZERO, Vec2::new(out_w as f32, out_h as f32));
    let oc_scope = spec.scope;

    let mut atlas = SpriteAtlas::default();
    // `ToolState` has private fields of its own (WP-C); its `selection` is
    // public, so build a default and set that one field rather than a
    // struct literal.
    let mut tool_state = tools::ToolState::default();
    tool_state.selection = spec.selection;
    let mut off = Offscreen::new(theme, 1.0, 8192);
    // one full-scene tessellation; `rasterize` below crops it band by band,
    // so only the (small) vector mesh data lives for the whole image, never
    // a full-resolution f32 canvas (plan §3.4 "B")
    let prims = off.render(vp.size(), 2, |ctx| {
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
            let painter = ui.painter_at(vp);
            let oc = OverlayCtx { theme, pack, scope: oc_scope, cam: &cam, vp, ppp: 1.0 };
            overlay::draw_grid(&painter, &oc, &spec.toggles);
            let mc = layers::MarkerCtx { theme, toggles: &spec.toggles, state, hover: None, selected: None, night: spec.night, ctx };
            layers::draw_markers(&painter, vp, &cam, pack, oc_scope, &mc, &mut atlas);
            overlay::draw_map_labels(&painter, &oc, &spec.toggles);
            route_path::draw(&painter, &oc, state, &spec.toggles);
            tool_state.draw(&painter, &oc);
        });
    });

    let total_bands = ((out_h as i32) + OVERLAY_BAND_PX - 1) / OVERLAY_BAND_PX;
    let mut y = 0i32;
    let mut band_i = 0i32;
    while y < out_h as i32 {
        if !progress(base_weight + (band_i as f32 / total_bands.max(1) as f32) * (1.0 - base_weight)) {
            return Err(mexport::ExportError::Cancelled.to_string());
        }
        let band_h = OVERLAY_BAND_PX.min(out_h as i32 - y);
        let band_rect = Rect::from_min_size(Pos2::new(0.0, y as f32), Vec2::new(out_w as f32, band_h as f32));
        let canvas = off.rasterize(&prims, band_rect)?;
        canvas.blend_over_rgba8(&mut out.data, out.w, 0, y as usize);
        y += band_h;
        band_i += 1;
    }
    if !progress(1.0) {
        return Err(mexport::ExportError::Cancelled.to_string());
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// synchronous path (the smoke run, the example, tests) + app.rs helpers
// ---------------------------------------------------------------------------

/// A synchronous export with no worker thread and no dialog: used by the
/// smoke run (`ShotKind::Map` in `app.rs::export_shot`, `XPR_SMOKE_EXPORT=
/// map`), `examples/map_export_png.rs` and `tests/map_export.rs`. `out`:
/// `Some(path)` writes a PNG and returns `Exported`; `None` skips the file
/// and returns `Copied` with the finished pixel data.
pub fn run_export_blocking(view: &MapView, theme: &Theme, spec: &ExportSpec, out: Option<&Path>) -> Result<ExportOutcome, String> {
    let loaded = view.loaded.as_ref().ok_or_else(|| "no map data loaded".to_string())?;
    let pixmap = render_full(&loaded.pack, &loaded.comp, theme, spec, &view.state, &mut |_| true)?;
    match out {
        Some(path) => finish(pixmap, &JobTarget::File(path.to_path_buf())),
        None => finish(pixmap, &JobTarget::Clipboard),
    }
}

/// Resolve `region` against `view`'s current state into a spec at `scale`
/// with the view's current toggles. `app.rs` cannot see `MapView`'s private
/// fields (camera, scope, tools.selection's world rect, ...) itself, so
/// `ShotKind::Map`'s "the current view at 1x with the current toggles"
/// goes through this instead.
pub fn spec_for(view: &MapView, region: ExportRegion, scale: u32, transparent: bool) -> Option<ExportSpec> {
    let pack = view.pack()?.clone();
    let region = if is_region_available(view, region) { region } else { default_region(view) };
    let (scope, rect) = region_rect(view, &pack, region);
    Some(ExportSpec { scope, rect, scale, night: view.night, toggles: view.toggles, selection: view.tools.selection, transparent })
}

/// Test-only: start a job as if the dialog's Export (file) / Copy
/// (clipboard) button had just been clicked, bypassing the click itself.
/// `tests/map_export.rs` uses this rather than locating the button by its
/// drawn text, which the toolbar's own always-on-screen "Export" button
/// would also match.
pub fn start_export_test(view: &mut MapView, theme: &Theme, spec: ExportSpec, out: Option<PathBuf>) -> bool {
    let Some(loaded) = view.loaded.as_ref() else { return false };
    let pack = loaded.pack.clone();
    let comp = loaded.comp.clone();
    let state = view.state.clone();
    let target = match out {
        Some(p) => JobTarget::File(p),
        None => JobTarget::Clipboard,
    };
    let job = spawn_job(pack, comp, theme.clone(), state, spec, target);
    view.export.job = Some(job);
    view.export.open = true;
    true
}
