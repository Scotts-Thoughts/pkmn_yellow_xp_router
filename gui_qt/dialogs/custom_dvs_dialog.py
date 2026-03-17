import logging

from PySide6.QtWidgets import QVBoxLayout, QHBoxLayout, QWidget
from PySide6.QtCore import Qt

from gui_qt.dialogs.base_dialog import BaseDialog
from gui_qt.components.custom_components import SimpleButton
from gui_qt.pkmn_components.custom_dvs import CustomDVsFrame
from pkmn.gen_factory import current_gen_info
from pkmn.universal_data_objects import Nature, StatBlock

logger = logging.getLogger(__name__)


class CustomDvsDialog(BaseDialog):
    """Dialog for setting custom DVs/IVs, ability, and nature for the solo pokemon."""

    def __init__(self, parent, controller, init_dvs: StatBlock, init_ability_idx: int, init_nature: Nature, **kwargs):
        super().__init__(parent, title="Custom DVs/IVs", **kwargs)
        self._controller = controller

        layout = QVBoxLayout(self)

        self._dvs_frame = CustomDVsFrame(
            self._controller.get_init_state().solo_pkmn.species_def,
            parent=self,
            target_game=current_gen_info(),
            init_dvs=init_dvs,
            init_ability_idx=init_ability_idx,
            init_nature=init_nature,
        )
        layout.addWidget(self._dvs_frame)

        # Buttons
        buttons = QWidget()
        btn_layout = QHBoxLayout(buttons)
        btn_layout.setContentsMargins(0, 0, 0, 0)

        self.create_button = SimpleButton("Set New DVs")
        self.create_button.clicked.connect(self.set_dvs)
        self.cancel_button = SimpleButton("Cancel")
        self.cancel_button.clicked.connect(self.close)
        btn_layout.addWidget(self.create_button)
        btn_layout.addWidget(self.cancel_button)

        layout.addWidget(buttons)

        self._dvs_frame.recalc_hidden_power()

    def keyPressEvent(self, event):
        if event.key() == Qt.Key_Return or event.key() == Qt.Key_Enter:
            self.set_dvs()
        else:
            super().keyPressEvent(event)

    def set_dvs(self, *args, **kwargs):
        dvs, ability_idx, nature = self._dvs_frame.get_dvs()
        self._controller.customize_innate_stats(dvs, ability_idx, nature)
        self.close()
