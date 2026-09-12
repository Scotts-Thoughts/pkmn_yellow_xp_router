"""Shared helpers for the Rust-port baseline benchmarks.

Locates the app's saved_routes directory from config.json (or argv[1]) and
returns the N largest route files there.  Run from the repo root with
``py -3.14 docs/rust_port/bench/<script>.py [saved_routes_dir]``.
"""
import json
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)


def saved_routes_dir():
    if len(sys.argv) > 1:
        return sys.argv[1]
    import appdirs
    cfg = os.path.join(appdirs.user_data_dir(appname="pkmn_xp_router", appauthor="pkmn_xp_router"), "config.json")
    with open(cfg) as f:
        user_dir = json.load(f)["user_data_location"]
    return os.path.join(user_dir, "saved_routes")


def largest_routes(n=3):
    d = saved_routes_dir()
    files = [os.path.join(d, x) for x in os.listdir(d) if x.endswith(".json")]
    files.sort(key=os.path.getsize, reverse=True)
    return files[:n]
