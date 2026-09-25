//! Port of `utils/config_manager.py`: the same file, keys, defaults and
//! validation rules. Writes are atomic (temp file + rename).

use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde_json::Value;

use crate::consts;
use crate::io_utils;
use crate::pyjson;

/// Default keyboard shortcuts (action id -> Qt key-sequence string).
pub const DEFAULT_SHORTCUTS: &[(&str, &str)] = &[
    ("customize_dvs", "Ctrl+X"),
    ("new_route", "Ctrl+N"),
    ("load_route", "Ctrl+L"),
    ("compare_routes", "Ctrl+Shift+C"),
    ("save_route", "Ctrl+S"),
    ("close_route", "Alt+W"),
    ("auto_load_recent", "F2"),
    ("export_notes", "Ctrl+Shift+W"),
    ("screenshot_events", "F5"),
    ("screenshot_battle", "F6"),
    ("screenshot_player", "F7"),
    ("screenshot_enemy", "F8"),
    ("open_image_folder", "F12"),
    ("config_font", "Ctrl+Shift+D"),
    ("custom_gens", "Ctrl+Shift+E"),
    ("app_config", "Ctrl+Shift+A"),
    ("open_data_folder", "Ctrl+Shift+O"),
    ("keyboard_shortcuts", "Ctrl+Shift+K"),
    ("undo", "Ctrl+Z"),
    ("move_event_up", "Ctrl+E"),
    ("move_event_down", "Ctrl+D"),
    ("move_event_up_folder", "Ctrl+Shift+E"),
    ("move_event_down_folder", "Ctrl+Shift+D"),
    ("enable_disable", "Ctrl+C"),
    ("toggle_highlight", "Ctrl+V"),
    ("delete_event", "Ctrl+B"),
    ("highlight_1", "Shift+1"),
    ("highlight_2", "Shift+2"),
    ("highlight_3", "Shift+3"),
    ("highlight_4", "Shift+4"),
    ("highlight_5", "Shift+5"),
    ("highlight_6", "Shift+6"),
    ("highlight_7", "Shift+7"),
    ("highlight_8", "Shift+8"),
    ("highlight_9", "Shift+9"),
    ("new_folder", "Ctrl+Shift+Alt+F"),
    ("rename_folder", "Ctrl+Shift+F"),
    ("split_folder", "Alt+X"),
    ("toggle_recording", "F1"),
    ("final_trainers", ""),
    ("toggle_move_highlights", "Shift+F2"),
    ("toggle_fade_no_highlight", "Shift+F3"),
    ("toggle_test_moves", "Shift+F1"),
    ("candy_decrement", "F3"),
    ("candy_increment", "F4"),
    ("toggle_player_strat", "F9"),
    ("toggle_enemy_strat", "F10"),
    ("delete_key", "Delete"),
    ("scroll_home", "Home"),
    ("scroll_end", "End"),
    ("toggle_tabs", "`"),
    ("toggle_summary", "Ctrl+`"),
    ("toggle_map", "Ctrl+M"),
    ("show_on_map", "Ctrl+Shift+M"),
    ("export_map", "Ctrl+Shift+P"),
    ("gym_1", "1"),
    ("gym_2", "2"),
    ("gym_3", "3"),
    ("gym_4", "4"),
    ("gym_5", "5"),
    ("gym_6", "6"),
    ("gym_7", "7"),
    ("gym_8", "8"),
    ("gym_blue", "9"),
    ("e4_1", "Ctrl+1"),
    ("e4_2", "Ctrl+2"),
    ("e4_3", "Ctrl+3"),
    ("e4_4", "Ctrl+4"),
    ("e4_5", "Ctrl+5"),
    ("e4_6", "Ctrl+6"),
    ("e4_7", "Ctrl+7"),
    ("filter_trainer", "Ctrl+F"),
    ("filter_rare_candy", "Ctrl+R"),
    ("filter_tm_hm", "Ctrl+T"),
    ("filter_vitamin", "Ctrl+G"),
    ("filter_wild_pkmn", "Ctrl+W"),
    ("filter_acquire_item", ""),
    ("filter_purchase_item", ""),
    ("filter_use_item", ""),
    ("filter_reorder_bag", ""),
    ("filter_sell_item", ""),
    ("filter_hold_item", ""),
    ("filter_levelup_move", ""),
    ("filter_save", ""),
    ("filter_heal", ""),
    ("filter_blackout", ""),
    ("filter_evolution", ""),
    ("filter_ev_override", ""),
    ("filter_notes", ""),
    ("filter_common", "Ctrl+A"),
    ("filter_reset", "Ctrl+Shift+R"),
];

pub const SHORTCUT_LABELS: &[(&str, &str)] = &[
    ("customize_dvs", "Customize DVs"),
    ("new_route", "New Route"),
    ("load_route", "Load Route"),
    ("compare_routes", "Compare Routes"),
    ("save_route", "Save Route"),
    ("close_route", "Close Route"),
    ("auto_load_recent", "Auto-Load Most Recent Route"),
    ("export_notes", "Export Notes"),
    ("screenshot_events", "Screenshot Event List"),
    ("screenshot_battle", "Screenshot Battle Summary"),
    ("screenshot_player", "Screenshot Player Ranges"),
    ("screenshot_enemy", "Screenshot Enemy Ranges"),
    ("open_image_folder", "Open Image Folder"),
    ("config_font", "Config Font"),
    ("custom_gens", "Custom Gens"),
    ("app_config", "App Config"),
    ("open_data_folder", "Open Data Folder"),
    ("keyboard_shortcuts", "Keyboard Shortcuts"),
    ("undo", "Undo"),
    ("move_event_up", "Move Event Up"),
    ("move_event_down", "Move Event Down"),
    ("move_event_up_folder", "Move Event Up To Next Folder"),
    ("move_event_down_folder", "Move Event Down To Next Folder"),
    ("enable_disable", "Enable/Disable"),
    ("toggle_highlight", "Toggle Highlight"),
    ("delete_event", "Delete Event"),
    ("highlight_1", "Highlight 1"),
    ("highlight_2", "Highlight 2"),
    ("highlight_3", "Highlight 3"),
    ("highlight_4", "Highlight 4"),
    ("highlight_5", "Highlight 5"),
    ("highlight_6", "Highlight 6"),
    ("highlight_7", "Highlight 7"),
    ("highlight_8", "Highlight 8"),
    ("highlight_9", "Highlight 9"),
    ("new_folder", "New Folder"),
    ("rename_folder", "Rename Current Folder"),
    ("split_folder", "Split Folder"),
    ("toggle_recording", "Toggle Recording"),
    ("final_trainers", "Configure Final Trainers"),
    ("toggle_move_highlights", "Toggle Move Highlights"),
    ("toggle_fade_no_highlight", "Toggle Fade Moves Without Highlight"),
    ("toggle_test_moves", "Toggle Test Moves"),
    ("candy_decrement", "Decrement Pre-Fight Candies"),
    ("candy_increment", "Increment Pre-Fight Candies"),
    ("toggle_player_strat", "Toggle Player Highlight Strategy"),
    ("toggle_enemy_strat", "Toggle Enemy Highlight Strategy"),
    ("delete_key", "Delete Event (Delete Key)"),
    ("scroll_home", "Scroll to Top"),
    ("scroll_end", "Scroll to Bottom"),
    ("toggle_tabs", "Toggle Event Tabs"),
    ("toggle_summary", "Toggle Summary Window"),
    ("toggle_map", "Toggle Map"),
    ("show_on_map", "Show Selected Event on Map"),
    ("export_map", "Export Map Image"),
    ("gym_1", "Select Gym Leader 1"),
    ("gym_2", "Select Gym Leader 2"),
    ("gym_3", "Select Gym Leader 3"),
    ("gym_4", "Select Gym Leader 4"),
    ("gym_5", "Select Gym Leader 5"),
    ("gym_6", "Select Gym Leader 6"),
    ("gym_7", "Select Gym Leader 7"),
    ("gym_8", "Select Gym Leader 8"),
    ("gym_blue", "Select Blue (Gen 2 / HGSS)"),
    ("e4_1", "Select Elite Four 1"),
    ("e4_2", "Select Elite Four 2"),
    ("e4_3", "Select Elite Four 3"),
    ("e4_4", "Select Elite Four 4"),
    ("e4_5", "Select Elite Four 5"),
    ("e4_6", "Select Elite Four/Champion 6"),
    ("e4_7", "Select Elite Four/Champion 7"),
    ("filter_trainer", "Toggle Trainer Filter"),
    ("filter_rare_candy", "Toggle Rare Candy Filter"),
    ("filter_tm_hm", "Toggle TM/HM Filter"),
    ("filter_vitamin", "Toggle Vitamin Filter"),
    ("filter_wild_pkmn", "Toggle Wild Pkmn Filter"),
    ("filter_acquire_item", "Toggle Acquire Item Filter"),
    ("filter_purchase_item", "Toggle Purchase Item Filter"),
    ("filter_use_item", "Toggle Use/Drop Item Filter"),
    ("filter_reorder_bag", "Toggle Reorder Bag Filter"),
    ("filter_sell_item", "Toggle Sell Item Filter"),
    ("filter_hold_item", "Toggle Hold Item Filter"),
    ("filter_levelup_move", "Toggle Levelup Move Filter"),
    ("filter_save", "Toggle Game Save Filter"),
    ("filter_heal", "Toggle Heal Filter"),
    ("filter_blackout", "Toggle Blackout Filter"),
    ("filter_evolution", "Toggle Evolution Filter"),
    ("filter_ev_override", "Toggle EV Override Filter"),
    ("filter_notes", "Toggle Notes Only Filter"),
    ("filter_common", "Toggle Common Filters"),
    ("filter_reset", "Reset All Filters"),
];

pub const SHORTCUT_CATEGORIES: &[(&str, &[&str])] = &[
    (
        "File",
        &[
            "customize_dvs",
            "new_route",
            "load_route",
            "compare_routes",
            "save_route",
            "close_route",
            "auto_load_recent",
            "export_notes",
            "screenshot_events",
            "screenshot_battle",
            "screenshot_player",
            "screenshot_enemy",
            "open_image_folder",
            "config_font",
            "custom_gens",
            "app_config",
            "open_data_folder",
            "keyboard_shortcuts",
        ],
    ),
    (
        "Events",
        &[
            "undo",
            "move_event_up",
            "move_event_down",
            "move_event_up_folder",
            "move_event_down_folder",
            "enable_disable",
            "toggle_highlight",
            "delete_event",
        ],
    ),
    (
        "Highlight",
        &[
            "highlight_1",
            "highlight_2",
            "highlight_3",
            "highlight_4",
            "highlight_5",
            "highlight_6",
            "highlight_7",
            "highlight_8",
            "highlight_9",
        ],
    ),
    ("Folders", &["new_folder", "rename_folder", "split_folder"]),
    ("Recording", &["toggle_recording", "final_trainers"]),
    (
        "Battle Summary",
        &[
            "toggle_move_highlights",
            "toggle_fade_no_highlight",
            "toggle_test_moves",
            "candy_decrement",
            "candy_increment",
            "toggle_player_strat",
            "toggle_enemy_strat",
        ],
    ),
    (
        "Navigation",
        &[
            "delete_key",
            "scroll_home",
            "scroll_end",
            "toggle_tabs",
            "toggle_summary",
            "toggle_map",
            "show_on_map",
            "export_map",
            "gym_1",
            "gym_2",
            "gym_3",
            "gym_4",
            "gym_5",
            "gym_6",
            "gym_7",
            "gym_8",
            "gym_blue",
            "e4_1",
            "e4_2",
            "e4_3",
            "e4_4",
            "e4_5",
            "e4_6",
            "e4_7",
        ],
    ),
    (
        "Filters",
        &[
            "filter_trainer",
            "filter_rare_candy",
            "filter_tm_hm",
            "filter_vitamin",
            "filter_wild_pkmn",
            "filter_acquire_item",
            "filter_purchase_item",
            "filter_use_item",
            "filter_reorder_bag",
            "filter_sell_item",
            "filter_hold_item",
            "filter_levelup_move",
            "filter_save",
            "filter_heal",
            "filter_blackout",
            "filter_evolution",
            "filter_ev_override",
            "filter_notes",
            "filter_common",
            "filter_reset",
        ],
    ),
];

pub const DEFAULT_FINAL_TRAINERS_PER_GAME: &[(&str, &[&str])] = &[
    ("Red", &["Rival3 Squirtle", "Rival3 Bulbasaur", "Rival3 Charmander"]),
    ("Blue", &["Rival3 Squirtle", "Rival3 Bulbasaur", "Rival3 Charmander"]),
    ("Yellow", &["Rival3 Jolteon", "Rival3 Flareon", "Rival3 Vaporeon"]),
    ("Gold", &["Leader Red"]),
    ("Silver", &["Leader Red"]),
    ("Crystal", &["Leader Red"]),
    ("Ruby", &["Champion Steven"]),
    ("Sapphire", &["Champion Steven"]),
    ("Emerald", &["Rival Steven"]),
    ("FireRed", &["Champion Squirtle", "Champion Bulbasaur", "Champion Charmander"]),
    ("LeafGreen", &["Champion Squirtle", "Champion Bulbasaur", "Champion Charmander"]),
    ("Diamond", &["Champion Cynthia"]),
    ("Pearl", &["Champion Cynthia"]),
    ("Platinum", &["Champion Cynthia"]),
    ("HeartGold", &["Pokemon Trainer Red"]),
    ("SoulSilver", &["Pokemon Trainer Red"]),
];

pub fn default_shortcut(action_id: &str) -> &'static str {
    DEFAULT_SHORTCUTS
        .iter()
        .find(|(k, _)| *k == action_id)
        .map(|(_, v)| *v)
        .unwrap_or("")
}

pub fn shortcut_label(action_id: &str) -> String {
    SHORTCUT_LABELS
        .iter()
        .find(|(k, _)| *k == action_id)
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| action_id.to_string())
}

pub const COLOR_SCHEME_VERSION: i64 = 1;
pub const COLOR_SCHEME_VERSION_KEY: &str = "color_scheme_version";

pub const DEFAULT_SUCCESS: &str = "#4ec97a";
pub const DEFAULT_WARNING: &str = "#e8b730";
pub const DEFAULT_FAILURE: &str = "#e05555";
pub const DEFAULT_DIVIDER: &str = "#555555";
pub const DEFAULT_HEADER: &str = "#e8a850";
pub const DEFAULT_PRIMARY: &str = "#7cb8e0";
pub const DEFAULT_SECONDARY: &str = "#999999";
pub const DEFAULT_CONTRAST: &str = "#e0e0e0";
pub const DEFAULT_BACKGROUND: &str = "#1e1e1e";
pub const DEFAULT_TEXT_COLOR: &str = "#d4d4d4";
pub const DEFAULT_FONT_NAME: &str = "Segoe UI";

pub const DEFAULT_CONSISTENT_THRESHOLD: i64 = 90;
pub const DEFAULT_IGNORE_ACCURACY: bool = false;
pub const DEFAULT_FORCE_FULL_SEARCH: bool = false;
pub const DEFAULT_DAMAGE_SEARCH_DEPTH: i64 = 20;
pub const DEFAULT_DEBUG_MODE: bool = false;
pub const DEFAULT_AUTO_SWITCH: bool = true;
pub const DEFAULT_NOTES_VISIBILITY: &str = "when_space_allows";

/// One source of truth for the highlight palette (Appendix B item 6).
pub const HIGHLIGHT_COLOR_DEFAULTS: [&str; 9] = [
    "#903858", "#388038", "#887828", "#983030", "#787878", "#5028a0", "#c83838", "#389080", "#606060",
];

pub const FIGHT_CATEGORY_COLOR_DEFAULTS: &[(&str, &str)] = &[
    ("rival", "#12196b"),
    ("gym_leader", "#0d0d0d"),
    ("elite_four", "#38084d"),
    ("champion", "#054d3f"),
    ("post_game", "#0c2a0c"),
    ("boss", "#808080"),
    ("team_leader", "#702060"),
];

pub const FIGHT_CATEGORY_ORDER: [&str; 7] = [
    "rival", "gym_leader", "elite_four", "champion", "post_game", "boss", "team_leader",
];

/// The three config keys that change computed battle-summary output,
/// passed by value into the calculation layer (no ambient globals).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CalcConfig {
    pub damage_search_depth: i64,
    pub force_full_search: bool,
    pub ignore_accuracy: bool,
    pub consistent_threshold: i64,
}

impl Default for CalcConfig {
    fn default() -> Self {
        CalcConfig {
            damage_search_depth: DEFAULT_DAMAGE_SEARCH_DEPTH,
            force_full_search: DEFAULT_FORCE_FULL_SEARCH,
            ignore_accuracy: DEFAULT_IGNORE_ACCURACY,
            consistent_threshold: DEFAULT_CONSISTENT_THRESHOLD,
        }
    }
}

/// Loose values are kept as raw JSON so that what the user's file contains
/// round-trips (a threshold stored as a string, say) exactly the way the
/// Python getters tolerate it.
#[derive(Clone, Debug)]
pub struct Config {
    pub config_file: PathBuf,
    pub config_dir: PathBuf,

    window_geometry: Value,
    user_data_dir: String,
    images_dir: String,

    success_color: String,
    warning_color: String,
    failure_color: String,
    divider_color: String,
    header_color: String,
    primary_color: String,
    secondary_color: String,
    contrast_color: String,
    background_color: String,
    text_color: String,

    player_highlight_strategy: Value,
    enemy_highlight_strategy: Value,
    consistent_threshold: Value,
    ignore_accuracy: Value,
    damage_search_depth: Value,
    force_full_search: Value,

    custom_font_name: Value,
    debug_mode: Value,
    auto_switch: Value,
    notes_visibility: Value,

    window_state: Value,
    auto_load_most_recent: Value,
    fade_folder_text: Value,
    highlight_branched_mandatory: Value,
    show_move_highlights: Value,
    show_legacy_controls: Value,
    fade_moves_without_highlight: Value,
    test_moves_enabled: Value,
    landing_search_filter: Value,
    landing_sort: Value,
    landing_game_filter: Value,
    run_summary_docked: Value,
    color_major_battles: Value,
    suppress_update_prompt: Value,
    notes_collapsed: Value,
    pre_state_left_fraction: Value,
    battle_summary_left_fraction: Value,
    map_docked: Value,
    map_open: Value,
    map_left_fraction: Value,
    map_night: Value,
    map_toggles: Value,
    map_view_state: Value,
    map_texture_budget_mb: Value,

    highlight_colors: IndexMap<i64, Value>,
    fight_category_colors: IndexMap<String, Value>,
    shortcut_overrides: IndexMap<String, Value>,
    final_trainers_per_game: IndexMap<String, Vec<String>>,
    recording_auto_stop_enabled: bool,

    // not persisted
    custom_image_path: String,
}

impl Config {
    pub fn load(config_file: &Path) -> Config {
        let raw: Value = std::fs::read(config_file)
            .ok()
            .and_then(|b| pyjson::loads(&pyjson::decode_text(&b)).ok())
            .unwrap_or(Value::Object(Default::default()));
        let raw_present = raw.as_object().map(|o| !o.is_empty()).unwrap_or(false);
        let g = |k: &str| pyjson::get(&raw, k).cloned();
        let gv = |k: &str, d: Value| g(k).unwrap_or(d);

        let user_data_dir = g(consts::USER_LOCATION_DATA_KEY)
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| io_utils::get_default_user_data_dir().to_string_lossy().to_string());
        let images_dir = g(consts::IMAGE_LOCATION_KEY)
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| {
                Path::new(&user_data_dir)
                    .join(consts::SAVED_IMAGES_FOLDER_NAME)
                    .to_string_lossy()
                    .to_string()
            });

        let colors_outdated = pyjson::get_i64(&raw, COLOR_SCHEME_VERSION_KEY).unwrap_or(0) != COLOR_SCHEME_VERSION;
        let color = |k: &str, d: &str| -> String {
            if colors_outdated {
                return d.to_string();
            }
            match g(k) {
                Some(Value::String(s)) => s,
                Some(other) => pyjson::python_str_number(&other),
                None => d.to_string(),
            }
        };

        let mut highlight_colors = IndexMap::new();
        let mut fight_category_colors = IndexMap::new();
        for cat in FIGHT_CATEGORY_ORDER {
            let key = format!("fight_category_color_{}", cat);
            if let Some(v) = g(&key) {
                fight_category_colors.insert(cat.to_string(), v);
            }
        }
        for i in 1..=9 {
            let key = format!("highlight_color_{}", i);
            if let Some(v) = g(&key) {
                highlight_colors.insert(i, v);
            }
        }
        let shortcut_overrides: IndexMap<String, Value> = match g("keyboard_shortcuts") {
            Some(Value::Object(o)) => o.into_iter().collect(),
            _ => IndexMap::new(),
        };
        let mut final_trainers_per_game = IndexMap::new();
        if let Some(Value::Object(o)) = g("final_trainers_per_game") {
            for (k, v) in o {
                if let Value::Array(items) = v {
                    final_trainers_per_game.insert(
                        k,
                        items.iter().map(pyjson::python_str_number).collect::<Vec<String>>(),
                    );
                }
            }
        }
        let recording_auto_stop_enabled = g("recording_auto_stop_enabled").map(|v| pyjson::truthy(&v)).unwrap_or(true);

        let cfg = Config {
            config_file: config_file.to_path_buf(),
            config_dir: config_file.parent().map(|p| p.to_path_buf()).unwrap_or_default(),
            window_geometry: gv(consts::CONFIG_WINDOW_GEOMETRY, Value::String(String::new())),
            user_data_dir,
            images_dir,
            success_color: color(consts::SUCCESS_COLOR_KEY, DEFAULT_SUCCESS),
            warning_color: color(consts::WARNING_COLOR_KEY, DEFAULT_WARNING),
            failure_color: color(consts::FAILURE_COLOR_KEY, DEFAULT_FAILURE),
            divider_color: color(consts::DIVIDER_COLOR_KEY, DEFAULT_DIVIDER),
            header_color: color(consts::HEADER_COLOR_KEY, DEFAULT_HEADER),
            primary_color: color(consts::PRIMARY_COLOR_KEY, DEFAULT_PRIMARY),
            secondary_color: color(consts::SECONDARY_COLOR_KEY, DEFAULT_SECONDARY),
            contrast_color: color(consts::CONTRAST_COLOR_KEY, DEFAULT_CONTRAST),
            background_color: color(consts::BACKGROUND_COLOR_KEY, DEFAULT_BACKGROUND),
            text_color: color(consts::TEXT_COLOR_KEY, DEFAULT_TEXT_COLOR),
            player_highlight_strategy: gv(
                consts::PLAYER_HIGHLIGHT_STRATEGY_KEY,
                Value::String(consts::HIGHLIGHT_FASTEST_KILL.to_string()),
            ),
            enemy_highlight_strategy: gv(
                consts::ENEMY_HIGHLIGHT_STRATEGY_KEY,
                Value::String(consts::HIGHLIGHT_FASTEST_KILL.to_string()),
            ),
            consistent_threshold: gv(consts::CONSISTENT_HIGHLIGHT_THRESHOLD, Value::from(DEFAULT_CONSISTENT_THRESHOLD)),
            ignore_accuracy: gv(consts::IGNORE_ACCURACY_IN_DAMAGE_CALCS, Value::Bool(DEFAULT_IGNORE_ACCURACY)),
            damage_search_depth: gv(consts::DAMAGE_SEARCH_DEPTH, Value::from(DEFAULT_DAMAGE_SEARCH_DEPTH)),
            force_full_search: gv(consts::FORCE_FULL_SEARCH, Value::Bool(DEFAULT_FORCE_FULL_SEARCH)),
            custom_font_name: gv(consts::CUSTOM_FONT_NAME_KEY, Value::String(DEFAULT_FONT_NAME.to_string())),
            debug_mode: gv(consts::DEBUG_MODE_KEY, Value::Bool(DEFAULT_DEBUG_MODE)),
            auto_switch: gv(consts::AUTO_SWITCH_KEY, Value::Bool(DEFAULT_AUTO_SWITCH)),
            notes_visibility: gv(consts::NOTES_VISIBILITY_KEY, Value::String(DEFAULT_NOTES_VISIBILITY.to_string())),
            window_state: gv("window_state", Value::String("normal".to_string())),
            auto_load_most_recent: gv("auto_load_most_recent_route", Value::Bool(false)),
            fade_folder_text: gv("fade_folder_text", Value::Bool(false)),
            highlight_branched_mandatory: gv("highlight_branched_mandatory", Value::Bool(false)),
            show_move_highlights: gv("show_move_highlights", Value::Bool(true)),
            show_legacy_controls: gv("show_legacy_controls", Value::Bool(true)),
            fade_moves_without_highlight: gv("fade_moves_without_highlight", Value::Bool(false)),
            test_moves_enabled: gv("test_moves_enabled", Value::Bool(false)),
            landing_search_filter: gv("landing_search_filter", Value::String(String::new())),
            landing_sort: gv("landing_sort", Value::String("recent".to_string())),
            landing_game_filter: gv("landing_game_filter", Value::String("All".to_string())),
            run_summary_docked: gv("run_summary_docked", Value::Bool(true)),
            color_major_battles: gv("color_major_battles", Value::Bool(true)),
            suppress_update_prompt: gv("suppress_update_prompt", Value::Bool(false)),
            notes_collapsed: gv("notes_collapsed", Value::Bool(false)),
            pre_state_left_fraction: gv("pre_state_left_fraction", Value::Null),
            battle_summary_left_fraction: gv("battle_summary_left_fraction", Value::Null),
            map_docked: gv("map_docked", Value::Bool(true)),
            map_open: gv("map_open", Value::Bool(false)),
            map_left_fraction: gv("map_left_fraction", Value::Null),
            map_night: gv("map_night", Value::Bool(false)),
            map_toggles: gv("map_toggles", Value::Null),
            map_view_state: gv("map_view_state", Value::Null),
            map_texture_budget_mb: gv("map_texture_budget_mb", Value::from(256)),
            highlight_colors,
            fight_category_colors,
            shortcut_overrides,
            final_trainers_per_game,
            recording_auto_stop_enabled,
            custom_image_path: String::new(),
        };
        if colors_outdated && raw_present {
            cfg.save();
        }
        cfg
    }

    /// `Config._save`: the same key order as the Python implementation.
    pub fn to_json(&self) -> Value {
        let mut pairs: Vec<(String, Value)> = vec![
            (COLOR_SCHEME_VERSION_KEY.into(), Value::from(COLOR_SCHEME_VERSION)),
            (consts::CONFIG_WINDOW_GEOMETRY.into(), self.window_geometry.clone()),
            (consts::USER_LOCATION_DATA_KEY.into(), Value::String(self.user_data_dir.clone())),
            (consts::SUCCESS_COLOR_KEY.into(), Value::String(self.success_color.clone())),
            (consts::WARNING_COLOR_KEY.into(), Value::String(self.warning_color.clone())),
            (consts::FAILURE_COLOR_KEY.into(), Value::String(self.failure_color.clone())),
            (consts::DIVIDER_COLOR_KEY.into(), Value::String(self.divider_color.clone())),
            (consts::HEADER_COLOR_KEY.into(), Value::String(self.header_color.clone())),
            (consts::PRIMARY_COLOR_KEY.into(), Value::String(self.primary_color.clone())),
            (consts::SECONDARY_COLOR_KEY.into(), Value::String(self.secondary_color.clone())),
            (consts::CONTRAST_COLOR_KEY.into(), Value::String(self.contrast_color.clone())),
            (consts::BACKGROUND_COLOR_KEY.into(), Value::String(self.background_color.clone())),
            (consts::TEXT_COLOR_KEY.into(), Value::String(self.text_color.clone())),
            (consts::CUSTOM_FONT_NAME_KEY.into(), self.custom_font_name.clone()),
            (consts::PLAYER_HIGHLIGHT_STRATEGY_KEY.into(), self.player_highlight_strategy.clone()),
            (consts::ENEMY_HIGHLIGHT_STRATEGY_KEY.into(), self.enemy_highlight_strategy.clone()),
            (consts::CONSISTENT_HIGHLIGHT_THRESHOLD.into(), self.consistent_threshold.clone()),
            (consts::IGNORE_ACCURACY_IN_DAMAGE_CALCS.into(), self.ignore_accuracy.clone()),
            (consts::DAMAGE_SEARCH_DEPTH.into(), self.damage_search_depth.clone()),
            (consts::FORCE_FULL_SEARCH.into(), self.force_full_search.clone()),
            (consts::DEBUG_MODE_KEY.into(), self.debug_mode.clone()),
            (consts::AUTO_SWITCH_KEY.into(), self.auto_switch.clone()),
            (consts::NOTES_VISIBILITY_KEY.into(), self.notes_visibility.clone()),
            ("window_state".into(), self.window_state.clone()),
            ("auto_load_most_recent_route".into(), self.auto_load_most_recent.clone()),
            ("fade_folder_text".into(), self.fade_folder_text.clone()),
            ("highlight_branched_mandatory".into(), self.highlight_branched_mandatory.clone()),
            ("show_move_highlights".into(), self.show_move_highlights.clone()),
            ("show_legacy_controls".into(), self.show_legacy_controls.clone()),
            ("fade_moves_without_highlight".into(), self.fade_moves_without_highlight.clone()),
            ("test_moves_enabled".into(), self.test_moves_enabled.clone()),
            ("landing_search_filter".into(), self.landing_search_filter.clone()),
            ("landing_sort".into(), self.landing_sort.clone()),
            ("landing_game_filter".into(), self.landing_game_filter.clone()),
            ("run_summary_docked".into(), self.run_summary_docked.clone()),
            ("color_major_battles".into(), self.color_major_battles.clone()),
            ("suppress_update_prompt".into(), self.suppress_update_prompt.clone()),
            ("notes_collapsed".into(), self.notes_collapsed.clone()),
            ("pre_state_left_fraction".into(), self.pre_state_left_fraction.clone()),
            ("battle_summary_left_fraction".into(), self.battle_summary_left_fraction.clone()),
            ("map_docked".into(), self.map_docked.clone()),
            ("map_open".into(), self.map_open.clone()),
            ("map_left_fraction".into(), self.map_left_fraction.clone()),
            ("map_night".into(), self.map_night.clone()),
            ("map_toggles".into(), self.map_toggles.clone()),
            ("map_view_state".into(), self.map_view_state.clone()),
            ("map_texture_budget_mb".into(), self.map_texture_budget_mb.clone()),
        ];
        for i in 1..=9 {
            if let Some(v) = self.highlight_colors.get(&i) {
                pairs.push((format!("highlight_color_{}", i), v.clone()));
            }
        }
        for (cat, v) in &self.fight_category_colors {
            pairs.push((format!("fight_category_color_{}", cat), v.clone()));
        }
        if !self.shortcut_overrides.is_empty() {
            let mut m = serde_json::Map::new();
            for (k, v) in &self.shortcut_overrides {
                m.insert(k.clone(), v.clone());
            }
            pairs.push(("keyboard_shortcuts".into(), Value::Object(m)));
        }
        if !self.final_trainers_per_game.is_empty() {
            let mut m = serde_json::Map::new();
            for (k, v) in &self.final_trainers_per_game {
                m.insert(k.clone(), pyjson::str_array(v));
            }
            pairs.push(("final_trainers_per_game".into(), Value::Object(m)));
        }
        if !self.recording_auto_stop_enabled {
            pairs.push(("recording_auto_stop_enabled".into(), Value::Bool(false)));
        }
        let mut map = serde_json::Map::new();
        for (k, v) in pairs {
            map.insert(k, v);
        }
        Value::Object(map)
    }

    pub fn save(&self) {
        let bytes = pyjson::dump_indent4_platform_bytes(&self.to_json());
        if let Err(e) = io_utils::write_atomic(&self.config_file, &bytes) {
            log::error!("Failed to write config file {}: {}", self.config_file.display(), e);
        }
    }

    // ---- getters / setters -------------------------------------------------

    pub fn get_window_geometry(&self) -> String {
        match &self.window_geometry {
            Value::String(s) => s.clone(),
            other => pyjson::python_str_number(other),
        }
    }
    pub fn set_window_geometry(&mut self, g: &str) {
        if self.window_geometry.as_str() != Some(g) {
            self.window_geometry = Value::String(g.to_string());
            self.save();
        }
    }

    pub fn get_user_data_dir(&self) -> PathBuf {
        PathBuf::from(&self.user_data_dir)
    }
    pub fn get_user_data_dir_str(&self) -> &str {
        &self.user_data_dir
    }
    pub fn set_user_data_dir(&mut self, new_dir: &str) {
        self.user_data_dir = new_dir.to_string();
        self.save();
    }

    pub fn get_images_dir(&self) -> PathBuf {
        PathBuf::from(&self.images_dir)
    }
    pub fn set_images_dir(&mut self, d: &str) {
        self.images_dir = d.to_string();
        self.save();
    }

    pub fn get_success_color(&self) -> &str { &self.success_color }
    pub fn get_warning_color(&self) -> &str { &self.warning_color }
    pub fn get_failure_color(&self) -> &str { &self.failure_color }
    pub fn get_divider_color(&self) -> &str { &self.divider_color }
    pub fn get_header_color(&self) -> &str { &self.header_color }
    pub fn get_primary_color(&self) -> &str { &self.primary_color }
    pub fn get_secondary_color(&self) -> &str { &self.secondary_color }
    pub fn get_contrast_color(&self) -> &str { &self.contrast_color }
    pub fn get_background_color(&self) -> &str { &self.background_color }
    pub fn get_text_color(&self) -> &str { &self.text_color }

    pub fn set_success_color(&mut self, c: &str) { self.success_color = c.into(); self.save(); }
    pub fn set_warning_color(&mut self, c: &str) { self.warning_color = c.into(); self.save(); }
    pub fn set_failure_color(&mut self, c: &str) { self.failure_color = c.into(); self.save(); }
    pub fn set_divider_color(&mut self, c: &str) { self.divider_color = c.into(); self.save(); }
    pub fn set_header_color(&mut self, c: &str) { self.header_color = c.into(); self.save(); }
    pub fn set_primary_color(&mut self, c: &str) { self.primary_color = c.into(); self.save(); }
    pub fn set_secondary_color(&mut self, c: &str) { self.secondary_color = c.into(); self.save(); }
    pub fn set_contrast_color(&mut self, c: &str) { self.contrast_color = c.into(); self.save(); }
    pub fn set_background_color(&mut self, c: &str) { self.background_color = c.into(); self.save(); }
    pub fn set_text_color(&mut self, c: &str) { self.text_color = c.into(); self.save(); }

    pub fn reset_all_colors(&mut self) {
        self.success_color = DEFAULT_SUCCESS.into();
        self.warning_color = DEFAULT_WARNING.into();
        self.failure_color = DEFAULT_FAILURE.into();
        self.divider_color = DEFAULT_DIVIDER.into();
        self.header_color = DEFAULT_HEADER.into();
        self.primary_color = DEFAULT_PRIMARY.into();
        self.secondary_color = DEFAULT_SECONDARY.into();
        self.contrast_color = DEFAULT_CONTRAST.into();
        self.background_color = DEFAULT_BACKGROUND.into();
        self.text_color = DEFAULT_TEXT_COLOR.into();
        self.save();
    }

    fn strat_of(v: &Value) -> &'static str {
        if let Some(s) = v.as_str() {
            for candidate in consts::ALL_HIGHLIGHT_STRATS {
                if candidate == s {
                    return candidate;
                }
            }
        }
        consts::HIGHLIGHT_NONE
    }

    pub fn get_player_highlight_strategy(&self) -> &'static str {
        Config::strat_of(&self.player_highlight_strategy)
    }
    pub fn get_enemy_highlight_strategy(&self) -> &'static str {
        Config::strat_of(&self.enemy_highlight_strategy)
    }
    pub fn set_player_highlight_strategy(&mut self, s: &str) {
        self.player_highlight_strategy = Value::String(s.into());
        self.save();
    }
    pub fn set_enemy_highlight_strategy(&mut self, s: &str) {
        self.enemy_highlight_strategy = Value::String(s.into());
        self.save();
    }

    /// Validation in the getter, as in Python: must be an int in 0..=99.
    pub fn get_consistent_threshold(&self) -> i64 {
        match &self.consistent_threshold {
            Value::Number(n) if n.is_i64() || n.is_u64() => {
                let v = n.as_i64().unwrap_or(-1);
                if !(0..=99).contains(&v) {
                    DEFAULT_CONSISTENT_THRESHOLD
                } else {
                    v
                }
            }
            _ => DEFAULT_CONSISTENT_THRESHOLD,
        }
    }
    pub fn set_consistent_threshold(&mut self, t: i64) {
        self.consistent_threshold = Value::from(t);
        self.save();
    }

    pub fn get_damage_search_depth(&self) -> i64 {
        match &self.damage_search_depth {
            Value::Number(n) if n.is_i64() || n.is_u64() => {
                let v = n.as_i64().unwrap_or(-1);
                if v < 0 {
                    DEFAULT_DAMAGE_SEARCH_DEPTH
                } else {
                    v
                }
            }
            _ => DEFAULT_DAMAGE_SEARCH_DEPTH,
        }
    }
    pub fn set_damage_search_depth(&mut self, d: i64) {
        self.damage_search_depth = Value::from(d);
        self.save();
    }

    pub fn do_force_full_search(&self) -> bool { pyjson::truthy(&self.force_full_search) }
    pub fn set_force_full_search(&mut self, v: bool) { self.force_full_search = Value::Bool(v); self.save(); }
    pub fn do_ignore_accuracy(&self) -> bool { pyjson::truthy(&self.ignore_accuracy) }
    pub fn set_ignore_accuracy(&mut self, v: bool) { self.ignore_accuracy = Value::Bool(v); self.save(); }
    pub fn is_debug_mode(&self) -> bool { pyjson::truthy(&self.debug_mode) }
    pub fn set_debug_mode(&mut self, v: bool) { self.debug_mode = Value::Bool(v); self.save(); }
    pub fn do_auto_switch(&self) -> bool { pyjson::truthy(&self.auto_switch) }
    pub fn set_auto_switch(&mut self, v: bool) { self.auto_switch = Value::Bool(v); self.save(); }

    pub fn calc_config(&self) -> CalcConfig {
        CalcConfig {
            damage_search_depth: self.get_damage_search_depth(),
            force_full_search: self.do_force_full_search(),
            ignore_accuracy: self.do_ignore_accuracy(),
            consistent_threshold: self.get_consistent_threshold(),
        }
    }

    pub fn get_pre_state_left_fraction(&self) -> Option<f64> { pyjson::value_as_f64(&self.pre_state_left_fraction) }
    pub fn set_pre_state_left_fraction(&mut self, v: Option<f64>) {
        self.pre_state_left_fraction = v.map(|f| Value::from(f)).unwrap_or(Value::Null);
        self.save();
    }
    pub fn get_battle_summary_left_fraction(&self) -> Option<f64> { pyjson::value_as_f64(&self.battle_summary_left_fraction) }
    pub fn set_battle_summary_left_fraction(&mut self, v: Option<f64>) {
        self.battle_summary_left_fraction = v.map(|f| Value::from(f)).unwrap_or(Value::Null);
        self.save();
    }

    // ---- world map (docs/rust_port/design/world_map/SPEC.md §3.8) ----
    pub fn get_map_docked(&self) -> bool { self.map_docked.as_bool().unwrap_or(true) }
    pub fn set_map_docked(&mut self, v: bool) { self.map_docked = Value::Bool(v); self.save(); }
    pub fn get_map_open(&self) -> bool { pyjson::truthy(&self.map_open) }
    pub fn set_map_open(&mut self, v: bool) { self.map_open = Value::Bool(v); self.save(); }
    pub fn get_map_left_fraction(&self) -> Option<f64> { pyjson::value_as_f64(&self.map_left_fraction) }
    pub fn set_map_left_fraction(&mut self, v: Option<f64>) {
        self.map_left_fraction = v.map(|f| Value::from(f)).unwrap_or(Value::Null);
        self.save();
    }
    pub fn get_map_night(&self) -> bool { pyjson::truthy(&self.map_night) }
    pub fn set_map_night(&mut self, v: bool) { self.map_night = Value::Bool(v); self.save(); }
    pub fn get_map_toggles(&self) -> Value { self.map_toggles.clone() }
    pub fn set_map_toggles(&mut self, v: Value) { self.map_toggles = v; self.save(); }
    pub fn get_map_view_state(&self) -> Value { self.map_view_state.clone() }
    pub fn set_map_view_state(&mut self, v: Value) { self.map_view_state = v; self.save(); }
    pub fn get_map_texture_budget_mb(&self) -> usize { self.map_texture_budget_mb.as_u64().unwrap_or(256).clamp(32, 4096) as usize }

    pub fn are_notes_visible_in_battle_summary(&self) -> bool {
        self.notes_visibility.as_str() != Some("never")
    }
    /// `'when_space_allows' | 'always' | 'never'`
    pub fn get_notes_visibility_mode(&self) -> &'static str {
        match &self.notes_visibility {
            Value::String(s) if s == "when_space_allows" => "when_space_allows",
            Value::String(s) if s == "always" => "always",
            Value::String(s) if s == "never" => "never",
            Value::Bool(true) => "always",
            Value::String(s) if s == "True" => "always",
            Value::Bool(false) => "never",
            Value::String(s) if s == "False" => "never",
            _ => "when_space_allows",
        }
    }
    pub fn set_notes_visibility_mode(&mut self, mode: &str) {
        self.notes_visibility = Value::String(mode.into());
        self.save();
    }
    pub fn set_notes_visibility_in_battle_summary(&mut self, visible: bool) {
        self.notes_visibility = Value::String(if visible { "always" } else { "never" }.into());
        self.save();
    }
    pub fn get_notes_collapsed(&self) -> bool { pyjson::truthy(&self.notes_collapsed) }
    pub fn set_notes_collapsed(&mut self, v: bool) { self.notes_collapsed = Value::Bool(v); self.save(); }

    pub fn get_custom_font_name(&self) -> String {
        match &self.custom_font_name {
            Value::String(s) => s.clone(),
            other => pyjson::python_str_number(other),
        }
    }
    pub fn set_custom_font_name(&mut self, n: &str) { self.custom_font_name = Value::String(n.into()); self.save(); }

    pub fn get_window_state(&self) -> String {
        match &self.window_state {
            Value::String(s) => s.clone(),
            other => pyjson::python_str_number(other),
        }
    }
    pub fn set_window_state(&mut self, s: &str) { self.window_state = Value::String(s.into()); self.save(); }

    pub fn get_auto_load_most_recent_route(&self) -> bool { pyjson::truthy(&self.auto_load_most_recent) }
    pub fn set_auto_load_most_recent_route(&mut self, v: bool) { self.auto_load_most_recent = Value::Bool(v); self.save(); }

    pub fn get_highlight_color(&self, idx: i64) -> String {
        if let Some(Value::String(s)) = self.highlight_colors.get(&idx) {
            return s.clone();
        }
        if (1..=9).contains(&idx) {
            return HIGHLIGHT_COLOR_DEFAULTS[(idx - 1) as usize].to_string();
        }
        "#444444".to_string()
    }
    pub fn set_highlight_color(&mut self, idx: i64, color: &str) {
        self.highlight_colors.insert(idx, Value::String(color.into()));
        self.save();
    }
    pub fn reset_highlight_colors_to_defaults(&mut self) {
        self.highlight_colors.clear();
        self.save();
    }

    pub fn get_fight_category_color(&self, category: &str) -> String {
        if let Some(Value::String(s)) = self.fight_category_colors.get(category) {
            return s.clone();
        }
        FIGHT_CATEGORY_COLOR_DEFAULTS
            .iter()
            .find(|(c, _)| *c == category)
            .map(|(_, v)| v.to_string())
            .unwrap_or_else(|| "#1f1f1f".to_string())
    }
    pub fn set_fight_category_color(&mut self, category: &str, color: &str) {
        self.fight_category_colors.insert(category.into(), Value::String(color.into()));
        self.save();
    }
    pub fn reset_fight_category_colors(&mut self) {
        self.fight_category_colors.clear();
        self.save();
    }

    pub fn get_color_major_battles(&self) -> bool { pyjson::truthy(&self.color_major_battles) }
    pub fn set_color_major_battles(&mut self, v: bool) { self.color_major_battles = Value::Bool(v); self.save(); }
    pub fn get_fade_folder_text(&self) -> bool { pyjson::truthy(&self.fade_folder_text) }
    pub fn set_fade_folder_text(&mut self, v: bool) { self.fade_folder_text = Value::Bool(v); self.save(); }
    pub fn get_highlight_branched_mandatory(&self) -> bool { pyjson::truthy(&self.highlight_branched_mandatory) }
    pub fn set_highlight_branched_mandatory(&mut self, v: bool) { self.highlight_branched_mandatory = Value::Bool(v); self.save(); }
    pub fn get_show_move_highlights(&self) -> bool { pyjson::truthy(&self.show_move_highlights) }
    pub fn set_show_move_highlights(&mut self, v: bool) { self.show_move_highlights = Value::Bool(v); self.save(); }
    pub fn get_show_legacy_controls(&self) -> bool { pyjson::truthy(&self.show_legacy_controls) }
    pub fn set_show_legacy_controls(&mut self, v: bool) { self.show_legacy_controls = Value::Bool(v); self.save(); }
    pub fn get_fade_moves_without_highlight(&self) -> bool { pyjson::truthy(&self.fade_moves_without_highlight) }
    pub fn set_fade_moves_without_highlight(&mut self, v: bool) { self.fade_moves_without_highlight = Value::Bool(v); self.save(); }
    pub fn get_test_moves_enabled(&self) -> bool { pyjson::truthy(&self.test_moves_enabled) }
    pub fn set_test_moves_enabled(&mut self, v: bool) { self.test_moves_enabled = Value::Bool(v); self.save(); }

    pub fn get_landing_page_search_filter(&self) -> String {
        self.landing_search_filter.as_str().map(|s| s.to_string()).unwrap_or_default()
    }
    pub fn set_landing_page_search_filter(&mut self, v: &str) { self.landing_search_filter = Value::String(v.into()); self.save(); }
    pub fn get_landing_page_sort(&self) -> String {
        self.landing_sort.as_str().map(|s| s.to_string()).unwrap_or_else(|| "recent".into())
    }
    pub fn set_landing_page_sort(&mut self, v: &str) { self.landing_sort = Value::String(v.into()); self.save(); }
    pub fn get_landing_page_game_filter(&self) -> String {
        self.landing_game_filter.as_str().map(|s| s.to_string()).unwrap_or_else(|| "All".into())
    }
    pub fn set_landing_page_game_filter(&mut self, v: &str) { self.landing_game_filter = Value::String(v.into()); self.save(); }

    pub fn get_suppress_update_prompt(&self) -> bool { pyjson::truthy(&self.suppress_update_prompt) }
    pub fn set_suppress_update_prompt(&mut self, v: bool) { self.suppress_update_prompt = Value::Bool(v); self.save(); }
    pub fn get_run_summary_docked(&self) -> bool { pyjson::truthy(&self.run_summary_docked) }
    pub fn set_run_summary_docked(&mut self, v: bool) { self.run_summary_docked = Value::Bool(v); self.save(); }

    pub fn set_custom_image_path(&mut self, p: &str) { self.custom_image_path = p.into(); }
    pub fn get_custom_image_path(&self) -> &str { &self.custom_image_path }

    // ---- shortcuts ---------------------------------------------------------

    pub fn get_shortcut(&self, action_id: &str) -> String {
        if let Some(v) = self.shortcut_overrides.get(action_id) {
            return match v {
                Value::String(s) => s.clone(),
                other => pyjson::python_str_number(other),
            };
        }
        default_shortcut(action_id).to_string()
    }

    pub fn get_all_shortcuts(&self) -> IndexMap<String, String> {
        let mut merged: IndexMap<String, String> =
            DEFAULT_SHORTCUTS.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        for (k, v) in &self.shortcut_overrides {
            merged.insert(k.clone(), match v {
                Value::String(s) => s.clone(),
                other => pyjson::python_str_number(other),
            });
        }
        merged
    }

    pub fn set_shortcut(&mut self, action_id: &str, key_sequence: &str) {
        if key_sequence == default_shortcut(action_id) {
            self.shortcut_overrides.shift_remove(action_id);
        } else {
            self.shortcut_overrides.insert(action_id.into(), Value::String(key_sequence.into()));
        }
        self.save();
    }

    pub fn reset_shortcut(&mut self, action_id: &str) {
        self.shortcut_overrides.shift_remove(action_id);
        self.save();
    }

    pub fn reset_all_shortcuts(&mut self) {
        self.shortcut_overrides.clear();
        self.save();
    }

    pub fn is_shortcut_customized(&self, action_id: &str) -> bool {
        self.shortcut_overrides.contains_key(action_id)
    }

    pub fn export_shortcuts(&self, file_path: &Path) -> std::io::Result<()> {
        let mut m = serde_json::Map::new();
        for (k, v) in self.get_all_shortcuts() {
            m.insert(k, Value::String(v));
        }
        let bytes = pyjson::dump_indent4_platform_bytes(&Value::Object(m));
        std::fs::write(file_path, bytes)
    }

    pub fn import_shortcuts(&mut self, file_path: &Path) -> Result<(), String> {
        let bytes = std::fs::read(file_path).map_err(|e| e.to_string())?;
        let data = pyjson::loads(&pyjson::decode_text(&bytes)).map_err(|e| e.to_string())?;
        let Some(obj) = data.as_object() else {
            return Err("Shortcut file must contain a JSON object".into());
        };
        self.shortcut_overrides.clear();
        for (action_id, key_seq) in obj {
            if let Some((_, default)) = DEFAULT_SHORTCUTS.iter().find(|(k, _)| k == action_id) {
                if key_seq.as_str() != Some(default) {
                    self.shortcut_overrides.insert(action_id.clone(), key_seq.clone());
                }
            }
        }
        self.save();
        Ok(())
    }

    // ---- final trainers ----------------------------------------------------

    pub fn get_final_trainers(&self, game_version: &str) -> Vec<String> {
        if let Some(v) = self.final_trainers_per_game.get(game_version) {
            return v.clone();
        }
        DEFAULT_FINAL_TRAINERS_PER_GAME
            .iter()
            .find(|(k, _)| *k == game_version)
            .map(|(_, v)| v.iter().map(|s| s.to_string()).collect())
            .unwrap_or_default()
    }

    pub fn set_final_trainers(&mut self, game_version: &str, trainers: Vec<String>) {
        self.final_trainers_per_game.insert(game_version.into(), trainers);
        self.save();
    }

    pub fn reset_final_trainers(&mut self, game_version: &str) {
        if self.final_trainers_per_game.shift_remove(game_version).is_some() {
            self.save();
        }
    }

    pub fn get_recording_auto_stop_enabled(&self) -> bool { self.recording_auto_stop_enabled }
    pub fn set_recording_auto_stop_enabled(&mut self, v: bool) { self.recording_auto_stop_enabled = v; self.save(); }
}
