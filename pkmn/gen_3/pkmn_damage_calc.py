import copy
import math
import logging
from typing import Dict, List

from pkmn import universal_data_objects, damage_calc
from pkmn.gen_3.data_objects import GenThreeBadgeList, get_hidden_power_type, get_hidden_power_base_power, modify_stat_by_stage
from utils.constants import const
from pkmn.gen_3.gen_three_constants import gen_three_const


logger = logging.getLogger(__name__)


MIN_RANGE = 85
MAX_RANGE = 100
NUM_ROLLS = MAX_RANGE - MIN_RANGE + 1


def get_crit_rate(cur_mon:universal_data_objects.EnemyPkmn, move:universal_data_objects.Move, custom_move_data:str):
    # NOTE: the game also boosts crit stage for Scope Lens (+1), Lucky Punch on Chansey (+2),
    # Stick on Farfetch'd (+2) and Focus Energy (+2), and forces the rate to 0 against
    # Battle Armor / Shell Armor. None of those are implementable here without adding a
    # `defending_pkmn` parameter to this function (and to the shared CurrentGen interface
    # and every other gen's implementation, plus the controller call site) -- out of scope
    # for a gen-3-only change. See gen_3_findings.md bug #25.
    stage = 0
    if const.FLAVOR_HIGH_CRIT in move.attack_flavor:
        stage += 1
    if move.name == gen_three_const.NATURE_POWER_MOVE_NAME and custom_move_data is not None and gen_three_const.LONG_GRASS_TERRAIN in custom_move_data:
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


def get_move_accuracy(pkmn:universal_data_objects.EnemyPkmn, move:universal_data_objects.Move, custom_move_data:str, defending_pkmn:universal_data_objects.EnemyPkmn, weather:str, special_types:List[str]):
    # OHKO moves ignore all the normal accuracy machinery: chance = acc + (Latk - Ldef), hit
    # iff Latk >= Ldef (Cmd_tryKO). Also blocked outright by Sturdy.
    if move.name in gen_three_const.OHKO_MOVE_NAMES:
        if defending_pkmn.ability == gen_three_const.STURDY_ABILITY:
            return 0
        if pkmn.level < defending_pkmn.level:
            return 0
        return max(0, move.accuracy + (pkmn.level - defending_pkmn.level) - 1)

    if move.name == gen_three_const.NATURE_POWER_MOVE_NAME:
        custom_move_data = custom_move_data or ""
        if gen_three_const.TALL_GRASS_TERRAIN in custom_move_data:
            result = 75
        elif gen_three_const.LONG_GRASS_TERRAIN in custom_move_data:
            result = 95
        elif gen_three_const.UNDERWATER_TERRAIN in custom_move_data:
            result = 80
        elif gen_three_const.ROCK_TERRAIN in custom_move_data:
            result = 90
        else:
            # Plain/Sand/Cave/Pond/Sea Water all resolve to a 100%-accuracy move
            result = 100
    else:
        result = move.accuracy

    weather_has_effect = damage_calc.is_weather_active(pkmn.ability, defending_pkmn.ability, weather)

    # Certain weather makes specific moves bypass accuracy checks entirely
    # (they always hit, unaffected by accuracy/evasion). Gen 3 has no Blizzard/hail rule
    # (that's a gen 4/5 mechanic) and Thunder is only guaranteed to hit in rain -- in sun
    # its accuracy drops to 50, it isn't a guaranteed miss/hit.
    if move.name == gen_three_const.THUNDER_MOVE_NAME and weather_has_effect:
        if weather == const.WEATHER_RAIN:
            return None
        elif weather == const.WEATHER_SUN:
            result = 50

    if result is None:
        return None

    if pkmn.ability == gen_three_const.COMPOUND_EYES_ABILITY:
        result = math.floor(result * 1.3)
    elif pkmn.ability == gen_three_const.HUSTLE_ABILITY:
        is_physical = False
        if move.name == gen_three_const.NATURE_POWER_MOVE_NAME:
            is_physical = any(t in custom_move_data for t in (gen_three_const.PLAIN_TERRAIN, gen_three_const.SAND_TERRAIN, gen_three_const.CAVE_TERRAIN, gen_three_const.ROCK_TERRAIN))
        elif move.name == const.HIDDEN_POWER_MOVE_NAME:
            is_physical = get_hidden_power_type(pkmn.dvs) not in special_types
        elif move.move_type not in special_types:
            is_physical = True

        if is_physical:
            result = math.floor(result * 3277 / 4096)

    if defending_pkmn.ability == gen_three_const.SAND_VEIL_ABILITY and weather == const.WEATHER_SANDSTORM and weather_has_effect:
        result = math.floor(result * 3277 / 4096)

    # The game only ever clamps the final byte (to 255/256 == effectively 100%); do the same
    # here instead of clamping Compound Eyes' 1.3x in isolation, which previously masked a
    # later Sand Veil reduction (bug #21).
    return min(result, 100)


def calculate_gen_three_damage(
    attacking_pkmn:universal_data_objects.EnemyPkmn,
    attacking_species:universal_data_objects.PokemonSpecies,
    move:universal_data_objects.Move,
    defending_pkmn:universal_data_objects.EnemyPkmn,
    defending_species:universal_data_objects.PokemonSpecies,
    special_types:List[str],
    type_chart:Dict[str, Dict[str, str]],
    held_item_boost_table:Dict[str, str],
    attacking_stage_modifiers:universal_data_objects.StageModifiers=None,
    defending_stage_modifiers:universal_data_objects.StageModifiers=None,
    is_crit:bool=False,
    defender_has_light_screen:bool=False,
    defender_has_reflect:bool=False,
    custom_move_data:str="",
    weather:str=const.WEATHER_NONE,
    is_double_battle:bool=False,
    attacking_battle_stats:universal_data_objects.StatBlock=None,
    defending_battle_stats:universal_data_objects.StatBlock=None,
):
    # NOTE: resolved up front because everything that keys off the weather (Forecast,
    # Weather Ball's type) has to be settled before any type-based immunity check
    is_weather_active = damage_calc.is_weather_active(attacking_pkmn.ability, defending_pkmn.ability, weather)

    # Forecast retypes Castform to match the weather, which changes STAB when it's
    # attacking and type effectiveness when it's defending
    attacking_species = damage_calc.apply_forecast(attacking_species, attacking_pkmn.ability, weather, is_weather_active)
    defending_species = damage_calc.apply_forecast(defending_species, defending_pkmn.ability, weather, is_weather_active)

    # Wonder Guard's typecalc still runs for these fixed/level-damage moves even though
    # they skip the normal formula entirely -- the shared override below only checks
    # plain type immunity, not Wonder Guard's "must be SE and not also NVE" rule (bug #26).
    if (
        defending_pkmn.ability == gen_three_const.WONDER_GUARD_ABILITY and
        move.name in (const.DRAGON_RAGE_MOVE_NAME, "SonicBoom", "Seismic Toss", "Night Shade")
    ):
        first_effectiveness = type_chart.get(move.move_type, {}).get(defending_species.first_type)
        second_effectiveness = type_chart.get(move.move_type, {}).get(defending_species.second_type)
        if first_effectiveness != const.SUPER_EFFECTIVE and second_effectiveness != const.SUPER_EFFECTIVE:
            return None

    # Special-damage moves (Dragon Rage 40, Sonic Boom 20, Seismic Toss/Night Shade = level)
    # don't store their damage in base_power. Type immunity IS respected here (gen 2+).
    special_override = damage_calc.get_special_damage_override(move, attacking_pkmn, defending_species, type_chart)
    if special_override is not None:
        return special_override

    if move.name == const.HIDDEN_POWER_MOVE_NAME:
        move_type = get_hidden_power_type(attacking_pkmn.dvs)
        base_power = get_hidden_power_base_power(attacking_pkmn.dvs)
    else:
        move_type = move.move_type
        base_power = move.base_power

    if base_power is None or base_power == 0:
        return None

    # Weather Ball takes on the weather's type. Has to happen before the immunity checks
    # below, otherwise e.g. a sun-boosted (Fire) Weather Ball would still be treated as
    # Normal, and read as immune against a Ghost type. The power doubling is a
    # dmgMultiplier applied after +2/crit (bug #8), not a base-power change -- see the
    # double_damage handling further down.
    if move.name == const.WEATHER_BALL_MOVE_NAME and is_weather_active:
        move_type = damage_calc.get_weather_ball_type(weather, is_weather_active, default_type=move_type)

    # Future Sight / Doom Desire never run typecalc at all when they land (the hit was
    # computed and stored on the turn the move was used); Struggle's `stab` command
    # returns before typecalc too. None of the three get STAB, type effectiveness, or any
    # immunity (Wonder Guard included) -- bug #26.
    skip_type_and_immunity = move.name in (const.FUTURE_SIGHT_MOVE_NAME, const.DOOM_DESIRE_MOVE_NAME, const.STRUGGLE_MOVE_NAME, gen_three_const.BEAT_UP_MOVE_NAME)

    if not skip_type_and_immunity:
        if (
            type_chart.get(move_type).get(defending_species.first_type) == const.IMMUNE or
            type_chart.get(move_type).get(defending_species.second_type) == const.IMMUNE
        ):
            return None
        elif (
            defending_pkmn.ability == gen_three_const.LEVITATE_ABILITY and
            move_type == const.TYPE_GROUND
        ):
            return None
        elif (
            defending_pkmn.ability == gen_three_const.DAMP_ABILITY and
            (
                move.name == const.SELFDESTRUCT_MOVE_NAME or
                move.name == const.EXPLOSION_MOVE_NAME
            )
        ):
            return None
        elif (
            defending_pkmn.ability == gen_three_const.VOLT_ABOSRB_ABILITY and
            move_type == const.TYPE_ELECTRIC
        ):
            return None
        elif (
            defending_pkmn.ability == gen_three_const.WATER_ABSORB_ABILITY and
            move_type == const.TYPE_WATER
        ):
            return None
        elif (
            defending_pkmn.ability == gen_three_const.FLASH_FIRE_ABILITY and
            move_type == const.TYPE_FIRE
        ):
            return None
        elif (
            defending_pkmn.ability == gen_three_const.SOUNDPROOF_ABILITY and
            move.name in gen_three_const.SOUND_MOVE_NAMES
        ):
            return None
        elif defending_pkmn.ability == gen_three_const.WONDER_GUARD_ABILITY:
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

    # special move interactions
    if const.FLAVOR_FIXED_DAMAGE in move.attack_flavor:
        # Lightning Rod only redirects Electric moves in gen 3 doubles; it is not an
        # absorb/immunity (bug #6, moved off the general Volt-Absorb-style check above).
        # Wonder Guard/typecalc still runs for these (game: typecalc runs, blocks Shedinja
        # unless SE-and-not-NVE; bug #26), even though the general gate above already
        # returned for the True-immunity cases -- redo the Wonder Guard-only check here.
        if defending_pkmn.ability == gen_three_const.WONDER_GUARD_ABILITY:
            first_effectiveness = type_chart.get(move_type, {}).get(defending_species.first_type)
            second_effectiveness = type_chart.get(move_type, {}).get(defending_species.second_type)
            if first_effectiveness != const.SUPER_EFFECTIVE and second_effectiveness != const.SUPER_EFFECTIVE:
                return None
        return damage_calc.DamageRange({base_power: 1})
    elif const.FLAVOR_LEVEL_DAMAGE in move.attack_flavor:
        return damage_calc.DamageRange({attacking_pkmn.level: 1})
    elif const.FLAVOR_PSYWAVE in move.attack_flavor:
        # 11 equiprobable values level*(50+10k)/100, k=0..10 (Cmd_psywavedamageeffect)
        return damage_calc.DamageRange({math.floor(attacking_pkmn.level * (50 + 10 * k) / 100): 1 for k in range(11)})
    elif move.name == gen_three_const.SUPER_FANG_MOVE_NAME:
        return damage_calc.DamageRange({max(defending_pkmn.cur_stats.hp // 2, 1): 1})
    elif move.name in gen_three_const.OHKO_MOVE_NAMES:
        if defending_pkmn.ability == gen_three_const.STURDY_ABILITY:
            return None
        if attacking_pkmn.level < defending_pkmn.level:
            return None
        return damage_calc.DamageRange({defending_pkmn.cur_stats.hp: 1})
    elif move.name == gen_three_const.ENDEAVOR_MOVE_NAME:
        try:
            user_hp_pct = int(custom_move_data)
        except (TypeError, ValueError):
            user_hp_pct = 100
        user_hp = math.floor(attacking_pkmn.cur_stats.hp * user_hp_pct / 100)
        target_hp = defending_pkmn.cur_stats.hp
        if target_hp <= user_hp:
            return None
        return damage_calc.DamageRange({target_hp - user_hp: 1})
    elif (
        move.name == gen_three_const.TRIPLE_KICK_MOVE_NAME and
        not (custom_move_data or "").startswith(gen_three_const.TRIPLE_KICK_SENTINEL_PREFIX)
    ):
        # Each kick is base power 10*k with its own accuracy check, crit and random roll
        # (battle_scripts_1.s:1384-1425), not one hit multiplied by N (bug #4). Recurse
        # once per landed kick with a sentinel custom_move_data so the base-power-override
        # section below can set that kick's power without re-entering this branch.
        try:
            num_kicks = max(1, min(int(custom_move_data), 3))
        except (TypeError, ValueError):
            num_kicks = 1
        total = None
        for kick_num in range(1, num_kicks + 1):
            kick_result = calculate_gen_three_damage(
                attacking_pkmn, attacking_species, move, defending_pkmn, defending_species,
                special_types, type_chart, held_item_boost_table,
                attacking_stage_modifiers, defending_stage_modifiers,
                is_crit=(is_crit and kick_num == num_kicks),
                defender_has_light_screen=defender_has_light_screen,
                defender_has_reflect=defender_has_reflect,
                custom_move_data=f"{gen_three_const.TRIPLE_KICK_SENTINEL_PREFIX}{10 * kick_num}",
                weather=weather,
                is_double_battle=is_double_battle,
            )
            if kick_result is None:
                continue
            total = kick_result if total is None else total + kick_result
        return total
    elif move.name == gen_three_const.BEAT_UP_MOVE_NAME:
        # One hit per eligible party member, using that member's own base Attack/level vs
        # the target's base Defense -- no STAB, type effectiveness, stats/stages, items or
        # screens (Cmd_trydobeatup). This app has no party data to draw on, so every "hit"
        # approximates a party of identical mons using the user's own base Attack/level
        # (matching the game's real 1-mon-party/wild-mon case exactly; an approximation
        # for a full 6-mon party).
        try:
            num_hits = max(1, min(int(custom_move_data), 6))
        except (TypeError, ValueError):
            num_hits = 1
        beat_up_base = math.floor(2 * attacking_pkmn.level / 5) + 2
        pre_roll = math.floor(math.floor(beat_up_base * base_power * attacking_species.stats.attack / defending_species.stats.defense) / 50) + 2
        if is_crit:
            pre_roll *= 2
        hit_vals = {}
        for numerator in range(MIN_RANGE, MAX_RANGE + 1):
            cur_damage = max(math.floor((pre_roll * numerator) / MAX_RANGE), 1)
            hit_vals[cur_damage] = hit_vals.get(cur_damage, 0) + 1
        one_hit = damage_calc.DamageRange(hit_vals)
        total = one_hit
        for _ in range(1, num_hits):
            total = total + one_hit
        return total
    elif move.name in (gen_three_const.COUNTER_MOVE_NAME, gen_three_const.MIRROR_COAT_MOVE_NAME, gen_three_const.BIDE_MOVE_NAME):
        # No natural source for "damage the target dealt last turn" in this app; accept it
        # as a raw number via custom_move_data (no dropdown is wired up for this yet --
        # needs GUI/controller support to expose a free-form input).
        try:
            damage_taken = int(custom_move_data)
        except (TypeError, ValueError):
            return None
        if damage_taken <= 0:
            return None
        return damage_calc.DamageRange({damage_taken * 2: 1})

    # Handle base_power and move_type override cases first
    if move.name == gen_three_const.MAGNITUDE_MOVE_NAME:
        if gen_three_const.MAGNITUDE_4 in custom_move_data:
            base_power = 10
        elif gen_three_const.MAGNITUDE_5 in custom_move_data:
            base_power = 30
        elif gen_three_const.MAGNITUDE_6 in custom_move_data:
            base_power = 50
        elif gen_three_const.MAGNITUDE_7 in custom_move_data:
            base_power = 70
        elif gen_three_const.MAGNITUDE_8 in custom_move_data:
            base_power = 90
        elif gen_three_const.MAGNITUDE_9 in custom_move_data:
            base_power = 110
        elif gen_three_const.MAGNITUDE_10 in custom_move_data:
            base_power = 150
    elif move.name in [const.FLAIL_MOVE_NAME, const.REVERSAL_MOVE_NAME]:
        if gen_three_const.FLAIL_FULL_HP in custom_move_data:
            base_power = 20
        elif gen_three_const.FLAIL_HALF_HP in custom_move_data:
            base_power = 40
        elif gen_three_const.FLAIL_QUARTER_HP in custom_move_data:
            base_power = 80
        elif gen_three_const.FLAIL_TEN_PERCENT_HP in custom_move_data:
            base_power = 100
        elif gen_three_const.FLAIL_FIVE_PERCENT_HP in custom_move_data:
            base_power = 150
        elif gen_three_const.FLAIL_MIN_HP in custom_move_data:
            base_power = 200
    elif move.name in (gen_three_const.RETURN_MOVE_NAME, gen_three_const.FRUSTRATION_MOVE_NAME):
        try:
            base_power = int(custom_move_data)
        except Exception as e:
            logger.warning(f"Failed to convert return move power to an int: {custom_move_data}")
    elif move.name == gen_three_const.ERUPTION_MOVE_NAME:
        try:
            base_power = math.floor(base_power * int(custom_move_data) / 100.0)
        except Exception as e:
            logger.warning(f"Failed to convert return move power to an int: {custom_move_data}")
    elif move.name == gen_three_const.WATER_SPOUT_MOVE_NAME:
        try:
            base_power = math.floor(base_power * int(custom_move_data) / 100.0)
        except Exception as e:
            logger.warning(f"Failed to convert return move power to an int: {custom_move_data}")
    elif move.name == gen_three_const.PRESENT_MOVE_NAME:
        custom_move_data = custom_move_data or ""
        if gen_three_const.PRESENT_80 in custom_move_data:
            base_power = 80
        elif gen_three_const.PRESENT_120 in custom_move_data:
            base_power = 120
        else:
            base_power = 40
    elif move.name == gen_three_const.TRIPLE_KICK_MOVE_NAME:
        # Only ever reached via the sentinel recursion above -- sets this single kick's power.
        prefix = gen_three_const.TRIPLE_KICK_SENTINEL_PREFIX
        if custom_move_data and custom_move_data.startswith(prefix):
            base_power = int(custom_move_data[len(prefix):])
    elif move.name in (gen_three_const.ROLLOUT_MOVE_NAME, gen_three_const.ICE_BALL_MOVE_NAME):
        # Rollout/Ice Ball's power doubling-per-turn (and Defense Curl) is a base-power
        # change (gDynamicBasePower), not a final-damage multiplier (bug #1).
        try:
            rollout_turn = int(custom_move_data)
            defense_curl_active = False
        except (TypeError, ValueError):
            rollout_turn = 5
            defense_curl_active = True  # "5 + DefenseCurl"
        base_power = base_power * (2 ** (rollout_turn - 1)) * (2 if defense_curl_active else 1)
    elif move.name == gen_three_const.FURY_CUTTER_MOVE_NAME:
        # Counter caps at 5 (max power 10*2**4 = 160); also a base-power change, not a
        # final-damage multiplier (bug #3).
        try:
            fury_cutter_turn = min(int(custom_move_data), 5)
        except (TypeError, ValueError):
            fury_cutter_turn = 1
        base_power = base_power * (2 ** (fury_cutter_turn - 1))
    elif move.name == gen_three_const.NATURE_POWER_MOVE_NAME:
        custom_move_data = custom_move_data or ""
        if gen_three_const.PLAIN_TERRAIN in custom_move_data:
            base_power = 60
            move_type = const.TYPE_NORMAL
        elif gen_three_const.SAND_TERRAIN in custom_move_data:
            base_power = 100
            move_type = const.TYPE_GROUND
        elif gen_three_const.CAVE_TERRAIN in custom_move_data:
            base_power = 80
            move_type = const.TYPE_GHOST
        elif gen_three_const.ROCK_TERRAIN in custom_move_data:
            base_power = 75
            move_type = const.TYPE_ROCK
        elif gen_three_const.TALL_GRASS_TERRAIN in custom_move_data:
            # ugly, but wtv. This turns into stun spore, a status move
            return None
        elif gen_three_const.LONG_GRASS_TERRAIN in custom_move_data:
            base_power = 55
            move_type = const.TYPE_GRASS
        elif gen_three_const.POND_WATER_TERRAIN in custom_move_data:
            base_power = 65
            move_type = const.TYPE_WATER
        elif gen_three_const.SEA_WATER_TERRAIN in custom_move_data:
            base_power = 95
            move_type = const.TYPE_WATER
        elif gen_three_const.UNDERWATER_TERRAIN in custom_move_data:
            base_power = 120
            move_type = const.TYPE_WATER

    if attacking_stage_modifiers is None:
        attacking_stage_modifiers = universal_data_objects.StageModifiers()
    if defending_stage_modifiers is None:
        defending_stage_modifiers = universal_data_objects.StageModifiers()

    # Saved before the crit-only reset below so the multi-hit recursion at the
    # bottom of this function can pass the TRUE stage modifiers for its non-crit hits
    # instead of the (possibly zeroed) crit-adjusted ones (bug #14).
    original_attacking_stage_modifiers = attacking_stage_modifiers
    original_defending_stage_modifiers = defending_stage_modifiers

    # Gen 3 Rage has no damage multiplier at all -- it is a plain 20 BP hit whose ONLY
    # effect is +1 Attack stage each time the user is hit while it's active (bug #10).
    # That stage change is driven by the move's ATK+1-self `effects` entry through the
    # normal stat-stage-setup dropdown (like Metal Claw/Charge Beam), not a per-call
    # custom_move_data override here -- doing it that way lets the boost persist and
    # affect every other move in the matchup, not just Rage's own damage number.

    # Spit Up / Future Sight / Doom Desire never actually crit (their scripts never call
    # critcalc) even though the UI may still ask for their "crit" range -- treat that as
    # not-a-real-crit so the stage-zeroing and screen-drop rules below don't fire (bug #26).
    is_real_crit = is_crit and move.name not in (const.SPIT_UP_MOVE_NAME, const.FUTURE_SIGHT_MOVE_NAME, const.DOOM_DESIRE_MOVE_NAME)

    # when a crit occurs, always ignore negative modifiers for the attacking pokemon, and always ignore positive modifiers for the defensive pokemon
    if is_real_crit:
        if move_type in special_types:
            if attacking_stage_modifiers.special_attack_stage < 0:
                attacking_stage_modifiers = universal_data_objects.StageModifiers()
            if defending_stage_modifiers.special_defense_stage > 0:
                defending_stage_modifiers = universal_data_objects.StageModifiers()
        else:
            if attacking_stage_modifiers.attack_stage < 0:
                attacking_stage_modifiers = universal_data_objects.StageModifiers()
            if defending_stage_modifiers.defense_stage > 0:
                defending_stage_modifiers = universal_data_objects.StageModifiers()

    # Item/ability stat modifiers apply to the RAW (badge-boosted, unstaged) stat; the
    # stage multiplier is the LAST thing applied before the stat reaches the formula
    # (pokemon.c:3106-3232, gen_3_findings.md bug #9). When the caller supplies pre-baked
    # battle stats directly there's no raw value to re-derive the stage from, so that
    # legacy path keeps applying modifiers after the (already-included) stage.
    reorder_attacker_stage = attacking_battle_stats is None
    reorder_defender_stage = defending_battle_stats is None

    if attacking_battle_stats is None:
        attacking_battle_stats = attacking_pkmn.get_battle_stats(universal_data_objects.StageModifiers())
    if defending_battle_stats is None:
        defending_battle_stats = defending_pkmn.get_battle_stats(universal_data_objects.StageModifiers())

    if attacking_pkmn.held_item == gen_three_const.THICK_CLUB_NAME and attacking_pkmn.name in (gen_three_const.MAROWAK_NAME, gen_three_const.CUBONE_NAME):
        attacking_battle_stats.attack *= 2
    elif attacking_pkmn.name == gen_three_const.PIKACHU_NAME and attacking_pkmn.held_item == gen_three_const.LIGHT_BALL_NAME:
        attacking_battle_stats.special_attack *= 2
    elif attacking_pkmn.name == gen_three_const.CLAMPERL_NAME and attacking_pkmn.held_item == gen_three_const.DEEP_SEA_TOOTH_NAME:
        attacking_battle_stats.special_attack *= 2
    elif (
        (attacking_pkmn.name == gen_three_const.LATIOS_NAME or attacking_pkmn.name == gen_three_const.LATIAS_NAME) and
        attacking_pkmn.held_item == gen_three_const.SOULD_DEW_NAME
    ):
        attacking_battle_stats.special_attack = math.floor(attacking_battle_stats.special_attack * 1.5)

    if (
        (defending_pkmn.name == gen_three_const.LATIOS_NAME or defending_pkmn.name == gen_three_const.LATIAS_NAME) and
        defending_pkmn.held_item == gen_three_const.SOULD_DEW_NAME
    ):
        defending_battle_stats.special_defense = math.floor(defending_battle_stats.special_defense * 1.5)
    elif defending_pkmn.name == gen_three_const.CLAMPERL_NAME and defending_pkmn.held_item == gen_three_const.DEEP_SEA_SCALE_NAME:
        defending_battle_stats.special_defense *= 2
    elif defending_pkmn.name == gen_three_const.DITTO_NAME and defending_pkmn.held_item == gen_three_const.METAL_POWDER_NAME:
        # this should only apply while transformed... but also like, we don't support transforming in the app rn lmao
        defending_battle_stats.defense *= 2

    if attacking_pkmn.ability == gen_three_const.HUSTLE_ABILITY:
        attacking_battle_stats.attack = math.floor(attacking_battle_stats.attack * 1.5)

    if (
        attacking_pkmn.ability == gen_three_const.HUGE_POWER_ABILITY or
        attacking_pkmn.ability == gen_three_const.PURE_POWER_ABILITY
    ):
        attacking_battle_stats.attack = math.floor(attacking_battle_stats.attack * 2)

    if attacking_pkmn.held_item == gen_three_const.CHOICE_BAND_NAME:
        attacking_battle_stats.attack = math.floor(attacking_battle_stats.attack * 1.5)

    type_boost_type = held_item_boost_table.get(attacking_pkmn.held_item)
    if type_boost_type == move_type:
        if move_type in special_types:
            attacking_battle_stats.special_attack = math.floor(attacking_battle_stats.special_attack * 1.1)
        else:
            attacking_battle_stats.attack = math.floor(attacking_battle_stats.attack * 1.1)
    elif attacking_pkmn.held_item == gen_three_const.SEA_INCENSE_NAME and move_type == const.TYPE_WATER:
        # Sea Incense's boost param is 5, not the usual 10 (items.h HOLD_EFFECT_WATER_POWER)
        attacking_battle_stats.special_attack = math.floor(attacking_battle_stats.special_attack * 1.05)

    if defending_pkmn.ability == gen_three_const.THICK_FAT_ABILITY and move_type in [const.TYPE_FIRE, const.TYPE_ICE]:
        # oddity: this is technically how the actual code does it, despite it being a bit weird
        # When thick fat is applicable, it debuffs the special attack (technically before applying stages, but wtv)
        attacking_battle_stats.special_attack = math.floor(attacking_battle_stats.special_attack / 2)
        # I'm paranoid, since physical/special types are technically editable in a custom gen
        attacking_battle_stats.attack = math.floor(attacking_battle_stats.attack / 2)

    # NOTE: ignoring plus/minus abilities. They would modify special attack here, if ever relevant

    # TODO: bunch of abilities below that have a conditional activation. Need to figure out how to implement them properly
    # until then, they're just fully disabled
    if defending_pkmn.ability == gen_three_const.MARVEL_SCALE_ABILITY and False:
        defending_battle_stats.defense = math.floor(defending_battle_stats.defense * 1.5)
    if attacking_pkmn.ability == gen_three_const.GUTS_ABILITY and False:
        attacking_battle_stats.attack = math.floor(attacking_battle_stats.attack * 1.5)
    if attacking_pkmn.ability == gen_three_const.OVERGROW_ABILITY and move_type == const.TYPE_GRASS and False:
        base_power = math.floor(base_power * 1.5)
    if attacking_pkmn.ability == gen_three_const.BLAZE_ABILITY and move_type == const.TYPE_FIRE and False:
        base_power = math.floor(base_power * 1.5)
    if attacking_pkmn.ability == gen_three_const.TORRENT_ABILITY and move_type == const.TYPE_WATER and False:
        base_power = math.floor(base_power * 1.5)
    if attacking_pkmn.ability == gen_three_const.SWARM_ABILITY and move_type == const.TYPE_BUG and False:
        base_power = math.floor(base_power * 1.5)

    if move.name == const.EXPLOSION_MOVE_NAME or move.name == const.SELFDESTRUCT_MOVE_NAME:
        # Applied to the raw (unstaged) defence, before the stage multiplier (bug #9)
        defending_battle_stats.defense = max(math.floor(defending_battle_stats.defense / 2), 1)

    # The stage multiplier is the LAST stat modifier applied (pokemon.c:3224-3247)
    if reorder_attacker_stage:
        attacking_battle_stats.attack = modify_stat_by_stage(attacking_battle_stats.attack, attacking_stage_modifiers.attack_stage)
        attacking_battle_stats.special_attack = modify_stat_by_stage(attacking_battle_stats.special_attack, attacking_stage_modifiers.special_attack_stage)
    if reorder_defender_stage:
        defending_battle_stats.defense = modify_stat_by_stage(defending_battle_stats.defense, defending_stage_modifiers.defense_stage)
        defending_battle_stats.special_defense = modify_stat_by_stage(defending_battle_stats.special_defense, defending_stage_modifiers.special_defense_stage)

    if move_type in special_types:
        attacking_stat = attacking_battle_stats.special_attack
        defending_stat = defending_battle_stats.special_defense
        if defender_has_light_screen and not is_real_crit and move.name != gen_three_const.BRICK_BREAK_MOVE_NAME:
            screen_active = True
        else:
            screen_active = False
    else:
        attacking_stat = attacking_battle_stats.attack
        defending_stat = defending_battle_stats.defense
        if defender_has_reflect and not is_real_crit and move.name != gen_three_const.BRICK_BREAK_MOVE_NAME:
            screen_active = True
        else:
            screen_active = False

    is_stab = (attacking_species.first_type == move_type) or (attacking_species.second_type == move_type)
    if move.name == const.FUTURE_SIGHT_MOVE_NAME or move.name == const.DOOM_DESIRE_MOVE_NAME:
        is_stab = False

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
            # 2 alive defenders: dmg = 2*(dmg/3) instead of a plain halving (bug #16)
            temp = 2 * (temp // 3)
        else:
            temp = math.floor(temp / 2)

    if is_double_battle and move.targeting == const.TARGETING_BOTH_ENEMIES:
        temp = math.floor(temp / 2)

    if move_type not in special_types and temp == 0:
        # physical-only minimum of 1 before the weather/+2 steps (pokemon.c:3270-3271, bug #18)
        temp = 1

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
        is_real_crit and
        defending_pkmn.ability not in [gen_three_const.BATTLE_ARMOR_ABILITY, gen_three_const.SHELL_ARMOR_ABILITY]
    ):
        temp *= 2
    
    # handle all the special moves that may affect the damage formula in other ways
    # (Rollout, Fury Cutter and Triple Kick are now resolved as base-power changes
    # earlier, not as a final-damage multiplier -- bugs #1, #3, #4; Rage's stage
    # change is handled via its moves.json effect entry -- bug #10)
    move_modifier = 1

    if move.name == gen_three_const.SPIT_UP_MOVE_NAME:
        move_modifier = int(custom_move_data)

    temp *= move_modifier

    if move.name in [
        gen_three_const.GUST_MOVE_NAME,
        gen_three_const.TWISTER_MOVE_NAME,
        gen_three_const.SURF_MOVE_NAME,
        gen_three_const.WHIRLPOOL_MOVE_NAME,
        gen_three_const.EARTHQUAKE_MOVE_NAME,
        gen_three_const.PURSUIT_MOVE_NAME,
        gen_three_const.STOMP_MOVE_NAME,
        gen_three_const.EXTRASENSORY_MOVE_NAME,
        gen_three_const.ASTONISH_MOVE_NAME,
        gen_three_const.NEEDLE_ARM_MOVE_NAME,
        gen_three_const.FACADE_MOVE_NAME,
        gen_three_const.SMELLING_SALT_MOVE_NAME,
        gen_three_const.REVENGE_MOVE_NAME,
    ]:
        double_damage = custom_move_data and (gen_three_const.NO_BONUS not in custom_move_data)
    elif move.name == gen_three_const.MAGNITUDE_MOVE_NAME:
        # Magnitude folds the bonus into the same string as the magnitude level
        # ("Mag 7" vs "Mag 7 Dig Bonus"), so there is no "No Bonus" option to test for
        double_damage = (gen_three_const.DIG_BONUS in custom_move_data)
    elif move.name == gen_three_const.NATURE_POWER_MOVE_NAME:
        custom_move_data = custom_move_data or ""
        double_damage = (gen_three_const.DIG_BONUS in custom_move_data) or (gen_three_const.DIVE_BONUS in custom_move_data)
    elif move.name == const.WEATHER_BALL_MOVE_NAME:
        # dmgMultiplier = 2 while the weather is active (bug #8), not a base-power change
        double_damage = is_weather_active
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
        temp = math.floor(temp * 1.5)

    if not skip_type_and_immunity:
        for test_type in type_chart.get(move_type):
            if test_type == defending_species.first_type or test_type == defending_species.second_type:
                effectiveness = type_chart.get(move_type).get(test_type)
                if effectiveness == const.SUPER_EFFECTIVE:
                    temp *= 2
                elif effectiveness == const.NOT_VERY_EFFECTIVE:
                    temp = math.floor(temp / 2)
    
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
                other_damage = calculate_gen_three_damage(
                    attacking_pkmn,
                    attacking_species,
                    move,
                    defending_pkmn,
                    defending_species,
                    special_types,
                    type_chart,
                    held_item_boost_table,
                    original_attacking_stage_modifiers,
                    original_defending_stage_modifiers,
                    defender_has_light_screen=defender_has_light_screen,
                    defender_has_reflect=defender_has_reflect,
                    custom_move_data="",
                    weather=weather,
                    is_double_battle=is_double_battle,
                )
            else:
                other_damage = result
            
            for _ in range(1, multi_hit_multiplier):
                result = result + other_damage
    
    return result
