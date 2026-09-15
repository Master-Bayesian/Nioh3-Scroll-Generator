# v0.8.0 backend completion gates

Status: **bounded checklist. The local default backend switch is owner-authorized
and landed in the build/package/workflow defaults; this remains neither a release
nor a publication, and no live-game or user-save acceptance has run.**

This document owns the remaining-work and acceptance checklist for the v0.8.0
Rust backend migration. It derives the work from the shipped broker, the
contracts and the current Rust endpoints rather than from a fresh audit, and it
depends on the migration record [V080_RUST_BACKEND_MIGRATION.md](V080_RUST_BACKEND_MIGRATION.md)
and the detailed matrix in
`deliverables/v080-completion-readiness/MATRIX.md`.

Baseline: branch `codex/v080-rust-backend`, HEAD `3e44c41` (verified). Evidence
accepted without re-deriving it: M0 (domain/data generation tables), M1 (enemy
generation and variants), M2.1 (effect sequences and scroll records), M2.2 (R4
finalization and paired records), M2.3a (preview worker), M2.3b1 (bounded
search slice). Nothing here is packaged, tagged, released or cut over.

## Standing boundaries

- Preserve the shipped React/WebView2 frontend and the single-EXE delivery
  contract. `apps/tauri/*` bridges the existing UI to the Tauri broker; do not
  rewrite React.
- Legacy Tk compatibility stays a separate track. `nioh3_scroll_editor/app.py`
  is the legacy host; it is not replaced by this migration.
- No research runtime in the product. Research, CE tables and capture tooling
  stay out of `crates/`.
- Python stays the acceptance oracle until a cutover is owner-approved. Parity
  means matching the shipped Python worker on the same request, not "the Rust
  value looks right".
- Never mark a gate complete from static reading or unit tests alone. Each row
  below names its own authoritative gate.

## Completion gates

Gate types: **P** parity (cross-language, exact), **F** fault/recovery,
**B** build/package, **U** UI/packaged acceptance.

| ID | Gate | Type | Status |
| --- | --- | --- | --- |
| G1 | Search method surface parity: `search.catalog`, `recommended_level.resolve`, `cache.register` are served and field-exact | P | green locally (`tests/migration/test_application_worker_parity.py`, 4 passed; all eleven contract methods served, `UNIMPLEMENTED_METHODS` empty). Evidence `deliverables/m23c-application/EVIDENCE_CATALOG.md`; not packaged or cut over. |
| G2 | NG4/NG5 cache and save-bound rarity-5 map parity (`cache.register` + playthrough 4/5) | P | open |
| G3 | Missing effect-preimage routes (unconstrained, R3 primary, R5 primary, effect-constrained, Grace, secondary/roll replay) | P | open |
| G4 | `GenerationContext` mutation binding over the wire and candidate export raw-record pairing | P | open |
| G5 | Protected save surface parity: discover/register/inventory/edit/delete/install/restore/template/backups/recycle/commit/discard | P | open |
| G6 | Protected runtime surface parity: generate/search/export/capture-grace/count/live-add/live-batch/override/status | P | open |
| G7 | Protected fault, recovery and ownership: interruption, `job.cancel`, resume tokens, kill-safe, receipt-before-retry | F | open |
| G8 | Broker/channel completeness: every `window.*` call with a REACHABLE UI caller has a served channel (verify caller reachability before implementing or removing) | B | open |
| G9 | Resource/build/dependency cleanup: single EXE, no stray Python/Node/runtime dependency, deterministic artifact | B | partial - the packaged default is the Rust graph (no PyInstaller worker, no Python runtime, no bundled decryptor) and the packaging job is a required CI gate; the exact candidate package for this tree has not yet been built |
| G10 | No frontend or packaging regression versus v0.7.5 (behavior, performance, layout) | U | partial - lane dev-shape evidence is green for every named flow and the save lifecycle is faster than the shipped host, but the same-candidate packaged acceptance is pending (`/root/m4_quality_review`) |
| G11 | Minimum live acceptance before protected cutover (real save + real game process, not synthetic; fault cases on isolated fixtures only) | U | open |

## Evidence that closes a gate

- P: a cross-language gate in `tests/migration` (or the protected equivalent)
  that drives both implementations over the shipped transport and compares
  exact values, with the Python answer as the oracle. It must fail, not skip,
  when the Rust surface is missing.
- F: a gate that interrupts or fails an operation and shows recovery, with the
  failure path named by code.
- B: a reproducible local or hosted build that produces the EXE plus manifest
  and hash, plus the identity check that binds them.
- U: packaged-app acceptance against a real save and the real game process,
  recorded with paths; synthetic saves alone do not close a U gate.

## Explicitly out of scope

Equipment generation research is the next program, not part of this one. This
checklist stops when every gate above is either closed or honestly `deferred`
with a named owner, and the next research program can start without a partially
built product.
