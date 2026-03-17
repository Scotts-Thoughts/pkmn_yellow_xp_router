import logging

from PySide6.QtWidgets import (
    QWidget, QLabel, QVBoxLayout, QMenuBar,
)
from PySide6.QtCore import Qt
from PySide6.QtGui import QAction, QKeySequence, QShortcut

from controllers.main_controller import MainController
from pkmn.gen_factory import current_gen_info

logger = logging.getLogger(__name__)


class SetupSummaryWindow(QWidget):
    """Non-modal window that lists setup moves used in major/highlighted fights."""

    def __init__(self, main_window, controller: MainController, parent=None):
        super().__init__(parent)
        self.setWindowFlags(Qt.Window)
        self.setAttribute(Qt.WA_DeleteOnClose)

        self._controller = controller
        self._main_window = main_window

        self.setWindowTitle("Setup Summary")

        # ---- layout ----
        outer = QVBoxLayout(self)
        outer.setContentsMargins(0, 0, 0, 0)
        outer.setSpacing(0)

        # ---- menu bar ----
        self._menu_bar = QMenuBar(self)
        export_action = QAction("Export Screenshot (Ctrl+P)", self)
        export_action.setShortcut(QKeySequence("Ctrl+P"))
        export_action.triggered.connect(self._export_screen_shot)
        self._menu_bar.addAction(export_action)
        outer.addWidget(self._menu_bar)

        # ---- content ----
        content_frame = QWidget()
        content_layout = QVBoxLayout(content_frame)
        content_layout.setContentsMargins(15, 15, 15, 15)

        self._text = QLabel()
        self._text.setWordWrap(True)
        self._text.setAlignment(Qt.AlignLeft | Qt.AlignTop)
        content_layout.addWidget(self._text)

        outer.addWidget(content_frame)

        # ---- keyboard shortcuts ----
        QShortcut(QKeySequence("Ctrl+P"), self, self._export_screen_shot)

        # ---- controller subscription ----
        self._unsubscribe_route = self._controller.register_route_change(self._refresh)

        self._refresh()

    # ------------------------------------------------------------------
    # Cleanup
    # ------------------------------------------------------------------
    def closeEvent(self, event):
        if self._unsubscribe_route is not None:
            self._unsubscribe_route()
            self._unsubscribe_route = None
        super().closeEvent(event)

    # ------------------------------------------------------------------
    # Screenshot
    # ------------------------------------------------------------------
    def _export_screen_shot(self):
        geo = self.geometry()
        bbox = (geo.x(), geo.y(), geo.x() + geo.width(), geo.y() + geo.height())
        self._controller.take_screenshot("setup_summary", bbox)

    # ------------------------------------------------------------------
    # Refresh
    # ------------------------------------------------------------------
    def _refresh(self):
        moves_used = []

        cur_event = self._controller.get_next_event()
        while cur_event is not None:
            if (
                cur_event.event_definition.trainer_def is not None
                and cur_event.event_definition.enabled
                and (
                    cur_event.event_definition.is_highlighted()
                    or current_gen_info().is_major_fight(
                        cur_event.event_definition.trainer_def.trainer_name
                    )
                )
                and cur_event.event_definition.trainer_def.setup_moves
            ):
                cur_setup_moves = {}
                for sm in cur_event.event_definition.trainer_def.setup_moves:
                    cur_setup_moves[sm] = cur_setup_moves.get(sm, 0) + 1
                setup_moves_text = [
                    f"x{count} {sm}" for sm, count in cur_setup_moves.items()
                ]

                moves_used.append(
                    f"{cur_event.event_definition.get_label()}: {','.join(setup_moves_text)}"
                )

            cur_event = self._controller.get_next_event(cur_event.group_id)

        if moves_used:
            final_text = "\n".join(["Setup Moves:"] + moves_used)
        else:
            final_text = "No setup moves used"

        self._text.setText(final_text)
