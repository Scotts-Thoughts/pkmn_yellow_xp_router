"""Record every damage-calc call the Python test-suite makes, with its result.

Runs pytest over the damage tests (and the unit calc tests) with the gen
objects' `calculate_damage` / `get_crit_rate` / `get_move_accuracy` wrapped,
and writes one JSON case per call:

    py -3.14 docs/rust_port/golden/record_damage_cases.py [OUT.json]

Default OUT is docs/rust_port/golden/damage_cases.json. The Rust test
`crates/xpr-calc/tests/damage_cases.rs` replays the file and checks the
Rust port produces identical numbers for every case (min, max and the full
damage distribution) -- i.e. it inherits every expectation of the 200+
Python damage tests without hand-porting them.
"""
import inspect
import json
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
sys.path.insert(0, ROOT)

import pytest  # noqa: E402

import controllers.main_controller  # noqa: E402,F401  (resolves the import chain)
from pkmn.gen_1 import gen_one_object, pkmn_damage_calc as gen1_calc  # noqa: E402
from pkmn.gen_2 import gen_two_object  # noqa: E402
from pkmn.gen_3 import gen_three_object, pkmn_damage_calc as gen3_calc  # noqa: E402
from pkmn.gen_4 import gen_four_object  # noqa: E402
from pkmn.gen_5 import gen_five_object  # noqa: E402
from pkmn import universal_data_objects  # noqa: E402

CASES = []
_NESTED = [0]


def _stat_block(sb):
    if sb is None:
        return None
    return {
        "hp": sb.hp,
        "attack": sb.attack,
        "defense": sb.defense,
        "special_attack": sb.special_attack,
        "special_defense": sb.special_defense,
        "speed": sb.speed,
        "is_stat_xp": bool(getattr(sb, "_is_stat_xp", False)),
    }


def _badges(b):
    if b is None:
        return None
    return {k: v for k, v in vars(b).items() if isinstance(v, bool)}


def _nature(n):
    if n is None:
        return None
    return str(n)


def _mon(m):
    return {
        "name": m.name,
        "level": m.level,
        "xp": m.xp,
        "move_list": list(m.move_list),
        "cur_stats": _stat_block(m.cur_stats),
        "base_stats": _stat_block(m.base_stats),
        "dvs": _stat_block(m.dvs),
        "stat_xp": _stat_block(m.stat_xp),
        "badges": _badges(m.badges),
        "held_item": m.held_item,
        "custom_move_data": m.custom_move_data,
        "is_trainer_mon": m.is_trainer_mon,
        "exp_split": m.exp_split,
        "mon_order": m.mon_order,
        "definition_order": m.definition_order,
        "ability": m.ability,
        "nature": _nature(m.nature),
    }


def _stages(s):
    if s is None:
        return None
    return {
        "attack_stage": s.attack_stage,
        "defense_stage": s.defense_stage,
        "speed_stage": s.speed_stage,
        "special_attack_stage": s.special_attack_stage,
        "special_defense_stage": s.special_defense_stage,
        "accuracy_stage": s.accuracy_stage,
        "evasion_stage": s.evasion_stage,
        "attack_badge_boosts": s.attack_badge_boosts,
        "defense_badge_boosts": s.defense_badge_boosts,
        "speed_badge_boosts": s.speed_badge_boosts,
        "special_badge_boosts": s.special_badge_boosts,
    }


def _field(f):
    if f is None:
        return None
    return {k: v for k, v in vars(f).items() if isinstance(v, bool)}


def _move(m):
    return {
        "name": m.name,
        "accuracy": m.accuracy,
        "pp": m.pp,
        "base_power": m.base_power,
        "move_type": m.move_type,
        "attack_flavor": list(m.attack_flavor),
    }


def _result(r):
    if r is None:
        return None
    return {
        "min_damage": r.min_damage,
        "max_damage": r.max_damage,
        "size": r.size,
        "num_attacks": getattr(r, "num_attacks", 1),
        "damage_vals": [[k, v] for k, v in r.damage_vals.items()],
    }


def _current_test():
    return os.environ.get("PYTEST_CURRENT_TEST", "")


def _wrap_calc(cls):
    orig = cls.calculate_damage
    sig = inspect.signature(orig)

    def wrapped(self, *args, **kwargs):
        bound = sig.bind(self, *args, **kwargs)
        bound.apply_defaults()
        a = dict(bound.arguments)
        _NESTED[0] += 1
        try:
            result = orig(self, *args, **kwargs)
        finally:
            _NESTED[0] -= 1
        CASES.append({
            "kind": "calculate_damage",
            "test": _current_test(),
            "version": self.version_name(),
            "attacking": _mon(a["attacking_pkmn"]),
            "move": _move(a["move"]),
            "defending": _mon(a["defending_pkmn"]),
            "attacking_stages": _stages(a.get("attacking_stage_modifiers")),
            "defending_stages": _stages(a.get("defending_stage_modifiers")),
            "attacking_field": _field(a.get("attacking_field")),
            "defending_field": _field(a.get("defending_field")),
            "is_crit": bool(a.get("is_crit", False)),
            "custom_move_data": a.get("custom_move_data", ""),
            "weather": a.get("weather"),
            "is_double_battle": bool(a.get("is_double_battle", False)),
            "attacking_battle_stats": _stat_block(a.get("attacking_battle_stats")),
            "defending_battle_stats": _stat_block(a.get("defending_battle_stats")),
            "attacker_is_enemy": bool(a.get("attacker_is_enemy", False)),
            "result": _result(result),
        })
        return result

    cls.calculate_damage = wrapped


def _wrap_crit(cls):
    orig = cls.get_crit_rate

    def wrapped(self, pkmn, move, custom_move_data):
        _NESTED[0] += 1
        try:
            result = orig(self, pkmn, move, custom_move_data)
        finally:
            _NESTED[0] -= 1
        CASES.append({
            "kind": "get_crit_rate",
            "test": _current_test(),
            "version": self.version_name(),
            "attacking": _mon(pkmn),
            "move": _move(move),
            "custom_move_data": custom_move_data,
            "result": result,
        })
        return result

    cls.get_crit_rate = wrapped


def _wrap_accuracy(cls):
    orig = cls.get_move_accuracy

    def wrapped(self, pkmn, move, custom_move_data, defending_pkmn, weather):
        _NESTED[0] += 1
        try:
            result = orig(self, pkmn, move, custom_move_data, defending_pkmn, weather)
        finally:
            _NESTED[0] -= 1
        CASES.append({
            "kind": "get_move_accuracy",
            "test": _current_test(),
            "version": self.version_name(),
            "attacking": _mon(pkmn),
            "move": _move(move),
            "custom_move_data": custom_move_data,
            "defending": _mon(defending_pkmn),
            "weather": weather,
            "result": result,
        })
        return result

    cls.get_move_accuracy = wrapped


def _wrap_gen1_module_crit():
    orig = gen1_calc.get_crit_rate

    def wrapped(pkmn, move, custom_move_data=""):
        result = orig(pkmn, move, custom_move_data=custom_move_data)
        if _NESTED[0] == 0:
            CASES.append({
                "kind": "get_crit_rate",
                "test": _current_test(),
                "version": "Yellow",
                "attacking": _mon(pkmn),
                "move": _move(move),
                "custom_move_data": custom_move_data,
                "result": result,
            })
        return result

    gen1_calc.get_crit_rate = wrapped


def _wrap_gen3_module_calc():
    orig = gen3_calc.calculate_gen_three_damage
    sig = inspect.signature(orig)

    def wrapped(*args, **kwargs):
        result = orig(*args, **kwargs)
        if _NESTED[0] == 0:
            bound = sig.bind(*args, **kwargs)
            bound.apply_defaults()
            a = dict(bound.arguments)
            # the direct call passes the screen flags instead of a FieldStatus
            dfield = _field(a.get("defending_field"))
            if a.get("defender_has_reflect") or a.get("defender_has_light_screen"):
                dfield = dfield or _field(universal_data_objects.FieldStatus())
                dfield["reflect"] = bool(a.get("defender_has_reflect")) or dfield.get("reflect", False)
                dfield["light_screen"] = bool(a.get("defender_has_light_screen")) or dfield.get("light_screen", False)
            CASES.append({
                "kind": "calculate_damage",
                "test": _current_test(),
                "version": os.environ.get("XPR_GEN3_VERSION", "Emerald"),
                "attacking": _mon(a["attacking_pkmn"]),
                "move": _move(a["move"]),
                "defending": _mon(a["defending_pkmn"]),
                "attacking_stages": _stages(a.get("attacking_stage_modifiers")),
                "defending_stages": _stages(a.get("defending_stage_modifiers")),
                "attacking_field": _field(a.get("attacking_field")),
                "defending_field": dfield,
                "is_crit": bool(a.get("is_crit", False)),
                "custom_move_data": a.get("custom_move_data", "") or "",
                "weather": a.get("weather"),
                "is_double_battle": bool(a.get("is_double_battle", False)),
                "attacking_battle_stats": _stat_block(a.get("attacking_battle_stats")),
                "defending_battle_stats": _stat_block(a.get("defending_battle_stats")),
                "attacker_is_enemy": False,
                "result": _result(result),
            })
        return result

    gen3_calc.calculate_gen_three_damage = wrapped


def main(argv):
    out = argv[0] if argv else os.path.join(ROOT, "docs", "rust_port", "golden", "damage_cases.json")
    for cls in (gen_one_object.GenOne, gen_two_object.GenTwo, gen_three_object.GenThree, gen_four_object.GenFour, gen_five_object.GenFive):
        _wrap_calc(cls)
        _wrap_crit(cls)
        _wrap_accuracy(cls)
    _wrap_gen1_module_crit()
    _wrap_gen3_module_calc()
    tests = [os.path.join(ROOT, "tests", f) for f in (
        "test_damage_gen1.py", "test_damage_gen2.py", "test_damage_gen3.py",
        "test_damage_gen4.py", "test_damage_gen5.py", "test_unit_calcs.py",
    )]
    rc = pytest.main(["-q", "-p", "no:cacheprovider"] + tests)
    with open(out, "w", encoding="utf-8") as f:
        json.dump({"pytest_exit_code": int(rc), "cases": CASES}, f, indent=1, ensure_ascii=True)
    kinds = {}
    for c in CASES:
        kinds[c["kind"]] = kinds.get(c["kind"], 0) + 1
    print(f"pytest exit code {int(rc)}; recorded {len(CASES)} cases {kinds} -> {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
