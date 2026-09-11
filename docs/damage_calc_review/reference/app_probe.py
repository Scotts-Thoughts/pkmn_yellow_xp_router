import sys, os, math
sys.path.insert(0, r"A:\pkmn_yellow_xp_router")
os.chdir(r"A:\pkmn_yellow_xp_router")
import controllers.main_controller
from pkmn.gen_4 import gen_four_object
from pkmn import universal_data_objects as udo
from utils.constants import const
g = gen_four_object.gen_four_platinum
STG = [(10,40),(10,35),(10,30),(10,25),(10,20),(10,15),(10,10),(15,10),(20,10),(25,10),(30,10),(35,10),(40,10)]

def divide(a, b):
    if a == 0:
        return 0
    s = -1 if a < 0 else 1
    q = int(a / b)
    return q if q != 0 else s

def game(level, power, atk, dfn, stab, effs, crit=1, as_=0, ds=0, screen=False, spread=False, weather=1.0):
    a = atk if (crit > 1 and as_ <= 0) else atk * STG[as_ + 6][0] // STG[as_ + 6][1]
    d = a * power * (level * 2 // 5 + 2)
    dv = dfn if (crit > 1 and ds >= 0) else dfn * STG[ds + 6][0] // STG[ds + 6][1]
    d = d // dv // 50
    if screen and crit == 1:
        d //= 2
    if spread:
        d = d * 3 // 4
    if weather == 1.5:
        d = d * 15 // 10
    elif weather == 0.5:
        d //= 2
    d += 2
    d *= crit
    if stab:
        d = d * 15 // 10
    for e in effs:
        d = divide(d * e, 10)
    out = {}
    for r in range(16):
        v = d * (100 - r) // 100
        if v == 0:
            v = 1
        out[v] = out.get(v, 0) + 1
    return dict(sorted(out.items()))

def show(tag, rng):
    print(tag, None if rng is None else dict(sorted(rng.damage_vals.items())))

atk = g.create_trainer_pkmn("Chimchar", 20)
dfn = g.create_trainer_pkmn("Starly", 20)
print("Chimchar L20", atk.cur_stats)
print("Starly L20", dfn.cur_stats)
A = atk.cur_stats
D = dfn.cur_stats

def mv(n):
    return g.move_db().get_move(n)

def calc(a, m, d, **kw):
    return g.calculate_damage(a, m, d, **kw)

print("\n[Scratch]"); show(" app", calc(atk, mv("Scratch"), dfn)); print(" game", game(20, 40, A.attack, D.defense, False, []))
print("\n[Ember STAB]"); show(" app", calc(atk, mv("Ember"), dfn)); print(" game", game(20, 40, A.special_attack, D.special_defense, True, []))
print("\n[Rollout 1 vs Flying SE]"); show(" app", calc(atk, mv("Rollout"), dfn, custom_move_data="1")); print(" game(power 30)", game(20, 30, A.attack, D.defense, False, [20]))
print("\n[Rollout 5 + DefenseCurl]"); show(" app", calc(atk, mv("Rollout"), dfn, custom_move_data="5 + DefenseCurl")); print(" game(power 30*16*2=960)", game(20, 960, A.attack, D.defense, False, [20]))
print("\n[Fury Cutter 6]"); show(" app", calc(atk, mv("Fury Cutter"), dfn, custom_move_data="6")); print(" game(power 160, Bug vs Flying NVE)", game(20, 160, A.attack, D.defense, False, [5]))
print("\n[Gyro Ball]"); show(" app", calc(atk, mv("Gyro Ball"), dfn, custom_move_data=""))
gp = min(150, 1 + 25 * D.speed // A.speed)
print(" game power=1+25*defSpe/atkSpe =", gp, "->", game(20, gp, A.attack, D.defense, False, []))
print("\n[Trump Card 4+]"); show(" app", calc(atk, mv("Trump Card"), dfn, custom_move_data="4+")); print(" game(power 40)", game(20, 40, A.special_attack, D.special_defense, False, []))
print("\n[Punishment, defender +2 Atk]"); dm = udo.StageModifiers(attack=2); show(" app", calc(atk, mv("Punishment"), dfn, defending_stage_modifiers=dm)); print(" game(power 100)", game(20, 100, A.attack, D.defense, False, []))
print("\n[Wring Out 100]"); show(" app", calc(atk, mv("Wring Out"), dfn, custom_move_data="100")); print(" game(power 121)", game(20, 121, A.special_attack, D.special_defense, False, []))
print("\n[Spit Up 3]"); show(" app", calc(atk, mv("Spit Up"), dfn, custom_move_data="3")); d0 = game(20, 300, A.special_attack, D.special_defense, False, []); print(" game(power 300, no variance) =", max(d0))
print("\n[Present]"); show(" app", calc(atk, mv("Present"), dfn)); print(" game(40/80/120 @ 40%/30%/10%, heal 20%) p40:", game(20, 40, A.attack, D.defense, False, []))
print("\n[Frustration]"); show(" app", calc(atk, mv("Frustration"), dfn, custom_move_data="")); print(" game(friendship 0 -> power 102)", game(20, 102, A.attack, D.defense, False, []))
print("\n[Double Hit]"); show(" app", calc(atk, mv("Double Hit"), dfn)); print(" game: TWO hits of", game(20, 35, A.attack, D.defense, False, []))
gs = g.create_trainer_pkmn("Gastly", 20)
print("\n[Struggle vs Gastly]"); show(" app", calc(atk, mv("Struggle"), gs)); print(" game: typeless 50 power hits Ghost:", game(20, 50, A.attack, gs.cur_stats.defense, False, []))
geo = g.create_trainer_pkmn("Geodude", 20)
print("\n[Earthquake doubles spread vs Geodude]"); show(" app", calc(atk, mv("Earthquake"), geo, is_double_battle=True)); print(" game(x3/4, Ground vs Rock/Ground: SE*NVE)", game(20, 100, A.attack, geo.cur_stats.defense, False, [20], spread=True))
print("\n[crit + stages: atk +2, spa -1, physical Scratch, crit]"); am = udo.StageModifiers(attack=2, special_attack=-1); show(" app", calc(atk, mv("Scratch"), dfn, attacking_stage_modifiers=am, is_crit=True)); print(" game (keeps +2 atk)", game(20, 40, A.attack, D.defense, False, [], crit=2, as_=2))
print("\n[crit + stages: def +2 on defender, Scratch crit]"); dm2 = udo.StageModifiers(defense=2); show(" app", calc(atk, mv("Scratch"), dfn, defending_stage_modifiers=dm2, is_crit=True)); print(" game (ignores +2 def)", game(20, 40, A.attack, D.defense, False, [], crit=2, ds=2))
print("\n[crit rate Night Slash]", g.get_crit_rate(atk, mv("Night Slash"), ""), "game: 1/8")
print("[crit rate Slash]", g.get_crit_rate(atk, mv("Slash"), ""))
sn = g.create_trainer_pkmn("Snover", 20); sn.ability = "Snow Cloak"
print("[Snow Cloak acc] hail:", g.get_move_accuracy(atk, mv("Scratch"), "", sn, const.WEATHER_HAIL), " sandstorm:", g.get_move_accuracy(atk, mv("Scratch"), "", sn, const.WEATHER_SANDSTORM), " game: hail 80, sand 100")
t = g.create_trainer_pkmn("Scizor", 30); t.ability = "Technician"
print("\n[Technician Bullet Punch 40 vs Starly]"); show(" app", calc(t, mv("Bullet Punch"), dfn)); print(" game power 60:", game(30, 60, t.cur_stats.attack, D.defense, True, []))
print("\n[Technician Rollout turn 3 (30*4=120 > 60 -> no boost)]"); show(" app", calc(t, mv("Rollout"), dfn, custom_move_data="3")); print(" game power 120:", game(30, 120, t.cur_stats.attack, D.defense, False, [20]))
mk = g.create_trainer_pkmn("Magikarp", 20)
print("\n[Low Kick vs Magikarp 10.0kg]"); show(" app", calc(atk, mv("Low Kick"), mk)); print(" game power 20:", game(20, 20, A.attack, mk.cur_stats.defense, False, []))
print("\n[Nature Power Plain]"); show(" app", calc(atk, mv("Nature Power"), dfn, custom_move_data="Plain")); print(" game: Earthquake 100 Ground vs Flying = immune -> None")
ob = g.create_trainer_pkmn("Chimchar", 20); ob.held_item = "Occa Berry"
print("\n[Natural Gift Occa Berry]"); show(" app", calc(ob, mv("Natural Gift"), dfn)); print(" game power 60 Fire STAB:", game(20, 60, A.attack, D.defense, True, []))
ar = g.create_trainer_pkmn("Arceus", 50); ar.held_item = "Earth Plate"; ar.ability = "Multitype"
print("\n[Judgment Earth Plate vs Starly]"); show(" app", calc(ar, mv("Judgment"), dfn)); print(" game: Ground vs Flying -> immune (None)")
pt = udo.FieldStatus(power_trick=True); s = atk.get_battle_stats(udo.StageModifiers(), mon_field=pt)
print("\n[Power Trick] app stats:", s, " game swaps Atk<->Def: atk", A.defense, "def", A.attack)
ch = g.create_trainer_pkmn("Cherrim", 20); ch.ability = "Flower Gift"; C = ch.cur_stats
print("\n[Flower Gift defender in sun, Ember]"); show(" app", calc(atk, mv("Ember"), ch, weather=const.WEATHER_SUN)); print(" game: SpD*1.5 =", C.special_defense * 15 // 10, "->", game(20, 40, A.special_attack, C.special_defense * 15 // 10, True, [20], weather=1.5))
sl = g.create_trainer_pkmn("Snorlax", 20); sl.ability = "Thick Fat"
print("\n[Thick Fat Ember vs Snorlax]"); show(" app", calc(atk, mv("Ember"), sl)); print(" game power 20:", game(20, 20, A.special_attack, sl.cur_stats.special_defense, True, []))
pk = g.create_trainer_pkmn("Pikachu", 20); pk.held_item = "Light Ball"; P = pk.cur_stats
print("\n[Light Ball Pikachu Quick Attack]"); show(" app", calc(pk, mv("Quick Attack"), dfn)); print(" game power 80:", game(20, 80, P.attack, D.defense, True, []))
print("\n[Triple Kick 3]"); show(" app", calc(atk, mv("Triple Kick"), dfn, custom_move_data="3")); print(" game: hits of 10/20/30 power:", game(20, 10, A.attack, D.defense, False, [20, 5]), game(20, 20, A.attack, D.defense, False, [20, 5]), game(20, 30, A.attack, D.defense, False, [20, 5]))
print("\n[Rage 3]"); show(" app", calc(atk, mv("Rage"), dfn, custom_move_data="3")); print(" game: plain 20 power:", game(20, 20, A.attack, D.defense, False, []))
pw = calc(atk, mv("Psywave"), dfn)
print("\n[Psywave] app size:", len(pw.damage_vals), "vals", sorted(pw.damage_vals), " game: {", sorted(set(max(1, 20 * (r + 5) // 10) for r in range(11))), "} uniform over r=0..10")
print("\n[Accuracy: Thunder in sun] app:", g.get_move_accuracy(atk, mv("Thunder"), "", dfn, const.WEATHER_SUN), "game: 50")
print("[Accuracy: Nature Power Plain] app:", g.get_move_accuracy(atk, mv("Nature Power"), "Plain", dfn, const.WEATHER_NONE), "game: Earthquake 100")
print("[Accuracy: Fissure L20 vs L20] app:", g.get_move_accuracy(atk, mv("Fissure"), "", dfn, const.WEATHER_NONE), "game: 30+(20-20)=30")
sd = g.create_trainer_pkmn("Latios", 50); sd.held_item = "Soul Dew"
print("\n[Soul Dew Latios Dragon Pulse]"); show(" app", calc(sd, mv("Dragon Pulse"), dfn)); L = sd.cur_stats; print(" game SpA*1.5=", L.special_attack * 15 // 10, "->", game(50, 90, L.special_attack * 15 // 10, D.special_defense, True, []))
lo = g.create_trainer_pkmn("Chimchar", 20); lo.held_item = "Life Orb"
print("\n[Life Orb Scratch]"); show(" app", calc(lo, mv("Scratch"), dfn)); base = game(20, 40, A.attack, D.defense, False, []); print(" game: x1.3 after crit, before STAB/type; base", base)
print("\n[Dragon Scale / Dragon Fang Dragon Pulse]"); ds1 = g.create_trainer_pkmn("Chimchar", 20); ds1.held_item = "Dragon Scale"; show(" app scale", calc(ds1, mv("Dragon Pulse"), dfn)); ds1.held_item = "Dragon Fang"; show(" app fang", calc(ds1, mv("Dragon Pulse"), dfn)); print(" game: Scale no boost; Fang power 108:", game(20, 108, A.special_attack, D.special_defense, False, []), " none:", game(20, 90, A.special_attack, D.special_defense, False, []))
print("\n[Weather Ball fog]"); show(" app", calc(atk, mv("Weather Ball"), dfn, weather=const.WEATHER_FOG)); print(" game: power 100 Normal:", game(20, 100, A.special_attack, D.special_defense, False, []))
print("\n[SolarBeam in fog]"); show(" app", calc(atk, mv("SolarBeam"), dfn, weather=const.WEATHER_FOG)); print(" game: halved:", game(20, 120, A.special_attack, D.special_defense, False, [5], weather=0.5))
print("\n[Nature Power Sand / Cave]"); show(" app sand", calc(atk, mv("Nature Power"), dfn, custom_move_data="Sand")); show(" app cave", calc(atk, mv("Nature Power"), dfn, custom_move_data="Cave"))
print("[Hidden Power (IVs 8/9/8/8/8/8)] app:", g.get_hidden_power(atk.dvs), " game: Fighting 30")
hp = calc(atk, mv("Hidden Power"), dfn); show(" app HP dmg", hp); print(" game (Fighting 30 vs Normal/Flying):", game(20, 30, A.special_attack, D.special_defense, False, [20, 5]))
tt = g.create_trainer_pkmn("Scizor", 30); tt.ability = "Technician"
print("\n[Technician Low Kick vs Magikarp (20 -> 30)]"); show(" app", calc(tt, mv("Low Kick"), mk)); print(" game power 30:", game(30, 30, tt.cur_stats.attack, mk.cur_stats.defense, False, []))
print("\n[Technician Fury Cutter 2 (10*2=20 -> 30)]"); show(" app", calc(tt, mv("Fury Cutter"), dfn, custom_move_data="2")); print(" game power 30:", game(30, 30, tt.cur_stats.attack, D.defense, True, [5]))
print("\n[Future Sight vs Gastly (Ghost/Poison)]"); show(" app", calc(atk, mv("Future Sight"), gs)); print(" game: no STAB/type chart, power 80:", game(20, 80, A.special_attack, gs.cur_stats.special_defense, False, []))
print("\n[Spit Up 3 crit]"); show(" app", calc(atk, mv("Spit Up"), dfn, custom_move_data="3", is_crit=True)); print(" game crit allowed: 2x")
print("\n[Choice Band + stage +1 Scratch]"); cb = g.create_trainer_pkmn("Chimchar", 20); cb.held_item = "Choice Band"; show(" app", calc(cb, mv("Scratch"), dfn, attacking_stage_modifiers=udo.StageModifiers(attack=1))); print(" game atk=floor(floor(30*1.5)*1.5)=", (A.attack*150//100)*15//10, "->", game(20, 40, A.attack*150//100, D.defense, False, [], as_=1))
