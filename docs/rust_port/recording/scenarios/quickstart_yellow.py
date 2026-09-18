"""Yellow (deprecated mapper), "Start Recording" from the landing page.

The party is empty when the router connects; the starter (Charmander, an
uneven DV spread) lands in slot 1 a few seconds later, and the scenario then
carries on with a couple of fights so the recorder that takes over after the
hand-off has something to record. Played with ``--shared-playback`` so the
steps keep flowing while the app swaps its quick-start client for the real
recorder.

Expected route: Yellow / Charmander / DVs 10-8-5-12 (HP DV 2), events after
the starter.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mock_gamehook import Scenario  # noqa: E402
from yellow_charmander import Game, initial_properties  # noqa: E402

TICK = "gameTime.seconds"
EXPECTED = {"species": "Charmander", "version": "Yellow", "dvs": {"hp": 2, "attack": 10, "defense": 8, "speed": 5, "special_attack": 12, "special_defense": 12}}


def build() -> Scenario:
    props = initial_properties()
    props["player.teamCount"] = 0
    s = Scenario("Pokemon Yellow - Deprecated Mapper", props, TICK, tick_delay=0.05, step_delay=0.05)
    g = Game(s)

    s.note("boot: empty party for a while")
    s.tick(4)
    s.wait(2.0)

    s.note("receive the starter (fields arrive in two batches)")
    s.set("player.team.0.species", "Charmander", "player.team.0.level", 5, "player.teamCount", 1)
    s.wait(0.3)
    s.set(
        "player.team.0.expPoints", 135,
        "player.team.0.dvAttack", 10, "player.team.0.dvDefense", 8, "player.team.0.dvSpeed", 5, "player.team.0.dvSpecial", 12,
        "player.team.0.move1", "SCRATCH", "player.team.0.move2", "GROWL",
    )
    g.exp = 135
    s.tick(2)
    # the app settles for a second, creates the route and reconnects with the recorder
    s.wait(8.0)
    s.tick(4)

    s.note("rival lab fight, won")
    g.start_trainer("RIVAL1", 1, ("Eevee", 5))
    g.ko(30)
    g.level_up(6)
    g.end_battle(prize=175)

    g.go_to("Route 1")
    g.start_wild("Pidgey", 3)
    g.ko(15)
    g.end_battle()
    g.start_wild("Rattata", 2)
    g.ko(12)
    g.end_battle()
    s.tick(4)
    return s
