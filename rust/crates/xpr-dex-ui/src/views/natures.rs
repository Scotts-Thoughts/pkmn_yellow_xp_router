//! Natures tab. Port of Solodex `NaturesView.tsx`: the 25 natures with the
//! stat each raises and lowers and the flavours it likes / dislikes, and an
//! Increase / Decrease picker that highlights the nature a pair of stats makes.

use egui::{Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use xpr_dex::model::NatureData;

use crate::palette::{self, hex, px, px_bold};
use crate::DexCx;

/// The stats a nature can change, in the order the selectors list them
/// (`STAT_KEYS`; the data's key spelling).
const STAT_KEYS: [&str; 5] = ["attack", "defense", "specialAttack", "specialDefense", "speed"];

/// Full stat names (`STAT_LABELS`).
fn stat_label(key: &str) -> &'static str {
    match key {
        "attack" => "Attack",
        "defense" => "Defense",
        "speed" => "Speed",
        "specialAttack" => "Special Attack",
        "specialDefense" => "Special Defense",
        _ => "",
    }
}

/// The stat colours of the Stats tab's bars (`STAT_COLOR`).
fn stat_color(key: &str) -> Color32 {
    hex(match key {
        "attack" => "#F8D030",
        "defense" => "#F08030",
        "speed" => "#F85888",
        "specialAttack" => "#6890F0",
        "specialDefense" => "#7038F8",
        _ => "#9ca3af",
    })
}

/// Darkened versions for cell backgrounds (`STAT_BG`).
fn stat_bg(key: &str) -> Color32 {
    hex(match key {
        "attack" => "#3d3312",
        "defense" => "#3d2412",
        "speed" => "#3d1522",
        "specialAttack" => "#1a243d",
        "specialDefense" => "#1e103d",
        _ => "#111827",
    })
}

fn flavor_bg(flavor: &str) -> Option<Color32> {
    Some(hex(match flavor {
        "spicy" => "#3d2412",
        "sour" => "#333312",
        "sweet" => "#3d1522",
        "dry" => "#1a243d",
        "bitter" => "#1e103d",
        _ => return None,
    }))
}

/// Neutral natures sit on the table's diagonal: index 0 -> Attack, 6 -> Defense,
/// 12 -> Speed, 18 -> Sp. Atk, 24 -> Sp. Def.
fn stat_by_index(index: i32) -> Option<&'static str> {
    match index {
        0 => Some("attack"),
        6 => Some("defense"),
        12 => Some("speed"),
        18 => Some("specialAttack"),
        24 => Some("specialDefense"),
        _ => None,
    }
}

fn flavor_by_index(index: i32) -> &'static str {
    match index {
        0 => "spicy",
        6 => "sour",
        12 => "sweet",
        18 => "dry",
        24 => "bitter",
        _ => "\u{2014}",
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// The nature a picked Increase / Decrease pair makes: the same stat twice is
/// that stat's neutral nature; no pair, no match.
pub fn matched_nature(increase: Option<&str>, decrease: Option<&str>) -> Option<&'static str> {
    let (inc, dec) = (increase?, decrease?);
    let list = sorted_natures();
    if inc == dec {
        list.iter().find(|(_, d)| d.increased.is_none() && d.decreased.is_none() && stat_by_index(d.index) == Some(inc)).map(|(n, _)| *n)
    } else {
        list.iter().find(|(_, d)| d.increased.as_deref() == Some(inc) && d.decreased.as_deref() == Some(dec)).map(|(n, _)| *n)
    }
}

/// All natures by table index.
fn sorted_natures() -> Vec<(&'static str, &'static NatureData)> {
    let mut list = xpr_dex::natures::natures();
    list.sort_by_key(|(_, d)| d.index);
    list
}

#[derive(Default)]
pub struct NaturesView {
    increase: Option<&'static str>,
    decrease: Option<&'static str>,
}

const TABLE_W: f32 = 680.0;
const ROW_H: f32 = 28.0;
const SELECTOR_H: f32 = 44.0;

impl NaturesView {
    /// The nature the current Increase / Decrease picks make.
    pub fn matched(&self) -> Option<&'static str> {
        matched_nature(self.increase, self.decrease)
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut DexCx) {
        let theme = cx.theme;
        let full = ui.available_rect_before_wrap();
        let content_h = SELECTOR_H * 2.0 + 8.0 + 16.0 + ROW_H * 26.0 + 1.0;
        let area = egui::ScrollArea::both().id_salt("natures_scroll").auto_shrink([false, false]);
        xpr_ui_kit::widgets::show_scroll(ui, area, |ui| {
            let avail = ui.available_rect_before_wrap();
            // centred horizontally; centred vertically when there is room (p-6 otherwise)
            let x = (avail.center().x - TABLE_W / 2.0).max(avail.min.x + 24.0);
            let y = avail.min.y + ((full.height() - content_h) / 2.0).max(24.0);
            let mut cy = y;
            for (label, is_increase) in [("Increase", true), ("Decrease", false)] {
                let rect = Rect::from_min_size(Pos2::new(x, cy), Vec2::new(TABLE_W, SELECTOR_H));
                let current = if is_increase { self.increase } else { self.decrease };
                if let Some(picked) = stat_selector(ui, theme, rect, label, is_increase, current) {
                    let slot = if is_increase { &mut self.increase } else { &mut self.decrease };
                    *slot = if *slot == Some(picked) { None } else { Some(picked) };
                }
                cy += SELECTOR_H + if is_increase { 8.0 } else { 16.0 };
            }
            let matched = self.matched();
            let table_top = cy;
            self.table(ui, theme, x, table_top, matched);
            let bottom = table_top + ROW_H * 26.0 + 1.0 + 24.0;
            ui.allocate_rect(Rect::from_min_max(avail.min, Pos2::new(x + TABLE_W + 24.0, bottom)), Sense::hover());
        });
    }

    fn table(&self, ui: &mut Ui, theme: &xpr_ui_kit::theme::Theme, x: f32, top: f32, matched: Option<&str>) {
        let border = Stroke::new(1.0_f32, palette::GRAY_600);
        // fixed layout: the "#" column is 32 px, the other five share the rest
        let rest = (TABLE_W - 32.0) / 5.0;
        let col_x = |i: usize| -> f32 {
            if i == 0 {
                x
            } else {
                x + 32.0 + ((i - 1) as f32 * rest).round()
            }
        };
        let col_w = |i: usize| -> f32 {
            if i == 5 {
                x + TABLE_W - col_x(5)
            } else if i == 0 {
                32.0
            } else {
                col_x(i + 1) - col_x(i)
            }
        };
        let cell = |ui: &Ui, row: usize, col: usize, fill: Color32, text: &str, color: Color32, bold: bool, left: bool| {
            let r = Rect::from_min_size(Pos2::new(col_x(col), top + row as f32 * ROW_H), Vec2::new(col_w(col), ROW_H + 1.0));
            ui.painter().rect_filled(r, CornerRadius::ZERO, fill);
            ui.painter().rect_stroke(r, CornerRadius::ZERO, border, StrokeKind::Inside);
            let font = if bold { px_bold(theme, 13.0) } else { px(theme, 13.0) };
            if left {
                ui.painter().text(Pos2::new(r.min.x + 9.0, r.center().y), Align2::LEFT_CENTER, text, font, color);
            } else {
                ui.painter().text(r.center(), Align2::CENTER_CENTER, text, font, color);
            }
        };
        for (c, h) in ["#", "Nature", "Increased", "Decreased", "Likes", "Dislikes"].iter().enumerate() {
            cell(ui, 0, c, palette::GRAY_800, h, palette::GRAY_400, true, c == 1);
        }
        for (i, (name, d)) in sorted_natures().iter().enumerate() {
            let row = i + 1;
            let is_neutral = d.increased.is_none() && d.decreased.is_none();
            let is_matched = matched == Some(*name);
            let row_bg = if is_matched {
                hex("#3a3616")
            } else if i % 2 == 0 {
                hex("#111827")
            } else {
                hex("#0d1117")
            };
            let diag = stat_by_index(d.index);
            cell(ui, row, 0, row_bg, &d.index.to_string(), palette::GRAY_500, false, false);
            cell(ui, row, 1, row_bg, name, Color32::WHITE, true, true);
            for (col, stat) in [(2, d.increased.as_deref()), (3, d.decreased.as_deref())] {
                match stat {
                    Some(s) => cell(ui, row, col, stat_bg(s), stat_label(s), stat_color(s), false, false),
                    None if is_neutral => {
                        let s = diag.unwrap_or("attack");
                        cell(ui, row, col, row_bg, stat_label(s), stat_color(s), false, false)
                    }
                    None => cell(ui, row, col, row_bg, "\u{2014}", palette::GRAY_400, false, false),
                }
            }
            for (col, flavor) in [(4, d.favorite_flavor.as_deref()), (5, d.disliked_flavor.as_deref())] {
                let bg = flavor.and_then(flavor_bg).unwrap_or(row_bg);
                let text = match flavor {
                    Some(f) => capitalize(f),
                    None if is_neutral => capitalize(flavor_by_index(d.index)),
                    None => "\u{2014}".to_string(),
                };
                cell(ui, row, col, bg, &text, palette::GRAY_200, false, false);
            }
            if is_matched {
                // the highlighted row: a 2 px amber outline drawn inside it
                let r = Rect::from_min_size(Pos2::new(x, top + row as f32 * ROW_H), Vec2::new(TABLE_W, ROW_H + 1.0));
                ui.painter().rect_stroke(r, CornerRadius::ZERO, Stroke::new(2.0_f32, hex("#fbbf24")), StrokeKind::Inside);
            }
        }
    }
}

/// One Increase / Decrease row: a tinted bar with the five stat buttons.
/// Returns the stat clicked.
fn stat_selector(ui: &mut Ui, theme: &xpr_ui_kit::theme::Theme, rect: Rect, label: &str, increase: bool, selected: Option<&'static str>) -> Option<&'static str> {
    let (label_color, bg) = if increase { (hex("#ef4444"), Color32::from_rgba_unmultiplied(239, 68, 68, 31)) } else { (hex("#3b82f6"), Color32::from_rgba_unmultiplied(59, 130, 246, 31)) };
    ui.painter().rect_filled(rect, CornerRadius::same(8), bg);
    ui.painter().text(Pos2::new(rect.min.x + 12.0, rect.center().y), Align2::LEFT_CENTER, format!("{}:", label), px_bold(theme, 13.0), label_color);
    let x0 = rect.min.x + 8.0 + 80.0 + 8.0;
    let x1 = rect.max.x - 8.0;
    let gap = 6.0;
    let bw = (x1 - x0 - gap * 4.0) / 5.0;
    let mut clicked = None;
    for (i, key) in STAT_KEYS.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(x0 + i as f32 * (bw + gap), rect.min.y + 6.0), Vec2::new(bw, rect.height() - 12.0));
        let resp = ui.interact(r, ui.id().with(("nature_stat", label, *key)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        let is_sel = selected == Some(*key);
        let (fill, line, text) = if is_sel { (stat_bg(key), stat_color(key), stat_color(key)) } else { (hex("#111827"), hex("#374151"), hex("#9ca3af")) };
        ui.painter().rect(r, CornerRadius::same(4), fill, Stroke::new(1.0_f32, line), StrokeKind::Inside);
        ui.painter().text(r.center(), Align2::CENTER_CENTER, stat_label(key), px_bold(theme, 12.0), text);
        if resp.clicked() {
            clicked = Some(*key);
        }
    }
    clicked
}
