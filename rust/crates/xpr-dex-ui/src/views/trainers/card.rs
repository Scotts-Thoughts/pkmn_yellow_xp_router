//! One party card of the trainer detail (Solodex `TrainerDetail.tsx`
//! `PartyCard`): sprite, name, level, types, ability / nature / held item,
//! stat bars with a total, and the move table.
//!
//! The card is laid out by hand, in CSS pixels, so every card of a grid row
//! has the same height without a measuring pass: [`measure`] computes the
//! layout (heights, wrapped meta line, move columns) and [`draw`] paints it.

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Stroke, Ui, UiBuilder, Vec2};
use xpr_core::consts;
use xpr_data::{EnemyPkmn, Move, Nature};
use xpr_dex::PokemonData;
use xpr_ui_kit::theme::{self, Theme};

use super::data::{PartyMon, VersionIndex};
use crate::images::{self, DexImages, SpriteSize};
use crate::palette::{self, px, px_bold};
use crate::widgets;

pub const BORDER: f32 = 1.0;
const PAD: f32 = 12.0;
const SPRITE: f32 = 56.0;
const GAP: f32 = 12.0;
const STAT_ROW: f32 = 20.0;
const MOVE_ROW: f32 = 24.0;
const MOVE_HEAD: f32 = 18.0;
const BAR_MAX: f32 = 300.0;
/// The narrowest card that still fits the move table.
pub const MIN_CARD_W: f32 = 300.0;

/// A run of meta text in one colour.
type Run = (String, Color32);

#[derive(Clone, Debug)]
pub struct MetaItem {
    runs: Vec<Run>,
    width: f32,
}

#[derive(Clone, Debug)]
pub struct MoveRow {
    name: String,
    move_type: Option<String>,
    category: Option<String>,
    power: String,
    accuracy: String,
    pp: String,
}

#[derive(Clone, Debug)]
pub struct CardLayout {
    pub height: f32,
    top_h: f32,
    types: Vec<String>,
    meta_lines: Vec<Vec<MetaItem>>,
    moves: Vec<MoveRow>,
    cat_w: f32,
    pow_w: f32,
    acc_w: f32,
    pp_w: f32,
    stat_count: usize,
}

/// What a card needs from its surroundings.
pub struct CardEnv<'a> {
    pub theme: &'a Theme,
    pub ix: &'a VersionIndex,
}

fn dash(v: Option<i64>) -> String {
    match v {
        Some(n) if n > 0 => n.to_string(),
        _ => "\u{2014}".to_string(),
    }
}

/// The damage category of a move: the router's own, else (gens 1-3 store none)
/// the Dex's, else derived from the type.
fn category_of(ix: &VersionIndex, name: &str, m: &Move) -> Option<String> {
    if !m.category.is_empty() {
        return Some(m.category.clone());
    }
    if let Some(game) = ix.dex_game {
        if let Some(d) = xpr_dex::get_move_data(name, game) {
            if !d.category.is_empty() {
                return Some(d.category);
            }
        }
    }
    if m.base_power.unwrap_or(0) == 0 {
        return Some("Status".to_string());
    }
    let special = ix.gen.special_types.iter().any(|t| *t == m.move_type);
    Some(if special { "Special" } else { "Physical" }.to_string())
}

/// `+Atk` / `-Def` for a nature, in the order Solodex lists them.
pub fn nature_effect(n: Nature) -> Option<(&'static str, &'static str)> {
    let stats = [(consts::ATTACK, "Atk"), (consts::DEFENSE, "Def"), (consts::SPEED, "Spe"), (consts::SPECIAL_ATTACK, "SpA"), (consts::SPECIAL_DEFENSE, "SpD")];
    let up = stats.iter().find(|(k, _)| n.is_stat_raised(k)).map(|(_, l)| *l)?;
    let down = stats.iter().find(|(k, _)| n.is_stat_lowered(k)).map(|(_, l)| *l)?;
    Some((up, down))
}

/// Solodex `getSpriteScale`: first and middle evolution stages are drawn smaller.
pub fn sprite_scale(data: Option<&PokemonData>) -> f32 {
    let Some(d) = data else { return 1.0 };
    let family = &d.evolution_family;
    if family.len() <= 1 || xpr_dex::is_mega_form(&d.species) {
        return 1.0;
    }
    let evolved_from: Vec<&str> = family.iter().filter(|e| e.method.is_some()).map(|e| e.species.as_str()).collect();
    let evolves_into = family.iter().any(|e| e.species != d.species && e.method.is_some());
    let is_evolved_from = evolved_from.contains(&d.species.as_str());
    if evolves_into && !is_evolved_from {
        0.75
    } else if evolves_into && is_evolved_from {
        0.9
    } else {
        1.0
    }
}

pub fn effect_label(m: f64) -> String {
    if m == 0.0 {
        "0x".into()
    } else if m == 0.25 {
        "\u{bc}x".into()
    } else if m == 0.5 {
        "\u{bd}x".into()
    } else if m == 1.0 {
        "1x".into()
    } else {
        format!("{}x", m)
    }
}

/// Compute a card's layout for a card `width` wide.
pub fn measure(ui: &Ui, env: &CardEnv, enemy: &EnemyPkmn, mon: &PartyMon, width: f32) -> CardLayout {
    let theme = env.theme;
    let ix = env.ix;
    let gen1 = ix.generation == 1;
    // types
    let mut types = Vec::new();
    if let Some(s) = ix.gen.pkmn_db().get_pkmn(&mon.name) {
        types.push(s.first_type.clone());
        if s.second_type != s.first_type && !s.second_type.is_empty() {
            types.push(s.second_type.clone());
        }
    }
    // meta line items (text-xs): ability, nature, held item
    let f12 = px(theme, 12.0);
    let mut items: Vec<MetaItem> = Vec::new();
    let mut push = |ui: &Ui, runs: Vec<Run>| {
        let width = runs.iter().map(|(t, _)| widgets::text_w(ui, t, &f12)).sum();
        items.push(MetaItem { runs, width });
    };
    if !enemy.ability.is_empty() {
        push(ui, vec![(enemy.ability.clone(), palette::BLUE_400)]);
    }
    if ix.generation >= 3 {
        let name = enemy.nature.display_name();
        match nature_effect(enemy.nature) {
            Some((up, down)) => push(
                ui,
                vec![(format!("{} (", name), palette::GRAY_300), (format!("+{}", up), palette::RED_400), ("/".to_string(), palette::GRAY_300), (format!("-{}", down), palette::BLUE_400), (")".to_string(), palette::GRAY_300)],
            ),
            None => push(ui, vec![(name, palette::GRAY_400)]),
        }
    }
    if let Some(item) = enemy.held_item.as_deref().filter(|s| !s.is_empty()) {
        push(ui, vec![(item.to_string(), palette::YELLOW_400)]);
    }
    let text_w = width - 2.0 * BORDER - PAD - SPRITE - GAP - PAD;
    let mut meta_lines: Vec<Vec<MetaItem>> = Vec::new();
    let mut cur_w = 0.0;
    for it in items {
        if let Some(line) = meta_lines.last_mut() {
            if cur_w + 12.0 + it.width <= text_w {
                cur_w += 12.0 + it.width;
                line.push(it);
                continue;
            }
        }
        cur_w = it.width;
        meta_lines.push(vec![it]);
    }
    let meta_h = if meta_lines.is_empty() { 0.0 } else { 4.0 + 16.0 * meta_lines.len() as f32 + 2.0 * (meta_lines.len() - 1) as f32 };
    let text_h = 24.0 + 4.0 + 18.0 + meta_h;
    let top_h = text_h.max(SPRITE);

    // moves
    let moves: Vec<MoveRow> = enemy
        .move_list
        .iter()
        .flatten()
        .filter(|n| !n.is_empty())
        .map(|name| match ix.gen.move_db().get_move(name) {
            Some(m) => MoveRow { name: name.clone(), move_type: Some(m.move_type.clone()), category: category_of(ix, name, m), power: dash(m.base_power), accuracy: dash(m.accuracy), pp: dash(m.pp) },
            None => MoveRow { name: name.clone(), move_type: None, category: None, power: dash(None), accuracy: dash(None), pp: dash(None) },
        })
        .collect();
    let col_w = |head: &str, vals: Vec<&str>, pad: f32| -> f32 { vals.into_iter().chain(std::iter::once(head)).map(|t| widgets::text_w(ui, t, &f12)).fold(0.0, f32::max) + pad };
    let pow_w = col_w("Pow", moves.iter().map(|m| m.power.as_str()).collect(), 12.0);
    let acc_w = col_w("Acc", moves.iter().map(|m| m.accuracy.as_str()).collect(), 12.0);
    let pp_w = col_w("PP", moves.iter().map(|m| m.pp.as_str()).collect(), 6.0);
    let cat_w = 29.0;

    let stat_count = if gen1 { 5 } else { 6 };
    let stats_h = 4.0 + (stat_count as f32 * STAT_ROW + (stat_count - 1) as f32 * 4.0) + (4.0 + 1.0 + 4.0 + STAT_ROW) + 8.0;
    let moves_h = MOVE_HEAD + MOVE_ROW * moves.len() as f32 + 12.0;
    let height = 2.0 * BORDER + (PAD + top_h + 8.0) + stats_h + moves_h;
    CardLayout { height, top_h, types, meta_lines, moves, cat_w, pow_w, acc_w, pp_w, stat_count }
}

fn truncated(ui: &Ui, text: &str, font: &FontId, max_w: f32) -> String {
    xpr_ui_kit::widgets::elide(ui, text, font, max_w)
}

/// Paint the card into `rect` (whose height is at least `lay.height`).
#[allow(clippy::too_many_arguments)]
pub fn draw(ui: &mut Ui, env: &CardEnv, images: &mut DexImages, lay: &CardLayout, enemy: &EnemyPkmn, mon: &PartyMon, index: usize, rect: Rect) {
    let theme = env.theme;
    let ix = env.ix;
    let gen1 = ix.generation == 1;
    let game = ix.dex_game;
    let p = ui.painter().clone();
    // card: gray-800/50 with a gray-700/60 border, rounded-lg
    p.rect(rect, CornerRadius::same(8), Color32::from_rgba_unmultiplied(0x1f, 0x29, 0x37, 128), Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(0x37, 0x41, 0x51, 153)), egui::StrokeKind::Inside);
    let x0 = rect.min.x + BORDER;
    let x1 = rect.max.x - BORDER;
    let mut y = rect.min.y + BORDER + PAD;

    // ---- top row: sprite + name / types / meta
    let sprite_rect = Rect::from_min_size(Pos2::new(x0 + PAD, y + (lay.top_h - SPRITE) / 2.0), Vec2::splat(SPRITE));
    let tex = mon.dex_key.and_then(|k| images.sprite(ui.ctx(), k, mon.dex, SpriteSize::Full));
    match tex {
        Some(tex) => {
            let data = mon.dex_key.zip(game).and_then(|(k, g)| xpr_dex::get_pokemon_data(k, g));
            let scale = sprite_scale(data.as_deref());
            images::paint_fit(ui, &tex, Rect::from_center_size(sprite_rect.center(), Vec2::splat(SPRITE * scale)), Color32::WHITE);
        }
        None => {
            p.circle_filled(sprite_rect.center(), SPRITE / 2.0, palette::GRAY_700);
            p.text(sprite_rect.center(), Align2::CENTER_CENTER, (index + 1).to_string(), px_bold(theme, 18.0), palette::GRAY_500);
        }
    }
    let tx = x0 + PAD + SPRITE + GAP;
    let tw = x1 - PAD - tx;
    let f16b = px_bold(theme, 16.0);
    let f14 = px(theme, 14.0);
    let lv = format!("Lv{}", mon.level);
    let lv_w = widgets::text_w(ui, &lv, &f14);
    let name = truncated(ui, &mon.display, &f16b, (tw - lv_w - 8.0).max(20.0));
    let name_rect = p.text(Pos2::new(tx, y + 12.0), Align2::LEFT_CENTER, &name, f16b.clone(), Color32::WHITE);
    p.text(Pos2::new(name_rect.max.x + 8.0, y + 12.0 + 2.0), Align2::LEFT_CENTER, &lv, f14, palette::GRAY_500);
    // type badges
    let mut bx = tx;
    let by = y + 24.0 + 4.0;
    for (k, t) in lay.types.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(bx, by), Vec2::new(68.0, 18.0));
        let mut child = ui.new_child(UiBuilder::new().id_salt(("trainer_card_type", index, k)).max_rect(r));
        widgets::type_badge(&mut child, theme, t, true, game);
        bx += 68.0 + 4.0;
    }
    // meta lines
    let f12 = px(theme, 12.0);
    let mut my = by + 18.0 + 4.0;
    for line in &lay.meta_lines {
        let mut mx = tx;
        for it in line {
            let mut x = mx;
            for (text, color) in &it.runs {
                let r = p.text(Pos2::new(x, my + 8.0), Align2::LEFT_CENTER, text, f12.clone(), *color);
                x = r.max.x;
            }
            mx += it.width + 12.0;
        }
        my += 16.0 + 2.0;
    }
    y += lay.top_h + 8.0;

    // ---- stats
    y += 4.0;
    let en = enemy;
    let st = &en.cur_stats;
    let rows: Vec<(xpr_dex::StatKey, i64)> = if gen1 {
        vec![(xpr_dex::StatKey::Hp, st.hp), (xpr_dex::StatKey::Attack, st.attack), (xpr_dex::StatKey::Defense, st.defense), (xpr_dex::StatKey::SpecialAttack, st.special_attack), (xpr_dex::StatKey::Speed, st.speed)]
    } else {
        vec![
            (xpr_dex::StatKey::Hp, st.hp),
            (xpr_dex::StatKey::Attack, st.attack),
            (xpr_dex::StatKey::Defense, st.defense),
            (xpr_dex::StatKey::SpecialAttack, st.special_attack),
            (xpr_dex::StatKey::SpecialDefense, st.special_defense),
            (xpr_dex::StatKey::Speed, st.speed),
        ]
    };
    debug_assert_eq!(rows.len(), lay.stat_count);
    let label_right = x0 + PAD + 32.0;
    let value_right = label_right + 6.0 + 28.0;
    let bar_x = value_right + 6.0;
    let bar_w = (x1 - PAD - bar_x).max(10.0);
    let f14b = px_bold(theme, 14.0);
    let mut total = 0;
    for (k, (key, value)) in rows.iter().enumerate() {
        let cy = y + STAT_ROW / 2.0;
        let color = palette::stat_color(*key, gen1);
        p.text(Pos2::new(label_right, cy), Align2::RIGHT_CENTER, palette::stat_label(*key, gen1), px_bold(theme, 12.0), palette::GRAY_500);
        p.text(Pos2::new(value_right, cy), Align2::RIGHT_CENTER, value.to_string(), f14b.clone(), color);
        let track = Rect::from_min_size(Pos2::new(bar_x, cy - 7.0), Vec2::new(bar_w, 14.0));
        p.rect_filled(track, CornerRadius::same(2), palette::GRAY_700);
        let pct = (*value as f32 / BAR_MAX).clamp(0.0, 1.0);
        if pct > 0.0 {
            widgets::paint_glossy(ui, Rect::from_min_size(track.min, Vec2::new(bar_w * pct, 14.0)), 2, theme::with_alpha(color, 217));
        }
        total += value;
        y += STAT_ROW + if k + 1 < rows.len() { 4.0 } else { 0.0 };
    }
    // total row: border-t gray-700 after a 4 px gap
    y += 4.0;
    p.hline((x0 + PAD)..=(x1 - PAD), y + 0.5, Stroke::new(1.0_f32, palette::GRAY_700));
    y += 1.0 + 4.0;
    let cy = y + STAT_ROW / 2.0;
    p.text(Pos2::new(label_right, cy), Align2::RIGHT_CENTER, "Total", px_bold(theme, 12.0), palette::GRAY_500);
    p.text(Pos2::new(value_right, cy), Align2::RIGHT_CENTER, total.to_string(), f14b, Color32::WHITE);
    y += STAT_ROW + 8.0;

    // ---- moves
    let left = x0 + PAD;
    let right = x1 - PAD;
    let pp_x = right - lay.pp_w;
    let acc_x = pp_x - lay.acc_w;
    let pow_x = acc_x - lay.pow_w;
    let cat_x = pow_x - lay.cat_w;
    let move_x = left + 74.0;
    let head_c = y + 9.0 - 1.0;
    let gray600 = palette::GRAY_600;
    p.text(Pos2::new(left, head_c), Align2::LEFT_CENTER, "Type", f12.clone(), gray600);
    p.text(Pos2::new(move_x, head_c), Align2::LEFT_CENTER, "Move", f12.clone(), gray600);
    p.text(Pos2::new(cat_x + lay.cat_w / 2.0, head_c), Align2::CENTER_CENTER, "Cat", f12.clone(), gray600);
    p.text(Pos2::new(acc_x - 6.0, head_c), Align2::RIGHT_CENTER, "Pow", f12.clone(), gray600);
    p.text(Pos2::new(pp_x - 6.0, head_c), Align2::RIGHT_CENTER, "Acc", f12.clone(), gray600);
    p.text(Pos2::new(right, head_c), Align2::RIGHT_CENTER, "PP", f12.clone(), gray600);
    y += MOVE_HEAD;
    let f14 = px(theme, 14.0);
    for (k, m) in lay.moves.iter().enumerate() {
        let cy = y + MOVE_ROW / 2.0;
        if let Some(t) = &m.move_type {
            let r = Rect::from_min_size(Pos2::new(left, cy - 9.0), Vec2::new(68.0, 18.0));
            let mut child = ui.new_child(UiBuilder::new().id_salt(("trainer_card_move_type", index, k)).max_rect(r));
            widgets::type_badge(&mut child, theme, t, true, game);
        }
        let shown = truncated(ui, &m.name, &f14, (cat_x - move_x - 6.0).max(20.0));
        p.text(Pos2::new(move_x, cy), Align2::LEFT_CENTER, shown, f14.clone(), palette::GRAY_200);
        if let Some(cat) = &m.category {
            if let Some(tex) = images.category_icon(ui.ctx(), cat) {
                images::paint_fit(ui, &tex, Rect::from_center_size(Pos2::new(cat_x + lay.cat_w / 2.0, cy), Vec2::new(21.0, 14.0)), Color32::WHITE);
            }
        }
        p.text(Pos2::new(acc_x - 6.0, cy), Align2::RIGHT_CENTER, &m.power, f12.clone(), palette::GRAY_500);
        p.text(Pos2::new(pp_x - 6.0, cy), Align2::RIGHT_CENTER, &m.accuracy, f12.clone(), palette::GRAY_500);
        p.text(Pos2::new(right, cy), Align2::RIGHT_CENTER, &m.pp, f12.clone(), palette::GRAY_500);
        y += MOVE_ROW;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xpr_dex::EvolutionEntry;

    fn entry(species: &str, method: Option<&str>) -> EvolutionEntry {
        EvolutionEntry { species: species.to_string(), method: method.map(|s| s.to_string()), parameter: None }
    }

    fn data(species: &str, family: Vec<EvolutionEntry>) -> PokemonData {
        let mut d: PokemonData = serde_json::from_value(serde_json::json!({
            "species": species, "national_dex_number": 1, "base_stats": {"hp":1,"attack":1,"defense":1,"special_attack":1,"special_defense":1,"speed":1},
            "type_1": "Normal", "type_2": "Normal", "growth_rate": "medium_fast"
        }))
        .unwrap();
        d.evolution_family = family;
        d
    }

    #[test]
    fn sprite_scale_follows_solodex() {
        assert_eq!(sprite_scale(None), 1.0);
        // no family
        assert_eq!(sprite_scale(Some(&data("Tauros", vec![entry("Tauros", None)]))), 1.0);
        // a first stage (it evolves into something and nothing evolves into it)
        let bulb = data("Bulbasaur", vec![entry("Bulbasaur", None), entry("Ivysaur", Some("level")), entry("Venusaur", None)]);
        assert_eq!(sprite_scale(Some(&bulb)), 0.75);
        // a last stage: no member has a method
        let venu = data("Venusaur", vec![entry("Bulbasaur", None), entry("Ivysaur", None), entry("Venusaur", None)]);
        assert_eq!(sprite_scale(Some(&venu)), 1.0);
        // the species itself having a method (evolved from something) and others having methods: middle
        let mid = data("Ivysaur", vec![entry("Bulbasaur", None), entry("Ivysaur", Some("level")), entry("Venusaur", Some("level"))]);
        assert_eq!(sprite_scale(Some(&mid)), 0.9);
    }

    #[test]
    fn effect_labels() {
        assert_eq!(effect_label(0.0), "0x");
        assert_eq!(effect_label(0.25), "\u{bc}x");
        assert_eq!(effect_label(0.5), "\u{bd}x");
        assert_eq!(effect_label(1.0), "1x");
        assert_eq!(effect_label(2.0), "2x");
        assert_eq!(effect_label(4.0), "4x");
    }

    #[test]
    fn nature_effects() {
        // Adamant (3): +Atk -SpA; Hardy: neutral
        assert_eq!(nature_effect(Nature::from_name("Adamant").unwrap()), Some(("Atk", "SpA")));
        assert_eq!(nature_effect(Nature::from_name("Timid").unwrap()), Some(("Spe", "Atk")));
        assert_eq!(nature_effect(Nature::HARDY), None);
    }
}
