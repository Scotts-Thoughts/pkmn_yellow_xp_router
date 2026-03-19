from PySide6.QtWidgets import (
    QDialog, QVBoxLayout, QGridLayout, QWidget, QPushButton,
)
from PySide6.QtCore import Qt

from gui_qt.components.quick_trainer_add import QuickTrainerAdd
from gui_qt.components.quick_item_add import QuickItemAdd
from gui_qt.components.quick_wild_pkmn import QuickWildPkmn
from gui_qt.components.quick_misc import QuickMiscEvents


class AddEventsDialog(QDialog):
    """Non-modal popover for adding events to the route (Ctrl+F1)."""

    def __init__(self, controller, parent=None):
        super().__init__(parent)
        self._controller = controller
        self.setWindowTitle("Add Events")
        self.setWindowFlags(
            Qt.Window | Qt.WindowCloseButtonHint
        )
        self.setMinimumWidth(900)

        layout = QVBoxLayout(self)
        layout.setContentsMargins(6, 6, 6, 6)
        layout.setSpacing(4)

        grid = QGridLayout()
        grid.setSpacing(4)
        grid.setColumnStretch(0, 2)
        grid.setColumnStretch(1, 2)
        grid.setColumnStretch(2, 1)

        self.trainer_add = QuickTrainerAdd(self._controller)
        grid.addWidget(self.trainer_add, 0, 0)

        self.item_add = QuickItemAdd(self._controller)
        grid.addWidget(self.item_add, 0, 1)

        self.wild_pkmn_add = QuickWildPkmn(self._controller)
        grid.addWidget(self.wild_pkmn_add, 0, 2)

        self.misc_add = QuickMiscEvents(self._controller)
        grid.addWidget(self.misc_add, 1, 2)

        layout.addLayout(grid)

        self._center_on_parent()

    def _center_on_parent(self):
        parent = self.parentWidget()
        if parent is None:
            return
        try:
            parent_geo = parent.geometry()
            dialog_geo = self.geometry()
            x = parent_geo.x() + (parent_geo.width() // 2) - (dialog_geo.width() // 2)
            y = parent_geo.y() + (parent_geo.height() // 2) - (dialog_geo.height() // 2)
            self.move(x, y)
        except Exception:
            pass

    def showEvent(self, event):
        super().showEvent(event)
        self._center_on_parent()

    def keyPressEvent(self, event):
        if event.key() == Qt.Key_F1 and event.modifiers() == Qt.ControlModifier:
            self.close()
        else:
            super().keyPressEvent(event)
