# v0.8.0 Rust backend migration

Status: active; the published v0.7.5 baseline is integrated, M0 and M1 offline
slices verified on 2026-09-15, no product cutover.
Decision owner: Astra primary agent under the project owner's direction.
Implementation: bounded DeepSeek V4.1 Flash assignments, reviewed by the primary.

## Baseline and purpose

- Published behavior: v0.7.5 product commit
  `533694ebad21906aecbb6ab5283e04e760ce6c09`, which the owner manually
  confirmed for the NG3 search-continuation and window/taskbar icon fixes. The
  earlier v0.7.4 commit `df438ed3a9a1b92e68b3c77da0b1c094e0663327` is the
  historical predecessor.
- Starting checkout: `a1601bb242ebfdc509ab853016cc64639f69320c`, which adds the
  v0.7.4 publication record. Work branch: `codex/v080-rust-backend`.
- The published v0.7.5 source arrived through integration merge `263f46d`, which
  combines the M0/M1 checkpoint `7dba4ca` with `d847cff` (containing `533694e`).
- Existing uncommitted research-ledger edits and untracked work are outside this
  slice and remain uncommitted in the working tree.
- Preserve the current React/WebView2 product and one-file delivery contract while
  replacing backend components incrementally. Equipment generation and editing
  require separate research and are not promised by this migration.

The baseline is shipped behavior, accepted native fixtures, and operation
contracts. A model's output is a proposed implementation, not a new oracle.
Use [current handoff](CURRENT_HANDOFF.md) for acceptance boundaries and
[feature catalog](../product/FEATURES.md) for user-visible behavior.

## Architecture

Current execution path:

`React -> Tauri desktop_request -> Rust broker -> framed Python workers`

The broker already owns process lifecycle, private candidate transfer, schema
checks, storage, and updates. Retain this boundary during migration. Introduce
Rust components in this dependency order:

1. **Domain library** (`crates/nioh3-domain`): deterministic math and typed
   records. No filesystem, process, UI, research-script, or transport dependency.
2. **Read-only application worker**: catalog loading, preview, search, cursors,
   cancellation, and context binding. Keep search in a killable process;
   calling pure Rust from the UI event loop is not the target architecture.
3. **Protected operation worker**: save and runtime application services with
   explicit ownership, backup/readback, uncertain outcomes, and recovery.
4. **Platform adapters**: narrow Windows process/memory and native helper
   interfaces. Port each adapter only with its matching fault and game evidence.

Do not create all these crates before they contain working functionality.
The first crate is standalone and does not change the shipped host's Cargo
dependency graph. JSON worker schemas stay the compatibility boundary until a
reviewed slice explicitly replaces them. C++/CUDA numerical helpers need their
own parity/performance decision; language uniformity alone cannot establish a
correct or faster replacement.

## Migration sequence and exit criteria

| Stage | Work | Required result before advancing |
| --- | --- | --- |
| M0 | Record contracts and add the domain crate with enemy-state RNG | Cross-language values, final states, draw/rejection counts and numerical edge cases agree |
| M1 | Port enemy variants, eligible Wraith selection and native-table loading | Exact ordered occurrences and state agree, including `86872488` solo/expedition and `156062997` solo controls; unsupported data remains rejected |
| M2 | Port scroll records, context, effect/auxiliary generation, R4 finalization and read-only worker requests | Existing native byte fixtures and full preview payloads agree; search/resume/cancel and candidate identities survive process boundaries |
| M3 | Replace save services and native runtime adapters in bounded steps | Backup, account/slot identity, rollback races, uncertainty and no-replay behavior pass; relevant live acceptance precedes product cutover |
| M4 | Remove superseded runtime dependencies and consolidate build/tests | One-file launch/update, locales, collections, editor and recovery work in the candidate package; product runtime no longer depends on research tooling |

Each assignment names owned paths, input/reference data, public interfaces,
verification commands, and an explicit completion boundary. DeepSeek implements
the slice and returns evidence. Astra reviews semantics and decides integration
and the next assignment. Start workers with a focused task rather than the full
historical conversation. Keep model context defaults until a measured task need
justifies an override.

## M0 numerical contract

Reference: `nioh3_scroll_editor/enemy_state_rng.py` at the starting checkout,
SHA-256 `ca696a7f6dee4c1ae4a3d336daa76ba5a1ac0a60e0c56f1a2f51b0e8987c0dd4`.

- LCG: wrapping uint32 multiply by 69069 then add 1; expose high 16 bits and
  exact draw count. Affine jumps preserve the same state.
- MT19937: preserve in-place refill and draw order across 624-word boundaries.
- Inclusive sampling: zero bound consumes no draw; rejected samples increment
  both draw and rejection accounting; uint32 maximum is a direct draw.
- Shuffle: forward Fisher-Yates and exact stream consumption.
- Lottery/config threshold: preserve binary32 intermediates and x86-style
  invalid float-to-int sentinel (`i32::MIN`), not Rust's saturating cast alone.
- The Rust stream owns numerical state only. Python's per-draw dictionaries
  are reference instrumentation; observation belongs outside the hot RNG path.

The Rust example is a development parity emitter, not a product CLI. The Python
gate compares its output against the retained product implementation, covering
all 65,536 lottery inputs, six seeds, multiple MT refills, large affine jumps,
rejection-heavy bounds, and complete shuffled order. Rust unit tests separately
pin known sequence and conversion/row boundaries. This establishes bounded
algorithm parity; it does not establish new game captures or end-to-end preview
equivalence. Product wiring remains a later explicit step.

## Behavior gates and test policy

| Affected contract | Preserve | Evidence anchor |
| --- | --- | --- |
| `SCROLL-SEARCH`, `SCROLL-PREVIEW` | RNG state, exact fields, NG3 ordinary-solo/eligible-Wraith UI, R4 preview/stage-one pairing | `test_effect_sequence.py`, `test_enemy_state_product.py`, `test_r4_finalizer_engine.py` |
| Private candidate ownership | Context digest, cache validity, request schema, framed transport, ordered resume/cancel | `test_frontend_v2.py`, `test_v2_operations.py` |
| `SCROLL-COLLECTIONS` | Existing favorite/cart identity and exact selected subset | Existing desktop collection and protected-operation tests; packaged flows |
| `SCROLL-ADD/EDIT/COUNT/DELETE`, `MISSION-OVERRIDE`, `SAVE-BACKUP` | Backup, account/slot ownership, readback, rollback and no write replay | `test_backend_freeze.py`, `test_v072_save_races.py`, `test_v073_restore_races.py` |
| `APP-UPDATE`, `LOCALIZATION`, `SUPPORT-DIAGNOSTICS` | Existing user flows and one-file contract | Existing Tauri/launcher tests and packaged WebView2 acceptance |

Run the affected slice's behavioral tests on each implementation change. Run
the broader integration/package gates at cutover. Retain research and legacy
Tk tests for their own supported workflows, without translating all of them
into Rust. Source-string, layout-constant and historical research tests are
not evidence of numerical or protected-operation parity. Remove/merge a test
only after identifying its protected behavior and replacement coverage.

Initial CI review found Rust host/launcher tests in the release workflow but
no equivalent Rust push/PR gate. TypeScript tests already run in the
path-filtered `frontend-v2.yml`; they are not absent. Add the new domain gate
to push/PR CI before growing this crate.

## Dependency findings requiring precision

The bounded import search found no production import of
`research/find_runtime_*_xrefs.py`; tests and research tools consume those files.
The old Electron packager copies research Lua resources. This does not by
itself establish a dependency in the current Tauri package. The v2.01 profile
contains an `audit/` provenance path, but `load_native_runtime_profile` does not
open that manifest. Treat it as metadata, not a demonstrated dangling runtime
read. M2/M4 must inspect actual runtime/resource consumers before moving code
or removing provenance.

## Verification

M0 completed locally with Cargo/Rust 1.91.0 and the repository-selected Python
environment `.codex_tmp/v2-build-env/Scripts/python.exe`:

- Eight Rust unit tests passed; formatting and Clippy checks passed.
- Six cross-language tests passed against the optimized release emitter:
  84,016 rows across eight operation groups, with no mismatches. This includes
  12 float-conversion and 84 threshold cases using exact IEEE bit patterns.
- Retained enemy-state product tests: 62 passed, 3 optional tests skipped.
- Documentation audit passed. The new domain/parity push/PR CI step is prepared
  locally; no hosted run has occurred.

The numerical reference file is unchanged. The primary review removed a
redundant protocol/fixture framework and test-helper roundtrip test, retained
typed numerical interfaces, and verified the final implementation. Local Rust
incremental-cache hardlink attempts fell back to copying; both checks exited
successfully. This is a filesystem cache warning, not a numerical discrepancy.

Repeat the gate through the project Python selector:

```powershell
cargo test --locked --offline --manifest-path crates/nioh3-domain/Cargo.toml
./tools/run_python_tests.ps1 -TestPath tests/migration/test_rng_parity.py
```

No product version bump, package, publication or new live-game acceptance is
implied by this checkpoint. Track completion in
[v0.8.0 engineering record](../product/releases/v0.8.0.md).

## M1 offline enemy-generation slice

The standard-library-only domain crate now includes `rng`, `enemy`, `context`,
`roster`, `wraith`, and `preview`. The preview entry rejects unsupported
non-zero selectors; low-level explicit rosters remain research-only. The
`nioh3-data` adapter reads shipped auxiliary v3/R4 v1/state resources and
verifies binary manifest sizes/hashes and path containment, plus state-capture
identity, observed lookup, and row schema. Duplicate normalized keys/declarations are
rejected; state JSON has no sidecar digest. There is no runtime research
dependency and no Tauri/frontend/product wiring.

Primary verification: Cargo passed 28 domain and 7 data unit tests. All 13
migration Python checks (6 M0 + 7 M1) passed through
`tools/run_python_tests.ps1` using `.codex_tmp/v2-build-env/Scripts/python.exe`.
The optimized Rust example matches
untouched Python for 1,340 seed/progression/variant cases across three branches,
seven modes, 20 terrains, both variants, and pp1..5, including ordered full
occurrence fields, RNG states/draws, and Wraith trials; non-zero selectors are
rejected. Existing native fixtures independently check `86872488` solo (6 enemies,
no Wraith), `86872488` expedition (10 enemies, Wraith `0xF3F`), and `156062997` solo (6 enemies,
(Wraith `0xF40`). Incomplete eligibility captures remain unknown instead of
becoming negative results. The original 84,016 TSV rows still pass.
Final `cargo fmt --check` and Clippy (`-D warnings`) pass for both crates. Combined
retained enemy-state and migration checks report 97 passed, 3 skipped through
the project selector. No fresh live-game, UI, or package acceptance is claimed.
