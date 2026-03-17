import logging
from typing import List, Optional, Set, Dict

from PySide6.QtWidgets import (
    QTreeView, QAbstractItemView, QHeaderView,
)
from PySide6.QtCore import Qt, QModelIndex, Signal, QItemSelectionModel
from PySide6.QtGui import (
    QStandardItemModel, QStandardItem, QColor, QBrush,
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
    ("LevelUpsInto", "get_pkmn_after_levelups",      220,   False),
    ("Level",        "pkmn_level",                    50,    False),
    ("Total Exp",    "total_xp",                      80,    False),
    ("Exp per sec",  "experience_per_second",          80,    False),
    ("Exp Gain",     "xp_gain",                        80,    False),
    ("ToNextLevel",  "xp_to_next_level",               80,    False),
    ("% TNL",        "percent_xp_to_next_level",       80,    False),
    ("LvlsGained",   "level_gain",                     80,    False),
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


def _get_attr(obj, attr_name):
    """Retrieve an attribute from *obj*, calling it if callable."""
    val = getattr(obj, attr_name, None)
    if val is None:
        return None
    if callable(val):
        return val()
    return val


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

        # --- selection mode -----------------------------------------------
        self.setSelectionMode(QAbstractItemView.ExtendedSelection)
        self.setSelectionBehavior(QAbstractItemView.SelectRows)

        # --- columns ------------------------------------------------------
        header = self.header()
        for idx, (_, _, width, hidden) in enumerate(_COLUMN_DEFS):
            if hidden:
                self.setColumnHidden(idx, True)
            elif width:
                self.setColumnWidth(idx, width)
                header.setSectionResizeMode(idx, QHeaderView.Fixed)
            else:
                header.setSectionResizeMode(idx, QHeaderView.Stretch)

        # The first column (tree/name) gets the remaining stretch.
        header.setSectionResizeMode(_COL_NAME, QHeaderView.Interactive)
        header.setStretchLastSection(False)

        # --- internal bookkeeping -----------------------------------------
        # Maps semantic group_id -> QModelIndex (persistent) stored as
        # (parent_group_id_or_None, row) -- but we just keep QStandardItem refs.
        self._item_lookup: Dict[int, QStandardItem] = {}

        # Cache for highlight colors so we don't re-query config every cell.
        self._highlight_colors: Dict[str, QColor] = {}
        self._folder_fg_color: Optional[QColor] = None
        self._update_highlight_colors()
        self._update_folder_text_color()

        # --- signals / events ---------------------------------------------
        self.expanded.connect(self._on_item_expanded)
        self.collapsed.connect(self._on_item_collapsed)
        self._model.itemChanged.connect(self._on_item_changed)
        self.selectionModel().selectionChanged.connect(self._on_selection_changed)

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
        self._highlight_colors.clear()
        for idx, label in enumerate(const.ALL_HIGHLIGHT_LABELS, 1):
            self._highlight_colors[label] = QColor(config.get_highlight_color(idx))
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
        """Override to handle left-click focus and right-click toggle."""
        if event.button() == Qt.LeftButton:
            self._unregister_text_field_focus()
            super().mousePressEvent(event)
            return

        if event.button() == Qt.RightButton:
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

    def _unregister_text_field_focus(self):
        """Notify the top-level window to unregister text-field focus."""
        try:
            top = self.window()
            if hasattr(top, "unregister_text_field_focus"):
                top.unregister_text_field_focus()
        except Exception:
            pass

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

    def refresh(self, *args, **kwargs):
        """Rebuild/update the tree from the controller's route data."""
        to_delete_ids: Set[int] = set(self._item_lookup.keys())
        root_item = self._model.invisibleRootItem()

        raw_route = self._controller.get_raw_route()
        if raw_route is None:
            return

        self._refresh_recursively(
            root_item,
            raw_route.root_folder.children,
            to_delete_ids,
        )

        # Remove any items that are no longer present in the route.
        for del_id in to_delete_ids:
            name_item = self._item_lookup.pop(del_id, None)
            if name_item is None:
                continue
            parent_item = name_item.parent()
            if parent_item is None:
                parent_item = self._model.invisibleRootItem()
            row = name_item.row()
            if row >= 0:
                parent_item.removeRow(row)

        self.route_list_refreshed.emit()

    def _refresh_recursively(
        self,
        parent_item: QStandardItem,
        event_list,
        to_delete_ids: Set[int],
    ):
        cur_search = self._controller.get_route_search_string()
        cur_filter = self._controller.get_route_filter_types()
        actual_pos = 0

        for event_obj in event_list:
            semantic_id = _get_attr(event_obj, "group_id")
            if semantic_id is None:
                continue

            if not event_obj.do_render(search=cur_search, filter_types=cur_filter):
                continue

            is_folder = isinstance(event_obj, route_events.EventFolder)
            force_open = event_obj.expanded if is_folder else False

            # Upsert the row.
            if semantic_id in to_delete_ids:
                to_delete_ids.discard(semantic_id)

            name_item = self._upsert_row(event_obj, parent_item, force_open)

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

                    # Level-up moves rendered as siblings of the EventGroup.
                    for level_up_item in level_up_moves:
                        item_semantic_id = _get_attr(level_up_item, "group_id")
                        if item_semantic_id is not None:
                            to_delete_ids.discard(item_semantic_id)
                            lu_name_item = self._upsert_row(level_up_item, parent_item, False)
                            cur_row = lu_name_item.row()
                            cur_par = lu_name_item.parent()
                            if cur_par is None:
                                cur_par = self._model.invisibleRootItem()
                            if cur_row != actual_pos or cur_par is not parent_item:
                                taken = cur_par.takeRow(cur_row)
                                if taken:
                                    parent_item.insertRow(actual_pos, taken)
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
    ) -> QStandardItem:
        """Create or update a row for *event_obj* under *parent_item*.

        Returns the name-column QStandardItem.
        """
        semantic_id = _get_attr(event_obj, "group_id")
        text_val = str(_get_attr(event_obj, "name") or "")
        tags = _get_attr(event_obj, "get_tags") or []
        is_enabled = _get_attr(event_obj, "is_enabled")

        # Determine checkbox state.
        if is_enabled is not None:
            check_state = Qt.Checked if is_enabled else Qt.Unchecked
        else:
            check_state = None

        # Determine background color from tags.
        bg_brush = self._brush_for_tags(tags)

        # Determine foreground color for folders.
        is_folder = isinstance(event_obj, route_events.EventFolder)
        fg_brush = None
        if is_folder and self._folder_fg_color is not None:
            fg_brush = QBrush(self._folder_fg_color)

        # Unchecked style: gray foreground.
        if check_state == Qt.Unchecked:
            fg_brush = QBrush(QColor("#cbcbcb"))

        existing_name_item = self._item_lookup.get(semantic_id)

        if existing_name_item is not None:
            # Update existing row.
            name_item = existing_name_item
            self._model.blockSignals(True)
            try:
                name_item.setText(text_val)
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

        # Handle expand/collapse for folders.
        if force_open:
            idx = self._model.indexFromItem(name_item)
            if idx.isValid() and not self.isExpanded(idx):
                self.expand(idx)
        elif is_folder and not force_open:
            idx = self._model.indexFromItem(name_item)
            if idx.isValid() and self.isExpanded(idx):
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
