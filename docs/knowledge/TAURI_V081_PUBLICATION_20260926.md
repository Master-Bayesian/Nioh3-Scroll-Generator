# Tauri v0.8.1 publication record (2026-09-26)

## Immutable identity

- Version: `0.8.1`, a fix release for the v0.8.0 regressions
  ([engineering record](V081_REGRESSION_FIXES_20260925.md)).
- Candidate/product commit: `522536aa3c8a2e350d474574a63f7d253622fa81`,
  promoted to `main` (fast-forward from `140ea78`) by the same atomic push that
  created the tag.
- Annotated tag: `v0.8.1`, tag object
  `7fd9ec0acb9a5f9df5019f057f03abe08e20889b` -> commit `522536a`.
- Hosted preparation run: `36208738081`
  (https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/actions/runs/36208738081),
  bounded profile (`extended_search=false`), conclusion `success`.
- Public release:
  `https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/releases/tag/v0.8.1`,
  release id `397025876`, published `2026-09-26T01:48:21Z`, draft `false`,
  prerelease `false`, six assets; `/releases/latest` resolves to `v0.8.1`.

## Published assets (six, uploaded unmodified from the workflow artifact)

| Asset | Bytes | SHA-256 |
| --- | --- | --- |
| `Nioh3Studio-0.8.1-win-x64.exe` | 11,322,609 | `eba1ad34b8026fa05dcf8aec2f6b159d8ffabeb4f11aab712d69dbfd8b5eb1f1` |
| `Nioh3Studio-0.8.1-win-x64.exe.sha256` | 97 | `13d5e0cf22d151ca0fd383ae2e21e22d7f5f7cbc2c3825c6695cc35f3fa122e6` |
| `Nioh3Studio-0.8.1-win-x64.sha256` | 97 | `aa3d2abb54a2c12f165b579199644c2715df57df80bbd4de73b673bb2e38097f` |
| `Nioh3Studio-0.8.1-win-x64.zip` | 10,730,681 | `cf786ec020f05342da1560a8f073ab50a6a933341b5966452f4b271be9b21792` |
| `tauri-update.json` | 3,504 | `7f51bb932e96c6c400d404b70a1fba950cff221e68581eb98d6701d177733a36` |
| `test-inventory.json` | 87,733 | `c4cbcbdaa5942f15323f09d08a09f25785e2e698aa85eefb5a1ab31f06890394` |

## Verification chain

1. Two earlier preparation runs failed and were repaired, not retried:
   `36207149773` and `36207375581` stopped at the UI locale audit (untranslated
   template-slot words) and then at the packaged UI driver, which still waited
   for the removed "后端已连接" status and required a failure to overwrite the
   clipboard. Both drivers now assert the v0.8.1 behavior; every packaged
   acceptance step of the workflow was first reproduced locally against a
   portable build of the same tree.
2. Run `36208738081` passed every required gate, then signed the update feed.
3. `tools/publish_tauri_release.ps1` verified the downloaded artifact (37
   checks over the six assets), promoted `main` and the tag atomically,
   published the release and re-verified it from the public downloads (53/53
   checks, `state: published-verified`).

Plan, result and verification reports:
`D:\Nioh3_v080_deliverables\deliverables\v081-release-publish\`.

## Acceptance scope

- Development lanes on the candidate: `tests/migration` 325 passed / 2
  skipped; worker library 141/141 (and stable across repeated runs after the
  accelerator test lock); frontend 75 pass / 0 fail; context goldens moved to
  the 0.8.1 product identity in Rust and Python.
- Live game (PC v2.02, owner's machine): two online live additions persisted
  after an in-game save; temporary overrides applied, replaced and stopped.
- Search speed with the game closed (measured after publication with the
  shipped worker): every benchmark query is faster than the Python worker, and
  all implementations return identical results
  ([benchmark](V081_REGRESSION_FIXES_20260925.md#search-speed-against-the-python-worker)).
- Not established: the foreign-identity/offline bit-25 live-add branch; any
  claim beyond the bounded release profile (extended search was not selected).
