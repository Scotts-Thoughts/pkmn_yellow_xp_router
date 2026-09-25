//! PNG export of a scope region at an integer scale
//! (`docs/rust_port/design/world_map/GRAPHICS_TOOLS_PLAN.md` WP-A, SPEC
//! Phase 5 step 4): the compositor's native-scale pixels, nearest-neighbour
//! upscaled like pokemap's export dialog (1–8×), with a size bound, progress
//! reporting and cancellation. No egui here; the marker / grid / route-path
//! overlays are drawn by `xpr-app`'s map module on top of the returned
//! pixmap.
//!
//! `render` never materialises the whole region at compositor scale: it
//! walks it in horizontal bands (`BAND_ROWS` world px tall), composites and
//! upscales one band at a time straight into the output pixmap, and skips
//! bands `Compositor::covers` says no map touches. That bounds peak memory
//! to one band instead of the full (up to `MAX_PIXELS`) output, and gives
//! `progress`/cancel a checkpoint between bands. Because nearest-neighbour
//! upscaling is a per-pixel, row-local operation, slicing the region into
//! bands and upscaling each independently produces byte-identical output to
//! upscaling the whole region in one shot (plan §3.4 "A": "no seams at band
//! boundaries") — see `tests/export.rs`.

use crate::compose::{Compositor, RenderOpts, BACKGROUND};
use crate::geom::{scope_rect, IRect, Scope};
use crate::lod::Pixmap;

/// Largest nearest-neighbour scale the exporter accepts (pokemap: 1–8×).
pub const MAX_SCALE: u32 = 8;
/// Upper bound on output pixels: 2^28 px is 1 GiB of RGBA in memory, which
/// leaves Emerald's whole world (137 Mpx) exportable at 1× and nothing
/// larger than that.
pub const MAX_PIXELS: u64 = 1 << 28;

/// World-pixel rows composited per band. Chosen so a band's pixmap (and its
/// upscaled footprint at `MAX_SCALE`) stay small compared to the whole
/// output, while keeping the per-band `Compositor` call cheap enough that
/// `progress`/cancel checkpoints land often.
const BAND_ROWS: i32 = 256;

/// Fully transparent RGBA: the fill `render` uses outside every map when
/// `ExportRequest::transparent` is set, instead of [`BACKGROUND`].
const TRANSPARENT: [u8; 4] = [0, 0, 0, 0];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportRequest {
    pub scope: Scope,
    /// The world-pixel region of the scope to export; clipped to the scope.
    pub rect: IRect,
    /// Output pixels per world pixel, `1..=MAX_SCALE`.
    pub scale: u32,
    pub opts: RenderOpts,
    /// Leave pixels no map covers transparent instead of the world background.
    pub transparent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportError {
    /// The region is empty (or entirely outside the scope).
    Empty,
    /// The output would exceed [`MAX_PIXELS`].
    TooLarge { pixels: u64, max: u64 },
    /// The scale is 0 or above [`MAX_SCALE`].
    BadScale(u32),
    /// The progress callback asked to stop.
    Cancelled,
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportError::Empty => write!(f, "nothing to export: the region is empty"),
            ExportError::TooLarge { pixels, max } => write!(f, "the image would have {:.0} Mpx; the limit is {:.0} Mpx", *pixels as f64 / 1e6, *max as f64 / 1e6),
            ExportError::BadScale(s) => write!(f, "scale {}× is not between 1× and {}×", s, MAX_SCALE),
            ExportError::Cancelled => write!(f, "export cancelled"),
        }
    }
}

impl std::error::Error for ExportError {}

impl ExportRequest {
    /// The request's region clipped to the scope's rectangle.
    pub fn clipped_rect(&self, comp: &Compositor) -> IRect {
        self.rect.intersect(&scope_rect(comp.pack(), self.scope))
    }
}

/// Output size in pixels of `rect` (world px) at `scale`.
pub fn output_size(rect: IRect, scale: u32) -> (u32, u32) {
    // saturating: `check` refuses anything near this long before it matters
    let side = |n: i32| (n.max(0) as u64).saturating_mul(scale as u64).min(u32::MAX as u64) as u32;
    (side(rect.width()), side(rect.height()))
}

/// Output pixel count, computed in `u64` throughout so an absurd rectangle
/// is refused by [`check`] instead of wrapping in `u32` first.
pub fn output_pixels(rect: IRect, scale: u32) -> u64 {
    let w = rect.width().max(0) as u64;
    let h = rect.height().max(0) as u64;
    w.saturating_mul(h).saturating_mul(scale as u64).saturating_mul(scale as u64)
}

/// Refuse empty regions, bad scales and outputs past [`MAX_PIXELS`] before
/// anything is allocated.
pub fn check(rect: IRect, scale: u32) -> Result<(), ExportError> {
    if scale == 0 || scale > MAX_SCALE {
        return Err(ExportError::BadScale(scale));
    }
    if rect.is_empty() {
        return Err(ExportError::Empty);
    }
    let pixels = output_pixels(rect, scale);
    if pixels > MAX_PIXELS {
        return Err(ExportError::TooLarge { pixels, max: MAX_PIXELS });
    }
    Ok(())
}

/// Nearest-neighbour upscale of `src` by `factor` into `dst` at
/// `(dst_x, dst_y)`, clipped to `dst`.
pub fn upscale_into(dst: &mut Pixmap, dst_x: usize, dst_y: usize, src: &Pixmap, factor: usize) {
    if factor == 0 {
        return;
    }
    for sy in 0..src.h {
        let src_row = &src.data[sy * src.w * 4..(sy + 1) * src.w * 4];
        for fy in 0..factor {
            let dy = dst_y + sy * factor + fy;
            if dy >= dst.h {
                break;
            }
            let row = &mut dst.data[dy * dst.w * 4..(dy + 1) * dst.w * 4];
            for sx in 0..src.w {
                let px = &src_row[sx * 4..sx * 4 + 4];
                for fx in 0..factor {
                    let dx = dst_x + sx * factor + fx;
                    if dx >= dst.w {
                        break;
                    }
                    row[dx * 4..dx * 4 + 4].copy_from_slice(px);
                }
            }
        }
    }
}

/// Render `req` to a pixmap, banded (see the module doc). `progress(fraction)`
/// is called once before any compositing and again after every band
/// (`fraction` is the share of rows completed so far, reaching exactly 1.0
/// after the last band); returning `false` stops the render and yields
/// [`ExportError::Cancelled`] without compositing further bands.
pub fn render(comp: &Compositor, req: &ExportRequest, progress: &mut dyn FnMut(f32) -> bool) -> Result<Pixmap, ExportError> {
    let rect = req.clipped_rect(comp);
    check(rect, req.scale)?; // (d) refuse before allocating anything
    if !progress(0.0) {
        return Err(ExportError::Cancelled);
    }

    let scale = req.scale as usize;
    let (out_w, out_h) = output_size(rect, req.scale);
    // (c) `transparent`: leave pixels no map covers at alpha 0 rather than
    // painting the world background; `fill` seeds both the output pixmap
    // (so skipped bands are already correct) and each band's own pixmap.
    let fill = if req.transparent { TRANSPARENT } else { BACKGROUND };
    let mut out = Pixmap::filled(out_w as usize, out_h as usize, fill);

    let total_rows = rect.height().max(0) as i64;
    let mut y = rect.y0;
    while y < rect.y1 {
        let band_h = BAND_ROWS.min(rect.y1 - y);
        let band_rect = IRect::new(rect.x0, y, rect.x1, y + band_h);
        // Bands no map of the scope touches stay exactly `fill`, which `out`
        // already has, so skip the compositor call entirely.
        if comp.covers(req.scope, &band_rect) {
            let band = comp.render_region_with_fill(req.scope, band_rect, req.opts, fill);
            let dst_y = (y - rect.y0) as usize * scale;
            upscale_into(&mut out, 0, dst_y, &band, scale);
        }
        y += band_h;
        let done = (y - rect.y0) as i64;
        if !progress((done as f64 / total_rows.max(1) as f64) as f32) {
            return Err(ExportError::Cancelled);
        }
    }
    Ok(out)
}
