# Open issues found while adding send-out prediction (2026-11-04)

These came up while building the recorder's send-out prediction (see "Enemy
team order" in [README.md](README.md)). The prediction caused none of them: a
run with it switched off gave byte-identical routes for Emerald, Platinum and
Black.

**Update 2026-11-05:** all eight are closed (issue 2 by removing the Dex page).
See [Results](#results-2026-11-05) for what fixed them, three more problems the
work turned up, and what is still inexact. The sections below are the original
write-ups, kept for their reasoning.

Each issue stands alone. Items marked **Proven** were confirmed from code or
decomps. Items marked **Lead** are a best guess that still needs checking.

| # | Issue | Area | Status |
|---|---|---|---|
| 1 | [Short `mon_order` drops the wrong mon](#1-a-missed-send-out-writes-a-short-mon_order) | Recorder, all gens 2-5 | Fixed: `full_mon_order` fills in unseen mons |
| 2 | [Dex calculator disagrees with the games](#2-the-dex-page-calculator-is-wrong-in-gen-3-and-incomplete) | Dex page | Moot: page removed 2026-11-05 |
| 3 | [Emerald route falls behind the game](#3-emerald-route-ends-268k-exp-behind) | Recorder, gen 3 | Fixed: new Day Care event |
| 4 | [Black route runs ahead of the game](#4-black-route-ends-one-level-ahead) | Recorder or exp, gen 5 | Fixed: four causes in gen 5 exp |
| 5 | [Platinum check fails on the last fight](#5-platinum-check-fails-only-on-the-final-fight) | Corpus check | Fixed: the check read too early |
| 6 | [Prediction miss: Fisherman Travis](#6-prediction-miss-platinum-fisherman-travis) | Send-out rules, gen 4 | Fixed: gen 4 weather |
| 7 | [Prediction miss: Cheren](#7-prediction-miss-black-cheren-404) | Send-out rules, gen 5 | Fixed: stale `battle_end` |
| 8 | [Black Belt Grant order](#8-black-belt-grant-recorded-as-2-1-3) | Recorder, gen 5 | Fixed: a wiped struct is not a faint |

## Results (2026-11-05)

Every game in `run_corpus.py` passes the end-of-run check and `exp_drift.py`,
which fails a run where route and game disagree going into a Rare Candy: 22 candy
uses, none with a gap. White 2 needs the mapper patch below.

| Game | Level | Exp | Prediction |
|---|---|---|---|
| Red | 55 | 212,491 | not run in gen 1 |
| Yellow (5 events) | 6 | 232 | not run in gen 1 |
| Crystal | 84 | 630,987 | 64/66 |
| Emerald | 95 | 876,560 | 45/47 |
| FireRed | 76 | 443,309 | 68/68 |
| HeartGold | 88 | 729,556 | 61/61 |
| Platinum | 91 | 772,958 | 55/55 |
| Black | 71 | 456,740 | 35/35 |
| White | 66 | 289,871 | 36/36 |
| White 2 | 84 | 475,979 | 26/35, see below |

Causes:

- **3 Emerald:** a Day Care stay raised exp and changed moves. New `Day Care`
  route event.
- **4 Black, also White and White 2:** wrong base yields (63 species in BW, 66 in
  B2W2, now from the ROM), exp rounding, an ally in two-trainer fights taking
  half, and an enemy that fell after the solo mon went down paying nothing.
- **5 Platinum, HeartGold:** the route was right. Gen 4 writes the party back
  after the battle, so the check now waits for that.
- **6, 7, 8:** see the status table. Neither miss was a rule bug.

Found while verifying:

- **9. A passing check does not prove a route exact.** A Rare Candy sets exp to
  its level's minimum, which erases any earlier error. The import now also checks
  when the game's exp moves, and `exp_drift.py` reads those lines.
- **10. Gen 4 double battles recorded the wrong `exp_split`.** HeartGold ran
  +2,914 and Platinum +1,018, both hidden by candies. The game keeps one exp mask
  per enemy field slot, and `gen45.rs` now mirrors that. HeartGold's mapper also
  reads a dead address for the second enemy (`opponent_2.active_pokemon`), so the
  recorder reads that enemy from its party instead.
- **11. White 2 drifted for four reasons:** a mutual KO paid exp it should not
  have; triple and rotation battles counted as doubles; White Kyurem read as
  Kyurem; and KO order in tag battles. `gen5.rs` now takes each `exp_split` from
  what the game paid, with the old participant count as a fallback.

Still open:

- **White 2 needs `gen5_mappers_save_counter.patch`.** Tested on a copy only; the
  live mapper repo is unpatched. Without it the last run failed by +121,231,
  before fix 11, and nobody has re-run that since.
- **Not run:** Black 2. Live recording of any of this: everything above is from
  replays. A live feed could merge two quick KOs into one gen 5 payment.
- **Saved routes:** gen 5 exp will recalculate, and the new `Day Care` event
  won't load correctly in older builds. The event has not been opened in the app.
  Gens 2, 4 and 5 may have the same Day Care gap; unchecked.
- **The route can't express:** a gen 5 ally fainting or switching, a won trainer
  fight where the solo mon got nothing for one enemy, a share in a lost fight or
  a wild double, Exp Share, a zero-step Day Care fee, and fights won by another
  mon during a Day Care stay. None caused a gap in these runs.
- **White 2 prediction, 9 of 35:** four are real differences: Elesa and the Rival
  (Volt Switch and U-turn change the order, so the record is right), Charles and
  Denae (triple and rotation; explained, not traced). The other five are tag
  fights, where `mon_order` is now KO order rather than send-out order, so that
  comparison means nothing.
- **Data:** `double_battle` in the B2W2 `trainers.json` is wrong for 28 trainers.
  Exp no longer depends on it, but `send_out_inputs` skips 8 of them for the wrong
  reason. Solodex has the same wrong gen 5 yields.
- **Rules:** Iron Ball is missing from `gen4_super_effective` in both copies; gen 3
  Pass 2 ignores weather; `games/gen4/enemyPanel.ts` in the frontend still assumes
  clear skies.

## Before you start

- Keep heavy work to 8 of 24 cores: `cargo -j 8`, `replay_to_route --cpus 8`.
- Never run `cargo fmt`.
- Use `py -3.14` for Python. `python` on PATH is devkitPro's.
- A rule change in send-out prediction must go in both copies:
  `rust/crates/xpr-data/src/send_out_order.rs` and the unified frontend's
  `A:\Programs\stp-unified-frontend\src\renderer\core\logic\sendOutOrder.ts`.
  The rules and their decomp citations are in that repo's `docs/send-out-order.md`.

### Reproducing the replay results

The 2026-11-04 logs were kept in a session scratch folder and may be gone, so
expect to regenerate them. From `tools/replay_to_route` (see its README):

```
py -3.14 run_corpus.py --exe A:\pkmn_yellow_xp_router\rust\target\release\replay_to_route.exe --work <dir> --only emerald,platinum,black
```

This writes `<dir>/<name>.json` (the route) and `<dir>/logs_<name>/` (the log).
The driver doesn't set `--cpus` itself, so pass `--extra "--cpus 8"` to stay
within the budget. Runs took 7 minutes (Emerald), 32 (Platinum), 20
(Black) and 23 (Crystal).

| Game | Replay |
|---|---|
| Crystal | `c-chikorita-1-20904` |
| Emerald | `e-nosepass-2-35147` |
| Platinum | `p-eevee-1-22928` |
| Black | `b-victini-1-12015` |

Useful log lines:

- `exp check at frame N (M events): route R, game G, difference D`, written as
  the import runs. A battle's exp lands in the route when the event is added,
  which is before the game shows it, so a gap that closes at the next check is
  normal. Only a gap that never closes is a real one.
- `predicted send-out order for <trainer>: [...]`. Compare it with `mon_order`
  in the saved route, treating an empty list as party order.
- `XPR_REPLAY_TRACE=<path>,<path>@<from>-<to>` logs every change to a mapper
  property in a frame range.

Only Crystal, Emerald, Platinum and Black were run. Prediction has not been
replayed on Red, Yellow, FireRed, HeartGold, White or White 2.

## 1. A missed send-out writes a short `mon_order`

**Status:** proven from code, not reproduced in a route. All 682 trainer fights
in the four replay routes have a `mon_order` that is empty or the full team
length, so this needs a missed send-out to appear, such as a stalled live feed.
The replay runs don't test that case.

**Where:** the same block in all four recorders.

- `rust/crates/xpr-recorder/src/games/gen2.rs:1145-1150`
- `rust/crates/xpr-recorder/src/games/gen3.rs:1202-1208`
- `rust/crates/xpr-recorder/src/games/gen45.rs:1797-1804`
- `rust/crates/xpr-recorder/src/games/gen5.rs:1089-1095`
- Reader: `rust/crates/xpr-engine/src/events.rs:1374-1404`

**The bug:** the recorder ranks only the slots it saw. In a three-mon team where
it saw slots 0 and 2, that gives `[1, 2]`. The list has the wrong length, and
its positions no longer line up with party slots:

- `mon_order[definition idx]` is read as the position of that party slot, so
  slot 1, which never came out, gets position 2.
- `get_pokemon_list` stops at `mon_order.len()`, so slot 2, which was fought,
  disappears from the team.
- `recalc.rs:413` builds the fight from that list, so the route should lose that
  mon's exp as well. That follows from the code and has not been tested.

Since send-out prediction went in, this also throws away a good prediction: the
end-of-battle update overwrites it with the short list.

`battle_summary.rs:1614` already falls back to party order on a wrong-length
list, but only for its drag-to-reorder path.

**Suggested fix:** one shared helper that always returns a full-length order:
seen slots in the order seen, then unseen slots in predicted order (party order
if there is no prediction). Use it in all four recorders. Keep the two-trainer
cases as they are.

Routes already saved could carry short lists. Decide whether the reader should
also pad them. That changes how those routes calculate, so check with the owner.

**Done when:** a test per generation feeds a win with a skipped send-out and the
saved team keeps every mon. `gen2_mon_order.rs` and `gen3_mon_order.rs` are the
models. Gens 4 and 5 need their equivalent; `gen5_fsm.rs` already asserts on
`mon_order`.

## 2. The Dex page calculator is wrong in gen 3 and incomplete

**Update 2026-11-05:** the Dex page was removed from the router, so this no
longer applies here. Any fix belongs in Solodex.

**Status:** proven. The Dex page has its own older port from Solodex, separate
from the recorder's rules.

**Where:** `rust/crates/xpr-dex-ui/src/views/trainers/order.rs`, used by
`order_ui.rs:317` and `detail.rs:335`.

| Problem | Evidence |
|---|---|
| Gen 3 counts status moves as super effective. The header comment at `order.rs:23-24` says this matches the games. It holds for gens 2 and 4, not gen 3. | pokeemerald `ModulateDmgByType2`, `src/battle_script_commands.c:1502`, sets the flag only `if (gBattleMoves[move].power ...)` |
| It stops at Gen 3 Pass 2 and Gen 4 Stage 2, the damage-based fallbacks. | `order.rs:15-21`, `is_fallback` |
| No gen 5. | `supported_gen`, `order.rs:138` |
| Curse is treated as Ghost in gens 2-4. The ROMs use `???`. | `send_out_order.rs:919` handles it; `GenEnv::move_type` does not |
| Effectiveness is a multiplier product, not a walk of the ROM-order chart, which matters because the engines truncate entry by entry. Which trainers this changes is unmeasured. | `combined_multiplier`, `order.rs:155` |

**Suggested fix:** decide between two options.

- **Small:** gate gen 3's super-effective check on move power and correct the
  comment. `OrderEnv` will need a way to read a move's power.
- **Full:** drive the page from `xpr_data::send_out_order`. This is a bigger job
  because the page takes a free-form player (types only) and shows per-slot
  evaluations, while `predict` needs the player's stats, ability, item and moves
  and returns only an order.

Which one to take is the owner's call. The Dex page is meant to follow Solodex,
so this may also be a Solodex fix. Read `docs/rust_port/design/dex/PORTING.md`
first.

**Done when:** a gen 3 test shows a candidate whose only super-effective move is
a status move no longer qualifies in Pass 1.

## 3. Emerald route ends 26.8k exp behind

**Status:** cause unknown.

**Result:** level 94 vs game 95, exp 849,769 vs 876,560 (-3.1%). The moves also
differ: the game has Harden, Rest, Mimic, Hidden Power, and the route has Rock
Smash, Rest, Mimic, Harden.

**What the log shows:**

- The route and the game last agree at frame 1108958 (event 512, exp 74,367).
- The route first falls behind at frame 1162830 (event 526, -1,326), and the
  gap never closes after that.
- Between frames 1138497 and 1178772 the log repeats `skipping regeneration of
  move cache, as no valid solo mon is present` (`gen3.rs:728`, 261 times in the
  run). This is the first place to look.
- The gap peaks at -26,791. The final fight adds 19,185 to both sides, so the
  gap is already there before the last fight.
- The run has many resets late on. Each logs `ERROR RECORDING! When removing
  trainer event ...` (`controller.rs:361`) and `Got multiple deleted moves`
  (`gen3.rs:788`), which suggests resets may not roll back cleanly. Rival Steven
  logged it 29 times, Champion Wallace 10.
- `Solo mon gained experience, but we didn't properly cache which enemy mon was
  defeated` appears 115 times (`gen3.rs:1436`).

**Start here:** trace frames 1138497-1162830 and find what gave the game 1,326
exp that the route never recorded. Then find where the route gained Rock Smash
or lost Hidden Power; the `Use/Drop HM06 Rock Smash` lines after each reset are
worth a look.

**Done when:** `run_corpus.py --only emerald` reports `check against the game:
passed`. The check tolerates moves in a different order.

## 4. Black route ends one level ahead

**Status:** Rare Candy is a strong lead, not confirmed.

**Result:** level 72 vs game 71, exp 474,138 vs 456,740 (+3.8%). Moves match.

**What the log shows:** there are two separate effects.

- **Slow drift:** after fights settle, the route is about 1.7% ahead around
  frame 535000 and about 0.9% ahead around frame 1083000. Notes from 2026-10-08
  already recorded gens 4/5 over-counting by 0.5-3%. The gap never reaches zero
  at any check in the run, and it already appears in the first fights.
- **One jump:** the gap goes from +2,572 at frame 1083066 to +16,089 at frame
  1089270, and the only events between them are `Rare Candy, x5` and a skipped
  Double-Edge level-up move. After that it stays around +17,500. The final level
  is also one too high, which fits one candy too many in the route, or a
  mismatch in the exp a candy sets.

**Start here:** the candy burst in the replay, frames 1083066-1091149 (events
`x5`, `x1`, `x2`, `x1`). Compare the bag count and party level before and after
with `XPR_REPLAY_TRACE`. Check the drift separately, starting with the first
trainer fight's exp against `xpr_data::exp::calc_xp_yield_gen5`.

**Done when:** `run_corpus.py --only black` passes.

## 5. Platinum check fails only on the final fight

**Status:** lead. This may be a problem with the check, not the route.

**Result:** level 91 matches, moves match, exp 772,958 vs 757,345 (+15,613).

**What the log shows:** the route and game match exactly at frame 2127841, just
before Cynthia. The whole gap appears when her event is added at frame 2130635.
Recording stops at frame 2148539 as soon as the final trainer is beaten
(`controller.rs:389`, `replay_import.rs:260`). So the check probably reads the
party before the game writes the fight's exp back, but nothing confirms that yet.

**Start here:** read party exp a little after frame 2148539. If it rises to
772,958, delay the check rather than the recording stop, and see whether other
games' final-trainer stops are exposed to the same thing. If it doesn't, the
route's Cynthia fight is over-counting and this is a real bug. There were two
Cynthia events, so check that the retry isn't involved.

**Done when:** `run_corpus.py --only platinum` passes, or the real difference is
explained.

## 6. Prediction miss: Platinum Fisherman Travis

**Status:** cause unknown. The neutral-field assumption is only a suspicion. The
end-of-battle update corrects the order, so the only harm is a wrong order
during the fight.

- **Team:** Barboach 24, Barboach 25, Gyarados 26, Barboach 25 (rom id 93,
  Route 212, Basic AI).
- **Predicted:** `[1, 3, 2, 4]`, so Gyarados second. **Game:** party order.
- **Where:** the replay around frame 1175016, event 333. The step is Stage 2,
  after a real faint.
- **Rule:** pokeplatinum `BattleAI_PostKOSwitchIn`,
  `src/battle/battle_lib.c:7923`, and `next_gen4` in `send_out_order.rs`.

Stage 2 reads the player's stat stages, screens and weather, and the prediction
assumes none (`send-out-order.md:85`, `:114`). The player's species and moves in
this fight weren't captured.

**Start here:** dump the battle state from the replay when the lead faints, then
work Stage 2 by hand from the decomp with those values. If the neutral values
explain it, this is a known limit and needs only a note. If not, it's a rule bug
to fix in both copies.

## 7. Prediction miss: Black Cheren (404)

**Status:** cause unknown. Corrected at the end of the battle, like issue 6.

- **Team:** Pidove 20, Liepard 20, Pansear 20, Dewott 22 (Route 4).
- **Player:** the run's solo mon, Victini.
- **Predicted:** `[1, 4, 3, 2]`, so Dewott second. **Game:** party order.
- **Where:** the replay at frames 349414-359331, events 99-101.
- **Rule:** `predict_gen5`, read from B2W2 (`fuddlesworth/pokebw2`,
  `src/ov167/btl_client.c:3654-3720, 3814-3869`).

Black/White 1 rules are inferred, as `send-out-order.md:105` and `:153` say. The
other 15 reordered Black fights matched, so the rule isn't simply wrong for
BW1. `send-out-order.md:155` also leaves open whether the mapper's party list
follows the battle's order.

**Start here:** work the fight by hand with the B2W2 rule and Victini's moves at
that point. If the rule gives Dewott, look for a difference in BW1. There is no
BW1 decomp, so a second real fight that exercises the same branch is the
practical test. The `b2w2-decomp` repo doesn't build yet.

## 8. Black Belt Grant recorded as `[2, 1, 3]`

**Status:** lead, low priority. The two Cubchoo in slots 1 and 2 are identical
(trainer 215), so this fight's result is unaffected.

The log lists `opponent slot 2 fainted:  Lv0` before `trainer battle: id 215`,
and `battle over` then gives `fainted ["2: Lv0", "1:Cubchoo Lv35", ...]`. The
same run has six `unknown wild Pokémon ""` errors (`gen5.rs:974`). A faint seems
to be read before the battle is set up, in `gen5.rs` around line 958. On a team
with different mons, that would record a wrong order.

**Done when:** a faint seen before the trainer id is known no longer enters the
order.

## Not bugs

These prediction mismatches are expected and need no work:

- **Crystal, Brock and Hiker Russell:** when every candidate is bad, gen 2 picks
  at random.
- **Emerald, Tate & Liza and Maxie 3:** double and two-trainer battles stay in
  party order on purpose.

Owner decisions, not defects:

- Manually added trainer events still start in party order. Prediction runs only
  while recording.
- The rules exist in two places, the frontend and the router.
