from PySide6.QtWidgets import QGridLayout, QLabel
from PySide6.QtCore import Qt

from gui_qt.dialogs.base_dialog import BaseDialog
from gui_qt.components.custom_components import SimpleButton


class DeleteConfirmationDialog(BaseDialog):
    """Dialog for confirming deletion of events or non-empty folders."""

    def __init__(self, parent, controller, event_ids, **kwargs):
        super().__init__(parent, title="Confirm Delete", **kwargs)
        self._controller = controller
        self._event_ids = event_ids

        grid = QGridLayout(self)
        grid.setContentsMargins(10, 10, 10, 10)
        grid.setHorizontalSpacing(10)
        grid.setVerticalSpacing(10)

        if len(self._event_ids) == 1:
            text = (
                "You are trying to delete a non-empty folder.\n"
                "Are you sure you want to delete the folder and all child events?"
            )
        else:
            text = (
                "You are trying to delete multiple items at once.\n"
                "Are you sure you want to delete all selected items?"
            )

        self._label = QLabel(text)
        self._label.setAlignment(Qt.AlignCenter)
        grid.addWidget(self._label, 0, 0, 1, 2)

        self._confirm_button = SimpleButton("Delete")
        self._confirm_button.clicked.connect(self.delete)
        self._cancel_button = SimpleButton("Cancel")
        self._cancel_button.clicked.connect(self.close)
        grid.addWidget(self._confirm_button, 1, 0)
        grid.addWidget(self._cancel_button, 1, 1)

    def keyPressEvent(self, event):
        if event.key() == Qt.Key_Return or event.key() == Qt.Key_Enter:
            self.delete()
        else:
            super().keyPressEvent(event)

    def delete(self, *args, **kwargs):
        self.close()
        self._controller.delete_events(self._event_ids)
