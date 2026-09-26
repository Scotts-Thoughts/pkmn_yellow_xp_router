"""Ground truth from a Black/White trace: battles (trainer / wild, fainted enemies,
won or lost), levels, bag and money changes.

py -3.14 gt.py <trace> [from] [to] [--json out.json] [--game black|white_2]
"""
import json
import sys
from g5 import Trace, pkm_info, bag_items, refs, u16, u32, fmt_time

sys.stdout.reconfigure(encoding="utf-8")
GAME = sys.argv[sys.argv.index("--game") + 1] if "--game" in sys.argv else "black"
# per game: the trainer id's offset in the `opp` region, the router's trainer data set
OPP_ID, DATASET = {"black": (0x0E, "black_white"), "white_2": (0x12, "black2_white2")}[GAME]
R = refs(GAME)
TR = {t["rom_id"]: t for t in json.load(open(rf"A:\pkmn_yellow_xp_router\raw_pkmn_data\gen_five\{DATASET}\trainers.json", encoding="utf-8"))["trainers"]}
path = sys.argv[1]
lo = int(sys.argv[2]) if len(sys.argv) > 2 and not sys.argv[2].startswith("--") else 0
hi = int(sys.argv[3]) if len(sys.argv) > 3 and not sys.argv[3].startswith("--") else 10**9
BS = 0x226D670
S = 0x224

events = []
battle = None
prev_bag = None
prev_money = None
prev_levels = None
prev_count = None


def emit(frame, t, kind, **kw):
    e = {"frame": frame, "time": t, "kind": kind, **kw}
    events.append(e)
    print(f"{frame:7d} {t} {kind} " + " ".join(f"{k}={v}" for k, v in kw.items()))


for frame, st, ch in Trace(path):
    if frame < lo:
        # still track state so diffs start from the right place
        pass
    if frame > hi:
        break
    t = fmt_time(st["time"]) if "time" in st else "?"
    flags = st.get("btlflags")
    in_battle = flags is not None and u32(flags, 0x20) == 21828
    party = st.get("party")
    pcount = party[0] if party else 0
    if in_battle and battle is None:
        battle = {"start": frame, "trainer": 0, "trainer2": 0, "alive": {}, "fainted": [], "pcount": pcount}
    if battle is not None and in_battle:
        o = st["opp"]
        if u16(o, OPP_ID):
            battle["trainer"] = battle["trainer"] or u16(o, OPP_ID)
        if u16(o, OPP_ID + 0x50):
            battle["trainer2"] = battle["trainer2"] or u16(o, OPP_ID + 0x50)
        dyn = st["dynparty"]
        ocount = dyn[0x560 * 3 + 0x14 - 4]
        bs = st["bstruct"]
        for s in range(min(ocount, 6)):
            off = S * (battle["pcount"] + s)
            if off + S > len(bs):
                break
            hp = u16(bs, off + 68)
            sp = u16(bs, off + 64)
            lv = bs[off + 76]
            if hp > 0:
                battle["alive"][s] = True
            elif battle["alive"].get(s) and s not in [f[0] for f in battle["fainted"]]:
                battle["fainted"].append((s, R["species"].get(sp, sp), lv))
    if battle is not None and not in_battle:
        # outcome from the party right after
        mons = [pkm_info(bytes(party[4 + 220 * i: 4 + 220 * (i + 1)]), GAME) for i in range(min(pcount, 6))] if party else []
        lost = bool(mons) and all(m and m["hp"] == 0 for m in mons)
        if battle["start"] >= lo:
            tr = battle["trainer"]
            name = TR.get(tr, {}).get("trainer_name") if tr else None
            name2 = TR.get(battle["trainer2"], {}).get("trainer_name") if battle["trainer2"] else None
            emit(battle["start"], t, "trainer" if tr else "wild", id=tr, name=name, second=name2, fainted=battle["fainted"], lost=lost)
        battle = None
    if party and not in_battle:
        mons = [pkm_info(bytes(party[4 + 220 * i: 4 + 220 * (i + 1)]), GAME) for i in range(min(pcount, 6))]
        if all(mons) and mons:
            levels = tuple((m["species"], m["level"]) for m in mons)
            if prev_levels is not None and levels != prev_levels and frame >= lo:
                emit(frame, t, "party", mons=levels)
            prev_levels = levels
    if "bag" in ch or "money" in ch:
        b = bag_items(st["bag"], game=GAME)
        flat = {}
        for lst in b.values():
            for it, q in lst:
                flat[it] = flat.get(it, 0) + q
        money = u32(st["money"], 0)
        if prev_bag is not None and (flat != prev_bag) and frame >= lo and not in_battle:
            diff = {k: flat.get(k, 0) - prev_bag.get(k, 0) for k in set(flat) | set(prev_bag) if flat.get(k, 0) != prev_bag.get(k, 0)}
            if all(abs(v) < 100 for v in diff.values()):
                emit(frame, t, "bag", diff=diff, money=money - (prev_money or money))
        prev_bag = flat
        prev_money = money

if "--json" in sys.argv:
    json.dump(events, open(sys.argv[sys.argv.index("--json") + 1], "w", encoding="utf-8"), ensure_ascii=False, indent=1)
