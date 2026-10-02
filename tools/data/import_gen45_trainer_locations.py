#!/usr/bin/env python3
"""Copy the gen 4/5 trainer locations from the data_objects repo into the router.

    python3 tools/data/import_gen45_trainer_locations.py            # rewrite the files
    python3 tools/data/import_gen45_trainer_locations.py --check    # report only

data_objects (`~/Documents/data_objects`, or $DATA_OBJECTS) fills each trainer's `location`
with `generate_gen4_5_trainer_locations.py`: the in-game place name, several joined with " / "
(e.g. Platinum's daily Pokémon Center trainers), or null when the record is unused, a
placeholder or can't be placed. Its records are keyed by ROM trainer id, which is the router's
`rom_id`, so the join ignores names (the router names trainers its own way).

Only `trainer_location` changes; every file is rewritten in its own formatting. A router record
marked "Unused" keeps that (the loader skips those records).
"""
import argparse
import json
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DATA_OBJECTS = Path(os.environ.get("DATA_OBJECTS", Path.home() / "Documents" / "data_objects"))
GAMES = [
    ("diamond_pearl", "gen_four"),
    ("platinum", "gen_four"),
    ("heartgold_soulsilver", "gen_four"),
    ("black_white", "gen_five"),
    ("black2_white2", "gen_five"),
]
UNUSED = "Unused"


def load_js_object(path):
    """data_objects' `export const trainers = {...};` (JSON inside)."""
    s = path.read_text(encoding="utf-8")
    return json.loads(s[s.index("{"):s.rindex("}") + 1])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="report, write nothing")
    args = ap.parse_args()
    changed_any = False
    for game, gen_dir in GAMES:
        src = load_js_object(DATA_OBJECTS / "trainers" / f"{game}.js")
        locations = {int(t["rom_id"]): t.get("location") for t in src.values()}
        path = ROOT / "raw_pkmn_data" / gen_dir / game / "trainers.json"
        text = path.read_text(encoding="utf-8")
        data = json.loads(text)
        changed = located = missing = 0
        for t in data["trainers"]:
            rom_id = int(t["rom_id"])
            if rom_id not in locations:
                missing += 1
                continue
            if t.get("trainer_location") == UNUSED:
                continue
            new = locations[rom_id] or None
            located += new is not None
            if t.get("trainer_location") != new:
                t["trainer_location"] = new
                changed += 1
        out = json.dumps(data, indent=4, ensure_ascii=False) + ("\n" if text.endswith("\n") else "")
        print(f"{game}: {located}/{len(data['trainers'])} trainers located, {changed} changed"
              + (f", {missing} rom_ids not in data_objects" if missing else ""))
        if out != text:
            changed_any = True
            if not args.check:
                path.write_text(out, encoding="utf-8")
    if args.check and changed_any:
        sys.exit(1)


if __name__ == "__main__":
    main()
