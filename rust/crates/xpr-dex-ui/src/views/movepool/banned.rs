//! The "Define Banned Moves" dialog (Solodex `BannedMovesModal.tsx`): the
//! user's globally banned, conditionally banned and per-game postgame move
//! lists, on top of the built-in lists of `unobtainable_moves`.

use egui::{Align2, Color32, CornerRadius, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use xpr_dex::natures::{static_banned_moves, static_conditional_moves, static_postgame_moves};
use xpr_dex::{all_move_names, game_abbrev, UserBans, GAMES};
use xpr_ui_kit::modal::{behind_modal, popup_open};
use xpr_ui_kit::theme::Theme;

use super::paint::text_at;
use crate::palette;
use crate::widgets::{self, text_w};
use crate::DexCx;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BanTab {
    Banned,
    Conditional,
    PerGame,
}

impl BanTab {
    pub const ALL: [BanTab; 3] = [BanTab::Banned, BanTab::Conditional, BanTab::PerGame];

    pub fn label(self) -> &'static str {
        match self {
            BanTab::Banned => "Banned globally",
            BanTab::Conditional => "Conditional",
            BanTab::PerGame => "Per-game",
        }
    }
}

/// The dialog's own state (reset each time it opens).
pub struct BannedState {
    pub opened: bool,
    pub tab: BanTab,
    pub game: String,
    pub query: String,
    pub highlight: usize,
    focus: bool,
}

impl Default for BannedState {
    fn default() -> Self {
        BannedState {
            opened: false,
            tab: BanTab::Banned,
            game: GAMES[0].to_string(),
            query: String::new(),
            highlight: 0,
            focus: true,
        }
    }
}

impl BannedState {
    /// Start a fresh session on `game` (Solodex `initialGame={selectedGame}`).
    pub fn open(&mut self, game: &str) {
        *self = BannedState::default();
        self.opened = true;
        if GAMES.contains(&game) {
            self.game = game.to_string();
        }
    }
}

/// The user's list the tab edits.
pub fn user_list<'a>(bans: &'a UserBans, tab: BanTab, game: &str) -> &'a [String] {
    match tab {
        BanTab::Banned => &bans.banned,
        BanTab::Conditional => &bans.conditional,
        BanTab::PerGame => bans.by_game.get(game).map(|v| v.as_slice()).unwrap_or(&[]),
    }
}

/// The built-in list of the tab.
pub fn static_list(tab: BanTab, game: &str) -> Vec<String> {
    match tab {
        BanTab::Banned => static_banned_moves(),
        BanTab::Conditional => static_conditional_moves(),
        BanTab::PerGame => static_postgame_moves(game),
    }
}

/// `bans` with `name` added to the tab's list (sorted); unchanged when it is already there.
pub fn with_move(bans: &UserBans, tab: BanTab, game: &str, name: &str) -> UserBans {
    let mut next = bans.clone();
    let add = |list: &mut Vec<String>| {
        if !list.iter().any(|m| m == name) {
            list.push(name.to_string());
            list.sort();
        }
    };
    match tab {
        BanTab::Banned => add(&mut next.banned),
        BanTab::Conditional => add(&mut next.conditional),
        BanTab::PerGame => add(next.by_game.entry(game.to_string()).or_default()),
    }
    next
}

/// `bans` without `name` in the tab's list (an emptied per-game list is dropped).
pub fn without_move(bans: &UserBans, tab: BanTab, game: &str, name: &str) -> UserBans {
    let mut next = bans.clone();
    match tab {
        BanTab::Banned => next.banned.retain(|m| m != name),
        BanTab::Conditional => next.conditional.retain(|m| m != name),
        BanTab::PerGame => {
            let cur: Vec<String> = bans
                .by_game
                .get(game)
                .map(|v| v.iter().filter(|m| *m != name).cloned().collect())
                .unwrap_or_default();
            if cur.is_empty() {
                next.by_game.remove(game);
            } else {
                next.by_game.insert(game.to_string(), cur);
            }
        }
    }
    next
}

/// Up to eight moves containing the query (case-insensitive) that are not in the user's list yet.
pub fn suggestions(query: &str, user: &[String]) -> Vec<String> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    all_move_names()
        .iter()
        .filter(|m| m.to_lowercase().contains(&q) && !user.contains(m))
        .take(8)
        .cloned()
        .collect()
}

fn description(tab: BanTab, game: &str) -> String {
    match tab {
        BanTab::Banned => "These moves are crossed out across every game when \"Cross-out banned moves\" is on.".to_string(),
        BanTab::Conditional => "A separate global list, crossed out when \"Cross-out conditionally banned moves\" is on. Use it for moves you want to flag without lumping them in with hard bans.".to_string(),
        BanTab::PerGame => format!("Crossed out only in {} when \"Cross-out post-game moves\" is on.", game),
    }
}

fn tab_strip(ui: &mut Ui, theme: &Theme, state: &mut BannedState) {
    let font = palette::px_bold(theme, 12.0);
    let widths: Vec<f32> = BanTab::ALL
        .iter()
        .map(|t| text_w(ui, t.label(), &font) + 24.0)
        .collect();
    let total: f32 = widths.iter().sum();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(total, 28.0), Sense::hover());
    ui.painter().rect(
        rect,
        CornerRadius::same(4),
        palette::GRAY_900,
        Stroke::new(1.0_f32, palette::GRAY_700),
        StrokeKind::Inside,
    );
    let mut x = rect.min.x;
    for (t, w) in BanTab::ALL.iter().zip(&widths) {
        let r = Rect::from_min_size(Pos2::new(x, rect.min.y), Vec2::new(*w, rect.height()));
        let resp = ui.interact(r, Id::new(("banned_tab", *t as u8)), Sense::click());
        super::probe::record("tab", "", *t as usize, t.label(), r);
        let active = state.tab == *t;
        if active {
            let radius = CornerRadius {
                nw: if *t == BanTab::Banned { 4 } else { 0 },
                sw: if *t == BanTab::Banned { 4 } else { 0 },
                ne: if *t == BanTab::PerGame { 4 } else { 0 },
                se: if *t == BanTab::PerGame { 4 } else { 0 },
            };
            ui.painter().rect_filled(r, radius, palette::BLUE_600);
        } else if resp.hovered() {
            ui.painter()
                .rect_filled(r.shrink(1.0), CornerRadius::ZERO, palette::GRAY_700);
        }
        ui.painter().text(
            r.center(),
            Align2::CENTER_CENTER,
            t.label(),
            font.clone(),
            if active {
                Color32::WHITE
            } else {
                palette::GRAY_300
            },
        );
        if resp
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
            && !active
        {
            state.tab = *t;
            state.query.clear();
            state.highlight = 0;
            state.focus = true;
        }
        x += w;
    }
}

/// A rounded chip with a name and an optional red-on-hover × (returns true when the × was clicked).
fn chip(ui: &mut Ui, theme: &Theme, name: &str, removable: bool) -> bool {
    let font = palette::px(theme, if removable { 14.0 } else { 12.0 });
    let tw = text_w(ui, name, &font);
    let xw = if removable {
        6.0 + text_w(ui, "\u{d7}", &palette::px(theme, 12.0))
    } else {
        0.0
    };
    let h = if removable { 28.0 } else { 24.0 };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(tw + xw + 16.0, h), Sense::click());
    let border = if removable {
        palette::GRAY_700
    } else {
        palette::GRAY_800
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(4),
        palette::GRAY_900,
        Stroke::new(1.0_f32, border),
        StrokeKind::Inside,
    );
    let color = if removable {
        palette::GRAY_200
    } else {
        palette::GRAY_500
    };
    ui.painter().text(
        Pos2::new(rect.min.x + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        name,
        font,
        color,
    );
    if !removable {
        resp.on_hover_text("Built-in entry \u{2014} cannot be removed");
        return false;
    }
    let xr = Rect::from_min_max(
        Pos2::new(rect.max.x - xw - 8.0 + 6.0 - 4.0, rect.min.y),
        rect.max,
    );
    let x_resp = ui.interact(xr, resp.id.with("remove"), Sense::click());
    super::probe::record("remove", "", 0, name, xr);
    let xc = if x_resp.hovered() {
        palette::RED_400
    } else {
        palette::GRAY_500
    };
    ui.painter().text(
        Pos2::new(rect.max.x - 8.0, rect.center().y),
        Align2::RIGHT_CENTER,
        "\u{d7}",
        palette::px(theme, 12.0),
        xc,
    );
    x_resp
        .on_hover_text("Remove")
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

/// Draw the dialog; closes it (`cx.state.banned_editor_open = false`) on the
/// Close button, the × or Escape.
pub fn show(ctx: &egui::Context, cx: &mut DexCx, state: &mut BannedState) {
    if !state.opened {
        let game = cx.state.game().to_string();
        state.open(&game);
    }
    let theme = cx.theme;
    let mut close = false;
    let mut next_bans: Option<UserBans> = None;
    let bans = cx.state.settings.user_bans.clone();
    widgets::modal(
        ctx,
        theme,
        "dex_banned_moves",
        "Define Banned Moves",
        640.0,
        |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
            // Escape closes, also from the search field (Solodex listens in the capture phase)
            if !behind_modal(ui) && !popup_open(ui.ctx()) && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                close = true;
            }
            tab_strip(ui, theme, state);
            if state.tab == BanTab::PerGame {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("GAME")
                            .font(palette::px(theme, 12.0))
                            .color(palette::GRAY_400),
                    );
                    let options: Vec<String> = GAMES
                        .iter()
                        .map(|g| format!("{} ({})", g, game_abbrev(g)))
                        .collect();
                    let mut current = format!("{} ({})", state.game, game_abbrev(&state.game));
                    let combo_at = ui.cursor().min;
                    super::probe::record("game", "", 0, "combo", Rect::from_min_size(combo_at, Vec2::new(300.0, 22.0)));
                    if xpr_ui_kit::widgets::option_menu(
                        ui,
                        theme,
                        Id::new("banned_game"),
                        &mut current,
                        &options,
                        Some(300.0),
                        true,
                    ) {
                        if let Some(i) = options.iter().position(|o| *o == current) {
                            state.game = GAMES[i].to_string();
                            state.highlight = 0;
                        }
                    }
                });
            }
            ui.add(
                egui::Label::new(
                    egui::RichText::new(description(state.tab, &state.game))
                        .font(palette::px(theme, 12.0))
                        .color(palette::GRAY_500),
                )
                .wrap(),
            );
            widgets::hairline(ui, palette::GRAY_700);

            // search
            let user = user_list(&bans, state.tab, &state.game).to_vec();
            let statics = static_list(state.tab, &state.game);
            let field_id = Id::new("banned_query");
            let before = state.query.clone();
            let er = xpr_ui_kit::widgets::Entry::new(theme, &mut state.query)
                .width(ui.available_width())
                .hint("Search a move to ban\u{2026}")
                .id(field_id)
                .clearable()
                .min_height(32.0)
                .font(palette::px(theme, 14.0))
                .show(ui);
            if state.focus {
                ui.memory_mut(|m| m.request_focus(field_id));
                state.focus = false;
            }
            if state.query != before {
                state.highlight = 0;
            }
            let sugg = suggestions(&state.query, &user);
            let mut pick: Option<String> = None;
            if er.has_focus || er.enter_pressed {
                let (down, up) = ui.input(|i| {
                    (
                        i.key_pressed(egui::Key::ArrowDown),
                        i.key_pressed(egui::Key::ArrowUp),
                    )
                });
                if down && !sugg.is_empty() {
                    state.highlight = (state.highlight + 1).min(sugg.len() - 1);
                }
                if up {
                    state.highlight = state.highlight.saturating_sub(1);
                }
                if er.enter_pressed {
                    let q = state.query.trim();
                    if let Some(p) = sugg.get(state.highlight) {
                        pick = Some(p.clone());
                    } else if !q.is_empty() && all_move_names().iter().any(|m| m == q) {
                        pick = Some(q.to_string());
                    }
                }
            }
            if !sugg.is_empty() {
                let row_h = 28.0;
                let list_w = ui.available_width();
                ui.spacing_mut().item_spacing.y = 0.0;
                let (lrect, _) = ui.allocate_exact_size(
                    Vec2::new(list_w, row_h * sugg.len() as f32 + 2.0),
                    Sense::hover(),
                );
                ui.painter().rect(
                    lrect,
                    CornerRadius::same(4),
                    palette::GRAY_900,
                    Stroke::new(1.0_f32, palette::GRAY_700),
                    StrokeKind::Inside,
                );
                for (i, m) in sugg.iter().enumerate() {
                    let r = Rect::from_min_size(
                        Pos2::new(lrect.min.x + 1.0, lrect.min.y + 1.0 + i as f32 * row_h),
                        Vec2::new(list_w - 2.0, row_h),
                    );
                    let resp = ui.interact(r, Id::new(("banned_sugg", i)), Sense::click());
                    if resp.hovered() {
                        state.highlight = i;
                    }
                    if i == state.highlight {
                        ui.painter().rect_filled(
                            r,
                            CornerRadius::same(3),
                            crate::palette::BLUE_600.gamma_multiply(0.3),
                        );
                    }
                    let c = if i == state.highlight {
                        Color32::WHITE
                    } else {
                        palette::GRAY_200
                    };
                    text_at(
                        ui,
                        Pos2::new(r.min.x + 12.0, r.center().y),
                        Align2::LEFT_CENTER,
                        m,
                        palette::px(theme, 14.0),
                        c,
                        false,
                    );
                    if statics.contains(m) {
                        ui.painter().text(
                            Pos2::new(r.max.x - 12.0, r.center().y),
                            Align2::RIGHT_CENTER,
                            "ALREADY BUILT-IN",
                            palette::px(theme, 10.0),
                            palette::GRAY_500,
                        );
                    }
                    if resp
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        pick = Some(m.clone());
                    }
                }
                ui.spacing_mut().item_spacing.y = 8.0;
            }
            if let Some(p) = pick {
                if !user.contains(&p) {
                    next_bans = Some(with_move(&bans, state.tab, &state.game, &p));
                }
                state.query.clear();
                state.highlight = 0;
                state.focus = true;
            }
            widgets::hairline(ui, palette::GRAY_700);

            // the lists
            egui::ScrollArea::vertical()
                .id_salt("banned_lists")
                .max_height(300.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                    ui.label(
                        egui::RichText::new(format!("YOUR BANS ({})", user.len()))
                            .font(palette::px_bold(theme, 12.0))
                            .color(palette::GRAY_500),
                    );
                    if user.is_empty() {
                        ui.label(
                            egui::RichText::new("No moves added yet.")
                                .font(palette::px(theme, 12.0))
                                .color(palette::GRAY_600),
                        );
                    } else {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                            for m in &user {
                                if chip(ui, theme, m, true) {
                                    next_bans =
                                        Some(without_move(&bans, state.tab, &state.game, m));
                                }
                            }
                        });
                    }
                    if !statics.is_empty() {
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(format!("BUILT-IN ({})", statics.len()))
                                .font(palette::px_bold(theme, 12.0))
                                .color(palette::GRAY_500),
                        );
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                            for m in &statics {
                                chip(ui, theme, m, false);
                            }
                        });
                    }
                });
            widgets::hairline(ui, palette::GRAY_700);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::small_button(ui, theme, "Close").clicked() {
                        close = true;
                    }
                });
            });
        },
    );
    if let Some(n) = next_bans {
        cx.state.settings.user_bans = n;
        cx.state.touch();
    }
    if close {
        state.opened = false;
        cx.state.banned_editor_open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_remove_keep_lists_sorted_and_tidy() {
        let bans = UserBans::default();
        let b = with_move(&bans, BanTab::Banned, "Emerald", "Surf");
        let b = with_move(&b, BanTab::Banned, "Emerald", "Dig");
        assert_eq!(b.banned, vec!["Dig", "Surf"]);
        // adding twice changes nothing
        assert_eq!(with_move(&b, BanTab::Banned, "Emerald", "Dig"), b);
        let b = with_move(&b, BanTab::PerGame, "Emerald", "Fly");
        assert_eq!(b.by_game["Emerald"], vec!["Fly"]);
        // removing the last per-game move drops the game's entry
        let b = without_move(&b, BanTab::PerGame, "Emerald", "Fly");
        assert!(!b.by_game.contains_key("Emerald"));
        assert_eq!(
            without_move(&b, BanTab::Banned, "Emerald", "Dig").banned,
            vec!["Surf"]
        );
    }

    #[test]
    fn suggestions_filter_and_cap() {
        let s = suggestions("thunder", &["Thunder".to_string()]);
        assert!(!s.is_empty() && s.len() <= 8);
        assert!(s
            .iter()
            .all(|m| m.to_lowercase().contains("thunder") && m != "Thunder"));
        assert!(suggestions("  ", &[]).is_empty());
    }
}
