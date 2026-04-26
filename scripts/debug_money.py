"""Debug helper: load a route and report money state per event, flagging errors."""
import sys
import os

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

# Force the import chain to resolve before registering gens (mirrors tests/conftest.py).
import controllers.main_controller  # noqa: F401

from utils.constants import const
from pkmn import gen_factory
from pkmn.gen_1 import gen_one_object
from pkmn.gen_2 import gen_two_object
from pkmn.gen_3 import gen_three_object
from pkmn.gen_4 import gen_four_object
from routing.router import Router
from routing import route_events


def _safe_register(gen, name):
    try:
        gen_factory._gen_factory.register_gen(gen, name)
    except ValueError:
        pass


_safe_register(gen_one_object.gen_one_red, const.RED_VERSION)
_safe_register(gen_one_object.gen_one_blue, const.BLUE_VERSION)
_safe_register(gen_one_object.gen_one_yellow, const.YELLOW_VERSION)
_safe_register(gen_two_object.gen_two_gold, const.GOLD_VERSION)
_safe_register(gen_two_object.gen_two_silver, const.SILVER_VERSION)
_safe_register(gen_two_object.gen_two_crystal, const.CRYSTAL_VERSION)
_safe_register(gen_three_object.gen_three_ruby, const.RUBY_VERSION)
_safe_register(gen_three_object.gen_three_sapphire, const.SAPPHIRE_VERSION)
_safe_register(gen_three_object.gen_three_emerald, const.EMERALD_VERSION)
_safe_register(gen_three_object.gen_three_fire_red, const.FIRE_RED_VERSION)
_safe_register(gen_three_object.gen_three_leaf_green, const.LEAF_GREEN_VERSION)
_safe_register(gen_four_object.gen_four_platinum, const.PLATINUM_VERSION)
_safe_register(gen_four_object.gen_four_diamond, const.DIAMOND_VERSION)
_safe_register(gen_four_object.gen_four_pearl, const.PEARL_VERSION)
_safe_register(gen_four_object.gen_four_heartgold, const.HEART_GOLD_VERSION)
_safe_register(gen_four_object.gen_four_soulsilver, const.SOUL_SILVER_VERSION)
gen_factory.change_version(const.PLATINUM_VERSION)


ROUTE_PATH = r"A:/Dropbox/stp-projects/programs/router_data/saved_routes/p-staraptor-line-2-13930.json"


def walk(obj, depth=0, path=""):
    if isinstance(obj, route_events.EventFolder):
        for child in obj.children:
            yield from walk(child, depth + 1, path + "/" + obj.name)
    elif isinstance(obj, route_events.EventGroup):
        yield path, obj
    else:
        pass


def main():
    r = Router()
    r.load(ROUTE_PATH)
    print(f"Loaded route. Final state money: {r.get_final_state().inventory.cur_money}")

    last_money = 3000
    error_count = 0
    for path, group in walk(r.root_folder):
        cur_money = group.final_state.inventory.cur_money if group.final_state else None
        delta = (cur_money - last_money) if cur_money is not None else None
        err = group.error_message if hasattr(group, "error_message") else ""
        # also check inner items
        item_errors = []
        for it in getattr(group, "event_items", []) or []:
            if it.error_message:
                item_errors.append(it.error_message)
        all_err = err
        if item_errors:
            all_err = (all_err + " | " if all_err else "") + " | ".join(item_errors)

        label = group.event_definition.get_label() if group.event_definition else "?"
        marker = ""
        if all_err:
            marker = "  <-- ERROR"
            error_count += 1
        print(f"{path} :: {label} | money={cur_money} (delta {delta}) {marker}{(' ' + all_err) if all_err else ''}")
        if cur_money is not None:
            last_money = cur_money

    print(f"\nTotal errors: {error_count}")


if __name__ == "__main__":
    main()
