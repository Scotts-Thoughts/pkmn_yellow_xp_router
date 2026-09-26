# Recording harness

Replays a scripted GameHook session to the Rust app without an emulator or
Poke-A-Byte. Gen 5 is checked against real runs instead: see [gen5.md](gen5.md)
(what the games do and how the recorder reads it) and `gen5_replay/` (Super
Shuckie replays played back headlessly into a private Poke-A-Byte). (Until 2026-09-18 this folder also held a Python-vs-Rust pair
diff, `run_pair.py`; it went with the Python app. All five replay scenarios
recorded byte-identical routes in both apps as of 2026-09-12.)

## Pieces

| File | What it is |
|---|---|
| `mock_gamehook.py` | A stand-in for GameHook / Poke-A-Byte (stdlib only): `POST /updates/negotiate`, `GET /mapper`, `PUT /mapper/properties/*`, and the SignalR hub over a hand-rolled websocket. It plays a scenario as `PropertiesChanged` invocations once a client has completed the handshake and fetched the mapper. `GET /mock/status` reports playback progress for the launchers. |
| `scenarios/*.py` | One module per game: `initial_properties()` (the mapper) and `build()` (the steps, through the small `Scenario` DSL: `set`, `tick`, `wait`, `note`). `ROUTE` names the species/version of the empty route the app records into. |
| `run_quickstart.py` | The landing page's "Start Recording": the mock in `--shared-playback` mode, the app launched with no route and `XPR_SMOKE_ACTION=quickstart`, then a check of the saved route (version from the mapper name, solo mon / DVs / nature / ability from the first Pokémon, events recorded after the hand-off). |

The Rust app is driven with its smoke hooks (`XPR_SMOKE_ACTION=record|quickstart`,
`XPR_SMOKE_STOP_URL`, `XPR_SMOKE_SAVE_NAME`, …; see `KNOWN_ISSUES.md`).

## Running

```
py -3.14 docs/rust_port/recording/run_quickstart.py --scenario quickstart_yellow --rust-exe A:\...\rust\target\release\xpr-app.exe
py -3.14 docs/rust_port/recording/run_quickstart.py --scenario quickstart_emerald
```

Needs the Rust app built (`cargo build -p xpr-app`; `--rust-exe` for another
binary — pass an absolute path), a desktop session (the app opens a window),
and no other process on the port the driver picks. A run takes about half a
minute; `--work DIR` / `--keep` retain the app log (recording debug logging
is on), the mock log, the screenshot and the saved route.

Scenarios: `quickstart_yellow` and `quickstart_emerald` are for
`run_quickstart.py` (the party starts empty and the starter lands in slot 1 a
few seconds in). `emerald_geodude` (gen 3), `yellow_charmander` (gen 1,
deprecated mapper), `crystal_totodile` (gen 2, deprecated mapper),
`platinum_chimchar` (gen 4) and `emerald_thief`
(gen 3: Thief in trainer and wild battles, taking the loot off, selling it)
are full replay scenarios that walk every recorder path of their generation:
registration, area folders (and "Trip 2"), wild / trainer / multi-mon /
double battles with switches, level-ups with and without a learned move,
pickups, purchases, sales, item use, rare candy, vitamin, TM, HM, tutor, held
items, heal / save, trainer losses with blackouts, and soft resets. They have
no Rust-only driver yet; to use one, start `mock_gamehook.py --port N
--scenario scenarios/<name>.py`, then launch the app with
`XPR_GLOBAL_CONFIG_DIR=<scratch dir>`, `XPR_SMOKE_NEW_ROUTE=<version>|<species>`
(the scenario's `ROUTE`), `XPR_GAMEHOOK_URL=http://127.0.0.1:N`,
`XPR_SMOKE_ACTION=record`, `XPR_SMOKE_STOP_URL=http://127.0.0.1:N/mock/status`,
`XPR_SMOKE_SAVE_NAME=<name>` and `XPR_SMOKE_SCREENSHOT=<png>`; afterwards load
the saved route and check it has no errors. `emerald_geodude` also covers two gen 3 recorder regressions: a
sale right after a whiteout (the halving is a money change with no bag
change, and the sale's money lands before the bag slot) must come out as a
sale, and a champion win must survive the credits reboot (the recorder adds
the Hall of Fame autosave so the "reset" has nothing to roll back).

## Reading a problem

* `config/pkmn_router_logs.log` in the work dir: with recording debug logging
  on, the FSM logs every property change and every event it adds, so the
  first unexpected line usually names the cause.
* The app queues events from the GameHook thread and adds them from a
  processing thread that polls every 100 ms; the folder an event lands in
  depends on when it is processed relative to the next area change. The
  scenarios pause 0.75 s before a map change for that reason; real map changes
  take longer.
* The mock plays a scenario to the first client only (or to every client with
  `--loop`). `--shared-playback` instead plays it once, to whichever clients are
  connected as it goes: what a quick start needs, since the app swaps its
  detection client for the real recorder once the starter has been read.
* `Recorded Time` comes from Super Shuckie's live timer, so it differs between
  runs when Super Shuckie happens to be running.
