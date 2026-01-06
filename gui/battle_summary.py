import tkinter as tk
import tkinter.ttk as ttk
import tkinter.font
from typing import Dict, List
import logging
from controllers.battle_summary_controller import BattleSummaryController, MoveRenderInfo

from gui import custom_components
from gui.popups.battle_config_popup import BattleConfigWindow
from pkmn import damage_calc, universal_data_objects
from routing import full_route_state
from routing import route_events
from utils.config_manager import config
from utils.constants import const
from pkmn.gen_factory import current_gen_info


def _blend_color(color1: str, color2: str, alpha: float) -> str:
    """Blend two hex colors with the given alpha (0.0 to 1.0).
    alpha=0.0 means fully color2, alpha=1.0 means fully color1.
    Returns a hex color string."""
    def hex_to_rgb(hex_color: str) -> tuple:
        """Convert hex color to RGB tuple."""
        hex_color = hex_color.lstrip('#')
        if len(hex_color) == 3:
            hex_color = ''.join([c*2 for c in hex_color])
        return tuple(int(hex_color[i:i+2], 16) for i in (0, 2, 4))
    
    def rgb_to_hex(rgb: tuple) -> str:
        """Convert RGB tuple to hex color string."""
        return '#{:02x}{:02x}{:02x}'.format(int(rgb[0]), int(rgb[1]), int(rgb[2]))
    
    # Convert Tcl objects to strings if needed (always convert to string)
    color1_str = str(color1) if color1 else ""
    color2_str = str(color2) if color2 else ""
    
    # Handle empty strings or invalid colors
    if not color1_str or not color1_str.startswith('#'):
        return color2_str if color2_str else "#000000"
    if not color2_str or not color2_str.startswith('#'):
        return color1_str if color1_str else "#000000"
    
    try:
        rgb1 = hex_to_rgb(color1_str)
        rgb2 = hex_to_rgb(color2_str)
        # Blend: result = color1 * alpha + color2 * (1 - alpha)
        blended = tuple(
            int(rgb1[i] * alpha + rgb2[i] * (1 - alpha))
            for i in range(3)
        )
        return rgb_to_hex(blended)
    except Exception:
        # If blending fails, return color2 (background)
        return str(color2) if color2 else "#000000"

logger = logging.getLogger(__name__)


class BattleSummary(ttk.Frame):
    def __init__(self, controller:BattleSummaryController, *args, **kwargs):
        self._controller = controller
        super().__init__(*args, **kwargs)

        self.columnconfigure(0, weight=1)

        # these are matched lists, with the solo mon being updated for each enemy pkmn, in case of level-ups
        self._enemy_pkmn:List[universal_data_objects.EnemyPkmn] = None
        self._solo_pkmn:List[universal_data_objects.EnemyPkmn] = None
        self._source_state:full_route_state.RouteState = None
        self._source_event_group:route_events.EventGroup = None
        self._player_stage_modifiers:universal_data_objects.StageModifiers = universal_data_objects.StageModifiers()
        self._enemy_stage_modifiers:universal_data_objects.StageModifiers = universal_data_objects.StageModifiers()
        self._mimic_selection = ""
        self._custom_move_data = None
        self._loading = False

        # Create canvas and scrollbar for scrolling when notes are always shown
        self._canvas = tk.Canvas(self, highlightthickness=0)
        self._scrollbar = ttk.Scrollbar(self, orient="vertical", command=self._canvas.yview)
        self._canvas.configure(yscrollcommand=self._scrollbar.set)
        
        self._base_frame = ttk.Frame(self._canvas)
        self._canvas_window = self._canvas.create_window((0, 0), window=self._base_frame, anchor="nw")
        
        # Initially grid without scrollbar
        self._canvas.grid(row=0, column=0, sticky=tk.NSEW)
        self._scrollbar.grid(row=0, column=1, sticky=tk.NS)
        self._scrollbar.grid_remove()  # Hide scrollbar initially
        
        self.columnconfigure(0, weight=1)
        self.rowconfigure(0, weight=1)
        
        self._base_frame.columnconfigure(0, weight=1)
        
        # Bind canvas resize to update scroll region
        self._canvas.bind('<Configure>', self._on_canvas_configure)
        self._base_frame.bind('<Configure>', self._on_frame_configure)
        
        # Bind mouse wheel scrolling to canvas
        self._canvas.bind('<MouseWheel>', self._on_mousewheel)
        self._canvas.bind('<Button-4>', self._on_mousewheel)  # Linux scroll up
        self._canvas.bind('<Button-5>', self._on_mousewheel)  # Linux scroll down

        self._top_bar = ttk.Frame(self._base_frame)
        self._top_bar.grid(row=0, column=0, sticky=tk.EW)

        self._setup_half = ttk.Frame(self._top_bar)
        self._setup_half.grid(row=0, column=0, sticky=tk.EW)
        self._weather_half = ttk.Frame(self._top_bar)
        self._weather_half.grid(row=0, column=1, sticky=tk.EW)

        self._top_bar.columnconfigure(0, weight=1)

        self.setup_moves = SetupMovesSummary(self._setup_half, callback=self._player_setup_move_callback)
        self.setup_moves.grid(row=0, column=0, pady=(0, 2))

        self.transform_checkbox = custom_components.CheckboxLabel(self._setup_half, text="Transform:", toggle_command=self._player_transform_callback, flip=True)
        self.transform_checkbox.grid(row=0, column=1, sticky=tk.EW)

        self.enemy_setup_moves = SetupMovesSummary(self._setup_half, callback=self._enemy_setup_move_callback, is_player=False)
        self.enemy_setup_moves.grid(row=1, column=0, sticky=tk.EW)

        self.config_button = custom_components.SimpleButton(self._weather_half, text="Configure/Help", command=self._launch_config_popup)
        self.config_button.grid(row=0, column=0, sticky=tk.EW, padx=10, pady=(0, 2))

        self.double_label = ttk.Label(self._weather_half, text="Single Battle")
        self.double_label.grid(row=1, column=0, sticky=tk.EW, padx=10, pady=(0, 2))

        self.weather_status = WeatherSummary(self._weather_half, callback=self._weather_callback)
        self.weather_status.grid(row=0, column=1, sticky=tk.EW, padx=2, pady=(0, 2))

        self.candy_summary = PrefightCandySummary(self._weather_half, callback=self._candy_callback)
        self.candy_summary.grid(row=1, column=1, sticky=tk.EW, padx=2, pady=(0, 2))

        self._mon_pairs:List[MonPairSummary] = []
        self._did_draw_mon_pairs:List[bool] = []

        for idx in range(6):
            self._mon_pairs.append(MonPairSummary(self._controller, idx, self._base_frame))
            self._did_draw_mon_pairs.append(False)
        
        self.error_message = tk.Label(self, text="Select a battle to see damage calculations")
        self.should_render = False
        self.bind(self._controller.register_refresh(self), self._on_full_refresh)
        self.set_team(None)
    
    def configure_weather(self, possible_weather_vals):
        self.weather_status.configure_weather(possible_weather_vals)

    def configure_setup_moves(self, possible_setup_moves):
        self.setup_moves.configure_moves(possible_setup_moves)
        self.enemy_setup_moves.configure_moves(possible_setup_moves)
    
    def _on_canvas_configure(self, event):
        """Update canvas scroll region when canvas is resized."""
        self._canvas.itemconfig(self._canvas_window, width=event.width)
        self._update_scroll_region()
    
    def _on_frame_configure(self, event):
        """Update canvas scroll region when frame content changes."""
        self._update_scroll_region()
    
    def _update_scroll_region(self):
        """Update the scrollable region of the canvas."""
        self._canvas.update_idletasks()
        bbox = self._canvas.bbox("all")
        if bbox:
            self._canvas.config(scrollregion=bbox)
    
    def _on_mousewheel(self, event):
        """Handle mouse wheel scrolling on canvas."""
        # Only scroll if scrollbar is visible
        if self._scrollbar.winfo_viewable():
            # Windows and Mac
            if event.delta:
                self._canvas.yview_scroll(int(-1 * (event.delta / 120)), "units")
            # Linux
            elif event.num == 4:
                self._canvas.yview_scroll(-1, "units")
            elif event.num == 5:
                self._canvas.yview_scroll(1, "units")
    
    def enable_scrollbar(self):
        """Enable scrollbar for battle summary content only if content exceeds visible area."""
        self._update_scroll_region()
        # Check if scrolling is actually needed - use after_idle to ensure layout is complete
        self._canvas.after_idle(self._check_and_enable_scrollbar)
    
    def _check_and_enable_scrollbar(self):
        """Check if scrollbar is needed and enable/disable accordingly."""
        self._canvas.update_idletasks()
        canvas_height = self._canvas.winfo_height()
        bbox = self._canvas.bbox("all")
        
        if bbox and canvas_height > 0:
            content_height = bbox[3] - bbox[1]  # bottom - top
            # Only show scrollbar if content is taller than canvas (with small threshold to avoid flicker)
            if content_height > canvas_height + 5:  # 5px threshold
                self._scrollbar.grid(row=0, column=1, sticky=tk.NS)
                self.columnconfigure(1, weight=0)  # Don't expand scrollbar column
            else:
                self._scrollbar.grid_remove()
                # Reset scroll position to top when scrollbar is hidden
                self._canvas.yview_moveto(0.0)
        else:
            self._scrollbar.grid_remove()
            self._canvas.yview_moveto(0.0)
    
    def disable_scrollbar(self):
        """Disable scrollbar for battle summary content."""
        self._scrollbar.grid_remove()
        self._update_scroll_region()
    
    def hide_contents(self):
        self.should_render = False
        self._canvas.grid_forget()
        self._scrollbar.grid_remove()
    
    def show_contents(self):
        self.should_render = True
        self._canvas.grid_forget()
        self._on_full_refresh()
        self._canvas.grid(row=0, column=0, sticky=tk.NSEW)
        self._update_scroll_region()

    def _launch_config_popup(self, *args, **kwargs):
        BattleConfigWindow(self.winfo_toplevel(), battle_controller=self._controller)

    def _weather_callback(self, *args, **kwargs):
        self._controller.update_weather(self.weather_status.get_weather())

    def _candy_callback(self, *args, **kwargs):
        self._controller.update_prefight_candies(self.candy_summary.get_prefight_candy_count())

    def _player_setup_move_callback(self, *args, **kwargs):
        self._controller.update_player_setup_moves(self.setup_moves._move_list.copy())

    def _player_transform_callback(self, *args, **kwargs):
        if not self._loading:
            self._controller.update_player_transform(self.transform_checkbox.is_checked())

    def _enemy_setup_move_callback(self, *args, **kwargs):
        self._controller.update_enemy_setup_moves(self.enemy_setup_moves._move_list.copy())

    def set_team(
        self,
        enemy_pkmn:List[universal_data_objects.EnemyPkmn],
        cur_state:full_route_state.RouteState=None,
        event_group:route_events.EventGroup=None,
    ):
        if event_group is not None:
            self._controller.load_from_event(event_group)
        elif cur_state is not None and enemy_pkmn is not None:
            self._controller.load_from_state(cur_state, enemy_pkmn)
        else:
            self._controller.load_empty()

    def _on_full_refresh(self, *args, **kwargs):
        if not self.should_render:
            return

        self._loading = True
        self.candy_summary.set_candy_count(self._controller.get_prefight_candy_count())
        if not self._controller.can_support_prefight_candies():
            self.candy_summary.disable()
        else:
            self.candy_summary.enable()

        if self._controller.is_double_battle():
            self.double_label.configure(text="Double Battle")
        else:
            self.double_label.configure(text="Single Battle")

        self.transform_checkbox.set_checked(self._controller.is_player_transformed())
        self.weather_status.set_weather(self._controller.get_weather())
        self.setup_moves.set_move_list(self._controller.get_player_setup_moves())
        self.enemy_setup_moves.set_move_list(self._controller.get_enemy_setup_moves())
        for idx in range(6):
            player_info = self._controller.get_pkmn_info(idx, True)
            enemy_info = self._controller.get_pkmn_info(idx, False)

            if player_info is None and enemy_info is None:
                if self._did_draw_mon_pairs[idx]:
                    self._mon_pairs[idx].grid_forget()
                    self._did_draw_mon_pairs[idx] = False
            else:
                if not self._did_draw_mon_pairs[idx]:
                    self._mon_pairs[idx].grid(row=idx + 2, column=0, sticky=tk.EW)
                    self._did_draw_mon_pairs[idx] = True
                self._mon_pairs[idx].update_rendering()

        self._loading = False
        # Update scroll region after rendering
        self._update_scroll_region()
    
    def get_content_bounding_box(self):
        """Get bounding box that includes only visible content, excluding blank space at bottom."""
        # Ensure widgets are updated before measuring
        self.update_idletasks()
        
        leftmost = self.winfo_rootx()
        topmost = self.winfo_rooty()
        rightmost = leftmost + self.winfo_width()
        
        # Start with the top bar as the initial bottom
        bottommost = self._top_bar.winfo_rooty() + self._top_bar.winfo_height()
        
        # Check each mon pair to find the last visible one
        for idx in range(5, -1, -1):  # Check from last to first
            if self._did_draw_mon_pairs[idx]:
                mon_pair = self._mon_pairs[idx]
                # Get the bottom of this mon pair in root coordinates
                mon_pair_bottom = mon_pair.winfo_rooty() + mon_pair.winfo_height()
                bottommost = max(bottommost, mon_pair_bottom)
                break
        
        return (leftmost, topmost, rightmost, bottommost)
    
    def get_player_ranges_bounding_box(self):
        """Get bounding box for player damage ranges (left side only)."""
        self.update_idletasks()
        
        # Find the leftmost and rightmost bounds of player columns (0-3)
        leftmost = None
        rightmost = None
        topmost = None
        bottommost = None
        
        # Check each visible mon pair to find the bounds
        for idx in range(6):
            if self._did_draw_mon_pairs[idx]:
                mon_pair = self._mon_pairs[idx]
                # Player side is columns 0-3 (left_mon_label_frame spans 0-3, moves are in 0-3)
                # Get the left edge from the left_mon_label_frame
                left_edge = mon_pair.left_mon_label_frame.winfo_rootx()
                # Get the right edge from the divider (which separates player and enemy sides)
                right_edge = mon_pair.divider.winfo_rootx()
                
                if leftmost is None or left_edge < leftmost:
                    leftmost = left_edge
                if rightmost is None or right_edge > rightmost:
                    rightmost = right_edge
                
                # Get top from first visible mon pair
                if topmost is None:
                    topmost = mon_pair.winfo_rooty()
                
                # Update bottommost to include this mon pair
                mon_pair_bottom = mon_pair.winfo_rooty() + mon_pair.winfo_height()
                if bottommost is None or mon_pair_bottom > bottommost:
                    bottommost = mon_pair_bottom
        
        # If no mon pairs are visible, use the frame bounds for left half
        if leftmost is None:
            leftmost = self.winfo_rootx()
            rightmost = self.winfo_rootx() + self.winfo_width() // 2
            topmost = self.winfo_rooty()
            bottommost = self.winfo_rooty() + self.winfo_height()
        
        # Contract by 2 pixels on the right side
        rightmost = rightmost - 2
        
        return (leftmost, topmost, rightmost, bottommost)
    
    def get_enemy_ranges_bounding_box(self):
        """Get bounding box for enemy damage ranges (right side only)."""
        self.update_idletasks()
        
        # Find the leftmost and rightmost bounds of enemy columns (5-8)
        leftmost = None
        rightmost = None
        topmost = None
        bottommost = None
        
        # Check each visible mon pair to find the bounds
        for idx in range(6):
            if self._did_draw_mon_pairs[idx]:
                mon_pair = self._mon_pairs[idx]
                # Enemy side is columns 5-8 (right_mon_label_frame spans 5-8, moves are in 5-8)
                # Get the left edge from just after the divider (column 4)
                left_edge = mon_pair.divider.winfo_rootx() + mon_pair.divider.winfo_width()
                # Get the right edge from the right_mon_label_frame
                right_edge = mon_pair.right_mon_label_frame.winfo_rootx() + mon_pair.right_mon_label_frame.winfo_width()
                
                if leftmost is None or left_edge < leftmost:
                    leftmost = left_edge
                if rightmost is None or right_edge > rightmost:
                    rightmost = right_edge
                
                # Get top from first visible mon pair
                if topmost is None:
                    topmost = mon_pair.winfo_rooty()
                
                # Update bottommost to include this mon pair
                mon_pair_bottom = mon_pair.winfo_rooty() + mon_pair.winfo_height()
                if bottommost is None or mon_pair_bottom > bottommost:
                    bottommost = mon_pair_bottom
        
        # If no mon pairs are visible, use the frame bounds for right half
        if leftmost is None:
            leftmost = self.winfo_rootx() + self.winfo_width() // 2
            rightmost = self.winfo_rootx() + self.winfo_width()
            topmost = self.winfo_rooty()
            bottommost = self.winfo_rooty() + self.winfo_height()
        
        # Contract by 2 pixels on the left side
        leftmost = leftmost + 2
        
        return (leftmost, topmost, rightmost, bottommost)


class SetupMovesSummary(ttk.Frame):
    def __init__(self, *args, callback=None, is_player=True, **kwargs):
        super().__init__(*args, **kwargs)

        self._callback = callback
        self._move_list = []

        self.reset_button = custom_components.SimpleButton(self, text="Reset Setup", command=self._reset)
        self.reset_button.grid(row=0, column=0, padx=2)

        self.setup_label = ttk.Label(self, text="Move:")
        self.setup_label.grid(row=0, column=1, padx=2)

        self.setup_moves = custom_components.SimpleOptionMenu(self, ["N/A"])
        self.setup_moves.grid(row=0, column=2, padx=2)

        self.add_button = custom_components.SimpleButton(self, text="Apply Move", command=self._add_setup_move)
        self.add_button.grid(row=0, column=3, padx=2)

        if is_player:
            label_text = "Player Setup:"
        else:
            label_text = "Enemy Setup:"
        self.extra_label = ttk.Label(self, text=label_text)
        self.extra_label.grid(row=0, column=4, padx=2)

        self.move_list_label = ttk.Label(self)
        self.move_list_label.grid(row=0, column=5, padx=2)
    
    def _reset(self, *args, **kwargs):
        self._move_list = []
        self._move_list_updated()
    
    def _add_setup_move(self, *args, **kwargs):
        self._move_list.append(self.setup_moves.get())
        self._move_list_updated()
    
    def configure_moves(self, new_moves):
        self.setup_moves.new_values(new_moves)
    
    def set_move_list(self, new_moves, trigger_update=False):
        self._move_list = new_moves
        self._move_list_updated(trigger_update=trigger_update)
    
    def get_stage_modifiers(self):
        result = universal_data_objects.StageModifiers()

        for cur_move in self._move_list:
            result = result.apply_stat_mod(current_gen_info().move_db().get_stat_mod(cur_move))
        
        return result
    
    def _move_list_updated(self, trigger_update=True):
        to_display = ", ".join(self._move_list)
        if not to_display:
            to_display = "None"

        self.move_list_label.configure(text=to_display)
        if self._callback is not None and trigger_update:
            self._callback()


class WeatherSummary(ttk.Frame):
    def __init__(self, *args, callback=None, **kwargs):
        super().__init__(*args, **kwargs)
        self._outer_callback = callback
        self._loading = False

        self.label = ttk.Label(self, text="Weather:")
        self.label.grid(row=0, column=0, padx=2)

        self.weather_dropdown = custom_components.SimpleOptionMenu(self, [const.WEATHER_NONE], callback=self._callback)
        self.weather_dropdown.grid(row=0, column=1, padx=2)
    
    def _callback(self, *args, **kwargs):
        if self._loading:
            return
        
        if self._outer_callback is not None:
            self._outer_callback()
    
    def set_weather(self, new_weather):
        self._loading = True
        self.weather_dropdown.set(new_weather)
        self._loading = False
    
    def configure_weather(self, weather_vals):
        self.weather_dropdown.new_values(weather_vals)
    
    def get_weather(self):
        return self.weather_dropdown.get()


class PrefightCandySummary(ttk.Frame):
    def __init__(self, *args, callback=None, **kwargs):
        super().__init__(*args, **kwargs)
        self._outer_callback = callback
        self._loading = False
        
        # Debounce timer for candy callback to prevent focus loss during typing
        self._candy_callback_timer = None

        self.label = ttk.Label(self, text="Prefight Candies:")
        self.label.grid(row=0, column=0, padx=2)

        self.candy_count = custom_components.AmountEntry(self, min_val=0, init_val=0, callback=self._callback, width=5)
        self.candy_count.grid(row=0, column=1, padx=2)
    
    def _callback(self, *args, **kwargs):
        if self._loading:
            return
        
        # Cancel any pending callback
        if self._candy_callback_timer is not None:
            self.after_cancel(self._candy_callback_timer)
        
        # Schedule the callback to run after a short delay (300ms)
        # This allows the user to type multiple characters without triggering route changes on each keystroke
        if self._outer_callback is not None:
            self._candy_callback_timer = self.after(300, self._delayed_candy_callback)
    
    def _delayed_candy_callback(self):
        """Execute the candy callback after a delay to prevent focus loss during typing."""
        self._candy_callback_timer = None
        if self._outer_callback is not None:
            self._outer_callback()
    
    def _increment_candy(self, event=None):
        """Increment pre-fight rare candies (F3 shortcut)."""
        if not self._loading:
            self.candy_count._raise_amt()
        return "break"
    
    def _decrement_candy(self, event=None):
        """Decrement pre-fight rare candies (F4 shortcut)."""
        if not self._loading:
            self.candy_count._lower_amt()
        return "break"
    
    def set_candy_count(self, new_amount):
        self._loading = True
        self.candy_count.set(new_amount)
        self._loading = False
    
    def get_prefight_candy_count(self):
        try:
            return int(self.candy_count.get())
        except Exception:
            return 0
    
    def disable(self):
        self.candy_count.disable()
    
    def enable(self):
        self.candy_count.enable()


class MonPairSummary(ttk.Frame):
    def __init__(self, controller:BattleSummaryController, mon_idx, *args, **kwargs):
        super().__init__(*args, **kwargs)

        self._controller = controller
        self._mon_idx = mon_idx

        bold_font = tkinter.font.nametofont("TkDefaultFont").copy()
        bold_font.configure(weight="bold")

        self.left_mon_label_frame = ttk.Frame(self, style="Header.TFrame")
        self.left_mon_label_frame.grid(row=0, column=0, columnspan=4, sticky=tk.EW, padx=2, pady=2)

        self.left_label = ttk.Label(self.left_mon_label_frame, text="", style="Header.TLabel")
        self.left_label.grid(row=0, column=1)

        self.left_mon_label_frame.columnconfigure(0, weight=1, uniform="left_col_group")
        self.left_mon_label_frame.columnconfigure(2, weight=1, uniform="left_col_group")

        self.divider = ttk.Frame(self, width=4, style="Divider.TFrame")
        self.divider.grid(row=0, column=4, rowspan=2, sticky=tk.NS)
        self.divider.grid_propagate(0)

        self.right_mon_label_frame = ttk.Frame(self, style="Header.TFrame")
        self.right_mon_label_frame.grid(row=0, column=5, columnspan=4, sticky=tk.EW, padx=2, pady=2)

        self.right_label = ttk.Label(self.right_mon_label_frame, text="", style="Header.TLabel")
        self.right_label.grid(row=0, column=1)

        self.right_mon_label_frame.columnconfigure(0, weight=1, uniform="right_col_group")
        self.right_mon_label_frame.columnconfigure(2, weight=1, uniform="right_col_group")

        self.columnconfigure(0, weight=1, uniform="label_group")
        self.columnconfigure(1, weight=1, uniform="label_group")

        self.columnconfigure(0, weight=1, uniform="move_group")
        self.columnconfigure(1, weight=1, uniform="move_group")
        self.columnconfigure(2, weight=1, uniform="move_group")
        self.columnconfigure(3, weight=1, uniform="move_group")

        self.columnconfigure(5, weight=1, uniform="move_group")
        self.columnconfigure(6, weight=1, uniform="move_group")
        self.columnconfigure(7, weight=1, uniform="move_group")
        self.columnconfigure(8, weight=1, uniform="move_group")

        self.move_list:List[DamageSummary] = []
        self._did_draw:List[bool] = []
        # Create 8 regular moves (4 player, 4 enemy)
        for cur_idx in range(8):
            self.move_list.append(DamageSummary(self._controller, self._mon_idx, cur_idx % 4, cur_idx < 4, self))
            self._did_draw.append(False)
        
        # Create 4 test move slots (player moves 5-8) - only shown when test moves is enabled
        self.test_move_slots:List[DamageSummary] = []
        self._did_draw_test_moves:List[bool] = []
        for slot_idx in range(4):
            # Test moves are player moves with indices 4-7
            self.test_move_slots.append(DamageSummary(self._controller, self._mon_idx, 4 + slot_idx, True, self, is_test_move=True))
            self._did_draw_test_moves.append(False)
    
    def update_rendering(self):
        player_rendering_info = self._controller.get_pkmn_info(self._mon_idx, True)
        enemy_rendering_info = self._controller.get_pkmn_info(self._mon_idx, False)

        self.left_label.configure(text=f"{player_rendering_info}")
        self.right_label.configure(text=f"{enemy_rendering_info}")

        # Check if test moves is enabled
        test_moves_enabled = self._controller.get_test_moves_enabled()

        # Render regular moves (4 player moves always, 4 enemy moves only if test moves disabled)
        for cur_idx, cur_move in enumerate(self.move_list):
            column_idx = cur_idx
            if column_idx >= 4:
                column_idx += 1

            # Hide enemy moves if test moves is enabled
            if cur_idx >= 4 and test_moves_enabled:
                if self._did_draw[cur_idx]:
                    cur_move.grid_forget()
                    self._did_draw[cur_idx] = False
            elif self._controller.get_move_info(cur_move._mon_idx, cur_move._move_idx, cur_move._is_player_mon) is not None:
                if not self._did_draw[cur_idx]:
                    cur_move.grid(row=1, column=column_idx, sticky=tk.NSEW)
                    self._did_draw[cur_idx] = True

                cur_move.update_rendering()
            else:
                if self._did_draw[cur_idx]:
                    cur_move.grid_forget()
                    self._did_draw[cur_idx] = False
        
        # Show/hide test move slots (replace enemy moves on right side)
        if test_moves_enabled:
            for slot_idx, test_move in enumerate(self.test_move_slots):
                column_idx = 5 + slot_idx  # Columns 5-8 for right side (where enemy moves were)
                if not self._did_draw_test_moves[slot_idx]:
                    test_move.grid(row=1, column=column_idx, sticky=tk.NSEW)
                    self._did_draw_test_moves[slot_idx] = True
                test_move.update_rendering()
        else:
            # Hide test move slots
            for slot_idx, test_move in enumerate(self.test_move_slots):
                if self._did_draw_test_moves[slot_idx]:
                    test_move.grid_forget()
                    self._did_draw_test_moves[slot_idx] = False


class AutocompleteEntry(ttk.Frame):
    """A text entry with autocomplete dropdown that filters as you type"""
    def __init__(self, parent, values, callback=None, width=20, **kwargs):
        super().__init__(parent, **kwargs)
        
        self._all_values = values
        self._callback = callback
        self._original_value = ""
        self._selecting = False  # Flag to track when a selection is being made
        
        # Create the entry widget
        self._entry_var = tk.StringVar()
        self._entry = ttk.Entry(self, textvariable=self._entry_var, width=width)
        self._entry.pack(fill=tk.BOTH, expand=True)
        
        # Create the listbox for suggestions (initially hidden)
        self._listbox = None
        self._listbox_visible = False
        
        # Bind events
        self._entry.bind('<KeyRelease>', self._on_key_release)
        self._entry.bind('<FocusIn>', self._on_focus_in)
        self._entry.bind('<FocusOut>', self._on_focus_out)
        self._entry.bind('<Return>', self._on_return)
        self._entry.bind('<Escape>', self._on_escape)
        self._entry.bind('<Down>', self._on_down_arrow)
        self._entry.bind('<Button-1>', self._on_click)
        
        # Also bind click on the frame
        self.bind('<Button-1>', self._on_frame_click)
    
    def _on_click(self, event):
        """Handle click to ensure focus is set."""
        self._entry.focus_set()
        self._register_focus()
        return None
    
    def _on_frame_click(self, event):
        """Handle click on frame to focus the entry field."""
        if event.widget == self:
            self._entry.focus_set()
            self._register_focus()
    
    def _register_focus(self):
        """Register focus with main window if available."""
        try:
            root = self.winfo_toplevel()
            if hasattr(root, 'register_text_field_focus'):
                root.register_text_field_focus(self._entry)
        except Exception:
            pass  # Main window might not exist yet
    
    def _unregister_focus(self):
        """Unregister focus with main window if available."""
        try:
            root = self.winfo_toplevel()
            if hasattr(root, 'unregister_text_field_focus'):
                root.unregister_text_field_focus()
        except Exception:
            pass  # Main window might not exist yet
        
    def _on_focus_in(self, event):
        """Store original value and clear text when focused"""
        self._selecting = False  # Reset flag when gaining focus
        self._original_value = self._entry_var.get()
        # Clear the text to allow easy typing
        self._entry_var.set("")
        # Register focus with main window
        self._register_focus()
    
    def _on_key_release(self, event):
        """Filter and show suggestions as user types"""
        # Ignore special keys
        if event.keysym in ['Return', 'Escape', 'Tab', 'Up', 'Down', 'Left', 'Right', 
                           'Shift_L', 'Shift_R', 'Control_L', 'Control_R', 'Alt_L', 'Alt_R']:
            return
        
        # Get current text
        current_text = self._entry_var.get()
        
        # Filter values
        if not current_text:
            filtered = self._all_values[:50]  # Show first 50 if empty
        else:
            search = current_text.lower()
            filtered = [v for v in self._all_values if search in v.lower()][:50]  # Limit to 50 results
        
        # Show filtered results
        if filtered and current_text:
            self._show_listbox(filtered)
        else:
            self._hide_listbox()
    
    def _on_down_arrow(self, event):
        """Move focus to listbox on down arrow"""
        if self._listbox_visible and self._listbox:
            self._selecting = True  # Prevent focus out from closing listbox
            self._listbox.focus_set()
            if self._listbox.size() > 0:
                self._listbox.selection_clear(0, tk.END)
                self._listbox.selection_set(0)
                self._listbox.see(0)
        return "break"
    
    def _on_return(self, event):
        """Accept the current value or first filtered result"""
        self._selecting = True
        current = self._entry_var.get()
        
        # If listbox is visible and has items, use the selected one
        if self._listbox_visible and self._listbox and self._listbox.size() > 0:
            selection = self._listbox.curselection()
            if selection:
                value = self._listbox.get(selection[0])
                self.set(value)
            else:
                # Use first item if nothing selected
                value = self._listbox.get(0)
                self.set(value)
        elif current in self._all_values:
            # Current value is valid
            pass
        else:
            # Invalid value, revert to original
            self._entry_var.set(self._original_value)
        
        self._hide_listbox()
        if self._callback:
            self._callback()
        # Reset flag after a delay to ensure focus out handler sees it
        self.after(200, lambda: setattr(self, '_selecting', False))
        return "break"
    
    def _on_escape(self, event):
        """Revert to original value and hide listbox"""
        self._selecting = False
        self._entry_var.set(self._original_value)
        self._hide_listbox()
        return "break"
    
    def _on_focus_out(self, event):
        """Handle focus loss"""
        # Unregister focus with main window
        self._unregister_focus()
        # Small delay to allow listbox click to register
        self.after(100, self._delayed_focus_out)
    
    def _delayed_focus_out(self):
        """Always revert to original value when focus is lost (unless explicitly selected)"""
        # If a selection is being made, don't interfere
        if self._selecting:
            return
        
        # Hide the listbox if it's still visible
        self._hide_listbox()
        
        # Always revert to original value when clicking away
        # Only explicit selection (Enter or clicking listbox) should change the value
        self._entry_var.set(self._original_value)
    
    def _show_listbox(self, values):
        """Show the listbox with filtered values"""
        # Recreate listbox if it doesn't exist or was destroyed
        if not self._listbox or not self._listbox.winfo_exists():
            # Create listbox as a toplevel window
            self._listbox_window = tk.Toplevel(self)
            self._listbox_window.wm_overrideredirect(True)
            
            self._listbox = tk.Listbox(self._listbox_window, height=10)
            self._listbox.pack(fill=tk.BOTH, expand=True)
            
            # Bind listbox events
            self._listbox.bind('<ButtonPress-1>', self._on_listbox_button_press)
            self._listbox.bind('<ButtonRelease-1>', self._on_listbox_click)
            self._listbox.bind('<Return>', self._on_listbox_select)
            self._listbox.bind('<Escape>', self._on_escape)
            self._listbox.bind('<FocusOut>', self._on_listbox_focus_out)
        
        # Update listbox content
        self._listbox.delete(0, tk.END)
        for value in values:
            self._listbox.insert(tk.END, value)
        
        # Position the listbox below the entry
        try:
            x = self._entry.winfo_rootx()
            y = self._entry.winfo_rooty() + self._entry.winfo_height()
            width = self._entry.winfo_width()
            self._listbox_window.geometry(f"{width}x200+{x}+{y}")
            self._listbox_window.deiconify()  # Make sure it's visible
            self._listbox_window.lift()
            self._listbox_visible = True
        except Exception:
            # If positioning fails, recreate next time
            self._listbox = None
            self._listbox_visible = False
    
    def _hide_listbox(self):
        """Hide the listbox"""
        if self._listbox and self._listbox.winfo_exists():
            try:
                self._listbox_window.withdraw()
            except Exception:
                pass
        self._listbox_visible = False
    
    def _on_listbox_button_press(self, event):
        """Set flag when mouse button is pressed on listbox (before focus change)"""
        self._selecting = True
    
    def _on_listbox_click(self, event):
        """Handle click on listbox item"""
        if self._listbox:
            selection = self._listbox.curselection()
            if selection:
                value = self._listbox.get(selection[0])
                self.set(value)
                self._hide_listbox()
                if self._callback:
                    self._callback()
        # Reset flag after a delay to ensure focus out handler sees it
        self.after(200, lambda: setattr(self, '_selecting', False))
    
    def _on_listbox_select(self, event):
        """Handle Enter key in listbox"""
        self._selecting = True
        if self._listbox:
            selection = self._listbox.curselection()
            if selection:
                value = self._listbox.get(selection[0])
                self.set(value)
                self._hide_listbox()
                if self._callback:
                    self._callback()
        # Reset flag after a delay to ensure focus out handler sees it
        self.after(200, lambda: setattr(self, '_selecting', False))
        return "break"
    
    def _on_listbox_focus_out(self, event):
        """Hide listbox when it loses focus"""
        # If selection is in progress (Enter was pressed), let the selection handler deal with it
        # Otherwise, revert and hide after a delay
        self.after(100, self._handle_listbox_focus_out)
    
    def _handle_listbox_focus_out(self):
        """Handle listbox losing focus after a delay"""
        if not self._selecting:
            # No selection in progress, so revert to original value
            self._entry_var.set(self._original_value)
        self._hide_listbox()
        # Reset the selecting flag after a delay
        self.after(100, lambda: setattr(self, '_selecting', False))
    
    def get(self):
        """Get the current value"""
        return self._entry_var.get()
    
    def set(self, value):
        """Set the current value"""
        self._entry_var.set(value)
        self._original_value = value
    
    def enable(self):
        """Enable the entry field"""
        self._entry.configure(state="normal")
    
    def disable(self):
        """Disable the entry field"""
        self._entry.configure(state="disabled")


class DamageSummary(ttk.Frame):
    def __init__(self, controller:BattleSummaryController, mon_idx, move_idx, is_player_mon, *args, is_test_move=False, **kwargs):
        super().__init__(*args, **kwargs)
        self._controller = controller
        self._mon_idx = mon_idx
        self._move_idx = move_idx
        self._is_player_mon = is_player_mon
        self._is_test_move = is_test_move
        self._move_name = None
        self._is_loading = False
        self._setup_move_cache = {}  # Cache for setup move info: {move_name: (is_setup, max_count)}

        self.columnconfigure(0, weight=1)

        self.padx = 2
        self.pady = 0
        self.row_idx = 0

        # Use tk.Frame instead of ttk.Frame to support background colors for highlights
        # Get the primary color from the theme for default background
        try:
            # Try to get the primary color from the style
            style = ttk.Style()
            bg_color = style.lookup("Primary.TFrame", "background")
            fg_color = style.lookup("Primary.TLabel", "foreground")
            if not bg_color:
                bg_color = ""
            if not fg_color:
                fg_color = ""
        except Exception:
            bg_color = ""
            fg_color = ""
        
        self.header = tk.Frame(self, bg=bg_color)
        self.header.grid(row=self.row_idx, column=0, sticky=tk.NSEW, padx=self.padx, pady=self.pady)
        self.header.columnconfigure(0, weight=1)
        self.row_idx += 1
        
        # For test moves, create a move selection dropdown instead of a label
        # Only show dropdown for the first Pokemon (mon_idx == 0) since test moves are global
        if self._is_test_move and self._mon_idx == 0:
            all_moves = current_gen_info().move_db().get_filtered_names()
            all_moves.insert(0, "")  # Add empty option at the start
            
            # Use custom autocomplete entry for filtering
            self.test_move_dropdown = AutocompleteEntry(
                self.header, 
                all_moves, 
                callback=self._on_test_move_changed,
                width=18
            )
        
        self.move_name_label = tk.Label(self.header, bg=bg_color, fg=fg_color, anchor="center", padx=0, pady=4)
        # Make player move labels clickable for highlights (cursor will be updated when highlights are enabled)
        if self._is_player_mon and not self._is_test_move:
            self.move_name_label.bind("<Button-1>", self._on_move_name_click)
            self.move_name_label.bind("<Button-3>", self._on_move_name_right_click)  # Right-click to reset
        self.custom_data_dropdown = custom_components.SimpleOptionMenu(self.header, [""], callback=self._custom_data_callback, width=14)
        # Also bind to <<ComboboxSelected>> event as a backup to ensure callback fires
        self.custom_data_dropdown.bind("<<ComboboxSelected>>", self._custom_data_callback)
        
        # Setup move dropdown (for moves that modify stats)
        self.setup_move_dropdown = custom_components.SimpleOptionMenu(self.header, ["0"], callback=self._setup_move_callback, width=8)
        self.setup_move_dropdown.bind("<<ComboboxSelected>>", self._setup_move_callback)

        self.range_frame = ttk.Frame(self, style="Contrast.TFrame")
        self.range_frame.grid(row=self.row_idx, column=0, sticky=tk.NSEW, padx=self.padx, pady=self.pady)
        self.range_frame.columnconfigure(0, weight=1)
        self.row_idx += 1

        self.damage_range = ttk.Label(self.range_frame, style="Contrast.TLabel")
        self.damage_range.grid(row=0, column=0, sticky=tk.W)
        self.pct_damage_range = ttk.Label(self.range_frame, style="Contrast.TLabel")
        self.pct_damage_range.grid(row=0, column=1, sticky=tk.E)
        self.crit_damage_range = ttk.Label(self.range_frame, style="Contrast.TLabel")
        self.crit_damage_range.grid(row=1, column=0, sticky=tk.W)
        self.crit_pct_damage_range = ttk.Label(self.range_frame, style="Contrast.TLabel")
        self.crit_pct_damage_range.grid(row=1, column=1, sticky=tk.E)

        self.kill_frame = ttk.Frame(self, style="Secondary.TFrame")
        self.kill_frame.grid(row=self.row_idx, column=0, sticky=tk.NSEW, padx=self.padx, pady=self.pady)
        self.rowconfigure(self.row_idx, weight=1)
        self.row_idx += 1

        self.num_to_kill = ttk.Label(self.kill_frame, justify=tk.LEFT, style="Secondary.TLabel")
        self.num_to_kill.grid(row=0, column=0, sticky=tk.NSEW)
    
    def flag_as_best_move(self):
        if self._is_player_mon:
            style = "Success"
        else:
            style = "Failure"

        self.kill_frame.configure(style=f"{style}.TFrame")
        self.num_to_kill.configure(style=f"{style}.TLabel")

    def unflag_as_best_move(self):
        self.kill_frame.configure(style="Secondary.TFrame")
        self.num_to_kill.configure(style="Secondary.TLabel")
    
    def _custom_data_callback(self, *args, **kwargs):
        if self._is_loading:
            return
        if self._move_name == const.MIMIC_MOVE_NAME:
            self._controller.update_mimic_selection(self.custom_data_dropdown.get())
        else:
            self._controller.update_custom_move_data(self._mon_idx, self._move_idx, self._is_player_mon, self.custom_data_dropdown.get())
    
    def _setup_move_callback(self, *args, **kwargs):
        if self._is_loading:
            return
        count = int(self.setup_move_dropdown.get())
        self._controller.update_move_setup_usage(self._mon_idx, self._move_idx, self._is_player_mon, count)
    
    def _on_test_move_changed(self, *args, **kwargs):
        """Called when user selects a test move from the dropdown"""
        if self._is_loading:
            return
        
        if not hasattr(self, 'test_move_dropdown'):
            return
            
        selected_move = self.test_move_dropdown.get()
        # Test move indices are 4-7, so slot_idx is move_idx - 4
        slot_idx = self._move_idx - 4
        self._controller.update_test_move(slot_idx, selected_move)

    def _on_move_name_click(self, event):
        """Handle click on move name to cycle highlight state"""
        if not self._controller.get_show_move_highlights():
            return
        if self._is_player_mon:
            # Update state in controller
            self._controller.update_move_highlight(self._mon_idx, self._move_idx, self._is_player_mon, reset=False)
            # Update UI immediately for instant feedback
            self._update_highlight_colors()
    
    def _on_move_name_right_click(self, event):
        """Handle right-click on move name to reset highlight state"""
        if not self._controller.get_show_move_highlights():
            return
        if self._is_player_mon:
            # Update state in controller
            self._controller.update_move_highlight(self._mon_idx, self._move_idx, self._is_player_mon, reset=True)
            # Update UI immediately for instant feedback
            self._update_highlight_colors()
    
    def _restore_dropdowns_immediately(self):
        """Restore dropdown grid positions immediately based on current move state"""
        try:
            move = self._controller.get_move_info(self._mon_idx, self._move_idx, self._is_player_mon)
            if move is None:
                return
            
            # Get custom data options (fast - already available on move object)
            custom_data_options = None
            custom_data_selection = None
            if move is not None:
                custom_data_options = move.custom_data_options
                custom_data_selection = move.custom_data_selection
            if self._move_name == const.MIMIC_MOVE_NAME:
                custom_data_options = move.mimic_options
                custom_data_selection = move.mimic_data
            
            # Restore custom_data_dropdown immediately if it exists
            col = 1
            if custom_data_options:
                if hasattr(self, 'custom_data_dropdown') and self.custom_data_dropdown.winfo_exists():
                    self.custom_data_dropdown.grid(row=0, column=col)
                    self.custom_data_dropdown.new_values(custom_data_options, default_val=custom_data_selection)
                col += 1
            
            # Check if this is a setup move (use cache to avoid repeated database lookups)
            is_setup_move = False
            max_setup_count = 0
            if move is not None and self._move_name:
                # Check cache first
                if self._move_name in self._setup_move_cache:
                    is_setup_move, max_setup_count = self._setup_move_cache[self._move_name]
                else:
                    # Cache miss - show dropdown IMMEDIATELY with default, don't wait for anything
                    # Show it first, then get the current value and update async
                    if hasattr(self, 'setup_move_dropdown') and self.setup_move_dropdown.winfo_exists():
                        # Show dropdown immediately with default max count (6) and default value (0)
                        # Don't call get_move_setup_usage() here - it might be slow
                        setup_options = [str(i) for i in range(7)]  # 0-6
                        if custom_data_options:
                            self.setup_move_dropdown.grid(row=0, column=col)
                        else:
                            self.setup_move_dropdown.grid(row=0, column=1)
                        self.setup_move_dropdown.new_values(setup_options, default_val="0")  # Use "0" as default, update async
                    
                    # Do the database lookup in the background and update if needed
                    def update_setup_dropdown():
                        try:
                            stat_mods = current_gen_info().move_db().get_stat_mod(self._move_name)
                            if stat_mods:
                                is_setup = True
                                max_stage_change = 0
                                for stat_mod in stat_mods:
                                    # stat_mod is a tuple (stat_name, stage_change)
                                    stage_change = abs(stat_mod[1]) if isinstance(stat_mod, tuple) else abs(stat_mod)
                                    if stage_change > max_stage_change:
                                        max_stage_change = stage_change
                                if max_stage_change >= 2:
                                    max_count = 3
                                elif max_stage_change == 1:
                                    max_count = 6
                                else:
                                    max_count = 0
                            else:
                                is_setup = False
                                max_count = 0
                            
                            # Cache the result
                            self._setup_move_cache[self._move_name] = (is_setup, max_count)
                            
                            # Update dropdown if it's actually a setup move
                            if is_setup and hasattr(self, 'setup_move_dropdown') and self.setup_move_dropdown.winfo_exists():
                                # Get current setup count and update dropdown
                                current_setup_count = self._controller.get_move_setup_usage(self._mon_idx, self._move_idx, self._is_player_mon)
                                if max_count != 6:  # Update options if different from default
                                    setup_options = [str(i) for i in range(max_count + 1)]
                                else:
                                    setup_options = [str(i) for i in range(7)]  # Keep 0-6
                                self.setup_move_dropdown.new_values(setup_options, default_val=str(current_setup_count))
                            else:
                                # Not a setup move - hide the dropdown
                                if hasattr(self, 'setup_move_dropdown') and self.setup_move_dropdown.winfo_exists():
                                    self.setup_move_dropdown.grid_remove()
                        except Exception:
                            pass
                    
                    # Schedule the lookup to run asynchronously (non-blocking)
                    # Use after(0) to run as soon as possible, but don't block UI
                    self.after(0, update_setup_dropdown)
                    return  # Return early, dropdown is already shown
            
            # Get current setup usage count
            current_setup_count = self._controller.get_move_setup_usage(self._mon_idx, self._move_idx, self._is_player_mon)
            
            # Restore setup_move_dropdown if it's a setup move
            if is_setup_move and hasattr(self, 'setup_move_dropdown') and self.setup_move_dropdown.winfo_exists():
                setup_options = [str(i) for i in range(max_setup_count + 1)]
                if custom_data_options:
                    self.setup_move_dropdown.grid(row=0, column=col)
                else:
                    self.setup_move_dropdown.grid(row=0, column=1)
                self.setup_move_dropdown.new_values(setup_options, default_val=str(current_setup_count))
        except Exception:
            pass
    
    def _update_highlight_colors(self):
        """Update highlight colors immediately without full refresh"""
        if not self._controller.get_show_move_highlights() or not self._is_player_mon:
            return
        
        move = self._controller.get_move_info(self._mon_idx, self._move_idx, self._is_player_mon)
        if move is None:
            return
        
        highlight_state = self._controller.get_move_highlight_state(self._mon_idx, self._move_idx, self._is_player_mon)
        
        # Get default colors
        try:
            style = ttk.Style()
            default_bg = str(style.lookup("Primary.TFrame", "background") or "")
            default_fg = str(style.lookup("Primary.TLabel", "foreground") or "")
        except Exception:
            default_bg = ""
            default_fg = ""
        
        # Check if fade is enabled and move has no highlight
        fade_enabled = config.get_fade_moves_without_highlight() and config.get_show_move_highlights()
        should_fade = fade_enabled and highlight_state == 0
        
        # Update colors immediately
        if highlight_state == 1:
            # Dark green
            self.header.configure(bg="#165416")
            self.move_name_label.configure(background="#165416", foreground="white")
            self._reset_fade_elements_to_normal()
            self._restore_dropdowns_immediately()  # Restore dropdowns immediately when highlighted
        elif highlight_state == 2:
            # Dark blue
            self.header.configure(bg="#212168")
            self.move_name_label.configure(background="#212168", foreground="white")
            self._reset_fade_elements_to_normal()
            self._restore_dropdowns_immediately()  # Restore dropdowns immediately when highlighted
        elif highlight_state == 3:
            # Dark orange
            self.header.configure(bg="#69400f")
            self.move_name_label.configure(background="#69400f", foreground="white")
            self._reset_fade_elements_to_normal()
            self._restore_dropdowns_immediately()  # Restore dropdowns immediately when highlighted
        else:
            # Default - apply fade if enabled
            if should_fade:
                # Blend foreground color with background at 0.1 opacity
                faded_fg = _blend_color(default_fg, default_bg, 0.1)
                self.header.configure(bg=default_bg)
                self.move_name_label.configure(background=default_bg, foreground=faded_fg)
                self._apply_fade_to_all_elements(faded_fg, default_bg)
            else:
                # Normal default
                self.header.configure(bg=default_bg)
                self.move_name_label.configure(background=default_bg, foreground=default_fg)
                self._reset_fade_elements_to_normal()
                self._restore_dropdowns_immediately()  # Restore dropdowns when not faded
    
    
    def _apply_fade_to_all_elements(self, faded_fg, default_bg):
        """Apply fade effect to all elements immediately - no delays"""
        try:
            style = ttk.Style()
            # Fade damage range labels
            contrast_fg = str(style.lookup("Contrast.TLabel", "foreground") or "")
            contrast_bg = str(style.lookup("Contrast.TFrame", "background") or "")
            if contrast_fg and contrast_bg:
                faded_contrast_fg = _blend_color(contrast_fg, contrast_bg, 0.1)
                self.damage_range.configure(foreground=faded_contrast_fg)
                self.pct_damage_range.configure(foreground=faded_contrast_fg)
                self.crit_damage_range.configure(foreground=faded_contrast_fg)
                self.crit_pct_damage_range.configure(foreground=faded_contrast_fg)
            
            # Fade KO text (num_to_kill)
            secondary_fg = str(style.lookup("Secondary.TLabel", "foreground") or "")
            secondary_bg = str(style.lookup("Secondary.TFrame", "background") or "")
            if secondary_fg and secondary_bg:
                faded_secondary_fg = _blend_color(secondary_fg, secondary_bg, 0.1)
                self.num_to_kill.configure(foreground=faded_secondary_fg)
            
            # Hide dropdowns when faded (setup_move_dropdown, custom_data_dropdown)
            if hasattr(self, 'setup_move_dropdown') and self.setup_move_dropdown.winfo_exists():
                self.setup_move_dropdown.grid_remove()
            if hasattr(self, 'custom_data_dropdown') and self.custom_data_dropdown.winfo_exists():
                self.custom_data_dropdown.grid_remove()
            if hasattr(self, 'test_move_dropdown') and self.test_move_dropdown.winfo_exists():
                # Fade the entry text in test_move_dropdown
                try:
                    self.test_move_dropdown._entry.configure(foreground=faded_fg)
                    self.test_move_dropdown.disable()
                except Exception:
                    pass
        except Exception:
            pass
    
    def _reset_fade_elements_to_normal(self):
        """Reset all fade elements to normal colors immediately - no delays"""
        try:
            style = ttk.Style()
            # Reset damage range labels
            contrast_fg = str(style.lookup("Contrast.TLabel", "foreground") or "")
            if contrast_fg:
                self.damage_range.configure(foreground=contrast_fg)
                self.pct_damage_range.configure(foreground=contrast_fg)
                self.crit_damage_range.configure(foreground=contrast_fg)
                self.crit_pct_damage_range.configure(foreground=contrast_fg)
            
            # Reset KO text
            secondary_fg = str(style.lookup("Secondary.TLabel", "foreground") or "")
            if secondary_fg:
                self.num_to_kill.configure(foreground=secondary_fg)
            
            # Show and re-enable dropdowns
            if hasattr(self, 'setup_move_dropdown') and self.setup_move_dropdown.winfo_exists():
                # Restore grid position if it was previously shown
                # The grid position will be restored in update_rendering when needed
                pass
            if hasattr(self, 'custom_data_dropdown') and self.custom_data_dropdown.winfo_exists():
                # Restore grid position if it was previously shown
                # The grid position will be restored in update_rendering when needed
                pass
            if hasattr(self, 'test_move_dropdown') and self.test_move_dropdown.winfo_exists():
                # Reset the entry text color in test_move_dropdown
                try:
                    style = ttk.Style()
                    default_fg = str(style.lookup("TEntry", "foreground") or "")
                    if default_fg:
                        self.test_move_dropdown._entry.configure(foreground=default_fg)
                    self.test_move_dropdown.enable()
                except Exception:
                    pass
        except Exception:
            pass

    @staticmethod
    def format_message(kill_info):
        kill_pct = kill_info[1]
        if kill_pct == -1:
            if config.do_ignore_accuracy():
                return f"{kill_info[0]}-hit kill: 100 %"
            else:
                return f"{kill_info[0]}-hit kill, IGNORING ACC"

        if round(kill_pct, 1) == int(kill_pct):
            rendered_kill_pct = f"{int(kill_pct)}"
        else:
            rendered_kill_pct = f"{kill_pct:.1f}"
        if config.do_ignore_accuracy():
            return f"{kill_info[0]}-hit kill: {rendered_kill_pct} %"
        return f"{kill_info[0]}-turn kill: {rendered_kill_pct} %"

    def update_rendering(self):
        move = self._controller.get_move_info(self._mon_idx, self._move_idx, self._is_player_mon)
        self._move_name = None if move is None else move.name

        self._is_loading = True
        # Check if fade is enabled and move has no highlight
        fade_enabled = config.get_fade_moves_without_highlight() and config.get_show_move_highlights()
        highlight_state = 0
        if self._is_player_mon and self._controller.get_show_move_highlights():
            highlight_state = self._controller.get_move_highlight_state(self._mon_idx, self._move_idx, self._is_player_mon)
        
        # Disable Player Highlight Strategy entirely when fade is enabled (for performance)
        if fade_enabled and self._is_player_mon:
            self.unflag_as_best_move()
        elif move is None or not move.is_best_move:
            self.unflag_as_best_move()
        else:
            self.flag_as_best_move()
        
        custom_data_options = None
        custom_data_selection = None
        if move is not None:
            custom_data_options = move.custom_data_options
            custom_data_selection = move.custom_data_selection
        if self._move_name == const.MIMIC_MOVE_NAME:
            custom_data_options = move.mimic_options
            custom_data_selection = move.mimic_data

        # Check if this is a setup move (stat modifier move)
        is_setup_move = False
        max_setup_count = 0
        if move is not None and self._move_name:
            # Check cache first to avoid repeated database lookups
            if self._move_name in self._setup_move_cache:
                is_setup_move, max_setup_count = self._setup_move_cache[self._move_name]
            else:
                # Cache miss - do database lookup and cache the result
                stat_mods = current_gen_info().move_db().get_stat_mod(self._move_name)
                if stat_mods:
                    is_setup_move = True
                    # Determine max count based on stat stage changes
                    # Find the maximum absolute value of stat changes
                    max_stage_change = 0
                    for stat_mod in stat_mods:
                        max_stage_change = max(max_stage_change, abs(stat_mod[1]))
                    
                    # +2 stages: max 3 uses (to reach +6)
                    # +1 stages: max 6 uses (to reach +6)
                    if max_stage_change >= 2:
                        max_setup_count = 3
                    elif max_stage_change == 1:
                        max_setup_count = 6
                # Cache the result for future use
                self._setup_move_cache[self._move_name] = (is_setup_move, max_setup_count)
        
        # Get current setup usage count
        current_setup_count = self._controller.get_move_setup_usage(self._mon_idx, self._move_idx, self._is_player_mon)
        
        # Layout header components
        # For test moves, show dropdown for first Pokemon only, label for others
        if self._is_test_move:
            # Update test move dropdown selection
            test_moves = self._controller.get_test_moves()
            slot_idx = self._move_idx - 4
            if slot_idx >= 0 and slot_idx < len(test_moves):
                current_test_move = test_moves[slot_idx]
                
                # Only show dropdown for first Pokemon (mon_idx == 0), show label for others
                if self._mon_idx == 0:
                    if hasattr(self, 'test_move_dropdown'):
                        # Set the combobox value
                        current_val = current_test_move if current_test_move else ""
                        if self.test_move_dropdown.get() != current_val:
                            self.test_move_dropdown.set(current_val)
                        self.test_move_dropdown.grid(row=0, column=0)
                    self.move_name_label.grid_forget()
                else:
                    # For other Pokemon, just show the selected move as a label
                    self.move_name_label.configure(text=current_test_move if current_test_move else "")
                    self.move_name_label.grid(row=0, column=0, columnspan=2)
                    if hasattr(self, 'test_move_dropdown'):
                        self.test_move_dropdown.grid_forget()
            
            self.custom_data_dropdown.grid_forget()
            self.setup_move_dropdown.grid_forget()
        elif custom_data_options:
            if hasattr(self, 'test_move_dropdown'):
                self.test_move_dropdown.grid_forget()
            self.move_name_label.grid_forget()
            self.move_name_label.grid(row=0, column=0)
            col = 1
            self.custom_data_dropdown.grid(row=0, column=col)
            self.custom_data_dropdown.new_values(custom_data_options, default_val=custom_data_selection)
            col += 1
            if is_setup_move:
                setup_options = [str(i) for i in range(max_setup_count + 1)]
                self.setup_move_dropdown.grid(row=0, column=col)
                self.setup_move_dropdown.new_values(setup_options, default_val=str(current_setup_count))
            else:
                self.setup_move_dropdown.grid_forget()
        else:
            if hasattr(self, 'test_move_dropdown'):
                self.test_move_dropdown.grid_forget()
            self.move_name_label.grid_forget()
            if is_setup_move:
                self.move_name_label.grid(row=0, column=0)
                setup_options = [str(i) for i in range(max_setup_count + 1)]
                self.setup_move_dropdown.grid(row=0, column=1)
                self.setup_move_dropdown.new_values(setup_options, default_val=str(current_setup_count))
            else:
                self.move_name_label.grid(row=0, column=0, columnspan=2)
                self.setup_move_dropdown.grid_forget()
            self.custom_data_dropdown.grid_forget()


        if move is None:
            self.move_name_label.configure(text="")
            self.damage_range.configure(text="")
            self.pct_damage_range.configure(text="")
            self.crit_damage_range.configure(text="")
            self.crit_pct_damage_range.configure(text="")
            self.num_to_kill.configure(text="")
        else:
            self.move_name_label.configure(text=f"{move.name}")
            # Update highlight color based on state - change the header frame background
            if self._is_player_mon and self._controller.get_show_move_highlights():
                highlight_state = self._controller.get_move_highlight_state(self._mon_idx, self._move_idx, self._is_player_mon)
                self.move_name_label.configure(cursor="hand2")
                # Get default colors for reset
                try:
                    style = ttk.Style()
                    default_bg = style.lookup("Primary.TFrame", "background") or ""
                    default_fg = style.lookup("Primary.TLabel", "foreground") or ""
                except Exception:
                    default_bg = ""
                    default_fg = ""
                
                # Check if fade is enabled and move has no highlight
                fade_enabled = config.get_fade_moves_without_highlight() and config.get_show_move_highlights()
                should_fade = fade_enabled and highlight_state == 0
                
                if highlight_state == 1:
                    # Dark green - set both header frame and label background
                    self.header.configure(bg="#006400")
                    self.move_name_label.configure(background="#006400", foreground="white")
                elif highlight_state == 2:
                    # Dark blue - set both header frame and label background
                    self.header.configure(bg="#00008B")
                    self.move_name_label.configure(background="#00008B", foreground="white")
                elif highlight_state == 3:
                    # Dark orange - set both header frame and label background
                    self.header.configure(bg="#FF8C00")
                    self.move_name_label.configure(background="#FF8C00", foreground="white")
                else:
                    # Default - apply fade if enabled
                    if should_fade:
                        # Blend foreground color with background at 0.1 opacity
                        faded_fg = _blend_color(default_fg, default_bg, 0.1)
                        self.header.configure(bg=default_bg)
                        self.move_name_label.configure(background=default_bg, foreground=faded_fg)
                        
                        # Also fade damage range labels, KO text, and dropdowns
                        try:
                            style = ttk.Style()
                            contrast_fg = str(style.lookup("Contrast.TLabel", "foreground") or "")
                            contrast_bg = str(style.lookup("Contrast.TFrame", "background") or "")
                            if contrast_fg and contrast_bg:
                                faded_contrast_fg = _blend_color(contrast_fg, contrast_bg, 0.1)
                                self.damage_range.configure(foreground=faded_contrast_fg)
                                self.pct_damage_range.configure(foreground=faded_contrast_fg)
                                self.crit_damage_range.configure(foreground=faded_contrast_fg)
                                self.crit_pct_damage_range.configure(foreground=faded_contrast_fg)
                            
                            # Fade KO text
                            secondary_fg = str(style.lookup("Secondary.TLabel", "foreground") or "")
                            secondary_bg = str(style.lookup("Secondary.TFrame", "background") or "")
                            if secondary_fg and secondary_bg:
                                faded_secondary_fg = _blend_color(secondary_fg, secondary_bg, 0.1)
                                self.num_to_kill.configure(foreground=faded_secondary_fg)
                            
                            # Hide dropdowns when faded (only if they were previously shown)
                            # Check if they should be visible based on move properties
                            if hasattr(self, 'setup_move_dropdown') and self.setup_move_dropdown.winfo_exists():
                                # Only hide if it was previously shown (grid_info exists)
                                try:
                                    if self.setup_move_dropdown.grid_info():
                                        self.setup_move_dropdown.grid_remove()
                                except Exception:
                                    pass
                            if hasattr(self, 'custom_data_dropdown') and self.custom_data_dropdown.winfo_exists():
                                # Only hide if it was previously shown (grid_info exists)
                                try:
                                    if self.custom_data_dropdown.grid_info():
                                        self.custom_data_dropdown.grid_remove()
                                except Exception:
                                    pass
                            if hasattr(self, 'test_move_dropdown') and self.test_move_dropdown.winfo_exists():
                                try:
                                    self.test_move_dropdown._entry.configure(foreground=faded_fg)
                                    self.test_move_dropdown.disable()
                                except Exception:
                                    pass
                        except Exception:
                            pass
                    else:
                        # Normal default
                        self.header.configure(bg=default_bg)
                        self.move_name_label.configure(background=default_bg, foreground=default_fg)
                        
                        # Reset damage range labels, KO text, and dropdowns to default
                        try:
                            style = ttk.Style()
                            contrast_fg = str(style.lookup("Contrast.TLabel", "foreground") or "")
                            if contrast_fg:
                                self.damage_range.configure(foreground=contrast_fg)
                                self.pct_damage_range.configure(foreground=contrast_fg)
                                self.crit_damage_range.configure(foreground=contrast_fg)
                                self.crit_pct_damage_range.configure(foreground=contrast_fg)
                            
                            secondary_fg = str(style.lookup("Secondary.TLabel", "foreground") or "")
                            if secondary_fg:
                                self.num_to_kill.configure(foreground=secondary_fg)
                            
                            # Reset and re-enable dropdowns
                            # Show dropdowns (grid position will be restored in update_rendering)
                            # No need to do anything here - update_rendering will handle showing them
                            if hasattr(self, 'test_move_dropdown') and self.test_move_dropdown.winfo_exists():
                                try:
                                    default_fg = str(style.lookup("TEntry", "foreground") or "")
                                    if default_fg:
                                        self.test_move_dropdown._entry.configure(foreground=default_fg)
                                    self.test_move_dropdown.enable()
                                except Exception:
                                    pass
                        except Exception:
                            pass
            else:
                # Reset to default when highlights are disabled
                if self._is_player_mon:
                    self.move_name_label.configure(cursor="")
                try:
                    style = ttk.Style()
                    default_bg = str(style.lookup("Primary.TFrame", "background") or "")
                    default_fg = str(style.lookup("Primary.TLabel", "foreground") or "")
                except Exception:
                    default_bg = ""
                    default_fg = ""
                self.header.configure(bg=default_bg)
                self.move_name_label.configure(background=default_bg, foreground=default_fg)
            
            # Handle enemy move fading (fade enemy moves that are NOT the best move)
            if not self._is_player_mon and config.get_fade_moves_without_highlight() and config.get_show_move_highlights():
                # Fade enemy moves that are NOT the best move (not highlighted in red)
                if move is not None and not move.is_best_move:
                    try:
                        style = ttk.Style()
                        default_bg = str(style.lookup("Primary.TFrame", "background") or "")
                        default_fg = str(style.lookup("Primary.TLabel", "foreground") or "")
                        if default_bg and default_fg:
                            faded_fg = _blend_color(default_fg, default_bg, 0.1)
                            self.header.configure(bg=default_bg)
                            self.move_name_label.configure(background=default_bg, foreground=faded_fg)
                            self._apply_fade_to_all_elements(faded_fg, default_bg)
                            # Disable dropdowns for faded enemy moves
                            if hasattr(self, 'custom_data_dropdown') and self.custom_data_dropdown.winfo_exists():
                                self.custom_data_dropdown.disable()
                    except Exception:
                        pass
                else:
                    # Reset to normal if it's the best move or move is None
                    try:
                        style = ttk.Style()
                        default_bg = str(style.lookup("Primary.TFrame", "background") or "")
                        default_fg = str(style.lookup("Primary.TLabel", "foreground") or "")
                        if default_bg and default_fg:
                            self.header.configure(bg=default_bg)
                            self.move_name_label.configure(background=default_bg, foreground=default_fg)
                            self._reset_fade_elements_to_normal()
                            # Re-enable dropdowns for unfaded enemy moves
                            if hasattr(self, 'custom_data_dropdown') and self.custom_data_dropdown.winfo_exists():
                                self.custom_data_dropdown.enable()
                    except Exception:
                        pass
            if move.damage_ranges is None:
                self.damage_range.configure(text="")
                self.pct_damage_range.configure(text="")
                self.crit_damage_range.configure(text="")
                self.crit_pct_damage_range.configure(text="")
            
            else:
                # Apply fade to damage range labels if enabled and move has no highlight
                fade_enabled = config.get_fade_moves_without_highlight() and config.get_show_move_highlights()
                should_fade = False
                if fade_enabled:
                    if self._is_player_mon:
                        highlight_state = self._controller.get_move_highlight_state(self._mon_idx, self._move_idx, self._is_player_mon)
                        should_fade = highlight_state == 0
                    else:
                        # For enemy moves, fade if NOT the best move (not highlighted in red)
                        should_fade = not move.is_best_move
                
                # Get contrast colors for damage range labels
                try:
                    style = ttk.Style()
                    contrast_fg = str(style.lookup("Contrast.TLabel", "foreground") or "")
                    contrast_bg = str(style.lookup("Contrast.TFrame", "background") or "")
                except Exception:
                    contrast_fg = ""
                    contrast_bg = ""
                
                # Apply fade to damage range labels, KO text, and dropdowns if needed
                if should_fade and contrast_fg and contrast_bg:
                    faded_contrast_fg = _blend_color(contrast_fg, contrast_bg, 0.1)
                    self.damage_range.configure(foreground=faded_contrast_fg)
                    self.pct_damage_range.configure(foreground=faded_contrast_fg)
                    self.crit_damage_range.configure(foreground=faded_contrast_fg)
                    self.crit_pct_damage_range.configure(foreground=faded_contrast_fg)
                    
                    # Fade KO text
                    try:
                        secondary_fg = str(style.lookup("Secondary.TLabel", "foreground") or "")
                        secondary_bg = str(style.lookup("Secondary.TFrame", "background") or "")
                        if secondary_fg and secondary_bg:
                            faded_secondary_fg = _blend_color(secondary_fg, secondary_bg, 0.1)
                            self.num_to_kill.configure(foreground=faded_secondary_fg)
                    except Exception:
                        pass
                    
                    # Fade and disable dropdowns
                    try:
                        # Get faded foreground for move name (already calculated above)
                        default_fg_str = str(default_fg) if default_fg else ""
                        default_bg_str = str(default_bg) if default_bg else ""
                        if default_fg_str and default_bg_str:
                            faded_fg = _blend_color(default_fg_str, default_bg_str, 0.1)
                            
                            if hasattr(self, 'setup_move_dropdown') and self.setup_move_dropdown.winfo_exists():
                                self.setup_move_dropdown.grid_remove()
                            if hasattr(self, 'custom_data_dropdown') and self.custom_data_dropdown.winfo_exists():
                                self.custom_data_dropdown.grid_remove()
                            if hasattr(self, 'test_move_dropdown') and self.test_move_dropdown.winfo_exists():
                                try:
                                    self.test_move_dropdown._entry.configure(foreground=faded_fg)
                                    self.test_move_dropdown.disable()
                                except Exception:
                                    pass
                    except Exception:
                        pass
                else:
                    # Reset to default contrast colors
                    if contrast_fg:
                        self.damage_range.configure(foreground=contrast_fg)
                        self.pct_damage_range.configure(foreground=contrast_fg)
                        self.crit_damage_range.configure(foreground=contrast_fg)
                        self.crit_pct_damage_range.configure(foreground=contrast_fg)
                    
                    # Reset KO text and dropdowns to default
                    try:
                        secondary_fg = str(style.lookup("Secondary.TLabel", "foreground") or "")
                        if secondary_fg:
                            self.num_to_kill.configure(foreground=secondary_fg)
                        
                        # Reset and re-enable dropdowns
                        # Show dropdowns (grid position will be restored in update_rendering)
                        # No need to do anything here - update_rendering will handle showing them
                        if hasattr(self, 'test_move_dropdown') and self.test_move_dropdown.winfo_exists():
                            try:
                                default_fg = str(style.lookup("TEntry", "foreground") or "")
                                if default_fg:
                                    self.test_move_dropdown._entry.configure(foreground=default_fg)
                                self.test_move_dropdown.enable()
                            except Exception:
                                pass
                    except Exception:
                        pass
                
                self.damage_range.configure(text=f"{move.damage_ranges.min_damage} - {move.damage_ranges.max_damage}")
                pct_min_damage = round(move.damage_ranges.min_damage / move.defending_mon_hp * 100)
                pct_max_damage = round(move.damage_ranges.max_damage / move.defending_mon_hp * 100)
                self.pct_damage_range.configure(text=f"{pct_min_damage} - {pct_max_damage}%")

                self.crit_damage_range.configure(text=f"{move.crit_damage_ranges.min_damage} - {move.crit_damage_ranges.max_damage}")
                crit_pct_min_damage = round(move.crit_damage_ranges.min_damage / move.defending_mon_hp * 100)
                crit_pct_max_damage = round(move.crit_damage_ranges.max_damage / move.defending_mon_hp * 100)
                self.crit_pct_damage_range.configure(text=f"{crit_pct_min_damage} - {crit_pct_max_damage}%")

            
            max_num_messages = 3
            kill_ranges = move.kill_ranges
            if len(kill_ranges) > max_num_messages:
                kill_ranges = kill_ranges[:max_num_messages - 1] + [kill_ranges[-1]]

            kill_ranges = [self.format_message(x) for x in kill_ranges]
            self.num_to_kill.configure(text="\n".join(kill_ranges))

        self._is_loading = False
