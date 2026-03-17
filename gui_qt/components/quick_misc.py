import logging

from PySide6.QtWidgets import (
    QGroupBox, QWidget, QGridLayout, QVBoxLayout,
)
from PySide6.QtCore import Qt

from gui_qt.components.custom_components import SimpleButton
from pkmn.gen_factory import current_gen_info
from routing.route_events import (
    EventDefinition, SaveEventDefinition, HealEventDefinition,
    BlackoutEventDefinition, EvolutionEventDefinition,
    RareCandyEventDefinition, LearnMoveEventDefinition,
)
from utils.constants import const

logger = logging.getLogger(__name__)


class QuickMiscEvents(QGroupBox):
    def __init__(self, controller, parent=None):
        super().__init__("Misc", parent)
        self._controller = controller
        self._uninitialized = True

        main_layout = QVBoxLayout(self)
        main_layout.setContentsMargins(5, 5, 5, 5)

        buttons_widget = QWidget(self)
        buttons_layout = QGridLayout(buttons_widget)
        buttons_layout.setContentsMargins(0, 0, 0, 0)
        buttons_layout.setHorizontalSpacing(5)
        buttons_layout.setVerticalSpacing(2)

        # Configure columns to have equal weight
        buttons_layout.setColumnStretch(0, 1)
        buttons_layout.setColumnStretch(1, 1)

        # Column 0
        self._btn_move_tutor = SimpleButton(text="Tutor Move", parent=buttons_widget)
        self._btn_move_tutor.clicked.connect(self.add_move)
        buttons_layout.addWidget(self._btn_move_tutor, 0, 0)

        self._btn_add_save = SimpleButton(text="Add Save", parent=buttons_widget)
        self._btn_add_save.clicked.connect(self.add_save)
        buttons_layout.addWidget(self._btn_add_save, 1, 0)

        self._btn_add_heal = SimpleButton(text="Add Heal", parent=buttons_widget)
        self._btn_add_heal.clicked.connect(self.add_heal)
        buttons_layout.addWidget(self._btn_add_heal, 2, 0)

        self._btn_add_black_out = SimpleButton(text="Add Black Out", parent=buttons_widget)
        self._btn_add_black_out.clicked.connect(self.add_black_out)
        buttons_layout.addWidget(self._btn_add_black_out, 3, 0)

        self._btn_add_notes = SimpleButton(text="Add Notes", parent=buttons_widget)
        self._btn_add_notes.clicked.connect(self.add_notes)
        buttons_layout.addWidget(self._btn_add_notes, 4, 0)

        # Column 1
        self._btn_evolve = SimpleButton(text="Evolve", parent=buttons_widget)
        self._btn_evolve.clicked.connect(self.add_evolve)
        buttons_layout.addWidget(self._btn_evolve, 0, 1)

        self._btn_add_candies_2 = SimpleButton(text="Add 2 Candies", parent=buttons_widget)
        self._btn_add_candies_2.clicked.connect(self.add_candies_2)
        buttons_layout.addWidget(self._btn_add_candies_2, 1, 1)

        self._btn_add_candies_3 = SimpleButton(text="Add 3 Candies", parent=buttons_widget)
        self._btn_add_candies_3.clicked.connect(self.add_candies_3)
        buttons_layout.addWidget(self._btn_add_candies_3, 2, 1)

        self._btn_add_candies_5 = SimpleButton(text="Add 5 Candies", parent=buttons_widget)
        self._btn_add_candies_5.clicked.connect(self.add_candies_5)
        buttons_layout.addWidget(self._btn_add_candies_5, 3, 1)

        self._btn_add_candies_10 = SimpleButton(text="Add 10 Candies", parent=buttons_widget)
        self._btn_add_candies_10.clicked.connect(self.add_candies_10)
        buttons_layout.addWidget(self._btn_add_candies_10, 4, 1)

        main_layout.addWidget(buttons_widget)

        # Register callbacks
        self._unregister_selection = self._controller.register_event_selection(self.update_button_status)
        self._unregister_version = self._controller.register_version_change(self.update_pkmn_version)
        self.update_button_status()

    def update_button_status(self):
        if not self._controller.can_insert_after_current_selection() or self._uninitialized:
            self._btn_move_tutor.disable()
            self._btn_evolve.disable()
            self._btn_add_save.disable()
            self._btn_add_heal.disable()
            self._btn_add_black_out.disable()
            self._btn_add_notes.disable()
            self._btn_add_candies_2.disable()
            self._btn_add_candies_3.disable()
            self._btn_add_candies_5.disable()
            self._btn_add_candies_10.disable()
            return

        self._btn_move_tutor.enable()
        self._btn_evolve.enable()
        self._btn_add_save.enable()
        self._btn_add_heal.enable()
        self._btn_add_black_out.enable()
        self._btn_add_notes.enable()
        self._btn_add_candies_2.enable()
        self._btn_add_candies_3.enable()
        self._btn_add_candies_5.enable()
        self._btn_add_candies_10.enable()

    def update_pkmn_version(self):
        self._uninitialized = False

    def add_save(self):
        self._controller.new_event(
            EventDefinition(save=SaveEventDefinition()),
            insert_after=self._controller.get_single_selected_event_id(),
        )

    def add_heal(self):
        self._controller.new_event(
            EventDefinition(heal=HealEventDefinition()),
            insert_after=self._controller.get_single_selected_event_id(),
        )

    def add_black_out(self):
        self._controller.new_event(
            EventDefinition(blackout=BlackoutEventDefinition()),
            insert_after=self._controller.get_single_selected_event_id(),
        )

    def add_evolve(self):
        cur_state = self._controller.get_active_state()
        self._controller.new_event(
            EventDefinition(evolution=EvolutionEventDefinition(cur_state.solo_pkmn.name)),
            insert_after=self._controller.get_single_selected_event_id(),
        )

    def add_move(self):
        cur_state = self._controller.get_active_state()
        self._controller.new_event(
            EventDefinition(learn_move=LearnMoveEventDefinition(
                None,
                cur_state.solo_pkmn.get_move_destination(None, None)[0],
                const.MOVE_SOURCE_TUTOR,
            )),
            insert_after=self._controller.get_single_selected_event_id(),
        )

    def add_notes(self):
        self._controller.new_event(
            EventDefinition(),
            insert_after=self._controller.get_single_selected_event_id(),
        )

    def add_candies_2(self):
        self._controller.new_event(
            EventDefinition(rare_candy=RareCandyEventDefinition(2)),
            insert_after=self._controller.get_single_selected_event_id(),
        )

    def add_candies_3(self):
        self._controller.new_event(
            EventDefinition(rare_candy=RareCandyEventDefinition(3)),
            insert_after=self._controller.get_single_selected_event_id(),
        )

    def add_candies_5(self):
        self._controller.new_event(
            EventDefinition(rare_candy=RareCandyEventDefinition(5)),
            insert_after=self._controller.get_single_selected_event_id(),
        )

    def add_candies_10(self):
        self._controller.new_event(
            EventDefinition(rare_candy=RareCandyEventDefinition(10)),
            insert_after=self._controller.get_single_selected_event_id(),
        )
