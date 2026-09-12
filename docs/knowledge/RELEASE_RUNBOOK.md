# Windows Tauri release runbook

This is the authoritative release procedure for the current Tauri 2 product.
The withdrawn Electron v0.7.0 and the legacy Tk executable use different
package and update formats and must never enter this workflow.

The default product is now one install-free outer EXE. Read
`TAURI_ONEFILE_DELIVERY_20260912.md`. Every release requires explicit owner
authorization before pushing the candidate, creating a tag, or publishing
assets. A local review artifact or a successful workflow is not publication
authorization. The v0.7.3 local-review-only boundary ended when the owner
authorized and completed its publication; do not carry that historical gate
forward as current project status.

See `V070_HOSTED_BUILD_FIXES_20260909.md` for the historical failures that
established the source, line-ending, native-identity, and hosted WebView2 gates.
See `TAURI_V073_PUBLICATION_20260912.md` for the exact v0.7.3 hosted failures,
successful run, promoted hashes, and public-redownload verification.

## 1. Freeze one candidate commit

1. Read `CURRENT_HANDOFF.md`, the previous publication record, and the release
   notes.
2. Inspect `git status`, the complete diff, remote `main`, existing tags, and
   GitHub authentication. Stage only reviewed source, tests, generated contracts,
   generated locales, and release documentation. Never use `git add .` in this
   research checkout.
3. Keep saves, captures, private dumps, signing keys, `.codex_tmp`, local build
   output, and unrelated research outside the commit.
4. Synchronize the version in `package.json`, `package-lock.json`,
   `nioh3_scroll_editor/version.py`, `apps/tauri/src-tauri/Cargo.toml`,
   `Cargo.lock`, `tauri.conf.json`, `apps/launcher/Cargo.toml` and its lockfile,
   the visible-version acceptance, README files,
   and release notes.
5. Commit before packaging. Any later product-code change creates a new candidate
   and invalidates the previous build evidence.

The official builder sets `NIOH3_REQUIRE_CLEAN_SOURCE=1`. A build from a dirty
tree is not a release artifact even if its tests pass.

Query the current GitHub release state before selecting a version: publication
records describe the event, but an owner can subsequently withdraw a release
to draft. Never infer the current latest download from an old publication note.

## 2. Run local checks in failure-cost order

Use an explicit Python executable through `NIOH3_PYTHON`; do not assume `python`
is on PATH. Run:

```powershell
python tools/export_knowledge_catalog_manifest.py
python tools/export_v2_ui_locales.py
node tools/audit_v2_ui_locales.mjs
python tools/verify_native_build_manifest.py
python tools/write_test_inventory.py --output deliverables/release/test-inventory.json
python tools/run_cpu_only_tests.py
python -m unittest discover -s tests -t . -v
npm test
npm run typecheck
cargo test --locked --manifest-path apps/tauri/src-tauri/Cargo.toml
cargo test --locked --manifest-path apps/launcher/Cargo.toml
./tools/verify_native_faults.ps1
```

Regenerated contracts, catalogs, and locales must produce no tracked diff.
Native source/DLL/ABI identity must remain exact. Do not update identity hashes
to bless a CRLF checkout or a modified binary. Record unique Python test count
and hardware skips separately from untracked developer tests.

## 3. Build and validate from a clean checkout

The hosted workflow may perform the one clean release build after local source
checks. In that case download its exact artifacts for local WebView2, one-file,
update, and live-game acceptance before publication; do not build a redundant
local candidate merely to repeat the hosted compilation.

Build the portable directory once, test its real workers and WebView2 host, then
derive both downloadable artifacts from that same verified directory:

```powershell
./tools/build_tauri.ps1 -Python $env:NIOH3_PYTHON -Output deliverables/release/portable
python tools/archive_frontend_v2.py deliverables/release/portable deliverables/release/Nioh3Studio-<version>-win-x64.zip
python tools/build_tauri_onefile.py deliverables/release/Nioh3Studio-<version>-win-x64.zip deliverables/release/Nioh3Studio-<version>-win-x64.exe
npm run test:packaged
node apps/tauri/verify.mjs
node apps/tauri/verify-update.mjs
$env:NIOH3_ONEFILE_EXE=Join-Path $PWD 'deliverables/release/Nioh3Studio-<version>-win-x64.exe'
node apps/tauri/verify-onefile.mjs
node apps/tauri/verify-onefile-update.mjs
```

The outer EXE is the default player download: it launches directly without
installation or manual extraction. The ZIP remains the signed internal input to
the updater, including older Tauri directory-mode clients. Keep the manifest-owned
`launcher/Nioh3Launcher.exe` inside that ZIP so a one-file update can reconstruct
the new outer EXE. Never advertise the inner application EXE as self-contained.
Do not run the archived NSIS builder against the new candidate; it patches the
inner application and would invalidate the already verified package.

Verify direct launch with isolated local app data, no installation registration,
cache reuse/pruning, and the real WebView2 acceptance driver. Verify both the
legacy directory updater and outer-EXE replacement, acknowledgement, rollback
and cleanup. Keep previous installer artifacts only as historical local evidence.

Limits: ZIP and outer EXE must each be at most 60 MiB. Every manifest entry must
match its size and SHA-256; the archive must contain no traversal paths or extra
files. The manifest must record the exact candidate SHA and `dirty: false`.

## 4. Prepare signed hosted artifacts

Push the candidate branch and dispatch `release.yml` on that exact ref:

```powershell
gh workflow run release.yml --ref codex/tauri2-migration
gh run list --commit <candidate-sha> --json databaseId,headSha,status,conclusion,workflowName
```

Inspect runs by exact commit SHA. The workflow installs locked dependencies,
runs source and native-fault tests, builds from a clean Windows checkout, tests
the packaged workers and app, builds both downloads, and signs
`tauri-update.json`. It prepares artifacts only; it does not publish a release.

There is no candidate-reuse branch in the current workflow. The v0.7.1
signing-only rescue route was tied to one historical acceptance record and was
removed so it cannot accidentally emit a later release with hard-coded v0.7.1
names.

If the run fails, inspect `gh run view <run-id> --log-failed`, fix the observed
cause, commit the fix, and dispatch the new SHA. Do not repeatedly rebuild the
same known-bad commit. UI timeouts require captured UI/log evidence; retries do
not turn a failed acceptance into a pass.

Packaged WebView2 acceptance passes its isolated debugging port through
`NIOH3_TAURI_TEST_DEBUG_PORT`. The application applies that value directly to
the test-only WebView builder when `NIOH3_TAURI_TEST_ROOT` is also present.
Do not replace this with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`: GitHub hosted
Windows runners have launched a healthy application while silently omitting
the environment-only remote-debugging switch, leaving Playwright unable to
inspect the running UI.

After the debugging endpoint becomes reachable, wait for WebView2 to publish
its first page target before selecting the page. Hosted runners can expose
`/json/version` several seconds before the Tauri page appears.

UI search acceptance must allow a bounded search to complete before the test
can press Cancel. The hosted runner can finish an auxiliary-only page faster
than a local workstation, so a disabled Cancel button is a valid completed
state rather than an acceptance failure.

### 4.1 Hosted Windows failure patterns retained from v0.7.3

The failed v0.7.3 runs are reusable release-gate evidence, not disposable CI
noise:

| Run | Symptom | Root cause | Permanent rule |
| --- | --- | --- | --- |
| `34687187920` | Ten save-race tests failed only on the hosted runner. | Windows temporary paths appeared in both long and 8.3 short forms, so textual path comparison rejected the same resolved file. | Resolve fixture and observed paths before comparing file identities. Never weaken the transaction assertions merely because the runner uses short paths. |
| `34687811828` | The native maximize assertion failed on a 1024x768 desktop. | The initial 1028x779 window was already larger than the work area, so a correct maximize operation reduced rather than enlarged it. | Validate that the maximized window matches or stays within the work area. Do not require width or height growth. |
| `34688734252` | The layout gate found a one-pixel sidebar drift and controls below the initial fold. | A real CSS alignment regression was mixed with an invalid assumption that every reachable control must fit in the first viewport on the small native desktop. | Keep exact alignment checks for geometry defects, but test first-viewport fit and scroll reachability as separate contracts. |
| `34689783181` | Comprehensive WebView2 acceptance could not find a CPU setting. | The UI had correctly changed the control from an ARIA checkbox to a switch, while two acceptance drivers retained the obsolete role. | Locate controls by their shipped semantic role and update every acceptance driver in the same product change. Do not replace the role check with an unscoped text selector. |

After any hosted failure, preserve the run ID and evidence, commit the actual
fix, and dispatch a new clean candidate SHA. v0.7.3 was published only from the
subsequent successful run `34690776011` at
`893996e4c11a9b0c20b125c696c89a0a47ec9048`.

## 5. Verify, promote, and publish exact bytes

Download `nioh3-tauri-release` into a new directory. Verify:

- ZIP and outer EXE SHA-256 sidecars and the embedded ZIP footer/hash;
- `tauri-update.json` through the production Ed25519 public key;
- signed manifest version, filename, official GitHub tag URL, size, and ZIP hash;
- every ZIP member against `build-manifest.json` and ZIP CRC;
- clean source SHA equals the candidate commit;
- extracted package startup, direct outer-EXE startup, and updater acceptance.

Refresh remote `main`. Require `origin/main` to be an ancestor of the candidate.
Create an annotated version tag at the verified SHA, then push `main` and the tag
atomically. Never move an existing release tag.

```powershell
git fetch origin main
git merge-base --is-ancestor origin/main <candidate-sha>
git tag -a v<version> <candidate-sha> -m "Release v<version>"
git push --atomic origin <candidate-sha>:refs/heads/main refs/tags/v<version>
```

Create the GitHub release from the already verified workflow downloads. Upload
the outer EXE, its SHA sidecar, the internal update ZIP, its SHA sidecar,
`tauri-update.json`, and the test inventory. Do not rebuild during publication.

Treat release state as an explicit progression:

1. Pushing the candidate branch makes source available remotely; it does not
   create a version tag, a GitHub Release, downloadable assets, or an update
   feed.
2. A successful `release.yml` run produces hosted artifacts; it still does not
   publish them.
3. An annotated tag fixes the immutable product commit; it does not by itself
   create a GitHub Release page or upload files.
4. A public, non-prerelease GitHub Release with all six verified assets makes
   the player EXE and update metadata public.
5. Publication is complete only after `/releases/latest` resolves to the new
   tag and all public downloads pass the same hash, signature, archive, and
   source-commit checks as the hosted artifacts.

The six expected assets are the outer EXE, EXE SHA-256 sidecar, update ZIP, ZIP
SHA-256 sidecar, `tauri-update.json`, and `test-inventory.json`. The update feed
is not considered published merely because a signed manifest exists in a
workflow artifact; the matching manifest and ZIP must be attached to the public
release at their signed URL.

After publishing, query the release again, download its public assets, and repeat
hash, signature, ZIP-member, source-SHA, and latest-stable checks. Record the
release URL, tag commit, workflow URL, artifact sizes and SHA-256 values in a new
publication record and update `CURRENT_HANDOFF.md` without moving the tag. The
documentation commit may follow the immutable product tag and move `main`
forward; record that separation so the later branch head is not mistaken for
the tagged product source.

## 6. Product safety gates

- Tests and package smoke checks are bounded evidence. Keep earlier in-game
  acceptance only when the native implementation is unchanged.
- New or changed memory writes require matching live-game acceptance.
- Generated-scroll append, permanent edits/deletions, and backup restoration
  permit title-screen use or a closed game. The UI collects the appropriate acknowledgement; process presence
  is not a native title-screen detector.
- The owner closed the unreproduced title-save and intermittent live-add reports
  pending a fresh affected save and log. They are not release blockers; closure
  is not a proven root-cause fix. Preserve automatic verified backups,
  pre-restore checkpoints, transaction identities and no-replay recovery. Do not
  restart deferred native-save/possessed-enemy research as a packaging gate.
- Live addition requires the game to be running, creates a verified backup,
  retries only a non-mutating released preview miss, and never replays an actual
  insertion.
- The updater authenticates the complete ZIP, replaces the outer EXE in one-file
  mode or the runtime directory in legacy mode, and retains a
  rollback copy until startup handshake, then removes the previous version and
  download cache. Authenticode signing is not currently provided; do not claim
  a Windows publisher certificate.
