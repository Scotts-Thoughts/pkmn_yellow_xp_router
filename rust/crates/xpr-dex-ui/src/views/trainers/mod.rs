//! Trainers tab: trainer list, trainer detail (with the team order
//! calculator) and the speed list, on the router's trainer data. Port of
//! Solodex `TrainerList.tsx`, `TrainerDetail.tsx`, `TrainerSpeedList.tsx`,
//! `TeamOrderCalculator.tsx`, `TrainerSpotlightSearch.tsx`.
//!
//! Layout like Solodex: list | detail | speed list. The router's names,
//! classes and fight categories replace Solodex's name heuristics:
//! `GenData::get_fight_category` gives the colours and the "major fight"
//! split, and rematches / repeated fights are grouped (see `grouping`).

mod card;
mod data;
mod detail;
mod grouping;
mod list;
mod order;
mod order_ui;
mod speed;
mod spotlight;

use std::sync::Arc;

use egui::{CornerRadius, Pos2, Rect, Sense, Ui, UiBuilder, Vec2};

use self::data::{index_for, VersionIndex};
use self::detail::DetailState;
use self::list::ListState;
use self::speed::SpeedState;
use crate::palette;
use crate::widgets;
use crate::DexCx;

pub use self::spotlight::spotlight_rows;

#[derive(Default)]
pub struct TrainersView {
    version: Option<String>,
    ix: Option<Arc<VersionIndex>>,
    list: ListState,
    speed: SpeedState,
    detail: DetailState,
    drag: bool,
}

impl TrainersView {
    pub fn ui(&mut self, ui: &mut Ui, cx: &mut DexCx) {
        let theme = cx.theme;
        let Some(version) = cx.state.settings.version.clone() else {
            widgets::placeholder(ui, theme, "No trainer data for this game");
            return;
        };
        let Some(ix) = index_for(cx.registry, &version) else {
            widgets::placeholder(ui, theme, &format!("No trainer data for {}", version));
            return;
        };
        // the game changed: the filters and the shown variant start over
        if self.version.as_deref() != Some(version.as_str()) {
            self.version = Some(version.clone());
            self.list.reset();
            self.detail = DetailState::default();
        }
        self.ix = Some(ix.clone());

        let full = ui.available_rect_before_wrap();
        let max_w = ((full.width() - 5.0 - 300.0) / 2.0).max(150.0);
        let side_w = cx.state.settings.list_width.min(max_w);
        let list_rect = Rect::from_min_size(full.min, Vec2::new(side_w, full.height()));
        let handle = Rect::from_min_size(Pos2::new(list_rect.max.x, full.min.y), Vec2::new(4.0, full.height()));
        let speed_rect = Rect::from_min_max(Pos2::new(full.max.x - side_w, full.min.y), full.max);
        let detail_rect = Rect::from_min_max(Pos2::new(handle.max.x, full.min.y), Pos2::new(speed_rect.min.x - 1.0, full.max.y));
        let selected = cx.state.selected_trainer.as_deref().and_then(|n| ix.find(n));

        let mut picked: Option<usize> = None;
        // list
        {
            let mut lui = ui.new_child(UiBuilder::new().max_rect(list_rect).layout(egui::Layout::top_down(egui::Align::Min)));
            lui.set_clip_rect(list_rect.intersect(ui.clip_rect()));
            if let Some(p) = list::list_ui(&mut lui, theme, &ix, &mut self.list, selected, side_w) {
                picked = Some(p);
            }
        }
        // splitter
        let resp = ui.interact(handle, ui.id().with("trainers_splitter"), Sense::drag());
        let hot = resp.hovered() || resp.dragged();
        ui.painter().rect_filled(handle, CornerRadius::ZERO, if hot { palette::BLUE_500 } else { palette::GRAY_700 });
        if hot {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        if resp.dragged() {
            cx.state.settings.list_width = (cx.state.settings.list_width + resp.drag_delta().x).clamp(180.0, max_w.max(180.0));
            self.drag = true;
        } else if self.drag {
            self.drag = false;
            cx.state.touch();
        }
        // detail
        {
            let mut dui = ui.new_child(UiBuilder::new().max_rect(detail_rect).layout(egui::Layout::top_down(egui::Align::Min)));
            dui.set_clip_rect(detail_rect.intersect(ui.clip_rect()));
            detail::detail_ui(&mut dui, cx, &ix, selected, &mut self.detail);
        }
        // speed list, with a border on its left
        ui.painter().rect_filled(Rect::from_min_size(Pos2::new(speed_rect.min.x - 1.0, full.min.y), Vec2::new(1.0, full.height())), CornerRadius::ZERO, palette::GRAY_700);
        {
            let mut sui = ui.new_child(UiBuilder::new().max_rect(speed_rect).layout(egui::Layout::top_down(egui::Align::Min)));
            sui.set_clip_rect(speed_rect.intersect(ui.clip_rect()));
            if let Some(p) = speed::speed_ui(&mut sui, theme, &ix, &mut self.speed) {
                picked = Some(p);
            }
        }
        ui.allocate_rect(full, Sense::hover());

        if let Some(i) = picked {
            cx.state.selected_trainer = Some(ix.trainers[i].trainer.name.clone());
        }
    }
}
