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
