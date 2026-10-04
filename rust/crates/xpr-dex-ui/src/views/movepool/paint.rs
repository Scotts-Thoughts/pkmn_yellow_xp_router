//! Small painting helpers of the movepool views.

use egui::text::{LayoutJob, TextFormat};
use egui::{Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};
use xpr_ui_kit::theme::Theme;

use crate::palette;
use crate::widgets::text_w;

/// Paint `text` anchored at `pos` by `align`; returns the text's rect. With
/// `strike` the text is struck through (Solodex's crossed-out moves).
pub fn text_at(
    ui: &Ui,
    pos: Pos2,
    align: Align2,
    text: &str,
    font: FontId,
    color: Color32,
    strike: bool,
) -> Rect {
    let mut job = LayoutJob::simple_singleline(text.to_string(), font, color);
    if strike {
        job.sections[0].format.strikethrough = Stroke::new(1.0_f32, color);
    }
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let rect = align.anchor_size(pos, galley.size());
    ui.painter().galley(rect.min, galley, color);
    rect
}

/// Italic text anchored like [`text_at`].
pub fn italic_at(
    ui: &Ui,
    pos: Pos2,
    align: Align2,
    text: &str,
    font: FontId,
    color: Color32,
) -> Rect {
    let mut job = LayoutJob::default();
    job.append(
        text,
        0.0,
        TextFormat {
            font_id: font,
            color,
            italics: true,
            ..Default::default()
        },
    );
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let rect = align.anchor_size(pos, galley.size());
    ui.painter().galley(rect.min, galley, color);
    rect
}

/// Width of italic text.
pub fn italic_w(ui: &Ui, text: &str, font: &FontId) -> f32 {
    let mut job = LayoutJob::default();
    job.append(
        text,
        0.0,
        TextFormat {
            font_id: font.clone(),
            color: Color32::WHITE,
            italics: true,
            ..Default::default()
        },
    );
    ui.fonts_mut(|f| f.layout_job(job)).size().x
}

/// A small text link ("Bulbapedia ↗"): gray, blue on hover. Returns the
/// click response.
pub fn link_button(ui: &mut Ui, theme: &Theme, text: &str) -> Response {
    let font = palette::px(theme, 12.0);
    let label = format!("{} \u{2197}", text);
    let w = text_w(ui, &label, &font);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 16.0), Sense::click());
    super::probe::record("link", "", 0, text, rect);
    let color = if resp.hovered() {
        palette::BLUE_400
    } else {
        palette::GRAY_500
    };
    ui.painter()
        .text(rect.left_center(), Align2::LEFT_CENTER, label, font, color);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}
