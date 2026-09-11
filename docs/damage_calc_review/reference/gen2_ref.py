"""Reference implementation of the Gen 2 (pokecrystal / pokegold) damage formula,
transcribed from the asm, used only to generate expected numbers for the audit.

pokecrystal/engine/battle/effect_commands.asm:
  BattleCommand_DamageStats/PlayerAttackDamage  2525-2612
  TruncateHL_BC                                 2614-2658
  CheckDamageStatsCritical                      2660-2695
  DittoMetalPowder                              2488-2523
  SpeciesItemBoost                              2731-2766
  BattleCommand_DamageCalc                      2900-3131
  BattleCommand_Stab (+ weather, badge, type)   1214-1409
  BattleCommand_DamageVariation                 1496-1544
  DoubleDamage / DoubleFlying / DoubleUnderground / DoubleMinimize 5962-5986, 6515-6533
pokecrystal/engine/battle/misc.asm DoWeatherModifiers 52-145, DoBadgeTypeBoosts 147-215
"""
import math

# data/types/type_matchups.asm order (attacker, defender, multiplier*10)
TYPE_MATCHUPS = [
 ("Normal","Rock",5),("Normal","Steel",5),
 ("Fire","Fire",5),("Fire","Water",5),("Fire","Grass",20),("Fire","Ice",20),("Fire","Bug",20),("Fire","Rock",5),("Fire","Dragon",5),("Fire","Steel",20),
 ("Water","Fire",20),("Water","Water",5),("Water","Grass",5),("Water","Ground",20),("Water","Rock",20),("Water","Dragon",5),
 ("Electric","Water",20),("Electric","Electric",5),("Electric","Grass",5),("Electric","Ground",0),("Electric","Flying",20),("Electric","Dragon",5),
 ("Grass","Fire",5),("Grass","Water",20),("Grass","Grass",5),("Grass","Poison",5),("Grass","Ground",20),("Grass","Flying",5),("Grass","Bug",5),("Grass","Rock",20),("Grass","Dragon",5),("Grass","Steel",5),
 ("Ice","Water",5),("Ice","Grass",20),("Ice","Ice",5),("Ice","Ground",20),("Ice","Flying",20),("Ice","Dragon",20),("Ice","Steel",5),("Ice","Fire",5),
 ("Fighting","Normal",20),("Fighting","Ice",20),("Fighting","Poison",5),("Fighting","Flying",5),("Fighting","Psychic",5),("Fighting","Bug",5),("Fighting","Rock",20),("Fighting","Dark",20),("Fighting","Steel",20),
 ("Poison","Grass",20),("Poison","Poison",5),("Poison","Ground",5),("Poison","Rock",5),("Poison","Ghost",5),("Poison","Steel",0),
 ("Ground","Fire",20),("Ground","Electric",20),("Ground","Grass",5),("Ground","Poison",20),("Ground","Flying",0),("Ground","Bug",5),("Ground","Rock",20),("Ground","Steel",20),
 ("Flying","Electric",5),("Flying","Grass",20),("Flying","Fighting",20),("Flying","Bug",20),("Flying","Rock",5),("Flying","Steel",5),
 ("Psychic","Fighting",20),("Psychic","Poison",20),("Psychic","Psychic",5),("Psychic","Dark",0),("Psychic","Steel",5),
 ("Bug","Fire",5),("Bug","Grass",20),("Bug","Fighting",5),("Bug","Poison",5),("Bug","Flying",5),("Bug","Psychic",20),("Bug","Ghost",5),("Bug","Dark",20),("Bug","Steel",5),
 ("Rock","Fire",20),("Rock","Ice",20),("Rock","Fighting",5),("Rock","Ground",5),("Rock","Flying",20),("Rock","Bug",20),("Rock","Steel",5),
 ("Ghost","Normal",0),("Ghost","Psychic",20),("Ghost","Dark",5),("Ghost","Steel",5),("Ghost","Ghost",20),
 ("Dragon","Dragon",20),("Dragon","Steel",5),
 ("Dark","Fighting",5),("Dark","Psychic",20),("Dark","Ghost",20),("Dark","Dark",5),("Dark","Steel",5),
 ("Steel","Fire",5),("Steel","Water",5),("Steel","Electric",5),("Steel","Ice",20),("Steel","Rock",20),("Steel","Steel",5),
 # -2 marker: entries below are skipped when the target is Foresight-identified
 ("Normal","Ghost",0),("Fighting","Ghost",0),
]
FORESIGHT_SPLIT = 108  # index of first post-"-2" entry

STAGE_MULT = [(25,100),(28,100),(33,100),(40,100),(50,100),(66,100),(1,1),(15,10),(2,1),(25,10),(3,1),(35,10),(4,1)]

def calc_mon_stat(base, dv, statexp, level, hp=False):
    """engine/pokemon/move_mon.asm CalcMonStatC 1424-1612"""
    b = 0
    # GetSquareRoot: first b with b*b >= statexp, capped at NUM_SQUARE_ROOTS (255)
    while True:
        b += 1
        if b == 255:
            break
        if b * b >= statexp:
            break
    if statexp == 0:
        b = 1  # loop returns b=1 for de=0 (1 >= 0)
    v = (base + dv) * 2 + (b >> 2)
    v = v * level // 100
    v += (level + 10) if hp else 5
    return min(v, 999)

def stage_stat(stat, stage):
    """CalcBattleStats 4817-4886: stat*num/den, min 1, cap 999"""
    n, d = STAGE_MULT[stage + 6]
    v = stat * n // d
    if v == 0:
        v = 1
    return min(v, 999)

def badge_boost(stat):
    """core.asm BoostStat 6826-6855: stat + stat>>3, cap 999"""
    return min(stat + (stat >> 3), 999)

def truncate_hl_bc(atk, dfn, gs=False):
    """TruncateHL_BC 2614-2658 (Crystal loops; Gold/Silver single pass then wraps)"""
    while (atk >> 8) or (dfn >> 8):
        dfn >>= 2
        if dfn == 0:
            dfn = 1
        atk >>= 2
        if atk == 0:
            atk = 1
        if gs:
            break
    return atk & 0xFF, dfn & 0xFF

def ditto_metal_powder(atk8, def8):
    """DittoMetalPowder 2488-2523 (applied AFTER truncation, to Def or SpDef)"""
    c = def8 + (def8 >> 1)
    if c <= 255:
        return atk8, c
    atk8 >>= 1
    if atk8 == 0:
        atk8 = 1
    return atk8, c >> 1

def damage_stats(atk_stat, def_stat, atk_unboosted, def_unboosted, crit, atk_stage, def_stage,
                 screen, species_item_x2=False, metal_powder=False, gs=False):
    """PlayerAttackDamage 2530-2612. Returns (b=atk8, c=def8)."""
    dfn = def_stat
    atk = atk_stat
    if screen:
        dfn = (dfn * 2) & 0xFFFF
    if crit and def_stage >= atk_stage:      # CheckDamageStatsCritical: carry iff defLevel < atkLevel
        dfn = def_unboosted
        atk = atk_unboosted
    if species_item_x2:                      # Thick Club (Cubone/Marowak) / Light Ball (Pikachu)
        atk = (atk * 2) & 0xFFFF
    a8, d8 = truncate_hl_bc(atk, dfn, gs)
    if metal_powder:
        a8, d8 = ditto_metal_powder(a8, d8)
    return a8, d8

def damage_calc(level, bp, atk8, def8, item_boost=False, crit=False, selfdestruct=False):
    """BattleCommand_DamageCalc 2900-3131 -> wCurDamage (before stab command)"""
    c = def8
    if selfdestruct:
        c >>= 1
        if c == 0:
            c = 1
    if bp == 0:
        return 0
    if c == 0:
        c = 1
    x = (2 * level) // 5 + 2
    x = x * bp
    x = x * atk8
    x = x // c
    x = x // 50
    if item_boost:
        x = x * 110 // 100
    if crit:
        x = min(x * 2, 0xFFFF)
    x = min(x, 997)
    return x + 2

def stab_command(dmg, move_type, atk_types, def_types, weather=None, is_solarbeam=False,
                 badge_boost_type=False, is_struggle=False, foresight=False):
    """BattleCommand_Stab 1214-1409 incl. DoWeatherModifiers / DoBadgeTypeBoosts.
    Returns (damage, immune)."""
    if is_struggle:
        return dmg, False
    # DoWeatherModifiers (misc.asm 52-145): x15/10 or x5/10, min 1, cap ffff
    mult = None
    if weather == "Rain" and move_type == "Water": mult = 15
    if weather == "Rain" and move_type == "Fire": mult = 5
    if weather == "Sun" and move_type == "Fire": mult = 15
    if weather == "Sun" and move_type == "Water": mult = 5
    if mult is None and weather == "Rain" and is_solarbeam: mult = 5
    if mult is not None:
        q = dmg * mult // 10
        if q > 0xFFFF: q = 0xFFFF
        if q == 0: q = 1
        dmg = q
    # DoBadgeTypeBoosts (misc.asm 147-215): + max(dmg>>3, 1), cap ffff
    if badge_boost_type:
        add = dmg >> 3
        if add == 0:
            add = 1
        dmg = min(dmg + add, 0xFFFF)
    # STAB: + dmg>>1
    if move_type in atk_types:
        dmg = (dmg + (dmg >> 1)) & 0xFFFF
    # type matchups in table order
    immune = False
    for i, (a, d, m) in enumerate(TYPE_MATCHUPS):
        if i >= FORESIGHT_SPLIT and foresight:
            break
        if a == move_type and d in def_types:
            if m == 0:
                dmg = 0
                immune = True
            else:
                q = dmg * m // 10
                if q == 0:
                    q = 1
                dmg = q
    return dmg, immune

def variation(dmg):
    """BattleCommand_DamageVariation 1496-1544: r in 217..255, dmg*r/255; skipped if dmg<2"""
    if dmg < 2:
        return {dmg: 39}
    out = {}
    for r in range(217, 256):
        v = dmg * r // 255
        out[v] = out.get(v, 0) + 1
    return out

def double_after(rolls):
    return {min(v * 2, 0xFFFF): c for v, c in rolls.items()}

def summarize(rolls):
    ks = sorted(rolls)
    return f"{ks[0]}..{ks[-1]} ({len(ks)} distinct, {sum(rolls.values())} rolls)"

def hidden_power(atk_dv, def_dv, spd_dv, spc_dv):
    """engine/battle/hidden_power.asm"""
    x = ((atk_dv >> 3) & 1) << 3 | ((def_dv >> 3) & 1) << 2 | ((spd_dv >> 3) & 1) << 1 | ((spc_dv >> 3) & 1)
    power = (5 * x + (spc_dv & 3)) // 2 + 31
    types = ["Fighting","Flying","Poison","Ground","Rock","Bug","Ghost","Steel","Fire","Water","Grass","Electric","Psychic","Ice","Dragon","Dark"]
    t = types[((atk_dv & 3) << 2) | (def_dv & 3)]
    return t, power

def crit_chance_byte(stage):
    table = [256//15, 256//8, 256//4, 256//3, 256//2, 256//2, 256//2]
    return table[min(stage, 6)]

def accuracy_byte(pct):
    return pct * 255 // 100
