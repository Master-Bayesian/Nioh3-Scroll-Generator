# v0.8.1 integration continuation — 2026-09-21

Status: **active decision record; development only**. This document maps the
remaining source-matrix work to bounded evidence and gates. It does not change
the published v0.8.0 product, authorize a write, or authorize a v0.8.1 package.

## Evidence closed in this continuation

- **Browser continuation repair.** `EquipmentBrowser` now clears stale rows and
  selection before a read, anchors a session to process PID/creation FILETIME
  and observed slot count, rejects mixed-session/count responses, and treats an
  explicit refresh as a new slot-0 session. The repeatable artifact records 48
  E2E checks, TypeScript `--noEmit`, and the 641-message locale audit; no live
  write, packaged acceptance, or stable item identity is claimed. See
  [browser-triage](D:/Nioh3_v080_deliverables/deliverables/v081-integration-continuation-20260921/browser-triage/REPORT.md)
  and `browser/verification.json`.
- **Private P3/save correlation.** P3 raw slot 0 (`0xF0`, 240 bytes) is
  byte-identical to one private decrypted-save candidate at the declared
  equipment offset (`240/240`, ID `0xF6E8`, level 180, plus 20, and all listed
  effect IDs). This is a candidate same-item read comparison only: no owner,
  region bound/capacity, write semantics, or save/reload closure. Raw private
  bytes remain outside product and Pro packages. See
  [save-correlation](D:/Nioh3_v080_deliverables/deliverables/v081-integration-continuation-20260921/save-correlation/REPORT.md).
- **Catalog source decision.** Shipped resources contain final-effect names and
  32 special-rule item qualifiers, not a general equipment-name map. The
  save-editor code is Apache-2.0, but its item/effect JSON is explicitly
  unlicensed/unclear; CT and Pro name data carry no redistribution grant. The
  selected-file local path is implemented for a user-selected
  `items_little_endian.json` read as raw bytes; its separate native debug
  acceptance records 60 checks and 696 locale messages. It is not an
  arbitrary-directory, CT, or 7z importer. A user-provided local file does
  not require a redistribution grant, while bundling vendor data does; no
  game/save write is allowed. See
  [catalog-source](D:/Nioh3_v080_deliverables/deliverables/v081-integration-continuation-20260921/continuation/catalog-source/REPORT.md).
- **Legality continuation.** `INVENTORY-LEGALITY-AUDIT` is **in_progress** with
  bounded backend protected dispatch (snapshot/source SHA/context proof), a UI
  six-row fixture accepted by 28 focused checks, TypeScript and 682-message
  locale checks across zh-CN/en-US/ja-JP, and a corrected native debug route
  that reaches `save.inventory`, `save.operations`, and `save.audit_scrolls`.
  The catalog native run is separate provenance (60 checks/696 messages), not
  a joint-run claim. The bounded target is PC `2.0.2.0`, mapped type `0xE604`
  (NG3/R3/R4); R4 stage-one and final records are separate. Every current row
  remains `INSUFFICIENT_DATA`; packaged acceptance and evidence for the actual
  normal-input domain remain open. Replay matches are diagnostic only. R5,
  equipment and soul-core hard rules remain unsupported
  (`equipment_hard_terminal_rules_enabled = 0`). See
  [legality-triage](D:/Nioh3_v080_deliverables/deliverables/v081-integration-continuation-20260921/legality-triage/REPORT.md),
  the [backend dispatch artifact](D:/Nioh3_v080_deliverables/deliverables/v081-integration-continuation-20260921/scroll-audit/e2e-audit-result.json),
  and the [UI acceptance report](D:/Nioh3_v080_deliverables/deliverables/v081-integration-continuation-20260921/scroll-audit-ui/REPORT.md).

The native-normal-generation Pro packet is delivered as static/offline research.
It does not establish current-game natural generation, legal roll ranges,
insertion, persistence, or a complete legality engine.

## Source-matrix disposition

| Matrix family | Remaining disposition | Gate / bounded evidence |
| --- | --- | --- |
| Save container, integrity, identity | **Implementation + evidence:** existing Rust checksum/envelope and scroll transaction paths remain the product baseline; transaction outcome repair has focused synthetic E2E evidence. **Needs current evidence:** non-scroll region identity, `-cs` integrity-bypass meaning, `-sid` transfer semantics, and save/reload after any new field write. | Keep generic raw `WriteMain` as a transaction test surface only. No typed non-scroll writer until an operation-specific owner/field contract exists. |
| Character and equipment records | **Read evidence:** internal browser page plus exact-build P3 candidate and one private save-byte correlation. **Needs current fields/owner/save-reload:** equipment flags, widths, effects, equipped tails, logical key/owner and edit readback. Static upstream spawn is a canned/unseeded template, not legal generation. | `EQUIPMENT-EDIT` remains future. T1/T5 immutable patch/preview is a candidate only after purpose-qualified P3/P7 evidence; do not add schema-only scaffolding. |
| Inventories, catalogs, import/export | **Implementation + evidence:** scroll inventory/generation/save path and internal effect/catalog adapter remain bounded. **Needs current field/ownership/save-reload:** usable quantity/region, storage and inventory keys, section/character import, any non-scroll create/transfer. **Absent upstream:** item removal and spreadsheet export; storage has no spawn path. **Catalog:** selected-file `items_little_endian.json` raw-byte read is implemented with bounded backend/UI/native debug evidence (60 checks/696 messages in a separate run), not bundled and never a game write. | Preserve numeric fallback and source/hash/version/locale/namespace provenance. Do not port raw block replacement or upstream ID remapping as product behavior; no arbitrary-directory, CT, or 7z importer. |
| Supplied CT (v2.00.02) | **Static/runtime-unvalidated:** equipment editor, currencies, stats and inventory controls remain evidence inputs. **Tracked:** settings mirrors are not combat presets. **Placeholder/backlog:** 131 `+`/NEXT-TBL-UPDATE counter rows remain retained source inventory pending owner decision; Titles is author-declared broken; dropdowns are lookup definitions, not features. | Current version owner/field capture is required for currencies, stats, usable quantity and equipment edit; no old RVA guessing. |
| Pro trainer-side equipment research | **Delivered static package:** 1,063 equipment / 3,296 effects / 833 hell-skill catalogs and trainer-side contracts are reproduced. **Still open:** game module identity, normal-source provenance, natural RNG/legal generation, game-side insertion, persistence and current acceptance. | Treat the packet as a handoff/evidence artifact, not a product dependency or native normal-generation approval. |

## Current implementation candidates and stop conditions

1. Keep the browser continuation and read-only equipment page internal until a
   packaged/user-facing acceptance exists. Stable owner/key identity is a gate
   for editing/create, not a reason to hide read-only browsing.
2. Preserve the completed bounded scroll backend/UI/native-debug evidence;
   remaining work is packaged acceptance and actual normal-input-domain
   evidence. Change only that segment after root freeze, and preserve the
   four-state contract.
   Never turn replay miss, timeout, missing Seed, or context drift into an
   illegal verdict.
3. `CATALOG-LOCAL-IMPORT` is implemented for a user-selected
   `items_little_endian.json` file read as raw bytes, with separate bounded
   native catalog acceptance. It is not an arbitrary directory, CT, or 7z
   importer. A local user-provided file needs no redistribution grant; bundling
   vendor data does. The adapter must not infer namespaces across 2-/4-byte
   catalogs or write a save/game.
4. Currencies, core stats, usable quantity and basic equipment edit remain
   blocked on current owner/field evidence and save/reload acceptance. Inventory
   create/transfer, runtime tweaks, settings-as-features, counters, Titles and
   full equipment legality remain deferred under the matrix boundaries.

No product source, schema, catalog, save, game process or release asset was
changed by this record.
