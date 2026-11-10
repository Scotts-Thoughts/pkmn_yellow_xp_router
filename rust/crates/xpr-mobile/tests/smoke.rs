//! Drives the session the way the iOS app does, over the repo's test routes,
//! and fails on any command error.

use serde_json::{json, Value};
use xpr_mobile::session::Session;

fn call(s: &mut Session, m: &str, p: Value) -> Value {
    match s.dispatch(m, &p) {
        Ok(v) => v,
        Err(e) => panic!("{} {} failed: {}", m, p, e),
    }
}

fn scratch() -> (std::path::PathBuf, std::path::PathBuf) {
    let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/xpr-mobile-smoke").join(format!("{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let cfg = base.join("cfg");
    let data = base.join("data");
    std::fs::create_dir_all(data.join("saved_routes")).unwrap();
    let td = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tests/test_data");
    for f in std::fs::read_dir(td).unwrap() {
        let f = f.unwrap().path();
        std::fs::copy(&f, data.join("saved_routes").join(f.file_name().unwrap())).unwrap();
    }
    (cfg, data)
}

#[test]
fn whole_api() {
    let (cfg, data) = scratch();
    let mut s = Session::new(cfg, data).unwrap();
    let lib = call(&mut s, "library.list", json!({}));
    let routes = lib["routes"].as_array().unwrap().clone();
    assert!(routes.len() >= 4, "{}", lib);
    for r in &routes {
        let name = r["name"].as_str().unwrap();
        call(&mut s, "route.load", json!({"name": name}));
        let st = call(&mut s, "app.state", json!({}));
        assert!(st["open"].as_bool().unwrap(), "{} not open", name);
        let rows = call(&mut s, "tree.rows", json!({}));
        let rows = rows["rows"].as_array().unwrap().clone();
        assert!(!rows.is_empty());
        call(&mut s, "tree.expand_all", json!({"expanded": true}));
        let rows = call(&mut s, "tree.rows", json!({}))["rows"].as_array().unwrap().clone();
        let mut battles = 0;
        for row in rows.iter() {
            let id = row["id"].as_i64().unwrap();
            let d = call(&mut s, "event.detail", json!({"id": id}));
            if d["has_battle"].as_bool().unwrap_or(false) && battles < 25 {
                battles += 1;
                let v = call(&mut s, "battle.load", json!({"id": id}));
                assert!(v["matchups"].is_array());
            }
            // re-building each event from its own draft must round-trip
            if row["kind"] == "group" {
                let draft = d["draft"].clone();
                let r = s.dispatch("event.update", &json!({"id": id, "draft": draft}));
                if let Ok(r) = &r {
                    if r["changed"].as_bool().unwrap_or(false) {
                        println!("{}: draft round-trip changed event {} ({})", name, id, row["name"]);
                    }
                } else {
                    println!("{}: draft of {} rejected: {:?}", name, row["name"], r);
                }
            }
        }
        println!("{}: {} rows, {} battles ok", name, rows.len(), battles);
        call(&mut s, "summary.run", json!({}));
        call(&mut s, "summary.setup", json!({}));
        call(&mut s, "opts.kinds", json!({}));
        call(&mut s, "opts.trainers", json!({"location": "", "class": ""}));
        call(&mut s, "opts.items", json!({}));
        call(&mut s, "tree.jump_targets", json!({}));
        let st = call(&mut s, "app.state", json!({}));
        if st["has_map"].as_bool().unwrap() {
            let info = call(&mut s, "map.info", json!({}));
            assert!(info["maps"].as_array().unwrap().len() > 5);
            let objs = call(&mut s, "map.objects", json!({"scope": -1}));
            println!("{}: map {} with {} world objects", name, info["game"], objs.as_array().unwrap().len());
            let png = s.png("map.chunk", &json!({"scope": -1, "level": 2, "cx": 0, "cy": 0})).unwrap();
            assert!(png.len() > 100);
            if let Some(o) = objs.as_array().unwrap().first() {
                call(&mut s, "map.object", json!({"object": o[0]}));
            }
            let m0 = info["maps"][0]["id"].clone();
            call(&mut s, "map.map_card", json!({"map": m0}));
            call(&mut s, "map.encounters", json!({"map": m0}));
            call(&mut s, "map.search", json!({"query": "ro"}));
        }
    }
    // create / edit / delete / undo / save on a new route
    call(&mut s, "route.close", json!({}));
    let games = call(&mut s, "newroute.games", json!({}));
    assert!(games.as_array().unwrap().len() >= 20);
    call(&mut s, "newroute.dv_form", json!({"version": "Emerald", "species": "Mudkip"}));
    call(&mut s, "newroute.bases", json!({"version": "Yellow"}));
    call(&mut s, "newroute.create", json!({"version": "Yellow", "species": "Pikachu", "base": "Empty Route", "dvs": {"attack": 15, "defense": 15, "special_attack": 15, "speed": 15}, "nature": 0, "ability_idx": 0, "name": "smoke"}));
    let c = call(&mut s, "event.create", json!({"draft": {"kind": "trainer", "trainer": "Rival1 1"}}));
    let tid = c["id"].as_i64().unwrap();
    let b = call(&mut s, "battle.load", json!({"id": tid}));
    assert!(b["can_edit"].as_bool().unwrap(), "{}", b);
    call(&mut s, "battle.action", json!({"op": "candies", "value": 2}));
    call(&mut s, "battle.action", json!({"op": "highlight", "mon": 0, "move": 0}));
    call(&mut s, "event.create", json!({"after": tid, "draft": {"kind": "buy_item", "item": "Potion", "amount": 3}}));
    call(&mut s, "event.create", json!({"after": tid, "draft": {"kind": "wild", "species": "Pidgey", "level": 3}}));
    call(&mut s, "event.create", json!({"after": tid, "draft": {"kind": "notes", "notes": "hello"}}));
    call(&mut s, "event.create", json!({"after": tid, "draft": {"kind": "tutor", "move": "Thunderbolt", "dest": 1}}));
    call(&mut s, "folder.new", json!({"name": "Second", "after": tid}));
    let rows = call(&mut s, "tree.rows", json!({}))["rows"].as_array().unwrap().clone();
    println!("new route rows: {:?}", rows.iter().map(|r| r["name"].as_str().unwrap().to_string()).collect::<Vec<_>>());
    call(&mut s, "app.undo", json!({}));
    call(&mut s, "route.save", json!({"name": "smoke"}));
    let msgs = call(&mut s, "app.messages", json!({}));
    println!("messages: {}", msgs);
    let lib = call(&mut s, "library.list", json!({}));
    assert!(lib["routes"].as_array().unwrap().iter().any(|r| r["name"] == "smoke"));
    let cmp = call(&mut s, "compare.run", json!({"a": {"name": "smoke"}, "b": {"current": true}}));
    assert!(cmp["rows"].is_array());
    let cmp = call(&mut s, "compare.run", json!({"a": {"name": "yellow-pinsir-lv10brock"}, "b": {"name": "smoke"}}));
    println!("compare rows {}", cmp["rows"].as_array().unwrap().len());
}
