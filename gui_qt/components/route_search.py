import logging

from PySide6.QtWidgets import QWidget, QLabel, QGridLayout
from PySide6.QtCore import Qt, QTimer

from gui_qt.components.custom_components import SimpleButton, SimpleEntry, CheckboxLabel
from utils.constants import const

logger = logging.getLogger(__name__)


class RouteSearch(QWidget):
    def __init__(self, controller, parent=None):
        super().__init__(parent)
        self._controller = controller

        self._filter_vals = []
        self._filter_components = []

        self._search_timer = QTimer(self)
        self._search_timer.setSingleShot(True)
        self._search_timer.setInterval(300)
        self._search_timer.timeout.connect(self._delayed_search_callback)

        layout = QGridLayout(self)
        layout.setContentsMargins(5, 1, 5, 1)
        layout.setHorizontalSpacing(5)
        layout.setVerticalSpacing(1)

        num_filters_per_row = 5
        for cur_idx, event_type in enumerate(const.ROUTE_EVENT_TYPES):
            cur_row = cur_idx // num_filters_per_row
            cur_col = (cur_idx % num_filters_per_row) * 2

            cur_checkbox = CheckboxLabel(
                text=event_type,
                toggle_command=self._curry_filter_callback(event_type),
                parent=self,
            )
            layout.addWidget(cur_checkbox, cur_row, cur_col, 1, 1)
            self._filter_components.append(cur_checkbox)

        row_idx = (len(const.ROUTE_EVENT_TYPES) // num_filters_per_row) + 1

        self.reset_button = SimpleButton(text="Reset All Filters", parent=self)
        self.reset_button.clicked.connect(self.reset_all_filters)
        layout.addWidget(self.reset_button, row_idx, 0, 1, 1, Qt.AlignLeft)

        self.search_label = QLabel("Search:", self)
        layout.addWidget(self.search_label, row_idx, 2, 1, 1, Qt.AlignLeft)

        self.search_val = SimpleEntry(callback=self._search_callback, parent=self)
        layout.addWidget(self.search_val, row_idx, 4, 1, 3)

    def reset_all_filters(self):
        for cur_checkbox in self._filter_components:
            if cur_checkbox.is_checked():
                cur_checkbox.toggle_checked()

        self.search_val.set("")

    def _search_callback(self):
        self._search_timer.stop()
        self._search_timer.start()

    def _delayed_search_callback(self):
        self._controller.set_route_search(self.search_val.get())

    def _curry_filter_callback(self, string_val):
        def inner():
            if string_val in self._filter_vals:
                self._filter_vals.remove(string_val)
            else:
                self._filter_vals.append(string_val)

            self._controller.set_route_filter_types(self._filter_vals)

        return inner

    def toggle_filter_by_type(self, event_type):
        for checkbox in self._filter_components:
            if checkbox.text() == event_type:
                checkbox.toggle_checked()
                return

    def set_filter_by_type(self, event_type, checked):
        for checkbox in self._filter_components:
            if checkbox.text() == event_type:
                # set_checked blocks signals, so manually sync filter state
                checkbox.set_checked(checked)
                if checked and event_type not in self._filter_vals:
                    self._filter_vals.append(event_type)
                elif not checked and event_type in self._filter_vals:
                    self._filter_vals.remove(event_type)
                self._controller.set_route_filter_types(self._filter_vals)
                return

    def is_filter_checked(self, event_type):
        for checkbox in self._filter_components:
            if checkbox.text() == event_type:
                return checkbox.is_checked()
        return False
