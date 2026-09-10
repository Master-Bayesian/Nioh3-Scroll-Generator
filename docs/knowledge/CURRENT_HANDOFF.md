# Current project handoff — 2026-09-10

## Status and entry points

**Published:** Tauri 2 v0.7.2 is the latest stable release. The official setup
EXE is 27,324,865 bytes and the portable/update ZIP is 29,683,023 bytes. Both
are exact products of successful hosted run `34513941110`, and the public
downloads were verified again after publication. Read
[v0.7.2 publication evidence](TAURI_V072_PUBLICATION_20260910.md) and
[the release runbook](RELEASE_RUNBOOK.md). Electron v0.7.0 remains withdrawn.
The older Electron and v0.7.1 notes below are historical.

The connected Electron/React V2 is prepared at 0.7.0 with Chinese, English and Japanese UI.
This handoff records the local preparation checkpoint; hosted checks and the
GitHub version tag record the subsequent publication status. Read
[the pre-push completion report](V070_PREPUSH_COMPLETION_20260909.md) first; it
supersedes the pending-gate lists in the older reports below.
See [hosted build fixes](V070_HOSTED_BUILD_FIXES_20260909.md) for the subsequent
Windows checkout reproducibility repair and release workflow.
Read [V070_RELEASE_READINESS_REVIEW_20260909.md](V070_RELEASE_READINESS_REVIEW_20260909.md)
for the original findings. Read [V070_RC1_FOLLOWUP_20260909.md](V070_RC1_FOLLOWUP_20260909.md) for subsequent repairs, new evidence and remaining release gates.
Historical progress is preserved in
[the previous handoff](CURRENT_HANDOFF_PRE_V070_REVIEW_20260909.md); its pending
Figma/CE/NG1-NG2 statements are historical, not current product status.

## Architecture and verified invariants

- Freeze baseline: `backend-freeze-before-v0.7.0`, commit
  `8ad89ea4aee088b542977be14ec9e7c54e6bc3d1`. Preserve its B1-B6 contracts.
- Desktop brokers use typed private transfers; the renderer never receives raw
  records/process pointers. Search is killable; writes and native calls retain
  protected ownership and durable no-replay receipts.
- Preserve GenerationContext, R4 final-preview/stage-one installation pairing,
  exact RNG/replay, strict GPU fallback policy and the legacy Tk entry.
- Algorithm context now uses `scroll-generation-v0.7-native-completion-1` so
  early-R3 completion changes invalidate previous candidate/map caches.

## Live acceptance

All nine NG1-NG3 x requested R3/R4/R5 cells were inserted without CE. Normal
save/reload preserved all 39 inventory instances, effect fields and native
index entries. Each item had a verified automatic save backup.
R5 remains 5/5 in the captured saved file and loads as 4/4; the user reports the
R5 icon remains and explicitly requests no further workaround.
Temporary rule override/restoration for R3 seed 10030565 passed with no inventory
record change. See [native executor evidence](NATIVE_LIVE_ADD_EXECUTOR_20260909.md).
The subsequent final-package favorites -> cart subset -> native insertion was verified for seed 10030609: 39 -> 40 entries, unselected item absent, per-item backup verified, normal save/reload verified across all 40 records. See [RC1 live UI acceptance](V070_RC1_LIVE_UI_ACCEPTANCE_20260909.md).

## Current source and testing

The immutable v0.7.2 product commit is
`6b7689d75ec43f7f42b813a9ea6af396046930f7`. Hosted acceptance passed 571
Python tests, 123 CPU-policy tests, 47 Node tests, 5 Rust tests, packaged
R3/R4/R5 parity, real WebView2 UI flows, the signed in-place updater, and the
single-file installer lifecycle. The public package contains 756
manifest-verified files and preserves the earlier live-game evidence without
introducing a new mutation path.

The RC follow-up added distinct-slot effect requirements, between-item live batch cancellation/progress, signed local update replacement/rollback tests, and isolated native fault tests. Temporary challenge capacity editing passed packaged UI/live acceptance on revealed R3 and R4 scrolls. The RC1 UI acceptance subsequently performed one verified native insertion and normal user save/reload. The user confirmed R3 6/7 -> 2/5 and R4 3/4 -> 2/6; independent current-count changes were first accepted through a backed-up research probe. The pre-push pass added the reviewed instance-count command and formal UI, with automatic backup/readback and no-replay recovery. All capacity ownership is stopped and safe to shut down; counts remain at 2 by explicit user request. See the RC1 live acceptance report for scope and evidence.

- Default native executor; optional CE transport remains research compatibility.
- New live backups now use the normal account/slot/hash backup manifest and
  appear in backup management. Prior experiment backups remain at their recorded paths.
- Batch status reconciles verified children without replay; the UI can request
  child recovery. NG1/NG2 live additions are enabled in the current UI source.
- The production review UI participates in TypeScript checking and CI paths.
  CI runs real encrypted synthetic-save UI workflows in source and package modes.
- Logs remain bounded (five 4 MiB segments), search history retains three batches.
- Failed operations automatically replace the clipboard with a bounded support
  log containing the save path, candidate record, native receipt, and worker
  error needed for diagnosis.
- Offline save plans and commits fail closed while Nioh 3 is running. Live-add
  preview can retry a narrowly proved idle miss, but actual insertion is never
  replayed.
- The v0.7.2 UI supports shared exact values for grouped rule families, keeps
  default filters empty, and makes Add to cart visually prominent.
- Exact published assets: `deliverables/releases/v0.7.2/`.

## Remaining work and constraints

Three UI languages are enabled; native-speaker review remains separate.
Production UI code is in `apps/workshop`. Favorites and cart each cap at 50;
favorites persist exact broker-owned candidate transfers. The web edition is
deferred. Independent possessed-Crucible-enemy selection remains research-only;
do not infer a supported filter from seed `86872488`.
Do not claim natural early-playthrough R4/R5 drops, propagation acceptance, or
all-state native fault tolerance beyond the recorded evidence.
Keep experiment data, user saves, game dumps, signing material, build state and
unrelated root scripts out of a future selectively reviewed commit.
