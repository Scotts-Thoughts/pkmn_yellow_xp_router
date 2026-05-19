"""Verify the HGSS trainers.json loads correctly by mimicking the loader's expectations."""
import json
import sys
from pathlib import Path

DEST = Path(r"A:\pkmn_yellow_xp_router\raw_pkmn_data\gen_four\heartgold_soulsilver\trainers.json")
PKMN_DB = Path(r"A:\pkmn_yellow_xp_router\raw_pkmn_data\gen_four\heartgold_soulsilver\pokemon.json")

REQUIRED_TRAINER = ["rom_id", "is_double_battle", "trainer_class", "trainer_name", "pokemon", "trainer_location", "money"]
REQUIRED_MON = ["species", "level", "experience_yield", "ivs", "stats", "held_item", "moves", "ability", "nature"]
STAT_KEYS = ["hp", "attack", "defense", "special_attack", "special_defense", "speed"]


def main():
    data = json.loads(DEST.read_text(encoding="utf-8"))
    pkmn_db = json.loads(PKMN_DB.read_text(encoding="utf-8"))
    known_species = {p["species"] for p in pkmn_db.get("pokemon", pkmn_db.values())}

    trainers = data["trainers"]
    seen_names = {}
    errors = []
    nature_out_of_range = []
    species_unknown = set()

    for t in trainers:
        for key in REQUIRED_TRAINER:
            if key not in t:
                errors.append(f"rom_id={t.get('rom_id')} missing field {key}")
        name = t.get("trainer_name")
        if name in seen_names:
            errors.append(f"duplicate trainer_name {name!r} (rom_ids {seen_names[name]} and {t['rom_id']})")
        else:
            seen_names[name] = t["rom_id"]
        if t.get("trainer_location") == "Unused":
            continue  # would be filtered out by loader
        for i, mon in enumerate(t.get("pokemon", [])):
            for key in REQUIRED_MON:
                if key not in mon:
                    errors.append(f"rom_id={t['rom_id']} mon[{i}] missing field {key}")
            for sk in STAT_KEYS:
                if sk not in mon.get("stats", {}):
                    errors.append(f"rom_id={t['rom_id']} mon[{i}] stats missing {sk}")
                if sk not in mon.get("ivs", {}):
                    errors.append(f"rom_id={t['rom_id']} mon[{i}] ivs missing {sk}")
            nv = mon.get("nature")
            if not isinstance(nv, int) or not (0 <= nv <= 24):
                nature_out_of_range.append((t["rom_id"], i, nv))
            sp = mon.get("species")
            if sp not in known_species:
                species_unknown.add(sp)

    print(f"Trainers: {len(trainers)}")
    print(f"Unique names: {len(seen_names)}")
    print(f"Errors: {len(errors)}")
    for e in errors[:20]:
        print("  ", e)
    if nature_out_of_range:
        print(f"Nature out-of-range: {len(nature_out_of_range)}")
        for r, i, n in nature_out_of_range[:10]:
            print(f"  rom_id={r} mon[{i}] nature={n!r}")
    if species_unknown:
        print(f"Unknown species: {sorted(species_unknown)}")
    return 0 if not errors and not nature_out_of_range and not species_unknown else 1


if __name__ == "__main__":
    sys.exit(main())
