//! The app build embeds the map pack (feature `embed-map-data` of
//! `xpr-map`): every gen 1–3 game loads with no `map_data/` directory in
//! reach, and each built-in version maps to a game or to "no map".

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
    }
    assert!(MapPack::load("platinum", &source).is_err());
}

#[test]
fn built_in_versions_map_to_games() {
    for v in consts::VERSION_LIST {
        let game = game_for_version(v);
        let gen = xpr_data::registry::gen_of_builtin(v).map(|g| g.number()).unwrap_or(0);
        if gen <= 3 {
            assert!(game.is_some(), "{} should have a map", v);
        } else {
            assert!(game.is_none(), "{} has no map yet", v);
        }
    }
    assert_eq!(game_for_version("Red"), game_for_version("Blue"));
    assert_eq!(game_for_version("Yellow"), Some("yellow"));
}
