from PySide6.QtWidgets import (
    QVBoxLayout, QHBoxLayout, QLabel, QWidget, QPushButton, QCheckBox,
)
from PySide6.QtCore import Qt

from gui_qt.dialogs.base_dialog import BaseDialog
from gui_qt.components.custom_components import SimpleButton, ConfigColorUpdater
from utils.config_manager import config


_FIGHT_CATEGORY_LABELS = {
    "rival": "Rival:",
    "gym_leader": "Gym Leader:",
    "elite_four": "Elite Four:",
    "champion": "Champion:",
    "post_game": "Post-Game:",
    "boss": "Boss:",
    "team_leader": "Team Leader:",
}


class HighlightColorConfigDialog(BaseDialog):
    """Dialog for configuring highlight and fight category colors in the route list."""

    def __init__(self, parent, **kwargs):
        super().__init__(parent, title="Configure Highlight Colors", **kwargs)
        self._main_window_ref = parent

        main_layout = QVBoxLayout(self)

        # Color frame
        color_frame = QWidget()
        color_layout = QVBoxLayout(color_frame)
        color_layout.setContentsMargins(20, 20, 20, 20)

        color_header = QLabel("Highlight Color Configuration:")
        color_header.setStyleSheet("font-size: 12pt; font-weight: bold;")
        color_header.setAlignment(Qt.AlignCenter)
        color_layout.addWidget(color_header)

        # Create color updaters for each highlight (1-9)
        self._highlight_color_updaters = []
        for i in range(1, 10):
            label_text = f"Highlight {i}:"
            getter = lambda idx=i: config.get_highlight_color(idx)
            setter = lambda color, idx=i: self._set_highlight_color(idx, color)

            color_updater = ConfigColorUpdater(
                label_text=label_text,
                setter=setter,
                getter=getter,
                callback=self.raise_,
            )
            color_layout.addWidget(color_updater)
            self._highlight_color_updaters.append(color_updater)

        # Fight category colors section
        cat_header = QLabel("Fight Category Colors:")
        cat_header.setStyleSheet("font-size: 12pt; font-weight: bold;")
        cat_header.setAlignment(Qt.AlignCenter)
        color_layout.addWidget(cat_header)

        # Toggle for coloring major battles
        toggle_row = QWidget()
        toggle_layout = QHBoxLayout(toggle_row)
        toggle_layout.setContentsMargins(0, 0, 0, 0)
        self._color_major_battles_cb = QCheckBox("Color Major Battles")
        self._color_major_battles_cb.setChecked(config.get_color_major_battles())
        self._color_major_battles_cb.stateChanged.connect(self._on_color_major_battles_toggled)
        toggle_layout.addWidget(self._color_major_battles_cb)
        toggle_layout.addStretch(1)
        color_layout.addWidget(toggle_row)

        self._category_color_updaters = []
        for cat, label_text in _FIGHT_CATEGORY_LABELS.items():
            getter = lambda c=cat: config.get_fight_category_color(c)
            setter = lambda color, c=cat: self._set_category_color(c, color)

            color_updater = ConfigColorUpdater(
                label_text=label_text,
                setter=setter,
                getter=getter,
                callback=self.raise_,
            )
            color_layout.addWidget(color_updater)
            self._category_color_updaters.append(color_updater)

        # Set initial enabled state of category color pickers
        self._update_category_enabled()

        main_layout.addWidget(color_frame)

        # Buttons
        button_widget = QWidget()
        btn_layout = QHBoxLayout(button_widget)
        btn_layout.setContentsMargins(0, 15, 0, 0)

        self._reset_button = QPushButton("Reset to Defaults")
        self._reset_button.clicked.connect(self._reset_to_defaults)
        btn_layout.addWidget(self._reset_button)

        self._close_button = SimpleButton("Close")
        self._close_button.clicked.connect(self.close)
        btn_layout.addWidget(self._close_button)

        main_layout.addWidget(button_widget)

    def _set_highlight_color(self, highlight_num, color):
        """Set highlight color and refresh the event list."""
        config.set_highlight_color(highlight_num, color)
        self._refresh_event_list()

    def _set_category_color(self, category, color):
        """Set fight category color and refresh the event list."""
        config.set_fight_category_color(category, color)
        self._refresh_event_list()

    def _on_color_major_battles_toggled(self, state):
        config.set_color_major_battles(state == Qt.Checked.value)
        self._update_category_enabled()
        self._refresh_event_list()

    def _update_category_enabled(self):
        enabled = config.get_color_major_battles()
        for updater in self._category_color_updaters:
            updater.setEnabled(enabled)

    def _refresh_event_list(self):
        if hasattr(self._main_window_ref, 'event_list'):
            self._main_window_ref.event_list._update_highlight_colors()
            self._main_window_ref.event_list.refresh()

    def _reset_to_defaults(self):
        """Reset all highlight and category colors to defaults."""
        default_colors = {
            1: "#5a3a7a", 2: "#8b4513", 3: "#1e4a72",
            4: "#2d5a3d", 5: "#8b1a1a", 6: "#4a4a00",
            7: "#006060", 8: "#4a2060", 9: "#605030",
        }
        for i in range(1, 10):
            config.set_highlight_color(i, default_colors[i])

        for cat, default_color in config.FIGHT_CATEGORY_COLOR_DEFAULTS.items():
            config.set_fight_category_color(cat, default_color)

        # Refresh all color updaters
        for updater in self._highlight_color_updaters:
            updater.refresh_color()
        for updater in self._category_color_updaters:
            updater.refresh_color()

        self._refresh_event_list()
