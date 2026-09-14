# Current project handoff — 2026-09-14

## v0.7.4 final product gates

The current release-candidate scope includes two final product changes. The
official Nioh 3 manual confirms the player-facing enemy-state terminology as
zh-CN `地狱附身`, en-US `Crucible Wraith`, and ja-JP `地獄憑き`; internal
`possessed` identifiers must not be presented as the official English label.
The packaged app now performs one update check after updater startup readiness
on every launch, prompts once when a newer version is found, defers that prompt
while another dialog is open, and retains a working manual Settings check.
The controller, isolated update UI, and native WebView2 surfaces have passed
bounded acceptance. Packaged update-feed behavior and the final outer EXE still
require candidate acceptance before publication.

## Status and entry points

**Current stable release:** Tauri v0.7.3 is public and is GitHub's latest stable
release. The immutable product commit is
`893996e4c11a9b0c20b125c696c89a0a47ec9048`, annotated tag object
`84218abf850282f9b8da34d3aaa325c795727878`, and successful hosted release run
`34690776011`. Read [the publication record](TAURI_V073_PUBLICATION_20260912.md)
and [the release runbook](RELEASE_RUNBOOK.md).

The player download is the true single-file outer executable
`Nioh3Studio-0.7.3-win-x64.exe`, 30,724,544 bytes, SHA-256
`2c1766751b0746eede917b04e2f4edac9b4c3348893349e2de4c3e38e84f9513`.
It does not require an installer, manual extraction, Python, Node.js, Electron,
or Cheat Engine. On launch it validates and extracts its embedded runtime into
a bounded LocalAppData cache and uses the Windows WebView2 system runtime.
The inner executable in the update ZIP is not a separately supported standalone
download. Hosted direct launch, cache reuse and pruning, real outer-EXE update
and restart, and public-release redownload verification all passed with zero
game writes.

Exact hosted assets and acceptance evidence are under
`F:/Nioh3_ScrollEditor/deliverables/releases/0.7.3-hosted-34690776011/`.
An independent redownload of all public assets is under
`F:/Nioh3_ScrollEditor/deliverables/releases/0.7.3-public-verification-20260912/`.
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

The immutable v0.7.3 product commit is
`893996e4c11a9b0c20b125c696c89a0a47ec9048`. Hosted acceptance passed 631
discovered Python tests with the documented hardware skips, 57 Node tests,
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
- Exact published v0.7.3 assets and evidence:
  `F:/Nioh3_ScrollEditor/deliverables/releases/0.7.3-hosted-34690776011/` and
  `F:/Nioh3_ScrollEditor/deliverables/releases/0.7.3-public-verification-20260912/`.

## Remaining work and constraints

Three UI languages are enabled; native-speaker review remains separate.
Production UI code is in `apps/workshop`. Favorites and cart each cap at 50;
favorites persist exact broker-owned candidate transfers. The web edition is
deferred. Native possessed-enemy research is active as bounded read-only
evidence collection. Read [the current resume and evidence record](POSSESSED_ENEMY_RESEARCH_RESUME_20260912.md)
and [the prior freeze](CRUCIBLE_POSSESSED_RESEARCH_FREEZE_20260911.md). Seed
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

The enemy-state product integration is now present in the working tree under a
revised owner-approved boundary. The NG3 search and preview flow uses the
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
challenge-count fields. ScrollCard Crucible Wraith markers are fixed-width
icons immediately after the enemy name, center-aligned rather than placed in a
separate right-justified column; the measured gap is about 3.0px, center delta about
0.54px, and row-center delta about 0.55px. Verification totals are 1,258 Python tests passed / 7
skipped, 61 desktop tests passed, and 596 localized messages audited.
This remains an unpublished working-tree feature: no packaged release,
live-game write, save mutation, or persistence acceptance is claimed. The
feature record is maintained in [the product catalog](../product/FEATURES.md);
the research and fixture boundary remains documented in
[the Pro integration record](ENEMY_STATES_PRO_INTEGRATION_20260914.md).

The same native WebView2 acceptance also covers the current UI micro-tuning.
The former top explanatory banner is removed, with its explanation incorporated
into the equipment and mission columns. The usage/help entry is in the selected-
conditions title bar; that panel has more vertical space, and enemy conditions
use a compact single-line layout. Crucible Wraith markers are fixed-width icons
inline after the enemy name and center-aligned. The interface-font-size setting is removed. Favorite
cards use the same fixed geometry as the main preview and support local search
over scroll ID and visible card metadata. These are observed in the
unpublished working tree under bounded native WebView2 acceptance only; they
are not packaged or published, and no live-game write, save, or persistence
acceptance is claimed. The bounded measurements are stored locally at
`deliverables/v074-ui-acceptance-20260914/verification.json`.
