//! The move detail (Solodex `MoveDetailView.tsx`): one card per run of
//! generations with identical stats, change callouts between them, and (an
//! addition) the species that learn the move in the current game.

use std::sync::Arc;

use egui::epaint::text::{LayoutJob, TextFormat};
use egui::{Align, Align2, Color32, CornerRadius, FontId, Galley, Id, Layout, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2};
use xpr_ui_kit::theme::Theme;

use super::data::{self, Change, Field, GenGroup, Val};
use super::kit::{self, tw, RULE};
use super::learners::{self, Learners, Style};
use super::MovedexView;
use crate::palette::{self, hex};
use crate::widgets::{self, text_w};
use crate::{DexAction, DexCx};

#[derive(Default)]
pub(super) struct Detail {
    learners: Learners,
}

const HEADER_H: f32 = 52.0;

/// Tailwind `border-gray-700/60`.
fn card_border() -> Color32 {
    tw("#374151", 0.6)
}

/// The rounded header strip of a card (`bg-gray-800/60`, square below).
fn paint_card_header(ui: &Ui, head: Rect) {
    ui.painter().rect_filled(head, CornerRadius { nw: 7, ne: 7, sw: 0, se: 0 }, tw("#1f2937", 0.6));
}

fn galley(ui: &Ui, text: &str, font: FontId, color: Color32, wrap: f32) -> Arc<Galley> {
    ui.fonts_mut(|f| f.layout(text.to_string(), font, color, wrap))
}

fn italic_galley(ui: &Ui, text: &str, font: FontId, color: Color32, wrap: f32) -> Arc<Galley> {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    job.append(text, 0.0, TextFormat { font_id: font, color, italics: true, ..Default::default() });
    ui.fonts_mut(|f| f.layout_job(job))
}

impl MovedexView {
    pub(super) fn detail_ui(&mut self, ui: &mut Ui, cx: &mut DexCx, name: &str) {
        let theme = cx.theme;
        let full = ui.available_rect_before_wrap();
        let history = xpr_dex::move_across_gens(name);
        let groups = data::collapse_gens(&history);
        let game = cx.state.game().to_string();

        // header: Back, name, generations, Bulbapedia
        let header = Rect::from_min_size(full.min, Vec2::new(full.width(), HEADER_H));
        let back_font = palette::px(theme, 14.0);
        let back_w = text_w(ui, "\u{2190} Back", &back_font);
        let back = Rect::from_min_size(Pos2::new(header.min.x + 20.0, header.center().y - 12.0), Vec2::new(back_w, 24.0));
        let back_resp = ui.interact(back, Id::new("movedex_detail_back"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        ui.painter().text(back.left_center(), Align2::LEFT_CENTER, "\u{2190} Back", back_font, if back_resp.hovered() { Color32::WHITE } else { palette::GRAY_400 });
        let title = ui.painter().text(Pos2::new(back.max.x + 12.0, header.center().y), Align2::LEFT_CENTER, name, palette::px_bold(theme, 18.0), Color32::WHITE);
        if let (Some((first, _)), Some((last, _))) = (history.first(), history.last()) {
            let label = if history.len() > 1 { format!("Gen {}\u{2013}{}", first, last) } else { format!("Gen {}", first) };
            ui.painter().text(Pos2::new(title.max.x + 12.0, header.center().y + 1.0), Align2::LEFT_CENTER, label, palette::px(theme, 12.0), palette::GRAY_500);
        }
        let bulba_w = text_w(ui, "Bulbapedia", &palette::px(theme, 12.0)) + 16.0;
        let bulba = Rect::from_min_size(Pos2::new(header.max.x - 20.0 - bulba_w, header.center().y - 11.0), Vec2::new(bulba_w, 22.0));
        let mut bui = ui.new_child(UiBuilder::new().id_salt("movedex_bulba").max_rect(bulba));
        if widgets::small_button(&mut bui, theme, "Bulbapedia").on_hover_text("Open the move's Bulbapedia article").clicked() {
            cx.actions.push(DexAction::OpenUrl(data::bulbapedia_url(name)));
        }
        kit::rule_bottom(ui, header, RULE);
        if back_resp.clicked() {
            cx.state.focused_move = None;
            ui.ctx().request_repaint();
        }

        // body
        let body = Rect::from_min_max(Pos2::new(full.min.x, header.max.y), full.max);
        let mut sui = ui.new_child(UiBuilder::new().id_salt("movedex_detail_body").max_rect(body).layout(Layout::top_down(Align::Min)));
        sui.set_clip_rect(body.intersect(ui.clip_rect()));
        let mut clicked = None;
        egui::ScrollArea::vertical().id_salt("movedex_detail").auto_shrink([false, false]).show(&mut sui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let avail = ui.available_width();
            let col_w = (avail - 40.0).clamp(240.0, 672.0);
            ui.add_space(16.0);
            for (i, g) in groups.iter().enumerate() {
                if i > 0 && !g.changes.is_empty() {
                    callout(ui, theme, avail, col_w, &groups[i - 1], g);
                }
                gen_card(ui, theme, avail, col_w, g);
            }
            if groups.is_empty() {
                let (r, _) = ui.allocate_exact_size(Vec2::new(avail, 96.0), Sense::hover());
                ui.painter().text(r.center(), Align2::CENTER_CENTER, "No data found for this move.", palette::px(theme, 16.0), palette::GRAY_500);
            } else {
                ui.add_space(20.0);
                clicked = self.learners_card(ui, cx, avail, col_w, name, &game, &history);
            }
            ui.add_space(24.0);
        });
        if let Some(species) = clicked {
            learners::open_in_pokedex(cx, &species);
            ui.ctx().request_repaint();
        }
        ui.allocate_rect(full, Sense::hover());
    }

    /// "Learned in <game>" card: who learns the move in the current game and
    /// how. Clicking a species returns it.
    #[allow(clippy::too_many_arguments)]
    fn learners_card(&mut self, ui: &mut Ui, cx: &mut DexCx, avail: f32, col_w: f32, name: &str, game: &str, history: &[(u8, &xpr_dex::MoveData)]) -> Option<String> {
        let theme = cx.theme;
        let gen = xpr_dex::game_gen(game);
        let in_game = history.iter().any(|(g, _)| *g == gen);
        let game_name = xpr_dex::move_name_for_game(name, game);
        let header_h = 36.0;
        let style = Style { sprite: true, name_px: 14.0, width: col_w - 2.0 };
        let count = if in_game { self.detail.learners.learners(&game_name, game).len() } else { 0 };
        let rows_h = if in_game && count > 0 { self.detail.learners.height(ui, theme, style) } else { 56.0 };
        let (full, _) = ui.allocate_exact_size(Vec2::new(avail, header_h + rows_h + 2.0), Sense::hover());
        let card = Rect::from_center_size(full.center(), Vec2::new(col_w, full.height()));
        let head = Rect::from_min_size(card.min + Vec2::splat(1.0), Vec2::new(card.width() - 2.0, header_h));
        paint_card_header(ui, head);
        ui.painter().text(Pos2::new(head.min.x + 16.0, head.center().y), Align2::LEFT_CENTER, format!("Learned in {}", game), palette::px_bold(theme, 14.0), palette::GRAY_200);
        let mut clicked = None;
        if in_game {
            ui.painter().text(Pos2::new(head.max.x - 16.0, head.center().y), Align2::RIGHT_CENTER, format!("{} Pok\u{e9}mon", count), palette::px(theme, 12.0), palette::GRAY_500);
            if count == 0 {
                ui.painter().text(Pos2::new(card.center().x, head.max.y + 28.0), Align2::CENTER_CENTER, format!("No Pokemon learn this move in {}.", game), palette::px(theme, 14.0), palette::GRAY_500);
            } else {
                let area = Rect::from_min_size(Pos2::new(card.min.x + 1.0, head.max.y), Vec2::new(style.width, rows_h));
                clicked = self.detail.learners.show(ui, cx, area, style, Id::new("movedex_detail_rows"));
            }
        } else {
            ui.painter().text(Pos2::new(card.center().x, head.max.y + 28.0), Align2::CENTER_CENTER, format!("This move does not exist in {}.", game), palette::px(theme, 14.0), palette::GRAY_500);
        }
        ui.painter().rect_stroke(card, CornerRadius::same(8), Stroke::new(1.0_f32, card_border()), StrokeKind::Inside);
        clicked
    }
}

// ---------------------------------------------------------------------------
// generation card
// ---------------------------------------------------------------------------

const FLAGS: [(Field, &str); 6] = [
    (Field::Contact, "Contact"),
    (Field::Protect, "Protect"),
    (Field::MagicCoat, "Magic Coat"),
    (Field::Snatch, "Snatch"),
    (Field::MirrorMove, "Mirror Move"),
    (Field::KingsRock, "King's Rock"),
];

fn gen_card(ui: &mut Ui, theme: &Theme, avail: f32, col_w: f32, g: &GenGroup) {
    let d = &g.data;
    let inner_w = col_w - 2.0;
    let text_w_max = inner_w - 32.0;
    let label_font = palette::px(theme, 12.0);
    let label_col = 64.0 + 8.0;
    let effect = galley(ui, &data::format_effect(&d.effect), label_font.clone(), palette::GRAY_300, text_w_max - label_col);
    let target = galley(ui, &d.target, label_font.clone(), palette::GRAY_300, text_w_max - label_col);
    let desc = italic_galley(ui, &d.description, label_font.clone(), palette::GRAY_500, text_w_max);
    // flag pills, wrapped
    let pills: Vec<f32> = FLAGS.iter().map(|(_, l)| text_w(ui, l, &label_font) + 18.0).collect();
    let mut pill_pos = Vec::new();
    let (mut x, mut y) = (0.0_f32, 0.0_f32);
    for w in &pills {
        if x > 0.0 && x + w > text_w_max {
            x = 0.0;
            y += 22.0 + 6.0;
        }
        pill_pos.push(Vec2::new(x, y));
        x += w + 6.0;
    }
    let flags_h = y + 22.0;
    let effect_h = effect.size().y.max(16.0);
    let target_h = target.size().y.max(16.0);
    let header_h = 36.0;
    let stats_h = 62.0 + 1.0;
    let details_h = 12.0 + effect_h + 8.0 + target_h + 8.0 + 4.0 + flags_h + 8.0 + 4.0 + desc.size().y + 12.0;
    let height = 2.0 + header_h + stats_h + details_h;
    let (full, _) = ui.allocate_exact_size(Vec2::new(avail, height), Sense::hover());
    let card = Rect::from_center_size(full.center(), Vec2::new(col_w, height));
    let inner = card.shrink(1.0);

    // header: title, type badge, category
    let head = Rect::from_min_size(inner.min, Vec2::new(inner.width(), header_h));
    paint_card_header(ui, head);
    let title = if g.start == g.end { format!("Generation {}", g.start) } else { format!("Generations {}\u{2013}{}", g.start, g.end) };
    ui.painter().text(Pos2::new(head.min.x + 16.0, head.center().y), Align2::LEFT_CENTER, title, palette::px_bold(theme, 14.0), palette::GRAY_200);
    let cat_r = ui.painter().text(Pos2::new(head.max.x - 16.0, head.center().y), Align2::RIGHT_CENTER, &d.category, palette::px_bold(theme, 12.0), palette::category_color(&d.category));
    let badge = Rect::from_min_size(Pos2::new(cat_r.min.x - 12.0 - 68.0, head.center().y - 9.0), Vec2::new(68.0, 18.0));
    let mut bui = ui.new_child(UiBuilder::new().id_salt(("movedex_card_badge", g.start)).max_rect(badge));
    widgets::type_badge(&mut bui, theme, &d.move_type, true, None);

    // stats row
    let stats = Rect::from_min_size(Pos2::new(inner.min.x, head.max.y), Vec2::new(inner.width(), stats_h));
    let cell_w = (inner.width() - 32.0 - 4.0 * 12.0) / 5.0;
    let cells: [(&str, Option<i32>); 5] = [("Power", d.power), ("Accuracy", d.accuracy), ("PP", d.pp), ("Priority", Some(d.priority)), ("Effect %", d.effect_chance)];
    let p = ui.painter();
    for (i, (label, value)) in cells.iter().enumerate() {
        let cx0 = stats.min.x + 16.0 + i as f32 * (cell_w + 12.0) + cell_w / 2.0;
        p.text(Pos2::new(cx0, stats.min.y + 12.0 + 8.0), Align2::CENTER_CENTER, *label, label_font.clone(), palette::GRAY_500);
        let (text, color) = match value {
            Some(v) => (v.to_string(), palette::GRAY_100),
            None => ("\u{2014}".to_string(), palette::GRAY_600),
        };
        p.text(Pos2::new(cx0, stats.min.y + 12.0 + 16.0 + 2.0 + 10.0), Align2::CENTER_CENTER, text, palette::px_bold(theme, 14.0), color);
    }
    p.hline(stats.x_range(), stats.max.y - 0.5, Stroke::new(1.0_f32, tw("#1f2937", 0.6)));

    // details
    let mut y = stats.max.y + 12.0;
    let x0 = inner.min.x + 16.0;
    p.text(Pos2::new(x0, y + 8.0), Align2::LEFT_CENTER, "Effect", label_font.clone(), palette::GRAY_500);
    p.galley(Pos2::new(x0 + label_col, y), effect, palette::GRAY_300);
    y += effect_h + 8.0;
    p.text(Pos2::new(x0, y + 8.0), Align2::LEFT_CENTER, "Target", label_font.clone(), palette::GRAY_500);
    p.galley(Pos2::new(x0 + label_col, y), target, palette::GRAY_300);
    y += target_h + 8.0 + 4.0;
    for (i, (field, label)) in FLAGS.iter().enumerate() {
        let on = matches!(field.get(d), Val::Bool(true));
        let r = Rect::from_min_size(Pos2::new(x0, y) + pill_pos[i], Vec2::new(pills[i], 22.0));
        let (fill, border, text) = if on { (tw("#14532d", 0.4), tw("#15803d", 0.4), hex("#4ade80")) } else { (tw("#1f2937", 0.6), tw("#374151", 0.4), palette::GRAY_600) };
        p.rect(r, CornerRadius::same(11), fill, Stroke::new(1.0_f32, border), StrokeKind::Inside);
        p.text(r.center(), Align2::CENTER_CENTER, *label, label_font.clone(), text);
    }
    y += flags_h + 8.0 + 4.0;
    p.galley(Pos2::new(x0, y), desc, palette::GRAY_500);
    p.rect_stroke(card, CornerRadius::same(8), Stroke::new(1.0_f32, card_border()), StrokeKind::Inside);
}

// ---------------------------------------------------------------------------
// change callout between two cards
// ---------------------------------------------------------------------------

enum Piece {
    Text { text: String, font: FontId, color: Color32, w: f32 },
    /// a small type badge (the old one dimmed)
    Badge { t: String, dim: bool },
}

impl Piece {
    fn width(&self) -> f32 {
        match self {
            Piece::Text { w, .. } => *w,
            Piece::Badge { .. } => 68.0,
        }
    }
}

/// One change chip: label and old -> new.
struct ChipLayout {
    pieces: Vec<Piece>,
    w: f32,
    h: f32,
}

fn chip_layout(ui: &Ui, theme: &Theme, c: &Change) -> ChipLayout {
    let font = palette::px(theme, 12.0);
    let bold = palette::px_bold(theme, 12.0);
    let text = |s: String, font: &FontId, color: Color32| {
        let w = text_w(ui, &s, font);
        Piece::Text { text: s, font: font.clone(), color, w }
    };
    let mut pieces = vec![text(c.field.label().to_string(), &bold, tw("#eab308", 0.7))];
    let arrow = || text("\u{2192}".to_string(), &font, palette::YELLOW_400);
    let (old, new) = (c.old.shown(), c.new.shown());
    match c.field {
        Field::Type => {
            pieces.push(Piece::Badge { t: old, dim: true });
            pieces.push(arrow());
            pieces.push(Piece::Badge { t: new, dim: false });
        }
        Field::Category => {
            let old_color = palette::category_color(&old).gamma_multiply(0.6);
            let new_color = palette::category_color(&new);
            pieces.push(text(old, &font, old_color));
            pieces.push(arrow());
            pieces.push(text(new, &font, new_color));
        }
        _ => {
            pieces.push(text(old, &font, palette::GRAY_400));
            pieces.push(arrow());
            pieces.push(text(new, &bold, hex("#fde047")));
        }
    }
    let has_badge = pieces.iter().any(|p| matches!(p, Piece::Badge { .. }));
    let w = 16.0 + pieces.iter().map(Piece::width).sum::<f32>() + 6.0 * (pieces.len() - 1) as f32 + 2.0;
    ChipLayout { pieces, w, h: if has_badge { 24.0 } else { 22.0 } }
}

fn callout(ui: &mut Ui, theme: &Theme, avail: f32, col_w: f32, prev: &GenGroup, g: &GenGroup) {
    let font = palette::px_bold(theme, 12.0);
    let label = format!("Gen {} \u{2192} {}", prev.end, g.start);
    let label_w = text_w(ui, &label, &font) + 16.0;
    let chips: Vec<ChipLayout> = g.changes.iter().map(|c| chip_layout(ui, theme, c)).collect();
    // flow: the label, then the chips, wrapped and centred (gap-2)
    let cw = col_w - 32.0;
    let mut items: Vec<(f32, f32)> = vec![(label_w, 16.0)];
    items.extend(chips.iter().map(|c| (c.w, c.h)));
    let mut lines: Vec<Vec<usize>> = vec![Vec::new()];
    let mut x = 0.0;
    for (i, (w, _)) in items.iter().enumerate() {
        if x > 0.0 && x + w > cw {
            lines.push(Vec::new());
            x = 0.0;
        }
        lines.last_mut().unwrap().push(i);
        x += w + 8.0;
    }
    let line_h: Vec<f32> = lines.iter().map(|l| l.iter().map(|i| items[*i].1).fold(0.0_f32, f32::max)).collect();
    let content_h = line_h.iter().sum::<f32>() + 8.0 * (lines.len() - 1) as f32;
    let (full, _) = ui.allocate_exact_size(Vec2::new(avail, content_h + 24.0), Sense::hover());
    let card = Rect::from_center_size(full.center(), Vec2::new(col_w, full.height()));
    ui.painter().hline(card.x_range(), full.center().y, Stroke::new(1.0_f32, tw("#eab308", 0.3)));
    let bg = palette::page_bg(theme);
    let mut y = full.min.y + 12.0;
    for (li, line) in lines.iter().enumerate() {
        let total: f32 = line.iter().map(|i| items[*i].0).sum::<f32>() + 8.0 * (line.len() - 1) as f32;
        let mut x = full.center().x - total / 2.0;
        for &i in line {
            let (w, h) = items[i];
            let r = Rect::from_min_size(Pos2::new(x, y + (line_h[li] - h) / 2.0), Vec2::new(w, h));
            if i == 0 {
                // the label hides the rule behind it
                ui.painter().rect_filled(r, CornerRadius::ZERO, bg);
                ui.painter().text(r.center(), Align2::CENTER_CENTER, &label, font.clone(), palette::YELLOW_500);
            } else {
                paint_chip(ui, theme, &chips[i - 1], r);
            }
            x += w + 8.0;
        }
        y += line_h[li] + 8.0;
    }
}

fn paint_chip(ui: &mut Ui, theme: &Theme, chip: &ChipLayout, r: Rect) {
    ui.painter().rect(r, CornerRadius::same(4), tw("#eab308", 0.1), Stroke::new(1.0_f32, tw("#eab308", 0.3)), StrokeKind::Inside);
    let mut x = r.min.x + 8.0;
    for p in &chip.pieces {
        match p {
            Piece::Text { text, font, color, w } => {
                ui.painter().text(Pos2::new(x, r.center().y), Align2::LEFT_CENTER, text, font.clone(), *color);
                x += w;
            }
            Piece::Badge { t, dim } => {
                let rect = Rect::from_min_size(Pos2::new(x, r.center().y - 9.0), Vec2::new(68.0, 18.0));
                let mut bui = ui.new_child(UiBuilder::new().id_salt(("movedex_chip_badge", rect.min.x as i32, rect.min.y as i32)).max_rect(rect));
                if *dim {
                    bui.multiply_opacity(0.6);
                }
                // Solodex's small badge: a click shows the type's matchups (gen 4 chart)
                widgets::type_badge(&mut bui, theme, t, true, None);
                x += 68.0;
            }
        }
        x += 6.0;
    }
}
