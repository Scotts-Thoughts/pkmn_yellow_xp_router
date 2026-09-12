# Golden corpus: Python reference vs. Rust port

The Python app (branch `ui`) is the reference implementation. The scripts in
this folder run it headlessly and write JSON records; the Rust workspace
replays the same inputs and diffs its results against them.

## Generating records (Python)

```
py -3.14 docs/rust_port/golden/dump.py OUT_DIR --tests                 # the 4 routes in tests/test_data
py -3.14 docs/rust_port/golden/dump.py OUT_DIR --data-dir              # every saved + outdated route of the configured data dir
py -3.14 docs/rust_port/golden/dump.py OUT_DIR --battles --data-dir    # ... plus the battle summary of every trainer / wild fight
py -3.14 docs/rust_port/golden/dump.py OUT_DIR path/to/route.json ...  # explicit files or folders
```

One `<route name>.golden.json` per route: the bytes `Router.save` would
write, the notes export, the final state, and one entry per event group
(rendered name/label, row values, tags, error messages, init/final states,
items, level-up move definitions). With `--battles` each trainer/wild fight
also carries the `BattleSummaryController` output under the user's config
(`battles.py`).

Damage-calculation cases (every `calculate_damage` / crit-rate / accuracy call
made while the pytest suite runs) are recorded with:

```
py -3.14 docs/rust_port/golden/record_damage_cases.py            # writes docs/rust_port/golden/damage_cases.json
```

## Verifying (Rust)

```
cd rust
cargo run --release -p xpr-golden -- verify OUT_DIR                # all records, in parallel
cargo run --release -p xpr-golden -- verify OUT_DIR --limit 200    # the first N records
cargo run --release -p xpr-golden -- verify OUT_DIR --filter crystal --max-diffs 5
cargo run --release -p xpr-golden -- verify OUT_DIR --no-battles   # skip the battle summaries
cargo run --release -p xpr-golden -- dump RUST_OUT route.json ...  # the Rust side of the same records
cargo run --release -p xpr-golden -- bench route.json              # load / recalc / save timings
cargo test -p xpr-calc --test damage_cases                         # replays damage_cases.json
```

`verify` exits non-zero and prints the first differing fields when a record
does not match byte-for-byte (saved route), string-for-string (notes,
labels, values) or value-for-value (states, battle summaries).

## Notes

- The Rust side needs the same data the Python side used: the built-in
  `raw_pkmn_data/` of this checkout plus the same custom gens folder
  (`user_data_location/custom_gens`), because custom-gen routes reference it.
- Battle records depend on the config keys `damage_search_depth`,
  `force_full_search`, `ignore_accuracy_in_damage_calcs`,
  `consistent_highlight_threshold`, the two highlight strategies and
  `test_moves_enabled`; dump and verify must run against the same
  `config.json`.
- Known, intentional differences are listed in `docs/rust_port/KNOWN_ISSUES.md`.
