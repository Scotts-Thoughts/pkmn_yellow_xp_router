"""Exercise the landing page's "Start Recording" against the mock GameHook.

Starts the mock in shared-playback mode, launches the Rust app with no route
loaded and ``XPR_SMOKE_ACTION=quickstart`` (it presses "Start Recording",
records until the mock is done, saves the route, screenshots and exits), then
checks the saved route: version and solo mon from the mapper / the first
Pokémon, its DVs, and that events were recorded after the hand-off.

    py -3.14 docs/rust_port/recording/run_quickstart.py [--scenario quickstart_yellow] [--work DIR]
"""
from __future__ import annotations

import argparse
import importlib.util
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
FOLDER_KEY = "Event Folder Name"


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


def load_expected(scenario_path: str) -> dict:
    spec = importlib.util.spec_from_file_location("qs_scenario", scenario_path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod.EXPECTED


def start_mock(scenario_path: str, port: int, log_path: str) -> subprocess.Popen:
    f = open(log_path, "w", encoding="utf-8")
    p = subprocess.Popen(
        PY + [os.path.join(HERE, "mock_gamehook.py"), "--port", str(port), "--scenario", scenario_path, "--shared-playback"],
        stdout=f, stderr=subprocess.STDOUT, cwd=HERE,
    )
    if not wait_http(f"http://127.0.0.1:{port}/mock/status"):
        p.kill()
        raise RuntimeError("mock GameHook did not start; see " + log_path)
    return p


def run(work: str, scenario_path: str, exe: str, max_secs: float) -> str:
    cfg_dir = os.path.join(work, "config")
    data_dir = os.path.join(work, "data")
    write_config(cfg_dir, data_dir, debug=True)
    port = free_port()
    mock = start_mock(scenario_path, port, os.path.join(work, "mock.log"))
    env = dict(os.environ)
    env.update({
        "XPR_GLOBAL_CONFIG_DIR": cfg_dir,
        "XPR_GAMEHOOK_URL": f"http://127.0.0.1:{port}",
        "XPR_DISABLE_AUTO_UPDATE": "1",
        "XPR_SMOKE_SCREENSHOT": os.path.join(work, "final.png"),
        "XPR_SMOKE_ACTION": "quickstart",
        "XPR_SMOKE_SAVE_NAME": "recorded",
        "XPR_SMOKE_RECORD_SECS": str(int(max_secs)),
        "XPR_SMOKE_STOP_URL": f"http://127.0.0.1:{port}/mock/status",
    })
    started = time.time()
    try:
        with open(os.path.join(work, "app.out"), "w", encoding="utf-8") as out:
            subprocess.run([exe], env=env, cwd=ROOT, stdout=out, stderr=subprocess.STDOUT, timeout=max_secs + 60)
    finally:
        mock.kill()
    print(f"app ran {time.time() - started:.0f}s")
    return os.path.join(data_dir, "saved_routes", "recorded.json")


def check(route_path: str, expected: dict) -> int:
    if not os.path.exists(route_path):
        print("FAIL: no route was saved:", route_path)
        return 1
    with open(route_path, encoding="utf-8") as f:
        route = json.load(f)
    problems = []
    version = route.get("Version")
    if version != expected["version"]:
        problems.append(f"version {version!r} != {expected['version']!r}")
    solo = route.get("name")
    if solo != expected["species"]:
        problems.append(f"solo mon {solo!r} != {expected['species']!r}")
    dvs = route.get("dv") or {}
    for key, want in expected["dvs"].items():
        got = dvs.get(key)
        if got != want:
            problems.append(f"DV {key}: {got!r} != {want!r}")
    for key in ("ability", "nature"):
        if key in expected and route.get(key) != expected[key]:
            problems.append(f"{key}: {route.get(key)!r} != {expected[key]!r}")
    events = [e for _, e in flatten(route) if "folder" not in e]
    kinds = sorted({k for ev in events for k in ev.keys() if k not in ("Enabled", "Recorded Time", "Notes", "Tags")})
    if not events:
        problems.append("no events were recorded after the hand-off")
    print(f"route: version={version!r} solo={solo!r} dvs={dvs} ability={route.get('ability')!r} nature={route.get('nature')!r} events={len(events)} keys={kinds}")
    if problems:
        print("FAIL:")
        for p in problems:
            print("  -", p)
        return 1
    print("OK")
    return 0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--scenario", default="quickstart_yellow")
    ap.add_argument("--work", default=None)
    ap.add_argument("--rust-exe", default=os.path.join(ROOT, "rust", "target", "debug", "xpr-app.exe"))
    ap.add_argument("--max-secs", type=float, default=90)
    ap.add_argument("--keep", action="store_true")
    args = ap.parse_args()
    scenario_path = os.path.join(HERE, "scenarios", f"{args.scenario}.py")
    expected = load_expected(scenario_path)
    work = args.work or tempfile.mkdtemp(prefix="xpr_quickstart_")
    os.makedirs(work, exist_ok=True)
    print("work dir:", work)
    route = run(work, scenario_path, args.rust_exe, args.max_secs)
    rc = check(route, expected)
    if rc == 0 and not args.keep and args.work is None:
        shutil.rmtree(work, ignore_errors=True)
    sys.exit(rc)


if __name__ == "__main__":
    main()
