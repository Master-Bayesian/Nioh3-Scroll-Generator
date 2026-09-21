# Windows Tauri release runbook

This is the procedure for the Rust-backed Tauri 2 single-EXE product. Current
release status belongs in [CURRENT_HANDOFF](CURRENT_HANDOFF.md); completed
releases have separate immutable publication records. The v0.8.0 evidence is
[here](TAURI_V080_PUBLICATION_20260921.md).

## 1. Choose the work, then its acceptance

A release verifies that reviewed behavior survives packaging and delivery. It
does not restart reverse engineering or exhaustively rediscover known seeds.

| Lane | Purpose | Normal trigger |
| --- | --- | --- |
| Source checks | Versions, generated contracts/locales, resource/ABI identity, type checking | Before the release build |
| Bounded release E2E | Known-seed generation, small searches/continuation, actual packaged UI/worker graph, isolated save flows, one-file launch/update/rollback | Every candidate |
| Extended search | Full rare-condition search and solver/performance investigation | Explicitly selected for a relevant solver change or investigation |
| Development/reference | Broad unit, migration, Python-reference and research suites | Independent development CI, not duplicated in release |
| Live game | Version-matched acceptance of changed native writes | When that implementation or supported game contract changes |

An actual product failure remains a blocker regardless of which lane finds it.
Choose a check for a named risk, not for a larger test count. Known seeds are
inputs to generation checks, not prizes to rediscover in millions of CPU trials.
Synthetic fixtures, test inventory and offline parity retain their named scope;
none alone proves game legality or real-save acceptance.

## 2. Freeze and check cheap prerequisites

Read the current version scope and intended diff. Preserve unrelated work.
Synchronize package/lock files, Python version metadata, both Rust packages and
lockfiles, Tauri config and release notes. Stage an explicit whitelist.

Use PowerShell 7 and the actual project Python environment:

```powershell
$env:NIOH3_PYTHON = '<project-environment>/Scripts/python.exe'
./tools/run_python_tests.ps1 -Python $env:NIOH3_PYTHON -ScriptPath tools/preflight_tauri_release.py
```

During development a dirty observation is expected. A candidate build uses a
clean checkout with `--require-clean --expected-sha <full-sha>`. Source changes
produce a new candidate identity; the build manifest must name its real source.

The project build-root resolver keeps local Cargo targets, staging and test
temporary files off the checkout and C: system temp. Routine builds reuse the
shared target. Explicit NIOH3_BUILD_ROOT/CARGO_TARGET_DIR take precedence.
An exceptional isolated Cargo target is disposable and cleaned when finished.

Before compilation, the workflow checks versions, generated files, native
resource identity and tool availability. Hosted Windows has neither the game
nor a GPU. Its setup resolves the real WebView2 runtime first, then creates a
never-executed game-version PE in an isolated Steam tree with
`tools/prepare_ci_game_identity.ps1`. This supplies VERSIONINFO for normal host
discovery only; it supplies no native-write authority. Create and verify it early,
but activate its discovery environment only after compilation so compiler/SDK
discovery retains the real Windows environment. Local acceptance uses the
actual installed game instead of adding a second, ambiguous installation.

## 3. Prepare once, accept the actual bytes

After authorization to push/dispatch, use `.github/workflows/release.yml`.
Normal preparation uses bounded release acceptance; extended search is opt-in.

```powershell
gh workflow run release.yml --ref <candidate-branch> -f extended_search=false
gh run list --workflow release.yml --commit <full-sha> --json databaseId,headSha,status,conclusion
```

Follow the exact run ID. One worker can wait for its completion and report the
result or first concrete failure; the root does not repeatedly read all logs.

The workflow is the executable command source of truth. Its stages are:

1. Cheap source/environment preflight.
2. One clean portable build using the shared external Cargo cache.
3. Archive that same directory and wrap its ZIP with its own launcher.
4. Bounded real packaged E2E, with isolated writable state and retained evidence.
5. Verify limits and sign the update manifest; upload the six release assets.

The frontend driver accepts `--profile release` or `--profile extended` and
records the selected scope. Keep its direct known-seed assertions and small
search/cancel/resume flows separate from the optional long solver search.
Release-mode driver results identify an actual release host, not a debug one.

Retain candidate and diagnostic artifacts when a later stage fails. Inspect the
existing bytes before deciding whether compilation is needed again. A product
change invalidates prior product acceptance; a harness/environment repair needs
its own truthful evidence. Every required release gate must pass before signing
and promotion. Earlier successes cannot substitute for a failed gate.

The current preparation workflow does not resume from an injected candidate.
Its retained unsigned artifact supports diagnosis and focused local retesting;
a fresh dispatch rebuilds at the selected ref using the shared Cargo cache.
Promotion is separate and consumes a successful run without rebuilding.

### Failure handling

Record source SHA, run/step, error and evidence path. For UI failures preserve
page state, worker status/progress and logs. Synchronize asynchronous acceptance
on real responses/process outcomes, not tiny timing windows. A measured,
intentionally long workload belongs in the extended lane rather than acquiring
an ever-larger routine-release timeout.

An observation or network timeout first checks the same live run or transfer.
Use bounded retries for transport, not for known deterministic failures.
One factual ledger entry records the defect and repair. Historical failure
catalogs are references for matching issues, not instructions loaded every time.

## 4. Verify and promote without rebuilding

`.github/workflows/publish-release.yml` is manual only. Its inputs identify the
successful preparation run, full product SHA and version. Publication defaults
off; the corresponding PowerShell helper also defaults to a read-only plan.

```powershell
./tools/publish_tauri_release.ps1 -RunId <successful-run-id> -ExpectedSha <full-sha> -Version <version> -Output <fresh-deliverables-directory> -Python $env:NIOH3_PYTHON
```

The helper checks the exact repository, workflow, successful run and source,
downloads the prepared artifact, and emits a plan. It verifies:

- exactly six assets: outer EXE, EXE sidecar, update ZIP, ZIP sidecar,
  tauri-update.json and test-inventory.json;
- production Ed25519 authenticity, stable version/platform and official URLs;
- both sidecars, ZIP CRC and unique safe paths, every manifest member's size/hash;
- clean source SHA, outer footer/payload and exact manifest-owned launcher;
- the 60 MiB EXE/ZIP limits and branch/tag state.

After explicit authorization, add `-Publish` using a fresh output directory,
or dispatch the publication workflow with its explicit publication switch.
That path promotes the immutable source, uploads those same six files and
makes the release public/latest. It never recompiles or creates a new signature.

An existing matching public release is verification-only. Conflicting tags,
assets or unexplained partial state stop with an actionable report; automation
does not overwrite them or move a released tag. A full release authorization
covers the agreed sequence, including in-scope repairs. A new version, target,
destructive replacement or safety boundary needs a new owner decision, not
repeated approval of the same operation.

## 5. Verify public delivery and close

Publication is complete only when public downloads and the latest stable feed
agree with the accepted bytes. The verifier can also run independently:

```powershell
./tools/run_python_tests.ps1 -Python $env:NIOH3_PYTHON -ScriptPath tools/verify_release_artifacts.py -ScriptArgument @('--directory','<six-assets-directory>','--version','<version>','--expected-sha','<full-sha>','--report','<verification.json>','--public','--public-download-dir','<fresh-public-directory>')
```

Persist the plan, verification JSON and acceptance artifacts. Report the actual
gate scope, especially extended checks not run. Test inventory is a catalog,
not a claim that every listed test ran.

Update the publication record and current handoff in a documentation-only
commit after the immutable product tag. Report published/not published first,
with the outer EXE and Release links. Disable the release-specific wakeup.
A workflow, tag or signed local file alone is not a completed public release.

## Product boundaries retained

The supported download is the outer single EXE, not the inner app from the ZIP.
It needs no adjacent user-managed runtime files. Its launcher uses a verified,
bounded LocalAppData cache; the ZIP remains the internal signed updater input.
Updates preserve startup acknowledgement, rollback and exact cleanup.

Keep automatic backups, transaction identities, no-replay recovery and
GenerationContext. Changed native writes require their own matching live-game
acceptance; unchanged packaging does not reopen settled game research.
Production Ed25519 authenticates updates; no Authenticode publisher certificate
is claimed. Older Electron/Tk packaging is outside this release workflow.
