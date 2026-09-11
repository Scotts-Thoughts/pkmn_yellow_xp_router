import os

from utils.constants import const

class GenFourConstants:
    def __init__(self):
        self.GEN_FOUR_DATA_PATH = os.path.join(const.POKEMON_RAW_DATA, "gen_four")
        self.ITEM_DB_PATH = os.path.join(self.GEN_FOUR_DATA_PATH, const.ITEM_DB_FILE_NAME)
        self.MOVE_DB_PATH = os.path.join(self.GEN_FOUR_DATA_PATH, const.MOVE_DB_FILE_NAME)
        self.TYPE_INFO_PATH = os.path.join(self.GEN_FOUR_DATA_PATH, const.TYPE_INFO_FILE_NAME)
        self.FIGHTS_INFO_PATH = os.path.join(self.GEN_FOUR_DATA_PATH, const.FIGHTS_INFO_FILE_NAME)

        self.PLATINUM_POKEMON_PATH = os.path.join(self.GEN_FOUR_DATA_PATH, "platinum", const.POKEMON_DB_FILE_NAME)
        self.PLATINUM_TRAINER_DB_PATH = os.path.join(self.GEN_FOUR_DATA_PATH, "platinum", const.TRAINERS_DB_FILE_NAME)
        self.DP_POKEMON_PATH = os.path.join(self.GEN_FOUR_DATA_PATH, "diamond_pearl", const.POKEMON_DB_FILE_NAME)
        self.DP_TRAINER_DB_PATH = os.path.join(self.GEN_FOUR_DATA_PATH, "diamond_pearl", const.TRAINERS_DB_FILE_NAME)
        self.HGSS_POKEMON_PATH = os.path.join(self.GEN_FOUR_DATA_PATH, "heartgold_soulsilver", const.POKEMON_DB_FILE_NAME)
        self.HGSS_TRAINER_DB_PATH = os.path.join(self.GEN_FOUR_DATA_PATH, "heartgold_soulsilver", const.TRAINERS_DB_FILE_NAME)

        self.COAL_BADGE = "coal"
        self.FOREST_BADGE = "forest"
        self.COBBLE_BADGE = "cobble"
        self.FEN_BADGE = "fen"
        self.RELIC_BADGE = "relic"
        self.MINE_BADGE = "mine"
        self.ICICLE_BADGE = "icicle"
        self.BEACON_BADGE = "beacon"

        self.ZEPHYR_BADGE = "zephyr"
        self.HIVE_BADGE = "hive"
        self.PLAIN_BADGE = "plain"
        self.FOG_BADGE = "fog"
        self.STORM_BADGE = "storm"
        self.MINERAL_BADGE = "mineral"
        self.GLACIER_BADGE = "glacier"
        self.RISING_BADGE = "rising"

        self.BOULDER_BADGE = "boulder"
        self.CASCADE_BADGE = "cascade"
        self.THUNDER_BADGE = "thunder"
        self.RAINDBOW_BADGE = "rainbow"
        self.SOUL_BADGE = "soul"
        self.MARSH_BADGE = "marsh"
        self.VOLCANO_BADGE = "volcano"
        self.EARTH_BADGE = "earth"

        self.MAROWAK_NAME = "Marowak"
        self.CUBONE_NAME = "Cubone"
        self.THICK_CLUB_NAME = "Thick Club"

        self.PIKACHU_NAME = "Pikachu"
        self.LIGHT_BALL_NAME = "Light Ball"

        self.DITTO_NAME = "Ditto"
        self.METAL_POWDER_NAME = "Metal Powder"

        self.CLAMPERL_NAME = "Clamperl"
        self.DEEP_SEA_TOOTH_NAME = "DeepSeaTooth"
        self.DEEP_SEA_SCALE_NAME = "DeepSeaScale"
        self.CHOICE_BAND_NAME = "Choice Band"
        self.CHOICE_SPECS_NAME = "Choice Specs"
        self.CHOICE_SCARF_NAME = "Choice Scarf"
        self.WIDE_LENS_NAME = "Wide Lens"

        self.LATIOS_NAME = "Latios"
        self.LATIAS_NAME = "Latias"
        self.SOULD_DEW_NAME = "Soul Dew"

        self.DIALGA_NAME = "Dialga"
        self.ADAMANT_ORB_NAME = "Adamant Orb"
        self.PALKIA_NAME = "Palkia"
        self.LUSTROUS_ORB_NAME = "Lustrous Orb"
        self.GIRATINA_NAME = "Giratina"
        self.GRISEOUS_ORB_NAME = "Griseous Orb"

        self.PLATE_TYPE_LOOKUP = {
            "Draco Plate": const.TYPE_DRAGON,
            "Dread Plate": const.TYPE_DARK,
            "Earth Plate": const.TYPE_GROUND,
            "Fist Plate": const.TYPE_FIGHTING,
            "Flame Plate": const.TYPE_FIRE,
            "Icicle Plate": const.TYPE_ICE,
            "Insect Plate": const.TYPE_BUG,
            "Iron Plate": const.TYPE_STEEL,
            "Meadow Plate": const.TYPE_GRASS,
            "Mind Plate": const.TYPE_PSYCHIC,
            "Sky Plate": const.TYPE_FLYING,
            "Splash Plate": const.TYPE_WATER,
            "Spooky Plate": const.TYPE_GHOST,
            "Stone Plate": const.TYPE_ROCK,
            "Toxic Plate": const.TYPE_POISON,
            "Zap Plate": const.TYPE_ELECTRIC,
        }

        self.COMPOUND_EYES_ABILITY = "Compound Eyes"
        self.LEVITATE_ABILITY = "Levitate"
        self.DAMP_ABILITY = "Damp"
        self.VOLT_ABOSRB_ABILITY = "Volt Absorb"
        self.LIGHTNING_ROD_ABILITY = "Lightning Rod"
        self.WATER_ABSORB_ABILITY = "Water Absorb"
        self.FLASH_FIRE_ABILITY = "Flash Fire"
        self.WONDER_GUARD_ABILITY = "Wonder Guard"
        self.BATTLE_ARMOR_ABILITY = "Battle Armor"
        self.SHELL_ARMOR_ABILITY = "Shell Armor"
        self.HUSTLE_ABILITY = "Hustle"
        self.SAND_VEIL_ABILITY = "Sand Veil"
        self.HUGE_POWER_ABILITY = "Huge Power"
        self.PURE_POWER_ABILITY = "Pure Power"
        self.THICK_FAT_ABILITY = "Thick Fat"
        self.MARVEL_SCALE_ABILITY = "Marvel Scale"
        self.GUTS_ABILITY = "Guts"
        self.OVERGROW_ABILITY = "Overgrow"
        self.BLAZE_ABILITY = "Blaze"
        self.TORRENT_ABILITY = "Torrent"
        self.SWARM_ABILITY = "Swarm"
        self.NO_GUARD_ABILITY = "No Guard"
        self.SCRAPPY_ABILITY = "Scrappy"
        self.SNIPER_ABILITY = "Sniper"
        self.SNOW_CLOAK_ABILITY = "Snow Cloak"
        self.SUPER_LUCK_ABILITY = "Super Luck"
        self.ADAPTABILITY_ABILITY = "Adaptability"
        self.DRY_SKIN_ABILITY = "Dry Skin"
        self.FILTER_ABILITY = "Filter"
        self.SOLID_ROCK_ABILITY = "Solid Rock"
        self.FLOWER_GIFT_ABILITY = "Flower Gift"
        self.HEATPROOF_ABILITY = "Heatproof"
        self.KLUTZ_ABILITY = "Klutz"
        self.MOTOR_DRIVE_ABILITY = "Motor Drive"
        self.NORMALIZE_ABILITY = "Normalize"
        self.SOLAR_POWER_ABILITY = "Solar Power"
        self.TECHNICIAN_ABILITY = "Technician"
        self.TINTED_LENS_ABILITY = "Tinted Lens"
        self.MULTITYPE_ABILITY = "Multitype"
        self.INSOMNIA_ABILITY = "Insomnia"
        self.SLOW_START_ABILITY = "Slow Start"
        self.SOUNDPROOF_ABILITY = "Soundproof"
        self.IRON_FIST_ABILITY = "Iron Fist"
        self.RECKLESS_ABILITY = "Reckless"
        self.STURDY_ABILITY = "Sturdy"

        self.SCOPE_LENS_NAME = "Scope Lens"
        self.RAZOR_CLAW_NAME = "Razor Claw"
        self.LUCKY_PUNCH_NAME = "Lucky Punch"
        self.STICK_NAME = "Stick"
        self.CHANSEY_NAME = "Chansey"
        self.FARFETCHD_NAME = "Farfetch'd"
        self.LIFE_ORB_NAME = "Life Orb"
        self.EXPERT_BELT_NAME = "Expert Belt"
        self.MUSCLE_BAND_NAME = "Muscle Band"
        self.WISE_GLASSES_NAME = "Wise Glasses"
        self.BRIGHT_POWDER_NAME = "BrightPowder"
        self.LAX_INCENSE_NAME = "Lax Incense"
        self.IRON_BALL_NAME = "Iron Ball"

        # Sound-based moves: Soundproof mons are immune (subscript_blocked_by_soundproof)
        self.SOUND_MOVES = {"Uproar", "Snore", "Hyper Voice", "Bug Buzz", "Chatter"}
        # Punching moves boosted 1.2x by Iron Fist
        self.PUNCH_MOVES = {
            "Ice Punch", "Fire Punch", "ThunderPunch", "Mach Punch", "Focus Punch",
            "Dizzy Punch", "DynamicPunch", "Hammer Arm", "Mega Punch", "Comet Punch",
            "Meteor Mash", "Shadow Punch", "Drain Punch", "Bullet Punch", "Sky Uppercut",
        }
        # Recoil moves boosted 1.2x power by Reckless
        self.RECKLESS_MOVES = {
            "Jump Kick", "Hi Jump Kick", "Take Down", "Submission", "Double-Edge",
            "Volt Tackle", "Brave Bird", "Wood Hammer", "Flare Blitz", "Head Smash",
        }

        self.SUPER_FANG_MOVE_NAME = "Super Fang"
        self.ENDEAVOR_MOVE_NAME = "Endeavor"
        self.PRESENT_MOVE_NAME = "Present"
        self.FLING_MOVE_NAME = "Fling"
        self.STRUGGLE_MOVE_NAME = "Struggle"
        self.FRUSTRATION_MOVE_NAME = "Frustration"

        # OHKO moves: damage = target's current HP; fails outright if the user isn't
        # at least as fast as the target, or the target has Sturdy.
        self.OHKO_MOVE_NAMES = {"Guillotine", "Horn Drill", "Fissure", "Sheer Cold"}

        self.TARGETING_ALL_FOES = "All Foes"
        self.TARGETING_OTHERS = "Others"

        # BattleSystem_FlingItem (battle_lib.c 5854+) fling-power table, by held item name.
        self.FLING_POWER_TABLE = {
            "Iron Ball": 130,
            "Hard Stone": 100, "Rare Bone": 100,
            "Helix Fossil": 100, "Dome Fossil": 100, "Old Amber": 100, "Root Fossil": 100,
            "Claw Fossil": 100, "Armor Fossil": 100, "Skull Fossil": 100, "Cover Fossil": 100,
            "Plume Fossil": 100,
            "Draco Plate": 90, "Dread Plate": 90, "Earth Plate": 90, "Fist Plate": 90,
            "Flame Plate": 90, "Icicle Plate": 90, "Insect Plate": 90, "Iron Plate": 90,
            "Meadow Plate": 90, "Mind Plate": 90, "Sky Plate": 90, "Splash Plate": 90,
            "Spooky Plate": 90, "Stone Plate": 90, "Toxic Plate": 90, "Zap Plate": 90,
            "DeepSeaTooth": 90, "Thick Club": 90, "Grip Claw": 90,
            "Razor Claw": 80, "Quick Claw": 80, "Sticky Barb": 80, "Dawn Stone": 80,
            "Dusk Stone": 80, "Shiny Stone": 80, "Electirizer": 80, "Magmarizer": 80,
            "Protector": 80, "Oval Stone": 80, "Odd Keystone": 80,
            "Dragon Fang": 70, "Poison Barb": 70, "Power Anklet": 70, "Power Band": 70,
            "Power Belt": 70, "Power Bracer": 70, "Power Lens": 70, "Power Weight": 70,
            "Adamant Orb": 60, "Lustrous Orb": 60, "Griseous Orb": 60, "Damp Rock": 60,
            "Heat Rock": 60, "Macho Brace": 60, "Stick": 60,
            "Sharp Beak": 50, "Dubious Disc": 50,
            "Lucky Punch": 40, "Icy Rock": 40,
            "Life Orb": 30, "Light Ball": 30, "Scope Lens": 30, "Metronome": 30,
            "Soul Dew": 30, "DeepSeaScale": 30, "King's Rock": 30, "Razor Fang": 30,
            "Shell Bell": 30, "Amulet Coin": 30, "Lucky Egg": 30, "Everstone": 30,
            "Exp. Share": 30, "Black Sludge": 30, "Flame Orb": 30, "Toxic Orb": 30,
            "Light Clay": 30, "Cleanse Tag": 30, "Smoke Ball": 30, "Up-Grade": 30,
            "Dragon Scale": 30, "Black Belt": 30, "BlackGlasses": 30, "Charcoal": 30,
            "Magnet": 30, "Metal Coat": 30, "Miracle Seed": 30, "Mystic Water": 30,
            "NeverMeltIce": 30, "Spell Tag": 30, "TwistedSpoon": 30,
            "Silk Scarf": 10, "SilverPowder": 10, "Soft Sand": 10,
            "Choice Band": 10, "Choice Specs": 10, "Choice Scarf": 10,
            "Expert Belt": 10, "Focus Band": 10, "Focus Sash": 10, "Muscle Band": 10,
            "Wise Glasses": 10, "Wide Lens": 10, "Zoom Lens": 10, "BrightPowder": 10,
            "Lax Incense": 10, "Full Incense": 10, "Odd Incense": 10, "Rock Incense": 10,
            "Rose Incense": 10, "Sea Incense": 10, "Wave Incense": 10, "Luck Incense": 10,
            "Pure Incense": 10, "Leftovers": 10, "Metal Powder": 10, "Quick Powder": 10,
            "Big Root": 10, "Destiny Knot": 10, "Mental Herb": 10, "Power Herb": 10,
            "Shed Shell": 10, "Smooth Rock": 10, "Soothe Bell": 10, "White Herb": 10,
            "Lagging Tail": 10, "Reaper Cloth": 10,
        }

        self.NO_BONUS = "No Bonus"
        self.DIG_BONUS = "Dig Bonus"
        self.DIVE_BONUS = "Dive Bonus"
        self.FLY_BONUS = "Fly/Bounce Bonus"
        self.SWITCH_BONUS = "Switch Bonus"
        self.MINIMIZE_BONUS = "Minimize Bonus"
        self.STATUS_BONUS = "Status Bonus"
        self.PARALYSIS_BONUS = "Paralysis Bonus"
        self.DAMAGED_BONUS = "Damaged Bonus"
        self.LOW_HEALTH_BONUS = "Low Health Bonus"
        self.SECOND_BONUS = "Move Second Bonus"
        self.SLEEPING_BONUS = "Sleeping Bonus"

        # Gen 4's Nature Power terrain->move table (HGSS asm 109-111 / to_move.h). This
        # replaces the gen-3 terrain list the app previously (and incorrectly) used for
        # every gen. Terrain pairs that call the same move (e.g. Plain and Sand both
        # call Earthquake) are collapsed into one dropdown entry.
        self.NATURE_POWER_PLAIN_SAND = "Plain/Sand"
        self.NATURE_POWER_GRASS_PUDDLE = "Grass/Puddle"
        self.NATURE_POWER_MOUNTAIN_CAVE = "Mountain/Cave"
        self.NATURE_POWER_SNOW = "Snow"
        self.NATURE_POWER_WATER = "Water"
        self.NATURE_POWER_ICE = "Ice"
        self.NATURE_POWER_BUILDING = "Building"
        self.NATURE_POWER_GREAT_MARSH = "Great Marsh"
        self.NATURE_POWER_BRIDGE = "Bridge"

        # terrain -> (base_power, move_type, accuracy) of the move Nature Power calls
        self.NATURE_POWER_MOVE_TABLE = {
            self.NATURE_POWER_PLAIN_SAND: (100, const.TYPE_GROUND, 100),      # Earthquake
            self.NATURE_POWER_GRASS_PUDDLE: (80, const.TYPE_GRASS, 100),      # Seed Bomb
            self.NATURE_POWER_MOUNTAIN_CAVE: (75, const.TYPE_ROCK, 90),       # Rock Slide
            self.NATURE_POWER_SNOW: (120, const.TYPE_ICE, 70),                # Blizzard
            self.NATURE_POWER_WATER: (120, const.TYPE_WATER, 80),             # Hydro Pump
            self.NATURE_POWER_ICE: (95, const.TYPE_ICE, 100),                 # Ice Beam
            self.NATURE_POWER_BUILDING: (80, const.TYPE_NORMAL, 100),         # Tri Attack
            self.NATURE_POWER_GREAT_MARSH: (65, const.TYPE_GROUND, 85),       # Mud Bomb
            self.NATURE_POWER_BRIDGE: (75, const.TYPE_FLYING, 95),            # Air Slash
        }
        # terrains whose called move is Physical (used for Hustle's accuracy penalty)
        self.NATURE_POWER_PHYSICAL_TERRAINS = {
            self.NATURE_POWER_PLAIN_SAND,
            self.NATURE_POWER_GRASS_PUDDLE,
            self.NATURE_POWER_MOUNTAIN_CAVE,
        }

        self.MAGNITUDE_MOVE_NAME = "Magnitude"
        self.MAGNITUDE_4 = "Mag 4"
        self.MAGNITUDE_5 = "Mag 5"
        self.MAGNITUDE_6 = "Mag 6"
        self.MAGNITUDE_7 = "Mag 7"
        self.MAGNITUDE_8 = "Mag 8"
        self.MAGNITUDE_9 = "Mag 9"
        self.MAGNITUDE_10 = "Mag 10"

        self.FLAIL_FULL_HP = "100-69 % HP"
        self.FLAIL_HALF_HP = "69-35 % HP"
        self.FLAIL_QUARTER_HP = "35-20 % HP"
        self.FLAIL_TEN_PERCENT_HP = "20-10 % HP"
        self.FLAIL_FIVE_PERCENT_HP = "10-4 % HP"
        self.FLAIL_MIN_HP = "4-0 % HP"

        self.FURY_CUTTER_MOVE_NAME = "Fury Cutter"
        self.ROLLOUT_MOVE_NAME = "Rollout"
        self.ICE_BALL_MOVE_NAME = "Ice Ball"
        self.TRIPLE_KICK_MOVE_NAME = "Triple Kick"
        self.RAGE_MOVE_NAME = "Rage"
        self.SPIT_UP_MOVE_NAME = "Spit Up"
        self.BLIZZARD_MOVE_NAME = "Blizzard"
        self.THUNDER_MOVE_NAME = "Thunder"
        self.PURSUIT_MOVE_NAME = "Pursuit"
        self.STOMP_MOVE_NAME = "Stomp"
        self.GUST_MOVE_NAME = "Gust"
        self.TWISTER_MOVE_NAME = "Twister"
        self.SURF_MOVE_NAME = "Surf"
        self.WHIRLPOOL_MOVE_NAME = "Whirlpool"
        self.EARTHQUAKE_MOVE_NAME = "Earthquake"
        self.RETURN_MOVE_NAME = "Return"
        self.FACADE_MOVE_NAME = "Facade"
        self.NEEDLE_ARM_MOVE_NAME = "Needle Arm"
        self.ASTONISH_MOVE_NAME = "Astonish"
        self.EXTRASENSORY_MOVE_NAME = "Extrasensory"
        self.SMELLING_SALT_MOVE_NAME = "SmellingSalt"
        self.REVENGE_MOVE_NAME = "Revenge"
        self.NATURE_POWER_MOVE_NAME = "Nature Power"
        self.BRICK_BREAK_MOVE_NAME = "Brick Break"
        self.ERUPTION_MOVE_NAME = "Eruption"
        self.WATER_SPOUT_MOVE_NAME = "Water Spout"
        self.ASSURANCE_MOVE_NAME = "Assurance"
        self.AVALANCHE_MOVE_NAME = "Avalanche"
        self.BRINE_MOVE_NAME = "Brine"
        self.PAYBACK_MOVE_NAME = "Payback"
        self.PUNISHMENT_MOVE_NAME = "Punishment"
        self.JUDGMENT_MOVE_NAME = "Judgment"
        self.TRUMP_CARD_MOVE_NAME = "Trump Card"
        self.WAKE_UP_SLAP_MOVE_NAME = "Wake-Up Slap"
        self.CRUSH_GRIP_MOVE_NAME = "Crush Grip"
        self.WRING_OUT_MOVE_NAME = "Wring Out"
        self.GYRO_BALL_MOVE_NAME = "Gyro Ball"
        self.LOW_KICK_MOVE_NAME = "Low Kick"
        self.GRASS_KNOW_MOVE_NAME = "Grass Knot"
        self.NATURAL_GIFT_MOVE_NAME = "Natural Gift"

        # Maps a held berry name to (base_power, type) for Natural Gift in gen 4.
        # In gen 5 the powers were each increased by 20; this table is the gen-4 version.
        self.NATURAL_GIFT_BERRY_DATA = {
            "Cheri Berry":  (60, const.TYPE_FIRE),
            "Chesto Berry": (60, const.TYPE_WATER),
            "Pecha Berry":  (60, const.TYPE_ELECTRIC),
            "Rawst Berry":  (60, const.TYPE_GRASS),
            "Aspear Berry": (60, const.TYPE_ICE),
            "Leppa Berry":  (60, const.TYPE_FIGHTING),
            "Oran Berry":   (60, const.TYPE_POISON),
            "Persim Berry": (60, const.TYPE_GROUND),
            "Lum Berry":    (60, const.TYPE_FLYING),
            "Sitrus Berry": (60, const.TYPE_PSYCHIC),
            "Figy Berry":   (60, const.TYPE_BUG),
            "Wiki Berry":   (60, const.TYPE_ROCK),
            "Mago Berry":   (60, const.TYPE_GHOST),
            "Aguav Berry":  (60, const.TYPE_DRAGON),
            "Iapapa Berry": (60, const.TYPE_DARK),
            "Razz Berry":   (60, const.TYPE_STEEL),

            "Bluk Berry":   (70, const.TYPE_FIRE),
            "Nanab Berry":  (70, const.TYPE_WATER),
            "Wepear Berry": (70, const.TYPE_ELECTRIC),
            "Pinap Berry":  (70, const.TYPE_GRASS),
            "Pomeg Berry":  (70, const.TYPE_ICE),
            "Kelpsy Berry": (70, const.TYPE_FIGHTING),
            "Qualot Berry": (70, const.TYPE_POISON),
            "Hondew Berry": (70, const.TYPE_GROUND),
            "Grepa Berry":  (70, const.TYPE_FLYING),
            "Tamato Berry": (70, const.TYPE_PSYCHIC),
            "Cornn Berry":  (70, const.TYPE_BUG),
            "Magost Berry": (70, const.TYPE_ROCK),
            "Rabuta Berry": (70, const.TYPE_GHOST),
            "Nomel Berry":  (70, const.TYPE_DRAGON),
            "Spelon Berry": (70, const.TYPE_DARK),
            "Pamtre Berry": (70, const.TYPE_STEEL),

            "Watmel Berry": (80, const.TYPE_FIRE),
            "Durin Berry":  (80, const.TYPE_WATER),
            "Belue Berry":  (80, const.TYPE_ELECTRIC),
            "Occa Berry":   (60, const.TYPE_FIRE),
            "Passho Berry": (60, const.TYPE_WATER),
            "Wacan Berry":  (60, const.TYPE_ELECTRIC),
            "Rindo Berry":  (60, const.TYPE_GRASS),
            "Yache Berry":  (60, const.TYPE_ICE),
            "Chople Berry": (60, const.TYPE_FIGHTING),
            "Kebia Berry":  (60, const.TYPE_POISON),
            "Shuca Berry":  (60, const.TYPE_GROUND),
            "Coba Berry":   (60, const.TYPE_FLYING),
            "Payapa Berry": (60, const.TYPE_PSYCHIC),
            "Tanga Berry":  (60, const.TYPE_BUG),
            "Charti Berry": (60, const.TYPE_ROCK),
            "Kasib Berry":  (60, const.TYPE_GHOST),
            "Haban Berry":  (60, const.TYPE_DRAGON),
            "Colbur Berry": (60, const.TYPE_DARK),
            "Babiri Berry": (60, const.TYPE_STEEL),
            "Chilan Berry": (60, const.TYPE_NORMAL),
            "Liechi Berry": (80, const.TYPE_GRASS),
            "Ganlon Berry": (80, const.TYPE_ICE),
            "Salac Berry":  (80, const.TYPE_FIGHTING),
            "Petaya Berry": (80, const.TYPE_POISON),
            "Apicot Berry": (80, const.TYPE_GROUND),
            "Lansat Berry": (80, const.TYPE_FLYING),
            "Starf Berry":  (80, const.TYPE_PSYCHIC),
            "Enigma Berry": (80, const.TYPE_BUG),
            "Micle Berry":  (80, const.TYPE_ROCK),
            "Custap Berry": (80, const.TYPE_GHOST),
            "Jaboca Berry": (80, const.TYPE_DRAGON),
            "Rowap Berry":  (80, const.TYPE_DARK),
        }

        self.CUSTOM_MOVE_DATA = {
            self.MAGNITUDE_MOVE_NAME: [
                self.MAGNITUDE_7, self.MAGNITUDE_7 + " " + self.DIG_BONUS,
                self.MAGNITUDE_4, self.MAGNITUDE_4 + " " + self.DIG_BONUS,
                self.MAGNITUDE_5, self.MAGNITUDE_5 + " " + self.DIG_BONUS,
                self.MAGNITUDE_6, self.MAGNITUDE_6 + " " + self.DIG_BONUS,
                self.MAGNITUDE_8, self.MAGNITUDE_8 + " " + self.DIG_BONUS,
                self.MAGNITUDE_9, self.MAGNITUDE_9 + " " + self.DIG_BONUS,
                self.MAGNITUDE_10, self.MAGNITUDE_10 + " " + self.DIG_BONUS,
            ],
            const.FLAIL_MOVE_NAME: [
                self.FLAIL_FULL_HP,
                self.FLAIL_HALF_HP,
                self.FLAIL_QUARTER_HP,
                self.FLAIL_TEN_PERCENT_HP,
                self.FLAIL_FIVE_PERCENT_HP,
                self.FLAIL_MIN_HP,
            ],
            const.REVERSAL_MOVE_NAME: [
                self.FLAIL_FULL_HP,
                self.FLAIL_HALF_HP,
                self.FLAIL_QUARTER_HP,
                self.FLAIL_TEN_PERCENT_HP,
                self.FLAIL_FIVE_PERCENT_HP,
                self.FLAIL_MIN_HP,
            ],
            self.NATURE_POWER_MOVE_NAME: list(self.NATURE_POWER_MOVE_TABLE.keys()),
            self.FURY_CUTTER_MOVE_NAME: ["1", "2", "3", "4", "5"],
            self.ROLLOUT_MOVE_NAME: ["1", "2", "3", "4", "5", "5 + DefenseCurl"],
            self.ICE_BALL_MOVE_NAME: ["1", "2", "3", "4", "5", "5 + DefenseCurl"],
            self.TRIPLE_KICK_MOVE_NAME: ["1", "2", "3"],

            self.PURSUIT_MOVE_NAME: [self.NO_BONUS, self.SWITCH_BONUS],
            self.STOMP_MOVE_NAME: [self.NO_BONUS, self.MINIMIZE_BONUS],
            self.GUST_MOVE_NAME: [self.NO_BONUS, self.FLY_BONUS],
            self.TWISTER_MOVE_NAME: [self.NO_BONUS, self.FLY_BONUS],
            self.EARTHQUAKE_MOVE_NAME: [self.NO_BONUS, self.DIG_BONUS],
            self.SURF_MOVE_NAME: [self.NO_BONUS, self.DIVE_BONUS],
            self.WHIRLPOOL_MOVE_NAME: [self.NO_BONUS, self.DIVE_BONUS],
            self.FACADE_MOVE_NAME: [self.NO_BONUS, self.STATUS_BONUS],
            self.SMELLING_SALT_MOVE_NAME: [self.NO_BONUS, self.PARALYSIS_BONUS],
            self.REVENGE_MOVE_NAME: [self.NO_BONUS, self.DAMAGED_BONUS],
            self.RETURN_MOVE_NAME: [str(x) for x in range(102, 0, -1)],
            self.FRUSTRATION_MOVE_NAME: [str(x) for x in range(102, 0, -1)],
            self.ERUPTION_MOVE_NAME: [str(x) for x in range(100, 0, -1)],
            self.WATER_SPOUT_MOVE_NAME: [str(x) for x in range(100, 0, -1)],
            self.SPIT_UP_MOVE_NAME: [str(x) for x in range(1, 4)],
            self.PRESENT_MOVE_NAME: ["40", "80", "120", "Heal"],

            self.ASSURANCE_MOVE_NAME: [self.NO_BONUS, self.DAMAGED_BONUS],
            self.AVALANCHE_MOVE_NAME: [self.NO_BONUS, self.DAMAGED_BONUS],
            self.BRINE_MOVE_NAME: [self.NO_BONUS, self.LOW_HEALTH_BONUS],
            self.PAYBACK_MOVE_NAME: [self.NO_BONUS, self.SECOND_BONUS],
            self.TRUMP_CARD_MOVE_NAME: ["4+", "3", "2", "1", "0"],
            self.WAKE_UP_SLAP_MOVE_NAME: [self.NO_BONUS, self.SLEEPING_BONUS],
            self.CRUSH_GRIP_MOVE_NAME: [str(x) for x in range(100, 0, -1)],
            self.WRING_OUT_MOVE_NAME: [str(x) for x in range(100, 0, -1)],
        }


gen_four_const = GenFourConstants()
