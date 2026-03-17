from PySide6.QtWidgets import QGridLayout, QLabel
from PySide6.QtCore import Qt

from gui_qt.dialogs.base_dialog import BaseDialog
from gui_qt.components.custom_components import SimpleButton, SimpleEntry


class NewFolderDialog(BaseDialog):
    """Dialog for creating or renaming a folder in the route."""

    def __init__(self, parent, controller, cur_folder_names, prev_folder_name, insert_after=None, **kwargs):
        if prev_folder_name is None:
            title = "Create New Folder"
        else:
            title = "Update Folder Name"

        super().__init__(parent, title=title, **kwargs)
        self._controller = controller
        self._cur_folder_names = cur_folder_names
        self._prev_folder_name = prev_folder_name
        self._insert_after = insert_after

        grid = QGridLayout(self)
        grid.setContentsMargins(10, 10, 10, 10)
        grid.setHorizontalSpacing(10)
        grid.setVerticalSpacing(10)

        # Label and entry
        self._label = QLabel()
        self._folder_name = SimpleEntry(callback=self.folder_name_update)
        grid.addWidget(self._label, 0, 0)
        grid.addWidget(self._folder_name, 0, 1)

        # Buttons
        self._add_button = SimpleButton()
        self._add_button.clicked.connect(self.create)
        self._cancel_button = SimpleButton("Cancel")
        self._cancel_button.clicked.connect(self.close)
        grid.addWidget(self._add_button, 1, 0)
        grid.addWidget(self._cancel_button, 1, 1)

        if prev_folder_name is None:
            self._label.setText("New Folder Name")
            self._add_button.setText("New Folder")
        else:
            self._label.setText("Update Folder Name")
            self._folder_name.set(prev_folder_name)
            self._add_button.setText("Update Folder")

        self._folder_name.setFocus()

    def keyPressEvent(self, event):
        if event.key() == Qt.Key_Return or event.key() == Qt.Key_Enter:
            self.create()
        else:
            super().keyPressEvent(event)

    def folder_name_update(self, *args, **kwargs):
        cur_name = self._folder_name.get()
        if cur_name in self._cur_folder_names:
            self._add_button.disable()
        else:
            self._add_button.enable()

    def create(self, *args, **kwargs):
        cur_name = self._folder_name.get()
        if not cur_name:
            return
        elif cur_name in self._cur_folder_names:
            return

        self.close()
        self._controller.finalize_new_folder(
            cur_name,
            prev_folder_name=self._prev_folder_name,
            insert_after=self._insert_after,
        )
