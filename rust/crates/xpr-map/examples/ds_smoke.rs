//! Smoke test for image-world packs: load, pick, render chunks to PNG.
//! cargo run -p xpr-map --example ds_smoke -- <game> <imagery.zip>? <out dir>
use std::sync::Arc;
use xpr_map::imagery::{Imagery, ZipFiles};
use xpr_map::*;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let game = args.get(1).map(|s| s.as_str()).unwrap_or("platinum");
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../map_data");
    let t = std::time::Instant::now();
    let pack = Arc::new(MapPack::load(game, &PackSource::new(Some(root))).expect("load"));
    println!("loaded {} in {:?}: {} maps, {} objects, {} encounter maps, gen {}", game, t.elapsed(), pack.maps.len(), pack.objects.len(), pack.encounters.len(), pack.gen);
    let img = pack.image.as_ref().unwrap();
    println!("world {}x{} levels {} masks world={} interiors={}", img.w_px, img.h_px, img.levels, img.world_mask.is_some(), img.interior_masks.len());
    let imagery = args.get(2).filter(|s| s.ends_with(".zip")).map(|p| Arc::new(Imagery::new(Box::new(ZipFiles::open(std::path::Path::new(p)).expect("zip")))));
    let out = std::path::PathBuf::from(args.last().unwrap());
    let comp = Compositor::with_imagery(pack.clone(), imagery);
    let dm = img.default_map.unwrap();
    let m = pack.map(dm).unwrap();
    let (px, py) = m.world_pos.unwrap();
    println!("default map {} at tile {:?}", m.display, (px, py));
    let opts = RenderOpts { night: false, mask: true };
    for level in [0u8, 2, 4] {
        let span = 512 << level;
        let (cx, cy) = ((px * 16) / span, (py * 16) / span);
        let t = std::time::Instant::now();
        let chunk = comp.render_chunk(Scope::World, level, cx as u32, cy as u32, opts);
        println!("chunk L{} ({},{}) in {:?}", level, cx, cy, t.elapsed());
        std::fs::write(out.join(format!("{}_L{}.png", game, level)), chunk.to_png().unwrap()).unwrap();
    }
    // an interior
    let inner = pack.maps.iter().find(|m| m.image.is_some() && m.display.contains("Pokémon Center")).or_else(|| pack.maps.iter().find(|m| m.image.is_some())).unwrap();
    println!("interior {} frame {:?}", inner.display, inner.frame);
    let r = geom::scope_rect(&pack, Scope::Map(inner.id));
    let px = comp.render_region(Scope::Map(inner.id), r, opts);
    std::fs::write(out.join(format!("{}_interior.png", game)), px.to_png().unwrap()).unwrap();
    // picking: the default map's centre
    let (wx, wy) = (px_center(m.world_pos.unwrap().0, m.w), px_center(m.world_pos.unwrap().1, m.h));
    println!("pick at map centre: {:?}, terrain {:?}", geom::pick_step(&pack, Scope::World, wx, wy), geom::pick_step(&pack, Scope::World, wx, wy).map(|(id, x, y)| pack.terrain_at(id, x as u32, y as u32)));
    let grid = ObjectGrid::build(&pack);
    let o = pack.objects.iter().position(|o| o.map == dm && matches!(o.kind, ObjectKind::Trainer | ObjectKind::Npc)).unwrap() as u32;
    let (ox, oy) = geom::object_center_px(&pack, Scope::World, o).unwrap();
    println!("object {} at {:?}, hit -> {:?}", o, (ox, oy), grid.hit(&pack, Scope::World, ox, oy, |_| true));
}

fn px_center(t: i32, n: u32) -> i32 {
    t * 16 + n as i32 * 8
}
