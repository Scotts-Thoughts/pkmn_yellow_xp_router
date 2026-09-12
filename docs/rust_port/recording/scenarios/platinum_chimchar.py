"""Platinum, Chimchar solo: the gen 4 recorder paths.

Paths/values follow ``route_recording/game_recorders/gen_four/platinum_gamehook_constants.py``.
Differences from gen 3 that the script models: battle entry/exit is driven by
``meta.state`` ("Battle" / "From Battle"), blackouts are confirmed on the map
change after the battle with the whole team at 0 HP, saves come from
``meta.saves`` incrementing, heals from the heal sound effect, rare candies /
vitamins go through INVENTORY_CHANGE first, sales are validated against the
sell price, 10+ balls bought earn a Premier Ball, and evolutions defer the
level-up move check to the next overworld entry.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from mock_gamehook import Scenario  # noqa: E402

TICK = "game_time.seconds"
ROUTE = {"species": "Chimchar", "version": "Platinum", "dvs": None}

HEAL_SOUND = 36335692
SOLO_PID = 70675
TRISTAN = 1        # Youngster Tristan, Route 202: Starly L5, $20
LIV_LIZ = 15       # Twins Liv & Liz (merged double battle): Pachirisu L11 x2, $88
DANIEL = 18        # Hiker Daniel, Route 205: Geodude L10, L11, L12, $96
ROARK = 246        # Leader Roark: Geodude L12, Onix L12, Cranidos L14, $420


def initial_properties() -> dict:
    p = {}
    p["meta.state"] = "Overworld"
    p["meta.saves"] = 0
    p["overworld.map_name"] = "Twinleaf Town - Map 1"
    p["player.player_id"] = 64036
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
    p["battle.mode"] = None
    p["battle.outcome"] = None
    p["battle.other.outcome_flags"] = 0
    p["battle.player.party_position"] = 0
    p["battle.player.active_pokemon.stats.hp"] = 0
    p["battle.player.party_position_2"] = 0
    p["battle.player.active_pokemon_2.stats.hp"] = 0
    p["battle.player.active_pokemon_2.internals.personality_value"] = 0
    p["battle.opponent.id"] = 0
    p["battle.opponent_2.id"] = 0
    p["battle.ally.id"] = 0
    for side in ("opponent", "opponent_2"):
        p[f"battle.{side}.active_pokemon.species"] = None
        p[f"battle.{side}.active_pokemon.level"] = 0
        p[f"battle.{side}.active_pokemon.stats.hp"] = 0
        p[f"battle.{side}.party_position"] = 0
        p[f"battle.{side}.active_pokemon.internals.personality_value"] = 0
        for i in range(6):
            p[f"battle.{side}.team.{i}.species"] = None
            p[f"battle.{side}.team.{i}.internals.personality_value"] = 0
    p["audio.save_sound"] = 0
    p["audio.heal_sound"] = 0
    for pocket, n in (("items", 40), ("medicine", 20), ("balls", 16), ("berries", 63), ("tmhm", 99)):
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
        self.next_pid = 1000

    def pid(self):
        self.next_pid += 7
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

    # -- battles ------------------------------------------------------------
    def _enter_battle(self, my_hp):
        s = self.s
        s.set("battle.player.party_position", 0, "battle.player.active_pokemon.stats.hp", my_hp, "battle.player.team.0.stats.hp", my_hp)
        s.set("battle.outcome", None)
        s.set("meta.state", "Battle")
        s.tick(4)  # BATTLE on the tick, then the delayed initialisation (3 s)

    def start_wild(self, species, level, hp, my_hp=30):
        s = self.s
        s.note(f"wild {species} L{level}")
        pid = self.pid()
        s.set("battle.mode", "Wild", "battle.opponent.id", 0, "battle.opponent_2.id", 0, "battle.ally.id", 0)
        s.set("battle.opponent.team.0.species", species, "battle.opponent.team.0.internals.personality_value", pid)
        s.set("battle.opponent.active_pokemon.species", species, "battle.opponent.active_pokemon.level", level, "battle.opponent.active_pokemon.stats.hp", hp, "battle.opponent.party_position", 0, "battle.opponent.active_pokemon.internals.personality_value", pid)
        self._enter_battle(my_hp)

    def start_trainer(self, trainer_id, team, second_id=0, team2=None, ally_pos=None, ally_pid=None, my_hp=30):
        """team: [(species, level, hp)]; a second trainer/team makes it a double battle."""
        s = self.s
        s.note(f"trainer {trainer_id} {team}")
        s.set("battle.mode", "Trainer", "battle.opponent.id", trainer_id, "battle.opponent_2.id", second_id, "battle.ally.id", 0)
        pids = []
        for i in range(6):
            sp = team[i][0] if i < len(team) else None
            pid = self.pid() if sp else 0
            pids.append(pid)
            s.set(f"battle.opponent.team.{i}.species", sp, f"battle.opponent.team.{i}.internals.personality_value", pid)
        pids2 = []
        for i in range(6):
            sp = team2[i][0] if team2 and i < len(team2) else None
            pid = self.pid() if sp else 0
            pids2.append(pid)
            s.set(f"battle.opponent_2.team.{i}.species", sp, f"battle.opponent_2.team.{i}.internals.personality_value", pid)
        s.set("battle.opponent.active_pokemon.species", team[0][0], "battle.opponent.active_pokemon.level", team[0][1], "battle.opponent.active_pokemon.stats.hp", team[0][2], "battle.opponent.party_position", 0, "battle.opponent.active_pokemon.internals.personality_value", pids[0])
        if team2:
            s.set("battle.opponent_2.active_pokemon.species", team2[0][0], "battle.opponent_2.active_pokemon.level", team2[0][1], "battle.opponent_2.active_pokemon.stats.hp", team2[0][2], "battle.opponent_2.party_position", 0, "battle.opponent_2.active_pokemon.internals.personality_value", pids2[0])
        if ally_pos is not None:
            s.set("battle.player.party_position_2", ally_pos, "battle.player.active_pokemon_2.stats.hp", 20, "battle.player.active_pokemon_2.internals.personality_value", ally_pid, f"battle.player.team.{ally_pos}.stats.hp", 20)
        self._enter_battle(my_hp)
        self._pids, self._pids2 = pids, pids2

    def ko_first(self, exp_gain):
        self.s.set("battle.opponent.active_pokemon.stats.hp", 0)
        self.s.tick(1)
        self.gain_exp(exp_gain)
        self.s.tick(1)

    def ko_second(self, exp_gain):
        self.s.set("battle.opponent_2.active_pokemon.stats.hp", 0)
        self.s.tick(1)
        self.gain_exp(exp_gain)
        self.s.tick(1)

    def next_enemy(self, party_pos, species, level, hp):
        self.s.set("battle.opponent.party_position", party_pos)
        self.s.set("battle.opponent.active_pokemon.species", species, "battle.opponent.active_pokemon.level", level, "battle.opponent.active_pokemon.stats.hp", hp, "battle.opponent.active_pokemon.internals.personality_value", self._pids[party_pos])
        self.s.tick(1)

    def level_up(self, new_level, learn_move=None, into_slot=None):
        s = self.s
        s.set("player.team.0.level", new_level)
        if learn_move is not None:
            s.tick(1)
            s.set(f"player.team.0.moves.{into_slot}.move", learn_move)
        s.tick(4)

    def end_battle(self, prize=0, outcome="Won"):
        s = self.s
        if prize:
            self.set_money(self.money + prize)
        s.set("battle.outcome", outcome)
        s.set("meta.state", "From Battle")
        s.tick(1)
        s.set("battle.mode", None)
        s.set("meta.state", "Overworld")
        s.tick(2)


def build() -> Scenario:
    s = Scenario("STP Pokemon Platinum", initial_properties(), TICK, tick_delay=0.05, step_delay=0.05)
    g = Game(s)

    s.note("boot")
    s.tick(4)
    s.tick(3)  # the empty slot 1 wait

    s.note("receive the starter")
    s.set(
        "player.team.0.species", "Chimchar", "player.team.0.level", 5, "player.team.0.exp", 135, "battle.player.team.0.exp", 135,
        "player.team.0.stats.hp", 22, "player.team.0.internals.personality_value", SOLO_PID,
        "player.team.0.ivs.hp", 31, "player.team.0.ivs.attack", 31, "player.team.0.ivs.defense", 31, "player.team.0.ivs.speed", 31,
        "player.team.0.ivs.special_attack", 31, "player.team.0.ivs.special_defense", 31,
        "player.team.0.moves.0.move", "Scratch", "player.team.0.moves.1.move", "Leer",
    )
    g.exp = 135
    s.tick(4)

    g.go_to("Route 201 - Map 3")
    g.start_wild("Starly", 2, 12)
    g.ko_first(9)
    g.end_battle()

    g.go_to("Route 202 - Map 4")
    g.start_trainer(TRISTAN, [("Starly", 5, 19)])
    g.ko_first(25)
    g.level_up(6)
    g.end_battle(prize=20)

    s.note("Jubilife mart: 10 poke balls (+ free premier ball), then a potion; sell a potion for its sell price")
    g.go_to("Jubilife City - Map 5")
    s.set("bag.balls.0.item", "Poke Ball", "bag.balls.0.quantity", 10)
    s.set("bag.balls.1.item", "Premier Ball", "bag.balls.1.quantity", 1)
    g.set_money(g.money - 2000)
    s.tick(4)
    s.set("bag.medicine.0.quantity", 3)
    g.set_money(g.money - 300)
    s.tick(4)
    s.set("bag.medicine.0.quantity", 2)
    g.set_money(g.money + 150)
    s.tick(4)
    s.note("toss a potion: money unchanged -> use/drop")
    s.set("bag.medicine.0.quantity", 1)
    s.tick(4)

    s.note("wild fight with the level-up to 7 learning Ember")
    g.go_to("Route 203 - Map 6")
    g.start_wild("Bidoof", 4, 18)
    g.ko_first(40)
    g.level_up(7, learn_move="Ember", into_slot=2)
    g.end_battle()

    s.note("heal (sound effect) and save (save counter) in Oreburgh")
    g.go_to("Oreburgh City - Map 7")
    s.set("audio.heal_sound", HEAL_SOUND)
    s.tick(1)
    s.set("audio.heal_sound", 0)
    s.tick(4)
    s.set("meta.saves", 1)
    s.tick(4)

    s.note("rare candy from the medicine pocket: level 8")
    s.set("bag.medicine.1.item", "Rare Candy", "bag.medicine.1.quantity", 2)
    s.tick(4)
    s.set("bag.medicine.1.quantity", 1)
    s.tick(1)
    s.set("player.team.0.level", 8)
    s.tick(4)

    s.note("protein (medicine pocket), EVs change afterwards")
    s.set("bag.medicine.2.item", "Protein", "bag.medicine.2.quantity", 1)
    s.tick(4)
    s.set("bag.medicine.2.item", None, "bag.medicine.2.quantity", 0)
    s.tick(1)
    s.set("player.team.0.evs.attack", 10)
    s.tick(7)

    s.note("TM39 Rock Tomb over Leer, HM06 Rock Smash into slot 4, then forget Scratch at the move deleter")
    s.set("bag.tmhm.0.item", "TM39", "bag.tmhm.0.quantity", 1)
    s.tick(4)
    s.set("player.team.0.moves.1.move", "Rock Tomb")
    s.tick(1)
    s.set("bag.tmhm.0.item", None, "bag.tmhm.0.quantity", 0)
    s.tick(4)
    s.set("bag.tmhm.1.item", "HM06", "bag.tmhm.1.quantity", 1)
    s.tick(4)
    s.set("player.team.0.moves.3.move", "Rock Smash")
    s.tick(4)
    s.set("player.team.0.moves.0.move", None)
    s.tick(4)

    s.note("hold a shell bell")
    s.set("bag.items.0.item", "Shell Bell", "bag.items.0.quantity", 1)
    s.tick(4)
    s.set("player.team.0.held_item", "Shell Bell")
    s.set("bag.items.0.item", None, "bag.items.0.quantity", 0)
    s.tick(4)

    s.note("three-mon trainer with switches")
    g.go_to("Route 205 - Map 8")
    g.start_trainer(DANIEL, [("Geodude", 10, 30), ("Geodude", 11, 32), ("Geodude", 12, 34)])
    g.ko_first(50)
    g.next_enemy(1, "Geodude", 11, 32)
    g.ko_first(55)
    g.next_enemy(2, "Geodude", 12, 34)
    g.ko_first(60)
    g.level_up(9)
    g.end_battle(prize=96)

    s.note("catch a Bidoof (second team member)")
    g.start_wild("Bidoof", 5, 20)
    s.set("bag.balls.0.quantity", 9)
    s.tick(2)
    s.set("player.team.1.species", "Bidoof", "player.team.1.level", 5, "player.team.1.stats.hp", 24, "player.team.1.ivs.attack", 3, "player.team.1.ivs.defense", 4, "player.team.1.ivs.speed", 5, "player.team.1.ivs.special_attack", 6, "player.team.1.ivs.special_defense", 7, "player.team.1.ivs.hp", 8)
    g.end_battle(outcome="Caught")

    s.note("double battle vs the merged twins entry, Bidoof as ally")
    g.start_trainer(LIV_LIZ, [("Pachirisu", 11, 33)], second_id=LIV_LIZ, team2=[("Pachirisu", 11, 33)], ally_pos=1, ally_pid=88888)
    g.ko_first(70)
    g.ko_second(70)
    g.level_up(11)
    g.end_battle(prize=88)

    s.note("evolution: level 14 in a wild fight, Chimchar -> Monferno")
    g.start_wild("Machop", 7, 30)
    g.ko_first(400)
    s.set("player.team.0.level", 14)
    s.tick(4)
    s.set("player.team.0.species", "Monferno", "battle.player.team.0.stats.hp", 40)
    s.tick(2)
    g.end_battle()

    s.note("lose to Roark: solo and team HP 0, map change -> blackout")
    g.go_to("Oreburgh City - Map 9")
    g.start_trainer(ROARK, [("Geodude", 12, 36), ("Onix", 12, 40), ("Cranidos", 14, 44)])
    g.ko_first(80)
    g.next_enemy(1, "Onix", 12, 40)
    s.set("battle.player.active_pokemon.stats.hp", 0, "battle.player.team.0.stats.hp", 0)
    s.tick(1)
    s.set("battle.player.team.1.stats.hp", 0)
    s.tick(1)
    g.end_battle(outcome="Lost")
    s.set("player.team.0.stats.hp", 0, "player.team.1.stats.hp", 0)
    s.wait(0.75)
    s.set("overworld.map_name", "Oreburgh City - Map 7")
    s.tick(2)
    s.set("audio.heal_sound", HEAL_SOUND)
    s.tick(1)
    s.set("audio.heal_sound", 0)
    s.set("player.team.0.stats.hp", 40, "player.team.1.stats.hp", 24)
    s.tick(4)

    s.note("save, one more fight, then a soft reset back to that save")
    s.set("meta.saves", 2)
    s.tick(4)
    g.go_to("Route 207 - Map 10")
    g.start_wild("Ponyta", 15, 40)
    g.ko_first(150)
    g.end_battle()
    s.set("player.player_id", 0)
    s.tick(2)
    s.set("player.player_id", 64036)
    s.set("overworld.map_name", "Oreburgh City - Map 7")
    s.tick(4)

    s.note("after the reset: a wild fight")
    g.go_to("Route 207 - Map 10")
    g.start_wild("Geodude", 14, 38)
    g.ko_first(140)
    g.end_battle()

    s.note("done")
    s.tick(3)
    return s
