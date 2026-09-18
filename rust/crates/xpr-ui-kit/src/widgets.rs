//! The egui counterparts of `gui_qt/components/custom_components.py` and the
//! handful of Qt controls the app styles by stylesheet (buttons, entries,
//! option menus, check boxes, group boxes, tabs, segmented toggles).
//!
//! Every widget is immediate-mode: it draws from the value it is given and
//! reports what changed. Per-widget transient state (a typed filter, an open
//! popup) lives in egui's memory keyed by the widget id.

use egui::{
    Align, Color32, CornerRadius, FontId, Id, Key, Layout, Modifiers, Pos2, Rect, Response, Sense, Stroke, TextEdit,
    TextStyle, Ui, Vec2, WidgetText,
};

use crate::theme::Theme;

// ---------------------------------------------------------------------------
// Scroll areas
// ---------------------------------------------------------------------------

/// Show `area` with its scrollbar handle painted in the border colour (the
/// Qt `QScrollBar::handle` rule). egui draws the handle with
/// `widgets.inactive.bg_fill`, which the theme sets only 2 % lighter than the
/// track, so the handle all but vanished. The override is scoped to the
/// scrollbar: the content ui gets the unmodified style back so check boxes,
/// radios and sliders inside keep their normal fill.
pub fn show_scroll<R>(
    ui: &mut Ui,
    area: egui::ScrollArea,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> egui::scroll_area::ScrollAreaOutput<R> {
    let saved = ui.style().clone();
    {
        let w = &mut ui.style_mut().visuals.widgets;
        let handle = w.inactive.bg_stroke.color;
        w.inactive.bg_fill = handle;
        w.hovered.bg_fill = crate::theme::lighten(handle, 0.15);
    }
    let out = area.show(ui, |ui| {
        ui.set_style(saved.clone());
        add_contents(ui)
    });
    ui.set_style(saved);
    out
}

// ---------------------------------------------------------------------------
// Labels
// ---------------------------------------------------------------------------

pub fn label(ui: &mut Ui, theme: &Theme, text: impl Into<String>) -> Response {
    ui.label(egui::RichText::new(text.into()).font(theme.body()).color(theme.text))
}

pub fn label_colored(ui: &mut Ui, theme: &Theme, text: impl Into<String>, color: Color32) -> Response {
    ui.label(egui::RichText::new(text.into()).font(theme.body()).color(color))
}

pub fn label_bold(ui: &mut Ui, theme: &Theme, text: impl Into<String>) -> Response {
    ui.label(egui::RichText::new(text.into()).font(theme.body_bold()).color(theme.text))
}

pub fn label_font(ui: &mut Ui, text: impl Into<String>, font: FontId, color: Color32) -> Response {
    ui.label(egui::RichText::new(text.into()).font(font).color(color))
}

/// A label whose text is elided with "…" to fit `max_width`.
pub fn label_elided(ui: &mut Ui, text: &str, font: FontId, color: Color32, max_width: f32) -> Response {
    let shown = elide(ui, text, &font, max_width);
    ui.label(egui::RichText::new(shown).font(font).color(color))
}

/// Qt `elidedText(..., ElideRight, width)`.
pub fn elide(ui: &Ui, text: &str, font: &FontId, max_width: f32) -> String {
    elide_with(ui, text, font, max_width, "…")
}

/// Hard-truncate to `max_width` with no ellipsis marker at all — for cramped
/// slots (the Mimic dropdown) where the "…" costs more room than it's worth.
pub fn truncate_plain(ui: &Ui, text: &str, font: &FontId, max_width: f32) -> String {
    elide_with(ui, text, font, max_width, "")
}

fn elide_with(ui: &Ui, text: &str, font: &FontId, max_width: f32, marker: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let full = text_width(ui, text, font);
    if full <= max_width {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut lo = 0usize;
    let mut hi = chars.len();
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        let candidate: String = chars[..mid].iter().collect::<String>() + marker;
        if text_width(ui, &candidate, font) <= max_width {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    if lo == 0 {
        return marker.to_string();
    }
    chars[..lo].iter().collect::<String>() + marker
}

pub fn text_width(ui: &Ui, text: &str, font: &FontId) -> f32 {
    ui.fonts_mut(|f| f.layout_no_wrap(text.to_string(), font.clone(), Color32::WHITE).size().x)
}

// ---------------------------------------------------------------------------
// Buttons
// ---------------------------------------------------------------------------

/// `QPushButton` with the theme's normal/hover/pressed/disabled/checked looks.
pub struct StyledButton<'a> {
    text: WidgetText,
    theme: &'a Theme,
    enabled: bool,
    checked: bool,
    min_size: Vec2,
    padding: Vec2,
    font: Option<FontId>,
    text_color: Option<Color32>,
    fill: Option<Color32>,
    hover_fill: Option<Color32>,
    corner_radius: CornerRadius,
    stroke: Option<Stroke>,
}

impl<'a> StyledButton<'a> {
    pub fn new(theme: &'a Theme, text: impl Into<WidgetText>) -> Self {
        StyledButton {
            text: text.into(),
            theme,
            enabled: true,
            checked: false,
            min_size: Vec2::new(0.0, 22.0),
            padding: Vec2::new(8.0, 2.0),
            font: None,
            text_color: None,
            fill: None,
            hover_fill: None,
            corner_radius: CornerRadius::same(3),
            stroke: None,
        }
    }

    pub fn enabled(mut self, v: bool) -> Self {
        self.enabled = v;
        self
    }

    pub fn checked(mut self, v: bool) -> Self {
        self.checked = v;
        self
    }

    pub fn min_size(mut self, s: Vec2) -> Self {
        self.min_size = s;
        self
    }

    pub fn fixed_width(mut self, w: f32) -> Self {
        self.min_size.x = w;
        self
    }

    pub fn padding(mut self, p: Vec2) -> Self {
        self.padding = p;
        self
    }

    pub fn font(mut self, f: FontId) -> Self {
        self.font = Some(f);
        self
    }

    pub fn text_color(mut self, c: Color32) -> Self {
        self.text_color = Some(c);
        self
    }

    pub fn fill(mut self, c: Color32) -> Self {
        self.fill = Some(c);
        self
    }

    pub fn hover_fill(mut self, c: Color32) -> Self {
        self.hover_fill = Some(c);
        self
    }

    pub fn corner_radius(mut self, r: impl Into<CornerRadius>) -> Self {
        self.corner_radius = r.into();
        self
    }

    pub fn stroke(mut self, s: Stroke) -> Self {
        self.stroke = Some(s);
        self
    }

    /// `QPushButton[class="large"]`: 6px 16px padding, 11 pt.
    pub fn large(mut self) -> Self {
        self.padding = Vec2::new(16.0, 6.0);
        self.font = Some(self.theme.font(11.0));
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let theme = self.theme;
        let font = self.font.clone().unwrap_or_else(|| theme.body());
        let galley = self.text.into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, font);
        let desired = Vec2::new(
            (galley.size().x + 2.0 * self.padding.x).max(self.min_size.x),
            (galley.size().y + 2.0 * self.padding.y).max(self.min_size.y),
        );
        let sense = if self.enabled { Sense::click() } else { Sense::hover() };
        let (rect, mut response) = ui.allocate_exact_size(desired, sense);
        if !self.enabled {
            response = response.on_hover_cursor(egui::CursorIcon::Default);
        }
        if ui.is_rect_visible(rect) {
            let hovered = self.enabled && response.hovered();
            let pressed = self.enabled && response.is_pointer_button_down_on();
            let (fill, stroke, text_color) = if !self.enabled {
                (theme.bg_darker, Stroke::new(1.0_f32, theme.subtle_border), theme.disabled_text)
            } else if self.checked {
                if hovered {
                    (theme.accent_hover, Stroke::new(1.0_f32, theme.accent_hover), Color32::WHITE)
                } else {
                    (theme.accent, Stroke::new(1.0_f32, theme.accent), Color32::WHITE)
                }
            } else if pressed {
                (theme.accent_pressed, Stroke::new(1.0_f32, theme.accent), Color32::WHITE)
            } else if hovered {
                (
                    self.hover_fill.unwrap_or(theme.hover_bg),
                    Stroke::new(1.0_f32, theme.accent),
                    self.text_color.unwrap_or(theme.text),
                )
            } else {
                (
                    self.fill.unwrap_or(theme.bg_lighter),
                    self.stroke.unwrap_or(Stroke::new(1.0_f32, theme.border)),
                    self.text_color.unwrap_or(theme.text),
                )
            };
            let painter = ui.painter();
            painter.rect(rect, self.corner_radius, fill, stroke, egui::StrokeKind::Inside);
            let text_pos = Pos2::new(
                rect.center().x - galley.size().x / 2.0,
                rect.center().y - galley.size().y / 2.0,
            );
            painter.galley(text_pos, galley, text_color);
        }
        response
    }
}

pub fn button(ui: &mut Ui, theme: &Theme, text: impl Into<WidgetText>) -> Response {
    StyledButton::new(theme, text).show(ui)
}

pub fn button_enabled(ui: &mut Ui, theme: &Theme, text: impl Into<WidgetText>, enabled: bool) -> Response {
    StyledButton::new(theme, text).enabled(enabled).show(ui)
}

/// `QPushButton[class="amount-btn"]`: 22x22, bold 10 pt, accent on hover.
pub fn amount_button(ui: &mut Ui, theme: &Theme, text: &str, enabled: bool) -> Response {
    StyledButton::new(theme, egui::RichText::new(text).font(theme.font_bold(10.0)))
        .enabled(enabled)
        .min_size(Vec2::new(22.0, 22.0))
        .padding(Vec2::ZERO)
        .corner_radius(CornerRadius::same(2))
        .hover_fill(theme.accent)
        .show(ui)
}

/// `QPushButton[class="seg-toggle"]`: square, bold, red left bar when checked.
pub fn seg_toggle(ui: &mut Ui, theme: &Theme, text: &str, checked: bool) -> Response {
    let font = theme.body_bold();
    let galley = ui.fonts_mut(|f| f.layout_no_wrap(text.to_string(), font, Color32::WHITE));
    let desired = Vec2::new(galley.size().x + 28.0, (galley.size().y + 8.0).max(24.0));
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click());
    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();
        let (fill, text_color) = if checked {
            (if hovered { theme.hover_bg } else { theme.bg_lighter }, Color32::WHITE)
        } else if hovered {
            (theme.hover_bg, theme.text)
        } else {
            (theme.bg_darker, theme.secondary)
        };
        let painter = ui.painter();
        painter.rect(rect, CornerRadius::ZERO, fill, Stroke::new(1.0_f32, theme.border), egui::StrokeKind::Inside);
        if checked {
            let bar = Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
            painter.rect_filled(bar, CornerRadius::ZERO, theme.failure);
        }
        let text_pos = Pos2::new(rect.center().x - galley.size().x / 2.0, rect.center().y - galley.size().y / 2.0);
        painter.galley(text_pos, galley, text_color);
    }
    response
}

// ---------------------------------------------------------------------------
// Entries
// ---------------------------------------------------------------------------

/// `QLineEdit`: bg_input fill, 1 px border (accent when focused), radius 2.
pub struct Entry<'a> {
    text: &'a mut String,
    theme: &'a Theme,
    width: Option<f32>,
    enabled: bool,
    hint: Option<String>,
    id: Option<Id>,
    center: bool,
    font: Option<FontId>,
    margin: egui::Margin,
    radius: u8,
    min_height: Option<f32>,
}

/// What an entry reported this frame.
#[derive(Clone, Debug, Default)]
pub struct EntryResponse {
    pub response: Option<Response>,
    pub changed: bool,
    pub enter_pressed: bool,
    pub tab_pressed: bool,
    pub backtab_pressed: bool,
    pub escape_pressed: bool,
    pub lost_focus: bool,
    pub has_focus: bool,
}

impl<'a> Entry<'a> {
    pub fn new(theme: &'a Theme, text: &'a mut String) -> Self {
        Entry { text, theme, width: None, enabled: true, hint: None, id: None, center: false, font: None, margin: egui::Margin::symmetric(4, 2), radius: 2, min_height: None }
    }

    /// Inner padding (default 4 × 2).
    pub fn margin(mut self, m: egui::Margin) -> Self {
        self.margin = m;
        self
    }

    /// Corner radius of the outline (default 2).
    pub fn corner_radius(mut self, r: u8) -> Self {
        self.radius = r;
        self
    }

    /// Minimum height of the control.
    pub fn min_height(mut self, h: f32) -> Self {
        self.min_height = Some(h);
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    pub fn enabled(mut self, e: bool) -> Self {
        self.enabled = e;
        self
    }

    pub fn hint(mut self, h: impl Into<String>) -> Self {
        self.hint = Some(h.into());
        self
    }

    pub fn id(mut self, id: Id) -> Self {
        self.id = Some(id);
        self
    }

    pub fn centered(mut self) -> Self {
        self.center = true;
        self
    }

    pub fn font(mut self, f: FontId) -> Self {
        self.font = Some(f);
        self
    }

    pub fn show(self, ui: &mut Ui) -> EntryResponse {
        let theme = self.theme;
        let id = self.id.unwrap_or_else(|| ui.next_auto_id());
        let focused_before = ui.memory(|m| m.has_focus(id));
        let mut edit = TextEdit::singleline(self.text)
            .id(id)
            .font(self.font.clone().unwrap_or_else(|| theme.body()))
            .text_color(if self.enabled { theme.text } else { theme.disabled_text })
            .background_color(if self.enabled { theme.bg_input } else { theme.bg_darker })
            .margin(self.margin)
            .interactive(self.enabled)
            .frame(false)
            .lock_focus(true)
            .vertical_align(Align::Center);
        let pad_x = (self.margin.left + self.margin.right) as f32;
        if let Some(w) = self.width {
            edit = edit.desired_width(w - pad_x);
        }
        if let Some(h) = self.min_height {
            edit = edit.min_size(Vec2::new(self.width.unwrap_or(0.0), h));
        }
        if let Some(h) = &self.hint {
            edit = edit.hint_text(egui::RichText::new(h.clone()).color(theme.secondary));
        }
        if self.center {
            edit = edit.horizontal_align(Align::Center);
        }
        let output = edit.show(ui);
        let response = output.response;
        let has_focus = response.has_focus();
        let stroke = if !self.enabled {
            Stroke::new(1.0_f32, theme.subtle_border)
        } else if has_focus {
            Stroke::new(1.0_f32, theme.accent)
        } else {
            Stroke::new(1.0_f32, theme.border)
        };
        ui.painter().rect_stroke(response.rect, CornerRadius::same(self.radius), stroke, egui::StrokeKind::Inside);
        let mut r = EntryResponse {
            changed: response.changed(),
            has_focus,
            lost_focus: response.lost_focus() || (focused_before && !has_focus),
            ..Default::default()
        };
        if has_focus || focused_before {
            ui.input_mut(|i| {
                if i.consume_key(Modifiers::NONE, Key::Enter) {
                    r.enter_pressed = true;
                }
                if i.consume_key(Modifiers::NONE, Key::Tab) {
                    r.tab_pressed = true;
                }
                if i.consume_key(Modifiers::SHIFT, Key::Tab) {
                    r.backtab_pressed = true;
                }
                if i.key_pressed(Key::Escape) {
                    r.escape_pressed = true;
                }
            });
        }
        r.response = Some(response);
        r
    }
}

pub fn entry(ui: &mut Ui, theme: &Theme, text: &mut String, width: Option<f32>) -> EntryResponse {
    let mut e = Entry::new(theme, text);
    if let Some(w) = width {
        e = e.width(w);
    }
    e.show(ui)
}

/// `QPlainTextEdit`
pub fn text_area(ui: &mut Ui, theme: &Theme, text: &mut String, id: Id, height: f32, enabled: bool) -> Response {
    text_area_styled(ui, theme, text, id, height, enabled, egui::Margin::same(3), 2)
}

/// `text_area` with explicit inner padding and outline radius; the fill is
/// the well colour when the padding is not the default 3 px (the notes
/// footer), `bg_input` otherwise.
#[allow(clippy::too_many_arguments)]
pub fn text_area_styled(ui: &mut Ui, theme: &Theme, text: &mut String, id: Id, height: f32, enabled: bool, margin: egui::Margin, radius: u8) -> Response {
    let pad_y = (margin.top + margin.bottom) as f32;
    let rows = ((height - pad_y) / (theme.body().size * 1.3)).floor().max(1.0) as usize;
    let fill = if margin == egui::Margin::same(3) { theme.bg_input } else { theme.well_bg() };
    let output = TextEdit::multiline(text)
        .id(id)
        .font(theme.body())
        .text_color(if enabled { theme.text } else { theme.disabled_text })
        .background_color(fill)
        .margin(margin)
        .desired_rows(rows)
        .desired_width(f32::INFINITY)
        .min_size(Vec2::new(0.0, height))
        .interactive(enabled)
        .frame(false)
        .lock_focus(false)
        .show(ui);
    let response = output.response;
    let stroke = if response.has_focus() {
        Stroke::new(1.0_f32, theme.accent)
    } else {
        Stroke::new(1.0_f32, theme.border)
    };
    ui.painter().rect_stroke(response.rect, CornerRadius::same(radius), stroke, egui::StrokeKind::Inside);
    response
}

// ---------------------------------------------------------------------------
// Option menu (plain combo box)
// ---------------------------------------------------------------------------

/// `SimpleOptionMenu`: a non-editable combo. Returns true when the selection
/// changed. The current value is kept even if it is not in `options` (Qt's
/// `set` ignores unknown values; here the caller decides).
pub fn option_menu(ui: &mut Ui, theme: &Theme, id: Id, current: &mut String, options: &[String], width: Option<f32>, enabled: bool) -> bool {
    option_menu_ex(ui, theme, id, current, options, width, enabled, true)
}

/// `option_menu` with control over how an over-long selection is shortened:
/// `ellipsis == false` hard-truncates instead of appending "…".
#[allow(clippy::too_many_arguments)]
pub fn option_menu_ex(ui: &mut Ui, theme: &Theme, id: Id, current: &mut String, options: &[String], width: Option<f32>, enabled: bool, ellipsis: bool) -> bool {
    let font = theme.body();
    let widest = options.iter().map(|o| text_width(ui, o, &font)).fold(0.0f32, f32::max);
    let cur_w = text_width(ui, current, &font);
    // egui's ComboBox treats `.width()` as a *minimum*: the button grows to fit
    // the selected text + arrow + padding. So the text must be elided against
    // the chrome egui actually adds, or the combo overflows its `w`.
    // Qt's combo chrome is ~30 px, so use a tighter horizontal padding than
    // the theme's 8 px buttons to leave room for the text in narrow combos.
    let pad_x = 5.0_f32;
    let chrome = {
        let sp = ui.spacing();
        2.0 * pad_x + sp.icon_spacing + sp.icon_width
    };
    // Qt's sizeHint sizes to the current selection (+ chrome), min 40.
    let natural = (cur_w + chrome).max(40.0);
    let w = width.unwrap_or_else(|| natural.min(widest + chrome).max(natural));
    let mut changed = false;
    ui.scope(|ui| {
        if !enabled {
            ui.disable();
        }
        ui.spacing_mut().button_padding.x = pad_x;
        let visuals = ui.visuals_mut();
        visuals.widgets.inactive.bg_fill = theme.bg_input;
        visuals.widgets.inactive.weak_bg_fill = theme.bg_input;
        visuals.widgets.inactive.corner_radius = CornerRadius::same(2);
        visuals.widgets.hovered.bg_fill = theme.bg_input;
        visuals.widgets.hovered.weak_bg_fill = theme.bg_input;
        visuals.widgets.hovered.corner_radius = CornerRadius::same(2);
        visuals.widgets.active.bg_fill = theme.bg_input;
        visuals.widgets.active.weak_bg_fill = theme.bg_input;
        visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, theme.text);
        visuals.widgets.active.corner_radius = CornerRadius::same(2);
        visuals.widgets.open.corner_radius = CornerRadius::same(2);
        let shown = if ellipsis { elide(ui, current, &font, (w - chrome).max(10.0)) } else { truncate_plain(ui, current, &font, (w - chrome).max(10.0)) };
        let selected_text = egui::RichText::new(shown).font(font.clone()).color(if enabled { theme.text } else { theme.disabled_text });
        egui::ComboBox::from_id_salt(id)
            .width(w)
            .selected_text(selected_text)
            .show_ui(ui, |ui| {
                ui.set_min_width(w.max(widest + 24.0));
                for opt in options {
                    let is_sel = *opt == *current;
                    let resp = ui.selectable_label(is_sel, egui::RichText::new(opt).font(font.clone()));
                    if resp.clicked() && !is_sel {
                        *current = opt.clone();
                        changed = true;
                    }
                }
            });
    });
    changed
}

// ---------------------------------------------------------------------------
// Searchable dropdown (editable QComboBox with a contains-filter completer)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
struct SearchState {
    typed: String,
    editing: bool,
    highlighted: usize,
    open: bool,
    /// Scroll the popup to the highlighted row on the next frame it is shown
    /// (set when the list opens on the current value).
    scroll_to_highlighted: bool,
}

/// What a searchable dropdown reported this frame.
#[derive(Clone, Debug)]
pub struct SearchableResponse {
    pub changed: bool,
    /// Enter was pressed (after committing the best match).
    pub enter_pressed: bool,
    pub tab_pressed: bool,
    pub backtab_pressed: bool,
    pub has_focus: bool,
    pub rect: Rect,
}

impl Default for SearchableResponse {
    fn default() -> Self {
        SearchableResponse { changed: false, enter_pressed: false, tab_pressed: false, backtab_pressed: false, has_focus: false, rect: Rect::NOTHING }
    }
}

fn best_match_index(text: &str, options: &[String]) -> Option<usize> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Some(i) = options.iter().position(|o| o.eq_ignore_ascii_case(text)) {
        return Some(i);
    }
    let lower = text.to_lowercase();
    if let Some(i) = options.iter().position(|o| o.to_lowercase().starts_with(&lower)) {
        return Some(i);
    }
    options.iter().position(|o| o.to_lowercase().contains(&lower))
}

fn matching_indices(text: &str, options: &[String]) -> Vec<usize> {
    let lower = text.trim().to_lowercase();
    if lower.is_empty() {
        return (0..options.len()).collect();
    }
    options
        .iter()
        .enumerate()
        .filter(|(_, o)| o.to_lowercase().contains(&lower))
        .map(|(i, _)| i)
        .collect()
}

/// `_make_searchable(combo)`: an editable combo whose typed text filters the
/// list (case-insensitive contains); Enter/Tab commit the highlighted (or
/// best) match and report so the caller can advance focus. Losing focus also
/// commits the best match (Qt keeps the raw text until the form is submitted).
pub struct SearchableDropdown<'a> {
    theme: &'a Theme,
    id: Id,
    current: &'a mut String,
    options: &'a [String],
    min_width: f32,
    max_width: f32,
    placeholder: Option<String>,
    enabled: bool,
    request_focus: bool,
}

impl<'a> SearchableDropdown<'a> {
    pub fn new(theme: &'a Theme, id: Id, current: &'a mut String, options: &'a [String]) -> Self {
        SearchableDropdown {
            theme,
            id,
            current,
            options,
            min_width: 120.0,
            max_width: 200.0,
            placeholder: None,
            enabled: true,
            request_focus: false,
        }
    }

    pub fn widths(mut self, min: f32, max: f32) -> Self {
        self.min_width = min;
        self.max_width = max;
        self
    }

    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = Some(p.into());
        self
    }

    pub fn enabled(mut self, e: bool) -> Self {
        self.enabled = e;
        self
    }

    /// Focus the entry (and select its text) this frame.
    pub fn request_focus(mut self, v: bool) -> Self {
        self.request_focus = v;
        self
    }

    pub fn show(self, ui: &mut Ui) -> SearchableResponse {
        let theme = self.theme;
        let font = theme.body();
        let state_id = self.id.with("search_state");
        let edit_id = self.id.with("edit");
        let popup_id = self.id.with("popup");
        let mut st: SearchState = ui.data(|d| d.get_temp(state_id)).unwrap_or_default();

        let widest = self.options.iter().map(|o| text_width(ui, o, &font)).fold(0.0f32, f32::max);
        let w = (widest + 30.0).clamp(self.min_width, self.max_width);

        let mut result = SearchableResponse::default();
        let focused_before = ui.memory(|m| m.has_focus(edit_id));
        if !st.editing {
            st.typed = self.current.clone();
        }
        // Opening on the untouched current value lists everything with that
        // value highlighted (a plain combo box), rather than filtering the
        // list down to the single option equal to the current text.
        let current_index = self.options.iter().position(|o| o == self.current).unwrap_or(0);
        if self.request_focus {
            ui.memory_mut(|m| m.request_focus(edit_id));
            st.editing = true;
            st.open = true;
            st.highlighted = current_index;
            st.scroll_to_highlighted = true;
            // select all so typing replaces the text
            let mut tes = TextEdit::load_state(ui.ctx(), edit_id).unwrap_or_default();
            let range = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(st.typed.chars().count()));
            tes.cursor.set_char_range(Some(range));
            TextEdit::store_state(ui.ctx(), edit_id, tes);
        }

        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 22.0), Sense::hover());
        result.rect = rect;
        // entry area (leaves 20 px for the arrow)
        let entry_rect = Rect::from_min_max(rect.min, Pos2::new(rect.max.x - 20.0, rect.max.y));
        let arrow_rect = Rect::from_min_max(Pos2::new(rect.max.x - 20.0, rect.min.y), rect.max);
        let bg = if self.enabled { theme.bg_input } else { theme.bg_darker };
        ui.painter().rect_filled(rect, CornerRadius::same(2), bg);

        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(entry_rect).layout(Layout::left_to_right(Align::Center)));
        let mut edit = TextEdit::singleline(&mut st.typed)
            .id(edit_id)
            .font(font.clone())
            .text_color(if self.enabled { theme.text } else { theme.disabled_text })
            .background_color(Color32::TRANSPARENT)
            .margin(egui::Margin::symmetric(6, 2))
            .desired_width(entry_rect.width() - 12.0)
            .interactive(self.enabled)
            .frame(false)
            .lock_focus(true)
            .clip_text(true)
            .vertical_align(Align::Center);
        if let Some(p) = &self.placeholder {
            edit = edit.hint_text(egui::RichText::new(p.clone()).color(theme.secondary));
        }
        let out = edit.show(&mut child);
        let response = out.response;
        let has_focus = response.has_focus();
        result.has_focus = has_focus;

        // arrow button
        let arrow_resp = ui.interact(arrow_rect, self.id.with("arrow"), Sense::click());
        {
            let painter = ui.painter();
            painter.line_segment([Pos2::new(arrow_rect.min.x, rect.min.y), Pos2::new(arrow_rect.min.x, rect.max.y)], Stroke::new(1.0_f32, theme.border));
            let c = arrow_rect.center();
            let color = if self.enabled { theme.text } else { theme.disabled_text };
            painter.add(egui::Shape::convex_polygon(
                vec![Pos2::new(c.x - 4.0, c.y - 2.0), Pos2::new(c.x + 4.0, c.y - 2.0), Pos2::new(c.x, c.y + 3.0)],
                color,
                Stroke::NONE,
            ));
        }
        let stroke = if !self.enabled {
            Stroke::new(1.0_f32, theme.subtle_border)
        } else if has_focus || response.hovered() || arrow_resp.hovered() {
            Stroke::new(1.0_f32, theme.accent)
        } else {
            Stroke::new(1.0_f32, theme.border)
        };
        ui.painter().rect_stroke(rect, CornerRadius::same(2), stroke, egui::StrokeKind::Inside);

        if response.gained_focus() {
            st.editing = true;
            st.open = true;
            st.highlighted = current_index;
            st.scroll_to_highlighted = true;
            // select all on focus
            let mut tes = TextEdit::load_state(ui.ctx(), edit_id).unwrap_or_default();
            let range = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(st.typed.chars().count()));
            tes.cursor.set_char_range(Some(range));
            TextEdit::store_state(ui.ctx(), edit_id, tes);
        }
        if arrow_resp.clicked() && self.enabled {
            st.open = !st.open;
            st.editing = true;
            if st.open {
                st.highlighted = current_index;
                st.scroll_to_highlighted = true;
            }
            ui.memory_mut(|m| m.request_focus(edit_id));
        }
        if response.changed() {
            st.open = true;
            st.highlighted = 0;
        }

        // Untouched text (still the current value) is not a filter.
        let unedited = st.typed == *self.current;
        let matches = if unedited { (0..self.options.len()).collect::<Vec<usize>>() } else { matching_indices(&st.typed, self.options) };
        let mut commit: Option<usize> = None;
        let mut advance_forward = false;
        let mut advance_back = false;
        let mut enter = false;
        if has_focus || focused_before {
            ui.input_mut(|i| {
                if i.consume_key(Modifiers::NONE, Key::ArrowDown) && !matches.is_empty() {
                    st.open = true;
                    st.highlighted = (st.highlighted + 1).min(matches.len() - 1);
                }
                if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                    st.highlighted = st.highlighted.saturating_sub(1);
                }
                if i.consume_key(Modifiers::NONE, Key::Enter) {
                    enter = true;
                    advance_forward = true;
                }
                if i.consume_key(Modifiers::NONE, Key::Tab) {
                    advance_forward = true;
                }
                if i.consume_key(Modifiers::SHIFT, Key::Tab) {
                    advance_back = true;
                }
                if i.consume_key(Modifiers::NONE, Key::Escape) {
                    st.open = false;
                }
            });
        }
        if advance_forward || advance_back {
            // the highlighted popup row wins; otherwise the best match of the text
            if st.open && !matches.is_empty() && st.highlighted < matches.len() {
                commit = Some(matches[st.highlighted]);
            } else {
                commit = best_match_index(&st.typed, self.options);
            }
        }

        // popup list
        // (also on the frame the click on a row steals the focus)
        if st.open && (has_focus || focused_before) && !self.options.is_empty() {
            let popup_w = w.max(widest + 24.0);
            let popup = egui::Area::new(popup_id)
                .order(egui::Order::Foreground)
                .fixed_pos(Pos2::new(rect.min.x, rect.max.y + 1.0))
                .constrain(true);
            let inner = popup.show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgb(0x1a, 0x1f, 0x2a))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(0x44, 0x44, 0x44)))
                    .inner_margin(egui::Margin::same(2))
                    .show(ui, |ui| {
                        ui.set_width(popup_w);
                        let max_h = 22.0 * 10.0;
                        show_scroll(ui, egui::ScrollArea::vertical().max_height(max_h), |ui| {
                            for (row, opt_idx) in matches.iter().enumerate() {
                                let opt = &self.options[*opt_idx];
                                let is_hl = row == st.highlighted;
                                let (r, resp) = ui.allocate_exact_size(Vec2::new(popup_w - 4.0, 22.0), Sense::click());
                                let hovered = resp.hovered();
                                let fill = if is_hl {
                                    Color32::from_rgb(0x1a, 0x8a, 0xe8)
                                } else if hovered {
                                    Color32::from_rgb(0x3a, 0x4a, 0x5e)
                                } else {
                                    Color32::TRANSPARENT
                                };
                                ui.painter().rect_filled(r, CornerRadius::ZERO, fill);
                                if is_hl && st.scroll_to_highlighted {
                                    ui.scroll_to_rect(r, None);
                                }
                                let color = if is_hl || hovered { Color32::WHITE } else { Color32::from_rgb(0xc8, 0xc8, 0xc8) };
                                let f = if is_hl { theme.body_bold() } else { font.clone() };
                                ui.painter().text(Pos2::new(r.min.x + 8.0, r.center().y), egui::Align2::LEFT_CENTER, opt, f, color);
                                if resp.clicked() {
                                    commit = Some(*opt_idx);
                                }
                            }
                            if matches.is_empty() {
                                ui.label(egui::RichText::new("No matches").font(font.clone()).color(theme.secondary));
                            }
                        });
                    });
            });
            st.scroll_to_highlighted = false;
            // click outside closes
            if ui.input(|i| i.pointer.any_click()) {
                let pos = ui.input(|i| i.pointer.interact_pos());
                if let Some(p) = pos {
                    if !inner.response.rect.contains(p) && !rect.contains(p) {
                        st.open = false;
                    }
                }
            }
        }

        // focus lost: commit the best match of whatever was typed
        if focused_before && !has_focus && st.editing && commit.is_none() {
            if let Some(i) = best_match_index(&st.typed, self.options) {
                commit = Some(i);
            } else {
                st.typed = self.current.clone();
            }
            st.editing = false;
            st.open = false;
        }

        if let Some(i) = commit {
            let new_val = self.options[i].clone();
            if new_val != *self.current {
                *self.current = new_val.clone();
                result.changed = true;
            }
            st.typed = new_val;
            if advance_forward || advance_back {
                st.open = false;
                st.editing = false;
            } else {
                st.open = false;
            }
        }
        if !has_focus && !focused_before {
            st.editing = false;
            st.open = false;
        }
        result.enter_pressed = enter;
        result.tab_pressed = advance_forward && !enter;
        result.backtab_pressed = advance_back;
        ui.data_mut(|d| d.insert_temp(state_id, st));
        result
    }
}

// ---------------------------------------------------------------------------
// Amount entry: [−] [value] [+]
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct AmountResponse {
    pub changed: bool,
    pub enter_pressed: bool,
    pub tab_pressed: bool,
    pub backtab_pressed: bool,
    pub has_focus: bool,
}

/// `AmountEntry`. `width` is the Qt `width` argument (`*8` px); `None` = 50 px.
pub struct AmountEntry<'a> {
    theme: &'a Theme,
    id: Id,
    value: &'a mut String,
    min: Option<i64>,
    max: Option<i64>,
    width: Option<i64>,
    enabled: bool,
    request_focus: bool,
}

impl<'a> AmountEntry<'a> {
    pub fn new(theme: &'a Theme, id: Id, value: &'a mut String) -> Self {
        AmountEntry { theme, id, value, min: None, max: None, width: None, enabled: true, request_focus: false }
    }

    pub fn min(mut self, m: Option<i64>) -> Self {
        self.min = m;
        self
    }

    pub fn max(mut self, m: Option<i64>) -> Self {
        self.max = m;
        self
    }

    pub fn width(mut self, w: Option<i64>) -> Self {
        self.width = w;
        self
    }

    pub fn enabled(mut self, e: bool) -> Self {
        self.enabled = e;
        self
    }

    pub fn request_focus(mut self, v: bool) -> Self {
        self.request_focus = v;
        self
    }

    pub fn show(self, ui: &mut Ui) -> AmountResponse {
        let theme = self.theme;
        let mut r = AmountResponse::default();
        let parsed: Option<i64> = self.value.trim().parse::<i64>().ok();
        let (down_ok, up_ok) = match parsed {
            Some(v) => (
                self.min.map(|m| v > m).unwrap_or(true),
                self.max.map(|m| v < m).unwrap_or(true),
            ),
            None => (true, true),
        };
        let entry_w = self.width.map(|w| (w * 8) as f32).unwrap_or(50.0);
        let edit_id = self.id.with("amount_edit");
        ui.spacing_mut().item_spacing.x = 1.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 1.0;
            if amount_button(ui, theme, "−", self.enabled && down_ok).clicked() {
                match parsed {
                    Some(v) => {
                        let mut nv = v - 1;
                        if let Some(m) = self.min {
                            if nv < m {
                                nv = m;
                            }
                        }
                        *self.value = nv.to_string();
                        r.changed = true;
                    }
                    None => {
                        if let Some(m) = self.min {
                            *self.value = m.to_string();
                            r.changed = true;
                        }
                    }
                }
            }
            if self.request_focus {
                ui.memory_mut(|m| m.request_focus(edit_id));
                let mut tes = TextEdit::load_state(ui.ctx(), edit_id).unwrap_or_default();
                let range = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(self.value.chars().count()));
                tes.cursor.set_char_range(Some(range));
                TextEdit::store_state(ui.ctx(), edit_id, tes);
            }
            let er = Entry::new(theme, self.value).id(edit_id).width(entry_w).enabled(self.enabled).centered().show(ui);
            if let Some(resp) = &er.response {
                if resp.gained_focus() {
                    let mut tes = TextEdit::load_state(ui.ctx(), edit_id).unwrap_or_default();
                    let n = self.value.chars().count();
                    let range = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n));
                    tes.cursor.set_char_range(Some(range));
                    TextEdit::store_state(ui.ctx(), edit_id, tes);
                }
            }
            r.changed |= er.changed;
            r.enter_pressed = er.enter_pressed;
            r.tab_pressed = er.tab_pressed;
            r.backtab_pressed = er.backtab_pressed;
            r.has_focus = er.has_focus;
            if amount_button(ui, theme, "+", self.enabled && up_ok).clicked() {
                match parsed {
                    Some(v) => {
                        let mut nv = v + 1;
                        if let Some(m) = self.max {
                            if nv > m {
                                nv = m;
                            }
                        }
                        *self.value = nv.to_string();
                        r.changed = true;
                    }
                    None => {
                        if let Some(m) = self.min {
                            *self.value = m.to_string();
                            r.changed = true;
                        }
                    }
                }
            }
        });
        r
    }
}

// ---------------------------------------------------------------------------
// Check boxes
// ---------------------------------------------------------------------------

/// Draw a 16 px check indicator in the style of the theme's box/check images.
fn paint_check_indicator(ui: &Ui, rect: Rect, theme: &Theme, checked: bool, hovered: bool, enabled: bool) {
    let painter = ui.painter();
    let border = if !enabled {
        theme.subtle_border
    } else if hovered {
        theme.accent
    } else {
        theme.border_focus
    };
    let fill = if checked {
        if !enabled {
            theme.disabled_text
        } else if hovered {
            theme.accent_hover
        } else {
            theme.accent
        }
    } else {
        theme.bg_input
    };
    painter.rect(rect, CornerRadius::same(2), fill, Stroke::new(1.0_f32, border), egui::StrokeKind::Inside);
    if checked {
        let c = rect.center();
        let pts = vec![
            Pos2::new(c.x - 4.0, c.y),
            Pos2::new(c.x - 1.5, c.y + 3.0),
            Pos2::new(c.x + 4.5, c.y - 3.5),
        ];
        painter.add(egui::Shape::line(pts, Stroke::new(2.0_f32, Color32::WHITE)));
    }
}

/// `QCheckBox` (indicator only, or with a text label on the right).
pub fn checkbox(ui: &mut Ui, theme: &Theme, checked: &mut bool, text: &str, enabled: bool) -> Response {
    let font = theme.body();
    let galley = if text.is_empty() {
        None
    } else {
        Some(ui.fonts_mut(|f| f.layout_no_wrap(text.to_string(), font, Color32::WHITE)))
    };
    let text_w = galley.as_ref().map(|g| g.size().x + 4.0).unwrap_or(0.0);
    let desired = Vec2::new(16.0 + text_w, 20.0);
    let (rect, mut response) = ui.allocate_exact_size(desired, if enabled { Sense::click() } else { Sense::hover() });
    if ui.is_rect_visible(rect) {
        let box_rect = Rect::from_center_size(Pos2::new(rect.min.x + 8.0, rect.center().y), Vec2::splat(16.0));
        paint_check_indicator(ui, box_rect, theme, *checked, enabled && response.hovered(), enabled);
        if let Some(g) = galley {
            ui.painter().galley(Pos2::new(rect.min.x + 20.0, rect.center().y - g.size().y / 2.0), g, if enabled { theme.text } else { theme.disabled_text });
        }
    }
    if enabled && response.clicked() {
        *checked = !*checked;
        response.mark_changed();
    }
    response
}

/// `CheckboxLabel(text, flip)`: label + box (box first unless `flip`); the
/// label is clickable too. Returns true when toggled.
pub fn checkbox_label(ui: &mut Ui, theme: &Theme, checked: &mut bool, text: &str, flip: bool, enabled: bool) -> bool {
    let mut toggled = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if flip {
            let r = ui.add(egui::Label::new(egui::RichText::new(text).font(theme.body()).color(theme.text)).sense(Sense::click()));
            if enabled && r.clicked() {
                *checked = !*checked;
                toggled = true;
            }
            if checkbox(ui, theme, checked, "", enabled).changed() {
                toggled = true;
            }
        } else {
            if checkbox(ui, theme, checked, "", enabled).changed() {
                toggled = true;
            }
            let r = ui.add(egui::Label::new(egui::RichText::new(text).font(theme.body()).color(theme.text)).sense(Sense::click()));
            if enabled && r.clicked() {
                *checked = !*checked;
                toggled = true;
            }
        }
    });
    toggled
}

// ---------------------------------------------------------------------------
// Disclosure triangle
// ---------------------------------------------------------------------------

/// `DisclosureTriangle`: a solid triangle pointing down (expanded) or right.
pub fn disclosure_triangle(ui: &mut Ui, expanded: bool, size: f32, color: Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if ui.is_rect_visible(rect) {
        let margin = size * 0.15;
        let tri = size - 2.0 * margin;
        let o = rect.min;
        let pts = if expanded {
            vec![
                Pos2::new(o.x + margin, o.y + margin),
                Pos2::new(o.x + margin + tri, o.y + margin),
                Pos2::new(o.x + margin + tri / 2.0, o.y + margin + tri),
            ]
        } else {
            vec![
                Pos2::new(o.x + margin, o.y + margin),
                Pos2::new(o.x + margin + tri, o.y + margin + tri / 2.0),
                Pos2::new(o.x + margin, o.y + margin + tri),
            ]
        };
        ui.painter().add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
    }
    response
}

// ---------------------------------------------------------------------------
// Group box, sections, separators
// ---------------------------------------------------------------------------

/// `QGroupBox`: 1 px border, radius 3, bold title in the primary colour
/// sitting on the top border.
pub fn group_box<R>(ui: &mut Ui, theme: &Theme, title: &str, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    let font = theme.body_bold();
    let galley = ui.fonts_mut(|f| f.layout_no_wrap(title.to_string(), font, Color32::WHITE));
    let title_h = galley.size().y;
    ui.add_space(title_h / 2.0);
    let frame = egui::Frame::new()
        .stroke(Stroke::new(1.0_f32, theme.border))
        .corner_radius(CornerRadius::same(3))
        .inner_margin(egui::Margin { left: 5, right: 5, top: (title_h / 2.0 + 6.0) as i8, bottom: 5 });
    let inner = frame.show(ui, |ui| add_contents(ui));
    let rect = inner.response.rect;
    // title label with the background colour behind it (breaks the border line)
    let title_pos = Pos2::new(rect.min.x + 8.0, rect.min.y - title_h / 2.0);
    let title_rect = Rect::from_min_size(title_pos, Vec2::new(galley.size().x + 8.0, title_h));
    ui.painter().rect_filled(title_rect, CornerRadius::ZERO, theme.bg);
    ui.painter().galley(Pos2::new(title_pos.x + 4.0, title_pos.y), galley, theme.primary);
    inner.inner
}

/// `RoundedSection`-style frame: solid fill, rounded corners, inner margin.
pub fn rounded_section<R>(ui: &mut Ui, fill: Color32, radius: u8, margin: egui::Margin, add_contents: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    egui::Frame::new().fill(fill).corner_radius(CornerRadius::same(radius)).inner_margin(margin).show(ui, add_contents)
}

/// `QFrame.HLine`
pub fn hline(ui: &mut Ui, theme: &Theme) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::ZERO, theme.border);
}

pub fn vline(ui: &mut Ui, theme: &Theme, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, height), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::ZERO, theme.border);
}

// ---------------------------------------------------------------------------
// Tab bar
// ---------------------------------------------------------------------------

/// Height of the pane tab strip drawn by [`tab_bar`].
pub const TAB_STRIP_H: f32 = 40.0;

/// The pane tab strip: a 40 px row spanning the available width with
/// text-only tabs (12 px horizontal padding; the active one is bold, strong
/// and sits on a 2 px accent underline flush with the strip's bottom edge), a
/// pane-divider hairline along the bottom, and `trailing` drawn right-aligned
/// inside the row. Returns true when the current tab changed.
pub fn tab_bar(ui: &mut Ui, theme: &Theme, tabs: &[&str], current: &mut usize, trailing: impl FnOnce(&mut Ui)) -> bool {
    let mut changed = false;
    let (strip, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), TAB_STRIP_H), Sense::hover());
    let pad = 16.0;
    let mut x = strip.min.x + pad;
    let strong = theme.text_strong();
    for (i, t) in tabs.iter().enumerate() {
        let selected = i == *current;
        let font = if selected { theme.body_bold() } else { theme.body() };
        let galley = ui.fonts_mut(|f| f.layout_no_wrap(t.to_string(), font, Color32::WHITE));
        let w = galley.size().x + 24.0;
        let rect = Rect::from_min_size(Pos2::new(x, strip.min.y), Vec2::new(w, strip.height()));
        let resp = ui.interact(rect, ui.id().with(("tab", i)), Sense::click());
        if ui.is_rect_visible(rect) {
            let color = if selected {
                strong
            } else if resp.hovered() {
                theme.text
            } else {
                theme.secondary
            };
            let painter = ui.painter();
            painter.galley(Pos2::new(rect.center().x - galley.size().x / 2.0, rect.center().y - galley.size().y / 2.0), galley, color);
            if selected {
                let bar = Rect::from_min_max(Pos2::new(rect.min.x, strip.max.y - 2.0), Pos2::new(rect.max.x, strip.max.y));
                painter.rect_filled(bar, CornerRadius::ZERO, theme.accent);
            }
        }
        if resp.clicked() && !selected {
            *current = i;
            changed = true;
        }
        if !selected {
            resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
        x += w + 4.0;
    }
    ui.painter().hline(strip.x_range(), strip.max.y - 0.5, Stroke::new(1.0_f32, theme.pane_divider()));
    let trailing_rect = Rect::from_min_max(Pos2::new(x, strip.min.y), Pos2::new(strip.max.x - pad, strip.max.y - 1.0));
    if trailing_rect.width() > 0.0 {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(trailing_rect).layout(Layout::right_to_left(Align::Center)));
        trailing(&mut child);
    }
    changed
}

// ---------------------------------------------------------------------------
// Menus
// ---------------------------------------------------------------------------

/// A menu row: label on the left, the shortcut text right-aligned (Qt
/// `QMenu::item` look). Returns true when clicked (the menu closes).
pub fn menu_item(ui: &mut Ui, theme: &Theme, text: &str, shortcut: &str, enabled: bool) -> bool {
    menu_row(ui, theme, text, shortcut, None, enabled)
}

/// A checkable menu row.
pub fn menu_check_item(ui: &mut Ui, theme: &Theme, text: &str, shortcut: &str, checked: bool, enabled: bool) -> bool {
    menu_row(ui, theme, text, shortcut, Some(checked), enabled)
}

fn menu_row(ui: &mut Ui, theme: &Theme, text: &str, shortcut: &str, checked: Option<bool>, enabled: bool) -> bool {
    let font = theme.body();
    let galley = ui.fonts_mut(|f| f.layout_no_wrap(text.to_string(), font.clone(), Color32::WHITE));
    let sc_galley = if shortcut.is_empty() {
        None
    } else {
        Some(ui.fonts_mut(|f| f.layout_no_wrap(shortcut.to_string(), font.clone(), Color32::WHITE)))
    };
    let indicator_w = if checked.is_some() { 20.0 } else { 0.0 };
    let min_w = ui.available_width().max(galley.size().x + sc_galley.as_ref().map(|g| g.size().x + 32.0).unwrap_or(0.0) + 40.0 + indicator_w);
    let desired = Vec2::new(min_w, galley.size().y + 6.0);
    let (rect, resp) = ui.allocate_exact_size(desired, if enabled { Sense::click() } else { Sense::hover() });
    if ui.is_rect_visible(rect) {
        let hovered = enabled && resp.hovered();
        if hovered {
            ui.painter().rect_filled(rect, CornerRadius::ZERO, theme.accent);
        }
        let color = if !enabled {
            theme.disabled_text
        } else if hovered {
            Color32::WHITE
        } else {
            theme.text
        };
        let mut x = rect.min.x + 8.0;
        if let Some(c) = checked {
            let box_rect = Rect::from_center_size(Pos2::new(x + 8.0, rect.center().y), Vec2::splat(14.0));
            paint_check_indicator(ui, box_rect, theme, c, false, enabled);
            x += indicator_w + 4.0;
        }
        ui.painter().galley(Pos2::new(x, rect.center().y - galley.size().y / 2.0), galley, color);
        if let Some(g) = sc_galley {
            ui.painter().galley(Pos2::new(rect.max.x - 12.0 - g.size().x, rect.center().y - g.size().y / 2.0), g, color);
        }
    }
    if enabled && resp.clicked() {
        ui.close();
        return true;
    }
    false
}

pub fn menu_separator(ui: &mut Ui, theme: &Theme) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 5.0), Sense::hover());
    let y = rect.center().y;
    ui.painter().line_segment([Pos2::new(rect.min.x + 6.0, y), Pos2::new(rect.max.x - 6.0, y)], Stroke::new(1.0_f32, theme.border));
}

// ---------------------------------------------------------------------------
// Stepper group: [−] [content] [+] as a single rounded control
// ---------------------------------------------------------------------------

/// The battle summary's stepper (`stepper-group` / `stepper-btn` rules).
/// Returns -1/0/+1 for which button was clicked.
/// Height of a stepper group (buttons and content alike).
pub const STEPPER_H: f32 = 26.0;

pub fn stepper_group(ui: &mut Ui, theme: &Theme, id: Id, button_w: f32, minus_enabled: bool, plus_enabled: bool, tooltips: (&str, &str), content: impl FnOnce(&mut Ui)) -> i32 {
    stepper_group_with_height(ui, theme, id, button_w, STEPPER_H, minus_enabled, plus_enabled, tooltips, content)
}

/// `stepper_group` at an explicit row height (the move-header stepper
/// matches the dropdowns beside it rather than the toolbar's `STEPPER_H`).
#[allow(clippy::too_many_arguments)]
pub fn stepper_group_with_height(ui: &mut Ui, theme: &Theme, id: Id, button_w: f32, h: f32, minus_enabled: bool, plus_enabled: bool, tooltips: (&str, &str), content: impl FnOnce(&mut Ui)) -> i32 {
    let mut delta = 0;
    let _ = theme;
    let frame = egui::Frame::new().fill(crate::theme::rgba(255, 255, 255, 0.04)).corner_radius(CornerRadius::same(6)).inner_margin(egui::Margin::ZERO);
    frame.show(ui, |ui| {
        // Fixed-height row: `ui.horizontal` starts at `interact_size.y` and
        // grows as taller children arrive, which shifts already-placed
        // (centered) children and staircases consecutive groups in the bar.
        ui.allocate_ui_with_layout(Vec2::new(0.0, h), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            if stepper_button(ui, id.with("minus"), "−", button_w, h, true, minus_enabled).on_hover_text(tooltips.0).clicked() {
                delta = -1;
            }
            content(ui);
            if stepper_button(ui, id.with("plus"), "+", button_w, h, false, plus_enabled).on_hover_text(tooltips.1).clicked() {
                delta = 1;
            }
        });
    });
    delta
}

/// Height egui gives an `option_menu` combo button: the body font's row
/// height (or the dropdown icon, whichever is taller) plus the theme's
/// vertical button padding. Inline widgets that sit beside a dropdown use it.
pub fn option_menu_height(ui: &Ui, theme: &Theme) -> f32 {
    let row_h = ui.fonts_mut(|f| f.row_height(&theme.body()));
    let sp = ui.spacing();
    row_h.max(sp.icon_width) + 2.0 * sp.button_padding.y
}

fn stepper_button(ui: &mut Ui, id: Id, text: &str, w: f32, h: f32, left: bool, enabled: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), if enabled { Sense::click() } else { Sense::hover() });
    let _ = id;
    if ui.is_rect_visible(rect) {
        let hovered = enabled && resp.hovered();
        let pressed = enabled && resp.is_pointer_button_down_on();
        let fill = if pressed {
            crate::theme::rgba(80, 130, 220, 0.60)
        } else if hovered {
            crate::theme::rgba(120, 170, 255, 0.45)
        } else {
            crate::theme::rgba(255, 255, 255, 0.11)
        };
        let radius = if left { CornerRadius { nw: 6, sw: 6, ne: 0, se: 0 } } else { CornerRadius { ne: 6, se: 6, nw: 0, sw: 0 } };
        ui.painter().rect_filled(rect, radius, fill);
        let color = if !enabled {
            Color32::from_rgb(0x66, 0x66, 0x66)
        } else if hovered {
            Color32::WHITE
        } else {
            Color32::from_rgb(0xcc, 0xcc, 0xcc)
        };
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, text, FontId::new(crate::theme::pt(13.0), egui::FontFamily::Name(std::sync::Arc::from(crate::theme::FAMILY_BOLD))), color);
    }
    resp
}

// ---------------------------------------------------------------------------
// Option stepper: flat inline `− value +`, stepping through an option list
// by index (never raw ±1 arithmetic — some option lists are non-contiguous,
// e.g. Belly Drum's ["0","2","6"]). No frame, no boxed chrome — it reads as
// part of the header row. Clicking the value switches it to a click-to-edit
// `Entry` for the duration of the edit.
// ---------------------------------------------------------------------------

/// Width of the stat-stage stepper in the battle summary move header:
/// 14 px minus button + 30 px value + 14 px plus button.
pub const OPTION_STEPPER_W: f32 = 58.0;
const OPTION_STEPPER_BUTTON_W: f32 = 14.0;
const OPTION_STEPPER_VALUE_W: f32 = 30.0;
/// The click-to-edit `Entry` fills the value cell exactly, so it never
/// overlaps the minus/plus buttons on either side.
const OPTION_STEPPER_EDIT_W: f32 = OPTION_STEPPER_VALUE_W;

/// Map free-typed text onto the closest entry of an ascending numeric option
/// list. `None` when the text is not an integer or `options` is empty.
fn snap_to_option(typed: &str, options: &[String]) -> Option<String> {
    let trimmed = typed.trim();
    if options.iter().any(|o| o == trimmed) {
        return Some(trimmed.to_string());
    }
    let value: i64 = trimmed.parse().ok()?;
    let mut best: Option<(i64, &String)> = None;
    for opt in options {
        let Ok(n) = opt.parse::<i64>() else { continue };
        let dist = (n - value).abs();
        best = match best {
            None => Some((dist, opt)),
            Some((bd, bopt)) => {
                let bn: i64 = bopt.parse().unwrap_or(0);
                if dist < bd || (dist == bd && n < bn) {
                    Some((dist, opt))
                } else {
                    Some((bd, bopt))
                }
            }
        };
    }
    best.map(|(_, opt)| opt.clone())
}

/// `− value +` stepper through an ordered `options` list, stepping by index
/// so the call signature mirrors `option_menu`'s. Flat, inline layout —
/// clicking the value switches it to a click-to-edit `Entry`; Enter commits
/// via `snap_to_option`, Escape/focus-loss reverts. Returns true iff
/// `current` changed this frame.
pub fn option_stepper(ui: &mut Ui, theme: &Theme, id: Id, current: &mut String, options: &[String], enabled: bool) -> bool {
    let idx = options.iter().position(|o| o == current);
    let minus_enabled = enabled && idx.map_or(!options.is_empty(), |i| i > 0);
    let plus_enabled = enabled && idx.map_or(!options.is_empty(), |i| i + 1 < options.len());
    let edit_id = id.with("edit");
    let editing_key = id.with("editing");
    let typed_key = id.with("typed");

    let mut changed = false;
    let mut editing: bool = ui.data(|d| d.get_temp::<bool>(editing_key)).unwrap_or(false);

    // Same `[−] value [+]` control as the pre-fight candy / vitamin steppers
    // at the top of the summary, just with narrower buttons.
    let mut delta = 0i32;
    let mut cancel_edit = false;
    // Same height as the `option_menu` dropdowns it sits beside in the header.
    let h = option_menu_height(ui, theme);
    let group_delta = stepper_group_with_height(ui, theme, id, OPTION_STEPPER_BUTTON_W, h, minus_enabled, plus_enabled, ("Remove one use of this move", "Add one use of this move"), |ui| {
        let (value_rect, _) = ui.allocate_exact_size(Vec2::new(OPTION_STEPPER_VALUE_W, h), Sense::hover());
        if editing {
            let edit_rect = Rect::from_center_size(value_rect.center(), Vec2::new(OPTION_STEPPER_EDIT_W, h));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(edit_rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
            let mut typed: String = ui.data_mut(|d| d.get_temp_mut_or_insert_with(typed_key, || current.clone()).clone());
            if !ui.memory(|m| m.has_focus(edit_id)) {
                ui.memory_mut(|m| m.request_focus(edit_id));
                let mut tes = TextEdit::load_state(ui.ctx(), edit_id).unwrap_or_default();
                let n = typed.chars().count();
                let range = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n));
                tes.cursor.set_char_range(Some(range));
                TextEdit::store_state(ui.ctx(), edit_id, tes);
            }
            let er = Entry::new(theme, &mut typed).width(OPTION_STEPPER_EDIT_W).centered().enabled(true).id(edit_id).show(&mut child);
            if let Some(resp) = &er.response {
                if resp.gained_focus() {
                    let mut tes = TextEdit::load_state(ui.ctx(), edit_id).unwrap_or_default();
                    let n = typed.chars().count();
                    let range = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n));
                    tes.cursor.set_char_range(Some(range));
                    TextEdit::store_state(ui.ctx(), edit_id, tes);
                }
            }
            if er.enter_pressed {
                if let Some(v) = snap_to_option(&typed, options) {
                    if v != *current {
                        *current = v;
                        changed = true;
                    }
                }
                cancel_edit = true;
            } else if er.escape_pressed || (er.lost_focus && !er.enter_pressed) {
                cancel_edit = true;
            }
            ui.data_mut(|d| d.insert_temp(typed_key, typed));
        } else {
            let value_resp = ui.interact(value_rect, id.with("value"), if enabled { Sense::click() } else { Sense::hover() });
            if ui.is_rect_visible(value_rect) {
                let hovered = enabled && value_resp.hovered();
                if hovered {
                    ui.painter().rect_stroke(value_rect.shrink(1.0), CornerRadius::same(3), Stroke::new(1.0_f32, crate::theme::rgba(255, 255, 255, 0.18)), egui::StrokeKind::Inside);
                }
                let color = if !enabled { Color32::from_rgb(0x77, 0x77, 0x77) } else { Color32::WHITE };
                ui.painter().text(value_rect.center(), egui::Align2::CENTER_CENTER, current.as_str(), theme.body_bold(), color);
            }
            let value_resp = if enabled { value_resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { value_resp };
            if enabled && value_resp.clicked() {
                editing = true;
                ui.data_mut(|d| d.insert_temp(typed_key, current.clone()));
            }
        }
    });
    if cancel_edit {
        editing = false;
        ui.memory_mut(|m| m.surrender_focus(edit_id));
    }
    if group_delta < 0 && minus_enabled {
        delta = -1;
    } else if group_delta > 0 && plus_enabled {
        delta = 1;
    }
    if delta != 0 && editing {
        // Clicking a button while editing cancels the edit, then steps.
        editing = false;
        ui.memory_mut(|m| m.surrender_focus(edit_id));
    }

    ui.data_mut(|d| d.insert_temp(editing_key, editing));

    if delta != 0 {
        if let Some(i) = idx {
            let ni = if delta < 0 { i.saturating_sub(1) } else { (i + 1).min(options.len().saturating_sub(1)) };
            if let Some(v) = options.get(ni) {
                if v != current {
                    *current = v.clone();
                    changed = true;
                }
            }
        } else if !options.is_empty() {
            let parsed: Option<i64> = current.trim().parse().ok();
            let snapped = match parsed {
                Some(cv) if delta < 0 => options
                    .iter()
                    .filter_map(|o| o.parse::<i64>().ok().map(|n| (n, o)))
                    .filter(|(n, _)| *n < cv)
                    .max_by_key(|(n, _)| *n)
                    .map(|(_, o)| o.clone())
                    .unwrap_or_else(|| options.first().cloned().unwrap()),
                Some(cv) if delta > 0 => options
                    .iter()
                    .filter_map(|o| o.parse::<i64>().ok().map(|n| (n, o)))
                    .filter(|(n, _)| *n > cv)
                    .min_by_key(|(n, _)| *n)
                    .map(|(_, o)| o.clone())
                    .unwrap_or_else(|| options.last().cloned().unwrap()),
                _ => options[0].clone(),
            };
            if snapped != *current {
                *current = snapped.clone();
                changed = true;
            }
        }
        // Stepping discards any in-progress typed text and shows the new value.
        ui.data_mut(|d| d.insert_temp(typed_key, current.clone()));
    }

    changed
}

#[cfg(test)]
mod tests {
    use super::snap_to_option;

    fn opts(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn exact_match() {
        assert_eq!(snap_to_option("2", &opts(&["0", "2", "6"])), Some("2".to_string()));
    }

    #[test]
    fn clamp_below() {
        assert_eq!(snap_to_option("-5", &opts(&["0", "1", "2", "3"])), Some("0".to_string()));
    }

    #[test]
    fn clamp_above() {
        assert_eq!(snap_to_option("9", &opts(&["0", "1", "2", "3"])), Some("3".to_string()));
    }

    #[test]
    fn nearest() {
        assert_eq!(snap_to_option("5", &opts(&["0", "2", "6"])), Some("6".to_string()));
    }

    #[test]
    fn tie_goes_to_lower() {
        assert_eq!(snap_to_option("4", &opts(&["0", "2", "6"])), Some("2".to_string()));
        assert_eq!(snap_to_option("1", &opts(&["0", "2", "6"])), Some("0".to_string()));
    }

    #[test]
    fn non_numeric_is_none() {
        assert_eq!(snap_to_option("abc", &opts(&["0", "1", "2"])), None);
    }

    #[test]
    fn trims_whitespace() {
        assert_eq!(snap_to_option("  3 ", &opts(&["0", "1", "2", "3"])), Some("3".to_string()));
    }

    #[test]
    fn empty_options_is_none() {
        assert_eq!(snap_to_option("3", &[]), None);
    }
}

// ---------------------------------------------------------------------------
// Misc helpers
// ---------------------------------------------------------------------------

/// Chip-style label (status bar version / run status).
pub fn chip(ui: &mut Ui, theme: &Theme, text: &str, bg: Color32, fg: Color32, padding: Vec2, clickable: bool) -> Response {
    let font = theme.body();
    let galley = ui.fonts_mut(|f| f.layout_no_wrap(text.to_string(), font, Color32::WHITE));
    let desired = galley.size() + 2.0 * padding;
    let (rect, resp) = ui.allocate_exact_size(desired, if clickable { Sense::click() } else { Sense::hover() });
    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, CornerRadius::same(3), bg);
        ui.painter().galley(Pos2::new(rect.min.x + padding.x, rect.center().y - galley.size().y / 2.0), galley, fg);
    }
    if clickable {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// Use the theme body font for a plain `RichText`.
pub fn rich(theme: &Theme, text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into()).font(theme.body()).color(theme.text)
}

pub fn set_body_style(ui: &mut Ui, theme: &Theme) {
    ui.style_mut().override_font_id = Some(theme.body());
    ui.style_mut().text_styles.insert(TextStyle::Body, theme.body());
}

// ---------------------------------------------------------------------------
// Pre-event state pane helpers (docs/rust_port/design/pre_event_state/SPEC.md)
// ---------------------------------------------------------------------------

/// `1234567` -> `1,234,567`.
pub fn fmt_thousands(v: i64) -> String {
    let digits = v.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if v < 0 {
        out.insert(0, '-');
    }
    out
}

/// Letter spacing of the uppercase captions, in px.
pub const CAPTION_TRACKING: f32 = 0.9;

/// Lay out `text` upper-cased with the caption tracking.
pub fn caption_galley(ui: &Ui, text: &str, font: FontId, color: Color32) -> std::sync::Arc<egui::Galley> {
    let fmt = egui::TextFormat { font_id: font, extra_letter_spacing: CAPTION_TRACKING, color, ..Default::default() };
    let job = egui::text::LayoutJob::single_section(text.to_uppercase(), fmt);
    ui.fonts_mut(|f| f.layout_job(job))
}

/// Small uppercase caption (11 px, letter-spaced); `color` defaults to muted.
pub fn caption(ui: &mut Ui, theme: &Theme, text: &str, color: Option<Color32>) -> Response {
    let color = color.unwrap_or(theme.secondary);
    let galley = caption_galley(ui, text, theme.caption_font(), color);
    let (rect, resp) = ui.allocate_exact_size(galley.size(), Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().galley(rect.min, galley, color);
    }
    resp
}

/// Default card padding: 12 top / 14 sides / 10 bottom.
pub const CARD_PADDING: egui::Margin = egui::Margin { left: 14, right: 14, top: 12, bottom: 10 };

/// Card frame: card bg, 1 px card border, radius 8, [`CARD_PADDING`] unless overridden.
pub fn card<R>(ui: &mut Ui, theme: &Theme, padding: Option<egui::Margin>, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    egui::Frame::new()
        .fill(theme.card_bg())
        .stroke(Stroke::new(1.0_f32, theme.card_border()))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(padding.unwrap_or(CARD_PADDING))
        .show(ui, add)
}

/// Height of a card title row.
pub const CARD_TITLE_H: f32 = 22.0;

/// 22 px title row: uppercase section title (header colour) left, optional
/// muted caption right. Returns the row rect so callers can place column
/// heads over it.
pub fn card_title(ui: &mut Ui, theme: &Theme, title: &str, right: Option<&str>) -> Rect {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), CARD_TITLE_H), Sense::hover());
    if ui.is_rect_visible(rect) {
        let g = caption_galley(ui, title, theme.caption_font_bold(), theme.header);
        ui.painter().galley(Pos2::new(rect.min.x, rect.center().y - g.size().y / 2.0), g, theme.header);
        if let Some(r) = right {
            let g = ui.fonts_mut(|f| f.layout_no_wrap(r.to_string(), theme.caption_font(), theme.secondary));
            ui.painter().galley(Pos2::new(rect.max.x - g.size().x, rect.center().y - g.size().y / 2.0), g, theme.secondary);
        }
    }
    rect
}

/// Rounded pill ("Lv 5"): 12 px bold text, padding 2 x 8, radius 999.
pub fn pill(ui: &mut Ui, theme: &Theme, text: &str, fg: Color32, bg: Color32, border: Color32) -> Response {
    let galley = ui.fonts_mut(|f| f.layout_no_wrap(text.to_string(), theme.body_bold(), fg));
    let (rect, resp) = ui.allocate_exact_size(galley.size() + Vec2::new(16.0, 4.0), Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().rect(rect, CornerRadius::same(255), bg, Stroke::new(1.0_f32, border), egui::StrokeKind::Inside);
        ui.painter().galley(Pos2::new(rect.min.x + 8.0, rect.center().y - galley.size().y / 2.0), galley, fg);
    }
    resp
}

/// Outlined chip with uppercase caption text (the event-type chip): 11 px
/// bold, padding 2 x 8, radius 4.
pub fn chip_outlined(ui: &mut Ui, theme: &Theme, text: &str, fg: Color32, bg: Color32, border: Color32) -> Response {
    let galley = caption_galley(ui, text, theme.caption_font_bold(), fg);
    let (rect, resp) = ui.allocate_exact_size(galley.size() + Vec2::new(16.0, 4.0), Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().rect(rect, CornerRadius::same(4), bg, Stroke::new(1.0_f32, border), egui::StrokeKind::Inside);
        ui.painter().galley(Pos2::new(rect.min.x + 8.0, rect.center().y - galley.size().y / 2.0), galley, fg);
    }
    resp
}

/// Paint a progress bar into `rect` (fraction 0..1).
pub fn paint_progress_bar(ui: &Ui, rect: Rect, fraction: f32, fill: Color32, track: Color32) {
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(3), track);
    let w = rect.width() * fraction.clamp(0.0, 1.0);
    if w > 0.0 {
        painter.rect_filled(Rect::from_min_size(rect.min, Vec2::new(w, rect.height())), CornerRadius::same(3), fill);
    }
}

/// 6 px progress bar of the given width; fraction 0..1.
pub fn progress_bar(ui: &mut Ui, width: f32, fraction: f32, fill: Color32, track: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 6.0), Sense::hover());
    if ui.is_rect_visible(rect) {
        paint_progress_bar(ui, rect, fraction, fill, track);
    }
    resp
}

/// Stroked warning triangle (the banner icon) fitted to `rect`.
pub fn paint_warning_triangle(ui: &Ui, rect: Rect, color: Color32) {
    let painter = ui.painter();
    let s = rect.width();
    let o = rect.min;
    let stroke = Stroke::new(1.6_f32, color);
    let pts = vec![
        Pos2::new(o.x + s * 0.5, o.y + s * 0.12),
        Pos2::new(o.x + s * 0.94, o.y + s * 0.86),
        Pos2::new(o.x + s * 0.06, o.y + s * 0.86),
        Pos2::new(o.x + s * 0.5, o.y + s * 0.12),
    ];
    painter.add(egui::Shape::line(pts, stroke));
    painter.line_segment([Pos2::new(o.x + s * 0.5, o.y + s * 0.4), Pos2::new(o.x + s * 0.5, o.y + s * 0.6)], stroke);
    painter.circle_filled(Pos2::new(o.x + s * 0.5, o.y + s * 0.73), 1.0, color);
}

/// Warning banner: padding 8 x 12, radius 6, warning bg + 1 px warning
/// border, a 16 px stroked triangle, then **Warning:** + message in the
/// warning colour.
pub fn warning_banner(ui: &mut Ui, theme: &Theme, message: &str) -> Response {
    let bold = ui.fonts_mut(|f| f.layout_no_wrap("Warning:".to_string(), theme.body_bold(), theme.warning));
    let text_w = (ui.available_width() - 24.0 - 16.0 - 10.0 - bold.size().x - 4.0).max(40.0);
    let msg = ui.fonts_mut(|f| f.layout(message.to_string(), theme.body(), theme.warning, text_w));
    let h = (msg.size().y.max(bold.size().y).max(16.0) + 16.0).max(34.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().rect(rect, CornerRadius::same(6), theme.warning_bg(), Stroke::new(1.0_f32, theme.warning_border()), egui::StrokeKind::Inside);
        let icon = Rect::from_center_size(Pos2::new(rect.min.x + 12.0 + 8.0, rect.center().y), Vec2::splat(16.0));
        paint_warning_triangle(ui, icon, theme.warning);
        let x = icon.max.x + 10.0;
        let y = rect.center().y - bold.size().y / 2.0;
        let bw = bold.size().x;
        ui.painter().galley(Pos2::new(x, y), bold, theme.warning);
        ui.painter().galley(Pos2::new(x + bw + 4.0, rect.center().y - msg.size().y / 2.0), msg, theme.warning);
    }
    resp
}

/// Paint `text` inside `cell` (vertically centred) with the given alignment.
pub fn col_text(ui: &Ui, cell: Rect, text: &str, font: FontId, color: Color32, align: Align) {
    let galley = ui.fonts_mut(|f| f.layout_no_wrap(text.to_string(), font, color));
    let x = match align {
        Align::Min => cell.min.x,
        Align::Center => cell.center().x - galley.size().x / 2.0,
        Align::Max => cell.max.x - galley.size().x,
    };
    ui.painter().galley(Pos2::new(x, cell.center().y - galley.size().y / 2.0), galley, color);
}

/// Allocate a full-width row of `height`, drawing a 1 px hairline of
/// `divider` along its top edge when given.
pub fn table_row(ui: &mut Ui, height: f32, divider: Option<Color32>) -> Rect {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    if let Some(c) = divider {
        if ui.is_rect_visible(rect) {
            ui.painter().hline(rect.x_range(), rect.min.y + 0.5, Stroke::new(1.0_f32, c));
        }
    }
    rect
}

/// A 1 px full-width hairline in `color`.
pub fn hairline(ui: &mut Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::ZERO, color);
}

/// Stroked chevron (`up`: pointing up, else down) centred in `rect`.
pub fn paint_chevron(ui: &Ui, rect: Rect, up: bool, color: Color32) {
    let c = rect.center();
    let s = rect.width() / 2.0;
    let dy = if up { s * 0.5 } else { -s * 0.5 };
    let pts = vec![Pos2::new(c.x - s, c.y + dy), Pos2::new(c.x, c.y - dy), Pos2::new(c.x + s, c.y + dy)];
    ui.painter().add(egui::Shape::line(pts, Stroke::new(2.0_f32, color)));
}

/// Stroked chevron pointing right (collapsed) or down (expanded).
pub fn paint_disclosure_chevron(ui: &Ui, rect: Rect, expanded: bool, color: Color32) {
    let c = rect.center();
    let s = rect.width() / 2.0;
    let pts = if expanded {
        vec![Pos2::new(c.x - s, c.y - s * 0.5), Pos2::new(c.x, c.y + s * 0.5), Pos2::new(c.x + s, c.y - s * 0.5)]
    } else {
        vec![Pos2::new(c.x - s * 0.5, c.y - s), Pos2::new(c.x + s * 0.5, c.y), Pos2::new(c.x - s * 0.5, c.y + s)]
    };
    ui.painter().add(egui::Shape::line(pts, Stroke::new(2.0_f32, color)));
}

/// 26 x 22 outlined arrow button with a 12 px chevron; disabled buttons use
/// the disabled colour and do not react to hover.
pub fn chevron_button(ui: &mut Ui, theme: &Theme, up: bool, enabled: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(26.0, 22.0), if enabled { Sense::click() } else { Sense::hover() });
    if ui.is_rect_visible(rect) {
        let hovered = enabled && resp.hovered();
        let (border, fg) = if !enabled {
            (theme.subtle_border, theme.disabled_text)
        } else if hovered {
            (theme.accent, theme.text)
        } else {
            (theme.border, theme.text)
        };
        let fill = if hovered { theme.hover_bg } else { theme.card_bg() };
        ui.painter().rect(rect, CornerRadius::same(4), fill, Stroke::new(1.0_f32, border), egui::StrokeKind::Inside);
        paint_chevron(ui, Rect::from_center_size(rect.center(), Vec2::splat(9.0)), up, fg);
    }
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// Three horizontal lines (the drag handle) fitted to `rect`.
pub fn paint_drag_handle(ui: &Ui, rect: Rect, color: Color32) {
    let painter = ui.painter();
    let stroke = Stroke::new(1.6_f32, color);
    let x0 = rect.min.x + rect.width() * 0.15;
    let x1 = rect.max.x - rect.width() * 0.15;
    for f in [0.3, 0.5, 0.7] {
        let y = rect.min.y + rect.height() * f;
        painter.line_segment([Pos2::new(x0, y), Pos2::new(x1, y)], stroke);
    }
}

/// Outline bag glyph (the empty-bag placeholder) fitted to `rect`.
pub fn paint_bag_glyph(ui: &Ui, rect: Rect, color: Color32) {
    let painter = ui.painter();
    let stroke = Stroke::new(1.6_f32, color);
    let s = rect.width();
    let o = rect.min;
    // body: a slightly flared trapezoid
    let body = vec![
        Pos2::new(o.x + s * 0.25, o.y + s * 0.34),
        Pos2::new(o.x + s * 0.75, o.y + s * 0.34),
        Pos2::new(o.x + s * 0.80, o.y + s * 0.84),
        Pos2::new(o.x + s * 0.20, o.y + s * 0.84),
        Pos2::new(o.x + s * 0.25, o.y + s * 0.34),
    ];
    painter.add(egui::Shape::line(body, stroke));
    // handle: a semicircle on short legs
    let cx = o.x + s * 0.5;
    let r = s * 0.13;
    let top_y = o.y + s * 0.34;
    let mut handle = vec![Pos2::new(cx - r, top_y)];
    for i in 0..=8 {
        let t = std::f32::consts::PI * (1.0 + i as f32 / 8.0);
        handle.push(Pos2::new(cx + r * t.cos(), o.y + s * 0.12 + r + r * t.sin()));
    }
    handle.push(Pos2::new(cx + r, top_y));
    painter.add(egui::Shape::line(handle, stroke));
}

#[cfg(test)]
mod pane_tests {
    use super::fmt_thousands;

    #[test]
    fn thousands() {
        assert_eq!(fmt_thousands(0), "0");
        assert_eq!(fmt_thousands(999), "999");
        assert_eq!(fmt_thousands(1000), "1,000");
        assert_eq!(fmt_thousands(12400), "12,400");
        assert_eq!(fmt_thousands(1234567), "1,234,567");
        assert_eq!(fmt_thousands(-2210), "-2,210");
    }
}
