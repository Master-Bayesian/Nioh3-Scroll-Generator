# PC v2.02 armor remodel research tools

See the [research result](../../docs/research/V202_ARMOR_REMODEL_BATCH_20261008.md)
for formulas, evidence counts, and limits.

## Offline use

From a checkout with the shipped item resource and name catalog:

```powershell
./tools/run_python_tests.ps1 -Python <prepared-python> -ScriptPath tools/armor_remodel_batch_v202.py -ScriptArgument @('--output','<new-delivery-directory>')
```

Use a new output directory. On the owner's host, delivery/build/temp paths
must resolve under the project build root on D:. The offline calculator uses
the public evidence by default; no private capture directory or game is
required. It exports UTF-8 JSON, Excel-readable UTF-8 BOM CSV, and validation
JSON. Existing output files are refused.

Supported options:

- `--rarity 0..5`, `--level 0..65535`, repeatable `--plus 0..65535`.
- `--stage 1..5`, `--flags 0x...`, repeatable `--item-id 0x...`.
- `--include-internal` to include alternate target rows.
- `--names PATH` for the exact supported PC v2.01 catalog schema.
- `--evidence-root PATH` for reviewed original capture inputs when available;
  the private 1,190-return corpus remains a separate evidence mode.

The defaults are rarity 4, level 180, +20, stage 3, flags 0. Levels and flags
are scenario inputs, not inferred live equipment state. All candidates are
parameter IDs; the exporter does not establish legal or obtainable gear.

## Reviewed public inputs

- `public-evidence.json`: identity, file hashes, counts, and validation scope.
- `batch-dependencies/progression-full.json`: all 502 coefficient rows.
- `batch-dependencies/toughness-cold-table.json`: three numeric rows and the
  seven-slot signed-byte lookup map.
- `native-returns.json`: all 1,352 scalar native returns, grouped by capture
  method, with raw source and cleanup SHA-256 references.
- `owner-acceptance.json`: the scoped owner report of manual agreement.

These files contain no process IDs, live pointers, user paths, inventory
serials, save/account data, or full executable/section dumps. Original raw
captures remain private. Hash links preserve provenance; public cleanup
summaries do not replace those raw private proofs.

## Live research boundaries

The passive observers in `research/armor_remodel_*_observer_ce.lua` require a
runner-supplied verified executable/process identity and bounded controls.
They install only owned breakpoints and include cleanup/timer checks. Their
mock gates do not attach to a game.

`research/armor_cross_series_query_ce.lua` returns an API when loaded and
does not allocate or call game code until `api.run(config)` is invoked.
The caller must obtain explicit owner authorization for native calls and
temporary target-memory writes. This is an adapter for an approved runner,
not an unattended startup script.

The runner contract requires:

1. Fresh PID, creation FILETIME, module base, pinned executable SHA-256, and
   expected process age; stable parameter-manager, item/curve, map, and stage
   pointer chains.
2. Reviewed full byte ranges for the three getters and eight helpers:
   RVAs 0x818018, 0x817FD4, 0x2F9E30, 0x8180AC, 0x2F9E88, 0x2F9EEC,
   0x2FB078, 0x2F9F34, 0x1111850, 0xF4DBC, 0xC624A8.
3. Exactly 18 unique vectors: IDs 0xC18E, 0x35BC, 0xC288 in all six modes;
   original/selected row indices and their full 416-byte readbacks; expected
   weight, toughness, and seven requirements.
4. A running debugger or no active debugger, with no foreign breakpoints.
   Set `authorized_native_queries=true` only after owner approval.

The adapter writes one owned 272-byte allocation: a 240-byte temporary test
record and two 16-byte guards. CE's
[executeCodeEx interface](https://wiki.cheatengine.org/index.php?title=Lua:executeCodeEx)
provides the call wrapper/thread. It does not write inventory records,
patch original code, or write saves. The pass is bounded to 162 calls, one
second per call, 60 seconds total, and 256 KiB of reads.

A completed error stops further calls and releases the owned record.
An unconfirmed native completion or changed attachment retains it and
blocks retry. Never free an allocation potentially still in use, reset that
state to bypass the guard, or replay an unknown operation. Independently
verify process lifetime, allocation release, unchanged function/table bytes,
and the query state after any completed pass.

The Rust example `armor_remodel_snapshot` is separate read-only inventory
inspection. It never calls getters or writes target memory:

```text
armor_remodel_snapshot --item-id 0xC18E --max-records 16 --out <private-capture-path>
```

Build it through the project shared Cargo target, resolved by
`tests/migration/cargo_target.py`; capture outputs remain private.

## Tests

```powershell
./tools/run_python_tests.ps1 -Python <prepared-python> -TestPath tests/test_armor_remodel_batch_v202.py,tests/test_armor_cross_series_query_ce.py,tests/test_armor_remodel_field_xrefs_v202.py,tests/test_armor_remodel_weight_observer_ce.py,tests/test_armor_remodel_stats_observer_ce.py,tests/test_armor_remodel_extended_observer_ce.py
```

The offline calculator is standard-library Python. Observer/query mocks use
the repository Lua test runtime / Lupa; xref tests use Capstone. Tests use
portable synthetic identities and the committed numerical corpus. No test
invokes a live getter.
