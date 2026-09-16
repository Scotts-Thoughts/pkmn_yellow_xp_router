# Bag reordering (gen 1) — implementation plan

Rust port only. The Python app is not touched.

## Decisions (agreed 2026-09-16)

| Topic | Decision |
|---|---|
| Scope | Generation 1 only (Red / Blue / Yellow). Recorder detection and UI creation are gen-1 gated; the engine applies the event for any route that contains one. |
| Storage | A list of swaps, each `item name + slot` on both sides: `Potion (3) <-> Master Ball (7)`. |
| Slot numbering | 1-based (top of the bag is slot 1). The inventory panel changes from `# 00` to `# 01` to match. |
| What counts as a reorder | The relative order of the items that exist both before and after the change differs. A removal that shifts everything up keeps relative order, so it is not a reorder. |
| Mixed window (quantity change + swap in one detection window) | Emit the quantity events as today, then one reorder event computed against the bag *after* those quantity changes. |
| Mismatch on recalc | Slots are exact only at recording time (the recorder reads them from the real bag). When the engine applies a swap it matches by **item name**; the stored slot is a hint. Name found at the recorded slot: clean. Name found at another slot: swap it anyway and flag a **warning** (yellow row). Name not in the bag: skip that swap, flag a warning. Warnings never make the run invalid. |
| Merging consecutive reorders | Never. Each detection window is its own event. |
| Editor | Drag / up-down list of the bag as it is right before the event. Saving stores the swaps that turn the pre-event order into the edited order. |
| Filter bar | New toggle with its own icon, shortcut id and config entry. |

## Game facts that shape the design

- The in-game SELECT swap (`pokeyellow/engine/menus/swap_items.asm`) is a
  true two-slot exchange, not an insert. Swapping two stacks of the same item
  merges them instead (99 cap); the app stores one stack per name so that case
  is invisible to it.
- A pickup appends to the end of the bag; using the last of an item removes
  its slot and shifts the rest up. The engine's `Inventory::add_item` /
  `remove_item` behave the same way (`rust/crates/xpr-engine/src/state.rs:77-149`),
  so after the recorder's quantity events the engine bag order equals the
  game's order minus the swaps.
- The recorder already snapshots the bag in slot order:
  `ItemCache = IndexMap<Value, i64>` (`xpr-recorder/src/games/common.rs:149`),
  filled by `Gen1Machine::item_cache` (`games/gen1.rs:757`). Only
  `item_diff` (counts per name) is compared today, so swaps produce nothing.
- Detection timing is already coalesced: the inventory-change state exits 2
  game-seconds after the last bag property change (`gen1.rs:1016-1036`), and
  in-battle bag changes wait 2 seconds too (`gen1.rs:1113-1148`). Several
  quick swaps land in one snapshot diff.
- The recorder never adds Oak's Parcel to the route (`gen1.rs:781,796`), so
  the engine bag has no parcel while the game's does. Reorder detection must
  drop the parcel from both orders before computing slots.

## Data model

New in `xpr-core/src/consts.rs`:

```rust
pub const TASK_REORDER_BAG: &str = "Reorder Bag";   // event type AND the route-file key
```

Add it to `ROUTE_EVENT_TYPES` (array length 17 -> 18, after `TASK_USE_ITEM`).
`xpr-golden/src/record.rs:48` derives its filter list from that array, so it
picks the type up automatically.

New in `xpr-engine/src/events.rs`:

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct BagSwap {
    pub item_a: String,
    pub slot_a: usize,   // 1-based
    pub item_b: String,
    pub slot_b: usize,   // 1-based
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BagReorderEventDefinition {
    pub swaps: Vec<BagSwap>,
}
```

Route JSON (the value under the `"Reorder Bag"` key):

```json
"Reorder Bag": [["Potion", 3, "Master Ball", 7], ["Antidote", 1, "Potion", 7]]
```

Swaps apply in order; the slot numbers of a later swap refer to the bag after
the earlier swaps in the same event. Deserialize accepts any array of 4-element
arrays (an empty array is a valid, empty reorder); a malformed entry fails the
route load with a message naming the entry, like a malformed trainer or
evolution does, because degrading to notes would drop the swaps on the next
save. `EventDefinition` gains
`bag_reorder: Option<BagReorderEventDefinition>`, a `with_bag_reorder`
constructor, and entries in `Default`, `serialize`, `deserialize`,
`get_event_type`, `non_battle_label`.

Label / notes-export text (`get_label`, used by `export_single_entry_def`):

```
Reorder Bag: Potion (3) <-> Master Ball (7), Antidote (1) <-> Potion (7)
Reorder Bag: (no swaps)
```

### Shared helper: order -> swaps

`xpr-engine/src/events.rs` (or a small `bag_order.rs` module re-exported from
`lib.rs`):

```rust
/// Swaps that turn `old` into `new` (same names, different order).
/// Err when the two lists are not permutations of each other.
pub fn swaps_between(old: &[String], new: &[String]) -> Result<Vec<BagSwap>, String>
```

Selection-style walk: `cur = old.clone()`; for `i` in `0..n`, if
`cur[i] != new[i]`, find `j > i` with `cur[j] == new[i]`, record
`BagSwap { item_a: cur[i], slot_a: i+1, item_b: cur[j], slot_b: j+1 }`, swap.
A single transposition yields exactly one swap; a k-cycle yields k-1. Names
are unique in the engine bag (`add_item` merges by name), so lookups are
unambiguous. Used by both the recorder and the editor, so the two always
agree on the stored form.

## Engine (`xpr-engine`)

1. `state.rs` — `Inventory::swap_items(&self, swaps: &[BagSwap]) -> (Inventory, Vec<String>)`.
   Name-first, never fails. For each swap, in order, on a clone:
   - locate `item_a` and `item_b` by name (`index_of`);
   - both found at their recorded slots: `Vec::swap`, no message;
   - both found, one or both elsewhere: `Vec::swap` at the found positions and
     push `"Potion was at slot 2, not slot 3"` (one message per side that moved);
   - either name missing: skip the swap and push
     `"Cannot swap Potion (3) <-> Master Ball (7): no Master Ball in bag"`.
   The returned messages are warnings, not errors.
2. `state.rs` — `RouteState::reorder_bag(&self, swaps) -> (RouteState, Vec<String>)`
   wrapping the above; the state always advances to the swapped inventory.
3. `recalc.rs:262-268` — add an `else if let Some(r) = &item.event_definition.bag_reorder`
   branch next to the `item_event_def` one. Its messages go to the new
   `item.warning_message` (joined with `", "`), never to `error_message`, so
   the run status stays Valid and the Invalid Events filter ignores them.

### Warning state (new, engine + UI)

Today an event is either clean or in error (red tag `EVENT_TAG_ERRORS`, group
name replaced by the error text at `recalc.rs:478`). Reorders need a third
state that keeps the label and colours the row yellow:

- `tree.rs` — `EventItem.warning_message: String`,
  `EventGroup.warning_messages: Vec<String>`, `has_warnings()` on both.
  `has_errors()` is unchanged (warnings do not count).
- `recalc.rs:464-481` — collect item warnings into the group the same way
  errors are collected, but leave `g.name` as the label.
- `recalc.rs:215-227` — reset `warning_message` where `error_message` is
  reset (disabled events clear both).
- `consts.rs` — `EVENT_TAG_WARNINGS`; `view.rs:99-126` `get_tags` returns it
  for a group/item with warnings and no errors, after the error check and
  before highlights. Folders do not propagate warnings (a collapsed folder
  with a warning inside stays uncoloured; errors keep their existing
  `child_errors` propagation).
- `route_list.rs` — map `EVENT_TAG_WARNINGS` to `theme.warning` wherever
  `EVENT_TAG_ERRORS` is mapped to `theme.failure` (the `highlight_colors`
  table the row painter reads at `route_list.rs:634`).
- Details panel: show the warning text in `theme.warning` above the editor
  when the selected event has one (the editor also lists it, see below).
- Search: the Invalid Events toggle stays errors-only; the warning text is
  not searchable. No new filter for warnings.

Recorded slots stay as recorded; the engine does not rewrite them. The editor
hands the loaded swaps back untouched on every save (a notes edit saves through
the same path) and only recomputes them from the current pre-event bag once
the user has rearranged the list, so rearranging is how a user "heals" stale
slots. A warning never hides a highlight: `get_tags` checks highlights before
warnings (errors still come first).
4. `events.rs` — everything in "Data model" above. `do_render` searches the
   label, so `"potion"` finds reorder events involving Potion for free.
5. `lib.rs` — export `BagSwap`, `BagReorderEventDefinition`, `swaps_between`.

Nothing in `router.rs` needs to change: reorder events are ordinary single-item
groups, and export uses `get_label`.

## Recorder (`xpr-recorder/src/games/gen1.rs` only)

`item_cache_update` (`gen1.rs:769`) already has `old = self.cached_items`
(before it is overwritten at line 772) and `new_cache`. After the existing
gained/lost loops, when `generate_events`:

```
old_names = keys of old (raw GameHook names, bag order) minus Oak's Parcel
new_names = keys of new_cache                            minus Oak's Parcel
survivors_old = old_names.filter(in new_names)
survivors_new = new_names.filter(in old_names)
if survivors_old != survivors_new:
    expected = survivors_old ++ new_names.filter(not in old_names)   // what the engine bag looks like after the queued quantity events
    swaps = swaps_between(app_names(expected), app_names(new_names))
    queue EventDefinition::with_bag_reorder(swaps)
```

`app_names` maps through `conv.item_name_convert`; names the item DB does not
know are left out of both orders (like the parcel), since their acquire never
reached the route and the engine's bag cannot hold them. The deprecated
mapper's `--End of list--` terminator is skipped the same way.
Slots come straight from the console's bag, so a freshly recorded event is
exact by construction; the recorder does no matching of its own. Queue
the reorder *after* the quantity events so the engine sees the bag they
produce. Every existing caller benefits (inventory-change exit, in-battle
item wait, rare candy / TM / vitamin exits); the last three cannot produce a
reorder but the check is harmless.

`process_one` (`gen1.rs:1316`): add a branch that validates each swap's item
names against `gen.item_db()` and calls `ctx.add_error` on an unknown name,
mirroring the item-event branch.

`controller.rs:461-478` (consecutive-event merging): no branch for reorders
(decision: never merge).

Gen 2-5 machines keep their own `item_cache_update` copies untouched.

## UI (`xpr-app`)

1. **Inventory panel** `state_views.rs:240,243,248` — print `idx + 1`
   (`# 01`..`# 20`) so slots match the event text.
2. **Editor** `editors.rs` — new `BagReorderEditor` in `EventEditors`:
   - `load_event(ctx, def)`: base order = `ctx.cur_state.inventory.cur_items`
     names (this is the pre-event state, same source the backpack filter uses
     at `editors.rs:826`). Apply the stored swaps with `Inventory::swap_items`
     to get the displayed order; any warnings it returns are listed in
     `theme.warning` under the list.
   - `ui`: one row per slot `1. 3x Potion` with a drag handle
     (egui 0.33 `ui.dnd_drag_source` / `dnd_drop_zone`) plus up/down buttons
     as the keyboard-friendly fallback. Any change returns
     `EditorOutput::delayed()` like the notes editor.
   - `get_event`: `swaps_between(base_order, displayed_order)` ->
     `EventDefinition::with_bag_reorder`.
   - Wire the three `match ctx.event_type` blocks (`editors.rs:1193,1222,1239`).
3. **Creating one by hand** (gen 1 only, gated with `gen.get_generation() == 1`
   the way `quick_add.rs:331` gates hold items):
   - `quick_add.rs` Items group: a `Reorder` button that inserts an empty
     reorder event after the selection and selects it; the user then arranges
     it in the details panel.
   - `inline_creator.rs:22` `EVENT_TYPES` gains `("Reorder Bag", "reorder_bag")`
     (16 -> 17) and `key_for_event_type` gains the mapping; the strip has no
     per-type fields for it and inserts an empty event the same way.
4. **Filter bar** — `filter_bar.rs` `TOGGLE_ORDER` (17 -> 18, after
   `TASK_USE_ITEM`), `short_label` `"Rb"`, `icon_file` `"TASK_REORDER_BAG"`,
   `tooltip` `"Reorder Bag"`, `shortcut_id` `"filter_reorder_bag"`;
   `app.rs:573` filter shortcut table; `config.rs` default (`:89`), description
   (`:177`) and ordering (`:290`) tables; new greyscale-with-alpha
   `icons/filter icons/TASK_REORDER_BAG.png` (two opposing arrows, same
   dimensions as its siblings; `build.rs` embeds the folder, `assets.rs:74`
   loads by stem). Add it to the asset smoke test list at `assets.rs:135`.
5. `route_list.rs:650-661` `quantity_of` returns `None` for reorders, so the
   inline quantity editor is not offered. No other list change: rows render
   the label.

## Save-file compatibility

- Rust: routes without the key load as before; the new key is only written
  when the event exists.
- Python (deprecated): `EventDefinition.deserialize` ignores unknown keys, so a
  reorder event opens there as an empty notes event and is dropped on re-save
  from Python. Record this in `docs/rust_port/KNOWN_ISSUES.md` under the
  recorder section.
- `docs/rust_port/recording` compares Rust against Python, which can never
  emit the event, so no parity scenario is added; the recorder is covered by
  the unit test below.

## Tests

`xpr-engine` (unit tests beside the code and a new `tests/bag_reorder.rs`):

- `swaps_between`: identity -> empty; one transposition -> one swap with the
  right names/slots; a 3-cycle -> two swaps; property check that applying the
  returned swaps to `old` reproduces `new`; mismatched multisets -> `Err`.
- `Inventory::swap_items`: exact slots -> swapped, no messages; name at a
  different slot -> swapped at the real position, one warning naming both
  slots; missing name -> that swap skipped with a warning, later swaps still
  applied; slot past the end with the name elsewhere -> treated as "different
  slot".
- Definition serialize/deserialize round trip, `get_event_type`, label text
  for one, several and zero swaps.
- Router-level: build a Yellow route through the `Router` API (pattern in
  `tests/route_processing.rs`), add three purchases, a reorder, check the
  final `cur_items` order, no error, no warning; insert another purchase
  before the reorder -> the reorder still swaps the right items, carries a
  warning, `has_errors()` is false and `root().has_errors()` is false; delete
  one of the swapped items' purchases -> warning, the other swaps still
  applied; `get_tags` returns `EVENT_TAG_WARNINGS`; disabling the event
  clears the warning.

`xpr-recorder/tests/gen1_fsm.rs` (same `Sim` scaffolding, bag seeded with
Potion / Poke Ball / Antidote):

- Swap slots 1 and 3 via `bag.items.0.item` / `.quantity` and
  `bag.items.2.*`, tick 3 -> exactly one event, a reorder
  `Potion (1) <-> Antidote (3)`, no item events.
- Use the last Potion at slot 1 (`item_count` 3 -> 2, entries shift up)
  -> one use event, no reorder.
- Mixed: use the Potion and swap the remaining two in the same window
  -> the use event, then a reorder computed on the shifted bag
  (`Poke Ball (1) <-> Antidote (2)`).
- Oak's Parcel in slot 1 plus a swap of the two items below it -> slots
  reported without the parcel.

`xpr-app`: the existing embedded-asset test covers the new icon; run the app
with a gen 1 route to check the editor, panel numbering and the toggle.

Commands: `cargo test -p xpr-engine -p xpr-recorder -p xpr-app`,
`cargo clippy --workspace`, then `cargo build -p xpr-app` and a manual pass.

## Implementation order

1. `xpr-core` const + `ROUTE_EVENT_TYPES`.
2. `xpr-engine`: warning fields + tag + `get_tags`; `BagSwap`, definition,
   `swaps_between`, `Inventory::swap_items`, `RouteState::reorder_bag`,
   `recalc` branch, exports, unit tests.
3. `xpr-recorder` gen 1 detection + `process_one` validation + FSM tests.
4. `xpr-app`: warning row colour + details-panel text, panel numbering,
   editor, quick-add / inline entries, filter bar, icon, config tables.
5. Docs: `KNOWN_ISSUES.md` note; `TESTING.md` checklist line for the editor and
   the toggle.

## Edge cases and limits

- **Initial inventory differing from the game.** A route whose starting bag
  has extra or missing items shifts every recorded slot. The swaps still
  apply by name, so the bag order is right, but each such event shows a
  warning until the start state is fixed or the events are re-saved from the
  editor. Worth a line in the recorder docs.
- **Duplicate stacks (99 cap).** The recorder's bag cache is keyed by name
  (pre-existing, same as Python's dict): a second stack of the same item keeps
  the first stack's position but overwrites its quantity with the second
  stack's. The engine holds one uncapped stack per name, so engine and
  recorder agree with each other, but not with the physical bag: a swap
  that only moves the second stack is not seen, and SELECT-merging two
  stacks in the game (the sum lands in the second-selected slot, the first
  slot is dropped) can record a spurious acquire and a reorder. Needs more
  than 99 of one item; not handled, noted.
- **PC item box** (`wNumBoxItems`) is not watched; deposits and withdrawals
  keep showing as item events and a withdrawal appends to the bag end, as the
  engine already assumes.
- **Resets / soft resets** rebuild the cache without events
  (`update_all_cached_info`), so nothing spurious is recorded.
- **Disabled reorder events** pass the state through like any disabled event
  (`recalc.rs:222-227`).
