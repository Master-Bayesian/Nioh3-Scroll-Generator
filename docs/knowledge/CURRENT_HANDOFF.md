# Current project handoff — 2026-09-20

## v0.7.5 published release

The 0.7.5 emergency release fixes two shipped problems: NG3 searches that
stopped at an internal trial budget and read as empty results, and the
window/taskbar icon that Windows stretched from a 16x16 bitmap. The immutable
product commit is `533694ebad21906aecbb6ab5283e04e760ce6c09` on
`codex/v075-search-hotfix` (promoted to `main`), annotated tag object
`c4cfce1523aa10a2532b79319c1e16ad4fbd6ee7`, hosted release run `34936188564`.
The public outer EXE is `Nioh3Studio-0.7.5-win-x64.exe`, 30,988,811 bytes,
SHA-256 `a81a30dd72b813c1253bcbca7a8f393fdf51468fed0d997a36a4b4535e7c6d09`. Read
[the publication record](TAURI_V075_PUBLICATION_20260915.md) and
[the v0.7.5 product record](../product/releases/v0.7.5.md). A later
documentation-only commit may follow the immutable tag; the tag still owns the
product source.

## v0.8.0 development (unpublished)

The owner has started an Astra-led incremental Rust backend migration, with
bounded implementation assigned to DeepSeek V4.1 Flash. See the
[migration baseline and sequence](V080_RUST_BACKEND_MIGRATION.md) and
[v0.8.0 engineering record](../product/releases/v0.8.0.md). The migration
baseline is the published v0.7.5 product commit
`533694ebad21906aecbb6ab5283e04e760ce6c09`; the owner manually confirmed the
v0.7.5 search-continuation and icon fixes.
M0 and M1 are standalone offline Rust domain/data slices covering enemy
generation, variants, Wraith trials, and native-table loading; the production
backend has not switched to them. See the migration record for exact evidence.
M2.1 adds the ordinary NG3 scroll-record/effect-sequence slice with an offline
cross-language parity gate (549 emitted rows, zero differences, including a
216-row cross-level sweep through the curve clamp, three native byte anchors)
and a permanent adapter content-parity guard; it is local offline evidence with
no product cutover. M2.2 adds R4 completion finalization and the paired
stage-one/install and finalized/preview records, byte-exact against all ten
tracked 232-byte native pairs through the same adapter, with a measured
reveal-branch comparison and an effect-area level-sensitivity measurement;
M2.3a adds a development-only Rust read-only worker that serves the preview
subset over the shipped protocol and matches the Python worker on a 189-preview
subprocess matrix, with runtime independence checks (no Python on `PATH`, empty
data root fails closed, contract digest read from disk). GenerationContext
digest binding over the wire, candidate identity beyond the preview payload,
search orchestration and the product worker cutover remain future work, and save
writes stay out of this line. Nothing in this section is packaged, tagged, or
published.

M2.3b1 now serves a bounded search surface inside this development worker, and
its gates are green on the frozen tree: `tests/migration` 78 passed / 0 failed
(an independent re-run of the three worker/parity modules alone: 33 passed), with
`cargo test` worker 66 library + 3 binary, domain 71, data 17, and `cargo fmt` /
`cargo clippy -D warnings` clean. The served routes are the fused auxiliary pivot
(terrain/special-rule/enemy criteria at playthrough 3, any certified rarity 3/4/5)
and the rarity-4 primary pivot, including a rarity-4 primary query that also
carries auxiliary criteria. The handshake capability object now comes from the
loaded accelerator's own probe (`cuda_pivot_and_auxiliary` and
`bulk_cpu_requires_opt_in` report `true`, matching the Python worker), and the
DirectCompute effect-filter capability and NG4/NG5 cache stayed `false`/absent
while those paths were unported; both are published now (see the M2.3d note
below).

Deliberately not served, each refused with its own reason instead of one blanket
claim: rarity-5 effect searches (effect-preimage accelerator not implemented), a
rarity-3 or other non-rarity-4 primary search (batched primary/replay route not
compiled), an effect-constraint search without a primary id (DirectCompute effect
route not implemented), an unconstrained sweep (fixed-draw replay not compiled),
secondary/roll-only replay, Grace-filtered pivots, terrain option ids, and
playthrough 4/5 (which need an exact save-bound rarity-5 map). Every one of those
gaps was then closed by M2.3d: the complete-composition preimage (rarity 3 and
rarity 5 with Grace), the rarity-5 one-wildcard route, the partial-effect filter
at rarities 3/4/5 including a selected Grace as the draw-1 pivot, and the
save-bound NG4/NG5 cached rarity-5 route all match the shipped worker on the
whole returned candidate payload, per-candidate cursors, page cursor, stop
reason, cancel/resume and structural refusals, so the handshake now publishes
the DirectCompute effect-filter capability and `cached_rarity5_playthroughs:
[4, 5]`. Read [the M2.3d handoff](../../deliverables/m23d-preimage/HANDOFF.md)
and [the migration record](V080_RUST_BACKEND_MIGRATION.md) for the gates and the
matched fixed-budget numbers. Two items remain explicitly open and are not
claimed: **live NG4/NG5 cache capture** (the cached gates use clearly labeled
synthetic valid partitions with a real Python oracle, since no genuine
`0xDD82`/`0xD523` capture exists) and **G4/private write materialization** (the
read-only slice never writes a save; `installable` stays false).

Search is still not wired into the shipped host: no product cutover, package,
tag or publication, and save writes stay out of this line.

### Current position (M3/M4, 2026-09-15)

The M2.3a sentence above that lists digest binding over the wire, candidate
identity beyond the preview payload, search orchestration and the product worker
cutover as "future work" is superseded and kept only as history:

- The read-only worker serves its whole surface, including the DirectCompute
  effect filter and `cached_rarity5_playthroughs: [4, 5]`.
- `crates/nioh3-protected` (new) serves the runtime role - ownership, temporary
  overrides, reviewed live add and batch, and the `generate` / `search` /
  `capture_grace` scan and measured-map loops - and the save role.
- Packaged profile (M4) is implemented and is now the **default packaged graph**:
  the packaged host selects the Rust graph from the staged
  `worker/worker-backend.json`, validating and refusing a bad manifest by name
  rather than falling back, and acceptance compares the resolved argv for all
  three roles. The owner authorized this local backend switch for internal
  v0.8.0 backend validation only - no remote push, dispatch, tag, feed or
  publication. The PyInstaller worker stays in the tree for development, parity
  and the legacy Tk path and is no longer part of the shipped graph.
- The product version is deliberately unchanged for this internal build, so its
  bytes are not the published v0.7.5 stable release and must not be described as
  one. See `deliverables/v080-completion-readiness/` for the current lane record.
- Frontend acceptance: the cart/R4 and editor/delete/restore legs were blocked on
  the Rust save host refusing `effect_sequence_only` candidates at
  `install_record` (`/root/m3_save_acceptance`); the lane record now reports both
  former blockers fixed and every named leg green on the rust-packaged graph,
  and the app-level save lifecycle faster than the shipped Python host (4.833 s
  vs 8.121 s). Those results predate the cipher optimization on some legs, so
  the same-candidate packaged UI acceptance remains open and is owned by
  `/root/m4_quality_review`.
- Open: four live flows (real game process and real save) and any
  player-facing acceptance. Live acceptance has not run.

Current boundary statement and evidence:
[`RUNTIME_HANDOFF.md`](../../deliverables/m3-protected-host/RUNTIME_HANDOFF.md),
[`EVIDENCE_RUNTIME_HOST.md`](../../deliverables/m3-protected-host/EVIDENCE_RUNTIME_HOST.md),
[`v0.8.0` engineering record](../product/releases/v0.8.0.md).

### Same-byte candidate acceptance (2026-09-19)

This supersedes the "same-candidate packaged UI acceptance remains open" line
above, and is bounded package and synthetic evidence only. The final internal
v0.8.0 backend-review candidate is artifact source commit
`b20e493ff0b2374978d008451a461cb9caa6d44b` (`dirty=false`, 748 manifest members,
version still 0.7.5). Outer EXE 11,027,487 bytes SHA-256
`546be5fa25a2df8f78ff41d0775b7f72c16065a2fd244f0963f773adc12581e8`; ZIP
10,403,815 bytes SHA-256
`1ef50eef2c4290c6309620ea55a7c914a0a97f7cc3e69480a608f0a8c543c8b2`; the
extracted runtime is byte-identical to that ZIP (749 of 749 files).

Closed on those bytes: the release-host frontend gate
(`TAURI_PACKAGED_FRONTEND_OK`), host resolution and per-role identity, one-file
cold/warm launch and cache, update replacement/rollback, the responsive layout
matrix, and protected-save performance (Rust 4.845 s steady / 4.841 s cold versus
the shipped Python host 7.531 s / 8.507 s; the old "13.7 s versus 8.0 s" line was
stale). Evidence root
`D:\Nioh3_v080_deliverables\deliverables\v080-backend-review\evidence\`:
`packaged-frontend-b20e493/`, `host-package-b20e493.json`,
`identity-*-b20e493.json`, `cold-start-b20e493.json`,
`onefile-rollback-gate-b20e493.json`, `add-layout-b20e493/`,
`M3B_PROTECTED_SAVE_PERF-packaged-b20e493.json`.

Still open: live game and real-save acceptance, equipment readiness, and
protected live add/save/reload before product PC v2.02 can be enabled. The PC
v2.02 level-cap research is closed at native `600` / display `356`, and `添画` on
Divine completion is owner-deferred, not a blocker. The v2.02 Rust backend wiring
and the live noop-gate result are in the status reconciliation below.

## v0.7.4 published release

The published v0.7.4 scope includes two shipped product changes. The
official Nioh 3 manual confirms the player-facing enemy-state terminology as
zh-CN `地狱附身`, en-US `Crucible Wraith`, and ja-JP `地獄憑き`; internal
`possessed` identifiers must not be presented as the official English label.
The packaged app now performs one update check after updater startup readiness
on every launch, prompts once when a newer version is found, defers that prompt
while another dialog is open, and retains a working manual Settings check.
The controller, isolated update UI, and native WebView2 surfaces have passed
bounded acceptance. Packaged update-feed behavior and the final outer EXE also
passed hosted and public acceptance; publication is complete.

## Status and entry points

**Current stable release:** Tauri v0.7.5 is public and is GitHub's latest stable
release. The immutable product commit is
`533694ebad21906aecbb6ab5283e04e760ce6c09`, annotated tag object
`c4cfce1523aa10a2532b79319c1e16ad4fbd6ee7`, and successful hosted release run
`34936188564`. Read [the v0.7.5 publication record](TAURI_V075_PUBLICATION_20260915.md)
and [the release runbook](RELEASE_RUNBOOK.md). The v0.7.4 record below is
historical; its release remains available and was replaced only in the updater
feed.

The player download is the true single-file outer executable
`Nioh3Studio-0.7.4-win-x64.exe`, 30,862,753 bytes, SHA-256
`634315acf853fc1b9a722a1b34fc7ee1734b4023fd0dfd0d555060b8905cd4e9`.
It does not require an installer, manual extraction, Python, Node.js, Electron,
or Cheat Engine. On launch it validates and extracts its embedded runtime into
a bounded LocalAppData cache and uses the Windows WebView2 system runtime.
The inner executable in the update ZIP is not a separately supported standalone
download. Hosted direct launch, cache reuse and pruning, real outer-EXE update
and restart, and public-release redownload verification all passed with zero
game writes.

Exact hosted assets and acceptance evidence are under
`F:/Nioh3_ScrollEditor/deliverables/v074-hosted-candidate-df438ed-20260915/`.
An independent redownload of all public assets is under
`F:/Nioh3_ScrollEditor/deliverables/releases/0.7.4-public-verification-20260915/`.
The earlier local one-file and installer candidates remain historical and are
not the published product.

The v0.7.3 preparation audit verified both Pro patch archives in full and found
an additional backup-restore transaction path requiring the same race guards.
That path is now fixed with eight new fault regressions. Read the
[item-by-item closure](V073_PRO_REVIEW_CLOSURE_20260912.md) to distinguish
integrated code, actual verification, and the explicitly deferred research.

The integrated transaction and recovery guards remain required. The unreproduced
title-save and intermittent live-add reports are closed pending a fresh affected
save and diagnostic log; this does not establish a game or user-side cause.
Read the [current risk decision](TITLE_SAVE_APPROACH_RESET_20260912.md).
Electron v0.7.0 remains withdrawn, and the v0.7.1/v0.7.2 publication notes below
are historical.

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

The immutable v0.7.4 product commit is
`df438ed3a9a1b92e68b3c77da0b1c094e0663327`. Hosted acceptance passed 671
discovered Python tests with the documented hardware skips, 61 Node tests,
TypeScript checking, both Rust test groups, catalog and locale drift checks,
packaged R3/R4/R5 parity, real WebView2 workflows, the complete responsive UI
matrix, synthetic encrypted-save operations, single-file direct launch, and a
real single-file update/restart lifecycle. The public release redownloads match
the hosted products and their signed update metadata. These are bounded package
and synthetic-save results; they introduced no new live-game writes.

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
- Exact published v0.7.4 assets and evidence:
  `F:/Nioh3_ScrollEditor/deliverables/v074-hosted-candidate-df438ed-20260915/` and
  `F:/Nioh3_ScrollEditor/deliverables/releases/0.7.4-public-verification-20260915/`.

## Remaining work and constraints

Three UI languages are enabled; native-speaker review remains separate.
Production UI code is in `apps/workshop`. Favorites and cart each cap at 50;
favorites persist exact broker-owned candidate transfers. The web edition is
deferred. The shipped Crucible Wraith feature and its research boundary are
recorded in [the Pro integration record](ENEMY_STATES_PRO_INTEGRATION_20260914.md).
The chronological capture history below remains supporting evidence; read
[the earlier resume](POSSESSED_ENEMY_RESEARCH_RESUME_20260912.md) and
[the prior freeze](CRUCIBLE_POSSESSED_RESEARCH_FREEZE_20260911.md). Seed
`86872488` remains invalid as a seed-only positive because possession differs
between normal solo and a one-person expedition.

Natural-drop seed `156062997` supplied repeated normal-solo positives: fresh
pre-armed runs A and B each mapped the visibly possessed first-wave Koroka to
the only task record with `record+0x8F = 1`. A separately classified post-spawn
salvage and one misconfigured third repeat agree but are not additional formal
controls. The corrected `86872488` normal-solo run supplied an owner-observed
negative with all six task records at `+0x8F = 0`.

The same `86872488` seed in a one-person expedition then supplied a stronger
same-run control. The owner reviewed the recording and confirmed that one of the
 two second-wave Nuppeppo was possessed while both were Curse (中文一难). Native
records show only spawn `0xF3F` at `+0x8F = 1`; the other second-wave Nuppeppo
and every other task record are zero. This same-wave, same-species split makes
`+0x8F` the leading native possession-state marker across the captured Koroka
and Nuppeppo conditions. The returned Pro analysis has now recovered its
descriptor source and typed copy path; live producer validation remains pending.

The expedition also exposed a separate Curse (中文一难) correlation. The only
two task records with `record+0xE9 = 0` are third-wave Ippon-Datara spawn
`0xF42` and fourth-wave Nuppeppo spawn `0xF45`, exactly the two paired waves in
which the owner observed one ordinary occurrence. Eight other task records have
`+0xE9 = 1`. Preserve this as a scoped candidate semantic pending an independent
repeat. Candidate-only summaries omit the zero records; all future analysis must
retain every mission record before applying selector filters. `record+0xEA` and
the 12-byte selection mask remained zero in every completed control.

The independent expedition repeat is complete. All ten task identities repeated
at newly allocated addresses. `record+0x8F` again marked only the visually
possessed wave-2 Nuppeppo partition. `record+0xE9` again matched the complete
Curse visual partition, but its two zero records moved from Run A's
wave-3/wave-4 ordinary occurrences to Run B's wave-1/wave-3 ordinary
occurrences. This supports a generated state marker rather than a fixed task-
record partition.

The Pro response recovered a strong PC v2.01 static chain: `0x10285C4` writes
generated enemy descriptor `+0x0F`; the full 20-byte descriptor is copied into
temporary and persistent task `+0x80`, producing task `+0x8F`. The later
`0xE3ADF0` path consumes the existing byte and can separately assign `+0xE9`.
Run D subsequently validated the descriptor trial, parent-frame LCG stream,
copy path, and persistent task link for one expedition transaction. Read
[the Pro integration record](POSSESSED_ENEMY_PRO_REVIEW_INTEGRATION_20260913.md).

Community follow-up indicates that expedition may activate pre-authored enemy
spawn positions that normal solo suppresses, rather than inventing four enemies
at entry. In `86872488`, the possessed wave-2 Nuppeppo occupies a world position
with no solo spawn; Curse can change while that possession association
does not. This is owner-relayed observed behavior plus an inferred mode-filter
mechanism, not yet native materialization proof. Do not equate the captured
task `spawn_id` with that physical spawn point: solo task `0xF3F` has lookup
`0x4388`, while expedition `0xF3F` is Nuppeppo `0xDCB98`.

The fresh paired Run D is indexed by the [run manifest](../../audit/possessed_enemy_capture/86872488/20260913-mode-pair-d/run_manifest.json):
run ID `assignment-86872488-expedition-mode-pair-d-20260913`, one-person
expedition, and 29 events (2 origin + 7 trials + 10 task copies + 10 linked
tasks). All ten task objects linked. Only spawn `0xF3F` (decimal 3903), class
1, wave index 1, position 1 had `flag8f=1`. Parent-frame `+0xC0` LCG states
and tickets validate from `2281170847` to `115239214`; the constant
`fallback_global` owner-route diagnostic is not the consumed stream. Cleanup
is verified empty. The external invincibility trainer was enabled to hold the
wave, and no actor/spatial join was captured.

The mode-dependent upstream object and request construction remain unresolved.
Do not infer them from downstream configured-count branches or request another
live mode comparison before the correct causal site is identified. The next
analysis input is the verified Pro handoff
`deliverables/Nioh3_PC_v2.01_Possessed_Enemy_Mode_Upstream_Pro_Handoff_20260913_v1/`.
Its handoff directory and matching ZIP are complete and verified: the ZIP SHA-256
is `649B58C372C7C282BFD8C4F8B42E26A8A94F8F285CE095ECBA5E58266374167C`, size is
`54,603,470` bytes, and the directory contains 217 files with 216 hashes
verified; the validator passed.

The accepted [mode-upstream Pro integration](POSSESSED_ENEMY_MODE_UPSTREAM_PRO_INTEGRATION_20260913.md)
records a native-confirmed request producer/queue/consumer chain: request `+0x9`
is the consumed extra-generation input on this chain, while `+0xA` and `+0xB`
are copied tail fields and are not projected. The packaged `86872488` controls
contain six base occurrences and four dynamically appended occurrences. The
stable anchor is build/table identity plus wave, terrain, and `point_key`; the
possessed expedition occurrence `(D4, 1)` is absent from solo tasks. The
`LCG^35` sample boundary does not establish a universal oracle. Local static
verification covers 160 targeted tests, 38 collector signatures, and a native
verifier covering 36 functions, 30,248 bytes, 7,683 instruction starts, 14
calls, and 4 sites. Run A was a confirmed no-trigger procedural invalid
experiment. Run B captured four events and passed validation/cleanup, but its
owner-observed one-person-expedition mode is not a native mode-enum capture.
The result requires Pro reconciliation with Run D's `+0x9=1`, `+0xA=2`, and
10-task result. The returned reconciliation corrects the old interpretation:
the old capture stopped after one `extra=0` call, and pure requeue does not
turn `0` into `1`. Its new `mode-upstream-sequence` phase retains four
read-only breakpoints and moves the fourth to `0x2237978` so one 120-second
window can observe multiple requests, consumers, and generator returns. The
live-result Pro handoff is discoverable at
`deliverables/Nioh3_PC_v2.01_Mode_Upstream_Live_Result_Pro_Handoff_20260913_v1/`
and its ZIP (SHA-256
`5C05679A4C19DC47097D5A2171CE14E30FC8159AE412C9F04E52595CFDBF30B4`,
205,978 bytes); its research handoff validator passed with 68 files. The
reconciliation package is archived at
`deliverables/Nioh3_Mode_Reconciliation_v201_20260913/` and its matching ZIP;
external ZIP SHA-256 is `7D08DC2FF52223FC502BCEDF9614DF48FD6E5C1604191768E624463CF519ACA3`,
with CRC and MANIFEST 26/26 verified. Integration is currently 8/8 byte
matches, 159 tests passed, and static 4-site/15-call-edge verification passed.
Sequence C is now live-accepted for seed `86872488`: the owner observed a
one-person expedition through the full fixed-site 120.641-second window. One
`owned_scroll_branch` request `A8912D05B700010301000000` had `Q[9]=0`, and one
linked generator return contained six descriptors across waves `[1,1,2,2]`
(`class0=6`, `class1=0`). No second event occurred at the same four sites;
the validator accepted the capture and cleanup was verified. This remains
bounded evidence: it is not an all-writers trace, native mode enum, task/actor
join, or product oracle. The new Pro package is
`deliverables/Nioh3_PC_v2.01_Mode_Sequence_Live_Result_Pro_Handoff_20260913_v1/`
and its ZIP (SHA-256
`ba2e1f07a6b4db1d6b5742d1314a02b8ab075703008709cacf4da03b6442bd1a`,
310,727 bytes, 96 files verified). The next Pro target is the alternate
insertion/consumer/post-generation/task-materialization/session route supplying
the four extra tasks. The current focused collector regression, run through the
tracked project-environment entry point, reports 161 passed.

The returned augmentation Pro archive is preserved at
`deliverables/Nioh3_Augmentation_Fork_v201_20260913/` and its matching ZIP;
the ZIP SHA-256 is
`0303E14360115EF6E5A07896111300FCFFF417B54C5C0CEE441DC38B2F34644C`, size
314,541 bytes, with 112 entries and 111 manifest payloads verified. No
evidence shows C going from 6 to 10: D already had 10 pre-materialization
descriptors through the known wrapper chain, and D manager counts 11..20. The
E39D40 prepass, existing-key reuse, and tagged auxiliary factory are new static
forks; the factory cannot create D's F42..F45 low-28 keys from C's F3C..F41
keys, so missing bodies remain required. Pro's 15-file source proposal is
integrated. The local exporter now treats `0x13684C` as a verified no-unwind
leaf `[0x13684C,0x1368A2)` with pinned full-control-flow hash
`627C74A4D65ED70D8F1D6A6D699C6786B23E4A49D5C3EDCF0B81FD134851BF59`; the
other seven target bodies use exact pdata BEGIN. Offline export succeeded from
text SHA `F879...8023` and pdata SHA `928D...B904`, reading 3,602 bytes without
process/CE/game access. Regression: 261 passed in 3.15s via
`tools/run_python_tests.ps1`. The minimal follow-up package is
`deliverables/Nioh3_Augmentation_Code_Bodies_v201_20260913/` and its ZIP
(10 files, 2,061,353 bytes, SHA-256
`586392e0af2881ae02aee74a52474166f1c8fecb8437f8cfc8307e9e379bd132`). Send
only that code-bodies ZIP to Pro before any live materialization-frontier run.

The returned `Nioh3_Augmentation_Code_Analysis_v201_20260913` Pro archive is
preserved in its same-named `deliverables/` directory and ZIP. The input ZIP is
254,535 bytes, SHA-256
`DA28DA6768DC694A187D3C460F5B26C8F63D483165912BCE82999E7A0C390578`; sidecar
matches and 170 members passed 169/169 MANIFEST size/hash verification. Pro's
core decision remains no evidence for C 6→10: E39D40 is a non-RNG temporary
key→index prepass; `22364FC` writes queue node `+38/+3C/+3D`; `1BF6EC` copies
to backing; `13684C/4BD5F8` perform unsigned key-only lookup/sorted-unique
insert and skip descriptor copying on reuse (Run C has no mismatch capture).
The tagged factory does not explain D F42..F45; D's ten descriptors predate
materialization. Corrected closure export uses `UNW_FLAG_CHAININFO`, pins leaf
`13684C`, and offline-exported eight groups with exact package-core parity from
pinned text/rdata/pdata. The 12-file Pro integration needed only import routing
and optional-objdump skips; regression is 297 passed, 3 skipped in 3.04s.
There was no CE/game capture or product change. Next is one read-only seed
`86872488` expedition, 120-second four-site materialization-frontier capture
for the source→actual-task join; do not repeat old sequence runs or claim
actor/global-manager finality. Stale CE PID 35580 was detached; game PID 35484
is current and not armed.

The current PC v2.01 source, five controlled runs, raw section evidence, bounded
static triage, staged read-only collectors, and revised Pro task belong in the
immutable package
`deliverables/Nioh3_PC_v2.01_Possessed_Enemy_Assignment_Pro_Handoff_20260913_v1/`
and its matching ZIP. The package records owner observations as accurate text;
no surviving replay is required. The old `20260910_v2` package and its task are
superseded and must never be rebuilt or used as current ground truth.
The returned Pro package is preserved at
`deliverables/Nioh3_Assignment_Origin_v201_20260913/` and matching ZIP; ZIP
SHA-256 is
`9611A80DBA9A414493AD25DD112344C49D5AF26A4D9D6C9091DEC0CE28ED7601`.
The first gate is two native-confirmed single-player positive captures followed
by a negative comparison, not immediate inverse work.
All future research and reverse-engineering tasks must follow
[the research handoff workflow](RESEARCH_HANDOFF_WORKFLOW.md) and produce a
self-contained Pro package before they are treated as handed off.
The product follow-up review package was generated at
`deliverables/Nioh3_v0.7.2_Followup_Code_Review_20260911/` with a matching ZIP.
Pro reviewed title-screen save ownership, intermittent live-add lifecycle
recovery, and bounded support diagnostics. Its patch is now integrated; read
[the verified integration result](V072_PRO_REVIEW_INTEGRATION_20260911.md).
Usage statistics and possessed-enemy inverse analysis remain explicitly
excluded from that older product review package.
Do not claim natural early-playthrough R4/R5 drops, propagation acceptance, or
all-state native fault tolerance beyond the recorded evidence.
Keep experiment data, user saves, game dumps, signing material, build state and
unrelated root scripts out of a future selectively reviewed commit.

Materialization-frontier Run C is complete at
`audit/possessed_enemy_capture/86872488/20260913-materialization-frontier-c/`.
PID 35484 creation filetime `134338049984156850` matches prior sequence C
identity but is a distinct transaction 13,098,688 ms apart. Return `0x2237994`
and queue `A8912D05B700010301010000` (`Q[9]=1`, context only) yielded 10 entry
descriptors across `[2,2,3,3]` (`class0=6,class1=4`); prepass stayed 10, all
lookups were `new_path`, and all 10 tasks linked with zero mismatches. Validator:
22 events, 120,109 ms, 1,919 bytes, no errors; cleanup verified inactive,
empty, and debugger intact. This closes the current augmentation/reuse fork but
does not explain prior sequence C and is not mode-enum, actor/all-writer, or
product-oracle evidence. VEH `debugProcess(2)` replaced failing Windows
`debugProcess(1)` after Error 87; focused frontier regression is now 52 passed
in 1.99s. The final Pro ZIP is
`deliverables/Nioh3_PC_v2.01_Materialization_Frontier_Live_Result_Pro_Handoff_20260913_v1/`
with 38 files/37 hashes, 110,363 bytes, SHA-256
`988eda1248fc15b443d6b15639a8b4f3fc534432ae98c1e306c55a69ee08333d`; package
validation passed with 38 files, 37 hashes, and 38 ZIP files.
Send it to Pro; no further game run now.

The entry-transaction question is now closed by the bounded
[mode-transaction-join live result](POSSESSED_ENEMY_ENTRY_TRANSACTION_LIVE_RESULT_20260913.md).
One unique `parameterized_session_branch` consume at `0x21DC438` (`Q[9]=1`)
carried request `A8912D05B700010301010200` into 10 generated and byte-identical
materializer descriptors across waves `2/2/3/3` (`class0=6,class1=4`), with
`source_flag=1` only on spawn `0xF3F`/3903. It matches Run D exactly. Prior
Sequence C's six-descriptor call was not observed materializing and is not a
6→10 transformation. Cleanup passed; validator coverage was 50 signatures and
focused research regression was 460 passed, 3 skipped; 9 documentation and
handoff tests also passed. This remains bounded evidence, not native UI-enum,
product-oracle, or all-writers proof. No further live entry capture is needed
for this sub-question. The superseded research target was explicit Curse (中文一难),
Possessed, and expedition-only enemy choices in filter/preview. The owner has
since removed Curse and Expedition from the user-facing product; only exact
native-eligible Possessed choices remain in the current UI. Remaining
research is offline class1 append replay, scoped-LCG `+8F` starting point, and
the `+E9` deterministic/random boundary. The former v1 product-research
handoff is superseded by v2 at
`deliverables/Nioh3_PC_v2.01_Possessed_OneDifficulty_Expedition_Product_Research_Pro_Handoff_20260913_v2/`
and its ZIP (359,669 bytes, SHA-256
`A9A42333ABFBD8CC78D122EB0F3E45BF1416341723DEFA9C236048C987500FE5`,
111 files/110 payload hashes/111 ZIP files verified). The v2 package also
records the root TASK as the sole instruction authority.
The v1 package remains only as a transaction-only historical archive at
`deliverables/Nioh3_PC_v2.01_Entry_Transaction_Live_Result_Pro_Handoff_20260913_v1/`
with a matching ZIP and sidecar: 41 files, 40 payload hashes, 106,496 bytes,
SHA-256
`D8AB99E4BE0EBF02E03BDB6AB289CB59B43ED34DE174CDBF705250A4E7AF3847`.
Both directory and ZIP validation passed. It is not the current task to send;
send only the v2 product-research package.

The v2 product-research task has now returned and is integrated as the bounded
[enemy-states backend/reference foundation](ENEMY_STATES_PRO_INTEGRATION_20260914.md).
For `86872488`, offline preview reproduces six normal-solo occurrences with no
Possessed enemy and ten expedition occurrences with Possessed spawn `0xF3F`.
Curse remains a separate state and defaults to `unknown` without complete late
context. The subsequent PC v2.01 native-table capture succeeded as a
zero-breakpoint, read-only, no-Cheat-Engine capture. Offline replay against
the native fixtures passes all three required Possessed controls, including
`156062997` solo expected/actual spawn `0xF40`; the other controls remain
`86872488` solo empty and `86872488` expedition `0xF3F`. This is offline replay
versus native fixtures, not a new native generation run, and it does not make
any claim that Curse is exact. Local verification reports 73 focused tests
passed with 3 optional NumPy skips, 42 existing auxiliary tests passed, and
the complete Python suite at 1,252 passed with 7 skips. No UI/worker or release
claim follows from this capture; this remains a backend/reference foundation,
not a shipped filter or release feature.

The enemy-state product integration shipped in v0.7.4 under a revised
owner-approved boundary. The NG3 search and preview flow uses the
ordinary solo roster. Its only user-facing enemy-state control is an iOS-style
Crucible Wraith (地狱附身) switch, offered only for exact identities with a valid
native-table-eligible low-pool variant. The catalog marks 50 capable groups;
14 other low-pool groups are ineligible, and one group has mixed eligibility.
The UI never offers Crucible Wraith for arbitrary low-pool enemies, and the worker
independently rejects stale or impossible requests. Curse (中文一难) and
Expedition/常世同行 controls and previews were removed from the user-facing
product. Backend tables, research fixtures, and internal analysis support may
remain as reference material, but must not be described as shipped UI.

Native WebView2 acceptance is recorded locally at
`deliverables/v074-ui-acceptance-20260914/verification.json`: seed `86872488`
in the solo/default roster showed six occurrences and zero Crucible Wraiths;
seed `156062997` in the same roster showed six occurrences and one Crucible Wraith. The
acceptance verified that Expedition/常世同行 and Curse controls are absent,
that the Crucible Wraith switch appears only for exact eligible identities and is
absent for ineligible low-pool and high-pool identities, and that the enemy
list frame and enemy-only combination help are present. It covered `zh-CN`,
`en-US`, and `ja-JP`, maximized-window layout, no overflow, and paired
12px/16.8px label-value typography for recommendation level and
challenge-count fields. ScrollCard Crucible Wraith markers are compact text
labels immediately after the enemy name, center-aligned rather than placed in a
separate right-justified column; the measured gap is about 3.0px, center delta about
0.54px, and row-center delta about 0.55px. Verification totals are 1,258 Python tests passed / 7
skipped, 61 desktop tests passed, and 596 localized messages audited.
This feature is packaged and published in v0.7.4. Its acceptance did not add a
live-game write, save mutation, or persistence claim. The
feature record is maintained in [the product catalog](../product/FEATURES.md);
the research and fixture boundary remains documented in
[the Pro integration record](ENEMY_STATES_PRO_INTEGRATION_20260914.md).

The same native WebView2 acceptance also covers the current UI micro-tuning.
The former top explanatory banner is removed, with its explanation incorporated
into the equipment and mission columns. The usage/help entry is in the selected-
conditions title bar; that panel has more vertical space, and enemy conditions
use a compact single-line layout. Crucible Wraith markers are compact localized
text immediately after the enemy name and are center-aligned. The
interface-font-size setting is removed. Favorite
cards use the same fixed geometry as the main preview and support local search
over scroll ID and visible card metadata. These changes are packaged and
published in v0.7.4; no live-game write, save, or persistence acceptance is
claimed. The bounded measurements are stored locally at
`deliverables/v074-ui-acceptance-20260914/verification.json`.

## PC v2.02 Pro handoff (final package v5, ACCEPTED for bounded Pro analysis)

Self-contained bounded handoff: D:\\Nioh3_v080_deliverables\\deliverables\\game-version-update-20260919\\pro-handoff\\nioh3-pc-v2.02-addon-revision-and-level-clamp-pro-handoff-20260919-v5 and .zip (137,038 bytes, 60 members, SHA-256 74E1576DF78F804D7734F2C948B3413665F2418255A79A5BE10D931B7764EDC9).
Accepted 2026-09-19 on independent closure review (archive-derived, read-only); that review also confirms the v4->v5 diff is 4 files (README.md, TASK_FOR_PRO.md, KNOWN_LIMITS.md, SHA256SUMS.txt) with 56 byte-identical members.
Status: accepted Pro handoff awaiting Pro analysis / next-probe design. Product PC v2.02 remains UNAPPROVED (product_enablement_allowed: false), no release, no packaging, and the overall compatibility goal is NOT complete.
Known limits: 24 raw records = second seed only (seed 1 raw unrecoverable); no revision artifact for the Q1 additive-effect question; 600/180 clamp consumer body not located; item/multiplier row-store addresses are runtime-only; table field meanings unknown.

## PC v2.02 P0 evidence corrections (2026-09-19, supersedes earlier P0 numbers)

- item row 3358: the true row-relative change is `+0x84` at absolute `0x15514C`
  (`0x380` -> `0x0`). The v5 `+0x8C` was window-relative only; the exported
  window started 8 bytes early. Real row `+0x8C` is unchanged.
- optional_multiplier: verified keyed diff (validated schema, key u32 at
  `+0x14`, stride 32, header 8) = 0 keys removed, 3 added (`0x3472`, `0xAA65`,
  `0xD56F`), 3 payload changes (`0x39E8` 80 -> 35, `0xA899` 30 -> 15, `0xD7C3`
  1400 -> 600), 104 position shifts, 646 metadata-only changes. The earlier
  "105 added / 101 removed / 546 changed / duplicates 85 -> 84" came from an
  8-byte key read at the row start with per-key dict collapse and is rejected.
- 42-row display curve: the retained PC v2.02 capture verifies 42/42 identical
  points with matching blob hashes; identity grade is
  `signature_bound_read_only_capture` (no PID, module base, or executable hash
  recorded). `tables/level_curve.bin` (501 x 10 bytes) is a different table and
  is not evidence for the display curve.
- Parity scope: R3 masks `0x1B` (10,000 runtime header mismatches) and R5 keeps
  10,000 full-record mismatches; neither may be summarised as a full-record
  pass. Raw reports stay immutable with pinned hashes in `tools/parity_scope.py`.
- Private save: exactly one game-recognized save file, unchanged since
  `2026-09-14T19:45:54Z`; 43 occupied records; one record (slot 45, `0xE604`,
  seed 180443387, serial 2375795, key 50409) stores raw internal recommended
  1400 (predicted display 700) and is identical in the 9/2-era product backup.
  The owner reports that some scrolls now show 356, but that observation is not
  yet joined to this record, so the stable 1400 proves only that the durable
  value has not been rewritten and that no post-update save exists yet.
  Identity, persistence, and per-user generality stay unproven.
  Superseded 2026-09-19: a post-update durable save now exists and stores raw
  internal 600 (display 356) for this exact tuple; the sentence above is kept as
  the pre-update snapshot. See the status reconciliation below.
- Strongest static lead: u32 key `0xD7C3` is consumed at `0x110DE06`
  (base x scale threshold) and at `0x227FE4B` (parameter manager `+0x230` ->
  getters `0x6084C0` -> `0x20E544C` -> writes record `+0x10`/`+0x12`). Static
  relation only; no causality and no product change.
- Status unchanged: PC v2.02 remains UNAPPROVED, nothing published, and the
  overall compatibility plus backend live-acceptance goal is NOT complete.

## PC v2.02 status reconciliation (2026-09-19)

Compact closure update; it supersedes the "no post-update save exists yet" clause
above and does not rewrite the raw P0 snapshot.

- Level-cap research is closed: native cap `600` -> derived display `356`. The
  prior durable `1400` record (slot 45, type `0xE604`, seed 180443387, serial
  2375795, key 50409) reads `600` live and is `600` in the first post-update
  durable save. The UI-selection pointer join is not established and is not
  claimed. Strongest level-persistence evidence:
  `deliverables/game-version-update-20260919/reports/current-save-after-load-20260919.json`
  with the live probe `.../reports/ce-selected-record-probe-20260919.json`.
- Product PC v2.02 stays disabled and unapproved pending a protected live
  add/save/reload.
- Dispatch-thread attribution is closed by the `go-v202-thread-attribution`
  report: TID 44388 is the unique same-capture stack owner and no named-thread
  product requirement exists.
- The acquisition writer is proven by the `go-v202-acquisition-contract` report;
  width stays live qword vs save u32, and the conditional insertion gate remains
  a runtime qualification risk.
- `添画` on Divine completion is owner-deferred, not a blocker; there is still no
  raw rarity-5 sample, so its maximum-6-effects semantics stay `unknown`.
- The Rust v2.02 backend halves are wired offline: the version-aware effect
  resource loader, and the `Materializer`'s version-selected effect plus preview
  tables. Both named worker seeds (`226061463`, `10030700`) are byte-exact against
  the Python PC v2.02 reference and the engine's own table choice is proven by a
  resource identity row. Evidence:
  `deliverables/router-recovery-20260919/EFFECT_RESOURCE_FIX.md` (loader; legacy
  entry point unchanged) and `.../V202_PARITY.md`. Offline only - no live-game,
  packaged or save acceptance is claimed.
- Native noop gate: **blocked, not passed**. The corrected-window attempt
  (`v202-noop-39932`, the same PC v2.02 process instance) did reach the dispatch
  entry and was accepted once - `entry_hits 1`, `acknowledgement_hits 1`,
  `redirect_count 1`, `stop_reason "accepted"`, 49 ms - and then cleanup failed
  with `GetThreadContext(0)` error 6, leaving the receipt `phase: "uncertain"`,
  `released: false`, a retained allocation and `breakpoint_count: -1`. Inventory
  read identical before and after and the debugger detached, but that is bounded
  evidence about visible inventory only, not a safety proof. After the capture
  the game showed a fatal `0xC0000005` dialog at `0x00007FF69AA272D6`; the
  derived RVA `0x1E72D6` is arithmetic, not causality. No success claim, no Pro
  or package claim, and no retry. Evidence:
  `deliverables/router-recovery-20260919/native-noop-deadline-fixed/evidence/`
  (no top-level report; raw receipt preserved; ledger entry 2026-09-20). The
  earlier title-screen idle miss is history:
  `deliverables/router-recovery-20260919/native-noop/REPORT.md`.
- Incident handoff packet A for that cleanup failure is built and structurally
  validated: `deliverables/native-noop-cleanup-pro-20260920/` plus `.zip`
  (143,021 bytes, 36 files, ZIP SHA-256
  `72d4c74dabb11e3dbadda937adda08a3cf3316b156b778c408caee34e3906073`). It asks
  Pro to explain the invalid-handle cleanup path, decide what the packaged
  evidence does or does not say about the later access violation, and propose the
  smallest fix with a deterministic regression and one safe acceptance plan.
  The package states the noop's real target-process writes (trampoline, `RIP`
  redirect, debug registers) and its absence of inventory/serial/save mutation;
  nothing in it claims a root cause or enables writes.

## Rust backend migration Pro review package (packet B, ready for Pro, 2026-09-20)

Self-contained review package for the unpublished six-crate Rust backend
migration: `deliverables/rust-migration-pro-review-20260920/` plus `.zip`
(2,639,869 bytes, 464 files, ZIP SHA-256
`cc6eb6647f93de43450c08ed669ec42b49a5c2390bd6721aa122aeb20d9a55ba`). It asks for
an evidence-graded module decision (`KEEP` / `LOCAL_REPAIR` / `REFACTOR` /
`REWRITE`, or `NOT_ASSESSED` / `INSUFFICIENT_EVIDENCE`) plus an aggregate
`KEEP` / `PARTIAL_REPAIR` / `FULL_BACKEND_REWRITE` recommendation, both citing
packaged paths and lines. It bundles all six crates with every manifest and
lock, the 28 migration gates with the fixtures they load, 53 current Python
peers and 54 baseline peers extracted from the published v0.7.5 commit
`533694ebad21906aecbb6ab5283e04e760ce6c09` (what `v0.7.5^{commit}` resolves to;
annotated tag object `c4cfce1523aa10a2532b79319c1e16ad4fbd6ee7`), every
repository product data resource including the large versioned tables, packet A
unpacked once as incident reference, and portable standard-library validation.

Status: ready, pending Pro review. No review conclusion exists, nothing is
published, and the package enables no PC v2.02 write path. Snapshot boundary: the
package was built before this entry, so this paragraph is intentionally not part
of it and recording the link here invalidates no package hash.

## v0.8.0 repair-wave r4/r5 verdict and evidence-link closure (2026-09-20)

Self-contained Pro review of the r3 repair wave:
`deliverables/Nioh3_v080_RepairWave_r4_Closure_Review_20260920/` plus the
same-named `.zip` (1,446,401 bytes, 190 files and 189 manifest entries, SHA-256
`74d7d3ee1138e47ba27544b4204678ece8ed1175980835d9e4a4b5cc5eaf70e0`). Both
repository validators - the checkout-bound `tools/validate_research_handoff.py`
and the package-local `verify_package.py` - and an independent verifier returned
PASS on this package.

The independent r4 return is verified at
`D:\Downloads\Nioh3_RepairWave_r4_Independent_Review_20260920.zip` (SHA-256
`b1fd922371f9b52d8de040aa23d5b8485b10d729599942e78a9dc7eed2799c05`). Its
six-item result is three `PASS`, three `REPAIR`, and no `REWRITE`: RF01, RF04,
and contract idempotence pass; RF02, RF03, and RF05 required bounded repair.

Those three repairs and final closure evidence are now packaged in
`deliverables/Nioh3_v080_RepairWave_r5_Final_Closure_Review_20260920/` plus the
same-named `.zip` (756,191 bytes, 155 files / 154 manifest entries, SHA-256
`1da548d9d8aa1878e9f4d3615596fe0638ddd19754f1f0d435ab4e71d6d0a631`). Both
`tools/validate_research_handoff.py` and the package-local `verify_package.py`
passed the directory and ZIP.

Current bounded gates: save transaction 54; RF02/RF05 plus protected-host
closure 24; save crate 40; runtime all-features 148; Tauri 41 passed / one
explicitly ignored; packaged host resolver 7/7; build-root policy 13; save,
runtime, and Tauri clippy/rustfmt clean; four contract outputs byte-identical
before/run1/run2; five compact raw crash cases retained. The seven previously
unattributed RW09 failures are now mapped to all seven packaged-host resolver
nodes and closed by the 7/7 rerun.

The independent r5 return is verified at
`D:\Downloads\Nioh3_RepairWave_r5_Independent_Review_20260920.zip` (109,156
bytes, SHA-256
`a921d8bee87e93198994978fb38c7b0455b8087e61709fbb69cd817367848235`; 32
internal hashes passed). RF02, RF03, and RF05 all pass. Its aggregate remained
`REPAIR_FIRST` only because the earlier 7/7 report did not retain the executed
debug-host identity, both JavaScript verifier sources, and their result JSONs in
one traceable record. Pro explicitly limited the remaining work to
`R5-EVIDENCE-LINK` and stated that a contradiction-free closure promotes the
tree to `PASS_TO_LOCAL_RC`; it did not request another backend repair wave.

That final link is now closed in
`deliverables/Nioh3_v080_R5_Evidence_Link_20260920/` and the same-named ZIP
(241,340 bytes, 94 files / 93 manifest entries, SHA-256
`714e7194b39053d6c9d64106ae64636ee530e6493441ab9de764f954b608958a`). The
repository handoff validator verified every directory hash and archived byte.
The bounded rerun rebuilt the Tauri frontend, explicitly confirmed the current
Cargo debug host and two debug workers, and then ran only the seven packaged-host
resolver nodes: 7 passed, 0 failed, 0 skipped. The host and frontend verifiers
both executed debug host SHA-256
`1d1632f6be42fbcc112fcbba0291ec12efe25ac795ef3e52722049be2d5b9251`;
all three roles carried game file version `2.0.2.0`, and the frontend handshake
selected `r4_finalizer/pc_v2_02/resource_v1` with bundle digest
`df15220de9e356755bd8b4e2ec33f4617cf0e7c140347898b8374fd75515acbf`.
Source and artifact identities were unchanged across the run.

The independent evidence-link return is verified at
`D:\Downloads\Nioh3_R5_Evidence_Link_Independent_Review_20260920.zip`
(39,963 bytes, SHA-256
`a73e5c2c6d098b94c4b5f4a74b6490b9b0a7e783d5b6061a789388e874ee38a5`;
10 internal hashes passed). It independently recomputed 45 consistency groups
with no failures, accepted the retained 7/7 Windows results under their stated
debug/synthetic boundary, and returned `PASS_TO_LOCAL_RC` with no remaining
repair-wave tickets or new product findings. This closes `R5-EVIDENCE-LINK` and
the repair wave; do not reopen the backend or run another global audit without a
new concrete failure or relevant code change.

Status: **`PASS_TO_LOCAL_RC` for the repair-wave gate.** This means the tree may
proceed to a local v0.8.0 RC; it is not itself an RC package or publication
approval. The retained acceptance is automated debug WebView2 functional
evidence on synthetic saves. No manual visual acceptance, one-file user
installation/startup acceptance, real game, real user save, full global Python
rerun, push, tag, update-feed change, or release acceptance is claimed. PC v2.02
protected writes remain disabled and unapproved. The old Electron/Python
source-development launch remains a non-shipped explicit-version follow-up.

## Agent orchestration note

Multi-agent work in this repository runs an Astra root that owns the outcome,
ticket design, prioritization, risk and uncertainty decisions, and acceptance,
with DeepSeek V4.1 Flash workers executing coding, tests, documentation,
packaging, and bounded evidence collection. Worker routing is the official
`router_deepseek_deepseek_v4_1_flash` role (model
`deepseek/deepseek-v4.1-flash`), the default and only DeepSeek worker route the
owner set. Read `$nioh3-agent-orchestration`
(`.agents/skills/nioh3-agent-orchestration/SKILL.md`) for ticket shape,
parallelism, context, review, and reporting detail; `AGENTS.md` keeps the routing
summary and links the skill so a fresh thread finds it. This is operational
routing, not a capability claim.
