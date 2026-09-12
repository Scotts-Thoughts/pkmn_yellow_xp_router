"""Yellow (deprecated mapper), Charmander solo: every gen 1 recorder path.

Paths/values follow ``route_recording/game_recorders/gen_one/yellow_gamehook_constants.py``
in its deprecated-mapper configuration (the game name carries "Deprecated").
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from mock_gamehook import Scenario  # noqa: E402

TICK = "gameTime.seconds"
ROUTE = {"species": "Charmander", "version": "Yellow", "dvs": None}


def initial_properties() -> dict:
    p = {}
    p["overworld.map"] = "Pallet Town"
    p["audio.channel4"] = 0
    p["audio.channel5"] = 0
    p["audio.channel7"] = 0
    p["player.playerId"] = 4242
    p["player.money"] = 3000
    p["player.team.0.expPoints"] = 0
    for i in range(6):
        p[f"player.team.{i}.species"] = None
        p[f"player.team.{i}.level"] = 0
        for dv in ("dvAttack", "dvDefense", "dvSpeed", "dvSpecial"):
            p[f"player.team.{i}.{dv}"] = 0
    for m in range(1, 5):
        p[f"player.team.0.move{m}"] = None
    for st in ("statExpHp", "statExpAttack", "statExpDefense", "statExpSpeed", "statExpSpecial"):
        p[f"player.team.0.{st}"] = 0
    p["gameTime.seconds"] = 0
    p["battle.type"] = "None"
    p["battle.trainer.class"] = None
    p["battle.trainer.number"] = 0
    p["battle.yourPokemon.species"] = None
    p["battle.yourPokemon.battleStatHp"] = 0
    p["battle.enemyPokemon.species"] = None
    p["battle.enemyPokemon.level"] = 0
    p["player.itemCount"] = 1
    for i in range(20):
        p[f"player.items.{i}.item"] = None
        p[f"player.items.{i}.quantity"] = 0
    p["player.items.0.item"] = "POTION"
    p["player.items.0.quantity"] = 1
    p["player.items.1.item"] = "--End of list--"
    p["player.items.1.quantity"] = 0
    return p


class Game:
    def __init__(self, s: Scenario):
        self.s = s
        self.exp = 0
        self.money = 3000
        self.items = [("POTION", 1)]  # bag order

    def go_to(self, map_name):
        # walking to another map takes a while: let both apps drain their event
        # queues first (Python's processing thread polls every 100 ms and needs a
        # UI round trip per event, so it can lag a fast scripted map change)
        self.s.wait(0.75)
        self.s.set("overworld.map", map_name)
        self.s.tick(1)

    def gain_exp(self, amount):
        self.exp += amount
        self.s.set("player.team.0.expPoints", self.exp)

    def set_money(self, value):
        self.money = value
        self.s.set("player.money", value)

    # -- bag ------------------------------------------------------------------
    def _write_bag(self):
        s = self.s
        changes = {"player.itemCount": len(self.items)}
        for i in range(20):
            if i < len(self.items):
                changes[f"player.items.{i}.item"] = self.items[i][0]
                changes[f"player.items.{i}.quantity"] = self.items[i][1]
            elif i == len(self.items):
                changes[f"player.items.{i}.item"] = "--End of list--"
                changes[f"player.items.{i}.quantity"] = 0
            else:
                changes[f"player.items.{i}.item"] = None
                changes[f"player.items.{i}.quantity"] = 0
        # only send what changed, item slots first then the count (like the game writes them)
        cur = s.initial if not s.steps else None
        s.set(**{k: v for k, v in changes.items() if k != "player.itemCount"})
        s.set("player.itemCount", changes["player.itemCount"])

    def add_item(self, name, n):
        for idx, (it, q) in enumerate(self.items):
            if it == name:
                self.items[idx] = (it, q + n)
                self.s.set(f"player.items.{idx}.quantity", q + n)
                return
        self.items.append((name, n))
        self._write_bag()

    def remove_item(self, name, n):
        for idx, (it, q) in enumerate(self.items):
            if it == name:
                if q - n > 0:
                    self.items[idx] = (it, q - n)
                    self.s.set(f"player.items.{idx}.quantity", q - n)
                else:
                    del self.items[idx]
                    self._write_bag()
                return
        raise KeyError(name)

    # -- battles ----------------------------------------------------------------
    def start_wild(self, species, level, my_hp=30):
        s = self.s
        s.note(f"wild {species} L{level}")
        s.set("battle.enemyPokemon.species", species, "battle.enemyPokemon.level", level)
        s.set("battle.yourPokemon.species", "Charmander", "battle.yourPokemon.battleStatHp", my_hp)
        s.set("battle.type", "Wild")
        s.tick(2)

    def start_trainer(self, trainer_class, number, first, my_hp=30):
        s = self.s
        s.note(f"trainer {trainer_class} {number}")
        s.set("battle.trainer.class", trainer_class, "battle.trainer.number", number)
        s.set("battle.enemyPokemon.species", first[0], "battle.enemyPokemon.level", first[1])
        s.set("battle.yourPokemon.species", "Charmander", "battle.yourPokemon.battleStatHp", my_hp)
        s.set("battle.type", "Trainer")
        s.tick(2)  # the initial trainer event is created on the first second

    def ko(self, exp_gain):
        self.gain_exp(exp_gain)
        self.s.tick(1)

    def next_enemy(self, species, level):
        self.s.set("battle.enemyPokemon.species", species, "battle.enemyPokemon.level", level)
        self.s.tick(1)

    def level_up(self, new_level, learn_move=None, into_slot=None):
        s = self.s
        s.set("player.team.0.level", new_level)
        if learn_move is not None:
            s.tick(1)
            s.set(f"player.team.0.move{into_slot}", learn_move)
        s.tick(4)

    def end_battle(self, prize=0):
        s = self.s
        if prize:
            self.set_money(self.money + prize)
        s.set("battle.type", "None")
        s.set("battle.enemyPokemon.species", None, "battle.enemyPokemon.level", 0)
        s.tick(2)


def build() -> Scenario:
    s = Scenario("Pokemon Yellow - Deprecated Mapper", initial_properties(), TICK, tick_delay=0.05, step_delay=0.05)
    g = Game(s)

    s.note("boot")
    s.tick(4)

    s.note("receive the starter")
    s.set(
        "player.team.0.species", "Charmander", "player.team.0.level", 5, "player.team.0.expPoints", 135,
        "player.team.0.dvAttack", 15, "player.team.0.dvDefense", 15, "player.team.0.dvSpeed", 15, "player.team.0.dvSpecial", 15,
        "player.team.0.move1", "SCRATCH", "player.team.0.move2", "GROWL",
    )
    g.exp = 135
    s.tick(4)

    s.note("rival lab fight (Eevee) - lost, no blackout")
    g.start_trainer("RIVAL1", 1, ("Eevee", 5))
    s.set("battle.yourPokemon.battleStatHp", 0)
    s.tick(1)
    g.end_battle()

    s.note("rival lab fight again, won")
    g.start_trainer("RIVAL1", 1, ("Eevee", 5))
    g.ko(30)
    g.level_up(6)
    g.end_battle(prize=175)

    s.note("Oak's parcel is ignored; potions bought in Viridian")
    g.go_to("Route 1")
    g.start_wild("Pidgey", 3)
    g.ko(15)
    g.end_battle()
    g.start_wild("Rattata", 2)
    g.ko(12)
    g.end_battle()

    g.go_to("Viridian City")
    g.add_item("OAK'S PARCEL", 1)
    s.tick(4)
    g.go_to("Viridian City - Mart")
    g.add_item("POTION", 2)
    s.tick(1)
    g.set_money(g.money - 600)
    s.tick(4)
    s.note("sell an antidote-less bag? no: pick up a free antidote then sell it")
    g.add_item("ANTIDOTE", 1)
    s.tick(4)
    g.remove_item("ANTIDOTE", 1)
    s.tick(1)
    g.set_money(g.money + 50)
    s.tick(4)

    s.note("heal in the Viridian center (channel 5)")
    g.go_to("Viridian City - Pokemon Center")
    s.set("audio.channel5", 158)
    s.tick(1)
    s.set("audio.channel5", 0)
    s.tick(3)

    s.note("Viridian Forest: bug catchers")
    g.go_to("Viridian Forest")
    g.start_trainer("BUG CATCHER", 1, ("Caterpie", 7))
    g.ko(20)
    g.next_enemy("Caterpie", 7)
    g.ko(20)
    g.level_up(7)
    g.end_battle(prize=70)
    g.start_trainer("BUG CATCHER", 2, ("Metapod", 6))
    g.ko(18)
    g.next_enemy("Caterpie", 6)
    g.ko(16)
    g.next_enemy("Metapod", 6)
    g.ko(18)
    g.level_up(8)
    g.end_battle(prize=60)

    s.note("wild fight with a level up that learns Ember (9)")
    g.start_wild("Weedle", 6)
    g.ko(40)
    g.level_up(9, learn_move="EMBER", into_slot=3)
    g.end_battle()

    s.note("rare candy (quantity 2 -> 1), then a vitamin")
    g.add_item("RARE CANDY", 2)
    s.tick(4)
    s.set("player.team.0.level", 10)
    s.tick(1)
    g.remove_item("RARE CANDY", 1)
    s.tick(4)
    g.add_item("PROTEIN", 1)
    s.tick(4)
    s.set("player.team.0.statExpAttack", 2560)
    s.tick(1)
    g.remove_item("PROTEIN", 1)
    s.tick(4)

    s.note("TM34 Bide over Growl, then HM01 Cut into slot 4")
    g.add_item("TM34: BIDE", 1)
    s.tick(4)
    s.set("player.team.0.move2", "BIDE")
    s.tick(1)
    g.remove_item("TM34: BIDE", 1)
    s.tick(4)
    g.add_item("HM01: CUT", 1)
    s.tick(4)
    s.set("player.team.0.move4", "CUT")
    s.tick(4)

    s.note("Pewter: Brock")
    g.go_to("Pewter City")
    g.go_to("Pewter City - Gym")
    g.start_trainer("BROCK", 1, ("Geodude", 10))
    g.ko(60)
    g.next_enemy("Onix", 12)
    g.ko(80)
    g.level_up(12)
    g.end_battle(prize=1188)

    s.note("nugget bridge rocket: lose, get the nugget, black out")
    g.go_to("Route 24")
    g.start_trainer("ROCKET", 6, ("Ekans", 15))
    g.ko(70)
    g.next_enemy("Zubat", 15)
    s.set("battle.yourPokemon.battleStatHp", 0)
    s.tick(1)
    g.end_battle()
    g.go_to("Cerulean City - Pokemon Center")
    s.set("audio.channel5", 158)
    s.tick(1)
    s.set("audio.channel5", 0)
    s.tick(3)

    s.note("vending machine drink (forced purchase) in Celadon, then save")
    g.go_to("Celadon City - Dept Store Roof")
    g.add_item("FRESH WATER", 1)
    s.tick(4)
    g.set_money(g.money - 200)
    s.tick(3)
    g.go_to("Celadon City - Pokemon Center")
    s.set("audio.channel5", 182)
    s.tick(1)
    s.set("audio.channel5", 0)
    s.tick(2)

    s.note("a wild fight, then a soft reset back to the Celadon save")
    g.go_to("Route 7")
    g.start_wild("Growlithe", 18)
    g.ko(120)
    g.end_battle()
    s.set("player.playerId", 0)
    s.tick(2)
    s.set("player.playerId", 4242)
    s.set("overworld.map", "Celadon City - Pokemon Center")
    s.tick(4)

    s.note("after the reset: a wild fight on Route 8")
    g.go_to("Route 8")
    g.start_wild("Meowth", 18)
    g.ko(110)
    g.end_battle()

    s.note("done")
    s.tick(3)
    return s
