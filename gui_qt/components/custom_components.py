import os
import logging

from PySide6.QtWidgets import (
    QWidget, QLabel, QPushButton, QLineEdit, QComboBox, QCheckBox,
    QHBoxLayout, QVBoxLayout, QGridLayout, QSpinBox, QPlainTextEdit,
    QColorDialog, QFrame, QSizePolicy,
)
from PySide6.QtCore import Qt, QTimer, QPointF, Signal, QPropertyAnimation, QPoint
from PySide6.QtGui import QBrush, QColor, QPainter, QPen, QPolygonF

from utils.constants import const
from utils.config_manager import config
from utils import io_utils

logger = logging.getLogger(__name__)


class SimpleButton(QPushButton):
    def __init__(self, text="", parent=None, **kwargs):
        super().__init__(text, parent)
        if "width" in kwargs:
            self.setFixedWidth(kwargs["width"])

    def enable(self):
        self.setEnabled(True)

    def disable(self):
        self.setEnabled(False)


class SimpleEntry(QLineEdit):
    value_changed = Signal(str)

    def __init__(self, initial_value="", callback=None, parent=None, **kwargs):
        super().__init__(parent)
        self.setText(initial_value)
        self._callback = callback
        if "width" in kwargs:
            self.setMinimumWidth(kwargs["width"])
        if callback:
            self.textChanged.connect(self._on_text_changed)

    def _on_text_changed(self, text):
        if self._callback:
            self._callback()

    def get(self):
        return self.text()

    def set(self, value):
        self.setText(str(value))

    def enable(self):
        self.setEnabled(True)

    def disable(self):
        self.setEnabled(False)


class SimpleOptionMenu(QComboBox):
    def __init__(self, option_list=None, callback=None, default_val=None, parent=None, **kwargs):
        super().__init__(parent)
        self.setEditable(False)
        self._callback = callback
        self.cur_options = []
        if "width" in kwargs:
            self.setMinimumWidth(kwargs["width"])
        if option_list:
            self.new_values(option_list, default_val)
        if callback:
            self.currentIndexChanged.connect(self._on_index_changed)

    def _on_index_changed(self, index):
        if self._callback:
            self._callback()

    def get(self):
        return self.currentText()

    def set(self, val):
        idx = self.findText(val)
        if idx >= 0:
            self.setCurrentIndex(idx)

    def enable(self):
        self.setEnabled(True)

    def disable(self):
        self.setEnabled(False)

    def new_values(self, option_list, default_val=None):
        if option_list == self.cur_options:
            if default_val is not None:
                self.blockSignals(True)
                self.setCurrentText(default_val)
                self.blockSignals(False)
            return
        self.cur_options = list(option_list)
        self.blockSignals(True)
        self.clear()
        self.addItems(option_list)
        if default_val and default_val in option_list:
            self.setCurrentText(default_val)
        elif option_list:
            self.setCurrentIndex(0)
        self.blockSignals(False)


class CheckboxLabel(QWidget):
    toggled = Signal(bool)

    def __init__(self, text="", toggle_command=None, flip=False, parent=None):
        super().__init__(parent)
        self._checkbox = QCheckBox()
        self._label = QLabel(text)
        self._label.setStyleSheet("background: transparent;")
        self._toggle_command = toggle_command

        layout = QHBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(4)

        if flip:
            self._label.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Preferred)
            layout.addWidget(self._label)
            layout.addWidget(self._checkbox)
        else:
            layout.addWidget(self._checkbox)
            self._label.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Preferred)
            layout.addWidget(self._label)

        self._checkbox.stateChanged.connect(self._on_state_changed)
        self._label.mousePressEvent = lambda e: self.toggle_checked()

    def _on_state_changed(self, state):
        if self._toggle_command:
            self._toggle_command()
        self.toggled.emit(self._checkbox.isChecked())

    def is_checked(self):
        return self._checkbox.isChecked()

    def set_checked(self, val):
        self._checkbox.blockSignals(True)
        self._checkbox.setChecked(bool(val))
        self._checkbox.blockSignals(False)

    def toggle_checked(self):
        self._checkbox.setChecked(not self._checkbox.isChecked())

    def enable(self):
        self._checkbox.setEnabled(True)

    def disable(self):
        self._checkbox.setEnabled(False)

    def text(self):
        return self._label.text()


class AmountEntry(QWidget):
    value_changed = Signal(str)

    def __init__(self, callback=None, max_val=None, min_val=None, init_val=None, width=None, parent=None):
        super().__init__(parent)
        self._callback = callback
        self._max_val = max_val
        self._min_val = min_val

        layout = QHBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(1)

        self._down_button = QPushButton("\u2212")  # minus sign
        self._down_button.setProperty("class", "amount-btn")
        self._down_button.setFixedWidth(22)
        self._down_button.setFixedHeight(22)
        self._down_button.setFocusPolicy(Qt.NoFocus)
        self._down_button.clicked.connect(self._lower_amt)

        self._entry = QLineEdit()
        if width:
            self._entry.setFixedWidth(width * 8)
        else:
            self._entry.setFixedWidth(50)
        self._entry.setFixedHeight(22)
        self._entry.setAlignment(Qt.AlignCenter)
        self._entry.textChanged.connect(self._on_text_changed)

        self._up_button = QPushButton("+")
        self._up_button.setProperty("class", "amount-btn")
        self._up_button.setFixedWidth(22)
        self._up_button.setFixedHeight(22)
        self._up_button.setFocusPolicy(Qt.NoFocus)
        self._up_button.clicked.connect(self._raise_amt)

        layout.addWidget(self._down_button)
        layout.addWidget(self._entry)
        layout.addWidget(self._up_button)

        if init_val is not None:
            self._entry.setText(str(init_val))
        elif min_val is not None:
            self._entry.setText(str(min_val))
        else:
            self._entry.setText("1")

        self._update_buttons()

    def _on_text_changed(self, text):
        self._update_buttons()
        if self._callback:
            self._callback()
        self.value_changed.emit(text)

    def _lower_amt(self):
        try:
            val = int(self._entry.text()) - 1
            if self._min_val is not None and val < self._min_val:
                val = self._min_val
            self._entry.setText(str(val))
        except ValueError:
            if self._min_val is not None:
                self._entry.setText(str(self._min_val))

    def _raise_amt(self):
        try:
            val = int(self._entry.text()) + 1
            if self._max_val is not None and val > self._max_val:
                val = self._max_val
            self._entry.setText(str(val))
        except ValueError:
            if self._min_val is not None:
                self._entry.setText(str(self._min_val))

    def _update_buttons(self):
        try:
            val = int(self._entry.text())
            self._down_button.setEnabled(self._min_val is None or val > self._min_val)
            self._up_button.setEnabled(self._max_val is None or val < self._max_val)
        except ValueError:
            self._down_button.setEnabled(True)
            self._up_button.setEnabled(True)

    @property
    def max_val(self):
        return self._max_val

    @max_val.setter
    def max_val(self, value):
        self._max_val = value
        self._update_buttons()

    @property
    def min_val(self):
        return self._min_val

    @min_val.setter
    def min_val(self, value):
        self._min_val = value
        self._update_buttons()

    def get(self):
        return self._entry.text()

    def set(self, value, update_buttons_only=False):
        if not update_buttons_only:
            self._entry.setText(str(value))
        self._update_buttons()

    def enable(self):
        self._down_button.setEnabled(True)
        self._entry.setEnabled(True)
        self._up_button.setEnabled(True)
        self._update_buttons()

    def disable(self):
        self._down_button.setEnabled(False)
        self._entry.setEnabled(False)
        self._up_button.setEnabled(False)


class AutoClearingLabel(QLabel):
    def __init__(self, clear_timeout=3000, parent=None):
        super().__init__(parent)
        self._clear_timeout = clear_timeout
        self._message_id = 0
        self.setVisible(False)

    def set_message(self, value):
        self._message_id += 1
        current_id = self._message_id
        self.setText(value)
        self.setVisible(bool(value))
        QTimer.singleShot(self._clear_timeout, lambda: self._auto_clear(current_id))

    def _auto_clear(self, message_id):
        if message_id == self._message_id:
            self.setText("")
            self.setVisible(False)


class NotificationPopup(QWidget):
    def __init__(self, parent_window):
        super().__init__(parent_window, Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.Tool)
        self._parent_window = parent_window
        self._animation = None
        self._hide_timer = None
        self.setAttribute(Qt.WA_TranslucentBackground, False)
        self.setStyleSheet("""
            NotificationPopup {
                background-color: #4CAF50;
                border-radius: 6px;
            }
            QLabel { color: white; background: transparent; }
            QPushButton {
                color: white;
                background-color: #66BB6A;
                border: 1px solid #81C784;
                border-radius: 3px;
                padding: 4px 10px;
            }
            QPushButton:hover { background-color: #81C784; }
        """)

        self._layout = QVBoxLayout(self)
        self._layout.setContentsMargins(12, 8, 12, 8)

        self._message_label = QLabel()
        self._message_label.setWordWrap(True)
        self._message_label.setMaximumWidth(400)
        self._message_label.setStyleSheet("font-weight: bold; font-size: 11pt;")
        self._layout.addWidget(self._message_label)

        self._folder_button = QPushButton("Open Folder")
        self._folder_button.setVisible(False)
        self._folder_button.clicked.connect(self._open_folder)
        self._layout.addWidget(self._folder_button)

        self._folder_path = None
        self.hide()

    def show_notification(self, message, duration=5000, folder_path=None):
        self._message_label.setText(message)
        self._folder_path = folder_path
        self._folder_button.setVisible(folder_path is not None)

        self.adjustSize()
        self._position_popup()
        self.setWindowOpacity(0.0)
        self.show()
        self.raise_()

        # Fade in
        self._animation = QPropertyAnimation(self, b"windowOpacity")
        self._animation.setDuration(200)
        self._animation.setStartValue(0.0)
        self._animation.setEndValue(1.0)
        self._animation.start()

        # Schedule hide
        if self._hide_timer:
            self._hide_timer.stop()
        self._hide_timer = QTimer(self)
        self._hide_timer.setSingleShot(True)
        self._hide_timer.timeout.connect(self._fade_out)
        self._hide_timer.start(duration)

    def _fade_out(self):
        self._animation = QPropertyAnimation(self, b"windowOpacity")
        self._animation.setDuration(200)
        self._animation.setStartValue(1.0)
        self._animation.setEndValue(0.0)
        self._animation.finished.connect(self.hide)
        self._animation.start()

    def _position_popup(self):
        parent_geo = self._parent_window.geometry()
        x = parent_geo.right() - self.width() - 20
        y = parent_geo.bottom() - self.height() - 20
        self.move(x, y)

    def _open_folder(self):
        if self._folder_path:
            io_utils.open_explorer(self._folder_path)
        self.hide()


class ConfigColorUpdater(QWidget):
    color_changed = Signal(str)

    def __init__(self, label_text, getter, setter, callback=None, parent=None):
        super().__init__(parent)
        self._getter = getter
        self._setter = setter
        self._callback = callback

        layout = QHBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)

        self._label = QLabel(label_text)
        layout.addWidget(self._label, 1)

        right_frame = QHBoxLayout()
        right_frame.setSpacing(6)

        self._button = QPushButton("Change Color")
        self._button.clicked.connect(self._change_color)
        right_frame.addWidget(self._button)

        self._preview = QFrame()
        self._preview.setFixedSize(20, 20)
        self._preview.setFrameShape(QFrame.Box)
        self.refresh_color()
        right_frame.addWidget(self._preview)

        layout.addLayout(right_frame)

    def refresh_color(self):
        color = self._getter()
        self._preview.setStyleSheet(f"background-color: {color}; border: 1px solid #555;")

    def _change_color(self):
        current = QColor(self._getter())
        color = QColorDialog.getColor(current, self, "Select Color")
        if color.isValid():
            hex_color = color.name()
            self._setter(hex_color)
            self.refresh_color()
            self.color_changed.emit(hex_color)
            if self._callback:
                self._callback()


class DisclosureTriangle(QWidget):
    """A small widget that paints a solid triangle pointing down (expanded)
    or right (collapsed).  Both orientations use the exact same triangle
    size so the indicator never appears to shrink or distort."""

    def __init__(self, size=12, color="#cccccc", parent=None):
        super().__init__(parent)
        self._expanded = True
        self._color = QColor(color)
        self.setFixedSize(size, size)

    def set_expanded(self, expanded: bool):
        if self._expanded != expanded:
            self._expanded = expanded
            self.update()

    def paintEvent(self, event):
        painter = QPainter(self)
        painter.setRenderHint(QPainter.Antialiasing)
        painter.setPen(Qt.NoPen)
        painter.setBrush(QBrush(self._color))

        s = min(self.width(), self.height())
        margin = s * 0.15
        tri_size = s - 2 * margin

        if self._expanded:
            # Down-pointing triangle
            poly = QPolygonF([
                QPointF(margin, margin),
                QPointF(margin + tri_size, margin),
                QPointF(margin + tri_size / 2, margin + tri_size),
            ])
        else:
            # Right-pointing triangle
            poly = QPolygonF([
                QPointF(margin, margin),
                QPointF(margin + tri_size, margin + tri_size / 2),
                QPointF(margin, margin + tri_size),
            ])

        painter.drawPolygon(poly)
        painter.end()
