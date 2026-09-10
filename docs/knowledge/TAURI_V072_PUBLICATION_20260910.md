# Tauri v0.7.2 publication — 2026-09-10

Published stable release:
https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/releases/tag/v0.7.2

## Product identity

- Immutable product source/tag commit:
  `6b7689d75ec43f7f42b813a9ea6af396046930f7`.
- Annotated tag object: `11146880d1dd84778d3902689765ddb8d4c06502`.
- Clean hosted build and complete release gate: run `34513941110`.
- Setup EXE: `Nioh3Studio-0.7.2-win-x64-setup.exe`, 27,324,865 bytes.
- Setup SHA-256:
  `dba0d9a176c75c1db366bf40f7cb330b28c112c9112c28b68728a0fda9ec0ff6`.
- Portable/update ZIP: `Nioh3Studio-0.7.2-win-x64.zip`, 29,683,023 bytes.
- ZIP SHA-256:
  `f332ddb0ca3f6a6b6c8a9ed196fe0309aca4ae7ac569ba2450c699e0c9f38dea`.
- All 756 declared package files match the embedded clean-source manifest.
- The Ed25519 update signature, sidecar hashes, ZIP CRC, archive paths, sizes,
  version, channel, download URL, and exact source commit were verified from the
  hosted workflow artifact and again from the public GitHub release downloads.
- GitHub's latest stable endpoint resolves to `v0.7.2`.

The local copy of the exact hosted assets is under
`deliverables/releases/v0.7.2/`. The setup EXE is the default download for new
users. The complete ZIP remains the input to the in-app updater and a portable
fallback.

## Release acceptance

The successful hosted run passed:

- 571 discovered Python tests;
- the 123-test CPU-only policy suite, with the two expected CUDA skips;
- 47 Node/TypeScript tests;
- 5 Rust tests;
- contracts, generated catalog drift, 554 locale entries, native build
  identity, and isolated native fault injection;
- packaged R3/R4/R5 parity;
- real WebView2 search, empty default filters, Japanese preview, favorites, and
  isolated inventory acceptance;
- real signed update replacement, restart, rollback ownership, old-version
  cleanup, and download-cache cleanup;
- one-file NSIS installation, installed startup through the same WebView2
  driver, manifest verification, and uninstall.

No new live-game mutation was introduced after the previously recorded native
executor and capacity-edit acceptance. Save-file writes now require Nioh 3 to be
fully closed; live addition remains a separate running-game path and retains a
verified automatic backup.

## Failed hosted attempts and permanent fixes

Three earlier candidate runs were intentionally not published:

- `34509911134`: `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` was silently ignored on
  the hosted runner. The Tauri app now accepts a test-only debugging port only
  when the isolated test root is also present and applies it directly through
  the WebView builder.
- `34511567142`: the WebView debugging endpoint became reachable before its
  first page target. Both acceptance drivers now wait for a bounded page target.
- `34512596397`: an auxiliary search completed before the driver could press
  Cancel. The check now accepts either a bounded cancellation or a valid natural
  completion and still requires a terminal state.

These cases are part of `RELEASE_RUNBOOK.md`. A release must use a new clean run
at the exact candidate commit after a fix; failed-run binaries must never be
promoted or rebuilt during publication.

## Deliberate product boundaries

- The release is self-contained and does not require Cheat Engine, Python,
  Node.js, or Electron on the user's machine.
- The independent possessed-Crucible-enemy selector remains unavailable. Seed
  `86872488` proves Crucible terrain and two Nuppeppo occurrences, but the
  per-occurrence possession field has not been identified well enough to expose
  an honest deterministic filter. See
  `CRUCIBLE_POSSESSED_ENEMY_RESEARCH_20260910.md`.
- A web edition remains deferred by product priority.
