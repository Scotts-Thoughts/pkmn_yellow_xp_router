from PySide6.QtWidgets import QGridLayout, QLabel
from PySide6.QtCore import Qt

from gui_qt.dialogs.base_dialog import BaseDialog
from gui_qt.components.custom_components import SimpleButton, SimpleEntry, SimpleOptionMenu
from utils.constants import const


class TransferEventDialog(BaseDialog):
    """Dialog for transferring events to an existing or new folder."""

    def __init__(self, parent, controller, all_existing_folders, valid_dest_folders, event_ids, **kwargs):
        super().__init__(parent, title="Transfer Events", **kwargs)
        self._controller = controller
        self._all_existing_folders = all_existing_folders
        self._valid_dest_folders = valid_dest_folders
        self._event_ids = event_ids

        self._grid = QGridLayout(self)
        self._grid.setContentsMargins(10, 10, 10, 10)
        self._grid.setHorizontalSpacing(10)
        self._grid.setVerticalSpacing(10)

        # Transfer type selection
        self._transfer_type_label = QLabel("Transfer to:")
        self._transfer_type = SimpleOptionMenu(
            option_list=[const.TRANSFER_EXISTING_FOLDER, const.TRANSFER_NEW_FOLDER],
            callback=self._transfer_type_callback,
        )
        self._grid.addWidget(self._transfer_type_label, 0, 0)
        self._grid.addWidget(self._transfer_type, 0, 1)

        # New folder name entry (shown when "New Folder" selected)
        self._new_folder_label = QLabel("New folder:")
        self._new_folder_name = SimpleEntry(callback=self._new_folder_callback)
        self._new_folder_label.setVisible(False)
        self._new_folder_name.setVisible(False)

        # Dest folder dropdown (shown when "Existing Folder" selected)
        self._dest_folder_label = QLabel("Destination folder:")
        self._dest_folder_name = SimpleOptionMenu(option_list=self.get_possible_folders())

        # Filter entry
        self.filter_label = QLabel("Filter:")
        self.filter = SimpleEntry(callback=self._filter_callback)

        # Place all widgets in grid (visibility controlled separately)
        self._grid.addWidget(self._dest_folder_label, 1, 0)
        self._grid.addWidget(self._dest_folder_name, 1, 1)
        self._grid.addWidget(self.filter_label, 2, 0)
        self._grid.addWidget(self.filter, 2, 1)
        self._grid.addWidget(self._new_folder_label, 1, 0)
        self._grid.addWidget(self._new_folder_name, 1, 1)

        # Buttons
        self._add_button = SimpleButton("Transfer to Folder")
        self._add_button.clicked.connect(self.transfer)
        self._cancel_button = SimpleButton("Cancel")
        self._cancel_button.clicked.connect(self.close)
        self._grid.addWidget(self._add_button, 3, 0)
        self._grid.addWidget(self._cancel_button, 3, 1)

        self._transfer_type_callback()
        self.filter.setFocus()

    def keyPressEvent(self, event):
        if event.key() == Qt.Key_Return or event.key() == Qt.Key_Enter:
            self.transfer()
        else:
            super().keyPressEvent(event)

    def get_possible_folders(self, filter_text=""):
        filter_text = filter_text.lower()
        result = [x for x in self._valid_dest_folders if filter_text in x.lower()]

        if not result:
            result = [const.NO_FOLDERS]

        return result

    def _transfer_type_callback(self, *args, **kwargs):
        if self._transfer_type.get() == const.TRANSFER_EXISTING_FOLDER:
            self._dest_folder_label.setVisible(True)
            self._dest_folder_name.setVisible(True)
            self.filter_label.setVisible(True)
            self.filter.setVisible(True)
            self._new_folder_label.setVisible(False)
            self._new_folder_name.setVisible(False)
            self._filter_callback()
        else:
            self._dest_folder_label.setVisible(False)
            self._dest_folder_name.setVisible(False)
            self.filter_label.setVisible(False)
            self.filter.setVisible(False)
            self._new_folder_label.setVisible(True)
            self._new_folder_name.setVisible(True)
            self._new_folder_callback()

    def _filter_callback(self, *args, **kwargs):
        dest_folder_vals = self.get_possible_folders(filter_text=self.filter.get())
        if const.NO_FOLDERS in dest_folder_vals:
            self._add_button.disable()
        else:
            self._add_button.enable()

        self._dest_folder_name.new_values(dest_folder_vals)

    def _new_folder_callback(self, *args, **kwargs):
        if self._new_folder_name.get() in self._all_existing_folders:
            self._add_button.disable()
        else:
            self._add_button.enable()

    def transfer(self, *args, **kwargs):
        if self._transfer_type.get() == const.TRANSFER_EXISTING_FOLDER:
            if self._dest_folder_name.get() != const.NO_FOLDERS:
                self.close()
                self._controller.transfer_to_folder(self._event_ids, self._dest_folder_name.get())
        else:
            if self._new_folder_name.get() not in self._all_existing_folders:
                self.close()
                self._controller.transfer_to_folder(self._event_ids, self._new_folder_name.get().strip())
