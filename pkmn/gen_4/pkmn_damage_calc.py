import math
import logging
import copy
from typing import Dict, List

from pkmn import universal_data_objects, damage_calc
from pkmn.gen_4.data_objects import get_hidden_power_type, get_hidden_power_base_power, modify_stat_by_stage
from utils.constants import const
from pkmn.gen_4.gen_four_constants import gen_four_const


logger = logging.getLogger(__name__)


MIN_RANGE = 85
MAX_RANGE = 100
NUM_ROLLS = MAX_RANGE - MIN_RANGE + 1


def get_crit_rate(cur_mon:universal_data_objects.EnemyPkmn, move:universal_data_objects.Move, custom_move_data:str):
    stage = 0
    if const.FLAVOR_HIGH_CRIT in move.attack_flavor:
        stage += 1
    # NOTE: Nature Power never carries a crit boost in gen 4 (it delegates entirely
    # to the called move, none of which raise crit stage); the old "Long Grass"
    # bonus here was a gen-3 leftover.
    if cur_mon.ability == gen_four_const.SUPER_LUCK_ABILITY:
        stage += 1
    if cur_mon.held_item in (gen_four_const.SCOPE_LENS_NAME, gen_four_const.RAZOR_CLAW_NAME):
        stage += 1
    if cur_mon.name == gen_four_const.CHANSEY_NAME and cur_mon.held_item == gen_four_const.LUCKY_PUNCH_NAME:
        stage += 2
    if cur_mon.name == gen_four_const.FARFETCHD_NAME and cur_mon.held_item == gen_four_const.STICK_NAME:
        stage += 2
    stage = min(stage, 4)

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
    if pkmn.ability == gen_four_const.NO_GUARD_ABILITY or defending_pkmn.ability == gen_four_const.NO_GUARD_ABILITY:
        return None

    if move.name == gen_four_const.NATURE_POWER_MOVE_NAME:
        # Gen 4 Nature Power calls a real move (see the base-power table); accuracy
        # is that called move's accuracy, not a fixed 100 for every terrain.
        terrain_data = gen_four_const.NATURE_POWER_MOVE_TABLE.get(custom_move_data)
        result = terrain_data[2] if terrain_data is not None else 100
    elif move.name in gen_four_const.OHKO_MOVE_NAMES:
        # Sturdy/speed-gating is a hard fail handled in the damage calc itself (it
        # returns None outright); here we only need the game's `30 + (Latk-Ldef)` term.
        result = max(0, 30 + (pkmn.level - defending_pkmn.level))
    else:
        result = move.accuracy

    # Certain weather makes specific moves bypass accuracy checks entirely
    # (they always hit, unaffected by accuracy/evasion)
    if move.name == gen_four_const.BLIZZARD_MOVE_NAME and weather == const.WEATHER_HAIL:
        return None
    if move.name == gen_four_const.THUNDER_MOVE_NAME and weather == const.WEATHER_RAIN:
        return None
    if move.name == gen_four_const.THUNDER_MOVE_NAME and weather == const.WEATHER_SUN:
        result = 50

    if result is None:
        return None

    if pkmn.ability == gen_four_const.COMPOUND_EYES_ABILITY:
        result = min(
            math.floor(result * 1.3),
            100
        )
    elif pkmn.ability == gen_four_const.HUSTLE_ABILITY:
        is_physical = move.category == const.CATEGORY_PHYSICAL
        if move.name == gen_four_const.NATURE_POWER_MOVE_NAME:
            is_physical = custom_move_data in gen_four_const.NATURE_POWER_PHYSICAL_TERRAINS
        if is_physical:
            result = math.floor(result * 3277 / 4096)

    if defending_pkmn.held_item in (gen_four_const.BRIGHT_POWDER_NAME, gen_four_const.LAX_INCENSE_NAME):
        result = max(result - 10, 0)

    if (
        pkmn.held_item == gen_four_const.WIDE_LENS_NAME and
        pkmn.ability != gen_four_const.KLUTZ_ABILITY
    ):
        result = min(
            math.floor(result * 1.1),
            100
        )

    if defending_pkmn.ability == gen_four_const.SAND_VEIL_ABILITY and weather == const.WEATHER_SANDSTORM:
        result = math.floor(result * 3277 / 4096)

    if defending_pkmn.ability == gen_four_const.SNOW_CLOAK_ABILITY and weather == const.WEATHER_HAIL:
        result = math.floor(result * 3277 / 4096)

    if weather == const.WEATHER_FOG:
        result = math.floor(result * 6 / 10)

    return result


def calculate_gen_four_damage(
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

    attacking_ability = attacking_pkmn.ability
    defending_ability = defending_pkmn.ability

    if attacking_field.worry_seed:
        attacking_ability = gen_four_const.INSOMNIA_ABILITY
    elif attacking_field.gastro_acid:
        attacking_ability = ""

    if defending_field.worry_seed:
        defending_ability = gen_four_const.INSOMNIA_ABILITY
    elif defending_field.gastro_acid:
        defending_ability = ""

    if attacking_stage_modifiers is None:
        attacking_stage_modifiers = universal_data_objects.StageModifiers()
    if defending_stage_modifiers is None:
        defending_stage_modifiers = universal_data_objects.StageModifiers()

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

    # Endeavor: typeless, damage = defender's HP - attacker's HP (fails if not positive).
    if move.name == gen_four_const.ENDEAVOR_MOVE_NAME:
        diff = defending_pkmn.cur_stats.hp - attacking_pkmn.cur_stats.hp
        if diff <= 0:
            return None
        return damage_calc.DamageRange({diff: 1})

    if move.name == const.HIDDEN_POWER_MOVE_NAME:
        move_type = get_hidden_power_type(attacking_pkmn.dvs)
        base_power = get_hidden_power_base_power(attacking_pkmn.dvs)
    else:
        move_type = move.move_type
        base_power = move.base_power

    attacking_mon_first_type = attacking_species.first_type
    attacking_mon_second_type = attacking_species.second_type

    if attacking_ability == gen_four_const.MULTITYPE_ABILITY:
        new_type = gen_four_const.PLATE_TYPE_LOOKUP.get(attacking_pkmn.held_item)
        if new_type:
            attacking_mon_first_type = new_type
            attacking_mon_second_type = new_type

    # Struggle is entirely typeless (no STAB, no type chart, no immunities -- it hits
    # Ghosts); Future Sight/Doom Desire's stored damage is likewise never passed
    # through the type chart (no STAB, no effectiveness, no immunity -- it hits Dark).
    is_no_type_effects = move.name in (
        gen_four_const.STRUGGLE_MOVE_NAME,
        const.FUTURE_SIGHT_MOVE_NAME,
        const.DOOM_DESIRE_MOVE_NAME,
    )

    # Triple Kick is 3 independent hits with their own power (10/20/30), each with its
    # own crit/roll/accuracy -- model it as up to 3 recursive single-hit calls (tagged
    # via a private custom_move_data sentinel so the recursive calls fall straight
    # through to the vanilla pipeline below instead of re-entering this branch).
    if move.name == gen_four_const.TRIPLE_KICK_MOVE_NAME and not custom_move_data.startswith("__tk_hit_"):
        try:
            num_kicks = int(custom_move_data)
        except (ValueError, TypeError):
            num_kicks = 1
        num_kicks = max(1, min(num_kicks, 3))

        combined = None
        for kick_num in range(1, num_kicks + 1):
            kick_range = calculate_gen_four_damage(
                attacking_pkmn, attacking_species, move, defending_pkmn, defending_species,
                type_chart, held_item_boost_table, attacking_stage_modifiers, defending_stage_modifiers,
                attacking_field=attacking_field, defending_field=defending_field, is_crit=is_crit,
                custom_move_data=f"__tk_hit_{kick_num * 10}__", weather=weather, is_double_battle=is_double_battle,
            )
            if kick_range is None:
                continue
            combined = kick_range if combined is None else combined + kick_range
        return combined

    # Need to resolve any moves/abilities that change move type/power as early as possible
    if move.name == gen_four_const.TRIPLE_KICK_MOVE_NAME:
        # sentinel from the recursion above: "__tk_hit_10__" / "_20__" / "_30__"
        base_power = int(custom_move_data[len("__tk_hit_"):-2])
    elif move.name == gen_four_const.NATURE_POWER_MOVE_NAME:
        terrain_data = gen_four_const.NATURE_POWER_MOVE_TABLE.get(custom_move_data)
        if terrain_data is None:
            return None
        base_power, move_type, _ = terrain_data
    elif move.name == const.WEATHER_BALL_MOVE_NAME and is_weather_active:
        base_power *= 2
        move_type = damage_calc.get_weather_ball_type(weather, is_weather_active, default_type=move_type)
    elif move.name == gen_four_const.NATURAL_GIFT_MOVE_NAME:
        # Klutz suppresses the held item, so Natural Gift fails
        if attacking_ability == gen_four_const.KLUTZ_ABILITY:
            return None
        berry_data = gen_four_const.NATURAL_GIFT_BERRY_DATA.get(attacking_pkmn.held_item)
        if berry_data is None:
            return None
        base_power, move_type = berry_data
    elif move.name == gen_four_const.FLING_MOVE_NAME:
        if attacking_ability == gen_four_const.KLUTZ_ABILITY or not attacking_pkmn.held_item:
            return None
        fling_power = gen_four_const.FLING_POWER_TABLE.get(attacking_pkmn.held_item)
        if fling_power is None:
            return None
        base_power = fling_power
    elif move.name == gen_four_const.PRESENT_MOVE_NAME:
        if custom_move_data == "Heal":
            # Present heals the target instead of dealing damage; not this calc's job.
            return None
        try:
            base_power = int(custom_move_data)
        except (ValueError, TypeError):
            base_power = 40

    if move.name in gen_four_const.RECKLESS_MOVES and attacking_ability == gen_four_const.RECKLESS_ABILITY:
        base_power = math.floor(base_power * 1.2)

    if (
        attacking_ability == gen_four_const.TECHNICIAN_ABILITY and
        move.name != gen_four_const.STRUGGLE_MOVE_NAME and
        base_power <= 60
    ):
        base_power = math.floor(base_power * 1.5)

    if move.name in gen_four_const.PUNCH_MOVES and attacking_ability == gen_four_const.IRON_FIST_ABILITY:
        base_power = math.floor(base_power * 1.2)

    if attacking_pkmn.name == gen_four_const.PIKACHU_NAME and attacking_pkmn.held_item == gen_four_const.LIGHT_BALL_NAME:
        # Light Ball doubles move POWER (both categories), not Special Attack.
        base_power *= 2

    if base_power is None or base_power == 0:
        return None

    if attacking_ability == gen_four_const.NORMALIZE_ABILITY:
        move_type = const.TYPE_NORMAL

    if move.name == gen_four_const.JUDGMENT_MOVE_NAME:
        new_type = gen_four_const.PLATE_TYPE_LOOKUP.get(attacking_pkmn.held_item)
        if new_type:
            move_type = new_type

    if move.name == gen_four_const.PUNISHMENT_MOVE_NAME:
        num_buffs = 0
        for cur_stage in [
            defending_stage_modifiers.attack_stage,
            defending_stage_modifiers.defense_stage,
            defending_stage_modifiers.special_attack_stage,
            defending_stage_modifiers.special_defense_stage,
            defending_stage_modifiers.speed_stage,
            defending_stage_modifiers.accuracy_stage,
            defending_stage_modifiers.evasion_stage,
        ]:
            if cur_stage > 0:
                num_buffs += cur_stage

        base_power = min(200, 60 + (num_buffs * 20))

    is_scrappy_active = (
        (defending_species.first_type == const.TYPE_GHOST or defending_species.second_type == const.TYPE_GHOST) and
        (move_type == const.TYPE_NORMAL or move_type == const.TYPE_FIGHTING) and
        attacking_ability == gen_four_const.SCRAPPY_ABILITY
    )

    ignore_ground_immunity = (
        (defending_species.first_type == const.TYPE_FLYING or defending_species.second_type == const.TYPE_FLYING) and
        move_type == const.TYPE_GROUND and
        (defending_field.gravity or defending_field.roost)
    )

    ignore_dark_immunity = (
        (defending_species.first_type == const.TYPE_DARK or defending_species.second_type == const.TYPE_DARK) and
        move_type == const.TYPE_PSYCHIC and
        defending_field.miracle_eye
    )

    if not is_no_type_effects:
        if (
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
            defending_ability == gen_four_const.LEVITATE_ABILITY and
            move_type == const.TYPE_GROUND and
            (not ignore_ground_immunity)
        ):
            return None
        elif (
            defending_ability == gen_four_const.DAMP_ABILITY and
            (
                move.name == const.SELFDESTRUCT_MOVE_NAME or
                move.name == const.EXPLOSION_MOVE_NAME
            )
        ):
            return None
        elif (
            (
                defending_ability == gen_four_const.VOLT_ABOSRB_ABILITY or
                defending_ability == gen_four_const.MOTOR_DRIVE_ABILITY
            ) and
            move_type == const.TYPE_ELECTRIC
        ):
            # NOTE: Lightning Rod does NOT absorb in gen 4 -- it only redirects
            # single-target Electric moves in doubles, which this single-target
            # calculator doesn't model. It must not grant immunity.
            return None
        elif (
            defending_ability == gen_four_const.WATER_ABSORB_ABILITY and
            move_type == const.TYPE_WATER
        ):
            return None
        elif (
            defending_ability == gen_four_const.FLASH_FIRE_ABILITY and
            move_type == const.TYPE_FIRE
        ):
            return None
        elif (
            defending_ability == gen_four_const.DRY_SKIN_ABILITY and
            move_type == const.TYPE_WATER
        ):
            return None
        elif (
            defending_ability == gen_four_const.SOUNDPROOF_ABILITY and
            move.name in gen_four_const.SOUND_MOVES
        ):
            return None
        elif defending_ability == gen_four_const.WONDER_GUARD_ABILITY:
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
            move_type == const.TYPE_GROUND
        ):
            return None

    # Super Fang: half the target's current HP (min 1); no crit/STAB/roll, but
    # ordinary Normal-type immunity (Ghost) still applies -- handled above.
    if move.name == gen_four_const.SUPER_FANG_MOVE_NAME:
        return damage_calc.DamageRange({max(defending_pkmn.cur_stats.hp // 2, 1): 1})

    # special move interactions
    if const.FLAVOR_FIXED_DAMAGE in move.attack_flavor:
        return damage_calc.DamageRange({base_power: 1})
    elif const.FLAVOR_LEVEL_DAMAGE in move.attack_flavor:
        return damage_calc.DamageRange({attacking_pkmn.level: 1})
    elif const.FLAVOR_PSYWAVE in move.attack_flavor:
        # damage = level*(r+5)//10, r uniform on 0..10, min 1 (11 equiprobable values)
        psywave_vals = {}
        for r in range(11):
            dmg = max(1, (attacking_pkmn.level * (r + 5)) // 10)
            psywave_vals[dmg] = psywave_vals.get(dmg, 0) + 1
        return damage_calc.DamageRange(psywave_vals)

    # kept for the multi-hit recursion below: the non-crit hits of a multi-hit move
    # must use the ORIGINAL stage modifiers, not these crit-adjusted ones
    original_attacking_stage_modifiers = attacking_stage_modifiers
    original_defending_stage_modifiers = defending_stage_modifiers

    # when a crit occurs, ignore only the negative stage of the attacking stat actually
    # used by this move's category, and only the positive stage of the defending stat
    # actually used -- not every stage on the mon (bug: this used to wipe all stages,
    # and checked the wrong category's stats entirely).
    if is_crit:
        if move.category == const.CATEGORY_PHYSICAL:
            if attacking_stage_modifiers.attack_stage < 0:
                attacking_stage_modifiers = attacking_stage_modifiers.apply_stat_mod(
                    [(const.ATK, -attacking_stage_modifiers.attack_stage)])
            if defending_stage_modifiers.defense_stage > 0:
                defending_stage_modifiers = defending_stage_modifiers.apply_stat_mod(
                    [(const.DEF, -defending_stage_modifiers.defense_stage)])
        else:
            if attacking_stage_modifiers.special_attack_stage < 0:
                attacking_stage_modifiers = attacking_stage_modifiers.apply_stat_mod(
                    [(const.SPA, -attacking_stage_modifiers.special_attack_stage)])
            if defending_stage_modifiers.special_defense_stage > 0:
                defending_stage_modifiers = defending_stage_modifiers.apply_stat_mod(
                    [(const.SPD, -defending_stage_modifiers.special_defense_stage)])

    if attacking_battle_stats is None:
        attacking_battle_stats = attacking_pkmn.get_battle_stats(attacking_stage_modifiers, mon_field=attacking_field)
    else:
        # caller-supplied stats may be reused across multiple calls (e.g. a crit and
        # a non-crit call sharing the same precomputed StatBlock); the item branches
        # below mutate stats in place, so copy first to avoid corrupting the caller's copy
        attacking_battle_stats = copy.copy(attacking_battle_stats)

    if defending_battle_stats is None:
        defending_battle_stats = defending_pkmn.get_battle_stats(defending_stage_modifiers, mon_field=defending_field)
    else:
        defending_battle_stats = copy.copy(defending_battle_stats)

    # OHKO moves: damage = target's current HP; the move fails outright (no partial
    # hit chance beyond the accuracy roll handled in get_move_accuracy) if the user
    # isn't at least as fast as the target, or the target has Sturdy.
    if move.name in gen_four_const.OHKO_MOVE_NAMES:
        if defending_ability == gen_four_const.STURDY_ABILITY:
            return None
        if attacking_battle_stats.speed < defending_battle_stats.speed:
            return None
        return damage_calc.DamageRange({defending_pkmn.cur_stats.hp: 1})

    # Handle base_power and move_type override cases first
    if move.name == gen_four_const.MAGNITUDE_MOVE_NAME:
        if gen_four_const.MAGNITUDE_4 in custom_move_data:
            base_power = 10
        elif gen_four_const.MAGNITUDE_5 in custom_move_data:
            base_power = 30
        elif gen_four_const.MAGNITUDE_6 in custom_move_data:
            base_power = 50
        elif gen_four_const.MAGNITUDE_7 in custom_move_data:
            base_power = 70
        elif gen_four_const.MAGNITUDE_8 in custom_move_data:
            base_power = 90
        elif gen_four_const.MAGNITUDE_9 in custom_move_data:
            base_power = 110
        elif gen_four_const.MAGNITUDE_10 in custom_move_data:
            base_power = 150
        if gen_four_const.DIG_BONUS in custom_move_data:
            base_power *= 2
    elif move.name in [const.FLAIL_MOVE_NAME, const.REVERSAL_MOVE_NAME]:
        if gen_four_const.FLAIL_FULL_HP in custom_move_data:
            base_power = 20
        elif gen_four_const.FLAIL_HALF_HP in custom_move_data:
            base_power = 40
        elif gen_four_const.FLAIL_QUARTER_HP in custom_move_data:
            base_power = 80
        elif gen_four_const.FLAIL_TEN_PERCENT_HP in custom_move_data:
            base_power = 100
        elif gen_four_const.FLAIL_FIVE_PERCENT_HP in custom_move_data:
            base_power = 150
        elif gen_four_const.FLAIL_MIN_HP in custom_move_data:
            base_power = 200
    elif move.name in (gen_four_const.RETURN_MOVE_NAME, gen_four_const.FRUSTRATION_MOVE_NAME):
        try:
            base_power = int(custom_move_data)
        except Exception as e:
            logger.warning(f"Failed to convert {move.name} power to an int: {custom_move_data}")
    elif move.name == gen_four_const.ERUPTION_MOVE_NAME:
        try:
            base_power = math.floor(base_power * int(custom_move_data) / 100.0)
        except Exception as e:
            logger.warning(f"Failed to convert return move power to an int: {custom_move_data}")
    elif move.name == gen_four_const.WATER_SPOUT_MOVE_NAME:
        try:
            base_power = math.floor(base_power * int(custom_move_data) / 100.0)
        except Exception as e:
            logger.warning(f"Failed to convert return move power to an int: {custom_move_data}")
    elif (
        move.name == gen_four_const.CRUSH_GRIP_MOVE_NAME or
        move.name == gen_four_const.WRING_OUT_MOVE_NAME
    ):
        try:
            base_power = 1 + ((120 * int(custom_move_data)) // 100)
        except Exception as e:
            logger.warning(f"Failed to convert return move power to an int: {custom_move_data}")
    elif move.name == gen_four_const.GYRO_BALL_MOVE_NAME:
        base_power = 1 + ((25 * defending_battle_stats.speed) // max(attacking_battle_stats.speed, 1))
        base_power = min(base_power, 150)
    elif move.name == gen_four_const.TRUMP_CARD_MOVE_NAME:
        base_power = {"4+": 40, "3": 50, "2": 60, "1": 80, "0": 200}.get(custom_move_data, 40)
    elif move.name in [gen_four_const.LOW_KICK_MOVE_NAME, gen_four_const.GRASS_KNOW_MOVE_NAME]:
        if defending_species.weight is None:
            base_power = 20
            logger.warning(f"Undefined weight for species: {defending_species.name}")
        else:
            weight_hg = round(defending_species.weight * 10)
            if weight_hg <= 100:
                base_power = 20
            elif weight_hg <= 250:
                base_power = 40
            elif weight_hg <= 500:
                base_power = 60
            elif weight_hg <= 1000:
                base_power = 80
            elif weight_hg <= 2000:
                base_power = 100
            else:
                base_power = 120
    elif move.name == gen_four_const.ROLLOUT_MOVE_NAME or move.name == gen_four_const.ICE_BALL_MOVE_NAME:
        if "DefenseCurl" in custom_move_data:
            num_turns = 5
            defense_curl_bonus = True
        else:
            try:
                num_turns = int(custom_move_data)
            except ValueError:
                num_turns = 1
            defense_curl_bonus = False
        num_turns = min(max(num_turns, 1), 5)
        base_power = 30 * (2 ** (num_turns - 1))
        if defense_curl_bonus:
            base_power *= 2
    elif move.name == gen_four_const.FURY_CUTTER_MOVE_NAME:
        try:
            num_turns = min(int(custom_move_data), 5)
        except (ValueError, TypeError):
            num_turns = 1
        base_power = 10 * (2 ** (num_turns - 1))
    elif move.name == gen_four_const.SPIT_UP_MOVE_NAME:
        try:
            base_power = 100 * int(custom_move_data)
        except (ValueError, TypeError):
            base_power = 100

    if move.name in (
        gen_four_const.GUST_MOVE_NAME,
        gen_four_const.TWISTER_MOVE_NAME,
        gen_four_const.SURF_MOVE_NAME,
        gen_four_const.WHIRLPOOL_MOVE_NAME,
        gen_four_const.EARTHQUAKE_MOVE_NAME,
        gen_four_const.PURSUIT_MOVE_NAME,
        gen_four_const.STOMP_MOVE_NAME,
        gen_four_const.FACADE_MOVE_NAME,
        gen_four_const.SMELLING_SALT_MOVE_NAME,
        gen_four_const.REVENGE_MOVE_NAME,
        gen_four_const.ASSURANCE_MOVE_NAME,
        gen_four_const.AVALANCHE_MOVE_NAME,
        gen_four_const.BRINE_MOVE_NAME,
        gen_four_const.PAYBACK_MOVE_NAME,
        gen_four_const.WAKE_UP_SLAP_MOVE_NAME,
    ) and custom_move_data and (gen_four_const.NO_BONUS not in custom_move_data):
        base_power *= 2

    # NOTE: for now, just ignoring the "edge case" of: what if the mon for mon-specific unique items has klutz?
    # it never occurs in normal gameplay, and would require a hack. so, wtv
    if (
        attacking_pkmn.name in (gen_four_const.MAROWAK_NAME, gen_four_const.CUBONE_NAME) and
        attacking_pkmn.held_item == gen_four_const.THICK_CLUB_NAME
    ):
        attacking_battle_stats.attack *= 2
    elif attacking_pkmn.name == gen_four_const.PIKACHU_NAME and attacking_pkmn.held_item == gen_four_const.LIGHT_BALL_NAME:
        # Light Ball doubles Pikachu's move POWER (both categories), not its
        # Special Attack stat -- handled via base_power below instead.
        pass
    elif attacking_pkmn.name == gen_four_const.CLAMPERL_NAME and attacking_pkmn.held_item == gen_four_const.DEEP_SEA_TOOTH_NAME:
        attacking_battle_stats.special_attack *= 2
    elif attacking_pkmn.name == gen_four_const.CLAMPERL_NAME and attacking_pkmn.held_item == gen_four_const.DEEP_SEA_SCALE_NAME:
        attacking_battle_stats.special_defense *= 2
    elif (
        (attacking_pkmn.name == gen_four_const.LATIOS_NAME or attacking_pkmn.name == gen_four_const.LATIAS_NAME) and
        attacking_pkmn.held_item == gen_four_const.SOULD_DEW_NAME
    ):
        attacking_battle_stats.special_attack = math.floor(attacking_battle_stats.special_attack * 1.5)
        attacking_battle_stats.special_defense = math.floor(attacking_battle_stats.special_defense * 1.5)

    if (
        (defending_pkmn.name == gen_four_const.LATIOS_NAME or defending_pkmn.name == gen_four_const.LATIAS_NAME) and
        defending_pkmn.held_item == gen_four_const.SOULD_DEW_NAME
    ):
        defending_battle_stats.special_attack = math.floor(defending_battle_stats.special_attack * 1.5)
        defending_battle_stats.special_defense = math.floor(defending_battle_stats.special_defense * 1.5)
    elif defending_pkmn.name == gen_four_const.CLAMPERL_NAME and defending_pkmn.held_item == gen_four_const.DEEP_SEA_SCALE_NAME:
        defending_battle_stats.special_defense *= 2
    elif defending_pkmn.name == gen_four_const.DITTO_NAME and defending_pkmn.held_item == gen_four_const.METAL_POWDER_NAME:
        # this should only apply while transformed... but also like, we don't support transforming in the app rn lmao
        defending_battle_stats.defense *= 2
    
    if attacking_ability == gen_four_const.HUSTLE_ABILITY:
        attacking_battle_stats.attack = math.floor(attacking_battle_stats.attack * 1.5)

    if (
        attacking_ability == gen_four_const.HUGE_POWER_ABILITY or
        attacking_ability == gen_four_const.PURE_POWER_ABILITY
    ):
        attacking_battle_stats.attack = math.floor(attacking_battle_stats.attack * 2)
    
    if (
        attacking_pkmn.held_item == gen_four_const.CHOICE_BAND_NAME and
        attacking_ability != gen_four_const.KLUTZ_ABILITY
    ):
        attacking_battle_stats.attack = math.floor(attacking_battle_stats.attack * 1.5)

    if (
        attacking_pkmn.held_item == gen_four_const.CHOICE_SPECS_NAME and
        attacking_ability != gen_four_const.KLUTZ_ABILITY
    ):
        attacking_battle_stats.special_attack = math.floor(attacking_battle_stats.special_attack * 1.5)
    
    if defending_ability == gen_four_const.THICK_FAT_ABILITY and move_type in [const.TYPE_FIRE, const.TYPE_ICE]:
        # oddity: this is technically how the actual code does it, despite it being a bit weird
        # When thick fat is applicable, it debuffs the special attack (technically before applying stages, but wtv)
        attacking_battle_stats.special_attack = math.floor(attacking_battle_stats.special_attack / 2)
        attacking_battle_stats.attack = math.floor(attacking_battle_stats.attack / 2)

    if defending_ability == gen_four_const.HEATPROOF_ABILITY and move_type == const.TYPE_FIRE:
        # seems to reuse the same code as thick-fat (based on bulbapedia's description)
        attacking_battle_stats.special_attack = math.floor(attacking_battle_stats.special_attack / 2)
        attacking_battle_stats.attack = math.floor(attacking_battle_stats.attack / 2)
    
    # Flower Gift (sun only): the side WITH the ability boosts its own Attack;
    # the OTHER side's Special Defense is boosted (not the Flower Gift mon's own
    # Sp.Def, and not Sp.Atk on either side).
    if is_weather_active and weather == const.WEATHER_SUN:
        if attacking_ability == gen_four_const.FLOWER_GIFT_ABILITY:
            attacking_battle_stats.attack = math.floor(attacking_battle_stats.attack * 1.5)
        if defending_ability == gen_four_const.FLOWER_GIFT_ABILITY:
            defending_battle_stats.special_defense = math.floor(defending_battle_stats.special_defense * 1.5)
        if attacking_ability == gen_four_const.SOLAR_POWER_ABILITY:
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
    if defending_ability == gen_four_const.MARVEL_SCALE_ABILITY and False:
        defending_battle_stats.defense = math.floor(defending_battle_stats.defense * 1.5)
    if attacking_ability == gen_four_const.GUTS_ABILITY and False:
        attacking_battle_stats.attack = math.floor(attacking_battle_stats.attack * 1.5)
    if attacking_ability == gen_four_const.OVERGROW_ABILITY and move_type == const.TYPE_GRASS and False:
        base_power = math.floor(base_power * 1.5)
    if attacking_ability == gen_four_const.BLAZE_ABILITY and move_type == const.TYPE_FIRE and False:
        base_power = math.floor(base_power * 1.5)
    if attacking_ability == gen_four_const.TORRENT_ABILITY and move_type == const.TYPE_WATER and False:
        base_power = math.floor(base_power * 1.5)
    if attacking_ability == gen_four_const.SWARM_ABILITY and move_type == const.TYPE_BUG and False:
        base_power = math.floor(base_power * 1.5)
    
    if move.category == const.CATEGORY_SPECIAL:
        attacking_stat = attacking_battle_stats.special_attack
        defending_stat = defending_battle_stats.special_defense
        if defending_field.light_screen and not is_crit and move.name != gen_four_const.BRICK_BREAK_MOVE_NAME:
            screen_active = True
        else:
            screen_active = False
    else:
        attacking_stat = attacking_battle_stats.attack
        defending_stat = defending_battle_stats.defense
        if defending_field.reflect and not is_crit and move.name != gen_four_const.BRICK_BREAK_MOVE_NAME:
            screen_active = True
        else:
            screen_active = False
    
    if  move.name == const.EXPLOSION_MOVE_NAME or move.name == const.SELFDESTRUCT_MOVE_NAME:
        defending_stat = max(math.floor(defending_stat / 2), 1)

    is_stab = (attacking_mon_first_type == move_type) or (attacking_mon_second_type == move_type)
    if move.name == const.FUTURE_SIGHT_MOVE_NAME:
        is_stab = False

    if (
        held_item_boost_table.get(attacking_pkmn.held_item) == move_type and
        attacking_ability != gen_four_const.KLUTZ_ABILITY
    ):
        attacking_stat = math.floor(attacking_stat * 1.2)
    elif (
        attacking_pkmn.name == gen_four_const.DIALGA_NAME and
        attacking_pkmn.held_item == gen_four_const.ADAMANT_ORB_NAME and
        (move_type == const.TYPE_DRAGON or move_type == const.TYPE_STEEL)
    ):
        attacking_stat = math.floor(attacking_stat * 1.2)
    elif (
        attacking_pkmn.name == gen_four_const.PALKIA_NAME and
        attacking_pkmn.held_item == gen_four_const.LUSTROUS_ORB_NAME and
        (move_type == const.TYPE_DRAGON or move_type == const.TYPE_WATER)
    ):
        attacking_stat = math.floor(attacking_stat * 1.2)
    elif (
        attacking_pkmn.name == gen_four_const.GIRATINA_NAME and
        attacking_pkmn.held_item == gen_four_const.GRISEOUS_ORB_NAME and
        (move_type == const.TYPE_DRAGON or move_type == const.TYPE_GHOST)
    ):
        attacking_stat = math.floor(attacking_stat * 1.2)

    if attacking_pkmn.held_item == gen_four_const.MUSCLE_BAND_NAME and move.category == const.CATEGORY_PHYSICAL:
        attacking_stat = math.floor(attacking_stat * 1.1)
    elif attacking_pkmn.held_item == gen_four_const.WISE_GLASSES_NAME and move.category == const.CATEGORY_SPECIAL:
        attacking_stat = math.floor(attacking_stat * 1.1)

    # begin actual formula
    temp = 2 * attacking_pkmn.level
    temp = math.floor(temp / 5) + 2

    temp *= base_power
    temp *= attacking_stat
    temp = math.floor(temp / defending_stat)

    temp = math.floor(temp / 50)

    # start accounting for multipliers
    if screen_active:
        if is_double_battle:
            # game: *2/3 in doubles (assuming 2 alive defenders), not /2
            temp = max(1, (temp * 2) // 3)
        else:
            temp = math.floor(temp / 2)

    if is_double_battle and move.targeting in (gen_four_const.TARGETING_ALL_FOES, gen_four_const.TARGETING_OTHERS):
        # game: *3/4 spread reduction; moves.json uses "All Foes"/"Others", never the
        # old "target_both_enemies" string this used to (and never actually) match
        temp = (temp * 3) // 4

    weather_boost = False
    weather_penalty = False
    if is_weather_active:
        if weather == const.WEATHER_RAIN:
            weather_boost = (move_type == const.TYPE_WATER)
            weather_penalty = (
                move_type == const.TYPE_FIRE or
                move.name == const.SOLAR_BEAM_MOVE_NAME
            )
        elif weather == const.WEATHER_SUN:
            weather_boost = (move_type == const.TYPE_FIRE)
            weather_penalty = (move_type == const.TYPE_WATER)
        elif weather != const.WEATHER_NONE:
            weather_penalty = (
                move.name == const.SOLAR_BEAM_MOVE_NAME
        )

    if weather_boost:
        temp = math.floor(temp * 1.5)
    elif weather_penalty:
        temp = math.floor(temp * 0.5)
    
    # TODO: when we support flash fire, support goes here
    flash_fire_activated = False
    if flash_fire_activated:
        temp = math.floor(temp * 1.5)

    temp += 2

    if (
        is_crit and
        move.name not in (const.DOOM_DESIRE_MOVE_NAME, const.FUTURE_SIGHT_MOVE_NAME) and
        defending_ability not in [gen_four_const.BATTLE_ARMOR_ABILITY, gen_four_const.SHELL_ARMOR_ABILITY]
    ):
        if attacking_ability == gen_four_const.SNIPER_ABILITY:
            temp *= 3
        else:
            temp *= 2

    # NOTE: Rollout/Ice Ball/Fury Cutter/Spit Up/the double-power "bonus" moves and
    # Rage's old damage multiplier used to live here, applied to `temp` after the
    # crit multiplier. They're now folded into `base_power` up front (matching the
    # game, which resolves the move's real power before Technician/items even run);
    # Rage in gen 4 has no damage multiplier at all -- it just raises the user's
    # Attack stage when hit, which the Attack-stage dropdowns already express.

    if attacking_pkmn.held_item == gen_four_const.LIFE_ORB_NAME and attacking_ability != gen_four_const.KLUTZ_ABILITY:
        temp = math.floor(temp * 1.3)

    # TODO: pretty much wholly outside the use case ofthis app, but here just in case
    is_helping_hand_active = False
    if is_helping_hand_active:
        temp = math.floor(temp * 1.5)

    # TODO: one day we might need to support this
    is_charge_active = False
    if is_charge_active and move_type == const.TYPE_ELECTRIC:
        temp *= 2

    if is_stab and not is_no_type_effects:
        if attacking_ability == gen_four_const.ADAPTABILITY_ABILITY:
            temp = math.floor(temp * 2)
        else:
            temp = math.floor(temp * 1.5)

    net_effectiveness = 1.0
    if not is_no_type_effects:
        for test_type in type_chart.get(move_type):
            if test_type == defending_species.first_type or test_type == defending_species.second_type:
                effectiveness = type_chart.get(move_type).get(test_type)
                if effectiveness == const.SUPER_EFFECTIVE:
                    temp *= 2
                    net_effectiveness *= 2
                elif effectiveness == const.NOT_VERY_EFFECTIVE:
                    temp = math.floor(temp / 2)
                    net_effectiveness *= 0.5

        # Filter/Solid Rock/Expert Belt/Tinted Lens key off the NET effectiveness
        # (SE and NVE cancel out), applied once -- not once per matching type.
        if net_effectiveness > 1:
            if defending_ability in (gen_four_const.FILTER_ABILITY, gen_four_const.SOLID_ROCK_ABILITY):
                temp = max(1, (temp * 3) // 4)
            elif attacking_pkmn.held_item == gen_four_const.EXPERT_BELT_NAME:
                temp = math.floor(temp * 1.2)
        elif 0 < net_effectiveness < 1:
            if attacking_ability == gen_four_const.TINTED_LENS_ABILITY:
                temp *= 2

    if (
        defending_ability == gen_four_const.DRY_SKIN_ABILITY and
        move_type == const.TYPE_FIRE
    ):
        temp = math.floor(temp * 1.25)

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
                other_damage = calculate_gen_four_damage(
                    attacking_pkmn,
                    attacking_species,
                    move,
                    defending_pkmn,
                    defending_species,
                    type_chart,
                    held_item_boost_table,
                    original_attacking_stage_modifiers,
                    original_defending_stage_modifiers,
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
