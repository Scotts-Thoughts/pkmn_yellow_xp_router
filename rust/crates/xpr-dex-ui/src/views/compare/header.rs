//! The pieces of a comparison's header that are not the stat rows: the
//! species identity (name, artwork, typing), the defensive type matchups,
//! the evolution family chips and the self comparison's game chips.

use std::collections::HashMap;

use egui::{Align2, Color32, CornerRadius, Id, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use indexmap::IndexMap;
use xpr_dex::types::{ability_immunity_type, EffGroup, EFF_GROUPS};
use xpr_dex::{display_name, EvolutionEntry, PokemonData};
use xpr_ui_kit::theme::{self, Theme};

use super::draw::{centered_text, centered_text_height, in_rect, text_width};
use super::probe;
use crate::images::SpriteSize;
use crate::palette::{self, hex};
use crate::views::movepool::paint::{italic_at, italic_w};
use crate::views::pokedex::sprite_scale;
use crate::widgets;
use crate::DexCx;

// ---------------------------------------------------------------------------
// identity
// ---------------------------------------------------------------------------

/// How a species' identity block is laid out (the three views differ).
#[derive(Clone, Copy, Debug)]
pub struct IdentityLook {
    /// artwork box (`w-36` 144 pair / self, `w-28` 112 triple)
    pub sprite: f32,
    /// name font size (`text-lg` 18, `text-base` 16)
    pub name_px: f32,
    /// the form ("(Aria)", "Mega") goes on a line of its own (pair, self)
    pub split_form: bool,
    /// how far the typing badges are pulled up over the artwork (`-mt-5` 20, `-mt-4` 16)
    pub overlap: f32,
    /// gap between name, artwork and badges (`gap-1.5` 6, `gap-1` 4)
    pub gap: f32,
}

pub const PAIR_IDENTITY: IdentityLook = IdentityLook {
    sprite: 144.0,
    name_px: 18.0,
    split_form: true,
    overlap: 20.0,
    gap: 6.0,
};

pub const TRIPLE_IDENTITY: IdentityLook = IdentityLook {
    sprite: 112.0,
    name_px: 16.0,
    split_form: false,
    overlap: 16.0,
    gap: 4.0,
};

fn name_text(pokemon: &PokemonData, look: &IdentityLook) -> String {
    let shown = display_name(&pokemon.species);
    if look.split_form {
        let (base, form) = xpr_dex::split_form_name(shown);
        match form {
            Some(f) => format!("{}\n{}", base, f),
            None => base.to_string(),
        }
    } else {
        shown.to_string()
    }
}

/// Size of the identity block when its name wraps at `wrap_w`.
pub fn identity_size(
    ui: &Ui,
    theme: &Theme,
    pokemon: &PokemonData,
    look: &IdentityLook,
    wrap_w: f32,
) -> Vec2 {
    let font = palette::px_bold(theme, look.name_px);
    let text = name_text(pokemon, look);
    let name_h = centered_text_height(ui, &text, font.clone(), wrap_w);
    let mut name_w = 0.0_f32;
    for line in text.split('\n') {
        name_w = name_w.max(text_width(ui, line, &font));
    }
    let ntypes = pokemon.types().len() as f32;
    let badges_w = ntypes * 80.0 + (ntypes - 1.0) * 6.0;
    let h = name_h + look.gap + look.sprite + look.gap - look.overlap + 24.0;
    Vec2::new(look.sprite.max(name_w.min(wrap_w)).max(badges_w), h)
}

/// Draw the identity block centred on `cx` with its top at `top`; returns
/// its height.
#[allow(clippy::too_many_arguments)]
pub fn paint_identity(
    ui: &mut Ui,
    cx: &mut DexCx,
    pokemon: &PokemonData,
    game: &str,
    look: &IdentityLook,
    center_x: f32,
    top: f32,
    wrap_w: f32,
    salt: &str,
) -> f32 {
    let theme = cx.theme;
    let font = palette::px_bold(theme, look.name_px);
    let text = name_text(pokemon, look);
    let name_h = centered_text(ui, center_x, top, &text, font, Color32::WHITE, wrap_w);
    let sprite_top = top + name_h + look.gap;
    let sprite_box = Rect::from_center_size(
        Pos2::new(center_x, sprite_top + look.sprite / 2.0),
        Vec2::splat(look.sprite),
    );
    if let Some(tex) = cx.images.sprite(
        ui.ctx(),
        &pokemon.species,
        pokemon.national_dex_number,
        SpriteSize::Full,
    ) {
        let scale = sprite_scale(pokemon);
        let r = Rect::from_center_size(sprite_box.center(), sprite_box.size() * scale);
        crate::images::paint_fit(ui, &tex, r, Color32::WHITE);
    } else {
        ui.painter().text(
            sprite_box.center(),
            Align2::CENTER_CENTER,
            "?",
            palette::px(theme, 36.0),
            palette::GRAY_700,
        );
    }
    let types = pokemon.types();
    let total = types.len() as f32 * 80.0 + (types.len() as f32 - 1.0) * 6.0;
    let badge_top = sprite_box.max.y + look.gap - look.overlap;
    let mut x = center_x - total / 2.0;
    for t in types {
        let r = Rect::from_min_size(Pos2::new(x, badge_top), Vec2::new(80.0, 24.0));
        in_rect(ui, r, ("cmp_id_type", salt, t), |ui| {
            widgets::type_badge(ui, theme, t, false, Some(game));
        });
        x += 86.0;
    }
    badge_top + 24.0 - top
}

// ---------------------------------------------------------------------------
// defensive type matchups
// ---------------------------------------------------------------------------

/// How the matchups block is laid out.
#[derive(Clone, Copy, Debug)]
pub struct EffLook {
    /// the badge is on the right, the multiplier and ability note to its left
    /// (the left column of the pair and self views: `flex-row-reverse`)
    pub reverse: bool,
    /// gap inside a row (`gap-2` 8, `gap-1.5` 6)
    pub gap: f32,
    /// space above and below a group separator (`mt-2 pt-2` 8, `mt-1 pt-1` 4)
    pub sep: f32,
}

pub const PAIR_EFF_LEFT: EffLook = EffLook {
    reverse: true,
    gap: 8.0,
    sep: 8.0,
};
pub const PAIR_EFF_RIGHT: EffLook = EffLook {
    reverse: false,
    gap: 8.0,
    sep: 8.0,
};
pub const TRIPLE_EFF: EffLook = EffLook {
    reverse: false,
    gap: 6.0,
    sep: 4.0,
};

const EFF_ROW_H: f32 = 24.0;

struct EffEntry {
    t: String,
    /// the other side's multiplier differs (self comparison: yellow chip)
    unique: bool,
    ability: Option<String>,
}

/// The matchup groups of a typing (Solodex `ComparisonEffectiveness` /
/// `SelfEffectiveness` / `TripleEffectiveness`).
pub struct Eff {
    groups: Vec<(&'static EffGroup, Vec<EffEntry>)>,
    game: String,
}

impl Eff {
    /// `other`: the other side's matchups, when the types in each group are
    /// ordered shared-first and the unshared ones marked (self comparison).
    pub fn new(
        type1: &str,
        type2: &str,
        game: &str,
        abilities: &[String],
        other: Option<&IndexMap<String, f64>>,
    ) -> Eff {
        let matchups = xpr_dex::defense_matchups(type1, type2, game);
        let mut immune_by: HashMap<&str, &str> = HashMap::new();
        for a in abilities {
            if let Some(t) = ability_immunity_type(a, game) {
                immune_by.insert(t, a.as_str());
            }
        }
        let mut groups = Vec::new();
        for g in EFF_GROUPS.iter() {
            let types: Vec<&str> = matchups
                .iter()
                .filter(|(_, v)| **v == g.value)
                .map(|(t, _)| t.as_str())
                .collect();
            if types.is_empty() {
                continue;
            }
            let entry = |t: &str, unique: bool| EffEntry {
                t: t.to_string(),
                unique,
                ability: immune_by.get(t).map(|a| a.to_string()),
            };
            let mut entries = Vec::new();
            match other {
                Some(o) => {
                    let (shared, unique): (Vec<&str>, Vec<&str>) =
                        types.iter().partition(|t| o.get(**t) == Some(&g.value));
                    entries.extend(shared.iter().map(|t| entry(t, false)));
                    entries.extend(unique.iter().map(|t| entry(t, true)));
                }
                None => entries.extend(types.iter().map(|t| entry(t, false))),
            }
            groups.push((g, entries));
        }
        Eff {
            groups,
            game: game.to_string(),
        }
    }

    fn row_w(&self, ui: &Ui, theme: &Theme, look: &EffLook, e: &EffEntry) -> f32 {
        let mut w = 68.0 + look.gap + 28.0;
        if let Some(a) = &e.ability {
            w += look.gap + italic_w(ui, &format!("({})", a), &palette::px(theme, 12.0));
        }
        w
    }

    /// Width and height of the block.
    pub fn size(&self, ui: &Ui, theme: &Theme, look: &EffLook) -> Vec2 {
        let mut w = 0.0_f32;
        let mut rows = 0usize;
        for (_, entries) in &self.groups {
            for e in entries {
                w = w.max(self.row_w(ui, theme, look, e));
                rows += 1;
            }
        }
        let seps = self.groups.len().saturating_sub(1) as f32;
        Vec2::new(
            w,
            rows as f32 * EFF_ROW_H + seps * (2.0 * look.sep + 1.0),
        )
    }

    /// Draw the block with its top at `top`, flush with `edge_x` (its right
    /// edge when `look.reverse`, else its left edge).
    pub fn paint(
        &self,
        ui: &mut Ui,
        theme: &Theme,
        look: &EffLook,
        edge_x: f32,
        top: f32,
        salt: &str,
    ) {
        let size = self.size(ui, theme, look);
        let mut y = top;
        let small = palette::px(theme, 12.0);
        for (gi, (group, entries)) in self.groups.iter().enumerate() {
            if gi > 0 {
                y += look.sep;
                let (x0, x1) = if look.reverse {
                    (edge_x - size.x, edge_x)
                } else {
                    (edge_x, edge_x + size.x)
                };
                ui.painter().hline(
                    x0..=x1,
                    y + 0.5,
                    Stroke::new(1.0_f32, palette::GRAY_800),
                );
                y += 1.0 + look.sep;
            }
            for e in entries {
                let (badge, chip, note_x) = if look.reverse {
                    let b = Rect::from_min_size(
                        Pos2::new(edge_x - 68.0, y + 3.0),
                        Vec2::new(68.0, 18.0),
                    );
                    let c = Rect::from_min_size(
                        Pos2::new(b.min.x - look.gap - 28.0, y + 2.0),
                        Vec2::new(28.0, 20.0),
                    );
                    (b, c, c.min.x - look.gap)
                } else {
                    let b = Rect::from_min_size(Pos2::new(edge_x, y + 3.0), Vec2::new(68.0, 18.0));
                    let c = Rect::from_min_size(
                        Pos2::new(b.max.x + look.gap, y + 2.0),
                        Vec2::new(28.0, 20.0),
                    );
                    (b, c, c.max.x + look.gap)
                };
                in_rect(ui, badge, ("cmp_eff", salt, &e.t), |ui| {
                    widgets::type_badge(ui, theme, &e.t, true, Some(&self.game));
                });
                probe::record("type", salt, gi, &e.t, badge);
                ui.painter()
                    .rect_filled(chip, CornerRadius::same(4), hex(group.bg));
                ui.painter().text(
                    chip.center(),
                    Align2::CENTER_CENTER,
                    group.multiplier_label,
                    palette::px_bold(theme, 12.0),
                    if e.unique {
                        hex("#facc15")
                    } else {
                        hex(group.text)
                    },
                );
                if let Some(a) = &e.ability {
                    let text = format!("({})", a);
                    let (pos, align) = if look.reverse {
                        (Pos2::new(note_x, y + EFF_ROW_H / 2.0), Align2::RIGHT_CENTER)
                    } else {
                        (Pos2::new(note_x, y + EFF_ROW_H / 2.0), Align2::LEFT_CENTER)
                    };
                    italic_at(ui, pos, align, &text, small.clone(), palette::GRAY_500);
                }
                y += EFF_ROW_H;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// evolution family chips
// ---------------------------------------------------------------------------

/// The arrow-less label before an evolution chip (`Lv.16`, the item,
/// the method).
fn evo_label(evo: &EvolutionEntry) -> Option<String> {
    let method = evo.method.as_deref()?;
    Some(match method {
        "level" => format!(
            "Lv.{}",
            evo.parameter
                .as_ref()
                .map(|p| p.to_string())
                .unwrap_or_default()
        ),
        "item" => evo
            .parameter
            .as_ref()
            .map(|p| p.to_string())
            .unwrap_or_default(),
        other => other.to_string(),
    })
}

/// Greedy wrapping of items of the given widths into lines of at most `max_w`.
fn wrap_lines(widths: &[f32], max_w: f32, gap: f32) -> Vec<Vec<usize>> {
    let mut lines: Vec<Vec<usize>> = vec![Vec::new()];
    let mut used = 0.0;
    for (i, w) in widths.iter().enumerate() {
        let line = lines.last_mut().unwrap();
        if !line.is_empty() && used + gap + w > max_w {
            lines.push(vec![i]);
            used = *w;
        } else {
            used += if line.is_empty() { *w } else { gap + w };
            line.push(i);
        }
    }
    lines
}

fn line_width(widths: &[f32], line: &[usize], gap: f32) -> f32 {
    line.iter().map(|i| widths[*i]).sum::<f32>() + gap * line.len().saturating_sub(1) as f32
}

/// The chip rows of an evolution family: their height when wrapped in `w`.
pub fn evo_chips_height(ui: &Ui, theme: &Theme, family: &[EvolutionEntry], current: &str, w: f32) -> f32 {
    let (_, lines) = evo_layout(ui, theme, family, current, w);
    lines.len() as f32 * 20.0 + (lines.len() as f32 - 1.0) * 4.0
}

struct EvoItem {
    label: Option<String>,
    label_w: f32,
    btn_w: f32,
    current: bool,
}

fn evo_layout(
    ui: &Ui,
    theme: &Theme,
    family: &[EvolutionEntry],
    current: &str,
    w: f32,
) -> (Vec<EvoItem>, Vec<Vec<usize>>) {
    let small = palette::px(theme, 12.0);
    let bold = palette::px_bold(theme, 12.0);
    let items: Vec<EvoItem> = family
        .iter()
        .enumerate()
        .map(|(i, evo)| {
            let label = if i > 0 { evo_label(evo) } else { None };
            let is_current = evo.species == current;
            EvoItem {
                label_w: label
                    .as_ref()
                    .map(|l| text_width(ui, l, &small) + 4.0)
                    .unwrap_or(0.0),
                label,
                btn_w: text_width(
                    ui,
                    display_name(&evo.species),
                    if is_current { &bold } else { &small },
                ) + 16.0,
                current: is_current,
            }
        })
        .collect();
    let widths: Vec<f32> = items.iter().map(|it| it.label_w + it.btn_w).collect();
    let lines = wrap_lines(&widths, w, 4.0);
    (items, lines)
}

/// Draw an evolution family as centred, wrapping chips in `[x0, x0 + w]`
/// from `y`. Returns the species whose chip was clicked.
#[allow(clippy::too_many_arguments)]
pub fn evo_chips(
    ui: &mut Ui,
    theme: &Theme,
    family: &[EvolutionEntry],
    current: &str,
    x0: f32,
    w: f32,
    y: f32,
    salt: &str,
) -> Option<String> {
    let small = palette::px(theme, 12.0);
    let bold = palette::px_bold(theme, 12.0);
    let (items, lines) = evo_layout(ui, theme, family, current, w);
    let widths: Vec<f32> = items.iter().map(|it| it.label_w + it.btn_w).collect();
    let mut picked = None;
    for (li, line) in lines.iter().enumerate() {
        let ly = y + li as f32 * 24.0;
        let mut x = x0 + (w - line_width(&widths, line, 4.0)) / 2.0;
        for &i in line {
            let it = &items[i];
            if let Some(l) = &it.label {
                ui.painter().text(
                    Pos2::new(x, ly + 10.0),
                    Align2::LEFT_CENTER,
                    l,
                    small.clone(),
                    palette::GRAY_600,
                );
            }
            let btn = Rect::from_min_size(Pos2::new(x + it.label_w, ly), Vec2::new(it.btn_w, 20.0));
            let evo = &family[i];
            let resp = ui.interact(
                btn,
                Id::new(("cmp_evo", salt, i)),
                if it.current {
                    Sense::hover()
                } else {
                    Sense::click()
                },
            );
            let (fill, fg) = if it.current {
                (palette::GRAY_600, Color32::WHITE)
            } else if resp.hovered() {
                (palette::GRAY_700, Color32::WHITE)
            } else {
                (palette::GRAY_800, palette::GRAY_400)
            };
            ui.painter().rect_filled(btn, CornerRadius::same(4), fill);
            ui.painter().text(
                btn.center(),
                Align2::CENTER_CENTER,
                display_name(&evo.species),
                if it.current {
                    bold.clone()
                } else {
                    small.clone()
                },
                fg,
            );
            probe::record("evo", salt, i, &evo.species, btn);
            if !it.current && resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                picked = Some(evo.species.clone());
            }
            x += it.label_w + it.btn_w + 4.0;
        }
    }
    picked
}

// ---------------------------------------------------------------------------
// game chips (self comparison)
// ---------------------------------------------------------------------------

/// The self comparison's `GameSelector`: one chip per game of the species,
/// centred and wrapping in `[x0, x0 + w]`; the other side's game is
/// disabled. Returns the height used and the game that was clicked.
#[allow(clippy::too_many_arguments)]
pub fn game_chips(
    ui: &mut Ui,
    theme: &Theme,
    games: &[String],
    selected: &str,
    disabled: &str,
    x0: f32,
    w: f32,
    y: f32,
    salt: &str,
    paint: bool,
) -> (f32, Option<String>) {
    let font = palette::px_bold(theme, 12.0);
    let widths: Vec<f32> = games
        .iter()
        .map(|g| {
            let border = if g != selected && g != disabled {
                2.0
            } else {
                0.0
            };
            text_width(ui, xpr_dex::game_abbrev(g), &font) + 12.0 + border
        })
        .collect();
    let lines = wrap_lines(&widths, w, 4.0);
    let height = lines.len() as f32 * 22.0 + (lines.len() as f32 - 1.0) * 4.0;
    if !paint {
        return (height, None);
    }
    let mut picked = None;
    for (li, line) in lines.iter().enumerate() {
        let ly = y + li as f32 * 26.0;
        let mut x = x0 + (w - line_width(&widths, line, 4.0)) / 2.0;
        for &i in line {
            let g = &games[i];
            let r = Rect::from_min_size(Pos2::new(x, ly), Vec2::new(widths[i], 22.0));
            let is_active = g == selected;
            let is_disabled = g == disabled;
            let color = palette::game_color(g);
            let resp = ui
                .interact(
                    r,
                    Id::new(("cmp_game", salt, g.as_str())),
                    if is_disabled {
                        Sense::hover()
                    } else {
                        Sense::click()
                    },
                )
                .on_hover_text(if is_disabled {
                    "Already shown on the other side"
                } else {
                    g.as_str()
                });
            if is_active {
                ui.painter().rect_filled(r, CornerRadius::same(4), color);
            } else if !is_disabled {
                ui.painter().rect_stroke(
                    r,
                    CornerRadius::same(4),
                    Stroke::new(1.0_f32, theme::with_alpha(color, 64)),
                    egui::StrokeKind::Inside,
                );
            }
            let fg = if is_active {
                Color32::WHITE
            } else if is_disabled {
                palette::GRAY_700
            } else if resp.hovered() {
                palette::GRAY_200
            } else {
                palette::GRAY_400
            };
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                xpr_dex::game_abbrev(g),
                font.clone(),
                fg,
            );
            probe::record("game", salt, i, g, r);
            if !is_disabled && resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                picked = Some(g.clone());
            }
            x += widths[i] + 4.0;
        }
    }
    (height, picked)
}
