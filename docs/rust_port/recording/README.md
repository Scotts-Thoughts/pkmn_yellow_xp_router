# Recording parity harness

Replays a scripted GameHook session to the Python app and to the Rust app and
diffs the routes they save. Same inputs to both recorders, so any difference
in the saved bytes is a port divergence (or a timing artifact — see below).

## Pieces

| File | What it is |
|---|---|
| `mock_gamehook.py` | A stand-in for GameHook / Poke-A-Byte (stdlib only): `POST /updates/negotiate`, `GET /mapper`, `PUT /mapper/properties/*`, and the SignalR hub over a hand-rolled websocket. It plays a scenario as `PropertiesChanged` invocations once a client has completed the handshake and fetched the mapper. `GET /mock/status` reports playback progress for the launchers. |
| `scenarios/*.py` | One module per game: `initial_properties()` (the mapper) and `build()` (the steps, through the small `Scenario` DSL: `set`, `tick`, `wait`, `note`). `ROUTE` names the species/version of the empty route both apps record into. |
| `python_app_harness.py` | Boots the Qt app like `main.pyw` but with the global config dir and the GameHook URL redirected (monkeypatches, nothing in the Python tree changes), loads the route, records until the mock reports it is done, saves and quits. |
| `run_pair.py` | The driver: private config + data dirs per app, a fresh base route (made with the Python engine), a mock per app on a free port, both launches, then the comparison. |

The Rust app is driven with its smoke hooks (`XPR_SMOKE_ACTION=record`,
`XPR_SMOKE_STOP_URL`, `XPR_SMOKE_SAVE_NAME`, …; see `KNOWN_ISSUES.md`).

## Running

```
py -3.14 docs/rust_port/recording/run_pair.py --scenario emerald_geodude
py -3.14 docs/rust_port/recording/run_pair.py --scenario yellow_charmander --only rust --work C:\tmp\rec
py -3.14 docs/rust_port/recording/run_pair.py --compare-only a.json b.json
```

Needs the Rust app built (`cargo build -p xpr-app`; `--rust-exe` for another
binary), a desktop session (both apps open windows), and no other process
on the ports the driver picks. A run takes one to three minutes per app; the
work dir keeps both apps' logs (recording debug logging is on), the mock logs,
the Rust screenshot and both routes.

Scenarios: `emerald_geodude` (gen 3), `yellow_charmander` (gen 1, deprecated
mapper), `crystal_totodile` (gen 2, deprecated mapper), `platinum_chimchar`
(gen 4), `black_tepig` (gen 5). Each walks every recorder path of its
generation: registration, area folders (and "Trip 2"), wild / trainer /
multi-mon / double battles with switches, level-ups with and without a learned
move, pickups, purchases, sales, item use, rare candy, vitamin, TM, HM, tutor,
held items, heal / save, trainer losses with blackouts, and soft resets. All
five record byte-identical routes in both apps.

## Reading a difference

* Compare the two `config/pkmn_router_logs.log` files: with recording debug
  logging on, both FSMs log every property change and every event they add,
  so the first divergent line usually names the cause.
* The two apps process queued events on their own threads and the folder an
  event lands in depends on when it is processed relative to the next area
  change. The Rust processing loop polls exactly like Python's (100 ms), but a
  scenario that changes the map a few milliseconds after a battle can still
  land an event on either side of the folder boundary. The scenarios pause
  0.75 s before a map change for that reason; real map changes take longer.
* `Recorded Time` comes from Super Shuckie's live timer; the comparer only
  checks that both apps stamped (or did not stamp) an event.
