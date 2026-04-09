import logging

from PySide6.QtWidgets import (
    QVBoxLayout, QHBoxLayout, QLabel, QWidget, QListWidget, QListWidgetItem,
    QLineEdit,
)
from PySide6.QtCore import Qt

from gui_qt.dialogs.base_dialog import BaseDialog
from gui_qt.components.custom_components import SimpleButton, SimpleOptionMenu
from utils.config_manager import config
from pkmn import gen_factory

logger = logging.getLogger(__name__)


class FinalTrainersDialog(BaseDialog):
    """Dialog for configuring which trainer(s) count as the 'final trainer' for each
    game version. When recording, defeating any of these trainers automatically
    stops recording so additional events aren't accidentally captured."""

    def __init__(self, parent, **kwargs):
        super().__init__(parent, title="Configure Final Trainers", **kwargs)

        # Get all available game versions
        self._all_versions = gen_factory._gen_factory.get_gen_names()
        if not self._all_versions:
            self._all_versions = []

        # Default to currently-loaded version if available
        default_version = None
        try:
            cur_gen = gen_factory.current_gen_info()
            if cur_gen is not None:
                # find the name of the current gen
                for name in self._all_versions:
                    if gen_factory.specific_gen_info(name) is cur_gen:
                        default_version = name
                        break
        except Exception:
            default_version = None
        if default_version is None and self._all_versions:
            default_version = self._all_versions[0]

        outer = QVBoxLayout(self)

        # Help text
        help_label = QLabel(
            "Pick the trainer(s) that mark the end of a run for each game.\n"
            "While recording, defeating any of these trainers will automatically "
            "turn recording off so no extra events are captured."
        )
        help_label.setWordWrap(True)
        outer.addWidget(help_label)

        # Game version selector
        version_row = QWidget()
        version_layout = QHBoxLayout(version_row)
        version_layout.setContentsMargins(0, 5, 0, 5)
        version_layout.addWidget(QLabel("Game Version:"))
        self._version_menu = SimpleOptionMenu(
            option_list=self._all_versions,
            default_val=default_version,
            callback=self._on_version_changed,
        )
        version_layout.addWidget(self._version_menu, stretch=1)
        outer.addWidget(version_row)

        # Filter
        filter_row = QWidget()
        filter_layout = QHBoxLayout(filter_row)
        filter_layout.setContentsMargins(0, 0, 0, 5)
        filter_layout.addWidget(QLabel("Filter:"))
        self._filter_entry = QLineEdit()
        self._filter_entry.setPlaceholderText("Type to filter trainers by name")
        self._filter_entry.textChanged.connect(self._apply_filter)
        filter_layout.addWidget(self._filter_entry, stretch=1)
        outer.addWidget(filter_row)

        # Trainer list (checkable)
        self._trainer_list = QListWidget()
        self._trainer_list.itemChanged.connect(self._on_item_changed)
        outer.addWidget(self._trainer_list, stretch=1)

        # Selected count + clear button
        bottom_row = QWidget()
        bottom_layout = QHBoxLayout(bottom_row)
        bottom_layout.setContentsMargins(0, 5, 0, 0)
        self._count_label = QLabel("")
        bottom_layout.addWidget(self._count_label, stretch=1)

        self._reset_button = SimpleButton("Reset to Defaults")
        self._reset_button.clicked.connect(self._reset_current_game)
        bottom_layout.addWidget(self._reset_button)

        self._clear_button = SimpleButton("Clear All For This Game")
        self._clear_button.clicked.connect(self._clear_current_game)
        bottom_layout.addWidget(self._clear_button)

        self._close_button = SimpleButton("Close")
        self._close_button.clicked.connect(self.close)
        bottom_layout.addWidget(self._close_button)

        outer.addWidget(bottom_row)

        # Re-entrancy guard so save logic doesn't fire while we populate
        self._loading = False
        # Map row -> trainer name (since filter hides items)
        self._populate_trainers()

        self.resize(520, 600)

    def _on_version_changed(self, *args, **kwargs):
        self._populate_trainers()

    def _current_version(self):
        return self._version_menu.get()

    def _populate_trainers(self):
        self._loading = True
        try:
            self._trainer_list.clear()
            version = self._current_version()
            if not version:
                self._update_count_label([], [])
                return

            gen_info = gen_factory.specific_gen_info(version)
            if gen_info is None:
                self._update_count_label([], [])
                return

            try:
                trainer_db = gen_info.trainer_db()
            except Exception as e:
                logger.error(f"Failed to load trainer db for {version}: {e}")
                self._update_count_label([], [])
                return

            trainers = trainer_db.get_valid_trainers()
            trainers = sorted(set(trainers))

            already_selected = set(config.get_final_trainers(version))

            for trainer_name in trainers:
                item = QListWidgetItem(trainer_name)
                item.setFlags(item.flags() | Qt.ItemIsUserCheckable)
                checked = trainer_name in already_selected
                item.setCheckState(Qt.Checked if checked else Qt.Unchecked)
                self._trainer_list.addItem(item)

            self._update_count_label(trainers, list(already_selected))
            self._apply_filter(self._filter_entry.text())
        finally:
            self._loading = False

    def _apply_filter(self, text=None):
        if text is None:
            text = self._filter_entry.text()
        needle = text.strip().lower()
        for i in range(self._trainer_list.count()):
            item = self._trainer_list.item(i)
            if not needle or needle in item.text().lower():
                item.setHidden(False)
            else:
                item.setHidden(True)

    def _on_item_changed(self, item):
        if self._loading:
            return
        version = self._current_version()
        if not version:
            return
        # Collect all checked trainer names from the list
        selected = []
        for i in range(self._trainer_list.count()):
            it = self._trainer_list.item(i)
            if it.checkState() == Qt.Checked:
                selected.append(it.text())
        config.set_final_trainers(version, selected)
        self._update_count_label_from_state()

    def _clear_current_game(self):
        version = self._current_version()
        if not version:
            return
        self._loading = True
        try:
            for i in range(self._trainer_list.count()):
                self._trainer_list.item(i).setCheckState(Qt.Unchecked)
        finally:
            self._loading = False
        config.set_final_trainers(version, [])
        self._update_count_label_from_state()

    def _reset_current_game(self):
        """Forget any user override for this game, restoring built-in defaults
        and re-checking the corresponding trainers in the list."""
        version = self._current_version()
        if not version:
            return
        config.reset_final_trainers(version)
        self._populate_trainers()

    def _update_count_label(self, all_trainers, selected_trainers):
        total = len(all_trainers)
        sel = len([t for t in selected_trainers if t in set(all_trainers)])
        self._count_label.setText(f"Selected: {sel} / {total}")

    def _update_count_label_from_state(self):
        total = self._trainer_list.count()
        sel = sum(
            1 for i in range(total)
            if self._trainer_list.item(i).checkState() == Qt.Checked
        )
        self._count_label.setText(f"Selected: {sel} / {total}")
