//! The spread card (Solodex `SpreadCard.tsx`): the damage view's spread
//! graphic. Artwork, identity, level and nature, then one row per stat with
//! its IV / EV (DV / Stat Exp in gens 1-2), a bar whose lighter tail is what
//! training added, and the resulting stat.
//!
//! It is a plain widget: [`show_spread_card`] takes the values (nothing is
//! derived from the species except the zero-investment baseline) and paints
//! the 1120 px design scaled to the width it is given. Solodex mounts the same
//! component offscreen for its PNG export; that export is not ported.

use std::collections::HashSet;

use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};
use xpr_dex::stats::{
    calc_gen12_stats, calc_gen3_stats, derive_hp_dv, CalcStats, Gen12Dvs, Gen12StatExps,
    Gen3Spread, NatureMods,
};
use xpr_dex::{BaseStats, StatKey};
use xpr_ui_kit::theme::Theme;

use crate::images::{paint_fit, DexImages, SpriteSize};
use crate::palette::{self, hex};

/// The width the card is designed at (Solodex: `width: 1120`).
pub const SPREAD_CARD_WIDTH: f32 = 1120.0;

/// What the card shows. Mirrors Solodex's `SpreadCardProps`.
#[derive(Clone, Debug)]
pub struct SpreadCardProps {
    pub species: String,
    pub dex_number: i32,
    pub type1: String,
    pub type2: String,
    /// the Dex game (badge boosts and the footer use it)
    pub game: String,
    pub gen: u8,
    pub level: i32,
    /// the species' base stats, for the zero-investment baseline
    pub base_stats: BaseStats,
    /// final (unboosted) stats, as the damage view's Stats fields hold them
    pub stats: CalcStats,
    pub ivs: Gen3Spread,
    pub evs: Gen3Spread,
    pub dvs: Gen12Dvs,
    pub stat_exps: Gen12StatExps,
    pub nature_name: String,
    pub nature_mods: NatureMods,
    pub held_item_name: Option<String>,
    /// badge ids (see [`all_badge_ids`]); their stat boosts are folded in
    pub badges: HashSet<String>,
    /// the user hand-edited a stat, so IV / EV no longer derive it
    pub stats_locked: bool,
}

// ---------------------------------------------------------------------------
// badge boosts (Solodex `utils/damage/badges.ts`)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum BadgeStat {
    Attack,
    Defense,
    SpAttack,
    SpDefense,
    Speed,
}

use BadgeStat::*;

type BadgeTable = &'static [(&'static str, &'static [BadgeStat])];

const GEN1_BADGES: BadgeTable = &[
    ("boulder", &[Attack]),
    ("cascade", &[]),
    ("thunder", &[Defense]),
    ("rainbow", &[]),
    ("soul", &[Speed]),
    ("marsh", &[]),
    ("volcano", &[SpAttack, SpDefense]),
    ("earth", &[]),
];

const GEN2_BADGES: BadgeTable = &[
    ("zephyr", &[Attack]),
    ("hive", &[]),
    ("plain", &[Speed]),
    ("fog", &[]),
    ("storm", &[]),
    ("mineral", &[Defense]),
    ("glacier", &[SpAttack, SpDefense]),
    ("rising", &[]),
    ("k_boulder", &[]),
    ("k_cascade", &[]),
    ("k_thunder", &[]),
    ("k_rainbow", &[]),
    ("k_soul", &[]),
    ("k_marsh", &[]),
    ("k_volcano", &[]),
    ("k_earth", &[]),
];

/// FRLG: badge 1 Attack, badge 5 Defense, badge 7 Special.
const FRLG_BADGES: BadgeTable = &[
    ("boulder", &[Attack]),
    ("cascade", &[]),
    ("thunder", &[]),
    ("rainbow", &[]),
    ("soul", &[Defense]),
    ("marsh", &[]),
    ("volcano", &[SpAttack, SpDefense]),
    ("earth", &[]),
];

const RSE_BADGES: BadgeTable = &[
    ("stone", &[Attack]),
    ("knuckle", &[]),
    ("dynamo", &[Speed]),
    ("heat", &[]),
    ("balance", &[Defense]),
    ("feather", &[]),
    ("mind", &[SpAttack, SpDefense]),
    ("rain", &[]),
];

fn badge_table(game: &str) -> Option<BadgeTable> {
    match game {
        "Red and Blue" | "Yellow" => Some(GEN1_BADGES),
        "Gold and Silver" | "Crystal" => Some(GEN2_BADGES),
        "Ruby and Sapphire" | "Emerald" => Some(RSE_BADGES),
        "FireRed and LeafGreen" => Some(FRLG_BADGES),
        _ => None,
    }
}

/// 1, 2 or 3 for the games with badge stat boosts, else 0.
fn badge_gen(game: &str) -> u8 {
    match game {
        "Red and Blue" | "Yellow" => 1,
        "Gold and Silver" | "Crystal" => 2,
        g if badge_table(g).is_some() => 3,
        _ => 0,
    }
}

/// Every badge id of a game (the damage view's default: all obtained).
pub fn all_badge_ids(game: &str) -> HashSet<String> {
    badge_table(game)
        .map(|t| t.iter().map(|(id, _)| id.to_string()).collect())
        .unwrap_or_default()
}

fn badge_boosts_stat(stat: BadgeStat, badges: &HashSet<String>, game: &str) -> bool {
    let Some(table) = badge_table(game) else {
        return false;
    };
    !badges.is_empty()
        && table
            .iter()
            .any(|(id, stats)| badges.contains(*id) && stats.contains(&stat))
}

/// Gen 1-2: `stat + (stat >> 3)` capped at 999; gen 3: `floor(stat * 110 / 100)`.
fn apply_badge_stat_boost(
    value: i32,
    stat: BadgeStat,
    badges: &HashSet<String>,
    game: &str,
) -> i32 {
    if !badge_boosts_stat(stat, badges, game) {
        return value;
    }
    if badge_gen(game) == 3 {
        value * 110 / 100
    } else {
        (value + (value >> 3)).min(999)
    }
}

// ---------------------------------------------------------------------------
// rows
// ---------------------------------------------------------------------------

/// The `CalcStats` field a card row shows.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Row {
    Hp,
    Attack,
    Defense,
    SpAttack,
    SpDefense,
    Speed,
}

impl Row {
    fn stat_key(self) -> StatKey {
        match self {
            Row::Hp => StatKey::Hp,
            Row::Attack => StatKey::Attack,
            Row::Defense => StatKey::Defense,
            Row::SpAttack => StatKey::SpecialAttack,
            Row::SpDefense => StatKey::SpecialDefense,
            Row::Speed => StatKey::Speed,
        }
    }

    fn label(self, gen1: bool) -> &'static str {
        palette::stat_label(self.stat_key(), gen1)
    }

    fn value(self, s: &CalcStats) -> i32 {
        match self {
            Row::Hp => s.hp,
            Row::Attack => s.attack,
            Row::Defense => s.defense,
            Row::SpAttack => s.spattack,
            Row::SpDefense => s.spdefense,
            Row::Speed => s.speed,
        }
    }

    fn badge_stat(self) -> Option<BadgeStat> {
        match self {
            Row::Hp => None,
            Row::Attack => Some(Attack),
            Row::Defense => Some(Defense),
            Row::SpAttack => Some(SpAttack),
            Row::SpDefense => Some(SpDefense),
            Row::Speed => Some(Speed),
        }
    }

    fn nature_mod(self, m: &NatureMods) -> f64 {
        match self {
            Row::Hp => 1.0,
            Row::Attack => m.attack,
            Row::Defense => m.defense,
            Row::SpAttack => m.spattack,
            Row::SpDefense => m.spdefense,
            Row::Speed => m.speed,
        }
    }
}

const ROWS_GEN1: [Row; 5] = [
    Row::Hp,
    Row::Attack,
    Row::Defense,
    Row::SpAttack,
    Row::Speed,
];
const ROWS_MODERN: [Row; 6] = [
    Row::Hp,
    Row::Attack,
    Row::Defense,
    Row::SpAttack,
    Row::SpDefense,
    Row::Speed,
];

/// `[Atk, Def, SpA, SpD, Spe]` of the nature, with the multiplier.
fn nature_stats(m: &NatureMods) -> [(&'static str, f64); 5] {
    [
        ("Atk", m.attack),
        ("Def", m.defense),
        ("SpA", m.spattack),
        ("SpD", m.spdefense),
        ("Spe", m.speed),
    ]
}

/// Solodex `splitFormName`: "Meloetta (Aria)" -> ("Meloetta", Some("(Aria)")).
fn split_form_name(name: &str) -> (&str, Option<&str>) {
    if name.ends_with(')') {
        // `^(.+?)\s*(\(.+\))$`: the form starts at the first "(" after at least one character
        if let Some((i, _)) = name
            .char_indices()
            .skip(1)
            .find(|(i, c)| *c == '(' && name.len() - *i >= 3)
        {
            let base = name[..i].trim_end();
            if !base.is_empty() {
                return (base, Some(&name[i..]));
            }
        }
    }
    (name, None)
}

/// Blend a colour toward white (Solodex `lighten`).
fn lighten(c: Color32, amount: f32) -> Color32 {
    let mix = |v: u8| (v as f32 + (255.0 - v as f32) * amount).round() as u8;
    Color32::from_rgb(mix(c.r()), mix(c.g()), mix(c.b()))
}

fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, (a * 255.0).round() as u8)
}

// ---------------------------------------------------------------------------
// design metrics (CSS px of the 1120 px card)
// ---------------------------------------------------------------------------

const PAD_X: f32 = 40.0;
const PAD_TOP: f32 = 34.0;
const PAD_BOTTOM: f32 = 24.0;
const ART: f32 = 172.0;
const HEADER_GAP: f32 = 26.0;
const PILL_W: f32 = 176.0;
const CELL_LABEL: f32 = 78.0;
const CELL_IV: f32 = 52.0;
const CELL_EV: f32 = 66.0;
const CELL_VALUE: f32 = 96.0;
const CELL_GAP: f32 = 14.0;
const BAR_H: f32 = 16.0;
const BAR_RADIUS: f32 = 2.0;
const ROW_H: f32 = 30.8;
const ROW_GAP: f32 = 9.0;
const HEADING_H: f32 = 12.0;
const NATURE_UP: Color32 = Color32::from_rgb(0x60, 0xa5, 0xfa);
const NATURE_DOWN: Color32 = Color32::from_rgb(0xf8, 0x71, 0x71);

struct Metrics {
    rows: &'static [Row],
    pills_h: f32,
    header_h: f32,
    table_top: f32,
    footer_top: f32,
    total: f32,
}

fn metrics(props: &SpreadCardProps) -> Metrics {
    let gen12 = props.gen <= 2;
    let rows: &'static [Row] = if props.gen <= 1 {
        &ROWS_GEN1
    } else {
        &ROWS_MODERN
    };
    let level_pill = 9.0 + 12.0 + 38.0 * 1.1 + 11.0 + 2.0;
    let second_pill = if gen12 {
        9.0 + 12.0 + 26.0 * 1.3 + 11.0 + 2.0
    } else {
        9.0 + 12.0 + 22.0 * 1.35 + 4.0 + 20.0 + 11.0 + 2.0
    };
    let pills_h = level_pill + 10.0 + second_pill;
    let header_h = ART.max(pills_h);
    let divider_top = PAD_TOP + header_h + 26.0;
    let table_top = divider_top + 1.0 + 16.0;
    let table_h = HEADING_H + rows.len() as f32 * (ROW_GAP + ROW_H);
    let footer_top = table_top + table_h + 20.0;
    let total = footer_top + 1.0 + 13.0 + 16.0 + PAD_BOTTOM;
    Metrics {
        rows,
        pills_h,
        header_h,
        table_top,
        footer_top,
        total,
    }
}

/// The card's height when drawn `width` wide.
pub fn spread_card_height(props: &SpreadCardProps, width: f32) -> f32 {
    metrics(props).total * width / SPREAD_CARD_WIDTH
}

// ---------------------------------------------------------------------------
// painting
// ---------------------------------------------------------------------------

/// Paints in design px, scaled by `k` from `origin`.
struct Canvas<'a> {
    ui: &'a Ui,
    theme: &'a Theme,
    origin: Pos2,
    k: f32,
}

impl Canvas<'_> {
    fn pt(&self, x: f32, y: f32) -> Pos2 {
        self.origin + Vec2::new(x * self.k, y * self.k)
    }

    fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(self.pt(x, y), Vec2::new(w * self.k, h * self.k))
    }

    fn radius(&self, r: f32) -> CornerRadius {
        CornerRadius::same((r * self.k).round().clamp(0.0, 255.0) as u8)
    }

    fn font(&self, size: f32, bold: bool) -> FontId {
        if bold {
            palette::px_bold(self.theme, size * self.k)
        } else {
            palette::px(self.theme, size * self.k)
        }
    }

    fn width(&self, text: &str, font: &FontId) -> f32 {
        crate::widgets::text_w(self.ui, text, font)
    }

    /// Text with its vertical centre at design `y`; returns its width in screen px.
    #[allow(clippy::too_many_arguments)]
    fn text(
        &self,
        x: f32,
        y: f32,
        align: Align2,
        text: &str,
        size: f32,
        bold: bool,
        color: Color32,
    ) -> f32 {
        let font = self.font(size, bold);
        let w = self.width(text, &font);
        self.ui
            .painter()
            .text(self.pt(x, y), align, text, font, color);
        w
    }

    /// Text with CSS `letter-spacing` (also after the last letter, like CSS).
    #[allow(clippy::too_many_arguments)]
    fn tracked(
        &self,
        x: f32,
        y: f32,
        halign: egui::Align,
        text: &str,
        size: f32,
        color: Color32,
        tracking: f32,
    ) -> f32 {
        let mut job = egui::text::LayoutJob::default();
        job.append(
            text,
            0.0,
            egui::text::TextFormat {
                font_id: self.font(size, true),
                color,
                extra_letter_spacing: tracking * self.k,
                ..Default::default()
            },
        );
        let galley = self.ui.fonts_mut(|f| f.layout_job(job));
        // CSS also spaces after the last letter
        let total = galley.size().x + tracking * self.k;
        let at = self.pt(x, y);
        let left = match halign {
            egui::Align::Min => at.x,
            egui::Align::Center => at.x - total / 2.0,
            egui::Align::Max => at.x - total,
        };
        self.ui
            .painter()
            .galley(Pos2::new(left, at.y - galley.size().y / 2.0), galley, color);
        total
    }

    fn filled(
        &self,
        rect: Rect,
        radius: CornerRadius,
        fill: Color32,
        stroke: Option<(f32, Color32)>,
    ) {
        match stroke {
            Some((w, c)) => self.ui.painter().rect(
                rect,
                radius,
                fill,
                Stroke::new(w, c),
                egui::StrokeKind::Inside,
            ),
            None => self.ui.painter().rect_filled(rect, radius, fill),
        };
    }

    /// A vertical sheen (`linear-gradient(180deg, rgba(255,255,255,.18), transparent)`) over `rect`.
    fn sheen(&self, rect: Rect, opacity: f32) {
        let top = Color32::from_white_alpha((0.18 * opacity * 255.0).round() as u8);
        let bottom = Color32::from_white_alpha(0);
        let mut mesh = Mesh::default();
        for (p, c) in [
            (rect.left_top(), top),
            (rect.right_top(), top),
            (rect.right_bottom(), bottom),
            (rect.left_bottom(), bottom),
        ] {
            mesh.vertices.push(Vertex {
                pos: p,
                uv: WHITE_UV,
                color: c,
            });
        }
        mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        self.ui.painter().add(egui::Shape::mesh(mesh));
    }
}

/// `linear-gradient(155deg, #1d2533 0%, #151b26 46%, #10141c 100%)` at a point of the card.
fn card_gradient(p: Vec2, size: Vec2) -> Color32 {
    let stops = [
        (0.0_f32, [0x1d, 0x25, 0x33]),
        (0.46, [0x15, 0x1b, 0x26]),
        (1.0, [0x10, 0x14, 0x1c]),
    ];
    // CSS: angle 155deg points down and slightly right; the gradient line passes through the centre
    let a = 155.0_f32.to_radians();
    let dir = Vec2::new(a.sin(), -a.cos());
    let len = (size.x * dir.x.abs() + size.y * dir.y.abs()).max(1.0);
    let t = (((p - size / 2.0).dot(dir)) / len + 0.5).clamp(0.0, 1.0);
    let (i, _) = stops
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, (s, _))| t <= *s)
        .unwrap_or((2, &stops[2]));
    let (t0, c0) = stops[i - 1];
    let (t1, c1) = stops[i];
    let f = ((t - t0) / (t1 - t0)).clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * f).round() as u8;
    Color32::from_rgb(mix(c0[0], c1[0]), mix(c0[1], c1[1]), mix(c0[2], c1[2]))
}

/// The outline of a rounded rectangle as a closed polygon (clockwise from the top-left arc's start).
fn rounded_outline(rect: Rect, r: f32) -> Vec<Pos2> {
    let steps = 10;
    let mut pts = Vec::new();
    let corners = [
        (Pos2::new(rect.min.x + r, rect.min.y + r), 180.0_f32),
        (Pos2::new(rect.max.x - r, rect.min.y + r), 270.0),
        (Pos2::new(rect.max.x - r, rect.max.y - r), 0.0),
        (Pos2::new(rect.min.x + r, rect.max.y - r), 90.0),
    ];
    for (c, start) in corners {
        for i in 0..=steps {
            let a = (start + 90.0 * i as f32 / steps as f32).to_radians();
            pts.push(Pos2::new(c.x + r * a.cos(), c.y + r * a.sin()));
        }
    }
    pts
}

fn paint_card_background(cv: &Canvas, rect: Rect, radius: f32) {
    let outline = rounded_outline(rect, radius);
    let size = rect.size();
    let mut mesh = Mesh::default();
    let center = rect.center();
    mesh.vertices.push(Vertex {
        pos: center,
        uv: WHITE_UV,
        color: card_gradient(Vec2::new(size.x / 2.0, size.y / 2.0), size),
    });
    for p in &outline {
        mesh.vertices.push(Vertex {
            pos: *p,
            uv: WHITE_UV,
            color: card_gradient(*p - rect.min, size),
        });
    }
    let n = outline.len() as u32;
    for i in 0..n {
        mesh.indices.extend_from_slice(&[0, 1 + i, 1 + (i + 1) % n]);
    }
    // a few interior vertices keep the 46 % stop's kink close to exact
    cv.ui.painter().add(egui::Shape::mesh(mesh));
}

/// A horizontal gradient strip along the card's top edge, following its rounded corners.
fn paint_top_bar(cv: &Canvas, rect: Rect, radius: f32, left: Color32, right: Color32, height: f32) {
    let rows = 6;
    let mut mesh = Mesh::default();
    let color_at = |x: f32| {
        let t = ((x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Color32::from_rgba_unmultiplied(
            mix(left.r(), right.r()),
            mix(left.g(), right.g()),
            mix(left.b(), right.b()),
            (0.9 * 255.0) as u8,
        )
    };
    for i in 0..=rows {
        let y = height * i as f32 / rows as f32;
        // the rounded corner's inset at this depth
        let inset = if y < radius {
            radius
                - (radius * radius - (radius - y) * (radius - y))
                    .max(0.0)
                    .sqrt()
        } else {
            0.0
        };
        let (xl, xr) = (rect.min.x + inset, rect.max.x - inset);
        mesh.vertices.push(Vertex {
            pos: Pos2::new(xl, rect.min.y + y),
            uv: WHITE_UV,
            color: color_at(xl),
        });
        mesh.vertices.push(Vertex {
            pos: Pos2::new(xr, rect.min.y + y),
            uv: WHITE_UV,
            color: color_at(xr),
        });
    }
    for i in 0..rows as u32 {
        let a = i * 2;
        mesh.indices
            .extend_from_slice(&[a, a + 1, a + 3, a, a + 3, a + 2]);
    }
    cv.ui.painter().add(egui::Shape::mesh(mesh));
}

/// `radial-gradient(circle closest-side, c 4d, c 22 at 45%, c 00 at 78%)`.
fn paint_glow(cv: &Canvas, center: Pos2, radius: f32, color: Color32) {
    let steps = 48;
    let mut mesh = Mesh::default();
    let c = |a: u8| Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), a);
    mesh.vertices.push(Vertex {
        pos: center,
        uv: WHITE_UV,
        color: c(0x4d),
    });
    for (frac, alpha) in [(0.45_f32, 0x22_u8), (0.78, 0x00)] {
        for i in 0..steps {
            let a = std::f32::consts::TAU * i as f32 / steps as f32;
            mesh.vertices.push(Vertex {
                pos: center + Vec2::new(a.cos(), a.sin()) * radius * frac,
                uv: WHITE_UV,
                color: c(alpha),
            });
        }
    }
    let n = steps as u32;
    for i in 0..n {
        let j = (i + 1) % n;
        mesh.indices.extend_from_slice(&[0, 1 + i, 1 + j]);
        mesh.indices.extend_from_slice(&[1 + i, 1 + j, 1 + n + j]);
        mesh.indices
            .extend_from_slice(&[1 + i, 1 + n + j, 1 + n + i]);
    }
    cv.ui.painter().add(egui::Shape::mesh(mesh));
}

/// The card, drawn `width` wide (its design is 1120). Returns the card's response.
pub fn show_spread_card(
    ui: &mut Ui,
    theme: &Theme,
    images: &mut DexImages,
    props: &SpreadCardProps,
    width: f32,
) -> Response {
    let m = metrics(props);
    let k = width / SPREAD_CARD_WIDTH;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, m.total * k), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return resp;
    }
    let cv = Canvas {
        ui,
        theme,
        origin: rect.min,
        k,
    };
    let gen12 = props.gen <= 2;
    let gen1 = props.gen <= 1;
    let rows = m.rows;
    let is_dual = props.type1 != props.type2;
    let accent1 = palette::type_color(&props.type1);
    let accent2 = palette::type_color(if is_dual { &props.type2 } else { &props.type1 });

    // ---- derived values
    let with_badges = |row: Row, v: i32| match row.badge_stat() {
        Some(b) => apply_badge_stat_boost(v, b, &props.badges, &props.game),
        None => v,
    };
    let final_of = |row: Row| with_badges(row, row.value(&props.stats));
    let zero_invest = if gen12 {
        calc_gen12_stats(
            &props.base_stats,
            props.level,
            &props.dvs,
            &Gen12StatExps::default(),
        )
    } else {
        calc_gen3_stats(
            &props.base_stats,
            props.level,
            &props.ivs,
            &Gen3Spread::ZERO,
            &props.nature_mods,
            Some(&props.species),
        )
    };
    let values: Vec<i32> = rows.iter().map(|r| final_of(*r)).collect();
    let max_value = values.iter().copied().max().unwrap_or(0) as f32;
    let scale_max = 10.0_f32.max(((max_value * 1.08) / 10.0).ceil() * 10.0);
    // (iv, iv max, ev, ev max)
    let invest_of = |row: Row| -> (i32, i32, i32, i32) {
        if gen12 {
            let dv = match row {
                Row::Hp => derive_hp_dv(&props.dvs),
                Row::SpAttack | Row::SpDefense => props.dvs.special,
                Row::Attack => props.dvs.attack,
                Row::Defense => props.dvs.defense,
                Row::Speed => props.dvs.speed,
            };
            let exp = match row {
                Row::Hp => props.stat_exps.hp,
                Row::SpAttack | Row::SpDefense => props.stat_exps.special,
                Row::Attack => props.stat_exps.attack,
                Row::Defense => props.stat_exps.defense,
                Row::Speed => props.stat_exps.speed,
            };
            (dv, 15, exp, 65535)
        } else {
            let pick = |s: &Gen3Spread| match row {
                Row::Hp => s.hp,
                Row::Attack => s.attack,
                Row::Defense => s.defense,
                Row::SpAttack => s.spattack,
                Row::SpDefense => s.spdefense,
                Row::Speed => s.speed,
            };
            (pick(&props.ivs), 31, pick(&props.evs), 252)
        }
    };
    let nature_up = nature_stats(&props.nature_mods)
        .into_iter()
        .find(|(_, v)| *v > 1.0)
        .map(|(l, _)| l);
    let nature_down = nature_stats(&props.nature_mods)
        .into_iter()
        .find(|(_, v)| *v < 1.0)
        .map(|(l, _)| l);
    let badge_boosted = rows.iter().any(|r| final_of(*r) != r.value(&props.stats));
    let show_ev_tail = !props.stats_locked;
    let invest_total = if gen12 {
        props.dvs.attack
            + props.dvs.defense
            + props.dvs.speed
            + props.dvs.special
            + derive_hp_dv(&props.dvs)
    } else {
        props.evs.hp
            + props.evs.attack
            + props.evs.defense
            + props.evs.spattack
            + props.evs.spdefense
            + props.evs.speed
    };
    let invest_label = if gen12 {
        format!("DVs {}/75", invest_total)
    } else {
        format!("EVs {}/510", invest_total)
    };
    let any_investment = rows.iter().any(|r| invest_of(*r).2 > 0);

    // ---- card body
    let radius = 28.0 * k;
    paint_card_background(&cv, rect, radius);
    cv.filled(
        rect,
        cv.radius(28.0),
        Color32::TRANSPARENT,
        Some(((1.0_f32 * k).max(1.0), rgba(148, 163, 184, 0.16))),
    );
    // inset highlight
    ui.painter().hline(
        rect.min.x + radius..=rect.max.x - radius,
        rect.min.y + (1.5 * k).max(1.5),
        Stroke::new(1.0_f32, Color32::from_white_alpha(13)),
    );
    paint_top_bar(&cv, rect, radius, accent1, accent2, 5.0 * k);

    // ---- header: artwork / identity / level and nature
    let header_cy = PAD_TOP + m.header_h / 2.0;
    let art = cv.rect(PAD_X, header_cy - ART / 2.0, ART, ART);
    paint_glow(&cv, art.center(), 112.0 * k, accent1);
    if let Some(tex) = images.sprite(ui.ctx(), &props.species, props.dex_number, SpriteSize::Full) {
        // one soft, downward shadow (`drop-shadow(0 14px 12px rgba(0,0,0,.55))`)
        let sh = art.translate(Vec2::new(0.0, 14.0 * k));
        let offsets = std::iter::once(Vec2::ZERO)
            .chain((0..8).map(|i| Vec2::angled(std::f32::consts::TAU * i as f32 / 8.0) * 4.0 * k))
            .chain(
                (0..8).map(|i| {
                    Vec2::angled(std::f32::consts::TAU * (i as f32 + 0.5) / 8.0) * 8.0 * k
                }),
            );
        for o in offsets {
            paint_fit(ui, &tex, sh.translate(o), Color32::from_black_alpha(9));
        }
        paint_fit(ui, &tex, art, Color32::WHITE);
    }

    let name = xpr_dex::display_name(&props.species);
    let (name_base, name_form) = split_form_name(name);
    let id_x = PAD_X + ART + HEADER_GAP;
    let id_w = SPREAD_CARD_WIDTH - PAD_X - id_x - PILL_W - HEADER_GAP;
    let id_h =
        14.0 + 2.0 + 44.0 * 1.08 + if name_form.is_some() { 20.0 * 1.2 } else { 0.0 } + 12.0 + 25.0;
    let mut y = header_cy - id_h / 2.0;
    cv.tracked(
        id_x,
        y + 7.0,
        egui::Align::Min,
        &format!("#{:04}", props.dex_number),
        12.0,
        hex("#64748b"),
        2.0,
    );
    y += 14.0 + 2.0;
    {
        let font = cv.font(44.0, true);
        let shown = xpr_ui_kit::widgets::elide(ui, name_base, &font, id_w * k);
        ui.painter().text(
            cv.pt(id_x, y + 44.0 * 1.08 / 2.0),
            Align2::LEFT_CENTER,
            shown,
            font,
            hex("#f8fafc"),
        );
    }
    y += 44.0 * 1.08;
    if let Some(form) = name_form {
        cv.text(
            id_x,
            y + 12.0,
            Align2::LEFT_CENTER,
            form,
            20.0,
            true,
            hex("#94a3b8"),
        );
        y += 24.0;
    }
    y += 12.0;
    let mut chip_x = id_x;
    for (i, t) in [
        Some(&props.type1),
        if is_dual { Some(&props.type2) } else { None },
    ]
    .into_iter()
    .flatten()
    .enumerate()
    {
        let font = cv.font(14.0, true);
        let w = (cv.width(t, &font) / k + 24.0).max(84.0);
        let r = cv.rect(chip_x, y, w, 25.0);
        cv.filled(r, cv.radius(6.0), palette::type_color(t), None);
        crate::widgets::paint_shadowed_text(ui, r, t, font, Color32::WHITE);
        chip_x += w + 8.0;
        let _ = i;
    }

    // pills
    let pill_x = SPREAD_CARD_WIDTH - PAD_X - PILL_W;
    let mut py = header_cy - m.pills_h / 2.0;
    let pill = |py: f32, h: f32| {
        let r = cv.rect(pill_x, py, PILL_W, h);
        cv.filled(
            r,
            cv.radius(14.0),
            rgba(148, 163, 184, 0.07),
            Some(((1.0_f32 * k).max(1.0), rgba(148, 163, 184, 0.14))),
        );
    };
    let right_x = pill_x + PILL_W - 1.0 - 16.0;
    // LEVEL
    let level_h = 9.0 + 12.0 + 38.0 * 1.1 + 11.0 + 2.0;
    pill(py, level_h);
    cv.tracked(
        right_x,
        py + 1.0 + 9.0 + 6.0,
        egui::Align::Max,
        "LEVEL",
        10.0,
        hex("#6b7280"),
        1.5,
    );
    cv.text(
        right_x,
        py + 1.0 + 9.0 + 12.0 + 38.0 * 1.1 / 2.0,
        Align2::RIGHT_CENTER,
        &props.level.to_string(),
        38.0,
        true,
        hex("#f8fafc"),
    );
    py += level_h + 10.0;
    if gen12 {
        let h = 9.0 + 12.0 + 26.0 * 1.3 + 11.0 + 2.0;
        pill(py, h);
        cv.tracked(
            right_x,
            py + 1.0 + 9.0 + 6.0,
            egui::Align::Max,
            "HP DV",
            10.0,
            hex("#6b7280"),
            1.5,
        );
        let cy = py + 1.0 + 9.0 + 12.0 + 26.0 * 1.3 / 2.0;
        let suffix_w = cv.text(
            right_x,
            cy + 1.0,
            Align2::RIGHT_CENTER,
            " /15",
            14.0,
            true,
            hex("#64748b"),
        );
        cv.text(
            right_x - suffix_w / k,
            cy,
            Align2::RIGHT_CENTER,
            &derive_hp_dv(&props.dvs).to_string(),
            26.0,
            true,
            hex("#f8fafc"),
        );
    } else {
        let h = 9.0 + 12.0 + 22.0 * 1.35 + 4.0 + 20.0 + 11.0 + 2.0;
        pill(py, h);
        cv.tracked(
            right_x,
            py + 1.0 + 9.0 + 6.0,
            egui::Align::Max,
            "NATURE",
            10.0,
            hex("#6b7280"),
            1.5,
        );
        let ny = py + 1.0 + 9.0 + 12.0;
        cv.text(
            right_x,
            ny + 22.0 * 1.35 / 2.0,
            Align2::RIGHT_CENTER,
            &props.nature_name,
            22.0,
            true,
            hex("#f8fafc"),
        );
        let cy = ny + 22.0 * 1.35 + 4.0;
        match (nature_up, nature_down) {
            (Some(up), Some(down)) => {
                let chip = |x_right: f32, up_chip: bool, label: &str| -> f32 {
                    let text = format!("{}{}", if up_chip { "+" } else { "\u{2212}" }, label);
                    let font = cv.font(12.0, true);
                    let w = cv.width(&text, &font) / k + 16.0 + 2.0;
                    let r = cv.rect(x_right - w, cy, w, 20.0);
                    let (fg, bg, border) = if up_chip {
                        (
                            hex("#93c5fd"),
                            rgba(59, 130, 246, 0.16),
                            rgba(59, 130, 246, 0.32),
                        )
                    } else {
                        (
                            hex("#fca5a5"),
                            rgba(239, 68, 68, 0.14),
                            rgba(239, 68, 68, 0.3),
                        )
                    };
                    cv.filled(
                        r,
                        cv.radius(5.0),
                        bg,
                        Some(((1.0_f32 * k).max(1.0), border)),
                    );
                    ui.painter()
                        .text(r.center(), Align2::CENTER_CENTER, text, font, fg);
                    w
                };
                let w_down = chip(right_x, false, down);
                chip(right_x - w_down - 5.0, true, up);
            }
            _ => {
                cv.text(
                    right_x,
                    cy + 10.0,
                    Align2::RIGHT_CENTER,
                    "neutral",
                    12.0,
                    true,
                    hex("#64748b"),
                );
            }
        }
    }

    // ---- divider
    let div_y = PAD_TOP + m.header_h + 26.0;
    {
        let r = cv.rect(PAD_X, div_y, SPREAD_CARD_WIDTH - 2.0 * PAD_X, 1.0);
        let steps = [(0.0_f32, 0.0_f32), (0.12, 0.24), (0.88, 0.24), (1.0, 0.0)];
        let mut mesh = Mesh::default();
        for (t, a) in steps {
            let x = r.min.x + r.width() * t;
            let c = rgba(148, 163, 184, a);
            mesh.vertices.push(Vertex {
                pos: Pos2::new(x, r.min.y),
                uv: WHITE_UV,
                color: c,
            });
            mesh.vertices.push(Vertex {
                pos: Pos2::new(x, r.min.y + (1.0 * k).max(1.0)),
                uv: WHITE_UV,
                color: c,
            });
        }
        for i in 0..3_u32 {
            let a = i * 2;
            mesh.indices
                .extend_from_slice(&[a, a + 1, a + 3, a, a + 3, a + 2]);
        }
        ui.painter().add(egui::Shape::mesh(mesh));
    }

    // ---- stat table
    let x_iv = PAD_X + CELL_LABEL + CELL_GAP;
    let x_ev = x_iv + CELL_IV + CELL_GAP;
    let x_bar = x_ev + CELL_EV + CELL_GAP;
    let x_value = SPREAD_CARD_WIDTH - PAD_X - CELL_VALUE;
    let bar_w = x_value - CELL_GAP - x_bar;
    let heading_cy = m.table_top + HEADING_H / 2.0;
    let heading = hex("#64748b");
    cv.tracked(
        x_iv + CELL_IV / 2.0,
        heading_cy,
        egui::Align::Center,
        if gen12 { "DV" } else { "IV" },
        10.0,
        heading,
        1.4,
    );
    cv.tracked(
        x_ev + CELL_EV / 2.0,
        heading_cy,
        egui::Align::Center,
        if gen12 { "EXP" } else { "EV" },
        10.0,
        heading,
        1.4,
    );
    cv.tracked(
        SPREAD_CARD_WIDTH - PAD_X,
        heading_cy,
        egui::Align::Max,
        "STAT",
        10.0,
        heading,
        1.4,
    );

    let mut ry = m.table_top + HEADING_H + ROW_GAP;
    for row in rows {
        let color = palette::stat_color(row.stat_key(), gen1);
        let value = final_of(*row);
        let (iv, iv_max, ev, ev_max) = invest_of(*row);
        let nat = row.nature_mod(&props.nature_mods);
        let cy = ry + ROW_H / 2.0;
        // label with the nature marker
        let label = row.label(gen1).to_uppercase();
        {
            let w = cv.tracked(PAD_X, cy, egui::Align::Min, &label, 16.0, color, 0.6);
            if nat != 1.0 {
                let (glyph, c) = if nat > 1.0 {
                    ("\u{25B2}", NATURE_UP)
                } else {
                    ("\u{25BC}", NATURE_DOWN)
                };
                ui.painter().text(
                    Pos2::new(cv.pt(PAD_X, cy).x + w + 5.0 * k, cv.pt(PAD_X, cy).y),
                    Align2::LEFT_CENTER,
                    glyph,
                    cv.font(11.0, false),
                    c,
                );
            }
        }
        let iv_color = if iv >= iv_max {
            hex("#f1f5f9")
        } else if iv == 0 {
            hex("#475569")
        } else {
            hex("#94a3b8")
        };
        cv.text(
            x_iv + CELL_IV / 2.0,
            cy,
            Align2::CENTER_CENTER,
            &iv.to_string(),
            15.0,
            true,
            iv_color,
        );
        let ev_color = if ev >= ev_max {
            color
        } else if ev == 0 {
            hex("#475569")
        } else {
            hex("#cbd5e1")
        };
        cv.text(
            x_ev + CELL_EV / 2.0,
            cy,
            Align2::CENTER_CENTER,
            &ev.to_string(),
            15.0,
            true,
            ev_color,
        );

        // bar: solid up to the zero-investment value, a lighter tail for what training added
        let track = cv.rect(x_bar, cy - BAR_H / 2.0, bar_w, BAR_H);
        cv.filled(track, cv.radius(BAR_RADIUS), rgba(55, 65, 81, 0.8), None);
        let total_pct = ((value as f32 / scale_max) * 100.0).min(100.0);
        let zero_value = {
            let z = match row {
                Row::Hp => zero_invest.hp,
                Row::Attack => zero_invest.attack,
                Row::Defense => zero_invest.defense,
                Row::SpAttack => zero_invest.spattack,
                Row::SpDefense => zero_invest.spdefense,
                Row::Speed => zero_invest.speed,
            };
            with_badges(*row, z)
        };
        let base_pct = if show_ev_tail {
            total_pct.min((zero_value as f32 / scale_max) * 100.0)
        } else {
            total_pct
        };
        let tail_pct = (total_pct - base_pct).max(0.0);
        let r = BAR_RADIUS * k;
        let rr = (r.round().clamp(0.0, 255.0)) as u8;
        let base_rect = Rect::from_min_size(
            track.min,
            Vec2::new(track.width() * base_pct / 100.0, track.height()),
        );
        if base_rect.width() > 0.0 {
            let radius = if tail_pct > 0.0 {
                CornerRadius {
                    nw: rr,
                    sw: rr,
                    ne: 0,
                    se: 0,
                }
            } else {
                CornerRadius::same(rr)
            };
            ui.painter().rect_filled(
                base_rect,
                radius,
                Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 230),
            );
            cv.sheen(base_rect, 0.9);
        }
        if tail_pct > 0.0 {
            let tail_rect = Rect::from_min_size(
                Pos2::new(base_rect.max.x, track.min.y),
                Vec2::new(track.width() * tail_pct / 100.0, track.height()),
            );
            let lc = lighten(color, 0.5);
            ui.painter().rect_filled(
                tail_rect,
                CornerRadius {
                    nw: 0,
                    sw: 0,
                    ne: rr,
                    se: rr,
                },
                Color32::from_rgba_unmultiplied(lc.r(), lc.g(), lc.b(), 230),
            );
            cv.sheen(tail_rect, 0.9);
        }
        cv.text(
            x_value + CELL_VALUE,
            cy,
            Align2::RIGHT_CENTER,
            &value.to_string(),
            28.0,
            true,
            hex("#f8fafc"),
        );
        ry += ROW_H + ROW_GAP;
    }

    // ---- footer
    let line_y = m.footer_top;
    ui.painter().hline(
        cv.rect(PAD_X, line_y, SPREAD_CARD_WIDTH - 2.0 * PAD_X, 1.0)
            .x_range(),
        cv.pt(0.0, line_y).y + 0.5,
        Stroke::new(1.0_f32, rgba(148, 163, 184, 0.12)),
    );
    let text_cy = line_y + 1.0 + 13.0 + 8.0;
    let mut bits: Vec<String> = vec![
        props.game.clone(),
        format!("Lv {}", props.level),
        invest_label,
    ];
    if !gen12 {
        bits.push(props.nature_name.clone());
    }
    if let Some(item) = &props.held_item_name {
        bits.push(item.clone());
    }
    if badge_boosted {
        bits.push("incl. badge boosts".to_string());
    }
    if props.stats_locked {
        bits.push("stats set manually".to_string());
    }
    let mut x = cv.pt(PAD_X, text_cy).x;
    let fy = cv.pt(PAD_X, text_cy).y;
    for (i, bit) in bits.iter().enumerate() {
        if i > 0 {
            let w = cv.width(" \u{b7} ", &cv.font(13.0, false));
            ui.painter().text(
                Pos2::new(x, fy),
                Align2::LEFT_CENTER,
                " \u{b7} ",
                cv.font(13.0, false),
                hex("#3f4a5a"),
            );
            x += w;
        }
        let font = cv.font(13.0, i == 0);
        let w = cv.width(bit, &font);
        ui.painter().text(
            Pos2::new(x, fy),
            Align2::LEFT_CENTER,
            bit,
            font,
            if i == 0 {
                hex("#94a3b8")
            } else {
                hex("#64748b")
            },
        );
        x += w;
    }
    if show_ev_tail && any_investment {
        let label = if gen12 { "Stat Exp gain" } else { "EV gain" };
        let font = cv.font(13.0, false);
        let w = cv.width(label, &font);
        let right = cv.pt(SPREAD_CARD_WIDTH - PAD_X, text_cy).x;
        ui.painter().text(
            Pos2::new(right, fy),
            Align2::RIGHT_CENTER,
            label,
            font,
            hex("#64748b"),
        );
        let sw = Rect::from_min_size(
            Pos2::new(right - w - 6.0 * k - 22.0 * k, fy - 4.5 * k),
            Vec2::new(22.0 * k, 9.0 * k),
        );
        ui.painter()
            .rect_filled(sw, cv.radius(BAR_RADIUS), lighten(hex("#94a3b8"), 0.5));
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_names_split() {
        assert_eq!(
            split_form_name("Meloetta (Aria)"),
            ("Meloetta", Some("(Aria)"))
        );
        assert_eq!(split_form_name("Pikachu"), ("Pikachu", None));
        assert_eq!(split_form_name("Mr. Mime"), ("Mr. Mime", None));
        assert_eq!(
            split_form_name("Zacian (Crowned Sword)"),
            ("Zacian", Some("(Crowned Sword)"))
        );
    }

    #[test]
    fn badge_boosts() {
        let all = all_badge_ids("Red and Blue");
        // gen 1: stat + stat / 8, capped at 999
        assert_eq!(
            apply_badge_stat_boost(100, Attack, &all, "Red and Blue"),
            112
        );
        assert_eq!(
            apply_badge_stat_boost(990, Attack, &all, "Red and Blue"),
            999
        );
        // HP-less rows and other games
        assert_eq!(
            apply_badge_stat_boost(100, Attack, &all_badge_ids("Emerald"), "Emerald"),
            110
        );
        assert_eq!(
            apply_badge_stat_boost(
                100,
                Speed,
                &all_badge_ids("FireRed and LeafGreen"),
                "FireRed and LeafGreen"
            ),
            100
        );
        assert_eq!(
            apply_badge_stat_boost(100, Attack, &HashSet::new(), "Emerald"),
            100
        );
        assert_eq!(
            apply_badge_stat_boost(100, Attack, &all_badge_ids("Platinum"), "Platinum"),
            100
        );
    }

    #[test]
    fn lightening() {
        assert_eq!(
            lighten(Color32::from_rgb(0, 100, 255), 0.5),
            Color32::from_rgb(128, 178, 255)
        );
    }
}
