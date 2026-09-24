# Reference calculators (2026-09-23 verification)

Transcriptions of the games' damage routines (`ref_calcs.py`: gen 1 pokeyellow
`CalculateDamage`, gen 2 pokecrystal `BattleCommand_DamageCalc`/`Stab`, gen 3
pokeemerald `CalculateBaseDamage`/`Cmd_typecalc`, gen 4 pokeplatinum
`BattleSystem_CalcMoveDamage`/`ApplyTypeChart`) and the scripts that sweep the
Rust calculator against them.

Setup: clone the pret repos (pokered, pokeyellow, pokegold, pokecrystal,
pokeruby, pokeemerald, pokefirered, pokediamond, pokeplatinum, pokeheartgold)
into one directory and point `XPR_DECOMPS` at it (default `~/decomps`); build
the harness with `cargo build --release -p xpr-calc --example sweep`.

```
S=rust/target/release/examples/sweep
for v in Yellow Crystal Gold Emerald Platinum; do $S --options $v > options_$v.json; done
python3 gen_cases.py            # writes cases_<version>.json
python3 compare.py               # runs the harness and diffs (0 mismatches expected)
python3 gen_tests.py             # regenerates rust/crates/xpr-calc/tests/game_formulas.rs
```

`XPR_SWEEP_OUT` chooses where the generated case / result files go (default:
this directory; they are not meant to be committed).
