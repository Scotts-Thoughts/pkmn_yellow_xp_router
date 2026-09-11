import math
import logging
import copy
from typing import Dict, List

from pkmn import universal_data_objects, damage_calc
from pkmn.gen_5.data_objects import get_hidden_power_type, get_hidden_power_base_power, modify_stat_by_stage
from utils.constants import const
from pkmn.gen_5.gen_five_constants import gen_five_const


logger = logging.getLogger(__name__)


MIN_RANGE = 85
MAX_RANGE = 100
NUM_ROLLS = MAX_RANGE - MIN_RANGE + 1


def get_crit_rate(cur_mon:universal_data_objects.EnemyPkmn, move:universal_data_objects.Move, custom_move_data:str):
    # Frost Breath / Storm Throw always crit in gen 5; their moves.json `effect` is
    # mislabelled "high_crit_rate" (a +1 crit-stage effect), not a guaranteed crit.
    if move.name in gen_five_const.ALWAYS_CRIT_MOVES:
        return 1.0

    stage = 0
    if const.FLAVOR_HIGH_CRIT in move.attack_flavor:
        stage += 1
    if move.name == gen_five_const.NATURE_POWER_MOVE_NAME and custom_move_data == gen_five_const.LONG_GRASS_TERRAIN:
        stage += 1
    if cur_mon.ability == gen_five_const.SUPER_LUCK_ABILITY:
        stage += 1

    if stage == 0:
        return (1/16)
    elif stage == 1:
        return (1/8)
    elif stage == 2:
        return (1/4)
    elif stage == 3:
        return (1/3)
    # stages 4+
    return (1/2)


def get_move_accuracy(
    pkmn:universal_data_objects.EnemyPkmn,
    move:universal_data_objects.Move,
    custom_move_data:str,
    defending_pkmn:universal_data_objects.EnemyPkmn,
    weather:str,
):
    if pkmn.ability == gen_five_const.NO_GUARD_ABILITY or defending_pkmn.ability == gen_five_const.NO_GUARD_ABILITY:
        return None

    if move.name == gen_five_const.NATURE_POWER_MOVE_NAME:
        # Gen 5 Nature Power calls the mapped move (Earthquake/Seed Bomb/Rock
        # Slide/.../Air Slash per terrain), so its accuracy should be the CALLED
        # move's accuracy, not a terrain-keyed value. A prior bug here (bug X7 in
        # docs/damage_calc_review/README.md) unconditionally overwrote every branch
        # with 100 regardless of terrain; this was never fully modelled and is left
        # at the called moves' accuracy is not tracked by this dropdown, so fall back
        # to the move's own table accuracy (None, i.e. no data) rather than guessing.
        result = move.accuracy
    else:
        result = move.accuracy

    # Certain weather makes specific moves bypass accuracy checks entirely
    # (they always hit, unaffected by accuracy/evasion)
    if move.name == gen_five_const.BLIZZARD_MOVE_NAME and weather == const.WEATHER_HAIL:
        return None
    if move.name == gen_five_const.THUNDER_MOVE_NAME and weather == const.WEATHER_RAIN:
        return None
    if move.name == gen_five_const.THUNDER_MOVE_NAME and weather == const.WEATHER_SUN:
        result = 50
    if move.name == gen_five_const.HURRICANE_MOVE_NAME:
        if weather == const.WEATHER_RAIN:
            return None
        elif weather == const.WEATHER_SUN:
            result = 50

    if result is None:
        return None

    if pkmn.ability == gen_five_const.COMPOUND_EYES_ABILITY:
        result = min(
            math.floor(result * 1.3),
            100
        )
    elif pkmn.ability == gen_five_const.HUSTLE_ABILITY:
        is_physical = move.category == const.CATEGORY_PHYSICAL
        if move.name == gen_five_const.NATURE_POWER_MOVE_NAME:
            is_physical = custom_move_data in [gen_five_const.SAND_TERRAIN, gen_five_const.CAVE_TERRAIN, gen_five_const.TALL_GRASS_TERRAIN]
        if is_physical:
            result = math.floor(result * 3277 / 4096)
    
    if defending_pkmn.ability == gen_five_const.SAND_VEIL_ABILITY and weather == const.WEATHER_SANDSTORM:
        result = math.floor(result * 3277 / 4096)

    if defending_pkmn.ability == gen_five_const.SNOW_CLOAK_ABILITY and weather == const.WEATHER_HAIL:
        result = math.floor(result * 3277 / 4096)

    if (
        pkmn.held_item == gen_five_const.WIDE_LENS_NAME and
        pkmn.ability != gen_five_const.KLUTZ_ABILITY
    ):
        result = min(
            math.floor(result * 4506 / 4096),
            100
        )

    return result


def calculate_gen_five_damage(
    attacking_pkmn:universal_data_objects.EnemyPkmn,
    attacking_species:universal_data_objects.PokemonSpecies,
    move:universal_data_objects.Move,
    defending_pkmn:universal_data_objects.EnemyPkmn,
    defending_species:universal_data_objects.PokemonSpecies,
    type_chart:Dict[str, Dict[str, str]],
    held_item_boost_table:Dict[str, str],
    attacking_stage_modifiers:universal_data_objects.StageModifiers=None,
    defending_stage_modifiers:universal_data_objects.StageModifiers=None,
    attacking_field:universal_data_objects.FieldStatus=None,
    defending_field:universal_data_objects.FieldStatus=None,
    is_crit:bool=False,
    custom_move_data:str="",
    weather:str=const.WEATHER_NONE,
    is_double_battle:bool=False,
    attacking_battle_stats:universal_data_objects.StatBlock=None,
    defending_battle_stats:universal_data_objects.StatBlock=None,
):
    if attacking_field is None:
        attacking_field = universal_data_objects.FieldStatus()
    if defending_field is None:
        defending_field = universal_data_objects.FieldStatus()
    # Defaulted up front (not just before the crit-stage handling further down) because
    # Punishment's power depends on defending_stage_modifiers and is resolved earlier.
    if attacking_stage_modifiers is None:
        attacking_stage_modifiers = universal_data_objects.StageModifiers()
    if defending_stage_modifiers is None:
        defending_stage_modifiers = universal_data_objects.StageModifiers()

    attacking_ability = attacking_pkmn.ability
    defending_ability = defending_pkmn.ability

    if attacking_field.worry_seed:
        attacking_ability = gen_five_const.INSOMNIA_ABILITY
    elif attacking_field.gastro_acid:
        attacking_ability = ""

    if defending_field.worry_seed:
        defending_ability = gen_five_const.INSOMNIA_ABILITY
    elif defending_field.gastro_acid:
        defending_ability = ""

    # NOTE: resolved up front because everything that keys off the weather (Forecast,
    # Weather Ball's type) has to be settled before any type-based immunity check
    is_weather_active = damage_calc.is_weather_active(attacking_ability, defending_ability, weather)

    # Forecast retypes Castform to match the weather, which changes STAB when it's
    # attacking and type effectiveness when it's defending
    attacking_species = damage_calc.apply_forecast(attacking_species, attacking_ability, weather, is_weather_active)
    defending_species = damage_calc.apply_forecast(defending_species, defending_ability, weather, is_weather_active)

    # Special-damage moves (Dragon Rage 40, Sonic Boom 20, Seismic Toss/Night Shade = level)
    # don't store their damage in base_power. Type immunity IS respected here (gen 2+).
    special_override = damage_calc.get_special_damage_override(move, attacking_pkmn, defending_species, type_chart)
    if special_override is not None:
        return special_override

    # Struggle is typeless in gen 5 (no STAB, no type chart, no immunities -- it hits
    # Ghosts) rather than a Normal-type move; resolved up front since it gates both the
    # immunity checks below and STAB later.
    is_struggle = move.name == const.STRUGGLE_MOVE_NAME

    if move.name == const.HIDDEN_POWER_MOVE_NAME:
        move_type = get_hidden_power_type(attacking_pkmn.dvs)
        base_power = get_hidden_power_base_power(attacking_pkmn.dvs)
    else:
        move_type = move.move_type
        base_power = move.base_power
    
    # NOTE: unlike gen 1-4 (which use a placeholder power of 1 for variable-power
    # moves), gen 5's moves.json uses `power: null` for them. There used to be a
    # `base_power is None or base_power == 0: return None` guard right here, before any
    # of the per-move power overrides below had a chance to run -- which meant every
    # variable-power move (Low Kick, Flail, Return, Present, Gyro Ball, Natural Gift,
    # Trump Card, Crush Grip, Wring Out, Punishment, Psywave, Super Fang, Endeavor, the
    # OHKO moves, Frustration, Spit Up, Heavy Slam, Heat Crash, Electro Ball, Stored
    # Power...) always reported "no damage" (gen_5_findings.md section 1, item 4). The
    # equivalent guard now runs much further down, after every override has resolved.
    attacking_mon_first_type = attacking_species.first_type
    attacking_mon_second_type = attacking_species.second_type

    if attacking_ability == gen_five_const.MULTITYPE_ABILITY:
        new_type = gen_five_const.PLATE_TYPE_LOOKUP.get(attacking_pkmn.held_item)
        if new_type:
            attacking_mon_first_type = new_type
            attacking_mon_second_type = new_type

    # Need to resolve any moves/abilities that change move type as early as possible
    if move.name == gen_five_const.NATURE_POWER_MOVE_NAME:
        # TODO: nature power emulates 2 moves that have possible bonus damage: earthquake, and surf. We're just fully ignoring those for now
        # TODO: should i just... pull the move info directly, instead of hard-coding it like this? idk
        if custom_move_data == gen_five_const.PLAIN_TERRAIN:
            base_power = 60
            move_type = const.TYPE_NORMAL
        elif custom_move_data == gen_five_const.SAND_TERRAIN:
            base_power = 100
            move_type = const.TYPE_GROUND
        elif custom_move_data == gen_five_const.CAVE_TERRAIN:
            base_power = 80
            move_type = const.TYPE_GHOST
        elif custom_move_data == gen_five_const.ROCK_TERRAIN:
            base_power = 75
            move_type = const.TYPE_ROCK
        elif custom_move_data == gen_five_const.TALL_GRASS_TERRAIN:
            # ugly, but wtv. This turns into stun spore
            return None
        elif custom_move_data == gen_five_const.LONG_GRASS_TERRAIN:
            base_power = 55
            move_type = const.TYPE_GRASS
        elif custom_move_data == gen_five_const.POND_WATER_TERRAIN:
            base_power = 65
            move_type = const.TYPE_WATER
        elif custom_move_data == gen_five_const.SEA_WATER_TERRAIN:
            base_power = 95
            move_type = const.TYPE_WATER
        elif custom_move_data == gen_five_const.UNDERWATER_TERRAIN:
            base_power = 120
            move_type = const.TYPE_WATER
    elif move.name == const.WEATHER_BALL_MOVE_NAME and is_weather_active:
        base_power *= 2
        move_type = damage_calc.get_weather_ball_type(weather, is_weather_active, default_type=move_type)
    elif move.name == gen_five_const.NATURAL_GIFT_MOVE_NAME:
        # Klutz suppresses the held item, so Natural Gift fails
        if attacking_ability == gen_five_const.KLUTZ_ABILITY:
            return None
        berry_data = gen_five_const.NATURAL_GIFT_BERRY_DATA.get(attacking_pkmn.held_item)
        if berry_data is None:
            return None
        base_power, move_type = berry_data

    if (
        attacking_ability == gen_five_const.TECHNICIAN_ABILITY and
        base_power <= 60
    ):
        base_power = math.floor(60 * 1.5)

    if attacking_ability == gen_five_const.NORMALIZE_ABILITY:
        move_type = const.TYPE_NORMAL
    
    if move.name == gen_five_const.JUDGMENT_MOVE_NAME:
        new_type = gen_five_const.PLATE_TYPE_LOOKUP.get(attacking_pkmn.held_item)
        if new_type:
            move_type = new_type
    
    if move.name == gen_five_const.PUNISHMENT_MOVE_NAME:
        # power = 60 + 20*(sum of the target's positive stat stages, incl. acc/eva),
        # cap 200. json power is null, and the old code added the stage bonus onto that
        # null power instead of the real base of 60 (gen_4_findings.md bug #4d, ported).
        positive_stage_total = sum(max(stage, 0) for stage in [
            defending_stage_modifiers.attack_stage,
            defending_stage_modifiers.defense_stage,
            defending_stage_modifiers.special_attack_stage,
            defending_stage_modifiers.special_defense_stage,
            defending_stage_modifiers.speed_stage,
            defending_stage_modifiers.accuracy_stage,
            defending_stage_modifiers.evasion_stage,
        ])
        base_power = min(60 + 20 * positive_stage_total, 200)

    is_scrappy_active = (
        (defending_species.first_type == const.TYPE_GHOST or defending_species.second_type == const.TYPE_GHOST) and
        (move.move_type == const.TYPE_NORMAL or move.move_type == const.TYPE_FIGHTING) and
        attacking_ability == gen_five_const.SCRAPPY_ABILITY
    )

    ignore_ground_immunity = (
        (defending_species.first_type == const.TYPE_FLYING or defending_species.second_type == const.TYPE_FLYING) and
        move.move_type == const.TYPE_GROUND and
        (defending_field.gravity or defending_field.roost)
    )

    ignore_dark_immunity = (
        (defending_species.first_type == const.TYPE_DARK or defending_species.second_type == const.TYPE_DARK) and
        move.move_type == const.TYPE_PSYCHIC and
        defending_field.miracle_eye
    )

    
    if is_struggle:
        pass
    elif (
        (
            type_chart.get(move_type).get(defending_species.first_type) == const.IMMUNE or
            type_chart.get(move_type).get(defending_species.second_type) == const.IMMUNE
        ) and
        (not is_scrappy_active) and
        (not ignore_ground_immunity) and
        (not ignore_dark_immunity)
    ):
        return None
    elif (
        defending_ability == gen_five_const.LEVITATE_ABILITY and
        move_type == const.TYPE_GROUND and
        (not ignore_ground_immunity)
    ):
        return None
    elif (
        defending_ability == gen_five_const.DAMP_ABILITY and
        (
            move.name == const.SELFDESTRUCT_MOVE_NAME or
            move.name == gen_five_const.SELFDESTRUCT_MOVE_NAME or
            move.name == const.EXPLOSION_MOVE_NAME
        )
    ):
        return None
    elif (
        (
            defending_ability == gen_five_const.VOLT_ABOSRB_ABILITY or
            defending_ability == gen_five_const.LIGHTNING_ROD_ABILITY or
            defending_ability == gen_five_const.MOTOR_DRIVE_ABILITY
        ) and
        move_type == const.TYPE_ELECTRIC
    ):
        return None
    elif (
        defending_ability == gen_five_const.WATER_ABSORB_ABILITY and
        move_type == const.TYPE_WATER
    ):
        return None
    elif (
        defending_ability == gen_five_const.FLASH_FIRE_ABILITY and
        move_type == const.TYPE_FIRE
    ):
        return None
    elif (
        defending_ability == gen_five_const.DRY_SKIN_ABILITY and
        move_type == const.TYPE_WATER
    ):
        return None
    elif defending_ability == gen_five_const.WONDER_GUARD_ABILITY:
        first_effectiveness = type_chart.get(move_type).get(defending_species.first_type)
        second_effectiveness = type_chart.get(move_type).get(defending_species.second_type)
        # if either is immune, then shedinja is immune
        # if either is not very effective, then shedinja is net neutral at worst. And thus immune
        if (
            first_effectiveness == const.IMMUNE or first_effectiveness == const.NOT_VERY_EFFECTIVE or
            second_effectiveness == const.IMMUNE or second_effectiveness == const.NOT_VERY_EFFECTIVE
        ):
            return None
        # if neither is super effective, then shedinja is also immune
        elif (
            first_effectiveness != const.SUPER_EFFECTIVE and second_effectiveness != const.SUPER_EFFECTIVE
        ):
            return None
        # we are left with only the cases where at least one is super effective, and the other is either neutral or super effective
    elif (
        defending_field.magnet_rise and
        move.move_type == const.TYPE_GROUND
    ):
        return None
    
    # special move interactions
    if const.FLAVOR_FIXED_DAMAGE in move.attack_flavor:
        return damage_calc.DamageRange({base_power: 1})
    elif const.FLAVOR_LEVEL_DAMAGE in move.attack_flavor:
        return damage_calc.DamageRange({attacking_pkmn.level: 1})
    elif const.FLAVOR_PSYWAVE in move.attack_flavor:
        # Gen 5 Psywave (documented, unverified -- no gen 5 decomp exists):
        # floor(level * (r + 50) / 100), r uniform 0..100 (101 equally likely values),
        # min 1. This differs from every earlier gen's Psywave formula.
        damage_vals = {}
        for r in range(0, 101):
            dmg = max(math.floor(attacking_pkmn.level * (r + 50) / 100), 1)
            damage_vals[dmg] = damage_vals.get(dmg, 0) + 1
        return damage_calc.DamageRange(damage_vals)

    # when a crit occurs, always ignore negative modifiers for the attacking pokemon, and always ignore positive modifiers for the defensive pokemon
    # gen_4_findings.md bug #1 (ported verbatim into this gen 5 copy): a crit should
    # ignore only the USED attacking stat's negative stage and the USED defending
    # stat's positive stage -- not check the opposite category's stats, and not wipe
    # every other stage (accuracy, evasion, Speed, ...) along with it.
    if is_crit:
        if move.category == const.CATEGORY_PHYSICAL:
            if attacking_stage_modifiers.attack_stage < 0:
                attacking_stage_modifiers = attacking_stage_modifiers.apply_stat_mod([(const.ATK, -attacking_stage_modifiers.attack_stage)])
            if defending_stage_modifiers.defense_stage > 0:
                defending_stage_modifiers = defending_stage_modifiers.apply_stat_mod([(const.DEF, -defending_stage_modifiers.defense_stage)])
        else:
            if attacking_stage_modifiers.special_attack_stage < 0:
                attacking_stage_modifiers = attacking_stage_modifiers.apply_stat_mod([(const.SPA, -attacking_stage_modifiers.special_attack_stage)])
            if defending_stage_modifiers.special_defense_stage > 0:
                defending_stage_modifiers = defending_stage_modifiers.apply_stat_mod([(const.SPD, -defending_stage_modifiers.special_defense_stage)])

    if attacking_battle_stats is None:
        attacking_battle_stats = attacking_pkmn.get_battle_stats(attacking_stage_modifiers, mon_field=attacking_field)

    if defending_battle_stats is None:
        defending_battle_stats = defending_pkmn.get_battle_stats(defending_stage_modifiers, mon_field=defending_field)

    # TODO: present. ughhhhh
    # Handle base_power and move_type override cases first
    if move.name == gen_five_const.MAGNITUDE_MOVE_NAME:
        if gen_five_const.MAGNITUDE_4 in custom_move_data:
            base_power = 10
        elif gen_five_const.MAGNITUDE_5 in custom_move_data:
            base_power = 30
        elif gen_five_const.MAGNITUDE_6 in custom_move_data:
            base_power = 50
        elif gen_five_const.MAGNITUDE_7 in custom_move_data:
            base_power = 70
        elif gen_five_const.MAGNITUDE_8 in custom_move_data:
            base_power = 90
        elif gen_five_const.MAGNITUDE_9 in custom_move_data:
            base_power = 110
        elif gen_five_const.MAGNITUDE_10 in custom_move_data:
            base_power = 150
    elif move.name in [const.FLAIL_MOVE_NAME, const.REVERSAL_MOVE_NAME]:
        if gen_five_const.FLAIL_FULL_HP in custom_move_data:
            base_power = 20
        elif gen_five_const.FLAIL_HALF_HP in custom_move_data:
            base_power = 40
        elif gen_five_const.FLAIL_QUARTER_HP in custom_move_data:
            base_power = 80
        elif gen_five_const.FLAIL_TEN_PERCENT_HP in custom_move_data:
            base_power = 100
        elif gen_five_const.FLAIL_FIVE_PERCENT_HP in custom_move_data:
            base_power = 150
        elif gen_five_const.FLAIL_MIN_HP in custom_move_data:
            base_power = 200
    elif move.name == gen_five_const.RETURN_MOVE_NAME:
        try:
            base_power = int(custom_move_data)
        except Exception as e:
            logger.warning(f"Failed to convert return move power to an int: {custom_move_data}")
    elif move.name == gen_five_const.ERUPTION_MOVE_NAME:
        try:
            base_power = math.floor(base_power * int(custom_move_data) / 100.0)
        except Exception as e:
            logger.warning(f"Failed to convert return move power to an int: {custom_move_data}")
    elif move.name == gen_five_const.WATER_SPOUT_MOVE_NAME:
        try:
            base_power = math.floor(base_power * int(custom_move_data) / 100.0)
        except Exception as e:
            logger.warning(f"Failed to convert return move power to an int: {custom_move_data}")
    elif (
        move.name == gen_five_const.CRUSH_GRIP_MOVE_NAME or
        move.name == gen_five_const.WRING_OUT_MOVE_NAME
    ):
        try:
            # power = floor(1 + 120*curHP/maxHP); custom_move_data is that HP % (1..100),
            # so it must be divided back down to a fraction. The prior version omitted the
            # /100 entirely, so "100" HP produced a base power of 12001 instead of 121.
            base_power = math.floor(1 + (120 * int(custom_move_data) / 100))
        except Exception as e:
            logger.warning(f"Failed to convert return move power to an int: {custom_move_data}")
    elif move.name == gen_five_const.GYRO_BALL_MOVE_NAME:
        # power = floor(1 + 25*targetSpeed/userSpeed), cap 150 -- the ratio was inverted
        # (a fast user got a low-power Gyro Ball instead of a high-power one).
        base_power = math.floor(1 + ((25 * defending_battle_stats.speed) / attacking_battle_stats.speed))
        base_power = min(base_power, 150)
    elif move.name == gen_five_const.TRUMP_CARD_MOVE_NAME:
        if custom_move_data == "4+":
            base_power = 40
        elif custom_move_data == "3":
            base_power = 50
        elif custom_move_data == "2":
            base_power = 60
        elif custom_move_data == "1":
            base_power = 80
        elif custom_move_data == "0":
            base_power = 200
    elif move.name in [gen_five_const.LOW_KICK_MOVE_NAME, gen_five_const.GRASS_KNOW_MOVE_NAME, gen_five_const.HEAVY_SLAM_MOVE_NAME, gen_five_const.HEAT_CRASH_MOVE_NAME]:
        is_weight_ratio_move = move.name in [gen_five_const.HEAVY_SLAM_MOVE_NAME, gen_five_const.HEAT_CRASH_MOVE_NAME]
        if defending_species.weight is None or (is_weight_ratio_move and attacking_species.weight is None):
            base_power = 40 if is_weight_ratio_move else 20
            logger.warning(f"Undefined weight for species: {defending_species.name}")
        elif is_weight_ratio_move:
            # Heavy Slam / Heat Crash: power keyed on the USER's weight relative to the
            # target's, not the target's absolute weight.
            ratio = attacking_species.weight / defending_species.weight
            if ratio >= 5:
                base_power = 120
            elif ratio >= 4:
                base_power = 100
            elif ratio >= 3:
                base_power = 80
            elif ratio >= 2:
                base_power = 60
            else:
                base_power = 40
        elif defending_species.weight <= 10:
            base_power = 20
        elif defending_species.weight <= 25:
            base_power = 40
        elif defending_species.weight <= 50:
            base_power = 60
        elif defending_species.weight <= 100:
            base_power = 80
        elif defending_species.weight <= 200:
            base_power = 100
        else:
            base_power = 120
    elif move.name == gen_five_const.ELECTRO_BALL_MOVE_NAME:
        # power by speed ratio user/target (battle speeds, incl. stages/items): >=4x:150,
        # >=3x:120, >=2x:80, >=1x:60, else 40.
        if defending_battle_stats.speed <= 0:
            ratio = float("inf")
        else:
            ratio = attacking_battle_stats.speed / defending_battle_stats.speed
        if ratio >= 4:
            base_power = 150
        elif ratio >= 3:
            base_power = 120
        elif ratio >= 2:
            base_power = 80
        elif ratio >= 1:
            base_power = 60
        else:
            base_power = 40
    elif move.name == gen_five_const.STORED_POWER_MOVE_NAME:
        # power = 20 + 20 * (sum of the user's positive stat stages, incl. acc/eva), cap 860.
        positive_stage_total = sum(max(stage, 0) for stage in [
            attacking_stage_modifiers.attack_stage,
            attacking_stage_modifiers.defense_stage,
            attacking_stage_modifiers.special_attack_stage,
            attacking_stage_modifiers.special_defense_stage,
            attacking_stage_modifiers.speed_stage,
            attacking_stage_modifiers.accuracy_stage,
            attacking_stage_modifiers.evasion_stage,
        ])
        base_power = min(20 + 20 * positive_stage_total, 860)
    elif move.name == gen_five_const.HEX_MOVE_NAME:
        if custom_move_data == gen_five_const.STATUS_BONUS:
            base_power = 100
    elif move.name == gen_five_const.VENOSHOCK_MOVE_NAME:
        if custom_move_data == gen_five_const.POISONED_BONUS:
            base_power = 130
    elif move.name == gen_five_const.RETALIATE_MOVE_NAME:
        if custom_move_data == gen_five_const.ALLY_FAINTED_BONUS:
            base_power = 140
    elif move.name == gen_five_const.ECHOED_VOICE_MOVE_NAME:
        try:
            consecutive_turns = int(custom_move_data)
        except Exception:
            consecutive_turns = 1
        base_power = min(40 * consecutive_turns, 200)
    elif move.name == gen_five_const.ACROBATICS_MOVE_NAME:
        if not attacking_pkmn.held_item:
            base_power = 110
    elif move.name == gen_five_const.PRESENT_MOVE_NAME:
        if custom_move_data == "80":
            base_power = 80
        elif custom_move_data == "120":
            base_power = 120
        elif custom_move_data == gen_five_const.HEAL_OPTION:
            # Present heals instead of dealing damage; no damage row to report.
            base_power = None
        else:
            base_power = 40
    elif move.name == gen_five_const.FRUSTRATION_MOVE_NAME:
        try:
            base_power = int(custom_move_data)
        except Exception as e:
            logger.warning(f"Failed to convert frustration move power to an int: {custom_move_data}")

    # These moves don't go through the base_power/Attack/Defense formula at all -- their
    # damage is derived directly from HP or speed -- so they return straight away. This
    # app doesn't track in-battle HP loss, so "current HP" here means "full HP" (the same
    # simplification used elsewhere, e.g. Flail/Reversal default to a HP% dropdown rather
    # than a live value); Endeavor gets a dropdown for the attacker's assumed HP % since
    # its whole point is being used after the attacker has already taken damage.
    if move.name in gen_five_const.ONE_HIT_KO_MOVE_NAMES:
        if attacking_battle_stats.speed < defending_battle_stats.speed:
            return None
        return damage_calc.DamageRange({defending_pkmn.cur_stats.hp: 1})
    elif move.name == gen_five_const.SUPER_FANG_MOVE_NAME:
        return damage_calc.DamageRange({max(defending_pkmn.cur_stats.hp // 2, 1): 1})
    elif move.name == gen_five_const.ENDEAVOR_MOVE_NAME:
        try:
            attacker_hp_pct = int(custom_move_data)
        except Exception:
            attacker_hp_pct = 100
        attacker_hp = math.floor(attacking_pkmn.cur_stats.hp * attacker_hp_pct / 100)
        remaining = defending_pkmn.cur_stats.hp - attacker_hp
        if remaining <= 0:
            return None
        return damage_calc.DamageRange({remaining: 1})
    elif move.name == gen_five_const.FINAL_GAMBIT_MOVE_NAME:
        return damage_calc.DamageRange({attacking_pkmn.cur_stats.hp: 1})

    # NOTE: for now, just ignoring the "edge case" of: what if the mon for mon-specific unique items has klutz?
    # it never occurs in normal gameplay, and would require a hack. so, wtv
    if attacking_pkmn.name == gen_five_const.MAROWAK_NAME and attacking_pkmn.held_item == gen_five_const.THICK_CLUB_NAME:
        attacking_battle_stats.attack *= 2
    elif attacking_pkmn.name == gen_five_const.PIKACHU_NAME and attacking_pkmn.held_item == gen_five_const.LIGHT_BALL_NAME:
        attacking_battle_stats.special_attack *= 2
    elif attacking_pkmn.name == gen_five_const.CLAMPERL_NAME and attacking_pkmn.held_item == gen_five_const.DEEP_SEA_TOOTH_NAME:
        attacking_battle_stats.special_attack *= 2
    elif attacking_pkmn.name == gen_five_const.CLAMPERL_NAME and attacking_pkmn.held_item == gen_five_const.DEEP_SEA_SCALE_NAME:
        attacking_battle_stats.special_defense *= 2
    elif (
        (attacking_pkmn.name == gen_five_const.LATIOS_NAME or attacking_pkmn.name == gen_five_const.LATIAS_NAME) and
        attacking_pkmn.held_item == gen_five_const.DEEP_SEA_SCALE_NAME
    ):
        attacking_battle_stats.special_attack = math.floor(attacking_battle_stats.special_attack * 1.5)
        attacking_battle_stats.special_defense = math.floor(attacking_battle_stats.special_defense * 1.5)

    if (
        (defending_pkmn.name == gen_five_const.LATIOS_NAME or defending_pkmn.name == gen_five_const.LATIAS_NAME) and
        defending_pkmn.held_item == gen_five_const.DEEP_SEA_SCALE_NAME
    ):
        defending_battle_stats.special_attack = math.floor(defending_battle_stats.special_attack * 1.5)
        defending_battle_stats.special_defense = math.floor(defending_battle_stats.special_defense * 1.5)
    elif defending_pkmn.name == gen_five_const.DITTO_NAME and defending_pkmn.held_item == gen_five_const.METAL_POWDER_NAME:
        # this should only apply while transformed... but also like, we don't support transforming in the app rn lmao
        defending_battle_stats.defense *= 2
    
    if attacking_ability == gen_five_const.HUSTLE_ABILITY:
        attacking_battle_stats.attack = math.floor(attacking_battle_stats.attack * 1.5)

    if (
        attacking_ability == gen_five_const.HUGE_POWER_ABILITY or
        attacking_ability == gen_five_const.PURE_POWER_ABILITY
    ):
        attacking_battle_stats.attack = math.floor(attacking_battle_stats.attack * 2)
    
    if (
        attacking_pkmn.held_item == gen_five_const.CHOICE_BAND_NAME and
        attacking_ability != gen_five_const.KLUTZ_ABILITY
    ):
        attacking_battle_stats.attack = math.floor(attacking_battle_stats.attack * 1.5)

    if (
        attacking_pkmn.held_item == gen_five_const.CHOICE_SPECS_NAME and
        attacking_ability != gen_five_const.KLUTZ_ABILITY
    ):
        attacking_battle_stats.special_attack = math.floor(attacking_battle_stats.special_attack * 1.5)
    
    if defending_ability == gen_five_const.THICK_FAT_ABILITY and move_type in [const.TYPE_FIRE, const.TYPE_ICE]:
        # oddity: this is technically how the actual code does it, despite it being a bit weird
        # When thick fat is applicable, it debuffs the special attack (technically before applying stages, but wtv)
        attacking_battle_stats.special_attack = math.floor(attacking_battle_stats.special_attack / 2)
        attacking_battle_stats.attack = math.floor(attacking_battle_stats.attack / 2)

    if defending_ability == gen_five_const.HEATPROOF_ABILITY and move_type == const.TYPE_FIRE:
        # seems to reuse the same code as thick-fat (based on bulbapedia's description)
        attacking_battle_stats.special_attack = math.floor(attacking_battle_stats.special_attack / 2)
        attacking_battle_stats.attack = math.floor(attacking_battle_stats.attack / 2)
    
    if (
        defending_ability == gen_five_const.FLOWER_GIFT_ABILITY and
        is_weather_active and
        weather == const.WEATHER_SUN
    ):
        defending_battle_stats.special_attack = math.floor(defending_battle_stats.special_attack * 1.5)
        defending_battle_stats.special_defense = math.floor(defending_battle_stats.special_attack * 1.5)
    elif (
        attacking_ability == gen_five_const.FLOWER_GIFT_ABILITY and
        is_weather_active and
        weather == const.WEATHER_SUN
    ):
        attacking_battle_stats.special_attack = math.floor(attacking_battle_stats.special_attack * 1.5)
        attacking_battle_stats.special_defense = math.floor(attacking_battle_stats.special_attack * 1.5)
    elif (
        attacking_ability == gen_five_const.SOLAR_POWER_ABILITY and
        is_weather_active and
        weather == const.WEATHER_SUN
    ):
        attacking_battle_stats.special_attack = math.floor(attacking_battle_stats.special_attack * 1.5)

    if (
        is_weather_active and
        weather == const.WEATHER_SANDSTORM and
        (defending_species.first_type == const.TYPE_ROCK or defending_species.second_type == const.TYPE_ROCK)
    ):
        defending_battle_stats.special_defense = math.floor(defending_battle_stats.special_defense * 1.5)

    # NOTE: ignoring plus/minus abilities. They would modify special attack here, if ever relevant

    # TODO: bunch of abilities below that have a conditional activation. Need to figure out how to implement them properly
    # until then, they're just fully disabled
    if defending_ability == gen_five_const.MARVEL_SCALE_ABILITY and False:
        defending_battle_stats.defense = math.floor(defending_battle_stats.defense * 1.5)
    if attacking_ability == gen_five_const.GUTS_ABILITY and False:
        attacking_battle_stats.attack = math.floor(attacking_battle_stats.attack * 1.5)
    if attacking_ability == gen_five_const.OVERGROW_ABILITY and move_type == const.TYPE_GRASS and False:
        base_power = math.floor(base_power * 1.5)
    if attacking_ability == gen_five_const.BLAZE_ABILITY and move_type == const.TYPE_FIRE and False:
        base_power = math.floor(base_power * 1.5)
    if attacking_ability == gen_five_const.TORRENT_ABILITY and move_type == const.TYPE_WATER and False:
        base_power = math.floor(base_power * 1.5)
    if attacking_ability == gen_five_const.SWARM_ABILITY and move_type == const.TYPE_BUG and False:
        base_power = math.floor(base_power * 1.5)
    
    if move.category == const.CATEGORY_SPECIAL:
        attacking_stat = attacking_battle_stats.special_attack
        # Psyshock / Psystrike / Secret Sword are Special moves that hit the target's
        # DEFENSE (and Defense stage/screen), not Sp. Defense.
        if move.name in gen_five_const.USES_TARGET_DEFENSE_MOVES:
            defending_stat = defending_battle_stats.defense
            screen_field = defending_field.reflect
        else:
            defending_stat = defending_battle_stats.special_defense
            screen_field = defending_field.light_screen
        if screen_field and not is_crit and move.name != gen_five_const.BRICK_BREAK_MOVE_NAME:
            screen_active = True
        else:
            screen_active = False
    else:
        attacking_stat = attacking_battle_stats.attack
        defending_stat = defending_battle_stats.defense
        if defending_field.reflect and not is_crit and move.name != gen_five_const.BRICK_BREAK_MOVE_NAME:
            screen_active = True
        else:
            screen_active = False

    if move.name == gen_five_const.FOUL_PLAY_MOVE_NAME:
        # Foul Play uses the TARGET's Attack stat (with the target's own stages/items/
        # ability already folded into defending_battle_stats), not the user's Attack.
        attacking_stat = defending_battle_stats.attack

    if move.name in gen_five_const.IGNORES_DEFENSE_STAGES_MOVES:
        # Chip Away / Sacred Sword ignore the target's Defense/evasion stat stages.
        unboosted_defending_stats = defending_pkmn.get_battle_stats(
            universal_data_objects.StageModifiers(), mon_field=defending_field
        )
        defending_stat = unboosted_defending_stats.defense

    # Gen 5 removed the "halve the target's Defense" rule that Explosion/Selfdestruct had
    # in gens 2-4 (gen_5_findings.md move table), so defending_stat is left untouched here.

    is_stab = (not is_struggle) and ((attacking_mon_first_type == move_type) or (attacking_mon_second_type == move_type))

    if (
        held_item_boost_table.get(attacking_pkmn.held_item) == move_type and
        attacking_ability != gen_five_const.KLUTZ_ABILITY
    ):
        attacking_stat = math.floor(attacking_stat * 1.2)
    elif (
        attacking_pkmn.name == gen_five_const.DIALGA_NAME and
        attacking_pkmn.held_item == gen_five_const.ADAMANT_ORB_NAME and
        (move.move_type == const.TYPE_DRAGON or move.move_type == const.TYPE_STEEL)
    ):
        attacking_stat = math.floor(attacking_stat * 1.2)
    elif (
        attacking_pkmn.name == gen_five_const.PALKIA_NAME and
        attacking_pkmn.held_item == gen_five_const.LUSTROUS_ORB_NAME and
        (move.move_type == const.TYPE_DRAGON or move.move_type == const.TYPE_WATER)
    ):
        attacking_stat = math.floor(attacking_stat * 1.2)
    elif (
        attacking_pkmn.name == gen_five_const.GIRATINA_NAME and
        attacking_pkmn.held_item == gen_five_const.GRISEOUS_ORB_NAME and
        (move.move_type == const.TYPE_DRAGON or move.move_type == const.TYPE_GHOST)
    ):
        attacking_stat = math.floor(attacking_stat * 1.2)

    # Every per-move power override above has now run; a still-missing power means the
    # move genuinely has no damage formula implemented here (Counter, Mirror Coat, Metal
    # Burst, Bide, Fling, Beat Up, Techno Blast's drive-typing aside, Present's "Heal"
    # option, ...) rather than being blocked by the old too-early null check.
    if base_power is None or base_power == 0:
        return None

    # begin actual formula
    temp = 2 * attacking_pkmn.level
    temp = math.floor(temp / 5) + 2

    temp *= base_power
    temp *= attacking_stat
    temp = math.floor(temp / defending_stat)

    temp = math.floor(temp / 50)

    # start accounting for multipliers
    if screen_active:
        temp = math.floor(temp / 2)
    
    # Gen 5 spread factor is x0.75 (gen 4 is x0.5); the app's gen 4/5 move data uses
    # "All Foes"/"Others" for spread targeting, not the gen-3-only "target_both_enemies"
    # vocabulary, so that check alone never fired (gen_5_findings.md section 1, item 6).
    if is_double_battle and move.targeting in (const.TARGETING_BOTH_ENEMIES, "All Foes", "Others"):
        temp = math.floor(temp * 3 / 4)

    is_solar_beam = move.name == const.SOLAR_BEAM_MOVE_NAME or move.name == gen_five_const.SOLAR_BEAM_MOVE_NAME
    weather_boost = False
    weather_penalty = False
    if is_weather_active:
        if weather == const.WEATHER_RAIN:
            weather_boost = (move_type == const.TYPE_WATER)
            weather_penalty = (
                move_type == const.TYPE_FIRE or
                is_solar_beam
            )
        elif weather == const.WEATHER_SUN:
            weather_boost = (move_type == const.TYPE_FIRE)
            weather_penalty = (move_type == const.TYPE_WATER)
        elif weather != const.WEATHER_NONE:
            weather_penalty = is_solar_beam

    if weather_boost:
        temp = math.floor(temp * 1.5)
    elif weather_penalty:
        temp = math.floor(temp * 0.5)
    
    # TODO: when we support flash fire, support goes here
    flash_fire_activated = False
    if flash_fire_activated:
        temp = math.floor(temp * 1.5)

    temp += 2

    # Spit Up, Future Sight and Doom Desire all CAN crit in gen 5 (this used to exclude
    # them, a gen-4-era bug ported into the copy -- see gen_4_findings.md bug #5, and
    # gen_5_findings.md's Future Sight/Doom Desire row for the crit change).
    if (
        is_crit and
        defending_ability not in [gen_five_const.BATTLE_ARMOR_ABILITY, gen_five_const.SHELL_ARMOR_ABILITY]
    ):
        if attacking_ability == gen_five_const.SNIPER_ABILITY:
            temp *= 3
        else:
            temp *= 2
    
    # handle all the special moves that may affect the damage formula in other ways
    move_modifier = 1

    if move.name in [gen_five_const.ROLLOUT_MOVE_NAME, gen_five_const.ICE_BALL_MOVE_NAME]:
        # Turn n's power is base*2^(n-1) (turn 1 is NOT doubled), with Defense Curl
        # doubling it again on top -- the old `2**n` doubled turn 1 too, and "5 +
        # DefenseCurl" produced 2**6 (x64) instead of 2**4*2 (x32).
        if "DefenseCurl" in custom_move_data:
            num_rollout_turns = 5
            has_defense_curl = True
        else:
            try:
                num_rollout_turns = int(custom_move_data)
            except ValueError:
                num_rollout_turns = 5
            has_defense_curl = False

        move_modifier = 2 ** (num_rollout_turns - 1)
        if has_defense_curl:
            move_modifier *= 2
    elif move.name == gen_five_const.FURY_CUTTER_MOVE_NAME:
        # Gen 5 caps the doubling at the 4th hit (20 -> 40 -> 80 -> 160, then holds);
        # the dropdown's "6" option is kept only as a "5 or more" label.
        move_modifier = 2 ** (min(int(custom_move_data), 5) - 1)
    elif move.name == gen_five_const.TRIPLE_KICK_MOVE_NAME:
        move_modifier = int(custom_move_data)
    elif move.name == gen_five_const.SPIT_UP_MOVE_NAME:
        move_modifier = int(custom_move_data)
    
    temp *= move_modifier

    if move.name in [
        gen_five_const.GUST_MOVE_NAME,
        gen_five_const.TWISTER_MOVE_NAME,
        gen_five_const.SURF_MOVE_NAME,
        gen_five_const.WHIRLPOOL_MOVE_NAME,
        gen_five_const.EARTHQUAKE_MOVE_NAME,
        gen_five_const.PURSUIT_MOVE_NAME,
        gen_five_const.STOMP_MOVE_NAME,
        gen_five_const.STEAMROLLER_MOVE_NAME,
        gen_five_const.FACADE_MOVE_NAME,
        gen_five_const.SMELLING_SALT_MOVE_NAME,
        gen_five_const.REVENGE_MOVE_NAME,
        gen_five_const.ASSURANCE_MOVE_NAME,
        gen_five_const.AVALANCHE_MOVE_NAME,
        gen_five_const.BRINE_MOVE_NAME,
        gen_five_const.PAYBACK_MOVE_NAME,
        gen_five_const.WAKE_UP_SLAP_MOVE_NAME,
    ]:
        double_damage = custom_move_data and (gen_five_const.NO_BONUS not in custom_move_data)
    elif move.name == gen_five_const.MAGNITUDE_MOVE_NAME:
        # Magnitude folds the bonus into the same string as the magnitude level
        # ("Mag 7" vs "Mag 7 Dig Bonus"), so there is no "No Bonus" option to test for
        double_damage = (gen_five_const.DIG_BONUS in custom_move_data)
    else:
        double_damage = False

    if double_damage:
        temp *= 2

    # TODO: pretty much wholly outside the use case ofthis app, but here just in case
    is_helping_hand_active = False
    if is_helping_hand_active:
        temp = math.floor(temp * 1.5)
    
    # TODO: one day we might need to support this
    is_charge_active = False
    if is_charge_active and move.move_type == const.TYPE_ELECTRIC:
        temp *= 2

    if is_stab:
        if attacking_ability == gen_five_const.ADAPTABILITY_ABILITY:
            temp = math.floor(temp * 2)
        else:
            temp = math.floor(temp * 1.5)

    is_tinted_lens_active = False
    if not is_struggle:
        for test_type in type_chart.get(move_type):
            if test_type == defending_species.first_type or test_type == defending_species.second_type:
                effectiveness = type_chart.get(move_type).get(test_type)
                if effectiveness == const.SUPER_EFFECTIVE:
                    if (
                        defending_ability == gen_five_const.FILTER_ABILITY or
                        defending_ability == gen_five_const.SOLID_ROCK_ABILITY
                    ):
                        temp = math.floor(temp * 1.5)
                    else:
                        temp *= 2
                elif effectiveness == const.NOT_VERY_EFFECTIVE:
                    if attacking_ability == gen_five_const.TINTED_LENS_ABILITY:
                        is_tinted_lens_active = True
                    temp = math.floor(temp / 2)

    # doing all this so that we guarantee that you only get one tinted lens boost if the move is doubly resisted
    if is_tinted_lens_active:
        temp *= 2

    # NOTE: I don't really know where in the formula gen 4 multipliers go
    # So, just throwing them at the end
    if (
        defending_ability == gen_five_const.DRY_SKIN_ABILITY and
        move.move_type == const.TYPE_FIRE
    ):
        temp = math.floor(temp * 1.3)

    
    if temp <= 0:
        # damage must be at least 1
        temp = 1

    multi_hit_multiplier = 1
    if const.DOUBLE_HIT_FLAVOR in move.attack_flavor:
        multi_hit_multiplier = 2
    elif const.FLAVOR_MULTI_HIT in move.attack_flavor:
        # NOTE: if no custom_move_data is provided, we will only calculate one strike
        # this is intentional
        if const.MULTI_HIT_2 in custom_move_data:
            multi_hit_multiplier = 2
        elif const.MULTI_HIT_3 in custom_move_data:
            multi_hit_multiplier = 3
        elif const.MULTI_HIT_4 in custom_move_data:
            multi_hit_multiplier = 4
        elif const.MULTI_HIT_5 in custom_move_data:
            multi_hit_multiplier = 5

    damage_vals = {}
    if move.name in [const.SPIT_UP_MOVE_NAME]:
        damage_vals[temp] = 1
        result = damage_calc.DamageRange(damage_vals)
    else:
        for numerator in range(MIN_RANGE, MAX_RANGE + 1):
            cur_damage = max(math.floor((temp * numerator) / MAX_RANGE), 1)

            if cur_damage not in damage_vals:
                damage_vals[cur_damage] = 0
            
            damage_vals[cur_damage] += 1
        
        result = damage_calc.DamageRange(damage_vals)
        if multi_hit_multiplier > 1:
            if is_crit:
                # Currently forcing "crit" calculations to assume only one crit out of all strikes
                # So, when calculating full damage, need to get the damage of a single non-crit strike as well
                # intentionally overwriting custom_move_data to make sure we get the damage of only a single strike
                other_damage = calculate_gen_five_damage(
                    attacking_pkmn,
                    attacking_species,
                    move,
                    defending_pkmn,
                    defending_species,
                    type_chart,
                    held_item_boost_table,
                    attacking_stage_modifiers,
                    defending_stage_modifiers,
                    attacking_field=attacking_field,
                    defending_field=defending_field,
                    weather=weather,
                    is_double_battle=is_double_battle,
                    custom_move_data=""
                )
            else:
                other_damage = result
            
            for _ in range(1, multi_hit_multiplier):
                result = result + other_damage
    
    return result
