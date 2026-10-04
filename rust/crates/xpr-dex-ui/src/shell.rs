//! The page frame (Solodex `App.tsx`): tab bar, game toggle, the tab
//! bodies, the Pokédex list / detail split, keyboard shortcuts and the
//! search overlays.

use std::sync::Arc;

use egui::{Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use xpr_data::Registry;
use xpr_dex::GEN_GROUPS;
use xpr_ui_kit::modal::{behind_modal, pointer_blocked};
use xpr_ui_kit::theme::Theme;

use crate::images::DexImages;
use crate::palette;
use crate::state::{
    dex_game_for_version, router_versions, DexAction, DexSettings, DexState, DexTab, RouteContext,
    Spotlight,
};
#[allow(unused_imports)]
use crate::views;
use crate::widgets::{self, SpotlightOutcome, SpotlightRow, SpotlightState, ToggleItem};
use crate::{DexCx, DexHost};

pub struct DexView {
    pub state: DexState,
    images: DexImages,
    spotlight: SpotlightState,
    #[cfg(feature = "pokedex")]
    pokedex: views::pokedex::PokedexView,
    #[cfg(feature = "movepool")]
    movepool: views::movepool::MovepoolView,
    #[cfg(feature = "compare")]
    compare: views::compare::CompareView,
    #[cfg(feature = "trainers")]
    trainers: views::trainers::TrainersView,
    #[cfg(feature = "evs")]
    evs: views::evs::EvsView,
    #[cfg(feature = "movedex")]
    movedex: views::movedex::MovedexView,
    #[cfg(feature = "stats")]
    stats: views::stats::StatsView,
    #[cfg(feature = "misc")]
    natures: views::natures::NaturesView,
    #[cfg(feature = "misc")]
    misc: views::misc::MiscView,
    list_drag: bool,
    /// the keyboard-shortcut list is open
    shortcuts_help: bool,
}

impl DexView {
    /// A Dex restored from the config's `dex_settings` (anything unreadable
    /// falls back to the defaults).
    pub fn new(settings: serde_json::Value) -> DexView {
        let settings: DexSettings = serde_json::from_value(settings).unwrap_or_default();
        let mut v = DexView {
            state: DexState::new(settings),
            images: DexImages::new(),
            spotlight: SpotlightState::default(),
            #[cfg(feature = "pokedex")]
            pokedex: views::pokedex::PokedexView::default(),
            #[cfg(feature = "movepool")]
            movepool: views::movepool::MovepoolView::default(),
            #[cfg(feature = "compare")]
            compare: views::compare::CompareView::default(),
            #[cfg(feature = "trainers")]
            trainers: views::trainers::TrainersView::default(),
            #[cfg(feature = "evs")]
            evs: views::evs::EvsView::default(),
            #[cfg(feature = "movedex")]
            movedex: views::movedex::MovedexView::default(),
            #[cfg(feature = "stats")]
            stats: views::stats::StatsView::default(),
            #[cfg(feature = "misc")]
            natures: views::natures::NaturesView::default(),
            #[cfg(feature = "misc")]
            misc: views::misc::MiscView::default(),
            list_drag: false,
            shortcuts_help: false,
        };
        v.sanitize();
        v
    }

    /// Repair settings that name games / species that no longer exist.
    fn sanitize(&mut self) {
        let s = &mut self.state.settings;
        if xpr_dex::games::canonical_game(&s.game).is_none() {
            s.game = String::new();
        }
        if let Some(sel) = &s.selected {
            if xpr_dex::species_entry(sel).is_none() {
                s.selected = None;
            }
        }
        for c in [&mut s.comparing_with, &mut s.comparing_third] {
            if c.as_deref()
                .map(|n| xpr_dex::species_entry(n).is_none())
                .unwrap_or(false)
            {
                *c = None;
            }
        }
        if s.selected.is_none() {
            s.selected = xpr_dex::get_all_pokemon().first().map(|e| e.name.clone());
        }
        if s.game.is_empty() {
            s.game = s
                .selected
                .as_deref()
                .and_then(|n| xpr_dex::get_games_for_pokemon(n).into_iter().next())
                .unwrap_or_else(|| xpr_dex::GAMES[0].to_string());
        }
        s.list_width = s.list_width.clamp(180.0, 600.0);
    }

    /// The settings to persist (`Config::set_dex_settings`).
    pub fn settings_json(&self) -> serde_json::Value {
        serde_json::to_value(&self.state.settings).unwrap_or(serde_json::Value::Null)
    }

    /// Whether the settings changed since the last call.
    pub fn take_settings_dirty(&mut self) -> bool {
        self.state.take_settings_dirty()
    }

    /// Called when the page is opened from a route: start on the route's
    /// game (and solo Pokémon) the first time, or whenever the route changed.
    pub fn sync_to_route(&mut self, route: &RouteContext, registry: &Arc<Registry>) {
        let Some(version) = route.version.as_deref() else {
            return;
        };
        if let Some(game) = dex_game_for_version(registry, version) {
            self.state.set_game(game);
        }
        if self.state.settings.version.as_deref() != Some(version) {
            self.state.settings.version = Some(version.to_string());
            self.state.touch();
        }
        if let Some(species) = route
            .solo_species
            .as_deref()
            .and_then(xpr_dex::resolve_species)
        {
            if self.state.selected() != Some(species)
                && self.state.settings.comparing_with.is_none()
                && !self.state.settings.self_compare
            {
                self.state.select_species(species);
            }
        }
    }

    pub fn tab(&self) -> DexTab {
        self.state.tab()
    }

    /// Switch tabs (Solodex `handleViewModeChange`).
    pub fn set_tab(&mut self, tab: DexTab, registry: &Arc<Registry>) {
        let st = &mut self.state;
        st.settings.tab = tab;
        if tab != DexTab::Movedex {
            st.focused_move = None;
        }
        if tab.uses_router_versions() {
            let versions = router_versions(registry);
            let current_ok = st
                .settings
                .version
                .as_deref()
                .map(|v| versions.iter().any(|x| x == v))
                .unwrap_or(false);
            let matches_game = st
                .settings
                .version
                .as_deref()
                .and_then(|v| dex_game_for_version(registry, v))
                == Some(st.settings.game.as_str());
            if !current_ok || !matches_game {
                // the current Dex game's first version, else the last game with trainers
                let pick = xpr_dex::games::router_versions(&st.settings.game)
                    .first()
                    .map(|v| v.to_string())
                    .filter(|_| current_ok || !matches_game)
                    .or_else(|| st.settings.version.clone().filter(|_| current_ok))
                    .or_else(|| {
                        versions
                            .iter()
                            .rev()
                            .find(|v| xpr_dex::games::game_for_router_version(v).is_some())
                            .cloned()
                    });
                if let Some(v) = pick {
                    st.settings.version = Some(v);
                }
            }
        }
        st.touch();
    }

    pub fn open_spotlight(&mut self, which: Spotlight) {
        self.state.spotlight = Some(which);
        self.spotlight.reset();
    }

    /// Open the banned-moves editor (the router's Dex menu).
    pub fn open_banned_moves_editor(&mut self) {
        self.state.banned_editor_open = true;
    }

    /// Show the list of the Dex's keyboard shortcuts (the router's Dex menu).
    pub fn open_shortcuts_help(&mut self) {
        self.shortcuts_help = true;
    }

    /// Draw the page; returns what the host should do.
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        theme: &Theme,
        registry: &Arc<Registry>,
        route: &RouteContext,
        host: &mut dyn DexHost,
    ) -> Vec<DexAction> {
        let mut actions = Vec::new();
        self.handle_keys(ui, registry);
        let full = ui.available_rect_before_wrap();
        ui.painter()
            .rect_filled(full, CornerRadius::ZERO, palette::page_bg(theme));
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
        self.header(ui, theme, registry, &mut actions);
        let body = ui.available_rect_before_wrap();
        let mut bui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(body)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        bui.set_clip_rect(body.intersect(ui.clip_rect()));
        self.body(&mut bui, theme, registry, route, host, &mut actions);
        ui.allocate_rect(body, Sense::hover());
        self.overlays(ui.ctx(), theme, registry, route, &mut actions);
        actions
    }

    // ---- header -----------------------------------------------------------------------------

    fn header(
        &mut self,
        ui: &mut Ui,
        theme: &Theme,
        registry: &Arc<Registry>,
        actions: &mut Vec<DexAction>,
    ) {
        let width = ui.available_width();
        // tab bar, centred; Back on the left
        let (bar, _) = ui.allocate_exact_size(Vec2::new(width, 40.0), Sense::hover());
        let back = Rect::from_min_size(
            Pos2::new(bar.min.x + 8.0, bar.center().y - 13.0),
            Vec2::new(74.0, 26.0),
        );
        let back_resp = ui
            .interact(back, ui.id().with("dex_back"), Sense::click())
            .on_hover_text("Leave the Dex");
        ui.painter().rect(
            back,
            CornerRadius::same(4),
            if back_resp.hovered() {
                palette::GRAY_700
            } else {
                palette::GRAY_800
            },
            Stroke::new(1.0_f32, palette::GRAY_700),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            back.center(),
            Align2::CENTER_CENTER,
            "\u{25C0} Back",
            palette::px(theme, 12.0),
            palette::GRAY_300,
        );
        if back_resp.clicked() {
            actions.push(DexAction::Close);
        }
        let tab_w = 100.0;
        let tabs = DexTab::ALL;
        let total = tab_w * tabs.len() as f32;
        let strip = Rect::from_center_size(bar.center(), Vec2::new(total, 28.0));
        ui.painter().rect(
            strip,
            CornerRadius::same(4),
            palette::GRAY_800,
            Stroke::new(1.0_f32, palette::GRAY_700),
            egui::StrokeKind::Inside,
        );
        let mut clicked = None;
        for (i, tab) in tabs.iter().enumerate() {
            let r = Rect::from_min_size(
                Pos2::new(strip.min.x + i as f32 * tab_w, strip.min.y),
                Vec2::new(tab_w, strip.height()),
            );
            let resp = ui
                .interact(r, ui.id().with(("dex_tab", i)), Sense::click())
                .on_hover_text(tab.label());
            let active = self.state.tab() == *tab;
            if active {
                let radius = CornerRadius {
                    nw: if i == 0 { 4 } else { 0 },
                    sw: if i == 0 { 4 } else { 0 },
                    ne: if i == tabs.len() - 1 { 4 } else { 0 },
                    se: if i == tabs.len() - 1 { 4 } else { 0 },
                };
                ui.painter()
                    .rect_filled(r, radius, palette::tab_color(*tab));
            } else if resp.hovered() {
                ui.painter()
                    .rect_filled(r.shrink(1.0), CornerRadius::ZERO, palette::GRAY_700);
            }
            let fg = if active {
                if *tab == DexTab::Natures {
                    palette::GRAY_900
                } else {
                    Color32::WHITE
                }
            } else if resp.hovered() {
                Color32::WHITE
            } else {
                palette::GRAY_300
            };
            let key = format!("{:?}", tab.key());
            let label_font = palette::px_bold(theme, 12.0);
            let key_font = palette::px(theme, 12.0);
            let lw = widgets::text_w(ui, tab.label(), &label_font);
            let kw = widgets::text_w(ui, &format!(" [{}]", key), &key_font);
            let x0 = r.center().x - (lw + kw) / 2.0;
            ui.painter().text(
                Pos2::new(x0, r.center().y),
                Align2::LEFT_CENTER,
                tab.label(),
                label_font,
                fg,
            );
            let key_color = if active {
                Color32::from_white_alpha(150)
            } else {
                palette::GRAY_500
            };
            let key_color = if active && *tab == DexTab::Natures {
                Color32::from_black_alpha(150)
            } else {
                key_color
            };
            ui.painter().text(
                Pos2::new(x0 + lw, r.center().y),
                Align2::LEFT_CENTER,
                format!(" [{}]", key),
                key_font,
                key_color,
            );
            if resp.clicked() {
                clicked = Some(*tab);
            }
        }
        if let Some(t) = clicked {
            self.set_tab(t, registry);
        }
        // game toggle
        let tab = self.state.tab();
        if !tab.gameless() {
            self.game_toggle(ui, theme, registry);
        }
        let (line, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), Sense::hover());
        ui.painter().hline(
            line.x_range(),
            line.center().y,
            Stroke::new(1.0_f32, palette::GRAY_700),
        );
    }

    fn game_toggle(&mut self, ui: &mut Ui, theme: &Theme, registry: &Arc<Registry>) {
        let tab = self.state.tab();
        let st = &mut self.state;
        let mut items: Vec<ToggleItem> = Vec::new();
        let selected: String;
        if tab.uses_router_versions() {
            for v in router_versions(registry) {
                let game = dex_game_for_version(registry, &v);
                let color = game.map(palette::game_color).unwrap_or(palette::GRAY_500);
                items.push(ToggleItem {
                    key: v.clone(),
                    label: v.clone(),
                    color,
                    tooltip: game
                        .map(|g| format!("{} ({})", v, g))
                        .unwrap_or_else(|| v.clone()),
                });
            }
            selected = st.settings.version.clone().unwrap_or_default();
        } else {
            let per_game = matches!(tab, DexTab::Pokedex | DexTab::Stats);
            let games: Vec<String> = if tab == DexTab::Pokedex {
                st.selected()
                    .map(xpr_dex::get_games_for_pokemon)
                    .unwrap_or_default()
            } else {
                xpr_dex::GAMES.iter().map(|g| g.to_string()).collect()
            };
            if per_game {
                for g in &games {
                    items.push(ToggleItem {
                        key: g.clone(),
                        label: xpr_dex::game_abbrev(g).to_string(),
                        color: palette::game_color(g),
                        tooltip: g.clone(),
                    });
                }
                selected = st.game().to_string();
            } else {
                for grp in GEN_GROUPS
                    .iter()
                    .filter(|grp| grp.games.iter().any(|g| games.iter().any(|x| x == g)))
                {
                    let first = grp
                        .games
                        .iter()
                        .find(|g| games.iter().any(|x| x == *g))
                        .unwrap();
                    items.push(ToggleItem {
                        key: first.to_string(),
                        label: grp.label.to_string(),
                        color: palette::hex(grp.color),
                        tooltip: grp.games.join(", "),
                    });
                }
                // the active group is the one holding the current game
                selected = xpr_dex::games::gen_group_of(st.game())
                    .and_then(|grp| grp.games.iter().find(|g| games.iter().any(|x| x == *g)))
                    .map(|g| g.to_string())
                    .unwrap_or_default();
            }
        }
        let comparing = tab == DexTab::Pokedex
            && (st.settings.comparing_with.is_some() || st.settings.self_compare);
        let row_top = ui.cursor().min.y;
        let resp = ui.horizontal(|ui| {
            ui.add_space(8.0);
            let w = ui.available_width() - if comparing { 150.0 } else { 8.0 };
            ui.allocate_ui_with_layout(
                Vec2::new(w, 28.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| widgets::game_toggle(ui, theme, &items, &selected),
            )
            .inner
        });
        let r = resp.inner;
        if comparing {
            let full = ui.max_rect();
            let b = Rect::from_min_size(
                Pos2::new(full.max.x - 146.0, row_top + 3.0),
                Vec2::new(138.0, 22.0),
            );
            let br = ui.interact(b, ui.id().with("dex_exit_compare"), Sense::click());
            ui.painter().rect(
                b,
                CornerRadius::same(4),
                if br.hovered() {
                    palette::GRAY_700
                } else {
                    palette::GRAY_800
                },
                Stroke::new(1.0_f32, palette::GRAY_700),
                egui::StrokeKind::Inside,
            );
            ui.painter().text(
                b.center(),
                Align2::CENTER_CENTER,
                "Exit Comparison [Esc]",
                palette::px(theme, 12.0),
                if br.hovered() {
                    Color32::WHITE
                } else {
                    palette::GRAY_400
                },
            );
            if br.clicked() {
                if st.settings.comparing_with.is_some() {
                    st.exit_compare();
                } else {
                    st.exit_self_compare();
                }
            }
        }
        if let Some(k) = r.clicked {
            if tab.uses_router_versions() {
                st.set_version(&k, registry);
                if tab == DexTab::Trainers {
                    st.selected_trainer = None;
                }
            } else {
                st.set_game(&k);
            }
        }
        if let Some(k) = r.right_clicked {
            if tab == DexTab::Pokedex && st.selected().is_some() && k != st.game() {
                st.compare_games(&k);
            }
        }
    }

    // ---- body -------------------------------------------------------------------------------

    fn body(
        &mut self,
        ui: &mut Ui,
        theme: &Theme,
        registry: &Arc<Registry>,
        route: &RouteContext,
        host: &mut dyn DexHost,
        actions: &mut Vec<DexAction>,
    ) {
        let tab = self.state.tab();
        if tab == DexTab::Pokedex {
            self.pokedex_body(ui, theme, registry, route, actions);
            return;
        }
        let mut cx = DexCx {
            theme,
            registry,
            route,
            state: &mut self.state,
            images: &mut self.images,
            actions,
        };
        match tab {
            DexTab::Pokedex => {}
            DexTab::Damage => host.damage_ui(ui, &mut cx),
            DexTab::Trainers => {
                #[cfg(feature = "trainers")]
                self.trainers.ui(ui, &mut cx);
                #[cfg(not(feature = "trainers"))]
                widgets::placeholder(ui, theme, "Trainers are not built in.");
            }
            DexTab::Evs => {
                #[cfg(feature = "evs")]
                self.evs.ui(ui, &mut cx);
                #[cfg(not(feature = "evs"))]
                widgets::placeholder(ui, theme, "EVs are not built in.");
            }
            DexTab::Movedex => {
                #[cfg(feature = "movedex")]
                self.movedex.ui(ui, &mut cx);
                #[cfg(not(feature = "movedex"))]
                widgets::placeholder(ui, theme, "The Movedex is not built in.");
            }
            DexTab::Stats => {
                #[cfg(feature = "stats")]
                self.stats.ui(ui, &mut cx);
                #[cfg(not(feature = "stats"))]
                widgets::placeholder(ui, theme, "Stats are not built in.");
            }
            DexTab::Natures => {
                #[cfg(feature = "misc")]
                self.natures.ui(ui, &mut cx);
                #[cfg(not(feature = "misc"))]
                widgets::placeholder(ui, theme, "Natures are not built in.");
            }
            DexTab::Misc => {
                #[cfg(feature = "misc")]
                self.misc.ui(ui, &mut cx);
                #[cfg(not(feature = "misc"))]
                widgets::placeholder(ui, theme, "The calculators are not built in.");
            }
        }
        let _ = &mut cx;
    }

    /// Species list (resizable) on the left, detail or comparison on the right.
    #[allow(unused_variables)]
    fn pokedex_body(
        &mut self,
        ui: &mut Ui,
        theme: &Theme,
        registry: &Arc<Registry>,
        route: &RouteContext,
        actions: &mut Vec<DexAction>,
    ) {
        let full = ui.available_rect_before_wrap();
        let list_open = self.state.settings.list_open;
        let list_w = if list_open {
            self.state
                .settings
                .list_width
                .min(full.width() - 200.0)
                .max(180.0)
        } else {
            0.0
        };
        let list_rect = Rect::from_min_size(full.min, Vec2::new(list_w, full.height()));
        let handle = Rect::from_min_size(
            Pos2::new(full.min.x + list_w, full.min.y),
            Vec2::new(if list_open { 4.0 } else { 0.0 }, full.height()),
        );
        let right = Rect::from_min_max(Pos2::new(handle.max.x, full.min.y), full.max);
        if list_open {
            let mut lui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(list_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            lui.set_clip_rect(list_rect.intersect(ui.clip_rect()));
            let mut cx = DexCx {
                theme,
                registry,
                route,
                state: &mut self.state,
                images: &mut self.images,
                actions: &mut *actions,
            };
            #[cfg(feature = "pokedex")]
            self.pokedex.list_ui(&mut lui, &mut cx);
            let _ = &mut cx;
            #[cfg(not(feature = "pokedex"))]
            widgets::placeholder(&mut lui, theme, "The Pokédex list is not built in.");
            // splitter
            let resp = ui.interact(handle, ui.id().with("dex_list_splitter"), Sense::drag());
            let hot = resp.hovered() || resp.dragged();
            ui.painter().rect_filled(
                handle,
                CornerRadius::ZERO,
                if hot {
                    palette::BLUE_500
                } else {
                    palette::GRAY_700
                },
            );
            if hot {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
            if resp.dragged() {
                self.state.settings.list_width = (self.state.settings.list_width
                    + resp.drag_delta().x)
                    .clamp(180.0, (full.width() - 300.0).max(180.0));
                self.list_drag = true;
            } else if self.list_drag {
                self.list_drag = false;
                self.state.touch();
            }
        }
        let mut rui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(right)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        rui.set_clip_rect(right.intersect(ui.clip_rect()));
        // list show / hide toggle in the corner
        let tb = Rect::from_min_size(right.min + Vec2::new(8.0, 8.0), Vec2::new(22.0, 22.0));
        let s = &self.state.settings;
        let has_selection = s.selected.is_some();
        let comparing = s.comparing_with.is_some();
        let triple = comparing && s.comparing_third.is_some();
        let self_cmp = s.self_compare;
        {
            #[allow(unused_mut)]
            let mut cx = DexCx {
                theme,
                registry,
                route,
                state: &mut self.state,
                images: &mut self.images,
                actions: &mut *actions,
            };
            if !has_selection {
                widgets::placeholder(&mut rui, theme, "Select a Pok\u{e9}mon");
            } else if comparing || self_cmp {
                #[cfg(feature = "compare")]
                {
                    if triple {
                        self.compare
                            .triple_ui(&mut rui, &mut cx, &mut self.movepool);
                    } else if comparing {
                        self.compare.pair_ui(&mut rui, &mut cx, &mut self.movepool);
                    } else {
                        self.compare.self_ui(&mut rui, &mut cx, &mut self.movepool);
                    }
                }
                #[cfg(not(feature = "compare"))]
                widgets::placeholder(&mut rui, theme, "Comparisons are not built in.");
            } else {
                #[cfg(feature = "pokedex")]
                {
                    #[cfg(feature = "movepool")]
                    self.pokedex
                        .detail_ui(&mut rui, &mut cx, Some(&mut self.movepool));
                    #[cfg(not(feature = "movepool"))]
                    self.pokedex.detail_ui(&mut rui, &mut cx, None);
                }
                #[cfg(all(not(feature = "pokedex"), feature = "movepool"))]
                {
                    let sel = cx.state.selected().map(str::to_string).unwrap();
                    let game = cx.state.game().to_string();
                    if let Some(data) = xpr_dex::get_pokemon_data(&sel, &game) {
                        self.movepool.ui(&mut rui, &mut cx, &data, &game);
                    }
                }
                #[cfg(all(not(feature = "pokedex"), not(feature = "movepool")))]
                widgets::placeholder(&mut rui, theme, "The Pok\u{e9}mon detail is not built in.");
            }
        }
        let tr = ui
            .interact(tb, ui.id().with("dex_list_toggle"), Sense::click())
            .on_hover_text(if list_open { "Hide list" } else { "Show list" });
        ui.painter().rect_filled(
            tb,
            CornerRadius::same(4),
            if tr.hovered() {
                palette::GRAY_700
            } else {
                palette::GRAY_800
            },
        );
        ui.painter().text(
            tb.center(),
            Align2::CENTER_CENTER,
            if list_open { "\u{25C0}" } else { "\u{25B6}" },
            palette::px(theme, 12.0),
            if tr.hovered() {
                Color32::WHITE
            } else {
                palette::GRAY_400
            },
        );
        if tr.clicked() {
            self.state.settings.list_open = !list_open;
            self.state.touch();
        }
    }

    // ---- overlays ---------------------------------------------------------------------------

    #[allow(unused_variables)]
    fn overlays(
        &mut self,
        ctx: &egui::Context,
        theme: &Theme,
        registry: &Arc<Registry>,
        route: &RouteContext,
        actions: &mut Vec<DexAction>,
    ) {
        #[cfg(not(feature = "movepool"))]
        {
            self.state.banned_editor_open = false;
        }
        #[cfg(feature = "movepool")]
        if self.state.banned_editor_open {
            let mut cx = DexCx {
                theme,
                registry,
                route,
                state: &mut self.state,
                images: &mut self.images,
                actions: &mut *actions,
            };
            self.movepool.banned_moves_modal(ctx, &mut cx);
        }
        if self.shortcuts_help {
            self.shortcuts_help = shortcuts_help(ctx, theme);
        }
        let Some(which) = self.state.spotlight else {
            return;
        };
        let (rows, title, accent): (Vec<SpotlightRow>, &str, Color32) = match which {
            Spotlight::Pokemon | Spotlight::Compare => {
                let rows = pokemon_rows(&self.spotlight.query);
                // Solodex picks an exact name match as soon as it is typed
                let q = self.spotlight.query.trim().to_lowercase();
                if !q.is_empty() {
                    if let Some(exact) = rows.iter().find(|r| r.key.to_lowercase() == q) {
                        let name = exact.key.clone();
                        self.pick_species(which, &name);
                        return;
                    }
                }
                let title = if which == Spotlight::Compare {
                    "Compare with..."
                } else {
                    "Pok\u{e9}mon"
                };
                (rows, title, palette::RED_500)
            }
            Spotlight::Trainer => {
                #[cfg(feature = "trainers")]
                let rows = views::trainers::spotlight_rows(
                    &self.spotlight.query,
                    registry,
                    self.state.settings.version.as_deref(),
                );
                #[cfg(not(feature = "trainers"))]
                let rows = Vec::new();
                (rows, "Trainers", palette::BLUE_400)
            }
            Spotlight::Move => {
                #[cfg(feature = "movedex")]
                let rows = views::movedex::spotlight_rows(&self.spotlight.query);
                #[cfg(not(feature = "movedex"))]
                let rows = Vec::new();
                (rows, "Moves", palette::AMBER_500)
            }
        };
        let outcome = widgets::spotlight(
            ctx,
            theme,
            &mut self.images,
            "dex_spotlight",
            title,
            accent,
            &mut self.spotlight,
            &rows,
        );
        match outcome {
            SpotlightOutcome::Open => {}
            SpotlightOutcome::Closed => self.state.spotlight = None,
            SpotlightOutcome::Picked(key) => {
                match which {
                    Spotlight::Pokemon | Spotlight::Compare => self.pick_species(which, &key),
                    Spotlight::Trainer => {
                        // key = "<version>\u{1f}<trainer>"
                        if let Some((version, trainer)) = key.split_once('\u{1f}') {
                            self.state.settings.tab = DexTab::Trainers;
                            self.state.set_version(version, registry);
                            self.state.selected_trainer = Some(trainer.to_string());
                            self.state.touch();
                        }
                    }
                    Spotlight::Move => {
                        self.state.focused_move = Some(key);
                        self.state.settings.tab = DexTab::Movedex;
                        self.state.touch();
                    }
                }
                self.state.spotlight = None;
            }
        }
    }

    fn pick_species(&mut self, which: Spotlight, name: &str) {
        if which == Spotlight::Compare && self.state.selected().is_some() {
            self.state.compare_with(name);
        } else {
            self.state.select_species(name);
        }
        self.state.spotlight = None;
    }

    // ---- keys -------------------------------------------------------------------------------

    /// Solodex's keyboard handler. Stands down under a dialog or menu, while
    /// a search overlay is open, and (for plain keys) while a text field has
    /// focus.
    fn handle_keys(&mut self, ui: &Ui, registry: &Arc<Registry>) {
        if behind_modal(ui)
            || pointer_blocked(ui)
            || self.state.spotlight.is_some()
            || self.state.banned_editor_open
            || self.shortcuts_help
        {
            return;
        }
        let in_text = ui.ctx().memory(|m| m.focused().is_some()) && ui.ctx().wants_keyboard_input();
        let tab = self.state.tab();
        let pressed = |k: egui::Key, m: egui::Modifiers| ui.input_mut(|i| i.consume_key(m, k));
        let none = egui::Modifiers::NONE;
        if !in_text {
            for t in DexTab::ALL {
                if pressed(t.key(), none) {
                    self.set_tab(t, registry);
                    return;
                }
            }
        }
        if pressed(egui::Key::Space, egui::Modifiers::SHIFT) {
            self.open_spotlight(Spotlight::Trainer);
            return;
        }
        if pressed(
            egui::Key::Space,
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        ) {
            self.open_spotlight(Spotlight::Move);
            return;
        }
        if !in_text
            && tab == DexTab::Pokedex
            && self.state.selected().is_some()
            && pressed(egui::Key::Space, egui::Modifiers::COMMAND)
        {
            self.open_spotlight(Spotlight::Compare);
            return;
        }
        // Ctrl+1-9: cycle the selected species' games in that generation
        if !in_text {
            for (n, key) in [
                egui::Key::Num1,
                egui::Key::Num2,
                egui::Key::Num3,
                egui::Key::Num4,
                egui::Key::Num5,
                egui::Key::Num6,
                egui::Key::Num7,
                egui::Key::Num8,
                egui::Key::Num9,
            ]
            .iter()
            .enumerate()
            {
                if pressed(*key, egui::Modifiers::COMMAND) {
                    if let Some(sel) = self.state.selected().map(str::to_string) {
                        let available = xpr_dex::get_games_for_pokemon(&sel);
                        let in_gen: Vec<&str> = GEN_GROUPS[n]
                            .games
                            .iter()
                            .copied()
                            .filter(|g| available.iter().any(|a| a == g))
                            .collect();
                        if !in_gen.is_empty() {
                            let idx = in_gen.iter().position(|g| *g == self.state.game());
                            let next = in_gen[idx.map(|i| (i + 1) % in_gen.len()).unwrap_or(0)];
                            self.state.set_game(next);
                        }
                    }
                    return;
                }
            }
        }
        if in_text {
            return;
        }
        if pressed(egui::Key::Escape, none) {
            if self.state.settings.comparing_with.is_some() {
                self.state.exit_compare();
            } else if self.state.settings.self_compare {
                self.state.exit_self_compare();
            }
            return;
        }
        if pressed(egui::Key::ArrowLeft, none) {
            self.state.settings.list_open = false;
            self.state.touch();
        } else if pressed(egui::Key::ArrowRight, none) {
            self.state.settings.list_open = true;
            self.state.touch();
        } else if tab == DexTab::Pokedex
            && (ui.input(|i| {
                i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::ArrowDown)
            }))
        {
            let up = ui.input(|i| i.key_pressed(egui::Key::ArrowUp));
            let names: Vec<String> = if self.state.filtered_names.is_empty() {
                xpr_dex::get_all_pokemon()
                    .iter()
                    .map(|e| e.name.clone())
                    .collect()
            } else {
                self.state.filtered_names.clone()
            };
            if let Some(idx) = self
                .state
                .selected()
                .and_then(|s| names.iter().position(|n| n == s))
            {
                let next = if up {
                    idx.checked_sub(1)
                } else {
                    Some(idx + 1).filter(|i| *i < names.len())
                };
                if let Some(n) = next {
                    let name = names[n].clone();
                    self.state.select_species(&name);
                }
            }
        } else if tab != DexTab::Movedex && pressed(egui::Key::Space, none) {
            self.open_spotlight(Spotlight::Pokemon);
        }
    }
}

/// Pokémon search results (Solodex `SpotlightSearch` filter: name, display
/// name, or exact dex number).
pub fn pokemon_rows(query: &str) -> Vec<SpotlightRow> {
    let q = query.trim().to_lowercase();
    xpr_dex::get_all_pokemon()
        .iter()
        .filter(|p| {
            q.is_empty()
                || p.name.to_lowercase().contains(&q)
                || xpr_dex::display_name(&p.name).to_lowercase().contains(&q)
                || p.national_dex_number.to_string() == q
        })
        .map(|p| SpotlightRow {
            key: p.name.clone(),
            label: xpr_dex::display_name(&p.name).to_string(),
            detail: format!("#{:03}", p.national_dex_number),
            sprite: Some((p.name.clone(), p.national_dex_number)),
            color: None,
        })
        .collect()
}

/// The Dex's keys (Solodex's keyboard shortcuts, minus Route and Map).
/// Returns whether it stays open.
fn shortcuts_help(ctx: &egui::Context, theme: &Theme) -> bool {
    let rows: [(&str, &str); 16] = [
        ("F1", "Pokedex"),
        ("F2", "EVs"),
        ("F3", "Trainers"),
        ("F4", "Damage"),
        ("F5", "Movedex"),
        ("F6", "Natures"),
        ("F8", "Stats"),
        ("F9", "Misc"),
        ("Space", "Pok\u{e9}mon search"),
        ("Ctrl + Space", "Compare search (Pok\u{e9}dex)"),
        ("Shift + Space", "Trainer search"),
        ("Ctrl + Shift + Space", "Move search"),
        ("\u{2191} / \u{2193}", "Previous / next Pok\u{e9}mon in the list"),
        ("\u{2190} / \u{2192}", "Hide / show the list"),
        ("Esc", "Exit the comparison"),
        ("Ctrl + 1-9", "Cycle the games of that generation"),
    ];
    let mut open = true;
    widgets::modal(ctx, theme, "dex_shortcuts_help", "Dex Keyboard Shortcuts", 420.0, |ui| {
        egui::Grid::new("dex_shortcuts_grid").num_columns(2).spacing(Vec2::new(18.0, 6.0)).show(ui, |ui| {
            for (key, what) in rows {
                ui.label(egui::RichText::new(key).font(palette::px_bold(theme, 13.0)).color(palette::GRAY_200));
                ui.label(egui::RichText::new(what).font(palette::px(theme, 13.0)).color(palette::GRAY_400));
                ui.end_row();
            }
        });
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Ctrl + K opens and closes the Dex (File > Keyboard Shortcuts... rebinds it).").font(palette::px(theme, 12.0)).color(palette::GRAY_500));
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::small_button(ui, theme, "Close").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    open = false;
                }
            });
        });
    });
    open
}
