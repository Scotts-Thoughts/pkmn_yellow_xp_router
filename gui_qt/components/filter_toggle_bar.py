import logging
import os

from PySide6.QtWidgets import QWidget, QPushButton, QHBoxLayout, QLineEdit
from PySide6.QtGui import QIcon, QImage, QPixmap
from PySide6.QtCore import Qt, QSize, QTimer

from utils.constants import const
from utils.config_manager import config

logger = logging.getLogger(__name__)

_ICONS_DIR = os.path.join(const.SOURCE_ROOT_PATH, "icons", "filter icons")

# Fallback text labels for filters without an icon file.
_FILTER_SHORT_LABELS = {
    const.TASK_TRAINER_BATTLE:     "Tr",
    const.TASK_LEARN_MOVE_LEVELUP: "Lv",
    const.TASK_SELL_ITEM:          "Se",
    const.TASK_NOTES_ONLY:         "No",
    const.TASK_HOLD_ITEM:          "Hd",
    const.TASK_RARE_CANDY:         "RC",
    const.TASK_FIGHT_WILD_PKMN:    "Wi",
    const.TASK_GET_FREE_ITEM:      "Gt",
    const.TASK_PURCHASE_ITEM:      "Bu",
    const.TASK_USE_ITEM:           "Us",
    const.TASK_VITAMIN:            "Vt",
    const.TASK_SAVE:               "Sv",
    const.TASK_HEAL:               "He",
    const.TASK_BLACKOUT:           "BO",
    const.TASK_EVOLUTION:          "Ev",
    const.TASK_LEARN_MOVE_TM:      "TM",
    const.ERROR_SEARCH:            "Er",
}

# Icon file names (without extension), keyed by constant value.
# Only entries with a corresponding .png in the icons/filter icons folder.
_FILTER_ICON_FILES = {
    const.TASK_TRAINER_BATTLE:     "TASK_TRAINER_BATTLE",
    const.TASK_LEARN_MOVE_LEVELUP: "TASK_LEARN_MOVE_LEVELUP",
    const.TASK_SELL_ITEM:          "TASK_SELL_ITEM",
    const.TASK_NOTES_ONLY:         "TASK_NOTES_ONLY",
    const.TASK_HOLD_ITEM:          "TASK_HOLD_ITEM",
    const.TASK_RARE_CANDY:         "TASK_RARE_CANDY",
    const.TASK_FIGHT_WILD_PKMN:    "TASK_FIGHT_WILD_PKMN",
    const.TASK_GET_FREE_ITEM:      "TASK_GET_FREE_ITEM",
    const.TASK_PURCHASE_ITEM:      "TASK_PURCHASE_ITEM",
    const.TASK_USE_ITEM:           "TASK_USE_ITEM",
    const.TASK_VITAMIN:            "TASK_VITAMIN",
    const.TASK_HEAL:               "TASK_HEAL",
    const.TASK_BLACKOUT:           "TASK_BLACKOUT",
    const.TASK_EVOLUTION:          "TASK_EVOLUTION",
    const.TASK_LEARN_MOVE_TM:      "TASK_LEARN_MOVE_TM",
    const.ERROR_SEARCH:            "ERROR_SEARCH",
}

# Tooltip labels for hover context.
_FILTER_TOOLTIPS = {
    const.TASK_TRAINER_BATTLE:     "Trainer Battle",
    const.TASK_LEARN_MOVE_LEVELUP: "Level Up Move",
    const.TASK_SELL_ITEM:          "Sell Item",
    const.TASK_NOTES_ONLY:         "Notes Only",
    const.TASK_HOLD_ITEM:          "Hold Item",
    const.TASK_RARE_CANDY:         "Rare Candy",
    const.TASK_FIGHT_WILD_PKMN:    "Wild Pkmn",
    const.TASK_GET_FREE_ITEM:      "Get Free Item",
    const.TASK_PURCHASE_ITEM:      "Purchase Item",
    const.TASK_USE_ITEM:           "Use / Drop Item",
    const.TASK_VITAMIN:            "Vitamin",
    const.TASK_SAVE:               "Save",
    const.TASK_HEAL:               "Heal",
    const.TASK_BLACKOUT:           "Blackout",
    const.TASK_EVOLUTION:          "Evolution",
    const.TASK_LEARN_MOVE_TM:      "TM / HM",
    const.ERROR_SEARCH:            "Invalid Events",
}

# Map event type constants to shortcut action IDs in config_manager.
_FILTER_SHORTCUT_IDS = {
    const.TASK_TRAINER_BATTLE:     "filter_trainer",
    const.TASK_RARE_CANDY:         "filter_rare_candy",
    const.TASK_LEARN_MOVE_TM:      "filter_tm_hm",
    const.TASK_VITAMIN:            "filter_vitamin",
    const.TASK_FIGHT_WILD_PKMN:    "filter_wild_pkmn",
    const.TASK_GET_FREE_ITEM:      "filter_acquire_item",
    const.TASK_PURCHASE_ITEM:      "filter_purchase_item",
    const.TASK_USE_ITEM:           "filter_use_item",
    const.TASK_SELL_ITEM:          "filter_sell_item",
    const.TASK_HOLD_ITEM:          "filter_hold_item",
    const.TASK_LEARN_MOVE_LEVELUP: "filter_levelup_move",
    const.TASK_SAVE:               "filter_save",
    const.TASK_HEAL:               "filter_heal",
    const.TASK_BLACKOUT:           "filter_blackout",
    const.TASK_EVOLUTION:          "filter_evolution",
    const.TASK_NOTES_ONLY:         "filter_notes",
}

_ICON_SIZE = 20


class FilterToggleBar(QWidget):
    """Horizontal row of small toggle buttons for route event filters.

    Sits above the event list so filters are always one click away.
    Each button is checkable; checked = that filter type is active.
    Uses icons when available, falls back to short text labels.
    """

    def __init__(self, controller, parent=None):
        super().__init__(parent)
        self._controller = controller

        layout = QHBoxLayout(self)
        layout.setContentsMargins(4, 2, 4, 2)
        layout.setSpacing(2)

        self._buttons = {}  # event_type -> QPushButton

        # Custom display order for the toggle bar.
        toggle_order = [
            const.TASK_TRAINER_BATTLE,
            const.TASK_RARE_CANDY,
            const.TASK_VITAMIN,
            const.TASK_LEARN_MOVE_LEVELUP,
            const.TASK_LEARN_MOVE_TM,
            const.TASK_FIGHT_WILD_PKMN,
            const.TASK_HOLD_ITEM,
            const.TASK_GET_FREE_ITEM,
            const.TASK_PURCHASE_ITEM,
            const.TASK_SELL_ITEM,
            const.TASK_USE_ITEM,
            const.TASK_BLACKOUT,
            const.TASK_HEAL,
            const.TASK_EVOLUTION,
            const.TASK_NOTES_ONLY,
            const.ERROR_SEARCH,
        ]

        for event_type in toggle_order:
            tooltip = _FILTER_TOOLTIPS.get(event_type, event_type)
            action_id = _FILTER_SHORTCUT_IDS.get(event_type)
            if action_id:
                key = config.get_shortcut(action_id)
                if key:
                    tooltip = f"{tooltip} ({key})"

            btn = QPushButton()
            btn.setCheckable(True)
            btn.setFocusPolicy(Qt.NoFocus)
            btn.setToolTip(tooltip)

            # Try to load an icon (greyscale); fall back to text label
            icon_name = _FILTER_ICON_FILES.get(event_type)
            if icon_name:
                icon_path = os.path.join(_ICONS_DIR, f"{icon_name}.png")
                if os.path.isfile(icon_path):
                    img = QImage(icon_path)
                    grey = img.convertToFormat(QImage.Format_Grayscale8)
                    # Preserve alpha by compositing greyscale back with original alpha
                    if img.hasAlphaChannel():
                        grey = grey.convertToFormat(QImage.Format_ARGB32)
                        for y in range(img.height()):
                            for x in range(img.width()):
                                alpha = img.pixelColor(x, y).alpha()
                                c = grey.pixelColor(x, y)
                                c.setAlpha(alpha)
                                grey.setPixelColor(x, y, c)
                    btn.setIcon(QIcon(QPixmap.fromImage(grey)))
                    btn.setIconSize(QSize(_ICON_SIZE, _ICON_SIZE))
                else:
                    btn.setText(_FILTER_SHORT_LABELS.get(event_type, event_type[:2]))
            else:
                btn.setText(_FILTER_SHORT_LABELS.get(event_type, event_type[:2]))

            btn.setStyleSheet(
                "QPushButton { padding: 2px; }"
                "QPushButton:checked { background-color: #3a7bd5; }"
            )
            btn.clicked.connect(
                lambda checked, et=event_type: self._on_toggle(et, checked)
            )
            layout.addWidget(btn)
            self._buttons[event_type] = btn

        # Reset button at the end
        reset_btn = QPushButton("Clear Filters")
        reset_btn.setFocusPolicy(Qt.NoFocus)
        reset_btn.setStyleSheet(
            "QPushButton { padding: 2px 4px; font-size: 11px; }"
        )
        reset_btn.clicked.connect(self._reset_all)
        layout.addWidget(reset_btn)

        # Text search box to the right of Clear Filters
        self._search_timer = QTimer(self)
        self._search_timer.setSingleShot(True)
        self._search_timer.setInterval(300)
        self._search_timer.timeout.connect(self._delayed_search_callback)

        self._search_entry = QLineEdit(self)
        self._search_entry.setPlaceholderText("Search events...")
        self._search_entry.setClearButtonEnabled(True)
        self._search_entry.setMaximumWidth(200)
        self._search_entry.textChanged.connect(self._on_search_text_changed)
        layout.addWidget(self._search_entry)

        layout.addStretch()

    # ------------------------------------------------------------------
    # Filter toggling
    # ------------------------------------------------------------------

    def _on_toggle(self, event_type, checked):
        current = self._controller.get_route_filter_types()
        current = list(current) if current else []

        if checked and event_type not in current:
            current.append(event_type)
        elif not checked and event_type in current:
            current.remove(event_type)

        self._controller.set_route_filter_types(current)

    def _reset_all(self):
        self._controller.set_route_filter_types([])
        self.sync()

    # ------------------------------------------------------------------
    # Text search
    # ------------------------------------------------------------------

    def _on_search_text_changed(self, _text):
        self._search_timer.stop()
        self._search_timer.start()

    def _delayed_search_callback(self):
        self._controller.set_route_search(self._search_entry.text())

    # ------------------------------------------------------------------
    # State synchronization
    # ------------------------------------------------------------------

    def sync(self):
        """Update button checked states from the controller's current filters."""
        active = self._controller.get_route_filter_types() or []
        for event_type, btn in self._buttons.items():
            btn.setChecked(event_type in active)
