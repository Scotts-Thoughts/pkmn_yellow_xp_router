//! The species right-click menu (Solodex `PokemonContextMenu.tsx`): Compare
//! / Add to comparison / Compare across generations. Used by the species
//! list, the ranking popovers, the evolution family and (later) the
//! comparison views.

use egui::{Align2, CornerRadius, Pos2, Response, Sense, Ui, Vec2};
use xpr_ui_kit::theme::Theme;

use crate::palette;
use crate::state::DexState;
use crate::widgets;
use crate::DexCx;

/// What the menu's enable rules need to know about the page.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MenuCtx {
    /// the selected (left) species
    pub selected: Option<String>,
    /// the game being viewed
    pub game: String,
    /// the species being compared with, if any
    pub comparing_with: Option<String>,
}

impl MenuCtx {
    /// The page's current selection, game and comparison partner.
    pub fn of(state: &DexState) -> MenuCtx {
        MenuCtx {
            selected: state.selected().map(str::to_string),
            game: state.game().to_string(),
            comparing_with: state.settings.comparing_with.clone(),
        }
    }
}

/// Which entries of the menu for `target` are enabled: (compare, add to
/// comparison, compare across generations). Solodex's rules.
pub fn enabled_entries(target: &str, mc: &MenuCtx) -> (bool, bool, bool) {
    let target_games = xpr_dex::get_games_for_pokemon(target);
    let selected_games = mc
        .selected
        .as_deref()
        .map(xpr_dex::get_games_for_pokemon)
        .unwrap_or_default();
    let has = |games: &[String]| games.iter().any(|g| *g == mc.game);
    let is_self = mc.selected.as_deref() == Some(target);
    let compare =
        !(is_self || mc.selected.is_none() || !has(&target_games) || !has(&selected_games));
    let triple = !(mc.comparing_with.is_none()
        || is_self
        || mc.comparing_with.as_deref() == Some(target)
        || !has(&target_games));
    let self_compare = target_games.len() > 1;
    (compare, triple, self_compare)
}

/// One entry of the menu (`px-3 py-1.5 text-sm`, hover gray-700, disabled
/// gray-600) `width` wide; true when clicked.
fn entry(ui: &mut Ui, theme: &Theme, text: &str, enabled: bool, width: f32) -> bool {
    let font = palette::px(theme, 14.0);
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(width, 28.0),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    if enabled && resp.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(3), palette::GRAY_700);
    }
    ui.painter().text(
        Pos2::new(rect.min.x + 12.0, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        font,
        if enabled {
            palette::GRAY_200
        } else {
            palette::GRAY_600
        },
    );
    enabled
        && resp
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
}

/// Attach the species menu for `target` to `resp` (a right click opens it).
pub fn pokemon_context_menu(cx: &mut DexCx, resp: &Response, target: &str, mc: &MenuCtx) {
    let DexCx { theme, state, .. } = cx;
    let theme = *theme;
    widgets::context_menu(theme, resp, |ui: &mut Ui| {
        let (compare, triple, self_compare) = enabled_entries(target, mc);
        let target_name = xpr_dex::display_name(target);
        let selected_name = mc
            .selected
            .as_deref()
            .map(xpr_dex::display_name)
            .unwrap_or("\u{2026}");
        let labels = [
            format!("Compare {} to {}", selected_name, target_name),
            format!("Add {} to comparison", target_name),
            format!("Compare {} across generations", target_name),
        ];
        // `min-w-[220px]`, as wide as the longest label
        let font = palette::px(theme, 14.0);
        let width = labels
            .iter()
            .map(|l| widgets::text_w(ui, l, &font) + 24.0)
            .fold(220.0_f32, f32::max);
        if entry(ui, theme, &labels[0], compare, width) {
            state.compare_with(target);
            ui.close();
        }
        if entry(ui, theme, &labels[1], triple, width) {
            state.triple_compare(target);
            ui.close();
        }
        if entry(ui, theme, &labels[2], self_compare, width) {
            state.self_compare(Some(target));
            ui.close();
        }
    });
}
