# Gen 6+ recorders: what a mapper has to expose

A working guide for adding a route recorder for a game past gen 5 (X/Y, Omega
Ruby/Alpha Sapphire, Sun/Moon, Ultra Sun/Ultra Moon, and whatever comes
after). It is written for an agent that has to (1) work out which Poke-A-Byte
mapper properties the recorder will need, (2) find the memory behind the ones
the mapper does not have yet, and (3) write the recorder. It was derived from
the gen 3, gen 4 and gen 5 recorders in `rust/crates/xpr-recorder/src/games/`
(`gen3.rs`, `gen45.rs`, `gen5.rs`) and from the mappers they run against in
`C:\Users\scott\AppData\Roaming\PokeAByte\mappers`.

Read in this order:

1. [How a recorder is wired](#1-how-a-recorder-is-wired) - the fixed parts
   (client, controller, host, data) a new machine plugs into, and the two
   machine designs that exist.
2. [Functionality a recorder needs](#2-functionality-a-recorder-needs) - every
   thing the existing recorders detect, the event each one produces, and the
   signal each generation reads it from.
3. [Property catalogue](#3-property-catalogue) - the properties to look for
   in a new mapper, with the gen 3/4/5 names, the type and timing the recorder
   expects, and where to hunt for each in a 3DS game.
4. [The gen 6 and gen 7 mappers today](#4-the-gen-6-and-gen-7-mappers-today) -
   what the mapper repo already has and what is missing.
5. [Procedure](#5-procedure) - the steps, in order, with the gap table to fill.
6. [Measurements to make before writing the machine](#6-measurements-to-make-before-writing-the-machine).

## 1. How a recorder is wired

### The fixed parts

| Piece | File | What it does |
|---|---|---|
| GameHook client | `xpr-recorder/src/gamehook.rs` | Connects to Poke-A-Byte (SignalR hub + REST), fetches `GET /mapper` into a `PropertyStore` (path -> `GameHookProperty { value, bytes_value, length, address, frozen }`), applies each `PropertiesChanged` batch one property at a time **in mapper order**, then calls `on_idle`. `resolve_case` finds a path case-insensitively. `address` is exposed, so a recorder can check *where* a mapper reads a property from (gen 5 does this for its save flag). |
| Session | `xpr-recorder/src/controller.rs` (`Session`) | Validates the mapper's `game_name` against the names `games::create_recorder` returns for the route's version (`names_game`: substring, but not the start of a sequel's name), calls `GameRecorder::on_mapper_loaded`, drops watched paths the mapper does not serve (with a warning), tells the user about invalid keys, then `startup`. |
| `GameRecorder` trait | `controller.rs` | `on_mapper_loaded(store) -> (paths to watch, invalid keys)`, `startup`, `handle_event(store, new, prev)` per changed property, `on_idle(store)` after each batch (or 500 ms of silence), `shutdown`. |
| Event queue + processing thread | `games/common.rs` (`ProcessCtx`, `EventQueue`) | Machines push `EventDefinition`s; a thread polls every 100 ms and runs the gen's `process_one`, which resolves mapper names against `GenData` and hands the event to the controller. The 100 ms lag is part of the behaviour: the folder an event lands in depends on the area change that is pending when it is processed. |
| Controller | `controller.rs` (`RecorderController`) | `entered_new_area` (a folder per area name, `Name: Trip N` on a revisit), `add_event` (finalises the pending folder, refuses a trainer already defeated, turns a level-up learn event into `update_levelup_move`, merges an item / vitamin / candy event into the previous one of the same kind), `game_reset` (deletes events back to the last save event and purges empty folders), `lost_trainer_battle` (deletes the trainer event), `check_final_trainer` (auto-stop). |
| Host | `xpr-recorder/src/host.rs` (`RecorderHost`) | What the app answers on the UI thread: previous events, folders, the route's DVs / solo species / held item / inventory, `can_evolve_into`, `is_valid_levelup_move`, `final_trainers`, messages. `StartInfo` carries the version, `GenData`, URL, DVs and species at start. |
| Data | `xpr-data` | `pkmn_db` (species names, level-up moves, evolutions), `move_db`, `item_db` (names, `sell_price`, `is_key_item`, the move a TM teaches), `trainer_db` (`get_trainer_by_id` = the game's trainer id, `money`, `double_battle`, `trainer_class`), `fights_info.json` (`fight_rewards` = the item a trainer hands over, `badge_rewards`, `major_fights`). |
| Quick start | `xpr-recorder/src/starter.rs` | The landing page's "Start Recording" with no route: `detect_version` from the mapper name, then party slot 1 (`player.team.0.species`, `.level`, `player.team_count`, `ivs.*`, `nature`, `ability`) once it has held a Pokémon for a second. |

### What a new game needs outside the recorder

None of this exists for gen 6+ yet; the recorder cannot be started without it.

| Where | What to add |
|---|---|
| `raw_pkmn_data/gen_<n>/` | `items.json`, `moves.json`, `type_info.json`, `fights_info.json` (`fight_rewards`, `badge_rewards`, `major_fights`, `trainer_timing_info`), and one `<pair>/` folder per game pair with `pokemon.json` and `trainers.json`. Every trainer needs the **`rom_id` the game uses** (the value the mapper's trainer-id property will read), `money`, `is_double_battle`, `trainer_class` (`Champion` triggers the Hall of Fame autosave). |
| `xpr-data/src/registry.rs` `builtin_sources` | The version -> folder mapping. |
| `xpr-data/src/model.rs` `Gen` | `Six`, `Seven` (and every `match` on `Gen`: `exp.rs` - gen 6 uses the flat gen 1-4 yield, gen 7 the scaled gen 5 one; `stats.rs`; `gen_data.rs` field moves). |
| `xpr-core/src/consts.rs` | `X_VERSION` etc., the version list, a colour. |
| `xpr-recorder/src/games/mod.rs` `create_recorder` | The version -> machine + accepted mapper names. |
| `xpr-recorder/src/starter.rs` `detect_version` / `detect_sibling` | Mapper-name keywords (order matters: `ultrasun` before `sun`, `omegaruby` before `ruby`), the second game of a pair mapper. |
| `raw_pkmn_data/gen_<n>/` and the engine | Anything the games do that the route engine cannot model yet shows up as recording "errors" (gen 5's scaled experience did until it was fixed). For gen 6+: the Exp. Share key item (whole-party experience), affection, Z-moves / Mega Evolution in damage calcs, the gen 7 Poké Ride replacing HMs. These are engine work, but list them when the recorder is planned so nobody debugs the recorder for them. |

### The two machine designs

**Change-driven FSM (gen 3, gen 4; `gen3.rs`, `gen45.rs`).** Every property
change is an input to a state machine (`Uninitialized`, `Resetting`,
`Overworld`, `Battle`, `InventoryChange`, `RareCandy`, `Tm`, `MoveDelete`,
`Vitamin`). A change to a bag slot enters `InventoryChange`; a change to slot
1's level enters `RareCandy`; a move slot enters `Tm` or `MoveDelete`; an EV
enters `Vitamin`. Each state waits a few ticks of `game_time.seconds`
(`DelayedUpdate`, `BASE_DELAY = 2`, `BATTLE_DELAY = 3`) for the rest of the
change to land, then diffs its cache (team, moves, items, money) on exit. In
battle the machine keys on individual changes: the enemy's HP reaching 0 caches
the species/level, the next change of the solo mon's experience attributes the
faint, `party_position` changes track switches for `exp_split`.

**Settled-snapshot diff (gen 5; `gen5.rs`).** Any change to a party, bag or
money path marks the store dirty and restarts a 400 ms timer. Once nothing has
changed for that long, the whole party + bag + money is read into a `Snap`,
validated (species known, level 1-100, HP within max, a personality value, the
"decrypted in place" flag clear, no bag stack over 999) and diffed against the
previous snapshot; the diff is turned into events by rules (money down + items
gained = purchase; EVs up + vitamin gone = vitamin; level up + Rare Candy gone =
candy; species changed = evolution; ...). Battles are followed from per-slot
battle structs after every batch (`update_battle`), and the whole battle's
outcome (experience, level-ups, moves, held items, prize money, whiteout) is
read from the party once the battle is over and the store has settled. All
decisions are made in `on_idle`, never mid-batch.

**Use the gen 5 design for gen 6+.** The reasons it was needed in gen 5 all
hold on the 3DS: the party is stored encrypted and decrypted by the mapper's JS
every frame (a torn read is garbage for a frame or two), the game writes the
party once at the end of a battle rather than as things happen, and the
mapper's derived properties (`meta.state`, `party_position`) are computed from
memory the game can leave stale. The FSM design's tick delays and
per-property triggers were tuned to GBA/DS timing and would need re-tuning
anyway. Start from `gen5.rs`: its `Gen5Keys` names are the target names for a
new mapper (section 3), so a gen 6/7 mapper that uses them lets the machine be
parameterised rather than rewritten.

### The event vocabulary

What `xpr_engine::EventDefinition` can carry (the recorder never invents other
event kinds):

| Event | Constructor | Fields the recorders fill |
|---|---|---|
| Trainer fight | `with_trainer` | `trainer_name` (the game's trainer id as a string until `process_one` resolves it), `second_trainer_name`, `exp_split` (participants per enemy mon, in enemy party order; empty when every entry is 1), `mon_order` (1-based, the order the enemies came out / fell), `thief_mons` (enemy party positions whose item was stolen), `custom_move_data` (Return's power from friendship), `pay_day_amount` (money gained beyond the expected prize). |
| Wild fight / an enemy mon | `with_wild` | species, level, quantity, `trainer_pkmn` (true for a trainer's mon recorded after a loss). |
| Item gained / lost | `with_item` | name, amount, `is_acquire`, `with_money` (purchase / sale), custom price. |
| Hold item | `with_hold_item` | the item or "hold nothing"; `consumed` for an item used up in battle. |
| Vitamin, Rare Candy | `with_vitamin`, `with_rare_candy` | name + count / count. |
| Learn move | `with_learn_move` | the move, the move it replaces (`destination_name`), source (`level-up`, a TM/HM item name, `tutor`), level, species; a level-up event with the move *not* learned is how "ignore this level-up move" is recorded. |
| Evolution | `with_evolution` | the new species. |
| Blackout | `with_blackout` | - |
| Heal, Save | `with_heal`, `with_save` | the area name (raw mapper value). |
| Notes only | `notes_only` | recording errors (`ERROR RECORDING! ...`) and the machine's internal flags (reset, trainer loss, "update the trainer event"). |

## 2. Functionality a recorder needs

Every row is something one of the gen 3+ recorders does. The "signal" columns
say what property each generation reads it from; a gen 6+ mapper has to offer
*some* signal for every row marked required. Property paths are the mapper's;
the ST prefix means the property only exists in the project's own mapper fork
(`mappers/ST/`), not in the stock `STANDARD/` one.

### A. Session and save file

| # | Capability | Event / effect | Gen 3 signal | Gen 4 signal | Gen 5 signal | Required |
|---|---|---|---|---|---|---|
| A1 | Recognise the mapper as the route's game | recording ready / "wrong mapper" | `game_name` contains `Pokemon Emerald` / `Pokemon FireRed & LeafGreen` (the ST mappers are named `... - Deprecated Mapper`) | contains `Pokemon Platinum` / `Pokemon HeartGold` / `Pokemon SoulSilver` (ST: `STP Pokemon Platinum`) | contains `Pokemon Black` etc., not followed by a digit | yes |
| A2 | Know a save is loaded and which one | init; `route_restarted` on a different file | `player.playerId` non-zero; a change = new file | `player.player_id` | `player.player_id` (B2W2 read garbage while loading; the file is recognised by a party PID instead) | yes |
| A3 | Detect a soft reset / return to the title screen | `Resetting`, then `game_reset` rolls the route back to the last save | `player.playerId == 0` **and** `pointers.dma1 == 0` | `player.player_id == 0` | `player.team_count == 0` (the party empties in one frame) | yes |
| A4 | Detect a save | Save event (+ the reset rollback point) | ST `pointers.sStpTracking == "SAVE"` - a value written by the **STP ROM patch**, not by the vanilla game | ST `meta.saves` goes up by 1 (a counter in the save block: `sSaveDataPtr + 0x20018`) | `flags.new_game` flips, **only** when the mapper reads it from the trainer-info block's save counter (the recorder checks the property's `address` against `SAVE_COUNTER_ADDRESSES`; the stock mappers need `gen5_mappers_save_counter.patch`) | yes (gen 5 degrades: without it a reset can only be undone when the loaded save matches a known state, and the user is told) |
| A5 | Decide what a reset undoes | delete back to the save, or add the save the mapper missed | always back to the last Save event | same | compare the loaded save with the state at the last save and the state at the reset (`settle_reset`) | yes (gen 5 logic; copy it) |
| A6 | Champion autosave / end of recording | Save event after the champion; auto-stop after a configured final trainer | `trainer_class == "Champion"` (data) | same | same | data only |

### B. Areas

| # | Capability | Event / effect | Gen 3 | Gen 4 | Gen 5 | Required |
|---|---|---|---|---|---|---|
| B1 | Name the area the player is in | a folder per area, `Trip N` on revisits; Heal/Save locations | `overworld.mapName` (glossary string) | `overworld.map_name` | `overworld.map_index` (zone id) through the recorder's own zone -> place table (`gen5_places.rs`), because the mapper names zones from the wrong list; `overworld.map_name` as the fallback | yes |

### C. Party and the solo mon

| # | Capability | Event / effect | Gen 3 | Gen 4 | Gen 5 | Required |
|---|---|---|---|---|---|---|
| C1 | Read every party slot | team cache / snapshot | `player.team.N.{species, level, ivAttack, ivDefense, ivSpeed, ivSpecialAttack, ivSpecialDefense}` | `player.team.N.{species, level, stats.hp, held_item, ivs.*, evs.*}` | `player.team.N.{species, level, exp, held_item, friendship, internals.personality_value, flags.skip_checksum, stats.hp, stats.hp_max, moves.M.move, ivs.*, evs.*}` and `player.team_count` | yes |
| C2 | Find the solo mon and follow it | everything below hangs off it | slot 0 species == route species, else species + IVs among gained mons (`MonKey`) | same | route species (or an evolution of it) + IVs, then by personality value in any slot | yes |
| C3 | Know a party read is garbage | skip the snapshot | (DMA: `pointers.dma1..3`, count > 100 ignored) | - | `flags.skip_checksum != 0` (decrypted in place), species unknown, level outside 1-100, HP > max, PID 0 | yes for the gen 5 design |
| C4 | Evolution | Evolution event + the new species' level-up moves at that level | solo mon lost, a gained mon with the same IVs that `can_evolve_into` | same | same PID, new species (also checked in the end-of-battle update; stones consumed from the bag first) | yes |
| C5 | Level-up (battle, Rare Candy) | level-up move learn / ignore events per level gained | `player.team.0.level` change (in battle: delayed 3 ticks; overworld: `RareCandy` state) | same | pre/post level in the snapshots, every level walked | yes |
| C6 | Which move a level-up move replaced | `destination_name` | slot diff of `player.team.0.move1..4` | `player.team.0.moves.M.move` | same | yes |
| C7 | Late forget-a-move prompt | the "not learned" event is replaced when the move appears later | - | `pending_evolution_level_check` | `pending_levelup` (after a Rare Candy / evolution the prompt comes ~900 frames after the level) | yes |
| C8 | Heal | Heal event | ST `pointers.sStpTracking == "HEAL"` (ROM patch) | ST `audio.heal_sound == 36335692` | every `stats.hp == stats.hp_max` with nothing leaving the bag (`healed && lost_nothing`) | yes |
| C9 | Quick start (first Pokémon) | the new route's version, species, IVs, nature, ability | `player.teamCount`, `player.team.0.{species, level, ivHp, ivAttack, ..., nature, ability(bool)}` | `player.team_count`, `player.team.0.{species, level, ivs.*, nature (index), ability (name)}` | same as gen 4 | yes |

### D. Moves from items and people

| # | Capability | Event / effect | Gen 3 | Gen 4 | Gen 5 | Required |
|---|---|---|---|---|---|---|
| D1 | TM / HM taught | Learn move with the TM/HM as source | a TM leaves the bag while a move changes (`Tm` state; `item_name_convert` keeps the `TMxx` prefix, `resolve_tm_name` expands it) | same | TMs are not used up: the move's TM is looked up in the bag (`tm_for_move`); HMs by name (`get_hm_name`) | yes |
| D2 | Tutor / Move Reminder / Sketch | Learn move, source `tutor` | a move changes with no TM lost | same | a move changes with no TM for it in the bag, or a Heart Scale left the bag | yes |
| D3 | Move deleter | Learn move with only `destination_name` | a move slot becomes null / duplicate | same | a move gone with no replacement | yes |
| D4 | A move learned during a battle that is not a level-up move | Learn move (tutor) | - | - | `other_move_events` on the post-battle diff | nice |

### E. Bag and money

| # | Capability | Event / effect | Gen 3 | Gen 4 | Gen 5 | Required |
|---|---|---|---|---|---|---|
| E1 | Item gained / lost | Acquire / Drop-Use | `player.bag.{items,pokeBalls,berries,tmhm}.N.{item,quantity}`, `player.bag.keyItems.N.item` (count 1) | `bag.{items,medicine,balls,berries,tmhm}.N.{item,quantity}` (key items **not** recorded) | same pockets, slot count discovered from the mapper; item ids collapsed to router names (`item_name`) | yes |
| E2 | Purchase / sale | `with_money`; a sale must match the sum of `sell_price`, else it is a use/drop | `player.bag.money` moved in `InventoryChange` (gain vs loss) | `bag.money` delta (`money_change_amount`) | money delta in the snapshot diff | yes |
| E3 | Premier Ball bonus (10+ balls) | free Acquire | - | gen 4 rule | same | yes (gen 4+) |
| E4 | Battle reward (gym TM) | the gain is dropped | previous event is that trainer and `fight_rewards` names the item (`process_one`) | same | `pending_reward` taken out of the next bag gain | yes (data) |
| E5 | Duplicate key item | the gain is dropped | `is_key_item && final_inventory_has` | same | same | data |
| E6 | Whiteout money loss | not an event; the sale that follows must still classify | `player.bag.money` change with no bag change enters `InventoryChange` | same | money is part of the snapshot | yes |
| E7 | Special shop prices | custom price | - | Goldenrod lottery (`GOLDENROD_LOTTERY_*`) | - | per game |

### F. Held items

| # | Capability | Event / effect | Gen 3 | Gen 4 | Gen 5 | Required |
|---|---|---|---|---|---|---|
| F1 | Give / take / swap the held item in the overworld | Hold item; the bag change it explains is consumed | `player.team.0.itemHeld` change + one bag stack lost/gained (`held_item_changed`) | `player.team.0.held_item` | `o.held != n.held` matched against the bag diff (`solo_changes`) | yes |
| F2 | Item used up in battle (berry, Focus Sash) | Hold item (nothing, consumed) | held item becomes null during the battle (3-tick delay, only while `partyPos == 0`) | same | pre vs post held item | yes |
| F3 | Thief / Covet from a trainer | `thief_mons` on the trainer event | held item appears from nothing; the enemy on the field (`battle.enemyPokemon.partyPos`) is the victim | same | `battle.opponent.team.N.held_item` going null names the slot | yes |
| F4 | Thief from a wild mon | Acquire + Hold | same, wild | same | same | yes |
| F5 | Held item moved to / from another party member | Hold nothing / Acquire + Hold | - | - | gen 5 rules (never touched the bag) | nice |

### G. Battles

| # | Capability | Event / effect | Gen 3 | Gen 4 | Gen 5 | Required |
|---|---|---|---|---|---|---|
| G1 | A battle started | `Battle` state; trainer event queued at the **start** (so items and levels during the fight land after it) | `battle.type.is_battle` true with `battle.outcome` null (init deferred to `on_idle`: the enemy party is in the same batch) | `meta.state == "Battle"` with `battle.outcome` null | `battle.other.battle == 21828` | yes |
| G2 | The outcome is decided / the battle is over | end-of-battle updates; back to `Overworld` | `battle.turnInfo.battleBackgroundTiles == 0`; `battle.outcome` non-null | `meta.state != "Battle"` (deferred while a blackout is being tracked) | `battle.other.battle_end == 1` (decided), then the word clears (over); the party update lands between the two | yes |
| G3 | Trainer or wild | trainer event vs wild events | `battle.type.trainer` | `battle.mode` (`Trainer` / `Wild` / null, derived in JS from the opponent trainer) | `battle.opponent.id != 0`, which arrives 3-5 frames after the word (an id still 0 at the first faint = wild) | yes |
| G4 | Which trainer(s) | `trainer_name` = the id, resolved through `trainer_db.get_trainer_by_id` | `battle.trainer.opponentAId` (+ `opponentBId` with `battle.type.two_opponents`; FRLG: `battle.trainer.opponentId`) | `battle.opponent.id`, `battle.opponent_2.id`, `battle.ally.id` (an ally = multi battle) | `battle.opponent.id`, `battle.opponent_2.id` | yes |
| G5 | Double battle | both leads share every enemy's experience; `mon_order` `[1, 2]` seeded | `battle.type.double` | a second trainer id | trainer data `double_battle` | yes |
| G6 | Enemy party size and order | `mon_order`, `exp_split` length | `battle.trainer.team.N.species` (count); `battle.enemyPokemon.partyPos` / `enemySecondPokemon.partyPos` changes record the order | `battle.opponent.team.N.{species, internals.personality_value}`, `battle.opponent.party_position` | `battle.opponent.team_count`, `battle.opponent.party_position`; order = faints, then first appearance | yes |
| G7 | An enemy fainted (species, level) | Wild event (wild) / trainer mon for a loss; attributes experience | `battle.enemyPokemon.hp == 0` caches `.species` / `.level`; the next `player.team.0.expPoints` change confirms | `battle.opponent.active_pokemon.stats.hp` 0 -> cached; `player.team.0.exp` or `battle.player.team.0.exp` confirms; PIDs (`internals.personality_value`) key the split | `battle.opponent.pkmn_ram.pokemon_N_ram.stats.hp` > 0 -> 0 (per party slot, no switching of slots); `.species`, `.level` | yes |
| G8 | Participants per enemy (`exp_split`) | `exp_split` | `battle.yourPokemon.partyPos` (+ `yourSecondPokemon.partyPos`) while the enemy is out; a player mon at HP 0 leaves the set | `battle.player.party_position(_2)`, `battle.player.active_pokemon(_2).stats.hp`, ally PID changes | `battle.player.party_position` + `battle.opponent.party_position` each batch; `battle.player.pkmn_ram.pokemon_N_ram.stats.hp` for faints | yes |
| G9 | Experience / level / moves / held item from the fight | level-up events, Learn move, Hold item | slot 0 changes during the battle (3-tick delays, `partyPos == 0` only) | same | pre vs post party snapshot | yes |
| G10 | Prize money and Pay Day | `pay_day_amount` = money delta minus the trainer's `money` (x2 with an Amulet Coin held) | `player.bag.money` at start and end | `bag.money` | snapshot money | yes |
| G11 | Return's power per enemy | `custom_move_data` | `player.team.0.friendship` at each faint | same | pre-battle friendship | nice |
| G12 | Bag use in battle (balls, potions) | Drop-Use | bag slot changes with a 3-tick delay | same | pre vs post bag | yes |
| G13 | Tutorial battle | nothing recorded | `battle.type.old_man_tutorial` | - | - | per game |
| G14 | Loss / whiteout | Blackout (+ trainer event deleted, the trainer's beaten mons recorded as trainer mons) | `battle.yourPokemon.hp <= 0` at `partyPos 0` -> `loss_detected` | `battle.player.team.0.stats.hp` 0 and every `battle.player.team.N.stats.hp` 0, then the map changes (`BlackoutData`) | every post-battle `stats.hp == 0`; the heal that follows is not a Heal event | yes |

### H. Timing and hygiene

| # | Capability | Gen 3 / 4 | Gen 5 | Required |
|---|---|---|---|---|
| H1 | A clock for delays | `gametime.seconds` / `game_time.seconds` ticks drive every `DelayedUpdate` | wall clock (`Instant`), 400 ms settle; `game_time.seconds` only filtered out of the debug log | only for the FSM design |
| H2 | Batch order | gen 3's enemy party arrives *after* the battle flag in the same batch (`init_after_batch`) | every decision in `on_idle` | yes: never act on one property of a batch |
| H3 | Registered paths | `all_keys_to_register` after `fix_case`; unmapped ones skipped | `Gen5Keys::all`; missing ones reported as invalid | yes |
| H4 | Name conversion | `Gen3Converter` / `Gen45Converter`: `name_prettify`, per-item spelling fixes (`Thunderstone` -> `Thunder Stone`, ...), `Mr. Mime` -> `MrMime`, TM prefix kept | same converter | yes: the glossaries must reach the router's `pokemon.json` / `items.json` / `moves.json` names through it |

## 3. Property catalogue

The properties to look for (or add) in a gen 6+ mapper. **Name new
properties like the gen 5 column**: `gen5.rs` reads exactly those paths, and a
gen 6/7 machine started from it then only needs the per-game switches. The
"expect" column is what the recorder assumes about the value; "3DS: where to
look" is the starting point for the hunt (PKHeX's `PK6` / `PK7` and
`SAV6` / `SAV7` sources document the structures; pkNX the trainer and text
data).

### Identity, clock, save file

| Concept | Gen 3 | Gen 4 (ST) | Gen 5 | Expect | 3DS: where to look |
|---|---|---|---|---|---|
| Mapper game name | `Pokemon Emerald - Deprecated Mapper` | `STP Pokemon Platinum` | `Pokemon Black - Beta` | contains the name `create_recorder` lists; `detect_version` keywords | `<mapper name=...>`; the gen 6/7 mappers are `Pokemon X and Y`, `Pokemon Omega Ruby and Alpha Sapphire`, `Pokemon Sun and Moon`, `Pokemon Ultra Sun and Ultra Moon` (pairs: the first game gets the route, the second is the sibling) |
| Game state (derived) | `pointers.sStpCallback1/2` -> `mainState` | `meta.state` (`Battle`, `Overworld`, ...) | `meta.state` (informational) | a string; gen 5 stopped trusting it | JS postprocessor; SM sets it from `battle.opponent_active.species` |
| Clock | `gametime.seconds` | `game_time.seconds` | `game_time.seconds` | +1 per game second | play time lives in the save block (SAV7 `PlayedTime`); SM's old `0x34197648` address is wrong (comment in the mapper) |
| Player / save id | `player.playerId` | `player.player_id` | `player.player_id` | non-zero while a save is loaded; stable per file; 0 (or garbage) while loading | TID16 at the save block's MyStatus (SM: `player.tid16`, `.sid`; `player.tid` is the 6-digit display id) |
| Party count | `player.teamCount` | `player.team_count` | `player.team_count` | 0..6; **0 = reset** in gen 5; only slots below it are refreshed | SM: not mapped; the party block's count byte (SAV7 `PartyCount`, after the 6 slots), or count slots whose EC != 0 in JS |
| Save signal | ST `pointers.sStpTracking` (ROM patch: 1 HEAL, 2 SAVE) | ST `meta.saves` (u32 counter, +1 per save) | `flags.new_game` (bit 0 of the trainer-info block's u16 save counter) | changes on every save, and on nothing else; a counter is best (gen 4's `n == p + 1` check) | the in-RAM save image: block footers / the save's write counter; failing that a "saved the game" message flag that flips per save. Verify it changes on a save made with nothing else changed (gen 5's party-block counter did not) |
| Reset signal | `player.playerId == 0 && pointers.dma1 == 0` | `player.player_id == 0` | `player.team_count == 0` | one frame is enough; must not fire during a map load or a menu | what the title screen clears: party count, player id, the save-block pointer the JS scans for |

### Area

| Concept | Gen 3 | Gen 4 | Gen 5 | Expect | 3DS: where to look |
|---|---|---|---|---|---|
| Area name | `overworld.mapName` | `overworld.map_name` | `overworld.map_index` (zone id) + recorder table; `overworld.map_name` | a stable string per area; sub-maps of one town should share it (a new folder per name); null keeps the current area | the current zone / map id in the overworld manager; names from the game's location text bank (pkNX) into a glossary or a recorder table like `gen5_places.rs` |

### Party slot `player.team.N.*` (N = 0..5)

| Field | Gen 3 | Gen 4/5 | Expect | 3DS (PK6/PK7, decrypted) |
|---|---|---|---|---|
| species | `species` | `species` | glossary string == `pokemon.json` name after `pkmn_name_convert`; null/empty = empty slot | 0x08 u16 |
| level | `level` | `level` | 1..100 | party stats 0xEC |
| experience | `expPoints` | `exp` | u32 | 0x10 |
| held item | `itemHeld` | `held_item` | glossary string or null | 0x0A u16 |
| friendship | `friendship` | `friendship` | 0..255 | 0x28 |
| identity | (species + IVs) | `internals.personality_value` | non-zero, unique per mon, stable through evolution and level-ups | PID 0x18: all four gen 6/7 mappers already map `internals.personality_value` (the EC at 0x00 would do as well) |
| decrypted-in-place tell | - | gen 5 `flags.skip_checksum` (3 while the game edits the mon) | 0 when the slot is readable | none known: the mapper JS decrypts from the live encrypted slot each frame. Rely on validation + settle, and measure how often a torn read appears (section 6) |
| HP / max HP | - | `stats.hp`, `stats.hp_max` | 0..max, max 1..999 | party stats 0xF0 / 0xF2 |
| moves | `move1`..`move4` | `moves.M.move` (M = 0..3) | glossary strings, null for an empty slot | 0x30..0x36 u16 (the `<moves>` block is commented out in all four gen 6/7 mappers; add `moves.M.move`; PP at 0x38 is welcome for PP tracking later) |
| IVs | `ivAttack`, `ivDefense`, `ivSpeed`, `ivSpecialAttack`, `ivSpecialDefense` (+ `ivHp` for quick start) | `ivs.{hp,attack,defense,speed,special_attack,special_defense}` | 0..31 | packed u32 at 0x74 (SM already maps `ivs.*`) |
| EVs | `evHp`, ... | `evs.{...}` | 0..252 | 0x1E..0x23 |
| nature | `nature` | `nature` | a name (`Nature::from_name`) or an index (`Nature::from_index`) | 0x1C |
| ability | `ability` (bool: second ability) | `ability` (name) | bool, name or index (`resolve_ability_idx`) | 0x14 (ability id) / 0x15 (ability number) |

### Bag

| Concept | Gen 3 | Gen 4/5 | Expect | 3DS |
|---|---|---|---|---|
| Pockets | `player.bag.{items,pokeBalls,berries,tmhm}.N.{item,quantity}`, `player.bag.keyItems.N.item` | `bag.{items,medicine,balls,berries,tmhm}.N.{item,quantity}` | every slot of every pocket the recorder should see (gen 5 discovers `N` from the mapper, so map the whole pocket); `item` a glossary string or null, `quantity` 0..999 | pouch records in the save image relative to MyStatus (SM: items and medicine are mapped; **balls, TMs, berries, key items, Z-crystals are not**). Gen 7 packs id and count in one u32 (`after-read-value-expression`) |
| Money | `player.bag.money` | `bag.money` | u32, changes only on purchases, sales, prizes, Pay Day, whiteout | SM: `bag.money` = MyStatus + 0x27F0 |
| TM names | `TM01` ... | `TM01` ... | the recorder expands the prefix to `items.json`'s `TMxx Move` name; gen 5+ TMs are not consumed | the TM pocket must be mapped for D1 |

### Battle

| Concept | Gen 3 | Gen 4 (ST) | Gen 5 | Expect | 3DS |
|---|---|---|---|---|---|
| Battle on | `battle.type.is_battle` | `meta.state == "Battle"` | `battle.other.battle == 21828` | true from the first frame of the battle to after the party is written | SM derives `battle.in_battle` from `opponent_active.species`, which may go stale after the battle: find the engine's own battle flag |
| Outcome decided | `battle.outcome` (null while undecided) | `battle.outcome`, `battle.other.outcome_flags` | `battle.other.battle_end == 1` | set when the last mon falls / the player runs / catches, before the party update | the battle result field; alternatively "every enemy HP 0 and the party changed" |
| Battle over | `battle.turnInfo.battleBackgroundTiles == 0` | `meta.state` | `battle.other.battle` cleared | after the party update | when the field structs are freed / the flag clears |
| Trainer id(s) | `battle.trainer.opponentAId` / `opponentBId` (Emerald), `battle.trainer.opponentId` (FRLG) | `battle.opponent.id`, `battle.opponent_2.id`, `battle.ally.id` | `battle.opponent.id`, `battle.opponent_2.id` | the game's trainer index == `trainers.json` `rom_id`; 0 for a wild battle; may arrive a few frames late, may hold junk outside a battle | the trainer battle parameters (the trainer index passed to the battle setup); pkNX numbers the trainers the same way |
| Battle type flags | `battle.type.{trainer,double,two_opponents,old_man_tutorial}` | `battle.mode` | (trainer data) | bools | the battle setup's rules word (single / double / horde / SOS / totem) |
| Enemy party count | `battle.trainer.team.N.species` (non-null count) | `battle.opponent.team.N.species` | `battle.opponent.team_count` | 1..6 (hordes: 5; SOS: grows) | SM: count `battle.opponent_team.N.species != 0` |
| Enemy on the field | `battle.enemyPokemon.partyPos` (+ `enemySecondPokemon`) | `battle.opponent.party_position` | `battle.opponent.party_position` | the party slot on the field (not the struct index) | SM: none; the same +0x1ED field flag the JS uses for the player side |
| Enemy struct per slot | `battle.enemyPokemon.{species,level,hp}` (field position) | `battle.opponent.active_pokemon.{species,level,stats.hp,internals.personality_value}`, `battle.opponent.team.N.*` | `battle.opponent.pkmn_ram.pokemon_N_ram.{species,level,stats.hp}` (party slot) | HP goes > 0 -> 0 exactly once per faint; species/level valid while the struct is live | SM: `battle.opponent_team.N.{species,hp,hp_max,level}` at `0x30004DB4 + 0x330 * N` (slots do not swap on switch) |
| Enemy held item | - | - | `battle.opponent.team.N.held_item` | goes null when stolen | the enemy's party block (not the field struct) |
| Player on the field | `battle.yourPokemon.partyPos` (+ `yourSecondPokemon`) | `battle.player.party_position(_2)` | `battle.player.party_position` | party slot 0..5 | SM: `battle.party_position` (JS, first slot with the field flag set); X/Y: `battle.player.N.slot_index` and `exp_participated` (bit 7 set once the mon has fought: the game's own participants list, worth looking for in SM's struct too) |
| Player HP per slot | `battle.yourPokemon.hp` | `battle.player.team.N.stats.hp`, `battle.player.active_pokemon(_2).stats.hp` | `battle.player.pkmn_ram.pokemon_N_ram.stats.hp` | 0 = fainted (no experience for that mon; all 0 = whiteout) | SM: `battle.player_team.N.hp` |
| Player experience in battle | `player.team.0.expPoints` | `battle.player.team.0.exp` | (post-battle party) | changes once per defeated enemy | measure whether the live party or the field struct carries it (section 6) |
| Ally (multi battle) | - | `battle.ally.id`, `battle.player.active_pokemon_2.internals.personality_value` | - | | SM: the ally slot in the player field structs |

### Sounds / tracking (the FSM designs only)

| Concept | Gen 3 | Gen 4 (ST) | Gen 5 | Note |
|---|---|---|---|---|
| Heal sound | `audio.soundEffect1/2` (registered, unused) | `audio.heal_sound` (`36335692`) | - | superseded by C8's HP rule; do not look for it on the 3DS |
| Save sound | - | `audio.save_sound` (superseded by `meta.saves`) | - | same |
| ROM-patch tracking | `pointers.sStpTracking`, `<patch>` `sStp*`, `STP_VAR_*` | - | - | gen 3 only works on the patched ROM; there is no patch for the 3DS games, so the save/heal signals must be found in the game's own memory |

## 4. The gen 6 and gen 7 mappers today

The stock mappers under `mappers/STANDARD/gen6` and `gen7` (`platform="N3DS"`,
version `0.0.1`) were started in March 2026 ("gen6 + 7 mappers", "unified gen6
& 7 mappers", "Adding addresses", "3ds work") and the working tree holds
further uncommitted work on all of them (`git status` in the mapper repo).
`git diff` there shows what changed most recently. The inventory below was
read from the working tree on 2026-09-26.

### How 3DS memory reaches the recorder

* Super Shuckie 2 runs the 3DS games (replays under
  `...\supershuckie 2\UserData\Pokemon Sun.3ds-data\replays\`, replay format
  v9, zstd-framed) and serves memory to Poke-A-Byte through the
  `EDPS_MemoryData.bin` shared memory, exactly as for the DS games.
* The 3DS address space is far bigger than the shared memory, so each mapper
  declares **`<memory><read start end/>` windows**, and only those bytes are
  served. Sun/Moon serves three: the save image in the linear heap
  (`0x330C0000-0x330F0000`), the live party (`0x34195E10-0x34196970`, six
  484-byte encrypted PK7 slots) and the battle field structs
  (`0x30000000-0x30040000`). **A new property is invisible until its address
  is inside a window**, and every extra byte costs poll time.
* Addresses differ per game and per game version, and the save image moves
  in the heap: the SM JS finds it by scanning the window for the trainer-card
  signature (`isTrainerBlock`: TID, game id 30-33, gender, a printable name,
  a plausible money value) and binds `save_block`, `heap_money`,
  `heap_items`, `heap_medicine` from it. Expect the same for anything else in
  the save image.
* The party is encrypted (PID/EC-seeded LCRNG over shuffled 56-byte blocks,
  PKHeX `PokeCrypto`); the JS decrypts each slot every frame into a named
  memory container (`player_party_structure_N`) the `party_pokemon` class
  reads from. A slot whose EC reads 0 is zero-filled.
* The scratch scripts left in the mapper repo root (`tmp_poll_mmf.py`,
  `tmp_dump_mmf_battle.py`, `tmp_scan_headers.py`, `tmp_scan_ptrs.py`,
  `tmp_walk_replay.py`) show the method that found the SM battle structs:
  dump the shared memory, search for species/max HP/HP/level signatures of the
  mons on the field, then search for pointers to the slot bases. They read the
  shared memory directly, without Poke-A-Byte.

### Sun / Moon (`STANDARD/gen7/pokemon_sun_moon.xml` + `.js`)

Mapped and usable:

| Property | Notes |
|---|---|
| `player.team.N.{species, dex_number, exp, ot_id, ability, nature, held_item, friendship, pokerus, nickname, ot_name, level, stats.{hp,hp_max,attack,defense,speed,special_attack,special_defense}, ivs.*, evs.*, flags.{is_egg,is_nicknamed}, internals.{personality_value,checksum,secret_id}}` | from the decrypted PK7; `level` and `stats.*` come from the party-stats section, valid once the game has filled it; `internals.personality_value` is the identity C2 needs; `hidden_power.*` is a placeholder with no address |
| `player.{name, tid, tid16, sid}` | from the scanned save block |
| `bag.money` | save block + 0x27F0 |
| `bag.items.0-29.{item,quantity}`, `bag.medicine.0-19.{item,quantity}` | 30 of 430 item slots, 20 of 64 medicine slots |
| `battle.in_battle`, `meta.state`, `meta.state_enemy` | derived in the postprocessor from `battle.opponent_active.species` |
| `battle.party_position` | the first player field slot whose +0x1ED flag is 1 |
| `battle.player_active.*`, `battle.opponent_active.*` | field structs; `opponent_active` is fixed at slot 0 |
| `battle.player_team.N.*`, `battle.opponent_team.N.*` (N = 0..5) | `species, hp_max, hp, ability, level, stats.*, modifiers.*, move_0..3.{id,pp,pp_max}` at `0x30002774 + 0x330 N` / `0x30004DB4 + 0x330 N` |

Missing for the recorder (section 2 rows in brackets):

| Gap | Rows |
|---|---|
| `player.team.N.moves.M.move` (the `<moves>` block is commented out, in all four gen 6/7 mappers) | C6, D1-D4 |
| `player.team_count` | A3, C1 |
| A player id the reset logic can use (`player.tid16` reads 0 only while the save block is not found) | A2-A3 |
| A save signal | A4-A5 |
| `overworld.map_index` / `map_name` | B1 |
| `game_time.seconds` (address unknown; the old one is wrong) | H1 (optional with the gen 5 design) |
| Bag pockets: balls, TMs, berries, key items, Z-crystals; the rest of the item and medicine slots | E1, D1, E3 |
| A real battle flag and an outcome / decided signal (`battle.in_battle` is inferred from the enemy struct) | G1-G2 |
| Trainer id(s), battle type (single / double / SOS / totem) | G3-G5 |
| `battle.opponent.team_count`, `battle.opponent.party_position` (which enemy is on the field) | G6-G8 |
| The enemy's held items | F3 |
| Where experience lands during a battle (live party vs field struct) | G7, G9 |

### X / Y (`STANDARD/gen6/pokemon_x_y.xml` + `.js`, 465 properties)

X/Y keeps its data in the process heap (`0x08xxxxxx` windows: the party at
`0x08C67554-0x08CE2807`, decrypted by the JS like SM's; the battle structs at
`0x08203ED4 + 0x244 N`, "may change between battles" per the comment), so
nothing found for SM carries over address-wise.

| Mapped | Notes |
|---|---|
| `player.team.0-5.*` | the same `party_pokemon` class as SM (no moves) |
| `player.{name, tid, sid}`, `bag.money` | literal addresses |
| `bag.items.0`, `bag.medicine.0`, `bag.key_items.0` | **one slot each**; `tm_hm` and `berries` declared with every slot commented out |
| `gametime.{hours, minutes, seconds}` | the only gen 6/7 mapper with a play time |
| `battle.party_position` | |
| `battle.player.0-5.*` | six player field structs; the gen 6 `battle_pokemon` class adds `slot_index` (+0x0D, the party slot in the struct) and `exp_participated` (+0x0F, bit 7 set once the mon has been in the fight, the game's own Exp. Share bookkeeping) - a direct participants signal for G8 that SM's struct should be checked for |
| `battle.opponent.0.*` | **one** of six opponent slots |
| `rival.name` | |

Missing: a battle flag (`getGamestate()` in the JS is hard-coded to
`Overworld`, so `meta.state` never changes), trainer ids, the other five
opponent slots, the bag pockets, a party count, a save signal, an area id.

### Omega Ruby / Alpha Sapphire (`pokemon_omega_ruby_alpha_sapphire.xml`, 332 properties)

Process heap like X/Y. Mapped: `player.{name, tid, sid}`, `player.team.0-5.*`
(same class), and a `test.team.0` debug leftover. The battle section is a
single `battle.player_active` / `battle.opponent_active` pair **at Ultra
Sun/Moon's addresses**, which the comment says read as zeros in ORAS. No bag,
no money, no play time, no battle flag (same hard-coded `Overworld`).
`meta.generation` is `7` (a leftover of the template every gen 6/7 file was
cloned from; should be `6`).

### Ultra Sun / Ultra Moon (`pokemon_ultra_sun_moon.xml`, 139 properties at HEAD)

The least developed: `player.team.0` only (the party base `0x33F7FA44` is the
first placeholder from the initial commit and was never confirmed),
`test.team.0`, and `battle.player_active` / `battle.opponent_active` at the
placeholder addresses `0x30002774` / `0x30009928`. Nothing else. Its windows
(`0x330128E4-0x33012EFC`, `0x33F7FA44-0x33F8059C`, `0x30002774-0x30009B50`)
mirror SM's layout, so SM's method (save-block scan, per-slot field structs)
is the starting point, with every address re-found.

### Across all four

* The `party_pokemon` class is one template: `species, dex_number, exp,
  ot_id, ability, nature (a stored byte, not PID % 25), held_item,
  friendship, pokerus, nickname, ot_name, level, stats.*, ivs.*, evs.*,
  flags.{is_egg, is_nicknamed}, internals.{personality_value, checksum,
  secret_id}`; `hidden_power.*` has no address; **`moves` is commented out
  everywhere**.
* Seven of the ten declared classes (`active_pokemon_move`, `battle_move`,
  `battle_structure`, `player_active_pokemon`, `pokemon_moves`,
  `usum_battle_move`, `wild_encounter`) are never instantiated; the `maps` /
  `map_group` glossaries exist but no property uses them.
* The paired versions (X vs Y, ...) were merged into one file per pair in
  "unified gen6 & 7 mappers", i.e. the pairs are treated as address-identical.
  Check that on the second game of each pair before trusting it.
* The JS helper exports `getBattleMode`, `getBattleOutcome`,
  `getEncounterRate`, `getMetaEnemyState` are stubs returning null; only SM's
  `postprocessor` sets a state at all.

## 5. Procedure

Work one game pair at a time. Each step produces something that is checked in
(a table in a `gen6.md` / `gen7.md` next to `gen5.md`, a mapper change in the
mapper repo, a test), so the next agent can pick it up.

### Step 0 - prerequisites

* The data set for the pair (section 1, "What a new game needs outside the
  recorder"). Without `trainers.json` `rom_id`s no trainer fight can be
  recorded, and without `fights_info.json` no gym TM is skipped. Check the
  trainer ids against the mapper's trainer-id property on the first trainer
  battle you measure, before doing anything else with trainers.
* Super Shuckie 2 with the ROM, a save near the start, and Poke-A-Byte with
  the mapper loaded. Confirm the shared memory is being served (the tmp
  scripts open `EDPS_MemoryData.bin`).
* The app built (`cargo build -p xpr-app`) with recording debug logging on:
  the FSM logs every property change (`Change of <path> from .. to ..`), which
  is how the timings in section 6 are measured.

### Step 1 - inventory the mapper

Dump the loaded mapper's properties (`GET http://localhost:8085/mapper` gives
every property with `path`, `type`, `address`, `value`; the SM XML is small
enough to read directly) and fill this table for the pair. One row per
catalogue concept (section 3); the "status" column is one of `mapped`,
`mapped but wrong` (with why), `missing`, `not needed`.

```
| Concept (section 3) | Mapper path | Type | Address / window | Status | Rows it serves | Notes |
```

Every `missing` row with a required section 2 row behind it is a work item.
Order them: identity + party (C1-C3) first (nothing works without a readable
party and a solo mon), then battle flag / trainer id / enemy structs
(G1-G8), then bag pockets + money (E), then save / reset (A3-A5), then area
(B1), then the nice-to-haves.

### Step 2 - find the memory for each gap

For each work item:

1. **Start from what is known.** The PKHeX `PK6`/`PK7` layout for party fields;
   `SAV6`/`SAV7` for everything in the save image (offsets are relative to a
   block the JS has to locate, as SM's `findSaveBlock` does for MyStatus);
   pkNX for trainer indices, item and move ids, location names. The existing
   mapper comments record what was confirmed and how.
2. **Search the served memory.** Dump the window (`tmp_dump_mmf_battle.py`
   pattern) at two moments where the value is known to differ (before / after
   a purchase, a save, a map change, a trainer battle start) and diff.
   Species ids, HP pairs and levels of a known mon make good signatures for
   structs; a trainer battle's trainer index (from pkNX) for the trainer id.
3. **Widen the window** if the value is outside the served ranges: add a
   `<read>` and re-check the poll cost.
4. **Confirm the semantics the recorder expects** (section 3 "expect"
   column): value range, when it changes, what it holds outside its context
   (gen 5's `battle.opponent.id` held a real trainer id in the overworld).
5. **Add the property with the gen 5 name** (section 3), plus a glossary
   (`reference=`) whose strings match the router's data after
   `Gen45Converter` (or extend the converter, as `Mime Jr` / `Giratina` did).
6. Record the address, the game version it was found on, and the check that
   confirmed it in the XML comment and in the pair's doc.

If a concept has no clean memory signal, look for the gen 5 fallback before
inventing a patch: heals from HP restored with no item lost, wild-vs-trainer
from the trainer id staying 0, the enemy order from faints + first
appearance, a missed save recognised when the loaded save equals the
pre-reset state.

### Step 3 - measure the behaviour

Section 6 lists the measurements. Each one is a row in the pair's doc in the
form `gen5.md` uses (what was measured / what the recorder does about it).
Do them with the app recording against the mapper and the debug log open, or
with a script on the shared memory when frame-level timing matters.

### Step 4 - write the mapper changes

* Keep the recorder-facing properties in the mapper the recorder names
  (`create_recorder`). For gens 3 and 4 the project keeps its own forks under
  `mappers/ST/` because the stock mappers lack the save/heal signals; gen 5
  uses the stock mappers plus a one-line patch. Decide which it is for the
  pair and record it in the pair's doc so the user knows which mapper to load.
* Derived values (`meta.state`, `party_position`, a decrypted party) live in
  the JS `preprocessor` / `postprocessor`; keep them cheap, they run every
  poll.
* Declare every new address inside a `<memory><read>` window.

### Step 5 - write the machine

* Copy `gen5.rs` to `gen6.rs` (or `gen7.rs`); keep `Gen45Converter` and
  `process_one` from `gen45.rs` unless the names need a new converter.
  Parameterise what the measurements showed differs (the battle word / flag,
  the decided signal, the save signal and its address check, the area table,
  bag pockets, the reset signal).
* Register it: `games/mod.rs` `create_recorder` (with the accepted mapper
  names), `starter.rs` `detect_version` / `detect_sibling`, `consts.rs`
  versions, `registry.rs` sources.
* Tests: `xpr-recorder/tests/gen5_fsm.rs` drives the gen 5 machine with a
  `FakeHost` and a scripted `PropertyStore` that behaves as measured; write
  the same for the new gen from the section 6 table (the test *is* the
  behaviour table). Add a mock-harness scenario under
  `docs/rust_port/recording/scenarios/` for the quick start
  (`run_quickstart.py`).
* Document the pair in `docs/rust_port/recording/gen<n>.md` (behaviour table,
  mapper problems worth fixing upstream, known gaps) and add it to
  `README.md` and `KNOWN_ISSUES.md`.

### Step 6 - verify against a real run

Gen 5 was verified by playing Super Shuckie replays back headlessly into a
private Poke-A-Byte (`gen5_replay/`, `shuckie-feeder`) and comparing the
recorded route with ground truth from a memory trace (`gt.py`, `compare.py`).
The feeder links melonDS and mGBA only (`shuckie-feeder/build.rs`); a 3DS
run needs the 3DS core from Super Shuckie 2 added to it, or the recording
done live against the emulator with the replay driven by hand. Either way,
record a run that has: a wild fight, a trainer fight with a switch, a double /
horde / SOS battle, a level-up with and without a learned move, a TM, a
purchase and a sale, a held item given and used up, a Rare Candy, a vitamin,
a heal, two saves, a reset after each, a whiteout, and an evolution. Then
check the route loads with no `ERROR RECORDING!` notes.

## 6. Measurements to make before writing the machine

Everything gen 5 had to learn the hard way, as questions. Answer each on the
real game and put the answer in the pair's doc.

| Area | Question | Why it matters |
|---|---|---|
| Battle start | Which value changes first when a battle starts, and is it set for wild and trainer battles alike? Does it hold a distinct value ("battle word") or a bool? | G1; gen 5 needed the word, not `meta.state` |
| Battle end | What is set when the outcome is decided, when does the party get its update (experience, level, moves, EVs, held item, money) relative to it, and when is the battle flag cleared? Is the party written once or as things happen? | G2, G9; gen 5 reads pre/post because the party updates once, ~37 frames after `battle_end` |
| Trainer id | When after the battle flag does the trainer id become valid? What does it read in the overworld and after a wild battle? Does a wild battle ever show a non-zero value? | G3-G4 |
| Enemy structs | One struct per enemy party slot or per field position? Do slots swap on a switch? Does HP go > 0 -> 0 exactly once per faint? What do the structs read before the first send-out and after the battle? | G6-G8; gen 5's `active_pokemon` was slot 0, not the mon on the field |
| Participants | Which property says which party slot is on the field, for the player and the enemy? In a double / horde / SOS / totem battle, how many player slots are out and how are the enemies numbered? Does an SOS ally get a new struct? | G8; gen 5 follows only the first field position |
| Experience | Does the live party's `exp` change during the battle (gen 3/4) or only at the end (gen 5)? Does the field struct carry experience? | G7, G9 |
| Whiteout | What does the party look like after a lost battle (all HP 0?), when does the heal come, does the money halve in the same update? | G14, C8 |
| Garbage reads | Open the summary screen, reorder moves, use an item from the bag, give a held item: does any party field read garbage for a frame? How long? Is there a flag while it happens? | C3; gen 5 saw 1-12 frames of garbage with `skip_checksum == 3` |
| Party refresh | Are slots past the party count refreshed, or stale? | C1 |
| Level-up prompts | After a Rare Candy / evolution in the overworld, how long after the level change does the move choice land in the party? In battle, before or after the party update? | C7 |
| TMs | Consumed or not (gen 5+: not)? Does teaching change anything in the bag? | D1 |
| Evolution | Does the species change before the battle flag clears (gen 5: yes for a post-battle evolution)? Is the evolution scene visible in the party at all? | C4 |
| Save | Which value changes on every save (including a save with nothing changed since the previous one, and two saves in a row)? At which address? Does loading a save after a reset change it too? | A4; gen 5's stock mappers read the wrong counter |
| Reset | What does memory look like the frame the game returns to the title screen (party count, player id, the save-block signature)? What is garbage while the save loads, and for how long? | A3, A5 |
| New game | What changes when a new file is started over an old one? | A2 |
| Area | Which id changes when the player crosses a map boundary, and does one town have one id or many? Is there a name table in the game text? | B1 |
| Heal | Does a Pokémon Center heal change anything except HP / PP / status? Do Mom / nurse trainers / the free heals in gen 7 look the same? | C8 |
| Bag | Which pockets exist, how many slots each, are stacks capped at 999, does a pocket keep an ordering the diff can rely on? Where do key items and Z-crystals live? | E1 |
| Money | Purchases, sales, prize money, Pay Day, Amulet Coin: does the game's prize money match `trainers.json` `money` x last level (gen 5 convention)? | E2, G10 |
| Held items | Does the player's held item change in the live party during the battle or only at the end? Does the enemy party block show its held items, and do they go null when stolen? | F2-F3 |
| Timing | How fast does Poke-A-Byte poll the three windows, and how many properties change per batch in the overworld (walking) vs in battle? | H2; the 400 ms settle assumes a quiet overworld |
