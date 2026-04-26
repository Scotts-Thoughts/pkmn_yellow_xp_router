import json
import os
import copy

from utils.constants import const
from utils import io_utils

# -----------------------------------------------------------------------
# Default keyboard shortcuts -- canonical source of truth
# Keys are action IDs used internally; values are Qt key-sequence strings.
# -----------------------------------------------------------------------
DEFAULT_SHORTCUTS = {
    # File menu
    "customize_dvs":            "Ctrl+X",
    "new_route":                "Ctrl+N",
    "load_route":               "Ctrl+L",
    "save_route":               "Ctrl+S",
    "close_route":              "Ctrl+Shift+C",
    "auto_load_recent":         "F2",
    "export_notes":             "Ctrl+Shift+W",
    "screenshot_events":        "F5",
    "screenshot_battle":        "F6",
    "screenshot_player":        "F7",
    "screenshot_enemy":         "F8",
    "open_image_folder":        "F12",
    "config_font":              "Ctrl+Shift+D",
    "custom_gens":              "Ctrl+Shift+E",
    "app_config":               "Ctrl+Shift+A",
    "open_data_folder":         "Ctrl+Shift+O",
    "keyboard_shortcuts":       "Ctrl+Shift+K",
    # Events menu
    "undo":                     "Ctrl+Z",
    "move_event_up":            "Ctrl+E",
    "move_event_down":          "Ctrl+D",
    "move_event_up_folder":     "Ctrl+Shift+E",
    "move_event_down_folder":   "Ctrl+Shift+D",
    "enable_disable":           "Ctrl+C",
    "toggle_highlight":         "Ctrl+V",
    "delete_event":             "Ctrl+B",
    # Highlight menu
    "highlight_1":              "Shift+1",
    "highlight_2":              "Shift+2",
    "highlight_3":              "Shift+3",
    "highlight_4":              "Shift+4",
    "highlight_5":              "Shift+5",
    "highlight_6":              "Shift+6",
    "highlight_7":              "Shift+7",
    "highlight_8":              "Shift+8",
    "highlight_9":              "Shift+9",
    # Folders menu
    "new_folder":               "Ctrl+Shift+Alt+F",
    "rename_folder":            "Ctrl+Shift+F",
    "split_folder":             "Alt+X",
    # Recording menu
    "toggle_recording":         "F1",
    "final_trainers":           "",
    # Battle Summary menu
    "toggle_move_highlights":   "Shift+F2",
    "toggle_fade_no_highlight": "Shift+F3",
    "toggle_test_moves":        "Shift+F1",
    "candy_decrement":          "F3",
    "candy_increment":          "F4",
    "toggle_player_strat":      "F9",
    "toggle_enemy_strat":       "F10",
    # QShortcut-only (not in menus)
    "delete_key":               "Delete",
    "scroll_home":              "Home",
    "scroll_end":               "End",
    "toggle_tabs":              "`",
    "toggle_summary":           "Ctrl+`",
    "gym_1":                    "1",
    "gym_2":                    "2",
    "gym_3":                    "3",
    "gym_4":                    "4",
    "gym_5":                    "5",
    "gym_6":                    "6",
    "gym_7":                    "7",
    "gym_8":                    "8",
    "gym_blue":                 "9",
    "e4_1":                     "Ctrl+1",
    "e4_2":                     "Ctrl+2",
    "e4_3":                     "Ctrl+3",
    "e4_4":                     "Ctrl+4",
    "e4_5":                     "Ctrl+5",
    "e4_6":                     "Ctrl+6",
    # Filter toggles (application-wide)
    "filter_trainer":           "Ctrl+F",
    "filter_rare_candy":        "Ctrl+R",
    "filter_tm_hm":             "Ctrl+T",
    "filter_vitamin":           "Ctrl+G",
    "filter_wild_pkmn":         "Ctrl+W",
    "filter_acquire_item":      "",
    "filter_purchase_item":     "",
    "filter_use_item":          "",
    "filter_sell_item":         "",
    "filter_hold_item":         "",
    "filter_levelup_move":      "",
    "filter_save":              "",
    "filter_heal":              "",
    "filter_blackout":          "",
    "filter_evolution":         "",
    "filter_notes":             "",
    "filter_common":            "Ctrl+A",
    "filter_reset":             "Ctrl+Shift+R",
}

# Human-readable labels for each shortcut action, grouped by category
SHORTCUT_LABELS = {
    # File menu
    "customize_dvs":            "Customize DVs",
    "new_route":                "New Route",
    "load_route":               "Load Route",
    "save_route":               "Save Route",
    "close_route":              "Close Route",
    "auto_load_recent":         "Auto-Load Most Recent Route",
    "export_notes":             "Export Notes",
    "screenshot_events":        "Screenshot Event List",
    "screenshot_battle":        "Screenshot Battle Summary",
    "screenshot_player":        "Screenshot Player Ranges",
    "screenshot_enemy":         "Screenshot Enemy Ranges",
    "open_image_folder":        "Open Image Folder",
    "config_font":              "Config Font",
    "custom_gens":              "Custom Gens",
    "app_config":               "App Config",
    "open_data_folder":         "Open Data Folder",
    "keyboard_shortcuts":       "Keyboard Shortcuts",
    # Events menu
    "undo":                     "Undo",
    "move_event_up":            "Move Event Up",
    "move_event_down":          "Move Event Down",
    "move_event_up_folder":     "Move Event Up To Next Folder",
    "move_event_down_folder":   "Move Event Down To Next Folder",
    "enable_disable":           "Enable/Disable",
    "toggle_highlight":         "Toggle Highlight",
    "delete_event":             "Delete Event",
    # Highlight menu
    "highlight_1":              "Highlight 1",
    "highlight_2":              "Highlight 2",
    "highlight_3":              "Highlight 3",
    "highlight_4":              "Highlight 4",
    "highlight_5":              "Highlight 5",
    "highlight_6":              "Highlight 6",
    "highlight_7":              "Highlight 7",
    "highlight_8":              "Highlight 8",
    "highlight_9":              "Highlight 9",
    # Folders menu
    "new_folder":               "New Folder",
    "rename_folder":            "Rename Current Folder",
    "split_folder":             "Split Folder",
    # Recording menu
    "toggle_recording":         "Toggle Recording",
    "final_trainers":           "Configure Final Trainers",
    # Battle Summary menu
    "toggle_move_highlights":   "Toggle Move Highlights",
    "toggle_fade_no_highlight": "Toggle Fade Moves Without Highlight",
    "toggle_test_moves":        "Toggle Test Moves",
    "candy_decrement":          "Decrement Pre-Fight Candies",
    "candy_increment":          "Increment Pre-Fight Candies",
    "toggle_player_strat":      "Toggle Player Highlight Strategy",
    "toggle_enemy_strat":       "Toggle Enemy Highlight Strategy",
    # QShortcut-only
    "delete_key":               "Delete Event (Delete Key)",
    "scroll_home":              "Scroll to Top",
    "scroll_end":               "Scroll to Bottom",
    "toggle_tabs":              "Toggle Event Tabs",
    "toggle_summary":           "Toggle Summary Window",
    "gym_1":                    "Select Gym Leader 1",
    "gym_2":                    "Select Gym Leader 2",
    "gym_3":                    "Select Gym Leader 3",
    "gym_4":                    "Select Gym Leader 4",
    "gym_5":                    "Select Gym Leader 5",
    "gym_6":                    "Select Gym Leader 6",
    "gym_7":                    "Select Gym Leader 7",
    "gym_8":                    "Select Gym Leader 8",
    "gym_blue":                 "Select Blue (Gen 2 / HGSS)",
    "e4_1":                     "Select Elite Four 1",
    "e4_2":                     "Select Elite Four 2",
    "e4_3":                     "Select Elite Four 3",
    "e4_4":                     "Select Elite Four 4",
    "e4_5":                     "Select Elite Four 5",
    "e4_6":                     "Select Elite Four/Champion 6",
    # Filter toggles
    "filter_trainer":           "Toggle Trainer Filter",
    "filter_rare_candy":        "Toggle Rare Candy Filter",
    "filter_tm_hm":             "Toggle TM/HM Filter",
    "filter_vitamin":           "Toggle Vitamin Filter",
    "filter_wild_pkmn":         "Toggle Wild Pkmn Filter",
    "filter_acquire_item":      "Toggle Acquire Item Filter",
    "filter_purchase_item":     "Toggle Purchase Item Filter",
    "filter_use_item":          "Toggle Use/Drop Item Filter",
    "filter_sell_item":         "Toggle Sell Item Filter",
    "filter_hold_item":         "Toggle Hold Item Filter",
    "filter_levelup_move":      "Toggle Levelup Move Filter",
    "filter_save":              "Toggle Game Save Filter",
    "filter_heal":              "Toggle Heal Filter",
    "filter_blackout":          "Toggle Blackout Filter",
    "filter_evolution":         "Toggle Evolution Filter",
    "filter_notes":             "Toggle Notes Only Filter",
    "filter_common":            "Toggle Common Filters",
    "filter_reset":             "Reset All Filters",
}

SHORTCUT_CATEGORIES = {
    "File":             ["customize_dvs", "new_route", "load_route", "save_route", "close_route",
                         "auto_load_recent", "export_notes", "screenshot_events", "screenshot_battle",
                         "screenshot_player", "screenshot_enemy", "open_image_folder",
                         "config_font", "custom_gens", "app_config", "open_data_folder", "keyboard_shortcuts"],
    "Events":           ["undo", "move_event_up", "move_event_down", "move_event_up_folder",
                         "move_event_down_folder", "enable_disable", "toggle_highlight", "delete_event"],
    "Highlight":        [f"highlight_{i}" for i in range(1, 10)],
    "Folders":          ["new_folder", "rename_folder", "split_folder"],
    "Recording":        ["toggle_recording", "final_trainers"],
    "Battle Summary":   ["toggle_move_highlights", "toggle_fade_no_highlight", "toggle_test_moves",
                         "candy_decrement", "candy_increment", "toggle_player_strat", "toggle_enemy_strat"],
    "Navigation":       ["delete_key", "scroll_home", "scroll_end", "toggle_tabs", "toggle_summary",
                         "gym_1", "gym_2", "gym_3", "gym_4", "gym_5", "gym_6", "gym_7", "gym_8",
                         "gym_blue",
                         "e4_1", "e4_2", "e4_3", "e4_4", "e4_5", "e4_6"],
    "Filters":          ["filter_trainer", "filter_rare_candy", "filter_tm_hm", "filter_vitamin",
                         "filter_wild_pkmn", "filter_acquire_item", "filter_purchase_item",
                         "filter_use_item", "filter_sell_item", "filter_hold_item",
                         "filter_levelup_move", "filter_save", "filter_heal",
                         "filter_blackout", "filter_evolution", "filter_notes",
                         "filter_common", "filter_reset"],
}


# Default "final trainer" lists per game version. Defeating any of these
# trainers while recording (with auto-stop enabled) will turn recording off
# automatically. Users can override these per-game in the Recording menu.
DEFAULT_FINAL_TRAINERS_PER_GAME = {
    "Red":        ["Rival3 Squirtle", "Rival3 Bulbasaur", "Rival3 Charmander"],
    "Blue":       ["Rival3 Squirtle", "Rival3 Bulbasaur", "Rival3 Charmander"],
    "Yellow":     ["Rival3 Jolteon", "Rival3 Flareon", "Rival3 Vaporeon"],
    "Gold":       ["Leader Red"],
    "Silver":     ["Leader Red"],
    "Crystal":    ["Leader Red"],
    "Ruby":       ["Champion Steven"],
    "Sapphire":   ["Champion Steven"],
    "Emerald":    ["Rival Steven"],
    "FireRed":    ["Champion Squirtle", "Champion Bulbasaur", "Champion Charmander"],
    "LeafGreen":  ["Champion Squirtle", "Champion Bulbasaur", "Champion Charmander"],
    "Diamond":    ["Champion Cynthia"],
    "Pearl":      ["Champion Cynthia"],
    "Platinum":   ["Champion Cynthia"],
    "HeartGold":  ["Pokemon Trainer Red"],
    "SoulSilver": ["Pokemon Trainer Red"],
}


class Config:
    # Bump this version any time the default color palette changes.
    # When the stored version doesn't match, colours are reset to defaults
    # (all other settings are preserved).
    COLOR_SCHEME_VERSION = 1
    COLOR_SCHEME_VERSION_KEY = "color_scheme_version"

    DEFAULT_SUCCESS = "#4ec97a"
    DEFAULT_WARNING = "#e8b730"
    DEFAULT_FAILURE = "#e05555"
    DEFAULT_DIVIDER = "#555555"
    DEFAULT_HEADER = "#e8a850"
    DEFAULT_PRIMARY = "#7cb8e0"
    DEFAULT_SECONDARY = "#999999"
    DEFAULT_CONTRAST = "#e0e0e0"
    DEFAULT_BACKGROUND = "#1e1e1e"
    DEFAULT_TEXT_COLOR = "#d4d4d4"
    DEFAULT_FONT_NAME = "Segoe UI"

    DEFAULT_PLAYER_HIGHLIGHT_STRATEGY = const.HIGHLIGHT_FASTEST_KILL
    DEFAULT_ENEMY_HIGHLIGHT_STRATEGY = const.HIGHLIGHT_FASTEST_KILL
    DEFAULT_CONSISTENT_THRESHOLD = 90
    DEFAULT_IGNORE_ACCURACY = False
    DEFAULT_FORCE_FULL_SEARCH = False
    DEFAULT_DAMAGE_SEARCH_DEPTH = 20
    DEFAULT_DEBUG_MODE = False
    DEFAULT_AUTO_SWITCH = True
    DEFAULT_NOTES_VISIBILITY = "when_space_allows"

    def __init__(self):
        self.reload()
    
    def reload(self):
        try:
            with open(const.GLOBAL_CONFIG_FILE, 'r') as f:
                raw = json.load(f)
        except Exception as e:
            raw = {}
        
        self._window_geometry = raw.get(const.CONFIG_WINDOW_GEOMETRY, "")
        self._user_data_dir = raw.get(const.USER_LOCATION_DATA_KEY, io_utils.get_default_user_data_dir())
        const.config_user_data_dir(self._user_data_dir)
        self._images_dir = raw.get(
            const.IMAGE_LOCATION_KEY,
            os.path.join(self._user_data_dir, const.SAVED_IMAGES_FOLDER_NAME),
        )

        # If the stored color scheme version is outdated (or missing),
        # ignore any persisted colours so the current dark-mode defaults win.
        colors_outdated = raw.get(self.COLOR_SCHEME_VERSION_KEY, 0) != self.COLOR_SCHEME_VERSION
        def _color(key, default):
            return default if colors_outdated else raw.get(key, default)

        self._success_color = _color(const.SUCCESS_COLOR_KEY, self.DEFAULT_SUCCESS)
        self._warning_color = _color(const.WARNING_COLOR_KEY, self.DEFAULT_WARNING)
        self._failure_color = _color(const.FAILURE_COLOR_KEY, self.DEFAULT_FAILURE)
        self._divider_color = _color(const.DIVIDER_COLOR_KEY, self.DEFAULT_DIVIDER)
        self._header_color = _color(const.HEADER_COLOR_KEY, self.DEFAULT_HEADER)
        self._primary_color = _color(const.PRIMARY_COLOR_KEY, self.DEFAULT_PRIMARY)
        self._secondary_color = _color(const.SECONDARY_COLOR_KEY, self.DEFAULT_SECONDARY)
        self._contrast_color = _color(const.CONTRAST_COLOR_KEY, self.DEFAULT_CONTRAST)
        self._background_color = _color(const.BACKGROUND_COLOR_KEY, self.DEFAULT_BACKGROUND)
        self._text_color = _color(const.TEXT_COLOR_KEY, self.DEFAULT_TEXT_COLOR)

        self._player_highlight_strategy = raw.get(const.PLAYER_HIGHLIGHT_STRATEGY_KEY, self.DEFAULT_PLAYER_HIGHLIGHT_STRATEGY)
        self._enemy_highlight_strategy = raw.get(const.ENEMY_HIGHLIGHT_STRATEGY_KEY, self.DEFAULT_ENEMY_HIGHLIGHT_STRATEGY)
        self._consistent_threshold = raw.get(const.CONSISTENT_HIGHLIGHT_THRESHOLD, self.DEFAULT_CONSISTENT_THRESHOLD)
        self._ignore_accuracy = raw.get(const.IGNORE_ACCURACY_IN_DAMAGE_CALCS, self.DEFAULT_IGNORE_ACCURACY)
        self._damage_search_depth = raw.get(const.DAMAGE_SEARCH_DEPTH, self.DEFAULT_DAMAGE_SEARCH_DEPTH)
        self._force_full_search = raw.get(const.FORCE_FULL_SEARCH, self.DEFAULT_FORCE_FULL_SEARCH)

        self._custom_font_name = raw.get(const.CUSTOM_FONT_NAME_KEY, self.DEFAULT_FONT_NAME)
        self._debug_mode = raw.get(const.DEBUG_MODE_KEY, self.DEFAULT_DEBUG_MODE)
        self._auto_switch = raw.get(const.AUTO_SWITCH_KEY, self.DEFAULT_AUTO_SWITCH)
        self._notes_visibility = raw.get(const.NOTES_VISIBILITY_KEY, self.DEFAULT_NOTES_VISIBILITY)

        # Qt-specific settings
        self._window_state = raw.get("window_state", "normal")
        self._auto_load_most_recent = raw.get("auto_load_most_recent_route", False)
        self._fade_folder_text = raw.get("fade_folder_text", False)
        self._highlight_branched_mandatory = raw.get("highlight_branched_mandatory", False)
        self._show_move_highlights = raw.get("show_move_highlights", True)
        self._show_legacy_controls = raw.get("show_legacy_controls", True)
        self._fade_moves_without_highlight = raw.get("fade_moves_without_highlight", False)
        self._test_moves_enabled = raw.get("test_moves_enabled", False)
        self._landing_search_filter = raw.get("landing_search_filter", "")
        self._landing_sort = raw.get("landing_sort", "recent")
        self._landing_game_filter = raw.get("landing_game_filter", "All")
        self._run_summary_docked = raw.get("run_summary_docked", True)
        self._color_major_battles = raw.get("color_major_battles", True)
        self._suppress_update_prompt = raw.get("suppress_update_prompt", False)
        self._notes_collapsed = raw.get("notes_collapsed", False)
        # Splitter fractions: left-panel width / total splitter width, per tab.
        # None means "no saved preference -- use the natural default".
        self._pre_state_left_fraction = raw.get("pre_state_left_fraction", None)
        self._battle_summary_left_fraction = raw.get("battle_summary_left_fraction", None)
        self._highlight_colors = {}
        self._fight_category_colors = {}
        for cat in ["rival", "gym_leader", "elite_four", "champion", "post_game", "boss", "team_leader"]:
            key = f"fight_category_color_{cat}"
            if key in raw:
                self._fight_category_colors[cat] = raw[key]
        for i in range(1, 10):
            key = f"highlight_color_{i}"
            if key in raw:
                self._highlight_colors[i] = raw[key]

        # Keyboard shortcuts -- only store overrides (diff from defaults)
        self._shortcut_overrides = raw.get("keyboard_shortcuts", {})

        # Final trainers per game version (game_version -> list of trainer names)
        # When recording, defeating any of these trainers automatically stops recording.
        raw_final = raw.get("final_trainers_per_game", {})
        self._final_trainers_per_game = {
            str(k): list(v) for k, v in raw_final.items() if isinstance(v, (list, tuple))
        }
        # Whether the "final trainer auto-stop" logic is enabled at all
        self._recording_auto_stop_enabled = bool(raw.get("recording_auto_stop_enabled", True))

        # Persist the reset so the user's config file is stamped with the
        # current color scheme version (avoids resetting again next launch).
        if colors_outdated and raw:
            self._save()
    
    def _save(self):
        if not os.path.exists(const.GLOBAL_CONFIG_DIR):
            os.makedirs(const.GLOBAL_CONFIG_DIR)

        data = {
                self.COLOR_SCHEME_VERSION_KEY: self.COLOR_SCHEME_VERSION,
                const.CONFIG_WINDOW_GEOMETRY: self._window_geometry,
                const.USER_LOCATION_DATA_KEY: self._user_data_dir,
                const.SUCCESS_COLOR_KEY: self._success_color,
                const.WARNING_COLOR_KEY: self._warning_color,
                const.FAILURE_COLOR_KEY: self._failure_color,
                const.DIVIDER_COLOR_KEY: self._divider_color,
                const.HEADER_COLOR_KEY: self._header_color,
                const.PRIMARY_COLOR_KEY: self._primary_color,
                const.SECONDARY_COLOR_KEY: self._secondary_color,
                const.CONTRAST_COLOR_KEY: self._contrast_color,
                const.BACKGROUND_COLOR_KEY: self._background_color,
                const.TEXT_COLOR_KEY: self._text_color,
                const.CUSTOM_FONT_NAME_KEY: self._custom_font_name,
                const.PLAYER_HIGHLIGHT_STRATEGY_KEY: self._player_highlight_strategy,
                const.ENEMY_HIGHLIGHT_STRATEGY_KEY: self._enemy_highlight_strategy,
                const.CONSISTENT_HIGHLIGHT_THRESHOLD: self._consistent_threshold,
                const.IGNORE_ACCURACY_IN_DAMAGE_CALCS: self._ignore_accuracy,
                const.DAMAGE_SEARCH_DEPTH: self._damage_search_depth,
                const.FORCE_FULL_SEARCH: self._force_full_search,
                const.DEBUG_MODE_KEY: self._debug_mode,
                const.AUTO_SWITCH_KEY: self._auto_switch,
                const.NOTES_VISIBILITY_KEY: self._notes_visibility,
                # Qt-specific settings
                "window_state": getattr(self, '_window_state', 'normal'),
                "auto_load_most_recent_route": getattr(self, '_auto_load_most_recent', False),
                "fade_folder_text": getattr(self, '_fade_folder_text', False),
                "highlight_branched_mandatory": getattr(self, '_highlight_branched_mandatory', False),
                "show_move_highlights": getattr(self, '_show_move_highlights', True),
                "show_legacy_controls": getattr(self, '_show_legacy_controls', True),
                "fade_moves_without_highlight": getattr(self, '_fade_moves_without_highlight', False),
                "test_moves_enabled": getattr(self, '_test_moves_enabled', False),
                "landing_search_filter": getattr(self, '_landing_search_filter', ''),
                "landing_sort": getattr(self, '_landing_sort', 'recent'),
                "landing_game_filter": getattr(self, '_landing_game_filter', 'All'),
                "run_summary_docked": getattr(self, '_run_summary_docked', True),
                "color_major_battles": getattr(self, '_color_major_battles', True),
                "suppress_update_prompt": getattr(self, '_suppress_update_prompt', False),
                "notes_collapsed": getattr(self, '_notes_collapsed', False),
                "pre_state_left_fraction": getattr(self, '_pre_state_left_fraction', None),
                "battle_summary_left_fraction": getattr(self, '_battle_summary_left_fraction', None),
        }
        # Save highlight colors
        for i in range(1, 10):
            colors = getattr(self, '_highlight_colors', {})
            if i in colors:
                data[f"highlight_color_{i}"] = colors[i]
        # Save fight category colors
        for cat, color in getattr(self, '_fight_category_colors', {}).items():
            data[f"fight_category_color_{cat}"] = color
        # Save keyboard shortcut overrides
        if self._shortcut_overrides:
            data["keyboard_shortcuts"] = self._shortcut_overrides
        # Save final trainers per game version
        final_trainers = getattr(self, '_final_trainers_per_game', {})
        if final_trainers:
            data["final_trainers_per_game"] = final_trainers
        # Save recording auto-stop toggle (only when non-default to keep file tidy)
        if not getattr(self, '_recording_auto_stop_enabled', True):
            data["recording_auto_stop_enabled"] = False

        with open(const.GLOBAL_CONFIG_FILE, 'w') as f:
            json.dump(data, f, indent=4)
    
    def set_window_geometry(self, new_geometry):
        if new_geometry != self._window_geometry:
            self._window_geometry = new_geometry
            self._save()

    def get_window_geometry(self):
        return self._window_geometry
    
    def get_user_data_dir(self):
        return self._user_data_dir
    
    def set_user_data_dir(self, new_dir):
        self._user_data_dir = new_dir
        const.config_user_data_dir(new_dir)
        self._save()
    
    def set_success_color(self, new_color):
        self._success_color = new_color
        self._save()
    
    def set_warning_color(self, new_color):
        self._warning_color = new_color
        self._save()
    
    def set_failure_color(self, new_color):
        self._failure_color = new_color
        self._save()
    
    def set_divider_color(self, new_color):
        self._divider_color = new_color
        self._save()
    
    def set_header_color(self, new_color):
        self._header_color = new_color
        self._save()
    
    def set_primary_color(self, new_color):
        self._primary_color = new_color
        self._save()
    
    def set_secondary_color(self, new_color):
        self._secondary_color = new_color
        self._save()
    
    def set_contrast_color(self, new_color):
        self._contrast_color = new_color
        self._save()
    
    def set_background_color(self, new_color):
        self._background_color = new_color
        self._save()

    def set_text_color(self, new_color):
        self._text_color = new_color
        self._save()

    def set_player_highlight_strategy(self, strat):
        self._player_highlight_strategy = strat
        self._save()

    def set_enemy_highlight_strategy(self, strat):
        self._enemy_highlight_strategy = strat
        self._save()

    def set_consistent_threshold(self, threshold):
        self._consistent_threshold = threshold
        self._save()

    def set_ignore_accuracy(self, do_include):
        self._ignore_accuracy = do_include
        self._save()

    def set_damage_search_depth(self, depth):
        self._damage_search_depth = depth
        self._save()

    def set_force_full_search(self, do_force):
        self._force_full_search = do_force
        self._save()

    def set_debug_mode(self, is_debug_mode):
        self._debug_mode = is_debug_mode
        self._save()

    def set_auto_switch(self, do_auto_switch):
        self._auto_switch = do_auto_switch
        self._save()

    def get_pre_state_left_fraction(self):
        return getattr(self, '_pre_state_left_fraction', None)

    def set_pre_state_left_fraction(self, val):
        self._pre_state_left_fraction = val
        self._save()

    def get_battle_summary_left_fraction(self):
        return getattr(self, '_battle_summary_left_fraction', None)

    def set_battle_summary_left_fraction(self, val):
        self._battle_summary_left_fraction = val
        self._save()

    def set_notes_visibility_in_battle_summary(self, are_notes_visible):
        if isinstance(are_notes_visible, bool):
            self._notes_visibility = "always" if are_notes_visible else "never"
        else:
            self._notes_visibility = are_notes_visible
        self._save()

    def set_images_dir(self, images_dir):
        self._images_dir = images_dir
        self._save()

    def get_success_color(self):
        return self._success_color

    def get_warning_color(self):
        return self._warning_color

    def get_failure_color(self):
        return self._failure_color

    def get_divider_color(self):
        return self._divider_color

    def get_header_color(self):
        return self._header_color

    def get_primary_color(self):
        return self._primary_color

    def get_secondary_color(self):
        return self._secondary_color

    def get_contrast_color(self):
        return self._contrast_color

    def get_background_color(self):
        return self._background_color
    
    def get_text_color(self):
        return self._text_color
    
    def get_player_highlight_strategy(self):
        result = self._player_highlight_strategy
        if result not in const.ALL_HIGHLIGHT_STRATS:
            result = const.HIGHLIGHT_NONE
        return result
    
    def get_enemy_highlight_strategy(self):
        result = self._enemy_highlight_strategy
        if result not in const.ALL_HIGHLIGHT_STRATS:
            result = const.HIGHLIGHT_NONE
        return result
    
    def get_consistent_threshold(self):
        result = self._consistent_threshold
        if not isinstance(result, int) or result < 0 or result > 99:
            result = self.DEFAULT_CONSISTENT_THRESHOLD
        return result
    
    def get_damage_search_depth(self):
        result = self._damage_search_depth
        if not isinstance(result, int) or result < 0:
            result = self.DEFAULT_DAMAGE_SEARCH_DEPTH
        return result
    
    def do_force_full_search(self):
        return self._force_full_search
    
    def do_ignore_accuracy(self):
        return self._ignore_accuracy
    
    def is_debug_mode(self):
        return self._debug_mode
    
    def do_auto_switch(self):
        return self._auto_switch
    
    def are_notes_visible_in_battle_summary(self):
        return self._notes_visibility != "never"

    def get_notes_visibility_mode(self):
        """Returns 'when_space_allows', 'always', or 'never'."""
        mode = self._notes_visibility
        if mode in ("when_space_allows", "always", "never"):
            return mode
        # Legacy boolean support
        if mode is True or mode == "True":
            return "always"
        if mode is False or mode == "False":
            return "never"
        return "when_space_allows"

    def set_notes_visibility_mode(self, mode):
        """Set notes visibility mode: 'when_space_allows', 'always', or 'never'."""
        self._notes_visibility = mode
        self._save()

    def get_notes_collapsed(self):
        return self._notes_collapsed

    def set_notes_collapsed(self, collapsed):
        self._notes_collapsed = collapsed
        self._save()

    def reset_all_colors(self):
        self._success_color = self.DEFAULT_SUCCESS
        self._warning_color = self.DEFAULT_WARNING
        self._failure_color = self.DEFAULT_FAILURE
        self._divider_color = self.DEFAULT_DIVIDER
        self._header_color = self.DEFAULT_HEADER
        self._primary_color = self.DEFAULT_PRIMARY
        self._secondary_color = self.DEFAULT_SECONDARY
        self._contrast_color = self.DEFAULT_CONTRAST
        self._background_color = self.DEFAULT_BACKGROUND
        self._text_color = self.DEFAULT_TEXT_COLOR
        self._save()
    
    def set_custom_font_name(self, new_name):
        self._custom_font_name = new_name
        self._save()
    
    def get_custom_font_name(self):
        return self._custom_font_name
    
    def get_images_dir(self):
        return self._images_dir

    # --- Window state (zoomed/normal/iconic) ---
    def get_window_state(self):
        return getattr(self, '_window_state', 'normal')

    def set_window_state(self, state):
        self._window_state = state
        self._save()

    # --- Auto-load most recent route ---
    def get_auto_load_most_recent_route(self):
        return bool(getattr(self, '_auto_load_most_recent', False))

    def set_auto_load_most_recent_route(self, val):
        self._auto_load_most_recent = val
        self._save()

    # --- Highlight colors (1-9) ---
    def get_highlight_color(self, idx):
        colors = getattr(self, '_highlight_colors', {})
        defaults = {
            1: "#903858", 2: "#388038", 3: "#887828",
            4: "#983030", 5: "#787878", 6: "#5028a0",
            7: "#c83838", 8: "#389080", 9: "#606060",
        }
        return colors.get(idx, defaults.get(idx, "#444444"))

    def set_highlight_color(self, idx, color):
        if not hasattr(self, '_highlight_colors'):
            self._highlight_colors = {}
        self._highlight_colors[idx] = color
        self._save()

    # --- Fight category colors ---
    FIGHT_CATEGORY_COLOR_DEFAULTS = {
        "rival": "#12196b",
        "gym_leader": "#0d0d0d",
        "elite_four": "#38084d",
        "champion": "#054d3f",
        "post_game": "#0c2a0c",
        "boss": "#808080",
        "team_leader": "#702060",
    }

    def get_fight_category_color(self, category):
        colors = getattr(self, '_fight_category_colors', {})
        return colors.get(category, self.FIGHT_CATEGORY_COLOR_DEFAULTS.get(category, "#1f1f1f"))

    def set_fight_category_color(self, category, color):
        if not hasattr(self, '_fight_category_colors'):
            self._fight_category_colors = {}
        self._fight_category_colors[category] = color
        self._save()

    def get_color_major_battles(self):
        return getattr(self, '_color_major_battles', True)

    def set_color_major_battles(self, val):
        self._color_major_battles = val
        self._save()

    # --- Fade folder text ---
    def get_fade_folder_text(self):
        return getattr(self, '_fade_folder_text', False)

    def set_fade_folder_text(self, val):
        self._fade_folder_text = val
        self._save()

    # --- Highlight branched mandatory ---
    def get_highlight_branched_mandatory(self):
        return getattr(self, '_highlight_branched_mandatory', False)

    def set_highlight_branched_mandatory(self, val):
        self._highlight_branched_mandatory = val
        self._save()

    # --- Show move highlights ---
    def get_show_move_highlights(self):
        return getattr(self, '_show_move_highlights', True)

    def set_show_move_highlights(self, val):
        self._show_move_highlights = val
        self._save()

    # --- Show legacy battle-summary controls ---
    def get_show_legacy_controls(self):
        return getattr(self, '_show_legacy_controls', True)

    def set_show_legacy_controls(self, val):
        self._show_legacy_controls = bool(val)
        self._save()

    # --- Fade moves without highlight ---
    def get_fade_moves_without_highlight(self):
        return getattr(self, '_fade_moves_without_highlight', False)

    def set_fade_moves_without_highlight(self, val):
        self._fade_moves_without_highlight = val
        self._save()

    # --- Test moves ---
    def get_test_moves_enabled(self):
        return getattr(self, '_test_moves_enabled', False)

    def set_test_moves_enabled(self, val):
        self._test_moves_enabled = val
        self._save()

    # --- Landing page search filter ---
    def get_landing_page_search_filter(self):
        return getattr(self, '_landing_search_filter', '')

    def set_landing_page_search_filter(self, val):
        self._landing_search_filter = val
        self._save()

    # --- Custom image path ---
    def set_custom_image_path(self, path):
        self._custom_image_path = path

    def get_custom_image_path(self):
        return getattr(self, '_custom_image_path', '')

    # --- Landing page sort ---
    def get_landing_page_sort(self):
        return getattr(self, '_landing_sort', 'recent')

    def set_landing_page_sort(self, val):
        self._landing_sort = val
        self._save()

    def get_landing_page_game_filter(self):
        return getattr(self, '_landing_game_filter', 'All')

    def set_landing_page_game_filter(self, val):
        self._landing_game_filter = val
        self._save()

    # --- Suppress update prompt ---
    def get_suppress_update_prompt(self):
        return getattr(self, '_suppress_update_prompt', False)

    def set_suppress_update_prompt(self, val):
        self._suppress_update_prompt = val
        self._save()

    # --- Run summary docked ---
    def get_run_summary_docked(self):
        return getattr(self, '_run_summary_docked', True)

    def set_run_summary_docked(self, val):
        self._run_summary_docked = val
        self._save()

    # --- Keyboard shortcuts ---
    def get_shortcut(self, action_id):
        """Return the key sequence string for *action_id*, respecting overrides."""
        return self._shortcut_overrides.get(action_id, DEFAULT_SHORTCUTS.get(action_id, ""))

    def get_all_shortcuts(self):
        """Return a merged dict of all shortcuts (defaults + overrides)."""
        merged = copy.copy(DEFAULT_SHORTCUTS)
        merged.update(self._shortcut_overrides)
        return merged

    def set_shortcut(self, action_id, key_sequence):
        """Set a custom shortcut. If it matches the default, remove the override."""
        default = DEFAULT_SHORTCUTS.get(action_id, "")
        if key_sequence == default:
            self._shortcut_overrides.pop(action_id, None)
        else:
            self._shortcut_overrides[action_id] = key_sequence
        self._save()

    def reset_shortcut(self, action_id):
        """Reset a single shortcut to its default."""
        self._shortcut_overrides.pop(action_id, None)
        self._save()

    def reset_all_shortcuts(self):
        """Reset all shortcuts to defaults."""
        self._shortcut_overrides.clear()
        self._save()

    def is_shortcut_customized(self, action_id):
        return action_id in self._shortcut_overrides

    def export_shortcuts(self, file_path):
        """Export the current shortcut profile to a JSON file."""
        data = self.get_all_shortcuts()
        with open(file_path, 'w') as f:
            json.dump(data, f, indent=4)

    # --- Final trainers per game version ---
    def get_final_trainers(self, game_version):
        """Return the list of trainer names configured as 'final trainers' for the given game version.
        Defeating any of these trainers while recording will automatically stop recording.
        Falls back to DEFAULT_FINAL_TRAINERS_PER_GAME when the user has not explicitly set
        a list for this game (an explicit empty list is honored as 'user disabled all defaults')."""
        per_game = getattr(self, '_final_trainers_per_game', {})
        if game_version in per_game:
            return list(per_game[game_version])
        return list(DEFAULT_FINAL_TRAINERS_PER_GAME.get(game_version, []))

    def set_final_trainers(self, game_version, trainer_list):
        """Replace the final-trainer list for a given game version. Always persists,
        even when the list is empty, so that the user clearing every default is
        remembered (otherwise defaults would re-appear next launch)."""
        if not hasattr(self, '_final_trainers_per_game'):
            self._final_trainers_per_game = {}
        self._final_trainers_per_game[game_version] = [str(t) for t in trainer_list]
        self._save()

    def reset_final_trainers(self, game_version):
        """Forget any user override for this game version, restoring built-in defaults."""
        per_game = getattr(self, '_final_trainers_per_game', None)
        if per_game and game_version in per_game:
            del per_game[game_version]
            self._save()

    # --- Recording auto-stop toggle ---
    def get_recording_auto_stop_enabled(self):
        """Whether recording should automatically stop when a configured 'final trainer' is defeated."""
        return bool(getattr(self, '_recording_auto_stop_enabled', True))

    def set_recording_auto_stop_enabled(self, enabled):
        self._recording_auto_stop_enabled = bool(enabled)
        self._save()

    def import_shortcuts(self, file_path):
        """Import a shortcut profile from a JSON file. Only stores diffs from defaults."""
        with open(file_path, 'r') as f:
            data = json.load(f)
        self._shortcut_overrides = {}
        for action_id, key_seq in data.items():
            if action_id in DEFAULT_SHORTCUTS and key_seq != DEFAULT_SHORTCUTS[action_id]:
                self._shortcut_overrides[action_id] = key_seq
        self._save()

config = Config()