//! Screenshot export (`_grab_transparent` / `widget.render(pixmap)` in the Qt
//! app): the widget is drawn into a private egui context on a transparent
//! canvas of any size, tessellated, and rasterised here in software. The PNG
//! therefore holds only the widget — no window background between the cards,
//! no menus, tooltips or hover states, and no scroll viewport cutting the
//! content off — with the same anti-aliased rounded corners the cards have
//! on screen. The framebuffer capture (`ViewportCommand::Screenshot`) is
//! only kept for the unattended smoke run.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use egui::epaint::{ClippedPrimitive, ImageData, ImageDelta, Primitive, TextureId, Vertex};
use egui::{ColorImage, Pos2, RawInput, Rect, TextureFilter, TextureOptions, TextureWrapMode, Vec2, ViewportId, ViewportInfo};
use xpr_ui_kit::theme::Theme;

use crate::battle_ui::ScreenshotMode;

#[derive(Clone, Debug, PartialEq)]
pub enum ShotKind {
    EventList,
    BattleSummary,
    PlayerRanges,
    EnemyRanges,
    Matchup { idx: usize, mode: ScreenshotMode },
    RunSummary,
    SetupSummary,
    /// The active tab of the route-compare page.
    Compare,
}

impl ShotKind {
    /// `take_screenshot`'s image name (the part after the route name).
    pub fn image_name(&self) -> String {
        match self {
            ShotKind::EventList => "event_list".to_string(),
            ShotKind::BattleSummary => "battle_summary".to_string(),
            ShotKind::PlayerRanges => "player_ranges".to_string(),
            ShotKind::EnemyRanges => "enemy_ranges".to_string(),
            ShotKind::Matchup { idx, mode } => match mode {
                ScreenshotMode::Full => format!("matchup_{}", idx + 1),
                ScreenshotMode::Player => format!("matchup_{}_player_ranges", idx + 1),
                ScreenshotMode::Enemy => format!("matchup_{}_enemy_ranges", idx + 1),
            },
            ShotKind::RunSummary => "run_summary".to_string(),
            ShotKind::SetupSummary => "setup_summary".to_string(),
            ShotKind::Compare => "compare".to_string(),
        }
    }

    /// The battle summary layout the kind is drawn with, if it is one.
    pub fn battle_mode(&self) -> Option<ScreenshotMode> {
        match self {
            ShotKind::BattleSummary => Some(ScreenshotMode::Full),
            ShotKind::PlayerRanges => Some(ScreenshotMode::Player),
            ShotKind::EnemyRanges => Some(ScreenshotMode::Enemy),
            ShotKind::Matchup { mode, .. } => Some(*mode),
            _ => None,
        }
    }
}

/// The corner radius of the matchup cards (`border-radius: 6px`), which the
/// cut edge of a half export gets too.
pub const CARD_RADIUS: f32 = 6.0;

// ---------------------------------------------------------------------------
// Offscreen egui context
// ---------------------------------------------------------------------------

/// A texture as the rasteriser samples it: premultiplied gamma-space RGBA in
/// 0..1, exactly what egui's shaders see.
struct SoftTexture {
    size: [usize; 2],
    pixels: Vec<[f32; 4]>,
    options: TextureOptions,
}

/// An egui context of its own, with the app's fonts and style, that draws on
/// a transparent canvas. Textures the pass uploads (the font atlas, icons)
/// are kept here for [`Offscreen::rasterize`].
pub struct Offscreen {
    ctx: egui::Context,
    ppp: f32,
    max_texture_side: usize,
    textures: HashMap<TextureId, SoftTexture>,
    frame: u64,
}

impl Offscreen {
    /// `ppp` and `max_texture_side` are the main window's, so text is laid
    /// out and rasterised exactly as on screen.
    pub fn new(theme: &Theme, ppp: f32, max_texture_side: usize) -> Offscreen {
        let ctx = egui::Context::default();
        let mut theme = theme.clone();
        theme.install_fonts(&ctx);
        theme.apply(&ctx);
        Offscreen { ctx, ppp, max_texture_side, textures: HashMap::new(), frame: 0 }
    }

    /// A context with egui's built-in fonts only (tests).
    pub fn with_default_fonts(ppp: f32) -> Offscreen {
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::default());
        Offscreen { ctx, ppp, max_texture_side: 2048, textures: HashMap::new(), frame: 0 }
    }

    pub fn pixels_per_point(&self) -> f32 {
        self.ppp
    }

    /// Draw `draw` on a transparent canvas of `size` points and return the
    /// tessellated result. egui settles a fresh layout over a couple of
    /// passes (scroll bars, first-frame sizes), so the canvas is drawn
    /// `passes` times and the last pass is what comes back.
    pub fn render(&mut self, size: Vec2, passes: usize, mut draw: impl FnMut(&egui::Context)) -> Vec<ClippedPrimitive> {
        let mut prims = Vec::new();
        for _ in 0..passes.max(1) {
            let mut input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                max_texture_side: Some(self.max_texture_side),
                time: Some(self.frame as f64 / 60.0),
                ..Default::default()
            };
            input.viewports.insert(ViewportId::ROOT, ViewportInfo { native_pixels_per_point: Some(self.ppp), ..Default::default() });
            self.frame += 1;
            let out = self.ctx.run(input, |ctx| draw(ctx));
            for (id, delta) in &out.textures_delta.set {
                self.set_texture(*id, delta);
            }
            prims = self.ctx.tessellate(out.shapes, out.pixels_per_point);
            for id in &out.textures_delta.free {
                self.textures.remove(id);
            }
        }
        prims
    }

    fn set_texture(&mut self, id: TextureId, delta: &ImageDelta) {
        let ImageData::Color(img) = &delta.image;
        let src: Vec<[f32; 4]> = img.pixels.iter().map(|c| c.to_array().map(|v| v as f32 / 255.0)).collect();
        match delta.pos {
            None => {
                self.textures.insert(id, SoftTexture { size: img.size, pixels: src, options: delta.options });
            }
            Some([px, py]) => {
                let Some(tex) = self.textures.get_mut(&id) else {
                    log::warn!("texture patch for an unknown texture {:?}", id);
                    return;
                };
                let [tw, th] = tex.size;
                let [w, h] = img.size;
                for y in 0..h {
                    for x in 0..w {
                        let (tx, ty) = (px + x, py + y);
                        if tx < tw && ty < th {
                            tex.pixels[ty * tw + tx] = src[y * w + x];
                        }
                    }
                }
            }
        }
    }

    /// Rasterise `prims` into the `crop` rectangle (points) of the canvas.
    pub fn rasterize(&self, prims: &[ClippedPrimitive], crop: Rect) -> Result<Canvas, String> {
        let ppp = self.ppp;
        let x0 = (crop.min.x * ppp).round() as i64;
        let y0 = (crop.min.y * ppp).round() as i64;
        let x1 = (crop.max.x * ppp).round() as i64;
        let y1 = (crop.max.y * ppp).round() as i64;
        if x1 <= x0 || y1 <= y0 {
            return Err("empty screenshot region".to_string());
        }
        let mut canvas = Canvas::new((x1 - x0) as usize, (y1 - y0) as usize);
        for prim in prims {
            let Primitive::Mesh(mesh) = &prim.primitive else { continue };
            // the scissor rectangle, rounded to pixels the way egui_glow does
            let scissor = [
                ((prim.clip_rect.min.x * ppp).round() as i64 - x0).max(0),
                ((prim.clip_rect.min.y * ppp).round() as i64 - y0).max(0),
                ((prim.clip_rect.max.x * ppp).round() as i64 - x0).min(canvas.width as i64),
                ((prim.clip_rect.max.y * ppp).round() as i64 - y0).min(canvas.height as i64),
            ];
            if scissor[2] <= scissor[0] || scissor[3] <= scissor[1] {
                continue;
            }
            let tex = self.textures.get(&mesh.texture_id);
            for tri in mesh.indices.as_chunks::<3>().0 {
                let v = [&mesh.vertices[tri[0] as usize], &mesh.vertices[tri[1] as usize], &mesh.vertices[tri[2] as usize]];
                draw_triangle(&mut canvas, v, tex, ppp, (x0 as f32, y0 as f32), scissor);
            }
        }
        Ok(canvas)
    }
}

impl SoftTexture {
    fn wrap(&self, i: i64, n: usize) -> usize {
        let n = n as i64;
        match self.options.wrap_mode {
            TextureWrapMode::ClampToEdge => i.clamp(0, n - 1) as usize,
            TextureWrapMode::Repeat => i.rem_euclid(n) as usize,
            TextureWrapMode::MirroredRepeat => {
                let m = i.rem_euclid(2 * n);
                (if m < n { m } else { 2 * n - 1 - m }) as usize
            }
        }
    }

    fn texel(&self, x: i64, y: i64) -> [f32; 4] {
        let [w, h] = self.size;
        self.pixels[self.wrap(y, h) * w + self.wrap(x, w)]
    }

    /// Sample at normalised `(u, v)` like the GPU: texel centres sit at
    /// `(i + 0.5) / size`, and linear filtering blends the four nearest.
    fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let [w, h] = self.size;
        if w == 0 || h == 0 {
            return [1.0; 4];
        }
        match self.options.magnification {
            TextureFilter::Nearest => self.texel((u * w as f32).floor() as i64, (v * h as f32).floor() as i64),
            TextureFilter::Linear => {
                let fx = u * w as f32 - 0.5;
                let fy = v * h as f32 - 0.5;
                let (x0, y0) = (fx.floor(), fy.floor());
                let (tx, ty) = (fx - x0, fy - y0);
                let (x0, y0) = (x0 as i64, y0 as i64);
                let p00 = self.texel(x0, y0);
                let p10 = self.texel(x0 + 1, y0);
                let p01 = self.texel(x0, y0 + 1);
                let p11 = self.texel(x0 + 1, y0 + 1);
                let mut out = [0.0; 4];
                for c in 0..4 {
                    let top = p00[c] + (p10[c] - p00[c]) * tx;
                    let bottom = p01[c] + (p11[c] - p01[c]) * tx;
                    out[c] = top + (bottom - top) * ty;
                }
                out
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Software rasteriser
// ---------------------------------------------------------------------------

/// Premultiplied gamma-space RGBA, 0..1 — the blend egui's shaders do
/// (`ONE, ONE_MINUS_SRC_ALPHA`), kept in floats until the PNG is written.
pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<[f32; 4]>,
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Canvas {
        Canvas { width, height, pixels: vec![[0.0; 4]; width * height] }
    }

    #[inline]
    fn blend(&mut self, x: usize, y: usize, src: [f32; 4]) {
        let dst = &mut self.pixels[y * self.width + x];
        let keep = 1.0 - src[3];
        for c in 0..4 {
            dst[c] = src[c] + dst[c] * keep;
        }
    }

    /// The pixel as straight-alpha RGBA bytes.
    pub fn rgba8(&self, x: usize, y: usize) -> [u8; 4] {
        let p = self.pixels[y * self.width + x];
        let a = p[3].clamp(0.0, 1.0);
        if a <= 0.0 {
            return [0, 0, 0, 0];
        }
        let to_u8 = |v: f32| (v / a * 255.0).round().clamp(0.0, 255.0) as u8;
        [to_u8(p[0]), to_u8(p[1]), to_u8(p[2]), (a * 255.0).round() as u8]
    }

    pub fn to_image(&self) -> image::RgbaImage {
        let mut out = image::RgbaImage::new(self.width as u32, self.height as u32);
        for y in 0..self.height {
            for x in 0..self.width {
                out.put_pixel(x as u32, y as u32, image::Rgba(self.rgba8(x, y)));
            }
        }
        out
    }

    pub fn save(&self, out_path: &Path) -> Result<(), String> {
        if let Some(parent) = out_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        self.to_image().save(out_path).map_err(|e| e.to_string())
    }

    /// `_round_container_corners`: fade the four corners of each container
    /// (canvas pixels, `[x0, y0, x1, y1]` with exclusive maxima) to `radius`,
    /// anti-aliased, so the cut edge of a half export gets the rounding the
    /// drawn edge has. Corners that are already transparent are unaffected.
    pub fn round_corners(&mut self, rects: &[[i64; 4]], radius: f32) {
        if radius <= 0.0 {
            return;
        }
        let span = radius.ceil() as i64;
        for &[x0, y0, x1, y1] in rects {
            if x1 - x0 < 2 * span || y1 - y0 < 2 * span {
                continue;
            }
            // (corner square origin, circle centre) for each corner
            let corners = [
                (x0, y0, x0 as f32 + radius, y0 as f32 + radius),
                (x1 - span, y0, x1 as f32 - radius, y0 as f32 + radius),
                (x0, y1 - span, x0 as f32 + radius, y1 as f32 - radius),
                (x1 - span, y1 - span, x1 as f32 - radius, y1 as f32 - radius),
            ];
            for (sx, sy, cx, cy) in corners {
                for py in sy..sy + span {
                    for px in sx..sx + span {
                        if px < 0 || py < 0 || px >= self.width as i64 || py >= self.height as i64 {
                            continue;
                        }
                        let dx = px as f32 + 0.5 - cx;
                        let dy = py as f32 + 0.5 - cy;
                        let coverage = (radius + 0.5 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0);
                        if coverage < 1.0 {
                            let p = &mut self.pixels[py as usize * self.width + px as usize];
                            for c in p.iter_mut() {
                                *c *= coverage;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// `(b - a) x (p - a)`: positive on the interior side of the edge for a
/// triangle whose signed area is positive.
#[inline]
fn edge(a: (f32, f32), b: (f32, f32), x: f32, y: f32) -> f32 {
    (b.0 - a.0) * (y - a.1) - (b.1 - a.1) * (x - a.0)
}

/// Whether an edge (oriented as in a positive-area triangle) owns the pixel
/// centres lying exactly on it: the top-left fill rule, so a pixel centre on
/// an edge shared by two triangles is drawn by exactly one of them (egui's
/// anti-aliasing strips share edges with the fill they feather).
#[inline]
fn is_top_left(a: (f32, f32), b: (f32, f32)) -> bool {
    (b.1 == a.1 && b.0 > a.0) || b.1 < a.1
}

fn draw_triangle(canvas: &mut Canvas, v: [&Vertex; 3], tex: Option<&SoftTexture>, ppp: f32, origin: (f32, f32), scissor: [i64; 4]) {
    let to_px = |v: &Vertex| (v.pos.x * ppp - origin.0, v.pos.y * ppp - origin.1);
    let (va, mut vb, mut vc) = (v[0], v[1], v[2]);
    let (a, mut b, mut c) = (to_px(va), to_px(vb), to_px(vc));
    let mut area = edge(a, b, c.0, c.1);
    if area == 0.0 || !area.is_finite() {
        return;
    }
    if area < 0.0 {
        std::mem::swap(&mut b, &mut c);
        std::mem::swap(&mut vb, &mut vc);
        area = -area;
    }
    let min_x = (a.0.min(b.0).min(c.0).floor() as i64).max(scissor[0]);
    let max_x = (a.0.max(b.0).max(c.0).ceil() as i64).min(scissor[2]);
    let min_y = (a.1.min(b.1).min(c.1).floor() as i64).max(scissor[1]);
    let max_y = (a.1.max(b.1).max(c.1).ceil() as i64).min(scissor[3]);
    if max_x <= min_x || max_y <= min_y {
        return;
    }
    // w0 weights `a` (edge b→c), w1 weights `b` (edge c→a), w2 weights `c` (edge a→b)
    let tl = [is_top_left(b, c), is_top_left(c, a), is_top_left(a, b)];
    let col = |v: &Vertex| v.color.to_array().map(|x| x as f32 / 255.0);
    let (ca, cb, cc) = (col(va), col(vb), col(vc));
    let inv_area = 1.0 / area;
    for py in min_y..max_y {
        let y = py as f32 + 0.5;
        for px in min_x..max_x {
            let x = px as f32 + 0.5;
            let w0 = edge(b, c, x, y);
            let w1 = edge(c, a, x, y);
            let w2 = edge(a, b, x, y);
            let inside = (w0 > 0.0 || (w0 == 0.0 && tl[0])) && (w1 > 0.0 || (w1 == 0.0 && tl[1])) && (w2 > 0.0 || (w2 == 0.0 && tl[2]));
            if !inside {
                continue;
            }
            // the weights sum to exactly 1 so flat colours stay flat
            let (l0, l1) = (w0 * inv_area, w1 * inv_area);
            let l2 = 1.0 - l0 - l1;
            let mut src = [0.0f32; 4];
            for ch in 0..4 {
                src[ch] = l0 * ca[ch] + l1 * cb[ch] + l2 * cc[ch];
            }
            if let Some(t) = tex {
                let u = l0 * va.uv.x + l1 * vb.uv.x + l2 * vc.uv.x;
                let vv = l0 * va.uv.y + l1 * vb.uv.y + l2 * vc.uv.y;
                let s = t.sample(u, vv);
                for ch in 0..4 {
                    src[ch] *= s[ch];
                }
            }
            if src[3] <= 0.0 && src[0] <= 0.0 && src[1] <= 0.0 && src[2] <= 0.0 {
                continue;
            }
            canvas.blend(px as usize, py as usize, src);
        }
    }
}

/// `rect` (points) as canvas pixel bounds relative to the crop origin.
pub fn rect_to_pixels(rect: Rect, ppp: f32, crop: Rect) -> [i64; 4] {
    let ox = (crop.min.x * ppp).round() as i64;
    let oy = (crop.min.y * ppp).round() as i64;
    [
        (rect.min.x * ppp).round() as i64 - ox,
        (rect.min.y * ppp).round() as i64 - oy,
        (rect.max.x * ppp).round() as i64 - ox,
        (rect.max.y * ppp).round() as i64 - oy,
    ]
}

// ---------------------------------------------------------------------------
// Framebuffer capture (smoke run only)
// ---------------------------------------------------------------------------

/// Crop a viewport screenshot (physical pixels) to `rect` (logical points)
/// and save it opaque. Only the unattended smoke run uses this.
pub fn save_cropped(image: &Arc<ColorImage>, pixels_per_point: f32, rect: Rect, out_path: &Path) -> Result<(), String> {
    let [w, h] = image.size;
    let x0 = ((rect.min.x * pixels_per_point).round().max(0.0) as usize).min(w);
    let y0 = ((rect.min.y * pixels_per_point).round().max(0.0) as usize).min(h);
    let x1 = ((rect.max.x * pixels_per_point).round().max(0.0) as usize).min(w);
    let y1 = ((rect.max.y * pixels_per_point).round().max(0.0) as usize).min(h);
    if x1 <= x0 || y1 <= y0 {
        return Err("empty screenshot region".to_string());
    }
    let mut out = image::RgbaImage::new((x1 - x0) as u32, (y1 - y0) as u32);
    for y in y0..y1 {
        for x in x0..x1 {
            let p = image.pixels[y * w + x];
            out.put_pixel((x - x0) as u32, (y - y0) as u32, image::Rgba([p.r(), p.g(), p.b(), 255]));
        }
    }
    if let Some(parent) = out_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    out.save(out_path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Color32, CornerRadius, Stroke};

    fn alpha_at(c: &Canvas, x: usize, y: usize) -> u8 {
        c.rgba8(x, y)[3]
    }

    /// A filled rounded rectangle on the canvas: opaque inside, transparent
    /// outside (the canvas itself is see-through), anti-aliased corners.
    #[test]
    fn rect_on_transparent_canvas() {
        let mut off = Offscreen::with_default_fonts(1.0);
        let prims = off.render(Vec2::new(100.0, 60.0), 2, |ctx| {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
                ui.painter().rect(Rect::from_min_max(Pos2::new(10.0, 10.0), Pos2::new(90.0, 50.0)), CornerRadius::same(6), Color32::from_rgb(40, 40, 40), Stroke::NONE, egui::StrokeKind::Inside);
            });
        });
        let canvas = off.rasterize(&prims, Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 60.0))).unwrap();
        assert_eq!(canvas.rgba8(0, 0), [0, 0, 0, 0], "outside the rect stays transparent");
        assert_eq!(canvas.rgba8(5, 30), [0, 0, 0, 0]);
        assert_eq!(canvas.rgba8(50, 30), [40, 40, 40, 255], "inside is the fill, opaque");
        assert_eq!(canvas.rgba8(10, 30), [40, 40, 40, 255], "the straight edge is crisp");
        assert_eq!(canvas.rgba8(9, 30), [0, 0, 0, 0]);
        assert_eq!(alpha_at(&canvas, 10, 10), 0, "the corner pixel is cut off by the radius");
        let partial = (10..16).flat_map(|x| (10..16).map(move |y| (x, y))).map(|(x, y)| alpha_at(&canvas, x, y)).filter(|a| *a > 0 && *a < 255).count();
        assert!(partial >= 3, "rounded corners are anti-aliased, got {} partial pixels", partial);
    }

    /// Two triangles sharing an edge (every rectangle egui draws) never
    /// blend a pixel twice: a 50 % fill stays exactly 50 % along the diagonal.
    #[test]
    fn shared_edges_are_drawn_once() {
        let off = Offscreen::with_default_fonts(1.0);
        let mut mesh = egui::Mesh::default();
        let c = Color32::from_rgba_premultiplied(0, 0, 0, 128);
        mesh.add_rect_with_uv(Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(20.0, 20.0)), Rect::from_min_max(egui::epaint::WHITE_UV, egui::epaint::WHITE_UV), c);
        let prims = vec![ClippedPrimitive { clip_rect: Rect::EVERYTHING, primitive: Primitive::Mesh(mesh) }];
        let canvas = off.rasterize(&prims, Rect::from_min_size(Pos2::ZERO, Vec2::new(20.0, 20.0))).unwrap();
        let expected = 128.0 / 255.0;
        for i in 0..20 {
            let (d, ad) = (canvas.pixels[i * 20 + i][3], canvas.pixels[i * 20 + (19 - i)][3]);
            assert!((d - expected).abs() < 1e-5, "diagonal pixel {} blended twice: {}", i, d);
            assert!((ad - expected).abs() < 1e-5, "anti-diagonal pixel {} blended twice: {}", i, ad);
        }
    }

    /// Text goes through the font atlas texture.
    #[test]
    fn text_is_rasterised() {
        let mut off = Offscreen::with_default_fonts(2.0);
        let prims = off.render(Vec2::new(120.0, 40.0), 2, |ctx| {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
                ui.painter().text(Pos2::new(4.0, 4.0), egui::Align2::LEFT_TOP, "Damage", egui::FontId::proportional(14.0), Color32::WHITE);
            });
        });
        let canvas = off.rasterize(&prims, Rect::from_min_size(Pos2::ZERO, Vec2::new(120.0, 40.0))).unwrap();
        assert_eq!(canvas.width, 240);
        let covered = canvas.pixels.iter().filter(|p| p[3] > 0.5).count();
        assert!(covered > 100, "text should cover pixels, got {}", covered);
        assert_eq!(canvas.rgba8(230, 70), [0, 0, 0, 0], "away from the text the canvas is transparent");
    }

    /// The cut edge of a half export gets anti-aliased rounded corners.
    #[test]
    fn round_corners_fades_the_corner_squares() {
        let mut canvas = Canvas::new(40, 30);
        for p in canvas.pixels.iter_mut() {
            *p = [0.2, 0.2, 0.2, 1.0];
        }
        canvas.round_corners(&[[0, 0, 40, 30]], 6.0);
        assert_eq!(alpha_at(&canvas, 0, 0), 0);
        assert_eq!(alpha_at(&canvas, 39, 0), 0);
        assert_eq!(alpha_at(&canvas, 0, 29), 0);
        assert_eq!(alpha_at(&canvas, 39, 29), 0);
        assert_eq!(alpha_at(&canvas, 20, 15), 255, "the middle is untouched");
        assert_eq!(alpha_at(&canvas, 6, 0), 255, "past the radius the edge is untouched");
        assert_eq!(alpha_at(&canvas, 0, 6), 255);
        let a = alpha_at(&canvas, 1, 1);
        assert!(a > 0 && a < 255, "the corner is anti-aliased, got {}", a);
        // the fade is symmetric
        assert_eq!(alpha_at(&canvas, 1, 1), alpha_at(&canvas, 38, 28));
        assert_eq!(alpha_at(&canvas, 2, 0), alpha_at(&canvas, 0, 2));
    }

    #[test]
    fn rect_to_pixels_is_relative_to_the_crop() {
        let crop = Rect::from_min_max(Pos2::new(10.0, 20.0), Pos2::new(110.0, 220.0));
        let r = Rect::from_min_max(Pos2::new(10.0, 30.0), Pos2::new(60.0, 40.0));
        assert_eq!(rect_to_pixels(r, 1.0, crop), [0, 10, 50, 20]);
        assert_eq!(rect_to_pixels(r, 2.0, crop), [0, 20, 100, 40]);
    }
}
