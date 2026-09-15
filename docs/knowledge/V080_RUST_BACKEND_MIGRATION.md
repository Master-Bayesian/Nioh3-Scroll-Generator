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

## M2.1 offline effect-sequence slice

The ordinary NG3 effect-sequence path is ported behind the same offline parity
policy: `crates/nioh3-domain` now carries `effect`, `record` and `sequence`
modules plus a development vector emitter, `crates/nioh3-data` carries the
effect-resource adapter and its own emitters, and `tests/migration` carries the
sequence parity gate and the permanent adapter parity gate. Nothing in the
ships-product path changed: no Tauri, broker, `packages/contracts`,
`search_jobs.py`, `search_worker.py` or `worker_contracts.py` edit.

Reference identity read at `d81a532`:
`effect_sequence.py` `83CABD7B237C627E22CE3AD6ECA4D4BB9A1726615A73D2A943C91AE59F1AE303`,
`effect_generation_tables.py` `6DE6116D7013A567567CFDA2A5ADD08475A137576DB9CE7F355E813EDB6B4EBC`,
`r4_finalizer_reference.py` `4E9B0E4D545A6FEC4F63D63A9C8D194594BAE85470FF972D76F3AA3721924F84`.

Verified locally and committed as `4906583` under owner approval:

- Sequence parity: 549 emitted rows with zero differences between the Rust
  adapter path and the retained Python reference - 546 generations and 3
  capacity rows. The generation set is 110 seeds at level 180 across `r3`,
  `r4_stage_one` and `r5` (14 hand-picked boundary seeds plus 96 stride seeds),
  plus a 216-row cross-level sweep (levels 1, 30, 90, 150, 180, 300, 500, 700
  and the `u16` maximum 65535, eight seeds, three paths). Compared per row: slot
  order and `source_index`, per-slot rolls, resolved values including the R4
  stage-one and R5 terminal values, `candidate_count`, promoted source indexes,
  draw count and final LCG state. Both promotion branches appear in the sweep.
- Level scaling: the sweep is measured, not assumed, to be non-vacuous - 5 of
  the 24 level groups change resolved values, producing 7 distinct resolved
  tuples, and the gate pins that distribution so a table or reference change
  forces a re-measure. Levels above the 500-row curve clamp (700 and 65535) keep
  their verbatim level column and reproduce the level-500 effect block, asserted
  explicitly. Only level and resolved values move with level: identifiers, slot
  order, rolls, category/flags, candidate counts, prefixes, promotions, draw
  counts and final state are level-invariant by test.
- Native anchors: three retained byte fixtures (r5 seed 1, r5 seed 241719428,
  r3 seed 6096970) are reproduced from their record bytes, not from the
  generator alone.
- Adapter parity: a permanent gate runs the production
  `nioh3_data::load_effect_resource` path and compares per-table row digests
  against the shipped files; an explicit swap guard keeps `effect` (0xD8 x 3609)
  and `level_curve` (10 x 501) content-distinct, and a mutation that swaps the
  two struct fields fails with `216 != 10`.
- Gates: domain crate 48 passed, data crate 12 passed, `tests/migration` 30
  passed, retained `test_effect_sequence.py` / `test_r4_finalizer_engine.py` /
  `test_effect_path_inverse.py` 34 passed, `cargo fmt --check` and
  `clippy --all-targets -D warnings` clean for both crates, all through the
  repository Python selector with `CARGO_TARGET_DIR` outside the checkout.

Correction to an earlier planning note: the seed-1 vector
A051/D40A/34F3/3E7A/AE5A/6553 (rolls 94/91/94/96/91/0, 24 draws, state
`0x2FAC1E69`) is the **rarity-5** row. Seed-1 `r4_stage_one` is
B613/4647/D411/3F41/6553 (rolls 88/82/88/92/0, 21 draws, state `0xF18AF3AA`).
The fixture classification is authoritative.

Not covered by M2.1: `GenerationContext` digest binding, candidate
hashing/identity, the R4 finalized-preview/stage-one dual record and the R4
finalizer, weighted search-side primary selection, worker/process cutover,
product wiring, and packaged or live-game acceptance. NG4/NG5 generation fails
closed because no captured Grace map exists for those record types.

The earlier reviewer finding that every row and native anchor used level 180 is
closed by the cross-level sweep above; no other limitation remains open for this
slice.

## M2.2 offline R4 finalization and paired records

The R4 completion finalizer and its paired records are ported behind the same
offline parity policy: `crates/nioh3-domain/src/r4_finalizer.rs` with the
`record`/`effect` changes, `crates/nioh3-data/examples/r4_finalizer_vectors.rs`
for the production-adapter emitter, and
`tests/migration/test_r4_finalizer_parity.py` for the tracked gate. No product
wiring and no write path exist in this slice.

Reference identity: `r4_finalizer_engine.py`
`5096ABB817944EE805105AC2E9EEB6A2DF609CDD948F528855FA75DBBF32E3E7`,
`r4_finalizer_reference.py`
`4E9B0E4D545A6FEC4F63D63A9C8D194594BAE85470FF972D76F3AA3721924F84`,
`effect_sequence.py`
`83CABD7B237C627E22CE3AD6ECA4D4BB9A1726615A73D2A943C91AE59F1AE303`, retained
anchor `tests/test_r4_finalizer_engine.py`
`DD9248205261BFC41B3183D92A87FAF69B632A8892EC75C4C94A6356625512E2`. The
Python materializers `materialize_ng3_rarity4_stage_one_record` and
`materialize_ng3_rarity4_final_record` are the shape the Rust pair reproduces.

- Native byte oracle: `test_fixtures/r4_native_corpus/{base,distributed}` holds
  ten stage/final pairs (nine unique Seeds; Seed 1 appears in both corpora), 232
  bytes each, record type `0xE604`, level 180. The tracked pairs come from
  `research/validate_ng3_rarity4_native_parity_live.py`, which calls
  signature-gated native stage generation and completion finalization in isolated
  remote buffers and never reads or writes a save. Tracked copies are sanitized:
  they differ from the private captures only in the eight origin-account bytes
  (`0x02..0x05`, `0x14..0x17`), which the gate proves belong to no effect slot
  and are reproduced verbatim.
- `test_native_corpus_pairs_are_byte_exact` compares all 232 bytes of both
  records through the real `nioh3_data::load_effect_resource` route: 4,640 bytes
  equal (10 pairs x 2 records x 232 bytes).
- Pairing audit independent of the generator: two pairs are byte-identical
  because no-change finalization is valid, and eight pairs differ in exactly
  eight bytes confined to a single effect slot (0-based slots 1..4, absolute
  `0x4C`/`0x64`/`0x7C`/`0x94`); nothing outside that slot changes.
- Stage-one preservation: `Rarity4RecordPair` owns the install and preview
  records separately, so they cannot alias; for every pair the install record
  equals the tracked stage bytes while the preview equals the tracked final
  bytes, and a second emitter run reproduces every row.
- Reveal branch: `reveal` true/false is emitted and compared (12 rows) and is
  measured reachable and outcome-relevant - the resolved weight slot shifts by
  one (`0x3C`/`0x3B` versus `0x3E`/`0x3D` for `field_15c = 0x3B` on `0xE604`),
  and `reveal = false` changes the accepted slot and final bytes for three of six
  probes.
- Level sensitivity, corrected: an earlier "nine distinct final records"
  statement was metadata-only because the header follows the level. The gate now
  measures the effect area (`0x34..0xDC`) and per-slot resolved values over seven
  seeds and nine levels: two seeds move the stage-only effect area with seven
  distinct resolved tuples each, one moves the finalized preview's effect area,
  and the accepted slot is level-invariant for every seed. The single sensitive
  seed comes from prior-row eligibility reading a resolved value, so crossing
  that value changes later pool sizes; the finalizer RNG seed itself is
  level-independent. Outside the effect area only the level bytes `0x06..0x09`
  move, and inside only the resolved-value bytes `+0x08..+0x0B`.
- Reference-only coverage: 136 rows (61 seed-sweep at level 180, 63 level rows
  over nine levels and seven seeds, 12 reveal rows) plus 6 rejection rows, with
  the emitter rows recorded in `deliverables/m22-r4/evidence/emitter_rows.tsv`.
- Negatives: truncated and oversized records (`RecordLength`), wrong record type
  (`UnsupportedRecordType`), wrong rarity (`UnsupportedRarity`), unsupported
  template context (`TemplateRecordType`), out-of-range promotion target
  (`InvalidTargetIndex`), plus adapter-level missing-table, wrong-context,
  non-dense-partition, altered-digest, duplicate-declaration and escaping-path
  guards.
- Gates: domain crate 58, data crate 14, `tests/migration` 45, retained R4 and
  sequence anchors 41, `cargo fmt --check` and `clippy --all-targets -D warnings`
  clean for both crates.

Bounded limits for this slice: no native capture exercises `reveal = false` or
any level other than 180, so native level coverage stays an explicit bounded
limit rather than a claim; the cross-level evidence is reference parity only.
Playthroughs other than 3 and record types other than `0xE604` are rejected, not
covered. `GenerationContext` digest binding, candidate identity/hashing, the
read-only worker cutover, product wiring, and packaged or live acceptance remain
out of scope, and the slice exposes no write path.

## M2.3a development read-only worker and preview parity

M2.3a adds a development-only Rust worker process that serves the read-only
preview subset over the shipped framed-JSON protocol. It reuses the existing
crates rather than introducing a new preview crate:
`crates/nioh3-domain/src/auxiliary.rs` plus the extended `preview.rs`, `wraith.rs`
and `enemy.rs`, `crates/nioh3-data::preview_resource::load_preview_resources`,
and the new `crates/nioh3-worker` crate (`context`, `engine`, `model`, `native`,
`payload`, `protocol`, `transport`, `main`). Its dependency graph is
`nioh3-worker -> nioh3-data -> nioh3-domain` plus `serde_json` and `sha2`; no
process, IPC or Python dependency exists, and the only native input is the seed
accelerator ABI/build identity.

- Supported methods: `handshake`, `candidate.preview`, `shutdown`. The eight
  other protocol methods (`search.catalog`, `recommended_level.resolve`,
  `search.start`, `cache.register`, `candidate.export`, `job.snapshot`,
  `job.current`, `job.cancel`) return `UNSUPPORTED_METHOD`; a completely unknown
  method fails the versioned request schema and returns `INVALID_REQUEST`.
  Search orchestration - continuous search, resume and cancel - is M2.3b.
- Bounded development deviation: the three supported methods keep full strict
  parameter validation and the shipped error framing, but a known-but-unimplemented
  method short-circuits to `UNSUPPORTED_METHOD` before parameter validation, so
  malformed params on those methods answer `UNSUPPORTED_METHOD` instead of the
  Python worker's `INVALID_REQUEST`. This is a development-only partial-worker
  limitation with no production compatibility claim; M2.3b must restore full
  validation for every method it implements. The deviation is pinned by a
  permanent assertion in the subprocess gate rather than left to prose.
- Production selection is structurally impossible: the binary requires an
  explicit `--dev-preview-only` acknowledgement and otherwise writes a refusal
  and exits non-zero before serving any frame.
- Identity: `contract_digest` is the SHA-256 of the two shipped schema files read
  byte-for-byte, and the eight context fields (including `resources_digest`,
  `algorithm_version`, `policy_version` and `context_digest`) equal the Python
  worker's values.
- Capabilities disclose the reduced subset honestly (`playthroughs: [3]`,
  `rarities: [3, 4, 5]`, `cpu_exact_replay: true`, GPU flags false,
  `save_write`/`runtime_calls` false).
- Subprocess parity: 189 previews (21 deterministic seeds x rarities 3/4/5 x
  levels 1/90/180) match the Python worker field for field after normalising only
  run identifiers and timestamps, covering the 15 required payload fields, the
  whole `transfer` block and all eight effect fields. The matrix is measured to
  be non-vacuous: it exercises terrain display keys, special rules, enemy groups,
  challenge capacity, at least one Wraith occurrence, at least one
  expedition-only occurrence, and all three rarities.
- Independence evidence (bounded interpreter-independence and resource checks,
  all runtime): with `PATH` isolated to an empty directory and `PYTHONHOME`,
  `PYTHONPATH`, `NIOH3_PYTHON` and `NIOH3_SEED_ACCELERATOR` cleared, the Rust
  worker still handshakes and matches the oracle produced by a separate Python
  process for unusual seeds and rarity/level combinations; an empty `--data-root`
  fails closed instead of returning the shipped payload, and a mutated
  `--contract-dir` changes the reported `contract_digest`. This is a bounded
  interpreter-independence check - it does not by itself prove the absence of
  every possible out-of-process helper, so it is read together with the crate
  dependency graph (`nioh3-worker -> nioh3-data -> nioh3-domain`, plus
  `serde_json` and `sha2`, with no process/IPC dependency) and the fact that the
  composition functions live only in `nioh3-domain` and are called directly by
  `crates/nioh3-worker/src/engine.rs`. The binary is rebuilt from the current
  candidate before every run, so a stale debug artifact cannot satisfy the gate,
  and a missing worker crate or toolchain fails the gate rather than skipping it.
- Identity and mutation binding are covered by permanent tests, not only probe
  scripts: `crates/nioh3-worker/src/context.rs`
  (`context_digest_covers_exactly_the_seven_identity_fields`),
  `crates/nioh3-worker/src/model.rs` (port of `core_services.candidate_identity`
  with pinned vectors), and the subprocess gate's contract-directory mutation,
  which proves the reported `contract_digest` is read from disk.
- Gates: worker crate 17 plus 3 passing, `tests/migration` 11-test subprocess
  gate plus the retained preview/sequence/R4 gates, domain crate 71, data crate
  17, `cargo fmt --check` and `clippy --all-targets -D warnings` clean for all
  three crates.

Bounded limits for M2.3a: previews are `effect_sequence_only` with no record
bytes, and the R4 install/preview pair stays a template-driven library
capability verified by the M2.2 native gate rather than a wire method; payloads
built from this path report `certified_offline_replay` only; descriptor selector
and `caller_option` other than zero are rejected; `enemy_states` is composed only
for playthrough 3 (the preview method fixes playthrough 3) and is null
otherwise; graded rule families whose grade occurs only at playthrough 1 are
covered by the domain parity gate, not by the subprocess gate; and
`possessed_complete = false` / `Possessed::Unknown` is unreachable with the
shipped capture (all 1,022 eligibility rows and 133 summary rows are captured),
so it is a documented capture limit covered by unit tests rather than a
seed-selectable behaviour. Locale and name resolution remain response
composition, and `raw_value` (f32) versus `display_value` (f64) are compared
without re-rounding. `GenerationContext` mutation binding over the wire,
candidate identity beyond the preview payload, product wiring, packaging and
live acceptance remain out of scope, and the worker exposes no write path.
