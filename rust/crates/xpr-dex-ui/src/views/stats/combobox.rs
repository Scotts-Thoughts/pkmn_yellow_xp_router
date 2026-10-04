//! Solodex's `Combobox`: a searchable single-select with keyboard navigation.
//! Shows up to 80 options, filtered by label / sublabel as the user types.

use egui::{Align, Align2, Color32, CornerRadius, Id, Key, Modifiers, Pos2, Rect, Sense, Stroke, StrokeKind, TextEdit, Ui, Vec2};
use xpr_ui_kit::modal::behind_modal;
use xpr_ui_kit::theme::Theme;

use crate::palette::{self, px};
use crate::widgets::text_w;

/// The most rows the list shows.
pub const MAX_OPTIONS: usize = 80;

#[derive(Clone, Debug)]
pub struct ComboOption {
    pub id: String,
    pub label: String,
    pub sublabel: String,
    pub color: Option<Color32>,
    /// lower-case label and sublabel, for the filter
    search_label: String,
    search_sublabel: String,
}

impl ComboOption {
    pub fn new(id: impl Into<String>, label: impl Into<String>, sublabel: impl Into<String>, color: Option<Color32>) -> ComboOption {
        let label = label.into();
        let sublabel = sublabel.into();
        ComboOption { id: id.into(), search_label: label.to_lowercase(), search_sublabel: sublabel.to_lowercase(), label, sublabel, color }
    }
}

/// What a combobox remembers between frames.
#[derive(Clone, Debug, Default)]
pub struct ComboState {
    text: String,
    open: bool,
    hilite: usize,
    scroll_to_hilite: bool,
}

impl ComboState {
    /// The options that match the typed query (at most [`MAX_OPTIONS`]).
    fn filtered(&self, options: &[ComboOption]) -> Vec<usize> {
        let q = self.text.trim().to_lowercase();
        let it = options.iter().enumerate().filter(|(_, o)| q.is_empty() || o.search_label.contains(&q) || o.search_sublabel.contains(&q)).map(|(i, _)| i);
        it.take(MAX_OPTIONS).collect()
    }
}

/// Draw the combobox `width` wide (32 px high) showing `value` (or the typed
/// query while open). Returns the id of the option picked this frame.
#[allow(clippy::too_many_arguments)]
pub fn combobox(ui: &mut Ui, theme: &Theme, id: Id, st: &mut ComboState, value: &str, options: &[ComboOption], placeholder: &str, width: f32) -> Option<String> {
    let font = px(theme, 14.0);
    let edit_id = id.with("edit");
    let popup_id = id.with("popup");
    let focused_before = ui.memory(|m| m.has_focus(edit_id));
    if !focused_before && !st.open {
        // closed: the field shows the current value
        st.text = value.to_string();
    }
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 32.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(4), palette::GRAY_700);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(Vec2::new(10.0, 0.0))).layout(egui::Layout::left_to_right(Align::Center)));
    let out = TextEdit::singleline(&mut st.text)
        .id(edit_id)
        .font(font.clone())
        .text_color(Color32::WHITE)
        .background_color(Color32::TRANSPARENT)
        .frame(false)
        .margin(egui::Margin::ZERO)
        .desired_width(rect.width() - 20.0)
        .hint_text(egui::RichText::new(placeholder).font(font.clone()).color(palette::GRAY_500))
        .vertical_align(Align::Center)
        .clip_text(true)
        .show(&mut child);
    let resp = out.response;
    let has_focus = resp.has_focus();
    if has_focus {
        ui.painter().rect_stroke(rect, CornerRadius::same(4), Stroke::new(1.0_f32, palette::GRAY_500), StrokeKind::Inside);
    }
    if resp.gained_focus() {
        // focusing starts a fresh query
        st.text.clear();
        st.open = true;
        st.hilite = 0;
        ui.ctx().request_repaint();
    }
    if resp.changed() {
        st.open = true;
        st.hilite = 0;
    }
    let filtered = st.filtered(options);
    if st.hilite >= filtered.len() {
        st.hilite = filtered.len().saturating_sub(1);
    }

    let mut picked: Option<usize> = None;
    let mut escape = false;
    if has_focus || (focused_before && st.open) {
        // keys while the field itself has focus
        ui.input_mut(|i| {
            if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                st.open = true;
                st.hilite = (st.hilite + 1).min(filtered.len().saturating_sub(1));
                st.scroll_to_hilite = true;
            }
            if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                st.hilite = st.hilite.saturating_sub(1);
                st.scroll_to_hilite = true;
            }
            if i.consume_key(Modifiers::NONE, Key::Enter) && st.open {
                picked = filtered.get(st.hilite).copied();
            }
            if i.consume_key(Modifiers::NONE, Key::Escape) {
                escape = true;
            }
        });
        if escape {
            // closed again: the field shows the current value
            st.open = false;
            st.text = value.to_string();
            ui.memory_mut(|m| m.surrender_focus(edit_id));
        }
    }

    // the list, also on the frame a click on a row takes the field's focus away
    if st.open && (has_focus || focused_before) && !filtered.is_empty() {
        let area = egui::Area::new(popup_id).order(egui::Order::Foreground).fixed_pos(Pos2::new(rect.min.x, rect.max.y + 2.0)).constrain(true);
        let inner = area.show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(palette::GRAY_800)
                .stroke(Stroke::new(1.0_f32, palette::GRAY_600))
                .corner_radius(CornerRadius::same(4))
                .shadow(egui::Shadow { offset: [0, 8], blur: 20, spread: 0, color: Color32::from_black_alpha(120) })
                .show(ui, |ui| {
                    ui.set_width(rect.width() - 2.0);
                    let area = egui::ScrollArea::vertical().id_salt(popup_id.with("scroll")).max_height(208.0).auto_shrink([false, true]);
                    xpr_ui_kit::widgets::show_scroll(ui, area, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        let w = ui.available_width();
                        for (row, oi) in filtered.iter().enumerate() {
                            let opt = &options[*oi];
                            let (r, rresp) = ui.allocate_exact_size(Vec2::new(w, 32.0), Sense::click());
                            if row == st.hilite {
                                ui.painter().rect_filled(r, CornerRadius::ZERO, palette::GRAY_600);
                                if st.scroll_to_hilite {
                                    ui.scroll_to_rect(r, None);
                                }
                            } else if rresp.hovered() {
                                ui.painter().rect_filled(r, CornerRadius::ZERO, palette::GRAY_700);
                            }
                            let mut x = r.min.x + 12.0;
                            if let Some(c) = opt.color {
                                ui.painter().rect_filled(Rect::from_center_size(Pos2::new(x + 4.0, r.center().y), Vec2::splat(8.0)), CornerRadius::same(2), c);
                                x += 16.0;
                            }
                            let sub_w = if opt.sublabel.is_empty() { 0.0 } else { text_w(ui, &opt.sublabel, &px(theme, 12.0)) + 8.0 };
                            let label = xpr_ui_kit::widgets::elide(ui, &opt.label, &font, (r.max.x - 12.0 - sub_w - x).max(20.0));
                            ui.painter().text(Pos2::new(x, r.center().y), Align2::LEFT_CENTER, label, font.clone(), Color32::WHITE);
                            if !opt.sublabel.is_empty() {
                                ui.painter().text(Pos2::new(r.max.x - 12.0, r.center().y), Align2::RIGHT_CENTER, &opt.sublabel, px(theme, 12.0), palette::GRAY_400);
                            }
                            if rresp.clicked() {
                                picked = Some(*oi);
                            }
                        }
                    });
                });
        });
        st.scroll_to_hilite = false;
        // a click outside the field and the list closes it
        if !behind_modal(ui) && ui.input(|i| i.pointer.any_click()) {
            if let Some(p) = ui.input(|i| i.pointer.interact_pos()) {
                if !inner.response.rect.contains(p) && !rect.contains(p) {
                    st.open = false;
                }
            }
        }
    }
    if !has_focus && !focused_before {
        st.open = false;
    }
    if let Some(oi) = picked {
        st.open = false;
        st.text = options[oi].label.clone();
        ui.memory_mut(|m| m.surrender_focus(edit_id));
        return Some(options[oi].id.clone());
    }
    None
}
