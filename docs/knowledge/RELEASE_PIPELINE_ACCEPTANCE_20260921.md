# Release pipeline acceptance - 2026-09-21

Bounded acceptance record for the release-preparation pipeline and its read-only
promotion path. Pipeline integration only: no product release or `v0.8.0` change.

## Accepted identity

| Item | Value |
| --- | --- |
| Preparation run | `35633923489` - success |
| Preparation run URL | https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/actions/runs/35633923489 |
| Accepted source SHA | `33040400fe3243b6ced7549d4c5e353e0c2fb38b` |
| Dispatch | `workflow_dispatch`, `extended_search=false`, exactly one dispatch |
| Actual acceptance profile | bounded `release`; the 158M-trial search regression did not run |
| Promotion check run | `35636033524` - success, read-only, `publish=false` |
| Promotion run URL | https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/actions/runs/35636033524 |
| Hosted evidence | `D:/Nioh3_v080_deliverables/deliverables/release-pipeline-20260921/hosted/HOSTED_PREPARATION_REPORT.md` |

## Source and integration

One frozen candidate commit, staged by explicit path only (18 files: 15 modified
tracked, 3 new publication files); no `git add -A`, and unrelated untracked
research files were preserved. `main` was fast-forwarded
`bf85fb73468a1d183a4ac0170f779dad8decf909..3304040` after verifying that
`origin/main` was the expected base, an ancestor of the accepted SHA, and the
only incoming commit. Branch `codex/release-pipeline-20260921` and `main` carry
the same SHA.

## Timings (preparation run)

Job `release` 15m23s (2026-09-21T17:44:28Z-18:00:02Z). Prerequisites and source
gates ~97s; `Build the candidate from a clean checkout` 10m10s; package,
packaged E2E, one-file/update/rollback and signing ~2m42s (release binaries
77s, packaged Rust identity/host/frontend 39s, one-file launch/update/rollback
27s, budget and signing 1s); uploads 12s; rust-cache save 44s.

## Gates that ran

Contracts regeneration with no generated diff, UI locale export and audit,
native build manifest identity, test inventory, frontend typecheck, native
fault matrix, packaged Rust graph assertion (no Python worker, declared
exclusions, per-binary hashes), packaged parity/host/layout/update checks,
packaged host package record, packaged worker identities for `offline_search`,
`save` and `runtime`, shipped frontend against the packaged graph, direct
one-file launch, runtime cache, outer update and rollback, the 60 MiB download
budget and production Ed25519 signing.

## Retained artifacts

`nioh3-tauri-release` (19,940,588 bytes: ZIP, outer EXE, both SHA-256 sidecars,
`tauri-update.json`, `test-inventory.json`) and `tauri-acceptance-evidence`
(3,648,077 bytes). The `if: failure()` candidate-retention step was skipped,
which is the positive signal that no leg failed. Artifact bytes were not
downloaded for this record; identities come from run and artifact metadata.

## Read-only promotion check (run 35636033524)

Independently confirmed by the promotion worker, with the mutation branch never
executing: the typed `publish` job condition evaluated false and the job shows
`skipped`; `plan.json` records `mode=plan`, `planned_mutations=[]`,
`tag_state=same`, `release_state=matching`; no `publish-result.json` exists
because that branch never ran. It targeted the already-published preparation run
`35625590622` (`candidate_sha` `3798693c...`, version `0.8.0`), not the new
assets. Its verification job was green in local mode:
`verify-report.json` 37 checks, 0 failed, outer EXE SHA-256
`2d5fde55da28a75c2859cfaf320d2d2fe6942244fc361733425e8f5f30cd05cd`, consistent
with the published asset. Evidence:
`D:/Nioh3_v080_deliverables/deliverables/release-pipeline-20260921/hosted-promotion/`.

## Immutability and product state

- `refs/tags/v0.8.0` is still the same annotated tag object
  `8f5c1d0c39a9c2081d7895eda5af71794e81f2a6` (commit `3798693c...`); remote
  tag count 40 before and after.
- Release `v0.8.0` (id 393136624) is still published, not draft or prerelease,
  at 2026-09-21T16:58:39Z, targeting `main`, with its same six assets and
  sizes: EXE 11,276,251; EXE sidecar 97; ZIP sidecar 97; ZIP 10,684,323;
  `tauri-update.json` 4,037; `test-inventory.json` 87,365.
- Integrity here is the payload digest plus the production Ed25519 signature and
  sidecars; GitHub reports `isImmutable` as `false`. Non-blocking annotation:
  the pinned actions target Node 20 and are force-run on Node 24.

## Independent development CI - not claimed

The main push also triggered the ordinary push-triggered development lanes for
the accepted SHA: `Tests` `35635824867` and `Frontend V2 foundation`
`35635824912`, both `in_progress` with empty conclusions at observation. They
are outside this bounded release trial: no outcome was investigated and no
green claim is made. From job/step metadata rather than logs, that lane was
already red on the previous main SHA `3798693c...` (`rust-packaging` step 9,
`rust-crates` step 6).

## Limits

Bounded package and synthetic-fixture acceptance for the PC v2.02 path only; it
is not live-game, GPU, visual or player acceptance, and not a publication. No
tag, GitHub release, asset, update-feed or `-Publish` action ran; no timeout was
raised and no gate was weakened. Still unexercised on hosted infrastructure: the
`-Publish` mutation branch (local simulation only), the post-publish public
re-download path, and the publish job's token scoping and `contents: write`.
