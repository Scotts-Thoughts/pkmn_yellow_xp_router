import re, json
rom = {}
for line in open(r"A:\Cygwin\home\scott\pokeyellow\data\moves\moves.asm", encoding="utf-8"):
    m = re.match(r"\s*move\s+(\w+),\s*(\w+),\s*(\d+),\s*(\w+),\s*(\d+),\s*(\d+)", line)
    if m:
        name, eff, pw, ty, acc, pp = m.groups()
        rom[name] = (eff, int(pw), ty, int(acc), int(pp))
def norm(n):
    n = n.upper().replace(" ", "_").replace("-", "_")
    return {"PSYCHIC": "PSYCHIC_M", "SOLAR_BEAM": "SOLARBEAM"}.get(n, n)
tymap = {"PSYCHIC_TYPE": "Psychic"}
d = json.load(open(r"A:\pkmn_yellow_xp_router\raw_pkmn_data\gen_one\moves.json"))
mv = d.get("moves", d.values()) if isinstance(d, dict) else d
seen = set()
for m in mv:
    key = norm(m["name"]); seen.add(key)
    if key not in rom:
        print("NOT IN ROM:", m["name"]); continue
    eff, pw, ty, acc, pp = rom[key]
    rty = tymap.get(ty, ty.capitalize())
    jp = m["base_power"]; ja = m["accuracy"]; jpp = m["pp"]
    issues = []
    if (jp or 0) != pw and not (jp in (None, -1) and pw in (0, 1)) and not (jp == 0 and pw == 0):
        issues.append(f"power json={jp} rom={pw}")
    if m["type"] != rty: issues.append(f"type json={m['type']} rom={rty}")
    if ja is not None and ja != acc: issues.append(f"acc json={ja} rom={acc} (rom byte={acc*255//100}, P={acc*255//100}/256={acc*255//100/256:.4f})")
    if ja is None and pw > 1: issues.append(f"acc json=None rom={acc}")
    if jpp != pp: issues.append(f"pp json={jpp} rom={pp}")
    if issues: print(f"{m['name']:14s} [{eff}]: " + "; ".join(issues))
print("missing from json:", sorted(set(rom) - seen))
print("\n--- accuracy byte table (json acc -> game byte -> real hit chance)")
for a in (100, 95, 90, 85, 80, 75, 70, 65, 55, 30):
    b = a * 255 // 100
    print(f"  {a:3d}% -> byte {b:3d} -> {b}/256 = {b/256*100:.2f}%")
