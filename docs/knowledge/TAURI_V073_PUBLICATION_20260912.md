# Tauri v0.7.3 publication — 2026-09-12

Published stable release:
https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/releases/tag/v0.7.3

## Product identity

- Immutable product source/tag commit:
  `893996e4c11a9b0c20b125c696c89a0a47ec9048`.
- Annotated tag object: `84218abf850282f9b8da34d3aaa325c795727878`.
- Clean hosted build and complete release gate: run `34690776011`.
- Single-file player EXE: `Nioh3Studio-0.7.3-win-x64.exe`, 30,724,544 bytes.
- EXE SHA-256:
  `2c1766751b0746eede917b04e2f4edac9b4c3348893349e2de4c3e38e84f9513`.
- Portable/update ZIP: `Nioh3Studio-0.7.3-win-x64.zip`, 30,132,616 bytes.
- ZIP SHA-256:
  `d3a2113b9fa8b576a620589eb05bc5d3e35b5e64e40bd76efeb1d51d6d20df7e`.
- Update manifest: `tauri-update.json`, 4,007 bytes, SHA-256
  `f5762d611c20abec1d4e5c1d259a60bd3839270b504cf84610ce418d6910621b`.
- Test inventory: `test-inventory.json`, 70,892 bytes, SHA-256
  `542a04be7bb426861fae4c821c12c0885e6a7e7fd12873f012f8b04cc33fdeff`.
- All 764 declared package files match the embedded clean-source manifest.
- The outer EXE footer and embedded ZIP boundaries, both sidecar hashes, ZIP
  CRC and safe paths, required runtime, exact source commit, version, platform,
  channel, download URL, asset size and hash, and production Ed25519 signature
  were verified from the successful hosted artifact and again from public
  GitHub release downloads.
- GitHub's latest stable endpoint resolves to `v0.7.3`.

The exact hosted products and acceptance evidence are under
`F:/Nioh3_ScrollEditor/deliverables/releases/0.7.3-hosted-34690776011/`.
An independent copy downloaded from the public release is under
`F:/Nioh3_ScrollEditor/deliverables/releases/0.7.3-public-verification-20260912/`.

## Release acceptance

The successful hosted run passed:

- 631 discovered Python tests, with the documented hardware-policy skips;
- 57 Node tests and TypeScript checking;
- both Rust test groups;
- contracts, generated catalog and locale drift checks, native build identity,
  and isolated native fault injection;
- strict packaged R3/R4/R5 parity;
- real WebView2 startup, worker handshake, empty default filters, search,
  Japanese UI, favorites, isolated inventory, backup restoration, permanent
  editing and deletion, and automatic diagnostic clipboard behavior;
- the responsive addition, collection, help, settings, modal, maximized-window,
  and DPI/viewport layout matrix;
- direct outer-EXE launch twice, embedded payload validation, bounded cache
  reuse and pruning, unchanged installation registry, and preservation of
  unrelated files; and
- a real outer-EXE update and restart with distinct replacement bytes, actual
  launcher/application process waits, worker handshake, completed receipt, and
  cleanup of previous/download files.

All save-file operations in this publication gate used encrypted synthetic
fixtures. Packaging and update acceptance recorded `gameWrites: 0`; no new
live-game mutation was performed for v0.7.3.

## Failed hosted attempts and permanent fixes

Four earlier candidates were intentionally not published:

- `34687187920`: save-race assertions compared unresolved long paths with the
  hosted runner's Windows 8.3 short paths. Fixtures now resolve paths before
  comparing identities.
- `34687811828`: the maximize check assumed that maximizing must enlarge the
  window, but the hosted 1024x768 desktop correctly reduced an oversized
  starting window to its work area. The check now validates work-area bounds.
- `34688734252`: the UI gate found a real one-pixel sidebar alignment drift and
  also conflated scroll reachability with first-viewport fit on the native
  runner. The CSS alignment was corrected and the two layout contracts are now
  tested separately.
- `34689783181`: the full WebView2 verifier still located a CPU setting by its
  obsolete checkbox role after settings moved to switches. Both acceptance
  drivers now use the actual switch role.

Each fix was committed before a new clean hosted run. No binary from a failed
run was promoted or rebuilt during publication.

## Deliberate product boundaries

- The supported player download is the outer
  `Nioh3Studio-0.7.3-win-x64.exe`. It is a single file and requires no installer,
  manual extraction, Python, Node.js, Electron, or Cheat Engine.
- On first launch, the outer EXE validates and extracts its embedded application
  to a bounded LocalAppData cache. It uses the Windows WebView2 system runtime;
  the cache is an implementation detail, not an additional file the player must
  download or place beside the EXE.
- The inner `Nioh3Studio.exe` inside the update ZIP depends on its packaged
  runtime tree and is not a standalone substitute for the outer player EXE.
- The independent possessed-Crucible-enemy selector and a web edition remain
  outside this release. Historical unconfirmed title-save and intermittent
  live-add reports are closed pending a fresh affected save and diagnostic log;
  that disposition is not a demonstrated root-cause claim.
