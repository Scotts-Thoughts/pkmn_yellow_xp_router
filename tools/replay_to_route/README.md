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
Pokémon's level, experience and moves in the route against the game's party
once recording has stopped and the game has left the battle (recording stops at
the final trainer, but gen 4 only copies a battle's results back to the party
about 400 frames later, so the check keeps reading until then).

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
     the generation recorder's paths, `xpr_recorder::offline::watched_paths`)
     or where the cartridge's save memory changed (the worker hashes it at each
     keyframe): the game saved there. The gen 1/2 recorders see a save only by
     its sound, which plays for about 30 frames, and nothing else the mapper
     reads changes, so without this an import missed about half the saves and
     a later reset undid everything back to the save before (a Red run lost 66
     events and 5 levels). Gen 1/2 also use cartridge RAM as a scratch buffer
     for pictures, so the save memory changes in more intervals than there are
     saves; nearly all of those are emulated anyway;
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

The worker answers `sram on` (save memory hashes); an older worker cannot, and
the import then logs a warning and may miss saves: rebuild it.

`build.sh` needs the .NET 10 SDK, MSYS2's UCRT64 Rust toolchain (the cores are
MinGW archives) and a Super Shuckie checkout at `A:\Programs\supershuckie` that
has been built once. It snapshots the committed tree of the Poke-A-Byte
checkout (`A:\Programs\PokeAByte`, or `--pokeabyte <dir>`) into `.vendor/`:
its sources are AGPL and are not part of this repository.

The cores in Super Shuckie's `build/` are compiled with its melonDS patches
(`melonds-rs/patches`), and `melonds-rs/interface.cpp` needs them: if the
`melonds-rs/melonDS` submodule has been reset to upstream, the worker fails to
compile (`'class melonDS::GPU' has no member named 'SkipDrawing'`). Apply them
first (`git -C melonds-rs/melonDS apply --ignore-whitespace ../patches/*.patch`,
as Super Shuckie's `scripts/build-melonds.sh` does).

`replay_to_route` finds `out/` from the executable's or the working
directory's folders (`--tools <dir>` or `XPR_REPLAY_TOOLS` otherwise) and the
mappers in `%APPDATA%\PokeAByte\mappers` (`XPR_MAPPERS_DIR`).

## Shipping

A released router needs nothing else installed. The worker, a self-contained
`pab-host`, the mappers and their licences are one zip of their own (48 MB). It
is not in the exe, so an update that leaves it alone stays small: the exe is
built with `rust/replay_tools.json`, which names the bundle it needs, and
`xpr-app/src/replay_bundle.rs` downloads that bundle at the first import and
unpacks it into `<config dir>/replay_tools/<bundle>/`. Later imports reuse the
folder. If a new version names a different bundle, the next import fetches it
and removes the old one. Only the ROM comes from the user, and the app asks for
it when it isn't beside the replay or in Super Shuckie's recent ROMs. There is
no bundle for Mac, because there is no Mac worker yet.

The bundle can't be built in CI (see `build.sh`'s needs and the local mappers),
so it is made here and kept on a release, named after its content.
**Make a new one whenever the worker, `pab-host` or the mappers change**,
including after any Super Shuckie core change:

```
tools/replay_to_route/build.sh --skip-pab-host     # the worker
py -3.14 tools/replay_to_route/bundle.py           # also updates rust/replay_tools.json
gh release upload <tag> <the zip> --repo Scotts-Thoughts/pkmn_yellow_xp_router
```

Then commit `rust/replay_tools.json`. The release workflow fails if that file
is on no release (`rust/check_replay_tools.py`), because every copy of that
version would then fail to import. Don't delete old releases that hold a
bundle: older versions still fetch theirs.

## Checking a route

The end-of-run check alone does not prove a route is exact: a Rare Candy sets
exp to its level's minimum in both route and game, which erases any error
before it. `exp_drift.py <route.json> <log folder>` reads the import's
`exp check` lines and fails if a candy was used with the two apart. Run it too.
