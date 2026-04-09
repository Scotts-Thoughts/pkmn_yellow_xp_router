import os
import json
import logging

from PySide6.QtWidgets import (
    QWidget, QLabel, QVBoxLayout, QHBoxLayout, QTreeWidget,
    QTreeWidgetItem, QHeaderView, QAbstractItemView, QApplication,
    QMessageBox, QSizePolicy,
)
from PySide6.QtCore import Qt, QTimer, QSize
from PySide6.QtGui import QColor, QBrush, QIcon

from gui_qt.components.custom_components import SimpleEntry, SimpleOptionMenu, SimpleButton
from gui_qt.pkmn_components.custom_dvs import CustomDVsFrame
from gui_qt import box_art
from utils.constants import const
from utils import io_utils
from pkmn.gen_factory import _gen_factory as gen_factory, current_gen_info

logger = logging.getLogger(__name__)


class NewRoutePage(QWidget):
    """Full-page widget for creating a new route."""

    # Game information mapping: version -> (generation, platform, recorder_status)
    GAME_INFO = {
        const.RED_VERSION: ("Generation 1", "GB/GBC", "Available"),
        const.BLUE_VERSION: ("Generation 1", "GB/GBC", "Available"),
        const.YELLOW_VERSION: ("Generation 1", "GB/GBC", "Available"),
        const.GOLD_VERSION: ("Generation 2", "GBC", "Available"),
        const.SILVER_VERSION: ("Generation 2", "GBC", "Available"),
        const.CRYSTAL_VERSION: ("Generation 2", "GBC", "Available"),
        const.RUBY_VERSION: ("Generation 3", "GBA", "Available"),
        const.SAPPHIRE_VERSION: ("Generation 3", "GBA", "Available"),
        const.EMERALD_VERSION: ("Generation 3", "GBA", "Available"),
        const.FIRE_RED_VERSION: ("Generation 3", "GBA", "Available"),
        const.LEAF_GREEN_VERSION: ("Generation 3", "GBA", "Available"),
        const.DIAMOND_VERSION: ("Generation 4", "NDS", "Unavailable"),
        const.PEARL_VERSION: ("Generation 4", "NDS", "Unavailable"),
        const.PLATINUM_VERSION: ("Generation 4", "NDS", "In Beta"),
        const.HEART_GOLD_VERSION: ("Generation 4", "NDS", "In Alpha"),
        const.SOUL_SILVER_VERSION: ("Generation 4", "NDS", "In Alpha"),
    }

    def __init__(self, parent, controller, on_cancel=None, on_create=None):
        super().__init__(parent)
        self._controller = controller
        self._on_cancel = on_cancel
        self._on_create = on_create
        self._selected_game = None
        self._selected_gen_obj = None
        self._current_gen_num = None
        self._route_cache_per_game = {}
        self._pkmn_list_cache = {}
        self._last_pkmn_filter = ""
        self._suppress_busy_cursor = True

        self._build_ui()

    # ------------------------------------------------------------------
    # UI construction
    # ------------------------------------------------------------------
    def _build_ui(self):
        outer = QVBoxLayout(self)
        outer.setContentsMargins(0, 0, 0, 0)

        # ---- Title ----------------------------------------------------
        title = QLabel("Create New Route")
        title.setAlignment(Qt.AlignCenter)
        title_font = title.font()
        title_font.setPointSize(24)
        title_font.setBold(True)
        title.setFont(title_font)
        outer.addSpacing(30)
        outer.addWidget(title, 0, Qt.AlignCenter)
        outer.addSpacing(10)

        # ---- Centred content column -----------------------------------
        content = QWidget()
        content_layout = QVBoxLayout(content)
        content_layout.setContentsMargins(100, 0, 100, 0)
        content_layout.setSpacing(5)

        padx = 10
        pady = 5

        # -- Pokemon Version: game table --------------------------------
        version_row = QHBoxLayout()
        version_row.setSpacing(padx)
        version_label = QLabel("Pokemon Version:")
        lbl_font = version_label.font()
        lbl_font.setPointSize(12)
        version_label.setFont(lbl_font)
        version_row.addWidget(version_label, 0, Qt.AlignTop)

        self.game_treeview = QTreeWidget()
        self.game_treeview.setHeaderLabels(["Box Art", "Game", "Generation", "Platform", "Recorder"])
        self.game_treeview.setRootIsDecorated(False)
        self.game_treeview.setSelectionMode(QAbstractItemView.SingleSelection)
        self.game_treeview.setAlternatingRowColors(True)
        # Prefer showing all rows; shrink with scrollbar in small windows
        self.game_treeview.setMinimumHeight(100)
        self.game_treeview.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Expanding)
        # Icon size for the box-art column. Rows grow to fit this height so
        # box art is big enough to be recognizable at a glance.
        self._box_art_size = QSize(72, 72)
        self.game_treeview.setIconSize(self._box_art_size)

        header = self.game_treeview.header()
        header.setStretchLastSection(True)
        header.setSectionResizeMode(0, QHeaderView.Fixed)
        header.setSectionResizeMode(1, QHeaderView.Interactive)
        header.setSectionResizeMode(2, QHeaderView.Interactive)
        header.setSectionResizeMode(3, QHeaderView.Interactive)
        header.setSectionResizeMode(4, QHeaderView.Stretch)
        self.game_treeview.setColumnWidth(0, self._box_art_size.width() + 12)
        self.game_treeview.setColumnWidth(1, 120)
        self.game_treeview.setColumnWidth(2, 120)
        self.game_treeview.setColumnWidth(3, 100)

        self.game_treeview.itemSelectionChanged.connect(self._on_game_selection_changed)

        version_row.addWidget(self.game_treeview, 1)
        content_layout.addLayout(version_row, 1)

        # Populate table
        self._populate_game_table()

        # -- Solo Pokemon Filter ----------------------------------------
        filter_row1 = QHBoxLayout()
        filter_row1.setSpacing(padx)
        pkmn_filter_label = QLabel("Solo Pokemon Filter:")
        pkmn_filter_label.setFont(lbl_font)
        filter_row1.addWidget(pkmn_filter_label, 0, Qt.AlignTop)
        self.pkmn_filter = SimpleEntry(callback=self._pkmn_filter_callback)
        self.pkmn_filter.setMinimumWidth(300)
        entry_font = self.pkmn_filter.font()
        entry_font.setPointSize(11)
        self.pkmn_filter.setFont(entry_font)
        filter_row1.addWidget(self.pkmn_filter, 1)
        content_layout.addLayout(filter_row1)

        # -- Solo Pokemon selector --------------------------------------
        selector_row1 = QHBoxLayout()
        selector_row1.setSpacing(padx)
        solo_label = QLabel("Solo Pokemon:")
        solo_label.setFont(lbl_font)
        selector_row1.addWidget(solo_label, 0, Qt.AlignTop)
        self.solo_selector = SimpleOptionMenu(
            option_list=[const.NO_POKEMON],
            callback=self._pkmn_selector_callback,
        )
        self.solo_selector.setMinimumWidth(250)
        selector_row1.addWidget(self.solo_selector, 1)
        content_layout.addLayout(selector_row1)

        # -- Base Route Filter ------------------------------------------
        filter_row2 = QHBoxLayout()
        filter_row2.setSpacing(padx)
        base_filter_label = QLabel("Base Route Filter:")
        base_filter_label.setFont(lbl_font)
        filter_row2.addWidget(base_filter_label, 0, Qt.AlignTop)
        self.min_battles_filter = SimpleEntry(callback=self._base_route_filter_callback)
        self.min_battles_filter.setMinimumWidth(300)
        self.min_battles_filter.setFont(entry_font)
        filter_row2.addWidget(self.min_battles_filter, 1)
        content_layout.addLayout(filter_row2)

        # -- Base Route selector ----------------------------------------
        self._min_battles_cache = [const.EMPTY_ROUTE_NAME]
        selector_row2 = QHBoxLayout()
        selector_row2.setSpacing(padx)
        base_route_label = QLabel("Base Route:")
        base_route_label.setFont(lbl_font)
        selector_row2.addWidget(base_route_label, 0, Qt.AlignTop)
        self.min_battles_selector = SimpleOptionMenu(
            option_list=self._min_battles_cache,
        )
        self.min_battles_selector.setMinimumWidth(250)
        selector_row2.addWidget(self.min_battles_selector, 1)
        content_layout.addLayout(selector_row2)

        # -- Custom DVs frame -------------------------------------------
        self.custom_dvs_frame = CustomDVsFrame(
            None, parent=self, target_game=current_gen_info()
        )
        content_layout.addWidget(self.custom_dvs_frame, 0, Qt.AlignCenter)

        # -- Warning label ----------------------------------------------
        self.warning_label = QLabel(
            "WARNING: Any unsaved changes in your current route\n"
            "will be lost when creating a new route!"
        )
        self.warning_label.setAlignment(Qt.AlignCenter)
        warn_font = self.warning_label.font()
        warn_font.setPointSize(10)
        self.warning_label.setFont(warn_font)
        self.warning_label.setStyleSheet("color: red;")
        content_layout.addWidget(self.warning_label, 0, Qt.AlignCenter)

        # -- Buttons ----------------------------------------------------
        btn_row = QHBoxLayout()
        btn_row.setSpacing(padx)

        self.create_button = SimpleButton("Create Route", width=180)
        self.create_button.clicked.connect(self.create)
        btn_row.addWidget(self.create_button)

        self.cancel_button = SimpleButton("Cancel", width=180)
        self.cancel_button.clicked.connect(self._handle_cancel)
        btn_row.addWidget(self.cancel_button)

        content_layout.addLayout(btn_row)

        outer.addWidget(content, 1)
        outer.addSpacing(10)

        # ---- Initial selection ----------------------------------------
        if self.game_treeview.topLevelItemCount() > 0:
            first = self.game_treeview.topLevelItem(0)
            self.game_treeview.setCurrentItem(first)
            # The signal fires automatically; data will be loaded there.

        self.pkmn_filter.setFocus()

    # ------------------------------------------------------------------
    # Keyboard shortcuts
    # ------------------------------------------------------------------
    def keyPressEvent(self, event):  # noqa: N802
        if event.key() in (Qt.Key_Return, Qt.Key_Enter):
            self.create()
            return
        if event.key() == Qt.Key_Escape:
            self._handle_cancel()
            return
        super().keyPressEvent(event)

    # ------------------------------------------------------------------
    # Game table helpers
    # ------------------------------------------------------------------
    def _populate_game_table(self):
        """Populate the game selection tree with all available games."""
        self.game_treeview.clear()

        all_games = gen_factory.get_gen_names(real_gens=True, custom_gens=True)

        official_games = [
            const.RED_VERSION, const.BLUE_VERSION, const.YELLOW_VERSION,
            const.GOLD_VERSION, const.SILVER_VERSION, const.CRYSTAL_VERSION,
            const.RUBY_VERSION, const.SAPPHIRE_VERSION, const.EMERALD_VERSION,
            const.FIRE_RED_VERSION, const.LEAF_GREEN_VERSION,
            const.DIAMOND_VERSION, const.PEARL_VERSION, const.PLATINUM_VERSION,
            const.HEART_GOLD_VERSION, const.SOUL_SILVER_VERSION,
        ]

        sorted_games = [g for g in official_games if g in all_games]
        custom_gens = [g for g in all_games if g not in official_games]
        sorted_games.extend(sorted(custom_gens))

        for game_name in sorted_games:
            if game_name in self.GAME_INFO:
                gen, platform, recorder = self.GAME_INFO[game_name]
            else:
                try:
                    gen_obj = gen_factory.get_specific_version(game_name)
                    gen_num = gen_obj.get_generation()
                    gen = f"Generation {gen_num}"
                    platform = "Custom"
                    base_version = gen_obj.base_version_name()
                    if base_version and base_version in self.GAME_INFO:
                        _, _, recorder = self.GAME_INFO[base_version]
                    elif game_name in self.GAME_INFO:
                        _, _, recorder = self.GAME_INFO[game_name]
                    else:
                        recorder = "Unknown"
                except Exception:
                    gen = "Unknown"
                    platform = "Unknown"
                    recorder = "Unknown"

            item = QTreeWidgetItem(["", game_name, gen, platform, recorder])
            # Attach box art (if available) to the leading column
            pm = box_art.get_box_art(
                game_name,
                self._box_art_size.width(),
                self._box_art_size.height(),
            )
            if pm is not None:
                item.setIcon(0, QIcon(pm))
            # Version colors removed for cleaner appearance
            self.game_treeview.addTopLevelItem(item)

    # ------------------------------------------------------------------
    # Busy cursor helpers (replaces the Tk loading popup)
    # ------------------------------------------------------------------
    def _show_busy(self):
        if not self._suppress_busy_cursor:
            QApplication.setOverrideCursor(Qt.WaitCursor)
            QApplication.processEvents()

    def _hide_busy(self):
        if not self._suppress_busy_cursor:
            QApplication.restoreOverrideCursor()
        # After the first full load cycle, allow busy cursor for future ops
        self._suppress_busy_cursor = False

    # ------------------------------------------------------------------
    # Game selection change
    # ------------------------------------------------------------------
    def _on_game_selection_changed(self):
        items = self.game_treeview.selectedItems()
        if not items:
            return
        new_game = items[0].text(1)
        if new_game == self._selected_game:
            return

        needs_loading = (
            new_game not in self._route_cache_per_game
            or (new_game, self.pkmn_filter.get().strip()) not in self._pkmn_list_cache
        )
        if needs_loading:
            self._show_busy()

        self._selected_game = new_game
        self._selected_gen_obj = gen_factory.get_specific_version(self._selected_game)

        if self._selected_gen_obj is None:
            try:
                gen_factory.reload_all_custom_gens(retry_skipped=True)
                self._selected_gen_obj = gen_factory.get_specific_version(self._selected_game)
            except Exception as e:
                logger.warning(f"Could not load custom gen {self._selected_game}: {e}")

        if self._selected_gen_obj is None:
            self._hide_busy()
            QMessageBox.critical(
                self,
                "Error",
                f"Could not load game version '{self._selected_game}'. "
                "The base generation may not be available yet.",
            )
            return

        new_gen_num = self._selected_gen_obj.get_generation()
        gen_changed = self._current_gen_num != new_gen_num
        self._current_gen_num = new_gen_num

        self._update_pokemon_list_immediate()

        # Defer the heavier work so the treeview selection paints first
        QTimer.singleShot(0, lambda: self._pkmn_version_callback(gen_changed=gen_changed))

    # ------------------------------------------------------------------
    # Pokemon list helpers
    # ------------------------------------------------------------------
    def _update_pokemon_list_immediate(self):
        if not self._selected_game or not self._selected_gen_obj:
            return

        filter_val = self.pkmn_filter.get().strip()
        cache_key = (self._selected_game, filter_val)

        if cache_key in self._pkmn_list_cache:
            pkmn_list = self._pkmn_list_cache[cache_key]
        else:
            pkmn_list = self._selected_gen_obj.pkmn_db().get_filtered_names(filter_val=filter_val)
            self._pkmn_list_cache[cache_key] = pkmn_list

        self.solo_selector.new_values(pkmn_list)

    def _pkmn_version_callback(self, *args, **kwargs):
        if not self._selected_game or not self._selected_gen_obj:
            self._hide_busy()
            return

        gen_changed = kwargs.get("gen_changed", True)

        # Build / retrieve route cache
        if self._selected_game in self._route_cache_per_game:
            all_routes = self._route_cache_per_game[self._selected_game]
        else:
            all_routes = [const.EMPTY_ROUTE_NAME]
            for preset_route_name in self._selected_gen_obj.min_battles_db().data:
                all_routes.append(const.PRESET_ROUTE_PREFIX + preset_route_name)

            route_names = io_utils.get_existing_route_names()
            for test_route in route_names:
                try:
                    with open(io_utils.get_existing_route_path(test_route), "r") as f:
                        raw = json.load(f)
                        if raw[const.PKMN_VERSION_KEY] == self._selected_game:
                            all_routes.append(test_route)
                except Exception:
                    pass

            self._route_cache_per_game[self._selected_game] = all_routes

        self._min_battles_cache = all_routes
        self._base_route_filter_callback()

        if gen_changed:
            selected_pokemon = self.solo_selector.get()
            if selected_pokemon and selected_pokemon != const.NO_POKEMON:
                QTimer.singleShot(
                    0,
                    lambda: self._update_dvs_frame_and_finish(
                        self._selected_gen_obj,
                        self._selected_gen_obj.pkmn_db().get_pkmn(selected_pokemon),
                    ),
                )
            else:
                QTimer.singleShot(
                    0,
                    lambda: self._update_dvs_frame_and_finish(self._selected_gen_obj, None),
                )
        else:
            self._hide_busy()

    def _update_dvs_frame_and_finish(self, gen_obj, pokemon):
        self.custom_dvs_frame.config_for_target_game_and_mon(gen_obj, pokemon)
        self._hide_busy()

    # ------------------------------------------------------------------
    # Filter / selector callbacks
    # ------------------------------------------------------------------
    def _pkmn_filter_callback(self):
        if not self._selected_game or not self._selected_gen_obj:
            return

        filter_val = self.pkmn_filter.get().strip()
        cache_key = (self._selected_game, filter_val)

        if cache_key in self._pkmn_list_cache:
            pkmn_list = self._pkmn_list_cache[cache_key]
        else:
            if filter_val == "":
                pkmn_list = self._selected_gen_obj.pkmn_db().get_all_names()
            else:
                pkmn_list = self._selected_gen_obj.pkmn_db().get_filtered_names(filter_val=filter_val)
            self._pkmn_list_cache[cache_key] = pkmn_list

        self.solo_selector.new_values(pkmn_list)
        # Signals are blocked during new_values, so explicitly update the DV/ability frame
        self._pkmn_selector_callback()

    def _pkmn_selector_callback(self):
        if not self._selected_game or not self._selected_gen_obj:
            return

        selected_pokemon = self.solo_selector.get()
        if selected_pokemon and selected_pokemon != const.NO_POKEMON:
            QTimer.singleShot(
                0,
                lambda: self.custom_dvs_frame.config_for_target_game_and_mon(
                    self._selected_gen_obj,
                    self._selected_gen_obj.pkmn_db().get_pkmn(selected_pokemon),
                ),
            )
        else:
            QTimer.singleShot(
                0,
                lambda: self.custom_dvs_frame.config_for_target_game_and_mon(
                    self._selected_gen_obj, None
                ),
            )

    def _base_route_filter_callback(self):
        filter_val = self.min_battles_filter.get().strip().lower()
        new_vals = [x for x in self._min_battles_cache if filter_val in x.lower()]
        if not new_vals:
            new_vals = [const.EMPTY_ROUTE_NAME]
        self.min_battles_selector.new_values(new_vals)

    # ------------------------------------------------------------------
    # Actions
    # ------------------------------------------------------------------
    def create(self):
        """Gather form values and invoke the on_create callback."""
        selected_base_route = self.min_battles_selector.get()
        if selected_base_route == const.EMPTY_ROUTE_NAME:
            selected_base_route = None
        elif selected_base_route.startswith(const.PRESET_ROUTE_PREFIX):
            if not self._selected_game or not self._selected_gen_obj:
                return
            selected_base_route = os.path.join(
                self._selected_gen_obj.min_battles_db().get_dir(),
                selected_base_route[len(const.PRESET_ROUTE_PREFIX):] + ".json",
            )
        else:
            selected_base_route = io_utils.get_existing_route_path(selected_base_route)

        if not self._selected_game:
            return

        custom_dvs, custom_ability_idx, custom_nature = self.custom_dvs_frame.get_dvs()

        if self._on_create:
            self._on_create(
                self.solo_selector.get(),
                selected_base_route,
                self._selected_game,
                custom_dvs=custom_dvs,
                custom_ability_idx=custom_ability_idx,
                custom_nature=custom_nature,
            )

    def _handle_cancel(self):
        if self._on_cancel:
            self._on_cancel()

    # ------------------------------------------------------------------
    # Public helpers
    # ------------------------------------------------------------------
    def refresh_game_list(self):
        """Refresh the game list (e.g. after background loading completes)."""
        try:
            gen_factory.reload_all_custom_gens(retry_skipped=True)
        except Exception as e:
            logger.warning(f"Could not reload some custom gens: {e}")

        # Remember current selection
        current_selection = None
        items = self.game_treeview.selectedItems()
        if items:
            current_selection = items[0].text(1)

        self._populate_game_table()

        # Restore previous selection or fall back to first item
        if current_selection:
            for idx in range(self.game_treeview.topLevelItemCount()):
                item = self.game_treeview.topLevelItem(idx)
                if item.text(1) == current_selection:
                    self.game_treeview.setCurrentItem(item)
                    break
        elif self.game_treeview.topLevelItemCount() > 0:
            self.game_treeview.setCurrentItem(self.game_treeview.topLevelItem(0))

    def reset_form(self):
        """Reset the form to its initial state."""
        self.pkmn_filter.set("")
        self.min_battles_filter.set("")
        self._populate_game_table()
        self._selected_game = None
        self._selected_gen_obj = None
        self._current_gen_num = None
        self._last_pkmn_filter = ""
        self._pkmn_list_cache.clear()

        if self.game_treeview.topLevelItemCount() > 0:
            first = self.game_treeview.topLevelItem(0)
            self.game_treeview.setCurrentItem(first)
            # Signal fires automatically, triggers _on_game_selection_changed
        else:
            self.solo_selector.new_values([const.NO_POKEMON])
            self.min_battles_selector.new_values([const.EMPTY_ROUTE_NAME])
            self.custom_dvs_frame.config_for_target_game_and_mon(current_gen_info(), None)
