# Gen 4/5 world maps: what pokemap must produce for router parity

Date: 2026-09-26. **Audience: the agent working in pokemap.** This document
says what pokemap's new gen 4/5 data (commit `9ff6859` "gen4 & 5 maps", plus
the uncommitted mask and Diamond/Pearl work) already gives the router. It also
says what is missing and the exact export the router needs. The router will
implement its side only after pokemap delivers this export.

- pokemap: `A:\Dropbox\stp-projects\programs\pokemap` (GitHub
  `Scotts-Thoughts/pokemap`, only `main` exists and it is at `9ff6859`).
- Router: `A:\pkmn_yellow_xp_router`. The gen 1–3 integration is specified in
  `docs/rust_port/design/world_map/SPEC.md`. Read §3.2 (pack format v1), §4
  (pipeline and linking rules) and §12 (what was actually built). Also read
  `GRAPHICS_TOOLS_PLAN.md` in the same folder, which lists the viewer tools.
- Games: `diamond_pearl`, `platinum`, `heartgold_soulsilver`,
  `black_white`, `black2_white2`. These are pokemap's ids and also the
  router's `raw_pkmn_data/gen_four|gen_five/<id>/` folder names.
- Every number below was measured on 2026-09-26 against the working tree.
  The scripts are listed in Appendix A.

---

## Status 2026-10-01 (read this first; it replaces the gap table in §0 and the order in §7)

**Everything is done: P1–P6 in pokemap, and the router side (§8). Gens 4/5
show in the Map tab.** P6 (imagery) is done for all five games. The work is on pokemap branch `router-export` (not yet merged or pushed).

| step | status |
|---|---|
| P1: commit the exporter | ✅ `export-router.js` + `pipeline/router/` committed. The Emerald Route 103 fix is ported into `src/render/world-map.js` (`LAYOUT_HEIGHTS`, `DRAW_ABOVE`; the Route 114 nudge is gone). `--check` is byte-identical for all seven gen 1–3 packs apart from `manifest.json`. |
| P2: residuals | ✅ No builder changes were needed. Every DS map with grass/water but no table has `ENCOUNTERS_NONE` / `ENCDATA_NA` in its decomp header (HGSS's Safari Zone gate has an all-Rattata placeholder that pokemap skips on purpose). They are listed in `coverage.md`. Dynamic warps already had `candidates`. The Oreburgh sign outside its map is dropped and listed. Rematch records, and gen 5 id-suffixed copies (`"Smasher Elena (1)"`, Challenge/Easy Mode and second-round ids), inherit their namesakes' anchors. |
| P3: format 2 exporter | ✅ `pipeline/router/ds.js`, dispatched from `export-router.js`. It does all §6 validation (including the §5.1 party guard) and `--check` for all twelve games. |
| P4: masks | ✅ The resolved auto-mask, with the hand edits from `public/data/<game>/masks.json` composed over it, goes into each pack's `masks.json`. The user's world-mask edits are in (pokemap `c9a8301`, 2026-10-02) for Pt, HGSS, BW and B2W2. DP has none, so it uses the automatic mask. No interior was hand-edited. |
| P5: metadata export | ✅ `map_data/{diamond_pearl,platinum,heartgold_soulsilver,black_white,black2_white2}/`, ≈ 0.45–0.7 MB deflated each (≈ 2.7 MB in total). |
| P6: imagery | ✅ All five games were re-rendered on the Mac (M1, moderngl over OpenGL 4.1, apicula built from source). Gen 5 used the retail US ROMs from `~/Dropbox/stp-projects/programs/roms/originals/`. The exporter writes each game's pictures to `map_data/<game>/imagery.zip` at quality 92: DP 87 MB, Pt 91 MB, HGSS 72 MB, BW 47 MB, B2W2 68 MB. The app executable embeds them (§8), so every user has them. |

Coverage, per `coverage.md`:

| | DP | Pt | HGSS | BW | B2W2 |
|---|---|---|---|---|---|
| eligible trainers anchored | 447/447 | 448/448 | 438/438 | 454/454 | 622/622 |
| key trainers missing | 0 | 0 | 0 | 0 | 0 |
| item objects resolved | 610/610 | 699/699 | 523/523 | 429/429 | 595/595 |
| maps with an encounter table | 156 | 156 | 138 | 112 | 134 |

The B2W2 Lass Helia mismatch was not a real mismatch. The router's gen 5
data writes ♂/♀ as the font glyphs `⑭`/`⑮` (and "PKMN" as `⒆⒇`). The
exporter folds them for matching only. Router trainer names are untouched,
because saved routes refer to them. Cleaning the raw data (§8) is still
worthwhile for display, but it renames trainers, so it needs a route
migration.

Regenerate with `cd ~/Documents/pokemap && node pipeline/export-router.js
--router ../pkmn_yellow_xp_router [--games platinum,…] [--check]`. It reads
only pokemap's committed `public/data/<game>/*.json`, so it runs on any
machine. `python3 pipeline/ds/router_audit.py --router
../pkmn_yellow_xp_router -v` is the independent cross-check.

### Rendering on the Mac (P6)

- **Decomp commits.** The gen 4 builders were written against pret clones
  made on 2026-03-07; newer upstream renamed files they read. Pin
  `pokemap/repos` to the same upstream commits: pokediamond `038ccca`,
  pokeplatinum `007a26c`, pokeheartgold `d11b7ef`. Each is the last commit
  before 2026-03-08 03:21 UTC (`git fetch --depth 1 origin <sha> && git checkout <sha>`).
- **apicula.** `git clone https://github.com/scurest/apicula repos/apicula &&
  cargo build --release`. `pipeline/ds/apicula.py` finds `target/release/apicula`.
- **Gen 5 ROMs.** Use the unmodified `originals/` (header codes IRBO/IRAO/IREO/IRDO). The
  top-level `5-* v1.nds` files in that folder are edited builds.
- **Python.** Use a venv with `numpy pillow moderngl ndspy`, then run
  `python pipeline/gen4/build.py <game> [--world-quality N]` (or `gen5/build.py`).
  Each game takes 1.5–3 min.
- **Output.** The maps, world layout, trainers, encounters and sprite lists
  are identical to the Windows build. Per-tile `lift` differs on chunk seams,
  where the GPU breaks depth ties differently (a few hundred tiles per game,
  plus single outliers). So `tiles.json`/`events.json` were re-committed
  from the same run as the images, and the packs were re-exported. Imagery
  and lift always have to come from the same render.
- **Imagery zip.** It is stored (uncompressed; webp doesn't deflate), with
  `imagery.json` first, then `world/<L>/<x>_<y>.webp` and
  `interiors/*.webp`. `imagery.json` is `{game, version, tile_size, levels,
  w_px, h_px, top_px, world: {level: ["x_y", …]}, interiors: [...], bytes:
  {path: n}}`. `version` is a content hash, not a counter.

### Format 2 as built: details §4 left open, or where it differs

The router loader should follow these; they win over §4 where they disagree.

- **`manifest.json`**
  - `encounter_methods` and `encounter_conditions` are `{id: label}`
    objects, taken from pokemap's `_methods`/`_conditions`, e.g.
    `"walk_spots": "Rustling Grass"`, `"dual_firered": "FireRed (slot 2)"`.
  - `blobs.classes` / `blobs.lift` entries are `{map, offset, len}`, with
    offset and len in **bytes**.
  - `imagery` is `{version, zip, bytes, sha256, files}`, or `null` while a game has no renders.
  - `sources.repo` is the decomp head for gen 4 and `null` for gen 5 (built
    from ROMs).
- **`maps.json`** fields:
  - `id`, `const`, `display`, `location`, `category`, `kind` (`outdoor` | `indoor`), `w`, `h`, `can_fly`
  - outdoor maps: `pos`
  - indoor maps: `image` and `image_origin`
  - There is no `name` field: `display` is the name.
- **`layout.json`** is `{positions, ownership}` and has no `draw_order`. An
  ownership cell is 32 tiles, and `cells` is row-major, holding a MapId or -1.
- **`objects.json`** additions:
  - `lift` (px, omitted when 0)
  - `version` (B2W2's version-exclusive objects, e.g. `"Black 2"`; absent means both)
  - payload `flag` (the visibility flag, a number or a `FLAG_*` string, as the source has it)
  - warp payload: `dynamic`, plus `candidates` (list of map consts)
  - item payload: `count`
  - `kind: "obstacle"`, with payload `{obstacle: "cut" | "rock_smash" | "strength", label}`
  - NPC payload `label` is always `null`; DS NPCs carry `text_key` instead
  - `sprite` is `null` when there is no sheet (signposts, which are part of the imagery)
- **`links.json`**:
  - Map-level anchors (`precision: "map"`) use the map's centre tile.
  - `source` is one of `trainer`, `script:coord`, `script:level`,
    `trainer-outside-map`, `inherit:<name>`, `override`; for items it is
    the object kind.
- **`encounters.json`** has the format 1 shape. Method keys are
  `<method>[_<condition>]` from the manifest. Time of day: the unsuffixed
  table is day. A condition table exists only where it changes some slot. BW
  seasons: `_spring` … `_winter`.
- **`sign_text.json`** holds signs and NPCs. The keys are `<map const>#<n>`.
- **`masks.json`** is
  `{cell_px: 16, world: {w, h, rle}, interiors: {<image path>: {w, h, rle}}}`.
  `rle` is `[value, count, …]` and 1 = visible. **Cells outside a surface's
  `w × h` are hidden.** Without the interior images, an interior surface is
  sized to its maps' extent plus a margin wider than any growth step. So it
  can be smaller or larger than the image, and the visible result is the
  same. A surface hand-edited in pokemap's mask editor uses the image's real size.

---

---

## 0. Summary

**Parity target.** For gen 1–3 the router ships a Rust/egui port of the
pokemap viewer, docked beside the route's event list. It has a world view and
indoor maps, warps, toggleable markers with sprites, and info cards. Its
routing features are *Show on map*, *Add to route* (trainer, item, wild Pokémon
from an encounter row, "add all trainers here"), markers dimmed once routed,
and trip paths. It also has export/copy to PNG, marquee, ruler, grid, labels,
a navigator, and trainer/item finders. The data comes from a *map pack* that
pokemap's `pipeline/export-router.js` writes into the router's `map_data/<game>/`.
It is linked to the router's own trainer/item/species identities at export
time, with no runtime string matching.

**What pokemap's DS output already gives us:**
- A pre-rendered, tilted-3D world as a 512 px webp tile pyramid with the same
  tile size and level maths as the router's own chunk cache.
- Pre-rendered interiors.
- Per-tile walk/grass/water classes and terrain lift.
- Warps, trainer, item, hidden-item, berry, obstacle, NPC and sign objects.
- A trainer id on trainer objects that is the router's `rom_id`.
- Encounter tables whose species names match the router's 97–100 %.
- Pre-coloured overworld sprites.

**What blocks parity** (details in §3):

| # | gap | size of the problem |
|---|---|---|
| G0 | The gen 1–3 exporter (`pipeline/export-router.js`, `pipeline/router/*.js`) was never committed. It exists only in the Mac working tree. | Blocks everything: gen 4/5 export has to extend it. |
| G1 | Trainer links | Router trainers with a map position: DP 80 %, Pt 76 %, HGSS 83 %, BW 57 %, B2W2 55 %. Target ≥ 95 % plus every gym leader, Elite Four, champion, rival and boss. Rivals are 0–4 % everywhere. Gen 5 drops every flag-gated object, which removes most gym leaders, N and Ghetsis. |
| G2 | Item names | DP 67 %, HGSS 74 %, BW 74 %, B2W2 79 % match the router. Bare `TM01` has no move name, and there is `Pok\ufffd Ball` mojibake. |
| G3 | Encounters | Gen 4 tables fold swarm, time of day, radar and dual-slot into one anonymous list. There are no version columns in any DS game and no gen 5 seasons. Gen 5 tables are curated JS matched by content, not read from the ROM. |
| G4 | Map names | 130/399 (BW) and 248/588 (B2W2) maps are named `"<Location> (#<zone>)"`. |
| G5 | Warps | Unresolved destinations: DP 6, Pt 10, HGSS 9. Includes Platinum warps to DP-only headers and HGSS elevators. |
| G6 | DP tile classes | A one-line parser fix is uncommitted. Before it, 91 % of DP tiles read as grass. |
| G7 | Masks | The mask editor is uncommitted and no `masks.json` exists yet. |
| G8 | Size | Imagery is 46–91 MB per game, ≈ 363 MB for the five games. The router embeds its whole pack (2.8 MB today), so imagery cannot go in the binary. |

**The work, in order** (details in §7): commit the exporter (G0). Fix the
data at the source in the DS builders (G1–G6), which also improves pokemap's
own web viewer. Finish masks (G7). Extend the exporter with pack format 2 (§4)
and the gen 4/5 linking rules (§5). Emit the imagery as a separate artifact
(G8, §4.12). Validate (§6) and commit the metadata to the router.

**Decisions the user has to make** are listed in §9. The export in §4 is
designed so that none of them block the pokemap work.

---

## 1. Parity target: router features and the data each one needs

| router feature (gen 1–3 today) | data it needs from the pack | DS source today | status for DS |
|---|---|---|---|
| World view with LOD, pan/zoom, navigator minimap | composited chunks at levels 0..max | `world.json` + `world/<L>/<x>_<y>.webp` | ✅ usable as-is (§4.12) |
| Indoor maps (`Scope::Map`), Back to World | per-map tiles, placed at the origin | `interiors/*.webp` + `imageX/imageY` | ✅ usable as-is |
| Map list/search grouped Cities / Routes / Indoors | display name, kind | `maps.json` `name`, `category`, `world` | ⚠ gen 5 names (G4) |
| Warps, double-click to follow | warp objects, `dest_map`, `dest_warp` | `events.json` warps | ⚠ G5 |
| Marker toggles: trainers/items/hidden/berries/signs/NPCs/warps | typed objects in step coords | `events.json` | ✅, plus the new kind `obstacle` |
| Sprite markers, routed ones dimmed with a check | sprite sheet + facing | `sprites.json` + `sprites/*.png` (RGBA, 4 frames d/u/l/r) | ✅ |
| Trainer card (party from the router's `TrainerDB`) | `trainer` = router trainer name | `trainerId` = ROM id = router `rom_id` | ⚠ G1 coverage |
| Item / hidden item / berry cards, **Add pickup** | router item name, quantity | `itemId`, `item` ("Name ×N"), `berry` | ⚠ G2 |
| Sign card | sign text | `bgEvents[].text` inline | ✅ (move text into `sign_text.json`) |
| Click grass/water → encounter card, version column, EV yield, **Add wild** | per-tile terrain class, map → table, version columns | `tiles.json` `cls`, `maps.json.encounters` | ⚠ G3 |
| Gen 2 night palettes | — | — | n/a (the DS equivalent is gen 5 seasons, optional, §4.12) |
| **Show on map**, focus banner, trainer/item finder, "follow selection" | `links.json` anchors | from the trainer and item links | ⚠ G1, G2 |
| **Add all trainers here** / in selection | linked trainer objects per map | same | ⚠ G1 |
| Trip path (one folder), warp projection to the outdoor door | anchors + warps | same | ⚠ G1, G5 |
| Export PNG / copy view (1–8×, layers, transparent outside maps) | chunks + an "is this pixel a map" test | pyramid tiles have alpha outside maps | ✅ |
| Grid (step/block lines, map outlines), map-name labels | map rects, step size | `maps.json` rects, 16 px tiles | ✅ |
| Marquee / ruler (snapped to steps) | step size | 16 px tiles | ✅ |
| Status strip: map, step and terrain under the cursor | map at pixel, tile at pixel | `chunkMaps` + `tiles.json` (`cls`, `dyRle`) | ✅ (§4.5 algorithm) |
| Least-steps paths (SPEC §13.3, not built for any gen) | walkability per step | `tiles.json` `.`/`#` | head start: DS already has walkability |

Router rules that constrain the export (SPEC D6, D11, D12):
- Links are resolved at export time and keyed by the router's own identities.
  Never match strings at runtime.
- The pack carries no trainer parties and no Pokémon icons. The router has
  both already.
- Encounter tables are keyed by map id.
- Version differences are columns, and the route's version picks one.

---

## 2. What pokemap's DS output is today

Schema reference: `pipeline/ds/output.py:1-25`. Builders:
`pipeline/gen4/{build.py, source_dp.py, source_pt.py, source_hgss.py, extras_hgss.py}`,
`pipeline/gen5/{build.py, source.py, rom.py, scripts.py, script_cmds.py, rails.py, naming.py, extras.py}`,
and the shared `pipeline/ds/*`.

Current outputs per game in `public/data/<game>/`: `world.json`, `world/`,
`interiors/`, `maps.json`, `events.json`, `tiles.json`, `trainers.json`,
`encounters.json`, `sprites.json`, `sprites/`, `icons/`, `ev_yields.json`.
Stale leftovers from the pre-DS pipelines are all gitignored: `platinum/maps/`,
`*/bulbapedia_maps/`, `*/world_spring.png`, and the whole of `public/data/black/`.
Note that `world/`, `interiors/` and `sprites/` are *current* outputs but are
also gitignored (`.gitignore`, "DS-era map renders").

### 2.1 Counts

| | DP | Pt | HGSS | BW | B2W2 |
|---|---|---|---|---|---|
| maps (world / interior) | 506 (66/440) | 524 (66/458) | 490 (75/415) | 399 (40/359) | 588 (53/535) |
| warps (unresolved dest) | 1118 (6) | 1190 (10) | 1317 (9) | 846 (0) | 1046 (0) |
| trainer objects | 472 | 461 | 466 | 366 | 484 |
| item balls / hidden items | 260 / 241 | 328 / 262 | 257 / 235 | 275 / 152 | 405 / 180 |
| berry / apricorn trees | 118 | 118 | 31 | — | — |
| obstacles (cut / smash / strength) | 695 | 690 | 180 | — | — |
| NPCs (no text) | 1088 (376) | 1229 (966) | 1173 (385) | 840 (160) | 1332 (286) |
| signs (text is inline) | 467 | 72 | 705 | 339 | 688 |
| objects with a fractional visual `y` | 2493 | 2685 | 1809 | 977 | 1581 |
| objects outside their map's rect | 2 | 2 | 0 | 11 | 12 |
| maps with an encounter table | 163 | 159 | 151 | 121 | 154 |

One world per game: Sinnoh (DP/Pt, 15360 × 15360 px, 30 × 30 chunks).
HGSS has Johto + Kanto together (24064 × 8800 px, `top: 96`). Unova (BW/B2W2)
is 14848 × 13824 px. Platinum's Distortion World, Spear Pillar and the
Underground are interiors, not separate worlds.

### 2.2 Link coverage against the router's data

| | DP | Pt | HGSS | BW | B2W2 |
|---|---|---|---|---|---|
| router trainers (not "Rematch", not "Unused") | 508 | 527 | 483 | 615 | 813 |
| … with ≥ 1 map position today | 408 (80 %) | 400 (76 %) | 399 (83 %) | 352 (57 %) | 445 (55 %) |
| rivals with a position | 1/24 | 1/33 | 0/24 | Cheren 0/24, Bianca 0/18 | Cheren 0/7, Bianca 0/4 |
| distinct item names matching `items.json` | 114/170 (67 %) | 195/197 (99 %) | 106/144 (74 %) | 74 % | 79 % |
| species matching `pokemon.json` | 100 % | 100 % | 100 % | 97.5 % | 98.9 % |

Trainer identity already agrees. pokemap's `trainerId` / `trainers.json` key is
the ROM trainer index, and that is the router's `rom_id`: `Trainer.trainer_id`,
from `rust/crates/xpr-data/src/loaders.rs:614-634`. For gen 5, all 615 (BW) and
all 813 (B2W2) ids exist on both sides with equal money. Router trainer names
are unique within every gen 4/5 game, because gen 5 names carry the id, e.g.
`"Elite Four Shauntal (38)"`. pokemap's names are *not* unique.

### 2.3 Sizes

| | DP | Pt | HGSS | BW | B2W2 |
|---|---|---|---|---|---|
| world L0 (tiles / MB) | 575 / 54.0 | 575 / 57.1 | 579 / 49.5 | 324 / 23.1 | 393 / 32.1 |
| world L1..max (MB) | 17.8 | 18.6 | 18.1 | 7.9 | 11.1 |
| interiors (files / MB) | 222 / 19.6 | 263 / 20.0 | 259 / 7.4 | 236 / 17.8 | 330 / 28.2 |
| JSON, raw (MB) | 3.5 | 3.4 | 3.0 | 2.4 | 3.4 |
| **metadata as a router pack, deflated** (estimate: objects, maps, encounters, tile classes + lift as binary blobs, sprites) | ≈ 0.35 MB | ≈ 0.34 MB | ≈ 0.43 MB | ≈ 0.34 MB | ≈ 0.49 MB |

World tiles are lossy VP8 at quality 92 with an ALPH chunk
(`pipeline/ds/tiles.py:33-56`). Interiors are lossless VP8L. I re-encoded 25
random tiles per sample:
- World L0 at q85 is 0.70× its current size, and at q75 it is 0.47×.
- Interiors are 2–9× **larger** as lossy.

So interiors must stay lossless, and world L0 dominates the bytes.

---

## 3. Gaps, with the fix each one needs

### G0. The exporter is not in the repo

`pipeline/export-router.js` and its helpers `pipeline/router/{common, routerdata, objects, blobs, items, encounters, links-gen1, links-gen2, links-gen3}.js`
(SPEC §12) are not on this machine and not on GitHub. Every router pack's
`manifest.sources.pokemap` is `d80c88e`, the commit before `9ff6859`, so they
were exported from the Mac working tree (`~/Documents/pokemap`).

**Fix:** commit and push them from the Mac, then merge into `9ff6859`. That
commit rewrote `pipeline/gen4`/`gen5` and deleted `gapfill-world-images.js`,
`stitch-world-map.js`, `scrape-bulbapedia-maps.js` and `world-report.mjs`.
Check `node pipeline/export-router.js --check` still gives byte-identical gen
1–3 output against `A:\pkmn_yellow_xp_router\map_data\`.

Before re-exporting Emerald, read the Route 103 note below: the committed
Emerald pack has a hand fix that a plain re-export undoes. If the Mac copy is
lost, rebuild the exporter from SPEC §3.2, §4 and §12, using the committed
`map_data/<game>/` as the golden output.

> Emerald: `map_data/emerald/layout.json` and `manifest.json` were hand-edited
> on 2026-09-25. Route 103 is solved as 20 rows tall, Littleroot, Route 101
> and Oldale move up 2 blocks, Oldale is drawn over Route 103, and the world
> height goes 534 → 532. pokemap's `POSITION_OVERRIDES` still has the old
> "nudge Route 114 up 2" band-aid. Port the fix into `src/render/world-map.js`
> (a layout-height override for `MAP_ROUTE103` = 20, Oldale in the overlay
> order, and the Route 114 nudge removed). Otherwise `--check` against
> the committed pack will (correctly) show a diff.

### G1. Trainer links

The join key is the ROM trainer id = router `rom_id` (§5.1). What is missing
is the *object → trainer* association for scripted battles.

- **Gen 4 rivals (all three games).** The `StartTrainerBattle` scanner
  (`pipeline/gen4/source_pt.py:296-321`) follows jumps but takes one path. Rival
  scripts branch on the starter, so only one variant or none is found. Follow
  **every** branch (`GoToIfEq`/`CallIfEq`/etc. both ways, to the existing depth
  limit), and attach every trainer id found to the object as `variants`. The
  route contains one of them, and the others link to the same tile, like gen 1–3
  rivals (SPEC §4.2).
- **HGSS**: Champion Lance, all four Kanto Elite Four, both Rocket executives
  and Giovanni are unresolved (19/28 boss-class trainers found). The same
  branch-following, and the coord-trigger rule below, should cover them.
  Check each one.
- **Gen 5**: `pipeline/gen5/source.py:362-370` scans an object's zone script for
  `CallTrainerBattle`/`CallTrainerMultiBattle` only when `flag == 0`. Then
  `pipeline/ds/mapbuild.py:67-69` drops every flagged non-trainer object as
  "story-only". A story flag only controls whether the object is visible. It does
  not mean the object is not a battle. Gym leaders, N, Ghetsis, Cheren and Bianca
  are all flag-gated. Fixes:
  1. Scan flagged objects too.
  2. Keep flagged objects in the output with their `flag`, so the router can
     show them.
  3. Follow calls into the shared/common script files (script ids ≥ 2000,
     currently out of range).
  4. Accept every battle command `script_cmds.py` knows (single, multi,
     double, partner).
- **Battles started with no object** (coord triggers, map-entry/level scripts,
  e.g. a rival stepping out on a route). Anchor them at the trigger's tile with
  precision `script`. If there is no tile, give the map a map-level anchor
  (precision `map`). This is SPEC §4.2's rule for gen 1–3.
- **Double/multi battles**: a pair that shares one id at two positions is
  already correct (e.g. rom_id 75 "Double Team Zac & Jen"). For a
  multi battle with two ids, set `double: true` on each object and list the
  partner in `variants` (the router sets `exp_split` from `double`).
- **Rematches**: gen 4 router names contain `"Rematch"`. They inherit the base
  trainer's anchors, as in gen 1–3. Gen 5 rematch copies (e.g. B2W2 leader
  copies 764–771, BW Elite Four second-round 563–566) are separate ids without
  that word. Link them wherever their script is, or inherit from the same-named
  base id when unreferenced. Say which in `coverage.md`.

### G2. Item names

- DP (`source_dp.py`) and HGSS (`source_hgss.py`) emit bare `"TM01"`/`"HM02"`.
  Platinum appends the move (`source_pt.py:143-144`). The exporter resolves
  TMs through the router's own `items.json` names (`"TM01 Focus Punch"`) by
  number, the same as gen 1–3 (SPEC §12 "Items"). That handles the bare form.
  Fixing the builders too makes pokemap's own viewer right.
- `Pok\ufffd Ball` / `Pok\ufffd Doll`: é is mis-decoded in the gen 4 item-name
  text. Fix the decoding. The exporter additionally folds é → e before
  matching, because router names are ASCII (`"Poke Ball"`).
- Quantities: hidden items carry `"Name ×N"`. Export the name and `count: N`
  separately (§4.6). The router's pickup event takes an amount.

### G3. Encounters

Today every DS game's tables are the curated `encounters/<game>.js` modules.
These are keyed by location name, with swarm/time/radar/dual-slot Pokémon
mixed into one list per method. The ROM/decomp data is read only to link maps
to those tables by content (`source_dp.py:684-720`, `output.py:137-170`).
None of the five games has version columns. The router needs map-keyed,
version-columned, condition-resolved tables from the game data:

- **Platinum**: `repos/pokeplatinum/res/field/encounters/encounters_<map>.json`
  has the pieces separately: `land_rate`, 12 `land_encounters`, `swarms`,
  `day`, `night`, `radar`, the five GBA dual-slot lists
  (`ruby`/`sapphire`/`emerald`/`firered`/`leafgreen`), surf and the rods.
  Apply each replacement the way the game code does (`src/overlay006/wild_encounters.c`,
  `include/overlay006/dual_slot_encounters.h`), not by guessed slot indices.
  Take slot rates from the same code.
- **DP**: `repos/pokediamond/files/fielddata/encountdata/{d_enc_data, p_enc_data}`
  is one set per version, which gives the Diamond / Pearl columns.
- **HGSS**: `repos/pokeheartgold/files/fielddata/encountdata/gs_enc_data.json`.
  It has morning/day/night per slot, rock smash, surf, the rods, Hoenn/Sinnoh
  radio and swarms. Headbutt (per-tree) and the Bug-Catching Contest
  (`files/data/mushi/`) can come later. Split HeartGold vs SoulSilver columns
  where the data differs.
- **BW/B2W2**: read the ROM encounter NARC for **each** cartridge (Black, White,
  Black 2 and White 2 are all in `roms/`) instead of the curated tables. Zones
  with seasonal data have four tables. `Gen5Source.encounter_slots` currently
  always passes `season=0` (`source.py:500`). Emit all four seasons. Methods:
  grass, dark grass, rustling grass, surf, rippling water, fishing, fishing
  ripples, dust clouds, bridge shadows; B2W2 hidden grottos later.
- **Map → table** must come from each map header's encounter id, not from
  names or content (`match_encounters*` in `output.py`).
- **Species forms** (gen 5): emit the router's base names. `Basculin`, not
  `Basculin Red Striped`. `Frillish`/`Jellicent` without the gender.
  `Tornadus`/`Thundurus`/`Landorus` without "Incarnate". The router has no
  Therian rows. `Nidoran♀`/`Nidoran♂`, not `Nidoran F/M`.

The emitted schema is §4.8. pokemap's own encounter popover should gain the
same version/condition switches, which is how you verify it.

### G4. Gen 5 map names

`naming.py:200-212` falls back to `"<Location> (#<zone>)"` for 33 % (BW) and
42 % (B2W2) of maps. Every map needs a human name in the router's location
style, e.g. `"Castelia City Pokémon Center"`, `"Chargestone Cave B1F"`,
`"Route 4 Gate"`. They should be unique within a game where the game has
distinct places. Keep the zone id as the stable `const`.

### G5. Warps

- Platinum has 4 warps into DP-only headers (`MAP_HEADER_ETERNA_CITY_DP_GYM`,
  `…HEARTHOME_CITY_DP_GYM_ELEVATOR_ROOM_1`, `…TRAINER_ROOM_1/2`). Resolve them
  to Platinum's own headers.
- The remaining unresolved destinations (6 DP, 6 Pt, 9 HGSS elevators /
  Safari exits) are runtime-chosen. Export them with `dest_map: null`,
  `dynamic: true`, and a `candidates` list where the script makes it knowable.
- One B2W2 rail warp is dropped silently (`rails.py`). Log it in the coverage
  report.

### G6. Diamond/Pearl tile classes (uncommitted)

The uncommitted `source_dp.py:427-431` fix skips the label line before
`behavior_flags`. Without it, 91 % of DP tiles read as `g` and only 0.4 % as
walls. With it: `#` 50 %, `.` 43 %, `g` 4 %, `w` 3 %. Commit it and rebuild
DP. The router's terrain click, encounter cards and any future pathfinding
read these classes.

### G7. Masks (uncommitted)

`src/render/mask.js`, `mask-layer.js` and `src/ui/mask-editor.js` dim the
unreachable padding around the world and the interiors. Masks are an auto grid
(from walkable tiles plus events) plus hand edits, per surface, saved to
`public/data/<game>/masks.json` through a dev-server endpoint. No `masks.json`
exists yet.

The router wants the same dimming. It should not re-implement the auto-mask
algorithm, so the exporter emits the **resolved** visible cells (§4.11).
Finish the feature, commit it, and produce `masks.json` for all five games.

### G8. Size

≈ 363 MB of webp for the five games, against a 2.8 MB embedded pack today. The
router is a single self-contained binary (rust_port_plan D1/D6), so imagery is
a **separate per-game artifact** and only the metadata (≈ 0.35–0.5 MB per game)
goes into `map_data/`. §4.1 fixes that split. §9 lists the distribution
choice for the user.

### Smaller issues to fix or report in `coverage.md`

- Platinum finds far fewer signs (72) and NPC texts (966 of 1229 NPCs have
  none) than DP (467 signs) and HGSS (705 signs).
- B2W2 has one zone on the matrix that owns no chunk cell and is left out of
  `maps.json`.
- A few objects sit outside their map's rect: 2 DP, 2 Pt, 11 BW, 12 B2W2.
  Give them map-level anchors as in gen 1–3, or fix their coordinates.

---

## 4. Pack format 2 ("image world"): the export contract

Written by `export-router.js` for the five DS games, next to the existing
format 1 packs, into `A:\pkmn_yellow_xp_router\map_data\<game>\`.
Conventions from format 1 are kept wherever they fit, so the router can share
code:
- The same file names.
- `objects.json` / `links.json` / `encounters.json` / `sign_text.json` /
  `sprites.json` have the same shape, with additive fields only.
- JSON colours and sizes as in v1.
- Byte-identical output on re-run, and `--check`.

### 4.1 Two outputs

| output | where | committed? | embedded in the router binary? |
|---|---|---|---|
| **metadata**: every file in §4.2–4.11, §4.13 | `A:\pkmn_yellow_xp_router\map_data\<game>\` | yes (router repo) | yes, ≈ 0.35–0.5 MB deflated per game |
| **imagery**: `world/`, `interiors/`, `imagery.json` (§4.12) | `pokemap/dist/router-imagery/<game>/` plus `<game>-imagery-<imagery_version>.zip` | **no**, never into the router repo | no, delivered separately (§9 D-A) |

The router must work without imagery: links, cards, encounters, add-to-route,
the map list, and a blank-canvas map with outlines and markers. It overlays
the imagery when present. For development, `XPR_MAP_DATA_DIR` can point at a
folder where `world/` and `interiors/` sit beside the metadata.

### 4.2 `manifest.json`

```jsonc
{
  "format": 2,
  "kind": "image",                    // format 1 packs are implicitly "tiles"
  "game": "platinum", "gen": 4,
  "versions": ["Platinum"],           // DP: ["Diamond","Pearl"]; HGSS: ["HeartGold","SoulSilver"];
                                      // BW: ["Black","White"]; B2W2: ["Black 2","White 2"] — exactly the
                                      // router's consts.rs version strings
  "tile_px": 16,                      // one map tile = one step = one "block"
  "world": { "w_px": 15360, "h_px": 15360, "top_px": 0, "levels": 6, "tile_size": 512,
             "default_map": "MAP_HEADER_TWINLEAF_TOWN" },
  "encounter_conditions": ["morning","day","night","swarm","radar","dual_ruby", "..."],  // §4.8
  "blobs": { "classes": [{ "map": "…", "offset": 0, "len": 4096 }],   // §4.5
             "lift":    [{ "map": "…", "offset": 0, "len": 8192 }] },
  "imagery": { "version": 1, "zip": "platinum-imagery-1.zip", "bytes": 91234567,
               "sha256": "…", "files": 1334 },
  "sources": { "pokemap": "<commit>", "repo": "<decomp commit or ROM sha1s>", "router_raw_data": "<router commit>" }
}
```

`imagery.version` goes up whenever any image changes. The router keeps a
downloaded imagery set only while this version and sha256 match. No timestamps:
the output must be byte-identical on re-run.

### 4.3 `maps.json`

This is an array in stable order, whose index is the router's `MapId`. Each
entry has:
- `const`: pokemap's map id, e.g. `MAP_HEADER_ROUTE_201` or `Z355`. It must stay
  stable across exports.
- `display`: the human name (G4).
- `category`: `city | route | area | building | dungeon`.
- `kind`: `outdoor` (world) or `indoor`.
- `w`, `h`: in tiles.
- `can_fly`.
- `location`: pokemap's location string, e.g. "Route 201".
- Outdoor maps: `pos: [x, y]`, the top-left in world tiles, in the pyramid's
  pixel frame (`world px = tile · 16`, `top_px` included).
- Indoor maps: `image: "interiors/m005_a055.webp"` plus `image_origin: [ox, oy]`.
  This is the **map-local pixel where the image's top-left pixel goes**, i.e.
  `(−imageX, −imageY)` of today's schema, as `main.js:163` draws it. Several
  maps may share one image.

No `connections`: the DS world is placed by the game's matrix, not by
connection stitching.

### 4.4 `layout.json`

```jsonc
{ "positions": { "<const>": [x_tiles, y_tiles] },     // outdoor maps, duplicate of maps.json pos (the v1 shape)
  "ownership": { "origin": [0, 0],                    // world tile of cell (0,0) = [0, top_px/16]
                 "cell": 32,                          // tiles per cell (today's chunkMaps)
                 "cols": 30, "rows": 30,
                 "cells": [12, 12, -1, …] } }         // row-major MapId or -1
```

Map rectangles overlap in the DS world, so "which map is at this pixel" is the
ownership grid, never a rectangle test. This is what `main.js:1541-1543` and
`computeLandWater` do. Validate that `top_px % 16 == 0`.

### 4.5 Terrain blobs: `classes.bin`, `lift.bin`

These replace `tiles.json` for the router. Each map has one entry per tile,
row-major `w · h`, and offsets are in the manifest's `blobs`.
- `classes.bin`: `u8` ASCII: `.` walkable, `#` blocked, `g` land encounters,
  `w` water, ` ` no data.
- `lift.bin`: `i16 LE`, the upward pixel shift of that tile in the tilted
  render (today's expanded `dyRle`; measured range −706 … 869).

If the pipeline can cheaply tell them apart, add more classes later: `d` dark
grass (gen 5), `l` ledge, `c` cave spot. Document any additions in the manifest.

The router picks the tile under a map-local pixel `(lx, ly)` exactly like
`pickTileLocal` (`main.js:1513-1528`):

```
tx = floor(lx / 16); base = floor(ly / 16)
for ty = min(h-1, base+16) down to max(0, base-4):
    top = ty*16 - lift[ty*w + tx]
    if top <= ly < top+16: return (tx, ty)
```

In the world scope, across maps, the front-most row wins (largest `pos.y + ty`),
and a tile belongs to a map only if the ownership grid says so. Keep the
algorithm and its constants (+16 / −4) stable, or list the new ones in the
manifest.

### 4.6 `objects.json`

This is the format 1 array, grouped by map in map order. Changes:
- `x`, `y` are the **logical** tile (today's `tx`, `ty`), as integers. This
  keeps the router's step grid (hit-tests, grid, ruler, trip path) integer.
- **New `lift`**: integer px, the tile's upward shift at the object =
  `round((ty − y_visual) · 16)`. The router draws the marker/sprite that many px
  higher. Omit it when 0.
- `kind` ∈ `trainer | item | hidden_item | berry | sign | warp | npc |`
  **`obstacle`** (new: payload `{ "obstacle": "cut" | "rock_smash" | "strength" | … }`).
- `sprite` = the `sprites.json` key (`OBJ_EVENT_GFX_*` / `OBJ_<n>`). `facing` =
  `down|up|left|right`. There is no `pal`.
- Trainer payload: `{ "trainer": "<router trainer_name>" | null, "variants": [names], "double": bool, "raw": "<rom id as string>" }`.
  `trainer` and `variants` are the router's **exact** `trainer_name` strings,
  found by `rom_id` (§5.1).
- NPC payload: `{ "label": "<script id or const>", "battles": [router names], "text_key": "<key>" | null, "flag": <n> | null }`.
  A flag-gated NPC with a battle is how gen 5 gym leaders arrive. The router
  already treats an NPC with `battles` as a trainer (`MapObject::effective_kind`).
- Item / hidden / berry payload: `{ "item": "<router item name>" | null, "raw": "<source name or constant>", "fake": false, "count": 1 }`.
- Sign payload: `{ "text_key": "<const>#<n>" }`, with the text in `sign_text.json`.
- Warp payload: `{ "dest_map": "<const>" | null, "dest_warp": n | null, "dynamic": bool, "candidates": ["<const>", …] }`.
- An object outside its map's rect is dropped from `objects.json` and becomes a
  map-level anchor (gen 1–3 rule), listed in `coverage.md`.

### 4.7 `links.json`

This is format 1 unchanged: `trainers: {"<router trainer_name>": [Anchor…]}`,
`items: {"<router item name>": [Anchor…]}`, `stats: {…}`, where
`Anchor = {map, x, y, precision: object|script|map, object: index|null, source}`.
Anchor `x`/`y` are logical tiles, the same as objects.

### 4.8 `encounters.json`

This is the format 1 shape: `{"<map const>": {"<method>": {"base_rate": n, "slots": {"<version or *>": [{species, min_level, max_level, rate}]}}}}`,
where `rate` is the slot's percent (float), `species` is the router's species
name, and version keys are the manifest's `versions` strings, or `"*"` when
both cartridges agree.

Method names are `<base>` or `<base>_<condition>`. The base must start with one
of the words the router already sorts and classifies, so water/land detection
keeps working (`cards.rs:454`, `pack.rs:511-523`): `walk`, `surf`, `old_rod`,
`good_rod`, `super_rod`, `rock_smash`, `headbutt`. New bases (list them in the
manifest):

| game | bases | conditions (each `<base>_<condition>` is a **fully resolved** table: every slot as the player meets it with that one condition active, on the default time of day) |
|---|---|---|
| DP / Pt | `walk`, `surf`, `old_rod`, `good_rod`, `super_rod`, `honey_tree`, `great_marsh`, `trophy_garden` | `morning`, `day`, `night` (time of day; the unsuffixed table = day), `swarm`, `radar`, `dual_ruby`, `dual_sapphire`, `dual_emerald`, `dual_firered`, `dual_leafgreen` |
| HGSS | `walk`, `surf`, `old_rod`, `good_rod`, `super_rod`, `rock_smash`, `headbutt` (later), `bug_contest` (later) | `morning`, `day`, `night`, `swarm`, `hoenn_sound`, `sinnoh_sound` |
| BW / B2W2 | `walk`, `dark_grass`, `walk_spots` (rustling grass), `surf`, `surf_spots` (rippling water), `super_rod`, `super_rod_spots`, `cave_spots` (dust clouds), `bridge_spots` | `spring`, `summer`, `autumn`, `winter`: only for zones whose data is seasonal, and then **every** method of that zone gets the four suffixed tables and no unsuffixed one |

Gen 2's `good_rod_night` is the precedent. Static/gift/roaming Pokémon are not
wild tables. Leave them out of `encounters.json`: the router adds those as
wild events by hand. List them in `coverage.md` so we can decide later.

### 4.9 `sign_text.json`

`{"<const>#<n>": "text"}` for signs, and for NPCs that have text. The key is
`text_key` in `objects.json`.

### 4.10 `sprites.json` + `sprites/*.png`

The same content as today, e.g.
`{"OBJ_EVENT_GFX_AARON": {"file": "aaron", "w": 32, "h": 32, "frames": 4}}`.
Each sheet is a horizontal RGBA strip of `frames` frames in the order
down, up, left, right, as `main.js:330-352` draws it: feet at the bottom
centre of the tile, raised by `lift`.

Copy only the sheets that some exported object references. Leave out icons
(the router has its own, SPEC D11).

### 4.11 `masks.json` (resolved)

```jsonc
{ "cell_px": 16,
  "world":     { "w": 960, "h": 960, "rle": [0, 1234, 1, 56, …] },          // 1 = visible, run-length [value,count…], row-major cells over the world image
  "interiors": { "interiors/m005_a055.webp": { "w": 40, "h": 30, "rle": […] } } }  // cells over that image (keyed by image: maps sharing an image share it)
```

Emit this after the auto mask and the hand edits are composed
(`composeVisible`). The router just dims, or skips drawing, the invisible
cells. If a surface has no mask, omit it (everything visible).

### 4.12 Imagery (separate artifact)

`world/<L>/<x>_<y>.webp` and `interiors/<name>.webp` are exactly today's
files. A level-`L` tile covers `512 · 2^L` world px at `(x, y) · 512 · 2^L`,
and is RGBA with alpha 0 outside maps. `L` runs to the level where one tile
covers the world.

This is precisely the router's chunk key (`compose.rs:18` `CHUNK_PX = 512`,
`lod.rs`), so the router loads a tile as the chunk with no compositing.
Missing tiles mean empty. `imagery.json` lists the present tiles per level
(today's `world.json.tiles`), the interiors, and each file's byte size.

- Keep interiors lossless.
- World tiles: expose the quality as a pipeline parameter (`tiles.py`, today
  92), so the user can trade size against sharpness when choosing distribution
  (§2.3 numbers). Re-encode from the renders, not from the lossy tiles.
- Optional, later: drop pyramid tiles whose area is entirely masked out.
  Seasonal variants of gen 5 world tiles (`world/<season>/<L>/…`, BW
  `area_has_seasons`, `rom.py:283`) are optional; spring alone is enough for
  parity.

### 4.13 `coverage.md`, `overrides.json`

Same as format 1:
- `coverage.md` (not embedded) has totals and every router trainer with no
  anchor, grouped by class, bosses and rivals first. It also lists unresolved
  item names, maps with `g`/`w` tiles but no table, unresolved warps, dropped
  objects, and static/gift encounters.
- `overrides.json` (hand-written, merged last) takes `{trainer, map, x, y, note}`
  or `{trainer, map}`.

---

## 5. Linking rules for gen 4/5

### 5.1 Trainers

- **Join by `rom_id`, never by name.** pokemap's names repeat (e.g. Elite Four
  Shauntal is 2 ids in BW and 4 in B2W2). The router's
  `raw_pkmn_data/gen_four|gen_five/<game>/trainers.json` has `rom_id` and
  `trainer_name` per record. Emit `trainer_name` exactly.
- **Guard the id space.** DP derives ids arithmetically (`tid = script −
  3000/5000 + 1`). For every linked id, check that the pokemap party (species +
  level list) equals the router record's `pokemon`. Fail the export on a
  mismatch, because an off-by-one would link every trainer to the wrong party.
- Object → id: direct trainer objects. Then every battle command reachable from
  the object's script, following every branch and calls into common scripts
  (G1). Then coord triggers / level scripts → a script anchor at the trigger
  tile, else a map anchor. Then `overrides.json`.
- **Eligible (coverage denominator).** Referenced by some script or object in
  the game data (the exporter knows the referenced set; ids never referenced
  are unused/beta). Also: not `"Rematch"` in the router name, and router
  location not `"Unused"`. Also not a battle-facility trainer (Battle
  Tower/Frontier, Battle Subway, PWT, Black Tower/White Treehollow; these are
  dynamic, like Emerald's Pyramid).
- **Acceptance per game:** ≥ 95 % of eligible trainers have an anchor. **All**
  of these do (take the classes from the router's `trainer_class`):
  - Leader, Elite Four, Champion.
  - Every rival starter variant: gen 4 `Rival <Starter> n`; gen 5 Cheren,
    Bianca, Hugh.
  - Evil-team bosses and admins: Cyrus, Mars, Jupiter, Saturn, Charon; Archer,
    Ariana, Proton, Petrel, Giovanni; N, Ghetsis, Colress, the Shadow Triad
    where they battle.

  Do **not** use `raw_pkmn_data/gen_four/fights_info.json` `major_fights` as
  the list. 41 of its 129 names exist in no gen 4 `trainers.json`, including
  gen 3 names such as `Rival Brendan 1 Treecko`. They are deliberately left
  in the file, unlinked (§8).

### 5.2 Items

Router names are from `raw_pkmn_data/gen_four/items.json` and
`gen_five/items.json`. Rules:
1. Fold é → e.
2. TM/HM → router `"TMnn <Move>"` by number.
3. Split `×N` into `count`.
4. Match with the router's `sanitize_string` equality: keep alphanumerics
   (Unicode-aware, like Rust `char::is_alphanumeric`) and lowercase.
5. Keep a small alias table for spelling differences, and list every unresolved
   name.

Target: 100 % of distinct names, apart from deliberately unlinked things (key
items the router does not track, if any), which are listed.

### 5.3 Species

Router names, with the gen 5 form folding of G3. Every species in
`encounters.json` must exist in the game's `pokemon.json`. Fail otherwise.

### 5.4 Maps → encounter tables

From the map header's encounter id (G3). Every outdoor map, and every indoor
map with `g`/`w` tiles, should have a table. Report exceptions such as the
Great Marsh and Safari Zone areas, which have their own mechanics.

---

## 6. Validation (fail the export on error)

- Trainer keys in `links.json` / `objects.json` exist in the router's
  `trainers.json`, and the party check of §5.1 passes. Item keys exist in
  `items.json`. Species exist in `pokemon.json`.
- Every anchor and object is inside its map. Every warp `dest_map` exists
  unless `dynamic`.
- Blob lengths are `w · h` (classes) and `2 · w · h` (lift) for every map.
- Ownership grid: every outdoor map owns at least one cell, and every non-empty
  cell is an outdoor map.
- Every object's `sprite` exists in `sprites.json`.
- `imagery.json`: every listed file exists. The zip's sha256 equals the
  manifest's.
- `--check` re-exports all twelve games to a temp dir and diffs: gen 1–3 must
  stay byte-identical, and gen 4/5 must be byte-identical on re-run.

---

## 7. Suggested order of work in pokemap

1. **G0**: get `export-router.js` + `pipeline/router/` committed and merged.
   `--check` must stay green for gen 1–3, including the Emerald Route 103 port.
2. **G6**: commit the DP tile-class fix and rebuild DP.
3. **G1** trainer links in the DS builders. Watch the numbers in pokemap's own
   viewer (every gym leader, rival and boss clickable with the right party).
4. **G2** items, **G5** warps, **G4** gen 5 names.
5. **G3** encounters from game data, with pokemap's popover showing version
   and condition switches.
6. **G7** masks: finish, commit `masks.json` for all five games.
7. Exporter: `pipeline/router/links-gen4.js`, `links-gen5.js` (or Python,
   whatever fits the DS builders), format 2 writers (§4). Imagery zip +
   `imagery.json`. Validation (§6). `coverage.md` meeting §5.1.
8. Export into `A:\pkmn_yellow_xp_router\map_data\<game>\` (metadata only).
   Put the imagery zips under `pokemap/dist/router-imagery/`. Report
   `coverage.md` numbers and zip sizes back to the user.

Things the pokemap agent should **not** do: edit router code, commit to the
router repo, or put imagery into `map_data/`.

---

## 8. Router side (built 2026-10-01)

The router loads all five DS packs, shows them in the Map tab, and does
everything it does for gens 1–3, plus encounter conditions and masks.

**`xpr-map`**
- `pack.rs`: `MapPack::load` reads the manifest's `format` first; format 2
  goes to `load_image`. Objects, links, encounters, text and sprites share
  `load_common` with format 1. `MapPack::image: Option<ImageWorld>` holds the
  format 2 data. The geometry is `block_px == step_px == 16` (one tile = one
  step). `game_for_version` maps the nine DS versions.
- `image_world.rs`: `ImageWorld` holds the ownership grid, per-tile classes
  and lift, the per-map max lift, the decoded masks, the imagery entry and
  the encounter method/condition labels. `pick_local` is pokemap's
  `pickTileLocal`.
- `model.rs`: `MapFrame` places a map in its own scope. An interior's scope
  is the union of the map and its picture. Objects gain `lift`, `version`,
  `ObjectKind::Obstacle`, and the new payload fields: item `count`, warp
  `dynamic`/`candidates`, NPC `text_key`.
- `geom.rs`:
  - `map_origin_px` / `scope_rect` follow the frame.
  - `map_at_world_px` asks the ownership grid in an image world.
  - New `pick_step` finds the lifted tile under a pixel (front-most row,
    ownership-checked).
  - `step_center_px` / `object_center_px` are raised by lift.
  - New `fit_rect` fits an interior to its mask's visible cells.
- `spatial.rs`: objects are filed and hit-tested where their marker is drawn
  (raised by lift), across every overworld map near the pointer.
- `imagery.rs`:
  - `DirFiles` reads `world/` + `interiors/` folders.
  - `ZipFiles` reads a zip in place, from a file or from bytes embedded in
    the executable (`from_static`).
  - `register_embedded` / `embedded` hold the zips the executable carries.
  - `Imagery` decodes webp and caches interior mip chains.
  - `verify_zip` checks the size and sha256 against the manifest.
  - `xpr-map/build.rs` leaves `imagery.zip` out of its own (deflated) table.
- `compose.rs`: image packs draw:
  - world chunks straight from the pyramid tiles;
  - interiors from the mip chain;
  - an overworld map on its own, cropped from the world;
  - the masks dimmed toward the background at pokemap's 0.9 (`RenderOpts::mask`);
  - a terrain-class schematic when no imagery is installed.
  Export goes through the same path.
- `sprites.rs`: gen 4/5 sheets are four RGBA frames (down, up, left, right).

**`xpr-app/src/map/`**
- **The pictures are built into the executable, like the gen 1–3 maps
  (decided 2026-10-01).**
  - `xpr-app/build.rs` (`write_map_imagery`) `include_bytes!`s every
    `map_data/<game>/imagery.zip` into code that only `main.rs` includes,
    and `main` registers the zips at startup. The library and the test
    binaries don't carry them.
  - The executable is ≈ 414 MB in release; the build takes ≈ 3 min with fat
    LTO.
  - `XPR_NO_EMBED_MAP_IMAGERY=1` builds without them.
  - `map/imagery.rs` finds the pictures in this order: the embedded zip;
    `<map_data dir>/<game>/imagery.zip` (run from source, or
    `XPR_MAP_DATA_DIR`); loose `world/` + `interiors/` folders. A zip whose
    size doesn't match the manifest is ignored.
  - Without the pictures the map draws the terrain sketch, and the status
    line says "map pictures missing from this build".
  - **Committing the zips puts ≈ 365 MB into git.** Each file is under
    GitHub's 100 MB limit but over its 50 MB warning; consider Git LFS for
    `map_data/*/imagery.zip`.
- Markers and sprites are raised by lift. Objects of the other version are
  hidden: in markers, hover, keyboard navigation, map cards, "add all" and
  the marquee. The version is the route's, or the base version for a
  custom gen (`RouteMapState::version`).
- Layers gain "Obstacles" and "Dim unreachable areas" (image worlds only).
- Encounter cards show condition chips per map:
  - gen 4: Day / Morning / Night / Swarm / Poké Radar / the dual-slot games;
  - HGSS: also the radio;
  - gen 5: seasons, defaulting to Spring in seasonal zones.
  Each method shows the chosen condition's table. Labels come from the
  manifest.
- Cards: NPC text, item counts, obstacles, and elevator warps listing every
  floor.
- The map list groups by `category` and sorts by name. The world titles are
  Sinnoh, Johto & Kanto, and Unova.

**Tests**
- `xpr-map/tests/image_pack.rs`: all five packs load and are framed and
  owned; lifted tiles and raised objects are picked where they are drawn;
  the encounter order; gen 4/5 sprite frames; the schematic; imagery from a
  folder and from a zip; masks; `verify_zip`.
- `xpr-app/tests/map_image_world.rs`: a Platinum route opens Sinnoh drawn
  from `map_data/platinum/imagery.zip`, a grass card's Night chip switches
  tables, a build without pictures shows the sketch and the note, and loose
  picture folders work.
- `xpr-app/tests/legacy_glyph_names.rs`: the gen 5 glyph rename and the route
  migration (below).
- `xpr-app/tests/embedded_map_data.rs`: every built-in version has a map.
- No new raw input: the chips are widgets.

**Not done (decide separately)**
- **Gen 4/5 trainer locations: done (2026-10-02).** The source is the
  data_objects repo (`~/Documents/data_objects/trainers/<game>.js`), whose
  `generate_gen4_5_trainer_locations.py` fills each record's in-game place
  (several joined with " / ") from pokemap, the Vs. Seeker and phone rematch
  tables, Platinum's daily Pokémon Center trainers, the B2W2 research and the
  BW stadium roster.
  - `tools/data/import_gen45_trainer_locations.py` copies the locations into
    the router's `trainers.json` by `rom_id`, keeping "Unused". `--check`
    reports whether the router is out of date.
  - The loader reads `trainer_location` for gens 4/5, as for gen 3. A null
    location reads as none.
  - Located trainers: DP 659/849, Pt 720/927, HGSS 638/737, BW 538/616,
    B2W2 797/814.
  - The map export follows the same data:
    - A trainer with a location but no object or script on the map gets a
      map-level anchor on its place's outdoor map(s): DP 3, Pt 38, BW 54,
      B2W2 103. B2W2's 18 "Funfest Mission" trainers name no map.
    - Rematch and namesake inheritance happens only for trainers the data
      places, and keeps only the namesakes' spots inside that place.
  - Result: every anchor of every game lies in its trainer's location, and
    every linked trainer has one.
- **Gen 5 glyph names: fixed (2026-10-01).** The data spelled ♂/♀ and
  "PKMN" with the game's font glyphs (`⑭⑮⒆⒇`), which no font draws.
  - `raw_pkmn_data/gen_five/{black_white,black2_white2}/trainers.json` and
    `gen_five/fights_info.json` now spell them out: `⒆⒇` → `Pokemon`,
    `⑭` → `M`, `⑮` → `F` (the gen 3/4 style, e.g. "Pokemon Trainer Bianca
    (4)", "Clerk M Chaz (19)"). Party species are `Nidoran♂` / `Nidoran♀`, as
    in `pokemon.json`. No renamed trainer clashes with an existing name.
  - `xpr_core::io_utils::fix_legacy_name_glyphs` does the same at runtime:
    - Route migration: trainer events (`trainer_name`,
      `second_trainer_name`) are renamed as they load, so routes saved with
      the old names open in a new build.
    - The trainer loader and fights info apply it too, so a custom gen
      copied from the old data still works.
- **`fights_info.json` stays as it is (decided 2026-10-01).**
  `raw_pkmn_data/gen_four/fights_info.json` `major_fights` names 41 trainers
  that no gen 4 `trainers.json` has, such as `Rival Brendan 1 Treecko`. They
  stay in the file, unlinked. The loader keeps them in a set that nothing
  matches, so they have no effect. Don't use `major_fights` as a list of
  trainers to link or to check coverage against (§5.1).

---

## 9. Decisions for the user

The export above is designed so that none of these block pokemap work.

- **D-A: how imagery reaches users.** **Decided 2026-10-01: embed it in the
  executable, like the gen 1–3 maps (§8).** The options were:
  1. On-demand per-game download from a GitHub release asset of the router
     repo. It is cached in the app data dir and checked against the manifest's
     sha256, and the map works without it until it arrives.
  2. A folder shipped beside the executable, which breaks the single-file
     rule.
  3. Embedding a reduced pyramid. Even L1+ only (8 px per tile) plus interiors
     is 20–40 MB per game, so this is not viable for five games.
- **D-B: world tile quality.** q92 today. q85 is ≈ 0.7× and q75 ≈ 0.47× of L0
  size. This only matters for download size under D-A (1).
- **D-C: gen 5 seasons.** Encounter seasons are in scope (§4.8). Seasonal
  *imagery* is optional.
- **D-D: static/gift/roaming Pokémon.** Leave them out of encounter tables
  (proposed) or add them as a separate object kind the map can place.

---

## Appendix A. Measurement scripts (2026-09-26)

These live in this session's scratchpad (not committed) and are read-only
against both repos. Per-game audits: the gen 4 scripts `analyze1..11.py`,
`level_sizes.py`, `sizes.py` and the gen 5 scripts `a_trainers.py` …
`e_stats.py`. The numbers in §2 were also re-derived with one-off `py -3.14`
snippets:
- counts over `events.json` / `maps.json`;
- pyramid bytes per level;
- a Pillow re-encode of 25 random L0 tiles and interiors at q92/85/75;
- `zlib` level 9 over the JSON, and `tiles.json` rebuilt as `u8` classes plus
  `i16` lift, for the metadata estimate;
- duplicate `sanitize(trainer_name)` counts in the router's gen 4/5
  `trainers.json` (zero in all five games).

Rerun them against a fresh export before trusting a changed number.
