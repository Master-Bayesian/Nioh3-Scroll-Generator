# Tauri v0.8.0 publication record (2026-09-21)

## Immutable identity

- Version: `0.8.0`
- Candidate/product commit: `3798693c48cef2238480da66dc0cc0d2a098c78b`,
  promoted to `main` by the same atomic push that created the tag
- Annotated tag: `v0.8.0`, tag object
  `8f5c1d0c39a9c2081d7895eda5af71794e81f2a6` -> commit `3798693`
- Hosted run: `35625590622`
  (https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/actions/runs/35625590622),
  conclusion `success` in 29m40s, `headSha` equal to the candidate commit
- Public release:
  `https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/releases/tag/v0.8.0`,
  release id `393136624`, published `2026-09-21T16:58:39Z`, draft `false`,
  prerelease `false`, six assets; `/releases/latest` resolves to `v0.8.0`

The manifest inside the published ZIP records `dirty: false` and the exact
product commit `3798693c48cef2238480da66dc0cc0d2a098c78b`.

## Published assets (six, uploaded unmodified from the workflow artifact)

| Asset | Bytes | SHA-256 |
| --- | --- | --- |
| `Nioh3Studio-0.8.0-win-x64.exe` | 11,276,251 | `2d5fde55da28a75c2859cfaf320d2d2fe6942244fc361733425e8f5f30cd05cd` |
| `Nioh3Studio-0.8.0-win-x64.exe.sha256` | 97 | `ea72b9d1fb4aa1b65994e798767bf735995e04c6fa2b8a38ebc31a5e0d69528b` |
| `Nioh3Studio-0.8.0-win-x64.zip` | 10,684,323 | `8a15b0b1eb2c8a6d7fb4a64cfff7d963329ea7eb0956b68018810bfd7d523c31` |
| `Nioh3Studio-0.8.0-win-x64.sha256` | 97 | `641681598ae38a3f90bd5f88811093c86fc222760f49e9d3da9646b8aaa6e304` |
| `tauri-update.json` | 4,037 | `98c29aa16f42aaa87e5c95ac98d3dfd07a3f289ca751918cd277e7a5517e0ec4` |
| `test-inventory.json` | 87,365 | `f343b12fdde642d660d10192e2863904169faa3cafb8742cce46c217161f695c` |

## Verification chain

1. The hosted workflow passed every required gate: the six crate suites, the CI
   game-identity fixture, the packaged R3/R4/R5 parity gate, `verify.mjs`,
   add-layout, update, host resolution, the three worker-identity roles, and the
   packaged-frontend regression (CPU-only search, allow-CPU true, seed
   `226061463` at cursor `158614759`, 294,390 ms on the hosted runner). Direct
   one-file launch, outer update, rollback and signing also passed. The three legacy Python steps
   were intentionally skipped as the retired parity/oracle lane.
2. Hosted artifact verification confirmed the production Ed25519 signature, 764
   ZIP members against 763 manifest entries, the outer-EXE payload and footer,
   and the 60 MiB size budget.
3. An independent, unauthenticated public re-download re-verified the six
   assets: all six downloads identical to the hosted copies, Ed25519 signature,
   CRC coverage with no corrupt member, 764 members with zero hash or size
   mismatches, `source_dirty_false` with the candidate SHA, and the outer footer
   payload hash equal to the ZIP. The run passed 27/27 checks, and
   `/releases/latest` plus `latest/download/tauri-update.json` resolved to
   `0.8.0`.

## Game boundary

The accepted live-game scope is unchanged and narrow: the PC v2.02 native add
and persistence path at seed `123456`, accepted earlier. This release performed
no new game writes, and every other unverified native write stays disabled.
Nothing here is native-write acceptance, save-reload acceptance, or a broader
game-compatibility claim.

## Evidence

- `D:/Nioh3_v080_deliverables/deliverables/v080-final-3798693/release` -
  verified hosted copies.
- `D:/Nioh3_v080_deliverables/deliverables/v080-final-3798693/acceptance` -
  packaged acceptance including the CI game identity and one-file records.
- `D:/Nioh3_v080_deliverables/deliverables/v080-final-3798693/public-redownload`
  - unauthenticated public downloads.
- `D:/Nioh3_v080_deliverables/deliverables/v080-final-3798693/PUBLIC_VERIFICATION.json`
  and `.md` - the 27-check public verification, with the verifier source
  `verify_public_release.py` beside them.
