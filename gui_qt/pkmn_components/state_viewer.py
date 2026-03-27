import logging

from PySide6.QtWidgets import (
    QWidget, QLabel, QGridLayout, QVBoxLayout,
)
from PySide6.QtCore import Qt

from gui_qt.pkmn_components.inventory_viewer import InventoryViewer
from gui_qt.pkmn_components.pkmn_viewer import PkmnViewer
from gui_qt.pkmn_components.stat_exp_viewer import StatExpViewer
from gui_qt.pkmn_components.stat_column import RoundedSection

from routing import full_route_state
from utils.config_manager import config

logger = logging.getLogger(__name__)


def _lighten_color(hex_color, amount):
    h = hex_color.lstrip('#')
    rgb = tuple(int(h[i:i+2], 16) for i in (0, 2, 4))
    lightened = tuple(min(255, int(c + (255 - c) * amount)) for c in rgb)
    return f"#{lightened[0]:02x}{lightened[1]:02x}{lightened[2]:02x}"


def _make_section(object_name, parent=None):
    """Create a styled section with subtle background and rounded corners."""
    section = RoundedSection(parent)
    section.setObjectName(object_name)
    bg = _lighten_color(config.get_background_color(), 0.06)
    section.setStyleSheet(
        f"#{object_name} {{"
        f"  background-color: {bg};"
        f"  border-radius: 6px;"
        f"}}"
    )
    return section


class StateViewer(QWidget):
    def __init__(self, parent=None):
        super().__init__(parent)

        layout = QGridLayout(self)
        layout.setContentsMargins(4, 4, 4, 4)
        layout.setSpacing(6)

        # ---- Pokemon Stats Section ----
        pkmn_section = _make_section("statePkmnSection", self)
        pkmn_inner = QVBoxLayout(pkmn_section)
        pkmn_inner.setContentsMargins(8, 6, 8, 6)
        pkmn_inner.setSpacing(0)

        self.pkmn = PkmnViewer(pkmn_section, font_size=12)
        pkmn_inner.addWidget(self.pkmn)

        layout.addWidget(pkmn_section, 0, 0, alignment=Qt.AlignTop)

        # ---- Stat Experience Section ----
        stat_section = _make_section("stateStatXpSection", self)
        stat_inner = QVBoxLayout(stat_section)
        stat_inner.setContentsMargins(8, 6, 8, 6)
        stat_inner.setSpacing(4)

        self.stat_xp = StatExpViewer(stat_section)
        stat_inner.addWidget(self.stat_xp)

        self._badge_boost_label = QLabel("Stats with * are calculated with a badge boost")
        self._badge_boost_label.setStyleSheet(
            f"color: {config.get_contrast_color()}; font-style: italic;"
        )
        stat_inner.addWidget(self._badge_boost_label, alignment=Qt.AlignLeft)

        layout.addWidget(stat_section, 1, 0, alignment=Qt.AlignTop)

        # ---- Inventory Section ----
        inv_section = _make_section("stateInvSection", self)
        inv_inner = QVBoxLayout(inv_section)
        inv_inner.setContentsMargins(8, 6, 8, 6)
        inv_inner.setSpacing(0)

        self.inventory = InventoryViewer(inv_section)
        self.inventory.setAutoFillBackground(False)
        inv_inner.addWidget(self.inventory)

        layout.addWidget(inv_section, 0, 1, 2, 1, alignment=Qt.AlignTop)

        # Column stretches
        layout.setColumnStretch(0, 3)
        layout.setColumnStretch(1, 2)

    def set_state(self, cur_state: full_route_state.RouteState):
        if cur_state is None:
            # Handle None state - clear all displays
            from routing import state_objects
            empty_inventory = state_objects.Inventory()
            self.inventory.set_inventory(empty_inventory)
            # Clear stat display
            self.stat_xp.set_state(None)
            # For pkmn, we can't easily create an empty one, so just skip updating it
            # The display will remain showing the last valid state, which is acceptable
            return

        self.inventory.set_inventory(cur_state.inventory)
        self.pkmn.set_pkmn(cur_state.solo_pkmn.get_pkmn_obj(cur_state.badges), cur_state.badges)
        self.stat_xp.set_state(cur_state)
