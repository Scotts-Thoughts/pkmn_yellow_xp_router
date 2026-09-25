# Map graphics tools — status review, gaps and implementation plan

Date: 2026-09-25. Companion to `SPEC.md` (the world-map integration).
Rust/egui only; the Python app gets none of this.

## 1. Where the "photoshop-esque" tooling stands

The router's graphical surface is the world-map viewer
(`rust/crates/xpr-app/src/map/`, data and compositing in `xpr-map`) plus the
PNG export pipeline (`xpr-app/src/screenshot.rs`: widgets drawn into an
offscreen egui context and rasterised in software to a transparent PNG).
Status per capability, with the Photoshop analogue in brackets:

| Capability | State | Where |
|---|---|---|
| Canvas [hand tool]: drag pan, wheel / pinch zoom about the cursor, arrow-key pan, `+`/`-`/`0`, Fit, eased go-to | done | `map/camera.rs`, `map/mod.rs::viewport` |
| Zoom readout with `−` / `+` / Fit | done; no presets, no 1:1, no zoom menu | `map/mod.rs::toolbar` |
| Layers panel [layer visibility]: trainers, items, hidden items, warps, signs, berries, NPCs, sprites-vs-circles | done, persisted in `map_toggles` | `Toggles` |
| Adjustment [colour variant]: gen 2 night palettes | done | `night`, `RenderOpts` |
| Tiled LOD rendering with a texture LRU, worker-thread compositing | done | `map/chunks.rs`, `map/layers.rs`, `xpr-map/src/compose.rs` |
| Info cards, add-to-route, show-on-map, focus banner, map search | done | `map/cards.rs`, `map/list.rs`, `map/state.rs` |
| Headless rendering to PNG | examples only | `xpr-app/examples/map_card_png.rs`, `xpr-map/examples/render_world.rs` |
| PNG export from the app (view / region / map / world at 1–8× with layers) | **missing** (SPEC Phase 5 step 4; pokemap's web viewer has it) | — |
| Selection [marquee] and crop-to-selection export | **missing** | — |
| Copy image to clipboard | **missing** (egui 0.33 `Context::copy_image` reaches arboard through eframe; nothing to add to Cargo) | — |
| Grid / guides overlay [pixel grid] | **missing** (the status strip shows the step under the cursor only) | — |
| Measure tool [ruler] | **missing** | — |
| Navigator [navigator panel] | **missing** | — |
| Map name labels layer | **missing** | — |
| Route path overlay (numbered route order) | **missing** (SPEC §9) | — |
| Item / trainer finder | **missing** (SPEC Phase 5 step 3) | — |
| Keyboard marker navigation, follow selection | **missing** (SPEC Phase 5 step 5) | — |
| Pinned per-event anchors [move tool on markers] | **missing**; needs a route-file key (SPEC §3.9) | — |
| Widget screenshots (event list, battle summary, matchups, run summary, compare) | done; file only, no clipboard | `screenshot.rs`, `app.rs::export_shot` |

`cargo test -p xpr-map -p xpr-app --no-run` was green on this Mac
(rustc 1.96) when this plan was written; nothing is committed.

## 2. Functionality gaps for graphical manipulation

| # | Gap | Notes |
|---|---|---|
| G1 | Export the map as PNG: current view / marquee selection / one map / whole world; 1–8× nearest-neighbour; markers as toggled (sprites or circles, routed state), grid, labels, route path; night; transparent outside the maps; size readout and an upper bound; worker thread with progress and cancel; file in the images dir with the router's naming; toast with Open Folder; `XPR_SMOKE_EXPORT=map` | pokemap: `index.html` export dialog, `main.js:1735-1955` |
| G2 | Copy the current view or the selection to the clipboard as an image | `egui::Context::copy_image` |
| G3 | Marquee selection tool: drag to select, snapped to steps; size readout; Esc clears; zoom to selection; export / copy the selection; add all trainers inside it to the route | |
| G4 | Measure tool: drag from A to B, Δx / Δy / Manhattan steps | routing wants step counts |
| G5 | Grid overlay: steps at high zoom, blocks lower, map boundaries in the world | a layer toggle |
| G6 | Map name labels layer (readable when zoomed out) | a layer toggle |
| G7 | Navigator minimap with the viewport rectangle; click / drag to move | the coarsest LOD chunk is one texture of the whole scope |
| G8 | Zoom presets and 1:1 through a menu on the zoom readout; `1` key = 100 % | |
| G9 | Trip path overlay: the selected folder's events (or the folder around the selected event) as numbered discs with straight segments between consecutive anchored events in the same scope; never the whole route (SPEC D15 / §13, decided 2026-09-25) | a layer toggle |
| G10 | Find trainers and items from the search box (with the maps), cycling anchors through the focus banner | the item finder of SPEC Phase 5 |
| G11 | Keyboard marker navigation: Tab / Shift+Tab through the visible markers, Enter opens the card | |
| G12 | Follow selection: the camera follows the selected route event | |
| G13 | Tool shortcuts (H pan, M marquee, R ruler, Space held = temporary pan) that respect text focus and modal dialogs | |
| G14 | Pinned anchors (drag a marker to place a trainer the game code cannot place) | **deferred**: route-file change, SPEC §3.9 |
| G15 | Clipboard copy for the widget screenshots | **deferred** follow-up; same `copy_image` path |

## 3. Plan

### 3.1 Rules

- No egui in `xpr-map`. Every new viewer feature lives in its own file under
  `xpr-app/src/map/`; `mod.rs` only holds the hooks the scaffold placed
  (module declarations, `Toggles` fields, `MapView` fields, `MapAction`
  variants, one call per hook). Accessors a test needs go in an
  `impl MapView` block inside the feature's own file.
- Every raw input read (`ui.input`, `key_pressed`) on the viewport asks
  `xpr_ui_kit::modal::behind_modal(ui)` first and stands down; text focus
  (`ctx.memory(|m| m.focused().is_some())`) disables single-letter keys.
- Layer toggles are fields of `Toggles`, persisted through `map_toggles`.
- Overlays are free functions of `(painter, &OverlayCtx, …)` so the export
  path can draw them with an export camera into the offscreen rasteriser.
- Tests are headless, following `xpr-app/tests/map_view.rs` (an
  `egui::Context` fed synthetic events) and `xpr-map/tests/pack.rs`.
- Never run `cargo fmt` (the code is not rustfmt-formatted) or `cargo clean`
  (7.5 GB of build cache, 54 GB free); never edit `map_data/`; do not commit.
- Style: `//!` module docs that say what and why, comments that cite the
  SPEC section or the pokemap source being ported, long lines are fine.

### 3.2 Work packages

| WP | Owner | Files (owned) | Gaps |
|---|---|---|---|
| A | agent | `xpr-map/src/export.rs`, `xpr-map/tests/export.rs` | G1 core: banded rendering, transparency, progress / cancel, size bound |
| B | agent | `xpr-app/src/map/export.rs`, `xpr-app/tests/map_export.rs`, small edits in `app.rs` (Map menu, `ShotKind::Map`, `XPR_SMOKE_EXPORT=map`, toast), `xpr-core/src/config.rs` (shortcut `export_map`) | G1 UI + overlays, G2 |
| C | agent | `xpr-app/src/map/tools.rs`, `xpr-app/src/map/overlay.rs`, `xpr-app/tests/map_tools.rs` | G3, G4, G5, G6, G13 |
| D | agent | `xpr-app/src/map/navigator.rs`, `xpr-app/src/map/camera.rs` (presets), `xpr-app/tests/map_navigator.rs` | G7, G8 |
| E | agent | `xpr-app/src/map/finder.rs`, `xpr-app/src/map/route_path.rs`, `xpr-app/src/map/keynav.rs`, `xpr-app/src/map/state.rs`, `xpr-app/src/map/list.rs`, `xpr-app/tests/map_navigation.rs` | G9, G10, G11, G12 |
| F | lead | `SPEC.md` §12, `rust/TESTING.md` §17, `rust/PORT_STATUS.md`, verification | docs, review, smoke run |

A–E run in parallel in the same working tree (the map integration is
uncommitted, so worktrees cannot be used). B compiles against the scaffold's
minimal `xpr_map::export` and picks up A's real implementation without code
changes; the contract is the signatures below.

### 3.3 Contracts placed by the scaffold

`xpr_map::export` (A implements, B consumes):

```rust
pub const MAX_SCALE: u32 = 8;
pub const MAX_PIXELS: u64 = 1 << 28;              // 1 GiB of RGBA
pub struct ExportRequest { pub scope: Scope, pub rect: IRect, pub scale: u32, pub opts: RenderOpts, pub transparent: bool }
pub enum ExportError { Empty, TooLarge { pixels: u64, max: u64 }, BadScale(u32), Cancelled }
pub fn output_size(rect: IRect, scale: u32) -> (u32, u32);
pub fn output_pixels(rect: IRect, scale: u32) -> u64;
pub fn check(rect: IRect, scale: u32) -> Result<(), ExportError>;
pub fn upscale_into(dst: &mut Pixmap, dst_x: usize, dst_y: usize, src: &Pixmap, factor: usize);
/// `progress(fraction)` after every band; return false to cancel.
pub fn render(comp: &Compositor, req: &ExportRequest, progress: &mut dyn FnMut(f32) -> bool) -> Result<Pixmap, ExportError>;
```

`xpr-app/src/map/` hooks (each stub is a no-op until its WP fills it):

```rust
// overlay.rs (C)                       — also used by B's export with an export camera
pub struct OverlayCtx<'a> { pub theme: &'a Theme, pub pack: &'a MapPack, pub scope: Scope, pub cam: &'a Camera, pub vp: Rect, pub ppp: f32 }
pub fn draw_grid(painter: &egui::Painter, oc: &OverlayCtx, toggles: &Toggles);
pub fn draw_map_labels(painter: &egui::Painter, oc: &OverlayCtx, toggles: &Toggles);
// tools.rs (C)
pub enum Tool { Pan, Select, Measure }
pub struct Selection { pub rect: IRect }            // world px, step-aligned
pub struct ToolState { pub tool: Tool, pub selection: Option<Selection>, .. }
impl ToolState {
    pub fn toolbar(&mut self, ui: &mut Ui, theme: &Theme);
    pub fn handle_keys(&mut self, ctx: &egui::Context);
    pub fn handle_input(view: &mut MapView, ui: &Ui, resp: &egui::Response, vp: Rect) -> bool;  // true = consumed (no pan / click)
    pub fn draw(&self, painter: &egui::Painter, oc: &OverlayCtx);
    pub fn status_text(&self, pack: &MapPack) -> Option<String>;
    pub fn on_escape(&mut self) -> bool;
}
// navigator.rs (D)
pub struct Navigator { .. }
impl Navigator {
    pub fn handle_input(view: &mut MapView, ui: &Ui, resp: &egui::Response, vp: Rect) -> bool;  // true = pointer is on the navigator
    pub fn draw(view: &mut MapView, ui: &mut Ui, vp: Rect, theme: &Theme);
}
pub fn zoom_control(ui: &mut Ui, theme: &Theme, cam: &mut Camera, vp: Rect) -> bool;
// export.rs (B)
pub struct ExportUi { .. }  impl ExportUi { pub fn request_open(&mut self); pub fn is_open(&self) -> bool; }
pub(super) fn export_ui(view: &mut MapView, ui: &mut Ui, theme: &Theme, cfg: &Config, ctrl: &MainController, actions: &mut Vec<MapAction>);
// route_path.rs (E)
pub fn draw(painter: &egui::Painter, oc: &OverlayCtx, state: &RouteMapState, toggles: &Toggles);
pub fn handle_click(view: &mut MapView, ui: &Ui, resp: &egui::Response, vp: Rect) -> bool;   // a disc click selects the event
// keynav.rs (E)
pub fn handle_keys(view: &mut MapView, ui: &Ui, vp: Rect);
pub fn on_route_synced(view: &mut MapView);
// finder.rs (E): search over trainers / items → anchors for the focus banner
```

`Toggles` gains `grid` (off), `labels` (on), `path` (off), `navigator` (on),
`follow` (off). `MapAction` gains `AddTrainersNamed { folder, names }`,
`Exported(PathBuf)`, `ExportFailed(String)`, `CopiedImage`.

### 3.4 Design notes per package

**A.** Render in horizontal bands of 256 world-px rows through
`Compositor::render_region`, upscale each band with nearest-neighbour into
the output, report progress after each band, stop on cancel. `transparent`
leaves pixels no map covers at alpha 0 (a transparent fill instead of
`BACKGROUND`; `Compositor::covers` tells which bands can be skipped). Bound:
`output_pixels > MAX_PIXELS` is refused before any allocation. Tests: size
maths; a 1× export equals `render_scope`; a 3× export equals the nearest
upscale pixel for pixel including across band boundaries; transparency
outside maps and opacity inside; cancel after the first band returns
`Cancelled` quickly; Emerald's world at 1× (137 Mpx) stays under `MAX_PIXELS`
and renders (release timing printed).

**B.** The dialog is an `egui::Modal` drawn by the map itself (so it works
docked and in the map's own window): Region (View / Selection when one
exists / This map when in a map scope / Whole world), Scale 1–8, Night
(gen 2), layer checkboxes prefilled from the toggles (markers, sprites,
grid, labels, route path, selection outline), Transparent background, an
output-size line (px and MB) that turns into the refusal text past the
bound, buttons Export / Copy / Cancel. The job runs on a `std::thread`:
`xpr_map::export::render` for the base, then the overlays through
`screenshot::Offscreen` (theme fonts, ppp 1, an export `Camera` with
`zoom = scale` centred on the region, `vp = (0,0)..(w·scale, h·scale)`) using
`layers::draw_markers` with a fresh `SpriteAtlas` bound to the offscreen
context, `overlay::draw_grid`, `overlay::draw_map_labels`,
`route_path::draw`, `tools::ToolState::draw`; rasterised band by band
(`Offscreen::rasterize(prims, band)`) and alpha-blended onto the pixmap so
the canvas never holds the whole image in f32. Progress bar + Cancel in the
modal while it runs; the result comes back as `MapAction::Exported(path)`
(file: `ctrl.screenshot_path(&cfg.get_images_dir(), "map")`) or
`CopiedImage` (`ctx.copy_image`). `app.rs`: Map menu "Export Map Image…"
(`export_map`, Ctrl+Shift+P) and "Copy Map View"; `Exported` → toast with the
folder; `ShotKind::Map` + `XPR_SMOKE_EXPORT=map` export the current view at
1× synchronously.

**C.** `Tool::Pan` keeps today's behaviour. `Select`: drag draws a
step-snapped rectangle (marching-ants stroke, dimmed outside optional),
handles Shift+drag from Pan as a shortcut, shows "24 × 13 steps (384 × 208
px)" in the status strip, a small floating toolbar under the selection with
Zoom / Export / Copy / Add trainers / ✕ (Export and Copy push through
`view.export` — `request_open` preselects Selection; Copy pushes
`MapAction`s), Esc clears. `Measure`: drag draws a line with the Δ label; the
last measurement stays until Esc or a new drag. Grid: step lines at zoom ≥ 3,
block lines at zoom ≥ 1, map outlines in the world scope always; subtle
alpha. Labels: map display names at map centres when the map is at least
~80 screen px wide, elided, with a dark halo; not in a map scope.

**D.** Navigator: a 200×140 panel in the viewport's bottom-right showing the
scope through the coarsest chunk (`layers::max_level`, `ChunkKey {level:
max, cx: 0, cy: 0}` requested if absent), aspect-fitted, with the visible
world rect outlined; click or drag moves the camera (`Camera::center_on`);
the pointer over the panel never reaches the map. Zoom control: clicking the
readout opens a menu with 25 / 50 / 100 / 200 / 400 / 800 %, Fit and 1:1;
`Camera::set_zoom_centered` exists; key `1` = 100 %.

**E.** Trip path (SPEC D15 / §13, superseding the whole-route idea):
`RouteMapState::sync` builds `trip: Option<TripPath>` from the selected
folder, or the nearest enclosing folder of the selected event, sub-folders
included, in `Router::all_groups` order — enabled trainer fights and
no-money pickups whose anchor is `Precision::Object` / `Script`, first anchor
when there are several, consecutive nodes on one step collapsed into a
"3–5" badge; root-level events give no trip. `route_path::draw` draws
straight segments between consecutive nodes in the current scope (phase 1,
no warp projection, no pathfinding) and numbered discs, the selected
event's ringed; a disc click emits `MapAction::SelectEvent`; never a line
between two folders. Hints in the viewport when there is no trip / no
anchored events.
Finder: the toolbar search popup gains Trainers and Items groups (router
names, from `pack.links` keys resolved against `TrainerDB` / `ItemDB` for
display); choosing one calls `focus_anchors`. Keynav: Tab / Shift+Tab move
the selection through the visible, toggled-on objects of the map under the
viewport centre (nearest first), Enter opens the card, the card follows.
Follow: when `toggles.follow` and the selected event has anchors,
`on_route_synced` centres the camera on the first one without a banner.

### 3.5 Verification (F)

`cargo test -p xpr-map -p xpr-app`; clippy is not enforced. Headless
pictures through `Offscreen` (a new `examples/map_export_png.rs` from B and
the tests' PNG outputs) read as images. A smoke run of the app:
`XPR_GLOBAL_CONFIG_DIR=<tmp> XPR_SMOKE_SCREENSHOT=<png> XPR_SMOKE_ACTION=map
XPR_SMOKE_EVENT=Brock XPR_SMOKE_EXPORT=map cargo run --release -p xpr-app`
must exit cleanly with the map screenshot and the exported PNG. Docs updated
(SPEC §12 dated block, TESTING §17 checklist, PORT_STATUS world-map paragraph).

## 4. Outcome (2026-09-25)

All five packages landed the same day, in parallel, in one working tree.
Gaps G1–G13 are built (G9 to the one-trip rule of SPEC D15 / §13, phase 1);
G14 and G15 stay deferred. Also fixed on the way: egui's bare-Tab focus grab
(the toolbar search box took every Tab before the map saw it — `keynav::
reclaim_hover_tab_focus`), and the shared test config path (`xpr_app::
scratch_config_path()`; the harnesses all wrote `rust/target/
no-such-config.json` and leaked map view state into each other).

Verification, 2026-09-25 on this Mac (rustc 1.96, dev profile):

| Check | Result |
|---|---|
| `cargo test --workspace` | 229 passed, 0 failed, 1 ignored (Emerald's 137 Mpx world at 1×: release-only timing test, 0.2 s composite) over 61 binaries; new: `xpr-map` export 10, `map_export` 4, `map_tools` 7, `map_navigator` 2, `map_navigation` 11, `map_modal_input` 5, screenshot blend 2 |
| `cargo build -p xpr-app --all-targets` | no warnings |
| Headless pictures | `export_png` (Pewter City 2×, Crystal night world transparent), `map_export_png` (Pewter Gym 3× with sprites + routed badges + warps, Pewter City 2×), `map_tools_png` (selection, ruler badge, grid, label, Layers popup, navigator in one frame) read as images and matched the geometry |
| Smoke run | `XPR_SMOKE_ACTION=map XPR_SMOKE_EVENT=Brock XPR_SMOKE_EXPORT=map` on an isolated config: exit 0, window screenshot shows the Map tab with the new toolbar, navigator and banner; `<timestamp>-yellow-pinsir-lv10brock_map.png` written; no `ERROR` in the log |

An independent review of the new code (a second agent reading every file
and tracing egui's focus and modal internals) found two bugs and three nits;
all fixed the same day except the badge: the navigator captured any drag
that merely crossed its panel (it now owns only a drag that began on it;
regression test `a_drag_that_only_crosses_the_panel_is_not_captured`);
"Copy Map View" from the menu was dropped while the map pane was not
showing (it now opens the map first, like "Export Map Image…"); the export
size maths multiplied in `u32` before the `u64` bound check (now `u64` and
saturating throughout). Left as is: `MapList::hovered` is a stale field from
the MVP; the "1 of n" badge for a trainer with several anchors (SPEC §13.2)
is not drawn — the first anchor is used silently.

Known, not fixed here: on this Mac the toolbar's "✕" / "⤢" / "⤡" glyphs
(from the MVP) draw as boxes — the configured system font is not found on
this machine and egui's fallback fonts lack those glyphs; the new Layers
button therefore paints its chevron (`widgets::paint_chevron`) instead of
using a "▾" glyph.
Windows was not built (the app cannot be cross-checked from the Mac; see
the Windows cross-check note). Not committed: nothing in this repo is.
