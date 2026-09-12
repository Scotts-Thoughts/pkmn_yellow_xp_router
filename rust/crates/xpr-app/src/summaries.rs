//! The run summary panel (`secondary_windows/route_summary_window.py`,
//! docked or in its own window) and the setup summary window
//! (`setup_summary_window.py`).

use std::collections::HashSet;

use egui::{Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use xpr_core::consts;
use xpr_engine::NodeId;
use xpr_ui_kit::theme::{self, Theme};
use xpr_ui_kit::widgets::{self, StyledButton};

use crate::controller::MainController;

/// Pokémon type -> background colour (the Tk dark theme values).
pub fn type_color(t: &str) -> Color32 {
    let hex = match t {
        "Normal" => "#a8a878",
        "Fighting" => "#c03028",
        "Grass" => "#78c850",
        "Fire" => "#f08030",
        "Water" => "#6890f0",
        "Electric" => "#f3d230",
        "Ground" => "#e0c068",
        "Rock" => "#b8a038",
        "Psychic" => "#f85888",
        "Poison" => "#a040a0",
        "Flying" => "#a890f0",
        "Bug" => "#a8b820",
        "Ice" => "#98d8d8",
        "Ghost" => "#705898",
        "Dragon" => "#7038f8",
        "Steel" => "#b8b8d0",
        "Dark" => "#705848",
        "Fairy" => "#ee99ac",
        "Curse" => "#2e9fa3",
        _ => "#333333",
    };
    theme::parse_hex(hex)
}

const LIGHT_TEXT_TYPES: [&str; 5] = ["Poison", "Ghost", "Dark", "Dragon", "Fighting"];
const SUMMARY_HEADER_BG: &str = "#737373";
const SUMMARY_HEADER_CANDY_BG: &str = "#61520f";
const SUMMARY_HELD_ITEM_BG: &str = "#506878";

#[derive(Clone, Debug)]
struct SummaryInfo {
    trainer_name: String,
    mon_level: i64,
    held_item: String,
    moves: Vec<Option<String>>,
    rare_candy_count: i64,
}

#[derive(Clone, Debug)]
struct RenderInfo {
    move_name: String,
    move_type: String,
    start_idx: usize,
    end_idx: usize,
}

/// The grid data of one refresh.
#[derive(Clone, Debug, Default)]
pub struct RunSummaryData {
    headers: Vec<(String, String, Color32)>,
    held_items: Vec<RenderInfo>,
    moves: Vec<Vec<RenderInfo>>,
    empty: bool,
    show_held: bool,
}

/// `_gradient_css(base, lighten)`: top colour of the vertical gradient.
fn gradient_top(base: Color32, lighten: u8) -> Color32 {
    Color32::from_rgb(base.r().saturating_add(lighten), base.g().saturating_add(lighten), base.b().saturating_add(lighten))
}

fn gradient_mesh(ui: &mut Ui, r: Rect, top: Color32, bottom: Color32) {
    if r.width() <= 0.0 || r.height() <= 0.0 {
        return;
    }
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(r.left_top(), top);
    mesh.colored_vertex(r.right_top(), top);
    mesh.colored_vertex(r.right_bottom(), bottom);
    mesh.colored_vertex(r.left_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    ui.painter().add(egui::Shape::mesh(mesh));
}

/// A rounded cell with a lighter-top -> base vertical gradient. The corner
/// squares take the mid colour so the rounding stays exact (and transparent
/// exports keep their rounded edge).
fn gradient_rect(ui: &mut Ui, rect: Rect, base: Color32, lighten: u8, radius: u8) {
    let top = gradient_top(base, lighten);
    let mid = theme::blend(top, base, 0.5);
    ui.painter().rect_filled(rect, CornerRadius::same(radius), mid);
    let rr = radius as f32;
    gradient_mesh(ui, Rect::from_min_max(Pos2::new(rect.min.x + rr, rect.min.y), Pos2::new(rect.max.x - rr, rect.max.y)), top, base);
    let span = rect.height().max(1.0);
    let at = |y: f32| theme::blend(base, top, ((y - rect.min.y) / span) as f64);
    gradient_mesh(ui, Rect::from_min_max(Pos2::new(rect.min.x, rect.min.y + rr), Pos2::new(rect.min.x + rr, rect.max.y - rr)), at(rect.min.y + rr), at(rect.max.y - rr));
    gradient_mesh(ui, Rect::from_min_max(Pos2::new(rect.max.x - rr, rect.min.y + rr), Pos2::new(rect.max.x, rect.max.y - rr)), at(rect.min.y + rr), at(rect.max.y - rr));
}

pub struct RunSummary {
    pub docked: bool,
    data: RunSummaryData,
    pub cached_size: Vec2,
}

impl RunSummary {
    pub fn new(docked: bool) -> RunSummary {
        RunSummary { docked, data: RunSummaryData::default(), cached_size: Vec2::new(600.0, 200.0) }
    }

    /// `_refresh`: rebuild the summary from the route.
    pub fn refresh(&mut self, ctrl: &MainController) {
        let mut data = RunSummaryData::default();
        let Some(gen) = ctrl.gen() else {
            data.empty = true;
            self.data = data;
            return;
        };
        let mut summary_list: Vec<SummaryInfo> = Vec::new();
        let mut elite_four_seen: HashSet<String> = HashSet::new();
        let crystal_excluded = ["Leader Brock", "Leader Misty", "Leader Lt.Surge", "Leader Erika", "Leader Sabrina", "Leader Blaine", "Leader Janine"];
        let heartgold_excluded = [
            "Leader Brock",
            "Leader Misty",
            "Leader Lt.Surge",
            "Leader Erika",
            "Leader Sabrina",
            "Leader Blaine",
            "Leader Janine",
            "Elite Four Will Rematch 2",
            "Elite Four Koga Rematch 2",
            "Elite Four Bruno Rematch 2",
            "Elite Four Karen Rematch 2",
        ];
        let mut cur: Option<NodeId> = ctrl.get_next_event(None);
        while let Some(gid) = cur {
            if let Some(g) = ctrl.router.group(gid) {
                let ed = &g.event_definition;
                if let (Some(td), true) = (&ed.trainer_def, ed.is_enabled_flag()) {
                    let name = td.trainer_name.clone();
                    let is_e4 = name.starts_with("Elite Four ");
                    let skip = ed.is_highlighted()
                        || (gen.version_name() == consts::CRYSTAL_VERSION && crystal_excluded.contains(&name.as_str()))
                        || (gen.version_name() == consts::HEART_GOLD_VERSION && heartgold_excluded.contains(&name.as_str()))
                        || (is_e4 && elite_four_seen.contains(&name));
                    if !skip {
                        if is_e4 {
                            elite_four_seen.insert(name.clone());
                        }
                        if gen.is_major_fight(&name) {
                            if let Some(init) = &g.init_state {
                                summary_list.push(SummaryInfo {
                                    trainer_name: name,
                                    mon_level: init.solo_pkmn.cur_level,
                                    held_item: init.solo_pkmn.held_item.clone().unwrap_or_else(|| "None".to_string()),
                                    moves: init.solo_pkmn.move_list.clone(),
                                    rare_candy_count: 0,
                                });
                            }
                        }
                    }
                } else if let (Some(rc), true) = (&ed.rare_candy, ed.is_enabled_flag()) {
                    if rc.amount > 0 {
                        if let Some(fin) = &g.final_state {
                            summary_list.push(SummaryInfo {
                                trainer_name: String::new(),
                                mon_level: fin.solo_pkmn.cur_level,
                                held_item: fin.solo_pkmn.held_item.clone().unwrap_or_else(|| "None".to_string()),
                                moves: fin.solo_pkmn.move_list.clone(),
                                rare_candy_count: rc.amount,
                            });
                        }
                    }
                }
            }
            cur = ctrl.get_next_event(Some(gid));
        }
        if summary_list.is_empty() {
            data.empty = true;
            self.data = data;
            return;
        }
        let mut move_display: Vec<Vec<RenderInfo>> = vec![Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        let mut held_display: Vec<RenderInfo> = Vec::new();
        for (idx, s) in summary_list.iter().enumerate() {
            let (header_bg, trainer_text, level_text) = if !s.trainer_name.is_empty() {
                let split: Vec<&str> = s.trainer_name.split(' ').collect();
                let trainer_text = if split.len() > 2 {
                    format!("{}\n{}", split[0..2].join(" "), split[2..].join(" "))
                } else if split.len() == 2 && split[1].len() > 1 {
                    format!("{}\n{}", split[0], split[1])
                } else {
                    s.trainer_name.clone()
                };
                (theme::parse_hex(SUMMARY_HEADER_BG), trainer_text, format!("Lv: {}", s.mon_level))
            } else {
                (theme::parse_hex(SUMMARY_HEADER_CANDY_BG), format!("Rare Candy\nx{}", s.rare_candy_count), format!("Lv: {}->{}", s.mon_level - s.rare_candy_count, s.mon_level))
            };
            data.headers.push((trainer_text, level_text, header_bg));
            if held_display.last().map(|h| h.move_name != s.held_item).unwrap_or(true) {
                held_display.push(RenderInfo { move_name: s.held_item.clone(), move_type: String::new(), start_idx: idx, end_idx: idx });
            } else {
                held_display.last_mut().unwrap().end_idx = idx;
            }
            for move_idx in 0..4 {
                let mut next_move = s.moves.get(move_idx).cloned().flatten().unwrap_or_default();
                let move_type = if next_move.is_empty() {
                    String::new()
                } else if next_move == consts::HIDDEN_POWER_MOVE_NAME {
                    let t = ctrl.get_dvs().map(|d| gen.get_hidden_power(&d).0).unwrap_or_default();
                    next_move = format!("{} ({})", next_move, t);
                    t
                } else {
                    gen.move_db().get_move(&next_move).map(|m| m.move_type.clone()).unwrap_or_default()
                };
                let slot = &mut move_display[move_idx];
                if slot.last().map(|r| r.move_name != next_move).unwrap_or(true) {
                    slot.push(RenderInfo { move_name: next_move, move_type, start_idx: idx, end_idx: idx });
                } else {
                    slot.last_mut().unwrap().end_idx = idx;
                }
            }
        }
        data.held_items = held_display;
        data.moves = move_display;
        data.show_held = gen.get_generation() != 1;
        self.data = data;
    }

    /// Draw the panel (toolbar + grid). Returns (dock toggled, closed, export).
    pub fn ui(&mut self, ui: &mut Ui, theme: &Theme, ctrl: &MainController) -> (bool, bool, bool) {
        let mut dock_toggled = false;
        let mut closed = false;
        let mut export = false;
        // toolbar
        egui::Frame::new().fill(Color32::from_rgb(0x2a, 0x2a, 0x2a)).inner_margin(egui::Margin { left: 6, right: 6, top: 3, bottom: 3 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.label(egui::RichText::new("Run Summary").font(theme.body_bold()).color(Color32::from_rgb(0xd4, 0xd4, 0xd4)));
                if StyledButton::new(theme, "Export Screenshot").min_size(Vec2::new(0.0, 22.0)).show(ui).clicked() {
                    export = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if StyledButton::new(theme, "×").min_size(Vec2::new(22.0, 22.0)).padding(Vec2::ZERO).show(ui).on_hover_text("Close").clicked() {
                        closed = true;
                    }
                    let (label, tip) = if self.docked { ("Undock", "Pop out to separate window") } else { ("Dock", "Dock back into main window") };
                    if StyledButton::new(theme, label).min_size(Vec2::new(0.0, 22.0)).show(ui).on_hover_text(tip).clicked() {
                        dock_toggled = true;
                    }
                });
            });
        });
        ui.painter().line_segment([ui.cursor().left_top(), Pos2::new(ui.max_rect().max.x, ui.cursor().min.y)], Stroke::new(1.0_f32, Color32::from_rgb(0x44, 0x44, 0x44)));
        let _ = ctrl;
        egui::ScrollArea::horizontal().id_salt("run_summary_scroll").auto_shrink([false, true]).show(ui, |ui| {
            let r = self.grid(ui, theme);
            self.cached_size = Vec2::new(r.width() + 16.0 + 12.0, r.height() + 30.0 + 16.0 + 12.0);
        });
        (dock_toggled, closed, export)
    }

    /// The gradient-cell grid; returns its rect (for screenshots).
    pub fn grid(&self, ui: &mut Ui, theme: &Theme) -> Rect {
        let font = theme.body();
        let inner = egui::Frame::new().inner_margin(egui::Margin::same(6)).show(ui, |ui| {
            if self.data.empty {
                let text = "No major fights in route. Please add major fights or highlight other fights to see summary";
                let galley = ui.fonts_mut(|f| f.layout_no_wrap(text.to_string(), font.clone(), Color32::WHITE));
                let (rect, _) = ui.allocate_exact_size(galley.size() + Vec2::new(10.0, 10.0), Sense::hover());
                gradient_rect(ui, rect, theme::parse_hex(SUMMARY_HEADER_BG), 30, 3);
                ui.painter().galley(Pos2::new(rect.min.x + 5.0, rect.min.y + 5.0), galley, Color32::WHITE);
                return;
            }
            let n = self.data.headers.len();
            // column widths: widest header line / move name of that column
            let mut col_w: Vec<f32> = vec![0.0; n];
            for (i, (t, l, _)) in self.data.headers.iter().enumerate() {
                for line in t.split('\n').chain(std::iter::once(l.as_str())) {
                    col_w[i] = col_w[i].max(widgets::text_width(ui, line, &font) + 10.0);
                }
            }
            for slot in &self.data.moves {
                for info in slot {
                    let span = (info.end_idx - info.start_idx + 1) as f32;
                    let w = widgets::text_width(ui, &info.move_name, &font) + 8.0;
                    for i in info.start_idx..=info.end_idx {
                        col_w[i] = col_w[i].max(w / span);
                    }
                }
            }
            let spacing = 4.0;
            let header_h = 3.0 * font.size * 1.3 + 22.0;
            let cell_h = font.size * 1.3 + 6.0;
            let origin = ui.cursor().min;
            let x_of = |i: usize| -> f32 { origin.x + col_w[..i].iter().sum::<f32>() + i as f32 * spacing };
            let total_w: f32 = col_w.iter().sum::<f32>() + (n.saturating_sub(1)) as f32 * spacing;
            let mut y = origin.y;
            // header row
            for (i, (t, l, bg)) in self.data.headers.iter().enumerate() {
                let rect = Rect::from_min_size(Pos2::new(x_of(i), y), Vec2::new(col_w[i], header_h));
                gradient_rect(ui, rect, *bg, 30, 3);
                let lines: Vec<&str> = t.split('\n').collect();
                let mut ty = rect.min.y + 11.0;
                for line in lines {
                    ui.painter().text(Pos2::new(rect.center().x, ty), Align2::CENTER_TOP, line, font.clone(), Color32::WHITE);
                    ty += font.size * 1.3;
                }
                ui.painter().text(Pos2::new(rect.center().x, rect.max.y - 11.0), Align2::CENTER_BOTTOM, l, font.clone(), Color32::WHITE);
            }
            y += header_h + spacing;
            if self.data.show_held {
                for info in &self.data.held_items {
                    let x0 = x_of(info.start_idx);
                    let x1 = x_of(info.end_idx) + col_w[info.end_idx];
                    let rect = Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, y + cell_h));
                    gradient_rect(ui, rect, theme::parse_hex(SUMMARY_HELD_ITEM_BG), 25, 3);
                    let text = if info.move_name.is_empty() || info.move_name == "None" { "None" } else { info.move_name.as_str() };
                    ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, font.clone(), Color32::WHITE);
                }
                y += cell_h + spacing;
            }
            for slot in &self.data.moves {
                for info in slot {
                    let x0 = x_of(info.start_idx);
                    let x1 = x_of(info.end_idx) + col_w[info.end_idx];
                    let rect = Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, y + cell_h));
                    let bg = type_color(&info.move_type);
                    let fg = if LIGHT_TEXT_TYPES.contains(&info.move_type.as_str()) { Color32::WHITE } else { theme.bg };
                    gradient_rect(ui, rect, bg, 35, 2);
                    ui.painter().text(rect.center(), Align2::CENTER_CENTER, &info.move_name, font.clone(), fg);
                }
                y += cell_h + spacing;
            }
            ui.allocate_exact_size(Vec2::new(total_w, y - origin.y), Sense::hover());
        });
        inner.response.rect
    }
}

/// `SetupSummaryWindow._refresh`
pub fn setup_summary_text(ctrl: &MainController) -> String {
    let Some(gen) = ctrl.gen() else { return "No setup moves used".to_string() };
    let mut moves_used: Vec<String> = Vec::new();
    let mut cur = ctrl.get_next_event(None);
    while let Some(gid) = cur {
        if let Some(g) = ctrl.router.group(gid) {
            let ed = &g.event_definition;
            if let Some(td) = &ed.trainer_def {
                if ed.is_enabled_flag() && (ed.is_highlighted() || gen.is_major_fight(&td.trainer_name)) && !td.setup_moves.is_empty() {
                    let mut counts: Vec<(String, i64)> = Vec::new();
                    for sm in &td.setup_moves {
                        match counts.iter_mut().find(|(m, _)| m == sm) {
                            Some(e) => e.1 += 1,
                            None => counts.push((sm.clone(), 1)),
                        }
                    }
                    let text: Vec<String> = counts.iter().map(|(m, c)| format!("x{} {}", c, m)).collect();
                    moves_used.push(format!("{}: {}", ed.get_label(&gen).unwrap_or_default(), text.join(",")));
                }
            }
        }
        cur = ctrl.get_next_event(Some(gid));
    }
    if moves_used.is_empty() {
        "No setup moves used".to_string()
    } else {
        let mut lines = vec!["Setup Moves:".to_string()];
        lines.extend(moves_used);
        lines.join("\n")
    }
}
