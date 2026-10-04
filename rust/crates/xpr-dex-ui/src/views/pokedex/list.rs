//! The species list (Solodex `PokemonList.tsx`): search, Gen / Type /
//! Growth / Stage filters, Regional / Megas / Forms toggles and the
//! virtualised rows with sprite, dex number, name and typing.

use egui::{Align2, Color32, CornerRadius, Id, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use xpr_dex::{classify_form, display_name, get_all_pokemon, EvolutionStage, PokemonListEntry};
use xpr_ui_kit::widgets::Entry;

use super::common;
use super::context_menu::{pokemon_context_menu, MenuCtx};
use crate::palette;
use crate::widgets;
use crate::DexCx;

const ALL_TYPES: [&str; 18] = [
    "Normal", "Fire", "Water", "Electric", "Grass", "Ice", "Fighting", "Poison", "Ground",
    "Flying", "Psychic", "Bug", "Rock", "Ghost", "Dragon", "Dark", "Steel", "Fairy",
];
const ALL_GROWTH_RATES: [&str; 6] = [
    "Erratic",
    "Fast",
    "Medium Fast",
    "Medium Slow",
    "Slow",
    "Fluctuating",
];
/// National dex ranges of the nine generations.
const GEN_RANGES: [(i32, i32); 9] = [
    (1, 151),
    (152, 251),
    (252, 386),
    (387, 493),
    (494, 649),
    (650, 721),
    (722, 809),
    (810, 905),
    (906, 1025),
];
const ALL_EVO_STAGES: [(&str, &str); 5] = [
    ("first", "First Stage"),
    ("middle", "Middle Stage"),
    ("final", "Final Stage"),
    ("single", "Single Stage"),
    ("mega", "Mega Evolution"),
];
/// Below this list width the typing column is dropped.
const SHOW_TYPES_MIN_WIDTH: f32 = 280.0;
/// 24 px sprite + 1 px bottom border.
const ROW_HEIGHT: f32 = 25.0;

fn stage_value(s: EvolutionStage) -> &'static str {
    match s {
        EvolutionStage::Single => "single",
        EvolutionStage::First => "first",
        EvolutionStage::Middle => "middle",
        EvolutionStage::Final => "final",
        EvolutionStage::Mega => "mega",
    }
}

/// The list's filters (Solodex persists them in localStorage; here they
/// live as long as the page does).
#[derive(Clone, Debug, PartialEq)]
pub struct Filters {
    pub query: String,
    /// "" or "1".."9"
    pub gen: String,
    pub type_: String,
    pub growth: String,
    pub stage: String,
    pub regional: bool,
    pub megas: bool,
    pub forms: bool,
}

impl Default for Filters {
    fn default() -> Self {
        Filters {
            query: String::new(),
            gen: String::new(),
            type_: String::new(),
            growth: String::new(),
            stage: String::new(),
            regional: true,
            megas: true,
            forms: true,
        }
    }
}

/// The typing of a species in a game, falling back to the index's.
fn types_of<'a>(
    game_types: &'a [Option<(String, String)>],
    i: usize,
    p: &'a PokemonListEntry,
) -> (&'a str, &'a str) {
    match game_types.get(i).and_then(|t| t.as_ref()) {
        Some((a, b)) => (a.as_str(), b.as_str()),
        None => (p.type_1.as_str(), p.type_2.as_str()),
    }
}

/// Whether `p` passes the filters (`PokemonList.tsx`'s `filtered`).
pub fn passes(f: &Filters, q: &str, p: &PokemonListEntry, types: (&str, &str)) -> bool {
    if !q.is_empty()
        && !(p.name.to_lowercase().contains(q)
            || display_name(&p.name).to_lowercase().contains(q)
            || p.national_dex_number.to_string() == q
            || types.0.to_lowercase().contains(q)
            || types.1.to_lowercase().contains(q))
    {
        return false;
    }
    let form = classify_form(&p.name);
    if let Ok(gen) = f.gen.parse::<usize>() {
        let Some((lo, hi)) = GEN_RANGES.get(gen.wrapping_sub(1)) else {
            return false;
        };
        if p.national_dex_number < *lo || p.national_dex_number > *hi {
            return false;
        }
        // kinds of form introduced in later generations
        if form.introduced_gen as usize > gen {
            return false;
        }
    }
    if !f.type_.is_empty() && types.0 != f.type_ && types.1 != f.type_ {
        return false;
    }
    if !f.growth.is_empty() && p.growth_rate != f.growth {
        return false;
    }
    if !f.stage.is_empty() && stage_value(p.evolution_stage) != f.stage {
        return false;
    }
    if !f.regional && form.is_regional {
        return false;
    }
    if !f.megas && form.is_mega {
        return false;
    }
    if !f.forms && (form.is_variant || form.is_gmax) {
        return false;
    }
    true
}

/// Per-game typing of every species of the index (`None` where the game
/// lacks the species).
fn game_types(game: &str) -> Vec<Option<(String, String)>> {
    if game.is_empty() {
        return Vec::new();
    }
    get_all_pokemon()
        .iter()
        .map(|p| xpr_dex::get_pokemon_types(&p.name, game))
        .collect()
}

/// The names of the species that pass `f` in `game`, in list order
/// (what the list shows and writes to `DexState::filtered_names`).
pub fn filter_names(f: &Filters, game: &str) -> Vec<String> {
    let all = get_all_pokemon();
    let types = game_types(game);
    let q = f.query.trim().to_lowercase();
    all.iter()
        .enumerate()
        .filter(|(i, p)| passes(f, &q, p, types_of(&types, *i, p)))
        .map(|(_, p)| p.name.clone())
        .collect()
}

#[derive(Default)]
struct Cache {
    key: Option<(Filters, String)>,
    indices: Vec<u32>,
    game_types: Vec<Option<(String, String)>>,
    game_types_for: String,
}

pub struct ListState {
    pub filters: Filters,
    cache: Cache,
    scroll_y: f32,
    last_index: Option<usize>,
}

impl Default for ListState {
    fn default() -> Self {
        ListState {
            filters: Filters::default(),
            cache: Cache::default(),
            scroll_y: 0.0,
            last_index: None,
        }
    }
}

impl ListState {
    pub fn ui(&mut self, ui: &mut Ui, cx: &mut DexCx) {
        let theme = cx.theme;
        let game = cx.state.game().to_string();
        let width = ui.available_width();
        let top = ui.cursor().min.y;
        ui.spacing_mut().item_spacing = Vec2::ZERO;

        // ---- header: search, filters, toggles, count --------------------------------------------
        ui.add_space(8.0);
        let inner_w = width - 16.0;
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let r = Entry::new(theme, &mut self.filters.query)
                .width(inner_w)
                .hint("Search name, #, or type\u{2026}")
                .id(Id::new("dex_pokedex_search"))
                .font(palette::px(theme, 14.0))
                .min_height(34.0)
                .margin(egui::Margin::symmetric(8, 6))
                .corner_radius(4)
                .clearable()
                .show(ui);
            let _ = r;
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let gap = 4.0;
            let w = (inner_w - 3.0 * gap) / 4.0;
            let f = &mut self.filters;
            let gens: Vec<(String, String)> = std::iter::once((String::new(), "Gen".to_string()))
                .chain((1..=9).map(|g| (g.to_string(), format!("Gen {}", g))))
                .collect();
            common::select(
                ui,
                theme,
                Id::new("dex_filter_gen"),
                w,
                &mut f.gen,
                "Gen",
                &gens,
            );
            ui.add_space(gap);
            let types: Vec<(String, String)> = std::iter::once((String::new(), "Type".to_string()))
                .chain(ALL_TYPES.iter().map(|t| (t.to_string(), t.to_string())))
                .collect();
            common::select(
                ui,
                theme,
                Id::new("dex_filter_type"),
                w,
                &mut f.type_,
                "Type",
                &types,
            );
            ui.add_space(gap);
            let growth: Vec<(String, String)> =
                std::iter::once((String::new(), "Growth".to_string()))
                    .chain(
                        ALL_GROWTH_RATES
                            .iter()
                            .map(|t| (t.to_string(), t.to_string())),
                    )
                    .collect();
            common::select(
                ui,
                theme,
                Id::new("dex_filter_growth"),
                w,
                &mut f.growth,
                "Growth",
                &growth,
            );
            ui.add_space(gap);
            let stages: Vec<(String, String)> =
                std::iter::once((String::new(), "Stage".to_string()))
                    .chain(
                        ALL_EVO_STAGES
                            .iter()
                            .map(|(v, l)| (v.to_string(), l.to_string())),
                    )
                    .collect();
            common::select(
                ui,
                theme,
                Id::new("dex_filter_stage"),
                w,
                &mut f.stage,
                "Stage",
                &stages,
            );
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0 + 2.0);
            let f = &mut self.filters;
            common::mini_checkbox(
                ui,
                theme,
                Id::new("dex_filter_regional"),
                &mut f.regional,
                "Regional",
            );
            ui.add_space(12.0);
            common::mini_checkbox(
                ui,
                theme,
                Id::new("dex_filter_megas"),
                &mut f.megas,
                "Megas",
            );
            ui.add_space(12.0);
            common::mini_checkbox(
                ui,
                theme,
                Id::new("dex_filter_forms"),
                &mut f.forms,
                "Forms",
            );
        });

        // ---- the filtered list ----------------------------------------------------------------------
        let key = (self.filters.clone(), game.clone());
        if self.cache.key.as_ref() != Some(&key) {
            if self.cache.game_types_for != game || self.cache.game_types.is_empty() {
                self.cache.game_types = game_types(&game);
                self.cache.game_types_for = game.clone();
            }
            let all = get_all_pokemon();
            let q = self.filters.query.trim().to_lowercase();
            let indices: Vec<u32> = all
                .iter()
                .enumerate()
                .filter(|(i, p)| {
                    passes(
                        &self.filters,
                        &q,
                        p,
                        types_of(&self.cache.game_types, *i, p),
                    )
                })
                .map(|(i, _)| i as u32)
                .collect();
            cx.state.filtered_names = indices
                .iter()
                .map(|i| all[*i as usize].name.clone())
                .collect();
            self.cache.indices = indices;
            self.cache.key = Some(key);
            self.last_index = None;
        }
        let all = get_all_pokemon();
        let count = self.cache.indices.len();
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0 + 2.0);
            ui.label(
                egui::RichText::new(format!("{} Pok\u{e9}mon", count))
                    .font(palette::px(theme, 11.0))
                    .color(palette::GRAY_600),
            );
        });
        ui.add_space(8.0);
        let (line, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), Sense::hover());
        ui.painter().hline(
            line.x_range(),
            line.center().y,
            Stroke::new(1.0_f32, palette::GRAY_700),
        );
        let _ = top;

        let selected = cx.state.selected().map(str::to_string);
        let selected_index = selected.as_deref().and_then(|s| {
            self.cache
                .indices
                .iter()
                .position(|i| all[*i as usize].name == s)
        });
        let show_types = width >= SHOW_TYPES_MIN_WIDTH;
        let mc = MenuCtx::of(cx.state);

        let mut scroll = egui::ScrollArea::vertical()
            .id_salt("dex_pokedex_list")
            .auto_shrink([false, false]);
        // keep the selected row in view when it changes (keys, spotlight, evolution links)
        if selected_index != self.last_index {
            if let Some(idx) = selected_index {
                let viewport = ui.available_height();
                let (t, b) = (idx as f32 * ROW_HEIGHT, (idx + 1) as f32 * ROW_HEIGHT);
                let mut off = self.scroll_y;
                if t < off {
                    off = t;
                } else if b > off + viewport {
                    off = b - viewport;
                }
                if (off - self.scroll_y).abs() > 0.5 {
                    scroll = scroll.vertical_scroll_offset(off);
                }
            }
            self.last_index = selected_index;
        }
        if count == 0 {
            let rect = ui.available_rect_before_wrap();
            ui.painter().text(
                Pos2::new(rect.center().x, rect.min.y + 32.0),
                Align2::CENTER_CENTER,
                "No results",
                palette::px(theme, 14.0),
                palette::GRAY_600,
            );
            ui.allocate_rect(rect, Sense::hover());
            return;
        }
        let out = scroll.show_rows(ui, ROW_HEIGHT, count, |ui, range| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            for row in range {
                let p = &all[self.cache.indices[row] as usize];
                let types = types_of(&self.cache.game_types, self.cache.indices[row] as usize, p);
                self.row(
                    ui,
                    cx,
                    p,
                    types,
                    selected.as_deref() == Some(p.name.as_str()),
                    show_types,
                    &game,
                    &mc,
                );
            }
        });
        self.scroll_y = out.state.offset.y;
    }

    #[allow(clippy::too_many_arguments)]
    fn row(
        &self,
        ui: &mut Ui,
        cx: &mut DexCx,
        p: &PokemonListEntry,
        types: (&str, &str),
        is_selected: bool,
        show_types: bool,
        game: &str,
        mc: &MenuCtx,
    ) {
        let theme = cx.theme;
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_HEIGHT), Sense::hover());
        // the row is a real widget registered before its badges, so a badge click wins
        let resp = ui.interact(rect, Id::new(("dex_list_row", &p.name)), Sense::click());
        if !ui.is_rect_visible(rect) {
            return;
        }
        if is_selected {
            ui.painter()
                .rect_filled(rect, CornerRadius::ZERO, palette::GRAY_700);
        } else if resp.hovered() {
            ui.painter()
                .rect_filled(rect, CornerRadius::ZERO, palette::GRAY_800);
        }
        ui.painter().hline(
            rect.x_range(),
            rect.max.y - 0.5,
            Stroke::new(1.0_f32, palette::GRAY_800),
        );
        let cy = rect.min.y + 12.0;
        // dex number (font-mono, right-aligned)
        let num = format!("{:04}", p.national_dex_number);
        ui.painter().text(
            Pos2::new(rect.min.x + 4.0 + 34.0, cy),
            Align2::RIGHT_CENTER,
            num,
            egui::FontId::monospace(14.0 * 0.92),
            palette::GRAY_600,
        );
        // sprite
        let sprite =
            Rect::from_min_size(Pos2::new(rect.min.x + 42.0, rect.min.y), Vec2::splat(24.0));
        common::paint_sprite(ui, cx.images, &p.name, p.national_dex_number, sprite, true);
        // typing, right-aligned
        let dual = types.0 != types.1;
        let n_badges = if show_types { 1 + dual as usize } else { 0 };
        let badges_w = if n_badges > 0 {
            n_badges as f32 * 68.0 + (n_badges - 1) as f32 * 4.0
        } else {
            0.0
        };
        let name_x = sprite.max.x + 4.0;
        let name_max =
            (rect.max.x - 4.0 - badges_w - if n_badges > 0 { 4.0 } else { 0.0 } - name_x).max(10.0);
        let font = palette::px(theme, 14.0);
        let name = xpr_ui_kit::widgets::elide(ui, display_name(&p.name), &font, name_max);
        ui.painter().text(
            Pos2::new(name_x, cy),
            Align2::LEFT_CENTER,
            name,
            font,
            Color32::WHITE,
        );
        if n_badges > 0 {
            let brect = Rect::from_min_size(
                Pos2::new(rect.max.x - 4.0 - badges_w, rect.min.y + 3.0),
                Vec2::new(badges_w, 18.0),
            );
            common::in_rect(ui, brect, ("dex_row_types", &p.name), |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                widgets::type_badge(ui, theme, types.0, true, Some(game));
                if dual {
                    widgets::type_badge(ui, theme, types.1, true, Some(game));
                }
            });
        }
        if resp.clicked() {
            cx.state.select_species(&p.name);
        }
        pokemon_context_menu(cx, &resp, &p.name, mc);
    }
}
