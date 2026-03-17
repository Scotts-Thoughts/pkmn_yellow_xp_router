from PySide6.QtWidgets import (
    QVBoxLayout, QHBoxLayout, QLabel, QWidget, QPushButton,
)
from PySide6.QtCore import Qt

from gui_qt.dialogs.base_dialog import BaseDialog
from gui_qt.components.custom_components import SimpleButton, ConfigColorUpdater
from utils.config_manager import config


class HighlightColorConfigDialog(BaseDialog):
    """Dialog for configuring the 9 highlight colors used in the route list."""

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
        # Update the color in the route list immediately
        if hasattr(self._main_window_ref, 'event_list'):
            self._main_window_ref.event_list._update_highlight_colors()
            self._main_window_ref.event_list.refresh()

    def _reset_to_defaults(self):
        """Reset all highlight colors to defaults."""
        config.set_highlight_color(1, config.DEFAULT_HIGHLIGHT_COLOR_1)
        config.set_highlight_color(2, config.DEFAULT_HIGHLIGHT_COLOR_2)
        config.set_highlight_color(3, config.DEFAULT_HIGHLIGHT_COLOR_3)
        config.set_highlight_color(4, config.DEFAULT_HIGHLIGHT_COLOR_4)
        config.set_highlight_color(5, config.DEFAULT_HIGHLIGHT_COLOR_5)
        config.set_highlight_color(6, config.DEFAULT_HIGHLIGHT_COLOR_6)
        config.set_highlight_color(7, config.DEFAULT_HIGHLIGHT_COLOR_7)
        config.set_highlight_color(8, config.DEFAULT_HIGHLIGHT_COLOR_8)
        config.set_highlight_color(9, config.DEFAULT_HIGHLIGHT_COLOR_9)

        # Refresh all color updaters
        for updater in self._highlight_color_updaters:
            updater.refresh_color()

        # Update the event list
        if hasattr(self._main_window_ref, 'event_list'):
            self._main_window_ref.event_list._update_highlight_colors()
            self._main_window_ref.event_list.refresh()
