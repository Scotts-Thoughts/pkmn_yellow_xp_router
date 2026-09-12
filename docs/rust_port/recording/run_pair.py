"""Differential recording test: play one scenario to the Python app and to the
Rust app (each against its own mock GameHook), then diff the routes they saved.

    py -3.14 docs/rust_port/recording/run_pair.py --scenario emerald_geodude \
        [--only python|rust] [--work <dir>] [--rust-exe <path>] [--keep]

Each app gets a private global-config dir and user-data dir under the work
dir, a fresh base route (species / version from the scenario module's
``ROUTE`` dict), and a mock GameHook on its own port. Logs, screenshots and
the saved routes are left in the work dir.
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(HERE)))
PY = [sys.executable]


def free_port() -> int:
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def wait_http(url: str, timeout: float = 20) -> bool:
    end = time.time() + timeout
    while time.time() < end:
        try:
            with urllib.request.urlopen(url, timeout=1) as r:
                r.read()
                return True
        except Exception:
            time.sleep(0.2)
    return False


def write_config(cfg_dir: str, data_dir: str, debug: bool):
    os.makedirs(cfg_dir, exist_ok=True)
    os.makedirs(os.path.join(data_dir, "saved_routes"), exist_ok=True)
    cfg = {
        "user_data_location": data_dir,
        "debug_mode": debug,
        "auto_load_most_recent_route": False,
        "recording_auto_stop_enabled": False,
    }
    with open(os.path.join(cfg_dir, "config.json"), "w", encoding="utf-8") as f:
        json.dump(cfg, f, indent=4)


def make_base_route(cfg_dir: str, data_dir: str, route: dict, name: str) -> str:
    """Create the empty starting route with the Python engine (the reference)."""
    script = f"""
import os, sys, json
sys.path.insert(0, {ROOT!r}); os.chdir({ROOT!r})
from utils.constants import const
const.GLOBAL_CONFIG_DIR = {cfg_dir!r}
const.GLOBAL_CONFIG_FILE = os.path.join({cfg_dir!r}, "config.json")
from utils.config_manager import config
import controllers.main_controller  # resolves the import chain (see tests/conftest.py)
from utils import setup
setup.init_base_generations()
from controllers.main_controller import MainController
c = MainController(headless=True)
dvs = {route.get("dvs")!r}
custom = None
if dvs is not None:
    import pkmn.universal_data_objects as u
    custom = u.StatBlock(*dvs)
c.create_new_route({route["species"]!r}, None, {route["version"]!r}, custom_dvs=custom)
c.save_route({name!r})
print("base route saved")
"""
    subprocess.run(PY + ["-c", script], check=True, cwd=ROOT)
    path = os.path.join(data_dir, "saved_routes", f"{name}.json")
    assert os.path.exists(path), path
    return path


def start_mock(scenario_path: str, port: int, log_path: str) -> subprocess.Popen:
    f = open(log_path, "w", encoding="utf-8")
    p = subprocess.Popen(PY + [os.path.join(HERE, "mock_gamehook.py"), "--port", str(port), "--scenario", scenario_path], stdout=f, stderr=subprocess.STDOUT, cwd=HERE)
    if not wait_http(f"http://127.0.0.1:{port}/mock/status"):
        p.kill()
        raise RuntimeError("mock GameHook did not start; see " + log_path)
    return p


def run_python(work: str, scenario_path: str, route: dict, debug: bool, max_secs: float) -> str:
    cfg_dir = os.path.join(work, "python", "config")
    data_dir = os.path.join(work, "python", "data")
    write_config(cfg_dir, data_dir, debug)
    base = make_base_route(cfg_dir, data_dir, route, "base")
    port = free_port()
    mock = start_mock(scenario_path, port, os.path.join(work, "python", "mock.log"))
    env = dict(os.environ)
    env.update({
        "XPR_TEST_CONFIG_DIR": cfg_dir,
        "XPR_TEST_GAMEHOOK_URL": f"http://127.0.0.1:{port}",
        "XPR_TEST_ROUTE": base,
        "XPR_TEST_SAVE_NAME": "recorded",
        "XPR_TEST_MAX_SECS": str(max_secs),
    })
    try:
        with open(os.path.join(work, "python", "app.out"), "w", encoding="utf-8") as out:
            subprocess.run(PY + [os.path.join(HERE, "python_app_harness.py")], env=env, cwd=ROOT, stdout=out, stderr=subprocess.STDOUT, timeout=max_secs + 60)
    finally:
        mock.kill()
    return os.path.join(data_dir, "saved_routes", "recorded.json")


def run_rust(work: str, scenario_path: str, route: dict, debug: bool, max_secs: float, exe: str) -> str:
    cfg_dir = os.path.join(work, "rust", "config")
    data_dir = os.path.join(work, "rust", "data")
    write_config(cfg_dir, data_dir, debug)
    make_base_route(cfg_dir, data_dir, route, "base")
    port = free_port()
    mock = start_mock(scenario_path, port, os.path.join(work, "rust", "mock.log"))
    env = dict(os.environ)
    env.update({
        "XPR_GLOBAL_CONFIG_DIR": cfg_dir,
        "XPR_GAMEHOOK_URL": f"http://127.0.0.1:{port}",
        "XPR_DISABLE_AUTO_UPDATE": "1",
        "XPR_SMOKE_SCREENSHOT": os.path.join(work, "rust", "final.png"),
        "XPR_SMOKE_ACTION": "record",
        "XPR_SMOKE_ROUTE": "base",
        "XPR_SMOKE_SAVE_NAME": "recorded",
        "XPR_SMOKE_RECORD_SECS": str(int(max_secs)),
        "XPR_SMOKE_STOP_URL": f"http://127.0.0.1:{port}/mock/status",
    })
    try:
        with open(os.path.join(work, "rust", "app.out"), "w", encoding="utf-8") as out:
            subprocess.run([exe], env=env, cwd=ROOT, stdout=out, stderr=subprocess.STDOUT, timeout=max_secs + 60)
    finally:
        mock.kill()
    return os.path.join(data_dir, "saved_routes", "recorded.json")


# ----------------------------------------------------------------------------
# comparison
# ----------------------------------------------------------------------------
FOLDER_KEY = "Event Folder Name"


def flatten(route: dict):
    """(folder path, event json) for every folder and event group, depth first."""
    out = []

    def walk(events, path):
        for ev in events:
            if FOLDER_KEY in ev:
                out.append((path, {"folder": ev[FOLDER_KEY], "enabled": ev.get("Enabled"), "expanded": ev.get("Expanded"), "notes": ev.get("Just Notes")}))
                walk(ev.get("events", []), path + [ev[FOLDER_KEY]])
            else:
                out.append((path, ev))

    root = route.get("events")
    if isinstance(root, dict):
        walk([root], [])
    else:
        walk(root or [], [])
    return out


def describe(ev: dict) -> str:
    if "folder" in ev:
        return f"FOLDER {ev['folder']!r} enabled={ev['enabled']} notes={ev['notes']!r}"
    ev = dict(ev)
    # the Super Shuckie timestamp is wall-clock (when the timer is running); only
    # its presence is comparable
    if "Recorded Time" in ev:
        ev["Recorded Time"] = "<set>" if ev["Recorded Time"] is not None else None
    return json.dumps(ev, sort_keys=True)


def compare(py_path: str, rs_path: str) -> int:
    with open(py_path, encoding="utf-8") as f:
        py = json.load(f)
    with open(rs_path, encoding="utf-8") as f:
        rs = json.load(f)
    a = flatten(py)
    b = flatten(rs)
    print(f"python: {len(a)} rows, rust: {len(b)} rows")
    diffs = 0
    for i in range(max(len(a), len(b))):
        pa = a[i] if i < len(a) else None
        pb = b[i] if i < len(b) else None
        da = f"{'/'.join(pa[0])}: {describe(pa[1])}" if pa else "<missing>"
        db = f"{'/'.join(pb[0])}: {describe(pb[1])}" if pb else "<missing>"
        if da != db:
            diffs += 1
            print(f"--- row {i} differs\n  py: {da}\n  rs: {db}")
    # top-level keys other than events
    for k in sorted(set(py) | set(rs)):
        if k == "events":
            continue
        if py.get(k) != rs.get(k):
            diffs += 1
            print(f"--- top-level {k!r} differs\n  py: {json.dumps(py.get(k))[:300]}\n  rs: {json.dumps(rs.get(k))[:300]}")
    if diffs == 0:
        print("routes are identical")
    return diffs


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--scenario", default="emerald_geodude")
    ap.add_argument("--only", choices=["python", "rust"])
    ap.add_argument("--work")
    ap.add_argument("--rust-exe", default=os.path.join(ROOT, "rust", "target", "debug", "xpr-app.exe"))
    ap.add_argument("--max-secs", type=float, default=240)
    ap.add_argument("--no-debug", action="store_true", help="don't turn on recording debug logging")
    ap.add_argument("--compare-only", nargs=2, metavar=("PY_JSON", "RS_JSON"))
    args = ap.parse_args()
    if args.compare_only:
        sys.exit(1 if compare(*args.compare_only) else 0)
    scenario_path = os.path.join(HERE, "scenarios", f"{args.scenario}.py")
    # the scenario module also says which route to record into
    import importlib.util

    spec = importlib.util.spec_from_file_location("scn", scenario_path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    route = getattr(mod, "ROUTE", {"species": "Geodude", "version": "Emerald", "dvs": None})
    work = args.work or tempfile.mkdtemp(prefix="xpr_rec_")
    os.makedirs(work, exist_ok=True)
    print(f"work dir: {work}")
    debug = not args.no_debug
    py_json = rs_json = None
    if args.only in (None, "python"):
        print("=== python app ===")
        py_json = run_python(work, scenario_path, route, debug, args.max_secs)
        print("saved:", py_json, os.path.exists(py_json))
    if args.only in (None, "rust"):
        print("=== rust app ===")
        rs_json = run_rust(work, scenario_path, route, debug, args.max_secs, args.rust_exe)
        print("saved:", rs_json, os.path.exists(rs_json))
    if py_json and rs_json and os.path.exists(py_json) and os.path.exists(rs_json):
        print("=== compare ===")
        sys.exit(1 if compare(py_json, rs_json) else 0)


if __name__ == "__main__":
    main()
