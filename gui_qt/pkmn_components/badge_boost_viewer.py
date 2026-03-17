import logging
from typing import List

from PySide6.QtWidgets import (
    QWidget, QLabel, QVBoxLayout, QGridLayout,
)
from PySide6.QtCore import Qt

from gui_qt.components.custom_components import SimpleOptionMenu
from gui_qt.pkmn_components.pkmn_viewer import PkmnViewer

from pkmn.gen_factory import current_gen_info
from pkmn import universal_data_objects
from routing import full_route_state

logger = logging.getLogger(__name__)


class BadgeBoostViewer(QWidget):
    def __init__(self, parent=None):
        super().__init__(parent)

        self._grid_layout = QGridLayout(self)
        self._grid_layout.setContentsMargins(0, 0, 0, 0)
        self._grid_layout.setSpacing(3)

        # Info frame at position (0, 0) in the grid
        self._info_frame = QWidget(self)
        info_layout = QVBoxLayout(self._info_frame)
        info_layout.setContentsMargins(0, 0, 0, 0)
        info_layout.setSpacing(0)

        self._move_selector_label = QLabel("Setup Move: ")
        info_layout.addWidget(self._move_selector_label)

        self._move_selector = SimpleOptionMenu(
            option_list=["N/A"],
            callback=self._move_selected_callback,
        )
        info_layout.addWidget(self._move_selector)

        self._badge_summary = QLabel("")
        info_layout.addWidget(self._badge_summary)

        self._grid_layout.addWidget(self._info_frame, 0, 0, alignment=Qt.AlignTop)

        self._state: full_route_state.RouteState = None

        # 6 possible badge boosts from a single setup move, plus unmodified summary
        NUM_SUMMARIES = 7
        NUM_COLS = 4
        self._frames: List[QWidget] = []
        self._labels: List[QLabel] = []
        self._viewers: List[PkmnViewer] = []

        for idx in range(NUM_SUMMARIES):
            cur_frame = QWidget(self)
            cur_frame_layout = QVBoxLayout(cur_frame)
            cur_frame_layout.setContentsMargins(3, 3, 3, 3)
            cur_frame_layout.setSpacing(0)

            cur_label = QLabel("")
            cur_label.setAlignment(Qt.AlignCenter)
            cur_frame_layout.addWidget(cur_label)

            cur_viewer = PkmnViewer(cur_frame, stats_only=True)
            cur_frame_layout.addWidget(cur_viewer)

            # add 1 because the 0th cell is the info frame
            row = (idx + 1) // NUM_COLS
            col = (idx + 1) % NUM_COLS
            self._grid_layout.addWidget(cur_frame, row, col, alignment=Qt.AlignTop)

            self._frames.append(cur_frame)
            self._labels.append(cur_label)
            self._viewers.append(cur_viewer)

    def _clear_all_summaries(self):
        # intentionally skip base stat frame
        for idx in range(1, len(self._frames)):
            self._labels[idx].setVisible(False)
            self._viewers[idx].setVisible(False)

    def _update_base_summary(self):
        if self._state is None:
            self._labels[0].setVisible(False)
            self._viewers[0].setVisible(False)
            return

        self._labels[0].setText(f"Base: {self._state.solo_pkmn.name}")
        self._labels[0].setVisible(True)

        self._viewers[0].set_pkmn(
            self._state.solo_pkmn.get_pkmn_obj(self._state.badges),
            badges=self._state.badges,
        )
        self._viewers[0].setVisible(True)

    def _move_selected_callback(self, *args, **kwargs):
        self._update_base_summary()

        move = self._move_selector.get()
        if not move:
            self._clear_all_summaries()
            return

        prev_mod = universal_data_objects.StageModifiers()
        stage_mod = None
        for idx in range(1, len(self._frames)):
            stage_mod = prev_mod.apply_stat_mod(current_gen_info().move_db().get_stat_mod(move))
            if stage_mod == prev_mod:
                self._labels[idx].setVisible(False)
                self._viewers[idx].setVisible(False)
                continue

            prev_mod = stage_mod

            self._labels[idx].setText(f"{idx}x {move}")
            self._labels[idx].setVisible(True)

            self._viewers[idx].set_pkmn(
                self._state.solo_pkmn.get_pkmn_obj(self._state.badges, stage_mod),
                badges=self._state.badges,
            )
            self._viewers[idx].setVisible(True)

    def set_state(self, state: full_route_state.RouteState):
        self._state = state
        self._move_selector.new_values(current_gen_info().get_stat_modifer_moves())

        # when state changes, update the badge list label
        raw_badge_text = self._state.badges.to_string(verbose=False)
        final_badge_text = raw_badge_text.split(":")[0]
        badges = raw_badge_text.split(":")[1]

        if not badges.strip():
            final_badge_text += "\nNone"
        else:
            earned_badges = badges.split(',')
            badges = ""
            while len(earned_badges) > 0:
                if len(earned_badges) == 1:
                    badges += f"{earned_badges[0]}\n"
                    del earned_badges[0]
                else:
                    badges += f"{earned_badges[0]}, {earned_badges[1]}\n"
                    del earned_badges[0]
                    del earned_badges[0]

            final_badge_text += '\n' + badges.strip()

        self._badge_summary.setText(final_badge_text)
        self._move_selected_callback()
