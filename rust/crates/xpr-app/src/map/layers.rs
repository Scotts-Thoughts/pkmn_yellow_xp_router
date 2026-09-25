//! Drawing the map: the chunk base layer (with coarser-level fallback while
//! fine chunks are pending), the object markers, the routed-state overlay
//! and the focus pulse (SPEC §3.4, §3.6).

use std::collections::HashMap;

use egui::{Align2, Color32, Pos2, Rect, Stroke, Vec2};
use xpr_map::{geom, MapKind, MapPack, ObjectKind, Scope, CHUNK_PX};
use xpr_ui_kit::theme::{self, Theme};

use super::camera::Camera;
use super::chunks::{ChunkCache, ChunkKey};
use super::sprites::SpriteAtlas;
use super::state::RouteMapState;
use super::Toggles;

/// Below this zoom sprites are too small to read; markers are circles.
pub const SPRITE_MIN_ZOOM: f32 = 0.75;

pub const COLOR_WARP: Color32 = Color32::from_rgb(0xff, 0xaa, 0x00);
pub const COLOR_TRAINER: Color32 = Color32::from_rgb(0xff, 0x44, 0x44);
pub const COLOR_ITEM: Color32 = Color32::from_rgb(0x44, 0xff, 0x44);
pub const COLOR_NPC: Color32 = Color32::from_rgb(0x88, 0x88, 0xff);
pub const COLOR_SIGN: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
pub const COLOR_HIDDEN: Color32 = Color32::from_rgb(0xff, 0x66, 0xff);
pub const COLOR_BERRY: Color32 = Color32::from_rgb(0x66, 0xdd, 0xcc);

pub fn marker_style(kind: ObjectKind) -> (Color32, &'static str, bool) {
    match kind {
        ObjectKind::Trainer => (COLOR_TRAINER, "T", false),
        ObjectKind::Item => (COLOR_ITEM, "!", false),
        ObjectKind::HiddenItem => (COLOR_HIDDEN, "?", true),
        ObjectKind::Berry => (COLOR_BERRY, "B", false),
        ObjectKind::Sign => (COLOR_SIGN, "S", true),
        ObjectKind::Warp => (COLOR_WARP, "W", true),
        ObjectKind::Npc => (COLOR_NPC, "", true),
    }
}

/// The maximum LOD level needed for a scope (one chunk covers everything).
pub fn max_level(pack: &MapPack, scope: Scope) -> u8 {
    let r = geom::scope_rect(pack, scope);
    let side = r.width().max(r.height()).max(1) as f32;
    ((side / CHUNK_PX as f32).log2().ceil().max(0.0)) as u8
}

/// Draw the base layer. Returns the chunk keys that should be requested
/// (visible, not ready), nearest to the centre first.
pub fn draw_base(painter: &egui::Painter, vp: Rect, cam: &Camera, pack: &MapPack, scope: Scope, night: bool, cache: &mut ChunkCache) -> Vec<ChunkKey> {
    let max_lvl = max_level(pack, scope);
    let level = cam.lod_level(max_lvl);
    let span = CHUNK_PX << level;
    let scope_r = geom::scope_rect(pack, scope);
    let vis = cam.visible_world(vp).intersect(&scope_r);
    if vis.is_empty() {
        return Vec::new();
    }
    let cx0 = (vis.x0 / span).max(0);
    let cy0 = (vis.y0 / span).max(0);
    let cx1 = (vis.x1 - 1) / span;
    let cy1 = (vis.y1 - 1) / span;
    // world origin on screen: rounded to physical pixels so integer zooms stay crisp
    let ppp = painter.ctx().pixels_per_point();
    let origin = cam.world_to_screen(vp, Vec2::ZERO);
    let origin = Pos2::new((origin.x * ppp).round() / ppp, (origin.y * ppp).round() / ppp);
    let scale = cam.zoom;
    let mut wanted: Vec<(i32, ChunkKey)> = Vec::new();
    let vc = vis.x0 / 2 + vis.x1 / 2;
    let vcy = vis.y0 / 2 + vis.y1 / 2;
    let uv_full = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
    for cy in cy0..=cy1 {
        for cx in cx0..=cx1 {
            let key = ChunkKey { scope, level, cx: cx as u32, cy: cy as u32, night };
            let wx0 = cx * span;
            let wy0 = cy * span;
            let rect = Rect::from_min_size(origin + Vec2::new(wx0 as f32 * scale, wy0 as f32 * scale), Vec2::splat(span as f32 * scale));
            if let Some(tex) = cache.get(&key) {
                let mut mesh = egui::Mesh::with_texture(tex.id());
                mesh.add_rect_with_uv(rect, uv_full, Color32::WHITE);
                painter.add(egui::Shape::mesh(mesh));
                continue;
            }
            // fallback: the nearest coarser chunk that is ready
            let mut drawn = false;
            for k in 1..=(max_lvl.saturating_sub(level)) {
                let pk = ChunkKey { scope, level: level + k, cx: (cx >> k) as u32, cy: (cy >> k) as u32, night };
                if let Some(tex) = cache.get(&pk) {
                    let n = 1 << k;
                    let fx = (cx & (n - 1)) as f32 / n as f32;
                    let fy = (cy & (n - 1)) as f32 / n as f32;
                    let uv = Rect::from_min_size(Pos2::new(fx, fy), Vec2::splat(1.0 / n as f32));
                    let mut mesh = egui::Mesh::with_texture(tex.id());
                    mesh.add_rect_with_uv(rect, uv, Color32::WHITE);
                    painter.add(egui::Shape::mesh(mesh));
                    drawn = true;
                    break;
                }
            }
            if !drawn {
                painter.rect_filled(rect, 0.0, Color32::from_rgb(10, 10, 24));
            }
            let d = (wx0 + span / 2 - vc).abs() + (wy0 + span / 2 - vcy).abs();
            wanted.push((d, key));
        }
    }
    wanted.sort_by_key(|(d, _)| *d);
    let mut out: Vec<ChunkKey> = wanted.into_iter().map(|(_, k)| k).collect();
    // also warm the two coarser levels around the view so zooming out is smooth
    for k in 1..=2u8 {
        let l = level + k;
        if l > max_lvl {
            break;
        }
        let s = CHUNK_PX << l;
        for cy in (vis.y0 / s).max(0)..=((vis.y1 - 1) / s) {
            for cx in (vis.x0 / s).max(0)..=((vis.x1 - 1) / s) {
                let key = ChunkKey { scope, level: l, cx: cx as u32, cy: cy as u32, night };
                if !cache.has(&key) {
                    out.push(key);
                }
            }
        }
    }
    out
}

/// Keys of every chunk of the coarse levels (>= `from_level`) of a scope.
pub fn overview_keys(pack: &MapPack, scope: Scope, night: bool, from_level: u8) -> Vec<ChunkKey> {
    let max_lvl = max_level(pack, scope);
    let r = geom::scope_rect(pack, scope);
    let mut out = Vec::new();
    for level in (from_level.min(max_lvl)..=max_lvl).rev() {
        let span = CHUNK_PX << level;
        for cy in 0..=((r.y1 - 1).max(0) / span) {
            for cx in 0..=((r.x1 - 1).max(0) / span) {
                out.push(ChunkKey { scope, level, cx: cx as u32, cy: cy as u32, night });
            }
        }
    }
    out
}

pub struct MarkerCtx<'a> {
    pub theme: &'a Theme,
    pub toggles: &'a Toggles,
    pub state: &'a RouteMapState,
    pub hover: Option<u32>,
    pub selected: Option<u32>,
    pub night: bool,
    pub ctx: &'a egui::Context,
}

fn check_mark(painter: &egui::Painter, center: Pos2, radius: f32, c: Color32) {
    painter.line_segment([center + Vec2::new(-radius * 0.5, 0.0), center + Vec2::new(-radius * 0.1, radius * 0.45)], Stroke::new(2.0, c));
    painter.line_segment([center + Vec2::new(-radius * 0.1, radius * 0.45), center + Vec2::new(radius * 0.6, -radius * 0.5)], Stroke::new(2.0, c));
}

/// Draw the markers of every map intersecting the viewport: sprites for
/// trainers, NPCs, item balls and berry trees (one mesh per atlas page),
/// lettered circles for everything else and whenever sprites are off or
/// too small.
pub fn draw_markers(painter: &egui::Painter, vp: Rect, cam: &Camera, pack: &MapPack, scope: Scope, mc: &MarkerCtx, atlas: &mut SpriteAtlas) -> usize {
    let vis = cam.visible_world(vp);
    let zoom = cam.zoom;
    let r = (7.0 * zoom).clamp(3.5, 14.0);
    let r_small = (5.0 * zoom).clamp(2.5, 10.0);
    let show_labels = zoom >= 1.5;
    let use_sprites = mc.toggles.sprites && zoom >= SPRITE_MIN_ZOOM;
    let font = egui::FontId::monospace((9.0 * zoom).clamp(8.0, 13.0));
    let mut drawn = 0;
    let maps: Vec<(xpr_map::MapId, i32, i32)> = match scope {
        Scope::World => pack.layout.placed_rects.iter().filter(|(rect, _)| rect.intersects(&vis)).map(|(rect, id)| (*id, rect.x0, rect.y0)).collect(),
        Scope::Map(id) => vec![(id, 0, 0)],
    };
    let s = pack.geom.step_px as f32;
    // the same rounded origin as the chunk layer, so sprites sit exactly on their tiles
    let ppp = painter.ctx().pixels_per_point();
    let origin = cam.world_to_screen(vp, Vec2::ZERO);
    let origin = Pos2::new((origin.x * ppp).round() / ppp, (origin.y * ppp).round() / ppp);
    let mut meshes: HashMap<egui::TextureId, egui::Mesh> = HashMap::new();
    // overlays drawn after the sprite meshes: (center, radius, hover/selected, routed)
    let mut overlays: Vec<(Pos2, f32, bool, bool)> = Vec::new();
    for (map, ox, oy) in maps {
        let range = pack.object_range(map);
        for idx in range {
            let o = &pack.objects[idx as usize];
            let kind = o.effective_kind();
            if !mc.toggles.visible(kind) {
                continue;
            }
            let wx = ox as f32 + o.x as f32 * s + s / 2.0;
            let wy = oy as f32 + o.y as f32 * s + s / 2.0;
            if (wx as i32) < vis.x0 - 64 || (wx as i32) > vis.x1 + 64 || (wy as i32) < vis.y0 - 64 || (wy as i32) > vis.y1 + 64 {
                continue;
            }
            let center = cam.world_to_screen(vp, Vec2::new(wx, wy));
            let (color, letter, small) = marker_style(kind);
            let radius = if small { r_small } else { r };
            let routed = match kind {
                ObjectKind::Trainer | ObjectKind::Npc => o.trainer_names().iter().any(|n| mc.state.is_defeated(n)),
                ObjectKind::Item | ObjectKind::HiddenItem | ObjectKind::Berry => o.item_name().map(|n| mc.state.is_acquired(n)).unwrap_or(false),
                _ => false,
            };
            let highlighted = mc.hover == Some(idx) || mc.selected == Some(idx);
            let sprite_kind = matches!(kind, ObjectKind::Trainer | ObjectKind::Npc | ObjectKind::Item | ObjectKind::Berry);
            if use_sprites && sprite_kind {
                if let Some(f) = atlas.frame(mc.ctx, pack, o, mc.night) {
                    // 16 px of sprite = one step; taller frames rise above their step, wider ones centre on it
                    let step_left = origin.x + (ox as f32 + o.x as f32 * s) * zoom;
                    let step_bottom = origin.y + (oy as f32 + (o.y as f32 + 1.0) * s) * zoom;
                    let w = f.w as f32 * zoom;
                    let h = f.h as f32 * zoom;
                    let x0 = step_left - (w - s * zoom) / 2.0;
                    let rect = Rect::from_min_size(Pos2::new(x0, step_bottom - h), Vec2::new(w, h));
                    let tint = if routed { Color32::from_white_alpha(0x70) } else { Color32::WHITE };
                    meshes.entry(f.tex).or_insert_with(|| egui::Mesh::with_texture(f.tex)).add_rect_with_uv(rect, f.uv, tint);
                    overlays.push((center, radius, highlighted, routed));
                    drawn += 1;
                    continue;
                }
            }
            let fill_a: u8 = if routed { 0x30 } else { 0x88 };
            let stroke_c = if routed { theme::with_alpha(color, 0x70) } else { color };
            if kind == ObjectKind::HiddenItem {
                painter.circle(center, radius, theme::with_alpha(color, 0x44), Stroke::new(1.0, stroke_c));
            } else {
                painter.circle(center, radius, theme::with_alpha(color, fill_a), Stroke::new(1.5, stroke_c));
            }
            if highlighted {
                painter.circle_stroke(center, radius + 3.0, Stroke::new(2.0, mc.theme.accent));
            }
            if routed {
                check_mark(painter, center, radius, mc.theme.success);
            } else if show_labels && !letter.is_empty() {
                painter.text(center, Align2::CENTER_CENTER, letter, font.clone(), Color32::WHITE);
            }
            drawn += 1;
        }
    }
    for (_, mesh) in meshes {
        painter.add(egui::Shape::mesh(mesh));
    }
    for (center, radius, highlighted, routed) in overlays {
        if highlighted {
            painter.circle_stroke(center, radius + 3.0, Stroke::new(2.0, mc.theme.accent));
        }
        if routed {
            let badge = center + Vec2::new(radius * 0.9, -radius * 0.9);
            painter.circle(badge, radius * 0.55, mc.theme.bg, Stroke::new(1.0, mc.theme.success));
            check_mark(painter, badge, radius * 0.5, mc.theme.success);
        }
    }
    drawn
}

/// The pulsing ring around the focused anchor(s). Returns true while animating.
pub fn draw_focus(painter: &egui::Painter, vp: Rect, cam: &Camera, pack: &MapPack, scope: Scope, anchors: &[(xpr_map::MapId, i32, i32, bool)], t: f32, theme: &Theme) -> bool {
    let mut animating = false;
    for (map, x, y, map_level) in anchors {
        let Some((wx, wy)) = geom::step_center_px(pack, scope, *map, *x, *y) else { continue };
        let center = cam.world_to_screen(vp, Vec2::new(wx as f32, wy as f32));
        if !vp.expand(40.0).contains(center) {
            continue;
        }
        if *map_level {
            // whole-map anchor: outline the map
            if let Some(m) = pack.map(*map) {
                if let Some((ox, oy)) = geom::map_origin_px(pack, scope, *map) {
                    let (w, h) = geom::map_px_size(&pack.geom, m);
                    let a = cam.world_to_screen(vp, Vec2::new(ox as f32, oy as f32));
                    let b = cam.world_to_screen(vp, Vec2::new((ox + w) as f32, (oy + h) as f32));
                    painter.rect_stroke(Rect::from_min_max(a, b), 0.0, Stroke::new(2.0, theme.accent), egui::StrokeKind::Outside);
                }
            }
            continue;
        }
        let phase = (t % 1.2) / 1.2;
        let radius = 8.0 + phase * 16.0;
        let alpha = ((1.0 - phase) * 220.0) as u8;
        painter.circle_stroke(center, radius, Stroke::new(2.5, theme::with_alpha(theme.accent, alpha)));
        painter.circle_stroke(center, 9.0, Stroke::new(2.0, theme.accent));
        animating = true;
    }
    animating
}

pub fn map_is_outdoor(pack: &MapPack, id: xpr_map::MapId) -> bool {
    pack.map(id).map(|m| m.kind == MapKind::Outdoor).unwrap_or(false)
}
