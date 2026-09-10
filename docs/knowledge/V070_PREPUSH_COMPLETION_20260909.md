# v0.7.0 pre-push completion - 2026-09-09

This report supersedes the pending-gate lists in the earlier V070 review, RC1
follow-up and legacy parity reports. It describes local release preparation;
it does not claim a pushed tag, hosted CI run, public release or production signature.

## Completed product work

- Version metadata is synchronized at 0.7.0 across Python, npm and the portable
  executable. The executable uses the existing scroll icon and product metadata.
- Search, native early-playthrough generation, save editing, selected cart batch
  addition, backup management, favorites, three UI languages, recent-three history
  and bounded diagnostics are connected. The Tk source entry remains available.
- The native live-add executor is independent of CE. Exact selected-item addition,
  automatic backups and normal save/reload were already accepted in game; see
  [the real UI evidence](V070_RC1_LIVE_UI_ACCEPTANCE_20260909.md).
- Current remaining attempts have a separate reviewed editor command. The saved
  slot resolves privately to the full instance serial; the live instance is checked
  against the saved defined fields, allowing only the current count/new-item flag
  to differ. Preparation creates and verifies a normal backup. Confirmation
  rechecks the source, backup, process lifetime, inventory owner and complete record,
  writes only byte 0x33, and reads back the complete record.
- Count range is 0-7. PC v2.01 and the previously accepted zero 0x0E side field are
  required. Changed input invalidates a prepared confirmation. Durable claim/receipt
  files prevent replay after duplicate requests or uncertain completion. Recovery
  reads the same instance; it never retries the memory write. The renderer receives
  no record bytes, process pointers or private source path.
- Temporary maximum attempts remain separate, seed-scoped and bounded to 1-7.
  The existing R3 6/7 -> 2/5 and R4 3/4 -> 2/6 acceptance is reused. The user declined
  a redundant same-value game test; no additional game write or restoration was
  performed during this release-preparation pass. Formal count-button transitions
  are covered by simulated receipts, with the real transaction tested separately.
- English/Japanese navigation widths and English value controls accommodate longer
  labels. Search/editor/backups are checked at multiple window sizes and Chromium
  zoom factors. This does not substitute for native Windows DPI or every display.

## Release and reproducibility repairs

- The old tag workflow built only the legacy single EXE. The replacement builds
  and tests the complete V2 package, archives only manifest-listed files, and signs
  `v2-update.json` using the existing `UPDATE_SIGNING_PRIVATE_KEY_BASE64` secret.
  The V2 pinned public key matches the legacy key. Manual workflow dispatch retains
  preparation artifacts; only a pushed version tag publishes a release.
- v0.6 users need a one-time full ZIP download. No incompatible V2 ZIP is offered as
  a legacy single-EXE update. V2 uses whole-directory replacement and rollback.
- Portable documentation, release notes, icon/version stamping and ZIP checksums
  are part of the build. Executables do not have an Authenticode certificate;
  the local file manifest is an integrity check rather than a publisher signature.
- Five old verification drivers no longer hardcode the developer's Python path.
  A test that depended on a private `captures/` file now uses a sanitized, committed
  fixture. Its exact effect/counter transition assertions remain active.
- A selected source inventory excludes captures, user saves, build debris and
  unrelated root experiments. A separate clean source copy installs dependencies,
  builds and runs tests. Pattern-scan findings were reviewed: artificial Steam IDs
  belong to tests, and DLL key-like matches were adjacent Windows locale strings.

## Evidence boundaries and release operation

Final local checks: 569 Python tests from the clean source copy; 36 desktop/worker
tests against the bundled workers; packaged R3/R4/R5 replay parity; Electron startup
and search smoke; 18 connected production save/cart/backup checks; collection
persistence/copy/language checks; 16 release-surface/count-state assertions and
27 language/window/zoom observations. Native source/DLL/ABI identity is unchanged.
ZIP creation checks every archived file against the complete 152-file product
manifest; the archive adds the manifest itself as its 153rd file.

The latest local evidence and distribution are under
`deliverables/frontend-v2/release-0.7.0/`. The parent directory contains the full
test logs, source inventory, privacy report and previous test runs.

Already accepted in preceding passes: native insertion across nine context/rarity
cells, actual packaged favorites/cart subset addition with save/reload, R3/R4
temporary capacity changes, current-count byte edits, native fault fixtures, and
signed local whole-package replacement/rollback.

Not claimed: multiplayer propagation, natural early-R4/R5 drops, all-vendor GPU
performance, native-speaker sign-off for every translation, native Windows DPI
coverage, or the old white-screen/Discord issue being universally resolved.
The accepted game-side R5 normalization/icon discrepancy is deliberately unchanged.

Once the reviewed source is pushed, run the hosted preparation workflow on that
exact commit. Its private signing key is held by GitHub Actions. Inspect the artifact
and hosted result before pushing the version tag. Local tests cannot stand in for
that hosted result or a production-key signature that has not yet been generated.
