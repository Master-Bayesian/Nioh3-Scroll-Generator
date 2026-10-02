# v0.8.3 offline reliability repairs — 2026-10-02

Status: source repairs validated locally, uncommitted and not packaged or published.

## Source and scope

Machine: ARASHI. Baseline: `ec5575c37f340558c000d4f87ece6869802f1b96`
from the clean `claude/v082-integration` worktree at
`D:/Nioh3_v080_deliverables/source-claude-v081-fixes`.
Changes are isolated on `codex/astra-audit-20261002` at
`D:/Nioh3_v080_deliverables/source-codex-astra-audit-20261002`.
The older, dirty `F:/Nioh3_ScrollEditor` checkout was left intact.
Current production architecture is Rust/Tauri; the Python backend remains a
reference/test path. Historical feedback was checked against the current handoff.
No new feature, compatibility expansion, real game/save write, process stop,
commit, push, publication or external communication was performed.

## Confirmed defects and fixes

1. **Expired server plan caused an unrecoverable client fence.** The host starts
   its 600-second plan TTL before the renderer receives the plan. A submission
   can pass the renderer's local deadline and still receive the host's exact
   `Plan expired; prepare a new plan` refusal. `SaveApplication::commit` emits
   this before writing durable intent; there is no receipt to recover. The
   client previously retained `uncertainOperationId`, blocking a new plan even
   after refresh. The existing exact prewrite-refusal matcher now also recognizes
   this canonical message. It still discards the old plan/snapshot, requires a
   refresh and retains recovery for lost responses, unknown results and messages
   merely containing similar text. No automatic commit replay is introduced.
   Evidence: `apps/desktop/tests/save-session.test.ts`, through the actual
   `SaveSession`, `saveGateway` and `OperationController` with scripted IPC jobs.

2. **Explicit compatibility-backup retries reused stale success.**
   `CompatibilitySession::prepare` copied files only when no verified backup
   was cached. After a source changed or disappeared, Retry automatic backup
   still returned the previous paths and `verified: true`. Each explicit
   prepare now runs the existing copy/readback path again and replaces the
   current report with its actual outcome. Previous backup files remain intact;
   readback, process-bound consent and manual-backup confirmation stay in place.
   Evidence: `crates/nioh3-protected/tests/compatibility_backup_refresh.rs`,
   using only owned synthetic files, changed bytes, an added source, a missing
   source set and successful retry after failure.

The project AGENTS guidance now records the requested simple-fix preference,
explicitly retaining real boundary validation, backups, rollback, single-writer
locks, DLL verification and protected-operation recovery.

## Verification

Evidence root:
`D:/Nioh3_v080_deliverables/deliverables/codex-astra-audit-20261002`.
All build/test temp used this project's D: delivery volume. Cargo reused
`D:/Nioh3_v080_deliverables/build-cache/python-tests`; no dependency installation
or private game/save fixture was needed.

| Gate | Result | Evidence under `logs/` |
| --- | --- | --- |
| New SaveSession regressions before product fix | 9 passed, 2 failed at the phantom-fence assertions | `save-session-red.log` |
| Backup retry regressions before product fix | 0 passed, 2 failed (old bytes and stale verified status) | `backup-refresh-red.log` |
| Final SaveSession + OperationController targeted tests | 18 passed | `save-session-green-final.log` |
| Final complete `npm test` script | 89 passed, 0 failed, 1 skipped of 90 | `typescript-full-suite-final.log` |
| Complete TypeScript typecheck | passed | `typecheck-final.log` |
| Backup refresh + existing framed compatibility host tests | 3 passed; host test covers six version/backup combinations | `backup-refresh-green.log` |
| Protected crate Clippy, all targets, warnings denied | passed | `protected-clippy-final.log` |
| Protected crate formatting and Git whitespace | passed | `protected-fmt-final.log`, `diff-check.log` |

The one npm skip requires `NIOH3_PACKAGED_WORKER_EXE`; no package was selected or
built for this source-only task. The full Python and full Rust test suites were
not run. The npm suite includes existing Python IPC and encrypted synthetic-save
flows; it is not real-game or real-save acceptance.

Initial source gates exposed a new fixture's overly broad API result type,
13 pre-existing Clippy unwrap errors in the equipment-seed test module, and
pre-existing module-order formatting drift. The fixture now returns typed
ProtectedJob envelopes; the test module follows its neighbor's test-only unwrap
allowance while production denial stays unchanged; module ordering was formatted.
Original failure logs and the bounded failure-ledger entry remain available.

Repeat from the isolated checkout (PowerShell 7):

```powershell
$env:TEMP = 'D:\Nioh3_v080_deliverables\deliverables\codex-astra-audit-20261002\tmp'
$env:TMP = $env:TEMP
$env:NIOH3_BUILD_ROOT = 'D:\Nioh3_v080_deliverables\deliverables\codex-astra-audit-20261002\verification'
$env:CARGO_TARGET_DIR = 'D:\Nioh3_v080_deliverables\build-cache\python-tests'
npm test
npm run typecheck
cargo test --locked --offline --manifest-path crates/nioh3-protected/Cargo.toml --test compatibility_backup_refresh --test host_compatibility
cargo clippy --locked --offline --all-targets --manifest-path crates/nioh3-protected/Cargo.toml -- -D warnings
cargo fmt --check --manifest-path crates/nioh3-protected/Cargo.toml
git diff --check
```

## Remaining acceptance

This batch is complete as reviewable local source. Test9 and all older standalone
binaries are unchanged and do not contain these fixes. Integration and a new
matching package are separate next steps; native UI/game/save-reload acceptance
and publication are not claimed. No active environment blocker remains: a brief
exec-server disconnect prevented one final-suite process from starting, after
which a read-only machine check succeeded and the single final rerun passed.
