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
/// world, or its frame origin when the map is shown on its own ((0, 0) for
/// tile maps; image-world interiors leave room for their picture).
pub fn map_origin_px(pack: &MapPack, scope: Scope, id: MapId) -> Option<(i32, i32)> {
    match scope {
        Scope::World => {
            let (px, py) = pack.map(id)?.world_pos?;
            Some((px * pack.geom.block_px as i32, py * pack.geom.block_px as i32))
        }
        Scope::Map(m) if m == id => Some(pack.map(id)?.frame.origin),
        Scope::Map(_) => None,
    }
}

/// World rectangle covered by a scope.
pub fn scope_rect(pack: &MapPack, scope: Scope) -> IRect {
    match scope {
        Scope::World => IRect::from_size(0, 0, (pack.layout.w_blocks * pack.geom.block_px) as i32, (pack.layout.h_blocks * pack.geom.block_px) as i32),
        Scope::Map(id) => match pack.map(id) {
            Some(m) => IRect::from_size(0, 0, m.frame.size.0, m.frame.size.1),
            None => IRect::default(),
        },
    }
}

/// What "fit" shows of a scope: everything, except for an image-world
/// interior, whose frame is a whole matrix of 32-tile chunks around a room
/// that may fill a corner of it. That fits the mask's visible cells (else
/// the picture).
pub fn fit_rect(pack: &MapPack, scope: Scope) -> IRect {
    let full = scope_rect(pack, scope);
    let (Some(img), Scope::Map(id)) = (&pack.image, scope) else { return full };
    let Some(m) = pack.map(id) else { return full };
    let at = m.frame.image_at;
    let mask_bounds = m.image.as_ref().and_then(|p| img.interior_masks.get(p)).and_then(|mask| {
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for cy in 0..mask.h {
            for cx in 0..mask.w {
                if mask.cells[cy * mask.w + cx] != 0 {
                    x0 = x0.min(cx as i32);
                    y0 = y0.min(cy as i32);
                    x1 = x1.max(cx as i32 + 1);
                    y1 = y1.max(cy as i32 + 1);
                }
            }
        }
        (x1 > x0).then(|| IRect::new(at.0 + x0 * 16, at.1 + y0 * 16, at.0 + x1 * 16, at.1 + y1 * 16))
    });
    let r = mask_bounds.unwrap_or(full).intersect(&full);
    if r.is_empty() {
        full
    } else {
        r
    }
}

/// Centre of an object in world pixels for a scope (raised by its lift in
/// an image world, where the marker is drawn).
pub fn object_center_px(pack: &MapPack, scope: Scope, obj_idx: u32) -> Option<(i32, i32)> {
    let o = pack.objects.get(obj_idx as usize)?;
    let (ox, oy) = map_origin_px(pack, scope, o.map)?;
    let s = pack.geom.step_px as i32;
    Some((ox + o.x as i32 * s + s / 2, oy + o.y as i32 * s + s / 2 - o.lift as i32))
}

/// Centre of a step position of a map in world pixels for a scope (raised
/// by the tile's lift in an image world, where the tile is drawn).
pub fn step_center_px(pack: &MapPack, scope: Scope, map: MapId, x: i32, y: i32) -> Option<(i32, i32)> {
    let (ox, oy) = map_origin_px(pack, scope, map)?;
    let s = pack.geom.step_px as i32;
    Some((ox + x * s + s / 2, oy + y * s + s / 2 - pack.tile_lift(map, x, y)))
}

/// Which map (and local pixel) a world pixel falls in; overlay maps win
/// because the layout's draw order puts them last. Image worlds ask the
/// ownership grid, since their map rectangles overlap.
pub fn map_at_world_px(pack: &MapPack, scope: Scope, wx: i32, wy: i32) -> Option<(MapId, i32, i32)> {
    match scope {
        Scope::Map(id) => {
            let m = pack.map(id)?;
            let (lx, ly) = (wx - m.frame.origin.0, wy - m.frame.origin.1);
            let (w, h) = map_px_size(&pack.geom, m);
            if IRect::from_size(0, 0, w, h).contains(lx, ly) {
                Some((id, lx, ly))
            } else {
                None
            }
        }
        Scope::World if pack.image.is_some() => {
            let img = pack.image.as_ref()?;
            let t = pack.geom.block_px as i32;
            let id = img.ownership.owner(wx.div_euclid(t), wy.div_euclid(t))?;
            let (px, py) = pack.map(id)?.world_pos?;
            Some((id, wx - px * t, wy - py * t))
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

/// The step under a world pixel: `(map, x, y)`. Tile worlds take the step
/// the pixel is in; image worlds the front-most tile whose lifted picture
/// covers the pixel (GEN45_REQUIREMENTS §4.5), across every overworld map
/// near it and only where the ownership grid gives that map the tile.
pub fn pick_step(pack: &MapPack, scope: Scope, wx: i32, wy: i32) -> Option<(MapId, i32, i32)> {
    let s = pack.geom.step_px as i32;
    let Some(img) = &pack.image else {
        let (map, lx, ly) = map_at_world_px(pack, scope, wx, wy)?;
        return Some((map, lx.div_euclid(s), ly.div_euclid(s)));
    };
    match scope {
        Scope::Map(id) => {
            let m = pack.map(id)?;
            let (lx, ly) = (wx - m.frame.origin.0, wy - m.frame.origin.1);
            let (tx, ty) = img.pick_local(id, m.w, m.h, lx, ly)?;
            Some((id, tx, ty))
        }
        Scope::World => {
            let mut best: Option<(MapId, i32, i32, i32)> = None; // (map, tx, ty, world row)
            for (rect, id) in &pack.layout.placed_rects {
                let slack = img.max_lift.get(*id as usize).copied().unwrap_or(0) as i32 + s;
                if wx < rect.x0 || wx >= rect.x1 || wy < rect.y0 - slack || wy >= rect.y1 + 4 * s {
                    continue;
                }
                let m = pack.map(*id)?;
                let Some((tx, ty)) = img.pick_local(*id, m.w, m.h, wx - rect.x0, wy - rect.y0) else { continue };
                let (wtx, wty) = (rect.x0 / s + tx, rect.y0 / s + ty);
                if img.ownership.owner(wtx, wty) != Some(*id) {
                    continue;
                }
                if best.map(|b| wty > b.3).unwrap_or(true) {
                    best = Some((*id, tx, ty, wty));
                }
            }
            best.map(|(id, tx, ty, _)| (id, tx, ty)).or_else(|| {
                let (map, lx, ly) = map_at_world_px(pack, scope, wx, wy)?;
                Some((map, lx.div_euclid(s), ly.div_euclid(s)))
            })
        }
    }
}
