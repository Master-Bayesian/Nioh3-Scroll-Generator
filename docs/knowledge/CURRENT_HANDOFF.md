# Current project handoff — 2026-09-12

## Status and entry points

**Current delivery boundary:** the owner rejected the v0.7.3 NSIS installer:
one EXE must open the application directly, with no installation flow. A true
one-file replacement is being prepared under
[the one-file delivery contract](TAURI_ONEFILE_DELIVERY_20260912.md).
The earlier `4b4214d` installer and its
[acceptance evidence](V073_LOCAL_REVIEW_20260912.md) remain historical and do
not satisfy this delivery requirement. The owner requests review before any
push or publication. Do not push commits, tags,
release assets or an update feed until
the owner reviews and authorizes release. Permanent save edits and deletions,
like append and restore, must remain usable at the title screen without closing
the game. See [local preparation](V073_RELEASE_PREPARATION_20260912.md).

The v0.7.3 preparation audit verified both Pro patch archives in full and found
an additional backup-restore transaction path requiring the same race guards.
That path is now fixed with eight new fault regressions. Read the
[item-by-item closure](V073_PRO_REVIEW_CLOSURE_20260912.md) to distinguish
integrated code, actual verification, and the explicitly deferred research.

**Release status, rechecked 2026-09-12:** public latest is v0.7.1. The owner
subsequently withdrew v0.7.2 to draft. v0.7.3 is being prepared from the reviewed
follow-up worktree. The following v0.7.2 publication figures are historical. The official setup
EXE is 27,324,865 bytes and the portable/update ZIP is 29,683,023 bytes. Both
are exact products of successful hosted run `34513941110`, and the public
downloads were verified again after publication. Read
[v0.7.2 publication evidence](TAURI_V072_PUBLICATION_20260910.md) and
[the release runbook](RELEASE_RUNBOOK.md). Electron v0.7.0 remains withdrawn.
The older Electron and v0.7.1 notes below are historical.

**Unpublished follow-up:** the 2026-09-11 Pro review patch is integrated in the
current dirty worktree and has passed the full Windows verification recorded in
[the integration report](V072_PRO_REVIEW_INTEGRATION_20260911.md). Publication
policy changed by the project owner's latest 2026-09-12 decision: close the
unreproduced title-save and intermittent live-add reports pending a fresh
affected save and log. They are not current release blockers. This disposition
does not establish a game or user-side cause. The title-screen path remains available without a new native
save/cache protocol. Preserve the integrated transaction and recovery guards;
do not describe this decision as a demonstrated corruption fix or as overall
release acceptance. Read the [current risk decision](TITLE_SAVE_APPROACH_RESET_20260912.md).

The title-save Pro handoff has now returned and its static recovery plus
read-only v2.01 observer are integrated. Read
[the integration report](V072_TITLE_SAVE_STATIC_INTEGRATION_20260911.md) and
[the original research plan](TITLE_SAVE_OWNERSHIP_RESEARCH_PLAN_20260911.md).
The current save chain is recovered through snapshot, staging, asynchronous
worker I/O and completion consumption, with 28/28 observation locators
independently revalidated. C0.files.01 has now run without application mutation;
its CE profile captured no events and ended with cleanup unverified against the
exited process. The local 7.9 GB ETL remains unprocessed evidence. C1-C3 have
not run and are now deferred by the owner's risk decision. Further title-save
research and ETL processing are paused; preserve the current evidence for a
concrete failure report or an explicit request to resume. The simulated
delayed-write risk is not the player's proven corruption cause. Continue normal
follow-up release review without reinstating native title ownership as a
mandatory gate solely from the historical reports below.

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

All nine NG1-NG3 scroll-payload x requested R3/R4/R5 cells were inserted without
CE. Normal save/reload preserved all 39 inventory instances, effect fields and
native index entries. Each item had a verified automatic save backup. This
matrix describes the inserted scroll category. It did not vary or record the
running character's actual progression, so live insertion while actively in
NG1 and NG2 was initially unaccepted. A later isolated test inserted R3 seed
`10032001` while the game was actually set to NG1: inventory 41 -> 42, serial
`2446282`, slot 29, full container/index verification, all previous records
preserved, source save unchanged, and native cleanup verified. Normal save/reload
was not performed. The user deferred actual-NG2 testing until a matching user
report, so do not claim NG1 persistence or NG2 runtime acceptance.
R5 remains 5/5 in the captured saved file and loads as 4/4; the user reports the
R5 icon remains and explicitly requests no further workaround.
Temporary rule override/restoration for R3 seed 10030565 passed with no inventory
record change. See [native executor evidence](NATIVE_LIVE_ADD_EXECUTOR_20260909.md).
The subsequent final-package favorites -> cart subset -> native insertion was verified for seed 10030609: 39 -> 40 entries, unselected item absent, per-item backup verified, normal save/reload verified across all 40 records. See [RC1 live UI acceptance](V070_RC1_LIVE_UI_ACCEPTANCE_20260909.md).

## Current source and testing

The current unpublished working tree includes the Pro follow-up patch over
commit `6264cbd355729e0b434ba5f540232a5d1362a79d`. Windows integration passed 614
Python tests, 51 Node tests with source workers, the same 51 with packaged
workers, TypeScript checking, 11 Rust tests, the Electron production build, 15
production-surface checks, 20 encrypted synthetic-save UI checks, a complete
Tauri release build, strict packaged R3/R4/R5 GPU parity, portable manifest
verification and a 12-second packaged startup smoke. These results validate the
integrated application changes, not the unresolved game save-cache protocol or
unperformed live acceptance.

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
  child recovery. NG1/NG2 scroll payloads are enabled in the current UI source;
  actual-NG1 insertion has one current-memory positive, while actual-NG2 remains
  deferred and unaccepted.
- The production review UI participates in TypeScript checking and CI paths.
  CI runs real encrypted synthetic-save UI workflows in source and package modes.
- Logs remain bounded (five 4 MiB segments), search history retains three batches.
- Failed operations automatically replace the clipboard with a bounded support
  log containing the save path, candidate record, native receipt, and worker
  error needed for diagnosis.
- Copied support logs now span rotated segments and include version, data/log
  directories, and worker state.
- Permanent editing, deletion, backup restoration, and generated-scroll append
  permit the title screen or a closed game, per the owner's 2026-09-12 correction.
  Restore retains identity checks and a pre-restore checkpoint. These operations
  use related-save quiescence checks, an automatic multi-file backup, a durable
  write journal, exact decrypt/readback, and guarded rollback. The unreproduced
  title-save report is closed pending a fresh affected save and diagnostic log.
  Historical delayed-write simulations do not establish its cause or reopen a
  mandatory native-protocol research gate.
- Live-add preview can retry a narrowly proved idle miss, but actual insertion
  is never replayed. A rejected operation now clears its false adapter owner
  only when the default native transport proves that no in-memory or durable
  receipt exists; ambiguous failures still retain ownership.
- The v0.7.2 UI supports shared exact values for grouped rule families, keeps
  default filters empty, and makes Add to cart visually prominent.
- Exact published assets: `deliverables/releases/v0.7.2/`.

## Remaining work and constraints

Three UI languages are enabled; native-speaker review remains separate.
Production UI code is in `apps/workshop`. Favorites and cart each cap at 50;
favorites persist exact broker-owned candidate transfers. The web edition is
deferred. Independent possessed-Crucible-enemy selection is frozen. Seed
`86872488` is an ordinary single-player scroll and only showed possession in an
online session, so it is not a valid seed-only positive control. Read
[the research freeze](CRUCIBLE_POSSESSED_RESEARCH_FREEZE_20260911.md) before
using the preserved captures.
The current PC v2.01 source, raw section evidence, staged read-only collectors,
and Pro task are packaged at
`deliverables/Nioh3_PC_v2.01_Crucible_Possessed_Enemy_Materials_20260910_v2/`
and the matching ZIP. Record the final ZIP hash outside the archive after each
rebuild so that the package does not contain a self-referential stale hash.
Clean extraction imports the current backend, reproduces
`generate_complete_auxiliary(86872488, 3)`, passes the focused auxiliary test
suite, and checks the staged breakpoint signatures against the supplied PC
v2.01 `.text`. Runtime tables and two repeat captures are preserved, but they
must not be used as possessed-enemy ground truth. A known single-player
positive seed and an offline/online control pair are required before research
resumes.
All future research and reverse-engineering tasks must follow
[the research handoff workflow](RESEARCH_HANDOFF_WORKFLOW.md) and produce a
self-contained Pro package before they are treated as handed off.
The product follow-up review package was generated at
`deliverables/Nioh3_v0.7.2_Followup_Code_Review_20260911/` with a matching ZIP.
Pro reviewed title-screen save ownership, intermittent live-add lifecycle
recovery, and bounded support diagnostics. Its patch is now integrated; read
[the verified integration result](V072_PRO_REVIEW_INTEGRATION_20260911.md).
Usage statistics and the frozen possessed-enemy inverse problem remain
explicitly excluded.
Do not claim natural early-playthrough R4/R5 drops, propagation acceptance, or
all-state native fault tolerance beyond the recorded evidence.
Keep experiment data, user saves, game dumps, signing material, build state and
unrelated root scripts out of a future selectively reviewed commit.
