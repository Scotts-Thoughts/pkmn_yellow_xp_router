import sys
import os

# Add project root to path so imports work
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import pytest
from utils.constants import const

# Force the import chain to resolve before registering gens.
# controllers.main_controller triggers a full import of routing, pkmn, etc.
# which avoids circular import issues.
import controllers.main_controller  # noqa: F401

from pkmn import gen_factory
from pkmn.gen_1 import gen_one_object
from pkmn.gen_2 import gen_two_object
from pkmn.gen_3 import gen_three_object
from pkmn.gen_4 import gen_four_object
from routing.router import Router


TEST_DATA_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "test_data")


def _register_gens():
    """Register all game generations with the gen factory."""
    try:
        gen_factory._gen_factory.register_gen(gen_one_object.gen_one_red, const.RED_VERSION)
    except ValueError:
        pass  # Already registered
    try:
        gen_factory._gen_factory.register_gen(gen_one_object.gen_one_blue, const.BLUE_VERSION)
    except ValueError:
        pass
    try:
        gen_factory._gen_factory.register_gen(gen_one_object.gen_one_yellow, const.YELLOW_VERSION)
    except ValueError:
        pass
    try:
        gen_factory._gen_factory.register_gen(gen_two_object.gen_two_gold, const.GOLD_VERSION)
    except ValueError:
        pass
    try:
        gen_factory._gen_factory.register_gen(gen_two_object.gen_two_silver, const.SILVER_VERSION)
    except ValueError:
        pass
    try:
        gen_factory._gen_factory.register_gen(gen_two_object.gen_two_crystal, const.CRYSTAL_VERSION)
    except ValueError:
        pass
    try:
        gen_factory._gen_factory.register_gen(gen_three_object.gen_three_ruby, const.RUBY_VERSION)
    except ValueError:
        pass
    try:
        gen_factory._gen_factory.register_gen(gen_three_object.gen_three_sapphire, const.SAPPHIRE_VERSION)
    except ValueError:
        pass
    try:
        gen_factory._gen_factory.register_gen(gen_three_object.gen_three_emerald, const.EMERALD_VERSION)
    except ValueError:
        pass
    try:
        gen_factory._gen_factory.register_gen(gen_three_object.gen_three_fire_red, const.FIRE_RED_VERSION)
    except ValueError:
        pass
    try:
        gen_factory._gen_factory.register_gen(gen_three_object.gen_three_leaf_green, const.LEAF_GREEN_VERSION)
    except ValueError:
        pass
    try:
        gen_factory._gen_factory.register_gen(gen_four_object.gen_four_platinum, const.PLATINUM_VERSION)
    except ValueError:
        pass
    try:
        gen_factory._gen_factory.register_gen(gen_four_object.gen_four_diamond, const.DIAMOND_VERSION)
    except ValueError:
        pass
    try:
        gen_factory._gen_factory.register_gen(gen_four_object.gen_four_pearl, const.PEARL_VERSION)
    except ValueError:
        pass
    try:
        gen_factory._gen_factory.register_gen(gen_four_object.gen_four_heartgold, const.HEART_GOLD_VERSION)
    except ValueError:
        pass
    try:
        gen_factory._gen_factory.register_gen(gen_four_object.gen_four_soulsilver, const.SOUL_SILVER_VERSION)
    except ValueError:
        pass


_register_gens()


def _collect_event_groups(folder):
    """Walk the event tree and return a flat list of all EventGroups in order."""
    from routing import route_events
    results = []
    for child in folder.children:
        if isinstance(child, route_events.EventFolder):
            results.extend(_collect_event_groups(child))
        elif isinstance(child, route_events.EventGroup):
            results.append(child)
    return results


@pytest.fixture
def load_route():
    """Factory fixture that loads a route file and returns (router, all_event_groups)."""
    def _load(filename):
        path = os.path.join(TEST_DATA_DIR, filename)
        router = Router()
        router.load(path)
        events = _collect_event_groups(router.root_folder)
        return router, events
    return _load
