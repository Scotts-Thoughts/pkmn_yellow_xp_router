"""Battle-summary golden records (rust_port_plan.md §5.1, Phase 2).

For every trainer / wild event group of a loaded route, run the Python
BattleSummaryController exactly the way the Qt event-details pane does
(``load_from_event`` for trainer fights, ``load_from_state(..., is_wild=True)``
for wild fights) and dump everything it computed.
"""
from routing import route_events
from controllers.battle_summary_controller import BattleSummaryController


class _FakeMainController:
    """The controller only needs ``get_raw_route()`` (for the test moves)."""

    def __init__(self, router):
        self._router = router

    def get_raw_route(self):
        return self._router


def _stage(mod):
    if mod is None:
        return None
    return {
        "attack_stage": mod.attack_stage,
        "defense_stage": mod.defense_stage,
        "speed_stage": mod.speed_stage,
        "special_attack_stage": mod.special_attack_stage,
        "special_defense_stage": mod.special_defense_stage,
        "accuracy_stage": mod.accuracy_stage,
        "evasion_stage": mod.evasion_stage,
        "attack_badge_boosts": mod.attack_badge_boosts,
        "defense_badge_boosts": mod.defense_badge_boosts,
        "speed_badge_boosts": mod.speed_badge_boosts,
        "special_badge_boosts": mod.special_badge_boosts,
    }


def _field(f):
    if f is None:
        return None
    return {
        "light_screen": f.light_screen,
        "reflect": f.reflect,
        "gravity": f.gravity,
        "magnet_rise": f.magnet_rise,
        "miracle_eye": f.miracle_eye,
        "power_trick": f.power_trick,
        "roost": f.roost,
        "tailwind": f.tailwind,
        "trick_room": f.trick_room,
        "worry_seed": f.worry_seed,
        "gastro_acid": f.gastro_acid,
        "slow_start": f.slow_start,
    }


def _move_info(m):
    d = m.serialize()
    # a (data-quirk) non-string flavor entry is kept as its Python repr on the
    # Rust side; neither side ever matches it against a flavor constant
    d["attack_flavor"] = [f if isinstance(f, str) else str(f) for f in d["attack_flavor"]]
    return d


def _moves(data):
    return [[None if m is None else _move_info(m) for m in row] for row in data]


def _summary(ctrl):
    partial = ctrl.get_partial_trainer_definition()
    return {
        "trainer_name": ctrl._trainer_name or "",
        "second_trainer_name": ctrl._second_trainer_name or "",
        "double_battle": ctrl.is_double_battle(),
        "using_global_setup": ctrl._using_global_setup,
        "mimic_options": list(ctrl._mimic_options),
        "mimic_selection": ctrl._mimic_selection,
        "player_matchups": [p.serialize() for p in ctrl._player_pkmn_matchup_data],
        "enemy_matchups": [p.serialize() for p in ctrl._enemy_pkmn_matchup_data],
        "player_matchup_strings": [str(p) for p in ctrl._player_pkmn_matchup_data],
        "enemy_matchup_strings": [str(p) for p in ctrl._enemy_pkmn_matchup_data],
        "player_moves": _moves(ctrl._player_move_data),
        "enemy_moves": _moves(ctrl._enemy_move_data),
        "partial_trainer_definition": None if partial is None else partial.serialize(),
        "player_field_statuses": [_field(f) for f in ctrl._player_field_statuses],
        "enemy_field_statuses": [_field(f) for f in ctrl._enemy_field_statuses],
        "per_matchup_player_modifiers": [_stage(m) for m in ctrl._per_matchup_player_modifiers],
        "per_matchup_enemy_modifiers": [_stage(m) for m in ctrl._per_matchup_enemy_modifiers],
    }


def _groups(folder, out):
    for child in folder.children:
        if isinstance(child, route_events.EventFolder):
            _groups(child, out)
        else:
            out.append(child)


def dump_battles(router):
    ctrl = BattleSummaryController(_FakeMainController(router))
    groups = []
    _groups(router.root_folder, groups)
    result = []
    for idx, group in enumerate(groups):
        ed = group.event_definition
        rec = {"group_index": idx, "group_name": group.name}
        try:
            if ed.trainer_def is not None:
                rec["kind"] = "trainer"
                ctrl.load_from_event(group)
            elif ed.wild_pkmn_info is not None:
                wild_pkmn = ed.get_pokemon_list()
                if wild_pkmn and group.init_state is not None:
                    rec["kind"] = "wild"
                    ctrl.load_from_state(group.init_state, wild_pkmn, is_wild=True)
                else:
                    continue
            else:
                continue
            rec["summary"] = _summary(ctrl)
        except Exception as e:  # keep going; the Rust side must fail the same way
            rec["error"] = f"{type(e).__name__}: {e}"
        result.append(rec)
    return result
