//! Per-generation constant tables (`gen_X/gen_X_constants.py`). Only the
//! parts that reach behaviour are ported; path constants live in
//! `xpr_core::consts::Paths` / the registry.

use indexmap::IndexMap;
use once_cell::sync::Lazy;

use xpr_core::consts;

use crate::model::Gen;

pub const NO_BONUS: &str = "No Bonus";
pub const DIG_BONUS: &str = "Dig Bonus";
pub const DIVE_BONUS: &str = "Dive Bonus";
pub const FLY_BONUS_GEN2: &str = "Fly Bonus";
pub const FLY_BONUS: &str = "Fly/Bounce Bonus";
pub const SWITCH_BONUS: &str = "Switch Bonus";
pub const MINIMIZE_BONUS: &str = "Minimize Bonus";
pub const STATUS_BONUS: &str = "Status Bonus";
pub const PARALYSIS_BONUS: &str = "Paralysis Bonus";
pub const DAMAGED_BONUS: &str = "Damaged Bonus";
pub const LOW_HEALTH_BONUS: &str = "Low Health Bonus";
pub const SECOND_BONUS: &str = "Move Second Bonus";
pub const SLEEPING_BONUS: &str = "Sleeping Bonus";
pub const POISONED_BONUS: &str = "Poisoned Bonus";
pub const ALLY_FAINTED_BONUS: &str = "Ally Fainted Bonus";
pub const HEAL_OPTION: &str = "Heal";

pub const MAGNITUDE_4: &str = "Mag 4";
pub const MAGNITUDE_5: &str = "Mag 5";
pub const MAGNITUDE_6: &str = "Mag 6";
pub const MAGNITUDE_7: &str = "Mag 7";
pub const MAGNITUDE_8: &str = "Mag 8";
pub const MAGNITUDE_9: &str = "Mag 9";
pub const MAGNITUDE_10: &str = "Mag 10";

pub const FLAIL_FULL_HP: &str = "100-69 % HP";
pub const FLAIL_HALF_HP: &str = "69-35 % HP";
pub const FLAIL_QUARTER_HP: &str = "35-20 % HP";
pub const FLAIL_TEN_PERCENT_HP: &str = "20-10 % HP";
pub const FLAIL_FIVE_PERCENT_HP: &str = "10-4 % HP";
pub const FLAIL_MIN_HP: &str = "4-0 % HP";

pub const PLAIN_TERRAIN: &str = "Plain";
pub const SAND_TERRAIN: &str = "Sand";
pub const CAVE_TERRAIN: &str = "Cave";
pub const ROCK_TERRAIN: &str = "Rock";
pub const TALL_GRASS_TERRAIN: &str = "Tall Grass";
pub const LONG_GRASS_TERRAIN: &str = "Long Grass";
pub const POND_WATER_TERRAIN: &str = "Pond Water";
pub const SEA_WATER_TERRAIN: &str = "Sea Water";
pub const UNDERWATER_TERRAIN: &str = "Underwater";

// gen 1
pub const FLAVOR_ONE_HIT_KO: &str = "one_hit_ko";
pub const FLAVOR_COUNTER: &str = "counter";
pub const FLAVOR_BIDE: &str = "bide";
pub const FLAVOR_SUPER_FANG: &str = "super_fang";
pub const FLAVOR_FOCUS_ENERGY: &str = "focus_energy";
pub const FLAVOR_PARTIAL_TRAPPING: &str = "partial_trapping";
pub const FLAVOR_BEAT_UP: &str = "beat_up";
pub const FLAVOR_FALSE_SWIPE: &str = "false_swipe";

pub const SUPER_FANG_FULL_HP: &str = "Full HP";
pub const SUPER_FANG_75_PERCENT_HP: &str = "75% HP";
pub const SUPER_FANG_50_PERCENT_HP: &str = "50% HP";
pub const SUPER_FANG_25_PERCENT_HP: &str = "25% HP";
pub const SUPER_FANG_10_PERCENT_HP: &str = "10% HP";

pub fn super_fang_hp_percentage(custom: &str) -> i64 {
    match custom {
        SUPER_FANG_FULL_HP => 100,
        SUPER_FANG_75_PERCENT_HP => 75,
        SUPER_FANG_50_PERCENT_HP => 50,
        SUPER_FANG_25_PERCENT_HP => 25,
        SUPER_FANG_10_PERCENT_HP => 10,
        _ => 100,
    }
}

pub const PARTIAL_TRAP_2_TURNS: &str = "2 Turns";
pub const PARTIAL_TRAP_3_TURNS: &str = "3 Turns";
pub const PARTIAL_TRAP_4_TURNS: &str = "4 Turns";
pub const PARTIAL_TRAP_5_TURNS: &str = "5 Turns";

pub fn partial_trap_turn_count(custom: &str) -> i64 {
    match custom {
        PARTIAL_TRAP_2_TURNS => 2,
        PARTIAL_TRAP_3_TURNS => 3,
        PARTIAL_TRAP_4_TURNS => 4,
        PARTIAL_TRAP_5_TURNS => 5,
        _ => 1,
    }
}

const COUNTER_BIDE_CUSTOM_DATA: [&str; 9] = ["25", "50", "75", "100", "150", "200", "250", "300", "400"];

/// Bug-2 ROM `TypeEffects` row order for gen 1.
pub const GEN1_TYPE_EFFECT_ORDER: [(&str, &str); 82] = [
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
];

pub fn gen1_type_row(attacking: &str, defending: &str) -> Option<usize> {
    GEN1_TYPE_EFFECT_ORDER
        .iter()
        .position(|(a, d)| *a == attacking && *d == defending)
}

// gen 2
pub const GEN2_MAROWAK: &str = "Marowak";
pub const GEN2_CUBONE: &str = "Cubone";
pub const THICK_CLUB: &str = "Thick Club";
pub const PIKACHU: &str = "Pikachu";
pub const LIGHT_BALL: &str = "Light Ball";
pub const THUNDER_MOVE: &str = "Thunder";
pub const DITTO: &str = "Ditto";
pub const METAL_POWDER: &str = "Metal Powder";
pub const MAGNITUDE_MOVE: &str = "Magnitude";
pub const FURY_CUTTER_MOVE: &str = "Fury Cutter";
pub const ROLLOUT_MOVE: &str = "Rollout";
pub const ICE_BALL_MOVE: &str = "Ice Ball";
pub const TRIPLE_KICK_MOVE: &str = "Triple Kick";
pub const RAGE_MOVE: &str = "Rage";
pub const PURSUIT_MOVE: &str = "Pursuit";
pub const STOMP_MOVE: &str = "Stomp";
pub const GUST_MOVE: &str = "Gust";
pub const TWISTER_MOVE: &str = "Twister";
pub const EARTHQUAKE_MOVE: &str = "Earthquake";
pub const RETURN_MOVE: &str = "Return";
pub const FRUSTRATION_MOVE: &str = "Frustration";
pub const PRESENT_MOVE: &str = "Present";
pub const PRESENT_HEAL: &str = "Heal";
pub const COUNTER_MOVE: &str = "Counter";
pub const MIRROR_COAT_MOVE: &str = "Mirror Coat";
pub const BIDE_MOVE: &str = "Bide";
pub const METAL_BURST_MOVE: &str = "Metal Burst";
pub const SPIT_UP_MOVE: &str = "Spit Up";
pub const BLIZZARD_MOVE: &str = "Blizzard";
pub const SURF_MOVE: &str = "Surf";
pub const WHIRLPOOL_MOVE: &str = "Whirlpool";
pub const FACADE_MOVE: &str = "Facade";
pub const NEEDLE_ARM_MOVE: &str = "Needle Arm";
pub const ASTONISH_MOVE: &str = "Astonish";
pub const EXTRASENSORY_MOVE: &str = "Extrasensory";
pub const SMELLING_SALT_MOVE: &str = "SmellingSalt";
pub const SMELLING_SALTS_MOVE_GEN5: &str = "Smelling Salts";
pub const REVENGE_MOVE: &str = "Revenge";
pub const NATURE_POWER_MOVE: &str = "Nature Power";
pub const BRICK_BREAK_MOVE: &str = "Brick Break";
pub const ERUPTION_MOVE: &str = "Eruption";
pub const WATER_SPOUT_MOVE: &str = "Water Spout";
pub const SUPER_FANG_MOVE: &str = "Super Fang";
pub const ENDEAVOR_MOVE: &str = "Endeavor";
pub const BEAT_UP_MOVE: &str = "Beat Up";
pub const PRESENT_40: &str = "40 BP";
pub const PRESENT_80: &str = "80 BP";
pub const PRESENT_120: &str = "120 BP";
pub const TRIPLE_KICK_SENTINEL_PREFIX: &str = "__triple_kick_power_";
pub const ASSURANCE_MOVE: &str = "Assurance";
pub const AVALANCHE_MOVE: &str = "Avalanche";
pub const BRINE_MOVE: &str = "Brine";
pub const PAYBACK_MOVE: &str = "Payback";
pub const PUNISHMENT_MOVE: &str = "Punishment";
pub const JUDGMENT_MOVE: &str = "Judgment";
pub const TRUMP_CARD_MOVE: &str = "Trump Card";
pub const WAKE_UP_SLAP_MOVE: &str = "Wake-Up Slap";
pub const CRUSH_GRIP_MOVE: &str = "Crush Grip";
pub const WRING_OUT_MOVE: &str = "Wring Out";
pub const GYRO_BALL_MOVE: &str = "Gyro Ball";
pub const LOW_KICK_MOVE: &str = "Low Kick";
pub const GRASS_KNOT_MOVE: &str = "Grass Knot";
pub const FLING_MOVE: &str = "Fling";
pub const STEAMROLLER_MOVE: &str = "Steamroller";
pub const HEAVY_SLAM_MOVE: &str = "Heavy Slam";
pub const HEAT_CRASH_MOVE: &str = "Heat Crash";
pub const ELECTRO_BALL_MOVE: &str = "Electro Ball";
pub const STORED_POWER_MOVE: &str = "Stored Power";
pub const HEX_MOVE: &str = "Hex";
pub const VENOSHOCK_MOVE: &str = "Venoshock";
pub const RETALIATE_MOVE: &str = "Retaliate";
pub const ECHOED_VOICE_MOVE: &str = "Echoed Voice";
pub const PSYSHOCK_MOVE: &str = "Psyshock";
pub const PSYSTRIKE_MOVE: &str = "Psystrike";
pub const SECRET_SWORD_MOVE: &str = "Secret Sword";
pub const FOUL_PLAY_MOVE: &str = "Foul Play";
pub const CHIP_AWAY_MOVE: &str = "Chip Away";
pub const SACRED_SWORD_MOVE: &str = "Sacred Sword";
pub const ACROBATICS_MOVE: &str = "Acrobatics";
pub const FINAL_GAMBIT_MOVE: &str = "Final Gambit";
pub const FROST_BREATH_MOVE: &str = "Frost Breath";
pub const STORM_THROW_MOVE: &str = "Storm Throw";
pub const HURRICANE_MOVE: &str = "Hurricane";
pub const SELFDESTRUCT_MOVE_GEN5: &str = "Self-Destruct";
pub const SOLAR_BEAM_MOVE_GEN5: &str = "Solar Beam";

// gen 3+ names
pub const SEA_INCENSE: &str = "Sea Incense";
pub const CLAMPERL: &str = "Clamperl";
pub const DEEP_SEA_TOOTH: &str = "DeepSeaTooth";
pub const DEEP_SEA_SCALE: &str = "DeepSeaScale";
pub const CHOICE_BAND: &str = "Choice Band";
pub const CHOICE_SPECS: &str = "Choice Specs";
pub const WIDE_LENS: &str = "Wide Lens";
pub const LATIOS: &str = "Latios";
pub const LATIAS: &str = "Latias";
pub const SOUL_DEW: &str = "Soul Dew";
pub const DIALGA: &str = "Dialga";
pub const ADAMANT_ORB: &str = "Adamant Orb";
pub const PALKIA: &str = "Palkia";
pub const LUSTROUS_ORB: &str = "Lustrous Orb";
pub const GIRATINA: &str = "Giratina";
pub const GRISEOUS_ORB: &str = "Griseous Orb";
pub const SCOPE_LENS: &str = "Scope Lens";
pub const RAZOR_CLAW: &str = "Razor Claw";
pub const LUCKY_PUNCH: &str = "Lucky Punch";
pub const STICK: &str = "Stick";
pub const CHANSEY: &str = "Chansey";
pub const FARFETCHD: &str = "Farfetch'd";
pub const LIFE_ORB: &str = "Life Orb";
pub const EXPERT_BELT: &str = "Expert Belt";
pub const MUSCLE_BAND: &str = "Muscle Band";
pub const WISE_GLASSES: &str = "Wise Glasses";
pub const BRIGHT_POWDER: &str = "BrightPowder";
pub const LAX_INCENSE: &str = "Lax Incense";
pub const IRON_BALL: &str = "Iron Ball";

pub const COMPOUND_EYES: &str = "Compound Eyes";
pub const LEVITATE: &str = "Levitate";
pub const DAMP: &str = "Damp";
pub const VOLT_ABSORB: &str = "Volt Absorb";
pub const LIGHTNING_ROD: &str = "Lightning Rod";
pub const WATER_ABSORB: &str = "Water Absorb";
pub const FLASH_FIRE: &str = "Flash Fire";
pub const WONDER_GUARD: &str = "Wonder Guard";
pub const BATTLE_ARMOR: &str = "Battle Armor";
pub const SHELL_ARMOR: &str = "Shell Armor";
pub const HUSTLE: &str = "Hustle";
pub const SAND_VEIL: &str = "Sand Veil";
pub const HUGE_POWER: &str = "Huge Power";
pub const PURE_POWER: &str = "Pure Power";
pub const THICK_FAT: &str = "Thick Fat";
pub const SOUNDPROOF: &str = "Soundproof";
pub const STURDY: &str = "Sturdy";
pub const NO_GUARD: &str = "No Guard";
pub const SCRAPPY: &str = "Scrappy";
pub const SNIPER: &str = "Sniper";
pub const SNOW_CLOAK: &str = "Snow Cloak";
pub const SUPER_LUCK: &str = "Super Luck";
pub const ADAPTABILITY: &str = "Adaptability";
pub const DRY_SKIN: &str = "Dry Skin";
pub const FILTER: &str = "Filter";
pub const SOLID_ROCK: &str = "Solid Rock";
pub const FLOWER_GIFT: &str = "Flower Gift";
pub const HEATPROOF: &str = "Heatproof";
pub const KLUTZ: &str = "Klutz";
pub const MOTOR_DRIVE: &str = "Motor Drive";
pub const NORMALIZE: &str = "Normalize";
pub const SOLAR_POWER: &str = "Solar Power";
pub const TECHNICIAN: &str = "Technician";
pub const TINTED_LENS: &str = "Tinted Lens";
pub const MULTITYPE: &str = "Multitype";
pub const INSOMNIA: &str = "Insomnia";
pub const SLOW_START: &str = "Slow Start";
pub const IRON_FIST: &str = "Iron Fist";
pub const RECKLESS: &str = "Reckless";

pub const GEN3_SOUND_MOVES: [&str; 10] = [
    "Snore", "Uproar", "Hyper Voice", "Growl", "Roar", "Sing", "Supersonic", "Screech", "Metal Sound", "GrassWhistle",
];
pub const GEN4_SOUND_MOVES: [&str; 5] = ["Uproar", "Snore", "Hyper Voice", "Bug Buzz", "Chatter"];
pub const GEN4_PUNCH_MOVES: [&str; 15] = [
    "Ice Punch", "Fire Punch", "ThunderPunch", "Mach Punch", "Focus Punch", "Dizzy Punch", "DynamicPunch",
    "Hammer Arm", "Mega Punch", "Comet Punch", "Meteor Mash", "Shadow Punch", "Drain Punch", "Bullet Punch",
    "Sky Uppercut",
];
pub const GEN4_RECKLESS_MOVES: [&str; 10] = [
    "Jump Kick", "Hi Jump Kick", "Take Down", "Submission", "Double-Edge", "Volt Tackle", "Brave Bird",
    "Wood Hammer", "Flare Blitz", "Head Smash",
];
pub const OHKO_MOVE_NAMES: [&str; 4] = ["Guillotine", "Horn Drill", "Fissure", "Sheer Cold"];
pub const GEN4_TARGETING_ALL_FOES: &str = "All Foes";
pub const GEN4_TARGETING_OTHERS: &str = "Others";

pub const GEN5_USES_TARGET_DEFENSE_MOVES: [&str; 3] = [PSYSHOCK_MOVE, PSYSTRIKE_MOVE, SECRET_SWORD_MOVE];
pub const GEN5_IGNORES_DEFENSE_STAGES_MOVES: [&str; 2] = [CHIP_AWAY_MOVE, SACRED_SWORD_MOVE];
pub const GEN5_ALWAYS_CRIT_MOVES: [&str; 2] = [FROST_BREATH_MOVE, STORM_THROW_MOVE];

pub const PLATE_TYPE_LOOKUP: [(&str, &str); 16] = [
    ("Draco Plate", consts::TYPE_DRAGON),
    ("Dread Plate", consts::TYPE_DARK),
    ("Earth Plate", consts::TYPE_GROUND),
    ("Fist Plate", consts::TYPE_FIGHTING),
    ("Flame Plate", consts::TYPE_FIRE),
    ("Icicle Plate", consts::TYPE_ICE),
    ("Insect Plate", consts::TYPE_BUG),
    ("Iron Plate", consts::TYPE_STEEL),
    ("Meadow Plate", consts::TYPE_GRASS),
    ("Mind Plate", consts::TYPE_PSYCHIC),
    ("Sky Plate", consts::TYPE_FLYING),
    ("Splash Plate", consts::TYPE_WATER),
    ("Spooky Plate", consts::TYPE_GHOST),
    ("Stone Plate", consts::TYPE_ROCK),
    ("Toxic Plate", consts::TYPE_POISON),
    ("Zap Plate", consts::TYPE_ELECTRIC),
];

pub fn plate_type(item: Option<&str>) -> Option<&'static str> {
    let item = item?;
    PLATE_TYPE_LOOKUP.iter().find(|(i, _)| *i == item).map(|(_, t)| *t)
}

/// Gen 4 Nature Power terrain -> (base_power, type, accuracy); the called
/// move's category and targeting are in `gen4_nature_power_move`.
pub const NATURE_POWER_PLAIN_SAND: &str = "Plain/Sand";
pub const NATURE_POWER_GRASS_PUDDLE: &str = "Grass/Puddle";
pub const NATURE_POWER_MOUNTAIN_CAVE: &str = "Mountain/Cave";
pub const NATURE_POWER_SNOW: &str = "Snow";
pub const NATURE_POWER_WATER: &str = "Water";
pub const NATURE_POWER_ICE: &str = "Ice";
pub const NATURE_POWER_BUILDING: &str = "Building";
pub const NATURE_POWER_GREAT_MARSH: &str = "Great Marsh";
pub const NATURE_POWER_BRIDGE: &str = "Bridge";

pub const GEN4_NATURE_POWER_TABLE: [(&str, i64, &str, i64); 9] = [
    (NATURE_POWER_PLAIN_SAND, 100, consts::TYPE_GROUND, 100),
    (NATURE_POWER_GRASS_PUDDLE, 80, consts::TYPE_GRASS, 100),
    (NATURE_POWER_MOUNTAIN_CAVE, 75, consts::TYPE_ROCK, 90),
    (NATURE_POWER_SNOW, 120, consts::TYPE_ICE, 70),
    (NATURE_POWER_WATER, 120, consts::TYPE_WATER, 80),
    (NATURE_POWER_ICE, 95, consts::TYPE_ICE, 100),
    (NATURE_POWER_BUILDING, 80, consts::TYPE_NORMAL, 100),
    (NATURE_POWER_GREAT_MARSH, 65, consts::TYPE_GROUND, 85),
    (NATURE_POWER_BRIDGE, 75, consts::TYPE_FLYING, 95),
];

pub fn gen4_nature_power(terrain: &str) -> Option<(i64, &'static str, i64)> {
    GEN4_NATURE_POWER_TABLE
        .iter()
        .find(|(t, _, _, _)| *t == terrain)
        .map(|(_, p, ty, acc)| (*p, *ty, *acc))
}

pub const GEN4_NATURE_POWER_PHYSICAL_TERRAINS: [&str; 3] =
    [NATURE_POWER_PLAIN_SAND, NATURE_POWER_GRASS_PUDDLE, NATURE_POWER_MOUNTAIN_CAVE];

/// Gen 4 Nature Power terrain -> (called move, category, targeting)
/// (`include/data/terrain/to_move.h`: Earthquake, Seed Bomb, Rock Slide,
/// Blizzard, Hydro Pump, Ice Beam, Tri Attack, Mud Bomb, Air Slash).
pub fn gen4_nature_power_move(terrain: &str) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match terrain {
        NATURE_POWER_PLAIN_SAND => ("Earthquake", consts::CATEGORY_PHYSICAL, GEN4_TARGETING_OTHERS),
        NATURE_POWER_GRASS_PUDDLE => ("Seed Bomb", consts::CATEGORY_PHYSICAL, "Foe Or Ally"),
        NATURE_POWER_MOUNTAIN_CAVE => ("Rock Slide", consts::CATEGORY_PHYSICAL, GEN4_TARGETING_ALL_FOES),
        NATURE_POWER_SNOW => ("Blizzard", consts::CATEGORY_SPECIAL, GEN4_TARGETING_ALL_FOES),
        NATURE_POWER_WATER => ("Hydro Pump", consts::CATEGORY_SPECIAL, "Foe Or Ally"),
        NATURE_POWER_ICE => ("Ice Beam", consts::CATEGORY_SPECIAL, "Foe Or Ally"),
        NATURE_POWER_BUILDING => ("Tri Attack", consts::CATEGORY_SPECIAL, "Foe Or Ally"),
        NATURE_POWER_GREAT_MARSH => ("Mud Bomb", consts::CATEGORY_SPECIAL, "Foe Or Ally"),
        NATURE_POWER_BRIDGE => ("Air Slash", consts::CATEGORY_SPECIAL, "Foe Or Ally"),
        _ => return None,
    })
}

/// (berry, gen-4 power, gen-5 power, type). Gen 5 raised every power by 20
/// except the resistance berries (Occa..Chilan), which the Python table
/// lists at 100 rather than 80.
const NATURAL_GIFT_TABLE: &[(&str, i64, i64, &str)] = &[
    ("Cheri Berry", 60, 80, consts::TYPE_FIRE),
    ("Chesto Berry", 60, 80, consts::TYPE_WATER),
    ("Pecha Berry", 60, 80, consts::TYPE_ELECTRIC),
    ("Rawst Berry", 60, 80, consts::TYPE_GRASS),
    ("Aspear Berry", 60, 80, consts::TYPE_ICE),
    ("Leppa Berry", 60, 80, consts::TYPE_FIGHTING),
    ("Oran Berry", 60, 80, consts::TYPE_POISON),
    ("Persim Berry", 60, 80, consts::TYPE_GROUND),
    ("Lum Berry", 60, 80, consts::TYPE_FLYING),
    ("Sitrus Berry", 60, 80, consts::TYPE_PSYCHIC),
    ("Figy Berry", 60, 80, consts::TYPE_BUG),
    ("Wiki Berry", 60, 80, consts::TYPE_ROCK),
    ("Mago Berry", 60, 80, consts::TYPE_GHOST),
    ("Aguav Berry", 60, 80, consts::TYPE_DRAGON),
    ("Iapapa Berry", 60, 80, consts::TYPE_DARK),
    ("Razz Berry", 60, 80, consts::TYPE_STEEL),
    ("Bluk Berry", 70, 90, consts::TYPE_FIRE),
    ("Nanab Berry", 70, 90, consts::TYPE_WATER),
    ("Wepear Berry", 70, 90, consts::TYPE_ELECTRIC),
    ("Pinap Berry", 70, 90, consts::TYPE_GRASS),
    ("Pomeg Berry", 70, 90, consts::TYPE_ICE),
    ("Kelpsy Berry", 70, 90, consts::TYPE_FIGHTING),
    ("Qualot Berry", 70, 90, consts::TYPE_POISON),
    ("Hondew Berry", 70, 90, consts::TYPE_GROUND),
    ("Grepa Berry", 70, 90, consts::TYPE_FLYING),
    ("Tamato Berry", 70, 90, consts::TYPE_PSYCHIC),
    ("Cornn Berry", 70, 90, consts::TYPE_BUG),
    ("Magost Berry", 70, 90, consts::TYPE_ROCK),
    ("Rabuta Berry", 70, 90, consts::TYPE_GHOST),
    ("Nomel Berry", 70, 90, consts::TYPE_DRAGON),
    ("Spelon Berry", 70, 90, consts::TYPE_DARK),
    ("Pamtre Berry", 70, 90, consts::TYPE_STEEL),
    ("Watmel Berry", 80, 100, consts::TYPE_FIRE),
    ("Durin Berry", 80, 100, consts::TYPE_WATER),
    ("Belue Berry", 80, 100, consts::TYPE_ELECTRIC),
    ("Occa Berry", 60, 100, consts::TYPE_FIRE),
    ("Passho Berry", 60, 100, consts::TYPE_WATER),
    ("Wacan Berry", 60, 100, consts::TYPE_ELECTRIC),
    ("Rindo Berry", 60, 100, consts::TYPE_GRASS),
    ("Yache Berry", 60, 100, consts::TYPE_ICE),
    ("Chople Berry", 60, 100, consts::TYPE_FIGHTING),
    ("Kebia Berry", 60, 100, consts::TYPE_POISON),
    ("Shuca Berry", 60, 100, consts::TYPE_GROUND),
    ("Coba Berry", 60, 100, consts::TYPE_FLYING),
    ("Payapa Berry", 60, 100, consts::TYPE_PSYCHIC),
    ("Tanga Berry", 60, 100, consts::TYPE_BUG),
    ("Charti Berry", 60, 100, consts::TYPE_ROCK),
    ("Kasib Berry", 60, 100, consts::TYPE_GHOST),
    ("Haban Berry", 60, 100, consts::TYPE_DRAGON),
    ("Colbur Berry", 60, 100, consts::TYPE_DARK),
    ("Babiri Berry", 60, 100, consts::TYPE_STEEL),
    ("Chilan Berry", 60, 100, consts::TYPE_NORMAL),
    ("Liechi Berry", 80, 100, consts::TYPE_GRASS),
    ("Ganlon Berry", 80, 100, consts::TYPE_ICE),
    ("Salac Berry", 80, 100, consts::TYPE_FIGHTING),
    ("Petaya Berry", 80, 100, consts::TYPE_POISON),
    ("Apicot Berry", 80, 100, consts::TYPE_GROUND),
    ("Lansat Berry", 80, 100, consts::TYPE_FLYING),
    ("Starf Berry", 80, 100, consts::TYPE_PSYCHIC),
    ("Enigma Berry", 80, 100, consts::TYPE_BUG),
    ("Micle Berry", 80, 100, consts::TYPE_ROCK),
    ("Custap Berry", 80, 100, consts::TYPE_GHOST),
    ("Jaboca Berry", 80, 100, consts::TYPE_DRAGON),
    ("Rowap Berry", 80, 100, consts::TYPE_DARK),
];

pub static GEN4_NATURAL_GIFT: Lazy<IndexMap<&'static str, (i64, &'static str)>> = Lazy::new(|| {
    NATURAL_GIFT_TABLE.iter().map(|(b, p4, _, t)| (*b, (*p4, *t))).collect()
});
pub static GEN5_NATURAL_GIFT: Lazy<IndexMap<&'static str, (i64, &'static str)>> = Lazy::new(|| {
    NATURAL_GIFT_TABLE.iter().map(|(b, _, p5, t)| (*b, (*p5, *t))).collect()
});

pub fn natural_gift(gen: Gen, held_item: Option<&str>) -> Option<(i64, &'static str)> {
    let item = held_item?;
    match gen {
        Gen::Four => GEN4_NATURAL_GIFT.get(item).copied(),
        Gen::Five => GEN5_NATURAL_GIFT.get(item).copied(),
        _ => None,
    }
}

pub static GEN4_FLING_POWER: Lazy<IndexMap<&'static str, i64>> = Lazy::new(|| {
    let entries: &[(&str, i64)] = &[
        ("Iron Ball", 130),
        ("Hard Stone", 100), ("Rare Bone", 100),
        ("Helix Fossil", 100), ("Dome Fossil", 100), ("Old Amber", 100), ("Root Fossil", 100),
        ("Claw Fossil", 100), ("Armor Fossil", 100), ("Skull Fossil", 100),
        ("Draco Plate", 90), ("Dread Plate", 90), ("Earth Plate", 90), ("Fist Plate", 90),
        ("Flame Plate", 90), ("Icicle Plate", 90), ("Insect Plate", 90), ("Iron Plate", 90),
        ("Meadow Plate", 90), ("Mind Plate", 90), ("Sky Plate", 90), ("Splash Plate", 90),
        ("Spooky Plate", 90), ("Stone Plate", 90), ("Toxic Plate", 90), ("Zap Plate", 90),
        ("DeepSeaTooth", 90), ("Thick Club", 90), ("Grip Claw", 90),
        ("Razor Claw", 80), ("Quick Claw", 80), ("Sticky Barb", 80), ("Dawn Stone", 80),
        ("Dusk Stone", 80), ("Shiny Stone", 80), ("Electirizer", 80), ("Magmarizer", 80),
        ("Protector", 80), ("Oval Stone", 80), ("Odd Keystone", 80),
        ("Dragon Fang", 70), ("Poison Barb", 70), ("Power Anklet", 70), ("Power Band", 70),
        ("Power Belt", 70), ("Power Bracer", 70), ("Power Lens", 70), ("Power Weight", 70),
        ("Adamant Orb", 60), ("Lustrous Orb", 60), ("Griseous Orb", 60), ("Damp Rock", 60),
        ("Heat Rock", 60), ("Macho Brace", 60), ("Stick", 60),
        ("Sharp Beak", 50), ("Dubious Disc", 50),
        ("Lucky Punch", 40), ("Icy Rock", 40),
        ("Life Orb", 30), ("Light Ball", 30), ("Scope Lens", 30), ("Metronome", 30),
        ("Soul Dew", 30), ("DeepSeaScale", 30), ("King's Rock", 30), ("Razor Fang", 30),
        ("Shell Bell", 30), ("Amulet Coin", 30), ("Lucky Egg", 30), ("Everstone", 30),
        ("Exp. Share", 30), ("Black Sludge", 30), ("Flame Orb", 30), ("Toxic Orb", 30),
        ("Light Clay", 30), ("Cleanse Tag", 30), ("Smoke Ball", 30), ("Up-Grade", 30),
        ("Dragon Scale", 30), ("Black Belt", 30), ("BlackGlasses", 30), ("Charcoal", 30),
        ("Magnet", 30), ("Metal Coat", 30), ("Miracle Seed", 30), ("Mystic Water", 30),
        ("NeverMeltIce", 30), ("Spell Tag", 30), ("TwistedSpoon", 30),
        ("Silk Scarf", 10), ("SilverPowder", 10), ("Soft Sand", 10),
        ("Choice Band", 10), ("Choice Specs", 10), ("Choice Scarf", 10),
        ("Expert Belt", 10), ("Focus Band", 10), ("Focus Sash", 10), ("Muscle Band", 10),
        ("Wise Glasses", 10), ("Wide Lens", 10), ("Zoom Lens", 10), ("BrightPowder", 10),
        ("Lax Incense", 10), ("Full Incense", 10), ("Odd Incense", 10), ("Rock Incense", 10),
        ("Rose Incense", 10), ("Sea Incense", 10), ("Wave Incense", 10), ("Luck Incense", 10),
        ("Pure Incense", 10), ("Leftovers", 10), ("Metal Powder", 10), ("Quick Powder", 10),
        ("Big Root", 10), ("Destiny Knot", 10), ("Mental Herb", 10), ("Power Herb", 10),
        ("Shed Shell", 10), ("Smooth Rock", 10), ("Soothe Bell", 10), ("White Herb", 10),
        ("Lagging Tail", 10), ("Reaper Cloth", 10),
    ];
    entries.iter().copied().collect()
});

fn s(v: &str) -> String {
    v.to_string()
}

fn range_strings(from: i64, to_exclusive: i64, step: i64) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = from;
    if step > 0 {
        while i < to_exclusive {
            out.push(i.to_string());
            i += step;
        }
    } else {
        while i > to_exclusive {
            out.push(i.to_string());
            i += step;
        }
    }
    out
}

fn magnitude_options() -> Vec<String> {
    let mut v = Vec::new();
    for m in [MAGNITUDE_7, MAGNITUDE_4, MAGNITUDE_5, MAGNITUDE_6, MAGNITUDE_8, MAGNITUDE_9, MAGNITUDE_10] {
        v.push(s(m));
        v.push(format!("{} {}", m, DIG_BONUS));
    }
    v
}

fn flail_options() -> Vec<String> {
    vec![
        s(FLAIL_FULL_HP),
        s(FLAIL_HALF_HP),
        s(FLAIL_QUARTER_HP),
        s(FLAIL_TEN_PERCENT_HP),
        s(FLAIL_FIVE_PERCENT_HP),
        s(FLAIL_MIN_HP),
    ]
}

fn terrain_options() -> Vec<String> {
    vec![
        s(PLAIN_TERRAIN),
        s(SAND_TERRAIN),
        s(CAVE_TERRAIN),
        s(ROCK_TERRAIN),
        s(TALL_GRASS_TERRAIN),
        s(LONG_GRASS_TERRAIN),
        s(POND_WATER_TERRAIN),
        s(SEA_WATER_TERRAIN),
        s(UNDERWATER_TERRAIN),
    ]
}

fn two(a: &str, b: &str) -> Vec<String> {
    vec![s(a), s(b)]
}

/// `CUSTOM_MOVE_DATA` per gen (move name -> dropdown options).
pub fn custom_move_data_table(gen: Gen) -> &'static IndexMap<&'static str, Vec<String>> {
    static GEN1: Lazy<IndexMap<&'static str, Vec<String>>> = Lazy::new(|| {
        let mut m = IndexMap::new();
        m.insert(
            "Super Fang",
            vec![
                s(SUPER_FANG_FULL_HP),
                s(SUPER_FANG_75_PERCENT_HP),
                s(SUPER_FANG_50_PERCENT_HP),
                s(SUPER_FANG_25_PERCENT_HP),
                s(SUPER_FANG_10_PERCENT_HP),
            ],
        );
        let trap = vec![s(PARTIAL_TRAP_2_TURNS), s(PARTIAL_TRAP_3_TURNS), s(PARTIAL_TRAP_4_TURNS), s(PARTIAL_TRAP_5_TURNS)];
        m.insert("Bind", trap.clone());
        m.insert("Wrap", trap.clone());
        m.insert("Fire Spin", trap.clone());
        m.insert("Clamp", trap);
        let cb: Vec<String> = COUNTER_BIDE_CUSTOM_DATA.iter().map(|x| s(x)).collect();
        m.insert("Counter", cb.clone());
        m.insert("Bide", cb);
        m
    });
    static GEN2: Lazy<IndexMap<&'static str, Vec<String>>> = Lazy::new(|| {
        let mut m = IndexMap::new();
        m.insert(MAGNITUDE_MOVE, magnitude_options());
        m.insert(consts::FLAIL_MOVE_NAME, flail_options());
        m.insert(consts::REVERSAL_MOVE_NAME, flail_options());
        m.insert(FURY_CUTTER_MOVE, range_strings(1, 7, 1));
        m.insert(ROLLOUT_MOVE, vec![s("1"), s("2"), s("3"), s("4"), s("5"), s("5 + DefenseCurl")]);
        m.insert(TRIPLE_KICK_MOVE, vec![s("1 Kick"), s("2 Kicks"), s("3 Kicks")]);
        m.insert(RAGE_MOVE, range_strings(1, 7, 1));
        m.insert(PURSUIT_MOVE, two(NO_BONUS, SWITCH_BONUS));
        m.insert(STOMP_MOVE, two(NO_BONUS, MINIMIZE_BONUS));
        m.insert(GUST_MOVE, two(NO_BONUS, FLY_BONUS_GEN2));
        m.insert(TWISTER_MOVE, two(NO_BONUS, FLY_BONUS_GEN2));
        m.insert(EARTHQUAKE_MOVE, two(NO_BONUS, DIG_BONUS));
        m.insert(RETURN_MOVE, range_strings(102, 0, -1));
        m.insert(FRUSTRATION_MOVE, range_strings(102, 0, -1));
        m.insert(PRESENT_MOVE, vec![s("40"), s("80"), s("120"), s(PRESENT_HEAL)]);
        let cb: Vec<String> = COUNTER_BIDE_CUSTOM_DATA.iter().map(|x| s(x)).collect();
        m.insert(COUNTER_MOVE, cb.clone());
        m.insert(MIRROR_COAT_MOVE, cb.clone());
        m.insert(BIDE_MOVE, cb);
        m.insert(BEAT_UP_MOVE, range_strings(1, 7, 1));
        m
    });
    static GEN3: Lazy<IndexMap<&'static str, Vec<String>>> = Lazy::new(|| {
        let mut m = IndexMap::new();
        m.insert(MAGNITUDE_MOVE, magnitude_options());
        m.insert(consts::FLAIL_MOVE_NAME, flail_options());
        m.insert(consts::REVERSAL_MOVE_NAME, flail_options());
        m.insert(NATURE_POWER_MOVE, terrain_options());
        m.insert(FURY_CUTTER_MOVE, range_strings(1, 7, 1));
        m.insert(ROLLOUT_MOVE, vec![s("1"), s("2"), s("3"), s("4"), s("5"), s("5 + DefenseCurl")]);
        m.insert(ICE_BALL_MOVE, vec![s("1"), s("2"), s("3"), s("4"), s("5"), s("5 + DefenseCurl")]);
        m.insert(TRIPLE_KICK_MOVE, range_strings(1, 4, 1));
        m.insert(PURSUIT_MOVE, two(NO_BONUS, SWITCH_BONUS));
        m.insert(STOMP_MOVE, two(NO_BONUS, MINIMIZE_BONUS));
        m.insert(ASTONISH_MOVE, two(NO_BONUS, MINIMIZE_BONUS));
        m.insert(NEEDLE_ARM_MOVE, two(NO_BONUS, MINIMIZE_BONUS));
        m.insert(EXTRASENSORY_MOVE, two(NO_BONUS, MINIMIZE_BONUS));
        m.insert(GUST_MOVE, two(NO_BONUS, FLY_BONUS));
        m.insert(TWISTER_MOVE, two(NO_BONUS, FLY_BONUS));
        m.insert(EARTHQUAKE_MOVE, two(NO_BONUS, DIG_BONUS));
        m.insert(SURF_MOVE, two(NO_BONUS, DIVE_BONUS));
        m.insert(WHIRLPOOL_MOVE, two(NO_BONUS, DIVE_BONUS));
        m.insert(FACADE_MOVE, two(NO_BONUS, STATUS_BONUS));
        m.insert(SMELLING_SALT_MOVE, two(NO_BONUS, PARALYSIS_BONUS));
        m.insert(REVENGE_MOVE, two(NO_BONUS, DAMAGED_BONUS));
        m.insert(RETURN_MOVE, range_strings(102, 0, -1));
        m.insert(FRUSTRATION_MOVE, range_strings(102, 0, -1));
        m.insert(ERUPTION_MOVE, range_strings(100, 0, -1));
        m.insert(WATER_SPOUT_MOVE, range_strings(100, 0, -1));
        m.insert(ENDEAVOR_MOVE, range_strings(100, 0, -1));
        m.insert(SPIT_UP_MOVE, range_strings(1, 4, 1));
        m.insert(PRESENT_MOVE, vec![s(PRESENT_40), s(PRESENT_80), s(PRESENT_120)]);
        m.insert(BEAT_UP_MOVE, range_strings(1, 7, 1));
        let cb: Vec<String> = COUNTER_BIDE_CUSTOM_DATA.iter().map(|x| s(x)).collect();
        m.insert(COUNTER_MOVE, cb.clone());
        m.insert(MIRROR_COAT_MOVE, cb.clone());
        m.insert(BIDE_MOVE, cb);
        m
    });
    static GEN4: Lazy<IndexMap<&'static str, Vec<String>>> = Lazy::new(|| {
        let mut m = IndexMap::new();
        m.insert(MAGNITUDE_MOVE, magnitude_options());
        m.insert(consts::FLAIL_MOVE_NAME, flail_options());
        m.insert(consts::REVERSAL_MOVE_NAME, flail_options());
        m.insert(NATURE_POWER_MOVE, GEN4_NATURE_POWER_TABLE.iter().map(|(t, _, _, _)| s(t)).collect());
        m.insert(FURY_CUTTER_MOVE, range_strings(1, 6, 1));
        m.insert(ROLLOUT_MOVE, vec![s("1"), s("2"), s("3"), s("4"), s("5"), s("5 + DefenseCurl")]);
        m.insert(ICE_BALL_MOVE, vec![s("1"), s("2"), s("3"), s("4"), s("5"), s("5 + DefenseCurl")]);
        m.insert(TRIPLE_KICK_MOVE, range_strings(1, 4, 1));
        m.insert(PURSUIT_MOVE, two(NO_BONUS, SWITCH_BONUS));
        m.insert(STOMP_MOVE, two(NO_BONUS, MINIMIZE_BONUS));
        m.insert(GUST_MOVE, two(NO_BONUS, FLY_BONUS));
        m.insert(TWISTER_MOVE, two(NO_BONUS, FLY_BONUS));
        m.insert(EARTHQUAKE_MOVE, two(NO_BONUS, DIG_BONUS));
        m.insert(SURF_MOVE, two(NO_BONUS, DIVE_BONUS));
        m.insert(WHIRLPOOL_MOVE, two(NO_BONUS, DIVE_BONUS));
        m.insert(FACADE_MOVE, two(NO_BONUS, STATUS_BONUS));
        m.insert(SMELLING_SALT_MOVE, two(NO_BONUS, PARALYSIS_BONUS));
        m.insert(REVENGE_MOVE, two(NO_BONUS, DAMAGED_BONUS));
        m.insert(RETURN_MOVE, range_strings(102, 0, -1));
        m.insert(FRUSTRATION_MOVE, range_strings(102, 0, -1));
        m.insert(ERUPTION_MOVE, range_strings(100, 0, -1));
        m.insert(WATER_SPOUT_MOVE, range_strings(100, 0, -1));
        m.insert(SPIT_UP_MOVE, range_strings(1, 4, 1));
        m.insert(PRESENT_MOVE, vec![s("40"), s("80"), s("120"), s("Heal")]);
        m.insert(ASSURANCE_MOVE, two(NO_BONUS, DAMAGED_BONUS));
        m.insert(AVALANCHE_MOVE, two(NO_BONUS, DAMAGED_BONUS));
        m.insert(BRINE_MOVE, two(NO_BONUS, LOW_HEALTH_BONUS));
        m.insert(PAYBACK_MOVE, two(NO_BONUS, SECOND_BONUS));
        m.insert(TRUMP_CARD_MOVE, vec![s("4+"), s("3"), s("2"), s("1"), s("0")]);
        m.insert(WAKE_UP_SLAP_MOVE, two(NO_BONUS, SLEEPING_BONUS));
        m.insert(CRUSH_GRIP_MOVE, range_strings(100, 0, -1));
        m.insert(WRING_OUT_MOVE, range_strings(100, 0, -1));
        let cb: Vec<String> = COUNTER_BIDE_CUSTOM_DATA.iter().map(|x| s(x)).collect();
        m.insert(COUNTER_MOVE, cb.clone());
        m.insert(MIRROR_COAT_MOVE, cb.clone());
        m.insert(METAL_BURST_MOVE, cb.clone());
        m.insert(BIDE_MOVE, cb);
        m.insert(BEAT_UP_MOVE, range_strings(1, 7, 1));
        m
    });
    static GEN5: Lazy<IndexMap<&'static str, Vec<String>>> = Lazy::new(|| {
        let mut m = IndexMap::new();
        m.insert(MAGNITUDE_MOVE, magnitude_options());
        m.insert(consts::FLAIL_MOVE_NAME, flail_options());
        m.insert(consts::REVERSAL_MOVE_NAME, flail_options());
        m.insert(NATURE_POWER_MOVE, terrain_options());
        m.insert(FURY_CUTTER_MOVE, range_strings(1, 7, 1));
        m.insert(ROLLOUT_MOVE, vec![s("1"), s("2"), s("3"), s("4"), s("5"), s("5 + DefenseCurl")]);
        m.insert(ICE_BALL_MOVE, vec![s("1"), s("2"), s("3"), s("4"), s("5"), s("5 + DefenseCurl")]);
        m.insert(TRIPLE_KICK_MOVE, range_strings(1, 4, 1));
        m.insert(PURSUIT_MOVE, two(NO_BONUS, SWITCH_BONUS));
        m.insert(STOMP_MOVE, two(NO_BONUS, MINIMIZE_BONUS));
        m.insert(STEAMROLLER_MOVE, two(NO_BONUS, MINIMIZE_BONUS));
        m.insert(GUST_MOVE, two(NO_BONUS, FLY_BONUS));
        m.insert(TWISTER_MOVE, two(NO_BONUS, FLY_BONUS));
        m.insert(EARTHQUAKE_MOVE, two(NO_BONUS, DIG_BONUS));
        m.insert(SURF_MOVE, two(NO_BONUS, DIVE_BONUS));
        m.insert(WHIRLPOOL_MOVE, two(NO_BONUS, DIVE_BONUS));
        m.insert(FACADE_MOVE, two(NO_BONUS, STATUS_BONUS));
        m.insert(SMELLING_SALTS_MOVE_GEN5, two(NO_BONUS, PARALYSIS_BONUS));
        m.insert(REVENGE_MOVE, two(NO_BONUS, DAMAGED_BONUS));
        m.insert(RETURN_MOVE, range_strings(102, 0, -1));
        m.insert(ERUPTION_MOVE, range_strings(100, 0, -1));
        m.insert(WATER_SPOUT_MOVE, range_strings(100, 0, -1));
        m.insert(SPIT_UP_MOVE, range_strings(1, 4, 1));
        m.insert(ASSURANCE_MOVE, two(NO_BONUS, DAMAGED_BONUS));
        m.insert(AVALANCHE_MOVE, two(NO_BONUS, DAMAGED_BONUS));
        m.insert(BRINE_MOVE, two(NO_BONUS, LOW_HEALTH_BONUS));
        m.insert(PAYBACK_MOVE, two(NO_BONUS, SECOND_BONUS));
        m.insert(TRUMP_CARD_MOVE, vec![s("4+"), s("3"), s("2"), s("1"), s("0")]);
        m.insert(WAKE_UP_SLAP_MOVE, two(NO_BONUS, SLEEPING_BONUS));
        m.insert(CRUSH_GRIP_MOVE, range_strings(100, 0, -1));
        m.insert(WRING_OUT_MOVE, range_strings(100, 0, -1));
        m.insert(FRUSTRATION_MOVE, range_strings(102, 0, -1));
        m.insert(PRESENT_MOVE, vec![s("40"), s("80"), s("120"), s(HEAL_OPTION)]);
        m.insert(ENDEAVOR_MOVE, range_strings(100, 0, -10));
        m.insert(HEX_MOVE, two(NO_BONUS, STATUS_BONUS));
        m.insert(VENOSHOCK_MOVE, two(NO_BONUS, POISONED_BONUS));
        m.insert(RETALIATE_MOVE, two(NO_BONUS, ALLY_FAINTED_BONUS));
        m.insert(ECHOED_VOICE_MOVE, range_strings(1, 6, 1));
        m
    });
    match gen {
        Gen::One => &GEN1,
        Gen::Two => &GEN2,
        Gen::Three => &GEN3,
        Gen::Four => &GEN4,
        Gen::Five => &GEN5,
    }
}

/// Gen 2's `_PRESENT_TYPE_IDS`
pub fn present_type_id(t: &str) -> Option<i64> {
    Some(match t {
        consts::TYPE_NORMAL => 0,
        consts::TYPE_FIGHTING => 1,
        consts::TYPE_FLYING => 2,
        consts::TYPE_POISON => 3,
        consts::TYPE_GROUND => 4,
        consts::TYPE_ROCK => 5,
        consts::TYPE_BUG => 7,
        consts::TYPE_GHOST => 8,
        consts::TYPE_STEEL => 9,
        consts::TYPE_FIRE => 20,
        consts::TYPE_WATER => 21,
        consts::TYPE_GRASS => 22,
        consts::TYPE_ELECTRIC => 23,
        consts::TYPE_PSYCHIC => 24,
        consts::TYPE_ICE => 25,
        consts::TYPE_DRAGON => 26,
        consts::TYPE_DARK => 27,
        _ => return None,
    })
}

/// Gen 1 `StatModifierRatios` (used for accuracy / evasion stages): -6..=+6
pub const GEN1_STAGE_RATIOS: [(i64, i64); 13] = [
    (25, 100), (28, 100), (33, 100), (40, 100), (50, 100), (66, 100), (1, 1),
    (15, 10), (2, 1), (25, 10), (3, 1), (35, 10), (4, 1),
];

/// Gen 2 `AccuracyLevelMultipliers`, gen 3 `sAccuracyStageRatios` and gen 4
/// `HitRateByStage` (identical tables): -6..=+6
pub const ACCURACY_STAGE_RATIOS: [(i64, i64); 13] = [
    (33, 100), (36, 100), (43, 100), (50, 100), (60, 100), (75, 100), (1, 1),
    (133, 100), (166, 100), (2, 1), (233, 100), (133, 50), (3, 1),
];

pub fn bag_limit(gen: Gen) -> Option<usize> {
    match gen {
        Gen::One => Some(20),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Metronome
// ---------------------------------------------------------------------------

const METRONOME_UNCALLABLE_GEN1: &[&str] = &["Metronome", "Struggle"];

/// pokecrystal / pokegold `MetronomeExcepts`
const METRONOME_UNCALLABLE_GEN2: &[&str] = &[
    "Metronome", "Struggle", "Sketch", "Mimic", "Counter", "Mirror Coat", "Protect", "Detect",
    "Endure", "Destiny Bond", "Sleep Talk", "Thief",
];

/// pokeruby / pokeemerald / pokefirered `sMovesForbiddenToCopy`
const METRONOME_UNCALLABLE_GEN3: &[&str] = &[
    "Metronome", "Struggle", "Sketch", "Mimic", "Counter", "Mirror Coat", "Protect", "Detect",
    "Endure", "Destiny Bond", "Sleep Talk", "Thief", "Follow Me", "Snatch", "Helping Hand",
    "Covet", "Trick", "Focus Punch",
];

/// pokeplatinum `sCannotMetronomeMoves`, pokeheartgold
/// `sMetronomeUnuseableMoves` and the same table in pokediamond's battle
/// overlay (`ov11_0225E300`)
const METRONOME_UNCALLABLE_GEN4: &[&str] = &[
    "Metronome", "Struggle", "Sketch", "Mimic", "Chatter", "Sleep Talk", "Assist", "Mirror Move",
    "Counter", "Mirror Coat", "Protect", "Detect", "Endure", "Destiny Bond", "Thief", "Follow Me",
    "Snatch", "Helping Hand", "Covet", "Trick", "Focus Punch", "Feint", "Copycat", "Me First",
    "Switcheroo",
];

/// Black/White have no battle decomp: this is Pokémon Showdown's gen 5 set
/// (the gen 5 moves without its `metronome` flag), whose gen 4 set matches
/// the gen 4 decomp table above exactly.
const METRONOME_UNCALLABLE_GEN5: &[&str] = &[
    "After You", "Assist", "Bestow", "Chatter", "Copycat", "Counter", "Covet", "Destiny Bond",
    "Detect", "Endure", "Feint", "Focus Punch", "Follow Me", "Freeze Shock", "Helping Hand",
    "Ice Burn", "Me First", "Metronome", "Mimic", "Mirror Coat", "Mirror Move", "Nature Power",
    "Protect", "Quash", "Quick Guard", "Rage Powder", "Relic Song", "Secret Sword", "Sketch",
    "Sleep Talk", "Snarl", "Snatch", "Snore", "Struggle", "Switcheroo", "Techno Blast", "Thief",
    "Transform", "Trick", "V-create", "Wide Guard",
];

/// Moves Metronome can never call in `gen` (gen 1: pokered / pokeyellow
/// `MetronomePickMove` rerolls only these two).
pub fn metronome_uncallable_moves(gen: Gen) -> &'static [&'static str] {
    match gen {
        Gen::One => METRONOME_UNCALLABLE_GEN1,
        Gen::Two => METRONOME_UNCALLABLE_GEN2,
        Gen::Three => METRONOME_UNCALLABLE_GEN3,
        Gen::Four => METRONOME_UNCALLABLE_GEN4,
        Gen::Five => METRONOME_UNCALLABLE_GEN5,
    }
}

/// Whether Metronome also rerolls the moves its user knows: gen 2
/// (`CheckUserMove` in `BattleCommand_Metronome`) and gen 4 (the known-move
/// loop of every gen 4 game's metronome command). Gen 3 kept only an empty
/// leftover of the loop, and gen 5 dropped the rule.
pub fn metronome_skips_known_moves(gen: Gen) -> bool {
    matches!(gen, Gen::Two | Gen::Four)
}

/// Moves Metronome cannot call while Gravity is in effect: Platinum's
/// `Move_FailsInHighGravity` / HeartGold's `sGravityUnusableMoves`.
/// Diamond/Pearl's metronome command does not check Gravity.
pub const METRONOME_GRAVITY_BLOCKED: [&str; 6] = ["Fly", "Bounce", "Jump Kick", "Hi Jump Kick", "Splash", "Magnet Rise"];
