//! State shared by the Dex's tabs (Solodex `App.tsx` state), the persisted
//! settings, and what the Dex asks of its host.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use xpr_data::Registry;
use xpr_dex::UserBans;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DexTab {
    Pokedex,
    Evs,
    Trainers,
    Stats,
    Damage,
    Movedex,
    Natures,
    Misc,
}

impl DexTab {
    /// Tab-bar order (Solodex's minus Route).
    pub const ALL: [DexTab; 8] = [
        DexTab::Pokedex,
        DexTab::Evs,
        DexTab::Trainers,
        DexTab::Stats,
        DexTab::Damage,
        DexTab::Movedex,
        DexTab::Natures,
        DexTab::Misc,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DexTab::Pokedex => "Pokedex",
            DexTab::Evs => "EVs",
            DexTab::Trainers => "Trainers",
            DexTab::Stats => "Stats",
            DexTab::Damage => "Damage",
            DexTab::Movedex => "Movedex",
            DexTab::Natures => "Natures",
            DexTab::Misc => "Misc",
        }
    }

    /// Solodex's default key (F7 was Route, F10 / F11 the map).
    pub fn key(self) -> egui::Key {
        match self {
            DexTab::Pokedex => egui::Key::F1,
            DexTab::Evs => egui::Key::F2,
            DexTab::Trainers => egui::Key::F3,
            DexTab::Damage => egui::Key::F4,
            DexTab::Movedex => egui::Key::F5,
            DexTab::Natures => egui::Key::F6,
            DexTab::Stats => egui::Key::F8,
            DexTab::Misc => egui::Key::F9,
        }
    }

    /// Tabs that work on router versions (trainer data) instead of Dex games.
    pub fn uses_router_versions(self) -> bool {
        matches!(self, DexTab::Trainers | DexTab::Damage)
    }

    /// Tabs with no game toggle.
    pub fn gameless(self) -> bool {
        matches!(self, DexTab::Natures | DexTab::Misc)
    }
}

/// The search overlays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spotlight {
    /// pick a species (Space)
    Pokemon,
    /// pick a species to compare with the selected one (Ctrl+Space)
    Compare,
    /// pick a trainer of any version (Shift+Space)
    Trainer,
    /// pick a move for the Movedex (Ctrl+Shift+Space)
    Move,
}

/// Settings kept in the router's config (`dex_settings`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DexSettings {
    pub tab: DexTab,
    pub game: String,
    /// router version of the Trainers / Damage tabs
    pub version: Option<String>,
    pub selected: Option<String>,
    pub comparing_with: Option<String>,
    pub comparing_third: Option<String>,
    pub self_compare: bool,
    pub list_width: f32,
    pub list_open: bool,
    pub cross_out_banned: bool,
    pub cross_out_postgame: bool,
    pub cross_out_conditional: bool,
    pub user_bans: UserBans,
    pub show_movepool_diff: bool,
    pub show_bulk: bool,
    pub show_wbst: bool,
    pub show_ubst: bool,
    /// the Misc tab's sub-tab
    pub misc_tab: usize,
    /// the host shows the Dex in a window of its own instead of as a page
    pub windowed: bool,
}

impl Default for DexSettings {
    fn default() -> Self {
        DexSettings {
            tab: DexTab::Pokedex,
            game: String::new(),
            version: None,
            selected: None,
            comparing_with: None,
            comparing_third: None,
            self_compare: false,
            list_width: 288.0,
            list_open: true,
            cross_out_banned: false,
            cross_out_postgame: false,
            cross_out_conditional: false,
            user_bans: UserBans::default(),
            show_movepool_diff: true,
            show_bulk: false,
            show_wbst: false,
            show_ubst: false,
            misc_tab: 0,
            windowed: false,
        }
    }
}

/// What the Damage tab should load next (set by other tabs: the Pokédex's
/// right-clicked "test set", a trainer's "Calc Damage" button).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DamageRequest {
    /// bumped on every request so repeats count
    pub nonce: u64,
    /// router version
    pub version: Option<String>,
    /// router species name of the player's Pokémon
    pub species: Option<String>,
    /// router trainer name
    pub trainer: Option<String>,
    pub moves: Vec<String>,
}

/// What the route editor has open (for defaults and "Add to Route").
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RouteContext {
    /// router version of the open route
    pub version: Option<String>,
    /// router species name of the route's solo Pokémon
    pub solo_species: Option<String>,
}

/// Requests the Dex hands to its host after a frame.
#[derive(Clone, Debug, PartialEq)]
pub enum DexAction {
    /// insert a trainer fight into the open route (after the selected event)
    AddTrainerToRoute { version: String, trainer: String },
    /// leave the Dex page
    Close,
    /// a message for the status line / toast
    Toast(String),
    /// open a URL in the browser (Bulbapedia links)
    OpenUrl(String),
}

/// Everything the Dex's tabs share for one frame.
pub struct DexState {
    pub settings: DexSettings,
    settings_dirty: bool,
    pub spotlight: Option<Spotlight>,
    /// Movedex: the move the move search picked
    pub focused_move: Option<String>,
    /// Trainers: the selected trainer (router trainer name)
    pub selected_trainer: Option<String>,
    /// Pokédex movepool: right-clicked moves to seed the Damage tab with
    pub move_test_set: Vec<String>,
    /// Pokédex list: names after the list's filters, for Up / Down
    pub filtered_names: Vec<String>,
    /// Self comparison: the right-hand game (right-click on the game toggle)
    pub self_compare_right_game: Option<String>,
    pub damage_request: DamageRequest,
    /// the banned / postgame moves editor (movepool cross-outs) is open
    pub banned_editor_open: bool,
}

impl DexState {
    pub fn new(settings: DexSettings) -> DexState {
        DexState {
            settings,
            settings_dirty: false,
            spotlight: None,
            focused_move: None,
            selected_trainer: None,
            move_test_set: Vec::new(),
            filtered_names: Vec::new(),
            self_compare_right_game: None,
            damage_request: DamageRequest::default(),
            banned_editor_open: false,
        }
    }

    /// Mark the persisted settings as changed (the host saves them).
    pub fn touch(&mut self) {
        self.settings_dirty = true;
    }

    pub fn take_settings_dirty(&mut self) -> bool {
        std::mem::take(&mut self.settings_dirty)
    }

    pub fn game(&self) -> &str {
        &self.settings.game
    }

    pub fn tab(&self) -> DexTab {
        self.settings.tab
    }

    pub fn selected(&self) -> Option<&str> {
        self.settings.selected.as_deref()
    }

    /// Select a species normally: leaves every comparison mode (Solodex
    /// `clearCompareOnSelect`).
    pub fn select_species(&mut self, name: &str) {
        self.settings.selected = Some(name.to_string());
        self.settings.self_compare = false;
        self.settings.comparing_with = None;
        self.settings.comparing_third = None;
        self.move_test_set.clear();
        self.keep_game_for_selected();
        self.touch();
    }

    /// Change the selected species without leaving a comparison (the
    /// comparison views' own pickers).
    pub fn set_selected_keep_compare(&mut self, name: &str) {
        if self.settings.selected.as_deref() != Some(name) {
            self.move_test_set.clear();
        }
        self.settings.selected = Some(name.to_string());
        self.keep_game_for_selected();
        self.touch();
    }

    /// Keep the current game if the selected species is in it, else its first game.
    fn keep_game_for_selected(&mut self) {
        let Some(sel) = self.settings.selected.clone() else {
            return;
        };
        if self.settings.tab.uses_router_versions() || self.settings.tab.gameless() {
            return;
        }
        let games = xpr_dex::get_games_for_pokemon(&sel);
        if !games.iter().any(|g| *g == self.settings.game) {
            if let Some(first) = games.first() {
                self.settings.game = first.clone();
            }
        }
    }

    pub fn compare_with(&mut self, name: &str) {
        self.settings.comparing_with = Some(name.to_string());
        self.settings.self_compare = false;
        self.touch();
    }

    pub fn triple_compare(&mut self, name: &str) {
        self.settings.comparing_third = Some(name.to_string());
        self.touch();
    }

    pub fn self_compare(&mut self, name: Option<&str>) {
        if let Some(n) = name {
            self.settings.selected = Some(n.to_string());
        }
        self.settings.self_compare = true;
        self.self_compare_right_game = None;
        self.settings.comparing_with = None;
        self.touch();
    }

    pub fn compare_games(&mut self, right_game: &str) {
        self.settings.self_compare = true;
        self.self_compare_right_game = Some(right_game.to_string());
        self.settings.comparing_with = None;
        self.touch();
    }

    pub fn exit_compare(&mut self) {
        self.settings.comparing_with = None;
        self.settings.comparing_third = None;
        self.touch();
    }

    pub fn exit_self_compare(&mut self) {
        self.settings.self_compare = false;
        self.self_compare_right_game = None;
        self.touch();
    }

    pub fn set_game(&mut self, game: &str) {
        if self.settings.game != game {
            self.settings.game = game.to_string();
            self.touch();
        }
    }

    pub fn set_version(&mut self, version: &str, registry: &Arc<Registry>) {
        if self.settings.version.as_deref() != Some(version) {
            self.settings.version = Some(version.to_string());
            if let Some(g) = dex_game_for_version(registry, version) {
                self.settings.game = g.to_string();
            }
            self.touch();
        }
    }

    /// Ask the Damage tab to load something and switch to it.
    pub fn request_damage(
        &mut self,
        version: Option<String>,
        species: Option<String>,
        trainer: Option<String>,
        moves: Vec<String>,
    ) {
        let nonce = self.damage_request.nonce + 1;
        self.damage_request = DamageRequest {
            nonce,
            version,
            species,
            trainer,
            moves,
        };
        self.settings.tab = DexTab::Damage;
        self.touch();
    }
}

/// The Dex game whose Pokédex a router version uses (custom gens through
/// their base version).
pub fn dex_game_for_version(registry: &Arc<Registry>, version: &str) -> Option<&'static str> {
    if let Some(g) = xpr_dex::games::game_for_router_version(version) {
        return Some(g);
    }
    let gen = registry.get_version(version).ok()?;
    gen.base_version_name()
        .and_then(xpr_dex::games::game_for_router_version)
}

/// Router versions (built-in, then custom gens) with trainer data, for the
/// Trainers / Damage game toggle.
pub fn router_versions(registry: &Arc<Registry>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for g in xpr_dex::GAMES {
        for v in xpr_dex::games::router_versions(g) {
            out.push(v.to_string());
        }
    }
    let mut custom: Vec<String> = registry.get_gen_names(false, true);
    custom.sort();
    out.extend(custom);
    out
}
