# Gen 5 (Black) Mapper Gaps

This document records GameHook mapper paths that the Gen 5 recorder needs but
that the standard Black mapper does not currently expose.

It was produced by walking the structures in
`STANDARD/gen4/pokemon_platinum.xml` and `STANDARD/gen5/pokemon_black.xml`
and cross-referencing every path string referenced from
`black_gamehook_constants.py`.

Of the 77 unique mapper-path patterns the constants file references,
**17 are missing** from `pokemon_black.xml`. They split into two groups:

1. **Regressions vs. Platinum** — present in the Gen 4 mapper, missing in Black.
   These are the real Gen 5 mapper gaps.
2. **Dead in both mappers** — paths the constants file inherits from
   earlier-gen recorders but that never actually existed in the Gen 4 mapper
   either. Listed here for completeness; the Black recorder is no worse off
   than Platinum on these.

The black_gamehook_constants.py file has been patched so the recorder loads
and registers cleanly against the current Black mapper. Each missing path is
either repointed at a Black-mapper substitute or set to `None`, and
`ALL_KEYS_TO_REGISTER` filters out the `None` entries. The corresponding
features are flagged below as either "substituted", "degraded", or "disabled".

---

## Group 1 — Regressions vs. Platinum (14 paths)

### 1. Pokéball pocket — `bag.balls.{i}.item` / `bag.balls.{i}.quantity`
- Constants: `ALL_KEYS_BALL_TYPE`, `ALL_KEYS_BALL_QUANTITY` (40 indices each)
- Used by: inventory tracking — separate ball pocket loop in `black_fsm.py`
  (`update_item_cache` and friends).
- Black has `bag.{items, medicine, key_items, tmhm, berries, money}` but **no
  `bag.balls` section at all**.
- Fix in mapper: add a `bag.balls` `<class>` instantiation modeled after
  `bag.berries`. The Poké Ball pocket UI exists in BW so the data is in RAM;
  it just isn't mapped.
- Workaround: `ALL_KEYS_BALL_TYPE` / `ALL_KEYS_BALL_QUANTITY` are now empty
  lists. Ball pickups and ball usage will not be detected by the recorder
  until the mapper is fixed. **Status: disabled.**

### 2. Battle mode flag — `battle.mode`
- Constant: `KEY_TRAINER_BATTLE_FLAG`
- Used by: trainer-vs-wild battle detection. The Platinum recorder checks for
  `value == 'Trainer'`.
- Black has no top-level `battle.mode`. Closest analogues:
  `battle.other.battle_state_ready` and `battle.other.battle_state_ready_wild`
  (the existence of a `_wild` companion suggests these are how Black exposes
  the wild-vs-trainer split), and `battle.opponent.id` (non-zero implies a
  trainer battle).
- Fix in mapper: add a `battle.mode` string property mirroring Platinum's
  semantics, or document the canonical replacement.
- Workaround: `KEY_TRAINER_BATTLE_FLAG = None` for now. Trainer detection in
  the FSM will need to be reworked to consult `battle.opponent.id` or the
  `battle_state_ready_*` pair. **Status: disabled (pending FSM rework).**

### 3. Battle outcome — `battle.outcome`
- Constant: `KEY_BATTLE_OUTCOME`
- Black exposes `battle.other.outcome_flags` (already mapped separately as
  `KEY_BATTLE_FLAGS`) and `battle.other.outcome_flags_offset`, but no plain
  `battle.outcome`.
- The Gen 4 `battle.outcome` is an enum (`Win`, `Loss`, `Caught`, `Run`, …);
  `outcome_flags` is a bitmask. They're related but not drop-in.
- Workaround: `KEY_BATTLE_OUTCOME = None`. The FSM still has
  `KEY_BATTLE_FLAGS` to read end-of-battle state. The win/loss branch in
  `BattleState` may need to derive outcome from the flag bits instead.
  **Status: degraded.**

### 4. Opponent active mon PID — `battle.opponent.active_pokemon.internals.personality_value`
- Constant: `KEY_BATTLE_FIRST_ENEMY_PID`
- Used by: per-mon identity tracking inside a single battle (so the recorder
  can tell when the opponent switches between two mons of the same species).
- Black exposes the full `battle.opponent.active_pokemon.*` tree (species,
  level, stats, moves, …) but the `active_pokemon` class definition has no
  `internals` subobject. PIDs are reachable only via
  `battle.opponent.team.{i}.internals.personality_value` (per slot in the
  opponent's full team).
- Workaround: substituted with
  `battle.opponent.team.0.internals.personality_value`. This is correct as
  long as the opponent's lead mon is in slot 0, which is the common case.
  When the opponent rotates to a non-leading mon mid-battle the PID will
  stop matching the active mon. **Status: substituted (degraded).**
- Better fix: add `internals.personality_value` to the
  `battle_active_pokemon` class definition in the Black mapper, or have the
  recorder index into `battle.opponent.team[party_position]` dynamically.

### 5. Ally trainer id — `battle.ally.id`
- Constant: `KEY_BATTLE_ALLY_NUMBER`
- Used by: multi-trainer / tag battle detection.
- Black exposes the full `battle.ally.team.0..5.*` tree and
  `battle.ally.team_count` but no `battle.ally.id`. The opponent has both
  `battle.opponent.id` and `battle.opponent.trainer`, so the ally namespace
  is just incomplete.
- Workaround: `KEY_BATTLE_ALLY_NUMBER = None`. **Status: disabled.**

### 6. Double-battle "_2" slots (8 paths)
The Black mapper has only partial double-battle support. All of these are
missing:

| Constant | Path |
|---|---|
| `KEY_BATTLE_SECOND_ENEMY_SPECIES`   | `battle.opponent_2.active_pokemon.species` |
| `KEY_BATTLE_SECOND_ENEMY_LEVEL`     | `battle.opponent_2.active_pokemon.level` |
| `KEY_BATTLE_SECOND_ENEMY_HP`        | `battle.opponent_2.active_pokemon.stats.hp` |
| `KEY_BATTLE_SECOND_ENEMY_PID`       | `battle.opponent_2.active_pokemon.internals.personality_value` |
| `KEY_BATTLE_SECOND_ENEMY_PARTY_POS` | `battle.opponent_2.party_position` |
| `KEY_BATTLE_ALLY_MON_HP`            | `battle.player.active_pokemon_2.stats.hp` |
| `KEY_BATTLE_ALLY_MON_PID`           | `battle.player.active_pokemon_2.internals.personality_value` |
| `KEY_BATTLE_ALLY_MON_PARTY_POS`     | `battle.player.party_position_2` |

What Black *does* have: `battle.opponent_2.{id, team_count, trainer}`
(top-level scalars only — no `active_pokemon` subobject under it), and
`battle.player.party_position` (without `_2`). There is no `active_pokemon_2`
namespace anywhere, and `battle.opponent_2` doesn't have an `active_pokemon`
subobject or a `party_position`.

- Note: Black has a `battle.TESTING.*` namespace with `opponent_indirect_1..6`,
  `player_indirect_1..6`, `opponent_party_position_1..6`,
  `player_party_position_1..6`, and `enemy_state` — looks like a
  work-in-progress double-battle table that may eventually replace the
  missing `_2` slots once promoted out of TESTING. The mapper file's own
  header comment lists *"Map triple battle and rotation battle teams /
  trainer indexes"* as an open TODO.
- Workaround: all 8 constants set to `None`. Double / tag / rotation battles
  will not be recorded. **Status: disabled.**

---

## Group 2 — Dead in BOTH mappers (3 paths)

These constants exist in `black_gamehook_constants.py` (inherited from the
Gen 1/2/3 recorders, where audio sound effects and the `meta.saves` counter
were used as save and heal triggers) but were never wired into either the
Gen 4 Platinum mapper or the Gen 5 Black mapper. A raw text search confirms
none of these strings appear anywhere in either XML. The Platinum recorder
is just as exposed to this as the Black recorder.

| Constant | Path |
|---|---|
| `KEY_AUDIO_SOUND_EFFECT_1` | `audio.save_sound` |
| `KEY_AUDIO_SOUND_EFFECT_2` | `audio.heal_sound` |
| `KEY_SAVE_COUNT`           | `meta.saves` |

Both Gen 4 and Gen 5 use `meta.state` for the equivalent overworld/battle
detection, and `meta.state` *does* exist in the Black mapper. So the recorder
shouldn't actually need these three for its core flow.

- Workaround: all three constants set to `None`. **Status: disabled (no
  regression vs. Platinum).**

---

## Curiosities

These aren't gaps but are worth knowing while you build out the Gen 5 recorder:

- Black has `battle.field.weather` and `battle.field.weather_count` —
  Platinum doesn't. Useful for weather-aware damage logic.
- Black has `battle.wild.0..N.*` (full wild encounter mons) which Platinum
  doesn't expose. Useful for wild encounter recording.
- Black exposes a much richer `battle.player.active_pokemon.*` tree
  (modifiers, type_1/type_2, fainted flag, ability_temp, held_item_temp, …)
  than Platinum does.
- Black has `battle.other.battle_party_data_start`, `battle_header*`,
  `to_battle_pointer*`, `player_lock` — pointer plumbing that may enable
  smarter battle-state detection than the Gen 4 recorder uses.

---

## Methodology

```python
# Walked both XMLs collecting every (parent_path).property_name leaf,
# expanding <class type="X"> instantiations against the <classes> definitions.
# Cross-referenced against literal and f-string path patterns extracted
# from black_gamehook_constants.py.

constants_paths = 77   # unique patterns referenced
black_paths     = 5331 # leaves in pokemon_black.xml
plat_paths      = 3312 # leaves in pokemon_platinum.xml
missing         = 17   # constants paths absent from black
```
