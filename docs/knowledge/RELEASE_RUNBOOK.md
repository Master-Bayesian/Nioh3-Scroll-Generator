# Windows release runbook

Use this procedure for the Electron portable distribution. The legacy Tk
single-EXE build is a separate source entry and cannot install this package.
See [the v0.7.0 failure record](V070_HOSTED_BUILD_FIXES_20260909.md) for the
environment and integration failures that established these gates.

## Establish the candidate

1. Read `CURRENT_HANDOFF.md`, the previous release record and the release notes.
   Reuse completed game acceptance when its implementation is unchanged. Tests
   and package smoke checks are not substitutes for new game acceptance when a
   change actually affects an unverified memory operation.
2. Inspect `git status`, the staged diff, `gh auth status`, the remote main ref
   and any existing version tag. Stage only reviewed source, tests and docs.
   Keep saves, captures, private dumps, signing material and developer build
   directories out of the commit. Do not use `git add .` for this research tree.
3. Synchronize the Python and npm versions and the release notes. Record the
   complete candidate commit SHA. A subsequent code change invalidates earlier
   release preparation, even if its version string is unchanged.
4. Use a real fresh Windows Git checkout for cold-build reproduction, with
   `core.autocrlf=true`. Copying working files does not exercise checkout attributes.
   Use the workflow's Python/Node versions and pinned dependency files.

## Run inexpensive checks first

The release workflow executes these before packaging. Run the relevant checks
locally when changing their inputs; do not repeatedly rebuild before fixing a
deterministic early failure.

```powershell
python tools/verify_native_build_manifest.py
python tools/write_test_inventory.py --output deliverables/release/test-inventory.json
npx tsx --test apps/desktop/tests/portable-update.test.ts
```

- Native source/DLL/ABI identity must remain exact. Keep LF attributes for native
  sources. Do not update identity hashes merely to bless a CRLF checkout or alter
  verified RNG/finalizer code to make a packaging test pass.
- Test inventory must contain unique IDs with no discovery errors. Imported
  `TestCase` classes can be collected twice; import fixture modules instead.
  Report the unique inventory count and hardware skips separately.
- The metadata test reads the actual archive and signing commands in
  `release.yml`. The asset must be a safe ZIP basename with a matching official
  version-tag URL. A product name or `V2` prefix is not a trust boundary.
- Regenerate contracts and locales and require empty `git status --porcelain`,
  as well as no tracked content diff. Windows line-ending normalization can make
  the content diff empty while Git status still reports modified generated files.
  Generated files must match their committed bytes and declared EOL attributes.
  Official packaging requires `NIOH3_REQUIRE_CLEAN_SOURCE=1`. Use the native
  fault matrix and `tools/run_cpu_only_tests.py` for CPU-only coverage, even on a
  GPU-equipped development machine. `NIOH3_PARITY_ALLOW_CPU` configures drivers;
  it does not relax the native execution policy.

## Prepare on GitHub before publishing a tag

Push the reviewed candidate branch, then dispatch the existing workflow:

```powershell
gh workflow run release.yml --ref codex/todo-321
gh run list --commit <full-candidate-sha> --json databaseId,headSha,status,conclusion,workflowName
```

Use the actual candidate branch if different. Inspect runs by commit SHA, not by
the newest run's position. Require successful Tests, Frontend V2 foundation and
manual Signed Windows release runs for that candidate.

The manual release run performs dependency installation, source tests, native
fault checks, cold packaging, tests against the bundled workers, replay parity,
Electron startup, connected synthetic-save operations, collection persistence,
layout/language checks, archive verification and official manifest signing.
It retains `nioh3-v2-release` artifacts but does not publish a GitHub release.

The packager explicitly runs the pinned Electron package's installer. A prior
Electron launch must not be necessary for packaging. Keep the workflow capable
of going straight from `npm ci` to the packager.

When a run fails, inspect `gh run view <run-id> --log-failed`. Fix the observed
cause before dispatching again. Do not hide failures with forced clicks, broad
retries, permissive native fallbacks or omitted tests. For UI timeouts, examine
the captured visible status; wait for actual operation completion rather than
an old row count that was already true. A several-minute packaged verification
step is expected and is not itself evidence of a hang.

## Verify the prepared download

```powershell
gh run download <successful-manual-run-id> --name nioh3-v2-release --dir deliverables/release/prepared
```

Use a new directory per candidate. Before promotion:

- Verify `v2-update.json` through the real `validateUpdate` implementation in
  `apps/desktop/src/portable-update.ts`, using its embedded production public key.
  The signing workflow validates with the same key; no unsigned fallback is valid.
- Match the ZIP's actual size and SHA-256 with both the signed manifest and the
  `.sha256` file. The signature authenticates the complete archive.
- Verify every archive member against `build-manifest.json`, reject unexpected
  or unsafe entries, and check the ZIP CRC. The manifest must record the exact
  candidate SHA and `dirty: false`.
- Extract to a new directory and use the packaged startup/parity drivers if local
  verification is needed. Set `NIOH3_PORTABLE_EXE`, `NIOH3_WORKER_EXE` and
  `NIOH3_PROTECTED_WORKER_EXE` to this download. Use isolated test state and
  synthetic saves; never launch a test against a player's save by accident.

The repository secret signs the update manifest with Ed25519. This does not
provide Windows Authenticode signing. Never claim that the EXEs carry a Windows
publisher certificate when they do not.

## Promote and verify public delivery

Re-read remote main and tag refs immediately before publishing. Main must be an
ancestor of the verified candidate. If it advanced incompatibly, integrate the
change and prepare the resulting commit again. Never force-push over someone
else's work or silently move a published version tag.

Create an annotated version tag at the exact verified SHA, then push main and
the tag atomically. For example, after replacing the values with verified ones:

```powershell
git fetch origin main
if ($LASTEXITCODE -ne 0) { throw 'Cannot refresh remote main' }
git merge-base --is-ancestor origin/main <full-candidate-sha>
if ($LASTEXITCODE -ne 0) { throw 'Main is not an ancestor of this candidate' }
git tag -a v<version> <full-candidate-sha> -m "Release v<version>"
if ($LASTEXITCODE -ne 0) { throw 'Cannot create the version tag' }
git push --atomic origin <full-candidate-sha>:refs/heads/main refs/tags/v<version>
```

Stop if the ancestry command fails. On PowerShell, inspect annotated tags with
`git rev-list -n 1 <tag>` and `git cat-file -p <tag>`; an unquoted `^{}` expression
can be parsed incorrectly. Verify remote main and the peeled tag after pushing.

The tag workflow rebuilds, verifies and publishes. Wait for its success, verify
that the release is public and on the intended stable/beta channel, and download
its public assets. Repeat the signature, archive hash, member integrity and clean
source-commit checks on that public download. Build timestamps can change ZIP
bytes, so a manual-preparation checksum is not the final public checksum.

Confirm the latest stable release when publishing stable, record the final asset
name/size/SHA, successful run URLs and tag commit, and update `CURRENT_HANDOFF.md`
and the failure record. Publish those documentation updates without moving the
released tag. Preserve older local artifacts as historical evidence and label
which public download supersedes them.

Users migrating from v0.6 need the complete ZIP once. Keep `v2-update.json` as
the whole-package update protocol; do not offer the ZIP as the legacy updater's
single executable. Internal protocol or executable names can retain V2 for
compatibility without requiring it in the downloadable ZIP's name.

## Tauri release preparation gate

The release workflow now runs only by explicit dispatch. It prepares and signs
Tauri artifacts without publishing. After checking the exact downloaded artifact,
create the version tag at that workflow's commit and publish those same bytes;
never rebuild locally for upload. Tag pushes must not invoke the preserved
Electron workflow. The Electron implementation remains on
codex/electron-preserved-before-tauri2.

Limits: ZIP <= 60 MiB; no Chromium/Node payload; every dependency has notices;
manifest records a clean checkout; frontend and workers are tested from the
actual package. Use explicit NIOH3_PYTHON, locked Cargo/npm/Python dependencies,
and preserve existing native EOL/ABI identity checks. Windows resource paths may
have canonical or extended-length aliases; compare canonical paths before scoped
update cleanup. Test actual restart, not only a mock installer acknowledgement.

The first Tauri installation is a manual ZIP migration. Do not advertise the
legacy single-EXE or Electron manifest as compatible with the Tauri format.

### Tauri preparation follow-up

Two inherited CI assumptions were repaired before any hosted release package was
built: the release regression still expected an Electron manifest URL in YAML,
and the preserved Electron cleanup compared Windows 8.3 paths lexically. The
latter was reproduced locally using an actual short-path TEMP directory and now
compares physical parent/target paths while retaining symlink and user-file
refusals. Shared frontend CI no longer builds the retired Electron package;
packaged acceptance is owned once by the signed Tauri preparation workflow.

### WebView2 runner prerequisite

The first hosted portable build and packaged worker parity passed, but the runner
could not create the WebView2 process for UI acceptance. Resolve/install the
Microsoft-signed runtime before isolating LOCALAPPDATA for synthetic saves, and
pass its real Windows path as WEBVIEW2_BROWSER_EXECUTABLE_FOLDER. Forward-slash
paths can make the loader report a missing runtime despite an installed runtime;
the local explicit-runtime smoke passed with the native Windows path. Failed
startup now exports diagnostics, and failed acceptance retains both the complete
candidate package and Cargo cache so diagnosis does not require another build.

### Resume publication without rebuilding

Hosted run 34435249593 produced a clean candidate and passed packaged parity.
Its UI/backend startup logs were healthy, but the runner did not expose the CDP
port to the test client. The exact retained candidate passed both WebView2 UI and
real replacement/restart/cache-cleanup acceptance on the development Windows host.
See evidence/tauri-v071-acceptance.json for the source commit and manifest hash.
The explicit candidate_run signing route verifies that evidence and rejects any
product-source change since that build. It then archives/signs those exact files;
it does not claim that hosted CDP acceptance succeeded or rebuild the executable.
