"""Emerald (deprecated mapper), "Start Recording" from the landing page.

Like ``quickstart_yellow`` for gen 3: the starter (Geodude) brings an IV
spread, a nature and the second-ability bit, all of which the route must
pick up. Played with ``--shared-playback``.

Expected route: Emerald / Geodude / IVs 31-30-29-28-27-26 (hp, atk, def, spe,
spa, spd) / Adamant (3) / ability 1 (Sturdy), events after the starter.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mock_gamehook import Scenario  # noqa: E402
from emerald_geodude import Game, initial_properties  # noqa: E402

TICK = "gametime.seconds"
EXPECTED = {
    "species": "Geodude",
    "version": "Emerald",
    "dvs": {"hp": 31, "attack": 30, "defense": 29, "speed": 28, "special_attack": 27, "special_defense": 26},
    "nature": 3,
    "ability": 1,
}


def build() -> Scenario:
    props = initial_properties()
    props["player.teamCount"] = 0
    props["player.team.0.ivHp"] = 0
    props["player.team.0.nature"] = "Hardy"
    props["player.team.0.ability"] = False
    s = Scenario("Pokemon Emerald - Deprecated Mapper", props, TICK, tick_delay=0.05, step_delay=0.05)
    g = Game(s)

    s.note("boot: empty party for a while")
    s.tick(4)
    s.wait(2.0)

    s.note("receive the starter")
    s.set("player.team.0.species", "Geodude", "player.team.0.level", 5, "player.team.0.expPoints", 135, "player.teamCount", 1)
    s.wait(0.2)
    s.set(
        "player.team.0.ivHp", 31, "player.team.0.ivAttack", 30, "player.team.0.ivDefense", 29, "player.team.0.ivSpeed", 28,
        "player.team.0.ivSpecialAttack", 27, "player.team.0.ivSpecialDefense", 26,
        "player.team.0.nature", "Adamant", "player.team.0.ability", True,
        "player.team.0.move1", "TACKLE", "player.team.0.move2", "DEFENSE CURL",
    )
    g.exp = 135
    s.tick(2)
    # the app settles for a second, creates the route and reconnects with the recorder
    s.wait(8.0)
    s.tick(8)

    g.go_to("ROUTE_101")
    g.start_wild("Zigzagoon", 2, 11)
    g.ko_first(8)
    g.end_battle()
    g.start_wild("Wurmple", 2, 12)
    g.ko_first(9)
    g.end_battle()
    s.tick(4)
    return s
