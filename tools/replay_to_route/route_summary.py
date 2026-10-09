"""One line per event of a router route file, for reading or diffing routes.

    py -3.14 route_summary.py <route.json>                 print the events
    py -3.14 route_summary.py <a.json> <b.json>            unified diff of the two
    py -3.14 route_summary.py --no-folders <a> <b>         ... ignoring folder names

Each line is `folder | type | the event's fields`, with the fields every
event of that type carries by default left out.
"""

import difflib
import json
import sys

COMMON = {"Enabled", "Recorded Time", "Split Time", "Tags", "Notes", "Just Notes", "Expanded"}

# fight fields that are only ever set by hand in the editor
FIGHT_DEFAULTS = {
    "second_trainer_name": "", "verbose": False, "setup_moves": [], "enemy_setup_moves": [],
    "player_field_moves": [], "enemy_field_moves": [], "mimic_selection": "", "custom_move_data": [],
    "exp_split": [], "weather": "None", "weather_source_mon_idx": None, "player_screens": None,
    "enemy_screens": None, "player_intimidate": None, "enemy_intimidate": None, "pay_day_amount": 0,
    "mon_order": [], "transformed": False, "stat_stage_setup": None, "collapsed_mons": None,
}


def describe(event):
    parts = []
    for key, value in event.items():
        if key in COMMON:
            continue
        if isinstance(value, dict):
            if key == "Fight Trainer":
                value = {k: v for k, v in value.items() if FIGHT_DEFAULTS.get(k, object()) != v}
            parts.append(f"{key} {json.dumps(value, sort_keys=True, ensure_ascii=False)}")
        else:
            parts.append(f"{key} {json.dumps(value, ensure_ascii=False)}")
    notes = event.get("Notes") or ""
    if notes:
        parts.append(f"notes={notes!r}")
    return "; ".join(parts) if parts else "(empty)"


def walk(folder, path, out, with_folders):
    for item in folder.get("events", []):
        if "Event Folder Name" in item:
            walk(item, path + [item["Event Folder Name"]], out, with_folders)
        else:
            prefix = "/".join(path[1:]) + " | " if with_folders else ""
            enabled = "" if item.get("Enabled", True) else "[disabled] "
            out.append(prefix + enabled + describe(item))


def lines(path, with_folders=True):
    with open(path, encoding="utf-8") as f:
        route = json.load(f)
    out = [f"route {route.get('name')} {route.get('Version')} dv={route.get('dv')} ability={route.get('ability')} nature={route.get('nature')}"]
    for root in route.get("events", []):
        walk(root, [root.get("Event Folder Name", "")], out, with_folders)
    for lm in route.get("Learn Levelup Move", []):
        out.append(f"levelup-move {json.dumps(lm, sort_keys=True, ensure_ascii=False)}")
    return out


def main(argv):
    with_folders = True
    if "--no-folders" in argv:
        argv.remove("--no-folders")
        with_folders = False
    if len(argv) == 1:
        for line in lines(argv[0], with_folders):
            print(line)
    elif len(argv) == 2:
        a, b = lines(argv[0], with_folders), lines(argv[1], with_folders)
        diff = list(difflib.unified_diff(a, b, argv[0], argv[1], lineterm="", n=1))
        for line in diff:
            print(line)
        print(f"# {len(a)} vs {len(b)} lines, {sum(1 for l in diff if l[:1] in '+-' and l[:3] not in ('+++', '---'))} changed")
    else:
        print(__doc__)
        return 2
    return 0


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    sys.exit(main(sys.argv[1:]))
