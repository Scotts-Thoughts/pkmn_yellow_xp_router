import logging
from typing import List, Optional, Set, Dict

from PySide6.QtWidgets import (
    QTreeView, QAbstractItemView, QHeaderView,
    QLineEdit, QStyleOptionViewItem, QStyle, QStyledItemDelegate,
    QPushButton,
)
from PySide6.QtCore import Qt, QModelIndex, Signal, QItemSelectionModel, QEvent, QRect, QTimer, QSize
from PySide6.QtGui import (
    QStandardItemModel, QStandardItem, QColor, QBrush, QPalette,
)

from controllers.main_controller import MainController
from routing import route_events
from utils.constants import const
from utils.config_manager import config

logger = logging.getLogger(__name__)


# Column definitions matching the original CustomGridview column data.
# Each entry: (header_text, attr_name, width, hidden)
_COLUMN_DEFS = [
    ("Name",         "name",                         325,   False),
    ("Levels Up",    "get_pkmn_after_levelups",      114,   False),
    ("Level",        "pkmn_level",                    50,    False),
    ("% TNL",        "percent_xp_to_next_level",       -1,    False),
    ("Exp",          "total_xp",                      48,    False),
    ("Exp/sec",      "experience_per_second",          -1,    False),
    ("Exp Gain",     "xp_gain",                        -1,    False),
    ("ToNextLevel",  "xp_to_next_level",               -1,    False),
    ("LvlsGained",   "level_gain",                     -1,    False),
    ("event_id",     "group_id",                        0,    True),
]

# Column index constants for readability
_COL_NAME = 0
_COL_EVENT_ID = len(_COLUMN_DEFS) - 1


# Tag-to-background color mapping (static tags).
_STATIC_TAG_COLORS = {
    const.EVENT_TAG_ERRORS: QColor("#61520f"),
    const.EVENT_TAG_IMPORTANT: QColor("#1f1f1f"),
    const.HIGHLIGHT_LABEL: QColor("#156152"),
    const.EVENT_TAG_BRANCHED_MANDATORY: QColor("#5a5142"),
}


def _lighten_color(color: QColor, amount: float = 0.25) -> QColor:
    """Return a lighter version of *color* by blending toward white."""
    r = min(255, int(color.red()   + (255 - color.red())   * amount))
    g = min(255, int(color.green() + (255 - color.green()) * amount))
    b = min(255, int(color.blue()  + (255 - color.blue())  * amount))
    return QColor(r, g, b, color.alpha())


# Default hover color used when a row has no custom background.
_DEFAULT_HOVER_COLOR = None  # Computed lazily from config background.


def _get_default_hover_color() -> QColor:
    global _DEFAULT_HOVER_COLOR
    if _DEFAULT_HOVER_COLOR is None:
        bg = QColor(config.get_background_color())
        _DEFAULT_HOVER_COLOR = _lighten_color(bg, 0.10)
    return _DEFAULT_HOVER_COLOR




def _get_attr(obj, attr_name):
    """Retrieve an attribute from *obj*, calling it if callable."""
    val = getattr(obj, attr_name, None)
    if val is None:
        return None
    if callable(val):
        return val()
    return val


_QUANTITY_SUFFIX_ROLE = Qt.UserRole + 100


class _QuantityDelegate(QStyledItemDelegate):
    """Delegate for the Name column that styles quantity suffixes (e.g. 'x3')
    in blue with an underline to indicate they are clickable."""

    _SUFFIX_COLOR = QColor("#4da6ff")
    _SUFFIX_COLOR_DIMMED = QColor("#5a7a99")

    def paint(self, painter, option, index):
        suffix = index.data(_QUANTITY_SUFFIX_ROLE)
        if not suffix:
            super().paint(painter, option, index)
            return

        opt = QStyleOptionViewItem(option)
        self.initStyleOption(opt, index)
        style = opt.widget.style()

        # Draw everything (background, checkbox, icon, focus) except text.
        saved_text = opt.text
        opt.text = ""
        style.drawControl(QStyle.ControlElement.CE_ItemViewItem, opt, painter, opt.widget)
        opt.text = saved_text

        # Text bounding rect.
        text_rect = style.subElementRect(
            QStyle.SubElement.SE_ItemViewItemText, opt, opt.widget
        )

        full_text = opt.text
        prefix = full_text[:-len(suffix)]
        fm = painter.fontMetrics()
        prefix_width = fm.horizontalAdvance(prefix)

        # Determine base text color and whether it is dimmed (e.g. unchecked).
        fg_data = index.data(Qt.ForegroundRole)
        if isinstance(fg_data, QBrush):
            base_color = fg_data.color()
            is_dimmed = True
        elif isinstance(fg_data, QColor):
            base_color = fg_data
            is_dimmed = True
        else:
            base_color = opt.palette.color(QPalette.ColorRole.Text)
            is_dimmed = False

        painter.save()

        # Draw prefix in the normal text color.
        painter.setPen(base_color)
        painter.drawText(text_rect, Qt.AlignVCenter | Qt.AlignLeft, prefix)

        # Draw suffix in blue + underline.
        suffix_rect = QRect(
            text_rect.x() + prefix_width,
            text_rect.y(),
            text_rect.width() - prefix_width,
            text_rect.height(),
        )
        font = painter.font()
        font.setUnderline(True)
        painter.setFont(font)
        painter.setPen(self._SUFFIX_COLOR_DIMMED if is_dimmed else self._SUFFIX_COLOR)
        painter.drawText(suffix_rect, Qt.AlignVCenter | Qt.AlignLeft, suffix)

        painter.restore()


class RouteList(QTreeView):
    """QTreeView-based replacement for the Tk RouteList / CustomGridview / CheckboxTreeview."""

    # Emitted after a full refresh so other widgets can react.
    route_list_refreshed = Signal()

    def __init__(self, controller: MainController, parent=None):
        super().__init__(parent)
        self._controller = controller

        # --- model --------------------------------------------------------
        self._model = QStandardItemModel(self)
        self._model.setHorizontalHeaderLabels([c[0] for c in _COLUMN_DEFS])
        self.setModel(self._model)
        self.setItemDelegateForColumn(_COL_NAME, _QuantityDelegate(self))

        # --- tree decoration & indentation --------------------------------
        self.setRootIsDecorated(True)
        self.setItemsExpandable(True)
        self.setIndentation(16)

        # --- scroll bars --------------------------------------------------
        self.setHorizontalScrollBarPolicy(Qt.ScrollBarAlwaysOn)

        # --- hover tracking -----------------------------------------------
        self._init_hover_tracking()
        self.setMouseTracking(True)
        self.viewport().setMouseTracking(True)

        # --- inline "+" button overlay ------------------------------------
        self._plus_button = QPushButton("+", self.viewport())
        self._plus_button.setFixedSize(16, 16)
        self._plus_button.setCursor(Qt.PointingHandCursor)
        self._plus_button.setFocusPolicy(Qt.NoFocus)
        self._plus_button.setStyleSheet(
            "QPushButton { background: #0078d4; color: white; border: none;"
            " border-radius: 8px; font-size: 12px; font-weight: bold;"
            " padding: 0; margin: 0; }"
            "QPushButton:hover { background: #1a8ae8; }"
        )
        self._plus_button.hide()
        self._plus_button.clicked.connect(self._on_plus_clicked)
        self._plus_button.installEventFilter(self)
        self._plus_btn_hide_timer = QTimer(self)
        self._plus_btn_hide_timer.setSingleShot(True)
        self._plus_btn_hide_timer.setInterval(150)
        self._plus_btn_hide_timer.timeout.connect(self._maybe_hide_plus_button)

        # --- inline event creator -----------------------------------------
        self._inline_creator = None
        self._inline_after_id = None
        self._inline_inside_folder_id = None  # when set, inline creator is
                                              # placed as first child of this
                                              # folder instead of after a row.
        self._inline_row_item = None  # spacer row in model
        self._editing_group_id = None
        self._editing_original_enabled = None

        # --- empty-state overlay ("Add New Event" button) -----------------
        self._empty_state_btn = QPushButton("Add New Event", self.viewport())
        self._empty_state_btn.setCursor(Qt.PointingHandCursor)
        self._empty_state_btn.setFocusPolicy(Qt.NoFocus)
        self._empty_state_btn.setStyleSheet(
            "QPushButton { background: #0078d4; color: white; border: none;"
            " border-radius: 4px; font-size: 14px; font-weight: bold;"
            " padding: 10px 24px; }"
            "QPushButton:hover { background: #1a8ae8; }"
        )
        self._empty_state_btn.hide()
        self._empty_state_btn.clicked.connect(self.start_add_new_event)

        # --- selection mode -----------------------------------------------
        self.setSelectionMode(QAbstractItemView.ExtendedSelection)
        self.setSelectionBehavior(QAbstractItemView.SelectRows)

        # --- drag-and-drop ------------------------------------------------
        self.setDragEnabled(True)
        self.setAcceptDrops(True)
        self.setDropIndicatorShown(True)
        self.setDragDropMode(QAbstractItemView.InternalMove)
        self.setDefaultDropAction(Qt.MoveAction)
        self._drag_source_ids: List[int] = []

        # --- columns ------------------------------------------------------
        header = self.header()
        for idx, (_, _, width, hidden) in enumerate(_COLUMN_DEFS):
            if hidden:
                self.setColumnHidden(idx, True)
            elif width == -1:
                header.setSectionResizeMode(idx, QHeaderView.ResizeToContents)
            elif width:
                self.setColumnWidth(idx, width)
                header.setSectionResizeMode(idx, QHeaderView.Fixed)
            else:
                header.setSectionResizeMode(idx, QHeaderView.Stretch)

        # The first column (tree/name) and Levels Up column are user-resizable.
        header.setSectionResizeMode(_COL_NAME, QHeaderView.Interactive)
        header.setSectionResizeMode(1, QHeaderView.Interactive)
        header.setStretchLastSection(True)

        # --- internal bookkeeping -----------------------------------------
        # Maps semantic group_id -> QModelIndex (persistent) stored as
        # (parent_group_id_or_None, row) -- but we just keep QStandardItem refs.
        self._item_lookup: Dict[int, QStandardItem] = {}

        # Persistent folder expand state, keyed by folder name path. Survives
        # refreshes that drop/recreate folders (e.g. flatten-style filters).
        self._persistent_expand_state: Dict[str, bool] = {}

        # Tracks whether the last refresh ran in flatten mode (Major Battles
        # filter active). Used by refresh_filter_only() to decide whether
        # the tree structure changed and a full refresh is required.
        self._last_used_flatten_filter: bool = False

        # Guard against recursive refresh (expand/collapse signals during rebuild).
        self._refreshing: bool = False

        # Cache for highlight colors so we don't re-query config every cell.
        self._highlight_colors: Dict[str, QColor] = {}
        self._folder_fg_color: Optional[QColor] = None
        self._update_highlight_colors()
        self._update_folder_text_color()

        # --- popovers (created on first use) ------------------------------
        self._quick_add_popover = None

        # --- signals / events ---------------------------------------------
        self.expanded.connect(self._on_item_expanded)
        self.collapsed.connect(self._on_item_collapsed)
        self._model.itemChanged.connect(self._on_item_changed)
        self.selectionModel().selectionChanged.connect(self._on_selection_changed)

        # --- inline quantity editor ----------------------------------------
        self._quantity_editor: Optional[QLineEdit] = None
        self._quantity_group_id: Optional[int] = None
        self.verticalScrollBar().valueChanged.connect(self._cancel_quantity_edit)
        self.horizontalScrollBar().valueChanged.connect(self._cancel_quantity_edit)
        self.verticalScrollBar().valueChanged.connect(self._reposition_inline_creator)

    # ------------------------------------------------------------------
    # Selection change → notify controller
    # ------------------------------------------------------------------

    def _on_selection_changed(self, selected, deselected):
        """Report the new selection to the controller when the user clicks."""
        cur_selected = self.get_all_selected_event_ids()
        if self._controller.get_all_selected_ids() != cur_selected:
            self._controller.select_new_events(cur_selected)

    # ------------------------------------------------------------------
    #  Style helpers
    # ------------------------------------------------------------------

    def update_folder_text_style(self):
        """Re-read the fade-folder-text config and repaint existing folder rows."""
        self._update_folder_text_color()
        # Walk all tracked items and reapply folder foreground.
        for group_id, name_item in self._item_lookup.items():
            event_obj = self._controller.get_event_by_id(group_id)
            if isinstance(event_obj, route_events.EventFolder):
                self._apply_folder_foreground(name_item)

    def _update_folder_text_color(self):
        if config.get_fade_folder_text():
            self._folder_fg_color = QColor("#666666")
        else:
            self._folder_fg_color = None

    def _update_highlight_colors(self):
        """Cache highlight colors from config."""
        global _DEFAULT_HOVER_COLOR
        _DEFAULT_HOVER_COLOR = None  # Reset so it re-derives from config

        self._highlight_colors.clear()
        for idx, label in enumerate(const.ALL_HIGHLIGHT_LABELS, 1):
            self._highlight_colors[label] = QColor(config.get_highlight_color(idx))
        # Fight category colors from config.
        for cat, tag in const.FIGHT_CATEGORY_TO_TAG.items():
            self._highlight_colors[tag] = QColor(config.get_fight_category_color(cat))
        # Also keep the static tag colors.
        for tag, color in _STATIC_TAG_COLORS.items():
            self._highlight_colors[tag] = color

    # ------------------------------------------------------------------
    #  Checkbox handling
    # ------------------------------------------------------------------

    def _on_item_changed(self, item: QStandardItem):
        """Called whenever any item data changes -- we only care about checkbox toggles."""
        if item.column() != _COL_NAME:
            return
        if not item.isCheckable():
            return

        group_id = self._group_id_from_name_item(item)
        if group_id is None:
            return

        new_enabled = item.checkState() != Qt.Unchecked
        event_obj = self._controller.get_event_by_id(group_id)
        if event_obj is None:
            return

        event_obj.set_enabled_status(new_enabled)
        self._controller.update_existing_event(event_obj.group_id, event_obj.event_definition)

    def trigger_checkbox(self, single_item=None):
        """Toggle the checkbox for the currently selected rows (or a single item)."""
        if single_item is not None:
            items = [single_item] if isinstance(single_item, QStandardItem) else []
        else:
            items = self._selected_name_items()
            if not items:
                return

        # Block signals so we can batch the changes and do a single controller update.
        self._model.blockSignals(True)
        try:
            for name_item in items:
                if not name_item.isCheckable():
                    continue
                if name_item.checkState() == Qt.Unchecked:
                    name_item.setCheckState(Qt.Checked)
                else:
                    name_item.setCheckState(Qt.Unchecked)
        finally:
            self._model.blockSignals(False)

        # Now fire the callback for each toggled item.
        for name_item in items:
            if not name_item.isCheckable():
                continue
            group_id = self._group_id_from_name_item(name_item)
            if group_id is None:
                continue
            new_enabled = name_item.checkState() != Qt.Unchecked
            event_obj = self._controller.get_event_by_id(group_id)
            if event_obj is None:
                continue
            event_obj.set_enabled_status(new_enabled)
            self._controller.update_existing_event(event_obj.group_id, event_obj.event_definition)

    # ------------------------------------------------------------------
    #  Expand / collapse tracking
    # ------------------------------------------------------------------

    def _on_item_expanded(self, index: QModelIndex):
        if self._refreshing:
            return
        item = self._model.itemFromIndex(index)
        if item is None:
            return
        group_id = self._group_id_from_name_item(item)
        if group_id is None:
            return
        event_obj = self._controller.get_event_by_id(group_id)
        if isinstance(event_obj, route_events.EventFolder):
            event_obj.expanded = True
            self.refresh()

    def _on_item_collapsed(self, index: QModelIndex):
        if self._refreshing:
            return
        item = self._model.itemFromIndex(index)
        if item is None:
            return
        group_id = self._group_id_from_name_item(item)
        if group_id is None:
            return
        event_obj = self._controller.get_event_by_id(group_id)
        if isinstance(event_obj, route_events.EventFolder):
            event_obj.expanded = False
            self.refresh()

    # ------------------------------------------------------------------
    #  Click handling
    # ------------------------------------------------------------------

    def mousePressEvent(self, event):
        """Override to handle left-click focus, quantity click, and right-click toggle."""
        if event.button() == Qt.LeftButton:
            self._unregister_text_field_focus()
            self._close_quantity_editor()
            super().mousePressEvent(event)
            self._check_quantity_click(event.pos())
            return

        if event.button() == Qt.RightButton:
            self._close_quantity_editor()
            self._unregister_text_field_focus()
            index = self.indexAt(event.pos())
            if not index.isValid():
                return

            # Preserve current selection.
            saved_selection = self.selectionModel().selection()
            # Get the name-column item for the clicked row.
            name_index = index.sibling(index.row(), _COL_NAME)
            name_item = self._model.itemFromIndex(name_index)
            if name_item is not None and name_item.isCheckable():
                self._model.blockSignals(True)
                try:
                    if name_item.checkState() == Qt.Unchecked:
                        name_item.setCheckState(Qt.Checked)
                    else:
                        name_item.setCheckState(Qt.Unchecked)
                finally:
                    self._model.blockSignals(False)

                # Fire the controller update.
                group_id = self._group_id_from_name_item(name_item)
                if group_id is not None:
                    event_obj = self._controller.get_event_by_id(group_id)
                    if event_obj is not None:
                        new_enabled = name_item.checkState() != Qt.Unchecked
                        event_obj.set_enabled_status(new_enabled)
                        self._controller.update_existing_event(
                            event_obj.group_id, event_obj.event_definition
                        )

            # Restore previous selection so right-click doesn't change it.
            self.selectionModel().select(
                saved_selection,
                QItemSelectionModel.ClearAndSelect | QItemSelectionModel.Rows,
            )
            return

        super().mousePressEvent(event)

    def mouseDoubleClickEvent(self, event):
        """Double-click an EventGroup to edit it inline."""
        if event.button() != Qt.LeftButton:
            super().mouseDoubleClickEvent(event)
            return

        index = self.indexAt(event.pos())
        if not index.isValid():
            super().mouseDoubleClickEvent(event)
            return

        name_index = index.sibling(index.row(), _COL_NAME)
        item = self._model.itemFromIndex(name_index)
        if item is None:
            super().mouseDoubleClickEvent(event)
            return

        group_id = self._group_id_from_name_item(item)
        if group_id is None:
            super().mouseDoubleClickEvent(event)
            return

        event_obj = self._controller.get_event_by_id(group_id)

        # Only edit EventGroups — not folders or sub-items.
        if not isinstance(event_obj, route_events.EventGroup):
            super().mouseDoubleClickEvent(event)
            return

        # Skip levelup moves (auto-generated) and unmapped types.
        from gui_qt.components.inline_event_creator import _EVENT_TYPE_TO_KEY
        event_type = event_obj.event_definition.get_event_type()
        if event_type not in _EVENT_TYPE_TO_KEY:
            super().mouseDoubleClickEvent(event)
            return

        # Already editing this exact event — ignore.
        if self._editing_group_id == group_id:
            return

        self._start_inline_edit(group_id, event_obj)
        # Don't call super — prevent expand/collapse on double-click.

    # ------------------------------------------------------------------
    #  Quick-add popover (Space key)
    # ------------------------------------------------------------------

    _QUICK_ADD_KEYS = {
        Qt.Key_Q: 0,  # Trainer
        Qt.Key_W: 1,  # Item
        Qt.Key_E: 2,  # Move
        Qt.Key_R: 3,  # Wild Pkmn
        Qt.Key_T: 4,  # Misc
    }

    def keyPressEvent(self, event):
        if not event.modifiers():
            if event.key() == Qt.Key_Space:
                # Space → open inline event creator after the selected row.
                sel_id = self._controller.get_single_selected_event_id()
                if sel_id is not None:
                    self._restore_editing_state()
                    self._remove_inline_creator()
                    self._show_inline_creator(sel_id)
                    return
            elif event.key() in self._QUICK_ADD_KEYS:
                if self._controller.can_insert_after_current_selection():
                    indexes = self.selectionModel().selectedRows(_COL_NAME)
                    if indexes:
                        category = self._QUICK_ADD_KEYS[event.key()]
                        self._show_quick_add_popover(indexes[-1], category)
                        return
        super().keyPressEvent(event)

    # ------------------------------------------------------------------
    #  Public entry point for "Add New Event"
    # ------------------------------------------------------------------

    def start_add_new_event(self):
        """Open the inline creator. Behaves like the spacebar shortcut for
        non-empty routes. For an empty route, creates a new unnamed folder
        and starts inline creation inside it so the user gets their first
        event wrapped in a folder automatically."""
        if self._controller.is_empty():
            raw_route = self._controller.get_raw_route()
            if raw_route is None:
                return
            folder_name = self._unique_unnamed_folder_name()
            self._controller.finalize_new_folder(folder_name)
            folder = self._controller.get_raw_route().folder_lookup.get(folder_name)
            if folder is None:
                return
            self._restore_editing_state()
            self._remove_inline_creator()
            self._show_inline_creator(
                insert_after_id=None,
                dest_folder_name=folder_name,
                inside_folder_id=folder.group_id,
            )
            return

        sel_id = self._controller.get_single_selected_event_id()
        if sel_id is not None:
            self._restore_editing_state()
            self._remove_inline_creator()
            self._show_inline_creator(sel_id)
            return

        # Non-empty, no single selection: fall back to appending after the
        # last top-level event so the menu works even without a selection.
        last_id = self._last_top_level_event_id()
        if last_id is None:
            return
        self._restore_editing_state()
        self._remove_inline_creator()
        self._show_inline_creator(last_id)

    def _unique_unnamed_folder_name(self) -> str:
        existing = set(self._controller.get_all_folder_names())
        base = "New Folder"
        if base not in existing:
            return base
        n = 2
        while True:
            candidate = f"{base} ({n})"
            if candidate not in existing:
                return candidate
            n += 1

    def _last_top_level_event_id(self):
        raw_route = self._controller.get_raw_route()
        if raw_route is None:
            return None
        children = raw_route.root_folder.children
        if not children:
            return None
        return children[-1].group_id

    def _show_quick_add_popover(self, index, category_idx=None):
        """Show the quick-add popover above the row at *index*.

        If *category_idx* is given, skip straight to that category's detail
        panel.
        """
        rect = self.visualRect(index)
        top_center = self.viewport().mapToGlobal(rect.topLeft())
        top_center.setX(top_center.x() + rect.width() // 2)

        if self._quick_add_popover is None:
            from gui_qt.components.quick_add_popover import QuickAddPopover
            self._quick_add_popover = QuickAddPopover(self._controller, self)

        self._quick_add_popover.show_above(top_center, category_idx=category_idx)

    # ------------------------------------------------------------------
    #  Drag-and-drop
    # ------------------------------------------------------------------

    def startDrag(self, supportedActions):
        """Record the dragged event IDs before starting the drag."""
        self._drag_source_ids = self.get_all_selected_event_ids(allow_event_items=False)
        if not self._drag_source_ids:
            return
        super().startDrag(supportedActions)

    def dropEvent(self, event):
        """Handle the drop by computing the target position and calling the controller."""
        if not self._drag_source_ids:
            event.ignore()
            return

        drop_index = self.indexAt(event.position().toPoint())
        drop_pos = self.dropIndicatorPosition()

        target_info = self._resolve_drop_target(drop_index, drop_pos)
        if target_info is None:
            event.ignore()
            self._drag_source_ids = []
            return

        folder_id, after_id, before_id = target_info

        # Prevent the default model modification -- we handle it via the controller.
        event.setDropAction(Qt.IgnoreAction)
        event.accept()

        try:
            self._controller.move_events_to_position(
                self._drag_source_ids, folder_id,
                after_event_id=after_id, before_event_id=before_id,
            )
        except Exception as e:
            logger.error(f"Drag-and-drop move failed: {e}")

        self._drag_source_ids = []

    def _resolve_drop_target(self, drop_index, drop_pos):
        """Compute (folder_id, after_event_id, before_event_id) from drop position.

        Returns None if the drop target is invalid.
        """
        raw_route = self._controller.get_raw_route()
        if raw_route is None:
            return None

        if not drop_index.isValid():
            # Dropped on empty area -- append to root folder.
            root = raw_route.root_folder
            children = root.children
            if children:
                return root.group_id, children[-1].group_id, None
            return root.group_id, None, None

        name_item = self._model.itemFromIndex(
            drop_index.sibling(drop_index.row(), _COL_NAME)
        )
        if name_item is None:
            return None

        target_id = self._group_id_from_name_item(name_item)
        if target_id is None:
            return None

        target_obj = self._controller.get_event_by_id(target_id)
        if target_obj is None:
            return None

        # Redirect EventItem targets to their parent EventGroup.
        if isinstance(target_obj, route_events.EventItem):
            group = target_obj.parent
            if isinstance(group, route_events.EventGroup):
                target_obj = group
                target_id = group.group_id
            else:
                return None

        if drop_pos == QAbstractItemView.DropIndicatorPosition.OnItem:
            if isinstance(target_obj, route_events.EventFolder):
                # Drop into folder -- append at end.
                return target_obj.group_id, None, None
            # Drop on a non-folder -- treat as "below".
            drop_pos = QAbstractItemView.DropIndicatorPosition.BelowItem

        parent_folder = target_obj.parent
        if parent_folder is None or not isinstance(parent_folder, route_events.EventFolder):
            return None

        if drop_pos == QAbstractItemView.DropIndicatorPosition.AboveItem:
            return parent_folder.group_id, None, target_id

        if drop_pos == QAbstractItemView.DropIndicatorPosition.BelowItem:
            return parent_folder.group_id, target_id, None

        # OnViewport fallback.
        root = raw_route.root_folder
        children = root.children
        if children:
            return root.group_id, children[-1].group_id, None
        return root.group_id, None, None

    # ------------------------------------------------------------------

    def _init_hover_tracking(self):
        self._hovered_row = QModelIndex()

    def mouseMoveEvent(self, event):
        idx = self.indexAt(event.pos())
        # Normalize to the name column so we can compare rows consistently.
        row_idx = idx.sibling(idx.row(), _COL_NAME) if idx.isValid() else QModelIndex()
        if row_idx != self._hovered_row:
            self._hovered_row = row_idx
            self.viewport().update()

        # Position "+" button on hovered row (skip EventItem rows).
        if row_idx.isValid():
            item = self._model.itemFromIndex(row_idx)
            gid = self._group_id_from_name_item(item) if item else None
            show = False
            if gid is not None:
                ev = self._controller.get_event_by_id(gid)
                show = not isinstance(ev, route_events.EventItem)
            if show:
                rect = self.visualRect(row_idx)
                self._plus_button.move(
                    2,
                    rect.top() + (rect.height() - self._plus_button.height()) // 2,
                )
                self._plus_button.show()
                self._plus_button.raise_()
                self._plus_btn_hide_timer.stop()
            else:
                self._plus_btn_hide_timer.start()
        else:
            self._plus_btn_hide_timer.start()

        super().mouseMoveEvent(event)



    def leaveEvent(self, event):
        self._hovered_row = QModelIndex()
        self._plus_btn_hide_timer.start()
        self.viewport().update()
        super().leaveEvent(event)

    def _row_bg_color(self, index):
        """Compute the background color for a row given its state.

        Returns a QColor to fill, or None for no custom fill.
        """
        name_index = index.sibling(index.row(), _COL_NAME)
        item = self._model.itemFromIndex(name_index)
        bg_data = item.data(Qt.BackgroundRole) if item is not None else None
        has_custom_bg = isinstance(bg_data, QBrush)

        is_selected = self.selectionModel().isSelected(name_index)
        is_hovered = (self._hovered_row.isValid()
                      and name_index.row() == self._hovered_row.row()
                      and name_index.parent() == self._hovered_row.parent())

        # Selection always shows as blue, overriding any custom highlight.
        if is_selected and is_hovered:
            return _lighten_color(QColor("#0078d4"), 0.20)
        if is_selected:
            return QColor("#0078d4")
        if is_hovered:
            if has_custom_bg:
                return _lighten_color(bg_data.color(), 0.12)
            return _get_default_hover_color()
        if has_custom_bg:
            return bg_data.color()
        return None

    def drawBranches(self, painter, rect, index):
        """Paint the branch area using the color pre-computed by drawRow.

        This is called from inside super().drawRow() -- by that point drawRow
        has already temporarily deselected the row, so super().drawBranches()
        won't draw selection chrome.  We just need to fill with the correct
        color (stored by drawRow) so the branch area matches the item area.
        """
        color = getattr(self, '_draw_row_color', None)
        if color is not None:
            painter.fillRect(rect, color)
        super().drawBranches(painter, rect, index)

    def drawRow(self, painter, option, index):
        """Paint row background across full width, handling highlight colors,
        selection, and hover lightening uniformly for the entire row.

        We temporarily deselect the row and strip BackgroundRole from items
        so that super().drawRow() only paints text, icons, checkboxes, and
        branch arrows -- no backgrounds or selection chrome at all.
        """
        name_index = index.sibling(index.row(), _COL_NAME)

        # Compute the color BEFORE deselecting so selection state is visible.
        color = self._row_bg_color(index)
        # Store for drawBranches (called from within super().drawRow()).
        self._draw_row_color = color

        if color is not None:
            painter.fillRect(option.rect, color)

        # --- Temporarily strip BackgroundRole from all cells ---------------
        item = self._model.itemFromIndex(name_index)
        saved_bg = []
        model_was_blocked = False
        if item is not None:
            parent = item.parent() or self._model.invisibleRootItem()
            row = item.row()
            model_was_blocked = self._model.signalsBlocked()
            self._model.blockSignals(True)
            for col in range(self._model.columnCount()):
                col_item = parent.child(row, col)
                if col_item is not None:
                    old_bg = col_item.data(Qt.BackgroundRole)
                    if old_bg is not None:
                        saved_bg.append((col_item, old_bg))
                        col_item.setData(None, Qt.BackgroundRole)

        # --- Temporarily deselect so super paints zero selection chrome ----
        sel_model = self.selectionModel()
        was_selected = sel_model.isSelected(name_index)
        sel_was_blocked = sel_model.signalsBlocked()
        if was_selected:
            sel_model.blockSignals(True)
            sel_model.select(name_index, QItemSelectionModel.Deselect | QItemSelectionModel.Rows)

        super().drawRow(painter, option, index)

        # --- Restore selection ---------------------------------------------
        if was_selected:
            sel_model.select(name_index, QItemSelectionModel.Select | QItemSelectionModel.Rows)
            sel_model.blockSignals(sel_was_blocked)

        # --- Restore BackgroundRole ----------------------------------------
        for col_item, old_bg in saved_bg:
            col_item.setData(old_bg, Qt.BackgroundRole)
        if item is not None:
            self._model.blockSignals(model_was_blocked)

        self._draw_row_color = None

    def _unregister_text_field_focus(self):
        """Notify the top-level window to unregister text-field focus."""
        try:
            top = self.window()
            if hasattr(top, "unregister_text_field_focus"):
                top.unregister_text_field_focus()
        except Exception:
            pass

    # ------------------------------------------------------------------
    #  Inline quantity editor
    # ------------------------------------------------------------------

    @staticmethod
    def _get_event_quantity(event_obj):
        """Return the current editable quantity for an event, or None."""
        if not isinstance(event_obj, route_events.EventGroup):
            return None
        event_def = event_obj.event_definition
        if event_def.item_event_def is not None:
            return event_def.item_event_def.item_amount
        if event_def.vitamin is not None:
            return event_def.vitamin.amount
        if event_def.rare_candy is not None:
            return event_def.rare_candy.amount
        return None

    def _check_quantity_click(self, pos):
        """If *pos* is over a quantity suffix, open the inline editor."""
        index = self.indexAt(pos)
        if not index.isValid():
            return
        name_index = index.sibling(index.row(), _COL_NAME)
        name_item = self._model.itemFromIndex(name_index)
        if name_item is None:
            return
        group_id = self._group_id_from_name_item(name_item)
        if group_id is None:
            return
        event_obj = self._controller.get_event_by_id(group_id)
        quantity = self._get_event_quantity(event_obj)
        if quantity is None:
            return

        suffix_rect = self._compute_quantity_suffix_rect(name_index, name_item.text(), quantity)
        if suffix_rect is None:
            return

        # Expand hit area slightly for easier clicking.
        if suffix_rect.adjusted(-6, -2, 6, 2).contains(pos):
            self._show_quantity_editor(suffix_rect, group_id, quantity)

    def _compute_quantity_suffix_rect(self, index, text, quantity):
        """Compute the viewport rect of the 'xN' quantity suffix."""
        suffix = f"x{quantity}"
        if not text.endswith(suffix):
            return None

        option = QStyleOptionViewItem()
        option.initFrom(self)
        option.rect = self.visualRect(index)
        delegate = self.itemDelegate(index)
        delegate.initStyleOption(option, index)

        text_rect = self.style().subElementRect(
            QStyle.SubElement.SE_ItemViewItemText, option, self
        )

        fm = self.fontMetrics()
        prefix = text[:-len(suffix)]
        prefix_width = fm.horizontalAdvance(prefix)
        suffix_width = fm.horizontalAdvance(suffix)

        return QRect(
            text_rect.x() + prefix_width,
            text_rect.y(),
            suffix_width,
            text_rect.height(),
        )

    def _show_quantity_editor(self, suffix_rect, group_id, current_qty):
        """Create and show a QLineEdit over the quantity suffix."""
        self._close_quantity_editor()

        editor = QLineEdit(self.viewport())
        editor.setFont(self.font())
        editor.setText(str(current_qty))
        editor.selectAll()
        editor.setAlignment(Qt.AlignCenter)
        editor.setStyleSheet(
            "QLineEdit { padding: 0px 2px; border: 1px solid #0078d4; }"
        )

        # Size the editor so it's easy to type in.
        min_width = 45
        w = max(suffix_rect.width() + 16, min_width)
        h = suffix_rect.height()
        x = suffix_rect.x() + suffix_rect.width() // 2 - w // 2
        y = suffix_rect.y()
        editor.setGeometry(x, y, w, h)

        editor.returnPressed.connect(self._accept_quantity_edit)
        editor.installEventFilter(self)

        self._quantity_editor = editor
        self._quantity_group_id = group_id

        editor.show()
        editor.setFocus()

    def _accept_quantity_edit(self):
        """Apply the edited quantity and close the editor."""
        if self._quantity_editor is None:
            return
        try:
            new_qty = int(self._quantity_editor.text().strip())
        except (ValueError, TypeError):
            self._close_quantity_editor()
            return

        if new_qty < 1:
            new_qty = 1

        group_id = self._quantity_group_id
        self._close_quantity_editor()

        event_obj = self._controller.get_event_by_id(group_id)
        if not isinstance(event_obj, route_events.EventGroup):
            return

        event_def = event_obj.event_definition
        if event_def.item_event_def is not None:
            event_def.item_event_def.item_amount = new_qty
        elif event_def.vitamin is not None:
            event_def.vitamin.amount = new_qty
        elif event_def.rare_candy is not None:
            event_def.rare_candy.amount = new_qty
        else:
            return

        self._controller.update_existing_event(group_id, event_def)

    def _cancel_quantity_edit(self, *args):
        """Cancel the quantity edit (close without applying)."""
        self._close_quantity_editor()

    def _close_quantity_editor(self):
        """Clean up the quantity editor widget."""
        if self._quantity_editor is None:
            return
        editor = self._quantity_editor
        self._quantity_editor = None
        self._quantity_group_id = None
        editor.removeEventFilter(self)
        editor.hide()
        editor.deleteLater()

    def eventFilter(self, obj, event):
        """Handle focus-out and Escape on the quantity editor, and +button hover."""
        if obj is self._plus_button:
            if event.type() == QEvent.Type.Enter:
                self._plus_btn_hide_timer.stop()
                return False
            if event.type() == QEvent.Type.Leave:
                self._plus_btn_hide_timer.start()
                return False
        if obj is self._quantity_editor:
            if event.type() == QEvent.Type.FocusOut:
                self._accept_quantity_edit()
                return True
            if event.type() == QEvent.Type.KeyPress and event.key() == Qt.Key_Escape:
                self._cancel_quantity_edit()
                return True
        return super().eventFilter(obj, event)

    # ------------------------------------------------------------------
    #  Inline event creation ("+" button)
    # ------------------------------------------------------------------

    def _maybe_hide_plus_button(self):
        """Hide the + button unless the cursor is still over it."""
        if self._plus_button.underMouse():
            return
        self._plus_button.hide()

    def _on_plus_clicked(self):
        if not self._hovered_row.isValid():
            return
        item = self._model.itemFromIndex(self._hovered_row)
        if item is None:
            return
        group_id = self._group_id_from_name_item(item)
        if group_id is None:
            return
        # Cancel any active edit, then replace any existing creator.
        self._restore_editing_state()
        self._remove_inline_creator()
        self._show_inline_creator(group_id)
        self._plus_button.hide()

    def _show_inline_creator(self, insert_after_id, dest_folder_name=None,
                             inside_folder_id=None):
        from gui_qt.components.inline_event_creator import InlineEventCreator

        # Force the right panel to the Pre-Event State tab so the left panel
        # gets its wider layout — the inline creator needs the horizontal room
        # to lay out all of its fields. The user can still manually flip back
        # to the Battle Summary tab; the suppress flag only blocks automatic
        # selection-driven tab switches.
        self._set_suppress_battle_summary(True)

        self._inline_after_id = insert_after_id
        self._inline_inside_folder_id = inside_folder_id
        self._inline_creator = InlineEventCreator(
            self._controller, insert_after_id, parent=self.viewport(),
            dest_folder_name=dest_folder_name,
        )
        self._inline_creator.event_created.connect(self._on_inline_created)
        self._inline_creator.discarded.connect(self._on_inline_discarded)
        self._insert_inline_row()
        self._inline_creator.focus_type_combo()

    _INLINE_ROW_HEIGHT = 34

    def _insert_inline_row(self):
        """Insert a spacer row into the model and overlay the creator on it."""
        if self._inline_inside_folder_id is not None:
            # Inside-folder mode: spacer row becomes the folder's first child.
            folder_item = self._item_lookup.get(self._inline_inside_folder_id)
            if folder_item is None:
                self._remove_inline_creator()
                return
            folder_idx = self._model.indexFromItem(folder_item)
            if folder_idx.isValid():
                self.setExpanded(folder_idx, True)
            parent = folder_item
            insert_pos = 0
        else:
            target_item = self._item_lookup.get(self._inline_after_id)
            if target_item is None:
                self._remove_inline_creator()
                return
            parent = target_item.parent()
            if parent is None:
                parent = self._model.invisibleRootItem()
            insert_pos = target_item.row() + 1

        # Build a blank spacer row with a size hint tall enough for the
        # creator widget.  Do NOT block model signals -- the view must
        # receive ``rowsInserted`` so it allocates the correct row height.
        h = self._INLINE_ROW_HEIGHT
        self._inline_row_item = QStandardItem("")
        self._inline_row_item.setEditable(False)
        self._inline_row_item.setCheckable(False)
        self._inline_row_item.setDragEnabled(False)
        self._inline_row_item.setDropEnabled(False)
        self._inline_row_item.setSelectable(False)
        self._inline_row_item.setSizeHint(QSize(0, h))

        row = [self._inline_row_item]
        for _ in range(1, len(_COLUMN_DEFS)):
            col = QStandardItem("")
            col.setEditable(False)
            col.setSizeHint(QSize(0, h))
            row.append(col)

        parent.insertRow(insert_pos, row)

        # Span all columns so the widget fills the full width.
        parent_idx = (
            self._model.indexFromItem(parent)
            if parent is not self._model.invisibleRootItem()
            else QModelIndex()
        )
        self.setFirstColumnSpanned(insert_pos, parent_idx, True)

        # Force the view to recalculate layout so the tall row is measured.
        self.doItemsLayout()

        self._reposition_inline_creator()

    def _remove_inline_row(self):
        """Remove the spacer row from the model."""
        if self._inline_row_item is None:
            return
        try:
            parent = self._inline_row_item.parent()
            if parent is None:
                parent = self._model.invisibleRootItem()
            row = self._inline_row_item.row()
            if row >= 0:
                parent.removeRow(row)
        except RuntimeError:
            pass
        self._inline_row_item = None

    def _reposition_inline_creator(self, *args):
        """Position the creator widget over the spacer row."""
        if self._inline_creator is None or self._inline_row_item is None:
            return
        idx = self._model.indexFromItem(self._inline_row_item)
        if not idx.isValid():
            return
        rect = self.visualRect(idx)
        if not rect.isValid():
            self._inline_creator.hide()
            return
        self._inline_creator.show()
        self._inline_creator.raise_()
        h = max(rect.height(), self._INLINE_ROW_HEIGHT)
        self._inline_creator.setGeometry(
            rect.x(), rect.y(), self.viewport().width(), h,
        )

    def resizeEvent(self, event):
        """Re-anchor the inline creator when the viewport resizes.

        The left panel width changes whenever the user flips between the
        Pre-Event State and Battle Summary tabs (the splitter re-proportions
        30/70 ↔ 75/25). Without this override the floating inline creator
        overlay keeps its old geometry, so the "dummy event" spacer row
        appears mis-sized until the user scrolls or reopens the creator.
        """
        super().resizeEvent(event)
        self._reposition_inline_creator()
        if self._empty_state_btn.isVisible():
            self._position_empty_state_button()

    def _start_inline_edit(self, group_id, event_obj):
        """Disable *event_obj*, open a pre-populated inline editor."""
        # Cancel any prior edit / creator.
        self._restore_editing_state()
        self._remove_inline_creator()

        # Suppress the battle summary for the duration of the edit so the
        # user has room to work in the pre-event state tab.
        self._set_suppress_battle_summary(True)

        # Save original state.
        self._editing_group_id = group_id
        self._editing_original_enabled = event_obj.event_definition.enabled

        # Disable the event so the route recalculates without it.
        event_obj.set_enabled_status(False)
        self._controller.update_existing_event(group_id, event_obj.event_definition)

        # Show pre-populated inline editor after the disabled row.
        from gui_qt.components.inline_event_creator import InlineEventCreator

        self._inline_after_id = group_id
        self._inline_creator = InlineEventCreator(
            self._controller, group_id, parent=self.viewport(),
            editing_group_id=group_id,
            editing_event_def=event_obj.event_definition,
        )
        self._inline_creator.event_created.connect(self._on_inline_created)
        self._inline_creator.discarded.connect(self._on_inline_discarded)
        self._insert_inline_row()
        self._inline_creator.focus_primary_field()

    def _on_inline_created(self):
        if self._editing_group_id is not None:
            # Edit mode: apply the new definition and restore enabled state.
            self._set_suppress_battle_summary(False)
            new_def = self._inline_creator.result_event_def
            if new_def is not None:
                new_def.enabled = self._editing_original_enabled
                event_obj = self._controller.get_event_by_id(self._editing_group_id)
                if event_obj is not None:
                    event_obj.set_enabled_status(self._editing_original_enabled)
                self._controller.update_existing_event(self._editing_group_id, new_def)
            else:
                # Builder returned None — treat like discard.
                self._restore_editing_state()
            self._editing_group_id = None
            self._editing_original_enabled = None
        self._remove_inline_creator()

    def _on_inline_discarded(self):
        self._restore_editing_state()
        self._remove_inline_creator()

    def _restore_editing_state(self):
        """Re-enable the event that was temporarily disabled for editing."""
        if self._editing_group_id is None:
            return
        self._set_suppress_battle_summary(False)
        event_obj = self._controller.get_event_by_id(self._editing_group_id)
        if event_obj is not None:
            event_obj.set_enabled_status(self._editing_original_enabled)
            self._controller.update_existing_event(
                self._editing_group_id, event_obj.event_definition,
            )
        self._editing_group_id = None
        self._editing_original_enabled = None

    def _set_suppress_battle_summary(self, suppress):
        """Tell EventDetails to block/allow battle-summary auto-switching."""
        try:
            top = self.window()
            if hasattr(top, "event_details"):
                top.event_details.set_suppress_battle_summary(suppress)
        except Exception:
            pass

    def _remove_inline_creator(self):
        if self._inline_creator is not None:
            self._inline_creator.hide()
            self._inline_creator.deleteLater()
            self._inline_creator = None
        self._remove_inline_row()
        self._inline_after_id = None
        self._inline_inside_folder_id = None
        # Release the pre-state lock so normal auto-switch behavior resumes.
        self._set_suppress_battle_summary(False)

    # ------------------------------------------------------------------
    #  Selection helpers
    # ------------------------------------------------------------------

    def get_all_selected_event_ids(self, allow_event_items=True) -> List[int]:
        """Return semantic group_ids for all selected rows.

        Filters out EventItems when *allow_event_items* is False and removes
        children whose parent is also selected.
        """
        indexes = self.selectionModel().selectedRows(_COL_NAME)
        # Build a set of selected model indexes for parent-child filtering.
        selected_set: Set[QModelIndex] = {idx for idx in indexes}

        result = []
        for idx in indexes:
            item = self._model.itemFromIndex(idx)
            if item is None:
                continue
            group_id = self._group_id_from_name_item(item)
            if group_id is None:
                continue

            if not allow_event_items:
                event_obj = self._controller.get_event_by_id(group_id)
                if isinstance(event_obj, route_events.EventItem):
                    continue

            # If the parent row is also selected, skip this child.
            parent_idx = idx.parent()
            if parent_idx.isValid() and parent_idx in selected_set:
                continue

            result.append(group_id)
        return result

    def set_all_selected_event_ids(self, event_ids):
        """Select the rows corresponding to the given semantic ids."""
        sel_model = self.selectionModel()
        sel_model.clearSelection()
        for eid in event_ids:
            name_item = self._item_lookup.get(eid)
            if name_item is None:
                continue
            idx = self._model.indexFromItem(name_item)
            sel_model.select(
                idx,
                QItemSelectionModel.Select | QItemSelectionModel.Rows,
            )

    def _selected_name_items(self) -> List[QStandardItem]:
        """Return the name-column QStandardItem for every selected row."""
        indexes = self.selectionModel().selectedRows(_COL_NAME)
        result = []
        for idx in indexes:
            item = self._model.itemFromIndex(idx)
            if item is not None:
                result.append(item)
        return result

    # ------------------------------------------------------------------
    #  Scrolling
    # ------------------------------------------------------------------

    def scroll_to_selected_events(self):
        try:
            indexes = self.selectionModel().selectedRows(_COL_NAME)
            if indexes:
                self.scrollTo(indexes[-1], QAbstractItemView.EnsureVisible)
        except Exception:
            pass

    def scroll_to_top(self):
        try:
            if self._model.rowCount() > 0:
                self.scrollTo(self._model.index(0, 0), QAbstractItemView.EnsureVisible)
        except Exception:
            pass

    def scroll_to_bottom(self):
        try:
            count = self._model.rowCount()
            if count > 0:
                last_idx = self._last_visible_index(self._model.invisibleRootItem())
                if last_idx.isValid():
                    self.scrollTo(last_idx, QAbstractItemView.EnsureVisible)
        except Exception:
            pass

    def _last_visible_index(self, parent_item: QStandardItem) -> QModelIndex:
        """Recursively find the last visible item index in the tree."""
        row_count = parent_item.rowCount()
        if row_count == 0:
            return self._model.indexFromItem(parent_item)
        last_child = parent_item.child(row_count - 1, _COL_NAME)
        if last_child is None:
            return self._model.indexFromItem(parent_item)
        idx = self._model.indexFromItem(last_child)
        if self.isExpanded(idx) and last_child.rowCount() > 0:
            return self._last_visible_index(last_child)
        return idx

    # ------------------------------------------------------------------
    #  Refresh / tree building
    # ------------------------------------------------------------------

    def _collect_expand_state(self, parent_item: QStandardItem, path: str = "") -> Dict[str, bool]:
        """Walk the tree and record expand state keyed by folder name path."""
        result = {}
        for row in range(parent_item.rowCount()):
            child = parent_item.child(row, 0)
            if child is None:
                continue
            idx = self._model.indexFromItem(child)
            if not idx.isValid():
                continue
            # Only record folders (items that have children or are expandable)
            if child.rowCount() > 0 or self.isExpanded(idx):
                key = f"{path}/{child.text()}"
                result[key] = self.isExpanded(idx)
                result.update(self._collect_expand_state(child, key))
        return result

    def _restore_expand_state(self, parent_item: QStandardItem, saved: Dict[str, bool], path: str = ""):
        """Restore expand state from a saved snapshot."""
        for row in range(parent_item.rowCount()):
            child = parent_item.child(row, 0)
            if child is None:
                continue
            idx = self._model.indexFromItem(child)
            if not idx.isValid():
                continue
            key = f"{path}/{child.text()}"
            if key in saved:
                if saved[key]:
                    self.expand(idx)
                else:
                    self.collapse(idx)
            self._restore_expand_state(child, saved, key)

    def refresh(self, *args, **kwargs):
        """Rebuild/update the tree from the controller's route data."""
        if self._refreshing:
            return
        self._close_quantity_editor()
        self._refreshing = True

        # Temporarily remove the inline-creator spacer row so it doesn't
        # interfere with the rebuild.  We'll re-insert it afterward.
        had_inline = self._inline_creator is not None
        if had_inline:
            self._inline_creator.hide()
            self._remove_inline_row()

        # Save scroll position before any model changes.
        saved_scroll = self.verticalScrollBar().value()

        # Save current selection so it can be restored after the rebuild.
        saved_selection = self.get_all_selected_event_ids()

        self.setUpdatesEnabled(False)
        try:
            # Snapshot current expand state before rebuilding, then merge into
            # the persistent map so state survives refreshes that drop folders
            # (e.g. when the Major Battles flatten filter is toggled).
            self._persistent_expand_state.update(
                self._collect_expand_state(self._model.invisibleRootItem())
            )
            saved_expand = dict(self._persistent_expand_state)

            to_delete_ids: Set[int] = set(self._item_lookup.keys())
            root_item = self._model.invisibleRootItem()

            raw_route = self._controller.get_raw_route()
            if raw_route is None:
                return

            # When the Major Battles filter is active, flatten the tree so
            # the matching events are shown directly under root with no
            # surrounding folders.
            cur_filter = self._controller.get_route_filter_types() or []
            flatten_now = const.MAJOR_BATTLE_FILTER in cur_filter
            self._last_used_flatten_filter = flatten_now
            if flatten_now:
                events_to_render = list(
                    self._iter_flat_event_groups(raw_route.root_folder.children)
                )
            else:
                events_to_render = raw_route.root_folder.children

            self._refresh_recursively(
                root_item,
                events_to_render,
                to_delete_ids,
            )

            # Remove any items that are no longer present in the route.
            # NOTE: removing a parent row also removes all children, so some items
            # may already be deleted when we reach them -- just skip those.
            for del_id in to_delete_ids:
                name_item = self._item_lookup.pop(del_id, None)
                if name_item is None:
                    continue
                try:
                    parent_item = name_item.parent()
                    if parent_item is None:
                        parent_item = self._model.invisibleRootItem()
                    row = name_item.row()
                    if row >= 0:
                        parent_item.removeRow(row)
                except RuntimeError:
                    # C++ object already deleted (parent was removed first)
                    pass

            # Re-insert the inline-creator spacer row at the updated position.
            if had_inline and self._inline_creator is not None:
                self._insert_inline_row()

            # Restore expand state from before the rebuild.
            if saved_expand:
                self._restore_expand_state(self._model.invisibleRootItem(), saved_expand)

            # Restore selection from before the rebuild.
            if saved_selection:
                self.set_all_selected_event_ids(saved_selection)
        finally:
            self.setUpdatesEnabled(True)
            self._refreshing = False

        # Restore scroll position so the viewport doesn't jump.
        self.verticalScrollBar().setValue(saved_scroll)

        self._update_empty_state_button()

        self.route_list_refreshed.emit()

    def _update_empty_state_button(self):
        """Show the "Add New Event" prompt only when the route has no events."""
        try:
            is_empty = self._controller.is_empty()
        except Exception:
            is_empty = False
        # Hide while the inline creator owns the viewport so the button
        # doesn't float over the creator for the first event.
        if is_empty and self._inline_creator is None:
            self._position_empty_state_button()
            self._empty_state_btn.show()
            self._empty_state_btn.raise_()
        else:
            self._empty_state_btn.hide()

    def _position_empty_state_button(self):
        vp = self.viewport()
        btn = self._empty_state_btn
        btn.adjustSize()
        x = max(0, (vp.width() - btn.width()) // 2)
        y = max(20, (vp.height() - btn.height()) // 3)
        btn.move(x, y)

    def refresh_filter_only(self):
        """Update only row visibility based on the current filter/search.

        Skips the per-row data writes and re-parenting in refresh(). Falls
        back to a full refresh for filters that change tree structure (e.g.
        Major Battles, which flattens folders).
        """
        if self._refreshing:
            return

        cur_filter = self._controller.get_route_filter_types() or []
        flatten_now = const.MAJOR_BATTLE_FILTER in cur_filter

        # If we're entering or leaving flatten mode, the tree structure
        # changes, so we have to do a full rebuild.
        if flatten_now or self._last_used_flatten_filter:
            self.refresh()
            return

        raw_route = self._controller.get_raw_route()
        if raw_route is None:
            return

        self._refreshing = True
        self.setUpdatesEnabled(False)
        try:
            self._refresh_visibility_recursive(raw_route.root_folder.children)
        finally:
            self.setUpdatesEnabled(True)
            self._refreshing = False

    def _refresh_visibility_recursive(self, event_list):
        """Walk events and update only row visibility (no model writes)."""
        cur_search = self._controller.get_route_search_string()
        cur_filter = self._controller.get_route_filter_types()

        for event_obj in event_list:
            semantic_id = _get_attr(event_obj, "group_id")
            if semantic_id is None:
                continue
            name_item = self._item_lookup.get(semantic_id)
            if name_item is None:
                continue

            should_render = event_obj.do_render(search=cur_search, filter_types=cur_filter)

            parent_item = name_item.parent()
            if parent_item is None:
                parent_idx = self.rootIndex()
            else:
                parent_idx = self._model.indexFromItem(parent_item)
            self.setRowHidden(name_item.row(), parent_idx, not should_render)

            is_folder = isinstance(event_obj, route_events.EventFolder)
            if is_folder:
                self._refresh_visibility_recursive(event_obj.children)
            elif isinstance(event_obj, route_events.EventGroup):
                # Level-up children inherit their parent EventGroup's visibility.
                if len(event_obj.event_items) > 1:
                    for item_obj in event_obj.event_items:
                        is_level_up = (
                            item_obj.event_definition.learn_move is not None
                            and item_obj.event_definition.learn_move.source == const.MOVE_SOURCE_LEVELUP
                        )
                        if not is_level_up:
                            continue
                        item_id = _get_attr(item_obj, "group_id")
                        if item_id is None:
                            continue
                        lu_item = self._item_lookup.get(item_id)
                        if lu_item is None:
                            continue
                        lu_parent = lu_item.parent()
                        if lu_parent is None:
                            lu_parent_idx = self.rootIndex()
                        else:
                            lu_parent_idx = self._model.indexFromItem(lu_parent)
                        self.setRowHidden(lu_item.row(), lu_parent_idx, not should_render)

    def _iter_flat_event_groups(self, event_list):
        """Yield all EventGroup descendants of *event_list*, skipping folders.

        Used when a flatten-style filter (e.g. Major Battles) is active so
        matching events render under root without their containing folders.
        """
        for event_obj in event_list:
            if isinstance(event_obj, route_events.EventFolder):
                yield from self._iter_flat_event_groups(event_obj.children)
            else:
                yield event_obj

    def _refresh_recursively(
        self,
        parent_item: QStandardItem,
        event_list,
        to_delete_ids: Set[int],
    ):
        cur_search = self._controller.get_route_search_string()
        cur_filter = self._controller.get_route_filter_types()
        actual_pos = 0

        # Track which rows under this parent should be visible.
        visible_ids: Set[int] = set()

        for event_obj in event_list:
            semantic_id = _get_attr(event_obj, "group_id")
            if semantic_id is None:
                continue

            should_render = event_obj.do_render(search=cur_search, filter_types=cur_filter)

            is_folder = isinstance(event_obj, route_events.EventFolder)
            force_open = event_obj.expanded if is_folder else False

            # Always upsert the row (so it stays in the model), but hide it if filtered out.
            if semantic_id in to_delete_ids:
                to_delete_ids.discard(semantic_id)

            name_item = self._upsert_row(event_obj, parent_item, force_open if should_render else None)

            # Ensure correct position under the parent.
            current_row = name_item.row()
            current_parent = name_item.parent()
            if current_parent is None:
                current_parent_item = self._model.invisibleRootItem()
            else:
                current_parent_item = current_parent

            if current_row != actual_pos or current_parent_item is not parent_item:
                # Need to move: take the row and re-insert at the correct position.
                taken = current_parent_item.takeRow(current_row)
                if taken:
                    parent_item.insertRow(actual_pos, taken)

            # Hide or show based on filter.
            parent_idx = self._model.indexFromItem(parent_item) if parent_item is not self._model.invisibleRootItem() else self.rootIndex()
            self.setRowHidden(actual_pos, parent_idx, not should_render)

            if should_render:
                visible_ids.add(semantic_id)

            actual_pos += 1

            if is_folder:
                self._refresh_recursively(name_item, event_obj.children, to_delete_ids)
            elif isinstance(event_obj, route_events.EventGroup):
                if len(event_obj.event_items) > 1:
                    level_up_moves = []
                    other_items = []
                    for item_obj in event_obj.event_items:
                        is_level_up = (
                            item_obj.event_definition.learn_move is not None
                            and item_obj.event_definition.learn_move.source == const.MOVE_SOURCE_LEVELUP
                        )
                        if is_level_up:
                            level_up_moves.append(item_obj)
                        else:
                            other_items.append(item_obj)

                    # Level-up moves rendered as siblings of the EventGroup,
                    # but with a visual prefix to show they derive from the battle above.
                    for level_up_item in level_up_moves:
                        item_semantic_id = _get_attr(level_up_item, "group_id")
                        if item_semantic_id is not None:
                            to_delete_ids.discard(item_semantic_id)
                            lu_name_item = self._upsert_row(level_up_item, parent_item, False, is_level_up_child=True)
                            cur_row = lu_name_item.row()
                            cur_par = lu_name_item.parent()
                            if cur_par is None:
                                cur_par = self._model.invisibleRootItem()
                            if cur_row != actual_pos or cur_par is not parent_item:
                                taken = cur_par.takeRow(cur_row)
                                if taken:
                                    parent_item.insertRow(actual_pos, taken)
                            # Hide level-up siblings if parent is hidden
                            self.setRowHidden(actual_pos, parent_idx, not should_render)
                            actual_pos += 1

                    # Other items rendered as children of the EventGroup.
                    for item_idx, item_obj in enumerate(other_items):
                        item_semantic_id = _get_attr(item_obj, "group_id")
                        if item_semantic_id is not None:
                            to_delete_ids.discard(item_semantic_id)
                            child_name_item = self._upsert_row(item_obj, name_item, False)
                            cur_row = child_name_item.row()
                            cur_par = child_name_item.parent()
                            if cur_par is None:
                                cur_par = self._model.invisibleRootItem()
                            if cur_row != item_idx or cur_par is not name_item:
                                taken = cur_par.takeRow(cur_row)
                                if taken:
                                    name_item.insertRow(item_idx, taken)

    # ------------------------------------------------------------------
    #  Row upsert
    # ------------------------------------------------------------------

    def _upsert_row(
        self,
        event_obj,
        parent_item: QStandardItem,
        force_open: bool,
        is_level_up_child: bool = False,
    ) -> QStandardItem:
        """Create or update a row for *event_obj* under *parent_item*.

        Returns the name-column QStandardItem.
        """
        semantic_id = _get_attr(event_obj, "group_id")
        raw_name = str(_get_attr(event_obj, "name") or "")
        text_val = f"  \u2514 {raw_name}" if is_level_up_child else raw_name
        tags = _get_attr(event_obj, "get_tags") or []
        is_enabled = _get_attr(event_obj, "is_enabled")

        # Quantity suffix for inline-editing indicator.
        quantity = self._get_event_quantity(event_obj)
        qty_suffix = f"x{quantity}" if quantity is not None and text_val.endswith(f"x{quantity}") else None

        # Determine checkbox state.
        if is_enabled is not None:
            check_state = Qt.Checked if is_enabled else Qt.Unchecked
        else:
            check_state = None

        # Configure drag-and-drop flags per event type.
        is_folder = isinstance(event_obj, route_events.EventFolder)
        is_group = isinstance(event_obj, route_events.EventGroup)
        can_drag = (is_folder or is_group) and not is_level_up_child
        can_drop = is_folder

        # Determine background color from tags.
        bg_brush = self._brush_for_tags(tags)

        # Determine foreground color for folders.
        fg_brush = None
        if is_folder and self._folder_fg_color is not None:
            fg_brush = QBrush(self._folder_fg_color)

        # Level-up move children: dimmed foreground to show derivation.
        if is_level_up_child and fg_brush is None:
            fg_brush = QBrush(QColor("#8899aa"))

        # Unchecked style: gray foreground (overrides level-up dim).
        if check_state == Qt.Unchecked:
            fg_brush = QBrush(QColor("#cbcbcb"))

        existing_name_item = self._item_lookup.get(semantic_id)

        if existing_name_item is not None:
            # Update existing row.
            name_item = existing_name_item
            self._model.blockSignals(True)
            try:
                name_item.setText(text_val)
                name_item.setData(qty_suffix, _QUANTITY_SUFFIX_ROLE)
                if check_state is not None:
                    name_item.setCheckState(check_state)
                self._apply_row_style(name_item, bg_brush, fg_brush)
                # Update data columns.
                row_items = self._get_sibling_items(name_item)
                for col_idx in range(1, len(_COLUMN_DEFS)):
                    _, attr_name, _, hidden = _COLUMN_DEFS[col_idx]
                    if hidden and col_idx == _COL_EVENT_ID:
                        val = semantic_id
                    else:
                        val = _get_attr(event_obj, attr_name)
                    col_item = row_items.get(col_idx)
                    if col_item is not None:
                        col_item.setText(str(val) if val is not None else "")
                        if bg_brush is not None:
                            col_item.setData(bg_brush, Qt.BackgroundRole)
                        else:
                            col_item.setData(None, Qt.BackgroundRole)
                        if fg_brush is not None:
                            col_item.setData(fg_brush, Qt.ForegroundRole)
                        else:
                            col_item.setData(None, Qt.ForegroundRole)
            finally:
                self._model.blockSignals(False)
        else:
            # Create new row.
            name_item = QStandardItem(text_val)
            name_item.setEditable(False)
            name_item.setData(qty_suffix, _QUANTITY_SUFFIX_ROLE)
            name_item.setDragEnabled(can_drag)
            name_item.setDropEnabled(can_drop)
            if check_state is not None:
                name_item.setCheckable(True)
                name_item.setCheckState(check_state)
            else:
                name_item.setCheckable(False)

            row = [name_item]
            for col_idx in range(1, len(_COLUMN_DEFS)):
                _, attr_name, _, hidden = _COLUMN_DEFS[col_idx]
                if hidden and col_idx == _COL_EVENT_ID:
                    val = semantic_id
                else:
                    val = _get_attr(event_obj, attr_name)
                col_item = QStandardItem(str(val) if val is not None else "")
                col_item.setEditable(False)
                row.append(col_item)

            self._apply_row_style(name_item, bg_brush, fg_brush)
            for col_item in row[1:]:
                if bg_brush is not None:
                    col_item.setData(bg_brush, Qt.BackgroundRole)
                if fg_brush is not None:
                    col_item.setData(fg_brush, Qt.ForegroundRole)

            parent_item.appendRow(row)
            self._item_lookup[semantic_id] = name_item

        # Handle expand/collapse for folders — only for newly created items.
        # Existing items keep their current visual expand state (matches Tkinter
        # behaviour where force_open is only used on insert, not update).
        if existing_name_item is None and force_open is not None:
            idx = self._model.indexFromItem(name_item)
            if idx.isValid():
                if force_open:
                    self.expand(idx)
                elif is_folder:
                    self.collapse(idx)

        return name_item

    # ------------------------------------------------------------------
    #  Styling helpers
    # ------------------------------------------------------------------

    def _brush_for_tags(self, tags) -> Optional[QBrush]:
        """Return a QBrush for the highest-priority tag, or None."""
        if not tags:
            return None
        for tag in tags:
            color = self._highlight_colors.get(tag)
            if color is not None:
                return QBrush(color)
        return None

    def _apply_row_style(
        self,
        name_item: QStandardItem,
        bg_brush: Optional[QBrush],
        fg_brush: Optional[QBrush],
    ):
        """Apply background and foreground brushes to the name-column item."""
        if bg_brush is not None:
            name_item.setData(bg_brush, Qt.BackgroundRole)
        else:
            name_item.setData(None, Qt.BackgroundRole)
        if fg_brush is not None:
            name_item.setData(fg_brush, Qt.ForegroundRole)
        else:
            name_item.setData(None, Qt.ForegroundRole)

    def _apply_folder_foreground(self, name_item: QStandardItem):
        """Apply (or clear) the faded folder foreground color to a single row."""
        if self._folder_fg_color is not None:
            brush = QBrush(self._folder_fg_color)
        else:
            brush = None
        name_item.setData(brush, Qt.ForegroundRole)
        # Also apply to sibling columns in the same row.
        siblings = self._get_sibling_items(name_item)
        for col_item in siblings.values():
            col_item.setData(brush, Qt.ForegroundRole)

    # ------------------------------------------------------------------
    #  Internal utilities
    # ------------------------------------------------------------------

    @staticmethod
    def _group_id_from_name_item(name_item: QStandardItem) -> Optional[int]:
        """Extract the semantic group_id stored in the event_id column of the same row."""
        parent = name_item.parent()
        if parent is None:
            # Top-level row: use the model.
            model = name_item.model()
            if model is None:
                return None
            id_item = model.item(name_item.row(), _COL_EVENT_ID)
        else:
            id_item = parent.child(name_item.row(), _COL_EVENT_ID)
        if id_item is None:
            return None
        try:
            return int(id_item.text())
        except (ValueError, TypeError):
            return None

    @staticmethod
    def _get_sibling_items(name_item: QStandardItem) -> Dict[int, QStandardItem]:
        """Return a dict of column_index -> QStandardItem for all columns in the same row."""
        result = {}
        parent = name_item.parent()
        row = name_item.row()
        for col_idx in range(1, len(_COLUMN_DEFS)):
            if parent is not None:
                sib = parent.child(row, col_idx)
            else:
                model = name_item.model()
                sib = model.item(row, col_idx) if model else None
            if sib is not None:
                result[col_idx] = sib
        return result
