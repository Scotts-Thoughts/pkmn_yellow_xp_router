//! The Misc calculators' shared controls and styles (Solodex `misc/uiStyles.ts`
//! and `misc/NumberField.tsx`): the violet accent, pills, range sliders, the
//! never-NaN number field, cards and caps labels.

use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Galley, Id, Key, Pos2, Rect, Response, Sense,
    Stroke, StrokeKind, Ui, Vec2,
};
use xpr_ui_kit::theme::Theme;

use super::format::js_string;
use super::hit_probability::clamp;
use crate::palette::{self, px, px_bold};

/// The Misc area's colour (violet-500).
pub const MISC_ACCENT: Color32 = Color32::from_rgb(0x8b, 0x5c, 0xf6);

/// rgba(139,92,246,0.08): the tinted card behind headline results.
pub fn accent_tint() -> Color32 {
    Color32::from_rgba_unmultiplied(139, 92, 246, 20)
}

/// rgba(139,92,246,0.15): the highlighted table row.
pub fn accent_row_tint() -> Color32 {
    Color32::from_rgba_unmultiplied(139, 92, 246, 38)
}

/// `bg-gray-900/40`
pub fn card_fill() -> Color32 {
    Color32::from_rgba_unmultiplied(17, 24, 39, 102)
}

/// `rgba(17,24,39,0.5)`: the plain stat card.
pub fn stat_card_fill() -> Color32 {
    Color32::from_rgba_unmultiplied(17, 24, 39, 128)
}

/// Width of the calculators' column (`max-w-3xl`, including the 24 px padding).
pub const COLUMN_MAX_W: f32 = 768.0;
pub const COLUMN_PAD: f32 = 24.0;

/// Pieces of text in different fonts / colours, laid out on one line.
pub fn rich_line(ui: &Ui, parts: &[(&str, FontId, Color32)]) -> Arc<Galley> {
    let mut job = LayoutJob::default();
    for (text, font, color) in parts {
        job.append(
            text,
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: *color,
                ..Default::default()
            },
        );
    }
    ui.fonts_mut(|f| f.layout_job(job))
}

/// `LABEL_CLS`: xs, semibold, gray-400, uppercase.
pub fn caps_label(ui: &mut Ui, theme: &Theme, text: &str) {
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .font(px_bold(theme, 12.0))
            .color(palette::GRAY_400),
    );
}

/// The chart / table card headings: xs, semibold, gray-500, uppercase.
pub fn caps_label_gray500(ui: &mut Ui, theme: &Theme, text: &str) {
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .font(px_bold(theme, 12.0))
            .color(palette::GRAY_500),
    );
}

/// A rounded bordered card (`rounded-lg border bg-... p-N`) as a frame.
pub fn card(fill: Color32, border: Color32, pad: i8) -> egui::Frame {
    egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, border))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(pad))
}

/// `pillCls`: a pill-style toggle (quick accuracies, modifier chips, segmented
/// choices). `note` is drawn after the label at 70% opacity.
pub fn pill(
    ui: &mut Ui,
    theme: &Theme,
    label: &str,
    note: Option<&str>,
    active: bool,
    disabled: bool,
) -> Response {
    let font = px_bold(theme, 12.0);
    let (fill, border, text) = if disabled {
        (palette::GRAY_900, palette::GRAY_800, palette::GRAY_600)
    } else if active {
        (palette::VIOLET_600, MISC_ACCENT, Color32::WHITE)
    } else {
        (palette::GRAY_800, palette::GRAY_700, palette::GRAY_300)
    };
    let note_color = Color32::from_rgba_unmultiplied(
        text.r(),
        text.g(),
        text.b(),
        (text.a() as f32 * 0.7) as u8,
    );
    let mut parts: Vec<(&str, FontId, Color32)> = vec![(label, font.clone(), text)];
    if let Some(n) = note {
        parts.push((" ", font.clone(), text));
        parts.push((n, font.clone(), note_color));
    }
    let galley = rich_line(ui, &parts);
    let size = Vec2::new(galley.size().x + 20.0 + 2.0, 26.0);
    let (rect, resp) = ui.allocate_exact_size(
        size,
        if disabled {
            Sense::hover()
        } else {
            Sense::click()
        },
    );
    let border = if !disabled && !active && resp.hovered() {
        palette::GRAY_500
    } else {
        border
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(4),
        fill,
        Stroke::new(1.0_f32, border),
        StrokeKind::Inside,
    );
    ui.painter().galley(
        Pos2::new(rect.min.x + 11.0, rect.center().y - galley.size().y / 2.0),
        galley,
        text,
    );
    if disabled {
        resp.on_hover_cursor(egui::CursorIcon::NotAllowed)
    } else {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    }
}

/// `<label><input type=checkbox class="accent-violet-500"> <b>label</b> note</label>`:
/// the whole row toggles. Returns true when it was clicked.
pub fn accent_checkbox_row(
    ui: &mut Ui,
    theme: &Theme,
    checked: bool,
    label: &str,
    note: &str,
) -> bool {
    let bold = px_bold(theme, 14.0);
    let reg = px(theme, 14.0);
    let mut job = LayoutJob::default();
    job.append(
        label,
        0.0,
        TextFormat {
            font_id: bold,
            color: palette::GRAY_200,
            ..Default::default()
        },
    );
    // flex gap-2 between the two spans
    job.append(
        note,
        8.0,
        TextFormat {
            font_id: reg,
            color: palette::GRAY_500,
            ..Default::default()
        },
    );
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(16.0 + 8.0 + galley.size().x, 20.0),
        Sense::click(),
    );
    let b = Rect::from_center_size(
        Pos2::new(rect.min.x + 8.0, rect.center().y),
        Vec2::splat(16.0),
    );
    let p = ui.painter();
    if checked {
        p.rect_filled(b, CornerRadius::same(3), MISC_ACCENT);
        p.add(egui::Shape::line(
            vec![
                Pos2::new(b.min.x + 3.5, b.center().y + 0.5),
                Pos2::new(b.min.x + 6.5, b.max.y - 4.0),
                Pos2::new(b.max.x - 3.5, b.min.y + 4.5),
            ],
            Stroke::new(2.0_f32, Color32::WHITE),
        ));
    } else {
        p.rect(
            b,
            CornerRadius::same(3),
            palette::GRAY_800,
            Stroke::new(
                1.0_f32,
                if resp.hovered() {
                    palette::GRAY_300
                } else {
                    palette::GRAY_500
                },
            ),
            StrokeKind::Inside,
        );
    }
    p.galley(
        Pos2::new(rect.min.x + 24.0, rect.center().y - galley.size().y / 2.0),
        galley,
        Color32::WHITE,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

/// A native-looking range slider (`<input type=range>` with `accent-violet-500`)
/// over `min..=max`, stepping by 1. Returns the new value when the user moved it.
pub fn range_slider(
    ui: &mut Ui,
    id: Id,
    width: f32,
    value: f64,
    min: f64,
    max: f64,
) -> Option<f64> {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width.max(40.0), 20.0), Sense::hover());
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    let r = 8.0;
    let x0 = rect.min.x + r;
    let x1 = (rect.max.x - r).max(x0 + 1.0);
    let span = max - min;
    let frac = if span > 0.0 {
        ((value - min) / span).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    let tx = x0 + frac * (x1 - x0);
    let cy = rect.center().y;
    let p = ui.painter();
    p.rect_filled(
        Rect::from_min_max(Pos2::new(x0, cy - 2.0), Pos2::new(x1, cy + 2.0)),
        CornerRadius::same(2),
        palette::GRAY_600,
    );
    p.rect_filled(
        Rect::from_min_max(Pos2::new(x0, cy - 2.0), Pos2::new(tx.max(x0), cy + 2.0)),
        CornerRadius::same(2),
        MISC_ACCENT,
    );
    let hot = resp.hovered() || resp.dragged();
    p.circle_filled(
        Pos2::new(tx, cy),
        if hot { r } else { r - 1.0 },
        MISC_ACCENT,
    );
    p.circle_stroke(
        Pos2::new(tx, cy),
        if hot { r } else { r - 1.0 },
        Stroke::new(1.0_f32, Color32::from_white_alpha(60)),
    );
    if resp.has_focus() {
        p.circle_stroke(
            Pos2::new(tx, cy),
            r + 2.0,
            Stroke::new(1.5_f32, palette::VIOLET_400),
        );
    }
    let mut new = None;
    if resp.is_pointer_button_down_on() || resp.dragged() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let f = ((pos.x - x0) / (x1 - x0)).clamp(0.0, 1.0) as f64;
            let v = (min + f * span).round().clamp(min, max.max(min));
            if v != value {
                new = Some(v);
            }
        }
    }
    if resp.has_focus() {
        // arrow keys while the slider itself has keyboard focus
        let (down, up) = ui.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, Key::ArrowLeft)
                    || i.consume_key(egui::Modifiers::NONE, Key::ArrowDown),
                i.consume_key(egui::Modifiers::NONE, Key::ArrowRight)
                    || i.consume_key(egui::Modifiers::NONE, Key::ArrowUp),
            )
        });
        if down {
            new = Some((value - 1.0).max(min));
        } else if up {
            new = Some((value + 1.0).min(max));
        }
    }
    new.filter(|v| *v != value)
}

// ---- number field -----------------------------------------------------------------------------

/// `parseInt(s, 10)` / `parseFloat(s)`: the longest numeric prefix, or None.
pub fn parse_js_number(s: &str, integer: bool) -> Option<f64> {
    let s = s.trim_start();
    let b = s.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let int_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let int_digits = i - int_start;
    if integer {
        if int_digits == 0 {
            return None;
        }
        return s[..i].parse::<f64>().ok();
    }
    let mut frac_digits = 0;
    if i < b.len() && b[i] == b'.' {
        let mut j = i + 1;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        frac_digits = j - i - 1;
        if int_digits > 0 || frac_digits > 0 {
            i = j;
        }
    }
    if int_digits == 0 && frac_digits == 0 {
        return None;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        let ds = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > ds {
            i = j;
        }
    }
    let t = &s[..i];
    // "5." and ".5" are fine for JS, Rust wants a digit on both sides in some forms
    let t = t.strip_suffix('.').unwrap_or(t);
    t.parse::<f64>().ok()
}

#[derive(Clone)]
struct NumState {
    text: String,
    /// the value as of the last frame, to tell an outside change from our own
    last: f64,
}

/// Solodex's `NumberField`: a numeric input that is impossible to drive into a
/// NaN. It keeps a text buffer so the field can be cleared or half typed, but
/// only ever hands out a clamped, valid number; `value` stays the source of
/// truth. Up / Down step the value while the field has focus. Returns whether
/// `value` changed.
pub fn number_field(
    ui: &mut Ui,
    theme: &Theme,
    id: Id,
    value: &mut f64,
    min: f64,
    max: f64,
    integer: bool,
    step: f64,
) -> bool {
    let edit_id = id.with("edit");
    let mut st: NumState = ui.data(|d| d.get_temp(id)).unwrap_or_else(|| NumState {
        text: js_string(*value),
        last: *value,
    });
    let mut changed = false;
    let focused_before = ui.memory(|m| m.has_focus(edit_id));
    // resync the buffer when the value moved from outside (k clamped when n
    // shrinks); during normal typing the parsed buffer already equals `value`
    if st.last != *value {
        let normalized = parse_js_number(&st.text, integer).map(|p| clamp(p, min, max));
        if normalized != Some(*value) {
            st.text = js_string(*value);
        }
        st.last = *value;
    }
    let font = px(theme, 14.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(64.0, 30.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(4), palette::GRAY_800);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(Vec2::new(8.0, 0.0)))
            .layout(egui::Layout::left_to_right(Align::Center)),
    );
    if focused_before {
        let (up, down) = ui.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, Key::ArrowUp),
                i.consume_key(egui::Modifiers::NONE, Key::ArrowDown),
            )
        });
        if up || down {
            let cur = parse_js_number(&st.text, integer)
                .map(|p| clamp(p, min, max))
                .unwrap_or(*value);
            let next = clamp(cur + if up { step } else { -step }, min, max);
            st.text = js_string(next);
            if next != *value {
                *value = next;
                changed = true;
            }
        }
    }
    let out = egui::TextEdit::singleline(&mut st.text)
        .id(edit_id)
        .font(font)
        .text_color(Color32::WHITE)
        .background_color(Color32::TRANSPARENT)
        .frame(false)
        .margin(egui::Margin::ZERO)
        .desired_width(rect.width() - 16.0)
        .horizontal_align(Align::RIGHT)
        .vertical_align(Align::Center)
        .show(&mut child);
    let has_focus = out.response.has_focus();
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(
            1.0_f32,
            if has_focus {
                MISC_ACCENT
            } else {
                palette::GRAY_600
            },
        ),
        StrokeKind::Inside,
    );
    if out.response.changed() {
        // what a number input lets through
        st.text
            .retain(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'));
        if let Some(v) = parse_js_number(&st.text, integer) {
            let c = clamp(v, min, max);
            if c != *value {
                *value = c;
                changed = true;
            }
        }
    }
    if focused_before && !has_focus {
        // blur: show the clamped number that is actually in use
        st.text = match parse_js_number(&st.text, integer) {
            Some(v) => js_string(clamp(v, min, max)),
            None => js_string(*value),
        };
    }
    st.last = *value;
    ui.data_mut(|d| d.insert_temp(id, st));
    changed
}

/// Right-aligned or left-aligned text at a point.
pub fn text_at(
    ui: &Ui,
    pos: Pos2,
    align: Align2,
    text: &str,
    font: FontId,
    color: Color32,
) -> Rect {
    ui.painter().text(pos, align, text, font, color)
}
