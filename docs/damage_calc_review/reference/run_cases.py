import sys, os
sys.path.insert(0, os.getcwd())
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import logging
logging.disable(logging.CRITICAL)
import controllers.main_controller  # noqa
from utils.constants import const
from pkmn import gen_factory
from pkmn.gen_3 import gen_three_object
from pkmn.universal_data_objects import StageModifiers, FieldStatus
from pkmn.gen_3.gen_three_constants import gen_three_const as g3
import ref_gen3 as R

try:
    gen_factory._gen_factory.register_gen(gen_three_object.gen_three_emerald, const.EMERALD_VERSION)
except ValueError:
    pass
gen_factory.change_version(const.EMERALD_VERSION)
gen = gen_factory.current_gen_info()
mv = lambda n: gen.move_db().get_move(n)

def mon(name, lvl, **kw):
    m = gen.create_trainer_pkmn(name, lvl)
    for k, v in kw.items():
        setattr(m, k, v)
    return m

def show(label, dr):
    if dr is None:
        print(f"  APP {label}: None")
        return
    print(f"  APP {label}: min={dr.min_damage} max={dr.max_damage} n={dr.size} vals={dict(sorted(dr.damage_vals.items()))}")

def refshow(label, d):
    print(f"  REF {label}: min={min(d)} max={max(d)} vals={dict(sorted(d.items()))}")

a = mon("Machop", 50); d = mon("Machop", 50)
A = a.cur_stats; D = d.cur_stats
print("Machop L50 trainer stats (hp,atk,def,spa,spd,spe):", A.hp, A.attack, A.defense, A.special_attack, A.special_defense, A.speed)
sp = gen.pkmn_db().get_pkmn("Machop"); print("types", sp.first_type, sp.second_type)

print("\n[1] Karate Chop (vanilla, STAB)")
show("", gen.calculate_damage(a, mv("Karate Chop"), d))
refshow("", R.rolls(R.full(R.base_damage(50, 50, 'Fighting', A.attack, D.defense, A.special_attack, D.special_defense), stab=True)))
show("crit", gen.calculate_damage(a, mv("Karate Chop"), d, is_crit=True))
refshow("crit", R.rolls(R.full(R.base_damage(50, 50, 'Fighting', A.attack, D.defense, A.special_attack, D.special_defense, crit=True), crit=True, stab=True)))

print("\n[2] Rollout turns 1..5, 5+DC (Rock vs Fighting: NVE)")
for cmd in ["1", "2", "3", "4", "5", "5 + DefenseCurl"]:
    show(cmd, gen.calculate_damage(a, mv("Rollout"), d, custom_move_data=cmd))
for n in range(1, 6):
    bp = 30 * 2 ** (n - 1)
    refshow(f"turn {n} (bp {bp})", R.rolls(R.full(R.base_damage(50, bp, 'Rock', A.attack, D.defense, A.special_attack, D.special_defense), effs=(5,))))
refshow("turn 5 + DC (bp 960)", R.rolls(R.full(R.base_damage(50, 960, 'Rock', A.attack, D.defense, A.special_attack, D.special_defense), effs=(5,))))
refshow("turn 1 + DC (bp 60)", R.rolls(R.full(R.base_damage(50, 60, 'Rock', A.attack, D.defense, A.special_attack, D.special_defense), effs=(5,))))

print("\n[3] Fury Cutter 1..6 (Bug vs Fighting NVE)")
for cmd in ["1", "2", "3", "4", "5", "6"]:
    show(cmd, gen.calculate_damage(a, mv("Fury Cutter"), d, custom_move_data=cmd))
for n in range(1, 7):
    bp = 10 * 2 ** (min(n, 5) - 1)
    refshow(f"use {n} (bp {bp})", R.rolls(R.full(R.base_damage(50, bp, 'Bug', A.attack, D.defense, A.special_attack, D.special_defense), effs=(5,))))

print("\n[4] Triple Kick '3' (STAB)")
show("3", gen.calculate_damage(a, mv("Triple Kick"), d, custom_move_data="3"))
r1 = R.rolls(R.full(R.base_damage(50, 10, 'Fighting', A.attack, D.defense, A.special_attack, D.special_defense), stab=True))
r2 = R.rolls(R.full(R.base_damage(50, 20, 'Fighting', A.attack, D.defense, A.special_attack, D.special_defense), stab=True))
r3 = R.rolls(R.full(R.base_damage(50, 30, 'Fighting', A.attack, D.defense, A.special_attack, D.special_defense), stab=True))
print(f"  REF hit1 {R.rng(r1)} hit2 {R.rng(r2)} hit3 {R.rng(r3)} -> total range {min(r1)+min(r2)+min(r3)}..{max(r1)+max(r2)+max(r3)}")

print("\n[5] Weather Ball in rain (Water, neutral vs Fighting), Castform L50")
c = mon("Castform", 50); C = c.cur_stats
print("  castform spa", C.special_attack)
show("rain", gen.calculate_damage(c, mv("Weather Ball"), d, weather=const.WEATHER_RAIN))
refshow("rain", R.rolls(R.full(R.base_damage(50, 50, 'Water', C.attack, D.defense, C.special_attack, D.special_defense, weather='rain'), dmg_multiplier=2, stab=True)))
show("sand", gen.calculate_damage(c, mv("Weather Ball"), d, weather=const.WEATHER_SANDSTORM))
refshow("sand", R.rolls(R.full(R.base_damage(50, 50, 'Rock', C.attack, D.defense, C.special_attack, D.special_defense), dmg_multiplier=2, effs=(5,))))

print("\n[6] Explosion with defender Def -1")
print("  defender def:", D.defense)
show("def-1", gen.calculate_damage(a, mv("Explosion"), d, defending_stage_modifiers=StageModifiers(defense=-1)))
refshow("def-1", R.rolls(R.full(R.base_damage(50, 250, 'Normal', A.attack, D.defense, A.special_attack, D.special_defense, def_stage=-1, explosion=True))))
show("def 0", gen.calculate_damage(a, mv("Explosion"), d))
refshow("def 0", R.rolls(R.full(R.base_damage(50, 250, 'Normal', A.attack, D.defense, A.special_attack, D.special_defense, explosion=True))))

print("\n[7] Thick Fat: Ember from Machop vs Thick Fat defender, attacker SpA -1 / +1")
d2 = mon("Machop", 50, ability="Thick Fat")
for stg in (-1, 0, 1):
    show(f"spa{stg:+d}", gen.calculate_damage(a, mv("Ember"), d2, attacking_stage_modifiers=StageModifiers(special_attack=stg)))
    refshow(f"spa{stg:+d}", R.rolls(R.full(R.base_damage(50, 40, 'Fire', A.attack, D.defense, A.special_attack, D.special_defense, spa_stage=stg, thick_fat=True))))

print("\n[8] Choice Band + Atk +1 (Karate Chop)")
a2 = mon("Machop", 50, held_item="Choice Band")
show("cb +1", gen.calculate_damage(a2, mv("Karate Chop"), d, attacking_stage_modifiers=StageModifiers(attack=1)))
refshow("cb +1", R.rolls(R.full(R.base_damage(50, 50, 'Fighting', A.attack, D.defense, A.special_attack, D.special_defense, atk_stage=1, choice_band=True), stab=True)))
show("cb -1", gen.calculate_damage(a2, mv("Karate Chop"), d, attacking_stage_modifiers=StageModifiers(attack=-1)))
refshow("cb -1", R.rolls(R.full(R.base_damage(50, 50, 'Fighting', A.attack, D.defense, A.special_attack, D.special_defense, atk_stage=-1, choice_band=True), stab=True)))

print("\n[9] Stone badge + crit (Karate Chop)")
a3 = mon("Machop", 50); bl = gen.make_badge_list(); bl.stone = True; a3.badges = bl
show("badge", gen.calculate_damage(a3, mv("Karate Chop"), d))
show("badge crit", gen.calculate_damage(a3, mv("Karate Chop"), d, is_crit=True))
refshow("badge", R.rolls(R.full(R.base_damage(50, 50, 'Fighting', A.attack, D.defense, A.special_attack, D.special_defense, badge_atk=True), stab=True)))
refshow("badge crit", R.rolls(R.full(R.base_damage(50, 50, 'Fighting', A.attack, D.defense, A.special_attack, D.special_defense, badge_atk=True, crit=True), crit=True, stab=True)))
show("badge atk+1", gen.calculate_damage(a3, mv("Karate Chop"), d, attacking_stage_modifiers=StageModifiers(attack=1)))
refshow("badge atk+1", R.rolls(R.full(R.base_damage(50, 50, 'Fighting', A.attack, D.defense, A.special_attack, D.special_defense, badge_atk=True, atk_stage=1), stab=True)))

print("\n[10] placeholder-power moves (app output)")
for n in ["Frustration", "Present", "Low Kick", "Counter", "Mirror Coat", "Bide", "Super Fang", "Guillotine", "Horn Drill", "Fissure", "Sheer Cold", "Endeavor", "Beat Up", "Return", "Flail", "Reversal", "Magnitude", "Hidden Power", "Psywave", "Nature Power", "Rage", "Spit Up", "Struggle", "Future Sight", "Doom Desire", "SonicBoom", "Dragon Rage", "Seismic Toss", "Night Shade"]:
    m = mv(n)
    if m is None:
        print("  ", n, "-> move not found"); continue
    cmds = g3.CUSTOM_MOVE_DATA.get(n)
    cmd = cmds[0] if cmds else ""
    show(f"{n} [{cmd}]", gen.calculate_damage(a, m, d, custom_move_data=cmd))
print("\n[10b] Nature Power each terrain")
for t in g3.CUSTOM_MOVE_DATA[g3.NATURE_POWER_MOVE_NAME]:
    show(f"NP {t}", gen.calculate_damage(a, mv("Nature Power"), d, custom_move_data=t))
print("\n[10c] Psywave ref (L50): ", sorted(set(50 * (10 * k + 50) // 100 for k in range(11))))
print("\n[10d] Rage 1..6")
for cmd in ["1", "3", "6"]:
    show(f"Rage {cmd}", gen.calculate_damage(a, mv("Rage"), d, custom_move_data=cmd))
refshow("Rage (20 BP)", R.rolls(R.full(R.base_damage(50, 20, 'Normal', A.attack, D.defense, A.special_attack, D.special_defense))))

print("\n[11] Abilities: Lightning Rod / Water Absorb / Flash Fire defenders")
for ab, m in [("Lightning Rod", "Thunderbolt"), ("Water Absorb", "Ember"), ("Water Absorb", "Water Gun"), ("Flash Fire", "Ember"), ("Volt Absorb", "Thunderbolt"), ("Soundproof", "Hyper Voice"), ("Levitate", "Earthquake")]:
    dd = mon("Machop", 50, ability=ab); show(f"{ab} vs {m}", gen.calculate_damage(a, mv(m), dd))

print("\n[12] Doubles Reflect (Karate Chop, both alive)")
show("reflect doubles", gen.calculate_damage(a, mv("Karate Chop"), d, defending_field=FieldStatus(reflect=True), is_double_battle=True))
refshow("reflect doubles", R.rolls(R.full(R.base_damage(50, 50, 'Fighting', A.attack, D.defense, A.special_attack, D.special_defense, reflect=True, doubles=True, two_alive=True), stab=True)))
show("reflect singles", gen.calculate_damage(a, mv("Karate Chop"), d, defending_field=FieldStatus(reflect=True)))
refshow("reflect singles", R.rolls(R.full(R.base_damage(50, 50, 'Fighting', A.attack, D.defense, A.special_attack, D.special_defense, reflect=True), stab=True)))

print("\n[13] Spit Up 3 stockpiles")
show("spit 3", gen.calculate_damage(a, mv("Spit Up"), d, custom_move_data="3"))
print("  REF:", R.full(R.base_damage(50, 100, 'Normal', A.attack, D.defense, A.special_attack, D.special_defense), dmg_multiplier=3))
show("spit 3 crit flag", gen.calculate_damage(a, mv("Spit Up"), d, custom_move_data="3", is_crit=True, defending_field=FieldStatus(reflect=True)))
print("  REF (no crit exists; reflect applies):", R.full(R.base_damage(50, 100, 'Normal', A.attack, D.defense, A.special_attack, D.special_defense, reflect=True), dmg_multiplier=3))

print("\n[14] physical min-1 edge: Rattata L2 Tackle (35) vs Onix L30")
r = mon("Rattata", 2); o = mon("Onix", 30); RR = r.cur_stats; OO = o.cur_stats
print("  stats", RR.attack, OO.defense)
show("", gen.calculate_damage(r, mv("Tackle"), o))
refshow("", R.rolls(R.full(R.base_damage(2, 35, 'Normal', RR.attack, OO.defense, RR.special_attack, OO.special_defense), stab=True, effs=(5, 5))))

print("\n[16] Accuracy")
z = mon("Zapdos", 50)
print("  Thunder sun:", gen.get_move_accuracy(z, mv("Thunder"), "", d, const.WEATHER_SUN), " (game: 50)")
print("  Thunder rain:", gen.get_move_accuracy(z, mv("Thunder"), "", d, const.WEATHER_RAIN), " (game: always hit)")
print("  Blizzard hail:", gen.get_move_accuracy(z, mv("Blizzard"), "", d, const.WEATHER_HAIL), " (game: 70)")
for t in ["Plain", "Rock", "Long Grass", "Underwater", "Sand"]:
    print(f"  NaturePower {t}:", gen.get_move_accuracy(z, mv("Nature Power"), t, d, const.WEATHER_NONE))
ce = mon("Butterfree", 50, ability="Compound Eyes"); sv = mon("Sandshrew", 50, ability="Sand Veil")
print("  CompoundEyes Sleep Powder(75) vs SandVeil in sand:", gen.get_move_accuracy(ce, mv("Sleep Powder"), "", sv, const.WEATHER_SANDSTORM), " game:", (75 * 130 // 100) * 80 // 100)
print("  CompoundEyes Gust(100) vs SandVeil in sand:", gen.get_move_accuracy(ce, mv("Gust"), "", sv, const.WEATHER_SANDSTORM), " game:", (100 * 130 // 100) * 80 // 100, "(>100 => always)")
hu = mon("Machop", 50, ability="Hustle")
print("  Hustle Cross Chop(80):", gen.get_move_accuracy(hu, mv("Cross Chop"), "", d, const.WEATHER_NONE), " game:", 80 * 80 // 100)
print("  Hustle Ember(100):", gen.get_move_accuracy(hu, mv("Ember"), "", d, const.WEATHER_NONE), " game: 100")
cn = mon("Machop", 50, ability="Cloud Nine")
print("  Sand Veil target, attacker Cloud Nine, in sand:", gen.get_move_accuracy(cn, mv("Gust"), "", sv, const.WEATHER_SANDSTORM), " game: 100 (weather negated)")
print("\n[17] crit rate")
for n, item in [("Karate Chop", None), ("Tackle", None), ("Tackle", "Scope Lens"), ("Slash", "Scope Lens")]:
    a4 = mon("Machop", 50, held_item=item); print(f"  {n} {item}: {gen.get_crit_rate(a4, mv(n), '')}")
ch = mon("Chansey", 50, held_item="Lucky Punch"); print("  Chansey Lucky Punch Pound:", gen.get_crit_rate(ch, mv("Pound"), ''), " game: stage 2 -> 1/4")
ba = mon("Machop", 50, ability="Battle Armor"); print("  crit rate vs Battle Armor (app has no defender arg):", gen.get_crit_rate(a, mv("Slash"), ''))
print("\n[18] Hidden Power check")
from pkmn.gen_3.data_objects import get_hidden_power_type, get_hidden_power_base_power, GenThreeStatBlock
types = ['Fighting', 'Flying', 'Poison', 'Ground', 'Rock', 'Bug', 'Ghost', 'Steel', 'Fire', 'Water', 'Grass', 'Electric', 'Psychic', 'Ice', 'Dragon', 'Dark']
for dv in [(8, 9, 8, 8, 8, 8), (31, 31, 31, 31, 31, 31), (30, 31, 31, 31, 31, 31), (15, 15, 15, 15, 15, 15), (0, 0, 0, 0, 0, 0), (31, 30, 30, 31, 31, 31), (2, 3, 2, 0, 1, 3)]:
    sb = GenThreeStatBlock(*dv)  # (hp, atk, def, spa, spd, spe)
    hp, at, df, sa, sd, spe = dv
    tb = (hp & 1) | ((at & 1) << 1) | ((df & 1) << 2) | ((spe & 1) << 3) | ((sa & 1) << 4) | ((sd & 1) << 5)
    pb = ((hp & 2) >> 1) | ((at & 2)) | ((df & 2) << 1) | ((spe & 2) << 2) | ((sa & 2) << 3) | ((sd & 2) << 4)
    print(f"  dvs(hp,atk,def,spa,spd,spe)={dv}: app {get_hidden_power_type(sb)} {get_hidden_power_base_power(sb)} | game {types[15*tb//63]} {40*pb//63+30}")
print("\n[19] Beat Up app vs game (Machop party of 1: base atk 80 vs base def 50)")
show("beat up", gen.calculate_damage(a, mv("Beat Up"), d))
bu = 80 * 10 * (50 * 2 // 5 + 2) // 50 // 50 + 2; print("  REF one hit:", bu, "rolls", R.rolls(bu))
print("\n[20] Dragon Rage vs Shedinja (Wonder Guard)")
sh = mon("Shedinja", 30, ability="Wonder Guard"); show("dragon rage vs shedinja", gen.calculate_damage(a, mv("Dragon Rage"), sh))
show("night shade vs shedinja", gen.calculate_damage(a, mv("Night Shade"), sh))
print("\n[21] Cubone Thick Club")
cb = mon("Cubone", 30, held_item="Thick Club"); mw = mon("Marowak", 30, held_item="Thick Club")
show("cubone bone club", gen.calculate_damage(cb, mv("Bone Club"), d)); show("marowak bone club", gen.calculate_damage(mw, mv("Bone Club"), d))
CB = cb.cur_stats; refshow("cubone", R.rolls(R.full(R.base_damage(30, 65, 'Ground', CB.attack, D.defense, CB.special_attack, D.special_defense, thick_club=True), stab=True)))
MW = mw.cur_stats; refshow("marowak", R.rolls(R.full(R.base_damage(30, 65, 'Ground', MW.attack, D.defense, MW.special_attack, D.special_defense, thick_club=True), stab=True)))
print("\n[22] Soul Dew / DeepSeaScale on Latios")
lt = mon("Latios", 50, held_item="Soul Dew"); show("latios soul dew psychic", gen.calculate_damage(lt, mv("Psychic"), d))
LT = lt.cur_stats; refshow("", R.rolls(R.full(R.base_damage(50, 90, 'Psychic', LT.attack, D.defense, LT.special_attack, D.special_defense, soul_dew_atk=True), stab=True)))
lt2 = mon("Latios", 50, held_item="DeepSeaScale"); show("latios deepseascale psychic (app wrongly boosts)", gen.calculate_damage(lt2, mv("Psychic"), d))
print("\n[23] Clamperl DeepSeaScale defending, DeepSeaTooth attacking")
cl = mon("Clamperl", 30, held_item="DeepSeaScale"); show("confusion vs clamperl+DSS", gen.calculate_damage(a, mv("Confusion"), cl))
CL = cl.cur_stats; refshow("", R.rolls(R.full(R.base_damage(50, 50, 'Psychic', A.attack, CL.defense, A.special_attack, CL.special_defense, deep_sea_scale=True))))
cl2 = mon("Clamperl", 30, held_item="DeepSeaTooth"); show("clamperl+DST water gun", gen.calculate_damage(cl2, mv("Water Gun"), d))
refshow("", R.rolls(R.full(R.base_damage(30, 40, 'Water', CL.attack, D.defense, CL.special_attack, D.special_defense, deep_sea_tooth=True), stab=True)))
print("\n[24] Sea Incense / Dragon Fang / Silverpowder / Dragon Scale / Mystic Water")
for it, m in [("Sea Incense", "Water Gun"), ("Dragon Fang", "DragonBreath"), ("Silverpowder", "Fury Cutter"), ("Silver Powder", "Fury Cutter"), ("Dragon Scale", "DragonBreath"), ("Mystic Water", "Water Gun")]:
    ai = mon("Machop", 50, held_item=it); base = gen.calculate_damage(a, mv(m), d, custom_move_data="1"); boosted = gen.calculate_damage(ai, mv(m), d, custom_move_data="1")
    print(f"  {it} {m}: no-item max={base.max_damage} with-item max={boosted.max_damage}")
print("\n[25] crit multi-hit passes crit-modified stage mods to non-crit hits:")
show("Fury Attack 2 hits crit, atk -1", gen.calculate_damage(a, mv("Fury Attack"), d, custom_move_data="2 Hits", is_crit=True, attacking_stage_modifiers=StageModifiers(attack=-1)))
r_crit = R.rolls(R.full(R.base_damage(50, 15, 'Normal', A.attack, D.defense, A.special_attack, D.special_defense, atk_stage=-1, crit=True), crit=True))
r_non = R.rolls(R.full(R.base_damage(50, 15, 'Normal', A.attack, D.defense, A.special_attack, D.special_defense, atk_stage=-1)))
print(f"  REF crit hit {R.rng(r_crit)} + non-crit hit (atk -1 applies!) {R.rng(r_non)} -> {min(r_crit)+min(r_non)}..{max(r_crit)+max(r_non)}")
print("\n[26] crit multi-hit drops weather for non-crit hits: Icy Wind? no. Use Surf in rain is single-hit. Use spread+doubles for 2-hit (Bonemerang not spread). Skip.")
print("\n[27] DamageRange.add weighting (2 x 16 rolls should give 256 combos):")
dr = gen.calculate_damage(a, mv("Double Kick"), d); print("  Double Kick size:", dr.size, "(expected 256)")
print("\n[28] Eruption 100% / 50% / 1%")
tp = mon("Torkoal", 50); TP = tp.cur_stats
for pct in ["100", "50", "1"]:
    show(f"eruption {pct}", gen.calculate_damage(tp, mv("Eruption"), d, custom_move_data=pct))
refshow("100", R.rolls(R.full(R.base_damage(50, 150, 'Fire', TP.attack, D.defense, TP.special_attack, D.special_defense), stab=True)))
refshow("50 (hp 60/121 -> 150*60/121=74)", R.rolls(R.full(R.base_damage(50, 150 * 60 // 121, 'Fire', TP.attack, D.defense, TP.special_attack, D.special_defense), stab=True)))
print("  torkoal hp", TP.hp)
print("\n[29] SolarBeam in rain/sand/hail/sun, Surf in sun, Flamethrower in rain")
show("solarbeam rain", gen.calculate_damage(a, mv("SolarBeam"), d, weather=const.WEATHER_RAIN))
refshow("solarbeam rain", R.rolls(R.full(R.base_damage(50, 120, 'Grass', A.attack, D.defense, A.special_attack, D.special_defense, weather='rain', move_name='SolarBeam'))))
show("solarbeam sand", gen.calculate_damage(a, mv("SolarBeam"), d, weather=const.WEATHER_SANDSTORM))
show("surf sun", gen.calculate_damage(a, mv("Surf"), d, weather=const.WEATHER_SUN))
refshow("surf sun", R.rolls(R.full(R.base_damage(50, 95, 'Water', A.attack, D.defense, A.special_attack, D.special_defense, weather='sun'))))
show("flamethrower rain", gen.calculate_damage(a, mv("Flamethrower"), d, weather=const.WEATHER_RAIN))
refshow("flamethrower rain", R.rolls(R.full(R.base_damage(50, 95, 'Fire', A.attack, D.defense, A.special_attack, D.special_defense, weather='rain'))))
print("\n[30] Hustle / Huge Power / type item + stage ordering")
hp_ = mon("Azumarill", 50, ability="Huge Power"); HP = hp_.cur_stats
show("huge power +1 Return(102)", gen.calculate_damage(hp_, mv("Return"), d, custom_move_data="102", attacking_stage_modifiers=StageModifiers(attack=1)))
refshow("", R.rolls(R.full(R.base_damage(50, 102, 'Normal', HP.attack, D.defense, HP.special_attack, D.special_defense, atk_stage=1, huge_power=True))))
mw2 = mon("Machop", 50, held_item="Black Belt")
show("black belt karate chop, atk -1", gen.calculate_damage(mw2, mv("Karate Chop"), d, attacking_stage_modifiers=StageModifiers(attack=-1)))
refshow("", R.rolls(R.full(R.base_damage(50, 50, 'Fighting', A.attack, D.defense, A.special_attack, D.special_defense, atk_stage=-1, type_item_param=10), stab=True)))

