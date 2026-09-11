"""Run the app's gen-2 calculator against the asm-derived reference on concrete inputs."""
import os, sys
APP = r"A:\pkmn_yellow_xp_router"
SCR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, APP)
sys.path.insert(0, SCR)
os.chdir(APP)

import controllers.main_controller  # noqa (import chain)
from utils.constants import const
from pkmn import gen_factory, universal_data_objects as udo
from pkmn.gen_2 import gen_two_object
from pkmn.gen_2.data_objects import GenTwoBadgeList, GenTwoStatBlock, calc_stat
from pkmn.gen_2 import pkmn_damage_calc as g2calc
import gen2_ref as ref

for g, v in [(gen_two_object.gen_two_gold, const.GOLD_VERSION), (gen_two_object.gen_two_crystal, const.CRYSTAL_VERSION)]:
    try:
        gen_factory._gen_factory.register_gen(g, v)
    except ValueError:
        pass
gen_factory.change_version(const.CRYSTAL_VERSION)
gen = gen_factory.current_gen_info()
mdb = gen.move_db()
pdb = gen.pkmn_db()

def mk(name, level, dvs=(8, 9, 8, 8, 8, 8), badges=None, item=None, statexp=0):
    sp = pdb.get_pkmn(name)
    dv = GenTwoStatBlock(*dvs)
    sx = GenTwoStatBlock(statexp, statexp, statexp, statexp, statexp, statexp, is_stat_xp=True)
    cur = GenTwoStatBlock(
        calc_stat(sp.stats.hp, level, dv.hp, sx.hp, is_hp=True),
        calc_stat(sp.stats.attack, level, dv.attack, sx.attack),
        calc_stat(sp.stats.defense, level, dv.defense, sx.defense),
        calc_stat(sp.stats.special_attack, level, dv.special_attack, sx.special_attack),
        calc_stat(sp.stats.special_defense, level, dv.special_attack, sx.special_attack),
        calc_stat(sp.stats.speed, level, dv.speed, sx.speed),
    )
    return udo.EnemyPkmn(name, level, 0, [], cur, sp.stats, dv, sx, badges, held_item=item)

def app(att, move, dfn, **kw):
    r = gen.calculate_damage(att, mdb.get_move(move), dfn, **kw)
    if r is None:
        return "None"
    return f"{r.min_damage}..{r.max_damage} ({len(r.damage_vals)} distinct, {len(r)} rolls)"

def stats(mon):
    s = mon.cur_stats
    return f"HP{s.hp} A{s.attack} D{s.defense} SA{s.special_attack} SD{s.special_defense} S{s.speed}"

print("=" * 78)
print("CASE 1: TruncateHL_BC (/4 when a stat > 255). L100 Machamp Cross Chop vs L100 Snorlax")
a = mk("Machamp", 100); d = mk("Snorlax", 100)
print("  attacker", stats(a), "| defender", stats(d))
a8, d8 = ref.damage_stats(a.cur_stats.attack, d.cur_stats.defense, a.cur_stats.attack, d.cur_stats.defense, False, 0, 0, False)
x = ref.damage_calc(100, 100, a8, d8)
dm, _ = ref.stab_command(x, "Fighting", ("Fighting", "Fighting"), ("Normal", "Normal"))
print(f"  GAME: atk8={a8} def8={d8} -> calc={x} -> after STAB/type={dm} -> rolls {ref.summarize(ref.variation(dm))}")
print("  APP :", app(a, "Cross Chop", d))

print("=" * 78)
print("CASE 1b: same but Reflect up, def=151 -> 302 triggers truncation of BOTH stats. L50 Machop Mega Punch vs L50 Snorlax+Reflect")
a = mk("Machop", 50); d = mk("Snorlax", 50)
print("  attacker", stats(a), "| defender", stats(d))
a8, d8 = ref.damage_stats(a.cur_stats.attack, d.cur_stats.defense, a.cur_stats.attack, d.cur_stats.defense, False, 0, 0, True)
x = ref.damage_calc(50, 80, a8, d8)
dm, _ = ref.stab_command(x, "Normal", ("Fighting", "Fighting"), ("Normal", "Normal"))
print(f"  GAME: atk8={a8} def8={d8} -> calc={x} -> {dm} -> rolls {ref.summarize(ref.variation(dm))}")
print("  APP :", app(a, "Mega Punch", d, defending_field=udo.FieldStatus(reflect=True)))

print("=" * 78)
print("CASE 2: crit + attacker stage > defender stage keeps Reflect. L30 Machop(+2 Atk) Mega Punch crit vs L30 Rattata+Reflect")
a = mk("Machop", 30); d = mk("Rattata", 30)
print("  attacker", stats(a), "| defender", stats(d))
atk_b = ref.stage_stat(a.cur_stats.attack, 2)
a8, d8 = ref.damage_stats(atk_b, d.cur_stats.defense, a.cur_stats.attack, d.cur_stats.defense, True, 2, 0, True)
x = ref.damage_calc(30, 80, a8, d8, crit=True)
dm, _ = ref.stab_command(x, "Normal", ("Fighting", "Fighting"), ("Normal", "Normal"))
print(f"  GAME: boosted atk={atk_b} atk8={a8} def8={d8} -> calc={x} -> {dm} -> rolls {ref.summarize(ref.variation(dm))}")
print("  APP :", app(a, "Mega Punch", d, attacking_stage_modifiers=udo.StageModifiers(attack=2), defending_field=udo.FieldStatus(reflect=True), is_crit=True))
print("  (control: same crit with stages equal -> both ignore Reflect and stages)")
a8, d8 = ref.damage_stats(a.cur_stats.attack, d.cur_stats.defense, a.cur_stats.attack, d.cur_stats.defense, True, 0, 0, True)
x = ref.damage_calc(30, 80, a8, d8, crit=True)
dm, _ = ref.stab_command(x, "Normal", ("Fighting", "Fighting"), ("Normal", "Normal"))
print(f"  GAME: {ref.summarize(ref.variation(dm))}   APP: {app(a, 'Mega Punch', d, defending_field=udo.FieldStatus(reflect=True), is_crit=True)}")

print("=" * 78)
print("CASE 3: Fury Cutter dropdown '6' (game caps at 16x). L30 Scyther Fury Cutter vs L30 Rattata")
a = mk("Scyther", 30); d = mk("Rattata", 30)
a8, d8 = ref.damage_stats(a.cur_stats.attack, d.cur_stats.defense, a.cur_stats.attack, d.cur_stats.defense, False, 0, 0, False)
x = ref.damage_calc(30, 10, a8, d8)
dm, _ = ref.stab_command(x, "Bug", ("Bug", "Flying"), ("Normal", "Normal"))
print(f"  GAME: base after STAB={dm}; x16 = {dm*16} -> rolls {ref.summarize(ref.variation(dm*16))}")
print("  APP '5':", app(a, "Fury Cutter", d, custom_move_data="5"), " APP '6':", app(a, "Fury Cutter", d, custom_move_data="6"))

print("=" * 78)
print("CASE 4: badge type boost adds max(dmg/8,1). L5 Pidgey (Zephyr) Gust vs L5 Rattata")
bl = GenTwoBadgeList({}, zephyr=True)
a = mk("Pidgey", 5, badges=bl); d = mk("Rattata", 5)
print("  attacker", stats(a), "| defender", stats(d))
atk_b = ref.badge_boost(a.cur_stats.attack)
a8, d8 = ref.damage_stats(atk_b, d.cur_stats.defense, a.cur_stats.attack, d.cur_stats.defense, False, 0, 0, False)
x = ref.damage_calc(5, 40, a8, d8)
dm, _ = ref.stab_command(x, "Flying", ("Normal", "Flying"), ("Normal", "Normal"), badge_boost_type=True)
print(f"  GAME: atk(badge)={atk_b} calc={x} -> after badge type boost + STAB = {dm} -> rolls {ref.summarize(ref.variation(dm))}")
print("  APP :", app(a, "Gust", d, custom_move_data="No Bonus"))

print("=" * 78)
print("CASE 5: Hidden Power with DVs 8/8/8/8 (top bit set => power 68, app treats 8 as low)")
from pkmn.gen_2.data_objects import get_hidden_power_type, get_hidden_power_base_power
dv = GenTwoStatBlock(8, 8, 8, 8, 8, 8)
print("  GAME:", ref.hidden_power(8, 8, 8, 8), " APP:", get_hidden_power_type(dv), get_hidden_power_base_power(dv))
dv = GenTwoStatBlock(8, 15, 15, 15, 15, 15)
print("  DVs 15/15/15/15 GAME:", ref.hidden_power(15, 15, 15, 15), " APP:", get_hidden_power_type(dv), get_hidden_power_base_power(dv))
dv = GenTwoStatBlock(8, 9, 8, 9, 9, 8)
print("  DVs atk9/def8/spd8/spc9 GAME:", ref.hidden_power(9, 8, 8, 9), " APP:", get_hidden_power_type(dv), get_hidden_power_base_power(dv))

print("=" * 78)
print("CASE 6: Future Sight ignores type effectiveness (no stab command). L40 Espeon vs L40 Umbreon / Alakazam / Machoke")
a = mk("Espeon", 40)
for dn in ["Umbreon", "Alakazam", "Machoke"]:
    d = mk(dn, 40)
    a8, d8 = ref.damage_stats(a.cur_stats.special_attack, d.cur_stats.special_defense, a.cur_stats.special_attack, d.cur_stats.special_defense, False, 0, 0, False)
    x = ref.damage_calc(40, 80, a8, d8)
    print(f"  vs {dn}: GAME {ref.summarize(ref.variation(x))}   APP {app(a, 'Future Sight', d)}")

print("=" * 78)
print("CASE 7: Triple Kick multiplier is applied BEFORE stab. L30 Hitmontop Triple Kick '3' vs L30 Rattata")
a = mk("Hitmontop", 30); d = mk("Rattata", 30)
a8, d8 = ref.damage_stats(a.cur_stats.attack, d.cur_stats.defense, a.cur_stats.attack, d.cur_stats.defense, False, 0, 0, False)
x = ref.damage_calc(30, 10, a8, d8)
for k in (1, 2, 3):
    dm, _ = ref.stab_command(x * k, "Fighting", ("Fighting", "Fighting"), ("Normal", "Normal"))
    print(f"  kick {k}: GAME calc={x} x{k}={x*k} -> STAB+SE={dm} -> {ref.summarize(ref.variation(dm))}   APP '{k}': {app(a, 'Triple Kick', d, custom_move_data=str(k))}")

print("=" * 78)
print("CASE 8: Gust/Pursuit/EQ/Stomp/Twister/Magnitude doubling happens AFTER the random roll. L20 Pidgeotto Gust (Fly Bonus) vs L20 Rattata")
a = mk("Pidgeotto", 20); d = mk("Rattata", 20)
a8, d8 = ref.damage_stats(a.cur_stats.attack, d.cur_stats.defense, a.cur_stats.attack, d.cur_stats.defense, False, 0, 0, False)
x = ref.damage_calc(20, 40, a8, d8)
dm, _ = ref.stab_command(x, "Flying", ("Normal", "Flying"), ("Normal", "Normal"))
print(f"  GAME: {dm} -> roll then x2: {ref.summarize(ref.double_after(ref.variation(dm)))}")
print("  APP :", app(a, "Gust", d, custom_move_data="Fly Bonus"))

print("=" * 78)
print("CASE 9: Thick Club works for Cubone too. L30 Cubone+Thick Club Bone Club vs L30 Rattata")
a = mk("Cubone", 30, item="Thick Club"); d = mk("Rattata", 30)
a8, d8 = ref.damage_stats(a.cur_stats.attack, d.cur_stats.defense, a.cur_stats.attack, d.cur_stats.defense, False, 0, 0, False, species_item_x2=True)
x = ref.damage_calc(30, 65, a8, d8)
dm, _ = ref.stab_command(x, "Ground", ("Ground", "Ground"), ("Normal", "Normal"))
print(f"  GAME: atk8={a8} def8={d8} -> {ref.summarize(ref.variation(dm))}")
print("  APP :", app(a, "Bone Club", d))

print("=" * 78)
print("CASE 10: Metal Powder also boosts Ditto's SpDef. L30 Quilava Ember vs L30 Ditto+Metal Powder")
a = mk("Quilava", 30); d = mk("Ditto", 30, item="Metal Powder")
a8, d8 = ref.damage_stats(a.cur_stats.special_attack, d.cur_stats.special_defense, a.cur_stats.special_attack, d.cur_stats.special_defense, False, 0, 0, False, metal_powder=True)
x = ref.damage_calc(30, 40, a8, d8)
dm, _ = ref.stab_command(x, "Fire", ("Fire", "Fire"), ("Normal", "Normal"))
print(f"  GAME: atk8={a8} def8(after MP)={d8} -> {ref.summarize(ref.variation(dm))}")
print("  APP :", app(a, "Ember", d))

print("=" * 78)
print("CASE 11: moves with base_power -1 fall through to the formula and report 1 damage")
a = mk("Raticate", 30); d = mk("Rattata", 30)
for mv in ["Super Fang", "Present", "Frustration", "Horn Drill", "Fissure", "Mirror Coat", "Counter", "Guillotine", "Beat Up", "Return", "Bide"]:
    print(f"  {mv:12}:", app(mk("Machamp", 30), mv, d, custom_move_data="102" if mv == "Return" else ""))

print("=" * 78)
print("CASE 12: Thunder accuracy in sun (game 128/256 = 50%)")
print("  APP rain:", gen.get_move_accuracy(a, mdb.get_move("Thunder"), "", d, const.WEATHER_RAIN), " sun:", gen.get_move_accuracy(a, mdb.get_move("Thunder"), "", d, const.WEATHER_SUN))

print("=" * 78)
print("CASE 13: Aeroblast crit rate (game +2 stage = 1/4)")
print("  APP :", gen.get_crit_rate(a, mdb.get_move("Aeroblast"), ""), " Slash:", gen.get_crit_rate(a, mdb.get_move("Slash"), ""))

print("=" * 78)
print("CASE 14: 997 cap. L50 Golem Explosion crit vs L30 Rattata")
a = mk("Golem", 50); d = mk("Rattata", 30)
a8, d8 = ref.damage_stats(a.cur_stats.attack, d.cur_stats.defense, a.cur_stats.attack, d.cur_stats.defense, True, 0, 0, False)
x = ref.damage_calc(50, 250, a8, d8, crit=True, selfdestruct=True)
dm, _ = ref.stab_command(x, "Normal", ("Rock", "Ground"), ("Normal", "Normal"))
print(f"  GAME: atk8={a8} def8={d8} calc={x} -> {ref.summarize(ref.variation(dm))}")
print("  APP :", app(a, "Explosion", d, is_crit=True))

print("=" * 78)
print("CASE 15: Struggle + Pink Bow (item boost applies by move type NORMAL in DamageCalc)")
a = mk("Raticate", 30, item="Pink Bow"); d = mk("Rattata", 30)
a8, d8 = ref.damage_stats(a.cur_stats.attack, d.cur_stats.defense, a.cur_stats.attack, d.cur_stats.defense, False, 0, 0, False)
x = ref.damage_calc(30, 50, a8, d8, item_boost=True)
print(f"  GAME: calc={x} (no STAB/type) -> {ref.summarize(ref.variation(x))}")
print("  APP :", app(a, "Struggle", d))

print("=" * 78)
print("CASE 16: Flail single value / Psywave / SonicBoom / Seismic Toss sanity")
a = mk("Raticate", 30); d = mk("Rattata", 30)
print("  Flail 4-0%:", app(a, "Flail", d, custom_move_data="4-0 % HP"), " crit:", app(a, "Flail", d, custom_move_data="4-0 % HP", is_crit=True))
print("  Psywave L30:", app(mk("Abra", 30), "Psywave", d), " vs Umbreon:", app(mk("Abra", 30), "Psywave", mk("Umbreon", 30)))
print("  SonicBoom vs Gastly:", app(a, "SonicBoom", mk("Gastly", 30)), " Seismic Toss vs Gastly:", app(mk("Machop", 30), "Seismic Toss", mk("Gastly", 30)))

print("=" * 78)
print("CASE 17: Present in Gold/Silver (register clobber). Normal-type user vs pure-Normal target, power 120")
# level = defender type2 id (Normal=0), attack = wTypeMatchup=10, defense = 0 -> 1 (user gets STAB)
for p in (40, 80, 120):
    x = ref.damage_calc(0, p, 10, 1)
    dm, _ = ref.stab_command(x, "Normal", ("Normal", "Normal"), ("Normal", "Normal"))
    print(f"  G/S power {p}: calc={x} -> STAB {dm} -> {ref.summarize(ref.variation(dm))}")
print("  G/S Delibird (Ice/Flying, no STAB, type2=Flying=2) vs Dark-type target (type id 27): power 80")
x = ref.damage_calc(27, 80, 10, 2)
print(f"    calc={x} -> {ref.summarize(ref.variation(x))}")

print("=" * 78)
print("CASE 18: multi-hit crit modelling. L30 Sandslash Fury Swipes '3 Hits' crit")
a = mk("Sandslash", 30); d = mk("Rattata", 30)
print("  APP crit 3 hits:", app(a, "Fury Swipes", d, custom_move_data="3 Hits", is_crit=True), " non-crit:", app(a, "Fury Swipes", d, custom_move_data="3 Hits"))
