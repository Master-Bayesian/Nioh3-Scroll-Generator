# Product feature catalog

Status: **published baseline v0.8.2; v0.8.3 is an unpublished candidate with
source/runtime/UI follow-up gates passed; final TypeScript/npm reruns passed.** See the
[publication record](../knowledge/TAURI_V082_PUBLICATION_20260928.md) and
[current 0.8.3 engineering record](releases/v0.8.3.md).

Status updated: 2026-10-02. The historical catalog is not fully reconciled with
the Rust migration; unchanged historical implementation anchors and entries
below are not current acceptance claims. This update reconciles only the
affected compatibility, installation, save-plan and live-addition slice.

This file is the durable index of shipped product behavior. It is deliberately compact: detailed technical and reverse-engineering evidence stays in `docs/knowledge/`, while planning state stays in the shared spreadsheet.

The initial entries below are grounded in the current player README and current handoff. They cover the primary shipped workflows but have not yet been reconciled screen by screen against the published v0.7.4 release. Future product work should verify and improve the affected entries rather than treating this bootstrap list as exhaustive.

## Shipped capabilities

| ID | Capability and player outcome | Entry point | Essential contract | Primary anchors |
| --- | --- | --- | --- | --- |
| `SCROLL-SEARCH` | Search for scrolls that match effects, values, grace, enemies, rules, terrain, level, capacity, type, playthrough, rarity, and (where supported) Crucible Wraith state. | Search workspace | NG3 enemy filtering uses the ordinary solo roster. The UI exposes an iOS-style `Crucible Wraith` switch only for identities with a native-table-eligible low-pool variant; arbitrary low-pool enemies never receive the option. Curse (中文一难) and Expedition/常世同行 are not user-facing filters. Enemy conditions use a compact single-line layout; the selected-conditions panel has increased vertical space, and its help entry is in the selected-conditions title bar. The native labels are zh-CN `地狱附身`, en-US `Crucible Wraith`, and ja-JP `地獄憑き`. | `apps/workshop/native-search.ts`; `nioh3_scroll_editor/search_application.py`; `nioh3_scroll_editor/search_worker.py`; `nioh3_scroll_editor/enemy_state_search.py` |
| `SCROLL-PREVIEW` | Preview generated results and known scroll IDs before selecting an operation. | Search results and preview cards | The preview must represent the exact candidate transferred to later actions. NG3 previews use the ordinary solo roster and show Crucible Wraith occurrences immediately after the enemy name with compact localized labels: zh-CN `附身`, en-US `Wraith`, and ja-JP `憑き`. The short label is center-aligned while its tooltip and accessible label retain the full official term. The former top explanatory banner is removed and its explanation is incorporated into the equipment and mission columns. There are no user-facing Solo/Expedition selectors or Curse controls. R4 display uses the finalized record while installation retains its matching stage-one record. | `apps/workshop/ScrollCard.tsx`; `apps/workshop/presentation.ts`; `apps/workshop/presentation-jsx.ts`; `apps/workshop/desktop-bridge.ts` |
| `SCROLL-COLLECTIONS` | Save candidates as favorites, compare them in the cart, and select an exact subset for addition. | Favorites and cart | Favorites and cart each cap at 50. Selection must retain broker-owned candidate identity across searches and pagination. Favorite cards use the same fixed geometry as the main preview card and support local search over scroll ID and visible card metadata. | `apps/workshop/collections.ts`; `apps/workshop/CartActions.tsx`; `apps/workshop/ScrollCard.tsx` |
| `SCROLL-ADD` | Add the current result directly or add a selected cart subset, either live or through a supported save path. | Preview actions and cart | Every write requires explicit confirmation, compatibility checks, an automatic verified backup, exact readback, and no-replay recovery. | `apps/workshop/CartActions.tsx`; `apps/workshop/prepared-live-batch.ts`; `apps/workshop/desktop-bridge.ts` |
| `SCROLL-EDIT` | Discover owned scrolls and edit persistent base fields and effect slots. | Scroll editor | The player reviews the selected save entry before mutation. Local effect combinations are not claimed to be canonical or seed-propagating. | `apps/workshop/Editor.tsx`; `apps/workshop/save-workspace.ts` |
| `SCROLL-COUNT` | Change the remaining challenge attempts for an owned scroll. | Scroll editor | The current count and requested count remain distinct. Temporary overrides must not be confused with persistent count edits. | `apps/workshop/CountEditor.tsx`; `apps/workshop/Editor.tsx` |
| `SCROLL-DELETE` | Delete a selected owned scroll. | Scroll editor | Deletion uses the protected save transaction and automatic backup path and remains available at the title screen. | `apps/workshop/Editor.tsx`; `apps/workshop/save-workspace.ts` |
| `MISSION-OVERRIDE` | Temporarily override enemies, terrain, special rules, or challenge capacity for the active scroll mission. | Scroll editor temporary controls | Overrides are runtime-only, visibly marked as temporary, and provide explicit stop and recovery behavior. They do not rewrite permanent scroll data. | `apps/workshop/Editor.tsx` |
| `SAVE-BACKUP` | Browse, verify, restore, open, or recycle application-created backups. | Backup workspace | Restore creates a new pre-restore checkpoint and retains save identity and protected-operation checks. | `apps/workshop/BackupManager.tsx`; `apps/workshop/save-workspace.ts` |
| `APP-UPDATE` | Update and replace the install-free application executable through the signed update flow. | Update UI and Settings | After updater startup readiness, each launch checks once and prompts once when a newer update is available; the prompt is deferred while another dialog is open. Settings retains a manual `Check for updates` action. The update is verified before replacement, waits for the running process, supports rollback, and cleans bounded caches after successful startup. | `apps/workshop/Updates.tsx`; `apps/workshop/main.tsx`; `apps/launcher/`; `apps/tauri/` |
| `SUPPORT-DIAGNOSTICS` | Copy a bounded support log when an operation fails or when the player requests diagnostics. | Settings and automatic failure handling | Logs are size-bounded and rotated; copied diagnostics include enough operation context for support without proving a root cause. | `apps/workshop/main.tsx`; `apps/workshop/public-errors.ts`; desktop broker logging |
| `LOCALIZATION` | Use the desktop UI in Simplified Chinese, English, or Japanese. | Settings | Language choice applies to shipped product surfaces; native-speaker review remains separate acceptance. The separate interface-font-size setting has been removed; preview label/value typography is fixed and bounded by the current UI acceptance. | `apps/workshop/ui-locales.json`; `apps/workshop/ui-translations.tsv` |

## In-development capabilities (not shipped)

The entries below mix historical development evidence with the stated version
scope. Their individual status is authoritative: v0.8.2 editing is shipped,
while new v0.8.3 additions remain unpublished. No current-source test or old
package result alone promotes a candidate capability to shipped behavior.

| ID | Capability and intended outcome | Entry point | Essential bound | Primary anchors |
| --- | --- | --- | --- | --- |
| `EQUIPMENT-READ` | Browse owned equipment records and read their raw fields. This is the read stage of the single equipment/soul-core browsing-and-editing feature, not an independent product; soul-core field coverage is unverified. | In-development, read-only experimental equipment page | Read-only; no hidden developer-mode flag is required. Reuses the protected `runtime` read path (`runtime.inventory_snapshot`); adds no write, no new IPC write route, and no bundled catalog. Names come from shipped resources only, with an exact-ID fallback when a name is unknown. Internal browser continuation now clears stale state, anchors pages to process identity/container count, and rejects mixed sessions; the bounded E2E artifact records 48 checks. A private P3 slot-0/save record is byte-correlated as a candidate only. This remains one page of one debug session, not a whole-inventory capacity claim, not a write, not the one-file build, and not shipped or user-facing. Stable owner/key identity remains a gate for live-process edits and create; it is separate from the save-file candidate tracked under `EQUIPMENT-EDIT`. This source remains internal and not release-validated. | `crates/nioh3-data/src/equipment_catalog.rs`; `crates/nioh3-runtime/src/inventory.rs`; `crates/nioh3-protected/src/runtime_app.rs`; `apps/workshop/EquipmentBrowser.tsx`; `apps/workshop/desktop-bridge.ts`; `apps/tauri/verify-equipment-browser.mjs`; `D:/Nioh3_v080_deliverables/deliverables/v081-integration-continuation-20260921/browser-triage/REPORT.md`; `D:/Nioh3_v080_deliverables/deliverables/v081-integration-continuation-20260921/save-correlation/REPORT.md` |
| `CATALOG-LOCAL-IMPORT` | Read a user-selected `items_little_endian.json` file as raw bytes for offline name lookup. | **Internal implementation complete; no release entry point** | Not bundled and never a game/save write. The adapter retains source/hash/version/locale/ID-namespace provenance and keeps unknown IDs numeric. The bounded native catalog run is a separate debug acceptance (60 checks, 696 locale messages), not packaged or live-game acceptance. This is a selected-file local-read path only: not an arbitrary-directory, CT, or 7z importer, and not permission to bundle supplied vendor catalogs. A user-provided local file does not require a redistribution grant; bundling vendor data does. | `crates/nioh3-data/src/equipment_catalog.rs`; `apps/workshop/LocalCatalogImport.tsx`; `D:/Nioh3_v080_deliverables/deliverables/v081-local-catalog-20260921/ui/REPORT.md`; `D:/Nioh3_v080_deliverables/deliverables/v081-local-catalog-20260921/native/native-v081-evidence.json` |
| `LEGAL-EQUIPMENT-GENERATION` | Generate equipment records with the game's seed-driven generator and search seeds that yield requested effects; queue generated results for a character save. | **v0.8.3 candidate implementation; unpublished; package acceptance pending** | `runtime.equipment_seeds` searches the complete 16-bit seed domain for the zero-context item-grant route; enemy/region drop variants are excluded and no-match claims are route-specific; `save.prepare_character_edit` regenerates and adds records through the encrypted-save transaction using save-derived progress and played-difficulty checks. The new generator evidence targets PC v2.02 (file version 2.0.2.0); no older-version parity is claimed. Offline evidence: 900-record whole-record parity, exhaustive 65,536-seed accounting, a 1,000+ case replay audit, synthetic PC v2.02 seeded-add readback, the repaired group-key mutation host regression (47.80 s before / 48.19 s after), and UI typecheck, 1,115-message locale audit, and 45/45 route-scoped mocked-bridge browser checks. This does not drive live natural drops or write to live-game inventory. Real-save/game validation, in-game reload, native Tauri/package acceptance, and publication remain open; the historical 1,013/1,318 selected-field owner-save replay predates the group-key repair and is diagnostic only. | `crates/nioh3-domain/src/equipment_generation.rs`; `crates/nioh3-protected/src/equipment_seeds.rs`; `crates/nioh3-protected/src/save_app.rs`; `apps/workshop/CharacterEditor.tsx`; `D:/Nioh3_v080_deliverables/deliverables/codex-v083-handover-20260929/backend/REPORT.md`; `D:/Nioh3_v080_deliverables/deliverables/codex-v083-handover-20260929/backend/repair-group-key/REPRODUCER.md`; `D:/Nioh3_v080_deliverables/deliverables/codex-v083-handover-20260929/ui-scope/verify-seed-equipment-scope-verified.md`; `docs/knowledge/V083_EQUIPMENT_SEED_GENERATION_RESEARCH_20260929.md` |
| `EQUIPMENT-EDIT` | Edit, remove, or add equipment in a selected character save. | **Character Editor: v0.8.2 shipped editing; v0.8.3 candidate add/remove** | `save.character` reads equipment and `save.prepare_character_edit` patches occupied records, removes occupied records, and adds new items through the save transaction. Seeded additions use `LEGAL-EQUIPMENT-GENERATION`; other construction does not carry that generation claim. The synthetic PC v2.02 host E2E checks an existing-record patch, removal, ordinary addition, seeded generated-record bytes, and inventory counters. The UI run uses a scripted bridge. Historical live removal has an owner-confirmed save/title/reload sample; see V083_CC_CONTEXT_RECONCILIATION_20260929.md. This intake does not establish the new seeded save-add flow in game, final native Tauri/package acceptance, or publication. | `crates/nioh3-protected/src/save_app.rs`; `crates/nioh3-protected/tests/host_character_edit.rs`; `apps/workshop/CharacterEditor.tsx`; `D:/Nioh3_v080_deliverables/deliverables/codex-v083-handover-20260929/backend/REPORT.md`; `D:/Nioh3_v080_deliverables/deliverables/codex-v083-handover-20260929/ui-scope/verify-seed-equipment-scope-verified.md` |
| `INVENTORY-LEGALITY-AUDIT` | Independently scan owned scrolls and equipment (soul-core coverage explicit and unverified until captured) for impossible effect combinations and other validated rule violations. | **In progress; bounded backend/UI/native debug evidence complete; no release entry point** | The bounded slice has synthetic protected dispatch with snapshot/source SHA/context proof, a UI six-row fixture accepted by 28 focused checks, TypeScript and 682-message locale audit across zh-CN/en-US/ja-JP, and a corrected native debug route that reaches `save.inventory`, `save.operations`, and `save.audit_scrolls`. All six current rows remain `INSUFFICIENT_DATA`; packaged/release acceptance and actual normal-input-domain evidence remain open. Replay matches are diagnostic only. R5, equipment and soul-core hard rules remain unsupported (`equipment_hard_terminal_rules_enabled = 0`). Mismatch, timeout, missing Seed/context, or version drift never becomes an illegal verdict. No write, repair, anti-cheat claim, or whole-record history proof is enabled. | `D:/Nioh3_v080_deliverables/deliverables/v081-integration-continuation-20260921/scroll-audit/e2e-audit-result.json`; `D:/Nioh3_v080_deliverables/deliverables/v081-integration-continuation-20260921/scroll-audit-ui/REPORT.md`; `D:/Nioh3_v080_deliverables/deliverables/v081-integration-remaining-20260921/handoff/evidence/continuation/native-summary.json`; `V081_INTEGRATION_CONTINUATION_20260921.md` |

### In progress: equipment legality generation and inventory audit

October 2 admission follow-up affects `GAME-INSTALLATION`, `SCROLL-ADD`,
`LIVE-EQUIPMENT-ADD`, `SCROLL-COUNT`, `MISSION-OVERRIDE` and native search/preview.
Known live scroll/equipment addition validates the operation's registered
binding and transaction evidence without global compatibility consent. Code,
layout, identity, ownership, bounds, backups, receipts and readback remain
mandatory. Equipment uses a source/backup checkpoint bound to the reviewed
operation and rechecked at execution. Generation resources load only for
operations that consume them; control/status/recovery do not depend on their
startup availability. Other compatibility-consent paths retain their reviewed
plan, unchecked confirmations and durable verified-backup audit.

Primary current coverage is 2.0.2.0; retain evidenced 2.0.1.0 scroll/count/native
bindings and the existing 2.0.0.2 offline scope. Older native seeded equipment
insertion and Ver2.00.01 with missing samples/bindings remain unsupported.
Unknown structure matches do not authorize writes. Existing support code,
mappings, runtime data and regressions must survive each new adaptation.
See the [version policy](../research/V083_DLC1_COMPATIBILITY_POLICY_20261002.md).
Completed source gates: runtime 207/207, policy 21/21, checkpoint 2/2, bootstrap
2/2, parser 5/5, offline prediction 1/1, Tauri packaged-resolution unit tests
11/11 plus offline-session 1/1, and npm 91 pass / 1 skip. Frozen browser tests
pass equipment 66/66 and compatibility 146/146 with 1,312 locale messages.
Actual-error mapping is covered by the final browser run and public-errors
17/17; no-game stored-receipt status/cancel passes 4/4. Actual recovery retains
process identity checks. Runtime again passes 207/207; protected all-target
Clippy and three format targets pass. Final TypeScript, the 1,312-message locale audit, and full npm (91 pass / 1 skip) pass; their logs are operation-final-typecheck.log and operation-final-npm.log. Test10 follows a clean commit;
its exact outer-package identity and acceptance belong in the external receipt.
No current packaged/game acceptance or publication is claimed. These changes
are absent from test9. See the [engineering record](releases/v0.8.3.md) for logs
and the external `TEST_PLAN.md` for acceptance cases.

October 2 source follow-up for the shared save-plan workflow and compatibility
backup confirmation: a canonical host-expired plan invalidates the review
without leaving an unknown-write fence; explicit backup retries copy current
sources while retaining previous copies. Lost acknowledgements still require
receipt recovery, and backup/consent checks remain. This is isolated source-only
acceptance, absent from test9. See [regressions and limits](../research/V083_OFFLINE_RELIABILITY_20261002.md).

`EQUIPMENT-EDIT` follow-up (September 30): the save-plan confirmation and
write/discard actions stay in a fixed card footer while details scroll. Explicit
changed-save/no-write refusals invalidate the plan and reload the save without
a phantom uncertainty fence; other failures keep existing receipt recovery.
Bounded UI/SaveSession/native/package evidence is recorded in
`docs/research/V083_PLAN_FOOTER_ACCEPTANCE_20260930.md`. Seed/effect values and
save backup/write contracts are preserved.

The complete September 30 v0.8.3 candidate additionally implements these
owner-requested entries; earlier seed-only test7 artifacts do not contain them.

| ID | Behavior | Status | Boundary | Evidence |
| --- | --- | --- | --- | --- |
| `LIVE-EQUIPMENT-ADD` | Equipment & items -> Add new equipment -> Live editing: native seeded preview, reviewed confirmation and one insertion, with durable status/cancel/recovery. | Source integration in verification; unpublished | PC 2.0.2.0 item-grant binding; no global compatibility-consent prerequisite. The operation requires a verified source/backup checkpoint covered by its plan and rechecked before execute, plus same-process/code/owner/container/counter checks and receipt recovery. 2.01/2.00.02 seeded insertion is unsupported. Current matching-package and in-game persistence acceptance are pending; save addition remains available within its supported scope. | `crates/nioh3-runtime/src/mutation/equipment_add.rs`; `crates/nioh3-protected/src/runtime_backup.rs`; `crates/nioh3-protected/tests/equipment_checkpoint.rs`; `apps/workshop/LiveEquipmentAdd.tsx`; `docs/product/releases/v0.8.3.md` |
| `SCROLL-COMPLETION-PREDICTION` | Scroll editor -> Reroll & extra-painting prediction: ordinary-completion replacement choices, decline and branch simulation up to five rounds. | Candidate implemented; unpublished | PC 2.0.2.0, NG3 0xE604 R4 only; unsupported semantics reject. Deferred offline prediction loads its explicit 2.02 context without a running game or replacing live-generation state; host_scroll_completion passes 1/1. Revelation trigger/automatic slot and replacement-combination search remain unknown/outside this panel. Read-only simulated records cannot be installed. | `crates/nioh3-domain/src/scroll_completion.rs`; `crates/nioh3-protected/tests/host_scroll_completion.rs`; `apps/workshop/ScrollCompletion.tsx` |
| `SCROLL-EXTRA-PAINTING-PREDICTION` | The same panel shows the painting trigger and effect after the selected completion decision. | Candidate implemented; unpublished | Retained native projections and binary32 thresholds; supported ordinary-completion scope only. Game confirmation of this new UI remains separate from historical native evidence. | `docs/knowledge/V083_SCROLL_COMPLETION_REPLACEMENT_RESEARCH_20260928.md`; `docs/knowledge/V083_MISSING_FEATURE_INTEGRATION_20260930.md` |
| `GAME-INSTALLATION` | Automatically locate the sole running image, or select a game executable on disk from Settings/startup recovery. | Source integration in verification; unpublished | Actual FILEVERSION and operation-specific bindings determine support; invalid selection never silently falls back. Host diagnostics remain available for unsupported startup. Runtime control/status/recovery construction is independent of optional generation resources; consuming operations still validate them. No multi-PID picker is implemented. Earlier test8-r2 checks are historical; current native chooser and matching-package acceptance remain pending. | `apps/tauri/src-tauri/src/game_install.rs`; `crates/nioh3-protected/src/runtime_app.rs`; `crates/nioh3-protected/tests/runtime_operation_admission.rs`; `apps/workshop/GameInstallation.tsx`; `docs/product/releases/v0.8.3.md` |

Owner-requested scope recorded 2026-09-21. Browsing and editing owned
equipment/soul-core records remain one feature: the read-only page above is
the in-development read stage, while `EQUIPMENT-EDIT` retains shipped v0.8.2 editing and adds an unpublished
v0.8.3 save-file candidate slice for existing-record edits, additions, and
removals. Historical live removal is separately documented; new seeded-add acceptance remains bounded. Two separate
features build on one shared, versioned generation/rule dataset:

- `LEGAL-EQUIPMENT-GENERATION` now has a v0.8.3 candidate for seeded record
  generation, exhaustive seed search, replay, and adding generated records
  through the save-file transaction. Backend parity and synthetic-save checks,
  plus the mocked-bridge browser flow, passed. Native natural-drop generation,
  live inventory insertion, real-save/game validation, and packaged acceptance
  remain open; see the table row for evidence and limits.
- `INVENTORY-LEGALITY-AUDIT` performs an independent read-only scan and reports
  a limited verdict only: proven reachable, a known rule violation with a
  precise reason, no violation found within documented checks, or insufficient
  data. It keeps structural memory validity, incomplete rule coverage, and
  proven PRNG reachability separate, and never accuses provenance, guarantees
  anti-cheat safety, or edits inventory.

Scroll rules and replay remain the first target for
`INVENTORY-LEGALITY-AUDIT`. The protected dispatch, bounded UI acceptance, and
corrected native debug route are complete for that diagnostic slice; all
current rows remain `INSUFFICIENT_DATA` pending packaged acceptance and actual
normal-input-domain evidence. Equipment rules have zero enabled hard-terminal
rules. The save-file edit path under `EQUIPMENT-EDIT` is separate from this
read-only audit and does not establish legality for custom records or enable
audit-driven repair. Neither legality feature is a v0.8.1 shipping commitment,
and no full rules engine is implemented by this record.

### v0.8.1 continuation record (2026-09-21)

The compact disposition and evidence map is maintained in
[`V081_INTEGRATION_CONTINUATION_20260921.md`](../knowledge/V081_INTEGRATION_CONTINUATION_20260921.md).
It records the completed browser-session repair and 48-check artifact, the
private 240-byte P3/save candidate correlation, the catalog local-read option,
the source-family remaining gates, and the scroll-only legality audit status.

## Known reconciliation work

- The enemy-state search and preview implementation is packaged and published in v0.7.4. Its acceptance did not add a live-game write, save mutation, or persistence claim; keep its bounded PC v2.01 / NG3 scope visible.
- Preview cards now use paired label/value typography for the recommendation level and challenge-count fields; the English title overflow case is covered by the native WebView2 acceptance.
- The current UI micro-tuning shipped in v0.7.4 and is covered by a native WebView2 bounded acceptance. Local evidence is recorded at `deliverables/v074-ui-acceptance-short-labels-20260914/verification.json`: all tested views fit at normal and maximized geometry, the selected-conditions and compact enemy layouts remain reachable, localized Wraith labels are centered inline, and favorites match the fixed preview geometry with local metadata search. This does not establish live-game, save, or persistence acceptance.

- Reconcile every v0.7.4 packaged screen against this catalog before the next broad UI migration is declared complete.
- Add focused regression and acceptance links when an entry is next changed; do not perform a repository-wide evidence backfill during an unrelated fix.
- Keep intentionally unavailable or coming-soon tools separate from shipped capabilities until their workflows and safety boundaries are accepted.
