"""Reformat raw_pkmn_data/gen_five/*/trainers.json into the schema the program expects.

The trainer files that were dropped into the gen_five subfolders use a different
shape than the existing gen_4 files (which is what the loader in
pkmn/gen_4/gen_four_object.py — and, by extension, the upcoming gen_5 loader —
expects). This script rewrites them in-place.

Differences fixed:
  * Top-level container is a dict keyed by string ROM ids; b2w2 also has a
    JS-style ``export const trainers = { ... };`` wrapper. We unwrap and emit
    ``{"trainers": [...]}`` instead.
  * Trainer fields ``name`` / ``location`` / ``party`` are renamed to
    ``trainer_name`` / ``trainer_location`` / ``pokemon``. ``trainer_name`` is
    prefixed with the trainer class (matching gen_4 conventions like
    "Youngster Tristan"), with a rom_id suffix appended on collisions so the
    loader's uniqueness check passes.
  * The placeholder rom_id 0 trainer (name "--") is marked
    ``trainer_location: "Unused"`` so the loader skips it.
  * Each pokemon gains an ``ivs`` block (zeroed, matching gen_4 trainer mons)
    and its ``nature`` is converted from a name string ("Quirky") to the
    integer value of the Nature enum (24).
"""

import json
import os
import re
import sys

# Make the repo root importable so we can reuse the canonical Nature enum.
REPO_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), os.pardir))
if REPO_ROOT not in sys.path:
    sys.path.insert(0, REPO_ROOT)

from pkmn.universal_data_objects import Nature  # noqa: E402

GEN5_DIR = os.path.join(REPO_ROOT, "raw_pkmn_data", "gen_five")
SUBFOLDERS = ("black_white", "black2_white2")

STAT_KEYS = ("hp", "attack", "defense", "special_attack", "special_defense", "speed")


def _nature_name_to_int(name):
    if name is None:
        return Nature.HARDY.value
    try:
        return Nature[name.upper()].value
    except KeyError as e:
        raise ValueError(f"Unknown nature name: {name!r}") from e


def _load_raw(path):
    with open(path, "r", encoding="utf-8") as f:
        text = f.read()
    # Strip a leading ``export const <ident> =`` (b2w2) and any trailing ``;``.
    text = re.sub(r"^\s*export\s+const\s+\w+\s*=\s*", "", text)
    text = text.rstrip()
    if text.endswith(";"):
        text = text[:-1]
    return json.loads(text)


def _convert_pokemon(raw_mon):
    return {
        "species": raw_mon["species"],
        "level": raw_mon["level"],
        "experience_yield": raw_mon["experience_yield"],
        "ivs": {k: 0 for k in STAT_KEYS},
        "stats": {k: raw_mon["stats"][k] for k in STAT_KEYS},
        "held_item": raw_mon.get("held_item"),
        "moves": list(raw_mon.get("moves", [])),
        "ability": raw_mon.get("ability"),
        "nature": _nature_name_to_int(raw_mon.get("nature")),
        "forme": None,
    }


def _convert_trainer(raw_trainer):
    trainer_class = raw_trainer["trainer_class"]
    base_name = raw_trainer["name"]
    rom_id = raw_trainer["rom_id"]

    # Treat the rom_id 0 placeholder ("--") as unused so the loader skips it.
    if rom_id == 0 or base_name == "--":
        location = "Unused"
        trainer_name = f"{trainer_class} {base_name} ({rom_id})"
    else:
        location = raw_trainer.get("location")
        trainer_name = f"{trainer_class} {base_name}"

    return {
        "rom_id": rom_id,
        "is_double_battle": raw_trainer.get("is_double_battle", False),
        "trainer_class": trainer_class,
        "trainer_name": trainer_name,
        "trainer_gender": None,
        "battle_type": "Doubles" if raw_trainer.get("is_double_battle") else "Singles",
        "items": list(raw_trainer.get("items", [])),
        "pokemon": [_convert_pokemon(m) for m in raw_trainer.get("party", [])],
        "trainer_location": location,
        "ai_flags": [],
        "money": raw_trainer.get("money", 0),
    }


def _disambiguate_names(trainers):
    """Append a ``(rom_id N)`` suffix to any trainer_name that collides.

    The gen_4 loader keys trainers by ``trainer_name`` and raises if two share
    the same name, so duplicates have to be made unique here.
    """
    seen = {}
    for t in trainers:
        seen.setdefault(t["trainer_name"], []).append(t)
    for name, group in seen.items():
        if len(group) <= 1:
            continue
        for t in group:
            t["trainer_name"] = f"{name} ({t['rom_id']})"


def reformat_file(path):
    raw = _load_raw(path)
    if isinstance(raw, dict) and "trainers" in raw and isinstance(raw["trainers"], list):
        # Already in the target format — nothing to do.
        print(f"  {path}: already reformatted, skipping")
        return

    if isinstance(raw, dict):
        # Iterate in numeric rom_id order so output is stable.
        raw_trainers = [raw[k] for k in sorted(raw.keys(), key=lambda s: int(s))]
    elif isinstance(raw, list):
        raw_trainers = raw
    else:
        raise TypeError(f"Unexpected top-level type in {path}: {type(raw).__name__}")

    converted = [_convert_trainer(t) for t in raw_trainers]
    _disambiguate_names(converted)

    out = {"trainers": converted}
    with open(path, "w", encoding="utf-8") as f:
        json.dump(out, f, indent=4, ensure_ascii=False)
        f.write("\n")
    print(f"  {path}: wrote {len(converted)} trainers")


def main():
    for sub in SUBFOLDERS:
        path = os.path.join(GEN5_DIR, sub, "trainers.json")
        if not os.path.isfile(path):
            print(f"  {path}: not found, skipping")
            continue
        reformat_file(path)


if __name__ == "__main__":
    main()
