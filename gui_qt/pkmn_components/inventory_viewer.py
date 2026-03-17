import logging
from typing import List

from PySide6.QtWidgets import (
    QWidget, QLabel, QGridLayout,
)
from PySide6.QtCore import Qt

from routing import state_objects
from utils.config_manager import config

logger = logging.getLogger(__name__)


class InventoryViewer(QWidget):
    def __init__(self, parent=None, style_prefix="Inventory"):
        super().__init__(parent)
        self.setMinimumHeight(150)
        self.setMinimumWidth(250)

        layout = QGridLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(2)

        self._money_label = QLabel("Current Money: ")
        self._money_label.setStyleSheet(f"color: {config.get_header_color()};")
        self._money_label.setContentsMargins(0, 2, 0, 2)
        layout.addWidget(self._money_label, 0, 0, 1, 2)

        self._all_items: List[QLabel] = []

        # HARDCODED for now: only support showing 20 items...
        self.max_render_size = 20
        split_point = self.max_render_size // 2
        inv_color = config.get_secondary_color()
        for i in range(self.max_render_size):
            cur_item_label = QLabel(f"# {i:0>2}: ")
            cur_item_label.setAlignment(Qt.AlignLeft | Qt.AlignVCenter)
            cur_item_label.setMinimumWidth(160)
            cur_item_label.setStyleSheet(f"color: {inv_color};")
            layout.addWidget(cur_item_label, (i % split_point) + 1, i // split_point, alignment=Qt.AlignLeft)
            self._all_items.append(cur_item_label)

    def set_inventory(self, inventory: state_objects.Inventory):
        self._money_label.setText(f"Current Money: {inventory.cur_money}")

        idx = -1
        too_many_items = len(inventory.cur_items) > self.max_render_size

        for idx in range(min(len(inventory.cur_items), self.max_render_size)):
            cur_item = inventory.cur_items[idx]
            if too_many_items and idx == (self.max_render_size - 1):
                self._all_items[idx].setText(f"# {idx:0>2}+: More items...")
            else:
                self._all_items[idx].setText(f"# {idx:0>2}: {cur_item.num}x {cur_item.base_item.name}")

        for missing_idx in range(idx + 1, self.max_render_size):
            self._all_items[missing_idx].setText(f"# {missing_idx:0>2}:")
