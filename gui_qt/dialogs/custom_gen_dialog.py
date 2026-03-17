from PySide6.QtWidgets import (
    QVBoxLayout, QGridLayout, QLabel, QWidget, QPushButton,
)
from PySide6.QtCore import Qt

from gui_qt.dialogs.base_dialog import BaseDialog
from gui_qt.components.custom_components import SimpleButton, SimpleEntry, SimpleOptionMenu
from utils import io_utils
from pkmn.gen_factory import _gen_factory as gen_factory


class CustomGenDialog(BaseDialog):
    """Dialog for creating and managing custom gens (romhacks)."""

    def __init__(self, parent, controller, **kwargs):
        super().__init__(parent, title="Custom Gen Manager", **kwargs)
        self._controller = controller

        main_layout = QVBoxLayout(self)

        # === New custom gen section ===
        self.new_version_frame = QWidget()
        new_grid = QGridLayout(self.new_version_frame)
        new_grid.setContentsMargins(5, 10, 5, 10)
        new_grid.setHorizontalSpacing(5)
        new_grid.setVerticalSpacing(5)

        instructions_header = QLabel("Custom Gen Instructions:")
        instructions_header.setAlignment(Qt.AlignCenter)
        new_grid.addWidget(instructions_header, 0, 0, 1, 2)

        instructions = (
            "\nCreate a custom gen when you want to play a romhack, or a modified\n"
            "version of an official game (e.g. hacking in a pokemon from a newer gen).\n"
            "A custom gen will re-use all the damage calculations and mechanics\n"
            "from the base official generation, but want to route with new or changed content.\n"
            "\nCreating a custom gen will copy all the information for the official gen\n"
            "into a new folder. Once created, click on the button below to open that\n"
            "location, and then modify the files as necessary.\n"
            "\nThe application loads all custom gens on startup, so when you modify\n"
            "the custom gen, you must restart the app before those changes will be recognized.\n"
            "\nNOTE: All custom gens are validated on app startup. If any errors are detected,\n"
            "you will get a pop-up.  The app will still work fine,\n"
            "but any custom gens with errors detected will not be loaded\n"
        )
        instructions_label = QLabel(instructions)
        instructions_label.setWordWrap(True)
        new_grid.addWidget(instructions_label, 1, 0, 1, 2)

        # Base version
        base_version_label = QLabel("Base Version:")
        self.base_version_value = SimpleOptionMenu(
            option_list=gen_factory.get_gen_names(real_gens=True, custom_gens=False),
        )
        new_grid.addWidget(base_version_label, 11, 0)
        new_grid.addWidget(self.base_version_value, 11, 1)

        # Custom name
        custom_name_label = QLabel("Custom Version Name:")
        self.custom_name_value = SimpleEntry(callback=self.on_custom_name_change)
        new_grid.addWidget(custom_name_label, 12, 0)
        new_grid.addWidget(self.custom_name_value, 12, 1)

        # Create button
        self.custom_create_button = SimpleButton("Create Custom Version")
        self.custom_create_button.clicked.connect(self.create_custom_gen)
        self.custom_create_button.disable()
        new_grid.addWidget(self.custom_create_button, 17, 0, 1, 2)

        main_layout.addWidget(self.new_version_frame)

        # === Existing custom gens section ===
        self.cur_custom_gens_frame = QWidget()
        self._gens_grid = QGridLayout(self.cur_custom_gens_frame)
        self._gens_grid.setContentsMargins(5, 20, 5, 10)
        self._gens_grid.setHorizontalSpacing(5)
        self._gens_grid.setVerticalSpacing(5)

        self.loaded_gens_label = QLabel("All Custom Gens")
        self.loaded_gens_label.setAlignment(Qt.AlignCenter)
        self._gens_grid.addWidget(self.loaded_gens_label, 0, 0, 1, 2)

        self._dynamic_widgets = []
        self._populate_custom_gens()

        main_layout.addWidget(self.cur_custom_gens_frame)

        # Close button
        self.close_button = SimpleButton("Close")
        self.close_button.clicked.connect(self.close)
        main_layout.addWidget(self.close_button, alignment=Qt.AlignCenter)

    def keyPressEvent(self, event):
        if event.key() == Qt.Key_Return or event.key() == Qt.Key_Enter:
            self.create_custom_gen()
        else:
            super().keyPressEvent(event)

    def _populate_custom_gens(self):
        for widget in self._dynamic_widgets:
            self._gens_grid.removeWidget(widget)
            widget.deleteLater()

        self._dynamic_widgets = []
        for cur_idx, (cur_path, cur_base_version, cur_custom_gen_name) in enumerate(gen_factory.get_all_custom_gen_info()):
            cur_label = QLabel(f"{cur_custom_gen_name} ({cur_base_version})")
            cur_label.setAlignment(Qt.AlignLeft | Qt.AlignVCenter)
            self._gens_grid.addWidget(cur_label, cur_idx + 1, 0)
            self._dynamic_widgets.append(cur_label)

            cur_button = QPushButton("Open Custom Gen")
            cur_button.clicked.connect(self._curry_open_dialog(cur_path))
            self._gens_grid.addWidget(cur_button, cur_idx + 1, 1)
            self._dynamic_widgets.append(cur_button)

    def on_custom_name_change(self, *args, **kwargs):
        if self._verify_custom_gen_name(self.custom_name_value.get()):
            self.custom_create_button.enable()
        else:
            self.custom_create_button.disable()

    def _verify_custom_gen_name(self, name):
        if not name:
            return False
        return name not in gen_factory.get_gen_names(real_gens=True, custom_gens=True)

    def create_custom_gen(self, *args, **kwargs):
        new_name = self.custom_name_value.get()
        if not self._verify_custom_gen_name(new_name):
            return

        self._controller.create_custom_version(self.base_version_value.get(), new_name)
        self._populate_custom_gens()

    def _curry_open_dialog(self, path_to_open):
        def inner():
            io_utils.open_explorer(path_to_open)
        return inner
