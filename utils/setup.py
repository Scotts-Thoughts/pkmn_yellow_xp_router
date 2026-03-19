import os
import sys
import logging

from pkmn.gen_4 import gen_four_object
from utils.constants import const

from PySide6.QtWidgets import QMessageBox

from pkmn.gen_1 import gen_one_object
from pkmn.gen_2 import gen_two_object
from pkmn.gen_3 import gen_three_object
from pkmn import gen_factory

from utils.config_manager import config
from utils import auto_update

logger = logging.getLogger(__name__)


def route_startup_check_for_upgrade(main_app):
    new_app_version, new_asset_url = auto_update.get_new_version_info()
    logger.info(f"Latest version determined to be: {new_app_version}")

    if not auto_update.is_upgrade_needed(new_app_version, const.APP_VERSION):
        logger.info(f"No upgrade needed")
        return False

    if not auto_update.is_upgrade_possible():
        logger.info(f"Cannot upgrade this deployment")
        return False

    # Store update info on the window so the Update menu can use it later
    main_app._deferred_update_version = new_app_version
    main_app._deferred_update_url = new_asset_url

    # Enable the "Update" menu action on the GUI thread
    def _enable_update_action():
        if hasattr(main_app, '_act_apply_update'):
            main_app._act_apply_update.setEnabled(True)
            main_app._act_apply_update.setText(f"Update to {new_app_version}")
    main_app._blocking_dispatch(_enable_update_action)

    # If user has suppressed the prompt, don't ask — just leave the menu item enabled
    if config.get_suppress_update_prompt():
        logger.info(f"Update prompt suppressed by user preference")
        return False

    # Must show the dialog on the main/GUI thread since this runs in a background thread.
    # Use the main window's existing _blocking_dispatch mechanism for thread safety.
    user_accepted = [False]

    def _prompt():
        result = QMessageBox.question(
            main_app,
            "Update?",
            f"Found new version {new_app_version}\nDo you want to update?",
            QMessageBox.Yes | QMessageBox.No,
            QMessageBox.No,
        )
        user_accepted[0] = result == QMessageBox.Yes

    main_app._blocking_dispatch(_prompt)

    if not user_accepted[0]:
        logger.info(f"User rejected auto-update")
        return False

    logger.info(f"User requested auto-update")
    main_app._blocking_dispatch(main_app.cancel_and_quit)
    return True


def init_base_generations():
    gen_factory._gen_factory.register_gen(gen_one_object.gen_one_red, const.RED_VERSION)
    gen_factory._gen_factory.register_gen(gen_one_object.gen_one_blue, const.BLUE_VERSION)
    gen_factory._gen_factory.register_gen(gen_one_object.gen_one_yellow, const.YELLOW_VERSION)

    gen_factory._gen_factory.register_gen(gen_two_object.gen_two_gold, const.GOLD_VERSION)
    gen_factory._gen_factory.register_gen(gen_two_object.gen_two_silver, const.SILVER_VERSION)
    gen_factory._gen_factory.register_gen(gen_two_object.gen_two_crystal, const.CRYSTAL_VERSION)

    gen_factory._gen_factory.register_gen(gen_three_object.gen_three_ruby, const.RUBY_VERSION)
    gen_factory._gen_factory.register_gen(gen_three_object.gen_three_sapphire, const.SAPPHIRE_VERSION)
    gen_factory._gen_factory.register_gen(gen_three_object.gen_three_emerald, const.EMERALD_VERSION)
    gen_factory._gen_factory.register_gen(gen_three_object.gen_three_fire_red, const.FIRE_RED_VERSION)
    gen_factory._gen_factory.register_gen(gen_three_object.gen_three_leaf_green, const.LEAF_GREEN_VERSION)

    gen_factory._gen_factory.register_gen(gen_four_object.gen_four_platinum, const.PLATINUM_VERSION)
    gen_factory._gen_factory.register_gen(gen_four_object.gen_four_diamond, const.DIAMOND_VERSION)
    gen_factory._gen_factory.register_gen(gen_four_object.gen_four_pearl, const.PEARL_VERSION)
    gen_factory._gen_factory.register_gen(gen_four_object.gen_four_heartgold, const.HEART_GOLD_VERSION)
    gen_factory._gen_factory.register_gen(gen_four_object.gen_four_soulsilver, const.SOUL_SILVER_VERSION)

    gen_factory.change_version(const.YELLOW_VERSION)