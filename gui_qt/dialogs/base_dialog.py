import sys

from PySide6.QtWidgets import QDialog
from PySide6.QtCore import Qt


class BaseDialog(QDialog):
    """Base dialog class for all popup dialogs.

    Replaces the Tkinter Popup(tk.Toplevel) base class.
    Handles modality, centering on parent, and popup-open flag management.
    """

    def __init__(self, parent=None, title="", **kwargs):
        super().__init__(parent)
        self._main_window = parent

        if title:
            self.setWindowTitle(title)

        self.setWindowModality(Qt.ApplicationModal)

        # Suppress all keyboard shortcuts while popup is open
        if self._main_window is not None and hasattr(self._main_window, '_popup_open'):
            self._main_window._popup_open = True

    def showEvent(self, event):
        """Center on parent window after the dialog is shown and has its final size."""
        super().showEvent(event)
        self._center_on_parent()

    def _center_on_parent(self):
        """Center this dialog on the parent window instead of the screen."""
        if self._main_window is None:
            return

        try:
            parent_geo = self._main_window.geometry()
            dialog_geo = self.geometry()

            x = parent_geo.x() + (parent_geo.width() // 2) - (dialog_geo.width() // 2)
            y = parent_geo.y() + (parent_geo.height() // 2) - (dialog_geo.height() // 2)

            # Ensure dialog stays on screen
            if x < 0 or y < 0:
                screen = self.screen()
                if screen is not None:
                    screen_geo = screen.availableGeometry()
                    x = screen_geo.x() + (screen_geo.width() // 2) - (dialog_geo.width() // 2)
                    y = screen_geo.y() + (screen_geo.height() // 2) - (dialog_geo.height() // 2)

            self.move(x, y)
        except Exception:
            pass

    def close(self):
        """Clean up and close the dialog."""
        if self._main_window is not None and hasattr(self._main_window, '_popup_open'):
            self._main_window._popup_open = False
        super().close()

    def closeEvent(self, event):
        """Handle the window close button (X)."""
        if self._main_window is not None and hasattr(self._main_window, '_popup_open'):
            self._main_window._popup_open = False
        super().closeEvent(event)

    def keyPressEvent(self, event):
        """Handle Escape key to close dialog."""
        if event.key() == Qt.Key_Escape:
            self.close()
        else:
            super().keyPressEvent(event)
