import os

from utils.constants import const

# NOTE: This module was copied wholesale from gen_4/gen_four_constants.py as the
# starting point for gen 5 (Black/White/Black 2/White 2). The battle mechanics
# constants (abilities, moves, items, Natural Gift table, etc.) are currently the
# gen-4 values and are "good enough" since the gen 5 damage formula is essentially
# identical to gen 4. Anything that is genuinely gen-5-specific (e.g. Natural Gift
# base powers were each raised by 20 in gen 5, new abilities/items, weather damage
# tweaks) should be corrected here over time. Only the data paths and the badge
# definitions have been updated for gen 5 so far.
class GenFiveConstants:
    def __init__(self):
        self.GEN_FIVE_DATA_PATH = os.path.join(const.POKEMON_RAW_DATA, "gen_five")
        self.ITEM_DB_PATH = os.path.join(self.GEN_FIVE_DATA_PATH, const.ITEM_DB_FILE_NAME)
        self.MOVE_DB_PATH = os.path.join(self.GEN_FIVE_DATA_PATH, const.MOVE_DB_FILE_NAME)
        self.TYPE_INFO_PATH = os.path.join(self.GEN_FIVE_DATA_PATH, const.TYPE_INFO_FILE_NAME)
        self.FIGHTS_INFO_PATH = os.path.join(self.GEN_FIVE_DATA_PATH, const.FIGHTS_INFO_FILE_NAME)

        # Black/White share one dataset; Black 2/White 2 share another.
        self.BW_POKEMON_PATH = os.path.join(self.GEN_FIVE_DATA_PATH, "black_white", const.POKEMON_DB_FILE_NAME)
        self.BW_TRAINER_DB_PATH = os.path.join(self.GEN_FIVE_DATA_PATH, "black_white", const.TRAINERS_DB_FILE_NAME)
        self.B2W2_POKEMON_PATH = os.path.join(self.GEN_FIVE_DATA_PATH, "black2_white2", const.POKEMON_DB_FILE_NAME)
        self.B2W2_TRAINER_DB_PATH = os.path.join(self.GEN_FIVE_DATA_PATH, "black2_white2", const.TRAINERS_DB_FILE_NAME)

        # Unova badge identifiers. These match the values in gen_five/fights_info.json
        # "badge_rewards", covering both the BW gyms and the B2W2 gyms. Badges no
        # longer boost stats (gen 3+); they are tracked for the blackout-money calc
        # and for display only.
        self.TRIO_BADGE = "trio"
        self.BASIC_BADGE = "basic"
        self.TOXIC_BADGE = "toxic"
        self.INSECT_BADGE = "insect"
        self.BOLT_BADGE = "bolt"
        self.QUAKE_BADGE = "quake"
        self.JET_BADGE = "jet"
        self.FREEZE_BADGE = "freeze"
        self.LEGEND_BADGE = "legend"
        self.WAVE_BADGE = "wave"

        self.MAROWAK_NAME = "Marowak"
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

        self.PLAIN_TERRAIN = "Plain"
        self.SAND_TERRAIN = "Sand"
        self.CAVE_TERRAIN = "Cave"
        self.ROCK_TERRAIN = "Rock"
        self.TALL_GRASS_TERRAIN = "Tall Grass"
        self.LONG_GRASS_TERRAIN = "Long Grass"
        self.POND_WATER_TERRAIN = "Pond Water"
        self.SEA_WATER_TERRAIN = "Sea Water"
        self.UNDERWATER_TERRAIN = "Underwater"

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

        # Gen 5 spellings that diverge from the gen 1-4 constants in utils/constants.py
        # (docs/damage_calc_review/gen_5_findings.md section 1, item 5). Checked in
        # addition to (not instead of) the shared `const.*` spelling so both are matched.
        self.SELFDESTRUCT_MOVE_NAME = "Self-Destruct"
        self.SOLAR_BEAM_MOVE_NAME = "Solar Beam"
        # NOTE: this used to be "SmellingSalt" (the gen 1-4 spelling, inherited when this
        # file was bootstrapped from gen_four_constants.py); gen 5's moves.json spells it
        # "Smelling Salts", so the old value silently disabled the dropdown below.
        self.SMELLING_SALT_MOVE_NAME = "Smelling Salts"

        self.STEAMROLLER_MOVE_NAME = "Steamroller"
        self.FRUSTRATION_MOVE_NAME = "Frustration"
        self.PRESENT_MOVE_NAME = "Present"
        self.SUPER_FANG_MOVE_NAME = "Super Fang"
        self.ENDEAVOR_MOVE_NAME = "Endeavor"
        self.HEAVY_SLAM_MOVE_NAME = "Heavy Slam"
        self.HEAT_CRASH_MOVE_NAME = "Heat Crash"
        self.ELECTRO_BALL_MOVE_NAME = "Electro Ball"
        self.STORED_POWER_MOVE_NAME = "Stored Power"
        self.HEX_MOVE_NAME = "Hex"
        self.VENOSHOCK_MOVE_NAME = "Venoshock"
        self.RETALIATE_MOVE_NAME = "Retaliate"
        self.ECHOED_VOICE_MOVE_NAME = "Echoed Voice"
        self.PSYSHOCK_MOVE_NAME = "Psyshock"
        self.PSYSTRIKE_MOVE_NAME = "Psystrike"
        self.SECRET_SWORD_MOVE_NAME = "Secret Sword"
        self.FOUL_PLAY_MOVE_NAME = "Foul Play"
        self.CHIP_AWAY_MOVE_NAME = "Chip Away"
        self.SACRED_SWORD_MOVE_NAME = "Sacred Sword"
        self.ACROBATICS_MOVE_NAME = "Acrobatics"
        self.FINAL_GAMBIT_MOVE_NAME = "Final Gambit"
        self.FROST_BREATH_MOVE_NAME = "Frost Breath"
        self.STORM_THROW_MOVE_NAME = "Storm Throw"
        self.HURRICANE_MOVE_NAME = "Hurricane"

        # Moves that use the target's DEFENSE stat even though they're categorized
        # Special (gen_5_findings.md move table rows for Psyshock/Psystrike/Secret Sword).
        self.USES_TARGET_DEFENSE_MOVES = {
            self.PSYSHOCK_MOVE_NAME, self.PSYSTRIKE_MOVE_NAME, self.SECRET_SWORD_MOVE_NAME,
        }
        # Moves that ignore the target's Defense/evasion stat stages entirely.
        self.IGNORES_DEFENSE_STAGES_MOVES = {
            self.CHIP_AWAY_MOVE_NAME, self.SACRED_SWORD_MOVE_NAME,
        }
        # Gen 5 always crits with these moves regardless of the normal crit roll
        # (their moves.json `effect` is mislabelled "high_crit_rate", which only bumps
        # the crit stage by 1; the real gen 5 behavior is 100% crit chance).
        self.ALWAYS_CRIT_MOVES = {
            self.FROST_BREATH_MOVE_NAME, self.STORM_THROW_MOVE_NAME,
        }
        self.ONE_HIT_KO_MOVE_NAMES = {"Guillotine", "Horn Drill", "Fissure", "Sheer Cold"}

        self.POISONED_BONUS = "Poisoned Bonus"
        self.ALLY_FAINTED_BONUS = "Ally Fainted Bonus"
        self.HEAL_OPTION = "Heal"

        # Maps a held berry name to (base_power, type) for Natural Gift in gen 5.
        # Gen 5 raised every gen-4 berry power by 20 (gen_5_findings.md section 1 item 4 /
        # section 5 item 3); this table already has that +20 baked in.
        self.NATURAL_GIFT_BERRY_DATA = {
            "Cheri Berry":  (80, const.TYPE_FIRE),
            "Chesto Berry": (80, const.TYPE_WATER),
            "Pecha Berry":  (80, const.TYPE_ELECTRIC),
            "Rawst Berry":  (80, const.TYPE_GRASS),
            "Aspear Berry": (80, const.TYPE_ICE),
            "Leppa Berry":  (80, const.TYPE_FIGHTING),
            "Oran Berry":   (80, const.TYPE_POISON),
            "Persim Berry": (80, const.TYPE_GROUND),
            "Lum Berry":    (80, const.TYPE_FLYING),
            "Sitrus Berry": (80, const.TYPE_PSYCHIC),
            "Figy Berry":   (80, const.TYPE_BUG),
            "Wiki Berry":   (80, const.TYPE_ROCK),
            "Mago Berry":   (80, const.TYPE_GHOST),
            "Aguav Berry":  (80, const.TYPE_DRAGON),
            "Iapapa Berry": (80, const.TYPE_DARK),
            "Razz Berry":   (80, const.TYPE_STEEL),

            "Bluk Berry":   (90, const.TYPE_FIRE),
            "Nanab Berry":  (90, const.TYPE_WATER),
            "Wepear Berry": (90, const.TYPE_ELECTRIC),
            "Pinap Berry":  (90, const.TYPE_GRASS),
            "Pomeg Berry":  (90, const.TYPE_ICE),
            "Kelpsy Berry": (90, const.TYPE_FIGHTING),
            "Qualot Berry": (90, const.TYPE_POISON),
            "Hondew Berry": (90, const.TYPE_GROUND),
            "Grepa Berry":  (90, const.TYPE_FLYING),
            "Tamato Berry": (90, const.TYPE_PSYCHIC),
            "Cornn Berry":  (90, const.TYPE_BUG),
            "Magost Berry": (90, const.TYPE_ROCK),
            "Rabuta Berry": (90, const.TYPE_GHOST),
            "Nomel Berry":  (90, const.TYPE_DRAGON),
            "Spelon Berry": (90, const.TYPE_DARK),
            "Pamtre Berry": (90, const.TYPE_STEEL),

            "Watmel Berry": (100, const.TYPE_FIRE),
            "Durin Berry":  (100, const.TYPE_WATER),
            "Belue Berry":  (100, const.TYPE_ELECTRIC),
            "Occa Berry":   (100, const.TYPE_FIRE),
            "Passho Berry": (100, const.TYPE_WATER),
            "Wacan Berry":  (100, const.TYPE_ELECTRIC),
            "Rindo Berry":  (100, const.TYPE_GRASS),
            "Yache Berry":  (100, const.TYPE_ICE),
            "Chople Berry": (100, const.TYPE_FIGHTING),
            "Kebia Berry":  (100, const.TYPE_POISON),
            "Shuca Berry":  (100, const.TYPE_GROUND),
            "Coba Berry":   (100, const.TYPE_FLYING),
            "Payapa Berry": (100, const.TYPE_PSYCHIC),
            "Tanga Berry":  (100, const.TYPE_BUG),
            "Charti Berry": (100, const.TYPE_ROCK),
            "Kasib Berry":  (100, const.TYPE_GHOST),
            "Haban Berry":  (100, const.TYPE_DRAGON),
            "Colbur Berry": (100, const.TYPE_DARK),
            "Babiri Berry": (100, const.TYPE_STEEL),
            "Chilan Berry": (100, const.TYPE_NORMAL),
            "Liechi Berry": (100, const.TYPE_GRASS),
            "Ganlon Berry": (100, const.TYPE_ICE),
            "Salac Berry":  (100, const.TYPE_FIGHTING),
            "Petaya Berry": (100, const.TYPE_POISON),
            "Apicot Berry": (100, const.TYPE_GROUND),
            "Lansat Berry": (100, const.TYPE_FLYING),
            "Starf Berry":  (100, const.TYPE_PSYCHIC),
            "Enigma Berry": (100, const.TYPE_BUG),
            "Micle Berry":  (100, const.TYPE_ROCK),
            "Custap Berry": (100, const.TYPE_GHOST),
            "Jaboca Berry": (100, const.TYPE_DRAGON),
            "Rowap Berry":  (100, const.TYPE_DARK),
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
            self.NATURE_POWER_MOVE_NAME: [
                self.PLAIN_TERRAIN,
                self.SAND_TERRAIN,
                self.CAVE_TERRAIN,
                self.ROCK_TERRAIN,
                self.TALL_GRASS_TERRAIN,
                self.LONG_GRASS_TERRAIN,
                self.POND_WATER_TERRAIN,
                self.SEA_WATER_TERRAIN,
                self.UNDERWATER_TERRAIN,
            ],
            # NOTE: Fury Cutter caps at 5 doublings in gen 5 (20 -> 40 -> 80 -> 160, capped
            # after the 4th hit); "6" is kept only as a label meaning "5 or more" (calc.py
            # clamps it). Rage's dropdown was removed: gen 5 Rage (like gen 2+) is a plain
            # 20-power hit whose effect is +1 Attack stage per hit taken, not a damage
            # multiplier -- that's already expressible via the Attack-stage dropdowns.
            self.FURY_CUTTER_MOVE_NAME: ["1", "2", "3", "4", "5", "6"],
            self.ROLLOUT_MOVE_NAME: ["1", "2", "3", "4", "5", "5 + DefenseCurl"],
            self.ICE_BALL_MOVE_NAME: ["1", "2", "3", "4", "5", "5 + DefenseCurl"],
            self.TRIPLE_KICK_MOVE_NAME: ["1", "2", "3"],

            self.PURSUIT_MOVE_NAME: [self.NO_BONUS, self.SWITCH_BONUS],
            # Gen 5 only doubles Stomp/Steamroller vs a Minimized target (Needle Arm,
            # Astonish and Extrasensory lost the bonus after gen 4).
            self.STOMP_MOVE_NAME: [self.NO_BONUS, self.MINIMIZE_BONUS],
            self.STEAMROLLER_MOVE_NAME: [self.NO_BONUS, self.MINIMIZE_BONUS],
            self.GUST_MOVE_NAME: [self.NO_BONUS, self.FLY_BONUS],
            self.TWISTER_MOVE_NAME: [self.NO_BONUS, self.FLY_BONUS],
            self.EARTHQUAKE_MOVE_NAME: [self.NO_BONUS, self.DIG_BONUS],
            self.SURF_MOVE_NAME: [self.NO_BONUS, self.DIVE_BONUS],
            self.WHIRLPOOL_MOVE_NAME: [self.NO_BONUS, self.DIVE_BONUS],
            self.FACADE_MOVE_NAME: [self.NO_BONUS, self.STATUS_BONUS],
            self.SMELLING_SALT_MOVE_NAME: [self.NO_BONUS, self.PARALYSIS_BONUS],
            self.REVENGE_MOVE_NAME: [self.NO_BONUS, self.DAMAGED_BONUS],
            self.RETURN_MOVE_NAME: [str(x) for x in range(102, 0, -1)],
            self.ERUPTION_MOVE_NAME: [str(x) for x in range(100, 0, -1)],
            self.WATER_SPOUT_MOVE_NAME: [str(x) for x in range(100, 0, -1)],
            self.SPIT_UP_MOVE_NAME: [str(x) for x in range(1, 4)],

            self.ASSURANCE_MOVE_NAME: [self.NO_BONUS, self.DAMAGED_BONUS],
            self.AVALANCHE_MOVE_NAME: [self.NO_BONUS, self.DAMAGED_BONUS],
            self.BRINE_MOVE_NAME: [self.NO_BONUS, self.LOW_HEALTH_BONUS],
            self.PAYBACK_MOVE_NAME: [self.NO_BONUS, self.SECOND_BONUS],
            self.TRUMP_CARD_MOVE_NAME: ["4+", "3", "2", "1", "0"],
            self.WAKE_UP_SLAP_MOVE_NAME: [self.NO_BONUS, self.SLEEPING_BONUS],
            self.CRUSH_GRIP_MOVE_NAME: [str(x) for x in range(100, 0, -1)],
            self.WRING_OUT_MOVE_NAME: [str(x) for x in range(100, 0, -1)],

            self.FRUSTRATION_MOVE_NAME: [str(x) for x in range(102, 0, -1)],
            self.PRESENT_MOVE_NAME: ["40", "80", "120", self.HEAL_OPTION],
            # Attacker's current HP % -- Endeavor has no incoming-damage/HP tracking in
            # this app, so this lets the user model "used it after taking damage" by hand.
            # Default (no selection) is full HP, matching the rest of the app's model.
            self.ENDEAVOR_MOVE_NAME: [str(x) for x in range(100, 0, -10)],
            self.HEX_MOVE_NAME: [self.NO_BONUS, self.STATUS_BONUS],
            self.VENOSHOCK_MOVE_NAME: [self.NO_BONUS, self.POISONED_BONUS],
            self.RETALIATE_MOVE_NAME: [self.NO_BONUS, self.ALLY_FAINTED_BONUS],
            self.ECHOED_VOICE_MOVE_NAME: ["1", "2", "3", "4", "5"],
        }


gen_five_const = GenFiveConstants()
