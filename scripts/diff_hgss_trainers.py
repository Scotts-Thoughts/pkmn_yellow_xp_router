"""Report which trainer pokemon have changed stats between trainers.json and trainers.json.bak."""
import json
from pathlib import Path

NEW = Path(r"A:\pkmn_yellow_xp_router\raw_pkmn_data\gen_four\heartgold_soulsilver\trainers.json")
OLD = Path(r"A:\pkmn_yellow_xp_router\raw_pkmn_data\gen_four\heartgold_soulsilver\trainers.json.bak")

STAT_KEYS = ["hp", "attack", "defense", "special_attack", "special_defense", "speed"]


def main():
    new = json.loads(NEW.read_text(encoding="utf-8"))
    old = json.loads(OLD.read_text(encoding="utf-8"))
    old_by_rom = {t["rom_id"]: t for t in old["trainers"]}

    stat_diff_trainers = 0
    stat_diff_mons = 0
    other_diff_mons = 0
    samples = []
    for nt in new["trainers"]:
        ot = old_by_rom.get(nt["rom_id"])
        if ot is None:
            continue
        nm = nt["pokemon"]
        om = ot["pokemon"]
        trainer_dirty = False
        for i in range(min(len(nm), len(om))):
            nmon = nm[i]
            omon = om[i]
            stat_changed = any(nmon["stats"][k] != omon["stats"][k] for k in STAT_KEYS)
            other_changed = (
                nmon.get("level") != omon.get("level")
                or nmon.get("experience_yield") != omon.get("experience_yield")
                or nmon.get("moves") != omon.get("moves")
                or nmon.get("ability") != omon.get("ability")
                or nmon.get("held_item") != omon.get("held_item")
                or nmon.get("nature") != omon.get("nature")
                or nmon.get("species") != omon.get("species")
            )
            if stat_changed:
                stat_diff_mons += 1
                trainer_dirty = True
                if len(samples) < 15:
                    delta = {k: (omon["stats"][k], nmon["stats"][k]) for k in STAT_KEYS if omon["stats"][k] != nmon["stats"][k]}
                    samples.append((nt["trainer_name"], i, nmon["species"], delta))
            if other_changed:
                other_diff_mons += 1
        if trainer_dirty:
            stat_diff_trainers += 1

    print(f"Trainers with stat changes: {stat_diff_trainers}")
    print(f"Pokemon with stat changes: {stat_diff_mons}")
    print(f"Pokemon with non-stat changes (level/moves/ability/etc): {other_diff_mons}")
    print()
    print("Sample stat changes (old -> new):")
    for name, idx, species, delta in samples:
        print(f"  {name} mon[{idx}] {species}: {delta}")


if __name__ == "__main__":
    main()
