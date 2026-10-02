//! Format 2 ("image world") packs, gens 4/5 (GEN45_REQUIREMENTS §4): every
//! game loads from `map_data/`, maps are framed and owned, lifted tiles and
//! raised objects are picked where they are drawn, encounter methods come in
//! the manifest's order, gen 4/5 sprites cut into frames, and the
//! compositor draws the imagery (from a directory or a zip), the masks and,
//! without imagery, the terrain schematic.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sha2::{Digest, Sha256};
use xpr_map::imagery::{verify_zip, DirFiles, Imagery, ZipFiles};
use xpr_map::sprites::{render_frame, SpriteCache};
use xpr_map::*;

const GAMES: [&str; 5] = ["diamond_pearl", "platinum", "heartgold_soulsilver", "black_white", "black2_white2"];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn load(game: &str) -> Arc<MapPack> {
    Arc::new(MapPack::load(game, &PackSource::new(Some(repo_root().join("map_data")))).unwrap_or_else(|e| panic!("{}: {}", game, e)))
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("xpr_map_image_pack_{}_{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn png(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
    let p = Pixmap::filled(w as usize, h as usize, rgba);
    p.to_png().unwrap()
}

#[test]
fn every_gen4_and_5_pack_loads_with_its_world() {
    for game in GAMES {
        let pack = load(game);
        let img = pack.image.as_ref().expect("an image world");
        assert!(pack.gen == 4 || pack.gen == 5, "{}", game);
        assert_eq!(pack.geom.block_px, 16);
        assert_eq!(pack.geom.step_px, 16);
        assert_eq!(geom::scope_rect(&pack, Scope::World), IRect::from_size(0, 0, img.w_px as i32, img.h_px as i32), "{}", game);
        // every outdoor map is placed and owns part of the world
        let outdoor: Vec<&MapDef> = pack.maps.iter().filter(|m| m.kind == MapKind::Outdoor).collect();
        assert!(outdoor.len() > 30, "{}: {} outdoor maps", game, outdoor.len());
        for m in &outdoor {
            let (px, py) = m.world_pos.unwrap();
            let owned = (0..m.h as i32).any(|y| (0..m.w as i32).any(|x| img.ownership.owner(px + x, py + y) == Some(m.id)));
            assert!(owned, "{}: {} owns no tile", game, m.const_name);
        }
        // every interior has an image and a frame that holds both it and the map
        for m in pack.maps.iter().filter(|m| m.kind == MapKind::Indoor) {
            assert!(m.image.is_some(), "{}: {} has no image", game, m.const_name);
            let (w, h) = geom::map_px_size(&pack.geom, m);
            let f = m.frame;
            assert!(f.origin.0 >= 0 && f.origin.1 >= 0 && f.origin.0 + w <= f.size.0 && f.origin.1 + h <= f.size.1, "{}: {} frame {:?}", game, m.const_name, f);
            assert!(pack.has_view(m.id));
        }
        // terrain classes and lift for every map
        assert!(img.tiles.iter().all(|t| t.is_some()), "{}: a map without terrain", game);
        assert!(img.world_mask.is_some());
        assert!(img.imagery.is_some());
        assert_eq!(img.default_map.and_then(|id| pack.map(id)).map(|m| m.kind), Some(MapKind::Outdoor), "{}", game);
        assert!(!img.encounter_methods.is_empty() && !img.encounter_conditions.is_empty(), "{}", game);
    }
}

#[test]
fn version_exclusive_objects_and_new_kinds_are_read() {
    let b2 = load("black2_white2");
    let exclusive: Vec<&MapObject> = b2.objects.iter().filter(|o| o.version.is_some()).collect();
    assert!(!exclusive.is_empty());
    let o = exclusive[0];
    let v = o.version.clone().unwrap();
    assert!(o.in_version(Some(&v)) && o.in_version(None));
    let other = if v == "Black 2" { "White 2" } else { "Black 2" };
    assert!(!o.in_version(Some(other)));
    let pt = load("platinum");
    assert!(pt.objects.iter().any(|o| matches!(&o.payload, Payload::Obstacle { obstacle, .. } if obstacle == "rock_smash")));
    assert!(pt.objects.iter().any(|o| matches!(&o.payload, Payload::Warp { dynamic: true, candidates, .. } if !candidates.is_empty())));
    assert!(pt.objects.iter().any(|o| o.lift != 0));
}

#[test]
fn lifted_tiles_and_raised_objects_are_picked_where_they_are_drawn() {
    let pack = load("platinum");
    let img = pack.image.as_ref().unwrap();
    // a tile lifted by at least a whole tile: the pixel it is drawn at picks it
    let mut checked = 0;
    for m in pack.maps.iter().filter(|m| m.kind == MapKind::Outdoor) {
        let (px, py) = m.world_pos.unwrap();
        for ty in 0..m.h as i32 {
            for tx in 0..m.w as i32 {
                let lift = img.lift_at(m.id, m.w, m.h, tx, ty) as i32;
                if lift < 20 || img.ownership.owner(px + tx, py + ty) != Some(m.id) {
                    continue;
                }
                let (wx, wy) = geom::step_center_px(&pack, Scope::World, m.id, tx, ty).unwrap();
                assert_eq!(wy, (py + ty) * 16 + 8 - lift);
                if geom::pick_step(&pack, Scope::World, wx, wy) == Some((m.id, tx, ty)) {
                    checked += 1;
                }
            }
        }
        if checked > 50 {
            break;
        }
    }
    assert!(checked > 50, "lifted tiles picked at their drawn position: {}", checked);

    // objects are hit at their raised marker, in the world and in their own map
    let grid = ObjectGrid::build(&pack);
    let mut hits = 0;
    for (i, o) in pack.objects.iter().enumerate().filter(|(_, o)| o.lift >= 16 && o.kind == ObjectKind::Trainer) {
        let m = pack.map(o.map).unwrap();
        let scope = if m.kind == MapKind::Outdoor { Scope::World } else { Scope::Map(o.map) };
        let (wx, wy) = geom::object_center_px(&pack, scope, i as u32).unwrap();
        if grid.hit(&pack, scope, wx, wy, |_| true) == Some(i as u32) {
            hits += 1;
        }
    }
    assert!(hits > 20, "raised trainers hit at their markers: {}", hits);
}

#[test]
fn single_maps_are_framed_around_their_picture() {
    let pack = load("platinum");
    let m = pack.map_by_const("MAP_HEADER_OREBURGH_GATE_1F").unwrap();
    // the picture starts 112 px left of and 32 px above the map, and is 736 x 480
    assert_eq!(m.frame.origin, (112, 32));
    assert_eq!(m.frame.image_at, (0, 0));
    assert_eq!(m.frame.size, (736, 32 + 512));
    let scope = Scope::Map(m.id);
    assert_eq!(geom::map_origin_px(&pack, scope, m.id), Some((112, 32)));
    // step (0, 0) of the map
    assert_eq!(geom::map_at_world_px(&pack, scope, 112 + 3, 32 + 3), Some((m.id, 3, 3)));
    assert_eq!(geom::map_at_world_px(&pack, scope, 50, 50), None);
}

#[test]
fn encounter_tables_follow_the_manifest_order() {
    let pack = load("platinum");
    let id = pack.map_by_const("MAP_HEADER_ROUTE_201").unwrap().id;
    let names: Vec<&str> = pack.encounters[&id].methods.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names[0], "walk");
    assert!(names.iter().position(|n| *n == "walk_night").unwrap() < names.iter().position(|n| n.starts_with("surf")).unwrap_or(usize::MAX));
    let slots = pack.encounters[&id].slots_for("walk", "Platinum").unwrap();
    assert!(slots.iter().any(|s| s.species == "Starly"));
    // gen 5 seasonal zones have only per-season tables
    let bw = load("black_white");
    let seasonal = bw.encounters.values().find(|t| t.methods.iter().any(|m| m.name.ends_with("_spring"))).expect("a seasonal zone");
    assert!(seasonal.methods.iter().all(|m| ["_spring", "_summer", "_autumn", "_winter"].iter().any(|s| m.name.ends_with(s))));
    // and the species link finds the maps
    assert!(!pack.resolve(&LinkQuery::Species("Starly".into())).is_empty());
}

#[test]
fn gen4_and_5_sprites_cut_four_frames() {
    for game in ["platinum", "black2_white2"] {
        let pack = load(game);
        let mut cache = SpriteCache::default();
        let o = pack.objects.iter().find(|o| o.kind == ObjectKind::Trainer && o.sprite.is_some()).unwrap();
        let meta = pack.sprites[o.sprite.as_ref().unwrap()].clone();
        assert_eq!(meta.frames, 4, "{}", game);
        let f = render_frame(&pack, &mut cache, o, false).unwrap_or_else(|| panic!("{}: frame", game));
        assert_eq!((f.w as u32, f.h as u32), (meta.w, meta.h));
        assert!(f.data.chunks_exact(4).any(|p| p[3] == 255), "{}: opaque pixels", game);
        assert!(f.data.chunks_exact(4).any(|p| p[3] == 0), "{}: transparent pixels", game);
    }
}

#[test]
fn without_imagery_the_world_is_a_terrain_schematic() {
    let pack = load("platinum");
    let comp = Compositor::new(pack.clone());
    let m = pack.map(pack.image.as_ref().unwrap().default_map.unwrap()).unwrap();
    let (px, py) = m.world_pos.unwrap();
    let chunk = comp.render_chunk(Scope::World, 0, (px * 16 / 512) as u32, (py * 16 / 512) as u32, RenderOpts::default());
    let colours: std::collections::HashSet<[u8; 4]> = chunk.data.chunks_exact(4).map(|p| [p[0], p[1], p[2], p[3]]).collect();
    assert!(colours.len() >= 3, "walkable / blocked / background at least: {:?}", colours);
}

/// A stored zip of `entries` (what pokemap's `pipeline/router/zip.js` writes).
fn write_zip(path: &Path, entries: &[(&str, Vec<u8>)]) {
    let crc = |d: &[u8]| {
        let mut c = 0xFFFF_FFFFu32;
        for &b in d {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
        }
        !c
    };
    let mut out: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();
    for (name, data) in entries {
        let off = out.len() as u32;
        let c = crc(data);
        let n = name.as_bytes();
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0x21, 0]);
        out.extend_from_slice(&c.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(n.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(n);
        out.extend_from_slice(data);
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0x21, 0]);
        central.extend_from_slice(&c.to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(n.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0u8; 12]);
        central.extend_from_slice(&off.to_le_bytes());
        central.extend_from_slice(n);
    }
    let cd_off = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_off.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    std::fs::File::create(path).unwrap().write_all(&out).unwrap();
}

#[test]
fn the_imagery_draws_world_tiles_and_interiors_from_a_folder_or_a_zip() {
    let pack = load("platinum");
    let img = pack.image.as_ref().unwrap();
    let town = pack.map(img.default_map.unwrap()).unwrap();
    let (px, py) = town.world_pos.unwrap();
    let (cx, cy) = ((px * 16 / 512) as u32, (py * 16 / 512) as u32);
    let interior = pack.maps.iter().find(|m| m.image.is_some()).unwrap();
    let path = interior.image.clone().unwrap();
    let red = [200, 30, 30, 255];
    let blue = [20, 40, 220, 255];
    let world_tile = format!("world/0/{}_{}.webp", cx, cy);
    // png bytes under the webp names: the decoder goes by content
    let files: Vec<(&str, Vec<u8>)> = vec![(world_tile.as_str(), png(512, 512, red)), (path.as_str(), png(64, 48, blue))];

    let dir = scratch("dir");
    for (name, data) in &files {
        let p = dir.join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, data).unwrap();
    }
    let zip = dir.join("imagery.zip");
    write_zip(&zip, &files);

    // the executable embeds the zip and registers it (xpr-app/build.rs + main)
    let leaked: &'static [u8] = Box::leak(std::fs::read(&zip).unwrap().into_boxed_slice());
    imagery::register_embedded("test_game", leaked);
    assert_eq!(imagery::embedded("test_game").map(|b| b.len()), Some(leaked.len()));
    assert!(imagery::embedded("no_such_game").is_none());
    let sources: Vec<Box<dyn imagery::ImageFiles>> = vec![
        Box::new(DirFiles::open(&dir).unwrap()),
        Box::new(ZipFiles::open(&zip).unwrap()),
        Box::new(ZipFiles::from_static("test_game", imagery::embedded("test_game").unwrap()).unwrap()),
    ];
    for files in sources {
        let comp = Compositor::with_imagery(pack.clone(), Some(Arc::new(Imagery::new(files))));
        let opts = RenderOpts { night: false, mask: false };
        let chunk = comp.render_chunk(Scope::World, 0, cx, cy, opts);
        assert_eq!(chunk.get(10, 10), red);
        // a tile the imagery lacks is the background
        let other = comp.render_chunk(Scope::World, 0, cx + 1, cy, opts);
        assert_eq!(other.get(10, 10), compose::BACKGROUND);
        // the interior's picture sits at its frame's image_at
        let (ix, iy) = interior.frame.image_at;
        let px = comp.render_region(Scope::Map(interior.id), geom::scope_rect(&pack, Scope::Map(interior.id)), opts);
        assert_eq!(px.get(ix as usize + 5, iy as usize + 5), blue);
        // the mask dims hidden cells toward the background
        let masked = comp.render_chunk(Scope::World, 0, cx, cy, RenderOpts { night: false, mask: true });
        let hidden = (0..512).flat_map(|y| (0..512).map(move |x| (x, y))).find(|&(x, y)| !img.world_mask.as_ref().unwrap().visible((cx as i32 * 512 + x) / 16, (cy as i32 * 512 + y) / 16));
        if let Some((x, y)) = hidden {
            assert_ne!(masked.get(x as usize, y as usize), red);
        }
    }

    // verify_zip checks size and sha256 against the manifest entry
    let bytes = std::fs::read(&zip).unwrap();
    let sha: String = Sha256::digest(&bytes).iter().map(|b| format!("{:02x}", b)).collect();
    let good = ImageryInfo { version: "t".into(), zip: "imagery.zip".into(), bytes: bytes.len() as u64, sha256: sha, files: 2 };
    assert!(verify_zip(&zip, &good).is_ok());
    assert!(verify_zip(&zip, &ImageryInfo { bytes: 1, ..good.clone() }).is_err());
    assert!(verify_zip(&zip, &ImageryInfo { sha256: "00".into(), ..good }).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
