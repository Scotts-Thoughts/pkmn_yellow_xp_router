import tkinter as tk
import sys


class Popup(tk.Toplevel):
    def __init__(self, main_window:tk.Tk, *args, **kwargs):
        tk.Toplevel.__init__(self, main_window, *args, **kwargs)
        self._main_window = main_window
        # TODO: if we want the little flash thingy, try this instead of disabling: https://stackoverflow.com/a/28541762
        if sys.platform == "win32":
            self._main_window.attributes('-disabled', True)

        # Center the dialog on the main window
        self.update_idletasks()
        self._center_on_main_window()

        self.focus_set()
        self.protocol("WM_DELETE_WINDOW", self.close)
    
    def _center_on_main_window(self):
        """Center this dialog on the main window instead of the screen."""
        try:
            # Get main window position and size
            main_x = self._main_window.winfo_x()
            main_y = self._main_window.winfo_y()
            main_width = self._main_window.winfo_width()
            main_height = self._main_window.winfo_height()
            
            # Get dialog size
            dialog_width = self.winfo_width()
            dialog_height = self.winfo_height()
            
            # Calculate centered position relative to main window
            x = main_x + (main_width // 2) - (dialog_width // 2)
            y = main_y + (main_height // 2) - (dialog_height // 2)
            
            # Ensure dialog stays on screen (fallback to screen center if main window is off-screen)
            if x < 0 or y < 0:
                # Fallback: center on screen
                screen_width = self.winfo_screenwidth()
                screen_height = self.winfo_screenheight()
                x = (screen_width // 2) - (dialog_width // 2)
                y = (screen_height // 2) - (dialog_height // 2)
            
            self.geometry(f"+{x}+{y}")
        except Exception:
            # If anything goes wrong, just use default positioning
            pass

    def close(self, event=None):
        if sys.platform == "win32":
            self._main_window.attributes('-disabled', False)
        self.destroy()
