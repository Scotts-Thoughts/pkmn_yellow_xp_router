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
from pkmn.gen_3.data_objects import GenThreeStatBlock
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
        print(f"  APP {label}: None"); return
    print(f"  APP {label}: min={dr.min_damage} max={dr.max_damage} n={dr.size} vals={dict(sorted(dr.damage_vals.items()))}")

def refshow(label, d):
    print(f"  REF {label}: min={min(d)} max={max(d)} vals={dict(sorted(d.items()))}")

def stats(m):
    s = m.cur_stats; return (s.hp, s.attack, s.defense, s.special_attack, s.special_defense, s.speed)

a = mon("Machop", 50); d = mon("Machop", 50); A = a.cur_stats; D = d.cur_stats

print("[A] Latios L50 trainer stats:", stats(mon("Latios", 50)))
lt = mon("Latios", 50, held_item="Soul Dew"); LT = lt.cur_stats
show("latios soul dew psychic vs machop", gen.calculate_damage(lt, mv("Psychic"), d))
refshow("game (soul dew, STAB, SE)", R.rolls(R.full(R.base_damage(50, 90, 'Psychic', LT.attack, D.defense, LT.special_attack, D.special_defense, soul_dew_atk=True), stab=True, effs=(20,))))
refshow("game (NO soul dew)", R.rolls(R.full(R.base_damage(50, 90, 'Psychic', LT.attack, D.defense, LT.special_attack, D.special_defense), stab=True, effs=(20,))))
lt0 = mon("Latios", 50); show("latios no item", gen.calculate_damage(lt0, mv("Psychic"), d))

print("\n[B] Explosion, defender Def 63 (Geodude-ish) at -1")
# find a defender whose def is 63 at some level: brute force over db
found = None
for name in ["Geodude", "Onix", "Sandshrew", "Slugma", "Aron", "Makuhita", "Poochyena", "Zigzagoon", "Wurmple", "Machop", "Nosepass", "Whismur", "Lotad", "Seedot", "Taillow", "Wingull", "Ralts", "Surskit", "Shroomish", "Slakoth", "Nincada", "Abra", "Meditite", "Electrike", "Plusle", "Minun", "Volbeat", "Illumise", "Oddish", "Gulpin", "Carvanha", "Wailmer", "Numel", "Torkoal", "Spoink", "Spinda", "Trapinch", "Cacnea", "Swablu", "Zangoose", "Seviper", "Lunatone", "Solrock", "Barboach", "Corphish", "Baltoy", "Lileep", "Anorith", "Feebas", "Castform", "Kecleon", "Shuppet", "Duskull", "Tropius", "Chimecho", "Absol", "Wynaut", "Snorunt", "Spheal", "Clamperl", "Relicanth", "Luvdisc", "Bagon", "Beldum"]:
    for lvl in range(10, 60):
        try:
            m = mon(name, lvl)
        except Exception:
            break
        if m.cur_stats.defense == 63:
            found = (name, lvl); break
    if found: break
print("  defender with Def 63:", found)
if found:
    dd = mon(*found); DD = dd.cur_stats
    show("explosion def-1", gen.calculate_damage(a, mv("Explosion"), dd, defending_stage_modifiers=StageModifiers(defense=-1)))
    sp = gen.pkmn_db().get_pkmn(found[0]); print("  types", sp.first_type, sp.second_type)
    effs = []
    for t in (sp.first_type, sp.second_type):
        e = gen._type_chart['Normal'].get(t)
        if e == const.SUPER_EFFECTIVE: effs.append(20)
        elif e == const.NOT_VERY_EFFECTIVE: effs.append(5)
        elif e == const.IMMUNE: effs.append(0)
    if sp.first_type == sp.second_type: effs = effs[:1]
    refshow("game", R.rolls(R.full(R.base_damage(50, 250, 'Normal', A.attack, DD.defense, A.special_attack, DD.special_defense, def_stage=-1, explosion=True), effs=tuple(effs))))
    print("  detail: game def path 63//2=31 -> 31*10//15 =", 31*10//15, "; app path 63*10//15=", 63*10//15, "-> //2 =", (63*10//15)//2)

print("\n[C] Thick Fat with attacker SpA 45 at -1 (find mon with SpA 45)")
found = None
for name in ["Machop", "Makuhita", "Geodude", "Zigzagoon", "Poochyena", "Taillow", "Wurmple", "Whismur", "Aron", "Slakoth", "Nincada", "Meditite", "Numel", "Trapinch", "Barboach", "Corphish", "Shuppet", "Spheal", "Snorunt", "Bagon", "Torkoal", "Spinda", "Zangoose", "Seviper", "Carvanha"]:
    for lvl in range(10, 70):
        m = mon(name, lvl)
        if m.cur_stats.special_attack == 45:
            found = (name, lvl); break
    if found: break
print("  attacker with SpA 45:", found)
if found:
    at = mon(*found); AT = at.cur_stats; tf = mon("Machop", 50, ability="Thick Fat")
    show("ember spa-1 vs thick fat", gen.calculate_damage(at, mv("Ember"), tf, attacking_stage_modifiers=StageModifiers(special_attack=-1)))
    refshow("game", R.rolls(R.full(R.base_damage(found[1], 40, 'Fire', AT.attack, D.defense, AT.special_attack, D.special_defense, spa_stage=-1, thick_fat=True))))
    print("  detail: game 45//2=22 -> 22*10//15 =", 22*10//15, "; app 45*10//15 =", 45*10//15, "-> //2 =", (45*10//15)//2)

print("\n[D] Choice Band with attacker Atk 92 at -1")
found = None
for name in ["Machop", "Makuhita", "Geodude", "Zigzagoon", "Poochyena", "Taillow", "Aron", "Slakoth", "Nincada", "Meditite", "Numel", "Trapinch", "Corphish", "Bagon", "Zangoose", "Seviper", "Carvanha", "Mightyena", "Breloom", "Vigoroth"]:
    for lvl in range(10, 80):
        m = mon(name, lvl)
        if m.cur_stats.attack == 92:
            found = (name, lvl); break
    if found: break
print("  attacker with Atk 92:", found)
if found:
    at = mon(found[0], found[1], held_item="Choice Band"); AT = at.cur_stats
    sp = gen.pkmn_db().get_pkmn(found[0]); stab = 'Normal' in (sp.first_type, sp.second_type)
    show("tackle cb atk-1", gen.calculate_damage(at, mv("Tackle"), d, attacking_stage_modifiers=StageModifiers(attack=-1)))
    refshow("game", R.rolls(R.full(R.base_damage(found[1], 35, 'Normal', AT.attack, D.defense, AT.special_attack, D.special_defense, atk_stage=-1, choice_band=True), stab=stab)))
    print("  detail: game 92*150//100=138 -> 138*10//15 =", 138*10//15, "; app 92*10//15 =", 92*10//15, "-> *1.5 floor =", int((92*10//15)*1.5))

print("\n[E] Black Belt + Atk +1 (Machop atk 89)")
bb = mon("Machop", 50, held_item="Black Belt")
show("karate chop bb +1", gen.calculate_damage(bb, mv("Karate Chop"), d, attacking_stage_modifiers=StageModifiers(attack=1)))
refshow("game", R.rolls(R.full(R.base_damage(50, 50, 'Fighting', A.attack, D.defense, A.special_attack, D.special_defense, atk_stage=1, type_item_param=10), stab=True)))
print("  detail: game 89*110//100=97 -> 97*15//10 =", 97*15//10, "; app 89*15//10=133 -> floor(133*1.1) =", int(133*1.1))

print("\n[F] physical min-1 edge: Rattata L2 Tackle vs Machop L50 (neutral)")
r = mon("Rattata", 2); RR = r.cur_stats; print("  rattata atk", RR.attack, "machop def", D.defense)
show("", gen.calculate_damage(r, mv("Tackle"), d))
refshow("game", R.rolls(R.full(R.base_damage(2, 35, 'Normal', RR.attack, D.defense, RR.special_attack, D.special_defense), stab=True)))

print("\n[G] Future Sight (Psychic) vs Machop (Fighting): app applies SE, game applies no type calc")
show("future sight", gen.calculate_damage(a, mv("Future Sight"), d))
refshow("game", R.rolls(R.base_damage(50, 80, 'Psychic', A.attack, D.defense, A.special_attack, D.special_defense)))
ab = mon("Abra", 50); AB = ab.cur_stats
show("abra future sight vs machop (STAB+SE in app)", gen.calculate_damage(ab, mv("Future Sight"), d))
refshow("game", R.rolls(R.base_damage(50, 80, 'Psychic', AB.attack, D.defense, AB.special_attack, D.special_defense)))
dk = mon("Poochyena", 50)
show("future sight vs Dark type (app: None; game: hits)", gen.calculate_damage(a, mv("Future Sight"), dk))

print("\n[H] Beat Up (Machop L50, party = just itself): base atk 80, target base def 50")
show("beat up", gen.calculate_damage(a, mv("Beat Up"), d))
one = 80 * 10 * (50 * 2 // 5 + 2) // 50 // 50 + 2
print("  game per-hit base:", one, "rolls:", R.rolls(one), "crit:", R.rolls(one * 2))

print("\n[I] Water Absorb defender vs Fire move; Flash Fire defender vs Fire move")
wa = mon("Machop", 50, ability="Water Absorb"); ff = mon("Machop", 50, ability="Flash Fire")
show("ember vs water absorb", gen.calculate_damage(a, mv("Ember"), wa))
show("ember vs flash fire", gen.calculate_damage(a, mv("Ember"), ff))
refshow("game ember vs water absorb", R.rolls(R.full(R.base_damage(50, 40, 'Fire', A.attack, D.defense, A.special_attack, D.special_defense))))

print("\n[J] Hidden Power Fire (special) with Hustle accuracy; HP type from DVs")
hpm = mon("Machop", 50, ability="Hustle"); hpm.dvs = GenThreeStatBlock(31, 30, 30, 30, 31, 30)  # (hp, atk, def, spa, spd, spe)
from pkmn.gen_3.data_objects import get_hidden_power_type, get_hidden_power_base_power
print("  HP type/power:", get_hidden_power_type(hpm.dvs), get_hidden_power_base_power(hpm.dvs))
print("  app accuracy:", gen.get_move_accuracy(hpm, mv("Hidden Power"), "", d, const.WEATHER_NONE))

print("\n[K] Psywave L50 app distribution size:", gen.calculate_damage(a, mv("Psywave"), d).size, "(game: 11 outcomes 25..75 step 5)")
print("\n[L] Machop L50 Karate Chop crit with defender Def +2 and attacker Atk -2")
show("crit", gen.calculate_damage(a, mv("Karate Chop"), d, is_crit=True, attacking_stage_modifiers=StageModifiers(attack=-2, special_attack=3), defending_stage_modifiers=StageModifiers(defense=2)))
refshow("game", R.rolls(R.full(R.base_damage(50, 50, 'Fighting', A.attack, D.defense, A.special_attack, D.special_defense, atk_stage=-2, def_stage=2, crit=True), crit=True, stab=True)))
show("non-crit", gen.calculate_damage(a, mv("Karate Chop"), d, attacking_stage_modifiers=StageModifiers(attack=-2), defending_stage_modifiers=StageModifiers(defense=2)))
refshow("game", R.rolls(R.full(R.base_damage(50, 50, 'Fighting', A.attack, D.defense, A.special_attack, D.special_defense, atk_stage=-2, def_stage=2), stab=True)))
print("\n[M] Surf in doubles (spread) singles vs doubles")
show("surf doubles", gen.calculate_damage(a, mv("Surf"), d, is_double_battle=True))
refshow("game", R.rolls(R.full(R.base_damage(50, 95, 'Water', A.attack, D.defense, A.special_attack, D.special_defense, doubles=True, spread=True, two_alive=True))))
show("earthquake doubles (not halved in gen3)", gen.calculate_damage(a, mv("Earthquake"), d, is_double_battle=True, custom_move_data="No Bonus"))
refshow("game", R.rolls(R.full(R.base_damage(50, 100, 'Ground', A.attack, D.defense, A.special_attack, D.special_defense, doubles=True, spread=False, two_alive=True))))
print("\n[N] Sky Attack / Blaze Kick / Poison Tail crit rates:", [gen.get_crit_rate(a, mv(n), '') for n in ("Sky Attack", "Blaze Kick", "Poison Tail", "Razor Wind")])
print("\n[O] Return 102 vs Frustration (app):")
show("return 102", gen.calculate_damage(a, mv("Return"), d, custom_move_data="102"))
show("frustration", gen.calculate_damage(a, mv("Frustration"), d, custom_move_data=""))
refshow("game return 102 / frustration@0 friendship (both 102 BP)", R.rolls(R.full(R.base_damage(50, 102, 'Normal', A.attack, D.defense, A.special_attack, D.special_defense))))
print("\n[P] Low Kick vs Machop (19.5 kg = 195 hg -> 40 BP), vs Onix (210 kg -> 120)")
show("low kick vs machop", gen.calculate_damage(a, mv("Low Kick"), d))
refshow("game vs machop (40 BP, STAB)", R.rolls(R.full(R.base_damage(50, 40, 'Fighting', A.attack, D.defense, A.special_attack, D.special_defense), stab=True)))
o = mon("Onix", 50); OO = o.cur_stats
show("low kick vs onix", gen.calculate_damage(a, mv("Low Kick"), o))
refshow("game vs onix (120 BP, STAB, Rock SE, Ground neutral)", R.rolls(R.full(R.base_damage(50, 120, 'Fighting', A.attack, OO.defense, A.special_attack, OO.special_defense), stab=True, effs=(20,))))
print("\n[Q] Super Fang vs Machop (hp 134): game 67; app:")
show("super fang", gen.calculate_damage(a, mv("Super Fang"), d))
print("\n[R] Present vs Machop: game 40/80/120 BP:")
for bp in (40, 80, 120):
    refshow(f"game {bp}", R.rolls(R.full(R.base_damage(50, bp, 'Normal', A.attack, D.defense, A.special_attack, D.special_defense))))
show("app", gen.calculate_damage(a, mv("Present"), d))
