# Rust Port — Implementation Plan

Target: rewrite the router as a pure-Rust codebase that behaves exactly like the current app (branch `ui`), looks the same, ships as a single file on Windows and macOS, and boots and edits in milliseconds instead of seconds.

This plan is grounded in measurements taken on this machine on 2026-09-10 (Python 3.14.2, PySide6 6.10.2 / Qt 6.10.2, 1,947 saved routes in the configured data directory) and on three full read-throughs of the codebase (UI layer, core engine, I/O subsystems). Reproduction scripts are in `docs/rust_port/bench/`. Revision 2 records the decisions taken on 2026-09-10 (§0).

---

## 0. Summary and recorded decisions

**Approach.** A Cargo workspace with a pure-Rust core (data, stats/EXP/money, damage calc, routing engine, undo, recorder FSMs, updater) and a pure-Rust UI built on **egui/eframe**, themed to reproduce the current Qt stylesheet, fonts, bitmaps, layout and interaction behavior. Everything that makes the app slow today is architectural (§1) and is replaced by Rust-native designs during the port, so the port delivers the speed rather than a faster copy of the same bottlenecks. Behavior is proven identical by a golden corpus generated from your own 5,200 route files; visuals are proven near-identical by side-by-side screenshot overlays and a per-screen sign-off (§2, §5.3).

**Expected outcome** (targets, measured against the §1 baseline):

| Metric | Today | Target |
|---|---|---|
| Cold start to interactive landing page (dev) | ~12 s | < 0.3 s |
| Packaged start | ~15 s (3 s of that is onefile extraction) | < 0.4 s |
| Load a 2,800-event route (UI settled) | 1.2 s | < 100 ms |
| Delete / re-enable / highlight one event (UI settled) | 500–630 ms | < 16 ms (one frame) |
| Undo | 350 ms | < 16 ms |
| Select a 6-mon trainer fight (battle summary) | 400 ms | < 16 ms |
| Distributable | 173 MB single exe, extracts to temp on every launch | ~15–25 MB: one `.exe` on Windows, one universal `.app` on macOS; nothing extracted |

**Decisions recorded 2026-09-10:**

- **D1 — UI toolkit: pure Rust.** Primary: egui/eframe (§3). Fallback if the Phase 0 spike's text rendering is rejected: Slint with the Skia renderer. No Qt SDK, no C++ glue.
- **D2 — Source of truth: branch `ui`.** `main` is still the old tkinter app and is not consulted except to check for core commits missing from `ui` (`git log ui..main`).
- **D3 — Freeze policy (adopted as proposed; say so if you want otherwise).** Tag `ui` as `rust-port-baseline`; Python changes after the tag are bug fixes only, each with a test or golden record, mirrored in the Rust tracker.
- **D4 — Bugs: fix them.** The ledger in Appendix B becomes a fix list with a disposition per item. Engine-affecting fixes land in the Python reference first so the golden oracle stays valid (§5.6); UI/recorder-only fixes land directly in Rust. Known damage-calc divergences from the games (`docs/damage_calc_review/`) are a separate workstream and not port-blocking.
- **Platinum money (Appendix B item 1): deferred.** The port reproduces current behavior (3,240 in the test route, gen-4 prize money × 4) and records it as known issue KI-1 in the Rust code, the test suite and Appendix E. It is not fixed as part of the port.
- **D5 — Drop dead code:** `gui/` (tkinter; the upgrade window is rebuilt in Rust), `webserver/`, `scripts/`, `calculate_speed_tiers.py`, `scarlet_violet.js`, the damage-review reference harnesses, and the `msgpack`/`requests` dependencies.
- **D6 + D7 — Single distributable file, Windows and macOS.** Windows: one statically linked `.exe`. macOS: one universal (x86_64 + arm64) `.app` bundle delivered as one `.zip`/`.dmg` (a bundle is a directory by design; a bare Mach-O binary is possible but gets no icon, no Dock identity and worse Gatekeeper treatment — §4.5). The Windows release must still satisfy the old updater so existing installs upgrade themselves.
- **D8 — Interim Python relief: not planned** unless you ask for it.

---

## 1. What the measurements say

### 1.1 Boot (dev, warm cache; scripts `bench_gui.py`, `prof_gui.py`)

| Stage | Time | What it is |
|---|---|---|
| Python interpreter | 0.07 s | |
| `import PySide6.QtWidgets` | 0.13–0.22 s | |
| Import app modules | 1.8–2.5 s | Every gen object is built at import time: all 20 game versions parse their JSON eagerly and run 8 validation passes (gen 4: 0.37 s, gen 5: 0.30 s, gen 3: 0.26 s, gen 2: 0.12 s, gen 1: 0.04 s). Versions that share a dataset (Red/Blue, Diamond/Pearl…) parse it separately. Another ~0.45 s is `signalrcore`/`requests`/`websocket`/`numpy`, needed only for recording. |
| `QApplication` + stylesheet | 0.06 s | |
| `MainWindow()` | 6.1–6.7 s | `LandingPage.refresh_routes` runs twice and `json.load`s **all 1,947 route files (456 MB) each time** to read two fields: 3.7 s. Status bar 0.55 s, event-details pane 1.1 s, battle-summary widgets 0.7 s, new-route page 0.5 s (20 box-art decodes). |
| `window.run()` | 0.26 s | |
| First event-loop pass | 3.5 s | `NewRoutePage._pkmn_version_callback` parses all 1,947 route files a **third** time to fill the base-route dropdown. |
| **Total to first interactive frame** | **~11.9 s** | |
| Packaged onefile exe | +~3 s | The 173 MB PyInstaller onefile extracts to `%TEMP%` on every launch (5.0 s just to reach `argparse`). |

### 1.2 Interaction (crystal route: 2,834 event groups, 85 folders; script `bench_interact.py`)

| Action | Sync on GUI thread | UI settled |
|---|---|---|
| Load route | 628 ms | 1,240 ms |
| Select an ordinary event | 82 ms | 142 ms |
| Select a 6-mon trainer fight | 405 ms | 461 ms |
| Toggle highlight on one event | 498 ms | 551 ms |
| Delete one event | 629 ms | 679 ms |
| Undo | 347 ms | 420 ms |
| Search filter | 8 ms | 15 ms |

### 1.3 Engine (headless; scripts `bench_route.py`, `bench_bsc.py`)

| Operation | Time |
|---|---|
| `json.load` of a 1.2 MB route | 7 ms |
| `Router.load` (2,834 groups) | 87 ms |
| Full route recalculation | 40–60 ms |
| `Router.save` | 70 ms |
| One `calculate_damage` (gen 1) | ~34 µs |
| One `find_kill` at depth 20 | ~0.3 ms |
| Battle summary for a 6-mon fight | 2–14 ms |
| All 168–296 fights of a route | 0.6–0.8 s |

### 1.4 Implications for the port

- The route engine and damage math are **not** the bottleneck (50 ms and ~10 ms). Rust makes them 30–100× faster, which is welcome but not what you will feel.
- What you feel is **UI churn**: a one-event edit re-walks all 2,800 rows, upserts `QStandardItem`s, calls computed properties per row, re-parents rows with `takeRow`/`insertRow`, re-hides rows in three separate passes, and mutates the model inside `drawRow`; selecting a fight rebuilds stylesheet strings on 48 damage-summary widgets. An immediate-mode UI that draws only the visible rows from the engine state removes this class of problem entirely (§7).
- Boot is dominated by **parsing every saved route three times**, eager loading of all 20 game databases, and onefile extraction. These are design choices, not language costs. The port fixes them with a metadata index, lazy per-version loading, and a native binary.
- Honest caveat: roughly 80 % of the boot and edit latency could be removed in the Python app in a few days with the same three changes. Per D8 that is not planned; the port is the fix.

---

## 2. Goals, non-goals, and what "exact" means

**In scope:** everything reachable from the app: landing page, new-route page, route editor (route list, filter bar, inline creator, quick-add popover, all event editors, pre-event state viewers, battle summary with all controls), run/setup summary windows, 14 dialogs, menus and all ~100 configurable shortcut actions, recording (GameHook for Yellow/RB, Crystal, Emerald/FRLG, Platinum/HGSS, Black/White/B2W2; Super Shuckie timestamps), custom gens (create/load/ASM backport import), screenshots/exports, config file, auto-update, rotating logs — on Windows and macOS.

**Out of scope (D5):** the tkinter `gui/` package, `webserver/`, `scripts/`, `calculate_speed_tiers.py`, `scarlet_violet.js`, `docs/damage_calc_review/reference/*.py`, `msgpack` and `requests`.

**"Works exactly"** means, for every input the Python app accepts:
1. Identical route state after every event (level, EXP, stats, stat-EXP/EVs, money, inventory, badges, moves, evolution, error flags and messages, event items, tags, rendered names).
2. Identical battle-summary output (every damage range, crit range, kill-range line, percentage, best-move flag, speed comparison) under every strategy, threshold, search-depth, force-full-search and ignore-accuracy setting, because those config keys change computed output.
3. Byte-identical saved route files and notes exports; identical undo results (including that event ids are re-minted and the selection clears).
4. Identical recorder output for the same stream of GameHook property changes.
5. Identical config file location, keys and values per platform; existing configs, data directories, `outdated_routes/` backups and custom gens load unchanged.
6. Identical except where Appendix B records an approved fix, and except KI-1 (Appendix E), which is preserved.

**"Looks the same"** (D1 makes pixel identity impossible; this is the standard the port is held to):
- Same geometry: every widget's position and size within ±1 px of the Qt reference at 100 % and 150 % scale, same column widths, indentation (16 px), row heights (18 px minimum), splitter fractions, margins and paddings.
- Same colors, taken from the same config keys through the same derivation math as `theme.py`; same highlight precedence; same hover/pressed/checked/disabled/focus states; same bitmaps (checkbox and branch-arrow GIF frames, icons, box art) at the same sizes.
- Same fonts and point sizes (configured family, 9 pt base, the same per-widget overrides), same alignment, elision and wrapping.
- Same timings: tooltip 100 ms, toast fade 200 ms and 5 s hold, every debounce.
- Allowed differences: glyph rasterization (egui renders grayscale-antialiased text; Qt on Windows uses ClearType-style subpixel rendering; on macOS both are grayscale), the OS window frame, native file-dialog chrome, and the color picker (an in-app picker replaces `QColorDialog`).
- Verified by the screenshot overlay suite (§5.3) and a per-screen sign-off by you. The Phase 0 spike is where you accept or reject the text rendering before the UI phase starts.

---

## 3. UI toolkit (D1: pure Rust)

### 3.1 Choice: egui / eframe, with Slint as the fallback

Candidates considered: egui/eframe, Slint, iced, GPUI, Xilem/Masonry, Dioxus (webview). The UI is a dense editor: a 2,800-row tree with custom row painting, resizable columns, drag-and-drop reorder, inline editors overlaid on rows, floating popups, searchable dropdowns, secondary and always-on-top windows, and hundreds of small custom-styled controls. What decides it:

- **Custom painting is the common case here**, not the exception (row backgrounds, quantity suffixes, disclosure triangles, drag grips, gradient cells, drop indicators, rounded frames). egui is a painter-first toolkit: every one of those is a few lines of Rust. Slint pushes custom drawing through its component DSL (a second language, weaker for overlays and drag-and-drop); iced needs a custom widget type per case.
- **Immediate mode fits the data flow.** The route list becomes "draw the visible rows from the engine tree each frame"; there is no model/view synchronization layer to keep correct, which is exactly where today's latency comes from. egui only repaints on input, so idle cost is zero.
- **Multiple native windows** (quick-add popover, notification toast, undocked run summary, setup summary) are supported through eframe viewports.
- Mature for desktop tools, widely deployed, pure Rust end to end, AccessKit accessibility for free.
- Its known weakness is the one already accepted in D1: text is rasterized by egui itself (grayscale antialiasing, no ClearType). If the Phase 0 side-by-side is rejected on text quality, the fallback is Slint with the Skia renderer; GPUI (platform text via DirectWrite/CoreText) is the only candidate with native glyph rendering, but its API stability and documentation outside Zed make it a research option, not a plan.

### 3.2 How each Qt construct is reproduced

| Qt today | egui port |
|---|---|
| Application stylesheet (`theme.py`) | The same color-derivation functions produce an egui `Style`/`Visuals`; anything egui does not style the same way (scrollbars 10 px / 4 px radius / no arrows, tab bars, group boxes, menu items, tooltips, splitter handles) is drawn by app-owned widgets that read the same tokens |
| ~145 per-widget `setStyleSheet` strings | A `Look` struct per widget family; each Python string is ported to explicit fill/stroke/rounding/padding values, catalogued in a parity table with its source line |
| `QTreeView` route list with `QStandardItemModel`, `drawRow`, delegate | A custom virtualized table (`ScrollArea::show_rows` over the flattened visible rows): resizable/persisted column widths, 16 px indentation, GIF branch arrows as textures, checkbox column, row background precedence painted directly, quantity suffix with underline, inline quantity editor overlay, spacer row + inline creator overlay, floating `+` button, empty-state button, keyboard (Space, Q/W/E/R/T), right-click toggle without selection change, double-click inline edit, external-looking scrollbar |
| Drag and drop (`InternalMove` → controller) | egui drag-and-drop with a 3 px drop-indicator line; drop resolution rules (item → parent group, folder = append-into, non-folder = below) ported as-is |
| `QComboBox` + `QCompleter` (MatchContains, popup styling, Enter/Tab commit and focus advance) | An app-owned searchable dropdown widget: text field + popup list, first row pre-highlighted, best-match ordering (highlighted > exact > prefix > substring), Enter commits and advances focus, Tab/Backtab traversal in layout order, Enter on last field submits |
| `QSplitter` with persisted fractions | egui side panels with resizable width, fractions saved with the same 500 ms debounce and keys |
| `QTabWidget`, `QGroupBox`, `QScrollArea`, `QFrame` separators | App-owned tab bar (3 px tab radius, 2 px accent underline on the selected tab), group box with title in the primary color, `ScrollArea`, 1 px separators |
| `QMenuBar`/`QMenu`/`QAction` with configurable shortcuts | egui menu bar with the same menus, `aboutToShow`-style enable/disable logic, and a shortcut dispatcher that consumes key chords from the configured map (`Ctrl` chords are presented and handled as `Cmd` on macOS, as Qt does) |
| `QToolTip` + `QProxyStyle` delay override | Tooltip delay 0.1 s, grace 0, same colors and 3 × 6 px padding |
| `BaseDialog` (application-modal, centered, Escape closes) and `QMessageBox` | In-window modal layer with the same buttons, default button, Escape/Enter semantics; Qt draws these itself today, so an in-app modal matches the current look |
| `QFileDialog` | Native dialogs via `rfd` (visual deviation accepted) |
| `QColorDialog` | In-app color picker (visual deviation accepted) |
| `Qt.Popup` quick-add popover, toast window, undocked summary, setup summary | eframe viewports: frameless popup positioned above the hovered row and clamped to the screen with close-on-outside-click; always-on-top toast with 200 ms opacity animation; ordinary secondary windows |
| Rich-text `QLabel` (HTML tables in trainer cards, colored speed headers, kill-range lines) | Real layouts and `RichText` spans with the same text, colors and alignment |
| `QPropertyAnimation` (toast only) | `Context::animate_value_with_time` |
| Screenshot export (`grab`/`render`, transparent backgrounds, corner rounding) | Viewport screenshot cropped to the widget rectangle; transparent-background exports are rendered into an offscreen egui context and rasterized from the tessellated mesh, so the image content matches without a window grab |
| `QFontDatabase` / configured `custom_font_name` | System font lookup (`fontdb`): Segoe UI loaded from the OS on Windows; on macOS the same fallback Qt would take (the system UI font); the config value keeps working; bold/italic faces loaded as separate files |
| High-DPI | egui works in logical points with the OS scale factor, the same model as Qt 6; 9 pt = 12 px at 100 % |

### 3.3 What the Phase 0 spike must show

A throwaway eframe app rendering the route editor's left pane with the crystal route (real data through a JSON stub), the theme applied, a searchable dropdown, a modal dialog and a tooltip — next to the Python app, at 100 % and 150 %, on Windows and on a Mac. You judge text rendering and overall fidelity; the overlay tool reports geometry deltas. Pass → egui; fail on text only → Slint/Skia spike; fail on anything else → the issue is fixed in the spike before the UI phase starts.

---

## 4. Target architecture

### 4.1 Workspace layout

```
rust/
  Cargo.toml                 # workspace; release profile: lto = "fat", codegen-units = 1, strip = true
  crates/
    xpr-core/                # constants (utils/constants.py), config manager + per-platform paths (appdirs-compatible), rotating logging, sanitize_string, version parsing
    xpr-data/                # typed schemas + loaders for raw_pkmn_data (5 gens, 20 versions), custom gens, min_battles presets; validation with identical error text; embedded compressed data pack; lazy per-version registry (replaces gen_factory); ASM backport importer
    xpr-engine/              # StatBlock/BadgeList/Inventory/SoloPokemon/RouteState per gen, the 16 event definitions, RouteNode tree, Router, serialize/deserialize (byte-identical, all legacy shapes), undo snapshots, notes export
    xpr-calc/                # DamageRange, per-gen damage/crit/accuracy calc, kill-percentage search, BattleSummary model (the logic of battle_summary_controller.py), highlight strategies
    xpr-recorder/            # GameHook SignalR+REST client, Super Shuckie client, RecorderController, per-game FSMs (5), capture/replay format
    xpr-update/              # GitHub release check, download, exe/app swap, restart (Windows + macOS)
    xpr-golden/              # CLI: dump/verify/diff against the golden corpus; benchmarks; screenshot-overlay tool
    xpr-ui-kit/              # egui widget kit mirroring the Qt component set: theme tokens, searchable dropdown, amount entry, checkbox label, disclosure triangle, stepper group, tab bar, group box, modal layer, tooltip style, virtualized tree table
    xpr-app/                 # the application: main window, pages, route list, event details, editors, battle summary, dialogs, shortcuts, menus, viewports, startup sequencing
  build/                     # build.rs helpers: data pack (zstd), asset embedding
assets/pkmn_icons_scaled/    # pre-scaled icons generated ONCE by a PySide6 script (tools/prescale_icons.py) so pixels equal Qt's SmoothTransformation output; committed
```

Only `xpr-ui-kit` and `xpr-app` depend on egui. All other crates are plain Rust, testable headlessly, and are where the golden-corpus verification runs.

Two structural changes from the Python design, both invisible to the user:

- **No ambient globals.** Today `current_gen_info()`, `config` and `const` are module singletons read from deep inside the domain code, and `const` is mutated at runtime when the data directory changes. In Rust the game data is an `Arc<GenData>` carried by the `Router`/`RouteState`, and the three config keys that affect computation (`damage_search_depth`, `force_full_search`, `ignore_accuracy_in_damage_calcs`) are passed as a `CalcConfig` value.
- **One tree type.** `EventFolder`/`EventGroup`/`EventItem` share a de-facto interface with no base class and 52 `isinstance` sites. Rust uses `enum RouteNode { Folder, Group, Item }` with ids from an explicit counter owned by the `Router` (ids are still re-minted on load/undo, as today).

### 4.2 Data and persistence

- **Game data** stays JSON in `raw_pkmn_data/` as the editable source of truth (custom gens copy those flat files and users edit them). The build embeds a zstd-compressed pack; at runtime only the version needed by the current route is parsed with typed `serde` structs, deduplicated by dataset (Red/Blue share one parse). Validation runs with the same error messages. Each gen's loader has its own key names and quirks (gen 1's duplicated special stat, flavor as list-or-scalar, `"unused"` trainer filtering, TM/HM move-name derivation, fight categories as list-or-dict, gen 5 trainer alias tables including mojibake Nidoran names) and gets its own adapter. If parse time matters after measurement, precompile to `postcard`/`rkyv` in `build.rs`; keep JSON for custom gens.
- **Ordering is data.** Species/move/trainer order comes from JSON insertion order and flows into every dropdown; `MoveDB.stat_mod_moves` deliberately lists boosting moves before reductions. Use `IndexMap`/`Vec` + index, never `HashMap` iteration.
- **Route files**: same schema, same key order, `indent=4`, `", "`/`": "` separators, `ensure_ascii` escaping (every one of the 1,947 saved routes contains `\uXXXX` escapes), no floats present today. There is **no schema version field**; backward compatibility is shape-sniffing inside each deserializer (bare-string vitamin, `true` rare candy, 4-element learn-move list, old `hp/atk/def/spd/spc` DV keys, ability as index-or-name, `wild_pkmn_info` overriding `trainer_def`). Every branch is ported and exercised by the `outdated_routes/` corpus (3,260 backups of overwritten routes; `save()` moves the previous file there and `get_existing_route_path` falls back to it). Implemented with `serde_json` + `preserve_order` + a custom formatter; verified byte-for-byte.
- **Config**: same file per platform — Windows `%LOCALAPPDATA%\pkmn_xp_router\pkmn_xp_router\config.json`, macOS `~/Library/Application Support/pkmn_xp_router/config.json` (what `appdirs` produces there; do not use the `directories` crate's defaults) — same ~60 keys and defaults, `color_scheme_version` reset rule, shortcut overrides stored as diffs, unknown keys preserved, written atomically (temp + rename) and debounced (Appendix B item 15).
- **User data dir** (`user_data_location`, currently a Dropbox folder): `saved_routes/`, `outdated_routes/`, `custom_gens/`, `images/` — unchanged. Keep `read_json_file_safe`'s 2 s retry for cloud-placeholder files.
- **Route metadata index**: a sidecar (`saved_routes/.xpr_index.json`) mapping file → (size, mtime, version, species). Landing page and new-route page read only the index; a background thread stats the directory and re-parses only changed files. This replaces the three full parses at startup.
- **Assets**: `assets/theme/dark/*.gif` frames (checkboxes, branch arrows, combo arrow) become textures; `icons/**`, `assets/box_art/*.png`; the 650 Pokémon icons (75 MB) are only ever shown at 24, 28 and 72 px, so `tools/prescale_icons.py` (PySide6, run once, output committed) produces them with Qt's own `SmoothTransformation` and the port embeds ~2 MB. Filter icons are greyscaled by the same script instead of a per-pixel loop at startup.

### 4.3 Threading model (preserved semantics, safer primitives)

- The UI thread owns the engine state, as today.
- Recorder: a SignalR reader task, a Super Shuckie 10 Hz poller, and an FSM worker; the worker hands each event to the UI thread and **blocks until it is applied** (a channel + reply), preserving today's back-pressure where the FSM thread waits for the UI (`_blocking_dispatch`). Event queues are `mpsc` channels instead of a shared Python list. The UI thread drains the channel at the start of each frame and requests a repaint when work arrives.
- Update check and route-index rescans run on background threads and post results to the UI thread the same way.
- Damage math stays synchronous on the UI thread (target < 1 ms per fight). Existing debounce timers (candy 250 ms, vitamins 300 ms, searches 300 ms, delayed event save 2000 ms, battle-summary reveal 300 ms, splitter save 500 ms, toast 5 s) are kept because they are observable behavior; egui's `request_repaint_after` drives them without a busy loop.

### 4.4 Packaging (D6)

- **Windows:** `cargo build --release --target x86_64-pc-windows-msvc` → one `.exe`, assets and data embedded, no runtime dependencies, expected 15–25 MB. Release zip layout must satisfy the *old* Python updater so installed builds upgrade themselves: a zip whose top level contains exactly one `.exe`, asset name starting with `windows`, tag like `v6.0a` (the parser reads major, minor, and the last character as a bug-fix letter).
- **macOS:** universal binary (`x86_64-apple-darwin` + `aarch64-apple-darwin`, `lipo`) inside a `pkmn_xp_router.app` bundle with `Info.plist` and icon, shipped as one `macos_*.zip` (or `.dmg`). Code signing and notarization need an Apple Developer account; without them users must right-click → Open on first launch. Decide in Phase 0 whether to sign.
- **Updater:** the Rust updater keeps the GitHub flow (`releases/latest`, pick the asset for the running platform, download, swap, restart, cleanup on next start; any failure means "no update") and adds a `User-Agent` header (GitHub requires one; Python's `urllib` sends it implicitly). On Windows it renames the running exe to `.<name>.old` and copies the new one, exactly as today; on macOS it replaces the `.app` bundle next to the running one and relaunches with `open`.

### 4.5 Cross-platform notes (D7)

- Paths: `appdirs` semantics per platform (above); `user_data_location` default `~/Documents/pkmn_xp_router_data` on both.
- Shortcuts: the configured `Ctrl+…` chords map to `Cmd+…` on macOS (Qt's behavior); the shortcuts dialog shows the platform's names.
- Menus: drawn in-window on both platforms (the Windows Qt look is the reference). A native macOS menu bar (`muda`) is an optional later addition.
- Fonts: Segoe UI is loaded from the system on Windows; on macOS the app resolves the configured family through the system font database and falls back to the system UI font, which is what Qt does when the family is absent.
- Open-in-file-manager, clipboard, screenshots: cross-platform crates (`opener`, egui, viewport screenshots); the Windows virtual-desktop screen-grab math in `take_screenshot` is replaced by rendering the window content directly, which yields the same image.
- Recording: GameHook itself runs on Windows; the client only needs its HTTP/WebSocket endpoint, so recording from a Mac works when GameHook runs on a Windows machine on the network (URL becomes configurable, default unchanged).
- CI: GitHub Actions matrix (windows-latest, macos-latest) runs `cargo test`, the golden verification, and produces both release artifacts.

---

## 5. Fidelity strategy (how we prove "exactly")

### 5.1 Golden corpus (built in Phase 0, in Python, before any Rust)

Your data directory is the test suite: 1,947 saved routes, 3,260 outdated routes, 4 custom gens, 4 test routes, across all 20 versions. A Python tool (`docs/rust_port/golden/dump.py`) runs the current app headlessly over each file and writes JSON-lines records:

- `route`: re-serialized route JSON (byte-exact), notes export text, final state.
- `event` (one per group and per event item): rendered name, pre/post state (every field any viewer displays), error flags/messages, tags, `do_render` results for each filter type and a fixed set of search strings, the route-list columns.
- `battle` (one per trainer/wild event): the full `BattleSummaryController` output — every `MoveRenderInfo`/`PkmnRenderInfo` field, best moves, mimic options, per-matchup modifiers and field statuses — under the default config and under each highlight strategy, two thresholds, two search depths, and `force_full_search`/`ignore_accuracy` on and off; plus run-summary and setup-summary text.
- `undo`: for 200 sampled routes, a scripted sequence of edits (add/delete/move/transfer/split/toggle/highlight/replace/vitamin-adjust) recording state after each step and after each undo.
- `data`: for every version and custom gen, the validated database exactly as loaded (all species, trainers, items, moves, type chart, fight info, derived table order).

Rust `xpr-golden verify` replays all of it and diffs. Exit criterion for the engine phases is zero diffs. The Python dump takes ~1–2 h (one-time, incremental afterwards).

### 5.2 Python-semantics checklist (applied during translation, enforced by tests)

- `round()` is banker's rounding (3 call sites: `damage_calc.py:352`, `gen_4/pkmn_damage_calc.py:538`, `universal_utils.py:133`); `f"{x:.1f}"` also rounds half-even. Rust `f64::round` rounds half away from zero; use `round_ties_even` and a matching formatter.
- The stat and damage paths use `math.floor(a / b)` on floats, not `//`. Operands are non-negative in practice, so `i64` floor division is exact; translate the fractional multipliers (`* 1.1`, `* 0.9`, `* 1.125`, `* 1.3`, `* 3277/4096`…) to exact rational integer math and prove equality on the corpus. Use `div_euclid`/`rem_euclid` anywhere an operand can be negative (Trick Room `(10000 - speed) % 8192`).
- `int(x)` truncates toward zero (`percent_xp_to_next_level`); `math.floor` does not.
- `min(ceil(sqrt(stat_xp)), 255)` uses IEEE `sqrt`; integer `isqrt` with a ceiling adjustment gives identical results.
- `json.dump`: `ensure_ascii=True`, `indent=4`, separators, insertion-order keys; `json.load` accepts duplicate keys (last wins).
- Python ints are unbounded; use `i64` and add overflow tests around EXP/money accumulators.
- `dict` iteration = insertion order; `set` iteration is arbitrary — pin an order for `defeated_trainers` and any other set that reaches output.
- Strings: `sanitize_string` (keep alphanumerics, lowercase) for species/item/move lookups, but **raw case-sensitive** names for trainers, fight rewards and badges; `ItemDB`/`MoveDB` fall back to a case-insensitive linear scan on a miss; `custom_move_data` is matched by substring in most branches and by equality in some gen-5 branches; `.lower()` on Unicode; truthiness of `""`/`None`/`0`.
- Heterogeneous returns become explicit types: `""`-or-int display accessors, `None` accuracy = always hits, `None` damage = no row, `-1.0` kill percent = guaranteed-but-unquantified.
- Exceptions as control flow: `RouteState` mutators try strictly, catch, record the message and retry with `force=True`; `@handle_exceptions` turns controller exceptions into UI messages. Model each as `Result` plus an explicit warning channel, keeping the user-visible text. (The callback-dropping behavior is fixed per Appendix B item 8.)
- Aliasing that is load-bearing: `copy.copy` of `EnemyPkmn` shares `StatBlock`s across a team; `EventItem.apply` writes the chosen destination back into the event definition; `StageModifiers.apply_stat_mod` returns `self` when a boost is a no-op. Decide clone-vs-share per site and cover with corpus records. (Gen 5's in-place stat mutation is fixed first per Appendix B item 14.)
- Gen-specific arithmetic that must be kept literally: EV caps applied in RAM order (hp, atk, def, **spe**, spa, spd); gen 1 badge-boost stacking counts; gen 2 Glacier-badge Sp.Def glitch window; gen 1 `>255` stat scaling with `& 0xFF`; type effectiveness in ROM row order with a zero check per row; gen 2 Gold/Silver Present formula; Crystal vs GS `TruncateHL_BC`; wild mons computed at 0 and 15 DVs and merged.
- Undo snapshots stringify tuple keys and parse them back with `ast.literal_eval`; since snapshots are never persisted, the Rust representation may differ as long as observable undo results match the corpus.

### 5.3 Visual parity harness (near-identical standard, §2)

- Reference set: the Python app driven by a script into ~60 named states (landing page with N routes, new-route page per gen, route editor with the crystal route at several scroll/selection/filter/expand states, each event editor, battle summary for wild/single/double/transformed/test-move cases, each dialog, popover, toast, docked/undocked summary, status chips, recording states), grabbed with `QWidget::grab()` at 100 % and 150 % on Windows (Qt 6.10.2, Segoe UI). The same script also records widget geometry (`geometry()` of every named widget) as JSON.
- The Rust app is driven into the same states by `xpr-golden screenshots`, which also emits the geometry of every named widget.
- `xpr-golden overlay` compares: geometry deltas (fail above 1 px), per-region color histograms (fail on any missing token color), and a perceptual diff heatmap for you to review. Text regions are compared by geometry and color only.
- Sign-off: each screen is accepted by you from the side-by-side; the acceptance is recorded in `docs/rust_port/parity/`. Later changes that move a screen out of tolerance fail CI.

### 5.4 Recorder parity

- Add a capture hook to the Python `GameHookClient` (Phase 0) that logs every property change with timestamps to JSONL, and record real sessions for Yellow, Crystal, Emerald, Platinum and Black covering encounters, trainer wins and losses, level-ups with moves, evolutions, purchases/sales, vending machines, vitamins, candies, TMs, held items, heals, saves, resets and blackouts. This is the only part of the plan that needs your hands on an emulator.
- Both FSM implementations replay the captures; the emitted event sequences (with `recorded_time` stamps) must match. Unit tests also cover SignalR framing (negotiate → websocket → `{"protocol":"json","version":1}\x1e`, `0x1e`-delimited JSON, ping type 6), both `PropertiesChanged` argument shapes, case-insensitive path resolution, the 60 s mapper refresh, and the disconnect sentinel.

### 5.5 Test suite

The 328 existing tests are ported one-to-one as Rust unit tests. `TestPlatinumChimcharRoute::test_final_money` is rewritten to assert the current value (3,240) and annotated with KI-1 so the deferred question stays visible (Appendix E).

### 5.6 Bug fixes and the oracle (D4)

The golden corpus is a recording of the Python app, so every approved fix that changes engine output must land in Python **before** the corpus is generated; otherwise the Rust port would be required to reproduce the bug. Procedure:

1. Each Appendix B item gets a disposition: *Python-first* (changes engine/serialized output), *Rust-only* (UI, recorder, packaging), or *deferred* (KI-1).
2. Python-first fixes are made on `ui` before the baseline tag, each with a unit test; the corpus is generated after them.
3. Rust-only fixes are implemented directly in the port and listed in the release notes; the reference screenshots or capture replays that they affect are annotated.
4. Any bug found *during* the port is triaged the same way; if it is engine-affecting, the fix goes into Python, the corpus is regenerated for the affected records, and the Rust side follows. This is the only reason to touch Python after the tag.

---

## 6. Phases

Effort is in focused weeks for one developer working with AI-assisted coding; ranges reflect the unknowns called out per phase. Phases 2 and 4 can overlap once the engine's public types are frozen.

### Phase 0 — Baseline, fixes, freeze, spike (2–3 weeks)

1. Appendix B dispositions confirmed; Python-first fixes (items 3, 4, 14; item 2 if it reaches any output) made on `ui` with tests; KI-1 annotated.
2. `ui` tagged `rust-port-baseline`; freeze protocol in force (D3).
3. Golden dump tool and first full corpus (§5.1), stored outside the repo (several GB).
4. Recorder capture hook and first captures (§5.4).
5. Reference screenshot and geometry set from the Python app (§5.3); `tools/prescale_icons.py` run and its output committed.
6. **The spike** (§3.3) on Windows and macOS. Go/no-go on egui; fallback spike on Slint if text rendering is rejected.
7. Rust toolchain targets added (`x86_64-pc-windows-msvc`, `x86_64-apple-darwin`, `aarch64-apple-darwin`); CI matrix skeleton; decision on macOS signing.

Exit: spike accepted; corpus and captures exist; baseline tagged after fixes.

### Phase 1 — Data layer (`xpr-core`, `xpr-data`) (2 weeks)

- Constants, config manager (all keys, defaults, validation in getters), per-platform paths, rotating log (20 backups, rollover at start, tolerate a locked file).
- Typed loaders for the five generation schemas and their quirks; validation passes with identical messages; custom-gen metadata, base-gen resolution, aggregated error text, `create_new_custom_gen` file copy; ASM backport importer (macro extraction, positional `db` parsing, `difflib` 0.8-cutoff fuzzy move matching — port `get_close_matches` faithfully).
- Embedded data pack + lazy registry; `MinBattlesDB` directory listing (gens 1–2 only).
- Exit: `data` golden records match for all 20 versions and all custom gens; loader benchmarks recorded.

### Phase 2 — Engine and calc (`xpr-engine`, `xpr-calc`) (5–7 weeks)

- Six EXP curves and level lookup, XP yield with its three separate floors, stat blocks per gen (formulas, badge boosts, stage tables, natures, items, Power Trick/Tailwind/Trick Room, Macho Brace), badge lists, inventory (bag limit, money, marts, key items, sell price, `force` semantics), solo Pokémon with the realized/unrealized stat-EXP model, vitamins and EV berries, evolution rules, per-gen `GenData` trait.
- The 16 event types with all serialized shapes, rendering strings, `do_render` filter semantics, tags with the priority order, highlight numbers, recorded/split time, exp-split/mon-order interleaving for multi-battles, custom move data, weather/screens/intimidate/stat-stage setup, `pkmn_after_levelups`.
- `Router`: tree ops (insert/remove/move/transfer/split/adjacent folder/rename/purge, unique folder names with `Trip N`), full `_recalc` including the double-apply for mid-battle level-up moves and evolution move injection, defeated-trainer tracking (gen ≥ 3 treats everything as refightable), test moves, load/save/new/base-route templates, notes export, `restore_events_from_state` and the 15-step undo manager.
- Damage calc for gens 1–5 (~2,900 Python lines with per-move special cases), crit/accuracy, `DamageRange` joint distributions, kill-percentage search with memoization and the `> 200 rolls` guard, and the whole battle-summary model (strategies, consistent threshold, mimic, struggle, transform, test moves, per-matchup stage/field simulation, vitamin shadows, prefight candies, transient-state save/restore).
- Exit: zero diffs on `route`, `event`, `battle`, `undo` records; all 328 ported tests green; `cargo bench` shows full recalc < 5 ms and a 6-mon fight < 1 ms.

### Phase 3 — I/O subsystems (`xpr-recorder`, `xpr-update`) (2–3 weeks)

- GameHook client (SignalR framing on `tokio-tungstenite`; REST via `reqwest`), property store, change/once callbacks, mapper auto-refresh, case-insensitive constant validation with the same user-facing error text and status strings; configurable base URL.
- Super Shuckie poller, staleness rule, `H:MM:SS.CC` truncating formatter.
- `RecorderController` semantics (folder naming, game-reset rollback to the last save, loss rollback within 20 events, final-trainer auto-stop, item/vitamin/candy coalescing keeping the original timestamp).
- Five FSMs. Gens 3–5 are near-verbatim copies differing mainly in property paths and bag pockets, so implement one generic machine parameterized by a per-game table; gens 1–2 keep their specific logic (audio-channel save/heal detection, deprecated-mapper path sets, trainer name disambiguation tables, starter-modulo rival naming). Missing mapper paths are skipped with a warning on every gen (Appendix B item 13).
- Updater for both platforms and the small upgrade-progress window.
- Exit: capture replays match; protocol unit tests green; a live smoke test against GameHook for at least Yellow.

### Phase 4 — UI (`xpr-ui-kit`, `xpr-app`) (8–12 weeks)

Order chosen so the app is usable early and the riskiest widgets come first:
1. **Widget kit and theme:** tokens from the config colors through the `theme.py` math; the parity catalogue of every `setStyleSheet`/`setFixedSize`/font call in `gui_qt/` with its egui equivalent; searchable dropdown, amount entry with +/− buttons, checkbox label, disclosure triangle, stepper group, tab bar, group box, separators, scrollbars, modal layer, tooltip, menu bar and shortcut dispatcher (defaults, overrides, live rebinding, export/import, the text-field-focus guard and click-to-unfocus behavior), window geometry persistence (tkinter-format `WxH+X+Y` string and `zoomed`).
2. **Route list** (§3.2 row): virtualized tree table with all behaviors, drag-and-drop, inline creator and quantity editor overlays, `+` button, empty-state button.
3. Filter bar, route search, message label, status bar chips and record controls, recorder status.
4. Event details pane: tabs and auto-switch rules, state viewer, stat columns, stat-EXP/badge/inventory viewers, all editors with delayed-save semantics, notes editor and visibility modes, pre-save hook.
5. Battle summary: stepper groups, legacy controls, six matchup cards with 8 (+4 test) damage columns, highlight/fade coloring rules, speed-colored headers, matchup drag reorder, intimidate/weather/screen toggles, export paths including corner rounding.
6. Landing page (index-backed), new-route page (game table with box art, DV frame, natures/abilities, base-route list), inline event creator (focus choreography), quick-add popover viewport.
7. Dialogs (14), secondary windows (run summary docked/undocked with gradient cells, setup summary), notification toast viewport, custom-gen flows, screenshot helpers.
8. Startup sequencing: first frame before any I/O; index scan, update check and custom-gen load in the background; lazy construction of pages not yet shown.

Exit: every screen signed off in the overlay suite on Windows and macOS; every menu/shortcut/dialog exercised by a scripted UI test; interaction targets in §0 met on the crystal route.

### Phase 5 — Packaging, compatibility, cutover (2 weeks)

- Windows exe and macOS universal `.app`; release zips named for the old updater (`windows_*.zip` with one exe) and the new one (`macos_*.zip`); version `v6.0a`.
- Upgrade rehearsal on Windows: install the last Python release, let it auto-update into the Rust exe, confirm config, saved routes, outdated routes, custom gens and images all work; confirm the Rust updater can update itself again on both platforms.
- Parallel-run period: both apps installed; every route saved in the Rust app is verified by `xpr-golden verify --live` against the Python engine.
- Cutover: Rust becomes the release; the Python code stays in the repo (tagged) as the reference implementation and golden generator.

### Phase 6 — Performance hardening (1–2 weeks reserved)

- Incremental recalculation from the edited node forward. The engine always recalculates from the root today, and the double-apply, defeated-trainer bookkeeping and level-up move registration make incremental recompute subtle, so it is done after parity is proven and validated against the corpus.
- Profiling with the crystal/yellow/emerald routes; frame-time budget enforcement (< 16 ms for every interaction in §1.2).

**Total: roughly 20–29 weeks** of focused work. Phases 2 and 4 are the bulk and can overlap once the engine API is frozen (about two weeks into Phase 2).

---

## 7. Performance design (what specifically makes it fast)

| Today | Port |
|---|---|
| Parse all 1,947 route files three times at startup | Metadata index keyed by (size, mtime); background rescan; landing page renders from the index in < 20 ms |
| Construct all 20 game versions at import (1.1 s, duplicated per shared dataset) | Lazy: parse only the version the current route needs from an embedded compressed pack (< 50 ms; < 10 ms if precompiled) |
| Import network stack at startup (0.45 s) | Recorder/updater code is linked but idle until used |
| 173 MB onefile exe extracted to temp each launch (~3 s) | Native exe/app, assets embedded, nothing extracted; eframe window up in ~100 ms |
| Build every page and 48 damage widgets before first paint (2.5 s) | Immediate mode: only the visible page is laid out, every frame, from state |
| Route list: full upsert of every `QStandardItem` on every change; three tree walks; per-row computed properties; model mutation inside `drawRow` | Visible rows only (`show_rows`), drawn straight from the engine tree; per-column strings computed on demand; no retained model to synchronize |
| Full recalc from root on every edit (50 ms Python), each step allocating a new `SoloPokemon` and re-indexing the inventory | ~1–3 ms in Rust with arena-allocated states; later incremental (Phase 6) |
| Battle summary: 91 stylesheet rebuilds per refresh; 96–144 damage calcs + 96 kill searches per 6-mon fight in Python | Compute < 1 ms; drawing 48 columns is a few thousand rectangles and text runs per frame |
| Undo: full serialize twice per edit, full deserialize + recalc on undo | Snapshot is a cheap structural clone; restore re-mints ids and recalculates exactly as today |
| Config file rewritten on every setter | Debounced atomic writes |
| Filter icons greyscaled with a per-pixel loop at startup | Done once by the prescale script |
| `ItemDB`/`MoveDB` miss = linear case-insensitive scan | Precomputed case-folded index with identical resolution order |
| Idle CPU: Qt event loop | egui repaints only on input or timers |

Targets are in §0, verified by `xpr-golden bench` and the interaction script ported to the Rust app.

---

## 8. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Text rendering difference (grayscale vs ClearType) is rejected after all | Toolkit change late | The Phase 0 spike is judged by you on both platforms before any UI code is written; Slint/Skia fallback spike prepared |
| Visual drift from the 145 per-widget stylesheet strings and hand-measured geometry (`fm.horizontalAdvance`, `label_width * 8` heuristics) | Screens differ by a few pixels | Parity catalogue in Phase 4 step 1; geometry deltas fail CI above 1 px; per-screen sign-off |
| egui gaps: multi-viewport quirks per platform, IME/text-input edge cases, no native color dialog, offscreen rendering for transparent exports | A behavior needs a workaround | Spike covers viewports on both OSes; accepted deviations listed in §2; offscreen rasterization prototyped in Phase 4 step 1 |
| macOS packaging: signing/notarization, Gatekeeper, universal build | First-launch friction on Mac | Decide on an Apple Developer account in Phase 0; document the right-click → Open path otherwise |
| Font availability: Segoe UI absent on macOS; configured family missing | Different metrics on Mac | System-font resolution with Qt-equivalent fallback; geometry suite runs on both platforms |
| Moving target: two active branches, 50+ commits per quarter | Port never converges | D2/D3: tag, freeze, mirror rule; golden corpus regenerated on every approved Python change |
| Bug fixes invalidating the oracle | Rust reproduces bugs, or diffs are dismissed by habit | §5.6: engine fixes go into Python first, then the corpus is regenerated; diffs are never waived |
| Python semantics (banker's rounding, floor division, float repr, set order, `ensure_ascii`, aliasing) | Silent numeric/text divergence | §5.2 checklist; the corpus catches every case that appears in 5,000 real routes; property tests for the rest |
| SignalR framing and mapper-path fidelity; no captures exist today | Recording breaks silently | Capture hook first (Phase 0); replay tests; live smoke test per game before cutover |
| Undo/serialization subtleties (`str(tuple)` keys, re-minted ids) | Undo diverges | Corpus `undo` records; internal representation free to change |
| GitHub API `User-Agent`, unauthenticated rate limit (60/h) | Update check fails | Set header; treat any failure as "no update" exactly like today |
| Config path mismatch per platform | Existing config ignored | Hard-code the `appdirs` paths for Windows and macOS |
| Corpus verification time (5,200 routes) | Slow CI | Incremental verification keyed by file hash; `rayon` parallelism |

---

## 9. Remaining small decisions (settled in Phase 0 unless you say otherwise)

- eframe rendering backend: `glow` (smaller, faster start) by default; `wgpu` if a platform driver issue appears.
- Apple Developer account for signing/notarization: recommended if Mac users are expected beyond yourself.
- Native macOS menu bar (`muda`) versus the in-window menu bar: in-window first, native later if wanted.
- In-app color picker design: reproduce `QColorDialog`'s layout or use egui's picker; egui's picker is the default.

---

## Appendix A — Module map

| Python (branch `ui`) | Lines | Rust home | Notes |
|---|---|---|---|
| `utils/constants.py` | 739 | `xpr-core::consts` | ~350 attributes: event type names, JSON keys, colors, timing defaults, recording status strings, Super Shuckie URL |
| `utils/config_manager.py` | 913 | `xpr-core::config` | ~60 keys, ~100 default shortcuts, color scheme version reset |
| `utils/io_utils.py`, `custom_logging.py`, `setup.py` | 351 | `xpr-core` | `sanitize_string`, safe JSON read with retry, backups to `outdated_routes/`, route name/path helpers, data-dir migration, rotating log |
| `utils/backport_parser.py` | 422 | `xpr-data::backport` | ASM backport importer for custom gens (not a route upgrader) |
| `utils/auto_update.py` + `gui/auto_upgrade_window.py` | 283 | `xpr-update` + `xpr-app::upgrade_window` | Version parsing `vX.Ya`, GitHub asset selection per platform, exe/app swap, restart |
| `pkmn/pkmn_db.py`, `pkmn_info.py`, `gen_factory.py`, `universal_data_objects.py`, `universal_utils.py` | 1,446 | `xpr-data` + `xpr-engine::model` | DBs, `GenData` trait, species/trainer/item/move types, natures, stage modifiers, field status, EXP curves |
| `pkmn/gen_{1..5}/data_objects.py`, `gen_*_object.py`, `gen_*_constants.py` | 6,900 | `xpr-data::gen{1..5}` + `xpr-engine::gen{1..5}` | Stat blocks, badge lists, loaders, per-gen constants, custom-move-data tables, gen 5 alias tables |
| `pkmn/damage_calc.py`, `pkmn/gen_*/pkmn_damage_calc.py`, `pkmn_utils.py` | 3,268 | `xpr-calc` | Ranges, crit/accuracy, per-gen formulas and move special cases, kill search |
| `routing/route_events.py` | 1,432 | `xpr-engine::events` | 16 event types, all serialized shapes, rendering strings, tags, `do_render` |
| `routing/router.py`, `state_objects.py`, `full_route_state.py` | 1,536 | `xpr-engine::{router,state}` | Tree ops, recalc, save/load, inventory, solo Pokémon, badges, error protocol |
| `controllers/main_controller.py`, `undo_manager.py` | 1,016 | `xpr-app::controller` (thin) + `xpr-engine::undo` | Callback bus → app events; selection/preview/filter/search state; screenshots |
| `controllers/battle_summary_controller.py` | 2,289 | `xpr-calc::battle_summary` | Pure logic; view-model consumed by the battle summary screen |
| `route_recording/gamehook_client.py`, `supershuckie_client.py`, `recorder.py` | 1,005 | `xpr-recorder::{gamehook,shuckie,controller}` | See Appendix C |
| `route_recording/game_recorders/**` | 11,855 | `xpr-recorder::games::{gen1..gen5}` | One generic gen 3–5 machine + tables; gen 1/2 specific logic; `MAPPER_GAPS.md` for gen 5 |
| `gui_qt/theme.py` | 611 | `xpr-ui-kit::theme` | Same derivation math → egui `Style`/`Visuals` + app-drawn widget looks |
| `gui_qt/components/custom_components.py`, `filter_toggle_bar.py`, `route_search.py`, `recorder_status.py` | 1,003 | `xpr-ui-kit` | Searchable dropdown, entries, amount entry, checkbox label, disclosure triangle, toast, filter bar |
| `gui_qt/main_window.py`, `event_details.py`, `event_editors.py` | 3,947 | `xpr-app::{main_window,event_details,editors}` | |
| `gui_qt/pkmn_components/route_list.py` | 2,022 | `xpr-app::route_list` | Virtualized tree table (§3.2, §7) |
| `gui_qt/pkmn_components/*` (others) | 1,309 | `xpr-app::state_views` | Stat columns, viewers, custom DVs frame |
| `gui_qt/battle_summary/battle_summary.py` | 2,995 | `xpr-app::battle_summary` | |
| `gui_qt/components/inline_event_creator.py`, `quick_add_*.py`, `quick_*.py` | 2,107 | `xpr-app::{inline_creator,quick_add}` | Focus choreography, popover viewport |
| `gui_qt/pages/*`, `dialogs/*`, `secondary_windows/*`, `box_art.py`, `pkmn_icon.py` | 4,296 | `xpr-app::{pages,dialogs,windows,assets}` | |
| `main.pyw` | 79 | `xpr-app::main` | Startup sequence, tooltip timing, restart-after-update |
| `tests/*` | 3,174 | `cargo test` across crates | 328 tests; KI-1 annotated |
| `webserver/*`, `gui/*` (rest), `scripts/*`, `calculate_speed_tiers.py`, `scarlet_violet.js` | ~7,700 | — | Dropped (D5) |

## Appendix B — Bug fix list (D4) with dispositions

| # | Bug | Disposition |
|---|---|---|
| 1 | `TestPlatinumChimcharRoute::test_final_money` expects 3,060; code produces 3,240 after the gen-4 `money * 4` change | **Deferred — KI-1** (Appendix E). Test rewritten to the current value with a note. |
| 2 | `SoloPokemon.serialize` writes the `xp` and `xp_to_next_level` keys twice, so the second value silently wins | Rust-only (not part of saved routes); Python-first if the corpus generator finds it in any output |
| 3 | `Router.replace_event_group` raises for a level-up move when the key *already* exists (inverted check) | Python-first, with a test, before the corpus |
| 4 | `Router.add_event_object` checks the first trainer's `refightable` flag when recording the second trainer as defeated | Python-first, with a test, before the corpus |
| 5 | `EventFolder.contains_id` references a field folders do not have | Rust-only (the enum makes it impossible) |
| 6 | Highlight "Reset to Defaults" writes a different palette than `Config.get_highlight_color`'s defaults | Rust-only: one source of defaults |
| 7 | Font change in the color dialog calls a method that does not exist on the Qt window; applies on next launch only | Rust-only: apply live |
| 8 | `MainController._safely_invoke_callbacks` permanently removes any callback that raises once | Rust-only: errors are reported, subscriptions stay |
| 9 | `RecorderController` callback dispatch logs "removing" but never removes | Rust-only (same design as 8) |
| 10 | `GameHookClient.unignore_properties` calls a misspelled method | Rust-only |
| 11 | Gen 1 `UninitializedState` compares a property object to a string (always false) | Rust-only; confirm intended behavior against a Yellow capture |
| 12 | No socket-level reconnect or backoff in the GameHook client | Rust-only: automatic reconnect with backoff, manual button kept |
| 13 | Gen 1–4 recorders would crash on a missing mapper path; only gen 5 skips `None`/unmapped paths | Rust-only: all gens skip with a warning |
| 14 | Gen 5's damage calc mutates caller-supplied battle stats in place where gens 2–4 copy them | Python-first, with a test, before the corpus (may change gen 5 outputs) |
| 15 | Every config setter rewrites the whole config file | Rust-only: debounced atomic writes |
| 16 | The PyInstaller build bundles `.claude/*.json` and `.harness/**/*.json` | Moot (new build) |
| 17 | Documented damage-calc divergences from the games (`docs/damage_calc_review/*_findings.md`) | Out of scope for the port; continue as a separate workstream in Rust after cutover with new tests |

## Appendix C — External contracts the port must match

| Contract | Shape | Rust |
|---|---|---|
| GameHook SignalR hub `ws://localhost:8085/updates` (configurable) | negotiate → websocket → `{"protocol":"json","version":1}\x1e`; frames `0x1e`-terminated JSON; methods `PropertiesChanged` (list of dicts), `PropertyChanged` (positional list), `MapperLoaded`, `GameHookError`, `DriverError`, `SendDriverRecovered`, `UiConfigurationChanged`; ping `{"type":6}` | `tokio-tungstenite` + `serde_json`, hand-written framing |
| GameHook REST | `GET /mapper` → `{meta, glossary, properties[]}`; `PUT /mapper/properties/{path with / for .}` body `{"bytes":[...],"freeze":bool}`; 60 s auto-refresh | `reqwest` |
| Mapper property paths | 70–700 dotted paths per game/mapper variant; `"Deprecated"` in `meta.gameName` selects legacy paths for gens 1–2; case-insensitive fallback | static tables |
| Super Shuckie | `GET http://127.0.0.1:30158/stats` → `{"time_current": ms|null}`; 1 s timeout; 10 Hz; 2 s staleness; `H:MM:SS.CC` truncated | `reqwest` |
| GitHub releases | `GET /repos/Scotts-Thoughts/pkmn_yellow_xp_router/releases/latest`; Windows asset named `windows*` (zip with one `.exe`, required by the old updater); macOS asset named `macos*` | `reqwest` with `User-Agent` |
| `config.json` | Windows `%LOCALAPPDATA%\pkmn_xp_router\pkmn_xp_router\config.json`; macOS `~/Library/Application Support/pkmn_xp_router/config.json`; `indent=4` | `serde_json` |
| Route JSON | `routing/router.py::serialize` schema; `indent=4`; `ensure_ascii`; all legacy shapes; `recorded_time`/`split_time` strings | `serde_json` + custom formatter |
| `raw_pkmn_data/**/*.json`, custom gens | per-gen schemas | `serde` typed |

## Appendix D — Reproducing the baseline

```
py -3.14 docs/rust_port/bench/bench_route.py      # engine: load / recalc / save on the three largest routes
py -3.14 docs/rust_port/bench/bench_bsc.py        # battle summary compute per fight
py -3.14 docs/rust_port/bench/bench_gui.py        # startup stages to first event-loop pass
py -3.14 docs/rust_port/bench/prof_gui.py         # cProfile of MainWindow() and deferred init
py -3.14 docs/rust_port/bench/bench_interact.py   # end-to-end GUI latency for load/select/edit/undo/search
py -3.14 -m pytest -q                              # 328 tests, 1 known failure (KI-1)
```
The scripts read `user_data_location` from the app's `config.json` and pick the three largest routes in `saved_routes/`; pass a directory as the first argument to override.

## Appendix E — Known issues carried into the port

| Id | Issue | Where it is recorded |
|---|---|---|
| KI-1 | Gen-4 prize money: the route engine multiplies trainer money by 4 (`gen_four_object.py:503`); the Platinum Chimchar regression route expects 3,060 final money and the engine produces 3,240. Long-standing; whether the multiplier or the expectation is wrong is not resolved by the port. The port reproduces 3,240. | `xpr-engine::gen4` doc comment, the ported test (asserts 3,240, tagged `KI-1`), `docs/rust_port/KNOWN_ISSUES.md`, release notes |
