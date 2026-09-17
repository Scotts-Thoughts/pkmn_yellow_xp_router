# Stat stage stepper (replace dropdown with `- [value] +`)

> **Revision 3 (2026-09-16).** The rev-2 `stepper_group` + always-visible
> `Entry` chrome looked boxy in the header and, on Mimic rows (custom-data
> dropdown + stepper both showing), pushed the `+` button off the visible
> header. Replaced with a flat, frameless look: a fixed `OPTION_STEPPER_W =
> 46.0` px cell (14 px `−` glyph, 18 px value, 14 px `+` glyph), drawn
> directly with `ui.painter()` — no background in the resting state, a
> subtle rounded hover fill behind each glyph, and a thin hover outline
> around the value. The value is plain centered text, not a boxed `Entry`;
> clicking it sets a per-id `editing` bool in egui temp memory
> (`id.with("editing")`) and swaps in a real `Entry` (overflowing to 30 px,
> centered on the value cell, since 18 px is too narrow to type into) for
> that one frame range, following the same request-focus/select-all/Enter/
> Escape pattern `AmountEntry` uses; every exit path (Enter, Escape, focus
> loss, or clicking `−`/`+` while editing) clears `editing` and surrenders
> focus. Index-based stepping, not-in-options snapping, and disabled
> (screenshot mode) semantics are unchanged from rev 2.
>
> Also fixed the Mimic overflow itself in `battle_ui.rs`: the header now
> computes `avail = hrect.width() - 6.0` and the weather/screen/stepper
> reservations once, then sizes the custom-data dropdown against what's
> left. If showing the dropdown at its normal 70 px would squeeze the move
> name below its 20 px floor, the move name is dropped entirely (Mimic
> already shows the mimicked move in the dropdown) and the dropdown shrinks
> to `avail` minus the other reservations, clamped to a 40 px minimum, so
> nothing overflows the header's clip rect. The test-move row (`mon_idx ==
> 0`) is untouched by this rule.
>
> **Audit (2026-09-16).** Revision 2. Changes from rev 1:
> - The plan proposed a from-scratch widget and missed two existing ones:
>   `widgets::stepper_group` (the `[−] content [+]` control already used for
>   the candy/vitamin steppers in this same battle summary) and
>   `widgets::AmountEntry` (an existing `− [text] +` numeric stepper built on
>   `Entry`, with Enter/Escape/select-all-on-focus). The new widget now
>   composes `stepper_group` + `Entry` so it matches the existing chrome.
> - Dropped the separate "display mode / edit mode" state machine. An
>   always-present `Entry` already reads as a click-to-type field and is what
>   `AmountEntry` does; it removes a per-id "editing" flag and a fake label.
> - Rev 1 cited "existing text-edit revert behavior elsewhere in
>   `battle_ui.rs`". There is none; the Enter/Escape prior art is
>   `Entry::show` (`widgets.rs:373-389`) and `SearchableDropdown`
>   (`widgets.rs:713-725`).
> - Added the "stored value not in options" case. `stat_stage_setup` is
>   persisted in the trainer definition (`battle_summary.rs:384-397,1169`), so
>   the widget can be handed a string that is not in the option list.
> - Specified the snapping tie-break and pulled snapping into a pure,
>   unit-tested helper instead of manual-only verification.
> - Fixed the 74 vs 76 width inconsistency with one shared constant.
> - Removed the per-button `ui.disable()` idea; `stepper_group` already takes
>   per-button enable flags.

## Goal

Replace the stat-stage-applications `ComboBox` in the battle summary move row
with a `-` / editable value / `+` stepper: click the value to type a number
and accept with Enter, or use the side buttons to step by one application of
the move.

## Current state (verified against HEAD)

- `rust/crates/xpr-app/src/battle_ui.rs:883-889` renders the control:
  ```rust
  if show_stat_now {
      let opts = stat_stage_options.clone().unwrap_or_default();
      let mut cur = stat_stage_selection.clone();
      if widgets::option_menu(&mut header_ui, theme, ui.id().with(("stage", mon_idx, move_idx, is_player)), &mut cur, &opts, Some(50.0), !screenshot) {
          bc.update_stat_stage_setup(cfg, ctrl, mon_idx, move_idx, is_player, &cur);
      }
  }
  ```
  The header row `hrect` is 26 px tall (`battle_ui.rs:750`), and
  `header_spacing_x` is 2 px (`:755`).
- `opts` / `cur` come from `xpr-calc`'s `battle_summary.rs:895-906`, which calls
  `xpr-data`'s `MoveDb::get_stat_stage_dropdown_options` (`db.rs:612-630`).
- **The value is a count of *applications* of the move**, not a raw stage
  delta. For a normal ±1/±2/±3 stat move the options are the contiguous list
  `"0".."max_applications"` (`max_applications = 6 / modifier.abs()`,
  `db.rs:594-600,626-629`). **Belly Drum** has a non-contiguous set:
  `["0","6"]` normally, `["0","2","6"]` in Gen 2 (`db.rs:620-624`). Stepping
  must move through the *options list by index*, never do `±1` arithmetic.
- **The current value may not be in the options list.** `stat_stage_setup`
  is saved into the trainer definition (`battle_summary.rs:1169`) and read
  back on load (`:384-397`); `get_stat_stage_selection` (`:1619`) returns the
  stored string verbatim, defaulting to `"0"`. Today `option_menu` just shows
  whatever string it is handed. The stepper must not panic or get stuck on
  such a value (see Behavior 3).
- `bc.update_stat_stage_setup` (`battle.rs:201-206` → `battle_summary.rs:1627`)
  stores the `&str` as-is and does a full refresh. No signature change.
- `widgets::option_menu` (`widgets.rs:435-487`) is also used by the unrelated
  "custom data" dropdown (`battle_ui.rs:871-882`, Mimic etc.). That one is
  **out of scope** and keeps using `option_menu`.
- Existing widgets to build on (all in `rust/crates/xpr-ui-kit/src/widgets.rs`):
  - `stepper_group(ui, theme, id, button_w, minus_enabled, plus_enabled,
    tooltips, content) -> i32` (`:1248-1268`). Draws `[−] content [+]` in one
    rounded frame, 26 px tall buttons (`stepper_button`, `:1270`), returns
    -1/0/+1. Used for the candy stepper (`battle_ui.rs:323`, button_w 32) and
    the vitamin steppers (`:339`, button_w 30).
  - `Entry` (`:300-393`): themed single-line `TextEdit`; its `EntryResponse`
    exposes `changed`, `has_focus`, `lost_focus`, `enter_pressed`,
    `escape_pressed`. Enter is consumed so it does not leak to other widgets.
  - `AmountEntry` (`:854-961`): `− [Entry] +` with raw `±1` integer stepping
    and min/max clamps. **Not** reusable directly because of the arithmetic
    stepping and its 50 px minimum entry width, but it is the pattern to copy
    for select-all-on-focus (`:922-938`).
- Layout: `reserved_right` budgets `50.0 + header_spacing_x` px for this
  control (`battle_ui.rs:808-810`). This must change to the stepper's width.
- Screenshot mode hides the control when the value is `"0"` (`show_stat_now`,
  `battle_ui.rs:795`) and disables it otherwise (`!screenshot` as `enabled`).
  The stepper must preserve both.

## New widget: `xpr_ui_kit::widgets::option_stepper`

Add next to `stepper_group` in `widgets.rs`. Signature mirrors `option_menu`
so the call site is a drop-in swap:

```rust
/// Width of the stat-stage stepper in the battle summary move header.
pub const OPTION_STEPPER_W: f32 = 66.0; // 2 * 18 px buttons + 30 px entry

pub fn option_stepper(
    ui: &mut Ui,
    theme: &Theme,
    id: Id,
    current: &mut String,   // in/out; normally one of `options`
    options: &[String],     // ordered ascending, e.g. ["0","1","2"] or ["0","2","6"]
    enabled: bool,
) -> bool                   // true iff `current` changed this frame
```

Implementation:

1. **Chrome.** Call `stepper_group` with `button_w = 18.0`, tooltips
   `("Remove one use of this move", "Add one use of this move")`, and the
   `Entry` as `content`. Total width is fixed at `OPTION_STEPPER_W`; the
   `Entry` gets `.width(30.0).centered().enabled(enabled)` and
   `.id(id.with("edit"))`. The 26 px `stepper_button` height matches the 26 px
   header row exactly, so no vertical clipping.

2. **Index stepping.** `idx = options.iter().position(|o| o == current)`.
   `minus_enabled = enabled && idx.map_or(false, |i| i > 0)`,
   `plus_enabled = enabled && idx.map_or(false, |i| i + 1 < options.len())`.
   On -1/+1 set `*current = options[idx ∓ 1].clone()` and return `true`. This
   is what makes one click equal one application even for Belly Drum's
   `0 → 2 → 6` (Gen 2) or `0 → 6`.

3. **Current value not in options.** When `idx` is `None`, both buttons are
   still enabled and each snaps: `-` picks the largest option numerically
   below `current`, `+` the smallest option above (falling back to
   `options.first()` / `options.last()`). If `current` is not numeric at all,
   `-`/`+` both go to `options[0]`. The entry still shows the raw string so
   the user can see what was loaded.

4. **Typed text.** The `Entry` needs a `String` that survives across frames.
   Keep it in egui temp memory: `ui.data_mut(|d| d.get_temp_mut_or(id.with("typed"), current.clone()))`.
   While the entry does **not** have focus, overwrite the typed string with
   `current` each frame so external changes (undo, gen change, `+`/`-`) show
   up. This is the same resync-when-unfocused rule the test-move field uses
   (`battle_ui.rs:815-817`). On `gained_focus`, select all, copying
   `AmountEntry` (`widgets.rs:928-935`).

5. **Commit on Enter.** When `EntryResponse.enter_pressed`, run
   `snap_to_option(&typed, options) -> Option<String>` (below). `Some(v)` with
   `v != *current` sets `*current = v` and returns `true`; `None` reverts the
   typed text to `current`. Then surrender focus
   (`ui.memory_mut(|m| m.surrender_focus(edit_id))`) so the field goes back
   to display.

6. **Revert on Escape or focus loss.** `escape_pressed` or `lost_focus`
   without Enter this frame: reset typed text to `current`, no commit. Clicking
   `-`/`+` while editing therefore discards the typed text and then steps,
   which is consistent with "Enter is the commit gesture". Do not commit on
   `changed`; the dropdown today commits only on an explicit pick, and a
   per-keystroke `full_refresh` would be expensive.

7. **Disabled.** `enabled == false` (screenshot mode) disables both buttons via
   the `stepper_group` flags and passes `.enabled(false)` to the `Entry`.
   Nothing else is needed; do not add per-button `ui.disable()` scopes.

### `snap_to_option` (pure helper, unit-tested)

```rust
/// Map free-typed text onto the closest entry of an ascending numeric option
/// list. `None` when the text is not an integer or `options` is empty.
fn snap_to_option(typed: &str, options: &[String]) -> Option<String>
```
- Exact string match (after `trim`) wins.
- Otherwise parse `typed` as `i64`; `None` on failure.
- Clamp to `[options[0], options[last]]` numerically, then pick the option
  with the smallest absolute distance. **Ties go to the lower option**
  (`"4"` against `[0,2,6]` → `"2"`, `"1"` against `[0,2,6]` → `"0"`).
- Options that fail to parse are skipped (defensive; `MoveDb` only emits
  integers today).

Tests in `widgets.rs` (`#[cfg(test)]`), run with `cargo test -p xpr-ui-kit`:
exact match, clamp below/above, nearest, tie → lower, non-numeric → `None`,
`"  3 "` trims to `"3"`, empty options → `None`.

## Call site change

`battle_ui.rs:883-889`:
```rust
if show_stat_now {
    let opts = stat_stage_options.clone().unwrap_or_default();
    let mut cur = stat_stage_selection.clone();
    if widgets::option_stepper(&mut header_ui, theme, ui.id().with(("stage", mon_idx, move_idx, is_player)), &mut cur, &opts, !screenshot) {
        bc.update_stat_stage_setup(cfg, ctrl, mon_idx, move_idx, is_player, &cur);
    }
}
```
and `battle_ui.rs:808-810`:
```rust
if show_stat_now {
    reserved_right += widgets::OPTION_STEPPER_W + header_spacing_x;
}
```
`stat_stage_options`, `stat_stage_selection`, `show_stat`, `show_stat_now`
and everything else in the function stay as computed today.

## Sizing

- `OPTION_STEPPER_W = 66.0`: two 18 px buttons plus a 30 px entry, with
  `stepper_group` using zero inner spacing. Values are at most one digit
  (largest contiguous list is `0..=6`), so 30 px is enough for the centered
  text plus `Entry`'s 4 px margins. If the chosen font makes `6` look cramped,
  raise the entry to 34 px and the constant to 70; keep the two in one place.
- The budget goes from 50 to 66 px, so the move-name column loses 16 px when a
  stat control is showing. `name_w` is already elided (`widgets::elide`,
  `battle_ui.rs:836`) and floored at 20 px, so nothing breaks, but check the
  usual window width for over-aggressive eliding of long move names such as
  "Double-Edge" next to a stepper.
- `show_custom_now` and `show_stat_now` can both be true for the same move
  when a move has both custom data and a guaranteed self-buff (see the
  memory note on `stat_stage_setup` vs `custom_move_data`). The row already
  handles that case by budgeting both; the only change is the larger number.

## Out of scope / unaffected

- `xpr-calc::battle_summary.rs` stat-stage computation, `MoveDb` option
  generation, and `update_stat_stage_setup`. Pure UI change, so no
  `xpr-golden` or recording-parity impact.
- The "custom data" `option_menu` usage (`battle_ui.rs:871-882`) stays a
  dropdown.
- `AmountEntry` and `stepper_group` are used unchanged; do not alter their
  behavior for the candy/vitamin/DV call sites.
- Python `gui`/`gui_qt` are deprecated and not touched.

## Testing / verification

- `cargo test -p xpr-ui-kit` for the `snap_to_option` cases above.
- `cargo build -p xpr-app` and run it (`cargo run -p xpr-app`). Open a battle
  summary and check, on a ±1 move (e.g. Growl / Tail Whip on the enemy,
  Swords Dance on the player) and on Belly Drum in a Gen 1 and a Gen 2 route:
  - `+`/`-` step one application per click and disable at the ends; Belly
    Drum in Gen 2 goes `0 → 2 → 6` and back.
  - Clicking the value focuses it with the text selected; Enter commits a
    valid number; `9` on a `0..=3` list snaps to `3`; `4` on Belly Drum Gen 2
    snaps to `2`; `abc` reverts; Escape and clicking away revert.
  - The stepped/committed value survives an undo/redo and a route reload
    (it is stored in the trainer definition).
  - Screenshot mode hides the control at `"0"` and shows it disabled otherwise.
  - No clipping against the weather/screen checkboxes or the custom-data
    dropdown when all are present; the candy/vitamin steppers in the controls
    bar look unchanged.
