import os
import threading
import logging

from PySide6.QtWidgets import (
    QVBoxLayout, QGridLayout, QLabel, QWidget, QCheckBox, QFileDialog, QMessageBox,
)
from PySide6.QtCore import Qt

from gui_qt.dialogs.base_dialog import BaseDialog
from gui_qt.components.custom_components import SimpleButton
from utils.constants import const
from utils.config_manager import config
from utils import io_utils, auto_update

logger = logging.getLogger(__name__)


class DataDirConfigDialog(BaseDialog):
    """Dialog for application settings: version info, debug mode, data locations, and updates."""

    def __init__(self, parent, restart_callback, first_time_setup=False, **kwargs):
        super().__init__(parent, title="Application Settings", **kwargs)
        self._restart_callback = restart_callback
        self.first_time_setup = first_time_setup
        self._data_dir_changed = False
        self._thread = None
        self._new_app_version = None
        self._new_asset_url = None

        main_layout = QVBoxLayout(self)

        # === App Info Section ===
        app_info_frame = QWidget()
        info_grid = QGridLayout(app_info_frame)
        info_grid.setContentsMargins(5, 10, 5, 10)
        info_grid.setHorizontalSpacing(5)
        info_grid.setVerticalSpacing(5)

        # App version
        app_version_label = QLabel("App Version:")
        app_version_value = QLabel(const.APP_VERSION)
        info_grid.addWidget(app_version_label, 0, 0)
        info_grid.addWidget(app_version_value, 0, 1)

        # Release date
        app_release_date_label = QLabel("Release Date:")
        app_release_date_value = QLabel(const.APP_RELEASE_DATE)
        info_grid.addWidget(app_release_date_label, 1, 0)
        info_grid.addWidget(app_release_date_value, 1, 1)

        # Debug mode
        debug_mode_label = QLabel("Debug Logging when Recording:")
        self.debug_mode_checkbox = QCheckBox()
        self.debug_mode_checkbox.setChecked(config.is_debug_mode())
        self.debug_mode_checkbox.stateChanged.connect(self.toggle_debug_mode)
        info_grid.addWidget(debug_mode_label, 2, 0)
        info_grid.addWidget(self.debug_mode_checkbox, 2, 1)

        # Auto-update section
        windows_label = QLabel("Automatic updates only supported on windows machines")
        windows_label.setAlignment(Qt.AlignCenter)
        info_grid.addWidget(windows_label, 5, 0, 1, 2)

        self._latest_version_label = QLabel("Fetching newest version...")
        self._latest_version_label.setAlignment(Qt.AlignCenter)
        info_grid.addWidget(self._latest_version_label, 6, 0, 1, 2)

        self._check_for_updates_button = SimpleButton("No Upgrade Needed")
        self._check_for_updates_button.clicked.connect(self._kick_off_auto_update)
        self._check_for_updates_button.disable()
        info_grid.addWidget(self._check_for_updates_button, 7, 0, 1, 2)

        main_layout.addWidget(app_info_frame)

        # === Data Location Section ===
        data_frame = QWidget()
        data_grid = QGridLayout(data_frame)
        data_grid.setContentsMargins(5, 20, 5, 10)
        data_grid.setHorizontalSpacing(5)
        data_grid.setVerticalSpacing(5)

        # Data location
        self.data_location_value = QLabel(f"Data Location: {config.get_user_data_dir()}")
        self.data_location_value.setWordWrap(True)
        data_grid.addWidget(self.data_location_value, 15, 0, 1, 2)

        open_data_button = SimpleButton("Open Data Folder")
        open_data_button.clicked.connect(self.open_data_location)
        move_data_button = SimpleButton("Move Data Location")
        move_data_button.clicked.connect(self.change_data_location)
        data_grid.addWidget(open_data_button, 16, 0)
        data_grid.addWidget(move_data_button, 16, 1)

        # Image location
        self.image_location_value = QLabel(f"Image Location: {config.get_images_dir()}")
        self.image_location_value.setWordWrap(True)
        data_grid.addWidget(self.image_location_value, 17, 0, 1, 2)

        open_images_button = SimpleButton("Open Images Folder")
        open_images_button.clicked.connect(self.open_images_location)
        move_images_button = SimpleButton("Move Images Location")
        move_images_button.clicked.connect(self.change_image_location)
        data_grid.addWidget(open_images_button, 18, 0)
        data_grid.addWidget(move_images_button, 18, 1)

        # Config/logs folder
        app_location_button = SimpleButton("Open Config/Logs Folder")
        app_location_button.clicked.connect(self.open_global_config_location)
        data_grid.addWidget(app_location_button, 30, 0, 1, 2)

        main_layout.addWidget(data_frame)

        # Close button
        self.close_button = SimpleButton("Close")
        self.close_button.clicked.connect(self._final_cleanup)
        main_layout.addWidget(self.close_button, alignment=Qt.AlignCenter)

        # Start background version check
        self._thread = threading.Thread(target=self._get_new_updates_info, daemon=True)
        self._thread.start()

    def keyPressEvent(self, event):
        if event.key() == Qt.Key_Return or event.key() == Qt.Key_Enter:
            self._final_cleanup()
        else:
            super().keyPressEvent(event)

    def _final_cleanup(self, *args, **kwargs):
        if self.first_time_setup and not self._data_dir_changed:
            io_utils.change_user_data_location(None, config.get_user_data_dir())
        if self._thread is not None:
            self._thread.join(timeout=2)
        self.close()

    def _get_new_updates_info(self, *args, **kwargs):
        try:
            self._new_app_version, self._new_asset_url = auto_update.get_new_version_info()
            self._latest_version_label.setText(f"Newest Version: {self._new_app_version}")
            if auto_update.is_upgrade_needed(self._new_app_version, const.APP_VERSION):
                self._check_for_updates_button.setText("Upgrade")
                self._check_for_updates_button.enable()
        except Exception:
            self._latest_version_label.setText("Failed to fetch version info")

    def _kick_off_auto_update(self, *args, **kwargs):
        if self._thread is not None:
            self._thread.join(timeout=2)
        self._restart_callback()

    def open_global_config_location(self, *args, **kwargs):
        io_utils.open_explorer(const.GLOBAL_CONFIG_DIR)

    def open_data_location(self, *args, **kwargs):
        io_utils.open_explorer(config.get_user_data_dir())

    def open_images_location(self, *args, **kwargs):
        io_utils.open_explorer(config.get_images_dir())

    def toggle_debug_mode(self, *args, **kwargs):
        config.set_debug_mode(not config.is_debug_mode())

    def _change_location_helper(self, init_dir):
        logger.info(f"Trying to change location of init_dir: {init_dir}")
        valid_path_found = False
        while not valid_path_found:
            file_result = QFileDialog.getExistingDirectory(
                self, "Select Directory", init_dir,
            )
            if not file_result:
                return None

            new_path = os.path.realpath(file_result)
            if os.path.realpath(const.SOURCE_ROOT_PATH) in new_path:
                QMessageBox.critical(
                    self, "Error",
                    "Cannot place the dir inside the app, as it will be removed during automatic updates",
                )
            else:
                valid_path_found = True

        return new_path

    def change_data_location(self, *args, **kwargs):
        init_dir = config.get_user_data_dir()
        new_path = self._change_location_helper(init_dir)
        if new_path:
            logger.info(f"Trying to change data location to: {new_path}")
            if io_utils.change_user_data_location(init_dir, new_path):
                config.set_user_data_dir(new_path)
                self.data_location_value.setText(f"Data Location: {new_path}")
                self._data_dir_changed = True
            else:
                QMessageBox.critical(self, "Error", "Failed to change user data location...")

        self.raise_()

    def change_image_location(self, *args, **kwargs):
        init_dir = config.get_images_dir()
        new_path = self._change_location_helper(init_dir)
        if new_path:
            logger.info(f"Trying to change image location to: {new_path}")
            if io_utils.migrate_dir(init_dir, new_path):
                config.set_images_dir(new_path)
                self.image_location_value.setText(f"Image Location: {new_path}")
            else:
                QMessageBox.critical(self, "Error", "Failed to change image location...")

        self.raise_()
