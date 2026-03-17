from PySide6.QtWidgets import (
    QVBoxLayout, QGridLayout, QLabel, QWidget, QPushButton,
)
from PySide6.QtCore import Qt
from PySide6.QtGui import QFontDatabase

from gui_qt.dialogs.base_dialog import BaseDialog
from gui_qt.components.custom_components import (
    SimpleButton, SimpleOptionMenu, ConfigColorUpdater,
)
from utils.config_manager import config


class ColorConfigDialog(BaseDialog):
    """Dialog for configuring fonts and application colors."""

    def __init__(self, parent, **kwargs):
        super().__init__(parent, title="Font & Color Configuration", **kwargs)

        main_layout = QVBoxLayout(self)

        # === Font Configuration Section ===
        font_frame = QWidget()
        font_grid = QGridLayout(font_frame)
        font_grid.setContentsMargins(5, 5, 5, 5)
        font_grid.setHorizontalSpacing(5)
        font_grid.setVerticalSpacing(3)

        font_name_label = QLabel("Font Name:")
        font_grid.addWidget(font_name_label, 3, 0)

        available_fonts = sorted(QFontDatabase.families())
        self._font_name = SimpleOptionMenu(option_list=available_fonts)
        self._font_name.setMinimumWidth(200)
        custom_font_name = config.get_custom_font_name()
        if custom_font_name not in available_fonts:
            custom_font_name = config.DEFAULT_FONT_NAME
        self._font_name.set(custom_font_name)
        font_grid.addWidget(self._font_name, 3, 1)

        font_name_button = QPushButton("Set Font Name")
        font_name_button.clicked.connect(self.set_font_name)
        font_grid.addWidget(font_name_button, 4, 0, 1, 2)

        font_warning = QLabel(
            "If your custom font is not present in the list\n"
            "Make sure that it is installed on your system\n"
            "And then restart the program"
        )
        font_warning.setAlignment(Qt.AlignCenter)
        font_grid.addWidget(font_warning, 5, 0, 1, 2)

        main_layout.addWidget(font_frame)

        # === Color Configuration Section ===
        color_frame = QWidget()
        color_layout = QVBoxLayout(color_frame)
        color_layout.setContentsMargins(5, 5, 5, 5)

        color_header = QLabel("Color Config:")
        color_layout.addWidget(color_header)

        reset_colors_button = QPushButton("Reset all colors")
        reset_colors_button.clicked.connect(self._reset_all_colors)
        color_layout.addWidget(reset_colors_button)

        self._success_color = ConfigColorUpdater(
            label_text="Success Color:",
            setter=config.set_success_color,
            getter=config.get_success_color,
            callback=self.raise_,
        )
        color_layout.addWidget(self._success_color)

        self._warning_color = ConfigColorUpdater(
            label_text="Warning Color:",
            setter=config.set_warning_color,
            getter=config.get_warning_color,
            callback=self.raise_,
        )
        color_layout.addWidget(self._warning_color)

        self._failure_color = ConfigColorUpdater(
            label_text="Failure Color:",
            setter=config.set_failure_color,
            getter=config.get_failure_color,
            callback=self.raise_,
        )
        color_layout.addWidget(self._failure_color)

        self._divider_color = ConfigColorUpdater(
            label_text="Divider Color:",
            setter=config.set_divider_color,
            getter=config.get_divider_color,
            callback=self.raise_,
        )
        color_layout.addWidget(self._divider_color)

        self._header_color = ConfigColorUpdater(
            label_text="Header Color:",
            setter=config.set_header_color,
            getter=config.get_header_color,
            callback=self.raise_,
        )
        color_layout.addWidget(self._header_color)

        self._primary_color = ConfigColorUpdater(
            label_text="Primary Color:",
            setter=config.set_primary_color,
            getter=config.get_primary_color,
            callback=self.raise_,
        )
        color_layout.addWidget(self._primary_color)

        self._secondary_color = ConfigColorUpdater(
            label_text="Secondary Color:",
            setter=config.set_secondary_color,
            getter=config.get_secondary_color,
            callback=self.raise_,
        )
        color_layout.addWidget(self._secondary_color)

        self._contrast_color = ConfigColorUpdater(
            label_text="Contrast Color:",
            setter=config.set_contrast_color,
            getter=config.get_contrast_color,
            callback=self.raise_,
        )
        color_layout.addWidget(self._contrast_color)

        self._background_color = ConfigColorUpdater(
            label_text="Background Color:",
            setter=config.set_background_color,
            getter=config.get_background_color,
            callback=self.raise_,
        )
        color_layout.addWidget(self._background_color)

        self._text_color = ConfigColorUpdater(
            label_text="Text Color:",
            setter=config.set_text_color,
            getter=config.get_text_color,
            callback=self.raise_,
        )
        color_layout.addWidget(self._text_color)

        restart_label = QLabel(
            "After changing colors, you must restart the program\n"
            "before color changes will take effect"
        )
        restart_label.setAlignment(Qt.AlignCenter)
        color_layout.addWidget(restart_label)

        main_layout.addWidget(color_frame)

        # Close button
        self._close_button = SimpleButton("Close")
        self._close_button.clicked.connect(self.close)
        main_layout.addWidget(self._close_button, alignment=Qt.AlignCenter)

    def _reset_all_colors(self, *args, **kwargs):
        config.reset_all_colors()
        self._success_color.refresh_color()
        self._warning_color.refresh_color()
        self._failure_color.refresh_color()
        self._divider_color.refresh_color()
        self._header_color.refresh_color()
        self._primary_color.refresh_color()
        self._secondary_color.refresh_color()
        self._contrast_color.refresh_color()
        self._background_color.refresh_color()
        self._text_color.refresh_color()

    def set_font_name(self, *args, **kwargs):
        config.set_custom_font_name(self._font_name.get())
        if self._main_window is not None and hasattr(self._main_window, 'load_custom_font'):
            self._main_window.load_custom_font()
