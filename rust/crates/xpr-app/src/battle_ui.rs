//! Port of `gui_qt/battle_summary/battle_summary.py`: the controls bar
//! (candies / vitamins / held item / HP / Spe), the collapsible legacy
//! controls, and the six matchup cards with their damage columns.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use egui::{Align2, Color32, CornerRadius, Id, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use xpr_calc::battle_summary::{BattleSummary, MoveRenderInfo};
use xpr_core::consts;
use xpr_core::Config;
use xpr_ui_kit::theme::{self, Theme};
use xpr_ui_kit::widgets::{self, SearchableDropdown, StyledButton};

use crate::assets::Assets;
use crate::battle::BattleController;
use crate::controller::MainController;
use crate::editors::OptionMenu;

/// Stepper clicks closer together than this form one undo step (the Qt
/// widget coalesced them behind a 250 ms debounce before mutating the route
/// at all; here the route is mutated on every click, which is cheap enough
/// to show the new damage ranges on the next frame).
const CANDY_BURST_MS: u64 = 250;

const HIGHLIGHT_COLORS: [(i64, &str); 3] = [(1, "#006400"), (2, "#00008B"), (3, "#FF8C00")];
const HIGHLIGHT_COLORS_IMMEDIATE: [(i64, &str); 3] = [(1, "#165416"), (2, "#212168"), (3, "#69400f")];

const STAT_DISPLAY_ORDER: [(&str, &str); 6] = [
    (consts::HP, "HP"),
    (consts::ATK, "Atk"),
    (consts::DEF, "Def"),
    (consts::SPA, "SpA"),
    (consts::SPD, "SpD"),
    (consts::SPE, "Spe"),
];

/// Requests the battle summary hands back to the event-details panel.
#[derive(Clone, Debug, Default)]
pub struct BattleUiActions {
    pub open_config: bool,
    pub assign_move_slot: Option<i64>,
    pub export_matchup: Option<usize>,
    pub reorder: Option<(usize, usize)>,
}

/// Where the matchup cards were drawn (for screenshot cropping).
#[derive(Clone, Debug, Default)]
pub struct Geometry {
    pub base_rect: Option<Rect>,
    pub mon_pair_rects: Vec<Rect>,
    /// (left edge, right edge) of the first visible divider
    pub divider_x: Option<(f32, f32)>,
}

#[derive(Clone, Debug, Default)]
struct SetupMovesState {
    move_list: Vec<String>,
    selector: OptionMenu,
}

pub struct BattleSummaryUi {
    pub should_render: bool,
    legacy_expanded: bool,

    candy_displayed_count: i64,
    /// When the last stepper click was applied: clicks that follow within
    /// [`CANDY_BURST_MS`] join it as one undo step.
    candy_last_applied: Option<Instant>,

    vitamin_display: HashMap<&'static str, i64>,
    vitamin_pending: HashMap<&'static str, i64>,
    vitamin_deadline: Option<Instant>,

    held_item_options: Vec<String>,
    held_item_text: String,
    held_item_apply_in_flight: bool,

    player_setup: SetupMovesState,
    enemy_setup: SetupMovesState,
    weather: OptionMenu,
    legacy_candy: String,
    legacy_candy_deadline: Option<Instant>,
    transform: bool,

    expanded: [bool; 6],
    drag_src: Option<usize>,
    drop_indicator: Option<(usize, bool)>,

    /// screenshot mode: hide defaults / swap icons while rendering
    pub screenshot_mode: Option<ScreenshotMode>,
    pub geometry: Geometry,
    test_move_typed: [String; 4],
    all_moves_cache: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScreenshotMode {
    Full,
    Player,
    Enemy,
}

impl Default for BattleSummaryUi {
    fn default() -> Self {
        BattleSummaryUi::new()
    }
}

fn hex(s: &str) -> Color32 {
    theme::parse_hex(s)
}

impl BattleSummaryUi {
    pub fn new() -> BattleSummaryUi {
        BattleSummaryUi {
            should_render: false,
            legacy_expanded: false,
            candy_displayed_count: 0,
            candy_last_applied: None,
            vitamin_display: HashMap::new(),
            vitamin_pending: HashMap::new(),
            vitamin_deadline: None,
            held_item_options: Vec::new(),
            held_item_text: String::new(),
            held_item_apply_in_flight: false,
            player_setup: SetupMovesState::default(),
            enemy_setup: SetupMovesState::default(),
            weather: OptionMenu::new(vec![consts::WEATHER_NONE.to_string()], None),
            legacy_candy: "0".to_string(),
            legacy_candy_deadline: None,
            transform: false,
            expanded: [true; 6],
            drag_src: None,
            drop_indicator: None,
            screenshot_mode: None,
            geometry: Geometry::default(),
            test_move_typed: Default::default(),
            all_moves_cache: Vec::new(),
        }
    }

    /// `configure_weather` / `configure_setup_moves` / `refresh_test_move_options`
    pub fn configure_for_version(&mut self, ctrl: &MainController) {
        if let Some(gen) = ctrl.gen() {
            self.weather.new_values(gen.get_valid_weather().iter().map(|s| s.to_string()).collect(), None);
            let setup = gen.get_stat_modifer_moves();
            self.player_setup.selector.new_values(setup.clone(), None);
            self.enemy_setup.selector.new_values(setup, None);
            let mut all = gen.move_db().get_filtered_names(None, false);
            all.insert(0, String::new());
            self.all_moves_cache = all;
        }
    }

    pub fn hide_contents(&mut self) {
        self.should_render = false;
    }

    pub fn show_contents(&mut self) {
        self.should_render = true;
    }

    pub fn set_legacy_expanded(&mut self, v: bool) {
        self.legacy_expanded = v;
    }

    // ---- candy stepper (immediate) / debounced vitamins -----------------------------

    /// `_on_candy_adjust(delta)`: apply the click to the route right away.
    pub fn on_candy_adjust(&mut self, cfg: &Config, bc: &mut BattleController, ctrl: &mut MainController, delta: i64) {
        let current = bc.get_prefight_candy_count(ctrl);
        let new_count = (current + delta).max(0);
        if new_count == current {
            self.candy_displayed_count = current;
            return;
        }
        let now = Instant::now();
        let coalesce = self.candy_last_applied.map(|t| now.duration_since(t) < Duration::from_millis(CANDY_BURST_MS)).unwrap_or(false);
        bc.update_prefight_candies(cfg, ctrl, new_count, coalesce);
        self.candy_last_applied = Some(now);
        self.candy_displayed_count = bc.get_prefight_candy_count(ctrl);
        self.legacy_candy = self.candy_displayed_count.to_string();
    }

    pub fn increment_prefight_candies(&mut self, cfg: &Config, bc: &mut BattleController, ctrl: &mut MainController) {
        if bc.can_support_prefight_candies() {
            self.on_candy_adjust(cfg, bc, ctrl, 1);
        }
    }

    pub fn decrement_prefight_candies(&mut self, cfg: &Config, bc: &mut BattleController, ctrl: &mut MainController) {
        if bc.can_support_prefight_candies() && self.candy_displayed_count > 0 {
            self.on_candy_adjust(cfg, bc, ctrl, -1);
        }
    }

    fn on_vitamin_adjust(&mut self, stat: &'static str, delta: i64) {
        *self.vitamin_pending.entry(stat).or_insert(0) += delta;
        let cur = self.vitamin_display.get(stat).copied().unwrap_or(0);
        self.vitamin_display.insert(stat, (cur + delta).max(0));
        self.vitamin_deadline = Some(Instant::now() + Duration::from_millis(300));
    }

    /// Flush expired debounce timers. Call once per frame before drawing.
    pub fn tick(&mut self, ctx: &egui::Context, bc: &mut BattleController, ctrl: &mut MainController, cfg: &Config) {
        let now = Instant::now();
        if let Some(d) = self.vitamin_deadline {
            if now >= d {
                self.vitamin_deadline = None;
                let pending: Vec<(&'static str, i64)> = self.vitamin_pending.drain().filter(|(_, d)| *d != 0).collect();
                if !pending.is_empty() {
                    for (stat, delta) in pending {
                        bc.adjust_vitamin_for_stat(cfg, ctrl, stat, delta, true);
                    }
                    bc.full_refresh(cfg, ctrl);
                }
            } else {
                ctx.request_repaint_after(d - now);
            }
        }
        if let Some(d) = self.legacy_candy_deadline {
            if now >= d {
                self.legacy_candy_deadline = None;
                let n: i64 = self.legacy_candy.trim().parse().unwrap_or(0);
                bc.update_prefight_candies(cfg, ctrl, n, false);
            } else {
                ctx.request_repaint_after(d - now);
            }
        }
    }

    /// `_on_full_refresh`: pull display values from the controller.
    pub fn sync_from_controller(&mut self, bc: &BattleController, ctrl: &MainController) {
        let cur_candies = bc.get_prefight_candy_count(ctrl);
        self.candy_displayed_count = cur_candies;
        self.legacy_candy = cur_candies.to_string();
        let counts = bc.get_vitamins_used_per_stat(ctrl);
        for (stat, _) in STAT_DISPLAY_ORDER {
            let pending = self.vitamin_pending.get(stat).copied().unwrap_or(0);
            let c = counts.get(stat).copied().unwrap_or(0);
            // while a debounced adjustment is in flight keep the user's value
            self.vitamin_display.insert(stat, if pending != 0 { self.vitamin_display.get(stat).copied().unwrap_or(c) } else { c });
        }
        self.transform = bc.summary.is_player_transformed;
        let options = bc.get_held_item_options(ctrl);
        if options != self.held_item_options {
            self.held_item_options = options;
        }
        self.held_item_text = bc.get_player_held_item();
        self.weather.set(bc.summary.get_weather());
        self.player_setup.move_list = bc.summary.player_setup_move_list.clone();
        self.enemy_setup.move_list = bc.summary.enemy_setup_move_list.clone();
        for idx in 0..6 {
            self.expanded[idx] = !bc.summary.is_mon_collapsed(idx as i64);
        }
    }

    // ---- drawing --------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        theme: &Theme,
        cfg: &mut Config,
        bc: &mut BattleController,
        ctrl: &mut MainController,
        assets: &mut Assets,
        actions: &mut BattleUiActions,
    ) {
        let base_frame = egui::Frame::new().inner_margin(egui::Margin::same(2));
        let mut mon_pair_rects: Vec<Rect> = Vec::new();
        let mut divider_x: Option<(f32, f32)> = None;
        let inner = base_frame.show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 6.0);
            // Capture the panel width before anything above the matchups is
            // drawn: if the controls bar overflows (narrow window), egui grows
            // this ui's max_rect to include the overflow and every later
            // `available_width()` would push the matchup cards off-screen.
            let panel_w = ui.available_width();
            if self.screenshot_mode.is_none() {
                self.controls_bar(ui, theme, cfg, bc, ctrl, assets, actions);
                if cfg.get_show_legacy_controls() {
                    self.legacy_section(ui, theme, cfg, bc, ctrl, actions);
                }
            }
            for idx in 0..6 {
                let has = bc.get_pkmn_info(idx, true).is_some() || bc.get_pkmn_info(idx, false).is_some();
                if !has {
                    continue;
                }
                let (rect, div) = ui
                    .allocate_ui_with_layout(Vec2::new(panel_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.set_width(panel_w);
                        self.mon_pair(ui, theme, cfg, bc, ctrl, assets, idx, actions)
                    })
                    .inner;
                mon_pair_rects.push(rect);
                if divider_x.is_none() {
                    divider_x = div;
                }
            }
        });
        self.geometry = Geometry { base_rect: Some(inner.response.rect), mon_pair_rects, divider_x };
        // drag & drop of matchups: release handling
        if self.drag_src.is_some() && ui.input(|i| i.pointer.any_released()) {
            if let (Some(src), Some((target, top))) = (self.drag_src, self.drop_indicator) {
                if src != target {
                    let insertion = if top { target } else { target + 1 };
                    let to_idx = if insertion <= src { insertion } else { insertion - 1 };
                    if to_idx != src {
                        actions.reorder = Some((src, to_idx));
                    }
                }
            }
            self.drag_src = None;
            self.drop_indicator = None;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn controls_bar(&mut self, ui: &mut Ui, theme: &Theme, cfg: &Config, bc: &mut BattleController, ctrl: &mut MainController, assets: &mut Assets, _actions: &mut BattleUiActions) {
        let can_candies = bc.can_support_prefight_candies();
        // Horizontal scroll so a narrow window can't widen the whole panel.
        widgets::show_scroll(ui, egui::ScrollArea::horizontal().id_salt("battle_controls_scroll").auto_shrink([false, true]), |ui| {
        egui::Frame::new().inner_margin(egui::Margin { left: 6, right: 6, top: 2, bottom: 2 }).show(ui, |ui| {
            // Fixed-height row so every group is centered on the same
            // baseline (see `widgets::stepper_group`).
            let row_size = Vec2::new(ui.available_width(), widgets::STEPPER_H);
            ui.allocate_ui_with_layout(row_size, egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                // candy stepper
                let minus_ok = can_candies && self.candy_displayed_count > 0;
                let count = self.candy_displayed_count;
                let candy_icon = assets.candy_icon(ui.ctx());
                let delta = widgets::stepper_group(ui, theme, ui.id().with("candy"), 32.0, minus_ok, can_candies, ("Decrement Pre-Fight Candies", "Increment Pre-Fight Candies"), |ui| {
                    ui.add_space(8.0);
                    if let Some(tex) = &candy_icon {
                        crate::assets::draw_fit(ui, tex, 20.0).on_hover_text("Pre-Fight Rare Candies");
                    }
                    ui.add_space(4.0);
                    let (r, _) = ui.allocate_exact_size(Vec2::new(widgets::text_width(ui, &count.to_string(), &theme.font_bold(10.0)).max(20.0) + 12.0, widgets::STEPPER_H), Sense::hover());
                    ui.painter().text(Pos2::new(r.max.x - 8.0, r.center().y), Align2::RIGHT_CENTER, count.to_string(), theme.font_bold(10.0), Color32::WHITE);
                });
                if delta != 0 {
                    self.on_candy_adjust(cfg, bc, ctrl, delta as i64);
                }
                // vitamins per stat
                for (stat, stat_label) in STAT_DISPLAY_ORDER {
                    let count = self.vitamin_display.get(stat).copied().unwrap_or(0);
                    let tips = (format!("Remove a vitamin for {}", stat_label), format!("Add a vitamin for {}", stat_label));
                    let delta = widgets::stepper_group(ui, theme, ui.id().with(("vit", stat)), 30.0, can_candies, can_candies, (&tips.0, &tips.1), |ui| {
                        let label_w = widgets::text_width(ui, stat_label, &theme.body());
                        let count_s = count.to_string();
                        let count_w = widgets::text_width(ui, &count_s, &theme.body_bold());
                        let w = (label_w + 8.0 + count_w + 16.0).max(46.0);
                        let (r, resp) = ui.allocate_exact_size(Vec2::new(w, widgets::STEPPER_H), Sense::hover());
                        resp.on_hover_text(format!("Vitamins boosting {} used before this battle", stat_label));
                        ui.painter().text(Pos2::new(r.min.x + 8.0, r.center().y), Align2::LEFT_CENTER, stat_label, theme.body(), Color32::from_rgb(0x88, 0x88, 0x88));
                        ui.painter().text(Pos2::new(r.max.x - 8.0, r.center().y), Align2::RIGHT_CENTER, count_s, theme.body_bold(), Color32::WHITE);
                    });
                    if delta != 0 {
                        self.on_vitamin_adjust(stat, delta as i64);
                    }
                }
                ui.add_space(10.0);
                widgets::label(ui, theme, "Held:");
                let resp = SearchableDropdown::new(theme, ui.id().with("held_item"), &mut self.held_item_text, &self.held_item_options)
                    .widths(140.0, 200.0)
                    .enabled(can_candies)
                    .show(ui);
                let _ = resp.rect;
                if (resp.changed || resp.enter_pressed) && !self.held_item_apply_in_flight {
                    self.held_item_apply_in_flight = true;
                    let new_text = self.held_item_text.trim().to_string();
                    if new_text.is_empty() || self.held_item_options.iter().any(|o| *o == new_text) {
                        if bc.get_player_held_item() != new_text {
                            bc.update_player_held_item(cfg, ctrl, &new_text);
                        }
                    } else {
                        self.held_item_text = bc.get_player_held_item();
                    }
                    self.held_item_apply_in_flight = false;
                }
                let chip = |ui: &mut Ui, text: String, tip: &str| {
                    let galley = ui.fonts_mut(|f| f.layout_no_wrap(text, theme.body(), Color32::WHITE));
                    let (r, resp) = ui.allocate_exact_size(galley.size() + Vec2::new(8.0, 4.0), Sense::hover());
                    ui.painter().rect_stroke(r, CornerRadius::same(3), Stroke::new(1.0_f32, theme::rgba(255, 255, 255, 0.15)), egui::StrokeKind::Inside);
                    ui.painter().galley(Pos2::new(r.min.x + 4.0, r.center().y - galley.size().y / 2.0), galley, theme.text);
                    resp.on_hover_text(tip);
                };
                chip(ui, format!("HP {}", bc.summary.get_player_battle_hp()), "Player HP entering this battle");
                chip(ui, format!("Spe {}", bc.summary.get_player_battle_speed()), "Player Speed entering this battle");
            });
        });
        });
    }

    fn legacy_section(&mut self, ui: &mut Ui, theme: &Theme, cfg: &Config, bc: &mut BattleController, ctrl: &mut MainController, actions: &mut BattleUiActions) {
        egui::Frame::new().inner_margin(egui::Margin { left: 6, right: 6, top: 3, bottom: 3 }).show(ui, |ui| {
            ui.horizontal(|ui| {
                let tri = widgets::disclosure_triangle(ui, self.legacy_expanded, 14.0, Color32::from_rgb(0xcc, 0xcc, 0xcc));
                let lbl = ui.add(egui::Label::new(egui::RichText::new("Legacy Controls").font(theme.body_bold()).color(theme.text)).sense(Sense::click()));
                if tri.clicked() || lbl.clicked() {
                    self.legacy_expanded = !self.legacy_expanded;
                }
            });
        });
        if !self.legacy_expanded {
            return;
        }
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                // player setup row
                ui.horizontal(|ui| {
                    if let Some(moves) = setup_moves_row(ui, theme, &mut self.player_setup, true) {
                        bc.update_player_setup_moves(cfg, ctrl, moves);
                    }
                    if widgets::checkbox_label(ui, theme, &mut self.transform, "Transform:", true, true) {
                        bc.update_player_transform(cfg, ctrl, self.transform);
                    }
                    let held = bc.get_player_held_item();
                    if !held.is_empty() {
                        widgets::label_colored(ui, theme, format!("Held: {}", held), Color32::from_rgb(0xaa, 0xaa, 0xaa));
                    }
                });
                ui.horizontal(|ui| {
                    if let Some(moves) = setup_moves_row(ui, theme, &mut self.enemy_setup, false) {
                        bc.update_enemy_setup_moves(cfg, ctrl, moves);
                    }
                });
            });
            egui::Grid::new(ui.id().with("legacy_right")).spacing(Vec2::new(2.0, 2.0)).show(ui, |ui| {
                if widgets::button(ui, theme, "Configure/Help").clicked() {
                    actions.open_config = true;
                }
                ui.horizontal(|ui| {
                    widgets::label(ui, theme, "Weather:");
                    if self.weather.ui(ui, theme, ui.id().with("weather"), None, true) {
                        let w = self.weather.get().to_string();
                        bc.update_weather(cfg, ctrl, &w, None);
                    }
                });
                ui.end_row();
                widgets::label(ui, theme, if bc.summary.is_double_battle() { "Double Battle" } else { "Single Battle" });
                ui.horizontal(|ui| {
                    widgets::label(ui, theme, "Prefight Candies:");
                    let can = bc.can_support_prefight_candies();
                    let r = widgets::AmountEntry::new(theme, ui.id().with("legacy_candy"), &mut self.legacy_candy).min(Some(0)).width(Some(5)).enabled(can).show(ui);
                    if r.changed {
                        self.legacy_candy_deadline = Some(Instant::now() + Duration::from_millis(150));
                    }
                });
                ui.end_row();
            });
        });
    }

    /// One `MonPairSummary`. Returns the card rect and the divider x-range.
    #[allow(clippy::too_many_arguments)]
    fn mon_pair(
        &mut self,
        ui: &mut Ui,
        theme: &Theme,
        cfg: &Config,
        bc: &mut BattleController,
        ctrl: &mut MainController,
        assets: &mut Assets,
        mon_idx: usize,
        actions: &mut BattleUiActions,
    ) -> (Rect, Option<(f32, f32)>) {
        let matchup_bg = theme::darken(theme.bg, 0.35);
        let test_moves_enabled = cfg.get_test_moves_enabled();
        let visible_count = (0..6).filter(|i| bc.get_pkmn_info(*i, true).is_some() || bc.get_pkmn_info(*i, false).is_some()).count();
        let drag_enabled = visible_count >= 2 && bc.can_support_prefight_candies() && self.screenshot_mode.is_none();
        let mut divider: Option<(f32, f32)> = None;
        let player_info = bc.get_pkmn_info(mon_idx, true).cloned();
        let enemy_info = bc.get_pkmn_info(mon_idx, false).cloned();
        let frame = egui::Frame::new().fill(matchup_bg).corner_radius(CornerRadius::same(6)).inner_margin(egui::Margin::same(2));
        let inner = frame.show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            // ---- header ----
            let header_h = 30.0;
            let full_w = ui.available_width();
            let (header_rect, header_resp) = ui.allocate_exact_size(Vec2::new(full_w, header_h), Sense::click());
            let col_w = (full_w - 12.0 - 12.0) / 8.0;
            let left_center_x = header_rect.min.x + 6.0 + col_w * 2.0;
            let right_center_x = header_rect.min.x + 6.0 + col_w * 4.0 + 12.0 + col_w * 2.0;
            let text_color = match (&player_info, &enemy_info) {
                (Some(p), Some(_)) => {
                    if p.attacking_mon_speed > p.defending_mon_speed {
                        Color32::from_rgb(0x34, 0x98, 0xdb)
                    } else if p.attacking_mon_speed == p.defending_mon_speed {
                        Color32::from_rgb(0xf1, 0xc4, 0x0f)
                    } else {
                        Color32::from_rgb(0xe7, 0x4c, 0x3c)
                    }
                }
                _ => theme.text,
            };
            let mut header_click_consumed = false;
            if let (Some(p), Some(e)) = (&player_info, &enemy_info) {
                let player_text = format!("{} Lv{} Damage Ranges", p.attacking_mon_name, p.attacking_mon_level);
                let enemy_text = format!("{} Lv{} Damage Ranges", e.attacking_mon_name, e.attacking_mon_level);
                let bold = theme.body_bold();
                let player_icon = assets.pkmn_icon(ui.ctx(), &p.attacking_mon_name);
                let enemy_icon = assets.pkmn_icon(ui.ctx(), &e.attacking_mon_name);
                // icon layout: viewing shows the enemy icon in the corner; exports show icons next to headers
                let (show_player_icon, player_icon_tex, show_enemy_icon) = match self.screenshot_mode {
                    None => (false, None, false),
                    Some(ScreenshotMode::Full) => (player_icon.is_some(), player_icon.clone(), enemy_icon.is_some()),
                    Some(ScreenshotMode::Player) => (enemy_icon.is_some(), enemy_icon.clone(), false),
                    Some(ScreenshotMode::Enemy) => (false, None, enemy_icon.is_some()),
                };
                let draw_titled = |ui: &mut Ui, center_x: f32, text: &str, icon: Option<&egui::TextureHandle>| {
                    let galley = ui.fonts_mut(|f| f.layout_no_wrap(text.to_string(), bold.clone(), text_color));
                    let icon_w = if icon.is_some() { 32.0 } else { 0.0 };
                    let total = galley.size().x + icon_w;
                    let mut x = center_x - total / 2.0;
                    if let Some(tex) = icon {
                        let r = Rect::from_center_size(Pos2::new(x + 14.0, header_rect.center().y), Vec2::splat(28.0));
                        ui.painter().image(tex.id(), r, Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)), Color32::WHITE);
                        x += icon_w;
                    }
                    ui.painter().galley(Pos2::new(x, header_rect.center().y - galley.size().y / 2.0), galley, text_color);
                };
                draw_titled(ui, left_center_x, &player_text, if show_player_icon { player_icon_tex.as_ref() } else { None });
                draw_titled(ui, right_center_x, &enemy_text, if show_enemy_icon { enemy_icon.as_ref() } else { None });
                // corner: drag grip + chevron + enemy icon (viewing mode only)
                if self.screenshot_mode.is_none() {
                    let mut x = header_rect.min.x + 6.0;
                    if drag_enabled {
                        let grip_rect = Rect::from_min_size(Pos2::new(x, header_rect.center().y - 8.0), Vec2::new(12.0, 16.0));
                        let grip_resp = ui.interact(grip_rect, ui.id().with(("grip", mon_idx)), Sense::drag());
                        let grip_color = if grip_resp.hovered() || grip_resp.dragged() { Color32::from_rgb(0xdd, 0xdd, 0xdd) } else { Color32::from_rgb(0x88, 0x88, 0x88) };
                        for row in 0..3 {
                            for col in 0..2 {
                                let c = Pos2::new(grip_rect.min.x + 3.0 + col as f32 * 6.0, grip_rect.min.y + 3.0 + row as f32 * 5.0);
                                ui.painter().circle_filled(c, 1.5, grip_color);
                            }
                        }
                        grip_resp.clone().on_hover_cursor(egui::CursorIcon::Grab);
                        if grip_resp.drag_started() {
                            self.drag_src = Some(mon_idx);
                        }
                        x += 16.0;
                    }
                    let tri_rect = Rect::from_min_size(Pos2::new(x, header_rect.center().y - 7.0), Vec2::splat(14.0));
                    let tri_resp = ui.interact(tri_rect, ui.id().with(("tri", mon_idx)), Sense::click());
                    let margin = 14.0 * 0.15;
                    let tri = 14.0 - 2.0 * margin;
                    let o = tri_rect.min;
                    let pts = if self.expanded[mon_idx] {
                        vec![Pos2::new(o.x + margin, o.y + margin), Pos2::new(o.x + margin + tri, o.y + margin), Pos2::new(o.x + margin + tri / 2.0, o.y + margin + tri)]
                    } else {
                        vec![Pos2::new(o.x + margin, o.y + margin), Pos2::new(o.x + margin + tri, o.y + margin + tri / 2.0), Pos2::new(o.x + margin, o.y + margin + tri)]
                    };
                    ui.painter().add(egui::Shape::convex_polygon(pts, Color32::from_rgb(0xcc, 0xcc, 0xcc), Stroke::NONE));
                    if tri_resp.clicked() {
                        header_click_consumed = true;
                        self.toggle_expanded(bc, mon_idx);
                    }
                    x += 18.0;
                    if let Some(tex) = &enemy_icon {
                        let r = Rect::from_center_size(Pos2::new(x + 12.0, header_rect.center().y), Vec2::splat(24.0));
                        ui.painter().image(tex.id(), r, Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)), Color32::WHITE);
                    }
                }
                // intimidate toggles + export button (right-aligned in each half)
                let mut right_x = header_rect.min.x + 6.0 + col_w * 4.0 - 4.0;
                if bc.summary.pokemon_has_intimidate(mon_idx as i64, true) {
                    let active = bc.summary.is_intimidate_active(mon_idx as i64, true);
                    if self.screenshot_mode.is_none() || active {
                        if let Some(new_val) = intimidate_toggle(ui, theme, ui.id().with(("intim_p", mon_idx)), &mut right_x, header_rect, active, !bc.summary.is_intimidate_blocked(mon_idx as i64, true), self.screenshot_mode.is_some()) {
                            header_click_consumed = true;
                            bc.toggle_intimidate(cfg, ctrl, mon_idx as i64, true, new_val);
                        }
                    }
                }
                let mut right_x2 = header_rect.max.x - 6.0;
                if self.screenshot_mode.is_none() {
                    let export_rect = Rect::from_min_size(Pos2::new(right_x2 - 54.0, header_rect.center().y - 11.0), Vec2::new(54.0, 22.0));
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(export_rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
                    if StyledButton::new(theme, "Export").min_size(Vec2::new(54.0, 22.0)).show(&mut child).on_hover_text("Export this matchup as an image").clicked() {
                        header_click_consumed = true;
                        actions.export_matchup = Some(mon_idx);
                    }
                    right_x2 -= 58.0;
                }
                if bc.summary.pokemon_has_intimidate(mon_idx as i64, false) {
                    let active = bc.summary.is_intimidate_active(mon_idx as i64, false);
                    if self.screenshot_mode.is_none() || active {
                        if let Some(new_val) = intimidate_toggle(ui, theme, ui.id().with(("intim_e", mon_idx)), &mut right_x2, header_rect, active, !bc.summary.is_intimidate_blocked(mon_idx as i64, false), self.screenshot_mode.is_some()) {
                            header_click_consumed = true;
                            bc.toggle_intimidate(cfg, ctrl, mon_idx as i64, false, new_val);
                        }
                    }
                }
            }
            if header_resp.clicked() && !header_click_consumed && self.screenshot_mode.is_none() {
                self.toggle_expanded(bc, mon_idx);
            }
            if header_resp.hovered() && self.screenshot_mode.is_none() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            // ---- content ----
            if self.expanded[mon_idx] {
                let content_w = ui.available_width();
                let col_w = (content_w - 2.0 - 12.0) / 8.0;
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.add_space(1.0);
                    let mut max_h: f32 = 0.0;
                    let mut col_rects: Vec<Rect> = Vec::new();
                    // Resolve every column first so the row shares one kill-frame
                    // height (Qt's grid stretched each cell to the tallest; a
                    // recoil line must grow the whole row, not just its column).
                    let mut columns: Vec<Option<(usize, bool, bool, Option<MoveRenderInfo>)>> = Vec::with_capacity(8);
                    for cur_idx in 0..8usize {
                        let is_player = cur_idx < 4;
                        let move_idx = cur_idx % 4;
                        // Test move slots take the enemy columns when enabled.
                        if cur_idx >= 4 && test_moves_enabled {
                            let slot_idx = cur_idx - 4;
                            let tmv = bc.get_move_info(cfg, ctrl, mon_idx, 4 + slot_idx, true);
                            columns.push(Some((4 + slot_idx, true, true, tmv)));
                            continue;
                        }
                        let mv = bc.get_move_info(cfg, ctrl, mon_idx, move_idx, is_player);
                        columns.push(mv.map(|m| (move_idx, is_player, false, Some(m))));
                    }
                    let kill_h = columns
                        .iter()
                        .flatten()
                        .map(|(_, _, _, mv)| kill_frame_height(theme, mv.as_ref()))
                        .fold(0.0_f32, f32::max);
                    for (cur_idx, col) in columns.into_iter().enumerate() {
                        if cur_idx == 4 {
                            // divider column
                            let (drect, _) = ui.allocate_exact_size(Vec2::new(12.0, 1.0), Sense::hover());
                            divider = Some((drect.center().x - 1.0, drect.center().x + 1.0));
                            col_rects.push(drect);
                        }
                        match col {
                            Some((move_idx, is_player, is_test_move, mv)) => {
                                let r = self.damage_summary(ui, theme, cfg, bc, ctrl, mon_idx, move_idx, is_player, is_test_move, mv.as_ref(), col_w, kill_h, actions);
                                max_h = max_h.max(r.height());
                                col_rects.push(r);
                            }
                            None => {
                                let (r, _) = ui.allocate_exact_size(Vec2::new(col_w, 1.0), Sense::hover());
                                col_rects.push(r);
                            }
                        }
                    }
                    // paint the divider now that the row height is known
                    if let Some((l, r)) = divider {
                        let top = col_rects.first().map(|r| r.min.y).unwrap_or(0.0);
                        let div_rect = Rect::from_min_max(Pos2::new(l, top), Pos2::new(r, top + max_h));
                        ui.painter().rect_filled(div_rect, CornerRadius::ZERO, theme::darken(theme.divider, 0.3));
                    }
                });
            }
        });
        let rect = inner.response.rect;
        // drop indicator during a reorder drag
        if let Some(src) = self.drag_src {
            if src != mon_idx {
                if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                    if rect.contains(pos) {
                        let top = pos.y < rect.center().y;
                        self.drop_indicator = Some((mon_idx, top));
                        let y = if top { rect.min.y } else { rect.max.y - 3.0 };
                        ui.painter().rect_filled(Rect::from_min_size(Pos2::new(rect.min.x, y), Vec2::new(rect.width(), 3.0)), CornerRadius::ZERO, Color32::from_rgb(0x6c, 0xb1, 0xff));
                    } else if self.drop_indicator.map(|d| d.0 == mon_idx).unwrap_or(false) {
                        self.drop_indicator = None;
                    }
                }
            }
        }
        (rect, divider)
    }

    fn toggle_expanded(&mut self, bc: &mut BattleController, mon_idx: usize) {
        self.expanded[mon_idx] = !self.expanded[mon_idx];
        bc.update_mon_collapsed(mon_idx as i64, !self.expanded[mon_idx]);
    }

    /// One `DamageSummary` column. Returns its rect.
    #[allow(clippy::too_many_arguments)]
    fn damage_summary(
        &mut self,
        ui: &mut Ui,
        theme: &Theme,
        cfg: &Config,
        bc: &mut BattleController,
        ctrl: &mut MainController,
        mon_idx: usize,
        move_idx: usize,
        is_player: bool,
        is_test_move: bool,
        mv: Option<&MoveRenderInfo>,
        col_w: f32,
        kill_h: f32,
        actions: &mut BattleUiActions,
    ) -> Rect {
        let primary_bg = theme::lighten(theme.bg, 0.12);
        let primary_fg = theme.text;
        let contrast = theme.contrast;
        let secondary = theme.secondary;
        let base_bg = theme.bg;
        let show_highlights = cfg.get_show_move_highlights();
        let fade_enabled = cfg.get_fade_moves_without_highlight() && show_highlights;
        let highlight_state = if is_player && show_highlights && !is_test_move {
            bc.summary.get_move_highlight_state(mon_idx as i64, move_idx as i64, is_player)
        } else {
            0
        };
        let move_name = mv.map(|m| m.name.clone());
        let screenshot = self.screenshot_mode.is_some();

        // header colours
        let mut header_bg = primary_bg;
        let mut name_color = primary_fg;
        let mut name_bold = true;
        let mut should_fade = false;
        if mv.is_some() && is_player && show_highlights && !is_test_move {
            if let Some((_, c)) = HIGHLIGHT_COLORS.iter().find(|(s, _)| *s == highlight_state) {
                header_bg = hex(c);
                name_color = Color32::WHITE;
            } else if fade_enabled && highlight_state == 0 {
                should_fade = true;
                name_color = theme::blend(primary_fg, primary_bg, 0.1);
                name_bold = false;
            }
        }
        if mv.is_some() && !is_player && fade_enabled && !mv.map(|m| m.is_best_move).unwrap_or(false) {
            should_fade = true;
            name_color = theme::blend(primary_fg, primary_bg, 0.1);
            name_bold = false;
        }
        let _ = HIGHLIGHT_COLORS_IMMEDIATE;
        let range_bg = if should_fade { theme::blend(contrast, base_bg, 0.04) } else { theme::blend(contrast, base_bg, 0.10) };
        let range_fg = if should_fade { theme::blend(contrast, base_bg, 0.1) } else { contrast };
        let mut kill_bg = if should_fade { theme::blend(secondary, base_bg, 0.03) } else { theme::blend(secondary, base_bg, 0.08) };
        let mut kill_fg = if should_fade { theme::blend(secondary, base_bg, 0.1) } else { secondary };
        let mut kill_bold = false;
        let is_best = mv.map(|m| m.is_best_move).unwrap_or(false);
        if !(fade_enabled && is_player) && is_best {
            let flag_color = if is_player { theme.success } else { theme.failure };
            kill_bg = theme::blend(flag_color, base_bg, 0.20);
            kill_fg = flag_color;
            kill_bold = true;
        }

        let outer = egui::Frame::new().inner_margin(egui::Margin { left: 2, right: 2, top: 0, bottom: 2 });
        let inner = ui.allocate_ui_with_layout(Vec2::new(col_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(col_w);
            outer.show(ui, |ui| {
            ui.set_width(col_w - 4.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            let w = col_w - 4.0;
            // ---- header (26 px) ----
            let (hrect, _) = ui.allocate_exact_size(Vec2::new(w, 26.0), Sense::hover());
            ui.painter().rect_filled(hrect, CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 }, header_bg);
            let mut header_ui = ui.new_child(egui::UiBuilder::new().max_rect(hrect.shrink2(Vec2::new(3.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
            // never let header widgets paint into the neighbouring move column
            header_ui.set_clip_rect(hrect.intersect(ui.clip_rect()));
            let header_spacing_x = 2.0;
            header_ui.spacing_mut().item_spacing.x = header_spacing_x;
            // test move entry (mon 0) or name label
            let custom_data_options: Option<Vec<String>>;
            let custom_data_selection: Option<String>;
            if let Some(m) = mv {
                if move_name.as_deref() == Some(consts::MIMIC_MOVE_NAME) {
                    custom_data_options = Some(m.mimic_options.clone());
                    custom_data_selection = Some(m.mimic_data.clone());
                } else {
                    custom_data_options = m.custom_data_options.clone();
                    custom_data_selection = m.custom_data_selection.clone();
                }
            } else {
                custom_data_options = None;
                custom_data_selection = None;
            }
            let has_global_setup = if is_player { !bc.summary.player_setup_move_list.is_empty() } else { !bc.summary.enemy_setup_move_list.is_empty() };
            let (stat_stage_options, stat_stage_selection) = match mv {
                Some(m) if !has_global_setup => (m.stat_stage_options.clone(), if m.stat_stage_selection.is_empty() { "0".to_string() } else { m.stat_stage_selection.clone() }),
                _ => (None, "0".to_string()),
            };
            let test_moves = bc.get_test_moves(ctrl);
            let mut reserved_right = 0.0;
            let show_custom = !is_test_move && custom_data_options.as_ref().map(|o| !o.is_empty()).unwrap_or(false) && !(should_fade);
            let show_stat = !is_test_move && stat_stage_options.as_ref().map(|o| !o.is_empty()).unwrap_or(false);
            // For a Mimic slot, the weather/screen source move is whatever move is
            // currently being mimicked, not the literal "Mimic" display name.
            let resolved_move_name = if move_name.as_deref() == Some(consts::MIMIC_MOVE_NAME) {
                mv.map(|m| m.mimic_data.clone())
            } else {
                move_name.clone()
            };
            let weather_for_move = resolved_move_name.as_deref().and_then(|n| ctrl.gen().and_then(|g| BattleSummary::get_weather_for_move(&g, n)));
            let screen_for_move = resolved_move_name.as_deref().and_then(BattleSummary::get_screen_for_move);
            let cur_weather_active = weather_for_move.map(|w| bc.summary.get_weather() == w && bc.summary.weather_source_mon_idx == Some(mon_idx as i64)).unwrap_or(false);
            let screen_active = screen_for_move.map(|s| bc.summary.get_screen_source_mon_idx(is_player, s) == Some(mon_idx as i64)).unwrap_or(false);
            let custom_default = custom_data_options.as_ref().and_then(|o| o.first().cloned());
            let custom_is_default = custom_data_selection.as_deref().map(|s| Some(s.to_string()) == custom_default).unwrap_or(true);
            let show_custom_now = show_custom && !(screenshot && custom_is_default);
            let show_stat_now = show_stat && !(screenshot && stat_stage_selection == "0");
            let show_weather = weather_for_move.is_some() && !(screenshot && !cur_weather_active);
            let show_screen = screen_for_move.is_some() && !(screenshot && !screen_active);
            // each trailing widget also costs one item_spacing after the name.
            // All `reserved_right` arithmetic lives here so the numbers can't
            // drift: weather/screen/stepper are fixed-width, and the
            // custom-data dropdown (Mimic etc.) is normally 70 px but shrinks
            // — dropping the move name entirely — when the header is too
            // narrow to fit everything without overflowing.
            let avail = hrect.width() - 6.0;
            if show_weather {
                reserved_right += 20.0 + header_spacing_x;
            }
            if show_screen {
                reserved_right += 20.0 + header_spacing_x;
            }
            if show_stat_now {
                reserved_right += widgets::OPTION_STEPPER_W + header_spacing_x;
            }
            // `reserved_right` here covers everything except the custom-data
            // dropdown; decide the dropdown's width against what's left.
            // The dropdown never gets less than an arrow's worth of width, and
            // never takes more than what's left after the fixed widgets. The
            // name label (or the blank space standing in for it) is sized to
            // exactly `avail - reserved_right`, so the trailing widgets always
            // end flush with the header's right edge and the stepper's "+"
            // can never be pushed out of view.
            let custom_w = if show_custom_now {
                let fits_at_70 = avail - (reserved_right + 70.0 + header_spacing_x) >= 20.0;
                if fits_at_70 {
                    70.0
                } else {
                    (avail - reserved_right - header_spacing_x).clamp(24.0, 70.0)
                }
            } else {
                0.0
            };
            let is_mimic = move_name.as_deref() == Some(consts::MIMIC_MOVE_NAME);
            if show_custom_now {
                reserved_right += custom_w + header_spacing_x;
            }
            // Exact, never floored: a floor here is what pushed the "+" off
            // the right edge at narrow widths.
            let name_w = (avail - reserved_right).max(0.0);
            // Mimic's dropdown already shows the mimicked move name, so its
            // "Mimic" label goes away as soon as it would have to be elided
            // rather than lingering as a lone "…".
            let name_full_w = move_name.as_deref().map(|n| widgets::text_width(&header_ui, n, &theme.body_bold())).unwrap_or(0.0);
            let hide_name = show_custom_now && (custom_w < 70.0 || (is_mimic && name_full_w > name_w - 8.0));
            // The test-move branch (mon_idx == 0) renders a search dropdown,
            // not the move name label, and Mimic never appears there — leave
            // its width computation unaffected by `hide_name`.
            let hide_name = hide_name && !(is_test_move && mon_idx == 0);
            if is_test_move && mon_idx == 0 {
                let slot_idx = move_idx - 4;
                let current = test_moves.get(slot_idx).cloned().unwrap_or_default();
                if self.test_move_typed[slot_idx] != current && !header_ui.memory(|m| m.has_focus(ui.id().with(("test_move", mon_idx, slot_idx)).with("edit"))) {
                    self.test_move_typed[slot_idx] = current.clone();
                }
                let id = ui.id().with(("test_move", mon_idx, slot_idx));
                let mut typed = self.test_move_typed[slot_idx].clone();
                let resp = SearchableDropdown::new(theme, id, &mut typed, &self.all_moves_cache).widths(name_w.min(160.0), name_w.max(60.0)).enabled(!should_fade).show(&mut header_ui);
                self.test_move_typed[slot_idx] = typed.clone();
                if resp.changed || (resp.enter_pressed && typed != current) {
                    bc.update_test_move(cfg, ctrl, slot_idx, &typed);
                }
            } else if hide_name {
                // No label: the dropdown stands in for the move name, so
                // center it within the name's space. Half the slack goes
                // here, the other half after the dropdown (below), which
                // keeps the trailing widgets flush right.
                // Both halves are added around the dropdown below.
            } else {
                // Mimic (and any other move with both custom data and a
                // stat-stage stepper) shows the mimicked move name in the
                // custom-data dropdown already, so dropping the name label
                // here when space is tight loses no information.
                let text = if is_test_move {
                    test_moves.get(move_idx - 4).cloned().unwrap_or_default()
                } else {
                    move_name.clone().unwrap_or_default()
                };
                let font = if name_bold { theme.body_bold() } else { theme.body() };
                let (nrect, nresp) = header_ui.allocate_exact_size(Vec2::new(name_w, 26.0), if is_player && !is_test_move { Sense::click() } else { Sense::hover() });
                let shown = widgets::elide(&header_ui, &text, &font, name_w - 8.0);
                header_ui.painter().text(nrect.center(), Align2::CENTER_CENTER, shown, font, name_color);
                if is_player && !is_test_move {
                    if show_highlights {
                        nresp.clone().on_hover_cursor(egui::CursorIcon::PointingHand);
                    }
                    let ctrl_held = header_ui.input(|i| i.modifiers.command);
                    if nresp.clicked() {
                        if ctrl_held {
                            actions.assign_move_slot = Some(move_idx as i64);
                        } else if show_highlights {
                            bc.update_move_highlight(mon_idx as i64, move_idx as i64, true, false);
                        }
                    } else if nresp.secondary_clicked() && show_highlights {
                        bc.update_move_highlight(mon_idx as i64, move_idx as i64, true, true);
                    }
                }
            }
            // Trailing widgets, left to right: weather, screen, custom, stepper.
            // Their combined width is exactly `reserved_right`, and the name
            // (or its stand-in space) consumed exactly the remainder, so the
            // stepper ends flush with the header's right edge.
            if show_weather {
                let mut checked = cur_weather_active;
                let r = widgets::checkbox(&mut header_ui, theme, &mut checked, "", !screenshot);
                r.on_hover_text(format!("Set weather to {} for this matchup and later", weather_for_move.unwrap_or("")));
                if checked != cur_weather_active {
                    if let Some(n) = &resolved_move_name {
                        bc.toggle_weather_from_move(cfg, ctrl, n, checked, Some(mon_idx as i64));
                    }
                }
            }
            if show_screen {
                let mut checked = screen_active;
                let r = widgets::checkbox(&mut header_ui, theme, &mut checked, "", !screenshot);
                r.on_hover_text(format!("Apply {} for this matchup and later", resolved_move_name.clone().unwrap_or_default()));
                if checked != screen_active {
                    if let Some(n) = &resolved_move_name {
                        bc.toggle_screen_from_move(cfg, ctrl, n, checked, mon_idx as i64, is_player);
                    }
                }
            }
            if show_custom_now {
                let opts = custom_data_options.clone().unwrap_or_default();
                let mut cur = custom_data_selection.clone().unwrap_or_else(|| opts.first().cloned().unwrap_or_default());
                let enabled = !screenshot && !(should_fade && !is_player);
                // Mimic's dropdown is cramped; hard-truncate instead of "…"
                // so every pixel goes to the mimicked move's name.
                if hide_name && name_w > 0.0 {
                    header_ui.add_space(name_w / 2.0 + header_spacing_x);
                }
                if widgets::option_menu_ex(&mut header_ui, theme, ui.id().with(("custom", mon_idx, move_idx, is_player)), &mut cur, &opts, Some(custom_w), enabled, !is_mimic) {
                    if is_mimic {
                        bc.update_mimic_selection(cfg, ctrl, &cur);
                    } else {
                        bc.update_custom_move_data(cfg, ctrl, mon_idx, move_idx, is_player, &cur);
                    }
                }
                // Second half of the centering slack (see the `hide_name` branch).
                if hide_name && name_w > 0.0 {
                    header_ui.add_space(name_w / 2.0);
                }
            }
            if show_stat_now {
                let opts = stat_stage_options.clone().unwrap_or_default();
                let mut cur = stat_stage_selection.clone();
                if widgets::option_stepper(&mut header_ui, theme, ui.id().with(("stage", mon_idx, move_idx, is_player)), &mut cur, &opts, !screenshot) {
                    bc.update_stat_stage_setup(cfg, ctrl, mon_idx, move_idx, is_player, &cur);
                }
            }
            // ---- range frame (34 px) ----
            let (rrect, _) = ui.allocate_exact_size(Vec2::new(w, 34.0), Sense::hover());
            ui.painter().rect_filled(rrect, CornerRadius::ZERO, range_bg);
            if let Some(m) = mv {
                if m.min_damage != -1 {
                    let hp = m.defending_mon_hp.max(1) as f64;
                    let pct = |v: i64| py_round(v as f64 / hp * 100.0);
                    let font = theme.body();
                    let row1_y = rrect.min.y + 8.5;
                    let row2_y = rrect.min.y + 25.5;
                    ui.painter().text(Pos2::new(rrect.min.x + 4.0, row1_y), Align2::LEFT_CENTER, format!("{} - {}", m.min_damage, m.max_damage), font.clone(), range_fg);
                    ui.painter().text(Pos2::new(rrect.max.x - 4.0, row1_y), Align2::RIGHT_CENTER, format!("{} - {}%", pct(m.min_damage), pct(m.max_damage)), font.clone(), range_fg);
                    ui.painter().text(Pos2::new(rrect.min.x + 4.0, row2_y), Align2::LEFT_CENTER, format!("{} - {}", m.crit_min_damage, m.crit_max_damage), font.clone(), range_fg);
                    ui.painter().text(Pos2::new(rrect.max.x - 4.0, row2_y), Align2::RIGHT_CENTER, format!("{} - {}%", pct(m.crit_min_damage), pct(m.crit_max_damage)), font, range_fg);
                }
            }
            // ---- kill frame (>= 52 px; `kill_h` is shared by the whole row) ----
            let mut desc_lines: Vec<(String, Color32)> = Vec::new();
            let mut pct_lines: Vec<(String, Color32)> = Vec::new();
            if let Some(m) = mv {
                for kr in &shown_kill_ranges(m) {
                    let (d, p) = format_message(cfg, *kr);
                    desc_lines.push((d, kill_fg));
                    pct_lines.push((p, kill_fg));
                }
                if let Some((d, p)) = format_recoil_line(m) {
                    desc_lines.push((d, theme.failure));
                    pct_lines.push((p, theme.failure));
                }
            }
            let line_h = theme.body().size * 1.3;
            let (krect, _) = ui.allocate_exact_size(Vec2::new(w, kill_h), Sense::hover());
            ui.painter().rect_filled(krect, CornerRadius { sw: 6, se: 6, nw: 0, ne: 0 }, kill_bg);
            let font = if kill_bold { theme.body_bold() } else { theme.body() };
            for (i, (d, c)) in desc_lines.iter().enumerate() {
                let y = krect.min.y + 1.0 + i as f32 * line_h;
                ui.painter().text(Pos2::new(krect.min.x + 4.0, y), Align2::LEFT_TOP, d, font.clone(), *c);
            }
            for (i, (p, c)) in pct_lines.iter().enumerate() {
                let y = krect.min.y + 1.0 + i as f32 * line_h;
                ui.painter().text(Pos2::new(krect.max.x - 4.0, y), Align2::RIGHT_TOP, p, font.clone(), *c);
            }
            });
        });
        inner.response.rect
    }
}

/// The kill ranges a column lists: at most three, keeping the last one.
fn shown_kill_ranges(m: &MoveRenderInfo) -> Vec<(i64, f64)> {
    let mut kill_ranges = m.kill_ranges.clone();
    let max_num = 3usize;
    if kill_ranges.len() > max_num {
        let last = *kill_ranges.last().unwrap();
        kill_ranges.truncate(max_num - 1);
        kill_ranges.push(last);
    }
    kill_ranges
}

/// Height of one column's kill frame: its kill lines plus a recoil line, at least 52 px.
fn kill_frame_height(theme: &Theme, mv: Option<&MoveRenderInfo>) -> f32 {
    let lines = mv.map(|m| shown_kill_ranges(m).len() + usize::from(format_recoil_line(m).is_some())).unwrap_or(0);
    let line_h = theme.body().size * 1.3;
    (lines as f32 * line_h + 2.0).max(52.0)
}

/// `format_message(kill_info)`: (description, percentage) of a kill range.
pub fn format_message(cfg: &Config, kill_info: (i64, f64)) -> (String, String) {
    let (n, kill_pct) = kill_info;
    if kill_pct == -1.0 {
        if cfg.do_ignore_accuracy() {
            return (format!("{}-hit kill:", n), "100 %".to_string());
        }
        return (format!("{}-hit kill, IGNORING ACC", n), String::new());
    }
    let rendered = if py_round_1(kill_pct) == (kill_pct as i64) as f64 {
        format!("{}", kill_pct as i64)
    } else {
        format!("{:.1}", kill_pct)
    };
    if cfg.do_ignore_accuracy() {
        (format!("{}-hit kill:", n), format!("{} %", rendered))
    } else {
        (format!("{}-turn kill:", n), format!("{} %", rendered))
    }
}

/// Python `round(x)` (banker's rounding) as an integer.
fn py_round(x: f64) -> i64 {
    let r = x.round();
    if (x - x.trunc()).abs() == 0.5 {
        // ties to even
        let t = x.trunc() as i64;
        if t % 2 == 0 {
            t
        } else {
            t + x.signum() as i64
        }
    } else {
        r as i64
    }
}

fn py_round_1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

/// `_format_recoil_line(move)`
pub fn format_recoil_line(m: &MoveRenderInfo) -> Option<(String, String)> {
    if m.min_damage == -1 {
        return None;
    }
    let attacker_hp = m.attacking_mon_hp.max(0);
    let mut max_hp_divisor: Option<i64> = None;
    let mut damage_divisor: Option<i64> = None;
    for flavor in &m.attack_flavor {
        if let Some((_, d)) = consts::RECOIL_MAX_HP_FLAVOR_DIVISORS.iter().find(|(f, _)| f == flavor) {
            max_hp_divisor = Some(*d);
            break;
        }
        if let Some((_, d)) = consts::RECOIL_FLAVOR_DIVISORS.iter().find(|(f, _)| f == flavor) {
            damage_divisor = Some(*d);
            break;
        }
    }
    let (recoil_min, recoil_max) = if let (Some(d), true) = (max_hp_divisor, attacker_hp > 0) {
        let v = (attacker_hp / d).max(1);
        (v, v)
    } else if let Some(d) = damage_divisor {
        let hp = m.defending_mon_hp;
        let capped_min = m.min_damage.min(hp);
        let capped_max = m.max_damage.min(hp);
        ((capped_min / d).max(1), (capped_max / d).max(1))
    } else {
        return None;
    };
    if recoil_min == recoil_max {
        let pct = if attacker_hp > 0 { format!("{} %", py_round(recoil_min as f64 / attacker_hp as f64 * 100.0)) } else { String::new() };
        Some((format!("Recoil: {}", recoil_min), pct))
    } else {
        let pct = if attacker_hp > 0 {
            format!("{} - {} %", py_round(recoil_min as f64 / attacker_hp as f64 * 100.0), py_round(recoil_max as f64 / attacker_hp as f64 * 100.0))
        } else {
            String::new()
        };
        Some((format!("Recoil: {} - {}", recoil_min, recoil_max), pct))
    }
}

/// The intimidate label + checkbox drawn right-aligned in a header half.
/// Returns the new value when toggled.
#[allow(clippy::too_many_arguments)]
fn intimidate_toggle(ui: &mut Ui, theme: &Theme, id: Id, right_x: &mut f32, header_rect: Rect, active: bool, enabled: bool, screenshot: bool) -> Option<bool> {
    let label_w = widgets::text_width(ui, "Intimidate", &theme.body());
    let w = label_w + 4.0 + 16.0 + 8.0;
    let rect = Rect::from_min_size(Pos2::new(*right_x - w, header_rect.center().y - 10.0), Vec2::new(w, 20.0));
    *right_x -= w + 4.0;
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center)).id_salt(id));
    child.spacing_mut().item_spacing.x = 4.0;
    child.label(egui::RichText::new("Intimidate").font(theme.body()).color(theme.text));
    let mut checked = active;
    let r = widgets::checkbox(&mut child, theme, &mut checked, "", enabled && !screenshot);
    let r = r.on_hover_text(if enabled { "Lower the opposing Pokemon's Attack by 1 stage at switch-in" } else { "Opposing ability prevents stat reduction" });
    let _ = r;
    if checked != active {
        Some(checked)
    } else {
        None
    }
}

/// `SetupMovesSummary`: [Reset Setup] Move: [combo] [Apply Move] Player/Enemy Setup: <list>
fn setup_moves_row(ui: &mut Ui, theme: &Theme, st: &mut SetupMovesState, is_player: bool) -> Option<Vec<String>> {
    let mut changed = false;
    ui.spacing_mut().item_spacing.x = 2.0;
    if widgets::button(ui, theme, "Reset Setup").clicked() {
        st.move_list.clear();
        changed = true;
    }
    widgets::label(ui, theme, "Move:");
    st.selector.ui(ui, theme, ui.id().with(("setup_sel", is_player)), None, true);
    if widgets::button(ui, theme, "Apply Move").clicked() {
        st.move_list.push(st.selector.get().to_string());
        changed = true;
    }
    widgets::label(ui, theme, if is_player { "Player Setup:" } else { "Enemy Setup:" });
    let shown = if st.move_list.is_empty() { "None".to_string() } else { st.move_list.join(", ") };
    widgets::label(ui, theme, shown);
    if changed {
        Some(st.move_list.clone())
    } else {
        None
    }
}
