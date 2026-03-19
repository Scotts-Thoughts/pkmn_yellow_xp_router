import logging
from typing import List, Optional

from PySide6.QtWidgets import (
    QWidget, QLabel, QVBoxLayout, QHBoxLayout, QSizePolicy,
)
from PySide6.QtCore import Qt
from PySide6.QtGui import QPalette, QColor

from utils.config_manager import config

logger = logging.getLogger(__name__)


_STYLE_COLOR_MAP = {
    "Primary": "get_primary_color",
    "Secondary": "get_secondary_color",
    "Header": "get_header_color",
    "Success": "get_success_color",
    "Warning": "get_warning_color",
    "Failure": "get_failure_color",
    "Contrast": "get_contrast_color",
    "Divider": "get_divider_color",
}


def _color_for_style(style_prefix: str) -> str:
    getter = _STYLE_COLOR_MAP.get(style_prefix)
    if getter:
        return getattr(config, getter)()
    return config.get_text_color()


def tinted_bg_for_style(style_prefix: str, alpha=0.15) -> str:
    """Mix the style color into the base background for a subtle tinted panel."""
    fg_hex = _color_for_style(style_prefix).lstrip('#')
    bg_hex = config.get_background_color().lstrip('#')

    fg_rgb = tuple(int(fg_hex[i:i+2], 16) for i in (0, 2, 4))
    bg_rgb = tuple(int(bg_hex[i:i+2], 16) for i in (0, 2, 4))

    mixed = tuple(int(b + (f - b) * alpha) for f, b in zip(fg_rgb, bg_rgb))
    return f"#{mixed[0]:02x}{mixed[1]:02x}{mixed[2]:02x}"


class StatColumn(QWidget):
    def __init__(
        self,
        parent=None,
        num_rows=4,
        label_width=None,
        val_width=None,
        font=None,
        style_prefix="Primary",
    ):
        super().__init__(parent)

        self._style_prefix = style_prefix
        self._label_width = label_width
        self._val_width = val_width

        # Subtle tinted background for visual section grouping
        bg_tint = tinted_bg_for_style(style_prefix)
        self.setAutoFillBackground(True)
        pal = self.palette()
        pal.setColor(QPalette.Window, QColor(bg_tint))
        self.setPalette(pal)

        main_layout = QVBoxLayout(self)
        main_layout.setContentsMargins(4, 2, 4, 2)
        main_layout.setSpacing(0)

        # Header
        self._header = QLabel("")
        self._header.setAlignment(Qt.AlignCenter)
        self._header.setStyleSheet(f"color: {_color_for_style(style_prefix)};")
        self._header.setVisible(False)
        main_layout.addWidget(self._header)

        self._frames: List[QWidget] = []
        self._labels: List[QLabel] = []
        self._values: List[QLabel] = []

        base_color = _color_for_style(style_prefix)

        for idx in range(num_rows):
            row_widget = QWidget()
            row_layout = QHBoxLayout(row_widget)
            row_layout.setContentsMargins(0, 2, 0, 0)
            row_layout.setSpacing(2)

            cur_label = QLabel("")
            cur_label.setAlignment(Qt.AlignLeft | Qt.AlignVCenter)
            cur_label.setStyleSheet(f"color: {base_color};")
            if label_width is not None:
                cur_label.setMinimumWidth(label_width * 8)
            if font is not None:
                cur_label.setFont(font)

            cur_value = QLabel("")
            cur_value.setAlignment(Qt.AlignRight | Qt.AlignVCenter)
            cur_value.setStyleSheet(f"color: {base_color};")
            if val_width is not None:
                cur_value.setMinimumWidth(val_width * 8)
            if font is not None:
                cur_value.setFont(font)

            row_layout.addWidget(cur_label)
            row_layout.addWidget(cur_value, 1)

            main_layout.addWidget(row_widget)

            self._frames.append(row_widget)
            self._labels.append(cur_label)
            self._values.append(cur_value)

    def set_header(self, header):
        if header is None or header == "":
            self._header.setVisible(False)
            return

        self._header.setVisible(True)
        self._header.setText(header)

    def set_labels(self, label_text_iterable):
        for idx, cur_label_text in enumerate(label_text_iterable):
            if idx >= len(self._labels):
                break
            self._labels[idx].setText(cur_label_text)

        if len(label_text_iterable) < len(self._labels):
            for idx in range(len(label_text_iterable), len(self._labels)):
                self._labels[idx].setText("")

    def set_values(self, value_text_iterable, style_iterable=None):
        if style_iterable is None:
            style_iterable = [self._style_prefix for _ in value_text_iterable]
        else:
            style_iterable = list(style_iterable)
            for idx in range(len(style_iterable)):
                if style_iterable[idx] is None:
                    style_iterable[idx] = self._style_prefix

        for idx, cur_value_text in enumerate(value_text_iterable):
            if idx >= len(self._values):
                break
            color = _color_for_style(style_iterable[idx])
            self._values[idx].setText(str(cur_value_text))
            self._values[idx].setStyleSheet(f"color: {color};")
            self._labels[idx].setStyleSheet(f"color: {color};")

        if len(value_text_iterable) < len(self._values):
            for idx in range(len(value_text_iterable), len(self._values)):
                self._values[idx].setText("")
