import logging
from typing import List

from PySide6.QtWidgets import (
    QWidget, QGridLayout, QVBoxLayout,
)
from PySide6.QtCore import Qt

from gui_qt.pkmn_components.pkmn_viewer import PkmnViewer
from gui_qt.pkmn_components.stat_column import RoundedSection

from pkmn import universal_data_objects
from routing import full_route_state
from utils.config_manager import config

logger = logging.getLogger(__name__)


def _lighten_color(hex_color, amount):
    h = hex_color.lstrip('#')
    rgb = tuple(int(h[i:i+2], 16) for i in (0, 2, 4))
    lightened = tuple(min(255, int(c + (255 - c) * amount)) for c in rgb)
    return f"#{lightened[0]:02x}{lightened[1]:02x}{lightened[2]:02x}"


class EnemyPkmnTeam(QWidget):
    def __init__(self, parent=None):
        super().__init__(parent)

        self._layout = QGridLayout(self)
        self._layout.setContentsMargins(0, 0, 0, 0)
        self._layout.setSpacing(6)

        self._all_pkmn: List[PkmnViewer] = []
        self._all_frames: List[RoundedSection] = []

        bg = _lighten_color(config.get_background_color(), 0.06)
        for i in range(6):
            section = RoundedSection(self)
            obj_name = f"enemyPkmn{i}"
            section.setObjectName(obj_name)
            section.setStyleSheet(
                f"#{obj_name} {{"
                f"  background-color: {bg};"
                f"  border-radius: 6px;"
                f"}}"
            )
            section_layout = QVBoxLayout(section)
            section_layout.setContentsMargins(8, 6, 8, 6)
            section_layout.setSpacing(0)

            viewer = PkmnViewer(section, font_size=10)
            section_layout.addWidget(viewer)

            section.setVisible(False)
            self._all_pkmn.append(viewer)
            self._all_frames.append(section)

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
            self._all_frames[idx].setVisible(True)
            # Place in grid: 3 columns per row
            row = idx // 3
            col = idx % 3
            self._layout.addWidget(self._all_frames[idx], row, col)

        for missing_idx in range(idx + 1, 6):
            self._all_frames[missing_idx].setVisible(False)
