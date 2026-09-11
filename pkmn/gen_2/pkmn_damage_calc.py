import copy
import math
import logging
from typing import Dict, List

from pkmn import universal_data_objects, damage_calc
from pkmn.gen_2.data_objects import GenTwoBadgeList, get_hidden_power_type, get_hidden_power_base_power
from utils.constants import const
from pkmn.gen_2.gen_two_constants import gen_two_const


logger = logging.getLogger(__name__)


MIN_RANGE = 217
MAX_RANGE = 255
NUM_ROLLS = MAX_RANGE - MIN_RANGE + 1

# Present's type-id table (constants/type_constants.asm), used only by the Gold/Silver
# register-clobber formula below (see calculate_present_gold_silver_damage).
_PRESENT_TYPE_IDS = {
    const.TYPE_NORMAL: 0,
    const.TYPE_FIGHTING: 1,
    const.TYPE_FLYING: 2,
    const.TYPE_POISON: 3,
    const.TYPE_GROUND: 4,
    const.TYPE_ROCK: 5,
    const.TYPE_BUG: 7,
    const.TYPE_GHOST: 8,
    const.TYPE_STEEL: 9,
    const.TYPE_FIRE: 20,
    const.TYPE_WATER: 21,
    const.TYPE_GRASS: 22,
    const.TYPE_ELECTRIC: 23,
    const.TYPE_PSYCHIC: 24,
    const.TYPE_ICE: 25,
    const.TYPE_DRAGON: 26,
    const.TYPE_DARK: 27,
}


def get_crit_rate(pkmn:universal_data_objects.EnemyPkmn, move:universal_data_objects.Move):
    if gen_two_const.FLAVOR_ONE_HIT_KO in move.attack_flavor:
        # OHKO moves either connect for a guaranteed KO or miss outright; there's no crit roll.
        return 0
    if const.FLAVOR_HIGH_CRIT in move.attack_flavor:
        return (1/4)
    return (17 / 256)


def _type_effectiveness_multiplier(move_type, defending_species, type_chart):
    """Walks the type chart in table order the same way the main formula does, returning
    the running x10-scaled multiplier (10 = neutral, 20 = 2x, 5 = 0.5x, 0 = immune). Used
    both by the normal type-effectiveness step and by Present's Gold/Silver formula, which
    reads this same value out of a clobbered register as its "attack stat" (see 1.15 (2) /
    4.4 in the gen 2 findings doc)."""
    result = 10
    for test_type in type_chart.get(move_type, {}):
        if test_type == defending_species.first_type or test_type == defending_species.second_type:
            effectiveness = type_chart.get(move_type).get(test_type)
            if effectiveness == const.SUPER_EFFECTIVE:
                result = result * 20 // 10
            elif effectiveness == const.NOT_VERY_EFFECTIVE:
                result = result * 5 // 10
            elif effectiveness == const.IMMUNE:
                result = 0
    return result


def _is_immune(move_type, defending_species, type_chart):
    return (
        type_chart.get(move_type, {}).get(defending_species.first_type) == const.IMMUNE or
        type_chart.get(move_type, {}).get(defending_species.second_type) == const.IMMUNE
    )


def calculate_present_gold_silver_damage(
    attacking_pkmn:universal_data_objects.EnemyPkmn,
    attacking_species:universal_data_objects.PokemonSpecies,
    defending_species:universal_data_objects.PokemonSpecies,
    base_power:int,
    type_chart:Dict[str, Dict[str, str]],
    held_item_boost_table:Dict[str, str],
    is_crit:bool,
) -> damage_calc.DamageRange:
    """Gold/Silver's `BattleCommand_Present` fails to preserve registers around its call to
    `BattleCommand_Stab`, so `damagecalc` runs with garbage stat/level inputs instead of the
    user's real stats. Fully derived (and verified against the doc's worked examples) in the
    gen 2 findings doc, section 1.15 (2) / 4.4:
      A = the real Normal-type-vs-target type effectiveness product (x10 scale)
      D = 1 if the user is Normal-type (has "STAB"), else the id of the user's second type
      level (e) = the id of the target's second type
      base = floor(2e/5) + 2
    ...and the formula otherwise follows the normal damagecalc/stab pipeline (item boost,
    crit x2, +2, STAB, and the real type effectiveness applied again for real)."""
    a_value = _type_effectiveness_multiplier(const.TYPE_NORMAL, defending_species, type_chart)
    is_stab = attacking_species.first_type == const.TYPE_NORMAL or attacking_species.second_type == const.TYPE_NORMAL
    if is_stab:
        d_value = 1
    else:
        d_value = _PRESENT_TYPE_IDS.get(attacking_species.second_type, 1)
        if d_value == 0:
            d_value = 1

    level_proxy = _PRESENT_TYPE_IDS.get(defending_species.second_type, 0)
    base = math.floor(2 * level_proxy / 5) + 2

    temp = base * base_power * a_value
    temp = math.floor(temp / d_value)
    temp = math.floor(temp / 50)

    if held_item_boost_table.get(attacking_pkmn.held_item) == const.TYPE_NORMAL:
        temp = math.floor(temp * 1.1)

    if is_crit:
        temp *= 2

    temp = min(temp, 997) + 2

    if is_stab:
        temp += math.floor(temp / 2)

    # the `stab` command's type-effectiveness step for real, using the same multiplier
    temp = math.floor(temp * a_value / 10)

    if temp <= 0:
        temp = 1

    damage_vals = {}
    for numerator in range(MIN_RANGE, MAX_RANGE + 1):
        cur_damage = max(math.floor((temp * numerator) / MAX_RANGE), 1)
        damage_vals[cur_damage] = damage_vals.get(cur_damage, 0) + 1

    return damage_calc.DamageRange(damage_vals)


def calculate_gen_two_damage(
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
    attacking_battle_stats:universal_data_objects.StatBlock=None,
    defending_battle_stats:universal_data_objects.StatBlock=None,
    version_name:str=None,
):
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

    # Super Fang: floor(target current HP / 2), min 1. No crit, no STAB, no random roll;
    # type immunity (vs Ghost) still applies (data/moves/effects.asm SuperFang/ConstantDamage).
    if gen_two_const.FLAVOR_SUPER_FANG in move.attack_flavor:
        if _is_immune(move_type, defending_species, type_chart):
            return None
        return damage_calc.DamageRange({max(defending_pkmn.cur_stats.hp // 2, 1): 1})

    # One-hit-KO moves (Guillotine, Horn Drill, Fissure): damage equals the target's current
    # HP (a guaranteed KO) if the move connects at all; it fails outright (not just "misses")
    # if the user is under-levelled or the target is immune. Accuracy is handled separately
    # by get_move_accuracy (76 + 2*(Luser-Ltarget))/256.
    if gen_two_const.FLAVOR_ONE_HIT_KO in move.attack_flavor:
        if _is_immune(move_type, defending_species, type_chart):
            return None
        if attacking_pkmn.level < defending_pkmn.level:
            return None
        return damage_calc.DamageRange({defending_pkmn.cur_stats.hp: 1})

    # Beat Up needs the user's whole party (each non-fainted, non-statused member hits with
    # its own base Attack/level, no STAB/type/weather) -- data this calculator has no access
    # to (only the active attacker is passed in). Rather than silently produce the wrong
    # number via the vanilla formula, report it as not-implemented. See gen_2_findings.md 4.3.
    if gen_two_const.FLAVOR_BEAT_UP in move.attack_flavor:
        return None

    # Counter / Mirror Coat / Bide all need "damage the target dealt last turn/last few turns"
    # as an input, which this router has no source for (it isn't simulating turn order or
    # prior damage). Documented as not-implemented in gen_2_findings.md 4.7; return None
    # instead of the nonsense the old base_power=-1 fallthrough produced.
    if move.name in (gen_two_const.COUNTER_MOVE_NAME, gen_two_const.MIRROR_COAT_MOVE_NAME, gen_two_const.BIDE_MOVE_NAME):
        return None

    is_present = move.name == gen_two_const.PRESENT_MOVE_NAME
    if is_present:
        if gen_two_const.PRESENT_HEAL in custom_move_data:
            # Present rolled its 20% "heal the target" outcome -- no damage dealt.
            return None
        try:
            base_power = int(custom_move_data)
        except (TypeError, ValueError):
            base_power = 40

        is_gold_silver = version_name in (const.GOLD_VERSION, const.SILVER_VERSION)
        if is_gold_silver:
            return calculate_present_gold_silver_damage(
                attacking_pkmn, attacking_species, defending_species, base_power,
                type_chart, held_item_boost_table, is_crit,
            )
        # Crystal runs Present through the normal formula with the user's real stats; fall
        # through to the vanilla pipeline below with base_power already resolved.
    elif move.name == gen_two_const.FRUSTRATION_MOVE_NAME:
        try:
            base_power = int(custom_move_data)
        except (TypeError, ValueError):
            base_power = 0

    # NOTE: base_power == 0 (not <= 0) is deliberate: Magnitude/Flail/Reversal/Psywave all
    # carry a -1 placeholder in moves.json and are resolved to a real value either just below
    # (FLAVOR_PSYWAVE) or further down in the move-specific override block.
    if base_power is None or base_power == 0:
        return None

    # Future Sight has no `stab` command at all, so it skips type effectiveness (and
    # therefore immunity) entirely -- it can even hit Dark types head-on.
    if move.name != const.FUTURE_SIGHT_MOVE_NAME and _is_immune(move_type, defending_species, type_chart):
        return None

    # special move interactions
    if const.FLAVOR_FIXED_DAMAGE in move.attack_flavor:
        return damage_calc.DamageRange({base_power: 1})
    elif const.FLAVOR_LEVEL_DAMAGE in move.attack_flavor:
        return damage_calc.DamageRange({attacking_pkmn.level: 1})
    elif const.FLAVOR_PSYWAVE in move.attack_flavor:
        psywave_upper_limit = math.floor(attacking_pkmn.level * 1.5)
        return damage_calc.DamageRange({x:1 for x in range(1, psywave_upper_limit)})

    if attacking_stage_modifiers is None:
        attacking_stage_modifiers = universal_data_objects.StageModifiers()
    if defending_stage_modifiers is None:
        defending_stage_modifiers = universal_data_objects.StageModifiers()

    # Flail/Reversal/Future Sight have no `critical` command in their scripts at all, so
    # `wCriticalHit` is always 0 when they run: they can never crit, regardless of what the
    # caller passed in for is_crit (which would otherwise incorrectly feed into the stage-zeroing
    # rule just below).
    if move.name in (const.FLAIL_MOVE_NAME, const.REVERSAL_MOVE_NAME, const.FUTURE_SIGHT_MOVE_NAME):
        is_crit = False

    # for gen two, the "is_crit" flag in the upcoming get_battle_stats call is effectively a flag to ignore badge boosts
    # always calculate badge boosts during a non crit
    # during a crit, if the stage modifiers are in favor of the attack, calculate badge boosts and stage modifiers
    # during a crit, if the stage modifiers are equal or in favor of the defender, calculate stats without any badge boosts and without any stage modifiers
    ignore_badge_boosts = False
    # Keep the caller's real stage modifiers around: the crit rule below may replace the
    # local variables with a zeroed StageModifiers, but a multi-hit move's non-crit hits
    # (see the recursive call near the bottom of this function) must still use the real ones.
    original_attacking_stage_modifiers = attacking_stage_modifiers
    original_defending_stage_modifiers = defending_stage_modifiers

    if is_crit:
        if move_type in special_types and attacking_stage_modifiers.special_attack_stage <= defending_stage_modifiers.special_defense_stage:
            # stage modifiers do not favor the attacker for a special move: zero out the stage modifiers
            ignore_badge_boosts = True
            attacking_stage_modifiers = universal_data_objects.StageModifiers()
            defending_stage_modifiers = universal_data_objects.StageModifiers()
        elif move_type not in special_types and attacking_stage_modifiers.attack_stage <= defending_stage_modifiers.defense_stage:
            # stage modifiers do not favor the attacker for a physical move: zero out the stage modifiers
            ignore_badge_boosts = True
            attacking_stage_modifiers = universal_data_objects.StageModifiers()
            defending_stage_modifiers = universal_data_objects.StageModifiers()

    if attacking_battle_stats is None:
        attacking_battle_stats = attacking_pkmn.get_battle_stats(attacking_stage_modifiers, is_crit=ignore_badge_boosts)
    else:
        # Don't mutate a StatBlock the caller may reuse (e.g. across a normal + crit call pair).
        attacking_battle_stats = copy.copy(attacking_battle_stats)
    if defending_battle_stats is None:
        defending_battle_stats = defending_pkmn.get_battle_stats(defending_stage_modifiers, is_crit=ignore_badge_boosts)
    else:
        defending_battle_stats = copy.copy(defending_battle_stats)

    if (
        attacking_pkmn.name in (gen_two_const.CUBONE_NAME, gen_two_const.MAROWAK_NAME) and
        attacking_pkmn.held_item == gen_two_const.THICK_CLUB_NAME
    ):
        attacking_battle_stats.attack *= 2
    elif attacking_pkmn.name == gen_two_const.PIKACHU_NAME and attacking_pkmn.held_item == gen_two_const.LIGHT_BALL_NAME:
        attacking_battle_stats.special_attack *= 2

    if move_type in special_types:
        attacking_stat = attacking_battle_stats.special_attack
        defending_stat = defending_battle_stats.special_defense
        # Reflect/Light Screen double the (pre-truncation) defensive stat; this happens
        # before the crit rule's "keep the boosted stats" decision, so a crit still gets
        # the doubled screen value whenever the attacker's stage favors it (ignore_badge_boosts
        # is only True on the unfavorable branch, which reloads unboosted/unstaged stats anyway).
        doubled_def = defender_has_light_screen and not ignore_badge_boosts
    else:
        attacking_stat = attacking_battle_stats.attack
        defending_stat = defending_battle_stats.defense
        doubled_def = defender_has_reflect and not ignore_badge_boosts

    if doubled_def:
        defending_stat *= 2

    # TruncateHL_BC: whenever either 16-bit stat is >= 256, both are divided by 4 (floor, min 1).
    # Crystal re-loops until both are below 256; Gold/Silver only do a single pass and then
    # keep just the low byte, which can wrap if a stat was still >= 256 after that one pass.
    is_gold_silver = version_name in (const.GOLD_VERSION, const.SILVER_VERSION)
    while attacking_stat >= 256 or defending_stat >= 256:
        attacking_stat = max(math.floor(attacking_stat / 4), 1)
        defending_stat = max(math.floor(defending_stat / 4), 1)
        if is_gold_silver:
            attacking_stat &= 0xFF
            defending_stat &= 0xFF
            break

    # Metal Powder boosts a Ditto defender's truncated 8-bit defensive stat by ~1.5x
    # regardless of physical/special category. If that overflows a byte, the game's 8-bit
    # math instead halves the attacking stat and only gets a partial defense boost.
    if defending_pkmn.name == gen_two_const.DITTO_NAME and defending_pkmn.held_item == gen_two_const.METAL_POWDER_NAME:
        boosted_defending_stat = defending_stat + (defending_stat >> 1)
        if boosted_defending_stat > 255:
            attacking_stat = max(attacking_stat >> 1, 1)
            defending_stat = (defending_stat + (defending_stat >> 1)) >> 1
        else:
            defending_stat = boosted_defending_stat

    if move.name == const.EXPLOSION_MOVE_NAME or move.name == const.SELFDESTRUCT_MOVE_NAME:
        defending_stat = max(math.floor(defending_stat / 2), 1)

    is_future_sight = move.name == const.FUTURE_SIGHT_MOVE_NAME

    is_stab = (attacking_species.first_type == move_type) or (attacking_species.second_type == move_type)
    if is_future_sight:
        is_stab = False

    # Struggle's own move type is stored as "none" so it never gets STAB/type effectiveness,
    # but the game's held-item type-boost check keys directly on the move's real (Normal)
    # type, which is looked up separately from the `stab` command that skips Struggle.
    if move.name == const.STRUGGLE_MOVE_NAME:
        held_item_boost = held_item_boost_table.get(attacking_pkmn.held_item) == const.TYPE_NORMAL
    else:
        held_item_boost = held_item_boost_table.get(attacking_pkmn.held_item) == move_type

    badges:GenTwoBadgeList = attacking_pkmn.badges

    if badges is None:
        badge_type_boost = False
    elif move_type == const.TYPE_FLYING and badges.zephyr:
        badge_type_boost = True
    elif move_type == const.TYPE_BUG and badges.hive:
        badge_type_boost = True
    elif move_type == const.TYPE_NORMAL and badges.plain:
        badge_type_boost = True
    elif move_type == const.TYPE_GHOST and badges.fog:
        badge_type_boost = True
    elif move_type == const.TYPE_FIGHTING and badges.storm:
        badge_type_boost = True
    elif move_type == const.TYPE_STEEL and badges.mineral:
        badge_type_boost = True
    elif move_type == const.TYPE_ICE and badges.glacier:
        badge_type_boost = True
    elif move_type == const.TYPE_DRAGON and badges.rising:
        badge_type_boost = True
    elif move_type == const.TYPE_ROCK and badges.boulder:
        badge_type_boost = True
    elif move_type == const.TYPE_WATER and badges.cascade:
        badge_type_boost = True
    elif move_type == const.TYPE_ELECTRIC and badges.thunder:
        badge_type_boost = True
    elif move_type == const.TYPE_GRASS and badges.rainbow:
        badge_type_boost = True
    elif move_type == const.TYPE_POISON and badges.soul:
        badge_type_boost = True
    elif move_type == const.TYPE_PSYCHIC and badges.marsh:
        badge_type_boost = True
    elif move_type == const.TYPE_FIRE and badges.volcano:
        badge_type_boost = True
    elif move_type == const.TYPE_GROUND and badges.earth:
        badge_type_boost = True
    else:
        badge_type_boost = False

    if move.name == gen_two_const.MAGNITUDE_MOVE_NAME:
        if gen_two_const.MAGNITUDE_4 in custom_move_data:
            base_power = 10
        elif gen_two_const.MAGNITUDE_5 in custom_move_data:
            base_power = 30
        elif gen_two_const.MAGNITUDE_6 in custom_move_data:
            base_power = 50
        elif gen_two_const.MAGNITUDE_7 in custom_move_data:
            base_power = 70
        elif gen_two_const.MAGNITUDE_8 in custom_move_data:
            base_power = 90
        elif gen_two_const.MAGNITUDE_9 in custom_move_data:
            base_power = 110
        elif gen_two_const.MAGNITUDE_10 in custom_move_data:
            base_power = 150
    elif move.name in [const.FLAIL_MOVE_NAME, const.REVERSAL_MOVE_NAME]:
        if gen_two_const.FLAIL_FULL_HP in custom_move_data:
            base_power = 20
        elif gen_two_const.FLAIL_HALF_HP in custom_move_data:
            base_power = 40
        elif gen_two_const.FLAIL_QUARTER_HP in custom_move_data:
            base_power = 80
        elif gen_two_const.FLAIL_TEN_PERCENT_HP in custom_move_data:
            base_power = 100
        elif gen_two_const.FLAIL_FIVE_PERCENT_HP in custom_move_data:
            base_power = 150
        elif gen_two_const.FLAIL_MIN_HP in custom_move_data:
            base_power = 200
    elif move.name == gen_two_const.RETURN_MOVE_NAME:
        try:
            base_power = int(custom_move_data)
        except Exception as e:
            logger.warning(f"Failed to convert return move power to an int: {custom_move_data}")

    # begin actual formula
    temp = 2 * attacking_pkmn.level
    temp = math.floor(temp / 5) + 2

    temp *= base_power
    temp *= attacking_stat
    temp = math.floor(temp / defending_stat)

    temp = math.floor(temp / 50)

    if held_item_boost:
        temp = math.floor(temp * 1.1)

    # forcibly prevent crits for Flail, Reversal, and Future sight
    if is_crit and move.name not in [const.FLAIL_MOVE_NAME, const.REVERSAL_MOVE_NAME, const.FUTURE_SIGHT_MOVE_NAME]:
        temp *= 2

    temp = min(temp, 997) + 2

    is_triple_kick = move.name == gen_two_const.TRIPLE_KICK_MOVE_NAME

    def _finish(base_temp):
        """Everything from the `stab` command onward: weather, badge-type boost, STAB, type
        effectiveness, the Rollout/Fury Cutter/Rage move multiplier, and the random roll.
        Future Sight skips weather/badge/type entirely (no `stab` command at all); Flail and
        Reversal skip the random roll (single fixed value, computed a step earlier for
        anything move-multiplier-related, though none of them use one)."""
        t = base_temp

        weather_boost = False
        weather_penalty = False
        if not is_future_sight:
            if weather == const.WEATHER_RAIN:
                weather_boost = (move_type == const.TYPE_WATER)
                weather_penalty = (
                    move_type == const.TYPE_FIRE or
                    move.name == const.SOLAR_BEAM_MOVE_NAME
                )
            elif weather == const.WEATHER_SUN:
                weather_boost = (move_type == const.TYPE_FIRE)
                weather_penalty = (move_type == const.TYPE_WATER)

        if weather_boost:
            t = math.floor(t * 1.5)
        elif weather_penalty:
            t = math.floor(t * 0.5)

        if not is_future_sight and badge_type_boost:
            t = t + max(t >> 3, 1)

        if is_stab:
            t += math.floor(t / 2)

        # the order type effectiveness gets applied is based on an ordering in an internal table
        # the order is NOT based on which type is "first" or "second" for the defending mon
        # this usually doesn't matter, but there's one specific case where it can
        # specifically, if the move is both super effective and not very effective, the effective power will be neutral
        # but you may lose 1 point of power due to rounding if the division happens first
        if not is_future_sight:
            for test_type in type_chart.get(move_type):
                if test_type == defending_species.first_type or test_type == defending_species.second_type:
                    effectiveness = type_chart.get(move_type).get(test_type)
                    if effectiveness == const.SUPER_EFFECTIVE:
                        t *= 2
                    elif effectiveness == const.NOT_VERY_EFFECTIVE:
                        t = math.floor(t / 2)

        move_modifier = 1
        if move.name == gen_two_const.ROLLOUT_MOVE_NAME:
            # ugh, dumb hack. String is either just an int, or the special case of last turn + defense curl
            try:
                num_rollout_turns = int(custom_move_data)
            except ValueError:
                num_rollout_turns = 6

            move_modifier = math.pow(2, num_rollout_turns - 1)
        elif move.name == gen_two_const.FURY_CUTTER_MOVE_NAME:
            # capped at x16 (5 doublings); the dropdown's "6" is really "5 or more"
            move_modifier = math.pow(2, min(int(custom_move_data), 5) - 1)
        elif move.name == gen_two_const.RAGE_MOVE_NAME:
            move_modifier = int(custom_move_data)

        t *= move_modifier

        if t <= 0:
            # damage must be at least 1
            t = 1

        if move.name in [const.FLAIL_MOVE_NAME, const.REVERSAL_MOVE_NAME]:
            return damage_calc.DamageRange({int(t): 1})

        double_damage = False
        if move.name in [
            gen_two_const.GUST_MOVE_NAME,
            gen_two_const.TWISTER_MOVE_NAME,
            gen_two_const.EARTHQUAKE_MOVE_NAME,
            gen_two_const.STOMP_MOVE_NAME,
            gen_two_const.PURSUIT_MOVE_NAME,
        ]:
            double_damage = (gen_two_const.NO_BONUS not in custom_move_data)
        elif move.name == gen_two_const.MAGNITUDE_MOVE_NAME:
            double_damage = (gen_two_const.DIG_BONUS in custom_move_data)

        damage_vals = {}
        for numerator in range(MIN_RANGE, MAX_RANGE + 1):
            cur_damage = max(math.floor((t * numerator) / MAX_RANGE), 1)
            if double_damage:
                # Gust/Twister/Earthquake/Magnitude/Stomp/Pursuit's bonus is applied to each
                # rolled value, AFTER the random roll, not to the pre-roll damage.
                cur_damage = min(cur_damage * 2, 65535)

            if cur_damage not in damage_vals:
                damage_vals[cur_damage] = 0

            damage_vals[cur_damage] += 1

        return damage_calc.DamageRange(damage_vals)

    if is_triple_kick:
        # Triple Kick multiplies the post-"+2" damage by the kick number (1st/2nd/3rd kick)
        # BEFORE weather/badge/STAB/type, and each kick gets its own independent random roll;
        # the dropdown selects the total number of kicks that connected.
        try:
            num_kicks = int(custom_move_data[0])
        except (TypeError, ValueError, IndexError):
            num_kicks = 3
        num_kicks = max(1, min(num_kicks, 3))

        result = None
        for kick_index in range(1, num_kicks + 1):
            kick_result = _finish(temp * kick_index)
            result = kick_result if result is None else result.add(kick_result)
        return result

    result = _finish(temp)

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

    if multi_hit_multiplier > 1:
        if is_crit:
            # Currently forcing "crit" calculations to assume only one crit out of all strikes
            # So, when calculating full damage, need to get the damage of a single non-crit strike as well
            # intentionally overwriting custom_move_data to make sure we get the damage of only a single strike.
            # Use the ORIGINAL (un-zeroed) stage modifiers and the real weather: the non-crit
            # hits are not affected by the crit rule's stat reload.
            other_damage = calculate_gen_two_damage(
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
                version_name=version_name,
            )
        else:
            other_damage = result

        for _ in range(1, multi_hit_multiplier):
            result = result + other_damage

    if gen_two_const.FLAVOR_FALSE_SWIPE in move.attack_flavor:
        # False Swipe can never reduce the target below 1 HP.
        cap = max(defending_pkmn.cur_stats.hp - 1, 1)
        clamped_vals = {}
        for dmg, count in result.damage_vals.items():
            clamped_dmg = min(dmg, cap)
            clamped_vals[clamped_dmg] = clamped_vals.get(clamped_dmg, 0) + count
        result = damage_calc.DamageRange(clamped_vals, num_attacks=result.num_attacks)

    return result
