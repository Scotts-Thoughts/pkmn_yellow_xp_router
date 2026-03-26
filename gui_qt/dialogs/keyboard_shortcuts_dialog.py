import json
import logging

from PySide6.QtWidgets import (
    QVBoxLayout, QHBoxLayout, QLabel, QPushButton, QLineEdit,
    QTreeWidget, QTreeWidgetItem, QHeaderView, QFileDialog,
    QMessageBox, QWidget, QAbstractItemView, QApplication,
)
from PySide6.QtCore import Qt, Signal, QEvent, QPoint
from PySide6.QtGui import QKeySequence, QMouseEvent

from gui_qt.dialogs.base_dialog import BaseDialog
from utils.config_manager import (
    config, DEFAULT_SHORTCUTS, SHORTCUT_LABELS, SHORTCUT_CATEGORIES,
)

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# KeySequenceEdit -- small widget that captures a key combination
# ---------------------------------------------------------------------------
class KeySequenceEdit(QLineEdit):
    """A line edit that captures the next key press as a QKeySequence."""

    key_sequence_changed = Signal(str)

    def __init__(self, initial_sequence="", parent=None):
        super().__init__(parent)
        self._sequence = initial_sequence
        self.setText(initial_sequence)
        self.setReadOnly(True)
        self.setAlignment(Qt.AlignCenter)
        self.setPlaceholderText("Press a key combination...")
        self._capturing = False
        self.setMouseTracking(True)
        self.installEventFilter(self)

    def eventFilter(self, obj, event):
        """Forward mouse-move events to the parent tree viewport so
        row hover highlighting stays in sync when the cursor passes
        over embedded KeySequenceEdit widgets."""
        if obj is self and event.type() == QEvent.MouseMove and not self._capturing:
            tree = self.parent()
            while tree is not None and not isinstance(tree, QTreeWidget):
                tree = tree.parent()
            if tree is not None:
                viewport = tree.viewport()
                mapped = self.mapTo(viewport, event.position().toPoint())
                forwarded = QMouseEvent(
                    QEvent.MouseMove,
                    mapped,
                    viewport.mapToGlobal(mapped),
                    event.button(),
                    event.buttons(),
                    event.modifiers(),
                )
                QApplication.sendEvent(viewport, forwarded)
        return super().eventFilter(obj, event)

    def sequence(self):
        return self._sequence

    def set_sequence(self, seq):
        self._sequence = seq
        self.setText(seq)

    def mousePressEvent(self, event):
        self._capturing = True
        self.setText("...")
        self.setStyleSheet("background-color: #3a5a8a;")
        self.setFocus()
        event.accept()

    def keyPressEvent(self, event):
        if not self._capturing:
            super().keyPressEvent(event)
            return

        key = event.key()
        # Ignore bare modifier keys
        if key in (Qt.Key_Control, Qt.Key_Shift, Qt.Key_Alt, Qt.Key_Meta, Qt.Key_unknown):
            return

        modifiers = event.modifiers()
        combo = int(modifiers) | key
        seq = QKeySequence(combo).toString()

        if seq:
            self._sequence = seq
            self.setText(seq)
            self.key_sequence_changed.emit(seq)

        self._capturing = False
        self.setStyleSheet("")
        self.clearFocus()

    def focusOutEvent(self, event):
        if self._capturing:
            # Cancelled -- restore previous value
            self.setText(self._sequence)
            self._capturing = False
            self.setStyleSheet("")
        super().focusOutEvent(event)


# ---------------------------------------------------------------------------
# KeyboardShortcutsDialog
# ---------------------------------------------------------------------------
class KeyboardShortcutsDialog(BaseDialog):
    """Dialog that lists all keyboard shortcuts and allows customisation."""

    shortcuts_changed = Signal()

    def __init__(self, parent=None, apply_callback=None):
        super().__init__(parent, title="Keyboard Shortcuts")
        self._apply_callback = apply_callback
        self._editors = {}  # action_id -> KeySequenceEdit
        self._pending_changes = {}  # action_id -> new key string

        self.setMinimumSize(620, 520)
        self.resize(660, 600)

        layout = QVBoxLayout(self)
        layout.setSpacing(6)

        # --- Search bar ---
        search_row = QHBoxLayout()
        search_row.addWidget(QLabel("Search:"))
        self._search_entry = QLineEdit()
        self._search_entry.setPlaceholderText("Filter shortcuts...")
        self._search_entry.textChanged.connect(self._apply_filter)
        search_row.addWidget(self._search_entry)
        layout.addLayout(search_row)

        # --- Shortcut tree ---
        self._tree = QTreeWidget()
        self._tree.setHeaderLabels(["Action", "Shortcut", "Default", ""])
        self._tree.setRootIsDecorated(True)
        self._tree.setAlternatingRowColors(True)
        self._tree.setSelectionMode(QAbstractItemView.NoSelection)
        header = self._tree.header()
        header.setStretchLastSection(False)
        header.setSectionResizeMode(0, QHeaderView.Stretch)
        header.setSectionResizeMode(1, QHeaderView.ResizeToContents)
        header.setSectionResizeMode(2, QHeaderView.ResizeToContents)
        header.setSectionResizeMode(3, QHeaderView.ResizeToContents)
        layout.addWidget(self._tree)

        self._build_tree()

        # --- Button rows ---
        btn_row_1 = QHBoxLayout()

        self._btn_apply = QPushButton("Apply")
        self._btn_apply.clicked.connect(self._apply_changes)
        btn_row_1.addWidget(self._btn_apply)

        self._btn_reset_all = QPushButton("Reset All to Defaults")
        self._btn_reset_all.clicked.connect(self._reset_all)
        btn_row_1.addWidget(self._btn_reset_all)

        layout.addLayout(btn_row_1)

        btn_row_2 = QHBoxLayout()

        self._btn_export = QPushButton("Export Profile...")
        self._btn_export.clicked.connect(self._export_profile)
        btn_row_2.addWidget(self._btn_export)

        self._btn_import = QPushButton("Import Profile...")
        self._btn_import.clicked.connect(self._import_profile)
        btn_row_2.addWidget(self._btn_import)

        self._btn_close = QPushButton("Close")
        self._btn_close.clicked.connect(self.close)
        btn_row_2.addWidget(self._btn_close)

        layout.addLayout(btn_row_2)

    # ------------------------------------------------------------------ build
    def _build_tree(self):
        self._tree.clear()
        self._editors.clear()

        for category, action_ids in SHORTCUT_CATEGORIES.items():
            cat_item = QTreeWidgetItem(self._tree, [category])
            cat_item.setFlags(Qt.ItemIsEnabled)
            font = cat_item.font(0)
            font.setBold(True)
            cat_item.setFont(0, font)

            for action_id in action_ids:
                label = SHORTCUT_LABELS.get(action_id, action_id)
                default_seq = DEFAULT_SHORTCUTS.get(action_id, "")
                current_seq = config.get_shortcut(action_id)

                child = QTreeWidgetItem(cat_item)
                child.setText(0, label)
                child.setData(0, Qt.UserRole, action_id)
                child.setText(2, default_seq)

                # Editable key-sequence widget
                editor = KeySequenceEdit(current_seq)
                editor.setFixedWidth(160)
                editor.key_sequence_changed.connect(
                    lambda seq, aid=action_id: self._on_shortcut_edited(aid, seq)
                )
                self._tree.setItemWidget(child, 1, editor)
                self._editors[action_id] = editor

                # Reset button
                reset_btn = QPushButton("Reset")
                reset_btn.setFixedWidth(52)
                reset_btn.clicked.connect(
                    lambda checked=False, aid=action_id: self._reset_single(aid)
                )
                self._tree.setItemWidget(child, 3, reset_btn)

                self._style_row(child, action_id, current_seq)

            cat_item.setExpanded(True)

    def _style_row(self, item, action_id, current_seq):
        """Highlight rows that differ from their default."""
        default_seq = DEFAULT_SHORTCUTS.get(action_id, "")
        is_custom = current_seq != default_seq
        if is_custom:
            for col in range(3):
                item.setForeground(col, Qt.cyan)
        else:
            for col in range(3):
                item.setData(col, Qt.ForegroundRole, None)

    # ------------------------------------------------------------ callbacks
    def _on_shortcut_edited(self, action_id, new_seq):
        self._pending_changes[action_id] = new_seq

    def _apply_changes(self):
        if not self._pending_changes:
            return

        # Check for duplicate key bindings
        all_shortcuts = config.get_all_shortcuts()
        all_shortcuts.update(self._pending_changes)

        # Build reverse map to detect duplicates
        reverse = {}
        duplicates = []
        for aid, seq in all_shortcuts.items():
            if not seq:
                continue
            if seq in reverse:
                duplicates.append((seq, SHORTCUT_LABELS.get(reverse[seq], reverse[seq]),
                                   SHORTCUT_LABELS.get(aid, aid)))
            else:
                reverse[seq] = aid

        if duplicates:
            lines = [f"  {seq}: \"{a}\" and \"{b}\"" for seq, a, b in duplicates]
            reply = QMessageBox.warning(
                self,
                "Duplicate Shortcuts",
                "The following key sequences are assigned to multiple actions:\n\n"
                + "\n".join(lines)
                + "\n\nApply anyway?",
                QMessageBox.Yes | QMessageBox.No,
                QMessageBox.No,
            )
            if reply != QMessageBox.Yes:
                return

        for action_id, seq in self._pending_changes.items():
            config.set_shortcut(action_id, seq)
        self._pending_changes.clear()

        # Refresh styling
        self._refresh_tree_styles()

        if self._apply_callback is not None:
            self._apply_callback()
        self.shortcuts_changed.emit()

    def _refresh_tree_styles(self):
        root = self._tree.invisibleRootItem()
        for i in range(root.childCount()):
            cat_item = root.child(i)
            for j in range(cat_item.childCount()):
                child = cat_item.child(j)
                action_id = child.data(0, Qt.UserRole)
                current_seq = config.get_shortcut(action_id)
                self._style_row(child, action_id, current_seq)

    def _reset_single(self, action_id):
        default_seq = DEFAULT_SHORTCUTS.get(action_id, "")
        config.reset_shortcut(action_id)
        self._pending_changes.pop(action_id, None)

        if action_id in self._editors:
            self._editors[action_id].set_sequence(default_seq)

        self._refresh_tree_styles()

        if self._apply_callback is not None:
            self._apply_callback()
        self.shortcuts_changed.emit()

    def _reset_all(self):
        reply = QMessageBox.question(
            self,
            "Reset All Shortcuts",
            "Reset all keyboard shortcuts to their defaults?",
            QMessageBox.Yes | QMessageBox.No,
            QMessageBox.No,
        )
        if reply != QMessageBox.Yes:
            return

        config.reset_all_shortcuts()
        self._pending_changes.clear()

        for action_id, editor in self._editors.items():
            editor.set_sequence(DEFAULT_SHORTCUTS.get(action_id, ""))

        self._refresh_tree_styles()

        if self._apply_callback is not None:
            self._apply_callback()
        self.shortcuts_changed.emit()

    # --------------------------------------------------------- filter
    def _apply_filter(self, text):
        text = text.lower()
        root = self._tree.invisibleRootItem()
        for i in range(root.childCount()):
            cat_item = root.child(i)
            any_visible = False
            for j in range(cat_item.childCount()):
                child = cat_item.child(j)
                label = child.text(0).lower()
                shortcut = self._editors.get(child.data(0, Qt.UserRole))
                seq = shortcut.sequence().lower() if shortcut else ""
                visible = text in label or text in seq
                child.setHidden(not visible)
                if visible:
                    any_visible = True
            cat_item.setHidden(not any_visible)

    # --------------------------------------------------------- export / import
    def _export_profile(self):
        path, _ = QFileDialog.getSaveFileName(
            self, "Export Shortcut Profile", "shortcuts.json",
            "JSON Files (*.json);;All Files (*)",
        )
        if not path:
            return
        try:
            config.export_shortcuts(path)
            QMessageBox.information(self, "Export", f"Shortcuts exported to:\n{path}")
        except Exception as e:
            QMessageBox.critical(self, "Export Error", str(e))

    def _import_profile(self):
        path, _ = QFileDialog.getOpenFileName(
            self, "Import Shortcut Profile", "",
            "JSON Files (*.json);;All Files (*)",
        )
        if not path:
            return
        try:
            config.import_shortcuts(path)
        except Exception as e:
            QMessageBox.critical(self, "Import Error", str(e))
            return

        self._pending_changes.clear()
        for action_id, editor in self._editors.items():
            editor.set_sequence(config.get_shortcut(action_id))

        self._refresh_tree_styles()

        if self._apply_callback is not None:
            self._apply_callback()
        self.shortcuts_changed.emit()

        QMessageBox.information(self, "Import", "Shortcut profile imported successfully.")
