"""Black, Tepig solo: the gen 5 recorder paths.

Paths/values follow ``route_recording/game_recorders/gen_five/black_gamehook_constants.py``
(and its MAPPER_GAPS.md): there is no ``battle.mode`` / ``battle.outcome``
(trainer vs wild comes from ``battle.opponent.id``), no ball pocket, no
second-opponent / ally slots, no save or heal detection, and the enemy PID is
read from the lead slot.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from mock_gamehook import Scenario  # noqa: E402

TICK = "game_time.seconds"
ROUTE = {"species": "Tepig", "version": "Black", "dvs": None}

SOLO_PID = 51515
JIMMY = 1      # Youngster Jimmy: Patrat L7
JOEY = 7       # Youngster Joey: Patrat L7, Patrat L7, Lillipup L7
CHILI = 11     # Leader Chili: Lillipup L12, Pansear L14


def initial_properties() -> dict:
    p = {}
    p["meta.state"] = "Overworld"
    p["overworld.map_name"] = "Nuvema Town"
    p["player.player_id"] = 31337
    p["bag.money"] = 3000
    for i in range(6):
        p[f"player.team.{i}.species"] = None
        p[f"player.team.{i}.level"] = 0
        p[f"player.team.{i}.held_item"] = None
        p[f"player.team.{i}.stats.hp"] = 0
        p[f"battle.player.team.{i}.stats.hp"] = 0
        for st in ("hp", "attack", "defense", "speed", "special_attack", "special_defense"):
            p[f"player.team.{i}.ivs.{st}"] = 0
            p[f"player.team.{i}.evs.{st}"] = 0
    p["player.team.0.exp"] = 0
    p["battle.player.team.0.exp"] = 0
    p["player.team.0.friendship"] = 70
    p["player.team.0.internals.personality_value"] = 0
    for m in range(4):
        p[f"player.team.0.moves.{m}.move"] = None
    p["game_time.seconds"] = 0
    p["battle.other.outcome_flags"] = 0
    p["battle.player.party_position"] = 0
    p["battle.player.active_pokemon.stats.hp"] = 0
    p["battle.opponent.id"] = 0
    p["battle.opponent_2.id"] = 0
    p["battle.opponent.active_pokemon.species"] = None
    p["battle.opponent.active_pokemon.level"] = 0
    p["battle.opponent.active_pokemon.stats.hp"] = 0
    p["battle.opponent.party_position"] = 0
    for i in range(6):
        p[f"battle.opponent.team.{i}.species"] = None
        p[f"battle.opponent.team.{i}.internals.personality_value"] = 0
    for pocket, n in (("items", 40), ("medicine", 48), ("berries", 30), ("tmhm", 102)):
        for i in range(n):
            p[f"bag.{pocket}.{i}.item"] = None
            p[f"bag.{pocket}.{i}.quantity"] = 0
    p["bag.medicine.0.item"] = "Potion"
    p["bag.medicine.0.quantity"] = 2
    return p


class Game:
    def __init__(self, s: Scenario):
        self.s = s
        self.exp = 0
        self.money = 3000
        self.next_pid = 2000

    def pid(self):
        self.next_pid += 11
        return self.next_pid

    def go_to(self, map_name):
        self.s.wait(0.75)
        self.s.set("overworld.map_name", map_name)
        self.s.tick(1)

    def gain_exp(self, amount):
        self.exp += amount
        self.s.set("player.team.0.exp", self.exp, "battle.player.team.0.exp", self.exp)

    def set_money(self, value):
        self.money = value
        self.s.set("bag.money", value)

    def _enter_battle(self, my_hp):
        s = self.s
        s.set("battle.player.party_position", 0, "battle.player.active_pokemon.stats.hp", my_hp, "battle.player.team.0.stats.hp", my_hp)
        s.set("meta.state", "Battle")
        s.tick(4)

    def start_wild(self, species, level, hp, my_hp=30):
        s = self.s
        s.note(f"wild {species} L{level}")
        pid = self.pid()
        s.set("battle.opponent.id", 0, "battle.opponent_2.id", 0)
        s.set("battle.opponent.team.0.species", species, "battle.opponent.team.0.internals.personality_value", pid)
        s.set("battle.opponent.active_pokemon.species", species, "battle.opponent.active_pokemon.level", level, "battle.opponent.active_pokemon.stats.hp", hp, "battle.opponent.party_position", 0)
        self._enter_battle(my_hp)

    def start_trainer(self, trainer_id, team, my_hp=30):
        s = self.s
        s.note(f"trainer {trainer_id} {team}")
        s.set("battle.opponent.id", trainer_id, "battle.opponent_2.id", 0)
        pids = []
        for i in range(6):
            sp = team[i][0] if i < len(team) else None
            pid = self.pid() if sp else 0
            pids.append(pid)
            s.set(f"battle.opponent.team.{i}.species", sp, f"battle.opponent.team.{i}.internals.personality_value", pid)
        s.set("battle.opponent.active_pokemon.species", team[0][0], "battle.opponent.active_pokemon.level", team[0][1], "battle.opponent.active_pokemon.stats.hp", team[0][2], "battle.opponent.party_position", 0)
        self._enter_battle(my_hp)
        self._pids = pids

    def ko_first(self, exp_gain):
        self.s.set("battle.opponent.active_pokemon.stats.hp", 0)
        self.s.tick(1)
        self.gain_exp(exp_gain)
        self.s.tick(1)

    def next_enemy(self, party_pos, species, level, hp):
        self.s.set("battle.opponent.party_position", party_pos)
        self.s.set("battle.opponent.active_pokemon.species", species, "battle.opponent.active_pokemon.level", level, "battle.opponent.active_pokemon.stats.hp", hp)
        self.s.tick(1)

    def level_up(self, new_level, learn_move=None, into_slot=None):
        s = self.s
        s.set("player.team.0.level", new_level)
        if learn_move is not None:
            s.tick(1)
            s.set(f"player.team.0.moves.{into_slot}.move", learn_move)
        s.tick(4)

    def end_battle(self, prize=0):
        s = self.s
        if prize:
            self.set_money(self.money + prize)
        s.set("meta.state", "From Battle")
        s.tick(1)
        s.set("meta.state", "Overworld")
        s.set("battle.opponent.active_pokemon.species", None, "battle.opponent.team.0.species", None)
        s.tick(2)


def build() -> Scenario:
    s = Scenario("Pokemon Black - Beta", initial_properties(), TICK, tick_delay=0.05, step_delay=0.05)
    g = Game(s)

    s.note("boot")
    s.tick(4)
    s.tick(3)

    s.note("receive the starter")
    s.set(
        "player.team.0.species", "Tepig", "player.team.0.level", 5, "player.team.0.exp", 135, "battle.player.team.0.exp", 135,
        "player.team.0.stats.hp", 24, "player.team.0.internals.personality_value", SOLO_PID,
        "player.team.0.ivs.hp", 31, "player.team.0.ivs.attack", 31, "player.team.0.ivs.defense", 31, "player.team.0.ivs.speed", 31,
        "player.team.0.ivs.special_attack", 31, "player.team.0.ivs.special_defense", 31,
        "player.team.0.moves.0.move", "Tackle", "player.team.0.moves.1.move", "Tail Whip",
    )
    g.exp = 135
    s.tick(4)

    g.go_to("Route 1")
    g.start_wild("Patrat", 2, 12)
    g.ko_first(9)
    g.end_battle()
    g.start_wild("Lillipup", 3, 14)
    g.ko_first(14)
    g.level_up(6)
    g.end_battle()

    g.go_to("Accumula Town")
    s.note("buy potions, sell one, toss one")
    s.set("bag.medicine.0.quantity", 4)
    g.set_money(g.money - 600)
    s.tick(4)
    s.set("bag.medicine.0.quantity", 3)
    g.set_money(g.money + 150)
    s.tick(4)
    s.set("bag.medicine.0.quantity", 2)
    s.tick(4)

    g.go_to("Route 2")
    g.start_trainer(JIMMY, [("Patrat", 7, 25)])
    g.ko_first(40)
    g.level_up(7, learn_move="Ember", into_slot=2)
    g.end_battle(prize=4 * 4 * 7)

    g.start_trainer(JOEY, [("Patrat", 7, 25), ("Patrat", 7, 25), ("Lillipup", 7, 26)])
    g.ko_first(40)
    g.next_enemy(1, "Patrat", 7, 25)
    g.ko_first(40)
    g.next_enemy(2, "Lillipup", 7, 26)
    g.ko_first(45)
    g.level_up(8)
    g.end_battle(prize=4 * 4 * 7)

    s.note("rare candy, protein, TM, HM, move deleter, held item")
    g.go_to("Striaton City")
    s.set("bag.medicine.1.item", "Rare Candy", "bag.medicine.1.quantity", 1)
    s.tick(4)
    s.set("bag.medicine.1.item", None, "bag.medicine.1.quantity", 0)
    s.tick(1)
    s.set("player.team.0.level", 9)
    s.tick(4)
    s.set("bag.medicine.2.item", "Protein", "bag.medicine.2.quantity", 1)
    s.tick(4)
    s.set("bag.medicine.2.item", None, "bag.medicine.2.quantity", 0)
    s.tick(1)
    s.set("player.team.0.evs.attack", 10)
    s.tick(7)
    s.set("bag.tmhm.0.item", "TM39", "bag.tmhm.0.quantity", 1)
    s.tick(4)
    s.set("player.team.0.moves.1.move", "Rock Tomb")
    s.tick(1)
    s.set("bag.tmhm.0.item", None, "bag.tmhm.0.quantity", 0)
    s.tick(4)
    s.set("bag.tmhm.1.item", "HM01", "bag.tmhm.1.quantity", 1)
    s.tick(4)
    s.set("player.team.0.moves.3.move", "Cut")
    s.tick(4)
    s.set("player.team.0.moves.0.move", None)
    s.tick(4)
    s.set("bag.items.0.item", "Shell Bell", "bag.items.0.quantity", 1)
    s.tick(4)
    s.set("player.team.0.held_item", "Shell Bell")
    s.set("bag.items.0.item", None, "bag.items.0.quantity", 0)
    s.tick(4)

    s.note("lose to Chili: solo and team HP 0, map change -> blackout")
    g.start_trainer(CHILI, [("Lillipup", 12, 38), ("Pansear", 14, 42)])
    g.ko_first(90)
    g.next_enemy(1, "Pansear", 14, 42)
    s.set("battle.player.active_pokemon.stats.hp", 0, "battle.player.team.0.stats.hp", 0)
    s.tick(2)
    g.end_battle()
    s.set("player.team.0.stats.hp", 0)
    s.wait(0.75)
    s.set("overworld.map_name", "Striaton City - Pokemon Center")
    s.tick(3)
    s.set("player.team.0.stats.hp", 30)
    s.tick(2)

    s.note("evolution at 17 after a wild fight")
    g.go_to("Dreamyard")
    g.start_wild("Munna", 10, 40)
    g.ko_first(900)
    s.set("player.team.0.level", 17)
    s.tick(4)
    s.set("player.team.0.species", "Pignite", "battle.player.team.0.stats.hp", 50)
    s.tick(2)
    g.end_battle()

    s.note("one more wild fight as Pignite")
    g.start_wild("Patrat", 9, 30)
    g.ko_first(60)
    g.end_battle()
    # (no soft reset here: gen 5 records no saves, so a reset would roll the whole route back)

    s.note("done")
    s.tick(3)
    return s
