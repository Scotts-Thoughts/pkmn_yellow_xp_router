import math
from typing import Dict, List

from pkmn import universal_data_objects, damage_calc
from utils.constants import const
from pkmn.gen_1.gen_one_constants import gen_one_const

MIN_RANGE = 217
MAX_RANGE = 255
NUM_ROLLS = MAX_RANGE - MIN_RANGE + 1


def get_crit_rate(pkmn:universal_data_objects.EnemyPkmn, move:universal_data_objects.Move, custom_move_data:str=""):
    is_high_crit = const.FLAVOR_HIGH_CRIT in move.attack_flavor
    # Focus Energy (4.7, CriticalHitTest core.asm 4677-4685): quarters the normal crit rate and
    # halves the high-crit rate (a bug in the original game, faithfully replicated). Not wired
    # into any move's dropdown today (that would be a per-move custom_move_data option on every
    # damaging move, added by the caller), but the math is here and directly testable.
    has_focus_energy = bool(custom_move_data) and gen_one_const.FLAVOR_FOCUS_ENERGY in custom_move_data

    b = int(pkmn.base_stats.speed / 2)
    if has_focus_energy:
        b = int(b / 2)
    else:
        b = min(255, b * 2)

    if is_high_crit:
        b = min(255, b * 4)
    else:
        b = int(b / 2)

    return min(b, 255) / 256


def calculate_gen_one_damage(
    attacking_pkmn:universal_data_objects.EnemyPkmn,
    attacking_species:universal_data_objects.PokemonSpecies,
    move:universal_data_objects.Move,
    defending_pkmn:universal_data_objects.EnemyPkmn,
    defending_species:universal_data_objects.PokemonSpecies,
    special_types:List[str],
    type_chart:Dict[str, Dict[str, str]],
    attacking_stage_modifiers:universal_data_objects.StageModifiers=None,
    defending_stage_modifiers:universal_data_objects.StageModifiers=None,
    is_crit:bool=False,
    defender_has_light_screen:bool=False,
    defender_has_reflect:bool=False,
    custom_move_data:str="",
    attacking_battle_stats:universal_data_objects.StatBlock=None,
    defending_battle_stats:universal_data_objects.StatBlock=None,
    attacker_is_enemy:bool=False,
):
    # Special-damage moves (Dragon Rage 40, Sonic Boom 20, Seismic Toss/Night Shade = level)
    # don't store their damage in base_power. Type immunity is intentionally ignored in gen 1.
    special_override = damage_calc.get_special_damage_override(move, attacking_pkmn)
    if special_override is not None:
        return special_override

    if attacking_stage_modifiers is None:
        attacking_stage_modifiers = universal_data_objects.StageModifiers()
    if attacking_battle_stats is None:
        attacking_battle_stats = attacking_pkmn.get_battle_stats(attacking_stage_modifiers, is_crit=is_crit)

    if defending_stage_modifiers is None:
        defending_stage_modifiers = universal_data_objects.StageModifiers()
    if defending_battle_stats is None:
        defending_battle_stats = defending_pkmn.get_battle_stats(defending_stage_modifiers, is_crit=is_crit)

    first_type_effectiveness = type_chart.get(move.move_type, {}).get(defending_species.first_type)
    second_type_effectiveness = None
    if defending_species.first_type != defending_species.second_type:
        second_type_effectiveness = type_chart.get(move.move_type, {}).get(defending_species.second_type)
    is_immune = first_type_effectiveness == const.IMMUNE or second_type_effectiveness == const.IMMUNE

    # Super Fang (Bug 3 / 4.4): floor(target HP / 2), min 1; skips crit/STAB/type/immunity
    # entirely (SUPER_FANG_EFFECT jumps straight to the accuracy test, core.asm 4795-4812).
    if gen_one_const.FLAVOR_SUPER_FANG in move.attack_flavor:
        hp_pct = gen_one_const.SUPER_FANG_HP_PERCENTAGES.get(custom_move_data, 100)
        target_hp = math.floor(defending_pkmn.cur_stats.hp * hp_pct / 100)
        return damage_calc.DamageRange({max(math.floor(target_hp / 2), 1): 1})

    # OHKO moves (4.1): fails outright (no damage) if immune or the user isn't at least as fast
    # as the target; otherwise deals exactly the target's remaining HP. The 30% accuracy roll is
    # already carried by moves.json; the level-difference accuracy scaling isn't modelled.
    if gen_one_const.FLAVOR_ONE_HIT_KO in move.attack_flavor:
        if is_immune:
            return None
        if attacking_battle_stats.speed < defending_battle_stats.speed:
            return None
        return damage_calc.DamageRange({defending_pkmn.cur_stats.hp: 1})

    # Counter / Bide (4.2/4.3): both require "damage the target dealt", an input the app has no
    # other way to supply. custom_move_data carries it as a plain integer string (the target's
    # last hit for Counter, or the accumulated total for Bide); no type/crit check either way.
    if gen_one_const.FLAVOR_COUNTER in move.attack_flavor or gen_one_const.FLAVOR_BIDE in move.attack_flavor:
        prior_damage = None
        if custom_move_data and custom_move_data.strip().isdigit():
            prior_damage = int(custom_move_data.strip())
        if not prior_damage:
            return None
        return damage_calc.DamageRange({min(prior_damage * 2, 65535): 1})

    if move.base_power is None or move.base_power == 0:
        return None

    # special move interactions
    if const.FLAVOR_FIXED_DAMAGE in move.attack_flavor:
        return damage_calc.DamageRange({move.base_power: 1})
    elif const.FLAVOR_LEVEL_DAMAGE in move.attack_flavor:
        return damage_calc.DamageRange({attacking_pkmn.level: 1})
    elif const.FLAVOR_PSYWAVE in move.attack_flavor:
        # Bug 5: the enemy's Psywave can roll 0 (uniform 0..floor(1.5L)-1); the player's cannot
        # (uniform 1..floor(1.5L)-1) (core.asm 4828-4841 vs 4947-4960).
        psywave_upper_limit = math.floor(attacking_pkmn.level * 1.5)
        psywave_lower_limit = 0 if attacker_is_enemy else 1
        return damage_calc.DamageRange({x:1 for x in range(psywave_lower_limit, psywave_upper_limit)})

    if is_immune:
        return None

    if move.move_type in special_types:
        attacking_stat = attacking_battle_stats.special_attack
        defending_stat = defending_battle_stats.special_defense
        if defender_has_light_screen and not is_crit:
            doubled_def = True
        else:
            doubled_def = False
    else:
        attacking_stat = attacking_battle_stats.attack
        defending_stat = defending_battle_stats.defense
        if defender_has_reflect and not is_crit:
            doubled_def = True
        else:
            doubled_def = False
    
    if doubled_def:
        defending_stat *= 2              # 16-bit `sla c / rl b`, no cap (core.asm 4223-4224)

    # Bug 1 (GetDamageVarsForPlayerAttack .scaleStats, core.asm 4278-4298): if either 16-bit
    # stat is > 255, both are divided by 4 (floor); only the low byte reaches CalculateDamage,
    # so a doubled defence >= 1024 wraps modulo 256.
    if attacking_stat > 255 or defending_stat > 255:
        attacking_stat = attacking_stat // 4
        defending_stat = defending_stat // 4
        if attacking_stat == 0:
            attacking_stat = 1
    attacking_stat &= 0xFF               # `ld b, l`   (4300)
    defending_stat &= 0xFF               # only `c` reaches CalculateDamage (4301)

    # CalculateDamage EXPLODE_EFFECT (core.asm 4484-4489) -- AFTER scaling/truncation, not before.
    if move.name == const.EXPLOSION_MOVE_NAME or move.name == const.SELFDESTRUCT_MOVE_NAME:
        defending_stat = defending_stat >> 1
        if defending_stat == 0:
            defending_stat = 1

    if defending_stat == 0:
        # Reflect/Light Screen with a 512-513 defensive stat: the real game hangs in _Divide.
        return None

    is_stab = (attacking_species.first_type == move.move_type) or (attacking_species.second_type == move.move_type)

    temp = 2 * attacking_pkmn.level
    if is_crit:
        temp *= 2
    temp = math.floor(temp / 5) + 2

    temp *= move.base_power
    temp *= attacking_stat
    temp = math.floor(temp / defending_stat)

    temp = math.floor(temp / 50)
    # Bug 4: MAX_NEUTRAL_DAMAGE (999) - MIN_NEUTRAL_DAMAGE (2) cap, applied before the +2.
    temp = min(temp, 997)
    temp += 2

    stab_bonus = 0
    if is_stab:
        stab_bonus = math.floor(temp / 2)

    temp += stab_bonus

    # Bug 2: the game applies type effectiveness by walking the 82-row ROM `TypeEffects` table
    # top to bottom, not by checking the defender's type_1 then type_2 -- and it re-checks for
    # a 0 result after every row (a 0.25x hit of a small pre-roll damage "misses" mid-chain).
    effectiveness_steps = []
    if first_type_effectiveness is not None:
        row = gen_one_const.GEN1_TYPE_ROW_ORDER[(move.move_type, defending_species.first_type)]
        effectiveness_steps.append((row, first_type_effectiveness))
    if second_type_effectiveness is not None:
        row = gen_one_const.GEN1_TYPE_ROW_ORDER[(move.move_type, defending_species.second_type)]
        effectiveness_steps.append((row, second_type_effectiveness))
    effectiveness_steps.sort(key=lambda step: step[0])

    for _, effectiveness in effectiveness_steps:
        if effectiveness == const.SUPER_EFFECTIVE:
            temp *= 2
        elif effectiveness == const.NOT_VERY_EFFECTIVE:
            temp = math.floor(temp / 2)
        if temp == 0:
            return None

    # NOTE: in gen one, all multi-hit moves roll damage (including crit) only once
    # so, check whether a multi-hit occurs, and then just multiply the damage by the number of hits to get the final damage amount
    multi_hit_multiplier = 1
    if const.DOUBLE_HIT_FLAVOR in move.attack_flavor:
        multi_hit_multiplier = 2
    elif const.FLAVOR_MULTI_HIT in move.attack_flavor:
        if const.MULTI_HIT_2 in custom_move_data:
            multi_hit_multiplier = 2
        elif const.MULTI_HIT_3 in custom_move_data:
            multi_hit_multiplier = 3
        elif const.MULTI_HIT_4 in custom_move_data:
            multi_hit_multiplier = 4
        elif const.MULTI_HIT_5 in custom_move_data:
            multi_hit_multiplier = 5
    elif gen_one_const.FLAVOR_PARTIAL_TRAPPING in move.attack_flavor:
        # 4.6: Bind/Wrap/Fire Spin/Clamp roll damage (and crit, and accuracy) once on turn 1 and
        # repeat that exact damage for 2-5 turns total (core.asm 3725-3737).
        multi_hit_multiplier = gen_one_const.PARTIAL_TRAP_TURN_COUNTS.get(custom_move_data, 1)

    damage_vals = {}
    for numerator in range(MIN_RANGE, MAX_RANGE + 1):
        cur_damage = max(math.floor((temp * numerator) / MAX_RANGE), 1) * multi_hit_multiplier

        if cur_damage not in damage_vals:
            damage_vals[cur_damage] = 0
        
        damage_vals[cur_damage] += 1
    
    return damage_calc.DamageRange(damage_vals)
