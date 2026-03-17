from PySide6.QtWidgets import (
    QVBoxLayout, QGridLayout, QLabel, QWidget, QCheckBox,
)
from PySide6.QtCore import Qt

from gui_qt.dialogs.base_dialog import BaseDialog
from gui_qt.components.custom_components import SimpleButton, SimpleEntry, SimpleOptionMenu
from utils.constants import const
from utils import io_utils


class LoadRouteDialog(BaseDialog):
    """Dialog for loading an existing route from saved routes."""

    def __init__(self, parent, controller, **kwargs):
        super().__init__(parent, title="Load Route", **kwargs)
        self._controller = controller

        layout = QVBoxLayout(self)

        # Controls grid
        controls = QWidget()
        grid = QGridLayout(controls)
        grid.setContentsMargins(5, 5, 5, 5)
        grid.setHorizontalSpacing(5)
        grid.setVerticalSpacing(5)

        # Existing routes dropdown
        self.previous_route_label = QLabel("Existing Routes:")
        self.previous_route_names = SimpleOptionMenu(
            option_list=[const.NO_SAVED_ROUTES],
            callback=self._select_callback,
        )
        self.previous_route_names.setMinimumWidth(200)
        grid.addWidget(self.previous_route_label, 0, 0)
        grid.addWidget(self.previous_route_names, 0, 1)

        # Filter
        self.filter_label = QLabel("Filter:")
        self.filter = SimpleEntry(callback=self._filter_callback)
        grid.addWidget(self.filter_label, 1, 0)
        grid.addWidget(self.filter, 1, 1)

        # Show backup routes checkbox
        self.outdated_label = QLabel("Show Backup Routes?")
        self.outdated_checkbox = QCheckBox()
        self.outdated_checkbox.stateChanged.connect(self._filter_callback)
        grid.addWidget(self.outdated_label, 2, 0)
        grid.addWidget(self.outdated_checkbox, 2, 1)

        # Info label
        self.outdated_info_label = QLabel(
            "Backup Routes are older versions of your route.\n"
            "Every save makes a backup that is persisted, and can be reloaded if needed.\n"
            "These are hidden by default because they can quickly pile up"
        )
        self.outdated_info_label.setAlignment(Qt.AlignCenter)
        grid.addWidget(self.outdated_info_label, 3, 0, 1, 2)

        # Warning label
        self.warning_label = QLabel(
            "WARNING: Any unsaved changes in your current route\n"
            "will be lost when loading an existing route!"
        )
        self.warning_label.setAlignment(Qt.AlignCenter)
        grid.addWidget(self.warning_label, 4, 0, 1, 2)

        # Buttons
        self.create_button = SimpleButton("Load Route")
        self.create_button.clicked.connect(self.load)
        self.cancel_button = SimpleButton("Cancel")
        self.cancel_button.clicked.connect(self.close)
        grid.addWidget(self.create_button, 10, 0)
        grid.addWidget(self.cancel_button, 10, 1)

        layout.addWidget(controls)

        # Initialize
        self._filter_callback()
        self._select_callback()
        self.filter.setFocus()

    def keyPressEvent(self, event):
        if event.key() == Qt.Key_Return or event.key() == Qt.Key_Enter:
            self.load()
        else:
            super().keyPressEvent(event)

    def _filter_callback(self, *args, **kwargs):
        all_routes = io_utils.get_existing_route_names(
            filter_text=self.filter.get(),
            load_backups=self.outdated_checkbox.isChecked(),
        )

        if not all_routes:
            all_routes = [const.NO_SAVED_ROUTES]

        self.previous_route_names.new_values(all_routes)
        self._select_callback()

    def _select_callback(self, *args, **kwargs):
        selected_route = self.previous_route_names.get()
        if selected_route == const.NO_SAVED_ROUTES:
            self.create_button.disable()
        else:
            self.create_button.enable()

    def get_selected_route_path(self):
        """Return the full filesystem path for the currently selected route."""
        selected_route = self.previous_route_names.get()
        if selected_route == const.NO_SAVED_ROUTES:
            return None
        return io_utils.get_existing_route_path(selected_route)

    def load(self, *args, **kwargs):
        selected_route = self.previous_route_names.get()
        if selected_route == const.NO_SAVED_ROUTES:
            return

        self.accept()
