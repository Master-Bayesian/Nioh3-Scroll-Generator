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
  public-release evidence.
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
