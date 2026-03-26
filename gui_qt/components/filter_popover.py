import logging

from PySide6.QtWidgets import QWidget, QPushButton, QGridLayout, QVBoxLayout
from PySide6.QtCore import Qt

from utils.constants import const

logger = logging.getLogger(__name__)

# Short display labels for each filter type, keyed by the constant value.
_FILTER_LABELS = {
    const.TASK_TRAINER_BATTLE:     "Trainer",
    const.TASK_LEARN_MOVE_LEVELUP: "Lvl Move",
    const.TASK_SELL_ITEM:          "Sell",
    const.TASK_NOTES_ONLY:         "Notes",
    const.TASK_HOLD_ITEM:          "Hold",
    const.TASK_RARE_CANDY:         "Candy",
    const.TASK_FIGHT_WILD_PKMN:    "Wild",
    const.TASK_GET_FREE_ITEM:      "Get Item",
    const.TASK_PURCHASE_ITEM:      "Buy",
    const.TASK_USE_ITEM:           "Use/Drop",
    const.TASK_VITAMIN:            "Vitamin",
    const.TASK_SAVE:               "Save",
    const.TASK_HEAL:               "Heal",
    const.TASK_BLACKOUT:           "Blackout",
    const.TASK_EVOLUTION:          "Evolve",
    const.TASK_LEARN_MOVE_TM:      "TM/HM",
    const.ERROR_SEARCH:            "Errors",
}

_COLS = 5


class FilterPopover(QWidget):
    """Floating popover with toggle buttons for route event filters.

    Shown when the user presses F with the event list focused.  Each button
    represents a filter type; checked (highlighted) means the filter is
    active.  The popover stays open so the user can toggle several filters
    before dismissing it with Escape or F.
    """

    def __init__(self, controller, parent=None):
        super().__init__(parent, Qt.Popup | Qt.FramelessWindowHint)
        self._controller = controller
        self.setAttribute(Qt.WA_DeleteOnClose, False)

        self.setObjectName("filterPopover")
        self.setStyleSheet(
            "#filterPopover { border: 1px solid rgba(255, 255, 255, 0.15);"
            " border-radius: 4px; }"
        )

        layout = QVBoxLayout(self)
        layout.setContentsMargins(6, 6, 6, 6)
        layout.setSpacing(4)

        grid = QGridLayout()
        grid.setSpacing(3)

        self._filter_buttons = {}
        for idx, event_type in enumerate(const.ROUTE_EVENT_TYPES):
            row = idx // _COLS
            col = idx % _COLS
            label = _FILTER_LABELS.get(event_type, event_type)

            btn = QPushButton(label)
            btn.setCheckable(True)
            btn.setFocusPolicy(Qt.NoFocus)
            btn.clicked.connect(
                lambda checked, et=event_type: self._toggle_filter(et, checked)
            )
            grid.addWidget(btn, row, col)
            self._filter_buttons[event_type] = btn

        layout.addLayout(grid)

        reset_btn = QPushButton("Reset All")
        reset_btn.setFocusPolicy(Qt.NoFocus)
        reset_btn.clicked.connect(self._reset_all)
        layout.addWidget(reset_btn)

    # ------------------------------------------------------------------
    # Positioning
    # ------------------------------------------------------------------

    def show_above(self, global_pos):
        """Sync button states, position above *global_pos*, and show."""
        self._sync_buttons()
        self.adjustSize()
        x = global_pos.x() - self.width() // 2
        y = global_pos.y() - self.height() - 4
        if y < 0:
            y = global_pos.y() + 22
        self.move(x, y)
        self.show()

    # ------------------------------------------------------------------
    # Filter toggling
    # ------------------------------------------------------------------

    def _sync_buttons(self):
        """Set button checked states from the controller's current filters."""
        active = self._controller.get_route_filter_types() or []
        for event_type, btn in self._filter_buttons.items():
            btn.setChecked(event_type in active)

    def _toggle_filter(self, event_type, checked):
        current = self._controller.get_route_filter_types()
        current = list(current) if current else []

        if checked and event_type not in current:
            current.append(event_type)
        elif not checked and event_type in current:
            current.remove(event_type)

        self._controller.set_route_filter_types(current)

    def _reset_all(self):
        self._controller.set_route_filter_types([])
        self._sync_buttons()

    # ------------------------------------------------------------------
    # Keyboard
    # ------------------------------------------------------------------

    def keyPressEvent(self, event):
        if event.key() in (Qt.Key_Escape, Qt.Key_F):
            self.close()
            return
        super().keyPressEvent(event)
