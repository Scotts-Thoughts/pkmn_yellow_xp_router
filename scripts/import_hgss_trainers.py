"""One-shot converter: import HGSS trainer data from the canonical source .js file
into our raw_pkmn_data trainers.json, preserving metadata (trainer_name disambiguation,
ivs, ai_flags, money, items, trainer_location, etc.) by matching on rom_id.

Run from repo root:
    python scripts/import_hgss_trainers.py
"""
import json
import re
import sys
from pathlib import Path

SRC_PATH = Path(r"A:\Dropbox\stp-projects\programs\data_objects\trainers\heartgold_soulsilver.js")
DEST_PATH = Path(r"A:\pkmn_yellow_xp_router\raw_pkmn_data\gen_four\heartgold_soulsilver\trainers.json")

NATURE_MAP = {
    "Hardy": 0, "Lonely": 1, "Brave": 2, "Adamant": 3, "Naughty": 4,
    "Bold": 5, "Docile": 6, "Relaxed": 7, "Impish": 8, "Lax": 9,
    "Timid": 10, "Hasty": 11, "Serious": 12, "Jolly": 13, "Naive": 14,
    "Modest": 15, "Mild": 16, "Quiet": 17, "Bashful": 18, "Rash": 19,
    "Calm": 20, "Gentle": 21, "Sassy": 22, "Careful": 23, "Quirky": 24,
}
DEFAULT_IVS = {"hp": 3, "attack": 3, "defense": 3, "special_attack": 3, "special_defense": 3, "speed": 3}


def parse_source_js(path: Path) -> dict:
    text = path.read_text(encoding="utf-8")
    m = re.match(r"\s*export\s+const\s+\w+\s*=\s*", text)
    if not m:
        raise ValueError("Source file does not start with 'export const ... ='")
    body = text[m.end():].rstrip().rstrip(";").rstrip()
    return json.loads(body)


def main() -> int:
    src = parse_source_js(SRC_PATH)
    existing = json.loads(DEST_PATH.read_text(encoding="utf-8"))
    existing_by_rom = {t["rom_id"]: t for t in existing["trainers"]}

    new_trainers = []
    missing_in_existing = []
    nature_unknown = set()

    for src_t in src.values():
        rom_id = src_t["rom_id"]
        if rom_id == 0:
            continue  # Player placeholder, not a real trainer in our schema
        existing_t = existing_by_rom.get(rom_id)
        if existing_t is None:
            missing_in_existing.append((rom_id, src_t.get("name")))
            continue

        new_t = {
            "rom_id": rom_id,
            "is_double_battle": src_t.get("is_double_battle", existing_t.get("is_double_battle", False)),
            "trainer_class": src_t.get("trainer_class", existing_t.get("trainer_class")),
            "trainer_name": existing_t["trainer_name"],
            "trainer_gender": existing_t.get("trainer_gender"),
            "battle_type": existing_t.get("battle_type"),
            "items": existing_t.get("items", []),
            "pokemon": [],
            "trainer_location": existing_t.get("trainer_location"),
            "ai_flags": existing_t.get("ai_flags", []),
            "money": existing_t.get("money", 0),
        }
        if "refightable" in existing_t:
            new_t["refightable"] = existing_t["refightable"]

        existing_party = existing_t.get("pokemon", [])
        for i, src_mon in enumerate(src_t.get("party", [])):
            existing_mon = existing_party[i] if i < len(existing_party) else {}
            nature_raw = src_mon.get("nature")
            if isinstance(nature_raw, str):
                if nature_raw not in NATURE_MAP:
                    nature_unknown.add(nature_raw)
                nature_int = NATURE_MAP.get(nature_raw, 0)
            elif isinstance(nature_raw, int):
                nature_int = nature_raw
            else:
                nature_int = existing_mon.get("nature", 0)

            stats = src_mon.get("stats", {})
            new_mon = {
                "species": src_mon["species"],
                "level": src_mon["level"],
                "experience_yield": src_mon["experience_yield"],
                "ivs": existing_mon.get("ivs", DEFAULT_IVS),
                "stats": {
                    "hp": stats["hp"],
                    "attack": stats["attack"],
                    "defense": stats["defense"],
                    "special_attack": stats["special_attack"],
                    "special_defense": stats["special_defense"],
                    "speed": stats["speed"],
                },
                "held_item": src_mon.get("held_item"),
                "moves": src_mon.get("moves", []),
                "ability": src_mon.get("ability"),
                "nature": nature_int,
                "forme": existing_mon.get("forme"),
            }
            new_t["pokemon"].append(new_mon)

        new_trainers.append(new_t)

    new_trainers.sort(key=lambda t: t["rom_id"])

    # Sanity: unique trainer_names
    seen_names = {}
    dupes = []
    for t in new_trainers:
        if t["trainer_name"] in seen_names:
            dupes.append((t["trainer_name"], seen_names[t["trainer_name"]], t["rom_id"]))
        else:
            seen_names[t["trainer_name"]] = t["rom_id"]

    DEST_PATH.write_text(
        json.dumps({"trainers": new_trainers}, indent=4),
        encoding="utf-8",
    )

    print(f"Wrote {len(new_trainers)} trainers to {DEST_PATH}")
    if missing_in_existing:
        print(f"WARNING: {len(missing_in_existing)} source trainers had no matching rom_id in existing (skipped):")
        for rid, name in missing_in_existing[:30]:
            print(f"  rom_id={rid} name={name}")
    if nature_unknown:
        print(f"WARNING: unknown nature strings encountered: {sorted(nature_unknown)}")
    if dupes:
        print(f"ERROR: duplicate trainer_names detected (loader will fail):")
        for name, a, b in dupes[:30]:
            print(f"  {name!r}: rom_id {a} and {b}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
