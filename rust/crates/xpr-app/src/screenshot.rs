//! Screenshot export (`_grab_transparent` / `widget.render(pixmap)` in the Qt
//! app): the widget is drawn into a private egui context on a transparent
//! canvas of any size, tessellated, and rasterised here in software. The PNG
//! therefore holds only the widget — no window background between the cards,
//! no menus, tooltips or hover states, and no scroll viewport cutting the
//! content off — with the same anti-aliased rounded corners the cards have
//! on screen. The framebuffer capture (`ViewportCommand::Screenshot`) is
//! only kept for the unattended smoke run.

use std::path::Path;
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
    /// The active tab of the route-compare page.
    Compare,
    /// The world map (`map/export.rs`'s synchronous path), current view at
    /// 1× with the current toggles: `XPR_SMOKE_EXPORT=map`.
    Map,
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
            ShotKind::Map => "map".to_string(),
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

// The offscreen context and its software rasteriser live in xpr-ui-kit so
// other crates (the Dex page's examples) can render PNGs headless too.
pub use xpr_ui_kit::offscreen::{rect_to_pixels, Canvas, Offscreen};

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
