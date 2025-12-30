import os
import threading
import logging

import tkinter as tk
from tkinter import ttk, font, messagebox

from controllers.main_controller import MainController
from gui import custom_components, quick_add_components
from gui.event_details import EventDetails
from gui.pkmn_components.route_list import RouteList
from gui.route_summary_window import RouteSummaryWindow
from gui.popups.color_config import ConfigWindow
from gui.popups.custom_dvs_popup import CustomDvsWindow
from gui.popups.data_dir_config_popup import DataDirConfigWindow
from gui.popups.delete_confirmation_popup import DeleteConfirmation
from gui.popups.load_route_popup import LoadRouteWindow
from gui.popups.new_folder_popup import NewFolderWindow
from gui.popups.new_route_popup import NewRouteWindow
from gui.popups.transfer_event_popup import TransferEventWindow
from gui.popups.custom_gen_popup import CustomGenWindow
from gui.recorder_status import RecorderStatus
from gui.route_search_component import RouteSearch
from gui.setup_summary_window import SetupSummaryWindow
from gui.landing_page import LandingPage
from gui.new_route_page import NewRoutePage
from route_recording.recorder import RecorderController
from utils.constants import const
from utils.config_manager import config
from utils import io_utils, tk_utils
from routing.route_events import EventFolder

logger = logging.getLogger(__name__)
flag_to_auto_update = False


class MainWindow(tk.Tk):
    def __init__(self, controller:MainController):
        super().__init__()
        self._controller = controller
        self._recorder_controller = RecorderController(self._controller)

        geometry = config.get_window_geometry()
        if not geometry:
            geometry = "2000x1200"
        self.geometry(geometry)
        
        # Restore window state (maximized/normal)
        window_state = config.get_window_state()
        if window_state == "zoomed":
            self.state("zoomed")
        elif window_state == "iconic":
            self.state("iconic")
        
        self.title("Pokemon RBY XP Router")

        self.call("source", os.path.join(const.ASSETS_PATH, "azure.tcl"))
        self.call("set_theme", "dark")

        self.load_custom_font()

        # menu bar
        self.top_menu_bar = tk.Menu(self)
        self.config(menu=self.top_menu_bar)

        self.file_menu = tk.Menu(self.top_menu_bar, tearoff=0)
        self.file_menu.add_command(label="Customize DVs", accelerator="Ctrl+X", command=self.open_customize_dvs_window)
        self.file_menu.add_command(label="New Route", accelerator="Ctrl+N", command=self.open_new_route_window)
        self.file_menu.add_command(label="Load Route", accelerator="Ctrl+L", command=self.open_load_route_window)
        self.file_menu.add_command(label="Save Route", accelerator="Ctrl+S", command=self.save_route)
        self.file_menu.add_command(label="Close Route", accelerator="Ctrl+Shift+C", command=self.close_route)
        self.file_menu.add_separator()
        self.auto_load_menu_var = tk.BooleanVar(value=config.get_auto_load_most_recent_route())
        self.file_menu.add_checkbutton(
            label="Automatically Load Most Recent Route on Startup",
            accelerator="F2",
            command=self.toggle_auto_load_most_recent_route,
            variable=self.auto_load_menu_var
        )
        self.file_menu.add_command(label="Export Notes", accelerator="Ctrl+Shift+W", command=self.export_notes)
        self.file_menu.add_separator()
        self.file_menu.add_command(label="Screenshot Event List", accelerator="F5", command=self.screenshot_event_list)
        self.file_menu.add_command(label="Screenshot Battle Summary", accelerator="F6", command=self.screenshot_battle_summary)
        self.file_menu.add_command(label="Screenshot Player Ranges:", accelerator="F7", command=self.export_player_ranges)
        self.file_menu.add_command(label="Screenshot Enemy Ranges", accelerator="F8", command=self.export_enemy_ranges)
        self.file_menu.add_separator()
        self.file_menu.add_command(label="Config Font", accelerator="Ctrl+Shift+D", command=self.open_config_window)
        self.file_menu.add_command(label="Custom Gens", accelerator="Ctrl+Shift+E", command=self.open_custom_gens_window)
        self.file_menu.add_command(label="App Config", accelerator="Ctrl+Shift+Z", command=self.open_app_config_window)
        self.file_menu.add_command(label="Open Data Folder", accelerator="Ctrl+Shift+O", command=self.open_data_location)

        self.event_menu = tk.Menu(self.top_menu_bar, tearoff=0)
        self.event_menu.add_command(label="Move Event Up", accelerator="Ctrl+E", command=self.move_group_up)
        self.event_menu.add_command(label="Move Event Down", accelerator="Ctrl+D", command=self.move_group_down)
        self.event_menu.add_command(label="Enable/Disable", accelerator="Ctrl+C", command=self.toggle_enable_disable)
        self.event_menu.add_command(label="Toggle Highlight", accelerator="Ctrl+V", command=self.toggle_event_highlight)
        self.event_menu.add_command(label="Transfer Event", accelerator="Ctrl+R", command=self.open_transfer_event_window)
        self.event_menu.add_command(label="Delete Event", accelerator="Ctrl+B", command=self.delete_group)

        self.folder_menu = tk.Menu(self.top_menu_bar, tearoff=0)
        self.folder_menu.add_command(label="New Folder", command=self.open_new_folder_window)
        self.folder_menu.add_command(label="Rename Cur Folder", command=self.rename_folder)

        self.recording_menu = tk.Menu(self.top_menu_bar, tearoff=0)
        self.recording_menu.add_command(label="Enable/Disable Recording", accelerator="F1", command=self.record_button_clicked)
        
        # Battle Summary menu
        self.battle_summary_menu = tk.Menu(self.top_menu_bar, tearoff=0, postcommand=self._update_battle_summary_menu_state)
        # Battle Summary Notes submenu
        self.battle_summary_notes_menu = tk.Menu(self.battle_summary_menu, tearoff=0)
        self.battle_summary_menu.add_cascade(label="Battle Summary Notes", menu=self.battle_summary_notes_menu)
        self._update_battle_summary_notes_menu()
        # Show Move Highlights toggle
        self.battle_summary_menu.add_separator()
        self.show_move_highlights_var = tk.BooleanVar(value=config.get_show_move_highlights())
        self.battle_summary_menu.add_checkbutton(
            label="Show Move Highlights",
            variable=self.show_move_highlights_var,
            command=self._toggle_move_highlights
        )
        # Player Highlight Strategy submenu
        self.battle_summary_menu.add_separator()
        self.player_highlight_strategy_menu = tk.Menu(self.battle_summary_menu, tearoff=0)
        self.player_highlight_strategy_var = tk.StringVar(value=config.get_player_highlight_strategy())
        for strat in const.ALL_HIGHLIGHT_STRATS:
            self.player_highlight_strategy_menu.add_radiobutton(
                label=strat,
                variable=self.player_highlight_strategy_var,
                value=strat,
                command=self._update_player_highlight_strategy
            )
        self.battle_summary_menu.add_cascade(label="Player Highlight Strategy", menu=self.player_highlight_strategy_menu)
        # Enemy Highlight Strategy submenu
        self.enemy_highlight_strategy_menu = tk.Menu(self.battle_summary_menu, tearoff=0)
        self.enemy_highlight_strategy_var = tk.StringVar(value=config.get_enemy_highlight_strategy())
        for strat in const.ALL_HIGHLIGHT_STRATS:
            self.enemy_highlight_strategy_menu.add_radiobutton(
                label=strat,
                variable=self.enemy_highlight_strategy_var,
                value=strat,
                command=self._update_enemy_highlight_strategy
            )
        self.battle_summary_menu.add_cascade(label="Enemy Highlight Strategy", menu=self.enemy_highlight_strategy_menu)

        self.top_menu_bar.add_cascade(label="File", menu=self.file_menu)
        self.top_menu_bar.add_cascade(label="Events", menu=self.event_menu)
        self.top_menu_bar.add_cascade(label="Folders", menu=self.folder_menu)
        self.top_menu_bar.add_cascade(label="Recording", menu=self.recording_menu)
        self.top_menu_bar.add_cascade(label="Battle Summary", menu=self.battle_summary_menu)

        # main container for everything to sit in... might be unnecessary?
        self.primary_window = ttk.Frame(self)
        self.primary_window.pack(fill=tk.BOTH, expand=True)

        # Landing page (shown when no route is loaded)
        self.landing_page = LandingPage(
            self.primary_window,
            self._controller,
            on_create_route=self.open_new_route_window,
            on_load_route=self._load_route_from_landing_page,
            on_auto_load_toggle=self._on_landing_page_auto_load_toggle
        )
        # Don't pack initially - will be shown only if needed (after checking auto-load setting)

        # Track if route was loaded before opening new route page
        self._route_loaded_before_new_route = False
        
        # New route page (shown when creating a new route)
        self.new_route_page = NewRoutePage(
            self.primary_window,
            self._controller,
            on_cancel=self._cancel_new_route_page,
            on_create=self._create_route_from_page
        )
        # Don't pack initially - will be shown when creating new route

        # create container for split columns (hidden initially)
        self.info_panel = ttk.Frame(self.primary_window)
        # Don't pack initially - will be shown when route is loaded

        # left panel for controls and event list
        self.left_info_panel = ttk.Frame(self.info_panel)
        self.left_info_panel.grid(row=0, column=0, sticky="nsew")

        self.top_row = ttk.Frame(self.left_info_panel)
        self.top_row.pack(fill=tk.X)
        self.top_row.pack_propagate(False)

        self.record_button = custom_components.SimpleButton(self.top_row, text="Enable\nRecording", command=self.record_button_clicked)
        self.record_button.grid(row=0, column=0, sticky=tk.W, padx=3, pady=3)
        self.record_button.disable()

        self.run_status_frame = ttk.Frame(self.top_row, style="Success.TFrame")
        self.run_status_frame.grid(row=0, column=1, sticky=tk.W)

        self.run_status_label = ttk.Label(self.run_status_frame, text="Run Status: Valid", style="Success.TLabel")
        self.run_status_label.pack(padx=10, pady=10)

        # NOTE: Intentionally leaving this as a tk.Label so that we can just control the color in code
        self.route_version = tk.Label(self.top_row, text="RBY Version", anchor=tk.W, padx=10, pady=10, fg="black", bg="white")
        self.route_version.grid(row=0, column=2)

        self.route_name_label = ttk.Label(self.top_row, text="Route Name: ")
        self.route_name_label.grid(row=0, column=3)

        self._loading_route_name = False
        self.route_name = custom_components.SimpleEntry(self.top_row, callback=self._user_set_route_name)
        self.route_name.grid(row=0, column=4)
        self.route_name.config(width=30)

        self.message_label = custom_components.AutoClearingLabel(self.top_row, width=100, justify=tk.LEFT, anchor=tk.W)
        self.message_label.grid(row=0, column=5, sticky=tk.E)

        self.top_left_controls = ttk.Frame(self.left_info_panel)
        self.top_left_controls.pack(fill=tk.X, anchor=tk.CENTER)
        # Configure columns: 3 columns for Trainers/Items/Wild Pkmn
        self.top_left_controls.grid_columnconfigure(0, weight=1, uniform="quick_add")
        self.top_left_controls.grid_columnconfigure(1, weight=1, uniform="quick_add")
        self.top_left_controls.grid_columnconfigure(2, weight=1, uniform="quick_add")

        self.recorder_status = RecorderStatus(self._controller, self._recorder_controller, self.top_left_controls)

        self.trainer_add = quick_add_components.QuickTrainerAdd(
            self._controller,
            self.top_left_controls
        )

        self.item_add = quick_add_components.QuickItemAdd(
            self._controller,
            self.top_left_controls,
        )

        self.wild_pkmn_add = quick_add_components.QuickWildPkmn(
            self._controller,
            self.top_left_controls,
        )

        self.misc_add = quick_add_components.QuickMiscEvents(
            self._controller,
            self.top_left_controls,
        )

        # Container for filters and control buttons (left side of second row)
        self.filters_and_controls_frame = ttk.Frame(self.top_left_controls)
        
        # Route search (filters) - inside the filters_and_controls_frame
        self.route_search = RouteSearch(self._controller, self.filters_and_controls_frame)
        
        # Group controls (buttons) - inside the filters_and_controls_frame, above filters
        self.group_controls = ttk.Frame(self.filters_and_controls_frame)

        button_spacing_cols = []
        button_col_idx = 0

        # Compress buttons to fit within filter area - reduce width and padding
        self.show_summary_btn = custom_components.SimpleButton(self.group_controls, text='Run Summary', command=self.open_summary_window, width=12)
        self.show_summary_btn.grid(row=0, column=button_col_idx, padx=2, pady=1, sticky=tk.NSEW)
        self.show_setup_summary_btn = custom_components.SimpleButton(self.group_controls, text='Setup Summary', command=self.open_setup_summary_window, width=12)
        self.show_setup_summary_btn.grid(row=1, column=button_col_idx, padx=2, pady=1, sticky=tk.NSEW)
        button_col_idx += 1

        button_spacing_cols.append(button_col_idx)
        button_col_idx += 1
        
        self.move_group_up_button = custom_components.SimpleButton(self.group_controls, text='Move Event Up', command=self.move_group_up, width=12)
        self.move_group_up_button.grid(row=0, column=button_col_idx, padx=2, pady=1, sticky=tk.NSEW)
        self.move_group_down_button = custom_components.SimpleButton(self.group_controls, text='Move Event Down', command=self.move_group_down, width=12)
        self.move_group_down_button.grid(row=1, column=button_col_idx, padx=2, pady=1, sticky=tk.NSEW)
        button_col_idx += 1

        self.highlight_toggle_button = custom_components.SimpleButton(self.group_controls, text='Enable/Disable', command=self.toggle_enable_disable, width=12)
        self.highlight_toggle_button.grid(row=0, column=button_col_idx, padx=2, pady=1, sticky=tk.NSEW)
        self.highlight_toggle_button2 = custom_components.SimpleButton(self.group_controls, text='Toggle Highlight', command=self.toggle_event_highlight, width=12)
        self.highlight_toggle_button2.grid(row=1, column=button_col_idx, padx=2, pady=1, sticky=tk.NSEW)
        button_col_idx += 1

        button_spacing_cols.append(button_col_idx)
        button_col_idx += 1

        self.transfer_event_button = custom_components.SimpleButton(self.group_controls, text='Transfer Event', command=self.open_transfer_event_window, width=12)
        self.transfer_event_button.grid(row=0, column=button_col_idx, padx=2, pady=1, sticky=tk.NSEW)
        self.delete_event_button = custom_components.SimpleButton(self.group_controls, text='Delete Event', command=self.delete_group, width=12)
        self.delete_event_button.grid(row=1, column=button_col_idx, padx=2, pady=1, sticky=tk.NSEW)
        button_col_idx += 1

        button_spacing_cols.append(button_col_idx)
        button_col_idx += 1

        # New Folder and Rename Folder buttons - compressed to fit within filter toggle space
        self.new_folder_button = custom_components.SimpleButton(self.group_controls, text='New Folder', command=self.open_new_folder_window, width=12)
        self.new_folder_button.grid(row=0, column=button_col_idx, padx=2, pady=1, sticky=tk.NSEW)
        self.rename_folder_button = custom_components.SimpleButton(self.group_controls, text='Rename Folder', command=self.rename_folder, width=12)
        self.rename_folder_button.grid(row=1, column=button_col_idx, padx=2, pady=1, sticky=tk.NSEW)
        button_col_idx += 1

        button_spacing_cols.append(button_col_idx)
        button_col_idx += 1

        for cur_spacer_idx in button_spacing_cols:
            self.group_controls.columnconfigure(cur_spacer_idx, weight=1)

        # Pack group_controls first (above filters), then route_search (filters)
        self.group_controls.pack(fill=tk.X, anchor=tk.CENTER)
        self.route_search.pack(fill=tk.X, anchor=tk.CENTER)

        # Initially hide recorder_status, show quick add components
        self.recorder_status.grid(row=0, column=0, sticky=tk.NSEW, padx=5, pady=5, columnspan=3)
        self.recorder_status.grid_remove()  # Hide initially
        
        # Row 1: Trainers, Items, Wild Pkmn
        self.trainer_add.grid(row=1, column=0, sticky=tk.NSEW, padx=5, pady=5)
        self.item_add.grid(row=1, column=1, sticky=tk.NSEW, padx=5, pady=5)
        self.wild_pkmn_add.grid(row=1, column=2, sticky=tk.NSEW, padx=5, pady=5)
        
        # Row 2: Filters/Controls on left (columns 0-1), Misc on right (column 2, aligned with Wild Pkmn)
        self.filters_and_controls_frame.grid(row=2, column=0, columnspan=2, sticky=tk.NSEW, padx=5, pady=5)
        self.misc_add.grid(row=2, column=2, sticky=tk.NSEW, padx=5, pady=5)

        self.frame_for_event_list = ttk.Frame(self.left_info_panel)
        self.frame_for_event_list.pack(fill=tk.BOTH, anchor=tk.CENTER, expand=True)

        self.event_list = RouteList(self._controller, self.frame_for_event_list)
        self.scroll_bar = ttk.Scrollbar(self.frame_for_event_list, orient="vertical", command=self.event_list.yview, style="Wide.Vertical.TScrollbar")

        # intentionally pack event list after scrollbar, so they're ordered correctly
        self.scroll_bar.pack(side="right", fill=tk.BOTH)
        self.event_list.pack(padx=10, pady=10, fill=tk.BOTH, expand=True, side="right")
        self.event_list.configure(yscrollcommand=self.scroll_bar.set)

        # right panel for event details
        self.event_details = EventDetails(self._controller, self.info_panel)
        self.event_details.grid(row=0, column=1, sticky=tk.NSEW)
        self.event_details.pack_propagate(0)

        self.info_panel.grid_rowconfigure(0, weight=1)
        # these uniform values don't have to be a specific value, they just have to match
        self.info_panel.grid_columnconfigure(0, weight=1, uniform="test")

        # main route actions
        self.bind('<Control-x>', self.open_customize_dvs_window)
        self.bind('<Control-n>', self.open_new_route_window)
        self.bind('<Control-a>', self.open_load_route_window)
        self.bind('<Control-s>', self.save_route)
        self.bind('<Control-C>', self.close_route)  # Ctrl+Shift+C
        self.bind('<Control-W>', self.export_notes)
        # event actions
        self.bind('<Control-d>', self.move_group_down)
        self.bind('<Control-c>', self.toggle_enable_disable)
        self.bind('<Control-v>', self.toggle_event_highlight)
        self.bind('<Control-r>', self.open_transfer_event_window)
        self.bind('<Control-b>', self.delete_group)
        self.bind('<Delete>', self.delete_group)
        # recording actions
        self.bind_all('<KeyPress-F1>', self.record_button_clicked)
        self.bind_all('<F1>', self.record_button_clicked)
        # auto-load toggle - bind to window only, not all widgets, to avoid interfering with text entry
        self.bind('<KeyPress-F2>', self.toggle_auto_load_most_recent_route)
        self.bind('<F2>', self.toggle_auto_load_most_recent_route)
        # navigation
        self.bind('<Home>', self.scroll_to_top)
        self.bind('<End>', self.scroll_to_bottom)
        # folder actions (keyboard shortcuts removed - now used for export)
        # config integrations
        self.bind('<Control-D>', self.open_config_window)
        self.bind('<Control-Z>', self.open_app_config_window)
        self.bind('<Control-R>', self.open_summary_window)
        self.bind('<Control-T>', self.open_setup_summary_window)
        self.bind('<Control-A>', self.open_data_location)
        # Screenshot shortcuts
        self.bind('<F5>', self.screenshot_event_list)
        self.bind('<F6>', self.screenshot_battle_summary)
        self.bind('<F7>', self.export_player_ranges)
        self.bind('<F8>', self.export_enemy_ranges)
        # Pre-fight rare candy shortcuts
        self.bind('<F4>', self.increment_prefight_candies)
        self.bind('<F3>', self.decrement_prefight_candies)
        # Battle Summary shortcuts
        self.bind('<F9>', self.toggle_player_highlight_strategy)
        self.bind('<F10>', self.toggle_enemy_highlight_strategy)
        # Event actions
        self.bind('<Control-e>', self.move_group_up)
        # detail update function
        self.protocol("WM_DELETE_WINDOW", self._on_close)
        self.bind("<<TreeviewSelect>>", self._report_new_selection)
        self.bind("<Configure>", self._on_configure)
        self.bind("<Map>", self._on_window_mapped_focus)
        self.bind(const.ROUTE_LIST_REFRESH_EVENT, self.update_run_status)
        self.bind(const.FORCE_QUIT_EVENT, self.cancel_and_quit)

        self.bind(self._controller.register_event_preview(self), self.trainer_preview)
        self.bind(self._controller.register_event_selection(self), self._handle_new_selection)
        self.bind(self._controller.register_version_change(self), self.update_run_version)
        self.bind(self._controller.register_exception_callback(self), self._on_exception)
        self.bind(self._controller.register_name_change(self), self._on_name_change)
        self.bind(self._controller.register_record_mode_change(self), self._on_record_mode_changed)
        self.bind(self._controller.register_message_callback(self), self._on_route_message)
        # TODO: should this be moved directly to the event list class?
        self.bind(self._controller.register_route_change(self), self._on_route_change)

        self.event_list.refresh()
        self.new_event_window = None
        self.summary_window = None
        self.setup_summary_window = None
        
        # Check if auto-load is enabled - if so, skip landing page entirely
        self._auto_load_checked = False
        if config.get_auto_load_most_recent_route():
            # Find the most recent route immediately (synchronously)
            most_recent_route = self._find_most_recent_route()
            if most_recent_route:
                # Skip landing page entirely, go straight to loading route
                # Load route as soon as window is ready (minimal delay)
                self.after(10, lambda: self._load_route_immediately(most_recent_route))
            else:
                # No routes available - show landing page
                self._show_landing_page()
        else:
            # Show landing page normally
            self._show_landing_page()

    def run(self):
        # Load custom versions after background generations finish loading
        # Use after_idle to ensure it happens after the window is shown
        def _load_custom_versions_after_background():
            # Wait a bit for background loading to complete, then load custom gens
            # Custom gen loading will skip any whose base gens aren't ready yet
            import time
            from utils import setup
            # Give background thread a moment to start
            time.sleep(0.1)
            # Try to wait for background loading (with timeout)
            setup.wait_for_background_loading_complete(timeout=5)
            # Now load custom versions (will skip any whose base gens aren't ready)
            try:
                self._controller.load_all_custom_versions()
            except Exception as e:
                # Log but don't crash - custom gens will be skipped if base gens aren't ready
                logger.warning(f"Some custom gens couldn't be loaded: {e}")
        
        self.after_idle(_load_custom_versions_after_background)
        # Ensure window has focus when event loop starts
        self.update_idletasks()
        self.after_idle(self._ensure_window_focus)
        self.mainloop()

    def _on_configure(self, e):
        if e.widget == self:
            # Removed sleep - was causing lag on window resize
            pass
    
    def _on_close(self, *args, **kwargs):
        # Save window geometry and state
        config.set_window_geometry(self.geometry())
        # Get current window state (zoomed = maximized, normal = not maximized)
        current_state = self.state()
        if current_state == "zoomed":
            config.set_window_state("zoomed")
        elif current_state == "iconic":
            config.set_window_state("iconic")
        else:
            config.set_window_state("normal")

        if self._controller.has_unsaved_changes():
            if not messagebox.askyesno("Quit?", "Route has unsaved changes. Quit without saving?"):
                return

        self.destroy()
    
    def _on_exception(self, *args, **kwargs):
        exception_message = self._controller.get_next_exception_info()
        while exception_message is not None:
            threading.Thread(
                target=messagebox.showerror,
                args=("Error!", exception_message),
                daemon=True
            ).start()
            exception_message = self._controller.get_next_exception_info()
    
    def _on_name_change(self, *args, **kwargs):
        if self.route_name.get() == self._controller.get_current_route_name():
            return
        self._loading_route_name = True
        self.route_name.delete(0, tk.END)
        self.route_name.insert(0, self._controller.get_current_route_name())
        self._loading_route_name = False

    def _user_set_route_name(self, *args, **kwargs):
        if not self._loading_route_name:
            self._controller.set_current_route_name(self.route_name.get())
    
    def _on_route_message(self, *args, **kwargs):
        self.message_label.set_message(self._controller.get_next_message_info())
    
    def _on_route_change(self, *args, **kwargs):
        """Handle route change event - refresh event list and show route controls."""
        self.event_list.refresh()
        # Show route controls if we have a route loaded (has init_route_state set)
        # Check if route has been initialized (has a pokemon version)
        if self._controller.get_version() is not None:
            self._show_route_controls()
        else:
            self._show_landing_page()

    def save_route(self, *args, **kwargs):
        route_name = self.route_name.get()
        self._controller.save_route(route_name)
    
    def export_notes(self, *args, **kwargs):
        self._controller.export_notes(self.route_name.get())
    
    def screenshot_event_list(self, *args, **kwargs):
        # Save current selection
        current_selection = self.event_list.get_all_selected_event_ids()
        
        # Temporarily clear selection to remove highlight
        self.event_list.selection_set([])
        self.update_idletasks()
        
        # Take screenshot of the event list only (excluding scrollbar)
        bbox = tk_utils.get_bounding_box(self.event_list)
        self._controller.take_screenshot("event_list", bbox)
        
        # Restore selection
        self.event_list.set_all_selected_event_ids(current_selection)
    
    def screenshot_battle_summary(self, *args, **kwargs):
        self.event_details.take_battle_summary_screenshot()
    
    def increment_prefight_candies(self, *args, **kwargs):
        self.event_details._increment_prefight_candies()
    
    def decrement_prefight_candies(self, *args, **kwargs):
        self.event_details._decrement_prefight_candies()
    
    def export_player_ranges(self, *args, **kwargs):
        self.event_details.take_player_ranges_screenshot()
    
    def export_enemy_ranges(self, *args, **kwargs):
        self.event_details.take_enemy_ranges_screenshot()

    def load_custom_font(self):
        if config.get_custom_font_name() in font.families():
            defaultFont = font.nametofont("TkDefaultFont")
            defaultFont.configure(family=config.get_custom_font_name())
        else:
            defaultFont = font.nametofont("TkDefaultFont")
            defaultFont.configure(family=config.DEFAULT_FONT_NAME)

    def update_run_status(self, *args, **kwargs):
        if self._controller.has_errors():
            self.run_status_frame.config(style="Warning.TFrame")
            self.run_status_label.config(text="Run Status: Invalid", style="Warning.TLabel")
        else:
            self.run_status_frame.config(style="Success.TFrame")
            self.run_status_label.config(text="Run Status: Valid", style="Success.TLabel")
    
    def update_run_version(self, *args, **kwargs):
        self.route_version.config(
            text=f"{self._controller.get_version()} Version",
            background=const.VERSION_COLORS.get(self._controller.get_version(), "white")
        )
        self.record_button.enable()
    
    def _update_battle_summary_notes_menu(self):
        """Update the battle summary notes menu with current selection."""
        # Clear existing items
        self.battle_summary_notes_menu.delete(0, tk.END)
        
        current_mode = config.get_notes_visibility_mode()
        options = [
            ("Show notes in battle summary when space allows", "when_space_allows"),
            ("Show notes in battle summary at all times", "always"),
            ("Never show notes in battle summary", "never")
        ]
        
        for label, mode in options:
            is_selected = (current_mode == mode)
            self.battle_summary_notes_menu.add_command(
                label=("✓ " if is_selected else "  ") + label,
                command=lambda m=mode: self._set_battle_summary_notes_mode(m)
            )
    
    def _set_battle_summary_notes_mode(self, mode):
        """Set the battle summary notes visibility mode."""
        config.set_notes_visibility_mode(mode)
        self._update_battle_summary_notes_menu()
        # Trigger update in event details if it exists
        if hasattr(self, 'event_details'):
            self.event_details._update_notes_visibility_in_battle_summary()
        # Update dropdown in notes editor if it exists
        if hasattr(self, 'event_details') and hasattr(self.event_details, 'trainer_notes'):
            try:
                self.event_details.trainer_notes._load_visibility_setting()
            except Exception:
                pass
    
    def _toggle_move_highlights(self):
        """Toggle the Show Move Highlights setting."""
        config.set_show_move_highlights(self.show_move_highlights_var.get())
        # Refresh battle summary if it exists
        if hasattr(self, 'event_details') and hasattr(self.event_details, '_battle_summary_controller'):
            self.event_details._battle_summary_controller._on_refresh()
    
    def _update_move_highlights_menu_state(self):
        """Update the menu checkbox state to match the config state."""
        self.show_move_highlights_var.set(config.get_show_move_highlights())
    
    def _update_battle_summary_menu_state(self):
        """Update all Battle Summary menu states to match config (called when menu opens)."""
        self.show_move_highlights_var.set(config.get_show_move_highlights())
        self.player_highlight_strategy_var.set(config.get_player_highlight_strategy())
        self.enemy_highlight_strategy_var.set(config.get_enemy_highlight_strategy())
    
    def _update_player_highlight_strategy(self):
        """Update Player Highlight Strategy setting from menu selection."""
        config.set_player_highlight_strategy(self.player_highlight_strategy_var.get())
        # Refresh battle summary if it exists - need full refresh to recalculate best moves
        if hasattr(self, 'event_details') and hasattr(self.event_details, '_battle_summary_controller'):
            self.event_details._battle_summary_controller._full_refresh()
    
    def _update_enemy_highlight_strategy(self):
        """Update Enemy Highlight Strategy setting from menu selection."""
        config.set_enemy_highlight_strategy(self.enemy_highlight_strategy_var.get())
        # Refresh battle summary if it exists - need full refresh to recalculate best moves
        if hasattr(self, 'event_details') and hasattr(self.event_details, '_battle_summary_controller'):
            self.event_details._battle_summary_controller._full_refresh()
    
    def toggle_player_highlight_strategy(self, event=None):
        """Toggle Player Highlight Strategy between Guaranteed Kill and Don't Highlight (F9 shortcut)."""
        current_strat = config.get_player_highlight_strategy()
        if current_strat == const.HIGHLIGHT_GUARANTEED_KILL:
            new_strat = const.HIGHLIGHT_NONE
        else:
            new_strat = const.HIGHLIGHT_GUARANTEED_KILL
        
        config.set_player_highlight_strategy(new_strat)
        self.player_highlight_strategy_var.set(new_strat)
        # Refresh battle summary if it exists - need full refresh to recalculate best moves
        if hasattr(self, 'event_details') and hasattr(self.event_details, '_battle_summary_controller'):
            self.event_details._battle_summary_controller._full_refresh()
        return "break"  # Prevent default F9 behavior
    
    def toggle_enemy_highlight_strategy(self, event=None):
        """Toggle Enemy Highlight Strategy between Guaranteed Kill and Don't Highlight (F10 shortcut)."""
        current_strat = config.get_enemy_highlight_strategy()
        if current_strat == const.HIGHLIGHT_GUARANTEED_KILL:
            new_strat = const.HIGHLIGHT_NONE
        else:
            new_strat = const.HIGHLIGHT_GUARANTEED_KILL
        
        config.set_enemy_highlight_strategy(new_strat)
        self.enemy_highlight_strategy_var.set(new_strat)
        # Refresh battle summary if it exists - need full refresh to recalculate best moves
        if hasattr(self, 'event_details') and hasattr(self.event_details, '_battle_summary_controller'):
            self.event_details._battle_summary_controller._full_refresh()
        return "break"  # Prevent default F10 behavior
    
    def record_button_clicked(self, event=None):
        """Toggle recording mode - can be called from button, menu, or F1 key."""
        self._controller.set_record_mode(not self._controller.is_record_mode_active())
        return "break"  # Prevent default F1 behavior (Windows help)
    
    def _on_record_mode_changed(self, *args, **kwargs):
        if self._controller.is_record_mode_active():
            self.record_button.configure(text="Cancel\nRecording")
            # Show recorder status, hide quick add components and filters/controls
            self.recorder_status.grid()
            self.trainer_add.grid_remove()
            self.item_add.grid_remove()
            self.wild_pkmn_add.grid_remove()
            self.misc_add.grid_remove()
            self.filters_and_controls_frame.grid_remove()
        else:
            self.record_button.configure(text="Enable\nRecording")
            # Hide recorder status, show quick add components and filters/controls
            self.recorder_status.grid_remove()
            self.trainer_add.grid()
            self.item_add.grid()
            self.wild_pkmn_add.grid()
            self.misc_add.grid()
            self.filters_and_controls_frame.grid()
        self._handle_new_selection()

    def trainer_preview(self, *args, **kwargs):
        if self._controller.get_preview_event() is None:
            return

        all_event_ids = self.event_list.get_all_selected_event_ids()
        if len(all_event_ids) > 1:
            return

        init_state = self._controller.get_state_after(
            previous_event_id=None if len(all_event_ids) == 0 else all_event_ids[0]
        )

        # create a fake event_def just so we can show the trainer that the user is looking at
        # TODO: just using the init_state as the post_event state as well. Ideally would like to use None for an empty state, but that's not currently supported
        self.event_details.show_event_details(
            self._controller.get_preview_event(),
            init_state,
            init_state,
            allow_updates=False
        )
    
    def _report_new_selection(self, *args, **kwargs):
        # this is different from _handle_new_selection as we are reporting the new selection
        # from the users action (via a tk event) to the controller
        # _handle_new_selection will respond to the controller's event about the selection changing

        # guard against unnecessary updates so that the treeview can be updated without creating an infinite loop
        cur_treeview_selected = self.event_list.get_all_selected_event_ids()
        if self._controller.get_all_selected_ids() != cur_treeview_selected:
            self._controller.select_new_events(cur_treeview_selected)
    
    def _handle_new_selection(self, *args, **kwargs):
        # just re-use the variable temporarily
        all_event_ids = self._controller.get_all_selected_ids()
        if all_event_ids != self.event_list.get_all_selected_event_ids():
            self.event_list.set_all_selected_event_ids(all_event_ids)

        self.event_list.scroll_to_selected_events()

        # now assign it the value it will have for the rest of the function
        all_event_ids = self.event_list.get_all_selected_event_ids(allow_event_items=False)
        if len(all_event_ids) > 1 or len(all_event_ids) == 0:
            event_group = None
        else:
            event_group = self._controller.get_event_by_id(all_event_ids[0])
        
        disable_all = False
        if self._controller.is_record_mode_active() or self._controller.get_raw_route().init_route_state is None:
            disable_all = True
        
        if not disable_all and isinstance(event_group, EventFolder):
            self.rename_folder_button.enable()
        else:
            self.rename_folder_button.disable()
        
        if not disable_all and (event_group is not None or len(all_event_ids) > 0):
            # As long as we have any editable events selected, toggle the buttons to allow editing those events
            self.delete_event_button.enable()
            self.transfer_event_button.enable()
            self.move_group_down_button.enable()
            self.move_group_up_button.enable()
            self.highlight_toggle_button.enable()
        else:
            self.delete_event_button.disable()
            self.transfer_event_button.disable()
            self.move_group_down_button.disable()
            self.move_group_up_button.disable()
            self.highlight_toggle_button.disable()
        
        if not disable_all and (event_group is not None or len(self.event_list.get_all_selected_event_ids()) == 0):
            # as long as we have a finite place to create a new folder, enable the option
            # either only a single event is selected, or no events are selected
            # Need to check all events, not just editable ones
            self.new_folder_button.enable()
        else:
            self.new_folder_button.disable()
    
    def open_app_config_window(self, *args, **kwargs):
        DataDirConfigWindow(self, self.cancel_and_quit)
    
    def open_custom_gens_window(self, *args, **kwargs):
        CustomGenWindow(self, self._controller)
    
    def open_config_window(self, *args, **kwargs):
        ConfigWindow(self)
    
    def open_new_route_window(self, *args, **kwargs):
        """Show the new route creation page."""
        # Track if a route was loaded before opening new route page
        self._route_loaded_before_new_route = self._controller.get_version() is not None
        self._show_new_route_page()
    
    def _show_new_route_page(self):
        """Show new route page and hide other views."""
        self.landing_page.pack_forget()
        self.info_panel.pack_forget()
        self.new_route_page.pack(fill=tk.BOTH, expand=True)
        # Enable loading popup now that page is being shown to user
        self.new_route_page._suppress_loading_popup = False
    
    def _cancel_new_route_page(self):
        """Handle cancel from new route page - return to route if one was loaded, otherwise landing page."""
        if self._route_loaded_before_new_route:
            # Return to the route that was loaded before
            self._show_route_controls()
        else:
            # No route was loaded, go to landing page
            self._show_landing_page()
        self._route_loaded_before_new_route = False  # Reset flag
    
    def _create_route_from_page(self, solo_mon, base_route_path, pkmn_version, custom_dvs=None, custom_ability_idx=None, custom_nature=None):
        """Create route from the new route page."""
        self._controller.create_new_route(
            solo_mon,
            base_route_path,
            pkmn_version,
            custom_dvs=custom_dvs,
            custom_ability_idx=custom_ability_idx,
            custom_nature=custom_nature
        )
        # Route controls will be shown via route change event
    
    def close_route(self, *args, **kwargs):
        """Close the current route and return to landing page."""
        # Check if there's actually a route loaded
        if self._controller.get_version() is None:
            return
        
        # Check for unsaved changes
        if self._controller.has_unsaved_changes():
            response = messagebox.askyesnocancel(
                "Unsaved Changes",
                "Route has unsaved changes. Save before closing?"
            )
            if response is None:  # Cancel
                return
            elif response:  # Yes - save
                route_name = self.route_name.get()
                if not route_name:
                    messagebox.showwarning("No Route Name", "Please enter a route name before saving.")
                    return
                self._controller.save_route(route_name)
        
        # Clear the route by resetting the router state
        self._controller._data.init_route_state = None
        self._controller._data.pkmn_version = None
        self._controller._data._reset_events()
        self._controller._data.level_up_move_defs = {}
        self._controller._data.defeated_trainers = set()
        self._controller._route_name = ""
        self._controller._selected_ids = []
        self._controller._unsaved_changes = False
        self._controller._on_name_change()
        self._controller._on_version_change()
        self._controller._on_event_selection()
        self._controller._on_route_change()
    
    def open_load_route_window(self, *args, **kwargs):
        LoadRouteWindow(self, self._controller)
    
    def _load_route_from_landing_page(self, route_path):
        """Load a route from the landing page."""
        self._controller.load_route(route_path)
        self._show_route_controls()
        # Ensure the window has focus after switching views
        # Process pending events first, then set focus
        self.update_idletasks()
        # Use after to set focus in the next event loop iteration
        # This ensures all widget updates are complete before focusing
        self.after(50, self._ensure_window_focus)
    
    def _ensure_window_focus(self):
        """Ensure the main window has focus."""
        try:
            # Lift the window to ensure it's on top
            self.lift()
            # Use focus_force() to ensure focus is set even if another window has it
            # This is important for initial window focus
            self.focus_force()
        except tk.TclError:
            # Window might have been destroyed, ignore
            pass
    
    def _on_window_mapped_focus(self, event=None):
        """Handle window being mapped (becoming visible) - ensure it has focus."""
        # Only set focus if this is the main window (not a child widget)
        if event.widget == self:
            self.after_idle(self._ensure_window_focus)
    
    def _show_landing_page(self):
        """Show landing page and hide route controls."""
        self.new_route_page.pack_forget()
        self.info_panel.pack_forget()
        self.landing_page.pack(fill=tk.BOTH, expand=True)
        # Refresh landing page route list
        self.landing_page.refresh_routes()
        # Ensure focus is set on the main window
        self.update_idletasks()
        self.after(10, self._ensure_window_focus)
    
    def _show_route_controls(self):
        """Show route controls and hide landing page."""
        self.landing_page.pack_forget()
        self.new_route_page.pack_forget()
        self.info_panel.pack(expand=True, fill=tk.BOTH)
        # Ensure focus is set on the main window after switching views
        self.update_idletasks()
        self.after(10, self._ensure_window_focus)
    
    def _on_window_mapped(self, event=None):
        """Handle window being mapped (becoming visible) - check for auto-load."""
        # Only check once
        if self._auto_load_checked:
            return
        self._check_if_window_ready_for_auto_load()
    
    def _find_most_recent_route(self):
        """Find the most recent route synchronously. Returns route path or None."""
        # Get all routes and find the most recent one
        all_routes = io_utils.get_existing_route_names(load_backups=False)
        if not all_routes:
            return None
        
        # Get the most recent route by modification time
        most_recent_route = None
        most_recent_mtime = 0
        
        for route_name in all_routes:
            route_path = io_utils.get_existing_route_path(route_name)
            try:
                mtime = os.path.getmtime(route_path)
                if mtime > most_recent_mtime:
                    most_recent_mtime = mtime
                    most_recent_route = route_path
            except Exception:
                continue
        
        return most_recent_route
    
    def _load_route_immediately(self, route_path):
        """Load route immediately without showing landing page."""
        if self._auto_load_checked:
            return
        self._auto_load_checked = True
        
        # Load the route directly
        self._controller.load_route(route_path)
        self._show_route_controls()
        # Ensure the window has focus after loading
        self.update_idletasks()
        self.after(50, self._ensure_window_focus)

    def open_customize_dvs_window(self, *args, **kwargs):
        if self._controller.is_empty():
            return
        CustomDvsWindow(self, self._controller, self._controller.get_dvs(), self._controller.get_ability_idx(), self._controller.get_nature())

    def open_data_location(self, *args, **kwargs):
        io_utils.open_explorer(config.get_user_data_dir())

    def open_summary_window(self, *args, **kwargs):
        if self.summary_window is None or not tk.Toplevel.winfo_exists(self.summary_window):
            self.summary_window = RouteSummaryWindow(self, self._controller)
        self.summary_window.focus()

    def open_setup_summary_window(self, *args, **kwargs):
        if self.setup_summary_window is None or not tk.Toplevel.winfo_exists(self.setup_summary_window):
            self.setup_summary_window = SetupSummaryWindow(self, self._controller)
        self.setup_summary_window.focus()

    def move_group_up(self, event=None):
        self._controller.move_groups_up(self.event_list.get_all_selected_event_ids(allow_event_items=False))

    def move_group_down(self, event=None):
        # NOTE: have to reverse the list since we move items one at a time
        self._controller.move_groups_down(reversed(self.event_list.get_all_selected_event_ids(allow_event_items=False)))

    def toggle_event_highlight(self, event=None):
        self._controller.toggle_event_highlight(self.event_list.get_all_selected_event_ids(allow_event_items=False))

    def toggle_enable_disable(self, event=None):
        self.event_list.trigger_checkbox()

    def delete_group(self, event=None):
        all_event_ids = self.event_list.get_all_selected_event_ids(allow_event_items=False)
        if len(all_event_ids) == 0:
            return
        
        do_prompt = False
        if len(all_event_ids) == 1:
            cur_event_id = all_event_ids[0]
            event_obj = self._controller.get_event_by_id(cur_event_id)
            if isinstance(event_obj, EventFolder) and len(event_obj.children) > 0:
                do_prompt = True
        else:
            do_prompt = True
        
        if do_prompt:
            DeleteConfirmation(self, self._controller, all_event_ids)
        else:
            # only don't prompt when deleting a single event (or empty folder)
            self._controller.delete_events([all_event_ids[0]])
            self.event_list.refresh()
            self.trainer_add.trainer_filter_callback()

    def open_transfer_event_window(self, event=None):
        all_event_ids = self.event_list.get_all_selected_event_ids(allow_event_items=False)
        if len(all_event_ids) == 0:
            return

        invalid_folders = set()
        for cur_event_id in all_event_ids:
            for cur_invalid in self._controller.get_invalid_folders(cur_event_id):
                invalid_folders.add(cur_invalid)
        
        self.new_event_window = TransferEventWindow(
            self,
            self._controller,
            self._controller.get_all_folder_names(),
            [x for x in self._controller.get_all_folder_names() if x not in invalid_folders],
            all_event_ids
        )

    def scroll_to_top(self, event=None):
        """Scroll to the top of the event list."""
        self.event_list.scroll_to_top()
    
    def scroll_to_bottom(self, event=None):
        """Scroll to the bottom of the event list."""
        self.event_list.scroll_to_bottom()

    def rename_folder(self, *args, **kwargs):
        all_event_ids = self.event_list.get_all_selected_event_ids()
        if len(all_event_ids) > 1 or len(all_event_ids) == 0:
            return

        self.open_new_folder_window(**{const.EVENT_FOLDER_NAME: self._controller.get_event_by_id(all_event_ids[0]).name})

    def open_new_folder_window(self, *args, **kwargs):
        all_event_ids = self.event_list.get_all_selected_event_ids()
        if len(all_event_ids) > 1:
            return

        if const.EVENT_FOLDER_NAME in kwargs:
            existing_folder_name = kwargs.get(const.EVENT_FOLDER_NAME)
        else:
            existing_folder_name = None
        self.new_event_window = NewFolderWindow(
            self,
            self._controller,
            self._controller.get_all_folder_names(),
            existing_folder_name,
            insert_after=all_event_ids[0] if len(all_event_ids) == 1 else None
        )

    def toggle_auto_load_most_recent_route(self, event=None):
        """Toggle the auto-load most recent route setting."""
        new_value = not config.get_auto_load_most_recent_route()
        config.set_auto_load_most_recent_route(new_value)
        self.auto_load_menu_var.set(new_value)
        # Update landing page checkbox if it exists
        if hasattr(self, 'landing_page') and hasattr(self.landing_page, 'auto_load_var'):
            self.landing_page.auto_load_var.set(new_value)
        return "break"  # Prevent default F2 behavior
    
    def _on_landing_page_auto_load_toggle(self):
        """Handle auto-load toggle from landing page - sync menu."""
        self.auto_load_menu_var.set(config.get_auto_load_most_recent_route())
    
    def cancel_and_quit(self, *args, **kwargs):
        self.destroy()


def fixed_map(option, style):
    # Fix for setting text colour for Tkinter 8.6.9
    # From: https://core.tcl.tk/tk/info/509cafafae
    #
    # Returns the style map for 'option' with any styles starting with
    # ('!disabled', '!selected', ...) filtered out.

    # style.map() returns an empty list for missing options, so this
    # should be future-safe.
    return [elm for elm in style.map('Treeview', query_opt=option) if
      elm[:2] != ('!disabled', '!selected')]
