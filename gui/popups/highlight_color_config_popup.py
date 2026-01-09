import tkinter as tk
from tkinter import ttk

from gui.popups.base_popup import Popup
from gui import custom_components
from utils.config_manager import config
from utils.constants import const


class HighlightColorConfigWindow(Popup):
    def __init__(self, main_window, *args, **kwargs):
        super().__init__(main_window, *args, **kwargs)
        
        self.title("Configure Highlight Colors")
        
        self._color_frame = ttk.Frame(self)
        self._color_frame.grid(row=0, column=0, padx=20, pady=20)
        
        self._color_header = tk.Label(self._color_frame, text="Highlight Color Configuration:", font=("Arial", 12, "bold"))
        self._color_header.grid(row=0, column=0, columnspan=2, padx=5, pady=10, sticky=tk.EW)
        
        # Create color updaters for each highlight (1-9)
        self._highlight_color_updaters = []
        for i in range(1, 10):
            row = i
            label_text = f"Highlight {i}:"
            getter = lambda idx=i: config.get_highlight_color(idx)
            setter = lambda color, idx=i: self._set_highlight_color(idx, color)
            
            color_updater = custom_components.ConfigColorUpdater(
                self._color_frame,
                label_text=label_text,
                setter=setter,
                getter=getter,
                callback=self.lift
            )
            color_updater.grid(row=row, column=0, columnspan=2, sticky=tk.EW, padx=5, pady=2)
            self._highlight_color_updaters.append(color_updater)
        
        # Buttons
        button_frame = ttk.Frame(self._color_frame)
        button_frame.grid(row=10, column=0, columnspan=2, pady=15)
        
        self._reset_button = tk.Button(button_frame, text="Reset to Defaults", command=self._reset_to_defaults)
        self._reset_button.grid(row=0, column=0, padx=5)
        
        self._close_button = custom_components.SimpleButton(button_frame, text="Close", command=self.close)
        self._close_button.grid(row=0, column=1, padx=5)
        
        self.bind('<Escape>', self.close)
        
        # Refresh the event list after closing to update colors
        self._main_window_ref = main_window
    
    def _set_highlight_color(self, highlight_num, color):
        """Set highlight color and refresh the event list."""
        config.set_highlight_color(highlight_num, color)
        # Update the color in the route list immediately
        if hasattr(self._main_window_ref, 'event_list'):
            self._main_window_ref.event_list._update_highlight_colors()
            self._main_window_ref.event_list.refresh()
    
    def _reset_to_defaults(self):
        """Reset all highlight colors to defaults."""
        config.set_highlight_color(1, config.DEFAULT_HIGHLIGHT_COLOR_1)
        config.set_highlight_color(2, config.DEFAULT_HIGHLIGHT_COLOR_2)
        config.set_highlight_color(3, config.DEFAULT_HIGHLIGHT_COLOR_3)
        config.set_highlight_color(4, config.DEFAULT_HIGHLIGHT_COLOR_4)
        config.set_highlight_color(5, config.DEFAULT_HIGHLIGHT_COLOR_5)
        config.set_highlight_color(6, config.DEFAULT_HIGHLIGHT_COLOR_6)
        config.set_highlight_color(7, config.DEFAULT_HIGHLIGHT_COLOR_7)
        config.set_highlight_color(8, config.DEFAULT_HIGHLIGHT_COLOR_8)
        config.set_highlight_color(9, config.DEFAULT_HIGHLIGHT_COLOR_9)
        
        # Refresh all color updaters
        for updater in self._highlight_color_updaters:
            updater.refresh_color()
        
        # Update the event list
        if hasattr(self._main_window_ref, 'event_list'):
            self._main_window_ref.event_list._update_highlight_colors()
            self._main_window_ref.event_list.refresh()

