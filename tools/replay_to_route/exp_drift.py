"""Look for exp drift a passing end-of-run check cannot see.

    py -3.14 exp_drift.py <route.json> <import log folder>

The end-of-run check compares the route with the game once. A Rare Candy sets
exp to its level's minimum in both, so any error before it is gone by then.
This reads the import's `exp check` lines instead and reports:

* how often route and game agreed, and the longest stretch of frames they did
  not (a battle's exp reaches the route before the game, so a stretch the
  length of a fight is normal);
* the gap just before each Use Rare Candy, which must be +0 (the game moves
  first, so the candy's own steps show up as negative differences and are
  skipped).

Exits 1 if a candy has a gap: that much exp was wrong and nothing else would
have shown it. Needs a log from a build that checks when the game's exp moves.
"""

import glob
import json
import os
import re
import sys

LINE = re.compile(r"exp check at frame (\d+) \((\d+) events\): route (-?\d+), game (-?\d+), difference ([+-]?\d+)")


def flat(folder, out):
    for item in folder.get("events", []):
        if "Event Folder Name" in item:
            flat(item, out)
        else:
            out.append(item)
    return out


def checks(log_dir):
    rows = set()
    for path in glob.glob(os.path.join(log_dir, "pkmn_router_logs.log*")):
        with open(path, errors="replace") as f:
            for line in f:
                m = LINE.search(line)
                if m:
                    rows.add(tuple(int(x) for x in m.groups()))
    return sorted(rows)


def main(argv):
    if len(argv) != 2:
        print(__doc__)
        return 2
    with open(argv[0], encoding="utf-8") as f:
        route = json.load(f)
    events = []
    for root in route.get("events", []):
        flat(root, events)
    rows = checks(argv[1])
    if not rows:
        print("no exp check lines in that folder")
        return 2

    zero = [r for r in rows if r[4] == 0]
    longest, start = (0, 0), 0
    for r in zero + [rows[-1]]:
        if r[0] - start > longest[0]:
            longest = (r[0] - start, start)
        start = r[0]
    print(f"{len(rows)} checks, {len(zero)} exact, longest stretch not exact: {longest[0]} frames from frame {longest[1]}")

    gaps = candies = 0
    for i, event in enumerate(events):
        if "Use Rare Candy" not in event:
            continue
        before = [r for r in rows if r[1] <= i and r[4] >= 0]
        if not before:
            continue
        candies += 1
        frame, _, route_xp, game_xp, diff = before[-1]
        if diff != 0:
            gaps += 1
            print(f"event {i}: {event['Use Rare Candy']} candies after a gap of {diff:+} (frame {frame}: route {route_xp}, game {game_xp})")
    print(f"{candies} candy uses, {gaps} with a gap before them")
    return 1 if gaps else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
