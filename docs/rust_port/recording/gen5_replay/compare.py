"""Compare a recorded route with trace ground truth.

py -3.14 compare.py <route.json> <gt.json> <from_frame> <to_frame>
"""
import difflib
import json
import sys

sys.stdout.reconfigure(encoding="utf-8")
route = json.load(open(sys.argv[1], encoding="utf-8"))
gt = json.load(open(sys.argv[2], encoding="utf-8"))
lo, hi = int(sys.argv[3]), int(sys.argv[4])
gt = [e for e in gt if lo <= e["frame"] <= hi]


def flatten(evs, folder=None, out=None):
    out = [] if out is None else out
    for e in evs:
        if "Event Folder Name" in e:
            flatten(e.get("events", []), e["Event Folder Name"], out)
        else:
            out.append((folder, e))
    return out


ev = route["events"]
events = flatten(ev if isinstance(ev, list) else [ev])

rec_trainers = [(f, e["Fight Trainer"]) for f, e in events if "Fight Trainer" in e]
gt_trainers = [e for e in gt if e["kind"] == "trainer" and not e["lost"]]
gt_lost = [e for e in gt if e["kind"] == "trainer" and e["lost"]]
a = [t["trainer_name"] for _, t in rec_trainers]
b = [e["name"] for e in gt_trainers]
print(f"trainers: recorded {len(a)}, ground truth {len(b)} (+{len(gt_lost)} lost)")
for line in difflib.unified_diff(b, a, "ground_truth", "recorded", lineterm="", n=0):
    print("  ", line)
# per-trainer details
gt_by_name = {}
for e in gt_trainers:
    gt_by_name.setdefault(e["name"], []).append(e)
for folder, t in rec_trainers:
    extra = {k: t[k] for k in ("exp_split", "mon_order", "pay_day_amount", "second_trainer_name", "thief_mons") if t.get(k) not in (None, [], "", 0)}
    g = gt_by_name.get(t["trainer_name"], [None])[0]
    order = None
    if g and g["fainted"]:
        order = [s for s, _, _ in g["fainted"]]
    note = ""
    if order and order != sorted(order):
        expected = [order.index(i) + 1 for i in sorted(order)] if len(order) == max(order) + 1 else None
        note = f" gt faint order {order} -> expect mon_order {expected}"
    if extra or note:
        print(f"  [{folder}] {t['trainer_name']} {json.dumps(extra, ensure_ascii=False)}{note}")

wild_rec = [e["Fight Wild Pkmn"] for _, e in events if "Fight Wild Pkmn" in e]
wild_gt = [(sp, lv) for e in gt if e["kind"] == "wild" for _, sp, lv in e["fainted"]]
print(f"wild: recorded {len(wild_rec)} {wild_rec[:10]}, ground truth {len(wild_gt)} {wild_gt[:10]}")
kinds = {}
for _, e in events:
    for k in e:
        if k in ("Recorded Time", "Split Time", "Tags", "Enabled"):
            continue
        kinds[k] = kinds.get(k, 0) + 1
print("event kinds:", kinds)
for key in ("Blackout", "Game Save", "PkmnCenter Heal", "Evolution", "Learn TM/HM Move", "Use Rare Candy", "Use Vitamin", "Hold Item", "Just Notes"):
    hits = [(f, e[key]) for f, e in events if key in e]
    if hits:
        print(f"{key}: " + "; ".join(f"[{f}] {v}" for f, v in hits[:40]))
notes = [(f, e.get("Notes") or e.get("notes")) for f, e in events if e.get("Notes") or e.get("Just Notes")]
if notes:
    print("notes:", notes[:10])
