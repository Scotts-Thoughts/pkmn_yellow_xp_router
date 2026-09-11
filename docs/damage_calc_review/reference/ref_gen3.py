"""Reference implementation transcribed from pokeemerald CalculateBaseDamage (src/pokemon.c:3106-3372),
Cmd_damagecalc (battle_script_commands.c:1290), Cmd_typecalc (1355) / ModulateDmgByType (1318),
ApplyRandomDmgMultiplier (1639)."""
STAGE = [(10,40),(10,35),(10,30),(10,25),(10,20),(10,15),(10,10),(15,10),(20,10),(25,10),(30,10),(35,10),(40,10)]
SPECIAL = {'Fire','Water','Grass','Electric','Psychic','Ice','Dragon','Dark'}

def st(stat, stage):
    n, d = STAGE[stage + 6]
    return stat * n // d

def base_damage(level, power, move_type, atk, dfn, spa, spd, atk_stage=0, def_stage=0, spa_stage=0, spd_stage=0,
                crit=False, reflect=False, light_screen=False, doubles=False, two_alive=False, spread=False,
                weather=None, move_name='', flash_fire=False, burn=False, explosion=False,
                huge_power=False, badge_atk=False, badge_def=False, badge_spa=False, badge_spd=False,
                type_item_param=0, choice_band=False, soul_dew_atk=False, soul_dew_def=False, deep_sea_tooth=False,
                deep_sea_scale=False, light_ball=False, metal_powder=False, thick_club=False, thick_fat=False,
                hustle=False, guts=False, marvel_scale=False, overgrow_etc=False):
    if huge_power: atk *= 2
    if badge_atk: atk = 110 * atk // 100
    if badge_def: dfn = 110 * dfn // 100
    if badge_spa: spa = 110 * spa // 100
    if badge_spd: spd = 110 * spd // 100
    if type_item_param:
        if move_type in SPECIAL: spa = spa * (type_item_param + 100) // 100
        else: atk = atk * (type_item_param + 100) // 100
    if choice_band: atk = 150 * atk // 100
    if soul_dew_atk: spa = 150 * spa // 100
    if soul_dew_def: spd = 150 * spd // 100
    if deep_sea_tooth: spa *= 2
    if deep_sea_scale: spd *= 2
    if light_ball: spa *= 2
    if metal_powder: dfn *= 2
    if thick_club: atk *= 2
    if thick_fat and move_type in ('Fire', 'Ice'): spa //= 2
    if hustle: atk = 150 * atk // 100
    if guts: atk = 150 * atk // 100
    if marvel_scale: dfn = 150 * dfn // 100
    if overgrow_etc: power = 150 * power // 100
    if explosion: dfn //= 2
    special = move_type in SPECIAL
    if not special:
        a = (st(atk, atk_stage) if atk_stage > 0 else atk) if crit else st(atk, atk_stage)
        dmg = a * power * (2 * level // 5 + 2)
        d = (st(dfn, def_stage) if def_stage < 0 else dfn) if crit else st(dfn, def_stage)
        dmg = dmg // d
        dmg //= 50
        if burn and not guts: dmg //= 2
        if reflect and not crit: dmg = 2 * (dmg // 3) if (doubles and two_alive) else dmg // 2
        if doubles and spread and two_alive: dmg //= 2
        if dmg == 0: dmg = 1
    else:
        a = (st(spa, spa_stage) if spa_stage > 0 else spa) if crit else st(spa, spa_stage)
        dmg = a * power * (2 * level // 5 + 2)
        d = (st(spd, spd_stage) if spd_stage < 0 else spd) if crit else st(spd, spd_stage)
        dmg = dmg // d
        dmg //= 50
        if light_screen and not crit: dmg = 2 * (dmg // 3) if (doubles and two_alive) else dmg // 2
        if doubles and spread and two_alive: dmg //= 2
        if weather == 'rain':
            if move_type == 'Fire': dmg //= 2
            elif move_type == 'Water': dmg = 15 * dmg // 10
        if weather in ('rain', 'sand', 'hail') and move_name == 'SolarBeam': dmg //= 2
        if weather == 'sun':
            if move_type == 'Fire': dmg = 15 * dmg // 10
            elif move_type == 'Water': dmg //= 2
        if flash_fire and move_type == 'Fire': dmg = 15 * dmg // 10
    return dmg + 2

def full(base, crit=False, dmg_multiplier=1, charge=False, helping_hand=False, stab=False, effs=()):
    dmg = base * (2 if crit else 1) * dmg_multiplier
    if charge: dmg *= 2
    if helping_hand: dmg = dmg * 15 // 10
    if stab: dmg = dmg * 15 // 10
    for m in effs:  # 20 / 5 / 0, in gTypeEffectiveness row order
        dmg = dmg * m // 10
        if dmg == 0 and m != 0: dmg = 1
    return dmg

def rolls(dmg):
    out = {}
    for r in range(16):
        v = dmg * (100 - r) // 100
        if v == 0: v = 1
        out[v] = out.get(v, 0) + 1
    return out

def rng(d):
    return (min(d), max(d))
