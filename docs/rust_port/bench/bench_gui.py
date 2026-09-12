"""Startup baseline: wall time of each stage up to the first event-loop pass."""
import os, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import ROOT
os.chdir(ROOT)

T = {}
t = time.perf_counter()
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import QTimer
T["import PySide6"] = time.perf_counter() - t
t = time.perf_counter()
from controllers.main_controller import MainController
from gui_qt.main_window import MainWindow
from gui_qt.theme import generate_stylesheet
from utils import setup
T["import app modules"] = time.perf_counter() - t
t = time.perf_counter(); setup.init_base_generations(); T["init_base_generations"] = time.perf_counter() - t
t = time.perf_counter(); app = QApplication(sys.argv); app.setStyleSheet(generate_stylesheet()); T["QApplication + stylesheet"] = time.perf_counter() - t
t = time.perf_counter(); controller = MainController(); T["MainController()"] = time.perf_counter() - t
t = time.perf_counter(); window = MainWindow(controller); T["MainWindow()"] = time.perf_counter() - t
t = time.perf_counter(); window.run(); T["window.run()"] = time.perf_counter() - t
t = time.perf_counter()


def done():
    T["first event-loop pass"] = time.perf_counter() - t
    app.quit()


QTimer.singleShot(0, done)
app.exec()
total = 0.0
for name, secs in T.items():
    total += secs
    print(f"{name:28s} {secs:7.3f}s")
print(f"{'TOTAL':28s} {total:7.3f}s")
