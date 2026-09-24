# Gen 1 move data verification: `raw_pkmn_data/gen_one/moves.json` vs pokeyellow/pokered decompilation

Method: `moves.json` (165 moves) and `type_info.json` were parsed with Python and
diffed mechanically against `data/moves/moves.asm`, `data/types/type_matchups.asm`,
`constants/type_constants.asm`, `data/battle/critical_hit_moves.asm`, and the move-effect
routines in `engine/battle/effects.asm`, `engine/battle/move_effects/*.asm`, and
`engine/battle/core.asm` (all read from the pokeyellow decomp; pokered was diffed against
pokeyellow file-by-file, see §5). Every one of the 165 moves' `base_power`/`type`/`accuracy`/`pp`
was compared 1:1 (app `rom_id` is exactly the ROM's 1-indexed move id, confirmed by cross-checking
against `constants/move_constants.asm`). Every move's `effects`/`attack_flavor` was then
hand-verified against the effect routine its ROM "effect" byte dispatches to.

## 1. Field mismatches (base_power / type / accuracy / pp)

A Python pass diffed all 660 field comparisons (165 moves × 4 fields) mechanically and found
**17 raw byte-level mismatches** (rows 1-17 below). Hand-verifying the `null`-accuracy convention
against the assembly (as instructed, since `null` isn't a ROM byte value and has to be checked
against what the code actually does) surfaced **one further semantic mismatch that the byte-level
diff could not see**, because the ROM byte it corresponds to happens to equal the value the script
treated as "matches `null`" (row 18, Struggle's accuracy). Of these 18 rows, **7 are real
data-entry bugs** (6 pp + 1 accuracy, across 6 moves — Struggle has both), and the other **11 are
not bugs** — they are moves whose ROM "power"/"accuracy" byte is never actually read by the game
logic (explained per-row). All ROM line numbers are `pokeyellow/data/moves/moves.asm` unless noted.

| # | Move | Field | App value | ROM value | ROM citation | Verdict / suggested fix |
|---|------|-------|-----------|-----------|---------------|--------------------------|
| 1 | Swords Dance | pp | 20 | **30** | `moves.asm:27` — `move SWORDS_DANCE, ATTACK_UP2_EFFECT, 0, NORMAL, 100, 30` | **Bug.** Set pp to 30. |
| 2 | Jump Kick | pp | 10 | **25** | `moves.asm:39` — `move JUMP_KICK, JUMP_KICK_EFFECT, 70, FIGHTING, 95, 25` | **Bug.** Set pp to 25. |
| 3 | Submission | pp | 20 | **25** | `moves.asm:79` — `move SUBMISSION, RECOIL_EFFECT, 80, FIGHTING, 80, 25` | **Bug.** Set pp to 25. |
| 4 | Growth | pp | 20 | **40** | `moves.asm:87` — `move GROWTH, SPECIAL_UP1_EFFECT, 0, NORMAL, 100, 40` | **Bug.** Set pp to 40. |
| 5 | Petal Dance | pp | 10 | **20** | `moves.asm:93` — `move PETAL_DANCE, THRASH_PETAL_DANCE_EFFECT, 70, GRASS, 100, 20` | **Bug.** Set pp to 20. |
| 6 | Struggle | pp | 1 | **10** | `moves.asm:178` — `move STRUGGLE, RECOIL_EFFECT, 50, NORMAL, 100, 10` | **Bug** (as a literal ROM-byte check). Set pp to 10, *or* leave as a deliberate "1" sentinel meaning "not player-selectable/unlimited" — see note below. |
| 7 | Whirlwind | accuracy | `null` | 85 (byte is **never read** — see note) | `moves.asm:31` — `move WHIRLWIND, SWITCH_AND_TELEPORT_EFFECT, 0, NORMAL, 85, 20` | **Not actionable / footnote**, see below. |
| 8 | Guillotine | base_power | `null` | 1 (dead byte) | `moves.asm:25` | Not a bug, see OHKO note below. |
| 9 | Horn Drill | base_power | `null` | 1 (dead byte) | `moves.asm:45` | Not a bug, see OHKO note below. |
| 10 | Fissure | base_power | `null` | 1 (dead byte) | `moves.asm:103` | Not a bug, see OHKO note below. |
| 11 | Counter | base_power | `null` | 1 (dead byte) | `moves.asm:81` | Not a bug, see Counter note below. |
| 12 | SonicBoom | base_power | 20 (correct real value) | 1 (dead byte) | `moves.asm:62` | Not a bug — app stores the real fixed damage; see Special-damage note below. |
| 13 | Dragon Rage | base_power | 40 (correct real value) | 1 (dead byte) | `moves.asm:95` | Not a bug — app stores the real fixed damage; see Special-damage note below. |
| 14 | Seismic Toss | base_power | -1 (sentinel) | 1 (dead byte) | `moves.asm:82` | Not a bug, see Special-damage note below. |
| 15 | Night Shade | base_power | -1 (sentinel) | 0 (dead byte) | `moves.asm:114` | Not a bug, see Special-damage note below. |
| 16 | Psywave | base_power | -1 (sentinel) | 1 (dead byte) | `moves.asm:162` | Not a bug, see Special-damage note below. |
| 17 | Super Fang | base_power | -1 (sentinel) | 1 (dead byte) | `moves.asm:175` | Not a bug, see Super Fang note below. |
| 18 | Struggle | accuracy | `null` ("always hits") | **100 (normal accuracy roll, not a bypass)** — found only by hand-verification, not the byte diff (ROM byte is 100, which the automated pass treats as consistent with `null`) | `moves.asm:178`; dispatch trace: Struggle's effect is `RECOIL_EFFECT`, which is **not** `SWIFT_EFFECT` and is **not** in `ResidualEffects1`/`ResidualEffects2`/`SetDamageEffects` (`data/battle/residual_effects_1.asm`, `residual_effects_2.asm`, `set_damage_effects.asm`), so it runs the ordinary damage pipeline: `PlayerCalcMoveDamage` → `CalculateDamage` → `AdjustDamageForMoveType` → `RandomizeDamage` → `call MoveHitTest` (`engine/battle/core.asm:5410` ff.). `MoveHitTest` only bypasses the roll for `SWIFT_EFFECT` (`core.asm:5431-5432`: `cp SWIFT_EFFECT` / `ret z ; Swift never misses`). Struggle therefore gets the same `CalcHitChance`/255 roll as any other 100-accuracy move (can still miss ~1/256 of the time, and is affected by accuracy/evasion stat stages). | **Bug.** Should be `100`, not `null` — Struggle is mechanically an ordinary 100-accuracy move, unlike Swift. |

**Type field: 0 mismatches** across all 165 moves (every move's `type` matches `moves.asm` exactly,
including the easy-to-get-wrong cases like Karate Chop = Normal, not Fighting).

### Notes on the non-bug rows

- **OHKO moves (Guillotine/Horn Drill/Fissure, rows 8-10):** `OHKO_EFFECT` is handled by a
  special-case in `CalculateDamage` (`core.asm:4499` `cp OHKO_EFFECT` / `core.asm:4638
  JumpToOHKOMoveEffect`) that is reached *before* the generic "`ld a,d ; and a ; ret z`" power-zero
  check, and `OneHitKOEffect_` (`engine/battle/move_effects/one_hit_ko.asm:1-38`) sets `wDamage`
  to 65535 directly by comparing Speed — the table's power byte (1) is never read. OHKO only
  actually hits if the user's Speed ≥ the target's Speed *and* it then passes the normal
  `MoveHitTest` roll at the move's declared 30 accuracy (confirmed: after `OneHitKOEffect_`
  returns non-missed, `CalculateDamage` returns non-zero and `PlayerCalcMoveDamage` falls through
  to `AdjustDamageForMoveType`/`RandomizeDamage`/`MoveHitTest` normally) — Gen 1 OHKO has **no**
  level-difference-based accuracy scaling (that was added in later gens). App's `null` power for
  these is a reasonable representation of "not power-formula-driven," not a bug.
- **Counter (row 11):** effect is plain `NO_ADDITIONAL_EFFECT`; Counter is special-cased by move
  ID in `HandleCounterMove` (`core.asm:4718`), which computes double the damage just taken and
  short-circuits `CalculateDamage`. The table's power=1 is a dead byte. Bonus finding: Counter
  also has a turn-order quirk — using Counter puts you last unless the opponent also used Counter
  (`core.asm:391-400`), separate from Quick Attack's opposite (always-first) priority
  (`core.asm:379-389`, `cp QUICK_ATTACK`) — both correctly captured by the app's `'counter'` /
  `'quick_attack'` flavor tags even though neither is a real `*_EFFECT` accuracy/damage effect.
- **Special-damage moves (SonicBoom/Dragon Rage/Seismic Toss/Night Shade/Psywave, rows 12-16):**
  `SPECIAL_DAMAGE_EFFECT` is listed in `data/battle/set_damage_effects.asm:4-5`
  (`SetDamageEffects`), which makes `PlayerCalcMoveDamage` skip `CriticalHitTest`/
  `GetDamageVarsForPlayerAttack`/`CalculateDamage` entirely (`core.asm` `PlayerCalcMoveDamage`:
  `ld hl, SetDamageEffects / call IsInArray / jp c, .moveHitTest`). Actual damage is computed in
  `ApplyAttackToEnemyPokemon`/`ApplyAttackToPlayerPokemon` (`core.asm:4783-4850`,
  `:4902-4968`): Seismic Toss/Night Shade = user's level (`cp SEISMIC_TOSS`/`cp NIGHT_SHADE`,
  `core.asm:4818-4821`); SonicBoom = fixed 20 (`SONICBOOM_DAMAGE EQU 20`,
  `constants/battle_constants.asm:59`, used at `core.asm:4822`); Dragon Rage = fixed 40
  (`DRAGON_RAGE_DAMAGE EQU 40`, `battle_constants.asm:60`, used at `core.asm:4825`); Psywave =
  random in `[1, floor(level*1.5))` for the player / `[0, floor(level*1.5))` for the enemy
  (`core.asm:4833-4845`). The table's power byte (1 for Sonic Boom/Seismic Toss/Dragon
  Rage/Psywave, 0 for Night Shade — itself an inconsistency, but it's ROM's, not the app's) is
  never read. SonicBoom=20 and Dragon Rage=40 in the app are the **correct real fixed-damage
  values**; Seismic Toss/Night Shade/Psywave use a `-1` sentinel since they aren't fixed numbers.
  Consistency nit: OHKO/Counter use `null` as their "not a normal power move" sentinel while
  Seismic Toss/Night Shade/Psywave/Super Fang use `-1` — functionally fine (all four groups are
  genuinely different kinds of "non-formula" damage) but worth picking one convention.
- **Super Fang:** `SUPER_FANG_EFFECT` is also in `SetDamageEffects`; damage = target's current HP
  / 2, minimum 1 (`core.asm:4795-4810` `.superFangEffect`). Table power byte (1) unused. App's
  `-1` is reasonable.
- **Whirlwind (row 7):** `SWITCH_AND_TELEPORT_EFFECT` is listed in `data/battle/residual_effects_1.asm`,
  so `PlayerCanExecuteMove` jumps straight to the effect handler and **never calls `MoveHitTest`
  at all** for Whirlwind/Roar/Teleport. `SwitchAndTeleportEffect` (`engine/battle/effects.asm:846-947`)
  implements its own logic: in a **trainer battle** it always fails outright (forced switch is
  disallowed); in a **wild battle** success is a level-comparison RNG check (always succeeds if
  the user's level ≥ the wild mon's level, otherwise `rand[0, userLvl+wildLvl] ≥ wildLvl/4`) — the
  move's declared "accuracy" byte is **never consulted** by any of this. That byte is 85 for
  Whirlwind but 100 for Roar (`moves.asm:59`) and Teleport (`moves.asm:113`) — three ROM-vestigial
  values that happen to differ from each other despite being equally unused. The app stores `null`
  for all three, which is functionally correct (none of them use the accuracy stat), so there is
  nothing to "fix," but per the task's instruction to check `null` against the ROM's stored value:
  Whirlwind's stored byte is 85, not 100 like its effect-siblings.

## 2. Flavor (`attack_flavor`) omissions / spurious tags

**Result: none found that are clearly wrong.** Every flavor tag in the vocabulary was checked by
counting how many moves use the underlying ROM effect constant and confirming an exact match:

| Flavor | Moves tagged | ROM effect / table | Count match |
|---|---|---|---|
| `high_crit` | Karate Chop, Razor Leaf, Crabhammer, Slash | `HighCriticalMoves` (`data/battle/critical_hit_moves.asm:1-6`) | 4/4 exact |
| `multi_hit` | Doubleslap, Comet Punch, Fury Attack, Pin Missile, Spike Cannon, Barrage, Fury Swipes | `TWO_TO_FIVE_ATTACKS_EFFECT` | 7/7 exact |
| `two_hit` | Double Kick, Bonemerang, Twineedle | `ATTACK_TWICE_EFFECT` (×2) + `TWINEEDLE_EFFECT` (×1) | 3/3 exact |
| `one_hit_ko` | Guillotine, Horn Drill, Fissure | `OHKO_EFFECT` | 3/3 exact |
| `partial_trapping` | Bind, Wrap, Fire Spin, Clamp | `TRAPPING_EFFECT` | 4/4 exact |
| `quarter_recoil` | Take Down, Double-Edge, Submission | `RECOIL_EFFECT`, non-Struggle (`move_effects/recoil.asm:17-20`) | 3/3 exact |
| `half_recoil` | Struggle | `RECOIL_EFFECT`, Struggle-only 50% path (`move_effects/recoil.asm:18-19`) | 1/1 exact |
| `drain_hp` | Absorb, Mega Drain, Leech Life | `DRAIN_HP_EFFECT` | 3/3 exact, see Dream Eater note |
| `recharge` | Hyper Beam | `HYPER_BEAM_EFFECT` | 1/1 exact |
| `self_destruct_explosion` | Selfdestruct, Explosion | `EXPLODE_EFFECT` | 2/2 exact |
| `miss_crash` | Jump Kick, Hi Jump Kick | `JUMP_KICK_EFFECT` | 2/2 exact |
| `self_confuse_multi_hit` | Thrash, Petal Dance | `THRASH_PETAL_DANCE_EFFECT` | 2/2 exact |
| `two_turn` | Razor Wind, Solar Beam, Skull Bash, Sky Attack | `CHARGE_EFFECT` minus Dig | 4/4 exact |
| `two_turn_semi_invulnerable` | Fly, **Dig** | `FLY_EFFECT` + `CHARGE_EFFECT`-but-hardcoded-invulnerable-for-`DIG` | 2/2 exact — see note |
| `fixed_damage` | SonicBoom, Dragon Rage | `SPECIAL_DAMAGE_EFFECT` subset | 2/5 exact |
| `level_damage` | Seismic Toss, Night Shade | `SPECIAL_DAMAGE_EFFECT` subset | 2/5 exact |
| `psywave` | Psywave | `SPECIAL_DAMAGE_EFFECT` subset | 1/5 exact |
| `super_fang` | Super Fang | `SUPER_FANG_EFFECT` | 1/1 exact |
| `one_hit_ko`/`scatter_coins`/`bide`/`focus_energy`/`rage`/`counter` | Guillotine&al / Pay Day / Bide / Focus Energy / Rage / Counter | matching unique `*_EFFECT` | all exact |

**Noteworthy correct detail:** Dig is tagged `two_turn_semi_invulnerable` even though its ROM
effect is plain `CHARGE_EFFECT` (same as Razor Wind/Solar Beam/Skull Bash/Sky Attack, which are
correctly plain `two_turn`). This is right: `ChargeEffect` (`engine/battle/effects.asm:1037-1086`)
special-cases the move **by ID**, not just by effect — `cp FLY_EFFECT` sets `INVULNERABLE`
(`effects.asm:1051-1053`), and independently `cp DIG` (checking the move number, not the effect)
also sets `INVULNERABLE` (`effects.asm:1056-1059`). So Dig really is semi-invulnerable like Fly,
and the app has this exactly right.

Two minor observations, not clear bugs:
- **Dream Eater** is tagged `dream_eater`, not `drain_hp`, even though `DREAM_EATER_EFFECT`
  dispatches to the *same* `DrainHPEffect_` routine as Absorb/Mega Drain/Leech Life
  (`data/moves/effects_pointers.asm:8` — `dw DrainHPEffect ; DREAM_EATER_EFFECT`; and
  `move_effects/drain_hp.asm` halves `wDamage` and adds it to the user's HP identically for all
  four moves). This matches the app's apparent one-tag-per-`*_EFFECT`-constant convention (every
  unique effect ID gets its own bespoke tag: `mimic`, `transform`, `substitute`, `conversion`,
  `metronome`, `mirror_move`, `haze`, `bide`, `rage`, `focus_energy`, `recover`/`rest`, etc.), so
  it is very likely intentional, but flagging since the underlying HP-drain formula is identical.
- **Swift** (`SWIFT_EFFECT`) has `attack_flavor: []` — no tag at all — unlike every other unique
  effect, which gets a bespoke tag under the convention above. Its "always hits" behaviour is
  fully captured by `accuracy: null`, so nothing is functionally lost, but it stands out as the
  only *_EFFECT move with neither an `effects` entry nor an `attack_flavor` entry.
- `Splash` (`SPLASH_EFFECT`) also has both empty, but correctly so — `SplashEffect_`
  (`effects.asm:1338-1340`) truly does nothing but play an animation and print "but nothing
  happened," so there is nothing to encode.

## 3. Effect-chance findings (`effects[]`)

All side-effect chances were traced to their exact ROM formula (RGBDS `percent` macro =
`* $FF / 100`, defined at `macros/data.asm:3`) and cross-checked against every move using that
effect constant. **Every numeric chance matches**, with one exception:

| ROM effect constant | Formula (`engine/battle/effects.asm` unless noted) | Probability | App moves using it | App value |
|---|---|---|---|---|
| `*_SIDE_EFFECT1` (burn/freeze/paralyze) | `ld b, 10 percent + 1` (`:235`, mirrored `:300`) | 26/256 = 10.16% | Fire Punch, Ice Punch, ThunderPunch, Ember, Flamethrower, Ice Beam, Blizzard, Thundershock, Thunderbolt, Thunder | 10 (all) — matches |
| `*_SIDE_EFFECT2` (burn/paralyze; freeze2 unused) | `ld b, 30 percent + 1` (`:238`, mirrored `:303`) | 77/256 = 30.08% | Fire Blast, Body Slam, Lick | 30 (all) — matches |
| `FLINCH_SIDE_EFFECT1` | `ld b, 10 percent + 1` (`:1023`) | 26/256 = 10.16% | Bite, Bone Club, Hyper Fang | 10 (all) — matches |
| `FLINCH_SIDE_EFFECT2` | `ld b, 30 percent + 1` (`:1025`) | 77/256 = 30.08% | Stomp, Rolling Kick, Headbutt, Low Kick | 30 (all) — matches |
| `POISON_SIDE_EFFECT1` | `ld b, 20 percent + 1` (`:111`) | 52/256 = 20.31% | Poison Sting, Twineedle (2nd-hit poison, set by `TwoToFiveAttacksEffect` `:1002-1005`) | 20 (both) — matches |
| `POISON_SIDE_EFFECT2` | `ld b, 40 percent + 1` (`:114`) | 103/256 = 40.23% | Smog, Sludge | 40 (both) — matches |
| `CONFUSION_SIDE_EFFECT` | `cp 10 percent` — **no `+1`** (`:1172`) | 25/256 = 9.77% | Psybeam, Confusion (the move) | 10 (both) — reasonable rounding, not a bug, but note this is a slightly different ROM formula (no `+1`) from the nominally-identical-looking "10%" `*_SIDE_EFFECT1`/`FLINCH_SIDE_EFFECT1` above |
| `ATTACK_DOWN_SIDE_EFFECT` / `DEFENSE_DOWN_SIDE_EFFECT` / `SPEED_DOWN_SIDE_EFFECT` / `SPECIAL_DOWN_SIDE_EFFECT` | `cp 33 percent + 1 ; chance for side effects` (`StatModifierDownEffect`, `effects.asm:598`) | 85/256 = 33.20% | Aurora Beam (atk), Acid (def), BubbleBeam/Bubble/Constrict (spe), **Psychic (spa+spd)** | Aurora Beam/Acid/BubbleBeam/Bubble/Constrict = **33.2 (correct)**; **Psychic = 33 (WRONG, should be 33.2)** |
| non-side stat-down (`ATTACK_DOWN1/2`, `DEFENSE_DOWN1/2`, `SPEED_DOWN1`, `ACCURACY_DOWN1`, `EVASION_DOWN1`) | no RNG roll of its own — gated only by the move's own `accuracy` via `MoveHitTest` (`StatModifierDownEffect` `:603-613`), then always applies | 100% "if it hits" | Growl, Leer, Tail Whip, String Shot, Sand Attack, Screech, Smokescreen, Kinesis, Flash | 100 (all) — matches |
| self stat-up (`ATTACK_UP1/2`, `DEFENSE_UP1/2`, `SPEED_UP2`, `SPECIAL_UP1/2`, `EVASION_UP1`) | `StatModifierUpEffect` (`:387-419`) has **no RNG roll at all** and isn't even routed through `MoveHitTest` (`ATTACK_UP1_EFFECT` etc. are in `ResidualEffects2`, `data/battle/residual_effects_2.asm`) | always | Swords Dance, Meditate, Sharpen, Harden, Withdraw, Defense Curl, Barrier, Acid Armor, Agility, Growth, Amnesia, Double Team, Minimize | 100 (all) — matches |
| `SLEEP_EFFECT` / `CONFUSION_EFFECT` (pure status moves) / `POISON_EFFECT` / `PARALYZE_EFFECT` / `LEECH_SEED_EFFECT` / `DISABLE_EFFECT` | each routine calls `MoveHitTest` once itself, then always applies the status | 100% "if it hits" | Sing/Sleep Powder/Hypnosis/Lovely Kiss/Spore (sleep), Supersonic/Confuse Ray (confusion), PoisonPowder/Poison Gas/Toxic (poison), Stun Spore/Thunder Wave/Glare (paralysis), Leech Seed, Disable | 100 (all) — matches, and correctly distinct from the ~10% `CONFUSION_SIDE_EFFECT` used by Psybeam/Confusion(move) |

**Gen-1-single-Special-stat representation:** `SPECIAL_UP1_EFFECT`/`SPECIAL_UP2_EFFECT`/
`SPECIAL_DOWN_SIDE_EFFECT` all index the *same single* "Special" stat-mod slot in
`wPlayerMonStatMods`/`wEnemyMonStatMods` (Gen 1 has only 6 stat-mod slots: attack, defense,
speed, **special**, accuracy, evasion — confirmed by `StatModifierUpEffect`/`StatModifierDownEffect`
in `effects.asm`, which compute the array offset as `effect_id - ATTACK_UP1_EFFECT`). Growth
(`+1` self), Amnesia (`+2` self), and Psychic (`-1` enemy) each represent this **one** ROM stat
change as **two** JSON `effects[]` entries (`stat: 'spa'` and `stat: 'spd'`, same chance/modifier/
target on both) rather than a single `stat: 'omni'` entry (the schema documents `omni` as an
available key, but it is never used anywhere in `moves.json`). Both entries always carry the same
chance/magnitude (i.e. it isn't modelled as two independent 33.2% rolls for Psychic, which would
misrepresent the real probability), so this looks like a deliberate, consistent modelling choice
for mapping Gen 1's unified Special stat onto a schema built for Gen 2+'s split stats — flagging
for confirmation, not as a bug, except for the one real numeric error below.

**Confirmed bug:** Psychic's two `effects[]` entries use `"chance": 33` where every other move
driven by the same `StatModifierDownEffect` side-effect roll (Acid, Aurora Beam, BubbleBeam,
Bubble, Constrict) uses `"chance": 33.2`. `StatModifierDownEffect` (`effects.asm:591-599`) applies
identically regardless of which stat is being lowered, so Psychic's real chance is also 85/256 =
33.2%, not 33. **Fix: change both of Psychic's `effects[]` entries from `"chance": 33` to
`"chance": 33.2`.**

**Key-name inconsistency (as requested):** 4 of 74 `effects[]` entries use the key `"percent"`
instead of `"chance"` — all four are otherwise-correct `*_SIDE_EFFECT1`/`2` entries:

| Move | Entry |
|---|---|
| Fire Punch | `{"percent": 10, "status": "Burn"}` |
| Ice Punch | `{"percent": 10, "status": "Freeze"}` |
| ThunderPunch | `{"percent": 10, "status": "Paralysis"}` |
| Body Slam | `{"percent": 30, "status": "Paralysis"}` |

Numerically correct, but inconsistent with the other 70 entries which all use `"chance"`. Should
be renamed to `"chance"` for schema consistency (routing/simulation code that reads `e["chance"]`
would silently treat these four as 0%/absent otherwise).

## 4. Type chart findings

- **Exact match.** Parsed all 82 rows of `TypeEffects` (`data/types/type_matchups.asm:1-83`,
  terminated by `db -1`) and compared as a set of (attacker, defender, effectiveness) triples
  against `type_info.json`'s `type_chart`: **0 entries present in ROM but missing/wrong in the
  app, and 0 entries present in the app with no matching ROM row.** All 82 ROM rows ⟷ all 82 app
  entries (summed across attacking types) correspond 1:1.
- **`special_types`: exact set match.** `constants/type_constants.asm:19-26` defines
  `SPECIAL EQU const_value` immediately before `FIRE`, i.e. every type from `FIRE` ($14) through
  `DRAGON` ($1A) has id ≥ `SPECIAL`: {Fire, Water, Grass, Electric, Psychic, Ice, Dragon} — exactly
  the app's `special_types` list as a set. (List order differs — ROM/const order is Fire, Water,
  Grass, Electric, Psychic, Ice, Dragon; app order is Water, Grass, Fire, Ice, Electric, Psychic,
  Dragon — immaterial unless the app's consumer treats the list as ordered.)
- **ROM row order per attacking type** (preserves file order within each attacker; relevant to
  dual-type damage rounding, since the engine applies each matching multiplier sequentially and
  truncates after each division — see `AdjustDamageForMoveType`, `core.asm:5246-5343`):

  - Water: Fire(S), Rock(S), Water(¬), Grass(¬), Ground(S), Dragon(¬)
  - Fire: Grass(S), Ice(S), Fire(¬), Water(¬), Bug(S), Rock(¬), Dragon(¬)
  - Grass: Water(S), Grass(¬), Fire(¬), Ground(S), Bug(¬), Poison(¬), Rock(S), Flying(¬), Dragon(¬)
  - Electric: Water(S), Electric(¬), Grass(¬), Ground(immune), Flying(S), Dragon(¬)
  - Ground: Flying(immune), Fire(S), Electric(S), Grass(¬), Bug(¬), Rock(S), Poison(S)
  - Ice: Ice(¬), Water(¬), Grass(S), Ground(S), Flying(S), Dragon(S)
  - Psychic: Psychic(¬), Fighting(S), Poison(S)
  - Normal: Rock(¬), Ghost(immune)
  - Ghost: Ghost(S), Normal(immune), Psychic(immune)
  - Fighting: Normal(S), Poison(¬), Flying(¬), Psychic(¬), Bug(¬), Rock(S), Ice(S), Ghost(immune)
  - Poison: Grass(S), Poison(¬), Ground(¬), Bug(S), Rock(¬), Ghost(¬)
  - Flying: Electric(¬), Fighting(S), Bug(S), Grass(S), Rock(¬)
  - Bug: Fire(¬), Grass(S), Fighting(¬), Flying(¬), Psychic(S), Ghost(¬), Poison(S)
  - Rock: Fire(S), Fighting(¬), Ground(¬), Flying(S), Bug(S), Ice(S)
  - Dragon: Dragon(S)

  (S = Super Effective, ¬ = Not Very Effective, immune = Immune)
- **Yellow-only AI quirk found in passing (not a `type_chart` bug):** `AIGetTypeEffectiveness`
  (`core.asm:5363-5409`), used only by the trainer AI to help pick a move, has a Yellow-exclusive
  special case not present in pokered: `cp LORELEI` / `cp DEWGONG` / `call BattleRandom` / `cp $66`
  (`core.asm:5394-5403`) — Lorelei's Dewgong has a 40% chance for the AI to miscalculate type
  effectiveness when choosing a move. This never touches the real damage-calculation type chart
  (`AdjustDamageForMoveType`/`TypeEffects`, which are identical to pokered), so it does not affect
  `type_chart` verification — noted only because it showed up in the pokeyellow/pokered diff.

## 5. pokered vs pokeyellow diff (identity check)

Byte-identical (`diff` exit code 0): `data/moves/moves.asm`, `data/moves/effects_pointers.asm`,
`constants/move_effect_constants.asm`, `constants/move_constants.asm`, `constants/type_constants.asm`,
`constants/battle_constants.asm` (aside from two Yellow-only unused constants,
`BATTLE_TYPE_RUN`/`BATTLE_TYPE_PIKACHU`, appended after the shared ones — `SONICBOOM_DAMAGE`/
`DRAGON_RAGE_DAMAGE` unaffected), `data/types/type_matchups.asm`, `data/battle/critical_hit_moves.asm`,
`data/battle/unused_critical_hit_moves.asm`, `data/battle/always_happen_effects.asm`,
`data/battle/special_effects.asm`, `data/battle/set_damage_effects.asm`,
`data/battle/residual_effects_1.asm`, `data/battle/residual_effects_2.asm`, and all of
`engine/battle/move_effects/*.asm` except `transform.asm` (see below).

Differences found, all confirmed cosmetic/unrelated to move data:
- `engine/battle/effects.asm`: pokeyellow adds `wUnknownSerialFlag_d499`-gated "Stadium" link-
  compatibility branches inside `SleepEffect` and `FreezeBurnParalyzeEffect` (dead code in normal
  play — they only matter over a Stadium-compatible link cable, and the `FREEZE_SIDE_EFFECT2`
  branch they gate is unused by any move in either game); extra `SHAKE_SCREEN_ANIM` animation
  calls on the opponent-side burn/freeze/paralyze branches; an extra `ClearHyperBeam` call in
  `FlinchSideEffect` for link battles; extra Substitute-hide/show animation bracketing in
  `ChargeEffect`; and one extra `callfar Func_78e98` in `PlayBattleAnimationGotID`. None of these
  touch power/type/accuracy/pp or any effect chance/target/magnitude.
- `engine/battle/move_effects/transform.asm`: the loop that zero-fills PP for a Transformed mon's
  blank move slots is restructured (single combined loop in Yellow vs. two loops in Red), but both
  versions zero PP for blank slots and set 5 PP for real ones — not a behavioral difference, and
  irrelevant to any `moves.json` field.
- `engine/battle/core.asm`: 642 diff lines, but every one is UI/menu/animation/VC-compatibility
  code (old-man/Prof. Oak tutorial battles, debug "TestBattle" menu, Pikachu happiness/cry/
  animation hooks, `_YELLOW_VC` vs `_RED_VC`/`_BLUE_VC` build flags, a PP-display bitmask fix, and
  the Lorelei/Dewgong AI quirk noted in §4). `CalculateDamage`, `CriticalHitTest`,
  `GetDamageVarsForPlayerAttack`/`GetDamageVarsForEnemyAttack`, `ApplyAttackToEnemyPokemon`/
  `ApplyAttackToPlayerPokemon`, `AdjustDamageForMoveType`, and `MoveHitTest`/`CalcHitChance` — the
  routines this report's move-data/effect-chance findings rely on — are **byte-identical** between
  the two games (no diff hunks fall inside those routines' line ranges).
- `macros/data.asm`: pokeyellow adds an unrelated `sine_table` macro at the end; the `percent`
  macro itself (`* $FF / 100`) is identical in both.

## What was verified identical

Across all 165 moves, **642 of 660** base-field comparisons (`base_power`×165, `type`×165,
`accuracy`×165, `pp`×165) match the ROM's `moves.asm` table exactly and are behaviorally correct.
Of the 18 that don't: 11 are ROM-dead-byte/vestigial-value cases that are not bugs (10
`base_power`, 1 `accuracy` — Whirlwind), and 7 are real bugs across 6 moves (6 `pp` bugs, plus
Struggle's `accuracy`, which is also wrong but only detectable by reading the assembly rather than
comparing bytes — see §1 row 18). `type` has 0 mismatches across all 165 moves.
Every one of the 36 distinct `attack_flavor` tags used in the file was checked against the count
of ROM moves sharing its underlying effect constant and matched exactly (including the subtle
Dig-is-secretly-semi-invulnerable and Counter-reverses-turn-order cases). All 74 `effects[]`
entries' status/stat/direction/target were verified correct, and all-but-one (Psychic) of their
numeric chances exactly match the ROM's `percent`-macro-derived probability family (10/20/30/33.2/
40/100). The `type_chart` (82 rows) and `special_types` (7 types) are both exact matches to the
ROM as sets. `moves.asm`, `type_matchups.asm`, `critical_hit_moves.asm`, `effects_pointers.asm`,
`move_effect_constants.asm`, `type_constants.asm`, and every move-effect data table/routine cited
in this report are identical (or behaviorally identical) between pokered and pokeyellow.
