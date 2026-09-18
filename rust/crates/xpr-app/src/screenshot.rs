//! Screenshot export: a viewport capture cropped to a widget rectangle and
//! written as PNG (`take_screenshot`, `screenshot_event_list`, the battle
//! summary exports and the summary windows).

use std::path::PathBuf;
use std::sync::Arc;

use egui::{ColorImage, Rect};

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
}

/// A capture in flight: the rectangle is resolved on the frame that draws
/// with the export layout, then the viewport screenshot is requested.
#[derive(Clone, Debug)]
pub struct PendingShot {
    pub kind: ShotKind,
    pub viewport: egui::ViewportId,
    pub rect: Option<Rect>,
    pub out_path: PathBuf,
    pub requested: bool,
}

#[derive(Clone, Debug)]
pub struct ShotTag(pub u64);

/// Crop `image` (physical pixels) to `rect` (logical points) and save it.
pub fn save_cropped(image: &Arc<ColorImage>, pixels_per_point: f32, rect: Rect, out_path: &PathBuf, round_corners: Option<Vec<Rect>>) -> Result<(), String> {
    let [w, h] = image.size;
    let x0 = ((rect.min.x * pixels_per_point).round().max(0.0) as usize).min(w);
    let y0 = ((rect.min.y * pixels_per_point).round().max(0.0) as usize).min(h);
    let x1 = ((rect.max.x * pixels_per_point).round().max(0.0) as usize).min(w);
    let y1 = ((rect.max.y * pixels_per_point).round().max(0.0) as usize).min(h);
    if x1 <= x0 || y1 <= y0 {
        return Err("empty screenshot region".to_string());
    }
    let cw = x1 - x0;
    let ch = y1 - y0;
    let mut out = image::RgbaImage::new(cw as u32, ch as u32);
    for y in 0..ch {
        for x in 0..cw {
            let p = image.pixels[(y0 + y) * w + (x0 + x)];
            out.put_pixel(x as u32, y as u32, image::Rgba([p.r(), p.g(), p.b(), 255]));
        }
    }
    // `_round_container_corners`: clear the corner triangles of each container
    if let Some(rects) = round_corners {
        let radius = (6.0 * pixels_per_point).round() as i64;
        for r in rects {
            let rx0 = ((r.min.x * pixels_per_point).round() as i64 - x0 as i64).max(0);
            let ry0 = ((r.min.y * pixels_per_point).round() as i64 - y0 as i64).max(0);
            let rx1 = ((r.max.x * pixels_per_point).round() as i64 - x0 as i64).min(cw as i64);
            let ry1 = ((r.max.y * pixels_per_point).round() as i64 - y0 as i64).min(ch as i64);
            if rx1 <= rx0 || ry1 <= ry0 {
                continue;
            }
            for (cx, cy, sx, sy) in [(rx0, ry0, 1i64, 1i64), (rx1 - 1, ry0, -1, 1), (rx0, ry1 - 1, 1, -1), (rx1 - 1, ry1 - 1, -1, -1)] {
                let center = (cx + sx * (radius - 1), cy + sy * (radius - 1));
                for dy in 0..radius {
                    for dx in 0..radius {
                        let px = cx + sx * dx;
                        let py = cy + sy * dy;
                        if px < 0 || py < 0 || px >= cw as i64 || py >= ch as i64 {
                            continue;
                        }
                        let ddx = (px - center.0) as f32;
                        let ddy = (py - center.1) as f32;
                        if ddx * ddx + ddy * ddy > (radius as f32 - 0.5).powi(2) {
                            out.put_pixel(px as u32, py as u32, image::Rgba([0, 0, 0, 0]));
                        }
                    }
                }
            }
        }
    }
    if let Some(parent) = out_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    out.save(out_path).map_err(|e| e.to_string())
}
