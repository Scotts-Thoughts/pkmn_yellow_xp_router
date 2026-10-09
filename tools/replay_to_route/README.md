# replay_to_route: a route from a Super Shuckie replay

`replay_to_route <file.replay>` records a route from a Super Shuckie replay, the
way the Rust app's recorder would have recorded the run live, but without
playing it back in real time. A 9-hour run takes a few minutes.

```
A:\pkmn_yellow_xp_router\rust\target\release\replay_to_route.exe "...\replays\h-meganium-line-2-21608.replay"
```

The route is saved to the router's saved routes folder, named after the replay
(`--out <file>` saves it somewhere else), with a log in `replay_to_route_logs`
beside it. The report ends with a check of the route against the game: the solo
Pokémon's level, experience and moves in the route against what the game's
party held where recording stopped.

Options (`replay_to_route` with no arguments lists them): `--rom` when the ROM
is not found by itself, `--accept-current` for a replay that does not start
with a new game (a `-resume` file), `--frames a-b` for a part of the replay,
`--slots n` for the number of parallel readers, `--cpus n` to keep the import
(and its helpers) on n cores. By default it uses a quarter of the cores at
below-normal priority.

## How it works

```
replay ──► xpr-replay-worker ──memory──► pab-host ──property changes──► recorder ──► route
           (Super Shuckie cores)        (Poke-A-Byte's mapper engine)   (xpr-recorder, offline)
```

* **xpr-replay-worker** (`worker/`, Rust, links Super Shuckie's emulator cores)
  reads the game's memory out of the replay. A replay keeps a save state every
  120 frames (a keyframe); loading one needs no emulation, and stretches in
  between are emulated frame by frame from the keyframe before them.
* **pab-host** (`pab-host/`, .NET) is Poke-A-Byte's own mapper engine
  (`PokeAByte.Domain`) reading that memory instead of an emulator's, so the
  property values are exactly the ones Poke-A-Byte would have served: the same
  XML mappers and JavaScript.
* **xpr-replay** (`rust/crates/xpr-replay`) runs the two:
  1. a *scan* reads every keyframe through one mapper instance and notes where
     a property the recorder watches changes (Quick Start's slot-1 fields and
     the generation recorder's paths, `xpr_recorder::offline::watched_paths`);
  2. those stretches (and 1 keyframe before / 4 after: the gen 1-4 recorders
     count game-time seconds after an event) are read every frame, the rest
     only at keyframes;
  3. the work is cut into chunks run in parallel; a chunk starts on the
     overworld with its mapper instance seeded with the scan instance's script
     state there (`X`/`I` in pab-host), so mapper scripts that cache things
     (Emerald's patch detection) or keep a state machine behave as in one
     continuous run.
* **replay_import** (`rust/crates/xpr-app/src/replay_import.rs`) feeds the
  changes, in frame order, to Quick Start and then to the generation's
  recorder through `xpr_recorder::offline::OfflineClient`, on the replay's
  clock (`xpr_recorder::clock`: settle times run on replay time, queued events
  are processed after each batch, events carry Super Shuckie's run timer).

The parallel run is checked against a *reference*: every frame through one
mapper instance (`--dense-all --single-chunk`, many times slower). They must
produce the same route.

## Building

```
tools/replay_to_route/build.sh          # pab-host and the worker into out/
cd rust && cargo build --release --bin replay_to_route
```

`build.sh` needs the .NET 10 SDK, MSYS2's UCRT64 Rust toolchain (the cores are
MinGW archives) and a Super Shuckie checkout at `A:\Programs\supershuckie` that
has been built once. It snapshots the committed tree of the Poke-A-Byte
checkout (`A:\Programs\PokeAByte`, or `--pokeabyte <dir>`) into `.vendor/`:
its sources are AGPL and are not part of this repository.

`replay_to_route` finds `out/` from the executable's or the working
directory's folders (`--tools <dir>` or `XPR_REPLAY_TOOLS` otherwise) and the
mappers in `%APPDATA%\PokeAByte\mappers` (`XPR_MAPPERS_DIR`).

## Mappers

The mapper comes from the ROM header, the one each recorder was built against:

| Game | Mapper |
|---|---|
| Red, Blue, Yellow | `STANDARD/gen1/pokemon_red_blue.xml`, `pokemon_yellow.xml` |
| Crystal | `STANDARD/gen2/pokemon_crystal.xml` |
| Emerald | `ST/gen3/pokemon_emerald_ne.xml` (finds the STP patch version; the deprecated one reads the save marker at its v1.5 address) |
| FireRed, LeafGreen | `ST/gen3/pokemon_firered_leafgreen_deprecated_ne.xml` |
| Platinum, HeartGold, SoulSilver | `ST/gen4/*_ne.xml` |
| Black, White, Black 2, White 2 | `STANDARD/gen5/*.xml` (with `docs/rust_port/recording/gen5_mappers_save_counter.patch`) |

`--mapper <path>` overrides it.

What the route can be is bounded by the mapper. Gen 5 needs
`docs/rust_port/recording/gen5_mappers_save_counter.patch`: without it the
recorder cannot see saves (Black 2/White 2 read a dead byte, Black/White only
see saves that changed the party), so after a reset it cannot tell which
events to undo. A White 2 run with 54 resets ended 7 levels ahead of the game.

## Testing

```
py -3.14 run_corpus.py --exe <replay_to_route.exe> --work <dir> [--only red,crystal] [--ref]
py -3.14 route_summary.py <a.json> <b.json>
```

`run_corpus.py` imports one real run per game (the list is at the top of the
file) and tabulates time, events and the check against the game; `--ref` also
builds each reference and diffs the two routes. `route_summary.py` prints a
route one event per line, or diffs two.

Unit tests: `cargo test -p xpr-replay -p xpr-recorder` (`offline_client.rs`
covers the offline client and clock).
