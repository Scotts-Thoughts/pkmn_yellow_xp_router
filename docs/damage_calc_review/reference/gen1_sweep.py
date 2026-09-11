import sys
sys.path.insert(0, r"C:\Users\scott\AppData\Local\Temp\claude\A--pkmn-yellow-xp-router\7f621085-1927-4e9c-a4ad-317d76695d59\scratchpad")
from gen1_ref_harness import *

def quiet(attacker, move_name, defender, astages=None, dstages=None, crit=False, screen=False):
    move = gen.move_db().get_move(move_name)
    asp, dsp = species(attacker.name), species(defender.name)
    special = move.move_type in SPECIAL
    astat = stats_of(attacker, astages, crit); dstat = stats_of(defender, dstages, crit)
    atk = astat.special_attack if special else astat.attack
    dfn = dstat.special_attack if special else dstat.defense
    stab = move.move_type in (asp.first_type, asp.second_type)
    mults = rom_mults(move.move_type, dsp.first_type, dsp.second_type)
    explode = move.name in (const.EXPLOSION_MOVE_NAME, const.SELFDESTRUCT_MOVE_NAME)
    g = game_damage(attacker.level, move.base_power, atk, dfn, is_crit=crit, screen=screen, explode=explode, stab=stab, mults=mults)
    field = FieldStatus(reflect=screen and not special, light_screen=screen and special)
    a = app_rolls(gen.calculate_damage(attacker, move, defender, attacking_stage_modifiers=astages, defending_stage_modifiers=dstages, defending_field=field, is_crit=crit))
    return g, a, atk, dfn, stab, mults

print("=== SWEEP A: stat scaling (>255) / screens / explosion / crit")
cases = [("Alakazam","Psychic","Dewgong"),("Alakazam","Psychic","Slowbro"),("Nidoking","Earthquake","Rhydon"),("Nidoking","Thunderbolt","Gyarados"),
         ("Electrode","Explosion","Cloyster"),("Snorlax","Body Slam","Chansey"),("Machamp","Submission","Snorlax"),("Nidoking","Horn Attack","Onix"),
         ("Starmie","Surf","Rhydon"),("Tauros","Hyper Beam","Lapras")]
tot = 0; bad = 0; shown = 0
for an, mv, dn in cases:
    for L in range(30, 81, 5):
        for ast in (0, 2, 4, 6):
            for dst in (0, 2, 6):
                for screen in (False, True):
                    for crit in (False, True):
                        a = mon(an, L); d = mon(dn, L)
                        g, ap, atk, dfn, stab, mults = quiet(a, mv, d, StageModifiers(attack=ast, special_attack=ast), StageModifiers(defense=dst, special_defense=dst, special_attack=dst), crit, screen)
                        tot += 1
                        if g != ap:
                            bad += 1
                            if shown < 14 and not (crit and ast == 0):
                                shown += 1
                                print(f"  L{L} {an} {mv} +{ast} vs {dn} +{dst} screen={screen} crit={crit} | atk={atk} def={dfn} stab={stab} mults={mults}\n      game={fmt(g)}  app={fmt(ap)}")
print(f"  total={tot} mismatches={bad}")

print("\n=== SWEEP B: type table order (defender dual-type SE+NVE)")
tcases = [("Tangela","Vine Whip","Nidoking"),("Tangela","Razor Leaf","Nidoqueen"),("Nidoran M","Poison Sting","Weedle"),("Nidoran M","Poison Sting","Kakuna"),("Arbok","Sludge","Beedrill"),
          ("Charizard","Flamethrower","Dewgong"),("Charizard","Fire Blast","Lapras"),("Machamp","Karate Chop","Jynx"),("Hitmonlee","Hi Jump Kick","Articuno"),("Zapdos","Thunderbolt","Dragonite"),
          ("Jolteon","Thunderbolt","Dragonite"),("Beedrill","Twineedle","Golbat"),("Machamp","Submission","Aerodactyl")]
for an, mv, dn in tcases:
    found = 0
    for L in range(5, 71, 1):
        a = mon(an, L); d = mon(dn, L)
        g, ap, atk, dfn, stab, mults = quiet(a, mv, d)
        if g != ap and found < 2:
            found += 1
            print(f"  L{L} {an} {mv} vs {dn} | atk={atk} def={dfn} stab={stab} ROM order mults={mults}\n      game={fmt(g)}  app={fmt(ap)}   game rolls={dict(sorted(g.items()))}\n      app rolls={dict(sorted(ap.items()))}")
    if not found: print(f"  {an} {mv} vs {dn}: no mismatch found in L5-70")

print("\n=== C: Super Fang app output vs neutral / immune targets")
for dn in ("Rattata", "Gastly", "Chansey"):
    dr = gen.calculate_damage(mon("Raticate", 30), gen.move_db().get_move("Super Fang"), mon(dn, 30))
    print(f"  Raticate L30 Super Fang vs {dn}: {None if dr is None else dict(sorted(dr.damage_vals.items()))}")
for L in (10, 20, 40, 60):
    dr = gen.calculate_damage(mon("Raticate", L), gen.move_db().get_move("Super Fang"), mon("Rattata", L))
    print(f"  Raticate L{L} Super Fang vs Rattata L{L}: {None if dr is None else dict(sorted(dr.damage_vals.items()))}")

print("\n=== D: hand-check numbers for findings test cases")
def show(an, L, mv, dn, DL, ast=0, dst=0, screen=False, crit=False):
    a = mon(an, L); d = mon(dn, DL)
    g, ap, atk, dfn, stab, mults = quiet(a, mv, d, StageModifiers(attack=ast, special_attack=ast), StageModifiers(defense=dst, special_defense=dst, special_attack=dst), crit, screen)
    print(f"  L{L} {an} {mv} +{ast} vs L{DL} {dn} +{dst} screen={screen} crit={crit} | atk={atk} def={dfn} stab={stab} mults={mults}\n      game={dict(sorted(g.items())) if isinstance(g, dict) else g}\n      app ={dict(sorted(ap.items())) if isinstance(ap, dict) else ap}")
show("Alakazam", 61, "Psychic", "Dewgong", 54, ast=2)
show("Nidoking", 50, "Horn Attack", "Onix", 50, dst=6, screen=True)
show("Nidoking", 50, "Earthquake", "Rhydon", 50, ast=6)
show("Electrode", 45, "Explosion", "Cloyster", 45, dst=2)
show("Nidoking", 45, "Thunderbolt", "Gyarados", 45, screen=True)
show("Snorlax", 100, "Explosion", "Chansey", 5, crit=True)
