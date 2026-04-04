# Inline Event Creator

The inline event creator lets users add and edit route events directly inside the
event list, without opening separate dialogs or popovers.

## User-facing behavior

### Adding a new event

There are two ways to open the inline creator for a **new** event:

1. **Hover + click "+"** — When the mouse hovers over an event row, a small blue
   "+" button appears on the far left. Clicking it inserts a dummy event row
   immediately below the hovered event.
2. **Spacebar** — With an event selected, pressing Space opens the inline creator
   after the selected row (same as clicking "+").

The dummy row contains:

- A **searchable event-type dropdown** (type to filter) with all supported types:
  Fight Trainer, Get/Buy/Sell/Use-Drop Item, Hold Item, Wild Pkmn, Rare Candy,
  Vitamin, TM/HM Move, Save, Heal, Blackout, Evolve, Notes.
- **Dynamic config fields** that appear to the right once a type is chosen (e.g.
  Item + Qty for purchases, Location + Class + Trainer for battles, Pokemon +
  Level + Qty for wild encounters, TM selector + destination move slot for
  TM/HM moves).
- **Create** and **Discard** buttons.

Clicking **Create** inserts the event into the route after the target row.
Clicking **Discard** removes the dummy row. Clicking away leaves the dummy row
in place without affecting the route.

### Editing an existing event

**Double-click** any event row (except folders, sub-items, and auto-generated
levelup moves) to edit it inline:

1. The event is temporarily **disabled** so the route recalculates without it.
2. A dummy row appears below the disabled event, pre-populated with all of the
   event's current parameters.
3. The button reads **Confirm** instead of Create.

Clicking **Confirm** applies the changes and re-enables the event. Clicking
**Discard** restores the original event definition and re-enables it. If the
user clicks "+" or double-clicks a different event while editing, the current
edit is automatically discarded first.

While editing a trainer or battle event, the right-hand panel is forced to the
**Pre-Event State** tab so the battle summary does not steal horizontal space.

## Architecture

### Files

| File | Role |
|------|------|
| `gui_qt/components/inline_event_creator.py` | The `InlineEventCreator` widget — UI, config builders, event construction |
| `gui_qt/pkmn_components/route_list.py` | Integration into `RouteList` — "+" button, spacer row, double-click, lifecycle |
| `gui_qt/event_details.py` | Battle-summary suppression during editing |

### InlineEventCreator (`inline_event_creator.py`)

A `QFrame` overlaid on the route list's viewport. Contains a horizontal layout
with the type dropdown, a dynamic config container, and Confirm/Discard buttons,
followed by a stretch that pushes everything to the left.

#### Key data structures

- `_EVENT_TYPES` — ordered list of `(display_name, internal_key)` tuples that
  populate the type dropdown.
- `_EVENT_TYPE_TO_KEY` — maps `const.TASK_*` event-type constants to internal
  keys. Used during edit-mode to look up which dropdown index to select.
- `_KEY_TO_INDEX` — reverse map from internal key to dropdown index.

#### Config builders

Each event type has a `_cfg_*` method (e.g. `_cfg_trainer`, `_cfg_item`) that:

1. Adds labelled widgets (searchable `QComboBox` via `_combo()`, `AmountEntry`
   via `_spin()`, or `QLineEdit`) to `_config_layout`.
2. Stores widget references in `_config_refs` dict (e.g.
   `{"item": combo, "qty": spin}`) for edit-mode population.
3. Sets `_event_builder` — a callable returning an `EventDefinition`, or `None`
   if the current config is invalid.
4. Optionally sets `_create_validator` — a callable returning `bool` to
   enable/disable the Create button.

Quantity inputs use the app's `AmountEntry` widget (explicit +/- `QPushButton`
widgets) instead of `QSpinBox`, because native spinbox arrows do not reliably
receive clicks inside a viewport overlay.

#### Create vs Edit mode

The constructor accepts optional `editing_group_id` and `editing_event_def`
parameters:

- **Create mode** (defaults): `_on_create()` calls
  `controller.new_event(ev, insert_after=...)` directly.
- **Edit mode**: `_on_create()` stores the built `EventDefinition` in
  `result_event_def` and emits `event_created`. The `RouteList` reads
  `result_event_def`, restores the event's enabled state, and calls
  `controller.update_existing_event()`.

On construction in edit mode, `_init_from_event()` selects the matching type
index (triggering config build), then `_populate_config()` fills widget values
from the existing definition.

### RouteList integration (`route_list.py`)

#### "+" button

A 16x16 `QPushButton` parented to `self.viewport()`. Positioned in
`mouseMoveEvent` at the left edge of the hovered row (skipping `EventItem`
rows). A 150ms `QTimer` handles the hide delay so the cursor can travel from
the tree view to the button without it disappearing. Enter/leave events on the
button stop/start the timer via `eventFilter`.

#### Spacer row

When the inline creator is shown, a blank `QStandardItem` row is inserted into
the model immediately after the target event. The row has a `sizeHint` of
`QSize(0, 34)` and spans all columns (`setFirstColumnSpanned`). This pushes
subsequent rows down so the overlay does not occlude them.
`doItemsLayout()` is called after insertion to force the tree view to measure
the tall row immediately.

The `InlineEventCreator` widget is then positioned over the spacer row's
`visualRect` by `_reposition_inline_creator`, which is also connected to
`verticalScrollBar().valueChanged` for scroll tracking.

During `refresh()`, the spacer row is removed before the model rebuild and
re-inserted afterward (if the inline creator is still active).

#### Double-click editing

`mouseDoubleClickEvent` checks that the target is an `EventGroup` with a type
present in `_EVENT_TYPE_TO_KEY` (excludes folders, `EventItem` sub-rows, and
auto-generated levelup moves). It then calls `_start_inline_edit(group_id,
event_obj)` which:

1. Cancels any prior edit via `_restore_editing_state()`.
2. Calls `set_suppress_battle_summary(True)` on `EventDetails` to keep the
   right panel on Pre-Event State for the duration of the edit.
3. Saves `_editing_group_id` and `_editing_original_enabled`.
4. Disables the event and calls `update_existing_event` to recalculate the
   route without it.
5. Creates the `InlineEventCreator` in edit mode and inserts the spacer row.

#### Lifecycle cleanup

- **`_on_inline_created`** — In edit mode: clears battle-summary suppression,
  reads `result_event_def`, restores the original enabled state on both the
  `EventDefinition` and `EventGroup`, and calls `update_existing_event` once.
  In create mode: no-op (the creator already called `new_event`).
- **`_on_inline_discarded`** — Calls `_restore_editing_state()` which
  re-enables the event with its original definition and clears suppression.
- **`_restore_editing_state`** — Shared helper used by discard, by "+"
  click while editing, and by double-clicking a different event while editing.

### Battle-summary suppression (`event_details.py`)

`EventDetails.set_suppress_battle_summary(suppress)` sets a persistent flag
checked in `_handle_selection_inner()`. While the flag is `True`, the
auto-switch logic that normally shows the Battle Summary tab for trainer/wild
events is overridden to force the Pre-Event State tab instead.

A persistent flag (rather than one-shot) is necessary because a double-click
generates a mouse-press first, which selects the trainer event and triggers
`_handle_selection_inner` before `mouseDoubleClickEvent` fires. The flag must
survive across multiple selection callbacks during the editing session.
