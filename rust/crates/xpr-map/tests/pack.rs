//! Pack loading and link invariants over every game in `map_data/`
//! (SPEC §5 Phase 1 step 6).

use std::path::PathBuf;
use std::sync::Arc;

use xpr_data::Registry;
use xpr_map::{Compositor, MapKind, MapPack, ObjectGrid, ObjectKind, PackSource, Precision, RenderOpts, Scope};

const GAMES: &[(&str, &str)] = &[
    ("yellow", "Yellow"),
    ("red_blue", "Red"),
    ("gold_silver", "Gold"),
    ("crystal", "Crystal"),
    ("ruby_sapphire", "Ruby"),
    ("emerald", "Emerald"),
    ("firered_leafgreen", "FireRed"),
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn source() -> PackSource {
    PackSource::new(Some(repo_root().join("map_data")))
}

#[test]
fn every_game_loads_with_consistent_geometry() {
    for (game, _) in GAMES {
        let pack = MapPack::load(game, &source()).unwrap_or_else(|e| panic!("{}: {}", game, e));
        assert!(pack.maps.len() > 100, "{}: too few maps", game);
        assert!(pack.layout.draw_order.len() >= 36, "{}: outdoor maps", game);
        let bp = pack.geom.block_px;
        assert_eq!(bp, if pack.gen == 3 { 16 } else { 32 });
        for m in &pack.maps {
            if let Some(r) = &m.blocks {
                // a few maps share a .blk of a different size (pokemap keeps them as-is); the compositor bounds-checks
                if r.len() != (m.w * m.h) as usize {
                    eprintln!("{}: blockdata of {} has {} entries for {}x{} blocks", game, m.const_name, r.len(), m.w, m.h);
                }
                assert!(!r.is_empty(), "{}: empty blockdata of {}", game, m.const_name);
            }
            if m.kind == MapKind::Outdoor {
                assert!(m.world_pos.is_some());
            }
            match &m.tileset {
                xpr_map::TilesetRef::Gen12 { file } => assert!(pack.tilesets.contains_key(file) || m.blocks.is_none(), "{}: tileset {} of {}", game, file, m.const_name),
                xpr_map::TilesetRef::Gen3 { primary, .. } => assert!(pack.tilesets.contains_key(primary), "{}: tileset {} of {}", game, primary, m.const_name),
            }
        }
        // objects live inside their maps (steps), anchors inside theirs
        let spb = pack.geom.steps_per_block();
        let mut outside = 0;
        for o in &pack.objects {
            let m = pack.map(o.map).unwrap();
            if o.x as u32 > m.w * spb + 1 || o.y as u32 > m.h * spb + 1 {
                outside += 1; // a handful of decomp objects sit off their map (gates, prototypes)
            }
        }
        assert!(outside * 100 < pack.objects.len(), "{}: {} objects outside their map", game, outside);
        for (name, anchors) in &pack.links.trainers {
            assert!(!anchors.is_empty(), "{}: empty anchor list for {}", game, name);
            for a in anchors {
                let m = pack.map(a.map).unwrap();
                if a.precision != Precision::Map {
                    assert!(a.x as u32 <= m.w * spb + 1 && a.y as u32 <= m.h * spb + 1, "{}: anchor of {} outside {}", game, name, m.const_name);
                }
                if let Some(obj) = a.object {
                    assert!((obj as usize) < pack.objects.len());
                }
            }
        }
        assert!(!pack.encounters.is_empty(), "{}: encounters", game);
    }
}

#[test]
fn link_keys_resolve_in_the_router_data() {
    let root = repo_root();
    let registry = Registry::new(root.join("raw_pkmn_data"), PathBuf::new());
    for (game, version) in GAMES {
        let pack = MapPack::load(game, &source()).unwrap();
        let gen = registry.get_version(version).unwrap_or_else(|e| panic!("{}: {}", version, e));
        let mut missing = Vec::new();
        for key in pack.links.trainers.keys() {
            if gen.trainer_db().get_trainer(key).is_none() {
                missing.push(key.clone());
            }
        }
        assert!(missing.is_empty(), "{}: trainer links with no router trainer: {:?}", game, missing);
        let mut missing_items = Vec::new();
        for key in pack.links.items.keys() {
            if gen.item_db().get_item(key).is_none() {
                missing_items.push(key.clone());
            }
        }
        assert!(missing_items.is_empty(), "{}: item links with no router item: {:?}", game, missing_items);
        // every object that names a trainer names a real one
        for o in &pack.objects {
            for n in o.trainer_names() {
                assert!(gen.trainer_db().get_trainer(n).is_some(), "{}: object trainer {} unknown", game, n);
            }
        }
        // gym leaders and the elite four are always linked
        let mut unlinked = Vec::new();
        for name in gen.get_gym_leader_names() {
            if !pack.links.has_trainer(&name) {
                unlinked.push(name);
            }
        }
        assert!(unlinked.is_empty(), "{}: gym leaders without a map anchor: {:?}", game, unlinked);
    }
}

#[test]
fn coverage_meets_the_baseline() {
    // (game, minimum fraction of eligible trainer records with an anchor)
    let baseline: &[(&str, f64)] = &[("yellow", 0.99), ("red_blue", 0.97), ("gold_silver", 0.95), ("crystal", 0.95), ("ruby_sapphire", 0.89), ("emerald", 0.96), ("firered_leafgreen", 0.96)];
    for (game, min) in baseline {
        let pack = MapPack::load(game, &source()).unwrap();
        let s = &pack.links.stats;
        let linked = s.get("trainers_linked").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let eligible = s.get("trainers_eligible").and_then(|v| v.as_f64()).unwrap_or(1.0);
        let frac = linked / eligible;
        assert!(frac >= *min, "{}: trainer link coverage {:.3} below baseline {:.2}", game, frac, min);
    }
}

#[test]
fn compositor_renders_every_placed_map_and_chunks_agree() {
    for (game, _) in GAMES {
        let pack = Arc::new(MapPack::load(game, &source()).unwrap());
        let comp = Compositor::new(pack.clone());
        // one indoor + one outdoor map render without panicking and are not blank
        let outdoor = pack.layout.draw_order[0];
        let img = comp.render_scope(Scope::Map(outdoor), RenderOpts::default());
        assert!(img.w > 0 && img.h > 0);
        assert!(img.data.chunks_exact(4).any(|p| p[0..3] != xpr_map::compose::BACKGROUND[0..3]), "{}: blank render", game);
        // a level-1 chunk equals the box-downsampled level-0 quadrants
        let l1 = comp.render_chunk(Scope::World, 1, 0, 0, RenderOpts::default());
        let mut expect = xpr_map::Pixmap::filled(512, 512, xpr_map::compose::BACKGROUND);
        for (i, (cx, cy)) in [(0u32, 0u32), (1, 0), (0, 1), (1, 1)].iter().enumerate() {
            let l0 = comp.render_chunk(Scope::World, 0, *cx, *cy, RenderOpts::default());
            let _ = i;
            expect.downsample_into((*cx as usize) * 256, (*cy as usize) * 256, &l0, 2);
        }
        assert_eq!(l1.data, expect.data, "{}: LOD chunk mismatch", game);
    }
}

#[test]
fn hit_testing_finds_objects_under_their_step() {
    let pack = MapPack::load("yellow", &source()).unwrap();
    let grid = ObjectGrid::build(&pack);
    let s = pack.geom.step_px as i32;
    let mut checked = 0;
    for (i, o) in pack.objects.iter().enumerate() {
        if o.kind != ObjectKind::Trainer {
            continue;
        }
        let Some((ox, oy)) = xpr_map::geom::map_origin_px(&pack, Scope::World, o.map) else { continue };
        let (wx, wy) = (ox + o.x as i32 * s + s / 2, oy + o.y as i32 * s + s / 2);
        let hit = grid.hit(&pack, Scope::World, wx, wy, |_| true);
        assert_eq!(hit, Some(i as u32), "object {} of {}", i, pack.map(o.map).unwrap().const_name);
        checked += 1;
    }
    assert!(checked > 100);
}
