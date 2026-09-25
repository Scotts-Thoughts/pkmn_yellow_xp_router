//! Overworld sprite frames: layouts, mirroring, transparency and palettes
//! per generation (SPEC §5 Phase 5 step 1).

use std::path::PathBuf;

use xpr_map::sprites::{extract_frame, frame_key, gen1_sprite_palette, gen2_sprite_palette, render_frame, Dir, SpriteCache};
use xpr_map::{Facing, MapPack, ObjectKind, PackSource, Pixmap};

fn load(game: &str) -> MapPack {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap();
    MapPack::load(game, &PackSource::new(Some(root.join("map_data")))).unwrap()
}

fn opaque(p: &Pixmap) -> usize {
    p.data.chunks_exact(4).filter(|px| px[3] == 255).count()
}

fn mirrored(a: &Pixmap, b: &Pixmap) -> bool {
    a.w == b.w && a.h == b.h && (0..a.h).all(|y| (0..a.w).all(|x| a.get(x, y) == b.get(a.w - 1 - x, y)))
}

#[test]
fn gen1_walking_sheets_give_16px_frames_with_the_map_palette() {
    let pack = load("yellow");
    let mut cache = SpriteCache::default();
    let sheet = cache.sheet(&pack, "youngster").expect("youngster sheet");
    assert_eq!((sheet.w, sheet.h), (16, 96));
    let meta = pack.sprites.get("SPRITE_YOUNGSTER").unwrap().clone();
    let pallet = pack.map_by_const("PALLET_TOWN").unwrap().id;
    let (pal, idx) = gen1_sprite_palette(&pack, pallet).unwrap();
    assert_eq!(idx, 1, "Pallet Town uses PAL_PALLET");
    let down = extract_frame(1, &sheet, &meta, Dir::Down, Some(&pal), false).unwrap();
    assert_eq!((down.w, down.h), (16, 16));
    let n = opaque(&down);
    assert!(n > 40 && n < 256, "a sprite has transparent and opaque pixels ({})", n);
    // every opaque colour comes from the remapped map palette
    let allowed = [pal[1], pal[2], pal[3]];
    for px in down.data.chunks_exact(4).filter(|p| p[3] == 255) {
        assert!(allowed.contains(&[px[0], px[1], px[2]]), "colour {:?} not in palette", px);
    }
    let left = extract_frame(1, &sheet, &meta, Dir::Left, Some(&pal), false).unwrap();
    let right = extract_frame(1, &sheet, &meta, Dir::Right, Some(&pal), false).unwrap();
    assert!(mirrored(&left, &right), "right is the mirror of left");
    let up = extract_frame(1, &sheet, &meta, Dir::Up, Some(&pal), false).unwrap();
    assert_ne!(up.data, down.data);
    // a still sheet ignores the direction
    let ball = cache.sheet(&pack, "poke_ball").unwrap();
    let bmeta = pack.sprites.get("SPRITE_POKE_BALL").unwrap().clone();
    let a = extract_frame(1, &ball, &bmeta, Dir::Down, Some(&pal), false).unwrap();
    let b = extract_frame(1, &ball, &bmeta, Dir::Right, Some(&pal), false).unwrap();
    assert_eq!(a.data, b.data);
}

#[test]
fn gen2_npc_palettes_follow_the_palette_name_and_time_of_day() {
    let pack = load("crystal");
    let olivine = pack.map_by_const("OLIVINE_CITY").unwrap().id;
    let center = pack.map_by_const("OLIVINE_POKECENTER_1F").unwrap().id;
    let (red_day, key_day) = gen2_sprite_palette(&pack, olivine, Some("PAL_NPC_RED"), false).unwrap();
    let (red_nite, key_nite) = gen2_sprite_palette(&pack, olivine, Some("PAL_NPC_RED"), true).unwrap();
    assert_ne!(red_day, red_nite, "night uses the nite palettes outdoors");
    assert_ne!(key_day, key_nite);
    let (red_indoor, _) = gen2_sprite_palette(&pack, center, Some("PAL_NPC_RED"), true).unwrap();
    assert_eq!(red_indoor, red_day, "indoor maps keep the day palette at night");
    let (blue, _) = gen2_sprite_palette(&pack, olivine, Some("PAL_NPC_BLUE"), false).unwrap();
    let (unknown, _) = gen2_sprite_palette(&pack, olivine, Some("0"), false).unwrap();
    assert_eq!(unknown, blue, "unknown palette names fall back to blue, like pokemap");
    // a real trainer object renders with its palette
    let mut cache = SpriteCache::default();
    let obj = pack.objects.iter().find(|o| o.kind == ObjectKind::Trainer && o.sprite.is_some() && o.pal.is_some()).unwrap();
    let frame = render_frame(&pack, &mut cache, obj, false).unwrap();
    assert!(opaque(&frame) > 0);
    let key = frame_key(&pack, obj, false).unwrap();
    assert!(key.pal.contains(obj.pal.as_deref().unwrap()));
}

#[test]
fn gen3_frames_are_keyed_and_mirrored_and_berry_trees_are_grown() {
    let pack = load("emerald");
    let mut cache = SpriteCache::default();
    let meta = pack.sprites.get("OBJ_EVENT_GFX_YOUNGSTER").unwrap().clone();
    assert_eq!((meta.w, meta.h), (16, 32));
    let sheet = cache.sheet(&pack, &meta.file).unwrap();
    let down = extract_frame(3, &sheet, &meta, Dir::Down, None, false).unwrap();
    assert_eq!((down.w, down.h), (16, 32));
    assert_eq!(down.get(0, 0)[3], 0, "the green key colour is removed");
    assert!(opaque(&down) > 100);
    let left = extract_frame(3, &sheet, &meta, Dir::Left, None, false).unwrap();
    let right = extract_frame(3, &sheet, &meta, Dir::Right, None, false).unwrap();
    assert!(mirrored(&left, &right));
    assert_ne!(left.data, down.data);
    // berry trees: the last (grown) frame, regardless of facing
    let bmeta = pack.sprites.get("OBJ_EVENT_GFX_BERRY_TREE_ORAN").unwrap().clone();
    let bsheet = cache.sheet(&pack, &bmeta.file).unwrap();
    let grown = extract_frame(3, &bsheet, &bmeta, Dir::Left, None, true).unwrap();
    let frames = bsheet.w / bmeta.w as usize;
    let mut manual = Pixmap::new(16, 32);
    if let xpr_map::sprites::SheetPixels::Rgba(rgba) = &bsheet.px {
        for y in 0..32 {
            for x in 0..16 {
                let i = (y * bsheet.w + (frames - 1) * 16 + x) * 4;
                manual.put(x, y, [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]);
            }
        }
    }
    let same_opaque = grown.data.chunks_exact(4).zip(manual.data.chunks_exact(4)).filter(|(a, _)| a[3] == 255).all(|(a, b)| a[..3] == b[..3]);
    assert!(same_opaque && opaque(&grown) > 50);
    // objects facing differently get different keys; item balls have a frame too
    let ball = pack.objects.iter().find(|o| o.kind == ObjectKind::Item && o.sprite.as_deref() == Some("OBJ_EVENT_GFX_ITEM_BALL")).unwrap();
    assert!(render_frame(&pack, &mut cache, ball, false).is_some());
    let mut a = ball.clone();
    a.facing = Some(Facing::Left);
    let mut b = ball.clone();
    b.facing = Some(Facing::Up);
    assert_ne!(frame_key(&pack, &a, false), frame_key(&pack, &b, false));
}
