from typing import List
from dataclasses import dataclass
import logging

from PySide6.QtWidgets import (
    QWidget, QLabel, QGridLayout, QVBoxLayout, QHBoxLayout,
    QScrollArea, QMenuBar, QSizePolicy, QFrame,
)
from PySide6.QtCore import Qt
from PySide6.QtGui import QAction, QKeySequence, QShortcut

from controllers.main_controller import MainController
from pkmn.gen_factory import current_gen_info
from utils.constants import const
from utils.config_manager import config

logger = logging.getLogger(__name__)


# Pokemon type -> background colour mapping (from the Tk dark theme)
TYPE_COLORS = {
    "Normal":   "#a8a878",
    "Fighting": "#c03028",
    "Grass":    "#78c850",
    "Fire":     "#f08030",
    "Water":    "#6890f0",
    "Electric": "#f3d230",
    "Ground":   "#e0c068",
    "Rock":     "#b8a038",
    "Psychic":  "#f85888",
    "Poison":   "#a040a0",
    "Flying":   "#a890f0",
    "Bug":      "#a8b820",
    "Ice":      "#98d8d8",
    "Ghost":    "#705898",
    "Dragon":   "#7038f8",
    "Steel":    "#b8b8d0",
    "Dark":     "#705848",
    "Fairy":    "#ee99ac",
    "Curse":    "#2e9fa3",
    "none":     "#333333",
    "":         "#333333",
    None:       "#333333",
}

# Types whose colours are dark enough that white text is more readable
_LIGHT_TEXT_TYPES = {"Poison", "Ghost", "Dark", "Dragon", "Fighting"}

SUMMARY_HEADER_BG = "#737373"        # disabledbg in the Tk theme
SUMMARY_HEADER_CANDY_BG = "#61520f"  # warning colour in the Tk theme


@dataclass
class SummaryInfo:
    trainer_name: str
    mon_level: int
    held_item: str
    moves: List[str]
    rare_candy_count: int


@dataclass
class RenderInfo:
    move_name: str
    move_type: str
    start_idx: int
    end_idx: int


class RouteSummaryWindow(QWidget):
    """Non-modal window that displays a grid summary of the current route.

    Each column represents a major trainer fight or rare-candy usage,
    with header rows for trainer name / level, an optional held-item row,
    and up to four move-slot rows colour-coded by Pokemon type.
    """

    def __init__(self, main_window, controller: MainController, parent=None):
        super().__init__(parent)
        self.setWindowFlags(Qt.Window)
        self.setAttribute(Qt.WA_DeleteOnClose)

        self._controller = controller
        self._main_window = main_window

        self.setWindowTitle("Route Summary")

        # ---- menu bar ----
        outer = QVBoxLayout(self)
        outer.setContentsMargins(0, 0, 0, 0)
        outer.setSpacing(0)

        self._menu_bar = QMenuBar(self)
        export_action = QAction("Export Screenshot (Ctrl+P)", self)
        export_action.setShortcut(QKeySequence("Ctrl+P"))
        export_action.triggered.connect(self._export_screen_shot)
        self._menu_bar.addAction(export_action)
        outer.addWidget(self._menu_bar)

        # ---- scrollable content area ----
        self._scroll_area = QScrollArea()
        self._scroll_area.setWidgetResizable(True)
        self._scroll_area.setHorizontalScrollBarPolicy(Qt.ScrollBarAsNeeded)
        self._scroll_area.setVerticalScrollBarPolicy(Qt.ScrollBarAsNeeded)
        outer.addWidget(self._scroll_area)

        self._content_widget = QWidget()
        self._grid = QGridLayout(self._content_widget)
        self._grid.setSpacing(2)
        self._grid.setContentsMargins(4, 4, 4, 4)
        self._scroll_area.setWidget(self._content_widget)

        # ---- keyboard shortcuts ----
        QShortcut(QKeySequence("Ctrl+P"), self, self._export_screen_shot)
        QShortcut(QKeySequence("Ctrl+`"), self, lambda: self._main_window.open_summary_window())

        # ---- controller subscription ----
        self._unsubscribe_route = self._controller.register_route_change(self._refresh)

        self._refresh()

    # ------------------------------------------------------------------
    # Cleanup
    # ------------------------------------------------------------------
    def closeEvent(self, event):
        if self._unsubscribe_route is not None:
            self._unsubscribe_route()
            self._unsubscribe_route = None
        super().closeEvent(event)

    # ------------------------------------------------------------------
    # Screenshot
    # ------------------------------------------------------------------
    def _export_screen_shot(self):
        geo = self.geometry()
        bbox = (geo.x(), geo.y(), geo.x() + geo.width(), geo.y() + geo.height())
        self._controller.take_screenshot("run_summary", bbox)

    # ------------------------------------------------------------------
    # Resize to fit content
    # ------------------------------------------------------------------
    def _resize_to_content(self):
        self._grid.activate()
        content_size = self._content_widget.sizeHint()
        scroll_frame = self._scroll_area.frameWidth() * 2
        menu_h = self._menu_bar.sizeHint().height()
        target_w = content_size.width() + scroll_frame + 16
        target_h = content_size.height() + scroll_frame + menu_h + 16
        screen = self.screen().availableGeometry()
        self.resize(min(target_w, screen.width()), min(target_h, screen.height()))

    # ------------------------------------------------------------------
    # Helpers for building styled cells
    # ------------------------------------------------------------------
    @staticmethod
    def _make_header_cell(text: str, bg: str) -> QFrame:
        frame = QFrame()
        frame.setStyleSheet(f"QFrame {{ background-color: {bg}; border-radius: 3px; }}")
        layout = QVBoxLayout(frame)
        layout.setContentsMargins(5, 5, 5, 5)
        layout.setSpacing(2)
        label = QLabel(text)
        label.setAlignment(Qt.AlignCenter)
        label.setStyleSheet(f"color: white; background: transparent;")
        layout.addWidget(label)
        return frame, label

    @staticmethod
    def _make_type_cell(text: str, move_type: str) -> QFrame:
        bg = TYPE_COLORS.get(move_type, TYPE_COLORS[None])
        fg = "white" if move_type in _LIGHT_TEXT_TYPES else config.get_background_color()
        frame = QFrame()
        frame.setStyleSheet(f"QFrame {{ background-color: {bg}; border-radius: 2px; }}")
        layout = QVBoxLayout(frame)
        layout.setContentsMargins(4, 2, 4, 2)
        label = QLabel(text)
        label.setAlignment(Qt.AlignCenter)
        label.setStyleSheet(f"color: {fg}; background: transparent;")
        layout.addWidget(label)
        return frame

    # ------------------------------------------------------------------
    # Main refresh logic  (mirrors the Tk version)
    # ------------------------------------------------------------------
    def _refresh(self):
        # Tear down previous content
        while self._grid.count():
            item = self._grid.takeAt(0)
            widget = item.widget()
            if widget is not None:
                widget.setParent(None)
                widget.deleteLater()

        ROW_HEADER = 0
        ROW_HELD_ITEM = 1
        ROW_MOVES_START = 2

        # -- Collect summary data -----------------------------------------------
        summary_list: List[SummaryInfo] = []
        elite_four_seen = set()
        crystal_excluded_trainers = {
            "Leader Brock", "Leader Misty", "Leader Lt.Surge",
            "Leader Erika", "Leader Sabrina", "Leader Blaine", "Leader Janine",
        }
        heartgold_excluded_trainers = {
            "Leader Brock", "Leader Misty", "Leader Lt.Surge",
            "Leader Erika", "Leader Sabrina", "Leader Blaine", "Leader Janine",
            "Elite Four Will Rematch 2", "Elite Four Koga Rematch 2",
            "Elite Four Bruno Rematch 2", "Elite Four Karen Rematch 2",
        }

        cur_event = self._controller.get_next_event()
        while cur_event is not None:
            if (
                cur_event.event_definition.trainer_def is not None
                and cur_event.event_definition.enabled
            ):
                trainer_name = cur_event.event_definition.trainer_def.trainer_name
                is_elite_four = trainer_name.startswith("Elite Four ")

                if cur_event.event_definition.is_highlighted():
                    cur_event = self._controller.get_next_event(cur_event.group_id)
                    continue

                if (
                    current_gen_info().version_name() == const.CRYSTAL_VERSION
                    and trainer_name in crystal_excluded_trainers
                ):
                    cur_event = self._controller.get_next_event(cur_event.group_id)
                    continue

                if (
                    current_gen_info().version_name() == const.HEART_GOLD_VERSION
                    and trainer_name in heartgold_excluded_trainers
                ):
                    cur_event = self._controller.get_next_event(cur_event.group_id)
                    continue

                if is_elite_four:
                    if trainer_name in elite_four_seen:
                        cur_event = self._controller.get_next_event(cur_event.group_id)
                        continue
                    elite_four_seen.add(trainer_name)

                if current_gen_info().is_major_fight(trainer_name):
                    summary_list.append(
                        SummaryInfo(
                            trainer_name,
                            cur_event.init_state.solo_pkmn.cur_level,
                            cur_event.init_state.solo_pkmn.held_item,
                            cur_event.init_state.solo_pkmn.move_list,
                            0,
                        )
                    )
            elif (
                cur_event.event_definition.rare_candy is not None
                and cur_event.event_definition.enabled
                and cur_event.event_definition.rare_candy.amount > 0
            ):
                summary_list.append(
                    SummaryInfo(
                        "",
                        cur_event.final_state.solo_pkmn.cur_level,
                        cur_event.final_state.solo_pkmn.held_item,
                        cur_event.final_state.solo_pkmn.move_list,
                        rare_candy_count=cur_event.event_definition.rare_candy.amount,
                    )
                )
            cur_event = self._controller.get_next_event(cur_event.group_id)

        # -- Empty state ---------------------------------------------------------
        if len(summary_list) == 0:
            frame, _ = self._make_header_cell(
                "No major fights in route. Please add major fights or highlight other fights to see summary",
                SUMMARY_HEADER_BG,
            )
            self._grid.addWidget(frame, 0, 0)
            self._resize_to_content()
            return

        # -- Build display structures -------------------------------------------
        move_display_info: List[List[RenderInfo]] = [[], [], [], []]
        held_item_display_info: List[RenderInfo] = []

        for cur_idx, cur_summary in enumerate(summary_list):
            # Header cell
            if cur_summary.trainer_name:
                header_bg = SUMMARY_HEADER_BG
                level_text = f"Lv: {cur_summary.mon_level}"

                split_name = cur_summary.trainer_name.split(" ")
                if len(split_name) > 2:
                    trainer_text = " ".join(split_name[0:2]) + "\n" + " ".join(split_name[2:])
                elif len(split_name) == 2 and len(split_name[1]) > 1:
                    trainer_text = split_name[0] + "\n" + split_name[1]
                else:
                    trainer_text = cur_summary.trainer_name
            else:
                header_bg = SUMMARY_HEADER_CANDY_BG
                trainer_text = f"Rare Candy\nx{cur_summary.rare_candy_count}"
                level_text = f"Lv: {cur_summary.mon_level - cur_summary.rare_candy_count}->{cur_summary.mon_level}"

            # Build header frame with trainer + level
            header_frame = QFrame()
            header_frame.setStyleSheet(
                f"QFrame {{ background-color: {header_bg}; border-radius: 3px; }}"
            )
            header_layout = QVBoxLayout(header_frame)
            header_layout.setContentsMargins(5, 15, 5, 15)
            header_layout.setSpacing(2)

            trainer_label = QLabel(trainer_text)
            trainer_label.setAlignment(Qt.AlignCenter)
            trainer_label.setStyleSheet("color: white; background: transparent;")
            header_layout.addWidget(trainer_label)

            level_label = QLabel(level_text)
            level_label.setAlignment(Qt.AlignCenter)
            level_label.setStyleSheet("color: white; background: transparent;")
            header_layout.addWidget(level_label, 0, Qt.AlignBottom)

            self._grid.addWidget(header_frame, ROW_HEADER, cur_idx)

            # Held item tracking
            if (
                len(held_item_display_info) == 0
                or held_item_display_info[-1].move_name != cur_summary.held_item
            ):
                held_item_display_info.append(
                    RenderInfo(cur_summary.held_item, None, cur_idx, cur_idx)
                )
            else:
                held_item_display_info[-1].end_idx = cur_idx

            # Move tracking
            for move_idx in range(4):
                next_move = ""
                if move_idx < len(cur_summary.moves):
                    next_move = cur_summary.moves[move_idx]
                    if next_move is None:
                        next_move = ""

                if next_move == "":
                    move_type = ""
                elif next_move == const.HIDDEN_POWER_MOVE_NAME:
                    move_type = current_gen_info().get_hidden_power(self._controller.get_dvs())[0]
                    next_move = f"{next_move} ({move_type})"
                else:
                    move_type = current_gen_info().move_db().get_move(next_move).move_type

                if (
                    len(move_display_info[move_idx]) == 0
                    or move_display_info[move_idx][-1].move_name != next_move
                ):
                    move_display_info[move_idx].append(
                        RenderInfo(next_move, move_type, cur_idx, cur_idx)
                    )
                else:
                    move_display_info[move_idx][-1].end_idx = cur_idx

        # -- Held-item row (gen 2+) ---------------------------------------------
        if current_gen_info().get_generation() != 1:
            for info in held_item_display_info:
                display_text = info.move_name if info.move_name else "None"
                frame, _ = self._make_header_cell(display_text, SUMMARY_HEADER_BG)
                colspan = (info.end_idx - info.start_idx) + 1
                self._grid.addWidget(frame, ROW_HELD_ITEM, info.start_idx, 1, colspan)

        # -- Move rows -----------------------------------------------------------
        for move_slot_idx, slot_display in enumerate(move_display_info):
            for info in slot_display:
                cell = self._make_type_cell(info.move_name, info.move_type)
                colspan = (info.end_idx - info.start_idx) + 1
                self._grid.addWidget(
                    cell,
                    ROW_MOVES_START + move_slot_idx,
                    info.start_idx,
                    1,
                    colspan,
                )

        # -- Resize window to fit content ----------------------------------------
        self._resize_to_content()
