from __future__ import annotations
import os
import logging
import sys
from typing import List, Tuple
from datetime import datetime
import tkinter
from PIL import ImageGrab, Image
if sys.platform == 'win32':
    import ctypes

from utils.io_utils import sanitize_string
from utils.constants import const
from utils.config_manager import config
from utils import io_utils
from routing.route_events import EventDefinition, EventFolder, EventGroup, EventItem, TrainerEventDefinition
import routing.router
from pkmn import gen_factory
from controllers.undo_manager import UndoManager


logger = logging.getLogger(__name__)


def handle_exceptions(controller_fn):
    # must wrap an instance method from the MainController class
    def wrapper(*args, **kwargs):
        try:
            controller_fn(*args, **kwargs)
        except Exception as e:
            logger.error(f"Trying to run function: {controller_fn}, got error: {e}")
            logger.exception(e)
            controller:MainController = args[0]
            controller._on_exception(f"{type(e)}: {e}")
    
    return wrapper


class MainController:
    def __init__(self):
        self._data:routing.router.Router = routing.router.Router()
        self._current_preview_event = None
        self._route_name = ""
        self._selected_ids = []
        self._is_record_mode_active = False
        self._exception_info = []
        self._message_info = []
        self._route_filter_types = []
        self._route_search = ""
        self._unsaved_changes = False
        self._custom_image_path = None

        self._name_change_events = []
        self._version_change_events = []
        self._route_change_events = []
        self._event_change_events = []
        self._event_selection_events = []
        self._event_preview_events = []
        self._record_mode_change_events = []
        self._message_events = []
        self._exception_events = []

        self._pre_save_hooks = []
        
        # Undo manager for event list changes
        self._undo_manager = UndoManager(max_steps=15)
    
    def get_next_exception_info(self):
        if not len(self._exception_info):
            return None
        return self._exception_info.pop(0)

    def get_next_message_info(self):
        if not len(self._message_info):
            return None
        return self._message_info.pop(0)


    #####
    # Registration methods
    #####

    def register_name_change(self, tk_obj):
        new_event_name = const.EVENT_NAME_CHANGE.format(len(self._name_change_events))
        self._name_change_events.append((tk_obj, new_event_name))
        return new_event_name

    def register_version_change(self, tk_obj):
        new_event_name = const.EVENT_VERSION_CHANGE.format(len(self._version_change_events))
        self._version_change_events.append((tk_obj, new_event_name))
        return new_event_name

    def register_route_change(self, tk_obj):
        new_event_name = const.EVENT_ROUTE_CHANGE.format(len(self._route_change_events))
        self._route_change_events.append((tk_obj, new_event_name))
        return new_event_name

    def register_event_update(self, tk_obj):
        new_event_name = const.EVENT_EVENT_CHANGE.format(len(self._event_change_events))
        self._event_change_events.append((tk_obj, new_event_name))
        return new_event_name

    def register_event_selection(self, tk_obj):
        new_event_name = const.EVENT_SELECTION_CHANGE.format(len(self._event_selection_events))
        self._event_selection_events.append((tk_obj, new_event_name))
        return new_event_name

    def register_event_preview(self, tk_obj):
        new_event_name = const.EVENT_PREVIEW_CHANGE.format(len(self._event_preview_events))
        self._event_preview_events.append((tk_obj, new_event_name))
        return new_event_name

    def register_record_mode_change(self, tk_obj):
        new_event_name = const.EVENT_RECORD_MODE_CHANGE.format(len(self._record_mode_change_events))
        self._record_mode_change_events.append((tk_obj, new_event_name))
        return new_event_name

    def register_message_callback(self, tk_obj):
        new_event_name = const.MESSAGE_EXCEPTION.format(len(self._message_events))
        self._message_events.append((tk_obj, new_event_name))
        return new_event_name

    def register_exception_callback(self, tk_obj):
        new_event_name = const.EVENT_EXCEPTION.format(len(self._exception_events))
        self._exception_events.append((tk_obj, new_event_name))
        return new_event_name

    def register_pre_save_hook(self, fn_obj):
        self._pre_save_hooks.append(fn_obj)
    
    #####
    # Event callbacks
    #####

    def _safely_generate_events(self, event_list):
        to_delete = []
        for cur_idx, (tk_obj, cur_event_name) in enumerate(event_list):
            try:
                tk_obj.event_generate(cur_event_name, when="tail")
            except tkinter.TclError:
                logger.info(f"Removing the following event due to TclError: {cur_event_name}")
                to_delete.append(cur_idx)
        
        for cur_idx in sorted(to_delete, reverse=True):
            del event_list[cur_idx]
    
    def _on_name_change(self):
        self._safely_generate_events(self._name_change_events)
    
    def _on_version_change(self):
        self._safely_generate_events(self._version_change_events)
    
    def _on_route_change(self):
        self._unsaved_changes = True
        self._safely_generate_events(self._route_change_events)

    def _on_event_change(self):
        self._safely_generate_events(self._event_change_events)
        self._on_route_change()

    def _on_event_selection(self):
        self._safely_generate_events(self._event_selection_events)

    def _on_event_preview(self):
        self._safely_generate_events(self._event_preview_events)

    def _on_record_mode_change(self):
        self._safely_generate_events(self._record_mode_change_events)

    def _on_info_message(self, info_message):
        self._message_info.append(info_message)
        self._safely_generate_events(self._message_events)

    def _on_exception(self, exception_message):
        self._exception_info.append(exception_message)
        self._safely_generate_events(self._exception_events)
    
    def _fire_pre_save_hooks(self):
        for cur_hook in self._pre_save_hooks:
            try:
                cur_hook()
            except Exception:
                logger.exception(f"Failed to run pre-save hook: {cur_hook}")

    ######
    # Methods that induce a state change
    ######

    @handle_exceptions
    def select_new_events(self, all_event_ids):
        self._selected_ids = all_event_ids
        if len(all_event_ids) != 0:
            self._current_preview_event = None

        # kind of gross to have repeated check, but we want all state fully changed before triggering events
        self._on_event_selection()
        if len(all_event_ids) != 0:
            self._on_event_preview()

    @handle_exceptions
    def set_preview_trainer(self, trainer_name):
        if self._current_preview_event is not None and self._current_preview_event.trainer_def.trainer_name == trainer_name:
            return
        
        if gen_factory.current_gen_info().trainer_db().get_trainer(trainer_name) is None:
            self._current_preview_event = None
        else:
            self._current_preview_event = EventDefinition(trainer_def=TrainerEventDefinition(trainer_name))

        self._on_event_preview()

    @handle_exceptions
    def update_existing_event(self, event_group_id:int, new_event:EventDefinition):
        if new_event.learn_move is not None and new_event.learn_move.source == const.MOVE_SOURCE_LEVELUP:
            return self.update_levelup_move(new_event.learn_move)
        # Save state BEFORE the operation (adds pre-op state to stack)
        self._undo_manager.save_state(self._data, is_post_operation=False)
        self._data.replace_event_group(event_group_id, new_event)
        # Save state AFTER the operation (updates current state)
        self._undo_manager.save_state(self._data, is_post_operation=True)
        self._on_event_change()

    @handle_exceptions
    def update_levelup_move(self, new_learn_move_event):
        # Save state BEFORE the operation (adds pre-op state to stack)
        self._undo_manager.save_state(self._data, is_post_operation=False)
        self._data.replace_levelup_move_event(new_learn_move_event)
        # Save state AFTER the operation (updates current state)
        self._undo_manager.save_state(self._data, is_post_operation=True)
        self._on_event_change()
    
    @handle_exceptions
    def add_area(self, area_name, include_rematches, insert_after_id):
        self._data.add_area(
            area_name=area_name,
            insert_after=insert_after_id,
            include_rematches=include_rematches
        )
        self._on_route_change()

    @handle_exceptions
    def create_new_route(self, solo_mon, base_route_path, pkmn_version, custom_dvs=None, custom_ability_idx=None, custom_nature=None):
        if base_route_path == const.EMPTY_ROUTE_NAME:
            base_route_path = None

        self._route_name = ""
        self._selected_ids = []
        try:
            self._data.new_route(solo_mon, base_route_path, pkmn_version=pkmn_version, custom_dvs=custom_dvs, custom_ability_idx=custom_ability_idx, custom_nature=custom_nature)
        except Exception as e:
            logger.error(f"Exception ocurred trying to copy route: {base_route_path}")
            logger.exception(e)
            # load an empty route, just in case
            self._data.new_route("Abra")
            raise e
        finally:
            self._undo_manager.clear()
            # Save initial state after route is created
            if self._data.init_route_state is not None:
                self._undo_manager.save_state(self._data)
            self._on_name_change()
            self._on_version_change()
            self._on_event_selection()
            self._on_route_change()
    
    @handle_exceptions
    def load_route(self, full_path_to_route):
        try:
            _, route_name = os.path.split(full_path_to_route)
            route_name = os.path.splitext(route_name)[0]
            self._route_name = route_name

            self._data.load(full_path_to_route)
            self._selected_ids = []
        except Exception as e:
            logger.error(f"Exception ocurred trying to load route: {full_path_to_route}")
            logger.exception(e)
            self._route_name = ""
            # load an empty route, just in case. Hardcoded, but wtv, Abra is in every game
            self._data.new_route("Abra")
            raise e
        finally:
            self._undo_manager.clear()
            # Save initial state after route is loaded
            if self._data.init_route_state is not None:
                self._undo_manager.save_state(self._data)
            self._on_name_change()
            self._on_version_change()
            self._on_event_selection()
            self._on_route_change()
            self._unsaved_changes = False

    @handle_exceptions
    def customize_innate_stats(self, new_dvs, new_ability, new_nature):
        self._data.change_current_innate_stats(new_dvs, new_ability, new_nature)
        self._on_route_change()

    @handle_exceptions
    def move_groups_up(self, event_ids):
        # Save state BEFORE the operation (adds pre-op state to stack)
        self._undo_manager.save_state(self._data, is_post_operation=False)
        for cur_event in event_ids:
            self._data.move_event_object(cur_event, True)
        # Save state AFTER the operation (updates current state)
        self._undo_manager.save_state(self._data, is_post_operation=True)
        self._on_route_change()

    @handle_exceptions
    def move_groups_down(self, event_ids):
        # Save state BEFORE the operation (adds pre-op state to stack)
        self._undo_manager.save_state(self._data, is_post_operation=False)
        for cur_event in event_ids:
            self._data.move_event_object(cur_event, False)
        # Save state AFTER the operation (updates current state)
        self._undo_manager.save_state(self._data, is_post_operation=True)
        self._on_route_change()

    @handle_exceptions
    def move_groups_to_adjacent_folder_up(self, event_ids):
        # Save state BEFORE the operation (adds pre-op state to stack)
        self._undo_manager.save_state(self._data, is_post_operation=False)
        for cur_event in event_ids:
            self._data.move_event_to_adjacent_folder(cur_event, True)
        # Save state AFTER the operation (updates current state)
        self._undo_manager.save_state(self._data, is_post_operation=True)
        self._on_route_change()

    @handle_exceptions
    def move_groups_to_adjacent_folder_down(self, event_ids):
        # NOTE: list is already reversed in main_window before being passed here
        # Save state BEFORE the operation (adds pre-op state to stack)
        self._undo_manager.save_state(self._data, is_post_operation=False)
        for cur_event in event_ids:
            self._data.move_event_to_adjacent_folder(cur_event, False)
        # Save state AFTER the operation (updates current state)
        self._undo_manager.save_state(self._data, is_post_operation=True)
        self._on_route_change()

    @handle_exceptions
    def delete_events(self, event_ids):
        # Save state BEFORE the operation (adds pre-op state to stack)
        self._undo_manager.save_state(self._data, is_post_operation=False)
        self._data.batch_remove_events(event_ids)
        # Save state AFTER the operation (updates current state)
        self._undo_manager.save_state(self._data, is_post_operation=True)

        selection_changed = False
        for cur_event_id in event_ids:
            if cur_event_id in self._selected_ids:
                self._selected_ids.remove(cur_event_id)
                selection_changed = True

        self._on_route_change()
        if selection_changed:
            self._on_event_selection()

    @handle_exceptions
    def purge_empty_folders(self):
        while True:
            deleted_ids = []
            for cur_folder_name, cur_folder in self._data.folder_lookup.items():
                if cur_folder_name == const.ROOT_FOLDER_NAME:
                    continue
                if len(cur_folder.children) == 0:
                    deleted_ids.append(cur_folder.group_id)
            
            if len(deleted_ids) != 0:
                self.delete_events(deleted_ids)
            else:
                break
    
    @handle_exceptions
    def transfer_to_folder(self, event_ids, new_folder_name):
        # Save state BEFORE the operation (adds pre-op state to stack)
        self._undo_manager.save_state(self._data, is_post_operation=False)
        self._data.transfer_events(event_ids, new_folder_name)
        # Save state AFTER the operation (updates current state)
        self._undo_manager.save_state(self._data, is_post_operation=True)
        self._on_route_change()

    @handle_exceptions
    def split_folder_at_current_event(self, event_id):
        """Split the folder containing the current event at the current event position.
        
        Creates a new folder and moves all events from the current event to the end of the
        current folder into the new folder. The new folder is named based on the old folder.
        """
        # Get the event object
        event_obj = self.get_event_by_id(event_id)
        if event_obj is None:
            return
        
        # Don't allow splitting if the selected item is itself a folder
        if isinstance(event_obj, EventFolder):
            return
        
        # Check if the event is inside a folder (not root)
        parent_folder = event_obj.parent
        if parent_folder is None or parent_folder.name == const.ROOT_FOLDER_NAME:
            # Event is not inside a folder, cannot split
            return
        
        # Find the index of the current event in the parent folder's children
        try:
            event_index = parent_folder.children.index(event_obj)
        except ValueError:
            # Event not found in parent's children (shouldn't happen, but handle gracefully)
            return
        
        # Get all events from the current event to the end of the folder
        events_to_move = parent_folder.children[event_index:]
        if len(events_to_move) == 0:
            # No events to move
            return
        
        # Generate a unique folder name based on the old folder name
        old_folder_name = parent_folder.name
        new_folder_name = self._generate_unique_folder_name(old_folder_name)
        
        # Save state BEFORE the operation
        self._undo_manager.save_state(self._data, is_post_operation=False)
        
        # Create the new folder right after the old folder
        # Create the new folder
        new_folder_id = self._data.add_event_object(
            new_folder_name=new_folder_name,
            insert_after=parent_folder.group_id,
            dest_folder_name=parent_folder.parent.name,
            recalc=False
        )
        
        # Move all events from the current event to the end into the new folder
        event_ids_to_move = [event.group_id for event in events_to_move]
        self._data.transfer_events(event_ids_to_move, new_folder_name)
        
        # Save state AFTER the operation
        self._undo_manager.save_state(self._data, is_post_operation=True)
        self._on_route_change()
        
        # Select the new folder
        self.select_new_events([new_folder_id])
    
    def _generate_unique_folder_name(self, base_name):
        """Generate a unique folder name based on the base name."""
        all_folder_names = set(self.get_all_folder_names())
        
        # Try simple variations first
        variations = [
            f"{base_name} (Part 2)",
            f"{base_name} (2)",
            f"{base_name} - Split",
        ]
        
        for variation in variations:
            if variation not in all_folder_names:
                return variation
        
        # If all simple variations are taken, try numbered versions
        counter = 2
        while True:
            candidate = f"{base_name} (Part {counter})"
            if candidate not in all_folder_names:
                return candidate
            counter += 1

    @handle_exceptions
    def new_event(self, event_def:EventDefinition, insert_after:int=None, insert_before:int=None, dest_folder_name=const.ROOT_FOLDER_NAME, do_select=True):
        # Save state BEFORE the operation (adds pre-op state to stack)
        self._undo_manager.save_state(self._data, is_post_operation=False)
        result = self._data.add_event_object(event_def=event_def, insert_after=insert_after, insert_before=insert_before, dest_folder_name=dest_folder_name)
        # Save state AFTER the operation (updates current state)
        self._undo_manager.save_state(self._data, is_post_operation=True)
        self._on_route_change()
        if do_select:
            self.select_new_events([result])
        return result

    @handle_exceptions
    def finalize_new_folder(self, new_folder_name, prev_folder_name=None, insert_after=None):
        # Save state BEFORE the operation (adds pre-op state to stack)
        self._undo_manager.save_state(self._data, is_post_operation=False)
        if prev_folder_name is None and insert_after is None:
            self._data.add_event_object(new_folder_name=new_folder_name)
        elif prev_folder_name is None:
            self._data.add_event_object(new_folder_name=new_folder_name, insert_after=insert_after)
        else:
            self._data.rename_event_folder(prev_folder_name, new_folder_name)
        # Save state AFTER the operation (updates current state)
        self._undo_manager.save_state(self._data, is_post_operation=True)

        self._on_route_change()

    @handle_exceptions
    def toggle_event_highlight(self, event_ids):
        for cur_event in event_ids:
            self._data.toggle_event_highlight(cur_event)
        
        self._on_route_change()
    
    @handle_exceptions
    def set_event_highlight(self, event_ids, highlight_num):
        """Set a specific highlight type (1-9) or None to remove all highlights."""
        for cur_event in event_ids:
            self._data.set_event_highlight(cur_event, highlight_num)
        
        self._on_route_change()

    @handle_exceptions
    def set_record_mode(self, new_record_mode):
        self._is_record_mode_active = new_record_mode
        self._on_record_mode_change()

    @handle_exceptions
    def set_route_filter_types(self, filter_options):
        self._route_filter_types = filter_options
        self._on_route_change()
        self._on_event_selection()

    @handle_exceptions
    def set_route_search(self, search):
        self._route_search = search
        self._on_route_change()
        self._on_event_selection()
    
    @handle_exceptions
    def load_all_custom_versions(self):
        gen_factory._gen_factory.reload_all_custom_gens(retry_skipped=True)
    
    @handle_exceptions
    def create_custom_version(self, base_version, custom_version):
        gen_factory._gen_factory.get_specific_version(base_version).create_new_custom_gen(custom_version)
        # Reload custom gens to include the newly created one
        gen_factory._gen_factory.reload_all_custom_gens(retry_skipped=True)
    
    def send_message(self, message):
        self._on_info_message(message)

    def trigger_exception(self, exception_message):
        self._on_exception(exception_message)

    def set_current_route_name(self, new_name) -> str:
        self._route_name = new_name
        self._on_name_change()

    ######
    # Methods that do not induce a state change
    ######

    def get_raw_route(self) -> routing.router.Router:
        return self._data

    def get_current_route_name(self) -> str:
        return self._route_name
    
    def set_custom_image_path(self, path: str):
        """Set the custom image path for screenshots. Set to None or empty string to use default."""
        if path and path.strip():
            # Strip quotes from the beginning and end of the path
            cleaned_path = path.strip().strip('"').strip("'")
            self._custom_image_path = cleaned_path if cleaned_path else None
        else:
            self._custom_image_path = None
    
    def get_custom_image_path(self) -> str:
        """Get the custom image path for screenshots. Returns None if using default."""
        return self._custom_image_path

    def get_preview_event(self):
        return self._current_preview_event

    def get_event_by_id(self, event_id) -> EventGroup:
        return self._data.get_event_obj(event_id)
    
    def has_errors(self):
        return self._data.root_folder.has_errors()
    
    def get_version(self):
        return self._data.pkmn_version
    
    def get_state_after(self, previous_event_id=None):
        if previous_event_id is None:
            return self._data.init_route_state

        prev_event = self.get_event_by_id(previous_event_id)
        if prev_event is None:
            return self._data.init_route_state
        
        return prev_event.init_state
    
    def get_init_state(self):
        return self._data.init_route_state
    
    def get_final_state(self):
        return self._data.get_final_state()
    
    def get_all_folder_names(self):
        return list(self._data.folder_lookup.keys())
    
    def get_invalid_folders(self, event_id):
        return self._data.get_invalid_folder_transfers(event_id)
    
    def get_dvs(self):
        return self._data.init_route_state.solo_pkmn.dvs
    
    def get_ability_idx(self):
        return self._data.init_route_state.solo_pkmn.ability_idx
    
    def get_ability(self):
        return self._data.init_route_state.solo_pkmn.ability
    
    def get_nature(self):
        return self._data.init_route_state.solo_pkmn.nature
    
    def get_defeated_trainers(self):
        return self._data.defeated_trainers
    
    def get_route_search_string(self) -> str:
        if not self._route_search:
            return None
        return self._route_search
    
    def get_route_filter_types(self) -> List[str]:
        if len(self._route_filter_types) == 0:
            return None
        return self._route_filter_types
    
    def is_empty(self):
        return len(self._data.root_folder.children) == 0
    
    def is_valid_levelup_move(self, new_move_def):
        return self._data.is_valid_levelup_move(new_move_def)
    
    def can_evolve_into(self, species_name):
        target_mon = gen_factory.current_gen_info().pkmn_db().get_pkmn(species_name)
        if target_mon is None:
            return False
        
        return target_mon.growth_rate == self.get_final_state().solo_pkmn.species_def.growth_rate

    def has_unsaved_changes(self) -> routing.router.Router:
        return self._unsaved_changes
    
    def can_undo(self) -> bool:
        """Check if undo is available."""
        return self._undo_manager.can_undo()
    
    @handle_exceptions
    def undo(self):
        """Undo the last event list change."""
        if not self.can_undo():
            return
        
        # Get the previous state
        previous_state = self._undo_manager.get_undo_state()
        if previous_state is None:
            return
        
        # Temporarily disable saving state to avoid creating a new undo entry
        # We'll restore the current state after undoing
        current_state = self._undo_manager._current_state
        
        # Restore the state (this will trigger route change)
        self._data.restore_events_from_state(previous_state)
        
        # Restore the current state pointer (the state we just restored becomes current)
        self._undo_manager._current_state = previous_state
        
        # Clear selection since event IDs may have changed
        self._selected_ids = []
        
        # Trigger updates
        self._on_event_selection()
        self._on_route_change()
    
    def get_all_selected_ids(self, allow_event_items=True):
        if allow_event_items:
            return self._selected_ids

        return [x for x in self._selected_ids if not isinstance(self.get_event_by_id(x), EventItem)]
    
    def get_single_selected_event_id(self, allow_event_items=True):
        if len(self._selected_ids) == 0 or len(self._selected_ids) > 1:
            return None
        
        if not allow_event_items:
            event_obj = self.get_event_by_id(self._selected_ids[0])
            if isinstance(event_obj, EventItem):
                return None

        return self._selected_ids[0]
    
    def get_single_selected_event_obj(self, allow_event_items=True) -> EventGroup:
        return self.get_event_by_id(
            self.get_single_selected_event_id(allow_event_items=allow_event_items)
        )
    
    def get_active_state(self):
        # The idea here is we want to get the current state to operate on
        # MOST of the time, this is just the final state of the selected event
        # (since we will insert after the selected event)
        result = self.get_single_selected_event_obj(allow_event_items=False)
        if result is not None:
            return result.final_state
        
        # If no event is selected, because we are looking at an empty route
        # then just get the initial route state
        if self.is_empty():
            return self._data.init_route_state
        
        # If no event is selected, but the route is non-empty
        # then we will insert after the final event
        return self.get_final_state()

    
    def can_insert_after_current_selection(self):
        # Can always insert if the route is empty
        if self.is_empty():
            return True
        
        # Can't insert is if the route is non-empty, and nothing is selected
        cur_obj = self.get_single_selected_event_obj()
        if cur_obj is None:
            return False
        
        # Can't insert after EventItems, only other event types
        return not isinstance(cur_obj, EventItem)
    
    def find_first_event_by_trainer_name(self, trainer_name):
        """Find the first event in the route that matches the given trainer name."""
        def search_folder(folder):
            for child in folder.children:
                if isinstance(child, EventFolder):
                    result = search_folder(child)
                    if result is not None:
                        return result
                elif isinstance(child, routing.route_events.EventGroup):
                    if (child.event_definition.trainer_def is not None and 
                        child.event_definition.trainer_def.trainer_name == trainer_name):
                        return child.group_id
            return None
        
        return search_folder(self._data.root_folder)

    def save_route(self, route_name):
        try:
            self._fire_pre_save_hooks()
            self._data.save(route_name)
            self.send_message(f"Successfully saved route: {route_name}")
            self._unsaved_changes = False
        except Exception as e:
            self.trigger_exception(f"Couldn't save route due to exception! {type(e)}: {e}")
    
    def export_notes(self, route_name):
        out_path = self._data.export_notes(route_name)
        self.send_message(f"Exported notes to: {out_path}")
    
    def take_screenshot(self, image_name, bbox, custom_path=None):
        try:
            if self.is_empty():
                return
            
            # Get current date/time in format YYYYMMDDHHMMSS
            date_prefix = datetime.now().strftime("%Y%m%d%H%M%S")
            
            # Determine which directory to use
            # Use custom_path parameter if provided, otherwise use stored custom path, otherwise use default
            path_to_use = custom_path if custom_path is not None else self._custom_image_path
            
            if path_to_use and path_to_use.strip():
                # Strip quotes and whitespace, then normalize the path
                path_to_use = path_to_use.strip().strip('"').strip("'")
                path_to_use = os.path.normpath(path_to_use)
                if os.path.isdir(path_to_use):
                    # Path exists and is a directory, use it
                    save_dir = path_to_use
                else:
                    # Path doesn't exist, try to create it
                    try:
                        os.makedirs(path_to_use, exist_ok=True)
                        if os.path.isdir(path_to_use):
                            save_dir = path_to_use
                        else:
                            logger.warning(f"Could not create custom image directory {path_to_use}, using default")
                            save_dir = config.get_images_dir()
                    except Exception as e:
                        logger.warning(f"Could not create custom image directory {path_to_use}: {e}, using default")
                        save_dir = config.get_images_dir()
            else:
                save_dir = config.get_images_dir()
            
            out_path = io_utils.get_safe_path_no_collision(
                save_dir,
                f"{date_prefix}-{self.get_current_route_name()}_{image_name}",
                ext=".png",
            )
            
            # Handle multi-monitor setups on Windows
            # ImageGrab.grab(bbox=bbox) doesn't work correctly when the window is on a 
            # secondary monitor, especially when maximized - it produces black images.
            # Solution: Use Windows API to capture the entire virtual screen, then crop.
            left, top, right, bottom = bbox
            
            if sys.platform == 'win32':
                # Use Windows API to capture the entire virtual screen (all monitors)
                try:
                    # Get virtual screen dimensions
                    user32 = ctypes.windll.user32
                    virtual_width = user32.GetSystemMetrics(78)  # SM_CXVIRTUALSCREEN
                    virtual_height = user32.GetSystemMetrics(79)  # SM_CYVIRTUALSCREEN
                    virtual_left = user32.GetSystemMetrics(76)  # SM_XVIRTUALSCREEN
                    virtual_top = user32.GetSystemMetrics(77)  # SM_YVIRTUALSCREEN
                    
                    # Capture the entire virtual screen
                    full_screenshot = ImageGrab.grab(bbox=(
                        virtual_left, 
                        virtual_top, 
                        virtual_left + virtual_width, 
                        virtual_top + virtual_height
                    ))
                    
                    # Adjust bbox coordinates relative to virtual screen origin
                    adjusted_left = left - virtual_left
                    adjusted_top = top - virtual_top
                    adjusted_right = right - virtual_left
                    adjusted_bottom = bottom - virtual_top
                    
                    # Crop to the desired region
                    cropped_image = full_screenshot.crop((
                        adjusted_left, 
                        adjusted_top, 
                        adjusted_right, 
                        adjusted_bottom
                    ))
                    
                    # Verify the cropped image is not entirely black (common failure mode)
                    # Check a sample of pixels
                    try:
                        pixels = list(cropped_image.getdata())
                        if len(pixels) > 0:
                            sample_size = min(100, len(pixels))
                            sample = pixels[:sample_size]
                            all_black = all(
                                pixel == (0, 0, 0) if isinstance(pixel, tuple) and len(pixel) >= 3 
                                else pixel == 0 
                                for pixel in sample
                            )
                            
                            if all_black:
                                # Got a black image, try alternative method
                                logger.warning("Screenshot produced black image, trying alternative method")
                                raise ValueError("Black image detected")
                    except Exception:
                        pass  # If pixel check fails, proceed anyway
                    
                    cropped_image.save(out_path)
                except Exception as e:
                    # Fallback: Try using bbox directly (might work in some cases)
                    logger.warning(f"Virtual screen capture failed ({e}), trying direct bbox method")
                    try:
                        fallback_image = ImageGrab.grab(bbox=bbox)
                        # Check if fallback also produces black image
                        pixels = list(fallback_image.getdata())
                        if len(pixels) > 0:
                            sample_size = min(100, len(pixels))
                            sample = pixels[:sample_size]
                            all_black = all(
                                pixel == (0, 0, 0) if isinstance(pixel, tuple) and len(pixel) >= 3 
                                else pixel == 0 
                                for pixel in sample
                            )
                            if all_black:
                                raise ValueError("Direct bbox method also produced black image")
                        fallback_image.save(out_path)
                    except Exception as fallback_error:
                        logger.error(f"All screenshot methods failed: {fallback_error}")
                        raise ValueError(f"Could not capture screenshot on secondary monitor. Original error: {e}, Fallback error: {fallback_error}")
            else:
                # For non-Windows platforms, use standard bbox method
                ImageGrab.grab(bbox=bbox).save(out_path)
            
            self.send_message(f"Saved screenshot to: {out_path}")
        except Exception as e:
            self.trigger_exception(f"Couldn't save screenshot due to exception! {type(e)}: {e}")

    def is_record_mode_active(self):
        return self._is_record_mode_active
    
    def get_move_idx(self, move_name, state=None):
        if state is None:
            state = self.get_final_state()
        
        move_idx = None
        move_name = sanitize_string(move_name)
        for cur_idx, cur_move in enumerate(state.solo_pkmn.move_list):
            if sanitize_string(cur_move) == move_name:
                move_idx = cur_idx
                break
        
        return move_idx
    
    def _walk_events_helper(self, cur_folder:EventFolder, cur_event_id:int, cur_event_found:bool, enabled_only:bool, walk_forward=True) -> Tuple[bool, EventGroup]:
        if walk_forward:
            iterable = cur_folder.children
        else:
            iterable = reversed(cur_folder.children)

        for test_obj in iterable:
            if isinstance(test_obj, EventGroup):
                if cur_event_found and test_obj.is_enabled():
                    return cur_event_found, test_obj
                elif test_obj.group_id == cur_event_id:
                    cur_event_found = True
            elif isinstance(test_obj, EventFolder):
                cur_event_found, prev_result = self._walk_events_helper(test_obj, cur_event_id, cur_event_found, enabled_only, walk_forward=walk_forward)
                if cur_event_found and prev_result is not None:
                    return cur_event_found, prev_result
            else:
                logger.error(f"Encountered unexpected types walking events: {type(test_obj)}")

        return cur_event_found, None

    def get_next_event(self, cur_event_id=None, enabled_only=False) -> EventGroup:
        return self._walk_events_helper(
            self._data.root_folder,
            cur_event_id,
            cur_event_id == None,
            enabled_only=enabled_only,
            walk_forward=True
        )[1]

    def get_previous_event(self, cur_event_id=None, enabled_only=False) -> EventGroup:
        return self._walk_events_helper(
            self._data.root_folder,
            cur_event_id,
            cur_event_id == None,
            enabled_only,
            walk_forward=False
        )[1]
