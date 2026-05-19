from PySide6.QtWidgets import QVBoxLayout, QHBoxLayout, QLabel
from PySide6.QtCore import Qt

from gui_qt.dialogs.base_dialog import BaseDialog
from gui_qt.components.custom_components import SimpleButton


class MatchupExportDialog(BaseDialog):
    """Asks the user which slice of a matchup to export as an image."""

    MODE_FULL = "full"
    MODE_PLAYER = "player"
    MODE_ENEMY = "enemy"

    def __init__(self, parent, **kwargs):
        super().__init__(parent, title="Export Matchup", **kwargs)
        self.selected_mode = None

        layout = QVBoxLayout(self)
        layout.setContentsMargins(12, 12, 12, 12)
        layout.setSpacing(10)

        prompt = QLabel("Which graphic would you like to export?")
        prompt.setAlignment(Qt.AlignCenter)
        layout.addWidget(prompt)

        choice_row = QHBoxLayout()
        choice_row.setSpacing(8)
        for label, mode in (
            ("Match Up", self.MODE_FULL),
            ("Player Ranges", self.MODE_PLAYER),
            ("Enemy Ranges", self.MODE_ENEMY),
        ):
            btn = SimpleButton(label)
            btn.clicked.connect(lambda _=False, m=mode: self._choose(m))
            choice_row.addWidget(btn)
        layout.addLayout(choice_row)

        cancel = SimpleButton("Cancel")
        cancel.clicked.connect(self.close)
        layout.addWidget(cancel, alignment=Qt.AlignCenter)

    def _choose(self, mode):
        self.selected_mode = mode
        self.close()
