//! The app build embeds the map pack (feature `embed-map-data` of
//! `xpr-map`): every game loads with no `map_data/` directory in reach —
//! gens 1–3 as tileset packs, gens 4/5 as image-world packs (whose pictures
//! ship separately) — and each built-in version maps to a game.

use xpr_core::consts;
use xpr_map::{game_for_version, MapPack, PackSource};

#[test]
fn every_gen1_to_3_game_loads_from_the_embedded_pack() {
    let source = PackSource { dir: None };
    assert!(xpr_map::embedded::is_embedded());
    for game in ["red_blue", "yellow", "gold_silver", "crystal", "ruby_sapphire", "emerald", "firered_leafgreen"] {
        let pack = MapPack::load(game, &source).unwrap_or_else(|e| panic!("{}: {}", game, e));
        assert!(!pack.tilesets.is_empty(), "{}: tilesets", game);
        assert!(!pack.sprite_png.is_empty(), "{}: sprites", game);
        assert!(!pack.links.trainers.is_empty(), "{}: links", game);
        assert!(!pack.is_image(), "{}: a tileset pack", game);
    }
}

#[test]
fn every_gen4_and_5_game_loads_from_the_embedded_pack() {
    let source = PackSource { dir: None };
    for game in ["diamond_pearl", "platinum", "heartgold_soulsilver", "black_white", "black2_white2"] {
        let pack = MapPack::load(game, &source).unwrap_or_else(|e| panic!("{}: {}", game, e));
        let img = pack.image.as_ref().unwrap_or_else(|| panic!("{}: an image-world pack", game));
        assert!(img.imagery.is_some(), "{}: names its imagery zip", game);
        assert!(img.world_mask.is_some(), "{}: world mask", game);
        assert!(!pack.sprite_png.is_empty(), "{}: sprites", game);
        assert!(!pack.links.trainers.is_empty(), "{}: links", game);
        assert!(!pack.encounters.is_empty(), "{}: encounters", game);
        // the pictures are the executable's to embed (`xpr-app/build.rs`), not the library's
        assert!(!xpr_map::embedded::contains(&format!("{}/imagery.zip", game)));
    }
}

#[test]
fn built_in_versions_map_to_games() {
    for v in consts::VERSION_LIST {
        assert!(game_for_version(v).is_some(), "{} should have a map", v);
    }
    assert_eq!(game_for_version("Red"), game_for_version("Blue"));
    assert_eq!(game_for_version("Yellow"), Some("yellow"));
    assert_eq!(game_for_version("Black 2"), Some("black2_white2"));
    assert_eq!(game_for_version("HeartGold"), game_for_version("SoulSilver"));
}
