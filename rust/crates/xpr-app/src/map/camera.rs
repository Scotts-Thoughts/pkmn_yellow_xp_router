//! The map camera: a centre in world pixels and a zoom (screen px per world
//! px), plus the screen/world maths and a short eased animation used by
//! "show on map" and map-list navigation (SPEC §3.3).

use std::time::{Duration, Instant};

use egui::{Pos2, Rect, Vec2};
use xpr_map::IRect;

pub const MAX_ZOOM: f32 = 12.0;
pub const MIN_ZOOM_ABS: f32 = 0.02;
const ANIM_MS: u64 = 260;

#[derive(Clone, Debug)]
pub struct Camera {
    pub center: Vec2,
    pub zoom: f32,
    pub min_zoom: f32,
    anim: Option<CameraAnim>,
}

#[derive(Clone, Debug)]
struct CameraAnim {
    from_center: Vec2,
    from_zoom: f32,
    to_center: Vec2,
    to_zoom: f32,
    start: Instant,
}

impl Default for Camera {
    fn default() -> Self {
        Camera { center: Vec2::ZERO, zoom: 2.0, min_zoom: 0.1, anim: None }
    }
}

impl Camera {
    pub fn world_to_screen(&self, vp: Rect, w: Vec2) -> Pos2 {
        vp.center() + (w - self.center) * self.zoom
    }

    pub fn screen_to_world(&self, vp: Rect, s: Pos2) -> Vec2 {
        (s - vp.center()) / self.zoom + self.center
    }

    /// The world rectangle visible in the viewport.
    pub fn visible_world(&self, vp: Rect) -> IRect {
        let a = self.screen_to_world(vp, vp.min);
        let b = self.screen_to_world(vp, vp.max);
        IRect::new(a.x.floor() as i32, a.y.floor() as i32, b.x.ceil() as i32, b.y.ceil() as i32)
    }

    pub fn pan(&mut self, screen_delta: Vec2) {
        self.anim = None;
        self.center -= screen_delta / self.zoom;
    }

    /// Multiply the zoom by `factor`, keeping the world point under `screen_pos` fixed.
    pub fn zoom_at(&mut self, vp: Rect, screen_pos: Pos2, factor: f32) {
        self.anim = None;
        let before = self.screen_to_world(vp, screen_pos);
        self.zoom = (self.zoom * factor).clamp(self.min_zoom.max(MIN_ZOOM_ABS), MAX_ZOOM);
        let after = self.screen_to_world(vp, screen_pos);
        self.center += before - after;
    }

    pub fn set_zoom_centered(&mut self, vp: Rect, zoom: f32) {
        let c = vp.center();
        let factor = zoom / self.zoom;
        self.zoom_at(vp, c, factor);
    }

    /// Zoom so that `rect` (world px) fits the viewport, centred.
    pub fn fit(&mut self, vp: Rect, rect: IRect, animate: bool) {
        let w = rect.width().max(1) as f32;
        let h = rect.height().max(1) as f32;
        let zoom = ((vp.width() - 24.0) / w).min((vp.height() - 24.0) / h).clamp(MIN_ZOOM_ABS, MAX_ZOOM);
        let center = Vec2::new(rect.x0 as f32 + w / 2.0, rect.y0 as f32 + h / 2.0);
        self.go_to(center, zoom, animate);
    }

    /// The zoom at which the whole scope fits; used as the lower bound.
    pub fn set_min_zoom_for(&mut self, vp: Rect, scope: IRect) {
        let w = scope.width().max(1) as f32;
        let h = scope.height().max(1) as f32;
        let fit = (vp.width() / w).min(vp.height() / h);
        self.min_zoom = (fit * 0.5).clamp(MIN_ZOOM_ABS, 1.0);
    }

    pub fn go_to(&mut self, center: Vec2, zoom: f32, animate: bool) {
        let zoom = zoom.clamp(self.min_zoom.max(MIN_ZOOM_ABS), MAX_ZOOM);
        if animate {
            self.anim = Some(CameraAnim { from_center: self.center, from_zoom: self.zoom, to_center: center, to_zoom: zoom, start: Instant::now() });
        } else {
            self.anim = None;
            self.center = center;
            self.zoom = zoom;
        }
    }

    pub fn center_on(&mut self, center: Vec2, animate: bool) {
        let z = self.zoom;
        self.go_to(center, z, animate);
    }

    /// Advance the animation; returns true while one is running.
    pub fn tick(&mut self) -> bool {
        let Some(a) = &self.anim else { return false };
        let t = a.start.elapsed().as_secs_f32() / Duration::from_millis(ANIM_MS).as_secs_f32();
        if t >= 1.0 {
            self.center = a.to_center;
            self.zoom = a.to_zoom;
            self.anim = None;
            return false;
        }
        let e = 1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t); // ease-out cubic
        self.center = a.from_center + (a.to_center - a.from_center) * e;
        // interpolate zoom geometrically so the motion feels linear
        self.zoom = a.from_zoom * (a.to_zoom / a.from_zoom).powf(e);
        true
    }

    /// Keep the scope in view: the centre stays inside the scope rectangle
    /// (with a margin so small maps can still be scrolled to the edges).
    pub fn clamp_to(&mut self, vp: Rect, scope: IRect) {
        let half_w = vp.width() / self.zoom / 2.0;
        let half_h = vp.height() / self.zoom / 2.0;
        let (x0, y0, x1, y1) = (scope.x0 as f32, scope.y0 as f32, scope.x1 as f32, scope.y1 as f32);
        // allow the view to overshoot by half the viewport so edges can reach the centre
        let min_x = x0 - half_w * 0.5;
        let max_x = x1 + half_w * 0.5;
        let min_y = y0 - half_h * 0.5;
        let max_y = y1 + half_h * 0.5;
        if max_x > min_x {
            self.center.x = self.center.x.clamp(min_x, max_x);
        }
        if max_y > min_y {
            self.center.y = self.center.y.clamp(min_y, max_y);
        }
    }

    /// The LOD level for the current zoom: one texel is at least half a screen pixel.
    pub fn lod_level(&self, max_level: u8) -> u8 {
        if self.zoom >= 1.0 {
            return 0;
        }
        let l = (1.0 / self.zoom).log2().floor();
        (l.max(0.0) as u8).min(max_level)
    }
}
