"""cProfile of MainWindow() construction and of the deferred init on the first event-loop pass."""
import os, sys, cProfile, pstats, io
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import ROOT
os.chdir(ROOT)
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import QTimer
from controllers.main_controller import MainController
from gui_qt.main_window import MainWindow
from gui_qt.theme import generate_stylesheet
from utils import setup

setup.init_base_generations()
app = QApplication(sys.argv); app.setStyleSheet(generate_stylesheet())
controller = MainController()


def report(profile, title, n=35):
    buf = io.StringIO()
    pstats.Stats(profile, stream=buf).sort_stats("cumulative").print_stats(n)
    print(f"\n######## {title} ########")
    for line in buf.getvalue().splitlines():
        if any(k in line for k in ("pkmn_yellow_xp_router", "PySide6", "PIL", "json", "ncalls", "function calls")):
            print(line.replace(ROOT + os.sep, ""))


pr = cProfile.Profile(); pr.enable()
window = MainWindow(controller)
pr.disable(); report(pr, "MainWindow() construction")

pr2 = cProfile.Profile()
window.run()
pr2.enable()
QTimer.singleShot(0, app.quit)
app.exec()
pr2.disable(); report(pr2, "first event-loop pass (deferred init)")
