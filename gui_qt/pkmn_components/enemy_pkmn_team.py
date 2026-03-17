import logging
from typing import List

from PySide6.QtWidgets import (
    QWidget, QGridLayout,
)
from PySide6.QtCore import Qt

from gui_qt.pkmn_components.pkmn_viewer import PkmnViewer

from pkmn import universal_data_objects
from routing import full_route_state

logger = logging.getLogger(__name__)


class EnemyPkmnTeam(QWidget):
    def __init__(self, parent=None):
        super().__init__(parent)

        self._layout = QGridLayout(self)
        self._layout.setContentsMargins(0, 0, 0, 0)
        self._layout.setSpacing(5)

        self._all_pkmn: List[PkmnViewer] = []

        for _ in range(6):
            viewer = PkmnViewer(self, font_size=10)
            viewer.setVisible(False)
            self._all_pkmn.append(viewer)

    def set_team(self, enemy_pkmn: List[universal_data_objects.EnemyPkmn], cur_state: full_route_state.RouteState = None):
        if enemy_pkmn is None:
            enemy_pkmn = []

        idx = -1
        for idx, cur_pkmn in enumerate(enemy_pkmn):
            if cur_state is not None:
                if cur_state.solo_pkmn.cur_stats.speed > cur_pkmn.cur_stats.speed:
                    speed_style = "Success"
                elif cur_state.solo_pkmn.cur_stats.speed == cur_pkmn.cur_stats.speed:
                    speed_style = "Warning"
                else:
                    speed_style = "Failure"
                cur_state = cur_state.defeat_pkmn(cur_pkmn)[0]
            else:
                speed_style = "Contrast"

            self._all_pkmn[idx].set_pkmn(cur_pkmn, speed_style=speed_style)
            self._all_pkmn[idx].setVisible(True)
            # Place in grid: 3 columns per row
            row = idx // 3
            col = idx % 3
            self._layout.addWidget(self._all_pkmn[idx], row, col)

        for missing_idx in range(idx + 1, 6):
            self._all_pkmn[missing_idx].setVisible(False)
