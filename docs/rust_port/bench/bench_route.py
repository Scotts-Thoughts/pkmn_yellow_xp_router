"""Engine baseline: json.load / Router.load / full recalc / save on the largest routes."""
import os, sys, time, json, tempfile
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import largest_routes, ROOT

t0 = time.perf_counter()
import controllers.main_controller  # noqa: F401  (resolves the import cycle)
from utils import setup
setup.init_base_generations()
print(f"import + init_base_generations: {time.perf_counter() - t0:.3f}s")

from routing.router import Router
from routing import route_events
from utils.constants import const


def count(folder):
    groups = folders = 0
    for child in folder.children:
        if isinstance(child, route_events.EventFolder):
            folders += 1
            g, f = count(child)
            groups += g
            folders += f
        else:
            groups += 1
    return groups, folders


for path in largest_routes():
    size = os.path.getsize(path)
    t = time.perf_counter()
    with open(path) as fh:
        json.load(fh)
    t_json = time.perf_counter() - t
    router = Router()
    t = time.perf_counter(); router.load(path); t_load = time.perf_counter() - t
    groups, folders = count(router.root_folder)
    recalc = []
    for _ in range(3):
        t = time.perf_counter(); router._recalc(); recalc.append(round(time.perf_counter() - t, 3))
    # save() writes into const.SAVED_ROUTES_DIR; redirect to a temp dir so nothing is touched
    orig = const.SAVED_ROUTES_DIR
    const.SAVED_ROUTES_DIR = tempfile.mkdtemp()
    try:
        t = time.perf_counter(); router.save("bench_out"); t_save = time.perf_counter() - t
    finally:
        const.SAVED_ROUTES_DIR = orig
    print(f"{os.path.basename(path)}: {size/1e6:.2f} MB  json.load={t_json:.3f}s  router.load={t_load:.3f}s  "
          f"groups={groups} folders={folders}  recalc={recalc}  save={t_save:.3f}s")
