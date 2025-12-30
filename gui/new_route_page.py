import os
import json

import tkinter as tk
from tkinter import ttk

from controllers.main_controller import MainController
from gui.popups.custom_dvs_popup import CustomDVsFrame
from gui import custom_components
from utils.constants import const
from utils import io_utils
from pkmn.gen_factory import _gen_factory as gen_factory, current_gen_info


class NewRoutePage(ttk.Frame):
    """Full-page component for creating a new route."""
    
    # Game information mapping
    GAME_INFO = {
        const.RED_VERSION: ("Generation 1", "GB/GBC", "Available"),
        const.BLUE_VERSION: ("Generation 1", "GB/GBC", "Available"),
        const.YELLOW_VERSION: ("Generation 1", "GB/GBC", "Available"),
        const.GOLD_VERSION: ("Generation 2", "GBC", "Available"),
        const.SILVER_VERSION: ("Generation 2", "GBC", "Available"),
        const.CRYSTAL_VERSION: ("Generation 2", "GBC", "Available"),
        const.RUBY_VERSION: ("Generation 3", "GBA", "Available"),
        const.SAPPHIRE_VERSION: ("Generation 3", "GBA", "Available"),
        const.EMERALD_VERSION: ("Generation 3", "GBA", "Available"),
        const.FIRE_RED_VERSION: ("Generation 3", "GBA", "Available"),
        const.LEAF_GREEN_VERSION: ("Generation 3", "GBA", "Available"),
        const.DIAMOND_VERSION: ("Generation 4", "NDS", "Unavailable"),
        const.PEARL_VERSION: ("Generation 4", "NDS", "Unavailable"),
        const.PLATINUM_VERSION: ("Generation 4", "NDS", "In Beta"),
        const.HEART_GOLD_VERSION: ("Generation 4", "NDS", "In Alpha"),
        const.SOUL_SILVER_VERSION: ("Generation 4", "NDS", "In Alpha"),
    }
    
    def __init__(self, parent, controller: MainController, on_cancel: callable, on_create: callable, *args, **kwargs):
        super().__init__(parent, *args, **kwargs)
        self._controller = controller
        self._on_cancel = on_cancel
        self._on_create = on_create
        self._selected_game = None  # Track selected game
        self._selected_gen_obj = None  # Cache the gen object to avoid repeated lookups
        self._current_gen_num = None  # Track current generation number
        self._route_cache_per_game = {}  # Cache route lists per game to avoid re-reading files
        self._pkmn_list_cache = {}  # Cache pokemon lists per game/filter combination: (game, filter) -> list
        self._last_pkmn_filter = ""  # Track last filter to avoid unnecessary updates
        self._loading_popup = None  # Loading popup window
        self._suppress_loading_popup = True  # Suppress popup during initial startup
        
        self.grid_columnconfigure(0, weight=1)
        self.grid_rowconfigure(1, weight=1)
        
        # Title
        title_label = ttk.Label(self, text="Create New Route", font=("", 24, "bold"))
        title_label.grid(row=0, column=0, pady=(30, 10))
        
        # Main content frame - center aligned
        self.content_frame = ttk.Frame(self)
        self.content_frame.grid(row=1, column=0, sticky="nsew", padx=100, pady=10)
        # Use 3 columns: left spacer, content, right spacer - to center content
        self.content_frame.grid_columnconfigure(0, weight=1)
        self.content_frame.grid_columnconfigure(1, weight=0)  # Content column (no expansion)
        self.content_frame.grid_columnconfigure(2, weight=1)  # Right spacer
        self.content_frame.grid_rowconfigure(0, weight=0)  # Don't expand game table row
        
        self.padx = 10
        self.pady = 5
        
        # Inner frame to hold label and table together, centered
        version_container = ttk.Frame(self.content_frame)
        version_container.grid(row=0, column=1, padx=self.padx, pady=self.pady, sticky="")
        
        # Pokemon Version - Table selection (centered)
        self.pkmn_version_label = tk.Label(version_container, text="Pokemon Version:", font=("", 12))
        self.pkmn_version_label.grid(row=0, column=0, padx=self.padx, pady=self.pady, sticky=tk.N)
        
        # Game selection table frame (centered)
        game_table_frame = ttk.Frame(version_container)
        game_table_frame.grid(row=0, column=1, padx=self.padx, pady=self.pady, sticky=tk.N)
        game_table_frame.grid_columnconfigure(0, weight=1)
        game_table_frame.grid_rowconfigure(0, weight=1)
        
        # Create Treeview for game selection
        columns = ("Game", "Generation", "Platform", "Recorder")
        self.game_treeview = ttk.Treeview(
            game_table_frame,
            columns=columns,
            show="headings",
            selectmode="browse",
            height=8
        )
        
        # Configure column headings and widths
        self.game_treeview.heading("Game", text="Game")
        self.game_treeview.heading("Generation", text="Generation")
        self.game_treeview.heading("Platform", text="Platform")
        self.game_treeview.heading("Recorder", text="Recorder")
        
        # Set column widths
        self.game_treeview.column("Game", width=120, anchor=tk.W)
        self.game_treeview.column("Generation", width=120, anchor=tk.W)
        self.game_treeview.column("Platform", width=100, anchor=tk.W)
        self.game_treeview.column("Recorder", width=120, anchor=tk.W)
        
        scrollbar_games = ttk.Scrollbar(game_table_frame, orient="vertical", command=self.game_treeview.yview)
        self.game_treeview.configure(yscrollcommand=scrollbar_games.set)
        
        self.game_treeview.grid(row=0, column=0, sticky="nsew")
        scrollbar_games.grid(row=0, column=1, sticky="ns")
        
        self.game_treeview.bind("<<TreeviewSelect>>", self._on_game_selection_changed)
        
        # Populate game table
        self._populate_game_table()
        
        # Solo Pokemon Filter (centered)
        filter_container1 = ttk.Frame(self.content_frame)
        filter_container1.grid(row=1, column=1, padx=self.padx, pady=self.pady, sticky="")
        self.pkmn_filter_label = tk.Label(filter_container1, text="Solo Pokemon Filter:", font=("", 12))
        self.pkmn_filter_label.grid(row=0, column=0, padx=self.padx, pady=self.pady, sticky=tk.N)
        self.pkmn_filter = custom_components.SimpleEntry(filter_container1, callback=self._pkmn_filter_callback)
        self.pkmn_filter.config(width=40, font=("", 11))
        self.pkmn_filter.grid(row=0, column=1, padx=self.padx, pady=self.pady, sticky=tk.N)
        
        # Solo Pokemon Selector (centered)
        selector_container1 = ttk.Frame(self.content_frame)
        selector_container1.grid(row=2, column=1, padx=self.padx, pady=self.pady, sticky="")
        self.solo_selector_label = tk.Label(selector_container1, text="Solo Pokemon:", font=("", 12))
        self.solo_selector_label.grid(row=0, column=0, padx=self.padx, pady=self.pady, sticky=tk.N)
        self.solo_selector = custom_components.SimpleOptionMenu(
            selector_container1, 
            [const.NO_POKEMON], 
            callback=self._pkmn_selector_callback
        )
        self.solo_selector.config(width=30)
        self.solo_selector.grid(row=0, column=1, padx=self.padx, pady=self.pady, sticky=tk.N)
        
        # Base Route Filter (centered)
        filter_container2 = ttk.Frame(self.content_frame)
        filter_container2.grid(row=3, column=1, padx=self.padx, pady=self.pady, sticky="")
        self.min_battles_filter_label = tk.Label(filter_container2, text="Base Route Filter:", font=("", 12))
        self.min_battles_filter_label.grid(row=0, column=0, padx=self.padx, pady=self.pady, sticky=tk.N)
        self.min_battles_filter = custom_components.SimpleEntry(
            filter_container2, 
            callback=self._base_route_filter_callback
        )
        self.min_battles_filter.config(width=40, font=("", 11))
        self.min_battles_filter.grid(row=0, column=1, padx=self.padx, pady=self.pady, sticky=tk.N)
        
        # Base Route Selector (centered)
        self._min_battles_cache = [const.EMPTY_ROUTE_NAME]
        selector_container2 = ttk.Frame(self.content_frame)
        selector_container2.grid(row=4, column=1, padx=self.padx, pady=self.pady, sticky="")
        self.min_battles_selector_label = tk.Label(selector_container2, text="Base Route:", font=("", 12))
        self.min_battles_selector_label.grid(row=0, column=0, padx=self.padx, pady=self.pady, sticky=tk.N)
        self.min_battles_selector = custom_components.SimpleOptionMenu(
            selector_container2, 
            self._min_battles_cache
        )
        self.min_battles_selector.config(width=30)
        self.min_battles_selector.grid(row=0, column=1, padx=self.padx, pady=self.pady, sticky=tk.N)
        
        # Custom DVs Frame (centered)
        self.custom_dvs_frame = CustomDVsFrame(None, self.content_frame, target_game=current_gen_info())
        self.custom_dvs_frame.grid(row=5, column=1, sticky="", padx=self.padx, pady=(self.pady, self.pady))
        
        # Warning label (centered)
        self.warning_label = tk.Label(
            self.content_frame, 
            text="WARNING: Any unsaved changes in your current route\nwill be lost when creating a new route!", 
            justify=tk.CENTER, 
            anchor=tk.CENTER,
            font=("", 10),
            fg="red"
        )
        self.warning_label.grid(row=6, column=1, sticky="", padx=self.padx, pady=(self.pady, self.pady))
        
        # Buttons frame (centered)
        self.buttons_frame = ttk.Frame(self.content_frame)
        self.buttons_frame.grid(row=7, column=1, pady=(self.pady, 0))
        
        self.create_button = custom_components.SimpleButton(
            self.buttons_frame, 
            text="Create Route", 
            command=self.create,
            width=20
        )
        self.create_button.grid(row=0, column=0, padx=self.padx, pady=self.pady)
        
        self.cancel_button = custom_components.SimpleButton(
            self.buttons_frame, 
            text="Cancel", 
            command=self._on_cancel,
            width=20
        )
        self.cancel_button.grid(row=0, column=1, padx=self.padx, pady=self.pady)
        
        # Bind keyboard shortcuts
        self.bind('<Return>', self.create)
        self.bind('<Escape>', lambda e: self._on_cancel())
        
        # Initialize - select first game by default if available
        if len(self.game_treeview.get_children()) > 0:
            first_item = self.game_treeview.get_children()[0]
            self.game_treeview.selection_set(first_item)
            self.game_treeview.focus(first_item)
            self._on_game_selection_changed()
        
        self.pkmn_filter.focus()
    
    def _populate_game_table(self):
        """Populate the game selection table with all available games."""
        # Clear existing entries
        for item in self.game_treeview.get_children():
            self.game_treeview.delete(item)
        
        # Get all available games (including custom gens)
        all_games = gen_factory.get_gen_names(real_gens=True, custom_gens=True)
        
        # Sort games: official games first (in order), then custom gens
        official_games = [
            const.RED_VERSION, const.BLUE_VERSION, const.YELLOW_VERSION,
            const.GOLD_VERSION, const.SILVER_VERSION, const.CRYSTAL_VERSION,
            const.RUBY_VERSION, const.SAPPHIRE_VERSION, const.EMERALD_VERSION,
            const.FIRE_RED_VERSION, const.LEAF_GREEN_VERSION,
            const.DIAMOND_VERSION, const.PEARL_VERSION, const.PLATINUM_VERSION,
            const.HEART_GOLD_VERSION, const.SOUL_SILVER_VERSION
        ]
        
        # Add official games that are available
        sorted_games = [g for g in official_games if g in all_games]
        # Add custom gens
        custom_gens = [g for g in all_games if g not in official_games]
        sorted_games.extend(sorted(custom_gens))
        
        # Populate table
        for game_name in sorted_games:
            if game_name in self.GAME_INFO:
                gen, platform, recorder = self.GAME_INFO[game_name]
            else:
                # Custom gen - try to get generation from the gen object
                try:
                    gen_obj = gen_factory.get_specific_version(game_name)
                    gen_num = gen_obj.get_generation()
                    gen = f"Generation {gen_num}"
                    platform = "Custom"
                    recorder = "Unknown"
                except:
                    gen = "Unknown"
                    platform = "Unknown"
                    recorder = "Unknown"
            
            self.game_treeview.insert("", "end", values=(game_name, gen, platform, recorder))
    
    def _show_loading_popup(self):
        """Show loading popup."""
        if self._loading_popup is not None:
            return  # Already showing
        if self._suppress_loading_popup:
            return  # Suppressed during startup
        
        # Get the main window (parent of parent)
        main_window = self.winfo_toplevel()
        
        # Create loading popup
        self._loading_popup = tk.Toplevel(main_window)
        self._loading_popup.title("Loading")
        self._loading_popup.resizable(False, False)
        self._loading_popup.attributes('-topmost', True)
        
        # Disable interaction with main window
        self._loading_popup.transient(main_window)
        self._loading_popup.grab_set()
        
        # Create content
        content_frame = ttk.Frame(self._loading_popup, padding=20)
        content_frame.pack()
        
        loading_label = tk.Label(
            content_frame,
            text="Loading Generation Data...",
            font=("", 12)
        )
        loading_label.pack(pady=10)
        
        # Center the popup on the main window
        self._loading_popup.update_idletasks()
        try:
            main_x = main_window.winfo_x()
            main_y = main_window.winfo_y()
            main_width = main_window.winfo_width()
            main_height = main_window.winfo_height()
            
            popup_width = self._loading_popup.winfo_width()
            popup_height = self._loading_popup.winfo_height()
            
            x = main_x + (main_width // 2) - (popup_width // 2)
            y = main_y + (main_height // 2) - (popup_height // 2)
            
            self._loading_popup.geometry(f"+{x}+{y}")
        except Exception:
            # Fallback to screen center
            screen_width = self._loading_popup.winfo_screenwidth()
            screen_height = self._loading_popup.winfo_screenheight()
            x = (screen_width // 2) - (popup_width // 2)
            y = (screen_height // 2) - (popup_height // 2)
            self._loading_popup.geometry(f"+{x}+{y}")
        
        self._loading_popup.update()
    
    def _hide_loading_popup(self):
        """Hide loading popup."""
        if self._loading_popup is not None:
            self._loading_popup.destroy()
            self._loading_popup = None
    
    def _on_game_selection_changed(self, event=None):
        """Handle game selection change from table."""
        selection = self.game_treeview.selection()
        if selection:
            item = selection[0]
            values = self.game_treeview.item(item, "values")
            if len(values) > 0:
                new_game = values[0]  # Game name is in first column
                # Only update if game actually changed
                if new_game != self._selected_game:
                    # Show loading popup if we need to load data
                    needs_loading = (
                        new_game not in self._route_cache_per_game or
                        (new_game, self.pkmn_filter.get().strip()) not in self._pkmn_list_cache
                    )
                    
                    if needs_loading:
                        self._show_loading_popup()
                    
                    self._selected_game = new_game
                    # Cache the gen object to avoid repeated lookups
                    self._selected_gen_obj = gen_factory.get_specific_version(self._selected_game)
                    new_gen_num = self._selected_gen_obj.get_generation()
                    
                    # Only update DV frame if generation changed
                    gen_changed = (self._current_gen_num != new_gen_num)
                    self._current_gen_num = new_gen_num
                    
                    # Immediately update pokemon list (lightweight if cached)
                    self._update_pokemon_list_immediate()
                    
                    # Defer heavy operations to avoid blocking UI
                    self.after_idle(lambda: self._pkmn_version_callback(gen_changed=gen_changed))
    
    def _update_pokemon_list_immediate(self):
        """Update pokemon list immediately (uses cache if available)."""
        if not self._selected_game or not self._selected_gen_obj:
            return
        
        filter_val = self.pkmn_filter.get().strip()
        cache_key = (self._selected_game, filter_val)
        
        # Check cache first
        if cache_key in self._pkmn_list_cache:
            pkmn_list = self._pkmn_list_cache[cache_key]
        else:
            # Build pokemon list (this is the expensive operation)
            # Show loading popup if not already showing
            if self._loading_popup is None:
                self._show_loading_popup()
            pkmn_list = self._selected_gen_obj.pkmn_db().get_filtered_names(filter_val=filter_val)
            # Cache it
            self._pkmn_list_cache[cache_key] = pkmn_list
        
        # Only update dropdown if values changed
        self.solo_selector.new_values(pkmn_list)
    
    def _pkmn_version_callback(self, *args, **kwargs):
        """Handle pokemon version change - deferred heavy operations."""
        if not self._selected_game or not self._selected_gen_obj:
            self._hide_loading_popup()
            return
        
        gen_changed = kwargs.get('gen_changed', True)
        
        # Pokemon list already updated in _update_pokemon_list_immediate()
        # Now handle route cache building (can be slow)
        
        # Build route cache - use cached version if available
        if self._selected_game in self._route_cache_per_game:
            all_routes = self._route_cache_per_game[self._selected_game]
        else:
            # Build route cache (only do this once per game) - this can be slow
            all_routes = [const.EMPTY_ROUTE_NAME]
            for preset_route_name in self._selected_gen_obj.min_battles_db().data:
                all_routes.append(const.PRESET_ROUTE_PREFIX + preset_route_name)
            
            # Only read route files if we haven't cached this game yet
            # This is the slowest operation - reading all route files
            route_names = io_utils.get_existing_route_names()
            for test_route in route_names:
                try:
                    with open(io_utils.get_existing_route_path(test_route), 'r') as f:
                        raw = json.load(f)
                        if raw[const.PKMN_VERSION_KEY] == self._selected_game:
                            all_routes.append(test_route)
                except Exception:
                    pass
            
            # Cache the result
            self._route_cache_per_game[self._selected_game] = all_routes
        
        self._min_battles_cache = all_routes
        self._base_route_filter_callback()
        
        # Only update custom DVs frame if generation changed
        # This avoids expensive widget recreation when just switching between games of same gen
        if gen_changed:
            selected_pokemon = self.solo_selector.get()
            if selected_pokemon and selected_pokemon != const.NO_POKEMON:
                # Defer this heavy operation
                self.after_idle(lambda: self._update_dvs_frame_and_hide_popup(
                    self._selected_gen_obj, 
                    self._selected_gen_obj.pkmn_db().get_pkmn(selected_pokemon)
                ))
            else:
                # No pokemon selected, just update for the game
                self.after_idle(lambda: self._update_dvs_frame_and_hide_popup(
                    self._selected_gen_obj, 
                    None
                ))
        else:
            # No DV frame update needed, hide popup now
            self._hide_loading_popup()
    
    def _update_dvs_frame_and_hide_popup(self, gen_obj, pokemon):
        """Update DV frame and hide loading popup."""
        self.custom_dvs_frame.config_for_target_game_and_mon(gen_obj, pokemon)
        self._hide_loading_popup()
        # Enable popup for future use after initial setup
        self._suppress_loading_popup = False
    
    def _pkmn_filter_callback(self, *args, **kwargs):
        """Handle pokemon filter change."""
        if not self._selected_game or not self._selected_gen_obj:
            return
        
        # Use cached version if available
        filter_val = self.pkmn_filter.get().strip()
        cache_key = (self._selected_game, filter_val)
        
        # Check if we already have this cached
        if cache_key in self._pkmn_list_cache:
            pkmn_list = self._pkmn_list_cache[cache_key]
        else:
            # Build and cache pokemon list - defer if list is large
            # For empty filter, get all names is faster than filtered
            if filter_val == "":
                pkmn_list = self._selected_gen_obj.pkmn_db().get_all_names()
            else:
                pkmn_list = self._selected_gen_obj.pkmn_db().get_filtered_names(filter_val=filter_val)
            self._pkmn_list_cache[cache_key] = pkmn_list
        
        # Only update if values actually changed
        self.solo_selector.new_values(pkmn_list)
    
    def _pkmn_selector_callback(self, *args, **kwargs):
        """Handle pokemon selection change."""
        if not self._selected_game or not self._selected_gen_obj:
            return
        
        # Only update DV frame when pokemon changes, not when game changes
        # This avoids unnecessary widget recreation
        selected_pokemon = self.solo_selector.get()
        if selected_pokemon and selected_pokemon != const.NO_POKEMON:
            # Defer this operation to avoid blocking UI
            self.after_idle(lambda: self.custom_dvs_frame.config_for_target_game_and_mon(
                self._selected_gen_obj, 
                self._selected_gen_obj.pkmn_db().get_pkmn(selected_pokemon)
            ))
        else:
            # No pokemon selected
            self.after_idle(lambda: self.custom_dvs_frame.config_for_target_game_and_mon(
                self._selected_gen_obj, 
                None
            ))
    
    def _base_route_filter_callback(self, *args, **kwargs):
        """Handle base route filter change."""
        filter_val = self.min_battles_filter.get().strip().lower()
        new_vals = [x for x in self._min_battles_cache if filter_val in x.lower()]
        
        if not new_vals:
            new_vals = [const.EMPTY_ROUTE_NAME]
        
        self.min_battles_selector.new_values(new_vals)
    
    def create(self, *args, **kwargs):
        """Create the new route."""
        selected_base_route = self.min_battles_selector.get()
        if selected_base_route == const.EMPTY_ROUTE_NAME:
            selected_base_route = None
        elif selected_base_route.startswith(const.PRESET_ROUTE_PREFIX):
            if not self._selected_game or not self._selected_gen_obj:
                return
            selected_base_route = os.path.join(
                self._selected_gen_obj.min_battles_db().get_dir(), 
                selected_base_route[len(const.PRESET_ROUTE_PREFIX):] + ".json"
            )
        else:
            selected_base_route = io_utils.get_existing_route_path(selected_base_route)
        
        if not self._selected_game:
            return
        
        custom_dvs, custom_ability_idx, custom_nature = self.custom_dvs_frame.get_dvs()
        
        self._on_create(
            self.solo_selector.get(),
            selected_base_route,
            self._selected_game,
            custom_dvs=custom_dvs,
            custom_ability_idx=custom_ability_idx,
            custom_nature=custom_nature
        )
    
    def refresh_game_list(self):
        """Refresh the game list - useful after background loading completes."""
        # Remember the currently selected game
        current_selection = None
        selection = self.game_treeview.selection()
        if selection:
            item = selection[0]
            values = self.game_treeview.item(item, "values")
            if len(values) > 0:
                current_selection = values[0]
        
        # Repopulate the game table
        self._populate_game_table()
        
        # Try to restore the previous selection, or select first item
        if current_selection:
            # Find and select the previously selected game
            for item in self.game_treeview.get_children():
                values = self.game_treeview.item(item, "values")
                if len(values) > 0 and values[0] == current_selection:
                    self.game_treeview.selection_set(item)
                    self.game_treeview.focus(item)
                    break
        elif len(self.game_treeview.get_children()) > 0:
            # No previous selection, select first item
            first_item = self.game_treeview.get_children()[0]
            self.game_treeview.selection_set(first_item)
            self.game_treeview.focus(first_item)
    
    def reset_form(self):
        """Resets the form to its initial state."""
        self.pkmn_filter.set("")
        self.min_battles_filter.set("")
        self._populate_game_table()  # Repopulate to ensure all games are there
        self._selected_game = None
        self._selected_gen_obj = None
        self._current_gen_num = None
        self._last_pkmn_filter = ""
        # Clear caches to ensure fresh data
        self._pkmn_list_cache.clear()
        if len(self.game_treeview.get_children()) > 0:
            first_item = self.game_treeview.get_children()[0]
            self.game_treeview.selection_set(first_item)
            self.game_treeview.focus(first_item)
            self._on_game_selection_changed()  # Trigger selection change to update dependent fields
        else:
            self.solo_selector.new_values([const.NO_POKEMON])
            self.min_battles_selector.new_values([const.EMPTY_ROUTE_NAME])
            self.custom_dvs_frame.config_for_target_game_and_mon(current_gen_info(), None)

