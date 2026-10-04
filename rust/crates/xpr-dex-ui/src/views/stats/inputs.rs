//! The Stats tab's small integer inputs (Solodex's `<input type="number">`
//! fields for Level, DVs, IVs, EVs and Stat Exp).

use egui::{Align, Color32, CornerRadius, FontId, Id, Key, Margin, Modifiers, Rect, Stroke, StrokeKind, TextEdit, Ui};

use crate::palette;

/// What typing a number outside the field's range does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Range {
    /// the value is clamped (`Math.max(0, Math.min(max, parseInt(v) || 0))`);
    /// an empty field is 0
    Clamp,
    /// the typed value is ignored unless it is in range (`if (v >= 1 && v <= 100)`);
    /// the field snaps back
    Reject,
}

#[derive(Clone)]
struct IntState {
    text: String,
    /// the value as of the last frame, to tell an outside change from our own
    last: i32,
}

/// Digits only, saturating at i64.
fn parse_digits(s: &str) -> Option<i64> {
    if s.is_empty() {
        return None;
    }
    Some(s.bytes().fold(0_i64, |acc, b| acc.saturating_mul(10).saturating_add((b - b'0') as i64)))
}

/// An integer input drawn in `rect` (bg gray-700, right-aligned text). Up /
/// Down step by one while it has focus. Returns whether `value` changed.
#[allow(clippy::too_many_arguments)]
pub fn int_field(ui: &mut Ui, id: Id, rect: Rect, value: &mut i32, min: i32, max: i32, font: FontId, range: Range, enabled: bool) -> bool {
    let edit_id = id.with("edit");
    let mut st: IntState = ui.data(|d| d.get_temp(id)).unwrap_or_else(|| IntState { text: value.to_string(), last: *value });
    let focused_before = ui.memory(|m| m.has_focus(edit_id));
    if !focused_before || st.last != *value {
        st.text = value.to_string();
    }
    let mut changed = false;
    if enabled && focused_before {
        let (up, down) = ui.input_mut(|i| (i.consume_key(Modifiers::NONE, Key::ArrowUp), i.consume_key(Modifiers::NONE, Key::ArrowDown)));
        if up || down {
            let next = (*value + if up { 1 } else { -1 }).clamp(min, max);
            if next != *value {
                *value = next;
                changed = true;
            }
            st.text = value.to_string();
        }
    }
    ui.painter().rect_filled(rect, CornerRadius::same(4), if enabled { palette::GRAY_700 } else { palette::GRAY_800 });
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(Align::Center)));
    let out = TextEdit::singleline(&mut st.text)
        .id(edit_id)
        .font(font)
        .text_color(if enabled { Color32::WHITE } else { palette::GRAY_500 })
        .background_color(Color32::TRANSPARENT)
        .frame(false)
        .margin(Margin::symmetric(4, 0))
        .desired_width(rect.width())
        .horizontal_align(Align::RIGHT)
        .vertical_align(Align::Center)
        .interactive(enabled)
        .show(&mut child);
    if enabled && out.response.has_focus() {
        ui.painter().rect_stroke(rect, CornerRadius::same(4), Stroke::new(1.0_f32, palette::GRAY_500), StrokeKind::Inside);
    }
    if enabled && out.response.changed() {
        st.text.retain(|c| c.is_ascii_digit());
        match (range, parse_digits(&st.text)) {
            (Range::Clamp, parsed) => {
                let n = parsed.unwrap_or(0);
                let c = n.clamp(min as i64, max as i64) as i32;
                if parsed.is_some() && c as i64 != n {
                    st.text = c.to_string();
                }
                if c != *value {
                    *value = c;
                    changed = true;
                }
            }
            (Range::Reject, Some(n)) if n >= min as i64 && n <= max as i64 => {
                if n as i32 != *value {
                    *value = n as i32;
                    changed = true;
                }
            }
            // out of range: the field snaps back to the value in use
            (Range::Reject, Some(_)) => st.text = value.to_string(),
            // cleared: left empty until something is typed or the field is left
            (Range::Reject, None) => {}
        }
    }
    st.last = *value;
    ui.data_mut(|d| d.insert_temp(id, st));
    changed
}
