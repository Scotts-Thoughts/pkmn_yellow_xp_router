//! Small drawing helpers shared by the Pokédex tab's files: outlined
//! sprites, Solodex's uppercase "tracking-widest" headings, compact selects
//! and check boxes, wrapped flow layouts and the Bulbapedia links.

use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Id, Painter, Pos2, Rect, Sense, Stroke,
    TextureHandle, Ui, Vec2,
};
use xpr_ui_kit::theme::Theme;

use crate::images::{DexImages, SpriteSize};
use crate::palette;
use crate::widgets::text_w;

/// Paint a sprite fitted into `rect` with Solodex's `pokemon-icon-stroke`
/// (a black 1 px outline made of four offset black copies).
pub fn paint_outlined(painter: &Painter, tex: &TextureHandle, rect: Rect, tint: Color32) {
    let [w, h] = tex.size();
    let scale = (rect.width() / w as f32).min(rect.height() / h as f32);
    let r = Rect::from_center_size(rect.center(), Vec2::new(w as f32 * scale, h as f32 * scale));
    let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
    let black = Color32::from_black_alpha(tint.a());
    for d in [
        Vec2::new(1.0, 0.0),
        Vec2::new(-1.0, 0.0),
        Vec2::new(0.0, 1.0),
        Vec2::new(0.0, -1.0),
    ] {
        painter.image(tex.id(), r.translate(d), uv, black);
    }
    painter.image(tex.id(), r, uv, tint);
}

/// A species' sprite (lists use the 48 px texture, cards the full one) painted
/// into `rect`, outlined or not.
pub fn paint_sprite(
    ui: &Ui,
    images: &mut DexImages,
    species: &str,
    dex: i32,
    rect: Rect,
    outlined: bool,
) {
    let kind = if rect.width() <= 48.0 {
        SpriteSize::Small
    } else {
        SpriteSize::Full
    };
    let Some(tex) = images.sprite(ui.ctx(), species, dex, kind) else {
        return;
    };
    if outlined {
        paint_outlined(ui.painter(), &tex, rect, Color32::WHITE);
    } else {
        crate::images::paint_fit(ui, &tex, rect, Color32::WHITE);
    }
}

/// JavaScript's `encodeURIComponent`.
pub fn encode_uri_component(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Bulbapedia's article of an ability (`Static_(Ability)`).
pub fn ability_url(name: &str) -> String {
    format!(
        "https://bulbapedia.bulbagarden.net/wiki/{}",
        encode_uri_component(&format!("{}_(Ability)", name.replace(' ', "_")))
    )
}

/// Bulbapedia's article of a species: the display name with the form
/// decorations stripped (`PokemonDetail.tsx`'s external-link button).
pub fn species_url(species: &str) -> String {
    let mut name = xpr_dex::display_name(species).to_string();
    // strip a trailing " (...)" form
    if name.ends_with(')') {
        if let Some(i) = name.find(" (") {
            name.truncate(i);
        }
    }
    // strip the form prefixes
    for p in [
        "Mega ",
        "Primal ",
        "Alolan ",
        "Galarian ",
        "Hisuian ",
        "Paldean ",
    ] {
        if let Some(rest) = name.strip_prefix(p) {
            name = rest.trim_start().to_string();
            break;
        }
    }
    // strip Mega X / Y
    for s in [" X", " Y"] {
        if let Some(rest) = name.strip_suffix(s) {
            name = rest.trim_end().to_string();
            break;
        }
    }
    format!(
        "https://bulbapedia.bulbagarden.net/wiki/{}_(Pok%C3%A9mon)",
        encode_uri_component(&name)
    )
}

/// A text job with extra letter spacing (Tailwind's `tracking-*`).
pub fn spaced_galley(
    ui: &Ui,
    text: &str,
    font: FontId,
    color: Color32,
    spacing: f32,
    wrap_width: f32,
) -> std::sync::Arc<egui::Galley> {
    let fmt = egui::TextFormat {
        font_id: font,
        extra_letter_spacing: spacing,
        color,
        ..Default::default()
    };
    let mut job = egui::text::LayoutJob::single_section(text.to_string(), fmt);
    job.wrap.max_width = wrap_width;
    ui.fonts_mut(|f| f.layout_job(job))
}

/// Solodex's bold gray uppercase section heading (`text-xs font-bold
/// text-gray-600 uppercase tracking-widest`), drawn at `pos`. Returns its rect.
pub fn paint_heading(ui: &Ui, theme: &Theme, pos: Pos2, text: &str, color: Color32) -> Rect {
    let g = spaced_galley(
        ui,
        &text.to_uppercase(),
        palette::px_bold(theme, 12.0),
        color,
        1.2,
        f32::INFINITY,
    );
    let rect = Rect::from_min_size(pos, g.size());
    ui.painter().galley(pos, g, color);
    rect
}

/// Run `f` in a child `Ui` confined to `rect` (left-to-right, centred)
/// without moving the parent's cursor (`Ui::scope_builder` allocates the
/// rect in the parent, which drags the cursor back into a fixed-height row).
pub fn in_rect<R>(
    ui: &mut Ui,
    rect: Rect,
    salt: impl std::hash::Hash,
    f: impl FnOnce(&mut Ui) -> R,
) -> R {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .id_salt(salt)
            .layout(egui::Layout::left_to_right(Align::Center)),
    );
    f(&mut child)
}

/// Greedy wrapping of items of the given widths into lines of at most
/// `max_w` (a flex-wrap row with `gap` between items).
pub fn wrap_lines(widths: &[f32], max_w: f32, gap: f32) -> Vec<Vec<usize>> {
    let mut lines: Vec<Vec<usize>> = vec![Vec::new()];
    let mut used = 0.0;
    for (i, w) in widths.iter().enumerate() {
        let line = lines.last_mut().unwrap();
        if !line.is_empty() && used + gap + w > max_w {
            lines.push(vec![i]);
            used = *w;
        } else {
            used += if line.is_empty() { *w } else { gap + w };
            line.push(i);
        }
    }
    lines
}

/// The total width of a line of items.
pub fn line_width(widths: &[f32], line: &[usize], gap: f32) -> f32 {
    line.iter().map(|i| widths[*i]).sum::<f32>() + gap * line.len().saturating_sub(1) as f32
}

// ---------------------------------------------------------------------------
// compact select (the list's filter dropdowns)
// ---------------------------------------------------------------------------

/// Solodex's filter `<select>` (`text-[11px] bg-gray-800 border
/// border-gray-700 rounded px-1 py-0.5`): `options` are `(value, label)`;
/// the empty value shows `placeholder` in gray. Returns true when changed.
pub fn select(
    ui: &mut Ui,
    theme: &Theme,
    id: Id,
    width: f32,
    value: &mut String,
    placeholder: &str,
    options: &[(String, String)],
) -> bool {
    let font = palette::px(theme, 11.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 20.0), Sense::hover());
    let resp = ui.interact(rect, id, Sense::click());
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp));
    let focused = open || resp.hovered();
    let label = options
        .iter()
        .find(|(v, _)| v == value)
        .map(|(_, l)| l.as_str())
        .unwrap_or(placeholder);
    let set = !value.is_empty();
    ui.painter().rect(
        rect,
        CornerRadius::same(4),
        palette::GRAY_800,
        Stroke::new(
            1.0_f32,
            if focused {
                palette::GRAY_500
            } else {
                palette::GRAY_700
            },
        ),
        egui::StrokeKind::Inside,
    );
    let text = xpr_ui_kit::widgets::elide(ui, label, &font, (width - 8.0 - 14.0).max(8.0));
    ui.painter().text(
        Pos2::new(rect.min.x + 5.0, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        font.clone(),
        if set {
            palette::GRAY_300
        } else {
            palette::GRAY_500
        },
    );
    // the native arrow
    let c = Pos2::new(rect.max.x - 8.0, rect.center().y);
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            c + Vec2::new(-3.5, -1.5),
            c + Vec2::new(3.5, -1.5),
            c + Vec2::new(0.0, 2.5),
        ],
        palette::GRAY_500,
        Stroke::NONE,
    ));
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let mut changed = false;
    let popup_font = palette::px(theme, 12.0);
    egui::Popup::from_toggle_button_response(&resp)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
        .frame(
            egui::Frame::new()
                .fill(palette::GRAY_800)
                .stroke(Stroke::new(1.0_f32, palette::GRAY_600))
                .corner_radius(CornerRadius::same(4))
                .inner_margin(egui::Margin::same(2)),
        )
        .show(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let widest = options
                .iter()
                .map(|(_, l)| text_w(ui, l, &popup_font))
                .fold(0.0_f32, f32::max);
            let w = (widest + 24.0).max(width).max(80.0);
            for (v, l) in options {
                let (r, rr) = ui.allocate_exact_size(Vec2::new(w, 22.0), Sense::click());
                let sel = v == value;
                if rr.hovered() {
                    ui.painter()
                        .rect_filled(r, CornerRadius::same(3), palette::GRAY_700);
                } else if sel {
                    ui.painter().rect_filled(
                        r,
                        CornerRadius::same(3),
                        palette::GRAY_700.gamma_multiply(0.5),
                    );
                }
                ui.painter().text(
                    Pos2::new(r.min.x + 8.0, r.center().y),
                    Align2::LEFT_CENTER,
                    l,
                    popup_font.clone(),
                    if v.is_empty() {
                        palette::GRAY_500
                    } else if sel {
                        Color32::WHITE
                    } else {
                        palette::GRAY_300
                    },
                );
                if rr.clicked() && !sel {
                    *value = v.clone();
                    changed = true;
                }
            }
        });
    changed
}

/// Solodex's tiny filter check box (`w-3 h-3`, label `text-[11px] text-gray-400`).
pub fn mini_checkbox(ui: &mut Ui, theme: &Theme, id: Id, checked: &mut bool, label: &str) -> bool {
    let font = palette::px(theme, 11.0);
    let lw = text_w(ui, label, &font);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(12.0 + 4.0 + lw, 16.0), Sense::hover());
    let resp = ui
        .interact(rect, id, Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let b = Rect::from_center_size(
        Pos2::new(rect.min.x + 6.0, rect.center().y),
        Vec2::splat(12.0),
    );
    if *checked {
        ui.painter()
            .rect_filled(b, CornerRadius::same(2), palette::GRAY_500);
        let c = b.center();
        ui.painter().add(egui::Shape::line(
            vec![
                c + Vec2::new(-3.0, 0.0),
                c + Vec2::new(-1.0, 2.2),
                c + Vec2::new(3.0, -2.5),
            ],
            Stroke::new(1.6_f32, Color32::WHITE),
        ));
    } else {
        ui.painter().rect(
            b,
            CornerRadius::same(2),
            palette::GRAY_800,
            Stroke::new(
                1.0_f32,
                if resp.hovered() {
                    palette::GRAY_500
                } else {
                    palette::GRAY_600
                },
            ),
            egui::StrokeKind::Inside,
        );
    }
    ui.painter().text(
        Pos2::new(rect.min.x + 16.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font,
        if resp.hovered() {
            palette::GRAY_300
        } else {
            palette::GRAY_400
        },
    );
    if resp.clicked() {
        *checked = !*checked;
        return true;
    }
    false
}

/// A right-aligned `Align` helper for flow rows: x of a line of total width
/// `total` inside `[x0, x0 + w]`.
pub fn line_start(x0: f32, w: f32, total: f32, align: Align) -> f32 {
    match align {
        Align::Min => x0,
        Align::Center => x0 + (w - total) / 2.0,
        Align::Max => x0 + w - total,
    }
}

/// The small "external link" glyph (a box with an arrow leaving its corner).
pub fn paint_external_link(painter: &Painter, rect: Rect, color: Color32) {
    let s = Stroke::new(1.5_f32, color);
    let r = rect.shrink2(Vec2::new(rect.width() * 0.12, rect.height() * 0.12));
    // the box, open at the top right
    let (l, t, rt, b) = (r.min.x, r.min.y + r.height() * 0.18, r.max.x, r.max.y);
    let notch = r.width() * 0.55;
    painter.add(egui::Shape::line(
        vec![
            Pos2::new(l + notch, t),
            Pos2::new(l, t),
            Pos2::new(l, b),
            Pos2::new(rt - r.width() * 0.18, b),
            Pos2::new(rt - r.width() * 0.18, b - notch * 0.7),
        ],
        s,
    ));
    // the arrow
    let tip = Pos2::new(rt, r.min.y);
    let tail = Pos2::new(l + r.width() * 0.42, r.min.y + r.height() * 0.58);
    painter.line_segment([tail, tip], s);
    painter.add(egui::Shape::line(
        vec![
            Pos2::new(tip.x - r.width() * 0.38, tip.y),
            tip,
            Pos2::new(tip.x, tip.y + r.height() * 0.38),
        ],
        s,
    ));
}
