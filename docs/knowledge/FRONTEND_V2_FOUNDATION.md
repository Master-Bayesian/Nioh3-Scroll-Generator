# Frontend V2 engineering foundation

Current startup guide (0.7.0 preparation): extract the entire portable ZIP and run
`Nioh3ScrollEditorV2.exe`. No external Python or CE installation is required.
For source development, install `requirements.txt`, run `npm ci`, then launch
`tools/start_frontend_v2.ps1 -Python <your-python-executable>`.
Build with an isolated environment from `packaging/requirements-v2.lock.txt` and
`tools/build_frontend_v2.ps1 -Python <build-python> -Output <new-output-directory>`.
See [current acceptance and release preparation](V070_PREPUSH_COMPLETION_20260909.md).

The foundation notes below were written on 2026-09-07. Their pending items and
engineering-only descriptions are a historical checkpoint, not current release status.

## Baseline and bounded review

The base is commit `8ad89ea4aee088b542977be14ec9e7c54e6bc3d1`, reached by
the annotated tag `backend-freeze-before-v0.7.0`. The tag object itself has a
different hash; use `git rev-parse backend-freeze-before-v0.7.0^{commit}` when
comparing the commit identity.

Read `BACKEND_FREEZE_BEFORE_V070.md`, `CURRENT_HANDOFF.md`, both versioned
project statuses, the existing design notes, and the prior Astra audit in task
"审查架构设计" (`6a9e4227-aab8-83e9-86a6-823580599194`). The audit's B1-B6
findings were checked against the frozen implementation and regression gates.
No new freeze blocker was found in this bounded pass. This was not another
global audit or new gameplay acceptance.

## Architecture and ownership

React uses two typed, sandboxed preload surfaces: `window.nioh` for search and
`window.operations` for save/runtime commands. Electron validates the sender,
restricts public commands, resolves file selection through a native dialog, and
brokers private candidate/template/cache transfers. Renderer code never obtains
record bytes, template bytes, arbitrary filesystem access, or a process handle.

Three separately owned Python processes use bounded length-framed stdio JSON:

| Role | Responsibility | Termination policy |
| --- | --- | --- |
| Offline search | Exact replay, inverse search, measured-map reuse, catalog | Killable on failure; session tokens expire on restart |
| Save host | Snapshot-bound preview, serialized commit, backup/restore, operation receipts | Never force-killed; writes finish before shutdown |
| Runtime host | Verified running executable, native generation/search, map capture, temporary overrides | Must restore hooks and observe pending remote calls finish before shutdown |

Python `search_application.py` owns the 16 search definitions extracted from Tk;
Tk imports/re-exports them. `save_application.py`, `runtime_application.py`, and
`cache_application.py` own application orchestration over the existing adapters.
`worker_transport.py` has no role or application dependency. Generation/RNG,
R4 numerical finalization, accelerator kernels and native ABI were not rewritten.
The native adapter only gained visibility into retired remote-call ownership.
The save adapter gained optional source-hash checks and explicit empty-inventory
reading; its legacy defaults remain unchanged.

No equipment/trainer abstraction or speculative live-add interface was created.
Future domains can own their own contracts and application services without
adding methods to a widget class or borrowing an offline worker's kill policy.

## Contracts and search

The four schemas under `packages/contracts/` are canonical; `npm run contracts`
regenerates TypeScript. Both ends validate envelopes. Protected responses have
explicit handshake, job, inventory, plan, receipt and runtime status structures;
adapter report details and operation-specific previews are extensible objects.

NG3 rarity 3/4/5 uses the certified existing offline core. NG4/5 rarity 5 requires
`operations.bindCachedSearch` against a selected save snapshot, then the returned
`cache_id` in the ordinary search request. The save host validates the complete
legacy-compatible map against the decrypted save fingerprint and generation
context. The search host validates the category/rarity and binds resume tokens to
the immutable registered map. Registered maps are session-local and bounded.
No renderer-provided map path or map payload is accepted by the preload API.

Search retains one job, up to 100 candidates, bounded pages, cooperative
cancellation, explicit CPU fallback consent, and HMAC-bound page checkpoints.
Draft changes do not mutate submitted requests. Restart clears candidates and
resume eligibility. Catalogs include exact/aggregate terrain selections, enemy
roles, special-rule variants and legal family membership. Raw terrain-row indices
are not accepted. NG4/5 generated installation remains blocked by frozen policy.

## Save workflows

1. Discover or select a character save. Read inventory and retain `snapshot_id`.
2. Prepare edit/delete/restore, or call `operations.prepareInstall` with a
   search/native candidate ID. No write occurs during preparation.
3. Review the returned plan and explicitly submit `save.commit` with `plan_id`.
4. Observe the protected job and its receipt. Query `save.operation` or
   `save.operations` after a connection failure; never automatically replay.

Plans expire after ten minutes and bind the encrypted source hash. The adapter
checks the hash again inside its cross-process save lock. Generated installation
reuses the frozen policy and materializer; R4 final preview and installation
stage remain paired. Local free edits preserve all seven separate effect slots.
An empty inventory can be read/restored but cannot supply a native template.

Durable intent is fsynced before a write. Receipts distinguish committed,
committed-with-warning, not-committed and unknown. Interrupted intent becomes
unknown on reload. A restore exception is conservatively unknown because sibling
files may have changed even if the main save hash did not. Backups remain scoped
to the selected account and slot. Existing application data-directory settings
are honored; `NIOH3_STATE_ROOT` supports isolated engineering runs.

## Runtime workflows

`operations.generate`, `searchNative`, and `captureGrace` obtain templates through
the broker and require explicit title-screen acknowledgement. Native search is
bounded to at most 100,000 seeds per job and reuses the existing exact scanner.
Map capture calls the existing live map builder and writes its existing atomic,
context-bound cache format. The running process executable is verified before
opening the native adapter. The later 2026-09-07 title-screen research exercised
isolated native construction; see the experiment report for its narrower scope.

Temporary overrides expose stopped, armed-no-hit, applied-hit and unknown states.
A restoration failure retains session ownership. Native timeout retirement is
observable, and pending remote calls prevent safe shutdown. On broker EOF the
protected host finishes its operation and keeps trying safe cleanup. Electron
refuses to close when safety cannot be established, without killing that host.
This is not proof that an override was hit or accepted in a real game.

## Development and packaging

```powershell
$env:NIOH3_PYTHON = 'absolute/path/to/python.exe'
npm ci
npm run contracts
npm test
npm run test:electron
```

`tools/start_frontend_v2.ps1` starts the engineering workbench. Prepare an
isolated Python 3.12 environment from `packaging/requirements-v2.lock.txt`, then
run `tools/build_frontend_v2.ps1 -Python <python-executable> -Output <new-directory>`.
This builds both PyInstaller workers, the desktop, dependency/license inventories
and a SHA-256 file manifest. `tools/verify_frontend_v2.mjs <directory>` verifies
the result independently. Output directories must be new.
Launch `Nioh3ScrollEditorV2.exe` inside that directory.
`NIOH3_PORTABLE_EXE` makes the Electron smoke test launch the actual portable app.

The portable artifact is unsigned and local. Its manifest detects accidental
file differences; it is not a signature or a release trust root. The old
single-EXE updater is deliberately not connected to a multi-file Electron app.
Production signing, release channel configuration and an authenticated atomic
update delivery are separate release work, not implied by this build.

## Figma handoff and acceptance boundaries

Replace the temporary engineering presentation while preserving the typed APIs,
search controller, source/snapshot/plan identities and explicit operation states.
The operation command textarea is a developer harness, not final product UX.
Use the exact variant and policy metadata rather than rebuilding domain rules in
React. Preserve project authors, community and project links in product screens.

The central nonvisual workflows are connected: search, catalog, candidate transfer,
local inventory/edit/delete/install/restore, operation recovery, native generation,
bounded native search, measured-map capture/reuse and temporary override ownership.
Native search currently uses the conservative scanner without migrating Tk's
optional primary-map acceleration tuning; numerical optimization remains in the
existing backend. Old Tk remains the complete migration fallback.

Final layout, educational disclosure placement and interaction polish follow
Figma. Production signing/update delivery and the frozen real-game/real-save,
exact-role hook and non-NVIDIA hardware acceptance still require their own evidence.
See `V2_REQUIREMENTS_BACKLOG.md` for the screenshot and later live-inventory-add
research. Those research items are not silently turned into supported behavior.
See `deliverables/frontend-v2/VERIFICATION.md` for current local evidence.

## Locale and transport hardening follow-up

The engineering renderer, lifecycle labels, error summaries, native dialogs and
catalog fallbacks support `zh-CN`, `en-US` and `ja-JP`. The separate, strictly
limited preferences bridge persists locale in the OS user-data directory with
serialized atomic replacement. Locale changes preserve the query, submitted
job and results; labels are fetched from the existing localized native catalogs.
Stable numeric IDs and rule-family membership are invariant across all five
playthroughs. Machine diagnostic text remains available separately and may be
in its original language. No final layout decisions are implied.

`tools/audit_v2_localization.py` reports exact coverage before fallback. Japanese
now has all 32 talisman qualifiers captured from PC v2.01 (source update after r2); English
has five unnamed dummy effects. Do not fabricate official names. All 3,609
effect IDs have exact Chinese and Japanese names. This is resource coverage,
not proof of linguistic or game-version acceptance of every string.

Protected-client fault tests now exercise a real child with delayed frames and
an invalid frame: timeout never replays, late responses remain tolerable, no
owner is killed, and EOF cleanup does not turn an unknown state into a safe ACK.

Current portable artifact: `deliverables/frontend-v2/portable-v2-foundation-ready-r2`.
Read `deliverables/frontend-v2/EXPERIMENTS_20260907.md` before P1/P2, R3/R5 or
live-insertion work. The fixed R3 template flag and title-screen R5 header cap
are experimentally separated. No new offline context or live-write endpoint
was enabled by those findings.

## Displayed recommended-level input

`V2_RECOMMENDED_LEVEL_SELECTION.md` documents the read-only resolver now exposed
through the worker and sandboxed preload. A displayed target of 350 resolves to
canonical raw values 585 and 586, with an explicit lowest-raw selection policy.
The catalog supplies compact bounds and evidence metadata. Missing targets are
never silently clamped or rounded. Existing command fields and defaults continue
to use their previous raw semantics, and the frozen forward float32 functions
remain AST-identical. This supplies final UI controls with a reliable typed
mapping without selecting their visual layout.

## Nonvisual workflow completion

See `FRONTEND_V2_INTEGRATION_GUIDE.md` for the final frontend adapter contract.
`OperationController` and `SaveSession` expose stable external stores, precise
review/commit identities, cancellation acknowledgement and receipt recovery.
The broker recovers the latest acknowledged search across renderer reload and
filters private protected jobs from recovery responses. Unknown writes are
never replayed automatically, including after a new view loads durable history.

Offline queries can select seed-derived initial challenge capacities 4-7 with
`initial_challenge_counts`. Page checkpoints include rejected candidates and
resume tokens bind this filter. Candidate DTOs and save inventory expose capacity;
inventory separately exposes remaining attempts and predicted displayed level.
This adds no direct remaining-byte edit or new native-search parameter.

The final nonvisual gate is 501 Python tests, 22 source IPC tests, 22 packaged
IPC tests, strict-GPU source/packaged R3/R4/R5 parity, and actual portable Electron
startup/search/resume/reload/locales. The synthetic encrypted-save workflow covers
real crypto and cross-worker candidate installation/readback. Six core numerical
sources remain identical to freeze after newline normalization. Sanitized vectors
retain the three completed first-reveal observations and all three unaccepted
232-byte completion candidates for Seed 36526331.

`tools/build_frontend_v2.ps1` builds with pinned Python dependency verification
and emits dependency/notices inventories plus a complete versioned file manifest.
The final portable artifact has 137 verified entries. Startup uses Electron's
`original-fs` to verify actual ASAR file bytes before starting any worker. This
fixes the observed `PACKAGE_PATH_INVALID` startup failure of the first trial.
Single-instance handling and an allowlisted local diagnostics export are included.
The diagnostics report contains only versions, context/contract digests and
connection state. The manifest remains unsigned and is not an update trust root.
