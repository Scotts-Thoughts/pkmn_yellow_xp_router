"""Interactive baseline: end-to-end GUI latency for load / select / edit / undo / search."""
import os, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import ROOT, largest_routes
os.chdir(ROOT)
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import QTimer, QEventLoop
from controllers.main_controller import MainController
from gui_qt.main_window import MainWindow
from gui_qt.theme import generate_stylesheet
from utils import setup

setup.init_base_generations()
app = QApplication(sys.argv); app.setStyleSheet(generate_stylesheet())
controller = MainController(); window = MainWindow(controller); window.run()


def drain(ms=0):
    loop = QEventLoop(); QTimer.singleShot(ms, loop.quit); loop.exec()


drain(600)  # let deferred init settle


def measure(label, fn, settle_ms=500):
    t = time.perf_counter(); fn(); sync = time.perf_counter() - t
    drain(0); idle = time.perf_counter() - t
    drain(settle_ms); late = time.perf_counter() - t
    exc = controller.get_next_exception_info()
    print(f"{label:48s} sync={sync*1000:8.1f}ms  idle={idle*1000:8.1f}ms  settled(+{settle_ms}ms)={late*1000:8.1f}ms"
          + (f"  EXCEPTION: {exc}" if exc else ""))


routes = largest_routes(2)
measure("load_route (largest)", lambda: controller.load_route(routes[0]))
ids = list(controller.get_raw_route().event_lookup.keys()); mid = ids[len(ids) // 2]
fight = next(i for i in ids if controller.get_event_by_id(i).event_definition.trainer_def is not None
             and len(controller.get_event_by_id(i).event_definition.get_pokemon_list()) >= 4)
measure("select_new_events (mid event)", lambda: controller.select_new_events([mid]))
measure("select_new_events (4+ mon trainer fight)", lambda: controller.select_new_events([fight]))
measure("toggle_event_highlight", lambda: controller.toggle_event_highlight([mid]))
measure("delete_events (1 event)", lambda: controller.delete_events([mid]))
measure("undo", lambda: controller.undo())
measure("set_route_search('brock')", lambda: controller.set_route_search("brock"))
measure("set_route_search('')", lambda: controller.set_route_search(""))
if len(routes) > 1:
    measure("load_route (second largest)", lambda: controller.load_route(routes[1]))
app.quit()
