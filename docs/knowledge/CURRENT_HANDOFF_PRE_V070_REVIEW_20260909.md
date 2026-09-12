# Current project handoff — 2026-09-07

## Latest live test: independent executor and temporary override

Read [NATIVE_LIVE_ADD_EXECUTOR_20260909.md](NATIVE_LIVE_ADD_EXECUTOR_20260909.md).
FINAL: the user confirmed all added scrolls after normal save/reload. All 39
saved records were independently verified; loaded inventory/native index also
preserve all instances. R5 remains 5/5 in the save but loads as R4 4/4, with an
R5 icon per user. The user explicitly requests no further R5 workaround.
Use `deliverables/frontend-v2/portable-v2-native-matrix-20260909/`, built and
verified with four packaged protected-worker IPC tests. This supersedes the
pending persistence/build notes below. No publication.
Latest extension: all nine NG1-NG3 scroll-payload x R3-R5 combinations have now
passed actual CE-free insertion, with an automatic backup per item. This did
not vary the running character's current progression; actual NG1 and NG2
runtime compatibility remains a separate test. The eight-cell extension
increased inventory 31 to 39 and preserved all old records. The user has been
asked for one normal save/title reload; persistence verification is pending.
Early R3 native completion, search-preview serial suppression and narrow raw-R5
header preservation were fixed. A new matrix portable build is in progress;
the single-R3 package below predates these final fixes.
Temporary rule override/restoration for R3 seed 10030565 passed with unchanged
inventory and clean shutdown. Independent Windows dispatch passed synthetic
and actual-game no-call tests. After the user closed the conflicting trainer,
one native insertion passed: R3 seed 10030566, serial 2418728, count 30 to 31,
all previous records preserved, full native index and cleanup verified.
The user has been asked to normally save/reload and confirm this new seed.
Every addition requires an automatically verified save backup, rechecked before
dispatch. Native execution is now the default. The fresh portable package is
`deliverables/frontend-v2/portable-v2-native-live-add-20260909/`; four packaged
protected IPC tests passed. Frontend live-write and batch acceptance remain.

## Latest: V2 parity implementation, September 9

Read [V2_PARITY_IMPLEMENTATION_20260909.md](V2_PARITY_IMPLEMENTATION_20260909.md)
before the older sections below. It supersedes the open implementation gaps:
custom window chrome/original icon, editor sizing, stable hover, three-batch
history, bounded logs, multi-delete, data-root controls, native map acceleration,
temporary editor controls and signed whole-package updater are implemented.
Actual title-screen NG1/NG2 generation/search/cache reuse passed; the save hash
remained unchanged. Real-game writes, temporary override activation/restoration,
and public signed update delivery are still acceptance work. No publication.
The user was asked to open a scroll detail and provide its ID for the temporary
override test; do not claim that test complete without further evidence.

## Latest feedback: window adaptation, 25 results and legacy parity

Read [LEGACY_UI_PARITY_20260909.md](LEGACY_UI_PARITY_20260909.md) for the current
feature matrix and user acceptance sequence. The UI now discovers saves,
includes backup management, resolves buff labels, accepts grouped value filters,
and connects early-playthrough native search. NG1/NG2 native acceleration/cache
parity and real-game acceptance remain outstanding. No release was published.
This supersedes the NG3-only UI and maximum-20 statements below.


## Current: connected review UI, September 9, 2026

The user-approved review screen now has a development Electron/Python entry.
Read [REVIEW_UI_BACKEND_CONNECTION.md](REVIEW_UI_BACKEND_CONNECTION.md) first
for launch steps, current evidence and live-game acceptance limits. This
supersedes the demo-only / wait-for-Figma descriptions below for the review UI.
No release or user-save/game write was performed in this integration iteration.


## 2026-09-09 search UI demo review

The user supplied a search-only wireframe and requested an interactive demo.
Source: `apps/search-demo`; standalone output:
`deliverables/frontend-v2/search-ui-demo/index.html`. See its README and visual
preview. Strict TypeScript compile and 18 browser checks passed. This isolated
React demo uses illustrative fixtures; it does not replace the engineering
workbench or connect to GPU/game/save workers. No Tauri migration is implied.
The supplied diagnostic report concerns v0.6.10/v0.6.9, not v0.9.10. The old Tk
UI still contains synchronous CUDA availability probing; its driver-hang
startup issue must not be marked fixed by this demo. Details and intake are in
`apps/search-demo/DESIGN.md`. Prior backend and live-add acceptance limits remain.

This file is the durable entry point for the next development agent. It is a
working-tree handoff, not a release note and not evidence that unfinished code
is ready to ship.

## Current stopping point: wait for final Figma UI

The final engineering portable is `deliverables/frontend-v2/portable-v2-live-add-r5`.
Verification: 525 Python tests, 26 source IPC/frontend tests, 26 packaged tests,
strict GPU R3/R4/R5 source/package parity, actual portable Electron smoke, and all
144 manifest files passed. CE has no owned breakpoints, the probe allocation is
released and the temporary typed server is stopped. No new game mutation was
performed after the user closed the game. See
`deliverables/frontend-v2/live-add-followup/FINAL_VERIFICATION.json` and
`LIVE_ADD_ENGINEERING.md` for precise acceptance boundaries.
The localized r3 and live-add r4 builds below are historical/superseded artifacts.

## Frontend V2 foundation and follow-up

Read `LIVE_ADD_ENGINEERING.md` first for the latest 2026-09-08 live-add work.
The user confirmed the single native addition in game and after normal saving,
returning to title and reloading. Seed 10030565 now has a second, independently
allocated instance serial 2416080. Whole-container and native-index verification
confirmed exactly one added item (29 to 30), preserving every previous record.
Copied-save/reloaded-memory verification matched all 30 defined records, allowing
only the new-item marker cleared by viewing that added scroll. Private evidence
is under `deliverables/frontend-v2/live-add-followup/single-insertion-01/`.
Do not execute that directory's historical `arm-once.lua` again.

The user has now closed the game and requested a pause after completing the
live-add engineering work, pending Figma. Do not restart farming, challenge tests,
or equipment/trainer implementation. Preserve relevant research for a future
legal equipment adder; see the shared-approach section in LIVE_ADD_ENGINEERING.

An optional CE-backed live-add application path now implements candidate input
serialization, no-allocation native preview, matching encrypted-save backup,
version/code gates, process-instance-bound preparation, exclusive durable
at-most-once claims, status/recovery/cancellation, full-container/index receipts,
protected worker contracts, Electron broker/preload and a UI-independent session.
The local pipe accepts fixed commands, not Lua source. CE is an explicit optional
dependency. Production preflight rejects modified selected callees; the research
quantity-hook bypass is not silently accepted by the product adapter.
Only the earlier single R3 insertion and descriptor roundtrip have live-game
acceptance. The newly integrated candidate-to-worker-to-CE path was completed
and tested after game closure; do not claim an integrated live insertion, every
rarity, or all-state scheduling acceptance. No final Figma layout was chosen.

Current language resources contain all 32 native Chinese/Japanese/English
qualifiers. Five unnamed English dummy-effect entries remain distinct from
normal translation coverage. See `LIVE_ADD_AND_LOCALIZATION_FOLLOWUP.md` for
the capture history; its earlier no-insertion conclusions are superseded by the
bounded live-add acceptance above, not by a global concurrency proof.

The 2026-09-07 V2 foundation now follows the frozen commit
`8ad89ea4aee088b542977be14ec9e7c54e6bc3d1`. Read
`FRONTEND_V2_FOUNDATION.md` before continuing architecture or frontend work.
It records the bounded freeze check, shared search extraction, role-scoped
contracts, offline/save/runtime workers, safe shutdown, save plans and operation
receipts, native search/map capture, NG4/5 cache reuse, and the portable Electron
build. Current local evidence is 501 Python tests, 22 source IPC tests, 22 packaged
IPC tests, packaged search parity and actual portable Electron smoke; see
`deliverables/frontend-v2/VERIFICATION.md`. Final V2 visual design is pending
Figma; the engineering controls are temporary. Old Tk remains runnable.

The earlier localization portable was `deliverables/frontend-v2/portable-v2-localized-r3`.
It adds the verified three-language item qualifiers and passed 16 focused Python
tests, 22 source and 22 packaged IPC tests, strict GPU R3/R4/R5 parity and actual
portable Electron checks including native qualifier names across preload/worker.
See `deliverables/frontend-v2/verification-localized-r3.json`. The historical
501-test full-suite result below belongs to the r2 foundation baseline.
Read `FRONTEND_V2_INTEGRATION_GUIDE.md` before connecting final Figma components.
Nonvisual follow-up adds framework-independent protected-operation observation
and save sessions, renderer-reload search recovery, exact receipt recovery after
interruption, offline seed-capacity filtering, derived inventory input metadata,
sanitized real-reveal regressions, private diagnostics, single-instance ownership,
locked worker builds and complete startup file verification. The encrypted
synthetic-save IPC test now covers search through reviewed installation and
readback as well as edit/delete/restore. Its records are not real player saves.
All 137 manifest files in the final portable artifact verify. The first
`portable-v2-foundation-ready` build failed at startup because Electron presents
ASAR archives as virtual directories; `-r2` uses `original-fs` and passed actual
portable Electron startup/search/reload/locale verification. Do not launch the
superseded build or replace its files individually. CI includes the packaged
gates but has not run on GitHub; this remains an unsigned working-tree artifact.
Three-language engineering presentation, persisted language preferences, exact
catalog coverage auditing and protected-host timeout/frame fault tests are now
included. Source resources now include all 32 native Japanese talisman qualifiers;
English has five missing dummy-effect names. Do not call this complete native
translation acceptance. The read-only displayed recommended-level resolver is
also connected through the worker and preload; see
`V2_RECOMMENDED_LEVEL_SELECTION.md`. Both rebuilt workers and the actual portable
Electron flow passed with the updated contract. Displayed 350 predicts raw
585/586; the default remains unchanged pending game display acceptance.

Read `deliverables/frontend-v2/EXPERIMENTS_20260907.md`: 172,032 isolated native
constructions established a source-metadata distinction for R3 and a header/
effect-count distinction for capped R5. Save hashes remained unchanged. CE MCP
provided read-only confirmation of a possible live-add call chain; no inventory
insertion was invoked by the assistant. Those construction matrices alone do
not accept drops, completion/reveal or live insertion; the later bounded
gameplay evidence below is separate. The frozen numerical sources remain unchanged.

The loaded-game follow-up is recorded in
`deliverables/frontend-v2/live-acceptance/20260907T225708Z/SESSION.md`.
Three natural equipment/consumable pickups were observed through CE entry/return
breakpoints; no scroll was acquired. The 28 initial scrolls have **unknown
provenance**, explicitly including possible earlier experiments according to
the user. Never use them to establish natural drops or P1/P2/R3/R5 rules.
Static and runtime evidence identifies the 400-slot scroll container, remainder
return semantics, separate uint64 serial and uint32 acquisition-order counters,
and a duplicate-serial index overwrite hazard. That early capture alone did not accept live insertion; the later bounded
acceptance and optional adapter are documented in LIVE_ADD_ENGINEERING.md. The user authorized continued experiments and clarified
that the physical Escape stop signal came from opening the game menu; subsequent
menu cooperation uses a controller.

A subsequent user-reported three-run Small Hell trial recorded 75 insertion
entry hits, zero scroll hits and the same 28 full scroll records. The earlier
builder observation window was shorter and separate, so it does not establish
absence of generation across all three runs. See `SMALL_HELL_01_ANALYSIS.md`
in the loaded-game evidence folder. Deferred return-site removal exposed a
sampling limitation for rapid pickups; a combined observer with retained return
sites is being prepared as research tooling. Do not require the user to identify
a scroll before pickup: they explicitly cannot do that. The user confirmed the
unlock quest and first NPC scroll receipt. No further unlock confirmation is
needed. Existing-record display matching also corroborated raw recommended
592 displaying as 353, without accepting target 350 or natural origin.

Latest controlled acquisition succeeded at the user-identified Hongo-Yushima
Small Hell with maximum drop rate enabled. One builder call and 79 insertion
calls produced 80 matched returns; the new P3/R3 scroll is seed 10030565,
uint64 serial 2398468, slot 14, level 170, predicted recommended level 343.
The same serial links builder output, pickup source and the new inventory
record; all 28 earlier scrolls remained byte-identical. The user then saved at
a shrine. A stable read-only encrypted save copy contained 29 scrolls and the
new record matched all 232 runtime bytes. See `DLC_SMALL_HELL_03_ACQUISITION.md`
and `save-snapshots/first-readback/readback.json` in the evidence folder.
This accepts an observed engine-owned acquisition and saved-record readback,
not arbitrary remote insertion, natural drop frequency, DLC exclusivity,
guaranteed drops or a reload test. The fresh sample has no growth token. Its
first clear is now observed: all four effect IDs/raw values remain unchanged,
the UI reveals the expected values (313, 6.0%, 1.3%, 11), and attempts change
from stored 7 to displayed 6/7. Recommended level 343 is visually confirmed.
A second shrine save copy matches the subsequent runtime inventory across all
29 complete records; see `r3-10030565-first-reveal-analysis.md`. This is one
ordinary R3 first-clear sample, not growth-token-to-Grace conversion or a game
reload acceptance test.

The agreed three-scroll battle acceptance group is now complete; read
`deliverables/frontend-v2/live-acceptance/20260907T225708Z/BATTLE_ACCEPTANCE_WRAP_UP.md`.
For the existing R4 seed 43723117, first reveal replaced effect slot 3 exactly
as predicted; all 168 effect bytes match the frozen finalizer. For seed 36526331,
the game invoked the per-slot wrapper for indexes 1, 3 and 4 (UI slots 2, 4, 5),
with three matched returns on validated game thread 41620. All three candidate
buffers match offline prediction across all 232 bytes, none has its accepted
completion bit set, and the game retains the original effects. Its UI reveals
3/4 attempts. Both R4 samples retain unknown historical provenance. Each final
shrine-save copy matched its contemporary runtime inventory across all 29
records. No reload, external live-add or growth-token conversion is accepted.

The final observer is stopped, zero remaining/owned breakpoints were verified
at 00:38:54 UTC on September 8, and all three subagents completed their work.
The user requested a quick wrap-up and no further ultra subagents. Do not reopen
or append gameplay experiments automatically; this group required only three
first clears and is finished. Future subagents must use a lower reasoning effort.

The count-limit review also confirmed live parameters 4 and 7, a native setter
cap of 7, seed-derived capacity and a distinct unsigned remaining byte at +0x33.
See `challenge-count-upper-limit-review.md` in the evidence folder. A requested
full 7/7 result requires capacity-7 seed selection; simply writing the remaining
byte is not a capacity edit. The subsequent engineering follow-up added the
optional offline `initial_challenge_counts` filter (distinct 4-7 values) and
read-only capacity/remaining metadata. Remaining-count editing is not enabled;
current product defaults remain unchanged.

The combined observer's remaining caller-change cleanup gap was repaired and
12 closed flow mocks plus 10 builder mocks passed. Historical captures preserve
their original source hash and the old misnamed `playthrough_input` field;
future observers use `descriptor_byte_22`, since it controls record +0x0F
(zero maps to one), not playthrough. Playthrough is identified by record type;
record +0x30/+0x31 are rarity bytes. No frozen numerical source was rewritten.

Read `V2_REQUIREMENTS_BACKLOG.md` for the user's screenshot interpretation and
later research into live in-game scroll insertion without returning to title.
The screenshot priority labels are not authoritative. This research does not
relax existing write/native gates or count as implemented live insertion.

The active community catalog/live-verification task is documented separately in
`docs/knowledge/EQUIPMENT_CATALOG_LIVE_HANDOFF_20260902.md`. Read it before
resuming the equipment supplement or full-item catalog work.

## Repository state

- Repository: `F:\Nioh3_ScrollEditor`
- Branch: `codex/todo-321`
- Public baseline: stable tag `v0.6.10`, published on 2026-09-06 from commit
  `00a89b2` with PC v2.01 and PC v2.00.02 support.
- v0.6.10 adds final-record-only rarity-4 search matching and custom-only
  first-/second-playthrough rarity-4 Grace search and installation while
  retaining all v0.6.9 features.
- The earlier local `v0.6.7-beta.1` acceptance build remains historical and is
  not part of either update channel.
- Preserve unrelated untracked research, captures, packages, and user files.

## Unreleased backend freeze before Frontend V2

The branch after stable v0.6.10 contains the bounded B1-B6 backend
safety/interface freeze requested by the Astra audit. This work is intentionally
unreleased and is checkpointed by the annotated tag
`backend-freeze-before-v0.7.0`. Its authoritative scope, contracts, evidence
levels, and stop condition are recorded in
`docs/knowledge/BACKEND_FREEZE_BEFORE_V070.md`.

The freeze adds manifest-bound save transactions and rollback, conservative
remote-call/hook/window lifecycles, centralized generated-install policy,
strict Seed accelerator ABI v2 with no hidden bulk-CPU fallback, a minimal
UI-independent service/headless seam, generation-context-bound caches, and a
stable test inventory. Do not mistake this for Frontend V2 or a full backend
rewrite.

Final local verification passed 449 discovered tests in 67.171 seconds, the
native ABI/source/binary identity check, the headless context handshake, and a
five-second startup smoke of an 18,444,094-byte one-file package. The smoke
artifact is intentionally under `.codex_tmp`, not a release or delivery build.

## Most recent completed delivery

The complete effect-name catalog and the currently verifiable value ranges were
exported before resuming editor work:

- `deliverables/catalogs/Nioh3_PC_v2.00.02_Trilingual_Effect_Ranges_20260831.zip`
- `outputs/20260831-effect-catalog/Nioh3_PC_v2.00.02_Trilingual_Effect_Catalog.xlsx`
- 3,609 native effect IDs with Simplified Chinese, Japanese, and English names.
- Exact level 1–180, rarity 3/4/5 discrete raw values for the 51 effects that
  are reachable by captured scroll-generation contexts.
- Level-180 range attempts for all 3,609 effects; 182 contextual definitions
  are explicitly unresolved rather than guessed.

## Stable v0.6.10 rarity-4 search finalization repair

- Every native rarity-4 search route now keeps two distinct records: the
  native stage-one installation payload and the completed record used for
  filtering and UI preview. User constraints are evaluated only against the
  completed record.
- Native batch completion now mirrors the full outer loop: it tries every
  eligible source slot independently, accepts the first completed replacement,
  and treats exhaustive no-change as the game's valid unchanged final result.
  A rarity-4 stage-one record is also rejected at the application search
  boundary if a future route attempts to expose it as a candidate.
- Seeds `43723117` and `36526331` are permanent regression vectors. The first
  proves that a stage-one `0xD411` hit must be rejected after completion changes
  slot 3 to `0xF9BE`; the second proves that a completed preview, rather than an
  unresolved native stage record, reaches the candidate UI.
- The native scanner was exercised live for both reported Seeds on PC v2.01:
  `43723117` rejects stage-only `0xD411` and accepts final `0xF9BE`, while
  `36526331` returns an installable final candidate after exhaustive no-change.
  The subsequent in-game reveal matched both predicted final records; that is
  gameplay evidence for the native finalizer, not a substitute for the
  application-boundary regression.
- The tracked release tree passes all 423 repository tests, including the
  application search-boundary gate and both reported-Seed regressions.
- GitHub Actions run `34080366708` passed the same 423-test suite, built the
  one-file executable, signed `latest.json`, and published the stable Release.
  The downloaded 17,933,766-byte asset passed Ed25519 manifest verification,
  SHA-256 `E2ABD12562C615155E0B97F146B66BBB7FD6209863ACEE9391321D12AEC9621E`,
  packaged startup, exact-path cleanup, and the public stable-update check.

## Stable v0.6.9 changes

- Rarity-4 installation now separates the post-reveal candidate preview from
  the bytes written to the save. The installer writes the canonical native
  stage-one record so the game performs the completion pass exactly once.
  Seed `125804734` is the regression vector: one completion produces
  `23E8/190A/2B06/D40A/BABD`, while completing the old already-finalized
  payload again reproduces the reported wrong slot-4 `6AAF` result exactly.
- Each primary-effect candidate can now be marked "required as a secondary
  when not selected as primary". Marking both A and B compiles to the existing
  overlap-aware matcher as `(A primary and B secondary) OR (B primary and A
  secondary)` without duplicating one effect in the UI list.
- Legal terrain filtering now accepts multiple complete player-visible terrain
  results with OR semantics. The aggregate Hell option covers every native row
  whose visible result contains Hell without pretending that individual terrain
  effects are freely composable.
- A report that a propagated scroll's Grace changed is provisionally covered
  only when it is a rarity-4 first-reveal/completion case. Rarity-5 or an
  already-revealed record changing after network propagation requires a
  separate before/after save capture.
- The Seed accelerator build now ships native `sm_120` cubins and
  `compute_120` PTX for GeForce RTX 50-series GPUs, while retaining the
  existing `sm_75`, `sm_86`, `sm_89`, and `compute_89` images.
- CUDA availability now requires a successful kernel launch and synchronize,
  not merely a positive device count. If that compatibility self-test fails,
  effect-only searches select the existing DirectCompute GPU path instead of
  entering a CUDA-only R4 pivot path or silently scanning on the CPU.
- The native DLL exposes the last CUDA error code and failure stage so an
  unexpected R4 pivot failure no longer collapses into an untraceable generic
  message.
- The complete right-hand legal-search pane now has an independent vertical
  scrollbar, and the candidate-Seed list has its own vertical scrollbar. Long
  intersection reports no longer make the result and install controls
  unreachable.
- Local verification currently covers 413 passing tests, a withdrawn Tk UI
  startup smoke, a 1000x650 long-intersection scroll smoke, packaged executable
  startup and exact-path cleanup, an RTX 4070 Ti SUPER native-CUDA route, a
  forced CUDA-unavailable DirectCompute route with exact R4-primary parity,
  and the presence of an `sm_120` cubin in the rebuilt DLL. Physical RTX 5090
  execution and the first real rarity-4 reveal remain player-side acceptance
  evidence.
- GitHub Actions run `33648790533` passed the same 413-test suite, built the
  one-file executable, signed `latest.json`, and published the stable Release.
  The downloaded 17,927,879-byte asset passed Ed25519 manifest verification,
  SHA-256 `E520B92C5A70462399D5898B1E85E41420D7E82AB17745274DFE6C2832EDFD7A`,
  packaged startup, exact-path cleanup, and the public stable-update check.

## Stable v0.6.8 changes

- Fuses natural Seed construction, terrain, enemy, scratch-enemy, and ordered
  special-rule filtering into bounded native calls. CUDA keeps intermediate
  candidates on-device and returns only final survivors; the same ABI has an
  exact native CPU implementation for explicit no-CUDA fallback.
- Rejects failed effect constraints before auxiliary generation instead of
  sending every pivot survivor through repeated CUDA/Python/CUDA stages.
- Builds the DirectCompute effect shaders into the DLL as bytecode. Product
  startup no longer compiles the large HLSL programs on the first search.
- Makes one-wildcard preimage collection continue across internal pages until
  the requested count, advertised budget, cancellation, or family exhaustion.
  This fixes the reported first-search result count stopping at one.
- Replaces the no-CUDA hard rejection with a user confirmation dialog. CPU is
  still never selected silently, and supported effect stages continue through
  cross-vendor DirectCompute on AMD, Intel, or NVIDIA.
- Adds an independent vertical scrollbar and mouse-wheel routing to the local
  scroll editor's right pane for 1080p access.

## Published v0.6.7 implementation

The v0.6.7 baseline includes the following work:

- exact PC v2.01 executable detection and a separately validated native
  runtime profile, while retaining PC v2.00.02 support;
- byte-identical v2.01 effect, enemy, terrain, and special-rule resources;
- product handling for the v2.01 feature-flag-9 rarity-5 header cap without
  changing the verified effect-slot algorithm;
- exact same-name enemy-form choices backed by a shared trilingual registry;
- regenerated enemy-role and legal-combination catalogs with 148 player-facing
  entries while preserving 142 native localized identities;
- a hybrid R4 DirectCompute matcher: one or two independent ordinary-effect
  groups run the exact GPU completion finalizer, while three or more retain the
  cheaper lossless `N-1` stage filter; both paths independently replay GPU
  survivors through the exact CPU finalizer;
- removal of the incomplete stage-one R4 inverse from product final-record
  routing and enforcement of a single advertised batch-trial budget;

The following pre-existing editor/runtime work is retained from the
v0.6.5/v0.6.6 development line:

- `nioh3_scroll_editor/savegame.py`
  - Adds exact read/patch helpers for the mapped scroll header: playthrough,
    mirrored level, mirrored recommended level, Seed, mirrored rarity, and
    transfer count.
- `nioh3_scroll_editor/app.py`
  - Adds Seed/playthrough-derived terrain, grouped-enemy, and ordered-rule
    preview.
  - Imports the selected legal-search candidate Seed into the local editor.
  - Edits header fields and all seven unrestricted effect slots in one existing
    backup-gated transaction.
  - Shows the verifiable native raw-value set as guidance while retaining the
    full uint32 local input domain.
- `nioh3_scroll_editor/runtime_auxiliary_override.py`
  - Installs an exact-signature-gated rel32 trampoline at v2.00.02 RVA
    `0x20DD558`.
  - Matches one displayed Seed and reuses native enemy-vector capacity; it does
    not call an unverified game allocator.
  - Supports repeated enemy keys, one terrain enum, and three ordered rule keys.
  - Writes both the enemy lookup key and its exact native role into each inner
    descriptor. The earlier lookup-key-only overwrite could leave a name that
    looked correct in details but was internally inconsistent for challenge
    consumption.
  - Restores the original instruction on stop/app close and intentionally
    retires the 4 KiB trampoline until game exit to avoid an unload race.
- `test_beta_editor.py`
  - Adds exact byte-boundary, validation, auxiliary-formatting, raw-value-hint,
    runtime-profile, trampoline, and rel32 regression coverage.

Validation completed from source without touching a real save:

- `py_compile` passed for the modified modules and test file.
- 79 editor tests passed.
- 408 release-reproducible repository tests passed after rebuilding both native
  accelerators for the v0.6.8 working tree.
- At a live PC v2.01 title screen, 10,000 rarity-3 and rarity-4 native Seeds
  passed their complete parity gates, while 10,000 rarity-5 Seeds matched in
  every effect slot and differed only by the documented feature-flag-9 header
  cap. A final 64-Seed release-gate replay for each rarity also passed.
- Repeated native auxiliary descriptors matched the v2.00.02 control vectors,
  and read-only save decryption retained the 400-slot record layout.
- A post-change NVIDIA route matrix covered primary-only, unrestricted
  ordinary-effect, rule-only, enemy-only, terrain-only, and mixed requests.
  The three-rule report case scanned 100,000,000 mathematical cursors in about
  0.94 seconds, compared with about 3.08 seconds for 10,000,000 cursors before
  fusion. The UI-level automatic continuation found Seed `252159350` in its
  tenth 100,000,000-cursor page, with cursor 924,000,000 after about 7.75
  seconds. These are local source benchmarks, not other-vendor results.
- A withdrawn Tk application constructed and completed idle layout with
  `UI_SMOKE_OK`; the runtime pickers contain 487 legal enemy keys, 20 terrain
  enums, and 277 enabled rule keys including `None`.
- A running v2.00.02 process accepted and safely removed the application-owned
  trampoline. A three-Ichimokuren override was previously observed in details
  and a challenge, but the later exact-role write has source tests only and
  needs a new hit-backed challenge acceptance pass.

The user approved publishing the PC v2.01 compatibility release before a new
player-side challenge acceptance pass. The application offers deliberately
impossible layouts such as three copies of Ichimokuren as temporary runtime
state. It does not misrepresent enemy, terrain, or rule edits as save
persistence: they revert when the hook stops, the game regenerates the
descriptor, or the process restarts. Direct save-only persistence remains
unsupported.

## User decisions that remain authoritative

- Support rarities 3, 4, and 5. Rarity 5 is lower priority, not removed.
- The local editor must allow unrestricted edits even when the result is not
  native-legal or propagation-safe.
- Full header/effect editing is implemented in the working tree. Enemy, terrain,
  and special-rule customization is intentionally temporary runtime state, not
  a falsely persistent save edit.
- Show native raw-value ranges as guidance, but continue allowing any `uint32`
  raw value for local editing.
- Keep all source code and internal developer documentation in English. Speak
  to the user in Simplified Chinese.
- Preserve authorship as `MasterBayesian & Saber_Li`, the QQ group, and GitHub
  links in product-facing materials.
- Challenge-completion reroll prediction was abandoned as a product TODO on
  2026-09-01. Preserve the frozen evidence in `reroll-generation.md`, but do
  not resume implementation or request more captures.

## Remaining order

1. Do not publish the frozen checkpoint as a product release without a separate
   user instruction.
2. Continue from `FRONTEND_V2_FOUNDATION.md` and the forthcoming Figma design.
   The prior Astra audit has been consumed; do not repeat a global audit.
   Preserve the frozen service, policy, transaction, identity, and test
   contracts while migrating the remaining feature paths.
3. Collect any still-missing game-runtime acceptance evidence when the matching
   feature is changed or prepared for release. Automated tests are not gameplay
   acceptance.
4. Treat purple/empowered enemies and independently selectable possessed
   Underworld forms as P3 research. Do not add guessed forms before live
   validation, and do not let research block product development.
5. Validate and tune D3D11 compute on an AMD discrete GPU only when suitable
   hardware becomes available. Integrated-GPU parity is not a discrete-GPU
   performance result.

## Authoritative references

- `docs/knowledge/versions/pc-v2.00.02/project-status.md`
- `docs/knowledge/versions/pc-v2.00.02/effect-and-seed-solving.md`
- `docs/knowledge/versions/pc-v2.00.02/enemy-generation.md`
- `docs/knowledge/versions/pc-v2.00.02/special-rule-feasibility.md`
- `docs/knowledge/versions/pc-v2.00.02/scroll-legality.md`
- `docs/knowledge/versions/pc-v2.00.02/save-and-propagation.md`
- `docs/knowledge/versions/pc-v2.00.02/reroll-generation.md`
- `docs/knowledge/versions/pc-v2.00.02/evidence-register.md`

Before changing code, read this handoff and `project-status.md`, inspect the
current diff, and verify every claimed capability against the listed evidence
boundary. Do not infer live-game parity from compilation, tests, or static
analysis alone.

## Search UI demo revision 2 (2026-09-09)

The first demo was rejected by the user. A separate replacement now lives in
`apps/search-demo-v2`, with a self-contained review artifact at
`deliverables/frontend-v2/search-ui-demo-v2/index.html`. It uses full exported
catalogs and 192 bounded NG3 offline samples, Chinese UI, complex selections,
rarity-correct scroll inspectors, top-right contacts and bottom-left settings.
Strict TypeScript and 21 browser checks passed; this is UI-demo evidence only.
No game/save/GPU operations or production worker integration were added.
QQ protocol dispatch is wired, but successful native group-page opening was
not observed. See the delivery README for explicit limitations and evidence.
User visual acceptance is pending. Do not treat this as final Figma approval.

The user selected palette D (warm gray / teal) for the search demo, explicitly retaining section-specific colored backgrounds. Applied in apps/search-demo-v2; 33 browser checks pass. This approves the palette direction, not final UI acceptance.


Cart/UI revision: fixed exclusive accordions, bounded selection overlay, wheel/key/slider result navigation, padded scroll rows, cart and addition-mode UI, editor persistent/temporary groups and undo/redo. New batch application primitives and limitations are documented in docs/knowledge/CART_BATCH_ADDITION.md. Production worker/broker exposure and live acceptance remain outstanding.



### Current UI review build — 2026-09-09 follow-up

See the first section of `deliverables/frontend-v2/search-ui-demo-v2/README.md`
for the current evidence boundary. It supersedes the older demo counts above.
The latest user direction makes only ordinary effects sortable; enemy/rule drag
changes OR-group membership only. Groups are bounded rounded rectangles with
wrapped text, internal scrolling and a count. Grace selection is a default OR
set. Batch size is now capped at 20, and cart subsets and editor cart-seed selection
are implemented. Section return-to-top colors follow their parent backgrounds.
Native raw values are exported through the existing Python implementation for
3,427 effects, levels 0–180, R3–R5; 1,500 comparisons passed. Existing temporary
override contracts do not expose challenge capacity, so it is read only.
Validation: strict TypeScript, 71 browser assertions, and seven query-semantic
checks passed. This is UI and offline catalog evidence, not live-game acceptance.
Actual worker/broker integration of the review UI and batch primitives remains
outstanding. No game/save write, commit, release or publication occurred.
