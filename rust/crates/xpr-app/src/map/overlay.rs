//! Overlays drawn over the map by the viewer and by the export
//! (`docs/rust_port/design/world_map/GRAPHICS_TOOLS_PLAN.md` WP-C, gaps
//! G5 / G6): the step / block grid with map outlines, and map name labels.
//! Free functions of an [`OverlayCtx`] so the export can draw them with an
//! export camera into the offscreen rasteriser.
//!
//! Both overlays are drawn as individual `line_segment` / `text` calls
//! rather than the higher-level `rect_stroke` convenience (which folds a
//! whole outline into one `Shape::Rect`): a grid *is* naturally a bundle of
//! line segments, and it keeps the shapes easy to spot in a frame's output
//! (`tests/map_tools.rs` counts `Shape::LineSegment` / `Shape::Path`).

use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use xpr_map::{geom, IRect, MapPack, Scope};
use xpr_ui_kit::theme::{self, Theme};

use super::camera::Camera;
use super::Toggles;

/// Everything an overlay needs to place itself on the target.
pub struct OverlayCtx<'a> {
    pub theme: &'a Theme,
    pub pack: &'a MapPack,
    pub scope: Scope,
    pub cam: &'a Camera,
    /// the viewport in points (the screen's, or the export canvas)
    pub vp: Rect,
    /// pixels per point of the target
    pub ppp: f32,
}

/// Step lines when zoomed in, block lines lower, map outlines always (SPEC
/// WP-C "C": "step lines at zoom >= 3, block lines at zoom >= 1, map
/// outlines in the world scope always"). Only ever draws inside the visible
/// world rect, so this stays cheap regardless of the pack's size.
pub fn draw_grid(painter: &egui::Painter, oc: &OverlayCtx, toggles: &Toggles) {
    if !toggles.grid {
        return;
    }
    let scope_rect = geom::scope_rect(oc.pack, oc.scope);
    let vis_world = oc.cam.visible_world(oc.vp);
    let vis = vis_world.intersect(&scope_rect);
    let zoom = oc.cam.zoom;
    if !vis.is_empty() {
        // block lines first (bolder-ish, coarser) so step lines sit on top
        // once zoomed in enough to show both
        if zoom >= 1.0 {
            let block = oc.pack.geom.block_px.max(1) as i32;
            draw_grid_lines(painter, oc, vis, block, 0x38);
        }
        if zoom >= 3.0 {
            let step = oc.pack.geom.step_px.max(1) as i32;
            draw_grid_lines(painter, oc, vis, step, 0x28);
        }
    }
    match oc.scope {
        Scope::World => {
            for (rect, _id) in &oc.pack.layout.placed_rects {
                if rect.intersects(&vis_world) {
                    draw_rect_outline(painter, oc, *rect, 0x55);
                }
            }
        }
        Scope::Map(_) => draw_rect_outline(painter, oc, scope_rect, 0x55),
    }
}

/// Map display names at map centres (world scope), readable when zoomed out
/// (SPEC WP-C "C": drawn when the map is at least ~80 screen px wide,
/// elided to its screen width, with a dark halo; nothing in a map scope).
pub fn draw_map_labels(painter: &egui::Painter, oc: &OverlayCtx, toggles: &Toggles) {
    if !toggles.labels || oc.scope != Scope::World {
        return;
    }
    let vis = oc.cam.visible_world(oc.vp);
    for (rect, id) in &oc.pack.layout.placed_rects {
        if !rect.intersects(&vis) {
            continue;
        }
        let Some(m) = oc.pack.map(*id) else { continue };
        if m.display.is_empty() {
            continue;
        }
        let a = oc.cam.world_to_screen(oc.vp, Vec2::new(rect.x0 as f32, rect.y0 as f32));
        let b = oc.cam.world_to_screen(oc.vp, Vec2::new(rect.x1 as f32, rect.y1 as f32));
        let screen_w = (b.x - a.x).abs();
        if screen_w < 80.0 {
            continue;
        }
        let font_size = (screen_w / 18.0).clamp(9.0, 14.0);
        let font = oc.theme.font_bold(font_size);
        let center = Pos2::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        let label = elide_for_painter(painter, &m.display, &font, (screen_w - 10.0).max(12.0));
        draw_halo_text(painter, center, egui::Align2::CENTER_CENTER, &label, font, Color32::WHITE, theme::with_alpha(Color32::BLACK, 0xC8));
    }
}

// ---------------------------------------------------------------------------
// grid lines / outlines
// ---------------------------------------------------------------------------

fn div_floor(v: i32, d: i32) -> i32 {
    v.div_euclid(d)
}

fn div_ceil(v: i32, d: i32) -> i32 {
    -((-v).div_euclid(d))
}

/// Every line of `spacing` (world px) inside `vis`, drawn as a dark pass
/// under a light pass (both very low alpha) so the grid reads whether the
/// tile under it is light or dark, the same halo trick `draw_map_labels`
/// uses for text.
fn draw_grid_lines(painter: &egui::Painter, oc: &OverlayCtx, vis: IRect, spacing: i32, alpha: u8) {
    if spacing <= 0 {
        return;
    }
    let w = (1.0 / oc.ppp.max(1.0)).max(0.5);
    let light = theme::with_alpha(Color32::WHITE, alpha);
    let dark = theme::with_alpha(Color32::BLACK, alpha);
    let x0 = div_floor(vis.x0, spacing) * spacing;
    let x1 = div_ceil(vis.x1, spacing) * spacing;
    let y0 = div_floor(vis.y0, spacing) * spacing;
    let y1 = div_ceil(vis.y1, spacing) * spacing;
    let mut x = x0;
    while x <= x1 {
        let a = oc.cam.world_to_screen(oc.vp, Vec2::new(x as f32, vis.y0 as f32));
        let b = oc.cam.world_to_screen(oc.vp, Vec2::new(x as f32, vis.y1 as f32));
        painter.line_segment([a, b], Stroke::new(w, dark));
        painter.line_segment([a, b], Stroke::new(w, light));
        x += spacing;
    }
    let mut y = y0;
    while y <= y1 {
        let a = oc.cam.world_to_screen(oc.vp, Vec2::new(vis.x0 as f32, y as f32));
        let b = oc.cam.world_to_screen(oc.vp, Vec2::new(vis.x1 as f32, y as f32));
        painter.line_segment([a, b], Stroke::new(w, dark));
        painter.line_segment([a, b], Stroke::new(w, light));
        y += spacing;
    }
}

/// A rectangle's outline as four line segments (not `rect_stroke`; see the
/// module doc), dark-under-light like `draw_grid_lines`.
fn draw_rect_outline(painter: &egui::Painter, oc: &OverlayCtx, rect: IRect, alpha: u8) {
    let w = (1.0 / oc.ppp.max(1.0)).max(0.5);
    let light = theme::with_alpha(Color32::WHITE, alpha);
    let dark = theme::with_alpha(Color32::BLACK, alpha);
    let a = oc.cam.world_to_screen(oc.vp, Vec2::new(rect.x0 as f32, rect.y0 as f32));
    let b = oc.cam.world_to_screen(oc.vp, Vec2::new(rect.x1 as f32, rect.y0 as f32));
    let c = oc.cam.world_to_screen(oc.vp, Vec2::new(rect.x1 as f32, rect.y1 as f32));
    let d = oc.cam.world_to_screen(oc.vp, Vec2::new(rect.x0 as f32, rect.y1 as f32));
    for &(p, q) in &[(a, b), (b, c), (c, d), (d, a)] {
        painter.line_segment([p, q], Stroke::new(w, dark));
        painter.line_segment([p, q], Stroke::new(w, light));
    }
}

// ---------------------------------------------------------------------------
// map name labels
// ---------------------------------------------------------------------------

/// `widgets::elide`, but for a `Painter` (`draw_map_labels` has no `Ui` to
/// measure text with — the contract gives overlays a painter, not a ui).
fn elide_for_painter(painter: &egui::Painter, text: &str, font: &egui::FontId, max_width: f32) -> String {
    if text.is_empty() {
        return String::new();
    }
    let width = |s: &str| painter.layout_no_wrap(s.to_string(), font.clone(), Color32::WHITE).size().x;
    if width(text) <= max_width {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut lo = 0usize;
    let mut hi = chars.len();
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        let candidate: String = chars[..mid].iter().collect::<String>() + "…";
        if width(&candidate) <= max_width {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    if lo == 0 {
        return "…".to_string();
    }
    chars[..lo].iter().collect::<String>() + "…"
}

/// Text drawn 4 times offset by 1 px in `halo`, then once in `fg` — reads on
/// any tile underneath (SPEC WP-C "C").
fn draw_halo_text(painter: &egui::Painter, pos: Pos2, anchor: egui::Align2, text: &str, font: egui::FontId, fg: Color32, halo: Color32) {
    for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        painter.text(pos + Vec2::new(dx, dy), anchor, text, font.clone(), halo);
    }
    painter.text(pos, anchor, text, font, fg);
}
