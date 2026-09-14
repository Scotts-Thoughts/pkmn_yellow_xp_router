"""
EV-lowering berries (Pomeg, Kelpsy, Qualot, Hondew, Grepa, Tamato).

Every expectation below is the game's own rule, not the router's:
  * Emerald / gen 5: EV - 10, floored at 0.
  * Platinum (unk_02096420.c CalculateEVUpdate) and HGSS (use_item_on_mon.c TryModEV):
    ev += -10, THEN clamp anything over 100 down to 100, floor at 0.
    So 255 -> 100, 110 -> 100, but 105 -> 95.
Eating one recalculates stats, so EVs banked from battles without a level-up
are realized along with the reduction.
"""
import json
import math

import pytest

from pkmn.gen_factory import current_gen_info
from pkmn import universal_utils
from routing.router import Router
from routing.state_objects import SoloPokemon
from utils.constants import const


SPECIES = "Geodude"

BERRY_STAT = [
    ("Pomeg Berry", "hp"),
    ("Kelpsy Berry", "attack"),
    ("Qualot Berry", "defense"),
    ("Hondew Berry", "special_attack"),
    ("Grepa Berry", "special_defense"),
    ("Tamato Berry", "speed"),
]
STAT_ORDER = ["hp", "attack", "defense", "special_attack", "special_defense", "speed"]


def expected_after_berry(version, ev):
    if version in (const.PLATINUM_VERSION, const.HEART_GOLD_VERSION):
        ev -= 10
        if ev > 100:
            ev = 100
        return max(ev, 0)
    return max(ev - 10, 0)


def new_router(version):
    router = Router()
    router.new_route(SPECIES, pkmn_version=version)
    return router


def evs(**kw):
    vals = [kw.get(s, 0) for s in STAT_ORDER]
    return current_gen_info().make_stat_block(*vals, is_stat_xp=True)


def ev_tuple(block):
    return tuple(getattr(block, s) for s in STAT_ORDER)


def mon_with(router, realized, unrealized=None, level=50):
    base = router.init_route_state.solo_pkmn
    xp = universal_utils.level_lookups[base.species_def.growth_rate].get_xp_for_level(level)
    return SoloPokemon(
        base.name, base.species_def, base.dvs, base.badges, base._empty_stat_block,
        base.ability_idx, base.nature,
        move_list=base.move_list, cur_xp=xp,
        realized_stat_xp=realized, unrealized_stat_xp=unrealized,
    )


def state_with(router, mon, berry=None, count=5):
    init = router.init_route_state
    inv = init.inventory
    if berry is not None:
        inv = inv.add_item(current_gen_info().item_db().get_item(berry), count)
    return type(init)(mon, init.badges, inv)


def formula_stat(base, iv, ev, level, is_hp):
    core = math.floor((2 * base + iv + math.floor(ev / 4)) * level / 100)
    return core + level + 10 if is_hp else core + 5


def bag_count(inventory, item_name):
    return sum(x.num for x in inventory.cur_items if x.base_item.name == item_name)


GEN3_TO_5 = [const.EMERALD_VERSION, const.PLATINUM_VERSION, const.HEART_GOLD_VERSION, const.BLACK_VERSION]


@pytest.mark.parametrize("version", GEN3_TO_5)
@pytest.mark.parametrize("berry,stat", BERRY_STAT)
def test_each_berry_lowers_only_its_own_stat(version, berry, stat):
    router = new_router(version)
    start = {s: 50 for s in STAT_ORDER}
    state = state_with(router, mon_with(router, evs(**start)), berry)

    after, err = state.vitamin(berry)

    assert err == ""
    want = dict(start)
    want[stat] = 40
    assert ev_tuple(after.solo_pkmn.realized_stat_xp) == tuple(want[s] for s in STAT_ORDER)
    assert ev_tuple(after.solo_pkmn.unrealized_stat_xp) == tuple(want[s] for s in STAT_ORDER)


@pytest.mark.parametrize("version", GEN3_TO_5)
@pytest.mark.parametrize("before", [255, 252, 111, 110, 109, 105, 101, 100, 50, 10, 5, 1])
def test_reduction_matches_the_game(version, before):
    router = new_router(version)
    state = state_with(router, mon_with(router, evs(hp=before)), "Pomeg Berry")

    after, err = state.vitamin("Pomeg Berry")

    assert err == ""
    assert after.solo_pkmn.realized_stat_xp.hp == expected_after_berry(version, before)
    assert after.solo_pkmn.realized_stat_xp.hp < before


@pytest.mark.parametrize("version", GEN3_TO_5)
@pytest.mark.parametrize("berry,stat", BERRY_STAT)
def test_stats_are_recalculated_from_the_lowered_evs(version, berry, stat):
    router = new_router(version)
    level = 100
    state = state_with(router, mon_with(router, evs(**{stat: 252}), level=level), berry)
    before_stat = getattr(state.solo_pkmn.cur_stats, stat)

    after, _ = state.vitamin(berry)
    mon = after.solo_pkmn

    ev_after = getattr(mon.realized_stat_xp, stat)
    want = formula_stat(getattr(mon.species_def.stats, stat), 31, ev_after, level, stat == "hp")
    assert getattr(mon.cur_stats, stat) == want
    assert getattr(mon.cur_stats, stat) < before_stat


@pytest.mark.parametrize("version", GEN3_TO_5)
def test_banked_battle_evs_are_realized_then_lowered(version):
    router = new_router(version)
    state = state_with(router, mon_with(router, evs(), unrealized=evs(hp=30, speed=8)), "Pomeg Berry")

    after, err = state.vitamin("Pomeg Berry")

    assert err == ""
    assert ev_tuple(after.solo_pkmn.realized_stat_xp) == (20, 0, 0, 0, 0, 8)
    assert ev_tuple(after.solo_pkmn.unrealized_stat_xp) == (20, 0, 0, 0, 0, 8)


@pytest.mark.parametrize("version", GEN3_TO_5)
def test_berry_on_zero_evs_never_goes_negative_and_is_flagged(version):
    router = new_router(version)
    state = state_with(router, mon_with(router, evs(attack=40)), "Pomeg Berry")

    after, err = state.vitamin("Pomeg Berry")

    assert "Ineffective Berry" in err
    assert ev_tuple(after.solo_pkmn.realized_stat_xp) == (0, 40, 0, 0, 0, 0)


@pytest.mark.parametrize("version", GEN3_TO_5)
def test_berry_is_removed_from_the_bag(version):
    router = new_router(version)
    state = state_with(router, mon_with(router, evs(hp=60)), "Pomeg Berry", count=3)

    after, err = state.vitamin("Pomeg Berry")

    assert err == ""
    assert bag_count(after.inventory, "Pomeg Berry") == 2


@pytest.mark.parametrize("version", GEN3_TO_5)
def test_lowering_frees_room_under_the_total_cap(version):
    router = new_router(version)
    full = evs(hp=255, attack=255)
    state = state_with(router, mon_with(router, full), "Kelpsy Berry")
    assert full.add(evs(speed=4)).speed == 0

    after, _ = state.vitamin("Kelpsy Berry")
    freed = after.solo_pkmn.realized_stat_xp
    assert sum(ev_tuple(freed)) < 510
    assert freed.add(evs(speed=4)).speed == 4


def _route_json(version, more_events):
    events = [
        {"Enabled": True, "Tags": [], "Inventory Event": ["Pomeg Berry", 10, True, False]},
        {"Enabled": True, "Tags": [], "Inventory Event": ["Tamato Berry", 10, True, False]},
    ]
    events += more_events
    return {
        "name": SPECIES,
        "Version": version,
        "ability": 0,
        "nature": 0,
        "events": [{"Event Folder Name": "ROOT", "Just Notes": "", "events": [
            {"Event Folder Name": "Main", "Just Notes": "", "Expanded": True, "Enabled": True, "events": events}
        ]}],
    }


# (speed EV yield, HP EV yield)
WILD = {
    const.EMERALD_VERSION: ("Zigzagoon", "Wurmple"),
    const.PLATINUM_VERSION: ("Starly", "Bidoof"),
    const.HEART_GOLD_VERSION: ("Pidgey", "Hoothoot"),
    const.BLACK_VERSION: ("Purrloin", "Audino"),
}


@pytest.mark.parametrize("version", GEN3_TO_5)
def test_route_events_subtract_evs_end_to_end(version, tmp_path):
    """Earn EVs in battle, eat 3 Tamato and 1 Pomeg through route events, save, reload, check again."""
    speed_mon, hp_mon = WILD[version]
    route = _route_json(version, [
        {"Enabled": True, "Tags": [], "Fight Wild Pkmn": [speed_mon, 3, 25, False]},
        {"Enabled": True, "Tags": [], "Fight Wild Pkmn": [hp_mon, 3, 15, False]},
        {"Enabled": True, "Tags": [], "Use Vitamin": ["Tamato Berry", 3]},
        {"Enabled": True, "Tags": [], "Use Vitamin": ["Pomeg Berry", 1]},
    ])
    path = tmp_path / "route.json"
    path.write_text(json.dumps(route))

    router = Router()
    router.load(str(path))
    groups = [c for c in router.root_folder.children[0].children]
    fight, tamato, pomeg = groups[3], groups[4], groups[5]

    before = fight.final_state.solo_pkmn.unrealized_stat_xp
    assert before.speed > 0 and before.hp > 0, "the wild fights should have given speed and HP EVs"

    spe = before.speed
    for _ in range(3):
        spe = expected_after_berry(version, spe)
    after_tamato = tamato.final_state.solo_pkmn
    assert after_tamato.realized_stat_xp.speed == spe
    assert after_tamato.unrealized_stat_xp.speed == spe
    for s in ("hp", "attack", "defense", "special_attack", "special_defense"):
        assert getattr(after_tamato.realized_stat_xp, s) == getattr(before, s)
    assert len(tamato.event_items) == 3
    assert "Berry" in str(tamato.event_definition)
    assert bag_count(tamato.final_state.inventory, "Tamato Berry") == 7

    assert tamato.error_messages == []

    after_pomeg = pomeg.final_state.solo_pkmn
    assert after_pomeg.realized_stat_xp.hp == expected_after_berry(version, before.hp)
    assert after_pomeg.realized_stat_xp.speed == spe
    assert pomeg.error_messages == []

    final_before_save = ev_tuple(router.get_final_state().solo_pkmn.realized_stat_xp)

    saved = tmp_path / "saved.json"
    saved.write_text(json.dumps(router.serialize()))
    reloaded = Router()
    reloaded.load(str(saved))
    assert ev_tuple(reloaded.get_final_state().solo_pkmn.realized_stat_xp) == final_before_save
