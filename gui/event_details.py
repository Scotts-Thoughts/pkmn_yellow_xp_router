import tkinter as tk
from tkinter import ttk
import logging
from controllers.battle_summary_controller import BattleSummaryController
import time

from controllers.main_controller import MainController
from gui import custom_components, route_event_components, battle_summary
from gui.pkmn_components.state_viewer import StateViewer
from routing.route_events import EventDefinition, EventFolder, EventGroup, EventItem
from utils.constants import const
from utils.config_manager import config
from utils import tk_utils
from pkmn.gen_factory import current_gen_info

logger = logging.getLogger(__name__)


class EventDetails(ttk.Frame):
    def __init__(self, controller:MainController, *args, **kwargs):
        self.state_summary_width = 900
        self.battle_summary_width = 1400
        self.save_delay = 2
        super().__init__(*args, **kwargs, width=self.state_summary_width)
        self.grid_propagate(False)

        self._controller = controller
        self._battle_summary_controller = BattleSummaryController(self._controller)
        self._ignore_tab_switching = False
        self._cur_delayed_event_id = None
        self._cur_delayed_event_start = None

        self.columnconfigure(0, weight=1)
        self.rowconfigure(0, weight=1)

        self.notebook_holder = ttk.Frame(self)
        self.notebook_holder.grid(row=0, column=0, padx=2, pady=2, sticky=tk.NSEW)
        self.notebook_holder.columnconfigure(0, weight=1)
        self.notebook_holder.rowconfigure(0, weight=1)
        self.tabbed_states = ttk.Notebook(self.notebook_holder)
        self.tabbed_states.enable_traversal()

        self.pre_state_frame = ttk.Frame(self.tabbed_states)
        self.pre_state_frame.grid(row=0, column=0, padx=2, pady=2, sticky=tk.NSEW)
        self.auto_change_tab_checkbox = custom_components.CheckboxLabel(self.pre_state_frame, text="Switch tabs automatically", flip=True, toggle_command=self._handle_auto_switch_toggle)
        self.auto_change_tab_checkbox.grid(column=1, row=0, padx=10, pady=5, columnspan=2)
        self.auto_change_tab_checkbox.set_checked(config.do_auto_switch())
        self.state_pre_viewer = StateViewer(self.pre_state_frame)
        self.state_pre_viewer.grid(column=1, row=2, padx=10, pady=10, columnspan=2)

        self.pre_state_frame.columnconfigure(0, weight=1)
        self.pre_state_frame.columnconfigure(3, weight=1)
        self.pre_state_frame.rowconfigure(5, weight=1)

        self.battle_summary_frame = battle_summary.BattleSummary(self._battle_summary_controller, self.tabbed_states, width=self.battle_summary_width)
        self.battle_summary_frame.grid(row=1, column=0, padx=2, pady=2)
        self.battle_summary_frame.show_contents()

        self.tabbed_states.add(self.pre_state_frame, text="Pre-event State")
        self.pre_state_tab_index = 0
        self.tabbed_states.add(self.battle_summary_frame, text="Battle Summary")
        self.battle_summary_tab_index = 1
        self.tabbed_states.grid(row=0, column=0, sticky=tk.NSEW)
        self.tabbed_states.columnconfigure(0, weight=1)
        self.tabbed_states.rowconfigure(0, weight=1)

        self.event_details_frame = ttk.Frame(self.pre_state_frame)
        self.event_details_frame.grid(row=5, column=0, columnspan=4, sticky=tk.NSEW)
        self.event_details_frame.rowconfigure(0, weight=1, uniform="group")
        self.event_details_frame.rowconfigure(2, weight=1, uniform="group")
        self.event_details_frame.columnconfigure(0, weight=1, uniform="group")
        self.event_details_frame.columnconfigure(2, weight=1, uniform="group")

        self.footer_frame = ttk.Frame(self)
        self.footer_frame.grid(row=1, column=0, padx=5, pady=(2, 2), sticky=tk.EW)

        # create this slightly out of order because we need the reference
        self.event_editor_lookup = route_event_components.EventEditorFactory(self.event_details_frame)
        self.current_event_editor = None

        self.trainer_notes = route_event_components.EventEditorFactory(self.footer_frame).get_editor(
            route_event_components.EditorParams(const.TASK_NOTES_ONLY, None, None),
            save_callback=self.update_existing_event,
            delayed_save_callback=self.update_existing_event_after_delay,
            notes_visibility_callback=self._update_notes_visibility_in_battle_summary,
        )
        self.trainer_notes.grid(row=0, column=0, sticky=tk.EW)

        self.footer_frame.columnconfigure(0, weight=1)

        self.tabbed_states.bind('<<NotebookTabChanged>>', self._tab_changed_callback)
        self.bind(self._controller.register_event_selection(self), self._handle_selection)
        self.bind(self._controller.register_record_mode_change(self), self._handle_selection)
        self.bind(self._controller.register_route_change(self), self._handle_route_change)
        self.bind(self._controller.register_version_change(self), self._handle_version_change)
        self.bind(self._battle_summary_controller.register_nonload_change(self), self.update_existing_event_after_delay)
        # Bind to battle summary refresh to update notes visibility when matchups change
        self.bind(self._battle_summary_controller.register_refresh(self), self._on_battle_summary_refresh)
        self._controller.register_pre_save_hook(self.force_and_clear_event_update)

        # Bind keyboard shortcuts for pre-fight rare candies
        self.bind('<F3>', self._increment_prefight_candies)
        self.bind('<F4>', self._decrement_prefight_candies)

        self._tab_changed_callback()
    
    def _should_show_notes_in_battle_summary(self):
        """Determine if notes should be shown in battle summary based on visibility mode and number of visible matchups."""
        if not self.battle_summary_frame.should_render:
            return False
        
        mode = config.get_notes_visibility_mode()
        
        if mode == "never":
            return False
        elif mode == "always":
            return True
        elif mode == "when_space_allows":
            # Count visible Pokemon matchups
            visible_matchups = sum(self.battle_summary_frame._did_draw_mon_pairs)
            # If there are 3 or fewer matchups, there's room for notes
            # If there are 4 or more matchups, hide notes to make room for damage ranges
            return visible_matchups <= 3
        else:
            # Default to when_space_allows behavior
            visible_matchups = sum(self.battle_summary_frame._did_draw_mon_pairs)
            return visible_matchups <= 3
    
    def _on_battle_summary_refresh(self, *args, **kwargs):
        """Called when battle summary refreshes - update notes visibility."""
        self.after_idle(self._update_notes_visibility_in_battle_summary)
    
    def _update_notes_visibility_in_battle_summary(self):
        """Update notes visibility based on current battle summary state."""
        if not self.tabbed_states.select():
            return
        
        selected_tab_index = self.tabbed_states.index(self.tabbed_states.select())
        if selected_tab_index == self.battle_summary_tab_index:
            mode = config.get_notes_visibility_mode()
            should_show = self._should_show_notes_in_battle_summary()
            
            if should_show:
                # Always use minimal space for footer - let it use natural size, positioned at bottom
                # Battle summary takes remaining space, footer sits at bottom with minimal padding
                self.rowconfigure(0, weight=1)
                self.rowconfigure(1, weight=0)  # Footer doesn't expand, uses natural size
                self.footer_frame.grid(row=1, column=0, padx=5, pady=(2, 2), sticky=tk.EW)  # Only expand horizontally
                self.footer_frame.rowconfigure(0, weight=0)  # Notes editor uses natural size
                
                # If mode is "always", enable scrollbar on battle summary (only if needed)
                if mode == "always":
                    # Use after_idle to ensure layout is updated before checking scrollbar need
                    self.after_idle(self.battle_summary_frame.enable_scrollbar)
                else:
                    self.battle_summary_frame.disable_scrollbar()
            else:
                # Hide footer and let row 0 take all space
                self.rowconfigure(0, weight=1)
                self.rowconfigure(1, weight=0)
                self.footer_frame.grid_forget()
                self.battle_summary_frame.disable_scrollbar()
    
    def _tab_changed_callback(self, *args, **kwargs):
        if not self.tabbed_states.select():
            # This doesn't occur during normal processing, but can occur when closing down the app
            # Just prevent an extra error from occuring
            # this value should be a string containing an identifier (or empty, if no tabs exist)
            return

        selected_tab_index = self.tabbed_states.index(self.tabbed_states.select())
        if selected_tab_index == self.battle_summary_tab_index:
            self.configure(width=self.battle_summary_width)
            def show_and_update():
                self.battle_summary_frame.show_contents()
                self.after(350, self._update_notes_visibility_in_battle_summary)
            self.battle_summary_frame.after(300, show_and_update)
            # Initial visibility check (will be updated after battle summary renders)
            self._update_notes_visibility_in_battle_summary()
        else:
            self.battle_summary_frame.hide_contents()
            self.configure(width=self.state_summary_width)
            # On pre-state tab, always show footer and configure rows properly
            self.rowconfigure(0, weight=1)
            self.rowconfigure(1, weight=0)
            self.footer_frame.grid(row=1, column=0, padx=5, pady=(2, 2), sticky=tk.EW)
    
    def change_tabs(self, *args, **kwargs):
        if not self.tabbed_states.select():
            return

        selected_tab_index = self.tabbed_states.index(self.tabbed_states.select())
        if selected_tab_index == self.battle_summary_tab_index:
            self.tabbed_states.select(self.pre_state_tab_index)
        else:
            self.tabbed_states.select(self.battle_summary_tab_index)
    
    def _handle_version_change(self, *args, **kwargs):
        self._battle_summary_controller.load_empty()
        self.battle_summary_frame.configure_weather(current_gen_info().get_valid_weather())
        self.battle_summary_frame.configure_setup_moves(current_gen_info().get_stat_modifer_moves())
    
    def _handle_auto_switch_toggle(self, *args, **kwargs):
        config.set_auto_switch(self.auto_change_tab_checkbox.is_checked())
    
    def _handle_route_change(self, *args, **kwargs):
        event_group = self._controller.get_single_selected_event_obj()
        if event_group is None:
            self.state_pre_viewer.set_state(self._controller.get_init_state())
            self.battle_summary_frame.set_team(None)
        else:
            self.state_pre_viewer.set_state(event_group.init_state)
            if event_group.event_definition.trainer_def is not None:
                self.battle_summary_frame.set_team(
                    event_group.event_definition.get_pokemon_list(),
                    cur_state=event_group.init_state,
                    event_group=event_group
                )
            else:
                self.battle_summary_frame.set_team(None)
    
    def _handle_selection(self, *args, **kwargs):
        event_group = self._controller.get_single_selected_event_obj()

        if event_group is None:
            self.show_event_details(None, self._controller.get_init_state(), self._controller.get_final_state(), allow_updates=False)
        elif isinstance(event_group, EventFolder):
            self.show_event_details(event_group.event_definition, event_group.init_state, event_group.final_state)
        else:
            do_allow_updates = (
                isinstance(event_group, EventGroup) or 
                event_group.event_definition.get_event_type() == const.TASK_LEARN_MOVE_LEVELUP
            )
            trainer_event_group = event_group
            if isinstance(trainer_event_group, EventItem) and event_group.event_definition.learn_move is None:
                trainer_event_group = trainer_event_group.parent
            
            if self._ignore_tab_switching or self.auto_change_tab_checkbox.is_checked():
                if trainer_event_group.event_definition.trainer_def is not None:
                    self.tabbed_states.select(self.battle_summary_tab_index)
                else:
                    self.tabbed_states.select(self.pre_state_tab_index)
            self.show_event_details(event_group.event_definition, event_group.init_state, event_group.final_state, do_allow_updates, event_group=trainer_event_group)
    
    def show_event_details(self, event_def:EventDefinition, init_state, final_state, allow_updates=True, event_group:EventGroup=None):
        self.force_and_clear_event_update()
        if self._controller.is_record_mode_active():
            allow_updates = False

        self.state_pre_viewer.set_state(init_state)
        if self.current_event_editor is not None:
            self.current_event_editor.grid_forget()
            self.current_event_editor = None

        if event_def is None:
            self.trainer_notes.load_event(None)
            self.battle_summary_frame.set_team(None)
        else:
            self.trainer_notes.load_event(event_def)
            if event_def.trainer_def is not None:
                self.battle_summary_frame.set_team(event_def.get_pokemon_list(), cur_state=init_state, event_group=event_group)
            else:
                self.battle_summary_frame.set_team(None)

            if event_def.get_event_type() != const.TASK_NOTES_ONLY:
                # TODO: fix this gross ugly hack
                self.current_event_editor = self.event_editor_lookup.get_editor(
                    route_event_components.EditorParams(event_def.get_event_type(), None, init_state),
                    save_callback=self.update_existing_event,
                    delayed_save_callback=self.update_existing_event_after_delay,
                    is_enabled=allow_updates
                )
                self.current_event_editor.load_event(event_def)
                self.current_event_editor.grid(row=1, column=1)

    def update_existing_event(self, *args, **kwargs):
        self._event_update_helper(self._controller.get_single_selected_event_id())

    def update_existing_event_after_delay(self, *args, **kwargs):
        to_save = self._controller.get_single_selected_event_id()
        if self._cur_delayed_event_id is not None and self._cur_delayed_event_id != to_save:
            logger.error(f"Unexpected switch of event id from {self._cur_delayed_event_id} to {to_save}, something has gone wrong")

        self._cur_delayed_event_id = to_save
        self._cur_delayed_event_start = time.time() + self.save_delay
        self.after(int(self.save_delay * 1000), self._delayed_event_update)
    
    def force_and_clear_event_update(self, *args, **kwargs):
        if self._cur_delayed_event_id is None:
            return
        
        self._event_update_helper(self._cur_delayed_event_id)

    def _delayed_event_update(self, *args, **kwargs):
        if self._cur_delayed_event_id is None or self._cur_delayed_event_start is None:
            # if the save has already occurred, silently exit
            return
        
        if self._cur_delayed_event_start - time.time() > 0:
            return

        self._event_update_helper(self._cur_delayed_event_id)

    def _event_update_helper(self, event_to_update):
        if event_to_update is None:
            return

        if self._cur_delayed_event_id is not None and self._cur_delayed_event_id != event_to_update:
            logger.error(f"Found delayed update for: {self._cur_delayed_event_id} which is different from the current update occuring for {event_to_update}")
        
        self._cur_delayed_event_id = None
        self._cur_delayed_event_start = None
        try:
            if self.current_event_editor is None:
                new_event = EventDefinition()
            else:
                new_event = self.current_event_editor.get_event()
            
            if new_event.get_event_type() == const.TASK_TRAINER_BATTLE:
                new_trainer_def = self._battle_summary_controller.get_partial_trainer_definition()
                if new_trainer_def is None:
                    logger.error(f"Expected to get updated trainer def from battle summary controller, but got None instead")
                else:
                    new_trainer_def.exp_split = new_event.trainer_def.exp_split
                    new_trainer_def.pay_day_amount = new_event.trainer_def.pay_day_amount
                    new_trainer_def.mon_order = new_event.trainer_def.mon_order
                    new_event.trainer_def = new_trainer_def
            
            new_event.notes = self.trainer_notes.get_event().notes
        except Exception as e:
            logger.error("Exception occurred trying to update current event")
            logger.exception(e)
            self._controller.trigger_exception("Exception occurred trying to update current event")
            return
        
        self._controller.update_existing_event(event_to_update, new_event)
    
    def take_battle_summary_screenshot(self, *args, **kwargs):
        if self.tabbed_states.index(self.tabbed_states.select()) == self.battle_summary_tab_index:
            bbox = self.battle_summary_frame.get_content_bounding_box()
            self._battle_summary_controller.take_screenshot(bbox)
    
    def take_player_ranges_screenshot(self, *args, **kwargs):
        if self.tabbed_states.index(self.tabbed_states.select()) == self.battle_summary_tab_index:
            self._take_scaled_screenshot(
                self.battle_summary_frame.get_player_ranges_bounding_box,
                suffix="_player_ranges"
            )
    
    def take_enemy_ranges_screenshot(self, *args, **kwargs):
        if self.tabbed_states.index(self.tabbed_states.select()) == self.battle_summary_tab_index:
            self._take_scaled_screenshot(
                self.battle_summary_frame.get_enemy_ranges_bounding_box,
                suffix="_enemy_ranges"
            )
    
    def _increment_prefight_candies(self, event=None):
        """Handle F3 key to increment pre-fight rare candies."""
        try:
            if self.tabbed_states.index(self.tabbed_states.select()) == self.battle_summary_tab_index:
                if self.battle_summary_frame.should_render:
                    self.battle_summary_frame.candy_summary._increment_candy(event)
        except (tk.TclError, ValueError):
            # Tab might not be selected or widget might not exist
            pass
        return "break"
    
    def _decrement_prefight_candies(self, event=None):
        """Handle F4 key to decrement pre-fight rare candies."""
        try:
            if self.tabbed_states.index(self.tabbed_states.select()) == self.battle_summary_tab_index:
                if self.battle_summary_frame.should_render:
                    self.battle_summary_frame.candy_summary._decrement_candy(event)
        except (tk.TclError, ValueError):
            # Tab might not be selected or widget might not exist
            pass
        return "break"
    
    def _take_scaled_screenshot(self, bbox_getter, suffix=""):
        """Take a screenshot with UI scaled up by 1.5x for better quality."""
        root = self.winfo_toplevel()
        
        # Get current scaling factor
        current_scaling = float(root.tk.call('tk', 'scaling'))
        
        try:
            # Scale up by 1.5x
            new_scaling = current_scaling * 1.5
            root.tk.call('tk', 'scaling', new_scaling)
            
            # Update UI to reflect scaling changes
            self.update_idletasks()
            self.battle_summary_frame.update_idletasks()
            root.update_idletasks()
            
            # Small delay to ensure UI has updated
            time.sleep(0.1)
            
            # Get bounding box and take screenshot
            bbox = bbox_getter()
            self._battle_summary_controller.take_screenshot(bbox, suffix)
        finally:
            # Restore original scaling
            root.tk.call('tk', 'scaling', current_scaling)
            self.update_idletasks()
            root.update_idletasks()
