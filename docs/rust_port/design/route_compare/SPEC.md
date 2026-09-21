# Route Compare — implementation plan and UI spec

Status: design approved 2026-09-20, implemented 2026-09-20. Rust/egui only
(the Python app is deprecated and gets none of this).
`XPR_SMOKE_ACTION=compare` screenshots it; see §9.2.

Differences between this spec and the code as built are listed in §12.

Reference mockup in this folder: `mockup.html` (open in any browser; 1 CSS px =
1 egui logical px at 100 % scale; the three tabs switch, and on Checkpoints the
**Leader Winona** row expands). Every number in it is real: it is the output of
the algorithm in §5 run on two Emerald Houndoom routes from the saved-routes
folder — **A** = `e-houndoom-1-13454.json`, **B** = `e-ike-houndoom.json`. Use
that pair as the primary manual test case.

Live design canvas: https://claude.ai/artifact/CfBXs4XoqoENN4fLBQ1dzN

---

## 1. What this feature is

Pick two route files and see how the two runs differ: the Pokémon itself
(DVs/IVs, nature, ability, Hidden Power), what each run did (trainers, wild
Pokémon, candies, vitamins, moves, items, money, heals, blackouts), where each
run stood at the fights both have in common (level, stats, EVs, money, recorded
time), and an aligned event-by-event diff.

Typical use: "I ran Houndoom in Emerald; so did another player; they sent me
their `.json`. Where did we play differently?"

### 1.1 The three views

| tab | question it answers |
|---|---|
| **Overview** | How do the two runs differ in total? (the summary; default tab) |
| **Checkpoints** | At each fight both runs share, who was ahead and by how much? |
| **Event diff** | Exactly which events differ, and where in the run? |

### 1.2 Out of scope

- Editing either route from this page, merging routes, or copying events
  across.
- Comparing more than two routes.
- Battle-level comparison (damage ranges, kill chances per fight).
- Any change to the route file format. Nothing new is saved into routes.

---

## 2. Decisions (read these first)

| # | decision | why |
|---|---|---|
| D1 | All comparison logic lives in a new pure module **`xpr-engine/src/compare.rs`** with no egui types. The UI only renders its output. | Testable headlessly (like `xpr-engine/tests/route_processing.rs`); reusable by a future CLI/export. |
| D2 | The screen is a new top-level **`Page::Compare`**, not a dialog or secondary viewport. | It needs the full window, and must be reachable from the landing page with no route open. The view is a self-contained struct (§7.1) so it can be re-hosted in a viewport later at no cost. |
| D3 | Both routes are loaded into **their own fresh `Router`s** on a background thread; the editor's `MainController` is never touched. | One shared `Arc<Registry>` safely serves several routers, even of different games (`xpr-golden verify` already does this under rayon). The editor's undo stack, selection and unsaved edits stay intact. |
| D4 | Events are matched **structurally**, never by id. **Trainer fights are the anchors**; everything between two shared fights is compared as an unordered set. | Route files contain no event ids (`NodeId` is minted per load). Trainer fights are deterministic and near-unique; wild encounters and pickups are incidental and order-insensitive. |
| D5 | **Disabled events are ignored everywhere** (an event is disabled if it or any ancestor folder is, i.e. `Router::is_enabled(id) == false`). Their count is shown once, on the identity card. | A disabled event does not happen in the run. |
| D6 | Differences are **neutral**. Blue means "Route A", amber means "Route B", bold means "differs". **Green/red are used for recorded-time differences only.** | More trainers or a different nature is not "better". Less time is. |
| D7 | Deltas are always **B − A**, and the column header says so. | One rule, stated on screen, no guessing. |
| D8 | Different games or species are **allowed with a warning banner**, never blocked. | Red vs Blue, Ruby vs Emerald, FireRed vs LeafGreen are legitimate comparisons. |
| D9 | If the editor's open route is used as A, compare its **in-memory state** (`Router::serialize()`), not the file on disk. | Unsaved edits must be reflected. |

---

## 3. Visual tokens

Everything derives from `Theme` so user-configured colours keep working. No new
hard-coded hex. Defaults shown are for the stock theme.

### 3.1 Colours

Reuses every token of `pre_event_state/SPEC.md` §3.1 (card bg, card border, row
divider, pane divider, well bg, strip bg, control border, text, text strong,
text muted, disabled, warning text/bg/border). Additions and **overrides that
apply on this page only**:

| token | derivation | default | used for |
|---|---|---|---|
| **route A** | `theme.primary` | `#7cb8e0` | "A" badge, A column heads, A bars, A-only marker |
| route A bg / border | `tinted_bg("Primary", 0.12)` / `(…, 0.25)` (= `pill_bg()` / `pill_border()`) | `#293035` / `#36454f` | "A" badge |
| **route B** | `theme.header` | `#e8a850` | "B" badge, B column heads, B bars, B-only marker |
| route B bg / border | `tinted_bg("Header", 0.14)` / `(…, 0.28)` (= `chip_bg()` / `chip_border()`) | `#362e24` / `#54432b` | "B" badge |
| faster | `theme.success` | `#4ec97a` | time delta where B is faster than A |
| slower | `theme.failure` | `#e05555` | time delta where B is slower than A |
| bar track | `theme.subtle_border` | `#393939` | paired bars, stacked bars |
| exp: trainers / wild / candy | `lighten(bg, 0.42)` / `lighten(bg, 0.25)` / `consts` candy header `#61520f` lightened 0.35 | `#8a8a8a` / `#5f5f5f` / `#c9a227` | "Where the exp came from" stacked bar |

**Overrides — important.** Because amber now means "Route B" and blue means
"Route A", three things that are tinted elsewhere in the app are **neutral on
this page**:

1. **Card titles** use *text strong*, not `theme.header`. Add
   `widgets::card_title_colored(ui, theme, title, right, color)` and have
   `card_title` call it with `theme.header`.
2. The **level pill** is drawn with fg *text strong*, bg *well bg*, border
   *control border* (call `widgets::pill` with those; do not use `pill_bg`).
3. The **game chip** is drawn with fg *text muted*, bg *well bg*, border *card
   border* (call `widgets::chip_outlined` with those).

Nature modifiers (`+SpA −SpD`) are muted text, not green/red (D6).

### 3.2 Type scale

Same as `pre_event_state/SPEC.md` §3.2. In short: 22 px bold species name;
18 px bold final time and final-money values; 14 px bold for a number that
**differs**; 12 px body; 11 px captions and column heads (uppercase captions
get 0.9 px letter spacing). All numerals are right-aligned in fixed-width
columns via `widgets::col_text`.

### 3.3 How a difference is shown (applies to every table)

| situation | A cell and B cell | Δ cell (`B − A`) |
|---|---|---|
| values equal | both **muted**, 12 px | `—` muted |
| values differ | both **text strong, bold, 14 px** | signed number, **text strong bold** (`+49`, `−22`; U+2212 minus) |
| non-numeric differ (nature, ability, HP type) | both *text* colour | the word `differs`, text strong bold |
| one side missing | `—` muted on that side | blank |

Money is `$` + thousands separators (`widgets::fmt_thousands`); negative money
is `−$96,015`. Time deltas: see §6.3.

The **"Highlight differences"** checkbox (right end of the tab strip, default
on) toggles the bold/14 px treatment; off = every value in plain *text* colour.
It does not hide rows.

### 3.4 Geometry

- Page inner margin 16 px; 12 px between cards and between rows of cards.
- Cards: radius 8, 1 px card border, padding 12 / 14 / 10; title row 22 px.
- Table rows 27 px with a row-divider hairline on top; group-heading rows
  30 px with a pane-divider hairline on top; column heads 11 px muted above the
  first row.
- "A"/"B" **route badge**: 20 × 20, radius 4, 11 px bold letter, route bg +
  1 px route border. Add `widgets::route_badge(ui, theme, side)`.
- **Paired bars**: two 4 px bars (A above B, 2 px apart) in a 120 px column;
  each bar's fill = `value / max(a, b)`; track colour behind. Add
  `widgets::paired_bars(ui, theme, width, a_frac, b_frac)`.

---

## 4. Entry points and page shell

### 4.1 Entry points

| where | control | behaviour |
|---|---|---|
| **File** menu, directly under *Load Route* | `Compare Routes…` — new action id `compare_routes`, default **Ctrl+Shift+C** (unused today) | Open `Page::Compare`. If a route is open in the editor, A = that route (in-memory, D9) and the B picker opens immediately. Otherwise both slots are empty. |
| **Landing page** button row, after *Load Selected Route* | `Compare Routes` | Open `Page::Compare`. If a route is selected in the table, it becomes A. |

Register the shortcut in `DEFAULT_SHORTCUTS`, `SHORTCUT_LABELS` and
`SHORTCUT_CATEGORIES` (`xpr-core/src/config.rs`) and add the `fire(...)` branch
in `XprApp::handle_shortcuts`. The menu item is always enabled.

`XprApp` remembers `compare_return: Page` (the page Compare was opened from).
**Back** and **Esc** return to it. The `CompareView` is kept alive while the app
runs, so returning to Compare shows the last comparison instantly.

### 4.2 Shell layout (window 1280 × 800 shown; minimum supported width 1000)

```
┌ menu bar (unchanged) ───────────────────────────────────────────────────────────────────────┐
├ header strip · 56 px · strip bg · pane-divider hairline below ───────────────────────────────┤
│ [‹ Back]  COMPARE ROUTES  [A│e-houndoom-1-13454      ▾] [⇄] [B│e-ike-houndoom        ▾]        [Copy summary] [Export screenshot] │
│                               Emerald · Houndoom · current route     Emerald · Houndoom · saved 2026-09-14                          │
├ tab strip · 40 px ───────────────────────────────────────────────────────────────────────────┤
│  Overview   Checkpoints   Event diff                              [x] Highlight differences  │
├ body · scrolls vertically · 16 px margin ────────────────────────────────────────────────────┤
│  (banners, then the active tab's content)                                                    │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

- **Back**: `StyledButton`, 28 px tall.
- **COMPARE ROUTES**: `widgets::caption`, muted, bold.
- **Route picker button** (×2): 330 × 36, well bg, control border, radius 4.
  Contents: route badge · two lines (route name 12 px bold strong, elided; sub
  line 11 px muted `Game · Species · <origin>`) · `▾`. Origin is one of
  `current route`, `saved YYYY-MM-DD` (file mtime), or `external file`.
  Empty slot: muted text `Choose route A…` / `Choose route B…`.
- **⇄ Swap**: 28 × 28 button; swaps the two loaded routes and re-runs only
  `compare()` (no reload). Disabled until both are loaded.
- **Copy summary**, **Export screenshot**: right-aligned; disabled until a
  comparison exists (§8).
- **Tab strip**: `widgets::tab_bar(ui, theme, &["Overview", "Checkpoints",
  "Event diff"], &mut tab, trailing)`; the trailing closure draws the
  *Highlight differences* checkbox (`widgets::checkbox_label`).

### 4.3 Route picker popup

Clicking a picker button opens a popup anchored under it (`egui::Popup`, 420 px
wide, max 420 px tall, card bg, card border, radius 8, padding 10):

1. Search `Entry` (full width, focused on open, hint `Search routes…`).
2. Game filter `option_menu` (`All games` + `registry.get_gen_names()`).
   Default: the **other** slot's game if that slot is filled, else `All games`.
3. Scrolling list from the cached **`RouteIndex`** (`route_index.rs`; never
   parse route files to build this list). Rows 30 px: route name (12 px text) ·
   right-aligned `Game · Species` (11 px muted). Sorted by mtime, newest first.
   Hover = hover bg. Click = choose and close. The route already in the other
   slot is shown disabled.
4. Hairline, then two full-width text buttons:
   - `Use the route open in the editor` (only when one is open and it is not
     already in this slot),
   - `Browse for a file…` → `rfd::FileDialog` filtered to `*.json`, starting in
     `paths.saved_routes_dir`. This is how another player's file that was never
     imported gets compared.

Keyboard: ↑/↓ move, Enter chooses, Esc closes the popup (not the page).

### 4.4 Page states

| state | body shows |
|---|---|
| neither / one slot filled | Centred empty state: 40 px two-document outline icon (stroke = `theme.icon_stroke()`), 14 px bold strong `Pick two routes to compare`, 12 px muted `Choose route A and route B above. Routes can be from your saved routes or any route file on disk.` Tabs are drawn but disabled. |
| loading | Centred spinner (`egui::Spinner`) + 12 px muted `Loading <name>…`. Appears only if loading takes > 100 ms (avoid a flash). |
| load failed | `warning_banner` styled with failure colours: **`Could not load <file name>:`** + the engine's error string (e.g. unknown custom gen, invalid JSON). The other slot stays loaded. |
| ready | Banners (§4.5), then the active tab. |

### 4.5 Banners (top of body, all tabs, stacked 6 px apart)

Use `widgets::warning_banner`. Show each that applies, in this order:

| condition | text |
|---|---|
| `a.version != b.version` | **Different games:** A is *Emerald*, B is *Ruby*. Trainers are matched by name and team, so some fights may not line up. |
| generations differ | **Different generations:** stats, EVs and DVs/IVs are not comparable between Gen *N* and Gen *M*. The Pokémon setup and stats cards are hidden. |
| species differ | **Different Pokémon:** A is *Houndoom*, B is *Nosepass*. |
| either route has events with errors | **Route errors:** A has *2* events with errors and B has *1*. Totals after the first error may be off. |
| A and B are the same file | **Same route on both sides.** |

---

## 5. Engine: `xpr-engine/src/compare.rs`

Re-export from `xpr-engine/src/lib.rs`. No egui, no `xpr-app` types. Everything
returned is plain owned data (`String`, numbers, `Vec`) so it is `Send` and can
cross the loader thread's channel.

### 5.1 Step 1 — digest one route

`pub fn digest(router: &Router, label: &str, origin: RouteOrigin) -> RouteDigest`

Precondition: the router is loaded and recalculated (`Router::load` /
`load_value` already do this).

Walk `router.all_groups()` (depth-first, route order). **Skip** a group when
`!router.is_enabled(id)` (count it in `disabled_events`) or when its type is
`TASK_NOTES_ONLY`. For every other group push one `Entry` and update `Totals`.

```rust
pub struct RouteDigest {
    pub label: String,               // file stem, shown in the UI
    pub origin: RouteOrigin,         // CurrentRoute | Saved { mtime } | External
    pub version: String,             // router.pkmn_version
    pub generation: u8,              // gen.get_generation()
    pub species: String,
    pub dvs: [i64; 6],               // hp, atk, def, spa, spd, spe  (gen 1: spa == spd == special)
    pub nature: Option<String>,      // display name; None in gen 1-2
    pub nature_up: Option<String>,   // short stat name ("SpA"); None when neutral
    pub nature_down: Option<String>,
    pub ability: Option<String>,     // None in gen 1-2
    pub hidden_power: Option<(String, i64)>, // gen.get_hidden_power(&dvs); None in gen 1
    pub start: Snapshot,             // router.init_route_state
    pub end: Snapshot,               // router.get_final_state()
    pub totals: Totals,
    pub entries: Vec<Entry>,         // enabled, non-note events in route order
    pub moves_learned: Vec<LearnedMove>,
    pub folder_count: usize,
    pub disabled_events: usize,
    pub final_time: Option<f64>,     // last parseable recorded time, seconds
}

pub struct Snapshot {                // plain copy of a RouteState
    pub level: i64, pub xp: i64,
    pub stats: [i64; 6],             // solo_pkmn.cur_stats (no badge boosts)
    pub evs: [i64; 6],               // solo_pkmn.unrealized_stat_xp (the running total)
    pub money: i64,
    pub moves: [Option<String>; 4],
    pub held_item: Option<String>,
    pub badge_count: usize,
}

pub struct Entry {
    pub kind: EntryKind,             // Trainer, Wild, PickUp, Buy, Sell, UseItem, HoldItem, LearnMove,
                                     // RareCandy, Vitamin, Heal, Save, Blackout, Evolution, BagReorder, EvOverride
    pub key: String,                 // match key, §5.3
    pub label: String,               // display text without the kind ("Poochyena L3", "Poké Ball", "Leader Roxanne")
    pub qty: i64,                    // 1 unless the event has an amount (wild quantity, item amount, candies, vitamins)
    pub folder: String,              // enclosing folder name (display only)
    pub location: Option<String>,    // trainer.location for fights
    pub before: Snapshot,            // group.init_state
    pub xp_gain: i64,                // final.xp - init.xp
    pub money_delta: i64,            // final.money - init.money
    pub recorded_secs: Option<f64>,  // parse "H:MM:SS.ss" from recorded_time_str(); None if absent/unparseable
    pub is_major: bool,              // gen.is_major_fight(trainer_name)
    pub fight_category: Option<&'static str>, // for the checkpoint swatch (consts::FIGHT_CATEGORY_TO_TAG)
    pub team_sig: Option<String>,    // trainers only, §5.3
    pub enemy_count: usize,          // trainers only: get_pokemon_list(gen, false).len()
    pub has_error: bool,
}
```

Rules that are easy to get wrong:

- **Read definitions, never labels.** `EventGroup.name` is replaced by the error
  text when an event has errors. Build `label` from `event_definition` fields.
- **Deltas come from states**, not re-simulation: `xp_gain` and `money_delta`
  are `final_state − init_state` of the group. If either state is `None`
  (errored event), use 0 and set `has_error`.
- **A move counts as learned only if the moveset changed.** For explicit
  `LearnMove` groups and for the engine-injected level-up items
  (`EventItem.shares_group_definition == false` with a `learn_move` definition),
  compare `init_state.solo_pkmn.move_list` with `final_state…move_list`; record
  a `LearnedMove { name, source: LevelUp | TmHm(item) | Tutor, level }` only
  when they differ. A level-up move that was skipped is not "learned".
  Source mapping: `MOVE_SOURCE_LEVELUP` → `LevelUp`, `MOVE_SOURCE_TUTOR` →
  `Tutor`, anything else → `TmHm(source)`.
- **Wild events carry a quantity** (`[name, level, quantity, trainer_pkmn]`).
  `wild_pokemon += quantity`. Events with `trainer_pkmn == true` are counted in
  `trainer_owned_singles` instead (shown as its own row only when non-zero in
  either route).
- Inventory type comes from `InventoryEventDefinition`'s derived type, i.e. the
  same split `get_event_type()` returns: `TASK_GET_FREE_ITEM`,
  `TASK_PURCHASE_ITEM`, `TASK_SELL_ITEM`, `TASK_USE_ITEM`.
- Vitamins vs EV berries: `gen.is_ev_berry(name)` decides which total the
  amount goes to.
- **Heal / Save / Blackout labels**: recorded routes store map ids such as
  `OLDALE_TOWN - POKEMON_CENTER_1F`. `label` = the part before the first
  ` - `, underscores → spaces, title-cased (`Oldale Town`); hand-typed
  locations pass through unchanged; empty → the kind name alone.

```rust
pub struct Totals {
    pub trainers: i64, pub major_fights: i64, pub trainer_pokemon: i64,
    pub wild_pokemon: i64, pub wild_species: i64, pub trainer_owned_singles: i64,
    pub rare_candies: i64,
    pub vitamins: i64, pub vitamins_by_name: Vec<(String, i64)>,
    pub ev_berries: i64,
    pub moves_level_up: i64, pub moves_tm_hm: i64, pub moves_tutor: i64,
    pub items_picked_up: i64, pub purchases: i64, pub sales: i64, pub items_used: i64,
    pub held_item_changes: i64,
    pub heals: i64, pub saves: i64, pub blackouts: i64, pub evolutions: i64,
    pub events_with_errors: i64,
    pub xp_from_trainers: i64, pub xp_from_wild: i64, pub xp_from_candies: i64, pub xp_from_other: i64,
    pub money_from_trainers: i64, pub money_from_sales: i64,
    pub money_spent: i64,            // negative
    pub money_lost_blackouts: i64,   // negative
    pub money_other: i64,            // Pay Day, anything else
}
```

Sanity identities the tests must assert (they hold for the reference pair):
`sum(xp_from_*) == end.xp − start.xp` and
`start.money + sum(money_*) == end.money`.

### 5.2 Step 2 — compare two digests

`pub fn compare(a: RouteDigest, b: RouteDigest) -> RouteComparison`

```rust
pub struct RouteComparison {
    pub a: RouteDigest, pub b: RouteDigest,
    pub compat: Compat,                  // same_version, same_generation, same_species, same_file
    pub rows: Vec<DiffRow>,              // the Event diff tab, top to bottom
    pub checkpoints: Vec<Checkpoint>,    // every trainer present in both, in A's order
    pub trainer_sets: TrainerSets,       // counts + name lists for the Trainers card
    pub both_have_times: bool,
}
pub enum DiffRow {
    Anchor { a: usize, b: usize },                 // indices into a.entries / b.entries: same fight, same order
    Block  { a: Vec<BlockItem>, b: Vec<BlockItem> } // everything between two anchors
}
pub struct BlockItem { pub entry: usize, pub qty: i64, pub status: ItemStatus, pub merged_label: String }
pub enum ItemStatus {
    InBoth,                         // same key and same qty in the other lane of this block
    QtyDiffers { other_qty: i64 },  // same key, different qty
    OnlyHere,                       // not in the other lane of this block
    TrainerMoved { other_after: Option<String> }, // fought in both routes, different order; other_after = name of the
                                                  // anchor it follows on the other side
    TrainerRepeat,                  // a second fight with a trainer already matched earlier (rematch after blackout)
}
pub struct Checkpoint { pub a: usize, pub b: usize, pub same_order: bool, pub moves_differing: usize }
```

### 5.3 Match keys

| kind | key |
|---|---|
| Trainer | `t:` + `sanitize_string(trainer_name)`, plus `+` + sanitized second trainer when it is a non-empty string |
| Wild | `w:` + sanitized species (**level is not part of the key**) |
| PickUp / Buy / Sell / UseItem | `i:<kind>:` + sanitized item name |
| HoldItem | `h:` + sanitized item name (`h:none` when removing) |
| LearnMove | `m:` + sanitized move name (`m:delete:<slot>` for a deletion) |
| RareCandy | `candy` |
| Vitamin | `v:` + sanitized vitamin name |
| Heal / Save / Blackout | `heal` / `save` / `blackout` (location is **not** part of the key) |
| Evolution | `e:` + sanitized evolved species |
| BagReorder / EvOverride | `bag` / `evo` |

`team_sig` (trainers) = sanitized `trainer.location` + `|` + the trainer's
Pokémon in **definition order** as `species:level` joined by `,`.

### 5.4 The alignment algorithm — follow exactly

Let `TA` / `TB` be the trainer entries of A / B in route order.

1. **Equivalent trainers.** For each trainer key that appears in only one
   route, look for a trainer on the other side that is also unmatched by key
   and has an identical `team_sig`. If there is **exactly one** such candidate,
   give both the same synthetic key (`t:eq:<team_sig>`) and a combined display
   name `A-name / B-name` for shared rows.
   *Why:* `Rival Brendan 1 Mudkip` (player is a girl) and `Rival May 1 Mudkip`
   (player is a boy) are the same fight. This rule is data-driven and works for
   every game without an alias table. In the reference pair it pairs 5 rival
   fights.
2. **LCS.** Run a standard longest-common-subsequence DP over the key
   sequences of `TA` and `TB` (O(n·m); worst real case ≈ 300 × 300). When
   backtracking prefers neither side, advance **A first** (deterministic
   output). Every LCS pair becomes a `DiffRow::Anchor`.
3. **Classify the rest.** For each trainer not in the LCS, in route order:
   - its key is in the LCS already, or was already classified once on this side
     → `TrainerRepeat`;
   - else its key exists among the other side's non-LCS trainers →
     `TrainerMoved` (pair k-th occurrence with k-th occurrence);
     `other_after` = display name of the nearest preceding Anchor on the other
     side, or `None` → text "at the start";
   - else → `OnlyHere`.
4. **Blocks.** Between consecutive Anchors (and before the first / after the
   last) emit one `DiffRow::Block` holding, for each side, every entry that
   lies between those two anchors **in route order** — the non-anchor trainers
   from step 3 and all non-trainer entries. Omit a Block when both lanes are
   empty.
5. **Merge inside a lane.** Consecutive-or-not, entries in the same lane with
   the same key merge into one `BlockItem` at the position of the first:
   `qty` = sum; `merged_label` for wild = species + level range
   (`Poochyena L5–6`), otherwise the label.
6. **Status inside a block.** For each merged non-trainer item: other lane has
   the key with equal qty → `InBoth`; with a different qty → `QtyDiffers`;
   not at all → `OnlyHere`.
7. **Checkpoints** = all Anchors (`same_order = true`) plus all `TrainerMoved`
   pairs (`same_order = false`), sorted by A's order. `moves_differing` = number
   of moves in A's `before.moves` set not in B's (set comparison, slot order
   ignored).
8. **`TrainerSets`**: `same_order` = Anchor count; `different_order` = Moved
   pair count; `only_a` / `only_b` = `OnlyHere` trainers; `repeat_a` /
   `repeat_b`; each with the display-name list.

Reference pair expected output (verified against the engine): same order
**115**, different order **14**, only A **29**, only B **23**, fought twice in
A **2** (A: 115+14+29+2 = 160; B: 115+14+23 = 152). The equivalence pass of
step 1 pairs Emerald rivals 1, 3, 4 and 5; it correctly declines rival 2,
whose teams genuinely differ (Brendan's Slugma vs May's Torkoal).

### 5.5 Loading (in `xpr-app`)

```rust
pub enum RouteSource { Path(PathBuf), Value { label: String, json: serde_json::Value } } // Value = current route, D9
fn load_digest(reg: Arc<Registry>, src: RouteSource, origin: RouteOrigin) -> Result<RouteDigest, String>
```

`Router::new(reg)` → `load(&path, false)` or `load_value(&json, false)` →
`compare::digest(...)`. Run on `std::thread::spawn` with an `mpsc::channel`,
polled once per frame with `try_recv()` and
`ctx.request_repaint_after(50 ms)` while pending — the same pattern as the
route-index scan in `app.rs` (`index_rx`). Tag each request with a generation
counter and drop stale results (the user may pick another route before the
first finishes). Keep both `RouteDigest`s so **Swap** and re-picking one side
reload only what changed. `compare()` itself is cheap: run it on the UI thread.

---

## 6. The three tabs

### 6.1 Overview

```
┌ identity card A (50 %) ───────────────────────┐ ┌ identity card B (50 %) ───────────────────────┐
│ [icon] [A] Houndoom (Lv 71) [EMERALD]  FINAL TIME │ │ [icon] [B] Houndoom (Lv 71) [EMERALD]  FINAL TIME │
│        Early Bird · Rash · Held item: Charcoal    │ │        Flash Fire · Quiet · Held item: Leftovers  │
│        519 events in 205 folders · 0 disabled  1:34:54.3 │ │  433 events in 172 folders · 0 disabled  1:20:36.7 │
└───────────────────────────────────────────────┘ │                              14:17.6 faster than A │
                                                   └───────────────────────────────────────────────┘
┌ POKÉMON SETUP (440 px) ───────┐ ┌ FINAL STATS AND EVS (rest) ─────────────────────────────────┐
│ IVs        A    B   B − A     │ │ Stat     Stat A  Stat B  B − A  ▬▬▬   EVs A  EVs B  B − A   │
│ HP        31   30    −1       │ │ HP         223     222     −1   ▬▬▬     81     76     −5    │
│ … 6 rows, then Total          │ │ … 6 rows, then Total EVs                                     │
│ TRAITS                        │ └──────────────────────────────────────────────────────────────┘
│ Hidden Power [Water]70 [Grass]70 differs │ ┌ WHERE THE EXP CAME FROM ───────────────────────────┐
│ Nature   Rash +SpA −SpD  Quiet +SpA −Spe │ │ [A] ████████████████▌███████████   55 · 2 · 43 %    │
│ Ability  Early Bird  Flash Fire  differs │ │ [B] █████████████████▏██████████   57 · 1 · 42 %    │
└───────────────────────────────┘ └──────────────────────────────────────────────────────────────┘
┌ ROUTE TOTALS (rest) ──────────────────────────────────────┐ ┌ MONEY (440 px) ───────────────┐
│ Metric                     A     B   B − A  ▬▬▬   Detail   │ │ rows §6.1.6                   │
│ BATTLES                                                    │ ├ FINAL MOVES ──────────────────┤
│ Trainers fought          160   152    −8    ▬▬▬            │ │ [A] 4 slots │ [B] 4 slots     │
│ …                                                          │ ├ TRAINERS ─────────────────────┤
│                                                            │ │ set counts + expandable names │
└────────────────────────────────────────────────────────────┘ └───────────────────────────────┘
┌ MOVES LEARNED (full width) ──────────────────────────────────────────────────────────────────┐
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

Below 1100 px content width every two-column row stacks into one column, in
the reading order above. The right-hand column's last card (Trainers) grows to
match the height of Route totals.

#### 6.1.1 Identity cards

Same construction as `state_views::identity_header` (68 px icon tile with
`Assets::pkmn_icon` fitted to 60 px; 14 × 16 padding), with these parts:

- Row 1: route badge · species (22 px bold strong) · level pill `Lv N` = final
  level (neutral, §3.1) · game chip (neutral, §3.1).
- Row 2 (gen ≥ 3): `ability · nature · Held item: <name|none>`; gen 2: held
  item only; gen 1: row omitted.
- Row 3 (muted): `<N> events in <M> folders · <K> disabled`.
- Right block, right-aligned: caption `FINAL TIME`, value 18 px bold strong
  (`H:MM:SS.s`). On **B only**, third line: `<delta> faster than A` (success)
  or `<delta> slower than A` (failure), 11 px. If a route has no recorded
  times, the block shows `FINAL TIME` / `—` and the delta line is omitted.

#### 6.1.2 Pokémon setup card

Hidden when generations differ. Columns: name · A (72 px) · B (72 px) · B − A
(64 px).

- Head cell reads **`DVs`** in gen 1–2 and **`IVs`** in gen ≥ 3 (same rule as
  `custom_dvs.rs` `dv_text`).
- Rows: `HP, Attack, Defense, Sp. Atk, Sp. Def, Speed`; gen 1: `HP, Attack,
  Defense, Special, Speed`. Then a **Total** row (pane-divider on top).
- Group heading **TRAITS**, then:
  - **Hidden Power** (gen ≥ 2; omitted in gen 1): a type cell
    (`summaries::type_color(type)` background, radius 3, 11 px bold, text
    colour per `LIGHT_TEXT_TYPES`) followed by the base power. Δ = `differs`
    when type or power differ, else `—`.
  - **Nature** (gen ≥ 3): display name + muted `+SpA −SpD`; nothing after a
    neutral nature.
  - **Ability** (gen ≥ 3).

#### 6.1.3 Final stats and EVs card

Hidden when generations differ. Columns: Stat · Stat A · Stat B · B − A ·
paired bars (fill = stat / max stat among all 12 shown values) · EVs A · EVs B ·
B − A. Last row **Total EVs**. Gen 1–2 wording: `StatExp A`, `StatExp B`,
`Total StatExp` (same vocabulary switch as `state_views::stats_card`). Stats
are `end.stats` (no badge boosts, so the two routes are comparable); EVs are
`end.evs`.

#### 6.1.4 Where the exp came from

Two 30 px rows: route badge · 16 px stacked bar (radius 3; segments trainers /
wild / candy, plus "other" in track colour if non-zero; widths = share of that
route's total gained exp) · right caption `55 · 2 · 43 %`. Legend row under it
(8 px swatches, 11 px muted). Hovering a segment shows the exact exp with
thousands separators. Card title right-hand caption: total exp gained, or
`A 447,779 · B 451,002` when they differ.

#### 6.1.5 Route totals card

Columns: Metric · A (56) · B (56) · B − A (64) · paired bars (120) · Detail
(rest, 11 px muted, left-aligned). Group headings are uppercase muted captions.
Sub-rows are indented 14 px with muted names. **A row is omitted when it is 0
in both routes**, except the first row of each group.

| group | rows (source in `Totals`) |
|---|---|
| BATTLES | Trainers fought · ↳ of which major fights · Trainer Pokémon defeated · Wild Pokémon fought · ↳ different species · Trainer-owned singles (wild events flagged `trainer_pkmn`; 8 vs 2 in the reference pair) |
| LEVELLING | Final level (`end.level`) · Rare Candies used · Vitamins used (Detail: `A: Protein ×2 · B: Calcium ×6, Protein ×2`) · EV berries used |
| MOVES | Moves learned (sum) · ↳ by level-up · ↳ by TM / HM · ↳ by tutor or deleter |
| ITEMS | Items picked up · Purchases · Sales · Items used or dropped · Held item changes |
| OTHER | Pokémon Center heals · Saves · Blackouts · Evolutions · Events with errors |

#### 6.1.6 Money card

Title right caption `start $3,000` (`start.money`; if the two differ:
`start A $3,000 · B $0`). Rows: Won from trainers · Sales · Purchases · Lost to
blackouts · Other (omit when 0 in both) · **Final money** (pane-divider on top,
values 14 px bold). Δ column is `±$N`.

#### 6.1.7 Final moves card

Two columns, each headed by a route badge; four 30 px rows: slot number (11 px
muted) + move name. A move that the other route's final moveset **also
contains** (any slot) is plain *text*; a move the other lacks is **bold
strong**. Empty slot = muted `—`. `Hidden Power` shows its type:
`Hidden Power (Water)`.

#### 6.1.8 Trainers card

Title right caption `160 vs 152`. Five 30 px rows, count right-aligned 14 px
bold: `Fought in both, same order` · `Fought in both, different order` ·
`[A] Only in A` · `[B] Only in B` · `[A]/[B] Fought twice in A/B` (omitted when
0). The last three kinds have a disclosure triangle; expanding one lists its
trainers as small name chips (11 px, well bg, card border, radius 4, wrapping).
Clicking a chip switches to **Event diff** and scrolls to that trainer.

#### 6.1.9 Moves learned card

One row per distinct move learned in either route, ordered by the earlier of
the two levels. Columns: Move · Source (11 px muted: `Level-up`, `TM46`,
`Tutor`; if the routes used different sources: `TM35 / Level-up`) · `A · Lv`
(220 px) · `B · Lv` (220 px). A cell lists the level(s) at which that route
learned it (`30`; `34, 48` when learned twice; `—` when never). When both cells
are identical the row is muted; otherwise the move name is bold strong.

### 6.2 Checkpoints

```
[ Major fights | All shared trainers (130) ]   State before each fight both routes have. Click a row for stats and moves.
┌──────────────────────────────────────────────────────────────────────────────────────────────┐
│ Fight                 Lv A  Lv B  B − A   Time A   Time B    A vs B    Money A  Money B  EVs A  EVs B  Moves    │
│ ■ Leader Roxanne       18    16    −2    14:03.4  11:04.8   +2:58.6      $604   $2,583    100     69   same     │
│ ■ Leader Winona        38    38     —    55:33.7  44:40.1  +10:53.6   $55,479   $2,791    305    342   3 differ │
│   └ expanded: stats + EVs table │ [A] held item + 4 moves │ [B] held item + 4 moves                             │
```

- Toolbar: a two-button segmented control (`widgets::seg_toggle`): **Major
  fights** (default; `Entry.is_major` on either side) / **All shared trainers
  (N)**. Then an 11 px muted hint.
- One card holding one table; 27 px rows; hover = hover bg; the whole row is
  the click target.
- **Fight**: 8 px square swatch in the fight-category colour
  (`consts::FIGHT_CATEGORY_TO_TAG`, 1 px `theme.divider` outline so the near-
  black gym-leader colour stays visible) + trainer name, 12 px bold strong.
  A fight that is in both routes but in a different order gets a muted `↕`
  after the name with tooltip `Fought in a different order in B`.
- **Lv / EVs** follow §3.3. EVs = sum of `before.evs` (gen 1–2 head:
  `StatExp A/B`). Money cells are plain *text*.
- **Time A / Time B**: `M:SS.s` under an hour, else `H:MM:SS.s`.
  **A vs B** = `time A − time B`, signed, 80 px: positive (A took longer → B is
  ahead) in **failure**; negative in **success**; tooltip spells it out:
  `A reached this fight 2:58.6 later than B`. If `both_have_times` is false the
  three time columns are **not drawn at all**.
- **Moves**: `same` (muted) or `N differ` (text strong).
- **Expanded row** (one at a time; click again to collapse): well-bg panel,
  padding 12 × 14, three columns 1.3 / 1 / 1: a mini stats table
  (`Before <fight>` · Stat A · Stat B · B − A · EVs A · EVs B, 24 px rows), then
  for each route: badge + `Held: <item|none>` caption and the four moves
  (shared moves plain, differing moves bold strong).
- Empty state (no shared fights): centred muted
  `These routes have no trainer fights in common.`

### 6.3 Event diff

```
[Trainers][Wild][Items][Moves][Candy & vitamins][Heals & saves]        ▌only in A  ▌only in B  grey = in both  ═ same fight  ↕ different order
┌ [A] e-houndoom-1-13454 ───────────────────┬────┬ [B] e-ike-houndoom ─────────────────────────┐
│ Rival Brendan 1 / May 1 Mudkip  Route 103  Lv 5  1:21.3 │ ═ │ Rival Brendan 1 / May 1 Mudkip   −0:04.5  Lv 5  1:16.8 │  anchor row
│ ▌WILD  Poochyena L3                   ×2  │    │  ITEM  Poké Ball                        ×5  │  block
│ ▌WILD  Zigzagoon L2                       │    │                                             │
│  ITEM  Poké Ball                      ×5  │    │                                             │
│ ▌HEAL  Oldale Town                        │    │                                             │
│ Youngster Calvin  Route 102    Lv 6  2:09.6 │ ═ │ Youngster Calvin              −0:21.0  Lv 5  1:48.6 │
```

- One bordered container (card bg, card border, radius 8) with a 30 px sticky
  head (strip bg): route badge + route name per lane. Three columns:
  `1fr · 44 px gutter · 1fr`; the gutter has row-divider lines on both sides.
- **Anchor row** (30 px, bg = `lighten(card bg, 0.02)`, pane-divider on top):
  each lane shows trainer name (12 px bold strong, elided) · location (muted,
  A lane only) · right-aligned `Lv N` (N in the route colour) · recorded time
  (muted). The B lane also shows the time delta (`B − A`; success when
  negative, failure when positive, 11 px) before `Lv`. Gutter glyph `═`.
- **Block** (row-divider on top): each lane lists its `BlockItem`s as 24 px
  lines: kind caption (52 px, 11 px uppercase: `TRAINER, WILD, ITEM, BUY, SELL,
  USE, HOLD, MOVE, CANDY, VITAMIN, HEAL, SAVE, BLACKOUT, EVOLVE`) · label ·
  right-aligned qty (`×5`). An empty lane shows a single muted `—`.

  | status | rendering |
  |---|---|
  | `InBoth` | whole line muted; kind caption in disabled colour |
  | `QtyDiffers` | as `InBoth`, but qty reads `×10 vs ×7` in *text* colour |
  | `OnlyHere` | label text strong; kind caption in the route colour; a 3 × 12 px route-colour marker at the lane's left edge |
  | `TrainerMoved` | name bold, line otherwise muted; trailing 11 px muted `↕ A fights them after Team Aqua Grunt 15` (in the A lane: `↕ B fights them after …`; with `other_after == None`: `… at the start`). Always `them`; trainer gender is not in the data |
  | `TrainerRepeat` | as `OnlyHere`, trailing muted `rematch` |

- **Filter toggles** (multi-select `seg_toggle`s; default all on except *Heals
  & saves*): Trainers · Wild · Items (pick-ups, buys, sells, uses, held items,
  bag reorders) · Moves · Candy & vitamins · Heals & saves (heals, saves,
  blackouts). Turning **Trainers** off hides one-sided trainer lines inside
  blocks; anchor rows always remain. A block whose lanes are both empty after
  filtering is skipped.
- **"Highlight differences" off** renders every line in plain *text* colour
  with no markers.
- A legend sits right-aligned in the toolbar (as drawn above).
- **Virtualise** like `route_list.rs`: precompute each row's height
  (anchor = 30; block = 6 + 24 × max(lane lengths)), allocate every row, paint
  only rows intersecting `ui.clip_rect()`. A 300-trainer Gen 2 route must
  scroll at 60 fps.
- Footer caption (11 px muted): `<N> shared fights · <M> blocks with
  differences`.

---

## 7. App integration (`xpr-app`)

### 7.1 Files

| file | change |
|---|---|
| `xpr-engine/src/compare.rs` | **new** — §5 |
| `xpr-engine/src/lib.rs` | `pub mod compare;` |
| `xpr-engine/tests/route_compare.rs` | **new** — §9.1 |
| `xpr-engine/examples/compare_dump.rs` | **new** — prints a comparison headlessly, for checking §9.3 by hand |
| `xpr-app/src/compare/mod.rs` | **new** — `CompareView` (state, loading, shell, banners, picker popup) |
| `xpr-app/src/compare/overview.rs`, `checkpoints.rs`, `event_diff.rs` | **new** — one file per tab |
| `xpr-app/src/compare/text_export.rs` | **new** — §8.2 |
| `xpr-app/src/app.rs` | `Page::Compare`; `compare: CompareView` and `compare_return: Page` fields; match arm in the `CentralPanel` dispatch; File-menu item; `fire("compare_routes")`; poll the loader; `ShotKind::Compare` in `export_shot` |
| `xpr-app/src/pages.rs` | landing-page **Compare Routes** button |
| `xpr-app/src/screenshot.rs` | `ShotKind::Compare(CompareTab)` |
| `xpr-core/src/config.rs` | `compare_routes` shortcut (default, label, category "File") |
| `xpr-ui-kit/src/widgets.rs` | `route_badge`, `paired_bars`, `stacked_bar`, `card_title_colored`, `delta_cell` (§3.3 in one place) |
| `rust/TESTING.md` | new numbered section (§9.3) |

### 7.2 `CompareView`

```rust
pub struct CompareView { /* slots, digests, comparison, tab, filters, expanded checkpoint, picker state, loader rx */ }
pub enum CompareAction { None, Back, Export(CompareTab), CopySummary }
impl CompareView {
    pub fn open(&mut self, a: Option<RouteSource>);      // called by both entry points
    pub fn poll(&mut self, ctx: &egui::Context);          // drain the loader channel; call every frame while on the page
    pub fn ui(&mut self, ui: &mut Ui, theme: &Theme, assets: &mut Assets, env: &CompareEnv) -> CompareAction;
}
```

`CompareEnv` carries `Arc<Registry>`, `&Paths`, `&RouteIndex`, and an
`Option<RouteSource>` factory for "the route open in the editor". The view
never holds a `MainController`.

---

## 8. Export

### 8.1 Screenshot

**Export screenshot** → `request_shot(ShotKind::Compare(tab))`, rendered through
the existing offscreen pipeline (`screenshot.rs`) at the content's natural
size with no scroll clipping: for Overview and Checkpoints the whole tab; for
Event diff the whole diff when ≤ 4,000 px tall, otherwise the currently visible
range. The header's route names are drawn above the content as a 40 px title
strip (`[A] name   vs   [B] name`). File name
`compare_<a-label>_vs_<b-label>_<tab>.png` in the configured images dir; the
existing "Saved screenshot to …" toast with *Open Folder* applies.

### 8.2 Copy summary

**Copy summary** puts a plain-text table on the clipboard
(`ctx.copy_text(...)`) and shows the toast `Summary copied`. Format (monospace-
friendly, pastes cleanly into Discord inside a code block):

```
Route compare — Emerald · Houndoom
A: e-houndoom-1-13454      B: e-ike-houndoom
                          A          B      B-A
IVs (HP/At/Df/SA/SD/Sp)   31/31/31/30/31/30   30/30/31/30/31/31
Nature / Ability          Rash / Early Bird   Quiet / Flash Fire
Hidden Power              Water 70            Grass 70
Final level               71         71        0
Final time                1:34:54.3  1:20:36.7 -14:17.6
Trainers fought           160        152       -8
Wild Pokémon fought       76         43        -33
... (every Route totals and Money row that is non-zero in either route)
```

---

## 9. Testing

### 9.1 Engine tests — `xpr-engine/tests/route_compare.rs`

Fixtures: the routes in `tests/test_data/`. Build variants in the test by
mutating the parsed JSON and calling `Router::load_value`.

| test | expectation |
|---|---|
| route vs itself | every Δ is 0; all trainers are Anchors; no `OnlyHere`/`QtyDiffers`; `checkpoints.len()` = trainer count |
| an unknown trainer name | `Router::load` **fails outright** — such a route never reaches `digest`, which is why §4.4 needs a per-slot failure banner |
| identities | the two sanity identities of §5.1 hold for every fixture |
| remove one trainer from B | 1 `OnlyHere` trainer in A; `totals.trainers` differs by 1; B's later `before.level` values are ≤ A's |
| swap two adjacent trainers in B | exactly one `TrainerMoved` pair; `trainer_sets.different_order == 1` |
| Emerald `Rival Brendan 1 Mudkip` vs `Rival May 1 Mudkip` (minimal routes built in the test) | paired as one Anchor with the combined name |
| Emerald `Rival Brendan 1` vs `Rival Brendan 2` | **not** paired — different teams |
| two unmatched trainers on one side with the same `team_sig` | **not** paired (ambiguous) |
| duplicate a trainer in A | second occurrence is `TrainerRepeat` |
| disable a folder in B | its events vanish from entries/totals; `disabled_events` grows |
| wild `quantity = 3` | `wild_pokemon` rises by 3; one merged BlockItem `×3` |
| level-up move with no destination | not in `moves_learned` |
| DVs differ | `hidden_power` differs in gen 2+; `None` in gen 1 |
| Yellow vs Platinum | `compat.same_generation == false`; no panic |
| route with an errored event | `has_error` set; `events_with_errors` counted; no panic |

### 9.2 App smoke hook

`XPR_SMOKE_ACTION=compare` with `XPR_SMOKE_COMPARE_A` / `XPR_SMOKE_COMPARE_B`
(paths) and optional `XPR_SMOKE_COMPARE_TAB` (`overview|checkpoints|diff`):
opens the page, waits for the comparison, screenshots, exits. Same mechanism as
`XPR_SMOKE_ACTION=prestate`.

### 9.3 Manual checklist (add to `rust/TESTING.md`)

- [ ] Reference pair: Overview matches `mockup.html` number for number
      (IV total 184 vs 183; HP Water 70 vs Grass 70; trainers 160 vs 152; wild
      68 vs 41 plus 8 vs 2 trainer-owned; blackout loss −$96,015 vs −$2,583;
      final money $78,071 vs $90,047; final time 1:34:54.3 vs 1:20:36.7).
      `cargo run -p xpr-engine --example compare_dump -- A.json B.json` prints
      every one of these for comparison.
- [ ] Trainers card reads 115 / 14 / 29 / 23 / 2.
- [ ] `Rival Brendan 1 Mudkip` and `Rival May 1 Mudkip` share an anchor row
      (likewise 3, 4 and 5); rival 2 stays one-sided — the teams differ.
- [ ] Swap flips every sign and every colour role; nothing reloads.
- [ ] Open from the editor with unsaved edits: A reflects the edits; going
      Back leaves selection, undo stack and scroll position untouched.
- [ ] Open from the landing page with nothing selected: empty state, tabs
      disabled.
- [ ] Browse to a file outside the saved-routes folder.
- [ ] A route whose custom gen is not installed: failure banner, other slot
      stays loaded.
- [ ] Gen 1 pair: `DVs`, `Special`, `StatExp`; no Hidden Power, nature,
      ability or held-item text anywhere.
- [ ] Gen 2 pair: `DVs`, `StatExp`, Hidden Power shown, held item shown, no
      nature/ability.
- [ ] Pair without recorded times: no time columns, identity cards show `—`.
- [ ] Different games (Ruby vs Emerald) and different generations: correct
      banners; setup and stats cards hidden only for different generations.
- [ ] Custom theme colours recolour A/B consistently; nothing hard-coded.
- [ ] Widths 1000–1920: Overview reflows at 1100; nothing clips horizontally.
- [ ] 300-trainer Crystal route: Event diff scrolls smoothly.
- [ ] Both exports work on all three tabs.
- [ ] `cargo clippy` clean; `cargo test --workspace` green; `xpr-golden verify`
      unchanged (no engine behaviour was touched).

---

## 10. Build order

Each phase is independently shippable and reviewable.

| phase | delivers | done when |
|---|---|---|
| **1. Engine** | `compare.rs` (§5) + tests (§9.1). No UI. | tests green; a throwaway `println!` of the reference pair reproduces the numbers in §9.3 |
| **2. Shell + Overview** | `Page::Compare`, both entry points, picker popup, background loading, all page states and banners, the whole Overview tab, new ui-kit widgets | the Overview half of §9.3 passes |
| **3. Checkpoints** | §6.2 including the expanded row | matches the mockup's Checkpoints tab |
| **4. Event diff** | §6.3 with filters and virtualisation; Trainers-card chips jump into it | matches the mockup; large-route scroll check passes |
| **5. Export + polish** | §8, smoke hook, TESTING.md section, shortcut in the shortcuts dialog | all of §9.3 passes |

## 11. Deliberately left for later

- Hosting `CompareView` in a second OS window next to the editor, with
  "jump to this event in the editor".
- Per-fight comparison of battle strategy (setup moves, exp splits, mon order).
- Comparing against a run being recorded live.
- A `xpr-golden compare a.json b.json` CLI (trivial once phase 1 exists).

---

## 12. Changes made during implementation

Decided while building; the rest of this document describes the code as it
stands.

1. **Event-diff anchor rows show each route's own trainer name.** §5.4 step 1
   gives paired fights a combined `A-name / B-name` label. That label is used
   where one string must stand for both (the checkpoint table, the trainer
   lists), but each lane of the event diff shows the name *that route* uses —
   `Rival Brendan 1 Mudkip` on the left, `Rival May 1 Mudkip` on the right —
   which reads better and is what the two files actually say.

2. **Three glyphs are painted, not typed.** `\u{21c4}` (swap), `\u{2550}`
   (same fight) and `\u{2195}` (different order) are missing from some system
   fonts and rendered as tofu. They are drawn instead, by
   `widgets::paint_swap_icon`, `paint_equals_mark` and `paint_moved_mark`.
   The moved-fight note in a block reads `moved: B fights them after …`.

3. **Wild encounters flagged `trainer_pkmn` are their own total.** §6.1.5 lists
   "Trainer-owned singles" as a row; `Totals.wild_pokemon` therefore excludes
   them (68 vs 41 for the reference pair, with 8 vs 2 trainer-owned). An
   earlier draft of §9.3 quoted the combined figures.

4. **The trait rows of the setup card get their own wider columns**
   (`shared::trait_cols`, 108 px) — ability and nature names do not fit the
   72 px the DV/IV rows use. A nature's `+SpA \u{2212}SpD` modifiers are
   dropped rather than allowed to collide with a long nature name.

5. **`ShotKind::Compare` is exempt from the "a route must be open" check** in
   `request_shot`, like `RunSummary` and `SetupSummary`: the compare page has
   its own two routes and is reachable from the landing page with the editor
   empty. Its export is named `compare_<a>_vs_<b>_<tab>.png` (§8.1) rather
   than going through `MainController::screenshot_path`, which prefixes the
   editor's route name.

6. **Per-tab state lives in a `TabState` sub-struct** so the body can mutate it
   while `RouteComparison` is borrowed for drawing.

7. **`xpr-app/build.rs` embeds the Windows exe icon behind `#[cfg(windows)]`.**
   `winresource` is a Windows-only build dependency, so the unconditional call
   broke `cargo build` on macOS. Pre-existing, unrelated to this feature, fixed
   here because it blocked building at all.

8. **Still open from §9.1:** the "two unmatched trainers with the same
   `team_sig` are left alone" case is enforced in `equivalent_keys` but has no
   test of its own; the fixtures contain no such pair. The rule is covered
   indirectly by `differently_teamed_fights_do_not_pair`.

---

## 13. Review findings (2026-09-20, fixed)

A code review after the first implementation pass. The screenshot runs had
only ever *drawn* the page, so every defect below sat on a path they never
took. `xpr-app/tests/compare_page.rs` now drives the page through a headless
`egui::Context` with synthetic clicks and keys; each of its picker tests was
checked against the original code and fails there.

| # | defect | fix |
|---|---|---|
| 1 | **Editor shortcuts fired on the compare page.** The editor's route stays loaded underneath, so Ctrl+Z, Delete, Ctrl+B, move/highlight/candy keys changed a route nobody could see. | `handle_shortcuts` returns before the first editor shortcut when `page == Compare`; `request_shot` refuses every kind but `Compare` there (F5-F8 would have exported the editor as last drawn). |
| 2 | **The route picker closed on the click that opened it** (`any_click()` is still true that frame), so it could never be used; picking a game in its dropdown also counted as a click outside. | The opening frame is exempt, and a click only counts as outside when it lands below `Order::Foreground` — the popup and the dropdown are both foreground layers. The button toggles its own picker. |
| 3 | **The open route was serialized every frame** to fill `CompareEnv`. | The env carries the route's *name*; "Use the route open in the editor" is a `CompareActions::use_current_route` request the app answers once. |
| 4 | **Trainer-name chips jumped to the end of the diff.** The chips are one-sided fights, which live inside blocks; the search only looked at anchor rows. | `offset_of_trainer` searches both row kinds and both lanes. |
| 5 | **The event-diff export was unbounded** — about 18,000 px for the reference pair, i.e. a ~430 MB bitmap at 2 px/pt, and clipped by the 16,000 px canvas anyway. | Capped at `MAX_EXPORT_H` (6,000 px), starting at the row the page is scrolled to — the rule §8.1 already stated. |
| 6 | **No landing-page entry point**, though §4.1 specifies one. | `Compare Routes` button under *Load Selected Route*; the selected route becomes A. |
| 7 | **Block items were paired by display label, not match key**: heals in different towns never matched (against §5.3), and the `" L"` split used to strip wild levels could collide on item names. | `BlockItem.key`; merged heals/saves/blackouts in different places read `Heal` / `Save` / `Blackout` rather than borrow the first one's town. |
| 8 | `moves_differing` counted only "A has, B lacks", so a moveset that was a superset read as `same`. | The larger of the two one-sided differences. |
| 9 | Identity card B was narrower than A (it measured what A left over). | One width, measured before either card. |
| 10 | Reopening the page did not re-read an edited open route (D9). | `open()` always reloads a `RouteSource::Value`. |
| 11 | Esc closed the picker *and* left the page. | The picker consumes the key. |
| 12 | The search box re-took focus every frame; the list was silently cut at 400 rows; the popup sat at a hard-coded offset; "Summary copied" went to the editor's status label, invisible here. | Focus once on open; an "N more — search to narrow" line; anchored under its button; a toast. |
| 13 | "Same route on both sides" and the picker's "already chosen" compared file *stems*, so another player's identically named file was misreported. | Both compare paths; the engine's `Compat.same_file` is documented as a hint the caller overrides. |

Not defects, but noted: `xpr-golden verify` was not run — no golden corpus is
on this machine. The engine change is additive (`compare.rs` plus two lines of
`lib.rs`), so recalculation cannot have changed; run it before release anyway.
