# Nioh 3 scroll research knowledge base

This directory is the durable, versioned entry point for conclusions that must
survive chat loss, handoffs, and future game updates. Start with [the
documentation guide](../README.md), then use the classified map below before an
older checkpoint. A date or release number in a filename is not evidence that a
document remains current.

## Classification rules

- **Canonical/current** owns the present product decision, procedure, or
  accepted evidence boundary.
- **Workflow/reference** is reusable implementation or operational guidance;
  dated observations remain scoped to their recorded context.
- **Active research/handoff** transfers open work or preserves a still-relevant
  research boundary; it is not release approval.
- **Historical snapshot** preserves a point-in-time decision, preparation, or
  acceptance record. Do not treat its release state, pending list, or candidate
  artifact as current unless the current handoff says so.

## Storage model

- `docs/knowledge/`: human-readable current conclusions and capability status.
- `nioh3_scroll_editor/data/`: pointer-free product resources and localization
  catalogs consumed by the application.
- `audit/`: append-only raw captures, disassembly, parity corpora, screenshots,
  and experiment evidence. Private saves and account identifiers must never be
  promoted into public documentation.
- `research/`: reproducible capture and reverse-engineering methods.
- tests: executable claims that prevent a later implementation from silently
  contradicting the recorded knowledge.

The knowledge documents do not replace raw evidence. They link a conclusion to
its resource hash, native RVA, captured vectors, and regression tests whenever
those are available.

## Evidence grades

| Grade | Meaning |
| --- | --- |
| `native-byte-parity` | Offline output was compared with native output byte-for-byte on a declared corpus. |
| `native-control-flow` | Native table consumers and branch behavior were recovered and supported by controlled vectors. |
| `native-table` | A runtime parameter table was captured read-only and verified by hash, but not every consumer is closed. |
| `observed` | Reproducible in-game observation without a complete code path. |
| `inferred` | Best current explanation; must not be used as a product safety gate. |
| `unknown` | The available evidence does not justify a conclusion. |

Every claim is scoped by game version, record type/playthrough, rarity, normal
or exceptional caller path, and generation stage. A numeric ID alone is never a
global semantic key.

## Complete top-level document map

### Canonical/current

- [Documentation index (this page)](INDEX.md) — complete discoverability map.
- [Current project handoff](CURRENT_HANDOFF.md) — current product, release,
  acceptance, and remaining-work position.
- [v0.8.0 Rust backend migration](V080_RUST_BACKEND_MIGRATION.md) — active
  architecture, delegation boundaries, staged parity gates and first domain slice.
- [Release runbook](RELEASE_RUNBOOK.md) — authoritative current Tauri release
  procedure; publication still needs owner authorization.
- [Game-version update pipeline](GAME_VERSION_UPDATE_PIPELINE.md) — fail-closed
  compatibility procedure for a new executable.
- [Install-free one-file delivery](TAURI_ONEFILE_DELIVERY_20260912.md) — current
  direct-launch package contract.
- [Title-save investigation approach reset](TITLE_SAVE_APPROACH_RESET_20260912.md)
  — current owner policy and evidence boundary for title-screen save handling.
- [PC v2.01 native possessed-enemy research resume](POSSESSED_ENEMY_RESEARCH_RESUME_20260912.md)
  — current bounded evidence through the same-seed expedition controls.
- [PC v2.01 possessed-enemy Pro review integration](POSSESSED_ENEMY_PRO_REVIEW_INTEGRATION_20260913.md)
  — recovered and live-validated descriptor-to-task origin; the upstream mode
  mechanism remains assigned to the current Pro handoff.
- [PC v2.01 possessed-enemy mode-upstream Pro integration](POSSESSED_ENEMY_MODE_UPSTREAM_PRO_INTEGRATION_20260913.md)
  — accepted static/native proposal plus accepted sequence C live result;
  the upstream mode mechanism remains unresolved.
- [PC v2.01 augmentation fork Pro integration](POSSESSED_ENEMY_MODE_UPSTREAM_PRO_INTEGRATION_20260913.md#augmentation-fork-follow-up)
  — accepted augmentation correction, verified offline body export, and the
  code-bodies-only Pro follow-up package; live materialization remains pending.
- [PC v2.01 augmentation code-analysis Pro return](POSSESSED_ENEMY_MODE_UPSTREAM_PRO_INTEGRATION_20260913.md#augmentation-code-analysis-pro-return)
  — integrated eight-group closure export and bounded key-reuse/materialization
  findings; one read-only frontier capture remains pending.
- [PC v2.01 materialization-frontier Run C](POSSESSED_ENEMY_MODE_UPSTREAM_PRO_INTEGRATION_20260913.md#materialization-frontier-run-c)
  — completed bounded live join with 10 linked tasks and zero mismatches; Pro
  handoff is ready, with no further game run in this phase.
- [PC v2.01 possessed-enemy entry-transaction live result](POSSESSED_ENEMY_ENTRY_TRANSACTION_LIVE_RESULT_20260913.md)
  — bounded mode-transaction-to-materializer join with exact Run D parity;
  the entry sub-question is closed, while the filter/preview product research
  continues; the v1 handoff is superseded by the v2 product-research package.
- [PC v2.01 Curse/expedition product-research Pro handoff v2](../../deliverables/Nioh3_PC_v2.01_Possessed_OneDifficulty_Expedition_Product_Research_Pro_Handoff_20260913_v2/)
  — current product-research handoff for filter/preview choices and offline
  class1, `+8F`, and `+E9` research; 111 files/110 payload hashes/111 ZIP
  files verified, SHA-256 `A9A42333ABFBD8CC78D122EB0F3E45BF1416341723DEFA9C236048C987500FE5`.
- [PC v2.01 enemy-states Pro integration](ENEMY_STATES_PRO_INTEGRATION_20260914.md)
  — backend/reference foundation for Curse (中文一难), possessed, and
  expedition state evidence; UI/worker integration and release remain pending.
- [PC v2.01 mode-sequence C live-result Pro handoff](../../deliverables/Nioh3_PC_v2.01_Mode_Sequence_Live_Result_Pro_Handoff_20260913_v1/)
  — accepted one-person-expedition sequence C handoff; validator passed with
  96 files. Matching ZIP SHA-256 is
  `ba2e1f07a6b4db1d6b5742d1314a02b8ab075703008709cacf4da03b6442bd1a`.
- [PC v2.01 mode-request reconciliation](../../deliverables/Nioh3_Mode_Reconciliation_v201_20260913/)
  — archived Pro correction and sequence-phase handoff; matching ZIP SHA-256
  is `7D08DC2FF52223FC502BCEDF9614DF48FD6E5C1604191768E624463CF519ACA3`.
- [PC v2.01 Crucible possessed-enemy research freeze](CRUCIBLE_POSSESSED_RESEARCH_FREEZE_20260911.md)
  — historical negative-control correction whose claim boundaries still apply.
- [PC v2.02 noop cleanup-failure Pro handoff (incident packet A)](../../deliverables/native-noop-cleanup-pro-20260920/)
  — bounded incident package for the accepted noop whose cleanup failed and the
  later access-violation dialog; validator passed (36 files, ZIP SHA-256
  `72d4c74dabb11e3dbadda937adda08a3cf3316b156b778c408caee34e3906073`). The
  package records the noop's real target-process writes and asks Pro for a graded
  causal conclusion; no root cause is claimed.

### Workflow/reference

- [Cart addition boundaries](CART_BATCH_ADDITION.md) — batch-add transaction
  invariants and implementation boundaries.
- [Frontend V2 integration guide](FRONTEND_V2_INTEGRATION_GUIDE.md) — nonvisual
  implementation contract; its workbench is not an approved final layout.
- [Live scroll insertion engineering](LIVE_ADD_ENGINEERING.md) — version-aware
  insertion design and evidence scope.
- [Research handoff workflow](RESEARCH_HANDOFF_WORKFLOW.md) — required
  reproducible-Pro-handoff and integration process.
- [V2 displayed recommended-level selection](V2_RECOMMENDED_LEVEL_SELECTION.md)
  — versioned resolver and display-rule reference.

### Active research/handoff

- [UI follow-up transfer](UI_FOLLOWUP_HANDOFF_20260912.md) — current transfer
  for the remaining fullscreen/DPI, modal, settings, and card issues.
- [Crucible possessed-enemy research](CRUCIBLE_POSSESSED_ENEMY_RESEARCH_20260910.md)
  — preserved open evidence; read the newer resume and freeze before using it.
- [Equipment catalog live-verification handoff](EQUIPMENT_CATALOG_LIVE_HANDOFF_20260902.md)
  — bounded catalog verification state and unresolved rows.
- [Live insertion and localization follow-up](LIVE_ADD_AND_LOCALIZATION_FOLLOWUP.md)
  — follow-up evidence and deferred experiment context.
- [V2 requirements and later research](V2_REQUIREMENTS_BACKLOG.md) —
  non-authoritative backlog and later research topics.
- [Rust backend migration review package (packet B)](../../deliverables/rust-migration-pro-review-20260920/README.md)
  — ready, pending Pro review; six unpublished crates, migration gates, current
  and v0.7.5-baseline Python peers, bundled product data.
- [v0.8.0 repair-wave r4 closure review](../../deliverables/Nioh3_v080_RepairWave_r4_Closure_Review_20260920/README.md)
  — completed predecessor review; RF01/RF04/contract idempotence passed and
  RF02/RF03/RF05 were repaired and closed by the r5 review.
- [v0.8.0 r5 final evidence link](../../deliverables/Nioh3_v080_R5_Evidence_Link_20260920/README.md)
  — independently confirmed `PASS_TO_LOCAL_RC`: exact debug host/workers,
  verifier sources, PC `2.0.2.0` selected context, three synthetic roles, and
  7/7 node results are linked in one validated record. The 39,963-byte
  independent return has SHA-256
  `a73e5c2c6d098b94c4b5f4a74b6490b9b0a7e783d5b6061a789388e874ee38a5`
  and leaves no repair-wave tickets open. This is not a release or PC v2.02
  write authorization; the reviewed evidence-link ZIP has SHA-256
  `714e7194b39053d6c9d64106ae64636ee530e6493441ab9de764f954b608958a`.

### Historical snapshots

- [Backend freeze before Frontend V2](BACKEND_FREEZE_BEFORE_V070.md) —
  2026-09-07 v0.6.10 freeze checkpoint, not a release state.
- [Previous current-project handoff](CURRENT_HANDOFF_PRE_V070_REVIEW_20260909.md)
  — superseded 2026-09-07 handoff retained for context.
- [Frontend V2 engineering foundation](FRONTEND_V2_FOUNDATION.md) — dated 0.7.0
  preparation/foundation record; consult the current UI transfer.
- [Legacy UI parity](LEGACY_UI_PARITY_20260909.md) — superseded preliminary
  parity/acceptance checkpoint.
- [Independent live-add executor](NATIVE_LIVE_ADD_EXECUTOR_20260909.md) —
  captured live-acceptance evidence with stated dimensions and limits.
- [Connected review UI](REVIEW_UI_BACKEND_CONNECTION.md) — superseded UI
  connection checkpoint.
- [Tauri v0.7.1 publication](TAURI_V071_PUBLICATION_20260910.md) — historical
  public-release evidence.
- [Tauri v0.7.2 publication](TAURI_V072_PUBLICATION_20260910.md) — historical
  publication record; current handoff says v0.7.2 is draft.
- [Tauri v0.7.3 publication](TAURI_V073_PUBLICATION_20260912.md) — historical
  public-release evidence for the current stable release before v0.7.4.
- [Tauri v0.7.4 publication](TAURI_V074_PUBLICATION_20260915.md) — current
  public-release evidence; the v0.7.5 entry below is the latest publication.
- [Tauri v0.7.5 publication](TAURI_V075_PUBLICATION_20260915.md) — current
  public-release evidence for the emergency search-continuation and icon fixes.
- [Tauri 2 migration and Electron withdrawal](TAURI2_MIGRATION_20260909.md) —
  historical migration/withdrawal record.
- [Title-screen save ownership research plan](TITLE_SAVE_OWNERSHIP_RESEARCH_PLAN_20260911.md)
  — superseded mandatory research route retained as evidence.
- [v0.7.0 hosted build fixes](V070_HOSTED_BUILD_FIXES_20260909.md) — dated
  hosted-checkout repair record.
- [v0.7.0 pre-push completion](V070_PREPUSH_COMPLETION_20260909.md) — local
  preparation checkpoint, not a publication record.
- [v0.7.0 RC1 follow-up](V070_RC1_FOLLOWUP_20260909.md) — historical RC1
  progress and evidence.
- [RC1 packaged live UI acceptance](V070_RC1_LIVE_UI_ACCEPTANCE_20260909.md)
  — bounded live UI session record.
- [v0.7.0 release-readiness review](V070_RELEASE_READINESS_REVIEW_20260909.md)
  — superseded readiness findings.
- [v0.7.2 follow-up fixes prepared for review](V072_FOLLOWUP_FIXES_20260911.md)
  — pre-integration review snapshot.
- [v0.7.2 Pro review patch integration](V072_PRO_REVIEW_INTEGRATION_20260911.md)
  — integration checkpoint; its title-save release block was superseded by
  current policy.
- [v0.7.2 title-save static recovery integration](V072_TITLE_SAVE_STATIC_INTEGRATION_20260911.md)
  — static-recovery checkpoint superseded for next-step policy.
- [v0.7.3 addition and navigation review](V073_ADDITION_UI_REVIEW_20260912.md)
  — earlier UI review checkpoint; later transfer items remain pending.
- [v0.7.3 local review delivery](V073_LOCAL_REVIEW_20260912.md) — earlier local
  candidate delivery; the current handoff identifies the later one-file path.
- [v0.7.3 release preparation](V073_RELEASE_PREPARATION_20260912.md) —
  preparation record, not publication proof.
- [v0.7.3 Pro review closure](V073_PRO_REVIEW_CLOSURE_20260912.md) — dated
  item-by-item integration evidence; the current handoff owns present status.
- [V2 parity implementation and acceptance boundary](V2_PARITY_IMPLEMENTATION_20260909.md)
  — dated implementation/parity checkpoint.

## Versioned knowledge

- [PC v2.00.02](versions/pc-v2.00.02/README.md) — version-scoped catalogs,
  generation, save, legality, and evidence register.
- [PC v2.01 approved compatibility profile](versions/pc-v2.01/README.md) —
  approved profile and scoped evidence register.

## Update procedure

For a new executable version:

1. Create a new version directory; do not overwrite the old one.
2. Capture the runtime tables again and record executable identity, RVA/AOB,
   locale, capture manifest hash, and table hashes.
3. Regenerate versioned catalogs from raw keys. Never copy names by assumption.
4. Re-run native parity corpora and structural regression tests.
5. Record changed, unchanged, and unknown subsystems in the capability matrix.
6. Mark old documents with `superseded_by`; never silently edit history into a
   different version's truth.

## PC v2.02 Pro handoff (final package v5, ACCEPTED for bounded Pro analysis)

Self-contained bounded handoff: D:\\Nioh3_v080_deliverables\\deliverables\\game-version-update-20260919\\pro-handoff\\nioh3-pc-v2.02-addon-revision-and-level-clamp-pro-handoff-20260919-v5 and .zip (137,038 bytes, 60 members, SHA-256 74E1576DF78F804D7734F2C948B3413665F2418255A79A5BE10D931B7764EDC9).
Accepted 2026-09-19 on independent closure review (archive-derived, read-only); that review also confirms the v4->v5 diff is 4 files (README.md, TASK_FOR_PRO.md, KNOWN_LIMITS.md, SHA256SUMS.txt) with 56 byte-identical members.
Status: accepted Pro handoff awaiting Pro analysis / next-probe design. Product PC v2.02 remains UNAPPROVED (product_enablement_allowed: false), no release, no packaging, and the overall compatibility goal is NOT complete.
Known limits: 24 raw records = second seed only (seed 1 raw unrecoverable); no revision artifact for the Q1 additive-effect question; 600/180 clamp consumer body not located; item/multiplier row-store addresses are runtime-only; table field meanings unknown.

## PC v2.02 P0 evidence corrections (2026-09-19)

Corrected evidence lives in
`deliverables/game-version-update-20260919/reports/P0-corrections-20260919.md`
with machine-readable companions `P0-verified-facts-20260919.json`,
`recommended-level-curve-42point-verified-20260919.json`,
`private-save-discovery-20260919.json`, `private-record-scan-20260919.json` and
`backup-record-comparison-20260919.json`.

- item row 3358 is `+0x84` in true row coordinates (`0x15514C`); the published
  `+0x8C` was a window-relative coordinate from a window that started 8 bytes
  early.
- optional_multiplier is a keyed diff: 0 keys removed, 3 added, 3 payload
  changes (`0xD7C3` 1400 -> 600 among them), 104 position shifts, 646
  metadata-only changes. The 105/101/546/85 -> 84 figures are rejected as an
  artifact of an 8-byte row-start key with dict collapse.
- The 42-row display curve is verified identical in the retained PC v2.02
  capture, but only as a signature-bound read-only capture;
  `tables/level_curve.bin` (501 rows) is not evidence for it.
- R3 masks `0x1B` and R5 keeps 10,000 full-record mismatches; both raw reports
  stay immutable and must not be summarised as full-record passes.
- The only game-recognized save is unchanged since 2026-09-14 and stores the
  probed scroll at internal recommended 1400 (display 700), also present in the
  9/2-era backup. The owner's 356 report is not yet identity-joined to that
  record, so it is a candidate observation, not proof about this scroll.

## Rust backend migration Pro review package (packet B, ready, 2026-09-20)

`deliverables/rust-migration-pro-review-20260920/` plus `.zip` (2,639,869 bytes,
464 files, ZIP SHA-256
`cc6eb6647f93de43450c08ed669ec42b49a5c2390bd6721aa122aeb20d9a55ba`). Status:
ready, **pending Pro review** - no review conclusion exists yet, nothing is
published, and no PC v2.02 write path is enabled by it.

It asks Pro for evidence-graded module decisions (`KEEP` / `LOCAL_REPAIR` /
`REFACTOR` / `REWRITE`, or `NOT_ASSESSED` / `INSUFFICIENT_EVIDENCE`) plus an
aggregate `KEEP` / `PARTIAL_REPAIR` / `FULL_BACKEND_REWRITE` recommendation,
each citing packaged paths and lines. Contents: the six unpublished crates with
all manifests and locks; the 28 migration gates and their fixtures; 53 current
Python peers and 54 baseline peers extracted from the published v0.7.5 commit
`533694ebad21906aecbb6ab5283e04e760ce6c09` (`v0.7.5^{commit}`; annotated tag
object `c4cfce1523aa10a2532b79319c1e16ad4fbd6ee7`); every repository product data
resource including the large versioned tables; packet A unpacked once as
incident reference; and portable standard-library validation.

Snapshot boundary: the package was built before this index entry, so the entry
is deliberately not inside it and no packaged hash depends on it.
