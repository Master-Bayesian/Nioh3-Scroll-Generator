# Tauri v0.8.2 publication record (2026-09-28)

## Immutable identity

- Version: `0.8.2`, the first one-stop integration release: live and save-file
  editing of Amrita, gold, items, equipment and soul cores, NG1/NG2 offline
  search, and fixes ([engineering record](../product/releases/v0.8.2.md)).
- Candidate/product commit: `3e181cb6d2570097fc06fe253d531083d06f046f`,
  promoted to `main` (fast-forward from `8646562`) by the same atomic push that
  created the tag.
- Annotated tag: `v0.8.2`, tag object
  `a02390c03523f75f9b5a5321c995c950001937a4` -> commit `3e181cb`.
- Hosted preparation run: `36369277500`
  (https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/actions/runs/36369277500),
  bounded profile (`extended_search=false`), conclusion `success`.
- Public release:
  `https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/releases/tag/v0.8.2`,
  release id `397926506`, published `2026-09-28T03:18:13Z`, draft `false`,
  prerelease `false`, six assets; `/releases/latest` resolves to `v0.8.2`.

## Published assets (six, uploaded unmodified from the workflow artifact)

| Asset | Bytes | SHA-256 |
| --- | --- | --- |
| `Nioh3Studio-0.8.2-win-x64.exe` | 11,551,510 | `a5ba86ccd8289668dbd088b67ccdc9a23606c355bef73df34ea8ae6fc3c57ffe` |
| `Nioh3Studio-0.8.2-win-x64.exe.sha256` | 97 | `bfbf2f1103ebee078e657da3161cac17bb9423e43fc8e479298d75ad11b83067` |
| `Nioh3Studio-0.8.2-win-x64.sha256` | 97 | `c507e1e27b33ef0f5f213a66ee6d9256f0612424cadeb30c946d16db9cc5f294` |
| `Nioh3Studio-0.8.2-win-x64.zip` | 10,959,582 | `110a54d832471a217fdeb7becab20820feffd089a2191538aeb562366870ad24` |
| `tauri-update.json` | 3,861 | `bbe68e72db2cf1cac39f83e3d7b2f867c1c6b137c9306b0e1d8ac5bdf2b7c9c1` |
| `test-inventory.json` | 87,865 | `1a34b6a14b709679672008c013a2d091c8845efb96c551335c5ee85334becd40` |

## Verification chain

1. Three earlier preparation runs are superseded, not retried:
   `36352953300` (`ca0e1ec`) passed but predates the fixes below;
   `36367231427` (`7f0fa4e`) passed but its Tests lane failed hosted clippy
   (Rust 1.98 `chunks_exact_to_as_chunks`); `36368293940` (`ebfa62a`) failed
   the packaged UI driver: a delete clicked right after an edit met the
   save-content preview job and was refused as busy. The fix queues a user
   action behind a short observed job and retries the host's pre-start BUSY.
2. Run `36369277500` passed every required gate, then signed the update feed.
3. `tools/publish_tauri_release.ps1` verified the downloaded artifact (37
   checks over the six assets), promoted `main` and the tag atomically,
   published the release and re-verified it from the public downloads (53
   checks, `state: published-verified`).

Plan, result and verification reports:
`D:\Nioh3_v080_deliverables\deliverables\v082-release-publish\`.

## Acceptance scope

- Development lanes on the candidate: Frontend V2 passed (its first attempt
  failed once in the timing-sensitive `research/test_native_dispatch_fixture.py`
  native-breakpoint fixture, unrelated to this diff, and passed on rerun);
  worker library 145/145, save core 55/55, protected crate, frontend 79/80
  (1 skipped), protected save acceptance 23/23 run locally. The hosted
  `tests/migration` lane was still running at publication.
- Offline parity: NG1/NG2 R3/R4/R5 generation matches the live native
  generator on 10,000 records per context (PC v2.02); the rarity-5 preflight
  now follows 1,000 live NG3 records (one promoted effect in any ordinary slot).
- Packaged: the test14 portable build answered NG1/NG2 and NG3 rarity-5
  promoted-secondary searches and refused impossible sets.
- Live game (owner's machine): character, equipment and item edits were
  exercised during development. Not established by this release: in-game
  acceptance of the effect-icon marker fix, NG1/NG2 save installation, and the
  interrupted-write settlement against a real player's state directory.
