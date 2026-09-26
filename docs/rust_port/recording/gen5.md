# Gen 5 recording (Black, White, Black 2, White 2)

`rust/crates/xpr-recorder/src/games/gen5.rs` records the four gen 5 games through
the Poke-A-Byte "STANDARD/gen5" mappers (`pokemon_black.xml`, `pokemon_white.xml`,
`pokemon_black_2.xml`, `pokemon_white_2.xml`, all "Beta"). It replaced a gen 4
state machine with gen 5 switches (a port of the Python `BlackRecorder`) that had
never run against a real game. Everything below was measured on real runs played
back headlessly (see [Checking against real runs](#checking-against-real-runs)).

## How gen 5 memory behaves

| What | Measured behaviour | What the recorder does |
|---|---|---|
| Battle start / end | `battle.other.battle` goes 0 → 18002 → 21828 when a battle starts and back to 0 (or menu garbage) when it is over; `battle.other.battle_end` becomes 1 when the outcome is decided. The mappers build `meta.state` from these, but White 2's can stay at `To Battle` for a whole battle. | A battle is `battle.other.battle == 21828`; decided at `battle_end == 1`; finished once the word is cleared. `meta.state` is not used. |
| Party during a battle | `player.team.N` does not change until the battle ends: the experience, levels, moves, EVs, held item and the prize money of the whole battle land in one update, ~37 frames after `battle_end`, ~60 frames before the battle word clears. | Level-ups, learned moves, used-up held items and the money are taken from the party before vs after the battle. Every level gained is walked, so two level-ups in one fight both get their level-up moves. |
| In-battle progress | One battle struct per **party slot** (`battle.{player,opponent}.pkmn_ram.pokemon_N_ram`); `battle.opponent.active_pokemon` is party slot 0, not the mon on the field. The battle copy of the party (`battle.player.team.N`) gets each KO's experience. | An enemy has fainted when its slot's struct HP goes from > 0 to 0; the struct gives species and level. The experience arrives ~15 frames *before* the HP reads 0, so nothing is keyed on the order. |
| Trainer vs wild | `battle.opponent.id` is 0 when the battle word is set and gets the trainer id ~3-5 frames later; in the overworld it holds 550 (a real trainer id, Ranger Jaden) or stays 0 after a catch. | The id is only read while the battle is on; a battle whose id is still 0 when its first enemy faints is a wild one. |
| Enemy order, participants | `battle.opponent.party_position` / `battle.player.party_position` point at the mons on the field (garbage floats after the battle). | The trainer event's `mon_order` is the order the enemies fainted in, then any left standing in send-out order (Cheren sending his starter second came out as `[1, 4, 3, 2]`; in a double battle both leads are out from the start, so only the faints tell them apart); the player mons on the field while an enemy is out are its participants. Double battles (trainer data `double_battle`) count both leads, as gen 4 did. |
| Garbage party reads | The game decrypts a party Pokémon in place while it edits it (summary screen move order, items used from the bag). The mapper then reads garbage for 1-12 frames: species 40484, level 86, HP 29822/54833. Sometimes `flags.skip_checksum` is 3, sometimes not (a torn read). | Party/bag changes are only read once nothing has changed for 400 ms (`SETTLE`) and every slot is plausible (known species, level 1-100, HP within max, not flagged decrypted). |
| Stale party slots | The mapper only refreshes the slots below `player.team_count`. | Only slots below the count are read. |
| Move prompts | A level gained in battle has its forget-a-move choice made before the party update. After a Rare Candy (or an evolution) the level changes first and the choice comes ~900 frames later, after the dialogue. The Squirtle → Wartortle evolution scene of White 2 finished before the battle word was cleared, so the species changed in the end-of-battle update. | Level-up moves offered in the overworld and not in the move list yet are remembered until the next battle: when one appears, it is recorded as learned at that level (replacing the "not learned" event, which has the same level-up key). Evolutions are looked for in the end-of-battle update too. |
| TMs | Not used up. | A move learned in the overworld comes from an HM, then from a TM in the bag that teaches it, else a tutor / the Move Reminder (a Heart Scale spent). |
| Gym TMs | The leader's TM arrives in the overworld after the battle. | The trainer's `fight_reward` is dropped from the next bag gain (the processing thread's "previous event" check misses it once a held-item event sits in between). |
| Whiteout | The end-of-battle party update has every HP at 0 and the money reduced; the heal comes ~200 frames later with the warp. | Blackout (plus the trainer-loss flag and the defeated mons for a trainer); the heal that follows is not recorded as a Pokémon Center heal. |
| Heals | Pokémon Center, Mom, Nurse trainers: every party HP back to max with nothing else changing. | A Heal event when every party member is back at full HP and no item left the bag. |
| Saves | Each save block has a u16 save counter in its footer, and a save only rewrites (and counts up) the blocks that changed since the last save. The trainer-info block holds the play time, so every save rewrites it: its counter is at 0x2235014 (Black), 0x2235034 (White), 0x221EA94 (Black 2), 0x221EAD4 (White 2). The stock mappers read `flags.new_game` elsewhere: Black/White read bit 0 of the *party* block's counter (0x2234EE0 / 0x2234F00), which a save with the party exactly as it was at the previous save leaves alone (the White 2 run had two such saves; saving twice in a row does it too); Black 2 / White 2 read 0x221DA14 (-0x40), which never changes. | A flip of `flags.new_game` in the overworld is a save when the mapper reads it from a save counter (trainer-info block, or the party block with the gap above). A mapper that reads it anywhere else cannot report saves: the user is told once. `gen5_mappers_save_counter.patch` points all four mappers at the trainer-info counter. |
| Resets | A soft reset empties the party (count 0) and the money in one frame. Black/White and White 2 clear `player.player_id` with it; Black 2 keeps it. The save is loaded ~100 frames later, while the title screen is still up (party, money and save counter all back); Black/White have the player id back in the same frame, Black 2 / White 2 read garbage for it (White 2: 8354702 → 0 → 4096 → 3211313 → 0, ~950 frames at the replay's pace, longer if the player waits on the Continue menu) until the game goes on. | `player.team_count == 0` is a reset (the id alone is 0 now and then while B2W2 load). The save file is recognised by its Pokémon (a party PID the recorder knows), not by the player id, and the recorder waits for the real id before going on. What the route keeps is decided when the save loads, by comparing it with the state at the last save seen and the state at the reset: the last save → everything after it goes (the controller's reset rollback); the state at the reset → nothing goes, a save event marks the spot (a save the mapper missed); neither → back to the last save anyway, with a message, or with a mapper that reports no saves only a note. |
| Areas | Black/White: `overworld.map_index` is the zone id (4 bytes); the mapper names it from the *place-name* list, a different numbering (zone 7, the Striaton Gym, reads "Nacrene City"). Black 2/White 2: `overworld.map_index` is one byte holding the place-name id itself (117 = Aspertia City). | `games/gen5_places.rs`: the zone → place table and the place names read from the ROMs (byte 0x1A of each zone header in `a/0/1/2`; text file 89 / 109 of `a/0/0/2`). |

Verified: the trainer-info counter went up once per save on all 14 saves of the
Black run (a scan of the cartridge flash found the same 14 write bursts and no
others), all 10 of the White run, all of the White 2 run (including the two the
party counter missed) and on Black 2, and on a second save made right after the
first on Black and Black 2, where the party counter stayed put.

Resets, with the patched mappers (September 2026):

* White 2, the real run: saved, beat Veteran Rhona, reset, reloaded (through the
  garbage player ids), beat her again. The route holds the save and one Rhona.
  Frames 940k-1062k (Drayden, Plasma, Marlon): every save seen, three resets, each
  back to the latest save with the one change made since removed.
* Black, the Krookodile run: two resets in the middle of Elite Four fights, both
  back to the save before the fight; all 108 trainers match.
* Black, White and Black 2, scripted (`gen5_replay/scripts/*_resets.txt`, input
  injected by the feeder): save, toss an item, reset (the toss goes), toss one,
  save, toss another, reset (the last toss goes), reset again with nothing changed
  (nothing goes). Each route ends up as save, the second toss, save.

## Mapper problems worth fixing upstream

* All four mappers: `flags.new_game` should read the trainer-info block's save
  counter (`gen5_mappers_save_counter.patch`, one line per mapper). With the stock
  Black 2 / White 2 mappers saves are not recorded at all; with the stock Black /
  White mappers a save made with the party unchanged since the previous save is
  missed. A reset then rolls back to the save before it, unless nothing changed
  between the missed save and the reset (the loaded save is then recognised as
  the state at the reset).
* `pokemon_black.xml` / `pokemon_white.xml`: `overworld.map_name` looks the zone id
  up in the place-name list (see above).
* `pokemon_white_2.xml`: `meta.state` can stay at `To Battle` through a battle
  (the `battle_state_ready` address).
* The Black/White items pocket maps 40 of its 310 slots; an item past slot 40 is
  invisible to the recorder.
* `battle.opponent.active_pokemon` is party slot 0's struct, not the active mon.

## Known gaps

* **Not the recorder: the engine's gen 5 experience.** The route engine gives gen 5
  the gen 1-4 yield (`base * level / 7`, no level scaling; `xpr-data::exp::calc_xp_yield`).
  Gen 5 divides by 5 and scales by `((2L + 10) / (L + Lp + 10))^2.5`, so a recorded
  route's levels fall behind the game's (Landorus: route Lv14 vs game Lv25 after 32
  trainers; Bianca's Snivy Lv5 gave 43 EXP in game, 28 in the route). Events that
  depend on the route's level then fail: a TM taught over a level-up move the
  route's mon has not reached yet reports "Mon didn't have X", and purchases can
  exceed the route's (lower) money.
* Gym TMs in Black 2 / White 2: `fights_info.json` keyed the rewards by Black/White
  names; the B2W2 leaders (`Leader Roxie (157)`, …) are now listed too.
* Wings (+1 EV items) are recorded as plain item use: the engine has no wing
  support.
* Tag / multi battles (a second opponent): both trainers are recorded, but the
  second opponent's mons have no battle structs in the mappers, so their faints
  (and `mon_order`) are not followed.
* Only the first field position is followed for participants; triple and rotation
  battles are not modelled (the trainer data has none).
* Key items are not recorded (as in gen 4).

## Checking against real runs

There is no emulator in the loop: a Super Shuckie replay is played back headlessly
and its memory is served to a private Poke-A-Byte, which the app records from. The
pieces are in `gen5_replay/`:

| Piece | What it is |
|---|---|
| `shuckie-feeder/` | A small Rust program on `supershuckie-core`: plays a replay (seek, pace, pause, override the input) and serves the memory to Poke-A-Byte on its own UDP port (`EDPS_MemoryData_<port>.bin`). HTTP control on `--http` (`/status`, `/goto`, `/runto`, `/play`, `/press`, `/read`, `/dump`, `/trace`, `/savescan`, `/screenshot`, `/savestate`, `/loadstate`, `/savesram`, `/done`, …; see the top of `main.rs`). Without `--replay` it boots the ROM from `--sav` (a cartridge save, e.g. one `/savesram` wrote from another game's replay: Black 2 loads White 2's) and only `/press` drives it. Builds with MSYS2's UCRT64 cargo against a Super Shuckie checkout (paths in `Cargo.toml` / `build.rs`). |
| `pokeabyte-port.patch` | Lets a second Poke-A-Byte run beside the user's: `POKEAPROTOCOL_PORT` (and the matching shared-memory name), `POKEABYTE_CONFIG_DIR`; run it with `ASPNETCORE_URLS=http://localhost:8095`. |
| `run_rec.py` | Seeks the feeder, (re)loads the mapper, starts the app with `XPR_SMOKE_ACTION=quickstart` (off-screen, without focus) and records until the feeder reaches `--until`. With `--script` it plays a `drive.py` step file instead of the replay (`--no-seek` keeps a state loaded with `/loadstate`) and ends with `/done`. |
| `drive.py`, `scripts/` | Button presses, touch-screen taps and screenshots as steps (`a:4:40`, `tap@64,138:4:150`, `shot:name`); the reset sessions above. Menus differ between the games (Black 2 asks twice before saving, White leaves the "saved the game" box up, the bag remembers its pocket until a reset), so dry-run a script with its screenshots before recording it. |
| `fx.py`, `g5.py` | Feeder client; gen 5 decoding (party decryption, bag pockets, mapper glossaries). |
| `gt.py`, `compare.py`, `sample_counters.py` | Ground truth from a `/trace` of the same run (battles, faints, losses, bag and money changes) and a comparison with the recorded route; save counters sampled through a replay. |

A full run at 4x takes about an hour; a trace of one runs at ~300 frames/s.

A long playback can drift from the original run after a soft reset: the White 2
`/trace` from frame 395k missed two saves (at ~983k and ~1026k) that seeking to the
replay's keyframes shows, most likely because the game reseeds its RNG from the
emulated clock. Check the state after a reset with `/goto` (it starts from a
keyframe) rather than trusting a trace that ran through the reset. `gt.py` marks
a battle that a reset ended as `reset`, not as a win.
