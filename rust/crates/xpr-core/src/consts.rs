//! Port of `utils/constants.py`. Every attribute of the Python `Constants`
//! object becomes a `pub const`. Path-dependent values live in [`Paths`].

use std::path::{Path, PathBuf};

pub const APP_VERSION: &str = "v6.0a";
pub const APP_RELEASE_DATE: &str = "2026-Sep-11";
/// The Python release this port reproduces (used by the golden tooling).
pub const REFERENCE_APP_VERSION: &str = "v5.0a";

pub const DEBUG_MODE: bool = false;
pub const DEBUG_RECORDING_MODE: bool = false;
pub const APP_NAME: &str = "pkmn_xp_router";
pub const APP_DATA_FOLDER_DEFAULT_NAME: &str = "pkmn_xp_router_data";
pub const USER_LOCATION_DATA_KEY: &str = "user_data_location";
pub const IMAGE_LOCATION_KEY: &str = "image_export_location";

pub const SAVED_ROUTES_FOLDER_NAME: &str = "saved_routes";
pub const SAVED_IMAGES_FOLDER_NAME: &str = "images";
pub const OUTDATED_ROUTES_FOLDER_NAME: &str = "outdated_routes";
pub const CUSTOM_GENS_FOLDER_NAME: &str = "custom_gens";

pub const CUSTOM_GEN_META_FILE_NAME: &str = "custom_gen.json";
pub const CUSTOM_GEN_NAME_KEY: &str = "custom_gen_name";
pub const BASE_GEN_NAME_KEY: &str = "base_gen_name";

pub const MAJOR_FIGHTS_KEY: &str = "major_fights";
pub const BRANCHED_MANDATORY_FIGHTS_KEY: &str = "branched_mandatory_fights";
pub const BADGE_REWARDS_KEY: &str = "badge_rewards";
pub const FIGHT_REWARDS_KEY: &str = "fight_rewards";
pub const TYPE_CHART_KEY: &str = "type_chart";
pub const SPECIAL_TYPES_KEY: &str = "special_types";
pub const HELD_ITEM_BOOSTS_KEY: &str = "held_item_boosts";
pub const TRAINER_TIMING_INFO_KEY: &str = "trainer_timing_info";
pub const INTRO_TIME_KEY: &str = "intro_time";
pub const OUTRO_TIME_KEY: &str = "outro_time";
pub const KO_TIME_KEY: &str = "ko_time";
pub const SEND_OUT_TIME_KEY: &str = "send_out_time";

pub const ITEM_DB_FILE_NAME: &str = "items.json";
pub const MOVE_DB_FILE_NAME: &str = "moves.json";
pub const POKEMON_DB_FILE_NAME: &str = "pokemon.json";
pub const TRAINERS_DB_FILE_NAME: &str = "trainers.json";
pub const TYPE_INFO_FILE_NAME: &str = "type_info.json";
pub const FIGHTS_INFO_FILE_NAME: &str = "fights_info.json";

pub const SPECIES_KEY: &str = "species";
pub const NAME_KEY: &str = "name";
pub const STATS_KEY: &str = "stats";
pub const BASE_STATS_KEY: &str = "base_stats";
pub const BASE_HP_KEY: &str = "base_hp";
pub const BASE_ATK_KEY: &str = "base_atk";
pub const BASE_DEF_KEY: &str = "base_def";
pub const BASE_SPA_KEY: &str = "base_spc_atk";
pub const BASE_SPD_KEY: &str = "base_spc_def";
pub const OLD_BASE_SPD_KEY: &str = "base_spd";
pub const BASE_SPE_KEY: &str = "base_spe";
pub const BASE_SPC_KEY: &str = "base_spc";
pub const EV_YIELD_KEY: &str = "ev_yield";
pub const EV_YIELD_HP_KEY: &str = "ev_yield_hp";
pub const EV_YIELD_ATK_KEY: &str = "ev_yield_atk";
pub const EV_YIELD_DEF_KEY: &str = "ev_yield_def";
pub const EV_YIELD_SPC_ATK_KEY: &str = "ev_yield_spc_atk";
pub const EV_YIELD_SPC_DEF_KEY: &str = "ev_yield_spc_def";
pub const EV_YIELD_SPD_KEY: &str = "ev_yield_spd";
pub const FIRST_TYPE_KEY: &str = "type_1";
pub const SECOND_TYPE_KEY: &str = "type_2";
pub const CATCH_RATE_KEY: &str = "catch_rate";
pub const BASE_XP_KEY: &str = "base_xp";
pub const BASE_EXPERIENCE_KEY: &str = "base_experience";
pub const INITIAL_MOVESET_KEY: &str = "initial_moveset";
pub const LEARNED_MOVESET_KEY: &str = "levelup_moveset";
pub const LEVEL_UP_MOVESET_KEY: &str = "level_up_learnset";
pub const GROWTH_RATE_KEY: &str = "growth_rate";
pub const TM_HM_LEARNSET_KEY: &str = "tm_hm_learnset";
pub const DVS_KEY: &str = "dv";
pub const IVS_KEY: &str = "iv";
pub const IVS_PLURAL_KEY: &str = "ivs";
pub const HELD_ITEM_KEY: &str = "held_item";
pub const ABILITY_KEY: &str = "ability";
pub const ABILITY_LIST_KEY: &str = "abilities";
pub const WEIGHT_KEY: &str = "weight";
pub const STAT_KEY: &str = "stat";
pub const MODIFIER_KEY: &str = "modifier";
pub const TARGET_KEY: &str = "target";
pub const EFFECT_TARGET_SELF: &str = "self";
pub const EFFECT_TARGET_ENEMY: &str = "enemy";
pub const EFFECT_TARGET_FOE: &str = "foe";
pub const NATURE_KEY: &str = "nature";

pub const REALIZED_STAT_XP_KEY: &str = "realized_stat_xp";
pub const UNREALIZED_STAT_XP_KEY: &str = "unrealized_stat_xp";
pub const SOLO_MON_KEY: &str = "solo_mon";
pub const BADGES_KEY: &str = "badges";
pub const INVENTORY_KEY: &str = "inventory";
pub const XP_TO_NEXT_LEVEL: &str = "xp_to_next_level";
pub const PERCENT_XP_TO_NEXT_LEVEL: &str = "percent_xp_to_next_level";
pub const ITEMS_KEY: &str = "items";
pub const COUNT_KEY: &str = "count";

pub const LEVEL: &str = "level";
pub const HP: &str = "hp";
pub const ATK: &str = "atk";
pub const DEF: &str = "def";
pub const SPA: &str = "spa";
pub const SPD: &str = "spd";
pub const SPE: &str = "spe";
pub const SPC: &str = "spc";
pub const EXPERIENCE_YIELD_KEY: &str = "experience_yield";
pub const XP: &str = "xp";
pub const MOVES: &str = "moves";
pub const EV: &str = "ev";
pub const ACC: &str = "acc";

pub const ATTACK: &str = "attack";
pub const DEFENSE: &str = "defense";
pub const SPEED: &str = "speed";
pub const SPECIAL_ATTACK: &str = "special_attack";
pub const SPECIAL_DEFENSE: &str = "special_defense";

pub const TRAINER_NAME: &str = "trainer_name";
pub const SECOND_TRAINER_NAME: &str = "second_trainer_name";
pub const TRAINER_CLASS: &str = "trainer_class";
pub const ROM_ID: &str = "rom_id";
pub const TRAINER_ID: &str = "trainer_id";
pub const TRAINER_LOC: &str = "trainer_location";
pub const TRAINER_PARTY: &str = "party";
pub const TRAINER_POKEMON: &str = "pokemon";
pub const TRAINER_REFIGHTABLE: &str = "refightable";
pub const TRAINER_DOUBLE_BATTLE: &str = "is_double_battle";
pub const SPECIAL_MOVES: &str = "special_moves";
pub const MONEY: &str = "money";
pub const VERBOSE_KEY: &str = "verbose";
pub const TEST_MOVES_KEY: &str = "test_moves";
pub const SETUP_MOVES_KEY: &str = "setup_moves";
pub const ENEMY_SETUP_MOVES_KEY: &str = "enemy_setup_moves";
pub const PLAYER_FIELD_MOVES_KEY: &str = "player_field_moves";
pub const ENEMY_FIELD_MOVES_KEY: &str = "enemy_field_moves";
pub const MIMIC_SELECTION: &str = "mimic_selection";
pub const CUSTOM_MOVE_DATA: &str = "custom_move_data";
pub const STAT_STAGE_SETUP_KEY: &str = "stat_stage_setup";
pub const EXP_SPLIT: &str = "exp_split";
pub const WEATHER: &str = "weather";
pub const WEATHER_SOURCE_MON_IDX: &str = "weather_source_mon_idx";
pub const PLAYER_SCREENS_KEY: &str = "player_screens";
pub const ENEMY_SCREENS_KEY: &str = "enemy_screens";
pub const PLAYER_INTIMIDATE_KEY: &str = "player_intimidate";
pub const ENEMY_INTIMIDATE_KEY: &str = "enemy_intimidate";
pub const INTIMIDATE_ABILITY: &str = "Intimidate";
pub const CLEAR_BODY_ABILITY: &str = "Clear Body";
pub const HYPER_CUTTER_ABILITY: &str = "Hyper Cutter";
pub const INTIMIDATE_BLOCKING_ABILITIES: [&str; 2] = [CLEAR_BODY_ABILITY, HYPER_CUTTER_ABILITY];
pub const FORECAST_ABILITY: &str = "Forecast";
pub const PAY_DAY_AMOUNT: &str = "pay_day_amount";
pub const MON_ORDER: &str = "mon_order";
pub const TRANSFORMED: &str = "transformed";
pub const COLLAPSED_MONS_KEY: &str = "collapsed_mons";
/// Definition indices of the enemy mons whose held item the solo mon steals
/// (Thief / Covet) during a trainer fight.
pub const THIEF_MONS_KEY: &str = "thief_mons";
pub const PLAYER_KEY: &str = "player";
pub const ENEMY_KEY: &str = "enemy";
pub const EVOLVED_SPECIES: &str = "evolved_species";
pub const BY_STONE_KEY: &str = "by_stone";

pub const MOVE_TYPE: &str = "type";
pub const POWER: &str = "power";
pub const BASE_POWER: &str = "base_power";
pub const MOVE_PP: &str = "pp";
pub const MOVE_ACCURACY: &str = "accuracy";
pub const MOVE_EFFECTS: &str = "effects";
pub const MOVE_EFFECT: &str = "effect";
pub const MOVE_FLAVOR: &str = "attack_flavor";
pub const MOVE_TARGET: &str = "target";
pub const MOVE_CATEGORY: &str = "category";
pub const CATEGORY_PHYSICAL: &str = "Physical";
pub const CATEGORY_SPECIAL: &str = "Special";
pub const MOVE_HAS_FIELD_EFFECT: &str = "has_field_effect";

pub const GROWTH_RATE_FAST: &str = "growth_fast";
pub const GROWTH_RATE_MEDIUM_FAST: &str = "growth_medium_fast";
pub const GROWTH_RATE_MEDIUM_SLOW: &str = "growth_medium_slow";
pub const GROWTH_RATE_SLOW: &str = "growth_slow";
pub const GROWTH_RATE_ERRATIC: &str = "growth_erratic";
pub const GROWTH_RATE_FLUCTUATING: &str = "growth_fluctuating";

pub const HP_UP: &str = "HP Up";
pub const CARBOS: &str = "Carbos";
pub const IRON: &str = "Iron";
pub const CALCIUM: &str = "Calcium";
pub const ZINC: &str = "Zinc";
pub const PROTEIN: &str = "Protein";
pub const RARE_CANDY: &str = "Rare Candy";

pub const POMEG_BERRY: &str = "Pomeg Berry";
pub const KELPSY_BERRY: &str = "Kelpsy Berry";
pub const QUALOT_BERRY: &str = "Qualot Berry";
pub const HONDEW_BERRY: &str = "Hondew Berry";
pub const GREPA_BERRY: &str = "Grepa Berry";
pub const TAMATO_BERRY: &str = "Tamato Berry";

pub const HIGHLIGHT_NONE: &str = "Don't Highlight";
pub const HIGHLIGHT_GUARANTEED_KILL: &str = "Guaranteed Kill";
pub const HIGHLIGHT_CONSISTENT_KILL: &str = "Consistent Kill";
pub const HIGHLIGHT_FASTEST_KILL: &str = "Fastest Kill";
pub const ALL_HIGHLIGHT_STRATS: [&str; 4] = [
    HIGHLIGHT_GUARANTEED_KILL,
    HIGHLIGHT_CONSISTENT_KILL,
    HIGHLIGHT_FASTEST_KILL,
    HIGHLIGHT_NONE,
];

pub const DEFAULT_FOLDER_NAME: &str = "Main";
pub const EVENT_FOLDER_NAME: &str = "Event Folder Name";
pub const INVENTORY_EVENT_DEFINITON: &str = "Inventory Event";

pub const TASK_TRAINER_BATTLE: &str = "Fight Trainer";
pub const TASK_RARE_CANDY: &str = "Use Rare Candy";
pub const TASK_VITAMIN: &str = "Use Vitamin";
pub const TASK_FIGHT_WILD_PKMN: &str = "Fight Wild Pkmn";
pub const TASK_GET_FREE_ITEM: &str = "Acquire Item";
pub const TASK_PURCHASE_ITEM: &str = "Purchase Item";
pub const TASK_USE_ITEM: &str = "Use/Drop Item";
pub const TASK_SELL_ITEM: &str = "Sell Item";
pub const TASK_HOLD_ITEM: &str = "Hold Item";
pub const TASK_LEARN_MOVE_LEVELUP: &str = "Learn Levelup Move";
pub const TASK_LEARN_MOVE_TM: &str = "Learn TM/HM Move";
pub const TASK_SAVE: &str = "Game Save";
pub const TASK_HEAL: &str = "PkmnCenter Heal";
pub const TASK_BLACKOUT: &str = "Blackout";
pub const TASK_EVOLUTION: &str = "Evolution";
/// Gen 1 bag reordering (the in-game SELECT swap). Event type and route-file key.
pub const TASK_REORDER_BAG: &str = "Reorder Bag";
pub const TASK_NOTES_ONLY: &str = "Just Notes";
pub const ERROR_SEARCH: &str = "Invalid Events";
pub const MAJOR_BATTLE_FILTER: &str = "Major Battles";

pub const ITEM_ROUTE_EVENT_TYPES: [&str; 5] = [
    TASK_GET_FREE_ITEM,
    TASK_PURCHASE_ITEM,
    TASK_USE_ITEM,
    TASK_SELL_ITEM,
    TASK_HOLD_ITEM,
];

pub const ROUTE_EVENT_TYPES: [&str; 18] = [
    TASK_TRAINER_BATTLE,
    TASK_LEARN_MOVE_LEVELUP,
    TASK_SELL_ITEM,
    TASK_NOTES_ONLY,
    TASK_HOLD_ITEM,
    TASK_RARE_CANDY,
    TASK_FIGHT_WILD_PKMN,
    TASK_GET_FREE_ITEM,
    TASK_PURCHASE_ITEM,
    TASK_USE_ITEM,
    TASK_REORDER_BAG,
    TASK_VITAMIN,
    TASK_SAVE,
    TASK_HEAL,
    TASK_BLACKOUT,
    TASK_EVOLUTION,
    TASK_LEARN_MOVE_TM,
    ERROR_SEARCH,
];

pub const ITEM_TYPE_ALL_ITEMS: &str = "All Items";
pub const ITEM_TYPE_BACKPACK_ITEMS: &str = "Items in Backpack";
pub const ITEM_TYPE_KEY_ITEMS: &str = "Key Items";
pub const ITEM_TYPE_TM: &str = "TMs";
pub const ITEM_TYPE_OTHER: &str = "Other";
pub const ITEM_TYPES: [&str; 5] = [
    ITEM_TYPE_ALL_ITEMS,
    ITEM_TYPE_BACKPACK_ITEMS,
    ITEM_TYPE_KEY_ITEMS,
    ITEM_TYPE_TM,
    ITEM_TYPE_OTHER,
];

pub const ALL_TRAINERS: &str = "ALL";
pub const NO_TRAINERS: &str = "No Valid Trainers";
pub const NO_POKEMON: &str = "No Valid Pokemon";
pub const NO_ITEM: &str = "No Valid Items";
pub const NO_MOVE: &str = "No Valid Moves";
pub const UNUSED_TRAINER_LOC: &str = "Unused";
pub const EVENTS: &str = "events";
pub const ENABLED_KEY: &str = "Enabled";
pub const EXPANDED_KEY: &str = "Expanded";
pub const TAGS_KEY: &str = "Tags";
pub const RECORDED_TIME_KEY: &str = "Recorded Time";
pub const SPLIT_TIME_KEY: &str = "Split Time";

pub const SUPER_SHUCKIE_URL: &str = "http://127.0.0.1:30158";

pub const HIGHLIGHT_LABEL: &str = "highlight";
pub const HIGHLIGHT_LABEL_1: &str = "highlight1";
pub const HIGHLIGHT_LABEL_2: &str = "highlight2";
pub const HIGHLIGHT_LABEL_3: &str = "highlight3";
pub const HIGHLIGHT_LABEL_4: &str = "highlight4";
pub const HIGHLIGHT_LABEL_5: &str = "highlight5";
pub const HIGHLIGHT_LABEL_6: &str = "highlight6";
pub const HIGHLIGHT_LABEL_7: &str = "highlight7";
pub const HIGHLIGHT_LABEL_8: &str = "highlight8";
pub const HIGHLIGHT_LABEL_9: &str = "highlight9";
pub const ALL_HIGHLIGHT_LABELS: [&str; 9] = [
    HIGHLIGHT_LABEL_1,
    HIGHLIGHT_LABEL_2,
    HIGHLIGHT_LABEL_3,
    HIGHLIGHT_LABEL_4,
    HIGHLIGHT_LABEL_5,
    HIGHLIGHT_LABEL_6,
    HIGHLIGHT_LABEL_7,
    HIGHLIGHT_LABEL_8,
    HIGHLIGHT_LABEL_9,
];

pub const IS_KEY_ITEM: &str = "key_item";
pub const PURCHASE_PRICE: &str = "purchase_price";
pub const CUSTOM_PRICE_KEY: &str = "custom_price";
pub const MARTS: &str = "marts";

pub const EVENT_TAG_IMPORTANT: &str = "important";
pub const EVENT_TAG_ERRORS: &str = "errors";
/// An event that applied, but not exactly as written (e.g. a bag swap whose
/// items were found at other slots). Never makes the run invalid.
pub const EVENT_TAG_WARNINGS: &str = "warnings";
pub const EVENT_TAG_BRANCHED_MANDATORY: &str = "branched_mandatory";
pub const EVENT_TAG_FOLDER: &str = "folder";

pub const FIGHT_CATEGORY_RIVAL: &str = "rival";
pub const FIGHT_CATEGORY_GYM_LEADER: &str = "gym_leader";
pub const FIGHT_CATEGORY_ELITE_FOUR: &str = "elite_four";
pub const FIGHT_CATEGORY_CHAMPION: &str = "champion";
pub const FIGHT_CATEGORY_POST_GAME: &str = "post_game";
pub const FIGHT_CATEGORY_BOSS: &str = "boss";
pub const FIGHT_CATEGORY_TEAM_LEADER: &str = "team_leader";

pub const EVENT_TAG_RIVAL: &str = "fight_rival";
pub const EVENT_TAG_GYM_LEADER: &str = "fight_gym_leader";
pub const EVENT_TAG_ELITE_FOUR: &str = "fight_elite_four";
pub const EVENT_TAG_CHAMPION: &str = "fight_champion";
pub const EVENT_TAG_POST_GAME: &str = "fight_post_game";
pub const EVENT_TAG_BOSS: &str = "fight_boss";
pub const EVENT_TAG_TEAM_LEADER: &str = "fight_team_leader";

pub const FIGHT_CATEGORY_TO_TAG: [(&str, &str); 7] = [
    (FIGHT_CATEGORY_RIVAL, EVENT_TAG_RIVAL),
    (FIGHT_CATEGORY_GYM_LEADER, EVENT_TAG_GYM_LEADER),
    (FIGHT_CATEGORY_ELITE_FOUR, EVENT_TAG_ELITE_FOUR),
    (FIGHT_CATEGORY_CHAMPION, EVENT_TAG_CHAMPION),
    (FIGHT_CATEGORY_POST_GAME, EVENT_TAG_POST_GAME),
    (FIGHT_CATEGORY_BOSS, EVENT_TAG_BOSS),
    (FIGHT_CATEGORY_TEAM_LEADER, EVENT_TAG_TEAM_LEADER),
];

pub fn fight_category_to_tag(category: &str) -> Option<&'static str> {
    FIGHT_CATEGORY_TO_TAG
        .iter()
        .find(|(c, _)| *c == category)
        .map(|(_, t)| *t)
}

pub const ALL_FIGHT_CATEGORY_TAGS: [&str; 7] = [
    EVENT_TAG_RIVAL,
    EVENT_TAG_GYM_LEADER,
    EVENT_TAG_ELITE_FOUR,
    EVENT_TAG_CHAMPION,
    EVENT_TAG_POST_GAME,
    EVENT_TAG_BOSS,
    EVENT_TAG_TEAM_LEADER,
];

pub const MOVE_KEY: &str = "move";
pub const MOVE_DEST_KEY: &str = "destination_slot";
pub const MOVE_SOURCE_KEY: &str = "source";
pub const MOVE_LEVEL_KEY: &str = "level_learned";
pub const MOVE_MON_KEY: &str = "species_when_learned";
pub const MOVE_FORCE_DEST_KEY: &str = "force_destination";

pub const LEARN_MOVE_KEY: &str = "LearnMove";
pub const MOVE_SLOT_TEMPLATE: &str = "Move #{} (Over {})";
pub const MOVE_DONT_LEARN: &str = "Don't Learn";
pub const MOVE_SOURCE_LEVELUP: &str = "LevelUp";
pub const MOVE_SOURCE_TUTOR: &str = "Tutor/Deleter";
pub const MOVE_SOURCE_TM_HM: &str = "TM/HM";
pub const LEVEL_ANY: &str = "AnyLevel";
pub const DELETE_MOVE: &str = "Delete Move";

pub const SUCCESS_COLOR_KEY: &str = "success_color";
pub const WARNING_COLOR_KEY: &str = "warning_color";
pub const FAILURE_COLOR_KEY: &str = "failure_color";
pub const DIVIDER_COLOR_KEY: &str = "divider_color";
pub const HEADER_COLOR_KEY: &str = "header_color";
pub const PRIMARY_COLOR_KEY: &str = "primary_color";
pub const SECONDARY_COLOR_KEY: &str = "secondary_color";
pub const CONTRAST_COLOR_KEY: &str = "contrast_color";
pub const BACKGROUND_COLOR_KEY: &str = "background_color";
pub const TEXT_COLOR_KEY: &str = "text_color";
pub const PLAYER_HIGHLIGHT_STRATEGY_KEY: &str = "player_highlight_strategy";
pub const ENEMY_HIGHLIGHT_STRATEGY_KEY: &str = "enemy_highlight_strategy";
pub const CONSISTENT_HIGHLIGHT_THRESHOLD: &str = "consistent_highlight_threshold";
pub const IGNORE_ACCURACY_IN_DAMAGE_CALCS: &str = "ignore_accuracy_in_damage_calcs";
pub const DAMAGE_SEARCH_DEPTH: &str = "damage_search_depth";
pub const FORCE_FULL_SEARCH: &str = "force_full_search";

pub const CUSTOM_FONT_NAME_KEY: &str = "custom_font_name";
pub const DEBUG_MODE_KEY: &str = "debug_mode";
pub const AUTO_SWITCH_KEY: &str = "auto_switch";
pub const NOTES_VISIBILITY_KEY: &str = "notes_visibility";

pub const IMPORTANT_COLOR: &str = "#b3b6b7";
pub const USER_FLAGGED_COLOR: &str = "#ff8888";
pub const VALID_COLOR: &str = "#abebc6";
pub const ERROR_COLOR: &str = "#f9e79f";

pub const CONFIG_ROUTE_ONE_PATH: &str = "route_one_path";
pub const CONFIG_WINDOW_GEOMETRY: &str = "tkinter_window_geometry";

pub const STATE_SUMMARY_LABEL: &str = "State Summary";
pub const BADGE_BOOST_LABEL: &str = "Badge Boost Calculator";

pub const ROOT_FOLDER_NAME: &str = "ROOT";

pub const EMPTY_ROUTE_NAME: &str = "Empty Route";
pub const PRESET_ROUTE_PREFIX: &str = "PRESET: ";

pub const PKMN_VERSION_KEY: &str = "Version";

pub const RED_VERSION: &str = "Red";
pub const BLUE_VERSION: &str = "Blue";
pub const YELLOW_VERSION: &str = "Yellow";
pub const GOLD_VERSION: &str = "Gold";
pub const SILVER_VERSION: &str = "Silver";
pub const CRYSTAL_VERSION: &str = "Crystal";
pub const RUBY_VERSION: &str = "Ruby";
pub const SAPPHIRE_VERSION: &str = "Sapphire";
pub const EMERALD_VERSION: &str = "Emerald";
pub const FIRE_RED_VERSION: &str = "FireRed";
pub const LEAF_GREEN_VERSION: &str = "LeafGreen";
pub const DIAMOND_VERSION: &str = "Diamond";
pub const PEARL_VERSION: &str = "Pearl";
pub const PLATINUM_VERSION: &str = "Platinum";
pub const HEART_GOLD_VERSION: &str = "HeartGold";
pub const SOUL_SILVER_VERSION: &str = "SoulSilver";
pub const BLACK_VERSION: &str = "Black";
pub const WHITE_VERSION: &str = "White";
pub const BLACK_2_VERSION: &str = "Black 2";
pub const WHITE_2_VERSION: &str = "White 2";

pub const VERSION_LIST: [&str; 20] = [
    YELLOW_VERSION,
    RED_VERSION,
    BLUE_VERSION,
    GOLD_VERSION,
    SILVER_VERSION,
    CRYSTAL_VERSION,
    RUBY_VERSION,
    SAPPHIRE_VERSION,
    EMERALD_VERSION,
    FIRE_RED_VERSION,
    LEAF_GREEN_VERSION,
    DIAMOND_VERSION,
    PEARL_VERSION,
    PLATINUM_VERSION,
    HEART_GOLD_VERSION,
    SOUL_SILVER_VERSION,
    BLACK_VERSION,
    WHITE_VERSION,
    BLACK_2_VERSION,
    WHITE_2_VERSION,
];

pub const FRLG_VERSIONS: [&str; 2] = [FIRE_RED_VERSION, LEAF_GREEN_VERSION];

pub const VERSION_COLORS: [(&str, &str); 20] = [
    (RED_VERSION, "#b34444"),
    (BLUE_VERSION, "#4444a8"),
    (YELLOW_VERSION, "#b8a040"),
    (GOLD_VERSION, "#a8842a"),
    (SILVER_VERSION, "#909898"),
    (CRYSTAL_VERSION, "#50a8b0"),
    (RUBY_VERSION, "#904040"),
    (SAPPHIRE_VERSION, "#404890"),
    (EMERALD_VERSION, "#408040"),
    (FIRE_RED_VERSION, "#b06838"),
    (LEAF_GREEN_VERSION, "#48904a"),
    (DIAMOND_VERSION, "#7090a8"),
    (PEARL_VERSION, "#a08898"),
    (PLATINUM_VERSION, "#808078"),
    (HEART_GOLD_VERSION, "#a89030"),
    (SOUL_SILVER_VERSION, "#8898a0"),
    (BLACK_VERSION, "#444444"),
    (WHITE_VERSION, "#a0a0a0"),
    (BLACK_2_VERSION, "#3a4a5a"),
    (WHITE_2_VERSION, "#90a0b0"),
];

pub fn version_color(version: &str) -> Option<&'static str> {
    VERSION_COLORS.iter().find(|(v, _)| *v == version).map(|(_, c)| *c)
}

pub const NO_SAVED_ROUTES: &str = "No Saved Routes";
pub const NO_FOLDERS: &str = "No Matching Folders";

pub const TRANSFER_EXISTING_FOLDER: &str = "Existing Folder";
pub const TRANSFER_NEW_FOLDER: &str = "New Folder";

pub const MULTI_HIT_2: &str = "2 Hits";
pub const MULTI_HIT_3: &str = "3 Hits";
pub const MULTI_HIT_4: &str = "4 Hits";
pub const MULTI_HIT_5: &str = "5 Hits";
pub const MULTI_HIT_CUSTOM_DATA: [&str; 4] = [MULTI_HIT_2, MULTI_HIT_3, MULTI_HIT_4, MULTI_HIT_5];

pub const DOUBLE_HIT_FLAVOR: &str = "two_hit";
pub const FLAVOR_MULTI_HIT: &str = "multi_hit";
pub const FLAVOR_BELLY_DRUM: &str = "belly_drum";
pub const FLAVOR_HIGH_CRIT: &str = "high_crit";
pub const FLAVOR_FIXED_DAMAGE: &str = "fixed_damage";
pub const FLAVOR_LEVEL_DAMAGE: &str = "level_damage";
pub const FLAVOR_PSYWAVE: &str = "psywave";
pub const FLAVOR_RECHARGE: &str = "recharge";
pub const FLAVOR_TWO_TURN_INVULN: &str = "two_turn_semi_invlunerable";
pub const FLAVOR_TWO_TURN: &str = "two_turn";

pub const FLAVOR_RECOIL_QUARTER: &str = "quarter_recoil";
pub const FLAVOR_RECOIL_THIRD: &str = "third_recoil";
pub const FLAVOR_RECOIL_HALF: &str = "half_recoil";
pub const FLAVOR_RECOIL_QUARTER_MAX_HP: &str = "quarter_max_hp_recoil";
pub const RECOIL_FLAVOR_DIVISORS: [(&str, i64); 3] = [
    (FLAVOR_RECOIL_QUARTER, 4),
    (FLAVOR_RECOIL_THIRD, 3),
    (FLAVOR_RECOIL_HALF, 2),
];
pub const RECOIL_MAX_HP_FLAVOR_DIVISORS: [(&str, i64); 1] = [(FLAVOR_RECOIL_QUARTER_MAX_HP, 4)];
pub const ROCK_HEAD_ABILITY: &str = "Rock Head";

pub const STRUGGLE_MOVE_NAME: &str = "Struggle";
pub const MIMIC_MOVE_NAME: &str = "Mimic";
pub const EXPLOSION_MOVE_NAME: &str = "Explosion";
pub const SELFDESTRUCT_MOVE_NAME: &str = "Selfdestruct";
pub const DRAGON_RAGE_MOVE_NAME: &str = "Dragon Rage";
pub const FLAIL_MOVE_NAME: &str = "Flail";
pub const REVERSAL_MOVE_NAME: &str = "Reversal";
pub const FUTURE_SIGHT_MOVE_NAME: &str = "Future Sight";
pub const DOOM_DESIRE_MOVE_NAME: &str = "Doom Desire";
pub const SPIT_UP_MOVE_NAME: &str = "Spit Up";
pub const HIDDEN_POWER_MOVE_NAME: &str = "Hidden Power";
pub const NATURAL_GIFT_MOVE_NAME: &str = "Natural Gift";
pub const WEATHER_BALL_MOVE_NAME: &str = "Weather Ball";
pub const SOLAR_BEAM_MOVE_NAME: &str = "SolarBeam";
pub const LIGHTSCREEN_SANITIZED_MOVE_NAME: &str = "lightscreen";
pub const REFLECT_SANITIZED_MOVE_NAME: &str = "reflect";
pub const GRAVITY_SANITIZED_MOVE_NAME: &str = "gravity";
pub const MAGNET_RISE_SANITIZED_MOVE_NAME: &str = "magnetrise";
pub const MIRACLE_EYE_SANITIZED_MOVE_NAME: &str = "miracleeye";
pub const POWER_TRICK_SANITIZED_MOVE_NAME: &str = "powertrick";
pub const ROOST_SANITIZED_MOVE_NAME: &str = "roost";
pub const TAILWIND_SANITIZED_MOVE_NAME: &str = "tailwind";
pub const TRICK_ROOM_SANITIZED_MOVE_NAME: &str = "trickroom";
pub const WORRY_SEED_SANITIZED_MOVE_NAME: &str = "worryseed";
pub const GASTRO_ACID_SANITIZED_MOVE_NAME: &str = "gastroacid";
pub const SLOW_START_SANITIZED_NAME: &str = "slowstart";

pub const AMULET_COIN_ITEM_NAME: &str = "Amulet Coin";
pub const MACHO_BRACE_ITEM_NAME: &str = "Macho Brace";
pub const POWER_ANKLET_ITEM_NAME: &str = "Power Anklet";
pub const POWER_BAND_ITEM_NAME: &str = "Power Band";
pub const POWER_BELT_ITEM_NAME: &str = "Power Belt";
pub const POWER_BRACER_ITEM_NAME: &str = "Power Bracer";
pub const POWER_LENS_ITEM_NAME: &str = "Power Lens";
pub const POWER_WEIGHT_ITEM_NAME: &str = "Power Weight";
pub const CHOICE_SCARF_ITEM_NAME: &str = "Choice Scarf";
pub const SPEED_SLOWING_ITEMS: [&str; 7] = [
    MACHO_BRACE_ITEM_NAME,
    POWER_ANKLET_ITEM_NAME,
    POWER_BAND_ITEM_NAME,
    POWER_BELT_ITEM_NAME,
    POWER_BRACER_ITEM_NAME,
    POWER_LENS_ITEM_NAME,
    POWER_WEIGHT_ITEM_NAME,
];

pub const TARGETING_BOTH_ENEMIES: &str = "target_both_enemies";

pub const TYPE_TYPELESS: &str = "none";
pub const TYPE_NORMAL: &str = "Normal";
pub const TYPE_FIGHTING: &str = "Fighting";
pub const TYPE_FLYING: &str = "Flying";
pub const TYPE_POISON: &str = "Poison";
pub const TYPE_GROUND: &str = "Ground";
pub const TYPE_ROCK: &str = "Rock";
pub const TYPE_BUG: &str = "Bug";
pub const TYPE_GHOST: &str = "Ghost";
pub const TYPE_FIRE: &str = "Fire";
pub const TYPE_WATER: &str = "Water";
pub const TYPE_GRASS: &str = "Grass";
pub const TYPE_ELECTRIC: &str = "Electric";
pub const TYPE_PSYCHIC: &str = "Psychic";
pub const TYPE_ICE: &str = "Ice";
pub const TYPE_DRAGON: &str = "Dragon";
pub const TYPE_STEEL: &str = "Steel";
pub const TYPE_DARK: &str = "Dark";

pub const SUPER_EFFECTIVE: &str = "Super Effective";
pub const NOT_VERY_EFFECTIVE: &str = "Not Very Effective";
pub const IMMUNE: &str = "Immune";

pub const WEATHER_NONE: &str = "None";
pub const WEATHER_RAIN: &str = "Rain";
pub const WEATHER_SUN: &str = "Harsh Sunlight";
pub const WEATHER_SANDSTORM: &str = "Sandstorm";
pub const WEATHER_HAIL: &str = "Hail";
pub const WEATHER_FOG: &str = "Fog";

pub const WEATHER_MOVE_MAP: [(&str, &str); 4] = [
    ("Rain Dance", WEATHER_RAIN),
    ("Sunny Day", WEATHER_SUN),
    ("Sandstorm", WEATHER_SANDSTORM),
    ("Hail", WEATHER_HAIL),
];

pub fn weather_for_move(move_name: &str) -> Option<&'static str> {
    WEATHER_MOVE_MAP.iter().find(|(m, _)| *m == move_name).map(|(_, w)| *w)
}

pub const FORECAST_TYPE_MAP: [(&str, &str); 3] = [
    (WEATHER_SUN, TYPE_FIRE),
    (WEATHER_RAIN, TYPE_WATER),
    (WEATHER_HAIL, TYPE_ICE),
];

pub fn forecast_type(weather: &str) -> Option<&'static str> {
    FORECAST_TYPE_MAP.iter().find(|(w, _)| *w == weather).map(|(_, t)| *t)
}

pub const WEATHER_BALL_TYPE_MAP: [(&str, &str); 4] = [
    (WEATHER_SUN, TYPE_FIRE),
    (WEATHER_RAIN, TYPE_WATER),
    (WEATHER_HAIL, TYPE_ICE),
    (WEATHER_SANDSTORM, TYPE_ROCK),
];

pub fn weather_ball_type_for(weather: &str) -> Option<&'static str> {
    WEATHER_BALL_TYPE_MAP.iter().find(|(w, _)| *w == weather).map(|(_, t)| *t)
}

pub const WEATHER_SUPPRESSING_ABILITIES: [&str; 2] = ["Air Lock", "Cloud Nine"];

pub const SCREEN_REFLECT: &str = "reflect";
pub const SCREEN_LIGHT_SCREEN: &str = "light_screen";
pub const SCREEN_MAGNET_RISE: &str = "magnet_rise";
pub const SCREEN_MOVE_MAP: [(&str, &str); 3] = [
    ("Reflect", SCREEN_REFLECT),
    ("Light Screen", SCREEN_LIGHT_SCREEN),
    ("Magnet Rise", SCREEN_MAGNET_RISE),
];

pub fn screen_for_move(move_name: &str) -> Option<&'static str> {
    SCREEN_MOVE_MAP.iter().find(|(m, _)| *m == move_name).map(|(_, s)| *s)
}

pub const DEFAULT_INTRO_TIME: f64 = 4.69;
pub const DEFAULT_OUTRO_TIME: f64 = 1.195;
pub const DEFAULT_KO_TIME: f64 = 1.84;
pub const DEFAULT_SEND_OUT_TIME: f64 = 0.75;

pub const GAME_SAVED_FRAGMENT: &str = "Game Saved: ";
pub const RECORDING_ERROR_FRAGMENT: &str = "ERROR RECORDING! ";
pub const BACKPORT_SPECIES_CHECK: &str = "backport";
// trainer_class of the champion in the trainer DBs; recorders use it to know a
// win is followed by the Hall of Fame autosave + credits reboot
pub const CHAMPION_TRAINER_CLASS: &str = "Champion";
pub const POST_CHAMPION_AUTOSAVE_LOCATION: &str = "Post-Champion Autosave";

pub const RECORDING_STATUS_DISCONNECTED: &str = "Disconnected";
pub const RECORDING_STATUS_CONNECTED: &str = "Connected";
pub const RECORDING_STATUS_READY: &str = "Ready";
pub const RECORDING_STATUS_NO_MAPPER: &str =
    "Failed to load mapper. Have you loaded it in GameHook?";
pub const RECORDING_STATUS_WRONG_MAPPER: &str = "Incorrect Mapper Loaded";
pub const RECORDING_STATUS_FAILED_CONNECTION: &str =
    "Connection Failed. This usually means GameHook isn't running";
pub const RECORDING_STATUS_GAMEHOOK_FAILED: &str =
    "Reading GameHook data failed. This version of GameHook may be incompatible with the router";

/// Filesystem locations. `source_root` is the checkout root (the parent of
/// `rust/`) when running from source; a packaged binary embeds the data and
/// only needs the user data locations.
#[derive(Clone, Debug)]
pub struct Paths {
    pub source_root: PathBuf,
    pub global_config_dir: PathBuf,
    pub global_config_file: PathBuf,
    pub pokemon_raw_data: PathBuf,
    pub assets_path: PathBuf,
    pub saved_routes_dir: PathBuf,
    pub outdated_routes_dir: PathBuf,
    pub custom_gens_dir: PathBuf,
}

impl Paths {
    /// `appdirs.user_data_dir(appname="pkmn_xp_router", appauthor="pkmn_xp_router")`
    /// per platform. Hard-coded rather than taken from a crate so the file
    /// lands exactly where the Python app expects it.
    pub fn global_config_dir() -> PathBuf {
        // Test hook: point the whole app at a scratch config/log folder.
        if let Some(dir) = std::env::var_os("XPR_GLOBAL_CONFIG_DIR") {
            return PathBuf::from(dir);
        }
        #[cfg(target_os = "windows")]
        {
            let base = std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|| home_dir().join("AppData").join("Local"));
            base.join(APP_NAME).join(APP_NAME)
        }
        #[cfg(target_os = "macos")]
        {
            home_dir()
                .join("Library")
                .join("Application Support")
                .join(APP_NAME)
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            let base = std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home_dir().join(".local").join("share"));
            base.join(APP_NAME)
        }
    }

    pub fn new(source_root: PathBuf) -> Paths {
        let global_config_dir = Paths::global_config_dir();
        let global_config_file = global_config_dir.join("config.json");
        Paths {
            pokemon_raw_data: source_root.join("raw_pkmn_data"),
            assets_path: source_root.join("assets"),
            source_root,
            global_config_dir,
            global_config_file,
            saved_routes_dir: PathBuf::new(),
            outdated_routes_dir: PathBuf::new(),
            custom_gens_dir: PathBuf::new(),
        }
    }

    /// `Constants.config_user_data_dir`
    pub fn config_user_data_dir(&mut self, user_data_dir: &Path) {
        self.saved_routes_dir = user_data_dir.join(SAVED_ROUTES_FOLDER_NAME);
        self.outdated_routes_dir = user_data_dir.join(OUTDATED_ROUTES_FOLDER_NAME);
        self.custom_gens_dir = user_data_dir.join(CUSTOM_GENS_FOLDER_NAME);
    }

    pub fn all_user_data_paths(&self) -> [&Path; 3] {
        [
            &self.saved_routes_dir,
            &self.outdated_routes_dir,
            &self.custom_gens_dir,
        ]
    }

    /// `Constants.get_potential_user_data_dirs`
    pub fn potential_user_data_dirs(&self, potential: &Path) -> [(PathBuf, PathBuf); 3] {
        [
            (
                self.saved_routes_dir.clone(),
                potential.join(SAVED_ROUTES_FOLDER_NAME),
            ),
            (
                self.outdated_routes_dir.clone(),
                potential.join(OUTDATED_ROUTES_FOLDER_NAME),
            ),
            (
                self.custom_gens_dir.clone(),
                potential.join(CUSTOM_GENS_FOLDER_NAME),
            ),
        ]
    }
}

/// `os.path.expanduser("~")`
pub fn home_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(p) = std::env::var_os("USERPROFILE") {
            return PathBuf::from(p);
        }
        if let (Some(d), Some(p)) = (std::env::var_os("HOMEDRIVE"), std::env::var_os("HOMEPATH")) {
            let mut r = PathBuf::from(d);
            r.push(p);
            return r;
        }
    }
    if let Some(p) = std::env::var_os("HOME") {
        return PathBuf::from(p);
    }
    PathBuf::from(".")
}

/// Locate the repository root (the directory that holds `raw_pkmn_data/`)
/// by walking up from the executable and from the current directory.
pub fn find_source_root() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        candidates.push(exe);
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd);
    }
    for start in candidates {
        let mut cur: Option<&Path> = Some(start.as_path());
        while let Some(dir) = cur {
            if dir.join("raw_pkmn_data").is_dir() && dir.join("assets").is_dir() {
                return Some(dir.to_path_buf());
            }
            cur = dir.parent();
        }
    }
    None
}
