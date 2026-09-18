//! Port of `gui_qt/theme.py`: the same colour derivation math applied to the
//! configured colours, producing tokens for app-drawn widgets plus an egui
//! `Style`/`Visuals` for everything egui styles itself.

use std::collections::BTreeMap;
use std::sync::Arc;

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, Style, Visuals};
use xpr_core::Config;

/// `_hex_to_rgb`: 6-digit hex (with or without `#`), grey for anything else.
pub fn hex_to_rgb(hex: &str) -> (u8, u8, u8) {
    let h = hex.trim().trim_start_matches('#');
    if h.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&h[0..2], 16),
            u8::from_str_radix(&h[2..4], 16),
            u8::from_str_radix(&h[4..6], 16),
        ) {
            return (r, g, b);
        }
    }
    (128, 128, 128)
}

pub fn parse_hex(hex: &str) -> Color32 {
    let (r, g, b) = hex_to_rgb(hex);
    Color32::from_rgb(r, g, b)
}

/// `_rgb_to_hex` (Python `int()` truncation, clamped like the battle summary helper).
pub fn rgb_to_hex(r: f64, g: f64, b: f64) -> String {
    let clamp = |v: f64| (v.max(0.0).min(255.0)) as i64;
    format!("#{:02x}{:02x}{:02x}", clamp(r), clamp(g), clamp(b))
}

pub fn to_hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

/// `_lighten(hex, amount)`: blend toward white.
pub fn lighten_hex(hex: &str, amount: f64) -> String {
    let (r, g, b) = hex_to_rgb(hex);
    let f = |c: u8| (c as f64 + (255.0 - c as f64) * amount).min(255.0);
    rgb_to_hex(f(r), f(g), f(b))
}

/// `_darken(hex, amount)`: scale toward black.
pub fn darken_hex(hex: &str, amount: f64) -> String {
    let (r, g, b) = hex_to_rgb(hex);
    let f = |c: u8| (c as f64 * (1.0 - amount)).max(0.0);
    rgb_to_hex(f(r), f(g), f(b))
}

/// `_blend_color(color1, color2, alpha)`: colour1 over colour2.
pub fn blend_hex(c1: &str, c2: &str, alpha: f64) -> String {
    let c1 = c1.trim();
    let c2 = c2.trim();
    if c1.is_empty() || !c1.starts_with('#') {
        return if c2.is_empty() { "#000000".into() } else { c2.to_string() };
    }
    if c2.is_empty() || !c2.starts_with('#') {
        return c1.to_string();
    }
    let (r1, g1, b1) = hex_to_rgb(c1);
    let (r2, g2, b2) = hex_to_rgb(c2);
    let f = |a: u8, b: u8| a as f64 * alpha + b as f64 * (1.0 - alpha);
    rgb_to_hex(f(r1, r2), f(g1, g2), f(b1, b2))
}

pub fn lighten(c: Color32, amount: f64) -> Color32 {
    parse_hex(&lighten_hex(&to_hex(c), amount))
}

pub fn darken(c: Color32, amount: f64) -> Color32 {
    parse_hex(&darken_hex(&to_hex(c), amount))
}

pub fn blend(c1: Color32, c2: Color32, alpha: f64) -> Color32 {
    parse_hex(&blend_hex(&to_hex(c1), &to_hex(c2), alpha))
}

/// `tinted_bg_for_style`: mix `fg` into `bg` (`int(b + (f - b) * alpha)`).
pub fn tint(fg: Color32, bg: Color32, alpha: f64) -> Color32 {
    let f = |f: u8, b: u8| ((b as f64 + (f as f64 - b as f64) * alpha) as i64).clamp(0, 255) as u8;
    Color32::from_rgb(f(fg.r(), bg.r()), f(fg.g(), bg.g()), f(fg.b(), bg.b()))
}

pub fn with_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// `rgba(255,255,255,0.11)`-style translucent white/colour.
pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, (a * 255.0).round() as u8)
}

pub const ACCENT: &str = "#0078d4";
pub const ACCENT_HOVER: &str = "#1a8ae8";
pub const ACCENT_PRESSED: &str = "#005fa3";

/// Font family names registered by [`Theme::install_fonts`].
pub const FAMILY_REGULAR: &str = "xpr-regular";
pub const FAMILY_BOLD: &str = "xpr-bold";

/// 9 pt = 12 px at 100 %; egui font sizes are in logical pixels.
pub fn pt(points: f32) -> f32 {
    points * 4.0 / 3.0
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub bg: Color32,
    pub text: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub failure: Color32,
    pub divider: Color32,
    pub header: Color32,
    pub primary: Color32,
    pub secondary: Color32,
    pub contrast: Color32,

    pub bg_lighter: Color32,
    pub bg_darker: Color32,
    pub bg_input: Color32,
    pub border: Color32,
    pub border_focus: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_pressed: Color32,
    pub hover_bg: Color32,
    pub disabled_text: Color32,
    pub subtle_border: Color32,

    pub font_name: String,
    /// True when the configured font (and its bold face) could be loaded.
    pub font_loaded: bool,
}

impl Theme {
    pub fn from_config(cfg: &Config) -> Theme {
        let bg_hex = cfg.get_background_color().to_string();
        Theme {
            bg: parse_hex(&bg_hex),
            text: parse_hex(cfg.get_text_color()),
            success: parse_hex(cfg.get_success_color()),
            warning: parse_hex(cfg.get_warning_color()),
            failure: parse_hex(cfg.get_failure_color()),
            divider: parse_hex(cfg.get_divider_color()),
            header: parse_hex(cfg.get_header_color()),
            primary: parse_hex(cfg.get_primary_color()),
            secondary: parse_hex(cfg.get_secondary_color()),
            contrast: parse_hex(cfg.get_contrast_color()),
            bg_lighter: parse_hex(&lighten_hex(&bg_hex, 0.06)),
            bg_darker: parse_hex(&darken_hex(&bg_hex, 0.10)),
            bg_input: parse_hex(&lighten_hex(&bg_hex, 0.04)),
            border: parse_hex(&lighten_hex(&bg_hex, 0.22)),
            border_focus: parse_hex(&lighten_hex(&bg_hex, 0.35)),
            accent: parse_hex(ACCENT),
            accent_hover: parse_hex(ACCENT_HOVER),
            accent_pressed: parse_hex(ACCENT_PRESSED),
            hover_bg: parse_hex(&lighten_hex(&bg_hex, 0.10)),
            disabled_text: parse_hex(&lighten_hex(&bg_hex, 0.30)),
            subtle_border: parse_hex(&lighten_hex(&bg_hex, 0.12)),
            font_name: cfg.get_custom_font_name(),
            font_loaded: false,
        }
    }

    /// `_color_for_style("Primary")` etc.; the text colour for anything else.
    pub fn style_color(&self, prefix: &str) -> Color32 {
        match prefix {
            "Primary" => self.primary,
            "Secondary" => self.secondary,
            "Header" => self.header,
            "Success" => self.success,
            "Warning" => self.warning,
            "Failure" => self.failure,
            "Contrast" => self.contrast,
            "Divider" => self.divider,
            _ => self.text,
        }
    }

    /// `tinted_bg_for_style(prefix, alpha)`
    pub fn tinted_bg(&self, prefix: &str, alpha: f64) -> Color32 {
        tint(self.style_color(prefix), self.bg, alpha)
    }

    /// The rounded "section" background used by the state viewer & editors.
    pub fn section_bg(&self) -> Color32 {
        lighten(self.bg, 0.06)
    }

    // ---- pre-event state pane tokens (docs/rust_port/design/pre_event_state/SPEC.md §3.1)

    /// Card fill (= `bg_input`).
    pub fn card_bg(&self) -> Color32 {
        lighten(self.bg, 0.04)
    }

    /// 1 px card outline (= `hover_bg`).
    pub fn card_border(&self) -> Color32 {
        lighten(self.bg, 0.10)
    }

    /// Hairlines between rows inside a card.
    pub fn row_divider(&self) -> Color32 {
        lighten(self.bg, 0.085)
    }

    /// Hairlines between pane regions (under the tab strip, above the notes
    /// footer, under an editor title strip).
    pub fn pane_divider(&self) -> Color32 {
        lighten(self.bg, 0.095)
    }

    /// Recessed fill: text inputs, enemy mon cards, drag rows, the icon tile.
    pub fn well_bg(&self) -> Color32 {
        self.bg
    }

    /// Editor card title strip fill.
    pub fn strip_bg(&self) -> Color32 {
        self.section_bg()
    }

    /// Brightest text: names, stat values, money, card titles.
    pub fn text_strong(&self) -> Color32 {
        lighten(self.text, 0.7)
    }

    /// Stroke of outline icons (the empty-bag glyph).
    pub fn icon_stroke(&self) -> Color32 {
        lighten(self.bg, 0.3)
    }

    /// Event-type chip fill / border.
    pub fn chip_bg(&self) -> Color32 {
        self.tinted_bg("Header", 0.14)
    }

    pub fn chip_border(&self) -> Color32 {
        self.tinted_bg("Header", 0.28)
    }

    /// "Lv N" pill fill / border.
    pub fn pill_bg(&self) -> Color32 {
        self.tinted_bg("Primary", 0.12)
    }

    pub fn pill_border(&self) -> Color32 {
        self.tinted_bg("Primary", 0.25)
    }

    /// Warning banner fill / border.
    pub fn warning_bg(&self) -> Color32 {
        self.tinted_bg("Warning", 0.12)
    }

    pub fn warning_border(&self) -> Color32 {
        self.tinted_bg("Warning", 0.32)
    }

    /// 11 px caption face (column heads, card footers, slot numbers).
    pub fn caption_font(&self) -> FontId {
        self.font(8.25)
    }

    pub fn caption_font_bold(&self) -> FontId {
        self.font_bold(8.25)
    }

    pub fn font(&self, points: f32) -> FontId {
        FontId::new(pt(points), FontFamily::Name(Arc::from(FAMILY_REGULAR)))
    }

    pub fn font_bold(&self, points: f32) -> FontId {
        FontId::new(pt(points), FontFamily::Name(Arc::from(FAMILY_BOLD)))
    }

    /// The default 9 pt body font.
    pub fn body(&self) -> FontId {
        self.font(9.0)
    }

    pub fn body_bold(&self) -> FontId {
        self.font_bold(9.0)
    }

    /// Register the configured font (falling back to egui's built-in fonts
    /// when it cannot be found on the system) under [`FAMILY_REGULAR`] and
    /// [`FAMILY_BOLD`], and make the regular face the proportional default.
    pub fn install_fonts(&mut self, ctx: &egui::Context) {
        let mut defs = FontDefinitions::default();
        let (regular, bold) = find_system_font(&self.font_name);
        let mut regular_list: Vec<String> = Vec::new();
        let mut bold_list: Vec<String> = Vec::new();
        self.font_loaded = false;
        if let Some(bytes) = regular {
            defs.font_data.insert(FAMILY_REGULAR.to_string(), Arc::new(FontData::from_owned(bytes)));
            regular_list.push(FAMILY_REGULAR.to_string());
            self.font_loaded = true;
        }
        if let Some(bytes) = bold {
            defs.font_data.insert(FAMILY_BOLD.to_string(), Arc::new(FontData::from_owned(bytes)));
            bold_list.push(FAMILY_BOLD.to_string());
        }
        // Fall back to (and chain for glyph coverage) the built-in fonts.
        let builtin: Vec<String> = defs
            .families
            .get(&FontFamily::Proportional)
            .cloned()
            .unwrap_or_default();
        regular_list.extend(builtin.iter().cloned());
        if bold_list.is_empty() {
            bold_list.extend(regular_list.iter().cloned());
        } else {
            bold_list.extend(builtin.iter().cloned());
        }
        defs.families.insert(FontFamily::Name(Arc::from(FAMILY_REGULAR)), regular_list.clone());
        defs.families.insert(FontFamily::Name(Arc::from(FAMILY_BOLD)), bold_list);
        defs.families.insert(FontFamily::Proportional, regular_list);
        ctx.set_fonts(defs);
    }

    /// Build the egui style: the parts of the stylesheet egui draws itself
    /// (text colour, widget fills, selection, scrollbars, tooltips, menus).
    pub fn apply(&self, ctx: &egui::Context) {
        let mut style: Style = (*ctx.style()).clone();
        let mut v = Visuals::dark();
        v.override_text_color = Some(self.text);
        v.panel_fill = self.bg;
        v.window_fill = self.bg;
        v.window_stroke = Stroke::new(1.0_f32, self.border);
        v.window_corner_radius = CornerRadius::same(3);
        v.menu_corner_radius = CornerRadius::ZERO;
        v.extreme_bg_color = self.bg_input;
        v.text_edit_bg_color = Some(self.bg_input);
        v.faint_bg_color = self.bg_lighter;
        v.code_bg_color = self.bg_input;
        v.hyperlink_color = self.accent;
        v.warn_fg_color = self.warning;
        v.error_fg_color = self.failure;
        v.selection.bg_fill = self.accent;
        v.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
        v.striped = false;
        v.button_frame = true;
        v.collapsing_header_frame = false;
        v.indent_has_left_vline = false;
        v.popup_shadow = egui::epaint::Shadow::NONE;
        v.window_shadow = egui::epaint::Shadow::NONE;
        v.window_highlight_topmost = false;

        // Buttons / frames (QPushButton rules).
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = self.bg;
        w.noninteractive.weak_bg_fill = self.bg;
        w.noninteractive.bg_stroke = Stroke::new(1.0_f32, self.border);
        w.noninteractive.fg_stroke = Stroke::new(1.0_f32, self.text);
        w.noninteractive.corner_radius = CornerRadius::same(3);

        w.inactive.bg_fill = self.bg_lighter;
        w.inactive.weak_bg_fill = self.bg_lighter;
        w.inactive.bg_stroke = Stroke::new(1.0_f32, self.border);
        w.inactive.fg_stroke = Stroke::new(1.0_f32, self.text);
        w.inactive.corner_radius = CornerRadius::same(3);
        w.inactive.expansion = 0.0;

        w.hovered.bg_fill = self.hover_bg;
        w.hovered.weak_bg_fill = self.hover_bg;
        w.hovered.bg_stroke = Stroke::new(1.0_f32, self.accent);
        w.hovered.fg_stroke = Stroke::new(1.0_f32, self.text);
        w.hovered.corner_radius = CornerRadius::same(3);
        w.hovered.expansion = 0.0;

        w.active.bg_fill = self.accent_pressed;
        w.active.weak_bg_fill = self.accent_pressed;
        w.active.bg_stroke = Stroke::new(1.0_f32, self.accent);
        w.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
        w.active.corner_radius = CornerRadius::same(3);
        w.active.expansion = 0.0;

        w.open.bg_fill = self.bg_input;
        w.open.weak_bg_fill = self.bg_input;
        w.open.bg_stroke = Stroke::new(1.0_f32, self.accent);
        w.open.fg_stroke = Stroke::new(1.0_f32, self.text);
        w.open.corner_radius = CornerRadius::same(2);
        w.open.expansion = 0.0;

        style.visuals = v;
        style.spacing.item_spacing = egui::vec2(4.0, 4.0);
        style.spacing.button_padding = egui::vec2(8.0, 2.0);
        style.spacing.interact_size = egui::vec2(24.0, 20.0);
        style.spacing.icon_width = 16.0;
        style.spacing.icon_width_inner = 10.0;
        style.spacing.icon_spacing = 4.0;
        style.spacing.combo_width = 100.0;
        style.spacing.text_edit_width = 200.0;
        style.spacing.tooltip_width = 400.0;
        style.spacing.menu_margin = egui::Margin::symmetric(0, 2);
        style.spacing.window_margin = egui::Margin::same(6);
        style.spacing.scroll.bar_width = 10.0;
        style.spacing.scroll.handle_min_length = 24.0;
        style.spacing.scroll.bar_inner_margin = 1.0;
        style.spacing.scroll.bar_outer_margin = 0.0;
        style.spacing.scroll.floating = false;
        style.spacing.scroll.foreground_color = false;
        style.spacing.indent = 16.0;
        style.interaction.tooltip_delay = 0.1;
        style.interaction.tooltip_grace_time = 0.0;
        style.interaction.show_tooltips_only_when_still = false;
        style.interaction.selectable_labels = false;

        let body = self.body();
        let mut text_styles: BTreeMap<egui::TextStyle, FontId> = BTreeMap::new();
        text_styles.insert(egui::TextStyle::Small, self.font(7.5));
        text_styles.insert(egui::TextStyle::Body, body.clone());
        text_styles.insert(egui::TextStyle::Button, body.clone());
        text_styles.insert(egui::TextStyle::Monospace, FontId::new(pt(9.0), FontFamily::Monospace));
        text_styles.insert(egui::TextStyle::Heading, self.font_bold(12.0));
        style.text_styles = text_styles;
        style.wrap_mode = Some(egui::TextWrapMode::Extend);
        ctx.set_style(style);
    }

    /// Stroke used for the 1 px widget borders.
    pub fn border_stroke(&self) -> Stroke {
        Stroke::new(1.0_f32, self.border)
    }
}

/// Locate the regular and bold faces of a system font by name, e.g.
/// "Segoe UI" -> `segoeui.ttf` / `segoeuib.ttf` on Windows.
fn find_system_font(name: &str) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    let key: String = name.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase();
    if key.is_empty() {
        return (None, None);
    }
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    #[cfg(target_os = "windows")]
    {
        if let Some(w) = std::env::var_os("WINDIR") {
            dirs.push(std::path::PathBuf::from(w).join("Fonts"));
        }
        if let Some(l) = std::env::var_os("LOCALAPPDATA") {
            dirs.push(std::path::PathBuf::from(l).join("Microsoft").join("Windows").join("Fonts"));
        }
    }
    #[cfg(target_os = "macos")]
    {
        dirs.push("/System/Library/Fonts".into());
        dirs.push("/System/Library/Fonts/Supplemental".into());
        dirs.push("/Library/Fonts".into());
        dirs.push(xpr_core::consts::home_dir().join("Library").join("Fonts"));
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        dirs.push("/usr/share/fonts".into());
        dirs.push("/usr/local/share/fonts".into());
        dirs.push(xpr_core::consts::home_dir().join(".fonts"));
    }
    let mut regular: Option<Vec<u8>> = None;
    let mut bold: Option<Vec<u8>> = None;
    let mut candidates: Vec<(String, std::path::PathBuf)> = Vec::new();
    for d in dirs {
        collect_font_files(&d, &mut candidates, 0);
    }
    // Exact stem match first ("segoeui"), then stem + "b"/"bd"/"bold" for the bold face.
    let bold_keys = [format!("{}b", key), format!("{}bd", key), format!("{}bold", key), format!("{}-bold", key)];
    for (stem, path) in &candidates {
        if *stem == key && regular.is_none() {
            regular = std::fs::read(path).ok();
        } else if bold_keys.iter().any(|k| k == stem) && bold.is_none() {
            bold = std::fs::read(path).ok();
        }
    }
    if regular.is_none() {
        // Fall back to a "contains" match on the family name (e.g. "SegoeUI-Regular").
        for (stem, path) in &candidates {
            if stem.starts_with(&key) && (stem.ends_with("regular") || stem == &key) {
                regular = std::fs::read(path).ok();
                break;
            }
        }
    }
    (regular, bold)
}

fn collect_font_files(dir: &std::path::Path, out: &mut Vec<(String, std::path::PathBuf)>, depth: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth < 2 {
                collect_font_files(&path, out, depth + 1);
            }
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).unwrap_or_default();
        if ext != "ttf" && ext != "otf" && ext != "ttc" {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.chars().filter(|c| !c.is_whitespace() && *c != '_').collect::<String>().to_lowercase())
            .unwrap_or_default();
        out.push((stem, path));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derivation_matches_python() {
        // theme.py math on the default background #1e1e1e
        assert_eq!(lighten_hex("#1e1e1e", 0.06), "#2b2b2b");
        assert_eq!(darken_hex("#1e1e1e", 0.10), "#1b1b1b");
        assert_eq!(lighten_hex("#1e1e1e", 0.22), "#4f4f4f");
        assert_eq!(blend_hex("#e0e0e0", "#1e1e1e", 0.10), "#313131");
        assert_eq!(blend_hex("", "#1e1e1e", 0.5), "#1e1e1e");
        assert_eq!(blend_hex("#ffffff", "", 0.5), "#ffffff");
        let t = tint(Color32::from_rgb(0xe8, 0xa8, 0x50), Color32::from_rgb(0x1e, 0x1e, 0x1e), 0.25);
        assert_eq!(to_hex(t), "#50402a");
    }
}
