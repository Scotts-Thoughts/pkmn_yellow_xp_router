"""Emerald, Geodude solo: Thief in trainer and wild battles (Rust only).

The Python recorder records an in-battle steal as a ``Hold Item`` of an item
that never reached the bag and records nothing when the item is taken off
again in the overworld, so this scenario has no Python counterpart: run it
with ``run_pair.py --scenario emerald_thief --only rust`` and check the
saved route by hand (or with the engine: it must load without errors).

Expected events, in order:

* Lady Cindy with ``thief_mons: [0]`` (the steal is detected mid battle)
* ``Hold Item`` (hold nothing): the Nugget is taken off in the overworld
* Rich Boy Winston with ``thief_mons: [0]`` (the steal is the KO on the last
  mon, so it is only seen when the battle ends)
* ``Hold Item`` (hold nothing), then ``Sell Nugget``
* a wild Zigzagoon, then ``Acquire Oran Berry`` and ``Hold Oran Berry``
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from mock_gamehook import Scenario  # noqa: E402
from scenarios.emerald_geodude import CINDY, TICK, Game, initial_properties  # noqa: E402

ROUTE = {"species": "Geodude", "version": "Emerald", "dvs": None}

WINSTON = 136         # Rich Boy Winston, Route104: Zigzagoon L7 (Nugget), $1400


def build() -> Scenario:
    s = Scenario("Pokemon Emerald - Deprecated Mapper", initial_properties(), TICK, tick_delay=0.05, step_delay=0.05)
    g = Game(s)

    s.note("boot: uninitialised -> overworld")
    s.tick(4)
    s.tick(3)

    s.note("receive the starter")
    s.set(
        "player.team.0.species", "Geodude", "player.team.0.level", 5, "player.team.0.expPoints", 135,
        "player.team.0.ivAttack", 31, "player.team.0.ivDefense", 31, "player.team.0.ivSpeed", 31,
        "player.team.0.ivSpecialAttack", 31, "player.team.0.ivSpecialDefense", 31,
        "player.team.0.move1", "TACKLE", "player.team.0.move2", "THIEF",
    )
    g.exp = 135
    s.tick(4)

    g.go_to("ROUTE_104")
    s.note("Lady Cindy: Thief the Nugget, then KO a turn later")
    g.start_trainer(CINDY, [("Zigzagoon", 7, 22)])
    s.set("player.team.0.itemHeld", "NUGGET")
    s.tick(4)  # the delayed held-item update fires
    g.ko_first(60)
    g.end_battle(prize=1400)

    s.note("take the Nugget off in the overworld: the bag gains it, the mon holds nothing")
    s.set("player.bag.items.1.item", "NUGGET", "player.bag.items.1.quantity", 1)
    s.set("player.team.0.itemHeld", None)
    s.tick(4)

    s.note("Rich Boy Winston: the Thief hit is the KO")
    g.start_trainer(WINSTON, [("Zigzagoon", 7, 22)])
    s.tick(2)
    s.set("player.team.0.itemHeld", "NUGGET")
    g.ko_first(60)
    g.end_battle(prize=1400)

    s.note("take the second Nugget off too")
    s.set("player.bag.items.1.quantity", 2)
    s.set("player.team.0.itemHeld", None)
    s.tick(4)

    g.go_to("RUSTBORO_CITY")
    s.note("sell both Nuggets")
    g.set_money(g.money + 10000)
    s.set("player.bag.items.1.item", None, "player.bag.items.1.quantity", 0)
    s.tick(4)

    g.go_to("ROUTE_116")
    s.note("a wild Zigzagoon holding an Oran Berry: Thief it, then KO")
    g.start_wild("Zigzagoon", 5, 18)
    s.set("player.team.0.itemHeld", "ORAN BERRY")
    s.tick(4)
    g.ko_first(20)
    g.end_battle()

    s.note("done")
    s.tick(4)
    return s
