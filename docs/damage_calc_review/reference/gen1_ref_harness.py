"""Game-exact Gen 1 damage reference (transcribed from pokeyellow engine/battle/core.asm)
compared against the app's pkmn.gen_1 calc. Read-only w.r.t. the app repo."""
import sys, os, math
sys.path.insert(0, r"A:\pkmn_yellow_xp_router")
os.chdir(r"A:\pkmn_yellow_xp_router")
import controllers.main_controller  # noqa: F401  (import-order workaround, as in tests/conftest.py)
from utils.constants import const
from pkmn import gen_factory
from pkmn.gen_1 import gen_one_object, pkmn_utils
from pkmn.gen_1.data_objects import GenOneBadgeList
from pkmn.universal_data_objects import StageModifiers, FieldStatus

try:
    gen_factory._gen_factory.register_gen(gen_one_object.gen_one_yellow, const.YELLOW_VERSION)
except ValueError:
    pass
gen_factory.change_version(const.YELLOW_VERSION)
gen = gen_factory.current_gen_info()

# ---------------------------------------------------------------- game reference
SPECIAL = {"Fire", "Water", "Grass", "Electric", "Psychic", "Ice", "Dragon"}
ROM_ROWS = [l.split() for l in """WATER FIRE 20
FIRE GRASS 20
FIRE ICE 20
GRASS WATER 20
ELECTRIC WATER 20
WATER ROCK 20
GROUND FLYING 0
WATER WATER 5
FIRE FIRE 5
ELECTRIC ELECTRIC 5
ICE ICE 5
GRASS GRASS 5
PSYCHIC PSYCHIC 5
FIRE WATER 5
GRASS FIRE 5
WATER GRASS 5
ELECTRIC GRASS 5
NORMAL ROCK 5
NORMAL GHOST 0
GHOST GHOST 20
FIRE BUG 20
FIRE ROCK 5
WATER GROUND 20
ELECTRIC GROUND 0
ELECTRIC FLYING 20
GRASS GROUND 20
GRASS BUG 5
GRASS POISON 5
GRASS ROCK 20
GRASS FLYING 5
ICE WATER 5
ICE GRASS 20
ICE GROUND 20
ICE FLYING 20
FIGHTING NORMAL 20
FIGHTING POISON 5
FIGHTING FLYING 5
FIGHTING PSYCHIC 5
FIGHTING BUG 5
FIGHTING ROCK 20
FIGHTING ICE 20
FIGHTING GHOST 0
POISON GRASS 20
POISON POISON 5
POISON GROUND 5
POISON BUG 20
POISON ROCK 5
POISON GHOST 5
GROUND FIRE 20
GROUND ELECTRIC 20
GROUND GRASS 5
GROUND BUG 5
GROUND ROCK 20
GROUND POISON 20
FLYING ELECTRIC 5
FLYING FIGHTING 20
FLYING BUG 20
FLYING GRASS 20
FLYING ROCK 5
PSYCHIC FIGHTING 20
PSYCHIC POISON 20
BUG FIRE 5
BUG GRASS 20
BUG FIGHTING 5
BUG FLYING 5
BUG PSYCHIC 20
BUG GHOST 5
BUG POISON 20
ROCK FIRE 20
ROCK FIGHTING 5
ROCK GROUND 5
ROCK FLYING 20
ROCK BUG 20
ROCK ICE 20
GHOST NORMAL 0
GHOST PSYCHIC 0
FIRE DRAGON 5
WATER DRAGON 5
ELECTRIC DRAGON 5
GRASS DRAGON 5
ICE DRAGON 20
DRAGON DRAGON 20""".splitlines()]
ROM_ROWS = [(a.capitalize(), d.capitalize(), int(m)) for a, d, m in ROM_ROWS]


def rom_mults(move_type, t1, t2):
    return [m for a, d, m in ROM_ROWS if a == move_type and (d == t1 or d == t2)]


def game_calc_stat(base, dv, statexp, level, hp=False):
    # home/move_mon.asm CalcStat: b = min(255, ceil(sqrt(statexp))); term = b >> 2
    b = 0
    while True:
        b += 1
        if b == 255:
            break
        if b * b >= statexp:
            break
    t = (base + dv) * 2 + (b >> 2)
    v = (t * level) // 100
    v += (level + 10) if hp else 5
    return min(v, 999)


def game_damage(level, bp, atk, dfn, *, is_crit=False, screen=False, explode=False, stab=False, mults=()):
    """atk/dfn = 16-bit stats as the game reads them (in-battle modified, or unmodified on a crit).
    Returns dict roll->count, or None (no effect / miss)."""
    if screen and not is_crit:
        dfn = dfn * 2                      # sla c / rl b : 16-bit, no cap
    if atk > 255 or dfn > 255:             # 'or b' / 'or h' high-byte test
        atk //= 4
        dfn //= 4
        if atk == 0:
            atk = 1
    atk &= 0xFF                            # ld b, l
    dfn &= 0xFF                            # c
    if explode:                            # CalculateDamage EXPLODE_EFFECT
        dfn >>= 1
        if dfn == 0:
            dfn = 1
    if dfn == 0:
        return "DIV0-HANG"
    e = level * 2 if is_crit else level    # sla e on crit
    d = (e * 2) // 5 + 2
    d = d * bp * atk
    d = d // dfn
    d = d // 50
    d = min(d, 997) + 2
    if stab:
        d = d + d // 2
    for m in mults:
        d = (d * m) // 10
        if d == 0:
            return None
    if d < 2:
        return {d: 39}
    rolls = {}
    for r in range(217, 256):
        v = (d * r) // 255
        rolls[v] = rolls.get(v, 0) + 1
    return rolls


def game_crit_rate(base_speed, high_crit=False, focus_energy=False):
    b = base_speed >> 1
    if focus_energy:
        b >>= 1
    else:
        b = (b << 1) & 0xFF if (b << 1) < 256 else 255
    if high_crit:
        for _ in range(2):
            if b << 1 >= 256:
                b = 255
                break
            b <<= 1
    else:
        b >>= 1
    return b / 256


def fmt(r):
    if r is None or isinstance(r, str):
        return str(r)
    ks = sorted(r)
    return f"{ks[0]}..{ks[-1]} ({len(ks)} distinct, {sum(r.values())} rolls)"


def app_rolls(dr):
    return None if dr is None else dict(dr.damage_vals)


def species(name):
    return gen.pkmn_db().get_pkmn(name)


def mon(name, level, badges=None):
    m = gen.create_trainer_pkmn(name, level)
    if badges is not None:
        m.badges = badges
    return m


def stats_of(m, stages=None, crit=False):
    return m.get_battle_stats(stages or StageModifiers(), is_crit=crit)


def compare(title, attacker, move_name, defender, *, astages=None, dstages=None, crit=False, screen=False):
    move = gen.move_db().get_move(move_name)
    asp, dsp = species(attacker.name), species(defender.name)
    special = move.move_type in SPECIAL
    astat = stats_of(attacker, astages, crit)
    dstat = stats_of(defender, dstages, crit)
    atk = astat.special_attack if special else astat.attack
    dfn = dstat.special_attack if special else dstat.defense
    stab = move.move_type in (asp.first_type, asp.second_type)
    mults = rom_mults(move.move_type, dsp.first_type, dsp.second_type)
    explode = move.name in (const.EXPLOSION_MOVE_NAME, const.SELFDESTRUCT_MOVE_NAME)
    g = game_damage(attacker.level, move.base_power, atk, dfn, is_crit=crit, screen=screen,
                    explode=explode, stab=stab, mults=mults)
    field = FieldStatus(reflect=screen and not special, light_screen=screen and special)
    a = app_rolls(gen.calculate_damage(attacker, move, defender, attacking_stage_modifiers=astages,
                                       defending_stage_modifiers=dstages, defending_field=field, is_crit=crit))
    same = (g == a)
    print(f"[{'OK ' if same else 'BUG'}] {title}")
    print(f"      L{attacker.level} {attacker.name} {move_name}({move.base_power} {move.move_type}) vs L{defender.level} {defender.name}"
          f" | atk={atk} def={dfn} crit={crit} screen={screen} stab={stab} mults={mults}")
    print(f"      game: {fmt(g)}   app: {fmt(a)}")
    if not same and g and a and not isinstance(g, str):
        print(f"      game rolls: {dict(sorted(g.items()))}")
        print(f"      app  rolls: {dict(sorted(a.items()))}")
    return g, a


if __name__ == "__main__":
    print("=== T1 vanilla sanity: Pinsir L10 Vicegrip vs Geodude L10")
    compare("vanilla", mon("Pinsir", 10), "Vicegrip", mon("Geodude", 10))

    print("\n=== T2 stat>255 scaling: Alakazam L60 +2 Special Psychic vs Dewgong L54")
    compare("scaling +2 spc", mon("Alakazam", 60), "Psychic", mon("Dewgong", 54), astages=StageModifiers(special_attack=2))
    print("=== T2b: Alakazam L60 +2 Special Psychic vs Slowbro L54 (def side >255 too)")
    compare("scaling +2 spc vs slowbro +2", mon("Alakazam", 60), "Psychic", mon("Slowbro", 54),
            astages=StageModifiers(special_attack=2), dstages=StageModifiers(special_defense=2, special_attack=2))
    print("=== T2c: Nidoking L45 +2 Attack Earthquake vs Rhydon L45")
    compare("scaling +2 atk", mon("Nidoking", 45), "Earthquake", mon("Rhydon", 45), astages=StageModifiers(attack=2))

    print("\n=== T3 Reflect doubling pushes def over 255: Nidoking L40 Horn Attack vs Onix L40 +Reflect")
    compare("reflect scaling", mon("Nidoking", 40), "Horn Attack", mon("Onix", 40), screen=True)
    print("=== T4 Reflect low-byte wrap: Nidoking L50 Horn Attack vs Onix L50 +6 Def +Reflect")
    compare("reflect wrap", mon("Nidoking", 50), "Horn Attack", mon("Onix", 50), dstages=StageModifiers(defense=6), screen=True)
    print("=== T4b Light Screen: Alakazam L60 Psychic vs Slowbro L60 +2 Special + Light Screen")
    compare("light screen scaling", mon("Alakazam", 60), "Psychic", mon("Slowbro", 60),
            dstages=StageModifiers(special_defense=2, special_attack=2), screen=True)

    print("\n=== T5 Explosion order vs scaling: Electrode L40 Explosion vs Onix L40 +2 Def")
    compare("explosion no scaling", mon("Electrode", 40), "Explosion", mon("Onix", 40))
    compare("explosion +2 def scaling", mon("Electrode", 40), "Explosion", mon("Onix", 40), dstages=StageModifiers(defense=2))

    print("\n=== T6 crit ignores stages/badges/screens")
    bl = GenOneBadgeList({}, boulder=True, thunder=True, soul=True, volcano=True)
    compare("crit w/ badges+stages+reflect", mon("Nidoking", 40, bl), "Horn Attack", mon("Onix", 40),
            astages=StageModifiers(attack=2), dstages=StageModifiers(defense=2), crit=True, screen=True)
    compare("non-crit w/ badges (boulder)", mon("Nidoking", 40, bl), "Horn Attack", mon("Onix", 40))

    print("\n=== T7 997 cap: L100 crit Explosion Snorlax vs Chansey L5")
    compare("cap", mon("Snorlax", 100), "Explosion", mon("Chansey", 5), crit=True)
    compare("cap non-crit", mon("Snorlax", 100), "Explosion", mon("Chansey", 5))

    print("\n=== T8 Super Fang / Counter / OHKO / Bide app output")
    for mv in ("Super Fang", "Counter", "Guillotine", "Horn Drill", "Fissure", "Bide", "Struggle", "Swift"):
        dr = gen.calculate_damage(mon("Raticate", 30), mv, mon("Geodude", 30)) if False else gen.calculate_damage(mon("Raticate", 30), gen.move_db().get_move(mv), mon("Geodude", 30))
        print(f"   {mv}: {None if dr is None else dict(sorted(dr.damage_vals.items()))}")

    print("\n=== T9 Psywave: L30 player vs enemy")
    dr = gen.calculate_damage(mon("Kadabra", 30), gen.move_db().get_move("Psywave"), mon("Geodude", 30))
    print("   app:", sorted(dr.damage_vals), "n=", len(dr.damage_vals))
    print("   game player: 1..", 30 + 15 - 1, " enemy: 0..", 30 + 15 - 1)

    print("\n=== T10 special-damage moves vs immune types (app)")
    for mv, d in (("Seismic Toss", "Gastly"), ("Night Shade", "Rattata"), ("SonicBoom", "Gastly"), ("Dragon Rage", "Gastly")):
        dr = gen.calculate_damage(mon("Machamp", 30), gen.move_db().get_move(mv), mon(d, 30))
        print(f"   {mv} vs {d}: {None if dr is None else dict(dr.damage_vals)}")

    print("\n=== T11 stat exp 65535 off-by-one")
    print("   game calc_stat(base 100, dv 15, statexp 65535, L100):", game_calc_stat(100, 15, 65535, 100),
          " app:", pkmn_utils.calc_stat(100, 100, 15, 65535))
    print("   game calc_stat(base 100, dv 15, statexp 65025, L100):", game_calc_stat(100, 15, 65025, 100),
          " app:", pkmn_utils.calc_stat(100, 100, 15, 65025))

    print("\n=== T12 crit rate table (game vs app)")
    for bs in (30, 45, 55, 63, 64, 65, 90, 100, 130, 140, 150):
        m = mon("Pidgey", 10); m.base_stats.speed = bs
        app_n = gen.get_crit_rate(m, gen.move_db().get_move("Tackle"), None)
        app_h = gen.get_crit_rate(m, gen.move_db().get_move("Slash"), None)
        print(f"   base spd {bs:3d}: normal game={game_crit_rate(bs):.5f} app={app_n:.5f} | high game={game_crit_rate(bs, True):.5f} app={app_h:.5f}"
              f" | FE normal game={game_crit_rate(bs, focus_energy=True):.5f} FE high game={game_crit_rate(bs, True, True):.5f}")

    print("\n=== T13 badge re-application count check (Alakazam L60, Boulder+Volcano, Growth used once)")
    m = mon("Alakazam", 60, GenOneBadgeList({}, boulder=True, volcano=True))
    st0 = m.get_battle_stats(StageModifiers())
    sm = StageModifiers().apply_stat_mod([(const.SPA, 1), (const.SPD, 1)])
    st1 = m.get_battle_stats(sm)
    print("   before:", st0.attack, st0.defense, st0.speed, st0.special_attack, " after Growth:", st1.attack, st1.defense, st1.speed, st1.special_attack, " stages:", sm)
    unmod_atk = game_calc_stat(50, 9, 0, 60); unmod_spc = game_calc_stat(135, 8, 0, 60)
    a1 = min(999, unmod_atk + unmod_atk // 8); a2 = min(999, a1 + a1 // 8)
    s1 = min(999, (unmod_spc * 15) // 10); s1 = min(999, s1 + s1 // 8)
    print("   game expect: atk", a1, "->", a2, " spc", s1)

    print("\n=== T14 small-damage floors: Pidgey L3 Gust vs Geodude L3 ; Caterpie L3 Tackle vs Onix L12")
    compare("tiny NVE", mon("Pidgey", 3), "Gust", mon("Geodude", 3))
    compare("tiny NVE 2", mon("Caterpie", 3), "Tackle", mon("Onix", 12))
    compare("tiny 4x NVE", mon("Caterpie", 3), "Tackle", mon("Gastly", 3))
