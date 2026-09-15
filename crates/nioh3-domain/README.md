# nioh3-domain

Standard-library-only Rust port of deterministic Nioh 3 domain semantics. The
first slices cover PC v2.01 enemy-state RNG, typed enemy/roster/context records,
Wraith trial diagnostics, and seed-level preview generation.

## Layout

- `src/rng.rs` - parent LCG, MT19937, binary32 rounding and `cvtt` truncation,
  `lottery_10000`, the 0x20-byte configuration-row threshold, forward
  `native_shuffle` (generic over the element type), `affine`, and `state_after`.
- `examples/rng_vectors.rs` - TSV emitter for the cross-language parity gate.
  It owns the configuration-row builder because that exists only to manufacture
  vectors; the library reads captured rows.
- `src/{enemy,context,roster,wraith,preview}.rs` - typed generation records,
  context binding, rosters, Wraith trials, and seed-level preview.

`preview::generate_enemy_preview` is the seed-level library entry point. It
rejects unsupported non-zero selectors; explicit roster construction remains
available for research and is not product wiring.

The sibling `nioh3-data` crate loads the shipped native resources and provides
the development-only `enemy_preview` batch example used by the parity gate.

## Commands

```powershell
$env:CARGO_TARGET_DIR = 'F:\Nioh3_ScrollEditor\.codex_tmp\v080-domain-target'
cargo test --locked --offline --manifest-path crates/nioh3-domain/Cargo.toml
cargo test --locked --offline --manifest-path crates/nioh3-data/Cargo.toml
./tools/run_python_tests.ps1 -TestPath tests/migration
```

## Boundaries

- Numeric semantics only: no file, process, memory, network, or game access,
  and no third-party dependencies.
- RNG event bookkeeping remains out of contract. The Python reference records
  `LcgStream.events` as evidence bookkeeping and no returned value depends on
  them, so this port does not model them.
- `None` and wrong-length threshold rows stay Rust unit tests; the emitter only
  produces well-formed 0x20-byte rows.
- A green parity run is bounded evidence for the ported operations only. It is
  not game, save, persistence, or UI acceptance.
- Nothing here is wired into product entry points, the Tauri broker, or the
  packaged workers.
