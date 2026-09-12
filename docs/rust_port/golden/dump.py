"""Golden corpus generator for the Rust port (rust_port_plan.md §5.1).

Runs the Python engine headlessly over route files and writes one JSON
record per route to an output directory, so `xpr-golden verify` can replay
the same files through the Rust engine and diff the results.

Usage:
    py -3.14 docs/rust_port/golden/dump.py OUT_DIR ROUTE_FILE_OR_DIR [...]
    py -3.14 docs/rust_port/golden/dump.py OUT_DIR --data-dir     # saved + outdated routes
    py -3.14 docs/rust_port/golden/dump.py OUT_DIR --tests        # tests/test_data
    py -3.14 docs/rust_port/golden/dump.py OUT_DIR --battles ...  # also dump battle summaries

Each record (`<route name>.golden.json`) contains:
    route:    the bytes Python's Router.save would write (as text), the notes
              export text, and the final route state
    events:   one entry per EventGroup in route order with its rendered
              name/label, row values, tags, error messages, states, items,
              and level-up move defs
    battles:  (with --battles) the BattleSummaryController output for each
              trainer/wild event under the default config
"""
import json
import os
import sys
import traceback

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))))

import controllers.main_controller  # noqa: F401  (resolves the import chain)
from utils.constants import const
from utils.config_manager import config
from utils import setup as app_setup
from pkmn import gen_factory
from routing.router import Router
from routing import route_events

app_setup.init_base_generations()
try:
    gen_factory._gen_factory.reload_all_custom_gens()
except Exception as e:  # custom gens are optional for the corpus
    print(f"WARNING: custom gens failed to load: {e}")


def _state(route_state):
    if route_state is None:
        return None
    return {
        "solo_mon": route_state.solo_pkmn.serialize(),
        "badges_verbose": route_state.badges.to_string(verbose=True),
        "badges_short": route_state.badges.to_string(verbose=False),
        "inventory": route_state.inventory.serialize(),
        "move_list": list(route_state.solo_pkmn.move_list),
        "xp_to_next_level": route_state.solo_pkmn.xp_to_next_level,
        "percent_xp_to_next_level": route_state.solo_pkmn.percent_xp_to_next_level,
        "percent_xp_to_next_level_str": route_state.solo_pkmn.percent_xp_to_next_level_str,
        "unrealized_stat_xp": route_state.solo_pkmn.unrealized_stat_xp.serialize(),
        "realized_stat_xp": route_state.solo_pkmn.realized_stat_xp.serialize(),
        "held_item": route_state.solo_pkmn.held_item,
        "ability": route_state.solo_pkmn.ability,
        "nature": str(route_state.solo_pkmn.nature),
        "cur_stats": route_state.solo_pkmn.cur_stats.serialize(),
        "level": route_state.solo_pkmn.cur_level,
        "xp": route_state.solo_pkmn.cur_xp,
        "name": route_state.solo_pkmn.name,
    }


def _blank_or(v):
    return v


FILTER_TYPES = list(const.ROUTE_EVENT_TYPES) + [const.MAJOR_BATTLE_FILTER]
SEARCH_STRINGS = ["", "a", "rare", "trainer", "potion", "z"]


def _group_record(group):
    ed = group.event_definition
    rec = {
        "kind": "group",
        "name": group.name,
        "label": ed.get_label(),
        "item_label": ed.get_item_label(),
        "event_type": ed.get_event_type(),
        "is_enabled": group.is_enabled(),
        "has_errors": group.has_errors(),
        "error_messages": list(group.error_messages),
        "tags": group.get_tags(),
        "highlight_type": ed.get_highlight_type(),
        "pkmn_after_levelups": group.get_pkmn_after_levelups(),
        "pkmn_level": group.pkmn_level(),
        "xp_to_next_level": group.xp_to_next_level(),
        "percent_xp_to_next_level": group.percent_xp_to_next_level(),
        "xp_gain": group.xp_gain(),
        "total_xp": group.total_xp(),
        "level_gain": group.level_gain(),
        "experience_per_second": group.experience_per_second(),
        "is_major_fight": group.is_major_fight(),
        "definition": ed.serialize(),
        "init_state": _state(group.init_state),
        "final_state": _state(group.final_state),
        "level_up_learn_event_defs": [x.serialize() for x in group.level_up_learn_event_defs],
        "do_render_filters": {ft: group.do_render(filter_types=[ft]) for ft in FILTER_TYPES},
        "do_render_search": {s: group.do_render(search=s) for s in SEARCH_STRINGS},
        "items": [],
    }
    for item in group.event_items:
        rec["items"].append({
            "name": item.name,
            "error_message": item.error_message,
            "is_enabled": item.is_enabled(),
            "tags": item.get_tags(),
            "pkmn_level": item.pkmn_level(),
            "xp_to_next_level": item.xp_to_next_level(),
            "percent_xp_to_next_level": item.percent_xp_to_next_level(),
            "xp_gain": item.xp_gain(),
            "total_xp": item.total_xp(),
            "level_gain": item.level_gain(),
            "definition": item.event_definition.serialize(),
            "shares_group_definition": item.event_definition is group.event_definition,
            "to_defeat_mon": None if item.to_defeat_mon is None else item.to_defeat_mon.to_string(verbose=True),
            "exp_split_num": item.exp_split_num,
            "pay_day_amount": item.pay_day_amount,
            "defeating_trainer": item.defeating_trainer,
            "final_state": _state(item.final_state),
        })
    return rec


def _folder_record(folder):
    return {
        "kind": "folder",
        "name": folder.name,
        "notes": folder.event_definition.notes,
        "expanded": folder.expanded,
        "enabled": folder._enabled,
        "is_enabled": folder.is_enabled(),
        "has_errors": folder.has_errors(),
        "tags": folder.get_tags(),
        "init_state": _state(folder.init_state),
        "final_state": _state(folder.final_state),
        "do_render_filters": {ft: folder.do_render(filter_types=[ft]) for ft in FILTER_TYPES},
        "do_render_search": {s: folder.do_render(search=s) for s in SEARCH_STRINGS},
        "children": [],
    }


def _walk(folder):
    rec = _folder_record(folder)
    for child in folder.children:
        if isinstance(child, route_events.EventFolder):
            rec["children"].append(_walk(child))
        else:
            rec["children"].append(_group_record(child))
    return rec


def _notes_text(router):
    output = []
    router._export_recursive(router.root_folder, 0, output)
    return "\n".join(output)


def dump_route(path, out_dir, with_battles=False):
    name = os.path.splitext(os.path.basename(path))[0]
    router = Router()
    record = {"source": os.path.abspath(path), "name": name}
    try:
        router.load(path)
    except Exception as e:
        record["load_error"] = f"{type(e).__name__}: {e}"
        with open(os.path.join(out_dir, name + ".golden.json"), "w", encoding="utf-8") as f:
            json.dump(record, f, indent=1, ensure_ascii=True)
        return False

    out_obj = router.serialize()
    out_obj[const.TEST_MOVES_KEY] = router.test_moves
    record["route"] = {
        "version": router.pkmn_version,
        "generation": gen_factory.current_gen_info().get_generation(),
        "saved_text": json.dumps(out_obj, indent=4),
        "notes_text": _notes_text(router),
        "init_state": _state(router.init_route_state),
        "final_state": _state(router.get_final_state()),
        "defeated_trainers": sorted(router.defeated_trainers),
        "effective_defeated_trainers": sorted(router.get_effective_defeated_trainers()),
        "folder_names": list(router.folder_lookup.keys()),
        "level_up_move_defs": {str(k): v.serialize() for k, v in router.level_up_move_defs.items()},
        "test_moves": list(router.test_moves),
    }
    record["tree"] = _walk(router.root_folder)
    if with_battles:
        from docs.rust_port.golden import battles as battle_dump
        record["battles"] = battle_dump.dump_battles(router)
    with open(os.path.join(out_dir, name + ".golden.json"), "w", encoding="utf-8") as f:
        json.dump(record, f, indent=1, ensure_ascii=True)
    return True


def _collect(args):
    files = []
    for a in args:
        if a == "--tests":
            d = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))), "tests", "test_data")
            files.extend(os.path.join(d, f) for f in sorted(os.listdir(d)) if f.endswith(".json"))
        elif a == "--data-dir":
            for d in (const.SAVED_ROUTES_DIR, const.OUTDATED_ROUTES_DIR):
                if d and os.path.exists(d):
                    files.extend(os.path.join(d, f) for f in sorted(os.listdir(d)) if f.endswith(".json"))
        elif a == "--saved":
            d = const.SAVED_ROUTES_DIR
            files.extend(os.path.join(d, f) for f in sorted(os.listdir(d)) if f.endswith(".json"))
        elif os.path.isdir(a):
            files.extend(os.path.join(a, f) for f in sorted(os.listdir(a)) if f.endswith(".json"))
        else:
            files.append(a)
    return files


def main(argv):
    if len(argv) < 2:
        print(__doc__)
        return 2
    out_dir = argv[0]
    os.makedirs(out_dir, exist_ok=True)
    args = argv[1:]
    with_battles = "--battles" in args
    args = [a for a in args if a != "--battles"]
    limit = None
    for a in list(args):
        if a.startswith("--limit="):
            limit = int(a.split("=", 1)[1])
            args.remove(a)
    files = _collect(args)
    if limit is not None:
        files = files[:limit]
    ok = 0
    failed = 0
    for idx, path in enumerate(files):
        try:
            if dump_route(path, out_dir, with_battles=with_battles):
                ok += 1
            else:
                failed += 1
        except Exception:
            failed += 1
            print(f"ERROR dumping {path}")
            traceback.print_exc()
        if (idx + 1) % 50 == 0:
            print(f"... {idx + 1}/{len(files)}")
    print(f"done: {ok} dumped, {failed} failed/unloadable, {len(files)} total -> {out_dir}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
