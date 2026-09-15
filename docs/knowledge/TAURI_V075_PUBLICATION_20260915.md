# Tauri v0.7.5 publication record (2026-09-15)

## Immutable identity

- Version: `0.7.5`
- Candidate/product commit: `533694ebad21906aecbb6ab5283e04e760ce6c09`
  (parent `a1601bb`), branch `codex/v075-search-hotfix`, promoted to `main`
- Annotated tag: `v0.7.5`, tag object
  `c4cfce1523aa10a2532b79319c1e16ad4fbd6ee7` -> commit `533694e`
- Hosted run: `34936188564`
  (https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/actions/runs/34936188564),
  conclusion `success`, `headSha` equal to the candidate commit
- Public release:
  `https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/releases/tag/v0.7.5`,
  draft `false`, prerelease `false`, `/releases/latest` resolves to `v0.7.5`

## Published assets (six, uploaded unmodified from the workflow artifact)

| Asset | Bytes | SHA-256 |
| --- | --- | --- |
| `Nioh3Studio-0.7.5-win-x64.exe` | 30,988,811 | `a81a30dd72b813c1253bcbca7a8f393fdf51468fed0d997a36a4b4535e7c6d09` |
| `Nioh3Studio-0.7.5-win-x64.exe.sha256` | 97 | sidecar for the outer EXE |
| `Nioh3Studio-0.7.5-win-x64.zip` | 30,396,883 | `262e448e1da0a993b195502e8185b6aa09fe2e3527f6ccb8842e9e40e54473d6` |
| `Nioh3Studio-0.7.5-win-x64.sha256` | 97 | sidecar for the update ZIP |
| `tauri-update.json` | 4,805 | Ed25519-signed; whole-file SHA-256 `9bcc30bbe6be9f8debfb62450046dea56506d7695484f04ee950ebeb337922a` |
| `test-inventory.json` | 76,615 | `nioh3-test-inventory/v1` |

## Verification chain

1. Freeze preflight (`tools/preflight_tauri_release.py --repo . --require-clean
   --expected-sha 533694e...`) returned `ok: true` with a clean tree immediately
   before the push.
2. `origin/main` was `a1601bb` and an ancestor of the candidate; no `v0.7.5` tag
   existed before the atomic push.
3. The hosted workflow passed all gates including the Tauri crate
   `cargo test --locked`, then built the portable directory once, derived the ZIP
   and the outer EXE from it, ran packaged/one-file/update acceptance, and signed
   `tauri-update.json`.
4. Downloaded hosted bytes were verified by an independent local verifier
   (34 checks): sidecars, outer-EXE footer identity (`NIOH3_ONEFILE_V1`, ZIP size
   and ZIP SHA-256), 766 ZIP members against the 765-file
   `build-manifest.json` (no extra or traversal paths, every size and hash
   matching), required runtime paths, `dirty: false` at the candidate commit, and
   the Ed25519 signature against the production public key.
5. Packaged acceptance was repeated locally from the extracted artifact:
   `PACKAGED_R3_R4_R5_PARITY_OK`,
   `TAURI_WEBVIEW2_SEARCH_FAVORITES_INVENTORY_RESTORE_OK`, the add-layout suite,
   `TAURI_REAL_UPDATE_RESTART_CLEANUP_OK`,
   `TAURI_ONEFILE_DIRECT_LAUNCH_CACHE_OK`,
   `TAURI_ONEFILE_REAL_UPDATE_RESTART_CLEANUP_OK`, and
   `SEARCH_CONTINUATION_UI_ACCEPTANCE_OK` on the packaged app.
6. Live icon on the shipped outer EXE: window `独脚踏鞴工作室` reported
   `BIG=none`, `SMALL=256x256` (previously `16x16`).
7. Public redownload of all six assets reproduced the same hashes and passed the
   same 34-check verifier. The live feed
   `https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/releases/latest/download/tauri-update.json`
   is byte-identical to the signed manifest.

## Update discovery

`apps/tauri/src-tauri/src/update.rs` lists the repository's 20 newest releases,
selects releases carrying a `tauri-update.json` asset, validates it, and keeps
the highest stable version. With `v0.7.5` public, non-prerelease, and carrying a
valid signed manifest, existing v0.7.4 clients discover 0.7.5 and can apply it
through the normal prompt, verified replacement, and rollback-retaining flow.

## Boundary

Tests, hosted gates, packaged startup, and synthetic-save flows are bounded
evidence. No new live-game write, save mutation, or in-game acceptance was
performed for this release, and the shipped write contracts are unchanged.
The documentation commit that recorded this release intentionally follows the
immutable product tag, so the branch head after that commit is not the tagged
product source.
