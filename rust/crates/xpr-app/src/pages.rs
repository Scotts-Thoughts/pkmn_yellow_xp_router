//! The landing page (`pages/landing_page.py`) and the new-route page
//! (`pages/new_route_page.py`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use xpr_core::consts;
use xpr_core::io_utils;
use xpr_core::{Config, Paths};
use xpr_data::model::Nature;
use xpr_data::{GenData, Registry};
use xpr_recorder::QuickStartPhase;
use xpr_ui_kit::modal::behind_modal;
use xpr_ui_kit::theme::Theme;
use xpr_ui_kit::widgets::{self, Entry, StyledButton};

use crate::assets::Assets;
use crate::custom_dvs::CustomDvsFrame;
use crate::editors::OptionMenu;
use crate::route_index::RouteIndex;

pub const SORT_MOST_RECENT: &str = "most_recent";
pub const SORT_GAME: &str = "game";
pub const SORT_ALPHABETICAL: &str = "alphabetical";
const NO_ROUTES: &str = "No saved routes found";

/// What the landing page asks for.
#[derive(Clone, Debug, Default)]
pub struct LandingActions {
    pub create_route: bool,
    /// "Start Recording": connect to GameHook and build the route from the
    /// game (version from the mapper, solo mon from the first Pokémon).
    pub start_recording: bool,
    pub load_route: Option<PathBuf>,
    /// "Compare Routes": `Some(Some(path))` seeds slot A with the selected
    /// route, `Some(None)` opens the page empty.
    pub compare_routes: Option<Option<PathBuf>>,
    pub auto_load_toggled: bool,
}

/// What the quick-start panel asks for.
#[derive(Clone, Debug, Default)]
pub struct QuickStartActions {
    pub cancel: bool,
    /// take the Pokémon already in slot 1 instead of waiting for a new game
    pub use_current: bool,
    /// retry the connection / reload the mapper
    pub reconnect: bool,
}

pub struct LandingPage {
    current_sort: String,
    selected_game_filter: String,
    search_text: String,
    search_deadline: Option<Instant>,
    selected_route: Option<String>,
    game_filter: OptionMenu,
    pub auto_load: bool,
}

impl LandingPage {
    pub fn new(cfg: &Config) -> LandingPage {
        let mut sort = cfg.get_landing_page_sort();
        if sort.is_empty() {
            sort = SORT_MOST_RECENT.to_string();
        }
        let mut filter = cfg.get_landing_page_game_filter();
        if filter.is_empty() {
            filter = "All Games".to_string();
        }
        LandingPage {
            current_sort: sort,
            selected_game_filter: filter.clone(),
            search_text: cfg.get_landing_page_search_filter(),
            search_deadline: None,
            selected_route: None,
            game_filter: OptionMenu::new(vec!["All Games".to_string()], Some(&filter)),
            auto_load: cfg.get_auto_load_most_recent_route(),
        }
    }

    fn populate_game_filter(&mut self, registry: &Registry) {
        let mut all = vec!["All Games".to_string()];
        all.extend(registry.get_gen_names(true, true));
        let sel = self.selected_game_filter.clone();
        self.game_filter.new_values(all, Some(&sel));
        if self.game_filter.get() != sel {
            self.selected_game_filter = "All Games".to_string();
            self.game_filter.set("All Games");
        }
    }

    pub fn tick(&mut self, ctx: &egui::Context, cfg: &mut Config) {
        if let Some(d) = self.search_deadline {
            let now = Instant::now();
            if now >= d {
                self.search_deadline = None;
                cfg.set_landing_page_search_filter(&self.search_text);
            } else {
                ctx.request_repaint_after(d - now);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn ui(&mut self, ui: &mut Ui, theme: &Theme, cfg: &mut Config, paths: &Paths, registry: &Registry, index: &RouteIndex, actions: &mut LandingActions) {
        let key_load = !behind_modal(ui) && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.vertical_centered(|ui| {
            ui.add_space(50.0);
            ui.label(egui::RichText::new("Pokemon Solo Challenge Router").font(theme.font_bold(24.0)).color(theme.text));
            ui.add_space(20.0);
            let record = StyledButton::new(theme, egui::RichText::new("Start Recording").font(theme.font_bold(14.0)).color(Color32::from_rgb(0xe7, 0x4c, 0x3c)))
                .dot(Color32::from_rgb(0xe7, 0x4c, 0x3c))
                .min_size(Vec2::new(350.0, 50.0))
                .text_color(Color32::from_rgb(0xe7, 0x4c, 0x3c))
                .show(ui)
                .on_hover_text("Connect to GameHook now: the game comes from the loaded mapper and the route is set up from your first Pokémon (species, DVs/IVs, nature, ability) the moment you receive it.");
            if record.clicked() {
                actions.start_recording = true;
            }
            ui.add_space(10.0);
            let create = StyledButton::new(theme, egui::RichText::new("Create New Route").font(theme.font_bold(14.0))).min_size(Vec2::new(350.0, 50.0)).show(ui);
            if create.clicked() {
                actions.create_route = true;
            }
            ui.add_space(10.0);
            let can_load = self.selected_route.as_ref().map(|r| r != NO_ROUTES).unwrap_or(false);
            let load = StyledButton::new(theme, egui::RichText::new("Load Selected Route").font(theme.font_bold(14.0))).min_size(Vec2::new(350.0, 50.0)).enabled(can_load).show(ui);
            if (load.clicked() || key_load) && can_load {
                if let Some(r) = &self.selected_route {
                    actions.load_route = Some(io_utils::get_existing_route_path(paths, r));
                }
            }
            ui.add_space(10.0);
            let compare = StyledButton::new(theme, egui::RichText::new("Compare Routes").font(theme.font_bold(14.0))).min_size(Vec2::new(350.0, 50.0)).show(ui);
            if compare.on_hover_text("See how two routes differ. The selected route, if any, becomes route A.").clicked() {
                let selected = self.selected_route.as_ref().filter(|_| can_load).map(|r| io_utils::get_existing_route_path(paths, r));
                actions.compare_routes = Some(selected);
            }
            ui.add_space(10.0);
            let mut auto = self.auto_load;
            if widgets::checkbox(ui, theme, &mut auto, "Automatically Load Most Recent Route on Startup", true).changed() {
                self.auto_load = auto;
                cfg.set_auto_load_most_recent_route(auto);
                actions.auto_load_toggled = true;
            }
            ui.add_space(10.0);
            // ---- routes section (600 px wide) ----
            let width = 600.0;
            ui.allocate_ui_with_layout(Vec2::new(width, ui.available_height()), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = 4.0;
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("Routes").font(theme.font_bold(18.0)).color(theme.text));
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let mut new_sort: Option<&str> = None;
                    if widgets::seg_toggle(ui, theme, "Most Recent", self.current_sort == SORT_MOST_RECENT).clicked() {
                        new_sort = Some(SORT_MOST_RECENT);
                    }
                    if widgets::seg_toggle(ui, theme, "Alphabetical", self.current_sort == SORT_ALPHABETICAL).clicked() {
                        new_sort = Some(SORT_ALPHABETICAL);
                    }
                    if widgets::seg_toggle(ui, theme, "Game", self.current_sort == SORT_GAME).clicked() {
                        new_sort = Some(SORT_GAME);
                    }
                    if let Some(s) = new_sort {
                        self.current_sort = s.to_string();
                        cfg.set_landing_page_sort(s);
                        if s == SORT_GAME {
                            self.populate_game_filter(registry);
                        }
                    }
                    if self.current_sort == SORT_GAME {
                        ui.add_space(8.0);
                        if self.game_filter.options.len() <= 1 {
                            self.populate_game_filter(registry);
                        }
                        if self.game_filter.ui(ui, theme, ui.id().with("game_filter"), Some(160.0), true) {
                            self.selected_game_filter = self.game_filter.get().to_string();
                            cfg.set_landing_page_game_filter(&self.selected_game_filter);
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    widgets::label(ui, theme, "Search:");
                    let r = Entry::new(theme, &mut self.search_text).width(ui.available_width()).hint("Filter routes...").id(ui.id().with("landing_search")).clearable().show(ui);
                    if r.changed {
                        self.search_deadline = Some(Instant::now() + Duration::from_millis(300));
                    }
                });
                self.route_table(ui, theme, paths, index, actions);
            });
        });
    }

    fn route_table(&mut self, ui: &mut Ui, theme: &Theme, paths: &Paths, index: &RouteIndex, actions: &mut LandingActions) {
        // gather + filter + sort
        let mut entries: Vec<(&String, &String, &String, f64)> = index.entries.values().map(|e| (&e.name, &e.version, &e.species, e.mtime)).collect();
        if self.current_sort == SORT_GAME && self.selected_game_filter != "All Games" {
            entries.retain(|(_, v, _, _)| **v == self.selected_game_filter);
        }
        let st = self.search_text.trim().to_lowercase();
        if !st.is_empty() {
            entries.retain(|(n, v, s, _)| n.to_lowercase().contains(&st) || v.to_lowercase().contains(&st) || s.to_lowercase().contains(&st));
        }
        match self.current_sort.as_str() {
            SORT_GAME => entries.sort_by(|a, b| (a.1, a.0).cmp(&(b.1, b.0))),
            SORT_ALPHABETICAL => entries.sort_by_key(|e| e.0.to_lowercase()),
            _ => entries.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal)),
        }
        let font = theme.body();
        let total_w = ui.available_width();
        let widths = [80.0, 90.0, (total_w - 80.0 - 90.0 - 130.0).max(120.0), 130.0];
        let headers = ["Game", "Species", "Route Name", "Date Played"];
        let frame = egui::Frame::new().fill(theme.bg_input).stroke(Stroke::new(1.0_f32, theme.border));
        frame.show(ui, |ui| {
            ui.set_min_height(300.0);
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let (hrect, _) = ui.allocate_exact_size(Vec2::new(total_w, 22.0), Sense::hover());
            let mut x = hrect.min.x;
            for (i, h) in headers.iter().enumerate() {
                let r = Rect::from_min_size(Pos2::new(x, hrect.min.y), Vec2::new(widths[i], 22.0));
                widgets::paint_table_header_cell(ui.painter(), theme, r, h, i == 0);
                x += widths[i];
            }
            widgets::show_scroll(ui, egui::ScrollArea::vertical().id_salt("landing_routes").auto_shrink([false, false]).max_height(ui.available_height().max(300.0)), |ui| {
                if !index.loaded {
                    ui.add_space(4.0);
                    widgets::label_colored(ui, theme, "Loading routes...", theme.secondary);
                    return;
                }
                if entries.is_empty() {
                    let (r, _) = ui.allocate_exact_size(Vec2::new(total_w, 22.0), Sense::hover());
                    ui.painter().text(Pos2::new(r.min.x + widths[0] + widths[1] + 4.0, r.center().y), Align2::LEFT_CENTER, NO_ROUTES, font.clone(), theme.secondary);
                    self.selected_route = None;
                    return;
                }
                let mut clicked: Option<(String, bool)> = None;
                for (row_idx, (name, version, species, mtime)) in entries.iter().enumerate() {
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(total_w, 22.0), Sense::click());
                    let selected = self.selected_route.as_deref() == Some(name.as_str());
                    let fill = if selected {
                        theme.accent
                    } else if resp.hovered() {
                        theme.hover_bg
                    } else if row_idx % 2 == 1 {
                        theme.bg_lighter
                    } else {
                        theme.bg_input
                    };
                    ui.painter().rect_filled(r, CornerRadius::ZERO, fill);
                    let color = if selected { Color32::WHITE } else { theme.text };
                    let date = chrono::DateTime::<chrono::Local>::from(std::time::UNIX_EPOCH + Duration::from_secs_f64((*mtime).max(0.0))).format("%Y-%m-%d %H:%M").to_string();
                    let cells = [version.as_str(), species.as_str(), name.as_str(), date.as_str()];
                    let mut x = r.min.x;
                    for (i, c) in cells.iter().enumerate() {
                        let shown = widgets::elide(ui, c, &font, widths[i] - 8.0);
                        ui.painter().text(Pos2::new(x + 4.0, r.center().y), Align2::LEFT_CENTER, shown, font.clone(), color);
                        x += widths[i];
                    }
                    if resp.clicked() {
                        clicked = Some((name.to_string(), false));
                    }
                    if resp.double_clicked() {
                        clicked = Some((name.to_string(), true));
                    }
                }
                if let Some((name, dbl)) = clicked {
                    self.selected_route = Some(name.clone());
                    if dbl {
                        actions.load_route = Some(io_utils::get_existing_route_path(paths, &name));
                    }
                }
            });
        });
    }
}

// ---------------------------------------------------------------------------
// Quick start ("Start Recording") panel
// ---------------------------------------------------------------------------

/// The landing page while a quick start is running: what the GameHook
/// session is doing, what it found, and the ways out.
pub fn quick_start_ui(ui: &mut Ui, theme: &Theme, phase: &QuickStartPhase, url: &str, actions: &mut QuickStartActions) {
    if !behind_modal(ui) && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        actions.cancel = true;
    }
    let red = Color32::from_rgb(0xe7, 0x4c, 0x3c);
    let green = Color32::from_rgb(0x2e, 0xcc, 0x71);
    let busy = !matches!(phase, QuickStartPhase::Failed(_) | QuickStartPhase::UnsupportedGame(_));
    ui.vertical_centered(|ui| {
        ui.add_space(50.0);
        ui.label(egui::RichText::new("Pokemon Solo Challenge Router").font(theme.font_bold(24.0)).color(theme.text));
        ui.add_space(30.0);
        let width = 600.0;
        let frame = egui::Frame::new().fill(theme.bg_input).stroke(Stroke::new(1.0_f32, theme.border)).inner_margin(egui::Margin::same(20)).corner_radius(CornerRadius::same(4));
        frame.show(ui, |ui| {
            ui.set_width(width);
            ui.vertical_centered(|ui| {
                ui.horizontal(|ui| {
                    ui.add_space((width - 200.0) / 2.0);
                    if busy {
                        ui.add(egui::Spinner::new().size(18.0).color(red));
                    }
                    // painted, not "●": the configured font may lack the glyph
                    let (dot, _) = ui.allocate_exact_size(Vec2::new(14.0, 24.0), Sense::hover());
                    ui.painter().circle_filled(dot.center(), 6.0, red);
                    ui.label(egui::RichText::new("Start Recording").font(theme.font_bold(18.0)).color(red));
                });
                ui.add_space(14.0);
                // game line
                if let Some(game) = phase.game() {
                    let line = match &game.sibling {
                        None => format!("Game: {}   (route version: {})", game.mapper_name, game.version),
                        Some(other) => format!("Game: {}   (route version: {} — the mapper serves {} too; both share one data set)", game.mapper_name, game.version, other),
                    };
                    widgets::label_colored(ui, theme, line, green);
                    ui.add_space(8.0);
                }
                let body = theme.font(13.0);
                let say = |ui: &mut Ui, text: String, color: Color32| {
                    ui.add(egui::Label::new(egui::RichText::new(text).font(body.clone()).color(color)).wrap());
                };
                match phase {
                    QuickStartPhase::Connecting(msg) => say(ui, msg.clone(), theme.text),
                    QuickStartPhase::NoMapper => say(
                        ui,
                        "GameHook is connected but has no mapper loaded.\nOpen your game in the emulator and load its mapper in GameHook / Poke-A-Byte; the router picks it up automatically.".to_string(),
                        theme.text,
                    ),
                    QuickStartPhase::UnsupportedGame(name) => say(ui, format!("The loaded mapper ('{}') is not a game the router supports.\nLoad the mapper of a supported game, then retry.", name), red),
                    QuickStartPhase::WaitingForNewGame { current, .. } => {
                        say(
                            ui,
                            format!(
                                "Party slot 1 already holds {} (Lv {}).\nStart a new game: the route is set up, and recording begins, the moment you receive your first Pokémon.",
                                current.species, current.level
                            ),
                            theme.text,
                        );
                        ui.add_space(10.0);
                        if StyledButton::new(theme, format!("Use this {} instead", current.species)).fixed_width(260.0).show(ui).clicked() {
                            actions.use_current = true;
                        }
                    }
                    QuickStartPhase::WaitingForStarter(_) => say(ui, "Waiting for you to receive your first Pokémon...\nThe route takes its species, DVs/IVs, nature and ability from the game.".to_string(), theme.text),
                    QuickStartPhase::Settling { current, .. } => say(ui, format!("Received {} (Lv {})! Reading its stats...", current.species, current.level), green),
                    QuickStartPhase::Done(info) => say(ui, format!("Creating the route: {}", info.summary()), green),
                    QuickStartPhase::Failed(msg) => say(ui, msg.clone(), red),
                }
                ui.add_space(14.0);
                widgets::label_colored(ui, theme, format!("GameHook: {}", url), theme.secondary);
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    let retry = matches!(phase, QuickStartPhase::Connecting(_) | QuickStartPhase::NoMapper | QuickStartPhase::UnsupportedGame(_) | QuickStartPhase::Failed(_));
                    let n = if retry { 2.0 } else { 1.0 };
                    ui.add_space((width - n * 180.0 - (n - 1.0) * 10.0) / 2.0);
                    if retry && StyledButton::new(theme, "Retry / Reload Mapper").fixed_width(180.0).show(ui).clicked() {
                        actions.reconnect = true;
                    }
                    if StyledButton::new(theme, "Cancel").fixed_width(180.0).show(ui).clicked() {
                        actions.cancel = true;
                    }
                });
            });
        });
    });
}

// ---------------------------------------------------------------------------
// New route page
// ---------------------------------------------------------------------------

/// Game information: version -> (generation, platform, recorder status)
fn game_info(version: &str) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match version {
        consts::RED_VERSION | consts::BLUE_VERSION | consts::YELLOW_VERSION => ("Generation 1", "GB/GBC", "Available"),
        consts::GOLD_VERSION | consts::SILVER_VERSION | consts::CRYSTAL_VERSION => ("Generation 2", "GBC", "Available"),
        consts::RUBY_VERSION | consts::SAPPHIRE_VERSION | consts::EMERALD_VERSION | consts::FIRE_RED_VERSION | consts::LEAF_GREEN_VERSION => ("Generation 3", "GBA", "Available"),
        consts::DIAMOND_VERSION | consts::PEARL_VERSION => ("Generation 4", "NDS", "Unavailable"),
        consts::PLATINUM_VERSION => ("Generation 4", "NDS", "In Beta"),
        consts::HEART_GOLD_VERSION | consts::SOUL_SILVER_VERSION => ("Generation 4", "NDS", "In Alpha"),
        consts::BLACK_VERSION | consts::WHITE_VERSION | consts::BLACK_2_VERSION | consts::WHITE_2_VERSION => ("Generation 5", "NDS", "In Alpha"),
        _ => return None,
    })
}

#[derive(Clone, Debug)]
struct GameRow {
    name: String,
    gen: String,
    platform: String,
    recorder: String,
}

/// What the new-route page asks for.
#[derive(Clone, Debug, Default)]
pub struct NewRouteActions {
    pub cancel: bool,
    pub create: Option<CreateRequest>,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CreateRequest {
    pub solo_mon: String,
    pub base_route_path: Option<PathBuf>,
    pub version: String,
    pub dvs: xpr_data::model::StatBlock,
    pub ability_idx: i64,
    pub nature: Nature,
}

pub struct NewRoutePage {
    games: Vec<GameRow>,
    selected_game: Option<String>,
    selected_gen: Option<Arc<GenData>>,
    current_gen_num: Option<u8>,
    pkmn_filter: String,
    solo_selector: OptionMenu,
    min_battles_filter: String,
    min_battles_cache: Vec<String>,
    min_battles_selector: OptionMenu,
    pub dvs: CustomDvsFrame,
    /// false while the base-route list was built before the background
    /// route index finished loading (rebuilt once it has)
    base_routes_from_index: bool,
    pkmn_list_cache: HashMap<(String, String), Vec<String>>,
    pending_game_load: bool,
}

impl NewRoutePage {
    pub fn new() -> NewRoutePage {
        NewRoutePage {
            games: Vec::new(),
            selected_game: None,
            selected_gen: None,
            current_gen_num: None,
            pkmn_filter: String::new(),
            solo_selector: OptionMenu::new(vec![consts::NO_POKEMON.to_string()], None),
            min_battles_filter: String::new(),
            min_battles_cache: vec![consts::EMPTY_ROUTE_NAME.to_string()],
            min_battles_selector: OptionMenu::new(vec![consts::EMPTY_ROUTE_NAME.to_string()], None),
            dvs: CustomDvsFrame::new(),
            base_routes_from_index: true,
            pkmn_list_cache: HashMap::new(),
            pending_game_load: false,
        }
    }

    /// `_populate_game_table`
    pub fn populate_game_table(&mut self, registry: &Registry) {
        let all = registry.get_gen_names(true, true);
        // the page's own order (Red before Yellow), unlike const.VERSION_LIST
        let official: Vec<&str> = vec![
            consts::RED_VERSION,
            consts::BLUE_VERSION,
            consts::YELLOW_VERSION,
            consts::GOLD_VERSION,
            consts::SILVER_VERSION,
            consts::CRYSTAL_VERSION,
            consts::RUBY_VERSION,
            consts::SAPPHIRE_VERSION,
            consts::EMERALD_VERSION,
            consts::FIRE_RED_VERSION,
            consts::LEAF_GREEN_VERSION,
            consts::DIAMOND_VERSION,
            consts::PEARL_VERSION,
            consts::PLATINUM_VERSION,
            consts::HEART_GOLD_VERSION,
            consts::SOUL_SILVER_VERSION,
            consts::BLACK_VERSION,
            consts::WHITE_VERSION,
            consts::BLACK_2_VERSION,
            consts::WHITE_2_VERSION,
        ];
        let mut sorted: Vec<String> = official.iter().filter(|g| all.contains(&g.to_string())).map(|s| s.to_string()).collect();
        let mut custom: Vec<String> = all.iter().filter(|g| !official.contains(&g.as_str())).cloned().collect();
        custom.sort();
        sorted.extend(custom);
        self.games = sorted
            .into_iter()
            .map(|name| match game_info(&name) {
                Some((g, p, r)) => GameRow { name, gen: g.into(), platform: p.into(), recorder: r.into() },
                None => {
                    let (gen, platform, recorder) = match registry.get_version(&name) {
                        Ok(obj) => {
                            let g = format!("Generation {}", obj.get_generation());
                            let rec = obj.base_version_name().and_then(game_info).map(|(_, _, r)| r).unwrap_or("Unknown");
                            (g, "Custom".to_string(), rec.to_string())
                        }
                        Err(_) => ("Unknown".to_string(), "Unknown".to_string(), "Unknown".to_string()),
                    };
                    GameRow { name, gen, platform, recorder }
                }
            })
            .collect();
    }

    /// `refresh_game_list`
    pub fn refresh_game_list(&mut self, registry: &Registry, index: &RouteIndex) {
        if let Err(e) = registry.reload_all_custom_gens() {
            log::warn!("Could not reload some custom gens: {}", e);
        }
        let current = self.selected_game.clone();
        self.populate_game_table(registry);
        match current {
            Some(c) if self.games.iter().any(|g| g.name == c) => {}
            _ => {
                if let Some(first) = self.games.first().map(|g| g.name.clone()) {
                    self.select_game(&first, registry, index);
                }
            }
        }
    }

    /// `reset_form`
    pub fn reset_form(&mut self, registry: &Registry, index: &RouteIndex) {
        self.pkmn_filter.clear();
        self.min_battles_filter.clear();
        self.populate_game_table(registry);
        self.selected_game = None;
        self.selected_gen = None;
        self.current_gen_num = None;
        self.pkmn_list_cache.clear();
        if let Some(first) = self.games.first().map(|g| g.name.clone()) {
            self.select_game(&first, registry, index);
        } else {
            self.solo_selector.new_values(vec![consts::NO_POKEMON.to_string()], None);
            self.min_battles_selector.new_values(vec![consts::EMPTY_ROUTE_NAME.to_string()], None);
        }
    }

    /// `_on_game_selection_changed` + `_pkmn_version_callback`
    fn select_game(&mut self, new_game: &str, registry: &Registry, index: &RouteIndex) -> Option<String> {
        if self.selected_game.as_deref() == Some(new_game) {
            return None;
        }
        self.selected_game = Some(new_game.to_string());
        let gen = match registry.get_version(new_game) {
            Ok(g) => Some(g),
            Err(_) => {
                let _ = registry.reload_all_custom_gens();
                registry.get_version(new_game).ok()
            }
        };
        let Some(gen) = gen else {
            self.selected_gen = None;
            return Some(format!("Could not load game version '{}'. The base generation may not be available yet.", new_game));
        };
        let new_gen_num = gen.get_generation();
        let gen_changed = self.current_gen_num != Some(new_gen_num);
        self.current_gen_num = Some(new_gen_num);
        self.selected_gen = Some(gen.clone());
        self.update_pokemon_list();
        self.rebuild_base_routes(index);
        if gen_changed {
            let selected = self.solo_selector.get().to_string();
            let mon = if !selected.is_empty() && selected != consts::NO_POKEMON { gen.pkmn_db().get_pkmn(&selected).cloned() } else { None };
            self.dvs.config_for_target_game_and_mon(&gen, mon.as_deref(), None, None, None);
        }
        None
    }

    /// The "Base Route" choices for the selected game: the empty route, the
    /// gen's presets, then the saved routes of that version. The version
    /// comes from the route index, not from parsing every route file.
    fn rebuild_base_routes(&mut self, index: &RouteIndex) {
        let (Some(game), Some(gen)) = (self.selected_game.as_deref(), self.selected_gen.as_ref()) else { return };
        let mut all_routes = vec![consts::EMPTY_ROUTE_NAME.to_string()];
        for preset in &gen.min_battles_db().data {
            all_routes.push(format!("{}{}", consts::PRESET_ROUTE_PREFIX, preset));
        }
        let mut saved: Vec<String> = index.entries.values().filter(|e| e.version == game).map(|e| e.name.clone()).collect();
        saved.sort_by_key(|s| s.to_lowercase());
        all_routes.extend(saved);
        self.min_battles_cache = all_routes;
        self.base_routes_from_index = index.loaded;
        self.base_route_filter_callback();
    }

    fn update_pokemon_list(&mut self) {
        let (Some(game), Some(gen)) = (self.selected_game.clone(), self.selected_gen.clone()) else { return };
        let filter = self.pkmn_filter.trim().to_string();
        let key = (game, filter.clone());
        let list = match self.pkmn_list_cache.get(&key) {
            Some(l) => l.clone(),
            None => {
                let l = if filter.is_empty() { gen.pkmn_db().get_all_names(None) } else { gen.pkmn_db().get_filtered_names(Some(&filter), None) };
                self.pkmn_list_cache.insert(key, l.clone());
                l
            }
        };
        self.solo_selector.new_values(list, None);
    }

    /// `_pkmn_selector_callback`
    fn pkmn_selector_callback(&mut self) {
        let Some(gen) = self.selected_gen.clone() else { return };
        let selected = self.solo_selector.get().to_string();
        let mon = if !selected.is_empty() && selected != consts::NO_POKEMON { gen.pkmn_db().get_pkmn(&selected).cloned() } else { None };
        self.dvs.config_for_target_game_and_mon(&gen, mon.as_deref(), None, None, None);
    }

    fn base_route_filter_callback(&mut self) {
        let f = self.min_battles_filter.trim().to_lowercase();
        let mut vals: Vec<String> = self.min_battles_cache.iter().filter(|x| x.to_lowercase().contains(&f)).cloned().collect();
        if vals.is_empty() {
            vals = vec![consts::EMPTY_ROUTE_NAME.to_string()];
        }
        self.min_battles_selector.new_values(vals, None);
    }

    /// `create()`
    fn create_request(&self, paths: &Paths) -> Option<CreateRequest> {
        let game = self.selected_game.clone()?;
        let gen = self.selected_gen.clone()?;
        let selected = self.min_battles_selector.get().to_string();
        let base = if selected == consts::EMPTY_ROUTE_NAME {
            None
        } else if let Some(rest) = selected.strip_prefix(consts::PRESET_ROUTE_PREFIX) {
            Some(gen.min_battles_db().get_dir().join(format!("{}.json", rest)))
        } else {
            Some(io_utils::get_existing_route_path(paths, &selected))
        };
        let (dvs, ability_idx, nature) = self.dvs.get_dvs()?;
        Some(CreateRequest { solo_mon: self.solo_selector.get().to_string(), base_route_path: base, version: game, dvs, ability_idx, nature })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn ui(&mut self, ui: &mut Ui, theme: &Theme, registry: &Registry, paths: &Paths, index: &RouteIndex, assets: &mut Assets, actions: &mut NewRouteActions) {
        if self.games.is_empty() {
            self.populate_game_table(registry);
            if let Some(first) = self.games.first().map(|g| g.name.clone()) {
                if let Some(e) = self.select_game(&first, registry, index) {
                    actions.error = Some(e);
                }
            }
        }
        if !self.base_routes_from_index && index.loaded {
            self.rebuild_base_routes(index);
        }
        let _ = self.pending_game_load;
        let (enter, escape) = if behind_modal(ui) { (false, false) } else { ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape))) };
        if escape {
            actions.cancel = true;
        }
        let lbl_font = theme.font(11.0);
        let entry_font = theme.font(11.0);
        ui.vertical_centered(|ui| {
            ui.add_space(30.0);
            ui.label(egui::RichText::new("Create New Route").font(theme.font_bold(24.0)).color(theme.text));
            ui.add_space(10.0);
        });
        egui::Frame::new().inner_margin(egui::Margin { left: 100, right: 100, top: 0, bottom: 0 }).show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(10.0, 5.0);
            let label_w = 180.0;
            // ---- game table ----
            let page_w = ui.available_width();
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(Vec2::new(label_w, 20.0), egui::Layout::left_to_right(egui::Align::Min), |ui| {
                    ui.set_min_width(label_w);
                    ui.label(egui::RichText::new("Pokemon Version:").font(lbl_font.clone()).color(theme.text));
                });
                let table_h = (ui.available_height() * 0.45).max(100.0);
                // the scroll bar sits outside `total_w`; keep the table flush with the fields below
                let total_w = (page_w - label_w - 10.0).max(400.0) - 12.0;
                let widths = [84.0, 120.0, 120.0, 100.0, (total_w - 84.0 - 120.0 - 120.0 - 100.0).max(80.0)];
                let headers = ["Box Art", "Game", "Generation", "Platform", "Recorder"];
                egui::Frame::new().fill(theme.bg_input).stroke(Stroke::new(1.0_f32, theme.border)).show(ui, |ui| {
                    ui.set_width(total_w);
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    let (hrect, _) = ui.allocate_exact_size(Vec2::new(total_w, 22.0), Sense::hover());
                    let mut x = hrect.min.x;
                    for (i, h) in headers.iter().enumerate() {
                        let r = Rect::from_min_size(Pos2::new(x, hrect.min.y), Vec2::new(widths[i], 22.0));
                        widgets::paint_table_header_cell(ui.painter(), theme, r, h, i == 0);
                        x += widths[i];
                    }
                    let mut clicked: Option<String> = None;
                    widgets::show_scroll(ui, egui::ScrollArea::vertical().id_salt("game_table").max_height(table_h).auto_shrink([false, false]), |ui| {
                        for (row_idx, g) in self.games.clone().iter().enumerate() {
                            let (r, resp) = ui.allocate_exact_size(Vec2::new(total_w, 76.0), Sense::click());
                            let selected = self.selected_game.as_deref() == Some(g.name.as_str());
                            let fill = if selected {
                                theme.accent
                            } else if resp.hovered() {
                                theme.hover_bg
                            } else if row_idx % 2 == 1 {
                                theme.bg_lighter
                            } else {
                                theme.bg_input
                            };
                            ui.painter().rect_filled(r, CornerRadius::ZERO, fill);
                            if let Some(tex) = assets.box_art(ui.ctx(), &g.name) {
                                let [w, h] = tex.size();
                                let scale = (72.0 / w as f32).min(72.0 / h as f32);
                                let sz = Vec2::new(w as f32 * scale, h as f32 * scale);
                                let img = Rect::from_center_size(Pos2::new(r.min.x + 42.0, r.center().y), sz);
                                ui.painter().image(tex.id(), img, Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)), Color32::WHITE);
                            }
                            let color = if selected { Color32::WHITE } else { theme.text };
                            let cells = [g.name.as_str(), g.gen.as_str(), g.platform.as_str(), g.recorder.as_str()];
                            let mut x = r.min.x + widths[0];
                            for (i, c) in cells.iter().enumerate() {
                                ui.painter().text(Pos2::new(x + 4.0, r.center().y), Align2::LEFT_CENTER, *c, theme.body(), color);
                                x += widths[i + 1];
                            }
                            if resp.clicked() {
                                clicked = Some(g.name.clone());
                            }
                        }
                    });
                    if let Some(name) = clicked {
                        if let Some(e) = self.select_game(&name, registry, index) {
                            actions.error = Some(e);
                        }
                    }
                    });
                });
            });
            // ---- solo filter / selector ----
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(Vec2::new(label_w, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_min_width(label_w);
                    ui.label(egui::RichText::new("Solo Pokemon Filter:").font(lbl_font.clone()).color(theme.text));
                });
                let r = Entry::new(theme, &mut self.pkmn_filter).width((page_w - label_w - 10.0).max(400.0)).font(entry_font.clone()).id(ui.id().with("pkmn_filter")).show(ui);
                if r.changed {
                    self.update_pokemon_list();
                    self.pkmn_selector_callback();
                }
            });
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(Vec2::new(label_w, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_min_width(label_w);
                    ui.label(egui::RichText::new("Solo Pokemon:").font(lbl_font.clone()).color(theme.text));
                });
                if self.solo_selector.ui(ui, theme, ui.id().with("solo"), Some((page_w - label_w - 10.0).max(400.0)), true) {
                    self.pkmn_selector_callback();
                }
            });
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(Vec2::new(label_w, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_min_width(label_w);
                    ui.label(egui::RichText::new("Base Route Filter:").font(lbl_font.clone()).color(theme.text));
                });
                let r = Entry::new(theme, &mut self.min_battles_filter).width((page_w - label_w - 10.0).max(400.0)).font(entry_font.clone()).id(ui.id().with("base_filter")).show(ui);
                if r.changed {
                    self.base_route_filter_callback();
                }
            });
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(Vec2::new(label_w, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_min_width(label_w);
                    ui.label(egui::RichText::new("Base Route:").font(lbl_font.clone()).color(theme.text));
                });
                self.min_battles_selector.ui(ui, theme, ui.id().with("base_route"), Some((page_w - label_w - 10.0).max(400.0)), true);
            });
            ui.horizontal_top(|ui| {
                let est = if self.selected_gen.as_ref().map(|g| g.get_generation() > 2).unwrap_or(false) { 760.0 } else { 300.0 };
                ui.add_space(((page_w - est) / 2.0).max(0.0));
                self.dvs.ui(ui, theme);
            });
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("WARNING: Any unsaved changes in your current route\nwill be lost when creating a new route!").font(theme.font(10.0)).color(theme.failure));
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                // centred under the form, like the DV block and the warning above
                ui.add_space(((page_w - 370.0) / 2.0).max(0.0));
                if StyledButton::new(theme, "Create Route").fixed_width(180.0).show(ui).clicked() || enter {
                    if let Some(req) = self.create_request(paths) {
                        actions.create = Some(req);
                    }
                }
                if StyledButton::new(theme, "Cancel").fixed_width(180.0).show(ui).clicked() {
                    actions.cancel = true;
                }
            });
        });
    }
}
