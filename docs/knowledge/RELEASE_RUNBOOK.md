# Windows Tauri release runbook

This is the authoritative release procedure for the current Tauri 2 product.
The withdrawn Electron v0.7.0 and the legacy Tk executable use different
package and update formats and must never enter this workflow.

See `V070_HOSTED_BUILD_FIXES_20260909.md` for the historical failures that
established the source, line-ending, native-identity, and hosted WebView2 gates.

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
   `Cargo.lock`, `tauri.conf.json`, the visible-version acceptance, README files,
   and release notes.
5. Commit before packaging. Any later product-code change creates a new candidate
   and invalidates the previous build evidence.

The official builder sets `NIOH3_REQUIRE_CLEAN_SOURCE=1`. A build from a dirty
tree is not a release artifact even if its tests pass.

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
./tools/verify_native_faults.ps1
```

Regenerated contracts, catalogs, and locales must produce no tracked diff.
Native source/DLL/ABI identity must remain exact. Do not update identity hashes
to bless a CRLF checkout or a modified binary. Record unique Python test count
and hardware skips separately from untracked developer tests.

## 3. Build and validate from a clean checkout

Build the portable directory once, test its real workers and WebView2 host, then
derive both downloadable artifacts from that same verified directory:

```powershell
./tools/build_tauri.ps1 -Python $env:NIOH3_PYTHON -Output deliverables/release/portable
python tools/build_tauri_installer.py deliverables/release/portable deliverables/release/Nioh3Studio-<version>-win-x64-setup.exe
npm run test:packaged
node apps/tauri/verify.mjs
node apps/tauri/verify-update.mjs
./tools/verify_tauri_installer.ps1 -Installer deliverables/release/Nioh3Studio-<version>-win-x64-setup.exe -Python $env:NIOH3_PYTHON
python tools/archive_frontend_v2.py deliverables/release/portable deliverables/release/Nioh3Studio-<version>-win-x64.zip
```

The setup EXE is the default download for new users. The ZIP remains the signed
whole-package input to the in-app updater and a portable fallback. Do not point
the updater at the installer and do not distribute the inner application EXE by
itself. Build the installer before archiving: the installer tool applies the
same Tauri NSIS marker to the portable EXE and refreshes its manifest so both
downloads contain byte-identical application binaries.

Verify the installer with an isolated current-user install. Require the installed
root to pass `build-manifest.json` verification and start through the same
WebView2 acceptance driver. Uninstall the isolated test copy afterward.

Limits: ZIP and setup EXE must each be at most 60 MiB. Every manifest entry must
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

## 5. Verify, promote, and publish exact bytes

Download `nioh3-tauri-release` into a new directory. Verify:

- ZIP and setup EXE SHA-256 sidecars;
- `tauri-update.json` through the production Ed25519 public key;
- signed manifest version, filename, official GitHub tag URL, size, and ZIP hash;
- every ZIP member against `build-manifest.json` and ZIP CRC;
- clean source SHA equals the candidate commit;
- extracted package startup and, when changed, isolated installer startup.

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
the setup EXE, its SHA sidecar, the ZIP, its SHA sidecar,
`tauri-update.json`, and the test inventory. Do not rebuild during publication.

After publishing, query the release again, download its public assets, and repeat
hash, signature, ZIP-member, source-SHA, and latest-stable checks. Record the
release URL, tag commit, workflow URL, artifact sizes and SHA-256 values in a new
publication record and update `CURRENT_HANDOFF.md` without moving the tag.

## 6. Product safety gates

- Tests and package smoke checks are bounded evidence. Keep earlier in-game
  acceptance only when the native implementation is unchanged.
- New or changed memory writes require matching live-game acceptance.
- Offline save-file plans and commits require the Nioh 3 process to be fully
  closed. Title screen is no longer sufficient.
- Live addition requires the game to be running, creates a verified backup,
  retries only a non-mutating released preview miss, and never replays an actual
  insertion.
- The updater authenticates the complete ZIP, replaces in place, retains a
  rollback copy until startup handshake, then removes the previous version and
  download cache. Authenticode signing is not currently provided; do not claim
  a Windows publisher certificate.
