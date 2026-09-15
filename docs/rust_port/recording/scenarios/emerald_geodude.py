"""Emerald, Geodude solo: a scripted session covering every recorder path.

Property paths and values follow the Emerald mapper the recorder expects
(``route_recording/game_recorders/gen_three/emerald_gamehook_constants.py``).
The route to record into is a fresh Emerald / Geodude route (see run_pair.py).
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from mock_gamehook import Scenario  # noqa: E402

TICK = "gametime.seconds"

# the route both apps record into (created fresh by run_pair.py)
ROUTE = {"species": "Geodude", "version": "Emerald", "dvs": None}

# trainer ids from raw_pkmn_data/gen_three/emerald/trainers.json
CALVIN = 318          # Youngster Calvin, Route102: Poochyena L5, $80
RICK = 615            # Bug Catcher Rick, Route102: Wurmple L4, Wurmple L4, $64
ALLEN = 333           # Youngster Allen, Route102: Zigzagoon L4, Taillow L3, $48
GINA_MIA = 483        # Twins Gina & Mia, Route104 (double): Seedot L6, Lotad L6, $144
CINDY = 114           # Lady Cindy, Route104: Zigzagoon L7, $1400
WALLACE = 335         # Champion Wallace, EverGrandeCity: Wailord L57 ... Milotic L58, $11600


def initial_properties() -> dict:
    p = {}
    p["pointers.dma1"] = 0x2024000
    p["pointers.dma2"] = 0x2025000
    p["pointers.dma3"] = 0x2026000
    p["overworld.mapName"] = "LITTLEROOT_TOWN"
    p["player.playerId"] = 54321
    p["player.bag.money"] = 3000
    p["player.team.0.expPoints"] = 0
    p["player.team.0.level"] = 0
    p["player.team.0.species"] = None
    p["player.team.0.itemHeld"] = None
    p["player.team.0.friendship"] = 70
    for i in range(6):
        p[f"player.team.{i}.species"] = None
        p[f"player.team.{i}.level"] = 0
        for iv in ("ivAttack", "ivDefense", "ivSpeed", "ivSpecialAttack", "ivSpecialDefense"):
            p[f"player.team.{i}.{iv}"] = 0
    for m in range(1, 5):
        p[f"player.team.0.move{m}"] = None
    for ev in ("evHp", "evAttack", "evDefense", "evSpeed", "evSpecialAttack", "evSpecialDefense"):
        p[f"player.team.0.{ev}"] = 0
    p["gametime.seconds"] = 0
    p["gametime.frames"] = 0
    p["battle.type.is_battle"] = False
    p["battle.type.trainer"] = False
    p["battle.type.double"] = False
    p["battle.type.two_opponents"] = False
    p["battle.type.old_man_tutorial"] = False
    p["battle.outcome"] = None
    p["battle.turnInfo.battleBackgroundTiles"] = 0
    p["battle.yourPokemon.partyPos"] = 0
    p["battle.yourPokemon.hp"] = 0
    p["battle.yourSecondPokemon.partyPos"] = 0
    p["battle.yourSecondPokemon.hp"] = 0
    p["battle.trainer.opponentAId"] = 0
    p["battle.trainer.opponentBId"] = 0
    for side in ("enemyPokemon", "enemySecondPokemon"):
        p[f"battle.{side}.species"] = None
        p[f"battle.{side}.level"] = 0
        p[f"battle.{side}.hp"] = 0
        p[f"battle.{side}.partyPos"] = 0
    for i in range(6):
        p[f"battle.trainer.team.{i}.species"] = None
    p["audio.soundEffect1"] = 0
    p["audio.soundEffect2"] = 0
    p["pointers.sStpTracking"] = None
    for i in range(30):
        p[f"player.bag.items.{i}.item"] = None
        p[f"player.bag.items.{i}.quantity"] = 0
    for i in range(16):
        p[f"player.bag.pokeBalls.{i}.item"] = None
        p[f"player.bag.pokeBalls.{i}.quantity"] = 0
    for i in range(46):
        p[f"player.bag.berries.{i}.item"] = None
        p[f"player.bag.berries.{i}.quantity"] = 0
    for i in range(30):
        p[f"player.bag.keyItems.{i}.item"] = None
    for i in range(64):
        p[f"player.bag.tmhm.{i}.item"] = None
        p[f"player.bag.tmhm.{i}.quantity"] = 0
    # the player already carries a couple of things
    p["player.bag.items.0.item"] = "POTION"
    p["player.bag.items.0.quantity"] = 2
    p["player.bag.keyItems.0.item"] = "POKENAV"
    return p


class Game:
    """Small helpers that keep the scenario readable."""

    def __init__(self, s: Scenario):
        self.s = s
        self.exp = 0
        self.money = 3000

    # -- overworld --------------------------------------------------------
    def go_to(self, map_name):
        # walking to another map takes a while: let both apps drain their event
        # queues first (Python's processing thread polls every 100 ms and needs a
        # UI round trip per event, so it can lag a fast scripted map change)
        self.s.wait(0.75)
        self.s.set("overworld.mapName", map_name)
        self.s.tick(1)

    def gain_exp(self, amount):
        self.exp += amount
        self.s.set("player.team.0.expPoints", self.exp)

    def set_money(self, value):
        self.money = value
        self.s.set("player.bag.money", value)

    # -- battles ------------------------------------------------------------
    def start_wild(self, species, level, hp, my_hp=30):
        s = self.s
        s.note(f"wild {species} L{level}")
        # a new battle: the outcome clears first (OVERWORLD -> BATTLE on that change once one battle was won)
        s.set("battle.type.trainer", False, "battle.type.double", False)
        s.set("battle.enemyPokemon.species", species, "battle.enemyPokemon.level", level, "battle.enemyPokemon.hp", hp, "battle.enemyPokemon.partyPos", 0)
        s.set("battle.yourPokemon.partyPos", 0, "battle.yourPokemon.hp", my_hp)
        s.set("battle.outcome", None)
        s.set("battle.type.is_battle", True)
        s.set("battle.turnInfo.battleBackgroundTiles", 44)
        s.tick(4)  # BATTLE, then the delayed initialisation fires

    def start_trainer(self, trainer_id, team, double=False, ally_pos=None, my_hp=30):
        """team: list of (species, level, hp); double battles send out the first two."""
        s = self.s
        s.note(f"trainer {trainer_id} {team}")
        # the battle-type bits, ids and teams are in memory before the battle flag flips
        s.set("battle.type.trainer", True, "battle.type.double", double, "battle.trainer.opponentAId", trainer_id, "battle.trainer.opponentBId", 0)
        for i in range(6):
            s.set(f"battle.trainer.team.{i}.species", team[i][0] if i < len(team) else None)
        s.set("battle.enemyPokemon.species", team[0][0], "battle.enemyPokemon.level", team[0][1], "battle.enemyPokemon.hp", team[0][2], "battle.enemyPokemon.partyPos", 0)
        if double and len(team) > 1:
            s.set("battle.enemySecondPokemon.species", team[1][0], "battle.enemySecondPokemon.level", team[1][1], "battle.enemySecondPokemon.hp", team[1][2], "battle.enemySecondPokemon.partyPos", 1)
        s.set("battle.yourPokemon.partyPos", 0, "battle.yourPokemon.hp", my_hp)
        if ally_pos is not None:
            s.set("battle.yourSecondPokemon.partyPos", ally_pos, "battle.yourSecondPokemon.hp", 20)
        s.set("battle.outcome", None)
        s.set("battle.type.is_battle", True)
        s.set("battle.turnInfo.battleBackgroundTiles", 44)
        s.tick(4)

    def ko_first(self, exp_gain):
        self.s.set("battle.enemyPokemon.hp", 0)
        self.s.tick(1)
        self.gain_exp(exp_gain)
        self.s.tick(1)

    def ko_second(self, exp_gain):
        self.s.set("battle.enemySecondPokemon.hp", 0)
        self.s.tick(1)
        self.gain_exp(exp_gain)
        self.s.tick(1)

    def next_enemy(self, party_pos, species, level, hp):
        self.s.set("battle.enemyPokemon.partyPos", party_pos)
        self.s.set("battle.enemyPokemon.species", species, "battle.enemyPokemon.level", level, "battle.enemyPokemon.hp", hp)
        self.s.tick(1)

    def level_up(self, new_level, learn_move=None, into_slot=None):
        """A level-up mid battle; optionally the mon learns a move into a slot."""
        s = self.s
        s.set("player.team.0.level", new_level)
        if learn_move is not None:
            s.tick(1)
            s.set(f"player.team.0.move{into_slot}", learn_move)
        s.tick(4)  # the delayed level / move updates fire after 3 seconds

    def end_battle(self, outcome="WON", prize=0):
        s = self.s
        if prize:
            self.set_money(self.money + prize)
        s.set("battle.outcome", outcome)
        s.tick(1)
        s.set("battle.turnInfo.battleBackgroundTiles", 0)
        s.set("battle.type.is_battle", False, "battle.type.trainer", False, "battle.type.double", False)
        s.tick(2)


def build() -> Scenario:
    s = Scenario("Pokemon Emerald - Deprecated Mapper", initial_properties(), TICK, tick_delay=0.05, step_delay=0.05)
    g = Game(s)

    s.note("boot: uninitialised -> overworld")
    s.tick(4)
    # the overworld waits out the empty slot 1 before it looks at anything
    s.tick(3)

    s.note("receive the starter")
    s.set(
        "player.team.0.species", "Geodude", "player.team.0.level", 5, "player.team.0.expPoints", 135,
        "player.team.0.ivAttack", 31, "player.team.0.ivDefense", 31, "player.team.0.ivSpeed", 31,
        "player.team.0.ivSpecialAttack", 31, "player.team.0.ivSpecialDefense", 31,
        "player.team.0.move1", "TACKLE", "player.team.0.move2", "DEFENSE CURL",
    )
    g.exp = 135
    s.tick(4)  # registration delay -> team cache picks up the solo mon

    s.note("pick up 5 poke balls in Littleroot")
    s.set("player.bag.pokeBalls.0.item", "POKE BALL", "player.bag.pokeBalls.0.quantity", 5)
    s.tick(4)

    g.go_to("ROUTE_101")
    g.start_wild("Zigzagoon", 2, 11)
    g.ko_first(8)
    g.end_battle()

    g.start_wild("Wurmple", 2, 12)
    g.ko_first(9)
    g.end_battle()

    g.go_to("OLDALE_TOWN")
    s.note("heal at the Oldale center")
    s.set("pointers.sStpTracking", "HEAL")
    s.tick(2)
    s.set("pointers.sStpTracking", None)
    s.tick(1)

    s.note("buy 9 repels (item first, then the money) and 1 potion")
    s.set("player.bag.items.1.item", "REPEL", "player.bag.items.1.quantity", 9)
    g.set_money(3000 - 9 * 350)
    s.tick(4)
    s.set("player.bag.items.0.quantity", 3)
    g.set_money(g.money - 300)
    s.tick(4)

    g.go_to("ROUTE_103")
    g.start_wild("Wingull", 2, 12)
    g.ko_first(20)
    g.level_up(6, learn_move="MUD SPORT", into_slot=3)  # Geodude learns Mud Sport at 6
    g.end_battle()

    s.note("Rival fight (May, Mudkip)")
    g.start_trainer(535, [("Mudkip", 5, 20)])
    g.ko_first(30)
    g.level_up(7)
    g.end_battle(prize=300)

    g.go_to("ROUTE_102")
    g.start_trainer(CALVIN, [("Poochyena", 5, 19)])
    g.ko_first(28)
    g.end_battle(prize=80)

    s.note("two-mon trainer")
    g.start_trainer(RICK, [("Wurmple", 4, 16), ("Wurmple", 4, 16)])
    g.ko_first(18)
    g.next_enemy(1, "Wurmple", 4, 16)
    g.ko_first(18)
    g.level_up(8)
    g.end_battle(prize=64)

    s.note("pick up an oran berry, then use a potion")
    s.set("player.bag.berries.0.item", "ORAN BERRY", "player.bag.berries.0.quantity", 1)
    s.tick(4)
    s.set("player.bag.items.0.quantity", 2)
    s.tick(4)

    s.note("give the oran berry to Geodude to hold")
    s.set("player.team.0.itemHeld", "ORAN BERRY")
    s.set("player.bag.berries.0.item", None, "player.bag.berries.0.quantity", 0)
    s.tick(4)

    s.note("rare candy: level 9")
    s.set("player.bag.items.2.item", "RARE CANDY", "player.bag.items.2.quantity", 1)
    s.tick(4)
    s.set("player.team.0.level", 9)
    s.tick(1)
    s.set("player.bag.items.2.item", None, "player.bag.items.2.quantity", 0)
    s.tick(4)

    s.note("protein")
    s.set("player.bag.items.2.item", "PROTEIN", "player.bag.items.2.quantity", 1)
    s.tick(4)
    s.set("player.team.0.evAttack", 10)
    s.tick(1)
    s.set("player.bag.items.2.item", None, "player.bag.items.2.quantity", 0)
    s.tick(4)

    s.note("TM39 Rock Tomb over Tackle")
    s.set("player.bag.tmhm.0.item", "TM39", "player.bag.tmhm.0.quantity", 1)
    s.tick(4)
    s.set("player.team.0.move1", "ROCK TOMB")
    s.tick(1)
    s.set("player.bag.tmhm.0.item", None, "player.bag.tmhm.0.quantity", 0)
    s.tick(4)

    s.note("HM06 Rock Smash into the empty slot")
    s.set("player.bag.tmhm.1.item", "HM06", "player.bag.tmhm.1.quantity", 1)
    s.tick(4)
    s.set("player.team.0.move4", "ROCK SMASH")
    s.tick(4)

    s.note("trainer with a switch: Allen (Zigzagoon, Taillow), Geodude levels to 10")
    g.start_trainer(ALLEN, [("Zigzagoon", 4, 17), ("Taillow", 3, 15)])
    g.ko_first(16)
    g.next_enemy(1, "Taillow", 3, 15)
    g.ko_first(14)
    g.level_up(10)
    g.end_battle(prize=48)

    g.go_to("PETALBURG_CITY")
    s.note("save")
    s.set("pointers.sStpTracking", "SAVE")
    s.tick(2)
    s.set("pointers.sStpTracking", None)
    s.tick(1)

    s.note("a wild fight in Petalburg Woods, then a soft reset back to the save")
    g.go_to("PETALBURG_WOODS")
    g.start_wild("Shroomish", 5, 20)
    g.ko_first(25)
    g.end_battle()
    s.set("player.playerId", 0, "pointers.dma1", 0)
    s.tick(2)
    s.set("player.playerId", 54321, "pointers.dma1", 0x2024000)
    s.set("overworld.mapName", "PETALBURG_CITY")
    s.tick(4)

    s.note("catch a Zigzagoon (team change) on route 104")
    g.go_to("ROUTE_104")
    g.start_wild("Zigzagoon", 3, 13)
    s.set("player.bag.pokeBalls.0.quantity", 4)
    s.tick(2)
    s.set("player.team.1.species", "Zigzagoon", "player.team.1.level", 3, "player.team.1.ivAttack", 5, "player.team.1.ivDefense", 6, "player.team.1.ivSpeed", 7, "player.team.1.ivSpecialAttack", 8, "player.team.1.ivSpecialDefense", 9)
    g.end_battle()

    s.note("double battle: Twins Gina & Mia with the Zigzagoon as ally")
    g.start_trainer(GINA_MIA, [("Seedot", 6, 22), ("Lotad", 6, 23)], double=True, ally_pos=1)
    g.ko_first(20)
    g.ko_second(21)
    g.end_battle(prize=144)

    s.note("lose to Lady Cindy: trainer loss + blackout, back to the Petalburg heal")
    g.start_trainer(CINDY, [("Zigzagoon", 7, 25)])
    s.set("battle.yourPokemon.hp", 0)
    s.tick(2)
    g.end_battle(outcome="LOST")
    s.note("the whiteout halves the money in the overworld: a money change with no bag change")
    g.set_money(g.money // 2)
    s.tick(4)
    g.go_to("PETALBURG_CITY")
    s.set("pointers.sStpTracking", "HEAL")
    s.tick(2)
    s.set("pointers.sStpTracking", None)
    s.tick(1)

    s.note("sell a potion right after the whiteout; the money lands in the same batch, before the bag slot")
    # the sale price (75) is below the halved amount, so a recorder that missed the
    # halving compares against the old total and calls this a Use/Drop
    g.money += 75
    s.set("player.bag.money", g.money, "player.bag.items.0.quantity", 1)
    s.tick(4)

    s.note("Route 104 again (a second trip), one more wild fight")
    g.go_to("ROUTE_104")
    g.start_wild("Taillow", 4, 16)
    g.ko_first(17)
    g.end_battle()

    s.note("champion: save, beat Wallace, then the Hall of Fame autosave and the credits reboot")
    g.go_to("EVER_GRANDE_CITY")
    s.set("pointers.sStpTracking", "SAVE")
    s.tick(2)
    s.set("pointers.sStpTracking", None)
    s.tick(1)
    g.start_trainer(WALLACE, [("Wailord", 57, 200), ("Tentacruel", 55, 180), ("Ludicolo", 56, 190), ("Whiscash", 56, 190), ("Gyarados", 56, 190), ("Milotic", 58, 200)])
    g.ko_first(2000)
    g.next_enemy(1, "Tentacruel", 55, 180)
    g.ko_first(1900)
    g.next_enemy(2, "Ludicolo", 56, 190)
    g.ko_first(1900)
    g.next_enemy(3, "Whiscash", 56, 190)
    g.ko_first(1900)
    g.next_enemy(4, "Gyarados", 56, 190)
    g.ko_first(2000)
    g.next_enemy(5, "Milotic", 58, 200)
    g.ko_first(2100)
    g.level_up(11)
    g.end_battle(prize=11600)
    # the credits end with a reboot to the title screen; the game itself saved during
    # the Hall of Fame, so nothing between the Ever Grande save and here may be lost
    s.wait(0.75)
    s.set("player.playerId", 0, "pointers.dma1", 0)
    s.tick(2)
    s.set("player.playerId", 54321, "pointers.dma1", 0x2024000)
    s.set("overworld.mapName", "LITTLEROOT_TOWN")
    s.tick(4)

    s.note("post-game: pick up the S.S. Ticket")
    s.set("player.bag.keyItems.1.item", "SS TICKET")
    s.tick(4)

    s.note("done")
    s.tick(3)
    return s
