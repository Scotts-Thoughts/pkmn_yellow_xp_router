from controllers.main_controller import MainController
import logging
import tkinter as tk
import tkinter.ttk as ttk

from gui import custom_components
from routing import route_events
from utils.constants import const

logger = logging.getLogger(__name__)


class RouteList(custom_components.CustomGridview):
    def __init__(self, controller:MainController, *args, **kwargs):
        self._controller = controller
        self._suppress_focus_changes = False  # Flag to prevent focus changes during programmatic updates
        super().__init__(
            *args,
            custom_col_data=[
                custom_components.CustomGridview.CustomColumn('LevelUpsInto', 'get_pkmn_after_levelups', width=220),
                custom_components.CustomGridview.CustomColumn('Level', 'pkmn_level', width=50),
                custom_components.CustomGridview.CustomColumn('Total Exp', 'total_xp', width=80),
                custom_components.CustomGridview.CustomColumn('Exp per sec', 'experience_per_second', width=80),
                custom_components.CustomGridview.CustomColumn('Exp Gain', 'xp_gain', width=80),
                custom_components.CustomGridview.CustomColumn('ToNextLevel', 'xp_to_next_level', width=80),
                custom_components.CustomGridview.CustomColumn('% TNL', 'percent_xp_to_next_level', width=80),
                custom_components.CustomGridview.CustomColumn('LvlsGained', 'level_gain', width=80),
                custom_components.CustomGridview.CustomColumn('event_id', 'group_id', hidden=True),
            ],
            text_field_attr='name',
            semantic_id_attr='group_id',
            tags_attr='get_tags',
            checkbox_attr='is_enabled',
            req_column_width=325,
            #checkbox_callback=self.general_checkbox_callback_fn,
            checkbox_item_callback=self.checkbox_item_callback_fn,
            **kwargs
        )

        # TODO: connect these to the actual style somehow
        self.tag_configure(const.EVENT_TAG_ERRORS, background="#61520f")
        self.tag_configure(const.EVENT_TAG_IMPORTANT, background="#1f1f1f")
        self.tag_configure(const.HIGHLIGHT_LABEL, background="#156152")
        self.tag_configure(const.EVENT_TAG_BRANCHED_MANDATORY, background="#5a5142")  # Brown color

        self.bind("<<TreeviewOpen>>", self._treeview_opened_callback)
        self.bind("<<TreeviewClose>>", self._treeview_closed_callback)
        # Bind click events to unregister text field focus when clicking on event list
        # IMPORTANT: use add=True so we don't overwrite CheckboxTreeview's checkbox click handler
        self.bind("<Button-1>", self._on_event_list_click, True)
        # Right-click toggles enable/disable without changing current selection
        self.bind("<Button-3>", self._on_event_list_right_click, True)
        # Prevent treeview from taking focus during programmatic selection changes
        self.bind("<FocusIn>", self._on_treeview_focus_in)

    def general_checkbox_callback_fn(self):
        self._controller.get_raw_route()._recalc()
        self.refresh()

    def _treeview_opened_callback(self, *args, **kwargs):
        selected = self.get_all_selected_event_ids()
        # no easy way to figure out unless only one is sleected. Just give up otherwise
        if len(selected) == 1:
            cur_obj = self._controller.get_event_by_id(selected[0])
            if isinstance(cur_obj, route_events.EventFolder):
                cur_obj.expanded = True

            self.refresh()

    def _treeview_closed_callback(self, event):
        selected = self.get_all_selected_event_ids()
        # no easy way to figure out unless only one is sleected. Just give up otherwise
        if len(selected) == 1:
            cur_obj = self._controller.get_event_by_id(selected[0])
            if isinstance(cur_obj, route_events.EventFolder):
                cur_obj.expanded = False

            self.refresh()
    
    def _on_event_list_click(self, event):
        """Handle clicks on the event list to unregister text field focus."""
        # Unregister text field focus when clicking on the event list
        try:
            root = self.winfo_toplevel()
            if hasattr(root, 'unregister_text_field_focus'):
                root.unregister_text_field_focus()
        except Exception:
            pass

    def _on_event_list_right_click(self, event):
        """
        Right-click an event row to toggle enable/disable without selecting/focusing that row.
        This intentionally preserves the current selection.
        """
        # Treat right-click as interacting with the event list (so hotkeys are re-enabled if needed)
        self._on_event_list_click(event)

        clicked_item = self.identify_row(event.y)
        if not clicked_item:
            return "break"

        # Preserve selection so right-click does not change the currently selected event(s)
        current_selection = list(self.selection())
        self.trigger_checkbox(single_item=clicked_item)
        self.selection_set(current_selection)
        return "break"
    
    def _on_treeview_focus_in(self, event):
        """Prevent treeview from taking focus during programmatic updates."""
        if self._suppress_focus_changes:
            # If we're suppressing focus changes, restore focus to the text field
            try:
                root = self.winfo_toplevel()
                if hasattr(root, '_focused_text_field') and root._focused_text_field is not None:
                    focused_widget = root._focused_text_field
                    if focused_widget.winfo_exists():
                        focused_widget.focus_set()
                        return "break"
            except Exception:
                pass

    def _box_click(self, event):
        """
        Override CheckboxTreeview checkbox click behavior so checkbox clicks also unregister
        text-field focus (while still preventing selection changes).
        """
        try:
            elem = self.identify("element", event.x, event.y)
            if "image" in elem:
                # a checkbox was clicked; consider this an interaction with the event list
                self._on_event_list_click(event)
        except Exception:
            pass

        return super()._box_click(event)
    
    def checkbox_item_callback_fn(self, item_id, new_state):
        raw_obj = self._controller.get_event_by_id(self._get_route_id_from_item_id(item_id))
        raw_obj.set_enabled_status(new_state == self.CHECKED_TAG or new_state == self.TRISTATE_TAG)
        self._controller.update_existing_event(raw_obj.group_id, raw_obj.event_definition)
    
    def _get_route_id_from_item_id(self, iid):
        try:
            # super ugly. extract the value of the 'group_id' column. right now this is the last column, so just hard coding the index
            return int(self.item(iid)['values'][-1])
        except (ValueError, IndexError):
            return -1
    
    def set_all_selected_event_ids(self, event_ids):
        new_selection = []
        try:
            # Save current focus widget before updating selection
            focused_widget = self.focus_get()
            # Also check the main window's stored reference
            try:
                root = self.winfo_toplevel()
                if hasattr(root, '_focused_text_field') and root._focused_text_field is not None:
                    focused_widget = root._focused_text_field
            except Exception:
                pass
            
            for cur_event_id in event_ids:
                new_selection.append(self._treeview_id_lookup[cur_event_id])
            
            # Temporarily suppress focus changes to prevent treeview from taking focus
            self._suppress_focus_changes = True
            
            # Update selection
            self.selection_set(new_selection)
            
            # Immediately restore focus if it was on a text field
            if focused_widget is not None:
                try:
                    # Check if the focused widget is a text entry field
                    if isinstance(focused_widget, (tk.Text, ttk.Entry)) or (hasattr(focused_widget, 'winfo_class') and focused_widget.winfo_class() in ('Text', 'TEntry')):
                        # Restore focus immediately
                        if focused_widget.winfo_exists():
                            focused_widget.focus_set()
                except (tk.TclError, AttributeError):
                    pass  # Widget might have been destroyed
            
            # Re-enable focus changes
            self._suppress_focus_changes = False
        except Exception as e:
            # Re-enable focus changes even if there was an error
            self._suppress_focus_changes = False
            # This *should* only happen in the case that events are selected which are currently hidden by filters
            # So, just ignore and carry on
            pass
    
    def _restore_text_field_focus(self, widget):
        """Restore focus to a text field widget."""
        try:
            if widget.winfo_exists():
                widget.focus_set()
        except (tk.TclError, AttributeError):
            pass  # Widget might have been destroyed
    
    def scroll_to_selected_events(self):
        try:
            if self.selection():
                self.see(self.selection()[-1])
        except Exception as e:
            # NOTE: this seems to happen when the controller creates a new event and immediately selects it
            # in that case, the controller moves faster than the event list, so the event to select it fires before it exists
            # ...maybe. I'm not totally sure. But everything seems fine, so ignore these errors for now
            pass
    
    def _get_all_items_recursive(self, parent=""):
        """Recursively get all items in the treeview."""
        items = []
        children = self.get_children(parent)
        for child in children:
            items.append(child)
            items.extend(self._get_all_items_recursive(child))
        return items
    
    def scroll_to_top(self):
        """Scroll to the top of the event list."""
        try:
            all_items = self._get_all_items_recursive()
            if all_items:
                self.see(all_items[0])
        except Exception as e:
            pass
    
    def scroll_to_bottom(self):
        """Scroll to the bottom of the event list."""
        try:
            all_items = self._get_all_items_recursive()
            if all_items:
                self.see(all_items[-1])
        except Exception as e:
            pass
    
    def get_all_selected_event_ids(self, allow_event_items=True):
        temp = set(self.selection())
        result = []
        for cur_iid in self.selection():
            # event items can't be manipulated at all
            cur_route_id = self._get_route_id_from_item_id(cur_iid)
            if not allow_event_items and isinstance(self._controller.get_event_by_id(cur_route_id), route_events.EventItem):
                continue

            # if any folders are selected, ignore all events that are children of that folder
            # we basically say that you have selected the container, and thus do not need to select any of the child objects
            if self.parent(cur_iid) in temp:
                continue
            
            result.append(cur_route_id)

        return result

    def refresh(self, *args, **kwargs):
        # Save current focus widget before refreshing to preserve text field focus
        # Handle case where focus_get() might fail if a dropdown is open
        try:
            focused_widget = self.focus_get()
        except (KeyError, tk.TclError):
            focused_widget = None
        # Also check the main window's stored reference
        try:
            root = self.winfo_toplevel()
            if hasattr(root, '_focused_text_field') and root._focused_text_field is not None:
                focused_widget = root._focused_text_field
        except Exception:
            pass
        
        # Temporarily suppress focus changes to prevent treeview from taking focus during refresh
        self._suppress_focus_changes = True
        
        # begin keeping track of the stuff we already know we're displaying
        # so we can eventually delete stuff that has been removed
        to_delete_ids = set(self._treeview_id_lookup.keys())
        self._refresh_recursively("", self._controller.get_raw_route().root_folder.children, to_delete_ids)

        # we have now updated all relevant records, created missing ones, and ordered everything correctly
        # just need to remove any potentially deleted records
        for cur_del_id in to_delete_ids:
            try:
                self.delete(self._treeview_id_lookup[cur_del_id])
            except Exception:
                # note: this occurs because deleting an entry with children automatically removes all children too
                # so it will fail to remove the children aftewards
                # No actual problem though, just remove from the lookup and continue
                pass
            del self._treeview_id_lookup[cur_del_id]

        self.event_generate(const.ROUTE_LIST_REFRESH_EVENT)
        
        # Restore focus to the widget that had it (if it was a text field)
        # Do this immediately, not with after_idle, to prevent focus loss
        if focused_widget is not None:
            try:
                # Check if the focused widget is a text entry field
                if isinstance(focused_widget, (tk.Text, ttk.Entry)) or (hasattr(focused_widget, 'winfo_class') and focused_widget.winfo_class() in ('Text', 'TEntry')):
                    # Restore focus immediately
                    if focused_widget.winfo_exists():
                        focused_widget.focus_set()
            except (tk.TclError, AttributeError):
                pass  # Widget might have been destroyed
        
        # Re-enable focus changes
        self._suppress_focus_changes = False
    
    def _refresh_recursively(self, parent_id, event_list, to_delete_ids:set):
        cur_search = self._controller.get_route_search_string()
        cur_filter = self._controller.get_route_filter_types()
        # Track the actual position in the treeview, accounting for inserted level up moves
        actual_pos = 0
        for event_idx, event_obj in enumerate(event_list):
            semantic_id = self._get_attr_helper(event_obj, self._semantic_id_attr)

            if not event_obj.do_render(
                search=cur_search,
                filter_types=cur_filter,
            ):
                continue

            if isinstance(event_obj, route_events.EventFolder):
                is_folder = True
                force_open = event_obj.expanded
            else:
                is_folder = False
                force_open = False

            if semantic_id in to_delete_ids:
                to_delete_ids.remove(semantic_id)
                cur_event_id = self._treeview_id_lookup[semantic_id]
                self.custom_upsert(event_obj, parent=parent_id, force_open=force_open, update_checkbox=True)
            else:
                # when first creating the event, make sure it is defined with a checkbox
                cur_event_id = self.custom_upsert(event_obj, parent=parent_id, force_open=force_open, update_checkbox=True)

            if self.index(cur_event_id) != actual_pos or self.parent(cur_event_id) != parent_id:
                self.move(cur_event_id, parent_id, actual_pos)
            
            # Increment position for the EventGroup/Folder itself
            actual_pos += 1

            if is_folder:
                self._refresh_recursively(cur_event_id, event_obj.children, to_delete_ids)

            elif isinstance(event_obj, route_events.EventGroup):
                if len(event_obj.event_items) > 1:
                    # Separate level up moves from other event items
                    level_up_moves = []
                    other_items = []
                    for item_obj in event_obj.event_items:
                        # Check if this is a level up move
                        is_level_up = (
                            item_obj.event_definition.learn_move is not None and
                            item_obj.event_definition.learn_move.source == const.MOVE_SOURCE_LEVELUP
                        )
                        if is_level_up:
                            level_up_moves.append(item_obj)
                        else:
                            other_items.append(item_obj)
                    
                    # Render level up moves as siblings of the EventGroup (always visible)
                    # They appear right after the EventGroup in the parent's children
                    for level_up_item in level_up_moves:
                        item_semantic_id = self._get_attr_helper(level_up_item, self._semantic_id_attr)
                        if item_semantic_id in to_delete_ids:
                            item_id = self._treeview_id_lookup[item_semantic_id]
                            to_delete_ids.remove(item_semantic_id)
                            self.custom_upsert(level_up_item, parent=parent_id)
                        else:
                            item_id = self.custom_upsert(level_up_item, parent=parent_id)
                        
                        # Position level up moves right after the EventGroup
                        if self.index(item_id) != actual_pos or self.parent(item_id) != parent_id:
                            self.move(item_id, parent_id, actual_pos)
                        actual_pos += 1
                    
                    # Render other items as children of the EventGroup (can be hidden when collapsed)
                    for item_idx, item_obj in enumerate(other_items):
                        item_semantic_id = self._get_attr_helper(item_obj, self._semantic_id_attr)
                        if item_semantic_id in to_delete_ids:
                            item_id = self._treeview_id_lookup[item_semantic_id]
                            to_delete_ids.remove(item_semantic_id)
                            self.custom_upsert(item_obj, parent=cur_event_id)
                        else:
                            item_id = self.custom_upsert(item_obj, parent=cur_event_id)

                        if self.index(item_id) != item_idx or self.parent(item_id) != cur_event_id:
                            self.move(item_id, cur_event_id, item_idx)