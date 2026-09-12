"""Crystal (deprecated mapper), Totodile solo: the gen 2 recorder paths.

Paths/values follow ``route_recording/game_recorders/gen_two/crystal_gamehook_constants.py``
in its deprecated-mapper configuration. Gen 2 specifics modelled here: areas
are "<map group> Map <map number>", the trainer is identified by class +
in-class id ("Youngster:1"), a battle starts on any ``battle.mode`` change and
the trainer event is created on the first ``battle.textBuffer`` change, TMs
are per-TM counters, heals/saves are sound ids on ``audio.currentSound``, and
a held-item change in the overworld goes through the HELD_CHECK fix-up.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from mock_gamehook import Scenario  # noqa: E402

TICK = "gameTime.seconds"
ROUTE = {"species": "Totodile", "version": "Crystal", "dvs": None}

HEAL_SOUND = 18
SAVE_SOUND = 37

TM_KEYS = [
    "TM01-DynamicPunch", "TM02-Headbutt", "TM03-Curse", "TM04-Rollout", "TM05-Roar", "TM06-Toxic", "TM07-Zap Cannon", "TM08-Rock Smash",
    "TM09-Psych Up", "TM10-Hidden Power", "TM11-Sunny Day", "TM12-Sweet Scent", "TM13-Snore", "TM14-Blizzard", "TM15-Hyper Beam",
    "TM16-Icy Wind", "TM17-Protect", "TM18-Rain Dance", "TM19-Giga Drain", "TM20-Endure", "TM21-Frustration", "TM22-SolarBeam",
    "TM23-Iron Tail", "TM24-Dragonbreath", "TM25-Thunder", "TM26-Earthquake", "TM27-Return", "TM28-Dig", "TM29-Psychic", "TM30-Shadow Ball",
    "TM31-Mud-Slap", "TM32-Double Team", "TM33-Ice Punch", "TM34-Swagger", "TM35-Sleep Talk", "TM36-Sludge Bomb", "TM37-Sandstorm",
    "TM38-Fire Blast", "TM39-Swift", "TM40-Defense Curl", "TM41-ThunderPunch", "TM42-Dream Eater", "TM43-Detect", "TM44-Rest",
    "TM45-Attract", "TM46-Thief", "TM47-Steel Wing", "TM48-Fire Punch", "TM49-Fury Cutter", "TM50-Nightmare",
]
HM_KEYS = ["HM01-Cut", "HM02-Fly", "HM03-Surf", "HM04-Strength", "HM05-Flash", "HM06-Whirlpool", "HM07-Waterfall"]


def initial_properties() -> dict:
    p = {}
    p["overworld.mapGroup"] = 24
    p["overworld.mapNumber"] = 4
    p["overworld.x"] = 5
    p["overworld.y"] = 7
    p["player.playerId"] = 7777
    p["player.money"] = 3000
    p["player.team.0.expPoints"] = 0
    p["player.team.0.heldItem"] = None
    p["player.team.0.friendship"] = 70
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
    p["gameTime.frames"] = 0
    p["audio.currentSound"] = 0
    p["battle.mode"] = None
    p["battle.type"] = None
    p["battle.textBuffer"] = ""
    p["battle.result"] = None
    p["battle.battleStart"] = 0
    p["battle.trainer.class"] = None
    p["battle.trainer.name"] = None
    p["battle.trainer.id"] = 0
    p["battle.trainer.totalPokemon"] = 0
    p["battle.yourPokemon.partyPos"] = 0
    p["battle.yourPokemon.species"] = None
    p["battle.yourPokemon.hp"] = 0
    p["battle.enemyPokemon.species"] = None
    p["battle.enemyPokemon.level"] = 0
    p["battle.enemyPokemon.hp"] = 0
    p["battle.enemyPokemon.partyPos"] = 0
    p["player.itemCount"] = 1
    for i in range(20):
        p[f"player.items.{i}.item"] = None
        p[f"player.items.{i}.quantity"] = 0
    p["player.items.0.item"] = "POTION"
    p["player.items.0.quantity"] = 1
    p["player.pokeBallCount"] = 0
    for i in range(12):
        p[f"player.pokeBalls.{i}.item"] = None
        p[f"player.pokeBalls.{i}.quantity"] = 0
    p["player.totalKeyItems"] = 0
    for i in range(26):
        p[f"player.keyItems.{i}"] = None
    for k in TM_KEYS:
        p[f"player.tms.{k}"] = 0
    for k in HM_KEYS:
        p[f"player.hms.{k}"] = 0
    return p


class Game:
    def __init__(self, s: Scenario):
        self.s = s
        self.exp = 0
        self.money = 3000

    def go_to(self, group, number):
        self.s.wait(0.75)
        self.s.set("overworld.mapGroup", group, "overworld.mapNumber", number)
        self.s.tick(1)

    def gain_exp(self, amount):
        self.exp += amount
        self.s.set("player.team.0.expPoints", self.exp)

    def set_money(self, value):
        self.money = value
        self.s.set("player.money", value)

    def start_wild(self, species, level, hp, my_hp=30):
        s = self.s
        s.note(f"wild {species} L{level}")
        s.set("battle.battleStart", 1)
        s.set("battle.mode", "Wild")
        s.tick(1)
        # the enemy lead is written once the battle is set up (the party position changes to 0)
        s.set("battle.enemyPokemon.species", species, "battle.enemyPokemon.level", level, "battle.enemyPokemon.hp", hp, "battle.enemyPokemon.partyPos", 0)
        s.set("battle.yourPokemon.partyPos", 0, "battle.yourPokemon.species", "Totodile", "battle.yourPokemon.hp", my_hp)
        s.set("battle.textBuffer", f"Wild {species} appeared!")
        s.set("battle.battleStart", 0)
        s.tick(2)

    def start_trainer(self, trainer_class, trainer_id, team, my_hp=30):
        s = self.s
        s.note(f"trainer {trainer_class}:{trainer_id} {team}")
        s.set("battle.trainer.class", trainer_class, "battle.trainer.id", trainer_id, "battle.trainer.totalPokemon", len(team))
        s.set("battle.battleStart", 1)
        s.set("battle.mode", "Trainer")
        s.tick(1)
        s.set("battle.enemyPokemon.species", team[0][0], "battle.enemyPokemon.level", team[0][1], "battle.enemyPokemon.hp", team[0][2], "battle.enemyPokemon.partyPos", 0)
        s.set("battle.yourPokemon.partyPos", 0, "battle.yourPokemon.species", "Totodile", "battle.yourPokemon.hp", my_hp)
        s.set("battle.textBuffer", f"{trainer_class} wants to battle!")
        s.set("battle.battleStart", 0)
        s.tick(2)

    def ko(self, exp_gain):
        self.s.set("battle.enemyPokemon.hp", 0)
        self.s.tick(1)
        self.gain_exp(exp_gain)
        self.s.tick(1)

    def next_enemy(self, party_pos, species, level, hp):
        self.s.set("battle.enemyPokemon.partyPos", party_pos)
        self.s.set("battle.enemyPokemon.species", species, "battle.enemyPokemon.level", level, "battle.enemyPokemon.hp", hp)
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
        s.set("battle.mode", None)
        # back in the overworld the battle slots are cleared (position 7 = empty)
        s.set("battle.enemyPokemon.species", None, "battle.enemyPokemon.level", 0, "battle.enemyPokemon.partyPos", 7, "battle.textBuffer", "")
        s.tick(2)


def build() -> Scenario:
    s = Scenario("Pokemon Crystal - Deprecated Mapper", initial_properties(), TICK, tick_delay=0.05, step_delay=0.05)
    g = Game(s)

    s.note("boot")
    s.tick(4)

    s.note("receive the starter")
    s.set(
        "player.team.0.species", "Totodile", "player.team.0.level", 5, "player.team.0.expPoints", 135,
        "player.team.0.dvAttack", 15, "player.team.0.dvDefense", 15, "player.team.0.dvSpeed", 15, "player.team.0.dvSpecial", 15,
        "player.team.0.move1", "SCRATCH", "player.team.0.move2", "LEER",
    )
    g.exp = 135
    s.tick(4)

    g.go_to(26, 1)  # route 29
    g.start_wild("Pidgey", 2, 12)
    g.ko(9)
    g.end_battle()
    g.start_wild("Sentret", 3, 14)
    g.ko(12)
    g.level_up(6)
    g.end_battle()

    g.go_to(26, 2)  # cherrygrove
    s.note("buy 2 potions, sell one (money after the item), heal, save")
    s.set("player.items.0.quantity", 3)
    s.tick(1)
    g.set_money(g.money - 600)
    s.tick(4)
    s.set("player.items.0.quantity", 2)
    s.tick(1)
    g.set_money(g.money + 150)
    s.tick(4)
    s.set("audio.currentSound", HEAL_SOUND)
    s.tick(1)
    s.set("audio.currentSound", 0)
    s.tick(2)
    s.set("audio.currentSound", SAVE_SOUND)
    s.tick(1)
    s.set("audio.currentSound", 0)
    s.tick(2)

    # NOTE: the gen 2 recorder writes a 0-based mon_order (gens 3/4 add +1), so a
    # fight whose only enemy position change is to slot 0 records mon_order=[0],
    # which the route engine cannot resolve (Python's engine raises and stops
    # recalculating the route from there). Multi-mon trainers only, see
    # docs/rust_port/KNOWN_ISSUES.md.
    s.note("Bug Catcher Don (two Caterpie), won, with the level 7 Rage learned")
    g.start_trainer("BUG CATCHER", 1, [("Caterpie", 3, 14), ("Caterpie", 3, 14)])
    g.ko(20)
    g.next_enemy(1, "Caterpie", 3, 14)
    g.ko(20)
    g.level_up(7, learn_move="RAGE", into_slot=3)
    g.end_battle(prize=48)

    g.go_to(26, 3)  # route 30
    g.start_trainer("YOUNGSTER", 2, [("Pidgey", 2, 12), ("Rattata", 4, 16)])
    g.ko(10)
    g.next_enemy(1, "Rattata", 4, 16)
    g.ko(18)
    g.end_battle(prize=64)

    s.note("rare candy (level 8), then a protein")
    s.set("player.items.1.item", "RARE CANDY", "player.items.1.quantity", 1, "player.items.2.item", "--End of list--")
    s.set("player.itemCount", 2)
    s.tick(4)
    s.set("player.team.0.level", 8)
    s.tick(1)
    s.set("player.items.1.item", "--End of list--", "player.items.1.quantity", 0, "player.items.2.item", None)
    s.set("player.itemCount", 1)
    s.tick(4)
    s.set("player.items.1.item", "PROTEIN", "player.items.1.quantity", 1, "player.items.2.item", "--End of list--")
    s.set("player.itemCount", 2)
    s.tick(4)
    s.set("player.team.0.statExpAttack", 2560)
    s.tick(1)
    s.set("player.items.1.item", "--End of list--", "player.items.1.quantity", 0, "player.items.2.item", None)
    s.set("player.itemCount", 1)
    s.tick(4)

    s.note("TM31 Mud-Slap over Leer (counter 1 -> 0), HM01 Cut into slot 4, tutor: forget Scratch")
    s.set("player.tms.TM31-Mud-Slap", 1)
    s.tick(4)
    s.set("player.team.0.move2", "MUD-SLAP")
    s.tick(1)
    s.set("player.tms.TM31-Mud-Slap", 0)
    s.tick(4)
    s.set("player.hms.HM01-Cut", 1)
    s.tick(4)
    s.set("player.team.0.move4", "CUT")
    s.tick(4)
    s.set("player.team.0.move1", None)
    s.tick(4)

    s.note("give a berry to hold (held-check fix-up removes the drop event)")
    s.set("player.items.1.item", "BERRY", "player.items.1.quantity", 1, "player.items.2.item", "--End of list--")
    s.set("player.itemCount", 2)
    s.tick(4)
    s.set("player.items.1.item", "--End of list--", "player.items.1.quantity", 0, "player.items.2.item", None)
    s.set("player.itemCount", 1)
    s.tick(4)
    s.set("player.team.0.heldItem", "BERRY")
    s.tick(4)

    s.note("Sprout Tower sage with three mons, then Falkner")
    g.go_to(3, 1)
    g.start_trainer("SAGE", 1, [("Bellsprout", 3, 14), ("Bellsprout", 3, 14), ("Bellsprout", 3, 14)])
    g.ko(15)
    g.next_enemy(1, "Bellsprout", 3, 14)
    g.ko(15)
    g.next_enemy(2, "Bellsprout", 3, 14)
    g.ko(15)
    g.level_up(9)
    g.end_battle(prize=96)

    g.go_to(3, 2)
    s.note("lose to Falkner: trainer event removed, defeated mons kept, black out")
    g.start_trainer("FALKNER", 1, [("Pidgey", 7, 24), ("Pidgeotto", 9, 30)])
    g.ko(30)
    g.next_enemy(1, "Pidgeotto", 9, 30)
    s.set("battle.yourPokemon.hp", 0)
    s.tick(1)
    g.end_battle()
    g.go_to(26, 2)
    s.set("audio.currentSound", HEAL_SOUND)
    s.tick(1)
    s.set("audio.currentSound", 0)
    s.tick(2)

    s.note("a wild fight, a save, another wild fight, then a soft reset")
    g.go_to(26, 3)
    g.start_wild("Hoppip", 4, 16)
    g.ko(20)
    g.end_battle()
    s.set("audio.currentSound", SAVE_SOUND)
    s.tick(1)
    s.set("audio.currentSound", 0)
    s.tick(2)
    g.start_wild("Ledyba", 5, 18)
    g.ko(25)
    g.end_battle()
    s.set("player.playerId", 0)
    s.tick(2)
    s.set("player.playerId", 7777)
    s.tick(4)

    s.note("after the reset")
    g.start_wild("Spinarak", 5, 18)
    g.ko(25)
    g.end_battle()

    s.note("done")
    s.tick(3)
    return s
