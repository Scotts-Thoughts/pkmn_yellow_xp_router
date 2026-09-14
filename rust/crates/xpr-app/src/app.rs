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
use xpr_ui_kit::theme::{self, Theme};
use xpr_ui_kit::widgets::{self, Entry};
use xpr_ui_kit::{AutoClearingLabel, Geometry, ShortcutMap, ToastHost};

use crate::assets::Assets;
use crate::battle_ui::ScreenshotMode;
use crate::controller::MainController;
use crate::dialogs::{
    AppConfigDialog, AssignMoveDialog, BattleConfigDialog, ColorConfigDialog, CustomDvsDialog, CustomGenDialog, Dialog,
    DialogCtx, DialogOutcome, FinalTrainersDialog, HighlightColorDialog, LoadRouteDialog, MatchupExportDialog, MessageBox,
    MsgButtons, MsgChoice, MsgTag, NewFolderDialog, ShortcutsDialog, TransferDialog,
};
use crate::event_details::{DetailsActions, EventDetails, BATTLE_SUMMARY_TAB};
use crate::filter_bar::FilterBar;
use crate::pages::{LandingActions, LandingPage, NewRouteActions, NewRoutePage};
use crate::quick_add::QuickAddPopover;
use crate::recorder_glue::Recorder;
use crate::route_index::RouteIndex;
use crate::route_list::{ListActions, RouteList};
use crate::screenshot::{save_cropped, PendingShot, ShotKind};
use crate::summaries::{setup_summary_text, RunSummary};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Landing,
    NewRoute,
    Editor,
}

/// What `main` needs to know after the window closes.
#[derive(Clone, Debug, Default)]
pub struct ExitState {
    pub update_requested: bool,
    pub update_version: Option<String>,
    pub update_url: Option<String>,
}

pub type SharedExit = Arc<std::sync::Mutex<ExitState>>;

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
    quick_add: QuickAddPopover,
    dialog: Option<Dialog>,
    message: Option<MessageBox>,
    toast: ToastHost,
    message_label: AutoClearingLabel,
    index: RouteIndex,
    index_rx: Option<Receiver<RouteIndex>>,
    run_summary: Option<RunSummary>,
    setup_summary_open: bool,
    recorder: Recorder,
    route_name_text: String,
    image_path_text: String,
    route_loaded_before_new_route: bool,
    auto_load_checked: bool,
    deferred_post_init: Option<Instant>,
    first_frame: bool,
    update: UpdateState,
    pre_state_fraction: Option<f64>,
    battle_fraction: Option<f64>,
    splitter_save_deadline: Option<Instant>,
    splitter_dragging: bool,
    initial_splitter_applied: bool,
    pending_shot: Option<PendingShot>,
    shot_counter: u64,
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
    /// `XPR_SMOKE_ACTION=record`: (save-as name, when to stop recording at the latest)
    smoke_record: Option<(Option<String>, Instant)>,
    /// set by the `XPR_SMOKE_STOP_URL` poller once the mock reports the scenario is done
    smoke_stop_flag: Option<Arc<std::sync::atomic::AtomicBool>>,
}

impl XprApp {
    pub fn new(cc: &eframe::CreationContext<'_>, cfg: Config, paths: Paths, registry: Arc<Registry>, exit: SharedExit) -> XprApp {
        let mut theme = Theme::from_config(&cfg);
        theme.install_fonts(&cc.egui_ctx);
        theme.apply(&cc.egui_ctx);
        let ctrl = MainController::new(registry.clone(), paths.clone());
        let assets = Assets::new();
        let recorder = Recorder::new(cc.egui_ctx.clone());
        // background: route index scan and the start-up update check
        let (tx, rx) = channel();
        let scan_paths = paths.clone();
        std::thread::spawn(move || {
            let mut idx = RouteIndex::load(&scan_paths);
            idx.refresh(&scan_paths);
            let _ = tx.send(idx);
        });
        let (utx, urx) = channel();
        std::thread::spawn(move || {
            // `XPR_DISABLE_AUTO_UPDATE`: no request to GitHub at start
            // (isolated / offline test runs), so the check just reports "no
            // release information" and nothing is prompted.
            let info = if std::env::var_os("XPR_DISABLE_AUTO_UPDATE").is_some() { xpr_update::ReleaseInfo::default() } else { xpr_update::get_new_version_info() };
            let _ = utx.send(info);
        });
        let shortcuts = ShortcutMap::from_config(&cfg);
        let landing = LandingPage::new(&cfg);
        let route_list = RouteList::new(&cfg);
        let details = EventDetails::new(&cfg);
        let pre_state_fraction = cfg.get_pre_state_left_fraction();
        let battle_fraction = cfg.get_battle_summary_left_fraction();
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
            quick_add: QuickAddPopover::new(),
            dialog: None,
            message: None,
            toast: ToastHost::default(),
            message_label: AutoClearingLabel::new(3000),
            index: RouteIndex::default(),
            index_rx: Some(rx),
            run_summary: None,
            setup_summary_open: false,
            recorder,
            route_name_text: String::new(),
            image_path_text: String::new(),
            route_loaded_before_new_route: false,
            auto_load_checked: false,
            deferred_post_init: None,
            first_frame: true,
            update: UpdateState { rx: Some(urx), startup_check: true, deferred_version: None, deferred_url: None, requested: false, check_button_enabled: true },
            pre_state_fraction,
            battle_fraction,
            splitter_save_deadline: None,
            splitter_dragging: false,
            initial_splitter_applied: false,
            pending_shot: None,
            shot_counter: 0,
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
        self.new_route.refresh_game_list(&self.registry, &self.paths);
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

    /// `_deferred_post_init`: custom gens, then the auto-load.
    fn deferred_post_init(&mut self) {
        self.ctrl.load_all_custom_versions();
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
        self.refresh_index_now();
    }

    fn find_most_recent_route(&self) -> Option<PathBuf> {
        if self.index.loaded {
            return self.index.most_recent(&self.paths);
        }
        // index not ready yet: scan mtimes directly
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
        if let Some(_is_battle) = actions.tab_changed {
            // the splitter re-proportions from the saved fractions (drawn from state)
        }
        if actions.battle.open_config {
            self.dialog = Some(Dialog::BattleConfig(BattleConfigDialog::new(&self.cfg)));
        }
        if let Some(slot) = actions.battle.assign_move_slot {
            if let Some(gen) = self.ctrl.gen() {
                self.dialog = Some(Dialog::AssignMove(AssignMoveDialog::new(&gen, slot)));
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
        let filters: [(&str, &str); 16] = [
            ("filter_trainer", consts::TASK_TRAINER_BATTLE),
            ("filter_rare_candy", consts::TASK_RARE_CANDY),
            ("filter_tm_hm", consts::TASK_LEARN_MOVE_TM),
            ("filter_vitamin", consts::TASK_VITAMIN),
            ("filter_wild_pkmn", consts::TASK_FIGHT_WILD_PKMN),
            ("filter_acquire_item", consts::TASK_GET_FREE_ITEM),
            ("filter_purchase_item", consts::TASK_PURCHASE_ITEM),
            ("filter_use_item", consts::TASK_USE_ITEM),
            ("filter_sell_item", consts::TASK_SELL_ITEM),
            ("filter_hold_item", consts::TASK_HOLD_ITEM),
            ("filter_levelup_move", consts::TASK_LEARN_MOVE_LEVELUP),
            ("filter_save", consts::TASK_SAVE),
            ("filter_heal", consts::TASK_HEAL),
            ("filter_blackout", consts::TASK_BLACKOUT),
            ("filter_evolution", consts::TASK_EVOLUTION),
            ("filter_notes", consts::TASK_NOTES_ONLY),
        ];
        for (aid, et) in filters {
            if fire(aid) && !text_focus {
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

    fn request_shot(&mut self, kind: ShotKind) {
        if self.ctrl.is_empty() && kind != ShotKind::SetupSummary && kind != ShotKind::RunSummary {
            return;
        }
        match kind {
            ShotKind::BattleSummary | ShotKind::PlayerRanges | ShotKind::EnemyRanges => {
                if !self.details.is_battle_tab() {
                    return;
                }
            }
            _ => {}
        }
        let route = self.ctrl.get_current_route_name().to_string();
        let image_name = match &kind {
            ShotKind::EventList => "event_list".to_string(),
            ShotKind::BattleSummary => "battle_summary".to_string(),
            ShotKind::PlayerRanges => "player_ranges".to_string(),
            ShotKind::EnemyRanges => "enemy_ranges".to_string(),
            ShotKind::Matchup { idx, mode } => match mode {
                ScreenshotMode::Full => format!("matchup_{}", idx + 1),
                ScreenshotMode::Player => format!("matchup_{}_player_ranges", idx + 1),
                ScreenshotMode::Enemy => format!("matchup_{}_enemy_ranges", idx + 1),
            },
            ShotKind::RunSummary => "run_summary".to_string(),
            ShotKind::SetupSummary => "setup_summary".to_string(),
        };
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
        let _ = route;
        let out_path = self.ctrl.screenshot_path(&dir, &image_name);
        let viewport = match &kind {
            ShotKind::RunSummary if self.run_summary.as_ref().map(|r| !r.docked).unwrap_or(false) => egui::ViewportId::from_hash_of("run_summary"),
            ShotKind::SetupSummary => egui::ViewportId::from_hash_of("setup_summary"),
            _ => egui::ViewportId::ROOT,
        };
        match &kind {
            ShotKind::BattleSummary => self.details.battle_ui.screenshot_mode = Some(ScreenshotMode::Full),
            ShotKind::PlayerRanges => self.details.battle_ui.screenshot_mode = Some(ScreenshotMode::Player),
            ShotKind::EnemyRanges => self.details.battle_ui.screenshot_mode = Some(ScreenshotMode::Enemy),
            ShotKind::Matchup { mode, .. } => self.details.battle_ui.screenshot_mode = Some(*mode),
            _ => {}
        }
        self.pending_shot = Some(PendingShot { kind, viewport, rect: None, out_path, requested: false });
    }

    /// After drawing: resolve the capture rectangle and request the screenshot.
    fn resolve_shot(&mut self, ctx: &egui::Context, event_list_rect: Option<Rect>, run_summary_rect: Option<Rect>, setup_summary_rect: Option<Rect>) {
        let Some(shot) = self.pending_shot.as_mut() else { return };
        if shot.requested {
            return;
        }
        let geo = self.details.battle_ui.geometry.clone();
        let rect = match &shot.kind {
            ShotKind::EventList => event_list_rect,
            ShotKind::RunSummary => run_summary_rect,
            ShotKind::SetupSummary => setup_summary_rect,
            ShotKind::BattleSummary => {
                let (Some(base), false) = (geo.base_rect, geo.mon_pair_rects.is_empty()) else { return };
                let top = geo.mon_pair_rects.iter().map(|r| r.min.y).fold(f32::MAX, f32::min);
                let bottom = geo.mon_pair_rects.iter().map(|r| r.max.y).fold(f32::MIN, f32::max);
                Some(Rect::from_min_max(Pos2::new(base.min.x, top), Pos2::new(base.max.x, bottom)))
            }
            ShotKind::PlayerRanges => {
                let (Some(base), false) = (geo.base_rect, geo.mon_pair_rects.is_empty()) else { return };
                let top = geo.mon_pair_rects.iter().map(|r| r.min.y).fold(f32::MAX, f32::min);
                let bottom = geo.mon_pair_rects.iter().map(|r| r.max.y).fold(f32::MIN, f32::max);
                let split = geo.divider_x.map(|d| d.0).unwrap_or(base.center().x);
                Some(Rect::from_min_max(Pos2::new(base.min.x, top), Pos2::new(split, bottom)))
            }
            ShotKind::EnemyRanges => {
                let (Some(base), false) = (geo.base_rect, geo.mon_pair_rects.is_empty()) else { return };
                let top = geo.mon_pair_rects.iter().map(|r| r.min.y).fold(f32::MAX, f32::min);
                let bottom = geo.mon_pair_rects.iter().map(|r| r.max.y).fold(f32::MIN, f32::max);
                let split = geo.divider_x.map(|d| d.1).unwrap_or(base.center().x);
                Some(Rect::from_min_max(Pos2::new(split, top), Pos2::new(base.max.x, bottom)))
            }
            ShotKind::Matchup { idx, mode } => {
                // the visible matchups are in display order
                let visible: Vec<usize> = (0..6).filter(|i| self.details.bc.get_pkmn_info(*i, true).is_some() || self.details.bc.get_pkmn_info(*i, false).is_some()).collect();
                let pos = visible.iter().position(|i| i == idx);
                let Some(r) = pos.and_then(|p| geo.mon_pair_rects.get(p).copied()) else { return };
                match mode {
                    ScreenshotMode::Full => Some(r),
                    ScreenshotMode::Player => {
                        let split = geo.divider_x.map(|d| d.0).unwrap_or(r.center().x);
                        Some(Rect::from_min_max(r.min, Pos2::new(split, r.max.y)))
                    }
                    ScreenshotMode::Enemy => {
                        let split = geo.divider_x.map(|d| d.1).unwrap_or(r.center().x);
                        Some(Rect::from_min_max(Pos2::new(split, r.min.y), r.max))
                    }
                }
            }
        };
        let Some(r) = rect else {
            self.pending_shot = None;
            self.details.battle_ui.screenshot_mode = None;
            return;
        };
        shot.rect = Some(r);
        shot.requested = true;
        self.shot_counter += 1;
        let tag = crate::screenshot::ShotTag(self.shot_counter);
        ctx.send_viewport_cmd_to(shot.viewport, egui::ViewportCommand::Screenshot(egui::UserData::new(tag)));
        ctx.request_repaint();
    }

    fn handle_screenshot_events(&mut self, ctx: &egui::Context) {
        let Some(shot) = self.pending_shot.clone() else { return };
        if !shot.requested {
            return;
        }
        let image = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { viewport_id, image, .. } if *viewport_id == shot.viewport => Some(image.clone()),
                _ => None,
            })
        });
        let Some(image) = image else { return };
        let ppp = ctx.pixels_per_point();
        let corners = match &shot.kind {
            ShotKind::PlayerRanges | ShotKind::EnemyRanges | ShotKind::BattleSummary | ShotKind::Matchup { .. } => {
                let geo = self.details.battle_ui.geometry.clone();
                let r = shot.rect.unwrap_or(Rect::NOTHING);
                Some(geo.mon_pair_rects.iter().map(|m| m.intersect(r)).filter(|m| m.is_positive()).collect::<Vec<Rect>>())
            }
            _ => None,
        };
        match save_cropped(&image, ppp, shot.rect.unwrap_or(Rect::NOTHING), &shot.out_path, corners) {
            Ok(_) => self.ctrl.send_message(format!("Saved screenshot to: {}", shot.out_path.display())),
            Err(e) => self.ctrl.trigger_exception(format!("Couldn't save screenshot due to exception! {}", e)),
        }
        self.pending_shot = None;
        self.details.battle_ui.screenshot_mode = None;
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
            DialogOutcome::AssignMove { slot, mv } => {
                self.details.bc.assign_player_move_via_tutor(&self.cfg, &mut self.ctrl, slot, &mv);
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
            let version = self.ctrl.get_version().map(|s| s.to_string());
            let (vtext, vcolor) = match &version {
                Some(v) => (format!("{} Version", v), XprApp::version_color(v)),
                None => ("Version".to_string(), Color32::WHITE),
            };
            widgets::chip(ui, &theme, &vtext, vcolor, Color32::BLACK, Vec2::new(10.0, 6.0), false);
            let (rtext, rcolor) = if self.ctrl.has_errors() {
                ("Run Status: Invalid", theme::parse_hex(consts::ERROR_COLOR))
            } else {
                ("Run Status: Valid", theme::parse_hex(consts::VALID_COLOR))
            };
            if widgets::chip(ui, &theme, rtext, rcolor, Color32::BLACK, Vec2::new(10.0, 6.0), true).clicked() {
                self.cycle_invalid_events();
            }
            widgets::label_colored(ui, &theme, "Route Name:", theme.secondary);
            let r = Entry::new(&theme, &mut self.route_name_text).width(200.0).id(ui.id().with("route_name")).show(ui);
            if r.changed && self.route_name_text != self.ctrl.get_current_route_name() {
                let n = self.route_name_text.clone();
                self.ctrl.set_current_route_name(&n);
            }
            widgets::label_colored(ui, &theme, "Image Path:", theme.secondary);
            let r = Entry::new(&theme, &mut self.image_path_text).width(200.0).id(ui.id().with("image_path")).show(ui);
            if r.changed {
                let p = self.image_path_text.clone();
                self.ctrl.set_custom_image_path(&p);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let active = self.ctrl.is_record_mode_active();
                let enabled = self.ctrl.get_version().is_some();
                let color = if active { Color32::from_rgb(0xe7, 0x4c, 0x3c) } else if enabled { Color32::from_rgb(0x88, 0x88, 0x88) } else { Color32::from_rgb(0x55, 0x55, 0x55) };
                let hover = if active { Color32::from_rgb(0xff, 0x6b, 0x5b) } else { Color32::from_rgb(0xaa, 0xaa, 0xaa) };
                let btn = widgets::StyledButton::new(&theme, egui::RichText::new("● Record").font(theme.font(10.5)).color(color)).enabled(enabled).text_color(color).hover_fill(theme.bg_lighter).show(ui);
                let _ = hover;
                if btn.clicked() {
                    self.record_button_clicked();
                }
                if active {
                    if widgets::StyledButton::new(&theme, "↻").min_size(Vec2::new(24.0, 24.0)).padding(Vec2::ZERO).enabled(self.recorder.reconnect_enabled).show(ui).clicked() {
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
        let is_battle = self.details.is_battle_tab();
        let mut left = if is_battle {
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
                event_list_rect = Some(inner.response.rect);
            });
        }
        // splitter handle
        let resp = ui.interact(handle_rect, ui.id().with("splitter"), Sense::drag());
        let handle_color = if resp.hovered() || resp.dragged() { theme.accent } else { theme.border };
        ui.painter().rect_filled(handle_rect, CornerRadius::ZERO, handle_color);
        if resp.hovered() || resp.dragged() {
            ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        if resp.dragged() {
            let new_left = (left + resp.drag_delta().x).clamp(200.0, (total - 200.0).max(200.0));
            let fraction = (new_left / total) as f64;
            if is_battle {
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
        event_list_rect
    }

    fn tick_timers(&mut self, ctx: &egui::Context) {
        if let Some(d) = self.splitter_save_deadline {
            if Instant::now() >= d {
                self.splitter_save_deadline = None;
                if self.details.is_battle_tab() {
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
    /// The unattended smoke test: screenshot, then close.
    fn smoke_tick(&mut self, ctx: &egui::Context) {
        let Some((path, at, requested)) = self.smoke.clone() else { return };
        ctx.request_repaint_after(Duration::from_millis(100));
        if !requested {
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
                        let secs: u64 = std::env::var("XPR_SMOKE_RECORD_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(30);
                        let save_name = std::env::var("XPR_SMOKE_SAVE_NAME").ok().filter(|n| !n.is_empty());
                        if !self.ctrl.is_record_mode_active() {
                            self.record_button_clicked();
                        }
                        let stop_at = Instant::now() + Duration::from_secs(secs);
                        self.smoke_record = Some((save_name, stop_at));
                        self.smoke = Some((path.clone(), stop_at + Duration::from_millis(1500), false));
                        // `XPR_SMOKE_STOP_URL`: stop earlier, ~3 s after this JSON endpoint
                        // reports `"done": true` (the mock GameHook's /mock/status)
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
            if let Err(e) = save_cropped(&img, 1.0, full, &path, None) {
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
        if self.first_frame {
            self.first_frame = false;
            self.deferred_post_init = Some(Instant::now() + Duration::from_millis(200));
            ctx.request_repaint_after(Duration::from_millis(200));
            let _ = self.fonts_installed;
        }
        if let Some(t) = self.deferred_post_init {
            if Instant::now() >= t {
                self.deferred_post_init = None;
                self.deferred_post_init();
            } else {
                ctx.request_repaint_after(t - Instant::now());
            }
        }
        // background results
        if let Some(rx) = &self.index_rx {
            if let Ok(idx) = rx.try_recv() {
                self.index = idx;
                self.index_rx = None;
            } else {
                ctx.request_repaint_after(Duration::from_millis(200));
            }
        }
        self.poll_update_check(ctx);
        // recorder: queued host calls + status
        self.recorder.pump(&mut self.ctrl, &self.cfg);
        self.recorder.refresh_status();
        // timers, signals, shortcuts
        self.tick_timers(ctx);
        self.dispatch_signals(ctx);
        self.handle_shortcuts(ctx);
        self.handle_screenshot_events(ctx);
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
        let mut run_summary_rect: Option<Rect> = None;
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
                run_summary_rect = Some(ui.min_rect());
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
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme.bg)).show(ctx, |ui| match self.page {
            Page::Landing => {
                let mut actions = LandingActions::default();
                self.landing.ui(ui, &theme, &mut self.cfg, &self.paths, &self.registry, &self.index, &mut actions);
                if actions.create_route {
                    self.open_new_route_window();
                }
                if let Some(p) = actions.load_route {
                    self.ctrl.load_route(&p);
                    self.show_route_controls();
                }
            }
            Page::NewRoute => {
                let mut actions = NewRouteActions::default();
                self.new_route.ui(ui, &theme, &self.registry, &self.paths, &mut self.assets, &mut actions);
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
        });
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
                        run_summary_rect = Some(ui.min_rect());
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
        self.resolve_shot(ctx, event_list_rect, run_summary_rect, setup_summary_rect);
        self.dispatch_signals(ctx);
        self.smoke_tick(ctx);
    }
}

/// Font/geometry helpers used by `main`.
pub fn initial_viewport(cfg: &Config) -> egui::ViewportBuilder {
    let mut vb = egui::ViewportBuilder::default().with_title("Pokemon RBY XP Router").with_app_id("pkmn_xp_router");
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
