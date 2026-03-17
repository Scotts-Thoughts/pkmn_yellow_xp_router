import logging

from PySide6.QtWidgets import (
    QWidget, QLabel, QGridLayout,
)
from PySide6.QtCore import Qt

from gui_qt.pkmn_components.inventory_viewer import InventoryViewer
from gui_qt.pkmn_components.pkmn_viewer import PkmnViewer
from gui_qt.pkmn_components.stat_exp_viewer import StatExpViewer

from routing import full_route_state
from utils.config_manager import config

logger = logging.getLogger(__name__)


class StateViewer(QWidget):
    def __init__(self, parent=None):
        super().__init__(parent)

        layout = QGridLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(0)

        # Solo Pokemon stats at the top - expand horizontally to fill available space
        self.pkmn = PkmnViewer(self, font_size=12)
        layout.addWidget(self.pkmn, 0, 0, alignment=Qt.AlignTop)

        # Stat experience underneath the main stats - expand horizontally
        self.stat_xp = StatExpViewer(self)
        layout.addWidget(self.stat_xp, 1, 0, alignment=Qt.AlignTop)

        # Badge boost note in the main content area
        self._badge_boost_label = QLabel("Stats with * are calculated with a badge boost")
        self._badge_boost_label.setStyleSheet(f"color: {config.get_contrast_color()};")
        layout.addWidget(self._badge_boost_label, 2, 0, alignment=Qt.AlignLeft)

        # Inventory gets more space (spans all rows) - expand to fill
        self.inventory = InventoryViewer(self)
        layout.addWidget(self.inventory, 0, 1, 3, 1)

        # Configure columns: both columns expand to fill horizontal space
        layout.setColumnStretch(0, 1)  # Stats column expands
        layout.setColumnStretch(1, 2)  # Inventory gets more space

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
