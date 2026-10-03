# Tauri v0.8.3 publication record (2026-10-03)

## Immutable identity

- Product commit: `89b16c3ac86284a98ccd7cc993924d3ae45b3d10`, fast-forwarded
  to `main` before the final preparation dispatch.
- Annotated tag: `v0.8.3`, object `ee32af37d84e6ebc2ac1c3c130867d68b6773979`, pointing at the product
  commit. The publication helper pushed the tag atomically; main already matched.
- Final preparation run: [37153662165](https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/actions/runs/37153662165),
  conclusion `success`, bounded `release` profile (`extended_search=false`).
- [Public stable release](https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/releases/tag/v0.8.3):
  id `402698745`, published `2026-10-03T21:27:52Z`, draft/prerelease `false`.
  Six assets; `releases/latest` resolves to `v0.8.3`.
- Owner publication authorization supersedes the historical tester pause.
  No signing key, secret scope, repository permission or safety boundary changed.

## Published assets

These are the unchanged six files from the final signed workflow artifact.
The supported user download is the outer EXE; the ZIP is the internal update payload.

| Asset | Bytes | SHA-256 |
| --- | --- | --- |
| `Nioh3Studio-0.8.3-win-x64.exe` | 12,124,273 | `83bfbd054c1ffe0a0073ac1113c33bd720c740c052128e925dbf00a7c1679069` |
| `Nioh3Studio-0.8.3-win-x64.exe.sha256` | 97 | `67c3b08d7053547f1c86ccd413ec0b1d03d1e2911dfe4e319bd9d7fe10a2cf14` |
| `Nioh3Studio-0.8.3-win-x64.sha256` | 97 | `be77070489bd0728f347bedf7b5da5c6523ceae2085c49c9ac1cb72d3e02ee90` |
| `Nioh3Studio-0.8.3-win-x64.zip` | 11,423,289 | `cb1b7f4e957adb355b4ff6ea89c14605451298624874621827264d3d26fceeb5` |
| `tauri-update.json` | 4,361 | `fffaad2903a3880acd368baa5b60025c644c433363e1e40e9ae82b883abb9543` |
| `test-inventory.json` | 88,859 | `191205b5355a0fe03cbe637d4c2a8f66c802f51d9dae705af8a22a6de8719f60` |

## Verification and scope

- The final preparation passed packaged parity, synthetic save/UI operations,
  all three host-owned Rust worker identities, one-file independent cold startup,
  poisoned developer-selector isolation, missing-member cache repair, actual
  outer update/restart and rollback. System WebView2 remains a prerequisite.
- `tools/publish_tauri_release.ps1` produced a read-only plan before promotion:
  37 checks over all six assets, including production Ed25519 authenticity,
  clean full source SHA, both sidecars, ZIP CRC/path/member size/hash checks and
  exact outer launcher/footer/embedded payload identity (764 manifest members).
- After publication it re-downloaded the public assets: 53 checks passed,
  `state: published-verified`. A separate remote read confirmed the annotated
  tag, six asset digests, stable release and latest alias.
- Final source [Tests run 37153662482](https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/actions/runs/37153662482):
  `windows-tests` and `rust-packaging` succeeded. Windows unittest ran 778 tests
  (`OK`, 5 skipped); observer pytest passed 75 with 1 skipped. Packaging passed
  12 focused checks and 6 packaged-host checks with 1 skipped.
- At publication the broad `rust-crates` / Python migration lane was still
  running; do not report that full lane as passed. The same historical lane's
  successful run 36373503387 took about 1 hour 48 minutes. It remains enabled
  and running separately from the required bounded release acceptance.
- Local changed-core regression: save 77, runtime 226 and protected 125 unit /
  integration tests passed. Save/runtime/protected all-target Clippy with
  `-D warnings` and formatting passed on Rust 1.91 and 1.99; frontend typecheck,
  35 focused frontend tests and 20 final workflow/preflight tests passed.
- Synthetic save commits cover edit/readback, restore, delete and cart install.
  Packaged receipts record `gameWrites: 0` and `realSaveTouched: false`.
  No new live-game or real-user-save acceptance is claimed. Test inventory is
  a catalog, not evidence that every listed case ran.
- Offline documentation audit has 40 historical link/index findings, identical
  to frozen product 335faef; the publication documents add no new findings.

Source repairs before freezing are documented in the
[experiment failure ledger](../research/EXPERIMENT_FAILURE_LEDGER.md): current
confirmation selectors, the atomic one-shot fault consumer with concurrency
regression, typed complete-chunk iteration for Rust 1.99 lint and the explicit
isolated `NIOH3_BUILD_ROOT` in source CI. No write validation was weakened.

## Remaining boundaries

PC FILEVERSION `2.0.2.0` is the primary evidenced profile; retained older
capabilities and unsupported/experimental labels remain explicit. This release
makes no universal DLC1 or executable-distribution compatibility promise.
Reported menu recognition remains unresolved. The 235896-byte account system
save exceeds the supported 235384-byte format and is still refused, including
operations requiring that transaction member. Bounds, backups, rollback,
single-writer locks, native validation and no-replay recovery remain enabled.
Terrain/spawn feedback is deferred in issue #29.

Independent signing isolation remains incomplete in
[issue #28](https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/issues/28).
The owner-approved temporary repository-secret / trusted-workflow-writer policy
is retained. No enabled release-specific wakeup was found; unrelated schedules
were preserved.

## Local evidence and follow-up documentation

Worktree: `D:\Nioh3_v080_deliverables\source-codex-v083-publication-20261003`.
Reports and downloaded acceptance/public bytes:
`D:\Nioh3_v080_deliverables\deliverables\codex-v083-publication-20261003`.
Use `promotion-publish-89b16c3/publish-result.json`, `public-verify-report.json`,
`plan.json`, `PUBLICATION_REMOTE_STATE.json` and `acceptance-final-89b16c3/`.
The earlier standalone 335faef packet remains unchanged and has different hashes.

This record, the current handoff, index and engineering status are committed
separately after the immutable product tag. The tag is not moved for docs.