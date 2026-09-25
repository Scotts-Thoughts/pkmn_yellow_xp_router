//! Integer rectangles and the world / map / step coordinate maths
//! (SPEC §3.3): one world-pixel space per game at native scale.

use crate::model::{MapDef, MapGeom, MapId};
use crate::pack::MapPack;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IRect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl IRect {
    pub fn new(x0: i32, y0: i32, x1: i32, y1: i32) -> IRect {
        IRect { x0, y0, x1, y1 }
    }
    pub fn from_size(x: i32, y: i32, w: i32, h: i32) -> IRect {
        IRect { x0: x, y0: y, x1: x + w, y1: y + h }
    }
    pub fn width(&self) -> i32 {
        self.x1 - self.x0
    }
    pub fn height(&self) -> i32 {
        self.y1 - self.y0
    }
    pub fn is_empty(&self) -> bool {
        self.x1 <= self.x0 || self.y1 <= self.y0
    }
    pub fn intersect(&self, o: &IRect) -> IRect {
        IRect { x0: self.x0.max(o.x0), y0: self.y0.max(o.y0), x1: self.x1.min(o.x1), y1: self.y1.min(o.y1) }
    }
    pub fn intersects(&self, o: &IRect) -> bool {
        !self.intersect(o).is_empty()
    }
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x0 && x < self.x1 && y >= self.y0 && y < self.y1
    }
    pub fn translate(&self, dx: i32, dy: i32) -> IRect {
        IRect { x0: self.x0 + dx, y0: self.y0 + dy, x1: self.x1 + dx, y1: self.y1 + dy }
    }
}

/// Pixel size of a map.
pub fn map_px_size(geom: &MapGeom, m: &MapDef) -> (i32, i32) {
    ((m.w * geom.block_px) as i32, (m.h * geom.block_px) as i32)
}

/// The map's rectangle in world pixels (outdoor maps only).
pub fn map_world_rect(pack: &MapPack, id: MapId) -> Option<IRect> {
    let m = pack.map(id)?;
    let (px, py) = m.world_pos?;
    let (w, h) = map_px_size(&pack.geom, m);
    Some(IRect::from_size(px * pack.geom.block_px as i32, py * pack.geom.block_px as i32, w, h))
}

/// The world-pixel origin of a map in a scope: its layout position in the
/// world, or (0, 0) when the map is shown on its own.
pub fn map_origin_px(pack: &MapPack, scope: Scope, id: MapId) -> Option<(i32, i32)> {
    match scope {
        Scope::World => {
            let (px, py) = pack.map(id)?.world_pos?;
            Some((px * pack.geom.block_px as i32, py * pack.geom.block_px as i32))
        }
        Scope::Map(m) if m == id => Some((0, 0)),
        Scope::Map(_) => None,
    }
}

/// World rectangle covered by a scope.
pub fn scope_rect(pack: &MapPack, scope: Scope) -> IRect {
    match scope {
        Scope::World => IRect::from_size(0, 0, (pack.layout.w_blocks * pack.geom.block_px) as i32, (pack.layout.h_blocks * pack.geom.block_px) as i32),
        Scope::Map(id) => match pack.map(id) {
            Some(m) => {
                let (w, h) = map_px_size(&pack.geom, m);
                IRect::from_size(0, 0, w, h)
            }
            None => IRect::default(),
        },
    }
}

/// Centre of an object in world pixels for a scope.
pub fn object_center_px(pack: &MapPack, scope: Scope, obj_idx: u32) -> Option<(i32, i32)> {
    let o = pack.objects.get(obj_idx as usize)?;
    let (ox, oy) = map_origin_px(pack, scope, o.map)?;
    let s = pack.geom.step_px as i32;
    Some((ox + o.x as i32 * s + s / 2, oy + o.y as i32 * s + s / 2))
}

/// Centre of a step position of a map in world pixels for a scope.
pub fn step_center_px(pack: &MapPack, scope: Scope, map: MapId, x: i32, y: i32) -> Option<(i32, i32)> {
    let (ox, oy) = map_origin_px(pack, scope, map)?;
    let s = pack.geom.step_px as i32;
    Some((ox + x * s + s / 2, oy + y * s + s / 2))
}

/// Which map (and local pixel) a world pixel falls in; overlay maps win
/// because the layout's draw order puts them last.
pub fn map_at_world_px(pack: &MapPack, scope: Scope, wx: i32, wy: i32) -> Option<(MapId, i32, i32)> {
    match scope {
        Scope::Map(id) => {
            let r = scope_rect(pack, scope);
            if r.contains(wx, wy) {
                Some((id, wx, wy))
            } else {
                None
            }
        }
        Scope::World => {
            let mut hit = None;
            for (rect, id) in &pack.layout.placed_rects {
                if rect.contains(wx, wy) {
                    hit = Some((*id, wx - rect.x0, wy - rect.y0));
                }
            }
            hit
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    World,
    Map(MapId),
}
