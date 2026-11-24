# Move stats (recording)

While a route is recorded, the router counts every move both sides use in
battle: hits and misses, critical hits and secondary effects, each with the
chance the game really rolled. The numbers are stored in the route file and
served to the stream overlay. Spec: "Player profiles and move stats"
(2026-11-24).

## Where the code is

| Piece | File |
|---|---|
| Data types, JSON, interest groups, a move's effect kind | `rust/crates/xpr-engine/src/move_stats.rs` |
| Route file read/write, run total | `rust/crates/xpr-engine/src/router.rs` (`move_stats`, `move_stats_total`) |
| Detection per generation, the collector | `rust/crates/xpr-recorder/src/move_stats.rs` |
| Battle start/end, trainer link, resets, losses | `rust/crates/xpr-recorder/src/controller.rs` (`Session::track_moves`, `attach_move_stats`, `game_reset`, `lost_trainer_battle`) |
| Local server | `rust/crates/xpr-app/src/move_stats_server.rs` |
| "Recording → Move Stats..." window | `rust/crates/xpr-app/src/move_stats_view.rs` |
| Tests | `xpr-recorder/tests/move_stats.rs`, `xpr-engine/tests/move_stats_route.rs` |

## Mapper properties

All under `battle.move_tracking.*` (plus a few existing gen 3 paths under
`battle.flags.system.*`). A mapper without them records no move stats; nothing
else changes. Gen 5 has none yet.

## How a move is found

Each generation opens a "move window" when a move starts. Everything that
changes until the next move starts belongs to that move.

- **Gen 1:** `hWhoseTurn` changes, the player's PP drops, or the miss flags are
  cleared (`ExecutePlayerMove` / `ExecuteEnemyMove` clear them as a move starts).
  Two enemy moves in a row with nothing visible in between can merge.
- **Gen 2:** `wPlayerTurnsTaken` / `wEnemyTurnsTaken` go up. `BattleCommand_DoTurn`
  only runs once the move really executes (after sleep, paralysis and so on).
  The second turn of a charging move doesn't count up, so it merges with the
  first.
- **Gen 3:** `gCurrentTurnActionNumber` changes while the action is a move.
  `HandleAction_ActionFinished` clears the result flags in the same frame.
- **Gen 4:** the turn order counter (or turn number) changes while the battler
  whose turn it is picked FIGHT. MOVE_END clears the flags in the same frame.
  A turn only counts as a use once the move really ran: result flags, the
  battler's PP dropped, or its move showed while it was the attacker.

Flags are taken from change events only, so a value left over from the last
move never counts.

## What is counted

- **Hit chance:** `xpr-calc`'s accuracy formula with the stages at the start of
  the move, the real ability and held item (gens 3–4), and the species and
  level. Gen 1's 255/256 cap counts; in gen 2 a 100% move can't miss.
  Weather is not read yet (Thunder in rain or sun, Blizzard).
- **Crits:** one roll per damaging hit (gen 1: one per use). Fixed-damage and
  OHKO moves don't roll. Focus Energy: gen 1 as the game does it (it lowers the
  rate), +1 stage in gen 2, +2 in gens 3–4.
- **Secondary effect:** worked out from what changed (status, confusion, the
  flinch bit, stat stages), plus the game's own flags where reliable (gen 4's
  indirect-effect bit, gen 2's `wEffectFailed`). Not rolled when the move
  missed or failed. Counted as **blocked** (not a failed roll) when the target
  already had a status, was already confused, the stat was at ±6, the target
  fainted, or the type is immune (Fire can't burn, Ice can't freeze, Poison and
  Steel can't be poisoned). Chance: gen 2 reads the move's chance byte (/256),
  gens 3–4 double it for Serene Grace. Not handled: Substitute, Shield Dust,
  Mist, Clear Body.
- **Low-interest groups:** `interest` 1 (luck), 2 (only crits are random), 3
  (nothing is). Same rule as the overlay and the graphics program.

## Where a battle's numbers go

- **Trainer battle won:** on the trainer event (`move_stats`), once its
  opening event is in the route. Editing the event keeps them.
- **Wild battle:** added to the run's `wild` total.
- **Lost trainer battle:** the router takes its event out, so its numbers go
  to `reset_attempts` with `"outcome": "lost"`.
- **Reset:** trainer events since the last save, wild battles since the last
  save and a battle in progress go to `reset_attempts` with `"outcome": "reset"`.
- `total` (written on every save) = trainer events still in the route + wild.

## Route file compatibility

Old routes load and save byte-for-byte as before (no `move_stats` key is
written when there are none). **An older router, the Python app or an old iOS
build drops `move_stats` if it saves a new route again.** The automatic backup
keeps the previous file.

## Local server

`http://127.0.0.1:30161` (setting `move_stats_port` in the router's config),
only while recording. It refuses a `Host` other than 127.0.0.1/localhost and
`Sec-Fetch-Site: cross-site`, and sends no CORS headers (the overlay asks from
its main process).

- `GET /status` → `{ recording, game, run_id, solo_species }`
- `GET /move-stats` → the run object (`format`, `run_id`, `game`, `solo_species`,
  `recorded_with`, `total`, `wild`, `reset_attempts`) plus `battles` (every
  battle finished this recording session, trainer, wild and reset, with its
  `battle_id`, in order) and `in_battle`.

## Checking it in a game

Turn on the debug log: every counted move logs a `[MOVE STATS]` line (side,
move, hit, chance, crits, effect). Fight a battle, then open
Recording → Move Stats... and select the trainer event.
