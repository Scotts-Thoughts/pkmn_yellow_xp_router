//! List the events of a route file that have errors or warnings, with the
//! folder path, the group name and the message.
//!
//! `cargo run -p xpr-engine --example route_errors -- route.json`

use std::path::{Path, PathBuf};
use std::sync::Arc;

use xpr_data::Registry;
use xpr_engine::tree::{Node, NodeId};
use xpr_engine::Router;

fn walk(router: &Router, id: NodeId, path: &mut Vec<String>, idx: &mut usize) {
    match router.node(id) {
        Some(Node::Folder(f)) => {
            path.push(f.name.clone());
            for &c in &f.children {
                walk(router, c, path, idx);
            }
            path.pop();
        }
        Some(Node::Group(g)) => {
            *idx += 1;
            let lvl = g
                .final_state
                .as_ref()
                .map(|s| format!("Lv{}", s.solo_pkmn.cur_level))
                .unwrap_or_default();
            if let (Ok(item), Some(s)) = (std::env::var("XPR_TRACE_ITEM"), g.final_state.as_ref()) {
                let n: i64 = s.inventory.cur_items.iter().filter(|b| b.base_item.name == item).map(|b| b.num).sum();
                println!("#{idx} {} {item}={n} bag={}", g.name, s.inventory.cur_items.len());
            }
            for m in &g.error_messages {
                println!("#{idx} ERROR [{}] {} {lvl}: {m}", path[1..].join("/"), g.name);
            }
            for m in &g.warning_messages {
                println!("#{idx} warn  [{}] {} {lvl}: {m}", path[1..].join("/"), g.name);
            }
        }
        None => {}
    }
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: route_errors <route.json>");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap();
    let reg = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    let mut router = Router::new(reg);
    router.load(Path::new(&path), false).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut idx = 0;
    let root_id = router.root().id;
    walk(&router, root_id, &mut Vec::new(), &mut idx);
    println!("{idx} groups");
}
