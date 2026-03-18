import os
import logging
import threading
from functools import partial

from PySide6.QtWidgets import (
    QMainWindow, QWidget, QLabel, QPushButton, QLineEdit, QVBoxLayout,
    QHBoxLayout, QGridLayout, QSplitter, QTabWidget, QStackedWidget,
    QTreeView, QScrollBar, QMenuBar, QMenu, QCheckBox, QMessageBox,
    QSizePolicy, QFrame, QPlainTextEdit, QApplication,
)
from PySide6.QtCore import Qt, QObject, QTimer, QByteArray, Signal, Slot
from PySide6.QtGui import QAction, QKeySequence, QShortcut, QFont

from controllers.main_controller import MainController
from route_recording.recorder import RecorderController
from utils.constants import const
from utils.config_manager import config
from utils import io_utils
from routing.route_events import EventFolder

# Import Qt custom components
from gui_qt.components.custom_components import (
    SimpleButton, SimpleEntry, AutoClearingLabel, NotificationPopup,
    CheckboxLabel, SimpleOptionMenu,
)

# Attempt imports of sub-components; fall back to placeholders when the real
# widgets have not been ported yet.
try:
    from gui_qt.components.quick_add_components import (
        QuickTrainerAdd, QuickItemAdd, QuickWildPkmn, QuickMiscEvents,
    )
except ImportError:
    class _PlaceholderQuickAdd(QWidget):
        """Minimal stand-in used until the real quick-add widgets are ported."""
        def __init__(self, controller, parent=None):
            super().__init__(parent)
            self._controller = controller
            lbl = QLabel(self.__class__.__name__, self)
            lay = QVBoxLayout(self)
            lay.setContentsMargins(2, 2, 2, 2)
            lay.addWidget(lbl)
        def trainer_filter_callback(self):
            pass
    class QuickTrainerAdd(_PlaceholderQuickAdd):
        pass
    class QuickItemAdd(_PlaceholderQuickAdd):
        pass
    class QuickWildPkmn(_PlaceholderQuickAdd):
        pass
    class QuickMiscEvents(_PlaceholderQuickAdd):
        pass

try:
    from gui_qt.components.route_search import RouteSearch
except ImportError:
    class RouteSearch(QWidget):
        def __init__(self, controller, parent=None):
            super().__init__(parent)
            self._controller = controller
            lbl = QLabel("RouteSearch (placeholder)", self)
            lay = QVBoxLayout(self)
            lay.setContentsMargins(2, 2, 2, 2)
            lay.addWidget(lbl)
        def toggle_filter_by_type(self, filter_type):
            pass
        def is_filter_checked(self, filter_type):
            return False
        def set_filter_by_type(self, filter_type, checked):
            pass
        def reset_all_filters(self):
            pass

try:
    from gui_qt.components.recorder_status import RecorderStatus
except ImportError:
    class RecorderStatus(QObject):
        def __init__(self, main_controller, recorder_controller, client_status_label, reconnect_button, parent=None):
            super().__init__(parent)
        def on_recording_mode_changed(self):
            pass

try:
    from gui_qt.pages.landing_page import LandingPage
except ImportError:
    class LandingPage(QWidget):
        def __init__(self, parent, controller, on_create_route=None, on_load_route=None, on_auto_load_toggle=None):
            super().__init__(parent)
            self._controller = controller
            lbl = QLabel("LandingPage (placeholder)", self)
            lay = QVBoxLayout(self)
            lay.addWidget(lbl)
        def refresh_routes(self):
            pass

try:
    from gui_qt.pages.new_route_page import NewRoutePage
except ImportError:
    class NewRoutePage(QWidget):
        def __init__(self, parent, controller, on_cancel=None, on_create=None):
            super().__init__(parent)
            self._controller = controller
            lbl = QLabel("NewRoutePage (placeholder)", self)
            lay = QVBoxLayout(self)
            lay.addWidget(lbl)
        def refresh_game_list(self):
            pass

try:
    from gui_qt.pkmn_components.route_list import RouteList
except ImportError:
    class RouteList(QTreeView):
        def __init__(self, controller, parent=None):
            super().__init__(parent)
            self._controller = controller
        def refresh(self):
            pass
        def get_all_selected_event_ids(self, allow_event_items=True):
            return []
        def set_all_selected_event_ids(self, ids):
            pass
        def scroll_to_selected_events(self):
            pass
        def scroll_to_top(self):
            self.scrollToTop()
        def scroll_to_bottom(self):
            self.scrollToBottom()
        def trigger_checkbox(self):
            pass
        def update_folder_text_style(self):
            pass

from gui_qt.event_details import EventDetails

logger = logging.getLogger(__name__)


class MainWindow(QMainWindow):
    """Main application window -- PySide6 port of the tkinter MainWindow."""

    # ------------------------------------------------------------------
    # Initialisation
    # ------------------------------------------------------------------
    def __init__(self, controller: MainController):
        super().__init__()
        self._controller = controller
        self._recorder_controller = RecorderController(self._controller)

        # Bookkeeping
        self._text_field_has_focus = False
        self._popup_open = False
        self._loading_route_name = False
        self._route_loaded_before_new_route = False
        self._auto_load_checked = False
        self._unsubscribers: list = []

        self.new_event_window = None
        self.summary_window = None
        self.setup_summary_window = None

        self.setWindowTitle("Pokemon RBY XP Router")
        self._restore_geometry()

        # ---- Build UI ------------------------------------------------
        self._build_menu_bar()
        self._build_central_widget()
        self._build_status_bar()
        self.recorder_status = RecorderStatus(
            self._controller, self._recorder_controller,
            self._sb_client_status, self._sb_reconnect_btn,
            parent=self,
        )
        self._build_shortcuts()
        self._register_controller_callbacks()

        # Initial state
        self.event_list.refresh()

        # Show landing page initially; auto-load happens in run() after gens load
        self._show_landing_page()

    # ------------------------------------------------------------------
    # run()
    # ------------------------------------------------------------------
    def run(self):
        """Show the window and perform deferred initialisation.

        ``QApplication.exec()`` is called from the entry-point (main.pyw), so
        this method only needs to make the window visible and kick off any work
        that should happen after the event-loop starts.
        """
        self.show()

        # Deferred: wait for background gen loading, load custom gens, then auto-load route
        QTimer.singleShot(200, self._deferred_post_init)

    # ------------------------------------------------------------------
    # Geometry persistence
    # ------------------------------------------------------------------
    def _restore_geometry(self):
        geo = config.get_window_geometry()
        if geo:
            # geo is a tkinter geometry string like "2000x1200+100+50"
            try:
                parts = geo.replace("+", " +").replace("-", " -").split()
                wh = parts[0].split("x")
                w, h = int(wh[0]), int(wh[1])
                self.resize(w, h)
                if len(parts) >= 3:
                    x, y = int(parts[1]), int(parts[2])
                    self.move(x, y)
            except Exception:
                self.resize(2000, 1200)
        else:
            self.resize(2000, 1200)

        state = config.get_window_state()
        if state == "zoomed":
            self.showMaximized()

    def _save_geometry(self):
        geo = self.geometry()
        geo_str = f"{geo.width()}x{geo.height()}+{geo.x()}+{geo.y()}"
        config.set_window_geometry(geo_str)
        if self.isMaximized():
            config.set_window_state("zoomed")
        elif self.isMinimized():
            config.set_window_state("iconic")
        else:
            config.set_window_state("normal")

    # ------------------------------------------------------------------
    # Close event
    # ------------------------------------------------------------------
    def closeEvent(self, event):  # noqa: N802 (Qt override)
        self._save_geometry()
        if self._controller.has_unsaved_changes():
            reply = QMessageBox.question(
                self,
                "Quit?",
                "Route has unsaved changes. Quit without saving?",
                QMessageBox.Yes | QMessageBox.No,
                QMessageBox.No,
            )
            if reply != QMessageBox.Yes:
                event.ignore()
                return
        # Cleanup EventDetails controller callbacks
        if hasattr(self, 'event_details'):
            self.event_details.cleanup()
        # Unsubscribe from controller callbacks
        for unsub in self._unsubscribers:
            try:
                unsub()
            except Exception:
                pass
        event.accept()

    # ------------------------------------------------------------------
    # Menu bar construction
    # ------------------------------------------------------------------
    def _build_menu_bar(self):  # noqa: C901
        menu_bar = self.menuBar()

        # ---- File menu -----------------------------------------------
        self.file_menu = menu_bar.addMenu("&File")

        self._act_customize_dvs = self.file_menu.addAction("Customize DVs")
        self._act_customize_dvs.setShortcut(QKeySequence("Ctrl+X"))
        self._act_customize_dvs.triggered.connect(self.open_customize_dvs_window)

        self._act_new_route = self.file_menu.addAction("New Route")
        self._act_new_route.setShortcut(QKeySequence("Ctrl+N"))
        self._act_new_route.triggered.connect(self.open_new_route_window)

        self._act_load_route = self.file_menu.addAction("Load Route")
        self._act_load_route.setShortcut(QKeySequence("Ctrl+L"))
        self._act_load_route.triggered.connect(self.open_load_route_window)

        self._act_save_route = self.file_menu.addAction("Save Route")
        self._act_save_route.setShortcut(QKeySequence("Ctrl+S"))
        self._act_save_route.triggered.connect(self.save_route)

        self._act_close_route = self.file_menu.addAction("Close Route")
        self._act_close_route.setShortcut(QKeySequence("Ctrl+Shift+C"))
        self._act_close_route.triggered.connect(self.close_route)

        self.file_menu.addSeparator()

        self._act_auto_load = self.file_menu.addAction("Automatically Load Most Recent Route on Startup")
        self._act_auto_load.setShortcut(QKeySequence("F2"))
        self._act_auto_load.setCheckable(True)
        self._act_auto_load.setChecked(config.get_auto_load_most_recent_route())
        self._act_auto_load.triggered.connect(self.toggle_auto_load_most_recent_route)

        self._act_export_notes = self.file_menu.addAction("Export Notes")
        self._act_export_notes.setShortcut(QKeySequence("Ctrl+Shift+W"))
        self._act_export_notes.triggered.connect(self.export_notes)

        self.file_menu.addSeparator()

        self._act_screenshot_events = self.file_menu.addAction("Screenshot Event List")
        self._act_screenshot_events.setShortcut(QKeySequence("F5"))
        self._act_screenshot_events.triggered.connect(self.screenshot_event_list)

        self._act_screenshot_battle = self.file_menu.addAction("Screenshot Battle Summary")
        self._act_screenshot_battle.setShortcut(QKeySequence("F6"))
        self._act_screenshot_battle.triggered.connect(self.screenshot_battle_summary)

        self._act_screenshot_player = self.file_menu.addAction("Screenshot Player Ranges")
        self._act_screenshot_player.setShortcut(QKeySequence("F7"))
        self._act_screenshot_player.triggered.connect(self.export_player_ranges)

        self._act_screenshot_enemy = self.file_menu.addAction("Screenshot Enemy Ranges")
        self._act_screenshot_enemy.setShortcut(QKeySequence("F8"))
        self._act_screenshot_enemy.triggered.connect(self.export_enemy_ranges)

        self._act_open_image_folder = self.file_menu.addAction("Open Image Folder")
        self._act_open_image_folder.setShortcut(QKeySequence("F12"))
        self._act_open_image_folder.triggered.connect(self.open_image_folder)

        self.file_menu.addSeparator()

        self._act_config_font = self.file_menu.addAction("Config Font")
        self._act_config_font.setShortcut(QKeySequence("Ctrl+Shift+D"))
        self._act_config_font.triggered.connect(self.open_config_window)

        self._act_custom_gens = self.file_menu.addAction("Custom Gens")
        self._act_custom_gens.setShortcut(QKeySequence("Ctrl+Shift+E"))
        self._act_custom_gens.triggered.connect(self.open_custom_gens_window)

        self._act_app_config = self.file_menu.addAction("App Config")
        self._act_app_config.setShortcut(QKeySequence("Ctrl+Shift+A"))
        self._act_app_config.triggered.connect(self.open_app_config_window)

        self._act_open_data_folder = self.file_menu.addAction("Open Data Folder")
        self._act_open_data_folder.setShortcut(QKeySequence("Ctrl+Shift+O"))
        self._act_open_data_folder.triggered.connect(self.open_data_location)

        # ---- Events menu ---------------------------------------------
        self.event_menu = menu_bar.addMenu("&Events")
        self.event_menu.aboutToShow.connect(self._update_event_menu_state)

        self._act_undo = self.event_menu.addAction("Undo")
        self._act_undo.setShortcut(QKeySequence("Ctrl+Z"))
        self._act_undo.triggered.connect(self.undo_event_list)

        self.event_menu.addSeparator()

        self._act_move_up = self.event_menu.addAction("Move Event Up")
        self._act_move_up.setShortcut(QKeySequence("Ctrl+E"))
        self._act_move_up.triggered.connect(self.move_group_up)

        self._act_move_down = self.event_menu.addAction("Move Event Down")
        self._act_move_down.setShortcut(QKeySequence("Ctrl+D"))
        self._act_move_down.triggered.connect(self.move_group_down)

        self._act_move_up_folder = self.event_menu.addAction("Move Event Up To Next Folder")
        self._act_move_up_folder.setShortcut(QKeySequence("Ctrl+Shift+E"))
        self._act_move_up_folder.triggered.connect(self.move_group_to_adjacent_folder_up)

        self._act_move_down_folder = self.event_menu.addAction("Move Event Down To Next Folder")
        self._act_move_down_folder.setShortcut(QKeySequence("Ctrl+Shift+D"))
        self._act_move_down_folder.triggered.connect(self.move_group_to_adjacent_folder_down)

        self.event_menu.addSeparator()

        self._act_enable_disable = self.event_menu.addAction("Enable/Disable")
        self._act_enable_disable.setShortcut(QKeySequence("Ctrl+C"))
        self._act_enable_disable.triggered.connect(self.toggle_enable_disable)

        self._act_toggle_highlight = self.event_menu.addAction("Toggle Highlight")
        self._act_toggle_highlight.setShortcut(QKeySequence("Ctrl+V"))
        self._act_toggle_highlight.triggered.connect(self.toggle_event_highlight)

        self._act_transfer_event = self.event_menu.addAction("Transfer Event")
        self._act_transfer_event.setShortcut(QKeySequence("Ctrl+R"))
        self._act_transfer_event.triggered.connect(self.open_transfer_event_window)

        self._act_delete_event = self.event_menu.addAction("Delete Event")
        self._act_delete_event.setShortcut(QKeySequence("Ctrl+B"))
        self._act_delete_event.triggered.connect(self.delete_group)

        self.event_menu.addSeparator()

        self._act_highlight_branched = self.event_menu.addAction("Highlight Branched Mandatory Battles")
        self._act_highlight_branched.setCheckable(True)
        self._act_highlight_branched.setChecked(config.get_highlight_branched_mandatory())
        self._act_highlight_branched.triggered.connect(self._toggle_highlight_branched_mandatory)

        self._act_fade_folder_text = self.event_menu.addAction("Fade Folder Text")
        self._act_fade_folder_text.setCheckable(True)
        self._act_fade_folder_text.setChecked(config.get_fade_folder_text())
        self._act_fade_folder_text.triggered.connect(self._toggle_fade_folder_text)

        # ---- Highlight menu ------------------------------------------
        self.highlight_menu = menu_bar.addMenu("&Highlight")
        for i in range(1, 10):
            act = self.highlight_menu.addAction(f"Highlight {i}")
            act.setShortcut(QKeySequence(f"Shift+{i}"))
            act.triggered.connect(partial(self.set_event_highlight, i, None))
        self.highlight_menu.addSeparator()
        self._act_config_highlight_colors = self.highlight_menu.addAction("Configure Colors")
        self._act_config_highlight_colors.triggered.connect(self.open_highlight_color_config_window)

        # ---- Folders menu --------------------------------------------
        self.folder_menu = menu_bar.addMenu("F&olders")

        self._act_new_folder = self.folder_menu.addAction("New Folder")
        self._act_new_folder.triggered.connect(self.open_new_folder_window)

        self._act_rename_folder = self.folder_menu.addAction("Rename Cur Folder")
        self._act_rename_folder.setShortcut(QKeySequence("Ctrl+Shift+F"))
        self._act_rename_folder.triggered.connect(self.rename_selected_folder)

        self._act_split_folder = self.folder_menu.addAction("Split Folder")
        self._act_split_folder.setShortcut(QKeySequence("Alt+X"))
        self._act_split_folder.triggered.connect(self.split_folder_at_current_event)

        # ---- Recording menu ------------------------------------------
        self.recording_menu = menu_bar.addMenu("&Recording")

        self._act_toggle_recording = self.recording_menu.addAction("Enable/Disable Recording")
        self._act_toggle_recording.setShortcut(QKeySequence("F1"))
        self._act_toggle_recording.triggered.connect(self.record_button_clicked)

        # ---- Battle Summary menu -------------------------------------
        self.battle_summary_menu = menu_bar.addMenu("&Battle Summary")
        self.battle_summary_menu.aboutToShow.connect(self._update_battle_summary_menu_state)

        # Notes visibility toggle
        self._act_show_notes = self.battle_summary_menu.addAction("Show Notes in Battle Summary")
        self._act_show_notes.setCheckable(True)
        self._act_show_notes.setChecked(config.are_notes_visible_in_battle_summary())
        self._act_show_notes.triggered.connect(self._toggle_battle_summary_notes)

        # Player Highlight Strategy submenu
        self._player_strat_menu = self.battle_summary_menu.addMenu("Player Highlight Strategy")
        self._player_strat_actions = {}
        for strat in const.ALL_HIGHLIGHT_STRATS:
            act = self._player_strat_menu.addAction(strat)
            act.setCheckable(True)
            act.triggered.connect(partial(self._set_player_highlight_strategy, strat))
            self._player_strat_actions[strat] = act
        self._sync_player_strat_checks()

        # Enemy Highlight Strategy submenu
        self._enemy_strat_menu = self.battle_summary_menu.addMenu("Enemy Highlight Strategy")
        self._enemy_strat_actions = {}
        for strat in const.ALL_HIGHLIGHT_STRATS:
            act = self._enemy_strat_menu.addAction(strat)
            act.setCheckable(True)
            act.triggered.connect(partial(self._set_enemy_highlight_strategy, strat))
            self._enemy_strat_actions[strat] = act
        self._sync_enemy_strat_checks()

        self.battle_summary_menu.addSeparator()

        self._act_show_move_highlights = self.battle_summary_menu.addAction("Toggle Move Highlights")
        self._act_show_move_highlights.setShortcut(QKeySequence("Shift+F2"))
        self._act_show_move_highlights.setCheckable(True)
        self._act_show_move_highlights.setChecked(config.get_show_move_highlights())
        self._act_show_move_highlights.triggered.connect(self._toggle_move_highlights)

        self._act_fade_no_highlight = self.battle_summary_menu.addAction("Toggle Fade Moves Without Highlight")
        self._act_fade_no_highlight.setShortcut(QKeySequence("Shift+F3"))
        self._act_fade_no_highlight.setCheckable(True)
        self._act_fade_no_highlight.setChecked(config.get_fade_moves_without_highlight())
        self._act_fade_no_highlight.triggered.connect(self._toggle_fade_moves_without_highlight)

        self._act_test_moves = self.battle_summary_menu.addAction("Toggle Test Moves")
        self._act_test_moves.setShortcut(QKeySequence("Shift+F1"))
        self._act_test_moves.setCheckable(True)
        self._act_test_moves.setChecked(config.get_test_moves_enabled())
        self._act_test_moves.triggered.connect(self._toggle_test_moves)

        self.battle_summary_menu.addSeparator()

        # Consistent threshold config (simple submenu with a spin-box would be heavy;
        # for now expose as a label that opens the config window)
        self._act_consistent_threshold = self.battle_summary_menu.addAction("Configure Consistent Threshold...")
        self._act_consistent_threshold.triggered.connect(self.open_config_window)

        self._act_candy_dec = self.battle_summary_menu.addAction("Decrement Pre-Fight Candies")
        self._act_candy_dec.setShortcut(QKeySequence("F3"))
        self._act_candy_dec.triggered.connect(self.decrement_prefight_candies)

        self._act_candy_inc = self.battle_summary_menu.addAction("Increment Pre-Fight Candies")
        self._act_candy_inc.setShortcut(QKeySequence("F4"))
        self._act_candy_inc.triggered.connect(self.increment_prefight_candies)

        self._act_toggle_player_strat = self.battle_summary_menu.addAction("Toggle Player Highlight Strategy")
        self._act_toggle_player_strat.setShortcut(QKeySequence("F9"))
        self._act_toggle_player_strat.triggered.connect(self.toggle_player_highlight_strategy)

        self._act_toggle_enemy_strat = self.battle_summary_menu.addAction("Toggle Enemy Highlight Strategy")
        self._act_toggle_enemy_strat.setShortcut(QKeySequence("F10"))
        self._act_toggle_enemy_strat.triggered.connect(self.toggle_enemy_highlight_strategy)

    # ------------------------------------------------------------------
    # Central widget construction
    # ------------------------------------------------------------------
    def _build_central_widget(self):
        central = QWidget()
        self.setCentralWidget(central)
        root_layout = QVBoxLayout(central)
        root_layout.setContentsMargins(0, 0, 0, 0)
        root_layout.setSpacing(0)

        self._stacked = QStackedWidget()
        root_layout.addWidget(self._stacked)

        # Page 0: Landing page
        self.landing_page = LandingPage(
            self._stacked,
            self._controller,
            on_create_route=self.open_new_route_window,
            on_load_route=self._load_route_from_landing_page,
            on_auto_load_toggle=self._on_landing_page_auto_load_toggle,
        )
        self._stacked.addWidget(self.landing_page)  # index 0

        # Page 1: New-route page
        self.new_route_page = NewRoutePage(
            self._stacked,
            self._controller,
            on_cancel=self._cancel_new_route_page,
            on_create=self._create_route_from_page,
        )
        self._stacked.addWidget(self.new_route_page)  # index 1

        # Page 2: Route editor (splitter)
        self._route_editor = QWidget()
        editor_layout = QVBoxLayout(self._route_editor)
        editor_layout.setContentsMargins(0, 0, 0, 0)
        editor_layout.setSpacing(0)

        self._splitter = QSplitter(Qt.Horizontal)
        self._splitter.setChildrenCollapsible(False)
        editor_layout.addWidget(self._splitter)

        self._build_left_panel()
        self._build_right_panel()
        self._splitter.setStretchFactor(0, 3)
        self._splitter.setStretchFactor(1, 1)

        self._stacked.addWidget(self._route_editor)  # index 2

        # Notification popup (sits on top of everything)
        self.notification_popup = NotificationPopup(self)

    # ------------------------------------------------------------------
    # Left panel
    # ------------------------------------------------------------------
    def _build_left_panel(self):
        left_panel = QWidget()
        left_panel.setMinimumWidth(200)  # Allow left panel to shrink for battle summary
        left_layout = QVBoxLayout(left_panel)
        left_layout.setContentsMargins(4, 4, 4, 4)
        left_layout.setSpacing(2)

        # ---- Message label -------------------------------------------
        self.message_label = AutoClearingLabel()
        self.message_label.setMinimumWidth(200)
        left_layout.addWidget(self.message_label)

        # ---- Quick-add grid ------------------------------------------
        self._quick_add_container = QWidget()
        qa_grid = QGridLayout(self._quick_add_container)
        qa_grid.setContentsMargins(0, 0, 0, 0)
        qa_grid.setSpacing(4)
        qa_grid.setColumnStretch(0, 2)
        qa_grid.setColumnStretch(1, 2)
        qa_grid.setColumnStretch(2, 1)

        self.trainer_add = QuickTrainerAdd(self._controller, self._quick_add_container)
        qa_grid.addWidget(self.trainer_add, 0, 0)

        self.item_add = QuickItemAdd(self._controller, self._quick_add_container)
        qa_grid.addWidget(self.item_add, 0, 1)

        self.wild_pkmn_add = QuickWildPkmn(self._controller, self._quick_add_container)
        qa_grid.addWidget(self.wild_pkmn_add, 0, 2)

        self.misc_add = QuickMiscEvents(self._controller, self._quick_add_container)
        qa_grid.addWidget(self.misc_add, 1, 2)

        left_layout.addWidget(self._quick_add_container)

        # ---- Filters and control buttons -----------------------------
        self._filters_controls_container = QWidget()
        fc_layout = QVBoxLayout(self._filters_controls_container)
        fc_layout.setContentsMargins(0, 0, 0, 0)
        fc_layout.setSpacing(2)

        # Control buttons row
        btn_grid = QGridLayout()
        btn_grid.setSpacing(2)

        self.show_summary_btn = SimpleButton("Run Summary")
        self.show_summary_btn.clicked.connect(self.open_summary_window)
        btn_grid.addWidget(self.show_summary_btn, 0, 0)

        self.show_setup_summary_btn = SimpleButton("Setup Summary")
        self.show_setup_summary_btn.clicked.connect(self.open_setup_summary_window)
        btn_grid.addWidget(self.show_setup_summary_btn, 1, 0)

        self.move_group_up_button = SimpleButton("Move Event Up")
        self.move_group_up_button.clicked.connect(self.move_group_up)
        btn_grid.addWidget(self.move_group_up_button, 0, 1)

        self.move_group_down_button = SimpleButton("Move Event Down")
        self.move_group_down_button.clicked.connect(self.move_group_down)
        btn_grid.addWidget(self.move_group_down_button, 1, 1)

        self.highlight_toggle_button = SimpleButton("Enable/Disable")
        self.highlight_toggle_button.clicked.connect(self.toggle_enable_disable)
        btn_grid.addWidget(self.highlight_toggle_button, 0, 2)

        self.highlight_toggle_button2 = SimpleButton("Toggle Highlight")
        self.highlight_toggle_button2.clicked.connect(self.toggle_event_highlight)
        btn_grid.addWidget(self.highlight_toggle_button2, 1, 2)

        self.transfer_event_button = SimpleButton("Transfer Event")
        self.transfer_event_button.clicked.connect(self.open_transfer_event_window)
        btn_grid.addWidget(self.transfer_event_button, 0, 3)

        self.delete_event_button = SimpleButton("Delete Event")
        self.delete_event_button.clicked.connect(self.delete_group)
        btn_grid.addWidget(self.delete_event_button, 1, 3)

        self.new_folder_button = SimpleButton("New Folder")
        self.new_folder_button.clicked.connect(self.open_new_folder_window)
        btn_grid.addWidget(self.new_folder_button, 0, 4)

        self.rename_folder_button = SimpleButton("Rename Folder")
        self.rename_folder_button.clicked.connect(self.rename_folder)
        btn_grid.addWidget(self.rename_folder_button, 1, 4)

        fc_layout.addLayout(btn_grid)

        # Route search (filter checkboxes + search)
        self.route_search = RouteSearch(self._controller, self._filters_controls_container)
        fc_layout.addWidget(self.route_search)

        # Place filters/controls spanning columns 0-1 in the quick-add grid
        qa_grid = self._quick_add_container.layout()
        qa_grid.addWidget(self._filters_controls_container, 1, 0, 1, 2)

        left_layout.addWidget(self._quick_add_container)

        # ---- Event list ----------------------------------------------
        event_list_frame = QWidget()
        el_layout = QHBoxLayout(event_list_frame)
        el_layout.setContentsMargins(4, 4, 4, 4)
        el_layout.setSpacing(2)

        self.event_list = RouteList(self._controller, event_list_frame)
        self.event_list.setSelectionMode(QTreeView.ExtendedSelection)
        if hasattr(self.event_list, 'route_list_refreshed'):
            self.event_list.route_list_refreshed.connect(self.update_run_status)
        el_layout.addWidget(self.event_list, 1)

        self.scroll_bar = QScrollBar(Qt.Vertical)
        self.event_list.setVerticalScrollBar(self.scroll_bar)
        el_layout.addWidget(self.scroll_bar)

        left_layout.addWidget(event_list_frame, 1)  # stretch = 1

        self._splitter.addWidget(left_panel)

    # ------------------------------------------------------------------
    # Right panel
    # ------------------------------------------------------------------
    def _build_right_panel(self):
        # EventDetails manages the tabbed panel (Pre-state + Battle Summary),
        # auto-switch checkbox, and the notes footer.
        self.event_details = EventDetails(self._controller)

        # Expose sub-widgets used elsewhere in MainWindow
        self.battle_summary = self.event_details.battle_summary
        self.auto_switch_checkbox = self.event_details.auto_switch_checkbox
        self._tab_widget = self.event_details._tab_widget

        self._splitter.addWidget(self.event_details)

        # Auto-resize splitter when switching between pre-state and battle summary
        self.event_details.battle_summary_visible.connect(self._on_battle_summary_tab_changed)

    def _on_battle_summary_tab_changed(self, is_battle_summary: bool):
        """Widen right panel for battle summary, shrink it back for pre-state."""
        # Defer to ensure the splitter is laid out and has a valid width
        QTimer.singleShot(0, lambda: self._apply_splitter_ratio(is_battle_summary))

    def _apply_splitter_ratio(self, is_battle_summary: bool):
        total = self._splitter.width()
        if total <= 0:
            return

        # Capture the selected item's visual position before the layout changes
        selected_visual_y = None
        selected_index = None
        sel_indexes = self.event_list.selectionModel().selectedIndexes() if self.event_list.selectionModel() else []
        if sel_indexes:
            selected_index = sel_indexes[-1]
            rect = self.event_list.visualRect(selected_index)
            if not rect.isNull():
                selected_visual_y = rect.y()

        # Update stretch factors so future window resizes maintain the ratio
        if is_battle_summary:
            self._splitter.setStretchFactor(0, 1)
            self._splitter.setStretchFactor(1, 2)
            left = int(total * 0.30)
            right = total - left
        else:
            self._splitter.setStretchFactor(0, 3)
            self._splitter.setStretchFactor(1, 1)
            left = int(total * 0.75)
            right = total - left

        # Hide/show the top controls to allow the left panel to shrink
        self._quick_add_container.setVisible(not is_battle_summary)
        self.message_label.setVisible(not is_battle_summary)
        self._splitter.setSizes([left, right])

        # After layout settles, restore the selected item to its original screen position
        if selected_index is not None and selected_visual_y is not None:
            QTimer.singleShot(0, lambda: self._restore_scroll_position(selected_index, selected_visual_y))

    def _restore_scroll_position(self, index, target_visual_y):
        """Adjust scroll so the selected item stays at the same screen Y position."""
        rect = self.event_list.visualRect(index)
        if rect.isNull():
            return
        current_y = rect.y()
        delta = current_y - target_visual_y
        if delta == 0:
            return
        scrollbar = self.event_list.verticalScrollBar()
        if scrollbar:
            scrollbar.setValue(scrollbar.value() + delta)

    # ------------------------------------------------------------------
    # Status bar
    # ------------------------------------------------------------------
    def _build_status_bar(self):
        status_bar = self.statusBar()
        status_bar.setSizeGripEnabled(False)

        self.route_version_label = QLabel("Version")
        self.route_version_label.setStyleSheet(
            "background-color: white; color: black; padding: 3px 8px;"
        )
        status_bar.addWidget(self.route_version_label)

        self.run_status_label = QLabel("Run Status: Valid")
        self.run_status_label.setStyleSheet(
            "background-color: #abebc6; color: black; padding: 3px 8px; border-radius: 3px;"
        )
        status_bar.addWidget(self.run_status_label)

        status_bar.addWidget(QLabel("Route Name:"))
        self.route_name_entry = SimpleEntry(callback=self._user_set_route_name)
        self.route_name_entry.setMinimumWidth(100)
        self.route_name_entry.setMaximumWidth(200)
        status_bar.addWidget(self.route_name_entry)

        status_bar.addWidget(QLabel("Image Path:"))
        self.image_path_entry = SimpleEntry(callback=self._user_set_image_path)
        self.image_path_entry.setMinimumWidth(100)
        self.image_path_entry.setMaximumWidth(200)
        status_bar.addWidget(self.image_path_entry)

        # Client status label (visible only during recording).
        self._sb_client_status = QLabel("")
        self._sb_client_status.setVisible(False)
        status_bar.addPermanentWidget(self._sb_client_status)

        # Reconnect button (visible only during recording).
        self._sb_reconnect_btn = QPushButton("\u27f3")
        self._sb_reconnect_btn.setFixedSize(24, 24)
        self._sb_reconnect_btn.setEnabled(False)
        self._sb_reconnect_btn.setVisible(False)
        status_bar.addPermanentWidget(self._sb_reconnect_btn)

        self.record_button = QPushButton("\u25cf")
        self.record_button.setFixedSize(24, 24)
        self.record_button.setEnabled(False)
        self.record_button.clicked.connect(self.record_button_clicked)
        self._apply_record_button_style(active=False)
        status_bar.addPermanentWidget(self.record_button)

    # ------------------------------------------------------------------
    # Keyboard shortcuts (QShortcut-based, not menu actions)
    # ------------------------------------------------------------------
    def _build_shortcuts(self):
        # Delete key
        QShortcut(QKeySequence(Qt.Key_Delete), self, self.delete_group)

        # Home / End for scroll
        QShortcut(QKeySequence(Qt.Key_Home), self, self.scroll_to_top)
        QShortcut(QKeySequence(Qt.Key_End), self, self.scroll_to_bottom)

        # Grave accent (`) to toggle tabs
        QShortcut(QKeySequence(Qt.Key_QuoteLeft), self, self._toggle_event_tabs)

        # Ctrl+` for summary window
        QShortcut(QKeySequence("Ctrl+`"), self, self.open_summary_window)

        # 1-8 for gym leaders (only act when event list has focus)
        for i in range(1, 9):
            sc = QShortcut(QKeySequence(str(i)), self)
            sc.activated.connect(partial(self.select_gym_leader, i - 1))

        # Ctrl+1-6 for E4/Champion
        for i in range(1, 7):
            sc = QShortcut(QKeySequence(f"Ctrl+{i}"), self)
            sc.activated.connect(partial(self.select_elite_four_or_champion, i - 1))

        # Filter toggles (application-wide)
        def _app_shortcut(key_seq, slot):
            sc = QShortcut(QKeySequence(key_seq), self)
            sc.setContext(Qt.ApplicationShortcut)
            sc.activated.connect(slot)
            return sc

        _app_shortcut("Ctrl+F", self.toggle_fight_trainer_filter)
        _app_shortcut("Ctrl+Y", self.toggle_rare_candy_filter)
        _app_shortcut("Ctrl+T", self.toggle_tm_hm_filter)
        _app_shortcut("Ctrl+G", self.toggle_vitamin_filter)
        _app_shortcut("Ctrl+W", self.toggle_fight_wild_pkmn_filter)
        _app_shortcut("Ctrl+A", self.toggle_common_filters)
        _app_shortcut("Ctrl+Shift+R", self.reset_all_filters)

    # ------------------------------------------------------------------
    # Controller callback registration
    # ------------------------------------------------------------------
    def _register_controller_callbacks(self):
        unsub = self._controller.register_event_preview(self.trainer_preview)
        self._unsubscribers.append(unsub)

        unsub = self._controller.register_event_selection(self._handle_new_selection)
        self._unsubscribers.append(unsub)

        unsub = self._controller.register_version_change(self.update_run_version)
        self._unsubscribers.append(unsub)

        unsub = self._controller.register_exception_callback(self._on_exception)
        self._unsubscribers.append(unsub)

        unsub = self._controller.register_name_change(self._on_name_change)
        self._unsubscribers.append(unsub)

        unsub = self._controller.register_record_mode_change(self._on_record_mode_changed)
        self._unsubscribers.append(unsub)

        unsub = self._controller.register_message_callback(self._on_route_message)
        self._unsubscribers.append(unsub)

        unsub = self._controller.register_route_change(self._on_route_change)
        self._unsubscribers.append(unsub)

    # ------------------------------------------------------------------
    # Page switching helpers
    # ------------------------------------------------------------------
    def _show_landing_page(self):
        self._stacked.setCurrentIndex(0)
        self.landing_page.refresh_routes()

    def _show_new_route_page(self):
        self._stacked.setCurrentIndex(1)
        self.new_route_page.refresh_game_list()

    def _show_route_controls(self):
        self._stacked.setCurrentIndex(2)

    # ------------------------------------------------------------------
    # Controller callback handlers
    # ------------------------------------------------------------------
    def _on_exception(self):
        msg = self._controller.get_next_exception_info()
        while msg is not None:
            QMessageBox.critical(self, "Error!", msg)
            msg = self._controller.get_next_exception_info()

    def _on_name_change(self):
        if self.route_name_entry.get() == self._controller.get_current_route_name():
            return
        self._loading_route_name = True
        self.route_name_entry.set(self._controller.get_current_route_name())
        self._loading_route_name = False

    def _on_route_message(self):
        message = self._controller.get_next_message_info()
        if message is None:
            return
        if message.startswith("Successfully saved route:") or message.startswith("Saved screenshot to:"):
            folder_path = None
            if message.startswith("Successfully saved route:"):
                short_message = message.replace("Successfully saved route: ", "Route saved: ")
            else:
                full_path = message.replace("Saved screenshot to: ", "")
                folder_path = os.path.dirname(os.path.normpath(full_path))
                if not folder_path or not os.path.isdir(folder_path):
                    folder_path = None
                short_message = "Screenshot saved"
            self.notification_popup.show_notification(short_message, duration=5000, folder_path=folder_path)
        else:
            self.message_label.set_message(message)

    def _on_route_change(self):
        self.event_list.refresh()
        if self._controller.get_version() is not None:
            self._show_route_controls()
        else:
            self._show_landing_page()

    def _apply_record_button_style(self, active=False):
        if active:
            self.record_button.setStyleSheet(
                "QPushButton { color: #e74c3c; font-size: 14px; }"
                "QPushButton:hover { color: #ff6b5b; }"
            )
        else:
            self.record_button.setStyleSheet(
                "QPushButton { color: #888; font-size: 14px; }"
                "QPushButton:hover { color: #aaa; }"
                "QPushButton:disabled { color: #555; }"
            )

    def _on_record_mode_changed(self):
        if self._controller.is_record_mode_active():
            self._apply_record_button_style(active=True)
            self._sb_client_status.setVisible(True)
            self._sb_reconnect_btn.setVisible(True)
            self._quick_add_container.setVisible(False)
        else:
            self._apply_record_button_style(active=False)
            self._sb_client_status.setVisible(False)
            self._sb_reconnect_btn.setVisible(False)
            self._quick_add_container.setVisible(True)
        self._handle_new_selection()

    def trainer_preview(self):
        if self._controller.get_preview_event() is None:
            return
        all_ids = self.event_list.get_all_selected_event_ids()
        if len(all_ids) > 1:
            return
        init_state = self._controller.get_state_after(
            previous_event_id=None if len(all_ids) == 0 else all_ids[0]
        )
        # Placeholder: when real EventDetails is ported this will call show_event_details
        # self.event_details.show_event_details(
        #     self._controller.get_preview_event(), init_state, init_state, allow_updates=False
        # )

    def _handle_new_selection(self):
        all_event_ids = self._controller.get_all_selected_ids()
        if all_event_ids != self.event_list.get_all_selected_event_ids():
            self.event_list.set_all_selected_event_ids(all_event_ids)
        self.event_list.scroll_to_selected_events()

        all_event_ids = self.event_list.get_all_selected_event_ids(allow_event_items=False)
        if len(all_event_ids) > 1 or len(all_event_ids) == 0:
            event_group = None
        else:
            event_group = self._controller.get_event_by_id(all_event_ids[0])

        disable_all = (
            self._controller.is_record_mode_active()
            or self._controller.get_raw_route().init_route_state is None
        )

        if not disable_all and isinstance(event_group, EventFolder):
            self.rename_folder_button.enable()
        else:
            self.rename_folder_button.disable()

        if not disable_all and (event_group is not None or len(all_event_ids) > 0):
            self.delete_event_button.enable()
            self.transfer_event_button.enable()
            self.move_group_down_button.enable()
            self.move_group_up_button.enable()
            self.highlight_toggle_button.enable()
        else:
            self.delete_event_button.disable()
            self.transfer_event_button.disable()
            self.move_group_down_button.disable()
            self.move_group_up_button.disable()
            self.highlight_toggle_button.disable()

        if not disable_all and (
            event_group is not None or len(self.event_list.get_all_selected_event_ids()) == 0
        ):
            self.new_folder_button.enable()
        else:
            self.new_folder_button.disable()

    def update_run_status(self):
        if self._controller.has_errors():
            self.run_status_label.setText("Run Status: Invalid")
            self.run_status_label.setStyleSheet(
                "background-color: #f9e79f; color: black; padding: 6px 10px; border-radius: 3px;"
            )
        else:
            self.run_status_label.setText("Run Status: Valid")
            self.run_status_label.setStyleSheet(
                "background-color: #abebc6; color: black; padding: 6px 10px; border-radius: 3px;"
            )

    def update_run_version(self):
        version = self._controller.get_version()
        color = const.VERSION_COLORS.get(version, "white")
        self.route_version_label.setText(f"{version} Version")
        self.route_version_label.setStyleSheet(
            f"background-color: {color}; color: black; padding: 6px 10px;"
        )
        self.record_button.setEnabled(True)

    # ------------------------------------------------------------------
    # Route management
    # ------------------------------------------------------------------
    def save_route(self):
        route_name = self.route_name_entry.get()
        self._controller.save_route(route_name)

    def export_notes(self):
        self._controller.export_notes(self.route_name_entry.get())

    def screenshot_event_list(self):
        # Qt screenshot approach -- grab the event list widget
        pixmap = self.event_list.grab()
        if pixmap.isNull():
            return
        path_value = self.image_path_entry.get().strip()
        custom_path = path_value.strip('"').strip("'") if path_value else None
        # Save via controller for consistent path handling
        from datetime import datetime
        date_prefix = datetime.now().strftime("%Y%m%d%H%M%S")
        save_dir = custom_path if custom_path and os.path.isdir(custom_path) else config.get_images_dir()
        out_path = io_utils.get_safe_path_no_collision(
            save_dir,
            f"{date_prefix}-{self._controller.get_current_route_name()}_event_list",
            ext=".png",
        )
        os.makedirs(os.path.dirname(out_path), exist_ok=True)
        pixmap.save(out_path)
        self._controller.send_message(f"Saved screenshot to: {out_path}")

    def screenshot_battle_summary(self):
        self.event_details.take_battle_summary_screenshot()

    def export_player_ranges(self):
        self.event_details.take_player_ranges_screenshot()

    def export_enemy_ranges(self):
        self.event_details.take_enemy_ranges_screenshot()

    def increment_prefight_candies(self):
        self.event_details._increment_prefight_candies()

    def decrement_prefight_candies(self):
        self.event_details._decrement_prefight_candies()

    def open_image_folder(self):
        io_utils.open_explorer(config.get_images_dir())

    def open_data_location(self):
        io_utils.open_explorer(config.get_user_data_dir())

    def open_config_window(self):
        from gui_qt.dialogs import ColorConfigDialog
        dlg = ColorConfigDialog(self)
        dlg.exec()
        # Refresh stylesheet after color changes
        from gui_qt.theme import generate_stylesheet
        QApplication.instance().setStyleSheet(generate_stylesheet())

    def open_custom_gens_window(self):
        from gui_qt.dialogs import CustomGenDialog
        dlg = CustomGenDialog(self, self._controller)
        dlg.exec()

    def open_app_config_window(self):
        from gui_qt.dialogs import DataDirConfigDialog
        dlg = DataDirConfigDialog(self, restart_callback=self.cancel_and_quit)
        dlg.exec()

    def open_highlight_color_config_window(self):
        from gui_qt.dialogs import HighlightColorConfigDialog
        dlg = HighlightColorConfigDialog(self)
        dlg.exec()
        self.event_list.refresh()

    def open_new_route_window(self):
        self._route_loaded_before_new_route = self._controller.get_version() is not None
        self._show_new_route_page()

    def open_load_route_window(self):
        from gui_qt.dialogs import LoadRouteDialog
        dlg = LoadRouteDialog(self, self._controller)
        if dlg.exec():
            route_path = dlg.get_selected_route_path()
            if route_path:
                self._controller.load_route(route_path)
                self._show_route_controls()

    def open_customize_dvs_window(self):
        if self._controller.is_empty():
            return
        from gui_qt.dialogs import CustomDvsDialog
        from pkmn.gen_factory import current_gen_info
        init_state = self._controller.get_init_state()
        if init_state is None:
            return
        solo = init_state.solo_pkmn
        dlg = CustomDvsDialog(
            self,
            self._controller,
            solo.dvs,
            solo.ability_idx if hasattr(solo, 'ability_idx') else 0,
            solo.nature if hasattr(solo, 'nature') else None,
        )
        dlg.exec()

    def close_route(self):
        if self._controller.get_version() is None:
            return
        if self._controller.has_unsaved_changes():
            reply = QMessageBox.question(
                self,
                "Unsaved Changes",
                "Route has unsaved changes. Save before closing?",
                QMessageBox.Yes | QMessageBox.No | QMessageBox.Cancel,
                QMessageBox.Cancel,
            )
            if reply == QMessageBox.Cancel:
                return
            if reply == QMessageBox.Yes:
                route_name = self.route_name_entry.get()
                if not route_name:
                    QMessageBox.warning(self, "No Route Name", "Please enter a route name before saving.")
                    return
                self._controller.save_route(route_name)

        # Clear the route
        self._controller._data.init_route_state = None
        self._controller._data.pkmn_version = None
        self._controller._data._reset_events()
        self._controller._data.level_up_move_defs = {}
        self._controller._data.defeated_trainers = set()
        self._controller._route_name = ""
        self._controller._selected_ids = []
        self._controller._unsaved_changes = False
        self._controller._on_name_change()
        self._controller._on_version_change()
        self._controller._on_event_selection()
        self._controller._on_route_change()

    def _cancel_new_route_page(self):
        if self._route_loaded_before_new_route:
            self._show_route_controls()
        else:
            self._show_landing_page()
        self._route_loaded_before_new_route = False

    def _create_route_from_page(self, solo_mon, base_route_path, pkmn_version,
                                custom_dvs=None, custom_ability_idx=None, custom_nature=None):
        self._controller.create_new_route(
            solo_mon, base_route_path, pkmn_version,
            custom_dvs=custom_dvs, custom_ability_idx=custom_ability_idx,
            custom_nature=custom_nature,
        )

    def _load_route_from_landing_page(self, route_path):
        self._controller.load_route(route_path)
        self._show_route_controls()

    def _find_most_recent_route(self):
        all_routes = io_utils.get_existing_route_names(load_backups=False)
        if not all_routes:
            return None
        most_recent = None
        most_recent_mtime = 0
        for route_name in all_routes:
            route_path = io_utils.get_existing_route_path(route_name)
            try:
                mtime = os.path.getmtime(route_path)
                if mtime > most_recent_mtime:
                    most_recent_mtime = mtime
                    most_recent = route_path
            except Exception:
                continue
        return most_recent

    def _load_route_immediately(self, route_path):
        if self._auto_load_checked:
            return
        self._auto_load_checked = True
        self._controller.load_route(route_path)
        self._show_route_controls()

    # ------------------------------------------------------------------
    # Event actions
    # ------------------------------------------------------------------
    def move_group_up(self):
        self._controller.move_groups_up(
            self.event_list.get_all_selected_event_ids(allow_event_items=False)
        )

    def move_group_down(self):
        ids = self.event_list.get_all_selected_event_ids(allow_event_items=False)
        self._controller.move_groups_down(reversed(ids))

    def move_group_to_adjacent_folder_up(self):
        self._controller.move_groups_to_adjacent_folder_up(
            self.event_list.get_all_selected_event_ids(allow_event_items=False)
        )

    def move_group_to_adjacent_folder_down(self):
        ids = self.event_list.get_all_selected_event_ids(allow_event_items=False)
        self._controller.move_groups_to_adjacent_folder_down(reversed(ids))

    def toggle_event_highlight(self):
        self._controller.toggle_event_highlight(
            self.event_list.get_all_selected_event_ids(allow_event_items=False)
        )

    def set_event_highlight(self, highlight_num, _event=None):
        selected = self.event_list.get_all_selected_event_ids(allow_event_items=False)
        if not selected:
            return
        all_have = True
        for eid in selected:
            eobj = self._controller.get_event_by_id(eid)
            if eobj and hasattr(eobj, "event_definition"):
                if eobj.event_definition.get_highlight_type() != highlight_num:
                    all_have = False
                    break
        if all_have:
            highlight_num = None
        self._controller.set_event_highlight(selected, highlight_num)

    def toggle_enable_disable(self):
        self.event_list.trigger_checkbox()

    def delete_group(self):
        all_ids = self.event_list.get_all_selected_event_ids(allow_event_items=False)
        if not all_ids:
            return
        do_prompt = False
        if len(all_ids) == 1:
            eobj = self._controller.get_event_by_id(all_ids[0])
            if isinstance(eobj, EventFolder) and len(eobj.children) > 0:
                do_prompt = True
        else:
            do_prompt = True

        if do_prompt:
            reply = QMessageBox.question(
                self,
                "Confirm Delete",
                f"Delete {len(all_ids)} event(s)?",
                QMessageBox.Yes | QMessageBox.No,
                QMessageBox.No,
            )
            if reply != QMessageBox.Yes:
                return
            self._controller.delete_events(all_ids)
        else:
            self._controller.delete_events([all_ids[0]])
        self.event_list.refresh()
        self.trainer_add.trainer_filter_callback()

    def open_transfer_event_window(self):
        all_ids = self.event_list.get_all_selected_event_ids(allow_event_items=False)
        if not all_ids:
            return
        from gui_qt.dialogs import TransferEventDialog
        raw_route = self._controller.get_raw_route()
        all_folders = [f.name for f in raw_route.get_all_folders()]
        valid_dest = [f.name for f in raw_route.get_valid_transfer_destinations(all_ids)]
        dlg = TransferEventDialog(self, self._controller, all_folders, valid_dest, all_ids)
        dlg.exec()

    def split_folder_at_current_event(self):
        eid = self._controller.get_single_selected_event_id(allow_event_items=False)
        if eid is None:
            return
        self._controller.split_folder_at_current_event(eid)

    def open_new_folder_window(self, existing_folder_name=None):
        all_ids = self.event_list.get_all_selected_event_ids()
        if len(all_ids) > 1:
            return
        from gui_qt.dialogs import NewFolderDialog
        raw_route = self._controller.get_raw_route()
        cur_folder_names = [f.name for f in raw_route.get_all_folders()]
        insert_after = all_ids[0] if all_ids else None
        dlg = NewFolderDialog(self, self._controller, cur_folder_names, existing_folder_name, insert_after=insert_after)
        dlg.exec()

    def rename_folder(self):
        all_ids = self.event_list.get_all_selected_event_ids()
        if len(all_ids) != 1:
            return
        eobj = self._controller.get_event_by_id(all_ids[0])
        if eobj is not None and isinstance(eobj, EventFolder):
            self.open_new_folder_window(existing_folder_name=eobj.name)

    def rename_selected_folder(self):
        all_ids = self.event_list.get_all_selected_event_ids()
        if len(all_ids) != 1:
            return
        eobj = self._controller.get_event_by_id(all_ids[0])
        if eobj is None:
            return
        if isinstance(eobj, EventFolder):
            folder = eobj
        elif hasattr(eobj, "parent") and eobj.parent is not None:
            folder = eobj.parent
        else:
            return
        if folder.name == const.ROOT_FOLDER_NAME:
            return
        self.open_new_folder_window(existing_folder_name=folder.name)

    # ------------------------------------------------------------------
    # Scroll helpers
    # ------------------------------------------------------------------
    def scroll_to_top(self):
        self.event_list.scroll_to_top()

    def scroll_to_bottom(self):
        self.event_list.scroll_to_bottom()

    # ------------------------------------------------------------------
    # Summary windows
    # ------------------------------------------------------------------
    def open_summary_window(self):
        if self.summary_window is not None:
            try:
                # If the window still exists, just bring it to front
                self.summary_window.raise_()
                self.summary_window.activateWindow()
                return
            except RuntimeError:
                # Underlying C++ object has been deleted
                self.summary_window = None

        from gui_qt.secondary_windows import RouteSummaryWindow
        self.summary_window = RouteSummaryWindow(self, self._controller)
        self.summary_window.destroyed.connect(self._on_summary_window_destroyed)
        self.summary_window.show()

    def _on_summary_window_destroyed(self):
        self.summary_window = None

    def open_setup_summary_window(self):
        if self.setup_summary_window is not None:
            try:
                self.setup_summary_window.raise_()
                self.setup_summary_window.activateWindow()
                return
            except RuntimeError:
                self.setup_summary_window = None

        from gui_qt.secondary_windows import SetupSummaryWindow
        self.setup_summary_window = SetupSummaryWindow(self, self._controller)
        self.setup_summary_window.destroyed.connect(self._on_setup_summary_window_destroyed)
        self.setup_summary_window.show()

    def _on_setup_summary_window_destroyed(self):
        self.setup_summary_window = None

    # ------------------------------------------------------------------
    # Gym leader & E4 shortcuts
    # ------------------------------------------------------------------
    def select_gym_leader(self, gym_idx):
        if self._text_field_has_focus:
            return
        if self._controller.get_version() is None:
            return
        try:
            from pkmn.gen_factory import current_gen_info
            names = current_gen_info().get_gym_leader_names()
            if gym_idx >= len(names):
                return
            eid = self._controller.find_first_event_by_trainer_name(names[gym_idx])
            if eid is not None:
                self._controller.select_new_events([eid])
        except Exception as e:
            logger.error(f"Error selecting gym leader: {e}")

    def select_elite_four_or_champion(self, e4_idx):
        if self._controller.get_version() is None:
            return
        try:
            from pkmn.gen_factory import current_gen_info
            names = current_gen_info().get_elite_four_and_champion_names()
            if e4_idx >= len(names):
                return
            entry = names[e4_idx]
            eid = None
            if isinstance(entry, list):
                for variant in entry:
                    eid = self._controller.find_first_event_by_trainer_name(variant)
                    if eid is not None:
                        break
            else:
                eid = self._controller.find_first_event_by_trainer_name(entry)
            if eid is not None:
                self._controller.select_new_events([eid])
        except Exception as e:
            logger.error(f"Error selecting E4/Champion: {e}")

    # ------------------------------------------------------------------
    # Recording
    # ------------------------------------------------------------------
    def record_button_clicked(self):
        self._controller.set_record_mode(not self._controller.is_record_mode_active())

    # ------------------------------------------------------------------
    # Filter toggles
    # ------------------------------------------------------------------
    def toggle_fight_trainer_filter(self):
        if self._text_field_has_focus:
            return
        try:
            self.route_search.toggle_filter_by_type(const.TASK_TRAINER_BATTLE)
        except Exception as e:
            logger.error(f"Error toggling Fight Trainer filter: {e}")

    def toggle_rare_candy_filter(self):
        if self._text_field_has_focus:
            return
        try:
            self.route_search.toggle_filter_by_type(const.TASK_RARE_CANDY)
        except Exception as e:
            logger.error(f"Error toggling Rare Candy filter: {e}")

    def toggle_tm_hm_filter(self):
        if self._text_field_has_focus:
            return
        try:
            self.route_search.toggle_filter_by_type(const.TASK_LEARN_MOVE_TM)
        except Exception as e:
            logger.error(f"Error toggling TM/HM filter: {e}")

    def toggle_vitamin_filter(self):
        if self._text_field_has_focus:
            return
        try:
            self.route_search.toggle_filter_by_type(const.TASK_VITAMIN)
        except Exception as e:
            logger.error(f"Error toggling Vitamin filter: {e}")

    def toggle_fight_wild_pkmn_filter(self):
        if self._text_field_has_focus:
            return
        try:
            self.route_search.toggle_filter_by_type(const.TASK_FIGHT_WILD_PKMN)
        except Exception as e:
            logger.error(f"Error toggling Wild Pkmn filter: {e}")

    def toggle_common_filters(self):
        if self._text_field_has_focus:
            return
        try:
            trainer_on = self.route_search.is_filter_checked(const.TASK_TRAINER_BATTLE)
            candy_on = self.route_search.is_filter_checked(const.TASK_RARE_CANDY)
            vitamin_on = self.route_search.is_filter_checked(const.TASK_VITAMIN)
            new_state = not (trainer_on and candy_on and vitamin_on)
            self.route_search.set_filter_by_type(const.TASK_TRAINER_BATTLE, new_state)
            self.route_search.set_filter_by_type(const.TASK_RARE_CANDY, new_state)
            self.route_search.set_filter_by_type(const.TASK_VITAMIN, new_state)
        except Exception as e:
            logger.error(f"Error toggling common filters: {e}")

    def reset_all_filters(self):
        if self._text_field_has_focus:
            return
        try:
            self.route_search.reset_all_filters()
        except Exception as e:
            logger.error(f"Error resetting filters: {e}")

    # ------------------------------------------------------------------
    # Auto-load toggle
    # ------------------------------------------------------------------
    def toggle_auto_load_most_recent_route(self):
        new_val = not config.get_auto_load_most_recent_route()
        config.set_auto_load_most_recent_route(new_val)
        self._act_auto_load.setChecked(new_val)
        if hasattr(self.landing_page, "auto_load_checkbox"):
            try:
                self.landing_page.auto_load_checkbox.setChecked(new_val)
            except Exception:
                pass

    def _on_landing_page_auto_load_toggle(self):
        self._act_auto_load.setChecked(config.get_auto_load_most_recent_route())

    # ------------------------------------------------------------------
    # Undo
    # ------------------------------------------------------------------
    def undo_event_list(self):
        if self._text_field_has_focus:
            return
        if self._controller.can_undo():
            self._controller.undo()

    # ------------------------------------------------------------------
    # Battle Summary menu helpers
    # ------------------------------------------------------------------
    def _toggle_battle_summary_notes(self):
        new_val = self._act_show_notes.isChecked()
        config.set_notes_visibility_in_battle_summary(new_val)
        self.event_details._update_notes_visibility_in_battle_summary()

    def _sync_player_strat_checks(self):
        cur = config.get_player_highlight_strategy()
        for strat, act in self._player_strat_actions.items():
            act.setChecked(strat == cur)

    def _set_player_highlight_strategy(self, strat):
        config.set_player_highlight_strategy(strat)
        self._sync_player_strat_checks()

    def _sync_enemy_strat_checks(self):
        cur = config.get_enemy_highlight_strategy()
        for strat, act in self._enemy_strat_actions.items():
            act.setChecked(strat == cur)

    def _set_enemy_highlight_strategy(self, strat):
        config.set_enemy_highlight_strategy(strat)
        self._sync_enemy_strat_checks()

    def _toggle_move_highlights(self):
        new_val = not config.get_show_move_highlights()
        config.set_show_move_highlights(new_val)
        self._act_show_move_highlights.setChecked(new_val)
        self._act_fade_no_highlight.setEnabled(new_val)

    def _toggle_fade_moves_without_highlight(self):
        if not config.get_show_move_highlights():
            self._act_fade_no_highlight.setChecked(config.get_fade_moves_without_highlight())
            return
        new_val = not config.get_fade_moves_without_highlight()
        config.set_fade_moves_without_highlight(new_val)
        self._act_fade_no_highlight.setChecked(new_val)

    def _toggle_test_moves(self):
        new_val = not config.get_test_moves_enabled()
        config.set_test_moves_enabled(new_val)
        self._act_test_moves.setChecked(new_val)

    def _toggle_highlight_branched_mandatory(self):
        new_val = self._act_highlight_branched.isChecked()
        config.set_highlight_branched_mandatory(new_val)
        self.event_list.refresh()

    def _toggle_fade_folder_text(self):
        new_val = self._act_fade_folder_text.isChecked()
        config.set_fade_folder_text(new_val)
        self.event_list.update_folder_text_style()
        self.event_list.refresh()

    def toggle_player_highlight_strategy(self):
        cur = config.get_player_highlight_strategy()
        new = const.HIGHLIGHT_NONE if cur == const.HIGHLIGHT_GUARANTEED_KILL else const.HIGHLIGHT_GUARANTEED_KILL
        config.set_player_highlight_strategy(new)
        self._sync_player_strat_checks()

    def toggle_enemy_highlight_strategy(self):
        cur = config.get_enemy_highlight_strategy()
        new = const.HIGHLIGHT_NONE if cur == const.HIGHLIGHT_GUARANTEED_KILL else const.HIGHLIGHT_GUARANTEED_KILL
        config.set_enemy_highlight_strategy(new)
        self._sync_enemy_strat_checks()

    def _update_battle_summary_menu_state(self):
        self._act_show_move_highlights.setChecked(config.get_show_move_highlights())
        self._act_fade_no_highlight.setChecked(config.get_fade_moves_without_highlight())
        self._act_fade_no_highlight.setEnabled(config.get_show_move_highlights())
        self._act_test_moves.setChecked(config.get_test_moves_enabled())
        self._sync_player_strat_checks()
        self._sync_enemy_strat_checks()
        self._act_show_notes.setChecked(config.are_notes_visible_in_battle_summary())

    def _update_event_menu_state(self):
        self._act_undo.setEnabled(self._controller.can_undo())

        # Split folder availability
        eid = self._controller.get_single_selected_event_id(allow_event_items=False)
        can_split = False
        if eid is not None:
            eobj = self._controller.get_event_by_id(eid)
            if (
                eobj is not None
                and not isinstance(eobj, EventFolder)
                and hasattr(eobj, "parent")
                and eobj.parent is not None
                and eobj.parent.name != const.ROOT_FOLDER_NAME
            ):
                can_split = True
        self._act_split_folder.setEnabled(can_split)

        self._act_highlight_branched.setChecked(config.get_highlight_branched_mandatory())
        # Enable branched-mandatory only if the gen supports it
        try:
            from pkmn.gen_factory import current_gen_info
            has_branched = current_gen_info().has_branched_mandatory_fights()
            self._act_highlight_branched.setEnabled(has_branched)
        except Exception:
            self._act_highlight_branched.setEnabled(False)

    # ------------------------------------------------------------------
    # Tab toggle
    # ------------------------------------------------------------------
    def _toggle_event_tabs(self):
        self.event_details.change_tabs()

    # ------------------------------------------------------------------
    # Text-field helpers
    # ------------------------------------------------------------------
    def _user_set_route_name(self):
        if not self._loading_route_name:
            self._controller.set_current_route_name(self.route_name_entry.get())

    def _user_set_image_path(self):
        self._controller.set_custom_image_path(self.image_path_entry.get())

    def register_text_field_focus(self, widget=None):
        self._text_field_has_focus = True

    def unregister_text_field_focus(self):
        self._text_field_has_focus = False

    def is_text_field_focused(self):
        return self._text_field_has_focus

    # ------------------------------------------------------------------
    # Deferred post-init (background gen loading + auto-load route)
    # ------------------------------------------------------------------
    def _deferred_post_init(self):
        """Load custom gens, then auto-load route."""
        try:
            self._controller.load_all_custom_versions()
        except Exception as e:
            logger.warning(f"Some custom gens could not be loaded: {e}")
        self._post_init_ui_work()

    def _post_init_ui_work(self):
        """Called on the main thread after background gen loading completes."""
        if config.get_auto_load_most_recent_route():
            most_recent = self._find_most_recent_route()
            if most_recent:
                self._load_route_immediately(most_recent)
                return
        # If no auto-load, refresh the landing page to show updated game list
        self.landing_page.refresh_routes()

    # ------------------------------------------------------------------
    # Quit
    # ------------------------------------------------------------------
    def cancel_and_quit(self):
        self.close()
