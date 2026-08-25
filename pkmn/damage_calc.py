import copy
import math
import logging
from typing import Dict, Tuple

from utils.constants import const

logger = logging.getLogger(__name__)


# Moves whose damage the normal formula can't produce: either a constant amount
# (Dragon Rage 40, Sonic Boom 20) or an amount equal to the user's level (Seismic
# Toss, Night Shade). The move data doesn't carry these reliably -- base_power is
# often None or a placeholder like 1, and the gen 5 move data has no attack_flavor
# at all -- so they're defined here by name and applied uniformly across all
# generations. Matched on every spelling that appears (e.g. "SonicBoom" in gens
# 1-4 vs "Sonic Boom" in gen 5).
_FIXED_DAMAGE_BY_NAME = {
    const.DRAGON_RAGE_MOVE_NAME: 40,
    "SonicBoom": 20,
    "Sonic Boom": 20,
}
_LEVEL_DAMAGE_MOVE_NAMES = {
    "Seismic Toss",
    "Night Shade",
}


def get_special_damage_override(move, attacking_pkmn, defending_species=None, type_chart=None):
    """Return a DamageRange for moves that deal a special, formula-independent
    amount -- a constant (Dragon Rage -> 40, Sonic Boom -> 20) or the user's level
    (Seismic Toss, Night Shade) -- or None for any other move. Generation-agnostic;
    callers should consult this before running the normal damage formula (and
    before any base_power None early-out, since these moves don't store their
    damage in base_power consistently across gens).

    When ``defending_species`` and ``type_chart`` are supplied (gen 2+), type
    immunity is respected: an immune matchup returns None so the caller falls
    through to its normal no-damage handling. Gen 1 omits these args, so it is
    unconditional -- matching gen 1's historical behavior of dealing the fixed/level
    damage regardless of type immunity."""
    amount = _FIXED_DAMAGE_BY_NAME.get(move.name)
    if amount is None and move.name in _LEVEL_DAMAGE_MOVE_NAMES:
        amount = attacking_pkmn.level
    if amount is None:
        return None

    if defending_species is not None and type_chart is not None:
        move_effectiveness = type_chart.get(move.move_type, {})
        if (move_effectiveness.get(defending_species.first_type) == const.IMMUNE or
                move_effectiveness.get(defending_species.second_type) == const.IMMUNE):
            # Immune: defer to the caller's normal path, which produces no damage.
            return None

    return DamageRange({amount: 1})


def is_weather_active(attacking_ability:str, defending_ability:str, weather:str) -> bool:
    """Whether ``weather`` is actually in effect. Air Lock / Cloud Nine on either
    side shuts it off completely.

    TODO: technically inaccurate data if in a doubles battle, and either of the other
    mons outside the equation have these abilities. Wtv. It doesn't actually ever
    happen in vanilla games, so we're just fully ignoring it"""
    if weather == const.WEATHER_NONE:
        return False
    return (
        attacking_ability not in const.WEATHER_SUPPRESSING_ABILITIES and
        defending_ability not in const.WEATHER_SUPPRESSING_ABILITIES
    )


def get_weather_ball_type(weather:str, weather_is_active:bool, default_type:str=None) -> str:
    """The type Weather Ball takes on in the given weather. Returns ``default_type``
    (the move's own type, i.e. Normal) when there's no weather to key off of, and for
    weather with no type of its own -- gen 4/5 fog still doubles the move's power, but
    leaves it Normal."""
    if not weather_is_active:
        return default_type
    return const.WEATHER_BALL_TYPE_MAP.get(weather, default_type)


def apply_forecast(species, ability:str, weather:str, weather_is_active:bool):
    """Castform's Forecast ability retypes it to match the active weather
    (Sun -> Fire, Rain -> Water, Hail -> Ice). Returns a copy of ``species`` with
    both types replaced, or ``species`` untouched when Forecast isn't relevant.

    Returning a copy matters: the species objects come straight out of the pkmn
    db and are shared by every caller, so they must never be mutated in place.

    ``weather_is_active`` is the caller's already-resolved weather state, so
    abilities that suppress weather (Air Lock / Cloud Nine) also suppress
    Forecast, and ``ability`` should likewise already account for suppression
    (Gastro Acid) in the gens that support it."""
    if species is None or not weather_is_active or ability != const.FORECAST_ABILITY:
        return species

    new_type = const.FORECAST_TYPE_MAP.get(weather)
    if new_type is None:
        return species

    result = copy.copy(species)
    result.first_type = new_type
    result.second_type = new_type
    return result


class DamageRange:
    def __init__(self, damage_vals:dict, num_attacks=1):
        self.damage_vals = {}
        self.min_damage = None
        self.max_damage = None
        self.size = 0

        for cur_damage in damage_vals:
            if cur_damage not in self.damage_vals:
                self.damage_vals[cur_damage] = 0
            
            self.damage_vals[cur_damage] += damage_vals[cur_damage]
            self.size += damage_vals[cur_damage]

            if self.min_damage is None or cur_damage < self.min_damage:
                self.min_damage = cur_damage
            if self.max_damage is None or cur_damage > self.max_damage:
                self.max_damage = cur_damage

        if self.max_damage is None or self.min_damage is None:
            raise Exception

        self.num_attacks = num_attacks

    @staticmethod
    def merge_Pokemon_min_max(range_for_min, range_for_max):
        """Merge two DamageRanges: take min from one and max from the other.

        Used for wild pokemon where DVs are unknown: the min damage comes
        from the calculation against the tankiest/weakest variant, and the
        max comes from the squishiest/strongest variant.
        """
        if range_for_min is None or range_for_max is None:
            return range_for_min or range_for_max
        merged_vals = {}
        merged_vals[range_for_min.min_damage] = 1
        merged_vals[range_for_max.max_damage] = 1
        return DamageRange(merged_vals)

    def add(self, other):
        if not isinstance(other, DamageRange):
            raise ValueError("Can only add DamageRange to other DamageRanges")

        result_damage_vals = {}

        for my_cur_damage, my_count in self.damage_vals.items():
            for your_cur_damage, your_count in other.damage_vals.items():
                cur_total_damage = my_cur_damage + your_cur_damage
                if cur_total_damage not in result_damage_vals:
                    result_damage_vals[cur_total_damage] = 0

                result_damage_vals[cur_total_damage] += my_count + your_count

        return DamageRange(result_damage_vals, num_attacks=(self.num_attacks + other.num_attacks))

    def split_kills(self, hp_threshold):
        if hp_threshold > self.max_damage:
            return None, self
        elif hp_threshold <= self.min_damage:
            return self, None

        kill_damage_vals = {}
        non_kill_damage_vals = {}

        for cur_damage, cur_count in self.damage_vals.items():
            if cur_damage >= hp_threshold:
                kill_damage_vals[cur_damage] = cur_count
            else:
                non_kill_damage_vals[cur_damage] = cur_count

        return DamageRange(kill_damage_vals, num_attacks=self.num_attacks), DamageRange(non_kill_damage_vals, num_attacks=self.num_attacks)

    def __len__(self):
        return self.size

    def to_string(self, max_num=5, percent_of=None):
        result = []

        for cur_dam in sorted(self.damage_vals.keys()):
            if percent_of is not None:
                cur_percent = (cur_dam / percent_of) * 100
                result.append(f"{cur_percent:.1f} x{self.damage_vals[cur_dam]}")
            else:
                result.append(f"{cur_dam} x{self.damage_vals[cur_dam]}")

        if max_num is not None and max_num > 1 and len(result) > max_num:
            parts = max_num // 2
            result = result[:parts] + ['...'] + result[-parts:]
        
        return ", ".join(result)

    def __repr__(self):
        return self.to_string()

    def __add__(self, other):
        return self.add(other)


def percent_rolls_kill(
    num_non_crits:int,
    damage_range:DamageRange,
    num_crits:int,
    crit_damage_range:DamageRange,
    target_hp:int,
    memoization:Dict[Tuple[int, int, int], int]
):
    num_kill_rolls = _percent_rolls_kill_recursive(
        num_non_crits,
        damage_range,
        num_crits,
        crit_damage_range,
        target_hp,
        1,
        0,
        memoization
    )

    return 100.0 * (num_kill_rolls) / math.pow(len(damage_range), num_non_crits + num_crits)

def _percent_rolls_kill_recursive(
    num_non_crits:int,
    damage_range:DamageRange,
    num_crits:int,
    crit_damage_range:DamageRange,
    target_hp:int,
    num_roll_multiplier:int,
    total_damage:int,
    memoization:Dict[Tuple[int, int, int], int]
):

    cur_key = (num_non_crits, num_crits, num_roll_multiplier, total_damage)
    if cur_key in memoization:
        return memoization[cur_key]

    min_damage_left = num_non_crits * damage_range.min_damage
    min_damage_left += num_crits * crit_damage_range.min_damage

    max_damage_left = num_non_crits * damage_range.max_damage
    max_damage_left += num_crits * crit_damage_range.max_damage

    if total_damage + min_damage_left >= target_hp:
        # if kill is guaranteed, add all rolls for this and future attacks
        # NOTE: use the number of rolls in the provided damage_range, so that special moves like psywave still work properly
        result = num_roll_multiplier * math.pow(len(damage_range), num_non_crits + num_crits)
        memoization[cur_key] = result
        return result
    elif num_crits == 0 and num_non_crits == 0:
        # ran out of attacks without a kill being found, no kill rolls found
        memoization[cur_key] = 0
        return 0
    elif total_damage + max_damage_left < target_hp:
        # kill is impossible even with future rolls, just quit and don't bother calculating
        memoization[cur_key] = 0
        return 0

    # recursive case - a kill is still possible, but not found yet
    result = 0
    if num_crits > 0:
        next_damage_range = crit_damage_range
        num_crits -= 1
    else:
        next_damage_range = damage_range
        num_non_crits -= 1
    
    # find all possible kills from this point forward
    for next_damage in next_damage_range.damage_vals:
        result += _percent_rolls_kill_recursive(
            num_non_crits,
            damage_range,
            num_crits,
            crit_damage_range,
            target_hp,
            next_damage_range.damage_vals[next_damage],
            total_damage + next_damage,
            memoization
        )
    
    result *= num_roll_multiplier
    memoization[cur_key] = result
    return result


def find_kill(damage_range:DamageRange, crit_damage_range:DamageRange, crit_chance:float, accuracy:float, target_hp:int, attack_depth:int=10, percent_cutoff:float=0.1, force_full_search=False):
    # NOTE: if attack_depth is too deep, (10+ is where I started to notice the issues), you quickly get overflow issues
    result = []

    min_possible_damage = min(damage_range.min_damage, crit_damage_range.min_damage)
    max_possible_damage = max(damage_range.max_damage, crit_damage_range.max_damage)
    highest_found_kill_pct = 0
    memoization = {}
    hits_to_kill_table = {}

    # this is a quick and dirty way to ignore calculating psywave, which has vastly more possible rolls, and thus takes much longer to calculate
    # also, don't endlessly search for a move that can't find a guaranteed kill even if it hits every time
    if (
        (len(damage_range) <= 200 and (min_possible_damage * attack_depth) > target_hp) or
        force_full_search
    ):
        for cur_num_attacks in range(1, attack_depth + 1):
            if (max_possible_damage * cur_num_attacks) < target_hp:
                continue

            # a kill is possible, but not guaranteed
            # find the exact kill percent if all swings actually hit
            all_hits_kill_pct = 0
            for cur_num_crits in range(cur_num_attacks + 1):
                # get the kill percent for this exact combination of crits + non-crits
                kill_percent = percent_rolls_kill(
                    cur_num_attacks - cur_num_crits,
                    damage_range,
                    cur_num_crits,
                    crit_damage_range,
                    target_hp,
                    memoization
                )

                # and multiply that kill percent by the probability of actually getting
                # this combination of crits + non-crits
                all_hits_kill_pct += (
                    kill_percent *
                    math.comb(cur_num_attacks, cur_num_crits) *
                    math.pow(crit_chance, cur_num_crits) *
                    math.pow(1 - crit_chance, cur_num_attacks - cur_num_crits)
                )
            
            # store this in the lookup table for probability of killing on this many successful hits
            hits_to_kill_table[cur_num_attacks] = all_hits_kill_pct

            cur_total_kill_pct = 0
            # now, iterate through all possiblities of getting however many hits
            for cur_num_hits in range(1, cur_num_attacks + 1):
                cur_total_kill_pct += (
                    hits_to_kill_table.get(cur_num_hits, 0) *
                    math.comb(cur_num_attacks, cur_num_hits) *
                    math.pow(accuracy, cur_num_hits) *
                    math.pow(1 - accuracy, cur_num_attacks - cur_num_hits)
                )

            highest_found_kill_pct = cur_total_kill_pct
            if cur_total_kill_pct > percent_cutoff:
                result.append((cur_num_attacks, cur_total_kill_pct))
            if cur_total_kill_pct > 99:
                break
    
    if highest_found_kill_pct < 99 and highest_found_kill_pct < round(accuracy * 100):
        # if we haven't found close enough to a kill, get the guaranteed kill
        result.append((math.ceil(target_hp / damage_range.min_damage), -1))
    
    return result
