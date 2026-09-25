//! The Route Compare page (`docs/rust_port/design/route_compare/SPEC.md`).
//!
//! Two routes are loaded into their own `Router`s on a background thread and
//! reduced to `xpr_engine::compare` digests; this module owns the page shell
//! (route pickers, tabs, banners) and delegates each tab to its own file.

mod checkpoints;
mod event_diff;
mod overview;
mod shared;

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

use egui::{Align, Color32, Layout, Rect, Sense, Ui, Vec2};

use xpr_core::io_utils;
use xpr_core::Paths;
use xpr_data::Registry;
use xpr_engine::compare::{compare, digest, DiffFilter, RouteComparison, RouteDigest, RouteOrigin};
use xpr_engine::Router;
use xpr_ui_kit::modal::behind_modal;
use xpr_ui_kit::theme::Theme;
use xpr_ui_kit::widgets::{self, Side, StyledButton};

use crate::assets::Assets;
use crate::route_index::RouteIndex;

pub const TAB_OVERVIEW: usize = 0;
pub const TAB_CHECKPOINTS: usize = 1;
pub const TAB_DIFF: usize = 2;

const TABS: [&str; 3] = ["Overview", "Checkpoints", "Event diff"];

/// Which route a picker or a loaded digest belongs to.
fn side_of(is_a: bool) -> Side {
    if is_a {
        Side::A
    } else {
        Side::B
    }
}

/// Where a route to compare comes from.
#[derive(Clone, Debug)]
pub enum RouteSource {
    Path(PathBuf),
    /// The route open in the editor, serialized from memory so unsaved edits
    /// are reflected (D9).
    Value { label: String, json: serde_json::Value },
}

impl RouteSource {
    pub fn label(&self) -> String {
        match self {
            RouteSource::Path(p) => p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
            RouteSource::Value { label, .. } => label.clone(),
        }
    }
}

/// What the page wants the app to do after a frame.
#[derive(Clone, Debug, Default)]
pub struct CompareActions {
    pub back: bool,
    pub export: bool,
    pub copy_summary: Option<String>,
    /// "Use the route open in the editor" was picked for slot A (`true`) or
    /// B. The app serializes the route once and calls [`CompareView::set_source`].
    pub use_current_route: Option<bool>,
}

/// Everything the page needs from the app that it does not own.
pub struct CompareEnv<'a> {
    pub registry: Arc<Registry>,
    pub paths: &'a Paths,
    pub index: &'a RouteIndex,
    /// The name of the route open in the editor, if any. Only the name: the
    /// route itself is serialized on request, never per frame.
    pub current_route_name: Option<String>,
}

struct Slot {
    source: Option<RouteSource>,
    digest: Option<RouteDigest>,
    error: Option<String>,
    /// The load in flight. Replacing it drops the old receiver, which is how
    /// a superseded load is discarded.
    rx: Option<Receiver<Result<RouteDigest, String>>>,
    picker_open: bool,
    /// Set for the frame the picker opens in: the click that opened it must
    /// not count as a click outside it, and the search box takes focus once.
    picker_just_opened: bool,
    /// Where the picker button was drawn, to anchor the popup under it.
    button_rect: Option<Rect>,
    search: String,
    game_filter: String,
}

impl Default for Slot {
    fn default() -> Slot {
        Slot {
            source: None,
            digest: None,
            error: None,
            rx: None,
            picker_open: false,
            picker_just_opened: false,
            button_rect: None,
            search: String::new(),
            game_filter: ALL_GAMES.to_string(),
        }
    }
}

impl Slot {
    fn loading(&self) -> bool {
        self.rx.is_some()
    }

    fn open_picker(&mut self) {
        self.picker_open = true;
        self.picker_just_opened = true;
        self.search.clear();
    }
}

const ALL_GAMES: &str = "All games";
/// Rows the picker lays out at most; the search box narrows the rest.
const MAX_PICKER_ROWS: usize = 400;

/// The per-tab state the body mutates while `comparison` is borrowed.
struct TabState {
    /// Checkpoints tab: major fights only, or every shared trainer.
    major_only: bool,
    expanded_checkpoint: Option<usize>,
    filters: Vec<DiffFilter>,
    /// Set by the Trainers card to jump to a fight in the event diff.
    scroll_to_trainer: Option<String>,
    /// A tab switch requested from inside the body.
    pending_tab: Option<usize>,
    /// How far the event diff is scrolled (a capped export starts there).
    diff_scroll_y: f32,
}

impl Default for TabState {
    fn default() -> TabState {
        TabState {
            major_only: true,
            expanded_checkpoint: None,
            filters: DiffFilter::ALL.iter().copied().filter(|f| f.default_on()).collect(),
            scroll_to_trainer: None,
            pending_tab: None,
            diff_scroll_y: 0.0,
        }
    }
}

pub struct CompareView {
    a: Slot,
    b: Slot,
    comparison: Option<RouteComparison>,
    /// Set when the digests changed and `comparison` must be rebuilt.
    dirty: bool,
    pub tab: usize,
    pub highlight_differences: bool,
    state: TabState,
    /// The UI context (known from the first `poll`): a finished loader
    /// wakes the page instead of waiting for the next poll timer.
    wake_ctx: Option<egui::Context>,
}

impl Default for CompareView {
    fn default() -> CompareView {
        CompareView {
            a: Slot::default(),
            b: Slot::default(),
            comparison: None,
            dirty: false,
            tab: TAB_OVERVIEW,
            highlight_differences: true,
            state: TabState::default(),
            wake_ctx: None,
        }
    }
}

impl CompareView {
    pub fn new() -> CompareView {
        CompareView::default()
    }

    /// Open the page, optionally pre-filling slot A.
    pub fn open(&mut self, a: Option<RouteSource>, env: &CompareEnv) {
        if let Some(src) = a {
            // The editor's route is always re-read: it may have been edited
            // since the page was last open (D9). A file is only reloaded when
            // it is a different one.
            let same_file = matches!(&src, RouteSource::Path(_)) && self.a.source.as_ref().map(|s| s.label()) == Some(src.label()) && self.a.digest.is_some();
            if !same_file {
                self.set_source(true, src, env);
            }
        }
        if self.b.source.is_none() {
            self.b.open_picker();
        }
    }

    /// Whether slot A's (`true`) or B's route picker is showing.
    pub fn picker_open(&self, is_a: bool) -> bool {
        if is_a {
            self.a.picker_open
        } else {
            self.b.picker_open
        }
    }

    /// Where a slot's picker button was last drawn.
    pub fn picker_button_rect(&self, is_a: bool) -> Option<Rect> {
        if is_a {
            self.a.button_rect
        } else {
            self.b.button_rect
        }
    }

    /// Whether either slot is still loading its route.
    pub fn is_loading(&self) -> bool {
        self.a.loading() || self.b.loading()
    }

    pub fn has_comparison(&self) -> bool {
        self.comparison.is_some()
    }

    pub fn comparison(&self) -> Option<&RouteComparison> {
        self.comparison.as_ref()
    }

    /// Load one slot in the background (SPEC §5.5).
    pub fn set_source(&mut self, is_a: bool, src: RouteSource, env: &CompareEnv) {
        let slot = if is_a { &mut self.a } else { &mut self.b };
        slot.source = Some(src.clone());
        slot.digest = None;
        slot.error = None;
        self.comparison = None;

        let origin = match &src {
            RouteSource::Value { .. } => RouteOrigin::CurrentRoute,
            RouteSource::Path(p) => match std::fs::metadata(p).and_then(|m| m.modified()) {
                Ok(t) => RouteOrigin::Saved {
                    mtime: t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0),
                },
                Err(_) => RouteOrigin::External,
            },
        };
        let registry = env.registry.clone();
        let (tx, rx) = channel();
        let wake_ctx = self.wake_ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(load_digest(registry, src, origin));
            if let Some(ctx) = wake_ctx {
                ctx.request_repaint();
            }
        });
        let slot = if is_a { &mut self.a } else { &mut self.b };
        slot.rx = Some(rx);
    }

    /// `XPR_SMOKE_ACTION=compare` only: expand the nth shown checkpoint.
    pub fn expand_checkpoint_for_smoke(&mut self, nth: usize) {
        let Some(cmp) = &self.comparison else { return };
        let major_only = self.state.major_only;
        if let Some(cp) = cmp
            .checkpoints
            .iter()
            .filter(|cp| !major_only || cmp.a.entries[cp.a].is_major || cmp.b.entries[cp.b].is_major)
            .nth(nth)
        {
            self.state.expanded_checkpoint = Some(cp.a);
        }
    }

    /// Drain the loader channels; call once per frame while the page is up.
    pub fn poll(&mut self, ctx: &egui::Context) {
        if self.wake_ctx.is_none() {
            self.wake_ctx = Some(ctx.clone());
        }
        let mut changed = false;
        for slot in [&mut self.a, &mut self.b] {
            let Some(rx) = &slot.rx else { continue };
            match rx.try_recv() {
                Ok(Ok(d)) => {
                    slot.digest = Some(d);
                    slot.error = None;
                    slot.rx = None;
                    changed = true;
                }
                Ok(Err(e)) => {
                    slot.error = Some(e);
                    slot.digest = None;
                    slot.rx = None;
                    changed = true;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(50));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    slot.error = Some("The route could not be loaded.".to_string());
                    slot.rx = None;
                    changed = true;
                }
            }
        }
        if changed {
            self.dirty = true;
        }
        if self.dirty && !self.a.loading() && !self.b.loading() {
            self.dirty = false;
            self.comparison = match (&self.a.digest, &self.b.digest) {
                (Some(a), Some(b)) => {
                    let mut cmp = compare(a.clone(), b.clone());
                    // Same *file*, not merely the same file name: another
                    // player's route is often called what yours is.
                    cmp.compat.same_file = match (&self.a.source, &self.b.source) {
                        (Some(RouteSource::Path(x)), Some(RouteSource::Path(y))) => x == y,
                        (Some(RouteSource::Value { .. }), Some(RouteSource::Value { .. })) => true,
                        _ => false,
                    };
                    Some(cmp)
                }
                _ => None,
            };
            self.state.expanded_checkpoint = None;
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, theme: &Theme, cfg: &xpr_core::Config, assets: &mut Assets, env: &CompareEnv) -> CompareActions {
        let mut actions = CompareActions::default();
        self.header(ui, theme, env, &mut actions);
        self.tab_strip(ui, theme);
        self.body(ui, theme, cfg, assets);
        actions
    }

    /// Draw the active tab alone, for the screenshot export (SPEC §8.1):
    /// a title strip naming both routes, then the tab's content, with no
    /// header controls or scroll viewport. Returns the content rect to crop to.
    pub fn export_ui(&mut self, ui: &mut Ui, theme: &Theme, cfg: &xpr_core::Config, assets: &mut Assets) -> Rect {
        let Some(cmp) = self.comparison.as_ref() else { return Rect::NOTHING };
        let highlight = self.highlight_differences;
        let inner = egui::Frame::new().inner_margin(egui::Margin::same(16)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 12.0;
            // title strip: [A] name   vs   [B] name
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                widgets::route_badge(ui, theme, Side::A);
                ui.label(egui::RichText::new(&cmp.a.label).font(theme.body_bold()).color(theme.text_strong()));
                ui.label(egui::RichText::new("vs").font(theme.body()).color(theme.secondary));
                widgets::route_badge(ui, theme, Side::B);
                ui.label(egui::RichText::new(&cmp.b.label).font(theme.body_bold()).color(theme.text_strong()));
            });
            for text in banners(cmp) {
                widgets::warning_banner(ui, theme, &text);
            }
            match self.tab {
                TAB_CHECKPOINTS => checkpoints::ui(ui, theme, cfg, cmp, highlight, &mut self.state.major_only, &mut self.state.expanded_checkpoint),
                TAB_DIFF => event_diff::export_ui(ui, theme, cmp, highlight, &self.state.filters, self.state.diff_scroll_y),
                _ => {
                    overview::ui(ui, theme, assets, cmp, highlight);
                }
            }
        });
        inner.response.rect
    }

    // -- header ------------------------------------------------------------

    fn header(&mut self, ui: &mut Ui, theme: &Theme, env: &CompareEnv, actions: &mut CompareActions) {
        let ready = self.comparison.is_some();
        egui::Frame::new()
            .fill(theme.strip_bg())
            .inner_margin(egui::Margin { left: 16, right: 16, top: 0, bottom: 0 })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.set_height(56.0);
                    ui.spacing_mut().item_spacing.x = 12.0;
                    if StyledButton::new(theme, "\u{2039} Back").min_size(Vec2::new(0.0, 28.0)).show(ui).clicked() {
                        actions.back = true;
                    }
                    widgets::caption(ui, theme, "Compare routes", Some(theme.secondary));
                    let pick_a = self.picker_button(ui, theme, true);
                    let swap_enabled = self.a.digest.is_some() && self.b.digest.is_some();
                    let swap = StyledButton::new(theme, "")
                        .min_size(Vec2::new(28.0, 28.0))
                        .padding(Vec2::ZERO)
                        .enabled(swap_enabled)
                        .show(ui)
                        .on_hover_text("Swap A and B");
                    widgets::paint_swap_icon(ui, swap.rect, if swap_enabled { theme.text } else { theme.disabled_text });
                    let pick_b = self.picker_button(ui, theme, false);
                    if swap.clicked() {
                        std::mem::swap(&mut self.a, &mut self.b);
                        self.dirty = true;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if StyledButton::new(theme, "Export screenshot")
                            .min_size(Vec2::new(0.0, 28.0))
                            .enabled(ready)
                            .show(ui)
                            .clicked()
                        {
                            actions.export = true;
                        }
                        if StyledButton::new(theme, "Copy summary")
                            .min_size(Vec2::new(0.0, 28.0))
                            .enabled(ready)
                            .show(ui)
                            .clicked()
                        {
                            if let Some(cmp) = &self.comparison {
                                actions.copy_summary = Some(xpr_engine::compare::text_summary(cmp));
                            }
                        }
                    });
                    // Clicking an open picker's button closes it again.
                    if pick_a {
                        let was_open = self.a.picker_open;
                        self.a.picker_open = false;
                        self.b.picker_open = false;
                        if !was_open {
                            self.a.open_picker();
                        }
                    }
                    if pick_b {
                        let was_open = self.b.picker_open;
                        self.a.picker_open = false;
                        self.b.picker_open = false;
                        if !was_open {
                            self.b.open_picker();
                        }
                    }
                });
            });
        widgets::hairline(ui, theme.pane_divider());
        self.picker_popup(ui, theme, env, true, actions);
        self.picker_popup(ui, theme, env, false, actions);
    }

    /// The 330 x 36 route button. Returns true when it was clicked.
    fn picker_button(&mut self, ui: &mut Ui, theme: &Theme, is_a: bool) -> bool {
        let side = side_of(is_a);
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(330.0, 36.0), Sense::click());
        if is_a {
            self.a.button_rect = Some(rect);
        } else {
            self.b.button_rect = Some(rect);
        }
        let slot = if is_a { &self.a } else { &self.b };
        if ui.is_rect_visible(rect) {
            let bg = if resp.hovered() || slot.picker_open { theme.hover_bg } else { theme.well_bg() };
            ui.painter().rect(rect, egui::CornerRadius::same(4), bg, egui::Stroke::new(1.0_f32, theme.border), egui::StrokeKind::Inside);
            let badge = Rect::from_min_size(rect.min + Vec2::new(10.0, 8.0), Vec2::splat(20.0));
            widgets::paint_route_badge(ui, theme, badge, side);

            let text_x = badge.max.x + 8.0;
            let avail = rect.max.x - 20.0 - text_x;
            let (name, sub) = match (&slot.source, &slot.digest, &slot.error, slot.loading()) {
                (Some(src), _, _, true) => (src.label(), "Loading\u{2026}".to_string()),
                (Some(src), _, Some(_), _) => (src.label(), "Could not be loaded".to_string()),
                (_, Some(d), _, _) => (d.label.clone(), describe_origin(d)),
                (Some(src), _, _, _) => (src.label(), String::new()),
                _ => (
                    format!("Choose route {}\u{2026}", side.letter()),
                    "Saved routes, or any route file".to_string(),
                ),
            };
            let name_color = if slot.digest.is_some() || slot.loading() { theme.text_strong() } else { theme.secondary };
            let font = if slot.digest.is_some() { theme.body_bold() } else { theme.body() };
            let name = widgets::elide(ui, &name, &font, avail);
            widgets::col_text(
                ui,
                Rect::from_min_size(egui::Pos2::new(text_x, rect.min.y + 5.0), Vec2::new(avail, 14.0)),
                &name,
                font,
                name_color,
                Align::Min,
            );
            let sub = widgets::elide(ui, &sub, &theme.caption_font(), avail);
            widgets::col_text(
                ui,
                Rect::from_min_size(egui::Pos2::new(text_x, rect.min.y + 19.0), Vec2::new(avail, 12.0)),
                &sub,
                theme.caption_font(),
                theme.secondary,
                Align::Min,
            );
            let chev = Rect::from_center_size(egui::Pos2::new(rect.max.x - 12.0, rect.center().y), Vec2::splat(10.0));
            widgets::paint_chevron(ui, chev, false, theme.secondary);
        }
        resp.clicked()
    }

    fn picker_popup(&mut self, ui: &mut Ui, theme: &Theme, env: &CompareEnv, is_a: bool, actions: &mut CompareActions) {
        let slot = if is_a { &self.a } else { &self.b };
        if !slot.picker_open {
            return;
        }
        let just_opened = slot.picker_just_opened;
        let mut chosen: Option<RouteSource> = None;
        let mut close = false;
        let id = egui::Id::new(("compare_picker", is_a));
        // Anchored under its own button, kept inside the window.
        let button = slot.button_rect.unwrap_or_else(|| Rect::from_min_size(ui.max_rect().min, Vec2::new(330.0, 36.0)));
        let screen = ui.ctx().content_rect();
        let x = button.min.x.min(screen.max.x - 444.0).max(screen.min.x + 4.0);
        let area = egui::Area::new(id).order(egui::Order::Foreground).fixed_pos(egui::Pos2::new(x, button.max.y + 4.0));
        let response = area.show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(theme.card_bg())
                .stroke(egui::Stroke::new(1.0_f32, theme.card_border()))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::same(10))
                .shadow(egui::epaint::Shadow {
                    offset: [0, 4],
                    blur: 12,
                    spread: 0,
                    color: Color32::from_black_alpha(96),
                })
                .show(ui, |ui| {
                    ui.set_width(420.0);
                    if just_opened {
                        // Start on the other route's game: that is nearly
                        // always what is being compared against (SPEC §4.3).
                        let other_game = if is_a { self.b.digest.as_ref() } else { self.a.digest.as_ref() }.map(|d| d.version.clone());
                        let slot = if is_a { &mut self.a } else { &mut self.b };
                        slot.game_filter = other_game.unwrap_or_else(|| ALL_GAMES.to_string());
                    }
                    let slot = if is_a { &mut self.a } else { &mut self.b };
                    ui.horizontal(|ui| {
                        let e = widgets::Entry::new(theme, &mut slot.search).width(250.0).hint("Search routes\u{2026}").show(ui);
                        // Focus the search box once, when the popup opens.
                        // Asking every frame would take the focus back from
                        // the game filter beside it.
                        if just_opened {
                            if let Some(r) = &e.response {
                                r.request_focus();
                            }
                        }
                        let mut games: Vec<String> = vec![ALL_GAMES.to_string()];
                        games.extend(env.registry.get_gen_names(true, true));
                        widgets::option_menu(ui, theme, id.with("game"), &mut slot.game_filter, &games, Some(140.0), true);
                    });
                    ui.add_space(6.0);

                    let other = if is_a { &self.b } else { &self.a };
                    let other_label = other.digest.as_ref().map(|d| d.label.clone());
                    let other_path = match &other.source {
                        Some(RouteSource::Path(p)) => Some(p.clone()),
                        _ => None,
                    };
                    let other_source_is_editor = other.source.as_ref().map(|s| matches!(s, RouteSource::Value { .. }));
                    let slot = if is_a { &mut self.a } else { &mut self.b };
                    let needle = slot.search.to_lowercase();
                    let mut rows: Vec<&crate::route_index::IndexEntry> = env
                        .index
                        .entries
                        .values()
                        .filter(|e| needle.is_empty() || e.name.to_lowercase().contains(&needle))
                        .filter(|e| slot.game_filter == ALL_GAMES || e.version == slot.game_filter)
                        .collect();
                    rows.sort_by(|x, y| y.mtime.partial_cmp(&x.mtime).unwrap_or(std::cmp::Ordering::Equal));

                    widgets::show_scroll(ui, egui::ScrollArea::vertical().id_salt(id.with("list")).max_height(300.0), |ui| {
                        if rows.is_empty() {
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new("No saved routes match.").font(theme.body()).color(theme.secondary));
                        }
                        for e in rows.iter().take(MAX_PICKER_ROWS) {
                            let taken = other_path.as_ref() == Some(&io_utils::get_existing_route_path(env.paths, &e.name));
                            let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), if taken { Sense::hover() } else { Sense::click() });
                            if ui.is_rect_visible(rect) {
                                if resp.hovered() && !taken {
                                    ui.painter().rect_filled(rect, egui::CornerRadius::same(4), theme.hover_bg);
                                }
                                let fg = if taken { theme.disabled_text } else { theme.text };
                                let name_w = rect.width() - 150.0;
                                let name = widgets::elide(ui, &e.name, &theme.body(), name_w);
                                widgets::col_text(ui, Rect::from_min_size(rect.min + Vec2::new(6.0, 0.0), Vec2::new(name_w, rect.height())), &name, theme.body(), fg, Align::Min);
                                let meta = if taken { format!("{} \u{b7} already chosen", e.version) } else { format!("{} \u{b7} {}", e.version, e.species) };
                                widgets::col_text(
                                    ui,
                                    Rect::from_min_size(egui::Pos2::new(rect.max.x - 146.0, rect.min.y), Vec2::new(140.0, rect.height())),
                                    &meta,
                                    theme.caption_font(),
                                    theme.secondary,
                                    Align::Max,
                                );
                            }
                            if resp.clicked() {
                                chosen = Some(RouteSource::Path(io_utils::get_existing_route_path(env.paths, &e.name)));
                                close = true;
                            }
                        }
                        if rows.len() > MAX_PICKER_ROWS {
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(format!("{} more \u{2014} search to narrow the list.", rows.len() - MAX_PICKER_ROWS))
                                    .font(theme.caption_font())
                                    .color(theme.secondary),
                            );
                        }
                    });

                    ui.add_space(6.0);
                    widgets::hairline(ui, theme.row_divider());
                    ui.add_space(6.0);
                    if let Some(name) = &env.current_route_name {
                        let already = matches!(other_source_is_editor, Some(true)) && other_label.as_deref() == Some(name.as_str());
                        if !already
                            && StyledButton::new(theme, "Use the route open in the editor")
                                .min_size(Vec2::new(ui.available_width(), 26.0))
                                .show(ui)
                                .clicked()
                        {
                            actions.use_current_route = Some(is_a);
                            close = true;
                        }
                    }
                    if StyledButton::new(theme, "Browse for a file\u{2026}")
                        .min_size(Vec2::new(ui.available_width(), 26.0))
                        .show(ui)
                        .clicked()
                    {
                        if let Some(p) = rfd::FileDialog::new()
                            .set_title("Choose a route file")
                            .add_filter("Route", &["json"])
                            .set_directory(&env.paths.saved_routes_dir)
                            .pick_file()
                        {
                            chosen = Some(RouteSource::Path(p));
                        }
                        close = true;
                    }
                });
        });

        // A click closes the popup only when it lands on the page itself. The
        // click that opened the popup is still "this frame's click", and the
        // game filter's dropdown lives in a foreground layer of its own, so
        // neither may count as outside.
        // Neither counts while a dialog is above the page (it gets them).
        let ctx = ui.ctx().clone();
        let blocked = behind_modal(ui);
        let clicked_outside = !just_opened
            && !blocked
            && ctx.input(|i| i.pointer.any_click())
            && ctx
                .input(|i| i.pointer.interact_pos())
                .map(|pos| !response.response.rect.contains(pos) && ctx.layer_id_at(pos).map(|l| l.order < egui::Order::Foreground).unwrap_or(true))
                .unwrap_or(false);
        // Esc closes the popup and is consumed, so it does not also leave the page.
        let escaped = !blocked && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if clicked_outside || escaped {
            close = true;
        }
        if let Some(src) = chosen {
            self.set_source(is_a, src, env);
        }
        let slot = if is_a { &mut self.a } else { &mut self.b };
        slot.picker_just_opened = false;
        if close {
            slot.picker_open = false;
        }
    }

    // -- tabs --------------------------------------------------------------

    fn tab_strip(&mut self, ui: &mut Ui, theme: &Theme) {
        let ready = self.comparison.is_some();
        let mut tab = self.tab;
        egui::Frame::new()
            .inner_margin(egui::Margin { left: 16, right: 16, top: 0, bottom: 0 })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add_enabled_ui(ready, |ui| {
                    widgets::tab_bar(ui, theme, &TABS, &mut tab, |ui| {
                        widgets::checkbox_label(ui, theme, &mut self.highlight_differences, "Highlight differences", false, true);
                    });
                });
            });
        if ready {
            self.tab = tab;
        }
        widgets::hairline(ui, theme.pane_divider());
    }

    // -- body --------------------------------------------------------------

    fn body(&mut self, ui: &mut Ui, theme: &Theme, cfg: &xpr_core::Config, assets: &mut Assets) {
        // One slot failed to load: say so and keep the other.
        let errors: Vec<(Side, String)> = [(Side::A, &self.a), (Side::B, &self.b)]
            .into_iter()
            .filter_map(|(s, slot)| slot.error.clone().map(|e| (s, e)))
            .collect();

        if self.comparison.is_none() {
            egui::Frame::new().inner_margin(egui::Margin::same(16)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                for (side, err) in &errors {
                    let label = if *side == Side::A { "route A" } else { "route B" };
                    failure_banner(ui, theme, &format!("Could not load {}: {}", label, err));
                    ui.add_space(6.0);
                }
                if self.a.loading() || self.b.loading() {
                    ui.add_space(80.0);
                    ui.vertical_centered(|ui| {
                        ui.add(egui::Spinner::new().size(24.0));
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("Loading\u{2026}").font(theme.body()).color(theme.secondary));
                    });
                } else if errors.is_empty() {
                    empty_state(ui, theme);
                }
            });
            return;
        }

        let cmp = self.comparison.as_ref().expect("checked above");
        let highlight = self.highlight_differences;
        let tab = self.tab;
        // The event diff fills the page and scrolls internally; the other two
        // tabs scroll as a whole. Nesting the two would break both.
        let scrolls_here = tab != TAB_DIFF;
        let mut draw = |ui: &mut Ui, view: &mut TabState| {
            egui::Frame::new().inner_margin(egui::Margin::same(16)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 12.0;
                for (side, err) in &errors {
                    let label = if *side == Side::A { "route A" } else { "route B" };
                    failure_banner(ui, theme, &format!("Could not load {}: {}", label, err));
                }
                for text in banners(cmp) {
                    widgets::warning_banner(ui, theme, &text);
                }
                match tab {
                    TAB_CHECKPOINTS => {
                        checkpoints::ui(ui, theme, cfg, cmp, highlight, &mut view.major_only, &mut view.expanded_checkpoint);
                    }
                    TAB_DIFF => {
                        event_diff::ui(ui, theme, cmp, highlight, &mut view.filters, &mut view.scroll_to_trainer, &mut view.diff_scroll_y);
                    }
                    _ => {
                        if let Some(name) = overview::ui(ui, theme, assets, cmp, highlight) {
                            view.scroll_to_trainer = Some(name);
                            view.pending_tab = Some(TAB_DIFF);
                        }
                    }
                }
            });
        };
        // `self` is borrowed by `cmp`, so the tab state travels in a clone-free
        // side struct that is merged back below.
        let mut view = std::mem::take(&mut self.state);
        if scrolls_here {
            widgets::show_scroll(ui, egui::ScrollArea::vertical().id_salt("compare_body").auto_shrink([false, false]), |ui| {
                draw(ui, &mut view);
            });
        } else {
            draw(ui, &mut view);
        }
        self.state = view;
        if let Some(t) = self.state.pending_tab.take() {
            self.tab = t;
        }
    }
}

/// The banner texts that apply to a comparison (SPEC §4.5).
fn banners(cmp: &RouteComparison) -> Vec<String> {
    let (a, b) = (&cmp.a, &cmp.b);
    let mut out = Vec::new();
    if !cmp.compat.same_version {
        out.push(format!(
            "Different games: A is {}, B is {}. Trainers are matched by name and team, so some fights may not line up.",
            a.version, b.version
        ));
    }
    if !cmp.compat.same_generation {
        out.push(format!(
            "Different generations: stats, EVs and DVs/IVs are not comparable between Gen {} and Gen {}. The Pokémon setup and stats cards are hidden.",
            a.generation, b.generation
        ));
    }
    if !cmp.compat.same_species {
        out.push(format!("Different Pokémon: A is {}, B is {}.", a.species, b.species));
    }
    let (ea, eb) = (a.totals.events_with_errors, b.totals.events_with_errors);
    if ea > 0 || eb > 0 {
        out.push(format!(
            "Route errors: A has {} event{} with errors and B has {}. Totals after the first error may be off.",
            ea,
            if ea == 1 { "" } else { "s" },
            eb
        ));
    }
    if cmp.compat.same_file {
        out.push("Same route on both sides.".to_string());
    }
    out
}

/// A warning banner in the failure colours.
fn failure_banner(ui: &mut Ui, theme: &Theme, message: &str) {
    let msg = ui.fonts_mut(|f| f.layout(message.to_string(), theme.body(), theme.failure, ui.available_width() - 60.0));
    let h = (msg.size().y + 16.0).max(34.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().rect(
            rect,
            egui::CornerRadius::same(6),
            theme.tinted_bg("Failure", 0.12),
            egui::Stroke::new(1.0_f32, theme.tinted_bg("Failure", 0.32)),
            egui::StrokeKind::Inside,
        );
        ui.painter().galley(egui::Pos2::new(rect.min.x + 14.0, rect.center().y - msg.size().y / 2.0), msg, theme.failure);
    }
}

fn empty_state(ui: &mut Ui, theme: &Theme) {
    ui.add_space(90.0);
    ui.vertical_centered(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(60.0, 44.0), Sense::hover());
        if ui.is_rect_visible(rect) {
            let stroke = egui::Stroke::new(1.5_f32, theme.icon_stroke());
            for i in 0..2 {
                let r = Rect::from_min_size(rect.min + Vec2::new(i as f32 * 20.0, i as f32 * 4.0), Vec2::new(30.0, 38.0));
                ui.painter().rect_stroke(r, egui::CornerRadius::same(3), stroke, egui::StrokeKind::Inside);
            }
        }
        ui.add_space(12.0);
        ui.label(egui::RichText::new("Pick two routes to compare").font(theme.font_bold(10.5)).color(theme.text_strong()));
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Choose route A and route B above. Routes can be from your saved routes or any route file on disk.")
                .font(theme.body())
                .color(theme.secondary),
        );
    });
}

fn describe_origin(d: &RouteDigest) -> String {
    let origin = match &d.origin {
        RouteOrigin::CurrentRoute => "current route".to_string(),
        RouteOrigin::External => "external file".to_string(),
        RouteOrigin::Saved { mtime } => match chrono::DateTime::from_timestamp(*mtime as i64, 0) {
            Some(t) => format!("saved {}", t.format("%Y-%m-%d")),
            None => "saved".to_string(),
        },
    };
    format!("{} \u{b7} {} \u{b7} {}", d.version, d.species, origin)
}

/// Load one route into its own `Router` and digest it (SPEC §5.5).
fn load_digest(registry: Arc<Registry>, src: RouteSource, origin: RouteOrigin) -> Result<RouteDigest, String> {
    let mut router = Router::new(registry);
    let label = src.label();
    match &src {
        RouteSource::Path(p) => router.load(p, false)?,
        RouteSource::Value { json, .. } => router.load_value(json, false)?,
    }
    Ok(digest(&router, &label, origin))
}
