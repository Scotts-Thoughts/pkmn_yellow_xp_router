//! "Add to route" from the world map through the real controller: the event
//! lands after the selection with the right definition, undo restores the
//! previous route byte for byte, and "add all trainers here" makes one
//! folder in object order (`docs/rust_port/design/world_map/SPEC.md` §3.7).

use std::path::PathBuf;
use std::sync::Arc;

use xpr_app::controller::MainController;
use xpr_app::map::state::query_for_event;
use xpr_core::consts;
use xpr_core::{Config, Paths};
use xpr_data::Registry;
use xpr_engine::NodeId;
use xpr_map::{LinkQuery, MapPack, ObjectKind, PackSource};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn setup(route: &str) -> (Config, MainController) {
    let root = repo_root();
    let paths = Paths::new(root.clone());
    let cfg = Config::load(&root.join("rust/target/no-such-config.json"));
    let registry = Arc::new(Registry::new(root.join("raw_pkmn_data"), PathBuf::new()));
    let mut ctrl = MainController::new(registry, paths);
    ctrl.load_route(&root.join("tests/test_data").join(route));
    let _ = ctrl.take_signals();
    (cfg, ctrl)
}

fn nth_trainer(ctrl: &MainController, n: usize) -> NodeId {
    ctrl.router.all_groups().into_iter().filter(|g| ctrl.router.group(*g).unwrap().event_definition.trainer_def.is_some()).nth(n).expect("trainer fight")
}

fn event_type(ctrl: &MainController, id: NodeId) -> String {
    ctrl.router.group(id).unwrap().event_definition.get_event_type().to_string()
}

#[test]
fn a_trainer_fight_lands_after_the_selection_and_undo_restores_the_route() {
    let (_cfg, mut ctrl) = setup("yellow-pinsir-lv10brock.json");
    let before = ctrl.router.save_bytes();
    let anchor = nth_trainer(&ctrl, 1);
    ctrl.select_new_events(vec![anchor]);
    let _ = ctrl.take_signals();

    let id = ctrl.add_trainer_fight_from_map("Youngster 1").expect("event added");
    let sig = ctrl.take_signals();
    assert!(sig.route_changed && sig.selection_changed);
    assert_eq!(ctrl.get_single_selected_event_id(true), Some(id), "the new event is selected");
    assert_eq!(ctrl.get_previous_event(Some(id)), Some(anchor), "inserted right after the selection");
    let def = &ctrl.router.group(id).unwrap().event_definition;
    assert_eq!(def.trainer_def.as_ref().map(|t| t.trainer_name.as_str()), Some("Youngster 1"));
    assert!(ctrl.has_unsaved_changes());

    ctrl.undo();
    assert_eq!(ctrl.router.save_bytes(), before, "undo restores the route byte for byte");
}

#[test]
fn unknown_trainers_items_and_species_are_refused() {
    let (_cfg, mut ctrl) = setup("yellow-pinsir-lv10brock.json");
    let n = ctrl.router.all_groups().len();
    assert!(ctrl.add_trainer_fight_from_map("No Such Trainer 99").is_none());
    assert!(ctrl.add_item_pickup_from_map("Not An Item").is_none());
    assert!(ctrl.add_wild_from_map("Missingno", 5).is_none());
    assert_eq!(ctrl.router.all_groups().len(), n);
}

#[test]
fn item_pickups_and_wild_encounters_get_the_right_definitions() {
    let (_cfg, mut ctrl) = setup("yellow-pinsir-lv10brock.json");
    let anchor = nth_trainer(&ctrl, 0);
    ctrl.select_new_events(vec![anchor]);

    let item = ctrl.add_item_pickup_from_map("Potion").expect("item added");
    assert_eq!(event_type(&ctrl, item), consts::TASK_GET_FREE_ITEM);
    let inv = ctrl.router.group(item).unwrap().event_definition.item_event_def.clone().unwrap();
    assert_eq!((inv.item_name.as_str(), inv.item_amount, inv.is_acquire, inv.with_money), ("Potion", 1, true, false));

    // the selection followed the pickup, so the wild encounter lands after it
    let wild = ctrl.add_wild_from_map("Pidgey", 3).expect("wild added");
    assert_eq!(event_type(&ctrl, wild), consts::TASK_FIGHT_WILD_PKMN);
    let w = ctrl.router.group(wild).unwrap().event_definition.wild_pkmn_info.clone().unwrap();
    assert_eq!((w.name.as_str(), w.level, w.quantity, w.trainer_pkmn), ("Pidgey", 3, 1, false));
    assert_eq!(ctrl.get_previous_event(Some(wild)), Some(item));
}

#[test]
fn add_all_trainers_makes_one_folder_in_object_order() {
    let (_cfg, mut ctrl) = setup("yellow-pinsir-lv10brock.json");
    let anchor = nth_trainer(&ctrl, 0);
    ctrl.select_new_events(vec![anchor]);
    let names: Vec<String> = ["Bug Catcher 4", "Bug Catcher 5", "No Such Trainer", "Youngster 3"].iter().map(|s| s.to_string()).collect();
    let last = ctrl.add_trainers_in_new_folder("Mt. Moon", &names).expect("folder added");
    let folder = ctrl.router.parent_of(last).unwrap();
    let f = ctrl.router.folder(folder).unwrap();
    assert_eq!(f.name, "Mt. Moon");
    let kids: Vec<String> = f.children.iter().map(|c| ctrl.router.group(*c).unwrap().event_definition.trainer_def.as_ref().unwrap().trainer_name.clone()).collect();
    assert_eq!(kids, vec!["BugCatcher 4", "BugCatcher 5", "Youngster 3"], "unknown names are skipped, order kept, canonical names used");
    assert_eq!(ctrl.get_single_selected_event_id(true), Some(last));
    // a second folder for the same map gets a distinct name
    ctrl.select_new_events(vec![anchor]);
    let again = ctrl.add_trainers_in_new_folder("Mt. Moon", &names).unwrap();
    assert_eq!(ctrl.router.folder(ctrl.router.parent_of(again).unwrap()).unwrap().name, "Mt. Moon Trip:2");
}

#[test]
fn route_events_map_to_link_queries_that_resolve() {
    let (_cfg, ctrl) = setup("yellow-pinsir-lv10brock.json");
    let pack = MapPack::load("yellow", &PackSource::new(Some(repo_root().join("map_data")))).unwrap();
    let mut trainer_queries = 0;
    let mut resolved = 0;
    for g in ctrl.router.all_groups() {
        let Some(q) = query_for_event(&ctrl, g) else { continue };
        match &q {
            LinkQuery::Trainer(_) => {
                trainer_queries += 1;
                if !pack.resolve(&q).is_empty() {
                    resolved += 1;
                }
            }
            LinkQuery::Item(name) => {
                assert!(!pack.resolve(&q).is_empty(), "item pickup {} has no anchor", name);
            }
            LinkQuery::Species(_) => {
                assert!(!pack.resolve(&q).is_empty());
            }
        }
    }
    assert!(trainer_queries >= 5);
    assert_eq!(resolved, trainer_queries, "every trainer fight of the test route is on the map");
    // and the map objects name trainers the route can add
    let brock = pack.objects.iter().find(|o| o.kind == ObjectKind::Trainer && o.trainer_names().iter().any(|n| n == "Brock 1")).expect("Brock's marker");
    assert_eq!(pack.map(brock.map).unwrap().const_name, "PEWTER_GYM");
}
