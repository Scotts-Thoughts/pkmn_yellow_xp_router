import logging

from PySide6.QtWidgets import (
    QGroupBox, QWidget, QLabel, QGridLayout, QHBoxLayout, QVBoxLayout,
)
from PySide6.QtCore import Qt

from gui_qt.components.custom_components import (
    SimpleButton, SimpleEntry, SimpleOptionMenu, AmountEntry,
)
from pkmn.gen_factory import current_gen_info
from routing.route_events import EventDefinition, WildPkmnEventDefinition
from utils.constants import const

logger = logging.getLogger(__name__)


class QuickWildPkmn(QGroupBox):
    def __init__(self, controller, parent=None):
        super().__init__("Wild Pkmn", parent)
        self._controller = controller

        self.option_menu_width = 20

        main_layout = QVBoxLayout(self)
        main_layout.setContentsMargins(5, 5, 5, 5)

        # --- Dropdowns / inputs area ---
        dropdowns_widget = QWidget(self)
        dropdowns_layout = QGridLayout(dropdowns_widget)
        dropdowns_layout.setContentsMargins(0, 0, 0, 0)
        dropdowns_layout.setHorizontalSpacing(5)
        dropdowns_layout.setVerticalSpacing(1)

        cur_row = 0

        # Filter
        self._pkmn_filter_label = QLabel("Filter:", dropdowns_widget)
        self._pkmn_filter_label.setAlignment(Qt.AlignLeft)
        dropdowns_layout.addWidget(self._pkmn_filter_label, cur_row, 0)

        self._pkmn_filter = SimpleEntry(callback=self._pkmn_filter_callback, parent=dropdowns_widget)
        dropdowns_layout.addWidget(self._pkmn_filter, cur_row, 1)
        cur_row += 1

        # Wild Pkmn dropdown
        self._pkmn_types_label = QLabel("Wild Pkmn:", dropdowns_widget)
        self._pkmn_types_label.setAlignment(Qt.AlignLeft)
        dropdowns_layout.addWidget(self._pkmn_types_label, cur_row, 0)

        self._pkmn_types = SimpleOptionMenu(
            option_list=[const.NO_POKEMON],
            parent=dropdowns_widget,
        )
        dropdowns_layout.addWidget(self._pkmn_types, cur_row, 1)
        cur_row += 1

        # Level
        self._level_label = QLabel("Pkmn Level:", dropdowns_widget)
        self._level_label.setAlignment(Qt.AlignLeft)
        dropdowns_layout.addWidget(self._level_label, cur_row, 0)

        self._level_val = AmountEntry(
            min_val=2,
            max_val=100,
            callback=self._update_button_callback_wrapper,
            parent=dropdowns_widget,
        )
        dropdowns_layout.addWidget(self._level_val, cur_row, 1)
        cur_row += 1

        # Quantity
        self._quantity_label = QLabel("Quantity:", dropdowns_widget)
        self._quantity_label.setAlignment(Qt.AlignLeft)
        dropdowns_layout.addWidget(self._quantity_label, cur_row, 0)

        self._quantity_val = AmountEntry(
            min_val=1,
            callback=self._update_button_callback_wrapper,
            parent=dropdowns_widget,
        )
        dropdowns_layout.addWidget(self._quantity_val, cur_row, 1)
        cur_row += 1

        main_layout.addWidget(dropdowns_widget)

        # --- Buttons area ---
        buttons_widget = QWidget(self)
        buttons_layout = QHBoxLayout(buttons_widget)
        buttons_layout.setContentsMargins(0, 0, 0, 0)
        buttons_layout.setSpacing(5)

        self._add_wild_pkmn = SimpleButton(text="Add Wild Pkmn", parent=buttons_widget)
        self._add_wild_pkmn.clicked.connect(self.add_wild_pkmn_cmd)
        buttons_layout.addWidget(self._add_wild_pkmn)

        self._add_trainer_pkmn = SimpleButton(text="Add Trainer Pkmn", parent=buttons_widget)
        self._add_trainer_pkmn.clicked.connect(self.add_trainer_pkmn_cmd)
        buttons_layout.addWidget(self._add_trainer_pkmn)

        main_layout.addWidget(buttons_widget)

        # Set defaults
        self._level_val.set(5)

        # Register callbacks
        self._unregister_selection = self._controller.register_event_selection(self.update_button_status)
        self._unregister_version = self._controller.register_version_change(self.update_pkmn_version)
        self.update_button_status()

    def update_button_status(self):
        if not hasattr(self, '_add_wild_pkmn'):
            return
        if not self._controller.can_insert_after_current_selection():
            self._add_wild_pkmn.disable()
            self._add_trainer_pkmn.disable()
            return

        valid = True
        if self._pkmn_types.get().strip().startswith(const.NO_POKEMON):
            valid = False

        try:
            level = int(self._level_val.get().strip())
            if level < 2 or level > 100:
                raise ValueError
        except Exception:
            valid = False

        try:
            quantity = int(self._quantity_val.get().strip())
            if quantity < 1:
                raise ValueError
        except Exception:
            valid = False

        if not valid:
            if hasattr(self, '_add_wild_pkmn'):
                self._add_wild_pkmn.disable()
            if hasattr(self, '_add_trainer_pkmn'):
                self._add_trainer_pkmn.disable()
        else:
            if hasattr(self, '_add_wild_pkmn'):
                self._add_wild_pkmn.enable()
            if hasattr(self, '_add_trainer_pkmn'):
                self._add_trainer_pkmn.enable()

    def update_pkmn_version(self):
        self._pkmn_types.new_values(current_gen_info().pkmn_db().get_all_names())

    def _pkmn_filter_callback(self):
        self._pkmn_types.new_values(
            current_gen_info().pkmn_db().get_filtered_names(filter_val=self._pkmn_filter.get().strip())
        )
        self.update_button_status()

    def _update_button_callback_wrapper(self):
        self.update_button_status()

    def add_wild_pkmn_cmd(self):
        self._controller.new_event(
            EventDefinition(
                wild_pkmn_info=WildPkmnEventDefinition(
                    self._pkmn_types.get(),
                    int(self._level_val.get().strip()),
                    quantity=int(self._quantity_val.get().strip()),
                )
            ),
            insert_after=self._controller.get_single_selected_event_id(),
        )

    def add_trainer_pkmn_cmd(self):
        self._controller.new_event(
            EventDefinition(
                wild_pkmn_info=WildPkmnEventDefinition(
                    self._pkmn_types.get(),
                    int(self._level_val.get().strip()),
                    quantity=int(self._quantity_val.get().strip()),
                    trainer_pkmn=True,
                )
            ),
            insert_after=self._controller.get_single_selected_event_id(),
        )
