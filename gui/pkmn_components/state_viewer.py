import tkinter as tk
from tkinter import ttk
import logging
from gui.pkmn_components.inventory_viewer import InventoryViewer
from gui.pkmn_components.pkmn_viewer import PkmnViewer
from gui.pkmn_components.stat_exp_viewer import StatExpViewer

from routing import full_route_state

logger = logging.getLogger(__name__)



class StateViewer(ttk.Frame):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.pkmn = PkmnViewer(self, font_size=12)
        self.pkmn.grid(row=0, column=0, padx=5, pady=5, sticky=tk.S)
        self.stat_xp = StatExpViewer(self)
        self.stat_xp.grid(row=0, column=1, padx=5, pady=5, sticky=tk.S)
        self.inventory = InventoryViewer(self)
        self.inventory.grid(row=0, column=2, padx=5, pady=5, sticky=tk.S)
    
    def set_state(self, cur_state:full_route_state.RouteState):
        if cur_state is None:
            # Handle None state - clear all displays
            from routing import state_objects
            empty_inventory = state_objects.Inventory()
            self.inventory.set_inventory(empty_inventory)
            # Clear stat display
            self.stat_xp.set_state(None)
            # For pkmn, we can't easily create an empty one, so just skip updating it
            # The display will remain showing the last valid state, which is acceptable
            return
        
        self.inventory.set_inventory(cur_state.inventory)
        self.pkmn.set_pkmn(cur_state.solo_pkmn.get_pkmn_obj(cur_state.badges), cur_state.badges)
        self.stat_xp.set_state(cur_state)