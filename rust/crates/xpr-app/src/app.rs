//! The main window (`gui_qt/main_window.py`): menus, shortcuts, pages,
//! the route editor splitter, status bar, dialogs, secondary windows,
//! recorder wiring, startup sequencing and the update flow.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use xpr_core::consts;
use xpr_core::io_utils;
use xpr_core::{Config, Paths};
use xpr_data::Registry;
use xpr_engine::{NodeId, ObjKind};
use xpr_map::LinkQuery;
use xpr_recorder::{starter, QuickStart, QuickStartPhase, StarterInfo};
use xpr_ui_kit::theme::{self, Theme};
use xpr_ui_kit::widgets::{self, Entry};
use xpr_ui_kit::{AutoClearingLabel, Geometry, ShortcutMap, ToastHost};

use crate::assets::Assets;
use crate::battle_ui::ScreenshotMode;
use crate::compare::{CompareEnv, CompareView, RouteSource};
use crate::controller::MainController;
use crate::dialogs::{
    AppConfigDialog, AssignMoveDialog, BattleConfigDialog, ColorConfigDialog, CustomDvsDialog, CustomGenDialog, Dialog, EvOverrideDialog,
    DialogCtx, DialogOutcome, FinalTrainersDialog, HighlightColorDialog, LoadRouteDialog, MatchupExportDialog, MessageBox,
    MsgButtons, MsgChoice, MsgTag, NewFolderDialog, ShortcutsDialog, TransferDialog,
};
use crate::event_details::{DetailsActions, EventDetails, BATTLE_SUMMARY_TAB, MAP_TAB, PRE_STATE_TAB};
use crate::filter_bar::FilterBar;
use crate::map::{MapAction, MapView};
use crate::pages::{quick_start_ui, LandingActions, LandingPage, NewRouteActions, NewRoutePage, QuickStartActions};
use crate::quick_add::QuickAddPopover;
use crate::recorder_glue::{gamehook_url, Recorder};
use crate::route_index::RouteIndex;
use crate::route_list::{ListActions, RouteList};
use crate::battle_ui::BattleUiActions;
use crate::screenshot::{rect_to_pixels, save_cropped, Offscreen, ShotKind, CARD_RADIUS};
use crate::summaries::{setup_summary_text, RunSummary};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Landing,
    NewRoute,
    Editor,
    Compare,
}

/// What `main` needs to know after the window closes.
#[derive(Clone, Debug, Default)]
pub struct ExitState {
    pub update_requested: bool,
    pub update_version: Option<String>,
    pub update_url: Option<String>,
}

pub type SharedExit = Arc<std::sync::Mutex<ExitState>>;

/// The start-up work that needs no window: the route index scan, the custom
/// gens and the update check. `main` spawns it before the window exists, so
/// it runs while the OS creates the window and the GL context.
pub struct StartupWork {
    index_rx: Receiver<(RouteIndex, bool)>,
    custom_gens_rx: Receiver<Result<(), String>>,
    update_rx: Receiver<xpr_update::ReleaseInfo>,
    /// the UI context once there is one: a finished thread requests a
    /// repaint so its result is not left waiting on a poll timer
    waker: Arc<std::sync::OnceLock<egui::Context>>,
}

impl StartupWork {
    pub fn spawn(paths: &Paths, registry: &Arc<Registry>) -> StartupWork {
        let waker: Arc<std::sync::OnceLock<egui::Context>> = Arc::new(std::sync::OnceLock::new());
        let wake_with = |waker: &Arc<std::sync::OnceLock<egui::Context>>| {
            let waker = waker.clone();
            move || {
                if let Some(ctx) = waker.get() {
                    ctx.request_repaint();
                }
            }
        };
        let (tx, index_rx) = channel();
        let scan_paths = paths.clone();
        let wake = wake_with(&waker);
        std::thread::spawn(move || {
            let t0 = Instant::now();
            let mut idx = RouteIndex::load(&scan_paths);
            let t_load = t0.elapsed();
            if idx.loaded {
                let _ = tx.send((idx.clone(), false));
                wake();
            }
            let changed = idx.refresh(&scan_paths);
            log::info!("route index: {} routes; load {:.0} ms, refresh {:.0} ms{}", idx.entries.len(), t_load.as_secs_f64() * 1000.0, (t0.elapsed() - t_load).as_secs_f64() * 1000.0, if changed { " (changed, re-saved)" } else { "" });
            let _ = tx.send((idx, true));
            wake();
        });
        let (ctx_tx, custom_gens_rx) = channel();
        let scan_registry = registry.clone();
        let wake = wake_with(&waker);
        std::thread::spawn(move || {
            let t0 = Instant::now();
            let r = scan_registry.reload_all_custom_gens();
            log::info!("custom gens loaded in {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);
            let _ = ctx_tx.send(r);
            wake();
        });
        let (utx, update_rx) = channel();
        let wake = wake_with(&waker);
        std::thread::spawn(move || {
            // `XPR_DISABLE_AUTO_UPDATE`: no request to GitHub at start
            // (isolated / offline test runs), so the check just reports "no
            // release information" and nothing is prompted.
            let info = if std::env::var_os("XPR_DISABLE_AUTO_UPDATE").is_some() { xpr_update::ReleaseInfo::default() } else { xpr_update::get_new_version_info() };
            let _ = utx.send(info);
            wake();
        });
        StartupWork { index_rx, custom_gens_rx, update_rx, waker }
    }
}

struct UpdateState {
    rx: Option<Receiver<xpr_update::ReleaseInfo>>,
    startup_check: bool,
    deferred_version: Option<String>,
    deferred_url: Option<String>,
    requested: bool,
    check_button_enabled: bool,
}

pub struct XprApp {
    cfg: Config,
    paths: Paths,
    registry: Arc<Registry>,
    theme: Theme,
    shortcuts: ShortcutMap,
    ctrl: MainController,
    assets: Assets,
    page: Page,
    landing: LandingPage,
    new_route: NewRoutePage,
    route_list: RouteList,
    filter_bar: FilterBar,
    details: EventDetails,
    map: MapView,
    quick_add: QuickAddPopover,
    dialog: Option<Dialog>,
    message: Option<MessageBox>,
    toast: ToastHost,
    message_label: AutoClearingLabel,
    index: RouteIndex,
    /// start-up index scan: the persisted index first (shown at once), then
    /// the refreshed one with `true`
    index_rx: Option<Receiver<(RouteIndex, bool)>>,
    run_summary: Option<RunSummary>,
    setup_summary_open: bool,
    compare: CompareView,
    /// The compare page's on-screen width, for exports.
    compare_width: Option<f32>,
    /// `XPR_SMOKE_COMPARE_EXPAND`: the checkpoint row to open once loaded.
    smoke_compare_expand: Option<usize>,
    /// The page Compare was opened from, returned to by Back / Esc.
    compare_return: Page,
    recorder: Recorder,
    /// the landing page's "Start Recording": the GameHook session that
    /// detects the game and the first Pokémon before a route exists
    quick_start: Option<QuickStart>,
    route_name_text: String,
    image_path_text: String,
    route_loaded_before_new_route: bool,
    auto_load_checked: bool,
    deferred_post_init: Option<Instant>,
    /// the start-up custom-gen load, off the UI thread: the data dir may
    /// live in a cloud-synced folder whose reads can stall for many seconds
    custom_gens_rx: Option<Receiver<Result<(), String>>>,
    first_frame: bool,
    update: UpdateState,
    pre_state_fraction: Option<f64>,
    battle_fraction: Option<f64>,
    map_fraction: Option<f64>,
    map_restore_pending: bool,
    splitter_save_deadline: Option<Instant>,
    splitter_dragging: bool,
    initial_splitter_applied: bool,
    /// Exports requested this frame; drawn offscreen and saved at the start
    /// of the next one (`perform_pending_shots`).
    pending_shots: Vec<ShotKind>,
    /// Where the route list body was drawn last frame (its export size).
    event_list_rect: Option<Rect>,
    /// The setup summary window's content rect last frame (its export size).
    setup_summary_rect: Option<Rect>,
    invalid_cycle_ids: Vec<NodeId>,
    invalid_cycle_index: i64,
    exit: SharedExit,
    allow_close: bool,
    fonts_installed: bool,
    /// `XPR_SMOKE_SCREENSHOT=<png path>`: capture the window after a few
    /// seconds and exit (used for unattended visual checks).
    smoke: Option<(PathBuf, Instant, bool)>,
    smoke_action_done: bool,
    /// `XPR_FRAME_LOG=1`: log the duration of every frame slower than 1 ms.
    frame_log: bool,
    /// (route list, event details) draw times of the current frame
    frame_parts: (Duration, Duration),
    /// `XPR_SMOKE_ACTION=candy`: the candy clicks still to send, and when.
    smoke_candy: Option<(u32, Instant)>,
    /// `XPR_SMOKE_ACTION=prestate`: when to force the Pre-Event State tab
    /// back on after the selection's auto-switch.
    smoke_prestate: Option<Instant>,
    /// `XPR_SMOKE_ACTION=record`: (save-as name, when to stop recording at the latest)
    smoke_record: Option<(Option<String>, Instant)>,
    /// set by the `XPR_SMOKE_STOP_URL` poller once the mock reports the scenario is done
    smoke_stop_flag: Option<Arc<std::sync::atomic::AtomicBool>>,
}

impl XprApp {
    pub fn new(cc: &eframe::CreationContext<'_>, cfg: Config, paths: Paths, registry: Arc<Registry>, exit: SharedExit, background: StartupWork) -> XprApp {
        let mut theme = Theme::from_config(&cfg);
        theme.install_fonts(&cc.egui_ctx);
        theme.apply(&cc.egui_ctx);
        let ctrl = MainController::new(registry.clone(), paths.clone());
        let assets = Assets::new();
        let recorder = Recorder::new(cc.egui_ctx.clone());
        // from here on the background results wake the UI themselves
        let _ = background.waker.set(cc.egui_ctx.clone());
        let StartupWork { index_rx: rx, custom_gens_rx: ctx_rx, update_rx: urx, .. } = background;
        let shortcuts = ShortcutMap::from_config(&cfg);
        let landing = LandingPage::new(&cfg);
        let route_list = RouteList::new(&cfg);
        let details = EventDetails::new(&cfg);
        let pre_state_fraction = cfg.get_pre_state_left_fraction();
        let battle_fraction = cfg.get_battle_summary_left_fraction();
        let map_fraction = cfg.get_map_left_fraction();
        let map = MapView::new(&cfg, &paths.pokemon_raw_data);
        XprApp {
            cfg,
            paths,
            registry,
            theme,
            shortcuts,
            ctrl,
            assets,
            page: Page::Landing,
            landing,
            new_route: NewRoutePage::new(),
            route_list,
            filter_bar: FilterBar::new(),
            details,
            map,
            quick_add: QuickAddPopover::new(),
            dialog: None,
            message: None,
            toast: ToastHost::default(),
            message_label: AutoClearingLabel::new(3000),
            index: RouteIndex::default(),
            index_rx: Some(rx),
            run_summary: None,
            setup_summary_open: false,
            compare: CompareView::new(),
            compare_width: None,
            smoke_compare_expand: None,
            compare_return: Page::Landing,
            recorder,
            quick_start: None,
            route_name_text: String::new(),
            image_path_text: String::new(),
            route_loaded_before_new_route: false,
            auto_load_checked: false,
            deferred_post_init: None,
            custom_gens_rx: Some(ctx_rx),
            first_frame: true,
            update: UpdateState { rx: Some(urx), startup_check: true, deferred_version: None, deferred_url: None, requested: false, check_button_enabled: true },
            pre_state_fraction,
            battle_fraction,
            map_fraction,
            map_restore_pending: true,
            splitter_save_deadline: None,
            splitter_dragging: false,
            initial_splitter_applied: false,
            pending_shots: Vec::new(),
            event_list_rect: None,
            setup_summary_rect: None,
            invalid_cycle_ids: Vec::new(),
            invalid_cycle_index: -1,
            exit,
            allow_close: false,
            fonts_installed: true,
            smoke: std::env::var_os("XPR_SMOKE_SCREENSHOT").map(|p| {
                // `XPR_SMOKE_DELAY_MS`: how long to wait before the capture (default 4 s)
                let delay = std::env::var("XPR_SMOKE_DELAY_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(4000);
                (PathBuf::from(p), Instant::now() + Duration::from_millis(delay), false)
            }),
            smoke_action_done: false,
            frame_log: std::env::var_os("XPR_FRAME_LOG").is_some(),
            frame_parts: (Duration::ZERO, Duration::ZERO),
            smoke_candy: None,
            smoke_prestate: None,
            smoke_record: None,
            smoke_stop_flag: None,
        }
    }

    // ---- helpers ----------------------------------------------------------------------------

    fn text_field_focused(&self, ctx: &egui::Context) -> bool {
        ctx.memory(|m| m.focused().is_some())
    }

    fn show_message(&mut self, title: &str, text: &str, buttons: MsgButtons, tag: MsgTag) {
        self.message = Some(MessageBox::new(title, text, buttons, tag));
    }

    fn show_landing_page(&mut self) {
        self.page = Page::Landing;
        self.refresh_index_now();
    }

    fn refresh_index_now(&mut self) {
        if self.index.loaded {
            self.index.refresh(&self.paths);
        }
    }

    fn show_new_route_page(&mut self) {
        self.page = Page::NewRoute;
        // pick up routes saved since the last scan (mtime check only)
        self.refresh_index_now();
        self.new_route.refresh_game_list(&self.registry, &self.index);
    }

    fn show_route_controls(&mut self) {
        self.page = Page::Editor;
        if !self.initial_splitter_applied {
            self.initial_splitter_applied = true;
        }
    }

    fn version_color(version: &str) -> Color32 {
        consts::version_color(version).map(theme::parse_hex).unwrap_or(Color32::WHITE)
    }

    // ---- startup --------------------------------------------------------------------------------

    /// `_deferred_post_init`: the auto-load, once the background custom-gen
    /// load has finished (a route may be for a custom gen).
    fn deferred_post_init(&mut self) {
        // `XPR_SMOKE_ROUTE=<route name>` (smoke test only): load that route
        // instead of honouring the auto-load preference.
        let smoke_route = if self.smoke.is_some() { std::env::var("XPR_SMOKE_ROUTE").ok().map(|n| io_utils::get_existing_route_path(&self.paths, &n)) } else { None };
        // `XPR_SMOKE_NEW_ROUTE=<version>|<solo mon>` (smoke test only): start a
        // fresh route from the built-in data, e.g. for the standalone build check.
        if self.smoke.is_some() {
            if let Some((version, mon)) = std::env::var("XPR_SMOKE_NEW_ROUTE").ok().and_then(|v| v.split_once('|').map(|(a, b)| (a.to_string(), b.to_string()))) {
                log::info!("smoke: creating a new {} route with {}", version, mon);
                self.ctrl.create_new_route(&mon, None, &version, None, None, None);
                self.show_route_controls();
                return;
            }
        }
        if smoke_route.is_some() || (self.cfg.get_auto_load_most_recent_route() && !self.auto_load_checked) {
            self.auto_load_checked = true;
            if let Some(p) = smoke_route.or_else(|| self.find_most_recent_route()) {
                log::info!("Auto-loading most recent route: {}", p.display());
                self.ctrl.load_route(&p);
                self.show_route_controls();
                return;
            }
        }
        // a scan still running in the background delivers its own result
        if self.index_rx.is_none() {
            self.refresh_index_now();
        }
    }

    fn find_most_recent_route(&self) -> Option<PathBuf> {
        if self.index.loaded && self.index_rx.is_none() {
            return self.index.most_recent(&self.paths);
        }
        // index not refreshed yet (the persisted copy may be stale): scan mtimes directly
        let mut best: Option<(f64, PathBuf)> = None;
        for name in io_utils::get_existing_route_names(&self.paths, "", false) {
            let p = io_utils::get_existing_route_path(&self.paths, &name);
            let m = std::fs::metadata(&p).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs_f64()).unwrap_or(0.0);
            if best.as_ref().map(|(bm, _)| m > *bm).unwrap_or(true) {
                best = Some((m, p));
            }
        }
        best.map(|(_, p)| p)
    }

    // ---- controller signal dispatch ------------------------------------------------------------

    fn dispatch_signals(&mut self, ctx: &egui::Context) {
        let sig = self.ctrl.take_signals();
        if !sig.any() {
            return;
        }
        let mut actions = DetailsActions::default();
        if sig.name_changed {
            let name = self.ctrl.get_current_route_name().to_string();
            if self.route_name_text != name {
                self.route_name_text = name;
            }
        }
        if sig.version_changed {
            self.details.handle_version_change(&self.cfg, &mut self.ctrl);
            self.invalid_cycle_ids.clear();
            self.invalid_cycle_index = -1;
        }
        if sig.route_changed {
            self.route_list.mark_dirty();
            self.quick_add.on_route_change();
            if self.ctrl.get_version().is_some() {
                self.show_route_controls();
            } else {
                self.show_landing_page();
            }
            self.details.handle_route_change(&self.cfg, &mut self.ctrl);
            self.map.sync_route(&self.ctrl);
            if let Some(rs) = self.run_summary.as_mut() {
                rs.refresh(&self.ctrl);
            }
            self.invalid_cycle_ids.clear();
            self.invalid_cycle_index = -1;
        }
        if sig.filter_changed {
            self.route_list.mark_dirty();
            self.route_list.scroll_to_selected_events(&mut self.ctrl);
        }
        if sig.selection_changed || sig.record_mode_changed {
            // sync the list selection when the controller changed it programmatically
            let ids = self.ctrl.get_all_selected_ids(true);
            if ids != self.route_list.get_all_selected_event_ids(&self.ctrl, true) {
                self.route_list.set_all_selected_event_ids(&ids);
                self.route_list.scroll_to_selected_events(&mut self.ctrl);
            }
            self.details.handle_selection(&self.cfg, &mut self.ctrl, &mut actions);
            self.map.sync_route(&self.ctrl);
        }
        if sig.record_mode_changed {
            self.recorder.on_recording_mode_changed(&self.ctrl, &self.cfg);
        }
        if sig.preview_changed {
            // The Qt port left trainer preview as a placeholder (no-op).
        }
        if sig.message {
            while let Some(msg) = self.ctrl.get_next_message_info() {
                self.on_route_message(&msg);
            }
        }
        if sig.exception {
            while let Some(msg) = self.ctrl.get_next_exception_info() {
                self.message_label.set_message(format!("Error: {}", msg));
            }
        }
        self.apply_details_actions(ctx, actions);
    }

    /// `_on_route_message`
    fn on_route_message(&mut self, message: &str) {
        if let Some(route_name) = message.strip_prefix("Successfully saved route: ") {
            let full = self.paths.saved_routes_dir.join(format!("{}.json", route_name));
            let folder = full.parent().filter(|p| p.is_dir()).map(|p| p.to_path_buf());
            self.toast.show(format!("Route saved: {}", route_name), 5000, folder);
            self.refresh_index_now();
        } else if let Some(path) = message.strip_prefix("Saved screenshot to: ") {
            let folder = PathBuf::from(path).parent().filter(|p| p.is_dir()).map(|p| p.to_path_buf());
            self.toast.show("Screenshot saved", 5000, folder);
        } else {
            self.message_label.set_message(message.to_string());
        }
    }

    fn apply_details_actions(&mut self, ctx: &egui::Context, actions: DetailsActions) {
        let _ = ctx;
        if actions.show_on_map {
            self.show_selected_on_map();
        }
        if let Some(_is_battle) = actions.tab_changed {
            // the splitter re-proportions from the saved fractions (drawn from state)
        }
        if actions.battle.open_config {
            self.dialog = Some(Dialog::BattleConfig(BattleConfigDialog::new(&self.cfg)));
        }
        if let Some(slot) = actions.battle.assign_move_slot {
            if let Some(gen) = self.ctrl.gen() {
                self.dialog = Some(Dialog::AssignMove(AssignMoveDialog::new(&gen, slot, false)));
            }
        }
        if let Some(slot) = actions.pre_state_assign_move_slot {
            let has_event = matches!(
                self.ctrl.get_single_selected_event_id(true).map(|id| self.ctrl.router.obj_kind(id)),
                Some(Some(ObjKind::Group)) | Some(Some(ObjKind::Item))
            );
            if has_event {
                if let Some(gen) = self.ctrl.gen() {
                    self.dialog = Some(Dialog::AssignMove(AssignMoveDialog::new(&gen, slot, true)));
                }
            }
        }
        if actions.pre_state_override_evs {
            if let Some(id) = self.ctrl.get_single_selected_event_id(true) {
                let is_event = matches!(self.ctrl.router.obj_kind(id), Some(ObjKind::Group) | Some(ObjKind::Item));
                if let (true, Some(gen), Some(st)) = (is_event, self.ctrl.gen(), self.ctrl.router.init_state_of(id)) {
                    let sx = st.solo_pkmn.unrealized_stat_xp;
                    let current = [sx.hp, sx.attack, sx.defense, sx.special_attack, sx.special_defense, sx.speed];
                    self.dialog = Some(Dialog::EvOverride(EvOverrideDialog::new(&gen, current)));
                }
            }
        }
        if let Some(idx) = actions.battle.export_matchup {
            self.dialog = Some(Dialog::MatchupExport(MatchupExportDialog { mon_idx: idx }));
        }
        if let Some((from, to)) = actions.battle.reorder {
            self.details.handle_matchup_reorder(&self.cfg, &mut self.ctrl, from, to);
        }
    }

    // ---- shortcuts --------------------------------------------------------------------------

    /// Draw the compare page and report what it wants done.
    fn compare_page(&mut self, ui: &mut egui::Ui) -> crate::compare::CompareActions {
        self.compare.poll(ui.ctx());
        if let Some(n) = self.smoke_compare_expand {
            if self.compare.has_comparison() {
                self.compare.expand_checkpoint_for_smoke(n);
                self.smoke_compare_expand = None;
            }
        }
        let theme = self.theme.clone();
        let env = CompareEnv {
            registry: self.registry.clone(),
            paths: &self.paths,
            index: &self.index,
            current_route_name: self.current_route_name(),
        };
        let mut actions = self.compare.ui(ui, &theme, &self.cfg, &mut self.assets, &env);
        if let Some(is_a) = actions.use_current_route {
            // serialized once, on request (a big route takes milliseconds)
            if let Some(src) = self.current_route_source() {
                self.compare.set_source(is_a, src, &env);
            }
        }
        // An open picker consumes Esc itself, so this only sees the key when
        // it means "leave the page".
        if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
            actions.back = true;
        }
        actions
    }

    /// Open the compare page, seeding route A with the open route (D9).
    fn open_compare_page(&mut self) {
        let current = self.current_route_source();
        self.open_compare_with(current);
    }

    /// Open the compare page with `a` (if any) in slot A.
    fn open_compare_with(&mut self, a: Option<RouteSource>) {
        if self.page != Page::Compare {
            self.compare_return = self.page;
        }
        let env = CompareEnv {
            registry: self.registry.clone(),
            paths: &self.paths,
            index: &self.index,
            current_route_name: self.current_route_name(),
        };
        self.compare.open(a, &env);
        self.page = Page::Compare;
    }

    /// The name of the route open in the editor, if one is.
    fn current_route_name(&self) -> Option<String> {
        let name = self.ctrl.get_current_route_name();
        (!name.is_empty() && self.ctrl.gen().is_some()).then(|| name.to_string())
    }

    /// The route open in the editor, serialized from memory.
    fn current_route_source(&self) -> Option<RouteSource> {
        let name = self.current_route_name()?;
        self.ctrl.router.serialize().ok().map(|json| RouteSource::Value { label: name, json })
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if self.dialog.is_some() || self.message.is_some() {
            return;
        }
        let text_focus = self.text_field_focused(ctx);
        let sc = self.shortcuts.clone();
        let fire = |id: &str| -> bool {
            if let Some(s) = sc.get(id) {
                if text_focus && xpr_ui_kit::shortcuts::text_field_swallows(s) {
                    return false;
                }
                return sc.consume(ctx, id);
            }
            false
        };
        // File
        if fire("customize_dvs") {
            self.open_customize_dvs_window();
        }
        if fire("new_route") {
            self.open_new_route_window();
        }
        if fire("load_route") {
            self.dialog = Some(Dialog::LoadRoute(LoadRouteDialog::new(&self.paths)));
        }
        if fire("compare_routes") {
            self.open_compare_page();
        }
        if fire("save_route") {
            self.save_route();
        }
        if fire("close_route") {
            self.close_route();
        }
        if fire("auto_load_recent") {
            self.toggle_auto_load_most_recent_route();
        }
        if fire("export_notes") {
            self.export_notes();
        }
        if fire("screenshot_events") {
            self.request_shot(ShotKind::EventList);
        }
        if fire("screenshot_battle") {
            self.request_shot(ShotKind::BattleSummary);
        }
        if fire("screenshot_player") {
            self.request_shot(ShotKind::PlayerRanges);
        }
        if fire("screenshot_enemy") {
            self.request_shot(ShotKind::EnemyRanges);
        }
        if fire("open_image_folder") {
            io_utils::open_explorer(&self.cfg.get_images_dir());
        }
        if fire("config_font") {
            self.dialog = Some(Dialog::ColorConfig(ColorConfigDialog::new(&self.cfg)));
        }
        if fire("custom_gens") {
            self.dialog = Some(Dialog::CustomGen(CustomGenDialog::new(&self.registry)));
        }
        if fire("app_config") {
            self.dialog = Some(Dialog::AppConfig(AppConfigDialog::new(&self.cfg, false)));
        }
        if fire("open_data_folder") {
            io_utils::open_explorer(&self.cfg.get_user_data_dir());
        }
        if fire("keyboard_shortcuts") {
            self.dialog = Some(Dialog::Shortcuts(ShortcutsDialog::new(&self.cfg)));
        }
        // Events
        // Everything below edits or drives the route editor. The compare page
        // keeps the editor's route loaded underneath it, so none of it may
        // fire there: Ctrl+Z or Delete would change a route nobody can see.
        if self.page == Page::Compare {
            return;
        }
        if fire("undo") && !text_focus && self.ctrl.can_undo() {
            self.ctrl.undo();
        }
        if fire("move_event_up") {
            self.move_group_up();
        }
        if fire("move_event_down") {
            self.move_group_down();
        }
        if fire("move_event_up_folder") {
            let ids = self.route_list.get_all_selected_event_ids(&self.ctrl, false);
            self.ctrl.move_groups_to_adjacent_folder_up(&ids);
        }
        if fire("move_event_down_folder") {
            let mut ids = self.route_list.get_all_selected_event_ids(&self.ctrl, false);
            ids.reverse();
            self.ctrl.move_groups_to_adjacent_folder_down(&ids);
        }
        if fire("enable_disable") {
            self.route_list.trigger_checkbox(&mut self.ctrl);
        }
        if fire("toggle_highlight") {
            let ids = self.route_list.get_all_selected_event_ids(&self.ctrl, false);
            self.ctrl.toggle_event_highlight(&ids);
        }
        if fire("delete_event") || fire("delete_key") {
            self.delete_group();
        }
        for i in 1..=9 {
            if fire(&format!("highlight_{}", i)) {
                self.set_event_highlight(i);
            }
        }
        if fire("new_folder") {
            self.open_new_folder_window(None);
        }
        if fire("rename_folder") {
            self.rename_selected_folder();
        }
        if fire("split_folder") {
            if let Some(id) = self.ctrl.get_single_selected_event_id(false) {
                self.ctrl.split_folder_at_current_event(id);
            }
        }
        if fire("toggle_recording") {
            self.record_button_clicked();
        }
        if fire("final_trainers") {
            self.open_final_trainers_dialog();
        }
        if fire("toggle_move_highlights") {
            self.toggle_move_highlights();
        }
        if fire("toggle_fade_no_highlight") {
            self.toggle_fade_moves_without_highlight();
        }
        if fire("toggle_test_moves") {
            self.toggle_test_moves();
        }
        if fire("candy_decrement") {
            self.details.decrement_prefight_candies(&self.cfg, &mut self.ctrl);
        }
        if fire("candy_increment") {
            self.details.increment_prefight_candies(&self.cfg, &mut self.ctrl);
        }
        if fire("toggle_player_strat") {
            self.toggle_player_highlight_strategy();
        }
        if fire("toggle_enemy_strat") {
            self.toggle_enemy_highlight_strategy();
        }
        if fire("scroll_home") {
            self.route_list.scroll_to_top();
        }
        if fire("scroll_end") {
            self.route_list.scroll_to_bottom();
        }
        if fire("toggle_tabs") {
            let mut actions = DetailsActions::default();
            self.details.change_tabs(&self.cfg, &mut actions);
            self.apply_details_actions(ctx, actions);
        }
        if fire("toggle_summary") {
            self.open_summary_window();
        }
        if fire("toggle_map") {
            self.toggle_map();
        }
        if fire("show_on_map") {
            self.show_selected_on_map();
        }
        for i in 1..=8 {
            if fire(&format!("gym_{}", i)) && !text_focus {
                self.select_gym_leader(i - 1);
            }
        }
        if fire("gym_blue") && !text_focus {
            self.select_blue();
        }
        for i in 1..=7 {
            if fire(&format!("e4_{}", i)) {
                self.select_elite_four_or_champion(i - 1);
            }
        }
        // filters (application-wide; the handlers check the text-field flag)
        let filters: [(&str, &str); 18] = [
            ("filter_trainer", consts::TASK_TRAINER_BATTLE),
            ("filter_rare_candy", consts::TASK_RARE_CANDY),
            ("filter_tm_hm", consts::TASK_LEARN_MOVE_TM),
            ("filter_vitamin", consts::TASK_VITAMIN),
            ("filter_wild_pkmn", consts::TASK_FIGHT_WILD_PKMN),
            ("filter_acquire_item", consts::TASK_GET_FREE_ITEM),
            ("filter_purchase_item", consts::TASK_PURCHASE_ITEM),
            ("filter_use_item", consts::TASK_USE_ITEM),
            ("filter_reorder_bag", consts::TASK_REORDER_BAG),
            ("filter_sell_item", consts::TASK_SELL_ITEM),
            ("filter_hold_item", consts::TASK_HOLD_ITEM),
            ("filter_levelup_move", consts::TASK_LEARN_MOVE_LEVELUP),
            ("filter_save", consts::TASK_SAVE),
            ("filter_heal", consts::TASK_HEAL),
            ("filter_blackout", consts::TASK_BLACKOUT),
            ("filter_evolution", consts::TASK_EVOLUTION),
            ("filter_ev_override", consts::TASK_EV_OVERRIDE),
            ("filter_notes", consts::TASK_NOTES_ONLY),
        ];
        for (aid, et) in filters {
            if fire(aid) && !text_focus {
                // the reorder filter only exists on gen 1 routes (turning an active one off is always allowed)
                if et == consts::TASK_REORDER_BAG {
                    let active = self.ctrl.get_route_filter_types().map(|f| f.iter().any(|t| t == et)).unwrap_or(false);
                    if !active && !self.ctrl.gen().map(|g| g.supports_bag_reorder()).unwrap_or(false) {
                        continue;
                    }
                }
                FilterBar::toggle_filter_type(&mut self.ctrl, et);
            }
        }
        if fire("filter_common") && !text_focus {
            let mut current: Vec<String> = self.ctrl.get_route_filter_types().map(|f| f.to_vec()).unwrap_or_default();
            let common = [consts::TASK_TRAINER_BATTLE, consts::TASK_RARE_CANDY, consts::TASK_VITAMIN];
            let all_on = common.iter().all(|c| current.iter().any(|x| x == c));
            if all_on {
                current.retain(|x| !common.contains(&x.as_str()));
            } else {
                for c in common {
                    if !current.iter().any(|x| x == c) {
                        current.push(c.to_string());
                    }
                }
            }
            self.ctrl.set_route_filter_types(current);
        }
        if fire("filter_reset") && !text_focus {
            self.ctrl.set_route_filter_types(Vec::new());
        }
    }

    // ---- menu / action handlers -------------------------------------------------------------

    fn save_route(&mut self) {
        let name = self.route_name_text.clone();
        self.details.force_and_clear_event_update(&self.cfg, &mut self.ctrl);
        self.ctrl.save_route(&name);
    }

    fn export_notes(&mut self) {
        let name = self.route_name_text.clone();
        self.ctrl.export_notes(&name);
    }

    fn open_customize_dvs_window(&mut self) {
        if self.ctrl.get_init_state().is_none() {
            return;
        }
        if let Some(gen) = self.ctrl.gen() {
            self.dialog = Some(Dialog::CustomDvs(CustomDvsDialog::new(&gen, &self.ctrl)));
        }
    }

    fn open_new_route_window(&mut self) {
        self.route_loaded_before_new_route = self.ctrl.get_version().is_some();
        self.show_new_route_page();
    }

    fn new_route_from_current(&mut self) {
        if self.ctrl.get_version().is_none() || self.ctrl.get_init_state().is_none() {
            return;
        }
        if self.ctrl.has_unsaved_changes() {
            self.show_message("Unsaved Changes", "Route has unsaved changes. Save before starting a new route?", MsgButtons::YesNoCancel, MsgTag::NewRouteFromCurrentUnsaved);
            return;
        }
        self.ctrl.create_new_route_from_current();
    }

    fn close_route(&mut self) {
        if self.ctrl.get_version().is_none() {
            return;
        }
        if self.ctrl.has_unsaved_changes() {
            self.show_message("Unsaved Changes", "Route has unsaved changes. Save before closing?", MsgButtons::YesNoCancel, MsgTag::CloseRouteUnsaved);
            return;
        }
        self.ctrl.close_route();
    }

    fn toggle_auto_load_most_recent_route(&mut self) {
        let new_val = !self.cfg.get_auto_load_most_recent_route();
        self.cfg.set_auto_load_most_recent_route(new_val);
        self.landing.auto_load = new_val;
    }

    fn move_group_up(&mut self) {
        let ids = self.route_list.get_all_selected_event_ids(&self.ctrl, false);
        self.ctrl.move_groups_up(&ids);
    }

    fn move_group_down(&mut self) {
        let mut ids = self.route_list.get_all_selected_event_ids(&self.ctrl, false);
        ids.reverse();
        self.ctrl.move_groups_down(&ids);
    }

    fn set_event_highlight(&mut self, highlight_num: i64) {
        let selected = self.route_list.get_all_selected_event_ids(&self.ctrl, false);
        if selected.is_empty() {
            return;
        }
        let all_have = selected.iter().all(|id| match self.ctrl.router.obj_kind(*id) {
            Some(ObjKind::Group) => self.ctrl.router.group(*id).map(|g| g.event_definition.get_highlight_type() == Some(highlight_num)).unwrap_or(false),
            Some(ObjKind::Folder) => self.ctrl.router.folder(*id).map(|f| f.event_definition.get_highlight_type() == Some(highlight_num)).unwrap_or(false),
            _ => false,
        });
        let num = if all_have { None } else { Some(highlight_num) };
        self.ctrl.set_event_highlight(&selected, num);
    }

    fn delete_group(&mut self) {
        let ids = self.route_list.get_all_selected_event_ids(&self.ctrl, false);
        if ids.is_empty() {
            return;
        }
        let do_prompt = if ids.len() == 1 {
            self.ctrl.router.folder(ids[0]).map(|f| !f.children.is_empty()).unwrap_or(false)
        } else {
            true
        };
        if do_prompt {
            self.show_message("Confirm Delete", &format!("Delete {} event(s)?", ids.len()), MsgButtons::YesNo, MsgTag::DeleteEvents(ids));
        } else {
            self.ctrl.delete_events(&ids[..1]);
        }
    }

    fn open_transfer_event_window(&mut self) {
        let ids = self.route_list.get_all_selected_event_ids(&self.ctrl, false);
        if ids.is_empty() {
            return;
        }
        let all: Vec<String> = self.ctrl.get_all_folder_names();
        let mut invalid: std::collections::HashSet<String> = std::collections::HashSet::new();
        for id in &ids {
            for f in self.ctrl.get_invalid_folders(*id) {
                invalid.insert(f);
            }
        }
        let valid: Vec<String> = all.iter().filter(|f| !invalid.contains(*f)).cloned().collect();
        self.dialog = Some(Dialog::Transfer(TransferDialog::new(all, valid, ids)));
    }

    fn open_new_folder_window(&mut self, existing: Option<String>) {
        let ids = self.route_list.get_all_selected_event_ids(&self.ctrl, true);
        if ids.len() > 1 {
            return;
        }
        let names = self.ctrl.get_all_folder_names();
        self.dialog = Some(Dialog::NewFolder(NewFolderDialog::new(names, existing, ids.first().copied())));
    }

    fn rename_selected_folder(&mut self) {
        let ids = self.route_list.get_all_selected_event_ids(&self.ctrl, true);
        if ids.len() != 1 {
            return;
        }
        let id = ids[0];
        let folder_id = match self.ctrl.router.obj_kind(id) {
            Some(ObjKind::Folder) => Some(id),
            Some(ObjKind::Group) => self.ctrl.router.parent_of(id),
            Some(ObjKind::Item) => self.ctrl.router.item(id).map(|i| i.parent).and_then(|g| self.ctrl.router.parent_of(g)),
            None => None,
        };
        let Some(fid) = folder_id else { return };
        let Some(f) = self.ctrl.router.folder(fid) else { return };
        if f.name == consts::ROOT_FOLDER_NAME {
            return;
        }
        let name = f.name.clone();
        self.open_new_folder_window(Some(name));
    }

    fn open_summary_window(&mut self) {
        if let Some(rs) = &self.run_summary {
            if rs.docked {
                self.run_summary = None;
                return;
            }
            // undocked: bring to front (the viewport is drawn each frame)
            return;
        }
        let docked = self.cfg.get_run_summary_docked();
        let mut rs = RunSummary::new(docked);
        rs.refresh(&self.ctrl);
        self.run_summary = Some(rs);
    }

    fn toggle_summary_dock(&mut self) {
        if let Some(rs) = self.run_summary.as_mut() {
            let was_docked = rs.docked;
            self.cfg.set_run_summary_docked(!was_docked);
            rs.docked = !was_docked;
        }
    }

    // ---- world map (docs/rust_port/design/world_map/SPEC.md) --------------------------

    fn map_is_shown(&self) -> bool {
        if self.map.docked {
            self.details.is_map_tab()
        } else {
            self.map.open
        }
    }

    /// Show the map: the Map tab of the right pane, or its own window.
    fn open_map(&mut self) {
        if self.map.docked && self.page == Page::Editor {
            let mut da = DetailsActions::default();
            self.details.set_tab(&self.cfg, MAP_TAB, &mut da);
        }
        self.map.open = true;
        self.cfg.set_map_open(true);
    }

    fn close_map(&mut self) {
        if self.details.is_map_tab() {
            let mut da = DetailsActions::default();
            self.details.set_tab(&self.cfg, PRE_STATE_TAB, &mut da);
        }
        self.map.open = false;
        self.cfg.set_map_open(false);
    }

    fn toggle_map(&mut self) {
        if self.map_is_shown() {
            self.close_map();
        } else {
            self.open_map();
        }
    }

    fn undock_map(&mut self) {
        self.map.docked = false;
        self.cfg.set_map_docked(false);
        if self.details.is_map_tab() {
            let mut da = DetailsActions::default();
            self.details.set_tab(&self.cfg, PRE_STATE_TAB, &mut da);
        }
        self.map.open = true;
        self.cfg.set_map_open(true);
    }

    fn dock_map(&mut self) {
        self.map.docked = true;
        self.cfg.set_map_docked(true);
        self.open_map();
    }

    /// "Show on map": focus the selected event's trainer / item pickup / species.
    fn show_selected_on_map(&mut self) {
        let Some(id) = self.ctrl.get_single_selected_event_id(true) else {
            self.toast.show("Select an event first", 2500, None);
            return;
        };
        let Some(q) = crate::map::state::query_for_event(&self.ctrl, id) else {
            self.toast.show("This event has no map location", 2500, None);
            return;
        };
        if MapView::game_of(&self.ctrl).is_none() {
            self.toast.show("No map data for this game (gens 1\u{2013}3 only)", 3000, None);
            return;
        }
        self.open_map();
        let label = match &q {
            LinkQuery::Trainer(n) | LinkQuery::Item(n) | LinkQuery::Species(n) => n.clone(),
        };
        if self.map.request_focus(q, label) == Some(false) {
            self.toast.show("No map location known for this event", 3000, None);
        }
    }

    fn apply_map_actions(&mut self, _ctx: &egui::Context, actions: Vec<MapAction>) {
        for a in actions {
            match a {
                MapAction::AddTrainer { name } => {
                    if self.ctrl.add_trainer_fight_from_map(&name).is_none() {
                        self.toast.show(format!("Could not add {}", name), 2500, None);
                    }
                }
                MapAction::AddItem { name } => {
                    if self.ctrl.add_item_pickup_from_map(&name).is_none() {
                        self.toast.show(format!("Could not add {}", name), 2500, None);
                    }
                }
                MapAction::AddWild { species, level } => {
                    if self.ctrl.add_wild_from_map(&species, level).is_none() {
                        self.toast.show(format!("Could not add {}", species), 2500, None);
                    }
                }
                MapAction::AddAllTrainers { map } => {
                    let names = self.map.trainers_to_add(map);
                    if names.is_empty() {
                        self.toast.show("Nothing left to add on this map", 2500, None);
                        continue;
                    }
                    let base = self.map.map_display_name(map);
                    let count = names.len();
                    match self.ctrl.add_trainers_in_new_folder(&base, &names) {
                        Some(_) => self.toast.show(format!("Added {} trainers from {}", count, base), 3000, None),
                        None => self.toast.show("Could not add the trainers", 2500, None),
                    }
                }
                MapAction::SelectEvent(id) => self.ctrl.select_new_events(vec![id]),
                MapAction::Undock => self.undock_map(),
                MapAction::Dock => self.dock_map(),
                MapAction::Close => self.close_map(),
            }
        }
    }

    fn select_gym_leader(&mut self, idx: usize) {
        let Some(gen) = self.ctrl.gen() else { return };
        let names = gen.get_gym_leader_names();
        if idx >= names.len() {
            return;
        }
        if let Some(eid) = self.ctrl.find_first_event_by_trainer_name(&names[idx]) {
            self.ctrl.select_new_events(vec![eid]);
        }
    }

    fn select_blue(&mut self) {
        let v = self.ctrl.get_version().unwrap_or("").to_string();
        if ![consts::GOLD_VERSION, consts::SILVER_VERSION, consts::CRYSTAL_VERSION, consts::HEART_GOLD_VERSION, consts::SOUL_SILVER_VERSION].contains(&v.as_str()) {
            return;
        }
        if let Some(eid) = self.ctrl.find_first_event_by_trainer_name("Leader Blue") {
            self.ctrl.select_new_events(vec![eid]);
        }
    }

    fn select_elite_four_or_champion(&mut self, idx: usize) {
        let Some(gen) = self.ctrl.gen() else { return };
        let entries = gen.get_elite_four_and_champion_names();
        if idx >= entries.len() {
            return;
        }
        for name in entries[idx].names() {
            if let Some(eid) = self.ctrl.find_first_event_by_trainer_name(&name) {
                self.ctrl.select_new_events(vec![eid]);
                return;
            }
        }
    }

    fn record_button_clicked(&mut self) {
        let new_mode = !self.ctrl.is_record_mode_active();
        self.ctrl.set_record_mode(new_mode);
    }

    // ---- quick start ("Start Recording" on the landing page) --------------------------------

    /// Connect to GameHook right away and watch for the first Pokémon; the
    /// route is created (and recording started) when it arrives.
    fn begin_quick_start(&mut self, ctx: &egui::Context) {
        if self.quick_start.is_some() {
            return;
        }
        if self.ctrl.is_record_mode_active() {
            self.ctrl.set_record_mode(false);
        }
        let wake_ctx = ctx.clone();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || wake_ctx.request_repaint());
        self.quick_start = Some(QuickStart::start(&gamehook_url(), wake));
        self.page = Page::Landing;
    }

    fn cancel_quick_start(&mut self) {
        if let Some(qs) = self.quick_start.take() {
            log::info!("[Quick Start] cancelled");
            qs.stop();
        }
    }

    /// Runs every frame: repaint while the session is busy, and act on its
    /// terminal phase.
    fn poll_quick_start(&mut self, ctx: &egui::Context) {
        let Some(qs) = &self.quick_start else { return };
        let phase = qs.phase();
        match phase {
            QuickStartPhase::Done(info) => {
                let qs = self.quick_start.take().unwrap();
                qs.stop();
                self.finish_quick_start(info);
            }
            QuickStartPhase::Failed(_) | QuickStartPhase::UnsupportedGame(_) => {}
            // the spinner, and the mapper poll the connection thread does at its own pace
            _ => ctx.request_repaint_after(Duration::from_millis(100)),
        }
    }

    /// The first Pokémon arrived: build the route from it and start the
    /// ordinary recorder.
    fn finish_quick_start(&mut self, info: StarterInfo) {
        let version = info.game.version.clone();
        let gen = match self.registry.get_version(&version) {
            Ok(g) => g,
            Err(e) => {
                self.show_message("Start Recording", &format!("The mapper says the game is {} ({}), but that version could not be loaded: {}", info.game.mapper_name, version, e), MsgButtons::Ok, MsgTag::NewRouteError);
                return;
            }
        };
        // the mapper's species names sanitize to the DB's keys ("Mr. Mime" -> "mrmime")
        let Some(species) = gen.pkmn_db().get_pkmn(&info.species).cloned() else {
            self.show_message(
                "Start Recording",
                &format!("The game reports your first Pokémon as '{}', which is not in the {} data set, so no route could be created. Use Create New Route instead.", info.species, version),
                MsgButtons::Ok,
                MsgTag::NewRouteError,
            );
            return;
        };
        let ability_idx = if info.game.generation >= 3 { starter::resolve_ability_idx(&info.ability, &species.abilities) } else { 0 };
        log::info!("[Quick Start] creating a {} route for {} (ability {}, nature {:?}, DVs {})", version, species.name, ability_idx, info.nature, info.dvs.py_repr());
        self.ctrl.create_new_route(&species.name, None, &version, Some(info.dvs), Some(ability_idx), info.nature);
        if self.ctrl.get_version().is_none() {
            self.show_message("Start Recording", "The route could not be created; see the log for details.", MsgButtons::Ok, MsgTag::NewRouteError);
            return;
        }
        self.show_route_controls();
        self.ctrl.set_record_mode(true);
        let mut msg = format!("Recording started: {}", info.summary());
        if info.game.generation >= 3 {
            if let Some(a) = species.abilities.get(ability_idx as usize) {
                msg.push_str(&format!(" {}", a));
            }
        }
        self.message_label.set_message(msg);
    }

    fn open_final_trainers_dialog(&mut self) {
        let current = self.ctrl.get_version().map(|s| s.to_string());
        self.dialog = Some(Dialog::FinalTrainers(FinalTrainersDialog::new(&self.cfg, &self.registry, current.as_deref())));
    }

    fn toggle_move_highlights(&mut self) {
        let new_val = !self.cfg.get_show_move_highlights();
        self.cfg.set_show_move_highlights(new_val);
    }

    fn toggle_fade_moves_without_highlight(&mut self) {
        if !self.cfg.get_show_move_highlights() {
            return;
        }
        let new_val = !self.cfg.get_fade_moves_without_highlight();
        self.cfg.set_fade_moves_without_highlight(new_val);
    }

    fn toggle_test_moves(&mut self) {
        let new_val = !self.cfg.get_test_moves_enabled();
        self.cfg.set_test_moves_enabled(new_val);
        self.details.refresh_after_config_change(&self.cfg, &self.ctrl);
    }

    fn toggle_player_highlight_strategy(&mut self) {
        let cur = self.cfg.get_player_highlight_strategy();
        let new = if cur == consts::HIGHLIGHT_GUARANTEED_KILL { consts::HIGHLIGHT_NONE } else { consts::HIGHLIGHT_GUARANTEED_KILL };
        self.cfg.set_player_highlight_strategy(new);
    }

    fn toggle_enemy_highlight_strategy(&mut self) {
        let cur = self.cfg.get_enemy_highlight_strategy();
        let new = if cur == consts::HIGHLIGHT_GUARANTEED_KILL { consts::HIGHLIGHT_NONE } else { consts::HIGHLIGHT_GUARANTEED_KILL };
        self.cfg.set_enemy_highlight_strategy(new);
    }

    fn cycle_invalid_events(&mut self) {
        if !self.ctrl.has_errors() {
            return;
        }
        let current = self.ctrl.get_all_invalid_event_ids();
        if current.is_empty() {
            return;
        }
        if current != self.invalid_cycle_ids {
            self.invalid_cycle_ids = current;
            self.invalid_cycle_index = -1;
        }
        self.invalid_cycle_index = (self.invalid_cycle_index + 1) % self.invalid_cycle_ids.len() as i64;
        let eid = self.invalid_cycle_ids[self.invalid_cycle_index as usize];
        self.ctrl.select_new_events(vec![eid]);
    }

    /// `_check_for_updates` (menu)
    fn check_for_updates(&mut self) {
        if self.update.rx.is_some() {
            return;
        }
        self.update.check_button_enabled = false;
        self.update.startup_check = false;
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let info = xpr_update::get_new_version_info();
            let _ = tx.send(info);
        });
        self.update.rx = Some(rx);
    }

    fn poll_update_check(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.update.rx else { return };
        let Ok(info) = rx.try_recv() else {
            ctx.request_repaint_after(Duration::from_millis(250));
            return;
        };
        self.update.rx = None;
        self.update.check_button_enabled = true;
        let startup = self.update.startup_check;
        self.update.startup_check = false;
        let version = info.tag_name.clone();
        log::info!("Latest version determined to be: {:?}", version);
        if !xpr_update::is_upgrade_needed(version.as_deref(), consts::APP_VERSION) {
            if !startup {
                if version.is_none() {
                    self.show_message("Update check failed", "Could not check for updates:\nno release information", MsgButtons::Ok, MsgTag::UpdateError);
                } else {
                    self.show_message("Up to date", &format!("You are running the latest version ({}).", consts::APP_VERSION), MsgButtons::Ok, MsgTag::UpdateNoUpdate);
                }
            }
            return;
        }
        let v = version.unwrap_or_default();
        if !xpr_update::is_upgrade_possible() {
            if !startup {
                self.show_message("Update available", &format!("Version {} is available, but automatic updates are not supported for this installation.\nPlease download the new version manually.", v), MsgButtons::Ok, MsgTag::UpdateNotPossible);
            }
            return;
        }
        self.update.deferred_version = Some(v.clone());
        self.update.deferred_url = info.asset_url.clone();
        if startup && self.cfg.get_suppress_update_prompt() {
            log::info!("Update prompt suppressed by user preference");
            return;
        }
        let text = if startup { format!("Found new version {}\nDo you want to update?", v) } else { format!("Version {} is available.\nDo you want to update now?", v) };
        self.show_message(if startup { "Update?" } else { "Update available" }, &text, MsgButtons::YesNo, MsgTag::UpdateFound(v, info.asset_url.unwrap_or_default()));
    }

    fn apply_deferred_update(&mut self) {
        let Some(v) = self.update.deferred_version.clone() else { return };
        self.show_message("Apply Update", &format!("The application will close and update to {}.\nContinue?", v), MsgButtons::YesNo, MsgTag::ApplyUpdate);
    }

    fn cancel_and_quit(&mut self, ctx: &egui::Context) {
        self.allow_close = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    // ---- screenshots -------------------------------------------------------------------------

    /// Queue an export. It is drawn offscreen at the start of the next frame
    /// (`perform_pending_shots`), so the live window never shows the export
    /// layout and whatever is open right now (the menu that was just clicked,
    /// a tooltip) is not part of the PNG.
    fn request_shot(&mut self, kind: ShotKind) {
        // F5-F8 on the compare page would export the editor as it was last
        // drawn; only the page's own export makes sense there.
        if self.page == Page::Compare && kind != ShotKind::Compare {
            return;
        }
        if self.ctrl.is_empty() && kind != ShotKind::SetupSummary && kind != ShotKind::RunSummary && kind != ShotKind::Compare {
            return;
        }
        if kind.battle_mode().is_some() && !self.details.is_battle_tab() {
            return;
        }
        if !self.pending_shots.contains(&kind) {
            self.pending_shots.push(kind);
        }
    }

    /// Before drawing: export every queued shot from the layout of the last
    /// frame (panel widths, the list's scroll position).
    fn perform_pending_shots(&mut self, ctx: &egui::Context) {
        if self.pending_shots.is_empty() {
            return;
        }
        let kinds = std::mem::take(&mut self.pending_shots);
        for kind in kinds {
            match self.export_shot(ctx, &kind) {
                Ok(path) => self.ctrl.send_message(format!("Saved screenshot to: {}", path.display())),
                Err(e) => self.ctrl.trigger_exception(format!("Couldn't save screenshot due to exception! {}", e)),
            }
        }
    }

    /// `take_screenshot`'s output path for `kind`: the custom image path
    /// (event list only) or the images dir, `<timestamp>-<route>_<kind>.png`.
    fn shot_path(&self, kind: &ShotKind) -> PathBuf {
        if matches!(kind, ShotKind::Compare) {
            if let Some(cmp) = self.compare.comparison() {
                let tab = match self.compare.tab {
                    crate::compare::TAB_CHECKPOINTS => "checkpoints",
                    crate::compare::TAB_DIFF => "event_diff",
                    _ => "overview",
                };
                let name = format!("compare_{}_vs_{}_{}", io_utils::get_path_safe_string(&cmp.a.label), io_utils::get_path_safe_string(&cmp.b.label), tab);
                let date = chrono::Local::now().format("%Y%m%d%H%M%S").to_string();
                return io_utils::get_safe_path_no_collision(&self.cfg.get_images_dir(), &format!("{}-{}", date, name), ".png");
            }
        }
        let custom = if matches!(kind, ShotKind::EventList) { Some(self.image_path_text.clone()) } else { None };
        let dir = match custom {
            Some(p) if !p.trim().is_empty() => {
                let cleaned = p.trim().trim_matches('"').trim_matches('\'').to_string();
                let pb = PathBuf::from(&cleaned);
                if pb.is_dir() {
                    pb
                } else {
                    self.cfg.get_images_dir()
                }
            }
            _ => self.cfg.get_images_dir(),
        };
        self.ctrl.screenshot_path(&dir, &kind.image_name())
    }

    /// Draw `kind` into its own transparent canvas and save it (Qt's
    /// `widget.render(pixmap)` onto a transparent pixmap). Returns the path.
    fn export_shot(&mut self, ctx: &egui::Context, kind: &ShotKind) -> Result<PathBuf, String> {
        let out_path = self.shot_path(kind);
        let ppp = ctx.pixels_per_point();
        let max_texture_side = ctx.input(|i| i.max_texture_side);
        let mut off = Offscreen::new(&self.theme, ppp, max_texture_side);
        let theme = self.theme.clone();
        let canvas = match kind {
            ShotKind::EventList => {
                // the list body at its on-screen size and scroll position
                let size = self.event_list_rect.ok_or("no event list on screen")?.size();
                let route_list = &mut self.route_list;
                let (cfg, ctrl) = (&self.cfg, &mut self.ctrl);
                let prims = off.render(size, 3, |c| {
                    egui::CentralPanel::default().frame(egui::Frame::NONE).show(c, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(2.0, 2.0);
                        route_list.export_ui(ui, &theme, cfg, ctrl);
                    });
                });
                off.rasterize(&prims, Rect::from_min_size(Pos2::ZERO, size))?
            }
            ShotKind::BattleSummary | ShotKind::PlayerRanges | ShotKind::EnemyRanges | ShotKind::Matchup { .. } => {
                // The matchup cards at the panel's on-screen width, on a canvas
                // tall enough that nothing is cut off by the scroll viewport;
                // `screenshot_mode` hides the controls bar, the per-matchup
                // Export buttons, default dropdowns and unchecked toggles, and
                // swaps the header icons.
                let base = self.details.battle_ui.geometry.base_rect.ok_or("no battle summary on screen")?;
                let mode = kind.battle_mode().unwrap_or(ScreenshotMode::Full);
                let size = Vec2::new(base.width(), 16_000.0);
                let details = &mut self.details;
                let (cfg, ctrl) = (&mut self.cfg, &mut self.ctrl);
                details.battle_ui.screenshot_mode = Some(mode);
                let mut assets = Assets::new();
                let prims = off.render(size, 2, |c| {
                    egui::CentralPanel::default().frame(egui::Frame::NONE).show(c, |ui| {
                        let mut actions = BattleUiActions::default();
                        details.battle_ui.ui(ui, &theme, cfg, &mut details.bc, ctrl, &mut assets, &mut actions);
                    });
                });
                details.battle_ui.screenshot_mode = None;
                let geo = details.battle_ui.geometry.clone();
                let (Some(base), false) = (geo.base_rect, geo.mon_pair_rects.is_empty()) else {
                    return Err("no matchups to export".to_string());
                };
                // `_get_mon_pairs_rect` / `_get_divider_x_in_base_frame`: the
                // crop is the cards' vertical extent, split at the divider for
                // the player / enemy halves.
                let top = geo.mon_pair_rects.iter().map(|r| r.min.y).fold(f32::MAX, f32::min);
                let bottom = geo.mon_pair_rects.iter().map(|r| r.max.y).fold(f32::MIN, f32::max);
                let (mut x0, mut x1) = (base.min.x, base.max.x);
                let (mut y0, mut y1) = (top, bottom);
                if let ShotKind::Matchup { idx, .. } = kind {
                    // the visible matchups are in display order
                    let visible: Vec<usize> = (0..6).filter(|i| details.bc.get_pkmn_info(*i, true).is_some() || details.bc.get_pkmn_info(*i, false).is_some()).collect();
                    let r = visible.iter().position(|i| i == idx).and_then(|p| geo.mon_pair_rects.get(p).copied()).ok_or("matchup is not shown")?;
                    (x0, x1, y0, y1) = (r.min.x, r.max.x, r.min.y, r.max.y);
                }
                match mode {
                    ScreenshotMode::Full => {}
                    ScreenshotMode::Player => x1 = geo.divider_x.map(|d| d.0).unwrap_or((x0 + x1) / 2.0),
                    ScreenshotMode::Enemy => x0 = geo.divider_x.map(|d| d.1).unwrap_or((x0 + x1) / 2.0),
                }
                let crop = Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1));
                let mut canvas = off.rasterize(&prims, crop)?;
                // `_round_container_corners` on every card the crop cuts through
                let containers: Vec<[i64; 4]> = geo.mon_pair_rects.iter().map(|m| m.intersect(crop)).filter(|m| m.is_positive()).map(|m| rect_to_pixels(m, ppp, crop)).collect();
                canvas.round_corners(&containers, CARD_RADIUS * ppp);
                canvas
            }
            ShotKind::RunSummary => {
                // the gradient grid alone (no toolbar), whatever its size
                let rs = self.run_summary.as_ref().ok_or("no run summary open")?;
                let mut grid_rect = Rect::NOTHING;
                let prims = off.render(Vec2::new(8_000.0, 8_000.0), 2, |c| {
                    egui::CentralPanel::default().frame(egui::Frame::NONE).show(c, |ui| {
                        grid_rect = rs.grid(ui, &theme);
                    });
                });
                off.rasterize(&prims, grid_rect)?
            }
            ShotKind::SetupSummary => {
                // the window's text at its on-screen size, on the window background
                let size = self.setup_summary_rect.map(|r| r.size()).unwrap_or(Vec2::new(420.0, 300.0));
                let text = setup_summary_text(&self.ctrl);
                let prims = off.render(size, 2, |c| {
                    egui::CentralPanel::default().frame(egui::Frame::new().fill(theme.bg).inner_margin(egui::Margin::same(15))).show(c, |ui| {
                        ui.add(egui::Label::new(egui::RichText::new(&text).font(theme.body()).color(theme.text)).wrap());
                    });
                });
                off.rasterize(&prims, Rect::from_min_size(Pos2::ZERO, size))?
            }
            ShotKind::Compare => {
                // The active tab's content at the page's on-screen width, on a
                // canvas tall enough that the scroll viewport cuts nothing off.
                let width = self.compare_width.unwrap_or(1280.0);
                let size = Vec2::new(width, 16_000.0);
                let compare = &mut self.compare;
                let cfg = &self.cfg;
                let mut assets = Assets::new();
                let mut content = Rect::NOTHING;
                let prims = off.render(size, 2, |c| {
                    egui::CentralPanel::default().frame(egui::Frame::new().fill(theme.bg)).show(c, |ui| {
                        content = compare.export_ui(ui, &theme, cfg, &mut assets);
                    });
                });
                off.rasterize(&prims, content)?
            }
        };
        canvas.save(&out_path)?;
        Ok(out_path)
    }

    // ---- dialogs -------------------------------------------------------------------------

    fn handle_message_choice(&mut self, ctx: &egui::Context, tag: MsgTag, choice: MsgChoice) {
        match tag {
            MsgTag::QuitUnsaved => {
                if choice == MsgChoice::Yes {
                    self.allow_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            MsgTag::NewRouteFromCurrentUnsaved => match choice {
                MsgChoice::Cancel => {}
                MsgChoice::Yes => {
                    if self.route_name_text.is_empty() {
                        self.show_message("No Route Name", "Please enter a route name before saving.", MsgButtons::Ok, MsgTag::NoRouteName);
                    } else {
                        self.save_route();
                        self.ctrl.create_new_route_from_current();
                    }
                }
                _ => self.ctrl.create_new_route_from_current(),
            },
            MsgTag::CloseRouteUnsaved => match choice {
                MsgChoice::Cancel => {}
                MsgChoice::Yes => {
                    if self.route_name_text.is_empty() {
                        self.show_message("No Route Name", "Please enter a route name before saving.", MsgButtons::Ok, MsgTag::NoRouteName);
                    } else {
                        self.save_route();
                        self.ctrl.close_route();
                    }
                }
                _ => self.ctrl.close_route(),
            },
            MsgTag::DeleteEvents(ids) => {
                if choice == MsgChoice::Yes {
                    self.ctrl.delete_events(&ids);
                }
            }
            MsgTag::UpdateFound(v, url) => {
                if choice == MsgChoice::Yes {
                    self.update.deferred_version = Some(v);
                    self.update.deferred_url = Some(url);
                    self.update.requested = true;
                    self.cancel_and_quit(ctx);
                }
            }
            MsgTag::ApplyUpdate => {
                if choice == MsgChoice::Yes {
                    self.update.requested = true;
                    self.cancel_and_quit(ctx);
                }
            }
            MsgTag::BackportResult { species, custom_gen_name } => {
                if choice == MsgChoice::Extra {
                    self.dialog = None;
                    self.ctrl.create_new_route(&species, None, &custom_gen_name, None, None, None);
                }
            }
            MsgTag::ResetAllShortcuts => {
                if choice == MsgChoice::Yes {
                    if let Some(Dialog::Shortcuts(d)) = self.dialog.as_mut() {
                        d.reset_all(&mut self.cfg);
                    } else {
                        self.cfg.reset_all_shortcuts();
                    }
                    self.shortcuts = ShortcutMap::from_config(&self.cfg);
                }
            }
            MsgTag::DuplicateShortcuts => {
                if choice == MsgChoice::Yes {
                    if let Some(Dialog::Shortcuts(d)) = self.dialog.as_mut() {
                        if d.force_apply(&mut self.cfg) {
                            self.shortcuts = ShortcutMap::from_config(&self.cfg);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_dialog_outcome(&mut self, ctx: &egui::Context, outcome: DialogOutcome) {
        match outcome {
            DialogOutcome::LoadRoute(p) => {
                self.ctrl.load_route(&p);
                self.show_route_controls();
            }
            DialogOutcome::NewFolder { name, prev, insert_after } => {
                self.ctrl.finalize_new_folder(&name, prev.as_deref(), insert_after);
            }
            DialogOutcome::Transfer { ids, folder } => self.ctrl.transfer_to_folder(&ids, &folder),
            DialogOutcome::SetDvs(dvs, ability, nature) => self.ctrl.customize_innate_stats(dvs, ability, nature),
            DialogOutcome::RefreshEventList => {
                self.route_list.update_highlight_colors(&self.cfg);
                self.route_list.mark_dirty();
            }
            DialogOutcome::RefreshTheme => {
                self.theme = Theme::from_config(&self.cfg);
                self.theme.install_fonts(ctx);
                self.theme.apply(ctx);
                self.route_list.update_highlight_colors(&self.cfg);
                self.route_list.mark_dirty();
            }
            DialogOutcome::RefreshBattle => self.details.refresh_after_config_change(&self.cfg, &self.ctrl),
            DialogOutcome::ApplyShortcuts => self.shortcuts = ShortcutMap::from_config(&self.cfg),
            DialogOutcome::AssignMove { slot, mv, pre_state } => {
                if pre_state {
                    self.ctrl.assign_move_via_tutor_before_selected(slot, &mv);
                } else {
                    self.details.bc.assign_player_move_via_tutor(&self.cfg, &mut self.ctrl, slot, &mv);
                }
            }
            DialogOutcome::EvOverride(values) => {
                self.ctrl.override_evs_before_selected(values);
            }
            DialogOutcome::MatchupExport { idx, mode } => self.request_shot(ShotKind::Matchup { idx, mode }),
            DialogOutcome::RestartForUpdate => {
                self.update.requested = true;
                self.cancel_and_quit(ctx);
            }
            DialogOutcome::CreateRouteForBackport { species, custom_gen_name } => {
                self.ctrl.create_new_route(&species, None, &custom_gen_name, None, None, None);
            }
            DialogOutcome::Message(tag, choice) => self.handle_message_choice(ctx, tag, choice),
            DialogOutcome::ReloadCustomGens => {}
            DialogOutcome::DataDirChanged => {
                self.paths.config_user_data_dir(&self.cfg.get_user_data_dir());
                self.ctrl.paths = self.paths.clone();
                self.registry.set_custom_gens_dir(self.paths.custom_gens_dir.clone());
                self.index = RouteIndex::load(&self.paths);
                self.index.refresh(&self.paths);
            }
        }
    }

    fn draw_dialogs(&mut self, ctx: &egui::Context) {
        // message box on top of everything
        if let Some(mb) = self.message.clone() {
            if let Some(choice) = mb.ui(ctx, &self.theme) {
                self.message = None;
                self.handle_message_choice(ctx, mb.tag, choice);
            }
            return;
        }
        if let Some(mut dialog) = self.dialog.take() {
            let mut message = None;
            let (close, outcome) = {
                let mut d = DialogCtx { theme: &self.theme, cfg: &mut self.cfg, ctrl: &mut self.ctrl, registry: &self.registry, paths: &self.paths };
                dialog.ui(ctx, &mut d, &mut message)
            };
            if !close {
                self.dialog = Some(dialog);
            }
            if let Some(m) = message {
                self.message = Some(m);
            }
            if let Some(o) = outcome {
                self.handle_dialog_outcome(ctx, o);
            }
        }
    }

    // ---- drawing ----------------------------------------------------------------------------

    fn menu_bar(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let theme = self.theme.clone();
        let sc = self.shortcuts.clone();
        let label = |id: &str| sc.label(id);
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.spacing_mut().button_padding = Vec2::new(8.0, 2.0);
        egui::MenuBar::new().ui(ui, |ui| {
            ui.spacing_mut().button_padding = Vec2::new(8.0, 2.0);
            ui.visuals_mut().widgets.inactive.weak_bg_fill = theme.bg_darker;
            ui.visuals_mut().widgets.hovered.weak_bg_fill = theme.hover_bg;
            ui.visuals_mut().widgets.open.weak_bg_fill = theme.hover_bg;
            ui.menu_button("File", |ui| {
                ui.set_min_width(260.0);
                if widgets::menu_item(ui, &theme, "Customize DVs", &label("customize_dvs"), true) {
                    self.open_customize_dvs_window();
                }
                if widgets::menu_item(ui, &theme, "New Route", &label("new_route"), true) {
                    self.open_new_route_window();
                }
                if widgets::menu_item(ui, &theme, "Load Route", &label("load_route"), true) {
                    self.dialog = Some(Dialog::LoadRoute(LoadRouteDialog::new(&self.paths)));
                }
                if widgets::menu_item(ui, &theme, "Compare Routes\u{2026}", &label("compare_routes"), true) {
                    self.open_compare_page();
                }
                if widgets::menu_item(ui, &theme, "Save Route", &label("save_route"), true) {
                    self.save_route();
                }
                if widgets::menu_item(ui, &theme, "Close Route", &label("close_route"), true) {
                    self.close_route();
                }
                widgets::menu_separator(ui, &theme);
                if widgets::menu_check_item(ui, &theme, "Automatically Load Most Recent Route on Startup", &label("auto_load_recent"), self.cfg.get_auto_load_most_recent_route(), true) {
                    self.toggle_auto_load_most_recent_route();
                }
                if widgets::menu_item(ui, &theme, "Export Notes", &label("export_notes"), true) {
                    self.export_notes();
                }
                widgets::menu_separator(ui, &theme);
                if widgets::menu_item(ui, &theme, "Screenshot Event List", &label("screenshot_events"), true) {
                    self.request_shot(ShotKind::EventList);
                }
                if widgets::menu_item(ui, &theme, "Screenshot Battle Summary", &label("screenshot_battle"), true) {
                    self.request_shot(ShotKind::BattleSummary);
                }
                if widgets::menu_item(ui, &theme, "Screenshot Player Ranges", &label("screenshot_player"), true) {
                    self.request_shot(ShotKind::PlayerRanges);
                }
                if widgets::menu_item(ui, &theme, "Screenshot Enemy Ranges", &label("screenshot_enemy"), true) {
                    self.request_shot(ShotKind::EnemyRanges);
                }
                if widgets::menu_item(ui, &theme, "Open Image Folder", &label("open_image_folder"), true) {
                    io_utils::open_explorer(&self.cfg.get_images_dir());
                }
                widgets::menu_separator(ui, &theme);
                if widgets::menu_item(ui, &theme, "Config Font", &label("config_font"), true) {
                    self.dialog = Some(Dialog::ColorConfig(ColorConfigDialog::new(&self.cfg)));
                }
                if widgets::menu_item(ui, &theme, "Custom Gens", &label("custom_gens"), true) {
                    self.dialog = Some(Dialog::CustomGen(CustomGenDialog::new(&self.registry)));
                }
                if widgets::menu_item(ui, &theme, "App Config", &label("app_config"), true) {
                    self.dialog = Some(Dialog::AppConfig(AppConfigDialog::new(&self.cfg, false)));
                }
                if widgets::menu_item(ui, &theme, "Open Data Folder", &label("open_data_folder"), true) {
                    io_utils::open_explorer(&self.cfg.get_user_data_dir());
                }
                widgets::menu_separator(ui, &theme);
                if widgets::menu_item(ui, &theme, "Keyboard Shortcuts...", &label("keyboard_shortcuts"), true) {
                    self.dialog = Some(Dialog::Shortcuts(ShortcutsDialog::new(&self.cfg)));
                }
            });
            ui.menu_button("Events", |ui| {
                ui.set_min_width(260.0);
                let has_route = self.ctrl.get_version().is_some() && self.ctrl.get_init_state().is_some();
                let can_split = self
                    .ctrl
                    .get_single_selected_event_id(false)
                    .and_then(|id| match self.ctrl.router.obj_kind(id) {
                        Some(ObjKind::Folder) | None => None,
                        _ => self.ctrl.router.parent_of(id).and_then(|p| self.ctrl.router.folder(p)).map(|f| f.name != consts::ROOT_FOLDER_NAME),
                    })
                    .unwrap_or(false);
                if widgets::menu_item(ui, &theme, "Add New Event", "", true) {
                    let mut la = ListActions::default();
                    self.route_list.start_add_new_event(&mut self.ctrl, &mut la);
                    self.apply_list_actions(ctx, la);
                }
                if widgets::menu_item(ui, &theme, "New Route Based on Current Route", "", has_route) {
                    self.new_route_from_current();
                }
                widgets::menu_separator(ui, &theme);
                if widgets::menu_item(ui, &theme, "Undo", &label("undo"), self.ctrl.can_undo()) {
                    self.ctrl.undo();
                }
                widgets::menu_separator(ui, &theme);
                if widgets::menu_item(ui, &theme, "Move Event Up", &label("move_event_up"), true) {
                    self.move_group_up();
                }
                if widgets::menu_item(ui, &theme, "Move Event Down", &label("move_event_down"), true) {
                    self.move_group_down();
                }
                if widgets::menu_item(ui, &theme, "Move Event Up To Next Folder", &label("move_event_up_folder"), true) {
                    let ids = self.route_list.get_all_selected_event_ids(&self.ctrl, false);
                    self.ctrl.move_groups_to_adjacent_folder_up(&ids);
                }
                if widgets::menu_item(ui, &theme, "Move Event Down To Next Folder", &label("move_event_down_folder"), true) {
                    let mut ids = self.route_list.get_all_selected_event_ids(&self.ctrl, false);
                    ids.reverse();
                    self.ctrl.move_groups_to_adjacent_folder_down(&ids);
                }
                widgets::menu_separator(ui, &theme);
                if widgets::menu_item(ui, &theme, "Enable/Disable", &label("enable_disable"), true) {
                    self.route_list.trigger_checkbox(&mut self.ctrl);
                }
                if widgets::menu_item(ui, &theme, "Toggle Highlight", &label("toggle_highlight"), true) {
                    let ids = self.route_list.get_all_selected_event_ids(&self.ctrl, false);
                    self.ctrl.toggle_event_highlight(&ids);
                }
                if widgets::menu_item(ui, &theme, "Transfer Event", "", true) {
                    self.open_transfer_event_window();
                }
                if widgets::menu_item(ui, &theme, "Delete Event", &label("delete_event"), true) {
                    self.delete_group();
                }
                widgets::menu_separator(ui, &theme);
                let has_branched = self.ctrl.gen().map(|g| g.has_branched_mandatory_fights()).unwrap_or(false);
                if widgets::menu_check_item(ui, &theme, "Highlight Branched Mandatory Battles", "", self.cfg.get_highlight_branched_mandatory(), has_branched) {
                    let v = !self.cfg.get_highlight_branched_mandatory();
                    self.cfg.set_highlight_branched_mandatory(v);
                    self.route_list.mark_dirty();
                }
                if widgets::menu_check_item(ui, &theme, "Fade Folder Text", "", self.cfg.get_fade_folder_text(), true) {
                    let v = !self.cfg.get_fade_folder_text();
                    self.cfg.set_fade_folder_text(v);
                    self.route_list.update_folder_text_style(&self.cfg);
                    self.route_list.mark_dirty();
                }
                widgets::menu_separator(ui, &theme);
                if widgets::menu_item(ui, &theme, "Setup Summary", "", true) {
                    self.setup_summary_open = true;
                }
                if widgets::menu_item(ui, &theme, "Run Summary", "", true) {
                    self.open_summary_window();
                }
                let _ = can_split;
            });
            ui.menu_button("Map", |ui| {
                ui.set_min_width(260.0);
                let has_route = self.ctrl.get_version().is_some();
                if widgets::menu_check_item(ui, &theme, "Show Map", &label("toggle_map"), self.map_is_shown(), has_route) {
                    self.toggle_map();
                }
                if widgets::menu_item(ui, &theme, "Show Selected Event on Map", &label("show_on_map"), has_route) {
                    self.show_selected_on_map();
                }
                widgets::menu_separator(ui, &theme);
                let docked = self.map.docked;
                let text = if docked { "Open Map in Its Own Window" } else { "Dock Map in the Editor" };
                if widgets::menu_item(ui, &theme, text, "", has_route) {
                    if docked {
                        self.undock_map();
                    } else {
                        self.dock_map();
                    }
                }
            });
            ui.menu_button("Highlight", |ui| {
                ui.set_min_width(200.0);
                for i in 1..=9 {
                    if widgets::menu_item(ui, &theme, &format!("Highlight {}", i), &label(&format!("highlight_{}", i)), true) {
                        self.set_event_highlight(i);
                    }
                }
                widgets::menu_separator(ui, &theme);
                if widgets::menu_item(ui, &theme, "Configure Colors", "", true) {
                    self.dialog = Some(Dialog::HighlightColors(HighlightColorDialog::new()));
                }
            });
            ui.menu_button("Folders", |ui| {
                ui.set_min_width(220.0);
                let can_split = self
                    .ctrl
                    .get_single_selected_event_id(false)
                    .and_then(|id| match self.ctrl.router.obj_kind(id) {
                        Some(ObjKind::Folder) | None => None,
                        _ => self.ctrl.router.parent_of(id).and_then(|p| self.ctrl.router.folder(p)).map(|f| f.name != consts::ROOT_FOLDER_NAME),
                    })
                    .unwrap_or(false);
                if widgets::menu_item(ui, &theme, "New Folder", &label("new_folder"), true) {
                    self.open_new_folder_window(None);
                }
                if widgets::menu_item(ui, &theme, "Rename Cur Folder", &label("rename_folder"), true) {
                    self.rename_selected_folder();
                }
                if widgets::menu_item(ui, &theme, "Split Folder", &label("split_folder"), can_split) {
                    if let Some(id) = self.ctrl.get_single_selected_event_id(false) {
                        self.ctrl.split_folder_at_current_event(id);
                    }
                }
            });
            ui.menu_button("Recording", |ui| {
                ui.set_min_width(240.0);
                if widgets::menu_item(ui, &theme, "Enable/Disable Recording", &label("toggle_recording"), true) {
                    self.record_button_clicked();
                }
                widgets::menu_separator(ui, &theme);
                if widgets::menu_check_item(ui, &theme, "Automatically Stop Recording", "", self.cfg.get_recording_auto_stop_enabled(), true) {
                    let v = !self.cfg.get_recording_auto_stop_enabled();
                    self.cfg.set_recording_auto_stop_enabled(v);
                }
                if widgets::menu_item(ui, &theme, "Final Trainers...", &label("final_trainers"), true) {
                    self.open_final_trainers_dialog();
                }
            });
            ui.menu_button("Battle Summary", |ui| {
                ui.set_min_width(280.0);
                if widgets::menu_check_item(ui, &theme, "Show Notes in Battle Summary", "", self.cfg.are_notes_visible_in_battle_summary(), true) {
                    let v = !self.cfg.are_notes_visible_in_battle_summary();
                    self.cfg.set_notes_visibility_in_battle_summary(v);
                    self.details.update_notes_visibility(&self.cfg);
                }
                if widgets::menu_check_item(ui, &theme, "Show Legacy Controls", "", self.cfg.get_show_legacy_controls(), true) {
                    let v = !self.cfg.get_show_legacy_controls();
                    self.cfg.set_show_legacy_controls(v);
                }
                ui.menu_button("Player Highlight Strategy", |ui| {
                    for strat in consts::ALL_HIGHLIGHT_STRATS {
                        if widgets::menu_check_item(ui, &theme, strat, "", self.cfg.get_player_highlight_strategy() == strat, true) {
                            self.cfg.set_player_highlight_strategy(strat);
                        }
                    }
                });
                ui.menu_button("Enemy Highlight Strategy", |ui| {
                    for strat in consts::ALL_HIGHLIGHT_STRATS {
                        if widgets::menu_check_item(ui, &theme, strat, "", self.cfg.get_enemy_highlight_strategy() == strat, true) {
                            self.cfg.set_enemy_highlight_strategy(strat);
                        }
                    }
                });
                widgets::menu_separator(ui, &theme);
                if widgets::menu_check_item(ui, &theme, "Toggle Move Highlights", &label("toggle_move_highlights"), self.cfg.get_show_move_highlights(), true) {
                    self.toggle_move_highlights();
                }
                if widgets::menu_check_item(ui, &theme, "Toggle Fade Moves Without Highlight", &label("toggle_fade_no_highlight"), self.cfg.get_fade_moves_without_highlight(), self.cfg.get_show_move_highlights()) {
                    self.toggle_fade_moves_without_highlight();
                }
                if widgets::menu_check_item(ui, &theme, "Toggle Test Moves", &label("toggle_test_moves"), self.cfg.get_test_moves_enabled(), true) {
                    self.toggle_test_moves();
                }
                widgets::menu_separator(ui, &theme);
                if widgets::menu_item(ui, &theme, "Configure Consistent Threshold...", "", true) {
                    self.dialog = Some(Dialog::ColorConfig(ColorConfigDialog::new(&self.cfg)));
                }
                if widgets::menu_item(ui, &theme, "Decrement Pre-Fight Candies", &label("candy_decrement"), true) {
                    self.details.decrement_prefight_candies(&self.cfg, &mut self.ctrl);
                }
                if widgets::menu_item(ui, &theme, "Increment Pre-Fight Candies", &label("candy_increment"), true) {
                    self.details.increment_prefight_candies(&self.cfg, &mut self.ctrl);
                }
                if widgets::menu_item(ui, &theme, "Toggle Player Highlight Strategy", &label("toggle_player_strat"), true) {
                    self.toggle_player_highlight_strategy();
                }
                if widgets::menu_item(ui, &theme, "Toggle Enemy Highlight Strategy", &label("toggle_enemy_strat"), true) {
                    self.toggle_enemy_highlight_strategy();
                }
            });
            ui.menu_button("Update", |ui| {
                ui.set_min_width(220.0);
                if widgets::menu_item(ui, &theme, "Check for Updates", "", self.update.check_button_enabled) {
                    self.check_for_updates();
                }
                if widgets::menu_check_item(ui, &theme, "Never prompt for updates", "", self.cfg.get_suppress_update_prompt(), true) {
                    let v = !self.cfg.get_suppress_update_prompt();
                    self.cfg.set_suppress_update_prompt(v);
                }
                widgets::menu_separator(ui, &theme);
                let label_text = match &self.update.deferred_version {
                    Some(v) => format!("Update to {}", v),
                    None => "Update".to_string(),
                };
                if widgets::menu_item(ui, &theme, &label_text, "", self.update.deferred_version.is_some()) {
                    self.apply_deferred_update();
                }
            });
        });
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        let theme = self.theme.clone();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            // everything in the bar shares one control height
            let h = 24.0;
            if let Some(v) = self.ctrl.get_version().map(|s| s.to_string()) {
                widgets::status_chip(ui, &theme, &format!("{} Version", v), XprApp::version_color(&v), h, false);
            }
            let (rtext, rcolor) = if self.ctrl.has_errors() { ("Run Status: Invalid", theme.failure) } else { ("Run Status: Valid", theme.success) };
            if widgets::status_chip(ui, &theme, rtext, rcolor, h, true).clicked() {
                self.cycle_invalid_events();
            }
            ui.add_space(6.0);
            widgets::label_colored(ui, &theme, "Route Name:", theme.secondary);
            let r = Entry::new(&theme, &mut self.route_name_text).width(200.0).min_height(h).corner_radius(3).id(ui.id().with("route_name")).show(ui);
            if r.changed && self.route_name_text != self.ctrl.get_current_route_name() {
                let n = self.route_name_text.clone();
                self.ctrl.set_current_route_name(&n);
            }
            widgets::label_colored(ui, &theme, "Image Path:", theme.secondary);
            let r = Entry::new(&theme, &mut self.image_path_text).width(200.0).min_height(h).corner_radius(3).id(ui.id().with("image_path")).show(ui);
            if r.changed {
                let p = self.image_path_text.clone();
                self.ctrl.set_custom_image_path(&p);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let active = self.ctrl.is_record_mode_active();
                let enabled = self.ctrl.get_version().is_some();
                let dot = if active { Color32::from_rgb(0xe7, 0x4c, 0x3c) } else { Color32::from_rgb(0x88, 0x88, 0x88) };
                let btn = widgets::StyledButton::new(&theme, "Record").dot(dot).min_size(Vec2::new(0.0, h)).padding(Vec2::new(10.0, 2.0)).enabled(enabled).show(ui);
                if btn.clicked() {
                    self.record_button_clicked();
                }
                if active {
                    if widgets::StyledButton::new(&theme, "↻").min_size(Vec2::new(h, h)).padding(Vec2::ZERO).enabled(self.recorder.reconnect_enabled).show(ui).clicked() {
                        self.recorder.reconnect();
                    }
                    widgets::label_colored(ui, &theme, self.recorder.client_status.clone(), theme.secondary);
                }
            });
        });
    }

    fn apply_list_actions(&mut self, ctx: &egui::Context, la: ListActions) {
        if let Some(s) = la.suppress_battle_summary {
            let mut da = DetailsActions::default();
            self.details.set_suppress_battle_summary(&self.cfg, s, &mut da);
            self.apply_details_actions(ctx, da);
        }
        if let Some((anchor, cat)) = la.quick_add {
            self.quick_add.show_above(&self.ctrl, anchor, cat);
        }
    }

    /// The route editor: filter bar + list on the left, details on the right.
    fn editor_page(&mut self, ui: &mut Ui, ctx: &egui::Context) -> Option<Rect> {
        let theme = self.theme.clone();
        let total = ui.available_width();
        if self.map_restore_pending {
            // reopen the map tab when it was open at the last exit
            self.map_restore_pending = false;
            if self.map.open && self.map.docked {
                let mut da = DetailsActions::default();
                self.details.set_tab(&self.cfg, MAP_TAB, &mut da);
            }
        }
        let is_battle = self.details.is_battle_tab();
        let is_map = self.details.is_map_tab();
        let mut left = if is_map {
            let f = self.map_fraction.filter(|f| *f > 0.0).unwrap_or(0.35);
            (total as f64 * f).max(200.0) as f32
        } else if is_battle {
            let f = self.battle_fraction.filter(|f| *f > 0.0).unwrap_or(0.30);
            (total as f64 * f).max(200.0) as f32
        } else {
            match self.pre_state_fraction.filter(|f| *f > 0.0) {
                Some(f) => (total as f64 * f).max(200.0) as f32,
                None => {
                    let right = (total * 0.5).clamp(total * 0.40, total * 0.65);
                    (total - right).max(200.0)
                }
            }
        };
        left = left.min((total - 200.0).max(200.0));
        let full = ui.available_rect_before_wrap();
        let handle_x = full.min.x + left;
        let handle_rect = Rect::from_min_size(Pos2::new(handle_x - 1.5, full.min.y), Vec2::new(3.0, full.height()));
        // left panel
        let left_rect = Rect::from_min_max(full.min, Pos2::new(handle_x - 1.5, full.max.y));
        let right_rect = Rect::from_min_max(Pos2::new(handle_x + 1.5, full.min.y), full.max);
        let mut event_list_rect: Option<Rect> = None;
        let mut list_actions = ListActions::default();
        let mut map_actions: Vec<MapAction> = Vec::new();
        let text_focus = self.text_field_focused(ctx);
        {
            let mut lui = ui.new_child(egui::UiBuilder::new().max_rect(left_rect).layout(egui::Layout::top_down(egui::Align::Min)));
            egui::Frame::new().inner_margin(egui::Margin::same(4)).show(&mut lui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(2.0, 2.0);
                ui.set_min_width(left_rect.width() - 8.0);
                self.message_label.ui(ui, &theme);
                self.filter_bar.ui(ui, &theme, &self.cfg, &mut self.ctrl, &mut self.assets, &self.shortcuts);
                let list_area = ui.available_rect_before_wrap();
                let inner = egui::Frame::new().inner_margin(egui::Margin::same(4)).show(ui, |ui| {
                    ui.set_min_height(list_area.height() - 8.0);
                    ui.set_min_width(list_area.width() - 8.0);
                    let t = Instant::now();
                    self.route_list.ui(ui, &theme, &self.cfg, &mut self.ctrl, &mut list_actions, !text_focus);
                    if self.frame_log {
                        self.frame_parts.0 = t.elapsed();
                    }
                });
                event_list_rect = Some(inner.response.rect.shrink(4.0));
            });
        }
        // splitter handle
        let resp = ui.interact(handle_rect, ui.id().with("splitter"), Sense::drag());
        // a hairline at rest, the full accent grip while hovered / dragged
        if resp.hovered() || resp.dragged() {
            ui.painter().rect_filled(handle_rect, CornerRadius::ZERO, theme.accent);
        } else {
            ui.painter().vline(handle_rect.center().x, handle_rect.y_range(), Stroke::new(1.0_f32, theme.border));
        }
        if resp.hovered() || resp.dragged() {
            ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        if resp.dragged() {
            let new_left = (left + resp.drag_delta().x).clamp(200.0, (total - 200.0).max(200.0));
            let fraction = (new_left / total) as f64;
            if is_map {
                self.map_fraction = Some(fraction);
            } else if is_battle {
                self.battle_fraction = Some(fraction);
            } else {
                self.pre_state_fraction = Some(fraction);
            }
            self.splitter_save_deadline = Some(Instant::now() + Duration::from_millis(500));
            self.splitter_dragging = true;
        }
        // right panel
        {
            let mut rui = ui.new_child(egui::UiBuilder::new().max_rect(right_rect).layout(egui::Layout::top_down(egui::Align::Min)));
            let mut actions = DetailsActions::default();
            let t = Instant::now();
            self.details.ui(&mut rui, &theme, &mut self.cfg, &mut self.ctrl, &mut self.assets, &mut actions);
            if let Some(rect) = actions.map_body.take() {
                let mut mui = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)));
                map_actions.extend(self.map.ui(&mut mui, &theme, &mut self.cfg, &self.ctrl, &mut self.assets));
            }
            if self.frame_log {
                self.frame_parts.1 = t.elapsed();
            }
            self.apply_details_actions(ctx, actions);
        }
        ui.allocate_rect(full, Sense::hover());
        // keyboard focus follows the last click: outside the list, the list stops taking keys
        if let (true, Some(p), Some(r)) = (ui.input(|i| i.pointer.primary_pressed()), ui.input(|i| i.pointer.interact_pos()), event_list_rect) {
            if !r.contains(p) {
                self.route_list.focused = false;
            }
        }
        self.apply_list_actions(ctx, list_actions);
        self.apply_map_actions(ctx, map_actions);
        event_list_rect
    }

    fn tick_timers(&mut self, ctx: &egui::Context) {
        if let Some(d) = self.splitter_save_deadline {
            if Instant::now() >= d {
                self.splitter_save_deadline = None;
                if self.details.is_map_tab() {
                    self.cfg.set_map_left_fraction(self.map_fraction);
                } else if self.details.is_battle_tab() {
                    self.cfg.set_battle_summary_left_fraction(self.battle_fraction);
                } else {
                    self.cfg.set_pre_state_left_fraction(self.pre_state_fraction);
                }
            } else {
                ctx.request_repaint_after(d - Instant::now());
            }
        }
        self.filter_bar.tick(ctx, &mut self.ctrl);
        self.landing.tick(ctx, &mut self.cfg);
        self.details.tick(ctx, &self.cfg, &mut self.ctrl);
        self.details.drain_battle_signals(&self.cfg, &mut self.ctrl);
    }

    fn save_geometry(&mut self, ctx: &egui::Context) {
        let (inner, outer, maximized, minimized) = ctx.input(|i| {
            let v = i.viewport();
            (v.inner_rect, v.outer_rect, v.maximized.unwrap_or(false), v.minimized.unwrap_or(false))
        });
        if let Some(inner) = inner {
            let ppp = ctx.pixels_per_point();
            let outer = outer.unwrap_or(inner);
            let g = Geometry {
                width: (inner.width() * ppp).round() as i32,
                height: (inner.height() * ppp).round() as i32,
                x: Some((outer.min.x * ppp).round() as i32),
                y: Some((outer.min.y * ppp).round() as i32),
            };
            self.cfg.set_window_geometry(&g.format());
        }
        let state = if maximized {
            "zoomed"
        } else if minimized {
            "iconic"
        } else {
            "normal"
        };
        self.cfg.set_window_state(state);
    }
}

impl XprApp {
    /// The `record` / `quickstart` smoke actions: stop recording after
    /// `XPR_SMOKE_RECORD_SECS` (default 30) or ~3 s after `XPR_SMOKE_STOP_URL`
    /// (the mock GameHook's /mock/status) reports `"done": true`, save the
    /// route as `XPR_SMOKE_SAVE_NAME` (if set), then screenshot and exit.
    fn smoke_arm_record_stop(&mut self, shot_path: &std::path::Path) {
        let secs: u64 = std::env::var("XPR_SMOKE_RECORD_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(30);
        let save_name = std::env::var("XPR_SMOKE_SAVE_NAME").ok().filter(|n| !n.is_empty());
        let stop_at = Instant::now() + Duration::from_secs(secs);
        self.smoke_record = Some((save_name, stop_at));
        self.smoke = Some((shot_path.to_path_buf(), stop_at + Duration::from_millis(1500), false));
        if let Ok(url) = std::env::var("XPR_SMOKE_STOP_URL") {
            let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
            self.smoke_stop_flag = Some(flag.clone());
            std::thread::spawn(move || {
                let mut done_since: Option<Instant> = None;
                loop {
                    std::thread::sleep(Duration::from_millis(500));
                    if smoke_http_get(&url).map(|body| body.contains("\"done\": true") || body.contains("\"done\":true")).unwrap_or(false) {
                        let since = *done_since.get_or_insert_with(Instant::now);
                        if since.elapsed() >= Duration::from_secs(3) {
                            flag.store(true, std::sync::atomic::Ordering::SeqCst);
                            return;
                        }
                    }
                }
            });
        }
    }

    /// The unattended smoke test: screenshot, then close.
    fn smoke_tick(&mut self, ctx: &egui::Context) {
        let Some((path, at, requested)) = self.smoke.clone() else { return };
        ctx.request_repaint_after(Duration::from_millis(100));
        if !requested {
            // `XPR_SMOKE_ACTION=quickstart`: press "Start Recording" on the landing
            // page (no route loaded) and then behave like `record`
            if !self.smoke_action_done && self.page == Page::Landing && self.deferred_post_init.is_none() && std::env::var("XPR_SMOKE_ACTION").as_deref() == Ok("quickstart") {
                self.smoke_action_done = true;
                log::info!("smoke: quick start");
                self.begin_quick_start(ctx);
                self.smoke_arm_record_stop(&path);
            }
            // `XPR_SMOKE_ACTION=compare`: open the compare page on
            // `XPR_SMOKE_COMPARE_A` / `_B`, wait for both to load, then show
            // `XPR_SMOKE_COMPARE_TAB` (overview | checkpoints | diff).
            if !self.smoke_action_done && std::env::var("XPR_SMOKE_ACTION").as_deref() == Ok("compare") && self.deferred_post_init.is_none() {
                self.smoke_action_done = true;
                let pick = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty()).map(|v| RouteSource::Path(PathBuf::from(v)));
                let env = CompareEnv {
                    registry: self.registry.clone(),
                    paths: &self.paths,
                    index: &self.index,
                    current_route_name: None,
                };
                if let Some(a) = pick("XPR_SMOKE_COMPARE_A") {
                    self.compare.set_source(true, a, &env);
                }
                if let Some(b) = pick("XPR_SMOKE_COMPARE_B") {
                    self.compare.set_source(false, b, &env);
                }
                self.compare.tab = match std::env::var("XPR_SMOKE_COMPARE_TAB").as_deref() {
                    Ok("checkpoints") => crate::compare::TAB_CHECKPOINTS,
                    Ok("diff") => crate::compare::TAB_DIFF,
                    _ => crate::compare::TAB_OVERVIEW,
                };
                self.compare_return = Page::Landing;
                self.page = Page::Compare;
                if let Ok(n) = std::env::var("XPR_SMOKE_COMPARE_EXPAND") {
                    self.smoke_compare_expand = n.parse().ok();
                }
                log::info!("smoke: compare page");
            }
            // `XPR_SMOKE_ACTION` drives the window into a state before the capture.
            if !self.smoke_action_done && Instant::now() >= at - Duration::from_millis(1500) && self.page == Page::Editor {
                self.smoke_action_done = true;
                match std::env::var("XPR_SMOKE_ACTION").as_deref() {
                    Ok("battle") => {
                        // select the first trainer battle; auto-switch shows the battle tab
                        let first = self.ctrl.router.all_groups().into_iter().find(|g| self.ctrl.router.group(*g).map(|x| x.event_definition.trainer_def.is_some()).unwrap_or(false));
                        if let Some(g) = first {
                            self.ctrl.select_new_events(vec![g]);
                        }
                    }
                    Ok("battle_last") => {
                        let last = self.ctrl.router.all_groups().into_iter().filter(|g| self.ctrl.router.group(*g).map(|x| x.event_definition.trainer_def.is_some()).unwrap_or(false)).last();
                        if let Some(g) = last {
                            self.ctrl.select_new_events(vec![g]);
                        }
                    }
                    Ok("last") => {
                        // select the last event of the route (its editor shows in the details panel)
                        if let Some(g) = self.ctrl.router.all_groups().into_iter().last() {
                            self.ctrl.select_new_events(vec![g]);
                        }
                    }
                    Ok("prestate") => {
                        // select the group named by `XPR_SMOKE_EVENT` (a substring; else
                        // the first trainer fight) and show its Pre-Event State tab
                        let wanted = std::env::var("XPR_SMOKE_EVENT").ok();
                        let pick = match wanted.as_deref().and_then(|w| w.strip_prefix("folder:")) {
                            // `folder:<substring>` selects a folder instead
                            Some(fname) => self.ctrl.router.all_groups().into_iter().filter_map(|g| self.ctrl.router.parent_of(g)).find(|f| {
                                self.ctrl.router.folder(*f).map(|x| x.name.contains(fname)).unwrap_or(false)
                            }),
                            None => self.ctrl.router.all_groups().into_iter().find(|g| {
                                self.ctrl.router.group(*g).map(|x| match &wanted {
                                    Some(w) => x.name.contains(w.as_str()),
                                    None => x.event_definition.trainer_def.is_some(),
                                }).unwrap_or(false)
                            }),
                        };
                        if let Some(g) = pick {
                            self.ctrl.select_new_events(vec![g]);
                            self.smoke_prestate = Some(Instant::now() + Duration::from_millis(500));
                        }
                    }
                    Ok("candy") => {
                        // select the trainer fight named by `XPR_SMOKE_FIGHT` (else the
                        // one with the most mons), then click "+" candy a few times
                        let gen = self.ctrl.gen();
                        let wanted = std::env::var("XPR_SMOKE_FIGHT").ok();
                        let best = self.ctrl.router.all_groups().into_iter().filter_map(|g| {
                            let x = self.ctrl.router.group(g)?;
                            x.event_definition.trainer_def.as_ref()?;
                            if let Some(w) = &wanted {
                                return if x.name.contains(w.as_str()) { Some((usize::MAX, g)) } else { None };
                            }
                            let n = gen.as_ref().and_then(|gen| x.event_definition.pokemon_list(gen).ok()).map(|l| l.len()).unwrap_or(0);
                            Some((n, g))
                        }).max_by_key(|(n, _)| *n);
                        if let Some((_, g)) = best {
                            self.ctrl.select_new_events(vec![g]);
                            self.smoke_candy = Some((6, Instant::now() + Duration::from_millis(600)));
                            self.smoke = Some((path.clone(), Instant::now() + Duration::from_secs(8), false));
                        }
                    }
                    Ok("record") => {
                        // start recording against `XPR_GAMEHOOK_URL`, record for
                        // `XPR_SMOKE_RECORD_SECS` (default 30), save the route as
                        // `XPR_SMOKE_SAVE_NAME` (if set), then screenshot and exit
                        if !self.ctrl.is_record_mode_active() {
                            self.record_button_clicked();
                        }
                        self.smoke_arm_record_stop(&path);
                    }
                    Ok("map") => {
                        // open the map tab; `XPR_SMOKE_EVENT=<substring>` also selects
                        // that event and shows it on the map
                        self.open_map();
                        if let Ok(w) = std::env::var("XPR_SMOKE_EVENT") {
                            let pick = self.ctrl.router.all_groups().into_iter().find(|g| self.ctrl.router.group(*g).map(|x| x.name.contains(w.as_str())).unwrap_or(false));
                            if let Some(g) = pick {
                                self.ctrl.select_new_events(vec![g]);
                                self.show_selected_on_map();
                            }
                        }
                    }
                    Ok("newroute") => self.open_new_route_window(),
                    Ok("summary") => self.open_summary_window(),
                    Ok("inline") => {
                        let first = self.ctrl.router.all_groups().into_iter().next();
                        if let Some(g) = first {
                            self.ctrl.select_new_events(vec![g]);
                            let mut la = ListActions::default();
                            self.route_list.start_add_new_event(&mut self.ctrl, &mut la);
                            self.apply_list_actions(ctx, la);
                        }
                    }
                    _ => {}
                }
            }
            if let Some((save_name, stop_at)) = self.smoke_record.clone() {
                let early = self.smoke_stop_flag.as_ref().map(|f| f.load(std::sync::atomic::Ordering::SeqCst)).unwrap_or(false);
                if early {
                    // the screenshot still happens 1.5 s after the stop
                    self.smoke = Some((path.clone(), Instant::now() + Duration::from_millis(1500), false));
                }
                if early || Instant::now() >= stop_at {
                    self.smoke_record = None;
                    log::info!("smoke: stopping recording");
                    if self.ctrl.is_record_mode_active() {
                        self.record_button_clicked();
                    }
                    if let Some(name) = save_name {
                        log::info!("smoke: saving route as {}", name);
                        self.route_name_text = name;
                        self.save_route();
                    }
                }
                ctx.request_repaint_after(Duration::from_millis(50));
            }
            if let Some(when) = self.smoke_prestate {
                if Instant::now() >= when {
                    self.smoke_prestate = None;
                    let mut actions = DetailsActions::default();
                    self.details.set_tab(&self.cfg, crate::event_details::PRE_STATE_TAB, &mut actions);
                    self.apply_details_actions(ctx, actions);
                    // `XPR_SMOKE_SPLIT=<0..1>`: the splitter fraction for the shot (not saved)
                    if let Some(f) = std::env::var("XPR_SMOKE_SPLIT").ok().and_then(|v| v.parse::<f64>().ok()) {
                        self.pre_state_fraction = Some(f);
                    }
                    // `XPR_SMOKE_NOTES=open|closed`: the notes footer state for the shot
                    match std::env::var("XPR_SMOKE_NOTES").as_deref() {
                        Ok("open") => self.details.notes.set_collapsed(false),
                        Ok("closed") => self.details.notes.set_collapsed(true),
                        _ => {}
                    }
                }
                ctx.request_repaint_after(Duration::from_millis(10));
            }
            if let Some((left, when)) = self.smoke_candy {
                if Instant::now() >= when {
                    if left > 0 {
                        log::info!("smoke: candy click {}", 7 - left);
                        self.details.increment_prefight_candies(&self.cfg, &mut self.ctrl);
                        self.smoke_candy = Some((left - 1, Instant::now() + Duration::from_millis(700)));
                    } else {
                        self.smoke_candy = None;
                    }
                }
                ctx.request_repaint_after(Duration::from_millis(10));
            }
            if Instant::now() >= at {
                // `XPR_SMOKE_EXPORT=<kind>[,<kind>...]`: run these exports (they
                // land in the images dir) right before the capture; kinds are
                // `event_list`, `battle_summary`, `player_ranges`,
                // `enemy_ranges`, `run_summary`, `setup_summary`, `compare` and
                // `matchup:<n>[:player|:enemy]`
                if let Ok(list) = std::env::var("XPR_SMOKE_EXPORT") {
                    for kind in list.split(',').filter_map(smoke_export_kind) {
                        log::info!("smoke: export {:?}", kind);
                        self.request_shot(kind);
                    }
                }
                self.smoke = Some((path, at, true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new("smoke")));
            }
            return;
        }
        let image = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { viewport_id, image, user_data, .. } if *viewport_id == egui::ViewportId::ROOT && user_data.data.as_ref().and_then(|d| d.downcast_ref::<&str>()).copied() == Some("smoke") => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = image {
            let full = Rect::from_min_size(Pos2::ZERO, egui::Vec2::new(img.size[0] as f32, img.size[1] as f32));
            if let Err(e) = save_cropped(&img, 1.0, full, &path) {
                log::error!("smoke screenshot failed: {}", e);
            } else {
                log::info!("smoke screenshot saved to {}", path.display());
            }
            self.allow_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

/// Minimal blocking `GET` for the smoke poller (no HTTP client dependency in this crate).
fn smoke_http_get(url: &str) -> Option<String> {
    use std::io::{Read, Write};
    let rest = url.strip_prefix("http://")?;
    let (host_port, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let mut stream = std::net::TcpStream::connect(host_port).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    write!(stream, "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", path, host_port).ok()?;
    let mut body = String::new();
    stream.read_to_string(&mut body).ok()?;
    Some(body)
}

impl eframe::App for XprApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let frame_start = Instant::now();
        self.update_inner(ctx);
        if self.frame_log {
            let dt = frame_start.elapsed();
            if dt > Duration::from_millis(1) {
                log::info!("frame {:.2} ms (route list {:.2} ms, details {:.2} ms)", dt.as_secs_f64() * 1000.0, self.frame_parts.0.as_secs_f64() * 1000.0, self.frame_parts.1.as_secs_f64() * 1000.0);
            }
            self.frame_parts = (Duration::ZERO, Duration::ZERO);
        }
    }

    /// Hands the deferred update request to `main`, which runs the updater
    /// once the window is gone.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.details.force_and_clear_event_update(&self.cfg, &mut self.ctrl);
        let mut e = self.exit.lock().unwrap();
        e.update_requested = self.update.requested;
        e.update_version = self.update.deferred_version.clone();
        e.update_url = self.update.deferred_url.clone();
    }
}

impl XprApp {
    fn update_inner(&mut self, ctx: &egui::Context) {
        // background results (their threads wake the UI; the timers are a fallback)
        if let Some(rx) = &self.index_rx {
            let mut done = false;
            while let Ok((idx, is_final)) = rx.try_recv() {
                self.index = idx;
                done = is_final;
            }
            if done {
                self.index_rx = None;
            } else {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        }
        if self.first_frame {
            self.first_frame = false;
            self.deferred_post_init = Some(Instant::now());
            let _ = self.fonts_installed;
        }
        // usually on the very first frame: the custom gens load while the OS
        // creates the window, so it opens straight onto the auto-loaded route
        if self.deferred_post_init.is_some() {
            // the landing page stays live while the custom gens load
            if let Some(r) = self.custom_gens_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
                self.custom_gens_rx = None;
                self.ctrl.on_custom_versions_loaded(r);
            }
            if self.custom_gens_rx.is_some() {
                ctx.request_repaint_after(Duration::from_millis(50));
            } else {
                // not held up by the index scan: until that is final the
                // auto-load finds the most recent route by mtime itself
                self.deferred_post_init = None;
                self.deferred_post_init();
            }
        }
        self.poll_update_check(ctx);
        // recorder: queued host calls + status
        self.recorder.pump(&mut self.ctrl, &self.cfg);
        self.recorder.refresh_status();
        self.poll_quick_start(ctx);
        // timers, signals, shortcuts
        self.tick_timers(ctx);
        self.dispatch_signals(ctx);
        self.handle_shortcuts(ctx);
        self.perform_pending_shots(ctx);
        self.dispatch_signals(ctx);

        let theme = self.theme.clone();
        // close request
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            if self.ctrl.has_unsaved_changes() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.save_geometry(ctx);
                self.show_message("Quit?", "Route has unsaved changes. Quit without saving?", MsgButtons::YesNo, MsgTag::QuitUnsaved);
            } else {
                self.save_geometry(ctx);
                self.allow_close = true;
            }
        }

        egui::TopBottomPanel::top("menu_bar").frame(egui::Frame::new().fill(theme.bg_darker).inner_margin(egui::Margin::symmetric(0, 1)).stroke(Stroke::new(1.0_f32, theme.border))).show(ctx, |ui| {
            self.menu_bar(ui, ctx);
        });
        egui::TopBottomPanel::bottom("status_bar").frame(egui::Frame::new().fill(theme.bg_darker).inner_margin(egui::Margin::symmetric(4, 3)).stroke(Stroke::new(1.0_f32, theme.border))).show(ctx, |ui| {
            self.status_bar(ui);
        });
        // docked run summary above the status bar
        if self.run_summary.as_ref().map(|r| r.docked).unwrap_or(false) && self.page == Page::Editor {
            let mut toggled = false;
            let mut closed = false;
            let mut export = false;
            egui::TopBottomPanel::bottom("docked_summary").frame(egui::Frame::new().fill(theme.bg)).resizable(false).show(ctx, |ui| {
                if let Some(rs) = self.run_summary.as_mut() {
                    let (t, c, e) = rs.ui(ui, &theme, &self.ctrl);
                    toggled = t;
                    closed = c;
                    export = e;
                }
            });
            if toggled {
                self.toggle_summary_dock();
            }
            if closed {
                self.run_summary = None;
            }
            if export {
                self.request_shot(ShotKind::RunSummary);
            }
        }
        let mut event_list_rect: Option<Rect> = None;
        let mut compare_actions: Option<crate::compare::CompareActions> = None;
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme.bg)).show(ctx, |ui| match self.page {
            Page::Landing => {
                if let Some(qs) = &self.quick_start {
                    let mut actions = QuickStartActions::default();
                    quick_start_ui(ui, &theme, &qs.phase(), qs.url(), &mut actions);
                    if actions.use_current {
                        qs.accept_current_pokemon();
                    }
                    if actions.reconnect {
                        qs.reconnect();
                    }
                    if actions.cancel {
                        self.cancel_quick_start();
                    }
                } else {
                    let mut actions = LandingActions::default();
                    let t0 = Instant::now();
                    self.landing.ui(ui, &theme, &mut self.cfg, &self.paths, &self.registry, &self.index, &mut actions);
                    if self.frame_log {
                        let dt = t0.elapsed();
                        if dt > Duration::from_millis(1) {
                            log::info!("landing page {:.2} ms ({} routes indexed)", dt.as_secs_f64() * 1000.0, self.index.entries.len());
                        }
                    }
                    if actions.create_route {
                        self.open_new_route_window();
                    }
                    if actions.start_recording {
                        self.begin_quick_start(ctx);
                    }
                    if let Some(p) = actions.load_route {
                        self.ctrl.load_route(&p);
                        self.show_route_controls();
                    }
                    if let Some(a) = actions.compare_routes {
                        self.open_compare_with(a.map(RouteSource::Path));
                    }
                }
            }
            Page::NewRoute => {
                let mut actions = NewRouteActions::default();
                self.new_route.ui(ui, &theme, &self.registry, &self.paths, &self.index, &mut self.assets, &mut actions);
                if let Some(e) = actions.error {
                    self.show_message("Error", &e, MsgButtons::Ok, MsgTag::NewRouteError);
                }
                if actions.cancel {
                    if self.route_loaded_before_new_route {
                        self.show_route_controls();
                    } else {
                        self.show_landing_page();
                    }
                    self.route_loaded_before_new_route = false;
                }
                if let Some(req) = actions.create {
                    self.ctrl.create_new_route(&req.solo_mon, req.base_route_path.as_deref(), &req.version, Some(req.dvs), Some(req.ability_idx), Some(req.nature));
                }
            }
            Page::Editor => {
                event_list_rect = self.editor_page(ui, ctx);
            }
            Page::Compare => {
                self.compare_width = Some(ui.available_width());
                compare_actions = Some(self.compare_page(ui));
            }
        });
        if let Some(actions) = compare_actions {
            if actions.back {
                self.page = self.compare_return;
            }
            if actions.export {
                self.request_shot(ShotKind::Compare);
            }
            if let Some(text) = actions.copy_summary {
                ctx.copy_text(text);
                self.toast.show("Summary copied", 3000, None);
            }
        }
        // popover / dialogs / toast
        if let Some(trainer) = self.quick_add.ui(ctx, &theme, &mut self.ctrl, &mut self.assets) {
            self.ctrl.set_preview_trainer(&trainer);
        }
        self.draw_dialogs(ctx);
        if let Some(folder) = self.toast.ui(ctx, &theme) {
            io_utils::open_explorer(&folder);
        }
        // secondary windows
        let mut setup_summary_rect: Option<Rect> = None;
        if self.setup_summary_open {
            let text = setup_summary_text(&self.ctrl);
            let mut close = false;
            let mut export = false;
            let theme2 = theme.clone();
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("setup_summary"),
                egui::ViewportBuilder::default().with_title("Setup Summary").with_inner_size([420.0, 300.0]),
                |ctx, _class| {
                    egui::TopBottomPanel::top("setup_menu").frame(egui::Frame::new().fill(theme2.bg_darker)).show(ctx, |ui| {
                        if widgets::button(ui, &theme2, "Export Screenshot (Ctrl+P)").clicked() || ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::P)) {
                            export = true;
                        }
                    });
                    egui::CentralPanel::default().frame(egui::Frame::new().fill(theme2.bg).inner_margin(egui::Margin::same(15))).show(ctx, |ui| {
                        ui.add(egui::Label::new(egui::RichText::new(&text).font(theme2.body()).color(theme2.text)).wrap());
                        setup_summary_rect = Some(ctx.content_rect());
                    });
                    if ctx.input(|i| i.viewport().close_requested()) {
                        close = true;
                    }
                },
            );
            if close {
                self.setup_summary_open = false;
            }
            if export {
                self.request_shot(ShotKind::SetupSummary);
            }
        }
        if self.run_summary.as_ref().map(|r| !r.docked).unwrap_or(false) {
            let mut toggled = false;
            let mut closed = false;
            let mut export = false;
            let size = self.run_summary.as_ref().map(|r| r.cached_size).unwrap_or(Vec2::new(600.0, 200.0));
            let theme2 = theme.clone();
            let run_summary = self.run_summary.as_mut().unwrap();
            let ctrl = &self.ctrl;
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("run_summary"),
                egui::ViewportBuilder::default().with_title("Route Summary").with_inner_size([size.x.max(300.0), size.y.max(120.0)]),
                |ctx, _class| {
                    egui::CentralPanel::default().frame(egui::Frame::new().fill(theme2.bg)).show(ctx, |ui| {
                        let (t, c, e) = run_summary.ui(ui, &theme2, ctrl);
                        toggled = t;
                        closed = c;
                        export = e;
                    });
                    if ctx.input(|i| i.viewport().close_requested()) {
                        closed = true;
                    }
                    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Backtick)) {
                        closed = true;
                    }
                },
            );
            if toggled {
                self.toggle_summary_dock();
            }
            if closed {
                self.run_summary = None;
            }
            if export {
                self.request_shot(ShotKind::RunSummary);
            }
        }
        if self.map.open && !self.map.docked && self.page == Page::Editor {
            let theme2 = theme.clone();
            let mut closed = false;
            let mut map_actions: Vec<MapAction> = Vec::new();
            {
                let map = &mut self.map;
                let cfg = &mut self.cfg;
                let ctrl = &self.ctrl;
                let assets = &mut self.assets;
                ctx.show_viewport_immediate(
                    egui::ViewportId::from_hash_of("world_map"),
                    egui::ViewportBuilder::default().with_title("Map").with_inner_size([960.0, 720.0]),
                    |ctx, _class| {
                        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme2.bg).inner_margin(egui::Margin::same(0))).show(ctx, |ui| {
                            map_actions = map.ui(ui, &theme2, cfg, ctrl, assets);
                        });
                        if ctx.input(|i| i.viewport().close_requested()) {
                            closed = true;
                        }
                    },
                );
            }
            if closed {
                self.close_map();
            }
            self.apply_map_actions(ctx, map_actions);
        }
        self.event_list_rect = event_list_rect;
        self.setup_summary_rect = setup_summary_rect;
        self.dispatch_signals(ctx);
        self.smoke_tick(ctx);
    }
}

/// One `XPR_SMOKE_EXPORT` entry.
fn smoke_export_kind(name: &str) -> Option<ShotKind> {
    let mut parts = name.trim().split(':');
    match parts.next()? {
        "event_list" => Some(ShotKind::EventList),
        "compare" => Some(ShotKind::Compare),
        "battle_summary" => Some(ShotKind::BattleSummary),
        "player_ranges" => Some(ShotKind::PlayerRanges),
        "enemy_ranges" => Some(ShotKind::EnemyRanges),
        "run_summary" => Some(ShotKind::RunSummary),
        "setup_summary" => Some(ShotKind::SetupSummary),
        "matchup" => {
            let idx = parts.next()?.parse::<usize>().ok()?.checked_sub(1)?;
            let mode = match parts.next() {
                Some("player") => ScreenshotMode::Player,
                Some("enemy") => ScreenshotMode::Enemy,
                _ => ScreenshotMode::Full,
            };
            Some(ShotKind::Matchup { idx, mode })
        }
        _ => None,
    }
}

/// Font/geometry helpers used by `main`.
pub fn initial_viewport(cfg: &Config) -> egui::ViewportBuilder {
    let mut vb = egui::ViewportBuilder::default().with_title("Pokemon Solo Challenge Router").with_app_id("pkmn_xp_router");
    if let Some(icon) = crate::assets::app_icon() {
        vb = vb.with_icon(icon);
    }
    let geo = Geometry::parse(&cfg.get_window_geometry());
    match geo {
        Some(g) => {
            vb = vb.with_inner_size([g.width as f32, g.height as f32]);
            if let (Some(x), Some(y)) = (g.x, g.y) {
                vb = vb.with_position([x as f32, y as f32]);
            }
        }
        None => vb = vb.with_inner_size([2000.0, 1200.0]),
    }
    if cfg.get_window_state() == "zoomed" {
        vb = vb.with_maximized(true);
    }
    vb
}

/// Where the icon would come from (kept for parity; the Qt app sets none).
pub fn app_paths(source_root: &std::path::Path) -> Paths {
    Paths::new(source_root.to_path_buf())
}

#[allow(dead_code)]
fn _unused(_r: Registry, _t: Theme) {
    let _ = BATTLE_SUMMARY_TAB;
}
