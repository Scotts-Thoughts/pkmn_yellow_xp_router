# Gen 3 move data verification report

App data: `raw_pkmn_data/gen_three/moves.json` (354 moves), `type_info.json`, `items.json`.
ROM reference: pokeemerald (primary reference below), cross-checked against pokeruby (RS) and
pokefirered (FRLG). All three decomps agree byte-for-byte on `gBattleMoves`/`gTypeEffectiveness`
struct **layout** (bit-identical `FLAG_*`/`F_*` move flags, identical `MOVE_TARGET_*`/`TARGET_*`
values, identical `EFFECT_*` ids — two ids were simply renamed between ruby and emerald/firered
with no behavior change: id 150 `EFFECT_FLINCH_HIT_2`(ruby)/`EFFECT_FLINCH_MINIMIZE_HIT`(emerald),
id 151 `EFFECT_SOLARBEAM`/`EFFECT_SOLAR_BEAM`, id 155 `EFFECT_FLY`/`EFFECT_SEMI_INVULNERABLE`).

App `rom_id` = ROM `MOVE_*` constant **+ 1** (e.g. app `Pound.rom_id=2` ↔ `MOVE_POUND=1`). All 354
app moves matched a ROM move by this offset with matching names across the full range (spot-checked
every 15th move end‑to‑end, all matched).

Method: wrote Python parsers for `battle_moves.h`/`.c`, `constants/moves.h`,
`constants/battle_move_effects.h`, `constants/pokemon.h`, `gTypeEffectiveness`
(C array form for emerald/firered, `.inc` byte form for ruby), and `items.h`; merged with the
app JSON keyed by move id; ran field-by-field diffs; then hand-verified every anomaly and every
move-ID-specific script branch (`jumpifmove`/`jumpifnotmove MOVE_*`) by reading
`data/battle_scripts_1.s` and `src/battle_script_commands.c`. Scripts live in
`scratchpad/gen3work/` (`parse_rom.py`, `build_merged.py`, `diff_numeric.py`,
`effect_flavor_scan.py`, `diff_effects_chance.py`, `parse_typechart.py`, `diff_typechart.py`).

Target-string mapping inferred and confirmed with **zero exceptions across all 354 moves**:

| app `target` | ROM constant | value | moves |
|---|---|---|---|
| `target_single_enemy` | `MOVE_TARGET_SELECTED` | 0 | 246 |
| `target_depends` | `MOVE_TARGET_DEPENDS` | 1 | 9 |
| *(unused by any gen3 move)* | `MOVE_TARGET_USER_OR_SELECTED` | 2 | 0 |
| `target_random_enemy` | `MOVE_TARGET_RANDOM` | 4 | 4 |
| `target_both_enemies` | `MOVE_TARGET_BOTH` | 8 | 22 |
| `target_self` | `MOVE_TARGET_USER` | 16 | 67 |
| `target_all` | `MOVE_TARGET_FOES_AND_ALLY` | 32 | 5 |
| `target_enemy_field` | `MOVE_TARGET_OPPONENTS_FIELD` | 64 | 1 |

Flag mapping (also zero exceptions across 354 moves × 6 flags = 2124 checks):
`contact`→`FLAG_MAKES_CONTACT`, `affected_by_protect`→`FLAG_PROTECT_AFFECTED`,
`affected_by_magic_coat`→`FLAG_MAGIC_COAT_AFFECTED`, `affected_by_snatch`→`FLAG_SNATCH_AFFECTED`,
`affected_by_mirror_move`→`FLAG_MIRROR_MOVE_AFFECTED`, `affeted_by_kings_rock`(app's own
spelling)→`FLAG_KINGS_ROCK_AFFECTED`.

---

## 1. Mismatch table (app vs. pokeemerald reference)

| Move | Field | App value | ROM value | Citation | Suggested fix |
|---|---|---|---|---|---|
| Curse | `type` | `"Ghost"` | `TYPE_MYSTERY` (id 9) | `pokeemerald/src/data/battle_moves.h:2265-2276` (`.type = TYPE_MYSTERY,`) | See note A below — not a clean 1‑field fix |
| Nature Power | `base_power` | `1` | `0` | `pokeemerald/src/data/battle_moves.h:3474-3484` (`.power = 0,`) | Change `1`→`0` (cosmetic only, see note B) |
| Tri Attack | `effects[0..2].chance` | `6.3` (×3) | `20` ÷ 3 = `6.6667` | struct: `battle_moves.h:2096-2106` (`secondaryEffectChance = 20`); 3‑way split: `pokeemerald/src/battle_script_commands.c:2601` (`Random() % 3 + 3`) | Change all three `6.3` → `6.67` |
| Bounce | `effects[0]` | `{"flinch": true, "chance": 30}` | Applies `MOVE_EFFECT_PARALYSIS` (id 5), not flinch | `pokeemerald/data/battle_scripts_1.s:1997-2005` (`BattleScript_SecondTurnSemiInvulnerable`: `jumpifnotmove MOVE_BOUNCE, BattleScript_SemiInvulnerableTryHit` / `setmoveeffect MOVE_EFFECT_PARALYSIS`) | Change to `{"status": "Paralysis", "chance": 30}` |
| SonicBoom | `base_power` | `1` | `1` (matches struct, but real fixed damage is `20`) | struct: `battle_moves.h:640-650`; script: `pokeemerald/data/battle_scripts_1.s:1720-1728` (`setword gBattleMoveDamage, 20`) | Change `1`→`20` for consistency with Dragon Rage (see note C) |
| Dragon Rage | `base_power` | `40` | `1` (struct placeholder; real fixed damage is `40`) | struct: `battle_moves.h:1069-1079`; script: `pokeemerald/data/battle_scripts_1.s:819-829` (`setword gBattleMoveDamage, 40`) | **No fix needed** — app value is the correct effective damage |
| Struggle | `type` | `"none"` | `TYPE_NORMAL` (struct value; bypassed) | struct: `battle_moves.h:2148-2159`; bypass: `pokeemerald/src/battle_script_commands.c:1542` (`u8 TypeCalc(...) { if (move == MOVE_STRUGGLE) return 0; ...` — skips both type-effectiveness *and* STAB) | **No fix needed** — app is correct; ROM's own damage-calc special-cases Struggle by move ID, ignoring its `.type` field |
| Vital Throw | `accuracy` | `null` | `100` (struct value; bypassed) | struct: `battle_moves.h:3032-3042`; bypass: `pokeemerald/src/battle_script_commands.c` `AccuracyCalcHelper()`, `... || gBattleMoves[move].effect == EFFECT_VITAL_THROW) { JumpIfMoveFailed(...); return TRUE; }` (same always-hit path as `EFFECT_ALWAYS_HIT`) | **No fix needed** — confirmed always-hit despite non‑zero accuracy field |

(Tri Attack and Bounce above are `effects[]`-array issues, covered in depth in section 3 — they are
included here because they're mismatches, not because they touch `base_power`/`type`/`accuracy`.)
Restricting to the six struct-level fields (`base_power`/`type`/`accuracy`/`pp`/`priority`/
`target`), only 6 of 354 moves have any discrepancy at all (Curse, Nature Power, SonicBoom, Dragon
Rage, Struggle, Vital Throw — the last two confirmed non-bugs); the other 348 moves match the ROM
on all six fields with no caveats. All 2124 boolean-flag checks (`contact`/`affected_by_protect`/
`affected_by_magic_coat`/`affected_by_snatch`/`affected_by_mirror_move`/`affeted_by_kings_rock` ×
354 moves) matched the ROM's `FLAG_*` bits with zero exceptions, and all 354 `target` values
matched with zero exceptions (mapping table above).

**Note A — Curse's type.** The struct field is `TYPE_MYSTERY`, not `TYPE_GHOST`. It only resolves
to Ghost conditionally: `pokeemerald/src/battle_script_commands.c:7407-7411` (also repeated at
7433-7437) —
```c
if (moveType == TYPE_MYSTERY) {
    if (IS_BATTLER_OF_TYPE(gBattlerAttacker, TYPE_GHOST))
        moveType = TYPE_GHOST;
    else
        moveType = TYPE_NORMAL;
}
```
i.e. Curse only "counts as" Ghost-type (for same-type-move eligibility checks such as
Metronome/Assist move pools) when the *user* is already Ghost-type; otherwise it counts as
Normal. It never goes through `TypeCalc`'s STAB/type-effectiveness table as a damaging hit (the
Ghost-curse variant applies a volatile HP-drain status instead), so the mislabel has limited
practical impact, but `"Ghost"` overstates the real value and hides the Normal-type fallback.
Recommend either a `"Mystery"`/`"none"`-style sentinel type (Struggle already has precedent for a
`"none"` pseudo-type) or a code comment/caveat if the schema can't express the conditional.

**Note B — Nature Power's power.** `EFFECT_NATURE_POWER` calls another move dynamically (in gen 3,
always Swift, since there is no overworld terrain system yet); the move's own `.power` field is
unused filler. All three repos have it at `0`; the app's `1` is a harmless one-off (compare
`Psywave`, `Counter`, `Mirror Coat`, `Seismic Toss`, `Night Shade`, `Super Fang`, `Flail`,
`Reversal`, `Endeavor` — every other formula/variable-damage move in the app correctly mirrors the
ROM's `1` placeholder verbatim).

**Note C — SonicBoom vs. Dragon Rage.** These are the only two moves flavor-tagged
`fixed_damage`. Dragon Rage's app `base_power` (`40`) is the *real* fixed damage read out of the
script rather than the ROM struct's meaningless `1`; SonicBoom's app `base_power` (`1`) is the raw
struct placeholder rather than its own real fixed damage (`20`). The two sibling moves are
internally inconsistent with each other; recommend making SonicBoom `20` to match how Dragon Rage
is already handled (more useful for a damage-routing app than a dummy `1`).

---

## 2. `attack_flavor` findings

Verified by grouping all 354 moves by ROM `effect` id (198 distinct ids in use) and checking that
every move sharing an id carries an identical `attack_flavor` set — **100% internally consistent,
zero exceptions**. Then cross-checked every effect id's assigned flavor word against its actual
`BattleScript_Effect*` handler; every one of the ~140 flavor-bearing effect groups maps to a
semantically correct tag (spot list below). Also enumerated *every* `jumpifmove`/`jumpifnotmove
MOVE_*` branch in `pokeemerald/data/battle_scripts_1.s` (9 branch points, covering `MOVE_SURF`,
`MOVE_SKY_ATTACK`, `MOVE_WHIRLPOOL`, `MOVE_STRUGGLE` ×2, `MOVE_HEAL_BELL`, `MOVE_FLY`,
`MOVE_DIVE`, `MOVE_BOUNCE` ×2) to catch move-specific overrides layered inside a shared effect
script, since those are invisible to the per-effect grouping check above.

- **high_crit** — confirmed to be *exactly* `EFFECT_HIGH_CRITICAL | EFFECT_SKY_ATTACK |
  EFFECT_BLAZE_KICK | EFFECT_POISON_TAIL` via `Cmd_critcalc` (`pokeemerald/src/battle_script_commands.c:1253-1268`,
  the four `+= (gBattleMoves[gCurrentMove].effect == EFFECT_*)` terms). App's 11 high_crit moves
  (Karate Chop, Razor Leaf, Crabhammer, Slash, Aeroblast, Cross Chop, Air Cutter, Leaf Blade, Blaze
  Kick, Poison Tail, Sky Attack) match this set exactly — no extras, no omissions.
- **multi_hit** (12), **two_hit**/`EFFECT_DOUBLE_HIT`+`EFFECT_TWINEEDLE` (Double Kick, Bonemerang,
  Twineedle), **always_hit**/`EFFECT_ALWAYS_HIT`+`EFFECT_VITAL_THROW` (Swift, Faint Attack, Shadow
  Punch, Aerial Ace, Magical Leaf, Shock Wave, Vital Throw), **one_hit_ko**/`EFFECT_OHKO`
  (Guillotine, Horn Drill, Fissure, Sheer Cold), **trap**/`EFFECT_TRAP` (Bind, Wrap, Fire Spin,
  Clamp, Whirlpool, Sand Tomb), **recharge**/`EFFECT_RECHARGE` (Hyper Beam, Blast Burn, Hydro
  Cannon, Frenzy Plant), **drain_hp** (Absorb, Mega Drain, Leech Life, Giga Drain, Dream Eater) —
  all confirmed exact matches to their effect groups.
- **Recoil kinds — confirmed via source, not guessed.** `EFFECT_RECOIL` (Take Down, Submission,
  Struggle) uses `MOVE_EFFECT_RECOIL_25`, computed as `gBattleMoveDamage = gHpDealt / 4` at
  `pokeemerald/src/battle_script_commands.c:2636` → **quarter_recoil is correct (1/4)**.
  `EFFECT_DOUBLE_EDGE` (Double-Edge, Volt Tackle) uses `MOVE_EFFECT_RECOIL_33`, computed as
  `gBattleMoveDamage = gHpDealt / 3` at `battle_script_commands.c:2843` → **third_recoil is
  correct (1/3)**. Both app tags match exactly.
- **bonus_dig_damage** (Earthquake `EFFECT_EARTHQUAKE`, Magnitude `EFFECT_MAGNITUDE`),
  **bonus_fly_damage** (Gust `EFFECT_GUST`, Twister `EFFECT_TWISTER`), **bonus_minimize_damage**
  (Stomp, Astonish, Needle Arm, Extrasensory — all `EFFECT_FLINCH_MINIMIZE_HIT`) — all four
  categories confirmed exactly against their `BattleScript_Effect*` handlers (each sets
  `sDMG_MULTIPLIER, 2` conditioned on `STATUS3_UNDERGROUND`/`STATUS3_ON_AIR`/`STATUS3_MINIMIZED`
  respectively). No spurious or missing tags.
- **hits_fly** (Thunder) and **hits_air_semi_invulnerable** (Sky Uppercut) — correctly distinguish
  "can target a Flying-semi-invulnerable Pokémon but no bonus damage" (`orword gHitMarker,
  HITMARKER_IGNORE_ON_AIR` with **no** `sDMG_MULTIPLIER` set) from the bonus-damage moves above.
  Confirmed at `battle_scripts_1.s:1921-1923` (Thunder) and `:2713-2715` (Sky Uppercut).

### Missing flavor (real gap)

- **Surf and Whirlpool both deal double damage to a target that used Dive (`STATUS3_UNDERWATER`),
  and neither is tagged for it — and the app's flavor vocabulary has no tag for this case at all**
  (no `bonus_dive_damage` or equivalent exists anywhere in `moves.json`). Confirmed in script:
  - Surf: `pokeemerald/data/battle_scripts_1.s:236-240` (`BattleScript_EffectHit:: jumpifnotmove
    MOVE_SURF, ... jumpifnostatus3 BS_TARGET, STATUS3_UNDERWATER, ... orword gHitMarker,
    HITMARKER_IGNORE_UNDERWATER / setbyte sDMG_MULTIPLIER, 2`)
  - Whirlpool: `pokeemerald/data/battle_scripts_1.s:830-834` (`BattleScript_EffectTrap::
    jumpifnotmove MOVE_WHIRLPOOL, ...` — identical pattern)

  This exactly parallels the Dig/Fly/Minimize bonus-damage mechanics the app already models, just
  for the third semi-invulnerable state (Dive). Recommend adding a flavor tag (e.g.
  `bonus_dive_damage`) to Surf and Whirlpool.

### Spurious flavor

None found — no move carries a flavor tag its ROM effect doesn't justify.

### Verified-correct edge cases worth noting

- `EFFECT_THAW_HIT` is shared by Flame Wheel (no burn effect, `secondaryEffectChance=0`) and
  Sacred Fire (Burn 50%, `secondaryEffectChance=50`). Their differing `effects` arrays are correct
  — the two moves share the "thaws the user" mechanic (`self_thaw`) but have independently-set
  burn chances in their own struct rows, and the app correctly reflects each move's own value.
- Secret Power's `effects` entry is `{"status": "Secret Power", "chance": 30}` — a symbolic
  placeholder rather than a real status. This is actually correct: `Cmd_getsecretpowereffect`
  (`pokeemerald/src/battle_script_commands.c:9619-9648`) picks one of 8 different secondary
  effects (Poison/Sleep/Acc‑1/Def‑1/Atk‑1/Spd‑1/Confusion/Flinch, default Paralysis) based on
  `gBattleEnvironment` (the map/route terrain), which a static per-move table cannot represent as
  a single fixed status — the placeholder is the right call. `chance=30` matches
  `secondaryEffectChance` in the struct.

---

## 3. Effect-chance findings

Automated diff of all 354 moves' `effects[].chance` values against ROM `secondaryEffectChance`
(script: `diff_effects_chance.py`), correctly handling (a) guaranteed self-buff/debuff moves where
the struct's `secondaryEffectChance` is an unused `0` and the app correctly shows `100`
(Superpower, Overheat, Psycho Boost, Rock Smash-style "_HIT" chance moves excluded — those use
their real nonzero chance; the `0`→`100` pattern applies to certain moves like Bulk Up, Calm Mind,
Dragon Dance, Tail Glow, Howl, Belly Drum, Curse, Memento, Charm, Growth, Amnesia, Cosmic Power,
Tickle, Fake Tears, Metal Sound — all confirmed correct), and (b) Tri-Attack-style N-way chance
splits. Across all 354 moves this flagged **exactly one problem: Tri Attack** (see table above,
6.3 vs the correct 6.6667/6.67).

All of the explicitly-named moves in the task were individually verified against
`secondaryEffectChance` and (for stat-changes) against the effect's own script/kind, all correct:
Sacred Fire 50% burn; Fire Blast 10% burn; Blaze Kick 10% burn; Poison Fang 30% toxic; Poison Tail
10% poison; Crunch/Shadow Ball 20% SpDef‑1; Psychic 10% SpDef‑1; AncientPower/Silver Wind 10% all
five battle stats +1 (`omni`, confirmed via `BattleScript_AllStatsUp` at
`battle_scripts_1.s` raising ATK/DEF/SPEED/SPATK/SPDEF only, no acc/evasion — matches `omni`
semantics); Metal Claw 10%/Meteor Mash 20% Atk+1; Steel Wing 10% Def+1; Superpower 100% Atk‑1 &
Def‑1 self; Overheat/Psycho Boost 100% SpAtk‑2 self; Rock Smash/Crush Claw 50% Def‑1; Luster
Purge 50% SpDef‑1; Mist Ball 50% SpAtk‑1; Octazooka 50% acc‑1; Mud-Slap 100% acc‑1; Muddy Water
30% acc‑1; Iron Tail 30% Def‑1; Rock Tomb/Icy Wind 100% Spe‑1; Bubble/BubbleBeam/Constrict 10%
Spe‑1; Fake Tears/Metal Sound 100% SpDef‑2; Tickle 100% Atk‑1 & Def‑1 (enemy); Cosmic Power/Bulk
Up/Calm Mind/Dragon Dance 100% two-stat self raises; Tail Glow +2 SpAtk; Howl +1 Atk; Belly Drum
sets Attack to its maximum (see below); Curse -1 Spe/+1 Atk/+1 Def self (non-Ghost branch only —
see Curse type note above); Flatter/Swagger both 100% confuse + opposing stat change; Memento 100%
Atk‑2 & SpAtk‑2 enemy; Charm 100% Atk‑2 enemy; Growth 100% SpAtk+1 self; Amnesia 100% SpDef+2 self.
`Charge Beam` correctly does **not** exist in the gen 3 move list (it's a gen 4 move).

**Belly Drum's `modifier: 13`** (only single instance of a modifier outside the normal ±1/±2
range) was checked and is correct by design, not a bug: Belly Drum uses the dedicated
`maxattackhalvehp` script command (`pokeemerald/data/battle_scripts_1.s:1776-1788`,
`BattleScript_EffectBellyDrum`) which *sets* the user's Attack stat stage directly to its maximum
regardless of the starting stage (and fails if HP can't be halved or Attack is already maxed),
rather than applying a normal relative stage change. A modifier of `+13` added to any legal
starting stage (‑6..+6) and then clamped to the legal range always lands exactly on +6, so it is a
valid encoding of "set to max," not an accidental value.

---

## 4. Type chart / special types / held item findings

**(a) Set equality.** Parsed `gTypeEffectiveness` (`pokeemerald/src/battle_main.c:335-451`, 112
real rows after excluding the `TYPE_FORESIGHT` sentinel and `TYPE_ENDTABLE` marker) and compared
against every `(attacker, defender, label)` triple in `type_chart`. **0 entries in ROM but missing
from the JSON; 0 entries in the JSON but not backed by the ROM.** Full set equality confirmed.

Ghost-immunity placement (informational, not a data issue): in the raw table, the `Normal→Ghost`
and `Fighting→Ghost` immunity rows sit **after** the `TYPE_FORESIGHT` sentinel row
(`battle_main.c:449-450`), and `TypeCalc()` (`battle_script_commands.c:1560-1567`) breaks out of
the scan at the sentinel *only* when the defender has been hit by Foresight/Odor Sleuth
(`STATUS2_FORESIGHT`), which is how Foresight negates Ghost's Normal/Fighting immunity. The app's
flat dict representation (`type_chart["Normal"]["Ghost"]="Immune"`,
`type_chart["Fighting"]["Ghost"]="Immune"`) is the correct steady-state (non-Foresight) chart —
there's no JSON-level bug here, just noting where the ROM keeps this special case.

**(b) Per-attacking-type row order (Super-Effective vs. Not-Very-Effective).** Compared ROM row
order against the JSON key insertion order for each of the 17 attacking types, specifically
checking every (SE, NVE) pair's relative order. **0 attacking types have a Super-Effective/
Not-Very-Effective pair whose relative order differs** between ROM and JSON. (One cosmetic,
harmless difference exists: for attacking type Grass, the ROM lists `Dragon` before `Steel`, the
JSON lists `Steel` before `Dragon` — but both are Not-Very-Effective, so this is an NVE/NVE swap,
not an SE/NVE swap, and has no gameplay meaning since dict order doesn't affect lookup.)

**(c) `special_types` vs. id > `TYPE_MYSTERY`(9).** `TYPE_MYSTERY=9` in
`pokeemerald/include/constants/pokemon.h:15`; types with id 10‑17 are Fire, Water, Grass, Electric,
Psychic, Ice, Dragon, Dark. App's `special_types` = `["Water","Grass","Fire","Ice","Electric",
"Psychic","Dragon","Dark"]` — **exact set match** (order differs but the field is used as a set).

**(d) `held_item_boosts` vs. `HOLD_EFFECT_*_POWER`, param 10.** Parsed all items in
`pokeemerald/src/data/items.h` whose `holdEffect` matches `HOLD_EFFECT_*_POWER`
(`include/constants/hold_effects.h:31-64` defines exactly 17 such constants, one per type). Found
**18** matching items total: the 17 type-boost items at `holdEffectParam=10`, plus **Sea Incense**
at `holdEffectParam=5` (`items.h:2703-2710`, `HOLD_EFFECT_WATER_POWER` but half-strength). The
app's `held_item_boosts` contains exactly the 17 full-strength items and correctly **excludes**
Sea Incense — verified correct. All 17 item names in `held_item_boosts` (Black Belt, Blackglasses,
Charcoal, Dragon Fang, Hard Stone, Magnet, Metal Coat, Miracle Seed, Mystic Water, Silk Scarf,
Poison Barb, Sharp Beak, Silverpowder, Soft Sand, Spell Tag, Nevermeltice, Twistedspoon) exist with
matching spelling in `items.json`, and none of the 17 ids are missing. (`pokefirered`/`pokeruby`
don't have a decompiled `src/data/items.h` struct table available in these clones to cross-check
item hold effects against — Ruby/FireRed only expose `constants/items.h` id lists — so this check
used emerald only; item hold-effects are not part of the task's required RS/Emerald/FRLG
move-data diff.)

---

## 5. RS / Emerald / FRLG agreement

Scripted diff of all 354 moves' `power`/`type`/`accuracy`/`pp`/`effect`/`secondaryEffectChance`/
`target`/`priority`/`flags` across all three repos (`diff_numeric.py`, effect compared by numeric
id so the two cosmetic renames noted above don't false-flag). **351/354 moves are byte-identical
across RS, Emerald and FRLG.** Three FRLG-only deviations, all confirmed by source, and the app
follows RS/Emerald's value in all three cases:

| Move | Field | RS | Emerald | FRLG | App | Citations |
|---|---|---|---|---|---|---|
| Kinesis | `flags` (magic coat) | no `F_AFFECTED_BY_MAGIC_COAT` | no `FLAG_MAGIC_COAT_AFFECTED` | **has** `FLAG_MAGIC_COAT_AFFECTED` | `affected_by_magic_coat=False` (matches RS/Emerald) | `pokeruby/src/data/battle_moves.c:1616`, `pokeemerald/src/data/battle_moves.h:1745`, `pokefirered/src/data/battle_moves.h:1745` |
| Snatch | `flags` (mirror move) | `F_MIRROR_MOVE_COMPATIBLE` | `FLAG_MIRROR_MOVE_AFFECTED` | **missing** (flags=0) | `affected_by_mirror_move=True` (matches RS/Emerald) | `pokeruby/src/data/battle_moves.c:3476`, `pokeemerald/src/data/battle_moves.h:3760`, `pokefirered/src/data/battle_moves.h:3760` |
| Nature Power | `accuracy` | 95 | 95 | **0** | `accuracy=95` (matches RS/Emerald) | `pokeruby/src/data/battle_moves.c:3212`, `pokeemerald/src/data/battle_moves.h:3474`, `pokefirered/src/data/battle_moves.h:3474` |

(Nature Power's accuracy field is very likely gameplay-inert either way, since `EFFECT_NATURE_POWER`
dynamically calls another move — Swift, an always-hit move, absent an overworld terrain system in
gen 3 — rather than rolling its own accuracy.)

`gTypeEffectiveness` is **byte-for-byte identical** across all three repos: parsed 112 real rows
(+ sentinel/endtable) from `pokeemerald/src/battle_main.c:335`, `pokefirered/src/battle_main.c:312`,
and `pokeruby/data/type_effectiveness.inc:7`, and a full row-by-row equality check passed with 0
differences.

---

## What was verified identical

Across all 354 gen-3 moves, the app's `base_power`, `type`, `accuracy`, `pp`, `priority`, and
`target` fields matched the pokeemerald ROM reference exactly except for 6 moves: 3 that need a
data fix (Curse's type, Nature Power's power placeholder, SonicBoom's power vs. its sibling Dragon
Rage) and 3 whose raw struct value differs from the app on purpose and were confirmed correct by
reading the damage-calc/accuracy-calc source (Dragon Rage's and Struggle's ROM-placeholder fields,
Vital Throw's forced always-hit) — i.e. 348/354 moves are fully clean on every one of these six
fields with no caveats at all. All 2124 boolean-flag checks
(`contact`/`affected_by_protect`/`affected_by_magic_coat`/`affected_by_snatch`/
`affected_by_mirror_move`/`affeted_by_kings_rock` × 354 moves) matched the ROM's `FLAG_*` bits with
zero exceptions. `attack_flavor` was internally consistent for all 198 ROM effect ids actually used
by the move set and individually verified correct against source for every category the task named
(high_crit, multi_hit, two_hit, always_hit, one_hit_ko, trap, recharge, drain_hp, the three
bonus-damage-vs-semi-invulnerable categories, and both recoil fractions), with the single gap noted
in section 2 (Surf/Whirlpool vs. Dive). `effects` chance values matched `secondaryEffectChance`
(with correctly-modeled "always" and "N-way split" conventions) for 353/354 moves. The full
type-effectiveness table (112 rows), the special-type/id boundary, and the held-item power-boost
item list were all a 100% exact match with no omissions or extras. Finally, RS, Emerald and FRLG
were confirmed to agree on every one of the nine compared move fields for 351 of 354 moves and on
the entire type chart, with the app siding with RS/Emerald on all three of the FRLG-specific
deviations that do exist.
