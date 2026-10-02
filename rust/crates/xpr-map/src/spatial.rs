//! Per-map step grid of objects for O(1) hit-testing (SPEC §3.4). In an
//! image world (gens 4/5) a marker is drawn raised by its object's lift, so
//! objects are filed under the step their marker is drawn on.

use std::collections::HashMap;

use crate::geom::{map_at_world_px, map_origin_px, Scope};
use crate::model::{MapId, MapObject, ObjectKind};
use crate::pack::MapPack;

#[derive(Debug, Default)]
pub struct ObjectGrid {
    cells: HashMap<(MapId, u16, u16), Vec<u32>>,
}

/// pokemap's click tolerance: 12 px around the step centre (at native scale).
pub const HIT_THRESHOLD_PX: i32 = 12;

impl ObjectGrid {
    pub fn build(pack: &MapPack) -> ObjectGrid {
        let mut cells: HashMap<(MapId, u16, u16), Vec<u32>> = HashMap::new();
        let s = pack.geom.step_px as i32;
        for (i, o) in pack.objects.iter().enumerate() {
            let vy = (o.y as i32 * s + s / 2 - o.lift as i32).div_euclid(s).max(0) as u16;
            cells.entry((o.map, o.x, vy)).or_default().push(i as u32);
        }
        ObjectGrid { cells }
    }

    /// Objects on a step of a map.
    pub fn at(&self, map: MapId, x: u16, y: u16) -> &[u32] {
        self.cells.get(&(map, x, y)).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// The object under a world pixel: candidates from the 3x3 steps around
    /// the pixel, within the click tolerance, highest-priority kind first,
    /// then nearest. `visible` filters objects that are toggled off.
    pub fn hit(&self, pack: &MapPack, scope: Scope, wx: i32, wy: i32, visible: impl Fn(&MapObject) -> bool) -> Option<u32> {
        if let (Some(img), Scope::World) = (&pack.image, scope) {
            // raised markers can stand over a neighbouring map: try every map near the point
            let s = pack.geom.step_px as i32;
            let mut best: Option<(u32, i32, i32)> = None;
            for (rect, id) in &pack.layout.placed_rects {
                let slack = img.max_lift.get(*id as usize).copied().unwrap_or(0) as i32 + 2 * s;
                if wx < rect.x0 - s || wx >= rect.x1 + s || wy < rect.y0 - slack || wy >= rect.y1 + 2 * s {
                    continue;
                }
                if let Some(hit) = self.hit_in_map(pack, *id, wx - rect.x0, wy - rect.y0, &visible) {
                    if best.map(|b| (hit.1, hit.2) < (b.1, b.2)).unwrap_or(true) {
                        best = Some(hit);
                    }
                }
            }
            return best.map(|(i, _, _)| i);
        }
        if pack.image.is_some() {
            let Scope::Map(id) = scope else { return None };
            let (ox, oy) = map_origin_px(pack, scope, id)?;
            return self.hit_in_map(pack, id, wx - ox, wy - oy, &visible).map(|(i, _, _)| i);
        }
        let (map, lx, ly) = map_at_world_px(pack, scope, wx, wy)?;
        self.hit_in_map(pack, map, lx, ly, &visible).map(|(i, _, _)| i)
    }

    /// The best object of one map near a map-local pixel: (index, priority, distance).
    fn hit_in_map(&self, pack: &MapPack, map: MapId, lx: i32, ly: i32, visible: &impl Fn(&MapObject) -> bool) -> Option<(u32, i32, i32)> {
        let s = pack.geom.step_px as i32;
        let sx = lx.div_euclid(s);
        let sy = ly.div_euclid(s);
        let mut best: Option<(u32, i32, i32)> = None; // (idx, priority, dist)
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (cx, cy) = (sx + dx, sy + dy);
                if cx < 0 || cy < 0 {
                    continue;
                }
                for &i in self.at(map, cx as u16, cy as u16) {
                    let o = &pack.objects[i as usize];
                    if !visible(o) {
                        continue;
                    }
                    let px = o.x as i32 * s + s / 2;
                    let py = o.y as i32 * s + s / 2 - o.lift as i32;
                    let ddx = (lx - px).abs();
                    let ddy = (ly - py).abs();
                    if ddx >= HIT_THRESHOLD_PX || ddy >= HIT_THRESHOLD_PX {
                        continue;
                    }
                    let dist = ddx.max(ddy);
                    let pri = kind_priority(o.effective_kind());
                    let better = match best {
                        None => true,
                        Some((_, bp, bd)) => pri < bp || (pri == bp && dist < bd),
                    };
                    if better {
                        best = Some((i, pri, dist));
                    }
                }
            }
        }
        best
    }
}

fn kind_priority(k: ObjectKind) -> i32 {
    match k {
        ObjectKind::Trainer => 0,
        ObjectKind::Item => 1,
        ObjectKind::HiddenItem => 2,
        ObjectKind::Berry => 3,
        ObjectKind::Warp => 4,
        ObjectKind::Sign => 5,
        ObjectKind::Npc => 6,
        ObjectKind::Obstacle => 7,
    }
}
