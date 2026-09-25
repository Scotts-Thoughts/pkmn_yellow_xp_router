# World Map (pokemap) integration — implementation plan

Status: phases 0–4 (the MVP) implemented 2026-09-24; Phase 5–6 items and
gen 4/5 remain (§12). Rust/egui only (the Python app is deprecated and gets
none of this). File/line references in §2 are to the code as of commit
`9f1e337`, before the implementation; §12 lists what was built and where it
differs from the plan.

Companion project: **pokemap** (`~/Documents/pokemap`, GitHub
`Scotts-Thoughts/pokemap`, last commit `d80c88e` 2026-09-15) — a Vite/JavaScript
interactive world-map viewer with a Node pipeline that extracts map data from
the pret decompilations. The router does not embed that web app; it ports the
viewer to Rust and turns pokemap's pipeline into the router's map-data source.

The plan is deliberately incremental: seven phases, each leaving `main`
shippable. The minimum viable product the user asked for (map beside the event
list, add battles/wild/items from the map, jump from an event to its map
location, snappy, no game repos needed by users) is complete at the end of
Phase 4.

Every number in §2 was measured on this machine on 2026-09-24; the scripts are
listed in Appendix B so the measurements can be rerun.

---

## 0. Summary

**What we build.** A new pure-Rust crate `xpr-map` (data model, world layout,
tile compositor, spatial index, link resolution) plus a `map` module in
`xpr-app` (an egui viewport with pan/zoom, markers, info cards, and route
actions). The map data for the seven gen 1–3 games ships inside the binary as a
~2–3 MB deflated pack exported by a new pokemap pipeline script and committed
to this repo under `map_data/`. Route events and map objects are linked through
tables generated at pipeline time from the decompilations (exact where the game
code allows it, curated overrides where it does not), keyed by the router's own
trainer/item identities so the route file format does not change.

**Targets** (measured against the current web viewer, §2.4):

| Metric | pokemap today | Target |
|---|---|---|
| Open the map for a game (warm) | 2–15 s stall while the whole world is composited | < 150 ms to first frame, world fills in progressively in the background |
| Pan / zoom | redraws every marker each mouse move; stutters on gen 3 | 60 fps at any zoom on an integrated GPU, no per-frame allocation |
| Zoomed-out overview | scales a 137 Mpx canvas per frame | LOD pyramid: never more than ~40 chunk textures on screen |
| Shipped data (7 games) | 30.8 MB `public/data` served from disk | ≤ 3 MB deflated inside the binary; no game repos, no ROMs |
| Trainer link coverage | 43–83 % of trainer records reach a map position via runtime heuristics | ≥ 95 % of non-rematch, non-unused trainers per game via exact pipeline links + overrides |

---

## 1. Decisions (read these first)

| # | decision | why |
|---|---|---|
| D1 | **Port the viewer to Rust/egui; do not embed the web app.** pokemap stays the reference implementation and the pipeline owner. | The router is a single self-contained binary (rust_port_plan.md D1/D6). A webview would add a runtime, break the single-file packaging and make event↔route interaction a cross-process problem; and it would not fix the performance, which is architectural (§2.4). |
| D2 | **All map logic without egui lives in a new crate `rust/crates/xpr-map`**; the UI lives in `rust/crates/xpr-app/src/map/`. | Same split as route compare (`xpr-engine/src/compare.rs` + `xpr-app/src/compare/`): testable headlessly, reusable by a CLI (golden renders, coverage checks). |
| D3 | **Ship compact tile ingredients and composite at runtime** for gens 1–3 (blockdata, metatiles, palettes, tileset sheets, sprites). Pre-rendered images are used only where no tile model exists (gen 4/5, Phase 7). | Measured pack cost is ~2–3 MB for all seven games versus tens of MB of pre-rendered PNG (Emerald's world alone is 137 Mpx). Runtime compositing in Rust is cheap (one palette lookup per pixel), keeps pixel-exact fidelity, and makes gen 2 night mode and palette variants free. Phase 0 proves this with numbers before anything else is built; the fallback (a pre-rendered PNG pack, same loader API) is recorded in §8. |
| D4 | **The pack is produced by pokemap's pipeline** (new `pipeline/export-router.js`) and **committed to this repo** under `map_data/<game>/`. The router build embeds it like `raw_pkmn_data` (`xpr-map/build.rs`, feature `embed-map-data`, on for `xpr-app`). | pokemap already parses the decomps; one pipeline. The decomp repos are only needed on the machine that runs the export (they are not on this Mac, §2.7); users and CI never need them. |
| D5 | **Pack format v1 = JSON metadata + raw binary blobs + PNG sheets, each deflated by `build.rs`.** No new serialization dependency. | `serde_json`, `flate2`, `image`(png) are already in the workspace; the big arrays (blockdata, metatiles) become byte blobs (§3.2) which is what makes the pack small. |
| D6 | **Links are computed at pipeline time and keyed by the router's own identities** (`Trainer.name`, `BaseItem.name`, map constants), never by runtime string heuristics. Unlinkable cases go to a hand-curated `overrides.json` per game; the pipeline prints a coverage report listing every router trainer that still has no position. | pokemap's runtime matcher reaches 100 % of gen 1 trainer objects but 0 % of Platinum and leaves every gen 2 gym leader/rival/Elite Four unlinked (§2.3). Exact sources exist in the decomps for all three generations (§4.2), verified on 2026-09-24. |
| D7 | **Docked map = a third tab of the right pane ("Map") with its own splitter fraction; undocked map = a secondary OS window** (the run-summary pattern). The route list stays visible in both modes. | The right pane already hosts `Pre-Event State` / `Battle Summary` tabs (`event_details.rs:427-440`); a tab is the smallest change that keeps the event list on screen. Undocking reuses `ctx.show_viewport_immediate` exactly as the summaries do (`app.rs:2479-2544`). A three-column layout is deferred (§9). |
| D8 | **Rendering = chunked, LOD-pyramided textures with nearest magnification, composited on a worker thread, uploaded on the UI thread under a per-frame budget, LRU-capped.** Markers are drawn from a per-frame culled list; hit-testing uses per-map tile grids. | This is what makes it snappy (§3.4). It never renders more than what is visible plus a small margin, and the overview is a cheap downsampled pyramid instead of a giant canvas. |
| D9 | **No route file format change for the MVP.** Links resolve from `trainer_name` / `item_name` at runtime. A per-event `map_anchor` (user-pinned position) is a later, optional addition following the `thief_mons` recipe (`events.rs:662-668`). | Keeps every existing route byte-identical; no golden-corpus churn. |
| D10 | **Scope of the MVP = the seven gen 1–3 games pokemap ships**: Red/Blue, Yellow, Gold/Silver, Crystal, Ruby/Sapphire, Emerald, FireRed/LeafGreen. Gen 4/5 are Phase 7 with their own spec. | pokemap has complete tile data for exactly these; gen 4 has metadata but no images on disk, gen 5 has nothing (§2.7). Bulbapedia imagery is CC BY-NC-SA and must not be shipped. |
| D11 | **Reuse the router's data wherever it overlaps**: trainer parties come from `TrainerDB`, Pokémon icons from `Assets::pkmn_icon`, species/item names from `PkmnDB`/`ItemDB`. The pack carries no trainer data and no Pokémon icons. | pokemap's `trainers/*.js` is the same dataset as `raw_pkmn_data` reshaped (§2.6); shipping it twice would be 0.3–0.6 MB per game of duplication and a second source of truth. |
| D12 | **Encounter tables are map-keyed** (decomp map constants), taken from the router's own `*_encounter_tables.json` for gen 1/2 and from `src/data/wild_encounters.json` in the gen 3 decomps. Version differences (Red/Blue, Gold/Silver, Ruby/Sapphire, FireRed/LeafGreen) are kept as columns and the open route's version picks one. | These sources cover 65/65, 115/115 and 116/116 maps by constant name (§2.3) — no display-name matching, which today fails for 4–20 maps per game. |
| D13 | **Coordinates**: one world-pixel space per game at native scale (8 px tiles). Blocks are 32 px (gen 1/2) or 16 px (gen 3); every event/object position is in 16 px "steps" (px = x·16). Maps are placed in world space by the pipeline (`layout.json`), not at runtime. | Matches pokemap exactly, so pokemap's own PNG export is the golden reference for the compositor. |
| D14 | **Threading**: `std::thread` + `mpsc` + `ctx.request_repaint()`, the app's existing idiom (`app.rs:73-120`, `compare/mod.rs:241-288`). No async runtime; `rayon` only inside the compositor for full-world overview builds. | Consistency with the rest of `xpr-app`; the recorder-style host queue is not needed because the map never mutates the route from another thread. |

---

## 2. What exists today

### 2.1 pokemap — the client

`src/` is 5,199 lines of ES modules, no framework:

| file | lines | role |
|---|---|---|
| `src/main.js` | 2,073 | app state, world/indoor view modes, `redraw()`, overlay drawing (`drawOverlaysRaw`), click/double-click handling, hit-testing (`hitTestLocal`), grass/water tile detection, encounter/sign/item/trainer popovers, land:water HUD, item finder, PNG export dialog, game switching, view-state persistence in `localStorage` |
| `src/render/tile-renderer.js` | 441 | `renderNamedMap()`: gen 1 (one 4-colour palette per map), gen 2 (per-tile palette index + time-of-day/roof/tileset overrides), gen 3 (primary+secondary tileset, 2×2 metatiles × 2 layers, 16-colour palettes, flips). Constants: `TILE_SIZE=8`, `BLOCK_PX=32`, `GEN3_BLOCK_PX=16`, `getGen3Constants()` (512 primary metatiles/tiles, 640 for FR/LG, 6/7 primary palettes) |
| `src/render/world-map.js` | 460 | `computeWorldLayout()`: BFS over map connections from region seeds, cross-edge misalignment fix-up, multi-region packing (Johto+Kanto side by side, ≤ 900 blocks wide), `POSITION_OVERRIDES` (Mt. Silver, Route 28, Route 114 in Emerald, Ruins of Alph), `UNDERLAY_MAPS`/`OVERLAY_MAPS` draw order, `worldToMap()` |
| `src/render/sprite-renderer.js` | 370 | overworld sprite frames: gen 1/2 16×96 grayscale sheets → shade extraction → palette recolour (gen 1: map CGB palette; gen 2: `PAL_NPC_*` × time of day), gen 3 full-colour sheets with chroma key |
| `src/render/overlay-renderer.js` | 143 | older marker/hit-test helpers (superseded by `main.js`) |
| `src/data/trainer-match.js` | 223 | runtime heuristics linking a map trainer object to a `trainers.json` record (gen 1 class+number, gen 2/3 script-name parsing, gen 4 key equality); EV-yield rendering |
| `src/data/data-loader.js` | 153 | fetches the twelve JSON files + PNGs per game |
| `src/ui/info-panel.js`, `src/ui/map-list.js` | 267 | right info panel (trainer card, encounter tables, warp, sign, item), searchable map list grouped Cities/Routes/Indoors |
| `src/interaction/pan-zoom.js` | 90 | drag pan, wheel zoom about cursor (×1.18 / ×0.85), `centerOn()` |

Features worth carrying over: world view with seamless stitched overworld;
indoor maps rendered on demand with a "Back to World Map" button; warps
(double-click to follow); toggles for trainers/items/warps/signs/hidden/berries;
gen 2 night mode; click on grass/water → encounter table filtered to land/water
methods; trainer popover with party, moves, EV yields; item finder
(item → cycle through locations); PNG export of the world at 1–8×; per-game
view state persistence.

### 2.2 pokemap — the pipeline and data

`pipeline/` (8,948 lines) builds `public/data/<game>/` from local clones of the
pret decomps (`repos/pokered`, `pokeyellow`, `pokegold`, `pokecrystal`,
`pokeruby`, `pokeemerald`, `pokefirered`; `pokeplatinum`/`pokeheartgold` for gen
4; raw `.nds` ROMs for gen 5). `npm run pipeline` (= `pipeline/build-all.js`)
runs gen 1–3 + Platinum + EV yields + icons.

Per game (gen 1–3) the output is:

| file | contents |
|---|---|
| `maps.json` | keyed by map constant (`PALLET_TOWN`, `MAP_LITTLEROOT_TOWN`): display name, size **in blocks** (gen 1/2) or **in metatiles** (gen 3), tileset (gen 1/2: index + file name; gen 3: `primaryTileset`/`secondaryTileset` + `blockdataPath`), `connections[] {direction, mapConst, offset}` (offset in blocks), gen 2 `groupId`/`environment`, gen 3 `mapType` |
| `blockdata.json` | keyed by map constant: flat row-major block ids (gen 1/2: 0–127 bytes; gen 3: raw 16-bit `map.bin` values, metatile id = `v & 0x3FF`, collision/elevation in the high bits) |
| `blocksets.json` | gen 1/2: per tileset, blocks of 16 tile indices (4×4 tiles of 8 px; gen 2 VRAM bank 1 remapped to +0x60); gen 3: per tileset, metatiles of 8 `{tile, xflip, yflip, palette}` entries (bottom layer 4, top layer 4) |
| `palettes.json` | gen 1: 4-colour CGB palettes + per-map assignment (Red/Blue: monochrome); gen 2: `bgPalettes{morn,day,nite,dark,indoor}` (8 × 4 colours), `paletteMaps` (per-tile palette slot per tileset), `mapAssignments` (time-of-day class per map), `npcPalettes`, `roofPalettes` (per map group), `tilesetPalettes` (Crystal-only overrides); gen 3: per tileset 16 × 16 colours |
| `grass.json`, `water.json` | per tileset, the block/metatile ids that are tall grass / surfable water (drives "click grass → encounters") |
| `events.json` | keyed by map constant — see Appendix A for the per-gen schema (`objectEvents` typed `trainer`/`item`/`npc`/`berry`, `warps`, `bgEvents`, `hiddenItems`, gen 3 `coordEvents`) |
| `trainers.json` | `trainers/<game>.js` regrouped by location string (same dataset as the router's, §2.6) |
| `encounters.json` | keyed by **display location name** (`"Route 3"`, `"Cerulean Cave (1F)"`), methods Walk/Surf/Old Rod/Good Rod/Super Rod/Gift/Static…, `_versions` + per-version columns where two cartridges differ |
| `sprites.json` + `sprites/*.png` | `SPRITE_*`/`OBJ_EVENT_GFX_*` → sheet file (+ width/height in gen 3) |
| `sign_text.json` | sign/bg-event text keyed by text label (gen 1) or script label (gen 2/3) |
| `tilesets/*.png` | grayscale tile sheets, 128 px wide (16 tiles), 48–256 px tall |
| `icons/*.png` | Pokémon icons (40×40 gen 1/2, 32×64 gen 3); FR/LG has none |
| `ev_yields.json` | gen 3/4 only |

Provenance that matters for the plan:

- `trainers/*.js` (root) are ES-module exports of the **same computed dataset
  as this repo's `raw_pkmn_data/**/trainers.json`** (verified: identical
  stats/XP/money/nature values for Yellow "Youngster 1", Crystal "Falkner 1",
  Emerald id 37), reshaped with different field names (`party` vs `pokemon`,
  `experience_yield` vs `xp`, nested `stats`). Gen 1 keys are upper-case
  (`"YOUNGSTER 1"` vs the router's `"Youngster 1"`); gen 2 keys are `"<Name> <index>"`
  (`"Falkner 1"`, `"Youngster 1"`); gen 3 keys are the numeric ROM trainer id
  as a string (`"37"`).
- Ruby/Sapphire's build already stamps exact `trainerConst`/`trainerRomId` onto
  trainer objects (`pipeline/gen3/build-ruby-sapphire.js:77-96`, via
  `parseTrainerScriptMapping` + `parseTrainerConstants` in
  `pipeline/gen3/parse-events.js:130-189`) and resolves item balls from
  `item_ball_scripts.inc`. Emerald and FR/LG builds do **not** call these
  helpers — they exist and work (§2.3).
- The gen 3 encounter parser (`pipeline/gen3/parse-encounters.js`) already
  reads `src/data/wild_encounters.json` (map-keyed, version groups by
  `base_label` suffix) and then converts to display names; the map-keyed form
  is what the router pack needs (D12).
- `pipeline/world-report.mjs` hard-codes a Windows path (`A:/Dropbox/...`);
  three `*.tmp.*` editor leftovers sit next to real scripts. Neither affects
  the plan; both should be cleaned up in the pokemap repo.

### 2.3 Measurements (2026-09-24)

**World geometry** (pokemap's own layout code run under Node, Appendix B):

| game | maps | outdoor / indoor | world (blocks) | world (px) | Mpx | RGBA if fully rendered |
|---|---|---|---|---|---|---|
| yellow | 227 | 36 / 191 | 170 × 180 | 5,440 × 5,760 | 31.3 | 120 MiB |
| red_blue | 226 | 36 / 190 | 170 × 180 | 5,440 × 5,760 | 31.3 | 120 MiB |
| crystal | 388 | 69 / 319 | 379 × 135 | 12,128 × 4,320 | 52.4 | 200 MiB |
| gold_silver | 368 | 69 / 299 | 379 × 135 | 12,128 × 4,320 | 52.4 | 200 MiB |
| emerald | 518 | 64 / 454 | 1,004 × 534 (16 px) | 16,064 × 8,544 | 137.3 | 524 MiB |
| ruby_sapphire | 394 | 57 / 337 | 1,004 × 380 | 16,064 × 6,080 | 97.7 | 373 MiB |
| firered_leafgreen | 425 | 63 / 362 | 807 × 400 | 12,912 × 6,400 | 82.6 | 315 MiB |

**Objects** (from `events.json`):

| game | trainer objs | item balls | hidden items | berry trees | NPCs | warps |
|---|---|---|---|---|---|---|
| yellow | 329 | 122 | 66 | 0 | 492 | 813 |
| red_blue | 334 | 119 | 53 | 0 | 465 | 805 |
| crystal | 316 | 182 | 85 | 30 | 905 | 1,292 |
| gold_silver | 303 | 142 | 87 | 30 | 835 | 1,221 |
| emerald | 552 | 222 | 112 | 88 | 1,914 | 1,313 |
| ruby_sapphire | 348 | 144 | 98 | 88 | 1,523 | 1,156 |
| firered_leafgreen | 441 | 178 | 183 | 0 | 1,029 | 1,294 |

**Trainer link coverage with pokemap's runtime heuristics** (`matchTrainer`
over every trainer object; "records" = trainer records in `trainers.json`):

| game | objects matched | records with ≥ 1 position | what is missing |
|---|---|---|---|
| yellow | 329/329 (100 %) | 324/400 (81 %) | scripted rival fights, Jessie & James, unused/duplicate trainers |
| red_blue | 332/334 (99 %) | 328/395 (83 %) | same |
| crystal | 313/316 (99 %) | 309/541 (57 %) | **all 8 gym leaders, Elite Four, Champion, all 21 rival fights**, scripted grunts |
| gold_silver | 300/303 (99 %) | 297/491 (61 %) | same |
| emerald | 491/552 (89 %) | 425/854 (50 %) | rivals, Aqua/Magma bosses, Winstrates, rematch variants |
| ruby_sapphire | 321/348 (92 %) | 301/693 (43 %) | same |
| firered_leafgreen | 382/441 (87 %) | 372/639 (58 %) | Rocket grunts/admins, rivals |
| platinum | 0/407 (0 %) | 0/385 | gen 4 objects carry `id: LOCALID_*` and `script: null`; the matcher looks at `script` |

**Exact linking mechanisms verified against fresh shallow clones of
pokeyellow/pokecrystal/pokeemerald** (D6):

- Gen 3 (`data/maps/<Map>/scripts.inc`): a `trainerbattle_single|double|rematch|rematch_double|no_intro|trainerbattle TRAINER_X` inside the label an object points to, then `TRAINER_X` → number in `include/constants/opponents.h`, equals the router's `trainer_id` (`TRAINER_SAWYER_1 = 1` = "Hiker Sawyer", id 1). Emerald: **480/552 trainer objects (87 %)** exactly linked; the 72 unlinked are the Battle Pyramid's dynamic trainers. **484/563 non-rematch records (86 %)** get a position; the remaining 79 are scripted battles whose `trainerbattle` sits in a label reached by `goto`/`call` from an NPC script (rival fights on Routes 103/110/119, Archie/Maxie/Tabitha, the Winstrates, Gabby & Ty) — §4.2 handles those by following one level of script control flow and by map-level anchors.
- Gen 2 (`maps/<Map>.asm`): trainer objects use the `trainer CLASS, CONST` macro; scripted battles use `loadtrainer CLASS, CONST` inside the NPC's script label (58 map files, all gym leaders, Elite Four, Champion, rivals with starter-specific constants such as `RIVAL1_1_CHIKORITA`). `CONST` → index within class from `constants/trainer_constants.asm` (`trainerclass FALKNER ; 1` / `const FALKNER1`).
- Gen 1 (`scripts/<Map>.asm`): scripted battles are the pair `ld a, OPP_X` / `ld [wCurOpponent], a` followed by `ld a, N` / `ld [wTrainerNo], a` (16 files in pokeyellow); rival fights load `wRivalStarter` and add an offset (starter-dependent variants).

**Item name coverage** (map item names produced by pokemap's `extractItemName`
versus the router's `items.json`): 51 % / 55 % / 76 % / 75 % / 65 % / 68 % / 81 %
of distinct names for yellow / red_blue / crystal / gold_silver / emerald /
ruby_sapphire / firered_leafgreen. Nearly every miss is a TM/HM: pokemap strips
the number (`TM18_RAIN_DANCE` → "TM Rain Dance") while the router names items
`"TM18 Rain Dance"`. The rest are fake item balls (Voltorb/Electrode), coins,
and gen 3 key items with different spellings. §4.3 fixes all of these in the
exporter.

**Encounter table linking**: pokemap's display-name heuristics resolve 25/29,
25/29, 42/46, 42/46, 18/29, 18/24 and 23/43 of the maps that contain grass
tiles (Safari Zone, Mt. Pyre, Sevii Islands and gen 3 `PascalCase` names fail).
Map-constant-keyed sources cover everything: the router's own
`raw_pkmn_data/gen_one/yellow/yellow_encounter_tables.json` (65 maps, all
present in pokemap `maps.json`), `gen_two/crystal/crystal_encounter_tables.json`
(115/115; Red/Blue and Gold/Silver have per-version files), and pokeemerald's
`src/data/wild_encounters.json` (116/116).

**Sizes** (deflate level 9, the same compression `xpr-data/build.rs` uses):

| game | map JSON raw | map JSON deflated | tilesets + sprites PNG | sign text deflated | blockdata as bytes, deflated |
|---|---|---|---|---|---|
| yellow | 440 KB | 54 KB | 38 KB | 22 KB | 10 KB |
| red_blue | 433 KB | 53 KB | 35 KB | 22 KB | — |
| crystal | 866 KB | 105 KB | 81 KB | 65 KB | 16 KB |
| gold_silver | 774 KB | 94 KB | 55 KB | 59 KB | — |
| emerald | 8,467 KB | 411 KB | 359 KB | 129 KB | 135 KB |
| ruby_sapphire | 5,690 KB | 303 KB | 271 KB | 85 KB | — |
| firered_leafgreen | 5,834 KB | 298 KB | 221 KB | 61 KB | — |

Emerald's `blocksets.json` alone is 5.96 MB of text; as packed `u16` it is
≤ 1.1 MB raw and ~200 KB deflated. The whole seven-game pack lands at
**≈ 2–2.5 MB** deflated (§7). For comparison: the current `xpr-data` pack is
2.4 MB and the macOS release zip is 14.6 MB.

### 2.4 Why the current viewer is clunky

Everything below is structural and is designed out in §3.4:

1. **Whole-world synchronous compositing on game switch and on every night
   toggle.** `buildWorldMap()` (`main.js:329-402`) renders every outdoor map
   into one `OffscreenCanvas` the size of the entire world — up to
   16,064 × 8,544 (Emerald, 524 MiB of RGBA) — with progress text; the UI is
   unresponsive for seconds. Gen 3 is rendered pixel by pixel in JavaScript
   (`renderGen3Map`, `tile-renderer.js:643-750`); gen 1/2 issue 16 `drawImage`
   calls per block (`renderMap`, `tile-renderer.js:755-783`), ~380 k calls for
   Yellow's overworld.
2. **No level of detail.** Zoomed out, every frame scales the full-resolution
   world canvas (`mapCtx.drawImage(worldCanvas…)` under a transform).
3. **Every frame re-walks all maps and redraws every marker** with canvas
   arcs and text (`drawWorldOverlays` → `drawOverlaysRaw`), including during
   drag (one full redraw per `mousemove`) and wheel ticks. Sprite frames are
   cached but each marker still costs a path + fill + stroke + `fillText`.
4. **`redraw()` has side effects**: it dismisses the popover and schedules a
   view-state save on every call.
5. **Linear hit-testing** over all objects of the clicked map; indoor maps
   are re-rendered on every visit (no cache); sprites load lazily with a
   "redraw when it arrives" loop that can cascade.
6. **Browser limits shape the data**: the world width is packed to stay under
   the 16,384 px canvas limit (`MAX_BLOCK_WIDTH = 900`, `world-map.js:332`),
   which is why Johto and Kanto are placed side by side.

### 2.5 The router side — what the map plugs into

Only facts the plan relies on; line numbers as of `9f1e337`.

**Application shell** (`rust/crates/xpr-app/src/app.rs`): `XprApp` (`app.rs:132-212`),
`Page::{Landing, NewRoute, Editor, Compare}` (`app.rs:42-48`).
`update_inner` (`app.rs:2300-2549`) lays out `TopBottomPanel::top("menu_bar")`,
`TopBottomPanel::bottom("status_bar")`, the conditional
`TopBottomPanel::bottom("docked_summary")`, then a `CentralPanel` dispatching on
`page`; then the quick-add popover, dialogs, toast, and the two secondary
viewports. The editor is `editor_page` (`app.rs:1876-1964`): a hand-rolled
two-pane splitter (left: message label, filter bar, `RouteList`; right:
`EventDetails`), 200 px minimum for the right pane, split fraction persisted
per tab through `cfg.set_pre_state_left_fraction` /
`set_battle_summary_left_fraction` (`xpr-core/src/config.rs:836-845`) with a
500 ms debounce (`app.rs:1966-1983`).

**Secondary windows**: `ctx.show_viewport_immediate(ViewportId::from_hash_of("setup_summary"), ViewportBuilder…, |ctx, _| …)`
(`app.rs:2479-2508`; run summary `2509-2544`), redrawn every frame while a
flag is set, state kept in `XprApp`. Open/closed state is not persisted; only
`run_summary_docked` is (`config.rs:960-961`). Toggle plumbing:
`open_summary_window` / `toggle_summary_dock` (`app.rs:944-965`), menu entry
(`app.rs:1694-1696`), shortcut `toggle_summary` (`app.rs:732-734`,
`config.rs:66`).

**Route list and selection** (`rust/crates/xpr-app/src/route_list.rs`):
flat virtualised rows rebuilt when `dirty`; selection lives in
`MainController::selected_ids` and is mirrored by `dispatch_signals`
(`app.rs:422-430`), which also calls `route_list.scroll_to_selected_events`
(`route_list.rs:235-273`, expands ancestors and scrolls). To select and reveal
an event from anywhere: `ctrl.select_new_events(vec![id])`
(`controller.rs:436-445`). Current selection:
`ctrl.get_single_selected_event_id(true)` (`controller.rs:340-349`).
Right-click on a row toggles enabled (`route_list.rs:1076-1093`); there is no
context-menu widget in the app.

**Creating events** (`rust/crates/xpr-app/src/controller.rs:731-746`):

```rust
pub fn new_event(&mut self, event_def: EventDefinition, insert_after: Option<NodeId>,
    insert_before: Option<NodeId>, dest_folder_name: Option<&str>, do_select: bool) -> Option<NodeId>
```

records undo, calls `Router::add_event_object` (`xpr-engine/src/router.rs:428-524`,
incremental recalc), fires `route_changed`/`selection_changed`. Definitions
(`xpr-engine/src/events.rs`): `EventDefinition::with_trainer(TrainerEventDefinition::new(&name))`
(`1145`/`501`), `with_wild(WildPkmnEventDefinition::new(name, level, quantity, trainer_pkmn))`
(`1187`/`256`), `with_item(InventoryEventDefinition::new(item_name, amount, is_acquire, with_money, custom_price))`
(`1173`/`70`). The quick-add popover's `add_trainer` (`quick_add.rs:202-244`) is
the template for "add from map" (double battles set `exp_split`). Whole-area
insertion: `ctrl.add_area(location, show_rematches, insert_after)` →
`Router::add_area` (`router.rs:384-425`), which filters `TrainerDB` by the
`Trainer.location` string.

**Event details** (`rust/crates/xpr-app/src/event_details.rs`): tabs
`PRE_STATE_TAB = 0`, `BATTLE_SUMMARY_TAB = 1` via `widgets::tab_bar`
(`event_details.rs:427-440`); `before_info()` (`471-527`) yields the label and
the trainer's `location` string; the editor title strip
(`editors.rs:1703-1735`) and the identity header (`state_views.rs:172-188`)
are where a "Show on map" control fits. The selected definition is reachable
through `ctrl.router.obj_kind(id)` → `group(id).event_definition`;
`def.get_first_trainer_obj(gen)` (`events.rs:1274`) resolves the `Arc<Trainer>`.

**Data** (`rust/crates/xpr-data`): `Registry::get_version(&str) -> Arc<GenData>`
(`registry.rs:166-180`, lazy, cached); version ids in `xpr-core/src/consts.rs:433-475`;
`builtin_sources()` (`registry.rs:20-55`) maps versions to `gen_<n>/<folder>`.
`Trainer { trainer_class, name, location, money, pkmn, rematch, trainer_id, refightable, double_battle }`
(`model.rs:722-732`); `TrainerDB::get_trainer(name)` keyed by
`sanitize_string(name)` (`db.rs:208`, `xpr-core/src/io_utils.rs:12-17`: keep
alphanumerics, lowercase); `ItemDB::get_item` (`db.rs:353-365`, sanitized with a
case-insensitive fallback); `PkmnDB::get_pkmn` (`db.rs:113`). The gen 4/5 loader
discards `trainer_location` (`loaders.rs:614-634`) even though Platinum's raw
file has it. `raw_pkmn_data/gen_one|gen_two/**/*_encounter_tables.json` exist
(map-constant keyed, `{grass_cave, surfing, old_rod, good_rod, super_rod, …}`
with `base_rate` and per-slot `rate`) but nothing loads them.

**Embedding** (`rust/crates/xpr-data/build.rs`, `src/embedded.rs`): every
`.json` under `raw_pkmn_data` is deflated into `OUT_DIR/data/*.z` and listed in
a generated `static FILES: &[(&str, &[u8])]` table with `include_bytes!`;
`embedded::lookup(rel_path)` inflates on demand; `contains()`/`list_dir()`
exist; the `Registry` reads embedded first and falls back to disk. 64 files,
2.4 MB. Feature `embed-data`, always on for `xpr-app` (`xpr-app/Cargo.toml:13`).
`xpr-app/build.rs` prescales Pokémon icons (64 px) and box art (160 px) and
embeds `icons/**`.

**Textures** (`rust/crates/xpr-app/src/assets.rs:45-52`): `Assets::load_texture`
decodes PNG with `image` and calls `ctx.load_texture(key, img, TextureOptions::LINEAR)`;
it is the only texture cache in the app, always linear-filtered.
`Assets::pkmn_icon(ctx, species)` (`assets.rs:59-64`) gives the species icon.
Dependencies (`xpr-app/Cargo.toml`): `egui`/`eframe` 0.33.3 (glow backend),
`image` 0.25 with only the `png` feature.

**Config and shortcuts** (`xpr-core/src/config.rs`): a key is a `Value` field
seeded in `Config::load` with `gv("key", default)` (pattern `config.rs:583`),
written in `to_json`'s `pairs` (`config.rs:639`), with typed accessors
(`config.rs:960-961`); setters save immediately. Shortcuts are three parallel
tables — `DEFAULT_SHORTCUTS` (`config.rs:14-103`), `SHORTCUT_LABELS`
(`105-194`), `SHORTCUT_CATEGORIES` (`196-312`) — consumed by `handle_shortcuts`
(`app.rs`, e.g. `fire("toggle_summary")` at `732-734`).

**Tests and hooks**: headless controller tests (`xpr-app/tests/prefight_candies.rs`),
a headless egui harness that feeds synthetic `RawInput` events to a view
(`xpr-app/tests/compare_page.rs:21-84`), and smoke screenshots driven by
`XPR_SMOKE_SCREENSHOT` / `XPR_SMOKE_ROUTE` / `XPR_SMOKE_ACTION` (`app.rs:2088-2169`).
`XPR_FRAME_LOG=1` prints per-frame timings (`app.rs:2279-2285`). Background work
uses `std::thread::spawn` + `mpsc` + `ctx.request_repaint()` (`app.rs:73-120`;
`compare/mod.rs:241-288` for on-demand loads).

**Recorder**: the gen 2 FSM already reads `overworld.mapGroup`/`mapNumber`/`x`/`y`
(`xpr-recorder/src/games/gen2.rs:61-64,119-122`) — a future "follow the player
on the map" hook (§9).

### 2.6 Identity keys per generation

The link table must be keyed by what the route file stores: the trainer-fight
event's `trainer_name` (`events.rs:501`, JSON key `"trainer_name"`), the
inventory event's `item_name`, the wild event's species `name`. This is how the
same trainer is identified on each side today, and the exact decomp source
that joins them:

| gen | router `Trainer.name` (raw `trainer_name`) | router disambiguator | pokemap record key | decomp source of a map object's trainer | join used by the exporter |
|---|---|---|---|---|---|
| 1 | `"Youngster 1"`, `"Rival2 2 Jolteon"` | none (names unique) | `"YOUNGSTER 1"` (upper case) | `object_event … OPP_YOUNGSTER, 1`; scripted: `wCurOpponent`/`wTrainerNo` | `sanitize(OPP class + " " + number)` = `sanitize(trainer_name)`; rival variants by starter |
| 2 | `"Leader Falkner"`, `"Youngster Joey"`, `"Rival1 1 Chikorita"` | `trainer_class` + `trainer_id` (index within ROM class; not globally unique) | `"Falkner 1"`, `"Youngster 1"` | `trainer CLASS, CONST` on the object / `loadtrainer CLASS, CONST` in its script; `CONST` index from `constants/trainer_constants.asm`, class display name from `data/trainers/class_names.asm`, trainer name from `parties.asm` | `(class display, index)` → the router record whose `trainer_class`/`trainer_id` match, confirmed by `sanitize(class + " " + rom name)` = `sanitize(trainer_name)` |
| 3 | `"Hiker Sawyer"`, `"Rival Brendan 3 Treecko"` | `trainer_id` = `opponents.h` number (globally unique) | `"37"` (id as string) | `trainerbattle_* TRAINER_X` in the object's script label | `TRAINER_X` → number = `trainer_id` |
| 4 | `"Youngster Tristan"` | `rom_id` (int) | `"TRAINER_YOUNGSTER_TRISTAN"` | `id: LOCALID_*` on the object; trainer id in the map's script | Phase 7 |

Items: router `BaseItem.name` (`"Potion"`, `"TM18 Rain Dance"`, `"HP Up"`);
decomp constants `POTION` / `ITEM_TM18` / `HP_UP` (§4.3). Species: router
`PokemonSpecies.name`; decomp `SPECIES_WURMPLE` / `db 3, PIDGEY` → title case,
with the pokemap `formatPokemonName` rules for Nidoran♂/♀ etc.

### 2.7 Gen 4/5 status (why they are Phase 7)

- pokemap's Platinum data has `maps.json` (593 headers, matrix-cell placement,
  no connections), `events.json` (flat `trainers`/`items`/`signs`/`warps`;
  only 7 visible item balls versus 262 hidden ones — the visible-item
  extraction is incomplete), `encounters.json`, `sign_text.json`; **no map
  images** (`public/data/platinum/maps/` is gitignored and empty here). The
  3D-render pipeline (`pipeline/gen4/render-*.py` + apicula + trimesh) has never
  been run on this machine; the Dropbox copy that ran it holds only 0-byte
  online-only placeholders.
- HeartGold/SoulSilver has only `maps.json`; Diamond/Pearl has nothing; gen 5
  has nothing but Bulbapedia manifests.
- The stitched gen 4/5 world maps (`stitch-world-map.js`) are 100 % Bulbapedia
  imagery, licensed **CC BY-NC-SA 2.5** — not shippable inside the app.
- pokemap's gen 4 `trainers.json` has `'?'` stats and estimated money; the
  router's `raw_pkmn_data/gen_four` is the real data (D11).

---

## 3. Target architecture

### 3.1 Crates, modules, repositories

```
pokemap/ (JS, unchanged app; new exporter)
  pipeline/export-router.js          → writes <router>/map_data/<game>/**        (§4.1)
  pipeline/links/{gen1,gen2,gen3}.js → exact trainer/item/encounter links         (§4.2)
  pipeline/links/items.js            → item-name normaliser vs router items.json  (§4.3)

pkmn_yellow_xp_router/
  map_data/<game>/…                  committed pack, format v1                     (§3.2)
  map_data/<game>/overrides.json     hand-curated anchors (source, not output)
  rust/crates/xpr-map/               NEW crate, no egui
    src/lib.rs        pub use …
    src/pack.rs       MapPack, loading (embedded or XPR_MAP_DATA_DIR), format checks
    src/model.rs      MapDef, MapObject, Anchor, Links, EncounterTable, Tileset, Palettes
    src/geom.rs       MapGeom, world/map/step coordinate maths, WorldLayout index
    src/compose.rs    Compositor: gen 1/2/3 tile compositing into RGBA chunks
    src/lod.rs        chunk keys, pyramid levels, downsampling
    src/sprites.rs    sprite frame extraction + recolouring, atlas packing
    src/spatial.rs    per-map tile grid for hit-testing
    src/links.rs      resolve trainer/item/species queries → anchors
    src/embedded.rs   generated FILES table (feature embed-map-data)
    build.rs          deflate map_data/** (generic file walk), like xpr-data/build.rs
    examples/render_world.rs, examples/coverage.rs
  rust/crates/xpr-app/src/map/
    mod.rs            MapView (state, ui(), actions), MapEnv
    camera.rs         Camera (center, zoom), screen↔world, animations, clamping
    chunks.rs         ChunkCache: LRU textures, worker channel, upload budget
    layers.rs         base layer draw, marker layer, routed-state overlay, focus pulse
    hit.rs            pointer → object/tile resolution
    cards.rs          InfoCard variants (trainer/item/hidden/berry/sign/warp/encounters)
    list.rs           map list + search
    toolbar.rs        toggles, zoom, undock, night
    state.rs          RouteMapState (defeated trainers, acquired items) from the Router
```

`xpr-app` gains: a `map: MapView` field on `XprApp`, a `MAP_TAB` in
`EventDetails`, menu/shortcut/config entries, `MapAction` handling in the
controller glue, a `map` smoke action, and a `Show on map` button.

### 3.2 The map pack (format v1)

Directory `map_data/<game>/` for `game ∈ {red_blue, yellow, gold_silver, crystal, ruby_sapphire, emerald, firered_leafgreen}`
(the pokemap game ids; the router picks the game from the route's version via
`builtin_sources()`: Red/Blue → `red_blue`, Yellow → `yellow`, Gold/Silver →
`gold_silver`, Crystal → `crystal`, Ruby/Sapphire → `ruby_sapphire`, Emerald →
`emerald`, FireRed/LeafGreen → `firered_leafgreen`; custom gens use their base
version's game).

| file | format | contents |
|---|---|---|
| `manifest.json` | JSON | `format: 1`, `game`, `gen`, `versions: ["Red","Blue"]`, `block_px` (32/16), `step_px: 16`, `world: {w_blocks, h_blocks}`, `gen3: {num_metatiles_in_primary, num_tiles_in_primary, num_pals_in_primary}` (512/512/6; FR/LG 640/640/7), blob index (`blockdata: [{map, offset, len}]`, `blocksets: [{tileset, offset, count}]`), source commit hashes of pokemap and each decomp repo, export timestamp |
| `maps.json` | JSON | array in stable order; per map: `id` (index), `const` (`PALLET_TOWN`), `name` ("Pallet Town"), `w`, `h` (blocks; gen 3 metatiles), `kind: outdoor \| indoor`, `tileset` (gen 1/2: index + file; gen 3: `primary`, `secondary`), `palette` (gen 1: palette index; gen 2: `tod_class` morn/day/nite/indoor + `group`; gen 3: none), `connections[] {dir, map, offset}`, `warps[] {x, y, dest_map, dest_warp}`, `env`/`map_type` (informational) |
| `layout.json` | JSON | pokemap's computed world placement: `positions: {const: [x_blocks, y_blocks]}`, `draw_order: [const…]` (underlay, base, overlay), `regions: [{name, rect}]` (Johto/Kanto), `seeds` |
| `blockdata.bin` | bytes | concatenation of every map's block ids: `u8` per block (gen 1/2) or `u16 LE` raw `map.bin` values (gen 3, id = `v & 0x3FF`); index in the manifest |
| `blocksets.bin` | bytes | gen 1/2: per tileset, blocks × 16 `u8` tile indices; gen 3: per tileset, metatiles × 8 `u16 LE` packed `tile \| xflip<<10 \| yflip<<11 \| palette<<12` (bottom layer entries 0–3, top 4–7) |
| `palettes.json` | JSON | as in pokemap (§2.2), colours as `[r,g,b]` 0–255 |
| `terrain.json` | JSON | `grass: {tileset: [ids]}`, `water: {tileset: [ids]}` |
| `tilesets/<name>.png` | PNG | the grayscale sheets, copied verbatim (gen 1/2: 4 shades from the red channel thresholds 192/128/64; gen 3: 16 indices from `(255 − r) · 15 / 255`) |
| `objects.json` | JSON | one array of markers, grouped by map in map order; per object: `map`, `x`, `y` (steps), `kind: trainer \| item \| hidden_item \| berry \| sign \| warp \| npc`, `sprite` (sheet name + frame hint), `facing`, and a payload: trainer → `{trainer: "<router trainer_name>" \| null, variants: [names] , double: bool}`; item/hidden/berry → `{item: "<router item name>" \| null, raw: "<decomp constant>", fake: bool}`; sign → `{text_key}`; warp → `{dest_map, dest_warp}` |
| `links.json` | JSON | `trainers: {"<router trainer_name>": [Anchor…]}`, `items: {"<router item name>": [Anchor…]}`, `stats: {trainers_total, trainers_linked, …}`; `Anchor = {map, x, y, precision: object \| script \| map, object: index \| null, source: "object" \| "script:<label>" \| "override"}` |
| `encounters.json` | JSON | `{"<map const>": {"<method>": {"<version or *>": [{species, min_level, max_level, rate}]}}}` with `base_rate` per method; methods `walk`, `surf`, `old_rod`, `good_rod`, `super_rod`, `rock_smash`, `headbutt`, plus gen 2 time-of-day suffixes |
| `sign_text.json` | JSON | as in pokemap |
| `sprites.json` + `sprites/<name>.png` | JSON + PNG | sheet metadata (frame width/height, layout) and the sheets; gen 2 NPC palettes live in `palettes.json` |
| `coverage.md` | Markdown | generated report (not embedded): per-game link statistics and the list of router trainers with no anchor, item names with no router match, maps with grass but no encounter table |

`build.rs` deflates every file except `coverage.md`/`overrides.json` into
`OUT_DIR` and generates the `FILES` table; at runtime `pack::read(game, rel)`
tries the embedded table, then `XPR_MAP_DATA_DIR` (dev override, default
`<repo>/map_data`).

The pack is versioned by `manifest.format`; the loader refuses a mismatched
format with a clear error rather than guessing.

### 3.3 Coordinate model (D13)

- **World px**: `(map_pos.x · block_px + step_x · 16, map_pos.y · block_px + step_y · 16)`.
  Gen 1/2 event coordinates are already in 16 px steps (half-blocks); gen 3
  metatiles are the steps. Marker centre = `+8, +8`. Sprite feet align with
  the step bottom (gen 3 sprites are 16 × 32).
- **Camera**: `center: Vec2` (world px), `zoom: f32` (screen px per world px,
  clamped `[fit_zoom, 12]`), `screen = (world − center) · zoom + viewport.center`.
  Pan snaps the translation to whole physical pixels when `zoom` is an
  integer so tile edges stay crisp.
- **Scopes**: `Scope::World` (all outdoor maps at their layout positions) and
  `Scope::Map(map_id)` (one indoor map at origin). Both use the same chunk
  machinery; a scope switch is a camera reset, not a re-render of anything
  already cached.
- **Chunks**: 512 × 512 world px at level 0; level `L` chunks cover
  `512 · 2^L` world px and are stored as 512 × 512 textures (box-downsampled).
  Emerald's world is 32 × 17 level-0 chunks; level 5 is a single chunk.

### 3.4 Rendering design (D8) — what makes it snappy

**Compositor** (`xpr-map/src/compose.rs`), pure functions over the pack:

1. Decode each tileset PNG once into a `Vec<u8>` of shade/palette indices
   (`Tileset { w, h, idx: Vec<u8> }`).
2. Gen 1/2: `recolored(tileset, palette_key) -> Arc<RgbaSheet>` — the tileset
   recoloured with the map's palette set (gen 2: per-tile palette slot ×
   time-of-day palettes × roof override per map group × tileset override),
   cached by key. Rendering a block is then sixteen 8 × 8 row copies.
3. Gen 3: `metatiles(primary, secondary) -> Arc<MetatilePixels>` — every
   metatile of the tileset pair pre-composited to 16 × 16 RGBA (≤ 1,024 ×
   1 KB = 1 MB per pair, cached per pair). Rendering a map cell is one 16 × 16
   copy. Top-layer index 0 is transparent over the bottom layer, as in
   `tile-renderer.js:729-731`.
4. `render_chunk(scope, level, cx, cy, variant) -> ColorImage`: for level 0,
   iterate the maps intersecting the chunk (from `WorldLayout::index`, a
   sorted rect list) in draw order and copy their cells; for level `L > 0`,
   render the 2 × 2 children at `L − 1` (or take them from the cache) and box
   downsample. `variant` encodes gen 2 night mode.
5. Cost: one copy per pixel; a 512 × 512 chunk is 262 k pixels → well under
   1 ms in release. The full Emerald world (137 Mpx) is ≈ 0.3–0.6 s
   single-threaded, ≈ 0.1 s with `rayon` — used only for the background
   overview build.

**Chunk cache** (`xpr-app/src/map/chunks.rs`):

- `ChunkKey { scope, level, cx, cy, variant }` → `TextureHandle` in an LRU
  capped by a byte budget (`map_texture_budget_mb`, default 256 MiB = 256
  512² RGBA textures; a 1080p viewport at zoom 1 needs ~12).
- One worker thread (two on ≥ 8 cores) owns an `Arc<Compositor>`; requests go
  through a priority queue keyed by (visible now, level distance, distance to
  camera centre); results return as `(ChunkKey, ColorImage)` over `mpsc`;
  stale requests (no longer visible, or superseded by a scope/variant change)
  are dropped when dequeued.
- The UI thread uploads at most `N` chunks per frame (`N = 4`, ≈ 1 MB each;
  glow uploads are sub-millisecond) via `ctx.load_texture(name, image, opts)`
  with `TextureOptions { magnification: Nearest, minification: Linear, wrap_mode: ClampToEdge, mipmap_mode: Some(Linear) }`
  (egui 0.33 generates mipmaps on the glow backend, `egui_glow/src/painter.rs:627-628`),
  then `ctx.request_repaint()` while anything is pending.
- Levels ≥ 2 are kept outside the LRU budget (the whole Emerald overview at
  level 2 is 32 chunks = 32 MiB; level 3 is 8 MiB) so zooming out is always
  instant after the first visit.
- On opening a game the cache issues the visible level-0 chunks first, then a
  background "overview job" that composites the world once at level 0 in
  streaming fashion (`rayon`) and folds it into levels 1–5 without keeping
  level 0 beyond the budget. Progress is a thin bar in the toolbar, never a
  modal.

**Draw** (`layers.rs`): pick `level = clamp(floor(log2(1 / zoom)), 0, max)`
so one texel is ≥ 0.5 screen px; compute the visible chunk range from the
camera; emit one `Mesh::with_texture(id)` per ready chunk with
`add_rect_with_uv(rect, uv(0..1), WHITE)` (egui batches consecutive shapes per
texture; ~12–40 draw calls). Missing chunks draw the nearest coarser level's
chunk stretched (progressive refinement) or the background colour. Nothing is
allocated per frame beyond the shape list.

**Markers** (`layers.rs` + `spatial.rs`): per frame iterate only the maps
whose rect intersects the viewport (≤ 69 outdoor maps, or one indoor map),
then their objects (slice of `objects.json` per map, ≤ ~100). Markers are
circles/rings from `Painter` primitives; letters only above zoom 1.5; sprites
(Phase 5) come from one atlas texture per game so all sprite markers form one
mesh. Toggles filter the kinds. The selected/focused anchor draws a pulsing
ring (animated with `request_repaint_after(16 ms)` only while the pulse runs).

**Hit-testing** (`hit.rs`): pointer → world px → `WorldLayout::index` rect
lookup (binary search on x then linear on ≤ 8 overlapping candidates; overlay
maps win) → step coordinates → per-map `HashMap<(u16, u16), SmallVec<obj>>`
built once at load; check the 3 × 3 step neighbourhood with the 12 px
threshold pokemap uses. Grass/water are answered from `blockdata` +
`terrain.json` the same way `isGrassTile`/`isWaterTile` do
(`main.js:433-482`).

**Input**: the viewport is `ui.allocate_rect(rect, Sense::click_and_drag())`;
`response.drag_delta()` pans; `ui.input(|i| i.smooth_scroll_delta)` scrolls or,
with Ctrl / on pinch (`i.zoom_delta()`), zooms about the cursor; double-click
follows a warp; keyboard: arrows pan, `+`/`-` zoom, `0` fit, `Backspace` back
to world, `Esc` dismiss card, `F` focus search. Repaint is requested only
during animations or while chunks are pending — the map costs nothing when
idle, like the rest of the app.

**Budget** (release, integrated GPU, 1080p, Emerald):

| stage | target |
|---|---|
| open the map (pack already loaded) | < 150 ms to first frame (visible chunks), overview complete within ~1 s in the background |
| pack load (inflate + parse + decode sheets), first open per game | < 80 ms, off the UI thread, spinner in the tab |
| chunk composite (512², level 0) | < 1 ms |
| frame while panning at zoom 2 | < 2 ms CPU (≈ 20 chunk rects + ≈ 300 markers) |
| frame at full zoom-out (level 5) | < 1 ms |
| texture memory | ≤ 256 MiB LRU + ≤ 48 MiB overview levels |

### 3.5 Threading (D14)

Map worker thread(s) (compositing), UI thread (upload, draw, input), and the
existing pattern for one-off background jobs (pack load on first open, overview
build). Communication is `mpsc`; every message carries the `Arc<MapPack>`
generation id so a game switch invalidates stale work. Route mutations happen
only on the UI thread through `MainController`, exactly like every other
feature.

### 3.6 UI integration (D7)

**Docked**: `EventDetails` gets `MAP_TAB = 2` in its `tab_bar`; when active,
the right pane draws `MapView::ui` instead of the per-event content, and the
splitter uses a new `map_left_fraction` (default 0.35 so the map gets ~65 % of
the width). Tab state, toggles and per-game camera are remembered
(`map_view_state`). The left pane (route list, filter bar) is unchanged — the
event list is always visible next to the map.

**Undocked**: a `Map` secondary viewport (`ViewportId::from_hash_of("map")`,
`ViewportBuilder::default().with_title("Map").with_inner_size([960, 720])`)
drawn every frame from the same `MapView` while `map_docked == false`; the
docked tab then shows "Map is open in its own window — [Dock]". Closing the
window re-docks. Toggle from the View menu, the toolbar's ⤢ button, or the
`toggle_map_dock` shortcut.

**Layout of the map pane** (logical px; 1 CSS px = 1 egui px):

```
┌ toolbar (28) ───────────────────────────────────────────────────────────────┐
│ [◀ World] Route 3 ▾   [🔍 search maps…]   ○Trainers ○Items ○Hidden ○Warps  │
│                        ○Signs ○Berries  [Night]   [−] 200% [+] [Fit]  [⤢]  │
├ viewport (fills) ───────────────────────────────────────────────────────────┤
│                                                                             │
│      (chunks + markers)                 ┌ info card (≤ 320 wide) ────────┐  │
│                                         │ Youngster 1 · Route 3 · ¥165   │  │
│                                         │ Rattata Lv11 · Ekans Lv11      │  │
│                                         │ [Add to route] [Details] [✕]   │  │
│ ┌ focus banner (when arriving from an event) ──────────────────────────┐   │
│ │ Showing "Youngster 1" — Route 3 (1 of 1)  ◀ ▶   [Select in list]     │   │
│ └──────────────────────────────────────────────────────────────────────┘   │
├ status strip (18) ──────────────────────────────────────────────────────────┤
│ Route 3 · step (14, 7) · grass          overview 64 %    12 chunks · 1.8 ms │
└─────────────────────────────────────────────────────────────────────────────┘
```

- **Map list**: the search box opens a popup list grouped Cities & Towns /
  Routes / Indoors & Dungeons (pokemap's grouping rules, `map-list.js:20-30`),
  keyboard navigable; selecting an outdoor map centres the world camera on it,
  an indoor map switches scope.
- **Info card** (`cards.rs`), anchored above the clicked object and kept
  inside the viewport (pokemap's `positionPopoverAboveTile`): trainer (name,
  class, location, money, party with icons via `Assets::pkmn_icon`, levels,
  moves — from `TrainerDB`, so it is exactly what the route will compute),
  item / hidden item / berry (router item name, or the raw constant with a
  "not a bag item" note for fake balls/coins), sign text, warp (destination +
  "Go"), encounter table for the clicked grass/water tile (methods filtered
  to land or water; version columns when the game has them; the route's
  version column highlighted). Buttons: **Add to route** (§3.7), **Show
  details** (selects the linked route event if one exists), **Add all
  trainers here** on the map header card.
- **Routed state** on markers: trainers already fought in the route (from
  `Router::defeated_trainers`) draw dimmed with a check badge; items already
  picked (acquire events without money) dimmed; the event currently selected
  in the list, if linked, is ringed. This is what turns the map into a routing
  tool: what is left here?
- **Focus banner** appears after "Show on map" and cycles through multiple
  anchors (rematch positions, starter variants, duplicates).

### 3.7 Linking model (D6, D9)

```rust
pub struct Anchor { pub map: MapId, pub x: u16, pub y: u16, pub precision: Precision, pub object: Option<u32> }
pub enum Precision { Object, Script, Map }       // exact tile, tile of the triggering NPC, whole map

pub enum LinkQuery<'a> { Trainer(&'a str), Item(&'a str), Species(&'a str) }
impl Links {
    pub fn trainer(&self, router_name: &str) -> &[Anchor]   // key = sanitize_string(name)
    pub fn item(&self, router_name: &str) -> &[Anchor]
    pub fn encounters_for_species(&self, name: &str) -> Vec<(MapId, Method)>
}
```

Event → query: `TrainerEventDefinition.trainer_name` (and
`second_trainer_name` for multi battles) → `LinkQuery::Trainer`;
`InventoryEventDefinition` with `is_acquire && !with_money` → `Item`;
`WildPkmnEventDefinition.name` → `Species` (shows every map where the species
appears in an encounter table, filtered by the enclosing folder's name when it
matches a map — best effort, not MVP-critical). Save/Heal/Blackout events
carry free text; not linked.

Map → route: the info card's **Add to route** inserts after the current
selection (`ctrl.get_single_selected_event_id(true)`), like the quick-add
popover, with `do_select = true`:

| object | event created |
|---|---|
| trainer (linked) | `EventDefinition::with_trainer(TrainerEventDefinition::new(name))`; double battles set `exp_split = vec![2; n]` as `quick_add.rs:220-226`; starter/rematch variants prompt a pick |
| item ball / hidden item / berry (linked) | `with_item(InventoryEventDefinition::new(item, 1, true, false, None))` |
| grass/water tile → encounter row | `with_wild(WildPkmnEventDefinition::new(species, level, 1, false))`; level picker defaults to `min_level` |
| map header "Add all trainers here" | for each linked, undefeated, non-rematch trainer of the map in object order: `new_event` into a new folder named after the map (mirrors `Router::add_area` but keyed by map, not by the location string) |

Unlinked objects show what is known and no button. All actions go through
`MainController`, so undo, recalculation and the route-list refresh work
unchanged.

### 3.8 Config, shortcuts, menus

Config keys (all following the `config.rs` pattern in §2.5):

| key | type | default | meaning |
|---|---|---|---|
| `map_docked` | bool | `true` | docked tab vs own window |
| `map_open` | bool | `false` | reopen the map tab/window at launch (unlike the summaries, persisted: the user asked for the map as a working surface) |
| `map_left_fraction` | number \| null | `null` (0.35) | splitter for the Map tab |
| `map_toggles` | object | trainers/items/hidden/warps/berries on, signs off | marker filters |
| `map_night` | bool | `false` | gen 2 night palettes |
| `map_view_state` | object keyed by game | `{}` | `{scope, map, zoom, cx, cy}` |
| `map_texture_budget_mb` | int | `256` | LRU cap |

Shortcuts: `toggle_map` (`Ctrl+M`), `show_on_map` (`Ctrl+Shift+M`),
`toggle_map_dock` (unbound by default); check against `DEFAULT_SHORTCUTS` for
collisions before choosing, and add them to `SHORTCUT_LABELS` and the
`"Navigation"` category. Menu: View → "Map" (check item), View → "Undock Map";
Events → "Show on Map". Status bar hint when a route is selected but the map
is closed: none (keep the bar quiet).

### 3.9 Route file compatibility (D9)

No new keys in the MVP. If a pinned position is added later, it is
`"Map Anchor": {"map": "<const>", "x": n, "y": n}` on the event object,
written only when set (`events.rs:662-668` recipe), read permissively, and
carried through undo automatically because `SnapNode::Group` clones the whole
`EventDefinition` (`undo.rs:20-31`).

---

## 4. Pipeline: pokemap → map pack

### 4.1 `pipeline/export-router.js`

```
node pipeline/export-router.js [--games yellow,crystal,…] --router ../pkmn_yellow_xp_router [--repos ./repos]
```

Per game it: (1) loads `public/data/<game>/*.json`; (2) computes the world
layout by importing `src/render/world-map.js` (pure functions, runnable under
Node — this is exactly what the measurement script in Appendix B does) and
writes `layout.json`; (3) converts blockdata/blocksets to the binary blobs
and writes the manifest index; (4) copies tilesets/sprites, `palettes.json`,
`terrain.json`, `sign_text.json`; (5) builds `objects.json` from `events.json`
(unified schema, Appendix A → §3.2); (6) runs the per-gen linkers (§4.2) and
the item normaliser (§4.3) against the router's `raw_pkmn_data`; (7) builds
`encounters.json` (§4.4); (8) merges `map_data/<game>/overrides.json`; (9)
validates (§4.5) and writes `links.json` + `coverage.md`; (10) records source
commit hashes in the manifest.

The exporter never modifies `public/data` (the web app keeps working) and is
idempotent: re-running with unchanged inputs produces byte-identical output so
`git status` shows real changes only.

### 4.2 Trainer linking per generation

Shared rules: every trainer object gets zero or more router trainer names; an
NPC object whose script triggers a battle becomes a `Precision::Script` anchor
for that trainer; a battle found in a map's scripts with no object to attach
to becomes a `Precision::Map` anchor at the map centre; `overrides.json`
entries (`{trainer, map, x, y, note}` or `{trainer, map}`) replace or add
anchors; rematch records (`rematch == true` in the router, or "Rematch" in the
name) inherit the anchors of their base trainer when they have none; records
whose router `location` is `"Unused"` are excluded from the coverage
denominator.

- **Gen 1** (`links/gen1.js`, repos `pokered`/`pokeyellow`): objects with
  `trainerClass`/`trainerNum` → `"<Class> <num>"` (`OPP_JR_TRAINER_M`, 2 →
  `"Jr Trainer M 2"`) matched by `sanitize` against router names. Scripted:
  parse each `scripts/<Map>.asm` for `ld a, OPP_X` / `ld [wCurOpponent], a` /
  `ld a, N` / `ld [wTrainerNo], a`; constant `N` gives one trainer; the rival
  pattern (`ld a, [wRivalStarter]` + `add k`) emits all starter variants
  (Yellow: Jolteon/Flareon/Vaporeon; R/B: three starters) — the route contains
  only one, and the other names simply link to the same tile. Anchor = the
  map's NPC object whose sprite matches (`SPRITE_BLUE` for rivals; the class
  sprite otherwise), else map-level. Gym leaders in gen 1 are ordinary
  trainer objects (`OPP_BROCK, 1`) and already link.
- **Gen 2** (`links/gen2.js`, repos `pokegold`/`pokecrystal`): for each
  `maps/<Map>.asm`, index `object_event` rows (script label column), then
  find `trainer CLASS, CONST` under a label (trainer objects) and
  `loadtrainer CLASS, CONST` anywhere under an NPC's label (scripted battles).
  Resolve `CONST` → index within `CLASS` (`constants/trainer_constants.asm`),
  `CLASS` → display class (`data/trainers/class_names.asm`), the trainer's
  name (`data/trainers/parties.asm`, the `db "NAME@"` line of the
  `(index)`-th entry) → router record by `(trainer_class, trainer_id)` and
  confirm with `sanitize(class + " " + name)`; rival constants
  `RIVAL1_n_STARTER` map to the router's `"Rival1 n Starter"` naming. This
  links all 8 gym leaders, the Elite Four, the Champion, every rival fight and
  the Rocket executive fights that pokemap misses today.
- **Gen 3** (`links/gen3.js`, repos `pokeruby`/`pokeemerald`/`pokefirered`):
  reuse `parseTrainerConstants` and a widened `parseTrainerScriptMapping`
  (`pipeline/gen3/parse-events.js:130-189`) that (a) accepts every
  `trainerbattle*` command, (b) attributes a battle to the enclosing label
  and follows one level of `goto`/`call`/`goto_if_*`/`call_if_*` edges inside
  the same `scripts.inc` so rival/boss scripts reached from an NPC's script
  are attributed to that NPC, (c) records any remaining `TRAINER_*` mention
  in a map's scripts as a map-level anchor. Number → router `trainer_id`.
  Measured baseline before (b): 87 % of objects, 86 % of non-rematch records
  (Emerald).
- **Coverage report**: `coverage.md` lists, per game, totals and every router
  trainer with no anchor, sorted by category (`GenData.trainer_to_category`
  — gym leaders and rivals first), so curation of `overrides.json` is a short
  list, not a hunt. Acceptance for Phase 4: ≥ 95 % of non-rematch, non-unused
  records per game, and 100 % of `major_fights`.

### 4.3 Item name normalisation (`links/items.js`)

Input constants: gen 1 `itemId` (`MOON_STONE`, `TM_WHIRLWIND`), hidden `POTION`
/ `COIN+n`; gen 2 `flag` minus `EVENT_<MAP>_` (`ULTRA_BALL`, `TM_SLEEP_TALK`),
hidden `NUGGET`; gen 3 `finditem ITEM_X` from `item_ball_scripts.inc` (R/S) or
the map's `scripts.inc` (Emerald/FR/LG), hidden `ITEM_X`, berries
`"Oran Berry"`. Rules: strip `ITEM_`; `TM`/`HM` → look up the router item whose
name is `"TMnn <Move>"` by move name (numbers taken from the router's
`items.json`, so `TM_RAIN_DANCE` → `"TM18 Rain Dance"` in gen 3 and
`TM_WHIRLWIND` → `"TM02 Whirlwind"` in gen 1); `HP_UP` → `"HP Up"`,
`PP_UP` → `"PP Up"`, `X_SPECIAL` → `"X Special"`, `PARLYZ_HEAL` vs
`"Paralyze Heal"` and the other spelling differences via a small alias table
kept in the script; then `sanitize` equality against `ItemDB` names. Objects
with no `itemId` and a Poké Ball sprite are fake (Voltorb/Electrode) →
`fake: true`, no link; coins → no link (the router has no coin tracking yet,
`todo.txt`). The report lists every unresolved constant.

### 4.4 Encounter tables (D12)

- Gen 1/2: read the router's `raw_pkmn_data/gen_one/{red_blue,yellow}/*_encounter_tables.json`
  and `gen_two/{gold_silver,crystal}/*_encounter_tables.json` (already map-keyed,
  species names already router names) and emit `encounters.json` with version
  columns where Red/Blue or Gold/Silver differ, methods renamed to the pack's
  names, `base_rate` kept.
- Gen 3: read `src/data/wild_encounters.json` of each repo (`for_maps` group;
  R/S and FR/LG carry per-version `base_label` suffixes, which
  `pipeline/gen3/parse-encounters.js:26-58` already parses), slot rates from
  the `fields` definitions, `SPECIES_X` → router species names.
- Validation: every species must exist in the router's `pokemon.json` for
  that game; every map with grass or water tiles should have a table (the
  report lists exceptions such as Safari Zone areas that use their own
  mechanics).

### 4.5 Validation against router data (fail the export on error)

- Every `links.json` trainer key exists in the router's `trainers.json` for
  the game (`sanitize` equality); every item key exists in `items.json`;
  every species in `encounters.json` exists in `pokemon.json`.
- Every anchor's map exists in `maps.json` and its step lies inside the map.
- `layout.json` positions cover every map that pokemap's `getOutdoorMaps`
  reports, and no two base maps overlap except the documented
  underlay/overlay pairs.
- Blob lengths equal `w · h` (× 2 for gen 3) for every map.

### 4.6 Running it

The decomp repos are not on this Mac (the Dropbox copies are online-only
placeholders). Options: run on the Windows machine where `repos/` live, or
`git clone --depth 1` the seven pret repos into `pokemap/repos/` (pokeemerald
is 91 MB, pokecrystal 27 MB, pokeyellow 18 MB) — the coverage measurements in
§2.3 were made that way. The export takes seconds. Commit the resulting
`map_data/` changes to this repo with the pokemap commit hash in the message.

---

## 5. Phases and steps

Each phase ends with `cargo test --workspace` green, `rust/TESTING.md` updated,
and a screenshot in the PR. Estimates assume one developer.

### Phase 0 — Spike (3–5 days): prove D3 and D8 with numbers

1. Create `rust/crates/xpr-map` (lib crate; deps `xpr-core`, `serde`,
   `serde_json`, `flate2`, `image` (png), `log`, `thiserror`; `rayon` optional
   feature `parallel`). Add it to the workspace `members`.
2. Implement the gen 1 path only, reading pokemap's `public/data/yellow`
   directly through a dev `--data-dir` (no pack yet): `MapDef`/`WorldLayout`
   parsing, a Rust port of `computeWorldLayout` (temporary — Phase 1 moves
   the layout into `layout.json`), tileset decode, palette recolour, block
   compositing.
3. `examples/render_world.rs yellow out.png`: composite the whole Yellow world
   at 1× and compare pixel-for-pixel with pokemap's **Export PNG** (scale 1,
   sprites off, events off) from the web app. Record the diff (expected 0).
4. A throwaway egui binary (`xpr-app/src/bin/map_spike.rs`, not shipped):
   512² chunk textures with nearest magnification + mipmaps, LRU, worker
   thread, pan/zoom. Measure with `XPR_FRAME_LOG=1`: chunk composite time,
   upload time, frame time while panning, memory. Try the level-5 overview.
5. Decide and record in §1: keep runtime compositing (expected) or switch to
   a pre-rendered PNG pack (§8, same `MapPack` API either way). Delete the
   spike binary once Phase 2 replaces it.

### Phase 1 — Data pack and pipeline (1–2 weeks): no UI yet

1. pokemap: write `pipeline/export-router.js` (§4.1) with a `--games` filter;
   start with `yellow`.
2. `layout.json` from `world-map.js`; `objects.json` unification (Appendix A);
   binary blobs + manifest index; copy sheets, palettes, terrain, sign text.
3. `pipeline/links/gen1.js`, `gen2.js`, `gen3.js` (§4.2) and `items.js`
   (§4.3); `encounters.json` (§4.4); `overrides.json` merge; `coverage.md`.
4. Validation (§4.5); run for all seven games; commit `map_data/`.
5. Router: `xpr-map/build.rs` (generic file walk + deflate, feature
   `embed-map-data`; `xpr-app` enables it), `embedded.rs`, `pack.rs`
   (`MapPack::load(game: &str) -> Result<Arc<MapPack>, MapError>`,
   `XPR_MAP_DATA_DIR` override), `model.rs`, `geom.rs`, `links.rs`,
   `spatial.rs`. Map a route version to a game with `builtin_sources()`
   (add a small `map_game_for_version(&str) -> Option<&'static str>` in
   `xpr-data/src/registry.rs`).
6. Tests (`xpr-map/tests/pack.rs`): every game loads; blob lengths match;
   every anchor in bounds; every link key resolves in `TrainerDB`/`ItemDB`
   (test builds a `Registry` on `raw_pkmn_data` like `xpr-engine/tests`);
   `coverage.md` numbers are at or above a checked-in baseline
   (`tests/coverage_baseline.json`) so regressions in the pipeline fail CI.
7. `examples/coverage.rs`: prints the same report from the pack (keeps the
   Rust and JS views of coverage identical).

### Phase 2 — Viewer (2–3 weeks): pan, zoom, world and indoor maps

1. `xpr-map/src/compose.rs`: gen 1, gen 2 (time of day, roof, tileset
   overrides, night variant), gen 3 (metatile cache, flips, two layers,
   FR/LG constants); `lod.rs` downsampling. Golden tests
   (`xpr-map/tests/golden.rs`): hash of selected maps/chunks per game against
   PNGs exported from the web app and checked in under
   `tests/golden/map/*.png` (small: a few maps per game).
2. `xpr-app/src/map/{mod,camera,chunks,layers}.rs`: `MapView`, `Camera`,
   `ChunkCache` with worker + upload budget + LRU + overview job; base layer
   draw; input (drag, wheel, pinch, keys); fit-to-view.
3. Docked tab: `MAP_TAB = 2` in `EventDetails`, `map_left_fraction`,
   tab persistence; pack load on first open in a background thread with a
   spinner; game switch when the route's version changes (`route_loaded`
   signal). Menu View → Map, `toggle_map` shortcut, `map_open` persistence.
4. Undock: `show_viewport_immediate` window, `map_docked`, re-dock on close,
   toolbar ⤢ button, View → Undock Map.
5. `list.rs`/`toolbar.rs`: search + grouped map list, breadcrumb, Back to
   World, warp double-click, zoom buttons, per-game view-state persistence.
6. Performance pass against the §3.4 budget with `XPR_FRAME_LOG`; add the
   status strip counters (chunks drawn, frame ms) behind the same flag.
7. Headless UI test (`xpr-app/tests/map_view.rs`, harness copied from
   `compare_page.rs`): drag pans by the expected delta, wheel zooms about the
   cursor, fit centres the world, scope switch and back. Smoke action
   `XPR_SMOKE_ACTION=map` (opens the tab on the loaded route, waits for the
   visible chunks, screenshots).

### Phase 3 — Objects and info (1–2 weeks)

1. Marker layer with toggles and zoom-dependent labels; hover highlight.
2. `hit.rs` + `spatial.rs`; click → `InfoCard` (`cards.rs`) for trainer
   (party from `TrainerDB`, icons via `Assets::pkmn_icon`), item, hidden item,
   berry, sign, warp (Go), and grass/water encounters with version columns.
3. Card positioning and dismissal rules (Esc, click elsewhere, pan/zoom
   keeps a pinned card); keyboard focus handling so typing in the search box
   never pans the map.
4. Tests: hit-testing unit tests in `xpr-map` (object under pointer,
   threshold, overlapping maps); headless click → card kind.

### Phase 4 — Links: the MVP (2–3 weeks)

1. `state.rs`: `RouteMapState` rebuilt on `route_changed` (defeated trainers
   from `Router::defeated_trainers`, acquired items by walking `all_groups()`
   for acquire-without-money inventory events, the selected event's anchors);
   routed-state marker styles.
2. **Show on map**: `XprApp::show_selected_on_map()` — query from the selected
   definition (§3.7), `links.resolve`, open the tab/window if needed,
   `MapView::focus(anchors, label)` (scope switch, animated centre, pulse,
   focus banner with ◀ ▶ for several anchors), toast "No map location known
   for X" otherwise. Wire the button in the editor title strip
   (`editors.rs:1703-1735`), Events → Show on Map, and `show_on_map`.
3. **Add to route** actions (`MapAction` returned by `MapView::ui`, applied in
   `XprApp` next to `apply_list_actions`, `app.rs:1864-1873`): trainer
   (variants picker, double battles), item / hidden / berry, wild from an
   encounter row (level picker), "Add all trainers here" (new folder named
   after the map). Selection follows the new event; the route list refreshes
   through the existing signals.
4. **Select in list** from the focus banner and from a marker's card when the
   object is already routed (`ctrl.select_new_events`).
5. Curation: run the exporter, read `coverage.md`, write `overrides.json` for
   Yellow first (rivals, Jessie & James), then Red/Blue, Crystal, Gold/Silver,
   Emerald, Ruby/Sapphire, FireRed/LeafGreen until each meets the §4.2
   acceptance numbers. Commit `map_data/` and the baseline.
6. Tests: controller tests (`xpr-app/tests/map_actions.rs`) — add-from-map
   inserts after the selection, undo restores the previous `save_bytes()`,
   add-all creates the folder in object order and skips defeated trainers;
   link tests — every `major_fights` trainer of every game resolves to ≥ 1
   anchor; headless "show on map" test on `yellow-pinsir-lv10brock.json`.
7. `rust/TESTING.md` §17 "Map" manual checklist; `PORT_STATUS.md` row for
   `xpr-map`; this spec's §12 "Changes made during implementation".

### Phase 5 — Polish (1–2 weeks)

1. Sprites: `sprites.rs` frame extraction (gen 1/2 shade → palette; gen 3
   chroma key), one atlas texture per game, facing from movement/direction,
   berry-tree mature frame; markers become sprites above zoom 1.
2. Night mode (gen 2) as a chunk `variant`; NPC palettes at night.
3. Item finder (item dropdown → cycle anchors) reusing the focus banner.
4. Export PNG: composite the world (or the current view) at 1–4× with markers
   through the existing `screenshot.rs` rasteriser for the overlay, or by
   direct chunk composition; `XPR_SMOKE_EXPORT=map`.
5. Keyboard navigation between markers (Tab/Shift+Tab within the viewport),
   "follow selection" option (map recentres when the selected event has an
   anchor), land:water HUD if wanted.

### Phase 6 — Packaging and hardening (1 week)

1. Size budget: `windows_build.py`/`mac_build.py` print the embedded pack
   sizes; assert ≤ 3.5 MB for the map pack; check start-up time is unchanged
   (the pack is not touched until the map opens).
2. Memory: verify the LRU budget on a 4 GB machine; expose
   `map_texture_budget_mb`; drop level-0 chunks when the map is closed.
3. `docs/rust_port/KNOWN_ISSUES.md` entries for anything left (e.g. maps
   without blockdata: 4–5 per gen 1/2 game are placeholder-only, as in
   pokemap).
4. Release notes; update `README.md`.

### Phase 7 — Gen 4/5 (deferred; own spec)

Blocked on an image source that can be shipped: run pokemap's apicula/trimesh
renderer for Platinum (and write the missing Diamond/Pearl and HGSS pipelines)
or another ROM-derived render; Bulbapedia images are out (§2.7). Also: fix
Platinum's visible-item extraction, link `LOCALID_*` objects to `TRAINER_*`
constants through the map scripts and to router `rom_id`s, re-enable
`trainer_location` in the gen 4/5 loader, and add a pre-rendered image scope
to `xpr-map` (a per-map PNG at 16 px/tile behind the same chunk API).

---

## 6. Testing

- **Pipeline** (JS, in pokemap): `node pipeline/export-router.js --check`
  re-exports to a temp dir and diffs against `map_data/` (must be identical);
  coverage numbers per game printed and compared to the baseline.
- **`xpr-map` unit/golden tests**: pack loading, geometry, layout invariants,
  compositor goldens against pokemap PNG exports, LOD downsampling, hit-tests,
  link resolution, coverage baseline.
- **`xpr-app` headless tests**: `MapView` input (drag/zoom/keys), card
  opening, actions; controller-level add-from-map/undo; show-on-map.
- **Smoke**: `XPR_SMOKE_ACTION=map` (+ `XPR_SMOKE_ROUTE`) screenshots the
  docked map on the loaded route from an isolated config; `XPR_SMOKE_EXPORT=map`
  once export exists.
- **Manual** (`rust/TESTING.md` §17): open/undock/dock, every toggle, warp
  follow, grass click, add trainer/item/wild, show on map for a gym leader
  and a rival in each generation, night mode, item finder, performance feel
  on the Emerald world at every zoom, memory after 10 minutes of panning.
- **Golden corpus**: unchanged; route files are not touched by the MVP.

## 7. Packaging and size budget

| item | today | after Phase 4 |
|---|---|---|
| `xpr-data` pack (embedded) | 2.4 MB | 2.4 MB |
| map pack (embedded, 7 games) | — | ≈ 2–2.5 MB (JSON-deflated map data 1.3 MB measured; tilesets + sprites 1.06 MB; sign text 0.44 MB; blobs instead of `blocksets.json` remove ~0.6 MB) |
| macOS release zip | 14.6 MB | ≈ 17 MB |
| start-up | unchanged | unchanged (lazy) |

The pack is a separate build feature so a build can omit it; the loader
reports "map data not included in this build" instead of panicking.

## 8. Risks and mitigations

| risk | mitigation |
|---|---|
| Rust compositor differs from pokemap's pixels (shade thresholds, gen 2 roof/tileset overrides, gen 3 FR/LG constants) | Phase 0 pixel diff + Phase 2 goldens from the web app's own export; the pack keeps the untouched sheets, so any fix is code-only |
| Compositing turns out too slow or too complex (Phase 0 says no) | Fallback recorded here: pre-render per-map PNGs in the exporter (pngjs, already a pokemap dependency) into a `maps/<const>.png` scope behind the same `MapPack`/chunk API; cost ≈ 20–40 MB embedded |
| GPU limits: `max_texture_side` < 512 or tiny VRAM | 512 is safe on every GL 2/ES 2 device; the LRU budget is configurable; the overview levels are ≤ 48 MiB |
| Thousands of markers/labels per frame | culling per map, labels only above zoom 1.5, sprites via one atlas mesh |
| Decomp repos unavailable when the pack must be regenerated | pipeline is fully scripted; shallow clones suffice (verified); pack + baseline are committed, so the router builds without them |
| Trainer identity edge cases (rival starter variants, rematches, duplicated names, "Unused") | exact rules per gen + overrides + a report that lists every leftover; acceptance thresholds enforced by tests |
| Item naming drift between games (TM numbers, spellings) | normaliser keyed on the router's own `items.json` per gen; unresolved constants listed |
| Splitter/tab UX on narrow windows | 200 px minimum already enforced; undock is one click; three-column layout deferred (§9) |
| Binary size and build time | ≈ 2.5 MB, deflated at build time and cached in `OUT_DIR` like `xpr-data` |

Open questions for the user (defaults chosen; say so to change them):

1. Docked map as a **tab of the right pane** (chosen) versus a third column.
2. Persist "map open" across launches (chosen: yes).
3. Show NPC markers at all (chosen: no by default, toggle available).
4. Crate name `xpr-map` (chosen).
5. Whether "Add all trainers here" should also add the map's item balls.

## 9. Deliberately left for later

- Three-column editor layout (list | details | map) with two splitters.
- Per-event pinned anchors in route files (`"Map Anchor"`, §3.9) for
  trainers the game code cannot place.
- Route path overlay (numbered markers / lines in route order) and
  "follow the recorder" (gen 2 already exposes map group/number/x/y).
- Gen 4/5 (Phase 7). Custom gens with their own maps.
- Coins, Safari Zone mechanics, headbutt/rock-smash encounter UI beyond a
  table.
- Sign text search, land:water HUD, EV yields in encounter tables (the router
  has EV data in `PokemonSpecies.stat_xp_yield`; trivial to add to the card
  later).

---

## Appendix A — pokemap event schemas per generation (as they are today)

| gen | trainer object | item ball | hidden item | berry | sign | warp |
|---|---|---|---|---|---|---|
| 1 | `objectEvents[] {x,y,sprite,movement,direction,textLabel,type:"trainer",trainerClass:"OPP_ROCKET",trainerNum:5}` | `{…,type:"item",itemId:"MOON_STONE"\|null}` | `hiddenItems[] {x,y,item:"POTION"\|"10 Coins"}` | — | `bgEvents[] {x,y,textLabel}` | `warps[] {x,y,destMap,destWarpId}` (`LAST_MAP` resolved) |
| 2 | `{x,y,sprite,movement,pal,objectType:"OBJECTTYPE_TRAINER",sightRange,script:"TrainerSchoolboyChad1",flag,type:"trainer"}` | `{…,objectType:"OBJECTTYPE_ITEMBALL",script:"Route42UltraBall",flag:"EVENT_ROUTE_42_ULTRA_BALL",type:"item"}` | `{x,y,item:"NUGGET"}` | `{…,type:"berry",berry:"Berry"}` | `bgEvents[] {x,y,type:BGEVENT_*,script}` | same |
| 3 | `{x,y,sprite:"OBJ_EVENT_GFX_*",movement,script:"Route102_EventScript_Calvin",flag,type:"trainer",trainerType,sightRange}` (+ `trainerConst`,`trainerRomId` in R/S) | `{…,type:"item",script:"…_EventScript_ItemPotion",flag:"FLAG_ITEM_…"}` (+ `itemId` in R/S) | `{x,y,item:"ITEM_NUGGET"}` | `{…,type:"berry",berry:"Oran Berry"}` | `bgEvents[] {x,y,type:"sign"\|"secret_base",script}`; `coordEvents[] {x,y,type:"trigger"\|"weather",script}` | same |
| 4 | `trainers[] {x,y,script:null,trainerType,graphicsId,id:"LOCALID_*",sightRange}` | `items[] {x,y,itemId:"REPEL",script:7001,graphicsId}` | `items[] {x,y,hidden:true,script:8161}` | — | `signs[] {x,y,script:22,…}` | `warps[] {x,y,destMap,destWarpId}` |

Map sizes: gen 1/2 `width`/`height` in 32 px blocks; gen 3 in 16 px
metatiles; gen 4 in 16 px tiles from 32-tile matrix cells. Object coordinates
are 16 px steps in every generation.

## Appendix B — measurement scripts (2026-09-24)

Run from `~/Documents/pokemap` with Node 20 (`/usr/local/bin/node`); they
import pokemap's own modules, so numbers stay comparable when the data changes:

| script (session scratchpad) | what it measured |
|---|---|
| `world_dims.mjs` | world size per game via `computeWorldLayout`/`normalizeLayout`, outdoor/indoor counts, object counts (§2.3 tables 1–2) |
| `match_coverage.mjs` | `matchTrainer` over every trainer object; records with ≥ 1 position (§2.3 table 3) |
| `scripted_fallback.mjs`, `sprite_fallback.mjs` | what NPC-script / sprite-name heuristics would add for gen 2 (little; hence the `loadtrainer` rule) |
| `gen3_exact_link.mjs` | exact `scripts.inc` → `opponents.h` linking on a fresh `pokeemerald` clone (87 % objects, 86 % non-rematch records) |
| `item_coverage.py` | pokemap item names vs router `items.json` (51–81 %; TM numbering) |
| `encounter_link.mjs` | grass-bearing maps resolved by pokemap's display-name heuristics; router/decomp map-keyed tables checked with Python one-liners |
| Python one-liners | deflated sizes per file class (§2.3 sizes table) |

Copies of these scripts belong in `pokemap/pipeline/measure/` once Phase 1
starts; the exporter's `coverage.md` supersedes the coverage ones.

## Appendix C — egui 0.33.3 API used by the design (verified in the local registry)

- `Context::load_texture(name, impl Into<ImageData>, TextureOptions) -> TextureHandle`
  (`egui/src/context.rs:2247`); `TextureHandle::set_partial` / `set` / `size`.
- `TextureOptions { magnification, minification, wrap_mode, mipmap_mode: Option<TextureFilter> }`
  (`epaint/src/textures.rs:153-175`); mipmaps generated by `egui_glow`
  (`painter.rs:627-628`); `TextureFilter::{Nearest, Linear}`, `TextureWrapMode::ClampToEdge`.
- `InputState.max_texture_side` (`egui/src/input_state/mod.rs:289`); glow reports
  `GL_MAX_TEXTURE_SIZE`.
- `Mesh::with_texture(TextureId)`, `Mesh::add_rect_with_uv(Rect, Rect, Color32)`
  (`epaint/src/mesh.rs:65,191`), `Shape::mesh(impl Into<Arc<Mesh>>)`
  (`epaint/src/shapes/shape.rs:341`), `Painter::image(TextureId, Rect, Rect, Color32)`
  (`egui/src/painter.rs:488`).
- Input: `Response::drag_delta()`, `dragged()`, `clicked()`, `double_clicked()`,
  `secondary_clicked()`, `hover_pos()`; `InputState.smooth_scroll_delta`,
  `raw_scroll_delta` (`input_state/mod.rs:248,262`), `zoom_delta()` (`:638`),
  `multi_touch()` (`:900`); `Event::MouseWheel { unit, delta, modifiers }`.
- Windows: `Context::show_viewport_immediate` (`context.rs:3943`), as used by
  the summaries; `show_viewport_deferred` is not used anywhere in the app.

---

## 12. Changes made during implementation (2026-09-24)

What was built (phases 0–4) and where it departs from the plan above:

- **Pipeline**: `pokemap/pipeline/export-router.js` with helpers under
  `pipeline/router/` (`common.js`, `routerdata.js`, `objects.js`,
  `blobs.js`, `items.js`, `encounters.js`, `links-gen1.js`, `links-gen2.js`,
  `links-gen3.js`) — not `pipeline/links/`. The decomps are shallow clones
  in `pokemap/repos/` (gitignored). `--check` re-exports to a temp dir and
  diffs. The export is byte-identical on re-run.
- **Pack**: as §3.2, except that warps are objects in `objects.json` (kind
  `warp`) rather than a `maps.json` field, encounter slot `rate` is a float
  (gen 2 tables carry fractional rates), and `manifest.sources` records the
  pokemap / decomp / router commit hashes (no timestamp, for idempotency).
  Measured: 2.80 MB deflated for the seven games (16 MB on disk).
- **Links**: the exact rules of §4.2 plus: gen 1 parses `parties.asm` and
  joins by party (Yellow's Jessie & James and every rival variant resolve);
  gen 2 joins by party, then class + index, then name, then a unique party
  anywhere (Red in Mt. Silver needed an override — `map_data/crystal/
  overrides.json`, `gold_silver/overrides.json`); gen 3 merges the shared
  `data/scripts/*.inc` into every map's label graph (FireRed keeps all trainer
  battles in `trainers.inc`), follows three `goto`/`call` levels, handles
  pokeruby's version-conditional constants (`constants/version.inc`) by
  linking both versions' trainers, and ignores `TRAINER_BATTLE_*` / `LOCALID_*`
  arguments. Trainers the router loader skips (`location == "Unused"`) are
  never linked or named; off-map objects become map-level anchors.
  Coverage (eligible = not rematch, not unused): yellow 343/343, red_blue
  346/352, gold_silver 360/377, crystal 375/392, ruby_sapphire 406/451,
  emerald 542/560, firered_leafgreen 471/489. The gen 2 remainder is the S.S.
  Aqua cabins (pokemap's `constToCamel` cannot find
  `FastShipCabins_NNW_NNE_NE.asm`, so those maps have no events at all);
  the gen 3 remainder is unplaced beta trainers and Frontier brains.
- **Items**: TM/HM numbers are resolved through the router's own item names;
  gen 2 event flags are searched word-window by word-window
  (`EVENT_PICKED_UP_CHARCOAL_FROM_HO_OH_ITEM_ROOM` → Charcoal); a species name
  in the identifier marks a gift Pokémon ball. Gen 1/2 unresolved: Voltorb /
  Electrode decoys and coins only.
- **`xpr-map`**: as §3.1, with `lod.rs` holding the `Pixmap` type. The
  compositor is ~5× faster than budgeted (Emerald's whole world in 79 ms
  release), so the LOD pyramid is built by rendering sub-blocks on demand —
  no separate overview job; the viewer just requests levels ≥ 3 once after
  loading. Tests: `crates/xpr-map/tests/pack.rs` (5 tests incl. a coverage
  baseline and a LOD-consistency check).
- **Viewer** (`xpr-app/src/map/`): as §3.6 with a 30 px toolbar (◀ World,
  map name, search + grouped list popup, T/I/?/W/S/B/N chips, Night for gen 2,
  − % + Fit, undock/dock, close) and an 18 px status strip (map, step,
  terrain; chunks pending/cached; frame ms under `XPR_FRAME_LOG`). Cards as
  §3.6; the wild table adds a "+" per slot. The camera eases over 260 ms;
  chunk uploads are capped at 4 per frame; level 0/1 chunks live in a 256 MiB
  LRU (`map_texture_budget_mb`). Textures use nearest magnification, linear
  minification and mipmaps. The per-game view (scope, map, zoom, centre) is
  persisted in `map_view_state`.
- **App**: `MAP_TAB = 2` in `EventDetails` with its own splitter fraction
  (`map_left_fraction`); auto-switch leaves the Map tab alone; a "Map" menu
  (Show Map Ctrl+M, Show Selected Event on Map Ctrl+Shift+M, dock/undock);
  the editor title strip's "Map" button for trainer / pickup / wild events;
  the undocked window is `show_viewport_immediate("world_map")`; the
  add-to-route actions live in `MainController` (`add_trainer_fight_from_map`,
  `add_item_pickup_from_map`, `add_wild_from_map`, `add_trainers_in_new_folder`)
  so they are testable headlessly. `XPR_SMOKE_ACTION=map` (+ `XPR_SMOKE_EVENT`).
  Tests: `xpr-app/tests/map_actions.rs`, `map_view.rs`, `embedded_map_data.rs`.
- **Overworld sprites** (Phase 5 step 1, done 2026-09-24): `xpr-map/src/
  sprites.rs` ports pokemap's sprite renderer (gen 1/2 shade strips with the
  map's CGB palette through the OBP0 remap or the `PAL_NPC_*` palette of the
  map's time of day, night included; gen 3 frame rows with the key colour
  removed, berry trees on their grown frame, right = mirrored left). The
  exporter now carries each object's gen 2 `pal`. `xpr-app/src/map/sprites.rs`
  packs frames into 1024² atlas pages (shelf packer, `set_partial` uploads),
  so all sprite markers draw as one mesh per page; sprites replace circles for
  trainers, NPCs, item balls and berry trees at zoom ≥ 0.75 (toggle "Sp"),
  taller frames rise above their step, routed ones are dimmed with a check
  badge. NPC objects whose script starts a battle (gym leaders, the Elite Four,
  scripted rivals) count as trainers for toggles, styling and hit-testing
  (`MapObject::effective_kind`). Tests: `xpr-map/tests/sprites.rs`, the atlas
  test in `xpr-app/tests/map_view.rs`.
- **Not done yet** (Phase 5–7): the item finder, PNG export, keyboard marker
  navigation, "follow selection", the three-column layout, per-event pinned
  anchors, gen 4/5.
