# Damage calculation review — every move, every game (gens 1–5)

Review date: 2026-09-10. Branch `ui` at commit `591dc60` (working tree with uncommitted UI changes; none of them
touch the calc).

## What this is

A line-by-line audit of the app's damage calculation for every damaging move in every supported game, verified
against the game decompilations listed in `CLAUDE.md`:

| Gen | Games | Verified against | Detail file |
|---|---|---|---|
| 1 | Red, Blue, Yellow | pokeyellow + pokered (diffed) | [gen_1_findings.md](gen_1_findings.md) |
| 2 | Gold, Silver, Crystal | pokecrystal + pokegold (diffed) | [gen_2_findings.md](gen_2_findings.md) |
| 3 | Ruby, Sapphire, Emerald, FireRed, LeafGreen | pokeemerald + pokefirered + pokeruby | [gen_3_findings.md](gen_3_findings.md) |
| 4 | Diamond, Pearl, Platinum, HeartGold, SoulSilver | pokeplatinum + pokeheartgold (+ DP move table) | [gen_4_findings.md](gen_4_findings.md) |
| 5 | Black, White, Black 2, White 2 | **no decomp available — gap inventory only** | [gen_5_findings.md](gen_5_findings.md), [gen_5_move_table.md](gen_5_move_table.md) |

Each detail file follows the same layout: sources consulted (decomp file + line ranges), core-formula verdict
table, a per-move table covering every damaging move in that gen's `moves.json`, ranked bugs with root cause,
exact fix and a hand-computed numeric test case, and a not-implemented list with the game's formula and an
implementation plan. `reference/` holds the transcribed reference calculators and probe scripts that produced
the "game" numbers (`gen1_ref_harness.py`, `gen2_ref.py`, `ref_gen3.py`, `app_probe.py`, ...); they are the
starting point for regression tests.

Method: every claim about the game was read from the asm/C, not from wikis; every claim about the app was
checked by reading the code and, wherever a number is quoted, by running `gen.calculate_damage(...)` through the
real gen objects (`py -3.14`; the `python` on PATH lacks the project deps). Baseline: `py -3.14 -m pytest
tests/test_unit_calcs.py` → 81 passed.

Scope note: the app deliberately does not simulate HP, status, turn order or previous turns; those are exposed as
per-move dropdowns (`custom_move_data`). That design is accepted here. "Not implemented" means the game's result
cannot be expressed with the inputs the app has today.

---

## 1. Executive summary

**The core formula is right in every gen** for a plain move with no modifiers: base damage and its truncation
order, `+2`, crit ×2, STAB, type chart contents, random roll (217–255/255 in gens 1–2, 85–100 in gens 3–4),
min damage 1, Hidden Power (gen 3/4), stat/stage/nature/badge math, fixed-damage moves (SonicBoom, Dragon Rage,
Seismic Toss, Night Shade). Move data (power/type/accuracy) matches the games for every move in gens 1–4 with a
handful of exceptions listed below.

**Where it goes wrong** is (a) the edge steps of each gen's formula (8-bit stat truncation in gens 1–2, the crit /
screen / stage interactions, the order of stat modifiers vs stages in gen 3/4), (b) the variable-power moves
(Rollout, Fury Cutter, Triple Kick, Rage, Gyro Ball, Punishment, Crush Grip, Trump Card, Spit Up, Nature Power,
Frustration, Present, Beat Up, Low Kick...), (c) a set of data/plumbing mistakes that silently disable whole
mechanisms (Flash Fire spelled "Water Absorb", plate/item tables, target vocabulary, gen 5 move data with no
flavor lists), and (d) the shared multi-hit combiner.

Counts (bug = a documented mismatch with the game; "not implemented" = no way to get the game's number):

| Gen | Bugs | Not implemented (moves / mechanics) | Formula core |
|---|---|---|---|
| 1 | 7 | OHKO ×3, Counter, Bide, Super Fang, trapping multi-turn, Focus Energy, exact accuracy, burn/para | correct except `>255` stat scaling + type-order rounding |
| 2 | 18 | Present, Frustration, Beat Up, Super Fang, OHKO ×3, Counter/Mirror Coat/Bide, crit items, accuracy stages | correct except `TruncateHL_BC` + crit/screen rule |
| 3 | 26 | Nature Power, Frustration, Low Kick (needs weights), Present, Beat Up, Endeavor, Super Fang, OHKO ×4, Counter/Mirror Coat/Bide, 12 abilities/field effects | correct; modifier-before-stage ordering wrong |
| 4 | 28 | Present, Beat Up, Fling, Nature Power (gen 4 table), Triple Kick, OHKO ×4, Counter/Mirror Coat/Metal Burst/Bide, Endeavor, Super Fang, ~20 abilities, ~12 items, accuracy stages | correct; crit-stage block inverted |
| 5 | gen 4 copy + 6 structural breaks | 54 moves produce no number / need work, 40 more silently lose crit or multi-hit handling, 13 use a gen 2–4 rule that gen 5 changed | gen 4 chain; gen 5 modifier order not implemented |

### The ten highest-impact fixes (in order)

1. **Gen 4/5 crit stat-stage block is inverted** — physical crits test the Sp.Atk/Sp.Def stages and vice versa,
   and the reset wipes every stage. A crit vs a +2 Def target deals half what the game deals. (`gen_4/pkmn_damage_calc.py:351-361`; gen 4 bug #1)
2. **Gen 1 and gen 2 8-bit stat truncation is missing** — when attack or (screen-doubled) defence ≥ 256 the game
   divides *both* by 4 (gen 1 keeps only the low byte, so Reflect on a 512+ Defense wraps). Commented out in gen 1,
   absent in gen 2. Affects every late-game fight and every screen on a 128+ Def mon. (gen 1 bug 1, gen 2 bug 3.1)
3. **Flash Fire constant is the string "Water Absorb"** (gens 3/4/5) — Fire moves vs Poliwag/Wooper/Lapras-type
   mons read as no damage; real Flash Fire mons take damage. (`gen_three_constants.py:69`, `gen_four_constants.py:115`, gen 5 copy)
4. **Rollout / Ice Ball double on turn 1** (gens 3/4/5, `2**n` instead of `2**(n-1)`) and **Fury Cutter is uncapped**
   in every gen (dropdown "6" = ×32; cap is 160 power). (gen 3 bug 1, gen 4 bug 2/3, gen 2 bug 3.5)
5. **Nature Power never deals damage** in gens 3/4/5 — json power 0/null hits the early-out before the terrain
   branch; the terrain table is the gen 3 one even in gen 4/5. (gen 3 bug 2, gen 4 bug 23)
6. **Gen 1 applies type effectiveness in type-1/type-2 order** instead of ROM table order — off by one pre-roll on
   odd damage for 21 real matchups (Grass vs Nido-royals, Fire vs Cloyster/Lapras/Dewgong, Fighting vs
   Jynx/Aerodactyl/Articuno, Poison vs Weedle line, Bug vs Zubat...). (gen 1 bug 2)
7. **Gen 2 Hidden Power treats DV 8 as a low bit** (`> 8` instead of `>= 8`): trainer-style 8/8/8/8 DVs give
   power 31 instead of 68. Also gen 2 Future Sight wrongly gets type effectiveness, and crits drop Reflect/Light
   Screen when the game keeps them. (gen 2 bugs 3.4, 3.3, 3.2)
8. **Gen 4/5 variable-power moves**: Technician hardcodes power 90; Gyro Ball ratio inverted; Crush Grip/Wring Out
   power in the thousands; Punishment starts from 1 not 60; Trump Card "4+" unmatched; Spit Up power 1×n; Rage
   multiplies damage (gen 3+ Rage is an Attack-stage effect); Light Ball misses the Attack/power boost. (gen 4 bugs 4–7, 12)
9. **Gen 5 move data has no `attack_flavor`** → 18 high-crit moves get 1/16, 19 multi-hit/two-hit moves show one
   hit, and null-power moves (Low Kick, Flail, Return, Gyro Ball, Natural Gift, Trump Card...) return no damage.
   Plus name mismatches ("Self-Destruct", "Solar Beam", "Smelling Salts") disable Damp / weather / dropdowns. (gen 5 §1)
10. **`DamageRange.add` adds roll counts instead of multiplying them** — every multi-hit kill percentage in every
    gen is computed over a distorted distribution (min/max are fine). (`pkmn/damage_calc.py:153-159`)

---

## 2. Cross-cutting findings (shared code, data plumbing, UI wiring)

These were verified by running the app and apply to more than one gen. Per-gen numeric test cases are in the
detail files.

| # | Finding | Where | Fix |
|---|---|---|---|
| X1 | `DamageRange.add` combines two hit distributions with `count_a + count_b`; the joint distribution needs `count_a * count_b` (a={10:1,11:2} → a+a gives {20:2,21:6,22:4} size 12; correct {20:1,21:4,22:4} size 9). Every multi-hit kill % in every gen, and `find_kill`'s normalisation, are off. | `pkmn/damage_calc.py:141-155` | `result[total] += my_count * your_count` |
| X2 | Double-battle spread reduction keyed on `move.targeting == "target_both_enemies"`. Gen 3 json uses that string only for the 22 `MOVE_TARGET_BOTH` moves — **which is correct for gen 3** (Earthquake/Explosion/Magnitude are FOES_AND_ALLY and are *not* halved in gen 3). Gen 4/5 json uses "All Foes"/"Others", so the branch never fires; gen 4 factor is ×3/4, not ÷2, and applies to both vocabularies; gen 5 is ×0.75. Reflect/Light Screen in doubles are ×2/3 in gens 3–5, not ÷2. | gen 3 ~330, gen 4/5 ~615 | normalise targeting in `_load_move_db` per gen; gen 4/5: `temp = temp*3//4` for "All Foes"/"Others"; screens ×2/3 in doubles |
| X3 | `FLASH_FIRE_ABILITY = "Water Absorb"` in gens 3/4/5 → Water Absorb mons immune to Fire, Flash Fire mons not. | `gen_three_constants.py:69`, `gen_four_constants.py:115`, gen 5 | `"Flash Fire"` |
| X4 | Lightning Rod treated as Electric immunity in gens 3/4 (it only redirects in doubles; gen 5 is when it became an absorb). | gen 3 ~153, gen 4 ~290 | remove from the immunity list in gens 3/4 |
| X5 | Held-item boost table keys vs item names: gen 3 `"SilverPowder"`/`"TwistedSpoon"` never match items.json `"Silverpowder"`/`"Twistedspoon"` (Bug/Psychic ×1.1 never applies). Gen 3 table lists **Dragon Scale** (no boost) and lacks **Dragon Fang** and **Sea Incense** (×1.05!). Gen 4/5 table lists Dragon Scale (no boost in gen 4) and lacks Dragon Fang, Odd/Rock/Rose/Sea/Wave Incense (×1.2). | `raw_pkmn_data/gen_{three,four,five}/type_info.json` | fix keys, remove Dragon Scale, add Dragon Fang/incenses; make `_validate_held_item_boosts` check keys exist in the item db |
| X6 | `PLATE_TYPE_LOOKUP["Earth Plate"] = Dark` (gens 4/5) — Judgment and Multitype Arceus mis-typed. | `gen_four_constants.py:78`, gen 5 | `TYPE_GROUND` |
| X7 | `get_move_accuracy` Nature Power branch sets `result` per terrain then unconditionally overwrites with `100`. Also Blizzard-in-hail bypass is wrongly applied in gen 3; Thunder in sun should be 50% in gens 2/3/4/5; Compound Eyes clamped before Sand Veil (gen 3). | gen 3 39-48, gen 4/5 51-60 | `else: result = 100`; per-gen weather rules; drop the early clamp |
| X8 | Snow Cloak accuracy checked under `WEATHER_SANDSTORM` (gens 4/5); must be hail. | gen 4 98-99, gen 5 | `WEATHER_HAIL` |
| X9 | Move-name constants are gen 1–4 spellings; gen 5 json uses "Self-Destruct", "Solar Beam", "Smelling Salts" (Sonic Boom is handled). Damp, the Solar Beam weather penalty and the Smelling Salts dropdown are dead in gen 5. | `utils/constants.py`, `gen_five_constants.py` | compare on `sanitize_string(move.name)` everywhere |
| X10 | Placeholder base power (`1` in gens 3/4, `-1` in gen 2) flows into the vanilla formula for unimplemented moves and shows a bogus "1–4 damage" row: Super Fang (gen 1/2/3/4), Present, Frustration, Mirror Coat (gen 2), Counter/Bide/OHKO ×4/Low Kick (gen 3/4). In gen 5 the same moves have `null` power and return no damage instead — but so do Flail/Return/Gyro Ball/Natural Gift/... whose branches never run. | all gens | guard: `if base_power <= 0 and move not in VARIABLE_POWER_MOVES: return None`; run per-move overrides *before* the early-out |
| X11 | Multi-hit crit path: the recursive call for the non-crit hits passes the crit-reset stage modifiers and drops `weather` / `is_double_battle` (gens 2/3/4/5). Also the "exactly one hit crits" model is an approximation (game rolls crit per hit from gen 2 on; gen 1 rolls once — correct there). | gen 2 345-359, gen 3 543-557, gen 4 789-814 | keep the original modifiers; pass weather/doubles |
| X12 | `attacking_battle_stats` is mutated in place by the species-item branches; the controller reuses the same StatBlock for the normal and crit calls on the transformed-mon path. | gen 2 105-111, gen 3 297-339, gen 4 453-476 | copy before mutating |
| X13 | Fury Cutter has no 160 cap in any gen (dropdown "1".."6" → `2**(n-1)`): gen 2/3/4 max is n=5, gen 5 (base 20) n=4. | all gens | cap the multiplier / drop "6" |
| X14 | Frustration has no dropdown in any gen (power `(255-happiness)*10/25`); Return's list should be reused. | `*_constants.py` CUSTOM_MOVE_DATA | add `Frustration: ["102".."1"]` and extend the Return branch |
| X15 | Psywave: the app uses uniform 1..⌊1.5L⌋−1 in every gen. Correct for the gen 1 player and gen 2; the gen 1 *enemy* rolls 0..⌊1.5L⌋−1; gen 3 is 11 values `L*(50+10k)//100`; gen 4 is `L*(r+5)//10`, r∈0..10 (min 1). | shared flavor branch | per-gen distribution |
| X16 | `get_crit_rate` never sees the defender (Battle/Shell Armor → 0) or crit-stage items (Scope Lens, Razor Claw, Lucky Punch, Stick; Focus Energy needs a field flag) in gens 2–5. | per gen | add a defender param + item checks |
| X17 | High-crit flavour missing from data: gen 2 Aeroblast; gen 4 Razor Wind, Night Slash, Shadow Claw, Psycho Cut, Stone Edge, Cross Poison, Attack Order, Spacial Rend; gen 4 Double Hit lacks `two_hit`. Gen 1 accuracy data: Bind 85→75, Psywave 100→80 (Poison Gas 90→55, status). | `raw_pkmn_data/*/moves.json` | data edits |
| X18 | Accuracy in kill math: no gen applies accuracy/evasion stages; gen 1 has a 1/256 miss on every move and `floor(acc*255/100)/256` hit chances; gen 2+ `acc*255//100` (no 1/256 miss on 100%); BrightPowder/Lax Incense/Zoom Lens/fog/Gravity/Tangled Feet absent. `StageModifiers` already carries the stages. | `get_move_accuracy` per gen | optional precision work |

---

## 3. Per-generation findings

### Gen 1 (Red / Blue / Yellow) — [gen_1_findings.md](gen_1_findings.md)

Red/Blue and Yellow damage code, move table, type table, crit table and stat formulas are **byte-identical** (diffed);
one calc and one `moves.json` for all three is correct.

Verified correct: formula order/truncation, crit rate (`⌊bs/2⌋/256`, ×8 capped 255 for the 4 high-crit moves —
Slash from a fast mon really is 255/256) and crit stat handling (level doubled; stages, badge boosts and screens
all discarded), random roll, min 1, badge-boost value/mapping/stacking model (`StageModifiers` badge-boost counts
match the game's re-application on the player's own stat-ups and the enemy's landed stat-drops), stage table,
trainer DVs 9/8/8/8 (HP 8), multi-hit one-roll × N, fixed/level moves ignoring immunity (correct for gen 1),
Struggle/Swift, all 165 moves' power/type.

Bugs (gen 1 §3):
1. `>255 → both stats /4, low byte only` scaling commented out (`pkmn_damage_calc.py:97-105`), and Explosion's
   defence halving runs before it instead of after. 826 of 5280 swept cases differ; Reflect/Light Screen on a
   512+ stat wraps (Onix +6 Def + Reflect takes 4–5 from Horn Attack; app says 1–2). Exact replacement code in
   the file.
2. Type effectiveness must follow the ROM `TypeEffects` row order (ordered table + fix code in the file).
3. Super Fang (`base_power -1`) falls into the vanilla formula → `{1: 39}` / None.
4. Missing `min(q, 997) + 2` cap.
5. Enemy Psywave can roll 0.
6. Stat-exp sqrt term must cap `ceil(sqrt)` at 255 (only above 65025 stat exp).
7. moves.json accuracy: Bind 75, Psywave 80 (Poison Gas 55); Psychic's effect data should be a 33% *target*
   Special drop.

Not implemented (gen 1 §4): OHKO moves (speed-gated, 76/256), Counter, Bide, Super Fang (½ current HP), trapping
moves repeating turn-1 damage for 2–5 turns (model like multi-hit), Focus Energy crit formulas, the 1/256 miss
and accuracy/evasion stages, burn/paralysis penalties, level-up resetting stacked badge boosts (verify the router
calls `clear_badge_boosts`), X items via `apply_stat_mod` (verify), confusion self-hit.

### Gen 2 (Gold / Silver / Crystal) — [gen_2_findings.md](gen_2_findings.md)

Verified correct: formula order and floors, item ×1.1 position, crit ×2 and the `atkStage <= defStage → unboosted
stats` rule, STAB, type-chart row order vs the game table, weather table and position, random 217–255,
stat/stage/badge math including the exact Glacier/Sp.Def glitch windows, Flail/Reversal (no crit, no roll, single
value), fixed/level moves with immunity, Psywave, Magnitude/Rollout/Rage tables, multi-hit counts, all 251
moves' data.

Bugs (gen 2 §3, 18 items): 3.1 missing `TruncateHL_BC` (both stats /4 when either ≥ 256; G/S truncate once then
wrap, Crystal loops); 3.2 crits keep Reflect/Light Screen when the attacker's stage is higher; 3.3 Future Sight has
no `stab` command — no type effectiveness, weather or badge boost (app: None vs Dark, halves vs Psychic); 3.4 Hidden
Power `dv > 8` → `dv >= 8`; 3.5 Fury Cutter cap ×16; 3.6 Thick Club also for Cubone; 3.7 Metal Powder applies to
Sp.Def too, on the truncated value; 3.8 Aeroblast high-crit flag; 3.9 badge type boost is `+max(dmg>>3, 1)`;
3.10 Gust/Twister/Earthquake/Magnitude/Stomp/Pursuit double *after* the random roll (distribution only); 3.11
Triple Kick multiplies before STAB and the dropdown means "n-th kick"; 3.12 997 cap; 3.13 Thunder 50% in sun;
3.14 Struggle is boosted by Pink/Polkadot Bow; 3.15 multi-hit crit recursion (X11); 3.16 `DamageRange.add`
(X1); 3.17 in-place stat mutation (X12); 3.18 sqrt cap.

Not implemented (gen 2 §4): Present (Crystal: 40/80/120 dropdown; **Gold/Silver has a register-clobber formula**
that uses type ids as level/stats — fully derived in §1.15/§4.4), Frustration, Beat Up (per-party-member base
Atk/level, no STAB/type), Super Fang, OHKO (KO + `76 + 2·ΔL` accuracy), Counter/Mirror Coat/Bide (need damage
taken), False Swipe HP−1 cap, Focus Energy/Scope Lens/Lucky Punch/Stick crit stages, accuracy/evasion stages,
Bright Powder, Foresight, the G/S truncation wrap.

### Gen 3 (Ruby / Sapphire / Emerald / FireRed / LeafGreen) — [gen_3_findings.md](gen_3_findings.md)

Version notes: RS/E/FRLG damage code and all 354 moves' data are identical, **except** Ruby/Sapphire apply the
Atk/Def/SpA/SpD badge boosts only in trainer battles (app applies them in wild battles too — bug #17). FRLG does
have badge boosts.

Verified correct: base formula and every truncation, `+2`, crit ×2 placement and the crit stat-stage rule,
Battle/Shell Armor, Spit Up/Future Sight/Doom Desire never crit, physical/special split by type, Hidden Power
bit-for-bit, STAB, chart order and dual-type rounding, random 85–100, weather incl. SolarBeam in sand/hail and
Cloud Nine/Air Lock, Forecast, Weather Ball typing, screens in singles, spread set (`MOVE_TARGET_BOTH` only),
Explosion halving value, Damp, Magnitude/Flail/Return/Eruption/Spit Up tables, all `dmgMultiplier` bonuses, fixed
damage moves, crit table and the 11 high-crit moves, Compound Eyes/Hustle/Sand Veil/Thunder-rain accuracy.

Bugs (gen 3 §3, 26 items): 1 Rollout/Ice Ball ×2 on every turn (turn 1: game 8–10, app 17–21; "5 + DefenseCurl"
×64 vs ×32); 2 Nature Power dead code + Frustration placeholder; 3 Fury Cutter (no cap, damage-multiplier not
base power); 4 Triple Kick hits are 10/20/30 with independent rolls; 5 Flash Fire constant (X3); 6 Lightning Rod
not an immunity (X4); 7 Soundproof missing; 8 Weather Ball doubles base power instead of final damage (rounding);
9 all item/ability stat modifiers applied *after* the stage multiplier — game applies them before, then the
stage (4 concrete off-by-one cases); 10 Rage has no damage multiplier in gen 3 (it raises Attack stage); 11 Soul
Dew wired to DeepSeaScale, defending Clamperl/Lati@s items missing; 12 item boost table (X5); 13 Thick Club for
Cubone; 14 multi-hit crit recursion (X11); 15 `DamageRange.add` (X1); 16 Reflect/Light Screen ×2/3 in doubles;
17 RS wild-battle badge boosts; 18 physical min-1 before `+2` missing; 19 Nature Power accuracy (X7); 20 Thunder
in sun / Blizzard in hail (X7); 21 Compound Eyes clamp order; 22 Psywave 11-value distribution; 23 OHKO moves
produce 1–2 damage; 24 Future Sight/Doom Desire get type effectiveness and immunities (game: none); 25 crit rate
ignores Scope Lens/Lucky Punch/Stick/Battle Armor; 26 misc (Spit Up crit path, Dragon Rage vs Wonder Guard,
Struggle vs Wonder Guard, Hustle accuracy type, Rollout Defense Curl per turn).

Not implemented (gen 3 §4): Nature Power (delegate to the mapped move: Swift/Earthquake/Surf/Razor Leaf/Rock
Slide/Shadow Ball/BubbleBeam/Hydro Pump/Stun Spore), Frustration, Low Kick (needs species weights — gen 3
pokemon.json has none; source `pokedex_entries.h`), Counter/Mirror Coat/Bide, Super Fang, Present, Endeavor
(json power 0 → treated as status), Beat Up (typeless per-party formula), and the ability/field table in §4.9
(Guts, Marvel Scale, Overgrow-family at HP ≤ ⅓, Plus/Minus, Sturdy vs OHKO, Soundproof, Flash Fire boost, burn,
Charge, Helping Hand, Mud/Water Sport, Focus Energy, BrightPowder/Lax Incense, accuracy stages, Lock-On).

### Gen 4 (Diamond / Pearl / Platinum / HeartGold / SoulSilver) — [gen_4_findings.md](gen_4_findings.md)

Version notes: Platinum and HGSS move tables and damage code are identical for everything audited; DP's move table
differs only in Hypnosis accuracy (70). DP battle code is not decompiled anywhere available — everything is
"verified for Pt/HGSS, assumed identical for DP".

Verified correct: base formula/truncation/`+2`, crit ×2/×3 position, STAB ×1.5/×2 position, chart order and
per-step floor, random 85–100, no cap, stat formula and stage table, no badge boosts, Hidden Power bit-for-bit,
weather incl. SolarBeam halved in fog and Weather Ball in fog, sandstorm Rock Sp.Def ×1.5, Solar Power, Cloud
Nine/Air Lock, Forecast, Brick Break, screens skipped on crits, Damp, Hustle/Huge Power/Choice items/Scarf/Macho
Brace/Tailwind/Slow Start (Atk, Spe)/Metal Powder/DeepSeaTooth/Thick Club (Marowak), Adaptability, Sniper,
Super Luck, Normalize, Scrappy, Multitype, Klutz, Wonder Guard, Volt/Water Absorb, Motor Drive, Dry Skin
(Water), Levitate, Magnet Rise, Gravity/Roost/Miracle Eye, Worry Seed/Gastro Acid, fixed-damage moves,
Magnitude/Flail/Return/Eruption/Trump Card (3/2/1)/Gyro Ball cap, multi-hit per hit, Compound Eyes/Wide
Lens/Sand Veil/No Guard, all move data vs Pt and HGSS.

Bugs (gen 4 §3, 28 items): 1 crit stage block inverted and over-broad (`351-361`); 2 Rollout/Ice Ball `2**n`;
3 Fury Cutter uncapped; 4 Technician hardcoded to 90 and evaluated before variable powers resolve (Bullet Punch
2.3× instead of 1.5×); 4b Crush Grip/Wring Out `1 + 120*pct` (12001 power at 100%); 4c Gyro Ball ratio inverted
(and fed the Trick-Room-transformed speed); 4d Punishment base 1 not 60, cap on power 200, all 8 stages; 4e Trump
Card "4+" unmatched, no "0" (200); 5 Spit Up power 1×n and wrongly no crit; 6 Rage multiplies damage; 7
Frustration; 8 eight high-crit moves lack the flavour; 9 Double Hit is one hit; 10 Low Kick/Grass Knot brackets
`<` kg vs `<=` 0.1 kg (Magikarp, Flareon, Venusaur...); 11 Flower Gift boosts SpA and clobbers SpD (game: Atk /
SpD); 12 Light Ball is a power ×2 for both categories; 13 Soul Dew never triggers (compares to DeepSeaScale);
14 item boost table (X5); 15 Power Trick swaps Atk/SpA instead of Atk/Def; 16 doubles spread never fires and
uses ÷2 not ×3/4 (X2); 17 Struggle is typeless (app: Normal → immune vs Ghost, STAB); 18 Natural Gift: 17
berries 80 vs 60; 19 Earth Plate (X6); 20 Filter/Solid Rock per SE type (4× → 2.25× not 3×), Tinted Lens on
any NVE entry; 21 Psywave distribution; 22 `DamageRange.add` (X1); 23 Nature Power always None + gen 3 table
(gen 4: Plain/Sand→Earthquake, Grass/Puddle→Seed Bomb, Mountain/Cave→Rock Slide, Snow→Blizzard, Water→Hydro
Pump, Ice→Ice Beam, Building→Tri Attack, Great Marsh→Mud Bomb, Bridge→Air Slash); 24 Snow Cloak (X8); 25 Flash
Fire (X3); 26 Lightning Rod (X4); 27 Future Sight/Doom Desire through the type chart; 28 minor/rounding (Dry Skin
1.3 vs 1.25 on power, Thick Fat/Heatproof and type items/orbs on stat vs power, raw `move.move_type` used for
Scrappy/ground-immunity/orbs/Dry Skin, Explosion on staged Def, Cubone, DeepSeaScale side, Slow Start SpA,
Needle Arm/Astonish/Extrasensory Minimize dropdowns that don't exist in gen 4, Battle Armor crit rate, Thunder
in sun, Flail labels).

Not implemented (gen 4 §4): Present, Beat Up, Fling (full item power table in the file), Counter/Mirror
Coat/Metal Burst/Bide, Endeavor, Super Fang, OHKO (level-diff accuracy + Sturdy), Nature Power gen 4 table,
true 3-hit Triple Kick, abilities (Guts, Marvel Scale, Overgrow-family, Rivalry, Iron Fist, Reckless, Plus/Minus,
Simple, Unaware, Mold Breaker, Flash Fire boost, Soundproof, Tangled Feet, Sturdy, burn, Skill Link), field
effects (Charge, Helping Hand, Sports, Me First, Foresight, Ingrain, Iron Ball, Gravity/fog accuracy, doubles 2/3
screens), items (Life Orb, Expert Belt, Metronome, Muscle Band/Wise Glasses, resist berries, Scope Lens/Razor
Claw/Lucky Punch/Stick, BrightPowder/Lax Incense, Zoom Lens, Micle, Iron Ball, Quick Powder), accuracy/evasion
stages.

### Gen 5 (Black / White / Black 2 / White 2) — [gen_5_findings.md](gen_5_findings.md), [gen_5_move_table.md](gen_5_move_table.md)

No decomp exists, so nothing here is verified against game code; what IS verified is what the app does. Every gen
4 bug above applies verbatim (the gen 5 calc is a renamed copy). On top of that, verified by running the app:

- `moves.json` has no `attack_flavor` → high-crit stage never applied (Karate Chop ... Drill Run, 18 moves),
  2–5-hit and 2-hit moves show one hit and no "N Hits" dropdown (19 moves), Psywave/fixed-damage flavours
  never fire (fixed/level damage still works by name).
- `power: null` on variable-power moves hits the early-out before the move's branch → Low Kick, Grass Knot,
  Flail, Reversal, Return, Frustration, Present, Spit Up, Gyro Ball, Natural Gift, Trump Card, Crush Grip, Wring
  Out, Punishment, Beat Up, Fling, Psywave and the new Heavy Slam/Heat Crash/Electro Ball/Final Gambit all
  return no damage.
- Name mismatches: Self-Destruct (no Damp), Solar Beam (no weather penalty), Smelling Salts (no dropdown).
- Constants are gen 4's: Natural Gift table is 20 too low for every berry, Minimize bonus list should be
  {Stomp, Steamroller}, Nature Power table, Fog is offered as a weather, Explosion still halves defence.
- Formula-level gen 5 changes not implemented (documented, unverified): random roll before STAB/type, spread
  ×0.75, screens as a final ×0.5 modifier, 4096-based round-half-down chain, Technician on actual power, no
  Explosion halving, Struggle typeless, Future Sight as a normal hit, Storm Drain/Lightning Rod absorb, Sap
  Sipper, Sheer Force/Reckless/Iron Fist/Sand Force/Analytic/Multiscale/Defeatist/Contrary/Unaware, Life Orb/
  Expert Belt/Gems/Muscle Band/Wise Glasses/Eviolite/Metronome, Frost Breath/Storm Throw always crit, Psyshock/
  Psystrike/Secret Sword vs Defense, Foul Play, Chip Away/Sacred Sword, Acrobatics, Hex, Venoshock, Stored Power,
  Echoed Voice, Retaliate, Techno Blast, Fusion moves, Pledges, Final Gambit, weight/speed-ratio moves, Hurricane
  accuracy.

The per-move table classifies all 364 damaging gen 5 moves: 228 vanilla, 4 OK, 25 OK via a name-keyed dropdown,
40 BROKEN by the data shape, 13 CHECK (gen 2–4 rule that gen 5 changed), 54 NONE.

---

## 4. Recommended fix order

**Phase A — data & constants (no formula risk, biggest silent wins)**
1. `FLASH_FIRE_ABILITY = "Flash Fire"` (gens 3/4/5). `Earth Plate → Ground` (4/5). Snow Cloak → hail (4/5).
2. `moves.json`: add `high_crit` to Aeroblast (gen 2) and the 8 gen 4 moves; `two_hit` to Double Hit (gen 4);
   Bind 75 / Psywave 80 accuracy (gen 1). Gen 5: synthesise `attack_flavor` from `effect` in `_load_move_db`
   (`high_crit_rate`→`high_crit`, `two_to_five_hits`→`multi_hit`, `two_hits*`→`two_hit`, `psywave`, ...).
3. `type_info.json` held-item tables (X5). Natural Gift table: 17 berries → 60 (gen 4); gen 5 = gen 4 + 20.
4. Dropdowns: Frustration (all gens), Trump Card add "0" and map "4+", Rollout/Ice Ball "n + DefenseCurl",
   remove Rage (gens 3+) and Minimize options for Astonish/Needle Arm/Extrasensory (gens 4/5), Present
   "40/80/120", Nature Power = the gen's terrain list.
5. Gen 5: drop Fog; Minimize list {Stomp, Steamroller}; name constants via `sanitize_string`.

**Phase B — shared code**
6. `DamageRange.add`: multiply counts (X1). Add a regression test with the {10:1,11:2} example.
7. Targeting normalisation per gen + gen 4/5 ×3/4 spread + doubles screens ×2/3 (X2).
8. Placeholder-power guard (X10) so unimplemented moves show no row instead of "1 damage"; move all per-move
   power overrides *before* the `base_power` early-out (fixes Nature Power in gens 3/4/5 and every null-power
   gen 5 move at once).
9. Multi-hit crit recursion: pass original stage modifiers, weather, doubles (X11); copy StatBlocks (X12).

**Phase C — per-gen formula fixes (each has a numeric test in its detail file)**
10. Gen 4/5: crit stage block (bug #1) — 10-line fix, highest impact.
11. Gen 1: `.scaleStats` + low byte + Explosion order (replacement code in gen 1 bug 1); ROM-order type
    effectiveness (ordered table in gen 1 bug 2); 997 cap; enemy Psywave 0.
12. Gen 2: `TruncateHL_BC` (Crystal loop / G/S single pass + wrap); crit keeps screens when atkStage >
    defStage; Future Sight skips type/weather/badge; Hidden Power `>= 8`; badge type boost `+max(x>>3,1)`;
    Thick Club Cubone; Metal Powder Sp.Def; Thunder 50% in sun; 997 cap; double-after-roll for the ×2 bonuses.
13. Gen 3: modifier-before-stage restructure (bug #9), Soul Dew/DeepSeaScale sides, Lightning Rod, Weather Ball
    as a damage multiplier, physical min-1, Reflect ×2/3 doubles, RS wild badge rule, Psywave, Future Sight,
    Cubone, Soundproof, accuracy fixes.
14. Gen 3/4/5 variable-power moves: Rollout `2**(n-1)` + Defense Curl per turn, Fury Cutter cap, Triple Kick
    10/20/30 summed, Rage removed, Spit Up 100×n (crit allowed in gen 4), Technician after power resolution on
    the real power, Gyro Ball `25*def/atk + 1`, Crush Grip `120*hp/max + 1`, Punishment `60 + 20n` cap 200,
    Trump Card map, Light Ball power ×2 (gen 4), Flower Gift Atk/SpD, Power Trick Atk/Def, Low Kick `<=`
    hectograms, Filter/Solid Rock once on net SE, Struggle typeless (gen 4/5), Future Sight no chart.

**Phase D — new inputs / not-implemented moves (ranked by routing value)**
15. Low Kick (add `weight` to the gen 3 species db; gen 4/5 already have it), Super Fang (target HP % dropdown,
    default full), OHKO (damage = target HP, level-gated accuracy, Sturdy), Present, Beat Up (enemy trainers
    have the party in the trainer db; player side needs a party input or a "N hits" fallback), Endeavor, Fling
    (gen 4/5 item table in gen 4 §4), Counter/Mirror Coat/Metal Burst/Bide (a "damage taken" numeric input, or
    keep as no-number), Focus Energy / crit items, accuracy stages, Guts/Marvel Scale/Overgrow-family (a
    "status"/"≤⅓ HP" toggle), Life Orb/Expert Belt/Muscle Band/Wise Glasses/Metronome (gen 4/5 items).

**Phase E — gen 5 proper**
16. Write `calculate_gen_five_damage` as the gen 5 modifier chain (random before STAB/type, 4096-space
    round-half-down, screens/Multiscale/Tinted Lens/Filter/Expert Belt/Life Orb/berries as final modifiers,
    ×0.75 spread), then the new gen 5 moves and abilities listed in gen 5 §5. Until a reference exists, mark the
    numbers as unverified in the UI or tests.

---

## 5. Test plan

- Each bug in the detail files carries a concrete attacker/defender/level/stage case with the expected
  `DamageRange` (or min/max) computed from the game code by hand and cross-checked with the transcribed
  reference calculators in `reference/`. Turn them into `tests/test_damage_gen{N}.py` parametrised cases; the
  existing `TestWeatherBallAndForecast` / `TestMagnitude` classes show the fixture pattern
  (`gen_factory.change_version`, `create_trainer_pkmn`, `calculate_damage`).
- Keep `reference/gen1_ref_harness.py`, `gen2_ref.py`, `ref_gen3.py` and `app_probe.py` as oracles: they
  transcribe `CalculateDamage` (gen 1), `BattleCommand_DamageCalc` (gen 2), `CalculateBaseDamage` (gen 3) and
  `BattleSystem_CalcMoveDamage` (gen 4) and can sweep thousands of matchups against the app (`gen1_sweep.py`
  ran 5280; `gen2_compare.py` / `run_cases*.py` are the gen 2/3 runs).
- Add a data test per gen that every key in `held_item_boosts` exists in `items.json`, every move-name constant
  exists in that gen's `moves.json` (sanitised), and every `CUSTOM_MOVE_DATA` key is a real move.
- Run with `py -3.14 -m pytest` (the `python` on PATH is devkitPro's and lacks the deps).
