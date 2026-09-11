# Gen 5 (Black / White / Black 2 / White 2) damage-calc audit

**No gen 5 decompilation is available**, so nothing in this section is verified against game code. Every
"what the game does" statement below is documented community knowledge (marked *unverified*). What IS verified
is what the app does: every claim about the app was checked by reading `pkmn/gen_5/*` and by running the
gen 5 calc through `gen_factory.change_version(const.BLACK_VERSION)`.

## 0. How gen 5 is wired today

- `pkmn/gen_5/pkmn_damage_calc.py` is a verbatim copy of the gen 4 calc (only the `gen_four_const` ->
  `gen_five_const` renames differ, plus a Sand Veil / Snow Cloak accuracy block that was moved and a Wide Lens
  multiplier written as `4506/4096`). Every gen 4 bug in the gen 4 audit therefore applies to gen 5 too.
- `pkmn/gen_5/gen_five_constants.py` is the gen 4 constants file with the badge names swapped. The Natural Gift
  table, the plate table (incl. the `"Earth Plate": Dark` typo), the ability list, the Minimize-bonus move list and
  every `CUSTOM_MOVE_DATA` dropdown are gen 4 values.
- `pkmn/gen_5/data_objects.py` is the gen 4 stat code (no badge boosts, natures, Power Trick / Slow Start /
  Tailwind / Trick Room / Choice Scarf / speed-halving items). Stat formulas are unchanged in gen 5, so this is fine.
- `raw_pkmn_data/gen_five/moves.json` (559 usable moves; 18 "Shadow" moves are skipped by the loader) has **no
  `attack_flavor` list and no structured `effects`**; it only carries a scalar `effect` code. Variable-power moves
  have `power: null` where the gen 4 file uses the placeholder `1`.
- `get_valid_weather()` still offers **Fog**, which does not exist in gen 5.
- The type chart in `gen_five/type_info.json` is byte-identical to gen 4 (correct: no chart changes until gen 6).
- Species weights are present for every gen 5 species (needed for Low Kick / Grass Knot / Heavy Slam / Heat Crash).

## 1. Structural breakages caused by the data shape (verified by running the app)

| # | Symptom | Root cause | Moves affected |
|---|---|---|---|
| 1 | High-crit moves get the base 1/16 crit rate | `get_crit_rate` tests `const.FLAVOR_HIGH_CRIT in move.attack_flavor`; gen 5 flavor lists are empty | Karate Chop, Razor Leaf, Crabhammer, Slash, Aeroblast, Cross Chop, Air Cutter, Leaf Blade, Night Slash, Shadow Claw, Psycho Cut, Stone Edge, Attack Order, Spacial Rend, Drill Run, Poison Tail, Cross Poison, Blaze Kick (+ Sky Attack / Razor Wind, which the json never flagged in any gen) |
| 2 | 2-5 hit moves show ONE hit and offer no "N Hits" dropdown | controller: `if const.FLAVOR_MULTI_HIT in move.attack_flavor` -> never; calc: same | Double Slap, Comet Punch, Fury Attack, Pin Missile, Spike Cannon, Barrage, Fury Swipes, Bone Rush, Arm Thrust, Bullet Seed, Icicle Spear, Rock Blast, Tail Slap |
| 3 | 2-hit moves show ONE hit | `const.DOUBLE_HIT_FLAVOR in move.attack_flavor` -> never | Double Kick, Bonemerang, Double Hit, Twineedle, Dual Chop, Gear Grind |
| 4 | Variable-power moves return **None** (no damage row at all) | `if base_power is None or base_power == 0: return None` runs before every per-move `base_power =` override | Low Kick, Grass Knot, Flail, Reversal, Return, Frustration, Present, Spit Up, Gyro Ball, Natural Gift, Trump Card, Crush Grip, Wring Out, Punishment, Beat Up, Fling, Psywave (+ Counter, Mirror Coat, Metal Burst, Bide, Endeavor, Super Fang, the 4 OHKO moves, Final Gambit, Heavy Slam, Heat Crash, Electro Ball which are unimplemented anyway) |
| 5 | Name mismatches silently disable special handling | gen 5 json uses modern spellings; constants use gen 1-4 spellings | "Self-Destruct" (Damp immunity not applied), "Solar Beam" (no x0.5 in rain/sand/hail), "Smelling Salts" (no paralysis dropdown / x2). "Sonic Boom" IS handled (both spellings are in `_FIXED_DAMAGE_BY_NAME`). |
| 6 | Double-battle spread halving never fires | calc tests `move.targeting == "target_both_enemies"`; gen 4/5 json uses `"All Foes"` | every spread move (Earthquake, Surf, Discharge, Rock Slide, Heat Wave, Blizzard, Bulldoze, ...) - also broken in gen 4 (only gen 3 data uses the `target_both_enemies` vocabulary). BW has 41 and B2W2 59 double battles in the trainer DB. |

Fix for 1-3 (data or code, pick one):
- Data: derive `attack_flavor` for gen 5 from the `effect` code when loading (`high_crit_rate` /
  `*_and_high_crit_rate` -> `high_crit`; `two_to_five_hits` -> `multi_hit`; `two_hits` / `two_hits_and_may_poison`
  -> `two_hit`; `psywave` -> `psywave`; `fixed_damage` -> `fixed_damage`; `level_based_damage` -> `level_damage`;
  `one_hit_ko`, `bonus_minimize_damage_and_may_flinch` -> `bonus_minimize_damage`, etc.) in
  `gen_five_object._load_move_db`, OR regenerate the json with the same flavor vocabulary as gen 4.
- Code: alternatively key `get_crit_rate` / the multi-hit detection off `move.effects`/`effect` in gen 5.
Fix for 4: in the calc, move the `base_power is None` early-out AFTER the per-move power overrides (or give gen 5
json the same `1` placeholders as gen 4 and let the override branches run). The cleanest fix is a per-gen
`VARIABLE_POWER_MOVES` set that bypasses the early-out.
Fix for 5: compare on `sanitize_string(move.name)` everywhere a move-name constant is used, or add gen-5
spellings to the constants (`SELFDESTRUCT_MOVE_NAMES = {"Selfdestruct", "Self-Destruct"}` etc.).
Fix for 6: map the gen 4/5 `target` strings (`"All Foes"`, `"Others"`, `"All"`) to `const.TARGETING_BOTH_ENEMIES`
in `_load_move_db`, and use the gen 5 spread factor 0.75 (gen 4: 0.5).

## 2. Formula-level differences between gen 4 (what the copy implements) and gen 5 (*unverified*)

Gen 5 replaced the gen 3/4 "apply multipliers as you go" formula with a fixed modifier chain using 4096-based
fixed-point multipliers and round-half-down ("pokeRound") at each step. Where the copied gen 4 code differs:

| Step | Gen 5 (documented, unverified) | Gen 4 copy in the app |
|---|---|---|
| Base damage | `floor(floor(floor(2L/5 + 2) * Power * A / D) / 50) + 2` | same |
| Spread (multi-target hit) | x0.75, applied only when more than one target is actually hit | x0.5 (and never fires, see 1.6) |
| Weather | x1.5 / x0.5 (Water/Fire in rain/sun) | same position |
| Critical hit | x2 (x3 Sniper); stages 1/16, 1/8, 1/4, 1/3, 1/2; Frost Breath / Storm Throw always crit | x2/x3, same stage table; no always-crit support |
| **Random roll** | `damage * (85..100) / 100` **before STAB and type** | applied LAST, after STAB/type/abilities -> different rounding for many damage values |
| STAB | x1.5 (x2 Adaptability), pokeRound | same factors, but ordered after random in gen 5 |
| Type effectiveness | applied to the rolled damage | applied before the roll |
| Burn | x0.5 on physical (not with Guts) | not modelled (no status input in any gen) |
| Final modifier chain | Reflect / Light Screen x0.5 (x2/3 in doubles), Multiscale x0.5, Tinted Lens x2, Friend Guard x0.75, Sniper (crit only), Filter/Solid Rock x0.75, Metronome item, Expert Belt x1.2, Life Orb x1.3, type-resist berries x0.5 (Enigma n/a), all chained in 4096 space then pokeRound | Screens halve the intermediate damage BEFORE +2 (gen 3/4 rule); Life Orb / Expert Belt / Metronome / berries / Multiscale / Friend Guard absent; Filter/Solid Rock applied per SE type (see gen 4 audit) |
| Minimum | 1 | 1 |
| Explosion / Self-Destruct | no defense halving | halves defense ("Explosion" only, see 1.5) |
| Technician | x1.5 on the move's actual power if <= 60 | hardcoded 90 (`floor(60*1.5)`) regardless of the actual power - also wrong in gen 4 |
| Power modifiers (4096 space) | Technician, Iron Fist x1.2, Reckless x1.2, Rivalry x1.25/x0.75, Sand Force x1.3, Analytic x1.3, Sheer Force x1.3, Heatproof x0.5 (on power, not stat), Dry Skin fire x1.25, Flare Boost / Toxic Boost x1.5 (stat), Muscle Band / Wise Glasses x1.1, type items & plates & incenses x1.2, Gems x1.5 (single use), Charge x2, Helping Hand x1.5, Mud/Water Sport x1/3, Me First x1.5 | only Technician (wrong), Heatproof and Thick Fat (as stat halving), type items x1.2 (on the stat, gen 3/4 style), Adamant/Lustrous/Griseous Orb; none of the rest |
| Attack-stat modifiers | Huge/Pure Power x2, Hustle x1.5, Guts x1.5, Slow Start x0.5, Defeatist x0.5 (<= 1/2 HP), Solar Power x1.5 SpA, Flower Gift x1.5 Atk, Plus/Minus x1.5 SpA, Choice Band/Specs x1.5, Light Ball x2 (Atk AND SpA), Thick Club x2, Deep Sea Tooth x2 SpA, Soul Dew x1.5 SpA/SpD, Thick Fat (opponent's Fire/Ice attacks) x0.5 on the attacking stat; Unaware ignores stages; Foul Play uses the target's Atk | Huge/Pure Power, Hustle, Choice items, Light Ball (SpA only - gen 4 also boosts Atk, see gen 4 audit), Thick Club, Deep Sea items, Soul Dew, Thick Fat/Heatproof; Flower Gift boosts the WRONG stats (SpA/SpD instead of Atk/SpD, and reads SpA into SpD); Solar Power ok; no Guts/Defeatist/Plus/Minus/Unaware/Foul Play |
| Defense-stat modifiers | Marvel Scale x1.5 (statused), Eviolite x1.5 Def/SpD (NFE), Deep Sea Scale x2 SpD, Metal Powder x2 Def (Ditto, untransformed), Soul Dew, Flower Gift x1.5 SpD, sandstorm Rock SpD x1.5, Chip Away / Sacred Sword ignore stages, Psyshock / Psystrike / Secret Sword use Def | sandstorm Rock SpD ok; Deep Sea Scale ok; Metal Powder ok; no Eviolite / Marvel Scale / Chip Away / Psyshock |
| Immunities / type overrides | Levitate, Volt Absorb, Motor Drive, Lightning Rod (gen 5: absorbs Electric in singles, +1 SpA), Storm Drain (gen 5: absorbs Water, +1 SpA), Water Absorb, Dry Skin (Water), Flash Fire, Sap Sipper (Grass, +1 Atk), Soundproof, Wonder Guard, Damp, Sturdy vs OHKO, Struggle typeless, Normalize, Multitype, Techno Blast drives, Judgment plates, Weather Ball, Natural Gift, Hidden Power (IV-based 30-70) | Levitate, Volt Absorb, Motor Drive, Lightning Rod, Water Absorb, Dry Skin, Flash Fire (constant is the string "Water Absorb": never matches), Wonder Guard, Damp; no Storm Drain / Sap Sipper / Soundproof / Sturdy; Struggle treated as Normal; no drives |
| Accuracy | Compound Eyes x1.3, Victory Star x1.1, Hustle x0.8 (physical), Wide Lens x1.1, Zoom Lens x1.2, Bright Powder / Lax Incense x0.9, Sand Veil / Snow Cloak x0.8 in the matching weather, Tangled Feet x0.5, Gravity x5/3, No Guard, Wonder Skin (status), Thunder / Hurricane: rain always hits, sun 50%; Blizzard: hail always hits; OHKO = 30 + (Lu - Lt) | Compound Eyes, Hustle, Wide Lens, No Guard, Sand Veil, Snow Cloak (the gen 5 copy checks BOTH under SANDSTORM - Snow Cloak should be hail), Thunder rain, Blizzard hail; nothing else |

## 3. Per-move table (all 364 damaging moves)

The full table is in `gen_5_move_table.md` (same folder). Status meanings:
- **VANILLA** (228): goes through the plain formula; only the section-2 formula differences apply.
- **OK** (4): handled correctly by name (Sonic Boom, Dragon Rage, Seismic Toss, Night Shade).
- **OK\*** (25): the special-case code path runs (dropdown by name); correctness depends on the gen 4 audit of that
  path plus the section-2 differences.
- **BROKEN** (40): a gen 4 mechanism the app has, silently disabled in gen 5 by the data shape (section 1).
- **CHECK** (13): runs, but with a gen 2-4 rule that gen 5 changed, or a known-wrong table.
- **NONE** (54): produces no damage / no special handling; needs implementation.

Status lists (also in `gen_5_status_lists.txt`):

- BROKEN: Karate Chop, Double Slap, Comet Punch, Double Kick, Fury Attack, Twineedle, Pin Missile, Razor Leaf,
  Solar Beam, Self-Destruct, Spike Cannon, Barrage, Crabhammer, Fury Swipes, Bonemerang, Slash, Aeroblast,
  Bone Rush, Cross Chop, Smelling Salts, Arm Thrust, Blaze Kick, Air Cutter, Bullet Seed, Icicle Spear, Poison
  Tail, Leaf Blade, Rock Blast, Night Slash, Shadow Claw, Psycho Cut, Cross Poison, Stone Edge, Attack Order,
  Double Hit, Spacial Rend, Drill Run, Dual Chop, Tail Slap, Gear Grind
- NONE: Guillotine, Horn Drill, Low Kick, Counter, Fissure, Bide, Psywave, Super Fang, Flail, Reversal, Return,
  Present, Frustration, Mirror Coat, Beat Up, Spit Up, Endeavor, Sheer Cold, Gyro Ball, Natural Gift, Metal
  Burst, Fling, Trump Card, Wring Out, Punishment, Grass Knot, Crush Grip, Psyshock, Venoshock, Storm Throw,
  Heavy Slam, Synchronoise, Electro Ball, Foul Play, Round, Echoed Voice, Chip Away, Stored Power, Hex,
  Acrobatics, Retaliate, Final Gambit, Water Pledge, Fire Pledge, Grass Pledge, Frost Breath, Sacred Sword,
  Heat Crash, Steamroller, Psystrike, Techno Blast, Secret Sword, Fusion Flare, Fusion Bolt
- CHECK: Razor Wind, Sky Attack, Explosion, Struggle, Fury Cutter, Future Sight, Needle Arm, Astonish,
  Extrasensory, Doom Desire, Judgment, Smack Down, Hurricane
- OK*: Gust, Stomp, Surf, Blizzard, Thunder, Earthquake, Rage, Triple Kick, Rollout, Magnitude, Pursuit, Hidden
  Power, Twister, Facade, Revenge, Brick Break, Eruption, Ice Ball, Weather Ball, Water Spout, Wake-Up Slap,
  Brine, Payback, Assurance, Avalanche

Status moves (195) were skipped; none of them is treated as damaging by the app (power is null and category is
"Status").

## 4. Gen 5 move data changes vs the gen 4 file (the app already carries these in gen_five/moves.json)

Verified by diffing the two json files on `rom_id`: Bind 75->85 acc, Jump Kick 85->100, Tackle 35->50 / 95->100,
Wrap 85->90 acc, Thrash / Petal Dance 90->120, Disable 80->100 acc, Fire Spin 15->35 / 70->85, Toxic 85->90 acc,
Clamp 75->85 acc, High Jump Kick 100->130, Glare 75->100, Poison Gas 55->80, Crabhammer 85->90, Cotton Spore
85->100, Scary Face 90->100, Bone Rush 80->90, Giga Drain 60->75, Fury Cutter 10->20, Future Sight 80->100 /
90->100, Whirlpool 15->35 / 70->85, Uproar 50->90, Sand Tomb 15->35 / 70->85, Bullet Seed / Icicle Spear 10->25,
Covet 40->60, Rock Blast 80->90, Doom Desire 120->140 / 85->100, Feint 50->30, Last Resort 130->140, Drain Punch
60->75, Magma Storm 70->75. These match the documented BW changes (*unverified* against a ROM).

## 5. Implementation plan for gen 5

1. **Stop sharing the gen 4 calc.** Write `calculate_gen_five_damage` as the gen 5 modifier chain (section 2) with a
   `poke_round(x)` helper (`(x * m + 2048 - 1) >> 12` style round-half-down) and the random roll moved before STAB.
   Keep the gen 4 file untouched.
2. **Fix the data shape** (section 1): synthesize `attack_flavor` from `effect` in `_load_move_db`, drop Fog from
   `get_valid_weather`, add the `"All Foes"` -> spread mapping, and stop early-returning on `power: null` for the
   variable-power set.
3. **Gen 5 constants**: Natural Gift table +20 on every berry; plate typo; Minimize-bonus list = {Stomp,
   Steamroller}; drives for Techno Blast; remove the Fog weather; add the new dropdowns (Hex / Venoshock "Status
   Bonus", Echoed Voice "1".."5", Retaliate, Acrobatics is automatic, Fusion Flare/Bolt "Combo Bonus").
4. **Implement the NONE list** in priority order for routing use: Acrobatics (automatic from held item), Low Kick /
   Grass Knot / Heavy Slam / Heat Crash (weights already present), Electro Ball / Gyro Ball (speeds already
   computed), Stored Power (from stage modifiers), Hex / Venoshock / Retaliate / Echoed Voice (dropdowns), Psyshock /
   Psystrike / Secret Sword (use Defense), Foul Play (use target Attack), Chip Away / Sacred Sword (ignore
   stages), Frost Breath / Storm Throw (force the crit range only), Return / Frustration / Flail / Reversal / Spit Up
   / Trump Card / Punishment / Crush Grip / Wring Out (same code as gen 4 once the early-out is fixed), Final Gambit
   (needs an HP input), Natural Gift (+20 table), Beat Up / Fling / Present (new tables), Techno Blast (drive
   lookup). Counter / Mirror Coat / Metal Burst / Bide / Endeavor / Super Fang / OHKO stay "no damage number" unless
   the app grows an incoming-damage or current-HP input.
5. **Abilities / items** that matter for in-game routing (the player's mon and gym-leader mons): Sheer Force,
   Reckless, Iron Fist, Sand Force, Analytic, Technician (fix), Guts / Defeatist (need status / HP input),
   Sap Sipper / Storm Drain / Soundproof immunities, Sturdy vs OHKO, Multiscale, Eviolite, Life Orb, Expert Belt,
   Muscle Band / Wise Glasses, Gems, Metronome, Flash Fire (fix the constant), Snow Cloak weather check.
6. **Tests**: once a reference is available (a gen 5 decomp or trusted in-game captures), add a parametrised test
   module mirroring `TestWeatherBallAndForecast` for each implemented move.
