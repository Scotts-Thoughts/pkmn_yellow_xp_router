import os
import json
import tkinter as tk
from tkinter import ttk
from typing import List, Tuple, Callable, Optional
from datetime import datetime

from gui import custom_components
from utils.constants import const
from utils import io_utils
from utils.config_manager import config
from pkmn.gen_factory import _gen_factory as gen_factory


class LandingPage(ttk.Frame):
    """Landing page shown when no route is loaded."""
    
    SORT_MOST_RECENT = "most_recent"
    SORT_GAME = "game"
    SORT_ALPHABETICAL = "alphabetical"
    
    def __init__(self, parent, controller, on_create_route: Callable, on_load_route: Callable, on_auto_load_toggle: Callable = None, *args, **kwargs):
        super().__init__(parent, *args, **kwargs)
        self._controller = controller
        self._on_create_route = on_create_route
        self._on_load_route = on_load_route
        self._on_auto_load_toggle = on_auto_load_toggle
        # Load saved sort and filter selections from config
        self._current_sort = config.get_landing_page_sort()
        self._route_metadata_cache = {}  # Cache: route_name -> (game_version, mtime)
        self._selected_game_filter = config.get_landing_page_game_filter()
        # Load saved search filter from config
        saved_search_filter = config.get_landing_page_search_filter()
        self._search_text = saved_search_filter.strip().lower() if saved_search_filter else ""
        
        self.grid_columnconfigure(0, weight=1)
        self.grid_rowconfigure(4, weight=1)
        
        # Title
        title_label = ttk.Label(self, text="Pokemon XP Router", font=("", 24, "bold"))
        title_label.grid(row=0, column=0, pady=(50, 20))
        
        # Create new route button with larger font
        # Create a style for the button
        button_style = ttk.Style()
        button_style.configure("Large.TButton", font=("", 14, "bold"))
        
        self.create_button = custom_components.SimpleButton(
            self, 
            text="Create New Route", 
            command=self._handle_create_route,
            width=30,
            style="Large.TButton"
        )
        self.create_button.grid(row=1, column=0, pady=(20, 10), ipady=10)
        
        # Load selected route button (same formatting as Create New Route)
        self.load_button = custom_components.SimpleButton(
            self,
            text="Load Selected Route",
            command=self._handle_load_selected_route,
            width=30,
            style="Large.TButton"
        )
        self.load_button.grid(row=2, column=0, pady=(0, 10), ipady=10)
        self.load_button.disable()
        
        # Auto-load toggle checkbox
        self.auto_load_var = tk.BooleanVar(value=config.get_auto_load_most_recent_route())
        self.auto_load_checkbox = ttk.Checkbutton(
            self,
            text="Automatically Load Most Recent Route on Startup",
            variable=self.auto_load_var,
            command=self._on_auto_load_toggle_changed
        )
        self.auto_load_checkbox.grid(row=3, column=0, pady=(0, 20))
        
        # Routes section - limit width and prevent expansion
        routes_frame = ttk.Frame(self)
        routes_frame.grid(row=4, column=0, sticky="ns", padx=50, pady=20)
        routes_frame.grid_columnconfigure(0, weight=1)
        routes_frame.grid_rowconfigure(3, weight=1)
        # Set fixed width - don't expand beyond this
        routes_frame.configure(width=500)
        routes_frame.grid_propagate(False)  # Prevent children from changing frame size
        
        routes_label = ttk.Label(routes_frame, text="Recent Routes", font=("", 16))
        routes_label.grid(row=0, column=0, pady=(0, 10))
        
        # Sort controls (on their own row)
        sort_controls_frame = ttk.Frame(routes_frame)
        sort_controls_frame.grid(row=1, column=0, sticky="w", pady=(0, 5))
        
        sort_label = ttk.Label(sort_controls_frame, text="Sort by:")
        sort_label.pack(side=tk.LEFT, padx=(0, 10))
        
        self.sort_var = tk.StringVar(value=self._current_sort)
        self.sort_var.trace("w", self._on_sort_changed)
        
        # Most Recent (left)
        sort_most_recent = ttk.Radiobutton(
            sort_controls_frame, 
            text="Most Recent", 
            variable=self.sort_var, 
            value=self.SORT_MOST_RECENT
        )
        sort_most_recent.pack(side=tk.LEFT, padx=5)
        
        # Alphabetical (middle)
        sort_alphabetical = ttk.Radiobutton(
            sort_controls_frame, 
            text="Alphabetical", 
            variable=self.sort_var, 
            value=self.SORT_ALPHABETICAL
        )
        sort_alphabetical.pack(side=tk.LEFT, padx=5)
        
        # Game (right)
        sort_game = ttk.Radiobutton(
            sort_controls_frame, 
            text="Game", 
            variable=self.sort_var, 
            value=self.SORT_GAME
        )
        sort_game.pack(side=tk.LEFT, padx=5)
        
        # Get all available game versions
        all_games = ["All Games"] + gen_factory.get_gen_names(real_gens=True, custom_gens=True)
        
        # Game filter dropdown (shown when Game sort is selected)
        self.game_filter_dropdown = custom_components.SimpleOptionMenu(
            sort_controls_frame,
            all_games,
            callback=self._on_game_filter_changed,
            default_val=self._selected_game_filter
        )
        self.game_filter_dropdown.config(width=20)
        # Show dropdown if Game sort is the saved preference, otherwise hide it
        if self._current_sort == self.SORT_GAME:
            self.game_filter_dropdown.pack(side=tk.LEFT, padx=(10, 0))
        else:
            self.game_filter_dropdown.pack_forget()
        
        # Search bar (on its own row, full width)
        search_frame = ttk.Frame(routes_frame)
        search_frame.grid(row=2, column=0, sticky="ew", pady=(0, 10))
        search_frame.grid_columnconfigure(1, weight=1)  # Make search entry expand
        
        search_label = ttk.Label(search_frame, text="Search:")
        search_label.grid(row=0, column=0, sticky="w", padx=(0, 5))
        
        self.search_entry = custom_components.SimpleEntry(
            search_frame,
            callback=self._on_search_changed,
            width=60,
            initial_value=saved_search_filter if saved_search_filter else ""
        )
        self.search_entry.grid(row=0, column=1, sticky="ew")
        
        # Route list with Treeview (multi-column) and scrollbar
        list_frame = ttk.Frame(routes_frame)
        list_frame.grid(row=3, column=0, sticky="nsew")
        list_frame.grid_columnconfigure(0, weight=1)
        list_frame.grid_rowconfigure(0, weight=1)
        
        # Create Treeview with columns
        columns = ("Game", "Species", "Route Name", "Date Played")
        self.route_treeview = ttk.Treeview(
            list_frame,
            columns=columns,
            show="headings",
            selectmode="browse",
            height=15
        )
        
        # Configure column headings and widths
        self.route_treeview.heading("Game", text="Game")
        self.route_treeview.heading("Species", text="Species")
        self.route_treeview.heading("Route Name", text="Route Name")
        self.route_treeview.heading("Date Played", text="Date Played")
        
        # Set column widths - Game and Species smaller, Route Name wider
        self.route_treeview.column("Game", width=70, anchor=tk.W, minwidth=50)
        self.route_treeview.column("Species", width=80, anchor=tk.W, minwidth=60)
        self.route_treeview.column("Route Name", width=220, anchor=tk.W, minwidth=100)
        self.route_treeview.column("Date Played", width=130, anchor=tk.W, minwidth=100)
        
        scrollbar = ttk.Scrollbar(list_frame, orient="vertical", command=self.route_treeview.yview)
        self.route_treeview.configure(yscrollcommand=scrollbar.set)
        
        self.route_treeview.grid(row=0, column=0, sticky="nsew")
        scrollbar.grid(row=0, column=1, sticky="ns")
        
        self.route_treeview.bind("<Double-Button-1>", self._handle_route_double_click)
        self.route_treeview.bind("<Return>", self._handle_route_double_click)
        
        self.route_treeview.bind("<<TreeviewSelect>>", self._on_route_selection_changed)
        
        # Refresh route list
        self.refresh_routes()
    
    def _handle_create_route(self):
        """Handle create new route button click."""
        self._on_create_route()
    
    def _handle_load_selected_route(self):
        """Handle load selected route button click."""
        selection = self.route_treeview.selection()
        if selection:
            item = selection[0]
            values = self.route_treeview.item(item, "values")
            if len(values) >= 3:
                route_name = values[2]  # Route Name is in column 2
                # Skip if this is the "No saved routes found" placeholder
                if route_name and route_name != "No saved routes found":
                    route_path = io_utils.get_existing_route_path(route_name)
                    self._on_load_route(route_path)
    
    def _handle_route_double_click(self, event=None):
        """Handle double-click on route list."""
        selection = self.route_treeview.selection()
        if selection:
            item = selection[0]
            values = self.route_treeview.item(item, "values")
            if len(values) >= 3:
                route_name = values[2]  # Route Name is in column 2
                # Skip if this is the "No saved routes found" placeholder
                if route_name and route_name != "No saved routes found":
                    route_path = io_utils.get_existing_route_path(route_name)
                    self._on_load_route(route_path)
    
    def _on_auto_load_toggle_changed(self):
        """Handle auto-load toggle checkbox change."""
        config.set_auto_load_most_recent_route(self.auto_load_var.get())
        # Notify parent window to sync menu if callback provided
        if self._on_auto_load_toggle:
            self._on_auto_load_toggle()
    
    def _on_route_selection_changed(self, event=None):
        """Enable/disable load button based on selection."""
        selection = self.route_treeview.selection()
        if selection:
            item = selection[0]
            values = self.route_treeview.item(item, "values")
            # Enable only if we have valid route data (not the placeholder)
            if len(values) >= 3 and values[2] and values[2] != "No saved routes found":
                self.load_button.enable()
            else:
                self.load_button.disable()
        else:
            self.load_button.disable()
    
    def _on_sort_changed(self, *args):
        """Handle sort option change."""
        self._current_sort = self.sort_var.get()
        # Save the sort selection to config
        config.set_landing_page_sort(self._current_sort)
        
        # Show/hide game filter dropdown based on sort selection
        if self._current_sort == self.SORT_GAME:
            self.game_filter_dropdown.pack(side=tk.LEFT, padx=(10, 0))
            # Set the dropdown to the remembered value
            current_val = self.game_filter_dropdown.get()
            if current_val != self._selected_game_filter:
                # Try to set to remembered value, fallback to "All Games" if not available
                try:
                    self.game_filter_dropdown.set(self._selected_game_filter)
                except:
                    self.game_filter_dropdown.set("All Games")
                    self._selected_game_filter = "All Games"
                    config.set_landing_page_game_filter(self._selected_game_filter)
        else:
            self.game_filter_dropdown.pack_forget()
        
        self.refresh_routes()
    
    def _on_game_filter_changed(self, *args):
        """Handle game filter dropdown change."""
        selected_game = self.game_filter_dropdown.get()
        if selected_game:
            self._selected_game_filter = selected_game
            # Save the game filter selection to config
            config.set_landing_page_game_filter(self._selected_game_filter)
            self.refresh_routes()
    
    def _on_search_changed(self, *args):
        """Handle search text change."""
        search_value = self.search_entry.get()
        self._search_text = search_value.strip().lower()
        # Save the search filter to config
        config.set_landing_page_search_filter(search_value)
        self.refresh_routes()
    
    def _get_route_metadata(self, route_name: str) -> Tuple[str, str, str, float]:
        """Get metadata for a route: (game_version, species_name, route_name, mtime). Uses cache when possible."""
        route_path = io_utils.get_existing_route_path(route_name)
        
        # Get file modification time
        try:
            mtime = os.path.getmtime(route_path)
        except Exception:
            mtime = 0
        
        # Check cache - use cached data if file hasn't changed
        if route_name in self._route_metadata_cache:
            cached_data = self._route_metadata_cache[route_name]
            if len(cached_data) == 2:  # Old cache format (game_version, mtime)
                cached_version, cached_mtime = cached_data
                if cached_mtime == mtime:
                    # Need to read species, so invalidate cache and reload
                    pass
                else:
                    # File changed, need to reload
                    pass
            elif len(cached_data) == 3:  # New cache format (game_version, species_name, mtime)
                cached_version, cached_species, cached_mtime = cached_data
                if cached_mtime == mtime:
                    return cached_version, cached_species, route_name, mtime
        
        # Cache miss or file changed - read from file
        game_version = "Unknown"
        species_name = "Unknown"
        try:
            route_data = io_utils.read_json_file_safe(route_path, max_wait_seconds=0.5)
            game_version = route_data.get(const.PKMN_VERSION_KEY, "Unknown")
            species_name = route_data.get(const.NAME_KEY, "Unknown")
        except ValueError as e:
            # File is empty or cloud placeholder - check if it's a cloud sync issue
            if io_utils.is_likely_cloud_placeholder(route_path):
                game_version = "(Sync pending)"
                species_name = "(Sync pending)"
            # else leave as "Unknown"
        except Exception:
            pass
        
        # Update cache
        self._route_metadata_cache[route_name] = (game_version, species_name, mtime)
        
        return game_version, species_name, route_name, mtime
    
    def refresh_routes(self):
        """Refresh the route list based on current sort."""
        # Update game filter dropdown with current game list (in case custom gens were added)
        if self._current_sort == self.SORT_GAME:
            all_games = ["All Games"] + gen_factory.get_gen_names(real_gens=True, custom_gens=True)
            current_selection = self.game_filter_dropdown.get()
            self.game_filter_dropdown.new_values(all_games, default_val=current_selection if current_selection in all_games else "All Games")
        
        # Get all route names
        all_routes = io_utils.get_existing_route_names(load_backups=False)
        
        if not all_routes:
            # Clear treeview
            for item in self.route_treeview.get_children():
                self.route_treeview.delete(item)
            self.route_treeview.insert("", "end", values=("", "", "No saved routes found", ""))
            # Clear cache for routes that no longer exist
            self._route_metadata_cache = {
                k: v for k, v in self._route_metadata_cache.items() 
                if k in all_routes
            }
            return
        
        # Get metadata for all routes (uses cache when possible)
        route_metadata = []
        for route_name in all_routes:
            game_version, species_name, route_name_clean, mtime = self._get_route_metadata(route_name)
            route_metadata.append((route_name_clean, game_version, species_name, mtime))
        
        # Clear cache for routes that no longer exist
        existing_route_set = set(all_routes)
        self._route_metadata_cache = {
            k: v for k, v in self._route_metadata_cache.items() 
            if k in existing_route_set
        }
        
        # Filter by game if Game sort is selected and a specific game is chosen
        if self._current_sort == self.SORT_GAME and self._selected_game_filter != "All Games":
            route_metadata = [
                (name, version, species, mtime) for name, version, species, mtime in route_metadata
                if version == self._selected_game_filter
            ]
        
        # Filter by search text if provided
        if self._search_text:
            route_metadata = [
                (name, version, species, mtime) for name, version, species, mtime in route_metadata
                if (self._search_text in name.lower() or 
                    self._search_text in version.lower() or 
                    self._search_text in species.lower())
            ]
        
        # Sort based on current sort option
        if self._current_sort == self.SORT_MOST_RECENT:
            route_metadata.sort(key=lambda x: x[3], reverse=True)  # Sort by mtime descending
        elif self._current_sort == self.SORT_GAME:
            route_metadata.sort(key=lambda x: (x[1], x[0]))  # Sort by game version, then name
        elif self._current_sort == self.SORT_ALPHABETICAL:
            route_metadata.sort(key=lambda x: x[0].lower())  # Sort alphabetically
        
        # Update treeview
        for item in self.route_treeview.get_children():
            self.route_treeview.delete(item)
        
        for route_name, game_version, species_name, mtime in route_metadata:
            # Format date
            try:
                mtime_str = datetime.fromtimestamp(mtime).strftime("%Y-%m-%d %H:%M")
            except Exception:
                mtime_str = "Unknown"
            
            # Insert into treeview: Game, Species, Route Name, Date Played
            self.route_treeview.insert("", "end", values=(game_version, species_name, route_name, mtime_str))

