//! The modal dialogs of `gui_qt/dialogs/*` plus the `QMessageBox` prompts
//! the window raises. One dialog is open at a time (a message box may sit on
//! top of it); each returns what the window should do when it closes.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

use egui::{Color32, CornerRadius, Stroke, Ui, Vec2};

use xpr_core::config::{DEFAULT_SHORTCUTS, SHORTCUT_CATEGORIES, SHORTCUT_LABELS};
use xpr_core::consts;
use xpr_core::io_utils;
use xpr_core::{Config, Paths};
use xpr_data::model::{Nature, StatBlock};
use xpr_data::{GenData, Registry};
use xpr_engine::NodeId;
use xpr_ui_kit::shortcuts::format_key_sequence;
use xpr_ui_kit::theme::{self, Theme};
use xpr_ui_kit::widgets::{self, Entry, StyledButton};

use crate::controller::MainController;
use crate::custom_dvs::CustomDvsFrame;
use crate::editors::OptionMenu;

/// Identifies which prompt a message-box answer belongs to.
#[derive(Clone, Debug, PartialEq)]
pub enum MsgTag {
    QuitUnsaved,
    NewRouteFromCurrentUnsaved,
    CloseRouteUnsaved,
    NoRouteName,
    DeleteEvents(Vec<NodeId>),
    UpdateFound(String, String),
    UpdateNoUpdate,
    UpdateNotPossible,
    UpdateError,
    ApplyUpdate,
    NewRouteError,
    Info,
    BackportResult { species: String, custom_gen_name: String },
    ResetAllShortcuts,
    DuplicateShortcuts,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MsgButtons {
    Ok,
    YesNo,
    YesNoCancel,
    OkCreateRoute,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MsgChoice {
    Yes,
    No,
    Cancel,
    Ok,
    Extra,
}

#[derive(Clone, Debug)]
pub struct MessageBox {
    pub title: String,
    pub text: String,
    pub buttons: MsgButtons,
    pub tag: MsgTag,
}

/// Effects a dialog asks the window to perform on close.
#[derive(Clone, Debug)]
pub enum DialogOutcome {
    LoadRoute(PathBuf),
    NewFolder { name: String, prev: Option<String>, insert_after: Option<NodeId> },
    Transfer { ids: Vec<NodeId>, folder: String },
    SetDvs(StatBlock, i64, Nature),
    RefreshEventList,
    RefreshTheme,
    RefreshBattle,
    ApplyShortcuts,
    AssignMove { slot: i64, mv: String },
    MatchupExport { idx: usize, mode: crate::battle_ui::ScreenshotMode },
    RestartForUpdate,
    CreateRouteForBackport { species: String, custom_gen_name: String },
    Message(MsgTag, MsgChoice),
    ReloadCustomGens,
    DataDirChanged,
}

pub enum Dialog {
    LoadRoute(LoadRouteDialog),
    NewFolder(NewFolderDialog),
    Transfer(TransferDialog),
    CustomDvs(CustomDvsDialog),
    CustomGen(CustomGenDialog),
    BattleConfig(BattleConfigDialog),
    ColorConfig(ColorConfigDialog),
    HighlightColors(HighlightColorDialog),
    AppConfig(AppConfigDialog),
    Shortcuts(ShortcutsDialog),
    FinalTrainers(FinalTrainersDialog),
    MatchupExport(MatchupExportDialog),
    AssignMove(AssignMoveDialog),
}

/// Shared context handed to dialog `ui` calls.
pub struct DialogCtx<'a> {
    pub theme: &'a Theme,
    pub cfg: &'a mut Config,
    pub ctrl: &'a mut MainController,
    pub registry: &'a Arc<Registry>,
    pub paths: &'a Paths,
}

/// Draw the modal frame. `content` returns true to close.
fn modal<R>(ctx: &egui::Context, theme: &Theme, id: &str, title: &str, min_width: f32, content: impl FnOnce(&mut Ui) -> R) -> R {
    let frame = egui::Frame::new()
        .fill(theme.bg)
        .stroke(Stroke::new(1.0_f32, theme.border))
        .corner_radius(CornerRadius::same(3))
        .inner_margin(egui::Margin::same(10));
    let m = egui::Modal::new(egui::Id::new(id)).frame(frame).backdrop_color(Color32::from_black_alpha(120));
    m.show(ctx, |ui| {
        ui.set_min_width(min_width);
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
        ui.label(egui::RichText::new(title).font(theme.font_bold(10.0)).color(theme.text));
        widgets::hline(ui, theme);
        content(ui)
    })
    .inner
}

fn enter_pressed(ui: &Ui) -> bool {
    ui.input(|i| i.key_pressed(egui::Key::Enter))
}

fn escape_pressed(ui: &Ui) -> bool {
    ui.input(|i| i.key_pressed(egui::Key::Escape))
}

// ---------------------------------------------------------------------------
// Message box
// ---------------------------------------------------------------------------

impl MessageBox {
    pub fn new(title: &str, text: &str, buttons: MsgButtons, tag: MsgTag) -> MessageBox {
        MessageBox { title: title.to_string(), text: text.to_string(), buttons, tag }
    }

    /// Returns the choice when a button was pressed.
    pub fn ui(&self, ctx: &egui::Context, theme: &Theme) -> Option<MsgChoice> {
        let mut choice = None;
        modal(ctx, theme, "xpr_message_box", &self.title, 320.0, |ui| {
            ui.add(egui::Label::new(egui::RichText::new(&self.text).font(theme.body()).color(theme.text)).wrap());
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                match self.buttons {
                    MsgButtons::Ok => {
                        if widgets::button(ui, theme, "OK").clicked() || enter_pressed(ui) {
                            choice = Some(MsgChoice::Ok);
                        }
                    }
                    MsgButtons::YesNo => {
                        if widgets::button(ui, theme, "Yes").clicked() {
                            choice = Some(MsgChoice::Yes);
                        }
                        if widgets::button(ui, theme, "No").clicked() || enter_pressed(ui) {
                            choice = Some(MsgChoice::No);
                        }
                    }
                    MsgButtons::YesNoCancel => {
                        if widgets::button(ui, theme, "Yes").clicked() {
                            choice = Some(MsgChoice::Yes);
                        }
                        if widgets::button(ui, theme, "No").clicked() {
                            choice = Some(MsgChoice::No);
                        }
                        if widgets::button(ui, theme, "Cancel").clicked() || enter_pressed(ui) {
                            choice = Some(MsgChoice::Cancel);
                        }
                    }
                    MsgButtons::OkCreateRoute => {
                        if widgets::button(ui, theme, "OK").clicked() || enter_pressed(ui) {
                            choice = Some(MsgChoice::Ok);
                        }
                        if widgets::button(ui, theme, "Create Route for Backport").clicked() {
                            choice = Some(MsgChoice::Extra);
                        }
                    }
                }
            });
            if escape_pressed(ui) {
                choice = Some(match self.buttons {
                    MsgButtons::YesNoCancel => MsgChoice::Cancel,
                    MsgButtons::YesNo => MsgChoice::No,
                    _ => MsgChoice::Ok,
                });
            }
        });
        choice
    }
}

// ---------------------------------------------------------------------------
// Load route
// ---------------------------------------------------------------------------

pub struct LoadRouteDialog {
    routes: OptionMenu,
    filter: String,
    show_backups: bool,
}

impl LoadRouteDialog {
    pub fn new(paths: &Paths) -> LoadRouteDialog {
        let mut d = LoadRouteDialog { routes: OptionMenu::new(vec![consts::NO_SAVED_ROUTES.to_string()], None), filter: String::new(), show_backups: false };
        d.filter_callback(paths);
        d
    }

    fn filter_callback(&mut self, paths: &Paths) {
        let mut all = io_utils::get_existing_route_names(paths, &self.filter, self.show_backups);
        if all.is_empty() {
            all = vec![consts::NO_SAVED_ROUTES.to_string()];
        }
        self.routes.new_values(all, None);
    }

    pub fn ui(&mut self, ctx: &egui::Context, d: &mut DialogCtx) -> (bool, Option<DialogOutcome>) {
        let theme = d.theme;
        let mut close = false;
        let mut outcome = None;
        let mut refilter = false;
        modal(ctx, theme, "xpr_load_route", "Load Route", 420.0, |ui| {
            egui::Grid::new("load_route_grid").spacing(Vec2::new(5.0, 5.0)).show(ui, |ui| {
                widgets::label(ui, theme, "Existing Routes:");
                self.routes.ui(ui, theme, ui.id().with("routes"), Some(260.0), true);
                ui.end_row();
                widgets::label(ui, theme, "Filter:");
                if Entry::new(theme, &mut self.filter).width(260.0).id(ui.id().with("filter")).show(ui).changed {
                    refilter = true;
                }
                ui.end_row();
                widgets::label(ui, theme, "Show Backup Routes?");
                if widgets::checkbox(ui, theme, &mut self.show_backups, "", true).changed() {
                    refilter = true;
                }
                ui.end_row();
            });
            ui.vertical_centered(|ui| {
                widgets::label(ui, theme, "Backup Routes are older versions of your route.\nEvery save makes a backup that is persisted, and can be reloaded if needed.\nThese are hidden by default because they can quickly pile up");
                widgets::label(ui, theme, "WARNING: Any unsaved changes in your current route\nwill be lost when loading an existing route!");
            });
            let selected = self.routes.get().to_string();
            let can_load = selected != consts::NO_SAVED_ROUTES;
            ui.horizontal(|ui| {
                if (widgets::button_enabled(ui, theme, "Load Route", can_load).clicked() || enter_pressed(ui)) && can_load {
                    outcome = Some(DialogOutcome::LoadRoute(io_utils::get_existing_route_path(d.paths, &selected)));
                    close = true;
                }
                if widgets::button(ui, theme, "Cancel").clicked() || escape_pressed(ui) {
                    close = true;
                }
            });
        });
        if refilter {
            self.filter_callback(d.paths);
        }
        (close, outcome)
    }
}

// ---------------------------------------------------------------------------
// New folder / rename
// ---------------------------------------------------------------------------

pub struct NewFolderDialog {
    folder_names: Vec<String>,
    prev_name: Option<String>,
    insert_after: Option<NodeId>,
    name: String,
    focused: bool,
}

impl NewFolderDialog {
    pub fn new(folder_names: Vec<String>, prev_name: Option<String>, insert_after: Option<NodeId>) -> NewFolderDialog {
        NewFolderDialog { folder_names, name: prev_name.clone().unwrap_or_default(), prev_name, insert_after, focused: false }
    }

    pub fn ui(&mut self, ctx: &egui::Context, d: &mut DialogCtx) -> (bool, Option<DialogOutcome>) {
        let theme = d.theme;
        let mut close = false;
        let mut outcome = None;
        let title = if self.prev_name.is_none() { "Create New Folder" } else { "Update Folder Name" };
        modal(ctx, theme, "xpr_new_folder", title, 360.0, |ui| {
            let valid = !self.name.is_empty() && !self.folder_names.contains(&self.name);
            egui::Grid::new("new_folder_grid").spacing(Vec2::new(10.0, 10.0)).show(ui, |ui| {
                widgets::label(ui, theme, if self.prev_name.is_none() { "New Folder Name" } else { "Update Folder Name" });
                let id = ui.id().with("folder_name");
                if !self.focused {
                    ui.memory_mut(|m| m.request_focus(id));
                    self.focused = true;
                }
                let r = Entry::new(theme, &mut self.name).width(200.0).id(id).show(ui);
                ui.end_row();
                let label = if self.prev_name.is_none() { "New Folder" } else { "Update Folder" };
                if (widgets::button_enabled(ui, theme, label, valid).clicked() || r.enter_pressed) && valid {
                    outcome = Some(DialogOutcome::NewFolder { name: self.name.clone(), prev: self.prev_name.clone(), insert_after: self.insert_after });
                    close = true;
                }
                if widgets::button(ui, theme, "Cancel").clicked() || escape_pressed(ui) {
                    close = true;
                }
                ui.end_row();
            });
        });
        (close, outcome)
    }
}

// ---------------------------------------------------------------------------
// Transfer events
// ---------------------------------------------------------------------------

pub struct TransferDialog {
    all_folders: Vec<String>,
    valid_dest: Vec<String>,
    ids: Vec<NodeId>,
    transfer_type: OptionMenu,
    new_folder: String,
    dest: OptionMenu,
    filter: String,
}

impl TransferDialog {
    pub fn new(all_folders: Vec<String>, valid_dest: Vec<String>, ids: Vec<NodeId>) -> TransferDialog {
        let mut d = TransferDialog {
            all_folders,
            valid_dest,
            ids,
            transfer_type: OptionMenu::new(vec![consts::TRANSFER_EXISTING_FOLDER.to_string(), consts::TRANSFER_NEW_FOLDER.to_string()], None),
            new_folder: String::new(),
            dest: OptionMenu::default(),
            filter: String::new(),
        };
        d.filter_callback();
        d
    }

    fn possible_folders(&self) -> Vec<String> {
        let f = self.filter.to_lowercase();
        let mut r: Vec<String> = self.valid_dest.iter().filter(|x| x.to_lowercase().contains(&f)).cloned().collect();
        if r.is_empty() {
            r = vec![consts::NO_FOLDERS.to_string()];
        }
        r
    }

    fn filter_callback(&mut self) {
        let vals = self.possible_folders();
        self.dest.new_values(vals, None);
    }

    pub fn ui(&mut self, ctx: &egui::Context, d: &mut DialogCtx) -> (bool, Option<DialogOutcome>) {
        let theme = d.theme;
        let mut close = false;
        let mut outcome = None;
        let mut refilter = false;
        modal(ctx, theme, "xpr_transfer", "Transfer Events", 380.0, |ui| {
            let existing = self.transfer_type.get() == consts::TRANSFER_EXISTING_FOLDER;
            let enabled = if existing { self.dest.get() != consts::NO_FOLDERS } else { !self.all_folders.contains(&self.new_folder) };
            egui::Grid::new("transfer_grid").spacing(Vec2::new(10.0, 10.0)).show(ui, |ui| {
                widgets::label(ui, theme, "Transfer to:");
                self.transfer_type.ui(ui, theme, ui.id().with("type"), Some(200.0), true);
                ui.end_row();
                if existing {
                    widgets::label(ui, theme, "Destination folder:");
                    self.dest.ui(ui, theme, ui.id().with("dest"), Some(200.0), true);
                    ui.end_row();
                    widgets::label(ui, theme, "Filter:");
                    if Entry::new(theme, &mut self.filter).width(200.0).id(ui.id().with("filter")).show(ui).changed {
                        refilter = true;
                    }
                    ui.end_row();
                } else {
                    widgets::label(ui, theme, "New folder:");
                    Entry::new(theme, &mut self.new_folder).width(200.0).id(ui.id().with("new_folder")).show(ui);
                    ui.end_row();
                }
                if (widgets::button_enabled(ui, theme, "Transfer to Folder", enabled).clicked() || enter_pressed(ui)) && enabled {
                    let folder = if existing { self.dest.get().to_string() } else { self.new_folder.trim().to_string() };
                    outcome = Some(DialogOutcome::Transfer { ids: self.ids.clone(), folder });
                    close = true;
                }
                if widgets::button(ui, theme, "Cancel").clicked() || escape_pressed(ui) {
                    close = true;
                }
                ui.end_row();
            });
        });
        if refilter {
            self.filter_callback();
        }
        (close, outcome)
    }
}

// ---------------------------------------------------------------------------
// Custom DVs
// ---------------------------------------------------------------------------

pub struct CustomDvsDialog {
    frame: CustomDvsFrame,
}

impl CustomDvsDialog {
    pub fn new(gen: &Arc<GenData>, ctrl: &MainController) -> CustomDvsDialog {
        let mut frame = CustomDvsFrame::new();
        let init = ctrl.get_init_state();
        let species = init.as_ref().map(|s| s.solo_pkmn.species_def.clone());
        frame.config_for_target_game_and_mon(gen, species.as_deref(), ctrl.get_dvs(), Some(ctrl.get_ability_idx()), Some(ctrl.get_nature()));
        CustomDvsDialog { frame }
    }

    pub fn ui(&mut self, ctx: &egui::Context, d: &mut DialogCtx) -> (bool, Option<DialogOutcome>) {
        let theme = d.theme;
        let mut close = false;
        let mut outcome = None;
        modal(ctx, theme, "xpr_custom_dvs", "Custom DVs/IVs", 420.0, |ui| {
            self.frame.ui(ui, theme);
            ui.horizontal(|ui| {
                if widgets::button(ui, theme, "Set New DVs").clicked() || enter_pressed(ui) {
                    if let Some((dvs, ability, nature)) = self.frame.get_dvs() {
                        outcome = Some(DialogOutcome::SetDvs(dvs, ability, nature));
                    }
                    close = true;
                }
                if widgets::button(ui, theme, "Cancel").clicked() || escape_pressed(ui) {
                    close = true;
                }
            });
        });
        (close, outcome)
    }
}

// ---------------------------------------------------------------------------
// Custom gens
// ---------------------------------------------------------------------------

pub struct CustomGenDialog {
    base_version: OptionMenu,
    custom_name: String,
    gens: Vec<(PathBuf, String, String)>,
    pending_backport: Option<BackportPrompt>,
}

struct BackportPrompt {
    custom_gen_path: PathBuf,
    custom_gen_name: String,
    backport_content: String,
    moves_content: Option<String>,
    species: String,
}

impl CustomGenDialog {
    pub fn new(registry: &Registry) -> CustomGenDialog {
        let mut d = CustomGenDialog {
            base_version: OptionMenu::new(registry.get_gen_names(true, false), None),
            custom_name: String::new(),
            gens: Vec::new(),
            pending_backport: None,
        };
        d.populate(registry);
        d
    }

    fn populate(&mut self, registry: &Registry) {
        self.gens = registry.get_all_custom_gen_info().into_iter().map(|i| (i.path, i.base_version, i.name)).collect();
    }

    fn verify_name(&self, registry: &Registry, name: &str) -> bool {
        !name.is_empty() && !registry.get_gen_names(true, true).iter().any(|n| n == name)
    }

    pub fn ui(&mut self, ctx: &egui::Context, d: &mut DialogCtx, message: &mut Option<MessageBox>) -> (bool, Option<DialogOutcome>) {
        let theme = d.theme;
        let mut close = false;
        let mut outcome = None;
        // species-name prompt for a backport import
        if let Some(bp) = self.pending_backport.as_mut() {
            let mut done: Option<bool> = None;
            modal(ctx, theme, "xpr_backport_species", "Species Name", 320.0, |ui| {
                widgets::label(ui, theme, "Enter the species name:");
                let r = Entry::new(theme, &mut bp.species).width(260.0).id(ui.id().with("species")).show(ui);
                ui.horizontal(|ui| {
                    if widgets::button(ui, theme, "OK").clicked() || r.enter_pressed {
                        done = Some(true);
                    }
                    if widgets::button(ui, theme, "Cancel").clicked() || escape_pressed(ui) {
                        done = Some(false);
                    }
                });
            });
            if let Some(ok) = done {
                let bp = self.pending_backport.take().unwrap();
                if ok && !bp.species.trim().is_empty() {
                    let species = bp.species.trim().to_string();
                    match xpr_data::backport::import_backport(&bp.custom_gen_path, &bp.backport_content, bp.moves_content.as_deref(), Some(&species)) {
                        Ok(mut result) => {
                            if let Err(e) = d.registry.reload_all_custom_gens() {
                                result.push_str(&format!("\n\nWarning during reload:\n{}", e));
                            }
                            self.populate(d.registry);
                            *message = Some(MessageBox::new("Import Successful", &result, MsgButtons::OkCreateRoute, MsgTag::BackportResult { species, custom_gen_name: bp.custom_gen_name }));
                        }
                        Err(e) => *message = Some(MessageBox::new("Import Error", &e, MsgButtons::Ok, MsgTag::Info)),
                    }
                }
            }
            return (false, None);
        }
        let mut open_path: Option<PathBuf> = None;
        let mut import_for: Option<(PathBuf, String)> = None;
        modal(ctx, theme, "xpr_custom_gen", "Custom Gen Manager", 520.0, |ui| {
            ui.vertical_centered(|ui| widgets::label(ui, theme, "Custom Gen Instructions:"));
            let instructions = "\nCreate a custom gen when you want to play a romhack, or a modified\nversion of an official game (e.g. hacking in a pokemon from a newer gen).\nA custom gen will re-use all the damage calculations and mechanics\nfrom the base official generation, but want to route with new or changed content.\n\nCreating a custom gen will copy all the information for the official gen\ninto a new folder. Once created, click on the button below to open that\nlocation, and then modify the files as necessary.\n\nThe application loads all custom gens on startup, so when you modify\nthe custom gen, you must restart the app before those changes will be recognized.\n\nNOTE: All custom gens are validated on app startup. If any errors are detected,\nyou will get a pop-up.  The app will still work fine,\nbut any custom gens with errors detected will not be loaded\n";
            widgets::label(ui, theme, instructions);
            let valid = self.verify_name(d.registry, &self.custom_name);
            egui::Grid::new("custom_gen_grid").spacing(Vec2::new(5.0, 5.0)).show(ui, |ui| {
                widgets::label(ui, theme, "Base Version:");
                self.base_version.ui(ui, theme, ui.id().with("base"), Some(200.0), true);
                ui.end_row();
                widgets::label(ui, theme, "Custom Version Name:");
                Entry::new(theme, &mut self.custom_name).width(200.0).id(ui.id().with("custom_name")).show(ui);
                ui.end_row();
            });
            if (widgets::button_enabled(ui, theme, "Create Custom Version", valid).clicked() || enter_pressed(ui)) && valid {
                let base = self.base_version.get().to_string();
                let name = self.custom_name.clone();
                d.ctrl.create_custom_version(&base, &name);
                self.populate(d.registry);
            }
            ui.add_space(10.0);
            ui.vertical_centered(|ui| widgets::label(ui, theme, "All Custom Gens"));
            egui::Grid::new("custom_gens_list").spacing(Vec2::new(5.0, 5.0)).show(ui, |ui| {
                for (path, base, name) in self.gens.clone() {
                    widgets::label(ui, theme, format!("{} ({})", name, base));
                    if widgets::button(ui, theme, "Open Custom Gen").clicked() {
                        open_path = Some(path.clone());
                    }
                    if widgets::button(ui, theme, "Import Backport").clicked() {
                        import_for = Some((path.clone(), name.clone()));
                    }
                    ui.end_row();
                }
            });
            ui.vertical_centered(|ui| {
                if widgets::button(ui, theme, "Close").clicked() || escape_pressed(ui) {
                    close = true;
                }
            });
        });
        if let Some(p) = open_path {
            io_utils::open_explorer(&p);
        }
        if let Some((path, name)) = import_for {
            let backport = rfd::FileDialog::new().set_title("Select backports.asm").add_filter("ASM Files", &["asm"]).add_filter("All Files", &["*"]).pick_file();
            if let Some(bp_path) = backport {
                let moves = rfd::FileDialog::new().set_title("Select backport_moves.asm (Cancel to skip)").add_filter("ASM Files", &["asm"]).add_filter("All Files", &["*"]).pick_file();
                match std::fs::read_to_string(&bp_path) {
                    Ok(backport_content) => {
                        let moves_content = moves.and_then(|m| std::fs::read_to_string(m).ok());
                        let default_name = xpr_data::backport::get_backport_name(&backport_content).unwrap_or_default();
                        self.pending_backport = Some(BackportPrompt { custom_gen_path: path, custom_gen_name: name, backport_content, moves_content, species: default_name });
                    }
                    Err(e) => *message = Some(MessageBox::new("Import Error", &format!("Unexpected error: {}", e), MsgButtons::Ok, MsgTag::Info)),
                }
            }
        }
        if close {
            outcome = Some(DialogOutcome::ReloadCustomGens);
        }
        (close, outcome)
    }
}

// ---------------------------------------------------------------------------
// Battle config
// ---------------------------------------------------------------------------

pub struct BattleConfigDialog {
    search_depth: String,
    force_full_search: bool,
    ignore_accuracy: bool,
    player_strat: OptionMenu,
    enemy_strat: OptionMenu,
    consistent_threshold: String,
}

impl BattleConfigDialog {
    pub fn new(cfg: &Config) -> BattleConfigDialog {
        let strats: Vec<String> = consts::ALL_HIGHLIGHT_STRATS.iter().map(|s| s.to_string()).collect();
        BattleConfigDialog {
            search_depth: cfg.get_damage_search_depth().to_string(),
            force_full_search: cfg.do_force_full_search(),
            ignore_accuracy: cfg.do_ignore_accuracy(),
            player_strat: OptionMenu::new(strats.clone(), Some(cfg.get_player_highlight_strategy())),
            enemy_strat: OptionMenu::new(strats, Some(cfg.get_enemy_highlight_strategy())),
            consistent_threshold: cfg.get_consistent_threshold().to_string(),
        }
    }

    pub fn ui(&mut self, ctx: &egui::Context, d: &mut DialogCtx) -> (bool, Option<DialogOutcome>) {
        let theme = d.theme;
        let mut close = false;
        modal(ctx, theme, "xpr_battle_config", "Battle Configuration", 600.0, |ui| {
            egui::ScrollArea::vertical().max_height(500.0).show(ui, |ui| {
                ui.vertical_centered(|ui| widgets::label(ui, theme, "Battle calcs limitations and edge cases"));
                ui.add(egui::Label::new(egui::RichText::new(
                    "Possible kills with less than 0.1% chance are not reported.\nFor each move, a full search for kill percents is done, up to a certain # of turns (configurable below).\nUp to 3 ranges are reported: 2 fastest kills, if applicable. The number required for a guaranteed kill is always given as the last range\nFor weaker moves, the maximum number of HITS (assumes attack lands every time) needed to guarantee a kill are given instead\nWith respect to accuracy calculations, Gen 1 misses are ignored\nThe crit damage ranges for multi-hit moves in Gen 2 assume exactly one of the multi-hits crit",
                ).font(theme.body()).color(theme.text)).wrap());
                ui.add_space(10.0);
                egui::Grid::new("battle_cfg_grid").spacing(Vec2::new(5.0, 5.0)).show(ui, |ui| {
                    widgets::label(ui, theme, "Damage Calc Search Depth:");
                    if widgets::AmountEntry::new(theme, ui.id().with("depth"), &mut self.search_depth).min(Some(1)).max(Some(99)).show(ui).changed {
                        let v = self.search_depth.trim().parse::<i64>().unwrap_or(xpr_core::config::DEFAULT_DAMAGE_SEARCH_DEPTH);
                        d.cfg.set_damage_search_depth(v);
                    }
                    ui.end_row();
                    widgets::label(ui, theme, "\n# Of turns to search damage ranges to find kill %'s\nLarger gets more accurate guaranteed kills, but may take longer, especially on slower computers");
                    ui.end_row();
                    if widgets::checkbox_label(ui, theme, &mut self.force_full_search, "Fully calculate psywave (Not recommended):", true, true) {
                        d.cfg.set_force_full_search(self.force_full_search);
                    }
                    ui.end_row();
                    if widgets::checkbox_label(ui, theme, &mut self.ignore_accuracy, "Ignore Accuracy in Kill Ranges:", true, true) {
                        d.cfg.set_ignore_accuracy(self.ignore_accuracy);
                    }
                    ui.end_row();
                    widgets::label(ui, theme, "Player Highlight Strategy:");
                    if self.player_strat.ui(ui, theme, ui.id().with("pstrat"), Some(160.0), true) {
                        d.cfg.set_player_highlight_strategy(self.player_strat.get());
                    }
                    ui.end_row();
                    widgets::label(ui, theme, "Enemy Highlight Strategy:");
                    if self.enemy_strat.ui(ui, theme, ui.id().with("estrat"), Some(160.0), true) {
                        d.cfg.set_enemy_highlight_strategy(self.enemy_strat.get());
                    }
                    ui.end_row();
                    widgets::label(ui, theme, "Consistency Threshold:");
                    if widgets::AmountEntry::new(theme, ui.id().with("thresh"), &mut self.consistent_threshold).min(Some(1)).max(Some(99)).show(ui).changed {
                        let v = self.consistent_threshold.trim().parse::<i64>().unwrap_or(xpr_core::config::DEFAULT_CONSISTENT_THRESHOLD);
                        d.cfg.set_consistent_threshold(v);
                    }
                    ui.end_row();
                });
                ui.add_space(10.0);
                ui.vertical_centered(|ui| {
                    widgets::label(ui, theme, "Highlighting Strategies");
                    widgets::label(ui, theme, "Guaranteed Kill");
                });
                widgets::label(ui, theme, "This will highlight the move that has the lowest number of turns for a 'guaranteed' kill.\n'Guaranteed' is if the move has a 99% chance or higher to kill.");
                ui.vertical_centered(|ui| widgets::label(ui, theme, "Fastest Kill"));
                widgets::label(ui, theme, "This will highlight the move that has the lowest number of turns for any possible kill.\nKills that have a less than 0.1% chance of occuring are ignored by the damage calcs, but if the move has at least a 1% chance of killing,\nit will be reported");
                ui.vertical_centered(|ui| widgets::label(ui, theme, "Consistent Kill"));
                widgets::label(ui, theme, "This will highlight the move that has the lowest number of turns for a kill that has at least a chance above the consistency threshold.\nThis can be configured to suit your preferences");
                ui.add(egui::Label::new(egui::RichText::new("Regardless of the strategy, ties between moves with the same # of turns will be broken with successive checks for the following stats: Highest Accuracy, Punish 2 turn moves (dig/fly), Highest damage").font(theme.body()).color(theme.text)).wrap());
                ui.add(egui::Label::new(egui::RichText::new("Hyper Beam is also special cased to not be highlighted if it cannot kill with a single non-crit hit, due to the need to recharge").font(theme.body()).color(theme.text)).wrap());
            });
            ui.vertical_centered(|ui| {
                if widgets::button(ui, theme, "Close").clicked() || enter_pressed(ui) || escape_pressed(ui) {
                    close = true;
                }
            });
        });
        (close, if close { Some(DialogOutcome::RefreshBattle) } else { None })
    }
}

// ---------------------------------------------------------------------------
// Colour helpers shared by the colour dialogs
// ---------------------------------------------------------------------------

/// `ConfigColorUpdater`: label, "Change Color" (an in-app picker), preview.
/// Returns the new hex when the colour changed.
fn color_updater(ui: &mut Ui, theme: &Theme, label: &str, current_hex: &str, enabled: bool) -> Option<String> {
    let mut result = None;
    ui.horizontal(|ui| {
        widgets::label(ui, theme, label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mut color = theme::parse_hex(current_hex);
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(20.0), egui::Sense::hover());
            ui.painter().rect(rect, CornerRadius::ZERO, color, Stroke::new(1.0_f32, Color32::from_rgb(0x55, 0x55, 0x55)), egui::StrokeKind::Inside);
            ui.add_space(6.0);
            ui.add_enabled_ui(enabled, |ui| {
                let before = color;
                egui::color_picker::color_edit_button_srgba(ui, &mut color, egui::color_picker::Alpha::Opaque);
                if color != before {
                    result = Some(theme::to_hex(color));
                }
                widgets::label(ui, theme, "Change Color");
            });
        });
    });
    result
}

// ---------------------------------------------------------------------------
// Font & colour config
// ---------------------------------------------------------------------------

pub struct ColorConfigDialog {
    fonts: OptionMenu,
    changed: bool,
}

impl ColorConfigDialog {
    pub fn new(cfg: &Config) -> ColorConfigDialog {
        let mut fonts = system_font_families();
        let cur = cfg.get_custom_font_name();
        if !fonts.contains(&cur) {
            fonts.push(cur.clone());
        }
        fonts.sort();
        ColorConfigDialog { fonts: OptionMenu::new(fonts, Some(&cur)), changed: false }
    }

    pub fn ui(&mut self, ctx: &egui::Context, d: &mut DialogCtx) -> (bool, Option<DialogOutcome>) {
        let theme = d.theme;
        let mut close = false;
        modal(ctx, theme, "xpr_color_config", "Font & Color Configuration", 420.0, |ui| {
            egui::ScrollArea::vertical().max_height(600.0).show(ui, |ui| {
                egui::Grid::new("font_grid").spacing(Vec2::new(5.0, 3.0)).show(ui, |ui| {
                    widgets::label(ui, theme, "Font Name:");
                    self.fonts.ui(ui, theme, ui.id().with("font"), Some(200.0), true);
                    ui.end_row();
                });
                if widgets::button(ui, theme, "Set Font Name").clicked() {
                    d.cfg.set_custom_font_name(self.fonts.get());
                    self.changed = true;
                }
                ui.vertical_centered(|ui| widgets::label(ui, theme, "If your custom font is not present in the list\nMake sure that it is installed on your system\nAnd then restart the program"));
                ui.add_space(8.0);
                widgets::label(ui, theme, "Color Config:");
                if widgets::button(ui, theme, "Reset all colors").clicked() {
                    d.cfg.reset_all_colors();
                    self.changed = true;
                }
                let items: [(&str, String); 10] = [
                    ("Success Color:", d.cfg.get_success_color().to_string()),
                    ("Warning Color:", d.cfg.get_warning_color().to_string()),
                    ("Failure Color:", d.cfg.get_failure_color().to_string()),
                    ("Divider Color:", d.cfg.get_divider_color().to_string()),
                    ("Header Color:", d.cfg.get_header_color().to_string()),
                    ("Primary Color:", d.cfg.get_primary_color().to_string()),
                    ("Secondary Color:", d.cfg.get_secondary_color().to_string()),
                    ("Contrast Color:", d.cfg.get_contrast_color().to_string()),
                    ("Background Color:", d.cfg.get_background_color().to_string()),
                    ("Text Color:", d.cfg.get_text_color().to_string()),
                ];
                for (i, (label, cur)) in items.iter().enumerate() {
                    if let Some(new_hex) = color_updater(ui, theme, label, cur, true) {
                        match i {
                            0 => d.cfg.set_success_color(&new_hex),
                            1 => d.cfg.set_warning_color(&new_hex),
                            2 => d.cfg.set_failure_color(&new_hex),
                            3 => d.cfg.set_divider_color(&new_hex),
                            4 => d.cfg.set_header_color(&new_hex),
                            5 => d.cfg.set_primary_color(&new_hex),
                            6 => d.cfg.set_secondary_color(&new_hex),
                            7 => d.cfg.set_contrast_color(&new_hex),
                            8 => d.cfg.set_background_color(&new_hex),
                            _ => d.cfg.set_text_color(&new_hex),
                        }
                        self.changed = true;
                    }
                }
                ui.vertical_centered(|ui| widgets::label(ui, theme, "After changing colors, you must restart the program\nbefore color changes will take effect"));
            });
            ui.vertical_centered(|ui| {
                if widgets::button(ui, theme, "Close").clicked() || escape_pressed(ui) {
                    close = true;
                }
            });
        });
        (close, if close { Some(DialogOutcome::RefreshTheme) } else { None })
    }
}

/// The font families available (font file stems of the system font folders).
fn system_font_families() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut dirs: Vec<PathBuf> = Vec::new();
    #[cfg(target_os = "windows")]
    {
        if let Some(w) = std::env::var_os("WINDIR") {
            dirs.push(PathBuf::from(w).join("Fonts"));
        }
    }
    #[cfg(target_os = "macos")]
    {
        dirs.push("/System/Library/Fonts".into());
        dirs.push("/Library/Fonts".into());
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        dirs.push("/usr/share/fonts".into());
    }
    for d in dirs {
        if let Ok(rd) = std::fs::read_dir(&d) {
            for e in rd.flatten() {
                let p = e.path();
                let ext = p.extension().and_then(|x| x.to_str()).map(|x| x.to_lowercase()).unwrap_or_default();
                if ext == "ttf" || ext == "otf" || ext == "ttc" {
                    if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                        out.push(stem.to_string());
                    }
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

// ---------------------------------------------------------------------------
// Highlight colour config
// ---------------------------------------------------------------------------

const FIGHT_CATEGORY_LABELS: [(&str, &str); 7] = [
    ("rival", "Rival:"),
    ("gym_leader", "Gym Leader:"),
    ("elite_four", "Elite Four:"),
    ("champion", "Champion:"),
    ("post_game", "Post-Game:"),
    ("boss", "Boss:"),
    ("team_leader", "Team Leader:"),
];

pub struct HighlightColorDialog {
    changed: bool,
}

impl HighlightColorDialog {
    pub fn new() -> HighlightColorDialog {
        HighlightColorDialog { changed: false }
    }

    pub fn ui(&mut self, ctx: &egui::Context, d: &mut DialogCtx) -> (bool, Option<DialogOutcome>) {
        let theme = d.theme;
        let mut close = false;
        let mut refresh = false;
        modal(ctx, theme, "xpr_highlight_colors", "Configure Highlight Colors", 420.0, |ui| {
            egui::ScrollArea::vertical().max_height(650.0).show(ui, |ui| {
                ui.vertical_centered(|ui| ui.label(egui::RichText::new("Highlight Color Configuration:").font(theme.font_bold(12.0)).color(theme.text)));
                for i in 1..=9i64 {
                    let cur = d.cfg.get_highlight_color(i);
                    if let Some(h) = color_updater(ui, theme, &format!("Highlight {}:", i), &cur, true) {
                        d.cfg.set_highlight_color(i, &h);
                        refresh = true;
                    }
                }
                ui.vertical_centered(|ui| ui.label(egui::RichText::new("Fight Category Colors:").font(theme.font_bold(12.0)).color(theme.text)));
                let mut color_major = d.cfg.get_color_major_battles();
                if widgets::checkbox(ui, theme, &mut color_major, "Color Major Battles", true).changed() {
                    d.cfg.set_color_major_battles(color_major);
                    refresh = true;
                }
                for (cat, label) in FIGHT_CATEGORY_LABELS {
                    let cur = d.cfg.get_fight_category_color(cat);
                    if let Some(h) = color_updater(ui, theme, label, &cur, color_major) {
                        d.cfg.set_fight_category_color(cat, &h);
                        refresh = true;
                    }
                }
            });
            ui.add_space(15.0);
            ui.horizontal(|ui| {
                if widgets::button(ui, theme, "Reset to Defaults").clicked() {
                    d.cfg.reset_highlight_colors_to_defaults();
                    d.cfg.reset_fight_category_colors();
                    refresh = true;
                }
                if widgets::button(ui, theme, "Close").clicked() || escape_pressed(ui) {
                    close = true;
                }
            });
        });
        if refresh {
            self.changed = true;
        }
        (close, if refresh || close { Some(DialogOutcome::RefreshEventList) } else { None })
    }
}

// ---------------------------------------------------------------------------
// App config (data dir / updates)
// ---------------------------------------------------------------------------

pub struct AppConfigDialog {
    first_time_setup: bool,
    data_dir_changed: bool,
    debug_mode: bool,
    latest_version_text: String,
    upgrade_available: Option<(String, String)>,
    rx: Option<Receiver<xpr_update::ReleaseInfo>>,
}

impl AppConfigDialog {
    pub fn new(cfg: &Config, first_time_setup: bool) -> AppConfigDialog {
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let info = xpr_update::get_new_version_info();
            let _ = tx.send(info);
        });
        AppConfigDialog { first_time_setup, data_dir_changed: false, debug_mode: cfg.is_debug_mode(), latest_version_text: "Fetching newest version...".into(), upgrade_available: None, rx: Some(rx) }
    }

    pub fn ui(&mut self, ctx: &egui::Context, d: &mut DialogCtx, message: &mut Option<MessageBox>) -> (bool, Option<DialogOutcome>) {
        let theme = d.theme;
        if let Some(rx) = &self.rx {
            if let Ok(info) = rx.try_recv() {
                match &info.tag_name {
                    Some(v) => {
                        self.latest_version_text = format!("Newest Version: {}", v);
                        if xpr_update::is_upgrade_needed(Some(v), consts::APP_VERSION) {
                            self.upgrade_available = Some((v.clone(), info.asset_url.clone().unwrap_or_default()));
                        }
                    }
                    None => self.latest_version_text = "Failed to fetch version info".into(),
                }
                self.rx = None;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
            }
        }
        let mut close = false;
        let mut outcome = None;
        let mut open_config = false;
        let mut open_data = false;
        let mut open_images = false;
        let mut move_data = false;
        let mut move_images = false;
        modal(ctx, theme, "xpr_app_config", "Application Settings", 480.0, |ui| {
            egui::Grid::new("app_cfg_grid").spacing(Vec2::new(5.0, 5.0)).show(ui, |ui| {
                widgets::label(ui, theme, "App Version:");
                widgets::label(ui, theme, consts::APP_VERSION);
                ui.end_row();
                widgets::label(ui, theme, "Release Date:");
                widgets::label(ui, theme, consts::APP_RELEASE_DATE);
                ui.end_row();
                widgets::label(ui, theme, "Debug Logging when Recording:");
                if widgets::checkbox(ui, theme, &mut self.debug_mode, "", true).changed() {
                    d.cfg.set_debug_mode(self.debug_mode);
                }
                ui.end_row();
            });
            ui.vertical_centered(|ui| {
                widgets::label(ui, theme, "Automatic updates only supported on windows machines");
                widgets::label(ui, theme, self.latest_version_text.clone());
                let (label, enabled) = match &self.upgrade_available {
                    Some(_) => ("Upgrade", true),
                    None => ("No Upgrade Needed", false),
                };
                if widgets::button_enabled(ui, theme, label, enabled).clicked() {
                    outcome = Some(DialogOutcome::RestartForUpdate);
                    close = true;
                }
            });
            ui.add_space(10.0);
            widgets::label(ui, theme, format!("Data Location: {}", d.cfg.get_user_data_dir().display()));
            ui.horizontal(|ui| {
                if widgets::button(ui, theme, "Open Data Folder").clicked() {
                    open_data = true;
                }
                if widgets::button(ui, theme, "Move Data Location").clicked() {
                    move_data = true;
                }
            });
            widgets::label(ui, theme, format!("Image Location: {}", d.cfg.get_images_dir().display()));
            ui.horizontal(|ui| {
                if widgets::button(ui, theme, "Open Images Folder").clicked() {
                    open_images = true;
                }
                if widgets::button(ui, theme, "Move Images Location").clicked() {
                    move_images = true;
                }
            });
            if widgets::button(ui, theme, "Open Config/Logs Folder").clicked() {
                open_config = true;
            }
            ui.vertical_centered(|ui| {
                if widgets::button(ui, theme, "Close").clicked() || enter_pressed(ui) || escape_pressed(ui) {
                    close = true;
                }
            });
        });
        if open_config {
            io_utils::open_explorer(&d.paths.global_config_dir);
        }
        if open_data {
            io_utils::open_explorer(&d.cfg.get_user_data_dir());
        }
        if open_images {
            io_utils::open_explorer(&d.cfg.get_images_dir());
        }
        if move_data {
            let init = d.cfg.get_user_data_dir();
            if let Some(new_path) = pick_location(&init, d.paths, message) {
                if io_utils::change_user_data_location(d.paths, Some(&init), &new_path) {
                    d.cfg.set_user_data_dir(&new_path.to_string_lossy());
                    self.data_dir_changed = true;
                    outcome = Some(DialogOutcome::DataDirChanged);
                } else {
                    *message = Some(MessageBox::new("Error", "Failed to change user data location...", MsgButtons::Ok, MsgTag::Info));
                }
            }
        }
        if move_images {
            let init = d.cfg.get_images_dir();
            if let Some(new_path) = pick_location(&init, d.paths, message) {
                if io_utils::migrate_dir(&init, &new_path) {
                    d.cfg.set_images_dir(&new_path.to_string_lossy());
                } else {
                    *message = Some(MessageBox::new("Error", "Failed to change image location...", MsgButtons::Ok, MsgTag::Info));
                }
            }
        }
        if close && self.first_time_setup && !self.data_dir_changed {
            io_utils::change_user_data_location(d.paths, None, &d.cfg.get_user_data_dir());
        }
        (close, outcome)
    }
}

/// `_change_location_helper`: a folder picker that refuses the app folder.
fn pick_location(init_dir: &Path, paths: &Paths, message: &mut Option<MessageBox>) -> Option<PathBuf> {
    let picked = rfd::FileDialog::new().set_title("Select Directory").set_directory(init_dir).pick_folder()?;
    let new_path = std::fs::canonicalize(&picked).unwrap_or(picked);
    let root = std::fs::canonicalize(&paths.source_root).unwrap_or(paths.source_root.clone());
    if new_path.starts_with(&root) {
        *message = Some(MessageBox::new("Error", "Cannot place the dir inside the app, as it will be removed during automatic updates", MsgButtons::Ok, MsgTag::Info));
        return None;
    }
    Some(new_path)
}

// ---------------------------------------------------------------------------
// Keyboard shortcuts
// ---------------------------------------------------------------------------

pub struct ShortcutsDialog {
    search: String,
    editors: Vec<(String, String)>, // action_id -> current sequence text
    pending: Vec<(String, String)>,
    capturing: Option<String>,
}

impl ShortcutsDialog {
    pub fn new(cfg: &Config) -> ShortcutsDialog {
        let editors = DEFAULT_SHORTCUTS.iter().map(|(id, _)| (id.to_string(), cfg.get_shortcut(id))).collect();
        ShortcutsDialog { search: String::new(), editors, pending: Vec::new(), capturing: None }
    }

    fn seq_of(&self, id: &str) -> String {
        self.editors.iter().find(|(a, _)| a == id).map(|(_, s)| s.clone()).unwrap_or_default()
    }

    fn set_seq(&mut self, id: &str, seq: &str) {
        if let Some(e) = self.editors.iter_mut().find(|(a, _)| a == id) {
            e.1 = seq.to_string();
        }
    }

    fn apply(&mut self, cfg: &mut Config, message: &mut Option<MessageBox>, force: bool) -> bool {
        if self.pending.is_empty() {
            return false;
        }
        if !force {
            let mut all = cfg.get_all_shortcuts();
            for (a, s) in &self.pending {
                all.insert(a.clone(), s.clone());
            }
            let mut reverse: std::collections::HashMap<String, String> = std::collections::HashMap::new();
            let mut dups: Vec<String> = Vec::new();
            for (aid, seq) in all.iter() {
                if seq.is_empty() {
                    continue;
                }
                if let Some(other) = reverse.get(seq) {
                    dups.push(format!("  {}: \"{}\" and \"{}\"", seq, xpr_core::config::shortcut_label(other), xpr_core::config::shortcut_label(aid)));
                } else {
                    reverse.insert(seq.clone(), aid.clone());
                }
            }
            if !dups.is_empty() {
                *message = Some(MessageBox::new(
                    "Duplicate Shortcuts",
                    &format!("The following key sequences are assigned to multiple actions:\n\n{}\n\nApply anyway?", dups.join("\n")),
                    MsgButtons::YesNo,
                    MsgTag::DuplicateShortcuts,
                ));
                return false;
            }
        }
        for (a, s) in self.pending.drain(..) {
            cfg.set_shortcut(&a, &s);
        }
        true
    }

    /// Called by the window when the duplicate prompt was answered "Yes".
    pub fn force_apply(&mut self, cfg: &mut Config) -> bool {
        let mut none = None;
        self.apply(cfg, &mut none, true)
    }

    pub fn ui(&mut self, ctx: &egui::Context, d: &mut DialogCtx, message: &mut Option<MessageBox>) -> (bool, Option<DialogOutcome>) {
        let theme = d.theme;
        let mut close = false;
        let mut outcome = None;
        // key capture
        if let Some(cap) = self.capturing.clone() {
            let captured = ctx.input(|i| {
                for ev in &i.events {
                    if let egui::Event::Key { key, pressed: true, modifiers, .. } = ev {
                        let sc = egui::KeyboardShortcut::new(*modifiers, *key);
                        return Some(format_key_sequence(&sc));
                    }
                }
                None
            });
            if let Some(seq) = captured {
                self.set_seq(&cap, &seq);
                self.pending.retain(|(a, _)| *a != cap);
                self.pending.push((cap.clone(), seq));
                self.capturing = None;
            }
        }
        modal(ctx, theme, "xpr_shortcuts", "Keyboard Shortcuts", 660.0, |ui| {
            ui.horizontal(|ui| {
                widgets::label(ui, theme, "Search:");
                Entry::new(theme, &mut self.search).width(300.0).hint("Filter shortcuts...").id(ui.id().with("search")).show(ui);
            });
            let needle = self.search.to_lowercase();
            egui::ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
                egui::Grid::new("shortcuts_grid").spacing(Vec2::new(6.0, 4.0)).striped(true).show(ui, |ui| {
                    widgets::label_bold(ui, theme, "Action");
                    widgets::label_bold(ui, theme, "Shortcut");
                    widgets::label_bold(ui, theme, "Default");
                    widgets::label_bold(ui, theme, "");
                    ui.end_row();
                    for (category, ids) in SHORTCUT_CATEGORIES {
                        let visible: Vec<&&str> = ids
                            .iter()
                            .filter(|id| {
                                let label = xpr_core::config::shortcut_label(id).to_lowercase();
                                let seq = self.seq_of(id).to_lowercase();
                                needle.is_empty() || label.contains(&needle) || seq.contains(&needle)
                            })
                            .collect();
                        if visible.is_empty() {
                            continue;
                        }
                        widgets::label_bold(ui, theme, *category);
                        ui.end_row();
                        for id in visible {
                            let label = xpr_core::config::shortcut_label(id);
                            let default_seq = xpr_core::config::default_shortcut(id).to_string();
                            let cur = self.seq_of(id);
                            let is_custom = cur != default_seq;
                            let color = if is_custom { Color32::from_rgb(0x00, 0xff, 0xff) } else { theme.text };
                            widgets::label_colored(ui, theme, format!("    {}", label), color);
                            let capturing = self.capturing.as_deref() == Some(id);
                            let shown = if capturing { "...".to_string() } else { cur.clone() };
                            let (rect, resp) = ui.allocate_exact_size(Vec2::new(160.0, 22.0), egui::Sense::click());
                            let fill = if capturing { Color32::from_rgb(0x3a, 0x5a, 0x8a) } else { theme.bg_input };
                            ui.painter().rect(rect, CornerRadius::same(2), fill, Stroke::new(1.0_f32, theme.border), egui::StrokeKind::Inside);
                            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, shown, theme.body(), color);
                            if resp.clicked() {
                                self.capturing = Some(id.to_string());
                            }
                            widgets::label_colored(ui, theme, default_seq.clone(), color);
                            if StyledButton::new(theme, "Reset").fixed_width(52.0).show(ui).clicked() {
                                d.cfg.reset_shortcut(id);
                                self.pending.retain(|(a, _)| a != id);
                                self.set_seq(id, &default_seq);
                                outcome = Some(DialogOutcome::ApplyShortcuts);
                            }
                            ui.end_row();
                        }
                    }
                });
            });
            ui.horizontal(|ui| {
                if widgets::button(ui, theme, "Apply").clicked() && self.apply(d.cfg, message, false) {
                    outcome = Some(DialogOutcome::ApplyShortcuts);
                }
                if widgets::button(ui, theme, "Reset All to Defaults").clicked() {
                    *message = Some(MessageBox::new("Reset All Shortcuts", "Reset all keyboard shortcuts to their defaults?", MsgButtons::YesNo, MsgTag::ResetAllShortcuts));
                }
            });
            ui.horizontal(|ui| {
                if widgets::button(ui, theme, "Export Profile...").clicked() {
                    if let Some(p) = rfd::FileDialog::new().set_title("Export Shortcut Profile").set_file_name("shortcuts.json").add_filter("JSON Files", &["json"]).save_file() {
                        match d.cfg.export_shortcuts(&p) {
                            Ok(_) => *message = Some(MessageBox::new("Export", &format!("Shortcuts exported to:\n{}", p.display()), MsgButtons::Ok, MsgTag::Info)),
                            Err(e) => *message = Some(MessageBox::new("Export Error", &e.to_string(), MsgButtons::Ok, MsgTag::Info)),
                        }
                    }
                }
                if widgets::button(ui, theme, "Import Profile...").clicked() {
                    if let Some(p) = rfd::FileDialog::new().set_title("Import Shortcut Profile").add_filter("JSON Files", &["json"]).pick_file() {
                        match d.cfg.import_shortcuts(&p) {
                            Ok(_) => {
                                self.pending.clear();
                                for (a, s) in self.editors.iter_mut() {
                                    *s = d.cfg.get_shortcut(a);
                                }
                                outcome = Some(DialogOutcome::ApplyShortcuts);
                                *message = Some(MessageBox::new("Import", "Shortcut profile imported successfully.", MsgButtons::Ok, MsgTag::Info));
                            }
                            Err(e) => *message = Some(MessageBox::new("Import Error", &e, MsgButtons::Ok, MsgTag::Info)),
                        }
                    }
                }
                if widgets::button(ui, theme, "Close").clicked() || (escape_pressed(ui) && self.capturing.is_none()) {
                    close = true;
                }
            });
        });
        (close, outcome)
    }

    /// After "Reset All" was confirmed.
    pub fn reset_all(&mut self, cfg: &mut Config) {
        cfg.reset_all_shortcuts();
        self.pending.clear();
        for (a, s) in self.editors.iter_mut() {
            *s = xpr_core::config::default_shortcut(a).to_string();
        }
    }
}

// ---------------------------------------------------------------------------
// Final trainers
// ---------------------------------------------------------------------------

pub struct FinalTrainersDialog {
    versions: OptionMenu,
    filter: String,
    trainers: Vec<(String, bool)>,
}

impl FinalTrainersDialog {
    pub fn new(cfg: &Config, registry: &Registry, current_version: Option<&str>) -> FinalTrainersDialog {
        let all = registry.get_gen_names(true, true);
        let default = current_version.filter(|v| all.iter().any(|a| a == v)).map(|s| s.to_string()).or_else(|| all.first().cloned());
        let mut d = FinalTrainersDialog { versions: OptionMenu::new(all, default.as_deref()), filter: String::new(), trainers: Vec::new() };
        d.populate(cfg, registry);
        d
    }

    fn populate(&mut self, cfg: &Config, registry: &Registry) {
        self.trainers.clear();
        let version = self.versions.get().to_string();
        if version.is_empty() {
            return;
        }
        let Ok(gen) = registry.get_version(&version) else { return };
        let mut names = gen.trainer_db().get_valid_trainers(None, None, &[], false, false);
        names.sort();
        names.dedup();
        let selected = cfg.get_final_trainers(&version);
        self.trainers = names.into_iter().map(|n| (n.clone(), selected.contains(&n))).collect();
    }

    pub fn ui(&mut self, ctx: &egui::Context, d: &mut DialogCtx) -> (bool, Option<DialogOutcome>) {
        let theme = d.theme;
        let mut close = false;
        let mut repopulate = false;
        let mut save = false;
        modal(ctx, theme, "xpr_final_trainers", "Configure Final Trainers", 520.0, |ui| {
            ui.add(egui::Label::new(egui::RichText::new("Pick the trainer(s) that mark the end of a run for each game.\nWhile recording, defeating any of these trainers will automatically turn recording off so no extra events are captured.").font(theme.body()).color(theme.text)).wrap());
            ui.horizontal(|ui| {
                widgets::label(ui, theme, "Game Version:");
                if self.versions.ui(ui, theme, ui.id().with("version"), Some(300.0), true) {
                    repopulate = true;
                }
            });
            ui.horizontal(|ui| {
                widgets::label(ui, theme, "Filter:");
                Entry::new(theme, &mut self.filter).width(300.0).hint("Type to filter trainers by name").id(ui.id().with("filter")).show(ui);
            });
            let needle = self.filter.trim().to_lowercase();
            egui::Frame::new().fill(theme.bg_input).stroke(Stroke::new(1.0_f32, theme.border)).show(ui, |ui| {
                egui::ScrollArea::vertical().max_height(360.0).auto_shrink([false, false]).show(ui, |ui| {
                    for (name, checked) in self.trainers.iter_mut() {
                        if !needle.is_empty() && !name.to_lowercase().contains(&needle) {
                            continue;
                        }
                        if widgets::checkbox(ui, theme, checked, name, true).changed() {
                            save = true;
                        }
                    }
                });
            });
            let sel = self.trainers.iter().filter(|(_, c)| *c).count();
            ui.horizontal(|ui| {
                widgets::label(ui, theme, format!("Selected: {} / {}", sel, self.trainers.len()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::button(ui, theme, "Close").clicked() || escape_pressed(ui) {
                        close = true;
                    }
                    if widgets::button(ui, theme, "Clear All For This Game").clicked() {
                        for (_, c) in self.trainers.iter_mut() {
                            *c = false;
                        }
                        d.cfg.set_final_trainers(self.versions.get(), Vec::new());
                    }
                    if widgets::button(ui, theme, "Reset to Defaults").clicked() {
                        d.cfg.reset_final_trainers(self.versions.get());
                        repopulate = true;
                    }
                });
            });
        });
        if save {
            let selected: Vec<String> = self.trainers.iter().filter(|(_, c)| *c).map(|(n, _)| n.clone()).collect();
            d.cfg.set_final_trainers(self.versions.get(), selected);
        }
        if repopulate {
            self.populate(d.cfg, d.registry);
        }
        (close, None)
    }
}

// ---------------------------------------------------------------------------
// Matchup export / assign move
// ---------------------------------------------------------------------------

pub struct MatchupExportDialog {
    pub mon_idx: usize,
}

impl MatchupExportDialog {
    pub fn ui(&mut self, ctx: &egui::Context, d: &mut DialogCtx) -> (bool, Option<DialogOutcome>) {
        let theme = d.theme;
        let mut close = false;
        let mut outcome = None;
        modal(ctx, theme, "xpr_matchup_export", "Export Matchup", 360.0, |ui| {
            ui.vertical_centered(|ui| widgets::label(ui, theme, "Which graphic would you like to export?"));
            ui.horizontal(|ui| {
                for (label, mode) in [("Match Up", crate::battle_ui::ScreenshotMode::Full), ("Player Ranges", crate::battle_ui::ScreenshotMode::Player), ("Enemy Ranges", crate::battle_ui::ScreenshotMode::Enemy)] {
                    if widgets::button(ui, theme, label).clicked() {
                        outcome = Some(DialogOutcome::MatchupExport { idx: self.mon_idx, mode });
                        close = true;
                    }
                }
            });
            ui.vertical_centered(|ui| {
                if widgets::button(ui, theme, "Cancel").clicked() || escape_pressed(ui) {
                    close = true;
                }
            });
        });
        (close, outcome)
    }
}

pub struct AssignMoveDialog {
    pub slot_idx: i64,
    moves: Vec<String>,
    filter: String,
    selected: Option<String>,
}

impl AssignMoveDialog {
    pub fn new(gen: &GenData, slot_idx: i64) -> AssignMoveDialog {
        AssignMoveDialog { slot_idx, moves: gen.move_db().get_filtered_names(None, false), filter: String::new(), selected: None }
    }

    pub fn ui(&mut self, ctx: &egui::Context, d: &mut DialogCtx) -> (bool, Option<DialogOutcome>) {
        let theme = d.theme;
        let mut close = false;
        let mut outcome = None;
        modal(ctx, theme, "xpr_assign_move", &format!("Assign Move to Slot {}", self.slot_idx + 1), 360.0, |ui| {
            let r = Entry::new(theme, &mut self.filter).width(300.0).hint("Filter moves...").id(ui.id().with("filter")).show(ui);
            let needle = self.filter.trim().to_lowercase();
            let visible: Vec<String> = self.moves.iter().filter(|m| needle.is_empty() || m.to_lowercase().contains(&needle)).cloned().collect();
            egui::Frame::new().fill(theme.bg_input).stroke(Stroke::new(1.0_f32, theme.border)).show(ui, |ui| {
                egui::ScrollArea::vertical().max_height(300.0).auto_shrink([false, false]).show(ui, |ui| {
                    for m in &visible {
                        let is_sel = self.selected.as_deref() == Some(m.as_str());
                        let resp = ui.selectable_label(is_sel, egui::RichText::new(m).font(theme.body()));
                        if resp.clicked() {
                            self.selected = Some(m.clone());
                        }
                        if resp.double_clicked() {
                            outcome = Some(DialogOutcome::AssignMove { slot: self.slot_idx, mv: m.clone() });
                            close = true;
                        }
                    }
                });
            });
            if r.enter_pressed {
                let pick = self.selected.clone().or_else(|| visible.first().cloned());
                if let Some(m) = pick {
                    outcome = Some(DialogOutcome::AssignMove { slot: self.slot_idx, mv: m });
                    close = true;
                }
            }
            ui.horizontal(|ui| {
                let can = self.selected.is_some();
                if widgets::button_enabled(ui, theme, "Assign", can).clicked() && can {
                    outcome = Some(DialogOutcome::AssignMove { slot: self.slot_idx, mv: self.selected.clone().unwrap() });
                    close = true;
                }
                if widgets::button(ui, theme, "Cancel").clicked() || escape_pressed(ui) {
                    close = true;
                }
            });
        });
        (close, outcome)
    }
}

impl Dialog {
    /// Draw the open dialog; returns (close, outcome).
    pub fn ui(&mut self, ctx: &egui::Context, d: &mut DialogCtx, message: &mut Option<MessageBox>) -> (bool, Option<DialogOutcome>) {
        match self {
            Dialog::LoadRoute(x) => x.ui(ctx, d),
            Dialog::NewFolder(x) => x.ui(ctx, d),
            Dialog::Transfer(x) => x.ui(ctx, d),
            Dialog::CustomDvs(x) => x.ui(ctx, d),
            Dialog::CustomGen(x) => x.ui(ctx, d, message),
            Dialog::BattleConfig(x) => x.ui(ctx, d),
            Dialog::ColorConfig(x) => x.ui(ctx, d),
            Dialog::HighlightColors(x) => x.ui(ctx, d),
            Dialog::AppConfig(x) => x.ui(ctx, d, message),
            Dialog::Shortcuts(x) => x.ui(ctx, d, message),
            Dialog::FinalTrainers(x) => x.ui(ctx, d),
            Dialog::MatchupExport(x) => x.ui(ctx, d),
            Dialog::AssignMove(x) => x.ui(ctx, d),
        }
    }
}

/// Little helper for keyboard-shortcut labels in menus.
pub fn label_for(action_id: &str) -> String {
    SHORTCUT_LABELS.iter().find(|(a, _)| *a == action_id).map(|(_, l)| l.to_string()).unwrap_or_else(|| action_id.to_string())
}
