import os
import logging
from typing import Callable, Optional, Tuple
from datetime import datetime

from PySide6.QtWidgets import (
    QWidget, QLabel, QPushButton, QVBoxLayout, QHBoxLayout, QTreeWidget,
    QTreeWidgetItem, QCheckBox, QButtonGroup, QComboBox,
    QLineEdit, QHeaderView, QAbstractItemView, QSizePolicy,
)
from PySide6.QtCore import Qt, QTimer
from PySide6.QtGui import QColor, QBrush

from utils.constants import const
from utils.config_manager import config
from utils import io_utils

try:
    from pkmn.gen_factory import _gen_factory as gen_factory
except ImportError:
    gen_factory = None

logger = logging.getLogger(__name__)


class LandingPage(QWidget):
    """Landing page shown when no route is loaded.

    Displays a title, buttons to create or load a route, an auto-load
    checkbox, and a searchable/sortable list of saved routes.
    """

    SORT_MOST_RECENT = "most_recent"
    SORT_GAME = "game"
    SORT_ALPHABETICAL = "alphabetical"

    # ------------------------------------------------------------------
    # Construction
    # ------------------------------------------------------------------
    def __init__(
        self,
        parent: QWidget,
        controller,
        on_create_route: Callable,
        on_load_route: Callable,
        on_auto_load_toggle: Optional[Callable] = None,
    ):
        super().__init__(parent)
        self._controller = controller
        self._on_create_route = on_create_route
        self._on_load_route = on_load_route
        self._on_auto_load_toggle = on_auto_load_toggle

        # Persisted preferences
        self._current_sort: str = config.get_landing_page_sort()
        self._selected_game_filter: str = config.get_landing_page_game_filter()
        saved_search = config.get_landing_page_search_filter()
        self._search_text: str = saved_search.strip().lower() if saved_search else ""

        # Metadata cache: route_name -> (game_version, species_name, mtime)
        self._route_metadata_cache: dict = {}

        # Debounce timer for search
        self._search_timer = QTimer(self)
        self._search_timer.setSingleShot(True)
        self._search_timer.setInterval(300)
        self._search_timer.timeout.connect(self._apply_search)

        self._build_ui(saved_search)

    # ------------------------------------------------------------------
    # UI construction
    # ------------------------------------------------------------------
    def _build_ui(self, saved_search: str):
        outer = QVBoxLayout(self)
        outer.setAlignment(Qt.AlignHCenter)

        # ---- Title ---------------------------------------------------
        title = QLabel("Pokemon XP Router")
        title.setProperty("class", "title")
        title.setAlignment(Qt.AlignCenter)
        title_font = title.font()
        title_font.setPointSize(24)
        title_font.setBold(True)
        title.setFont(title_font)
        outer.addSpacing(50)
        outer.addWidget(title, 0, Qt.AlignCenter)
        outer.addSpacing(20)

        # ---- Create New Route button ---------------------------------
        self.create_button = QPushButton("Create New Route")
        self.create_button.setProperty("class", "large")
        btn_font = self.create_button.font()
        btn_font.setPointSize(14)
        btn_font.setBold(True)
        self.create_button.setFont(btn_font)
        self.create_button.setFixedWidth(350)
        self.create_button.setMinimumHeight(50)
        self.create_button.clicked.connect(self._handle_create_route)
        outer.addWidget(self.create_button, 0, Qt.AlignCenter)
        outer.addSpacing(10)

        # ---- Load Selected Route button ------------------------------
        self.load_button = QPushButton("Load Selected Route")
        self.load_button.setProperty("class", "large")
        self.load_button.setFont(btn_font)
        self.load_button.setFixedWidth(350)
        self.load_button.setMinimumHeight(50)
        self.load_button.setEnabled(False)
        self.load_button.clicked.connect(self._handle_load_selected_route)
        outer.addWidget(self.load_button, 0, Qt.AlignCenter)
        outer.addSpacing(10)

        # ---- Auto-load checkbox --------------------------------------
        self.auto_load_checkbox = QCheckBox("Automatically Load Most Recent Route on Startup")
        self.auto_load_checkbox.setChecked(config.get_auto_load_most_recent_route())
        self.auto_load_checkbox.stateChanged.connect(self._on_auto_load_toggle_changed)
        outer.addWidget(self.auto_load_checkbox, 0, Qt.AlignCenter)
        outer.addSpacing(10)

        # ---- Routes section ------------------------------------------
        routes_container = QWidget()
        routes_container.setFixedWidth(600)
        routes_layout = QVBoxLayout(routes_container)
        routes_layout.setContentsMargins(0, 0, 0, 0)
        routes_layout.setSpacing(4)

        routes_title = QLabel("Routes")
        routes_title.setStyleSheet("font-size: 18pt; font-weight: bold;")
        routes_title.setAlignment(Qt.AlignCenter)
        routes_layout.addWidget(routes_title)

        # Sort controls — segmented toggle
        sort_row = QHBoxLayout()
        sort_row.setSpacing(0)

        self._sort_group = QButtonGroup(self)
        self._sort_group.setExclusive(True)

        self._btn_recent = QPushButton("Most Recent")
        self._btn_alpha = QPushButton("Alphabetical")
        self._btn_game = QPushButton("Game")

        for btn in (self._btn_recent, self._btn_alpha, self._btn_game):
            btn.setCheckable(True)
            btn.setProperty("class", "seg-toggle")
            self._sort_group.addButton(btn)
            sort_row.addWidget(btn)

        # Set initial checked state
        if self._current_sort == self.SORT_ALPHABETICAL:
            self._btn_alpha.setChecked(True)
        elif self._current_sort == self.SORT_GAME:
            self._btn_game.setChecked(True)
        else:
            self._btn_recent.setChecked(True)

        # Game filter dropdown
        self.game_filter_dropdown = QComboBox()
        self.game_filter_dropdown.setMinimumWidth(160)
        self._populate_game_filter_dropdown()
        self.game_filter_dropdown.setVisible(self._current_sort == self.SORT_GAME)
        self.game_filter_dropdown.currentTextChanged.connect(self._on_game_filter_changed)
        sort_row.addSpacing(8)
        sort_row.addWidget(self.game_filter_dropdown)

        sort_row.addStretch()
        routes_layout.addLayout(sort_row)

        # Connect sort signals after initial state is set
        self._sort_group.buttonClicked.connect(self._on_sort_changed)

        # Search bar
        search_row = QHBoxLayout()
        search_row.setSpacing(6)

        search_label = QLabel("Search:")
        search_row.addWidget(search_label)

        self.search_entry = QLineEdit()
        self.search_entry.setPlaceholderText("Filter routes...")
        if saved_search:
            self.search_entry.setText(saved_search)
        self.search_entry.textChanged.connect(self._on_search_text_changed)
        search_row.addWidget(self.search_entry, 1)

        routes_layout.addLayout(search_row)

        # Route tree widget
        self.route_tree = QTreeWidget()
        self.route_tree.setHeaderLabels(["Game", "Species", "Route Name", "Date Played"])
        self.route_tree.setRootIsDecorated(False)
        self.route_tree.setAlternatingRowColors(True)
        self.route_tree.setSelectionMode(QAbstractItemView.SingleSelection)
        self.route_tree.setMinimumHeight(300)

        header = self.route_tree.header()
        header.setStretchLastSection(True)
        header.setSectionResizeMode(0, QHeaderView.Interactive)
        header.setSectionResizeMode(1, QHeaderView.Interactive)
        header.setSectionResizeMode(2, QHeaderView.Stretch)
        header.setSectionResizeMode(3, QHeaderView.Interactive)
        self.route_tree.setColumnWidth(0, 80)
        self.route_tree.setColumnWidth(1, 90)
        self.route_tree.setColumnWidth(3, 130)

        self.route_tree.itemSelectionChanged.connect(self._on_route_selection_changed)
        self.route_tree.itemDoubleClicked.connect(self._handle_route_double_click)

        # Enter key to load
        self.route_tree.setFocusPolicy(Qt.StrongFocus)

        routes_layout.addWidget(self.route_tree, 1)

        outer.addWidget(routes_container, 1, Qt.AlignHCenter)

        # Initial refresh
        self.refresh_routes()

    # ------------------------------------------------------------------
    # Game filter dropdown
    # ------------------------------------------------------------------
    def _populate_game_filter_dropdown(self):
        self.game_filter_dropdown.blockSignals(True)
        self.game_filter_dropdown.clear()
        all_games = ["All Games"]
        if gen_factory is not None:
            try:
                all_games += gen_factory.get_gen_names(real_gens=True, custom_gens=True)
            except Exception:
                pass
        self.game_filter_dropdown.addItems(all_games)
        idx = self.game_filter_dropdown.findText(self._selected_game_filter)
        if idx >= 0:
            self.game_filter_dropdown.setCurrentIndex(idx)
        else:
            self.game_filter_dropdown.setCurrentIndex(0)
            self._selected_game_filter = "All Games"
        self.game_filter_dropdown.blockSignals(False)

    # ------------------------------------------------------------------
    # Event handlers
    # ------------------------------------------------------------------
    def keyPressEvent(self, event):  # noqa: N802
        if event.key() in (Qt.Key_Return, Qt.Key_Enter):
            self._handle_load_selected_route()
            return
        super().keyPressEvent(event)

    def _handle_create_route(self):
        if self._on_create_route:
            self._on_create_route()

    def _handle_load_selected_route(self):
        items = self.route_tree.selectedItems()
        if not items:
            return
        route_name = items[0].text(2)  # column 2 = Route Name
        if not route_name or route_name == "No saved routes found":
            return
        route_path = io_utils.get_existing_route_path(route_name)
        if self._on_load_route:
            self._on_load_route(route_path)

    def _handle_route_double_click(self, item, column):
        route_name = item.text(2)
        if not route_name or route_name == "No saved routes found":
            return
        route_path = io_utils.get_existing_route_path(route_name)
        if self._on_load_route:
            self._on_load_route(route_path)

    def _on_auto_load_toggle_changed(self, state):
        config.set_auto_load_most_recent_route(self.auto_load_checkbox.isChecked())
        if self._on_auto_load_toggle:
            self._on_auto_load_toggle()

    def _on_route_selection_changed(self):
        items = self.route_tree.selectedItems()
        if items:
            route_name = items[0].text(2)
            if route_name and route_name != "No saved routes found":
                self.load_button.setEnabled(True)
            else:
                self.load_button.setEnabled(False)
        else:
            self.load_button.setEnabled(False)

    def _on_sort_changed(self, button):
        if button is self._btn_recent:
            self._current_sort = self.SORT_MOST_RECENT
        elif button is self._btn_alpha:
            self._current_sort = self.SORT_ALPHABETICAL
        elif button is self._btn_game:
            self._current_sort = self.SORT_GAME
        config.set_landing_page_sort(self._current_sort)

        # Show/hide game dropdown
        self.game_filter_dropdown.setVisible(self._current_sort == self.SORT_GAME)
        if self._current_sort == self.SORT_GAME:
            self._populate_game_filter_dropdown()

        self.refresh_routes()

    def _on_game_filter_changed(self, text):
        if not text:
            return
        self._selected_game_filter = text
        config.set_landing_page_game_filter(self._selected_game_filter)
        self.refresh_routes()

    def _on_search_text_changed(self, text):
        """Called on every keystroke; restarts the debounce timer."""
        self._search_timer.start()

    def _apply_search(self):
        """Fired by the debounce timer after 300 ms of inactivity."""
        search_value = self.search_entry.text()
        self._search_text = search_value.strip().lower()
        config.set_landing_page_search_filter(search_value)
        self.refresh_routes()

    # ------------------------------------------------------------------
    # Metadata helpers
    # ------------------------------------------------------------------
    def _get_route_metadata(self, route_name: str) -> Tuple[str, str, str, float]:
        """Return (game_version, species_name, route_name, mtime) for *route_name*.

        Uses an in-memory cache keyed on ``(route_name, mtime)`` so that the
        JSON file is only read when the file has actually been modified.
        """
        route_path = io_utils.get_existing_route_path(route_name)

        try:
            mtime = os.path.getmtime(route_path)
        except Exception:
            mtime = 0

        # Check cache
        if route_name in self._route_metadata_cache:
            cached = self._route_metadata_cache[route_name]
            if len(cached) == 3:
                cached_version, cached_species, cached_mtime = cached
                if cached_mtime == mtime:
                    return cached_version, cached_species, route_name, mtime

        # Cache miss -- read file
        game_version = "Unknown"
        species_name = "Unknown"
        try:
            import json
            with open(route_path, 'r') as f:
                route_data = json.load(f)
            game_version = route_data.get(const.PKMN_VERSION_KEY, "Unknown")
            species_name = route_data.get(const.NAME_KEY, "Unknown")
        except Exception:
            pass

        self._route_metadata_cache[route_name] = (game_version, species_name, mtime)
        return game_version, species_name, route_name, mtime

    # ------------------------------------------------------------------
    # Refresh / populate route list
    # ------------------------------------------------------------------
    def refresh_routes(self):
        """Reload and repopulate the route list based on current sort, filter,
        and search settings."""

        # Refresh game dropdown if sorting by game
        if self._current_sort == self.SORT_GAME and gen_factory is not None:
            self._populate_game_filter_dropdown()

        all_routes = io_utils.get_existing_route_names(load_backups=False)

        self.route_tree.clear()

        if not all_routes:
            placeholder = QTreeWidgetItem(["", "", "No saved routes found", ""])
            placeholder.setFlags(Qt.NoItemFlags)
            self.route_tree.addTopLevelItem(placeholder)
            self._route_metadata_cache = {}
            return

        # Gather metadata
        route_metadata = []
        for rname in all_routes:
            game_version, species_name, rname_clean, mtime = self._get_route_metadata(rname)
            route_metadata.append((rname_clean, game_version, species_name, mtime))

        # Prune cache
        existing = set(all_routes)
        self._route_metadata_cache = {
            k: v for k, v in self._route_metadata_cache.items() if k in existing
        }

        # Game filter
        if self._current_sort == self.SORT_GAME and self._selected_game_filter != "All Games":
            route_metadata = [
                (n, v, s, m) for n, v, s, m in route_metadata
                if v == self._selected_game_filter
            ]

        # Search filter
        if self._search_text:
            st = self._search_text
            route_metadata = [
                (n, v, s, m) for n, v, s, m in route_metadata
                if st in n.lower() or st in v.lower() or st in s.lower()
            ]

        # Sort
        if self._current_sort == self.SORT_MOST_RECENT:
            route_metadata.sort(key=lambda x: x[3], reverse=True)
        elif self._current_sort == self.SORT_GAME:
            route_metadata.sort(key=lambda x: (x[1], x[0]))
        elif self._current_sort == self.SORT_ALPHABETICAL:
            route_metadata.sort(key=lambda x: x[0].lower())

        # Populate tree
        for route_name, game_version, species_name, mtime in route_metadata:
            try:
                mtime_str = datetime.fromtimestamp(mtime).strftime("%Y-%m-%d %H:%M")
            except Exception:
                mtime_str = "Unknown"
            item = QTreeWidgetItem([game_version, species_name, route_name, mtime_str])
            hex_color = const.VERSION_COLORS.get(game_version)
            if hex_color:
                bg = QColor(hex_color)
                item.setBackground(0, QBrush(bg))
                lum = 0.299 * bg.red() + 0.587 * bg.green() + 0.114 * bg.blue()
                item.setForeground(0, QBrush(QColor("black") if lum > 128 else QColor("white")))
            self.route_tree.addTopLevelItem(item)
