"""Run the Python (Qt) app unattended against the mock GameHook.

Mirrors ``main.pyw`` but redirects the global config dir and the GameHook
URL, loads a route, records until the mock reports the scenario is done,
saves the route and quits. Nothing in the Python tree is modified; the
redirections are monkeypatches applied before the app modules are imported.

Environment:
  XPR_TEST_CONFIG_DIR   global config dir (config.json / logs) to use
  XPR_TEST_GAMEHOOK_URL mock GameHook base URL
  XPR_TEST_ROUTE        full path of the route file to load
  XPR_TEST_SAVE_NAME    route name to save as when done
  XPR_TEST_MAX_SECS     give up after this many seconds (default 300)
"""
import json
import os
import sys
import time
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
sys.path.insert(0, ROOT)
os.chdir(ROOT)

CONFIG_DIR = os.environ["XPR_TEST_CONFIG_DIR"]
GAMEHOOK_URL = os.environ["XPR_TEST_GAMEHOOK_URL"].rstrip("/")
ROUTE_PATH = os.environ["XPR_TEST_ROUTE"]
SAVE_NAME = os.environ.get("XPR_TEST_SAVE_NAME", "recorded_python")
MAX_SECS = float(os.environ.get("XPR_TEST_MAX_SECS", "300"))

# --- redirect the config dir before anything reads it ---------------------
from utils.constants import const  # noqa: E402

const.GLOBAL_CONFIG_DIR = CONFIG_DIR
const.GLOBAL_CONFIG_FILE = os.path.join(CONFIG_DIR, "config.json")

from utils.config_manager import config  # noqa: E402
import controllers.main_controller  # noqa: E402,F401  (resolves the import chain, see tests/conftest.py)
from utils import custom_logging, setup  # noqa: E402
import route_recording.gamehook_client as ghc  # noqa: E402

_orig_init = ghc.GameHookClient.__init__


def _patched_init(self, connection_string="http://localhost:8085", clear_callbacks_on_load=False):
    _orig_init(self, connection_string=GAMEHOOK_URL, clear_callbacks_on_load=clear_callbacks_on_load)


ghc.GameHookClient.__init__ = _patched_init

from PySide6.QtCore import QTimer  # noqa: E402
from PySide6.QtWidgets import QApplication  # noqa: E402

from controllers.main_controller import MainController  # noqa: E402
from gui_qt.main_window import MainWindow  # noqa: E402
from gui_qt.theme import generate_stylesheet  # noqa: E402


def mock_status():
    try:
        with urllib.request.urlopen(f"{GAMEHOOK_URL}/mock/status", timeout=2) as r:
            return json.loads(r.read())
    except Exception:
        return {}


def main():
    custom_logging.config_logging(const.GLOBAL_CONFIG_DIR)
    if not os.path.exists(config.get_user_data_dir()):
        os.makedirs(config.get_user_data_dir())
    setup.init_base_generations()

    qt_app = QApplication(sys.argv)
    qt_app.setApplicationName("Pokemon Solo Challenge Router (harness)")
    qt_app.setStyleSheet(generate_stylesheet())
    controller = MainController()
    window = MainWindow(controller)
    window.run()

    state = {"phase": "boot", "t0": time.time(), "quiet_since": None}

    def step():
        elapsed = time.time() - state["t0"]
        if elapsed > MAX_SECS:
            print("harness: timed out", flush=True)
            finish()
            return
        if state["phase"] == "boot":
            if elapsed < 2.0:
                return
            print(f"harness: loading {ROUTE_PATH}", flush=True)
            controller.load_route(ROUTE_PATH)
            state["phase"] = "loaded"
        elif state["phase"] == "loaded":
            if elapsed < 3.5:
                return
            print("harness: starting recording", flush=True)
            controller.set_record_mode(True)
            state["phase"] = "recording"
        elif state["phase"] == "recording":
            st = mock_status()
            if st.get("done"):
                if state["quiet_since"] is None:
                    state["quiet_since"] = time.time()
                elif time.time() - state["quiet_since"] > 3.0:
                    finish()
        # (finished: nothing to do)

    def finish():
        if state["phase"] == "finished":
            return
        state["phase"] = "finished"
        print("harness: stopping recording", flush=True)
        if controller.is_record_mode_active():
            controller.set_record_mode(False)
        QTimer.singleShot(1500, save_and_quit)

    def save_and_quit():
        print(f"harness: saving as {SAVE_NAME}", flush=True)
        controller.save_route(SAVE_NAME)
        QTimer.singleShot(500, qt_app.quit)

    timer = QTimer()
    timer.timeout.connect(step)
    timer.start(250)
    code = qt_app.exec()
    print(f"harness: exit {code}", flush=True)
    os._exit(0)


if __name__ == "__main__":
    main()
