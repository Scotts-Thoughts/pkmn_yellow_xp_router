from PySide6.QtWidgets import QDialog, QVBoxLayout
from PySide6.QtCore import Qt

from gui_qt.components.route_search import RouteSearch


class FiltersDialog(QDialog):
    """Non-modal popover for route filters and search (Ctrl+F2)."""

    def __init__(self, controller, parent=None):
        super().__init__(parent)
        self._controller = controller
        self.setWindowTitle("Filters")
        self.setWindowFlags(
            Qt.Window | Qt.WindowCloseButtonHint
        )

        layout = QVBoxLayout(self)
        layout.setContentsMargins(6, 6, 6, 6)

        self.route_search = RouteSearch(self._controller)
        layout.addWidget(self.route_search)

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
        if event.key() == Qt.Key_F2 and event.modifiers() == Qt.ControlModifier:
            self.close()
        else:
            super().keyPressEvent(event)
