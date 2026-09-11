import os

from utils.constants import const

class GenOneConstants:
    def __init__(self):
        self.GEN_ONE_DATA_PATH = os.path.join(const.POKEMON_RAW_DATA, "gen_one")
        self.ITEM_DB_PATH = os.path.join(self.GEN_ONE_DATA_PATH, const.ITEM_DB_FILE_NAME)
        self.MOVE_DB_PATH = os.path.join(self.GEN_ONE_DATA_PATH, const.MOVE_DB_FILE_NAME)
        self.TYPE_INFO_PATH = os.path.join(self.GEN_ONE_DATA_PATH, const.TYPE_INFO_FILE_NAME)
        self.FIGHTS_INFO_PATH = os.path.join(self.GEN_ONE_DATA_PATH, const.FIGHTS_INFO_FILE_NAME)

        self.YELLOW_ASSETS_PATH = os.path.join(self.GEN_ONE_DATA_PATH, "yellow")
        self.YELLOW_POKEMON_DB_PATH = os.path.join(self.YELLOW_ASSETS_PATH, const.POKEMON_DB_FILE_NAME)
        self.YELLOW_TRAINER_DB_PATH = os.path.join(self.YELLOW_ASSETS_PATH, const.TRAINERS_DB_FILE_NAME)
        self.YELLOW_MIN_BATTLES_DIR = os.path.join(self.YELLOW_ASSETS_PATH, "min_battles")

        self.RB_ASSETS_PATH = os.path.join(self.GEN_ONE_DATA_PATH, "red_blue")
        self.RB_POKEMON_DB_PATH = os.path.join(self.RB_ASSETS_PATH, const.POKEMON_DB_FILE_NAME)
        self.RB_TRAINER_DB_PATH = os.path.join(self.RB_ASSETS_PATH, const.TRAINERS_DB_FILE_NAME)
        self.RB_MIN_BATTLES_DIR = os.path.join(self.RB_ASSETS_PATH, "min_battles")

        # ok, actual consts
        self.BOULDER_BADGE = "boulder"
        self.CASCADE_BADGE = "cascade"
        self.THUNDER_BADGE = "thunder"
        self.RAINDBOW_BADGE = "rainbow"
        self.SOUL_BADGE = "soul"
        self.MARSH_BADGE = "marsh"
        self.VOLCANO_BADGE = "volcano"
        self.EARTH_BADGE = "earth"

        self.BAG_LIMIT = 20

        # Move-effect flavors used by raw_pkmn_data/gen_one/moves.json that have no shared
        # utils.constants entry (gen-1-only mechanics).
        self.FLAVOR_ONE_HIT_KO = "one_hit_ko"
        self.FLAVOR_COUNTER = "counter"
        self.FLAVOR_BIDE = "bide"
        self.FLAVOR_SUPER_FANG = "super_fang"
        self.FLAVOR_FOCUS_ENERGY = "focus_energy"
        self.FLAVOR_PARTIAL_TRAPPING = "partial_trapping"

        # Bug 2 (docs/damage_calc_review/gen_1_findings.md): the game applies type
        # effectiveness by walking the 82-row `TypeEffects` ROM table top to bottom, not by
        # checking the defender's type_1 then type_2. Order matters whenever one of the
        # defender's types is Super Effective and the other Not Very Effective, since
        # floor(d/2)*2 != d for odd d. This is that table's (attacking_type, defending_type)
        # order, verbatim (constants/type_constants.asm order via data/types/type_matchups.asm).
        self.GEN1_TYPE_EFFECT_ORDER = [
            ("Water", "Fire"), ("Fire", "Grass"), ("Fire", "Ice"), ("Grass", "Water"),
            ("Electric", "Water"), ("Water", "Rock"), ("Ground", "Flying"), ("Water", "Water"),
            ("Fire", "Fire"), ("Electric", "Electric"), ("Ice", "Ice"), ("Grass", "Grass"),
            ("Psychic", "Psychic"), ("Fire", "Water"), ("Grass", "Fire"), ("Water", "Grass"),
            ("Electric", "Grass"), ("Normal", "Rock"), ("Normal", "Ghost"), ("Ghost", "Ghost"),
            ("Fire", "Bug"), ("Fire", "Rock"), ("Water", "Ground"), ("Electric", "Ground"),
            ("Electric", "Flying"), ("Grass", "Ground"), ("Grass", "Bug"), ("Grass", "Poison"),
            ("Grass", "Rock"), ("Grass", "Flying"), ("Ice", "Water"), ("Ice", "Grass"),
            ("Ice", "Ground"), ("Ice", "Flying"), ("Fighting", "Normal"), ("Fighting", "Poison"),
            ("Fighting", "Flying"), ("Fighting", "Psychic"), ("Fighting", "Bug"),
            ("Fighting", "Rock"), ("Fighting", "Ice"), ("Fighting", "Ghost"), ("Poison", "Grass"),
            ("Poison", "Poison"), ("Poison", "Ground"), ("Poison", "Bug"), ("Poison", "Rock"),
            ("Poison", "Ghost"), ("Ground", "Fire"), ("Ground", "Electric"), ("Ground", "Grass"),
            ("Ground", "Bug"), ("Ground", "Rock"), ("Ground", "Poison"), ("Flying", "Electric"),
            ("Flying", "Fighting"), ("Flying", "Bug"), ("Flying", "Grass"), ("Flying", "Rock"),
            ("Psychic", "Fighting"), ("Psychic", "Poison"), ("Bug", "Fire"), ("Bug", "Grass"),
            ("Bug", "Fighting"), ("Bug", "Flying"), ("Bug", "Psychic"), ("Bug", "Ghost"),
            ("Bug", "Poison"), ("Rock", "Fire"), ("Rock", "Fighting"), ("Rock", "Ground"),
            ("Rock", "Flying"), ("Rock", "Bug"), ("Rock", "Ice"), ("Ghost", "Normal"),
            ("Ghost", "Psychic"), ("Fire", "Dragon"), ("Water", "Dragon"), ("Electric", "Dragon"),
            ("Grass", "Dragon"), ("Ice", "Dragon"), ("Dragon", "Dragon"),
        ]
        self.GEN1_TYPE_ROW_ORDER = {
            pair: idx for idx, pair in enumerate(self.GEN1_TYPE_EFFECT_ORDER)
        }

        # Super Fang (Bug 3 / 4.4): damage = a fraction of the target's current HP, min 1.
        # The router has no live HP tracking, so "Full HP" (first use) is the default; the
        # other options are for a Super Fang used again later in the same battle.
        self.SUPER_FANG_FULL_HP = "Full HP"
        self.SUPER_FANG_75_PERCENT_HP = "75% HP"
        self.SUPER_FANG_50_PERCENT_HP = "50% HP"
        self.SUPER_FANG_25_PERCENT_HP = "25% HP"
        self.SUPER_FANG_10_PERCENT_HP = "10% HP"
        self.SUPER_FANG_HP_PERCENTAGES = {
            self.SUPER_FANG_FULL_HP: 100,
            self.SUPER_FANG_75_PERCENT_HP: 75,
            self.SUPER_FANG_50_PERCENT_HP: 50,
            self.SUPER_FANG_25_PERCENT_HP: 25,
            self.SUPER_FANG_10_PERCENT_HP: 10,
        }

        # Partial trapping (4.6): Bind/Wrap/Fire Spin/Clamp roll damage/crit once and repeat it
        # for 2-5 turns (3/8, 3/8, 1/8, 1/8), just like the app's existing multi-hit moves.
        self.PARTIAL_TRAP_2_TURNS = "2 Turns"
        self.PARTIAL_TRAP_3_TURNS = "3 Turns"
        self.PARTIAL_TRAP_4_TURNS = "4 Turns"
        self.PARTIAL_TRAP_5_TURNS = "5 Turns"
        self.PARTIAL_TRAP_CUSTOM_DATA = [
            self.PARTIAL_TRAP_2_TURNS,
            self.PARTIAL_TRAP_3_TURNS,
            self.PARTIAL_TRAP_4_TURNS,
            self.PARTIAL_TRAP_5_TURNS,
        ]
        self.PARTIAL_TRAP_TURN_COUNTS = {
            self.PARTIAL_TRAP_2_TURNS: 2,
            self.PARTIAL_TRAP_3_TURNS: 3,
            self.PARTIAL_TRAP_4_TURNS: 4,
            self.PARTIAL_TRAP_5_TURNS: 5,
        }

        # Counter/Bide (4.2/4.3): both need "damage the target dealt" as an input the app has
        # no other way to supply. Represent it as a numeric custom_move_data value (a plain
        # integer string); the dropdown offers common round numbers but any digit string works.
        self.COUNTER_BIDE_CUSTOM_DATA = ["25", "50", "75", "100", "150", "200", "250", "300", "400"]

        self.CUSTOM_MOVE_DATA = {
            "Super Fang": [
                self.SUPER_FANG_FULL_HP,
                self.SUPER_FANG_75_PERCENT_HP,
                self.SUPER_FANG_50_PERCENT_HP,
                self.SUPER_FANG_25_PERCENT_HP,
                self.SUPER_FANG_10_PERCENT_HP,
            ],
            "Bind": self.PARTIAL_TRAP_CUSTOM_DATA,
            "Wrap": self.PARTIAL_TRAP_CUSTOM_DATA,
            "Fire Spin": self.PARTIAL_TRAP_CUSTOM_DATA,
            "Clamp": self.PARTIAL_TRAP_CUSTOM_DATA,
            "Counter": self.COUNTER_BIDE_CUSTOM_DATA,
            "Bide": self.COUNTER_BIDE_CUSTOM_DATA,
        }


gen_one_const = GenOneConstants()
