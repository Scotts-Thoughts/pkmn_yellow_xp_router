# AGENTS.md

Instructions for coding agents working in this repository. Read `CLAUDE.md`
too: it has the project rules (Rust port only, modal input, decomp repos).

## Git: no pull requests, work on main

- **Never open a pull request and never push a branch.** Scott commits and pushes
  himself.
- **Work in Scott's local checkout** of this repo on his PC, on the branch it is
  already on (normally `main`). Don't create or switch branches for your work.
- **Leave every change uncommitted**, so Scott can test it locally on main and
  commit it himself.
- If you are running in a cloud session with only a clone of this repo, do the
  work on Scott's PC instead (folder access or Remote Control). If you can't,
  stop and ask Scott; don't fall back to a branch or a PR.

## Debugging a recorded route: reproduce it yourself

When Scott reports that a recorded route is wrong ("the Yanmega run in
HeartGold has the wrong ability", "the route kept Rock Smash", "a battle is
missing"), **do not ask him for files or for a plan**. Every run he records is
also a Super Shuckie replay on his machine, and the repo has headless tools
that play a replay through the real recorder and let you watch the game's
memory frame by frame. Find the replay, reproduce the problem, read the RAM,
find the cause, fix it, and prove the fix with the same replay. Ask only if
the replay genuinely is not there.

Everything below runs on Scott's Windows machine, in `A:\pkmn_yellow_xp_router`
(the tools are Windows builds and the replays and ROMs live there). From a
cloud session, start a Remote Control session in that folder and do the work
there; the cloud container has no replays, ROMs or emulator cores.

### 1. Find the replay

Super Shuckie 2 keeps one replay folder per ROM:

```
A:\Dropbox\stp-projects\programs\supershuckie 2\UserData\<rom file name>-data\replays\
```

For example `...\UserData\4-HeartGold v6.nds-data\replays\`. Replays are
named `<game letter>-<species>[-line]-<n>-<run time or dnf>.replay`:
`h-yanmega-line-2-21334.replay` is a finished HeartGold Yanmega run,
`h-yanmega-line-2-dnf.replay` an unfinished one, and anything after the
number is Scott's note (`h-yanmega-2-dnf - wrong ability.replay`). Pick the
one matching the report; when several match, the newest (`dir /O-D`) is
usually the one he means, and the other folders (`1-Red v1.0.1.gb-data`,
`3-Emerald v1.6.0.gba-data`, ...) work the same way. `tools/replay_to_route/run_corpus.py`
lists a known-good replay for every game.

### 2. Reproduce: replay → recorder → route (`replay_to_route`)

`replay_to_route` plays the replay headlessly (Super Shuckie's own emulator
cores in `xpr-replay-worker`), feeds the memory to Poke-A-Byte's mapper engine
(`pab-host`) and the property changes to the real `xpr-recorder`, exactly as a
live recording would have, but a 9-hour run takes minutes. How it works and
how to build it: `tools/replay_to_route/README.md`.

```
cd A:\pkmn_yellow_xp_router\rust
cargo build --release --bin replay_to_route
target\release\replay_to_route.exe "A:\Dropbox\stp-projects\programs\supershuckie 2\UserData\4-HeartGold v6.nds-data\replays\h-yanmega-line-2-21334.replay" ^
    --out C:\tmp\dbg\yanmega.json --log-dir C:\tmp\dbg\logs
```

* **Always pass `--out` and `--log-dir`** to a scratch folder. Without them
  the route goes into Scott's real `saved_routes`.
* The worker and `pab-host` come from `tools/replay_to_route/out/` (already
  built; rebuild with `tools/replay_to_route/build.sh` only after a worker or
  core change). Mappers come from `%APPDATA%\PokeAByte\mappers`. The ROM is
  found from the replay's checksum (`--rom` if not); the log's first
  `[Replay Import]` line names it.
* `--frames a-b` imports only part of the replay; `--accept-current` for a
  replay that does not start at a new game.
* The report ends with a check of the solo mon's level, exp and moves against
  the game. Also run `py -3.14 tools/replay_to_route/exp_drift.py <route.json> <log dir>`.
* Read the result with `py -3.14 tools/replay_to_route/route_summary.py <route.json>`
  (one line per event; give it two routes to diff them) and
  `cargo run -p xpr-engine --example route_errors -- <route.json>`.

If Scott also sent or saved the route he recorded live, diff it against the
imported one: a difference between the two points at timing in the live app,
the same bug in both points at the recorder or the mapper.

### 3. Watch the game: mapper properties and RAM

**Mapper properties, as the recorder sees them.** `XPR_REPLAY_TRACE` logs
every change of the mapper paths that contain one of the given substrings,
optionally only in a frame range. Use `--dense-all` so every frame is read
(the default only reads the stretches around watched changes):

```
set XPR_REPLAY_TRACE=player.team.0.moves,player.team.0.held_item,battle.outcome@150000-260000
target\release\replay_to_route.exe <replay> --out C:\tmp\dbg\t.json --log-dir C:\tmp\dbg\tlogs --dense-all --frames 150000-260000
```

The lines are `[Replay Trace] frame N: <path> = <value>` in
`<log dir>\pkmn_router_logs.log`, next to the recorder's own debug lines (every
property change it handled and every event it added). The paths are in the
mapper XML in `%APPDATA%\PokeAByte\mappers` (gen 4: `STANDARD/gen4/*.xml`).
The first frame where the mapper value is wrong means a mapper or address
problem; a right value the recorder mishandles means a recorder bug
(`rust/crates/xpr-recorder/src/games/*.rs`).

**Raw RAM, frame by frame (NDS games).** `shuckie-feeder`
(`docs/rust_port/recording/gen5_replay/`, built per its README) plays a replay
headlessly and answers over HTTP:

```
shuckie-feeder --rom "<rom>.nds" --replay <file.replay> --paused --http 30190 --port 55390
curl "http://127.0.0.1:30190/goto?frame=200000"
curl "http://127.0.0.1:30190/read?addr=0x2000000&len=64"
curl "http://127.0.0.1:30190/screenshot?path=C:/tmp/dbg/f200000.png"
curl "http://127.0.0.1:30190/trace?to=260000&out=C:/tmp/dbg/trace.txt&spec=party@0x<addr>:<len>,..."
```

`/trace` writes every byte change of the named regions between the current
frame and `to`; `/dump` writes a raw memory block; `/bookmarks` lists the
replay's bookmarks; `/press` plays your own input from any frame. Its Python
client `fx.py` adds `goto`, `read`, `u16`, `u32`, `shot` and `bsearch` (the first
frame where a condition becomes true), which is the quickest way to find where
something changes. Look at the screenshots to see what the game was doing.
Take the addresses from the mapper XML and check what they mean in the decomp
(`CLAUDE.md` lists the repos; gen 4 HGSS is `pokeheartgold`). The gen 5
addresses in that README are Black's.

To watch the whole live pipeline (feeder → patched Poke-A-Byte → the app's
quick start) instead of the offline import, use `run_rec.py` from the same
folder; it never touches Scott's own Poke-A-Byte or Super Shuckie.

### 4. Fix and prove it

* Write a test that drives the recorder through what the replay showed, with
  the frame numbers and values you read, like
  `rust/crates/xpr-recorder/tests/gen4_day_care.rs` (Togekiss / Day Care) or
  `gen4_held_item.rs`. `cargo test -p xpr-recorder`.
* Import the same replay again and show that the route is now right (the
  `route_summary.py` diff of before and after, the end-of-run check, and
  `exp_drift.py`).
* Report to Scott what was wrong, at which frame, which value showed it, and
  what changed. Leave your scratch files out of the repo.
