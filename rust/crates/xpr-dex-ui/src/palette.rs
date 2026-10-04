//! Colours. Solodex is styled with Tailwind's dark grays; the Dex keeps its
//! semantic colours (types, games, stats, categories, tabs) and maps the
//! grays onto the router's theme where a surface is involved, so the page
//! sits in the router's look.

use egui::Color32;
use xpr_ui_kit::theme::{self, Theme};

/// `#rrggbb` -> colour (gray for anything unparsable).
pub fn hex(s: &str) -> Color32 {
    theme::parse_hex(s)
}

// Tailwind grays (Solodex uses them for text and borders).
// Solodex's grays: its tailwind.config.js replaces Tailwind's gray scale
// with a navy one ("Scheduler navy palette").
pub const GRAY_100: Color32 = Color32::from_rgb(0xe2, 0xe8, 0xed);
pub const GRAY_200: Color32 = Color32::from_rgb(0xc9, 0xd3, 0xdb);
pub const GRAY_300: Color32 = Color32::from_rgb(0xa8, 0xb6, 0xc2);
pub const GRAY_400: Color32 = Color32::from_rgb(0x8b, 0x96, 0xa5);
pub const GRAY_500: Color32 = Color32::from_rgb(0x6e, 0x7a, 0x87);
pub const GRAY_600: Color32 = Color32::from_rgb(0x5c, 0x64, 0x70);
pub const GRAY_700: Color32 = Color32::from_rgb(0x24, 0x2b, 0x38);
pub const GRAY_800: Color32 = Color32::from_rgb(0x1a, 0x1f, 0x29);
pub const GRAY_900: Color32 = Color32::from_rgb(0x0f, 0x14, 0x19);

pub const RED_400: Color32 = Color32::from_rgb(0xf8, 0x71, 0x71);
pub const RED_500: Color32 = Color32::from_rgb(0xef, 0x44, 0x44);
pub const RED_600: Color32 = Color32::from_rgb(0xdc, 0x26, 0x26);
pub const GREEN_400: Color32 = Color32::from_rgb(0x4a, 0xde, 0x80);
pub const GREEN_600: Color32 = Color32::from_rgb(0x16, 0xa3, 0x4a);
// Solodex's blue-400 / blue-500 overrides
pub const BLUE_400: Color32 = Color32::from_rgb(0x1d, 0x9b, 0xf0);
pub const BLUE_500: Color32 = Color32::from_rgb(0x1a, 0x8f, 0xd9);
pub const BLUE_600: Color32 = Color32::from_rgb(0x25, 0x63, 0xeb);
pub const YELLOW_400: Color32 = Color32::from_rgb(0xfa, 0xcc, 0x15);
pub const YELLOW_500: Color32 = Color32::from_rgb(0xea, 0xb3, 0x08);
pub const AMBER_400: Color32 = Color32::from_rgb(0xfb, 0xbf, 0x24);
pub const AMBER_500: Color32 = Color32::from_rgb(0xf5, 0x9e, 0x0b);
pub const ORANGE_400: Color32 = Color32::from_rgb(0xfb, 0x92, 0x3c);
pub const TEAL_400: Color32 = Color32::from_rgb(0x2d, 0xd4, 0xbf);
pub const TEAL_500: Color32 = Color32::from_rgb(0x14, 0xb8, 0xa6);
pub const PURPLE_400: Color32 = Color32::from_rgb(0xc0, 0x84, 0xfc);
pub const VIOLET_400: Color32 = Color32::from_rgb(0xa7, 0x8b, 0xfa);
pub const VIOLET_600: Color32 = Color32::from_rgb(0x7c, 0x3a, 0xed);
pub const SLATE_400: Color32 = Color32::from_rgb(0x94, 0xa3, 0xb8);
pub const SKY_400: Color32 = Color32::from_rgb(0x29, 0xb6, 0xf6);

/// Type colours (Solodex `TYPE_COLORS`).
pub fn type_color(t: &str) -> Color32 {
    let h = match t {
        "Normal" => "#9fa19f",
        "Fighting" => "#ff8000",
        "Grass" => "#3fa129",
        "Fire" => "#e62829",
        "Water" => "#2980ef",
        "Electric" => "#fac000",
        "Ground" => "#915121",
        "Rock" => "#afa981",
        "Psychic" => "#ef4179",
        "Poison" => "#9141cb",
        "Flying" => "#81b9ef",
        "Bug" => "#91a119",
        "Ice" => "#3dcef3",
        "Ghost" => "#704170",
        "Dragon" => "#5060e1",
        "Steel" => "#60a1b8",
        "Dark" => "#624d4e",
        "Fairy" => "#ef70ef",
        "Curse Type" | "Mystery" | "Unknown" | "???" => "#68A090",
        _ => "#6B7280",
    };
    hex(h)
}

/// "???" for the curse / mystery type.
pub fn type_label(t: &str) -> &str {
    match t {
        "Curse Type" | "Mystery" | "Unknown" => "???",
        other => other,
    }
}

/// Damage-category colour (Physical orange, Special blue, Status gray).
pub fn category_color(category: &str) -> Color32 {
    match category {
        "Physical" => ORANGE_400,
        "Special" => BLUE_400,
        _ => GRAY_400,
    }
}

/// Stat bar colours (Solodex `STAT_CONFIG`).
pub fn stat_color(key: xpr_dex::StatKey, gen1: bool) -> Color32 {
    use xpr_dex::StatKey::*;
    hex(match key {
        Hp => "#69dc12",
        Attack => "#efcc18",
        Defense => "#e86412",
        SpecialAttack if gen1 => "#56b2a2",
        SpecialAttack => "#14c3f1",
        SpecialDefense => "#4a6adf",
        Speed => "#d51dad",
    })
}

/// Stat label (Spc for gen 1's Special).
pub fn stat_label(key: xpr_dex::StatKey, gen1: bool) -> &'static str {
    use xpr_dex::StatKey::*;
    match key {
        Hp => "HP",
        Attack => "Atk",
        Defense => "Def",
        SpecialAttack if gen1 => "Spc",
        SpecialAttack => "SpA",
        SpecialDefense => "SpD",
        Speed => "Spe",
    }
}

/// The stats shown for a game (gen 1 has one Special).
pub fn stat_keys(gen1: bool) -> &'static [xpr_dex::StatKey] {
    use xpr_dex::StatKey::*;
    if gen1 {
        &[Hp, Attack, Defense, SpecialAttack, Speed]
    } else {
        &[Hp, Attack, Defense, SpecialAttack, SpecialDefense, Speed]
    }
}

/// Gen 1 Weighted / Useful Base Stat Total rows.
pub const WBST_COLOR: Color32 = Color32::from_rgb(0x56, 0xb2, 0xa2);
pub const UBST_COLOR: Color32 = Color32::from_rgb(0xa7, 0x8b, 0xfa);

pub fn game_color(game: &str) -> Color32 {
    hex(xpr_dex::game_color(game))
}

/// Trainer-name colour by the router's fight category (Solodex
/// `trainerNameColor`: champion / boss teal, rival blue, Elite Four purple,
/// other major fights gold).
pub fn trainer_category_color(category: Option<&str>) -> Color32 {
    match category {
        Some("champion") | Some("boss") => TEAL_400,
        Some("rival") => BLUE_400,
        Some("elite_four") => PURPLE_400,
        Some(_) => YELLOW_400,
        None => GRAY_200,
    }
}

/// Surfaces taken from the router theme.
/// Solodex's page background (`bg-gray-900`); the cards' contrast is
/// designed against it.
pub fn page_bg(_theme: &Theme) -> Color32 {
    GRAY_900
}

/// A side panel's background (Solodex `bg-gray-900`).
pub fn panel_bg(_theme: &Theme) -> Color32 {
    GRAY_900
}

pub fn border(_theme: &Theme) -> Color32 {
    GRAY_700
}

/// A hovered row (Solodex `hover:bg-gray-800`).
pub fn hover_bg(_theme: &Theme) -> Color32 {
    GRAY_800
}

/// The accent of the Dex's top-level tabs (Solodex's release-order colours).
pub fn tab_color(tab: crate::DexTab) -> Color32 {
    use crate::DexTab::*;
    match tab {
        Pokedex => RED_600,
        Evs => GREEN_600,
        Trainers => BLUE_600,
        Stats => TEAL_500,
        Damage => YELLOW_500,
        Movedex => AMBER_500,
        Natures => SLATE_400,
        Misc => VIOLET_600,
    }
}

/// The theme's font at a CSS pixel size, for sizes taken from Solodex's
/// Tailwind classes: `text-[10px]` 10, `text-xs` 12, `text-sm` 14,
/// `text-base` 16, `text-lg` 18, `text-xl` 20, `text-2xl` 24. (The theme's
/// own `font(points)` takes Qt points: 9 pt = 12 px.)
pub fn px(theme: &Theme, size: f32) -> egui::FontId {
    theme.font(size * 0.75)
}

pub fn px_bold(theme: &Theme, size: f32) -> egui::FontId {
    theme.font_bold(size * 0.75)
}
