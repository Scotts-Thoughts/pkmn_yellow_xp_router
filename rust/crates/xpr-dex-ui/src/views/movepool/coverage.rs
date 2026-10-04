//! The type coverage panel under the movepool (Solodex `TypeCoveragePanel.tsx`):
//! the right-clicked "test set" of up to four moves, how the best of their
//! types fares against every Pokémon of the game, and the type combinations
//! the set covers poorly.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use egui::{Align2, Color32, CornerRadius, Id, Pos2, Rect, Sense, Stroke, Ui, UiBuilder, Vec2};
use xpr_dex::{
    defense_matchups, get_all_pokemon_for_game, get_move_data, offensive_multiplier,
    types_for_game, PokemonData,
};
use xpr_ui_kit::theme::Theme;

use super::paint::{italic_at, italic_w, text_at};
use crate::images::{paint_fit, DexImages, SpriteSize};
use crate::palette::{self, hex};
use crate::widgets::{self, text_w};

/// One effectiveness bucket of the coverage breakdown.
pub struct CoverageBucket {
    pub label: &'static str,
    pub pokemon: Vec<Arc<PokemonData>>,
    pub color: &'static str,
    pub show_sprites: bool,
}

/// A type or type pair the move set hits for less than neutral.
#[derive(Clone, Debug, PartialEq)]
pub struct WeakCombo {
    pub type_1: String,
    pub type_2: Option<String>,
    pub best: f64,
}

/// A move of the test set as the panel shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct CoverageMove {
    pub name: String,
    pub move_type: Option<String>,
    pub is_status: bool,
}

/// Everything the panel shows, computed once per (test set, game).
pub struct Coverage {
    pub moves: Vec<CoverageMove>,
    pub all_status: bool,
    pub buckets: Option<Vec<CoverageBucket>>,
    pub weak: Option<Vec<WeakCombo>>,
}

/// The distinct types of the test set's damaging (non-Status) moves.
fn attack_types(test_set: &[String], game: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for name in test_set {
        if let Some(m) = get_move_data(name, game) {
            if m.category != "Status" && !out.contains(&m.move_type) {
                out.push(m.move_type);
            }
        }
    }
    out
}

fn best_multiplier(def: &indexmap::IndexMap<String, f64>, attack: &[String]) -> f64 {
    let mut best = 0.0_f64;
    for t in attack {
        let m = def.get(t).copied().unwrap_or(1.0);
        if m > best {
            best = m;
        }
    }
    best
}

/// `computeCoverage`: every Pokémon of the game in the bucket of the best
/// multiplier any of the set's attack types has against it.
fn compute_buckets(attack: &[String], game: &str) -> Option<Vec<CoverageBucket>> {
    if attack.is_empty() {
        return None;
    }
    let mut buckets: Vec<CoverageBucket> = vec![
        CoverageBucket {
            label: "No effect",
            pokemon: Vec::new(),
            color: "#4b5563",
            show_sprites: true,
        },
        CoverageBucket {
            label: "1/4x",
            pokemon: Vec::new(),
            color: "#991b1b",
            show_sprites: true,
        },
        CoverageBucket {
            label: "1/2x",
            pokemon: Vec::new(),
            color: "#b45309",
            show_sprites: true,
        },
        CoverageBucket {
            label: "Neutral",
            pokemon: Vec::new(),
            color: "#6b7280",
            show_sprites: false,
        },
        CoverageBucket {
            label: "2x",
            pokemon: Vec::new(),
            color: "#15803d",
            show_sprites: false,
        },
        CoverageBucket {
            label: "4x",
            pokemon: Vec::new(),
            color: "#047857",
            show_sprites: false,
        },
    ];
    let mut by_typing: HashMap<(String, String), f64> = HashMap::new();
    for poke in get_all_pokemon_for_game(game) {
        let key = (poke.type_1.clone(), poke.type_2.clone());
        let best = *by_typing.entry(key).or_insert_with(|| {
            best_multiplier(&defense_matchups(&poke.type_1, &poke.type_2, game), attack)
        });
        let i = if best == 0.0 {
            0
        } else if best <= 0.25 {
            1
        } else if best <= 0.5 {
            2
        } else if best <= 1.0 {
            3
        } else if best <= 2.0 {
            4
        } else {
            5
        };
        buckets[i].pokemon.push(poke);
    }
    Some(buckets)
}

/// `computeWeakCombos`: single types the set hits for less than neutral,
/// then dual types of the Pokédex whose combination alone makes the set weak
/// (neither of their single types is already in the list), worst first.
fn compute_weak(attack: &[String], game: &str) -> Option<Vec<WeakCombo>> {
    if attack.is_empty() {
        return None;
    }
    let mut weak: Vec<WeakCombo> = Vec::new();
    let mut single_weak: HashSet<String> = HashSet::new();
    for def in types_for_game(game) {
        let mut best = 0.0_f64;
        for atk in attack {
            let m = offensive_multiplier(atk, &def, game);
            if m > best {
                best = m;
            }
        }
        if best < 1.0 {
            single_weak.insert(def.clone());
            weak.push(WeakCombo {
                type_1: def,
                type_2: None,
                best,
            });
        }
    }
    let mut seen: HashSet<String> = HashSet::new();
    for poke in get_all_pokemon_for_game(game) {
        if poke.type_1 == poke.type_2 {
            continue;
        }
        let mut types = [poke.type_1.clone(), poke.type_2.clone()];
        types.sort();
        let key = types.join("/");
        if !seen.insert(key) {
            continue;
        }
        if single_weak.contains(&types[0]) || single_weak.contains(&types[1]) {
            continue;
        }
        let best = best_multiplier(&defense_matchups(&types[0], &types[1], game), attack);
        if best < 1.0 {
            weak.push(WeakCombo {
                type_1: types[0].clone(),
                type_2: Some(types[1].clone()),
                best,
            });
        }
    }
    // immunities first, then 1/4x, then 1/2x (stable)
    weak.sort_by(|a, b| {
        a.best
            .partial_cmp(&b.best)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Some(weak)
}

impl Coverage {
    pub fn compute(test_set: &[String], game: &str) -> Coverage {
        let moves: Vec<CoverageMove> = test_set
            .iter()
            .map(|name| {
                let data = get_move_data(name, game);
                CoverageMove {
                    name: name.clone(),
                    move_type: data.as_ref().map(|d| d.move_type.clone()),
                    is_status: data
                        .as_ref()
                        .map(|d| d.category == "Status")
                        .unwrap_or(false),
                }
            })
            .collect();
        let attack = attack_types(test_set, game);
        // `moveInfos.every(m => m.data?.category === 'Status')`
        let all_status = moves.iter().all(|m| m.is_status);
        Coverage {
            moves,
            all_status,
            buckets: compute_buckets(&attack, game),
            weak: compute_weak(&attack, game),
        }
    }
}

/// What the user did in the panel this frame.
#[derive(Default)]
pub struct PanelResult {
    pub remove: Option<String>,
    pub clear: bool,
}

const LINE_H: f32 = 16.0;

/// The text of a multiplier in the weak list: "0x", "¼x" or "½x".
pub fn weak_label(best: f64) -> &'static str {
    if best == 0.0 {
        "0x"
    } else if best <= 0.25 {
        "\u{bc}x"
    } else {
        "\u{bd}x"
    }
}

fn type_badge_at(
    ui: &mut Ui,
    theme: &Theme,
    rect: Rect,
    salt: Id,
    t: &str,
    game: &str,
) -> egui::Response {
    // a child ui: a scope would move the caller's cursor to the badge's end
    let mut bui = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .id_salt(salt)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    widgets::type_badge(&mut bui, theme, t, true, Some(game))
}

/// A small gray caption ("Weak coverage against").
fn caption(ui: &mut Ui, theme: &Theme, text: &str, color: Color32) {
    let font = palette::px(theme, 12.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), LINE_H), Sense::hover());
    text_at(
        ui,
        rect.left_center(),
        Align2::LEFT_CENTER,
        text,
        font,
        color,
        false,
    );
}

/// Draw the panel body (no frame; the caller supplies the background).
pub fn panel_ui(
    ui: &mut Ui,
    theme: &Theme,
    images: &mut DexImages,
    cov: &Coverage,
    game: &str,
) -> PanelResult {
    let mut out = PanelResult::default();
    let small = palette::px(theme, 12.0);
    ui.spacing_mut().item_spacing = Vec2::new(8.0, 4.0);

    // move chips: "Coverage" label and one chip per move; "Clear all" at the right edge
    let clear_label = "Clear all";
    let clear_w = text_w(ui, clear_label, &small);
    let row_w = ui.available_width();
    let row_top = ui.cursor().min;
    let clear_rect = Rect::from_min_size(
        Pos2::new(row_top.x + row_w - clear_w, row_top.y),
        Vec2::new(clear_w, 22.0),
    );
    let chips_rect = Rect::from_min_size(row_top, Vec2::new((row_w - clear_w - 12.0).max(120.0), 1.0e6));
    let mut chips_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(chips_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    chips_ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(8.0, 4.0);
        ui.label(
            egui::RichText::new("COVERAGE")
                .font(palette::px_bold(theme, 12.0))
                .color(palette::GRAY_400),
        );
        for (i, m) in cov.moves.iter().enumerate() {
            let name_w = text_w(ui, &m.name, &small);
            let status_w = if m.is_status {
                4.0 + italic_w(ui, "status", &small)
            } else {
                0.0
            };
            let x_w = text_w(ui, "\u{d7}", &small);
            let badge_w = if m.move_type.is_some() { 68.0 + 4.0 } else { 0.0 };
            let w = 8.0 + badge_w + name_w + status_w + 4.0 + x_w + 8.0;
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 22.0), Sense::click());
            super::probe::record("chip", "coverage", i, &m.name, rect);
            let resp = resp
                .on_hover_text("Click to remove")
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            let chip_id = resp.id;
            let fill = if resp.hovered() { palette::GRAY_600 } else { palette::GRAY_700 };
            ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
            let mut x = rect.min.x + 8.0;
            if let Some(t) = &m.move_type {
                let br = Rect::from_min_size(Pos2::new(x, rect.center().y - 9.0), Vec2::new(68.0, 18.0));
                type_badge_at(ui, theme, br, chip_id.with("badge"), t, game);
                x += 72.0;
            }
            let r = text_at(
                ui,
                Pos2::new(x, rect.center().y),
                Align2::LEFT_CENTER,
                &m.name,
                small.clone(),
                Color32::WHITE,
                false,
            );
            x = r.max.x;
            if m.is_status {
                let r = italic_at(
                    ui,
                    Pos2::new(x + 4.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    "status",
                    small.clone(),
                    palette::GRAY_500,
                );
                x = r.max.x;
            }
            ui.painter().text(
                Pos2::new(x + 4.0, rect.center().y),
                Align2::LEFT_CENTER,
                "\u{d7}",
                small.clone(),
                palette::GRAY_500,
            );
            if resp.clicked() {
                out.remove = Some(m.name.clone());
            }
        }
    });
    let clear = ui.interact(clear_rect, ui.id().with("cov_clear"), Sense::click());
    super::probe::record("clear", "coverage", 0, clear_label, clear_rect);
    let clear_color = if clear.hovered() { palette::GRAY_300 } else { palette::GRAY_500 };
    ui.painter().text(
        clear_rect.left_center(),
        Align2::LEFT_CENTER,
        clear_label,
        small.clone(),
        clear_color,
    );
    if clear.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        out.clear = true;
    }
    ui.allocate_rect(chips_ui.min_rect().union(clear_rect), Sense::hover());
    ui.add_space(4.0);

    if cov.all_status {
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), LINE_H), Sense::hover());
        italic_at(
            ui,
            rect.left_center(),
            Align2::LEFT_CENTER,
            "All moves are Status \u{2014} no type coverage to show.",
            small.clone(),
            palette::GRAY_500,
        );
        return out;
    }
    let Some(buckets) = &cov.buckets else {
        return out;
    };

    // counts row
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        for b in buckets {
            let lw = text_w(ui, b.label, &small);
            let w = lw.max(56.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 44.0), Sense::hover());
            ui.painter().text(
                Pos2::new(rect.center().x, rect.min.y + 8.0),
                Align2::CENTER_CENTER,
                b.label,
                small.clone(),
                palette::GRAY_500,
            );
            let color = if b.pokemon.is_empty() {
                hex("#374151")
            } else {
                hex(b.color)
            };
            ui.painter().text(
                Pos2::new(rect.center().x, rect.min.y + 30.0),
                Align2::CENTER_CENTER,
                b.pokemon.len().to_string(),
                palette::px_bold(theme, 18.0),
                color,
            );
        }
    });
    ui.add_space(4.0);

    // weak coverage combos
    if let Some(weak) = cov.weak.as_ref().filter(|w| !w.is_empty()) {
        caption(ui, theme, "Weak coverage against", palette::GRAY_500);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(12.0, 4.0);
            for (i, w) in weak.iter().enumerate() {
                let label = weak_label(w.best);
                let lw = text_w(ui, label, &small);
                let badges = if w.type_2.is_some() { 2 } else { 1 };
                let total = badges as f32 * 68.0 + (badges as f32) * 4.0 + lw;
                let (rect, _) = ui.allocate_exact_size(Vec2::new(total, 18.0), Sense::hover());
                let mut x = rect.min.x;
                for (j, t) in std::iter::once(&w.type_1)
                    .chain(w.type_2.as_ref())
                    .enumerate()
                {
                    let br = Rect::from_min_size(Pos2::new(x, rect.min.y), Vec2::new(68.0, 18.0));
                    type_badge_at(ui, theme, br, Id::new(("cov_weak_badge", i, j)), t, game);
                    x += 72.0;
                }
                ui.painter().text(
                    Pos2::new(x, rect.center().y),
                    Align2::LEFT_CENTER,
                    label,
                    small.clone(),
                    palette::GRAY_500,
                );
            }
        });
        ui.add_space(4.0);
    }

    // sprites of the bad matchups
    for b in buckets
        .iter()
        .filter(|b| b.show_sprites && !b.pokemon.is_empty())
    {
        caption(
            ui,
            theme,
            &format!("{} ({})", b.label, b.pokemon.len()),
            hex(b.color),
        );
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(2.0);
            for p in &b.pokemon {
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::hover());
                if ui.is_rect_visible(rect) {
                    if let Some(tex) = images.sprite(
                        ui.ctx(),
                        &p.species,
                        p.national_dex_number,
                        SpriteSize::Small,
                    ) {
                        // Solodex's 1 px black outline (`pokemon-icon-stroke`)
                        for d in [
                            Vec2::new(1.0, 0.0),
                            Vec2::new(-1.0, 0.0),
                            Vec2::new(0.0, 1.0),
                            Vec2::new(0.0, -1.0),
                        ] {
                            paint_fit(ui, &tex, rect.translate(d), Color32::BLACK);
                        }
                        paint_fit(ui, &tex, rect, Color32::WHITE);
                    }
                    let typing = if p.type_1 != p.type_2 {
                        format!("{}/{}", p.type_1, p.type_2)
                    } else {
                        p.type_1.clone()
                    };
                    resp.on_hover_text(format!("{} ({})", p.species, typing));
                }
            }
        });
        ui.add_space(4.0);
    }
    out
}

/// The panel's frame: gray-800 with rounded top corners and a gray-600 top border.
pub fn frame_fill(ui: &Ui, rect: Rect) {
    let radius = CornerRadius {
        nw: 8,
        ne: 8,
        sw: 0,
        se: 0,
    };
    ui.painter().rect_filled(rect, radius, palette::GRAY_800);
    ui.painter().hline(
        rect.x_range().shrink(6.0),
        rect.min.y + 0.5,
        Stroke::new(1.0_f32, palette::GRAY_600),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn electric_coverage_in_emerald() {
        let cov = Coverage::compute(&["Thunderbolt".to_string()], "Emerald");
        assert!(!cov.all_status);
        let buckets = cov.buckets.as_ref().unwrap();
        let in_bucket = |i: usize, name: &str| buckets[i].pokemon.iter().any(|p| p.species == name);
        // Ground types are immune, Water / Flying is 4x, Grass resists
        assert!(in_bucket(0, "Diglett"));
        assert!(in_bucket(5, "Gyarados"));
        assert!(in_bucket(2, "Bulbasaur"));
        assert!(in_bucket(3, "Machop"));
        // every species is in exactly one bucket
        let total: usize = buckets.iter().map(|b| b.pokemon.len()).sum();
        assert_eq!(total, get_all_pokemon_for_game("Emerald").len());
        // weak coverage: Ground first (immune), then 1/2x single types, no duplicates
        let weak = cov.weak.as_ref().unwrap();
        assert_eq!(weak[0], WeakCombo { type_1: "Ground".into(), type_2: None, best: 0.0 });
        assert!(weak.iter().any(|w| w.type_1 == "Grass" && w.type_2.is_none() && w.best == 0.5));
        // a dual type whose single types are weak is not listed again
        assert!(!weak.iter().any(|w| w.type_2.is_some() && (w.type_1 == "Ground" || w.type_2.as_deref() == Some("Ground"))));
    }

    #[test]
    fn status_moves_add_no_coverage() {
        let cov = Coverage::compute(&["Growl".to_string(), "Thunderbolt".to_string()], "Emerald");
        assert!(!cov.all_status);
        // only Thunderbolt's type counts
        assert!(cov.buckets.as_ref().unwrap()[0].pokemon.iter().any(|p| p.species == "Diglett"));
        let status = Coverage::compute(&["Growl".to_string()], "Emerald");
        assert!(status.all_status && status.buckets.is_none() && status.weak.is_none());
        // an unknown move is not "status": the panel shows an empty breakdown
        let unknown = Coverage::compute(&["Not A Move".to_string()], "Emerald");
        assert!(!unknown.all_status && unknown.buckets.is_none());
    }

    #[test]
    fn weak_labels() {
        assert_eq!(weak_label(0.0), "0x");
        assert_eq!(weak_label(0.25), "\u{bc}x");
        assert_eq!(weak_label(0.5), "\u{bd}x");
    }
}
