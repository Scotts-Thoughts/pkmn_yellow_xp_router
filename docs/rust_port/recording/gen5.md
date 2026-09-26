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
| Saves | Every save increments a u16 counter in the footer of each save block. `flags.new_game` is bit 0 of the party block's counter in Black (0x2234EE0) and White (0x2234F00), so it flips on every save. In the Black 2 / White 2 mappers it reads 0x221DA14 (-0x40), which never changes; the party block's counter there is 0x221E958 (White 2) / 0x221E918 (Black 2). | A flip of `flags.new_game` in the overworld is a save, but only when the mapper reads it from one of those four addresses. Otherwise saves are not reported, the user is told once, and a reset leaves a note instead of rolling events back to a save that was never seen. |
| Resets | A soft reset clears `player.player_id`, the party count and the money within ~60 frames (`meta.state` "No Pokemon"). | `player_id == 0` is a reset. |
| Areas | Black/White: `overworld.map_index` is the zone id (4 bytes); the mapper names it from the *place-name* list, a different numbering (zone 7, the Striaton Gym, reads "Nacrene City"). Black 2/White 2: `overworld.map_index` is one byte holding the place-name id itself (117 = Aspertia City). | `games/gen5_places.rs`: the zone → place table and the place names read from the ROMs (byte 0x1A of each zone header in `a/0/1/2`; text file 89 / 109 of `a/0/0/2`). |

Verified on Black: the save counter increments exactly once per save (14 saves in
a 4h23 run), and a scan of the cartridge flash found the same 14 write bursts and
no others.

## Mapper problems worth fixing upstream

* `pokemon_black_2.xml` / `pokemon_white_2.xml`: `flags.new_game` should read the
  save counter at `0x221E958 - 0x40` / `0x221E958` (`length="1" bits="0"`), like
  the Black/White mappers read `0x2234EE0`. Until then B2W2 saves are not recorded
  and resets are not rolled back.
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
| `shuckie-feeder/` | A small Rust program on `supershuckie-core`: plays a replay (seek, pace, pause, override the input) and serves the memory to Poke-A-Byte on its own UDP port (`EDPS_MemoryData_<port>.bin`). HTTP control on `--http` (`/status`, `/goto`, `/runto`, `/play`, `/press`, `/read`, `/dump`, `/trace`, `/savescan`, `/screenshot`, …; see the top of `main.rs`). Builds with MSYS2's UCRT64 cargo against a Super Shuckie checkout (paths in `Cargo.toml` / `build.rs`). |
| `pokeabyte-port.patch` | Lets a second Poke-A-Byte run beside the user's: `POKEAPROTOCOL_PORT` (and the matching shared-memory name), `POKEABYTE_CONFIG_DIR`; run it with `ASPNETCORE_URLS=http://localhost:8095`. |
| `run_rec.py` | Seeks the feeder, (re)loads the mapper, starts the app with `XPR_SMOKE_ACTION=quickstart` (off-screen, without focus) and records until the feeder reaches `--until`. |
| `fx.py`, `g5.py` | Feeder client; gen 5 decoding (party decryption, bag pockets, mapper glossaries). |
| `gt.py`, `compare.py` | Ground truth from a `/trace` of the same run (battles, faints, losses, bag and money changes) and a comparison with the recorded route. |

A full run at 4x takes about an hour; a trace of one runs at ~300 frames/s.
