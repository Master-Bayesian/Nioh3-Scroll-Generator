# Frontend V2 integration guide

This is the nonvisual integration contract for the final Figma implementation.
The engineering workbench is a runnable reference, not an approved layout.
Read `FRONTEND_V2_FOUNDATION.md` for ownership and acceptance boundaries.

## Entry points

| Responsibility | Entry point | Ownership |
| --- | --- | --- |
| Offline query, progress, cancellation and resume | `SearchController` / `window.nioh` | A killable offline Python worker; its latest job survives renderer reload |
| Save workflow and reviewed-plan identity | `SaveSession` / `saveGateway` | One workflow instance plus an `OperationController('save', ...)` |
| Protected task observation and recovery | `OperationController` / `window.operations` | The broker retains the save/runtime owner across renderer reload |
| Native generation, bounded search, measured maps, temporary overrides | `window.operations` | Runtime host with explicit game-state preconditions and safe cleanup |
| Catalogs and display-level conversion | `searchCatalog`, `resolveRecommendedLevel` | Stable numeric IDs and exact backend resolution |
| Presentation language | `LanguageProvider` / `window.preferences` | Persisted `zh-CN`, `en-US`, `ja-JP`; independent of query identity |
| Local support report | `window.support` | Version, contract and connection metadata only |

Renderer code must not import the Python implementation, invoke subprocesses,
read arbitrary files or transport raw scroll records. `preload.ts` is the
allowlisted bridge. Both JSON Schema pairs are authoritative; run `npm run
contracts` after intentional changes. Generated TypeScript is not edited by hand.

## Search state and inputs

Keep a form draft separate from `SearchState.submitted`. Resume uses the
submitted query, even when a draft has changed. Render `getSnapshot()` using
`useSyncExternalStore`; call `connect()` on mounting and `dispose()` on unmounting.
Disposal stops observation, not an owned operation. An explicit offline-worker
restart clears its session, candidate registry and resume-token signing key.

`currentSearch()` restores the latest job and its acknowledged submitted query
after a renderer reload. A context mismatch fails closed. Sequence numbers stop
late progress snapshots from undoing an acknowledged cancellation.

An optional offline query field selects seed-derived initial challenge capacity:

```json
{ "initial_challenge_counts": [7] }
```

The complete query still requires the other schema fields. Omission or `[]`
accepts any capacity; distinct integers 4 through 7 are supported. This filter
is applied to completed candidates after each bounded solver page. Rejected
candidates still advance the checkpoint. The resume token binds the filter,
context, CPU policy and map identity. Selective filtering may require more pages.
This field is not part of the native search contract.

Candidate DTOs expose `initial_challenge_capacity`. Save entries separately
expose `derived.initial_challenge_capacity`, `remaining_challenge_attempts`,
`recommended_displayed_level`, and `recommended_raw_was_clamped`. Derived
metadata is read-only; `editFromEntry(entry)` intentionally excludes it.
An experimental remaining value above 7 is visible as stored, not legitimized
or clamped by the inventory reader. Remaining-count editing is not enabled.

`resolveRecommendedLevel(displayed)` returns exact raw alternatives or an
explicit unavailable result. Only its exact `selected_internal_level` can be
used as a chosen raw value. Target 350 resolves to 585/586, selecting 585; existing
defaults remain unchanged pending real-game acceptance of that target. The
transfer field remains uint32; decimal 4294967295 represents all bits set.
Recipient increment/wrap has not been accepted.

## Save workflow

Create the observer and session once per workflow, independently of final view
components. A native file dialog supplies a `SaveReference`; renderer code
cannot register an arbitrary path. Then:

1. `session.select(reference)` reads a snapshot and durable operation history.
2. Use `editFromEntry`, `prepareEdit`, `prepareDelete`, `prepareInstall` or
   `prepareRestore`. Show the returned plan preview and retain its exact ID.
3. After reviewing that plan, call `session.commit(reviewedPlanId)` from the
   corresponding user action. This is a separate action from editing a draft.
4. Display the receipt, then `session.refresh()` for a new inventory snapshot.

The session binds save ID, snapshot ID, source hash, plan ID and expiry. Refresh
discards any previous review. A submitted commit invalidates its old plan and
snapshot immediately. Draft mutation cannot replace an already reviewed plan.

Transport interruption is an unknown observation, not proof of a failed write.
`recoverReceipt()` queries the exact operation ID; it never replays `save.commit`.
Unknown/executing receipts block further changes. A newly created view loads
unresolved durable history too. Explicit acknowledgement of an unknown result
requires a subsequent inventory refresh and review; it does not rewrite the
durable receipt. Acknowledgement is scoped to this session and may need review
again after application restart. Physical process-loss outcomes remain subject
to the same conservative backend rules.

`OperationController` distinguishes idle, submitting, running, completed,
failed, interrupted and busy elsewhere. Only a cancellable running task offers
cancellation. Recovery may wait for another owned operation to finish before
querying a receipt. The public broker hides template, cache and raw-record job
results even during current-job recovery.

`save-workflow.test.ts` exercises the full path with a new synthetic encrypted
save: real crypto, Python IPC, edit, receipt, delete, restore, real offline
search, candidate transfer, reviewed installation and inventory readback.
It is not a playable-save or in-game acceptance test.

## Build and diagnostics

Use Node 24 and an isolated Python 3.12 environment installed from
`packaging/requirements-v2.lock.txt`, plus `npm ci`. Build with:

```powershell
./tools/build_frontend_v2.ps1 -Python 'absolute/path/to/python.exe' -Output 'deliverables/frontend-v2/new-portable-directory'
node tools/verify_frontend_v2.mjs 'deliverables/frontend-v2/new-portable-directory'
```

Output must be a new directory. The script builds both workers and the desktop,
records locked dependencies and their available license notices, then verifies
all manifest files. Startup repeats verification before constructing workers.
Electron uses `original-fs` for physical ASAR verification; its virtual filesystem
would otherwise classify an archive as a directory. The manifest detects
accidental differences, not malicious replacement of an unsigned distribution.

One instance owns the normal application profile. Diagnostics export is local
and user-selected. It contains no save path, account ID, raw records, queries,
stderr, environment variables or memory addresses. It never attaches to the
game or starts a protected host just to collect diagnostics.

The workflow in `.github/workflows/frontend-v2.yml` includes source tests,
sanitized live-vector replay, locked worker builds, packaged IPC/parity and
actual portable Electron smoke. Hosted parity explicitly permits CPU fallback;
local strict-GPU parity is a separate recorded gate. The workflow has not been
run remotely as part of this working-tree task.

## Remaining scope

The final Figma work can replace the temporary views while using these APIs and
controllers. Equipment/trainer features should introduce their own application
services and capability evidence when concrete requirements exist; do not add
their numerical rules to React or turn the scroll service into a generic command
executor. Old Tk remains available during migration.

Still separate: production signing and authenticated updates; non-NVIDIA and
physical interruption acceptance; the remaining game tests in the freeze and
live handoffs; five unnamed English dummy effects. All 32 native qualifier names are now
verified in Chinese, Japanese and English. Live insertion has bounded gameplay
acceptance and an optional CE adapter; see LIVE_ADD_ENGINEERING.md for the
integration and remaining live acceptance boundary. No further
battles, new generation contexts or speculative equipment/trainer APIs were
required to complete this nonvisual foundation.

## Optional live addition

`window.operations.prepareLiveAdd({save_id, snapshot_id, source, job_id,
candidate_id})` transfers a broker-owned ready candidate and registered save
reference to the runtime worker. The renderer cannot send record bytes, process
addresses, an executable script, or an arbitrary save path. The response is a
protected job whose result contains `live_add` with a durable operation UUID,
plan digest, instance serial, backup location and review state.

Use `runtime.live_add_execute` with the exact reviewed UUID/digest; use
`runtime.live_add_status`, `runtime.live_add_recover` or `runtime.live_add_cancel`
with that UUID. Cancellation is available only before dispatch. Recovery queries
and reconciles the previous attempt; it never repeats insertion. `LiveAddSession`
and `liveAddGateway` compose the existing `OperationController` for React-independent
review, uncertainty handling and local reference restoration. Store only the UUID
and digest in view preferences; keep plans, raw evidence and backups in the host.

Configure the optional local executor with `configure-live-add.ps1` in a portable
build (or `tools/configure_live_add.ps1` in source). Its generated README explains
CE attachment, the local bootstrap and V2 launcher. CE remains open until remote
ownership is released. Ordinary search/save features do not require CE. Do not
expose this as universally accepted gameplay: the integration was completed after
the user closed the game. Final Figma controls should present the existing
preparation/commit/result flow without redesigning its backend semantics.
