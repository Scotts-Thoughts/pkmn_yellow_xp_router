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
use xpr_data::model::{Nature, StatBlock, SINGLE_STAT_EV_CAP, STAT_XP_CAP_GEN12, TOTAL_EV_CAP};
use xpr_data::{GenData, Registry};
use xpr_engine::{EvOverrideEventDefinition, NodeId};
use xpr_ui_kit::shortcuts::format_key_sequence;
use xpr_ui_kit::theme::{self, Theme};
use xpr_ui_kit::widgets::{self, AmountEntry, Entry};

use crate::controller::MainController;
use crate::custom_dvs::CustomDvsFrame;
use crate::editors::{ev_field_indices, OptionMenu, EV_STAT_NAMES, EV_STAT_NAMES_GEN12};

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

impl MsgTag {
    /// Whether "Yes" throws work away (its button is drawn in the failure
    /// colour). The unsaved-changes prompts name their buttons instead
    /// ([`MsgButtons::SaveDiscardCancel`]).
    fn is_destructive(&self) -> bool {
        matches!(self, MsgTag::DeleteEvents(_) | MsgTag::ResetAllShortcuts)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MsgButtons {
    Ok,
    YesNo,
    YesNoCancel,
    OkCreateRoute,
    /// An unsaved-changes prompt with its buttons named for what they do:
    /// `save` ("Save, then close") answers Yes, `discard` ("Close without
    /// saving") answers No, and Cancel answers Cancel.
    SaveDiscardCancel { save: &'static str, discard: &'static str },
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
    AssignMove { slot: i64, mv: String, pre_state: bool },
    /// `[hp, atk, def, spa, spd, spe]` for an EV override before the selected event
    EvOverride([i64; 6]),
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
    EvOverride(EvOverrideDialog),
}

/// Shared context handed to dialog `ui` calls.
pub struct DialogCtx<'a> {
    pub theme: &'a Theme,
    pub cfg: &'a mut Config,
    pub ctrl: &'a mut MainController,
    pub registry: &'a Arc<Registry>,
    pub paths: &'a Paths,
}

// ---------------------------------------------------------------------------
// Dialog chrome and parts
// ---------------------------------------------------------------------------
//
// The card language of the redesigned panes (docs/rust_port/design/
// pre_event_state/SPEC.md §3), one step up in elevation: a raised card with
// a title strip, body sections headed by uppercase captions, muted help
// text, lists in a recessed well, and a footer of right-aligned buttons
// whose main action is filled with the accent.

/// Horizontal padding of the title strip, body and footer.
const PAD_X: f32 = 20.0;
/// Height of the footer buttons.
const BUTTON_H: f32 = 30.0;
/// Width of the controls in a [`form`]'s second column.
const FIELD_W: f32 = 260.0;

/// The dialog surface: a step lighter than the cards on the pages (inputs
/// and wells then read as recessed, as in the panes).
fn surface(theme: &Theme) -> Color32 {
    theme.section_bg()
}

/// Draw a dialog `width` points wide with `title` in its title strip;
/// `content` draws the body (and usually ends with a [`footer`]).
pub(crate) fn modal<R>(ctx: &egui::Context, theme: &Theme, id: &str, title: &str, width: f32, content: impl FnOnce(&mut Ui) -> R) -> R {
    let frame = egui::Frame::new()
        .fill(surface(theme))
        .stroke(Stroke::new(1.0_f32, theme::lighten(theme.bg, 0.14)))
        .corner_radius(CornerRadius::same(10))
        .shadow(egui::Shadow { offset: [0, 12], blur: 40, spread: 0, color: Color32::from_black_alpha(150) });
    let m = egui::Modal::new(egui::Id::new(id)).frame(frame).backdrop_color(Color32::from_black_alpha(150));
    m.show(ctx, |ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let pad = PAD_X as i8;
        egui::Frame::new().inner_margin(egui::Margin { left: pad, right: pad, top: 15, bottom: 13 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add(egui::Label::new(egui::RichText::new(title).font(theme.font_bold(12.0)).color(theme.text_strong())).wrap());
        });
        let r = ui.max_rect();
        let y = ui.cursor().min.y;
        ui.painter().hline(r.x_range(), y + 0.5, Stroke::new(1.0_f32, theme.pane_divider()));
        ui.add_space(1.0);
        egui::Frame::new()
            .inner_margin(egui::Margin { left: pad, right: pad, top: 16, bottom: 16 })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                content(ui)
            })
            .inner
    })
    .inner
}

/// The button row that ends a dialog body: a hairline across the whole
/// dialog, then `add` laid out right to left, so the first button added
/// (the main action) is the rightmost. Buttons for the left edge go in a
/// nested left-to-right layout at the end.
fn footer(ui: &mut Ui, theme: &Theme, add: impl FnOnce(&mut Ui)) {
    ui.add_space(8.0);
    let r = ui.max_rect();
    let y = ui.cursor().min.y;
    ui.painter().hline((r.min.x - PAD_X)..=(r.max.x + PAD_X), y + 0.5, Stroke::new(1.0_f32, theme.pane_divider()));
    ui.add_space(16.0);
    ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), BUTTON_H), egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        add(ui);
    });
}

/// How much a dialog button asks to be pressed.
#[derive(Clone, Copy, PartialEq)]
enum Tone {
    /// The dialog's main action: accent fill.
    Primary,
    /// Everything else: outlined, quiet.
    Secondary,
    /// A main action that throws work away: failure-colour fill.
    Danger,
    /// A side option that throws work away: outlined in the failure colour.
    DangerOutline,
}

/// A footer button (30 px tall, radius 6).
fn action_button(ui: &mut Ui, theme: &Theme, text: &str, tone: Tone, enabled: bool) -> egui::Response {
    button_sized(ui, theme, text, tone, enabled, BUTTON_H, 76.0, theme.body_bold())
}

/// A compact secondary button for rows inside the body (24 px tall).
fn small_button(ui: &mut Ui, theme: &Theme, text: &str, enabled: bool) -> egui::Response {
    button_sized(ui, theme, text, Tone::Secondary, enabled, 24.0, 0.0, theme.body())
}

#[allow(clippy::too_many_arguments)]
fn button_sized(ui: &mut Ui, theme: &Theme, text: &str, tone: Tone, enabled: bool, h: f32, min_w: f32, font: egui::FontId) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, Color32::PLACEHOLDER);
    let pad = if h >= BUTTON_H { 16.0 } else { 10.0 };
    let size = Vec2::new((galley.size().x + 2.0 * pad).max(min_w).round(), h);
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    if ui.is_rect_visible(rect) {
        let hovered = enabled && resp.hovered();
        let pressed = enabled && resp.is_pointer_button_down_on();
        let shade = |c: Color32| if pressed { theme::darken(c, 0.12) } else if hovered { theme::lighten(c, 0.10) } else { c };
        let (fill, stroke, text_color) = match (tone, enabled) {
            (Tone::Primary, true) => (shade(theme.accent), Stroke::NONE, Color32::WHITE),
            (Tone::Danger, true) => (shade(theme::darken(theme.failure, 0.12)), Stroke::NONE, Color32::WHITE),
            (Tone::Secondary, true) => {
                let fill = if pressed { theme::lighten(surface(theme), 0.10) } else if hovered { theme::lighten(surface(theme), 0.06) } else { Color32::TRANSPARENT };
                (fill, Stroke::new(1.0_f32, if hovered { theme::lighten(theme.border, 0.15) } else { theme.border }), theme.text_strong())
            }
            (Tone::DangerOutline, true) => {
                let fill = if pressed { theme::with_alpha(theme.failure, 60) } else if hovered { theme::with_alpha(theme.failure, 30) } else { Color32::TRANSPARENT };
                (fill, Stroke::new(1.0_f32, theme::tint(theme.failure, surface(theme), if hovered { 0.9 } else { 0.6 })), theme::lighten(theme.failure, 0.25))
            }
            (Tone::Secondary | Tone::DangerOutline, false) => (Color32::TRANSPARENT, Stroke::new(1.0_f32, theme.subtle_border), theme.disabled_text),
            (_, false) => (theme::with_alpha(theme.accent, 70), Stroke::NONE, theme::with_alpha(Color32::WHITE, 110)),
        };
        ui.painter().rect(rect, CornerRadius::same(6), fill, stroke, egui::StrokeKind::Inside);
        ui.painter().galley(rect.center() - galley.size() / 2.0, galley, text_color);
    }
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// An uppercase section title (header colour), with an optional muted
/// caption at the right; every section but the first gets air above it.
fn section(ui: &mut Ui, theme: &Theme, title: &str, right: Option<&str>) {
    if ui.cursor().min.y > ui.max_rect().min.y + 1.0 {
        ui.add_space(8.0);
    }
    widgets::card_title(ui, theme, title, right);
}

/// Muted, wrapped explanatory text.
fn help(ui: &mut Ui, theme: &Theme, text: &str) {
    ui.add(egui::Label::new(egui::RichText::new(text).font(theme.body()).color(theme.secondary)).wrap());
}

/// Wrapped body text.
fn body_text(ui: &mut Ui, theme: &Theme, text: &str) {
    ui.add(egui::Label::new(egui::RichText::new(text).font(theme.body()).color(theme.text)).wrap());
}

/// A text field in the dialogs' control style (28 px tall, radius 4).
fn field<'a>(theme: &'a Theme, text: &'a mut String) -> Entry<'a> {
    Entry::new(theme, text).corner_radius(4).min_height(28.0)
}

/// A two-column grid of labelled controls (dropdowns as tall as the fields).
fn form<R>(ui: &mut Ui, id: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = 28.0;
        egui::Grid::new(id).num_columns(2).spacing(Vec2::new(16.0, 10.0)).show(ui, add).inner
    })
    .inner
}

/// A form row label.
fn form_label(ui: &mut Ui, theme: &Theme, text: &str) {
    widgets::label_font(ui, text, theme.body(), theme.text);
}

/// One of a set of mutually exclusive choices that act on click: a
/// full-width card with a title, a muted detail line and a chevron.
fn choice_row(ui: &mut Ui, theme: &Theme, title: &str, detail: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 46.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let hovered = resp.hovered();
        let (fill, stroke) = if hovered { (theme.hover_bg, theme.accent) } else { (theme.bg_input, theme.card_border()) };
        ui.painter().rect(rect, CornerRadius::same(8), fill, Stroke::new(1.0_f32, stroke), egui::StrokeKind::Inside);
        let x = rect.min.x + 14.0;
        ui.painter().text(egui::Pos2::new(x, rect.center().y - 8.0), egui::Align2::LEFT_CENTER, title, theme.body_bold(), theme.text_strong());
        ui.painter().text(egui::Pos2::new(x, rect.center().y + 9.0), egui::Align2::LEFT_CENTER, detail, theme.body(), theme.secondary);
        // chevron
        let c = egui::Pos2::new(rect.max.x - 18.0, rect.center().y);
        let color = if hovered { theme.text_strong() } else { theme.secondary };
        ui.painter().line(vec![egui::Pos2::new(c.x - 3.0, c.y - 5.0), c + Vec2::new(2.0, 0.0), egui::Pos2::new(c.x - 3.0, c.y + 5.0)], Stroke::new(1.5_f32, color));
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A read-only, recessed field showing a path (elided when it does not
/// fit; the full path is the tooltip).
fn path_field(ui: &mut Ui, theme: &Theme, path: &str, width: f32) {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width.max(80.0), 24.0), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().rect(rect, CornerRadius::same(6), theme.well_bg(), Stroke::new(1.0_f32, theme.card_border()), egui::StrokeKind::Inside);
        let shown = widgets::elide(ui, path, &theme.body(), rect.width() - 16.0);
        ui.painter().text(egui::Pos2::new(rect.min.x + 8.0, rect.center().y), egui::Align2::LEFT_CENTER, shown, theme.body(), theme.text);
    }
    resp.on_hover_text(path);
}

/// `preferred` scroll height, capped so a dialog with `other` points of
/// title strip, footer and fixed content still fits the window.
fn fit_h(ui: &Ui, preferred: f32, other: f32) -> f32 {
    preferred.min((ui.ctx().content_rect().height() - 40.0 - other).max(120.0))
}

/// A recessed, scrolling list box `max_h` tall.
fn list_well<R>(ui: &mut Ui, theme: &Theme, max_h: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(theme.well_bg())
        .stroke(Stroke::new(1.0_f32, theme.card_border()))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(4))
        .show(ui, |ui| {
            widgets::show_scroll(ui, egui::ScrollArea::vertical().max_height(max_h).auto_shrink([false, false]), |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 1.0);
                add(ui)
            })
            .inner
        })
        .inner
}

/// A selectable row of a [`list_well`].
fn list_row(ui: &mut Ui, theme: &Theme, text: &str, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let fill = if selected {
            theme::tint(theme.accent, theme.well_bg(), 0.32)
        } else if resp.hovered() {
            theme.hover_bg
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, CornerRadius::same(6), fill);
        let (font, color) = if selected { (theme.body_bold(), theme.text_strong()) } else { (theme.body(), theme.text) };
        ui.painter().text(egui::Pos2::new(rect.min.x + 10.0, rect.center().y), egui::Align2::LEFT_CENTER, text, font, color);
    }
    resp
}

/// Cover a secondary window (its own viewport, which the main window's
/// dialog cannot cover) while a dialog is up in the main window: the dialogs
/// are application-modal, like the Qt ones, so e.g. the popped-out map may
/// not select or add events under an "Assign Move" dialog.
pub fn block_secondary_window(ctx: &egui::Context, theme: &Theme) {
    if ctx.embed_viewports() {
        // no native windows: the "window" is an egui window in the main
        // viewport, already under the dialog (a second modal would cover it)
        return;
    }
    modal(ctx, theme, "xpr_secondary_window_blocked", "Dialog Open", 300.0, |ui| {
        help(ui, theme, "Finish the dialog in the main window first.");
    });
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
        let yes = if self.tag.is_destructive() { Tone::Danger } else { Tone::Primary };
        let width = match self.buttons {
            // room for the two long labels and Cancel on one row
            MsgButtons::SaveDiscardCancel { save, discard } => if save.len() + discard.len() > 40 { 540.0 } else { 480.0 },
            _ if self.text.len() > 160 => 480.0,
            _ => 400.0,
        };
        modal(ctx, theme, "xpr_message_box", &self.title, width, |ui| {
            body_text(ui, theme, &self.text);
            footer(ui, theme, |ui| match self.buttons {
                MsgButtons::Ok => {
                    if action_button(ui, theme, "OK", Tone::Primary, true).clicked() || enter_pressed(ui) {
                        choice = Some(MsgChoice::Ok);
                    }
                }
                MsgButtons::YesNo => {
                    if action_button(ui, theme, "Yes", yes, true).clicked() {
                        choice = Some(MsgChoice::Yes);
                    }
                    if action_button(ui, theme, "No", Tone::Secondary, true).clicked() || enter_pressed(ui) {
                        choice = Some(MsgChoice::No);
                    }
                }
                MsgButtons::YesNoCancel => {
                    if action_button(ui, theme, "Yes", yes, true).clicked() {
                        choice = Some(MsgChoice::Yes);
                    }
                    if action_button(ui, theme, "No", Tone::Secondary, true).clicked() {
                        choice = Some(MsgChoice::No);
                    }
                    if action_button(ui, theme, "Cancel", Tone::Secondary, true).clicked() || enter_pressed(ui) {
                        choice = Some(MsgChoice::Cancel);
                    }
                }
                MsgButtons::SaveDiscardCancel { save, discard } => {
                    if action_button(ui, theme, save, Tone::Primary, true).clicked() {
                        choice = Some(MsgChoice::Yes);
                    }
                    if action_button(ui, theme, "Cancel", Tone::Secondary, true).clicked() || enter_pressed(ui) {
                        choice = Some(MsgChoice::Cancel);
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        if action_button(ui, theme, discard, Tone::DangerOutline, true).clicked() {
                            choice = Some(MsgChoice::No);
                        }
                    });
                }
                MsgButtons::OkCreateRoute => {
                    if action_button(ui, theme, "OK", Tone::Primary, true).clicked() || enter_pressed(ui) {
                        choice = Some(MsgChoice::Ok);
                    }
                    if action_button(ui, theme, "Create Route for Backport", Tone::Secondary, true).clicked() {
                        choice = Some(MsgChoice::Extra);
                    }
                }
            });
            if escape_pressed(ui) {
                choice = Some(match self.buttons {
                    MsgButtons::YesNoCancel | MsgButtons::SaveDiscardCancel { .. } => MsgChoice::Cancel,
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
        modal(ctx, theme, "xpr_load_route", "Load Route", 460.0, |ui| {
            form(ui, "load_route_grid", |ui| {
                form_label(ui, theme, "Route");
                self.routes.ui(ui, theme, ui.id().with("routes"), Some(FIELD_W), true);
                ui.end_row();
                form_label(ui, theme, "Filter");
                if field(theme, &mut self.filter).width(FIELD_W).hint("Filter by name").id(ui.id().with("filter")).show(ui).changed {
                    refilter = true;
                }
                ui.end_row();
                ui.label("");
                if widgets::checkbox(ui, theme, &mut self.show_backups, "Show backup routes", true).changed() {
                    refilter = true;
                }
                ui.end_row();
            });
            help(ui, theme, "Backup routes are older versions of your route. Every save makes a backup that is kept and can be reloaded if needed. They are hidden by default because they can quickly pile up.");
            ui.add_space(2.0);
            widgets::warning_banner(ui, theme, "Any unsaved changes in your current route will be lost when loading an existing route.");
            let selected = self.routes.get().to_string();
            let can_load = selected != consts::NO_SAVED_ROUTES;
            footer(ui, theme, |ui| {
                if (action_button(ui, theme, "Load Route", Tone::Primary, can_load).clicked() || enter_pressed(ui)) && can_load {
                    outcome = Some(DialogOutcome::LoadRoute(io_utils::get_existing_route_path(d.paths, &selected)));
                    close = true;
                }
                if action_button(ui, theme, "Cancel", Tone::Secondary, true).clicked() || escape_pressed(ui) {
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
        modal(ctx, theme, "xpr_new_folder", title, 380.0, |ui| {
            let valid = !self.name.is_empty() && !self.folder_names.contains(&self.name);
            form_label(ui, theme, "Folder name");
            let id = ui.id().with("folder_name");
            if !self.focused {
                ui.memory_mut(|m| m.request_focus(id));
                self.focused = true;
            }
            let r = field(theme, &mut self.name).width(ui.available_width()).id(id).show(ui);
            if !self.name.is_empty() && self.folder_names.contains(&self.name) && self.prev_name.as_ref() != Some(&self.name) {
                widgets::label_font(ui, "A folder with this name already exists.", theme.body(), theme.failure);
            }
            footer(ui, theme, |ui| {
                let label = if self.prev_name.is_none() { "Create Folder" } else { "Rename Folder" };
                if (action_button(ui, theme, label, Tone::Primary, valid).clicked() || r.enter_pressed) && valid {
                    outcome = Some(DialogOutcome::NewFolder { name: self.name.clone(), prev: self.prev_name.clone(), insert_after: self.insert_after });
                    close = true;
                }
                if action_button(ui, theme, "Cancel", Tone::Secondary, true).clicked() || escape_pressed(ui) {
                    close = true;
                }
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
        modal(ctx, theme, "xpr_transfer", "Transfer Events", 440.0, |ui| {
            let existing = self.transfer_type.get() == consts::TRANSFER_EXISTING_FOLDER;
            let enabled = if existing { self.dest.get() != consts::NO_FOLDERS } else { !self.all_folders.contains(&self.new_folder) };
            form(ui, "transfer_grid", |ui| {
                form_label(ui, theme, "Transfer to");
                self.transfer_type.ui(ui, theme, ui.id().with("type"), Some(FIELD_W), true);
                ui.end_row();
                if existing {
                    form_label(ui, theme, "Destination");
                    self.dest.ui(ui, theme, ui.id().with("dest"), Some(FIELD_W), true);
                    ui.end_row();
                    form_label(ui, theme, "Filter");
                    if field(theme, &mut self.filter).width(FIELD_W).hint("Filter folders").id(ui.id().with("filter")).show(ui).changed {
                        refilter = true;
                    }
                    ui.end_row();
                } else {
                    form_label(ui, theme, "New folder");
                    field(theme, &mut self.new_folder).width(FIELD_W).hint("Folder name").id(ui.id().with("new_folder")).show(ui);
                    ui.end_row();
                }
            });
            footer(ui, theme, |ui| {
                if (action_button(ui, theme, "Transfer", Tone::Primary, enabled).clicked() || enter_pressed(ui)) && enabled {
                    let folder = if existing { self.dest.get().to_string() } else { self.new_folder.trim().to_string() };
                    outcome = Some(DialogOutcome::Transfer { ids: self.ids.clone(), folder });
                    close = true;
                }
                if action_button(ui, theme, "Cancel", Tone::Secondary, true).clicked() || escape_pressed(ui) {
                    close = true;
                }
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
        modal(ctx, theme, "xpr_custom_dvs", "Custom DVs/IVs", 440.0, |ui| {
            self.frame.ui(ui, theme);
            footer(ui, theme, |ui| {
                if action_button(ui, theme, "Set New DVs", Tone::Primary, true).clicked() || enter_pressed(ui) {
                    if let Some((dvs, ability, nature)) = self.frame.get_dvs() {
                        outcome = Some(DialogOutcome::SetDvs(dvs, ability, nature));
                    }
                    close = true;
                }
                if action_button(ui, theme, "Cancel", Tone::Secondary, true).clicked() || escape_pressed(ui) {
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
            modal(ctx, theme, "xpr_backport_species", "Species Name", 360.0, |ui| {
                form_label(ui, theme, "Enter the species name");
                let r = field(theme, &mut bp.species).width(ui.available_width()).id(ui.id().with("species")).show(ui);
                footer(ui, theme, |ui| {
                    if action_button(ui, theme, "OK", Tone::Primary, true).clicked() || r.enter_pressed {
                        done = Some(true);
                    }
                    if action_button(ui, theme, "Cancel", Tone::Secondary, true).clicked() || escape_pressed(ui) {
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
        modal(ctx, theme, "xpr_custom_gen", "Custom Gen Manager", 560.0, |ui| {
            section(ui, theme, "How custom gens work", None);
            help(ui, theme, "Create a custom gen when you want to play a romhack, or a modified version of an official game (e.g. hacking in a pokemon from a newer gen). A custom gen re-uses all the damage calculations and mechanics from the base official generation, but lets you route with new or changed content.");
            help(ui, theme, "Creating a custom gen copies all the information for the official gen into a new folder. Once created, open that folder from the list below and modify the files as necessary. Custom gens are loaded on startup, so restart the app after changing one.");
            help(ui, theme, "All custom gens are validated on startup. If errors are detected you will get a pop-up; the app still works, but a custom gen with errors is not loaded.");
            section(ui, theme, "New custom gen", None);
            let valid = self.verify_name(d.registry, &self.custom_name);
            form(ui, "custom_gen_grid", |ui| {
                form_label(ui, theme, "Base version");
                self.base_version.ui(ui, theme, ui.id().with("base"), Some(FIELD_W), true);
                ui.end_row();
                form_label(ui, theme, "Custom version name");
                field(theme, &mut self.custom_name).width(FIELD_W).hint("Name of the new version").id(ui.id().with("custom_name")).show(ui);
                ui.end_row();
                ui.label("");
                if (action_button(ui, theme, "Create Custom Version", Tone::Primary, valid).clicked() || enter_pressed(ui)) && valid {
                    let base = self.base_version.get().to_string();
                    let name = self.custom_name.clone();
                    d.ctrl.create_custom_version(&base, &name);
                    self.populate(d.registry);
                }
                ui.end_row();
            });
            let count = if self.gens.is_empty() { None } else { Some(format!("{}", self.gens.len())) };
            section(ui, theme, "Your custom gens", count.as_deref());
            if self.gens.is_empty() {
                help(ui, theme, "No custom gens yet.");
            }
            for (path, base, name) in self.gens.clone() {
                widgets::card(ui, theme, Some(egui::Margin::symmetric(12, 8)), |ui| {
                    ui.horizontal(|ui| {
                        ui.set_width(ui.available_width());
                        widgets::label_font(ui, &name, theme.body_bold(), theme.text_strong());
                        widgets::label_font(ui, format!("based on {}", base), theme.body(), theme.secondary);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if small_button(ui, theme, "Import Backport", true).clicked() {
                                import_for = Some((path.clone(), name.clone()));
                            }
                            if small_button(ui, theme, "Open Folder", true).clicked() {
                                open_path = Some(path.clone());
                            }
                        });
                    });
                });
            }
            footer(ui, theme, |ui| {
                if action_button(ui, theme, "Close", Tone::Secondary, true).clicked() || escape_pressed(ui) {
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
            widgets::show_scroll(ui, egui::ScrollArea::vertical().max_height(fit_h(ui, 520.0, 170.0)), |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                ui.set_width(ui.available_width() - 16.0);
                section(ui, theme, "Damage calculation", None);
                form(ui, "battle_cfg_calc", |ui| {
                    form_label(ui, theme, "Search depth (turns)");
                    if widgets::AmountEntry::new(theme, ui.id().with("depth"), &mut self.search_depth).min(Some(1)).max(Some(99)).show(ui).changed {
                        let v = self.search_depth.trim().parse::<i64>().unwrap_or(xpr_core::config::DEFAULT_DAMAGE_SEARCH_DEPTH);
                        d.cfg.set_damage_search_depth(v);
                    }
                    ui.end_row();
                });
                help(ui, theme, "The number of turns searched to find kill percentages. Larger values give more accurate guaranteed kills, but may take longer, especially on slower computers.");
                if widgets::checkbox(ui, theme, &mut self.ignore_accuracy, "Ignore accuracy in kill ranges", true).changed() {
                    d.cfg.set_ignore_accuracy(self.ignore_accuracy);
                }
                if widgets::checkbox(ui, theme, &mut self.force_full_search, "Fully calculate Psywave (not recommended)", true).changed() {
                    d.cfg.set_force_full_search(self.force_full_search);
                }

                section(ui, theme, "Highlighting", None);
                form(ui, "battle_cfg_highlight", |ui| {
                    form_label(ui, theme, "Player strategy");
                    if self.player_strat.ui(ui, theme, ui.id().with("pstrat"), Some(200.0), true) {
                        d.cfg.set_player_highlight_strategy(self.player_strat.get());
                    }
                    ui.end_row();
                    form_label(ui, theme, "Enemy strategy");
                    if self.enemy_strat.ui(ui, theme, ui.id().with("estrat"), Some(200.0), true) {
                        d.cfg.set_enemy_highlight_strategy(self.enemy_strat.get());
                    }
                    ui.end_row();
                    form_label(ui, theme, "Consistency threshold (%)");
                    if widgets::AmountEntry::new(theme, ui.id().with("thresh"), &mut self.consistent_threshold).min(Some(1)).max(Some(99)).show(ui).changed {
                        let v = self.consistent_threshold.trim().parse::<i64>().unwrap_or(xpr_core::config::DEFAULT_CONSISTENT_THRESHOLD);
                        d.cfg.set_consistent_threshold(v);
                    }
                    ui.end_row();
                });
                for (name, text) in [
                    ("Guaranteed Kill", "Highlights the move with the fewest turns for a 'guaranteed' kill: a 99% chance or higher to kill."),
                    ("Fastest Kill", "Highlights the move with the fewest turns for any possible kill. Kills with less than a 0.1% chance are ignored by the damage calcs, but a move with at least a 1% chance of killing is reported."),
                    ("Consistent Kill", "Highlights the move with the fewest turns for a kill whose chance is above the consistency threshold."),
                ] {
                    ui.add_space(2.0);
                    widgets::label_font(ui, name, theme.body_bold(), theme.text_strong());
                    help(ui, theme, text);
                }
                help(ui, theme, "Whatever the strategy, ties between moves with the same number of turns are broken by, in order: highest accuracy, punishing 2-turn moves (Dig/Fly), highest damage. Hyper Beam is not highlighted unless it kills with a single non-crit hit, because of the recharge turn.");

                section(ui, theme, "Limitations and edge cases", None);
                for line in [
                    "Possible kills with less than a 0.1% chance are not reported.",
                    "For each move, a full search for kill percentages is done, up to the search depth above.",
                    "Up to 3 ranges are reported: the 2 fastest kills, if applicable, and the number of turns for a guaranteed kill, which is always last.",
                    "For weaker moves, the maximum number of hits needed to guarantee a kill (assuming every attack lands) is given instead.",
                    "Gen 1 misses are ignored in accuracy calculations.",
                    "The crit damage ranges for multi-hit moves in Gen 2 assume exactly one of the hits crits.",
                ] {
                    help(ui, theme, &format!("•  {}", line));
                }
            });
            footer(ui, theme, |ui| {
                if action_button(ui, theme, "Close", Tone::Primary, true).clicked() || enter_pressed(ui) || escape_pressed(ui) {
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

/// `ConfigColorUpdater`: label, then the hex code and a swatch that opens an
/// in-app picker. Returns the new hex when the colour changed.
fn color_updater(ui: &mut Ui, theme: &Theme, label: &str, current_hex: &str, enabled: bool) -> Option<String> {
    let mut result = None;
    ui.horizontal(|ui| {
        ui.set_min_height(26.0);
        widgets::label_font(ui, label, theme.body(), if enabled { theme.text } else { theme.disabled_text });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mut color = theme::parse_hex(current_hex);
            ui.add_enabled_ui(enabled, |ui| {
                ui.spacing_mut().interact_size = Vec2::new(44.0, 22.0);
                let before = color;
                egui::color_picker::color_edit_button_srgba(ui, &mut color, egui::color_picker::Alpha::Opaque).on_hover_cursor(egui::CursorIcon::PointingHand);
                if color != before {
                    result = Some(theme::to_hex(color));
                }
                widgets::label_font(ui, current_hex.to_uppercase(), theme.body(), if enabled { theme.secondary } else { theme.disabled_text });
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
        let mut reset = false;
        modal(ctx, theme, "xpr_color_config", "Font & Color Configuration", 460.0, |ui| {
            widgets::show_scroll(ui, egui::ScrollArea::vertical().max_height(fit_h(ui, 600.0, 170.0)), |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                ui.set_width(ui.available_width() - 16.0);
                section(ui, theme, "Font", None);
                ui.horizontal(|ui| {
                    ui.spacing_mut().interact_size.y = 28.0;
                    self.fonts.ui(ui, theme, ui.id().with("font"), Some(FIELD_W), true);
                    if small_button(ui, theme, "Apply Font", true).clicked() {
                        d.cfg.set_custom_font_name(self.fonts.get());
                        self.changed = true;
                    }
                });
                help(ui, theme, "If your font is not in the list, make sure it is installed on your system, then restart the program.");
                section(ui, theme, "Colors", None);
                let items: [(&str, String); 10] = [
                    ("Success", d.cfg.get_success_color().to_string()),
                    ("Warning", d.cfg.get_warning_color().to_string()),
                    ("Failure", d.cfg.get_failure_color().to_string()),
                    ("Divider", d.cfg.get_divider_color().to_string()),
                    ("Header", d.cfg.get_header_color().to_string()),
                    ("Primary", d.cfg.get_primary_color().to_string()),
                    ("Secondary", d.cfg.get_secondary_color().to_string()),
                    ("Contrast", d.cfg.get_contrast_color().to_string()),
                    ("Background", d.cfg.get_background_color().to_string()),
                    ("Text", d.cfg.get_text_color().to_string()),
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
                help(ui, theme, "After changing colors, restart the program for the changes to take effect.");
            });
            footer(ui, theme, |ui| {
                if action_button(ui, theme, "Close", Tone::Primary, true).clicked() || escape_pressed(ui) {
                    close = true;
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    if action_button(ui, theme, "Reset All Colors", Tone::Secondary, true).clicked() {
                        reset = true;
                    }
                });
            });
        });
        if reset {
            d.cfg.reset_all_colors();
            self.changed = true;
        }
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
        let mut reset = false;
        modal(ctx, theme, "xpr_highlight_colors", "Configure Highlight Colors", 460.0, |ui| {
            widgets::show_scroll(ui, egui::ScrollArea::vertical().max_height(fit_h(ui, 620.0, 170.0)), |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                ui.set_width(ui.available_width() - 16.0);
                section(ui, theme, "Highlight colors", None);
                for i in 1..=9i64 {
                    let cur = d.cfg.get_highlight_color(i);
                    if let Some(h) = color_updater(ui, theme, &format!("Highlight {}", i), &cur, true) {
                        d.cfg.set_highlight_color(i, &h);
                        refresh = true;
                    }
                }
                section(ui, theme, "Fight categories", None);
                let mut color_major = d.cfg.get_color_major_battles();
                if widgets::checkbox(ui, theme, &mut color_major, "Color major battles", true).changed() {
                    d.cfg.set_color_major_battles(color_major);
                    refresh = true;
                }
                for (cat, label) in FIGHT_CATEGORY_LABELS {
                    let cur = d.cfg.get_fight_category_color(cat);
                    if let Some(h) = color_updater(ui, theme, label.trim_end_matches(':'), &cur, color_major) {
                        d.cfg.set_fight_category_color(cat, &h);
                        refresh = true;
                    }
                }
            });
            footer(ui, theme, |ui| {
                if action_button(ui, theme, "Close", Tone::Primary, true).clicked() || escape_pressed(ui) {
                    close = true;
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    if action_button(ui, theme, "Reset to Defaults", Tone::Secondary, true).clicked() {
                        reset = true;
                    }
                });
            });
        });
        if reset {
            d.cfg.reset_highlight_colors_to_defaults();
            d.cfg.reset_fight_category_colors();
            refresh = true;
        }
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
        modal(ctx, theme, "xpr_app_config", "Application Settings", 560.0, |ui| {
            section(ui, theme, "About", None);
            form(ui, "app_cfg_about", |ui| {
                form_label(ui, theme, "Version");
                widgets::label_font(ui, consts::APP_VERSION, theme.body_bold(), theme.text_strong());
                ui.end_row();
                form_label(ui, theme, "Released");
                widgets::label_font(ui, consts::APP_RELEASE_DATE, theme.body(), theme.text_strong());
                ui.end_row();
            });

            section(ui, theme, "Updates", None);
            ui.horizontal(|ui| {
                ui.set_width(ui.available_width());
                widgets::label_font(ui, self.latest_version_text.clone(), theme.body(), theme.text);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (label, tone, enabled) = match &self.upgrade_available {
                        Some(_) => ("Upgrade", Tone::Primary, true),
                        None => ("No Upgrade Needed", Tone::Secondary, false),
                    };
                    if button_sized(ui, theme, label, tone, enabled, 26.0, 0.0, theme.body()).clicked() {
                        outcome = Some(DialogOutcome::RestartForUpdate);
                        close = true;
                    }
                });
            });
            help(ui, theme, "Automatic updates are only supported on Windows.");

            section(ui, theme, "Recording", None);
            if widgets::checkbox(ui, theme, &mut self.debug_mode, "Debug logging when recording", true).changed() {
                d.cfg.set_debug_mode(self.debug_mode);
            }

            section(ui, theme, "Storage", None);
            for (label, path, open, relocate) in [
                ("Data folder", d.cfg.get_user_data_dir(), &mut open_data, &mut move_data),
                ("Images folder", d.cfg.get_images_dir(), &mut open_images, &mut move_images),
            ] {
                form_label(ui, theme, label);
                ui.horizontal(|ui| {
                    let field_w = ui.available_width() - 130.0;
                    path_field(ui, theme, &path.display().to_string(), field_w);
                    if small_button(ui, theme, "Open", true).clicked() {
                        *open = true;
                    }
                    if small_button(ui, theme, "Move…", true).clicked() {
                        *relocate = true;
                    }
                });
            }
            if small_button(ui, theme, "Open Config & Logs Folder", true).clicked() {
                open_config = true;
            }
            footer(ui, theme, |ui| {
                if action_button(ui, theme, "Close", Tone::Primary, true).clicked() || enter_pressed(ui) || escape_pressed(ui) {
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
        let mut reset_all = false;
        let mut export = false;
        let mut import = false;
        modal(ctx, theme, "xpr_shortcuts", "Keyboard Shortcuts", 680.0, |ui| {
            field(theme, &mut self.search).width(ui.available_width()).hint("Search actions or keys").id(ui.id().with("search")).show(ui);
            let needle = self.search.to_lowercase();
            list_well(ui, theme, fit_h(ui, 440.0, 290.0), |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                egui::Frame::new().inner_margin(egui::Margin { left: 10, right: 14, top: 6, bottom: 8 }).show(ui, |ui| {
                egui::Grid::new("shortcuts_grid").num_columns(4).spacing(Vec2::new(16.0, 6.0)).min_col_width(0.0).show(ui, |ui| {
                    widgets::caption(ui, theme, "Action", None);
                    widgets::caption(ui, theme, "Shortcut", None);
                    widgets::caption(ui, theme, "Default", None);
                    ui.label("");
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
                        widgets::caption(ui, theme, category, Some(theme.header));
                        ui.end_row();
                        for id in visible {
                            let label = xpr_core::config::shortcut_label(id);
                            let default_seq = xpr_core::config::default_shortcut(id).to_string();
                            let cur = self.seq_of(id);
                            let is_custom = cur != default_seq;
                            let color = if is_custom { theme.primary } else { theme.text };
                            widgets::label_font(ui, label, theme.body(), color);
                            let capturing = self.capturing.as_deref() == Some(id);
                            let (rect, resp) = ui.allocate_exact_size(Vec2::new(150.0, 24.0), egui::Sense::click());
                            let (fill, stroke, shown, text_color) = if capturing {
                                (theme::tint(theme.accent, theme.well_bg(), 0.30), theme.accent, "Press keys…".to_string(), theme.text_strong())
                            } else if resp.hovered() {
                                (theme.hover_bg, theme::lighten(theme.border, 0.15), cur.clone(), if is_custom { theme.primary } else { theme.text_strong() })
                            } else {
                                (theme.bg_input, theme.card_border(), cur.clone(), if is_custom { theme.primary } else { theme.text_strong() })
                            };
                            ui.painter().rect(rect, CornerRadius::same(6), fill, Stroke::new(1.0_f32, stroke), egui::StrokeKind::Inside);
                            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, shown, theme.body(), text_color);
                            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                self.capturing = Some(id.to_string());
                            }
                            widgets::label_font(ui, default_seq.clone(), theme.body(), theme.secondary);
                            if !is_custom {
                                ui.allocate_exact_size(Vec2::new(56.0, 24.0), egui::Sense::hover());
                            } else if button_sized(ui, theme, "Reset", Tone::Secondary, true, 24.0, 56.0, theme.body()).clicked() {
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
            });
            help(ui, theme, "Click a shortcut, then press the new key combination. Changed shortcuts are highlighted.");
            footer(ui, theme, |ui| {
                let pending = !self.pending.is_empty();
                if action_button(ui, theme, "Apply", Tone::Primary, pending).clicked() && self.apply(d.cfg, message, false) {
                    outcome = Some(DialogOutcome::ApplyShortcuts);
                }
                if action_button(ui, theme, "Close", Tone::Secondary, true).clicked() || (escape_pressed(ui) && self.capturing.is_none()) {
                    close = true;
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    export = action_button(ui, theme, "Export…", Tone::Secondary, true).clicked();
                    import = action_button(ui, theme, "Import…", Tone::Secondary, true).clicked();
                    reset_all = action_button(ui, theme, "Reset All…", Tone::Secondary, true).clicked();
                });
            });
        });
        if reset_all {
            *message = Some(MessageBox::new("Reset All Shortcuts", "Reset all keyboard shortcuts to their defaults?", MsgButtons::YesNo, MsgTag::ResetAllShortcuts));
        }
        if export {
            if let Some(p) = rfd::FileDialog::new().set_title("Export Shortcut Profile").set_file_name("shortcuts.json").add_filter("JSON Files", &["json"]).save_file() {
                match d.cfg.export_shortcuts(&p) {
                    Ok(_) => *message = Some(MessageBox::new("Export", &format!("Shortcuts exported to:\n{}", p.display()), MsgButtons::Ok, MsgTag::Info)),
                    Err(e) => *message = Some(MessageBox::new("Export Error", &e.to_string(), MsgButtons::Ok, MsgTag::Info)),
                }
            }
        }
        if import {
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
        let mut clear_all = false;
        let mut reset = false;
        modal(ctx, theme, "xpr_final_trainers", "Configure Final Trainers", 560.0, |ui| {
            help(ui, theme, "Pick the trainers that mark the end of a run for each game. While recording, defeating any of them turns recording off, so no extra events are captured.");
            ui.add_space(2.0);
            form(ui, "final_trainers_form", |ui| {
                form_label(ui, theme, "Game version");
                if self.versions.ui(ui, theme, ui.id().with("version"), Some(FIELD_W + 40.0), true) {
                    repopulate = true;
                }
                ui.end_row();
                form_label(ui, theme, "Filter");
                field(theme, &mut self.filter).width(FIELD_W + 40.0).hint("Filter trainers by name").id(ui.id().with("filter")).show(ui);
                ui.end_row();
            });
            let sel = self.trainers.iter().filter(|(_, c)| *c).count();
            section(ui, theme, "Trainers", Some(&format!("{} of {} selected", sel, self.trainers.len())));
            let needle = self.filter.trim().to_lowercase();
            list_well(ui, theme, fit_h(ui, 340.0, 380.0), |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 6.0);
                egui::Frame::new().inner_margin(egui::Margin { left: 8, right: 8, top: 4, bottom: 4 }).show(ui, |ui| {
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
            footer(ui, theme, |ui| {
                if action_button(ui, theme, "Close", Tone::Primary, true).clicked() || escape_pressed(ui) {
                    close = true;
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    reset = action_button(ui, theme, "Reset to Defaults", Tone::Secondary, true).clicked();
                    clear_all = action_button(ui, theme, "Clear All", Tone::Secondary, sel > 0).clicked();
                });
            });
        });
        if clear_all {
            for (_, c) in self.trainers.iter_mut() {
                *c = false;
            }
            d.cfg.set_final_trainers(self.versions.get(), Vec::new());
        }
        if reset {
            d.cfg.reset_final_trainers(self.versions.get());
            repopulate = true;
        }
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
        modal(ctx, theme, "xpr_matchup_export", "Export Matchup", 400.0, |ui| {
            help(ui, theme, "Which graphic would you like to export?");
            for (label, detail, mode) in [
                ("Match Up", "Both sides of the matchup", crate::battle_ui::ScreenshotMode::Full),
                ("Player Ranges", "Your moves' damage ranges", crate::battle_ui::ScreenshotMode::Player),
                ("Enemy Ranges", "The enemy's damage ranges", crate::battle_ui::ScreenshotMode::Enemy),
            ] {
                if choice_row(ui, theme, label, detail).clicked() {
                    outcome = Some(DialogOutcome::MatchupExport { idx: self.mon_idx, mode });
                    close = true;
                }
            }
            footer(ui, theme, |ui| {
                if action_button(ui, theme, "Cancel", Tone::Secondary, true).clicked() || escape_pressed(ui) {
                    close = true;
                }
            });
        });
        (close, outcome)
    }
}

pub struct AssignMoveDialog {
    pub slot_idx: i64,
    /// True when opened from the Pre-Event State moves card rather than the
    /// Battle Summary tab: the Tutor event is inserted before the currently
    /// selected event instead of before the loaded trainer fight.
    pre_state: bool,
    moves: Vec<String>,
    filter: String,
    selected: Option<String>,
    focused: bool,
}

impl AssignMoveDialog {
    pub fn new(gen: &GenData, slot_idx: i64, pre_state: bool) -> AssignMoveDialog {
        AssignMoveDialog { slot_idx, pre_state, moves: gen.move_db().get_filtered_names(None, false), filter: String::new(), selected: None, focused: false }
    }

    pub fn ui(&mut self, ctx: &egui::Context, d: &mut DialogCtx) -> (bool, Option<DialogOutcome>) {
        let theme = d.theme;
        let mut close = false;
        let mut outcome = None;
        modal(ctx, theme, "xpr_assign_move", &format!("Assign Move to Slot {}", self.slot_idx + 1), 380.0, |ui| {
            let filter_id = ui.id().with("filter");
            if !self.focused {
                ui.memory_mut(|m| m.request_focus(filter_id));
            }
            let r = field(theme, &mut self.filter).width(ui.available_width()).hint("Search moves").id(filter_id).show(ui);
            // A brand-new Modal id runs an invisible egui "sizing pass" on the
            // frame it first appears, which silently drops any focus
            // requested that frame; keep asking until it actually lands.
            if !self.focused && r.has_focus {
                self.focused = true;
            }
            let needle = self.filter.trim().to_lowercase();
            let visible: Vec<String> = self.moves.iter().filter(|m| needle.is_empty() || m.to_lowercase().contains(&needle)).cloned().collect();
            list_well(ui, theme, fit_h(ui, 320.0, 280.0), |ui| {
                for m in &visible {
                    let is_sel = self.selected.as_deref() == Some(m.as_str());
                    let resp = list_row(ui, theme, m, is_sel);
                    if resp.clicked() {
                        self.selected = Some(m.clone());
                    }
                    if resp.double_clicked() {
                        outcome = Some(DialogOutcome::AssignMove { slot: self.slot_idx, mv: m.clone(), pre_state: self.pre_state });
                        close = true;
                    }
                }
                if visible.is_empty() {
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| widgets::label_font(ui, "No moves match", theme.body(), theme.secondary));
                }
            });
            if r.enter_pressed {
                let pick = self.selected.clone().or_else(|| visible.first().cloned());
                if let Some(m) = pick {
                    outcome = Some(DialogOutcome::AssignMove { slot: self.slot_idx, mv: m, pre_state: self.pre_state });
                    close = true;
                }
            }
            help(ui, theme, "Double-click a move, or press Enter to assign the selected (or first) match.");
            footer(ui, theme, |ui| {
                let can = self.selected.is_some();
                if action_button(ui, theme, "Assign", Tone::Primary, can).clicked() && can {
                    outcome = Some(DialogOutcome::AssignMove { slot: self.slot_idx, mv: self.selected.clone().unwrap(), pre_state: self.pre_state });
                    close = true;
                }
                if action_button(ui, theme, "Cancel", Tone::Secondary, true).clicked() || escape_pressed(ui) {
                    close = true;
                }
            });
        });
        (close, outcome)
    }
}

/// Opened by clicking the EV column of the Pre-Event State stats card: six
/// fields, prefilled with the mon's stat XP before the selected event, that
/// become an EV Override event inserted before it.
pub struct EvOverrideDialog {
    values: [String; 6],
    /// gens 1-2: one Special field, mirrored into Sp. Def
    gen12: bool,
}

impl EvOverrideDialog {
    pub fn new(gen: &GenData, current: [i64; 6]) -> EvOverrideDialog {
        EvOverrideDialog { values: current.map(|v| v.to_string()), gen12: EvOverrideEventDefinition::has_single_special(gen) }
    }

    fn parsed(&self) -> Option<[i64; 6]> {
        let mut out = [0i64; 6];
        for (slot, raw) in out.iter_mut().zip(self.values.iter()) {
            *slot = raw.trim().parse().ok()?;
        }
        if self.gen12 {
            out[4] = out[3];
        }
        Some(out)
    }

    pub fn ui(&mut self, ctx: &egui::Context, d: &mut DialogCtx) -> (bool, Option<DialogOutcome>) {
        let theme = d.theme;
        let mut close = false;
        let mut outcome = None;
        let (unit, cap) = if self.gen12 { ("Stat Exp", STAT_XP_CAP_GEN12) } else { ("EVs", SINGLE_STAT_EV_CAP) };
        let names = if self.gen12 { &EV_STAT_NAMES_GEN12 } else { &EV_STAT_NAMES };
        let title = if self.gen12 { "Override Stat Exp Before This Event" } else { "Override EVs Before This Event" };
        modal(ctx, theme, "xpr_ev_override", title, 360.0, |ui| {
            help(ui, theme, &format!("{} from this point on, 0 to {} per stat.", unit, cap));
            ui.add_space(2.0);
            let mut enter = false;
            egui::Grid::new(ui.id().with("ev_grid")).num_columns(2).spacing(Vec2::new(24.0, 8.0)).show(ui, |ui| {
                for &idx in ev_field_indices(self.gen12) {
                    form_label(ui, theme, names[idx]);
                    let r = AmountEntry::new(theme, ui.id().with(("ev_amt", idx)), &mut self.values[idx]).min(Some(0)).max(Some(cap)).width(Some(6)).show(ui);
                    enter |= r.enter_pressed;
                    ui.end_row();
                }
            });
            let parsed = self.parsed();
            if !self.gen12 {
                let total: i64 = parsed.map(|v| v.iter().sum()).unwrap_or(0);
                let color = if total > TOTAL_EV_CAP { theme.failure } else { theme.secondary };
                widgets::label_font(ui, format!("Total {} / {}", total, TOTAL_EV_CAP), theme.body(), color);
            }
            footer(ui, theme, |ui| {
                let can = parsed.is_some();
                if (action_button(ui, theme, "Create Override", Tone::Primary, can).clicked() || enter) && can {
                    outcome = Some(DialogOutcome::EvOverride(parsed.unwrap()));
                    close = true;
                }
                if action_button(ui, theme, "Cancel", Tone::Secondary, true).clicked() || escape_pressed(ui) {
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
            Dialog::EvOverride(x) => x.ui(ctx, d),
        }
    }
}

/// Little helper for keyboard-shortcut labels in menus.
pub fn label_for(action_id: &str) -> String {
    SHORTCUT_LABELS.iter().find(|(a, _)| *a == action_id).map(|(_, l)| l.to_string()).unwrap_or_else(|| action_id.to_string())
}

#[cfg(test)]
mod focus_tests {
    //! Regression test for the "Assign Move" filter box not gaining
    //! keyboard focus on open: a brand-new `Modal`/`Area` id runs an
    //! invisible egui "sizing pass" on the frame it first appears, which
    //! silently surrenders any focus requested that same frame. Requesting
    //! focus every frame until the widget actually reports having it (as
    //! `AssignMoveDialog::ui` now does) survives that first frame.
    use egui::{Context, FontDefinitions, Id, Modal, RawInput, TextEdit};

    #[test]
    fn naive_one_shot_request_never_lands() {
        let ctx = Context::default();
        ctx.set_fonts(FontDefinitions::empty());
        let id = Id::new("xpr_assign_move_test").with("filter");
        let mut text = String::new();
        let mut focused = false;
        for _ in 0..2 {
            let _ = ctx.run(RawInput::default(), |ctx| {
                Modal::new(Id::new("xpr_assign_move_test")).show(ctx, |ui| {
                    if !focused {
                        ui.memory_mut(|m| m.request_focus(id));
                        focused = true;
                    }
                    TextEdit::singleline(&mut text).id(id).show(ui);
                });
            });
        }
        assert!(!ctx.memory(|m| m.has_focus(id)), "the naive one-shot request was expected to lose the race against the sizing pass");
    }

    #[test]
    fn retrying_until_it_sticks_lands_focus() {
        let ctx = Context::default();
        ctx.set_fonts(FontDefinitions::empty());
        let id = Id::new("xpr_assign_move_test_fixed").with("filter");
        let mut text = String::new();
        let mut focused = false;
        for _ in 0..2 {
            let _ = ctx.run(RawInput::default(), |ctx| {
                Modal::new(Id::new("xpr_assign_move_test_fixed")).show(ctx, |ui| {
                    if !focused {
                        ui.memory_mut(|m| m.request_focus(id));
                    }
                    let out = TextEdit::singleline(&mut text).id(id).show(ui);
                    if !focused && out.response.has_focus() {
                        focused = true;
                    }
                });
            });
        }
        assert!(ctx.memory(|m| m.has_focus(id)), "expected focus to have been (re)acquired by the second frame");
    }
}
