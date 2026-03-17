import logging

from PySide6.QtWidgets import (
    QGroupBox, QWidget, QLabel, QGridLayout, QHBoxLayout, QVBoxLayout,
)
from PySide6.QtCore import Qt

from gui_qt.components.custom_components import SimpleButton, SimpleOptionMenu, CheckboxLabel
from pkmn.universal_data_objects import Trainer
from pkmn import universal_utils
from pkmn.gen_factory import current_gen_info
from routing.route_events import EventDefinition, EventFolder, TrainerEventDefinition
from utils.constants import const

logger = logging.getLogger(__name__)


class QuickTrainerAdd(QGroupBox):
    def __init__(self, controller, parent=None):
        super().__init__("Trainers", parent)
        self._controller = controller
        self._ignore_preview = False
        self._multi_setup_mode = False
        self._saved_partner = None

        self.option_menu_width = 27

        main_layout = QVBoxLayout(self)
        main_layout.setContentsMargins(5, 5, 5, 5)

        # --- Dropdowns area ---
        dropdowns_widget = QWidget(self)
        dropdowns_layout = QGridLayout(dropdowns_widget)
        dropdowns_layout.setContentsMargins(0, 0, 0, 0)
        dropdowns_layout.setHorizontalSpacing(5)
        dropdowns_layout.setVerticalSpacing(1)

        cur_row = 0

        # Location
        self._trainers_by_loc_label = QLabel("Location:", dropdowns_widget)
        self._trainers_by_loc_label.setAlignment(Qt.AlignLeft)
        dropdowns_layout.addWidget(self._trainers_by_loc_label, cur_row, 0)

        self._trainers_by_loc = SimpleOptionMenu(
            option_list=[const.ALL_TRAINERS],
            callback=self.trainer_filter_callback,
            parent=dropdowns_widget,
        )
        dropdowns_layout.addWidget(self._trainers_by_loc, cur_row, 1)
        cur_row += 1

        # Trainer Class
        self._trainers_by_class_label = QLabel("Trainer Class:", dropdowns_widget)
        self._trainers_by_class_label.setAlignment(Qt.AlignLeft)
        dropdowns_layout.addWidget(self._trainers_by_class_label, cur_row, 0)

        self._trainers_by_class = SimpleOptionMenu(
            option_list=[const.ALL_TRAINERS],
            callback=self.trainer_filter_callback,
            parent=dropdowns_widget,
        )
        dropdowns_layout.addWidget(self._trainers_by_class, cur_row, 1)
        cur_row += 1

        # Trainer Name
        self._trainer_names_label = QLabel("Trainer:", dropdowns_widget)
        self._trainer_names_label.setAlignment(Qt.AlignLeft)
        dropdowns_layout.addWidget(self._trainer_names_label, cur_row, 0)

        self._trainer_names = SimpleOptionMenu(
            option_list=[const.NO_TRAINERS],
            callback=self._trainer_name_callback,
            parent=dropdowns_widget,
        )
        dropdowns_layout.addWidget(self._trainer_names, cur_row, 1)
        cur_row += 1

        # Show Rematches
        self._rematches_label = CheckboxLabel(
            text="Show Rematches:",
            flip=True,
            toggle_command=self.trainer_filter_callback,
            parent=dropdowns_widget,
        )
        dropdowns_layout.addWidget(self._rematches_label, cur_row, 0, 1, 2)
        cur_row += 1

        # Multi Partner display (hidden by default)
        self._partner_label_row = cur_row
        self._multi_partner_label = QLabel("Multi Partner: ", dropdowns_widget)
        self._multi_partner_name = QLabel("", dropdowns_widget)
        self._multi_partner_name.setAlignment(Qt.AlignRight)
        dropdowns_layout.addWidget(self._multi_partner_label, cur_row, 0)
        dropdowns_layout.addWidget(self._multi_partner_name, cur_row, 1)
        self._multi_partner_label.setVisible(False)
        self._multi_partner_name.setVisible(False)
        cur_row += 1

        main_layout.addWidget(dropdowns_widget)

        # --- Buttons area ---
        buttons_widget = QWidget(self)
        buttons_layout = QHBoxLayout(buttons_widget)
        buttons_layout.setContentsMargins(0, 0, 0, 0)
        buttons_layout.setSpacing(5)

        self._add_trainer = SimpleButton(text="Add Trainer", parent=buttons_widget)
        self._add_trainer.clicked.connect(self.add_trainer)
        buttons_layout.addWidget(self._add_trainer)

        self._add_area = SimpleButton(text="Add Area", parent=buttons_widget)
        self._add_area.clicked.connect(self.add_area)
        buttons_layout.addWidget(self._add_area)

        self._add_multi = SimpleButton(text="Multi Partner", parent=buttons_widget)
        self._add_multi.clicked.connect(self.toggle_multi)
        buttons_layout.addWidget(self._add_multi)

        main_layout.addWidget(buttons_widget)

        # Register callbacks
        self._unregister_selection = self._controller.register_event_selection(self.update_button_status)
        self._unregister_version = self._controller.register_version_change(self.update_pkmn_version)
        self._unregister_route = self._controller.register_route_change(self.route_change_callback)
        self.update_button_status()

    def update_button_status(self):
        if not self._controller.can_insert_after_current_selection():
            self._add_trainer.disable()
            self._add_area.disable()
            self._add_multi.disable()
            return

        selected_trainer = self._get_trainer_name()
        if selected_trainer == const.NO_TRAINERS:
            self._add_trainer.disable()
            self._add_area.disable()
            if self._multi_setup_mode:
                self._add_multi.enable()
            else:
                self._add_multi.disable()
        else:
            self._add_trainer.enable()
            loc_filter = self._trainers_by_loc.get()
            if loc_filter == const.ALL_TRAINERS or self._multi_setup_mode:
                self._add_area.disable()
            else:
                self._add_area.enable()
            if current_gen_info().get_generation() >= 3 and current_gen_info().trainer_db().can_trainer_multi_battle(selected_trainer):
                self._add_multi.enable()
            else:
                self._add_multi.disable()

    def update_pkmn_version(self):
        self._trainers_by_loc.new_values([const.ALL_TRAINERS] + sorted(current_gen_info().trainer_db().get_all_locations()))
        self._trainers_by_class.new_values([const.ALL_TRAINERS] + sorted(current_gen_info().trainer_db().get_all_classes()))
        self._set_multi_setup_mode(False)

        if current_gen_info().get_generation() >= 3:
            self._add_multi.setVisible(True)
        else:
            self._add_multi.setVisible(False)

        self._trainer_name_callback()

    def route_change_callback(self):
        self.trainer_filter_callback(ignore_trainer_preview=True)

    @staticmethod
    def _custom_trainer_name(trainer_obj: Trainer):
        return f"({universal_utils.experience_per_second(current_gen_info().get_trainer_timing_info(), trainer_obj.pkmn)}) {trainer_obj.name}"

    def _get_trainer_name(self):
        name_with_exp_per_sec = self._trainer_names.get()
        return name_with_exp_per_sec[name_with_exp_per_sec.find(')') + 1:].strip()

    def trainer_filter_callback(self, ignore_trainer_preview=False):
        loc_filter = self._trainers_by_loc.get()
        class_filter = self._trainers_by_class.get()

        self._ignore_preview = ignore_trainer_preview
        defeated = self._controller.get_defeated_trainers()
        if self._saved_partner:
            defeated = defeated.union(set([self._saved_partner]))
        valid_trainers = current_gen_info().trainer_db().get_valid_trainers(
            trainer_class=class_filter,
            trainer_loc=loc_filter,
            defeated_trainers=defeated,
            show_rematches=self._rematches_label.is_checked(),
            custom_name_fn=self._custom_trainer_name,
            multi_only=self._multi_setup_mode,
        )
        if not valid_trainers:
            valid_trainers.append(const.NO_TRAINERS)

        self._trainer_names.new_values(valid_trainers)
        self.update_button_status()
        self._ignore_preview = False

    def _trainer_name_callback(self):
        self.update_button_status()
        selected_trainer = self._get_trainer_name()
        if selected_trainer == const.NO_TRAINERS:
            return

        if not self._ignore_preview:
            self._controller.set_preview_trainer(selected_trainer)

    def add_trainer(self):
        if self._multi_setup_mode:
            temp = EventDefinition(trainer_def=TrainerEventDefinition(self._saved_partner, second_trainer_name=self._get_trainer_name()))
            temp.trainer_def.exp_split = [2 for _ in range(len(temp.get_pokemon_list()))]

            self._controller.new_event(temp, insert_after=self._controller.get_single_selected_event_id())
            self.toggle_multi()
        else:
            trainer_name = self._get_trainer_name()
            temp = EventDefinition(trainer_def=TrainerEventDefinition(trainer_name))
            trainer_obj = current_gen_info().trainer_db().get_trainer(trainer_name)
            if trainer_obj.double_battle:
                temp.trainer_def.exp_split = [2 for _ in range(len(temp.get_pokemon_list()))]

            selected_id = self._controller.get_single_selected_event_id()
            selected_obj = self._controller.get_single_selected_event_obj() if selected_id else None

            if isinstance(selected_obj, EventFolder):
                folder_base_name = trainer_obj.location if (trainer_obj.location and trainer_obj.location.strip()) else "New Folder"

                all_folder_names = set(self._controller.get_all_folder_names())
                folder_name = folder_base_name
                count = 1
                while folder_name in all_folder_names:
                    count += 1
                    folder_name = f"{folder_base_name} Trip:{count}"

                self._controller.finalize_new_folder(folder_name, insert_after=selected_id)
                self._controller.new_event(temp, dest_folder_name=folder_name)
            else:
                self._controller.new_event(temp, insert_after=selected_id)

    def add_area(self):
        insert_after = self._controller.get_single_selected_event_id()
        if insert_after is None:
            return

        self._controller.add_area(
            self._trainers_by_loc.get(),
            self._rematches_label.is_checked(),
            insert_after,
        )

    def toggle_multi(self):
        self._set_multi_setup_mode(not self._multi_setup_mode)
        self.trainer_filter_callback(ignore_trainer_preview=(not self._multi_setup_mode))
        self.update_button_status()

    def _set_multi_setup_mode(self, new_val):
        if new_val:
            selected_trainer = self._get_trainer_name()
            if selected_trainer == const.NO_TRAINERS:
                return
            if not current_gen_info().trainer_db().can_trainer_multi_battle(selected_trainer):
                return

            self._saved_partner = selected_trainer
            self._multi_setup_mode = True
            self._add_multi.setText("Cancel Multi")
            self._add_trainer.setText("Add Multi")
            self.setTitle("Select Partner")
            self._multi_partner_label.setVisible(True)
            self._multi_partner_name.setText(f"{self._saved_partner}")
            self._multi_partner_name.setVisible(True)

            trainer_obj = current_gen_info().trainer_db().get_trainer(selected_trainer)
            self._trainers_by_loc.set(trainer_obj.location)
            self._trainers_by_loc.disable()
        else:
            self._multi_setup_mode = False
            self._saved_partner = None
            self._add_multi.setText("Multi Partner")
            self._add_trainer.setText("Add Trainer")
            self.setTitle("Trainers")
            self._multi_partner_label.setVisible(False)
            self._multi_partner_name.setVisible(False)
            self._trainers_by_loc.enable()
