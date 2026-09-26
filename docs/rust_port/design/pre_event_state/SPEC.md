# Pre-Event State pane — implementation spec

Status: design approved 2026-09-18, implemented 2026-09-18 (`XPR_SMOKE_ACTION=prestate`
screenshots it; see KNOWN_ISSUES.md). Rust/egui only (the Python app is deprecated).

Reference mockups in this folder (open the `.html` in any browser; 1 CSS px =
1 egui logical px at 100 % scale; the `.png` files are 1:1 renders):

| file | shows |
|---|---|
| `main_emerald.html` / `.png` | Emerald, Nosepass Lv 5, before **Rival Brendan 1 Mudkip**; trainer-fight editor; empty bag; notes collapsed |
| `gen1_yellow.html` / `.png` | Yellow, Pinsir Lv 21, before a **Reorder Bag** event; Gen 1 stat layout; badge-boosted stat; warning banner; full bag; notes expanded |

Live design canvas: https://claude.ai/artifact/UCZPdgwYNmwLJkpTWZT58w

The numbers on the Gen 1 board are illustrative; every other value is real
route data.

---

## 1. Scope

Replace everything drawn in the right pane while the **Pre-Event State** tab
is showing:

1. The tab strip and the "Switch tabs automatically" checkbox
   (`event_details.rs` `EventDetails::ui`).
2. `state_views::state_viewer` — the Pokémon block, the stat-EXP/EV block and
   the inventory block. This module is only used by the pre-event tab, so it
   can be rewritten freely.
3. The warning labels drawn above the editor.
4. The editor wrapper (`editors.rs` `EventEditors::ui`) plus a restyle of the
   `TrainerFightEditor` mon cards and the `BagReorderEditor` rows.
5. The `NotesEditor` footer.

**Not in scope / must not change**

- The Battle Summary tab and `battle_ui.rs`.
- Behaviour: what auto-switch does, when editors save (`EditorOutput`),
  `last_pkmn` retention when the state is `None`, record-mode disabling
  (`EditorCtx::enabled`), the notes config keys and their values
  (`get_notes_visibility_mode` → `when_space_allows | always | never`,
  `get_notes_collapsed`, `do_auto_switch`).
- The inner controls of the other editors (vitamin, rare candy, learn move,
  wild, inventory, location, evolution). They keep their `egui::Grid`s and
  simply render inside the new editor card body.

---

## 2. Files to touch

| file | change |
|---|---|
| `rust/crates/xpr-ui-kit/src/theme.rs` | add the derived colours in §3.1 |
| `rust/crates/xpr-ui-kit/src/widgets.rs` | restyle `tab_bar` (only caller is `event_details.rs`); add `card`, `card_title`, `chip_outlined`, `pill`, `progress_bar`, `warning_banner`, `caption` helpers (§5) |
| `rust/crates/xpr-app/src/state_views.rs` | rewrite: `identity_header`, `stats_card`, `moves_card`, `bag_card`, new `state_viewer` |
| `rust/crates/xpr-app/src/event_details.rs` | tab strip row with the checkbox; `pre_state_tab` composition; pass `&mut Assets` and the "Before" label into the state viewer; notes footer height |
| `rust/crates/xpr-app/src/editors.rs` | `EventEditors::ui` draws the editor card + title strip; restyle `TrainerFightEditor::ui` cards and `BagReorderEditor::ui` rows; `NotesEditor::ui` footer; shorter display strings for `NOTES_OPTIONS` (values unchanged) |

Add a `fmt_thousands(i64) -> String` helper (xpr-ui-kit or xpr-core) — exp,
money and stat-EXP totals use thousands separators.

---

## 3. Tokens

Everything is derived from `Theme` so user-configured colours keep working.
Defaults are for the stock theme (bg `#1e1e1e`, text `#d4d4d4`).

### 3.1 Colours

| token | derivation | default | used for |
|---|---|---|---|
| pane bg | `theme.bg` | `#1e1e1e` | pane background |
| card bg | `lighten(bg, 0.04)` (= `theme.bg_input`) | `#272727` | all cards |
| card border | `lighten(bg, 0.10)` (= `theme.hover_bg`) | `#343434` | 1 px card outline |
| row divider | `lighten(bg, 0.085)` | `#313131` | hairlines between rows inside cards |
| pane divider | `lighten(bg, 0.095)` | `#333333` | under the tab strip, above the notes footer, under the editor title strip |
| well bg | `theme.bg` | `#1e1e1e` | text inputs, text area, enemy mon cards, drag rows, icon tile |
| strip bg | `theme.section_bg()` | `#2c2c2c` | editor card title strip |
| control border | `theme.border` | `#505050` | inputs, selects, arrow buttons |
| text | `theme.text` | `#d4d4d4` | body |
| text strong | `lighten(text, 0.7)` | `#f2f2f2` | Pokémon name, stat values, money, move names, card titles |
| text muted | `theme.secondary` | `#999999` | meta lines, column heads, hints, empty-slot dashes |
| text on bright | `theme.contrast` | `#e0e0e0` | bag item names |
| section title | `theme.header` | `#e8a850` | STATS / MOVES / BAG titles, "Before" event name, event-type chip text, badge-boost dot |
| chip bg / border | `tinted_bg("Header", 0.14)` / `tinted_bg("Header", 0.28)` | `#362e24` / `#54432b` | event-type chip |
| level pill text | `theme.primary` | `#7cb8e0` | "Lv N" pill, enemy level |
| level pill bg / border | `tinted_bg("Primary", 0.12)` / `tinted_bg("Primary", 0.25)` | | |
| exp bar fill / track | `theme.primary` / `theme.subtle_border` | `#7cb8e0` / `#393939` | |
| accent | `theme.accent` | `#0078d4` | active-tab underline, checkbox tick |
| warning text | `theme.warning` | `#e8b730` | banner text and icon |
| warning bg / border | `tinted_bg("Warning", 0.12)` / `tinted_bg("Warning", 0.32)` | | banner |
| disabled | `theme.disabled_text` | `#616161` | disabled arrow buttons / inputs |
| speed comparison | unchanged (`success` / `warning` / `failure` / `contrast`) | | enemy `Spe` in trainer cards |

### 3.2 Type scale

`theme.font(points)` — px = points × 4⁄3. Body stays the app's 9 pt so the
pane matches the route list.

| role | px | `theme.font(..)` | weight / colour |
|---|---|---|---|
| Pokémon name | 22 | `font_bold(16.5)` | strong |
| money value | 18 | `font_bold(13.5)` | strong |
| stat values ("Now" column) | 14 | `font_bold(10.5)` | strong |
| body: stat names, move names, bag item names, tab labels, editor label, hints | 12 | `body()` / `body_bold()` | text / strong |
| muted meta: ability · nature · item, exp line, control labels, bag qty, footer hints | 12 | `body()` | muted |
| level pill, enemy "Lv N" | 12 | `body_bold()` | primary |
| captions: column heads, card footers, slot numbers, "1 of 4", "BEFORE", section titles, event chip | 11 | `font(8.25)` (`font_bold` for section titles / chip) | muted (header colour for titles/chip) |

Uppercase captions ("BEFORE", section titles, chip) get
`TextFormat::extra_letter_spacing = 0.9` px.

### 3.3 Geometry

- Pane inner margin: 16 px left/right; 16 px between the tab strip and the
  header card; 12 px between cards; 16 px above the notes footer.
- Card: radius 8, 1 px card border, padding 12 top / 14 sides / 10 bottom
  (header card: 14 / 16).
- Card title row: 22 px tall.
- Stats rows 27 px; move rows 34 px; bag rows 25 px; enemy-card stat lines
  and drag rows 30 px.
- Inputs / selects 26–28 px tall, radius 4, 1 px control border, 6–8 px
  horizontal padding; arrow buttons 26 × 22.
- Pills radius 999; chips radius 4; banner radius 6.

---

## 4. Layout

```
┌ pane (width W, e.g. 960) ──────────────────────────────────────────────────┐
│ [Pre-Event State] [Battle Summary]                 [x] Auto-switch tabs   │ 40 px, bottom hairline
│                                                                            │
│ ┌ identity header ───────────────────────────────────────────────────────┐ │
│ │ [icon] Nosepass (Lv 5)                                   BEFORE        │ │
│ │        Sturdy · Adamant · Held item: none      Rival Brendan 1 Mudkip  │ │
│ │        Exp 196 ━━━━━━━━━━━━━━━━━━━━━━━░░░░░░ 20 to Lv 6     Route 103  │ │
│ └────────────────────────────────────────────────────────────────────────┘ │
│ ┌ STATS (43 %) ────────────┐ ┌ MOVES (23 %) ─┐ ┌ BAG (31 %) ───────────┐  │
│ │ Stat  Now  from EVs  EVs │ │ 1 Tackle      │ │ Money          $3,000 │  │
│ │ HP     19    +0     0/0  │ │ 2 —           │ │ 01  5× Poké Ball      │  │
│ │ …                        │ │ 3 —           │ │ …  / empty state      │  │
│ │ footer: legend · badges  │ │ 4 —           │ │                       │  │
│ └──────────────────────────┘ └───────────────┘ └───────────────────────┘  │
│ ⚠ Warning: …                              (only when the event has any)   │
│ ┌ editor card ───────────────────────────────────────────────────────────┐ │
│ │ [FIGHT TRAINER] Rival Brendan 1 Mudkip  Route 103          Exp/sec 9   │ │ title strip
│ │ Pay Day amount [0]                                                     │ │
│ │ ┌ enemy card ┐ ┌ enemy card ┐ ┌ enemy card ┐                           │ │
│ └────────────────────────────────────────────────────────────────────────┘ │
│                              (remaining space)                             │
│ ▸ Notes   No notes for this event                                          │ pinned footer
└────────────────────────────────────────────────────────────────────────────┘
```

Column widths for the cards row: content width `C = W − 32`; Stats = 43 %,
Moves = 23 %, Bag = 31 % of `C` (gaps 12 px; at W = 960 that is
400 / 216 / 288). Minimums 360 / 180 / 240; below `C ≈ 800` put the Bag card
on its own row under Stats + Moves (Stats 65 %, Moves 35 %).

The whole tab body scrolls vertically (keep the existing `ScrollArea`); the
notes footer stays pinned below it as it is today.

---

## 5. Components

### 5.1 Tab strip

- 40 px row, full pane width, pane-divider hairline along the bottom.
- Tabs: text only, 12 px horizontal padding, no boxes. Active: strong text,
  bold, 2 px accent underline flush with the strip's bottom edge. Inactive:
  muted text; hover → text colour. Click behaviour unchanged.
- Right-aligned: a checkbox (`widgets::checkbox_label`, tick in accent) with
  the label **Auto-switch tabs** (muted). It replaces the "Switch tabs
  automatically" row that currently sits above the notes; wiring stays
  `self.auto_switch` / `cfg.set_auto_switch`.

### 5.2 Identity header card

Full content width; three horizontal parts, vertically centred.

1. **Icon tile** 68 × 68, radius 12, well bg, card border. Draw
   `Assets::pkmn_icon(ctx, &pkmn.name)` fitted to 60 px
   (`assets::draw_fit`). No icon → leave the tile empty.
2. **Main column** (grows):
   - Row 1: name (22 px bold strong) + **level pill** "Lv N" (12 px bold
     primary, pill bg/border, padding 2 × 8, radius 999), 10 px gap.
   - Row 2 (gens ≥ 2 only; omit entirely in Gen 1): `ability` · `nature` ·
     `Held item: <name|none>` — 12 px, separators muted, ability and nature in
     text colour. Gen 2 has no ability/nature: show only the held item.
   - Row 3: exp line — "Exp **196**" (muted, value bold text) · progress bar
     (6 px tall, radius 3, track/fill colours, grows) · "**20** to Lv 6".
     Fill fraction = `1 − percent_xp_to_next_level / 100` clamped to 0..1.
     At max level (`xp_to_next_level <= 0`) draw the bar full and the right
     text "Max level".
3. **"Before" block** (right-aligned, does not shrink): caption **BEFORE**
   (11 px uppercase muted), then the event label (12 px bold, header colour),
   then the location (12 px muted).
   - Item/group selection: label = the group's `get_label(gen)` with a leading
     `Trainer: ` / `Multi: ` stripped and the trailing ` (Location)` removed
     for trainer fights (use `trainer.name` / `trainer.location` directly);
     location line = trainer location, else the enclosing folder's `name`.
   - Folder selection: label = folder name, location = its parent folder's
     name (none for a top-level folder).
   - No selection (route start): label "Route start", no location line.

Data: `RouteState.solo_pkmn` (`name`, `cur_level`, `cur_xp`,
`xp_to_next_level`, `percent_xp_to_next_level`, `ability`, `nature`,
`held_item`). Keep the current `last_pkmn` fallback when `state` is `None`.

### 5.3 Stats card

Title row: **STATS** (section title) left; column heads right-aligned over
their columns: `Now` (56 px), `from EVs` (78 px), `EVs real / total`
(108 px). Gen 1/2 wording: `from StatExp` (86 px), `StatExp real / total`
(118 px). The stat-name column takes the rest.

Rows (27 px, row-divider hairline on top): one per stat —
`HP, Attack, Defense, Sp. Atk, Sp. Def, Speed`; Gen 1: `HP, Attack, Defense,
Special, Speed`.

| column | text | source |
|---|---|---|
| name | 12 px text | — |
| Now | 14 px bold strong, right-aligned | `solo_pkmn.cur_stats` via the existing badge-boost logic in `pkmn_viewer` (including the Gen 2 Sp. Def rule) |
| from EVs | `+N` 12 px muted | `solo_pkmn.get_net_gain_from_stat_xp(&badges)` |
| EVs real / total | `R / T` 12 px muted, thousands separators | `realized_stat_xp` / `unrealized_stat_xp` |

Badge boost: instead of the `*` prefix, draw a 6 px header-colour dot 5 px to
the left of the value.

Footer row (hairline on top, 11 px muted): left = `● Badge boost applied`
when any stat is boosted, else `No EVs earned yet` when every total is 0,
else nothing; right = `badges.to_short_string()` (`Badges: Boulder, …`) or
`Badges: none` when `num_badges() == 0`.

When `state` is `None`: values from `last_pkmn`, EV columns show `—`.

### 5.4 Moves card

Title row: **MOVES** left, `N of 4` caption right (N = non-empty slots).
Four rows of 34 px: slot number (11 px muted, 14 px column) + move name
(12 px bold strong); empty slot = `—` in muted. Source: `solo_pkmn.move_list`
padded to 4.

PP (added 2026-09-25; see `../pp_tracking/PLAN.md` §5.2). When an event is
selected, each known move's row also shows PP from the PP ledger:

- **Numbers.** `cur / max` right-aligned, 12 px bold, e.g. `12 / 15`.
  Negative values use a real minus sign (`−3 / 15`). The move name is
  elided to the space left.
- **Colour.**
  - `failure` when `cur <= 0`;
  - `warning` when the selected fight (or matchup) spends more of the
    move than `cur`;
  - otherwise `secondary`.
- **PP Ups.** Up to three 4 px `secondary` dots left of the numbers.
- **Hover tooltip.** `Move: cur / max PP (n PP Ups)`, `This event uses N`
  for a fight, then the slot's history since it was last full: the
  refill or learn line, then `−3  Brock 1: Onix (Water Gun, 3 hits)`-style
  lines. It shows at most 14 lines, keeping the first.
- **Right-click menu.** `Use <item> on <move>` for each single-target PP
  item in the bag before the event (`Use <item>` for an Elixir). Picking
  one inserts that use before the selected event. Left-click still offers
  the tutor dialog.

With no event selected (route start), the rows show no PP.

PP lines are added to the §5.6 warning banners:

- `Surf is at −3 PP before this event: heal first`
- `This fight needs Surf ×6, but it has 2 PP left` (for a single matchup:
  `This matchup needs …`)
- the ledger's notes, e.g. `Surf was already full; the game would refuse
  this Ether`

### 5.5 Bag card

Title row: **BAG** left, `N of 20 slots` caption right (N = item count,
capped at 20; 20 is the current `max_render`).

Money row (34 px, hairline on top): `Money` muted left, `$3,000` 18 px bold
strong right (`inventory.cur_money`, thousands separators, `$` prefix).

Item rows (25 px, hairline on top), one per item, in bag order — numbering
must stay 1-based to match Reorder Bag event text:

- slot `01` (11 px muted, 18 px column)
- quantity `4×` (12 px muted, right-aligned in a 26 px column)
- name (12 px `contrast`)

Overflow: when `cur_items.len() > 20`, rows 1–19 then a final row
`20+ More items…` (same rule as today's `# 20+: More items...`).

Empty bag / no state: a centred block with a 26 px bag outline icon
(stroke `lighten(bg, 0.3)`) and `Bag is empty before this event` (12 px
muted). The card keeps the height of its row (all three cards stretch to the
tallest).

### 5.6 Warning banner

One per message in `selected_warnings(ctrl)`, stacked with 6 px gaps, drawn
between the cards row and the editor card. Full content width, padding
8 × 12, radius 6, warning bg + 1 px warning border, 10 px gap between a 16 px
stroked triangle icon and the text: **Warning:** (bold) + message, both in
`theme.warning`, 12 px.

### 5.7 Editor card

Only when `current_event_type` is set (i.e. not for notes-only events).

**Title strip** (strip bg, padding 9 × 14, pane-divider hairline below):

- chip: the event type string upper-cased (`FIGHT TRAINER`, `REORDER BAG`,
  `ACQUIRE ITEM`, …) — 11 px bold header colour, chip bg/border, padding
  2 × 8, radius 4;
- label (12 px bold strong) — trainer name for fights, otherwise the event's
  `get_label(gen)`;
- location (12 px muted) — as in §5.2;
- right-aligned readouts: trainer fights show `Exp/sec` + value (the string
  the editor already computes). Nothing for other types.

**Body**: padding 12 / 14 / 14, item spacing 12 px (8 px for the
non-trainer grids). `EventEditors::ui` draws the strip and then dispatches to
the editor exactly as now. Disabled state (record mode) keeps today's
behaviour; disabled inputs use `disabled_text`.

#### Trainer fight body

1. Control row: `Pay Day amount` (12 px muted label) + 64 px text entry
   (well bg, control border, radius 4, 28 px tall). The exp/sec label moves
   to the title strip.
2. Enemy cards, 300 px wide, 12 px gaps, wrapping (keep the existing 3-per-row
   logic, but let the count fall to 2 when `available_width < 3·300 + 24`):
   - well bg, card border, radius 8, padding 10 × 12, 8 px item spacing;
   - header: `Assets::pkmn_icon` at 38 px + name (12 px bold strong) + `Lv N`
     (12 px bold primary) on one line, then `ability · nature · Exp N` (12 px
     muted; nature omitted when Hardy, exp omitted when 0, whole line omitted
     in Gen 1/2 if empty);
   - stat grid 3 × 2 (`HP Atk Def / SpA SpD Spe`): 12 px, muted label left,
     bold `contrast` value right, hairline under the first row; `Spe` value
     coloured with the existing speed-comparison colour;
   - moves line: `Moves` muted + names joined with ` · `;
   - controls row (hairline on top, 6 px padding): `Order` select (55 px) and
     `Exp split` select (55 px), labels muted; below it the existing
     `Thief / Covet held item` checkbox when the mon holds an item.

#### Reorder bag body

Hint line (12 px muted, unchanged text), then one row per item, 4 px apart:
30 px tall, well bg, card border, radius 6, padding 0 × 8, 10 px gaps:
drag handle (three 14 px lines, muted; keeps `dnd_drag_source`), `1.` (11 px
muted, 18 px column), `4×` (12 px muted, 26 px right-aligned), name (grows,
`contrast`), then the ▲ ▼ buttons as 26 × 22 outlined buttons with 12 px
chevrons (disabled at the ends → disabled colour, no hover). Drag insertion
line stays the 2 px accent line. The "stale" warning line under the list is
unchanged.

### 5.8 Notes footer

Pinned to the bottom of the pane, pane-divider hairline on top, padding
10 / 16 / 12.

- Collapsed: chevron (12 px `disclosure_triangle`) + **Notes** (12 px bold) as
  one click target; then a 12 px muted hint: `No notes for this event`, or the
  first line of the notes elided to the available width when there are notes.
  Row height 40 px.
- Expanded: same header with the chevron pointing down; right-aligned
  `In battle summary` (12 px muted) + option menu with the display strings
  `Show when space allows` / `Show at all times` / `Never show`, mapping to the
  existing modes `when_space_allows` / `always` / `never` (config values
  unchanged). Below: the text area, full width, 100 px tall, well bg, control
  border, radius 6, 8 × 10 padding. Delayed-save behaviour unchanged.

`notes_h_estimate` in `EventDetails::ui` becomes 40 (collapsed) / 168
(expanded).

---

## 6. Widget helpers to add (xpr-ui-kit)

```rust
/// Card frame: card bg, 1 px card border, radius 8, padding (12,14,10,14) unless overridden.
pub fn card<R>(ui, theme, padding: Option<Margin>, add: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>
/// 22 px title row: uppercase section title (header colour) left, optional caption right.
pub fn card_title(ui, theme, title: &str, right: Option<&str>)
/// Small uppercase caption (11 px, letter-spaced); `color` defaults to muted.
pub fn caption(ui, theme, text: &str, color: Option<Color32>)
/// Rounded pill: "Lv 5" etc. (fg/bg/border supplied by the caller).
pub fn pill(ui, theme, text: &str, fg, bg, border) -> Response
/// Outlined chip with uppercase caption text (event-type chip).
pub fn chip_outlined(ui, theme, text: &str, fg, bg, border) -> Response
/// 6 px progress bar of the given width; fraction 0..1.
pub fn progress_bar(ui, theme, width: f32, fraction: f32, fill, track)
/// Warning banner (icon + bold prefix + message).
pub fn warning_banner(ui, theme, message: &str)
/// Right-aligned text in a fixed-width column (used by every table-like row).
pub fn col_text(ui, theme, width: f32, text: &str, font: FontId, color: Color32, align: Align)
```

`rounded_section` stays for the other editors.

---

## 7. Data sources (quick map)

| UI | source |
|---|---|
| name, level, exp, to-next, % | `state.solo_pkmn.{name, cur_level, cur_xp, xp_to_next_level, percent_xp_to_next_level}` |
| ability / nature / held item | `solo_pkmn.{ability, nature.display_name(), held_item}` (`gen.get_generation()` gates) |
| stats + boost markers | `solo_pkmn.get_pkmn_obj(&badges, None)` → `cur_stats`, `badges.is_*_boosted()` (reuse the block in `pkmn_viewer`) |
| from EVs | `solo_pkmn.get_net_gain_from_stat_xp(&state.badges)` |
| EVs realized / total | `solo_pkmn.realized_stat_xp` / `solo_pkmn.unrealized_stat_xp` |
| badges footer | `state.badges.to_short_string()`, `num_badges()` |
| moves | `solo_pkmn.move_list` |
| money, items | `state.inventory.cur_money`, `cur_items[i].{num, base_item.name}` |
| warnings | `EventDetails::selected_warnings(ctrl)` |
| "Before" label / location | selected node via `ctrl.get_single_selected_event_id(true)`; `router.group(id).name` / `.event_definition.get_first_trainer_obj(gen)`; `router.folder(parent).name` |
| species icon | `Assets::pkmn_icon(ctx, species_name)`; `Assets` must be threaded into `pre_state_tab` (it is already available in `EventDetails::ui`) |
| exp/sec | `TrainerFightEditor.exp_per_sec` (already computed on load; move the label text out) |

---

## 8. Acceptance checklist

- [ ] An Emerald route (the screenshot's `e-nosepass-2-` route, or any
      Gen 3+ route) selected on a trainer fight matches `main_emerald.png` in
      structure (header, three cards, editor card, pinned notes footer).
- [ ] `tests/test_data/yellow-pinsir-lv10brock.json` matches
      `gen1_yellow.png`: 5-row stats table with
      `Special`, `StatExp` column heads, no ability/nature/item line, dot on a
      badge-boosted stat, `Badges: Boulder` footer.
- [ ] Gen 2: held item line shown, no ability/nature; Sp. Def boost rule
      unchanged.
- [ ] Empty bag, 1–20 items, and >20 items all render (overflow row).
- [ ] `state == None` (route start / errors) keeps showing the last mon and
      draws `—` in the EV columns and an empty bag.
- [ ] Warnings from a Reorder Bag event render as banners above the editor.
- [ ] Every editor type opens inside the editor card with the correct chip
      text; notes-only events show no editor card.
- [ ] Auto-switch checkbox in the tab strip round-trips
      `cfg.do_auto_switch()`.
- [ ] Notes collapsed/expanded state and visibility mode persist through the
      unchanged config keys; the shorter menu strings map to the same values.
- [ ] Custom background/text/header/primary colours in config recolour the
      pane consistently (no hard-coded hex except via `Theme`).
- [ ] Pane widths 700–1200 px: cards row reflows per §4, nothing clips
      horizontally; vertical overflow scrolls.
- [ ] `cargo clippy` clean; `docs/rust_port/recording/run_quickstart.py`
      still passes.
