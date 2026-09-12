"""Battle-summary baseline: BattleSummaryController.load_from_event on the biggest fights."""
import os, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import largest_routes

import controllers.main_controller  # noqa: F401
from utils import setup
setup.init_base_generations()
from controllers.main_controller import MainController
from controllers.battle_summary_controller import BattleSummaryController
from routing import route_events


def groups(folder):
    out = []
    for child in folder.children:
        if isinstance(child, route_events.EventFolder):
            out.extend(groups(child))
        else:
            out.append(child)
    return out


for path in largest_routes():
    mc = MainController(headless=True)
    mc.load_route(path)
    exc = mc.get_next_exception_info()
    if exc:
        print("EXCEPTION", exc)
    bsc = BattleSummaryController(mc)
    fights = [g for g in groups(mc.get_raw_route().root_folder) if g.event_definition.trainer_def is not None]
    fights.sort(key=lambda g: len(g.event_definition.get_pokemon_list()), reverse=True)
    print(f"== {os.path.basename(path)}: {len(fights)} trainer fights")
    for g in fights[:3]:
        mons = len(g.event_definition.get_pokemon_list())
        times = []
        for _ in range(3):
            t = time.perf_counter(); bsc.load_from_event(g); times.append(round((time.perf_counter() - t) * 1000, 1))
        print(f"   {str(g.name)[:40]:40s} mons={mons} load_from_event={times} ms")
    t = time.perf_counter()
    for g in fights:
        bsc.load_from_event(g)
    print(f"   all {len(fights)} fights sequentially: {time.perf_counter() - t:.2f}s")
