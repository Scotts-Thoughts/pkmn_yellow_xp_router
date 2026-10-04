//! The left column: search, class / location filters and the windowed
//! trainer list (Solodex `TrainerList.tsx`).

use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{Color32, CornerRadius, Id, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use xpr_ui_kit::theme::Theme;

use super::data::VersionIndex;
use crate::palette::{self, px};
use crate::widgets;

/// py-1.5 + name line + mt-0.5 + party line + 1 px border
const ROW_H: f32 = 51.0;
const ALL_CLASSES: &str = "All Classes";
const ALL_LOCATIONS: &str = "All Locations";

#[derive(Default)]
pub struct ListState {
    pub search: String,
    /// "" = all
    pub class: String,
    pub location: String,
    /// list rows (indices into `VersionIndex::rows`) after the filters
    filtered: Vec<usize>,
    built_for: Option<(usize, String, String, String)>,
    /// scrolling that keeps the selected row in view
    scrolled_to: Option<usize>,
    offset: f32,
    settle: u8,
}

impl ListState {
    /// Clear the filters (the game changed).
    pub fn reset(&mut self) {
        self.search.clear();
        self.class.clear();
        self.location.clear();
        self.built_for = None;
        self.scrolled_to = None;
        self.offset = 0.0;
    }
}

fn rebuild(ix: &VersionIndex, st: &mut ListState) {
    let key = (ix as *const VersionIndex as usize, st.search.clone(), st.class.clone(), st.location.clone());
    if st.built_for.as_ref() == Some(&key) {
        return;
    }
    let q = st.search.to_lowercase();
    st.filtered = ix
        .rows
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            let t = &ix.trainers[r.primary].trainer;
            (q.is_empty() || r.name.to_lowercase().contains(&q) || t.trainer_class.to_lowercase().contains(&q) || (!t.location.is_empty() && t.location.to_lowercase().contains(&q)))
                && (st.class.is_empty() || t.trainer_class == st.class)
                && (st.location.is_empty() || t.location == st.location)
        })
        .map(|(i, _)| i)
        .collect();
    st.built_for = Some(key);
    st.scrolled_to = None;
}

fn fmt(theme: &Theme, size: f32, color: Color32) -> TextFormat {
    TextFormat { font_id: px(theme, size), color, ..Default::default() }
}

/// A one-line galley that ends in "…" when it does not fit `max_w`.
fn one_line(ui: &Ui, mut job: LayoutJob, max_w: f32) -> std::sync::Arc<egui::Galley> {
    job.wrap = TextWrapping::truncate_at_width(max_w.max(8.0));
    ui.fonts_mut(|f| f.layout_job(job))
}

/// Draw the list column; returns the trainer picked (an index into the
/// version's trainers).
pub fn list_ui(ui: &mut Ui, theme: &Theme, ix: &VersionIndex, st: &mut ListState, selected: Option<usize>, width: f32) -> Option<usize> {
    let mut picked = None;
    rebuild(ix, st);
    let full = ui.available_rect_before_wrap();
    ui.painter().rect_filled(full, CornerRadius::ZERO, palette::GRAY_900);

    // ---- search + filters (p-2, gap 6, border-b gray-700)
    let mut y = full.min.y + 8.0;
    let inner_w = full.width() - 16.0;
    let search_rect = Rect::from_min_size(Pos2::new(full.min.x + 8.0, y), Vec2::new(inner_w, 28.0));
    {
        let mut sui = ui.new_child(egui::UiBuilder::new().max_rect(search_rect));
        widgets::search_field(&mut sui, theme, &mut st.search, "Search trainers...", Id::new("trainer_search"), inner_w);
    }
    y += 28.0 + 6.0;
    let sel_w = (inner_w - 4.0) / 2.0;
    let sel_rect = Rect::from_min_size(Pos2::new(full.min.x + 8.0, y), Vec2::new(inner_w, 24.0));
    {
        let mut sui = ui.new_child(egui::UiBuilder::new().max_rect(sel_rect).layout(egui::Layout::left_to_right(egui::Align::Min)));
        sui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
        let mut cur = if st.class.is_empty() { ALL_CLASSES.to_string() } else { st.class.clone() };
        let mut options = vec![ALL_CLASSES.to_string()];
        options.extend(ix.classes.iter().cloned());
        if xpr_ui_kit::widgets::option_menu(&mut sui, theme, Id::new("trainer_class_filter"), &mut cur, &options, Some(sel_w), true) {
            st.class = if cur == ALL_CLASSES { String::new() } else { cur };
        }
        let mut cur = if st.location.is_empty() { ALL_LOCATIONS.to_string() } else { st.location.clone() };
        let mut options = vec![ALL_LOCATIONS.to_string()];
        options.extend(ix.locations.iter().cloned());
        let loc_w = sel_w.min(width * 0.5);
        if xpr_ui_kit::widgets::option_menu(&mut sui, theme, Id::new("trainer_location_filter"), &mut cur, &options, Some(loc_w), true) {
            st.location = if cur == ALL_LOCATIONS { String::new() } else { cur };
        }
    }
    y += 24.0 + 8.0;
    ui.painter().hline(full.x_range(), y + 0.5, Stroke::new(1.0_f32, palette::GRAY_700));
    y += 1.0;

    // ---- count
    let n = st.filtered.len();
    let count_h = 24.0;
    ui.painter().text(Pos2::new(full.min.x + 10.0, y + count_h / 2.0), egui::Align2::LEFT_CENTER, format!("{} trainer{}", n, if n == 1 { "" } else { "s" }), px(theme, 12.0), palette::GRAY_500);
    y += count_h;
    ui.painter().hline(full.x_range(), y + 0.5, Stroke::new(1.0_f32, palette::GRAY_800));
    y += 1.0;

    // ---- the windowed list
    let list = Rect::from_min_max(Pos2::new(full.min.x, y), full.max);
    let mut lui = ui.new_child(egui::UiBuilder::new().max_rect(list).layout(egui::Layout::top_down(egui::Align::Min)));
    lui.set_clip_rect(list.intersect(ui.clip_rect()));
    let sel_row = st.filtered.iter().position(|ri| ix.row_is_selected(&ix.rows[*ri], selected));
    let mut area = egui::ScrollArea::vertical().id_salt("trainer_list_rows").auto_shrink([false, false]);
    // keep the selected row in view whenever it changes (scrollIntoView "nearest"); the
    // page may still be settling its layout for a frame or two, so apply it for a few frames
    if sel_row != st.scrolled_to {
        st.scrolled_to = sel_row;
        st.settle = if sel_row.is_some() { 4 } else { 0 };
    }
    if st.settle > 0 {
        st.settle -= 1;
        if let Some(r) = sel_row {
            let top = r as f32 * ROW_H;
            let bottom = top + ROW_H;
            let mut off = st.offset;
            if top < off {
                off = top;
            } else if bottom > off + list.height() {
                off = bottom - list.height();
            }
            area = area.vertical_scroll_offset(off.max(0.0));
        }
    }
    // show_rows adds the *outer* ui's item spacing to every row height
    lui.spacing_mut().item_spacing = Vec2::ZERO;
    let out = area.show_rows(&mut lui, ROW_H, n, |ui, range| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        for r in range {
            let row = &ix.rows[st.filtered[r]];
            let t = &ix.trainers[row.primary];
            let is_sel = ix.row_is_selected(row, selected);
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_H), Sense::click());
            let p = ui.painter();
            if is_sel {
                p.rect_filled(rect, CornerRadius::ZERO, Color32::from_rgba_unmultiplied(0x1e, 0x3a, 0x8a, 102));
                p.rect_filled(Rect::from_min_size(rect.min, Vec2::new(2.0, ROW_H)), CornerRadius::ZERO, palette::BLUE_500);
            } else if resp.hovered() {
                p.rect_filled(rect, CornerRadius::ZERO, Color32::from_rgba_unmultiplied(0x1f, 0x29, 0x37, 153));
            }
            p.hline(rect.x_range(), rect.max.y - 0.5, Stroke::new(1.0_f32, palette::GRAY_800));
            let x = rect.min.x + 12.0;
            let right = rect.max.x - 10.0;
            // line 1: name (+ id) and the ace level
            let lv = format!("Lv{}", t.max_level);
            let lv_galley = ui.fonts_mut(|f| f.layout_no_wrap(lv, px(theme, 12.0), palette::GRAY_500));
            let name_w = right - x - lv_galley.size().x - 8.0;
            let mut job = LayoutJob::default();
            job.append(&row.name, 0.0, fmt(theme, 14.0, t.name_color()));
            // the id names one member, so a group shows none
            if let Some(id) = t.id_suffix().filter(|_| row.group.is_none()) {
                job.append(&id, 4.0, fmt(theme, 14.0, palette::GRAY_600));
            }
            let g = one_line(ui, job, name_w);
            let c1 = rect.min.y + 6.0 + 10.0;
            ui.painter().galley(Pos2::new(x, c1 - g.size().y / 2.0), g, Color32::WHITE);
            ui.painter().galley(Pos2::new(right - lv_galley.size().x, c1 - lv_galley.size().y / 2.0 + 1.0), lv_galley, palette::GRAY_500);
            // line 2: the party
            let mut job = LayoutJob::default();
            for (i, m) in t.party.iter().enumerate() {
                if i > 0 {
                    job.append(" / ", 0.0, fmt(theme, 12.0, palette::GRAY_700));
                }
                job.append(&m.display, 0.0, fmt(theme, 12.0, palette::GRAY_400));
                job.append(&m.level.to_string(), 2.0, fmt(theme, 12.0, palette::GRAY_600));
            }
            let g = one_line(ui, job, right - x);
            let c2 = rect.min.y + 6.0 + 20.0 + 2.0 + 8.0;
            ui.painter().galley(Pos2::new(x, c2 - g.size().y / 2.0), g, Color32::WHITE);
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                picked = Some(row.primary);
            }
        }
    });
    st.offset = out.state.offset.y;
    picked
}
