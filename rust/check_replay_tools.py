"""Fail unless the replay tools bundle in rust/replay_tools.json is on a release.

The app downloads that exact file the first time it makes a route from a replay. It can't be built
in CI (the worker needs Super Shuckie's MinGW emulator cores and the local mappers), so it is made
by tools/replay_to_route/bundle.py and uploaded by hand. A version released without it would fail
for everyone, so the release workflow runs this first.

    python rust/check_replay_tools.py
"""
from __future__ import annotations

import json
import os
import subprocess

LOCK = os.path.join(os.path.dirname(os.path.abspath(__file__)), "replay_tools.json")
REPO = "Scotts-Thoughts/pkmn_yellow_xp_router"


def main():
    lock = json.load(open(LOCK, encoding="utf-8"))
    asset = lock["asset"]
    sizes = subprocess.run(
        ["gh", "api", "--paginate", f"repos/{REPO}/releases?per_page=100", "--jq", f'.[].assets[] | select(.name == "{asset}") | .size'],
        check=True, capture_output=True, text=True,
    ).stdout.split()
    if not sizes:
        raise SystemExit(f"{asset} is not on any release: upload it (tools/replay_to_route/bundle.py prints how)")
    if int(sizes[0]) != lock["bytes"]:
        raise SystemExit(f"{asset} on GitHub is {sizes[0]} bytes; replay_tools.json says {lock['bytes']}")
    print(f"{asset} is on a release ({lock['bytes'] / 1e6:.1f} MB)")


if __name__ == "__main__":
    main()
