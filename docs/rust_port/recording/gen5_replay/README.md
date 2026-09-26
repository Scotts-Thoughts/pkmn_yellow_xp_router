# Recording real gen 5 runs without an emulator window

Plays a Super Shuckie replay headlessly, serves its memory to a private
Poke-A-Byte and records it with the Rust app, then compares the route with
ground truth read from the same replay. What was learned with it is in
`../gen5.md`.

```
replay ──► shuckie-feeder ──UDP/shared memory──► Poke-A-Byte (patched, :8095) ──SignalR──► xpr-app (quick start)
             │ HTTP :30190 (seek, pace, read, trace)
             └─► gt.py / compare.py
```

Nothing here touches the user's own Poke-A-Byte (`:8085`, UDP 55356) or Super
Shuckie: the feeder and the patched Poke-A-Byte use their own ports, shared
memory name and config folder.

## One-time setup

1. **Super Shuckie snapshot.** `shuckie-feeder` links `supershuckie-core`, which
   must be the committed tree (the working copy may carry unfinished work that
   no longer builds on its own). From the Super Shuckie checkout:

   ```
   git archive HEAD | tar -x -C <this folder>/ss-snap
   # the core's submodule sources (with their local patches applied), for the C++ glue
   tar --exclude=.git -cf - -C melonds-rs melonDS | tar -x -C ss-snap/melonds-rs
   tar --exclude=.git -cf - -C mgba-rs mgba | tar -x -C ss-snap/mgba-rs
   ```

   The emulator cores themselves (`libcore.a`, `libteakra.a`, `libmgba.a`) are
   taken from the checkout's `build/` (`build.rs`; `SUPERSHUCKIE_BUILD_DIR`
   overrides it), so build Super Shuckie once first.

2. **Build the feeder** with MSYS2's UCRT64 toolchain (the cores are GNU
   archives):

   ```
   PATH=/c/msys64/ucrt64/bin:$PATH cargo build --release --manifest-path shuckie-feeder/Cargo.toml
   ```

3. **Patched Poke-A-Byte.** Copy the Poke-A-Byte source, apply
   `pokeabyte-port.patch` (it also drops the npm build: the checkout's
   `Frontend/dist` is used as is) and build it:

   ```
   dotnet build src/PokeAByte.Web/PokeAByte.Web.csproj -c Release -o out
   ```

   Give it a config folder with the mappers in `Mappers/` (for example
   `Mappers/STANDARD/gen5/*.xml|js` copied from `%APPDATA%\PokeAByte\mappers`).

## Running

```
# the feeder (paused at frame 0), then the patched Poke-A-Byte on the same UDP port
shuckie-feeder --rom "Black-2026 v0.6.0.nds" --replay b-landorus-1-10502.replay --paused --http 30190 --port 55390
POKEAPROTOCOL_PORT=55390 POKEABYTE_CONFIG_DIR=<config> ASPNETCORE_URLS=http://localhost:8095 out/PokeAByte.Web.exe

# record frames 5000..300000 at 4x with the landing page's quick start
py -3.14 run_rec.py --work runs/black --start 5000 --until 300000 --speed 4 \
    --mapper STANDARD/gen5/pokemon_black.xml --exe A:\...\rust\target\release\xpr-app.exe
```

`run_rec.py` seeks the feeder, reloads the mapper (Poke-A-Byte keeps reading a
stale shared-memory block after a feeder restart until it does), writes a
config with the window placed off-screen, starts the app without focus and lets
it record until the feeder reports `done`. The route lands in
`<work>/data/saved_routes/recorded.json`, the app log (every property change;
debug logging is on) in `<work>/config/pkmn_router_logs.log`. Use `FX_BASE`
(e.g. `http://127.0.0.1:30192`) and `--pab` for a second stack in parallel; a
run at 4x takes about as long as the game time divided by four.

Ground truth and the comparison:

```
curl "http://127.0.0.1:30190/goto?frame=0"
curl "http://127.0.0.1:30190/trace?to=952012&out=trace.txt&spec=party@0x22349B0:1324,bag@0x2233FAC:2564,money@0x223CDAC:8,time@0x2256FD4:4,map@0x224F90C:16,opp@0x22697B0:0x70,btlflags@0x226D240:0x50,bstruct@0x226D670:0x19B0,dynparty@0x226A220:0x2B00"
py -3.14 gt.py trace.txt 0 952012 --json gt.json
py -3.14 compare.py runs/black/data/saved_routes/recorded.json gt.json 5000 300000
```

The addresses are Black's; White adds 0x20, Black 2 / White 2 are elsewhere
(`g5.py` has White 2's bag layout; the mapper XML has the rest). A trace runs at
about 300 frames a second.

`fx.py` is the feeder client for poking around by hand (`goto`, `read`, `u16`,
`bsearch` over frames, `shot` for a PNG of both screens), and `/press` drives the
game with your own input from any frame (a soft reset is
`/press?keys=l,r,start,select&frames=10`; tap the bottom screen with
`touch=X,Y`).

## Scripted sessions (saves and resets)

The replays have few resets, and none exist for Black 2, so resets were tested
with input the feeder injects:

```
# Black 2: no replay; boot from White 2's cartridge save (/savesram on a White 2 feeder)
shuckie-feeder --rom "Black2-2026 v1.nds" --sav w2.sav --paused --http 30192 --port 55394
# get to the overworld once (drive.py steps), then keep that point
curl "http://127.0.0.1:30192/savestate?path=b2/overworld.state"

# every run: load the state, then record while drive.py plays the script
curl "http://127.0.0.1:30192/loadstate?path=b2/overworld.state"
FX_BASE=http://127.0.0.1:30192 py -3.14 run_rec.py --work runs/b2_resets --start 0 --until 40000     --speed 2 --no-seek --script scripts/b2_resets.txt --pab http://127.0.0.1:8098     --mapper STANDARD/gen5/pokemon_black_2.xml --mode record --route "Black 2|Blastoise"
```

`drive.py <port> <steps>` plays steps such as `a:4:40` (hold A 4 frames, then 40
idle frames), `l+r+start+select:10:300` (soft reset), `tap@64,138:4:150` (touch
the bottom screen) and `shot:name` (screenshot to `shots/name.png`); `paced`
makes the rest run at the feeder's speed. `scripts/*_resets.txt` save, toss an
item, reset, toss, save, toss, reset and reset again; each expects the route
to end as save, toss, save. The menus differ per game and the bag remembers its
pocket until a reset, so dry-run a script against the feeder and look at its
screenshots before recording it. The states they start from came from
`b-landorus-1-10502.replay` frame 505300 (Black), `w-basculin-0-13300.replay`
frame 654000 (White) and White 2's save loaded into Black 2.

`sample_counters.py <port> <from> <to> <step> <addr>...` samples u16s through a
replay with `/goto` (for the save counters: every save shows as a step).
