import json, os, sys
sys.path.insert(0, r"A:\pkmn_yellow_xp_router")
from utils.constants import const

# ROM TypeEffects table order (pokeyellow data/types/type_matchups.asm, identical in pokered)
ROM = """WATER FIRE SE
FIRE GRASS SE
FIRE ICE SE
GRASS WATER SE
ELECTRIC WATER SE
WATER ROCK SE
GROUND FLYING NO
WATER WATER NVE
FIRE FIRE NVE
ELECTRIC ELECTRIC NVE
ICE ICE NVE
GRASS GRASS NVE
PSYCHIC PSYCHIC NVE
FIRE WATER NVE
GRASS FIRE NVE
WATER GRASS NVE
ELECTRIC GRASS NVE
NORMAL ROCK NVE
NORMAL GHOST NO
GHOST GHOST SE
FIRE BUG SE
FIRE ROCK NVE
WATER GROUND SE
ELECTRIC GROUND NO
ELECTRIC FLYING SE
GRASS GROUND SE
GRASS BUG NVE
GRASS POISON NVE
GRASS ROCK SE
GRASS FLYING NVE
ICE WATER NVE
ICE GRASS SE
ICE GROUND SE
ICE FLYING SE
FIGHTING NORMAL SE
FIGHTING POISON NVE
FIGHTING FLYING NVE
FIGHTING PSYCHIC NVE
FIGHTING BUG NVE
FIGHTING ROCK SE
FIGHTING ICE SE
FIGHTING GHOST NO
POISON GRASS SE
POISON POISON NVE
POISON GROUND NVE
POISON BUG SE
POISON ROCK NVE
POISON GHOST NVE
GROUND FIRE SE
GROUND ELECTRIC SE
GROUND GRASS NVE
GROUND BUG NVE
GROUND ROCK SE
GROUND POISON SE
FLYING ELECTRIC NVE
FLYING FIGHTING SE
FLYING BUG SE
FLYING GRASS SE
FLYING ROCK NVE
PSYCHIC FIGHTING SE
PSYCHIC POISON SE
BUG FIRE NVE
BUG GRASS SE
BUG FIGHTING NVE
BUG FLYING NVE
BUG PSYCHIC SE
BUG GHOST NVE
BUG POISON SE
ROCK FIRE SE
ROCK FIGHTING NVE
ROCK GROUND NVE
ROCK FLYING SE
ROCK BUG SE
ROCK ICE SE
GHOST NORMAL NO
GHOST PSYCHIC NO
FIRE DRAGON NVE
WATER DRAGON NVE
ELECTRIC DRAGON NVE
GRASS DRAGON NVE
ICE DRAGON SE
DRAGON DRAGON SE"""
rows = [l.split() for l in ROM.strip().splitlines()]
def cap(t): return t.capitalize()
rom_rows = [(cap(a), cap(d), m) for a, d, m in rows]

ti = json.load(open(r"A:\pkmn_yellow_xp_router\raw_pkmn_data\gen_one\type_info.json"))
chart = ti["type_chart"]
m2 = {"Super Effective": "SE", "Not Very Effective": "NVE", "Immune": "NO"}
# 1. verify app chart == ROM table (as a set)
rom_set = {(a, d): m for a, d, m in rom_rows}
app_set = {(a, d): m2[v] for a, row in chart.items() for d, v in row.items()}
print("chart entries ROM:", len(rom_set), "app:", len(app_set))
print("in ROM not app:", {k: v for k, v in rom_set.items() if app_set.get(k) != v})
print("in app not ROM:", {k: v for k, v in app_set.items() if rom_set.get(k) != v})

def game_seq(mt, t1, t2):
    # walk table in ROM order; each matching row (defender type1 or type2) applies once
    seq = []
    for a, d, m in rom_rows:
        if a == mt and (d == t1 or d == t2):
            seq.append((d, m))
    return seq
def app_seq(mt, t1, t2):
    seq = []
    r = chart.get(mt, {})
    if t1 in r: seq.append((t1, m2[r[t1]]))
    if t2 != t1 and t2 in r: seq.append((t2, m2[r[t2]]))
    return seq

for ver in ["yellow", "red_blue"]:
    p = os.path.join(r"A:\pkmn_yellow_xp_router\raw_pkmn_data\gen_one", ver, const.POKEMON_DB_FILE_NAME)
    db = json.load(open(p))
    mons = db.get("pokemon", db.values()) if isinstance(db, dict) else db
    diffs = []
    for mon in mons:
        t1, t2 = mon[const.FIRST_TYPE_KEY], mon[const.SECOND_TYPE_KEY]
        if t1 == t2: continue
        for mt in chart.keys():
            g = game_seq(mt, t1, t2); a = app_seq(mt, t1, t2)
            if [m for _, m in g] != [m for _, m in a]:
                diffs.append((mon[const.NAME_KEY], t1, t2, mt, g, a))
    print("=====", ver, "species with order-dependent mismatch:", len(diffs))
    for d in diffs: print("  ", d)
