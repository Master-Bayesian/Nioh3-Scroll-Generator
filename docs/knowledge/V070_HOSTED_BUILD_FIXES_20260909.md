# v0.7.0 hosted build fixes

## Native source identity on Windows checkouts

The first hosted preparation of commit
`375bbc27dbcc040e093c0edd75293d828142888f` reproduced the freeze baseline's
native source identity failure. A local copy of the working files had passed,
but a fresh Git checkout with `core.autocrlf=true` converted the CUDA source to
CRLF. Its hash became
`c4f3ea4ac18bb1f236ef0649a854e94cfd610cae9c36003c21928e1dc0e3e61c`
instead of the DLL's recorded LF source hash
`7826ab5226d82e8825eff32c81cdf6650129e46d6919cb04a58b7ea08065acf7`.

The repair pins native source files to LF in `.gitattributes`. It preserves the
strict source/DLL/ABI check and does not alter numerical code, the build manifest
or the accelerator DLL. Validation must use a real Git checkout with Windows
line-ending conversion enabled, not just a copy of the developer's files.

Hosted release preparation runs against the pushed commit, verifies the actual
packaged workers and UI, and signs its update manifest using the existing
repository secret. The version tag is pushed only after this preparation passes.
The tag workflow rebuilds, verifies, signs and publishes the release assets.

## Windows temporary-directory aliases

The review integration test compared a canonical application state path with an
unresolved temporary path. GitHub's Windows runner can expose `TEMP` through an
8.3 alias, so both paths referred to the same directory while their strings differed.
Resolve the fixture root before comparing it. Keep the assertions that changing
the next-launch data directory does not move the currently active operations.
Do not weaken application path canonicalization to accommodate a test fixture.

## Numerical tests on runners without CUDA

Twenty-three legacy numerical tests called native bulk helpers without entering
an explicit CPU-permitted execution policy. They passed on the developer's GPU,
but the same calls correctly failed closed on a hosted runner without CUDA.
`NIOH3_PARITY_ALLOW_CPU` only configures the packaged replay driver; it does not
change the native DLL's policy or these lower-level tests.

Numerical test fixtures now explicitly enter the permitted CPU policy and pass
the corresponding solver flag. The application default remains strict, and the
freeze tests still assert that forced CUDA failures cannot enter CPU execution
without explicit permission. `python tools/run_cpu_only_tests.py` exercises these
suites through the real native DLL with CUDA fault injection, even on a developer
machine with a healthy GPU. The release workflow runs this gate as well.

## Cart controls on compact Windows desktops

The connected UI test reached real search results and then could not click the
cart button: the install-mode section intercepted pointer events. The scroll
card had a deliberate fixed height, but its surrounding container also had a
fixed height. Action buttons wrapping on a narrow display overflowed that
container and overlapped the next section.

Keep the scroll card's fixed rows and height. Give the surrounding result and
action containers a minimum height and allow them to grow with wrapped controls.
The result pane remains scrollable. The connected synthetic-save test now requests
a compact 1280x800 content area, asserts that the cart button stays above the save
picker, and performs the real click and selected-item add. Do not replace this
with a forced Playwright click or an artificially enlarged test window.

## Repeated unittest discovery

`test_cart_batch.py` imported another module's `TestCase` class to reuse its
fixture. Unittest discovered that imported class again, so the earlier 569 test
executions represented 562 distinct test IDs. All executions passed after the
environment fixes, but the separate inventory gate correctly rejected duplicates.

Import the module and reference its fixture through the module, keeping the
class out of the importing module's discovery namespace. Verify that the unique
ID set before and after the change is identical. The inventory now reports 562
unique tests, with no tests removed. The release workflow runs inventory validation
before lengthy tests and packaging; errors report the duplicate IDs or discovery
exceptions directly. Keep this gate enabled and use its count in release reports.

## Electron's binary must be installed explicitly

Electron 44's npm package has no postinstall download. Its executable is installed
lazily when a launcher first resolves it. The foundation workflow launched Electron
before packaging, while the release workflow went directly from `npm ci` to
packaging. Only the latter exposed the missing `node_modules/electron/dist` folder.

The packager now invokes the installed Electron package's own `install.js` before
copying the distribution. That installer validates the pinned package's checksums
and treats an already installed matching version as a no-op. Test this with a fresh
dependency directory and no prior Electron launch. Preserve the release workflow's
ability to build directly from dependency installation; a smoke launch is not a
substitute for a build prerequisite.

## A development label is not an artifact identity

The first complete package passed all runtime checks but signing stopped with
`UPDATE_ASSET_INVALID`. The archive workflow used the `Nioh3ScrollEditor-` prefix,
while the updater unnecessarily required `Nioh3ScrollEditorV2-` ZIP files. The
private key was available; this was metadata validation, not a signing-key failure.

The initial workaround renamed the archive to match that restriction. The user
correctly identified this as a design bug: a development label must not determine
whether an official package is trusted. The final repair accepts safe ZIP basenames
without coupling them to a product name. It rejects path separators, control
characters, invalid Windows filename characters and reserved device names. The
signed download URL must identify the declared version and exact filename in the
official repository. Signature, platform, version, archive hash and complete
package-manifest checks remain mandatory.

Use `Nioh3ScrollEditor-<version>-win-x64.zip` consistently in archive creation,
the signer input, download URL and release notes. A regression test reads the
actual release workflow, checks agreement between those fields, signs the metadata
with a disposable test key and passes it through the real updater validator.
Additional tests accept renamed packages and reject unsafe names and mismatched
URLs even when signed. Run this check before the expensive release gates.

## Wait for backup refresh completion, not a previously satisfied row count

A later packaged run timed out waiting for the recycle success message after
restore. The test's `row count >= 3` condition was already true before refresh,
so it did not establish that refresh had completed. Inspection also found that
restore's local busy flag could clear while its newly triggered automatic backup
refresh was still using the shared protected-operation observer.

The backup view now observes the actual protected-operation state. Automatic
loading waits for the previous action, and both actions and selection stay
disabled while a request is active. The test waits for the backup page to become
idle and verifies the restore-created backup before selecting an item to recycle.
Connected-test failures now print and save the visible UI status so another
failure will include the actual error, rather than only a success-message timeout.
Do not hide this failure with a forced click, a broad retry or removal of the
real recycle operation.

## Terminal job publication preceded thread exit

The next hosted run exposed the underlying failure as `BUSY: protected operation
is still running`. `ProtectedJobs` published a terminal state before its final
sequence update and before `Thread.is_alive()` became false. A fast follow-up
inventory or backup request therefore received BUSY after observing completion.
The observer correctly treated a rejected protected submission conservatively;
the resulting locked controls were a symptom, not permission to replay writes.

The worker now publishes the terminal state, result/error and final sequence
together, then joins a terminal owner outside the state lock before accepting
the next request or confirming idle shutdown. An actually active action is still
rejected as busy, never cancelled or replaced. Worker closures retain their own
job object so final updates cannot mutate a later job.

A deterministic regression keeps a real thread alive after its target has
completed. Both successful and failed actions reproduced the old BUSY rejection;
the repaired implementation waits for owner exit and accepts exactly one next
request. Existing active-write, cancellation, receipt and hook-ownership tests
remain enabled. This added one distinct Python test: subsequent inventories
contain 563 unique IDs, while the earlier deduplication result remains 562.
