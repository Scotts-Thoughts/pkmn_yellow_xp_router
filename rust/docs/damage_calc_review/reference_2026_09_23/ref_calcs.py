"""Reference damage calculators transcribed from the decompilations.

Each `genN(case, crit)` takes one result record of the Rust sweep harness
(which carries the app's resolved battle stats, species types and move data)
and returns the game's damage distribution as {damage: count} (or None for
"no damage"), or the string 'SKIP' for inputs the reference does not model.

Sources:
  gen 1: pokeyellow engine/battle/core.asm (GetDamageVarsForPlayerAttack,
         CalculateDamage, AdjustDamageForMoveType, RandomizeDamage,
         ApplyAttackToEnemyPokemon, CriticalHitTest)
  gen 2: pokecrystal engine/battle/effect_commands.asm (DamageStats,
         TruncateHL_BC, DittoMetalPowder, DamageCalc, Stab, DamageVariation,
         ConstantDamage), misc.asm (weather, badge type boosts), move_effects/*
  gen 3: pokeemerald src/pokemon.c CalculateBaseDamage, battle_script_commands.c
         (Cmd_damagecalc, Cmd_typecalc, ApplyRandomDmgMultiplier, ...)
  gen 4: pokeplatinum src/battle/battle_lib.c BattleSystem_CalcMoveDamage,
         BattleSystem_ApplyTypeChart, battle_script.c BattleScript_CalcMoveDamage
"""
import json
import os
import re

ROOT = '/Users/scottross-molyneux/Documents/pkmn_yellow_xp_router'
DEC = '/private/tmp/claude-501/-Users-scottross-molyneux-Documents-pkmn-yellow-xp-router/b870b3d0-4488-4e83-9349-115d7cd1fae3/scratchpad/decomps'

ASM_TYPES = {
    'NORMAL': 'Normal', 'FIGHTING': 'Fighting', 'FLYING': 'Flying', 'POISON': 'Poison', 'GROUND': 'Ground',
    'ROCK': 'Rock', 'BUG': 'Bug', 'GHOST': 'Ghost', 'STEEL': 'Steel', 'FIRE': 'Fire', 'WATER': 'Water',
    'GRASS': 'Grass', 'ELECTRIC': 'Electric', 'PSYCHIC_TYPE': 'Psychic', 'PSYCHIC': 'Psychic', 'ICE': 'Ice',
    'DRAGON': 'Dragon', 'DARK': 'Dark',
}
MULT = {'SUPER_EFFECTIVE': 20, 'NOT_VERY_EFFECTIVE': 5, 'NO_EFFECT': 0,
        'TYPE_MUL_SUPER_EFFECTIVE': 20, 'TYPE_MUL_NOT_EFFECTIVE': 5, 'TYPE_MUL_NO_EFFECT': 0,
        'TYPE_MULTI_SUPER_EFF': 20, 'TYPE_MULTI_NOT_VERY_EFF': 5, 'TYPE_MULTI_IMMUNE': 0}

SPECIAL_G1 = {'Fire', 'Water', 'Grass', 'Electric', 'Psychic', 'Ice', 'Dragon'}
SPECIAL_G23 = SPECIAL_G1 | {'Dark'}

# type ids (gen 2 constants/type_constants.asm) for the G/S Present clobber
G2_TYPE_ID = {'Normal': 0, 'Fighting': 1, 'Flying': 2, 'Poison': 3, 'Ground': 4, 'Rock': 5, 'Bug': 7, 'Ghost': 8,
              'Steel': 9, 'Fire': 20, 'Water': 21, 'Grass': 22, 'Electric': 23, 'Psychic': 24, 'Ice': 25,
              'Dragon': 26, 'Dark': 27}

STAGE_G345 = [(10, 40), (10, 35), (10, 30), (10, 25), (10, 20), (10, 15), (10, 10), (15, 10), (20, 10), (25, 10), (30, 10), (35, 10), (40, 10)]


def _parse_asm_table(path, stop_markers):
    rows = []
    ghost_rows = []
    after_marker = False
    for line in open(path):
        line = line.strip()
        m = re.match(r'db\s+([A-Z_]+),\s*([A-Z_]+),\s*([A-Z_]+)', line)
        if m:
            row = (ASM_TYPES[m.group(1)], ASM_TYPES[m.group(2)], MULT[m.group(3)])
            (ghost_rows if after_marker else rows).append(row)
        elif re.match(r'db\s+-2', line):
            after_marker = True
    return rows, ghost_rows


def gen1_table():
    rows, _ = _parse_asm_table(os.path.join(DEC, 'pokeyellow/data/types/type_matchups.asm'), None)
    return rows


def gen2_table():
    rows, ghost = _parse_asm_table(os.path.join(DEC, 'pokecrystal/data/types/type_matchups.asm'), None)
    return rows + ghost  # Foresight not modelled: the Ghost immunities apply


def _parse_c_table(path, start_pat, end_pat, item_pat):
    text = open(path).read()
    start = text.index(start_pat)
    end = text.index(end_pat, start)
    body = text[start:end]
    rows = []
    ghost = []
    after = False
    for m in re.finditer(item_pat, body):
        a, d, mul = m.group(1), m.group(2), m.group(3)
        if a in ('TYPE_FORESIGHT',) or a == '0xFE':
            after = True
            continue
        if a in ('TYPE_ENDTABLE', '0xFF'):
            break
        row = (ASM_TYPES[a.replace('TYPE_', '')], ASM_TYPES[d.replace('TYPE_', '')], MULT[mul])
        (ghost if after else rows).append(row)
    return rows + ghost


def gen3_table():
    return _parse_c_table(os.path.join(DEC, 'pokeemerald/src/battle_main.c'), 'const u8 gTypeEffectiveness[336] =', '};',
                          r'(TYPE_[A-Z_]+|0xFE|0xFF),\s*(TYPE_[A-Z_]+|0xFE|0xFF),\s*(TYPE_MUL_[A-Z_]+)')


def gen4_table():
    return _parse_c_table(os.path.join(DEC, 'pokeplatinum/src/battle/battle_lib.c'), 'static const u8 sTypeMatchupMultipliers[][3] = {', '};',
                          r'\{\s*(TYPE_[A-Z_]+|0xFE|0xFF),\s*(TYPE_[A-Z_]+|0xFE|0xFF),\s*(TYPE_MULTI_[A-Z_]+)\s*\}')


TABLES = {}


def table(gen):
    if gen not in TABLES:
        TABLES[gen] = {1: gen1_table, 2: gen2_table, 3: gen3_table, 4: gen4_table}[gen]()
    return TABLES[gen]


def type_info(gen):
    name = {1: 'gen_one', 2: 'gen_two', 3: 'gen_three', 4: 'gen_four'}[gen]
    return json.load(open(os.path.join(ROOT, 'raw_pkmn_data', name, 'type_info.json')))


HELD = {}


def held_boost_type(gen, item):
    if gen not in HELD:
        HELD[gen] = type_info(gen).get('held_item_boosts', {})
    return HELD[gen].get(item or '')


def rolls_gen12(x, mult_after=1, cap=65535):
    """DamageVariation / RandomizeDamage: skipped below 2; r in 217..255, x*r/255."""
    out = {}
    if x < 2:
        v = min(x * mult_after, cap) if mult_after != 1 else x
        return {v: 39}
    for r in range(217, 256):
        v = x * r // 255
        if mult_after != 1:
            v = min(v * mult_after, cap)
        out[v] = out.get(v, 0) + 1
    return out


def rolls_gen34(x):
    out = {}
    for r in range(0, 16):
        v = x * (100 - r) // 100
        if v == 0:
            v = 1
        out[v] = out.get(v, 0) + 1
    return out


def convolve(a, b):
    out = {}
    for da, ca in a.items():
        for db, cb in b.items():
            out[da + db] = out.get(da + db, 0) + ca * cb
    return out


def scale(dist, n):
    return {d * n: c for d, c in dist.items()}


def multi_hits(case):
    custom = case['custom']
    fl = case['move']['flavors']
    if 'two_hit' in fl:
        return 2
    if 'multi_hit' in fl:
        for n in (5, 4, 3, 2):
            if f'{n} Hits' in custom:
                return n
        return 1
    return 1


def mult_types(row_type, d1, d2):
    return row_type == d1 or row_type == d2


# ---------------------------------------------------------------------------
# gen 1
# ---------------------------------------------------------------------------

def gen1(case, crit):
    a, d, mv = case['attacker'], case['defender'], case['move']
    name, custom, fl = mv['name'], case['custom'], mv['flavors']
    level = a['level']
    mtype = mv['type']
    d1, d2 = d['types']
    a1, a2 = a['types']

    if name in ('Seismic Toss', 'Night Shade'):
        return {level: 1}
    if name == 'SonicBoom':
        return {20: 1}
    if name == 'Dragon Rage':
        return {40: 1}
    if name == 'Psywave':
        b = level + level // 2
        lo = 0 if case['attacker_is_enemy'] else 1
        return {x: 1 for x in range(lo, b)} or None
    if 'super_fang' in fl:
        pct = {'Full HP': 100, '75% HP': 75, '50% HP': 50, '25% HP': 25, '10% HP': 10}.get(custom, 100)
        hp = d['cur_hp'] * pct // 100
        return {max(hp >> 1, 1): 1}
    if 'counter' in fl or 'bide' in fl:
        if custom.strip().isdigit() and int(custom) > 0:
            return {min(2 * int(custom), 65535): 1}
        return None

    # immunity (AdjustDamageForMoveType with a 0 multiplier)
    immune = any(r[2] == 0 and r[0] == mtype and mult_types(r[1], d1, d2) for r in table(1))

    if 'one_hit_ko' in fl:
        if immune:
            return None
        # in-battle speeds (stages/badges); a crit never changes the OHKO speed check
        if a['staged']['spe'] < d['staged']['spe']:
            return None
        return {d['cur_hp']: 1}

    bp = mv['power'] or 0
    if bp == 0:
        return None
    special = mtype in SPECIAL_G1
    if crit:
        A = a['crit']['spa'] if special else a['crit']['atk']
        D = d['crit']['spd'] if special else d['crit']['def']
    else:
        A = a['staged']['spa'] if special else a['staged']['atk']
        D = d['staged']['spd'] if special else d['staged']['def']
        df = case['defending_field'] or {}
        if (df.get('light_screen') if special else df.get('reflect')):
            D *= 2
    if A > 255 or D > 255:
        A //= 4
        D //= 4
        if A == 0:
            A = 1
    A &= 0xFF
    D &= 0xFF
    if name in ('Explosion', 'Selfdestruct'):
        D >>= 1
        if D == 0:
            D = 1
    if D == 0:
        return None  # the game freezes
    E = level * 2 if crit else level
    q = (2 * E) // 5 + 2
    q = q * bp * A // D // 50
    q = min(q, 997) + 2
    if mtype in (a1, a2):
        q += q >> 1
    for (ra, rd, m) in table(1):
        if ra == mtype and mult_types(rd, d1, d2):
            q = q * m // 10
            if q == 0:
                return None
    dist = rolls_gen12(q)
    n = multi_hits(case)
    if 'partial_trapping' in fl:
        for k in (2, 3, 4, 5):
            if custom == f'{k} Turns':
                n = k
    if n > 1:
        dist = scale(dist, n)
    return dist


# ---------------------------------------------------------------------------
# gen 2
# ---------------------------------------------------------------------------

def hidden_power_gen2(dvs):
    atk, dfn, spd, spc = dvs['atk'], dvs['def'], dvs['spe'], dvs['spa']
    top = ((atk >> 3) & 1) << 3 | ((dfn >> 3) & 1) << 2 | ((spd >> 3) & 1) << 1 | ((spc >> 3) & 1)
    power = (5 * top + (spc & 3)) // 2 + 31
    idx = ((atk & 3) << 2) | (dfn & 3)
    types = ['Fighting', 'Flying', 'Poison', 'Ground', 'Rock', 'Bug', 'Ghost', 'Steel', 'Fire', 'Water', 'Grass',
             'Electric', 'Psychic', 'Ice', 'Dragon', 'Dark']
    return types[idx], power


BADGE_TYPE_G2 = {'zephyr': 'Flying', 'hive': 'Bug', 'plain': 'Normal', 'fog': 'Ghost', 'mineral': 'Steel',
                 'storm': 'Fighting', 'glacier': 'Ice', 'rising': 'Dragon', 'boulder': 'Rock', 'cascade': 'Water',
                 'thunder': 'Electric', 'rainbow': 'Grass', 'soul': 'Poison', 'marsh': 'Psychic', 'volcano': 'Fire',
                 'earth': 'Ground'}


def gen2_stab_and_type(x, mtype, name, a_types, d_types, weather, badge_slots, apply_type=True):
    """BattleCommand_Stab: weather, badge type boost, STAB, type loop (table order)."""
    if name == 'Struggle':
        return x, False
    # weather (DoWeatherModifiers): first matching entry only
    m = None
    if weather == 'Rain':
        if mtype == 'Water':
            m = 15
        elif mtype == 'Fire':
            m = 5
        elif name == 'SolarBeam':
            m = 5
    elif weather == 'Harsh Sunlight':
        if mtype == 'Fire':
            m = 15
        elif mtype == 'Water':
            m = 5
    if m is not None:
        x = x * m // 10
        if x == 0:
            x = 1
        x = min(x, 65535)
    # badge type boost (player only): + max(x >> 3, 1)
    if any(BADGE_TYPE_G2.get(s) == mtype for s in badge_slots):
        x = min(x + max(x >> 3, 1), 65535)
    if mtype in a_types:
        x += x >> 1
    immune = False
    if apply_type:
        d1, d2 = d_types
        for (ra, rd, mul) in table(2):
            if ra == mtype and mult_types(rd, d1, d2):
                if mul == 0:
                    immune = True
                    x = 0
                    continue
                x = x * mul // 10
                if x == 0:
                    x = 1
    return x, immune


def gen2_damagecalc(x_level, bp, A, D, item_type_boost, crit, explosion):
    if explosion:
        D >>= 1
        if D == 0:
            D = 1
    if bp == 0:
        return None
    if D == 0:
        D = 1
    q = (2 * x_level) // 5 + 2
    q = q * bp * A // D // 50
    if item_type_boost:
        q = q * 110 // 100
    if crit:
        q = min(q * 2, 65535)
    q = min(q, 997) + 2
    return q


def gen2_stats(case, crit, special, use_screens=True):
    """DamageStats: returns (A, D) 8-bit after screens, crit reload, species item, truncation, Metal Powder."""
    a, d = case['attacker'], case['defender']
    df = case['defending_field'] or {}
    ast = case['attacking_stages'] or {}
    dst = case['defending_stages'] or {}
    if special:
        A = a['staged']['spa']
        D = d['staged']['spd']
        if use_screens and df.get('light_screen'):
            D *= 2
        atk_stage = ast.get('spa', 0)
        def_stage = dst.get('spd', 0)
    else:
        A = a['staged']['atk']
        D = d['staged']['def']
        if use_screens and df.get('reflect'):
            D *= 2
        atk_stage = ast.get('atk', 0)
        def_stage = dst.get('def', 0)
    if crit and def_stage >= atk_stage:
        # reload raw party stats (no stages, no badges); screen doubling discarded
        if special:
            A, D = a['crit']['spa'], d['crit']['spd']
        else:
            A, D = a['crit']['atk'], d['crit']['def']
    # Thick Club / Light Ball
    if a['name'] in ('Cubone', 'Marowak') and a['held_item'] == 'Thick Club' and not special:
        A *= 2
    if a['name'] == 'Pikachu' and a['held_item'] == 'Light Ball' and special:
        A *= 2
    gs = case['version'] in ('Gold', 'Silver')
    while A >= 256 or D >= 256:
        D = max(D >> 2, 1)
        A = max(A >> 2, 1)
        if gs:
            A &= 0xFF
            D &= 0xFF
            break
    if d['name'] == 'Ditto' and d['held_item'] == 'Metal Powder':
        c = D + (D >> 1)
        if c > 255:
            A = max(A >> 1, 1)
            D = c >> 1
        else:
            D = c
    return A, D


def gen2(case, crit, _single=False):
    a, d, mv = case['attacker'], case['defender'], case['move']
    name, custom, fl = mv['name'], case['custom'], mv['flavors']
    level = a['level']
    mtype = mv['type']
    d1, d2 = d['types']
    a_types = a['types']
    weather = case['weather']
    slots = (a['badges'] or {}).get('slots', [])

    def immune_to(t):
        return any(r[2] == 0 and r[0] == t and mult_types(r[1], d1, d2) for r in table(2))

    if name in ('Seismic Toss', 'Night Shade'):
        return None if immune_to(mtype) else {level: 1}
    if name == 'SonicBoom':
        return None if immune_to(mtype) else {20: 1}
    if name == 'Dragon Rage':
        return None if immune_to(mtype) else {40: 1}
    if name == 'Psywave':
        if immune_to(mtype):
            return None
        b = level + level // 2
        return {x: 1 for x in range(1, b)} or None
    if 'super_fang' in fl:
        if immune_to(mtype):
            return None
        return {max(d['cur_hp'] >> 1, 1): 1}
    if 'one_hit_ko' in fl:
        if immune_to(mtype) or level < d['level']:
            return None
        return {d['cur_hp']: 1}
    if name in ('Counter', 'Mirror Coat', 'Bide'):
        if immune_to(mtype):
            return None
        if custom.isdigit() and int(custom) > 0:
            return {min(2 * int(custom), 65535): 1}
        return None
    if name == 'Beat Up':
        n = int(custom) if custom.isdigit() else 1
        item_boost = held_boost_type(2, a['held_item']) == 'Dark'
        def hit(c):
            q = gen2_damagecalc(level, 10, a['base']['atk'], max(d['base']['def'], 1), item_boost, c, False)
            return rolls_gen12(q)
        total = hit(crit)
        for _ in range(n - 1):
            total = convolve(total, hit(False))
        return total
    if name == 'Hidden Power':
        mtype, bp = hidden_power_gen2(a['dvs'])
    else:
        bp = mv['power'] or 0
    if name == 'Present':
        if custom == 'Heal':
            return None
        bp = int(custom) if custom.isdigit() else 40
    elif name == 'Return' or name == 'Frustration':
        bp = int(custom) if custom.isdigit() else 0
    elif name == 'Magnitude':
        bp = {'Mag 4': 10, 'Mag 5': 30, 'Mag 6': 50, 'Mag 7': 70, 'Mag 8': 90, 'Mag 9': 110, 'Mag 10': 150}[[k for k in ('Mag 10', 'Mag 4', 'Mag 5', 'Mag 6', 'Mag 7', 'Mag 8', 'Mag 9') if k in custom][0]]
    elif name in ('Flail', 'Reversal'):
        bp = {'100-69 % HP': 20, '69-35 % HP': 40, '35-20 % HP': 80, '20-10 % HP': 100, '10-4 % HP': 150, '4-0 % HP': 200}[custom]
    if bp == 0:
        return None
    struggle = name == 'Struggle'
    special = (mtype in SPECIAL_G23) and not struggle
    item_boost_type = 'Normal' if struggle else mtype
    item_type_boost = held_boost_type(2, a['held_item']) == item_boost_type
    explosion = name in ('Explosion', 'Selfdestruct')
    future = name == 'Future Sight'
    flail = name in ('Flail', 'Reversal')
    if flail or future:
        crit = False

    if name == 'Present' and case['version'] in ('Gold', 'Silver'):
        # register clobber: level = target type-2 id, A = wTypeMatchup, D = user type-2 id (1 if STAB), bp = present power
        tm = 10
        for (ra, rd, mul) in table(2):
            if ra == 'Normal' and mult_types(rd, d1, d2):
                tm = tm * mul // 10
        if tm == 0:
            return None
        stab = 'Normal' in a_types
        D = 1 if stab else G2_TYPE_ID[a_types[1]]
        if D == 0:
            D = 1
        q = gen2_damagecalc(G2_TYPE_ID[d2], bp, tm, D, item_type_boost, crit, False)
    else:
        A, D = gen2_stats(case, crit, special)
        q = gen2_damagecalc(level, bp, A, D, item_type_boost, crit, explosion)
    if q is None:
        return None

    if name == 'Triple Kick':
        kicks = {'1 Kick': 1, '2 Kicks': 2, '3 Kicks': 3}.get(custom, 3)
        total = None
        q_normal = gen2_damagecalc(level, bp, A, D, item_type_boost, False, explosion)
        for k in range(1, kicks + 1):
            x = min((q if k == kicks else q_normal) * k, 65535)
            x, immune = gen2_stab_and_type(x, mtype, name, a_types, (d1, d2), weather, slots)
            if immune:
                return None
            dist = rolls_gen12(x)
            total = dist if total is None else convolve(total, dist)
        return total

    if future:
        x = q  # no stab command at all
    else:
        x, immune = gen2_stab_and_type(q, mtype, name, a_types, (d1, d2), weather, slots)
        if immune:
            return None
    if flail:
        return {x: 1}
    # post-stab multipliers (Rollout / Fury Cutter / Rage), before variation
    if name == 'Rollout':
        n = 6 if custom == '5 + DefenseCurl' else int(custom)
        for _ in range(n - 1):
            x = min(x * 2, 65535)
    elif name == 'Fury Cutter':
        n = min(int(custom), 5)
        for _ in range(n - 1):
            x = min(x * 2, 65535)
    elif name == 'Rage':
        x = min(x * int(custom), 65535)
    double_after = 1
    if name in ('Gust', 'Twister', 'Earthquake', 'Stomp', 'Pursuit'):
        double_after = 1 if 'No Bonus' in custom else 2
    elif name == 'Magnitude':
        double_after = 2 if 'Dig Bonus' in custom else 1
    dist = rolls_gen12(x, double_after)
    n = 1 if _single else multi_hits(case)
    if n > 1:
        # per-hit crit and roll in gen 2; the app models "exactly one crit": build the same shape
        if crit:
            other = gen2(case, False, _single=True)
            for _ in range(n - 1):
                dist = convolve(dist, other)
        else:
            one = dist
            for _ in range(n - 1):
                dist = convolve(dist, one)
    if 'false_swipe' in fl:
        cap = max(d['cur_hp'] - 1, 1)
        out = {}
        for k, v in dist.items():
            out[min(k, cap)] = out.get(min(k, cap), 0) + v
        dist = out
    return dist


# ---------------------------------------------------------------------------
# gen 3
# ---------------------------------------------------------------------------

def hidden_power_gen3(ivs):
    hp, atk, dfn, spe, spa, spd = ivs['hp'], ivs['atk'], ivs['def'], ivs['spe'], ivs['spa'], ivs['spd']
    power_bits = ((hp & 2) >> 1) | (atk & 2) | ((dfn & 2) << 1) | ((spe & 2) << 2) | ((spa & 2) << 3) | ((spd & 2) << 4)
    type_bits = (hp & 1) | ((atk & 1) << 1) | ((dfn & 1) << 2) | ((spe & 1) << 3) | ((spa & 1) << 4) | ((spd & 1) << 5)
    power = (40 * power_bits) // 63 + 30
    t = (15 * type_bits) // 63 + 1
    order = ['Normal', 'Fighting', 'Flying', 'Poison', 'Ground', 'Rock', 'Bug', 'Ghost', 'Steel', 'Mystery', 'Fire',
             'Water', 'Grass', 'Electric', 'Psychic', 'Ice', 'Dragon', 'Dark']
    if t >= 9:
        t += 1
    return order[t], power


def stage(stat, s):
    num, den = STAGE_G345[s + 6]
    return stat * num // den


def forecast(types, ability, weather, active):
    if not active or ability != 'Forecast':
        return types
    t = {'Harsh Sunlight': 'Fire', 'Rain': 'Water', 'Hail': 'Ice'}.get(weather)
    return [t, t] if t else types


def weather_active(case):
    w = case['weather']
    if w == 'None':
        return False
    return case['attacker']['ability'] not in ('Air Lock', 'Cloud Nine') and case['defender']['ability'] not in ('Air Lock', 'Cloud Nine')


NATURE_POWER_G3 = {'Plain': ('Swift', 60, 'Normal', 'target_both_enemies'), 'Sand': ('Earthquake', 100, 'Ground', 'target_all'),
                   'Cave': ('Shadow Ball', 80, 'Ghost', 'target_single_enemy'), 'Rock': ('Rock Slide', 75, 'Rock', 'target_both_enemies'),
                   'Tall Grass': None, 'Long Grass': ('Razor Leaf', 55, 'Grass', 'target_both_enemies'),
                   'Pond Water': ('BubbleBeam', 65, 'Water', 'target_single_enemy'), 'Sea Water': ('Surf', 95, 'Water', 'target_both_enemies'),
                   'Underwater': ('Hydro Pump', 120, 'Water', 'target_single_enemy')}


def gen3(case, crit, _single=False):
    a, d, mv = case['attacker'], case['defender'], case['move']
    name, custom, fl = mv['name'], case['custom'], mv['flavors']
    level = a['level']
    active = weather_active(case)
    weather = case['weather']
    a_types = forecast(a['types'], a['ability'], weather, active)
    d_types = forecast(d['types'], d['ability'], weather, active)
    d1, d2 = d_types
    ast = case['attacking_stages'] or {}
    dst = case['defending_stages'] or {}
    df = case['defending_field'] or {}
    doubles = case['doubles']
    a_item, d_item = a['held_item'] or '', d['held_item'] or ''
    a_ab, d_ab = a['ability'], d['ability']

    mtype = mv['type']
    bp = mv['power'] or 0
    targeting = mv['targeting']
    if name == 'Hidden Power':
        mtype, bp = hidden_power_gen3(a['dvs'])
    if name == 'Nature Power':
        np = NATURE_POWER_G3.get(custom)
        if np is None:
            return None
        name, bp, mtype, targeting = np
    if name == 'Weather Ball' and active:
        mtype = {'Rain': 'Water', 'Sandstorm': 'Rock', 'Harsh Sunlight': 'Fire', 'Hail': 'Ice'}.get(weather, 'Normal')

    struggle = name == 'Struggle'
    no_typecalc = struggle or name in ('Future Sight', 'Doom Desire')

    def effectiveness_flags():
        se = nve = imm = False
        for (ra, rd, mul) in table(3):
            if ra != mtype:
                continue
            for t in ([d1] + ([d2] if d2 != d1 else [])):
                if rd == t:
                    if mul == 0:
                        imm = True
                        se = nve = False
                    elif mul == 5:
                        if se:
                            se = False
                        else:
                            nve = True
                    elif mul == 20:
                        if nve:
                            nve = False
                        else:
                            se = True
        return se, nve, imm

    # ability immunities (attackcanceler / typecalc)
    if not no_typecalc and name != 'Beat Up':
        se, nve, imm = effectiveness_flags()
        if imm:
            return None
        if d_ab == 'Levitate' and mtype == 'Ground':
            return None
        if d_ab == 'Volt Absorb' and mtype == 'Electric' and bp:
            return None
        if d_ab == 'Water Absorb' and mtype == 'Water' and bp:
            return None
        if d_ab == 'Flash Fire' and mtype == 'Fire':
            return None
        if d_ab == 'Damp' and name in ('Explosion', 'Selfdestruct'):
            return None
        if d_ab == 'Soundproof' and name in ('Snore', 'Uproar', 'Hyper Voice'):
            return None
        if d_ab == 'Wonder Guard' and bp and (not se):
            return None

    if name in ('Seismic Toss', 'Night Shade'):
        return {level: 1}
    if name == 'SonicBoom':
        return {20: 1}
    if name == 'Dragon Rage':
        return {40: 1}
    if name == 'Psywave':
        out = {}
        for r in range(0, 11):
            v = level * (r * 10 + 50) // 100
            out[v] = out.get(v, 0) + 1
        return out
    if name == 'Super Fang':
        if d_ab == 'Wonder Guard' and wg_blocks():
            return None
        return {max(d['cur_hp'] // 2, 1): 1}
    if name in ('Guillotine', 'Horn Drill', 'Fissure', 'Sheer Cold'):
        if d_ab == 'Sturdy' or level < d['level']:
            return None
        return {d['cur_hp']: 1}
    if name == 'Endeavor':
        pct = int(custom) if custom.isdigit() else 100
        user_hp = a['cur_hp'] * pct // 100
        if d['cur_hp'] <= user_hp:
            return None
        return {d['cur_hp'] - user_hp: 1}
    if name in ('Counter', 'Mirror Coat', 'Bide'):
        if custom.isdigit() and int(custom) > 0:
            return {2 * int(custom): 1}
        return None
    if name == 'Beat Up':
        n = int(custom) if custom.isdigit() else 1
        base = a['base']['atk'] * 10 * (level * 2 // 5 + 2) // d['base']['def'] // 50 + 2
        hit = rolls_gen34(base * (2 if crit else 1))
        one = rolls_gen34(base)
        total = hit
        for _ in range(n - 1):
            total = convolve(total, one)
        return total
    if name == 'Triple Kick':
        kicks = int(custom) if custom.isdigit() else 1
        total = None
        for k in range(1, kicks + 1):
            sub = dict(case)
            sub['move'] = dict(mv, name='__tk__', power=10 * k)
            sub['custom'] = ''
            dist = gen3(sub, crit and k == kicks)
            if dist is None:
                continue
            total = dist if total is None else convolve(total, dist)
        return total
    if name == '__tk__':
        name = 'Triple Kick'

    # variable power
    if name == 'Magnitude':
        bp = {'Mag 4': 10, 'Mag 5': 30, 'Mag 6': 50, 'Mag 7': 70, 'Mag 8': 90, 'Mag 9': 110, 'Mag 10': 150}[[k for k in ('Mag 10', 'Mag 4', 'Mag 5', 'Mag 6', 'Mag 7', 'Mag 8', 'Mag 9') if k in custom][0]]
    elif name in ('Flail', 'Reversal'):
        bp = {'100-69 % HP': 20, '69-35 % HP': 40, '35-20 % HP': 80, '20-10 % HP': 100, '10-4 % HP': 150, '4-0 % HP': 200}[custom]
    elif name in ('Return', 'Frustration'):
        bp = int(custom) if custom.isdigit() else bp
    elif name in ('Eruption', 'Water Spout'):
        pct = int(custom) if custom.isdigit() else 100
        bp = bp * pct // 100
        if bp == 0:
            bp = 1
    elif name == 'Present':
        bp = {'40 BP': 40, '80 BP': 80, '120 BP': 120}.get(custom, 40)
    elif name in ('Rollout', 'Ice Ball'):
        if custom == '5 + DefenseCurl':
            bp = bp * 16 * 2
        else:
            bp = bp * (2 ** (int(custom) - 1))
    elif name == 'Fury Cutter':
        bp = bp * (2 ** (min(int(custom), 5) - 1))
    elif name == 'Low Kick':
        w = d['weight']
        if w is None:
            return 'SKIP'
        hg = int(round(w * 10))
        bp = 20 if hg < 100 else 40 if hg < 250 else 60 if hg < 500 else 80 if hg < 1000 else 100 if hg < 2000 else 120
    elif name == 'Spit Up':
        n = int(custom) if custom.isdigit() else 1
    if bp == 0:
        return None

    special = mtype in SPECIAL_G23 and not struggle
    # raw stats (no badges, no stages), then modifiers in CalculateBaseDamage order
    atk, dfn = a['raw']['atk'], d['raw']['def']
    spa, spd = a['raw']['spa'], d['raw']['spd']
    ab = a['badges'] or {}
    dbg = d['badges'] or {}
    if case.get('wild') and case['version'] in ('Ruby', 'Sapphire'):
        # BADGE_BOOST (pokeruby calculate_base_damage.c:78-89): trainer battles only
        ab = {}
        dbg = {}
    if a_ab in ('Huge Power', 'Pure Power'):
        atk *= 2
    if ab.get('atk'):
        atk = 110 * atk // 100
    if dbg.get('def'):
        dfn = 110 * dfn // 100
    if ab.get('spa'):
        spa = 110 * spa // 100
    if dbg.get('spd'):
        spd = 110 * spd // 100
    boost_t = held_boost_type(3, a_item)
    if a_item == 'Sea Incense':
        boost_t = 'Water'
    if boost_t == mtype:
        param = 5 if a_item == 'Sea Incense' else 10
        if special:
            spa = spa * (param + 100) // 100
        else:
            atk = atk * (param + 100) // 100
    if a_item == 'Choice Band':
        atk = 150 * atk // 100
    if a_item == 'Soul Dew' and a['name'] in ('Latias', 'Latios'):
        spa = 150 * spa // 100
    if d_item == 'Soul Dew' and d['name'] in ('Latias', 'Latios'):
        spd = 150 * spd // 100
    if a_item == 'DeepSeaTooth' and a['name'] == 'Clamperl':
        spa *= 2
    if d_item == 'DeepSeaScale' and d['name'] == 'Clamperl':
        spd *= 2
    if a_item == 'Light Ball' and a['name'] == 'Pikachu':
        spa *= 2
    if d_item == 'Metal Powder' and d['name'] == 'Ditto':
        dfn *= 2
    if a_item == 'Thick Club' and a['name'] in ('Cubone', 'Marowak'):
        atk *= 2
    if d_ab == 'Thick Fat' and mtype in ('Fire', 'Ice'):
        spa //= 2
    if a_ab == 'Hustle':
        atk = 150 * atk // 100
    if name in ('Explosion', 'Selfdestruct'):
        dfn //= 2
    real_crit = crit and name not in ('Spit Up', 'Future Sight', 'Doom Desire')
    if real_crit and d_ab in ('Battle Armor', 'Shell Armor'):
        real_crit = False
    cm = 2 if real_crit else 1
    if not special:
        s = ast.get('atk', 0)
        A = stage(atk, s) if (cm == 1 or s > 0) else atk
        dmg = A * bp * (2 * level // 5 + 2)
        s = dst.get('def', 0)
        Dm = stage(dfn, s) if (cm == 1 or s < 0) else dfn
        if Dm == 0:
            return None
        dmg = dmg // Dm // 50
        if df.get('reflect') and cm == 1 and name != 'Brick Break':
            dmg = 2 * (dmg // 3) if doubles else dmg // 2
        if doubles and targeting == 'target_both_enemies':
            dmg //= 2
        if dmg == 0:
            dmg = 1
    else:
        s = ast.get('spa', 0)
        A = stage(spa, s) if (cm == 1 or s > 0) else spa
        dmg = A * bp * (2 * level // 5 + 2)
        s = dst.get('spd', 0)
        Dm = stage(spd, s) if (cm == 1 or s < 0) else spd
        if Dm == 0:
            return None
        dmg = dmg // Dm // 50
        if df.get('light_screen') and cm == 1 and name != 'Brick Break':
            dmg = 2 * (dmg // 3) if doubles else dmg // 2
        if doubles and targeting == 'target_both_enemies':
            dmg //= 2
        if active:
            if weather == 'Rain':
                if mtype == 'Fire':
                    dmg //= 2
                elif mtype == 'Water':
                    dmg = 15 * dmg // 10
            if weather in ('Rain', 'Sandstorm', 'Hail') and name == 'SolarBeam':
                dmg //= 2
            if weather == 'Harsh Sunlight':
                if mtype == 'Fire':
                    dmg = 15 * dmg // 10
                elif mtype == 'Water':
                    dmg //= 2
    dmg += 2
    # Cmd_damagecalc: crit, dmgMultiplier
    dmg *= cm
    dm = 1
    bonus_moves = ('Gust', 'Twister', 'Surf', 'Whirlpool', 'Earthquake', 'Pursuit', 'Stomp', 'Extrasensory', 'Astonish',
                   'Needle Arm', 'Facade', 'SmellingSalt', 'Revenge')
    if name in bonus_moves and custom and 'No Bonus' not in custom:
        dm = 2
    elif name == 'Magnitude' and 'Dig Bonus' in custom:
        dm = 2
    elif name == 'Weather Ball' and active:
        dm = 2
    if name == 'Spit Up':
        dmg *= n
    dmg *= dm
    # typecalc
    if not no_typecalc:
        if mtype in a_types:
            dmg = dmg * 15 // 10
        for (ra, rd, mul) in table(3):
            if ra != mtype:
                continue
            for t in ([d1] + ([d2] if d2 != d1 else [])):
                if rd == t:
                    dmg = dmg * mul // 10
                    if dmg == 0 and mul != 0:
                        dmg = 1
    if name == 'Spit Up':
        return {dmg: 1}
    dist = rolls_gen34(dmg)
    nh = 1 if _single else multi_hits(case)
    if nh > 1:
        other = gen3(case, False, _single=True) if crit else dist
        for _ in range(nh - 1):
            dist = convolve(dist, other)
    return dist


# ---------------------------------------------------------------------------
# gen 4
# ---------------------------------------------------------------------------

PUNCH_G4 = {"Ice Punch", "Fire Punch", "ThunderPunch", "Mach Punch", "Focus Punch", "Dizzy Punch", "DynamicPunch",
            "Hammer Arm", "Mega Punch", "Comet Punch", "Meteor Mash", "Shadow Punch", "Drain Punch", "Bullet Punch",
            "Sky Uppercut"}
RECKLESS_G4 = {"Jump Kick", "Hi Jump Kick", "Take Down", "Submission", "Double-Edge", "Volt Tackle", "Brave Bird",
               "Wood Hammer", "Flare Blitz", "Head Smash"}
NATURE_POWER_G4 = {'Plain/Sand': ('Earthquake', 100, 'Ground', 'Physical', 'Others'), 'Grass/Puddle': ('Seed Bomb', 80, 'Grass', 'Physical', 'Foe Or Ally'),
                   'Mountain/Cave': ('Rock Slide', 75, 'Rock', 'Physical', 'All Foes'), 'Snow': ('Blizzard', 120, 'Ice', 'Special', 'All Foes'),
                   'Water': ('Hydro Pump', 120, 'Water', 'Special', 'Foe Or Ally'), 'Ice': ('Ice Beam', 95, 'Ice', 'Special', 'Foe Or Ally'),
                   'Building': ('Tri Attack', 80, 'Normal', 'Special', 'Foe Or Ally'), 'Great Marsh': ('Mud Bomb', 65, 'Ground', 'Special', 'Foe Or Ally'),
                   'Bridge': ('Air Slash', 75, 'Flying', 'Special', 'Foe Or Ally')}
PLATES = {"Draco Plate": 'Dragon', "Dread Plate": 'Dark', "Earth Plate": 'Ground', "Fist Plate": 'Fighting', "Flame Plate": 'Fire',
          "Icicle Plate": 'Ice', "Insect Plate": 'Bug', "Iron Plate": 'Steel', "Meadow Plate": 'Grass', "Mind Plate": 'Psychic',
          "Sky Plate": 'Flying', "Splash Plate": 'Water', "Spooky Plate": 'Ghost', "Stone Plate": 'Rock', "Toxic Plate": 'Poison',
          "Zap Plate": 'Electric'}


def divide_g4(dividend, divisor):
    if dividend == 0:
        return 0
    q = dividend // divisor
    return q if q != 0 else 1


def gen4(case, crit, _single=False):
    a, d, mv = case['attacker'], case['defender'], case['move']
    name, custom, fl = mv['name'], case['custom'], mv['flavors']
    level = a['level']
    af = case['attacking_field'] or {}
    df = case['defending_field'] or {}
    a_ab = 'Insomnia' if af.get('worry_seed') else ('' if af.get('gastro_acid') else a['ability'])
    d_ab = 'Insomnia' if df.get('worry_seed') else ('' if df.get('gastro_acid') else d['ability'])
    weather = case['weather']
    active = weather != 'None' and a_ab not in ('Air Lock', 'Cloud Nine') and d_ab not in ('Air Lock', 'Cloud Nine')
    a_types = forecast(a['types'], a_ab, weather, active)
    d_types = forecast(d['types'], d_ab, weather, active)
    d1, d2 = d_types
    ast = case['attacking_stages'] or {}
    dst = case['defending_stages'] or {}
    doubles = case['doubles']
    a_item, d_item = a['held_item'] or '', d['held_item'] or ''
    if a_ab == 'Klutz':
        a_item_eff = ''
    else:
        a_item_eff = a_item
    d_item_eff = '' if d_ab == 'Klutz' else d_item
    if a_ab == 'Multitype' and a_item in PLATES:
        a_types = [PLATES[a_item]] * 2

    mtype = mv['type']
    bp = mv['power'] or 0
    category = mv['category']
    targeting = mv['targeting']
    if name == 'Hidden Power':
        mtype, bp = hidden_power_gen3(a['dvs'])
    if name == 'Nature Power':
        np = NATURE_POWER_G4.get(custom)
        if np is None:
            return None
        name, bp, mtype, category, targeting = np
    if name == 'Weather Ball' and active:
        bp *= 2
        mtype = {'Rain': 'Water', 'Sandstorm': 'Rock', 'Harsh Sunlight': 'Fire', 'Hail': 'Ice'}.get(weather, 'Normal')
    if name == 'Natural Gift' or name == 'Fling':
        return 'SKIP'  # item tables verified separately
    if name == 'Present':
        if custom == 'Heal':
            return None
        bp = int(custom) if custom.isdigit() else 40
    if name == 'Judgment' and a_item in PLATES:
        mtype = PLATES[a_item]
    if a_ab == 'Normalize':
        mtype = 'Normal'
    struggle = name == 'Struggle'
    ignore_type_checks = struggle or name in ('Future Sight', 'Doom Desire', 'Counter', 'Mirror Coat', 'Metal Burst', 'Psywave', 'Bide')

    # fixed-damage / special moves
    def imm_check():
        for (ra, rd, mul) in table(4):
            if ra == mtype and mul == 0 and mult_types(rd, d1, d2):
                if rd == 'Flying' and (df.get('gravity') or df.get('roost') or d_item_eff == 'Iron Ball'):
                    continue
                if rd == 'Dark' and df.get('miracle_eye'):
                    continue
                if rd == 'Ghost' and a_ab == 'Scrappy':
                    continue
                return True
        return False

    def wg_blocks():
        se = nve = False
        for (ra, rd, mul) in table(4):
            if ra != mtype:
                continue
            for t in ([d1] + ([d2] if d2 != d1 else [])):
                if rd == t:
                    if rd == 'Flying' and mul == 0 and (df.get('gravity') or df.get('roost') or d_item_eff == 'Iron Ball'):
                        continue
                    if rd == 'Dark' and mul == 0 and df.get('miracle_eye'):
                        continue
                    if rd == 'Ghost' and mul == 0 and a_ab == 'Scrappy':
                        continue
                    if mul == 0:
                        return True
                    if mul == 5:
                        if se:
                            se = False
                        else:
                            nve = True
                    elif mul == 20:
                        if nve:
                            nve = False
                        else:
                            se = True
        return not se

    if name in ('Seismic Toss', 'Night Shade', 'SonicBoom', 'Dragon Rage'):
        if imm_check():
            return None
        if d_ab == 'Wonder Guard' and wg_blocks():
            return None
        return {{'Seismic Toss': level, 'Night Shade': level, 'SonicBoom': 20, 'Dragon Rage': 40}[name]: 1}
    if name == 'Endeavor':
        diff = d['cur_hp'] - a['cur_hp']
        return {diff: 1} if diff > 0 else None
    if name in ('Counter', 'Mirror Coat', 'Metal Burst', 'Bide'):
        if name != 'Metal Burst' and imm_check():
            return None
        if d_ab == 'Wonder Guard' and wg_blocks():
            return None
        if not (custom.isdigit() and int(custom) > 0):
            return None
        taken = int(custom)
        return {taken * 15 // 10 if name == 'Metal Burst' else taken * 2: 1}
    if name == 'Beat Up':
        n = int(custom) if custom.isdigit() else 1
        cm = 1
        if crit and d_ab not in ('Battle Armor', 'Shell Armor'):
            cm = 3 if a_ab == 'Sniper' else 2
        base = a['base']['atk'] * 10 * (level * 2 // 5 + 2) // max(d['base']['def'], 1) // 50 + 2
        total = rolls_gen34(base * cm)
        one = rolls_gen34(base)
        for _ in range(n - 1):
            total = convolve(total, one)
        return total
    if name == 'Triple Kick':
        kicks = int(custom) if custom.isdigit() else 1
        total = None
        for k in range(1, kicks + 1):
            sub = dict(case)
            sub['move'] = dict(mv, name='__tk__', power=10 * k)
            sub['custom'] = ''
            dist = gen4(sub, crit and k == kicks)
            if dist is None:
                continue
            total = dist if total is None else convolve(total, dist)
        return total
    if name == '__tk__':
        name = 'Triple Kick'

    # immunities
    if not ignore_type_checks:
        if imm_check():
            return None
        if d_ab == 'Levitate' and mtype == 'Ground' and not (df.get('gravity') or df.get('roost')) and d_item_eff != 'Iron Ball':
            return None
        if df.get('magnet_rise') and mtype == 'Ground' and d_item_eff != 'Iron Ball':
            return None
        if d_ab == 'Damp' and name in ('Explosion', 'Selfdestruct'):
            return None
        if d_ab in ('Volt Absorb', 'Motor Drive') and mtype == 'Electric':
            return None
        if d_ab in ('Water Absorb', 'Dry Skin') and mtype == 'Water':
            return None
        if d_ab == 'Flash Fire' and mtype == 'Fire':
            return None
        if d_ab == 'Soundproof' and name in ('Uproar', 'Snore', 'Hyper Voice', 'Bug Buzz', 'Chatter'):
            return None
    if name == 'Super Fang':
        if d_ab == 'Wonder Guard' and wg_blocks():
            return None
        return {max(d['cur_hp'] // 2, 1): 1}
    if name in ('Guillotine', 'Horn Drill', 'Fissure', 'Sheer Cold'):
        if d_ab == 'Sturdy' or level < d['level']:
            return None
        if d_ab == 'Wonder Guard' and wg_blocks():
            return None
        return {d['cur_hp']: 1}
    if name == 'Psywave':
        out = {}
        for r in range(0, 11):
            v = level * (r + 5) // 10
            if v == 0:
                v = 1
            out[v] = out.get(v, 0) + 1
        return out

    # variable power (script-level movePower)
    power_mul = 10
    bonus_moves = ('Gust', 'Twister', 'Surf', 'Whirlpool', 'Earthquake', 'Pursuit', 'Stomp', 'Facade', 'SmellingSalt',
                   'Revenge', 'Assurance', 'Avalanche', 'Brine', 'Payback', 'Wake-Up Slap')
    if name in bonus_moves and custom and 'No Bonus' not in custom:
        power_mul = 20
    if name == 'Magnitude':
        bp = {'Mag 4': 10, 'Mag 5': 30, 'Mag 6': 50, 'Mag 7': 70, 'Mag 8': 90, 'Mag 9': 110, 'Mag 10': 150}[[k for k in ('Mag 10', 'Mag 4', 'Mag 5', 'Mag 6', 'Mag 7', 'Mag 8', 'Mag 9') if k in custom][0]]
        if 'Dig Bonus' in custom:
            power_mul = 20
    elif name in ('Flail', 'Reversal'):
        bp = {'100-69 % HP': 20, '69-35 % HP': 40, '35-20 % HP': 80, '20-10 % HP': 100, '10-4 % HP': 150, '4-0 % HP': 200}[custom]
    elif name in ('Return', 'Frustration'):
        bp = int(custom) if custom.isdigit() else bp
    elif name in ('Eruption', 'Water Spout'):
        pct = int(custom) if custom.isdigit() else 100
        bp = bp * pct // 100
        if bp == 0:
            bp = 1
    elif name in ('Crush Grip', 'Wring Out'):
        pct = int(custom) if custom.isdigit() else 100
        bp = 1 + 120 * pct // 100
    elif name == 'Gyro Ball':
        aspe, dspe = a['staged']['spe'], d['staged']['spe']
        if af.get('trick_room'):
            aspe = 1808 - aspe
        if df.get('trick_room'):
            dspe = 1808 - dspe
        bp = min(150, 1 + 25 * dspe // max(aspe, 1))
    elif name == 'Trump Card':
        bp = {'4+': 40, '3': 50, '2': 60, '1': 80, '0': 200}.get(custom, 40)
    elif name in ('Low Kick', 'Grass Knot'):
        w = d['weight']
        if w is None:
            return 'SKIP'
        hg = int(round(w * 10))
        bp = 20 if hg <= 100 else 40 if hg <= 250 else 60 if hg <= 500 else 80 if hg <= 1000 else 100 if hg <= 2000 else 120
    elif name in ('Rollout', 'Ice Ball'):
        if 'DefenseCurl' in custom:
            bp = bp * 16 * 2
        else:
            bp = bp * (2 ** (int(custom) - 1))
    elif name == 'Fury Cutter':
        bp = bp * (2 ** (min(int(custom), 5) - 1))
    elif name == 'Spit Up':
        bp = 100 * (int(custom) if custom.isdigit() else 1)
    elif name == 'Punishment':
        s = sum(max(0, dst.get(k, 0)) for k in ('atk', 'def', 'spa', 'spd', 'spe', 'acc', 'eva'))
        bp = min(200, 60 + 20 * s)
    if name in RECKLESS_G4 and a_ab == 'Reckless':
        power_mul = 12
    if bp == 0:
        return None
    if name == 'Struggle':
        mtype = 'Normal'

    # ---- BattleSystem_CalcMoveDamage ----
    atk, dfn = a['raw']['atk'], d['raw']['def']
    spa, spd = a['raw']['spa'], d['raw']['spd']
    if af.get('power_trick'):
        atk, adef = a['raw']['def'], a['raw']['atk']
    if df.get('power_trick'):
        dfn = d['raw']['atk']
    power = bp * power_mul // 10
    if a_ab == 'Technician' and not struggle and power <= 60:
        power = power * 15 // 10
    if a_ab in ('Huge Power', 'Pure Power'):
        atk *= 2
    if af.get('slow_start'):
        atk //= 2
    boost_t = held_boost_type(4, a_item_eff)
    if boost_t == mtype:
        power = power * 120 // 100
    if a_item_eff == 'Choice Band':
        atk = atk * 150 // 100
    if a_item_eff == 'Choice Specs':
        spa = spa * 150 // 100
    if a_item_eff == 'Soul Dew' and a['name'] in ('Latios', 'Latias'):
        spa = spa * 150 // 100
    if d_item_eff == 'Soul Dew' and d['name'] in ('Latios', 'Latias'):
        spd = spd * 150 // 100
    if a_item_eff == 'DeepSeaTooth' and a['name'] == 'Clamperl':
        spa *= 2
    if d_item_eff == 'DeepSeaScale' and d['name'] == 'Clamperl':
        spd *= 2
    if a_item_eff == 'Light Ball' and a['name'] == 'Pikachu':
        power *= 2
    if d_item_eff == 'Metal Powder' and d['name'] == 'Ditto':
        dfn *= 2
    if a_item_eff == 'Thick Club' and a['name'] in ('Cubone', 'Marowak'):
        atk *= 2
    if a_item_eff == 'Adamant Orb' and a['name'] == 'Dialga' and mtype in ('Dragon', 'Steel'):
        power = power * 120 // 100
    if a_item_eff == 'Lustrous Orb' and a['name'] == 'Palkia' and mtype in ('Dragon', 'Water'):
        power = power * 120 // 100
    if a_item_eff == 'Griseous Orb' and a['name'] == 'Giratina' and mtype in ('Dragon', 'Ghost'):
        power = power * 120 // 100
    if a_item_eff == 'Muscle Band' and category == 'Physical':
        power = power * 110 // 100
    if a_item_eff == 'Wise Glasses' and category == 'Special':
        power = power * 110 // 100
    if d_ab == 'Thick Fat' and mtype in ('Fire', 'Ice'):
        power //= 2
    if a_ab == 'Hustle':
        atk = atk * 150 // 100
    if mtype == 'Fire' and d_ab == 'Heatproof':
        power //= 2
    if mtype == 'Fire' and d_ab == 'Dry Skin':
        power = power * 125 // 100
    if name in PUNCH_G4 and a_ab == 'Iron Fist':
        power = power * 12 // 10
    if active:
        if weather == 'Harsh Sunlight' and a_ab == 'Solar Power':
            spa = spa * 15 // 10
        if weather == 'Sandstorm' and 'Rock' in d_types:
            spd = spd * 15 // 10
        if weather == 'Harsh Sunlight' and a_ab == 'Flower Gift':
            atk = atk * 15 // 10
        if weather == 'Harsh Sunlight' and d_ab == 'Flower Gift':
            spd = spd * 15 // 10
    if name in ('Explosion', 'Selfdestruct'):
        dfn //= 2
    cm = 1
    if crit and name not in ('Future Sight', 'Doom Desire') and d_ab not in ('Battle Armor', 'Shell Armor'):
        cm = 3 if a_ab == 'Sniper' else 2
    if category == 'Physical':
        s = ast.get('atk', 0)
        A = stage(atk, s) if (cm == 1 or s > 0) else atk
        dmg = A * power * (level * 2 // 5 + 2)
        s = dst.get('def', 0)
        Dm = stage(dfn, s) if (cm == 1 or s < 0) else dfn
        if Dm == 0:
            return None
        dmg = dmg // Dm // 50
        if df.get('reflect') and cm == 1 and name != 'Brick Break':
            dmg = dmg * 2 // 3 if doubles else dmg // 2
    else:
        s = ast.get('spa', 0)
        A = stage(spa, s) if (cm == 1 or s > 0) else spa
        dmg = A * power * (level * 2 // 5 + 2)
        s = dst.get('spd', 0)
        Dm = stage(spd, s) if (cm == 1 or s < 0) else spd
        if Dm == 0:
            return None
        dmg = dmg // Dm // 50
        if df.get('light_screen') and cm == 1 and name != 'Brick Break':
            dmg = dmg * 2 // 3 if doubles else dmg // 2
    if doubles and targeting in ('All Foes', 'Others'):
        dmg = dmg * 3 // 4
    if active:
        if weather == 'Rain':
            if mtype == 'Fire':
                dmg //= 2
            elif mtype == 'Water':
                dmg = dmg * 15 // 10
        if weather in ('Rain', 'Sandstorm', 'Hail', 'Fog') and name == 'SolarBeam':
            dmg //= 2
        if weather == 'Harsh Sunlight':
            if mtype == 'Fire':
                dmg = dmg * 15 // 10
            elif mtype == 'Water':
                dmg //= 2
    dmg += 2
    # BattleScript_CalcMoveDamage
    dmg *= cm
    if a_item_eff == 'Life Orb':
        dmg = dmg * 130 // 100
    # ApplyTypeChart
    if not struggle:
        se = nve = False
        if not ignore_type_checks and mtype in a_types:
            dmg = dmg * 2 if a_ab == 'Adaptability' else dmg * 15 // 10
        if not ignore_type_checks:
            for (ra, rd, mul) in table(4):
                if ra != mtype:
                    continue
                for t in ([d1] + ([d2] if d2 != d1 else [])):
                    if rd == t:
                        if rd == 'Flying' and mul == 0 and (df.get('gravity') or df.get('roost') or d_item_eff == 'Iron Ball'):
                            continue
                        if rd == 'Dark' and mul == 0 and df.get('miracle_eye'):
                            continue
                        if rd == 'Ghost' and mul == 0 and a_ab == 'Scrappy':
                            continue
                        dmg = divide_g4(dmg * mul, 10)
                        if mul == 5:
                            if se:
                                se = False
                            else:
                                nve = True
                        elif mul == 20:
                            if nve:
                                nve = False
                            else:
                                se = True
            if d_ab == 'Wonder Guard' and not se and bp:
                return None
            if se:
                if d_ab in ('Filter', 'Solid Rock'):
                    dmg = divide_g4(dmg * 3, 4)
                if a_item_eff == 'Expert Belt':
                    dmg = dmg * 120 // 100
            if nve and a_ab == 'Tinted Lens':
                dmg *= 2
    if name == 'Spit Up':
        return {dmg: 1}
    dist = rolls_gen34(dmg)
    nh = 1 if _single else multi_hits(case)
    if nh > 1:
        other = gen4(case, False, _single=True) if crit else dist
        for _ in range(nh - 1):
            dist = convolve(dist, other)
    return dist


REF = {1: gen1, 2: gen2, 3: gen3, 4: gen4}
