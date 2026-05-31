# iOS App Port — Comprehensive Plan

## Core Architectural Decision: BeeWare (Briefcase + Toga)

The single-codebase requirement is the most important constraint. The recommendation is **BeeWare**, which lets Python run natively on iOS via Briefcase's build toolchain and a thin Objective-C bridge. This means:

- `routing/`, `pkmn/`, `controllers/`, `utils/`, and `raw_pkmn_data/` — all the business logic and Pokemon data — are shared and written **once**.
- Desktop UI (`gui_qt/`) stays entirely untouched.
- A new `gui_mobile/` directory holds the iOS-specific UI, also in Python/Toga.
- When you add a new game, a new event type, or change a damage calculation, you edit the core once. Both apps pick it up.
- The only time you touch two files is when you change visible UI behavior (a new editor dialog, a new stat display), since desktop and mobile layouts are inherently different.

> **Alternative considered:** Native SwiftUI. Best native look/feel, but requires rewriting all ~20K LOC of business logic in Swift. Ruled out because it severs the shared logic requirement entirely.

---

## Repository Structure (Additive — No Breaking Changes)

The existing structure stays intact. You add alongside it:

```
pkmn_yellow_xp_router/
├── routing/              # SHARED — no changes
├── pkmn/                 # SHARED — no changes
├── controllers/          # SHARED — no changes
├── utils/                # SHARED — no changes (minor iOS path adjustments)
├── raw_pkmn_data/        # SHARED — bundled into iOS app via Briefcase
├── assets/pkmn_icons/    # SHARED — bundled into iOS app
│
├── gui_qt/               # DESKTOP ONLY — no changes
├── main.pyw              # DESKTOP ENTRY — no changes
├── route_recording/      # DESKTOP ONLY — excluded from iOS build
├── webserver/            # DESKTOP ONLY — excluded from iOS build
│
├── gui_mobile/           # NEW — iOS Toga UI
│   ├── app.py            # Toga App subclass
│   ├── screens/
│   │   ├── home.py           # Landing, recent routes, open/create
│   │   ├── new_route.py      # Game/version/starter/DVs/nature setup
│   │   ├── route_editor.py   # Main editing screen (tab container)
│   │   ├── event_list.py     # Scrollable event tree (the route)
│   │   ├── event_detail.py   # Per-event viewer/editor (bottom sheet)
│   │   ├── add_event.py      # New event picker and form
│   │   ├── pkmn_state.py     # Stats, XP, badges, inventory
│   │   ├── battle_calc.py    # Damage calculator
│   │   └── settings.py       # App settings
│   ├── components/
│   │   ├── event_row.py      # Single event row (compact, swipeable)
│   │   ├── stat_card.py      # Compact stat display card
│   │   ├── folder_row.py     # Collapsible folder row
│   │   └── badge_row.py      # Badge display
│   └── theme.py              # Mobile color/font constants
│
├── main_mobile.py        # NEW — Briefcase/Toga iOS entry point
├── pyproject.toml        # NEW — Briefcase build config (replaces/supplements spec)
└── requirements.txt      # Updated (add briefcase, toga; remove PySide6/signalrcore for mobile build)
```

---

## Phase 1: Tooling & Core Compatibility (1–2 weeks)

**Goal:** Prove the Python core runs on iOS before building any UI.

1. Add `briefcase` to dev dependencies. Create `pyproject.toml` with:
   - `app_id`, `bundle`, `version` metadata
   - Two targets defined: `desktop` (existing) and `ios`
   - iOS target excludes `route_recording/`, `webserver/`, `gui_qt/`, PySide6, signalrcore
   - Includes `raw_pkmn_data/**`, `assets/pkmn_icons/**` as data files

2. **iOS file system adaptation** — the only core file requiring attention. iOS apps live in a sandboxed container; there is no `~/.config`. The change is isolated to `utils/io_utils.py`:
   - Detect when running on iOS (`sys.platform == 'ios'`)
   - Use `os.environ['HOME']` + `/Documents/` on iOS instead of appdirs
   - All other path logic stays the same

3. Strip iOS-incompatible dependencies: PIL `ImageGrab` (screenshot), signalrcore, Flask. These are already only imported in excluded modules, so no core changes needed.

4. Run `briefcase create ios` + `briefcase build ios`. Fix any import issues that surface. Run the existing test suite against the core to verify correctness is preserved.

---

## Phase 2: iOS UI — Screen-by-Screen Design (4–6 weeks)

The desktop UI is a wide multi-panel layout unsuitable for mobile. The mobile redesign uses standard iOS navigation patterns.

### Navigation Model

```
Tab Bar (bottom):
  [Route]  [Battle Calc]  [Settings]

Route Tab:
  Home → New Route Setup → Route Editor
                            ├── Events Tab  ← primary view
                            ├── Stats Tab
                            └── Summary
```

### Screen: Home
- List of recent routes (stored in app Documents)
- "Create New Route" button (prominent, top)
- "Open File…" button → iOS Files picker (opens routes from iCloud, email attachments, etc.)
- Tap a route → opens Route Editor

### Screen: New Route Setup
- Scrollable form matching the desktop New Route page
- Game/version picker (Selection widget, same game list as desktop)
- Starter Pokemon picker
- DV/IV entry (numeric inputs with +/- steppers, range-clamped)
- Nature picker (desktop has 25 natures — iOS uses a searchable Selection)
- Ability picker (conditional on game generation)
- "Create Route" → Route Editor

### Screen: Route Editor

Bottom tab bar with three tabs:

**Events Tab** (primary)
- Scrollable list of events and folders
- Folders are collapsible sections with a chevron toggle
- Each event row shows: type icon, name, XP delta, current level
- Disabled events shown with reduced opacity + strikethrough
- Swipe-left on event: Delete, Disable/Enable
- Long-press on event: Move (drag handle), Tag/Color picker
- "+" FAB (floating action button): opens Add Event sheet
- Search bar at top (same as desktop filter)
- Filter chips below search: Trainer / Wild / Items / Vitamins / Heal / All

**Stats Tab**
- Current Pokemon card: name, level, XP to next level, moveset
- Stat block (compact 2-column grid: ATK/DEF/SPD/SPC or ATK/DEF/SP.ATK/SP.DEF/SPD)
- Stat XP bar (realized vs unrealized, same concept as desktop)
- Badge list (horizontal scroll of badge icons, greyed out if not earned)
- Inventory (scrollable list of held items)
- Everything updates in real time as you scroll through the Events tab

**Summary**
- Route summary (all events, final state) — equivalent to the desktop Route Summary Window
- Export / Share button (see Phase 3)

### Screen: Event Detail (Bottom Sheet)
When tapping an event, a bottom sheet slides up with the full detail view:
- For Trainer Battle: opponent name, team, XP awarded, can expand to see move details
- For Wild Pokemon: species, level, XP
- For Item events: item name, quantity, price
- "Edit" button → transitions to Edit Event screen

### Screen: Add Event
Bottom sheet with a type picker at top (same 9 event types as desktop), then the relevant form slides in below. Forms mirror the desktop event editors but adapted for touch:
- Dropdowns replace combo boxes
- Numeric steppers for counts
- Trainer name uses a searchable list (same TrainerDB as desktop)
- "Add" confirms and closes the sheet, event appears in list

### Screen: Battle Calculator
- Equivalent to the desktop Battle Summary window
- Player move picker, enemy species/level, stage modifiers
- Damage range display (X – Y, Z% KO chance)
- Weather and screen toggles
- Scrollable matchup list for current route position

---

## Phase 3: Cross-Platform File Sharing (1 week)

Routes are already plain JSON. The iOS app reads and writes the exact same format. No conversion needed.

**Method 1: iCloud Drive (Recommended Primary)**
- Configure the iOS app to store routes in `iCloud Drive/pkmn_xp_router/`
- On desktop, configure the user data directory to point to the same iCloud Drive folder
- Routes appear on both platforms automatically, no manual sharing step
- Works completely offline; iCloud syncs when connected

**Method 2: Share Sheet Export**
- "Share Route" button in the Summary tab opens the iOS Share Sheet
- User can: Email attachment, AirDrop to Mac, Save to Files, copy to clipboard
- On desktop, open the received `.json` file via File → Load Route
- This is the fallback when iCloud isn't configured

**Method 3: Files App Integration**
- The app registers as a Files provider so `.json` route files open directly in the app when tapped from Files, Mail, or AirDrop

Implementation: `UIDocumentPickerViewController` (wrapped by Toga's file dialogs) for import; `UIActivityViewController` (Toga's Share Sheet) for export. Both are one-call APIs in Briefcase.

---

## Phase 4: UX Polish & Mobile-Specific Details (1–2 weeks)

### Gesture Mapping (replaces keyboard shortcuts)

| Desktop | Mobile |
|---------|--------|
| Ctrl+Z / Ctrl+Y | Shake to undo (or swipe from left edge) |
| Delete key | Swipe left → Delete |
| Ctrl+Click multi-select | Long-press to enter selection mode |
| Drag to reorder | Long-press drag handle |
| Ctrl+S | Auto-save on every change (no manual save on mobile) |
| Ctrl+F search | Tap search bar in Events tab |

**Auto-save:** Mobile users expect auto-save. Implement write-on-change with a debounce (500ms) to the route file. No save button needed.

**Compact stat display:** On small screens, the stat block should use abbreviations and a card layout. The desktop shows everything at once; mobile should default to showing just the current-level stats and let the user expand to see Stat XP detail.

**Undo:** The existing `undo_manager.py` works as-is. Surface it via a toolbar undo/redo button pair (standard iOS pattern) instead of keyboard shortcuts.

**Dark mode:** The desktop uses a custom dark theme. iOS respects the system appearance setting; implement a light/dark variant using Toga's style system.

**iPad layout:** On iPad, use a split view: the Events list on the left panel, Stats/Detail on the right — approximating the desktop layout more closely.

---

## Phase 5: App Store Preparation (1–2 weeks)

- App icon design (required at multiple resolutions)
- Privacy manifest (iOS 17+ requirement)
- Briefcase's `briefcase submit ios` workflow handles IPA signing and upload
- App Store metadata, screenshots, age rating (appropriate for a utility app)

---

## What Changes Per Feature Addition

| Change type | Files to edit |
|---|---|
| New Pokemon game support | `pkmn/gen_X/`, `raw_pkmn_data/gen_X/` (one place) |
| New event type | `routing/route_events.py` + `gui_qt/event_editors.py` + `gui_mobile/screens/add_event.py` |
| Damage calc fix | `pkmn/damage_calc.py` or `pkmn/gen_X/pkmn_damage_calc.py` (one place) |
| New stat display column | `gui_qt/pkmn_components/stat_column.py` + `gui_mobile/components/stat_card.py` |
| New route file field | `routing/route_events.py` or `routing/router.py` (one place — both apps read same JSON) |
| UI-only cosmetic change | Either `gui_qt/` OR `gui_mobile/` depending on which platform |

The business logic (70% of the codebase) is maintained once. The remaining 30% is split between two UI directories in the same repo, same language.

---

## Risks and Mitigations

| Risk | Mitigation |
|---|---|
| Python performance on iOS (heavy JSON parsing for large databases) | Pre-process `raw_pkmn_data/` into binary msgpack at build time; load lazily |
| Toga widget limitations for the event tree view | Implement tree as a flat list with indentation; store expand/collapse state in view model |
| Large app bundle size (all Pokemon databases bundled) | ~20MB of JSON data; acceptable. Compress with gzip if App Store size is a concern |
| BeeWare/Briefcase iOS support gaps | BeeWare has paid commercial support and active development; build a throwaway prototype in Phase 1 to smoke-test before committing |
| iCloud sync conflicts (edited on both platforms) | JSON is human-readable; implement a simple last-write-wins merge, same as any text file |

---

## Estimated Effort Summary

| Phase | Duration | Scope |
|---|---|---|
| 1: Tooling & Core Compatibility | 1–2 weeks | Briefcase setup, iOS file paths, dependency pruning, build verification |
| 2: iOS UI (all screens) | 4–6 weeks | All Toga screens and components |
| 3: File Sharing | 1 week | iCloud, Share Sheet, Files app |
| 4: UX Polish | 1–2 weeks | Gestures, auto-save, dark mode, iPad layout |
| 5: App Store | 1–2 weeks | Signing, metadata, submission |
| **Total** | **8–13 weeks** | |

The ordering above is de-risked: Phase 1 proves the architecture before investing in UI. If BeeWare has a blocking iOS issue, you find out in week 2, not week 10.
