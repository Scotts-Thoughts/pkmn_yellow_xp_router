import argparse
import os
<<<<<<< Updated upstream
=======
import subprocess
import sys
>>>>>>> Stashed changes
import concurrent.futures
from threading import Thread
import logging
from typing import Tuple

from PySide6.QtWidgets import QApplication
from PySide6.QtCore import QTimer

from controllers.main_controller import MainController
<<<<<<< Updated upstream
from controllers.battle_summary_controller import BattleSummaryController
from route_recording.recorder import RecorderController
from gui.main_window import MainWindow
from webserver import router_server
=======
from gui_qt.main_window import MainWindow
from gui_qt.theme import generate_stylesheet
>>>>>>> Stashed changes

from utils.constants import const
from utils.config_manager import config
from utils import setup, custom_logging

logger = logging.getLogger(__name__)


def make_controllers(headless) -> Tuple[MainController, BattleSummaryController, RecorderController]:
    custom_logging.config_logging(const.GLOBAL_CONFIG_DIR)

    if not os.path.exists(config.get_user_data_dir()):
        os.makedirs(config.get_user_data_dir())

    setup.init_base_generations()
    controller = MainController(headless=headless)
    battle_controller = BattleSummaryController(controller)
    recorder_controller = RecorderController(controller)
    controller.sync_register_record_mode_change(recorder_controller.on_recording_mode_changed)

    return controller, battle_controller, recorder_controller


def run(
    controller:MainController,
    battle_controller:BattleSummaryController,
    recorder_controller:RecorderController,
    port:int,
    headless:bool
):

    if headless:
        router_server.spawn_server(
            controller,
            battle_controller,
            recorder_controller,
            port,
        )
    else:
        MainWindow(
            controller,
            battle_controller,
            recorder_controller,
            #f"http://127.0.0.1:{port}/shutdown"
        ).run()


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument("--debug", action="store_true")
    parser.add_argument("--gui", action="store_true")
    parser.add_argument("--port", "-p", type=int, default=5000)
    args = parser.parse_args()
    const.DEBUG_MODE = args.debug

<<<<<<< Updated upstream
    controllers = make_controllers(headless=(not args.gui))
    run(
        *controllers,
        args.port,
        not args.gui,
    )
=======
    custom_logging.config_logging(const.GLOBAL_CONFIG_DIR)

    if not os.path.exists(config.get_user_data_dir()):
        os.makedirs(config.get_user_data_dir())

    # Load only default generation first for faster startup
    # Remaining generations will load in background after window is shown
    setup.init_default_generation_only()

    qt_app = QApplication(sys.argv)
    qt_app.setApplicationName("Pokemon RBY XP Router")
    qt_app.setStyleSheet(generate_stylesheet())

    controller = MainController()
    window = MainWindow(controller)

    # Load remaining generations in background after window is created
    QTimer.singleShot(0, setup.load_remaining_generations_in_background)

    with concurrent.futures.ThreadPoolExecutor() as executor:
        background_thread = executor.submit(setup.route_startup_check_for_upgrade, window)

        window.run()
        exit_code = qt_app.exec()

        flag_to_auto_update = background_thread.result()
        logger.info(f"App closed, autoupdate requested? {flag_to_auto_update}")

    if flag_to_auto_update:
        logger.info(f"Beginning cleanup of old version")
        auto_update.auto_cleanup_old_version()
        logger.info(f"Launching temp gui for AutoUpgrade")
        # Auto-upgrade still uses tkinter since it's a simple standalone window
        from gui.auto_upgrade_window import AutoUpgradeGUI
        app_upgrade = AutoUpgradeGUI()
        app_upgrade.mainloop()

        logger.info(f"About to restart: {sys.argv}")
        if os.path.splitext(sys.argv[0])[1] == ".pyw":
            subprocess.Popen([sys.executable] + sys.argv, start_new_session=True)
        else:
            subprocess.Popen(sys.argv, start_new_session=True)
    else:
        sys.exit(exit_code)
>>>>>>> Stashed changes
