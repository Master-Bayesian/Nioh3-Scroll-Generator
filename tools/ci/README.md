# Full CI scheduling with affected-selection shadow comparison

The `Tests` workflow is the single normal entry point: PRs, main pushes,
merge groups and manual dispatch. Feature-branch pushes no longer run a second
copy beside the PR workflow. The old frontend workflow is incorporated here.
Release preparation/promotion is unchanged.

## Execution and coverage

- `plan.mjs` discovers every Node test, formal model/control and required normal
  suite, then uses `timings.json` for deterministic longest-first balancing.
  Recorded suite costs and proportional bootstrap estimates are labelled.
- The reference is always `Full`. Eight Rust manifests receive tests and
  Clippy/format checks, including the launcher. Original feature combinations,
  workshop export verification, Python packaging/native provenance and actual
  read-only-worker UI acceptance remain required.
- Generation verification precedes immutable frontend build artifacts. Static
  checks, model shards and tests run independently where prerequisites permit.
  Required UI locales and assertions remain; text-fit/expanded/hell cases split
  by the original three locales. Other production-component suites remain whole.
- `shadow.mjs` follows transitive TypeScript-resolved literal imports and
  conservative browser/filesystem inputs. Global/non-imported, structural,
  unknown, unresolved and missing-history changes select Full. This initial
  graph is deliberately conservative; it does not prune any reference tests.
- PR/manual shadow cases actually execute, then compare against the full
  reference. Main/merge-group are FullOnly; this is an explicit event decision,
  not a missing shadow accepted as a pass.
- `gate.mjs` always verifies source/plan/invocation identity, exact unique case
  assignments, complete current reports and evidence hashes. Missing, failed,
  cancelled, stale or altered results fail. Empty assigned shards are recorded
  as NotApplicable rather than test passes.

## Commands

Use Node 24 and locked project dependencies. For native tasks also select the
pinned Rust toolchain and `NIOH3_PYTHON` prepared interpreter. Set the external
`NIOH3_BUILD_ROOT`, `CARGO_TARGET_DIR`, `TEMP` and `TMP` first. Keep each output
root unique. Commands run from a clean committed checkout:

```powershell
node tools/ci/plan.mjs --out <plan-directory>
node tools/ci/shadow.mjs --plan <plan-directory>/plan.json --out <shadow-directory>
node tools/ci/run.mjs --plan <plan-directory>/plan.json --group frontend-1 --out <fresh-shard-directory>
node tools/ci/gate.mjs --plan <plan-directory>/plan.json --reports <downloaded-reference-and-shadow-artifacts> --shadow <shadow-directory>/shadow.json --out <gate-directory>
```

The frontend build group produces `bundle/workshop.json` and `workshop.html`.
Set `NIOH3_CI_WORKSHOP_BUNDLE` to the verified manifest for UI groups. It binds
the source inputs and actual HTML bytes; consumers do not rebuild shared paths.

`run.mjs --task <id>` is an explicit diagnostic profile for one case. Its receipt
cannot satisfy a reference or shadow group. Use it to reproduce a selected test,
not to claim full CI coverage.

Run the real orchestration CLI E2E and formal controls:

```powershell
node tools/ci/verify-ci-e2e.mjs --out <fresh-owned-fixture-directory>
./tools/check_models.ps1 -Model CIValidation -Evidence <fresh-model-evidence>
```

The E2E uses an owned Git repository, real Node assertions and deliberate
artifact faults. Its controlled aggregate receipts are not Cargo/browser/model
product acceptance. The normal workflow separately executes those real suites.

Artifacts use a run-attempt namespace and plans bind the invocation. Retry the
whole workflow or dispatch a new full run; do not combine old successful shards
with a failed-jobs-only attempt. Failure evidence remains in its original attempt.

After a completed CI run, record actual wall/setup/shadow costs with:

```powershell
node tools/ci/measure.mjs --run <run-id> --baseline <baseline-summary.json> --out <fresh-comparison-directory>
```

The final artifact includes actual per-case `timings.json`. Review successful
compatible samples before updating the committed timing map. Failed/timeout
durations are not cheap successful timing samples. A single run establishes an
observation, not a p50/p95 promise.

The authoritative policy is
[`skills/nioh3-ci-quality/references/ci-policy.md`](../../skills/nioh3-ci-quality/references/ci-policy.md).
Affected pruning is still disabled and needs a separate acceptance decision.
