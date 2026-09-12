//! Port of `gui_qt/components/filter_toggle_bar.py`: the row of icon
//! toggles above the route list, the "Clear Filters" button and the
//! debounced (300 ms) search box.

use std::time::{Duration, Instant};

use egui::{Color32, CornerRadius, Sense, Stroke, Ui, Vec2};

use xpr_core::consts;
use xpr_core::Config;
use xpr_ui_kit::theme::Theme;
use xpr_ui_kit::widgets::{self, Entry};
use xpr_ui_kit::ShortcutMap;

use crate::assets::Assets;
use crate::controller::MainController;

const TOGGLE_ORDER: [&str; 17] = [
    consts::MAJOR_BATTLE_FILTER,
    consts::TASK_TRAINER_BATTLE,
    consts::TASK_RARE_CANDY,
    consts::TASK_VITAMIN,
    consts::TASK_LEARN_MOVE_LEVELUP,
    consts::TASK_LEARN_MOVE_TM,
    consts::TASK_FIGHT_WILD_PKMN,
    consts::TASK_HOLD_ITEM,
    consts::TASK_GET_FREE_ITEM,
    consts::TASK_PURCHASE_ITEM,
    consts::TASK_SELL_ITEM,
    consts::TASK_USE_ITEM,
    consts::TASK_BLACKOUT,
    consts::TASK_HEAL,
    consts::TASK_EVOLUTION,
    consts::TASK_NOTES_ONLY,
    consts::ERROR_SEARCH,
];

fn short_label(et: &str) -> &'static str {
    match et {
        consts::MAJOR_BATTLE_FILTER => "MB",
        consts::TASK_TRAINER_BATTLE => "Tr",
        consts::TASK_LEARN_MOVE_LEVELUP => "Lv",
        consts::TASK_SELL_ITEM => "Se",
        consts::TASK_NOTES_ONLY => "No",
        consts::TASK_HOLD_ITEM => "Hd",
        consts::TASK_RARE_CANDY => "RC",
        consts::TASK_FIGHT_WILD_PKMN => "Wi",
        consts::TASK_GET_FREE_ITEM => "Gt",
        consts::TASK_PURCHASE_ITEM => "Bu",
        consts::TASK_USE_ITEM => "Us",
        consts::TASK_VITAMIN => "Vt",
        consts::TASK_SAVE => "Sv",
        consts::TASK_HEAL => "He",
        consts::TASK_BLACKOUT => "BO",
        consts::TASK_EVOLUTION => "Ev",
        consts::TASK_LEARN_MOVE_TM => "TM",
        consts::ERROR_SEARCH => "Er",
        _ => "??",
    }
}

fn icon_file(et: &str) -> Option<&'static str> {
    Some(match et {
        consts::TASK_TRAINER_BATTLE => "TASK_TRAINER_BATTLE",
        consts::TASK_LEARN_MOVE_LEVELUP => "TASK_LEARN_MOVE_LEVELUP",
        consts::TASK_SELL_ITEM => "TASK_SELL_ITEM",
        consts::TASK_NOTES_ONLY => "TASK_NOTES_ONLY",
        consts::TASK_HOLD_ITEM => "TASK_HOLD_ITEM",
        consts::TASK_RARE_CANDY => "TASK_RARE_CANDY",
        consts::TASK_FIGHT_WILD_PKMN => "TASK_FIGHT_WILD_PKMN",
        consts::TASK_GET_FREE_ITEM => "TASK_GET_FREE_ITEM",
        consts::TASK_PURCHASE_ITEM => "TASK_PURCHASE_ITEM",
        consts::TASK_USE_ITEM => "TASK_USE_ITEM",
        consts::TASK_VITAMIN => "TASK_VITAMIN",
        consts::TASK_HEAL => "TASK_HEAL",
        consts::TASK_BLACKOUT => "TASK_BLACKOUT",
        consts::TASK_EVOLUTION => "TASK_EVOLUTION",
        consts::TASK_LEARN_MOVE_TM => "TASK_LEARN_MOVE_TM",
        consts::ERROR_SEARCH => "ERROR_SEARCH",
        _ => return None,
    })
}

fn tooltip(et: &str) -> &'static str {
    match et {
        consts::MAJOR_BATTLE_FILTER => "Major Battles",
        consts::TASK_TRAINER_BATTLE => "Trainer Battle",
        consts::TASK_LEARN_MOVE_LEVELUP => "Level Up Move",
        consts::TASK_SELL_ITEM => "Sell Item",
        consts::TASK_NOTES_ONLY => "Notes Only",
        consts::TASK_HOLD_ITEM => "Hold Item",
        consts::TASK_RARE_CANDY => "Rare Candy",
        consts::TASK_FIGHT_WILD_PKMN => "Wild Pkmn",
        consts::TASK_GET_FREE_ITEM => "Get Free Item",
        consts::TASK_PURCHASE_ITEM => "Purchase Item",
        consts::TASK_USE_ITEM => "Use / Drop Item",
        consts::TASK_VITAMIN => "Vitamin",
        consts::TASK_SAVE => "Save",
        consts::TASK_HEAL => "Heal",
        consts::TASK_BLACKOUT => "Blackout",
        consts::TASK_EVOLUTION => "Evolution",
        consts::TASK_LEARN_MOVE_TM => "TM / HM",
        consts::ERROR_SEARCH => "Invalid Events",
        _ => "",
    }
}

/// event type -> shortcut action id (`_FILTER_SHORTCUT_IDS`)
pub fn shortcut_id(et: &str) -> Option<&'static str> {
    Some(match et {
        consts::TASK_TRAINER_BATTLE => "filter_trainer",
        consts::TASK_RARE_CANDY => "filter_rare_candy",
        consts::TASK_LEARN_MOVE_TM => "filter_tm_hm",
        consts::TASK_VITAMIN => "filter_vitamin",
        consts::TASK_FIGHT_WILD_PKMN => "filter_wild_pkmn",
        consts::TASK_GET_FREE_ITEM => "filter_acquire_item",
        consts::TASK_PURCHASE_ITEM => "filter_purchase_item",
        consts::TASK_USE_ITEM => "filter_use_item",
        consts::TASK_SELL_ITEM => "filter_sell_item",
        consts::TASK_HOLD_ITEM => "filter_hold_item",
        consts::TASK_LEARN_MOVE_LEVELUP => "filter_levelup_move",
        consts::TASK_SAVE => "filter_save",
        consts::TASK_HEAL => "filter_heal",
        consts::TASK_BLACKOUT => "filter_blackout",
        consts::TASK_EVOLUTION => "filter_evolution",
        consts::TASK_NOTES_ONLY => "filter_notes",
        _ => return None,
    })
}

pub struct FilterBar {
    pub search_text: String,
    search_deadline: Option<Instant>,
}

impl Default for FilterBar {
    fn default() -> Self {
        FilterBar::new()
    }
}

impl FilterBar {
    pub fn new() -> FilterBar {
        FilterBar { search_text: String::new(), search_deadline: None }
    }

    /// `_toggle_filter_type` (also used by the shortcuts).
    pub fn toggle_filter_type(ctrl: &mut MainController, event_type: &str) {
        let mut current: Vec<String> = ctrl.get_route_filter_types().map(|f| f.to_vec()).unwrap_or_default();
        if let Some(pos) = current.iter().position(|t| t == event_type) {
            current.remove(pos);
        } else {
            current.push(event_type.to_string());
        }
        ctrl.set_route_filter_types(current);
    }

    pub fn tick(&mut self, ctx: &egui::Context, ctrl: &mut MainController) {
        if let Some(d) = self.search_deadline {
            let now = Instant::now();
            if now >= d {
                self.search_deadline = None;
                ctrl.set_route_search(&self.search_text);
            } else {
                ctx.request_repaint_after(d - now);
            }
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, theme: &Theme, _cfg: &Config, ctrl: &mut MainController, assets: &mut Assets, shortcuts: &ShortcutMap) {
        let active: Vec<String> = ctrl.get_route_filter_types().map(|f| f.to_vec()).unwrap_or_default();
        egui::Frame::new().inner_margin(egui::Margin { left: 4, right: 4, top: 2, bottom: 2 }).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for et in TOGGLE_ORDER {
                    let checked = active.iter().any(|a| a == et);
                    let mut tip = tooltip(et).to_string();
                    if let Some(aid) = shortcut_id(et) {
                        let key = shortcuts.label(aid);
                        if !key.is_empty() {
                            tip = format!("{} ({})", tip, key);
                        }
                    }
                    let icon = icon_file(et).and_then(|f| assets.filter_icon(ui.ctx(), f));
                    let size = Vec2::new(26.0, 26.0);
                    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                    let hovered = resp.hovered();
                    let (fill, stroke) = if checked {
                        (Color32::from_rgb(0x3a, 0x7b, 0xd5), Stroke::new(1.0_f32, theme.accent))
                    } else if hovered {
                        (theme.hover_bg, Stroke::new(1.0_f32, theme.accent))
                    } else {
                        (theme.bg_lighter, Stroke::new(1.0_f32, theme.border))
                    };
                    ui.painter().rect(rect, CornerRadius::same(3), fill, stroke, egui::StrokeKind::Inside);
                    match icon {
                        Some(tex) => {
                            let r = egui::Rect::from_center_size(rect.center(), Vec2::splat(20.0));
                            ui.painter().image(tex.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                        }
                        None => {
                            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, short_label(et), theme.body(), if checked { Color32::WHITE } else { theme.text });
                        }
                    }
                    let resp = resp.on_hover_text(tip);
                    if resp.clicked() {
                        FilterBar::toggle_filter_type(ctrl, et);
                    }
                }
                if widgets::StyledButton::new(theme, egui::RichText::new("Clear Filters").font(theme.font(8.25))).padding(Vec2::new(4.0, 2.0)).show(ui).clicked() {
                    ctrl.set_route_filter_types(Vec::new());
                }
                let r = Entry::new(theme, &mut self.search_text).width(200.0).hint("Search events...").id(ui.id().with("route_search")).show(ui);
                if r.changed {
                    self.search_deadline = Some(Instant::now() + Duration::from_millis(300));
                }
            });
        });
    }
}
